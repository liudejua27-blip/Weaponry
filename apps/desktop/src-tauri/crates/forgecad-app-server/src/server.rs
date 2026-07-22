use std::{
    collections::{HashMap, VecDeque},
    future::{poll_fn, Future},
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering as AtomicOrdering},
        Arc, Mutex,
    },
    task::Poll,
};

use forgecad_app_server_protocol::{
    parse_client_message, select_protocol_version, AckParams, AppServerCursor, CancelParams,
    ClientMessage, InitializeParams, InitializeResult, InitializedParams, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse, MigrationState, MigrationStateOwner, ProtocolCapabilities,
    ProtocolLimits, ReplayParams, ReplayResult, RequestId, RpcError, ServerInfo,
    ALREADY_INITIALIZED, CURSOR_RESYNC_REQUIRED, DEFAULT_MAX_FRAME_BYTES, DUPLICATE_REQUEST_ID,
    FORGECAD_PROTOCOL_VERSION, INITIALIZE_RESULT_SCHEMA_VERSION, METHOD_EVENTS_REPLAY,
    METHOD_INITIALIZE, METHOD_INITIALIZED, METHOD_NOTIFICATION_ACK, METHOD_REQUEST_CANCEL,
    REQUEST_CANCELLED, SERVER_OVERLOADED, UNKNOWN_REQUEST_ID,
};
use serde_json::{json, Value};
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};

use crate::{
    canonical::{canonical_json, sha256_hex},
    CancellationToken, EventQueue, RequestHandler,
};

#[derive(Debug, Clone)]
pub struct AppServerConfig {
    pub max_frame_bytes: usize,
    pub max_in_flight_requests: usize,
    pub max_cached_requests: usize,
    pub outbound_event_capacity: usize,
    pub replay_event_capacity: usize,
    pub replay_byte_capacity: usize,
    pub max_notification_bytes: usize,
}

impl Default for AppServerConfig {
    fn default() -> Self {
        Self {
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_in_flight_requests: 32,
            max_cached_requests: 1024,
            outbound_event_capacity: 128,
            replay_event_capacity: 2048,
            replay_byte_capacity: 64 * 1024 * 1024,
            max_notification_bytes: 16 * 1024 * 1024,
        }
    }
}

pub struct OpenConnection {
    pub connection_id: String,
    /// Serialized JSON-RPC notifications for the single Tauri event channel.
    pub notifications: mpsc::Receiver<String>,
}

pub struct AppServer {
    handler: Arc<dyn RequestHandler>,
    config: AppServerConfig,
    connections: Mutex<HashMap<String, Arc<Connection>>>,
    next_connection: AtomicU64,
}

struct Connection {
    id: String,
    handshake: Mutex<HandshakeState>,
    requests: Mutex<RequestRegistry>,
    in_flight: Arc<Semaphore>,
    events: EventQueue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HandshakeState {
    AwaitingInitialize,
    AwaitingInitialized { protocol_version: String },
    Ready,
}

struct RequestRegistry {
    entries: HashMap<RequestId, RequestEntry>,
    order: VecDeque<RequestId>,
    max_entries: usize,
}

struct RequestEntry {
    fingerprint: String,
    cancellation: CancellationToken,
    response: Option<JsonRpcResponse>,
}

enum BeginRequest {
    New,
    Replay(JsonRpcResponse),
}

impl AppServer {
    pub fn new(handler: Arc<dyn RequestHandler>, config: AppServerConfig) -> Self {
        Self {
            handler,
            config,
            connections: Mutex::new(HashMap::new()),
            next_connection: AtomicU64::new(1),
        }
    }

    pub fn open_connection(&self) -> OpenConnection {
        let id = format!(
            "connection_{}",
            self.next_connection.fetch_add(1, AtomicOrdering::Relaxed)
        );
        let (events, notifications) = EventQueue::new(
            self.config.outbound_event_capacity,
            self.config.replay_event_capacity,
            self.config.replay_byte_capacity,
            self.config.max_notification_bytes,
        );
        let connection = Arc::new(Connection {
            id: id.clone(),
            handshake: Mutex::new(HandshakeState::AwaitingInitialize),
            requests: Mutex::new(RequestRegistry::new(self.config.max_cached_requests)),
            in_flight: Arc::new(Semaphore::new(self.config.max_in_flight_requests.max(1))),
            events,
        });
        self.connections
            .lock()
            .expect("connections mutex poisoned")
            .insert(id.clone(), connection);
        OpenConnection {
            connection_id: id,
            notifications,
        }
    }

