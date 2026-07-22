use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc};

use forgecad_app_server_protocol::{
    valid_stable_id, CompatHttpRequest, CompatHttpResponse, ReplayParams, RpcError,
    SseSubscriptionParams, SseUnsubscribeParams, METHOD_COMPAT_HTTP, METHOD_COMPAT_SUBSCRIBE,
    METHOD_COMPAT_UNSUBSCRIBE, METHOD_EVENTS_REPLAY, SSE_SUBSCRIPTION_SCHEMA_VERSION,
    SSE_UNSUBSCRIBE_SCHEMA_VERSION,
};
use serde_json::Value;

use crate::{CancellationToken, HandlerFuture, RequestHandler};

/// Leaves enough room for the JSON-RPC envelope inside a 64 MiB frame.
pub const MAX_RAW_COMPAT_BODY_BYTES: usize = 47 * 1024 * 1024;
pub const MAX_ENCODED_COMPAT_BODY_BYTES: usize = MAX_RAW_COMPAT_BODY_BYTES.div_ceil(3) * 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalAgentEndpoint {
    origin: String,
}

impl LocalAgentEndpoint {
    pub fn parse(value: &str) -> Result<Self, RpcError> {
        let authority = value.strip_prefix("http://").ok_or_else(|| {
            RpcError::invalid_params("Compatibility endpoint must use loopback HTTP.")
        })?;
        if authority.is_empty()
            || authority.contains('/')
            || authority.contains('?')
            || authority.contains('#')
            || authority.contains('@')
        {
            return Err(RpcError::invalid_params(
                "Compatibility endpoint must contain only a loopback host and explicit port.",
            ));
        }
        let (host, port) = if let Some(rest) = authority.strip_prefix("[::1]:") {
            ("[::1]", rest)
        } else {
            authority.rsplit_once(':').ok_or_else(|| {
                RpcError::invalid_params("Compatibility endpoint requires an explicit port.")
            })?
        };
        if !matches!(host, "127.0.0.1" | "localhost" | "[::1]") {
            return Err(RpcError::invalid_params(
                "Compatibility endpoint host must be loopback.",
            ));
        }
        let port: u16 = port
            .parse()
            .map_err(|_| RpcError::invalid_params("Compatibility endpoint port is invalid."))?;
        if port == 0 {
            return Err(RpcError::invalid_params(
                "Compatibility endpoint port must be non-zero.",
            ));
        }
        Ok(Self {
            origin: format!("http://{host}:{port}"),
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedHttpMethod {
    Get,
    Post,
    Put,
    Patch,
}

impl AllowedHttpMethod {
    fn parse(value: &str) -> Result<Self, RpcError> {
        match value {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            _ => Err(RpcError::invalid_params(
                "compat/http method is not in the code-owned allow-list.",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCompatHttpRequest {
    pub endpoint: LocalAgentEndpoint,
    pub method: AllowedHttpMethod,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: forgecad_app_server_protocol::ProtocolHttpBody,
}

pub type CompatHttpFuture =
    Pin<Box<dyn Future<Output = Result<CompatHttpResponse, RpcError>> + Send + 'static>>;

pub trait CompatibilityHttpPort: Send + Sync + 'static {
    fn execute(
        &self,
        request: PreparedCompatHttpRequest,
        cancellation: CancellationToken,
    ) -> CompatHttpFuture;

    fn subscribe(
        &self,
        _params: SseSubscriptionParams,
        _cancellation: CancellationToken,
    ) -> HandlerFuture {
        Box::pin(async { Err(RpcError::method_not_found(METHOD_COMPAT_SUBSCRIBE)) })
    }

    fn unsubscribe(
        &self,
        _params: SseUnsubscribeParams,
        _cancellation: CancellationToken,
    ) -> HandlerFuture {
        Box::pin(async { Err(RpcError::method_not_found(METHOD_COMPAT_UNSUBSCRIBE)) })
    }

    fn replay(&self, _params: ReplayParams, _cancellation: CancellationToken) -> HandlerFuture {
        Box::pin(async { Err(RpcError::method_not_found(METHOD_EVENTS_REPLAY)) })
    }
}

pub struct CompatibilityAdapter<P> {
    endpoint: LocalAgentEndpoint,
    port: Arc<P>,
    max_encoded_body_bytes: usize,
}

impl<P> CompatibilityAdapter<P> {
    pub fn new(endpoint: LocalAgentEndpoint, port: Arc<P>) -> Self {
        Self {
            endpoint,
            port,
            max_encoded_body_bytes: MAX_ENCODED_COMPAT_BODY_BYTES,
        }
    }

    pub fn prepare(
        &self,
        request: CompatHttpRequest,
    ) -> Result<PreparedCompatHttpRequest, RpcError> {
        request.validate_schema()?;
        let method = AllowedHttpMethod::parse(&request.method)?;
        validate_path(&request.path, method)?;
        validate_headers(&request.headers)?;
        if !body_within_budget(
            &request.body,
            MAX_RAW_COMPAT_BODY_BYTES,
            self.max_encoded_body_bytes,
        ) {
            return Err(RpcError::new(
                forgecad_app_server_protocol::INPUT_TOO_LARGE,
                "COMPAT_BODY_TOO_LARGE",
                "compat/http body exceeds the bounded adapter limit.",
                false,
            ));
        }
        Ok(PreparedCompatHttpRequest {
            endpoint: self.endpoint.clone(),
            method,
            path: request.path,
            headers: request.headers,
            body: request.body,
        })
    }
}

impl<P: CompatibilityHttpPort> RequestHandler for CompatibilityAdapter<P> {
    fn handle(
        &self,
        method: String,
        params: Value,
        cancellation: CancellationToken,
    ) -> HandlerFuture {
        match method.as_str() {
            METHOD_COMPAT_HTTP => {
                let request: CompatHttpRequest = match serde_json::from_value(params) {
                    Ok(request) => request,
                    Err(error) => {
                        return Box::pin(async move {
                            Err(RpcError::invalid_params(format!(
                                "Invalid compat/http params: {error}"
                            )))
                        });
                    }
                };
                let prepared = match self.prepare(request) {
                    Ok(prepared) => prepared,
                    Err(error) => return Box::pin(async move { Err(error) }),
                };
                let port = Arc::clone(&self.port);
                Box::pin(async move {
                    let response = port.execute(prepared, cancellation).await?;
                    response.validate()?;
                    serde_json::to_value(response).map_err(|error| {
                        RpcError::internal(format!(
                            "compat/http response serialization failed: {error}"
                        ))
                    })
                })
            }
            METHOD_COMPAT_SUBSCRIBE => {
                let parsed: SseSubscriptionParams = match serde_json::from_value(params) {
                    Ok(value) => value,
                    Err(error) => {
                        return Box::pin(
                            async move { Err(RpcError::invalid_params(error.to_string())) },
                        )
                    }
                };
                if parsed.schema_version != SSE_SUBSCRIPTION_SCHEMA_VERSION
                    || !valid_stable_id(&parsed.stream_id)
                    || validate_path(&parsed.path, AllowedHttpMethod::Get).is_err()
                {
                    return Box::pin(async {
                        Err(RpcError::invalid_params("Invalid compat/subscribe params."))
                    });
                }
                self.port.subscribe(parsed, cancellation)
            }
            METHOD_COMPAT_UNSUBSCRIBE => {
                let parsed: SseUnsubscribeParams = match serde_json::from_value(params) {
                    Ok(value) => value,
                    Err(error) => {
                        return Box::pin(
                            async move { Err(RpcError::invalid_params(error.to_string())) },
                        )
                    }
                };
                if parsed.schema_version != SSE_UNSUBSCRIBE_SCHEMA_VERSION
                    || !valid_stable_id(&parsed.stream_id)
                {
                    return Box::pin(async {
                        Err(RpcError::invalid_params(
                            "Invalid compat/unsubscribe params.",
                        ))
                    });
                }
                self.port.unsubscribe(parsed, cancellation)
            }
            METHOD_EVENTS_REPLAY => {
                let parsed: ReplayParams = match serde_json::from_value(params) {
                    Ok(value) => value,
                    Err(error) => {
                        return Box::pin(
                            async move { Err(RpcError::invalid_params(error.to_string())) },
                        )
                    }
                };
                self.port.replay(parsed, cancellation)
            }
            _ => Box::pin(async move { Err(RpcError::method_not_found(&method)) }),
        }
    }
}

fn validate_path(path: &str, method: AllowedHttpMethod) -> Result<(), RpcError> {
    if path.is_empty()
        || path.len() > 4096
        || !path.starts_with('/')
        || path.starts_with("//")
        || path.contains("\\")
        || path.contains('#')
        || path.contains("://")
        || path.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(RpcError::invalid_params(
            "compat/http path is not a safe relative path.",
        ));
    }
    let lower = path.to_ascii_lowercase();
    if ["%2e", "%2f", "%5c", "%00"]
        .iter()
        .any(|encoded| lower.contains(encoded))
    {
        return Err(RpcError::invalid_params(
            "compat/http path contains an encoded path-control sequence.",
        ));
    }
    let route = path.split('?').next().unwrap_or(path);
    if route
        .split('/')
        .any(|segment| matches!(segment, "." | ".."))
    {
        return Err(RpcError::invalid_params(
            "compat/http path traversal is forbidden.",
        ));
    }
    if route == "/api/v1/app-server" || route.starts_with("/api/v1/app-server/") {
        return Err(RpcError::invalid_params(
            "Recursive app-server compatibility routing is forbidden.",
        ));
    }
    let segments = route
        .strip_prefix('/')
        .map(|value| value.split('/').collect::<Vec<_>>())
        .filter(|segments| {
            !segments.is_empty() && segments.iter().all(|segment| !segment.is_empty())
        })
        .ok_or_else(|| RpcError::invalid_params("compat/http route shape is invalid."))?;
    if !is_allowed_product_route(method, &segments) {
        return Err(RpcError::invalid_params(
            "compat/http method and path are not in the explicit code-owned product route allow-list.",
        ));
    }
    Ok(())
}

fn is_allowed_product_route(method: AllowedHttpMethod, segments: &[&str]) -> bool {
    matches!(
        (method, segments),
        (AllowedHttpMethod::Get, ["api", "health"])
    ) || is_allowed_k001_fixture_route(method, segments)
        || is_allowed_agent_route(method, segments)
        || is_allowed_core_route(method, segments)
        || is_allowed_legacy_route(method, segments)
}

fn is_allowed_k001_fixture_route(method: AllowedHttpMethod, segments: &[&str]) -> bool {
    match (method, segments) {
        (AllowedHttpMethod::Post, ["api", "v1", "k001", "json"])
        | (AllowedHttpMethod::Post, ["api", "v1", "k001", "binary"])
        | (AllowedHttpMethod::Get, ["api", "v1", "k001", "binary"])
        | (AllowedHttpMethod::Get, ["api", "v1", "k001", "slow"])
        | (AllowedHttpMethod::Get, ["api", "v1", "k001", "oversize"])
        | (AllowedHttpMethod::Get, ["api", "v1", "k001", "events"])
        | (AllowedHttpMethod::Get, ["api", "v1", "k001", "resync"]) => true,
        _ => false,
    }
}

fn is_allowed_agent_route(method: AllowedHttpMethod, segments: &[&str]) -> bool {
    match (method, segments) {
        (AllowedHttpMethod::Get, ["api", "v1", "agent", "threads"])
        | (AllowedHttpMethod::Get, ["api", "v1", "agent", "domain-packs"])
        | (AllowedHttpMethod::Get, ["api", "v1", "agent", "materials"])
        | (AllowedHttpMethod::Get, ["api", "v1", "agent", "material-textures"])
        | (AllowedHttpMethod::Get, ["api", "v1", "agent", "components"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "threads"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "material-textures"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "provider:check"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "blockouts"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "blockouts:concept-preview"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "blockouts:segment"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "blockouts:commit"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "imports:glb"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "reference-evidence:create"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "skills", "surface-adornment:enable"]) => {
            true
        }
        (
            AllowedHttpMethod::Post,
            ["api", "v1", "agent", "projects", project_id, "reference-guided-rebuild:preview"],
        ) => valid_route_id(project_id),
        (
            AllowedHttpMethod::Get,
            ["api", "v1", "agent", "projects", project_id, "reference-evidence"],
        ) => valid_route_id(project_id),
        (
            AllowedHttpMethod::Get,
            ["api", "v1", "agent", "projects", project_id, "reference-evidence", evidence_route],
        ) => {
            valid_route_id(project_id)
                && evidence_route
                    .strip_suffix(":content")
                    .is_some_and(valid_route_id)
        }
        (
            AllowedHttpMethod::Get,
            ["api", "v1", "agent", "projects", project_id, "reference-guided-rebuild-plans", rebuild_plan_id],
        ) => valid_route_id(project_id) && valid_route_id(rebuild_plan_id),
        (
            AllowedHttpMethod::Get,
            ["api", "v1", "agent", "projects", project_id, "turns", turn_id, "single-results", preview],
        ) => {
            valid_route_id(project_id)
                && valid_route_id(turn_id)
                && has_stable_id_suffix(preview, ":preview.glb")
        }
        (
            AllowedHttpMethod::Post,
            ["api", "v1", "agent", "projects", project_id, "turns", turn_id, "single-results", preview],
        ) => {
            valid_route_id(project_id)
                && valid_route_id(turn_id)
                && (has_stable_id_suffix(preview, ":confirm")
                    || has_stable_id_suffix(preview, ":reject"))
        }
        (AllowedHttpMethod::Get, ["api", "v1", "agent", "material-textures", texture_asset_id]) => {
            valid_route_id(texture_asset_id)
        }
        (
            AllowedHttpMethod::Post,
            ["api", "v1", "agent", "provider-checks", check_id, "cancel"],
        ) => valid_route_id(check_id),
        (AllowedHttpMethod::Get, ["api", "v1", "agent", "asset-versions", asset]) => {
            valid_route_id(asset)
                || has_stable_id_suffix(asset, ":preview.glb")
                || has_stable_id_suffix(asset, ":model.glb")
                || has_stable_id_suffix(asset, ":render")
                || has_stable_id_suffix(asset, ":render-package")
        }
        (AllowedHttpMethod::Post, ["api", "v1", "agent", "asset-versions", asset]) => {
            has_stable_id_suffix(asset, ":quality") || has_stable_id_suffix(asset, ":export")
        }
        (
            AllowedHttpMethod::Post,
            ["api", "v1", "agent", "asset-versions", asset_id, "components"],
        )
        | (
            AllowedHttpMethod::Post,
            ["api", "v1", "agent", "asset-versions", asset_id, "change-sets"],
        )
        | (
            AllowedHttpMethod::Post,
            ["api", "v1", "agent", "asset-versions", asset_id, "surface-adornments:preview"],
        ) => valid_route_id(asset_id),
        (
            AllowedHttpMethod::Get,
            ["api", "v1", "agent", "asset-versions", asset_id, "components:compatible"],
        )
        | (
            AllowedHttpMethod::Get,
            ["api", "v1", "agent", "asset-versions", asset_id, "structure-suggestions"],
        ) => valid_route_id(asset_id),
        (
            AllowedHttpMethod::Get,
            ["api", "v1", "agent", "asset-versions", asset_id, "parts", part_id, "semantic-proportions"],
        ) => valid_route_id(asset_id) && valid_route_id(part_id),
        (
            AllowedHttpMethod::Post,
            ["api", "v1", "agent", "asset-versions", asset_id, "parts", part_id, "component-recipes:expand"],
        ) => valid_route_id(asset_id) && valid_route_id(part_id),
        (AllowedHttpMethod::Get, ["api", "v1", "agent", "quality-reports", report_id]) => {
            valid_route_id(report_id)
        }
        (AllowedHttpMethod::Get, ["api", "v1", "agent", "change-sets", change_set]) => {
            has_stable_id_suffix(change_set, ":preview.glb")
        }
        (AllowedHttpMethod::Post, ["api", "v1", "agent", "change-sets", change_set]) => {
            has_stable_id_suffix(change_set, ":preview")
                || has_stable_id_suffix(change_set, ":confirm")
                || has_stable_id_suffix(change_set, ":reject")
        }
        (AllowedHttpMethod::Get, ["api", "v1", "agent", "threads", thread_id]) => {
            valid_route_id(thread_id)
        }
        (AllowedHttpMethod::Get, ["api", "v1", "agent", "threads", thread_id, "events"]) => {
            valid_route_id(thread_id)
        }
        (AllowedHttpMethod::Post, ["api", "v1", "agent", "threads", thread_id, "turns"])
        | (AllowedHttpMethod::Post, ["api", "v1", "agent", "threads", thread_id, "approvals"]) => {
            valid_route_id(thread_id)
        }
        (AllowedHttpMethod::Post, ["api", "v1", "agent", "turns", turn_id, "cancel"]) => {
            valid_route_id(turn_id)
        }
        (AllowedHttpMethod::Post, ["api", "v1", "agent", "approvals", approval_id, "resolve"]) => {
            valid_route_id(approval_id)
        }
        _ => false,
    }
}

fn is_allowed_core_route(method: AllowedHttpMethod, segments: &[&str]) -> bool {
    match (method, segments) {
        (AllowedHttpMethod::Get, ["api", "v1", "projects"])
        | (AllowedHttpMethod::Post, ["api", "v1", "projects"])
        | (AllowedHttpMethod::Get, ["api", "v1", "module-assets"])
        | (AllowedHttpMethod::Post, ["api", "v1", "concept-jobs", "work-once"]) => true,
        (AllowedHttpMethod::Get, ["api", "v1", "projects", project_id]) => {
            valid_route_id(project_id)
        }
        (AllowedHttpMethod::Post, ["api", "v1", "projects", project_action]) => {
            has_stable_id_suffix(project_action, ":initialize-workbench")
        }
        (AllowedHttpMethod::Get, ["api", "v1", "projects", project_id, "active-design"])
        | (
            AllowedHttpMethod::Get,
            ["api", "v1", "projects", project_id, "active-design:navigation"],
        )
        | (AllowedHttpMethod::Get, ["api", "v1", "projects", project_id, "variants"])
        | (AllowedHttpMethod::Get, ["api", "v1", "projects", project_id, "change-sets"])
        | (
            AllowedHttpMethod::Get,
            ["api", "v1", "projects", project_id, "change-set-audit-exports"],
        ) => valid_route_id(project_id),
        (AllowedHttpMethod::Post, ["api", "v1", "projects", project_id, action]) => {
            valid_route_id(project_id)
                && matches!(
                    *action,
                    "active-design:render-preset"
                        | "active-design:part-display"
                        | "active-design:select"
                        | "active-design:convert-legacy"
                        | "active-design:undo"
                        | "active-design:redo"
                        | "brief:interpret"
                        | "variants"
                        | "change-set-audit-exports"
                )
        }
        (
            AllowedHttpMethod::Post,
            ["api", "v1", "projects", project_id, "variants", variant_action],
        ) => valid_route_id(project_id) && has_stable_id_suffix(variant_action, ":select"),
        (AllowedHttpMethod::Get, ["api", "v1", "versions", version_id]) => {
            valid_route_id(version_id)
        }
        (AllowedHttpMethod::Post, ["api", "v1", "versions", version_id, action]) => {
            valid_route_id(version_id)
                && matches!(
                    *action,
                    "quality-runs:inspect"
                        | "quality-runs:inspect:enqueue"
                        | "exports"
                        | "change-sets"
                        | "change-sets:connector-snap"
                        | "change-sets:plan"
                )
        }
        (
            AllowedHttpMethod::Get,
            ["api", "v1", "module-assets", module_id, "file" | "thumbnail"],
        ) => valid_route_id(module_id),
        (AllowedHttpMethod::Put, ["api", "v1", "module-assets", module_id, "catalog-metadata"]) => {
            valid_route_id(module_id)
        }
        (AllowedHttpMethod::Get, ["api", "v1", "module-graphs", graph_id]) => {
            valid_route_id(graph_id)
        }
        (AllowedHttpMethod::Get, ["api", "v1", "quality-runs", quality_run_id]) => {
            valid_route_id(quality_run_id)
        }
        (AllowedHttpMethod::Get, ["api", "v1", "jobs", job_id]) => valid_route_id(job_id),
        (AllowedHttpMethod::Get, ["api", "v1", "exports", export_id, file]) => {
            valid_route_id(export_id)
                && matches!(
                    *file,
                    "file"
                        | "combined.glb"
                        | "combined.obj"
                        | "combined.mtl"
                        | "preview.png"
                        | "exploded.png"
                        | "renders.zip"
                        | "turntable.mp4"
                )
        }
        (AllowedHttpMethod::Get, ["api", "v1", "exports", export_id, "views", view]) => {
            valid_route_id(export_id) && matches!(*view, "front.png" | "side.png" | "top.png")
        }
        (AllowedHttpMethod::Get, ["api", "v1", "exports", export_id, "turntable", frame]) => {
            valid_route_id(export_id) && valid_turntable_frame(frame)
        }
        (
            AllowedHttpMethod::Get,
            ["api", "v1", "change-set-audit-exports", audit_export_id, "file"],
        ) => valid_route_id(audit_export_id),
        (AllowedHttpMethod::Post, ["api", "v1", "change-sets", change_set]) => {
            has_stable_id_suffix(change_set, ":preview")
                || has_stable_id_suffix(change_set, ":confirm")
                || has_stable_id_suffix(change_set, ":reject")
        }
        _ => false,
    }
}

fn is_allowed_legacy_route(method: AllowedHttpMethod, segments: &[&str]) -> bool {
    match (method, segments) {
        (AllowedHttpMethod::Get, ["api", "weapons"])
        | (AllowedHttpMethod::Post, ["api", "weapons"])
        | (AllowedHttpMethod::Get, ["api", "jobs"])
        | (AllowedHttpMethod::Get, ["api", "provider-settings"])
        | (AllowedHttpMethod::Post, ["api", "runtime", "recover"])
        | (AllowedHttpMethod::Post, ["api", "runtime", "work-once"]) => true,
        (AllowedHttpMethod::Get, ["api", "weapons", weapon_id]) => valid_route_id(weapon_id),
        (AllowedHttpMethod::Get, ["api", "weapons", weapon_id, "creative-graph"])
        | (AllowedHttpMethod::Post, ["api", "weapons", weapon_id, "interpretation"])
        | (AllowedHttpMethod::Post, ["api", "weapons", weapon_id, "patch"])
        | (AllowedHttpMethod::Post, ["api", "weapons", weapon_id, "generate-3d"])
        | (AllowedHttpMethod::Post, ["api", "weapons", weapon_id, "export-unity"]) => {
            valid_route_id(weapon_id)
        }
        (AllowedHttpMethod::Post, ["api", "weapons", weapon_id, "recast", "confirm"]) => {
            valid_route_id(weapon_id)
        }
        (
            AllowedHttpMethod::Post,
            ["api", "weapons", weapon_id, "versions", version_id, "assets" | "activate"],
        ) => valid_route_id(weapon_id) && valid_route_id(version_id),
        (AllowedHttpMethod::Get, ["api", "assets", asset_id])
        | (AllowedHttpMethod::Get, ["api", "assets", asset_id, "file"])
        | (AllowedHttpMethod::Post, ["api", "assets", asset_id, "reveal"]) => {
            valid_route_id(asset_id)
        }
        (AllowedHttpMethod::Get, ["api", "jobs", job_id])
        | (AllowedHttpMethod::Get, ["api", "jobs", job_id, "runtime"])
        | (AllowedHttpMethod::Get, ["api", "jobs", job_id, "actions"])
        | (AllowedHttpMethod::Get, ["api", "jobs", job_id, "events"])
        | (AllowedHttpMethod::Post, ["api", "jobs", job_id, "retry"])
        | (AllowedHttpMethod::Post, ["api", "jobs", job_id, "cancel"]) => valid_route_id(job_id),
        (AllowedHttpMethod::Post, ["api", "jobs", job_id, "retry-from", step_name]) => {
            valid_route_id(job_id) && valid_route_id(step_name)
        }
        _ => false,
    }
}

fn valid_route_id(value: &str) -> bool {
    valid_stable_id(value)
}

fn has_stable_id_suffix(value: &str, suffix: &str) -> bool {
    value
        .strip_suffix(suffix)
        .is_some_and(|stable_id| valid_route_id(stable_id))
}

fn valid_turntable_frame(value: &str) -> bool {
    value.strip_suffix(".png").is_some_and(|frame| {
        !frame.is_empty() && frame.len() <= 20 && frame.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn validate_headers(headers: &[(String, String)]) -> Result<(), RpcError> {
    if headers.len() > 64 {
        return Err(RpcError::invalid_params(
            "compat/http contains too many headers.",
        ));
    }
    let mut seen = HashSet::new();
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        let allowed = matches!(
            lower.as_str(),
            "accept"
                | "content-type"
                | "if-match"
                | "if-none-match"
                | "range"
                | "cache-control"
                | "last-event-id"
                | "idempotency-key"
                | "x-client-request-id"
                | "x-provider-check-id"
        ) || lower.starts_with("x-forgecad-");
        let sensitive = [
            "authorization",
            "cookie",
            "api-key",
            "apikey",
            "provider-key",
            "secret",
            "token",
        ]
        .iter()
        .any(|marker| lower.contains(marker));
        if !allowed
            || sensitive
            || !seen.insert(lower)
            || name.is_empty()
            || name.len() > 128
            || value.len() > 8192
            || name
                .bytes()
                .any(|byte| !byte.is_ascii_alphanumeric() && byte != b'-')
            || value
                .bytes()
                .any(|byte| byte == b'\r' || byte == b'\n' || byte == 0)
        {
            return Err(RpcError::invalid_params(
                "compat/http contains a forbidden or malformed header.",
            ));
        }
    }
    Ok(())
}

fn body_within_budget(
    body: &forgecad_app_server_protocol::ProtocolHttpBody,
    max_raw_bytes: usize,
    max_encoded_bytes: usize,
) -> bool {
    match body {
        forgecad_app_server_protocol::ProtocolHttpBody::Empty => true,
        forgecad_app_server_protocol::ProtocolHttpBody::Utf8 { data } => {
            data.len() <= max_raw_bytes && data.len() <= max_encoded_bytes
        }
        forgecad_app_server_protocol::ProtocolHttpBody::Base64 { data } => {
            if data.len() > max_encoded_bytes
                || data.len() % 4 != 0
                || !data
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
            {
                return false;
            }
            let padding = data.bytes().rev().take_while(|byte| *byte == b'=').count();
            if padding > 2 || data[..data.len().saturating_sub(padding)].contains('=') {
                return false;
            }
            data.len() / 4 * 3 - padding <= max_raw_bytes
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use forgecad_app_server_protocol::{
        ProtocolHttpBody, HTTP_COMPAT_REQUEST_SCHEMA_VERSION, HTTP_COMPAT_RESPONSE_SCHEMA_VERSION,
    };

    use super::*;

    #[derive(Default)]
    struct RecordingPort {
        paths: Mutex<Vec<String>>,
    }

    impl CompatibilityHttpPort for RecordingPort {
        fn execute(
            &self,
            request: PreparedCompatHttpRequest,
            _cancellation: CancellationToken,
        ) -> CompatHttpFuture {
            self.paths.lock().unwrap().push(request.path);
            Box::pin(async {
                Ok(CompatHttpResponse {
                    schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
                    status: 200,
                    headers: vec![],
                    body: ProtocolHttpBody::Empty,
                })
            })
        }
    }

    fn request_with_method(path: &str, method: &str) -> CompatHttpRequest {
        CompatHttpRequest {
            schema_version: HTTP_COMPAT_REQUEST_SCHEMA_VERSION.into(),
            path: path.into(),
            method: method.into(),
            headers: vec![],
            body: ProtocolHttpBody::Empty,
        }
    }

    fn request(path: &str) -> CompatHttpRequest {
        request_with_method(path, "GET")
    }

    #[test]
    fn endpoint_accepts_only_explicit_loopback_http() {
        for endpoint in [
            "http://127.0.0.1:8000",
            "http://localhost:8000",
            "http://[::1]:8000",
        ] {
            assert!(LocalAgentEndpoint::parse(endpoint).is_ok(), "{endpoint}");
        }
        for endpoint in [
            "https://127.0.0.1:8000",
            "http://example.com:8000",
            "http://127.0.0.1",
            "http://user@127.0.0.1:8000/path",
        ] {
            assert!(LocalAgentEndpoint::parse(endpoint).is_err(), "{endpoint}");
        }
    }

    #[test]
    fn policy_covers_current_forge_api_routes_and_k001_fixtures() {
        let port = Arc::new(RecordingPort::default());
        let adapter = CompatibilityAdapter::new(
            LocalAgentEndpoint::parse("http://127.0.0.1:8000").unwrap(),
            port,
        );
        let allowed = [
            ("GET", "/api/health"),
            ("POST", "/api/v1/k001/json?source=transport"),
            ("GET", "/api/v1/k001/binary"),
            ("POST", "/api/v1/k001/binary"),
            ("GET", "/api/v1/k001/slow"),
            ("GET", "/api/v1/k001/oversize"),
            ("GET", "/api/v1/k001/events?after=9"),
            ("GET", "/api/v1/k001/resync?after=20"),
            ("GET", "/api/v1/agent/threads"),
            ("POST", "/api/v1/agent/threads"),
            ("GET", "/api/v1/agent/domain-packs"),
            ("GET", "/api/v1/agent/materials"),
            ("GET", "/api/v1/agent/material-textures?source=builtin"),
            (
                "GET",
                "/api/v1/agent/material-textures/asset_tex_000000000000000000000000",
            ),
            ("POST", "/api/v1/agent/material-textures"),
            ("POST", "/api/v1/agent/provider:check"),
            ("POST", "/api/v1/agent/provider-checks/check_1/cancel"),
            ("POST", "/api/v1/agent/blockouts"),
            ("POST", "/api/v1/agent/blockouts:concept-preview"),
            ("POST", "/api/v1/agent/blockouts:segment"),
            ("POST", "/api/v1/agent/blockouts:commit"),
            ("GET", "/api/v1/agent/asset-versions/assetver_1"),
            ("POST", "/api/v1/agent/imports:glb"),
            ("POST", "/api/v1/agent/reference-evidence:create"),
            (
                "POST",
                "/api/v1/agent/projects/prj_1/reference-guided-rebuild:preview",
            ),
            (
                "GET",
                "/api/v1/agent/projects/prj_1/turns/turn_1/single-results/preview_1:preview.glb",
            ),
            (
                "POST",
                "/api/v1/agent/projects/prj_1/turns/turn_1/single-results/preview_1:confirm",
            ),
            (
                "POST",
                "/api/v1/agent/projects/prj_1/turns/turn_1/single-results/preview_1:reject",
            ),
            ("POST", "/api/v1/agent/skills/surface-adornment:enable"),
            ("POST", "/api/v1/agent/asset-versions/assetver_1:quality"),
            ("GET", "/api/v1/agent/quality-reports/quality_1"),
            ("POST", "/api/v1/agent/asset-versions/assetver_1/components"),
            ("POST", "/api/v1/agent/asset-versions/assetver_1:export"),
            ("GET", "/api/v1/agent/asset-versions/assetver_1:preview.glb"),
            ("GET", "/api/v1/agent/asset-versions/assetver_1:model.glb"),
            (
                "GET",
                "/api/v1/agent/asset-versions/assetver_1:render?width=640",
            ),
            (
                "GET",
                "/api/v1/agent/asset-versions/assetver_1:render-package?width=640&height=480",
            ),
            ("GET", "/api/v1/agent/components?project_id=prj_1"),
            (
                "GET",
                "/api/v1/agent/asset-versions/assetver_1/components:compatible?part_id=part_1",
            ),
            (
                "GET",
                "/api/v1/agent/asset-versions/assetver_1/structure-suggestions",
            ),
            (
                "GET",
                "/api/v1/agent/asset-versions/assetver_1/parts/part_1/semantic-proportions",
            ),
            (
                "POST",
                "/api/v1/agent/asset-versions/assetver_1/change-sets",
            ),
            (
                "POST",
                "/api/v1/agent/asset-versions/assetver_1/surface-adornments:preview",
            ),
            ("POST", "/api/v1/agent/change-sets/change_1:preview"),
            ("GET", "/api/v1/agent/change-sets/change_1:preview.glb"),
            ("POST", "/api/v1/agent/change-sets/change_1:confirm"),
            ("POST", "/api/v1/agent/change-sets/change_1:reject"),
            ("GET", "/api/v1/agent/threads/thread_1"),
            ("POST", "/api/v1/agent/threads/thread_1/turns"),
            ("POST", "/api/v1/agent/turns/turn_1/cancel"),
            ("POST", "/api/v1/agent/threads/thread_1/approvals"),
            ("POST", "/api/v1/agent/approvals/approval_1/resolve"),
            ("GET", "/api/v1/agent/threads/thread_1/events?after=0"),
            ("GET", "/api/v1/projects"),
            ("POST", "/api/v1/projects"),
            ("GET", "/api/v1/projects/prj_1"),
            ("POST", "/api/v1/projects/prj_1:initialize-workbench"),
            ("GET", "/api/v1/projects/prj_1/active-design"),
            ("GET", "/api/v1/projects/prj_1/active-design:navigation"),
            ("POST", "/api/v1/projects/prj_1/active-design:render-preset"),
            ("POST", "/api/v1/projects/prj_1/active-design:part-display"),
            ("POST", "/api/v1/projects/prj_1/active-design:select"),
            (
                "POST",
                "/api/v1/projects/prj_1/active-design:convert-legacy",
            ),
            ("POST", "/api/v1/projects/prj_1/active-design:undo"),
            ("POST", "/api/v1/projects/prj_1/active-design:redo"),
            ("GET", "/api/v1/versions/ver_1"),
            ("GET", "/api/v1/module-assets?pack_id=pack_1"),
            ("PUT", "/api/v1/module-assets/module_1/catalog-metadata"),
            ("GET", "/api/v1/module-assets/module_1/file"),
            ("GET", "/api/v1/module-assets/module_1/thumbnail"),
            ("GET", "/api/v1/module-graphs/mg_1"),
            ("POST", "/api/v1/versions/ver_1/quality-runs:inspect"),
            (
                "POST",
                "/api/v1/versions/ver_1/quality-runs:inspect:enqueue",
            ),
            ("GET", "/api/v1/quality-runs/quality_1"),
            ("GET", "/api/v1/jobs/job_1"),
            ("POST", "/api/v1/concept-jobs/work-once"),
            ("GET", "/api/v1/projects/prj_1/variants"),
            ("POST", "/api/v1/projects/prj_1/brief:interpret"),
            ("POST", "/api/v1/projects/prj_1/variants"),
            ("POST", "/api/v1/projects/prj_1/variants/variant_1:select"),
            ("POST", "/api/v1/versions/ver_1/exports"),
            ("GET", "/api/v1/exports/export_1/file"),
            ("GET", "/api/v1/exports/export_1/combined.glb"),
            ("GET", "/api/v1/exports/export_1/combined.obj"),
            ("GET", "/api/v1/exports/export_1/combined.mtl"),
            ("GET", "/api/v1/exports/export_1/preview.png"),
            ("GET", "/api/v1/exports/export_1/exploded.png"),
            ("GET", "/api/v1/exports/export_1/renders.zip"),
            ("GET", "/api/v1/exports/export_1/views/front.png"),
            ("GET", "/api/v1/exports/export_1/views/side.png"),
            ("GET", "/api/v1/exports/export_1/views/top.png"),
            ("GET", "/api/v1/exports/export_1/turntable/12.png"),
            ("GET", "/api/v1/exports/export_1/turntable.mp4"),
            ("POST", "/api/v1/versions/ver_1/change-sets"),
            ("POST", "/api/v1/versions/ver_1/change-sets:connector-snap"),
            ("POST", "/api/v1/versions/ver_1/change-sets:plan"),
            ("GET", "/api/v1/projects/prj_1/change-sets?limit=20"),
            ("POST", "/api/v1/projects/prj_1/change-set-audit-exports"),
            (
                "GET",
                "/api/v1/projects/prj_1/change-set-audit-exports?limit=20",
            ),
            ("GET", "/api/v1/change-set-audit-exports/audit_1/file"),
            ("POST", "/api/v1/change-sets/change_1:preview"),
            ("POST", "/api/v1/change-sets/change_1:confirm"),
            ("POST", "/api/v1/change-sets/change_1:reject"),
            ("GET", "/api/weapons"),
            ("POST", "/api/weapons"),
            ("GET", "/api/weapons/weapon_1"),
            ("POST", "/api/weapons/weapon_1/interpretation"),
            ("POST", "/api/weapons/weapon_1/recast/confirm"),
            ("GET", "/api/weapons/weapon_1/creative-graph"),
            ("GET", "/api/assets/asset_1"),
            ("GET", "/api/assets/asset_1/file"),
            ("POST", "/api/assets/asset_1/reveal?dry_run=true"),
            ("GET", "/api/jobs"),
            ("GET", "/api/jobs/job_1"),
            ("GET", "/api/jobs/job_1/runtime"),
            ("GET", "/api/jobs/job_1/actions?limit=20"),
            ("GET", "/api/jobs/job_1/events?after=evt_1"),
            ("POST", "/api/runtime/recover"),
            ("POST", "/api/runtime/work-once"),
            ("POST", "/api/jobs/job_1/retry"),
            ("POST", "/api/jobs/job_1/retry-from/geometry"),
            ("POST", "/api/jobs/job_1/cancel"),
            ("POST", "/api/weapons/weapon_1/versions/ver_1/assets"),
            ("POST", "/api/weapons/weapon_1/versions/ver_1/activate"),
            ("POST", "/api/weapons/weapon_1/patch"),
            ("POST", "/api/weapons/weapon_1/generate-3d"),
            ("POST", "/api/weapons/weapon_1/export-unity"),
            ("GET", "/api/provider-settings"),
        ];
        for (method, path) in allowed {
            adapter
                .prepare(request_with_method(path, method))
                .unwrap_or_else(|error| panic!("{method} {path}: {error:?}"));
        }
    }

    #[test]
    fn policy_rejects_unknown_management_wrong_methods_and_path_injection() {
        let port = Arc::new(RecordingPort::default());
        let adapter = CompatibilityAdapter::new(
            LocalAgentEndpoint::parse("http://127.0.0.1:8000").unwrap(),
            port,
        );
        let rejected = [
            ("GET", "https://evil.test/api/v1/agent/threads"),
            ("GET", "//evil.test/api/v1"),
            ("GET", "/api/v1/../secret"),
            ("GET", "/api/v1/%2e%2e/secret"),
            ("GET", "/api/v1/projects/prj_1%2fadmin"),
            ("GET", "/api/v1/app-server"),
            ("GET", "/api/v1/app-server/send"),
            ("GET", "/api/v1"),
            ("GET", "/api/v1/agent"),
            ("GET", "/api/v1/agent/admin"),
            ("GET", "/api/v1/agent/threads/"),
            ("GET", "/api/v1/projects/not%20stable"),
            ("GET", "/api/v1/projects/prj_1/admin"),
            ("POST", "/api/v1/projects/prj_1/delete"),
            ("GET", "/api/v1/admin/secrets"),
            ("GET", "/api/v1/k001/unknown"),
            ("GET", "/api/weapons/weapon_1/delete"),
            ("GET", "/api/runtime"),
            ("POST", "/api/provider-settings"),
            ("POST", "/api/health"),
            ("GET", "/api/v1/agent/provider:check"),
            ("POST", "/api/v1/agent/asset-versions/assetver_1:model.glb"),
            ("PUT", "/api/v1/agent/threads"),
            ("PATCH", "/api/v1/projects/prj_1"),
            ("DELETE", "/api/v1/projects/prj_1"),
            ("GET", "/docs"),
        ];
        for (method, path) in rejected {
            assert!(
                adapter.prepare(request_with_method(path, method)).is_err(),
                "{method} {path}"
            );
        }
        for path in [
            "/api/v1/projects//active-design",
            "/api/v1/projects/prj_1\\active-design",
            "/api/v1/projects/prj_1#fragment",
            "/api/v1/projects/prj_1?redirect=https://evil.test",
        ] {
            assert!(adapter.prepare(request(path)).is_err(), "{path}");
        }
    }

    #[test]
    fn sensitive_headers_are_rejected() {
        let port = Arc::new(RecordingPort::default());
        let adapter = CompatibilityAdapter::new(
            LocalAgentEndpoint::parse("http://127.0.0.1:8000").unwrap(),
            port,
        );
        for header in [
            "authorization",
            "cookie",
            "host",
            "proxy-authorization",
            "x-forgecad-provider-key",
            "x-forgecad-api-key",
            "x-forgecad-secret",
            "x-forgecad-session-token",
        ] {
            let mut value = request("/api/v1/agent/threads");
            value.headers.push((header.into(), "secret".into()));
            assert!(adapter.prepare(value).is_err(), "{header}");
        }
        let mut duplicate = request("/api/v1/agent/threads");
        duplicate
            .headers
            .push(("X-Client-Request-Id".into(), "one".into()));
        duplicate
            .headers
            .push(("x-client-request-id".into(), "two".into()));
        assert!(adapter.prepare(duplicate).is_err());
        let mut allowed = request_with_method("/api/v1/agent/provider:check", "POST");
        allowed
            .headers
            .push(("X-Provider-Check-Id".into(), "check_1".into()));
        adapter.prepare(allowed).unwrap();
    }

    #[test]
    fn compatibility_body_has_separate_raw_and_encoded_budgets() {
        assert_eq!(MAX_RAW_COMPAT_BODY_BYTES, 49_283_072);
        assert_eq!(MAX_ENCODED_COMPAT_BODY_BYTES, 65_710_764);
        assert!(MAX_ENCODED_COMPAT_BODY_BYTES < 64 * 1024 * 1024);
        assert!(body_within_budget(
            &ProtocolHttpBody::Utf8 {
                data: "xxxxx".into(),
            },
            5,
            8,
        ));
        assert!(!body_within_budget(
            &ProtocolHttpBody::Utf8 {
                data: "xxxxxx".into(),
            },
            5,
            8,
        ));
        assert!(!body_within_budget(
            &ProtocolHttpBody::Base64 {
                data: "AAAAAAAA".into(),
            },
            6,
            4,
        ));
    }
}
