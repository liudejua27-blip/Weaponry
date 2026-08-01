//! Deterministic, bounded Context Builder for the native Agent runtime.
//!
//! Context is an explicit value with a reproducible digest. Credentials,
//! machine paths, and Provider reasoning are rejected before a request can be
//! handed to a [`crate::ProviderClient`].

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::canonical::{canonical_json, sha256_hex};

pub const CONTEXT_SCHEMA_VERSION: &str = "AgentContext@1";
pub const PROVIDER_CONVERSATION_ENVELOPE_SCHEMA_VERSION: &str = "ProviderConversationEnvelope@2";
pub const PROJECT_CONVERSATION_MEMORY_SCHEMA_VERSION: &str = "ProjectConversationMemory@1";
pub const PROMPT_PREFIX_RECEIPT_SCHEMA_VERSION: &str = "PromptPrefixReceipt@1";
pub const DEFAULT_PROVIDER_INPUT_TOKEN_BUDGET: usize = 48_000;
pub const MAX_PROVIDER_RECENT_TURNS: usize = 4;
pub const MAX_PROJECT_MEMORY_ITEMS: usize = 24;
pub const MAX_PROJECT_MEMORY_ITEM_CHARS: usize = 1_024;
pub const MAX_CONTEXT_MESSAGES: usize = 8;
// User design briefs may be substantially longer than a chat message. The
// Provider transport still applies a finite byte/request ceiling, while the
// Context builder no longer imposes a small product-facing text limit.
pub const MAX_CONTEXT_TEXT_CHARS: usize = 200_000;

/// Structured, product-owned memory. It contains visible decisions and
/// bounded state only; Provider hidden reasoning is deliberately not a field
/// of this contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectConversationMemory {
    pub schema_version: String,
    pub subject_identity_and_intent: Vec<String>,
    pub confirmed_visual_decisions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_asset_snapshot_digest: Option<String>,
    pub unresolved_questions: Vec<String>,
    pub rejected_choices_and_limitations: Vec<String>,
    pub revision: u64,
}

impl Default for ProjectConversationMemory {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_CONVERSATION_MEMORY_SCHEMA_VERSION.into(),
            subject_identity_and_intent: Vec::new(),
            confirmed_visual_decisions: Vec::new(),
            current_asset_snapshot_digest: None,
            unresolved_questions: Vec::new(),
            rejected_choices_and_limitations: Vec::new(),
            revision: 0,
        }
    }
}

impl ProjectConversationMemory {
    pub fn validate(&self) -> Result<(), ContextBuildError> {
        if self.schema_version != PROJECT_CONVERSATION_MEMORY_SCHEMA_VERSION
            || self.revision > 1_000_000
        {
            return Err(ContextBuildError::new(
                "AGENT_PROJECT_MEMORY_SCHEMA_INVALID",
                ContextBuildErrorKind::Serialization,
                "Project conversation memory is outside the reviewed schema.",
            ));
        }
        for (field, values) in [
            (
                "subject_identity_and_intent",
                &self.subject_identity_and_intent,
            ),
            (
                "confirmed_visual_decisions",
                &self.confirmed_visual_decisions,
            ),
            ("unresolved_questions", &self.unresolved_questions),
            (
                "rejected_choices_and_limitations",
                &self.rejected_choices_and_limitations,
            ),
        ] {
            if values.len() > MAX_PROJECT_MEMORY_ITEMS {
                return Err(ContextBuildError::new(
                    "AGENT_PROJECT_MEMORY_TOO_LARGE",
                    ContextBuildErrorKind::InvalidText,
                    "Project conversation memory has too many entries.",
                ));
            }
            for value in values {
                if value.chars().count() > MAX_PROJECT_MEMORY_ITEM_CHARS {
                    return Err(ContextBuildError::new(
                        "AGENT_PROJECT_MEMORY_ITEM_TOO_LARGE",
                        ContextBuildErrorKind::InvalidText,
                        "Project conversation memory contains an oversized entry.",
                    ));
                }
                validate_text(field, value, false)?;
            }
        }
        if let Some(digest) = &self.current_asset_snapshot_digest {
            if !valid_sha256(digest) {
                return Err(ContextBuildError::new(
                    "AGENT_PROJECT_MEMORY_SNAPSHOT_DIGEST_INVALID",
                    ContextBuildErrorKind::InvalidText,
                    "Project memory snapshot digest must be a lowercase SHA-256.",
                ));
            }
        }
        Ok(())
    }

