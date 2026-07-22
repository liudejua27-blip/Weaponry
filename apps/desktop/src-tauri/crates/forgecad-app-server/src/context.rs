//! Deterministic, bounded Context Builder for the native Agent runtime.
//!
//! Context is an explicit value with a reproducible digest. Credentials,
//! machine paths, and Provider reasoning are rejected before a request can be
//! handed to a [`crate::ProviderClient`].

use std::{collections::BTreeSet, fmt};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::{canonical_json, sha256_hex};

pub const CONTEXT_SCHEMA_VERSION: &str = "AgentContext@1";
pub const MAX_CONTEXT_MESSAGES: usize = 8;
pub const MAX_CONTEXT_TEXT_CHARS: usize = 16_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextMessage {
    pub role: ContextRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl fmt::Debug for ContextMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContextMessage")
            .field("role", &self.role)
            .field("content", &"[REDACTED]")
            .field("name", &self.name)
            .field("tool_call_id", &self.tool_call_id)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContextToolManifest {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl fmt::Debug for ContextToolManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContextToolManifest")
            .field("name", &self.name)
            .field("description", &"[REDACTED]")
            .field("input_schema", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContextBuildInput {
    pub system_prompt: String,
    #[serde(default)]
    pub thread_summary: String,
    #[serde(default)]
    pub recent_messages: Vec<ContextMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_snapshot: Option<Value>,
    #[serde(default)]
    pub allowed_component_ids: Vec<String>,
    #[serde(default)]
    pub allowed_material_ids: Vec<String>,
    #[serde(default)]
    pub tools: Vec<ContextToolManifest>,
}

impl fmt::Debug for ContextBuildInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContextBuildInput")
            .field("system_prompt", &"[REDACTED]")
            .field("thread_summary", &"[REDACTED]")
            .field("recent_message_count", &self.recent_messages.len())
            .field("has_active_snapshot", &self.active_snapshot.is_some())
            .field("allowed_component_count", &self.allowed_component_ids.len())
            .field("allowed_material_count", &self.allowed_material_ids.len())
            .field("tool_count", &self.tools.len())
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentContext {
    pub schema_version: String,
    pub messages: Vec<ContextMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_snapshot: Option<Value>,
    pub allowed_component_ids: Vec<String>,
    pub allowed_material_ids: Vec<String>,
    pub tools: Vec<ContextToolManifest>,
    pub context_digest: String,
}

impl fmt::Debug for AgentContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentContext")
            .field("schema_version", &self.schema_version)
            .field("message_count", &self.messages.len())
            .field("has_active_snapshot", &self.active_snapshot.is_some())
            .field("allowed_component_count", &self.allowed_component_ids.len())
            .field("allowed_material_count", &self.allowed_material_ids.len())
            .field("tool_count", &self.tools.len())
            .field("context_digest", &self.context_digest)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextBuildErrorKind {
    InvalidText,
    DuplicateIdentifier,
    SensitiveField,
    MachinePath,
    Serialization,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextBuildError {
    pub code: String,
    pub kind: ContextBuildErrorKind,
    pub message: String,
}

impl ContextBuildError {
    fn new(code: &str, kind: ContextBuildErrorKind, message: &str) -> Self {
        Self {
            code: code.into(),
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build(&self, input: ContextBuildInput) -> Result<AgentContext, ContextBuildError> {
        validate_text("system_prompt", &input.system_prompt, false)?;
        validate_text("thread_summary", &input.thread_summary, true)?;
        if let Some(snapshot) = &input.active_snapshot {
            validate_safe_value(snapshot)?;
        }

        let mut messages = Vec::with_capacity(MAX_CONTEXT_MESSAGES + 2);
        messages.push(ContextMessage {
            role: ContextRole::System,
            content: input.system_prompt,
            name: None,
            tool_call_id: None,
        });
        if !input.thread_summary.is_empty() {
            messages.push(ContextMessage {
                role: ContextRole::System,
                content: input.thread_summary,
                name: Some("thread_summary".into()),
                tool_call_id: None,
            });
        }
        let recent_start = input
            .recent_messages
            .len()
            .saturating_sub(MAX_CONTEXT_MESSAGES);
        for message in input.recent_messages.into_iter().skip(recent_start) {
            validate_text("message.content", &message.content, false)?;
            validate_optional_identifier("message.name", message.name.as_deref())?;
            validate_optional_identifier("message.tool_call_id", message.tool_call_id.as_deref())?;
            messages.push(message);
        }

        let allowed_component_ids =
            sorted_unique_ids("allowed_component_ids", input.allowed_component_ids)?;
        let allowed_material_ids =
            sorted_unique_ids("allowed_material_ids", input.allowed_material_ids)?;

        let mut tools = input.tools;
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        let mut tool_names = BTreeSet::new();
        for tool in &tools {
            validate_identifier("tool.name", &tool.name)?;
            validate_text("tool.description", &tool.description, false)?;
            validate_safe_value(&tool.input_schema)?;
            if !tool_names.insert(tool.name.as_str()) {
                return Err(ContextBuildError::new(
                    "AGENT_CONTEXT_DUPLICATE_TOOL",
                    ContextBuildErrorKind::DuplicateIdentifier,
                    "Context tool names must be unique.",
                ));
            }
        }

        let active_snapshot = input.active_snapshot;
        let digest_value = serde_json::json!({
            "schema_version": CONTEXT_SCHEMA_VERSION,
            "messages": messages,
            "active_snapshot": active_snapshot,
            "allowed_component_ids": allowed_component_ids,
            "allowed_material_ids": allowed_material_ids,
            "tools": tools,
        });
        let context_digest = sha256_hex(canonical_json(&digest_value).as_bytes());

        Ok(AgentContext {
            schema_version: CONTEXT_SCHEMA_VERSION.into(),
            messages,
            active_snapshot,
            allowed_component_ids,
            allowed_material_ids,
            tools,
            context_digest,
        })
    }
}

fn sorted_unique_ids(field: &str, mut ids: Vec<String>) -> Result<Vec<String>, ContextBuildError> {
    ids.sort();
    for id in &ids {
        validate_identifier(field, id)?;
    }
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ContextBuildError::new(
            "AGENT_CONTEXT_DUPLICATE_IDENTIFIER",
            ContextBuildErrorKind::DuplicateIdentifier,
            "Context allow-list identifiers must be unique.",
        ));
    }
    Ok(ids)
}

fn validate_optional_identifier(field: &str, value: Option<&str>) -> Result<(), ContextBuildError> {
    if let Some(value) = value {
        validate_identifier(field, value)?;
    }
    Ok(())
}

fn validate_identifier(field: &str, value: &str) -> Result<(), ContextBuildError> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(ContextBuildError::new(
            "AGENT_CONTEXT_IDENTIFIER_INVALID",
            ContextBuildErrorKind::InvalidText,
            &format!("{field} is not a bounded stable identifier."),
        ));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, allow_empty: bool) -> Result<(), ContextBuildError> {
    let chars = value.chars().count();
    if (!allow_empty && chars == 0)
        || chars > MAX_CONTEXT_TEXT_CHARS
        || value.chars().any(|character| character == '\0')
    {
        return Err(ContextBuildError::new(
            "AGENT_CONTEXT_TEXT_INVALID",
            ContextBuildErrorKind::InvalidText,
            &format!("{field} is outside the bounded Context contract."),
        ));
    }
    if looks_like_machine_path(value) {
        return Err(ContextBuildError::new(
            "AGENT_CONTEXT_MACHINE_PATH_FORBIDDEN",
            ContextBuildErrorKind::MachinePath,
            "Machine-local paths cannot enter Provider context.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_safe_value(value: &Value) -> Result<(), ContextBuildError> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let normalized = key.to_ascii_lowercase();
                if is_sensitive_key(&normalized) {
                    return Err(ContextBuildError::new(
                        "AGENT_CONTEXT_SENSITIVE_FIELD_FORBIDDEN",
                        ContextBuildErrorKind::SensitiveField,
                        "Credential, endpoint, or Provider reasoning fields cannot enter Context.",
                    ));
                }
                validate_safe_value(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_safe_value(child)?;
            }
        }
        Value::String(value) if looks_like_machine_path(value) => {
            return Err(ContextBuildError::new(
                "AGENT_CONTEXT_MACHINE_PATH_FORBIDDEN",
                ContextBuildErrorKind::MachinePath,
                "Machine-local paths cannot enter Provider context.",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key,
        "api_key"
            | "apikey"
            | "authorization"
            | "password"
            | "secret"
            | "access_token"
            | "refresh_token"
            | "base_url"
            | "endpoint_url"
            | "reasoning_content"
    ) || key.ends_with("_api_key")
        || key.ends_with("_password")
        || key.ends_with("_secret")
}

fn looks_like_machine_path(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("/Users/")
        || trimmed.starts_with("/home/")
        || trimmed.starts_with("/private/")
        || trimmed.starts_with("file://")
        || (trimmed.len() > 3
            && trimmed.as_bytes()[1] == b':'
            && matches!(trimmed.as_bytes()[2], b'\\' | b'/'))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn input() -> ContextBuildInput {
        ContextBuildInput {
            system_prompt: "只设计非功能性的生产级机械概念外观。".into(),
            thread_summary: "用户要求连续曲面、PBR 和可编辑组件。".into(),
            recent_messages: vec![ContextMessage {
                role: ContextRole::User,
                content: "继续细化侧面视觉层级。".into(),
                name: None,
                tool_call_id: None,
            }],
            active_snapshot: Some(json!({"snapshot_id": "snapshot_1", "version_id": "v1"})),
            allowed_component_ids: vec!["vent_array".into(), "body_shell".into()],
            allowed_material_ids: vec!["anodized_metal".into(), "polymer_dark".into()],
            tools: vec![ContextToolManifest {
                name: "author_shape_program".into(),
                description: "Author a restricted ShapeProgram candidate.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {"candidate_id": {"type": "string"}},
                    "required": ["candidate_id"],
                    "additionalProperties": false
                }),
            }],
        }
    }

    #[test]
    fn digest_is_stable_for_semantically_unordered_inputs() {
        let builder = ContextBuilder;
        let first = builder.build(input()).unwrap();
        let mut reordered = input();
        reordered.allowed_component_ids.reverse();
        reordered.allowed_material_ids.reverse();
        reordered.tools[0].input_schema = json!({
            "additionalProperties": false,
            "required": ["candidate_id"],
            "properties": {"candidate_id": {"type": "string"}},
            "type": "object"
        });
        let second = builder.build(reordered).unwrap();
        assert_eq!(first.context_digest, second.context_digest);
    }

    #[test]
    fn digest_changes_when_active_design_context_changes() {
        let builder = ContextBuilder;
        let first = builder.build(input()).unwrap();
        let mut changed = input();
        changed.active_snapshot = Some(json!({"snapshot_id": "snapshot_2", "version_id": "v2"}));
        assert_ne!(
            first.context_digest,
            builder.build(changed).unwrap().context_digest
        );
    }

    #[test]
    fn context_is_bounded_and_rejects_secrets_paths_and_reasoning() {
        let builder = ContextBuilder;
        let mut bounded = input();
        bounded.recent_messages = (0..12)
            .map(|index| ContextMessage {
                role: ContextRole::User,
                content: format!("message {index}"),
                name: None,
                tool_call_id: None,
            })
            .collect();
        let context = builder.build(bounded).unwrap();
        assert_eq!(context.messages.len(), MAX_CONTEXT_MESSAGES + 2);

        for forbidden in [
            json!({"api_key": "sk-test"}),
            json!({"reasoning_content": "hidden"}),
            json!({"input": "/Users/person/private/model.glb"}),
        ] {
            let mut rejected = input();
            rejected.active_snapshot = Some(forbidden);
            assert!(builder.build(rejected).is_err());
        }
    }

    #[test]
    fn context_debug_is_structurally_redacted() {
        let mut sensitive = input();
        sensitive.system_prompt = "prompt-debug-sentinel".into();
        sensitive.thread_summary = "summary-debug-sentinel".into();
        sensitive.recent_messages[0].content = "message-debug-sentinel".into();
        sensitive.active_snapshot = Some(json!({"note": "snapshot-debug-sentinel"}));
        sensitive.allowed_component_ids = vec!["component_debug_sentinel".into()];
        sensitive.allowed_material_ids = vec!["material_debug_sentinel".into()];
        sensitive.tools[0].description = "tool-description-debug-sentinel".into();
        sensitive.tools[0].input_schema = json!({
            "type": "object",
            "description": "tool-schema-debug-sentinel"
        });

        let input_debug = format!("{sensitive:?}");
        let context = ContextBuilder.build(sensitive).unwrap();
        let context_debug = format!("{context:?}");
        let message_debug = format!("{:?}", context.messages.last().unwrap());
        let tool_debug = format!("{:?}", context.tools.last().unwrap());
        let all_debug = format!("{input_debug}\n{context_debug}\n{message_debug}\n{tool_debug}");
        for forbidden in [
            "prompt-debug-sentinel",
            "summary-debug-sentinel",
            "message-debug-sentinel",
            "snapshot-debug-sentinel",
            "component_debug_sentinel",
            "material_debug_sentinel",
            "tool-description-debug-sentinel",
            "tool-schema-debug-sentinel",
        ] {
            assert!(!all_debug.contains(forbidden));
        }
        assert!(all_debug.contains("[REDACTED]"));
        assert!(context_debug.contains(&context.context_digest));
    }
}