    pub fn disconnect(&self, connection_id: &str) -> bool {
        let removed = self
            .connections
            .lock()
            .expect("connections mutex poisoned")
            .remove(connection_id);
        if let Some(connection) = removed {
            connection
                .requests
                .lock()
                .expect("request registry mutex poisoned")
                .cancel_all();
            true
        } else {
            false
        }
    }

    /// Process one serialized frame. Requests return a serialized response;
    /// notifications intentionally return `None` per JSON-RPC 2.0.
    pub async fn handle_frame(&self, connection_id: &str, frame: &str) -> Option<String> {
        let message = match parse_client_message(frame, self.config.max_frame_bytes) {
            Ok(message) => message,
            Err(error) => return Some(serialize_response(JsonRpcResponse::failure(None, error))),
        };
        let connection = self.connection(connection_id);
        let Some(connection) = connection else {
            let id = match message {
                ClientMessage::Request(ref request) => Some(request.id.clone()),
                ClientMessage::Notification(_) => None,
            };
            return id.map(|id| {
                serialize_response(JsonRpcResponse::failure(
                    Some(id),
                    RpcError::new(
                        UNKNOWN_REQUEST_ID,
                        "UNKNOWN_CONNECTION_ID",
                        "The ForgeCAD app-server connection is unknown or closed.",
                        true,
                    ),
                ))
            });
        };
        match message {
            ClientMessage::Request(request) => Some(serialize_response(
                self.handle_request(connection, request).await,
            )),
            ClientMessage::Notification(notification) => {
                let _ = self.handle_notification(connection, notification);
                None
            }
        }
    }

    pub fn publish_notification(
        &self,
        connection_id: &str,
        method: impl Into<String>,
        cursor: AppServerCursor,
        params: Value,
    ) -> Result<forgecad_app_server_protocol::ServerNotification, RpcError> {
        self.connection(connection_id)
            .ok_or_else(unknown_connection_error)?
            .events
            .publish(method, cursor, params)
    }

    pub fn publish_transient_notification(
        &self,
        connection_id: &str,
        method: impl Into<String>,
        params: Value,
    ) -> Result<forgecad_app_server_protocol::ServerNotification, RpcError> {
        self.connection(connection_id)
            .ok_or_else(unknown_connection_error)?
            .events
            .publish_transient(method, params)
    }

    pub fn publish_resync_required(
        &self,
        connection_id: &str,
        reason: impl Into<String>,
    ) -> Result<forgecad_app_server_protocol::ServerNotification, RpcError> {
        self.connection(connection_id)
            .ok_or_else(unknown_connection_error)?
            .events
            .publish_resync_required(reason)
    }

    pub fn replay(&self, connection_id: &str, cursor: &str) -> Result<ReplayResult, RpcError> {
        self.connection(connection_id)
            .ok_or_else(unknown_connection_error)?
            .events
            .replay(cursor)
    }

    pub fn acknowledge(&self, connection_id: &str, params: AckParams) -> Result<(), RpcError> {
        self.connection(connection_id)
            .ok_or_else(unknown_connection_error)?
            .events
            .acknowledge(params)
    }

    pub fn cancel_request(
        &self,
        connection_id: &str,
        request_id: &str,
        cancel_token: &str,
    ) -> Result<(), RpcError> {
        let connection = self
            .connection(connection_id)
            .ok_or_else(unknown_connection_error)?;
        cancel_request_inner(&connection, request_id, cancel_token)
    }

    pub fn connection_is_ready(&self, connection_id: &str) -> bool {
        self.connection(connection_id)
            .is_some_and(|connection| connection.is_ready())
    }

    fn connection(&self, connection_id: &str) -> Option<Arc<Connection>> {
        self.connections
            .lock()
            .expect("connections mutex poisoned")
            .get(connection_id)
            .cloned()
    }

    async fn handle_request(
        &self,
        connection: Arc<Connection>,
        request: JsonRpcRequest,
    ) -> JsonRpcResponse {
        self.dispatch_registered_request(connection, request).await
    }

