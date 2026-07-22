use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{RpcError, INPUT_TOO_LARGE};

pub const JSONRPC_VERSION: &str = "2.0";
/// Temporary K001 text-frame ceiling. A production GLB is currently carried as
/// base64; the compatibility adapter therefore caps raw bodies at 47 MiB so
/// the encoded body and JSON-RPC envelope still fit this bound.
pub const DEFAULT_MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn validate(&self) -> Result<(), RpcError> {
        if self.0.is_empty() || self.0.len() > 160 || !self.0.is_ascii() {
            return Err(RpcError::invalid_request(
                "INVALID_REQUEST_ID",
                "Request IDs must contain between 1 and 160 ASCII bytes.",
            ));
        }
        Ok(())
    }
}

fn null_value() -> Value {
    Value::Null
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(default = "null_value")]
    pub params: Value,
}

impl JsonRpcRequest {
    pub fn new(id: RequestId, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.into(),
            params,
        }
    }

    pub fn validate(&self) -> Result<(), RpcError> {
        validate_common(&self.jsonrpc, &self.method)?;
        self.id.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default = "null_value")]
    pub params: Value,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }

    pub fn validate(&self) -> Result<(), RpcError> {
        validate_common(&self.jsonrpc, &self.method)
    }
}

fn validate_common(jsonrpc: &str, method: &str) -> Result<(), RpcError> {
    if jsonrpc != JSONRPC_VERSION {
        return Err(RpcError::invalid_request(
            "JSONRPC_VERSION_INVALID",
            "The jsonrpc field must be exactly \"2.0\".",
        ));
    }
    if method.is_empty() || method.len() > 160 {
        return Err(RpcError::invalid_request(
            "METHOD_INVALID",
            "JSON-RPC method names must contain between 1 and 160 bytes.",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
}

impl ClientMessage {
    pub fn method(&self) -> &str {
        match self {
            Self::Request(request) => &request.method,
            Self::Notification(notification) => &notification.method,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<RequestId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(id: Option<RequestId>, error: RpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    pub fn validate(&self) -> Result<(), RpcError> {
        if self.jsonrpc != JSONRPC_VERSION {
            return Err(RpcError::invalid_request(
                "JSONRPC_VERSION_INVALID",
                "The jsonrpc field must be exactly \"2.0\".",
            ));
        }
        if self.result.is_some() == self.error.is_some() {
            return Err(RpcError::invalid_request(
                "RESPONSE_SHAPE_INVALID",
                "A JSON-RPC response must contain exactly one of result or error.",
            ));
        }
        Ok(())
    }
}

pub fn parse_client_message(
    frame: &str,
    max_frame_bytes: usize,
) -> Result<ClientMessage, RpcError> {
    if frame.len() > max_frame_bytes {
        return Err(RpcError::new(
            INPUT_TOO_LARGE,
            "INPUT_TOO_LARGE",
            "The JSON-RPC frame exceeds the negotiated byte limit.",
            false,
        ));
    }
    let value: Value = serde_json::from_str(frame)
        .map_err(|error| RpcError::parse(format!("Malformed JSON: {error}")))?;
    let object = value.as_object().ok_or_else(|| {
        RpcError::invalid_request(
            "INVALID_REQUEST",
            "Batch and non-object JSON-RPC frames are not supported.",
        )
    })?;
    if object.contains_key("id") {
        if object.get("id").is_some_and(Value::is_null) {
            return Err(RpcError::invalid_request(
                "INVALID_REQUEST_ID",
                "Null request IDs are not accepted by ForgeCAD.",
            ));
        }
        let request: JsonRpcRequest = serde_json::from_value(value).map_err(|error| {
            RpcError::invalid_request(
                "INVALID_REQUEST",
                format!("Invalid JSON-RPC request: {error}"),
            )
        })?;
        request.validate()?;
        Ok(ClientMessage::Request(request))
    } else {
        let notification: JsonRpcNotification = serde_json::from_value(value).map_err(|error| {
            RpcError::invalid_request(
                "INVALID_NOTIFICATION",
                format!("Invalid JSON-RPC notification: {error}"),
            )
        })?;
        notification.validate()?;
        Ok(ClientMessage::Notification(notification))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn explicit_jsonrpc_2_request_and_notification_parse() {
        let request = parse_client_message(
            r#"{"jsonrpc":"2.0","id":"req_1","method":"thread/read","params":{"thread_id":"thread_1"}}"#,
            DEFAULT_MAX_FRAME_BYTES,
        )
        .expect("request parses");
        assert!(matches!(request, ClientMessage::Request(_)));

        let notification = parse_client_message(
            r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#,
            DEFAULT_MAX_FRAME_BYTES,
        )
        .expect("notification parses");
        assert!(matches!(notification, ClientMessage::Notification(_)));
    }

    #[test]
    fn missing_or_wrong_jsonrpc_and_batch_are_rejected() {
        for frame in [
            r#"{"id":1,"method":"initialize"}"#,
            r#"{"jsonrpc":"1.0","id":1,"method":"initialize"}"#,
            r#"[{"jsonrpc":"2.0","id":1,"method":"initialize"}]"#,
        ] {
            assert!(
                parse_client_message(frame, DEFAULT_MAX_FRAME_BYTES).is_err(),
                "{frame}"
            );
        }
    }

    #[test]
    fn response_requires_exactly_one_payload() {
        let valid = JsonRpcResponse::success(RequestId("req_1".into()), json!({"ok": true}));
        valid.validate().expect("valid response");
        let invalid = JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(RequestId("req_1".into())),
            result: Some(json!({})),
            error: Some(RpcError::internal("bad")),
        };
        assert_eq!(
            invalid.validate().unwrap_err().data.application_code,
            "RESPONSE_SHAPE_INVALID"
        );
    }

    #[test]
    fn frame_budget_is_enforced_before_parsing() {
        let error = parse_client_message("{}", 1).unwrap_err();
        assert_eq!(error.code, INPUT_TOO_LARGE);
    }

    #[test]
    fn production_sized_text_frame_is_allowed_but_over_64_mib_is_rejected() {
        let payload = "x".repeat(1024 * 1024 + 1);
        let frame = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": "req_large",
            "method": "compat/http",
            "params": {"body": payload}
        }))
        .unwrap();
        assert!(parse_client_message(&frame, DEFAULT_MAX_FRAME_BYTES).is_ok());

        let oversized = " ".repeat(DEFAULT_MAX_FRAME_BYTES + 1);
        let error = parse_client_message(&oversized, DEFAULT_MAX_FRAME_BYTES).unwrap_err();
        assert_eq!(error.code, INPUT_TOO_LARGE);
    }
}
