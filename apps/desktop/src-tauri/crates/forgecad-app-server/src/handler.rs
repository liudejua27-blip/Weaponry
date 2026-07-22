use std::{future::Future, pin::Pin};

use crate::CancellationToken;
use forgecad_app_server_protocol::RpcError;
use serde_json::Value;

pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<Value, RpcError>> + Send + 'static>>;

/// Application method port. K001 implementations normally point at the single
/// Python compatibility adapter; K002 can replace it without changing JSON-RPC.
pub trait RequestHandler: Send + Sync + 'static {
    fn handle(
        &self,
        method: String,
        params: Value,
        cancellation: CancellationToken,
    ) -> HandlerFuture;
}

#[derive(Debug, Default)]
pub struct MethodNotFoundHandler;

impl RequestHandler for MethodNotFoundHandler {
    fn handle(
        &self,
        method: String,
        _params: Value,
        _cancellation: CancellationToken,
    ) -> HandlerFuture {
        Box::pin(async move { Err(RpcError::method_not_found(&method)) })
    }
}