    fn initialize(&self, connection: &Connection, request: JsonRpcRequest) -> JsonRpcResponse {
        let mut handshake = connection
            .handshake
            .lock()
            .expect("handshake mutex poisoned");
        if *handshake != HandshakeState::AwaitingInitialize {
            return JsonRpcResponse::failure(
                Some(request.id),
                RpcError::new(
                    ALREADY_INITIALIZED,
                    "ALREADY_INITIALIZED",
                    "initialize may be called exactly once per connection.",
                    false,
                ),
            );
        }
        let result = serde_json::from_value::<InitializeParams>(request.params)
            .map_err(|error| {
                RpcError::invalid_params(format!("Invalid initialize params: {error}"))
            })
            .and_then(|params| {
                params.validate()?;
                let protocol_version =
                    select_protocol_version(&params.supported_protocol_versions)?;
                Ok(InitializeResult {
                    schema_version: INITIALIZE_RESULT_SCHEMA_VERSION.into(),
                    protocol_version: protocol_version.into(),
                    connection_id: connection.id.clone(),
                    server_info: ServerInfo {
                        name: "forgecad-rust-app-server".into(),
                        version: env!("CARGO_PKG_VERSION").into(),
                    },
                    capabilities: ProtocolCapabilities::REQUIRED,
                    limits: Some(ProtocolLimits {
                        max_in_flight_requests: Some(self.config.max_in_flight_requests as u32),
                        max_event_queue: Some(self.config.outbound_event_capacity as u32),
                        max_frame_bytes: Some(
                            self.config.max_frame_bytes.min(u32::MAX as usize) as u32
                        ),
                    }),
                    migration_state: Some(MigrationState {
                        state_owner: MigrationStateOwner::RustAppServer,
                    }),
                })
            })
            .and_then(|value| {
                serde_json::to_value(value).map_err(|error| {
                    RpcError::internal(format!("Initialize serialization failed: {error}"))
                })
            });
        if result.is_ok() {
            *handshake = HandshakeState::AwaitingInitialized {
                protocol_version: FORGECAD_PROTOCOL_VERSION.into(),
            };
        }
        response_from_result(request.id, result)
    }

    fn handle_notification(
        &self,
        connection: Arc<Connection>,
        notification: JsonRpcNotification,
    ) -> Result<(), RpcError> {
        if notification.method == METHOD_INITIALIZED {
            let params: InitializedParams = serde_json::from_value(notification.params)
                .map_err(|error| RpcError::invalid_params(error.to_string()))?;
            let mut handshake = connection
                .handshake
                .lock()
                .expect("handshake mutex poisoned");
            return match &*handshake {
                HandshakeState::AwaitingInitialized { protocol_version }
                    if protocol_version == &params.protocol_version =>
                {
                    *handshake = HandshakeState::Ready;
                    Ok(())
                }
                HandshakeState::Ready if params.protocol_version == FORGECAD_PROTOCOL_VERSION => {
                    Ok(())
                }
                HandshakeState::AwaitingInitialize => Err(RpcError::not_initialized()),
                _ => Err(RpcError::new(
                    forgecad_app_server_protocol::PROTOCOL_VERSION_UNSUPPORTED,
                    "PROTOCOL_VERSION_MISMATCH",
                    "initialized protocol_version does not match initialize result.",
                    false,
                )),
            };
        }
        if !connection.is_ready() {
            return Err(RpcError::not_initialized());
        }
        match notification.method.as_str() {
            METHOD_REQUEST_CANCEL => {
                let params: CancelParams = serde_json::from_value(notification.params)
                    .map_err(|error| RpcError::invalid_params(error.to_string()))?;
                cancel_request_inner(&connection, &params.request_id, &params.cancel_token)
            }
            METHOD_NOTIFICATION_ACK => {
                let params: AckParams = serde_json::from_value(notification.params)
                    .map_err(|error| RpcError::invalid_params(error.to_string()))?;
                connection.events.acknowledge(params)
            }
            _ => Ok(()),
        }
    }