    /// Advance memory only from visible user/assistant text. This helper is
    /// intentionally deterministic and never accepts Provider reasoning.
    pub fn after_completed_turn(
        &self,
        user_message: &str,
        assistant_message: &str,
        snapshot_digest: Option<String>,
    ) -> Result<Self, ContextBuildError> {
        self.validate()?;
        validate_text("user_message", user_message, false)?;
        validate_text("assistant_message", assistant_message, false)?;
        let mut next = self.clone();
        push_bounded_memory(&mut next.subject_identity_and_intent, user_message);
        push_bounded_memory(&mut next.confirmed_visual_decisions, assistant_message);
        next.current_asset_snapshot_digest = snapshot_digest;
        next.revision = next.revision.saturating_add(1);
        next.validate()?;
        Ok(next)
    }
}

fn push_bounded_memory(values: &mut Vec<String>, value: &str) {
    let bounded = value
        .chars()
        .take(MAX_PROJECT_MEMORY_ITEM_CHARS)
        .collect::<String>();
    if values.last() != Some(&bounded) {
        values.push(bounded);
    }
    if values.len() > MAX_PROJECT_MEMORY_ITEMS {
        let drop_count = values.len() - MAX_PROJECT_MEMORY_ITEMS;
        values.drain(0..drop_count);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderConversationTurn {
    pub messages: Vec<ContextMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StableProviderPrefix {
    pub schema_version: String,
    pub system_policy_version: String,
    pub forge_visual_program_schema_version: String,
    pub system_prompt: String,
    pub tool_definitions: Vec<ContextToolManifest>,
    pub capability_manifest_hash: String,
    pub provider_id: String,
    pub model: String,
    pub provider_behavior_flags: BTreeMap<String, String>,
    pub prefix_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProviderConversationEnvelope {
    pub schema_version: String,
    pub stable_prefix: StableProviderPrefix,
    pub project_memory: ProjectConversationMemory,
    pub recent_turns: Vec<ProviderConversationTurn>,
    pub current_turn: Vec<ContextMessage>,
    pub input_token_budget: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PromptPrefixReceipt {
    pub schema_version: String,
    pub prefix_hash: String,
    pub system_policy_version: String,
    pub tool_schema_hash: String,
    pub capability_manifest_hash: String,
    pub provider_model_config_hash: String,
    pub project_memory_hash: String,
    pub input_token_budget: u64,
    pub prompt_cache_hit_tokens: u64,
    pub prompt_cache_miss_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compaction_reason: Option<String>,
}

impl PromptPrefixReceipt {
    pub fn from_envelope(
        envelope: &ProviderConversationEnvelope,
        prompt_cache_hit_tokens: u64,
        prompt_cache_miss_tokens: u64,
    ) -> Self {
        let prefix = &envelope.stable_prefix;
        let tool_schema_hash = sha256_hex(
            canonical_json(&serde_json::to_value(&prefix.tool_definitions).unwrap_or(Value::Null))
                .as_bytes(),
        );
        let provider_model_config_hash = sha256_hex(
            canonical_json(&json!({
                "provider_id": prefix.provider_id,
                "model": prefix.model,
                "flags": prefix.provider_behavior_flags,
            }))
            .as_bytes(),
        );
        let project_memory_hash = sha256_hex(
            canonical_json(&serde_json::to_value(&envelope.project_memory).unwrap_or(Value::Null))
                .as_bytes(),
        );
        Self {
            schema_version: PROMPT_PREFIX_RECEIPT_SCHEMA_VERSION.into(),
            prefix_hash: prefix.prefix_hash.clone(),
            system_policy_version: prefix.system_policy_version.clone(),
            tool_schema_hash,
            capability_manifest_hash: prefix.capability_manifest_hash.clone(),
            provider_model_config_hash,
            project_memory_hash,
            input_token_budget: envelope.input_token_budget as u64,
            prompt_cache_hit_tokens,
            prompt_cache_miss_tokens,
            compaction_reason: envelope.compaction_reason.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConversationBuildInput {
    pub provider_id: String,
    pub model: String,
    pub system_policy_version: String,
    pub forge_visual_program_schema_version: String,
    pub capability_manifest_hash: String,
    pub provider_behavior_flags: BTreeMap<String, String>,
    pub project_memory: ProjectConversationMemory,
    pub current_turn: Vec<ContextMessage>,
    pub input_token_budget: usize,
}

impl ProviderConversationBuildInput {
    fn legacy(_input: &ContextBuildInput) -> Self {
        Self {
            provider_id: "legacy".into(),
            model: "legacy".into(),
            system_policy_version: CONTEXT_SCHEMA_VERSION.into(),
            forge_visual_program_schema_version: "ForgeVisualProgram@1".into(),
            capability_manifest_hash: "0".repeat(64),
            provider_behavior_flags: BTreeMap::new(),
            project_memory: ProjectConversationMemory::default(),
            current_turn: Vec::new(),
            input_token_budget: DEFAULT_PROVIDER_INPUT_TOKEN_BUDGET,
        }
    }
}

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
    pub provider_conversation: ProviderConversationEnvelope,
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
            .field(
                "provider_prefix_hash",
                &self.provider_conversation.stable_prefix.prefix_hash,
            )
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
        let conversation = ProviderConversationBuildInput::legacy(&input);
        self.build_with_provider_conversation(input, conversation)
    }

    pub fn build_with_provider_conversation(
        &self,
        input: ContextBuildInput,
        conversation_input: ProviderConversationBuildInput,
    ) -> Result<AgentContext, ContextBuildError> {
        validate_text("system_prompt", &input.system_prompt, false)?;
        validate_text("thread_summary", &input.thread_summary, true)?;
        if let Some(snapshot) = &input.active_snapshot {
            validate_safe_value(snapshot)?;
        }

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

        let mut memory = conversation_input.project_memory.clone();
        memory.validate()?;
        if memory == ProjectConversationMemory::default() && !input.thread_summary.is_empty() {
            push_bounded_memory(
                &mut memory.subject_identity_and_intent,
                &input.thread_summary,
            );
        }
        let recent_messages = input
            .recent_messages
            .into_iter()
            .map(|message| {
                validate_text("message.content", &message.content, false)?;
                validate_optional_identifier("message.name", message.name.as_deref())?;
                validate_optional_identifier(
                    "message.tool_call_id",
                    message.tool_call_id.as_deref(),
                )?;
                Ok(message)
            })
            .collect::<Result<Vec<_>, ContextBuildError>>()?;
        let current_turn = conversation_input
            .current_turn
            .clone()
            .into_iter()
            .map(|message| {
                validate_text("current_turn.content", &message.content, false)?;
                validate_optional_identifier("current_turn.name", message.name.as_deref())?;
                validate_optional_identifier(
                    "current_turn.tool_call_id",
                    message.tool_call_id.as_deref(),
                )?;
                Ok(message)
            })
            .collect::<Result<Vec<_>, ContextBuildError>>()?;
        let recent_turns = turnize_messages(recent_messages);
        let input_token_budget = conversation_input.input_token_budget;
        if input_token_budget == 0 || input_token_budget > 10_000_000 {
            return Err(ContextBuildError::new(
                "AGENT_CONTEXT_TOKEN_BUDGET_INVALID",
                ContextBuildErrorKind::InvalidText,
                "Provider input token budget is outside the reviewed bound.",
            ));
        }
        let stable_prefix =
            stable_prefix(&input.system_prompt, tools.clone(), &conversation_input)?;
        let (recent_turns, compaction_reason) = compact_turns(
            stable_prefix_token_estimate(&stable_prefix),
            &memory,
            recent_turns,
            &current_turn,
            input_token_budget,
        )?;
        let envelope = ProviderConversationEnvelope {
            schema_version: PROVIDER_CONVERSATION_ENVELOPE_SCHEMA_VERSION.into(),
            stable_prefix,
            project_memory: memory,
            recent_turns,
            current_turn,
            input_token_budget,
            compaction_reason,
        };
        let mut messages = envelope_messages(&envelope);
        if !input.thread_summary.is_empty()
            && !messages
                .iter()
                .any(|message| message.name.as_deref() == Some("thread_summary"))
        {
            messages.insert(
                1.min(messages.len()),
                ContextMessage {
                    role: ContextRole::System,
                    content: input.thread_summary,
                    name: Some("thread_summary".into()),
                    tool_call_id: None,
                },
            );
        }
        // Keep legacy callers' current message shape while the envelope is
        // the authoritative bounded representation for new Provider turns.
        for message in messages.iter_mut() {
            validate_text("message.content", &message.content, false)?;
        }

        let allowed_component_ids =
            sorted_unique_ids("allowed_component_ids", input.allowed_component_ids)?;
        let allowed_material_ids =
            sorted_unique_ids("allowed_material_ids", input.allowed_material_ids)?;

        let active_snapshot = input.active_snapshot;
        let digest_value = serde_json::json!({
            "schema_version": CONTEXT_SCHEMA_VERSION,
            "messages": messages,
            "provider_conversation": envelope,
            "active_snapshot": active_snapshot,
            "allowed_component_ids": allowed_component_ids,
            "allowed_material_ids": allowed_material_ids,
            "tools": tools,
        });
        let context_digest = sha256_hex(canonical_json(&digest_value).as_bytes());

        Ok(AgentContext {
            schema_version: CONTEXT_SCHEMA_VERSION.into(),
            messages,
            provider_conversation: envelope,
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

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn stable_prefix(
    system_prompt: &str,
    tool_definitions: Vec<ContextToolManifest>,
    input: &ProviderConversationBuildInput,
) -> Result<StableProviderPrefix, ContextBuildError> {
    validate_text("provider_id", &input.provider_id, false)?;
    validate_text("model", &input.model, false)?;
    validate_text("system_policy_version", &input.system_policy_version, false)?;
    validate_text(
        "forge_visual_program_schema_version",
        &input.forge_visual_program_schema_version,
        false,
    )?;
    if !valid_sha256(&input.capability_manifest_hash) {
        return Err(ContextBuildError::new(
            "AGENT_CONTEXT_CAPABILITY_HASH_INVALID",
            ContextBuildErrorKind::Serialization,
            "Capability manifest hash must be a lowercase SHA-256.",
        ));
    }
    for (key, value) in &input.provider_behavior_flags {
        validate_identifier("provider_behavior_flags.key", key)?;
        validate_text("provider_behavior_flags.value", value, false)?;
    }
    let mut prefix = StableProviderPrefix {
        schema_version: "StableProviderPrefix@1".into(),
        system_policy_version: input.system_policy_version.clone(),
        forge_visual_program_schema_version: input.forge_visual_program_schema_version.clone(),
        system_prompt: system_prompt.into(),
        tool_definitions,
        capability_manifest_hash: input.capability_manifest_hash.clone(),
        provider_id: input.provider_id.clone(),
        model: input.model.clone(),
        provider_behavior_flags: input.provider_behavior_flags.clone(),
        prefix_hash: String::new(),
    };
    let hash_value = serde_json::to_value(&prefix).map_err(|_| {
        ContextBuildError::new(
            "AGENT_CONTEXT_PREFIX_SERIALIZATION_FAILED",
            ContextBuildErrorKind::Serialization,
            "Stable Provider prefix could not be serialized.",
        )
    })?;
    prefix.prefix_hash = sha256_hex(canonical_json(&hash_value).as_bytes());
    Ok(prefix)
}

fn turnize_messages(messages: Vec<ContextMessage>) -> Vec<ProviderConversationTurn> {
    let mut turns = Vec::new();
    let mut current = Vec::new();
    for message in messages {
        if matches!(message.role, ContextRole::User) && !current.is_empty() {
            turns.push(ProviderConversationTurn { messages: current });
            current = Vec::new();
        }
        current.push(message);
    }
    if !current.is_empty() {
        turns.push(ProviderConversationTurn { messages: current });
    }
    turns
}

fn token_estimate(text: &str) -> usize {
    text.chars().count().saturating_add(3) / 4
}

fn message_token_estimate(message: &ContextMessage) -> usize {
    8usize
        .saturating_add(token_estimate(&message.content))
        .saturating_add(
            message
                .name
                .as_deref()
                .map(token_estimate)
                .unwrap_or_default(),
        )
        .saturating_add(
            message
                .tool_call_id
                .as_deref()
                .map(token_estimate)
                .unwrap_or_default(),
        )
}

fn turn_token_estimate(turn: &ProviderConversationTurn) -> usize {
    turn.messages.iter().map(message_token_estimate).sum()
}

fn stable_prefix_token_estimate(prefix: &StableProviderPrefix) -> usize {
    let value = serde_json::to_string(prefix).unwrap_or_default();
    token_estimate(&value).saturating_add(16)
}

fn memory_token_estimate(memory: &ProjectConversationMemory) -> usize {
    token_estimate(&serde_json::to_string(memory).unwrap_or_default()).saturating_add(16)
}

fn compact_turns(
    stable_prefix_tokens: usize,
    memory: &ProjectConversationMemory,
    turns: Vec<ProviderConversationTurn>,
    current_turn: &[ContextMessage],
    input_token_budget: usize,
) -> Result<(Vec<ProviderConversationTurn>, Option<String>), ContextBuildError> {
    let current_tokens = current_turn
        .iter()
        .map(message_token_estimate)
        .sum::<usize>();
    let fixed_tokens = stable_prefix_tokens
        .saturating_add(memory_token_estimate(memory))
        .saturating_add(current_tokens);
    if fixed_tokens > input_token_budget {
        return Err(ContextBuildError::new(
            "AGENT_CONTEXT_CURRENT_TURN_OVER_BUDGET",
            ContextBuildErrorKind::InvalidText,
            "Provider stable prefix, memory, and current turn exceed the input token budget.",
        ));
    }

    let mut kept_reversed = Vec::new();
    let mut used = fixed_tokens;
    let mut first_kept = turns.len();
    while first_kept > 0 && kept_reversed.len() < MAX_PROVIDER_RECENT_TURNS {
        let turn = &turns[first_kept - 1];
        let turn_tokens = turn_token_estimate(turn);
        if used.saturating_add(turn_tokens) > input_token_budget {
            break;
        }
        used = used.saturating_add(turn_tokens);
        kept_reversed.push(turn.clone());
        first_kept -= 1;
    }
    kept_reversed.reverse();
    let dropped = first_kept;
    let reason = if dropped > 0 {
        Some(format!(
            "deterministic_token_budget_compaction:dropped_turns={dropped};kept_turns={}",
            kept_reversed.len()
        ))
    } else {
        None
    };
    Ok((kept_reversed, reason))
}

fn envelope_messages(envelope: &ProviderConversationEnvelope) -> Vec<ContextMessage> {
    let mut messages = Vec::new();
    messages.push(ContextMessage {
        role: ContextRole::System,
        content: envelope.stable_prefix.system_prompt.clone(),
        name: Some("stable_prefix".into()),
        tool_call_id: None,
    });
    let memory_json = serde_json::to_string(&envelope.project_memory).unwrap_or_default();
    messages.push(ContextMessage {
        role: ContextRole::System,
        content: memory_json,
        name: Some("project_memory".into()),
        tool_call_id: None,
    });
    for turn in &envelope.recent_turns {
        messages.extend(turn.messages.clone());
    }
    messages.extend(envelope.current_turn.clone());
    messages
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
        assert_eq!(context.provider_conversation.recent_turns.len(), 4);
        assert!(context.messages.len() <= MAX_CONTEXT_MESSAGES + 2);

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

    fn provider_input(
        current_turn: Vec<ContextMessage>,
        input_token_budget: usize,
    ) -> ProviderConversationBuildInput {
        ProviderConversationBuildInput {
            provider_id: "deepseek".into(),
            model: "deepseek-chat".into(),
            system_policy_version: "ForgeCADNativeSystemPolicy@1".into(),
            forge_visual_program_schema_version: "ForgeVisualProgram@2".into(),
            capability_manifest_hash: "a".repeat(64),
            provider_behavior_flags: BTreeMap::from([(
                "thinking_tool_replay".into(),
                "ephemeral_reasoning_content".into(),
            )]),
            project_memory: ProjectConversationMemory::default(),
            current_turn,
            input_token_budget,
        }
    }

    #[test]
    fn stable_prefix_hash_is_independent_of_tool_order_and_json_key_order() {
        let builder = ContextBuilder;
        let mut first = input();
        first.tools.push(ContextToolManifest {
            name: "inspect_asset".into(),
            description: "Inspect the active asset.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"asset_id": {"type": "string"}},
                "required": ["asset_id"]
            }),
        });
        let first_context = builder
            .build_with_provider_conversation(
                first.clone(),
                provider_input(Vec::new(), DEFAULT_PROVIDER_INPUT_TOKEN_BUDGET),
            )
            .unwrap();
        first.tools.reverse();
        first.tools[0].input_schema = json!({
            "required": ["asset_id"],
            "properties": {"asset_id": {"type": "string"}},
            "type": "object"
        });
        let second_context = builder
            .build_with_provider_conversation(
                first,
                provider_input(Vec::new(), DEFAULT_PROVIDER_INPUT_TOKEN_BUDGET),
            )
            .unwrap();
        assert_eq!(
            first_context
                .provider_conversation
                .stable_prefix
                .prefix_hash,
            second_context
                .provider_conversation
                .stable_prefix
                .prefix_hash
        );
    }

    #[test]
    fn token_budget_compaction_is_deterministic_and_preserves_current_turn() {
        let builder = ContextBuilder;
        let mut bounded = input();
        bounded.thread_summary.clear();
        bounded.recent_messages = (0..6)
            .flat_map(|index| {
                [
                    ContextMessage {
                        role: ContextRole::User,
                        content: format!("user turn {index} with visible project intent"),
                        name: None,
                        tool_call_id: None,
                    },
                    ContextMessage {
                        role: ContextRole::Assistant,
                        content: format!("assistant decision {index}"),
                        name: None,
                        tool_call_id: None,
                    },
                ]
            })
            .collect();
        let conversation = provider_input(
            vec![ContextMessage {
                role: ContextRole::User,
                content: "current turn must survive compaction".into(),
                name: None,
                tool_call_id: None,
            }],
            600,
        );
        let first = builder
            .build_with_provider_conversation(bounded.clone(), conversation.clone())
            .unwrap();
        let second = builder
            .build_with_provider_conversation(bounded, conversation)
            .unwrap();
        assert_eq!(first.provider_conversation, second.provider_conversation);
        assert_eq!(first.provider_conversation.recent_turns.len(), 4);
        assert_eq!(
            first.provider_conversation.current_turn[0].content,
            "current turn must survive compaction"
        );
        assert_eq!(
            first.provider_conversation.compaction_reason.as_deref(),
            Some("deterministic_token_budget_compaction:dropped_turns=2;kept_turns=4")
        );
    }

    #[test]
    fn memory_and_prefix_receipt_are_visible_and_reasoning_free() {
        let builder = ContextBuilder;
        let context = builder
            .build_with_provider_conversation(
                input(),
                provider_input(
                    vec![ContextMessage {
                        role: ContextRole::User,
                        content: "remember this confirmed intent".into(),
                        name: None,
                        tool_call_id: None,
                    }],
                    DEFAULT_PROVIDER_INPUT_TOKEN_BUDGET,
                ),
            )
            .unwrap();
        let memory = context
            .provider_conversation
            .project_memory
            .after_completed_turn("remember this confirmed intent", "visible answer", None)
            .unwrap();
        let mut next = context.provider_conversation.clone();
        next.project_memory = memory;
        let receipt = PromptPrefixReceipt::from_envelope(&next, 64, 0);
        let serialized =
            serde_json::to_string(&json!({"memory": next.project_memory, "receipt": receipt}))
                .unwrap();
        assert!(serialized.contains(PROJECT_CONVERSATION_MEMORY_SCHEMA_VERSION));
        assert!(serialized.contains(PROMPT_PREFIX_RECEIPT_SCHEMA_VERSION));
        assert!(serialized.contains("prompt_cache_hit_tokens"));
        assert!(!serialized.contains("reasoning_content"));
        assert!(!serialized.contains("private chain of thought"));
    }
}
