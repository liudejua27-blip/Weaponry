use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_SET: &str = "forgecad-runtime-contracts@1";
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
/// The canonical MCP revision for ForgeCAD. Codex currently opens configured
/// stdio servers with the 2025-06-18 legacy revision, so that revision is an
/// explicit compatibility surface rather than an implicit downgrade.
pub const MCP_PROTOCOL_COMPAT_VERSION: &str = "2025-06-18";
pub const MCP_PROTOCOL_VERSIONS: &[&str] = &[MCP_PROTOCOL_VERSION, MCP_PROTOCOL_COMPAT_VERSION];

pub fn supports_mcp_protocol(version: &str) -> bool {
    MCP_PROTOCOL_VERSIONS.contains(&version)
}

pub fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Identifies one local development build cohort without exposing a path,
/// username, source file or secret. Release and ordinary test builds may omit
/// it; the MCP010A development packager always supplies a canonical SHA-256 to
/// every Rust component in the same build invocation.
pub fn build_cohort_sha256() -> Option<String> {
    option_env!("FORGECAD_BUILD_COHORT_SHA256")
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
}

pub fn is_opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCapabilities {
    pub contract_set: String,
    pub runtime_version: String,
    pub build_cohort_sha256: Option<String>,
    pub status: String,
    pub mcp_transport: String,
    pub ipc_transport: String,
    pub write_model: String,
    pub supports_reference_import: bool,
    pub supports_skill_registry: bool,
    pub supports_snapshot_read: bool,
    pub supports_job_read: bool,
    pub supports_cas: bool,
    pub supports_authenticated_ipc: bool,
    pub supports_resource_read: bool,
    pub supports_geometry_execution: bool,
    pub supports_render_execution: bool,
    pub operator_catalog_sha256: Option<String>,
    pub contract_versions: Vec<String>,
    pub mcp_protocol_versions: Vec<String>,
    pub resource_uris: Vec<String>,
    pub tool_manifest_hash: Option<String>,
    pub limitations: Vec<String>,
}