    async fn dispatch_registered_request(
        &self,
        connection: Arc<Connection>,
        request: JsonRpcRequest,
    ) -> JsonRpcResponse {
        let cancellation = CancellationToken::new();
        let fingerprint = request_fingerprint(&request.method, &request.params);
        let begin = connection
            .requests
            .lock()
            .expect("request registry mutex poisoned")
            .begin(request.id.clone(), fingerprint, cancellation.clone());
        match begin {
            Err(error) => JsonRpcResponse::failure(Some(request.id), error),
            Ok(BeginRequest::Replay(response)) => response,
            Ok(BeginRequest::New) => {
                let request_id = request.id.clone();
                let permit = match Arc::clone(&connection.in_flight).try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        connection
                            .requests
                            .lock()
                            .expect("request registry mutex poisoned")
                            .forget(&request_id);
                        return JsonRpcResponse::failure(
                            Some(request_id),
                            overloaded_error("The bounded in-flight request queue is full."),
                        );
                    }
                };
                let response = self
                    .run_new_request(Arc::clone(&connection), request, cancellation, permit)
                    .await;
                connection
                    .requests
                    .lock()
                    .expect("request registry mutex poisoned")
                    .complete(&request_id, response.clone());
                response
            }
        }
    }

    async fn run_new_request(
        &self,
        connection: Arc<Connection>,
        request: JsonRpcRequest,
        cancellation: CancellationToken,
        _permit: OwnedSemaphorePermit,
    ) -> JsonRpcResponse {
        if request.method == METHOD_INITIALIZE {
            return self.initialize(&connection, request);
        }
        if !connection.is_ready() {
            return JsonRpcResponse::failure(Some(request.id), RpcError::not_initialized());
        }
        match request.method.as_str() {
            METHOD_REQUEST_CANCEL => {
                let own_id = request.id.clone();
                let result = serde_json::from_value::<CancelParams>(request.params)
                    .map_err(|error| RpcError::invalid_params(error.to_string()))
                    .and_then(|params| {
                        if params.request_id == own_id.0 {
                            return Err(RpcError::invalid_params(
                                "request/cancel cannot target its own request ID.",
                            ));
                        }
                        cancel_request_inner(&connection, &params.request_id, &params.cancel_token)
                    })
                    .map(|_| json!({"cancelled": true}));
                return response_from_result(request.id, result);
            }
            METHOD_NOTIFICATION_ACK => {
                let result = serde_json::from_value::<AckParams>(request.params)
                    .map_err(|error| RpcError::invalid_params(error.to_string()))
                    .and_then(|params| connection.events.acknowledge(params))
                    .map(|_| json!({"acknowledged": true}));
                return response_from_result(request.id, result);
            }
            METHOD_EVENTS_REPLAY => {
                let params = request.params.clone();
                let result = serde_json::from_value::<ReplayParams>(params.clone())
                    .map_err(|error| RpcError::invalid_params(error.to_string()))
                    .and_then(|params| connection.events.replay(&params.cursor))
                    .and_then(|value| {
                        serde_json::to_value(value).map_err(|error| {
                            RpcError::internal(format!("Replay serialization failed: {error}"))
                        })
                    });
                if !result
                    .as_ref()
                    .is_err_and(|error| error.code == CURSOR_RESYNC_REQUIRED)
                {
                    return response_from_result(request.id, result);
                }
                let future = self.handler.handle(
                    METHOD_EVENTS_REPLAY.to_string(),
                    params,
                    cancellation.clone(),
                );
                let fallback = cancellation_aware(future, cancellation, &request.id.0).await;
                return response_from_result(request.id, fallback);
            }
            _ => {}
        }
        let method = request.method.clone();
        let params = request.params.clone();
        let request_id = request.id.clone();
        let future = self.handler.handle(method, params, cancellation.clone());
        let result = cancellation_aware(future, cancellation, &request_id.0).await;
        response_from_result(request.id, result)
    }
}

async fn cancellation_aware(
    mut handler: crate::HandlerFuture,
    cancellation: CancellationToken,
    request_id: &str,
) -> Result<Value, RpcError> {
    let mut cancelled = Box::pin(cancellation.clone().cancelled_owned());
    let result = poll_fn(|context| {
        if Pin::new(&mut cancelled).poll(context).is_ready() {
            return Poll::Ready(Err(cancelled_error(request_id)));
        }
        handler.as_mut().poll(context)
    })
    .await;
    if cancellation.is_cancelled() {
        Err(cancelled_error(request_id))
    } else {
        result
    }
}

impl Connection {
    fn is_ready(&self) -> bool {
        *self.handshake.lock().expect("handshake mutex poisoned") == HandshakeState::Ready
    }
}

