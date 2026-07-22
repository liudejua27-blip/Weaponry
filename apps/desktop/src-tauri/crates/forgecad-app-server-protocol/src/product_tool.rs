use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    contract_validation::{
        require_bounded_json, require_schema, require_sha256, require_stable_id, require_text,
    },
    RpcError,
};

pub const PRODUCT_TOOL_EXECUTION_REQUEST_SCHEMA_VERSION: &str = "ProductToolExecutionRequest@1";
pub const PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION: &str = "ProductToolExecutionResult@1";
pub const PRODUCT_TOOL_REGISTRY_SCHEMA_VERSION: &str = "ForgeCADProductToolRegistry@1";

const MAX_TOOL_MESSAGE_CHARS: usize = 500;
const FORBIDDEN_EXECUTOR_KEYS: &[&str] = &[
    "provider_key",
    "api_key",
    "authorization",
    "file_path",
    "filesystem_path",
    "database_path",
    "sqlite_path",
    "object_path",
    "url",
    "endpoint_url",
    "session",
    "session_id",
    "snapshot_write_token",
    "snapshot_token",
    "reasoning_content",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductToolApprovalPolicy {
    ReadOnly,
    CandidateOnly,
    UserConfirmationRequired,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductToolExecutionStatus {
    Completed,
    Failed,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductToolFailureCategory {
    Schema,
    Permission,
    Unsupported,
    Conflict,
    Cancelled,
    Timeout,
    Provider,
    Execution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ValidatedProductToolPayload {
    pub schema_id: String,
    pub schema_sha256: String,
    pub value: BTreeMap<String, Value>,
}

impl ValidatedProductToolPayload {
    fn validate(&self, field: &str, expected_schema_id: &str) -> Result<(), RpcError> {
        if self.schema_id != expected_schema_id {
            return Err(RpcError::invalid_params(format!(
                "{field}.schema_id must be {expected_schema_id}."
            )));
        }
        require_sha256(&format!("{field}.schema_sha256"), &self.schema_sha256)?;
        require_bounded_json(field, &self.value, FORBIDDEN_EXECUTOR_KEYS)?;
        reject_machine_locations(field, &self.value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProductToolExecutionRequest {
    pub schema_version: String,
    pub execution_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool_id: String,
    pub tool_name: String,
    pub registry_schema_version: String,
    pub idempotency_key: String,
    pub validated_arguments: ValidatedProductToolPayload,
    pub approval_policy: ProductToolApprovalPolicy,
    pub cancellation_id: String,
    pub cancellation_token: String,
}

impl ProductToolExecutionRequest {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "product_tool_request.schema_version",
            &self.schema_version,
            PRODUCT_TOOL_EXECUTION_REQUEST_SCHEMA_VERSION,
        )?;
        require_stable_id("product_tool_request.execution_id", &self.execution_id)?;
        require_stable_id("product_tool_request.turn_id", &self.turn_id)?;
        require_stable_id("product_tool_request.call_id", &self.call_id)?;
        validate_tool_id(&self.tool_id)?;
        validate_tool_name(&self.tool_name)?;
        require_schema(
            "product_tool_request.registry_schema_version",
            &self.registry_schema_version,
            PRODUCT_TOOL_REGISTRY_SCHEMA_VERSION,
        )?;
        require_sha256(
            "product_tool_request.idempotency_key",
            &self.idempotency_key,
        )?;
        require_stable_id(
            "product_tool_request.cancellation_id",
            &self.cancellation_id,
        )?;
        require_stable_id(
            "product_tool_request.cancellation_token",
            &self.cancellation_token,
        )?;
        self.validated_arguments.validate(
            "product_tool_request.validated_arguments",
            &format!("{}:input", self.tool_id),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProductToolExecutionResult {
    pub schema_version: String,
    pub execution_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool_id: String,
    pub cancellation_id: String,
    pub status: ProductToolExecutionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validated_output: Option<ValidatedProductToolPayload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_category: Option<ProductToolFailureCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub duration_ms: u64,
    pub permanent_side_effects: u32,
}

impl ProductToolExecutionResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "product_tool_result.schema_version",
            &self.schema_version,
            PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION,
        )?;
        require_stable_id("product_tool_result.execution_id", &self.execution_id)?;
        require_stable_id("product_tool_result.turn_id", &self.turn_id)?;
        require_stable_id("product_tool_result.call_id", &self.call_id)?;
        validate_tool_id(&self.tool_id)?;
        require_stable_id("product_tool_result.cancellation_id", &self.cancellation_id)?;
        if let Some(error_code) = self.error_code.as_deref() {
            require_stable_id("product_tool_result.error_code", error_code)?;
        }
        if let Some(message) = self.message.as_deref() {
            require_text(
                "product_tool_result.message",
                message,
                0,
                MAX_TOOL_MESSAGE_CHARS,
            )?;
        }
        if self.permanent_side_effects != 0 {
            return Err(RpcError::invalid_params(
                "K002 Product Tool results must report permanent_side_effects=0; permanent writes remain behind approval and product core.",
            ));
        }
        match self.status {
            ProductToolExecutionStatus::Completed => {
                let output = self.validated_output.as_ref().ok_or_else(|| {
                    RpcError::invalid_params(
                        "Completed Product Tool result requires validated_output.",
                    )
                })?;
                if self.failure_category.is_some() || self.error_code.is_some() {
                    return Err(RpcError::invalid_params(
                        "Completed Product Tool result cannot contain failure fields.",
                    ));
                }
                output.validate(
                    "product_tool_result.validated_output",
                    &format!("{}:output", self.tool_id),
                )?;
            }
            ProductToolExecutionStatus::Failed
            | ProductToolExecutionStatus::Cancelled
            | ProductToolExecutionStatus::Rejected => {
                if self.validated_output.is_some() || self.failure_category.is_none() {
                    return Err(RpcError::invalid_params(
                        "Non-completed Product Tool result requires failure_category and no output.",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_tool_id(value: &str) -> Result<(), RpcError> {
    require_stable_id("product_tool.tool_id", value)?;
    if !value.starts_with("forgecad.") || !value.ends_with(".v1") {
        return Err(RpcError::invalid_params(
            "product_tool.tool_id must be a code-owned forgecad.*.v1 ID.",
        ));
    }
    Ok(())
}

fn validate_tool_name(value: &str) -> Result<(), RpcError> {
    let valid = (2..=64).contains(&value.len())
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| match (index, byte) {
                (0, b'a'..=b'z') => true,
                (_, b'a'..=b'z' | b'0'..=b'9' | b'_') => true,
                _ => false,
            });
    if !valid {
        return Err(RpcError::invalid_params(
            "product_tool.tool_name must match ^[a-z][a-z0-9_]{1,63}$.",
        ));
    }
    Ok(())
}

fn reject_machine_locations(field: &str, value: &BTreeMap<String, Value>) -> Result<(), RpcError> {
    fn forbidden(value: &Value) -> bool {
        match value {
            Value::String(value) => {
                value.starts_with('/')
                    || value.starts_with("~/")
                    || value.starts_with("file://")
                    || value.starts_with("http://")
                    || value.starts_with("https://")
                    || (value.len() >= 3
                        && value.as_bytes()[0].is_ascii_alphabetic()
                        && value.as_bytes()[1] == b':'
                        && matches!(value.as_bytes()[2], b'/' | b'\\'))
            }
            Value::Array(values) => values.iter().any(forbidden),
            Value::Object(values) => values.values().any(forbidden),
            _ => false,
        }
    }

    if value.values().any(forbidden) {
        return Err(RpcError::invalid_params(format!(
            "{field} cannot contain machine paths, file URLs or network URLs."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn request() -> ProductToolExecutionRequest {
        ProductToolExecutionRequest {
            schema_version: PRODUCT_TOOL_EXECUTION_REQUEST_SCHEMA_VERSION.into(),
            execution_id: "tool_execution_1".into(),
            turn_id: "turn_1".into(),
            call_id: "call_1".into(),
            tool_id: "forgecad.geometry.compile_readback.v1".into(),
            tool_name: "compile_readback_candidate".into(),
            registry_schema_version: PRODUCT_TOOL_REGISTRY_SCHEMA_VERSION.into(),
            idempotency_key: "a".repeat(64),
            validated_arguments: ValidatedProductToolPayload {
                schema_id: "forgecad.geometry.compile_readback.v1:input".into(),
                schema_sha256: "b".repeat(64),
                value: BTreeMap::new(),
            },
            approval_policy: ProductToolApprovalPolicy::CandidateOnly,
            cancellation_id: "cancel_1".into(),
            cancellation_token: "cancel_token_1".into(),
        }
    }

    #[test]
    fn request_is_turn_scoped_but_has_no_thread_or_session_identity() {
        let request = request();
        request.validate().unwrap();
        let value = serde_json::to_value(request).unwrap();
        assert!(value.get("thread_id").is_none());
        assert!(value.get("session_id").is_none());
        assert!(value.get("provider_id").is_none());
    }

    #[test]
    fn request_rejects_sensitive_nested_executor_arguments() {
        for forbidden in [
            "provider_key",
            "file_path",
            "session_id",
            "snapshot_write_token",
            "reasoning_content",
        ] {
            let mut request = request();
            request
                .validated_arguments
                .value
                .insert("nested".into(), json!({(forbidden): "secret"}));
            assert!(request.validate().is_err(), "{forbidden}");
        }
    }

    #[test]
    fn request_requires_tool_bound_input_schema_identity() {
        let mut request = request();
        request.validated_arguments.schema_id = "other:input".into();
        assert!(request.validate().is_err());
    }

    #[test]
    fn geometry_path_is_allowed_but_machine_paths_and_urls_are_rejected() {
        let mut geometry_path = request();
        geometry_path.validated_arguments.value.insert(
            "path".into(),
            json!({"points": [[0, 0, 0], [1, 0, 0]], "closed": false}),
        );
        geometry_path.validate().unwrap();

        for location in [
            "/Users/example/private.glb",
            "C:\\Users\\example\\private.glb",
            "file:///tmp/private.glb",
            "https://example.invalid/private.glb",
        ] {
            let mut request = request();
            request
                .validated_arguments
                .value
                .insert("source".into(), json!(location));
            assert!(request.validate().is_err(), "{location}");
        }
    }

    #[test]
    fn completed_result_requires_schema_valid_output_and_zero_writes() {
        let mut result = ProductToolExecutionResult {
            schema_version: PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION.into(),
            execution_id: "tool_execution_1".into(),
            turn_id: "turn_1".into(),
            call_id: "call_1".into(),
            tool_id: "forgecad.geometry.compile_readback.v1".into(),
            cancellation_id: "cancel_1".into(),
            status: ProductToolExecutionStatus::Completed,
            validated_output: Some(ValidatedProductToolPayload {
                schema_id: "forgecad.geometry.compile_readback.v1:output".into(),
                schema_sha256: "c".repeat(64),
                value: BTreeMap::from([("evidence_source".into(), json!("glb_readback"))]),
            }),
            failure_category: None,
            error_code: None,
            message: None,
            duration_ms: 25,
            permanent_side_effects: 0,
        };
        result.validate().unwrap();
        result.permanent_side_effects = 1;
        assert!(result.validate().is_err());
    }

    #[test]
    fn unknown_provider_or_path_fields_fail_deserialization() {
        let mut value = serde_json::to_value(request()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("thread_id".into(), json!("thread_forbidden"));
        assert!(serde_json::from_value::<ProductToolExecutionRequest>(value).is_err());
    }
}
