use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

pub const SERVER_OVERLOADED: i32 = -32001;
pub const NOT_INITIALIZED: i32 = -32002;
pub const PROTOCOL_VERSION_UNSUPPORTED: i32 = -32003;
pub const ALREADY_INITIALIZED: i32 = -32004;
pub const DUPLICATE_REQUEST_ID: i32 = -32005;
pub const UNKNOWN_REQUEST_ID: i32 = -32006;
pub const REQUEST_CANCELLED: i32 = -32007;
pub const CURSOR_RESYNC_REQUIRED: i32 = -32008;
pub const COMPAT_BACKEND_UNAVAILABLE: i32 = -32009;
pub const MALFORMED_UPSTREAM_EVENT: i32 = -32010;
pub const SLOW_CONSUMER: i32 = -32011;
pub const INPUT_TOO_LARGE: i32 = -32012;
pub const CAPABILITY_UNSUPPORTED: i32 = -32013;

pub const ERROR_SCHEMA_VERSION: &str = "ForgeCADProtocolError@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RpcErrorData {
    pub schema_version: String,
    pub application_code: String,
    pub recoverable: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl RpcErrorData {
    pub fn new(application_code: impl Into<String>, recoverable: bool) -> Self {
        Self {
            schema_version: ERROR_SCHEMA_VERSION.to_string(),
            application_code: application_code.into(),
            recoverable,
            details: BTreeMap::new(),
            request_id: None,
            retry_after_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: RpcErrorData,
}

impl RpcError {
    pub fn new(
        code: i32,
        application_code: impl Into<String>,
        message: impl Into<String>,
        recoverable: bool,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            data: RpcErrorData::new(application_code, recoverable),
        }
    }

    pub fn parse(message: impl Into<String>) -> Self {
        Self::new(PARSE_ERROR, "PARSE_ERROR", message, false)
    }

    pub fn invalid_request(
        application_code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(INVALID_REQUEST, application_code, message, false)
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, "INVALID_PARAMS", message, false)
    }

    pub fn not_initialized() -> Self {
        Self::new(
            NOT_INITIALIZED,
            "NOT_INITIALIZED",
            "The connection must complete initialize/initialized before this method is used.",
            false,
        )
    }

    pub fn method_not_found(method: &str) -> Self {
        let mut error = Self::new(
            METHOD_NOT_FOUND,
            "METHOD_NOT_FOUND",
            "The requested app-server method is not registered.",
            false,
        );
        error
            .data
            .details
            .insert("method".into(), Value::String(method.to_string()));
        error
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(INTERNAL_ERROR, "INTERNAL_ERROR", message, false)
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.data.request_id = Some(request_id.into());
        self
    }
}
