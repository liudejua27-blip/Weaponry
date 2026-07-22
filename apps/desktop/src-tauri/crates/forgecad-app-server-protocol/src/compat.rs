use serde::{Deserialize, Serialize};

use crate::RpcError;

pub const HTTP_COMPAT_REQUEST_SCHEMA_VERSION: &str = "ForgeCADHttpCompatibilityRequest@1";
pub const HTTP_COMPAT_RESPONSE_SCHEMA_VERSION: &str = "ForgeCADHttpCompatibilityResponse@1";
pub const SSE_SUBSCRIPTION_SCHEMA_VERSION: &str = "ForgeCADSseSubscription@1";
pub const SSE_UNSUBSCRIBE_SCHEMA_VERSION: &str = "ForgeCADSseUnsubscribe@1";
pub const SSE_NOTIFICATION_SCHEMA_VERSION: &str = "ForgeCADSseNotification@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "encoding", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProtocolHttpBody {
    Empty,
    Utf8 { data: String },
    Base64 { data: String },
}

impl ProtocolHttpBody {
    pub fn encoded_len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Utf8 { data } | Self::Base64 { data } => data.len(),
        }
    }
}

/// The exact temporary HTTP bridge DTO consumed by the TypeScript transport.
/// The adapter, not the caller, owns the loopback origin and route policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CompatHttpRequest {
    pub schema_version: String,
    pub path: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: ProtocolHttpBody,
}

impl CompatHttpRequest {
    pub fn validate_schema(&self) -> Result<(), RpcError> {
        if self.schema_version != HTTP_COMPAT_REQUEST_SCHEMA_VERSION {
            return Err(RpcError::invalid_params(format!(
                "schema_version must be {HTTP_COMPAT_REQUEST_SCHEMA_VERSION}."
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CompatHttpResponse {
    pub schema_version: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: ProtocolHttpBody,
}

impl CompatHttpResponse {
    pub fn validate(&self) -> Result<(), RpcError> {
        if self.schema_version != HTTP_COMPAT_RESPONSE_SCHEMA_VERSION {
            return Err(RpcError::invalid_params(format!(
                "schema_version must be {HTTP_COMPAT_RESPONSE_SCHEMA_VERSION}."
            )));
        }
        if !(100..=599).contains(&self.status) {
            return Err(RpcError::invalid_params(
                "compat/http response status must be a valid HTTP status code.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SseSubscriptionParams {
    pub schema_version: String,
    pub stream_id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SseUnsubscribeParams {
    pub schema_version: String,
    pub stream_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SseNotificationParams {
    pub schema_version: String,
    pub stream_id: String,
    pub event: String,
    pub data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn http_request_matches_the_typescript_wire_fixture() {
        let fixture = json!({
            "schema_version": "ForgeCADHttpCompatibilityRequest@1",
            "path": "/api/v1/agent/threads",
            "method": "POST",
            "headers": [["content-type", "application/json"]],
            "body": {"encoding": "utf8", "data": "{}"}
        });
        let request: CompatHttpRequest = serde_json::from_value(fixture.clone()).unwrap();
        request.validate_schema().unwrap();
        assert_eq!(serde_json::to_value(request).unwrap(), fixture);
    }

    #[test]
    fn empty_body_has_no_data_field() {
        assert_eq!(
            serde_json::to_value(ProtocolHttpBody::Empty).unwrap(),
            json!({"encoding": "empty"})
        );
    }
}
