use serde::{Deserialize, Serialize};

use crate::{
    contract_validation::{require_optional_stable_id, require_schema, require_stable_id},
    RpcError,
};

pub const PROVIDER_PREFLIGHT_COMMAND_SCHEMA_VERSION: &str = "ProviderPreflightCommand@1";
pub const PROVIDER_PREFLIGHT_RESULT_SCHEMA_VERSION: &str = "ProviderPreflightResult@1";
pub const PROVIDER_CHECK_COMMAND_SCHEMA_VERSION: &str = "ProviderCheckCommand@1";
pub const PROVIDER_CHECK_RESULT_SCHEMA_VERSION: &str = "ProviderCheckResult@1";
pub const PROVIDER_CANCEL_COMMAND_SCHEMA_VERSION: &str = "ProviderCancelCommand@1";
pub const PROVIDER_CANCEL_RESULT_SCHEMA_VERSION: &str = "ProviderCancelResult@1";

const MAX_PROVIDER_TIMEOUT_MS: u32 = 120_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLifecycleStatus {
    Unconfigured,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureCategory {
    InvalidRequest,
    Authentication,
    Balance,
    RateLimited,
    ServerUnavailable,
    Timeout,
    Network,
    EmptyContent,
    InvalidJson,
    SchemaViolation,
    BudgetExceeded,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub prompt_cache_hit_tokens: u64,
    pub prompt_cache_miss_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderPreflightCommand {
    pub schema_version: String,
    pub execution_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_provider_id: Option<String>,
}

impl ProviderPreflightCommand {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "provider_preflight.schema_version",
            &self.schema_version,
            PROVIDER_PREFLIGHT_COMMAND_SCHEMA_VERSION,
        )?;
        require_stable_id("provider_preflight.execution_id", &self.execution_id)?;
        require_optional_stable_id(
            "provider_preflight.requested_provider_id",
            self.requested_provider_id.as_deref(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderPreflightResult {
    pub schema_version: String,
    pub execution_id: String,
    pub status: ProviderLifecycleStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    pub configured: bool,
    pub network_call_made: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_category: Option<ProviderFailureCategory>,
}

impl ProviderPreflightResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "provider_preflight_result.schema_version",
            &self.schema_version,
            PROVIDER_PREFLIGHT_RESULT_SCHEMA_VERSION,
        )?;
        require_stable_id("provider_preflight_result.execution_id", &self.execution_id)?;
        require_optional_stable_id(
            "provider_preflight_result.provider_id",
            self.provider_id.as_deref(),
        )?;
        if self.network_call_made {
            return Err(RpcError::invalid_params(
                "Provider preflight must never make a network call.",
            ));
        }
        match self.status {
            ProviderLifecycleStatus::Unconfigured => {
                if self.configured || self.failure_category.is_some() {
                    return Err(RpcError::invalid_params(
                        "Unconfigured preflight must report configured=false without a failure category.",
                    ));
                }
            }
            ProviderLifecycleStatus::Ready => {
                if !self.configured || self.provider_id.is_none() || self.failure_category.is_some()
                {
                    return Err(RpcError::invalid_params(
                        "Ready preflight requires configured provider identity and no failure.",
                    ));
                }
            }
            ProviderLifecycleStatus::Failed => {
                if self.failure_category.is_none() {
                    return Err(RpcError::invalid_params(
                        "Failed preflight requires a stable failure category.",
                    ));
                }
            }
            ProviderLifecycleStatus::Cancelled => {
                return Err(RpcError::invalid_params(
                    "Preflight cannot have cancelled status because it performs no network execution.",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderCheckCommand {
    pub schema_version: String,
    pub execution_id: String,
    pub provider_id: String,
    pub timeout_ms: u32,
    pub cancellation_id: String,
    pub cancellation_token: String,
}

impl ProviderCheckCommand {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "provider_check.schema_version",
            &self.schema_version,
            PROVIDER_CHECK_COMMAND_SCHEMA_VERSION,
        )?;
        require_stable_id("provider_check.execution_id", &self.execution_id)?;
        require_stable_id("provider_check.provider_id", &self.provider_id)?;
        require_stable_id("provider_check.cancellation_id", &self.cancellation_id)?;
        require_stable_id(
            "provider_check.cancellation_token",
            &self.cancellation_token,
        )?;
        if self.timeout_ms == 0 || self.timeout_ms > MAX_PROVIDER_TIMEOUT_MS {
            return Err(RpcError::invalid_params(format!(
                "provider_check.timeout_ms must be between 1 and {MAX_PROVIDER_TIMEOUT_MS}."
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderCheckResult {
    pub schema_version: String,
    pub execution_id: String,
    pub provider_id: String,
    pub status: ProviderLifecycleStatus,
    pub network_call_made: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<ProviderUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_category: Option<ProviderFailureCategory>,
}

impl ProviderCheckResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "provider_check_result.schema_version",
            &self.schema_version,
            PROVIDER_CHECK_RESULT_SCHEMA_VERSION,
        )?;
        require_stable_id("provider_check_result.execution_id", &self.execution_id)?;
        require_stable_id("provider_check_result.provider_id", &self.provider_id)?;
        match self.status {
            ProviderLifecycleStatus::Ready => {
                if !self.network_call_made || self.failure_category.is_some() {
                    return Err(RpcError::invalid_params(
                        "Ready provider check requires a real network call and no failure.",
                    ));
                }
            }
            ProviderLifecycleStatus::Unconfigured => {
                if self.network_call_made || self.failure_category.is_some() || self.usage.is_some()
                {
                    return Err(RpcError::invalid_params(
                        "Unconfigured provider check must stop before network and usage.",
                    ));
                }
            }
            ProviderLifecycleStatus::Failed | ProviderLifecycleStatus::Cancelled => {
                if self.failure_category.is_none() {
                    return Err(RpcError::invalid_params(
                        "Failed or cancelled provider check requires a stable failure category.",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderCancelCommand {
    pub schema_version: String,
    pub execution_id: String,
    pub cancellation_id: String,
    pub cancellation_token: String,
}

impl ProviderCancelCommand {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "provider_cancel.schema_version",
            &self.schema_version,
            PROVIDER_CANCEL_COMMAND_SCHEMA_VERSION,
        )?;
        require_stable_id("provider_cancel.execution_id", &self.execution_id)?;
        require_stable_id("provider_cancel.cancellation_id", &self.cancellation_id)?;
        require_stable_id(
            "provider_cancel.cancellation_token",
            &self.cancellation_token,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderCancelResult {
    pub schema_version: String,
    pub execution_id: String,
    pub cancellation_id: String,
    pub accepted: bool,
    pub already_terminal: bool,
}

impl ProviderCancelResult {
    pub fn validate(&self) -> Result<(), RpcError> {
        require_schema(
            "provider_cancel_result.schema_version",
            &self.schema_version,
            PROVIDER_CANCEL_RESULT_SCHEMA_VERSION,
        )?;
        require_stable_id("provider_cancel_result.execution_id", &self.execution_id)?;
        require_stable_id(
            "provider_cancel_result.cancellation_id",
            &self.cancellation_id,
        )?;
        if self.accepted == self.already_terminal {
            return Err(RpcError::invalid_params(
                "Provider cancel result must be either accepted or already_terminal.",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_is_explicitly_network_free() {
        let result = ProviderPreflightResult {
            schema_version: PROVIDER_PREFLIGHT_RESULT_SCHEMA_VERSION.into(),
            execution_id: "provider_preflight_1".into(),
            status: ProviderLifecycleStatus::Unconfigured,
            provider_id: None,
            configured: false,
            network_call_made: false,
            failure_category: None,
        };
        result.validate().unwrap();

        let mut invalid = result;
        invalid.network_call_made = true;
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn provider_contract_rejects_secret_fields_and_bad_timeout() {
        let with_secret = serde_json::json!({
            "schema_version": "ProviderCheckCommand@1",
            "execution_id": "provider_check_1",
            "provider_id": "deepseek",
            "timeout_ms": 30000,
            "cancellation_id": "cancel_1",
            "cancellation_token": "token_1",
            "provider_key": "forbidden"
        });
        assert!(serde_json::from_value::<ProviderCheckCommand>(with_secret).is_err());

        let command = ProviderCheckCommand {
            schema_version: PROVIDER_CHECK_COMMAND_SCHEMA_VERSION.into(),
            execution_id: "provider_check_1".into(),
            provider_id: "deepseek".into(),
            timeout_ms: 0,
            cancellation_id: "cancel_1".into(),
            cancellation_token: "token_1".into(),
        };
        assert!(command.validate().is_err());
    }
}
