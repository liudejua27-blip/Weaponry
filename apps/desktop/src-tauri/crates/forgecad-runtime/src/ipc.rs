use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use super::Runtime;

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
    #[error("local IPC runtime request failed")]
    RuntimeRequest,
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
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};

    pub struct LocalIpcServer {
        listener: UnixListener,
        socket_path: PathBuf,
        token_hash: String,
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
            serve_stream(stream, runtime, &self.token_hash)
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
    }

    impl LocalIpcClient {
        fn connect_internal(endpoint: &LocalIpcEndpoint) -> Result<Self, IpcError> {
            let stream = UnixStream::connect(&endpoint.socket_path)?;
            let reader_stream = stream.try_clone()?;
            let mut client = Self {
                reader: BufReader::new(reader_stream),
                writer: stream,
            };
            client.send(IpcRequest {
                version: 1,
                token: Some(endpoint.token.clone()),
                method: "authenticate".to_owned(),
                payload: Value::Null,
            })?;
            let response = client.receive()?;
            if !response.ok {
                return Err(IpcError::AuthenticationFailed);
            }
            Ok(client)
        }

        pub fn call(&mut self, method: &str, payload: Value) -> Result<Value, IpcError> {
            self.send(IpcRequest {
                version: 1,
                token: None,
                method: method.to_owned(),
                payload,
            })?;
            let response = self.receive()?;
            if !response.ok {
                return Err(IpcError::RuntimeRequest);
            }
            response.payload.ok_or(IpcError::Protocol)
        }

        fn send(&mut self, request: IpcRequest) -> Result<(), IpcError> {
            let bytes = serde_json::to_vec(&request).map_err(|_| IpcError::Protocol)?;
            if bytes.len() > 1024 * 1024 {
                return Err(IpcError::Protocol);
            }
            self.writer.write_all(&bytes)?;
            self.writer.write_all(b"\n")?;
            self.writer.flush()?;
            Ok(())
        }

        fn receive(&mut self) -> Result<IpcResponse, IpcError> {
            let mut line = String::new();
            let count = self.reader.read_line(&mut line)?;
            if count == 0 || line.len() > 1024 * 1024 {
                return Err(IpcError::Protocol);
            }
            serde_json::from_str(line.trim()).map_err(|_| IpcError::Protocol)
        }
    }

    fn serve_stream(
        stream: UnixStream,
        runtime: &Runtime,
        token_hash: &str,
    ) -> Result<(), IpcError> {
        let reader_stream = stream.try_clone()?;
        let mut reader = BufReader::new(reader_stream);
        let mut writer = stream;
        let Some(first) = receive(&mut reader)? else {
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
        )?;

        while let Some(request) = receive(&mut reader)? {
            if request.version != 1 || request.token.is_some() || request.method == "authenticate" {
                send(
                    &mut writer,
                    &IpcResponse {
                        version: 1,
                        ok: false,
                        code: Some("PROTOCOL_ERROR".to_owned()),
                        payload: None,
                    },
                )?;
                continue;
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
                )?,
                Err(_) => send(
                    &mut writer,
                    &IpcResponse {
                        version: 1,
                        ok: false,
                        code: Some("RUNTIME_REQUEST_FAILED".to_owned()),
                        payload: None,
                    },
                )?,
            }
        }
        Ok(())
    }

    fn receive(reader: &mut BufReader<UnixStream>) -> Result<Option<IpcRequest>, IpcError> {
        let mut line = String::new();
        let count = reader.read_line(&mut line)?;
        if count == 0 {
            return Ok(None);
        }
        if line.len() > 1024 * 1024 {
            return Err(IpcError::Protocol);
        }
        serde_json::from_str(line.trim())
            .map(Some)
            .map_err(|_| IpcError::Protocol)
    }

    fn send(writer: &mut UnixStream, response: &IpcResponse) -> Result<(), IpcError> {
        let bytes = serde_json::to_vec(response).map_err(|_| IpcError::Protocol)?;
        writer.write_all(&bytes)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }

    pub fn new_client(endpoint: &LocalIpcEndpoint) -> Result<LocalIpcClient, IpcError> {
        LocalIpcClient::connect_internal(endpoint)
    }

    pub fn new_server(endpoint: &LocalIpcEndpoint) -> Result<LocalIpcServer, IpcError> {
        LocalIpcServer::bind_internal(endpoint)
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
}

pub use platform::{LocalIpcClient, LocalIpcServer};

impl LocalIpcServer {
    pub fn bind(endpoint: &LocalIpcEndpoint) -> Result<Self, IpcError> {
        platform::new_server(endpoint)
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
        assert_eq!(capabilities["status"], "alpha-mcp003");
        drop(client);
        assert!(server_thread.join().expect("server thread").is_ok());
        let _ = fs::remove_dir_all(directory);
    }
}
