//! Redacted Action Loop evidence.
//!
//! Trace entries intentionally carry identities, timing, counts, categories,
//! and content digests only. Prompts, Provider output, reasoning, credentials,
//! Product Tool arguments, and Product Tool results are structurally absent.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    canonical::{canonical_json, sha256_hex},
    ProviderErrorCategory,
};

pub const REDACTED_TRACE_SCHEMA_VERSION: &str = "RedactedAgentTrace@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TracePhase {
    Context,
    Provider,
    ProductTool,
    Budget,
    Cancellation,
    Final,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraceEventKind {
    Started,
    Completed,
    Failed,
    Rejected,
    Cancelled,
    BudgetExceeded,
    LateResultIgnored,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RedactedTraceEntry {
    pub sequence: u64,
    pub phase: TracePhase,
    pub event: TraceEventKind,
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_failure_category: Option<ProviderErrorCategory>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_microusd: u64,
    pub network_call_made: bool,
}

impl RedactedTraceEntry {
    pub fn new(phase: TracePhase, event: TraceEventKind, elapsed_ms: u64) -> Self {
        Self {
            sequence: 0,
            phase,
            event,
            elapsed_ms,
            call_id: None,
            tool_name: None,
            input_sha256: None,
            output_sha256: None,
            error_code: None,
            provider_failure_category: None,
            input_tokens: 0,
            output_tokens: 0,
            estimated_cost_microusd: 0,
            network_call_made: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RedactedExecutionTrace {
    pub schema_version: String,
    pub execution_id: String,
    pub context_digest: String,
    pub entries: Vec<RedactedTraceEntry>,
}

impl RedactedExecutionTrace {
    pub fn new(execution_id: impl Into<String>, context_digest: impl Into<String>) -> Self {
        Self {
            schema_version: REDACTED_TRACE_SCHEMA_VERSION.into(),
            execution_id: execution_id.into(),
            context_digest: context_digest.into(),
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, mut entry: RedactedTraceEntry) {
        entry.sequence = self.entries.len() as u64 + 1;
        self.entries.push(entry);
    }

    pub fn digest_value(value: &Value) -> String {
        sha256_hex(canonical_json(value).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn serialized_trace_cannot_contain_sensitive_runtime_content() {
        let secret_input = json!({
            "api_key": "sk-do-not-store",
            "reasoning_content": "private chain",
            "arguments": {"instruction": "confidential prompt"}
        });
        let secret_output = json!({"mesh": "private product output"});
        let mut trace = RedactedExecutionTrace::new("execution_1", "a".repeat(64));
        let mut entry =
            RedactedTraceEntry::new(TracePhase::ProductTool, TraceEventKind::Completed, 14);
        entry.tool_name = Some("compile_readback_candidate".into());
        entry.input_sha256 = Some(RedactedExecutionTrace::digest_value(&secret_input));
        entry.output_sha256 = Some(RedactedExecutionTrace::digest_value(&secret_output));
        trace.push(entry);

        let serialized = serde_json::to_string(&trace).unwrap();
        for forbidden in [
            "sk-do-not-store",
            "private chain",
            "confidential prompt",
            "private product output",
            "reasoning_content",
            "arguments",
        ] {
            assert!(!serialized.contains(forbidden));
        }
        assert!(serialized.contains("input_sha256"));
        assert!(serialized.contains("output_sha256"));
    }
}