impl RequestRegistry {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries: max_entries.max(1),
        }
    }

    fn begin(
        &mut self,
        id: RequestId,
        fingerprint: String,
        cancellation: CancellationToken,
    ) -> Result<BeginRequest, RpcError> {
        if let Some(existing) = self.entries.get(&id) {
            if existing.fingerprint != fingerprint {
                return Err(RpcError::new(
                    DUPLICATE_REQUEST_ID,
                    "DUPLICATE_REQUEST_ID",
                    "The request ID was already used with a different method or params.",
                    false,
                )
                .with_request_id(id.0));
            }
            return existing
                .response
                .clone()
                .map(BeginRequest::Replay)
                .ok_or_else(|| {
                    let mut error = RpcError::new(
                        DUPLICATE_REQUEST_ID,
                        "REQUEST_IN_FLIGHT",
                        "The identical request is already in flight.",
                        true,
                    )
                    .with_request_id(id.0.clone());
                    error.data.retry_after_ms = Some(10);
                    error
                });
        }
        while self.entries.len() >= self.max_entries {
            let completed = self.order.iter().position(|candidate| {
                self.entries
                    .get(candidate)
                    .is_some_and(|entry| entry.response.is_some())
            });
            let Some(index) = completed else {
                return Err(overloaded_error("The bounded request cache is full."));
            };
            if let Some(evicted) = self.order.remove(index) {
                self.entries.remove(&evicted);
            }
        }
        self.order.push_back(id.clone());
        self.entries.insert(
            id,
            RequestEntry {
                fingerprint,
                cancellation,
                response: None,
            },
        );
        Ok(BeginRequest::New)
    }

    fn complete(&mut self, id: &RequestId, response: JsonRpcResponse) {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.response = Some(response);
        }
    }

    fn forget(&mut self, id: &RequestId) {
        self.entries.remove(id);
        self.order.retain(|candidate| candidate != id);
    }

    fn cancel(&self, id: &RequestId) -> Result<(), RpcError> {
        match self.entries.get(id) {
            Some(entry) if entry.response.is_none() => {
                entry.cancellation.cancel();
                Ok(())
            }
            _ => Err(RpcError::new(
                UNKNOWN_REQUEST_ID,
                "UNKNOWN_REQUEST_ID",
                "The request is unknown or no longer in flight.",
                true,
            )
            .with_request_id(id.0.clone())),
        }
    }

    fn cancel_all(&self) {
        for entry in self.entries.values() {
            if entry.response.is_none() {
                entry.cancellation.cancel();
            }
        }
    }
}

fn cancel_request_inner(
    connection: &Connection,
    request_id: &str,
    cancel_token: &str,
) -> Result<(), RpcError> {
    if request_id != cancel_token {
        return Err(RpcError::invalid_params(
            "cancel_token must equal request_id for protocol v1.",
        ));
    }
    connection
        .requests
        .lock()
        .expect("request registry mutex poisoned")
        .cancel(&RequestId(request_id.to_string()))
}

fn request_fingerprint(method: &str, params: &Value) -> String {
    let canonical = canonical_json(&json!({"method": method, "params": params}));
    sha256_hex(canonical.as_bytes())
}

fn response_from_result(id: RequestId, result: Result<Value, RpcError>) -> JsonRpcResponse {
    match result {
        Ok(value) => JsonRpcResponse::success(id, value),
        Err(error) => JsonRpcResponse::failure(Some(id), error),
    }
}

fn serialize_response(response: JsonRpcResponse) -> String {
    serde_json::to_string(&response).expect("JSON-RPC response serialization cannot fail")
}

fn cancelled_error(request_id: &str) -> RpcError {
    RpcError::new(
        REQUEST_CANCELLED,
        "REQUEST_CANCELLED",
        "Request cancelled.",
        true,
    )
    .with_request_id(request_id)
}

fn overloaded_error(message: &str) -> RpcError {
    let mut error = RpcError::new(SERVER_OVERLOADED, "SERVER_OVERLOADED", message, true);
    error.data.retry_after_ms = Some(25);
    error
}

