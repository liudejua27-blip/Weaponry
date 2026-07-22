use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::JSONRPC_VERSION;

/// Server notification shape shared with the TypeScript transport.  Delivery
/// metadata is top-level by design; method-specific data stays in `params`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ServerNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

impl ServerNotification {
    pub fn new(
        method: impl Into<String>,
        params: Value,
        notification_id: impl Into<String>,
        cursor: impl Into<String>,
    ) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params: Some(params),
            notification_id: Some(notification_id.into()),
            cursor: Some(cursor.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReplayResult {
    pub notifications: Vec<ServerNotification>,
}
