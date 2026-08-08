use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_SET: &str = "forgecad-runtime-contracts@1";
pub const SCHEMA_VERSION: &str = "@1";
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
    pub status: String,
    pub mcp_transport: String,
    pub ipc_transport: String,
    pub write_model: String,
    pub supports_reference_import: bool,
    pub supports_snapshot_read: bool,
    pub supports_job_read: bool,
    pub supports_cas: bool,
    pub supports_authenticated_ipc: bool,
    pub supports_resource_read: bool,
    pub supports_geometry_execution: bool,
    pub supports_render_execution: bool,
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
            status: "alpha-mcp003".to_owned(),
            mcp_transport: "stdio-json-rpc".to_owned(),
            ipc_transport: "authenticated-local".to_owned(),
            write_model: "single-writer-preview-confirm".to_owned(),
            supports_reference_import: false,
            supports_snapshot_read: true,
            supports_job_read: true,
            supports_cas: true,
            supports_authenticated_ipc: true,
            supports_resource_read: true,
            supports_geometry_execution: false,
            supports_render_execution: false,
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
                "MCP003 exposes read-only resources and tools; mutation tools remain disabled until MCP004.".to_owned(),
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
    pub state: String,
    pub request_sha256: String,
    pub manifest_hash: Option<String>,
    pub canonical_sha256: String,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
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
