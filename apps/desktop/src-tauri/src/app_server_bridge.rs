use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    env,
    future::{poll_fn, Future},
    pin::Pin,
    str,
    sync::{Arc, Mutex, OnceLock, Weak},
    task::{Context, Poll},
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
#[cfg(test)]
use forgecad_app_server::FakeDeepSeekClient;
use forgecad_app_server::{
    compatibility::{
        AllowedHttpMethod, CompatHttpFuture, CompatibilityAdapter, CompatibilityHttpPort,
        LocalAgentEndpoint, PreparedCompatHttpRequest, MAX_RAW_COMPAT_BODY_BYTES,
    },
    recipe_preview_output_contract, recipe_preview_shape_program_role, ActionLoopConfig, AppServer,
    AppServerConfig, CancellationToken, CompositeRequestHandler, HandlerFuture,
    LifecyclePersistencePort, LifecyclePortError, LifecyclePortErrorKind, LifecyclePortFuture,
    NativeAgentRuntime, NativeAgentRuntimeConfig, NativeNotificationSink, NativePreviewArtifact,
    NativeProductToolExecutor, NativeProductToolExecutorConfig, NotificationFuture,
    ProductToolCancelFuture, ProductToolExecutorPort, ProductToolPortError,
    ProductToolPortErrorKind, ProductToolPortFuture, ProductToolRegistry, ProviderClient,
    ProviderToolCall, RecipePreviewOutputContract, RequestHandler, RestrictedGeometryError,
    RestrictedGeometryErrorKind, RestrictedGeometryFuture, RestrictedGeometryInput,
    RestrictedGeometryOutput, RestrictedGeometryPort, RestrictedGeometryReadback,
    RestrictedQualityProfile, SystemRuntimeIdentityClock,
};
use forgecad_app_server_protocol::{
    valid_stable_id, AppServerCursor, CompatHttpRequest, CompatHttpResponse, CursorPhase,
    LifecyclePersistenceCommand, LifecyclePersistenceResult, ProductToolExecutionRequest,
    ProductToolExecutionResult, ProductToolExecutionStatus, ProtocolHttpBody, ReplayParams,
    RpcError, SseNotificationParams, SseSubscriptionParams, SseUnsubscribeParams,
    COMPAT_BACKEND_UNAVAILABLE, HTTP_COMPAT_REQUEST_SCHEMA_VERSION,
    HTTP_COMPAT_RESPONSE_SCHEMA_VERSION, INPUT_TOO_LARGE, MALFORMED_UPSTREAM_EVENT,
    METHOD_COMPAT_SSE, METHOD_COMPAT_SUBSCRIBE, METHOD_EVENTS_REPLAY, METHOD_NOT_FOUND,
    REQUEST_CANCELLED, SSE_NOTIFICATION_SCHEMA_VERSION,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{http, AppHandle, Emitter, State};

use forgecad_core::{
    canonical_json as core_canonical_json, materialize_assembly_delta,
    normalize_persisted_shape_program, semantic_sha256, verify_forgecad_glb, AgentAssetChangeSet,
    AgentAssetVersion, AgentComponentRecord, AgentStructureSuggestion, AssetStage,
    AssetVersionStatus, BlockoutCandidate, CandidateStatus, ChangeSetStatus, CoreError,
    ExpandedComponentCandidate, ForgeCadGlbReadback, ObjectReference, QualityReport, QualityStatus,
    SurfaceAdornmentProgram,
};

use crate::asset_render_compat::{
    parse_asset_render_request, render_package_response, render_set_response, seal_render_set,
    AssetRenderCompatError, AssetRenderCompatRequest, SealedRenderSet,
};
use crate::rust_core_runtime::{RustCoreActiveDesignSnapshotReader, RustCoreRuntime};

const PROTOCOL_EVENT: &str = "forgecad://app-server/message";
const RESOURCE_SCHEME_HOST: &str = "localhost";
const MAX_SSE_EVENT_BYTES: usize = 1024 * 1024;
const MAX_K002_INTERNAL_JSON_BYTES: usize = 1024 * 1024;
const K002_INTERNAL_CAPABILITY_HEADER: &str = "X-ForgeCAD-K002-Internal-Capability";
const RESTRICTED_GEOMETRY_PROTOCOL_VERSION: &str = "forgecad.restricted-geometry/1";
const RESTRICTED_GEOMETRY_EXECUTE_PATH: &str = "/api/v1/internal/geometry/execute";
const RESTRICTED_GEOMETRY_CANCEL_PATH: &str = "/api/v1/internal/geometry/cancel";
const RESTRICTED_GEOMETRY_CAPABILITY_HEADER: &str = "X-ForgeCAD-Restricted-Geometry-Capability";
const MAX_RESTRICTED_GEOMETRY_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_RESTRICTED_GEOMETRY_RESPONSE_BYTES: usize = 128 * 1024 * 1024;
const MAX_RESTRICTED_GEOMETRY_GLB_BYTES: usize = 64 * 1024 * 1024;
const MAX_RESTRICTED_GEOMETRY_VIEW_BYTES: usize = 16 * 1024 * 1024;
const LIVE_ACCEPTANCE_BUDGET_OVERRIDE: &str = "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_BUDGET_OVERRIDE";
const LIVE_ACCEPTANCE_ENABLE: &str = "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE";
const LIVE_MVP_ACCEPTANCE_BUDGET_OVERRIDE: &str =
    "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_BUDGET_OVERRIDE";
const LIVE_MVP_ACCEPTANCE_ENABLE: &str = "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE";
const LIVE_MVP_ACCEPTANCE_CONFIRM: &str = "FORGECAD_DEEPSEEK_MVP_ACCEPTANCE_CONFIRM";
const LIVE_ACCEPTANCE_CONFIRM: &str = "FORGECAD_DEEPSEEK_DELTA_ACCEPTANCE_CONFIRM";
const LIVE_ACCEPTANCE_CONFIRMATION: &str = "I_UNDERSTAND_THIS_MAY_INCUR_PROVIDER_COST";

fn explicit_live_acceptance_budget_override() -> bool {
    (env::var(LIVE_ACCEPTANCE_BUDGET_OVERRIDE).as_deref() == Ok("1")
        && (env::var(LIVE_ACCEPTANCE_ENABLE).as_deref() == Ok("1")
            && env::var(LIVE_ACCEPTANCE_CONFIRM).as_deref() == Ok(LIVE_ACCEPTANCE_CONFIRMATION)))
        || (env::var(LIVE_MVP_ACCEPTANCE_BUDGET_OVERRIDE).as_deref() == Ok("1")
            && env::var(LIVE_MVP_ACCEPTANCE_ENABLE).as_deref() == Ok("1")
            && env::var(LIVE_MVP_ACCEPTANCE_CONFIRM).as_deref() == Ok(LIVE_ACCEPTANCE_CONFIRMATION))
}
// M109A/C108 production_concept assets use the same bounded ShapeProgram as
// preview but compile 80k-150k triangles and a 1K five-channel PBR set. A cold
// packaged arm64 worker measured just above the former 40-second ceiling.
// Keep this phase below the Product Tool/Turn budgets while allowing the
// reviewed production profile to finish within the 250/280-second Product
// Tool/Turn budgets; cancellation still posts the exact capability-bound
// tombstone and late results remain invisible.
const RESTRICTED_GEOMETRY_COMPILE_TIMEOUT_MS: u64 = 240_000;
// The frozen packaged sidecar starts a fresh bounded worker for render. A cold
// arm64 launch dominates the legacy software-raster phase even for bounded
// thumbnails. The user-facing workbench renders the production GLB directly
// with its one Three.js context; these four deterministic PNGs are only
// evidence thumbnails. The enclosing Product Tool and Turn retain their
// 250/280-second hard ceilings.
const RESTRICTED_GEOMETRY_RENDER_TIMEOUT_MS: u64 = 120_000;
const RESTRICTED_GEOMETRY_NETWORK_GRACE_MS: u64 = 2_000;
// Reqwest otherwise applies its shorter default whole-request deadline before
// the reviewed restricted-executor deadline. Keep the loopback transport able
// to carry the protocol's maximum 240-second compile request; each geometry phase is
// still bounded by its stricter compile/render timeout below.
const RESTRICTED_GEOMETRY_HTTP_TIMEOUT_MS: u64 = 245_000;
const RESTRICTED_GEOMETRY_RENDERER_ID: &str = "forgecad-agent-software-raster@1";
const RESTRICTED_GEOMETRY_REQUIRED_VIEWS: [&str; 4] = ["front", "iso", "side", "top"];
// A single ChangeSet may ask for the same production ShapeProgram more than
// once (preview download, confirm, quality and export). Keep the cache small,
// in-memory and process-local: the sidecar artifact handle is intentionally
// never persisted across a restart. This is an execution optimization, not a
// second asset/version truth.
const RESTRICTED_GEOMETRY_CACHE_MAX_ENTRIES: usize = 6;
const RESTRICTED_GEOMETRY_CACHE_MAX_BYTES: usize = 192 * 1024 * 1024;
const SSE_RECONNECT_DELAY: Duration = Duration::from_millis(150);
const TURN_CANCEL_DISCOVERY_ATTEMPTS: usize = 100;
const TURN_CANCEL_DISCOVERY_DELAY: Duration = Duration::from_millis(10);
const TURN_CANCEL_STABILITY_CHECKS: usize = 3;
const TURN_CANCEL_STABILITY_DELAY: Duration = Duration::from_millis(20);
const TURN_CANCEL_CORE_REGISTRATION_ATTEMPTS: usize = 200;
const TURN_CANCEL_CORE_REGISTRATION_DELAY: Duration = Duration::from_millis(5);
const NATIVE_BLOCKOUT_CANDIDATE_LIMIT: usize = 16;
const NATIVE_BLOCKOUT_SEGMENT_IDEMPOTENCY_LIMIT: usize = 32;
const NATIVE_BLOCKOUT_COMPAT_TTL_MS: u64 = 5 * 60 * 1_000;

thread_local! {
    static ACTIVE_CONNECTION_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[derive(Clone)]
pub struct AppServerBridge {
    inner: Arc<BridgeInner>,
}

struct BridgeInner {
    server: Arc<AppServer>,
    adapter: Arc<CompatibilityAdapter<LoopbackHttpPort>>,
    port: Arc<LoopbackHttpPort>,
    native_product_tools: Option<Arc<NativeProductToolExecutor>>,
    native_notifications: BridgeNativeNotificationSink,
    connections: Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
    pending_turn_requests: Mutex<HashMap<(String, String), PersistedTurnCancellationTarget>>,
    legacy_persisted_turn_cancellation: bool,
    compat_roundtrip_logged: std::sync::atomic::AtomicBool,
}

#[derive(Clone, Default)]
struct BridgeNativeNotificationSink {
    inner: Arc<BridgeNativeNotificationSinkInner>,
}

#[derive(Default)]
struct BridgeNativeNotificationSinkInner {
    server: OnceLock<Weak<AppServer>>,
    connections: Mutex<HashSet<String>>,
}

impl BridgeNativeNotificationSink {
    fn attach_server(&self, server: Weak<AppServer>) -> Result<(), String> {
        self.inner
            .server
            .set(server)
            .map_err(|_| "ForgeCAD native notification sink was attached twice.".to_string())
    }

    fn register_connection(&self, connection_id: &str) {
        if let Ok(mut connections) = self.inner.connections.lock() {
            connections.insert(connection_id.to_string());
        }
    }

    fn unregister_connection(&self, connection_id: &str) {
        if let Ok(mut connections) = self.inner.connections.lock() {
            connections.remove(connection_id);
        }
    }
}

impl NativeNotificationSink for BridgeNativeNotificationSink {
    fn publish(
        &self,
        notification: forgecad_app_server_protocol::NativeAgentNotification,
    ) -> NotificationFuture {
        let inner = self.inner.clone();
        Box::pin(async move {
            notification.validate()?;
            let cursor = AppServerCursor::decode(&notification.cursor)?;
            let method = notification.method();
            let params = serde_json::to_value(&notification).map_err(|_| {
                RpcError::internal("Native Agent notification could not be serialized.")
            })?;
            let Some(server) = inner.server.get().and_then(Weak::upgrade) else {
                return Err(RpcError::internal(
                    "Native Agent notification sink is unavailable.",
                ));
            };
            let connections = inner
                .connections
                .lock()
                .map_err(|_| RpcError::internal("Native connection registry is unavailable."))?
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            for connection_id in connections {
                if !server.connection_is_ready(&connection_id) {
                    continue;
                }
                // Thread lifecycle has no persisted AgentItem sequence, and
                // approval create/resolve share their referenced Item's one
                // sequence. Keep those events observable but transient so
                // they cannot collide with durable Item/terminal cursors.
                // Their authoritative state is available through thread/read
                // and approval/read after reconnect.
                let published = if notification.turn_id.is_none()
                    || notification.approval_id.is_some()
                {
                    server.publish_transient_notification(&connection_id, method, params.clone())
                } else {
                    server.publish_notification(
                        &connection_id,
                        method,
                        cursor.clone(),
                        params.clone(),
                    )
                };
                if published.is_err() {
                    // Durable lifecycle persistence is authoritative even if
                    // a renderer falls behind. The client must resync from
                    // item/list instead of failing a committed mutation.
                    let _ = server.publish_resync_required(
                        &connection_id,
                        "native_agent_notification_backpressure",
                    );
                }
            }
            Ok(())
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct ProtocolSendRequest {
    connection_id: String,
    frame: Value,
}

#[derive(Debug, Deserialize)]
pub struct ProtocolDisconnectRequest {
    connection_id: String,
}

#[derive(Debug, Serialize)]
pub struct ProtocolConnectResult {
    connection_id: String,
}

#[derive(Clone, Serialize)]
struct ProtocolEventPayload {
    connection_id: String,
    frame: Value,
}

impl AppServerBridge {
    #[cfg(test)]
    pub fn new(endpoint: &str) -> Result<Self, String> {
        Self::new_with_internal_capability(endpoint, "k002-test-capability".to_string())
    }

    #[cfg(test)]
    pub fn new_with_internal_capability(
        endpoint: &str,
        internal_capability_token: String,
    ) -> Result<Self, String> {
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        Self::new_with_native_provider(endpoint, internal_capability_token, provider)
    }

    #[cfg(test)]
    fn new_with_native_provider(
        endpoint: &str,
        internal_capability_token: String,
        provider: Arc<dyn ProviderClient>,
    ) -> Result<Self, String> {
        let endpoint = LocalAgentEndpoint::parse(endpoint).map_err(|error| error.message)?;
        let port = Arc::new(LoopbackHttpPort::new(
            endpoint.clone(),
            internal_capability_token,
        )?);
        let adapter = Arc::new(CompatibilityAdapter::new(endpoint, Arc::clone(&port)));
        let lifecycle: Arc<dyn LifecyclePersistencePort> = port.clone();
        let tools: Arc<dyn ProductToolExecutorPort> = port.clone();
        Self::from_components(adapter, port, lifecycle, provider, tools, None, true)
    }

    pub fn new_production(
        endpoint: &str,
        internal_capability_token: String,
        provider: Arc<dyn ProviderClient>,
        rust_core: Arc<RustCoreRuntime>,
    ) -> Result<Self, String> {
        let endpoint = LocalAgentEndpoint::parse(endpoint).map_err(|error| error.message)?;
        let port = Arc::new(LoopbackHttpPort::new_with_rust_core(
            endpoint.clone(),
            internal_capability_token,
            Arc::clone(&rust_core),
        )?);
        let adapter = Arc::new(CompatibilityAdapter::new(endpoint, Arc::clone(&port)));
        let lifecycle: Arc<dyn LifecyclePersistencePort> = Arc::new(rust_core.lifecycle_port());
        let registry = Arc::new(ProductToolRegistry::forgecad_v1().map_err(|error| {
            format!(
                "Could not initialize the immutable Product Tool registry: {}",
                error.message
            )
        })?);
        let geometry: Arc<dyn RestrictedGeometryPort> = port.clone();
        let native_product_tools = Arc::new(
            NativeProductToolExecutor::with_embedded_catalog(
                registry,
                geometry,
                NativeProductToolExecutorConfig::default(),
            )
            .map_err(|error| {
                format!(
                    "Could not initialize the Rust Product Tool executor: {}",
                    error.message
                )
            })?,
        );
        native_product_tools
            .attach_active_snapshot_reader(Arc::new(RustCoreActiveDesignSnapshotReader::new(
                Arc::clone(&rust_core),
            )))
            .map_err(|error| {
                format!(
                    "Could not attach the Rust ActiveDesignSnapshot reader: {}",
                    error.message
                )
            })?;
        port.attach_native_product_tools(Arc::downgrade(&native_product_tools))?;
        let tools: Arc<dyn ProductToolExecutorPort> = native_product_tools.clone();
        Self::from_components(
            adapter,
            port,
            lifecycle,
            provider,
            tools,
            Some(native_product_tools),
            false,
        )
    }

    fn from_components(
        adapter: Arc<CompatibilityAdapter<LoopbackHttpPort>>,
        port: Arc<LoopbackHttpPort>,
        lifecycle: Arc<dyn LifecyclePersistencePort>,
        provider: Arc<dyn ProviderClient>,
        tools: Arc<dyn ProductToolExecutorPort>,
        native_product_tools: Option<Arc<NativeProductToolExecutor>>,
        legacy_persisted_turn_cancellation: bool,
    ) -> Result<Self, String> {
        let native_notifications = BridgeNativeNotificationSink::default();
        let runtime_config = if explicit_live_acceptance_budget_override() {
            NativeAgentRuntimeConfig {
                action_loop: ActionLoopConfig::for_explicit_live_acceptance(),
            }
        } else {
            NativeAgentRuntimeConfig::default()
        };
        let native = NativeAgentRuntime::with_components(
            lifecycle,
            provider,
            tools,
            Arc::new(SystemRuntimeIdentityClock::default()),
            Arc::new(native_notifications.clone()),
            runtime_config,
        )
        .map_err(|error| {
            format!(
                "Could not initialize native Agent runtime: {}",
                error.message
            )
        })?;
        let fallback: Arc<dyn RequestHandler> = adapter.clone();
        let handler: Arc<dyn RequestHandler> =
            Arc::new(CompositeRequestHandler::new(native, fallback));
        let server = Arc::new(AppServer::new(
            handler,
            AppServerConfig {
                outbound_event_capacity: 128,
                ..AppServerConfig::default()
            },
        ));
        port.attach_server(Arc::downgrade(&server))?;
        native_notifications.attach_server(Arc::downgrade(&server))?;
        Ok(Self {
            inner: Arc::new(BridgeInner {
                server,
                adapter,
                port,
                native_product_tools,
                native_notifications,
                connections: Mutex::new(HashMap::new()),
                pending_turn_requests: Mutex::new(HashMap::new()),
                legacy_persisted_turn_cancellation,
                compat_roundtrip_logged: std::sync::atomic::AtomicBool::new(false),
            }),
        })
    }

    pub(crate) fn preview_artifact(
        &self,
        preview_id: &str,
    ) -> Result<Option<NativePreviewArtifact>, String> {
        self.inner
            .native_product_tools
            .as_ref()
            .ok_or_else(|| {
                "Native Product Tool previews are unavailable in this bridge.".to_string()
            })?
            .preview_artifact(preview_id)
            .map_err(|error| error.message)
    }

    pub(crate) fn consume_preview(
        &self,
        preview_id: &str,
        turn_id: &str,
    ) -> Result<NativePreviewArtifact, String> {
        self.inner
            .native_product_tools
            .as_ref()
            .ok_or_else(|| {
                "Native Product Tool previews are unavailable in this bridge.".to_string()
            })?
            .consume_preview(preview_id, turn_id)
            .map_err(|error| error.message)
    }

    /// Runs packaged acceptance through the exact production compatibility
    /// dispatcher. It is crate-private and exposes no additional API surface.
    pub(crate) async fn execute_k003_packaged_compat(
        &self,
        request: PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Result<CompatHttpResponse, RpcError> {
        CompatibilityHttpPort::execute(self.inner.port.as_ref(), request, cancellation).await
    }

    /// Runs one internal packaged acceptance request through the exact native
    /// JSON-RPC lifecycle handler. This is deliberately crate-private and is
    /// used only by the opt-in mechanical-arm MVP probe; it creates no Tauri
    /// command or external HTTP surface.
    pub(crate) async fn execute_mvp_packaged_native(
        &self,
        request_id: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        let connection = self.inner.server.open_connection();
        let connection_id = connection.connection_id.clone();
        let initialize = json!({
            "jsonrpc": "2.0",
            "id": "mvp_arm_initialize",
            "method": "initialize",
            "params": {
                "schema_version": "ForgeCADInitializeParams@1",
                "supported_protocol_versions": ["forgecad.app-server/1"],
                "client_info": {
                    "name": "forgecad-mvp-arm-packaged-probe",
                    "version": "1",
                    "transport": "tauri"
                },
                "capabilities": {
                    "notifications": true,
                    "cursor_replay": true,
                    "cancellation": true,
                    "notification_ack": true,
                    "binary_body_base64": true
                }
            }
        })
        .to_string();
        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {"protocol_version": "forgecad.app-server/1"}
        })
        .to_string();
        let frame = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        })
        .to_string();
        let result = async {
            let initialize_response = self
                .inner
                .server
                .handle_frame(&connection_id, &initialize)
                .await
                .ok_or_else(|| {
                    "Packaged MVP native initialization returned no response.".to_string()
                })?;
            let initialize_value: Value =
                serde_json::from_str(&initialize_response).map_err(|_| {
                    "Packaged MVP native initialization returned invalid JSON.".to_string()
                })?;
            if !initialize_value.get("error").is_none_or(Value::is_null) {
                return Err("Packaged MVP native initialization was rejected.".to_string());
            }
            if self
                .inner
                .server
                .handle_frame(&connection_id, &initialized)
                .await
                .is_some()
            {
                return Err(
                    "Packaged MVP native initialization acknowledgement was rejected.".into(),
                );
            }
            let response = self
                .inner
                .server
                .handle_frame(&connection_id, &frame)
                .await
                .ok_or_else(|| "Packaged MVP native request returned no response.".to_string())?;
            let value: Value = serde_json::from_str(&response)
                .map_err(|_| "Packaged MVP native request returned invalid JSON.".to_string())?;
            if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
                let code = error
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("UNKNOWN");
                return Err(format!("Packaged MVP native request failed: {code}"));
            }
            value
                .get("result")
                .cloned()
                .ok_or_else(|| "Packaged MVP native request omitted result.".to_string())
        }
        .await;
        self.inner.server.disconnect(&connection_id);
        // Keep the receiver alive until after disconnect: native lifecycle
        // notifications are bounded and are intentionally not replayed by an
        // acceptance probe.
        drop(connection);
        result
    }

    fn open_connection(&self, app: &AppHandle) -> Result<ProtocolConnectResult, String> {
        let mut connection = self.inner.server.open_connection();
        let connection_id = connection.connection_id.clone();
        self.inner
            .native_notifications
            .register_connection(&connection_id);
        let event_connection_id = connection_id.clone();
        let app = app.clone();
        let forwarding_task = tauri::async_runtime::spawn(async move {
            while let Some(frame) = connection.notifications.recv().await {
                let frame = match serde_json::from_str::<Value>(&frame) {
                    Ok(frame) => frame,
                    Err(error) => {
                        eprintln!("ForgeCAD app-server notification serialization failed: {error}");
                        continue;
                    }
                };
                if let Err(error) = app.emit(
                    PROTOCOL_EVENT,
                    ProtocolEventPayload {
                        connection_id: event_connection_id.clone(),
                        frame,
                    },
                ) {
                    eprintln!("ForgeCAD app-server notification emit failed: {error}");
                }
            }
        });
        self.inner
            .connections
            .lock()
            .map_err(|_| "ForgeCAD app-server connection registry is unavailable.".to_string())?
            .insert(connection_id.clone(), forwarding_task);
        Ok(ProtocolConnectResult { connection_id })
    }

    async fn send_frame(&self, request: ProtocolSendRequest) -> Result<Value, String> {
        let method = request
            .frame
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let serialized = serde_json::to_string(&request.frame)
            .map_err(|_| "ForgeCAD app-server frame could not be serialized.".to_string())?;
        let connection_id = request.connection_id.clone();
        let turn_registration = (self.inner.legacy_persisted_turn_cancellation
            && self.inner.server.connection_is_ready(&connection_id))
        .then(|| protocol_turn_cancellation_registration(&request.frame))
        .flatten();
        let mut registration_owns_entry = if let Some((request_id, target)) = &turn_registration {
            let mut pending = self.inner.pending_turn_requests.lock().map_err(|_| {
                "ForgeCAD app-server Turn cancellation registry is unavailable.".to_string()
            })?;
            let key = (connection_id.clone(), request_id.clone());
            if pending.contains_key(&key) {
                false
            } else {
                pending.insert(key, target.clone());
                true
            }
        } else {
            false
        };
        if registration_owns_entry {
            let target = &turn_registration
                .as_ref()
                .expect("owned Turn cancellation registration is present")
                .1;
            if let Err(error) = self
                .inner
                .port
                .capture_persisted_turn_baseline(target)
                .await
            {
                eprintln!(
                    "ForgeCAD persisted Turn baseline failed code={}: {}",
                    error.data.application_code, error.message
                );
                if let Ok(mut pending) = self.inner.pending_turn_requests.lock() {
                    let request_id = &turn_registration
                        .as_ref()
                        .expect("owned Turn cancellation registration is present")
                        .0;
                    pending.remove(&(connection_id.clone(), request_id.clone()));
                }
                registration_owns_entry = false;
            }
        }
        // K002 test bridges retain the former Python persisted-Turn
        // cancellation oracle. Production K003 lets the native runtime and
        // Rust lifecycle store own cancellation end-to-end.
        if self.inner.legacy_persisted_turn_cancellation {
            self.propagate_protocol_cancel_notification(&connection_id, &request.frame)
                .await?;
        }
        let response = ConnectionScopedFuture::new(connection_id.clone(), async {
            self.inner
                .server
                .handle_frame(&connection_id, &serialized)
                .await
        })
        .await;
        if registration_owns_entry {
            let request_id = &turn_registration
                .as_ref()
                .expect("owned Turn cancellation registration is present")
                .0;
            if let Ok(mut pending) = self.inner.pending_turn_requests.lock() {
                pending.remove(&(connection_id.clone(), request_id.clone()));
            }
        }
        if method == "initialized" && self.inner.server.connection_is_ready(&connection_id) {
            super::append_supervisor_log(
                "ForgeCAD app-server ready protocol=forgecad.app-server/1 lifecycle_owner=rust-app-server state_owner=rust-core python_role=restricted_geometry_executor",
            );
        }
        match response {
            Some(response) => {
                let value: Value = serde_json::from_str(&response)
                    .map_err(|_| "ForgeCAD app-server returned an invalid response.".to_string())?;
                if method == "compat/http"
                    && value.get("error").is_none()
                    && !self
                        .inner
                        .compat_roundtrip_logged
                        .swap(true, std::sync::atomic::Ordering::AcqRel)
                {
                    super::append_supervisor_log(
                        "ForgeCAD app-server compat/http roundtrip protocol=forgecad.app-server/1",
                    );
                }
                Ok(value)
            }
            None => Ok(json!({"accepted": true})),
        }
    }

    async fn propagate_protocol_cancel_notification(
        &self,
        connection_id: &str,
        frame: &Value,
    ) -> Result<bool, String> {
        let Some((request_id, cancel_token)) = protocol_cancel_notification(frame) else {
            return Ok(false);
        };
        let has_persisted_turn_target = self
            .inner
            .pending_turn_requests
            .lock()
            .map_err(|_| {
                "ForgeCAD app-server Turn cancellation registry is unavailable.".to_string()
            })?
            .contains_key(&(connection_id.to_string(), request_id.clone()));
        let mut accepted = false;
        for attempt in 0..TURN_CANCEL_CORE_REGISTRATION_ATTEMPTS {
            if self
                .inner
                .server
                .cancel_request(connection_id, &request_id, &cancel_token)
                .is_ok()
            {
                accepted = true;
                break;
            }
            // A Tauri cancel invoke can be scheduled after the bridge has
            // registered the Turn target but immediately before the protocol
            // core has inserted its request ID.  Retry only that known Turn
            // mapping; unrelated/unknown IDs remain immediate no-ops.
            if !has_persisted_turn_target || attempt + 1 == TURN_CANCEL_CORE_REGISTRATION_ATTEMPTS {
                break;
            }
            tokio::time::sleep(TURN_CANCEL_CORE_REGISTRATION_DELAY).await;
        }
        if !accepted {
            return Ok(false);
        }
        let target = self
            .inner
            .pending_turn_requests
            .lock()
            .map_err(|_| {
                "ForgeCAD app-server Turn cancellation registry is unavailable.".to_string()
            })?
            .remove(&(connection_id.to_string(), request_id));
        if let Some(target) = target {
            self.inner.port.spawn_persisted_turn_cancellation(target);
        }
        Ok(true)
    }

    fn disconnect_connection(&self, connection_id: &str) -> Result<bool, String> {
        self.inner
            .native_notifications
            .unregister_connection(connection_id);
        let task = self
            .inner
            .connections
            .lock()
            .map_err(|_| "ForgeCAD app-server connection registry is unavailable.".to_string())?
            .remove(connection_id);
        if let Some(task) = task {
            task.abort();
        }
        let closed = self.inner.server.disconnect(connection_id);
        if closed && self.inner.legacy_persisted_turn_cancellation {
            let targets = self
                .inner
                .pending_turn_requests
                .lock()
                .map(|mut pending| {
                    let keys = pending
                        .keys()
                        .filter(|(candidate, _)| candidate == connection_id)
                        .cloned()
                        .collect::<Vec<_>>();
                    keys.into_iter()
                        .filter_map(|key| pending.remove(&key))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for target in targets {
                self.inner.port.spawn_persisted_turn_cancellation(target);
            }
        }
        self.inner.port.cancel_connection(connection_id);
        Ok(closed)
    }

    pub fn shutdown(&self) {
        let connection_ids = self
            .inner
            .connections
            .lock()
            .map(|connections| connections.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for connection_id in connection_ids {
            let _ = self.disconnect_connection(&connection_id);
        }
    }

    pub async fn resource_response(
        &self,
        request: http::Request<Vec<u8>>,
    ) -> http::Response<Vec<u8>> {
        match self.prepare_resource_request(&request) {
            Ok(prepared) => match CompatibilityHttpPort::execute(
                self.inner.port.as_ref(),
                prepared,
                CancellationToken::new(),
            )
            .await
            {
                Ok(response) => compat_to_resource_response(response),
                Err(error) => resource_error_response(error),
            },
            Err(error) => resource_error_response(error),
        }
    }

    fn prepare_resource_request(
        &self,
        request: &http::Request<Vec<u8>>,
    ) -> Result<PreparedCompatHttpRequest, RpcError> {
        let host = request.uri().host().unwrap_or_default();
        if !matches!(host, RESOURCE_SCHEME_HOST | "forgecad-resource.localhost") {
            return Err(RpcError::invalid_params(
                "ForgeCAD resource requests require the localhost custom-protocol host.",
            ));
        }
        if request.method() != http::Method::GET {
            return Err(RpcError::invalid_params(
                "ForgeCAD resource requests are read-only.",
            ));
        }
        let path = request
            .uri()
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or(request.uri().path())
            .to_string();
        let mut headers = Vec::new();
        for name in [
            http::header::ACCEPT,
            http::header::RANGE,
            http::header::IF_NONE_MATCH,
            http::header::CACHE_CONTROL,
        ] {
            if let Some(value) = request
                .headers()
                .get(&name)
                .and_then(|value| value.to_str().ok())
            {
                headers.push((name.as_str().to_string(), value.to_string()));
            }
        }
        self.inner.adapter.prepare(CompatHttpRequest {
            schema_version: HTTP_COMPAT_REQUEST_SCHEMA_VERSION.to_string(),
            path,
            method: "GET".to_string(),
            headers,
            body: ProtocolHttpBody::Empty,
        })
    }
}

#[tauri::command]
pub fn forgecad_protocol_connect(
    app: AppHandle,
    state: State<'_, AppServerBridge>,
) -> Result<ProtocolConnectResult, String> {
    state.open_connection(&app)
}

#[tauri::command]
pub async fn forgecad_protocol_send(
    request: ProtocolSendRequest,
    state: State<'_, AppServerBridge>,
) -> Result<Value, String> {
    state.send_frame(request).await
}

#[tauri::command]
pub fn forgecad_protocol_disconnect(
    request: ProtocolDisconnectRequest,
    state: State<'_, AppServerBridge>,
) -> Result<Value, String> {
    let closed = state.disconnect_connection(&request.connection_id)?;
    Ok(json!({"closed": closed}))
}

#[derive(Clone)]
struct LoopbackHttpPort {
    inner: Arc<LoopbackHttpPortInner>,
}

struct LoopbackHttpPortInner {
    endpoint: LocalAgentEndpoint,
    client: reqwest::Client,
    internal_capability_token: Arc<str>,
    rust_core: Option<Arc<RustCoreRuntime>>,
    native_product_tools: OnceLock<Weak<NativeProductToolExecutor>>,
    native_blockouts: Mutex<NativeBlockoutCompatState>,
    #[cfg(test)]
    native_commit_before_bundle_hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    server: OnceLock<Weak<AppServer>>,
    subscriptions: Mutex<HashMap<String, SubscriptionHandle>>,
    geometry_sequence: std::sync::atomic::AtomicU64,
    geometry_cache: Mutex<RestrictedGeometryArtifactCache>,
}

#[derive(Default)]
struct RestrictedGeometryArtifactCache {
    entries: HashMap<String, CompiledRestrictedGeometry>,
    order: VecDeque<String>,
    total_bytes: usize,
}

impl RestrictedGeometryArtifactCache {
    fn get(&mut self, key: &str) -> Option<CompiledRestrictedGeometry> {
        let value = self.entries.get(key).cloned()?;
        self.order.retain(|item| item != key);
        self.order.push_back(key.to_string());
        Some(value)
    }

    fn insert(&mut self, key: String, value: CompiledRestrictedGeometry) {
        if let Some(previous) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(previous.glb_bytes.len());
            self.order.retain(|item| item != &key);
        }
        self.total_bytes = self.total_bytes.saturating_add(value.glb_bytes.len());
        self.entries.insert(key.clone(), value);
        self.order.push_back(key);
        while self.entries.len() > RESTRICTED_GEOMETRY_CACHE_MAX_ENTRIES
            || self.total_bytes > RESTRICTED_GEOMETRY_CACHE_MAX_BYTES
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(evicted) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(evicted.glb_bytes.len());
            }
        }
    }
}

#[derive(Default)]
struct NativeBlockoutCompatState {
    candidates: HashMap<String, NativeBlockoutCompatCandidate>,
    candidate_order: VecDeque<String>,
    builds_in_flight: HashMap<String, String>,
}

#[derive(Clone)]
struct NativeBlockoutCompatCandidate {
    artifact_id: String,
    preview_id: String,
    turn_id: String,
    build_client_request_id: String,
    build_request_sha256: String,
    plan_id: String,
    direction_id: String,
    variant_id: String,
    variation_index: u8,
    presentation_profile: String,
    domain_pack_id: String,
    project_id: Option<String>,
    expires_at_unix_ms: u64,
    segment_idempotency: HashMap<String, String>,
    segment_idempotency_order: VecDeque<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeBuildBlockoutRequest {
    client_request_id: String,
    plan: Value,
    direction_id: String,
    #[serde(default)]
    variant_id: Option<String>,
    #[serde(default)]
    variation_index: u8,
    #[serde(default = "default_blockout_presentation_profile")]
    presentation_profile: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeSegmentBlockoutRequest {
    client_request_id: String,
    plan: Value,
    direction_id: String,
    #[serde(default)]
    variant_id: Option<String>,
    #[serde(default)]
    variation_index: u8,
    #[serde(default = "default_blockout_presentation_profile")]
    presentation_profile: String,
    #[serde(default)]
    artifact_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeCommitBlockoutRequest {
    client_request_id: String,
    artifact_id: String,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default = "default_blockout_commit_summary")]
    summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeConfirmSingleResultRequest {
    client_request_id: String,
    expected_artifact_sha256: String,
    #[serde(default = "default_single_result_commit_summary")]
    summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRejectSingleResultRequest {
    client_request_id: String,
    expected_artifact_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeSingleResultAction {
    PreviewGlb,
    Confirm,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeSingleResultRoute {
    project_id: String,
    turn_id: String,
    preview_id: String,
    action: NativeSingleResultAction,
}

fn default_blockout_presentation_profile() -> String {
    "quick_sketch".to_string()
}

fn default_blockout_commit_summary() -> String {
    "确认分件候选并保存为可编辑资产".to_string()
}

fn default_single_result_commit_summary() -> String {
    "确认唯一结果并保存为可编辑资产".to_string()
}

#[derive(Debug)]
struct NativeBlockoutCompatError {
    status: u16,
    code: String,
    message: String,
    recoverable: bool,
}

impl NativeBlockoutCompatError {
    fn invalid(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: 400,
            code: code.into(),
            message: message.into(),
            recoverable: false,
        }
    }

    fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: 404,
            code: code.into(),
            message: message.into(),
            recoverable: false,
        }
    }

    fn conflict(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: 409,
            code: code.into(),
            message: message.into(),
            recoverable: true,
        }
    }

    fn unavailable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: 503,
            code: code.into(),
            message: message.into(),
            recoverable: true,
        }
    }
}

#[derive(Debug)]
enum NativeAssetRenderCompatError {
    Product(NativeBlockoutCompatError),
    Payload(AssetRenderCompatError),
}

impl NativeAssetRenderCompatError {
    fn response(self) -> CompatHttpResponse {
        match self {
            Self::Product(error) => native_blockout_error_response(error),
            Self::Payload(error) => error.response(),
        }
    }
}

impl From<NativeBlockoutCompatError> for NativeAssetRenderCompatError {
    fn from(error: NativeBlockoutCompatError) -> Self {
        Self::Product(error)
    }
}

impl From<AssetRenderCompatError> for NativeAssetRenderCompatError {
    fn from(error: AssetRenderCompatError) -> Self {
        Self::Payload(error)
    }
}

struct SubscriptionHandle {
    connection_id: String,
    cancellation: CancellationToken,
    task: Option<tauri::async_runtime::JoinHandle<()>>,
}

#[derive(Debug, Clone)]
struct PersistedTurnCancellationTarget {
    thread_id: String,
    request_text: String,
    preexisting_turn_ids: Arc<Mutex<Option<Vec<String>>>>,
}

impl PersistedTurnCancellationTarget {
    fn set_preexisting_turn_ids(&self, turn_ids: Vec<String>) {
        if let Ok(mut baseline) = self.preexisting_turn_ids.lock() {
            *baseline = Some(turn_ids);
        }
    }

    fn preexisting_turn_ids(&self) -> Option<Vec<String>> {
        self.preexisting_turn_ids
            .lock()
            .ok()
            .and_then(|baseline| baseline.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PersistedTurnCancellationState {
    Missing,
    Running(String),
    Cancelled(String),
    Terminal { turn_id: String, status: String },
}

#[derive(Debug, Deserialize)]
struct K002InternalErrorEnvelope {
    error: K002InternalErrorBody,
}

#[derive(Debug, Deserialize)]
struct K002InternalErrorBody {
    code: String,
    message: String,
    recoverable: bool,
}

#[derive(Debug, Serialize)]
struct K002ProductToolCancellationRequest<'a> {
    schema_version: &'static str,
    cancellation_id: &'a str,
    cancellation_token: &'a str,
}

#[derive(Debug, Deserialize)]
struct K002ProductToolCancellationResult {
    schema_version: String,
    cancellation_id: String,
    accepted: bool,
}

#[derive(Debug)]
enum K002InternalCallError {
    Cancelled,
    Transport,
    InvalidResponse,
    Rejected {
        status: u16,
        code: String,
        recoverable: bool,
    },
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct RestrictedGeometryCompileRequest<'a> {
    schema_version: &'static str,
    protocol_version: &'static str,
    execution_id: &'a str,
    idempotency_key: &'a str,
    cancellation_id: &'a str,
    cancellation_token: &'a str,
    action: &'static str,
    timeout_ms: u64,
    artifact_profile_id: &'a str,
    shape_program: &'a Value,
    shape_program_canonical_json: &'a str,
    shape_program_sha256: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_sketch: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    section_set: Option<&'a Value>,
    #[serde(skip_serializing_if = "<[SurfaceAdornmentProgram]>::is_empty")]
    surface_adornment_programs: &'a [SurfaceAdornmentProgram],
    #[serde(skip_serializing_if = "Option::is_none")]
    surface_layer_input: Option<&'a forgecad_app_server::RestrictedSurfaceLayerInput>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct RestrictedGeometryRenderRequest<'a> {
    schema_version: &'static str,
    protocol_version: &'static str,
    execution_id: &'a str,
    idempotency_key: &'a str,
    cancellation_id: &'a str,
    cancellation_token: &'a str,
    action: &'static str,
    timeout_ms: u64,
    artifact_handle: &'a str,
    shape_program_sha256: &'a str,
    render: RestrictedGeometryRenderRequestOptions,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct RestrictedGeometryRenderRequestOptions {
    width: u16,
    height: u16,
    exploded_parts: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestrictedGeometryExecutionResponse {
    schema_version: String,
    protocol_version: String,
    execution_id: String,
    action: String,
    artifact_handle: String,
    artifact_profile_id: String,
    artifact_profile_sha256: String,
    shape_program_sha256: String,
    glb_sha256: String,
    glb_byte_size: u64,
    triangle_count: u32,
    bounds_mm: [f64; 3],
    readback: Option<Value>,
    glb_base64: Option<String>,
    render_views: Option<BTreeMap<String, String>>,
    render_view_sha256: Option<BTreeMap<String, String>>,
    renderer_id: Option<String>,
    #[serde(default)]
    exploded_part_ids: Vec<String>,
    exploded_unavailable_reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct RestrictedGeometryCancellationRequest<'a> {
    schema_version: &'static str,
    protocol_version: &'static str,
    cancellation_id: &'a str,
    cancellation_token: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestrictedGeometryCancellationResponse {
    schema_version: String,
    cancellation_id: String,
    accepted: bool,
    tombstoned: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestrictedGeometryErrorEnvelope {
    error: RestrictedGeometryErrorBody,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestrictedGeometryErrorBody {
    code: String,
    message: String,
    recoverable: bool,
    details: Value,
}

#[derive(Debug)]
enum RestrictedGeometryCallError {
    Cancelled,
    Timeout,
    Transport,
    InvalidResponse,
    Rejected {
        status: u16,
        code: String,
        recoverable: bool,
    },
}

#[derive(Clone)]
struct CompiledRestrictedGeometry {
    artifact_handle: String,
    artifact_profile_id: String,
    artifact_profile_sha256: String,
    shape_program_sha256: String,
    glb_sha256: String,
    glb_bytes: Vec<u8>,
    readback: RestrictedGeometryReadback,
}

#[derive(Clone)]
struct RestrictedGeometryPhaseIdentity {
    execution_id: String,
    idempotency_key: String,
    cancellation_id: String,
    cancellation_token: String,
}

struct RestrictedGeometryCancelGuard {
    port: LoopbackHttpPort,
    identity: Option<RestrictedGeometryPhaseIdentity>,
}

impl RestrictedGeometryCancelGuard {
    fn new(port: LoopbackHttpPort, identity: RestrictedGeometryPhaseIdentity) -> Self {
        Self {
            port,
            identity: Some(identity),
        }
    }

    fn disarm(&mut self) {
        self.identity = None;
    }
}

impl Drop for RestrictedGeometryCancelGuard {
    fn drop(&mut self) {
        let Some(identity) = self.identity.take() else {
            return;
        };
        let port = self.port.clone();
        tauri::async_runtime::spawn(async move {
            let _ = port.post_restricted_geometry_cancel(&identity).await;
        });
    }
}

impl NativeBlockoutCompatState {
    fn prune_expired(&mut self, now_unix_ms: u64) -> Vec<String> {
        let expired = self
            .candidates
            .iter()
            .filter_map(|(artifact_id, candidate)| {
                (candidate.expires_at_unix_ms <= now_unix_ms).then(|| artifact_id.clone())
            })
            .collect::<Vec<_>>();
        expired
            .into_iter()
            .filter_map(|artifact_id| self.remove_candidate(&artifact_id))
            .map(|candidate| candidate.preview_id)
            .collect()
    }

    fn remove_candidate(&mut self, artifact_id: &str) -> Option<NativeBlockoutCompatCandidate> {
        if let Some(index) = self
            .candidate_order
            .iter()
            .position(|candidate| candidate == artifact_id)
        {
            self.candidate_order.remove(index);
        }
        self.builds_in_flight.remove(artifact_id);
        self.candidates.remove(artifact_id)
    }

    fn insert_candidate(&mut self, candidate: NativeBlockoutCompatCandidate) -> Vec<String> {
        let artifact_id = candidate.artifact_id.clone();
        if let Some(previous) = self.remove_candidate(&artifact_id) {
            let _ = previous;
        }
        self.candidates.insert(artifact_id.clone(), candidate);
        self.candidate_order.push_back(artifact_id);
        let mut evicted = Vec::new();
        while self.candidates.len() > NATIVE_BLOCKOUT_CANDIDATE_LIMIT {
            let Some(oldest) = self.candidate_order.front().cloned() else {
                break;
            };
            if let Some(candidate) = self.remove_candidate(&oldest) {
                evicted.push(candidate.preview_id);
            }
        }
        evicted
    }
}

impl LoopbackHttpPort {
    #[cfg(test)]
    fn new(
        endpoint: LocalAgentEndpoint,
        internal_capability_token: String,
    ) -> Result<Self, String> {
        Self::new_inner(endpoint, internal_capability_token, None)
    }

    fn new_with_rust_core(
        endpoint: LocalAgentEndpoint,
        internal_capability_token: String,
        rust_core: Arc<RustCoreRuntime>,
    ) -> Result<Self, String> {
        Self::new_inner(endpoint, internal_capability_token, Some(rust_core))
    }

    fn new_inner(
        endpoint: LocalAgentEndpoint,
        internal_capability_token: String,
        rust_core: Option<Arc<RustCoreRuntime>>,
    ) -> Result<Self, String> {
        if internal_capability_token.is_empty()
            || internal_capability_token.len() > 256
            || !internal_capability_token
                .bytes()
                .all(|byte| byte.is_ascii_graphic())
        {
            return Err(
                "ForgeCAD internal capability token is outside the bounded contract.".to_string(),
            );
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_millis(RESTRICTED_GEOMETRY_HTTP_TIMEOUT_MS))
            .build()
            .map_err(|error| format!("Could not build ForgeCAD loopback client: {error}"))?;
        Ok(Self {
            inner: Arc::new(LoopbackHttpPortInner {
                endpoint,
                client,
                internal_capability_token: Arc::from(internal_capability_token),
                rust_core,
                native_product_tools: OnceLock::new(),
                native_blockouts: Mutex::new(NativeBlockoutCompatState::default()),
                #[cfg(test)]
                native_commit_before_bundle_hook: Mutex::new(None),
                server: OnceLock::new(),
                subscriptions: Mutex::new(HashMap::new()),
                geometry_sequence: std::sync::atomic::AtomicU64::new(1),
                geometry_cache: Mutex::new(RestrictedGeometryArtifactCache::default()),
            }),
        })
    }

    fn attach_server(&self, server: Weak<AppServer>) -> Result<(), String> {
        self.inner
            .server
            .set(server)
            .map_err(|_| "ForgeCAD app-server was attached more than once.".to_string())
    }

    fn attach_native_product_tools(
        &self,
        executor: Weak<NativeProductToolExecutor>,
    ) -> Result<(), String> {
        self.inner
            .native_product_tools
            .set(executor)
            .map_err(|_| "ForgeCAD native Product Tool executor was attached twice.".to_string())
    }

    fn native_product_tools(
        &self,
    ) -> Result<Arc<NativeProductToolExecutor>, NativeBlockoutCompatError> {
        self.inner
            .native_product_tools
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(|| {
                NativeBlockoutCompatError::unavailable(
                    "NATIVE_PRODUCT_TOOL_UNAVAILABLE",
                    "Rust Product Tool executor is unavailable.",
                )
            })
    }

    fn prune_native_blockouts(
        &self,
        executor: &NativeProductToolExecutor,
    ) -> Result<(), NativeBlockoutCompatError> {
        let expired_preview_ids = self
            .inner
            .native_blockouts
            .lock()
            .map_err(|_| native_blockout_state_unavailable())?
            .prune_expired(native_blockout_now_unix_ms());
        for preview_id in expired_preview_ids {
            executor
                .discard_preview(&preview_id)
                .map_err(native_blockout_product_tool_error)?;
        }
        Ok(())
    }

    async fn handle_native_asset_render_compat(
        &self,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Option<Result<CompatHttpResponse, RpcError>> {
        let parsed = parse_asset_render_request(request)?;
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(error) => return Some(Ok(error.response())),
        };
        let (asset_version_id, width, height, expected_fingerprint) = match parsed {
            AssetRenderCompatRequest::Views {
                asset_version_id,
                width,
                height,
            } => (asset_version_id, width, height, None),
            AssetRenderCompatRequest::Package {
                asset_version_id,
                width,
                height,
                render_set_sha256,
            } => (asset_version_id, width, height, Some(render_set_sha256)),
        };
        let render_set = match self
            .native_asset_render_set(&asset_version_id, width, height, cancellation)
            .await
        {
            Ok(render_set) => render_set,
            Err(error) => return Some(Ok(error.response())),
        };
        let response = if let Some(expected_fingerprint) = expected_fingerprint {
            match render_package_response(
                &render_set,
                &expected_fingerprint,
                MAX_RAW_COMPAT_BODY_BYTES,
            ) {
                Ok(response) => response,
                Err(error) => error.response(),
            }
        } else {
            render_set_response(&render_set)
        };
        Some(Ok(response))
    }

    async fn native_asset_render_set(
        &self,
        asset_version_id: &str,
        width: u16,
        height: u16,
        cancellation: CancellationToken,
    ) -> Result<SealedRenderSet, NativeAssetRenderCompatError> {
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled().into());
        }
        let rust_core = self.inner.rust_core.as_ref().ok_or_else(|| {
            NativeBlockoutCompatError::unavailable(
                "RUST_CORE_UNAVAILABLE",
                "Rust product core is unavailable for concept rendering.",
            )
        })?;
        let repository = rust_core.repository();
        let version = repository
            .version(asset_version_id)
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "ASSET_VERSION_NOT_FOUND",
                    "Agent asset version does not exist.",
                )
            })?;
        let snapshot = repository
            .snapshot(&version.project_id)
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "ACTIVE_DESIGN_NOT_FOUND",
                    "ActiveDesignSnapshot does not exist for this asset.",
                )
            })?;
        if snapshot.active_design.asset_version_id() != Some(asset_version_id) {
            return Err(NativeBlockoutCompatError::conflict(
                "ACTIVE_DESIGN_STALE",
                "Concept views can only be rendered from the active Agent asset.",
            )
            .into());
        }
        if snapshot.preview.is_some() {
            return Err(NativeBlockoutCompatError::conflict(
                "ACTIVE_DESIGN_PREVIEW_PENDING",
                "Resolve the active ChangeSet preview before rendering concept views.",
            )
            .into());
        }
        let quality_reference = snapshot.quality.as_ref().ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RENDER_QUALITY_REQUIRED",
                "Concept rendering requires Snapshot-bound production GLB readback quality.",
            )
        })?;
        if quality_reference.asset_version_id != asset_version_id {
            return Err(NativeBlockoutCompatError::conflict(
                "QUALITY_ASSET_STALE",
                "Snapshot quality does not belong to the requested asset version.",
            )
            .into());
        }
        let quality = repository
            .quality_report(&quality_reference.quality_report_id)
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "QUALITY_REPORT_NOT_FOUND",
                    "Snapshot-bound quality report does not exist.",
                )
            })?;
        if quality.asset_version_id != asset_version_id || quality.status != QualityStatus::Passed {
            return Err(NativeBlockoutCompatError::conflict(
                "RENDER_QUALITY_REQUIRED",
                "Concept rendering requires passed production GLB readback quality.",
            )
            .into());
        }
        let object = repository
            .object_for_reference(&ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: asset_version_id.into(),
                role: "production_glb".into(),
            })
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "PRODUCTION_GLB_REQUIRED",
                    "Concept rendering requires the current production GLB in Rust CAS.",
                )
            })?;
        let stored_glb = repository
            .read_object(&object.sha256)
            .map_err(native_blockout_core_error)?;
        let canonical = verify_forgecad_glb(&stored_glb, Some("production_concept"))
            .map_err(native_blockout_core_error)?;
        let shape_program_sha256 = native_blockout_semantic_sha256(&version.shape_program)?;
        let compile_readback = quality
            .report
            .get("compile_readback")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "COMPILE_READBACK_STALE",
                    "Concept rendering requires GeometryCompileReadback@2 quality facts.",
                )
            })?;
        if compile_readback
            .get("schema_version")
            .and_then(Value::as_str)
            != Some("GeometryCompileReadback@2")
            || compile_readback
                .get("shape_program_sha256")
                .and_then(Value::as_str)
                != Some(shape_program_sha256.as_str())
            || compile_readback.get("glb_sha256").and_then(Value::as_str)
                != Some(object.sha256.as_str())
            || object.sha256 != canonical.glb_sha256
            || object.byte_size != canonical.glb_byte_size
        {
            return Err(NativeBlockoutCompatError::conflict(
                "COMPILE_READBACK_STALE",
                "Snapshot quality, ShapeProgram and production GLB no longer match.",
            )
            .into());
        }

        let input = RestrictedGeometryInput {
            schema_version: "RestrictedGeometryInput@1".into(),
            shape_program: version.shape_program.clone(),
            profile_sketch: None,
            section_set: None,
            surface_adornment_programs: native_surface_adornment_programs(&version)?,
            surface_layer_input: None,
            quality_profile: RestrictedQualityProfile {
                profile_id: "production_concept".into(),
                runtime_manifest_version: "ShapeProgramRuntimeManifest@1".into(),
                max_triangle_count: 150_000,
                render_width: width,
                render_height: height,
                require_closed_manifold: true,
                require_surface_provenance: true,
            },
        };
        input
            .validate()
            .map_err(|error| NativeBlockoutCompatError::conflict(error.code, error.message))?;
        let rendered =
            RestrictedGeometryPort::build_compile_render(self, input, cancellation.clone())
                .await
                .map_err(native_blockout_restricted_geometry_error)?;
        let rendered_readback = native_verified_geometry_readback(
            &rendered.glb_bytes,
            &rendered.readback,
            "production_concept",
            &version.shape_program,
        )?;
        if rendered.glb_bytes != stored_glb
            || rendered.glb_sha256 != object.sha256
            || rendered_readback != canonical
        {
            return Err(NativeBlockoutCompatError::conflict(
                "RENDER_SOURCE_DRIFT",
                "Restricted geometry recompilation does not match the Snapshot-bound production GLB.",
            )
            .into());
        }
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled().into());
        }
        seal_render_set(
            asset_version_id,
            width,
            height,
            &rendered.renderer_id,
            &rendered.views,
            format!("unix_ms_{}", native_blockout_now_unix_ms()),
            MAX_RAW_COMPAT_BODY_BYTES,
        )
        .map_err(Into::into)
    }

    async fn handle_native_blockout_compat(
        &self,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Option<Result<CompatHttpResponse, RpcError>> {
        if request.method != AllowedHttpMethod::Post {
            return None;
        }
        let route = request.path.split('?').next().unwrap_or(&request.path);
        let result = match route {
            "/api/v1/agent/blockouts" => self
                .native_build_blockout(request, cancellation)
                .await
                .map(|body| native_blockout_json_response(200, body)),
            "/api/v1/agent/blockouts:segment" => self
                .native_segment_blockout(request, cancellation)
                .await
                .map(|body| native_blockout_json_response(200, body)),
            "/api/v1/agent/blockouts:commit" => self
                .native_commit_blockout(request, cancellation)
                .await
                .map(|body| native_blockout_json_response(201, body)),
            _ => return None,
        };
        Some(Ok(match result {
            Ok(response) => response,
            Err(error) => native_blockout_error_response(error),
        }))
    }

    async fn handle_native_single_result_compat(
        &self,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Option<Result<CompatHttpResponse, RpcError>> {
        let route = parse_native_single_result_route(request)?;
        let result = match route.action {
            NativeSingleResultAction::PreviewGlb => {
                self.native_single_result_preview(&route, request, cancellation)
                    .await
            }
            NativeSingleResultAction::Confirm => self
                .native_confirm_single_result(&route, request, cancellation)
                .await
                .map(|value| native_blockout_json_response(201, value)),
            NativeSingleResultAction::Reject => self
                .native_reject_single_result(&route, request)
                .map(|value| native_blockout_json_response(200, value)),
        };
        Some(Ok(match result {
            Ok(response) => response,
            Err(error) => native_blockout_error_response(error),
        }))
    }

    async fn native_single_result_preview(
        &self,
        route: &NativeSingleResultRoute,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Result<CompatHttpResponse, NativeBlockoutCompatError> {
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        native_change_set_require_empty_body(request)?;
        let expected_sha256 = native_single_result_if_match(request)?;
        let executor = self.native_product_tools()?;
        let artifact = executor
            .formal_preview_artifact(
                &route.project_id,
                &route.turn_id,
                &route.preview_id,
                &expected_sha256,
            )
            .map_err(native_blockout_product_tool_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "SINGLE_RESULT_PREVIEW_NOT_FOUND",
                    "The formal preview is missing, expired, rejected, consumed, or unavailable after restart.",
                )
            })?;
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        native_single_result_glb_response(&route.project_id, &route.turn_id, &artifact)
    }

    async fn native_confirm_single_result(
        &self,
        route: &NativeSingleResultRoute,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Result<Value, NativeBlockoutCompatError> {
        let input: NativeConfirmSingleResultRequest = native_blockout_request_json(request)?;
        native_blockout_require_idempotency(request, &input.client_request_id)?;
        native_blockout_validate_client_request_id(&input.client_request_id)?;
        native_single_result_validate_sha256(&input.expected_artifact_sha256)?;
        if native_single_result_if_match(request)? != input.expected_artifact_sha256 {
            return Err(NativeBlockoutCompatError::conflict(
                "SINGLE_RESULT_PRECONDITION_MISMATCH",
                "The formal preview body and If-Match identities differ.",
            ));
        }
        let artifact_id = native_single_result_artifact_id(
            &route.project_id,
            &route.turn_id,
            &route.preview_id,
            &input.expected_artifact_sha256,
        );
        let executor = self.native_product_tools()?;
        let artifact = executor
            .formal_preview_artifact(
                &route.project_id,
                &route.turn_id,
                &route.preview_id,
                &input.expected_artifact_sha256,
            )
            .map_err(native_blockout_product_tool_error)?;

        // An idempotent replay may arrive after the successful transaction
        // consumed transient bytes. native_commit_blockout checks the sealed
        // repository bundle before consulting this transient candidate.
        if let Some(artifact) = artifact {
            let provenance = artifact.formal_provenance.as_ref().ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "SINGLE_RESULT_PROVENANCE_REQUIRED",
                    "Only a trusted V003 preview can be confirmed.",
                )
            })?;
            let presentation_profile = match artifact.readback.artifact_profile_id.as_str() {
                "interactive_preview" => "quick_sketch",
                "production_concept" => "showcase",
                _ => {
                    return Err(NativeBlockoutCompatError::conflict(
                        "SINGLE_RESULT_PROFILE_INVALID",
                        "The formal preview uses an unsupported artifact profile.",
                    ))
                }
            };
            let candidate = NativeBlockoutCompatCandidate {
                artifact_id: artifact_id.clone(),
                preview_id: artifact.preview_id.clone(),
                turn_id: artifact.turn_id.clone(),
                build_client_request_id: provenance.decision.decision_id.clone(),
                build_request_sha256: provenance.decision_sha256.clone(),
                plan_id: provenance.plan_id.clone(),
                direction_id: provenance.direction_id.clone(),
                variant_id: "variant_single_result".into(),
                variation_index: 0,
                presentation_profile: presentation_profile.into(),
                domain_pack_id: provenance.domain_pack_id.clone(),
                project_id: Some(provenance.project_id.clone()),
                expires_at_unix_ms: artifact.expires_at_unix_ms,
                segment_idempotency: HashMap::new(),
                segment_idempotency_order: VecDeque::new(),
            };
            let evicted = self
                .inner
                .native_blockouts
                .lock()
                .map_err(|_| native_blockout_state_unavailable())?
                .insert_candidate(candidate);
            for preview_id in evicted {
                executor
                    .discard_preview(&preview_id)
                    .map_err(native_blockout_product_tool_error)?;
            }
        }
        let commit_request = PreparedCompatHttpRequest {
            endpoint: request.endpoint.clone(),
            method: AllowedHttpMethod::Post,
            path: "/api/v1/agent/blockouts:commit".into(),
            headers: request.headers.clone(),
            body: ProtocolHttpBody::Utf8 {
                data: json!({
                    "client_request_id": input.client_request_id,
                    "artifact_id": artifact_id,
                    "project_id": route.project_id,
                    "summary": input.summary,
                })
                .to_string(),
            },
        };
        self.native_commit_blockout(&commit_request, cancellation)
            .await
    }

    fn native_reject_single_result(
        &self,
        route: &NativeSingleResultRoute,
        request: &PreparedCompatHttpRequest,
    ) -> Result<Value, NativeBlockoutCompatError> {
        let input: NativeRejectSingleResultRequest = native_blockout_request_json(request)?;
        native_blockout_require_idempotency(request, &input.client_request_id)?;
        native_blockout_validate_client_request_id(&input.client_request_id)?;
        native_single_result_validate_sha256(&input.expected_artifact_sha256)?;
        if native_single_result_if_match(request)? != input.expected_artifact_sha256 {
            return Err(NativeBlockoutCompatError::conflict(
                "SINGLE_RESULT_PRECONDITION_MISMATCH",
                "The formal preview body and If-Match identities differ.",
            ));
        }
        let executor = self.native_product_tools()?;
        let rejected = executor
            .reject_formal_preview(
                &route.project_id,
                &route.turn_id,
                &route.preview_id,
                &input.expected_artifact_sha256,
            )
            .map_err(native_blockout_product_tool_error)?;
        let artifact_id = native_single_result_artifact_id(
            &route.project_id,
            &route.turn_id,
            &route.preview_id,
            &input.expected_artifact_sha256,
        );
        if let Ok(mut state) = self.inner.native_blockouts.lock() {
            state.remove_candidate(&artifact_id);
        }
        Ok(json!({
            "preview_id": route.preview_id,
            "rejected": rejected,
            "permanent_side_effects": 0,
        }))
    }

    async fn handle_native_change_set_compat(
        &self,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Option<Result<CompatHttpResponse, RpcError>> {
        let route = request.path.split('?').next().unwrap_or(&request.path);
        let change_set_route = route.strip_prefix("/api/v1/agent/change-sets/")?;
        let (change_set_id, action) = if request.method == AllowedHttpMethod::Post {
            if let Some(change_set_id) = change_set_route.strip_suffix(":preview") {
                (change_set_id, "preview")
            } else if let Some(change_set_id) = change_set_route.strip_suffix(":confirm") {
                (change_set_id, "confirm")
            } else {
                return None;
            }
        } else if request.method == AllowedHttpMethod::Get {
            if let Some(change_set_id) = change_set_route.strip_suffix(":preview.glb") {
                (change_set_id, "preview_glb")
            } else {
                return None;
            }
        } else {
            return None;
        };
        if change_set_id.is_empty() || change_set_id.len() > 256 || !valid_stable_id(change_set_id)
        {
            return Some(Ok(native_blockout_error_response(
                NativeBlockoutCompatError::invalid(
                    "CHANGE_SET_ID_INVALID",
                    "ChangeSet identity is outside the bounded stable-ID contract.",
                ),
            )));
        }

        let result = match action {
            "preview" => self
                .native_preview_change_set(change_set_id, request, cancellation)
                .await
                .map(|body| native_blockout_json_response(200, body)),
            "preview_glb" => {
                self.native_change_set_preview_glb(change_set_id, request, cancellation)
                    .await
            }
            "confirm" => self
                .native_confirm_change_set(change_set_id, request, cancellation)
                .await
                .map(|body| native_blockout_json_response(200, body)),
            _ => unreachable!(),
        };
        Some(Ok(match result {
            Ok(response) => response,
            Err(error) => native_blockout_error_response(error),
        }))
    }

    async fn native_compile_change_set_geometry(
        &self,
        version: &AgentAssetVersion,
        profile_id: &str,
        cancellation: CancellationToken,
    ) -> Result<RestrictedGeometryOutput, NativeBlockoutCompatError> {
        let (render_width, render_height) = match profile_id {
            "interactive_preview" => (320, 320),
            "production_concept" => (128, 128),
            _ => {
                return Err(NativeBlockoutCompatError::invalid(
                    "ARTIFACT_PROFILE_INVALID",
                    "ChangeSet geometry requested an unsupported artifact profile.",
                ));
            }
        };
        let input = RestrictedGeometryInput {
            schema_version: "RestrictedGeometryInput@1".into(),
            shape_program: version.shape_program.clone(),
            profile_sketch: None,
            section_set: None,
            surface_adornment_programs: native_surface_adornment_programs(version)?,
            surface_layer_input: None,
            quality_profile: RestrictedQualityProfile {
                profile_id: profile_id.into(),
                runtime_manifest_version: "ShapeProgramRuntimeManifest@1".into(),
                max_triangle_count: if profile_id == "production_concept" {
                    150_000
                } else {
                    100_000
                },
                render_width,
                render_height,
                require_closed_manifold: true,
                require_surface_provenance: true,
            },
        };
        input
            .validate()
            .map_err(|error| NativeBlockoutCompatError::invalid(error.code, error.message))?;
        RestrictedGeometryPort::build_compile_render(self, input, cancellation)
            .await
            .map_err(native_blockout_restricted_geometry_error)
    }

    async fn native_preview_change_set(
        &self,
        change_set_id: &str,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Result<Value, NativeBlockoutCompatError> {
        native_change_set_require_empty_body(request)?;
        let _idempotency_key = native_change_set_idempotency_key(request)?;
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        let rust_core = self.inner.rust_core.as_ref().ok_or_else(|| {
            NativeBlockoutCompatError::unavailable(
                "RUST_CORE_UNAVAILABLE",
                "Rust product core is unavailable.",
            )
        })?;
        let repository = rust_core.repository();
        if let Some(existing) = repository
            .read_change_set_preview_bundle(change_set_id)
            .map_err(native_blockout_core_error)?
        {
            return native_change_set_payload(&existing.change_set);
        }
        let change_set = repository
            .change_set(change_set_id)
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "CHANGE_SET_NOT_FOUND",
                    "Agent ChangeSet does not exist.",
                )
            })?;
        if change_set.status != ChangeSetStatus::Proposed
            || change_set.preview.is_some()
            || change_set.resulting_asset_version_id.is_some()
        {
            return Err(NativeBlockoutCompatError::conflict(
                "CHANGE_SET_PREVIEW_STATE_CONFLICT",
                "Only a proposed ChangeSet without prior preview state can be previewed.",
            ));
        }
        let base = repository
            .version(&change_set.base_asset_version_id)
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "ASSET_VERSION_NOT_FOUND",
                    "ChangeSet base asset version does not exist.",
                )
            })?;
        let snapshot = repository
            .snapshot(&change_set.project_id)
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "ACTIVE_DESIGN_NOT_FOUND",
                    "ActiveDesignSnapshot does not exist for this ChangeSet.",
                )
            })?;
        let head = repository
            .head(&change_set.project_id)
            .map_err(native_blockout_core_error)?;
        if snapshot
            .preview
            .as_ref()
            .is_some_and(|preview| preview.change_set_id != change_set.change_set_id)
        {
            return Err(NativeBlockoutCompatError::conflict(
                "ACTIVE_DESIGN_PREVIEW_PENDING",
                "Resolve the active ChangeSet preview before previewing another edit.",
            ));
        }
        if snapshot.active_design.asset_version_id()
            != Some(change_set.base_asset_version_id.as_str())
            || head.as_deref() != Some(change_set.base_asset_version_id.as_str())
        {
            return Err(NativeBlockoutCompatError::conflict(
                "CHANGE_SET_BASE_STALE",
                "ChangeSet base or head no longer matches ActiveDesignSnapshot.",
            ));
        }
        let mut sealed_preview = native_change_set_apply(repository, &base, &change_set)?;
        sealed_preview.shape_program =
            normalize_persisted_shape_program(&sealed_preview.shape_program)
                .map_err(native_blockout_core_error)?;
        let interactive = self
            .native_compile_change_set_geometry(
                &sealed_preview,
                "interactive_preview",
                cancellation.clone(),
            )
            .await?;
        let verified = native_verified_geometry_readback(
            &interactive.glb_bytes,
            &interactive.readback,
            "interactive_preview",
            &sealed_preview.shape_program,
        )?;
        if interactive.glb_sha256 != verified.glb_sha256 {
            return Err(NativeBlockoutCompatError::conflict(
                "RESTRICTED_GEOMETRY_READBACK_MISMATCH",
                "Canonical interactive GLB identity differs from restricted geometry output.",
            ));
        }
        let interactive_readback = serde_json::to_value(&interactive.readback).map_err(|_| {
            NativeBlockoutCompatError::unavailable(
                "CHANGE_SET_INTERACTIVE_READBACK_INVALID",
                "Restricted geometry readback could not be sealed.",
            )
        })?;
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        let bundle = repository
            .preview_change_set_bundle(
                change_set_id,
                &sealed_preview,
                &interactive.glb_bytes,
                &interactive_readback,
                snapshot.etag(),
                &change_set.created_at,
            )
            .map_err(native_blockout_core_error)?;
        native_change_set_payload(&bundle.change_set)
    }

    async fn native_change_set_preview_glb(
        &self,
        change_set_id: &str,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Result<CompatHttpResponse, NativeBlockoutCompatError> {
        native_change_set_require_empty_body(request)?;
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        let rust_core = self.inner.rust_core.as_ref().ok_or_else(|| {
            NativeBlockoutCompatError::unavailable(
                "RUST_CORE_UNAVAILABLE",
                "Rust product core is unavailable.",
            )
        })?;
        let repository = rust_core.repository();
        let bundle = repository
            .read_change_set_preview_bundle(change_set_id)
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "RUST_GEOMETRY_PREVIEW_REQUIRED",
                    "ChangeSet has no active sealed restricted geometry preview.",
                )
            })?;
        let bytes = repository
            .read_object(&bundle.interactive_preview_glb.sha256)
            .map_err(native_blockout_core_error)?;
        let executor_readback: RestrictedGeometryReadback =
            serde_json::from_value(bundle.interactive_readback.clone()).map_err(|_| {
                NativeBlockoutCompatError::conflict(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    "Stored ChangeSet interactive readback cannot be decoded.",
                )
            })?;
        let verified = native_verified_geometry_readback(
            &bytes,
            &executor_readback,
            "interactive_preview",
            &bundle.sealed_preview.shape_program,
        )?;
        if verified.glb_sha256 != bundle.interactive_preview_glb.sha256
            || verified.glb_byte_size != bundle.interactive_preview_glb.byte_size
        {
            return Err(NativeBlockoutCompatError::conflict(
                "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                "Stored ChangeSet preview object identity differs from canonical GLB readback.",
            ));
        }
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        native_change_set_glb_response(
            &bundle.change_set,
            &bundle.sealed_preview,
            &verified,
            &bytes,
        )
    }

    async fn native_confirm_change_set(
        &self,
        change_set_id: &str,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Result<Value, NativeBlockoutCompatError> {
        native_change_set_require_empty_body(request)?;
        let _idempotency_key = native_change_set_idempotency_key(request)?;
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        let rust_core = self.inner.rust_core.as_ref().ok_or_else(|| {
            NativeBlockoutCompatError::unavailable(
                "RUST_CORE_UNAVAILABLE",
                "Rust product core is unavailable.",
            )
        })?;
        let repository = rust_core.repository();
        let resulting_asset_version_id = native_change_set_version_id(change_set_id, "confirmed");
        let quality_report_id = native_blockout_quality_report_id(&resulting_asset_version_id);
        if let Some(existing) = repository
            .read_change_set_confirm_bundle(
                change_set_id,
                &resulting_asset_version_id,
                &quality_report_id,
            )
            .map_err(native_blockout_core_error)?
        {
            return native_change_set_confirm_payload(&existing.change_set, &existing.version);
        }
        let preview_bundle = repository
            .read_change_set_preview_bundle(change_set_id)
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "RUST_GEOMETRY_PREVIEW_REQUIRED",
                    "ChangeSet confirmation requires one active sealed restricted geometry preview.",
                )
            })?;
        let interactive_bytes = repository
            .read_object(&preview_bundle.interactive_preview_glb.sha256)
            .map_err(native_blockout_core_error)?;
        let interactive_readback: RestrictedGeometryReadback =
            serde_json::from_value(preview_bundle.interactive_readback.clone()).map_err(|_| {
                NativeBlockoutCompatError::conflict(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    "Stored ChangeSet interactive readback cannot be decoded.",
                )
            })?;
        let interactive_verified = native_verified_geometry_readback(
            &interactive_bytes,
            &interactive_readback,
            "interactive_preview",
            &preview_bundle.sealed_preview.shape_program,
        )?;
        if interactive_verified.glb_sha256 != preview_bundle.interactive_preview_glb.sha256
            || interactive_verified.glb_byte_size
                != preview_bundle.interactive_preview_glb.byte_size
        {
            return Err(NativeBlockoutCompatError::conflict(
                "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                "Stored ChangeSet preview object identity differs from canonical GLB readback.",
            ));
        }

        let mut resulting = preview_bundle.sealed_preview.clone();
        resulting.asset_version_id = resulting_asset_version_id;
        resulting.parent_asset_version_id =
            Some(preview_bundle.change_set.base_asset_version_id.clone());
        resulting.status = AssetVersionStatus::Committed;
        resulting.created_at = preview_bundle.change_set.created_at.clone();
        resulting.validate().map_err(native_blockout_core_error)?;

        let production = self
            .native_compile_change_set_geometry(
                &resulting,
                "production_concept",
                cancellation.clone(),
            )
            .await?;
        let production_verified = native_verified_geometry_readback(
            &production.glb_bytes,
            &production.readback,
            "production_concept",
            &resulting.shape_program,
        )?;
        if production.glb_sha256 != production_verified.glb_sha256 {
            return Err(NativeBlockoutCompatError::conflict(
                "RESTRICTED_GEOMETRY_READBACK_MISMATCH",
                "Canonical production GLB identity differs from restricted geometry output.",
            ));
        }
        let quality = native_geometry_quality_report(
            &resulting.project_id,
            &resulting.asset_version_id,
            &production.readback,
            &production.renderer_id,
            &production.view_sha256,
            &production_verified,
            &resulting.created_at,
        )?;
        // Final cancellable boundary. Once the synchronous Core bundle call
        // starts, a late cancellation cannot turn durable success into an HTTP
        // failure or leave the caller uncertain whether confirmation won.
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        #[cfg(test)]
        if let Ok(mut hook) = self.inner.native_commit_before_bundle_hook.lock() {
            if let Some(hook) = hook.take() {
                hook();
            }
        }
        let confirmed = repository
            .confirm_change_set_bundle(
                change_set_id,
                &preview_bundle.sealed_preview,
                &resulting,
                &interactive_bytes,
                &production.glb_bytes,
                &quality,
                preview_bundle.snapshot.etag(),
            )
            .map_err(native_blockout_core_error)?;
        native_change_set_confirm_payload(&confirmed.change_set, &confirmed.version)
    }

    async fn execute_native_blockout_preview(
        &self,
        plan: Value,
        direction_id: &str,
        variant_id: &str,
        presentation_profile: &str,
        artifact_id: &str,
        cancellation: CancellationToken,
    ) -> Result<NativePreviewArtifact, NativeBlockoutCompatError> {
        let executor = self.native_product_tools()?;
        let registry = ProductToolRegistry::forgecad_v1()
            .map_err(|error| NativeBlockoutCompatError::unavailable(error.code, error.message))?;
        let sequence = self
            .inner
            .geometry_sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nonce = sha256_hex(
            format!("{artifact_id}:{direction_id}:{variant_id}:{presentation_profile}:{sequence}")
                .as_bytes(),
        );
        let artifact_suffix = artifact_id.strip_prefix("artifact_").unwrap_or(artifact_id);
        let turn_id = format!("turn_blockout_{artifact_suffix}");
        let execution_id = format!("execution_blockout_{sequence}_{}", &nonce[..16]);
        let cancellation_id = format!("cancel_blockout_{sequence}_{}", &nonce[..16]);
        let cancellation_token = format!("token_blockout_{nonce}");
        // Legacy blockout compatibility owns a transient preview only. It is
        // intentionally distinct from V003's formal production decision and
        // from a later ChangeSet preview/confirmation bundle.
        let calls = [
            ("plan_complete_concept", json!({"plan": plan})),
            (
                "build_candidate_geometry",
                json!({
                    "direction_id": direction_id,
                    "variant_id": variant_id,
                    "presentation_profile": presentation_profile
                }),
            ),
            ("compile_readback_candidate", json!({})),
            ("render_candidate_views", json!({})),
        ];
        for (index, (name, arguments)) in calls.into_iter().enumerate() {
            if cancellation.is_cancelled() {
                let _ = executor
                    .cancel(cancellation_id.clone(), cancellation_token.clone())
                    .await;
                return Err(native_blockout_cancelled());
            }
            let tool_request = registry
                .build_execution_request(
                    &turn_id,
                    &ProviderToolCall {
                        call_id: format!("call_blockout_{sequence}_{index}"),
                        name: name.to_string(),
                        arguments,
                    },
                    &execution_id,
                    &cancellation_id,
                    &cancellation_token,
                )
                .map_err(|error| NativeBlockoutCompatError::invalid(error.code, error.message))?;
            let result = executor
                .execute(tool_request.clone(), cancellation.clone())
                .await
                .map_err(native_blockout_product_tool_error)?;
            registry
                .validate_result(&tool_request, &result)
                .map_err(|error| {
                    NativeBlockoutCompatError::unavailable(
                        "NATIVE_PRODUCT_TOOL_RESPONSE_INVALID",
                        error.message,
                    )
                })?;
            if result.status != ProductToolExecutionStatus::Completed {
                if result.status == ProductToolExecutionStatus::Cancelled
                    || cancellation.is_cancelled()
                {
                    return Err(native_blockout_cancelled());
                }
                let code = result
                    .error_code
                    .unwrap_or_else(|| "NATIVE_PRODUCT_TOOL_FAILED".to_string());
                let message = result.message.unwrap_or_else(|| {
                    "Rust Product Tool candidate generation failed.".to_string()
                });
                return Err(if result.status == ProductToolExecutionStatus::Rejected {
                    NativeBlockoutCompatError::invalid(code, message)
                } else {
                    NativeBlockoutCompatError::unavailable(code, message)
                });
            }
        }
        if cancellation.is_cancelled() {
            let _ = executor.cancel(cancellation_id, cancellation_token).await;
            return Err(native_blockout_cancelled());
        }
        let preview = executor
            .retain_compatibility_preview(&execution_id, &turn_id)
            .map_err(native_blockout_product_tool_error)?;
        if preview.turn_id != turn_id {
            let _ = executor.discard_preview(&preview.preview_id);
            return Err(NativeBlockoutCompatError::unavailable(
                "NATIVE_PREVIEW_IDENTITY_MISMATCH",
                "Rust Product Tool preview identity did not match its build request.",
            ));
        }
        preview
            .validate()
            .map_err(native_blockout_product_tool_error)?;
        Ok(preview)
    }

    async fn native_build_blockout(
        &self,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Result<Value, NativeBlockoutCompatError> {
        let input: NativeBuildBlockoutRequest = native_blockout_request_json(request)?;
        native_blockout_require_idempotency(request, &input.client_request_id)?;
        native_blockout_validate_client_request_id(&input.client_request_id)?;
        native_blockout_validate_direction_id(&input.direction_id)?;
        native_blockout_validate_profile(&input.presentation_profile)?;
        if input.variation_index > 2 {
            return Err(NativeBlockoutCompatError::invalid(
                "BLOCKOUT_VARIATION_INVALID",
                "Blockout variation_index must be between 0 and 2.",
            ));
        }
        let (plan_id, domain_pack_id, project_id) =
            native_blockout_plan_facts(&input.plan, &input.direction_id)?;
        let variant_id = native_blockout_variant_id(
            input.variant_id.as_deref(),
            &domain_pack_id,
            &input.direction_id,
            input.variation_index,
        )?;
        let build_request_sha256 = native_blockout_semantic_sha256(&json!({
            "client_request_id": input.client_request_id,
            "plan": input.plan,
            "direction_id": input.direction_id,
            "variant_id": input.variant_id,
            "variation_index": input.variation_index,
            "presentation_profile": input.presentation_profile,
        }))?;
        let artifact_id = format!(
            "artifact_{}",
            &sha256_hex(input.client_request_id.as_bytes())[..24]
        );
        let executor = self.native_product_tools()?;
        self.prune_native_blockouts(&executor)?;

        let existing = {
            let mut state = self
                .inner
                .native_blockouts
                .lock()
                .map_err(|_| native_blockout_state_unavailable())?;
            if let Some(existing) = state.candidates.get(&artifact_id).cloned() {
                if existing.build_client_request_id != input.client_request_id
                    || existing.build_request_sha256 != build_request_sha256
                {
                    return Err(NativeBlockoutCompatError::conflict(
                        "IDEMPOTENCY_CONFLICT",
                        "Idempotency-Key was reused with a different blockout request.",
                    ));
                }
                Some(existing)
            } else {
                if let Some(in_flight_hash) = state.builds_in_flight.get(&artifact_id) {
                    return Err(if in_flight_hash == &build_request_sha256 {
                        NativeBlockoutCompatError::unavailable(
                            "BLOCKOUT_BUILD_IN_FLIGHT",
                            "The same Rust blockout build is already in flight.",
                        )
                    } else {
                        NativeBlockoutCompatError::conflict(
                            "IDEMPOTENCY_CONFLICT",
                            "Idempotency-Key is already bound to another blockout request.",
                        )
                    });
                }
                state
                    .builds_in_flight
                    .insert(artifact_id.clone(), build_request_sha256.clone());
                None
            }
        };

        if let Some(existing) = existing {
            let preview = executor
                .preview_artifact(&existing.preview_id)
                .map_err(native_blockout_product_tool_error)?
                .ok_or_else(|| {
                    NativeBlockoutCompatError::not_found(
                        "BLOCKOUT_NOT_FOUND",
                        "Blockout candidate expired or was already consumed.",
                    )
                })?;
            return native_blockout_build_payload(&existing, &preview);
        }

        let preview_result = self
            .execute_native_blockout_preview(
                input.plan,
                &input.direction_id,
                &variant_id,
                &input.presentation_profile,
                &artifact_id,
                cancellation.clone(),
            )
            .await;
        let mut state = self
            .inner
            .native_blockouts
            .lock()
            .map_err(|_| native_blockout_state_unavailable())?;
        state.builds_in_flight.remove(&artifact_id);
        let preview = match preview_result {
            Ok(preview) if !cancellation.is_cancelled() => preview,
            Ok(preview) => {
                drop(state);
                let _ = executor.discard_preview(&preview.preview_id);
                return Err(native_blockout_cancelled());
            }
            Err(error) => return Err(error),
        };
        let created_at_unix_ms = preview.created_at_unix_ms;
        let expires_at_unix_ms = preview
            .expires_at_unix_ms
            .min(created_at_unix_ms.saturating_add(NATIVE_BLOCKOUT_COMPAT_TTL_MS));
        let candidate = NativeBlockoutCompatCandidate {
            artifact_id: artifact_id.clone(),
            preview_id: preview.preview_id.clone(),
            turn_id: preview.turn_id.clone(),
            build_client_request_id: input.client_request_id,
            build_request_sha256,
            plan_id,
            direction_id: input.direction_id,
            variant_id,
            variation_index: input.variation_index,
            presentation_profile: input.presentation_profile,
            domain_pack_id,
            project_id,
            expires_at_unix_ms,
            segment_idempotency: HashMap::new(),
            segment_idempotency_order: VecDeque::new(),
        };
        let payload = native_blockout_build_payload(&candidate, &preview)?;
        let evicted = state.insert_candidate(candidate);
        drop(state);
        for preview_id in evicted {
            executor
                .discard_preview(&preview_id)
                .map_err(native_blockout_product_tool_error)?;
        }
        Ok(payload)
    }

    async fn native_segment_blockout(
        &self,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Result<Value, NativeBlockoutCompatError> {
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        let input: NativeSegmentBlockoutRequest = native_blockout_request_json(request)?;
        native_blockout_require_idempotency(request, &input.client_request_id)?;
        native_blockout_validate_client_request_id(&input.client_request_id)?;
        native_blockout_validate_direction_id(&input.direction_id)?;
        native_blockout_validate_profile(&input.presentation_profile)?;
        if input.variation_index > 2 {
            return Err(NativeBlockoutCompatError::invalid(
                "BLOCKOUT_VARIATION_INVALID",
                "Blockout variation_index must be between 0 and 2.",
            ));
        }
        let artifact_id = input.artifact_id.as_deref().ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "BLOCKOUT_ARTIFACT_REQUIRED",
                "Rust segmentation requires the artifact_id returned by blockout build.",
            )
        })?;
        native_blockout_validate_artifact_id(artifact_id)?;
        let (plan_id, domain_pack_id, project_id) =
            native_blockout_plan_facts(&input.plan, &input.direction_id)?;
        let executor = self.native_product_tools()?;
        self.prune_native_blockouts(&executor)?;
        let request_sha256 = native_blockout_semantic_sha256(&json!({
            "client_request_id": input.client_request_id,
            "plan": input.plan,
            "direction_id": input.direction_id,
            "variant_id": input.variant_id,
            "variation_index": input.variation_index,
            "presentation_profile": input.presentation_profile,
            "artifact_id": input.artifact_id,
        }))?;
        let candidate = {
            let mut state = self
                .inner
                .native_blockouts
                .lock()
                .map_err(|_| native_blockout_state_unavailable())?;
            let candidate = state.candidates.get_mut(artifact_id).ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "BLOCKOUT_NOT_FOUND",
                    "Blockout candidate is missing, expired, or already consumed.",
                )
            })?;
            let requested_variant = input.variant_id.as_deref().unwrap_or(&candidate.variant_id);
            if candidate.plan_id != plan_id
                || candidate.direction_id != input.direction_id
                || candidate.variant_id != requested_variant
                || candidate.variation_index != input.variation_index
                || candidate.presentation_profile != input.presentation_profile
                || candidate.domain_pack_id != domain_pack_id
                || (candidate.project_id.is_some()
                    && project_id.is_some()
                    && candidate.project_id != project_id)
            {
                return Err(NativeBlockoutCompatError::conflict(
                    "BLOCKOUT_IDENTITY_CONFLICT",
                    "Segmentation must use the exact plan, direction, variation, and profile of its Rust blockout candidate.",
                ));
            }
            if let Some(previous) = candidate.segment_idempotency.get(&input.client_request_id) {
                if previous != &request_sha256 {
                    return Err(NativeBlockoutCompatError::conflict(
                        "IDEMPOTENCY_CONFLICT",
                        "Idempotency-Key was reused with a different segmentation request.",
                    ));
                }
            } else {
                candidate
                    .segment_idempotency
                    .insert(input.client_request_id.clone(), request_sha256);
                candidate
                    .segment_idempotency_order
                    .push_back(input.client_request_id.clone());
                while candidate.segment_idempotency.len()
                    > NATIVE_BLOCKOUT_SEGMENT_IDEMPOTENCY_LIMIT
                {
                    let Some(expired) = candidate.segment_idempotency_order.pop_front() else {
                        break;
                    };
                    candidate.segment_idempotency.remove(&expired);
                }
            }
            candidate.clone()
        };
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        let preview = executor
            .preview_artifact(&candidate.preview_id)
            .map_err(native_blockout_product_tool_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "BLOCKOUT_NOT_FOUND",
                    "Blockout candidate is missing, expired, or already consumed.",
                )
            })?;
        native_blockout_segment_payload(&candidate, &preview)
    }

    async fn native_production_blockout_preview(
        &self,
        preview: &NativePreviewArtifact,
        cancellation: CancellationToken,
    ) -> Result<NativePreviewArtifact, NativeBlockoutCompatError> {
        if preview.readback.artifact_profile_id == "production_concept" {
            return Ok(preview.clone());
        }
        if preview.readback.artifact_profile_id != "interactive_preview" {
            return Err(NativeBlockoutCompatError::conflict(
                "BLOCKOUT_PROFILE_INVALID",
                "Only interactive_preview or production_concept candidates may be committed.",
            ));
        }
        let input = RestrictedGeometryInput {
            schema_version: "RestrictedGeometryInput@1".into(),
            shape_program: preview.shape_program.clone(),
            profile_sketch: None,
            section_set: None,
            surface_adornment_programs: Vec::new(),
            surface_layer_input: None,
            quality_profile: RestrictedQualityProfile {
                profile_id: "production_concept".into(),
                runtime_manifest_version: "ShapeProgramRuntimeManifest@1".into(),
                max_triangle_count: 150_000,
                render_width: 128,
                render_height: 128,
                require_closed_manifold: true,
                require_surface_provenance: true,
            },
        };
        input
            .validate()
            .map_err(|error| NativeBlockoutCompatError::invalid(error.code, error.message))?;
        let production = RestrictedGeometryPort::build_compile_render(self, input, cancellation)
            .await
            .map_err(native_blockout_restricted_geometry_error)?;
        let mut upgraded = preview.clone();
        upgraded.glb_bytes = production.glb_bytes;
        upgraded.glb_sha256 = production.glb_sha256;
        upgraded.readback = production.readback;
        upgraded.views = production.views;
        upgraded.view_sha256 = production.view_sha256;
        upgraded.renderer_id = production.renderer_id;
        // The formal decision is sealed to the transient GLB shown to the
        // user. A derived production artifact has a different verified hash
        // and therefore must not carry that preview-only binding.
        upgraded.formal_provenance = None;
        upgraded
            .validate()
            .map_err(native_blockout_product_tool_error)?;
        Ok(upgraded)
    }

    async fn native_interactive_blockout_preview(
        &self,
        preview: &NativePreviewArtifact,
        cancellation: CancellationToken,
    ) -> Result<NativePreviewArtifact, NativeBlockoutCompatError> {
        if preview.readback.artifact_profile_id == "interactive_preview" {
            return Ok(preview.clone());
        }
        if preview.readback.artifact_profile_id != "production_concept" {
            return Err(NativeBlockoutCompatError::conflict(
                "BLOCKOUT_PROFILE_INVALID",
                "Only interactive_preview or production_concept candidates may be committed.",
            ));
        }
        // A showcase-authored ShapeProgram carries the production triangle
        // ceiling in its immutable JSON. Keep that exact ShapeProgram and use
        // the interactive render dimensions while the actual readback remains
        // the hard bound; this creates a true interactive_profile GLB without
        // rewriting the confirmed geometry program.
        let input = RestrictedGeometryInput {
            schema_version: "RestrictedGeometryInput@1".into(),
            shape_program: preview.shape_program.clone(),
            profile_sketch: None,
            section_set: None,
            surface_adornment_programs: Vec::new(),
            surface_layer_input: None,
            quality_profile: RestrictedQualityProfile {
                profile_id: "interactive_preview".into(),
                runtime_manifest_version: "ShapeProgramRuntimeManifest@1".into(),
                max_triangle_count: 100_000,
                render_width: 320,
                render_height: 320,
                require_closed_manifold: true,
                require_surface_provenance: true,
            },
        };
        input
            .validate()
            .map_err(|error| NativeBlockoutCompatError::invalid(error.code, error.message))?;
        let interactive = RestrictedGeometryPort::build_compile_render(self, input, cancellation)
            .await
            .map_err(native_blockout_restricted_geometry_error)?;
        let mut downgraded = preview.clone();
        downgraded.glb_bytes = interactive.glb_bytes;
        downgraded.glb_sha256 = interactive.glb_sha256;
        downgraded.readback = interactive.readback;
        downgraded.views = interactive.views;
        downgraded.view_sha256 = interactive.view_sha256;
        downgraded.renderer_id = interactive.renderer_id;
        // See the production conversion above: this derived profile is not
        // the exact GLB referenced by SingleResultDecision@1.
        downgraded.formal_provenance = None;
        downgraded
            .validate()
            .map_err(native_blockout_product_tool_error)?;
        Ok(downgraded)
    }

    fn best_effort_cleanup_committed_blockout(
        &self,
        executor: &NativeProductToolExecutor,
        candidate: &NativeBlockoutCompatCandidate,
    ) {
        if executor
            .consume_preview(&candidate.preview_id, &candidate.turn_id)
            .is_err()
        {
            let _ = executor.discard_preview(&candidate.preview_id);
        }
        if let Ok(mut state) = self.inner.native_blockouts.lock() {
            state.remove_candidate(&candidate.artifact_id);
        }
    }

    fn best_effort_cleanup_replayed_blockout(&self, artifact_id: &str) {
        let candidate = self
            .inner
            .native_blockouts
            .lock()
            .ok()
            .and_then(|state| state.candidates.get(artifact_id).cloned());
        let executor = self.native_product_tools().ok();
        if let (Some(candidate), Some(executor)) = (candidate, executor) {
            self.best_effort_cleanup_committed_blockout(&executor, &candidate);
        }
    }

    async fn native_commit_blockout(
        &self,
        request: &PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> Result<Value, NativeBlockoutCompatError> {
        let input: NativeCommitBlockoutRequest = native_blockout_request_json(request)?;
        native_blockout_require_idempotency(request, &input.client_request_id)?;
        native_blockout_validate_client_request_id(&input.client_request_id)?;
        native_blockout_validate_artifact_id(&input.artifact_id)?;
        let project_id = input.project_id.as_deref().ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "PROJECT_REQUIRED",
                "A valid open Project is required to commit the Rust blockout candidate.",
            )
        })?;
        if project_id.is_empty() || project_id.len() > 160 || !valid_stable_id(project_id) {
            return Err(NativeBlockoutCompatError::invalid(
                "PROJECT_ID_INVALID",
                "Project identity is outside the bounded API contract.",
            ));
        }
        if input.summary.is_empty()
            || input.summary.chars().count() > 500
            || input.summary.contains('\0')
        {
            return Err(NativeBlockoutCompatError::invalid(
                "BLOCKOUT_SUMMARY_INVALID",
                "Blockout commit summary must contain between 1 and 500 characters.",
            ));
        }
        let rust_core = self.inner.rust_core.as_ref().ok_or_else(|| {
            NativeBlockoutCompatError::unavailable(
                "RUST_CORE_UNAVAILABLE",
                "Rust product core is unavailable.",
            )
        })?;
        let repository = rust_core.repository();
        let asset_version_id = format!(
            "assetver_{}",
            &sha256_hex(input.client_request_id.as_bytes())[..24]
        );
        let quality_report_id = native_blockout_quality_report_id(&asset_version_id);
        if let Some(existing) = repository
            .read_candidate_bundle(&input.artifact_id, &asset_version_id, &quality_report_id)
            .map_err(native_blockout_core_error)?
        {
            if existing.version.project_id != project_id
                || existing.version.artifact_id != input.artifact_id
                || existing.version.summary != input.summary
            {
                return Err(NativeBlockoutCompatError::conflict(
                    "IDEMPOTENCY_CONFLICT",
                    "Idempotency-Key was reused with a different blockout commit request.",
                ));
            }
            let response = native_blockout_asset_version_payload(&existing.version)?;
            self.best_effort_cleanup_replayed_blockout(&input.artifact_id);
            return Ok(response);
        }
        repository
            .project(project_id)
            .map_err(native_blockout_core_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "PROJECT_REQUIRED",
                    "The requested Project does not exist or is unavailable.",
                )
            })?;
        if repository
            .head(project_id)
            .map_err(native_blockout_core_error)?
            .is_some()
        {
            return Err(NativeBlockoutCompatError::conflict(
                "BLOCKOUT_PROJECT_ALREADY_INITIALIZED",
                "Initial blockout confirmation only creates version 1 for an empty Project.",
            ));
        }

        let executor = self.native_product_tools()?;
        self.prune_native_blockouts(&executor)?;
        let candidate = self
            .inner
            .native_blockouts
            .lock()
            .map_err(|_| native_blockout_state_unavailable())?
            .candidates
            .get(&input.artifact_id)
            .cloned()
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "BLOCKOUT_NOT_FOUND",
                    "Blockout candidate is missing, expired, or already consumed.",
                )
            })?;
        if candidate
            .project_id
            .as_deref()
            .is_some_and(|bound| bound != project_id)
        {
            return Err(NativeBlockoutCompatError::conflict(
                "PROJECT_MISMATCH",
                "Blockout candidate belongs to a different Project.",
            ));
        }
        let preview = executor
            .preview_artifact(&candidate.preview_id)
            .map_err(native_blockout_product_tool_error)?
            .ok_or_else(|| {
                NativeBlockoutCompatError::not_found(
                    "BLOCKOUT_NOT_FOUND",
                    "Blockout candidate is missing, expired, or already consumed.",
                )
            })?;
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        let interactive_preview = self
            .native_interactive_blockout_preview(&preview, cancellation.clone())
            .await?;
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        let production_preview = self
            .native_production_blockout_preview(&preview, cancellation.clone())
            .await?;
        if preview.shape_program != production_preview.shape_program
            || preview.assembly != production_preview.assembly
            || preview.readback.shape_program_sha256
                != production_preview.readback.shape_program_sha256
            || interactive_preview.shape_program != production_preview.shape_program
            || interactive_preview.assembly != production_preview.assembly
            || interactive_preview.readback.shape_program_sha256
                != production_preview.readback.shape_program_sha256
        {
            return Err(NativeBlockoutCompatError::conflict(
                "BLOCKOUT_IDENTITY_CONFLICT",
                "Rust preview no longer matches its interactive and production recompiles.",
            ));
        }
        let interactive_verified = native_verified_geometry_readback(
            &interactive_preview.glb_bytes,
            &interactive_preview.readback,
            "interactive_preview",
            &interactive_preview.shape_program,
        )?;
        let production_verified = native_verified_geometry_readback(
            &production_preview.glb_bytes,
            &production_preview.readback,
            "production_concept",
            &production_preview.shape_program,
        )?;
        if interactive_verified.glb_sha256 != interactive_preview.glb_sha256
            || production_verified.glb_sha256 != production_preview.glb_sha256
        {
            return Err(NativeBlockoutCompatError::conflict(
                "RESTRICTED_GEOMETRY_READBACK_MISMATCH",
                "Canonical GLB identity does not match the restricted geometry result.",
            ));
        }
        let (parts, assembly_graph, material_bindings) =
            native_blockout_parts_and_graph(&production_preview, Some(project_id))?;
        // The transient preview creation time is sealed into the candidate so
        // concurrent retries of the same idempotency key build byte-for-byte
        // identical bundle input.
        let timestamp = format!("unix_ms_{}", preview.created_at_unix_ms);
        let persisted_candidate = BlockoutCandidate {
            artifact_id: candidate.artifact_id.clone(),
            project_id: Some(project_id.to_string()),
            plan_id: candidate.plan_id.clone(),
            direction_id: candidate.direction_id.clone(),
            domain_pack_id: candidate.domain_pack_id.clone(),
            status: CandidateStatus::Candidate,
            candidate: json!({
                "schema_version": "AgentBlockoutCandidate@1",
                "artifact_id": candidate.artifact_id,
                "plan_id": candidate.plan_id,
                "direction_id": candidate.direction_id,
                "variant_id": candidate.variant_id,
                "variation_index": candidate.variation_index,
                "presentation_profile": candidate.presentation_profile,
                "domain_pack_id": candidate.domain_pack_id,
                "artifact_profile_id": production_verified.artifact_profile_id.clone(),
                "readback": production_verified.clone(),
                "restricted_executor_readback": production_preview.readback.clone(),
                "view_sha256": production_preview.view_sha256,
                "renderer_id": production_preview.renderer_id,
                "component_recipe_candidate_sha256": production_preview.recipe_candidate_sha256,
                "permanent_side_effects_before_confirmation": 0
            }),
            shape_program: production_preview.shape_program.clone(),
            assembly_graph: assembly_graph.clone(),
            material_bindings: material_bindings.clone(),
            glb_sha256: production_preview.glb_sha256.clone(),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        let version = AgentAssetVersion {
            asset_version_id: asset_version_id.clone(),
            project_id: project_id.to_string(),
            parent_asset_version_id: None,
            version_no: 1,
            status: AssetVersionStatus::Committed,
            summary: input.summary,
            stage: AssetStage::SegmentedConcept,
            plan_id: candidate.plan_id.clone(),
            direction_id: candidate.direction_id.clone(),
            domain_pack_id: candidate.domain_pack_id.clone(),
            artifact_id: candidate.artifact_id.clone(),
            parts,
            shape_program: production_preview.shape_program.clone(),
            assembly_graph,
            material_bindings,
            created_at: timestamp.clone(),
        };
        let quality = native_geometry_quality_report(
            project_id,
            &asset_version_id,
            &production_preview.readback,
            &production_preview.renderer_id,
            &production_preview.view_sha256,
            &production_verified,
            &timestamp,
        )?;
        let response = native_blockout_asset_version_payload(&version)?;

        // This is the final cancellable boundary. The synchronous Core call
        // stages both GLBs and then owns one IMMEDIATE transaction; once that
        // call begins, a late cancellation must not turn a committed bundle
        // into an HTTP cancellation error.
        if cancellation.is_cancelled() {
            return Err(native_blockout_cancelled());
        }
        #[cfg(test)]
        if let Ok(mut hook) = self.inner.native_commit_before_bundle_hook.lock() {
            if let Some(hook) = hook.take() {
                hook();
            }
        }
        repository
            .commit_candidate_bundle(
                persisted_candidate,
                &production_preview.glb_bytes,
                &interactive_preview.glb_bytes,
                &version,
                &quality,
            )
            .map_err(native_blockout_core_error)?;

        // Transient Product Tool state is not authoritative after the bundle
        // commit. Cleanup is deliberately best effort and cannot make an
        // already committed immutable version appear to have failed.
        self.best_effort_cleanup_committed_blockout(&executor, &candidate);
        Ok(response)
    }

    fn cancel_connection(&self, connection_id: &str) {
        let removed = self
            .inner
            .subscriptions
            .lock()
            .map(|mut subscriptions| {
                let stream_ids = subscriptions
                    .iter()
                    .filter_map(|(stream_id, handle)| {
                        (handle.connection_id == connection_id).then(|| stream_id.clone())
                    })
                    .collect::<Vec<_>>();
                stream_ids
                    .into_iter()
                    .filter_map(|stream_id| subscriptions.remove(&stream_id))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for handle in removed {
            handle.cancellation.cancel();
            if let Some(task) = handle.task {
                task.abort();
            }
        }
    }

    async fn open_sse(
        &self,
        path: &str,
        last_event_id: Option<&str>,
    ) -> Result<reqwest::Response, RpcError> {
        let mut request = self
            .inner
            .client
            .get(format!("{}{}", self.inner.endpoint.origin(), path))
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CACHE_CONTROL, "no-store");
        if let Some(last_event_id) = last_event_id {
            request = request.header("last-event-id", last_event_id);
        }
        let response = request.send().await.map_err(backend_unavailable)?;
        if !response.status().is_success() {
            return Err(backend_unavailable(format!(
                "SSE compatibility endpoint returned HTTP {}.",
                response.status().as_u16()
            )));
        }
        let event_stream = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/event-stream"));
        if !event_stream {
            return Err(malformed_upstream(
                "SSE compatibility endpoint did not return text/event-stream.",
            ));
        }
        Ok(response)
    }

    fn server(&self) -> Result<Arc<AppServer>, RpcError> {
        self.inner
            .server
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(|| RpcError::internal("ForgeCAD app-server bridge is unavailable."))
    }

    async fn post_k002_internal<TRequest, TResponse>(
        &self,
        path: &str,
        request: &TRequest,
        cancellation: CancellationToken,
    ) -> Result<TResponse, K002InternalCallError>
    where
        TRequest: Serialize + ?Sized,
        TResponse: DeserializeOwned,
    {
        if !path.starts_with("/api/v1/internal/k002/") || path.contains('?') {
            return Err(K002InternalCallError::InvalidResponse);
        }
        let body =
            serde_json::to_vec(request).map_err(|_| K002InternalCallError::InvalidResponse)?;
        if body.len() > MAX_K002_INTERNAL_JSON_BYTES {
            return Err(K002InternalCallError::InvalidResponse);
        }
        let send = self
            .inner
            .client
            .post(format!("{}{}", self.inner.endpoint.origin(), path))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(
                K002_INTERNAL_CAPABILITY_HEADER,
                self.inner.internal_capability_token.as_ref(),
            )
            .body(body)
            .send();
        let mut response = k002_cancellation_aware(send, cancellation.clone())
            .await?
            .map_err(|_| K002InternalCallError::Transport)?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_K002_INTERNAL_JSON_BYTES as u64)
        {
            return Err(K002InternalCallError::InvalidResponse);
        }
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        loop {
            let chunk = k002_cancellation_aware(response.chunk(), cancellation.clone())
                .await?
                .map_err(|_| K002InternalCallError::Transport)?;
            let Some(chunk) = chunk else { break };
            if bytes.len().saturating_add(chunk.len()) > MAX_K002_INTERNAL_JSON_BYTES {
                return Err(K002InternalCallError::InvalidResponse);
            }
            bytes.extend_from_slice(&chunk);
        }
        if !(200..300).contains(&status) {
            let envelope = serde_json::from_slice::<K002InternalErrorEnvelope>(&bytes)
                .map_err(|_| K002InternalCallError::InvalidResponse)?;
            if !valid_stable_id(&envelope.error.code)
                || envelope.error.message.is_empty()
                || envelope.error.message.len() > 2_000
                || envelope
                    .error
                    .message
                    .chars()
                    .any(|character| character == '\0')
            {
                return Err(K002InternalCallError::InvalidResponse);
            }
            return Err(K002InternalCallError::Rejected {
                status,
                code: envelope.error.code,
                recoverable: envelope.error.recoverable,
            });
        }
        serde_json::from_slice(&bytes).map_err(|_| K002InternalCallError::InvalidResponse)
    }

    fn next_restricted_geometry_identity(
        &self,
        phase: &str,
        shape_program_sha256: &str,
    ) -> RestrictedGeometryPhaseIdentity {
        let sequence = self
            .inner
            .geometry_sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nonce = sha256_hex(
            format!(
                "{}:{shape_program_sha256}:{phase}:{sequence}",
                self.inner.internal_capability_token
            )
            .as_bytes(),
        );
        RestrictedGeometryPhaseIdentity {
            execution_id: format!("geom_{phase}_{sequence}_{}", &nonce[..16]),
            idempotency_key: format!("geom_idem_{phase}_{sequence}_{}", &nonce[..16]),
            cancellation_id: format!("geom_cancel_{phase}_{sequence}_{}", &nonce[..16]),
            cancellation_token: format!("geomct_{nonce}"),
        }
    }

    async fn post_restricted_geometry<TRequest, TResponse>(
        &self,
        path: &str,
        request: &TRequest,
    ) -> Result<TResponse, RestrictedGeometryCallError>
    where
        TRequest: Serialize + ?Sized,
        TResponse: DeserializeOwned,
    {
        if !matches!(
            path,
            RESTRICTED_GEOMETRY_EXECUTE_PATH | RESTRICTED_GEOMETRY_CANCEL_PATH
        ) {
            return Err(RestrictedGeometryCallError::InvalidResponse);
        }
        let body = serde_json::to_vec(request)
            .map_err(|_| RestrictedGeometryCallError::InvalidResponse)?;
        if body.len() > MAX_RESTRICTED_GEOMETRY_REQUEST_BYTES {
            return Err(RestrictedGeometryCallError::InvalidResponse);
        }
        let mut response = self
            .inner
            .client
            .post(format!("{}{}", self.inner.endpoint.origin(), path))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(
                RESTRICTED_GEOMETRY_CAPABILITY_HEADER,
                self.inner.internal_capability_token.as_ref(),
            )
            .body(body)
            .send()
            .await
            .map_err(|_| RestrictedGeometryCallError::Transport)?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESTRICTED_GEOMETRY_RESPONSE_BYTES as u64)
        {
            return Err(RestrictedGeometryCallError::InvalidResponse);
        }
        let json_content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|media_type| media_type.trim() == "application/json")
            });
        if !json_content_type {
            return Err(RestrictedGeometryCallError::InvalidResponse);
        }
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        loop {
            let chunk = response
                .chunk()
                .await
                .map_err(|_| RestrictedGeometryCallError::Transport)?;
            let Some(chunk) = chunk else { break };
            if bytes.len().saturating_add(chunk.len()) > MAX_RESTRICTED_GEOMETRY_RESPONSE_BYTES {
                return Err(RestrictedGeometryCallError::InvalidResponse);
            }
            bytes.extend_from_slice(&chunk);
        }
        if !(200..300).contains(&status) {
            let envelope = serde_json::from_slice::<RestrictedGeometryErrorEnvelope>(&bytes)
                .map_err(|_| RestrictedGeometryCallError::InvalidResponse)?;
            if !valid_stable_id(&envelope.error.code)
                || envelope.error.message.is_empty()
                || envelope.error.message.len() > 2_000
                || envelope.error.message.contains('\0')
                || !envelope
                    .error
                    .details
                    .as_object()
                    .is_some_and(serde_json::Map::is_empty)
            {
                return Err(RestrictedGeometryCallError::InvalidResponse);
            }
            return Err(RestrictedGeometryCallError::Rejected {
                status,
                code: envelope.error.code,
                recoverable: envelope.error.recoverable,
            });
        }
        serde_json::from_slice(&bytes).map_err(|_| RestrictedGeometryCallError::InvalidResponse)
    }

    async fn post_restricted_geometry_cancel(
        &self,
        identity: &RestrictedGeometryPhaseIdentity,
    ) -> Result<(), RestrictedGeometryCallError> {
        let request = RestrictedGeometryCancellationRequest {
            schema_version: "RestrictedGeometryCancellationRequest@1",
            protocol_version: RESTRICTED_GEOMETRY_PROTOCOL_VERSION,
            cancellation_id: &identity.cancellation_id,
            cancellation_token: &identity.cancellation_token,
        };
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            self.post_restricted_geometry::<_, RestrictedGeometryCancellationResponse>(
                RESTRICTED_GEOMETRY_CANCEL_PATH,
                &request,
            ),
        )
        .await
        .map_err(|_| RestrictedGeometryCallError::Timeout)??;
        if response.schema_version != "RestrictedGeometryCancellationResult@1"
            || response.cancellation_id != identity.cancellation_id
            || !response.accepted
            || !response.tombstoned
        {
            return Err(RestrictedGeometryCallError::InvalidResponse);
        }
        Ok(())
    }

    async fn post_restricted_geometry_phase<TRequest>(
        &self,
        request: &TRequest,
        identity: &RestrictedGeometryPhaseIdentity,
        cancellation: CancellationToken,
        timeout_ms: u64,
    ) -> Result<RestrictedGeometryExecutionResponse, RestrictedGeometryCallError>
    where
        TRequest: Serialize + ?Sized,
    {
        // The Product Tool executor deliberately races caller cancellation and
        // may drop this future before its inner cancellation branch is polled.
        // A drop guard therefore owns the authenticated sidecar tombstone; it
        // is disarmed only after a fully received success response.
        let mut cancel_guard = RestrictedGeometryCancelGuard::new(self.clone(), identity.clone());
        let call = restricted_geometry_cancellation_aware(
            self.post_restricted_geometry::<_, RestrictedGeometryExecutionResponse>(
                RESTRICTED_GEOMETRY_EXECUTE_PATH,
                request,
            ),
            cancellation,
        );
        let outcome = tokio::time::timeout(
            Duration::from_millis(timeout_ms + RESTRICTED_GEOMETRY_NETWORK_GRACE_MS),
            call,
        )
        .await;
        match outcome {
            Ok(Ok(response)) => {
                cancel_guard.disarm();
                Ok(response)
            }
            Ok(Err(RestrictedGeometryCallError::Cancelled)) => {
                cancel_guard.disarm();
                let _ = self.post_restricted_geometry_cancel(identity).await;
                Err(RestrictedGeometryCallError::Cancelled)
            }
            Ok(Err(error)) => Err(error),
            Err(_) => {
                cancel_guard.disarm();
                let _ = self.post_restricted_geometry_cancel(identity).await;
                Err(RestrictedGeometryCallError::Timeout)
            }
        }
    }

    async fn build_restricted_geometry(
        &self,
        input: &RestrictedGeometryInput,
        cancellation: CancellationToken,
    ) -> Result<RestrictedGeometryOutput, RestrictedGeometryError> {
        input.validate()?;
        let shape_program_canonical_json = core_canonical_json(&input.shape_program)
            .map_err(|_| restricted_geometry_invalid_response())?;
        let shape_program_sha256 = semantic_sha256(&input.shape_program)
            .map_err(|_| restricted_geometry_invalid_response())?;
        if sha256_hex(shape_program_canonical_json.as_bytes()) != shape_program_sha256 {
            return Err(restricted_geometry_invalid_response());
        }
        let cache_key = restricted_geometry_compile_cache_key(input, &shape_program_sha256)?;
        let compiled = if let Ok(mut cache) = self.inner.geometry_cache.lock() {
            cache.get(&cache_key)
        } else {
            None
        };
        let compiled = if let Some(compiled) = compiled {
            compiled
        } else {
            let compile_identity =
                self.next_restricted_geometry_identity("compile", &shape_program_sha256);
            let compile_request = RestrictedGeometryCompileRequest {
                schema_version: "RestrictedGeometryExecutionRequest@1",
                protocol_version: RESTRICTED_GEOMETRY_PROTOCOL_VERSION,
                execution_id: &compile_identity.execution_id,
                idempotency_key: &compile_identity.idempotency_key,
                cancellation_id: &compile_identity.cancellation_id,
                cancellation_token: &compile_identity.cancellation_token,
                action: "compile_readback",
                timeout_ms: RESTRICTED_GEOMETRY_COMPILE_TIMEOUT_MS,
                artifact_profile_id: &input.quality_profile.profile_id,
                shape_program: &input.shape_program,
                shape_program_canonical_json: &shape_program_canonical_json,
                shape_program_sha256: &shape_program_sha256,
                profile_sketch: input.profile_sketch.as_ref(),
                section_set: input.section_set.as_ref(),
                surface_adornment_programs: &input.surface_adornment_programs,
                surface_layer_input: input.surface_layer_input.as_ref(),
            };
            let compile_response = self
                .post_restricted_geometry_phase(
                    &compile_request,
                    &compile_identity,
                    cancellation.clone(),
                    RESTRICTED_GEOMETRY_COMPILE_TIMEOUT_MS,
                )
                .await
                .map_err(restricted_geometry_port_error)?;
            let compiled = validate_restricted_geometry_compile_response(
                compile_response,
                &compile_identity.execution_id,
                input,
                &shape_program_sha256,
            )?;
            if let Ok(mut cache) = self.inner.geometry_cache.lock() {
                cache.insert(cache_key, compiled.clone());
            }
            compiled
        };

        let render_identity =
            self.next_restricted_geometry_identity("render", &shape_program_sha256);
        let render_request = RestrictedGeometryRenderRequest {
            schema_version: "RestrictedGeometryExecutionRequest@1",
            protocol_version: RESTRICTED_GEOMETRY_PROTOCOL_VERSION,
            execution_id: &render_identity.execution_id,
            idempotency_key: &render_identity.idempotency_key,
            cancellation_id: &render_identity.cancellation_id,
            cancellation_token: &render_identity.cancellation_token,
            action: "render",
            timeout_ms: RESTRICTED_GEOMETRY_RENDER_TIMEOUT_MS,
            artifact_handle: &compiled.artifact_handle,
            shape_program_sha256: &compiled.shape_program_sha256,
            render: RestrictedGeometryRenderRequestOptions {
                width: input.quality_profile.render_width,
                height: input.quality_profile.render_height,
                exploded_parts: Vec::new(),
            },
        };
        let render_response = self
            .post_restricted_geometry_phase(
                &render_request,
                &render_identity,
                cancellation,
                RESTRICTED_GEOMETRY_RENDER_TIMEOUT_MS,
            )
            .await
            .map_err(restricted_geometry_port_error)?;
        let (views, view_sha256, renderer_id) = validate_restricted_geometry_render_response(
            render_response,
            &render_identity.execution_id,
            &compiled,
        )?;
        let output = RestrictedGeometryOutput {
            schema_version: "RestrictedGeometryOutput@1".into(),
            topology_hash: compiled.shape_program_sha256.clone(),
            glb_sha256: compiled.glb_sha256,
            glb_bytes: compiled.glb_bytes,
            readback: compiled.readback,
            views,
            view_sha256,
            renderer_id,
        };
        output.validate(input)?;
        Ok(output)
    }

    async fn capture_persisted_turn_baseline(
        &self,
        target: &PersistedTurnCancellationTarget,
    ) -> Result<(), RpcError> {
        let thread = self.read_persisted_thread(&target.thread_id).await?;
        target.set_preexisting_turn_ids(persisted_thread_turn_ids(&thread, &target.thread_id)?);
        Ok(())
    }

    fn spawn_persisted_turn_cancellation(&self, target: PersistedTurnCancellationTarget) {
        let port = self.clone();
        tokio::spawn(async move {
            if let Err(error) = port.cancel_persisted_turn(target).await {
                eprintln!(
                    "ForgeCAD persisted Turn cancellation failed code={}: {}",
                    error.data.application_code, error.message
                );
            }
        });
    }

    async fn cancel_persisted_turn(
        &self,
        target: PersistedTurnCancellationTarget,
    ) -> Result<(), RpcError> {
        let mut last_terminal = None;
        for _ in 0..TURN_CANCEL_DISCOVERY_ATTEMPTS {
            match self.persisted_turn_state(&target).await? {
                PersistedTurnCancellationState::Running(turn_id) => {
                    self.post_persisted_turn_cancel(&target.thread_id, &turn_id)
                        .await?;
                    for check in 0..TURN_CANCEL_STABILITY_CHECKS {
                        if check > 0 {
                            tokio::time::sleep(TURN_CANCEL_STABILITY_DELAY).await;
                        }
                        match self
                            .persisted_turn_state_by_id(&target.thread_id, &turn_id)
                            .await?
                        {
                            PersistedTurnCancellationState::Cancelled(candidate)
                                if candidate == turn_id => {}
                            PersistedTurnCancellationState::Terminal { status, .. } => {
                                return Err(persisted_cancel_error(format!(
                                    "Persisted Turn {turn_id} changed to {status} after cancellation."
                                )));
                            }
                            _ => {
                                return Err(persisted_cancel_error(format!(
                                    "Persisted Turn {turn_id} was not readable as cancelled after cancellation."
                                )));
                            }
                        }
                    }
                    return Ok(());
                }
                PersistedTurnCancellationState::Cancelled(_) => return Ok(()),
                PersistedTurnCancellationState::Terminal { turn_id, status } => {
                    last_terminal = Some((turn_id, status));
                }
                PersistedTurnCancellationState::Missing => {}
            }
            tokio::time::sleep(TURN_CANCEL_DISCOVERY_DELAY).await;
        }
        if let Some((turn_id, status)) = last_terminal {
            return Err(persisted_cancel_error(format!(
                "Persisted Turn {turn_id} reached {status} before cancellation could be committed."
            )));
        }
        Err(persisted_cancel_error(
            "The cancelled compatibility request did not expose its persisted running Turn.",
        ))
    }

    async fn persisted_turn_state(
        &self,
        target: &PersistedTurnCancellationTarget,
    ) -> Result<PersistedTurnCancellationState, RpcError> {
        let value = self.read_persisted_thread(&target.thread_id).await?;
        persisted_turn_state(&value, target, None)
    }

    async fn persisted_turn_state_by_id(
        &self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<PersistedTurnCancellationState, RpcError> {
        let value = self.read_persisted_thread(thread_id).await?;
        persisted_turn_state(
            &value,
            &PersistedTurnCancellationTarget {
                thread_id: thread_id.to_string(),
                request_text: String::new(),
                preexisting_turn_ids: Arc::new(Mutex::new(Some(Vec::new()))),
            },
            Some(turn_id),
        )
    }

    async fn read_persisted_thread(&self, thread_id: &str) -> Result<Value, RpcError> {
        let url = format!(
            "{}{}/{}",
            self.inner.endpoint.origin(),
            "/api/v1/agent/threads",
            thread_id
        );
        let mut response = self
            .inner
            .client
            .get(url)
            .send()
            .await
            .map_err(backend_unavailable)?;
        if !response.status().is_success() {
            return Err(persisted_cancel_error(format!(
                "Reading the persisted Turn returned HTTP {}.",
                response.status().as_u16()
            )));
        }
        read_bounded_json_response(&mut response).await
    }

    async fn post_persisted_turn_cancel(
        &self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<(), RpcError> {
        let url = format!(
            "{}{}/{}{}",
            self.inner.endpoint.origin(),
            "/api/v1/agent/turns",
            turn_id,
            "/cancel"
        );
        let mut response = self
            .inner
            .client
            .post(url)
            .header("idempotency-key", format!("k001-rust-cancel-{turn_id}"))
            .send()
            .await
            .map_err(backend_unavailable)?;
        if !response.status().is_success() {
            return Err(persisted_cancel_error(format!(
                "Cancelling the persisted Turn returned HTTP {}.",
                response.status().as_u16()
            )));
        }
        let value = read_bounded_json_response(&mut response).await?;
        if value.get("thread_id").and_then(Value::as_str) != Some(thread_id)
            || value.get("turn_id").and_then(Value::as_str) != Some(turn_id)
            || value.get("status").and_then(Value::as_str) != Some("cancelled")
        {
            return Err(persisted_cancel_error(
                "The persisted Turn cancellation response was not the authoritative cancelled Turn.",
            ));
        }
        Ok(())
    }
}

impl LifecyclePersistencePort for LoopbackHttpPort {
    fn execute(
        &self,
        command: LifecyclePersistenceCommand,
        cancellation: CancellationToken,
    ) -> LifecyclePortFuture<LifecyclePersistenceResult> {
        let port = self.clone();
        Box::pin(async move {
            command.validate().map_err(|_| LifecyclePortError {
                code: "LIFECYCLE_PERSISTENCE_REQUEST_INVALID".to_string(),
                kind: LifecyclePortErrorKind::InvalidData,
                message: "The sealed lifecycle persistence request was invalid.".to_string(),
                recoverable: false,
            })?;
            let result = port
                .post_k002_internal::<_, LifecyclePersistenceResult>(
                    "/api/v1/internal/k002/lifecycle/execute",
                    &command,
                    cancellation,
                )
                .await
                .map_err(lifecycle_port_error)?;
            result.validate().map_err(|_| LifecyclePortError {
                code: "LIFECYCLE_PERSISTENCE_RESPONSE_INVALID".to_string(),
                kind: LifecyclePortErrorKind::InvalidData,
                message: "The lifecycle persistence port returned an invalid response.".to_string(),
                recoverable: false,
            })?;
            Ok(result)
        })
    }
}

impl ProductToolExecutorPort for LoopbackHttpPort {
    fn execute(
        &self,
        request: ProductToolExecutionRequest,
        cancellation: CancellationToken,
    ) -> ProductToolPortFuture {
        let port = self.clone();
        Box::pin(async move {
            request.validate().map_err(|_| ProductToolPortError {
                code: "PRODUCT_TOOL_REQUEST_INVALID".to_string(),
                kind: ProductToolPortErrorKind::InvalidResponse,
                message: "The sealed Product Tool request was invalid.".to_string(),
                recoverable: false,
            })?;
            let result = port
                .post_k002_internal::<_, ProductToolExecutionResult>(
                    "/api/v1/internal/k002/product-tools/execute",
                    &request,
                    cancellation,
                )
                .await
                .map_err(product_tool_port_error)?;
            result.validate().map_err(|_| ProductToolPortError {
                code: "PRODUCT_TOOL_RESPONSE_INVALID".to_string(),
                kind: ProductToolPortErrorKind::InvalidResponse,
                message: "The Product Tool port returned an invalid response.".to_string(),
                recoverable: false,
            })?;
            Ok(result)
        })
    }

    fn cancel(
        &self,
        cancellation_id: String,
        cancellation_token: String,
    ) -> ProductToolCancelFuture {
        let port = self.clone();
        Box::pin(async move {
            if !valid_stable_id(&cancellation_id) || !valid_stable_id(&cancellation_token) {
                return Err(ProductToolPortError {
                    code: "PRODUCT_TOOL_CANCELLATION_INVALID".to_string(),
                    kind: ProductToolPortErrorKind::InvalidResponse,
                    message: "The Product Tool cancellation capability was invalid.".to_string(),
                    recoverable: false,
                });
            }
            let request = K002ProductToolCancellationRequest {
                schema_version: "ProductToolCancellationRequest@1",
                cancellation_id: &cancellation_id,
                cancellation_token: &cancellation_token,
            };
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                port.post_k002_internal::<_, K002ProductToolCancellationResult>(
                    "/api/v1/internal/k002/product-tools/cancel",
                    &request,
                    CancellationToken::new(),
                ),
            )
            .await
            .map_err(|_| ProductToolPortError::timeout())?
            .map_err(product_tool_port_error)?;
            if result.schema_version != "ProductToolCancellationResult@1"
                || result.cancellation_id != cancellation_id
            {
                return Err(ProductToolPortError::invalid_response(
                    "The Product Tool cancellation response identity was invalid.",
                ));
            }
            Ok(result.accepted)
        })
    }
}

impl RestrictedGeometryPort for LoopbackHttpPort {
    fn build_compile_render(
        &self,
        input: RestrictedGeometryInput,
        cancellation: CancellationToken,
    ) -> RestrictedGeometryFuture {
        let port = self.clone();
        Box::pin(async move {
            input.validate()?;
            port.build_restricted_geometry(&input, cancellation).await
        })
    }
}

fn restricted_geometry_compile_cache_key(
    input: &RestrictedGeometryInput,
    shape_program_sha256: &str,
) -> Result<String, RestrictedGeometryError> {
    let mut value =
        serde_json::to_value(input).map_err(|_| restricted_geometry_invalid_response())?;
    let profile = value
        .get_mut("quality_profile")
        .and_then(Value::as_object_mut)
        .ok_or_else(restricted_geometry_invalid_response)?;
    // Render dimensions affect the thumbnail phase, not the compiled GLB.
    // Excluding them lets an interactive and production request share the
    // compile artifact only when their actual artifact profile is otherwise
    // identical; profile_id remains part of the key and keeps the two quality
    // contracts isolated.
    profile.remove("render_width");
    profile.remove("render_height");
    value["shape_program_sha256"] = Value::String(shape_program_sha256.to_string());
    Ok(sha256_hex(canonical_json(&value).as_bytes()))
}

fn validate_restricted_geometry_compile_response(
    response: RestrictedGeometryExecutionResponse,
    execution_id: &str,
    input: &RestrictedGeometryInput,
    expected_shape_program_sha256: &str,
) -> Result<CompiledRestrictedGeometry, RestrictedGeometryError> {
    if response.schema_version != "RestrictedGeometryExecutionResult@1"
        || response.protocol_version != RESTRICTED_GEOMETRY_PROTOCOL_VERSION
        || response.execution_id != execution_id
        || response.action != "compile_readback"
        || response.artifact_profile_id != input.quality_profile.profile_id
        || !valid_restricted_geometry_artifact_handle(&response.artifact_handle)
        || !is_sha256(&response.artifact_profile_sha256)
        || response.shape_program_sha256 != expected_shape_program_sha256
        || !is_sha256(&response.glb_sha256)
        || response.glb_byte_size < 20
        || response.glb_byte_size > MAX_RESTRICTED_GEOMETRY_GLB_BYTES as u64
        || response.triangle_count == 0
        || !valid_positive_bounds(&response.bounds_mm)
        || response.render_views.is_some()
        || response.render_view_sha256.is_some()
        || response.renderer_id.is_some()
        || !response.exploded_part_ids.is_empty()
        || response.exploded_unavailable_reason.is_some()
    {
        return Err(restricted_geometry_invalid_response());
    }
    let encoded_glb = response
        .glb_base64
        .as_ref()
        .ok_or_else(restricted_geometry_invalid_response)?;
    let glb_bytes = decode_canonical_base64(encoded_glb, MAX_RESTRICTED_GEOMETRY_GLB_BYTES)?;
    if glb_bytes.len() < 20
        || !glb_bytes.starts_with(b"glTF")
        || glb_bytes.len() as u64 != response.glb_byte_size
        || sha256_hex(&glb_bytes) != response.glb_sha256
    {
        return Err(restricted_geometry_invalid_response());
    }

    let readback = response
        .readback
        .as_ref()
        .and_then(Value::as_object)
        .ok_or_else(restricted_geometry_invalid_response)?;
    let artifact_profile = readback
        .get("artifact_profile")
        .and_then(Value::as_object)
        .ok_or_else(restricted_geometry_invalid_response)?;
    let mesh_count = readback_u32(readback, "mesh_count")?;
    let primitive_count = readback_u32(readback, "primitive_count")?;
    let material_count = readback_u32(readback, "material_count")?;
    let (
        material_zone_count,
        visual_texture_set_count,
        visual_texture_map_count,
        visual_texture_provenance_verified,
    ) = readback_visual_material_summary(readback)?;
    let surface_provenance = readback
        .get("surface_provenance")
        .and_then(Value::as_array)
        .ok_or_else(restricted_geometry_invalid_response)?;
    let derived_surface_provenance_present = !surface_provenance.is_empty();
    let derived_closed_manifold = derived_surface_provenance_present
        && surface_provenance.iter().all(|item| {
            item.get("closed").and_then(Value::as_bool) == Some(true)
                && item.get("boundary_edge_count").and_then(Value::as_u64) == Some(0)
                && item.get("non_manifold_edge_count").and_then(Value::as_u64) == Some(0)
                && item
                    .get("degenerate_triangle_count")
                    .and_then(Value::as_u64)
                    == Some(0)
        });
    let closed_manifold = readback
        .get("closed_manifold")
        .and_then(Value::as_bool)
        .ok_or_else(restricted_geometry_invalid_response)?;
    let surface_provenance_present = readback
        .get("surface_provenance_present")
        .and_then(Value::as_bool)
        .ok_or_else(restricted_geometry_invalid_response)?;
    if readback.get("schema_version").and_then(Value::as_str) != Some("GeometryCompileReadback@2")
        || readback
            .get("runtime_manifest_version")
            .and_then(Value::as_str)
            != Some("ShapeProgramRuntimeManifest@1")
        || readback.get("shape_program_sha256").and_then(Value::as_str)
            != Some(expected_shape_program_sha256)
        || readback.get("glb_sha256").and_then(Value::as_str) != Some(response.glb_sha256.as_str())
        || readback.get("glb_byte_size").and_then(Value::as_u64) != Some(response.glb_byte_size)
        || readback.get("triangle_count").and_then(Value::as_u64)
            != Some(u64::from(response.triangle_count))
        || readback_bounds(readback)? != response.bounds_mm
        || artifact_profile
            .get("artifact_profile_id")
            .and_then(Value::as_str)
            != Some(input.quality_profile.profile_id.as_str())
        || artifact_profile
            .get("profile_sha256")
            .and_then(Value::as_str)
            != Some(response.artifact_profile_sha256.as_str())
        || closed_manifold != derived_closed_manifold
        || surface_provenance_present != derived_surface_provenance_present
    {
        return Err(restricted_geometry_invalid_response());
    }
    Ok(CompiledRestrictedGeometry {
        artifact_handle: response.artifact_handle,
        artifact_profile_id: response.artifact_profile_id,
        artifact_profile_sha256: response.artifact_profile_sha256,
        shape_program_sha256: response.shape_program_sha256,
        glb_sha256: response.glb_sha256.clone(),
        glb_bytes,
        readback: RestrictedGeometryReadback {
            runtime_manifest_version: "ShapeProgramRuntimeManifest@1".into(),
            artifact_profile_id: input.quality_profile.profile_id.clone(),
            shape_program_sha256: expected_shape_program_sha256.to_string(),
            glb_sha256: response.glb_sha256,
            glb_byte_size: response.glb_byte_size,
            triangle_count: response.triangle_count,
            bounds_mm: response.bounds_mm,
            mesh_count,
            primitive_count,
            material_count,
            closed_manifold,
            surface_provenance_present,
            compile_readback_sha256: sha256_hex(
                canonical_json(&Value::Object(readback.clone())).as_bytes(),
            ),
            material_zone_count,
            visual_texture_set_count,
            visual_texture_map_count,
            visual_texture_provenance_verified,
        },
    })
}

/// Extracts only bounded, compiler-verified material facts for the native
/// V003 gate.  We deliberately do not forward the worker's full readback or
/// arbitrary texture metadata into Product Tool state.
fn readback_visual_material_summary(
    readback: &serde_json::Map<String, Value>,
) -> Result<(u32, u32, u32, bool), RestrictedGeometryError> {
    let zones = readback
        .get("material_zone_faces")
        .and_then(Value::as_array)
        .ok_or_else(restricted_geometry_invalid_response)?;
    let texture_sets = readback
        .get("visual_texture_sets")
        .and_then(Value::as_array)
        .ok_or_else(restricted_geometry_invalid_response)?;
    if zones.is_empty() || texture_sets.is_empty() || zones.len() > 512 || texture_sets.len() > 64 {
        return Err(restricted_geometry_invalid_response());
    }

    let required_roles = BTreeSet::from([
        "base_color",
        "metallic_roughness",
        "normal",
        "occlusion",
        "emissive",
    ]);
    let mut texture_zone_materials = BTreeSet::new();
    let mut map_count = 0_u32;
    for set in texture_sets {
        let object = set
            .as_object()
            .ok_or_else(restricted_geometry_invalid_response)?;
        let material_id = object
            .get("material_id")
            .and_then(Value::as_str)
            .ok_or_else(restricted_geometry_invalid_response)?;
        let map_roles = object
            .get("maps")
            .and_then(Value::as_array)
            .ok_or_else(restricted_geometry_invalid_response)?;
        if map_roles.len() != required_roles.len() {
            return Err(restricted_geometry_invalid_response());
        }
        let roles = map_roles
            .iter()
            .map(|map| {
                let map = map
                    .as_object()
                    .ok_or_else(restricted_geometry_invalid_response)?;
                if map.get("source").and_then(Value::as_str) != Some("forgecad_builtin")
                    || map.get("license").and_then(Value::as_str) != Some("not_applicable")
                    || !map
                        .get("sha256")
                        .and_then(Value::as_str)
                        .is_some_and(is_sha256)
                {
                    return Err(restricted_geometry_invalid_response());
                }
                map.get("texture_role")
                    .and_then(Value::as_str)
                    .ok_or_else(restricted_geometry_invalid_response)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if roles != required_roles {
            return Err(restricted_geometry_invalid_response());
        }
        map_count = map_count
            .checked_add(
                u32::try_from(map_roles.len())
                    .map_err(|_| restricted_geometry_invalid_response())?,
            )
            .ok_or_else(restricted_geometry_invalid_response)?;
        let zone_ids = object
            .get("material_zone_ids")
            .and_then(Value::as_array)
            .ok_or_else(restricted_geometry_invalid_response)?;
        if zone_ids.is_empty() || zone_ids.len() > 512 {
            return Err(restricted_geometry_invalid_response());
        }
        for zone_id in zone_ids {
            let zone_id = zone_id
                .as_str()
                .ok_or_else(restricted_geometry_invalid_response)?;
            if !zone_id.starts_with("zone_") {
                return Err(restricted_geometry_invalid_response());
            }
            texture_zone_materials.insert((zone_id.to_string(), material_id.to_string()));
        }
    }

    for zone in zones {
        let object = zone
            .as_object()
            .ok_or_else(restricted_geometry_invalid_response)?;
        let zone_id = object
            .get("material_zone_id")
            .and_then(Value::as_str)
            .ok_or_else(restricted_geometry_invalid_response)?;
        let material_id = object
            .get("material_id")
            .and_then(Value::as_str)
            .ok_or_else(restricted_geometry_invalid_response)?;
        if object.get("texture_ready").and_then(Value::as_bool) != Some(true)
            || !texture_zone_materials.contains(&(zone_id.to_string(), material_id.to_string()))
        {
            return Err(restricted_geometry_invalid_response());
        }
    }

    Ok((
        u32::try_from(zones.len()).map_err(|_| restricted_geometry_invalid_response())?,
        u32::try_from(texture_sets.len()).map_err(|_| restricted_geometry_invalid_response())?,
        map_count,
        true,
    ))
}

fn validate_restricted_geometry_render_response(
    response: RestrictedGeometryExecutionResponse,
    execution_id: &str,
    compiled: &CompiledRestrictedGeometry,
) -> Result<(BTreeMap<String, Vec<u8>>, BTreeMap<String, String>, String), RestrictedGeometryError>
{
    if response.schema_version != "RestrictedGeometryExecutionResult@1"
        || response.protocol_version != RESTRICTED_GEOMETRY_PROTOCOL_VERSION
        || response.execution_id != execution_id
        || response.action != "render"
        || response.artifact_handle != compiled.artifact_handle
        || response.artifact_profile_id != compiled.artifact_profile_id
        || response.artifact_profile_sha256 != compiled.artifact_profile_sha256
        || response.shape_program_sha256 != compiled.shape_program_sha256
        || response.glb_sha256 != compiled.glb_sha256
        || response.glb_byte_size != compiled.glb_bytes.len() as u64
        || response.triangle_count != compiled.readback.triangle_count
        || response.bounds_mm != compiled.readback.bounds_mm
        || response.readback.is_some()
        || response.glb_base64.is_some()
        || response.renderer_id.as_deref() != Some(RESTRICTED_GEOMETRY_RENDERER_ID)
        || !response.exploded_part_ids.is_empty()
        || response.exploded_unavailable_reason.is_some()
    {
        return Err(restricted_geometry_invalid_response());
    }
    let encoded_views = response
        .render_views
        .as_ref()
        .ok_or_else(restricted_geometry_invalid_response)?;
    let view_sha256 = response
        .render_view_sha256
        .as_ref()
        .ok_or_else(restricted_geometry_invalid_response)?;
    let required = RESTRICTED_GEOMETRY_REQUIRED_VIEWS
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if encoded_views.keys().cloned().collect::<BTreeSet<_>>() != required
        || view_sha256.keys().cloned().collect::<BTreeSet<_>>() != required
    {
        return Err(restricted_geometry_invalid_response());
    }
    let mut views = BTreeMap::new();
    for (view_id, encoded) in encoded_views {
        let expected_sha256 = view_sha256
            .get(view_id)
            .ok_or_else(restricted_geometry_invalid_response)?;
        if !is_sha256(expected_sha256) {
            return Err(restricted_geometry_invalid_response());
        }
        let png = decode_canonical_base64(encoded, MAX_RESTRICTED_GEOMETRY_VIEW_BYTES)?;
        if !png.starts_with(b"\x89PNG\r\n\x1a\n") || sha256_hex(&png) != *expected_sha256 {
            return Err(restricted_geometry_invalid_response());
        }
        views.insert(view_id.clone(), png);
    }
    Ok((
        views,
        response
            .render_view_sha256
            .ok_or_else(restricted_geometry_invalid_response)?,
        response
            .renderer_id
            .ok_or_else(restricted_geometry_invalid_response)?,
    ))
}

fn readback_u32(
    readback: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<u32, RestrictedGeometryError> {
    readback
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(restricted_geometry_invalid_response)
}

fn readback_bounds(
    readback: &serde_json::Map<String, Value>,
) -> Result<[f64; 3], RestrictedGeometryError> {
    let values = readback
        .get("bounds_mm")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(restricted_geometry_invalid_response)?;
    let bounds = [
        values[0]
            .as_f64()
            .ok_or_else(restricted_geometry_invalid_response)?,
        values[1]
            .as_f64()
            .ok_or_else(restricted_geometry_invalid_response)?,
        values[2]
            .as_f64()
            .ok_or_else(restricted_geometry_invalid_response)?,
    ];
    if !valid_positive_bounds(&bounds) {
        return Err(restricted_geometry_invalid_response());
    }
    Ok(bounds)
}

fn decode_canonical_base64(
    encoded: &str,
    max_bytes: usize,
) -> Result<Vec<u8>, RestrictedGeometryError> {
    if encoded.len() > max_bytes.saturating_mul(4).saturating_add(2) / 3 + 4 {
        return Err(restricted_geometry_invalid_response());
    }
    let bytes = BASE64
        .decode(encoded)
        .map_err(|_| restricted_geometry_invalid_response())?;
    if bytes.is_empty() || bytes.len() > max_bytes || BASE64.encode(&bytes) != encoded {
        return Err(restricted_geometry_invalid_response());
    }
    Ok(bytes)
}

fn valid_restricted_geometry_artifact_handle(value: &str) -> bool {
    value.strip_prefix("geomart_").is_some_and(|suffix| {
        (32..=160).contains(&suffix.len())
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    })
}

fn valid_positive_bounds(bounds: &[f64; 3]) -> bool {
    bounds.iter().all(|value| value.is_finite() && *value > 0.0)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_json(value: &Value) -> String {
    fn write_value(value: &Value, output: &mut String) {
        match value {
            Value::Null => output.push_str("null"),
            Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
            Value::Number(value) => output.push_str(&value.to_string()),
            Value::String(value) => output.push_str(
                &serde_json::to_string(value).expect("serializing a JSON string cannot fail"),
            ),
            Value::Array(values) => {
                output.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    write_value(value, output);
                }
                output.push(']');
            }
            Value::Object(values) => {
                output.push('{');
                let mut keys = values.keys().collect::<Vec<_>>();
                keys.sort_unstable();
                for (index, key) in keys.into_iter().enumerate() {
                    if index > 0 {
                        output.push(',');
                    }
                    output.push_str(
                        &serde_json::to_string(key)
                            .expect("serializing a JSON object key cannot fail"),
                    );
                    output.push(':');
                    write_value(&values[key], output);
                }
                output.push('}');
            }
        }
    }
    let mut output = String::new();
    write_value(value, &mut output);
    output
}

fn sha256_hex(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn restricted_geometry_invalid_response() -> RestrictedGeometryError {
    RestrictedGeometryError::execution(
        "RESTRICTED_GEOMETRY_RESPONSE_INVALID",
        "Restricted geometry returned bytes or readback outside the frozen contract.",
    )
}

fn restricted_geometry_port_error(error: RestrictedGeometryCallError) -> RestrictedGeometryError {
    match error {
        RestrictedGeometryCallError::Cancelled => RestrictedGeometryError::cancelled(),
        RestrictedGeometryCallError::Timeout => RestrictedGeometryError {
            code: "RESTRICTED_GEOMETRY_TIMEOUT".into(),
            kind: RestrictedGeometryErrorKind::Timeout,
            message: "Restricted geometry exceeded its bounded phase deadline.".into(),
            recoverable: true,
        },
        RestrictedGeometryCallError::Transport => RestrictedGeometryError {
            code: "RESTRICTED_GEOMETRY_UNAVAILABLE".into(),
            kind: RestrictedGeometryErrorKind::Execution,
            message: "The loopback restricted geometry executor was unavailable.".into(),
            recoverable: true,
        },
        RestrictedGeometryCallError::InvalidResponse => restricted_geometry_invalid_response(),
        RestrictedGeometryCallError::Rejected {
            status,
            code,
            recoverable,
        } => RestrictedGeometryError {
            kind: if code == "GEOMETRY_EXECUTION_CANCELLED" {
                RestrictedGeometryErrorKind::Cancelled
            } else if code == "UNSUPPORTED_RUNTIME_OPERATION" {
                RestrictedGeometryErrorKind::Unsupported
            } else if matches!(status, 408 | 504) {
                RestrictedGeometryErrorKind::Timeout
            } else if matches!(status, 400 | 413 | 415 | 422) {
                RestrictedGeometryErrorKind::InvalidInput
            } else {
                RestrictedGeometryErrorKind::Execution
            },
            code,
            message: "The restricted geometry executor rejected the frozen request.".into(),
            recoverable,
        },
    }
}

fn lifecycle_port_error(error: K002InternalCallError) -> LifecyclePortError {
    match error {
        K002InternalCallError::Cancelled => LifecyclePortError::cancelled(),
        K002InternalCallError::Transport => LifecyclePortError {
            code: "LIFECYCLE_PERSISTENCE_UNAVAILABLE".to_string(),
            kind: LifecyclePortErrorKind::Unavailable,
            message: "The lifecycle persistence compatibility port was unavailable.".to_string(),
            recoverable: true,
        },
        K002InternalCallError::InvalidResponse => LifecyclePortError {
            code: "LIFECYCLE_PERSISTENCE_RESPONSE_INVALID".to_string(),
            kind: LifecyclePortErrorKind::InvalidData,
            message: "The lifecycle persistence compatibility response was invalid.".to_string(),
            recoverable: false,
        },
        K002InternalCallError::Rejected {
            status,
            code,
            recoverable,
        } => LifecyclePortError {
            code,
            kind: match status {
                404 => LifecyclePortErrorKind::NotFound,
                409 => LifecyclePortErrorKind::Conflict,
                503..=599 => LifecyclePortErrorKind::Unavailable,
                _ => LifecyclePortErrorKind::InvalidData,
            },
            message: "The lifecycle persistence compatibility port rejected the sealed command."
                .to_string(),
            recoverable,
        },
    }
}

fn product_tool_port_error(error: K002InternalCallError) -> ProductToolPortError {
    match error {
        K002InternalCallError::Cancelled => ProductToolPortError::cancelled(),
        K002InternalCallError::Transport => ProductToolPortError {
            code: "PRODUCT_TOOL_PORT_UNAVAILABLE".to_string(),
            kind: ProductToolPortErrorKind::Unavailable,
            message: "The Product Tool compatibility port was unavailable.".to_string(),
            recoverable: true,
        },
        K002InternalCallError::InvalidResponse => ProductToolPortError::invalid_response(
            "The Product Tool compatibility response was invalid.",
        ),
        K002InternalCallError::Rejected {
            status,
            code,
            recoverable,
        } => ProductToolPortError {
            code,
            kind: if matches!(status, 408 | 504) {
                ProductToolPortErrorKind::Timeout
            } else if status >= 500 {
                ProductToolPortErrorKind::Unavailable
            } else {
                ProductToolPortErrorKind::InvalidResponse
            },
            message: "The Product Tool compatibility port rejected the sealed request.".to_string(),
            recoverable,
        },
    }
}

fn rust_catalog_response(
    request: &PreparedCompatHttpRequest,
) -> Option<Result<CompatHttpResponse, RpcError>> {
    if request.method != AllowedHttpMethod::Get {
        return None;
    }
    let route = request.path.split('?').next().unwrap_or(&request.path);
    let value = match route {
        "/api/v1/agent/domain-packs" => crate::rust_product_catalog::domain_packs(),
        _ => return None,
    };
    Some(
        serde_json::to_string(&value)
            .map(|data| CompatHttpResponse {
                schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.to_string(),
                status: 200,
                headers: vec![
                    ("Content-Type".to_string(), "application/json".to_string()),
                    ("Cache-Control".to_string(), "no-store".to_string()),
                ],
                body: ProtocolHttpBody::Utf8 { data },
            })
            .map_err(|_| RpcError::internal("Rust product catalog could not be serialized.")),
    )
}

fn rust_owned_product_route_response() -> CompatHttpResponse {
    CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.to_string(),
        status: 410,
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Cache-Control".to_string(), "no-store".to_string()),
        ],
        body: ProtocolHttpBody::Utf8 {
            data: json!({
                "error": {
                    "code": "PRODUCT_STATE_RUST_OWNED",
                    "message": "This product-state route must be implemented by the Rust core and cannot fall back to Python.",
                    "recoverable": false,
                    "details": {}
                }
            })
            .to_string(),
        },
    }
}

fn is_python_sidecar_observation_route(request: &PreparedCompatHttpRequest) -> bool {
    request.method == AllowedHttpMethod::Get
        && request.path == "/api/health"
        && matches!(request.body, ProtocolHttpBody::Empty)
}

fn rust_owned_python_sse_retired(method: &str) -> RpcError {
    RpcError::new(
        METHOD_NOT_FOUND,
        "PYTHON_SSE_RETIRED",
        format!(
            "{method} is retired in the Rust-owned runtime; use native Item reads, notifications, and bounded protocol replay."
        ),
        false,
    )
}

fn native_blockout_request_json<T: DeserializeOwned>(
    request: &PreparedCompatHttpRequest,
) -> Result<T, NativeBlockoutCompatError> {
    let bytes = match &request.body {
        ProtocolHttpBody::Empty => Vec::new(),
        ProtocolHttpBody::Utf8 { data } => data.as_bytes().to_vec(),
        ProtocolHttpBody::Base64 { data } => BASE64.decode(data).map_err(|_| {
            NativeBlockoutCompatError::invalid(
                "REQUEST_BODY_INVALID",
                "Blockout request body is not valid base64.",
            )
        })?,
    };
    if bytes.is_empty() || bytes.len() > MAX_RAW_COMPAT_BODY_BYTES {
        return Err(NativeBlockoutCompatError::invalid(
            "REQUEST_BODY_INVALID",
            "Blockout request body is empty or exceeds the bounded API size.",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        NativeBlockoutCompatError::invalid(
            "REQUEST_BODY_INVALID",
            "Blockout request body does not match the strict API contract.",
        )
    })
}

fn native_blockout_require_idempotency(
    request: &PreparedCompatHttpRequest,
    client_request_id: &str,
) -> Result<(), NativeBlockoutCompatError> {
    let key = request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("idempotency-key"))
        .map(|(_, value)| value.as_str())
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "IDEMPOTENCY_KEY_REQUIRED",
                "Rust blockout mutations require Idempotency-Key.",
            )
        })?;
    if key != client_request_id {
        return Err(NativeBlockoutCompatError::conflict(
            "IDEMPOTENCY_CONFLICT",
            "Idempotency-Key must match client_request_id.",
        ));
    }
    Ok(())
}

fn native_blockout_validate_client_request_id(
    value: &str,
) -> Result<(), NativeBlockoutCompatError> {
    if value.is_empty() || value.len() > 120 || !valid_stable_id(value) {
        return Err(NativeBlockoutCompatError::invalid(
            "CLIENT_REQUEST_ID_INVALID",
            "client_request_id is outside the bounded stable-ID contract.",
        ));
    }
    Ok(())
}

fn native_blockout_validate_direction_id(value: &str) -> Result<(), NativeBlockoutCompatError> {
    if !value.starts_with("direction_") || !valid_stable_id(value) {
        return Err(NativeBlockoutCompatError::invalid(
            "BLOCKOUT_DIRECTION_INVALID",
            "Blockout direction_id must be one stable direction identity.",
        ));
    }
    Ok(())
}

fn native_blockout_validate_artifact_id(value: &str) -> Result<(), NativeBlockoutCompatError> {
    if !value.starts_with("artifact_") || !valid_stable_id(value) {
        return Err(NativeBlockoutCompatError::invalid(
            "BLOCKOUT_ARTIFACT_INVALID",
            "Blockout artifact_id must be one stable artifact identity.",
        ));
    }
    Ok(())
}

fn native_blockout_validate_profile(value: &str) -> Result<(), NativeBlockoutCompatError> {
    if !matches!(value, "quick_sketch" | "showcase") {
        return Err(NativeBlockoutCompatError::invalid(
            "PRESENTATION_PROFILE_INVALID",
            "Blockout presentation_profile must be quick_sketch or showcase.",
        ));
    }
    Ok(())
}

fn native_blockout_plan_facts(
    plan: &Value,
    direction_id: &str,
) -> Result<(String, String, Option<String>), NativeBlockoutCompatError> {
    let plan = plan.as_object().ok_or_else(|| {
        NativeBlockoutCompatError::invalid(
            "CONCEPT_PLAN_INVALID",
            "MechanicalConceptPlan must be a JSON object.",
        )
    })?;
    if plan
        .get("schema_version")
        .and_then(Value::as_str)
        .is_some_and(|version| version != "MechanicalConceptPlan@1")
    {
        return Err(NativeBlockoutCompatError::invalid(
            "CONCEPT_PLAN_INVALID",
            "MechanicalConceptPlan schema version is unsupported.",
        ));
    }
    let plan_id = plan
        .get("plan_id")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("plan_") && valid_stable_id(value))
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "CONCEPT_PLAN_INVALID",
                "MechanicalConceptPlan is missing a stable plan_id.",
            )
        })?;
    let domain_pack_id = plan
        .get("domain_pack_id")
        .and_then(Value::as_str)
        .filter(|value| {
            matches!(
                *value,
                "pack_future_weapon_prop"
                    | "pack_vehicle_concept"
                    | "pack_aircraft_concept"
                    | "pack_robotic_arm_concept"
            )
        })
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "CONCEPT_PLAN_DOMAIN_UNSUPPORTED",
                "MechanicalConceptPlan domain is not enabled.",
            )
        })?;
    let directions = plan
        .get("directions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "CONCEPT_PLAN_INVALID",
                "MechanicalConceptPlan directions are missing.",
            )
        })?;
    if !directions.iter().any(|direction| {
        direction.get("direction_id").and_then(Value::as_str) == Some(direction_id)
    }) {
        return Err(NativeBlockoutCompatError::conflict(
            "BLOCKOUT_DIRECTION_CONFLICT",
            "Requested direction is not present in MechanicalConceptPlan.",
        ));
    }
    let project_id = plan
        .get("spec")
        .and_then(Value::as_object)
        .and_then(|spec| spec.get("project_id"))
        .and_then(Value::as_str)
        .filter(|value| *value != "prj_unbound_agent_session")
        .map(str::to_string);
    if project_id
        .as_deref()
        .is_some_and(|value| value.len() > 160 || !valid_stable_id(value))
    {
        return Err(NativeBlockoutCompatError::invalid(
            "PROJECT_ID_INVALID",
            "MechanicalConceptPlan project identity is invalid.",
        ));
    }
    Ok((plan_id.to_string(), domain_pack_id.to_string(), project_id))
}

fn native_blockout_variant_id(
    requested: Option<&str>,
    domain_pack_id: &str,
    direction_id: &str,
    variation_index: u8,
) -> Result<String, NativeBlockoutCompatError> {
    if let Some(requested) = requested {
        if requested.len() < 2
            || requested.len() > 120
            || !requested.as_bytes()[0].is_ascii_lowercase()
            || !valid_stable_id(requested)
        {
            return Err(NativeBlockoutCompatError::invalid(
                "BLOCKOUT_VARIANT_INVALID",
                "Blockout variant_id is outside the bounded stable-ID contract.",
            ));
        }
        return Ok(requested.to_string());
    }
    let hash = sha256_hex(
        format!("{domain_pack_id}:{direction_id}:variation:{variation_index}").as_bytes(),
    );
    Ok(format!("variant_rust_{}", &hash[..24]))
}

fn native_blockout_semantic_sha256(value: &Value) -> Result<String, NativeBlockoutCompatError> {
    semantic_sha256(value).map_err(native_blockout_core_error)
}

fn native_blockout_build_payload(
    candidate: &NativeBlockoutCompatCandidate,
    preview: &NativePreviewArtifact,
) -> Result<Value, NativeBlockoutCompatError> {
    preview
        .validate()
        .map_err(native_blockout_product_tool_error)?;
    let (_, assembly_graph, _) =
        native_blockout_parts_and_graph(preview, candidate.project_id.as_deref())?;
    Ok(json!({
        "artifact_id": candidate.artifact_id,
        "plan_id": candidate.plan_id,
        "direction_id": candidate.direction_id,
        "variant_id": candidate.variant_id,
        "variation_index": candidate.variation_index,
        "presentation_profile": candidate.presentation_profile,
        "domain_pack_id": candidate.domain_pack_id,
        "triangle_count": preview.readback.triangle_count,
        "bounds_mm": preview.readback.bounds_mm,
        "topology_hash": preview.readback.shape_program_sha256,
        "assembly_graph": assembly_graph,
        "shape_program": preview.shape_program,
        "glb_base64": BASE64.encode(&preview.glb_bytes)
    }))
}

fn native_blockout_segment_payload(
    candidate: &NativeBlockoutCompatCandidate,
    preview: &NativePreviewArtifact,
) -> Result<Value, NativeBlockoutCompatError> {
    preview
        .validate()
        .map_err(native_blockout_product_tool_error)?;
    let (parts, assembly_graph, _) =
        native_blockout_parts_and_graph(preview, candidate.project_id.as_deref())?;
    Ok(json!({
        "artifact_id": candidate.artifact_id,
        "plan_id": candidate.plan_id,
        "direction_id": candidate.direction_id,
        "variant_id": candidate.variant_id,
        "variation_index": candidate.variation_index,
        "presentation_profile": candidate.presentation_profile,
        "domain_pack_id": candidate.domain_pack_id,
        "segmentation_status": "candidate",
        "parts": parts,
        "assembly_graph": assembly_graph
    }))
}

fn native_blockout_parts_and_graph(
    preview: &NativePreviewArtifact,
    project_id: Option<&str>,
) -> Result<(Vec<Value>, Value, BTreeMap<String, Value>), NativeBlockoutCompatError> {
    if let Some(recipe_graph) = preview.recipe_assembly_graph.as_ref() {
        return native_recipe_blockout_parts_and_graph(
            preview,
            recipe_graph,
            preview.recipe_component_instances.as_ref().ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "RECIPE_PREVIEW_EVIDENCE_INCOMPLETE",
                    "Recipe preview graph is missing component instance provenance.",
                )
            })?,
            preview.recipe_candidate_sha256.as_deref().ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "RECIPE_PREVIEW_EVIDENCE_INCOMPLETE",
                    "Recipe preview graph is missing its candidate identity.",
                )
            })?,
            project_id,
        );
    }
    let operations = preview
        .shape_program
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "SHAPE_PROGRAM_INVALID",
                "Rust preview ShapeProgram has no operations.",
            )
        })?;
    let operation_args = operations
        .iter()
        .filter_map(|operation| {
            Some((
                operation.get("operation_id")?.as_str()?.to_string(),
                operation.get("args")?.as_object()?,
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let operation_kinds = operations
        .iter()
        .filter_map(|operation| {
            Some((
                operation.get("operation_id")?.as_str()?.to_string(),
                operation.get("op")?.as_str()?.to_string(),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let role_counts =
        preview
            .assembly
            .parts
            .iter()
            .fold(BTreeMap::<&str, usize>::new(), |mut counts, part| {
                *counts.entry(part.part_role.as_str()).or_default() += 1;
                counts
            });
    let mut parts = Vec::with_capacity(preview.assembly.parts.len());
    let mut graph_parts = Vec::with_capacity(preview.assembly.parts.len());
    let mut material_bindings = BTreeMap::new();
    let mut connections = Vec::new();
    for part in &preview.assembly.parts {
        let args = operation_args.get(&part.operation_id).ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                "Preview part does not reference one ShapeProgram operation.",
            )
        })?;
        let zone_id = part.material_zone_id.as_deref().ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "MATERIAL_ZONE_REQUIRED",
                "Every Rust blockout part requires a ShapeProgram Material Zone.",
            )
        })?;
        let material_id = part.material_id.as_deref().ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "MATERIAL_BINDING_REQUIRED",
                "Every Rust blockout Material Zone requires a visual material binding.",
            )
        })?;
        let position = native_blockout_vec3(args.get("position"), [0.0, 0.0, 0.0], false)?;
        let size = native_blockout_operation_size(args, preview.readback.bounds_mm)?;
        let parent_part_id = (part.part_id != preview.assembly.root_part_id)
            .then(|| preview.assembly.root_part_id.clone());
        let operation_kind = operation_kinds
            .get(&part.operation_id)
            .map(String::as_str)
            .ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                    "Preview part operation kind is missing from ShapeProgram.",
                )
            })?;
        let editable_parameter_bindings = if !part.part_role.starts_with("visual_")
            && role_counts.get(part.part_role.as_str()) == Some(&1)
            && matches!(operation_kind, "box" | "wedge")
        {
            [
                ("x", "长度比例"),
                ("y", "高度比例"),
                ("z", "宽度比例"),
            ]
            .into_iter()
            .map(|(axis, display_name)| {
                json!({
                    "schema_version": "EditableParameterBinding@1",
                    "parameter_id": format!("editparam_{}_scale_{axis}", &sha256_hex(part.part_id.as_bytes())[..12]),
                    "path": format!("transform.scale.{axis}"),
                    "display_name": display_name,
                    "unit": "ratio",
                    "default": 1.0,
                    "min": 0.6,
                    "max": 1.4,
                    "step": 0.1
                })
            })
            .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let editable_parameters = editable_parameter_bindings
            .iter()
            .filter_map(|binding| binding.get("path").cloned())
            .collect::<Vec<_>>();
        parts.push(json!({
            "part_id": part.part_id,
            "role": part.part_role,
            "parent_part_id": parent_part_id,
            "position_mm": position,
            "size_mm": size,
            "material_zone_ids": [zone_id],
            "editable_parameters": editable_parameters,
            "editable_parameter_bindings": editable_parameter_bindings,
            "locked": false,
            "provenance": "agent_generated"
        }));
        graph_parts.push(json!({
            "part_id": part.part_id,
            "role": part.part_role,
            "parent_part_id": parent_part_id,
            "geometry_source": "shape_program",
            "output_id": part.output_id,
            "operation_id": part.operation_id,
            "transform": {
                "position": position,
                "rotation": [0.0, 0.0, 0.0],
                "scale": [1.0, 1.0, 1.0]
            },
            "connectors": [],
            "joints": [],
            "material_zones": [zone_id],
            "material_zone_ids": [zone_id],
            "editable_parameters": editable_parameters,
            "locked": false,
            "provenance": "agent_generated"
        }));
        material_bindings.insert(
            format!("{}:{zone_id}", part.part_id),
            Value::String(material_id.to_string()),
        );
        if part.part_id != preview.assembly.root_part_id {
            connections.push(json!({
                "connection_id": format!("connection_{}", &sha256_hex(format!("{}:{}", preview.assembly.root_part_id, part.part_id).as_bytes())[..24]),
                "from_part_id": preview.assembly.root_part_id,
                "to_part_id": part.part_id,
                "kind": "visual_attachment",
                "non_functional_only": true
            }));
        }
    }
    if parts.is_empty()
        || !parts.iter().any(|part| {
            part.get("part_id").and_then(Value::as_str)
                == Some(preview.assembly.root_part_id.as_str())
        })
    {
        return Err(NativeBlockoutCompatError::invalid(
            "NATIVE_PREVIEW_ASSEMBLY_INVALID",
            "Rust blockout assembly has no valid root part.",
        ));
    }
    let mut graph = json!({
        "schema_version": "AssemblyGraph@1",
        "graph_id": preview.assembly.assembly_id,
        "root_part_id": preview.assembly.root_part_id,
        "parts": graph_parts,
        "connections": connections,
        "joints": [],
        "non_functional_only": true
    });
    if let (Some(project_id), Some(graph)) = (project_id, graph.as_object_mut()) {
        graph.insert("project_id".into(), Value::String(project_id.to_string()));
    }
    Ok((parts, graph, material_bindings))
}

/// Convert the C105 carrier without rebuilding its AssemblyGraph.  The core
/// recipe expansion owns graph identities, parentage, connectors, bindings and
/// provenance; this compatibility boundary only adds the persisted Part facts
/// required by the Rust product repository.
fn native_recipe_blockout_parts_and_graph(
    preview: &NativePreviewArtifact,
    recipe_graph: &Value,
    recipe_instances: &Value,
    candidate_sha256: &str,
    project_id: Option<&str>,
) -> Result<(Vec<Value>, Value, BTreeMap<String, Value>), NativeBlockoutCompatError> {
    if candidate_sha256.len() != 64
        || !candidate_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(NativeBlockoutCompatError::invalid(
            "RECIPE_PREVIEW_CANDIDATE_INVALID",
            "Recipe preview candidate identity must be a lowercase SHA-256 digest.",
        ));
    }
    let graph = recipe_graph.as_object().ok_or_else(|| {
        NativeBlockoutCompatError::invalid(
            "RECIPE_PREVIEW_GRAPH_INVALID",
            "Recipe preview AssemblyGraph must be an object.",
        )
    })?;
    if graph.get("schema_version").and_then(Value::as_str) != Some("AssemblyGraph@1")
        || graph.get("graph_id").and_then(Value::as_str).is_none()
        || graph.get("root_part_id").and_then(Value::as_str).is_none()
    {
        return Err(NativeBlockoutCompatError::invalid(
            "RECIPE_PREVIEW_GRAPH_INVALID",
            "Recipe preview AssemblyGraph identity is incomplete.",
        ));
    }
    if graph.get("component_recipe_instances") != Some(recipe_instances) {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_PREVIEW_PROVENANCE_MISMATCH",
            "Recipe preview graph provenance does not match its carrier instances.",
        ));
    }
    let output_contract = recipe_preview_output_contract(recipe_instances)
        .map_err(native_blockout_product_tool_error)?;
    let c106_arm_semantic_components =
        output_contract == RecipePreviewOutputContract::C106ArmSemanticComponents;
    let _instance_domains = recipe_instances
        .as_array()
        .and_then(|instances| {
            instances
                .iter()
                .map(|instance| instance.get("domain_pack_id").and_then(Value::as_str))
                .collect::<Option<BTreeSet<_>>>()
        })
        .filter(|domains| domains.len() == 1)
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "RECIPE_PREVIEW_PROVENANCE_MISMATCH",
                "Recipe preview instance provenance must bind exactly one Domain Pack.",
            )
        })?;
    let operations = preview
        .shape_program
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "SHAPE_PROGRAM_INVALID",
                "Rust preview ShapeProgram has no operations.",
            )
        })?;
    let operation_args = operations
        .iter()
        .filter_map(|operation| {
            Some((
                operation.get("operation_id")?.as_str()?.to_string(),
                operation.get("args")?.as_object()?,
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let preview_parts = preview
        .assembly
        .parts
        .iter()
        .map(|part| ((part.operation_id.as_str(), part.output_id.as_str()), part))
        .collect::<BTreeMap<_, _>>();
    let graph_parts = graph
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe preview AssemblyGraph has no parts.",
            )
        })?;
    if graph_parts.is_empty()
        || (!c106_arm_semantic_components && graph_parts.len() != preview_parts.len())
    {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_PREVIEW_GRAPH_INVALID",
            "Recipe preview graph parts do not match the rendered assembly.",
        ));
    }

    let mut parts = Vec::with_capacity(graph_parts.len());
    let mut material_bindings = BTreeMap::new();
    let mut seen_bindings = BTreeSet::new();
    let mut semantic_component_outputs = Vec::new();
    for graph_part in graph_parts {
        let graph_part = graph_part.as_object().ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe preview AssemblyGraph contains a non-object part.",
            )
        })?;
        let part_id = graph_part
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "RECIPE_PREVIEW_GRAPH_INVALID",
                    "Recipe graph part is missing part_id.",
                )
            })?;
        let operation_id = graph_part
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "RECIPE_PREVIEW_GRAPH_INVALID",
                    "Recipe graph part is missing operation_id.",
                )
            })?;
        let output_id = graph_part
            .get("output_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "RECIPE_PREVIEW_GRAPH_INVALID",
                    "Recipe graph part is missing output_id.",
                )
            })?;
        let preview_part = preview_parts
            .get(&(operation_id, output_id))
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "RECIPE_PREVIEW_GRAPH_INVALID",
                    "Recipe graph operation/output binding is not rendered by the preview.",
                )
            })?;
        let graph_role = graph_part
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "RECIPE_PREVIEW_GRAPH_INVALID",
                    "Recipe graph part is missing its semantic role.",
                )
            })?;
        let shape_program_role = recipe_preview_shape_program_role(output_contract, graph_role);
        if !seen_bindings.insert((operation_id, output_id))
            || shape_program_role != preview_part.part_role
        {
            return Err(NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe graph role or operation/output binding is ambiguous.",
            ));
        }
        let args = operation_args.get(operation_id).ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe graph references a missing ShapeProgram operation.",
            )
        })?;
        let zones = graph_part
            .get("material_zone_ids")
            .and_then(Value::as_array)
            .filter(|zones| !zones.is_empty())
            .ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "MATERIAL_ZONE_REQUIRED",
                    "Recipe graph part has no material zones.",
                )
            })?;
        if graph_part.get("material_zones") != Some(&Value::Array(zones.clone())) {
            return Err(NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe graph material zone aliases disagree.",
            ));
        }
        let zone_values = zones
            .iter()
            .map(|zone| zone.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "MATERIAL_ZONE_REQUIRED",
                    "Recipe graph material zone is invalid.",
                )
            })?;
        if preview_part
            .material_zone_id
            .as_deref()
            .is_none_or(|zone| !zone_values.iter().any(|value| value == zone))
        {
            return Err(NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe graph zones do not include the rendered output zone.",
            ));
        }
        let position = graph_part
            .get("transform")
            .and_then(Value::as_object)
            .and_then(|transform| transform.get("position"))
            .map(|value| native_blockout_vec3(Some(value), [0.0, 0.0, 0.0], false))
            .transpose()?
            .unwrap_or(native_blockout_vec3(
                args.get("position"),
                [0.0, 0.0, 0.0],
                false,
            )?);
        let size = native_blockout_operation_size(args, preview.readback.bounds_mm)?;
        let material_id = args
            .get("material_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "MATERIAL_BINDING_REQUIRED",
                    "Recipe ShapeProgram operation has no visual material binding.",
                )
            })?;
        if c106_arm_semantic_components {
            let recipe_instance_id = graph_part
                .get("recipe_instance_id")
                .and_then(Value::as_str)
                .and_then(|value| value.strip_prefix("recipeinst_"))
                .ok_or_else(|| {
                    NativeBlockoutCompatError::invalid(
                        "RECIPE_PREVIEW_GRAPH_INVALID",
                        "C106 Recipe graph part is missing its stable instance identity.",
                    )
                })?;
            let operation_prefix = format!("op_{recipe_instance_id}_");
            for zone_id in &zone_values {
                let (_, output) = preview_parts
                    .iter()
                    .find(|((candidate_operation_id, _), candidate)| {
                        candidate_operation_id.starts_with(&operation_prefix)
                            && candidate.part_role == shape_program_role
                            && candidate.material_zone_id.as_deref() == Some(zone_id.as_str())
                    })
                    .ok_or_else(|| {
                        NativeBlockoutCompatError::conflict(
                            "RECIPE_PREVIEW_GRAPH_INVALID",
                            "C106 semantic component has no rendered output for one declared Material Zone.",
                        )
                    })?;
                let output_args = operation_args.get(&output.operation_id).ok_or_else(|| {
                    NativeBlockoutCompatError::conflict(
                        "RECIPE_PREVIEW_GRAPH_INVALID",
                        "C106 rendered output references no ShapeProgram operation.",
                    )
                })?;
                let output_material_id = output_args
                    .get("material_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        NativeBlockoutCompatError::invalid(
                            "MATERIAL_BINDING_REQUIRED",
                            "C106 rendered output has no visual material binding.",
                        )
                    })?;
                material_bindings.insert(
                    format!("{part_id}:{zone_id}"),
                    Value::String(output_material_id.to_string()),
                );
            }
            semantic_component_outputs.push((
                operation_prefix,
                shape_program_role.to_string(),
                zone_values.clone(),
            ));
        } else {
            for zone_id in &zone_values {
                material_bindings.insert(
                    format!("{part_id}:{zone_id}"),
                    Value::String(material_id.to_string()),
                );
            }
        }
        parts.push(json!({
            "part_id": part_id,
            "role": graph_role,
            "parent_part_id": graph_part.get("parent_part_id").cloned().unwrap_or(Value::Null),
            "position_mm": position,
            "size_mm": size,
            "material_zone_ids": zone_values,
            "editable_parameters": graph_part.get("editable_parameters").cloned().unwrap_or_else(|| json!([])),
            "editable_parameter_bindings": graph_part.get("editable_parameter_bindings").cloned().unwrap_or_else(|| json!([])),
            "locked": graph_part.get("locked").cloned().unwrap_or(Value::Bool(false)),
            "provenance": graph_part.get("provenance").cloned().unwrap_or_else(|| Value::String("agent_generated".into()))
        }));
    }
    let every_preview_output_has_c106_component = !c106_arm_semantic_components
        || preview_parts
            .iter()
            .all(|((operation_id, _), preview_part)| {
                semantic_component_outputs
                    .iter()
                    .any(|(operation_prefix, shape_role, zones)| {
                        operation_id.starts_with(operation_prefix)
                            && preview_part.part_role == *shape_role
                            && preview_part
                                .material_zone_id
                                .as_deref()
                                .is_some_and(|zone| zones.iter().any(|value| value == zone))
                    })
            });
    if (!c106_arm_semantic_components && seen_bindings.len() != preview_parts.len())
        || !every_preview_output_has_c106_component
    {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_PREVIEW_GRAPH_INVALID",
            "Recipe graph does not preserve every rendered operation/output binding.",
        ));
    }
    // The C105 graph is already schema-validated by the native preview
    // carrier.  Do not inject project/candidate fields here: Project ownership
    // lives on AgentAssetVersion and candidate identity lives in the candidate
    // metadata/Part facts, while the graph must remain byte-for-byte faithful
    // to the reviewed Recipe expansion.
    let _ = project_id;
    Ok((parts, recipe_graph.clone(), material_bindings))
}

fn native_blockout_vec3(
    value: Option<&Value>,
    fallback: [f64; 3],
    positive: bool,
) -> Result<[f64; 3], NativeBlockoutCompatError> {
    let result = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .map(|values| {
            [
                values[0].as_f64().unwrap_or(fallback[0]),
                values[1].as_f64().unwrap_or(fallback[1]),
                values[2].as_f64().unwrap_or(fallback[2]),
            ]
        })
        .unwrap_or(fallback);
    if result
        .iter()
        .any(|value| !value.is_finite() || (positive && *value <= 0.0))
    {
        return Err(NativeBlockoutCompatError::invalid(
            "SHAPE_PROGRAM_BOUNDS_INVALID",
            "ShapeProgram part transform or visual bounds are invalid.",
        ));
    }
    Ok(result)
}

fn native_blockout_operation_size(
    args: &serde_json::Map<String, Value>,
    fallback: [f64; 3],
) -> Result<[f64; 3], NativeBlockoutCompatError> {
    if args.get("size").is_some() {
        return native_blockout_vec3(args.get("size"), fallback, true);
    }
    if let (Some(radius), Some(height)) = (
        args.get("radius").and_then(Value::as_f64),
        args.get("height").and_then(Value::as_f64),
    ) {
        return native_blockout_vec3(
            Some(&json!([radius * 2.0, height, radius * 2.0])),
            fallback,
            true,
        );
    }
    native_blockout_vec3(None, fallback, true)
}

fn native_verified_geometry_readback(
    bytes: &[u8],
    executor: &RestrictedGeometryReadback,
    expected_profile_id: &str,
    shape_program: &Value,
) -> Result<ForgeCadGlbReadback, NativeBlockoutCompatError> {
    let verified = verify_forgecad_glb(bytes, Some(expected_profile_id))
        .map_err(native_blockout_core_error)?;
    let shape_program_sha256 = native_blockout_semantic_sha256(shape_program)?;
    let bounds_match = verified.bounds_mm.len() == 3
        && verified
            .bounds_mm
            .iter()
            .zip(executor.bounds_mm)
            .all(|(verified, executor)| (verified - executor).abs() <= 0.01);
    if executor.artifact_profile_id != expected_profile_id
        || executor.runtime_manifest_version != verified.runtime_manifest_version
        || executor.shape_program_sha256 != shape_program_sha256
        || executor.glb_sha256 != verified.glb_sha256
        || executor.glb_byte_size != verified.glb_byte_size
        || u64::from(executor.triangle_count) != verified.triangle_count
        || u64::from(executor.mesh_count) != verified.mesh_count
        || u64::from(executor.primitive_count) != verified.primitive_count
        || u64::from(executor.material_count) != verified.material_count
        || executor.closed_manifold != verified.closed_manifold
        || executor.surface_provenance_present != verified.surface_provenance_present
        || !bounds_match
    {
        return Err(NativeBlockoutCompatError::conflict(
            "RESTRICTED_GEOMETRY_READBACK_MISMATCH",
            "Restricted geometry readback does not match canonical GLB readback.",
        ));
    }
    Ok(verified)
}

fn native_geometry_quality_report(
    project_id: &str,
    asset_version_id: &str,
    executor_readback: &RestrictedGeometryReadback,
    renderer_id: &str,
    view_sha256: &BTreeMap<String, String>,
    verified: &ForgeCadGlbReadback,
    timestamp: &str,
) -> Result<QualityReport, NativeBlockoutCompatError> {
    if verified.artifact_profile_id != "production_concept"
        || !verified.closed_manifold
        || !verified.surface_provenance_present
        || verified.triangle_count == 0
    {
        return Err(NativeBlockoutCompatError::conflict(
            "PRODUCTION_READBACK_REQUIRED",
            "Committed blockout requires passed production GLB readback.",
        ));
    }
    let quality_report_id = native_blockout_quality_report_id(asset_version_id);
    Ok(QualityReport {
        quality_report_id: quality_report_id.clone(),
        project_id: project_id.to_string(),
        asset_version_id: asset_version_id.to_string(),
        report: json!({
            "schema_version": "AgentAssetQualityReport@1",
            "quality_report_id": quality_report_id,
            "asset_version_id": asset_version_id,
            "status": "passed",
            "evidence_source": "geometry_compile_readback",
            "triangle_count": verified.triangle_count,
            "bounds_mm": verified.bounds_mm,
            "compile_readback": {
                "schema_version": "GeometryCompileReadback@2",
                "runtime_manifest_version": verified.runtime_manifest_version,
                "artifact_profile": verified.artifact_profile,
                "shape_program_sha256": executor_readback.shape_program_sha256,
                "glb_sha256": verified.glb_sha256,
                "glb_byte_size": verified.glb_byte_size,
                "triangle_count": verified.triangle_count,
                "bounds_mm": verified.bounds_mm,
                "mesh_count": verified.mesh_count,
                "primitive_count": verified.primitive_count,
                "material_count": verified.material_count,
                "uv0_primitive_count": verified.uv0_primitive_count,
                "normal_primitive_count": verified.normal_primitive_count,
                "tangent_primitive_count": verified.tangent_primitive_count,
                "closed_manifold": verified.closed_manifold,
                "surface_provenance_present": verified.surface_provenance_present,
                "visual_texture_set_count": verified.visual_texture_set_count,
                "visual_texture_map_count": verified.visual_texture_map_count
            },
            "render_readback": {
                "renderer_id": renderer_id,
                "view_sha256": view_sha256
            }
        }),
        status: QualityStatus::Passed,
        created_at: timestamp.to_string(),
    })
}

fn native_blockout_quality_report_id(asset_version_id: &str) -> String {
    format!(
        "quality_{}",
        &sha256_hex(format!("{asset_version_id}:production_readback").as_bytes())[..24]
    )
}

fn native_blockout_asset_version_payload(
    version: &AgentAssetVersion,
) -> Result<Value, NativeBlockoutCompatError> {
    let mut payload = serde_json::to_value(version)
        .map_err(|_| {
            NativeBlockoutCompatError::unavailable(
                "ASSET_VERSION_RESPONSE_INVALID",
                "Rust asset version could not be serialized.",
            )
        })?
        .as_object()
        .cloned()
        .ok_or_else(|| {
            NativeBlockoutCompatError::unavailable(
                "ASSET_VERSION_RESPONSE_INVALID",
                "Rust asset version response is not an object.",
            )
        })?;
    payload.insert(
        "schema_version".into(),
        Value::String("AgentAssetVersion@1".into()),
    );
    Ok(Value::Object(payload))
}

fn native_change_set_payload(
    change_set: &AgentAssetChangeSet,
) -> Result<Value, NativeBlockoutCompatError> {
    let mut payload = serde_json::to_value(change_set)
        .map_err(|_| {
            NativeBlockoutCompatError::unavailable(
                "CHANGE_SET_RESPONSE_INVALID",
                "Rust ChangeSet could not be serialized.",
            )
        })?
        .as_object()
        .cloned()
        .ok_or_else(|| {
            NativeBlockoutCompatError::unavailable(
                "CHANGE_SET_RESPONSE_INVALID",
                "Rust ChangeSet response is not an object.",
            )
        })?;
    payload.insert(
        "schema_version".into(),
        Value::String("AgentAssetChangeSet@1".into()),
    );
    Ok(Value::Object(payload))
}

fn native_change_set_confirm_payload(
    change_set: &AgentAssetChangeSet,
    version: &AgentAssetVersion,
) -> Result<Value, NativeBlockoutCompatError> {
    Ok(json!({
        "change_set": native_change_set_payload(change_set)?,
        "asset_version": native_blockout_asset_version_payload(version)?
    }))
}

fn native_change_set_glb_response(
    change_set: &AgentAssetChangeSet,
    sealed_preview: &AgentAssetVersion,
    verified: &ForgeCadGlbReadback,
    bytes: &[u8],
) -> Result<CompatHttpResponse, NativeBlockoutCompatError> {
    let shape_program_sha256 = native_blockout_semantic_sha256(&sealed_preview.shape_program)?;
    Ok(CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
        status: 200,
        headers: vec![
            ("Content-Type".into(), "model/gltf-binary".into()),
            ("Cache-Control".into(), "no-store".into()),
            (
                "Content-Disposition".into(),
                format!(
                    "inline; filename=\"{}-preview.glb\"",
                    change_set.change_set_id
                ),
            ),
            ("ETag".into(), format!("\"sha256:{}\"", verified.glb_sha256)),
            (
                "X-ForgeCAD-Artifact-Profile".into(),
                verified.artifact_profile_id.clone(),
            ),
            (
                "X-ForgeCAD-Artifact-Profile-SHA256".into(),
                verified.artifact_profile_sha256.clone(),
            ),
            (
                "X-ForgeCAD-Shape-Program-SHA256".into(),
                shape_program_sha256,
            ),
            ("X-ForgeCAD-GLB-SHA256".into(), verified.glb_sha256.clone()),
            (
                "X-ForgeCAD-GLB-Byte-Size".into(),
                verified.glb_byte_size.to_string(),
            ),
            (
                "X-ForgeCAD-Triangle-Count".into(),
                verified.triangle_count.to_string(),
            ),
            (
                "X-ForgeCAD-Preview-GLB-SHA256".into(),
                verified.glb_sha256.clone(),
            ),
            (
                "X-ForgeCAD-Base-Asset-Version-ID".into(),
                change_set.base_asset_version_id.clone(),
            ),
            (
                "X-ForgeCAD-Preview-Triangle-Count".into(),
                verified.triangle_count.to_string(),
            ),
        ],
        body: ProtocolHttpBody::Base64 {
            data: BASE64.encode(bytes),
        },
    })
}

fn native_change_set_require_empty_body(
    request: &PreparedCompatHttpRequest,
) -> Result<(), NativeBlockoutCompatError> {
    let empty = match &request.body {
        ProtocolHttpBody::Empty => true,
        ProtocolHttpBody::Utf8 { data } => data.trim().is_empty(),
        ProtocolHttpBody::Base64 { data } => data.is_empty(),
    };
    if !empty {
        return Err(NativeBlockoutCompatError::invalid(
            "CHANGE_SET_BODY_NOT_ALLOWED",
            "ChangeSet preview and confirm requests do not accept a body.",
        ));
    }
    Ok(())
}

fn native_change_set_idempotency_key(
    request: &PreparedCompatHttpRequest,
) -> Result<&str, NativeBlockoutCompatError> {
    let value = request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("idempotency-key"))
        .map(|(_, value)| value.as_str())
        .filter(|value| !value.is_empty() && value.len() <= 256 && valid_stable_id(value))
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "IDEMPOTENCY_KEY_REQUIRED",
                "A bounded stable Idempotency-Key is required.",
            )
        })?;
    Ok(value)
}

fn native_change_set_version_id(change_set_id: &str, purpose: &str) -> String {
    format!(
        "assetver_{}",
        &sha256_hex(format!("{change_set_id}:{purpose}").as_bytes())[..24]
    )
}

fn native_surface_adornment_programs(
    version: &AgentAssetVersion,
) -> Result<Vec<SurfaceAdornmentProgram>, NativeBlockoutCompatError> {
    let Some(raw) = version
        .assembly_graph
        .get("surface_adornments")
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };
    if raw.len() > 8 {
        return Err(NativeBlockoutCompatError::conflict(
            "SURFACE_ADORNMENT_LIMIT_EXCEEDED",
            "Asset version contains more visual adornments than the reviewed compiler boundary.",
        ));
    }
    let mut programs = Vec::with_capacity(raw.len());
    let mut targets = BTreeSet::new();
    for value in raw {
        let program: SurfaceAdornmentProgram =
            serde_json::from_value(value.clone()).map_err(|_| {
                NativeBlockoutCompatError::conflict(
                    "SURFACE_ADORNMENT_PROVENANCE_INVALID",
                    "Asset version contains malformed visual adornment provenance.",
                )
            })?;
        program.validate().map_err(native_blockout_core_error)?;
        if !targets.insert((
            program.target_part_id.clone(),
            program.target_zone_id.clone(),
        )) {
            return Err(NativeBlockoutCompatError::conflict(
                "SURFACE_ADORNMENT_TARGET_DUPLICATE",
                "An asset version may contain only one active adornment per Part and Material Zone.",
            ));
        }
        programs.push(program);
    }
    Ok(programs)
}

fn native_change_set_apply(
    repository: &forgecad_core::CoreRepository,
    base: &AgentAssetVersion,
    change_set: &AgentAssetChangeSet,
) -> Result<AgentAssetVersion, NativeBlockoutCompatError> {
    if change_set.status != ChangeSetStatus::Proposed
        && change_set.status != ChangeSetStatus::Previewed
    {
        return Err(NativeBlockoutCompatError::conflict(
            "CHANGE_SET_STATE_CONFLICT",
            "Only proposed or exactly replayed previewed ChangeSets may be compiled.",
        ));
    }
    if change_set.project_id != base.project_id
        || change_set.base_asset_version_id != base.asset_version_id
    {
        return Err(NativeBlockoutCompatError::conflict(
            "CHANGE_SET_BASE_STALE",
            "ChangeSet does not target the current immutable base asset.",
        ));
    }
    let mut preview = base.clone();
    preview.asset_version_id = native_change_set_version_id(&change_set.change_set_id, "preview");
    preview.parent_asset_version_id = Some(base.asset_version_id.clone());
    preview.version_no = base.version_no.checked_add(1).ok_or_else(|| {
        NativeBlockoutCompatError::conflict(
            "ASSET_VERSION_NUMBER_INVALID",
            "Preview version number exceeds the bounded immutable chain.",
        )
    })?;
    preview.status = AssetVersionStatus::Committed;
    preview.summary = change_set.summary.clone();
    preview.stage = AssetStage::EditableAsset;
    preview.created_at = change_set.created_at.clone();

    // C110C is the first real continuation path after generation.  Keep the
    // bounded AssemblyDeltaProgram separate from the older one-part ChangeSet
    // operations, then run the same compiler/readback/preview/confirm flow
    // against its materialized ShapeProgram.  Mixed operations are rejected
    // rather than partially applying a user's second turn.
    let has_assembly_delta_operation = change_set.operations.iter().any(|operation| {
        matches!(
            operation.get("op").and_then(Value::as_str),
            Some(
                "add_reviewed_recipe"
                    | "replace_reviewed_recipe"
                    | "set_part_transform"
                    | "set_joint_pose"
                    | "snap_part_to_connector"
            )
        )
    });
    if has_assembly_delta_operation {
        let delta = native_change_set_assembly_delta(base, change_set)?;
        let mut materialized =
            materialize_assembly_delta(base, &delta).map_err(native_blockout_core_error)?;
        materialized.asset_version_id = preview.asset_version_id.clone();
        materialized.parent_asset_version_id = preview.parent_asset_version_id.clone();
        materialized.version_no = preview.version_no;
        materialized.status = preview.status;
        materialized.summary = preview.summary.clone();
        materialized.stage = preview.stage;
        materialized.created_at = preview.created_at.clone();
        if materialized.parts == base.parts
            && materialized.shape_program == base.shape_program
            && materialized.assembly_graph == base.assembly_graph
        {
            return Err(NativeBlockoutCompatError::conflict(
                "CHANGE_SET_NO_OP",
                "AssemblyDelta operations did not alter the sealed geometry or assembly graph.",
            ));
        }
        materialized
            .validate()
            .map_err(native_blockout_core_error)?;
        return Ok(materialized);
    }

    for operation in &change_set.operations {
        native_apply_change_set_operation(repository, base, change_set, &mut preview, operation)?;
    }
    if preview.parts == base.parts
        && preview.shape_program == base.shape_program
        && preview.assembly_graph == base.assembly_graph
        && preview.material_bindings == base.material_bindings
    {
        return Err(NativeBlockoutCompatError::conflict(
            "CHANGE_SET_NO_OP",
            "ChangeSet operations did not alter the sealed geometry or visual material state.",
        ));
    }
    preview.validate().map_err(native_blockout_core_error)?;
    Ok(preview)
}

/// Convert the lowered C110C operation vocabulary stored in a ChangeSet back
/// to the strict AssemblyDeltaProgram boundary.  The client never supplies
/// project/base/domain identity here; Rust derives it from the immutable base.
fn native_change_set_assembly_delta(
    base: &AgentAssetVersion,
    change_set: &AgentAssetChangeSet,
) -> Result<Value, NativeBlockoutCompatError> {
    let mut operations = Vec::with_capacity(change_set.operations.len());
    for operation in &change_set.operations {
        let object = operation.as_object().ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "ASSEMBLY_DELTA_OPERATION_INVALID",
                "AssemblyDelta ChangeSet operations must be JSON objects.",
            )
        })?;
        let op = object.get("op").and_then(Value::as_str).ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "ASSEMBLY_DELTA_OPERATION_INVALID",
                "AssemblyDelta operation is missing op.",
            )
        })?;
        if !matches!(
            op,
            "add_reviewed_recipe"
                | "replace_reviewed_recipe"
                | "set_part_transform"
                | "set_joint_pose"
                | "snap_part_to_connector"
        ) {
            return Err(NativeBlockoutCompatError::conflict(
                "ASSEMBLY_DELTA_MIXED_OPERATIONS",
                "An AssemblyDelta ChangeSet may contain only reviewed visual assembly operations.",
            ));
        }
        let required = |key: &str| {
            object.get(key).cloned().ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "ASSEMBLY_DELTA_OPERATION_INVALID",
                    format!("AssemblyDelta operation is missing {key}."),
                )
            })
        };
        let lowered = match op {
            "add_reviewed_recipe" => json!({
                "op": op,
                "operation_id": required("operation_id")?,
                "new_part_id": required("new_part_id")?,
                "parent_part_id": required("part_id")?,
                "parent_connector_id": required("parent_connector_id")?,
                "child_connector_id": required("child_connector_id")?,
                "recipe_id": required("recipe_id")?,
                "slot_id": required("slot_id")?,
                "transform": required("transform")?,
            }),
            "replace_reviewed_recipe" => json!({
                "op": op,
                "operation_id": required("operation_id")?,
                "part_id": required("part_id")?,
                "recipe_id": required("recipe_id")?,
            }),
            "set_part_transform" => json!({
                "op": op,
                "operation_id": required("operation_id")?,
                "part_id": required("part_id")?,
                "transform": required("transform")?,
            }),
            "set_joint_pose" => json!({
                "op": op,
                "operation_id": required("operation_id")?,
                "part_id": required("part_id")?,
                "joint_id": required("joint_id")?,
                "pose": required("pose")?,
            }),
            "snap_part_to_connector" => json!({
                "op": op,
                "operation_id": required("operation_id")?,
                "part_id": required("part_id")?,
                "target_part_id": required("target_part_id")?,
                "target_connector_id": required("target_connector_id")?,
                "connector_id": required("connector_id")?,
            }),
            _ => unreachable!("AssemblyDelta operation was checked above"),
        };
        operations.push(lowered);
    }
    Ok(json!({
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": base.domain_pack_id,
        "base_asset_version_id": base.asset_version_id,
        "summary": change_set.summary,
        "operations": operations,
        "visual_only": true,
    }))
}

fn native_apply_change_set_operation(
    repository: &forgecad_core::CoreRepository,
    base: &AgentAssetVersion,
    change_set: &AgentAssetChangeSet,
    preview: &mut AgentAssetVersion,
    operation: &Value,
) -> Result<(), NativeBlockoutCompatError> {
    let operation = operation.as_object().ok_or_else(|| {
        NativeBlockoutCompatError::invalid(
            "CHANGE_SET_OPERATION_INVALID",
            "Every ChangeSet operation must be a JSON object.",
        )
    })?;
    const ALLOWED_KEYS: [&str; 20] = [
        "operation_id",
        "op",
        "part_id",
        "path",
        "value",
        "transform",
        "material_id",
        "material_zone_id",
        "replacement_component_id",
        "recipe_request_id",
        "component_recipe_ref",
        "recipe_registry_sha256",
        "recipe_slot_bindings",
        "recipe_candidate_id",
        "recipe_candidate_sha256",
        "recipe_snapshot_revision",
        "target_part_id",
        "target_connector_id",
        "connector_id",
        "surface_adornment_program",
    ];
    if operation
        .keys()
        .any(|key| !ALLOWED_KEYS.contains(&key.as_str()) && key != "structure_suggestion_id")
    {
        return Err(NativeBlockoutCompatError::invalid(
            "CHANGE_SET_OPERATION_INVALID",
            "ChangeSet operation contains a field outside the frozen contract.",
        ));
    }
    let kind = native_change_set_operation_string(operation, "op")?;
    let part_id = native_change_set_operation_string(operation, "part_id")?;

    match kind {
        "set_part_transform" => {
            let operation_id = native_change_set_part_operation_id(preview, part_id)?;
            let (_, base_size, _) = native_change_set_base_part_facts(base, part_id)?;
            let transform = operation
                .get("transform")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    NativeBlockoutCompatError::invalid(
                        "TRANSFORM_REQUIRED",
                        "set_part_transform requires a complete bounded transform.",
                    )
                })?;
            if transform.len() != 3
                || !["position", "rotation", "scale"]
                    .iter()
                    .all(|key| transform.contains_key(*key))
            {
                return Err(NativeBlockoutCompatError::invalid(
                    "TRANSFORM_INVALID",
                    "Part transform must contain only position, rotation and scale.",
                ));
            }
            let position = native_change_set_vec3(
                transform.get("position"),
                -100_000.0,
                100_000.0,
                "position",
            )?;
            let rotation = native_change_set_vec3(
                transform.get("rotation"),
                -100_000.0,
                100_000.0,
                "rotation",
            )?;
            let scale = native_change_set_vec3(transform.get("scale"), 0.1, 10.0, "scale")?;
            let scaled_size = [
                base_size[0] * scale[0],
                base_size[1] * scale[1],
                base_size[2] * scale[2],
            ];
            native_change_set_update_part(preview, part_id, position, Some(scaled_size))?;
            native_change_set_update_graph_transform(preview, part_id, position, rotation, scale)?;
            native_change_set_update_shape_transform(
                preview,
                &operation_id,
                position,
                rotation,
                scale,
                base_size,
            )?;
        }
        "set_part_parameter" => {
            let operation_id = native_change_set_part_operation_id(preview, part_id)?;
            let (base_position, base_size, _) = native_change_set_base_part_facts(base, part_id)?;
            let path = native_change_set_operation_string(operation, "path")?;
            let value = operation
                .get("value")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    NativeBlockoutCompatError::invalid(
                        "PARAMETER_INVALID",
                        "Part parameter value must be finite.",
                    )
                })?;
            native_change_set_apply_parameter(
                base,
                preview,
                part_id,
                &operation_id,
                path,
                value,
                base_position,
                base_size,
            )?;
        }
        "apply_material_preset" => {
            let operation_id = native_change_set_part_operation_id(preview, part_id)?;
            let (_, _, zones) = native_change_set_base_part_facts(base, part_id)?;
            let material_id = native_change_set_operation_string(operation, "material_id")?;
            let zone_id = operation
                .get("material_zone_id")
                .and_then(Value::as_str)
                .or_else(|| zones.first().map(String::as_str))
                .ok_or_else(|| {
                    NativeBlockoutCompatError::conflict(
                        "MATERIAL_ZONE_NOT_FOUND",
                        "Part has no stable Material Zone for this visual preset.",
                    )
                })?;
            if !zones.iter().any(|zone| zone == zone_id) {
                return Err(NativeBlockoutCompatError::not_found(
                    "MATERIAL_ZONE_NOT_FOUND",
                    "Requested Material Zone does not belong to the target part.",
                ));
            }
            native_change_set_clear_surface_adornment(preview, part_id, zone_id)?;
            native_change_set_apply_material(
                preview,
                part_id,
                &operation_id,
                zone_id,
                material_id,
            )?;
        }
        "apply_surface_adornment" => {
            let value = operation.get("surface_adornment_program").ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "SURFACE_ADORNMENT_PROGRAM_REQUIRED",
                    "Surface appearance preview requires one bounded adornment program.",
                )
            })?;
            let program: SurfaceAdornmentProgram =
                serde_json::from_value(value.clone()).map_err(|_| {
                    NativeBlockoutCompatError::invalid(
                        "SURFACE_ADORNMENT_PROGRAM_INVALID",
                        "Surface appearance program does not match SurfaceAdornmentProgram@1.",
                    )
                })?;
            let zone_id = native_change_set_operation_string(operation, "material_zone_id")?;
            if program.target_part_id != part_id || program.target_zone_id != zone_id {
                return Err(NativeBlockoutCompatError::conflict(
                    "SURFACE_ADORNMENT_TARGET_MISMATCH",
                    "ChangeSet target and surface appearance provenance must name the same Part and Material Zone.",
                ));
            }
            repository
                .validate_surface_adornment_program(&base.asset_version_id, &program)
                .map_err(native_blockout_core_error)?;
            let (_, _, zones) = native_change_set_base_part_facts(base, part_id)?;
            if !zones.iter().any(|zone| zone == zone_id) {
                return Err(NativeBlockoutCompatError::not_found(
                    "MATERIAL_ZONE_NOT_FOUND",
                    "Surface appearance target zone does not belong to the selected Part.",
                ));
            }
            let base_material = crate::rust_core_runtime::canonical_surface_adornment_material(
                &native_change_set_base_material(base, part_id, zone_id)?,
            );
            if program.base_material != base_material {
                return Err(NativeBlockoutCompatError::conflict(
                    "SURFACE_ADORNMENT_BASE_MATERIAL_STALE",
                    "Surface appearance was authored for a different committed base material.",
                ));
            }
            let program_sha256 = program
                .canonical_sha256()
                .map_err(native_blockout_core_error)?;
            let material_id = format!("mat_a005_{}", &program_sha256[..32]);
            native_change_set_apply_surface_adornment(preview, &program, &material_id)?;
        }
        "set_joint_pose" => {
            let operation_id = native_change_set_part_operation_id(preview, part_id)?;
            let rotation = operation
                .get("transform")
                .and_then(Value::as_object)
                .and_then(|transform| transform.get("rotation"))
                .ok_or_else(|| {
                    NativeBlockoutCompatError::invalid(
                        "TRANSFORM_REQUIRED",
                        "set_joint_pose requires one bounded rotation vector.",
                    )
                })?;
            let rotation =
                native_change_set_vec3(Some(rotation), -100_000.0, 100_000.0, "rotation")?;
            native_change_set_apply_joint(preview, part_id, &operation_id, rotation)?;
        }
        "snap_part_to_connector" => {
            let operation_id = native_change_set_part_operation_id(preview, part_id)?;
            let connector_id = native_change_set_operation_string(operation, "connector_id")?;
            let target_part_id = native_change_set_operation_string(operation, "target_part_id")?;
            let target_connector_id =
                native_change_set_operation_string(operation, "target_connector_id")?;
            native_change_set_snap_part(
                preview,
                part_id,
                &operation_id,
                connector_id,
                target_part_id,
                target_connector_id,
            )?;
        }
        "replace_part" => {
            let has_legacy_component = operation.contains_key("replacement_component_id");
            let has_recipe = [
                "recipe_request_id",
                "component_recipe_ref",
                "recipe_registry_sha256",
                "recipe_slot_bindings",
                "recipe_candidate_id",
                "recipe_candidate_sha256",
                "recipe_snapshot_revision",
            ]
            .iter()
            .any(|field| operation.contains_key(*field));
            match (has_legacy_component, has_recipe) {
                (true, false) => {
                    let component_id =
                        native_change_set_operation_string(operation, "replacement_component_id")?;
                    let component = repository
                        .replacement_component(&base.asset_version_id, part_id, component_id)
                        .map_err(native_blockout_core_error)?;
                    native_change_set_replace_component(preview, part_id, &component)?;
                }
                (false, true) => {
                    let candidate = repository
                        .recipe_replacement_candidate(change_set, &Value::Object(operation.clone()))
                        .map_err(native_blockout_core_error)?;
                    native_change_set_replace_recipe(preview, part_id, &candidate)?;
                }
                _ => {
                    return Err(NativeBlockoutCompatError::invalid(
                        "REPLACE_PART_VARIANT_INVALID",
                        "replace_part must contain exactly one legacy component or sealed Recipe replacement reference.",
                    ));
                }
            }
        }
        "split_part" => {
            let suggestion_id =
                native_change_set_operation_string(operation, "structure_suggestion_id")?;
            let suggestion = repository
                .verified_structure_suggestion(
                    &base.asset_version_id,
                    suggestion_id,
                    "split_part",
                    part_id,
                    None,
                )
                .map_err(native_blockout_core_error)?;
            native_change_set_split_part(preview, &suggestion)?;
        }
        "merge_parts" => {
            let target_part_id = native_change_set_operation_string(operation, "target_part_id")?;
            let suggestion_id =
                native_change_set_operation_string(operation, "structure_suggestion_id")?;
            let suggestion = repository
                .verified_structure_suggestion(
                    &base.asset_version_id,
                    suggestion_id,
                    "merge_parts",
                    part_id,
                    Some(target_part_id),
                )
                .map_err(native_blockout_core_error)?;
            native_change_set_merge_parts(preview, &suggestion)?;
        }
        _ => {
            return Err(NativeBlockoutCompatError::invalid(
                "CHANGE_SET_OPERATION_UNSUPPORTED",
                "ChangeSet operation is outside the code-owned allowlist.",
            ));
        }
    }
    Ok(())
}

fn native_change_set_replace_component(
    preview: &mut AgentAssetVersion,
    part_id: &str,
    component: &AgentComponentRecord,
) -> Result<(), NativeBlockoutCompatError> {
    let operation_id = native_change_set_part_operation_id(preview, part_id)?;
    let source_part = component.part_template.as_object().ok_or_else(|| {
        NativeBlockoutCompatError::conflict(
            "AGENT_COMPONENT_INVALID",
            "Reusable component part snapshot is invalid.",
        )
    })?;
    let source_operation = component.shape_operation.as_object().ok_or_else(|| {
        NativeBlockoutCompatError::conflict(
            "AGENT_COMPONENT_INVALID",
            "Reusable component ShapeProgram operation is invalid.",
        )
    })?;
    if source_operation
        .get("inputs")
        .and_then(Value::as_array)
        .is_some_and(|inputs| !inputs.is_empty())
    {
        return Err(NativeBlockoutCompatError::conflict(
            "AGENT_COMPONENT_GEOMETRY_UNSUPPORTED",
            "Project-local components may only replace one self-contained bounded ShapeProgram operation.",
        ));
    }
    let role = source_part
        .get("role")
        .and_then(Value::as_str)
        .filter(|role| *role == component.role)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "AGENT_COMPONENT_ROLE_INVALID",
                "Reusable component role differs from its immutable part snapshot.",
            )
        })?;
    let target_position = preview
        .parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .and_then(|part| part.get("position_mm"))
        .cloned()
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found(
                "PART_NOT_FOUND",
                "Replacement target part is unavailable.",
            )
        })?;
    let zones = source_part
        .get("material_zone_ids")
        .and_then(Value::as_array)
        .cloned()
        .filter(|zones| !zones.is_empty())
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "AGENT_COMPONENT_ZONE_INVALID",
                "Reusable component has no stable Material Zone.",
            )
        })?;
    let target_zone = zones[0].as_str().ok_or_else(|| {
        NativeBlockoutCompatError::conflict(
            "AGENT_COMPONENT_ZONE_INVALID",
            "Reusable component Material Zone identity is invalid.",
        )
    })?;

    let target_part = preview
        .parts
        .iter_mut()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found(
                "PART_NOT_FOUND",
                "Replacement target part is unavailable.",
            )
        })?;
    for field in [
        "role",
        "size_mm",
        "material_zone_ids",
        "editable_parameters",
        "editable_parameter_bindings",
    ] {
        if let Some(value) = source_part.get(field) {
            target_part.insert(field.into(), value.clone());
        }
    }
    target_part.insert("position_mm".into(), target_position.clone());
    target_part.insert("provenance".into(), Value::String("agent_component".into()));

    let graph_part = native_change_set_graph_part_mut(preview, part_id)?;
    graph_part.insert("role".into(), Value::String(role.to_string()));
    graph_part.insert("material_zones".into(), Value::Array(zones.clone()));
    graph_part.insert("material_zone_ids".into(), Value::Array(zones.clone()));
    if let Some(value) = source_part.get("editable_parameters") {
        graph_part.insert("editable_parameters".into(), value.clone());
    }
    graph_part.insert("provenance".into(), Value::String("agent_component".into()));

    let replacement_bindings = component
        .material_bindings
        .iter()
        .filter_map(|(source_key, material)| {
            let (source_part_id, zone_id) = source_key.split_once(':')?;
            (source_part_id == component.source_part_id)
                .then(|| (zone_id.to_string(), material.clone()))
        })
        .collect::<Vec<_>>();
    let target_material_id = replacement_bindings
        .iter()
        .find(|(zone_id, _)| zone_id == target_zone)
        .and_then(|(_, material)| material.as_str())
        .map(str::to_string);
    {
        let target_operation = native_change_set_shape_operation_mut(preview, &operation_id)?;
        for field in ["op", "inputs", "args"] {
            let value = source_operation.get(field).cloned().ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "AGENT_COMPONENT_GEOMETRY_INVALID",
                    "Reusable component operation is missing a bounded geometry field.",
                )
            })?;
            target_operation.insert(field.into(), value);
        }
        let args = target_operation
            .get_mut("args")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "AGENT_COMPONENT_GEOMETRY_INVALID",
                    "Reusable component operation has no bounded arguments.",
                )
            })?;
        args.insert("position".into(), target_position);
        args.insert("part_role".into(), Value::String(role.to_string()));
        args.insert("zone_id".into(), Value::String(target_zone.to_string()));
        if let Some(material_id) = target_material_id {
            args.insert("material_id".into(), Value::String(material_id));
        }
    }

    for output in preview
        .shape_program
        .get_mut("outputs")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if output.get("operation_id").and_then(Value::as_str) == Some(operation_id.as_str()) {
            if let Some(output) = output.as_object_mut() {
                output.insert("part_role".into(), Value::String(role.to_string()));
            }
        }
    }
    let target_prefix = format!("{part_id}:");
    preview
        .material_bindings
        .retain(|key, _| !key.starts_with(&target_prefix));
    for (zone_id, material) in replacement_bindings {
        preview
            .material_bindings
            .insert(format!("{part_id}:{zone_id}"), material);
    }
    Ok(())
}

/// Atomically substitutes a reviewed C105 recipe subtree into one preview.
/// The target keeps its existing stable Part identity; recipe children retain
/// their deterministic candidate identities.  This intentionally rejects
/// external connections instead of guessing connector adaptation.
fn native_change_set_replace_recipe(
    preview: &mut AgentAssetVersion,
    target_part_id: &str,
    candidate: &ExpandedComponentCandidate,
) -> Result<(), NativeBlockoutCompatError> {
    let candidate_graph = candidate
        .expanded_assembly_graph
        .as_object()
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate AssemblyGraph is invalid.",
            )
        })?;
    let candidate_recipe_instances = serde_json::to_value(&candidate.component_recipe_instances)
        .expect("recipe instance provenance serializes");
    let c106_arm_semantic_components = recipe_preview_output_contract(&candidate_recipe_instances)
        .map_err(native_blockout_product_tool_error)?
        == RecipePreviewOutputContract::C106ArmSemanticComponents;
    let candidate_root = candidate_graph
        .get("root_part_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate AssemblyGraph has no root Part.",
            )
        })?
        .to_string();
    let base_graph = preview.assembly_graph.as_object().ok_or_else(|| {
        NativeBlockoutCompatError::conflict(
            "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
            "Preview AssemblyGraph is invalid.",
        )
    })?;
    let base_parts = base_graph
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "Preview AssemblyGraph has no parts.",
            )
        })?;
    if !base_parts
        .iter()
        .any(|part| part.get("part_id").and_then(Value::as_str) == Some(target_part_id))
    {
        return Err(NativeBlockoutCompatError::not_found(
            "PART_NOT_FOUND",
            "Recipe replacement target is unavailable.",
        ));
    }
    let target_graph_part = base_parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(target_part_id))
        .expect("checked target")
        .clone();
    let mut descendants = BTreeSet::from([target_part_id.to_string()]);
    loop {
        let before = descendants.len();
        for part in base_parts {
            if let (Some(part_id), Some(parent_id)) = (
                part.get("part_id").and_then(Value::as_str),
                part.get("parent_part_id").and_then(Value::as_str),
            ) {
                if descendants.contains(parent_id) {
                    descendants.insert(part_id.to_string());
                }
            }
        }
        if descendants.len() == before {
            break;
        }
    }
    let base_connections = base_graph
        .get("connections")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "Preview AssemblyGraph has no connections.",
            )
        })?;
    let candidate_root_part = candidate_graph
        .get("parts")
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts.iter().find(|part| {
                part.get("part_id").and_then(Value::as_str) == Some(candidate_root.as_str())
            })
        })
        .cloned()
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate root Part is missing.",
            )
        })?;
    for connection in base_connections {
        let from = connection.get("from_part_id").and_then(Value::as_str);
        let to = connection.get("to_part_id").and_then(Value::as_str);
        let from_inside = from.is_some_and(|id| descendants.contains(id));
        let to_inside = to.is_some_and(|id| descendants.contains(id));
        if from_inside != to_inside && !(from == Some(target_part_id) || to == Some(target_part_id))
        {
            return Err(NativeBlockoutCompatError::conflict(
                "RECIPE_REPLACEMENT_EXTERNAL_CONNECTION",
                "Recipe replacement refuses a subtree with an external connector until a reviewed compatible adapter exists.",
            ));
        }
        if from_inside != to_inside {
            let target_connector = if from == Some(target_part_id) {
                connection.get("from_connector_id")
            } else {
                connection.get("to_connector_id")
            }
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "RECIPE_REPLACEMENT_CONNECTOR_INCOMPATIBLE",
                    "External connection has no target connector.",
                )
            })?;
            let old_connector = target_graph_part
                .get("connectors")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items.iter().find(|item| {
                        item.get("connector_id").and_then(Value::as_str) == Some(target_connector)
                    })
                });
            let new_connector = candidate_root_part
                .get("connectors")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items.iter().find(|item| {
                        item.get("connector_id").and_then(Value::as_str) == Some(target_connector)
                    })
                });
            if !native_recipe_connectors_compatible(old_connector, new_connector)
                || !native_recipe_frames_compatible(
                    target_graph_part.get("pivot"),
                    candidate_root_part.get("pivot"),
                )
            {
                return Err(NativeBlockoutCompatError::conflict("RECIPE_REPLACEMENT_CONNECTOR_INCOMPATIBLE", "External connection connector frame is not explicitly compatible with the reviewed Recipe root."));
            }
        }
    }
    let old_transform = target_graph_part.get("transform").cloned().unwrap_or_else(
        || json!({"position":[0.0,0.0,0.0],"rotation":[0.0,0.0,0.0],"scale":[1.0,1.0,1.0]}),
    );
    let old_position = native_recipe_transform_position(&old_transform)?;
    if !native_recipe_translation_only(&old_transform)
        || !native_recipe_translation_only(
            candidate_root_part.get("transform").unwrap_or(&Value::Null),
        )
    {
        return Err(NativeBlockoutCompatError::conflict("RECIPE_REPLACEMENT_TRANSFORM_UNSUPPORTED", "C105 replacement only supports matching translation frames; rotation or scale requires a reviewed adapter."));
    }
    let candidate_position = native_recipe_transform_position(
        candidate_root_part.get("transform").unwrap_or(&Value::Null),
    )?;
    if old_position
        .iter()
        .zip(candidate_position)
        .any(|(old, placed)| (old - placed).abs() > 1e-9)
        || candidate_root_part.get("parent_part_id") != target_graph_part.get("parent_part_id")
    {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_CANDIDATE_PLACEMENT_MISMATCH",
            "The sealed Recipe candidate is not already placed at the immutable target anchor.",
        ));
    }

    let candidate_parts = candidate_graph
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate AssemblyGraph has no parts.",
            )
        })?;
    let mut id_map = BTreeMap::new();
    id_map.insert(candidate_root.clone(), target_part_id.to_string());
    let retained_ids = base_parts
        .iter()
        .filter_map(|part| part.get("part_id").and_then(Value::as_str))
        .filter(|id| !descendants.contains(*id))
        .collect::<BTreeSet<_>>();
    for part in candidate_parts {
        let id = part.get("part_id").and_then(Value::as_str).ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate Part identity is missing.",
            )
        })?;
        let mapped = id_map.get(id).cloned().unwrap_or_else(|| id.to_string());
        if retained_ids.contains(mapped.as_str())
            || id_map.values().any(|value| value == &mapped) && id != candidate_root
        {
            return Err(NativeBlockoutCompatError::conflict(
                "RECIPE_REPLACEMENT_PART_COLLISION",
                "Recipe replacement Part identities collide with the active AssemblyGraph.",
            ));
        }
        id_map.insert(id.to_string(), mapped);
    }

    let removed_primary_operation_ids = base_parts
        .iter()
        .filter(|part| {
            part.get("part_id")
                .and_then(Value::as_str)
                .is_some_and(|id| descendants.contains(id))
        })
        .filter_map(|part| part.get("operation_id").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    // C105/C106 recipes may compile one stable Part into a source operation
    // plus reviewed derived operations (panels, bevels, trim, etc.).  The
    // AssemblyGraph stores the Part's primary output operation, while every
    // operation for that Part carries the same Rust-owned recipe-instance
    // prefix. Treat that bounded instance closure as the replacement subtree;
    // otherwise an in-Part derived operation is misclassified as an external
    // consumer of the primary operation.
    let removed_recipe_operation_prefixes = base_parts
        .iter()
        .filter(|part| {
            part.get("part_id")
                .and_then(Value::as_str)
                .is_some_and(|id| descendants.contains(id))
        })
        .filter_map(|part| part.get("recipe_instance_id").and_then(Value::as_str))
        .map(|instance_id| format!("op_{}_", instance_id.trim_start_matches("recipeinst_")))
        .collect::<BTreeSet<_>>();
    let existing_operations = preview
        .shape_program
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_GEOMETRY_BINDING_MISSING",
                "Preview ShapeProgram has no operations.",
            )
        })?;
    let old_operation_ids = existing_operations
        .iter()
        .filter_map(|operation| operation.get("operation_id").and_then(Value::as_str))
        .filter(|operation_id| {
            removed_primary_operation_ids.contains(*operation_id)
                || removed_recipe_operation_prefixes
                    .iter()
                    .any(|prefix| operation_id.starts_with(prefix))
        })
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if existing_operations.iter().any(|operation| {
        operation
            .get("inputs")
            .and_then(Value::as_array)
            .is_some_and(|inputs| {
                inputs
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|input| old_operation_ids.contains(input))
                    && !old_operation_ids.contains(
                        operation
                            .get("operation_id")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    )
            })
    }) {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_REPLACEMENT_EXTERNAL_GEOMETRY",
            "Recipe replacement refuses a subtree referenced by external ShapeProgram operations.",
        ));
    }
    let candidate_operations = candidate
        .expanded_shape_program
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe candidate ShapeProgram has no operations.",
            )
        })?;
    let retained_operation_ids = existing_operations
        .iter()
        .filter_map(|operation| operation.get("operation_id").and_then(Value::as_str))
        .filter(|id| !old_operation_ids.contains(*id))
        .collect::<BTreeSet<_>>();
    if candidate_operations.iter().any(|operation| {
        operation
            .get("operation_id")
            .and_then(Value::as_str)
            .is_none_or(|id| retained_operation_ids.contains(id))
    }) {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_REPLACEMENT_OPERATION_COLLISION",
            "Recipe replacement ShapeProgram operations collide with the active asset.",
        ));
    }

    let mut new_graph = preview.assembly_graph.clone();
    let graph = new_graph.as_object_mut().expect("checked object");
    let mut merged_parts = base_parts
        .iter()
        .filter(|part| {
            !part
                .get("part_id")
                .and_then(Value::as_str)
                .is_some_and(|id| descendants.contains(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    for part in candidate_parts {
        let mut part = part.clone();
        let object = part.as_object_mut().expect("candidate graph validated");
        let original_id = object
            .get("part_id")
            .and_then(Value::as_str)
            .expect("candidate part id")
            .to_string();
        object.insert(
            "part_id".into(),
            Value::String(id_map[&original_id].clone()),
        );
        if original_id == candidate_root {
            // Rust Core has already baked the immutable target translation
            // and parent anchor into the candidate and its SHA.  The bridge
            // only assigns the stable target identity; it must not rewrite
            // geometry or placement after the Q003 input is sealed.
        } else if let Some(parent_id) = object
            .get("parent_part_id")
            .and_then(Value::as_str)
            .map(str::to_string)
        {
            object.insert(
                "parent_part_id".into(),
                Value::String(id_map[&parent_id].clone()),
            );
        }
        merged_parts.push(part);
    }
    graph.insert("parts".into(), Value::Array(merged_parts));
    if base_graph.get("root_part_id").and_then(Value::as_str) == Some(target_part_id) {
        graph.insert(
            "root_part_id".into(),
            Value::String(target_part_id.to_string()),
        );
    }
    let mut merged_connections = base_connections
        .iter()
        .filter(|connection| {
            let from = connection.get("from_part_id").and_then(Value::as_str);
            let to = connection.get("to_part_id").and_then(Value::as_str);
            !(from.is_some_and(|id| descendants.contains(id))
                && to.is_some_and(|id| descendants.contains(id)))
        })
        .cloned()
        .collect::<Vec<_>>();
    for connection in candidate_graph
        .get("connections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let mut connection = connection.clone();
        let object = connection
            .as_object_mut()
            .expect("candidate connection validated");
        for field in ["from_part_id", "to_part_id"] {
            let source = object
                .get(field)
                .and_then(Value::as_str)
                .expect("candidate connection part")
                .to_string();
            object.insert(field.into(), Value::String(id_map[&source].clone()));
        }
        merged_connections.push(connection);
    }
    graph.insert("connections".into(), Value::Array(merged_connections));
    let old_instance_ids = base_parts
        .iter()
        .filter(|part| {
            part.get("part_id")
                .and_then(Value::as_str)
                .is_some_and(|id| descendants.contains(id))
        })
        .filter_map(|part| {
            part.get("recipe_instance_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<BTreeSet<_>>();
    let mut provenance = base_graph
        .get("component_recipe_instances")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| {
            !item
                .get("instance_id")
                .and_then(Value::as_str)
                .is_some_and(|id| old_instance_ids.contains(id))
        })
        .collect::<Vec<_>>();
    provenance.extend(
        candidate
            .component_recipe_instances
            .iter()
            .map(|item| serde_json::to_value(item).expect("recipe provenance serializes")),
    );
    graph.insert(
        "component_recipe_instances".into(),
        Value::Array(provenance),
    );

    let candidate_profile_inputs = candidate
        .expanded_shape_program
        .get("profile_inputs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let candidate_input_ids = candidate_profile_inputs
        .iter()
        .filter_map(|input| input.get("input_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let old_profile_inputs = preview
        .shape_program
        .get("profile_inputs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut merged_operations = existing_operations
        .iter()
        .filter(|operation| {
            !operation
                .get("operation_id")
                .and_then(Value::as_str)
                .is_some_and(|id| old_operation_ids.contains(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    merged_operations.extend(candidate_operations.iter().cloned());
    let referenced_profile_ids = merged_operations
        .iter()
        .filter_map(|operation| operation.get("args"))
        .filter_map(Value::as_object)
        .filter_map(|args| {
            args.get("profile_input_id")
                .or_else(|| args.get("section_set_input_id"))
        })
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    if candidate_input_ids.iter().any(|id| {
        old_profile_inputs
            .iter()
            .any(|input| input.get("input_id").and_then(Value::as_str) == Some(*id))
    }) {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_REPLACEMENT_PROFILE_COLLISION",
            "Recipe replacement profile input identities collide with the active asset.",
        ));
    }
    let mut merged_inputs = old_profile_inputs
        .into_iter()
        .filter(|input| {
            input
                .get("input_id")
                .and_then(Value::as_str)
                .is_some_and(|id| referenced_profile_ids.contains(id))
        })
        .collect::<Vec<_>>();
    merged_inputs.extend(candidate_profile_inputs);
    let existing_outputs = preview
        .shape_program
        .get("outputs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut merged_outputs = existing_outputs
        .into_iter()
        .filter(|output| {
            !output
                .get("operation_id")
                .and_then(Value::as_str)
                .is_some_and(|id| old_operation_ids.contains(id))
        })
        .collect::<Vec<_>>();
    merged_outputs.extend(
        candidate
            .expanded_shape_program
            .get("outputs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .cloned(),
    );
    let program = preview
        .shape_program
        .as_object_mut()
        .expect("validated ShapeProgram");
    program.insert("profile_inputs".into(), Value::Array(merged_inputs));
    program.insert("operations".into(), Value::Array(merged_operations));
    program.insert("outputs".into(), Value::Array(merged_outputs));

    let mut retained_parts = preview
        .parts
        .iter()
        .filter(|part| {
            !part
                .get("part_id")
                .and_then(Value::as_str)
                .is_some_and(|id| descendants.contains(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut retained_bindings = preview
        .material_bindings
        .iter()
        .filter(|(key, _)| {
            !descendants
                .iter()
                .any(|part_id| key.starts_with(&format!("{part_id}:")))
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let operation_map = candidate_operations
        .iter()
        .filter_map(|operation| Some((operation.get("operation_id")?.as_str()?, operation)))
        .collect::<BTreeMap<_, _>>();
    let merged_graph_parts = new_graph
        .get("parts")
        .and_then(Value::as_array)
        .expect("merged graph parts");
    for graph_part in merged_graph_parts.iter().filter(|part| {
        part.get("part_id")
            .and_then(Value::as_str)
            .is_some_and(|id| {
                id == target_part_id
                    || id_map
                        .values()
                        .any(|candidate_id| candidate_id == id && id != target_part_id)
            })
    }) {
        let part_id = graph_part
            .get("part_id")
            .and_then(Value::as_str)
            .expect("merged recipe part id");
        let operation_id = graph_part
            .get("operation_id")
            .and_then(Value::as_str)
            .expect("merged recipe operation id");
        let operation = operation_map
            .get(operation_id)
            .expect("candidate operation exists");
        let args = operation
            .get("args")
            .and_then(Value::as_object)
            .expect("candidate operation arguments exist");
        let zones = graph_part
            .get("material_zone_ids")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let zone_ids = zones
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        if zone_ids.is_empty() {
            return Err(NativeBlockoutCompatError::conflict(
                "MATERIAL_ZONE_REQUIRED",
                "Recipe replacement part has no stable Material Zone.",
            ));
        }
        let material_id = args
            .get("material_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "MATERIAL_BINDING_REQUIRED",
                    "Recipe replacement operation has no material binding.",
                )
            })?;
        if c106_arm_semantic_components {
            let recipe_instance_id = graph_part
                .get("recipe_instance_id")
                .and_then(Value::as_str)
                .and_then(|value| value.strip_prefix("recipeinst_"))
                .ok_or_else(|| {
                    NativeBlockoutCompatError::invalid(
                        "RECIPE_PREVIEW_GRAPH_INVALID",
                        "C106 Recipe replacement Part is missing its stable instance identity.",
                    )
                })?;
            let operation_prefix = format!("op_{recipe_instance_id}_");
            for zone_id in &zone_ids {
                let materials = candidate_operations
                    .iter()
                    .filter(|candidate_operation| {
                        candidate_operation
                            .get("operation_id")
                            .and_then(Value::as_str)
                            .is_some_and(|id| id.starts_with(&operation_prefix))
                            && candidate_operation
                                .get("args")
                                .and_then(Value::as_object)
                                .and_then(|candidate_args| candidate_args.get("zone_id"))
                                .and_then(Value::as_str)
                                == Some(zone_id.as_str())
                    })
                    .filter_map(|candidate_operation| {
                        candidate_operation
                            .get("args")
                            .and_then(Value::as_object)
                            .and_then(|candidate_args| candidate_args.get("material_id"))
                            .and_then(Value::as_str)
                    })
                    .collect::<BTreeSet<_>>();
                // A production C106 Part may deliberately use more than one
                // material inside one visual zone (for example an aluminum
                // fastener on a composite deck).  The persisted
                // `part_id:zone_id` binding is the authoritative summary for
                // the replacement and must survive an exact Recipe replay;
                // only reject when neither the candidate nor the immutable
                // base provides a deterministic binding.
                let material = if materials.len() == 1 {
                    materials
                        .into_iter()
                        .next()
                        .expect("checked one C106 zone material")
                        .to_string()
                } else if let Some(material) = materials.iter().next() {
                    // Multiple operation materials can share one authored
                    // visual zone.  Preserve a stable summary by selecting
                    // the lexicographically first reviewed material when an
                    // older base does not already carry a binding.
                    (*material).to_string()
                } else {
                    preview
                        .material_bindings
                        .get(&format!("{part_id}:{zone_id}"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .ok_or_else(|| {
                            NativeBlockoutCompatError::conflict(
                                "RECIPE_PREVIEW_GRAPH_INVALID",
                                "C106 Recipe replacement Part has no deterministic material binding for one declared Material Zone.",
                            )
                        })?
                };
                retained_bindings.insert(format!("{part_id}:{zone_id}"), Value::String(material));
            }
        } else {
            for zone_id in &zone_ids {
                retained_bindings.insert(
                    format!("{part_id}:{zone_id}"),
                    Value::String(material_id.to_string()),
                );
            }
        }
        retained_parts.push(json!({
            "part_id": part_id,
            "role": graph_part.get("role").cloned().unwrap_or(Value::String("visual_detail".into())),
            "parent_part_id": graph_part.get("parent_part_id").cloned().unwrap_or(Value::Null),
            "position_mm": graph_part.get("transform").and_then(Value::as_object).and_then(|transform| transform.get("position")).cloned().unwrap_or_else(|| args.get("position").cloned().unwrap_or(json!([0.0,0.0,0.0]))),
            "size_mm": native_recipe_operation_size(operation, &operation_map)?,
            "material_zone_ids": zone_ids,
            "editable_parameters": graph_part.get("editable_parameters").cloned().unwrap_or_else(|| json!([])),
            "editable_parameter_bindings": graph_part.get("editable_parameter_bindings").cloned().unwrap_or_else(|| json!([])),
            "locked": false,
            "provenance": "agent_generated"
        }));
    }
    preview.parts = retained_parts;
    preview.material_bindings = retained_bindings;
    preview.assembly_graph = new_graph;
    Ok(())
}

fn native_recipe_operation_size(
    operation: &Value,
    operations: &BTreeMap<&str, &Value>,
) -> Result<[f64; 3], NativeBlockoutCompatError> {
    let args = operation
        .get("args")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe operation has no bounded arguments.",
            )
        })?;
    if args.get("size").is_some() {
        return native_blockout_vec3(args.get("size"), [100.0, 100.0, 100.0], true);
    }
    if let (Some(radius), Some(height)) = (
        args.get("radius").and_then(Value::as_f64),
        args.get("height").and_then(Value::as_f64),
    ) {
        return native_blockout_vec3(
            Some(&json!([radius * 2.0, height, radius * 2.0])),
            [100.0, 100.0, 100.0],
            true,
        );
    }
    if operation.get("op").and_then(Value::as_str) == Some("revolve") {
        return native_recipe_revolve_size(operation, operations);
    }
    // Reviewed C106 parts may expose a derived output (bevel, linear/radial
    // array, mirror or boolean) rather than the primitive source operation.
    // Carry a finite bound through that input closure instead of treating a
    // valid derived visual feature as an un-sized arbitrary mesh.
    if let Some(input_id) = operation
        .get("inputs")
        .and_then(Value::as_array)
        .and_then(|inputs| inputs.first())
        .and_then(Value::as_str)
    {
        let input = operations.get(input_id).ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe derived operation references a missing bounded input.",
            )
        })?;
        let mut size = native_recipe_operation_size(input, operations)?;
        match operation.get("op").and_then(Value::as_str) {
            Some("bevel_approx") => {
                let radius = args.get("radius").and_then(Value::as_f64).ok_or_else(|| {
                    NativeBlockoutCompatError::conflict(
                        "RECIPE_PREVIEW_GEOMETRY_INVALID",
                        "Recipe bevel operation has no bounded radius.",
                    )
                })?;
                if !radius.is_finite() || radius < 0.0 {
                    return Err(NativeBlockoutCompatError::conflict(
                        "RECIPE_PREVIEW_GEOMETRY_INVALID",
                        "Recipe bevel radius is invalid.",
                    ));
                }
                size = size.map(|value| value + radius * 2.0);
            }
            Some("array") => {
                let count = args
                    .get("count")
                    .and_then(Value::as_u64)
                    .filter(|count| (1..=128).contains(count))
                    .ok_or_else(|| {
                        NativeBlockoutCompatError::conflict(
                            "RECIPE_PREVIEW_GEOMETRY_INVALID",
                            "Recipe array count is outside the bounded visual range.",
                        )
                    })?;
                let spacing = args.get("spacing").and_then(Value::as_f64).ok_or_else(|| {
                    NativeBlockoutCompatError::conflict(
                        "RECIPE_PREVIEW_GEOMETRY_INVALID",
                        "Recipe array spacing is missing.",
                    )
                })?;
                let axis = native_blockout_vec3(args.get("axis"), [0.0, 0.0, 0.0], false)?;
                if !spacing.is_finite() || spacing.abs() > 100_000.0 {
                    return Err(NativeBlockoutCompatError::conflict(
                        "RECIPE_PREVIEW_GEOMETRY_INVALID",
                        "Recipe array spacing is outside the visual bound.",
                    ));
                }
                let extent = spacing.abs() * (count.saturating_sub(1) as f64);
                for axis_index in 0..3 {
                    size[axis_index] += axis[axis_index].abs() * extent;
                }
            }
            Some("radial_array") => {
                let radius = args.get("radius").and_then(Value::as_f64).ok_or_else(|| {
                    NativeBlockoutCompatError::conflict(
                        "RECIPE_PREVIEW_GEOMETRY_INVALID",
                        "Recipe radial array radius is missing.",
                    )
                })?;
                if !radius.is_finite() || radius < 0.0 || radius > 100_000.0 {
                    return Err(NativeBlockoutCompatError::conflict(
                        "RECIPE_PREVIEW_GEOMETRY_INVALID",
                        "Recipe radial array radius is outside the visual bound.",
                    ));
                }
                size = size.map(|value| value + radius * 2.0);
            }
            Some("mirror") | Some("translate") | Some("rotate") | Some("subtract") => {}
            Some("union") => {
                // A union's exact bounds require all inputs; the reviewed
                // primitive closure remains bounded by the first input plus
                // its own extent, which is sufficient for Part metadata.
            }
            _ => {
                return Err(NativeBlockoutCompatError::conflict(
                    "RECIPE_PREVIEW_GEOMETRY_INVALID",
                    "Recipe operation has no bounded visual size.",
                ));
            }
        }
        return Ok(size.map(|value| value.max(0.001)));
    }
    let points = args
        .get("path_points")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe operation has no bounded visual size.",
            )
        })?;
    let scale = args
        .get("profile_scale")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe sweep has no profile scale.",
            )
        })?;
    let radius = scale
        .iter()
        .filter_map(Value::as_f64)
        .fold(0.0_f64, f64::max);
    if !radius.is_finite() || radius <= 0.0 {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_PREVIEW_GEOMETRY_INVALID",
            "Recipe sweep profile scale is invalid.",
        ));
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for point in points {
        let point = native_blockout_vec3(Some(point), [0.0, 0.0, 0.0], false)?;
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    Ok([
        max[0] - min[0] + radius * 2.0,
        max[1] - min[1] + radius * 2.0,
        max[2] - min[2] + radius * 2.0,
    ]
    .map(|value| value.max(0.001)))
}

/// Derive an explicit finite world-space bound for the mesh generated by a
/// bounded ProfileSketch revolution.  The profile itself is a 2D helper and
/// must not be presented as an independently sized Part, but its points are
/// the only valid source for the revolved mesh extent.  A malformed profile is
/// rejected rather than replaced with a synthetic fallback size.
fn native_recipe_revolve_size(
    operation: &Value,
    operations: &BTreeMap<&str, &Value>,
) -> Result<[f64; 3], NativeBlockoutCompatError> {
    let input_id = operation
        .get("inputs")
        .and_then(Value::as_array)
        .filter(|inputs| inputs.len() == 1)
        .and_then(|inputs| inputs.first())
        .and_then(Value::as_str)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe revolve requires one bounded ProfileSketch input.",
            )
        })?;
    let profile = operations.get(input_id).ok_or_else(|| {
        NativeBlockoutCompatError::conflict(
            "RECIPE_PREVIEW_GEOMETRY_INVALID",
            "Recipe revolve ProfileSketch input is absent from the candidate closure.",
        )
    })?;
    if profile.get("op").and_then(Value::as_str) != Some("profile") {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_PREVIEW_GEOMETRY_INVALID",
            "Recipe revolve input must be a bounded ProfileSketch.",
        ));
    }
    let points = profile
        .get("args")
        .and_then(Value::as_object)
        .and_then(|args| args.get("points"))
        .and_then(Value::as_array)
        .filter(|points| points.len() >= 3)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe ProfileSketch has no bounded contour points.",
            )
        })?;
    let mut radial_extent = 0.0_f64;
    let mut axial_min = f64::INFINITY;
    let mut axial_max = f64::NEG_INFINITY;
    for point in points {
        let point = point
            .as_array()
            .filter(|point| point.len() == 2)
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "RECIPE_PREVIEW_GEOMETRY_INVALID",
                    "Recipe ProfileSketch contour point must be a finite two-axis point.",
                )
            })?;
        let radial = point[0]
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "RECIPE_PREVIEW_GEOMETRY_INVALID",
                    "Recipe ProfileSketch contour radius must be finite.",
                )
            })?;
        let axial = point[1]
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                NativeBlockoutCompatError::conflict(
                    "RECIPE_PREVIEW_GEOMETRY_INVALID",
                    "Recipe ProfileSketch contour axis must be finite.",
                )
            })?;
        radial_extent = radial_extent.max(radial.abs());
        axial_min = axial_min.min(axial);
        axial_max = axial_max.max(axial);
    }
    let axial_extent = axial_max - axial_min;
    if !radial_extent.is_finite()
        || !axial_extent.is_finite()
        || radial_extent <= 0.0
        || axial_extent <= 0.0
    {
        return Err(NativeBlockoutCompatError::conflict(
            "RECIPE_PREVIEW_GEOMETRY_INVALID",
            "Recipe ProfileSketch cannot form a non-degenerate revolve bound.",
        ));
    }
    Ok([radial_extent * 2.0, axial_extent, radial_extent * 2.0])
}

fn native_recipe_transform_position(value: &Value) -> Result<[f64; 3], NativeBlockoutCompatError> {
    native_blockout_vec3(value.get("position"), [0.0, 0.0, 0.0], false)
}

fn native_recipe_translation_only(value: &Value) -> bool {
    native_blockout_vec3(value.get("rotation"), [0.0, 0.0, 0.0], false)
        .is_ok_and(|rotation| rotation.iter().all(|value| value.abs() <= 1e-9))
        && native_blockout_vec3(value.get("scale"), [1.0, 1.0, 1.0], false)
            .is_ok_and(|scale| scale.iter().all(|value| (*value - 1.0).abs() <= 1e-9))
}

fn native_recipe_connectors_compatible(old: Option<&Value>, new: Option<&Value>) -> bool {
    let Some((old, new)) = old.zip(new) else {
        return false;
    };
    old.get("connector_id") == new.get("connector_id") && old.get("kind") == new.get("kind")
        && ["position", "normal", "up"].iter().all(|field| {
            let left = native_blockout_vec3(old.get(*field), [0.0, 0.0, 0.0], false);
            let right = native_blockout_vec3(new.get(*field), [0.0, 0.0, 0.0], false);
            matches!((left, right), (Ok(left), Ok(right)) if left.iter().zip(right).all(|(a,b)| (a-b).abs() <= 1e-9))
        })
}

fn native_recipe_frames_compatible(left: Option<&Value>, right: Option<&Value>) -> bool {
    let Some((left, right)) = left.zip(right) else {
        return false;
    };
    ["position", "normal", "up"].iter().all(|field| {
        let left = native_blockout_vec3(left.get(*field), [0.0, 0.0, 0.0], false);
        let right = native_blockout_vec3(right.get(*field), [0.0, 0.0, 0.0], false);
        matches!((left, right), (Ok(left), Ok(right)) if left.iter().zip(right).all(|(a,b)| (a-b).abs() <= 1e-9))
    })
}

fn native_change_set_split_part(
    preview: &mut AgentAssetVersion,
    suggestion: &AgentStructureSuggestion,
) -> Result<(), NativeBlockoutCompatError> {
    let source_part_id = suggestion.part_id.as_str();
    let source_part = preview
        .parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(source_part_id))
        .cloned()
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found(
                "PART_NOT_FOUND",
                "Split source part is unavailable.",
            )
        })?;
    let source_role = source_part
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "PART_ROLE_INVALID",
                "Split source part has no stable role.",
            )
        })?
        .to_string();
    let target_operation_id = preview
        .shape_program
        .get("outputs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|output| output.get("part_role").and_then(Value::as_str) == Some(&source_role))
        .filter_map(|output| output.get("operation_id").and_then(Value::as_str))
        .filter(|operation_id| {
            preview
                .shape_program
                .get("operations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|operation| {
                    operation.get("operation_id").and_then(Value::as_str) == Some(*operation_id)
                })
                .is_some_and(|operation| {
                    operation
                        .get("op")
                        .and_then(Value::as_str)
                        .is_some_and(|kind| {
                            matches!(kind, "box" | "cylinder" | "capsule" | "wedge")
                        })
                        && operation
                            .get("inputs")
                            .and_then(Value::as_array)
                            .is_none_or(Vec::is_empty)
                })
        })
        .last()
        .map(str::to_string)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "STRUCTURE_SUGGESTION_NOT_AVAILABLE",
                "Split geometry facts are no longer available.",
            )
        })?;
    let new_part_id = format!(
        "part_{}",
        &sha256_hex(suggestion.suggestion_id.as_bytes())[..18]
    );
    let suffix = new_part_id.trim_start_matches("part_");
    let suffix = &suffix[..suffix.len().min(8)];
    let maximum_prefix = 54usize.saturating_sub(suffix.len()).max(1);
    let role_prefix = source_role.chars().take(maximum_prefix).collect::<String>();
    let new_role = format!("{role_prefix}_detail_{suffix}");
    let new_zone_id = format!("zone_{new_role}");

    let operation = native_change_set_shape_operation_mut(preview, &target_operation_id)?;
    let args = operation
        .get_mut("args")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "STRUCTURE_SUGGESTION_NOT_AVAILABLE",
                "Split operation has no bounded arguments.",
            )
        })?;
    args.insert("part_role".into(), Value::String(new_role.clone()));
    args.insert("zone_id".into(), Value::String(new_zone_id.clone()));
    let operation_snapshot = Value::Object(operation.clone());
    let (position, size) = native_change_set_operation_bounds(&operation_snapshot)?;
    let output_id = preview
        .shape_program
        .get_mut("outputs")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .find(|output| {
            output.get("operation_id").and_then(Value::as_str) == Some(target_operation_id.as_str())
        })
        .and_then(|output| {
            let output_id = output.get("output_id").and_then(Value::as_str)?.to_string();
            output
                .as_object_mut()?
                .insert("part_role".into(), Value::String(new_role.clone()));
            Some(output_id)
        })
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "STRUCTURE_SUGGESTION_NOT_AVAILABLE",
                "Split output binding is no longer available.",
            )
        })?;
    let editable_parameters = source_part
        .get("editable_parameters")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let editable_parameter_bindings = source_part
        .get("editable_parameter_bindings")
        .cloned()
        .unwrap_or_else(|| json!([]));
    preview.parts.push(json!({
        "part_id": new_part_id,
        "role": new_role,
        "parent_part_id": source_part_id,
        "position_mm": position,
        "size_mm": size,
        "material_zone_ids": [new_zone_id],
        "editable_parameters": editable_parameters,
        "editable_parameter_bindings": editable_parameter_bindings,
        "locked": false,
        "provenance": "agent_generated"
    }));
    let graph_parts = preview
        .assembly_graph
        .get_mut("parts")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "Split source AssemblyGraph parts are unavailable.",
            )
        })?;
    graph_parts.push(json!({
        "part_id": new_part_id,
        "role": new_role,
        "parent_part_id": source_part_id,
        "geometry_source": "shape_program",
        "output_id": output_id,
        "operation_id": target_operation_id,
        "transform": {"position": position, "rotation": [0.0,0.0,0.0], "scale": [1.0,1.0,1.0]},
        "connectors": [],
        "joints": [],
        "material_zones": [new_zone_id],
        "material_zone_ids": [new_zone_id],
        "editable_parameters": editable_parameters,
        "locked": false,
        "provenance": "agent_generated"
    }));
    let source_zone = source_part
        .get("material_zone_ids")
        .and_then(Value::as_array)
        .and_then(|zones| zones.first())
        .and_then(Value::as_str);
    if let Some(material) = source_zone.and_then(|zone| {
        preview
            .material_bindings
            .get(&format!("{source_part_id}:{zone}"))
            .cloned()
    }) {
        preview
            .material_bindings
            .insert(format!("{new_part_id}:{new_zone_id}"), material);
    }
    Ok(())
}

fn native_change_set_merge_parts(
    preview: &mut AgentAssetVersion,
    suggestion: &AgentStructureSuggestion,
) -> Result<(), NativeBlockoutCompatError> {
    let survivor_id = suggestion.part_id.as_str();
    let absorbed_id = suggestion.target_part_id.as_deref().ok_or_else(|| {
        NativeBlockoutCompatError::conflict(
            "STRUCTURE_SUGGESTION_MISMATCH",
            "Merge suggestion has no target part.",
        )
    })?;
    let survivor = preview
        .parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(survivor_id))
        .cloned()
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found("PART_NOT_FOUND", "Merge survivor is unavailable.")
        })?;
    let absorbed = preview
        .parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(absorbed_id))
        .cloned()
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found("PART_NOT_FOUND", "Merge target is unavailable.")
        })?;
    let survivor_role = survivor
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "PART_ROLE_INVALID",
                "Merge survivor has no stable role.",
            )
        })?
        .to_string();
    let absorbed_role = absorbed
        .get("role")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "PART_ROLE_INVALID",
                "Merge target has no stable role.",
            )
        })?
        .to_string();
    let (position, size) = native_change_set_combined_part_bounds(&survivor, &absorbed)?;
    for operation in preview
        .shape_program
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if operation
            .get("args")
            .and_then(|args| args.get("part_role"))
            .and_then(Value::as_str)
            == Some(absorbed_role.as_str())
        {
            if let Some(args) = operation.get_mut("args").and_then(Value::as_object_mut) {
                args.insert("part_role".into(), Value::String(survivor_role.clone()));
            }
        }
    }
    for output in preview
        .shape_program
        .get_mut("outputs")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if output.get("part_role").and_then(Value::as_str) == Some(absorbed_role.as_str()) {
            if let Some(output) = output.as_object_mut() {
                output.insert("part_role".into(), Value::String(survivor_role.clone()));
            }
        }
    }
    let absorbed_zones = absorbed
        .get("material_zone_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let survivor_part = preview
        .parts
        .iter_mut()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(survivor_id))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found("PART_NOT_FOUND", "Merge survivor is unavailable.")
        })?;
    let mut zones = survivor_part
        .get("material_zone_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for zone in &absorbed_zones {
        if !zones.contains(zone) {
            zones.push(zone.clone());
        }
    }
    survivor_part.insert("material_zone_ids".into(), Value::Array(zones.clone()));
    survivor_part.insert("position_mm".into(), json!(position));
    survivor_part.insert("size_mm".into(), json!(size));
    preview
        .parts
        .retain(|part| part.get("part_id").and_then(Value::as_str) != Some(absorbed_id));

    let graph_parts = preview
        .assembly_graph
        .get_mut("parts")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "Merge AssemblyGraph parts are unavailable.",
            )
        })?;
    let survivor_graph = graph_parts
        .iter_mut()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(survivor_id))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "Merge survivor is missing from AssemblyGraph.",
            )
        })?;
    survivor_graph.insert("material_zones".into(), Value::Array(zones.clone()));
    survivor_graph.insert("material_zone_ids".into(), Value::Array(zones));
    let transform = survivor_graph
        .entry("transform")
        .or_insert_with(
            || json!({"position":position,"rotation":[0.0,0.0,0.0],"scale":[1.0,1.0,1.0]}),
        )
        .as_object_mut()
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "Merge survivor transform is invalid.",
            )
        })?;
    transform.insert("position".into(), json!(position));
    graph_parts.retain(|part| part.get("part_id").and_then(Value::as_str) != Some(absorbed_id));
    if let Some(connections) = preview
        .assembly_graph
        .get_mut("connections")
        .and_then(Value::as_array_mut)
    {
        connections.retain(|connection| {
            connection.get("from_part_id").and_then(Value::as_str) != Some(absorbed_id)
                && connection.get("to_part_id").and_then(Value::as_str) != Some(absorbed_id)
        });
    }
    let absorbed_prefix = format!("{absorbed_id}:");
    let moved = preview
        .material_bindings
        .iter()
        .filter(|(key, _)| key.starts_with(&absorbed_prefix))
        .filter_map(|(key, value)| {
            key.split_once(':')
                .map(|(_, zone)| (zone.to_string(), value.clone()))
        })
        .collect::<Vec<_>>();
    preview
        .material_bindings
        .retain(|key, _| !key.starts_with(&absorbed_prefix));
    for (zone, material) in moved {
        preview
            .material_bindings
            .insert(format!("{survivor_id}:{zone}"), material);
    }
    Ok(())
}

fn native_change_set_operation_bounds(
    operation: &Value,
) -> Result<([f64; 3], [f64; 3]), NativeBlockoutCompatError> {
    let args = operation
        .get("args")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "STRUCTURE_SUGGESTION_NOT_AVAILABLE",
                "Structure operation arguments are unavailable.",
            )
        })?;
    let position = args
        .get("position")
        .map(|value| native_change_set_vec3(Some(value), -100_000.0, 100_000.0, "position"))
        .transpose()?
        .unwrap_or([0.0, 0.0, 0.0]);
    let size = if matches!(
        operation.get("op").and_then(Value::as_str),
        Some("cylinder" | "capsule")
    ) {
        let radius = args
            .get("radius")
            .and_then(Value::as_f64)
            .filter(|value| *value > 0.0);
        let height = args
            .get("height")
            .and_then(Value::as_f64)
            .filter(|value| *value > 0.0);
        match radius.zip(height) {
            Some((radius, height)) => [radius * 2.0, height, radius * 2.0],
            None => {
                return Err(NativeBlockoutCompatError::conflict(
                    "STRUCTURE_SUGGESTION_NOT_AVAILABLE",
                    "Structure primitive dimensions are invalid.",
                ));
            }
        }
    } else {
        native_change_set_vec3(args.get("size"), 0.0001, 100_000.0, "size")?
    };
    Ok((position, size))
}

fn native_change_set_combined_part_bounds(
    first: &Value,
    second: &Value,
) -> Result<([f64; 3], [f64; 3]), NativeBlockoutCompatError> {
    let first_position = native_change_set_vec3(
        first.get("position_mm"),
        -100_000.0,
        100_000.0,
        "position_mm",
    )?;
    let first_size = native_change_set_vec3(first.get("size_mm"), 0.0001, 100_000.0, "size_mm")?;
    let second_position = native_change_set_vec3(
        second.get("position_mm"),
        -100_000.0,
        100_000.0,
        "position_mm",
    )?;
    let second_size = native_change_set_vec3(second.get("size_mm"), 0.0001, 100_000.0, "size_mm")?;
    let mut position = [0.0; 3];
    let mut size = [0.0; 3];
    for axis in 0..3 {
        let lower = (first_position[axis] - first_size[axis] / 2.0)
            .min(second_position[axis] - second_size[axis] / 2.0);
        let upper = (first_position[axis] + first_size[axis] / 2.0)
            .max(second_position[axis] + second_size[axis] / 2.0);
        position[axis] = (lower + upper) / 2.0;
        size[axis] = upper - lower;
    }
    Ok((position, size))
}

fn native_change_set_operation_string<'a>(
    operation: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, NativeBlockoutCompatError> {
    operation
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 256 && valid_stable_id(value))
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "CHANGE_SET_OPERATION_INVALID",
                format!("ChangeSet operation requires bounded field {field}."),
            )
        })
}

fn native_change_set_part_operation_id(
    version: &AgentAssetVersion,
    part_id: &str,
) -> Result<String, NativeBlockoutCompatError> {
    let matches = version
        .assembly_graph
        .get("parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .filter_map(|part| part.get("operation_id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if matches.len() != 1 || !matches[0].starts_with("op_") || !valid_stable_id(matches[0]) {
        return Err(NativeBlockoutCompatError::conflict(
            "CHANGE_SET_GEOMETRY_BINDING_MISSING",
            "Target part does not have one stable ShapeProgram operation binding.",
        ));
    }
    Ok(matches[0].to_string())
}

fn native_change_set_base_part_facts(
    base: &AgentAssetVersion,
    part_id: &str,
) -> Result<([f64; 3], [f64; 3], Vec<String>), NativeBlockoutCompatError> {
    let part = base
        .parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found(
                "PART_NOT_FOUND",
                "ChangeSet target part is unavailable.",
            )
        })?;
    let position = native_change_set_vec3(
        part.get("position_mm"),
        -100_000.0,
        100_000.0,
        "position_mm",
    )?;
    let size = native_change_set_vec3(part.get("size_mm"), 0.0001, 100_000.0, "size_mm")?;
    let zones = part
        .get("material_zone_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    Ok((position, size, zones))
}

fn native_change_set_vec3(
    value: Option<&Value>,
    minimum: f64,
    maximum: f64,
    label: &str,
) -> Result<[f64; 3], NativeBlockoutCompatError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "CHANGE_SET_VECTOR_INVALID",
                format!("ChangeSet {label} must contain exactly three numbers."),
            )
        })?;
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .filter(|value| value.is_finite() && (minimum..=maximum).contains(value))
            .ok_or_else(|| {
                NativeBlockoutCompatError::invalid(
                    "CHANGE_SET_VECTOR_INVALID",
                    format!("ChangeSet {label} is outside its bounded range."),
                )
            })?;
    }
    Ok(result)
}

fn native_change_set_update_part(
    preview: &mut AgentAssetVersion,
    part_id: &str,
    position: [f64; 3],
    size: Option<[f64; 3]>,
) -> Result<(), NativeBlockoutCompatError> {
    let part = preview
        .parts
        .iter_mut()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found(
                "PART_NOT_FOUND",
                "ChangeSet target part is unavailable.",
            )
        })?;
    part.insert("position_mm".into(), json!(position));
    if let Some(size) = size {
        part.insert("size_mm".into(), json!(size));
    }
    Ok(())
}

fn native_change_set_graph_part_mut<'a>(
    preview: &'a mut AgentAssetVersion,
    part_id: &str,
) -> Result<&'a mut serde_json::Map<String, Value>, NativeBlockoutCompatError> {
    preview
        .assembly_graph
        .get_mut("parts")
        .and_then(Value::as_array_mut)
        .and_then(|parts| {
            parts
                .iter_mut()
                .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        })
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "Target part is missing from the sealed AssemblyGraph.",
            )
        })
}

fn native_change_set_shape_operation_mut<'a>(
    preview: &'a mut AgentAssetVersion,
    operation_id: &str,
) -> Result<&'a mut serde_json::Map<String, Value>, NativeBlockoutCompatError> {
    preview
        .shape_program
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .and_then(|operations| {
            operations.iter_mut().find(|operation| {
                operation.get("operation_id").and_then(Value::as_str) == Some(operation_id)
            })
        })
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_GEOMETRY_BINDING_MISSING",
                "Target part ShapeProgram operation is unavailable.",
            )
        })
}

fn native_change_set_update_graph_transform(
    preview: &mut AgentAssetVersion,
    part_id: &str,
    position: [f64; 3],
    rotation: [f64; 3],
    scale: [f64; 3],
) -> Result<(), NativeBlockoutCompatError> {
    native_change_set_graph_part_mut(preview, part_id)?.insert(
        "transform".into(),
        json!({"position":position,"rotation":rotation,"scale":scale}),
    );
    Ok(())
}

fn native_change_set_update_shape_transform(
    preview: &mut AgentAssetVersion,
    operation_id: &str,
    position: [f64; 3],
    rotation: [f64; 3],
    scale: [f64; 3],
    base_size: [f64; 3],
) -> Result<(), NativeBlockoutCompatError> {
    let operation = native_change_set_shape_operation_mut(preview, operation_id)?;
    let kind = operation
        .get("op")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let args = operation
        .get_mut("args")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_GEOMETRY_BINDING_MISSING",
                "ShapeProgram operation has no bounded argument object.",
            )
        })?;
    args.insert("position".into(), json!(position));
    args.insert("rotation".into(), json!(rotation));
    if args.get("size").is_some() {
        args.insert(
            "size".into(),
            json!([
                base_size[0] * scale[0],
                base_size[1] * scale[1],
                base_size[2] * scale[2]
            ]),
        );
    } else if matches!(kind.as_str(), "cylinder" | "capsule")
        && args.get("radius").and_then(Value::as_f64).is_some()
        && args.get("height").and_then(Value::as_f64).is_some()
    {
        let radius = args.get("radius").and_then(Value::as_f64).unwrap();
        let height = args.get("height").and_then(Value::as_f64).unwrap();
        args.insert("radius".into(), json!(radius * scale[0].max(scale[2])));
        args.insert("height".into(), json!(height * scale[1]));
    } else {
        return Err(NativeBlockoutCompatError::conflict(
            "CHANGE_SET_GEOMETRY_EDIT_UNSUPPORTED",
            "Target ShapeProgram operation cannot accept a bounded transform edit.",
        ));
    }
    Ok(())
}

fn native_change_set_apply_parameter(
    base: &AgentAssetVersion,
    preview: &mut AgentAssetVersion,
    part_id: &str,
    operation_id: &str,
    path: &str,
    value: f64,
    base_position: [f64; 3],
    base_size: [f64; 3],
) -> Result<(), NativeBlockoutCompatError> {
    let (field, axis) = match path {
        "transform.position.x" => ("position", 0),
        "transform.position.y" => ("position", 1),
        "transform.position.z" => ("position", 2),
        "transform.scale.x" => ("scale", 0),
        "transform.scale.y" => ("scale", 1),
        "transform.scale.z" => ("scale", 2),
        _ => {
            return Err(NativeBlockoutCompatError::invalid(
                "PARAMETER_NOT_ALLOWED",
                "Only frozen position and scale parameter paths are executable.",
            ));
        }
    };
    let (current_part_position, current_part_size, _) =
        native_change_set_base_part_facts(preview, part_id)?;
    let graph_part = native_change_set_graph_part_mut(preview, part_id)?;
    let transform = graph_part
        .entry("transform")
        .or_insert_with(|| json!({"position":base_position,"rotation":[0,0,0],"scale":[1,1,1]}))
        .as_object_mut()
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "AssemblyGraph transform is invalid.",
            )
        })?;
    let fallback = if field == "position" {
        base_position
    } else {
        [1.0, 1.0, 1.0]
    };
    let mut vector = transform
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .map(|values| {
            [
                values[0].as_f64().unwrap_or(fallback[0]),
                values[1].as_f64().unwrap_or(fallback[1]),
                values[2].as_f64().unwrap_or(fallback[2]),
            ]
        })
        .unwrap_or(fallback);
    vector[axis] = value;
    transform.insert(field.into(), json!(vector));

    let operation = native_change_set_shape_operation_mut(preview, operation_id)?;
    let kind = operation
        .get("op")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let args = operation
        .get_mut("args")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_GEOMETRY_BINDING_MISSING",
                "ShapeProgram operation has no bounded argument object.",
            )
        })?;
    if field == "position" {
        let mut shape_position = if args.get("position").is_some() {
            native_change_set_vec3(
                args.get("position"),
                -100_000.0,
                100_000.0,
                "ShapeProgram position",
            )?
        } else {
            current_part_position
        };
        shape_position[axis] = value;
        args.insert("position".into(), json!(shape_position));

        let mut part_position = current_part_position;
        part_position[axis] = value;
        native_change_set_update_part(preview, part_id, part_position, None)?;
    } else {
        let target_axis_size = base_size[axis] * value;
        let mut part_size = current_part_size;
        part_size[axis] = target_axis_size;
        if args.get("size").is_some() {
            let mut shape_size =
                native_change_set_vec3(args.get("size"), 0.0001, 100_000.0, "ShapeProgram size")?;
            shape_size[axis] = target_axis_size;
            args.insert("size".into(), json!(shape_size));
        } else if kind == "sweep" && axis == 1 {
            let base_profile_scale = base
                .shape_program
                .get("operations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .find(|operation| {
                    operation.get("operation_id").and_then(Value::as_str) == Some(operation_id)
                })
                .and_then(|operation| operation.get("args"))
                .and_then(|args| args.get("profile_scale"))
                .and_then(Value::as_array)
                .filter(|scale| scale.len() == 2)
                .and_then(|scale| scale[1].as_f64())
                .filter(|scale| scale.is_finite() && *scale > 0.0)
                .ok_or_else(|| {
                    NativeBlockoutCompatError::conflict(
                        "CHANGE_SET_GEOMETRY_BINDING_MISSING",
                        "Sweep parameter edit requires a reviewed base profile scale.",
                    )
                })?;
            let profile_scale = args
                .get_mut("profile_scale")
                .and_then(Value::as_array_mut)
                .filter(|scale| scale.len() == 2)
                .ok_or_else(|| {
                    NativeBlockoutCompatError::conflict(
                        "CHANGE_SET_GEOMETRY_BINDING_MISSING",
                        "Sweep ShapeProgram operation has no mutable profile scale.",
                    )
                })?;
            profile_scale[1] = json!(base_profile_scale * value);
        } else if matches!(kind.as_str(), "cylinder" | "capsule") {
            if axis == 1 {
                args.insert("height".into(), json!(target_axis_size));
            } else {
                args.insert("radius".into(), json!(target_axis_size / 2.0));
            }
        } else {
            return Err(NativeBlockoutCompatError::conflict(
                "CHANGE_SET_GEOMETRY_EDIT_UNSUPPORTED",
                "Target ShapeProgram operation cannot accept this parameter edit.",
            ));
        }
        native_change_set_update_part(preview, part_id, current_part_position, Some(part_size))?;
    }
    Ok(())
}

fn native_change_set_apply_material(
    preview: &mut AgentAssetVersion,
    part_id: &str,
    operation_id: &str,
    zone_id: &str,
    material_id: &str,
) -> Result<(), NativeBlockoutCompatError> {
    let operation = native_change_set_shape_operation_mut(preview, operation_id)?;
    let args = operation
        .get_mut("args")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_GEOMETRY_BINDING_MISSING",
                "ShapeProgram operation has no bounded argument object.",
            )
        })?;
    if args.get("zone_id").and_then(Value::as_str) != Some(zone_id) {
        return Err(NativeBlockoutCompatError::conflict(
            "CHANGE_SET_MATERIAL_BINDING_AMBIGUOUS",
            "Material Zone does not map to one sealed ShapeProgram output.",
        ));
    }
    args.insert("material_id".into(), Value::String(material_id.to_string()));
    preview.material_bindings.insert(
        format!("{part_id}:{zone_id}"),
        Value::String(material_id.to_string()),
    );
    Ok(())
}

fn native_change_set_base_material(
    version: &AgentAssetVersion,
    part_id: &str,
    zone_id: &str,
) -> Result<String, NativeBlockoutCompatError> {
    if let Some(program) = native_surface_adornment_programs(version)?
        .into_iter()
        .find(|program| program.target_part_id == part_id && program.target_zone_id == zone_id)
    {
        return Ok(program.base_material);
    }
    if let Some(material_id) = version
        .material_bindings
        .get(&format!("{part_id}:{zone_id}"))
        .and_then(Value::as_str)
    {
        return Ok(material_id.to_string());
    }
    let operation_id = native_change_set_part_operation_id(version, part_id)?;
    version
        .shape_program
        .get("operations")
        .and_then(Value::as_array)
        .and_then(|operations| {
            operations.iter().find(|operation| {
                operation.get("operation_id").and_then(Value::as_str) == Some(operation_id.as_str())
            })
        })
        .and_then(|operation| operation.get("args"))
        .and_then(|args| args.get("material_id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "SURFACE_ADORNMENT_BASE_MATERIAL_MISSING",
                "Selected Material Zone has no committed visual base material.",
            )
        })
}

fn native_change_set_apply_surface_adornment(
    preview: &mut AgentAssetVersion,
    program: &SurfaceAdornmentProgram,
    material_id: &str,
) -> Result<(), NativeBlockoutCompatError> {
    let graph = preview.assembly_graph.as_object_mut().ok_or_else(|| {
        NativeBlockoutCompatError::conflict(
            "ASSEMBLY_GRAPH_INVALID",
            "Surface appearance requires an editable AssemblyGraph.",
        )
    })?;
    let adornments = graph
        .entry("surface_adornments")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "SURFACE_ADORNMENT_PROVENANCE_INVALID",
                "AssemblyGraph surface appearance provenance is malformed.",
            )
        })?;
    let serialized = serde_json::to_value(program).map_err(|_| {
        NativeBlockoutCompatError::unavailable(
            "SURFACE_ADORNMENT_PROVENANCE_INVALID",
            "Surface appearance provenance could not be sealed.",
        )
    })?;
    if let Some(existing) = adornments.iter_mut().find(|existing| {
        existing.get("target_part_id").and_then(Value::as_str)
            == Some(program.target_part_id.as_str())
            && existing.get("target_zone_id").and_then(Value::as_str)
                == Some(program.target_zone_id.as_str())
    }) {
        *existing = serialized;
    } else {
        if adornments.len() >= 8 {
            return Err(NativeBlockoutCompatError::conflict(
                "SURFACE_ADORNMENT_LIMIT_EXCEEDED",
                "Asset already contains the maximum number of reviewed surface appearances.",
            ));
        }
        adornments.push(serialized);
    }
    preview.material_bindings.insert(
        format!("{}:{}", program.target_part_id, program.target_zone_id),
        Value::String(material_id.to_string()),
    );
    Ok(())
}

fn native_change_set_clear_surface_adornment(
    preview: &mut AgentAssetVersion,
    part_id: &str,
    zone_id: &str,
) -> Result<(), NativeBlockoutCompatError> {
    let Some(adornments) = preview
        .assembly_graph
        .get_mut("surface_adornments")
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };
    adornments.retain(|existing| {
        existing.get("target_part_id").and_then(Value::as_str) != Some(part_id)
            || existing.get("target_zone_id").and_then(Value::as_str) != Some(zone_id)
    });
    Ok(())
}

fn native_change_set_apply_joint(
    preview: &mut AgentAssetVersion,
    part_id: &str,
    operation_id: &str,
    rotation: [f64; 3],
) -> Result<(), NativeBlockoutCompatError> {
    let graph_part = native_change_set_graph_part_mut(preview, part_id)?;
    if graph_part
        .get("joints")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        return Err(NativeBlockoutCompatError::not_found(
            "JOINT_NOT_FOUND",
            "Target part has no sealed concept joint.",
        ));
    }
    let transform = graph_part
        .entry("transform")
        .or_insert_with(|| json!({"position":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]}))
        .as_object_mut()
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "AssemblyGraph transform is invalid.",
            )
        })?;
    transform.insert("rotation".into(), json!(rotation));
    let args = native_change_set_shape_operation_mut(preview, operation_id)?
        .get_mut("args")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_GEOMETRY_BINDING_MISSING",
                "ShapeProgram operation has no bounded argument object.",
            )
        })?;
    args.insert("rotation".into(), json!(rotation));
    Ok(())
}

fn native_change_set_snap_part(
    preview: &mut AgentAssetVersion,
    part_id: &str,
    operation_id: &str,
    connector_id: &str,
    target_part_id: &str,
    target_connector_id: &str,
) -> Result<(), NativeBlockoutCompatError> {
    let graph_parts = preview
        .assembly_graph
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "AssemblyGraph has no part table.",
            )
        })?;
    let source = graph_parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .cloned()
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found("PART_NOT_FOUND", "Source part is missing.")
        })?;
    let target = graph_parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(target_part_id))
        .cloned()
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found("PART_NOT_FOUND", "Target part is missing.")
        })?;
    let connector = native_change_set_connector(&source, connector_id)?;
    let target_connector = native_change_set_connector(&target, target_connector_id)?;
    if connector.get("kind").and_then(Value::as_str)
        != target_connector.get("kind").and_then(Value::as_str)
    {
        return Err(NativeBlockoutCompatError::conflict(
            "CONNECTOR_INCOMPATIBLE",
            "Source and target connector kinds are incompatible.",
        ));
    }
    let source_offset = native_change_set_vec3(
        connector.get("position"),
        -100_000.0,
        100_000.0,
        "connector position",
    )?;
    let target_offset = native_change_set_vec3(
        target_connector.get("position"),
        -100_000.0,
        100_000.0,
        "connector position",
    )?;
    let target_position = native_change_set_vec3(
        target.pointer("/transform/position"),
        -100_000.0,
        100_000.0,
        "target position",
    )?;
    let position = [
        target_position[0] + target_offset[0] - source_offset[0],
        target_position[1] + target_offset[1] - source_offset[1],
        target_position[2] + target_offset[2] - source_offset[2],
    ];
    let (_, size, _) = native_change_set_base_part_facts(preview, part_id)?;
    native_change_set_update_part(preview, part_id, position, Some(size))?;
    let graph_part = native_change_set_graph_part_mut(preview, part_id)?;
    let transform = graph_part
        .entry("transform")
        .or_insert_with(|| json!({"position":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]}))
        .as_object_mut()
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_ASSEMBLY_BINDING_MISSING",
                "AssemblyGraph transform is invalid.",
            )
        })?;
    transform.insert("position".into(), json!(position));
    native_change_set_shape_operation_mut(preview, operation_id)?
        .get_mut("args")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "CHANGE_SET_GEOMETRY_BINDING_MISSING",
                "ShapeProgram operation has no bounded argument object.",
            )
        })?
        .insert("position".into(), json!(position));
    Ok(())
}

fn native_change_set_connector<'a>(
    graph_part: &'a Value,
    connector_id: &str,
) -> Result<&'a serde_json::Map<String, Value>, NativeBlockoutCompatError> {
    graph_part
        .get("connectors")
        .and_then(Value::as_array)
        .and_then(|connectors| {
            connectors.iter().find(|connector| {
                connector.get("connector_id").and_then(Value::as_str) == Some(connector_id)
            })
        })
        .and_then(Value::as_object)
        .ok_or_else(|| {
            NativeBlockoutCompatError::not_found(
                "CONNECTOR_NOT_FOUND",
                "Requested sealed AssemblyGraph connector is unavailable.",
            )
        })
}

fn native_blockout_json_response(status: u16, value: Value) -> CompatHttpResponse {
    CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
        status,
        headers: vec![
            ("Content-Type".into(), "application/json".into()),
            ("Cache-Control".into(), "no-store".into()),
        ],
        body: ProtocolHttpBody::Utf8 {
            data: value.to_string(),
        },
    }
}

fn parse_native_single_result_route(
    request: &PreparedCompatHttpRequest,
) -> Option<NativeSingleResultRoute> {
    let route = request.path.split('?').next().unwrap_or(&request.path);
    let segments = route.trim_start_matches('/').split('/').collect::<Vec<_>>();
    let ["api", "v1", "agent", "projects", project_id, "turns", turn_id, "single-results", leaf] =
        segments.as_slice()
    else {
        return None;
    };
    let (preview_id, action, expected_method) =
        if let Some(value) = leaf.strip_suffix(":preview.glb") {
            (
                value,
                NativeSingleResultAction::PreviewGlb,
                AllowedHttpMethod::Get,
            )
        } else if let Some(value) = leaf.strip_suffix(":confirm") {
            (
                value,
                NativeSingleResultAction::Confirm,
                AllowedHttpMethod::Post,
            )
        } else if let Some(value) = leaf.strip_suffix(":reject") {
            (
                value,
                NativeSingleResultAction::Reject,
                AllowedHttpMethod::Post,
            )
        } else {
            return None;
        };
    if request.method != expected_method
        || !valid_stable_id(project_id)
        || !valid_stable_id(turn_id)
        || !preview_id.starts_with("preview_")
        || !valid_stable_id(preview_id)
    {
        return None;
    }
    Some(NativeSingleResultRoute {
        project_id: (*project_id).to_string(),
        turn_id: (*turn_id).to_string(),
        preview_id: preview_id.to_string(),
        action,
    })
}

fn native_single_result_if_match(
    request: &PreparedCompatHttpRequest,
) -> Result<String, NativeBlockoutCompatError> {
    let value = request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("if-match"))
        .map(|(_, value)| value.as_str())
        .ok_or_else(|| {
            NativeBlockoutCompatError::conflict(
                "SINGLE_RESULT_PRECONDITION_REQUIRED",
                "Formal preview access requires the decision GLB hash in If-Match.",
            )
        })?;
    let hash = value
        .strip_prefix("\"sha256:")
        .and_then(|value| value.strip_suffix('\"'))
        .ok_or_else(|| {
            NativeBlockoutCompatError::invalid(
                "SINGLE_RESULT_PRECONDITION_INVALID",
                "If-Match must use the exact quoted sha256 ETag form.",
            )
        })?;
    native_single_result_validate_sha256(hash)?;
    Ok(hash.to_ascii_lowercase())
}

fn native_single_result_validate_sha256(value: &str) -> Result<(), NativeBlockoutCompatError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(NativeBlockoutCompatError::invalid(
            "SINGLE_RESULT_HASH_INVALID",
            "Formal preview GLB identity must be one lowercase SHA-256 value.",
        ));
    }
    Ok(())
}

fn native_single_result_artifact_id(
    project_id: &str,
    turn_id: &str,
    preview_id: &str,
    artifact_sha256: &str,
) -> String {
    format!(
        "artifact_v003_{}",
        &sha256_hex(format!("{project_id}:{turn_id}:{preview_id}:{artifact_sha256}").as_bytes())
            [..24]
    )
}

fn native_single_result_glb_response(
    project_id: &str,
    turn_id: &str,
    artifact: &NativePreviewArtifact,
) -> Result<CompatHttpResponse, NativeBlockoutCompatError> {
    artifact
        .validate()
        .map_err(native_blockout_product_tool_error)?;
    Ok(CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
        status: 200,
        headers: vec![
            ("Content-Type".into(), "model/gltf-binary".into()),
            ("Cache-Control".into(), "no-store".into()),
            (
                "Content-Disposition".into(),
                format!("inline; filename=\"{}-preview.glb\"", artifact.preview_id),
            ),
            ("ETag".into(), format!("\"sha256:{}\"", artifact.glb_sha256)),
            ("X-ForgeCAD-Project-ID".into(), project_id.into()),
            ("X-ForgeCAD-Turn-ID".into(), turn_id.into()),
            ("X-ForgeCAD-Preview-ID".into(), artifact.preview_id.clone()),
            (
                "X-ForgeCAD-Artifact-Profile".into(),
                artifact.readback.artifact_profile_id.clone(),
            ),
            (
                "X-ForgeCAD-Shape-Program-SHA256".into(),
                artifact.readback.shape_program_sha256.clone(),
            ),
            ("X-ForgeCAD-GLB-SHA256".into(), artifact.glb_sha256.clone()),
            (
                "X-ForgeCAD-GLB-Byte-Size".into(),
                artifact.glb_bytes.len().to_string(),
            ),
            (
                "X-ForgeCAD-Triangle-Count".into(),
                artifact.readback.triangle_count.to_string(),
            ),
        ],
        body: ProtocolHttpBody::Base64 {
            data: BASE64.encode(&artifact.glb_bytes),
        },
    })
}

fn native_blockout_error_response(error: NativeBlockoutCompatError) -> CompatHttpResponse {
    native_blockout_json_response(
        error.status,
        json!({
            "error": {
                "code": error.code,
                "message": error.message,
                "recoverable": error.recoverable,
                "details": {}
            }
        }),
    )
}

fn native_blockout_product_tool_error(error: ProductToolPortError) -> NativeBlockoutCompatError {
    match error.kind {
        ProductToolPortErrorKind::Cancelled => native_blockout_cancelled(),
        ProductToolPortErrorKind::InvalidResponse => {
            NativeBlockoutCompatError::invalid(error.code, error.message)
        }
        ProductToolPortErrorKind::Unavailable | ProductToolPortErrorKind::Timeout => {
            NativeBlockoutCompatError::unavailable(error.code, error.message)
        }
    }
}

fn native_blockout_restricted_geometry_error(
    error: RestrictedGeometryError,
) -> NativeBlockoutCompatError {
    match error.kind {
        RestrictedGeometryErrorKind::Cancelled => native_blockout_cancelled(),
        RestrictedGeometryErrorKind::InvalidInput | RestrictedGeometryErrorKind::Unsupported => {
            NativeBlockoutCompatError::invalid(error.code, error.message)
        }
        RestrictedGeometryErrorKind::Timeout | RestrictedGeometryErrorKind::Execution => {
            NativeBlockoutCompatError::unavailable(error.code, error.message)
        }
    }
}

fn native_blockout_core_error(error: CoreError) -> NativeBlockoutCompatError {
    let code = error.code().to_string();
    let message = error.to_string();
    match error {
        CoreError::InvalidData { .. } => NativeBlockoutCompatError::invalid(code, message),
        CoreError::Conflict { .. } | CoreError::ConflictWithDetails { .. } => {
            NativeBlockoutCompatError::conflict(code, message)
        }
        CoreError::NotFound { .. } => NativeBlockoutCompatError::not_found(code, message),
        CoreError::Sqlite(_) | CoreError::Io(_) | CoreError::Migration { .. } => {
            NativeBlockoutCompatError::unavailable(code, message)
        }
    }
}

fn native_blockout_state_unavailable() -> NativeBlockoutCompatError {
    NativeBlockoutCompatError::unavailable(
        "NATIVE_BLOCKOUT_STATE_UNAVAILABLE",
        "Rust transient blockout state is unavailable.",
    )
}

fn native_blockout_cancelled() -> NativeBlockoutCompatError {
    NativeBlockoutCompatError {
        status: 409,
        code: "REQUEST_CANCELLED".into(),
        message: "Rust blockout request was cancelled before confirmation.".into(),
        recoverable: true,
    }
}

fn native_blockout_now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

impl CompatibilityHttpPort for LoopbackHttpPort {
    fn execute(
        &self,
        request: PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> CompatHttpFuture {
        let client = self.inner.client.clone();
        let rust_core = self.inner.rust_core.clone();
        let port = self.clone();
        Box::pin(async move {
            if let Some(rust_core) = rust_core {
                if let Some(response) = rust_catalog_response(&request) {
                    return response;
                }
                if let Some(response) = port
                    .handle_native_asset_render_compat(&request, cancellation.clone())
                    .await
                {
                    return response;
                }
                if let Some(response) = port
                    .handle_native_single_result_compat(&request, cancellation.clone())
                    .await
                {
                    return response;
                }
                if let Some(response) = port
                    .handle_native_blockout_compat(&request, cancellation.clone())
                    .await
                {
                    return response;
                }
                if let Some(response) = port
                    .handle_native_change_set_compat(&request, cancellation.clone())
                    .await
                {
                    return response;
                }
                if let Some(response) = rust_core.handle_compat_http(&request) {
                    return response;
                }
                // The production Python process owns no product read or write
                // route after K003. Only its exact health observation may
                // cross this compatibility port; internal geometry uses the
                // separate capability-gated port.
                if !is_python_sidecar_observation_route(&request) {
                    return Ok(rust_owned_product_route_response());
                }
            }
            let url = format!("{}{}", request.endpoint.origin(), request.path);
            let method = reqwest::Method::from_bytes(request.method.as_str().as_bytes())
                .map_err(|_| RpcError::invalid_params("compat/http method is invalid."))?;
            let mut builder = client.request(method, url);
            for (name, value) in request.headers {
                builder = builder.header(name, value);
            }
            let body = decode_protocol_body(request.body)?;
            if !body.is_empty() {
                builder = builder.body(body);
            }
            let mut response =
                compatibility_cancellation_aware(builder.send(), cancellation.clone())
                    .await?
                    .map_err(backend_unavailable)?;
            if response
                .content_length()
                .is_some_and(|length| length > MAX_RAW_COMPAT_BODY_BYTES as u64)
            {
                return Err(response_too_large());
            }
            let status = response.status().as_u16();
            let headers = filtered_response_headers(response.headers());
            let mut body = Vec::new();
            loop {
                let chunk =
                    compatibility_cancellation_aware(response.chunk(), cancellation.clone())
                        .await?
                        .map_err(backend_unavailable)?;
                let Some(chunk) = chunk else { break };
                if body.len().saturating_add(chunk.len()) > MAX_RAW_COMPAT_BODY_BYTES {
                    return Err(response_too_large());
                }
                body.extend_from_slice(&chunk);
            }
            Ok(CompatHttpResponse {
                schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.to_string(),
                status,
                headers,
                body: encode_protocol_body(body),
            })
        })
    }

    fn subscribe(
        &self,
        params: SseSubscriptionParams,
        cancellation: CancellationToken,
    ) -> HandlerFuture {
        if self.inner.rust_core.is_some() {
            return Box::pin(async { Err(rust_owned_python_sse_retired(METHOD_COMPAT_SUBSCRIBE)) });
        }
        let connection_id = active_connection_id();
        let port = self.clone();
        Box::pin(async move {
            let connection_id = connection_id?;
            {
                let mut subscriptions =
                    port.inner.subscriptions.lock().map_err(|_| {
                        RpcError::internal("SSE subscription registry is unavailable.")
                    })?;
                if subscriptions.contains_key(&params.stream_id) {
                    return Err(RpcError::invalid_params("SSE stream_id is already active."));
                }
                subscriptions.insert(
                    params.stream_id.clone(),
                    SubscriptionHandle {
                        connection_id: connection_id.clone(),
                        cancellation: cancellation.clone(),
                        task: None,
                    },
                );
            }
            let initial_response = match port.open_sse(&params.path, None).await {
                Ok(response) => response,
                Err(error) => {
                    if let Ok(mut subscriptions) = port.inner.subscriptions.lock() {
                        subscriptions.remove(&params.stream_id);
                    }
                    return Err(error);
                }
            };
            let stream_id = params.stream_id.clone();
            let task_port = port.clone();
            let task_connection_id = connection_id.clone();
            let task_path = params.path.clone();
            let task_cancellation = cancellation.clone();
            let task = tauri::async_runtime::spawn(async move {
                let result = run_sse_subscription(
                    task_port.clone(),
                    task_connection_id.clone(),
                    stream_id.clone(),
                    task_path,
                    task_cancellation,
                    initial_response,
                )
                .await;
                if let Err(error) = result {
                    if let Ok(server) = task_port.server() {
                        let _ = server.publish_resync_required(
                            &task_connection_id,
                            error.data.application_code.to_ascii_lowercase(),
                        );
                    }
                    eprintln!(
                        "ForgeCAD SSE bridge stopped code={}: {}",
                        error.data.application_code, error.message
                    );
                }
                if let Ok(mut subscriptions) = task_port.inner.subscriptions.lock() {
                    if subscriptions
                        .get(&stream_id)
                        .is_some_and(|handle| handle.connection_id == task_connection_id)
                    {
                        subscriptions.remove(&stream_id);
                    }
                }
            });
            let mut subscriptions = port
                .inner
                .subscriptions
                .lock()
                .map_err(|_| RpcError::internal("SSE subscription registry is unavailable."))?;
            if let Some(handle) = subscriptions.get_mut(&params.stream_id) {
                handle.task = Some(task);
            } else {
                cancellation.cancel();
                task.abort();
                return Err(RpcError::invalid_params(
                    "SSE subscription was cancelled before it opened.",
                ));
            }
            Ok(json!({
                "schema_version": "ForgeCADSseSubscriptionResult@1",
                "stream_id": params.stream_id,
                "subscribed": true
            }))
        })
    }

    fn unsubscribe(
        &self,
        params: SseUnsubscribeParams,
        _cancellation: CancellationToken,
    ) -> HandlerFuture {
        let connection_id = active_connection_id();
        let port = self.clone();
        Box::pin(async move {
            let connection_id = connection_id?;
            let removed = {
                let mut subscriptions =
                    port.inner.subscriptions.lock().map_err(|_| {
                        RpcError::internal("SSE subscription registry is unavailable.")
                    })?;
                match subscriptions.get(&params.stream_id) {
                    Some(handle) if handle.connection_id != connection_id => {
                        return Err(RpcError::invalid_params(
                            "SSE stream belongs to another connection.",
                        ));
                    }
                    Some(_) => subscriptions.remove(&params.stream_id),
                    None => None,
                }
            };
            let unsubscribed = removed.is_some();
            if let Some(handle) = removed {
                handle.cancellation.cancel();
                if let Some(task) = handle.task {
                    task.abort();
                }
            }
            Ok(json!({
                "schema_version": "ForgeCADSseUnsubscribeResult@1",
                "stream_id": params.stream_id,
                "unsubscribed": unsubscribed
            }))
        })
    }

    fn replay(&self, params: ReplayParams, cancellation: CancellationToken) -> HandlerFuture {
        if self.inner.rust_core.is_some() {
            return Box::pin(async { Err(rust_owned_python_sse_retired(METHOD_EVENTS_REPLAY)) });
        }
        let connection_id = active_connection_id();
        let port = self.clone();
        Box::pin(async move {
            let connection_id = connection_id?;
            let cursor = AppServerCursor::decode(&params.cursor)?;
            let path = format!(
                "/api/v1/agent/threads/{}/events?after={}",
                cursor.thread_id, cursor.source_sequence
            );
            let response =
                compatibility_cancellation_aware(port.open_sse(&path, None), cancellation.clone())
                    .await??;
            let events =
                compatibility_cancellation_aware(read_sse_response(response), cancellation)
                    .await??;
            let stream_suffix = params.cursor.chars().take(48).collect::<String>();
            let stream_id = format!("replay_{stream_suffix}");
            let server = port.server()?;
            let mut notifications = Vec::with_capacity(events.len());
            for event in events {
                let (_, notification) =
                    publish_sse_event(&server, &connection_id, &stream_id, event)?;
                notifications.push(notification);
            }
            serde_json::to_value(forgecad_app_server_protocol::ReplayResult { notifications })
                .map_err(|error| {
                    RpcError::internal(format!("Persistent replay serialization failed: {error}"))
                })
        })
    }
}

async fn run_sse_subscription(
    port: LoopbackHttpPort,
    connection_id: String,
    stream_id: String,
    path: String,
    cancellation: CancellationToken,
    initial_response: reqwest::Response,
) -> Result<(), RpcError> {
    let server = port.server()?;
    let mut response = Some(initial_response);
    let mut last_event_id: Option<String> = None;
    loop {
        if cancellation.is_cancelled() {
            return Ok(());
        }
        let current = match response.take() {
            Some(response) => response,
            None => port.open_sse(&path, last_event_id.as_deref()).await?,
        };
        let events = read_sse_response(current).await?;
        for event in events {
            if cancellation.is_cancelled() {
                return Ok(());
            }
            let (event_id, _) = publish_sse_event(&server, &connection_id, &stream_id, event)?;
            if event_id.is_some() {
                last_event_id = event_id;
            }
        }
        tokio::time::sleep(SSE_RECONNECT_DELAY).await;
    }
}

fn publish_sse_event(
    server: &AppServer,
    connection_id: &str,
    stream_id: &str,
    event: ParsedSseEvent,
) -> Result<
    (
        Option<String>,
        forgecad_app_server_protocol::ServerNotification,
    ),
    RpcError,
> {
    if contains_forbidden_reasoning(&event.data) {
        return Err(malformed_upstream(
            "Hidden reasoning content must not cross the app-server protocol.",
        ));
    }
    let event_id = event.id.clone();
    let persisted_cursor = (event.event == "agent.item")
        .then(|| agent_event_cursor(&event))
        .transpose()?;
    let params = SseNotificationParams {
        schema_version: SSE_NOTIFICATION_SCHEMA_VERSION.to_string(),
        stream_id: stream_id.to_string(),
        event: event.event,
        data: event.data,
        id: event.id,
    };
    let serialized = serde_json::to_value(params).map_err(|error| {
        RpcError::internal(format!("SSE notification serialization failed: {error}"))
    })?;
    let notification = if let Some(cursor) = persisted_cursor {
        // Persisted Agent Items are the only K001 compatibility events with
        // an authoritative replay cursor.
        server.publish_notification(connection_id, METHOD_COMPAT_SSE, cursor, serialized)?
    } else {
        // Replay-complete and legacy job events remain observable and bounded,
        // but intentionally carry no fabricated cursor.
        server.publish_transient_notification(connection_id, METHOD_COMPAT_SSE, serialized)?
    };
    Ok((event_id, notification))
}

#[derive(Debug, Deserialize)]
struct PersistedAgentEvent {
    sequence: u64,
    thread_id: String,
    turn_id: String,
    item: PersistedAgentItemIdentity,
}

#[derive(Debug, Deserialize)]
struct PersistedAgentItemIdentity {
    item_id: String,
    thread_id: String,
    turn_id: String,
    sequence: u64,
}

fn agent_event_cursor(event: &ParsedSseEvent) -> Result<AppServerCursor, RpcError> {
    if contains_forbidden_reasoning(&event.data) {
        return Err(malformed_upstream(
            "Hidden reasoning content must not cross the app-server protocol.",
        ));
    }
    let persisted: PersistedAgentEvent = serde_json::from_str(&event.data)
        .map_err(|_| malformed_upstream("The upstream Agent Item does not match AgentEvent."))?;
    let event_id = event
        .id
        .as_deref()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| malformed_upstream("The upstream Agent Item id is not its sequence."))?;
    if event_id != persisted.sequence
        || persisted.sequence != persisted.item.sequence
        || persisted.thread_id != persisted.item.thread_id
        || persisted.turn_id != persisted.item.turn_id
        || !valid_stable_id(&persisted.thread_id)
        || !valid_stable_id(&persisted.turn_id)
        || !valid_stable_id(&persisted.item.item_id)
    {
        return Err(malformed_upstream(
            "The upstream Agent Item identity or sequence is inconsistent.",
        ));
    }
    Ok(AppServerCursor::new(
        persisted.thread_id,
        Some(persisted.turn_id),
        persisted.sequence,
        CursorPhase::Item,
        Some(persisted.item.item_id),
    ))
}

struct ConnectionScopedFuture<F> {
    connection_id: String,
    future: Pin<Box<F>>,
}

impl<F> ConnectionScopedFuture<F> {
    fn new(connection_id: String, future: F) -> Self {
        Self {
            connection_id,
            future: Box::pin(future),
        }
    }
}

impl<F: Future> Future for ConnectionScopedFuture<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let previous = ACTIVE_CONNECTION_ID
            .with(|active| active.borrow_mut().replace(self.connection_id.clone()));
        let result = self.future.as_mut().poll(context);
        ACTIVE_CONNECTION_ID.with(|active| *active.borrow_mut() = previous);
        result
    }
}

fn active_connection_id() -> Result<String, RpcError> {
    ACTIVE_CONNECTION_ID
        .with(|active| active.borrow().clone())
        .ok_or_else(|| RpcError::internal("SSE method has no active connection context."))
}

fn persisted_turn_cancellation_target(
    method: &str,
    path: &str,
    body: &[u8],
) -> Option<PersistedTurnCancellationTarget> {
    if method != "POST" || path.contains('?') {
        return None;
    }
    let thread_id = path
        .strip_prefix("/api/v1/agent/threads/")?
        .strip_suffix("/turns")?;
    if !valid_stable_id(thread_id) || thread_id.contains('/') {
        return None;
    }
    let request = serde_json::from_slice::<Value>(body).ok()?;
    let request_text = request.get("message")?.as_str()?;
    if request_text.is_empty() || request_text.len() > 8000 {
        return None;
    }
    Some(PersistedTurnCancellationTarget {
        thread_id: thread_id.to_string(),
        request_text: request_text.to_string(),
        preexisting_turn_ids: Arc::new(Mutex::new(None)),
    })
}

fn protocol_turn_cancellation_registration(
    frame: &Value,
) -> Option<(String, PersistedTurnCancellationTarget)> {
    if frame.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || frame.get("method").and_then(Value::as_str) != Some("compat/http")
    {
        return None;
    }
    let request_id = frame.get("id")?.as_str()?;
    if !valid_stable_id(request_id) {
        return None;
    }
    let params = frame.get("params")?;
    if params.get("schema_version").and_then(Value::as_str)
        != Some(HTTP_COMPAT_REQUEST_SCHEMA_VERSION)
    {
        return None;
    }
    let method = params.get("method")?.as_str()?;
    let path = params.get("path")?.as_str()?;
    let body = serde_json::from_value::<ProtocolHttpBody>(params.get("body")?.clone()).ok()?;
    let body = decode_protocol_body(body).ok()?;
    persisted_turn_cancellation_target(method, path, &body)
        .map(|target| (request_id.to_string(), target))
}

fn protocol_cancel_notification(frame: &Value) -> Option<(String, String)> {
    if frame.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || frame.get("method").and_then(Value::as_str) != Some("request/cancel")
        || frame.get("id").is_some()
    {
        return None;
    }
    let params = frame.get("params")?.as_object()?;
    if params.len() != 2 {
        return None;
    }
    let request_id = params.get("request_id")?.as_str()?;
    let cancel_token = params.get("cancel_token")?.as_str()?;
    if !valid_stable_id(request_id) || cancel_token != request_id {
        return None;
    }
    Some((request_id.to_string(), cancel_token.to_string()))
}

fn persisted_thread_turn_ids(thread: &Value, thread_id: &str) -> Result<Vec<String>, RpcError> {
    if thread.get("thread_id").and_then(Value::as_str) != Some(thread_id) {
        return Err(malformed_upstream(
            "The persisted Turn baseline returned another Thread.",
        ));
    }
    let turns = thread
        .get("turns")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            malformed_upstream("The persisted Turn baseline did not contain ordered Turns.")
        })?;
    let mut turn_ids = Vec::with_capacity(turns.len());
    for turn in turns {
        if turn.get("thread_id").and_then(Value::as_str) != Some(thread_id) {
            return Err(malformed_upstream(
                "The persisted Turn baseline crossed Thread identity.",
            ));
        }
        let turn_id = turn
            .get("turn_id")
            .and_then(Value::as_str)
            .filter(|turn_id| valid_stable_id(turn_id))
            .ok_or_else(|| {
                malformed_upstream("The persisted Turn baseline contained an invalid Turn ID.")
            })?;
        turn_ids.push(turn_id.to_string());
    }
    Ok(turn_ids)
}

fn persisted_turn_state(
    thread: &Value,
    target: &PersistedTurnCancellationTarget,
    exact_turn_id: Option<&str>,
) -> Result<PersistedTurnCancellationState, RpcError> {
    if thread.get("thread_id").and_then(Value::as_str) != Some(target.thread_id.as_str()) {
        return Err(malformed_upstream(
            "The persisted Turn cancellation readback returned another Thread.",
        ));
    }
    let turns = thread
        .get("turns")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            malformed_upstream(
                "The persisted Turn cancellation readback did not contain ordered Turns.",
            )
        })?;
    let preexisting_turn_ids = if exact_turn_id.is_none() {
        target.preexisting_turn_ids().ok_or_else(|| {
            persisted_cancel_error(
                "The persisted Turn cancellation baseline was unavailable; cancellation failed closed.",
            )
        })?
    } else {
        Vec::new()
    };
    for turn in turns.iter().rev() {
        let Some(turn_id) = turn.get("turn_id").and_then(Value::as_str) else {
            return Err(malformed_upstream(
                "The persisted Turn cancellation readback contained a Turn without an ID.",
            ));
        };
        if !valid_stable_id(turn_id) {
            return Err(malformed_upstream(
                "The persisted Turn cancellation readback contained an invalid Turn ID.",
            ));
        }
        if let Some(expected) = exact_turn_id {
            if turn_id != expected {
                continue;
            }
        } else {
            if preexisting_turn_ids
                .iter()
                .any(|candidate| candidate == turn_id)
            {
                continue;
            }
            if turn.get("request_text").and_then(Value::as_str)
                != Some(target.request_text.as_str())
            {
                continue;
            }
        }
        if turn.get("thread_id").and_then(Value::as_str) != Some(target.thread_id.as_str()) {
            return Err(malformed_upstream(
                "The persisted Turn cancellation readback crossed Thread identity.",
            ));
        }
        let status = turn.get("status").and_then(Value::as_str).ok_or_else(|| {
            malformed_upstream(
                "The persisted Turn cancellation readback contained a Turn without status.",
            )
        })?;
        return Ok(match status {
            "queued" | "running" => PersistedTurnCancellationState::Running(turn_id.to_string()),
            "cancelled" => PersistedTurnCancellationState::Cancelled(turn_id.to_string()),
            _ => PersistedTurnCancellationState::Terminal {
                turn_id: turn_id.to_string(),
                status: status.to_string(),
            },
        });
    }
    Ok(PersistedTurnCancellationState::Missing)
}

async fn read_bounded_json_response(response: &mut reqwest::Response) -> Result<Value, RpcError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RAW_COMPAT_BODY_BYTES as u64)
    {
        return Err(response_too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(backend_unavailable)? {
        if body.len().saturating_add(chunk.len()) > MAX_RAW_COMPAT_BODY_BYTES {
            return Err(response_too_large());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| {
        malformed_upstream("The persisted Turn cancellation response was not valid JSON.")
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSseEvent {
    id: Option<String>,
    event: String,
    data: String,
}

async fn read_sse_response(
    mut response: reqwest::Response,
) -> Result<Vec<ParsedSseEvent>, RpcError> {
    let mut pending = Vec::new();
    let mut events = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(backend_unavailable)? {
        if pending.len().saturating_add(chunk.len()) > MAX_SSE_EVENT_BYTES {
            return Err(malformed_upstream(
                "SSE event exceeded the bounded byte limit.",
            ));
        }
        pending.extend_from_slice(&chunk);
        while let Some((index, boundary_length)) = sse_boundary(&pending) {
            let block = pending.drain(..index).collect::<Vec<_>>();
            pending.drain(..boundary_length);
            if let Some(event) = parse_sse_block(&block)? {
                events.push(event);
            }
        }
    }
    if !pending.is_empty() {
        if let Some(event) = parse_sse_block(&pending)? {
            events.push(event);
        }
    }
    Ok(events)
}

fn parse_sse_block(block: &[u8]) -> Result<Option<ParsedSseEvent>, RpcError> {
    let text = str::from_utf8(block)
        .map_err(|_| malformed_upstream("The upstream SSE stream is not UTF-8."))?;
    let mut id = None;
    let mut event = "message".to_string();
    let mut data = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.starts_with(':') {
            continue;
        }
        let (field, value) = line
            .split_once(':')
            .map(|(field, value)| (field, value.strip_prefix(' ').unwrap_or(value)))
            .unwrap_or((line, ""));
        match field {
            "id" => {
                if value.contains('\0') || value.len() > 4096 {
                    return Err(malformed_upstream("The upstream SSE id is malformed."));
                }
                id = Some(value.to_string());
            }
            "event" => event = if value.is_empty() { "message" } else { value }.to_string(),
            "data" => data.push(value.to_string()),
            _ => {}
        }
    }
    if id.is_none() && event == "message" && data.is_empty() {
        return Ok(None);
    }
    Ok(Some(ParsedSseEvent {
        id,
        event,
        data: data.join("\n"),
    }))
}

fn sse_boundary(value: &[u8]) -> Option<(usize, usize)> {
    let line_feed = value.windows(2).position(|pair| pair == b"\n\n");
    let carriage_return = value.windows(4).position(|pair| pair == b"\r\n\r\n");
    match (line_feed, carriage_return) {
        (None, None) => None,
        (Some(index), None) => Some((index, 2)),
        (None, Some(index)) => Some((index, 4)),
        (Some(left), Some(right)) if left <= right => Some((left, 2)),
        (Some(_), Some(right)) => Some((right, 4)),
    }
}

fn contains_forbidden_reasoning(data: &str) -> bool {
    serde_json::from_str::<Value>(data)
        .ok()
        .is_some_and(|value| value_contains_key(&value, "reasoning_content"))
}

fn value_contains_key(value: &Value, forbidden: &str) -> bool {
    match value {
        Value::Object(object) => object
            .iter()
            .any(|(key, value)| key == forbidden || value_contains_key(value, forbidden)),
        Value::Array(values) => values
            .iter()
            .any(|value| value_contains_key(value, forbidden)),
        _ => false,
    }
}

fn decode_protocol_body(body: ProtocolHttpBody) -> Result<Vec<u8>, RpcError> {
    match body {
        ProtocolHttpBody::Empty => Ok(Vec::new()),
        ProtocolHttpBody::Utf8 { data } => Ok(data.into_bytes()),
        ProtocolHttpBody::Base64 { data } => BASE64
            .decode(data)
            .map_err(|_| RpcError::invalid_params("compat/http body is not valid base64.")),
    }
}

fn encode_protocol_body(body: Vec<u8>) -> ProtocolHttpBody {
    if body.is_empty() {
        ProtocolHttpBody::Empty
    } else if let Ok(text) = String::from_utf8(body.clone()) {
        ProtocolHttpBody::Utf8 { data: text }
    } else {
        ProtocolHttpBody::Base64 {
            data: BASE64.encode(body),
        }
    }
}

fn filtered_response_headers(headers: &reqwest::header::HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            let name = name.as_str().to_ascii_lowercase();
            let allowed = matches!(
                name.as_str(),
                "accept-ranges"
                    | "cache-control"
                    | "content-disposition"
                    | "content-length"
                    | "content-range"
                    | "content-type"
                    | "etag"
                    | "last-modified"
            ) || name.starts_with("x-forgecad-");
            (allowed && value.as_bytes().len() <= 8192)
                .then(|| value.to_str().ok().map(|value| (name, value.to_string())))
                .flatten()
        })
        .take(64)
        .collect()
}

fn compat_to_resource_response(response: CompatHttpResponse) -> http::Response<Vec<u8>> {
    let status = http::StatusCode::from_u16(response.status)
        .unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR);
    let body = match decode_protocol_body(response.body) {
        Ok(body) => body,
        Err(error) => return resource_error_response(error),
    };
    let mut builder = http::Response::builder()
        .status(status)
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("X-Content-Type-Options", "nosniff");
    for (name, value) in response.headers {
        if let (Ok(name), Ok(value)) = (
            http::HeaderName::from_bytes(name.as_bytes()),
            http::HeaderValue::from_str(&value),
        ) {
            builder = builder.header(name, value);
        }
    }
    builder.body(body).unwrap_or_else(|_| {
        http::Response::builder()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(Vec::new())
            .expect("static resource response is valid")
    })
}

fn resource_error_response(error: RpcError) -> http::Response<Vec<u8>> {
    let status = match error.code {
        forgecad_app_server_protocol::INVALID_PARAMS => http::StatusCode::BAD_REQUEST,
        INPUT_TOO_LARGE => http::StatusCode::PAYLOAD_TOO_LARGE,
        COMPAT_BACKEND_UNAVAILABLE => http::StatusCode::BAD_GATEWAY,
        _ => http::StatusCode::INTERNAL_SERVER_ERROR,
    };
    http::Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(error.data.application_code.into_bytes())
        .expect("static resource error response is valid")
}

fn backend_unavailable(error: impl std::fmt::Display) -> RpcError {
    RpcError::new(
        COMPAT_BACKEND_UNAVAILABLE,
        "ADAPTER_UNAVAILABLE",
        format!("The fixed loopback compatibility adapter is unavailable: {error}"),
        true,
    )
}

fn malformed_upstream(message: impl Into<String>) -> RpcError {
    RpcError::new(
        MALFORMED_UPSTREAM_EVENT,
        "MALFORMED_UPSTREAM_EVENT",
        message,
        false,
    )
}

fn response_too_large() -> RpcError {
    RpcError::new(
        INPUT_TOO_LARGE,
        "COMPAT_RESPONSE_TOO_LARGE",
        "The compatibility response exceeds the bounded adapter limit.",
        false,
    )
}

fn compat_request_cancelled() -> RpcError {
    RpcError::new(
        REQUEST_CANCELLED,
        "REQUEST_CANCELLED",
        "The compatibility request was cancelled.",
        true,
    )
}

fn persisted_cancel_error(message: impl Into<String>) -> RpcError {
    RpcError::new(
        COMPAT_BACKEND_UNAVAILABLE,
        "PERSISTED_TURN_CANCEL_FAILED",
        message,
        true,
    )
}

async fn compatibility_cancellation_aware<F, T>(
    future: F,
    cancellation: CancellationToken,
) -> Result<T, RpcError>
where
    F: Future<Output = T>,
{
    let mut future = Box::pin(future);
    let mut cancelled = Box::pin(cancellation.cancelled_owned());
    poll_fn(|context| {
        if cancelled.as_mut().poll(context).is_ready() {
            return Poll::Ready(Err(compat_request_cancelled()));
        }
        future.as_mut().poll(context).map(Ok)
    })
    .await
}

async fn k002_cancellation_aware<F, T>(
    future: F,
    cancellation: CancellationToken,
) -> Result<T, K002InternalCallError>
where
    F: Future<Output = T>,
{
    let mut future = Box::pin(future);
    let mut cancelled = Box::pin(cancellation.cancelled_owned());
    poll_fn(|context| {
        if cancelled.as_mut().poll(context).is_ready() {
            return Poll::Ready(Err(K002InternalCallError::Cancelled));
        }
        future.as_mut().poll(context).map(Ok)
    })
    .await
}

async fn restricted_geometry_cancellation_aware<F, T>(
    future: F,
    cancellation: CancellationToken,
) -> Result<T, RestrictedGeometryCallError>
where
    F: Future<Output = Result<T, RestrictedGeometryCallError>>,
{
    let mut future = Box::pin(future);
    let mut cancelled = Box::pin(cancellation.cancelled_owned());
    poll_fn(|context| {
        if cancelled.as_mut().poll(context).is_ready() {
            return Poll::Ready(Err(RestrictedGeometryCallError::Cancelled));
        }
        future.as_mut().poll(context)
    })
    .await
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};
    use std::{
        collections::HashMap as StdHashMap,
        env, fs,
        io::{Read, Write},
        net::{SocketAddr, TcpListener, TcpStream},
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    use tokio::sync::{mpsc, Mutex as AsyncMutex};

    use super::*;

    const TEST_TURN_MESSAGE: &str = "设计一个非功能的未来概念道具外观";
    const TEST_GEOMETRY_CAPABILITY: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn first_json_difference(before: &Value, after: &Value, path: &str) -> Option<String> {
        fn kind(value: &Value) -> &'static str {
            match value {
                Value::Null => "null",
                Value::Bool(_) => "bool",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            }
        }
        fn number_class(value: &Value) -> &'static str {
            value
                .as_number()
                .map(|number| {
                    if number.is_i64() {
                        "signed_integer"
                    } else if number.is_u64() {
                        "unsigned_integer"
                    } else {
                        "float"
                    }
                })
                .unwrap_or("not_number")
        }
        match (before, after) {
            (Value::Object(left), Value::Object(right)) => {
                let mut keys = left.keys().chain(right.keys()).collect::<Vec<_>>();
                keys.sort();
                keys.dedup();
                keys.into_iter().find_map(|key| {
                    let child = format!("{path}/{key}");
                    match (left.get(key), right.get(key)) {
                        (Some(left), Some(right)) => first_json_difference(left, right, &child),
                        (Some(left), None) => Some(format!(
                            "path={child} before_type={} after_type=missing number_class={}",
                            kind(left),
                            number_class(left)
                        )),
                        (None, Some(right)) => Some(format!(
                            "path={child} before_type=missing after_type={} number_class={}",
                            kind(right),
                            number_class(right)
                        )),
                        (None, None) => None,
                    }
                })
            }
            (Value::Array(left), Value::Array(right)) => {
                if left.len() != right.len() {
                    return Some(format!(
                        "path={path} before_type=array after_type=array number_class=length"
                    ));
                }
                left.iter()
                    .zip(right)
                    .enumerate()
                    .find_map(|(index, (left, right))| {
                        first_json_difference(left, right, &format!("{path}/{index}"))
                    })
            }
            (Value::Number(left), Value::Number(right)) if left != right => Some(format!(
                "path={path} before_type=number after_type=number number_class={}/{}",
                number_class(&Value::Number(left.clone())),
                number_class(&Value::Number(right.clone()))
            )),
            (left, right) if left != right => Some(format!(
                "path={path} before_type={} after_type={} number_class={}/{}",
                kind(left),
                kind(right),
                number_class(left),
                number_class(right)
            )),
            _ => None,
        }
    }

    fn seed_bridge_legacy_source(
        database: &Path,
        project_id: &str,
        legacy_version_id: &str,
        legacy_graph_id: &str,
    ) {
        let connection = Connection::open(database).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        let graph = json!({
            "schema_version":"ModuleGraph@1",
            "graph_id":legacy_graph_id,
            "project_id":project_id,
            "root_node_id":"node_root",
            "nodes":[{
                "node_id":"node_root",
                "module_id":"module_legacy_shell",
                "transform":{
                    "position":[0.0,0.0,0.0],
                    "rotation":[0.0,0.0,0.0],
                    "scale":[1.0,1.0,1.0]
                },
                "mirror_axis":"none",
                "locked":false,
                "visible":true
            }],
            "edges":[]
        });
        let graph_json = serde_json::to_string(&graph).unwrap();
        let graph_sha256 = semantic_sha256(&graph).unwrap();
        let spec = json!({
            "schema_version":"WeaponConceptSpec@1",
            "project_id":project_id,
            "profile_id":"profile_weapon_concept_v1",
            "name":"Bridge legacy conversion fixture",
            "archetype":"future_modular_sidearm",
            "intended_uses":["game_asset","film_prop"],
            "style":{
                "keywords":["future","mechanical"],
                "palette":["graphite","signal_red"],
                "detail_density":0.8
            },
            "proportions":{
                "overall_length_mm":320.0,
                "body_height_mm":120.0,
                "grip_angle_deg":12.0
            },
            "required_slots":["core.front","core.rear","core.grip"],
            "optional_slots":["core.top"],
            "constraints":{
                "symmetry":"mostly_symmetric",
                "max_triangle_count":250000
            },
            "assumptions":["Visual-only historical concept fixture."]
        });
        let spec_json = serde_json::to_string(&spec).unwrap();
        let spec_sha256 = semantic_sha256(&spec).unwrap();
        connection
            .execute(
                "INSERT INTO module_graphs(graph_id, project_id, version_id, root_node_id, schema_version, graph_json, graph_sha256, validation_status, created_at, updated_at) VALUES (?, ?, NULL, 'node_root', 'ModuleGraph@1', ?, ?, 'valid', '2026-07-17T00:00:00Z', '2026-07-17T00:00:00Z')",
                params![legacy_graph_id, project_id, graph_json, graph_sha256],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO project_versions(version_id, project_id, parent_version_id, version_no, status, summary, spec_schema_version, spec_json, spec_sha256, module_graph_id, change_set_id, created_at) VALUES (?, ?, NULL, 1, 'committed', 'legacy source', 'WeaponConceptSpec@1', ?, ?, ?, NULL, '2026-07-17T00:00:00Z')",
                params![legacy_version_id, project_id, spec_json, spec_sha256, legacy_graph_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE module_graphs SET version_id=? WHERE graph_id=?",
                params![legacy_version_id, legacy_graph_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE projects SET current_version_id=? WHERE project_id=?",
                params![legacy_version_id, project_id],
            )
            .unwrap();
    }

    fn bridge_legacy_semantic_hash(
        database: &Path,
        project_id: &str,
        legacy_version_id: &str,
        legacy_graph_id: &str,
    ) -> String {
        let connection = Connection::open(database).unwrap();
        let current_version_id = connection
            .query_row(
                "SELECT current_version_id FROM projects WHERE project_id=?",
                [project_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .unwrap();
        let version = connection
            .query_row(
                "SELECT project_id, version_no, status, spec_schema_version, spec_json, spec_sha256, module_graph_id FROM project_versions WHERE version_id=?",
                [legacy_version_id],
                |row| {
                    Ok(json!({
                        "project_id":row.get::<_, String>(0)?,
                        "version_no":row.get::<_, u64>(1)?,
                        "status":row.get::<_, String>(2)?,
                        "spec_schema_version":row.get::<_, String>(3)?,
                        "spec_json":row.get::<_, String>(4)?,
                        "spec_sha256":row.get::<_, String>(5)?,
                        "module_graph_id":row.get::<_, String>(6)?,
                    }))
                },
            )
            .unwrap();
        let graph = connection
            .query_row(
                "SELECT project_id, version_id, schema_version, graph_json, graph_sha256, validation_status FROM module_graphs WHERE graph_id=?",
                [legacy_graph_id],
                |row| {
                    Ok(json!({
                        "project_id":row.get::<_, String>(0)?,
                        "version_id":row.get::<_, String>(1)?,
                        "schema_version":row.get::<_, String>(2)?,
                        "graph_json":row.get::<_, String>(3)?,
                        "graph_sha256":row.get::<_, String>(4)?,
                        "validation_status":row.get::<_, String>(5)?,
                    }))
                },
            )
            .unwrap();
        semantic_sha256(&json!({
            "project_id":project_id,
            "current_version_id":current_version_id,
            "legacy_version_id":legacy_version_id,
            "legacy_graph_id":legacy_graph_id,
            "version":version,
            "graph":graph,
        }))
        .unwrap()
    }

    fn bridge_blockout_plan(project_id: &str, plan_id: &str) -> Value {
        let direction = |direction_id: &str| {
            json!({
                "direction_id":direction_id,
                "title":"Bridge direction",
                "summary":"Complete non-functional exterior concept.",
                "silhouette":"compact",
                "primary_part_roles":["primary_form","secondary_form"],
                "material_direction":"dark metal visual finish"
            })
        };
        json!({
            "schema_version":"MechanicalConceptPlan@1",
            "plan_id":plan_id,
            "domain_pack_id":"pack_future_weapon_prop",
            "brief":"non-functional future game prop exterior",
            "generation_stage":"blockout",
            "spec":{"project_id":project_id},
            "directions":[
                direction("direction_primary")
            ],
            "provider_id":"rust_app_server",
            "shape_program_ready":false
        })
    }

    fn compat_json(response: &CompatHttpResponse) -> Value {
        let ProtocolHttpBody::Utf8 { data } = &response.body else {
            panic!("compat response must be JSON text");
        };
        serde_json::from_str(data).unwrap()
    }

    struct FakeHttpRequest {
        method: String,
        path: String,
        headers: StdHashMap<String, String>,
        body: Vec<u8>,
    }

    #[derive(Debug, Default)]
    struct FakePersistedTurnState {
        turn_created: bool,
        cancelled: bool,
        late_completion_attempted: bool,
        late_completion_written: bool,
        completed_thread_reads: usize,
        request_read_failures: usize,
        fatal_accept_errors: usize,
        requested_paths: Vec<String>,
    }

    struct FakePersistedTurnBackend {
        endpoint: String,
        address: SocketAddr,
        state: Arc<Mutex<FakePersistedTurnState>>,
        events: AsyncMutex<mpsc::UnboundedReceiver<()>>,
        stop: Arc<AtomicBool>,
        accept_task: Option<thread::JoinHandle<()>>,
    }

    impl FakePersistedTurnBackend {
        fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let state = Arc::new(Mutex::new(FakePersistedTurnState::default()));
            let (event_tx, event_rx) = mpsc::unbounded_channel();
            let stop = Arc::new(AtomicBool::new(false));
            let task_state = Arc::clone(&state);
            let task_stop = Arc::clone(&stop);
            let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
            let accept_task = thread::spawn(move || {
                let _ = ready_tx.send(());
                while !task_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if task_stop.load(Ordering::Acquire) {
                                break;
                            }
                            let connection_state = Arc::clone(&task_state);
                            let connection_events = event_tx.clone();
                            thread::spawn(move || {
                                handle_fake_turn_connection(
                                    stream,
                                    connection_state,
                                    connection_events,
                                )
                            });
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::Interrupted
                                    | std::io::ErrorKind::ConnectionAborted
                            ) => {}
                        Err(_) => {
                            task_state.lock().unwrap().fatal_accept_errors += 1;
                            let _ = event_tx.send(());
                            break;
                        }
                    }
                }
            });
            ready_rx.recv().unwrap();
            Self {
                endpoint: format!("http://{address}"),
                address,
                state,
                events: AsyncMutex::new(event_rx),
                stop,
                accept_task: Some(accept_task),
            }
        }
    }

    impl Drop for FakePersistedTurnBackend {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            let _ = TcpStream::connect(self.address);
            if let Some(task) = self.accept_task.take() {
                let _ = task.join();
            }
        }
    }

    fn handle_fake_turn_connection(
        mut stream: TcpStream,
        state: Arc<Mutex<FakePersistedTurnState>>,
        events: mpsc::UnboundedSender<()>,
    ) {
        let Some((method, path)) = read_fake_http_request(&mut stream) else {
            state.lock().unwrap().request_read_failures += 1;
            let _ = events.send(());
            return;
        };
        state
            .lock()
            .unwrap()
            .requested_paths
            .push(format!("{method} {path}"));
        let _ = events.send(());
        match (method.as_str(), path.as_str()) {
            ("POST", "/api/v1/agent/threads/thread_1/turns") => {
                state.lock().unwrap().turn_created = true;
                let _ = events.send(());
                let deadline = Instant::now() + Duration::from_secs(2);
                while !state.lock().unwrap().cancelled && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(2));
                }
                // Simulate a Provider completion racing after the HTTP request
                // was cancelled.  The Python writer's terminal-state guard must
                // keep this attempted completion from replacing `cancelled`.
                thread::sleep(Duration::from_millis(30));
                let mut current = state.lock().unwrap();
                current.late_completion_attempted = true;
                if !current.cancelled {
                    current.late_completion_written = true;
                }
                let status = if current.cancelled {
                    "cancelled"
                } else {
                    "completed"
                };
                drop(current);
                let _ = events.send(());
                let _ = write_fake_json_response(
                    &mut stream,
                    200,
                    &json!({
                        "thread_id": "thread_1",
                        "turn_id": "turn_1",
                        "request_text": TEST_TURN_MESSAGE,
                        "status": status
                    }),
                );
            }
            ("GET", "/api/v1/agent/threads/thread_1") => {
                let current = state.lock().unwrap();
                let status = if current.cancelled {
                    "cancelled"
                } else if current.late_completion_written {
                    "completed"
                } else {
                    "running"
                };
                drop(current);
                if write_fake_json_response(
                    &mut stream,
                    200,
                    &json!({
                        "thread_id": "thread_1",
                        "turns": [{
                            "thread_id": "thread_1",
                            "turn_id": "turn_1",
                            "request_text": TEST_TURN_MESSAGE,
                            "status": status
                        }]
                    }),
                ) {
                    state.lock().unwrap().completed_thread_reads += 1;
                    let _ = events.send(());
                }
            }
            ("POST", "/api/v1/agent/turns/turn_1/cancel") => {
                state.lock().unwrap().cancelled = true;
                let _ = events.send(());
                let _ = write_fake_json_response(
                    &mut stream,
                    200,
                    &json!({
                        "thread_id": "thread_1",
                        "turn_id": "turn_1",
                        "request_text": TEST_TURN_MESSAGE,
                        "status": "cancelled"
                    }),
                );
            }
            _ => {
                let _ = write_fake_json_response(&mut stream, 404, &json!({"error": "not_found"}));
            }
        }
    }

    fn read_fake_http_request(stream: &mut TcpStream) -> Option<(String, String)> {
        let request = read_fake_http_request_full(stream)?;
        Some((request.method, request.path))
    }

    fn read_fake_http_request_full(stream: &mut TcpStream) -> Option<FakeHttpRequest> {
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let mut expected_length = None;
        loop {
            let read = stream.read(&mut buffer).ok()?;
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.len() > 1024 * 1024 {
                return None;
            }
            if expected_length.is_none() {
                if let Some(header_end) = request.windows(4).position(|value| value == b"\r\n\r\n")
                {
                    let headers = str::from_utf8(&request[..header_end]).ok()?;
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    expected_length = Some(header_end + 4 + content_length);
                }
            }
            if expected_length.is_some_and(|length| request.len() >= length) {
                break;
            }
        }
        let header_end = request.windows(4).position(|value| value == b"\r\n\r\n")?;
        let headers = str::from_utf8(&request[..header_end]).ok()?;
        let mut request_line = headers.lines().next()?.split_whitespace();
        let method = request_line.next()?.to_string();
        let path = request_line.next()?.to_string();
        let parsed_headers = headers
            .lines()
            .skip(1)
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
            })
            .collect();
        Some(FakeHttpRequest {
            method,
            path,
            headers: parsed_headers,
            body: request[header_end + 4..].to_vec(),
        })
    }

    fn write_fake_json_response(stream: &mut TcpStream, status: u16, value: &Value) -> bool {
        let body = serde_json::to_vec(value).unwrap();
        let reason = if status == 200 { "OK" } else { "Not Found" };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).is_ok()
            && stream.write_all(&body).is_ok()
            && stream.flush().is_ok()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FakeGeometryScenario {
        Success,
        TamperedGlbHash,
        ErrorEnvelope,
        BlockUntilCancelled,
    }

    #[derive(Debug, Clone)]
    struct FakeGeometryArtifact {
        handle: String,
        profile_id: String,
        profile_sha256: String,
        shape_program_sha256: String,
        glb_sha256: String,
        glb_byte_size: u64,
        triangle_count: u32,
        bounds_mm: [f64; 3],
    }

    #[derive(Debug, Default)]
    struct FakeGeometryState {
        requested_paths: Vec<String>,
        capability_headers: Vec<Option<String>>,
        request_bodies: Vec<Value>,
        artifact: Option<FakeGeometryArtifact>,
        cancel_seen: bool,
        request_failures: usize,
    }

    struct FakeGeometryBackend {
        endpoint: String,
        address: SocketAddr,
        state: Arc<Mutex<FakeGeometryState>>,
        stop: Arc<AtomicBool>,
        accept_task: Option<thread::JoinHandle<()>>,
    }

    impl FakeGeometryBackend {
        fn start(scenario: FakeGeometryScenario) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let state = Arc::new(Mutex::new(FakeGeometryState::default()));
            let stop = Arc::new(AtomicBool::new(false));
            let task_state = Arc::clone(&state);
            let task_stop = Arc::clone(&stop);
            let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
            let accept_task = thread::spawn(move || {
                let _ = ready_tx.send(());
                while !task_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if task_stop.load(Ordering::Acquire) {
                                break;
                            }
                            let state = Arc::clone(&task_state);
                            thread::spawn(move || {
                                handle_fake_geometry_connection(stream, state, scenario)
                            });
                        }
                        Err(_) => break,
                    }
                }
            });
            ready_rx.recv().unwrap();
            Self {
                endpoint: format!("http://{address}"),
                address,
                state,
                stop,
                accept_task: Some(accept_task),
            }
        }
    }

    impl Drop for FakeGeometryBackend {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            let _ = TcpStream::connect(self.address);
            if let Some(task) = self.accept_task.take() {
                let _ = task.join();
            }
        }
    }

    fn handle_fake_geometry_connection(
        mut stream: TcpStream,
        state: Arc<Mutex<FakeGeometryState>>,
        scenario: FakeGeometryScenario,
    ) {
        let Some(request) = read_fake_http_request_full(&mut stream) else {
            state.lock().unwrap().request_failures += 1;
            return;
        };
        let body = serde_json::from_slice::<Value>(&request.body).unwrap_or(Value::Null);
        {
            let mut current = state.lock().unwrap();
            current
                .requested_paths
                .push(format!("{} {}", request.method, request.path));
            current.capability_headers.push(
                request
                    .headers
                    .get("x-forgecad-restricted-geometry-capability")
                    .cloned(),
            );
            current.request_bodies.push(body.clone());
        }
        if request.method != "POST" {
            let _ = write_fake_json_response(&mut stream, 405, &json!({"error": "method"}));
            return;
        }
        if request.path == RESTRICTED_GEOMETRY_CANCEL_PATH {
            state.lock().unwrap().cancel_seen = true;
            let _ = write_fake_json_response(
                &mut stream,
                200,
                &json!({
                    "schema_version": "RestrictedGeometryCancellationResult@1",
                    "cancellation_id": body["cancellation_id"],
                    "accepted": true,
                    "tombstoned": true
                }),
            );
            return;
        }
        if request.path != RESTRICTED_GEOMETRY_EXECUTE_PATH {
            let _ = write_fake_json_response(&mut stream, 404, &json!({"error": "path"}));
            return;
        }
        if scenario == FakeGeometryScenario::ErrorEnvelope {
            let _ = write_fake_json_response(
                &mut stream,
                422,
                &json!({
                    "error": {
                        "code": "SHAPE_PROGRAM_INVALID",
                        "message": "The bounded ShapeProgram was rejected.",
                        "recoverable": false,
                        "details": {}
                    }
                }),
            );
            return;
        }
        match body.get("action").and_then(Value::as_str) {
            Some("compile_readback") => {
                if scenario == FakeGeometryScenario::BlockUntilCancelled {
                    let deadline = Instant::now() + Duration::from_secs(3);
                    while !state.lock().unwrap().cancel_seen && Instant::now() < deadline {
                        thread::sleep(Duration::from_millis(2));
                    }
                    let _ = write_fake_json_response(
                        &mut stream,
                        409,
                        &json!({
                            "error": {
                                "code": "GEOMETRY_EXECUTION_CANCELLED",
                                "message": "The geometry execution was cancelled.",
                                "recoverable": true,
                                "details": {}
                            }
                        }),
                    );
                    return;
                }
                let response = fake_geometry_compile_response(
                    &body,
                    scenario == FakeGeometryScenario::TamperedGlbHash,
                    &state,
                );
                let _ = write_fake_json_response(&mut stream, 200, &response);
            }
            Some("render") => {
                let response = fake_geometry_render_response(&body, &state);
                let _ = write_fake_json_response(&mut stream, 200, &response);
            }
            _ => {
                let _ = write_fake_json_response(&mut stream, 400, &json!({"error": "action"}));
            }
        }
    }

    fn fake_forgecad_profile_glb(profile_id: &str) -> Vec<u8> {
        fake_forgecad_profile_glb_with_profile_height(profile_id, 1.0)
    }

    fn fake_forgecad_profile_glb_with_profile_height(
        profile_id: &str,
        profile_height_scale: f32,
    ) -> Vec<u8> {
        let production = profile_id == "production_concept";
        assert!(production || profile_id == "interactive_preview");
        let mut profile = json!({
            "schema_version": "GeometryArtifactProfile@1",
            "artifact_profile_id": profile_id,
            "radial_segments": if production { 64 } else { 24 },
            "capsule_hemisphere_segments": if production { 14 } else { 5 },
            "smooth_loft_normals": production,
            "texture_width": if production { 1024 } else { 128 },
            "texture_height": if production { 1024 } else { 128 },
            "texture_mime_type": "image/png",
            "texture_compression": "png_deflate",
            "delivery": if production { "on_demand" } else { "interactive" },
            "triangle_budget_multiplier": if production { 6 } else { 1 },
            "max_triangle_count": if production { 250_000 } else { 100_000 }
        });
        profile["profile_sha256"] = Value::String(semantic_sha256(&profile).unwrap());
        let dimension = if production { 1024_u32 } else { 128_u32 };
        let texture_version = if production { "v4" } else { "v3" };
        let indices = [0_u16, 1, 2, 0, 3, 1, 0, 2, 3, 1, 3, 2];
        let positions = [
            0_f32,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            profile_height_scale,
            0.0,
            0.0,
            0.0,
            1.0,
        ];
        let normals = [0_f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let tangents = [
            1_f32, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0,
        ];
        let uvs = [0_f32, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let mut binary = Vec::new();
        let mut views = Vec::<Value>::new();
        let mut append_view = |payload: &[u8], target: Option<u64>| {
            let offset = binary.len();
            binary.extend_from_slice(payload);
            let index = views.len();
            let mut view = json!({
                "buffer": 0,
                "byteOffset": offset,
                "byteLength": payload.len()
            });
            if let Some(target) = target {
                view["target"] = json!(target);
            }
            views.push(view);
            while binary.len() % 4 != 0 {
                binary.push(0);
            }
            index
        };
        let index_bytes = indices
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let position_bytes = positions
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let normal_bytes = normals
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let tangent_bytes = tangents
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let uv_bytes = uvs
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let index_view = append_view(&index_bytes, Some(34963));
        let position_view = append_view(&position_bytes, Some(34962));
        let normal_view = append_view(&normal_bytes, Some(34962));
        let tangent_view = append_view(&tangent_bytes, Some(34962));
        let uv_view = append_view(&uv_bytes, Some(34962));

        let mut images = Vec::new();
        let mut textures = Vec::new();
        for (index, role) in [
            "base_color",
            "metallic_roughness",
            "normal",
            "occlusion",
            "emissive",
        ]
        .into_iter()
        .enumerate()
        {
            // A browser-facing GLB fixture must contain a decodable image,
            // not merely an IHDR-shaped byte prefix.  The former made the
            // Rust V003 bridge look valid to readback while GLTFLoader
            // rejected every embedded texture in the real workbench.
            // This deterministic opaque PNG is enough to exercise the
            // complete image-buffer -> texture -> PBR material path; the
            // production compiler remains responsible for profile-sized
            // texture generation.
            let png = if production {
                BASE64
                .decode("iVBORw0KGgoAAAANSUhEUgAAAgAAAAIACAYAAAD0eNT6AAAG40lEQVR42u3WMQEAAAQAQWmUk05CStjccAV++sjqAQB+CREAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAAAyAEABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAAAYABEAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAAAyAEABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAAAYABEAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAAAyAEABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAAAYABEAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAAAyAEABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAAAYABEAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAAAyAEABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAAAYABEAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAwAAGAAAAADAAAYAADAAAAABgAAMAAAgAEAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAABgAAMAAAAAGAAAwAACAAQAADAAAYAAAAAMAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAAwAAAAAYAAAyAEABgAAAAAwAAGAAAwAAAAAYAADAAAIABAAAMAABgAAAAAwAAGAAA4M4CWiyHPUOc5FAAAAAASUVORK5CYII=")
                    .expect("embedded production fixture PNG is valid base64")
            } else {
                // The interactive profile has a contractual 128×128 PBR
                // texture size.  A fixed 512×512 fixture makes real GLB
                // readback fail before the V003/A005 lifecycle can run.
                BASE64
                    .decode("iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAAA8klEQVR42u3SAQ0AMAjAsHMXF3aTmAQbJLQSlsX7WYe1rgQGwAAYAANgAAyAATAABsAAGAADYAAMgAEwAAbAABgAA2AADIABMAAGwAAYAANgAAyAATAABsAAGAADYAAMgAEwAAbAABgAA2AADIABMAAGwAAYAANgAAyAATAABsAAGAADYAAMgAEwAAbAABgAA2AADIABMAAGwAAYAANgAAyAATAABsAAGAADYAADSGAADIABMAAGwAAYAANgAAyAATAABsAAGAADYAAMgAEwAAbAABgAA2AADIABMAAGwAAYAANgAAyAATAABsAAGAADYAAMgAEwAAbAABgAA2AADIABMAAGwAAYAANgAAyAATAABsAAGAADYAADMEMDbDoDF9y970oAAAAASUVORK5CYII=")
                    .expect("embedded interactive fixture PNG is valid base64")
            };
            // Keep the legacy readable fixture above for historical review,
            // but bind the active production test artifact to a real 1024×1024
            // PNG.  The old embedded production sample is 512×512 and began
            // failing as soon as the production profile contract moved to 1K.
            let png = if production {
                fake_production_1024_png()
            } else {
                png
            };
            let view = append_view(&png, None);
            let sha = sha256_hex(&png);
            images.push(json!({
                "name": format!("vtex_test_{role}_{texture_version}"),
                "bufferView": view,
                "mimeType": "image/png",
                "extras": {"forgecad_visual_texture": {
                    "texture_id": format!("vtex_test_{role}_{texture_version}"),
                    "texture_role": role,
                    "mime_type": "image/png",
                    "byte_size": png.len(),
                    "sha256": sha,
                    "color_space": if matches!(role, "base_color" | "emissive") { "srgb" } else { "linear" },
                    "width": dimension,
                    "height": dimension,
                    "source": "forgecad_builtin",
                    "license": "not_applicable",
                    "fallback": "none",
                    "visual_only": true
                }}
            }));
            textures.push(json!({
                "name": format!("vtex_test_{role}_{texture_version}"),
                "source": index
            }));
        }
        drop(append_view);
        let document = json!({
            "asset": {"version": "2.0", "generator": "ForgeCAD bridge test"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0}],
            "meshes": [{"primitives": [{
                "attributes": {"POSITION":1,"NORMAL":2,"TANGENT":3,"TEXCOORD_0":4},
                "indices": 0,
                "material": 0,
                "mode": 4,
                "extras": {
                    "forgecad_feature_node_id": "op_shell",
                    "forgecad_material_zone_id": "zone_shell",
                    "forgecad_surface_ranges": [{"surface_role":"surface","first_triangle":0,"triangle_count":4}],
                    "forgecad_source_face_ids": [0,1,2,3]
                }
            }]}],
            "materials": [{
                "pbrMetallicRoughness": {
                    "baseColorFactor": [1,1,1,1],
                    "metallicFactor": 1,
                    "roughnessFactor": 1,
                    "baseColorTexture": {"index":0},
                    "metallicRoughnessTexture": {"index":1}
                },
                "normalTexture": {"index":2},
                "occlusionTexture": {"index":3},
                "emissiveTexture": {"index":4},
                "emissiveFactor": [1,1,1],
                "extras": {
                    "forgecad_visual_texture_set_id": format!("vtexset_primary_builtin_{texture_version}"),
                    "forgecad_texture_material_id": "mat_primary",
                    "forgecad_visual_only": true
                }
            }],
            "images": images,
            "textures": textures,
            "buffers": [{"byteLength": binary.len()}],
            "bufferViews": views,
            "accessors": [
                {"bufferView":index_view,"componentType":5123,"count":12,"type":"SCALAR"},
                {"bufferView":position_view,"componentType":5126,"count":4,"type":"VEC3","min":[0,0,0],"max":[1,profile_height_scale,1]},
                {"bufferView":normal_view,"componentType":5126,"count":4,"type":"VEC3"},
                {"bufferView":tangent_view,"componentType":5126,"count":4,"type":"VEC4"},
                {"bufferView":uv_view,"componentType":5126,"count":4,"type":"VEC2"}
            ],
            "extras": {
                "forgecad_geometry_artifact_profile": profile,
                "forgecad_feature_history": [{
                    "node_id":"op_shell",
                    "runtime_manifest_version":"ShapeProgramRuntimeManifest@1",
                    "result_sha256":"a".repeat(64)
                }]
            }
        });
        let mut json_chunk = serde_json::to_vec(&document).unwrap();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(b' ');
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let total_length = 12 + 8 + json_chunk.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4e4f534a_u32.to_le_bytes());
        glb.extend_from_slice(&json_chunk);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004e4942_u32.to_le_bytes());
        glb.extend_from_slice(&binary);
        glb
    }

    fn fake_production_1024_png() -> Vec<u8> {
        BASE64
            .decode("iVBORw0KGgoAAAANSUhEUgAABAAAAAQAAQAAAABXZhYuAAAAlklEQVR42u3BAQEAAACCIP+vbkhAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADvBgQeAAEN3jhkAAAAAElFTkSuQmCC")
            .expect("embedded production 1024 fixture PNG is valid base64")
    }

    fn fake_geometry_compile_response(
        request: &Value,
        tamper_hash: bool,
        state: &Arc<Mutex<FakeGeometryState>>,
    ) -> Value {
        let sealed_shape_program = request["shape_program_canonical_json"]
            .as_str()
            .expect("Rust compile request carries canonical ShapeProgram JSON");
        let shape_program_sha256 = request["shape_program_sha256"]
            .as_str()
            .expect("Rust compile request carries ShapeProgram SHA")
            .to_string();
        assert_eq!(
            sha256_hex(sealed_shape_program.as_bytes()),
            shape_program_sha256
        );
        assert_eq!(
            serde_json::from_str::<Value>(sealed_shape_program).unwrap(),
            request["shape_program"]
        );
        let profile_id = request["artifact_profile_id"].as_str().unwrap().to_string();
        let artifact_handle = format!("geomart_{}", "a".repeat(48));
        let profile_height_scale = request["shape_program"]["operations"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|operation| operation["op"] == "sweep")
            .and_then(|operation| operation["args"]["profile_scale"][1].as_f64())
            .map(|height| (height / 100.0).clamp(0.1, 10.0) as f32)
            .unwrap_or(1.0);
        let glb = fake_forgecad_profile_glb_with_profile_height(&profile_id, profile_height_scale);
        let canonical = verify_forgecad_glb(&glb, Some(&profile_id)).unwrap();
        let profile_sha256 = canonical.artifact_profile_sha256.clone();
        let bounds_mm: [f64; 3] = canonical.bounds_mm.clone().try_into().unwrap();
        let triangle_count = u32::try_from(canonical.triangle_count).unwrap();
        let actual_glb_sha256 = sha256_hex(&glb);
        let claimed_glb_sha256 = if tamper_hash {
            "f".repeat(64)
        } else {
            actual_glb_sha256.clone()
        };
        let artifact = FakeGeometryArtifact {
            handle: artifact_handle.clone(),
            profile_id: profile_id.clone(),
            profile_sha256: profile_sha256.clone(),
            shape_program_sha256: shape_program_sha256.clone(),
            glb_sha256: actual_glb_sha256,
            glb_byte_size: glb.len() as u64,
            triangle_count,
            bounds_mm,
        };
        state.lock().unwrap().artifact = Some(artifact.clone());
        json!({
            "schema_version": "RestrictedGeometryExecutionResult@1",
            "protocol_version": RESTRICTED_GEOMETRY_PROTOCOL_VERSION,
            "execution_id": request["execution_id"],
            "action": "compile_readback",
            "artifact_handle": artifact_handle,
            "artifact_profile_id": profile_id,
            "artifact_profile_sha256": profile_sha256,
            "shape_program_sha256": shape_program_sha256,
            "glb_sha256": claimed_glb_sha256,
            "glb_byte_size": glb.len(),
            "triangle_count": triangle_count,
            "bounds_mm": bounds_mm,
            "readback": {
                "schema_version": "GeometryCompileReadback@2",
                "runtime_manifest_version": "ShapeProgramRuntimeManifest@1",
                "artifact_profile": {
                    "artifact_profile_id": artifact.profile_id,
                    "profile_sha256": artifact.profile_sha256
                },
                "shape_program_sha256": artifact.shape_program_sha256,
                "glb_sha256": claimed_glb_sha256,
                "glb_byte_size": glb.len(),
                "triangle_count": triangle_count,
                "bounds_mm": bounds_mm,
                "mesh_count": canonical.mesh_count,
                "primitive_count": canonical.primitive_count,
                "material_count": canonical.material_count,
                // Keep the fake restricted-geometry port aligned with the
                // frozen V003 compiler contract: production validation reads
                // bounded Material Zone and five-map PBR provenance rather
                // than trusting an untyped visual claim from the harness.
                "material_zone_faces": [{
                    "material_zone_id": "zone_primary",
                    "material_id": "mat_graphite",
                    "texture_ready": true
                }],
                "visual_texture_sets": [{
                    "material_id": "mat_graphite",
                    "material_zone_ids": ["zone_primary"],
                    "maps": [
                        {"texture_role": "base_color", "source": "forgecad_builtin", "license": "not_applicable", "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
                        {"texture_role": "metallic_roughness", "source": "forgecad_builtin", "license": "not_applicable", "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
                        {"texture_role": "normal", "source": "forgecad_builtin", "license": "not_applicable", "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"},
                        {"texture_role": "occlusion", "source": "forgecad_builtin", "license": "not_applicable", "sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},
                        {"texture_role": "emissive", "source": "forgecad_builtin", "license": "not_applicable", "sha256": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"}
                    ]
                }],
                "surface_provenance": [{
                    "closed": true,
                    "boundary_edge_count": 0,
                    "non_manifold_edge_count": 0,
                    "degenerate_triangle_count": 0
                }],
                "closed_manifold": true,
                "surface_provenance_present": true
            },
            "glb_base64": BASE64.encode(glb),
            "render_views": null,
            "render_view_sha256": null,
            "renderer_id": null,
            "exploded_part_ids": [],
            "exploded_unavailable_reason": null
        })
    }

    fn fake_geometry_render_response(
        request: &Value,
        state: &Arc<Mutex<FakeGeometryState>>,
    ) -> Value {
        let artifact = state.lock().unwrap().artifact.clone().unwrap();
        let width = u32::try_from(request["render"]["width"].as_u64().unwrap()).unwrap();
        let height = u32::try_from(request["render"]["height"].as_u64().unwrap()).unwrap();
        let mut views = serde_json::Map::new();
        let mut hashes = serde_json::Map::new();
        for view_id in RESTRICTED_GEOMETRY_REQUIRED_VIEWS {
            let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
            png.extend_from_slice(&width.to_be_bytes());
            png.extend_from_slice(&height.to_be_bytes());
            png.extend_from_slice(&[8, 6, 0, 0, 0]);
            png.extend_from_slice(&[0, 0, 0, 0]);
            png.extend_from_slice(view_id.as_bytes());
            hashes.insert(view_id.into(), Value::String(sha256_hex(&png)));
            views.insert(view_id.into(), Value::String(BASE64.encode(png)));
        }
        json!({
            "schema_version": "RestrictedGeometryExecutionResult@1",
            "protocol_version": RESTRICTED_GEOMETRY_PROTOCOL_VERSION,
            "execution_id": request["execution_id"],
            "action": "render",
            "artifact_handle": artifact.handle,
            "artifact_profile_id": artifact.profile_id,
            "artifact_profile_sha256": artifact.profile_sha256,
            "shape_program_sha256": artifact.shape_program_sha256,
            "glb_sha256": artifact.glb_sha256,
            "glb_byte_size": artifact.glb_byte_size,
            "triangle_count": artifact.triangle_count,
            "bounds_mm": artifact.bounds_mm,
            "readback": null,
            "glb_base64": null,
            "render_views": views,
            "render_view_sha256": hashes,
            "renderer_id": RESTRICTED_GEOMETRY_RENDERER_ID,
            "exploded_part_ids": [],
            "exploded_unavailable_reason": null
        })
    }

    fn restricted_geometry_test_input() -> RestrictedGeometryInput {
        RestrictedGeometryInput {
            schema_version: "RestrictedGeometryInput@1".into(),
            shape_program: json!({
                "schema_version": "ShapeProgram@1",
                "program_id": "shape_bridge_test",
                "units": "millimeter",
                "seed": 7,
                "triangle_budget": 1000,
                "parameters": [],
                "operations": [{
                    "operation_id": "op_primary_shell",
                    "op": "box",
                    "inputs": [],
                    "args": {
                        "size": [100.0, 40.0, 20.0],
                        "position": [0.0, 0.0, 0.0],
                        "rotation": [0.0, 0.0, 0.0],
                        "part_role": "primary_form",
                        "zone_id": "zone_primary",
                        "material_id": "mat_graphite"
                    }
                }],
                "outputs": [{
                    "output_id": "output_primary_shell",
                    "operation_id": "op_primary_shell",
                    "kind": "mesh",
                    "part_role": "primary_form"
                }],
                "non_functional_only": true
            }),
            profile_sketch: None,
            section_set: None,
            surface_adornment_programs: Vec::new(),
            surface_layer_input: None,
            quality_profile: forgecad_app_server::RestrictedQualityProfile {
                profile_id: "interactive_preview".into(),
                runtime_manifest_version: "ShapeProgramRuntimeManifest@1".into(),
                max_triangle_count: 7_000,
                render_width: 320,
                render_height: 320,
                require_closed_manifold: true,
                require_surface_provenance: true,
            },
        }
    }

    fn restricted_geometry_test_port(endpoint: &str) -> LoopbackHttpPort {
        LoopbackHttpPort::new(
            LocalAgentEndpoint::parse(endpoint).unwrap(),
            TEST_GEOMETRY_CAPABILITY.into(),
        )
        .unwrap()
    }

    async fn make_bridge_connection_ready(bridge: &AppServerBridge, connection_id: &str) {
        let initialize = json!({
            "jsonrpc": "2.0",
            "id": "req_initialize",
            "method": "initialize",
            "params": {
                "schema_version": "ForgeCADInitializeParams@1",
                "supported_protocol_versions": ["forgecad.app-server/1"],
                "client_info": {
                    "name": "forgecad-desktop-test",
                    "version": "0.1.0",
                    "transport": "tauri"
                },
                "capabilities": {
                    "notifications": true,
                    "cursor_replay": true,
                    "cancellation": true,
                    "notification_ack": true,
                    "binary_body_base64": true
                }
            }
        })
        .to_string();
        let response = bridge
            .inner
            .server
            .handle_frame(connection_id, &initialize)
            .await
            .unwrap();
        assert!(serde_json::from_str::<Value>(&response).unwrap()["error"].is_null());
        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {"protocol_version": "forgecad.app-server/1"}
        })
        .to_string();
        assert!(bridge
            .inner
            .server
            .handle_frame(connection_id, &initialized)
            .await
            .is_none());
    }

    async fn wait_for_fake_turn_state(
        label: &str,
        backend: &FakePersistedTurnBackend,
        predicate: impl Fn(&FakePersistedTurnState) -> bool,
    ) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            if predicate(&backend.state.lock().unwrap()) {
                return;
            }
            let event = {
                let mut events = backend.events.lock().await;
                tokio::time::timeout_at(deadline, events.recv()).await
            };
            if !matches!(event, Ok(Some(()))) {
                break;
            }
        }
        panic!(
            "fake persisted Turn did not reach {label}: {:?}",
            backend.state.lock().unwrap()
        );
    }

    #[test]
    fn request_cancel_propagates_to_persisted_turn_and_late_completion_is_discarded() {
        let backend = FakePersistedTurnBackend::start();
        let bridge = AppServerBridge::new(&backend.endpoint).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let connection = bridge.inner.server.open_connection();
                let connection_id = connection.connection_id.clone();
                make_bridge_connection_ready(&bridge, &connection_id).await;
                let turn_body = json!({
                    "client_request_id": "client_turn_1",
                    "message": TEST_TURN_MESSAGE
                })
                .to_string();
                let turn_request_frame = json!({
                    "jsonrpc": "2.0",
                    "id": "req_turn_1",
                    "method": "compat/http",
                    "params": {
                        "schema_version": "ForgeCADHttpCompatibilityRequest@1",
                        "path": "/api/v1/agent/threads/thread_1/turns",
                        "method": "POST",
                        "headers": [
                            ["content-type", "application/json"],
                            ["idempotency-key", "turn-create-1"]
                        ],
                        "body": {"encoding": "utf8", "data": turn_body}
                    }
                });
                let (registered_request_id, registered_target) =
                    protocol_turn_cancellation_registration(&turn_request_frame).unwrap();
                registered_target.set_preexisting_turn_ids(Vec::new());
                bridge.inner.pending_turn_requests.lock().unwrap().insert(
                    (connection_id.clone(), registered_request_id),
                    registered_target,
                );
                let turn_request = turn_request_frame.to_string();
                let request_server = Arc::clone(&bridge.inner.server);
                let request_connection_id = connection_id.clone();
                let request_task = tokio::spawn(async move {
                    request_server
                        .handle_frame(&request_connection_id, &turn_request)
                        .await
                        .unwrap()
                });
                wait_for_fake_turn_state("created", &backend, |state| state.turn_created).await;

                let cancel = json!({
                    "jsonrpc": "2.0",
                    "method": "request/cancel",
                    "params": {
                        "request_id": "req_turn_1",
                        "cancel_token": "req_turn_1"
                    }
                })
                .to_string();
                let cancel_frame: Value = serde_json::from_str(&cancel).unwrap();
                assert!(bridge
                    .propagate_protocol_cancel_notification(&connection_id, &cancel_frame)
                    .await
                    .unwrap());
                assert!(bridge
                    .inner
                    .server
                    .handle_frame(&connection_id, &cancel)
                    .await
                    .is_none());
                assert!(bridge
                    .inner
                    .pending_turn_requests
                    .lock()
                    .unwrap()
                    .is_empty());

                let response: Value = serde_json::from_str(&request_task.await.unwrap()).unwrap();
                assert_eq!(response["error"]["code"], REQUEST_CANCELLED);
                wait_for_fake_turn_state(
                    "completed cancellation stability checks",
                    &backend,
                    |state| {
                        state.cancelled
                            && state.late_completion_attempted
                            && state.completed_thread_reads >= TURN_CANCEL_STABILITY_CHECKS + 1
                    },
                )
                .await;
            });

        let state = backend.state.lock().unwrap();
        assert!(state.cancelled);
        assert!(state.late_completion_attempted);
        assert!(!state.late_completion_written);
        assert_eq!(state.request_read_failures, 0);
        assert_eq!(state.fatal_accept_errors, 0);
        assert!(state.completed_thread_reads >= TURN_CANCEL_STABILITY_CHECKS + 1);
        assert!(state
            .requested_paths
            .iter()
            .any(|path| path == "POST /api/v1/agent/turns/turn_1/cancel"));
        assert!(
            state
                .requested_paths
                .iter()
                .filter(|path| path.as_str() == "GET /api/v1/agent/threads/thread_1")
                .count()
                >= TURN_CANCEL_STABILITY_CHECKS + 1
        );
    }

    #[test]
    fn persisted_turn_cancel_mapping_skips_preexisting_same_prompt_turn() {
        let body = json!({
            "client_request_id": "client_1",
            "message": TEST_TURN_MESSAGE
        })
        .to_string();
        let target = persisted_turn_cancellation_target(
            "POST",
            "/api/v1/agent/threads/thread_1/turns",
            body.as_bytes(),
        )
        .unwrap();
        target.set_preexisting_turn_ids(vec!["turn_other".to_string()]);
        let another_turn = json!({
            "thread_id": "thread_1",
            "turns": [{
                "thread_id": "thread_1",
                "turn_id": "turn_other",
                "request_text": TEST_TURN_MESSAGE,
                "status": "running"
            }]
        });
        assert_eq!(
            persisted_turn_state(&another_turn, &target, None).unwrap(),
            PersistedTurnCancellationState::Missing
        );
        assert!(persisted_turn_cancellation_target(
            "GET",
            "/api/v1/agent/threads/thread_1/turns",
            body.as_bytes()
        )
        .is_none());
        assert!(persisted_turn_cancellation_target(
            "POST",
            "/api/v1/agent/threads/thread_1/turns?unsafe=1",
            body.as_bytes()
        )
        .is_none());
        assert!(protocol_cancel_notification(&json!({
            "jsonrpc": "2.0",
            "method": "request/cancel",
            "params": {"request_id": "req_1", "cancel_token": "req_2"}
        }))
        .is_none());
    }

    #[test]
    fn restricted_geometry_port_compiles_then_renders_with_capability_and_real_bytes() {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::Success);
        let port = restricted_geometry_test_port(&backend.endpoint);
        let input = restricted_geometry_test_input();
        let output = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(port.build_compile_render(input.clone(), CancellationToken::new()))
            .unwrap();
        output.validate(&input).unwrap();
        assert!(output.glb_bytes.starts_with(b"glTF"));
        assert_eq!(output.glb_sha256, sha256_hex(&output.glb_bytes));
        assert_eq!(
            output
                .views
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            RESTRICTED_GEOMETRY_REQUIRED_VIEWS.into_iter().collect()
        );
        for (view_id, png) in &output.views {
            assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
            assert_eq!(output.view_sha256[view_id], sha256_hex(png));
        }

        let state = backend.state.lock().unwrap();
        assert_eq!(state.request_failures, 0);
        assert_eq!(
            state.requested_paths,
            [
                "POST /api/v1/internal/geometry/execute",
                "POST /api/v1/internal/geometry/execute"
            ]
        );
        assert!(state
            .capability_headers
            .iter()
            .all(|value| value.as_deref() == Some(TEST_GEOMETRY_CAPABILITY)));
        assert_eq!(state.request_bodies[0]["action"], "compile_readback");
        assert_eq!(state.request_bodies[1]["action"], "render");
        let sealed_shape_program = state.request_bodies[0]["shape_program_canonical_json"]
            .as_str()
            .unwrap();
        assert_eq!(
            sha256_hex(sealed_shape_program.as_bytes()),
            state.request_bodies[0]["shape_program_sha256"]
                .as_str()
                .unwrap()
        );
        assert_eq!(
            serde_json::from_str::<Value>(sealed_shape_program).unwrap(),
            state.request_bodies[0]["shape_program"]
        );
        assert!(state.request_bodies[0].get("plan").is_none());
        assert!(state.request_bodies[0].get("style_recipe").is_none());
        assert!(state.request_bodies[0].get("provider_key").is_none());
        assert!(state.request_bodies[1].get("glb_base64").is_none());
    }

    #[test]
    fn restricted_geometry_port_rejects_tampered_glb_hash_before_render() {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::TamperedGlbHash);
        let port = restricted_geometry_test_port(&backend.endpoint);
        let error =
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(port.build_compile_render(
                    restricted_geometry_test_input(),
                    CancellationToken::new(),
                ))
                .unwrap_err();
        assert_eq!(error.code, "RESTRICTED_GEOMETRY_RESPONSE_INVALID");
        assert_eq!(error.kind, RestrictedGeometryErrorKind::Execution);
        let state = backend.state.lock().unwrap();
        assert_eq!(
            state.requested_paths,
            ["POST /api/v1/internal/geometry/execute"]
        );
    }

    #[test]
    fn restricted_geometry_port_maps_strict_error_envelope() {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::ErrorEnvelope);
        let port = restricted_geometry_test_port(&backend.endpoint);
        let error =
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(port.build_compile_render(
                    restricted_geometry_test_input(),
                    CancellationToken::new(),
                ))
                .unwrap_err();
        assert_eq!(error.code, "SHAPE_PROGRAM_INVALID");
        assert_eq!(error.kind, RestrictedGeometryErrorKind::InvalidInput);
        assert!(!error.recoverable);
    }

    #[test]
    fn restricted_geometry_port_posts_cancel_and_discards_late_compile() {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::BlockUntilCancelled);
        let port = restricted_geometry_test_port(&backend.endpoint);
        let cancellation = CancellationToken::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime.block_on(async {
            let work = tokio::spawn(
                port.build_compile_render(restricted_geometry_test_input(), cancellation.clone()),
            );
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            loop {
                if backend
                    .state
                    .lock()
                    .unwrap()
                    .requested_paths
                    .iter()
                    .any(|path| path == "POST /api/v1/internal/geometry/execute")
                {
                    break;
                }
                assert!(tokio::time::Instant::now() < deadline);
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            cancellation.cancel();
            tokio::time::timeout(Duration::from_secs(3), work)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err()
        });
        assert_eq!(error.kind, RestrictedGeometryErrorKind::Cancelled);
        let state = backend.state.lock().unwrap();
        assert!(state.cancel_seen);
        assert!(state
            .requested_paths
            .iter()
            .any(|path| path == "POST /api/v1/internal/geometry/cancel"));
        assert!(state
            .capability_headers
            .iter()
            .all(|value| value.as_deref() == Some(TEST_GEOMETRY_CAPABILITY)));
    }

    #[test]
    fn sse_parser_handles_crlf_and_multiline_data() {
        let event = parse_sse_block(b"id: 7\r\nevent: agent.item\r\ndata: one\r\ndata: two")
            .unwrap()
            .unwrap();
        assert_eq!(event.id.as_deref(), Some("7"));
        assert_eq!(event.event, "agent.item");
        assert_eq!(event.data, "one\ntwo");
        assert_eq!(sse_boundary(b"a\r\n\r\nb"), Some((1, 4)));
    }

    #[test]
    fn hidden_reasoning_is_rejected_recursively() {
        assert!(contains_forbidden_reasoning(
            r#"{"item":{"reasoning_content":"secret"}}"#
        ));
        assert!(!contains_forbidden_reasoning(
            r#"{"item":{"content":"safe"}}"#
        ));
    }

    #[test]
    fn agent_item_cursor_uses_persisted_sequence_and_identity() {
        let event = ParsedSseEvent {
            id: Some("7".to_string()),
            event: "agent.item".to_string(),
            data: serde_json::json!({
                "sequence": 7,
                "thread_id": "thread_1",
                "turn_id": "turn_1",
                "item": {
                    "item_id": "item_7",
                    "thread_id": "thread_1",
                    "turn_id": "turn_1",
                    "sequence": 7
                }
            })
            .to_string(),
        };
        let cursor = agent_event_cursor(&event).unwrap();
        assert_eq!(cursor.thread_id, "thread_1");
        assert_eq!(cursor.turn_id.as_deref(), Some("turn_1"));
        assert_eq!(cursor.source_sequence, 7);
        assert_eq!(cursor.item_id.as_deref(), Some("item_7"));

        let mut inconsistent = event;
        inconsistent.id = Some("8".to_string());
        assert!(agent_event_cursor(&inconsistent).is_err());
    }

    #[test]
    fn non_item_sse_is_forwarded_without_a_fabricated_cursor() {
        let bridge = AppServerBridge::new("http://127.0.0.1:8000").unwrap();
        let mut connection = bridge.inner.server.open_connection();
        let event = ParsedSseEvent {
            id: None,
            event: "agent.replay.complete".to_string(),
            data: "{}".to_string(),
        };
        assert_eq!(
            publish_sse_event(
                &bridge.inner.server,
                &connection.connection_id,
                "stream_replay",
                event,
            )
            .unwrap()
            .0,
            None
        );
        let frame = connection.notifications.try_recv().unwrap();
        let notification: Value = serde_json::from_str(&frame).unwrap();
        assert_eq!(notification["method"], METHOD_COMPAT_SSE);
        assert_eq!(notification["params"]["event"], "agent.replay.complete");
        assert!(notification.get("cursor").is_none());
        assert!(notification.get("notification_id").is_none());
    }

    #[test]
    fn binary_body_round_trips_without_utf8_loss() {
        let body = vec![0, 255, 1, 128];
        let encoded = encode_protocol_body(body.clone());
        assert!(matches!(encoded, ProtocolHttpBody::Base64 { .. }));
        assert_eq!(decode_protocol_body(encoded).unwrap(), body);
    }

    #[test]
    fn static_rust_domain_catalog_returns_complete_array_without_python() {
        for (path, expected_count) in [("/api/v1/agent/domain-packs", 4)] {
            let request = PreparedCompatHttpRequest {
                endpoint: LocalAgentEndpoint::parse("http://127.0.0.1:8000").unwrap(),
                method: AllowedHttpMethod::Get,
                path: path.to_string(),
                headers: Vec::new(),
                body: ProtocolHttpBody::Empty,
            };
            let response = rust_catalog_response(&request)
                .expect("catalog route is Rust-owned")
                .unwrap();
            assert_eq!(response.status, 200);
            let ProtocolHttpBody::Utf8 { data } = response.body else {
                panic!("catalog response must be JSON text");
            };
            assert_eq!(
                serde_json::from_str::<Value>(&data)
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .len(),
                expected_count
            );
        }
    }

    #[test]
    fn rust_owned_unknown_write_returns_410_instead_of_python_fallback() {
        let response = rust_owned_product_route_response();
        assert_eq!(response.status, 410);
        let ProtocolHttpBody::Utf8 { data } = response.body else {
            panic!("Rust ownership rejection must be JSON text");
        };
        let body: Value = serde_json::from_str(&data).unwrap();
        assert_eq!(body["error"]["code"], "PRODUCT_STATE_RUST_OWNED");
    }

    #[test]
    fn production_bridge_uses_rust_core_and_never_forwards_unknown_writes() {
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-k003-production-bridge-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("bridge-test-{serial}"))
                .expect("open unpublished Rust core"),
        );
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            "http://127.0.0.1:8000",
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .expect("construct production bridge before publishing cutover");
        assert!(!bridge.inner.legacy_persisted_turn_cancellation);
        assert!(bridge.inner.native_product_tools.is_some());
        assert!(bridge
            .preview_artifact("preview_missing")
            .unwrap()
            .is_none());
        assert!(bridge
            .consume_preview("preview_missing", "turn_missing")
            .is_err());

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for (method, path, expected_status) in [
            (AllowedHttpMethod::Get, "/api/v1/projects", 200),
            (AllowedHttpMethod::Post, "/api/v1/agent/blockouts", 400),
        ] {
            let response = runtime
                .block_on(CompatibilityHttpPort::execute(
                    bridge.inner.port.as_ref(),
                    PreparedCompatHttpRequest {
                        endpoint: LocalAgentEndpoint::parse("http://127.0.0.1:8000").unwrap(),
                        method,
                        path: path.to_string(),
                        headers: Vec::new(),
                        body: ProtocolHttpBody::Empty,
                    },
                    CancellationToken::new(),
                ))
                .unwrap();
            assert_eq!(response.status, expected_status);
        }

        assert!(rust_core.rollback_cutover_before_publish().unwrap());
        drop(bridge);
        drop(rust_core);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn formal_single_result_routes_require_exact_method_and_stable_identity() {
        let endpoint = LocalAgentEndpoint::parse("http://127.0.0.1:8000").unwrap();
        let request = |method, suffix: &str| PreparedCompatHttpRequest {
            endpoint: endpoint.clone(),
            method,
            path: format!(
                "/api/v1/agent/projects/project_1/turns/turn_1/single-results/preview_1{suffix}"
            ),
            headers: Vec::new(),
            body: ProtocolHttpBody::Empty,
        };
        assert_eq!(
            parse_native_single_result_route(&request(AllowedHttpMethod::Get, ":preview.glb"))
                .unwrap()
                .action,
            NativeSingleResultAction::PreviewGlb
        );
        assert_eq!(
            parse_native_single_result_route(&request(AllowedHttpMethod::Post, ":confirm"))
                .unwrap()
                .action,
            NativeSingleResultAction::Confirm
        );
        assert_eq!(
            parse_native_single_result_route(&request(AllowedHttpMethod::Post, ":reject"))
                .unwrap()
                .action,
            NativeSingleResultAction::Reject
        );
        assert!(parse_native_single_result_route(&request(
            AllowedHttpMethod::Post,
            ":preview.glb"
        ))
        .is_none());
    }

    #[test]
    fn formal_single_result_preview_get_and_confirm_create_one_atomic_asset() {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::Success);
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-v003-formal-preview-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("v003-formal-preview-{serial}")).unwrap(),
        );
        // The browser development-shell fixture may bind this real Rust
        // execution to the project ID created by its isolated compatibility
        // shell.  The decision remains Rust-produced; the environment only
        // selects its already-owned project identity.
        let project_id = std::env::var("FORGECAD_V003_RUST_E2E_PROJECT_ID")
            .unwrap_or_else(|_| "project_v003_formal".into());
        assert!(
            project_id
                .chars()
                .all(|value| value.is_ascii_alphanumeric() || matches!(value, '_' | '-' | '.'))
                && !project_id.is_empty()
                && project_id.len() <= 160,
            "V003 fixture project identity must be a stable project ID"
        );
        let turn_id = "turn_v003_formal";
        rust_core
            .repository()
            .create_project(&forgecad_core::Project {
                project_id: project_id.clone(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "V003 formal preview".into(),
                status: forgecad_core::ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-18T00:00:00Z".into(),
                updated_at: "2026-07-18T00:00:00Z".into(),
            })
            .unwrap();
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .unwrap();
        let executor = bridge.inner.native_product_tools.as_ref().unwrap();
        let registry = ProductToolRegistry::default();
        let execution_id = "execution_v003_formal";
        executor
            .bind_execution_project(execution_id, turn_id, Some(project_id.as_str()))
            .unwrap();
        let plan = json!({
            "schema_version": "MechanicalConceptPlan@1",
            "plan_id": "plan_v003_formal",
            "domain_pack_id": "pack_future_weapon_prop",
            "brief": "non-functional future game prop exterior concept",
            "generation_stage": "blockout",
            "spec": {},
            "directions": [
                {
                    "direction_id": "direction_primary",
                    "title": "Primary",
                    "summary": "Complete visual-only future prop exterior.",
                    "silhouette": "compact",
                    "primary_part_roles": ["primary_form", "secondary_form"],
                    "material_direction": "painted metal visual finish"
                }
            ],
            "provider_id": "rust_app_server",
            "shape_program_ready": false
        });
        let calls = [
            ("plan_complete_concept", json!({"plan": plan})),
            (
                "select_style_recipe",
                json!({
                    "domain_pack_id": "pack_future_weapon_prop",
                    "intent": "紧凑流线"
                }),
            ),
            (
                "build_candidate_geometry",
                json!({
                    "direction_id": "direction_primary",
                    "variant_id": null,
                    // V003 is a production-concept decision: the formal
                    // Rust fixture must exercise the same production PBR
                    // hard gate that its sealed preview advertises.
                    "presentation_profile": "showcase"
                }),
            ),
            ("compile_readback_candidate", json!({})),
            ("render_candidate_views", json!({})),
            ("evaluate_candidate", json!({})),
            ("prepare_candidate_preview", json!({})),
        ];
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut final_result = None;
        let mut evaluated_candidate = None;
        for (index, (name, arguments)) in calls.into_iter().enumerate() {
            let request = registry
                .build_execution_request(
                    turn_id,
                    &ProviderToolCall {
                        call_id: format!("call_v003_formal_{index}"),
                        name: name.into(),
                        arguments,
                    },
                    execution_id,
                    "cancel_v003_formal",
                    "token_v003_formal",
                )
                .unwrap();
            let result = runtime
                .block_on(executor.execute(request, CancellationToken::new()))
                .unwrap();
            if name == "evaluate_candidate" {
                evaluated_candidate = result
                    .validated_output
                    .as_ref()
                    .map(|output| output.value.clone());
            }
            assert_eq!(
                result.status,
                ProductToolExecutionStatus::Completed,
                "{name}: error_code={:?} message={:?} output={:?} evaluated_candidate={:?}",
                result.error_code,
                result.message,
                result.validated_output,
                evaluated_candidate,
            );
            final_result = Some(result);
        }
        let output = final_result.unwrap().validated_output.unwrap().value;
        let decision = &output["single_result_decision"];
        let preview_id = decision["preview"]["preview_id"].as_str().unwrap();
        let artifact_sha256 = decision["preview"]["artifact_sha256"].as_str().unwrap();
        let etag = format!("\"sha256:{artifact_sha256}\"");
        let path = format!(
            "/api/v1/agent/projects/{project_id}/turns/{turn_id}/single-results/{preview_id}"
        );
        let endpoint = LocalAgentEndpoint::parse(&backend.endpoint).unwrap();
        let get = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Get,
                    path: format!("{path}:preview.glb"),
                    headers: vec![("If-Match".into(), etag.clone())],
                    body: ProtocolHttpBody::Empty,
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(get.status, 200);
        assert!(get.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("cache-control") && value == "no-store"
        }));
        let preview_headers = get.headers.clone();
        let ProtocolHttpBody::Base64 { data } = get.body else {
            panic!("formal preview GET must return binary base64 transport");
        };
        assert_eq!(sha256_hex(&BASE64.decode(&data).unwrap()), artifact_sha256);
        assert!(rust_core.repository().head(&project_id).unwrap().is_none());

        let client_request_id = "confirm_v003_formal";
        let confirm_body = json!({
            "client_request_id": client_request_id,
            "expected_artifact_sha256": artifact_sha256,
            "summary": "Confirmed V003 formal preview"
        });
        let confirm_request = || PreparedCompatHttpRequest {
            endpoint: endpoint.clone(),
            method: AllowedHttpMethod::Post,
            path: format!("{path}:confirm"),
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                ("Idempotency-Key".into(), client_request_id.into()),
                ("If-Match".into(), etag.clone()),
            ],
            body: ProtocolHttpBody::Utf8 {
                data: confirm_body.to_string(),
            },
        };
        let confirmed = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                confirm_request(),
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(confirmed.status, 201, "{:?}", confirmed.body);
        let head = rust_core.repository().head(&project_id).unwrap().unwrap();
        assert_eq!(
            rust_core
                .repository()
                .version(&head)
                .unwrap()
                .unwrap()
                .version_no,
            1
        );
        assert!(bridge.preview_artifact(preview_id).unwrap().is_none());
        let replay = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                confirm_request(),
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(replay.status, 201);
        assert_eq!(
            rust_core.repository().head(&project_id).unwrap().unwrap(),
            head
        );

        // The browser workbench cannot obtain a formal decision from the
        // legacy Python compatibility oracle.  When explicitly requested by
        // the V003 E2E harness, export the exact Rust-produced decision,
        // binary preview and confirmed response from this production bridge
        // test.  This is test data generated after the same trusted
        // `NativeProductToolExecutor -> RustCoreRuntime` path exercised above;
        // it is never a model-authored or Python-forged decision.
        if let Ok(fixture_path) = std::env::var("FORGECAD_V003_RUST_E2E_FIXTURE_PATH") {
            let fixture = json!({
                "schema_version": "ForgeCADV003RustWorkbenchFixture@1",
                "project_id": project_id,
                "turn_id": turn_id,
                "decision": decision,
                "preview_glb_base64": data,
                "preview_headers": preview_headers,
                "preview_sha256": artifact_sha256,
                "confirm_request": confirm_body,
                "confirm_response": compat_json(&confirmed),
                "confirm_status": confirmed.status,
                "replay_status": replay.status,
                "source": "rust_app_server_native_product_tools"
            });
            std::fs::write(
                fixture_path,
                serde_json::to_vec(&fixture).expect("serialize V003 Rust E2E fixture"),
            )
            .expect("write V003 Rust E2E fixture");
        }

        drop(bridge);
        drop(rust_core);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn c106_robotic_arm_single_result_uses_the_existing_atomic_lifecycle() {
        fn persistent_counts(db_path: &Path) -> Vec<(String, i64)> {
            let connection = Connection::open(db_path).expect("open C106 lifecycle database");
            [
                "projects",
                "agent_asset_versions",
                "agent_asset_heads",
                "active_design_snapshots",
                "agent_asset_change_sets",
                "agent_asset_quality_reports",
                "export_packages_v2",
                "forgecad_core_objects",
                "forgecad_core_object_references",
            ]
            .into_iter()
            .map(|table| {
                let count = connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .expect("read C106 lifecycle table count");
                (table.to_string(), count)
            })
            .collect()
        }

        fn object_manifest(root: &Path) -> Vec<(String, u64)> {
            fn visit(path: &Path, root: &Path, rows: &mut Vec<(String, u64)>) {
                let entries = match fs::read_dir(path) {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                    Err(error) => panic!("read C106 object directory: {error}"),
                };
                for entry in entries {
                    let entry = entry.expect("read C106 object entry");
                    let path = entry.path();
                    let metadata = entry.metadata().expect("read C106 object metadata");
                    if metadata.is_dir() {
                        visit(&path, root, rows);
                    } else if metadata.is_file() {
                        rows.push((
                            path.strip_prefix(root)
                                .expect("C106 object path stays under library")
                                .display()
                                .to_string(),
                            metadata.len(),
                        ));
                    }
                }
            }

            let mut rows = Vec::new();
            visit(&root.join("objects").join("sha256"), root, &mut rows);
            rows.sort();
            rows
        }

        let backend = FakeGeometryBackend::start(FakeGeometryScenario::Success);
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-c106-single-result-lifecycle-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("c106-single-result-{serial}"))
                .expect("open C106 Rust core"),
        );
        let project_id = "project_c106_single_result";
        rust_core
            .repository()
            .create_project(&forgecad_core::Project {
                project_id: project_id.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "C106 robotic-arm lifecycle".into(),
                status: forgecad_core::ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-18T00:00:00Z".into(),
                updated_at: "2026-07-18T00:00:00Z".into(),
            })
            .unwrap();
        let provider_probe =
            FakeDeepSeekClient::scripted("deepseek-chat", false, false, Vec::new());
        let provider: Arc<dyn ProviderClient> = Arc::new(provider_probe.clone());
        let bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .unwrap();
        let executor = bridge.inner.native_product_tools.as_ref().unwrap();
        let registry = ProductToolRegistry::default();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let endpoint = LocalAgentEndpoint::parse(&backend.endpoint).unwrap();
        let baseline_counts = persistent_counts(&library_root.join("library.db"));
        let baseline_objects = object_manifest(&library_root);

        let create_preview = |execution_id: &str, turn_id: &str, intent: &str| {
            executor
                .bind_execution_project(execution_id, turn_id, Some(project_id))
                .unwrap();
            let plan = json!({
                "schema_version": "MechanicalConceptPlan@1",
                "plan_id": format!("plan_{execution_id}"),
                "domain_pack_id": "pack_robotic_arm_concept",
                "brief": "non-functional articulated robotic-arm exterior concept",
                "generation_stage": "blockout",
                "spec": {},
                "directions": [
                    {
                        "direction_id": "direction_primary",
                        "title": "Robotic arm exterior",
                        "summary": "Complete visual-only articulated robotic arm exterior.",
                        "silhouette": "industrial",
                        "primary_part_roles": ["link_armor", "surface_trim"],
                        "material_direction": "anodized exterior panels, dark joints, and blue signal trim"
                    }
                ],
                "provider_id": "rust_app_server",
                "shape_program_ready": false
            });
            let calls = [
                ("plan_complete_concept", json!({"plan": plan})),
                (
                    "select_style_recipe",
                    json!({"domain_pack_id": "pack_robotic_arm_concept", "intent": intent}),
                ),
                (
                    "build_candidate_geometry",
                    json!({"direction_id": "direction_primary", "variant_id": null, "presentation_profile": "showcase"}),
                ),
                ("compile_readback_candidate", json!({})),
                ("render_candidate_views", json!({})),
                ("evaluate_candidate", json!({})),
                ("prepare_candidate_preview", json!({})),
            ];
            let mut final_output = None;
            for (index, (name, arguments)) in calls.into_iter().enumerate() {
                let cancellation_id = format!("cancel_{execution_id}");
                let cancellation_token = format!("token_{execution_id}");
                let request = registry
                    .build_execution_request(
                        turn_id,
                        &ProviderToolCall {
                            call_id: format!("call_{execution_id}_{index}"),
                            name: name.into(),
                            arguments,
                        },
                        execution_id,
                        &cancellation_id,
                        &cancellation_token,
                    )
                    .unwrap();
                let result = runtime
                    .block_on(executor.execute(request, CancellationToken::new()))
                    .unwrap();
                assert_eq!(
                    result.status,
                    ProductToolExecutionStatus::Completed,
                    "C106 {name} failed: {:?}",
                    result.error_code
                );
                final_output = result.validated_output.map(|output| output.value);
            }
            let output = final_output.expect("C106 V003 preview has output");
            assert_eq!(output["permanent_side_effects"], json!(0));
            assert_eq!(
                output["single_result_decision"]["state"],
                json!("ready_for_preview")
            );
            let decision = &output["single_result_decision"];
            let preview_id = decision["preview"]["preview_id"]
                .as_str()
                .unwrap()
                .to_string();
            let artifact_sha256 = decision["preview"]["artifact_sha256"]
                .as_str()
                .unwrap()
                .to_string();
            (preview_id, artifact_sha256)
        };

        // A rejected preview and an already-cancelled preview read are both
        // transient operations: neither may create Version, Snapshot,
        // Quality, Export, or CAS state.
        let (rejected_preview_id, rejected_sha256) =
            create_preview("execution_c106_reject", "turn_c106_reject", "厚重");
        assert_eq!(
            persistent_counts(&library_root.join("library.db")),
            baseline_counts
        );
        assert_eq!(object_manifest(&library_root), baseline_objects);
        let rejected_path = format!(
            "/api/v1/agent/projects/{project_id}/turns/turn_c106_reject/single-results/{rejected_preview_id}"
        );
        let rejected_etag = format!("\"sha256:{rejected_sha256}\"");
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let cancelled_preview = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Get,
                    path: format!("{rejected_path}:preview.glb"),
                    headers: vec![("If-Match".into(), rejected_etag.clone())],
                    body: ProtocolHttpBody::Empty,
                },
                cancelled,
            ))
            .unwrap();
        assert_ne!(cancelled_preview.status, 200);
        assert_eq!(
            persistent_counts(&library_root.join("library.db")),
            baseline_counts
        );
        let rejected = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: format!("{rejected_path}:reject"),
                    headers: vec![
                        ("Content-Type".into(), "application/json".into()),
                        ("Idempotency-Key".into(), "reject_c106_preview".into()),
                        ("If-Match".into(), rejected_etag.clone()),
                    ],
                    body: ProtocolHttpBody::Utf8 {
                        data: json!({
                            "client_request_id": "reject_c106_preview",
                            "expected_artifact_sha256": rejected_sha256,
                        })
                        .to_string(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(rejected.status, 200, "{:?}", rejected.body);
        assert!(rust_core.repository().head(project_id).unwrap().is_none());
        let late_preview = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Get,
                    path: format!("{rejected_path}:preview.glb"),
                    headers: vec![("If-Match".into(), rejected_etag)],
                    body: ProtocolHttpBody::Empty,
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(
            late_preview.status, 404,
            "late/rejected preview must not resurrect"
        );
        assert_eq!(
            persistent_counts(&library_root.join("library.db")),
            baseline_counts
        );
        assert_eq!(object_manifest(&library_root), baseline_objects);

        let (preview_id, artifact_sha256) =
            create_preview("execution_c106_confirm", "turn_c106_confirm", "流线");
        assert_eq!(
            persistent_counts(&library_root.join("library.db")),
            baseline_counts
        );
        assert_eq!(object_manifest(&library_root), baseline_objects);
        let path = format!(
            "/api/v1/agent/projects/{project_id}/turns/turn_c106_confirm/single-results/{preview_id}"
        );
        let etag = format!("\"sha256:{artifact_sha256}\"");
        let confirm_body = json!({
            "client_request_id": "confirm_c106_single_result",
            "expected_artifact_sha256": artifact_sha256,
            "summary": "Confirm C106 robotic-arm single result"
        });
        let confirm_request = || PreparedCompatHttpRequest {
            endpoint: endpoint.clone(),
            method: AllowedHttpMethod::Post,
            path: format!("{path}:confirm"),
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                (
                    "Idempotency-Key".into(),
                    "confirm_c106_single_result".into(),
                ),
                ("If-Match".into(), etag.clone()),
            ],
            body: ProtocolHttpBody::Utf8 {
                data: confirm_body.to_string(),
            },
        };
        let confirmed = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                confirm_request(),
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(confirmed.status, 201, "{:?}", confirmed.body);
        let head = rust_core.repository().head(project_id).unwrap().unwrap();
        let version = rust_core.repository().version(&head).unwrap().unwrap();
        assert_eq!(version.version_no, 1);
        assert_eq!(version.domain_pack_id, "pack_robotic_arm_concept");
        assert_eq!(
            version.assembly_graph["parts"].as_array().map(Vec::len),
            Some(10),
            "the confirmed C106 asset retains its ten editable semantic components"
        );
        let initial_snapshot = rust_core
            .repository()
            .snapshot(project_id)
            .unwrap()
            .unwrap();
        let initial_quality = rust_core
            .repository()
            .quality_report(
                &initial_snapshot
                    .quality
                    .as_ref()
                    .expect("confirmed C106 asset has quality")
                    .quality_report_id,
            )
            .unwrap()
            .expect("confirmed C106 quality report is readable");
        let production_glb_sha256 = initial_quality.report["compile_readback"]["glb_sha256"]
            .as_str()
            .expect("C106 quality binds production GLB hash")
            .to_string();
        let production_triangle_count = initial_quality.report["compile_readback"]
            ["triangle_count"]
            .as_u64()
            .expect("C106 quality binds production triangle count");
        let material_zone_count = version
            .parts
            .iter()
            .flat_map(|part| part["material_zone_ids"].as_array().into_iter().flatten())
            .filter_map(Value::as_str)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        assert_eq!(production_glb_sha256, artifact_sha256);
        assert!(production_triangle_count > 0);
        assert_eq!(material_zone_count, 19);
        let confirmed_counts = persistent_counts(&library_root.join("library.db"));
        assert_ne!(confirmed_counts, baseline_counts);
        assert_eq!(
            confirmed_counts
                .iter()
                .find(|(table, _)| table == "agent_asset_versions")
                .map(|(_, count)| *count),
            Some(1)
        );
        assert_eq!(
            confirmed_counts
                .iter()
                .find(|(table, _)| table == "agent_asset_quality_reports")
                .map(|(_, count)| *count),
            Some(1)
        );
        let replay = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                confirm_request(),
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(replay.status, 201);
        assert_eq!(
            rust_core.repository().head(project_id).unwrap().unwrap(),
            head
        );
        assert_eq!(
            persistent_counts(&library_root.join("library.db")),
            confirmed_counts
        );

        // Use the existing ChangeSet/Snapshot lifecycle for a normal visual
        // material edit, then ensure undo/redo retain the C106 arm lineage.
        let target_part = version.parts.first().unwrap();
        let target_part_id = target_part["part_id"].as_str().unwrap();
        let target_zone_id = target_part["material_zone_ids"]
            .as_array()
            .and_then(|zones| zones.first())
            .and_then(Value::as_str)
            .unwrap();
        let material = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: format!("/api/v1/agent/asset-versions/{head}/change-sets"),
                    headers: vec![
                        ("Content-Type".into(), "application/json".into()),
                        ("Idempotency-Key".into(), "propose_c106_material".into()),
                    ],
                    body: ProtocolHttpBody::Utf8 {
                        data: json!({
                            "client_request_id": "propose_c106_material",
                            "summary": "C106 visual material adjustment",
                            "operations": [{
                                "operation_id": "changeop_c106_material",
                                "op": "apply_material_preset",
                                "part_id": target_part_id,
                                "material_zone_id": target_zone_id,
                                "material_id": "mat_painted_steel"
                            }]
                        })
                        .to_string(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(material.status, 201, "{:?}", material.body);
        let change_set_id = compat_json(&material)["change_set_id"]
            .as_str()
            .unwrap()
            .to_string();
        for (suffix, body) in [
            ("preview", ProtocolHttpBody::Empty),
            ("confirm", ProtocolHttpBody::Empty),
        ] {
            let response = runtime
                .block_on(CompatibilityHttpPort::execute(
                    bridge.inner.port.as_ref(),
                    PreparedCompatHttpRequest {
                        endpoint: endpoint.clone(),
                        method: AllowedHttpMethod::Post,
                        path: format!("/api/v1/agent/change-sets/{change_set_id}:{suffix}"),
                        headers: vec![(
                            "Idempotency-Key".into(),
                            format!("{suffix}_c106_material"),
                        )],
                        body,
                    },
                    CancellationToken::new(),
                ))
                .unwrap();
            assert_eq!(response.status, 200, "{:?}", response.body);
        }
        let material_head = rust_core.repository().head(project_id).unwrap().unwrap();
        let enabled_surface_skill = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: "/api/v1/agent/skills/surface-adornment:enable".into(),
                    headers: vec![
                        ("Content-Type".into(), "application/json".into()),
                        ("Idempotency-Key".into(), "enable_c106_surface_skill".into()),
                    ],
                    body: ProtocolHttpBody::Utf8 {
                        data: json!({
                            "schema_version": "EnableSurfaceAdornmentSkillRequest@1",
                            "client_request_id": "enable_c106_surface_skill",
                            "confirm_enable": true,
                        })
                        .to_string(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(
            enabled_surface_skill.status, 200,
            "{:?}",
            enabled_surface_skill.body
        );
        let surface_preview = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: format!(
                        "/api/v1/agent/asset-versions/{material_head}/surface-adornments:preview"
                    ),
                    headers: vec![
                        ("Content-Type".into(), "application/json".into()),
                        (
                            "Idempotency-Key".into(),
                            "preview_c106_surface_adornment".into(),
                        ),
                    ],
                    body: ProtocolHttpBody::Utf8 {
                        data: json!({
                            "schema_version": "SurfaceAdornmentPreviewRequest@1",
                            "client_request_id": "preview_c106_surface_adornment",
                            "part_id": target_part_id,
                            "material_zone_id": target_zone_id,
                            // The selected C106 service root explicitly exposes a
                            // `flowline/double_flowline` visual-only slot.  Keep
                            // this lifecycle test inside the Recipe contract rather
                            // than asking A005 to accept an arbitrary motif.
                            "kind": "flowline",
                            "motif": "double_flowline",
                            "intensity": "subtle",
                            "coverage": "center_band",
                        })
                        .to_string(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(surface_preview.status, 201, "{:?}", surface_preview.body);
        let surface_change_set_id = compat_json(&surface_preview)["change_set_id"]
            .as_str()
            .expect("C106 surface preview returns a ChangeSet")
            .to_string();
        for suffix in ["preview", "confirm"] {
            let response = runtime
                .block_on(CompatibilityHttpPort::execute(
                    bridge.inner.port.as_ref(),
                    PreparedCompatHttpRequest {
                        endpoint: endpoint.clone(),
                        method: AllowedHttpMethod::Post,
                        path: format!("/api/v1/agent/change-sets/{surface_change_set_id}:{suffix}"),
                        headers: vec![(
                            "Idempotency-Key".into(),
                            format!("{suffix}_c106_surface_adornment"),
                        )],
                        body: ProtocolHttpBody::Empty,
                    },
                    CancellationToken::new(),
                ))
                .unwrap();
            assert_eq!(response.status, 200, "{:?}", response.body);
        }
        let adornment_head = rust_core.repository().head(project_id).unwrap().unwrap();
        let adornment_version = rust_core
            .repository()
            .version(&adornment_head)
            .unwrap()
            .unwrap();
        assert_eq!(adornment_version.version_no, 3);
        assert_eq!(adornment_version.domain_pack_id, "pack_robotic_arm_concept");
        assert_eq!(
            adornment_version.assembly_graph["surface_adornments"]
                .as_array()
                .map(Vec::len),
            Some(1),
            "C106 A005 preview confirmation seals one visual-only adornment into a new version"
        );
        let adornment_snapshot = rust_core
            .repository()
            .snapshot(project_id)
            .unwrap()
            .unwrap();
        let undo = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: format!("/api/v1/projects/{project_id}/active-design:undo"),
                    headers: vec![
                        ("Content-Type".into(), "application/json".into()),
                        ("Idempotency-Key".into(), "undo_c106_material".into()),
                        ("If-Match".into(), adornment_snapshot.etag().to_string()),
                    ],
                    body: ProtocolHttpBody::Utf8 {
                        data: json!({
                            "client_request_id": "undo_c106_material",
                            "snapshot_revision": adornment_snapshot.revision,
                        })
                        .to_string(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(undo.status, 200, "{:?}", undo.body);
        let undo_snapshot = rust_core
            .repository()
            .snapshot(project_id)
            .unwrap()
            .unwrap();
        let undo_head = undo_snapshot
            .active_design
            .asset_version_id()
            .unwrap()
            .to_string();
        let undo_version = rust_core.repository().version(&undo_head).unwrap().unwrap();
        assert_eq!(undo_version.domain_pack_id, "pack_robotic_arm_concept");
        assert_eq!(
            undo_version.shape_program,
            rust_core
                .repository()
                .version(&material_head)
                .unwrap()
                .unwrap()
                .shape_program,
            "undo removes only the confirmed visual adornment and restores the material edit"
        );
        let redo = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: format!("/api/v1/projects/{project_id}/active-design:redo"),
                    headers: vec![
                        ("Content-Type".into(), "application/json".into()),
                        ("Idempotency-Key".into(), "redo_c106_material".into()),
                        ("If-Match".into(), undo_snapshot.etag().to_string()),
                    ],
                    body: ProtocolHttpBody::Utf8 {
                        data: json!({
                            "client_request_id": "redo_c106_material",
                            "snapshot_revision": undo_snapshot.revision,
                        })
                        .to_string(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(redo.status, 200, "{:?}", redo.body);
        let redo_snapshot = rust_core
            .repository()
            .snapshot(project_id)
            .unwrap()
            .unwrap();
        let redo_head = redo_snapshot
            .active_design
            .asset_version_id()
            .unwrap()
            .to_string();
        let redo_version = rust_core.repository().version(&redo_head).unwrap().unwrap();
        assert_ne!(
            redo_head, adornment_head,
            "redo creates an immutable descendant"
        );
        assert_eq!(redo_version.domain_pack_id, "pack_robotic_arm_concept");
        assert_eq!(
            redo_version.shape_program,
            rust_core
                .repository()
                .version(&adornment_head)
                .unwrap()
                .unwrap()
                .shape_program
        );

        let exported = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: format!("/api/v1/agent/asset-versions/{redo_head}:export"),
                    headers: vec![
                        ("Content-Type".into(), "application/json".into()),
                        ("Idempotency-Key".into(), "export_c106_golden_path".into()),
                    ],
                    body: ProtocolHttpBody::Utf8 {
                        data: json!({"client_request_id": "export_c106_golden_path"}).to_string(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(exported.status, 200, "{:?}", exported.body);
        let export_glb = BASE64
            .decode(
                compat_json(&exported)["glb_base64"]
                    .as_str()
                    .expect("C106 export returns binary GLB as base64"),
            )
            .unwrap();
        let export_readback = verify_forgecad_glb(&export_glb, Some("production_concept")).unwrap();
        assert_eq!(export_readback.triangle_count, production_triangle_count);
        let export_glb_sha256 = sha256_hex(&export_glb);

        drop(bridge);
        drop(rust_core);
        let reopened = RustCoreRuntime::open(
            &library_root,
            format!("c106-single-result-restart-{serial}"),
        )
        .unwrap();
        let restarted_snapshot = reopened.repository().snapshot(project_id).unwrap().unwrap();
        assert_eq!(
            restarted_snapshot.active_design.asset_version_id(),
            Some(redo_head.as_str())
        );
        assert_eq!(
            reopened
                .repository()
                .version(&redo_head)
                .unwrap()
                .unwrap()
                .domain_pack_id,
            "pack_robotic_arm_concept"
        );
        let measured_provider_calls = provider_probe.records().len();
        assert_eq!(
            measured_provider_calls, 0,
            "the offline C106 lifecycle must never issue a Provider request"
        );
        if let Ok(evidence_file) = std::env::var("FORGECAD_C106_LIFECYCLE_EVIDENCE_FILE") {
            fs::write(
                evidence_file,
                serde_json::to_vec(&json!({
                    "schema_version": "C106LifecycleMeasuredEvidence@1",
                    "measured_provider_calls": measured_provider_calls,
                    "measurement_source": "FakeDeepSeekClient.records",
                    "provider_policy": "offline_deny_on_call",
                    "restart_readback": true,
                    "transient_cancel_reject_zero_persistent_writes": true,
                    "brief": "流线三关节维护机械臂，固定基座、双连杆、旋转腕部和夹爪",
                    "selected_root_recipe_id": "recipe_c106_arm_service_display",
                    "initial_asset_version_id": head,
                    "initial_asset_version_no": version.version_no,
                    "initial_snapshot_revision": initial_snapshot.revision,
                    "preview_glb_sha256": artifact_sha256,
                    "production_glb_sha256": production_glb_sha256,
                    "v003_preview_triangle_count": production_triangle_count,
                    "material_zone_count": material_zone_count,
                    "a005_confirmed_asset_version_id": adornment_head,
                    "restart_asset_version_id": redo_head,
                    "restart_snapshot_revision": restarted_snapshot.revision,
                    "export_glb_sha256": export_glb_sha256,
                    "export_triangle_count": export_readback.triangle_count
                }))
                .expect("serialize C106 measured lifecycle evidence"),
            )
            .expect("write C106 measured lifecycle evidence");
        }
        drop(reopened);
        drop(backend);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn production_bridge_unknown_retired_gets_and_legacy_sse_never_reach_python() {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::Success);
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-k003-no-python-get-fallback-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("no-python-get-{serial}"))
                .expect("open Rust core for GET fallback test"),
        );
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let execute_get = |path: &str| {
            runtime
                .block_on(bridge.execute_k003_packaged_compat(
                    PreparedCompatHttpRequest {
                        endpoint: LocalAgentEndpoint::parse(&backend.endpoint).unwrap(),
                        method: AllowedHttpMethod::Get,
                        path: path.into(),
                        headers: Vec::new(),
                        body: ProtocolHttpBody::Empty,
                    },
                    CancellationToken::new(),
                ))
                .unwrap()
        };

        let unknown = execute_get("/api/v1/agent/unknown-product-read");
        assert_eq!(unknown.status, 410);
        assert_eq!(
            compat_json(&unknown)["error"]["code"],
            "PRODUCT_STATE_RUST_OWNED"
        );
        let retired = execute_get("/api/v1/change-sets/legacy_change_set");
        assert_eq!(retired.status, 410);
        assert_eq!(
            compat_json(&retired)["error"]["code"],
            "LEGACY_CONCEPT_ROUTE_RETIRED"
        );
        assert!(
            backend.state.lock().unwrap().requested_paths.is_empty(),
            "unknown and retired product GETs must terminate in Rust"
        );

        let health = execute_get("/api/health");
        assert_eq!(health.status, 405);
        assert_eq!(
            backend.state.lock().unwrap().requested_paths,
            ["GET /api/health"],
            "only the exact sidecar health observation may cross the port"
        );
        assert!(!is_python_sidecar_observation_route(
            &PreparedCompatHttpRequest {
                endpoint: LocalAgentEndpoint::parse(&backend.endpoint).unwrap(),
                method: AllowedHttpMethod::Get,
                path: "/api/health?debug=1".into(),
                headers: Vec::new(),
                body: ProtocolHttpBody::Empty,
            }
        ));

        let subscription = runtime
            .block_on(CompatibilityHttpPort::subscribe(
                bridge.inner.port.as_ref(),
                SseSubscriptionParams {
                    schema_version: "ForgeCADSseSubscription@1".into(),
                    stream_id: "legacy_stream_must_not_open".into(),
                    path: "/api/v1/agent/threads/thread_legacy/events".into(),
                },
                CancellationToken::new(),
            ))
            .unwrap_err();
        assert_eq!(subscription.code, METHOD_NOT_FOUND);
        assert_eq!(subscription.data.application_code, "PYTHON_SSE_RETIRED");
        let cursor = AppServerCursor::new(
            "thread_legacy",
            Some("turn_legacy".into()),
            1,
            CursorPhase::Item,
            Some("item_legacy".into()),
        )
        .encode()
        .unwrap();
        let replay = runtime
            .block_on(CompatibilityHttpPort::replay(
                bridge.inner.port.as_ref(),
                ReplayParams { cursor },
                CancellationToken::new(),
            ))
            .unwrap_err();
        assert_eq!(replay.code, METHOD_NOT_FOUND);
        assert_eq!(replay.data.application_code, "PYTHON_SSE_RETIRED");
        assert_eq!(
            backend.state.lock().unwrap().requested_paths,
            ["GET /api/health"],
            "legacy subscribe/replay must not open or reconnect a Python SSE stream"
        );

        drop(bridge);
        drop(rust_core);
        drop(backend);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn legacy_conversion_public_http_build_segment_commit_is_restart_idempotent_and_preserves_source(
    ) {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::Success);
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-k003-legacy-public-http-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let project_id = "prj_k003_legacy_public_http";
        let legacy_version_id = "ver_k003_legacy_public_http";
        let legacy_graph_id = "mg_k003_legacy_public_http";
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("legacy-public-http-{serial}"))
                .expect("open Rust core for legacy bridge conversion"),
        );
        rust_core
            .repository()
            .create_project(&forgecad_core::Project {
                project_id: project_id.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "K003 legacy public HTTP conversion".into(),
                status: forgecad_core::ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-17T00:00:00Z".into(),
                updated_at: "2026-07-17T00:00:00Z".into(),
            })
            .unwrap();
        let database = library_root.join("library.db");
        seed_bridge_legacy_source(&database, project_id, legacy_version_id, legacy_graph_id);
        let legacy_before =
            bridge_legacy_semantic_hash(&database, project_id, legacy_version_id, legacy_graph_id);
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .expect("construct production bridge for legacy conversion");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let endpoint = LocalAgentEndpoint::parse(&backend.endpoint).unwrap();
        let execute = |bridge: &AppServerBridge,
                       method: AllowedHttpMethod,
                       path: &str,
                       headers: Vec<(String, String)>,
                       body: Value| {
            runtime
                .block_on(bridge.execute_k003_packaged_compat(
                    PreparedCompatHttpRequest {
                        endpoint: endpoint.clone(),
                        method,
                        path: path.into(),
                        headers,
                        body: if body.is_null() {
                            ProtocolHttpBody::Empty
                        } else {
                            ProtocolHttpBody::Utf8 {
                                data: body.to_string(),
                            }
                        },
                    },
                    CancellationToken::new(),
                ))
                .unwrap()
        };
        let post = |bridge: &AppServerBridge, path: &str, key: &str, body: Value| {
            execute(
                bridge,
                AllowedHttpMethod::Post,
                path,
                vec![
                    ("Content-Type".into(), "application/json".into()),
                    ("Idempotency-Key".into(), key.into()),
                ],
                body,
            )
        };

        let conversion_body = json!({
            "client_request_id":"authorize_k003_legacy_public_http",
            "snapshot_revision":1
        });
        let converted = execute(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/projects/{project_id}/active-design:convert-legacy"),
            vec![
                ("Content-Type".into(), "application/json".into()),
                (
                    "Idempotency-Key".into(),
                    "authorize_k003_legacy_public_http".into(),
                ),
                ("If-Match".into(), "W/\"active-design-1\"".into()),
            ],
            conversion_body.clone(),
        );
        assert_eq!(converted.status, 200, "{}", compat_json(&converted));
        let converted_json = compat_json(&converted);
        assert_eq!(converted_json["status"], "ready_for_agent_rebuild");
        assert_eq!(converted_json["snapshot_revision"], 1);
        assert_eq!(
            converted_json["source"]["legacy_version_id"],
            legacy_version_id
        );
        assert_eq!(converted_json["source"]["module_graph_id"], legacy_graph_id);
        let conversion_replay = execute(
            &bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/projects/{project_id}/active-design:convert-legacy"),
            vec![
                ("Content-Type".into(), "application/json".into()),
                (
                    "Idempotency-Key".into(),
                    "authorize_k003_legacy_public_http".into(),
                ),
                ("If-Match".into(), "W/\"active-design-1\"".into()),
            ],
            conversion_body,
        );
        assert_eq!(compat_json(&conversion_replay), converted_json);
        assert_eq!(
            rust_core
                .repository()
                .legacy_conversion_intent(project_id)
                .unwrap()
                .unwrap()
                .snapshot_revision,
            1
        );
        assert_eq!(
            bridge_legacy_semantic_hash(&database, project_id, legacy_version_id, legacy_graph_id,),
            legacy_before
        );

        let plan = bridge_blockout_plan(project_id, "plan_k003_legacy_public_http");
        let build_body = json!({
            "client_request_id":"build_k003_legacy_public_http",
            "plan":plan,
            "direction_id":"direction_primary",
            "variation_index":0,
            "presentation_profile":"quick_sketch"
        });
        let built = post(
            &bridge,
            "/api/v1/agent/blockouts",
            "build_k003_legacy_public_http",
            build_body.clone(),
        );
        assert_eq!(built.status, 200, "{}", compat_json(&built));
        let built_json = compat_json(&built);
        let artifact_id = built_json["artifact_id"].as_str().unwrap();
        let segmented = post(
            &bridge,
            "/api/v1/agent/blockouts:segment",
            "segment_k003_legacy_public_http",
            json!({
                "client_request_id":"segment_k003_legacy_public_http",
                "plan":plan,
                "direction_id":"direction_primary",
                "variant_id":built_json["variant_id"],
                "variation_index":0,
                "presentation_profile":"quick_sketch",
                "artifact_id":artifact_id
            }),
        );
        assert_eq!(segmented.status, 200, "{}", compat_json(&segmented));
        assert_eq!(compat_json(&segmented)["segmentation_status"], "candidate");
        let commit_body = json!({
            "client_request_id":"commit_k003_legacy_public_http",
            "artifact_id":artifact_id,
            "project_id":project_id,
            "summary":"K003 legacy-authorized Agent rebuild"
        });
        let committed = post(
            &bridge,
            "/api/v1/agent/blockouts:commit",
            "commit_k003_legacy_public_http",
            commit_body.clone(),
        );
        assert_eq!(committed.status, 201, "{}", compat_json(&committed));
        let committed_json = compat_json(&committed);
        let asset_version_id = committed_json["asset_version_id"].as_str().unwrap();
        let snapshot = rust_core
            .repository()
            .snapshot(project_id)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.revision, 2);
        assert_eq!(
            snapshot.active_design.asset_version_id(),
            Some(asset_version_id)
        );
        assert!(snapshot.quality.is_some());
        assert!(rust_core
            .repository()
            .legacy_conversion_intent(project_id)
            .unwrap()
            .is_none());
        assert_eq!(
            bridge_legacy_semantic_hash(&database, project_id, legacy_version_id, legacy_graph_id,),
            legacy_before,
            "legacy ProjectVersion and ModuleGraph semantics must remain read-only"
        );
        let commit_replay = post(
            &bridge,
            "/api/v1/agent/blockouts:commit",
            "commit_k003_legacy_public_http",
            commit_body.clone(),
        );
        assert_eq!(compat_json(&commit_replay), committed_json);

        drop(bridge);
        drop(rust_core);
        let reopened_core = Arc::new(
            RustCoreRuntime::open(
                &library_root,
                format!("legacy-public-http-restart-{serial}"),
            )
            .expect("reopen Rust core after legacy conversion"),
        );
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let reopened_bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&reopened_core),
        )
        .expect("reconstruct production bridge after legacy conversion");
        let restart_replay = post(
            &reopened_bridge,
            "/api/v1/agent/blockouts:commit",
            "commit_k003_legacy_public_http",
            commit_body,
        );
        assert_eq!(compat_json(&restart_replay), committed_json);
        let stale_conversion = execute(
            &reopened_bridge,
            AllowedHttpMethod::Post,
            &format!("/api/v1/projects/{project_id}/active-design:convert-legacy"),
            vec![
                ("Content-Type".into(), "application/json".into()),
                (
                    "Idempotency-Key".into(),
                    "authorize_after_agent_activation".into(),
                ),
                ("If-Match".into(), "W/\"active-design-2\"".into()),
            ],
            json!({
                "client_request_id":"authorize_after_agent_activation",
                "snapshot_revision":2
            }),
        );
        assert_eq!(stale_conversion.status, 409);
        assert_eq!(
            compat_json(&stale_conversion)["error"]["code"],
            "ACTIVE_DESIGN_NOT_LEGACY"
        );

        let second_plan = bridge_blockout_plan(project_id, "plan_existing_agent_reject");
        let second_built = post(
            &reopened_bridge,
            "/api/v1/agent/blockouts",
            "build_existing_agent_reject",
            json!({
                "client_request_id":"build_existing_agent_reject",
                "plan":second_plan,
                "direction_id":"direction_primary",
                "variation_index":0,
                "presentation_profile":"quick_sketch"
            }),
        );
        assert_eq!(second_built.status, 200);
        let rejected = post(
            &reopened_bridge,
            "/api/v1/agent/blockouts:commit",
            "commit_existing_agent_reject",
            json!({
                "client_request_id":"commit_existing_agent_reject",
                "artifact_id":compat_json(&second_built)["artifact_id"],
                "project_id":project_id,
                "summary":"must not replace existing Agent Snapshot"
            }),
        );
        assert_eq!(rejected.status, 409);
        assert_eq!(
            compat_json(&rejected)["error"]["code"],
            "BLOCKOUT_PROJECT_ALREADY_INITIALIZED"
        );
        let restart_snapshot = reopened_core
            .repository()
            .snapshot(project_id)
            .unwrap()
            .unwrap();
        assert_eq!(restart_snapshot.revision, 2);
        assert_eq!(
            restart_snapshot.active_design.asset_version_id(),
            Some(asset_version_id)
        );
        assert_eq!(
            bridge_legacy_semantic_hash(&database, project_id, legacy_version_id, legacy_graph_id,),
            legacy_before
        );

        drop(reopened_bridge);
        drop(reopened_core);
        drop(backend);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn loopback_routes_m103_registration_query_and_material_enrichment_to_rust_core() {
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-k003-m103-loopback-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("m103-loopback-{serial}"))
                .expect("open M103 Rust core"),
        );
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            "http://127.0.0.1:8000",
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let endpoint = LocalAgentEndpoint::parse("http://127.0.0.1:9").unwrap();
        let registration_body = json!({
            "display_name":"M103 Loopback Texture",
            "texture_role":"base_color",
            "mime_type":"image/png",
            "payload_base64":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNg+M/AAAADAQEAGN2NtAAAAABJRU5ErkJggg==",
            "source":"user_created",
            "license":"self_declared_original"
        });
        let register = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: "/api/v1/agent/material-textures".into(),
                    headers: vec![("Idempotency-Key".into(), "m103-loopback-register".into())],
                    body: ProtocolHttpBody::Utf8 {
                        data: serde_json::to_string(&registration_body).unwrap(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(register.status, 201);
        let ProtocolHttpBody::Utf8 { data } = register.body else {
            panic!("M103 registration must return JSON");
        };
        let created: Value = serde_json::from_str(&data).unwrap();
        assert_eq!(created["visual_only"], true);
        assert_eq!(created["object_exists"], true);

        let list = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Get,
                    path: "/api/v1/agent/material-textures?texture_role=base_color&q=Loopback"
                        .into(),
                    headers: Vec::new(),
                    body: ProtocolHttpBody::Empty,
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(list.status, 200);
        let ProtocolHttpBody::Utf8 { data } = list.body else {
            panic!("M103 list must return JSON");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&data).unwrap()["items"],
            json!([created])
        );

        let materials = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint,
                    method: AllowedHttpMethod::Get,
                    path: "/api/v1/agent/materials?domain=future_weapon_prop".into(),
                    headers: Vec::new(),
                    body: ProtocolHttpBody::Empty,
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(materials.status, 200);
        let ProtocolHttpBody::Utf8 { data } = materials.body else {
            panic!("material catalog must return JSON");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&data)
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            13
        );

        drop(bridge);
        drop(rust_core);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn loopback_routes_external_glb_import_and_readback_to_rust_without_python() {
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-k003-external-glb-loopback-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("external-glb-loopback-{serial}"))
                .expect("open external GLB Rust core"),
        );
        let project_id = "prj_external_glb_loopback";
        rust_core
            .repository()
            .create_project(&forgecad_core::Project {
                project_id: project_id.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "External GLB loopback".into(),
                status: forgecad_core::ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-17T00:00:00Z".into(),
                updated_at: "2026-07-17T00:00:00Z".into(),
            })
            .unwrap();
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            "http://127.0.0.1:8000",
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // Port 9 has no Python sidecar. Success therefore proves the
        // compatibility adapter terminated at the Rust-owned route.
        let endpoint = LocalAgentEndpoint::parse("http://127.0.0.1:9").unwrap();
        let glb = fake_forgecad_profile_glb("production_concept");
        let imported = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: "/api/v1/agent/imports:glb".into(),
                    headers: vec![(
                        "Idempotency-Key".into(),
                        "external-glb-loopback-import".into(),
                    )],
                    body: ProtocolHttpBody::Utf8 {
                        data: json!({
                            "client_request_id":"external-glb-loopback-import",
                            "project_id":project_id,
                            "domain_pack_id":"pack_vehicle_concept",
                            "file_name":"loopback-reference.glb",
                            "glb_base64":BASE64.encode(&glb),
                            "summary":"Loopback external reference"
                        })
                        .to_string(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(imported.status, 201);
        let ProtocolHttpBody::Utf8 { data } = imported.body else {
            panic!("external GLB import must return JSON");
        };
        let imported_json: Value = serde_json::from_str(&data).unwrap();
        let asset_version_id = imported_json["asset_version"]["asset_version_id"]
            .as_str()
            .unwrap();
        assert_eq!(
            imported_json["asset_version"]["shape_program"]["schema_version"],
            "ExternalGLBReference@1"
        );

        let preview = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint,
                    method: AllowedHttpMethod::Get,
                    path: format!("/api/v1/agent/asset-versions/{asset_version_id}:preview.glb"),
                    headers: Vec::new(),
                    body: ProtocolHttpBody::Empty,
                },
                CancellationToken::new(),
            ))
            .unwrap();
        assert_eq!(preview.status, 200);
        assert!(preview.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("x-forgecad-artifact-profile")
                && value == "external_reference"
        }));
        assert!(!preview.headers.iter().any(|(name, _)| {
            name.eq_ignore_ascii_case("x-forgecad-shape-program-sha256")
                || name.eq_ignore_ascii_case("x-forgecad-artifact-profile-sha256")
        }));
        let ProtocolHttpBody::Base64 { data } = preview.body else {
            panic!("external GLB preview must return binary base64");
        };
        assert_eq!(BASE64.decode(data).unwrap(), glb);

        drop(bridge);
        drop(rust_core);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn rust_blockout_compat_build_segment_commit_owns_glb_snapshot_quality_and_cas() {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::Success);
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-k003-blockout-compat-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("blockout-compat-test-{serial}"))
                .expect("open unpublished Rust core"),
        );
        let project_id = "prj_k003_blockout_compat";
        rust_core
            .repository()
            .create_project(&forgecad_core::Project {
                project_id: project_id.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "K003 Rust blockout compatibility".into(),
                status: forgecad_core::ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-17T00:00:00Z".into(),
                updated_at: "2026-07-17T00:00:00Z".into(),
            })
            .unwrap();
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .expect("construct production bridge");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let direction = |direction_id: &str, title: &str| {
            json!({
                "direction_id": direction_id,
                "title": title,
                "summary": "Complete non-functional exterior concept.",
                "silhouette": "compact",
                "primary_part_roles": ["primary_form", "secondary_form"],
                "material_direction": "dark metal visual finish"
            })
        };
        let plan = json!({
            "schema_version": "MechanicalConceptPlan@1",
            "plan_id": "plan_k003_blockout_compat",
            "domain_pack_id": "pack_future_weapon_prop",
            "brief": "non-functional future game prop exterior",
            "generation_stage": "blockout",
            "spec": {"project_id": project_id},
            "directions": [
                direction("direction_primary", "Primary")
            ],
            "provider_id": "rust_app_server",
            "shape_program_ready": false
        });
        let execute_with_cancellation =
            |path: &str, key: &str, body: Value, cancellation: CancellationToken| {
                runtime
                    .block_on(CompatibilityHttpPort::execute(
                        bridge.inner.port.as_ref(),
                        PreparedCompatHttpRequest {
                            endpoint: LocalAgentEndpoint::parse(&backend.endpoint).unwrap(),
                            method: AllowedHttpMethod::Post,
                            path: path.to_string(),
                            headers: vec![
                                ("Content-Type".into(), "application/json".into()),
                                ("Idempotency-Key".into(), key.into()),
                            ],
                            body: ProtocolHttpBody::Utf8 {
                                data: body.to_string(),
                            },
                        },
                        cancellation,
                    ))
                    .unwrap()
            };
        let execute = |path: &str, key: &str, body: Value| {
            execute_with_cancellation(path, key, body, CancellationToken::new())
        };
        let execute_get = |path: &str| {
            runtime
                .block_on(CompatibilityHttpPort::execute(
                    bridge.inner.port.as_ref(),
                    PreparedCompatHttpRequest {
                        endpoint: LocalAgentEndpoint::parse(&backend.endpoint).unwrap(),
                        method: AllowedHttpMethod::Get,
                        path: path.to_string(),
                        headers: Vec::new(),
                        body: ProtocolHttpBody::Empty,
                    },
                    CancellationToken::new(),
                ))
                .unwrap()
        };
        let json_body = |response: &CompatHttpResponse| {
            let ProtocolHttpBody::Utf8 { data } = &response.body else {
                panic!("compat response must be JSON text");
            };
            serde_json::from_str::<Value>(data).unwrap()
        };

        let build_body = json!({
            "client_request_id": "build_k003_blockout_compat",
            "plan": plan,
            "direction_id": "direction_primary",
            "variation_index": 2,
            "presentation_profile": "quick_sketch"
        });
        let built = execute(
            "/api/v1/agent/blockouts",
            "build_k003_blockout_compat",
            build_body.clone(),
        );
        assert_eq!(built.status, 200, "{}", json_body(&built));
        let built_json = json_body(&built);
        assert_eq!(built_json["triangle_count"], 4);
        assert_eq!(
            built_json["shape_program"]["schema_version"],
            "ShapeProgram@1"
        );
        let glb = BASE64
            .decode(built_json["glb_base64"].as_str().unwrap())
            .unwrap();
        assert_eq!(glb.get(..4), Some(b"glTF".as_slice()));
        let legacy_artifact_id = built_json["artifact_id"].as_str().unwrap();
        let legacy_preview_id = bridge
            .inner
            .port
            .inner
            .native_blockouts
            .lock()
            .unwrap()
            .candidates
            .get(legacy_artifact_id)
            .unwrap()
            .preview_id
            .clone();
        assert!(bridge
            .preview_artifact(&legacy_preview_id)
            .unwrap()
            .unwrap()
            .formal_provenance
            .is_none());
        assert_eq!(
            backend.state.lock().unwrap().requested_paths.len(),
            2,
            "build must call only restricted compile and render"
        );
        let replay = execute(
            "/api/v1/agent/blockouts",
            "build_k003_blockout_compat",
            build_body.clone(),
        );
        assert_eq!(json_body(&replay), built_json);
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 2);

        let mut conflicting_build = build_body;
        // Keep the conflicting replay independently valid under V003's
        // single-direction plan contract.  The same idempotency key with a
        // different valid presentation profile must still fail as an
        // idempotency conflict.
        conflicting_build["presentation_profile"] = json!("showcase");
        let conflict = execute(
            "/api/v1/agent/blockouts",
            "build_k003_blockout_compat",
            conflicting_build,
        );
        assert_eq!(conflict.status, 409);
        assert_eq!(
            json_body(&conflict)["error"]["code"],
            "IDEMPOTENCY_CONFLICT"
        );
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 2);

        let artifact_id = built_json["artifact_id"].as_str().unwrap();
        let segmented = execute(
            "/api/v1/agent/blockouts:segment",
            "segment_k003_blockout_compat",
            json!({
                "client_request_id": "segment_k003_blockout_compat",
                "plan": plan,
                "direction_id": "direction_primary",
                "variant_id": built_json["variant_id"],
                "variation_index": 2,
                "presentation_profile": "quick_sketch",
                "artifact_id": artifact_id
            }),
        );
        assert_eq!(segmented.status, 200);
        let segmented_json = json_body(&segmented);
        assert_eq!(segmented_json["segmentation_status"], "candidate");
        assert_eq!(segmented_json["parts"].as_array().unwrap().len(), 2);
        let bindings = segmented_json["parts"][0]["editable_parameter_bindings"]
            .as_array()
            .unwrap();
        assert_eq!(bindings.len(), 1);
        assert!(bindings.iter().all(|binding| {
            binding["min"] == json!(0.6)
                && binding["max"] == json!(1.4)
                && binding["step"] == json!(0.1)
        }));
        assert_eq!(bindings[0]["path"], "transform.scale.y");
        assert_eq!(
            segmented_json["assembly_graph"]["root_part_id"],
            segmented_json["parts"][0]["part_id"]
        );
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 2);

        let commit_body = json!({
            "client_request_id": "commit_k003_blockout_compat",
            "artifact_id": artifact_id,
            "project_id": project_id,
            "summary": "K003 Rust-owned editable concept"
        });
        // A cancellation and transient-consume race after the final check is
        // intentionally too late to override the atomic bundle result.
        let late_cancellation = CancellationToken::new();
        let cancel_after_final_check = late_cancellation.clone();
        let transient_to_remove = bridge
            .inner
            .port
            .inner
            .native_blockouts
            .lock()
            .unwrap()
            .candidates
            .get(artifact_id)
            .cloned()
            .unwrap();
        let cleanup_race_executor = Arc::clone(
            bridge
                .inner
                .native_product_tools
                .as_ref()
                .expect("production bridge native executor"),
        );
        *bridge
            .inner
            .port
            .inner
            .native_commit_before_bundle_hook
            .lock()
            .unwrap() = Some(Box::new(move || {
            cancel_after_final_check.cancel();
            cleanup_race_executor
                .consume_preview(
                    &transient_to_remove.preview_id,
                    &transient_to_remove.turn_id,
                )
                .expect("simulate transient cleanup racing the committed bundle");
        }));
        let committed = execute_with_cancellation(
            "/api/v1/agent/blockouts:commit",
            "commit_k003_blockout_compat",
            commit_body.clone(),
            late_cancellation.clone(),
        );
        assert!(late_cancellation.is_cancelled());
        assert_eq!(
            committed.status,
            201,
            "late cancellation must not override atomic commit: {}",
            json_body(&committed)
        );
        let committed_json = json_body(&committed);
        let asset_version_id = committed_json["asset_version_id"].as_str().unwrap();
        assert_eq!(committed_json["version_no"], 1);
        assert_eq!(committed_json["project_id"], project_id);
        let committed_version = rust_core
            .repository()
            .version(asset_version_id)
            .unwrap()
            .unwrap();
        let committed_part_id = committed_version.parts[0]["part_id"].as_str().unwrap();
        let committed_zone_id = committed_version.parts[0]["material_zone_ids"][0]
            .as_str()
            .unwrap();
        assert!(committed_version
            .material_bindings
            .contains_key(&format!("{committed_part_id}:{committed_zone_id}")));
        let snapshot = rust_core
            .repository()
            .snapshot(project_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            snapshot.active_design.asset_version_id(),
            Some(asset_version_id)
        );
        assert!(snapshot.quality.is_some());
        for role in ["interactive_preview_glb", "production_glb"] {
            let object = rust_core
                .repository()
                .object_for_reference(&ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: asset_version_id.into(),
                    role: role.into(),
                })
                .unwrap()
                .unwrap();
            let bytes = rust_core.repository().read_object(&object.sha256).unwrap();
            let expected_profile = if role == "interactive_preview_glb" {
                assert_eq!(bytes, glb);
                "interactive_preview"
            } else {
                assert_ne!(bytes, glb);
                "production_concept"
            };
            verify_forgecad_glb(&bytes, Some(expected_profile)).unwrap();
        }
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 4);
        assert!(backend
            .state
            .lock()
            .unwrap()
            .requested_paths
            .iter()
            .all(|path| path.starts_with("POST /api/v1/internal/geometry/")));
        let commit_replay = execute(
            "/api/v1/agent/blockouts:commit",
            "commit_k003_blockout_compat",
            commit_body.clone(),
        );
        assert_eq!(json_body(&commit_replay), committed_json);
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 4);
        // Existing version rows are not enough for replay success: remove one
        // required role and require the Core's complete-bundle conflict.
        rust_core
            .repository()
            .detach_object(&ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: asset_version_id.into(),
                role: "interactive_preview_glb".into(),
            })
            .unwrap();
        let incomplete_replay = execute(
            "/api/v1/agent/blockouts:commit",
            "commit_k003_blockout_compat",
            commit_body,
        );
        assert_eq!(incomplete_replay.status, 409);
        assert_eq!(
            json_body(&incomplete_replay)["error"]["code"],
            "CANDIDATE_BUNDLE_INCOMPLETE"
        );
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 4);
        let consumed_segment = execute(
            "/api/v1/agent/blockouts:segment",
            "segment_k003_blockout_consumed",
            json!({
                "client_request_id": "segment_k003_blockout_consumed",
                "plan": plan,
                "direction_id": "direction_primary",
                "variant_id": built_json["variant_id"],
                "variation_index": 2,
                "presentation_profile": "quick_sketch",
                "artifact_id": artifact_id
            }),
        );
        assert_eq!(consumed_segment.status, 404);
        assert_eq!(
            json_body(&consumed_segment)["error"]["code"],
            "BLOCKOUT_NOT_FOUND"
        );
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 4);

        // Force authoritative state to appear after geometry but before the
        // one transaction. The bundle must roll back every one of its rows,
        // while the still-unconfirmed transient candidate remains usable.
        let failure_project_id = "prj_k003_blockout_atomic_failure";
        rust_core
            .repository()
            .create_project(&forgecad_core::Project {
                project_id: failure_project_id.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "K003 atomic bundle failure".into(),
                status: forgecad_core::ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-17T00:00:00Z".into(),
                updated_at: "2026-07-17T00:00:00Z".into(),
            })
            .unwrap();
        let failure_plan = json!({
            "schema_version": "MechanicalConceptPlan@1",
            "plan_id": "plan_k003_blockout_atomic_failure",
            "domain_pack_id": "pack_future_weapon_prop",
            "brief": "non-functional future game prop exterior",
            "generation_stage": "blockout",
            "spec": {"project_id": failure_project_id},
            "directions": [
                direction("direction_failure_primary", "Primary")
            ],
            "provider_id": "rust_app_server",
            "shape_program_ready": false
        });
        let failure_build = execute(
            "/api/v1/agent/blockouts",
            "build_k003_blockout_atomic_failure",
            json!({
                "client_request_id": "build_k003_blockout_atomic_failure",
                "plan": failure_plan,
                "direction_id": "direction_failure_primary",
                "variation_index": 0,
                "presentation_profile": "quick_sketch"
            }),
        );
        assert_eq!(failure_build.status, 200);
        let failure_built_json = json_body(&failure_build);
        let failure_artifact_id = failure_built_json["artifact_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 6);

        let intervening_version = AgentAssetVersion {
            asset_version_id: "assetver_k003_intervening_head".into(),
            project_id: failure_project_id.into(),
            parent_asset_version_id: None,
            version_no: 1,
            status: AssetVersionStatus::Committed,
            summary: "Intervening committed state".into(),
            stage: AssetStage::EditableAsset,
            plan_id: "plan_k003_intervening".into(),
            direction_id: "direction_k003_intervening".into(),
            domain_pack_id: "pack_future_weapon_prop".into(),
            artifact_id: "artifact_k003_intervening".into(),
            parts: vec![json!({"part_id": "part_k003_intervening"})],
            shape_program: json!({
                "schema_version": "ShapeProgram@1",
                "program_id": "shape_k003_intervening"
            }),
            assembly_graph: json!({
                "schema_version": "AssemblyGraph@1",
                "graph_id": "graph_k003_intervening",
                "parts": [{
                    "part_id": "part_k003_intervening",
                    "material_zone_ids": []
                }]
            }),
            material_bindings: BTreeMap::new(),
            created_at: "2026-07-17T00:00:01Z".into(),
        };
        let failure_repository = rust_core.repository().clone();
        *bridge
            .inner
            .port
            .inner
            .native_commit_before_bundle_hook
            .lock()
            .unwrap() = Some(Box::new(move || {
            failure_repository
                .commit_initial_asset(&intervening_version)
                .unwrap();
        }));
        let failure_commit_id = "commit_k003_blockout_atomic_failure";
        let failed_commit = execute(
            "/api/v1/agent/blockouts:commit",
            failure_commit_id,
            json!({
                "client_request_id": failure_commit_id,
                "artifact_id": failure_artifact_id,
                "project_id": failure_project_id,
                "summary": "Must roll back the entire candidate bundle"
            }),
        );
        assert_eq!(failed_commit.status, 409);
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 8);
        let failed_asset_version_id = format!(
            "assetver_{}",
            &sha256_hex(failure_commit_id.as_bytes())[..24]
        );
        let failed_quality_report_id = native_blockout_quality_report_id(&failed_asset_version_id);
        assert!(rust_core
            .repository()
            .candidate(&failure_artifact_id)
            .unwrap()
            .is_none());
        assert!(rust_core
            .repository()
            .version(&failed_asset_version_id)
            .unwrap()
            .is_none());
        assert!(rust_core
            .repository()
            .quality_report(&failed_quality_report_id)
            .unwrap()
            .is_none());
        for role in ["interactive_preview_glb", "production_glb"] {
            assert!(rust_core
                .repository()
                .object_for_reference(&ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: failed_asset_version_id.clone(),
                    role: role.into(),
                })
                .unwrap()
                .is_none());
        }
        let surviving_failure_candidate = execute(
            "/api/v1/agent/blockouts:segment",
            "segment_k003_blockout_after_atomic_failure",
            json!({
                "client_request_id": "segment_k003_blockout_after_atomic_failure",
                "plan": failure_plan,
                "direction_id": "direction_failure_primary",
                "variant_id": failure_built_json["variant_id"],
                "variation_index": 0,
                "presentation_profile": "quick_sketch",
                "artifact_id": failure_artifact_id
            }),
        );
        assert_eq!(surviving_failure_candidate.status, 200);
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 8);

        let rendered = execute_get(&format!(
            "/api/v1/agent/asset-versions/{asset_version_id}:render?width=128&height=128"
        ));
        assert_eq!(rendered.status, 200, "{}", json_body(&rendered));
        let rendered_json = json_body(&rendered);
        assert_eq!(rendered_json["asset_version_id"], asset_version_id);
        assert_eq!(rendered_json["views"].as_array().unwrap().len(), 4);
        assert_eq!(rendered_json["exploded_view_available"], false);
        let render_set_sha256 = rendered_json["render_set_sha256"].as_str().unwrap();
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 10);

        let package = execute_get(&format!(
            "/api/v1/agent/asset-versions/{asset_version_id}:render-package?width=128&height=128&render_set_sha256={render_set_sha256}"
        ));
        assert_eq!(package.status, 200);
        assert!(package.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("x-forgecad-render-set-sha256") && value == render_set_sha256
        }));
        let ProtocolHttpBody::Base64 { data } = package.body else {
            panic!("render package must be binary");
        };
        assert!(BASE64.decode(data).unwrap().starts_with(b"PK\x03\x04"));
        assert_eq!(backend.state.lock().unwrap().requested_paths.len(), 12);

        drop(bridge);
        drop(rust_core);
        drop(backend);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn rust_change_set_compat_seals_preview_glb_and_confirms_one_atomic_bundle() {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::Success);
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-k003-change-set-compat-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("change-set-compat-{serial}"))
                .expect("open unpublished Rust core"),
        );
        let project_id = "prj_k003_change_set_compat";
        rust_core
            .repository()
            .create_project(&forgecad_core::Project {
                project_id: project_id.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "K003 Rust ChangeSet compatibility".into(),
                status: forgecad_core::ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-17T00:00:00Z".into(),
                updated_at: "2026-07-17T00:00:00Z".into(),
            })
            .unwrap();
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .expect("construct production bridge");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let execute = |method: AllowedHttpMethod,
                       path: &str,
                       idempotency_key: Option<&str>,
                       body: ProtocolHttpBody,
                       cancellation: CancellationToken| {
            let mut headers = Vec::new();
            if !matches!(&body, ProtocolHttpBody::Empty) {
                headers.push(("Content-Type".into(), "application/json".into()));
            }
            if let Some(key) = idempotency_key {
                headers.push(("Idempotency-Key".into(), key.into()));
            }
            runtime
                .block_on(bridge.execute_k003_packaged_compat(
                    PreparedCompatHttpRequest {
                        endpoint: LocalAgentEndpoint::parse(&backend.endpoint).unwrap(),
                        method,
                        path: path.into(),
                        headers,
                        body,
                    },
                    cancellation,
                ))
                .unwrap()
        };
        let post_json = |path: &str, key: &str, body: Value| {
            execute(
                AllowedHttpMethod::Post,
                path,
                Some(key),
                ProtocolHttpBody::Utf8 {
                    data: body.to_string(),
                },
                CancellationToken::new(),
            )
        };
        let empty_post = |path: &str, key: &str, cancellation: CancellationToken| {
            execute(
                AllowedHttpMethod::Post,
                path,
                Some(key),
                ProtocolHttpBody::Empty,
                cancellation,
            )
        };
        let json_body = |response: &CompatHttpResponse| {
            let ProtocolHttpBody::Utf8 { data } = &response.body else {
                panic!("compat response must be JSON text");
            };
            serde_json::from_str::<Value>(data).unwrap()
        };
        let direction = |direction_id: &str| {
            json!({
                "direction_id": direction_id,
                "title": "Direction",
                "summary": "Complete non-functional exterior concept.",
                "silhouette": "compact",
                "primary_part_roles": ["primary_form", "secondary_form"],
                "material_direction": "dark metal visual finish"
            })
        };
        let plan = json!({
            "schema_version": "MechanicalConceptPlan@1",
            "plan_id": "plan_k003_change_set_compat",
            "domain_pack_id": "pack_future_weapon_prop",
            "brief": "non-functional future game prop exterior",
            "generation_stage": "blockout",
            "spec": {"project_id": project_id},
            "directions": [
                direction("direction_primary")
            ],
            "provider_id": "rust_app_server",
            "shape_program_ready": false
        });
        let built = post_json(
            "/api/v1/agent/blockouts",
            "build_k003_change_set_compat",
            json!({
                "client_request_id": "build_k003_change_set_compat",
                "plan": plan,
                "direction_id": "direction_primary",
                "variation_index": 0,
                "presentation_profile": "quick_sketch"
            }),
        );
        assert_eq!(built.status, 200, "{}", json_body(&built));
        let built_json = json_body(&built);
        let compatibility_preview_id = bridge
            .inner
            .port
            .inner
            .native_blockouts
            .lock()
            .unwrap()
            .candidates
            .get(built_json["artifact_id"].as_str().unwrap())
            .unwrap()
            .preview_id
            .clone();
        let compatibility_preview = bridge
            .preview_artifact(&compatibility_preview_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            compatibility_preview.readback.artifact_profile_id,
            "interactive_preview"
        );
        assert!(compatibility_preview.formal_provenance.is_none());
        let committed = post_json(
            "/api/v1/agent/blockouts:commit",
            "commit_k003_change_set_compat",
            json!({
                "client_request_id": "commit_k003_change_set_compat",
                "artifact_id": built_json["artifact_id"],
                "project_id": project_id,
                "summary": "K003 editable base asset"
            }),
        );
        assert_eq!(committed.status, 201);
        let base_version_id = json_body(&committed)["asset_version_id"]
            .as_str()
            .unwrap()
            .to_string();
        let base = rust_core
            .repository()
            .version(&base_version_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            base.assembly_graph, built_json["assembly_graph"],
            "C105 commit must retain the exact reviewed AssemblyGraph instead of rebuilding it"
        );
        assert!(base.assembly_graph["component_recipe_instances"].is_array());
        assert!(base.assembly_graph["connections"].is_array());
        let allowed_part_keys = BTreeSet::from([
            "part_id",
            "role",
            "parent_part_id",
            "position_mm",
            "size_mm",
            "material_zone_ids",
            "editable_parameters",
            "editable_parameter_bindings",
            "locked",
            "provenance",
        ]);
        for part in &base.parts {
            assert!(
                part.as_object()
                    .unwrap()
                    .keys()
                    .all(|key| allowed_part_keys.contains(key.as_str())),
                "persisted C105 parts must obey AgentAssetVersion@1 additionalProperties=false"
            );
        }
        let part_id = base.parts[0]["part_id"].as_str().unwrap().to_string();
        let material_zone_id = base.parts[0]["material_zone_ids"][0]
            .as_str()
            .unwrap()
            .to_string();
        let base_graph_scale = base.assembly_graph["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|part| part["part_id"] == part_id)
            .and_then(|part| part["transform"].get("scale"))
            .map(|value| native_change_set_vec3(Some(value), 0.1, 10.0, "scale"))
            .transpose()
            .unwrap()
            .unwrap();
        let disabled_adornment = post_json(
            &format!("/api/v1/agent/asset-versions/{base_version_id}/surface-adornments:preview"),
            "surface_adornment_disabled",
            json!({
                "schema_version": "SurfaceAdornmentPreviewRequest@1",
                "client_request_id": "surface_adornment_disabled",
                "part_id": part_id,
                "material_zone_id": material_zone_id,
                "kind": "normal_relief",
                "motif": "parallel_groove",
                "intensity": "subtle",
                "coverage": "center_band"
            }),
        );
        assert_eq!(disabled_adornment.status, 409);
        assert_eq!(
            json_body(&disabled_adornment)["error"]["code"],
            "SURFACE_ADORNMENT_SKILL_DISABLED"
        );
        let enabled_adornment = post_json(
            "/api/v1/agent/skills/surface-adornment:enable",
            "surface_adornment_enable",
            json!({
                "schema_version": "EnableSurfaceAdornmentSkillRequest@1",
                "client_request_id": "surface_adornment_enable",
                "confirm_enable": true
            }),
        );
        assert_eq!(
            enabled_adornment.status,
            200,
            "{}",
            json_body(&enabled_adornment)
        );
        let enabled_adornment_json = json_body(&enabled_adornment);
        assert_eq!(enabled_adornment_json["status"], "enabled");
        let enabled_activation = &enabled_adornment_json["activation"];
        // The fixture now activates immutable A005 v2.  Bind the later
        // ChangeSet provenance to this exact activation rather than merely
        // accepting any positive skill version.
        assert_eq!(enabled_activation["skill_version"], 2);
        let enabled_skill_sha256 = enabled_activation["skill_sha256"]
            .as_str()
            .expect("enabled immutable A005 v2 activation must carry a hash")
            .to_string();
        assert_eq!(enabled_skill_sha256.len(), 64);
        let legacy_v1 = rust_core
            .repository()
            .skill_manifest("skill_first_party_surface_adornment", 1)
            .unwrap()
            .expect("the immutable legacy A005 v1 manifest remains sealed");
        assert_eq!(legacy_v1.version, 1);
        assert_ne!(
            legacy_v1.canonical_sha256().unwrap(),
            enabled_skill_sha256,
            "legacy A005 v1 must remain distinct from the C106-capable v2 activation"
        );
        let proposed_adornment = post_json(
            &format!("/api/v1/agent/asset-versions/{base_version_id}/surface-adornments:preview"),
            "surface_adornment_propose",
            json!({
                "schema_version": "SurfaceAdornmentPreviewRequest@1",
                "client_request_id": "surface_adornment_propose",
                "part_id": part_id,
                "material_zone_id": material_zone_id,
                "kind": "normal_relief",
                "motif": "parallel_groove",
                "intensity": "subtle",
                "coverage": "center_band"
            }),
        );
        assert_eq!(
            proposed_adornment.status,
            201,
            "{}",
            json_body(&proposed_adornment)
        );
        let adornment_change_set_id = json_body(&proposed_adornment)["change_set_id"]
            .as_str()
            .unwrap()
            .to_string();
        let adornment_change_set = rust_core
            .repository()
            .change_set(&adornment_change_set_id)
            .unwrap()
            .unwrap();
        let adornment_preview =
            native_change_set_apply(rust_core.repository(), &base, &adornment_change_set).unwrap();
        let sealed_program = &adornment_preview.assembly_graph["surface_adornments"][0];
        assert_eq!(sealed_program["target_part_id"], part_id);
        assert_eq!(sealed_program["target_zone_id"], material_zone_id);
        assert_eq!(
            sealed_program["skill_version"],
            enabled_activation["skill_version"]
        );
        assert_eq!(
            sealed_program["skill_sha256"],
            enabled_activation["skill_sha256"]
        );
        assert_ne!(
            sealed_program["skill_sha256"],
            legacy_v1.canonical_sha256().unwrap(),
            "a v1 hash must not be substituted into a v2-sealed preview"
        );
        assert!(
            adornment_preview.material_bindings[&format!("{part_id}:{material_zone_id}")]
                .as_str()
                .unwrap()
                .starts_with("mat_a005_")
        );
        let previewed_adornment = empty_post(
            &format!("/api/v1/agent/change-sets/{adornment_change_set_id}:preview"),
            "surface_adornment_compile_preview",
            CancellationToken::new(),
        );
        assert_eq!(
            previewed_adornment.status,
            200,
            "{}",
            json_body(&previewed_adornment)
        );
        let restricted_compile = backend
            .state
            .lock()
            .unwrap()
            .request_bodies
            .iter()
            .rev()
            .find(|body| body["action"] == "compile_readback")
            .cloned()
            .unwrap();
        assert_eq!(
            restricted_compile["surface_adornment_programs"][0]["program_id"],
            sealed_program["program_id"]
        );
        assert_eq!(
            restricted_compile["surface_adornment_programs"][0]["skill_sha256"],
            sealed_program["skill_sha256"]
        );
        let rejected_adornment = empty_post(
            &format!("/api/v1/agent/change-sets/{adornment_change_set_id}:reject"),
            "surface_adornment_reject",
            CancellationToken::new(),
        );
        assert_eq!(rejected_adornment.status, 200);
        let persisted_candidate = rust_core
            .repository()
            .candidate(built_json["artifact_id"].as_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            persisted_candidate.candidate["component_recipe_candidate_sha256"]
                .as_str()
                .map(str::len),
            Some(64),
            "C105 candidate identity belongs to persisted candidate metadata, not the graph schema"
        );
        let base_interactive_object = rust_core
            .repository()
            .object_for_reference(&ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: base_version_id.clone(),
                role: "interactive_preview_glb".into(),
            })
            .unwrap()
            .unwrap();
        let base_interactive_bounds = verify_forgecad_glb(
            &rust_core
                .repository()
                .read_object(&base_interactive_object.sha256)
                .unwrap(),
            Some("interactive_preview"),
        )
        .unwrap()
        .bounds_mm;
        let operation_id = native_change_set_part_operation_id(&base, &part_id).unwrap();
        let (_, base_size, _) = native_change_set_base_part_facts(&base, &part_id).unwrap();
        let fixture_change_set = AgentAssetChangeSet {
            change_set_id: "changeset_k003_real_parameter_fixture".into(),
            project_id: project_id.into(),
            base_asset_version_id: base_version_id.clone(),
            summary: "Increase primary exterior profile height".into(),
            operations: vec![json!({
                "operation_id": "operation_k003_scale_y",
                "op": "set_part_parameter",
                "part_id": part_id.clone(),
                "path": "transform.scale.y",
                "value": 1.1
            })],
            protected_part_ids: Vec::new(),
            preview: None,
            status: ChangeSetStatus::Proposed,
            resulting_asset_version_id: None,
            created_at: "2026-07-17T00:00:02Z".into(),
            updated_at: "2026-07-17T00:00:02Z".into(),
        };
        let raw_fixture_preview =
            native_change_set_apply(rust_core.repository(), &base, &fixture_change_set).unwrap();
        let normalized_fixture_shape =
            forgecad_core::normalize_persisted_shape_program(&raw_fixture_preview.shape_program)
                .unwrap();
        let normalized_fixture_again =
            forgecad_core::normalize_persisted_shape_program(&normalized_fixture_shape).unwrap();
        assert_eq!(normalized_fixture_shape, normalized_fixture_again);
        if let Some(difference) = first_json_difference(
            &raw_fixture_preview.shape_program,
            &normalized_fixture_shape,
            "$/shape_program",
        ) {
            assert!(difference.contains("path=") && difference.contains("before_type="));
        }

        let saved_component = post_json(
            &format!("/api/v1/agent/asset-versions/{base_version_id}/components"),
            "save_k003_component",
            json!({
                "client_request_id": "save_k003_component",
                "part_id": part_id,
                "display_name": "K003 production-readback component",
                "description": "Project-local visual concept component"
            }),
        );
        assert_eq!(
            saved_component.status,
            201,
            "{}",
            json_body(&saved_component)
        );
        let saved_component_json = json_body(&saved_component);
        let component_id = saved_component_json["component_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(saved_component_json["source_quality_status"], "passed");
        let saved_component_replay = post_json(
            &format!("/api/v1/agent/asset-versions/{base_version_id}/components"),
            "save_k003_component",
            json!({
                "client_request_id": "save_k003_component",
                "part_id": part_id,
                "display_name": "K003 production-readback component",
                "description": "Project-local visual concept component"
            }),
        );
        assert_eq!(json_body(&saved_component_replay), saved_component_json);

        let listed_components = execute(
            AllowedHttpMethod::Get,
            &format!("/api/v1/agent/components?project_id={project_id}"),
            None,
            ProtocolHttpBody::Empty,
            CancellationToken::new(),
        );
        assert_eq!(listed_components.status, 200);
        assert_eq!(
            json_body(&listed_components)[0]["component_id"],
            component_id
        );
        let compatible_components = execute(
            AllowedHttpMethod::Get,
            &format!(
                "/api/v1/agent/asset-versions/{base_version_id}/components:compatible?part_id={part_id}"
            ),
            None,
            ProtocolHttpBody::Empty,
            CancellationToken::new(),
        );
        assert_eq!(compatible_components.status, 200);
        assert_eq!(
            json_body(&compatible_components)[0]["compatibility"]["eligible"],
            true
        );
        let structure_suggestions = execute(
            AllowedHttpMethod::Get,
            &format!("/api/v1/agent/asset-versions/{base_version_id}/structure-suggestions"),
            None,
            ProtocolHttpBody::Empty,
            CancellationToken::new(),
        );
        assert_eq!(structure_suggestions.status, 200);
        assert_eq!(
            json_body(&structure_suggestions)["schema_version"],
            "AgentStructureSuggestionList@1"
        );

        let proposed = post_json(
            &format!("/api/v1/agent/asset-versions/{base_version_id}/change-sets"),
            "propose_k003_change_set",
            json!({
                "client_request_id": "propose_k003_change_set",
                "summary": "Increase primary exterior profile height",
                "operations": [
                    {
                        "operation_id": "operation_k003_scale_y",
                        "op": "set_part_parameter",
                        "part_id": part_id,
                        "path": "transform.scale.y",
                        "value": 1.1
                    }
                ]
            }),
        );
        assert_eq!(proposed.status, 201, "{}", json_body(&proposed));
        let change_set_id = json_body(&proposed)["change_set_id"]
            .as_str()
            .unwrap()
            .to_string();
        let preview_path = format!("/api/v1/agent/change-sets/{change_set_id}:preview");
        let geometry_calls_before_preview = backend.state.lock().unwrap().requested_paths.len();
        let previewed = empty_post(
            &preview_path,
            "preview_k003_change_set",
            CancellationToken::new(),
        );
        assert_eq!(previewed.status, 200, "{}", json_body(&previewed));
        let previewed_json = json_body(&previewed);
        assert_eq!(previewed_json["status"], "previewed");
        assert_eq!(
            backend.state.lock().unwrap().requested_paths.len(),
            geometry_calls_before_preview + 2
        );
        let preview_bundle = rust_core
            .repository()
            .read_change_set_preview_bundle(&change_set_id)
            .unwrap()
            .unwrap();
        assert!(rust_core
            .repository()
            .version(&preview_bundle.sealed_preview.asset_version_id)
            .unwrap()
            .is_none());
        assert_eq!(
            rust_core.repository().head(project_id).unwrap().as_deref(),
            Some(base_version_id.as_str())
        );
        assert_eq!(
            preview_bundle
                .snapshot
                .preview
                .as_ref()
                .map(|preview| preview.change_set_id.as_str()),
            Some(change_set_id.as_str())
        );
        let expected_size = [base_size[0], base_size[1] * 1.1, base_size[2]];
        let preview_part = preview_bundle
            .sealed_preview
            .parts
            .iter()
            .find(|part| part["part_id"] == part_id)
            .unwrap();
        assert_eq!(
            native_change_set_vec3(preview_part.get("size_mm"), 0.0001, 100_000.0, "size_mm")
                .unwrap(),
            expected_size,
            "the profile-height parameter edit must update the Part facts"
        );
        let preview_graph_part = preview_bundle.sealed_preview.assembly_graph["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|part| part["part_id"] == part_id)
            .unwrap();
        assert_eq!(
            native_change_set_vec3(
                preview_graph_part["transform"].get("scale"),
                0.1,
                10.0,
                "scale",
            )
            .unwrap(),
            [base_graph_scale[0], 1.1, base_graph_scale[2]],
            "AssemblyGraph must retain the profile-height parameter edit"
        );
        let preview_shape_operation = preview_bundle.sealed_preview.shape_program["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|operation| operation["operation_id"] == operation_id)
            .unwrap();
        assert_eq!(
            preview_shape_operation["op"], "sweep",
            "C105 root must remain the reviewed sweep operation"
        );
        let base_profile_height = base.shape_program["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|operation| operation["operation_id"] == operation_id)
            .unwrap()["args"]["profile_scale"][1]
            .as_f64()
            .unwrap();
        let preview_profile_height = preview_shape_operation["args"]["profile_scale"][1]
            .as_f64()
            .unwrap();
        assert!(
            (preview_profile_height - base_profile_height * 1.1).abs() <= 1e-9,
            "Sweep profile scale must change in the same preview ShapeProgram"
        );
        let preview_compile_request =
            backend.state.lock().unwrap().request_bodies[geometry_calls_before_preview].clone();
        assert_eq!(preview_compile_request["action"], "compile_readback");
        assert_eq!(
            preview_compile_request["shape_program"], preview_bundle.sealed_preview.shape_program,
            "restricted geometry must compile the merged ShapeProgram instead of the base program"
        );

        let preview_glb = execute(
            AllowedHttpMethod::Get,
            &format!("/api/v1/agent/change-sets/{change_set_id}:preview.glb"),
            None,
            ProtocolHttpBody::Empty,
            CancellationToken::new(),
        );
        assert_eq!(preview_glb.status, 200);
        let ProtocolHttpBody::Base64 { data } = &preview_glb.body else {
            panic!("ChangeSet preview GLB must use the binary protocol body");
        };
        let preview_bytes = BASE64.decode(data).unwrap();
        let verified_preview =
            verify_forgecad_glb(&preview_bytes, Some("interactive_preview")).unwrap();
        assert_eq!(
            verified_preview.glb_sha256,
            preview_bundle.interactive_preview_glb.sha256
        );
        assert_eq!(
            preview_bytes,
            rust_core
                .repository()
                .read_object(&preview_bundle.interactive_preview_glb.sha256)
                .unwrap()
        );
        assert_ne!(
            preview_bundle.interactive_preview_glb.sha256, base_interactive_object.sha256,
            "Sweep profile edit must produce a different compiled GLB"
        );
        assert_ne!(
            verified_preview.bounds_mm, base_interactive_bounds,
            "Sweep profile edit must produce different readback bounds"
        );
        let header = |name: &str| {
            preview_glb
                .headers
                .iter()
                .find(|(header, _)| header.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(
            header("X-ForgeCAD-Preview-GLB-SHA256"),
            Some(verified_preview.glb_sha256.as_str())
        );
        assert_eq!(
            header("X-ForgeCAD-Base-Asset-Version-ID"),
            Some(base_version_id.as_str())
        );
        let expected_triangle_count = verified_preview.triangle_count.to_string();
        assert_eq!(
            header("X-ForgeCAD-Preview-Triangle-Count"),
            Some(expected_triangle_count.as_str())
        );
        let calls_after_preview = backend.state.lock().unwrap().requested_paths.len();
        let preview_replay = empty_post(
            &preview_path,
            "preview_k003_change_set",
            CancellationToken::new(),
        );
        assert_eq!(json_body(&preview_replay), previewed_json);
        assert_eq!(
            backend.state.lock().unwrap().requested_paths.len(),
            calls_after_preview
        );

        let competing = post_json(
            &format!("/api/v1/agent/asset-versions/{base_version_id}/change-sets"),
            "propose_k003_competing_change_set",
            json!({
                "client_request_id": "propose_k003_competing_change_set",
                "summary": "Competing edit must not replace active preview",
                "operations": [{
                    "operation_id": "operation_k003_competing_scale_y",
                    "op": "set_part_parameter",
                    "part_id": part_id,
                    "path": "transform.scale.y",
                    "value": 1.1
                }]
            }),
        );
        assert_eq!(competing.status, 201);
        let competing_id = json_body(&competing)["change_set_id"]
            .as_str()
            .unwrap()
            .to_string();
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let cancelled_preview = empty_post(
            &format!("/api/v1/agent/change-sets/{competing_id}:preview"),
            "preview_k003_competing_cancelled",
            cancelled,
        );
        assert_eq!(cancelled_preview.status, 409);
        assert_eq!(
            json_body(&cancelled_preview)["error"]["code"],
            "REQUEST_CANCELLED"
        );
        assert!(rust_core
            .repository()
            .read_change_set_preview_bundle(&competing_id)
            .unwrap()
            .is_none());
        let competing_preview = empty_post(
            &format!("/api/v1/agent/change-sets/{competing_id}:preview"),
            "preview_k003_competing",
            CancellationToken::new(),
        );
        assert_eq!(competing_preview.status, 409);
        assert_eq!(
            json_body(&competing_preview)["error"]["code"],
            "ACTIVE_DESIGN_PREVIEW_PENDING"
        );
        assert_eq!(
            backend.state.lock().unwrap().requested_paths.len(),
            calls_after_preview
        );

        let late_cancellation = CancellationToken::new();
        let cancel_after_final_check = late_cancellation.clone();
        *bridge
            .inner
            .port
            .inner
            .native_commit_before_bundle_hook
            .lock()
            .unwrap() = Some(Box::new(move || cancel_after_final_check.cancel()));
        let confirm_path = format!("/api/v1/agent/change-sets/{change_set_id}:confirm");
        let confirmed = empty_post(
            &confirm_path,
            "confirm_k003_change_set",
            late_cancellation.clone(),
        );
        assert!(late_cancellation.is_cancelled());
        assert_eq!(confirmed.status, 200, "{}", json_body(&confirmed));
        let confirmed_json = json_body(&confirmed);
        assert_eq!(confirmed_json["change_set"]["status"], "confirmed");
        let resulting_asset_version_id = confirmed_json["asset_version"]["asset_version_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(
            confirmed_json["change_set"]["resulting_asset_version_id"],
            resulting_asset_version_id
        );
        assert_eq!(
            backend.state.lock().unwrap().requested_paths.len(),
            calls_after_preview + 2
        );
        let production_compile_request =
            backend.state.lock().unwrap().request_bodies[calls_after_preview].clone();
        assert_eq!(production_compile_request["action"], "compile_readback");
        assert_eq!(
            production_compile_request["shape_program"],
            preview_bundle.sealed_preview.shape_program,
            "production confirmation must recompile the same merged ShapeProgram"
        );
        let confirm_replay = empty_post(
            &confirm_path,
            "confirm_k003_change_set",
            CancellationToken::new(),
        );
        assert_eq!(json_body(&confirm_replay), confirmed_json);
        assert_eq!(
            backend.state.lock().unwrap().requested_paths.len(),
            calls_after_preview + 2
        );

        let confirmed_bundle = rust_core
            .repository()
            .read_change_set_confirm_bundle(
                &change_set_id,
                &resulting_asset_version_id,
                &native_blockout_quality_report_id(&resulting_asset_version_id),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            confirmed_bundle.change_set.status,
            ChangeSetStatus::Confirmed
        );
        assert!(confirmed_bundle.snapshot.preview.is_none());
        assert_eq!(
            confirmed_bundle.snapshot.active_design.asset_version_id(),
            Some(resulting_asset_version_id.as_str())
        );
        assert_eq!(
            confirmed_bundle
                .snapshot
                .quality
                .as_ref()
                .map(|quality| quality.asset_version_id.as_str()),
            Some(resulting_asset_version_id.as_str())
        );
        assert!(rust_core
            .repository()
            .object_for_reference(&ObjectReference {
                reference_kind: "preview".into(),
                owner_id: change_set_id.clone(),
                role: "interactive_preview_glb".into(),
            })
            .unwrap()
            .is_none());
        for role in ["interactive_preview_glb", "production_glb"] {
            assert!(rust_core
                .repository()
                .object_for_reference(&ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: resulting_asset_version_id.clone(),
                    role: role.into(),
                })
                .unwrap()
                .is_some());
        }

        let replace_change_set = post_json(
            &format!("/api/v1/agent/asset-versions/{resulting_asset_version_id}/change-sets"),
            "propose_k003_component_replace",
            json!({
                "client_request_id": "propose_k003_component_replace",
                "summary": "Replace the edited visual part from the verified project component",
                "operations": [{
                    "operation_id": "operation_k003_component_replace",
                    "op": "replace_part",
                    "part_id": part_id,
                    "replacement_component_id": component_id
                }]
            }),
        );
        assert_eq!(
            replace_change_set.status,
            201,
            "{}",
            json_body(&replace_change_set)
        );
        let replace_change_set_id = json_body(&replace_change_set)["change_set_id"]
            .as_str()
            .unwrap()
            .to_string();
        let geometry_calls_before_replace = backend.state.lock().unwrap().requested_paths.len();
        let replace_preview = empty_post(
            &format!("/api/v1/agent/change-sets/{replace_change_set_id}:preview"),
            "preview_k003_component_replace",
            CancellationToken::new(),
        );
        assert_eq!(
            replace_preview.status,
            200,
            "{}",
            json_body(&replace_preview)
        );
        assert_eq!(
            backend.state.lock().unwrap().requested_paths.len(),
            geometry_calls_before_replace + 2,
            "component replacement must execute compile_readback and preview render"
        );
        let replacement_bundle = rust_core
            .repository()
            .read_change_set_preview_bundle(&replace_change_set_id)
            .unwrap()
            .unwrap();
        let replacement_part = replacement_bundle
            .sealed_preview
            .parts
            .iter()
            .find(|part| part["part_id"] == part_id)
            .unwrap();
        assert_eq!(
            native_change_set_vec3(
                replacement_part.get("size_mm"),
                0.0001,
                100_000.0,
                "size_mm",
            )
            .unwrap(),
            base_size,
            "verified component geometry must replace the previously scaled part"
        );
        let replacement_compile_request =
            backend.state.lock().unwrap().request_bodies[geometry_calls_before_replace].clone();
        assert_eq!(replacement_compile_request["action"], "compile_readback");
        assert_eq!(
            replacement_compile_request["shape_program"],
            replacement_bundle.sealed_preview.shape_program,
            "restricted geometry must compile the recomputed component replacement"
        );

        // R007 must only turn reference evidence into a normal, new-asset
        // ChangeSet.  First clear the unrelated C105 preview so the reference
        // route exercises the same active-head/Snapshot boundary a user sees.
        let replacement_rejected = empty_post(
            &format!("/api/v1/agent/change-sets/{replace_change_set_id}:reject"),
            "reject_k003_component_replace_before_r007",
            CancellationToken::new(),
        );
        assert_eq!(
            replacement_rejected.status,
            200,
            "{}",
            json_body(&replacement_rejected)
        );
        assert_eq!(
            rust_core.repository().head(project_id).unwrap().as_deref(),
            Some(resulting_asset_version_id.as_str())
        );
        assert!(rust_core
            .repository()
            .snapshot(project_id)
            .unwrap()
            .unwrap()
            .preview
            .is_none());

        let evidence_key = "reference_evidence_k003_lifecycle";
        let evidence_created = post_json(
            "/api/v1/agent/reference-evidence:create",
            evidence_key,
            json!({
                "schema_version": "ReferenceEvidenceCreateRequest@1",
                "client_request_id": evidence_key,
                "project_id": project_id,
                "kind": "image",
                "reference_class": "single_image",
                "file_name": "authorized-prop-reference.png",
                "media_type": "image/png",
                "content_base64": "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGPQjVnwHx9mGBkKANXiigEwD3bkAAAAAElFTkSuQmCC",
                "source_statement": "User supplied this image as local visual reference.",
                "license_statement": "User declares local reference rights.",
                "missing_views": ["rear", "top"],
                "user_notes": "Visible dark shell, bright trim and compact layered silhouette.",
                "domain_pack_id": "pack_future_weapon_prop"
            }),
        );
        assert_eq!(
            evidence_created.status,
            201,
            "{}",
            json_body(&evidence_created)
        );
        let evidence_id = json_body(&evidence_created)["reference_evidence"]["evidence_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(
            rust_core.repository().head(project_id).unwrap().as_deref(),
            Some(resulting_asset_version_id.as_str()),
            "creating read-only evidence must not move the active head"
        );

        // R007B is deliberately fail-closed for this legacy future-prop C105
        // base. Only an exact active C106 robotic-arm root may receive a
        // surface-analysis plan. The rejection must happen before it can
        // create any plan, ChangeSet, preview, head mutation, or geometry work.
        let rejected_request_id = "reference_rebuild_non_c106_base";
        let snapshot_before_rejection = rust_core.repository().snapshot(project_id).unwrap();
        let head_before_rejection = rust_core.repository().head(project_id).unwrap();
        let change_set_count_before_rejection = Connection::open(library_root.join("library.db"))
            .unwrap()
            .query_row("SELECT COUNT(*) FROM agent_asset_change_sets", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        let rebuild_plan_count_before_rejection = Connection::open(library_root.join("library.db"))
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM reference_guided_rebuild_plans",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        let geometry_calls_before_rejection = backend.state.lock().unwrap().requested_paths.len();
        let rejected_proposal = post_json(
            &format!("/api/v1/agent/projects/{project_id}/reference-guided-rebuild:preview"),
            rejected_request_id,
            json!({
                "schema_version": "ReferenceGuidedRebuildPreviewRequest@1",
                "client_request_id": rejected_request_id,
                "evidence_id": evidence_id,
                "domain_pack_id": "pack_future_weapon_prop",
                "base_asset_version_id": resulting_asset_version_id
            }),
        );
        assert_eq!(
            rejected_proposal.status,
            409,
            "{}",
            json_body(&rejected_proposal)
        );
        assert_eq!(
            json_body(&rejected_proposal)["error"]["code"],
            "REFERENCE_REBUILD_C106_BASE_REQUIRED"
        );
        assert_eq!(
            Connection::open(library_root.join("library.db"))
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM reference_guided_rebuild_plans",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            rebuild_plan_count_before_rejection,
            "non-C106 rejection must not create a rebuild plan"
        );
        assert_eq!(
            Connection::open(library_root.join("library.db"))
                .unwrap()
                .query_row("SELECT COUNT(*) FROM agent_asset_change_sets", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            change_set_count_before_rejection,
            "non-C106 rejection must not create a ChangeSet"
        );
        assert_eq!(
            rust_core.repository().snapshot(project_id).unwrap(),
            snapshot_before_rejection,
            "non-C106 rejection must not create an interactive preview or alter the Snapshot"
        );
        assert_eq!(
            rust_core.repository().head(project_id).unwrap(),
            head_before_rejection,
            "non-C106 rejection must not move the active head"
        );
        assert_eq!(
            backend.state.lock().unwrap().requested_paths.len(),
            geometry_calls_before_rejection,
            "non-C106 rejection must not invoke the restricted geometry executor"
        );

        drop(bridge);
        drop(rust_core);
        let reopened =
            RustCoreRuntime::open(&library_root, format!("change-set-compat-restart-{serial}"))
                .expect("reopen Rust core after component replacement preview");
        assert_eq!(
            reopened
                .repository()
                .component(&component_id)
                .unwrap()
                .unwrap()
                .component_id,
            component_id,
            "project component must survive a Rust runtime restart"
        );
        let restarted_evidence = reopened
            .repository()
            .reference_evidence(&evidence_id)
            .unwrap()
            .expect("read-only reference evidence must survive a Rust runtime restart");
        assert_eq!(restarted_evidence.evidence_id, evidence_id);
        assert_eq!(restarted_evidence.project_id, project_id);
        assert_eq!(
            restarted_evidence.domain_pack_id,
            "pack_future_weapon_prop",
            "restart must preserve the immutable evidence boundary independently of a rejected rebuild"
        );
        let restarted_snapshot = reopened.repository().snapshot(project_id).unwrap().unwrap();
        assert!(restarted_snapshot.preview.is_none());
        assert_eq!(
            restarted_snapshot.active_design.asset_version_id(),
            Some(resulting_asset_version_id.as_str())
        );
        assert_eq!(
            reopened.repository().head(project_id).unwrap().as_deref(),
            Some(resulting_asset_version_id.as_str())
        );
        drop(reopened);
        drop(backend);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn rust_blockout_compat_c105_recipe_lifecycle_all_domains() {
        fn object_manifest(root: &Path) -> Vec<(String, u64)> {
            fn visit(path: &Path, root: &Path, rows: &mut Vec<(String, u64)>) {
                let entries = match fs::read_dir(path) {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
                    Err(error) => panic!("read Recipe lifecycle object directory: {error}"),
                };
                for entry in entries {
                    let entry = entry.expect("read Recipe lifecycle object entry");
                    let path = entry.path();
                    let metadata = entry
                        .metadata()
                        .expect("read Recipe lifecycle object metadata");
                    if metadata.is_dir() {
                        visit(&path, root, rows);
                    } else if metadata.is_file() {
                        rows.push((
                            path.strip_prefix(root)
                                .expect("object path stays below library root")
                                .display()
                                .to_string(),
                            metadata.len(),
                        ));
                    }
                }
            }

            let mut rows = Vec::new();
            visit(&root.join("objects").join("sha256"), root, &mut rows);
            rows.sort();
            rows
        }

        fn table_counts(db_path: &Path) -> Vec<(String, i64)> {
            let connection = Connection::open(db_path).expect("open Recipe lifecycle database");
            [
                "projects",
                "agent_asset_versions",
                "agent_asset_heads",
                "active_design_snapshots",
                "agent_asset_change_sets",
                "agent_blockout_candidates",
                "forgecad_core_objects",
                "forgecad_core_object_references",
            ]
            .into_iter()
            .map(|table| {
                let count = connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .expect("read Recipe lifecycle table count");
                (table.to_string(), count)
            })
            .collect()
        }

        let backend = FakeGeometryBackend::start(FakeGeometryScenario::Success);
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-c105-recipe-lifecycle-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("c105-recipe-lifecycle-{serial}"))
                .expect("open C105 lifecycle Rust core"),
        );
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .expect("construct C105 lifecycle bridge");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let execute = |method: AllowedHttpMethod,
                       path: &str,
                       idempotency_key: Option<&str>,
                       if_match: Option<String>,
                       body: ProtocolHttpBody| {
            let mut headers = Vec::new();
            if !matches!(&body, ProtocolHttpBody::Empty) {
                headers.push(("Content-Type".into(), "application/json".into()));
            }
            if let Some(key) = idempotency_key {
                headers.push(("Idempotency-Key".into(), key.into()));
            }
            if let Some(etag) = if_match {
                headers.push(("If-Match".into(), etag));
            }
            runtime
                .block_on(bridge.execute_k003_packaged_compat(
                    PreparedCompatHttpRequest {
                        endpoint: LocalAgentEndpoint::parse(&backend.endpoint).unwrap(),
                        method,
                        path: path.into(),
                        headers,
                        body,
                    },
                    CancellationToken::new(),
                ))
                .unwrap()
        };
        let post_json = |path: &str, key: &str, body: Value| {
            execute(
                AllowedHttpMethod::Post,
                path,
                Some(key),
                None,
                ProtocolHttpBody::Utf8 {
                    data: body.to_string(),
                },
            )
        };
        let empty_post = |path: &str, key: &str| {
            execute(
                AllowedHttpMethod::Post,
                path,
                Some(key),
                None,
                ProtocolHttpBody::Empty,
            )
        };
        let json_body = |response: &CompatHttpResponse| {
            let ProtocolHttpBody::Utf8 { data } = &response.body else {
                panic!("C105 lifecycle response must be JSON text");
            };
            serde_json::from_str::<Value>(data).unwrap()
        };

        let mut restart_expectations = Vec::new();
        // This test normally has no filesystem side effects outside its
        // disposable Rust Core library.  The aggregate C105 lifecycle gate
        // may opt in to a separate, caller-owned evidence directory so the
        // *exact* active-replacement ShapeProgram can be compiled by the real
        // Python RestrictedGeometryExecutor.  Keeping the export behind an
        // environment variable means ordinary cargo tests retain their
        // existing isolated behaviour and never write checked-in evidence.
        let mut real_geometry_evidence = Vec::new();
        for (slug, domain_pack_id) in [
            ("prop", "pack_future_weapon_prop"),
            ("vehicle", "pack_vehicle_concept"),
            ("aircraft", "pack_aircraft_concept"),
            ("arm", "pack_robotic_arm_concept"),
        ] {
            let project_id = format!("prj_c105_lifecycle_{slug}");
            rust_core
                .repository()
                .create_project(&forgecad_core::Project {
                    project_id: project_id.clone(),
                    profile_id: "profile_weapon_concept_v1".into(),
                    // K003's persisted Project compatibility profile is still
                    // intentionally the one code-owned Alpha profile.  The
                    // verified Recipe domain is the immutable AssetVersion
                    // domain_pack_id created from the plan below.
                    domain_type: "weapon_concept".into(),
                    name: format!("C105 {slug} Recipe lifecycle"),
                    status: forgecad_core::ProjectStatus::Active,
                    current_version_id: None,
                    created_at: "2026-07-18T12:00:00Z".into(),
                    updated_at: "2026-07-18T12:00:00Z".into(),
                })
                .unwrap();
            let direction_id = format!("direction_c105_{slug}");
            let plan = json!({
                "schema_version":"MechanicalConceptPlan@1",
                "plan_id":format!("plan_c105_{slug}"),
                "domain_pack_id":domain_pack_id,
                "brief":"non-functional visual mechanical concept exterior",
                "generation_stage":"blockout",
                "spec":{"project_id":project_id},
                // V003 freezes a plan to one code-owned synthesis direction.
                // This C105 lifecycle fixture verifies the shared blockout and
                // Recipe path, not the retired three-direction chooser.
                "directions":[
                    {"direction_id":direction_id,"title":"Reviewed exterior","summary":"Complete non-functional visual concept.","silhouette":"compact","primary_part_roles":["primary_form","secondary_form"],"material_direction":"reviewed PBR visual finish"}
                ],
                "provider_id":"rust_app_server",
                "shape_program_ready":false
            });
            let build_key = format!("build_c105_{slug}");
            let built = post_json(
                "/api/v1/agent/blockouts",
                &build_key,
                json!({
                    "client_request_id":build_key,
                    "plan":plan,
                    "direction_id":direction_id,
                    "variation_index":0,
                    "presentation_profile":"quick_sketch"
                }),
            );
            assert_eq!(built.status, 200, "{}", json_body(&built));
            let built_json = json_body(&built);
            let artifact_id = built_json["artifact_id"].as_str().unwrap().to_string();
            let segment_key = format!("segment_c105_{slug}");
            let segmented = post_json(
                "/api/v1/agent/blockouts:segment",
                &segment_key,
                json!({
                    "client_request_id":segment_key,
                    "plan":plan,
                    "direction_id":direction_id,
                    "variant_id":built_json["variant_id"],
                    "variation_index":0,
                    "presentation_profile":"quick_sketch",
                    "artifact_id":artifact_id
                }),
            );
            assert_eq!(segmented.status, 200, "{}", json_body(&segmented));
            let commit_key = format!("commit_c105_{slug}");
            let committed = post_json(
                "/api/v1/agent/blockouts:commit",
                &commit_key,
                json!({
                    "client_request_id":commit_key,
                    "artifact_id":artifact_id,
                    "project_id":project_id,
                    "summary":"C105 reviewed Recipe base asset"
                }),
            );
            assert_eq!(committed.status, 201, "{}", json_body(&committed));
            let base_asset_version_id = json_body(&committed)["asset_version_id"]
                .as_str()
                .unwrap()
                .to_string();
            let base = rust_core
                .repository()
                .version(&base_asset_version_id)
                .unwrap()
                .unwrap();
            let (target_part_id, recipe_ref) = if slug == "prop" {
                // Exercise a genuine non-root replacement.  The first
                // reviewed prop child is attached below root, so its active
                // candidate must preserve the immutable parent's anchor and
                // translation rather than being silently re-rooted at zero.
                let target_part_id = base.assembly_graph["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|part| part["parent_part_id"].is_string())
                    .unwrap()["part_id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let recipe_ref = base.assembly_graph["component_recipe_instances"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|instance| instance["instance_path"] != "root")
                    .unwrap()["recipe"]
                    .clone();
                (target_part_id, recipe_ref)
            } else {
                let target_part_id = base.assembly_graph["root_part_id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let recipe_ref = base.assembly_graph["component_recipe_instances"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|instance| instance["instance_path"] == "root")
                    .unwrap()["recipe"]
                    .clone();
                (target_part_id, recipe_ref)
            };
            let base_production = rust_core
                .repository()
                .object_for_reference(&ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: base_asset_version_id.clone(),
                    role: "production_glb".into(),
                })
                .unwrap()
                .unwrap();
            let base_production_bytes = rust_core
                .repository()
                .read_object(&base_production.sha256)
                .unwrap();
            // Only the reviewed robotic-arm root exposes a fixed optional
            // visual-detail slot.  The chosen child ref is created from the
            // code-owned registry, then carried verbatim through candidate
            // expansion and the sealed ChangeSet so replay cannot invent a
            // free component selection.
            let recipe_slot_bindings = if recipe_ref["recipe_id"] == "recipe_robotic_arm_link" {
                let registry = forgecad_core::RecipeRegistry::from_embedded().unwrap();
                let detail = registry.recipe("recipe_robotic_arm_detail").unwrap();
                json!([{
                    "slot_id":"slot_arm_detail",
                    "child_recipe":{
                        "schema_version":"ComponentRecipeRef@1",
                        "recipe_id":detail.recipe_id,
                        "version":detail.version,
                        "recipe_sha256":forgecad_core::RecipeValidator::recipe_sha256(detail).unwrap()
                    }
                }])
            } else {
                json!([])
            };

            // A registry v2 is a new reviewed catalog, never an in-place
            // mutation of the v1 AssetVersion/CAS object.  Exercise the real
            // v1 production GLB held by this compatibility lifecycle while a
            // local v2 registry rejects the stale v1 ref and creates a
            // distinct candidate identity.
            if slug == "vehicle" {
                let objects_before_upgrade = object_manifest(&library_root);
                let v1_registry = forgecad_core::RecipeRegistry::from_embedded().unwrap();
                let v1_ref: forgecad_core::ComponentRecipeRef =
                    serde_json::from_value(recipe_ref.clone()).unwrap();
                let v1_request = forgecad_core::RecipeInstantiationRequest {
                    schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                    context_mode: "initial_candidate".into(),
                    request_id: "recipereq_c105_vehicle_v1_upgrade_fixture".into(),
                    project_id: None,
                    base_asset_version_id: None,
                    snapshot_revision: None,
                    domain_pack_id: domain_pack_id.into(),
                    recipe_registry_sha256: v1_registry.registry_sha256().into(),
                    recipe: v1_ref.clone(),
                    target_part_id: None,
                    slot_bindings: Vec::new(),
                    parameter_values: Vec::new(),
                    material_zone_overrides: Vec::new(),
                };
                let v1_candidate = forgecad_core::RecipeExpander::expand(
                    &v1_registry,
                    &v1_request,
                    &forgecad_core::RecipeExpansionPolicy::default(),
                )
                .unwrap();
                let mut v2_document: Value = serde_json::from_str(include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../packages/concept-spec/fixtures/editable-component-recipe-registry.json"
                )))
                .unwrap();
                let recipe = v2_document["recipes"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|recipe| recipe["recipe_id"] == "recipe_vehicle_body_shell")
                    .unwrap();
                recipe["version"] = json!(2);
                recipe["display_name"] = json!("车辆主壳配方 v2 lifecycle");
                let v2_registry = forgecad_core::RecipeRegistry::from_json(
                    &serde_json::to_string(&v2_document).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    forgecad_core::RecipeExpander::expand(
                        &v2_registry,
                        &v1_request,
                        &forgecad_core::RecipeExpansionPolicy::default(),
                    )
                    .unwrap_err()
                    .code(),
                    "COMPONENT_RECIPE_REGISTRY_STALE"
                );
                let mut v2_registry_request = v1_request.clone();
                v2_registry_request.recipe_registry_sha256 = v2_registry.registry_sha256().into();
                assert_eq!(
                    forgecad_core::RecipeExpander::expand(
                        &v2_registry,
                        &v2_registry_request,
                        &forgecad_core::RecipeExpansionPolicy::default(),
                    )
                    .unwrap_err()
                    .code(),
                    "COMPONENT_RECIPE_REFERENCE_STALE"
                );
                let v2_recipe = v2_registry.recipe("recipe_vehicle_body_shell").unwrap();
                let mut v2_request = v2_registry_request;
                v2_request.recipe = forgecad_core::ComponentRecipeRef {
                    schema_version: "ComponentRecipeRef@1".into(),
                    recipe_id: v2_recipe.recipe_id.clone(),
                    version: v2_recipe.version,
                    recipe_sha256: forgecad_core::RecipeValidator::recipe_sha256(v2_recipe)
                        .unwrap(),
                };
                let v2_candidate = forgecad_core::RecipeExpander::expand(
                    &v2_registry,
                    &v2_request,
                    &forgecad_core::RecipeExpansionPolicy::default(),
                )
                .unwrap();
                assert_ne!(v2_candidate.candidate_sha256, v1_candidate.candidate_sha256);
                assert_eq!(
                    rust_core
                        .repository()
                        .read_object(&base_production.sha256)
                        .unwrap(),
                    base_production_bytes,
                    "registry v2 must not rewrite the real v1 production GLB bytes"
                );
                assert_eq!(
                    rust_core
                        .repository()
                        .object_for_reference(&ObjectReference {
                            reference_kind: "asset_version".into(),
                            owner_id: base_asset_version_id.clone(),
                            role: "production_glb".into(),
                        })
                        .unwrap()
                        .unwrap()
                        .sha256,
                    base_production.sha256,
                    "registry v2 must not repoint the old immutable production reference"
                );
                assert_eq!(
                    object_manifest(&library_root),
                    objects_before_upgrade,
                    "pure v2 expansion must not add, remove, or rewrite CAS objects"
                );
            }

            // Ratio and material are independent C105 ChangeSet paths.  They
            // must get a real restricted preview but remain non-destructive
            // until confirm; reject keeps the Recipe base usable for replace.
            if slug == "vehicle" {
                let ratio = post_json(
                    &format!("/api/v1/agent/asset-versions/{base_asset_version_id}/change-sets"),
                    "propose_c105_vehicle_ratio",
                    json!({"client_request_id":"propose_c105_vehicle_ratio","summary":"Preview bounded Recipe height","operations":[{"operation_id":"changeop_c105_vehicle_ratio","op":"set_part_parameter","part_id":target_part_id,"path":"transform.scale.y","value":1.1}]}),
                );
                assert_eq!(ratio.status, 201, "{}", json_body(&ratio));
                let ratio_id = json_body(&ratio)["change_set_id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                assert_eq!(
                    empty_post(
                        &format!("/api/v1/agent/change-sets/{ratio_id}:preview"),
                        "preview_c105_vehicle_ratio"
                    )
                    .status,
                    200
                );
                assert_eq!(
                    empty_post(
                        &format!("/api/v1/agent/change-sets/{ratio_id}:reject"),
                        "reject_c105_vehicle_ratio"
                    )
                    .status,
                    200
                );
            }
            let zone_id = base
                .parts
                .iter()
                .find(|part| part["part_id"] == target_part_id)
                .and_then(|part| part["material_zone_ids"].as_array())
                .and_then(|zones| zones.first())
                .and_then(Value::as_str)
                .unwrap()
                .to_string();
            let material = post_json(
                &format!("/api/v1/agent/asset-versions/{base_asset_version_id}/change-sets"),
                &format!("propose_c105_{slug}_material"),
                json!({"client_request_id":format!("propose_c105_{slug}_material"),"summary":"Preview reviewed material zone","operations":[{"operation_id":format!("changeop_c105_{slug}_material"),"op":"apply_material_preset","part_id":target_part_id,"material_zone_id":zone_id,"material_id":"mat_painted_steel"}]}),
            );
            assert_eq!(material.status, 201, "{}", json_body(&material));
            let material_id = json_body(&material)["change_set_id"]
                .as_str()
                .unwrap()
                .to_string();
            assert_eq!(
                empty_post(
                    &format!("/api/v1/agent/change-sets/{material_id}:preview"),
                    &format!("preview_c105_{slug}_material")
                )
                .status,
                200
            );
            assert_eq!(
                empty_post(
                    &format!("/api/v1/agent/change-sets/{material_id}:reject"),
                    &format!("reject_c105_{slug}_material")
                )
                .status,
                200
            );

            let counts_before_candidate = table_counts(&library_root.join("library.db"));
            let objects_before_candidate = object_manifest(&library_root);
            let candidate = post_json(
                &format!("/api/v1/agent/asset-versions/{base_asset_version_id}/parts/{target_part_id}/component-recipes:expand"),
                &format!("expand_c105_{slug}"),
                json!({
                    "schema_version":"ComponentRecipeActiveCandidateRequest@1",
                    "recipe_request_id":format!("recipereq_c105_lifecycle_{slug}"),
                    "component_recipe_ref":recipe_ref,
                    "slot_bindings":recipe_slot_bindings,"parameter_values":[],"material_zone_overrides":[]
                }),
            );
            assert_eq!(candidate.status, 200, "{}", json_body(&candidate));
            let candidate = json_body(&candidate);
            assert_eq!(candidate["context_mode"], "active_asset_edit");
            assert_eq!(candidate["base_asset_version_id"], base_asset_version_id);
            let candidate_typed: forgecad_core::ExpandedComponentCandidate =
                serde_json::from_value(candidate.clone()).unwrap();
            assert_eq!(
                candidate["candidate_sha256"],
                forgecad_core::RecipeExpander::candidate_sha256(&candidate_typed).unwrap(),
                "active placement must be included in the sealed C105 candidate identity"
            );
            if slug == "prop" {
                let base_target = base.assembly_graph["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|part| part["part_id"] == target_part_id)
                    .unwrap();
                let candidate_root_id = candidate["expanded_assembly_graph"]["root_part_id"]
                    .as_str()
                    .unwrap();
                let candidate_root = candidate["expanded_assembly_graph"]["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|part| part["part_id"] == candidate_root_id)
                    .unwrap();
                assert_eq!(
                    candidate_root["parent_part_id"],
                    base_target["parent_part_id"]
                );
                assert_eq!(
                    candidate_root["transform"]["position"], base_target["transform"]["position"],
                    "non-root Recipe candidate must retain its immutable parent translation"
                );
            }
            assert_eq!(
                table_counts(&library_root.join("library.db")),
                counts_before_candidate,
                "C105 candidate expansion must write zero DB/version/Snapshot rows"
            );
            assert_eq!(
                object_manifest(&library_root),
                objects_before_candidate,
                "C105 candidate expansion must write zero CAS objects"
            );

            let replace = post_json(
                &format!("/api/v1/agent/asset-versions/{base_asset_version_id}/change-sets"),
                &format!("propose_c105_{slug}_recipe_replace"),
                json!({
                    "client_request_id":format!("propose_c105_{slug}_recipe_replace"),
                    "summary":"Replace with the sealed reviewed Recipe candidate",
                    "operations":[{
                        "operation_id":format!("changeop_c105_{slug}_recipe_replace"),
                        "op":"replace_part","part_id":target_part_id,
                        "recipe_request_id":candidate["request_id"],
                        "component_recipe_ref":candidate["recipe"],
                        "recipe_registry_sha256":candidate["registry_sha256"],
                        "recipe_slot_bindings":recipe_slot_bindings,
                        "recipe_candidate_id":candidate["candidate_id"],
                        "recipe_candidate_sha256":candidate["candidate_sha256"],
                        "recipe_snapshot_revision":candidate["snapshot_revision"]
                    }]
                }),
            );
            assert_eq!(replace.status, 201, "{}", json_body(&replace));
            let replace_id = json_body(&replace)["change_set_id"]
                .as_str()
                .unwrap()
                .to_string();
            let compile_calls_before_preview = backend.state.lock().unwrap().request_bodies.len();
            let previewed = empty_post(
                &format!("/api/v1/agent/change-sets/{replace_id}:preview"),
                &format!("preview_c105_{slug}_recipe_replace"),
            );
            assert_eq!(
                previewed.status,
                200,
                "slug={slug}: {}",
                json_body(&previewed)
            );
            let preview_bundle = rust_core
                .repository()
                .read_change_set_preview_bundle(&replace_id)
                .unwrap()
                .unwrap();
            if slug == "prop" {
                // A non-root replacement compiles the immutable ancestor and
                // the placed replacement together.  Candidate records remain
                // only the substituted subtree, so exact whole-program
                // equality would accidentally demand that ancestors vanish.
                for field in ["profile_inputs", "operations", "outputs"] {
                    let sealed = preview_bundle.sealed_preview.shape_program[field]
                        .as_array()
                        .unwrap();
                    let candidate_items = candidate["expanded_shape_program"][field]
                        .as_array()
                        .unwrap();
                    assert!(
                        candidate_items.iter().all(|item| sealed.contains(item)),
                        "placed non-root Recipe {field} must be carried into the Q003 program"
                    );
                }
                let base_root = base.assembly_graph["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|part| part["part_id"] == base.assembly_graph["root_part_id"])
                    .unwrap();
                let root_operation_id = base_root["operation_id"].as_str().unwrap();
                let root_output_id = base_root["output_id"].as_str().unwrap();
                assert!(
                    preview_bundle.sealed_preview.shape_program["profile_inputs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|input| base.shape_program["profile_inputs"]
                            .as_array()
                            .unwrap()
                            .contains(input)),
                    "the non-root candidate may not invent an unreviewed profile input"
                );
                assert!(
                    preview_bundle.sealed_preview.shape_program["operations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|operation| operation["operation_id"] == root_operation_id),
                    "unreplaced ancestor operation must survive a non-root Recipe replacement"
                );
                assert!(
                    preview_bundle.sealed_preview.shape_program["outputs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|output| output["output_id"] == root_output_id),
                    "unreplaced ancestor output must survive a non-root Recipe replacement"
                );
            } else {
                assert_eq!(
                    preview_bundle.sealed_preview.shape_program["profile_inputs"],
                    candidate["expanded_shape_program"]["profile_inputs"]
                );
                assert_eq!(
                    preview_bundle.sealed_preview.shape_program["operations"],
                    candidate["expanded_shape_program"]["operations"]
                );
                assert_eq!(
                    preview_bundle.sealed_preview.shape_program["outputs"],
                    candidate["expanded_shape_program"]["outputs"]
                );
            }
            let candidate_shape_sha =
                semantic_sha256(&preview_bundle.sealed_preview.shape_program).unwrap();
            assert_eq!(
                backend.state.lock().unwrap().request_bodies[compile_calls_before_preview]
                    ["shape_program"],
                preview_bundle.sealed_preview.shape_program,
                "preview must compile the exact sealed Recipe ShapeProgram"
            );
            let preview_glb = execute(
                AllowedHttpMethod::Get,
                &format!("/api/v1/agent/change-sets/{replace_id}:preview.glb"),
                None,
                None,
                ProtocolHttpBody::Empty,
            );
            assert_eq!(preview_glb.status, 200);
            let preview_shape_header = preview_glb
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("X-ForgeCAD-Shape-Program-SHA256"))
                .map(|(_, value)| value.as_str());
            assert_eq!(preview_shape_header, Some(candidate_shape_sha.as_str()));
            let ProtocolHttpBody::Base64 { data } = preview_glb.body else {
                panic!("Recipe preview GLB must be binary");
            };
            verify_forgecad_glb(&BASE64.decode(data).unwrap(), Some("interactive_preview"))
                .unwrap();
            let preview_graph = &preview_bundle.sealed_preview.assembly_graph;
            let candidate_graph = &candidate["expanded_assembly_graph"];
            if slug == "prop" {
                assert_eq!(
                    preview_graph["parts"].as_array().map(Vec::len),
                    base.assembly_graph["parts"].as_array().map(Vec::len),
                    "non-root replacement must retain its immutable ancestor parts"
                );
                assert!(preview_graph["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|part| part["part_id"] == target_part_id));
                assert!(preview_graph["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|part| part["part_id"] == base.assembly_graph["root_part_id"]));
                assert_eq!(
                    preview_graph["component_recipe_instances"].as_array().map(Vec::len),
                    base.assembly_graph["component_recipe_instances"].as_array().map(Vec::len),
                    "non-root replacement must swap one provenance instance without losing its ancestor"
                );
                assert!(preview_graph["component_recipe_instances"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|instance| instance["recipe"] == candidate["recipe"]));
            } else {
                assert_eq!(
                    preview_graph["connections"].as_array().map(Vec::len),
                    candidate_graph["connections"].as_array().map(Vec::len),
                    "Recipe replacement must retain every candidate connector edge; optional slots may legitimately yield zero edges"
                );
                assert_eq!(
                    preview_graph["parts"].as_array().map(Vec::len),
                    candidate_graph["parts"].as_array().map(Vec::len),
                    "Recipe replacement must retain the exact candidate part cardinality"
                );
                assert_eq!(
                    preview_graph["component_recipe_instances"],
                    candidate["component_recipe_instances"],
                    "Recipe provenance must be carried exactly from the sealed active candidate"
                );
                if slug == "arm" {
                    // C106 replaced the former two-part C105 arm fixture with
                    // one reviewed, ten-component robotic-arm production
                    // subtree. The generic lifecycle must consume that exact
                    // current catalog fact instead of preserving the retired
                    // low-detail assertion.
                    assert_eq!(candidate_graph["parts"].as_array().map(Vec::len), Some(10));
                    assert_eq!(
                        candidate_graph["connections"].as_array().map(Vec::len),
                        Some(9)
                    );
                    assert_eq!(
                        candidate["component_recipe_instances"]
                            .as_array()
                            .map(Vec::len),
                        Some(10)
                    );
                    assert_eq!(preview_graph["parts"].as_array().map(Vec::len), Some(10));
                    assert_eq!(
                        preview_graph["connections"].as_array().map(Vec::len),
                        Some(9)
                    );
                    assert_eq!(
                        preview_graph["component_recipe_instances"]
                            .as_array()
                            .map(Vec::len),
                        Some(10)
                    );
                }
            }
            assert!(preview_graph["parts"]
                .as_array()
                .unwrap()
                .iter()
                .all(|part| part["pivot"]["up"].is_array()));

            let confirmed = empty_post(
                &format!("/api/v1/agent/change-sets/{replace_id}:confirm"),
                &format!("confirm_c105_{slug}_recipe_replace"),
            );
            assert_eq!(confirmed.status, 200, "{}", json_body(&confirmed));
            let confirmed_json = json_body(&confirmed);
            let active_asset_version_id = confirmed_json["asset_version"]["asset_version_id"]
                .as_str()
                .unwrap()
                .to_string();
            let confirmed_version = rust_core
                .repository()
                .version(&active_asset_version_id)
                .unwrap()
                .unwrap();
            let snapshot = rust_core
                .repository()
                .snapshot(&project_id)
                .unwrap()
                .unwrap();
            let quality_id = snapshot.quality.as_ref().unwrap().quality_report_id.clone();
            let quality = rust_core
                .repository()
                .quality_report(&quality_id)
                .unwrap()
                .unwrap();
            assert_eq!(
                quality.report["evidence_source"],
                "geometry_compile_readback"
            );
            assert_eq!(
                quality.report["compile_readback"]["shape_program_sha256"],
                candidate_shape_sha
            );
            let exported = post_json(
                &format!("/api/v1/agent/asset-versions/{active_asset_version_id}:export"),
                &format!("export_c105_{slug}_recipe_replace"),
                json!({"client_request_id":format!("export_c105_{slug}_recipe_replace")}),
            );
            assert_eq!(exported.status, 200, "{}", json_body(&exported));
            let exported_json = json_body(&exported);
            assert_eq!(exported_json["shape_program_sha256"], candidate_shape_sha);
            assert_eq!(exported_json["readback_status"], "passed");
            let export_bytes = BASE64
                .decode(exported_json["glb_base64"].as_str().unwrap())
                .unwrap();
            verify_forgecad_glb(&export_bytes, Some("production_concept")).unwrap();
            assert_eq!(
                rust_core
                    .repository()
                    .read_object(&base_production.sha256)
                    .unwrap(),
                base_production_bytes,
                "new Recipe versions must not rewrite an old production GLB"
            );

            assert_eq!(
                semantic_sha256(&confirmed_version.shape_program).unwrap(),
                candidate_shape_sha,
                "the confirmed version must preserve the exact sealed active Recipe preview program"
            );
            real_geometry_evidence.push(json!({
                "schema_version": "C105RecipeLifecycleDomainEvidence@1",
                "domain_slug": slug,
                "domain_pack_id": domain_pack_id,
                "project_id": project_id,
                "asset_version_id": active_asset_version_id,
                "target_part_id": target_part_id,
                "recipe_candidate_id": candidate["candidate_id"].clone(),
                "recipe_candidate_sha256": candidate["candidate_sha256"].clone(),
                "recipe": candidate["recipe"].clone(),
                "expected_shape_program_sha256": candidate_shape_sha,
                "preview_shape_program_sha256": candidate_shape_sha,
                "shape_program": confirmed_version.shape_program.clone(),
                "assembly_graph": confirmed_version.assembly_graph.clone(),
                "material_bindings": confirmed_version.material_bindings.clone(),
            }));

            // A second sealed replacement of the same target is the concrete
            // stale-provenance regression boundary: root/child instance paths
            // must replace, never accumulate, across immutable descendants.
            let second_candidate = post_json(
                &format!("/api/v1/agent/asset-versions/{active_asset_version_id}/parts/{target_part_id}/component-recipes:expand"),
                &format!("expand_c105_{slug}_second"),
                json!({"schema_version":"ComponentRecipeActiveCandidateRequest@1","recipe_request_id":format!("recipereq_c105_lifecycle_{slug}_second"),"component_recipe_ref":recipe_ref,"slot_bindings":recipe_slot_bindings,"parameter_values":[],"material_zone_overrides":[]}),
            );
            assert_eq!(
                second_candidate.status,
                200,
                "{}",
                json_body(&second_candidate)
            );
            let second_candidate = json_body(&second_candidate);
            let second_replace = post_json(
                &format!("/api/v1/agent/asset-versions/{active_asset_version_id}/change-sets"),
                &format!("propose_c105_{slug}_recipe_replace_second"),
                json!({"client_request_id":format!("propose_c105_{slug}_recipe_replace_second"),"summary":"Replay a new sealed Recipe candidate","operations":[{"operation_id":format!("changeop_c105_{slug}_recipe_replace_second"),"op":"replace_part","part_id":target_part_id,"recipe_request_id":second_candidate["request_id"],"component_recipe_ref":second_candidate["recipe"],"recipe_registry_sha256":second_candidate["registry_sha256"],"recipe_slot_bindings":recipe_slot_bindings,"recipe_candidate_id":second_candidate["candidate_id"],"recipe_candidate_sha256":second_candidate["candidate_sha256"],"recipe_snapshot_revision":second_candidate["snapshot_revision"]}]}),
            );
            assert_eq!(second_replace.status, 201, "{}", json_body(&second_replace));
            let second_replace_id = json_body(&second_replace)["change_set_id"]
                .as_str()
                .unwrap()
                .to_string();
            assert_eq!(
                empty_post(
                    &format!("/api/v1/agent/change-sets/{second_replace_id}:preview"),
                    &format!("preview_c105_{slug}_recipe_replace_second")
                )
                .status,
                200
            );
            let second_confirmed = empty_post(
                &format!("/api/v1/agent/change-sets/{second_replace_id}:confirm"),
                &format!("confirm_c105_{slug}_recipe_replace_second"),
            );
            assert_eq!(
                second_confirmed.status,
                200,
                "{}",
                json_body(&second_confirmed)
            );
            let final_asset_version_id = json_body(&second_confirmed)["asset_version"]
                ["asset_version_id"]
                .as_str()
                .unwrap()
                .to_string();
            let final_version = rust_core
                .repository()
                .version(&final_asset_version_id)
                .unwrap()
                .unwrap();
            let instances = final_version.assembly_graph["component_recipe_instances"]
                .as_array()
                .unwrap();
            let paths = instances
                .iter()
                .map(|instance| instance["instance_path"].as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                paths.len(),
                instances.len(),
                "second Recipe replace must not retain stale/duplicate provenance paths"
            );
            assert_eq!(
                instances
                    .iter()
                    .filter(|instance| instance["instance_path"] == "root")
                    .count(),
                1,
                "second Recipe replace must retain exactly one root provenance instance"
            );
            assert_eq!(
                final_version.assembly_graph["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|part| part["part_id"] == target_part_id)
                    .count(),
                1,
                "stable target part identity must remain unique after Recipe replace"
            );

            let current_snapshot = rust_core
                .repository()
                .snapshot(&project_id)
                .unwrap()
                .unwrap();
            let undo = execute(
                AllowedHttpMethod::Post,
                &format!("/api/v1/projects/{project_id}/active-design:undo"),
                Some(&format!("undo_c105_{slug}")),
                Some(current_snapshot.etag().to_string()),
                ProtocolHttpBody::Utf8 { data: json!({"client_request_id":format!("undo_c105_{slug}"),"snapshot_revision":current_snapshot.revision}).to_string() },
            );
            assert_eq!(undo.status, 200, "{}", json_body(&undo));
            let undo_snapshot = rust_core
                .repository()
                .snapshot(&project_id)
                .unwrap()
                .unwrap();
            let redo = execute(
                AllowedHttpMethod::Post,
                &format!("/api/v1/projects/{project_id}/active-design:redo"),
                Some(&format!("redo_c105_{slug}")),
                Some(undo_snapshot.etag().to_string()),
                ProtocolHttpBody::Utf8 { data: json!({"client_request_id":format!("redo_c105_{slug}"),"snapshot_revision":undo_snapshot.revision}).to_string() },
            );
            assert_eq!(redo.status, 200, "{}", json_body(&redo));
            let redo_snapshot = rust_core
                .repository()
                .snapshot(&project_id)
                .unwrap()
                .unwrap();
            let redo_asset_version_id = redo_snapshot
                .active_design
                .asset_version_id()
                .unwrap()
                .to_string();
            assert_ne!(redo_asset_version_id, final_asset_version_id, "navigation creates one immutable descendant rather than mutating the confirmed Recipe version");
            let redo_version = rust_core
                .repository()
                .version(&redo_asset_version_id)
                .unwrap()
                .unwrap();
            assert_eq!(redo_version.shape_program, final_version.shape_program);
            assert_eq!(redo_version.assembly_graph, final_version.assembly_graph);
            let mut restart_asset_version_id = redo_asset_version_id.clone();
            if slug == "arm" {
                // C110C continuation proof: keep the generated arm as the
                // immutable base, add one reviewed sensor Recipe at a real
                // Connector, compile the resulting ShapeProgram, and confirm
                // it as the next version. No client-side geometry is trusted.
                let parent = redo_version.assembly_graph["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|part| {
                        part["role"] == "joint_housing"
                            && part["connectors"]
                                .as_array()
                                .is_some_and(|connectors| !connectors.is_empty())
                    })
                    .expect("C110C arm base exposes a reviewed Connector");
                let parent_part_id = parent["part_id"].as_str().unwrap();
                let parent_connector_id = parent["connectors"][0]["connector_id"].as_str().unwrap();
                let root = redo_version.assembly_graph["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|part| part["parent_part_id"].is_null())
                    .expect("C110C arm base exposes a root Part");
                let root_part_id = root["part_id"].as_str().unwrap();
                let root_connector_id = root["connectors"][0]["connector_id"].as_str().unwrap();
                let delta = post_json(
                    &format!("/api/v1/agent/asset-versions/{redo_asset_version_id}/change-sets"),
                    "propose_c110c_sensor_pod",
                    json!({
                        "client_request_id":"propose_c110c_sensor_pod",
                        "summary":"Add a visual sensor pod to the upper link",
                        "operations":[{
                            "operation_id":"delta_add_sensor_pod",
                            "op":"add_reviewed_recipe",
                            "part_id":parent_part_id,
                            "new_part_id":"part_c110c_sensor_pod",
                            "parent_connector_id":parent_connector_id,
                            "child_connector_id":"connector_sensor_pod_mount",
                            "recipe_id":"recipe_c110c_arm_sensor_pod",
                            "slot_id":"slot_arm_sensor_pod",
                            "transform":{"position":[0.0,12.0,0.0],"rotation":[0.0,0.2,0.0],"scale":[1.0,1.0,1.0]}
                        },{
                            "operation_id":"delta_pose_joint",
                            "op":"set_joint_pose",
                            "part_id":parent_part_id,
                            "joint_id":"joint_c110c_visual_wrist",
                            "pose":{"rotation":[0.0,0.12,0.0],"translation":[4.0,0.0,0.0]}
                        },{
                            "operation_id":"delta_snap_joint",
                            "op":"snap_part_to_connector",
                            "part_id":parent_part_id,
                            "target_part_id":root_part_id,
                            "target_connector_id":root_connector_id,
                            "connector_id":parent_connector_id
                        }]
                    }),
                );
                assert_eq!(delta.status, 201, "{}", json_body(&delta));
                let delta_json = json_body(&delta);
                let delta_id = delta_json["change_set_id"].as_str().unwrap();
                let delta_preview = empty_post(
                    &format!("/api/v1/agent/change-sets/{delta_id}:preview"),
                    "preview_c110c_sensor_pod",
                );
                assert_eq!(delta_preview.status, 200, "{}", json_body(&delta_preview));
                let delta_bundle = rust_core
                    .repository()
                    .read_change_set_preview_bundle(delta_id)
                    .unwrap()
                    .expect("C110C preview must persist a sealed geometry bundle");
                assert_eq!(
                    delta_bundle.sealed_preview.parts.len(),
                    redo_version.parts.len() + 1
                );
                assert!(delta_bundle.sealed_preview.shape_program["operations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|operation| operation["operation_id"]
                        .as_str()
                        .is_some_and(|id| id.contains("sensor_pod"))));
                let delta_glb = execute(
                    AllowedHttpMethod::Get,
                    &format!("/api/v1/agent/change-sets/{delta_id}:preview.glb"),
                    None,
                    None,
                    ProtocolHttpBody::Empty,
                );
                assert_eq!(delta_glb.status, 200);
                let ProtocolHttpBody::Base64 { data } = delta_glb.body else {
                    panic!("C110C preview GLB must be binary");
                };
                verify_forgecad_glb(&BASE64.decode(data).unwrap(), Some("interactive_preview"))
                    .unwrap();
                let delta_confirmed = empty_post(
                    &format!("/api/v1/agent/change-sets/{delta_id}:confirm"),
                    "confirm_c110c_sensor_pod",
                );
                assert_eq!(
                    delta_confirmed.status,
                    200,
                    "{}",
                    json_body(&delta_confirmed)
                );
                restart_asset_version_id = json_body(&delta_confirmed)["asset_version"]
                    ["asset_version_id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let delta_version = rust_core
                    .repository()
                    .version(&restart_asset_version_id)
                    .unwrap()
                    .unwrap();
                assert_eq!(delta_version.parts.len(), redo_version.parts.len() + 1);
                // C110D continuation: the same confirmed V3 arm accepts two
                // distinct reviewed visual Recipes in one atomic ChangeSet.
                let c110d_parent = delta_version.assembly_graph["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|part| part["part_id"] == parent_part_id)
                    .expect("C110D keeps the existing arm parent Part");
                let c110d_parent_connector_id = c110d_parent["connectors"][0]["connector_id"]
                    .as_str()
                    .unwrap();
                let c110d = post_json(
                    &format!("/api/v1/agent/asset-versions/{restart_asset_version_id}/change-sets"),
                    "propose_c110d_arm_attachments",
                    json!({
                        "client_request_id":"propose_c110d_arm_attachments",
                        "summary":"Add an actuator cover and cable guide to the confirmed arm",
                        "operations":[{
                            "operation_id":"delta_c110d_actuator_cover",
                            "op":"add_reviewed_recipe",
                            "part_id":parent_part_id,
                            "new_part_id":"part_c110d_actuator_cover",
                            "parent_connector_id":c110d_parent_connector_id,
                            "child_connector_id":"connector_actuator_cover_mount",
                            "recipe_id":"recipe_c110d_arm_actuator_cover",
                            "slot_id":"slot_arm_guard_rail",
                            "transform":{"position":[0.0,24.0,0.0],"rotation":[0.0,0.18,0.0],"scale":[1.0,1.0,1.0]}
                        },{
                            "operation_id":"delta_c110d_cable_guide",
                            "op":"add_reviewed_recipe",
                            "part_id":parent_part_id,
                            "new_part_id":"part_c110d_cable_guide",
                            "parent_connector_id":c110d_parent_connector_id,
                            "child_connector_id":"connector_cable_guide_mount",
                            "recipe_id":"recipe_c110d_arm_cable_guide",
                            "slot_id":"slot_arm_camera_boom",
                            "transform":{"position":[0.0,-30.0,18.0],"rotation":[0.0,-0.12,0.0],"scale":[1.0,1.0,1.0]}
                        }]
                    }),
                );
                assert_eq!(c110d.status, 201, "{}", json_body(&c110d));
                let c110d_json = json_body(&c110d);
                let c110d_id = c110d_json["change_set_id"].as_str().unwrap();
                let c110d_preview = empty_post(
                    &format!("/api/v1/agent/change-sets/{c110d_id}:preview"),
                    "preview_c110d_arm_attachments",
                );
                assert_eq!(c110d_preview.status, 200, "{}", json_body(&c110d_preview));
                let c110d_bundle = rust_core
                    .repository()
                    .read_change_set_preview_bundle(c110d_id)
                    .unwrap()
                    .expect("C110D preview must persist a sealed geometry bundle");
                assert_eq!(
                    c110d_bundle.sealed_preview.parts.len(),
                    delta_version.parts.len() + 2
                );
                assert!(c110d_bundle.sealed_preview.shape_program["operations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|operation| operation["operation_id"]
                        .as_str()
                        .is_some_and(|id| id.contains("actuator_cover"))));
                assert!(c110d_bundle.sealed_preview.shape_program["operations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|operation| operation["operation_id"]
                        .as_str()
                        .is_some_and(|id| id.contains("cable_guide"))));
                let c110d_glb = execute(
                    AllowedHttpMethod::Get,
                    &format!("/api/v1/agent/change-sets/{c110d_id}:preview.glb"),
                    None,
                    None,
                    ProtocolHttpBody::Empty,
                );
                assert_eq!(c110d_glb.status, 200);
                let ProtocolHttpBody::Base64 { data } = c110d_glb.body else {
                    panic!("C110D preview GLB must be binary");
                };
                verify_forgecad_glb(&BASE64.decode(data).unwrap(), Some("interactive_preview"))
                    .unwrap();
                let c110d_confirmed = empty_post(
                    &format!("/api/v1/agent/change-sets/{c110d_id}:confirm"),
                    "confirm_c110d_arm_attachments",
                );
                assert_eq!(
                    c110d_confirmed.status,
                    200,
                    "{}",
                    json_body(&c110d_confirmed)
                );
                let c110d_asset_version_id = json_body(&c110d_confirmed)["asset_version"]
                    ["asset_version_id"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let c110d_version = rust_core
                    .repository()
                    .version(&c110d_asset_version_id)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    c110d_version.parent_asset_version_id.as_deref(),
                    Some(restart_asset_version_id.as_str())
                );
                assert_eq!(c110d_version.version_no, delta_version.version_no + 1);
                assert_eq!(c110d_version.parts.len(), delta_version.parts.len() + 2);
                restart_asset_version_id = c110d_asset_version_id;
            }
            let redo_production = rust_core
                .repository()
                .object_for_reference(&ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: restart_asset_version_id.clone(),
                    role: "production_glb".into(),
                })
                .unwrap()
                .unwrap();
            restart_expectations.push((
                project_id,
                restart_asset_version_id,
                redo_production.sha256,
            ));
            let _ = confirmed_version;
        }

        drop(bridge);
        drop(rust_core);
        if let Some(evidence_dir) = std::env::var_os("FORGECAD_C105_RECIPE_LIFECYCLE_EVIDENCE_DIR")
        {
            let evidence_dir = PathBuf::from(evidence_dir);
            fs::create_dir_all(&evidence_dir)
                .expect("create C105 real-geometry evidence directory");
            let evidence = json!({
                "schema_version": "C105RecipeLifecycleEvidence@1",
                "provider_calls": 0,
                "domains": real_geometry_evidence,
            });
            fs::write(
                evidence_dir.join("c105-recipe-lifecycle.json"),
                serde_json::to_vec_pretty(&evidence)
                    .expect("serialize C105 real-geometry evidence"),
            )
            .expect("write C105 real-geometry evidence");
        }
        let reopened = RustCoreRuntime::open(
            &library_root,
            format!("c105-recipe-lifecycle-restart-{serial}"),
        )
        .expect("restart C105 lifecycle Rust core");
        for (project_id, asset_version_id, production_sha256) in restart_expectations {
            let snapshot = reopened
                .repository()
                .snapshot(&project_id)
                .unwrap()
                .unwrap();
            assert_eq!(
                snapshot.active_design.asset_version_id(),
                Some(asset_version_id.as_str())
            );
            let production = reopened
                .repository()
                .object_for_reference(&ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: asset_version_id.clone(),
                    role: "production_glb".into(),
                })
                .unwrap()
                .unwrap();
            assert_eq!(production.sha256, production_sha256);
            verify_forgecad_glb(
                &reopened
                    .repository()
                    .read_object(&production.sha256)
                    .unwrap(),
                Some("production_concept"),
            )
            .unwrap();
        }
        drop(reopened);
        drop(backend);
        fs::remove_dir_all(library_root).unwrap();
    }

    #[test]
    fn rust_blockout_compat_cancellation_discards_late_preview_without_product_writes() {
        let backend = FakeGeometryBackend::start(FakeGeometryScenario::BlockUntilCancelled);
        let serial = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let library_root = std::env::temp_dir().join(format!(
            "forgecad-k003-blockout-cancel-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&library_root).unwrap();
        let rust_core = Arc::new(
            RustCoreRuntime::open(&library_root, format!("blockout-cancel-test-{serial}"))
                .expect("open unpublished Rust core"),
        );
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .expect("construct production bridge");
        let direction = |direction_id: &str| {
            json!({
                "direction_id": direction_id,
                "title": "Direction",
                "summary": "Complete non-functional exterior concept.",
                "silhouette": "compact",
                "primary_part_roles": ["primary_form", "secondary_form"],
                "material_direction": "dark visual coating"
            })
        };
        let request = PreparedCompatHttpRequest {
            endpoint: LocalAgentEndpoint::parse(&backend.endpoint).unwrap(),
            method: AllowedHttpMethod::Post,
            path: "/api/v1/agent/blockouts".into(),
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                (
                    "Idempotency-Key".into(),
                    "build_k003_blockout_cancel".into(),
                ),
            ],
            body: ProtocolHttpBody::Utf8 {
                data: json!({
                    "client_request_id": "build_k003_blockout_cancel",
                    "plan": {
                        "schema_version": "MechanicalConceptPlan@1",
                        "plan_id": "plan_k003_blockout_cancel",
                        "domain_pack_id": "pack_future_weapon_prop",
                        "brief": "non-functional future game prop exterior",
                        "generation_stage": "blockout",
                        "spec": {},
                        "directions": [
                            direction("direction_primary")
                        ],
                        "provider_id": "rust_app_server",
                        "shape_program_ready": false
                    },
                    "direction_id": "direction_primary",
                    "presentation_profile": "quick_sketch"
                })
                .to_string(),
            },
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let response = runtime.block_on(async {
            let cancellation = CancellationToken::new();
            let task_cancellation = cancellation.clone();
            let port = bridge.inner.port.clone();
            let work = tokio::spawn(async move {
                CompatibilityHttpPort::execute(port.as_ref(), request, task_cancellation).await
            });
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            loop {
                if backend
                    .state
                    .lock()
                    .unwrap()
                    .requested_paths
                    .iter()
                    .any(|path| path == "POST /api/v1/internal/geometry/execute")
                {
                    break;
                }
                assert!(tokio::time::Instant::now() < deadline);
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            cancellation.cancel();
            tokio::time::timeout(Duration::from_secs(3), work)
                .await
                .unwrap()
                .unwrap()
                .unwrap()
        });
        assert_eq!(response.status, 409);
        let ProtocolHttpBody::Utf8 { data } = response.body else {
            panic!("cancel response must be JSON text");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&data).unwrap()["error"]["code"],
            "REQUEST_CANCELLED"
        );
        assert!(bridge
            .inner
            .port
            .inner
            .native_blockouts
            .lock()
            .unwrap()
            .candidates
            .is_empty());
        assert!(rust_core
            .repository()
            .list_projects(false, 200)
            .unwrap()
            .is_empty());
        runtime.block_on(async {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            while !backend.state.lock().unwrap().cancel_seen {
                assert!(tokio::time::Instant::now() < deadline);
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        });
        let state = backend.state.lock().unwrap();
        assert!(state.cancel_seen);
        assert!(state
            .requested_paths
            .iter()
            .any(|path| path == "POST /api/v1/internal/geometry/cancel"));
        drop(state);

        drop(bridge);
        drop(rust_core);
        drop(backend);
        fs::remove_dir_all(library_root).unwrap();
    }

    const R007B_WORKBENCH_DRIVER_COMMAND_ENV: &str = "FORGECAD_R007B_WORKBENCH_DRIVER_COMMAND";
    const R007B_WORKBENCH_DRIVER_OUTPUT_ENV: &str = "FORGECAD_R007B_WORKBENCH_DRIVER_OUTPUT";
    const R007B_WORKBENCH_DRIVER_LIBRARY_ENV: &str = "FORGECAD_R007B_WORKBENCH_DRIVER_LIBRARY_ROOT";
    const R007B_WORKBENCH_DRIVER_SCHEMA: &str = "ForgeCADR007BWorkbenchRustDriverCommand@1";

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct R007BWorkbenchDriverCommand {
        schema_version: String,
        operation: String,
        project_id: String,
        #[serde(default)]
        request: Option<R007BWorkbenchDriverRequest>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct R007BWorkbenchDriverRequest {
        method: String,
        path: String,
        #[serde(default)]
        headers: Vec<(String, String)>,
        #[serde(default = "r007b_driver_empty_body")]
        body: ProtocolHttpBody,
    }

    fn r007b_driver_empty_body() -> ProtocolHttpBody {
        ProtocolHttpBody::Empty
    }

    fn r007b_driver_env_path(name: &str) -> Option<PathBuf> {
        env::var_os(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    }

    fn r007b_driver_method(method: &str) -> Option<AllowedHttpMethod> {
        match method {
            "GET" => Some(AllowedHttpMethod::Get),
            "POST" => Some(AllowedHttpMethod::Post),
            "PUT" => Some(AllowedHttpMethod::Put),
            "PATCH" => Some(AllowedHttpMethod::Patch),
            _ => None,
        }
    }

    fn r007b_driver_write_output(output: &Path, value: &Value) {
        let parent = output.parent().expect("driver output has parent");
        fs::create_dir_all(parent).expect("create driver output parent");
        let temporary = output.with_extension("tmp");
        fs::write(
            &temporary,
            serde_json::to_vec(value).expect("serialize driver output"),
        )
        .expect("write driver output");
        fs::rename(temporary, output).expect("publish driver output atomically");
    }

    fn is_r007b_driver_c106_v1(version: &AgentAssetVersion) -> bool {
        version.version_no == 1
            && version.domain_pack_id == "pack_robotic_arm_concept"
            && version.assembly_graph["component_recipe_instances"]
                .as_array()
                .is_some_and(|instances| {
                    instances.len() == 10
                        && instances.iter().any(|instance| {
                            instance["parent_instance_id"].is_null()
                                && instance["recipe"]["recipe_id"].as_str().is_some_and(
                                    |recipe_id| recipe_id.starts_with("recipe_c106_arm_"),
                                )
                        })
                })
    }

    /// Creates one actual C106 V1 through the production Rust Product Tool
    /// executor and normal single-result confirmation path. The FakeGeometry
    /// process supplies only the capability-gated restricted executor; Project,
    /// Snapshot, Version, Quality and CAS remain in RustCoreRuntime.
    fn bootstrap_r007b_driver_c106_v1(
        bridge: &AppServerBridge,
        rust_core: &Arc<RustCoreRuntime>,
        endpoint: &LocalAgentEndpoint,
        project_id: &str,
    ) -> Result<String, String> {
        if let Some(head) = rust_core
            .repository()
            .head(project_id)
            .map_err(|error| error.to_string())?
        {
            let version = rust_core
                .repository()
                .version(&head)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "R007B_DRIVER_EXISTING_HEAD_MISSING".to_string())?;
            if !is_r007b_driver_c106_v1(&version) {
                return Err("R007B_DRIVER_EXISTING_HEAD_NOT_C106_ARM".into());
            }
            return Ok(head);
        }

        if rust_core
            .repository()
            .project(project_id)
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("R007B_DRIVER_PROJECT_EXISTS_WITHOUT_C106_V1".into());
        }

        rust_core
            .repository()
            .create_project(&forgecad_core::Project {
                project_id: project_id.into(),
                profile_id: "profile_weapon_concept_v1".into(),
                // Project's legacy-neutral profile taxonomy remains
                // `weapon_concept`; the actual C106 asset domain is pinned by
                // its reviewed `pack_robotic_arm_concept` version below.
                domain_type: "weapon_concept".into(),
                name: "R007B workbench Rust driver arm".into(),
                status: forgecad_core::ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-19T00:00:00Z".into(),
                updated_at: "2026-07-19T00:00:00Z".into(),
            })
            .map_err(|error| error.to_string())?;

        let executor = bridge
            .inner
            .native_product_tools
            .as_ref()
            .ok_or_else(|| "R007B_DRIVER_NATIVE_TOOLS_UNAVAILABLE".to_string())?;
        let execution_id = "execution_r007b_driver_bootstrap";
        let turn_id = "turn_r007b_driver_bootstrap";
        executor
            .bind_execution_project(execution_id, turn_id, Some(project_id))
            .map_err(|error| error.message)?;
        let plan = json!({
            "schema_version": "MechanicalConceptPlan@1",
            "plan_id": "plan_r007b_driver_bootstrap",
            "domain_pack_id": "pack_robotic_arm_concept",
            "brief": "non-functional articulated robotic-arm exterior concept",
            "generation_stage": "blockout",
            "spec": {},
            "directions": [{
                "direction_id": "direction_primary",
                "title": "Robotic arm exterior",
                "summary": "Complete visual-only articulated robotic arm exterior.",
                "silhouette": "industrial",
                "primary_part_roles": ["link_armor", "surface_trim"],
                "material_direction": "anodized exterior panels, dark joints, and blue signal trim"
            }],
            "provider_id": "rust_app_server",
            "shape_program_ready": false
        });
        let calls = [
            ("plan_complete_concept", json!({"plan": plan})),
            (
                "select_style_recipe",
                json!({"domain_pack_id": "pack_robotic_arm_concept", "intent": "流线"}),
            ),
            (
                "build_candidate_geometry",
                json!({"direction_id": "direction_primary", "variant_id": null, "presentation_profile": "showcase"}),
            ),
            ("compile_readback_candidate", json!({})),
            ("render_candidate_views", json!({})),
            ("evaluate_candidate", json!({})),
            ("prepare_candidate_preview", json!({})),
        ];
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "R007B_DRIVER_RUNTIME_UNAVAILABLE".to_string())?;
        let registry = ProductToolRegistry::default();
        let mut final_output = None;
        for (index, (name, arguments)) in calls.into_iter().enumerate() {
            let request = registry
                .build_execution_request(
                    turn_id,
                    &ProviderToolCall {
                        call_id: format!("call_r007b_driver_bootstrap_{index}"),
                        name: name.into(),
                        arguments,
                    },
                    execution_id,
                    "cancel_r007b_driver_bootstrap",
                    "token_r007b_driver_bootstrap",
                )
                .map_err(|error| error.message)?;
            let result = runtime
                .block_on(executor.execute(request, CancellationToken::new()))
                .map_err(|error| error.message)?;
            if result.status != ProductToolExecutionStatus::Completed {
                return Err(result
                    .error_code
                    .unwrap_or_else(|| "R007B_DRIVER_C106_TOOL_FAILED".into()));
            }
            final_output = result.validated_output.map(|output| output.value);
        }
        let output = final_output.ok_or_else(|| "R007B_DRIVER_PREVIEW_MISSING".to_string())?;
        let decision = &output["single_result_decision"];
        if decision["state"].as_str() != Some("ready_for_preview") {
            return Err("R007B_DRIVER_PREVIEW_NOT_READY".into());
        }
        let preview_id = decision["preview"]["preview_id"]
            .as_str()
            .ok_or_else(|| "R007B_DRIVER_PREVIEW_ID_MISSING".to_string())?;
        let sha256 = decision["preview"]["artifact_sha256"]
            .as_str()
            .ok_or_else(|| "R007B_DRIVER_PREVIEW_SHA_MISSING".to_string())?;
        let response = runtime
            .block_on(CompatibilityHttpPort::execute(
                bridge.inner.port.as_ref(),
                PreparedCompatHttpRequest {
                    endpoint: endpoint.clone(),
                    method: AllowedHttpMethod::Post,
                    path: format!(
                        "/api/v1/agent/projects/{project_id}/turns/{turn_id}/single-results/{preview_id}:confirm"
                    ),
                    headers: vec![
                        ("Content-Type".into(), "application/json".into()),
                        ("Idempotency-Key".into(), "confirm_r007b_driver_bootstrap".into()),
                        ("If-Match".into(), format!("\"sha256:{sha256}\"")),
                    ],
                    body: ProtocolHttpBody::Utf8 {
                        data: json!({
                            "client_request_id": "confirm_r007b_driver_bootstrap",
                            "expected_artifact_sha256": sha256,
                            "summary": "Confirm R007B driver C106 mechanical-arm V1"
                        })
                        .to_string(),
                    },
                },
                CancellationToken::new(),
            ))
            .map_err(|error| error.message)?;
        if response.status != 201 {
            return Err("R007B_DRIVER_C106_CONFIRM_FAILED".into());
        }
        let head = rust_core
            .repository()
            .head(project_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "R007B_DRIVER_C106_HEAD_MISSING".to_string())?;
        let version = rust_core
            .repository()
            .version(&head)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "R007B_DRIVER_C106_VERSION_MISSING".to_string())?;
        if !is_r007b_driver_c106_v1(&version) {
            return Err("R007B_DRIVER_C106_V1_INVALID".into());
        }
        Ok(head)
    }

    /// An opt-in real Rust driver for the browser R007B test. It is ignored so
    /// ordinary cargo tests never read caller paths or retain test libraries.
    /// Every invocation reconstructs the bridge/backend, while the caller's
    /// persistent library_root remains the sole owner of SQLite/CAS state.
    #[test]
    #[ignore = "requires explicit R007B driver env command, output and persistent library root"]
    fn r007b_workbench_rust_driver() {
        let (Some(command_path), Some(output_path), Some(library_root)) = (
            r007b_driver_env_path(R007B_WORKBENCH_DRIVER_COMMAND_ENV),
            r007b_driver_env_path(R007B_WORKBENCH_DRIVER_OUTPUT_ENV),
            r007b_driver_env_path(R007B_WORKBENCH_DRIVER_LIBRARY_ENV),
        ) else {
            return;
        };
        let command: R007BWorkbenchDriverCommand =
            serde_json::from_slice(&fs::read(&command_path).expect("read R007B driver command"))
                .expect("parse R007B driver command");
        assert_eq!(command.schema_version, R007B_WORKBENCH_DRIVER_SCHEMA);
        assert!(valid_stable_id(&command.project_id));
        fs::create_dir_all(&library_root).expect("create persistent R007B library root");

        let backend = FakeGeometryBackend::start(FakeGeometryScenario::Success);
        let rust_core = Arc::new(
            RustCoreRuntime::open(
                &library_root,
                format!("r007b-driver-{}", std::process::id()),
            )
            .expect("open persistent R007B Rust core"),
        );
        let provider: Arc<dyn ProviderClient> = Arc::new(FakeDeepSeekClient::scripted(
            "deepseek-chat",
            false,
            false,
            Vec::new(),
        ));
        let bridge = AppServerBridge::new_production(
            &backend.endpoint,
            TEST_GEOMETRY_CAPABILITY.to_string(),
            provider,
            Arc::clone(&rust_core),
        )
        .expect("construct R007B production bridge");
        let endpoint = LocalAgentEndpoint::parse(&backend.endpoint).expect("parse test endpoint");

        let result = match command.operation.as_str() {
            "bootstrap" => {
                let asset_version_id = bootstrap_r007b_driver_c106_v1(
                    &bridge,
                    &rust_core,
                    &endpoint,
                    &command.project_id,
                )
                .expect("bootstrap exact C106 V1");
                json!({
                    "schema_version": "ForgeCADR007BWorkbenchRustDriverResponse@1",
                    "operation": "bootstrap",
                    "project_id": command.project_id,
                    "asset_version_id": asset_version_id,
                })
            }
            "request" => {
                assert!(
                    rust_core
                        .repository()
                        .head(&command.project_id)
                        .unwrap()
                        .is_some(),
                    "R007B driver request requires bootstrap C106 V1"
                );
                let request = command
                    .request
                    .expect("R007B request operation requires request payload");
                let method = r007b_driver_method(&request.method)
                    .expect("R007B driver method is allow-listed");
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                let response = runtime
                    .block_on(CompatibilityHttpPort::execute(
                        bridge.inner.port.as_ref(),
                        PreparedCompatHttpRequest {
                            endpoint,
                            method,
                            path: request.path,
                            headers: request.headers,
                            body: request.body,
                        },
                        CancellationToken::new(),
                    ))
                    .expect("execute R007B request through Rust CompatibilityHttpPort");
                json!({
                    "schema_version": "ForgeCADR007BWorkbenchRustDriverResponse@1",
                    "operation": "request",
                    "project_id": command.project_id,
                    "response": serde_json::to_value(response).expect("serialize compat response"),
                })
            }
            _ => panic!("R007B driver operation must be bootstrap or request"),
        };
        r007b_driver_write_output(&output_path, &result);
    }

    #[test]
    fn endpoint_is_fixed_to_loopback_with_an_explicit_port() {
        assert!(AppServerBridge::new("http://127.0.0.1:8000").is_ok());
        assert!(AppServerBridge::new("https://example.com").is_err());
        assert!(AppServerBridge::new("http://example.com:8000").is_err());
    }

    #[test]
    fn recipe_revolve_size_uses_its_profile_closure_and_never_sizes_profile_helpers_as_meshes() {
        let profile = json!({
            "operation_id": "op_profile",
            "op": "profile",
            "inputs": [],
            "args": {"points": [[0.0, -90.0], [120.0, -88.0], [318.0, 0.0], [120.0, 88.0], [0.0, 90.0]]}
        });
        let revolve = json!({
            "operation_id": "op_revolve",
            "op": "revolve",
            "inputs": ["op_profile"],
            "args": {"position": [240.0, 280.0, 0.0], "angle": 6.283185307179586, "radial_segments": 48, "part_role": "secondary_form", "material_id": "mat_graphite", "zone_id": "zone_arm_turntable"}
        });
        let operations =
            std::collections::BTreeMap::from([("op_profile", &profile), ("op_revolve", &revolve)]);

        assert_eq!(
            native_recipe_operation_size(&revolve, &operations).unwrap(),
            [636.0, 180.0, 636.0]
        );
        let helper_error = native_recipe_operation_size(&profile, &operations).unwrap_err();
        assert_eq!(helper_error.code, "RECIPE_PREVIEW_GEOMETRY_INVALID");

        let malformed_revolve = json!({
            "operation_id": "op_revolve",
            "op": "revolve",
            "inputs": ["missing_profile"],
            "args": {"position": [0.0, 0.0, 0.0]}
        });
        let malformed = std::collections::BTreeMap::from([("op_revolve", &malformed_revolve)]);
        assert_eq!(
            native_recipe_operation_size(&malformed_revolve, &malformed)
                .unwrap_err()
                .code,
            "RECIPE_PREVIEW_GEOMETRY_INVALID"
        );
    }

    #[test]
    fn resource_protocol_reuses_the_restricted_compatibility_contract() {
        let bridge = AppServerBridge::new("http://127.0.0.1:8000").unwrap();
        let allowed = http::Request::builder()
            .method(http::Method::GET)
            .uri("forgecad-resource://localhost/api/v1/agent/threads?limit=1")
            .body(Vec::new())
            .unwrap();
        let prepared = bridge.prepare_resource_request(&allowed).unwrap();
        assert_eq!(prepared.path, "/api/v1/agent/threads?limit=1");
        assert_eq!(prepared.method.as_str(), "GET");

        for denied in [
            http::Request::builder()
                .method(http::Method::GET)
                .uri("forgecad-resource://external.test/api/v1/agent/threads")
                .body(Vec::new())
                .unwrap(),
            http::Request::builder()
                .method(http::Method::POST)
                .uri("forgecad-resource://localhost/api/v1/agent/threads")
                .body(Vec::new())
                .unwrap(),
            http::Request::builder()
                .method(http::Method::GET)
                .uri("forgecad-resource://localhost/api/v1/app-server/connections")
                .body(Vec::new())
                .unwrap(),
        ] {
            assert!(bridge.prepare_resource_request(&denied).is_err());
        }
    }

    #[test]
    fn production_csp_and_capability_remove_loopback_and_frontend_emit() {
        let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let csp = config["app"]["security"]["csp"].as_str().unwrap();
        assert!(!csp.contains("127.0.0.1"));
        assert!(csp.contains("forgecad-resource:"));
        assert!(csp.contains(
            "connect-src 'self' forgecad-resource: http://forgecad-resource.localhost blob:"
        ));

        let capability: Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let permissions = capability["permissions"].as_array().unwrap();
        let has_permission = |permission: &str| {
            permissions
                .iter()
                .any(|candidate| candidate.as_str() == Some(permission))
        };
        assert!(has_permission("core:event:allow-listen"));
        assert!(has_permission("core:event:allow-unlisten"));
        assert!(has_permission("core:event:deny-emit"));
        assert!(has_permission("core:event:deny-emit-to"));
        assert!(!has_permission("core:event:allow-emit"));
        assert!(!has_permission("core:event:allow-emit-to"));
        assert!(!has_permission("core:default"));
    }

    #[test]
    fn stable_stream_ids_are_accepted_by_the_subscription_contract() {
        assert!(valid_stable_id("stream_0123456789"));
    }
}