fn unknown_connection_error() -> RpcError {
    RpcError::new(
        UNKNOWN_REQUEST_ID,
        "UNKNOWN_CONNECTION_ID",
        "The ForgeCAD app-server connection is unknown or closed.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use forgecad_app_server_protocol::{
        ClientInfo, ClientTransport, CursorPhase, INITIALIZE_PARAMS_SCHEMA_VERSION, NOT_INITIALIZED,
    };
    use tokio::sync::Notify;

    use super::*;
    use crate::HandlerFuture;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    struct EchoHandler {
        calls: AtomicUsize,
    }

    impl RequestHandler for EchoHandler {
        fn handle(
            &self,
            method: String,
            params: Value,
            _cancellation: CancellationToken,
        ) -> HandlerFuture {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move { Ok(json!({"method": method, "params": params})) })
        }
    }

    struct BlockingHandler {
        started: Arc<Notify>,
    }

    struct ReplayFallbackHandler {
        calls: AtomicUsize,
    }

    impl RequestHandler for ReplayFallbackHandler {
        fn handle(
            &self,
            method: String,
            _params: Value,
            _cancellation: CancellationToken,
        ) -> HandlerFuture {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if method != METHOD_EVENTS_REPLAY {
                    return Err(RpcError::method_not_found(&method));
                }
                Ok(json!({"notifications": []}))
            })
        }
    }

    impl RequestHandler for BlockingHandler {
        fn handle(
            &self,
            _method: String,
            _params: Value,
            cancellation: CancellationToken,
        ) -> HandlerFuture {
            let started = Arc::clone(&self.started);
            Box::pin(async move {
                started.notify_one();
                cancellation.cancelled().await;
                Ok(json!({"too_late": true}))
            })
        }
    }

    fn server(handler: Arc<dyn RequestHandler>) -> Arc<AppServer> {
        Arc::new(AppServer::new(handler, AppServerConfig::default()))
    }

    fn initialize_request(id: &str) -> String {
        let params = InitializeParams {
            schema_version: INITIALIZE_PARAMS_SCHEMA_VERSION.into(),
            supported_protocol_versions: vec![FORGECAD_PROTOCOL_VERSION.into()],
            client_info: ClientInfo {
                name: "forgecad-desktop".into(),
                version: "0.1.0".into(),
                transport: ClientTransport::Tauri,
            },
            capabilities: ProtocolCapabilities::REQUIRED,
        };
        serde_json::to_string(&JsonRpcRequest::new(
            RequestId(id.into()),
            METHOD_INITIALIZE,
            serde_json::to_value(params).unwrap(),
        ))
        .unwrap()
    }

    async fn make_ready(server: &AppServer, connection_id: &str) {
        let response = server
            .handle_frame(connection_id, &initialize_request("req_initialize"))
            .await
            .unwrap();
        let response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
        assert!(response.error.is_none());
        let initialized = serde_json::to_string(&JsonRpcNotification::new(
            METHOD_INITIALIZED,
            json!({"protocol_version": FORGECAD_PROTOCOL_VERSION}),
        ))
        .unwrap();
        assert!(server
            .handle_frame(connection_id, &initialized)
            .await
            .is_none());
    }

    fn request(id: &str, method: &str, params: Value) -> String {
        serde_json::to_string(&JsonRpcRequest::new(RequestId(id.into()), method, params)).unwrap()
    }

    #[test]
    fn rejects_before_initialize_and_requires_initialized_notification() {
        runtime().block_on(async {
            let server = server(Arc::new(EchoHandler {
                calls: AtomicUsize::new(0),
            }));
            let connection = server.open_connection();
            let response = server
                .handle_frame(
                    &connection.connection_id,
                    &request("req_1", "thread/list", json!({})),
                )
                .await
                .unwrap();
            let response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
            assert_eq!(response.error.unwrap().code, NOT_INITIALIZED);

            let initialize = server
                .handle_frame(&connection.connection_id, &initialize_request("req_init"))
                .await
                .unwrap();
            assert!(serde_json::from_str::<JsonRpcResponse>(&initialize)
                .unwrap()
                .error
                .is_none());
            let response = server
                .handle_frame(
                    &connection.connection_id,
                    &request("req_2", "thread/list", json!({})),
                )
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_str::<JsonRpcResponse>(&response)
                    .unwrap()
                    .error
                    .unwrap()
                    .code,
                NOT_INITIALIZED
            );
        });
    }

    #[test]
    fn initialize_and_protocol_requests_share_duplicate_id_semantics() {
        runtime().block_on(async {
            let server = server(Arc::new(EchoHandler {
                calls: AtomicUsize::new(0),
            }));
            let connection = server.open_connection();
            let initialize = initialize_request("req_initialize_stable");
            let first = server
                .handle_frame(&connection.connection_id, &initialize)
                .await
                .unwrap();
            let replay = server
                .handle_frame(&connection.connection_id, &initialize)
                .await
                .unwrap();
            assert_eq!(first, replay);
            let initialized = serde_json::to_string(&JsonRpcNotification::new(
                METHOD_INITIALIZED,
                json!({"protocol_version": FORGECAD_PROTOCOL_VERSION}),
            ))
            .unwrap();
            assert!(server
                .handle_frame(&connection.connection_id, &initialized)
                .await
                .is_none());

            let invalid_replay = request(
                "req_protocol_stable",
                METHOD_EVENTS_REPLAY,
                json!({"cursor": "invalid"}),
            );
            let first = server
                .handle_frame(&connection.connection_id, &invalid_replay)
                .await
                .unwrap();
            let replay = server
                .handle_frame(&connection.connection_id, &invalid_replay)
                .await
                .unwrap();
            assert_eq!(first, replay);
            let conflict = server
                .handle_frame(
                    &connection.connection_id,
                    &request(
                        "req_protocol_stable",
                        METHOD_EVENTS_REPLAY,
                        json!({"cursor": "different"}),
                    ),
                )
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_str::<JsonRpcResponse>(&conflict)
                    .unwrap()
                    .error
                    .unwrap()
                    .code,
                DUPLICATE_REQUEST_ID
            );
        });
    }

    #[test]
    fn replay_falls_back_to_compatibility_source_after_process_restart() {
        runtime().block_on(async {
            let handler = Arc::new(ReplayFallbackHandler {
                calls: AtomicUsize::new(0),
            });
            let server = server(handler.clone());
            let connection = server.open_connection();
            make_ready(&server, &connection.connection_id).await;
            let cursor = AppServerCursor::new(
                "thread_persisted",
                Some("turn_persisted".into()),
                8,
                CursorPhase::Item,
                Some("item_8".into()),
            )
            .encode()
            .unwrap();
            let response = server
                .handle_frame(
                    &connection.connection_id,
                    &request(
                        "req_restart_replay",
                        METHOD_EVENTS_REPLAY,
                        json!({"cursor": cursor}),
                    ),
                )
                .await
                .unwrap();
            let response: JsonRpcResponse = serde_json::from_str(&response).unwrap();
            assert_eq!(response.result.unwrap()["notifications"], json!([]));
            assert_eq!(handler.calls.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn identical_request_replays_response_and_conflicting_id_is_rejected() {
        runtime().block_on(async {
            let handler = Arc::new(EchoHandler {
                calls: AtomicUsize::new(0),
            });
            let server = server(handler.clone());
            let connection = server.open_connection();
            make_ready(&server, &connection.connection_id).await;
            let frame = request("req_same", "thread/read", json!({"thread_id": "thread_1"}));
            let first = server
                .handle_frame(&connection.connection_id, &frame)
                .await
                .unwrap();
            let second = server
                .handle_frame(&connection.connection_id, &frame)
                .await
                .unwrap();
            assert_eq!(first, second);
            assert_eq!(handler.calls.load(Ordering::SeqCst), 1);
            let conflict = server
                .handle_frame(
                    &connection.connection_id,
                    &request("req_same", "thread/read", json!({"thread_id": "thread_2"})),
                )
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_str::<JsonRpcResponse>(&conflict)
                    .unwrap()
                    .error
                    .unwrap()
                    .code,
                DUPLICATE_REQUEST_ID
            );
        });
    }

    #[test]
    fn cancellation_wins_the_completion_race_and_unknown_id_is_stable() {
        runtime().block_on(async {
            let started = Arc::new(Notify::new());
            let server = server(Arc::new(BlockingHandler {
                started: started.clone(),
            }));
            let connection = server.open_connection();
            make_ready(&server, &connection.connection_id).await;
            let server_task = Arc::clone(&server);
            let connection_id = connection.connection_id.clone();
            let task_connection_id = connection_id.clone();
            let task = tokio::spawn(async move {
                server_task
                    .handle_frame(
                        &task_connection_id,
                        &request("req_block", "turn/start", json!({})),
                    )
                    .await
                    .unwrap()
            });
            started.notified().await;
            server
                .cancel_request(&connection_id, "req_block", "req_block")
                .unwrap();
            let response: JsonRpcResponse = serde_json::from_str(&task.await.unwrap()).unwrap();
            assert_eq!(response.error.unwrap().code, REQUEST_CANCELLED);
            assert_eq!(
                server
                    .cancel_request(&connection_id, "req_missing", "req_missing")
                    .unwrap_err()
                    .code,
                UNKNOWN_REQUEST_ID
            );
        });
    }

    #[test]
    fn bounded_in_flight_queue_rejects_a_second_request() {
        runtime().block_on(async {
            let started = Arc::new(Notify::new());
            let mut config = AppServerConfig::default();
            config.max_in_flight_requests = 1;
            let server = Arc::new(AppServer::new(
                Arc::new(BlockingHandler {
                    started: started.clone(),
                }),
                config,
            ));
            let connection = server.open_connection();
            make_ready(&server, &connection.connection_id).await;
            let first_server = Arc::clone(&server);
            let connection_id = connection.connection_id.clone();
            let first_connection_id = connection_id.clone();
            let first = tokio::spawn(async move {
                first_server
                    .handle_frame(
                        &first_connection_id,
                        &request("req_1", "turn/start", json!({})),
                    )
                    .await
            });
            started.notified().await;
            let duplicate = server
                .handle_frame(&connection_id, &request("req_1", "turn/start", json!({})))
                .await
                .unwrap();
            let duplicate: JsonRpcResponse = serde_json::from_str(&duplicate).unwrap();
            assert_eq!(
                duplicate.error.unwrap().data.application_code,
                "REQUEST_IN_FLIGHT"
            );
            let second = server
                .handle_frame(&connection_id, &request("req_2", "thread/list", json!({})))
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_str::<JsonRpcResponse>(&second)
                    .unwrap()
                    .error
                    .unwrap()
                    .code,
                SERVER_OVERLOADED
            );
            server
                .cancel_request(&connection_id, "req_1", "req_1")
                .unwrap();
            first.await.unwrap();

            let retry_server = Arc::clone(&server);
            let retry_connection_id = connection_id.clone();
            let retry = tokio::spawn(async move {
                retry_server
                    .handle_frame(
                        &retry_connection_id,
                        &request("req_2", "thread/list", json!({})),
                    )
                    .await
            });
            started.notified().await;
            server
                .cancel_request(&connection_id, "req_2", "req_2")
                .unwrap();
            let retry = retry.await.unwrap().unwrap();
            assert_eq!(
                serde_json::from_str::<JsonRpcResponse>(&retry)
                    .unwrap()
                    .error
                    .unwrap()
                    .code,
                REQUEST_CANCELLED
            );
        });
    }

    #[test]
    fn malformed_frame_and_reconnect_are_connection_scoped() {
        runtime().block_on(async {
            let server = server(Arc::new(EchoHandler {
                calls: AtomicUsize::new(0),
            }));
            let first = server.open_connection();
            make_ready(&server, &first.connection_id).await;
            assert!(server.disconnect(&first.connection_id));
            let second = server.open_connection();
            let malformed = server
                .handle_frame(&second.connection_id, "{not json")
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_str::<JsonRpcResponse>(&malformed)
                    .unwrap()
                    .error
                    .unwrap()
                    .code,
                forgecad_app_server_protocol::PARSE_ERROR
            );
            let response = server
                .handle_frame(
                    &second.connection_id,
                    &request("req_after_reconnect", "thread/list", json!({})),
                )
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_str::<JsonRpcResponse>(&response)
                    .unwrap()
                    .error
                    .unwrap()
                    .code,
                NOT_INITIALIZED
            );
        });
    }

    #[test]
    fn published_notification_uses_top_level_delivery_metadata() {
        runtime().block_on(async {
            let server = server(Arc::new(EchoHandler {
                calls: AtomicUsize::new(0),
            }));
            let mut connection = server.open_connection();
            make_ready(&server, &connection.connection_id).await;
            server
                .publish_notification(
                    &connection.connection_id,
                    "item/completed",
                    AppServerCursor::new(
                        "thread_1",
                        Some("turn_1".into()),
                        1,
                        CursorPhase::Item,
                        Some("item_1".into()),
                    ),
                    json!({"item_id": "item_1"}),
                )
                .unwrap();
            let frame = connection.notifications.recv().await.unwrap();
            let value: Value = serde_json::from_str(&frame).unwrap();
            assert!(value.get("notification_id").is_some());
            assert!(value.get("cursor").is_some());
            assert_eq!(value["params"]["item_id"], "item_1");
        });
    }

    #[test]
    fn transient_notification_preserves_compatibility_event_without_cursor() {
        runtime().block_on(async {
            let server = server(Arc::new(EchoHandler {
                calls: AtomicUsize::new(0),
            }));
            let mut connection = server.open_connection();
            make_ready(&server, &connection.connection_id).await;
            server
                .publish_transient_notification(
                    &connection.connection_id,
                    "compat/sse",
                    json!({"event": "agent.replay.complete"}),
                )
                .unwrap();
            let frame = connection.notifications.recv().await.unwrap();
            let value: Value = serde_json::from_str(&frame).unwrap();
            assert!(value.get("notification_id").is_none());
            assert!(value.get("cursor").is_none());
            assert_eq!(value["params"]["event"], "agent.replay.complete");
        });
    }
}