impl Default for RuntimeCapabilities {
    fn default() -> Self {
        Self {
            contract_set: CONTRACT_SET.to_owned(),
            runtime_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_cohort_sha256: build_cohort_sha256(),
            status: "alpha-mcp004".to_owned(),
            mcp_transport: "stdio-json-rpc".to_owned(),
            ipc_transport: "authenticated-local".to_owned(),
            write_model: "single-writer-preview-confirm".to_owned(),
            supports_reference_import: false,
            supports_skill_registry: false,
            supports_snapshot_read: true,
            supports_job_read: true,
            supports_cas: true,
            supports_authenticated_ipc: true,
            supports_resource_read: true,
            supports_geometry_execution: false,
            supports_render_execution: false,
            operator_catalog_sha256: None,
            contract_versions: vec![CONTRACT_SET.to_owned()],
            mcp_protocol_versions: MCP_PROTOCOL_VERSIONS
                .iter()
                .map(|version| (*version).to_owned())
                .collect(),
            resource_uris: vec![
                "forgecad://capabilities".to_owned(),
                "forgecad://projects/{project_id}/snapshot".to_owned(),
                "forgecad://projects/{project_id}/selection".to_owned(),
                "forgecad://candidates/{candidate_id}".to_owned(),
                "forgecad://jobs/{job_id}".to_owned(),
                "forgecad://versions/{version_id}".to_owned(),
            ],
            tool_manifest_hash: None,
            limitations: vec![
                "MCP003 stdio remains read-only; MCP004 candidate, restore and path-free diagnostic export transactions are restricted to authenticated Runtime IPC until reference, geometry, render and quality adapters are enabled.".to_owned(),
                "Codex is the only supported external agent entry; no model SDK is bundled.".to_owned(),
                "Reference images, geometry and render workers remain capability-gated until their validators ship.".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project_id: String,
    pub name: String,
    pub updated_at: String,
    pub head_snapshot_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub schema_version: String,
    pub project_id: String,
    pub name: String,
    pub policy: Value,
    pub created_at: String,
    pub updated_at: String,
    pub active_snapshot_revision: i64,
    pub head_snapshot_id: Option<String>,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSummary {
    pub snapshot_id: String,
    pub project_id: String,
    pub parent_snapshot_id: Option<String>,
    pub status: String,
    pub manifest_hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotRecord {
    pub schema_version: String,
    pub snapshot_id: String,
    pub project_id: String,
    pub parent_snapshot_id: Option<String>,
    pub candidate_id: Option<String>,
    pub revision: i64,
    pub status: String,
    pub manifest_hash: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRecord {
    pub schema_version: String,
    pub candidate_id: String,
    pub project_id: String,
    pub base_version_id: Option<String>,
    pub source_version_id: Option<String>,
    pub prepared_object_id: Option<String>,
    pub prepared_object_sha256: Option<String>,
    pub state: String,
    pub request_sha256: String,
    pub manifest_hash: Option<String>,
    pub quality_report_id: Option<String>,
    pub quality_hard_gate_passed: bool,
    pub canonical_sha256: String,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Durable, V2-only evidence that binds a reviewable geometry candidate to
/// the exact typed program, strict readback and quality objects used at
/// confirmation time.  It intentionally lives beside `Candidate@1` rather
/// than changing the historical candidate contract in place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryCandidateEvidenceRecord {
    pub schema_version: String,
    pub candidate_id: String,
    pub project_id: String,
    pub reference_id: Option<String>,
    pub reference_sha256: Option<String>,
    pub geometry_program_sha256: String,
    pub geometry_program_object_sha256: String,
    pub operator_catalog_sha256: String,
    pub readback_config_sha256: String,
    pub artifact_object_sha256: String,
    pub artifact_readback_object_sha256: String,
    pub quality_report_object_sha256: String,
    pub quality_report_id: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignAssetVersionRecord {
    pub schema_version: String,
    pub version_id: String,
    pub project_id: String,
    pub parent_version_id: Option<String>,
    pub candidate_id: String,
    pub manifest_hash: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub job_id: String,
    pub project_id: String,
    pub kind: String,
    pub status: String,
    pub progress: u8,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub schema_version: String,
    pub job_id: String,
    pub project_id: String,
    pub kind: String,
    pub status: String,
    pub progress: u8,
    pub request_sha256: String,
    pub checkpoint_sha256: Option<String>,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEventRecord {
    pub schema_version: String,
    pub job_id: String,
    pub sequence: i64,
    pub kind: String,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CasObjectRecord {
    pub schema_version: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub mime: String,
    pub kind: String,
    pub reachability: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceAuthorization {
    pub user_authorized: bool,
    pub declaration: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReferenceImportSource {
    InlineContent {
        mime: String,
        content_base64: String,
    },
    CodexLocalFile {
        path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceImportRequest {
    pub project_id: String,
    pub source: ReferenceImportSource,
    pub authorization: ReferenceAuthorization,
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceEvidenceRecord {
    pub schema_version: String,
    pub reference_id: String,
    pub project_id: String,
    pub object_sha256: String,
    pub mime: String,
    pub size_bytes: u64,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    pub import_mode: String,
    pub authorization: ReferenceAuthorization,
    pub derived_object_sha256: Option<String>,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceImportResult {
    pub schema_version: String,
    pub reference: ReferenceEvidenceRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceGetResult {
    pub schema_version: String,
    pub reference: ReferenceEvidenceRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillExecutionAvailability {
    /// Every operator in the immutable Bundle lock has a semantic,
    /// product-owned executor in the current Runtime/Worker cohort.
    Active,
    /// At least one, but not all, locked operators have a real executor.
    Partial,
    /// None of the locked operators has a real executor in this cohort.
    Unavailable,
}

impl Default for SkillExecutionAvailability {
    fn default() -> Self {
        // A historical `SkillBundleManifest@1` is declarative metadata.  Its
        // lack of this runtime overlay must never be read as executable.
        Self::Unavailable
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillBundleManifestRecord {
    pub schema_version: String,
    pub skill_id: String,
    pub version: String,
    pub status: String,
    pub publisher: String,
    pub contract_range: String,
    pub input_schema: String,
    pub output_schema: String,
    pub recipe: String,
    pub operator_ids: Vec<String>,
    pub validator_ids: Vec<String>,
    pub capabilities: Value,
    pub budgets: Value,
    pub benchmark_suite: String,
    pub canonical_sha256: String,
    pub trust_profile: String,
    pub signature: String,
    /// Runtime-derived availability. It is deliberately outside the Bundle
    /// canonical hash so a signed/declarative manifest retains its identity
    /// across Runtime cohorts.
    #[serde(default)]
    pub execution_availability: SkillExecutionAvailability,
    /// Locked operator IDs that are not semantically executable by this
    /// Runtime/Worker cohort. It is empty only when availability is `active`.
    #[serde(default)]
    pub missing_operator_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillListResult {
    pub schema_version: String,
    pub skills: Vec<SkillBundleManifestRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillKnowledgeRecord {
    pub schema_version: String,
    pub overview: String,
    pub constraints: String,
    pub examples: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillGetResult {
    pub schema_version: String,
    pub skill: SkillBundleManifestRecord,
    pub knowledge: SkillKnowledgeRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionReceiptRecord {
    pub schema_version: String,
    pub receipt_id: String,
    pub skill_id: String,
    pub skill_version: String,
    pub input_sha256: String,
    pub output_sha256: Option<String>,
    pub status: String,
    pub validator_ids: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEvalReportRecord {
    pub schema_version: String,
    pub report_id: String,
    pub skill_id: String,
    pub skill_version: String,
    pub suite_id: String,
    pub status: String,
    pub metrics: Value,
    pub evidence_sha256: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventRecord {
    pub schema_version: String,
    pub audit_id: String,
    pub project_id: Option<String>,
    pub kind: String,
    pub object_id: Option<String>,
    pub request_sha256: Option<String>,
    pub payload: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalReceiptRecord {
    pub schema_version: String,
    pub approval_receipt_id: String,
    pub project_id: String,
    pub tool: String,
    pub base_version_id: Option<String>,
    pub prepared_object_id: String,
    pub prepared_object_sha256: String,
    pub quality_report_id: Option<String>,
    pub summary_sha256: String,
    pub decision: String,
    pub expires_at: String,
    pub session_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateConfirmRequest {
    pub project_id: String,
    pub candidate_id: String,
    pub base_version_id: Option<String>,
    pub prepared_object_id: String,
    pub prepared_object_sha256: String,
    pub quality_report_id: String,
    pub approval_receipt_id: String,
    pub approval_summary: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub idempotency_key: String,
}

/// Explicit approval envelope for promoting a multi-view proposal.  The
/// legacy CandidateConfirmRequest intentionally cannot consume a
/// CrossViewEvidenceBundle; this request keeps the session/canvas/bundle
/// binding visible at the transaction boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossViewPromotionRequest {
    pub project_id: String,
    pub session_id: String,
    pub source_candidate_id: String,
    pub candidate_id: String,
    pub bundle_sha256: String,
    pub base_version_id: Option<String>,
    pub prepared_object_id: String,
    pub prepared_object_sha256: String,
    pub quality_report_id: String,
    pub approved: bool,
    pub approval_receipt_id: String,
    pub approval_summary: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRejectRequest {
    pub project_id: String,
    pub candidate_id: String,
    pub approval_receipt_id: String,
    pub approval_summary: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidatePrepareResult {
    pub schema_version: String,
    pub candidate: CandidateRecord,
    pub job: JobSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateConfirmResult {
    pub schema_version: String,
    pub candidate_id: String,
    pub project_id: String,
    pub version_id: String,
    pub snapshot_id: String,
    pub approval_receipt_id: String,
    pub request_sha256: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossViewPromotionResult {
    pub schema_version: String,
    pub project_id: String,
    pub session_id: String,
    pub source_candidate_id: String,
    pub candidate_id: String,
    pub bundle_sha256: String,
    pub version_id: String,
    pub snapshot_id: String,
    pub approval_receipt_id: String,
    pub request_sha256: String,
    pub replayed: bool,
}

/// Explicit approval envelope for consuming a Runtime-owned RepairApplyIntent
/// and confirming its already-validated single-view proposal candidate.
/// Multi-view intents remain bound to CrossViewPromotionRequest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairApplyConfirmRequest {
    pub project_id: String,
    pub session_id: String,
    pub candidate_id: String,
    pub proposal_candidate_id: String,
    pub run_id: String,
    pub apply_intent_object_sha256: String,
    pub apply_intent_canonical_sha256: String,
    pub approved: bool,
    pub approval_receipt_id: String,
    pub approval_summary: String,
    pub approval_expires_at: String,
    pub approval_session_id: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairApplyConfirmResult {
    pub schema_version: String,
    pub project_id: String,
    pub session_id: String,
    pub candidate_id: String,
    pub source_candidate_id: String,
    pub proposal_candidate_id: String,
    pub run_id: String,
    pub apply_intent_object_sha256: String,
    pub apply_intent_canonical_sha256: String,
    pub version_id: String,
    pub snapshot_id: String,
    pub approval_receipt_id: String,
    pub request_sha256: String,
    pub source_candidate_unchanged: bool,
    pub proposal_candidate_confirmed: bool,
    pub active_design_state_mutated: bool,
    pub replayed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignCompositionRequest {
    pub project_id: String,
    pub session_id: String,
    pub candidate_id: String,
    pub composition_id: String,
    pub requested_stage: String,
    pub actions: Vec<Value>,
    pub input_sha256: String,
    pub approved: bool,
    pub approval_receipt_id: String,
    pub approval_summary: String,
    pub approval_expires_at: String,
    pub approval_session_id: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignCompositionResult {
    pub schema_version: String,
    pub composition_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub requested_stage: String,
    pub input_sha256: String,
    pub job_id: String,
    pub job_status: String,
    pub job_progress: u8,
    pub status: String,
    pub execution_mode: String,
    pub steps: Vec<Value>,
    pub action_runs: Vec<Value>,
    pub completed_count: usize,
    pub next_action_index: Option<usize>,
    pub aggregate: Value,
    pub composition_proposal: Value,
    pub failure_recovery: Value,
    pub runtime_write: bool,
    pub persistent_user_data_touched: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRejectResult {
    pub schema_version: String,
    pub candidate_id: String,
    pub project_id: String,
    pub state: String,
    pub approval_receipt_id: String,
    pub request_sha256: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePrepareRequest {
    pub project_id: String,
    pub base_version_id: Option<String>,
    pub source_version_id: String,
    pub request: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePrepareResult {
    pub schema_version: String,
    pub candidate: CandidateRecord,
    pub job: JobSummary,
    pub source_version_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreConfirmRequest {
    pub project_id: String,
    pub candidate_id: String,
    pub source_version_id: String,
    pub base_version_id: Option<String>,
    pub prepared_object_id: String,
    pub prepared_object_sha256: String,
    pub quality_report_id: String,
    pub approval_receipt_id: String,
    pub approval_summary: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreConfirmResult {
    pub schema_version: String,
    pub candidate_id: String,
    pub project_id: String,
    pub source_version_id: String,
    pub version_id: String,
    pub snapshot_id: String,
    pub approval_receipt_id: String,
    pub request_sha256: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifestRecord {
    pub schema_version: String,
    pub export_id: String,
    pub project_id: String,
    pub version_id: String,
    pub format: String,
    pub profile: String,
    pub manifest_sha256: String,
    pub artifact_hashes: Vec<String>,
    pub state: String,
    pub approval_receipt_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPrepareRequest {
    pub project_id: String,
    pub version_id: String,
    pub format: String,
    pub profile: String,
    pub request: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPrepareResult {
    pub schema_version: String,
    pub manifest: ExportManifestRecord,
    pub job: JobSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfirmRequest {
    pub project_id: String,
    pub export_id: String,
    pub version_id: String,
    pub format: String,
    pub profile: String,
    pub approval_receipt_id: String,
    pub approval_summary: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfirmResult {
    pub schema_version: String,
    pub export_id: String,
    pub project_id: String,
    pub version_id: String,
    pub manifest_sha256: String,
    pub output_sha256: String,
    pub approval_receipt_id: String,
    pub request_sha256: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeErrorRecord {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub next_action: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub mutates: bool,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeResourceDescriptor {
    pub schema_version: String,
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeResourceContents {
    pub schema_version: String,
    pub uri: String,
    pub mime_type: String,
    pub text: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionRecord {
    pub schema_version: String,
    pub available: bool,
    pub project_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub version_id: Option<String>,
    pub part_ids: Vec<String>,
    pub limitation: Option<String>,
}
