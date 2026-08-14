use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{CasError, Runtime, RuntimeError, StoreError};

const MAX_IPC_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
const IPC_AUTHENTICATION_TIMEOUT: Duration = Duration::from_millis(500);
// Codex tools have a 60 second outer timeout. Keep local transport bounded
// below that ceiling while leaving ample room for the 10 second geometry
// worker budget and response serialization.
const IPC_REQUEST_TIMEOUT: Duration = Duration::from_secs(55);

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("authenticated local IPC is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("local IPC I/O failure")]
    Io(#[from] std::io::Error),
    #[error("local IPC authentication failed")]
    AuthenticationFailed,
    #[error("local IPC protocol error")]
    Protocol,
    #[error("local IPC runtime request failed: {0}")]
    RuntimeRequest(String),
    #[error("local IPC server shutdown requested")]
    ShutdownRequested,
}

#[derive(Debug, Clone)]
pub struct LocalIpcEndpoint {
    socket_path: PathBuf,
    token: String,
}

impl LocalIpcEndpoint {
    pub fn new(directory: impl AsRef<Path>) -> Result<Self, IpcError> {
        let directory = directory.as_ref();
        std::fs::create_dir_all(directory)?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let socket_path = directory.join(format!(
            "fc-{}.sock",
            &uuid::Uuid::new_v4().simple().to_string()[..16]
        ));
        if socket_path.to_string_lossy().len() > 100 {
            return Err(IpcError::Protocol);
        }
        Ok(Self { socket_path, token })
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Rehydrates an endpoint handed from a signed Runtime launcher. The
    /// token stays in memory and is never serialised into a Runtime contract.
    pub fn from_parts(socket_path: impl Into<PathBuf>, token: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            token: token.into(),
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Checks whether an OS listener still owns the handed-off socket. This
    /// does not authenticate or authorize a Runtime call; supervisors use it
    /// only to avoid deleting a reachable endpoint while it is busy.
    pub fn listener_reachable(&self) -> bool {
        platform::listener_reachable(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IpcRequest {
    version: u16,
    token: Option<String>,
    method: String,
    payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IpcResponse {
    version: u16,
    ok: bool,
    code: Option<String>,
    payload: Option<Value>,
}

#[cfg(unix)]
mod platform {
    use super::*;
    use serde::de::DeserializeOwned;
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};

    pub struct LocalIpcServer {
        listener: UnixListener,
        socket_path: PathBuf,
        token_hash: String,
    }

    enum ClientConnection {
        Closed,
        ShutdownRequested,
    }

    impl LocalIpcServer {
        fn bind_internal(endpoint: &LocalIpcEndpoint) -> Result<Self, IpcError> {
            if endpoint.socket_path.exists() {
                let metadata = fs::symlink_metadata(&endpoint.socket_path)?;
                if !metadata.file_type().is_socket() {
                    return Err(IpcError::Protocol);
                }
                fs::remove_file(&endpoint.socket_path)?;
            }
            let listener = UnixListener::bind(&endpoint.socket_path)?;
            fs::set_permissions(&endpoint.socket_path, fs::Permissions::from_mode(0o600))?;
            Ok(Self {
                listener,
                socket_path: endpoint.socket_path.clone(),
                token_hash: hash_token(&endpoint.token),
            })
        }

        pub fn serve_once(&self, runtime: &Runtime) -> Result<(), IpcError> {
            let (stream, _) = self.listener.accept()?;
            match serve_stream(stream, runtime, &self.token_hash)? {
                ClientConnection::Closed => Ok(()),
                ClientConnection::ShutdownRequested => Err(IpcError::ShutdownRequested),
            }
        }
    }

    impl Drop for LocalIpcServer {
        fn drop(&mut self) {
            if let Ok(metadata) = fs::symlink_metadata(&self.socket_path) {
                if metadata.file_type().is_socket() {
                    let _ = fs::remove_file(&self.socket_path);
                }
            }
        }
    }

    pub struct LocalIpcClient {
        reader: BufReader<UnixStream>,
        writer: UnixStream,
        request_timeout: Duration,
        max_message_bytes: usize,
    }

    impl LocalIpcClient {
        fn connect_internal(endpoint: &LocalIpcEndpoint) -> Result<Self, IpcError> {
            Self::connect_internal_with_limits(
                endpoint,
                IPC_AUTHENTICATION_TIMEOUT,
                IPC_REQUEST_TIMEOUT,
                MAX_IPC_MESSAGE_BYTES,
            )
        }

        fn connect_internal_with_limits(
            endpoint: &LocalIpcEndpoint,
            authentication_timeout: Duration,
            request_timeout: Duration,
            max_message_bytes: usize,
        ) -> Result<Self, IpcError> {
            let stream = UnixStream::connect(&endpoint.socket_path)?;
            stream.set_read_timeout(Some(authentication_timeout))?;
            stream.set_write_timeout(Some(authentication_timeout))?;
            let reader_stream = stream.try_clone()?;
            let mut client = Self {
                reader: BufReader::new(reader_stream),
                writer: stream,
                request_timeout,
                max_message_bytes,
            };
            let authentication_deadline = deadline_after(authentication_timeout);
            client.send(
                IpcRequest {
                    version: 1,
                    token: Some(endpoint.token.clone()),
                    method: "authenticate".to_owned(),
                    payload: Value::Null,
                },
                authentication_deadline,
            )?;
            let response = client.receive(authentication_deadline)?;
            if !response.ok {
                return Err(IpcError::AuthenticationFailed);
            }
            client
                .reader
                .get_ref()
                .set_read_timeout(Some(request_timeout))?;
            client.writer.set_write_timeout(Some(request_timeout))?;
            Ok(client)
        }

        pub fn call(&mut self, method: &str, payload: Value) -> Result<Value, IpcError> {
            let request_deadline = deadline_after(self.request_timeout);
            self.send(
                IpcRequest {
                    version: 1,
                    token: None,
                    method: method.to_owned(),
                    payload,
                },
                request_deadline,
            )?;
            let response = self.receive(request_deadline)?;
            if !response.ok {
                return Err(IpcError::RuntimeRequest(
                    response
                        .code
                        .unwrap_or_else(|| "RUNTIME_REQUEST_FAILED".to_owned()),
                ));
            }
            response.payload.ok_or(IpcError::Protocol)
        }

        #[cfg(test)]
        pub(crate) fn configured_timeouts(
            &self,
        ) -> Result<(Option<Duration>, Option<Duration>), IpcError> {
            Ok((
                self.reader.get_ref().read_timeout()?,
                self.writer.write_timeout()?,
            ))
        }

        fn send(&mut self, request: IpcRequest, deadline: Instant) -> Result<(), IpcError> {
            let bytes = serde_json::to_vec(&request).map_err(|_| IpcError::Protocol)?;
            if bytes.len().saturating_add(1) >= self.max_message_bytes {
                return Err(IpcError::Protocol);
            }
            write_json_line(&mut self.writer, &bytes, deadline)
        }

        fn receive(&mut self, deadline: Instant) -> Result<IpcResponse, IpcError> {
            receive_json_line(&mut self.reader, deadline, self.max_message_bytes)?
                .ok_or(IpcError::Protocol)
        }
    }

    fn serve_stream(
        stream: UnixStream,
        runtime: &Runtime,
        token_hash: &str,
    ) -> Result<ClientConnection, IpcError> {
        serve_stream_with_limits(
            stream,
            runtime,
            token_hash,
            IPC_AUTHENTICATION_TIMEOUT,
            IPC_REQUEST_TIMEOUT,
            MAX_IPC_MESSAGE_BYTES,
        )
    }

    fn serve_stream_with_limits(
        stream: UnixStream,
        runtime: &Runtime,
        token_hash: &str,
        authentication_timeout: Duration,
        request_timeout: Duration,
        max_message_bytes: usize,
    ) -> Result<ClientConnection, IpcError> {
        stream.set_read_timeout(Some(authentication_timeout))?;
        stream.set_write_timeout(Some(authentication_timeout))?;
        let reader_stream = stream.try_clone()?;
        let mut reader = BufReader::new(reader_stream);
        let mut writer = stream;
        let authentication_deadline = deadline_after(authentication_timeout);
        let Some(first) = receive_json_line::<IpcRequest>(
            &mut reader,
            authentication_deadline,
            max_message_bytes,
        )?
        else {
            return Err(IpcError::AuthenticationFailed);
        };
        if first.version != 1
            || first.method != "authenticate"
            || first
                .token
                .as_deref()
                .is_none_or(|token| !constant_time_equal(&hash_token(token), token_hash))
        {
            send(
                &mut writer,
                &IpcResponse {
                    version: 1,
                    ok: false,
                    code: Some("AUTH_FAILED".to_owned()),
                    payload: None,
                },
                authentication_deadline,
                max_message_bytes,
            )?;
            return Err(IpcError::AuthenticationFailed);
        }
        send(
            &mut writer,
            &IpcResponse {
                version: 1,
                ok: true,
                code: None,
                payload: Some(json!({"authenticated":true})),
            },
            authentication_deadline,
            max_message_bytes,
        )?;
        // Authentication has a short probe timeout. Authenticated requests
        // receive the larger bounded tool-call window instead of being able
        // to occupy Runtime's sequential connection forever.
        reader.get_ref().set_read_timeout(Some(request_timeout))?;
        writer.set_write_timeout(Some(request_timeout))?;

        loop {
            let request_deadline = deadline_after(request_timeout);
            let Some(request) =
                receive_json_line::<IpcRequest>(&mut reader, request_deadline, max_message_bytes)?
            else {
                return Ok(ClientConnection::Closed);
            };
            if request.version != 1 || request.token.is_some() || request.method == "authenticate" {
                send(
                    &mut writer,
                    &IpcResponse {
                        version: 1,
                        ok: false,
                        code: Some("PROTOCOL_ERROR".to_owned()),
                        payload: None,
                    },
                    request_deadline,
                    max_message_bytes,
                )?;
                continue;
            }
            if request.method == "runtime_shutdown" {
                send(
                    &mut writer,
                    &IpcResponse {
                        version: 1,
                        ok: true,
                        code: None,
                        payload: Some(json!({"shutting_down":true})),
                    },
                    request_deadline,
                    max_message_bytes,
                )?;
                return Ok(ClientConnection::ShutdownRequested);
            }
            match runtime.dispatch_ipc(&request.method, &request.payload) {
                Ok(payload) => send(
                    &mut writer,
                    &IpcResponse {
                        version: 1,
                        ok: true,
                        code: None,
                        payload: Some(payload),
                    },
                    request_deadline,
                    max_message_bytes,
                )?,
                Err(error) => send(
                    &mut writer,
                    &IpcResponse {
                        version: 1,
                        ok: false,
                        code: Some(runtime_error_code(&error)),
                        payload: None,
                    },
                    request_deadline,
                    max_message_bytes,
                )?,
            }
        }
    }

    fn send(
        writer: &mut UnixStream,
        response: &IpcResponse,
        deadline: Instant,
        max_message_bytes: usize,
    ) -> Result<(), IpcError> {
        let bytes = serde_json::to_vec(response).map_err(|_| IpcError::Protocol)?;
        if bytes.len().saturating_add(1) >= max_message_bytes {
            return Err(IpcError::Protocol);
        }
        write_json_line(writer, &bytes, deadline)
    }

    fn receive_json_line<T: DeserializeOwned>(
        reader: &mut BufReader<UnixStream>,
        deadline: Instant,
        max_message_bytes: usize,
    ) -> Result<Option<T>, IpcError> {
        let mut bytes = Vec::with_capacity(max_message_bytes.min(8 * 1024));
        loop {
            reader
                .get_ref()
                .set_read_timeout(Some(remaining_timeout(deadline)?))?;
            let available = reader.fill_buf()?;
            if available.is_empty() {
                return if bytes.is_empty() {
                    Ok(None)
                } else {
                    Err(IpcError::Protocol)
                };
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let take = newline.map_or(available.len(), |index| index + 1);
            if bytes.len().saturating_add(take) >= max_message_bytes {
                return Err(IpcError::Protocol);
            }
            bytes.extend_from_slice(&available[..take]);
            reader.consume(take);
            if newline.is_some() {
                return serde_json::from_slice(&bytes)
                    .map(Some)
                    .map_err(|_| IpcError::Protocol);
            }
        }
    }

    fn write_json_line(
        writer: &mut UnixStream,
        bytes: &[u8],
        deadline: Instant,
    ) -> Result<(), IpcError> {
        for chunk in [bytes, b"\n".as_slice()] {
            let mut written = 0;
            while written < chunk.len() {
                writer.set_write_timeout(Some(remaining_timeout(deadline)?))?;
                let count = writer.write(&chunk[written..])?;
                if count == 0 {
                    return Err(IpcError::Io(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "local IPC write made no progress",
                    )));
                }
                written += count;
            }
        }
        writer.set_write_timeout(Some(remaining_timeout(deadline)?))?;
        writer.flush()?;
        Ok(())
    }

    fn deadline_after(timeout: Duration) -> Instant {
        Instant::now()
            .checked_add(timeout)
            .unwrap_or_else(Instant::now)
    }

    fn remaining_timeout(deadline: Instant) -> Result<Duration, IpcError> {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| {
                IpcError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "local IPC absolute deadline exceeded",
                ))
            })
    }

    pub fn new_client(endpoint: &LocalIpcEndpoint) -> Result<LocalIpcClient, IpcError> {
        LocalIpcClient::connect_internal(endpoint)
    }

    #[cfg(test)]
    pub(crate) fn new_client_with_limits_for_test(
        endpoint: &LocalIpcEndpoint,
        authentication_timeout: Duration,
        request_timeout: Duration,
        max_message_bytes: usize,
    ) -> Result<LocalIpcClient, IpcError> {
        LocalIpcClient::connect_internal_with_limits(
            endpoint,
            authentication_timeout,
            request_timeout,
            max_message_bytes,
        )
    }

    pub fn new_server(endpoint: &LocalIpcEndpoint) -> Result<LocalIpcServer, IpcError> {
        LocalIpcServer::bind_internal(endpoint)
    }

    pub fn serve_forever(server: &LocalIpcServer, runtime: &Runtime) -> Result<(), IpcError> {
        serve_forever_with_limits(
            server,
            runtime,
            IPC_AUTHENTICATION_TIMEOUT,
            IPC_REQUEST_TIMEOUT,
            MAX_IPC_MESSAGE_BYTES,
        )
    }

    fn serve_forever_with_limits(
        server: &LocalIpcServer,
        runtime: &Runtime,
        authentication_timeout: Duration,
        request_timeout: Duration,
        max_message_bytes: usize,
    ) -> Result<(), IpcError> {
        loop {
            // Listener/accept failures are server failures and remain fatal.
            let (stream, _) = server.listener.accept()?;
            match serve_stream_with_limits(
                stream,
                runtime,
                &server.token_hash,
                authentication_timeout,
                request_timeout,
                max_message_bytes,
            ) {
                Ok(ClientConnection::Closed) => {}
                Ok(ClientConnection::ShutdownRequested) => return Ok(()),
                Err(IpcError::AuthenticationFailed | IpcError::Protocol | IpcError::Io(_)) => {
                    // Pre-auth idle peers, malformed JSON, disconnects and
                    // BrokenPipe are scoped to this accepted connection.
                }
                Err(error) => return Err(error),
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn serve_forever_for_test(
        server: &LocalIpcServer,
        runtime: &Runtime,
        request_timeout: Duration,
    ) -> Result<(), IpcError> {
        serve_forever_with_limits(
            server,
            runtime,
            IPC_AUTHENTICATION_TIMEOUT,
            request_timeout,
            MAX_IPC_MESSAGE_BYTES,
        )
    }

    #[cfg(test)]
    pub(crate) fn serve_forever_with_limits_for_test(
        server: &LocalIpcServer,
        runtime: &Runtime,
        authentication_timeout: Duration,
        request_timeout: Duration,
        max_message_bytes: usize,
    ) -> Result<(), IpcError> {
        serve_forever_with_limits(
            server,
            runtime,
            authentication_timeout,
            request_timeout,
            max_message_bytes,
        )
    }

    pub fn listener_reachable(endpoint: &LocalIpcEndpoint) -> bool {
        UnixStream::connect(&endpoint.socket_path).is_ok()
    }
}

fn runtime_error_code(error: &RuntimeError) -> String {
    match error {
        RuntimeError::InvalidInput(detail) => {
            if detail.starts_with("SILHOUETTE_FIT_REJECTED:") {
                let reason = detail.trim_start_matches("SILHOUETTE_FIT_REJECTED:").trim();
                let code = if reason.starts_with("STRICT_GLB_READBACK_FAILED")
                    || reason.starts_with("strict GLB readback failed")
                {
                    "SILHOUETTE_FIT_REJECTED_GLB"
                } else if reason.starts_with("candidate geometry evidence is not bound") {
                    "SILHOUETTE_FIT_REJECTED_EVIDENCE_BINDING"
                } else if reason.starts_with("GeometryProgram CAS is invalid") {
                    "SILHOUETTE_FIT_REJECTED_GEOMETRY_CAS"
                } else if reason.starts_with("persisted GeometryProgram scope is invalid") {
                    "SILHOUETTE_FIT_REJECTED_GEOMETRY_SCOPE"
                } else if reason.starts_with("persisted GeometryProgram provenance drifted") {
                    "SILHOUETTE_FIT_REJECTED_GEOMETRY_PROVENANCE"
                } else {
                    "SILHOUETTE_FIT_REJECTED"
                };
                return code.to_owned();
            }
            detail
            .split(':')
            .map(str::trim)
            .find(|value| {
                value.starts_with("AGENTIC_")
                    || value.starts_with("PRIMARY_FORM_REPAIR_")
                    || value.starts_with("SILHOUETTE_FIT_GEOMETRY_")
                    || value.starts_with("SILHOUETTE_FIT_RENDER_FAILED")
                    || value.starts_with("CAMERA_FIT_")
                    || value.starts_with("SILHOUETTE_FIT_")
                    || value.starts_with("CANDIDATE_ARTIFACT_UNAVAILABLE")
                })
            .map(str::to_owned)
            .map_or_else(|| format!("INVALID_INPUT: {detail}"), |code| code)
        }
        RuntimeError::Store(StoreError::Contract { code, .. }) => {
            format!("STORE_CONTRACT: {code}")
        }
        RuntimeError::Store(StoreError::InvalidData(detail)) => {
            format!("STORE_INVALID_DATA: {detail}")
        }
        RuntimeError::Store(StoreError::Sqlite(_)) => "STORE_SQLITE".to_owned(),
        RuntimeError::Store(StoreError::Cas(CasError::HashMismatch { .. })) => {
            "STORE_CAS_HASH_MISMATCH".to_owned()
        }
        RuntimeError::Store(StoreError::Cas(CasError::InvalidHash)) => {
            "STORE_CAS_INVALID_HASH".to_owned()
        }
        RuntimeError::Store(StoreError::Cas(CasError::Corrupt)) => {
            "STORE_CAS_CORRUPT".to_owned()
        }
        RuntimeError::Store(StoreError::Cas(CasError::CapacityExceeded)) => {
            "STORE_CAS_CAPACITY_EXCEEDED".to_owned()
        }
        RuntimeError::Store(StoreError::Cas(CasError::UnsafeRoot)) => {
            "STORE_CAS_UNSAFE_ROOT".to_owned()
        }
        RuntimeError::Store(StoreError::Cas(CasError::Io(_))) => "STORE_CAS_IO".to_owned(),
        RuntimeError::Store(StoreError::Io(_)) => "STORE_IO".to_owned(),
        RuntimeError::Store(StoreError::BackupUnavailable) => "STORE_BACKUP_UNAVAILABLE".to_owned(),
        RuntimeError::Store(StoreError::MigrationVersionUnsupported) => "STORE_MIGRATION_UNSUPPORTED".to_owned(),
        RuntimeError::Store(StoreError::LegacyDatabaseRejected) => "STORE_LEGACY_DATABASE_REJECTED".to_owned(),
        RuntimeError::Store(StoreError::LockPoisoned) => "STORE_LOCK_POISONED".to_owned(),
        RuntimeError::Ipc(_) => "IPC_ERROR".to_owned(),
        RuntimeError::ProcessLock(_) => "RUNTIME_BUSY".to_owned(),
    }
}

#[cfg(not(unix))]
mod platform {
    use super::*;

    pub struct LocalIpcServer;
    pub struct LocalIpcClient;

    impl LocalIpcServer {
        fn bind_internal(_endpoint: &LocalIpcEndpoint) -> Result<Self, IpcError> {
            Err(IpcError::UnsupportedPlatform)
        }
        pub fn serve_once(&self, _runtime: &Runtime) -> Result<(), IpcError> {
            Err(IpcError::UnsupportedPlatform)
        }
    }

    impl LocalIpcClient {
        fn connect_internal(_endpoint: &LocalIpcEndpoint) -> Result<Self, IpcError> {
            Err(IpcError::UnsupportedPlatform)
        }
        pub fn call(&mut self, _method: &str, _payload: Value) -> Result<Value, IpcError> {
            Err(IpcError::UnsupportedPlatform)
        }
    }

    pub fn new_client(endpoint: &LocalIpcEndpoint) -> Result<LocalIpcClient, IpcError> {
        LocalIpcClient::connect_internal(endpoint)
    }

    pub fn new_server(endpoint: &LocalIpcEndpoint) -> Result<LocalIpcServer, IpcError> {
        LocalIpcServer::bind_internal(endpoint)
    }

    pub fn serve_forever(_server: &LocalIpcServer, _runtime: &Runtime) -> Result<(), IpcError> {
        Err(IpcError::UnsupportedPlatform)
    }

    pub fn listener_reachable(_endpoint: &LocalIpcEndpoint) -> bool {
        false
    }
}

pub use platform::{LocalIpcClient, LocalIpcServer};

impl LocalIpcServer {
    pub fn bind(endpoint: &LocalIpcEndpoint) -> Result<Self, IpcError> {
        platform::new_server(endpoint)
    }

    /// Keep the Runtime process available for independently launched MCP and
    /// Viewer clients. Listener failures remain fatal; authentication,
    /// malformed input, disconnects and client transport errors are isolated
    /// to the accepted connection.
    pub fn serve_forever(&self, runtime: &Runtime) -> Result<(), IpcError> {
        platform::serve_forever(self, runtime)
    }
}

impl LocalIpcClient {
    pub fn connect(endpoint: &LocalIpcEndpoint) -> Result<Self, IpcError> {
        platform::new_client(endpoint)
    }
}

fn hash_token(token: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(token.as_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_equal(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn runtime_error_code_preserves_bounded_primary_form_stage() {
        assert_eq!(
            runtime_error_code(&RuntimeError::InvalidInput(
                "PRIMARY_FORM_REPAIR_INVALID: canonical_sha256 does not bind intent".to_owned(),
            )),
            "PRIMARY_FORM_REPAIR_INVALID"
        );
        assert_eq!(
            runtime_error_code(&RuntimeError::InvalidInput(
                "PRIMARY_FORM_REPAIR_REJECTED: selected GeometryProgram project differs".to_owned(),
            )),
            "PRIMARY_FORM_REPAIR_REJECTED"
        );
    }

    #[cfg(unix)]
    #[test]
    fn authentication_probe_is_bounded_when_listener_does_not_accept() {
        let directory = std::env::temp_dir().join(format!(
            "fc-auth-timeout-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ));
        fs::create_dir_all(&directory).expect("directory");
        let endpoint = LocalIpcEndpoint::new(&directory).expect("endpoint");
        let server = LocalIpcServer::bind(&endpoint).expect("server");
        assert!(endpoint.listener_reachable());

        let started = Instant::now();
        assert!(matches!(
            LocalIpcClient::connect(&endpoint),
            Err(IpcError::Io(_))
        ));
        assert!(started.elapsed() < Duration::from_secs(2));

        drop(server);
        assert!(!endpoint.listener_reachable());
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn rogue_idle_malformed_and_disconnected_clients_do_not_stop_runtime() {
        use std::io::Write;
        use std::net::Shutdown;
        use std::os::unix::net::UnixStream;

        let directory = std::env::temp_dir().join(format!(
            "fc-ci-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ));
        fs::create_dir_all(&directory).expect("directory");
        let endpoint = LocalIpcEndpoint::new(&directory).expect("endpoint");
        let runtime = std::sync::Arc::new(Runtime::ephemeral().expect("runtime"));
        let server = runtime.ipc_server(&endpoint).expect("server");
        let runtime_for_thread = runtime.clone();
        let server_thread = thread::spawn(move || server.serve_forever(&runtime_for_thread));

        let idle = UnixStream::connect(endpoint.socket_path()).expect("idle rogue client");
        thread::sleep(IPC_AUTHENTICATION_TIMEOUT + Duration::from_millis(100));
        drop(idle);

        let mut malformed =
            UnixStream::connect(endpoint.socket_path()).expect("malformed rogue client");
        malformed
            .write_all(b"{not-json}\n")
            .expect("malformed bytes");
        drop(malformed);

        let mut disconnected =
            UnixStream::connect(endpoint.socket_path()).expect("disconnecting rogue client");
        let authentication = serde_json::to_vec(&IpcRequest {
            version: 1,
            token: Some(endpoint.token().to_owned()),
            method: "authenticate".to_owned(),
            payload: Value::Null,
        })
        .expect("authentication request");
        disconnected
            .write_all(&authentication)
            .and_then(|_| disconnected.write_all(b"\n"))
            .expect("authentication bytes");
        disconnected
            .shutdown(Shutdown::Both)
            .expect("disconnect immediately");
        drop(disconnected);
        thread::sleep(Duration::from_millis(50));

        let mut legitimate = LocalIpcClient::connect(&endpoint).expect("legitimate client");
        // The authenticated request window is larger than the short auth
        // probe window, so ordinary calls are not mistaken for idle probes.
        thread::sleep(IPC_AUTHENTICATION_TIMEOUT + Duration::from_millis(100));
        let capabilities = legitimate
            .call("capabilities_get", Value::Null)
            .expect("legitimate request after rogue clients");
        assert_eq!(capabilities["status"], "alpha-mcp004");
        assert_eq!(
            legitimate
                .call("runtime_shutdown", Value::Null)
                .expect("shutdown")["shutting_down"],
            true
        );
        drop(legitimate);
        assert!(server_thread.join().expect("server thread").is_ok());
        drop(runtime);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn request_timeout_fits_geometry_and_codex_budgets() {
        assert!(IPC_REQUEST_TIMEOUT > Duration::from_secs(10));
        assert!(IPC_REQUEST_TIMEOUT < Duration::from_secs(60));
    }

    #[cfg(unix)]
    #[test]
    fn authenticated_idle_client_times_out_without_stopping_runtime() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixStream;

        let directory = std::env::temp_dir().join(format!(
            "fc-ri-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ));
        fs::create_dir_all(&directory).expect("directory");
        let endpoint = LocalIpcEndpoint::new(&directory).expect("endpoint");
        let runtime = std::sync::Arc::new(Runtime::ephemeral().expect("runtime"));
        let server = runtime.ipc_server(&endpoint).expect("server");
        let runtime_for_thread = runtime.clone();
        let request_timeout = Duration::from_millis(75);
        let server_thread = thread::spawn(move || {
            platform::serve_forever_for_test(&server, &runtime_for_thread, request_timeout)
        });

        let mut idle = UnixStream::connect(endpoint.socket_path()).expect("idle client");
        idle.set_read_timeout(Some(Duration::from_secs(1)))
            .expect("idle read timeout");
        let authentication = serde_json::to_vec(&IpcRequest {
            version: 1,
            token: Some(endpoint.token().to_owned()),
            method: "authenticate".to_owned(),
            payload: Value::Null,
        })
        .expect("authentication request");
        idle.write_all(&authentication)
            .and_then(|_| idle.write_all(b"\n"))
            .expect("authentication bytes");
        let mut authentication_response = String::new();
        BufReader::new(idle.try_clone().expect("idle reader"))
            .read_line(&mut authentication_response)
            .expect("authentication response");
        assert!(
            serde_json::from_str::<IpcResponse>(authentication_response.trim())
                .expect("authentication JSON")
                .ok
        );
        thread::sleep(request_timeout + Duration::from_millis(75));
        drop(idle);

        let mut legitimate = LocalIpcClient::connect(&endpoint).expect("legitimate client");
        assert_eq!(
            legitimate.configured_timeouts().expect("client timeouts"),
            (Some(IPC_REQUEST_TIMEOUT), Some(IPC_REQUEST_TIMEOUT))
        );
        assert_eq!(
            legitimate
                .call("capabilities_get", Value::Null)
                .expect("legitimate request")["status"],
            "alpha-mcp004"
        );
        legitimate
            .call("runtime_shutdown", Value::Null)
            .expect("shutdown");
        drop(legitimate);
        assert!(server_thread.join().expect("server thread").is_ok());
        drop(runtime);
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn slow_drip_and_unterminated_oversize_lines_are_bounded_and_isolated() {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixStream;

        let directory = std::env::temp_dir().join(format!(
            "fc-br-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ));
        fs::create_dir_all(&directory).expect("directory");
        let endpoint = LocalIpcEndpoint::new(&directory).expect("endpoint");
        let runtime = std::sync::Arc::new(Runtime::ephemeral().expect("runtime"));
        let server = runtime.ipc_server(&endpoint).expect("server");
        let runtime_for_thread = runtime.clone();
        let authentication_timeout = Duration::from_millis(450);
        let request_timeout = Duration::from_secs(1);
        let injected_limit = 512;
        let server_thread = thread::spawn(move || {
            platform::serve_forever_with_limits_for_test(
                &server,
                &runtime_for_thread,
                authentication_timeout,
                request_timeout,
                injected_limit,
            )
        });

        let mut slow = UnixStream::connect(endpoint.socket_path()).expect("slow drip client");
        let slow_started = Instant::now();
        let mut slow_rejected = false;
        for _ in 0..10 {
            if slow.write(&[b'{']).is_err() {
                slow_rejected = true;
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }
        assert!(
            slow_rejected,
            "absolute auth deadline must reject slow drip"
        );
        assert!(slow_started.elapsed() < Duration::from_secs(2));
        drop(slow);

        let mut oversized =
            UnixStream::connect(endpoint.socket_path()).expect("oversized line client");
        oversized
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("oversized read timeout");
        let oversized_started = Instant::now();
        oversized
            .write_all(&vec![b' '; injected_limit])
            .expect("unterminated oversized bytes");
        let mut response = [0u8; 1];
        let closed = match oversized.read(&mut response) {
            Ok(0) => true,
            Err(error) => matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::NotConnected
            ),
            Ok(_) => false,
        };
        assert!(closed, "limit breach must close the rogue client");
        assert!(oversized_started.elapsed() < Duration::from_secs(1));
        drop(oversized);

        let mut legitimate = LocalIpcClient::connect(&endpoint).expect("legitimate client");
        assert_eq!(
            legitimate
                .call("project_list", Value::Null)
                .expect("legitimate request"),
            json!([])
        );
        legitimate
            .call("runtime_shutdown", Value::Null)
            .expect("shutdown");
        drop(legitimate);
        assert!(server_thread.join().expect("server thread").is_ok());
        drop(runtime);
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn client_response_reader_uses_one_absolute_deadline() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;

        let directory = std::env::temp_dir().join(format!(
            "fc-cr-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ));
        fs::create_dir_all(&directory).expect("directory");
        let endpoint = LocalIpcEndpoint::new(&directory).expect("endpoint");
        let listener = UnixListener::bind(endpoint.socket_path()).expect("listener");
        let fake_server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accepted client");
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .expect("server read timeout");
            let mut reader = BufReader::new(stream.try_clone().expect("server reader"));
            let mut line = String::new();
            reader.read_line(&mut line).expect("authentication line");
            let authentication: IpcRequest =
                serde_json::from_str(line.trim()).expect("authentication JSON");
            assert_eq!(authentication.method, "authenticate");
            let response = serde_json::to_vec(&IpcResponse {
                version: 1,
                ok: true,
                code: None,
                payload: Some(json!({"authenticated":true})),
            })
            .expect("authentication response");
            stream
                .write_all(&response)
                .and_then(|_| stream.write_all(b"\n"))
                .expect("authentication response bytes");

            line.clear();
            reader.read_line(&mut line).expect("request line");
            let request: IpcRequest = serde_json::from_str(line.trim()).expect("request JSON");
            assert_eq!(request.method, "project_list");
            let mut slow_response = serde_json::to_vec(&IpcResponse {
                version: 1,
                ok: true,
                code: None,
                payload: Some(json!([])),
            })
            .expect("slow response");
            slow_response.push(b'\n');
            for byte in slow_response {
                if stream.write_all(&[byte]).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(200));
            }
        });

        let request_timeout = Duration::from_millis(450);
        let mut client = platform::new_client_with_limits_for_test(
            &endpoint,
            Duration::from_secs(1),
            request_timeout,
            512,
        )
        .expect("client");
        let started = Instant::now();
        assert!(matches!(
            client.call("project_list", Value::Null),
            Err(IpcError::Io(_))
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(client);
        fake_server.join().expect("fake server");
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn authenticated_client_can_read_runtime_but_wrong_token_is_rejected() {
        let directory = std::env::temp_dir().join(format!(
            "fc-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ));
        fs::create_dir_all(&directory).expect("directory");
        let endpoint = LocalIpcEndpoint::new(&directory).expect("endpoint");
        let runtime = std::sync::Arc::new(Runtime::ephemeral().expect("runtime"));
        let server = runtime.ipc_server(&endpoint).expect("server");
        let wrong = LocalIpcEndpoint {
            socket_path: endpoint.socket_path.clone(),
            token: "wrong-token".to_owned(),
        };
        let wrong_thread = thread::spawn(move || LocalIpcClient::connect(&wrong));
        assert!(matches!(
            server.serve_once(&runtime),
            Err(IpcError::AuthenticationFailed)
        ));
        assert!(matches!(
            wrong_thread.join().expect("wrong client"),
            Err(IpcError::AuthenticationFailed)
        ));
        drop(server);

        let server = runtime.ipc_server(&endpoint).expect("server");
        let endpoint_for_thread = endpoint.clone();
        let runtime_for_thread = runtime.clone();
        let server_thread = thread::spawn(move || runtime_for_thread.serve_ipc_once(&server));
        let mut client = LocalIpcClient::connect(&endpoint_for_thread).expect("client");
        let capabilities = client
            .call("capabilities_get", Value::Null)
            .expect("capabilities");
        assert_eq!(capabilities["status"], "alpha-mcp004");

        let project = runtime
            .create_project("IPC MCP004 fixture", json!({"scope":"test"}))
            .expect("project");
        let object = runtime
            .put_object(
                b"ipc prepared object",
                None,
                "application/octet-stream",
                "prepared-object",
            )
            .expect("object");
        let prepared = client
            .call(
                "candidate_prepare",
                json!({
                    "project_id": project.project_id,
                    "prepared_object_id": "ipc-prepared-object",
                    "prepared_object_sha256": object.record.sha256,
                    "request": {"typed":"diagnostic"}
                }),
            )
            .expect("candidate prepare");
        let candidate_id = prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate id")
            .to_owned();
        runtime
            .mark_candidate_quality(&candidate_id, "ipc-quality", true)
            .expect("quality");
        let confirmed = client
            .call(
                "candidate_confirm",
                json!({
                    "project_id": project.project_id,
                    "candidate_id": candidate_id,
                    "base_version_id": Value::Null,
                    "prepared_object_id": "ipc-prepared-object",
                    "prepared_object_sha256": object.record.sha256,
                    "quality_report_id": "ipc-quality",
                    "approval_receipt_id": "ipc-approval",
                    "approval_summary": "Confirm the IPC diagnostic candidate",
                    "approval_session_id": "ipc-session",
                    "approval_expires_at": "9999999999",
                    "idempotency_key": "ipc-confirm-once"
                }),
            )
            .expect("candidate confirm");
        assert_eq!(confirmed["replayed"], false);
        let source_version_id = confirmed["version_id"]
            .as_str()
            .expect("source version id")
            .to_owned();
        let restored = client
            .call(
                "restore_prepare",
                json!({
                    "project_id": project.project_id,
                    "base_version_id": source_version_id,
                    "source_version_id": source_version_id,
                    "request": {"reason":"IPC restore fixture"}
                }),
            )
            .expect("restore prepare");
        let restore_candidate_id = restored["candidate"]["candidate_id"]
            .as_str()
            .expect("restore candidate id")
            .to_owned();
        let restore_object_id = restored["candidate"]["prepared_object_id"]
            .as_str()
            .expect("restore object id")
            .to_owned();
        let restore_object_hash = restored["candidate"]["prepared_object_sha256"]
            .as_str()
            .expect("restore object hash")
            .to_owned();
        let restore_quality_id = restored["candidate"]["quality_report_id"]
            .as_str()
            .expect("restore quality id")
            .to_owned();
        let restored_version = client
            .call(
                "restore_confirm",
                json!({
                    "project_id": project.project_id,
                    "candidate_id": restore_candidate_id,
                    "source_version_id": source_version_id,
                    "base_version_id": confirmed["version_id"],
                    "prepared_object_id": restore_object_id,
                    "prepared_object_sha256": restore_object_hash,
                    "quality_report_id": restore_quality_id,
                    "approval_receipt_id": "ipc-restore-approval",
                    "approval_summary": "Confirm the IPC restore fixture",
                    "approval_session_id": "ipc-session",
                    "approval_expires_at": "9999999999",
                    "idempotency_key": "ipc-restore-once"
                }),
            )
            .expect("restore confirm");
        assert_eq!(restored_version["replayed"], false);
        let export = client
            .call(
                "export_prepare",
                json!({
                    "project_id": project.project_id,
                    "version_id": restored_version["version_id"],
                    "format": "manifest-json",
                    "profile": "diagnostic",
                    "request": {"target":"cas-only"}
                }),
            )
            .expect("export prepare");
        let export_id = export["manifest"]["export_id"].clone();
        let exported = client
            .call(
                "export_confirm",
                json!({
                    "project_id": project.project_id,
                    "export_id": export_id,
                    "version_id": restored_version["version_id"],
                    "format": "manifest-json",
                    "profile": "diagnostic",
                    "approval_receipt_id": "ipc-export-approval",
                    "approval_summary": "Confirm the IPC diagnostic export",
                    "approval_session_id": "ipc-session",
                    "approval_expires_at": "9999999999",
                    "idempotency_key": "ipc-export-once"
                }),
            )
            .expect("export confirm");
        assert_eq!(exported["replayed"], false);
        assert_eq!(
            runtime
                .versions(Some(&project.project_id))
                .expect("versions")
                .len(),
            2
        );
        drop(client);
        assert!(server_thread.join().expect("server thread").is_ok());
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn authenticated_shutdown_releases_process_lock() {
        let root = std::env::temp_dir().join(format!(
            "fc-shutdown-{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ));
        fs::create_dir_all(&root).expect("root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let runtime = std::sync::Arc::new(
            Runtime::open_with_cas(&database, &cas).expect("runtime with process lock"),
        );
        let endpoint = LocalIpcEndpoint::new(root.join("ipc")).expect("endpoint");
        let server = runtime.ipc_server(&endpoint).expect("server");
        let runtime_for_thread = runtime.clone();
        let server_thread = thread::spawn(move || server.serve_forever(&runtime_for_thread));

        let mut client = LocalIpcClient::connect(&endpoint).expect("client");
        let response = client
            .call("runtime_shutdown", Value::Null)
            .expect("shutdown response");
        assert_eq!(response["shutting_down"], true);
        drop(client);
        assert!(server_thread.join().expect("server thread").is_ok());
        drop(runtime);

        let reopened = Runtime::open_with_cas(&database, &cas)
            .expect("runtime can restart immediately after graceful shutdown");
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }
}
