use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_SET: &str = "forgecad-runtime-contracts@1";
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
/// The canonical MCP revision for ForgeCAD. Codex currently opens configured
/// stdio servers with the 2025-06-18 legacy revision, so that revision is an
/// explicit compatibility surface rather than an implicit downgrade.
pub const MCP_PROTOCOL_COMPAT_VERSION: &str = "2025-06-18";
pub const MCP_PROTOCOL_VERSIONS: &[&str] = &[MCP_PROTOCOL_VERSION, MCP_PROTOCOL_COMPAT_VERSION];

/// Frozen policy identities for the additive animated-socket particle V2
/// contract. V2 consumes the durable transform-projection V2 record, whose
/// column-major matrix readback is part of the binding; the V1 policy names
/// must never be accepted by a V2 producer or consumer.
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_PARTICLES_V2_POLICY: &str =
    "projection-v2-driven-animated-socket-particles-dual-candidate@2";
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_PARTICLES_V2_TRANSFORM_PROJECTION_POLICY: &str =
    "glb-animation-linear-nlerp-flat-part-hierarchy-composed-world-trs-matrix@2";
/// Frozen policy identities for the additive animated-socket Trails V2
/// sequence. It consumes only Projection@2 and the projection-aware
/// Particles@2 sequence; the V1 policy names are never a valid substitute.
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_TRAILS_V2_POLICY: &str =
    "projection-v2-driven-animated-socket-trails-dual-candidate@2";
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_TRAILS_V2_HISTORY_POLICY: &str =
    "particles-v2-history-oldest-to-newest-ordinal-zero-last-immediate-previous-earlier-particle-frames-plus-current@2";
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_TRAILS_V2_HISTORY_PREROLL_POLICY: &str =
    "same-parent-particles-v2-frame-zero-is-preroll-output-frames-one-to-fifteen@2";
/// Frozen policy identities for the additive TrailsBloom V2 sequence.  It
/// consumes the terminal Trails@2 frame plus its Projection@2 and
/// Particles@2 lineage; the V1 Bloom policy is never accepted here.
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_TRAILS_BLOOM_V2_POLICY: &str =
    "projection-v2-driven-animated-socket-trails-bloom-dual-candidate@2";
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_TRAILS_BLOOM_V2_FRAME_SCOPE: &str =
    "lod0-animation-trails-bloom-v2-source-frames-1-15-with-trails-v2-frame-zero-preroll@2";
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_TRAILS_BLOOM_V2_TRAIL_KEY_SCOPE: &str =
    "animated-socket-trails-sequence-v2-frame-binding@2";
/// Frozen identity for the additive terminal animated-socket attachment
/// bridge.  The bridge consumes only the V2 projection, particles, trails
/// and TrailsBloom records; Attachment@1 and Attachment@2 remain immutable
/// sidecar-era contracts and are never valid substitutes.
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_POLICY: &str =
    "projection-v2-particles-v2-trails-v2-trails-bloom-v2-animated-socket-attachment-bridge@3";
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_FRAME_SCOPE: &str =
    "lod0-animation-attachment-v3-source-frames-1-15-with-trails-bloom-v2-frames-0-14@3";
pub const FICTIONAL_ENERGY_VFX_ANIMATED_SOCKET_ATTACHMENT_V3_STATUS: &str =
    "runtime-owned-durable-fictional-energy-vfx-animated-socket-attachment-v3";
/// Frozen identities for the additive CandidateAnimationVfxQuality@2 receipt.
/// Quality@2 keeps the V1 structural dependency surface but replaces the V1
/// socket-sidecar claim with an exact Attachment@3 record and complete ordered
/// fifteen-frame binding.  The frame count and ordered set digest are explicit
/// so a quality producer cannot silently omit part of the sequence.
pub const CANDIDATE_ANIMATION_VFX_QUALITY_V2_SCOPE: &str =
    "lod0-rigid-animation-full-vfx-stack-attachment-v3-all-15-frames@2";
pub const CANDIDATE_ANIMATION_VFX_QUALITY_V2_POLICY: &str =
    "candidate-animation-vfx-attachment-v3-structural-hard-gate@2";
pub const CANDIDATE_ANIMATION_VFX_QUALITY_V2_BINDING_STATUS: &str =
    "same-material-surface-head-candidate-exact-attachment-v3-all-15-frames-no-geometry-mutation";
pub const CANDIDATE_ANIMATION_VFX_QUALITY_V2_FRAME_COUNT: u64 = 15;
pub const CANDIDATE_ANIMATION_VFX_QUALITY_V2_FRAME_SET_SCHEMA: &str =
    "CandidateAnimationVfxQualityAttachmentFrameSet@1";

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

/// Durable production-pipeline transition receipt.  This is intentionally a
/// separate axis from the visual Agentic Design stages: it binds one exact
/// candidate/output/evidence lineage without confirming, versioning or
/// exporting the candidate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionRecord {
    pub schema_version: String,
    pub transition_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub from_stage: String,
    pub to_stage: String,
    pub candidate_state_sha256: String,
    pub artifact_sha256: String,
    pub output_kind: String,
    pub output_object_sha256: String,
    pub quality_report_object_sha256: Option<String>,
    pub comparison_report_object_sha256: Option<String>,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub evidence_sha256: String,
    pub parent_checkpoint_id: Option<String>,
    pub parent_checkpoint_sha256: Option<String>,
    pub gate_status: String,
    pub status: String,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionPrepareRequest {
    pub schema_version: String,
    pub transition_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub from_stage: String,
    pub to_stage: String,
    pub candidate_state_sha256: String,
    pub artifact_sha256: String,
    pub output_kind: String,
    pub output_object_sha256: String,
    pub quality_report_object_sha256: Option<String>,
    pub comparison_report_object_sha256: Option<String>,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub evidence_sha256: String,
    pub parent_checkpoint_id: Option<String>,
    pub parent_checkpoint_sha256: Option<String>,
    pub input_sha256: String,
    pub approved: bool,
    pub approval_receipt_id: String,
    pub approval_summary: String,
    pub approval_expires_at: String,
    pub approval_session_id: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionPrepareResult {
    pub schema_version: String,
    pub transition: ProductionStageTransitionRecord,
    pub production_stage: String,
    pub replayed: bool,
    pub runtime_write: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionGetRequest {
    pub schema_version: String,
    pub transition_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionGetResult {
    pub schema_version: String,
    pub transition: ProductionStageTransitionRecord,
    pub production_stage: String,
    pub replayed: bool,
    pub runtime_write: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// V2 production head for the first dual-candidate promotion boundary.  The
/// root remains the topology candidate while the head points at the distinct
/// material-surface output candidate; this is deliberately independent from
/// the V1 single-candidate production head.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageHeadV2Record {
    pub schema_version: String,
    pub session_id: String,
    pub project_id: String,
    pub root_candidate_id: String,
    pub root_candidate_role: String,
    pub root_candidate_state_sha256: String,
    pub source_artifact_id: String,
    pub root_artifact_sha256: String,
    pub root_stage: String,
    pub previous_head_candidate_id: String,
    pub previous_head_candidate_role: String,
    pub previous_head_candidate_state_sha256: String,
    pub previous_head_artifact_id: String,
    pub previous_head_artifact_sha256: String,
    pub previous_head_stage: String,
    pub head_candidate_id: String,
    pub head_candidate_role: String,
    pub head_candidate_state_sha256: String,
    pub output_artifact_id: String,
    pub head_artifact_sha256: String,
    pub head_stage: String,
    pub topology_quality_id: String,
    pub topology_quality_status: String,
    pub topology_quality_report_object_sha256: String,
    pub topology_quality_canonical_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_status: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub evidence_sha256: String,
    pub approval_receipt_id: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub approval_summary_sha256: String,
    pub candidate_binding_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub head_transition_id: String,
    pub head_transition_sha256: String,
    pub parent_topology_transition_id: String,
    pub parent_topology_transition_sha256: String,
    pub parent_topology_transition_schema_version: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub updated_at: String,
}

/// V2 approval-gated transition from the topology root candidate to a
/// distinct material-surface output candidate. Failed or invalid quality
/// input is rejected before any write, so every persisted V2 record is passed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV2Record {
    pub schema_version: String,
    pub transition_id: String,
    pub session_id: String,
    pub project_id: String,
    pub root_candidate_id: String,
    pub root_candidate_role: String,
    pub root_candidate_state_sha256: String,
    pub source_artifact_id: String,
    pub root_artifact_sha256: String,
    pub previous_head_candidate_id: String,
    pub previous_head_candidate_role: String,
    pub previous_head_candidate_state_sha256: String,
    pub previous_head_artifact_id: String,
    pub previous_head_artifact_sha256: String,
    pub previous_head_stage: String,
    pub head_candidate_id: String,
    pub head_candidate_role: String,
    pub head_candidate_state_sha256: String,
    pub output_artifact_id: String,
    pub head_artifact_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub topology_quality_id: String,
    pub topology_quality_status: String,
    pub topology_quality_report_object_sha256: String,
    pub topology_quality_canonical_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_status: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub candidate_binding_status: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub evidence_sha256: String,
    pub approval_receipt_id: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub approval_summary_sha256: String,
    pub parent_topology_transition_id: String,
    pub parent_topology_transition_sha256: String,
    pub parent_topology_transition_schema_version: String,
    pub gate_status: String,
    pub status: String,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV2PrepareRequest {
    pub schema_version: String,
    pub transition_id: String,
    pub session_id: String,
    pub project_id: String,
    pub root_candidate_id: String,
    pub root_candidate_role: String,
    pub root_candidate_state_sha256: String,
    pub source_artifact_id: String,
    pub root_artifact_sha256: String,
    pub previous_head_candidate_id: String,
    pub previous_head_candidate_role: String,
    pub previous_head_candidate_state_sha256: String,
    pub previous_head_artifact_id: String,
    pub previous_head_artifact_sha256: String,
    pub previous_head_stage: String,
    pub head_candidate_id: String,
    pub head_candidate_role: String,
    pub head_candidate_state_sha256: String,
    pub output_artifact_id: String,
    pub head_artifact_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub topology_quality_id: String,
    pub topology_quality_status: String,
    pub topology_quality_report_object_sha256: String,
    pub topology_quality_canonical_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_status: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub candidate_binding_status: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub evidence_sha256: String,
    pub approval_receipt_id: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub parent_topology_transition_id: String,
    pub parent_topology_transition_sha256: String,
    pub parent_topology_transition_schema_version: String,
    pub input_sha256: String,
    pub approved: bool,
    pub approval_summary: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV2PrepareResult {
    pub schema_version: String,
    pub transition: ProductionStageTransitionV2Record,
    pub production_stage_head: ProductionStageHeadV2Record,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV2GetRequest {
    pub schema_version: String,
    pub transition_id: String,
    pub session_id: String,
    pub project_id: String,
    pub root_candidate_id: String,
    pub head_candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV2GetResult {
    pub schema_version: String,
    pub transition: ProductionStageTransitionV2Record,
    pub production_stage_head: ProductionStageHeadV2Record,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// Short aliases make the V2 head/transition records convenient for the
/// Runtime implementation without changing the historical V1 names.
pub type ProductionStageHeadRecord = ProductionStageHeadV2Record;

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

/// Durable candidate-bound objective topology gate for the gray-model to
/// topology production transition.  The topology snapshot and authoring
/// topology remain read-only evidence; this record stores exact, ordered
/// bindings for every renderable Part together with bounded aggregate metrics.
/// AuthoringTopology hashes are optional because primitive/operator Parts may
/// not have an authoring cage.  It never
/// claims edge-flow, artistic quality, reference likeness or game-engine
/// readiness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateTopologyQualityRecord {
    pub schema_version: String,
    pub topology_quality_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub artifact_readback_sha256: String,
    pub artifact_readback_object_sha256: String,
    pub geometry_candidate_evidence_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_program_object_sha256: String,
    pub operator_catalog_sha256: String,
    pub readback_config_sha256: String,
    pub part_inventory_sha256: String,
    pub part_ids: Vec<String>,
    pub part_topology_snapshot_sha256s: Vec<String>,
    pub authoring_topology_status: String,
    pub part_authoring_topology_sha256s: Vec<Option<String>>,
    pub topology_quality_policy: String,
    pub topology_quality_policy_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub topology_status: String,
    pub thresholds: CandidateTopologyQualityThresholds,
    pub metrics: CandidateTopologyQualityMetrics,
    pub hard_gate: CandidateTopologyQualityHardGate,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub edge_flow_status: String,
    pub artistic_quality_status: String,
    pub visual_quality_status: String,
    pub materialization_status: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Frozen numeric limits carried in the receipt so a later readback does not
/// have to infer the gate from a Runtime cohort.  The policy hash identifies
/// the canonical policy; these values make the evaluated comparison explicit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateTopologyQualityThresholds {
    pub max_triangle_aspect_ratio: f64,
    pub max_vertex_valence: u64,
    pub min_triangle_area_m2: f64,
    pub min_semantic_part_coverage: f64,
    pub min_semantic_material_zone_coverage: f64,
    pub min_semantic_source_node_coverage: f64,
}

/// Aggregate measurements used by `CandidateTopologyQuality@1`.  These are
/// objective readback values, not an artistic evaluation or a mesh-edit
/// instruction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateTopologyQualityMetrics {
    pub invalid_index_count: u64,
    pub non_finite_count: u64,
    pub degenerate_triangle_count: u64,
    pub boundary_edge_count: u64,
    pub non_manifold_edge_count: u64,
    pub orientation_conflict_count: u64,
    pub winding_error_count: u64,
    pub part_count: u64,
    pub solid_part_count: u64,
    pub non_solid_part_count: u64,
    pub solid_boundary_violation_count: u64,
    pub triangle_count: u64,
    pub vertex_count: u64,
    pub edge_count: u64,
    pub min_triangle_area_m2: f64,
    pub max_triangle_aspect_ratio: f64,
    pub max_vertex_valence: u64,
    pub normal_non_finite_count: u64,
    pub normal_non_unit_count: u64,
    pub normal_alignment_error_count: u64,
    pub uv_non_finite_count: u64,
    pub uv_degenerate_triangle_count: u64,
    pub tangent_non_finite_count: u64,
    pub tangent_orthogonality_error_count: u64,
    pub tangent_handedness_error_count: u64,
    pub semantic_part_coverage: f64,
    pub semantic_material_zone_coverage: f64,
    pub semantic_source_node_coverage: f64,
}

/// Per-condition objective gate results.  A true durable quality pass
/// requires every condition to be true; a false result is still useful
/// diagnostic evidence but cannot advance the production head.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateTopologyQualityHardGate {
    pub finite_geometry: bool,
    pub valid_indices: bool,
    pub non_degenerate_triangles: bool,
    pub boundary_policy: bool,
    pub manifold: bool,
    pub orientation: bool,
    pub counts_within_budget: bool,
    pub triangle_aspect_ratio: bool,
    pub vertex_valence: bool,
    pub normal_integrity: bool,
    pub uv_integrity: bool,
    pub tangent_integrity: bool,
    pub semantic_coverage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateTopologyQualityPrepareRequest {
    pub schema_version: String,
    pub topology_quality_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub artifact_readback_sha256: String,
    pub artifact_readback_object_sha256: String,
    pub geometry_candidate_evidence_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_program_object_sha256: String,
    pub operator_catalog_sha256: String,
    pub readback_config_sha256: String,
    pub part_inventory_sha256: String,
    pub part_ids: Vec<String>,
    pub part_topology_snapshot_sha256s: Vec<String>,
    pub authoring_topology_status: String,
    pub part_authoring_topology_sha256s: Vec<Option<String>>,
    pub topology_quality_policy: String,
    pub topology_quality_policy_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateTopologyQualityPrepareResult {
    pub schema_version: String,
    pub topology_quality: CandidateTopologyQualityRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateTopologyQualityGetRequest {
    pub schema_version: String,
    pub topology_quality_id: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CandidateTopologyQualityGetResult {
    pub schema_version: String,
    pub topology_quality: CandidateTopologyQualityRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// Immutable structural material-surface evidence for one passing topology
/// source candidate and one distinct derived Appearance candidate.  The
/// record binds strict source/output readback, byte-exact renderable geometry
/// preservation and the first-party offline 2K PBR provenance chain.  It does
/// not claim visual likeness, artistic quality, human approval, commercial
/// FPS quality or a commercial-engine roundtrip.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateMaterialSurfaceQualityRecord {
    pub schema_version: String,
    pub material_surface_quality_id: String,
    pub project_id: String,
    pub source_candidate_id: String,
    pub source_candidate_state_sha256: String,
    pub source_artifact_id: String,
    pub source_artifact_sha256: String,
    pub source_artifact_readback_sha256: String,
    pub source_artifact_readback_object_sha256: String,
    pub source_geometry_candidate_evidence_sha256: String,
    pub source_geometry_program_sha256: String,
    pub source_topology_quality_id: String,
    pub source_topology_quality_report_object_sha256: String,
    pub source_topology_quality_canonical_sha256: String,
    pub output_candidate_id: String,
    pub output_candidate_state_sha256: String,
    pub output_artifact_id: String,
    pub output_artifact_sha256: String,
    pub output_artifact_readback_sha256: String,
    pub output_artifact_readback_object_sha256: String,
    pub output_geometry_program_sha256: String,
    pub appearance_source_lineage_sidecar_object_sha256: String,
    pub appearance_source_lineage_canonical_sha256: String,
    pub appearance_program_object_sha256: String,
    pub appearance_program_sha256: String,
    pub material_layer_stack_sha256: String,
    pub material_pack_id: String,
    pub material_pack_version: String,
    pub material_pack_license_spdx: String,
    pub material_pack_manifest_object_sha256: String,
    pub material_pack_manifest_sha256: String,
    pub material_pack_provenance_sha256: String,
    pub texture_build_receipt_object_sha256: String,
    pub texture_build_receipt_canonical_sha256: String,
    pub candidate_surface_bake_receipt_object_sha256: String,
    pub candidate_surface_bake_receipt_canonical_sha256: String,
    pub uv_binding_sha256: String,
    pub tangent_binding_sha256: String,
    pub material_zone_inventory_sha256: String,
    pub material_provenance_sha256: String,
    pub lod_scope: String,
    pub source_output_candidate_binding_status: String,
    pub geometry_preservation_projection_sha256: String,
    pub geometry_preservation_status: String,
    pub material_surface_quality_policy: String,
    pub material_surface_quality_policy_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub hard_gate: CandidateMaterialSurfaceQualityHardGate,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub visual_quality_status: String,
    pub artistic_quality_status: String,
    pub human_review_status: String,
    pub commercial_fps_quality_status: String,
    pub commercial_engine_status: String,
    pub materialization_status: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Every technical predicate required for the structural material-surface
/// gate. A passed record requires all fields to be true; visual and human
/// quality remain outside this gate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateMaterialSurfaceQualityHardGate {
    pub distinct_candidates: bool,
    pub source_topology_quality: bool,
    pub source_artifact_readback: bool,
    pub output_artifact_readback: bool,
    pub geometry_preserved: bool,
    pub appearance_source_lineage: bool,
    pub material_pack_2k: bool,
    pub texture_build_v2: bool,
    pub surface_bake_v1: bool,
    pub uv_integrity: bool,
    pub tangent_integrity: bool,
    pub material_provenance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateMaterialSurfaceQualityPrepareRequest {
    pub schema_version: String,
    pub material_surface_quality_id: String,
    pub project_id: String,
    pub source_candidate_id: String,
    pub source_candidate_state_sha256: String,
    pub source_artifact_id: String,
    pub source_artifact_sha256: String,
    pub source_artifact_readback_sha256: String,
    pub source_artifact_readback_object_sha256: String,
    pub source_geometry_candidate_evidence_sha256: String,
    pub source_geometry_program_sha256: String,
    pub source_topology_quality_id: String,
    pub source_topology_quality_report_object_sha256: String,
    pub source_topology_quality_canonical_sha256: String,
    pub output_candidate_id: String,
    pub output_candidate_state_sha256: String,
    pub output_artifact_id: String,
    pub output_artifact_sha256: String,
    pub output_artifact_readback_sha256: String,
    pub output_artifact_readback_object_sha256: String,
    pub output_geometry_program_sha256: String,
    pub appearance_source_lineage_sidecar_object_sha256: String,
    pub appearance_source_lineage_canonical_sha256: String,
    pub appearance_program_object_sha256: String,
    pub appearance_program_sha256: String,
    pub material_layer_stack_sha256: String,
    pub material_pack_manifest_object_sha256: String,
    pub material_pack_manifest_sha256: String,
    pub material_pack_provenance_sha256: String,
    pub texture_build_receipt_object_sha256: String,
    pub texture_build_receipt_canonical_sha256: String,
    pub candidate_surface_bake_receipt_object_sha256: String,
    pub candidate_surface_bake_receipt_canonical_sha256: String,
    pub uv_binding_sha256: String,
    pub tangent_binding_sha256: String,
    pub material_zone_inventory_sha256: String,
    pub material_provenance_sha256: String,
    pub lod_scope: String,
    pub geometry_preservation_projection_sha256: String,
    pub material_surface_quality_policy: String,
    pub material_surface_quality_policy_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateMaterialSurfaceQualityPrepareResult {
    pub schema_version: String,
    pub material_surface_quality: CandidateMaterialSurfaceQualityRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateMaterialSurfaceQualityGetRequest {
    pub schema_version: String,
    pub material_surface_quality_id: String,
    pub project_id: String,
    pub source_candidate_id: String,
    pub output_candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateMaterialSurfaceQualityGetResult {
    pub schema_version: String,
    pub material_surface_quality: CandidateMaterialSurfaceQualityRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// Immutable structural animation/VFX evidence bound to the exact material-
/// surface production head candidate.  This receipt composes the existing
/// delivery, animation, socket and typed VFX receipts; it does not create a
/// new geometry candidate or claim visual, artistic, human or commercial FPS
/// quality.  The owned report object is indexed by Store and intentionally is
/// not duplicated in this public record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityRecord {
    pub schema_version: String,
    pub animation_vfx_quality_id: String,
    pub project_id: String,
    pub source_material_surface_transition_id: String,
    pub source_material_surface_transition_sha256: String,
    pub source_material_surface_head_canonical_sha256: String,
    pub source_material_surface_quality_id: String,
    pub source_material_surface_quality_report_object_sha256: String,
    pub source_material_surface_quality_canonical_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub animated_socket_receipt_object_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub vfx_sequence_key_sha256: String,
    pub vfx_sequence_canonical_sha256: String,
    pub vfx_frame_key_sha256: String,
    pub vfx_frame_canonical_sha256: String,
    pub vfx_bloom_key_sha256: String,
    pub vfx_bloom_canonical_sha256: String,
    pub vfx_particle_key_sha256: String,
    pub vfx_particle_canonical_sha256: String,
    pub vfx_trail_key_sha256: String,
    pub vfx_trail_canonical_sha256: String,
    pub vfx_trail_bloom_key_sha256: String,
    pub vfx_trail_bloom_canonical_sha256: String,
    pub particle_history_key_sha256s: Vec<String>,
    pub sample_request_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub animation_vfx_scope: String,
    pub animation_vfx_policy: String,
    pub animation_vfx_policy_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub input_sha256: String,
    pub candidate_binding_status: String,
    pub hard_gate: CandidateAnimationVfxQualityHardGate,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub animation_status: String,
    pub vfx_status: String,
    pub visual_quality_status: String,
    pub artistic_quality_status: String,
    pub human_review_status: String,
    pub commercial_fps_quality_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub functional_semantics: bool,
    pub materialization_status: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// The twenty bounded dependency predicates required by the structural
/// animation/VFX hard gate.  A durable passing receipt requires every member
/// to be true and `validator_status` to be `passed`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityHardGate {
    pub material_surface_head_binding: bool,
    pub material_surface_quality: bool,
    pub delivery_lod0_binding: bool,
    pub anchor_set_binding: bool,
    pub animation_clip_binding: bool,
    pub animation_glb_readback: bool,
    pub animated_socket_readback: bool,
    pub vfx_profile_binding: bool,
    pub base_frame_stack: bool,
    pub bloom_stack: bool,
    pub particle_stack: bool,
    pub trail_stack: bool,
    pub trail_bloom_stack: bool,
    pub cross_layer_parent_binding: bool,
    pub sample_camera_binding: bool,
    pub worker_cohort_binding: bool,
    pub render_pass_byte_exact: bool,
    pub bounded_resource_policy: bool,
    pub vfx_glb_socket_attachment: bool,
    pub nonfunctional_scope: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityPrepareRequest {
    pub schema_version: String,
    pub animation_vfx_quality_id: String,
    pub project_id: String,
    pub source_material_surface_transition_id: String,
    pub source_material_surface_transition_sha256: String,
    pub source_material_surface_head_canonical_sha256: String,
    pub source_material_surface_quality_id: String,
    pub source_material_surface_quality_report_object_sha256: String,
    pub source_material_surface_quality_canonical_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub animated_socket_receipt_object_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub vfx_sequence_key_sha256: String,
    pub vfx_sequence_canonical_sha256: String,
    pub vfx_frame_key_sha256: String,
    pub vfx_frame_canonical_sha256: String,
    pub vfx_bloom_key_sha256: String,
    pub vfx_bloom_canonical_sha256: String,
    pub vfx_particle_key_sha256: String,
    pub vfx_particle_canonical_sha256: String,
    pub vfx_trail_key_sha256: String,
    pub vfx_trail_canonical_sha256: String,
    pub vfx_trail_bloom_key_sha256: String,
    pub vfx_trail_bloom_canonical_sha256: String,
    pub particle_history_key_sha256s: Vec<String>,
    pub sample_request_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub animation_vfx_scope: String,
    pub animation_vfx_policy: String,
    pub animation_vfx_policy_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityPrepareResult {
    pub schema_version: String,
    pub animation_vfx_quality: CandidateAnimationVfxQualityRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityGetRequest {
    pub schema_version: String,
    pub animation_vfx_quality_id: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityGetResult {
    pub schema_version: String,
    pub animation_vfx_quality: CandidateAnimationVfxQualityRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// Additive structural animation/VFX evidence bound to the exact
/// material-surface head and the terminal Attachment@3 bridge.  This type is
/// intentionally separate from the historical V1 record: V1 remains a
/// sidecar-era contract, while V2 can only report the durable Attachment@3
/// record and its complete ordered fifteen-frame set.  The owned V2 report object
/// is indexed by Store and is not duplicated in this public record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityV2Record {
    pub schema_version: String,
    pub animation_vfx_quality_id: String,
    pub project_id: String,
    pub source_material_surface_transition_id: String,
    pub source_material_surface_transition_sha256: String,
    pub source_material_surface_head_canonical_sha256: String,
    pub source_material_surface_quality_id: String,
    pub source_material_surface_quality_report_object_sha256: String,
    pub source_material_surface_quality_canonical_sha256: String,
    pub candidate_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub geometry_preservation_status: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub anchor_binding_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_bloom_sequence_key_sha256: String,
    pub trail_bloom_sequence_canonical_sha256: String,
    pub attachment_key_sha256: String,
    pub attachment_canonical_sha256: String,
    pub attachment_receipt_object_sha256: String,
    pub attachment_receipt_canonical_sha256: String,
    pub attachment_frame_count: u64,
    pub attachment_frame_set_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub trail_bloom_profile_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub attachment_policy: String,
    pub frame_scope: String,
    pub animation_vfx_scope: String,
    pub animation_vfx_policy: String,
    pub animation_vfx_policy_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub input_sha256: String,
    pub candidate_binding_status: String,
    pub hard_gate: CandidateAnimationVfxQualityV2HardGate,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub animation_status: String,
    pub vfx_status: String,
    pub visual_quality_status: String,
    pub artistic_quality_status: String,
    pub human_review_status: String,
    pub commercial_fps_quality_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub functional_semantics: bool,
    pub materialization_status: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// The V2 structural gate has the same twenty bounded dependency predicates
/// as V1.  Its `vfx_glb_socket_attachment` member has a stricter source: a
/// Runtime producer may set it only after an exact durable Attachment@3 get
/// and exact ordered fifteen-frame set binding.  A legacy sidecar boolean is not
/// a valid input to this gate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityV2HardGate {
    pub material_surface_head_binding: bool,
    pub material_surface_quality: bool,
    pub delivery_lod0_binding: bool,
    pub anchor_set_binding: bool,
    pub animation_clip_binding: bool,
    pub animation_glb_readback: bool,
    pub animated_socket_readback: bool,
    pub vfx_profile_binding: bool,
    pub base_frame_stack: bool,
    pub bloom_stack: bool,
    pub particle_stack: bool,
    pub trail_stack: bool,
    pub trail_bloom_stack: bool,
    pub cross_layer_parent_binding: bool,
    pub sample_camera_binding: bool,
    pub worker_cohort_binding: bool,
    pub render_pass_byte_exact: bool,
    pub bounded_resource_policy: bool,
    pub vfx_glb_socket_attachment: bool,
    pub nonfunctional_scope: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityV2PrepareRequest {
    pub schema_version: String,
    pub animation_vfx_quality_id: String,
    pub project_id: String,
    pub source_material_surface_transition_id: String,
    pub source_material_surface_transition_sha256: String,
    pub source_material_surface_head_canonical_sha256: String,
    pub source_material_surface_quality_id: String,
    pub source_material_surface_quality_report_object_sha256: String,
    pub source_material_surface_quality_canonical_sha256: String,
    pub candidate_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub geometry_preservation_status: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub anchor_binding_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_bloom_sequence_key_sha256: String,
    pub trail_bloom_sequence_canonical_sha256: String,
    pub attachment_key_sha256: String,
    pub attachment_canonical_sha256: String,
    pub attachment_receipt_object_sha256: String,
    pub attachment_receipt_canonical_sha256: String,
    pub attachment_frame_count: u64,
    pub attachment_frame_set_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub trail_bloom_profile_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub attachment_policy: String,
    pub frame_scope: String,
    pub animation_vfx_scope: String,
    pub animation_vfx_policy: String,
    pub animation_vfx_policy_sha256: String,
    pub from_stage: String,
    pub to_stage: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityV2PrepareResult {
    pub schema_version: String,
    pub animation_vfx_quality: CandidateAnimationVfxQualityV2Record,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityV2GetRequest {
    pub schema_version: String,
    pub animation_vfx_quality_id: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateAnimationVfxQualityV2GetResult {
    pub schema_version: String,
    pub animation_vfx_quality: CandidateAnimationVfxQualityV2Record,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// Durable SQLite index for one immutable SubdivisionArtifactLineageSidecar
/// CAS object. The complete sidecar stays in CAS and is embedded only in the
/// public `SubdivisionArtifactLineageLink@1` contract; keeping this index
/// compact avoids moving large lineage arrays into SQLite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubdivisionArtifactLineageLinkRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub artifact_id: String,
    pub artifact_readback_sha256: String,
    pub geometry_candidate_evidence_sha256: String,
    pub subdivision_node_id: String,
    pub request_sha256: String,
    pub sidecar_object_sha256: String,
    pub lineage_sha256: String,
    pub artifact_binding_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Compact SQLite index for one immutable, candidate-bound mechanical
/// animation clip stored as canonical JSON in CAS. The normalized RestFrame
/// and PoseAction remain in the CAS object; SQLite keeps only the exact
/// evidence and content hashes needed for deterministic lookup and conflict
/// detection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationClipLinkRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub artifact_id: String,
    pub artifact_readback_sha256: String,
    pub geometry_candidate_evidence_sha256: String,
    pub program_sha256: String,
    pub operator_catalog_sha256: String,
    pub readback_config_sha256: String,
    pub clip_id: String,
    pub request_sha256: String,
    pub clip_object_sha256: String,
    pub clip_sha256: String,
    pub rest_frame_sha256: String,
    pub pose_action_sha256: String,
    pub source_replay_worker_cohort_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Compact durable index for the appearance-aware V2 mechanical animation
/// clip.  Unlike the immutable V1 clip, this record binds the animated
/// action to an appearance candidate while retaining the exact geometry
/// source, material-surface quality report and appearance lineage ancestors.
/// The nested RestFrame/PoseAction and replay evidence remain in the CAS clip
/// object; SQLite only needs these hashes for restart lookup and conflict
/// detection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationClipV2LinkRecord {
    pub schema_version: String,
    pub project_id: String,
    pub clip_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_artifact_id: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub appearance_artifact_readback_object_sha256: String,
    pub source_geometry_candidate_id: String,
    pub source_geometry_candidate_state_sha256: String,
    pub source_geometry_artifact_id: String,
    pub source_geometry_artifact_sha256: String,
    pub source_geometry_candidate_evidence_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub appearance_source_lineage_sidecar_object_sha256: String,
    pub appearance_source_lineage_canonical_sha256: String,
    pub appearance_program_object_sha256: String,
    pub appearance_program_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub operator_catalog_sha256: String,
    pub readback_config_sha256: String,
    pub clip_object_sha256: String,
    pub clip_sha256: String,
    pub rest_frame_sha256: String,
    pub pose_action_sha256: String,
    pub request_sha256: String,
    pub source_replay_worker_cohort_sha256: String,
    pub replay_policy: String,
    pub materialization_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Immutable CAS payload for MechanicalAnimationClip@2.  The public
/// contract keeps the caller-authored rigid action typed as Value here so the
/// contracts crate remains independent of the Runtime's pose implementation;
/// the referenced RestFrame/PoseAction schemas still close the JSON shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationClipV2Record {
    pub schema_version: String,
    pub clip_id: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_artifact_id: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub appearance_artifact_readback_object_sha256: String,
    pub source_geometry_candidate_id: String,
    pub source_geometry_candidate_state_sha256: String,
    pub source_geometry_artifact_id: String,
    pub source_geometry_artifact_sha256: String,
    pub source_geometry_candidate_evidence_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub appearance_source_lineage_sidecar_object_sha256: String,
    pub appearance_source_lineage_canonical_sha256: String,
    pub appearance_program_object_sha256: String,
    pub appearance_program_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub operator_catalog_sha256: String,
    pub readback_config_sha256: String,
    pub request_sha256: String,
    pub rest_frame: Value,
    pub rest_frame_sha256: String,
    pub pose_action: Value,
    pub pose_action_sha256: String,
    pub sampling_policy: Value,
    pub sampling_policy_sha256: String,
    pub source_replay: Value,
    pub source_replay_worker_cohort_sha256: String,
    pub replay_policy: String,
    pub materialization_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationClipV2PrepareRequest {
    pub schema_version: String,
    pub clip_id: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_artifact_id: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub appearance_artifact_readback_object_sha256: String,
    pub source_geometry_candidate_id: String,
    pub source_geometry_candidate_state_sha256: String,
    pub source_geometry_artifact_id: String,
    pub source_geometry_artifact_sha256: String,
    pub source_geometry_candidate_evidence_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub appearance_source_lineage_sidecar_object_sha256: String,
    pub appearance_source_lineage_canonical_sha256: String,
    pub appearance_program_object_sha256: String,
    pub appearance_program_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub operator_catalog_sha256: String,
    pub readback_config_sha256: String,
    pub rest_frame: Value,
    pub pose_action: Value,
    pub sampling_policy: Value,
    pub replay_policy: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationClipV2PrepareResult {
    pub schema_version: String,
    pub clip: MechanicalAnimationClipV2Record,
    pub durable_link: MechanicalAnimationClipV2LinkRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationClipV2GetRequest {
    pub schema_version: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub clip_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationClipV2GetResult {
    pub schema_version: String,
    pub clip: MechanicalAnimationClipV2Record,
    pub durable_link: MechanicalAnimationClipV2LinkRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// Immutable CAS receipt for one appearance-aware rigid animated GLB.  The
/// receipt deliberately repeats the complete upstream appearance, geometry,
/// quality and material-pack lineage so a downstream GLB can be revalidated
/// after a Runtime restart without treating a Clip@2 hash as sufficient
/// provenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationGlbV2ReceiptRecord {
    pub schema_version: String,
    pub animation_glb_key_sha256: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_artifact_id: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub appearance_artifact_readback_object_sha256: String,
    pub source_geometry_candidate_id: String,
    pub source_geometry_candidate_state_sha256: String,
    pub source_geometry_artifact_id: String,
    pub source_geometry_artifact_sha256: String,
    pub source_geometry_candidate_evidence_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub appearance_source_lineage_sidecar_object_sha256: String,
    pub appearance_source_lineage_canonical_sha256: String,
    pub appearance_program_object_sha256: String,
    pub appearance_program_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub operator_catalog_sha256: String,
    pub readback_config_sha256: String,
    pub material_pack_id: String,
    pub material_pack_version: String,
    pub material_pack_license_spdx: String,
    pub material_pack_manifest_object_sha256: String,
    pub material_pack_manifest_sha256: String,
    pub material_pack_provenance_sha256: String,
    pub texture_build_receipt_object_sha256: String,
    pub texture_build_receipt_canonical_sha256: String,
    pub candidate_surface_bake_receipt_object_sha256: String,
    pub candidate_surface_bake_receipt_canonical_sha256: String,
    pub clip_id: String,
    pub clip_object_sha256: String,
    pub clip_sha256: String,
    pub rest_frame_sha256: String,
    pub pose_action_sha256: String,
    pub sampling_policy_sha256: String,
    pub source_replay_worker_cohort_sha256: String,
    pub frame_preview_hashes_sha256: String,
    pub frame_preview_worker_cohort_sha256: String,
    pub sample_time_ticks: Vec<u64>,
    pub timebase_hz: u64,
    pub interpolation: String,
    pub part_ids: Vec<String>,
    pub node_count: u64,
    pub sampler_count: u64,
    pub channel_count: u64,
    pub accessor_count_added: u64,
    pub buffer_view_count_added: u64,
    pub animated_artifact_sha256: String,
    pub animated_artifact_readback_sha256: String,
    pub animation_validation_sha256: String,
    pub source_static_projection_sha256: String,
    pub appearance_material_projection_sha256: String,
    pub source_static_projection_exact: bool,
    pub binary_prefix_exact: bool,
    pub appearance_material_projection_exact: bool,
    pub material_pack_identity_exact: bool,
    pub no_skinning: bool,
    pub no_morph_targets: bool,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub materialization_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub limitations: Vec<String>,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Compact SQLite projection for one immutable appearance-aware animated GLB.
/// The animation key is a deterministic non-CAS primary key; all upstream
/// object hashes remain direct fields for reachability and conflict checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationGlbV2LinkRecord {
    pub schema_version: String,
    pub animation_glb_key_sha256: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_artifact_id: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub appearance_artifact_readback_object_sha256: String,
    pub source_geometry_candidate_id: String,
    pub source_geometry_candidate_state_sha256: String,
    pub source_geometry_artifact_id: String,
    pub source_geometry_artifact_sha256: String,
    pub source_geometry_candidate_evidence_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub appearance_source_lineage_sidecar_object_sha256: String,
    pub appearance_source_lineage_canonical_sha256: String,
    pub appearance_program_object_sha256: String,
    pub appearance_program_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub operator_catalog_sha256: String,
    pub readback_config_sha256: String,
    pub material_pack_id: String,
    pub material_pack_version: String,
    pub material_pack_license_spdx: String,
    pub material_pack_manifest_object_sha256: String,
    pub material_pack_manifest_sha256: String,
    pub material_pack_provenance_sha256: String,
    pub texture_build_receipt_object_sha256: String,
    pub texture_build_receipt_canonical_sha256: String,
    pub candidate_surface_bake_receipt_object_sha256: String,
    pub candidate_surface_bake_receipt_canonical_sha256: String,
    pub clip_id: String,
    pub clip_object_sha256: String,
    pub clip_sha256: String,
    pub rest_frame_sha256: String,
    pub pose_action_sha256: String,
    pub sampling_policy_sha256: String,
    pub source_replay_worker_cohort_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_readback_sha256: String,
    pub receipt_object_sha256: String,
    pub receipt_canonical_sha256: String,
    pub request_sha256: String,
    pub materialization_policy: String,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub materialization_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationGlbV2PrepareRequest {
    pub schema_version: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub clip_id: String,
    pub clip_object_sha256: String,
    pub clip_sha256: String,
    pub materialization_policy: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationGlbV2PrepareResult {
    pub schema_version: String,
    pub animation_glb_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_size_bytes: u64,
    pub receipt_object_sha256: String,
    pub receipt: MechanicalAnimationGlbV2ReceiptRecord,
    pub durable_link: MechanicalAnimationGlbV2LinkRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationGlbV2GetRequest {
    pub schema_version: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub clip_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationGlbV2GetResult {
    pub schema_version: String,
    pub animation_glb_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_size_bytes: u64,
    pub receipt_object_sha256: String,
    pub receipt: MechanicalAnimationGlbV2ReceiptRecord,
    pub durable_link: MechanicalAnimationGlbV2LinkRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationClipV2PreviewRequest {
    pub schema_version: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub clip_id: String,
    pub sample_time_ticks: u64,
    pub preview_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicalAnimationClipV2Preview {
    pub schema_version: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub appearance_artifact_readback_object_sha256: String,
    pub source_geometry_candidate_id: String,
    pub source_geometry_candidate_state_sha256: String,
    pub source_geometry_artifact_sha256: String,
    pub source_geometry_candidate_evidence_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub rest_frame_sha256: String,
    pub pose_action_sha256: String,
    pub sample_time_ticks: u64,
    pub frame_sha256: String,
    pub source_replay_worker_cohort_sha256: String,
    pub appearance_transient_artifact_sha256: String,
    pub appearance_transient_artifact_readback_sha256: String,
    pub appearance_replay_worker_cohort_sha256: String,
    pub appearance_program_sha256: String,
    pub appearance_transient_program_sha256: String,
    pub material_pack_manifest_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub pose_geometry_preview: Value,
    pub geometry_materialization: String,
    pub appearance_materialization: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub limitations: Vec<String>,
    pub canonical_sha256: String,
}

/// Compact durable index for one immutable game-asset delivery cohort. The
/// complete LOD, collision, readiness and manifest documents remain in CAS;
/// SQLite keeps their exact hashes and the three candidate/artifact bindings
/// so restart readback and CAS reachability do not depend on scanning JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameAssetDeliveryLinkRecord {
    pub schema_version: String,
    pub project_id: String,
    pub lod_candidate_ids: Vec<String>,
    pub lod_artifact_sha256s: Vec<String>,
    pub request_sha256: String,
    pub lod_receipt_object_sha256: String,
    pub collision_proxy_object_sha256: String,
    pub readiness_object_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub animation_artifact_sha256: Option<String>,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Restart-safe binding between one immutable game delivery and its
/// engine-neutral fictional-weapon anchor sidecar. The sidecar contains only
/// visual/authoring transforms; it is not physics, hitbox or ballistic truth.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponAnchorLinkRecord {
    pub schema_version: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub lod0_artifact_sha256: String,
    pub request_sha256: String,
    pub anchor_set_object_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Compact durable index for one immutable engine-neutral GLB socket
/// materialization. The source delivery and AnchorSet remain immutable; this
/// parent record only binds the derived receipt and its deterministic key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponGlbSocketMaterializationLinkRecord {
    pub schema_version: String,
    pub socket_materialization_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub request_sha256: String,
    pub socket_materialization_policy: String,
    pub lod_scope: String,
    pub socket_node_id_encoding_sha256: String,
    pub receipt_object_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Child durable index for one source/derived LOD pair in the GLB socket
/// materialization. Inventory and BIN hashes point to the inline receipt
/// readback; no separate readback schema or CAS object is implied here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponGlbSocketMaterializationLodRecord {
    pub schema_version: String,
    pub socket_materialization_key_sha256: String,
    pub lod_level: u64,
    pub source_candidate_id: String,
    pub source_candidate_state_sha256: String,
    pub source_artifact_sha256: String,
    pub source_artifact_readback_sha256: String,
    pub derived_artifact_sha256: String,
    pub derived_artifact_readback_sha256: String,
    pub source_renderable_inventory_sha256: String,
    pub derived_renderable_inventory_sha256: String,
    pub socket_node_inventory_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Compact durable index for one immutable LOD0 animated GLB socket
/// materialization. The source static delivery, source animated GLB and
/// source animation receipt remain immutable; this record owns only the
/// derived animated socket GLB and its receipt references.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponAnimatedGlbSocketMaterializationLinkRecord {
    pub schema_version: String,
    pub animated_socket_materialization_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub lod0_artifact_sha256: String,
    pub source_artifact_sha256: String,
    pub source_artifact_readback_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_readback_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub request_sha256: String,
    pub socket_materialization_policy: String,
    pub lod_scope: String,
    pub socket_node_id_encoding_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub derived_animated_socket_artifact_readback_sha256: String,
    pub receipt_object_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Closed V2 socket node readback for an appearance-aware animated GLB.
/// Socket nodes remain visual, non-functional helpers; they are not physics,
/// hitbox or manufacturing semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketMaterializationV2SocketNodeRecord {
    pub socket_node_id: String,
    pub anchor_id: String,
    pub role: String,
    pub node_name: String,
    pub node_kind: String,
    pub parent_kind: String,
    pub parent_node_name: Option<String>,
    pub owner_part_id: Option<String>,
    pub local_translation_m: Vec<f64>,
    pub local_rotation_quat_xyzw: Vec<f64>,
    pub local_scale_xyz: Vec<f64>,
}

/// Immutable CAS receipt for the appearance-aware V2 animated GLB socket
/// materialization.  It retains the V1 source/derived structural readback and
/// explicitly binds the MechanicalAnimationGlb@2 key, Clip@2, sampling policy
/// and appearance-material projections.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord {
    pub schema_version: String,
    pub animated_socket_materialization_key_sha256: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub animation_glb_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_readback_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub clip_id: String,
    pub clip_object_sha256: String,
    pub clip_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub request_sha256: String,
    pub socket_materialization_policy: String,
    pub lod_scope: String,
    pub socket_node_id_encoding_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub derived_animated_socket_artifact_readback_sha256: String,
    pub source_animation_projection_sha256: String,
    pub derived_animation_projection_sha256: String,
    pub source_animation_validation_sha256: String,
    pub derived_animation_validation_sha256: String,
    pub source_renderable_inventory_sha256: String,
    pub derived_renderable_inventory_sha256: String,
    pub source_bin_sha256: String,
    pub derived_bin_sha256: String,
    pub source_appearance_material_projection_sha256: String,
    pub derived_appearance_material_projection_sha256: String,
    pub sampling_policy_sha256: String,
    pub sample_time_ticks: Vec<u64>,
    pub part_ids: Vec<String>,
    pub sampler_count: u64,
    pub channel_count: u64,
    pub node_count: u64,
    pub source_node_count: u64,
    pub derived_node_count: u64,
    pub accessor_count_added: u64,
    pub buffer_view_count_added: u64,
    pub socket_node_inventory_sha256: String,
    pub socket_node_count: u64,
    pub socket_nodes: Vec<GameWeaponAnimatedGlbSocketMaterializationV2SocketNodeRecord>,
    pub owned_cas_kinds: Vec<String>,
    pub animations_preserved: bool,
    pub channels_preserved: bool,
    pub samplers_preserved: bool,
    pub renderable_projection_exact: bool,
    pub bin_byte_exact: bool,
    pub source_static_projection_exact: bool,
    pub appearance_material_projection_exact: bool,
    pub material_pack_identity_exact: bool,
    pub no_skinning: bool,
    pub no_morph_targets: bool,
    pub socket_nodes_materialized: bool,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub production_stage_advanced: bool,
    pub actual_engine_roundtrip: bool,
    pub semantic_scope: String,
    pub functional_semantics: bool,
    pub materialization_status: String,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub limitations: Vec<String>,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Compact SQLite projection for one appearance-aware V2 animated GLB socket
/// materialization.  This field set is intentionally the 31-field Store
/// projection and does not duplicate the full receipt readback.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord {
    pub schema_version: String,
    pub animated_socket_materialization_key_sha256: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub animation_glb_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_readback_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub clip_id: String,
    pub clip_object_sha256: String,
    pub clip_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub request_sha256: String,
    pub socket_materialization_policy: String,
    pub lod_scope: String,
    pub socket_node_id_encoding_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub derived_animated_socket_artifact_readback_sha256: String,
    pub receipt_object_sha256: String,
    pub validator_status: String,
    pub hard_gate_passed: bool,
    pub materialization_status: String,
    pub quality_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponAnimatedGlbSocketMaterializationV2PrepareRequest {
    pub schema_version: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub clip_id: String,
    pub clip_object_sha256: String,
    pub clip_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub materialization_policy: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketMaterializationV2PrepareResult {
    pub schema_version: String,
    pub animated_socket_materialization_key_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub receipt_object_sha256: String,
    pub receipt: GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord,
    pub durable_link: GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub production_stage_advanced: bool,
    pub actual_engine_roundtrip: bool,
    pub quality_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponAnimatedGlbSocketMaterializationV2GetRequest {
    pub schema_version: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub clip_id: String,
    pub animated_socket_materialization_key_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketMaterializationV2GetResult {
    pub schema_version: String,
    pub animated_socket_materialization_key_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub receipt_object_sha256: String,
    pub receipt: GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord,
    pub durable_link: GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub production_stage_advanced: bool,
    pub actual_engine_roundtrip: bool,
    pub quality_status: String,
}

/// Restart-safe binding for one bounded fictional-energy VFX profile. The
/// profile is visual intent only and never represents ballistics, damage,
/// physics, manufacturing or an executed commercial-engine effect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxLinkRecord {
    pub schema_version: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub anchor_set_object_sha256: String,
    pub material_pack_manifest_sha256: String,
    pub request_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Compact durable index for one deterministic fictional-energy VFX frame.
/// The complete receipt and all nine fixed AOV PNGs remain in CAS. This row
/// exists so restart verification and garbage collection never depend on
/// scanning embedded JSON hashes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxFrameLinkRecord {
    pub schema_version: String,
    pub frame_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub source_candidate_id: String,
    pub source_artifact_sha256: String,
    pub sample_request_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub pass_object_sha256s: Vec<String>,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Compact durable index for one bounded fictional-energy VFX rendered
/// sequence. Each tick remains an immutable durable frame link; this row
/// binds the ordered tick keys and the sequence receipt without claiming an
/// engine animation, particles, trails or visual-quality result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxSequenceLinkRecord {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub source_candidate_id: String,
    pub source_artifact_sha256: String,
    pub request_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub receipt_object_sha256: String,
    pub frame_key_sha256s: Vec<String>,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Restart-safe binding for one bounded HDR bloom frame. The base nine-AOV
/// frame remains an independent durable link; this row owns only the
/// candidate-bound emissive source and post-process contribution passes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxBloomFrameLinkRecord {
    pub schema_version: String,
    pub bloom_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub source_candidate_id: String,
    pub source_artifact_sha256: String,
    pub sample_request_sha256: String,
    pub base_frame_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub bloom_profile_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub source_object_sha256: String,
    pub contribution_object_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Restart-safe binding for one deterministic typed-particle frame. The
/// particle passes are independent CAS artifacts; the link binds them to the
/// exact base nine-AOV frame, HDR bloom frame, weapon AnchorSet and LOD0 GLB
/// owner-node transform inventory used to derive the emitter positions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxParticlesFrameLinkRecord {
    pub schema_version: String,
    pub particle_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub anchor_set_object_sha256: String,
    pub source_candidate_id: String,
    pub source_artifact_sha256: String,
    pub sample_request_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub particle_seed_sha256: String,
    pub node_inventory_sha256: String,
    pub owner_world_transform_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub particle_color_object_sha256: String,
    pub particle_id_object_sha256: String,
    pub particle_depth_object_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Restart-safe binding for one deterministic typed-trail frame. Trail
/// passes are independent CAS artifacts; the link binds them to the exact
/// current/history particle receipts, base nine-AOV frame, HDR bloom frame,
/// weapon AnchorSet and LOD0 GLB owner-node transform inventory used to derive
/// the trail points.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxTrailsFrameLinkRecord {
    pub schema_version: String,
    pub trail_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub anchor_set_object_sha256: String,
    pub source_candidate_id: String,
    pub source_artifact_sha256: String,
    pub sample_request_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub current_particle_key_sha256: String,
    pub particle_history_key_sha256s: Vec<String>,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub trail_seed_sha256: String,
    pub node_inventory_sha256: String,
    pub owner_world_transform_sha256: String,
    pub trail_inventory_sha256: String,
    pub trail_id_encoding_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub trail_color_object_sha256: String,
    pub trail_id_object_sha256: String,
    pub trail_depth_object_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Restart-safe binding for one deterministic typed-trail Bloom frame. The
/// existing base, material Bloom, particle and source-trail passes remain
/// independent durable artifacts; this link owns only the two additional
/// trail-emissive and trail-bloom contribution passes plus their exact input
/// and candidate lineage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxTrailsBloomFrameLinkRecord {
    pub schema_version: String,
    pub trail_bloom_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub anchor_set_object_sha256: String,
    pub source_candidate_id: String,
    pub source_artifact_sha256: String,
    pub sample_request_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub source_trail_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub trail_bloom_profile_sha256: String,
    pub base_opaque_depth_object_sha256: String,
    pub trail_seed_sha256: String,
    pub node_inventory_sha256: String,
    pub owner_world_transform_sha256: String,
    pub trail_inventory_sha256: String,
    pub trail_id_encoding_sha256: String,
    pub source_trail_color_object_sha256: String,
    pub source_trail_id_object_sha256: String,
    pub source_trail_depth_object_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub source_object_sha256: String,
    pub contribution_object_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// One immutable frame readback for the bounded animated-socket attachment
/// slice.  The transform and emitter/trail inventories remain content
/// addressed sidecars; this typed row keeps their exact hashes together with
/// every upstream VFX frame key so restart validation can reject a retargeted
/// or reordered frame without treating a parent key as CAS.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentFrameRecord {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub animation_pose_readback_sha256: String,
    pub socket_transform_inventory_sha256: String,
    pub socket_transform_readback_sha256: String,
    pub emitter_socket_bindings_sha256: String,
    pub trail_socket_bindings_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub particle_key_sha256: String,
    pub trail_key_sha256: String,
    pub trail_bloom_key_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Durable parent binding for the bounded, structural-only animated socket
/// attachment.  A parent owns no geometry and its `attachment_key_sha256`
/// is deliberately only a canonical lookup key; the ordered frame rows carry
/// the per-sample evidence and are bounded to one through sixteen entries by
/// the JSON contract and Runtime/Store validators.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentRecord {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub source_artifact_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animated_artifact_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub vfx_sequence_key_sha256: String,
    pub vfx_sequence_canonical_sha256: String,
    pub attachment_policy: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub frame_scope: String,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketAttachmentFrameRecord>,
    pub attachment_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentPrepareRequest {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub source_artifact_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animated_artifact_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub vfx_sequence_key_sha256: String,
    pub vfx_sequence_canonical_sha256: String,
    pub attachment_policy: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub frame_scope: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentPrepareResult {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub attachment: FictionalEnergyVfxAnimatedSocketAttachmentRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentGetRequest {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentGetResult {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub attachment: FictionalEnergyVfxAnimatedSocketAttachmentRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// One projection-bound frame in the immutable Attachment@2 bridge.  Every
/// downstream key is explicit. Trail output indices are `0..=14`; their
/// current transform-projection and particle indices are `1..=15` because
/// source frame zero is retained exclusively as the bounded history pre-roll.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV2FrameRecord {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub frame_index: u64,
    pub projection_frame_index: u64,
    pub particle_sequence_frame_index: u64,
    pub sample_time_ticks: u64,
    pub animation_pose_readback_sha256: String,
    pub socket_transform_inventory_sha256: String,
    pub socket_transform_readback_sha256: String,
    pub emitter_socket_bindings_sha256: String,
    pub trail_socket_bindings_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub particle_key_sha256: String,
    pub trail_key_sha256: String,
    pub trail_bloom_key_sha256: String,
    pub projection_frame_canonical_sha256: String,
    pub particle_sequence_frame_canonical_sha256: String,
    pub trail_sequence_frame_canonical_sha256: String,
    pub trail_bloom_sequence_frame_canonical_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Projection-bound successor to Attachment@1.  V1 remains immutable and
/// sidecar-fail-closed; V2 composes one through fifteen explicit durable trail
/// output frames from the typed projection-aware animated VFX sequence stack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV2Record {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub source_artifact_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animated_artifact_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_bloom_sequence_key_sha256: String,
    pub trail_bloom_sequence_canonical_sha256: String,
    pub attachment_policy: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub frame_scope: String,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketAttachmentV2FrameRecord>,
    pub attachment_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareRequest {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub project_id: String,
    pub delivery_manifest_object_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub source_artifact_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animated_artifact_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_bloom_sequence_key_sha256: String,
    pub trail_bloom_sequence_canonical_sha256: String,
    pub attachment_policy: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub frame_scope: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV2PrepareResult {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub attachment: FictionalEnergyVfxAnimatedSocketAttachmentV2Record,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV2GetRequest {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV2GetResult {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub attachment: FictionalEnergyVfxAnimatedSocketAttachmentV2Record,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// One immutable frame in the terminal Attachment@3 bridge.  The bridge
/// owns no frame media: every field is an explicit readback/canonical binding
/// to Projection@2, Particles@2, Trails@2 or TrailsBloom@2.  Attachment frame
/// ordinals are `0..=14`; the current projection/particle source indices are
/// `1..=15`, while Trails and TrailsBloom use their output indices `0..=14`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV3FrameRecord {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub projection_frame_index: u64,
    pub particle_sequence_frame_index: u64,
    pub trail_frame_index: u64,
    pub trail_bloom_frame_index: u64,
    pub projection_frame_canonical_sha256: String,
    pub projection_socket_transform_inventory_sha256: String,
    pub projection_socket_transform_readback_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_frame_canonical_sha256: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_frame_canonical_sha256: String,
    pub trail_key_sha256: String,
    pub trail_inventory_sha256: String,
    pub trail_id_encoding_sha256: String,
    pub emitter_binding_sha256: String,
    pub trail_bloom_sequence_key_sha256: String,
    pub trail_bloom_sequence_frame_canonical_sha256: String,
    pub trail_bloom_key_sha256: String,
    pub trail_bloom_seed_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Terminal immutable Attachment@3 bridge.  It repeats both candidate
/// lineages and all upstream V2 keys so a restart read is independently
/// verifiable.  The only Attachment-owned CAS object is the canonical JSON
/// receipt identified by `attachment_receipt_object_sha256`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV3Record {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub geometry_preservation_status: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub anchor_binding_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_bloom_sequence_key_sha256: String,
    pub trail_bloom_sequence_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub trail_bloom_profile_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub attachment_policy: String,
    pub frame_scope: String,
    pub attachment_receipt_object_sha256: String,
    pub attachment_receipt_canonical_sha256: String,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketAttachmentV3FrameRecord>,
    pub attachment_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareRequest {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub geometry_preservation_status: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_bloom_sequence_key_sha256: String,
    pub trail_bloom_sequence_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub trail_bloom_profile_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub attachment_policy: String,
    pub frame_scope: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV3PrepareResult {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub attachment: FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV3GetRequest {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub appearance_candidate_id: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketAttachmentV3GetResult {
    pub schema_version: String,
    pub attachment_key_sha256: String,
    pub attachment: FictionalEnergyVfxAnimatedSocketAttachmentV3Record,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// One canonical TRS sample used by the independent animated GLB socket
/// projection.  The Runtime validates three-element translation, normalized
/// quaternion and identity scale before it serializes the bounded JSON
/// projection; matrices and shear never enter this contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionPose {
    pub translation_m: Vec<f64>,
    pub rotation_quat_xyzw: Vec<f64>,
    pub scale_xyz: Vec<f64>,
}

/// One of the six fixed non-rendering socket nodes at one sampled animation
/// tick.  `parent_world_transform` is the owning flat Part world TRS and
/// `composed_world_transform` is the deterministic parent-world * local TRS
/// result under the projection policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionSocketTransform {
    pub socket_node_id: String,
    pub anchor_id: String,
    pub role: String,
    pub node_index: u64,
    pub parent_node_index: i64,
    pub node_name: String,
    pub parent_node_name: Option<String>,
    pub node_kind: String,
    pub parent_kind: String,
    pub owner_part_id: Option<String>,
    pub local_transform: GameWeaponAnimatedGlbSocketTransformProjectionPose,
    pub parent_world_transform: GameWeaponAnimatedGlbSocketTransformProjectionPose,
    pub composed_world_transform: GameWeaponAnimatedGlbSocketTransformProjectionPose,
}

/// Bounded frame row for the six-socket transform projection.  The source and
/// derived sample digests are retained per frame so restart readback can
/// reject a retargeted GLB or reordered sample schedule without treating the
/// projection key as a CAS object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionFrame {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub source_animation_sample_sha256: String,
    pub derived_socket_sample_sha256: String,
    pub socket_transform_inventory_sha256: String,
    pub socket_transform_readback_sha256: String,
    pub socket_transforms: Vec<GameWeaponAnimatedGlbSocketTransformProjectionSocketTransform>,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Independent, reusable structural source truth for animation-driven GLB
/// socket transforms.  It deliberately does not extend the VFX Attachment@1
/// contract: VFX attachment evidence and transform replay have different
/// ownership and can be consumed independently by later Runtime gates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjection {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub source_artifact_sha256: String,
    pub source_artifact_readback_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_readback_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub derived_animated_socket_artifact_readback_sha256: String,
    pub derived_animated_socket_receipt_object_sha256: String,
    pub derived_animated_socket_receipt_canonical_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_node_inventory_sha256: String,
    pub socket_roles_sha256: String,
    pub socket_roles: Vec<String>,
    pub part_hierarchy_sha256: String,
    pub part_hierarchy_policy: String,
    pub transform_representation_policy: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub timebase_hz: u64,
    pub transform_projection_policy: String,
    pub coordinate_system: String,
    pub transform_convention: String,
    pub float_quantization_policy: String,
    pub input_sha256: String,
    pub frames: Vec<GameWeaponAnimatedGlbSocketTransformProjectionFrame>,
    pub projection_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub limitations: Vec<String>,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionPrepareRequest {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub source_artifact_sha256: String,
    pub source_artifact_readback_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_readback_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub derived_animated_socket_artifact_readback_sha256: String,
    pub derived_animated_socket_receipt_object_sha256: String,
    pub derived_animated_socket_receipt_canonical_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_node_inventory_sha256: String,
    pub socket_roles_sha256: String,
    pub socket_roles: Vec<String>,
    pub part_hierarchy_sha256: String,
    pub part_hierarchy_policy: String,
    pub transform_representation_policy: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub timebase_hz: u64,
    pub transform_projection_policy: String,
    pub coordinate_system: String,
    pub transform_convention: String,
    pub float_quantization_policy: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionPrepareResult {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection: GameWeaponAnimatedGlbSocketTransformProjection,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionGetRequest {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionGetResult {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection: GameWeaponAnimatedGlbSocketTransformProjection,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// V2 pose for the appearance-aware animated socket projection.  TRS remains
/// the semantic representation; the column-major matrix is a deterministic
/// readback aid and never carries independent shear or scale semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionV2Pose {
    pub translation_m: Vec<f64>,
    pub rotation_quat_xyzw: Vec<f64>,
    pub scale_xyz: Vec<f64>,
}

/// One of six fixed visual-only socket nodes at one sampled Clip@2 tick.
/// Local, parent-world and composed-world TRS are accompanied by their
/// column-major 4x4 matrix readbacks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionV2SocketTransform {
    pub socket_node_id: String,
    pub anchor_id: String,
    pub role: String,
    pub node_index: u64,
    pub parent_node_index: i64,
    pub node_name: String,
    pub parent_node_name: Option<String>,
    pub node_kind: String,
    pub parent_kind: String,
    pub owner_part_id: Option<String>,
    pub local_transform: GameWeaponAnimatedGlbSocketTransformProjectionV2Pose,
    pub parent_world_transform: GameWeaponAnimatedGlbSocketTransformProjectionV2Pose,
    pub composed_world_transform: GameWeaponAnimatedGlbSocketTransformProjectionV2Pose,
    pub local_matrix_4x4: Vec<f64>,
    pub parent_world_matrix_4x4: Vec<f64>,
    pub composed_world_matrix_4x4: Vec<f64>,
}

/// Bounded V2 frame.  `projection_frame_canonical_sha256` is the explicit
/// frame hash consumed by downstream projection-bound effects; the frame's
/// own canonical hash remains separate for canonical JSON persistence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionV2Frame {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub source_animation_sample_sha256: String,
    pub derived_socket_sample_sha256: String,
    pub socket_transform_inventory_sha256: String,
    pub socket_transform_readback_sha256: String,
    pub projection_frame_canonical_sha256: String,
    pub socket_transforms: Vec<GameWeaponAnimatedGlbSocketTransformProjectionV2SocketTransform>,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Additive appearance-aware V2 projection.  The owned projection report is
/// returned through the prepare/get result's `projection_object_sha256`; it is
/// deliberately not part of this record, avoiding a self-referential hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionV2 {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_glb_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_readback_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub derived_animated_socket_artifact_readback_sha256: String,
    pub derived_animated_socket_receipt_object_sha256: String,
    pub derived_animated_socket_receipt_canonical_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_node_inventory_sha256: String,
    pub socket_roles_sha256: String,
    pub socket_roles: Vec<String>,
    pub part_hierarchy_sha256: String,
    pub part_hierarchy_policy: String,
    pub transform_representation_policy: String,
    pub sampling_policy_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub timebase_hz: u64,
    pub transform_projection_policy: String,
    pub coordinate_system: String,
    pub transform_convention: String,
    pub float_quantization_policy: String,
    pub input_sha256: String,
    pub frames: Vec<GameWeaponAnimatedGlbSocketTransformProjectionV2Frame>,
    pub projection_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub limitations: Vec<String>,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionV2PrepareRequest {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub appearance_artifact_readback_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_glb_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_artifact_readback_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub derived_animated_socket_artifact_sha256: String,
    pub derived_animated_socket_artifact_readback_sha256: String,
    pub derived_animated_socket_receipt_object_sha256: String,
    pub derived_animated_socket_receipt_canonical_sha256: String,
    pub anchor_set_object_sha256: String,
    pub anchor_set_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_node_inventory_sha256: String,
    pub socket_roles_sha256: String,
    pub socket_roles: Vec<String>,
    pub part_hierarchy_sha256: String,
    pub part_hierarchy_policy: String,
    pub transform_representation_policy: String,
    pub sampling_policy_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub timebase_hz: u64,
    pub transform_projection_policy: String,
    pub coordinate_system: String,
    pub transform_convention: String,
    pub float_quantization_policy: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionV2PrepareResult {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection: GameWeaponAnimatedGlbSocketTransformProjectionV2,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionV2GetRequest {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub project_id: String,
    pub appearance_candidate_id: String,
    pub animation_clip_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameWeaponAnimatedGlbSocketTransformProjectionV2GetResult {
    pub schema_version: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection: GameWeaponAnimatedGlbSocketTransformProjectionV2,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write_performed: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// One immutable frame in the projection-driven animated socket particle
/// sequence.  The caller supplies only the projection sample and the exact
/// base/Bloom inputs; Runtime derives the emitter binding and particle CAS
/// outputs.  This remains structural evidence and does not claim visual or
/// commercial FPS quality.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame {
    pub schema_version: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub projection_frame_canonical_sha256: String,
    pub projection_socket_transform_inventory_sha256: String,
    pub projection_socket_transform_readback_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub emitter_socket_bindings_sha256: String,
    pub input_sha256: String,
    pub particle_key_sha256: String,
    pub particle_seed_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub particle_color_object_sha256: String,
    pub particle_id_object_sha256: String,
    pub particle_depth_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// The bounded input row accepted for one animated particle sample.  Derived
/// emitter bindings, seeds, render sets, receipts and pass objects are
/// deliberately absent: those values are Runtime-owned outputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput {
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub projection_frame_canonical_sha256: String,
    pub projection_socket_transform_inventory_sha256: String,
    pub projection_socket_transform_readback_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
}

/// Runtime-owned structural sequence that consumes the exact animated GLB
/// socket projection and materializes one typed particle output per sampled
/// frame.  The report CAS hash is intentionally not part of this public
/// record; Store keeps that ownership/index separately.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequence {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub source_artifact_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub particles_sequence_policy: String,
    pub emitter_binding_policy: String,
    pub transform_projection_policy: String,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketParticlesSequenceFrame>,
    pub sequence_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub source_artifact_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub particles_sequence_policy: String,
    pub emitter_binding_policy: String,
    pub transform_projection_policy: String,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketParticlesSequenceFrameInput>,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequencePrepareResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketParticlesSequence,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceGetRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceGetResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketParticlesSequence,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// V2 frame for a dual-candidate animated socket particle sequence.  Geometry
/// projection and appearance particle inputs are deliberately represented by
/// their independently bound parent records; this frame keeps the same
/// bounded, Runtime-derived output shape as V1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Frame {
    pub schema_version: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub projection_frame_canonical_sha256: String,
    pub projection_socket_transform_inventory_sha256: String,
    pub projection_socket_transform_readback_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub emitter_socket_bindings_sha256: String,
    pub input_sha256: String,
    pub particle_key_sha256: String,
    pub particle_seed_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub particle_color_object_sha256: String,
    pub particle_id_object_sha256: String,
    pub particle_depth_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Caller-supplied V2 frame input.  Particle outputs remain Runtime-owned
/// and cannot be smuggled into the prepare request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput {
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub projection_frame_canonical_sha256: String,
    pub projection_socket_transform_inventory_sha256: String,
    pub projection_socket_transform_readback_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
}

/// Runtime-owned V2 sequence joining one geometry candidate/delivery with a
/// distinct appearance candidate/delivery and the durable
/// `GameWeaponAnimatedGlbSocketTransformProjection@2` record. Its policy
/// fields are frozen by the V2 constants above; the V1 transform policy is
/// intentionally not a valid substitute. The material-surface quality
/// report is an ancestor binding; this sequence intentionally has no separate
/// owned report/receipt field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceV2 {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub geometry_preservation_status: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub anchor_binding_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub particles_sequence_policy: String,
    pub emitter_binding_policy: String,
    pub transform_projection_policy: String,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketParticlesSequenceV2Frame>,
    pub sequence_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub particles_sequence_policy: String,
    pub emitter_binding_policy: String,
    pub transform_projection_policy: String,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketParticlesSequenceV2FrameInput>,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceV2PrepareResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketParticlesSequenceV2,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceV2GetRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub appearance_candidate_id: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketParticlesSequenceV2GetResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketParticlesSequenceV2,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}
/// One semantically bound, integer-quantized trail point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailPoint {
    pub source_frame_index: u64,
    pub sample_time_ticks: u64,
    pub source_particle_key_sha256: String,
    pub source_particle_frame_index: u64,
    pub source_particle_id: u64,
    pub local_offset_micrometers: [i64; 3],
    pub world_position_micrometers: [i64; 3],
    pub depth_micrometers: u64,
}
/// One bounded trail polyline with the current point plus earlier history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrail {
    pub emitter_role: String,
    pub trail_id: u64,
    pub points: Vec<FictionalEnergyVfxAnimatedSocketTrailPoint>,
}
/// One unambiguous earlier projection/particle sample used by both trail children.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsHistorySample {
    pub history_ordinal: u64,
    pub projection_key_sha256: String,
    pub projection_frame_index: u64,
    pub projection_frame_canonical_sha256: String,
    pub projection_socket_transform_inventory_sha256: String,
    pub projection_socket_transform_readback_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_frame_index: u64,
    pub particle_key_sha256: String,
    pub particle_frame_canonical_sha256: String,
    pub sample_time_ticks: u64,
}
/// One immutable projection-bound animated socket trail frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame {
    pub schema_version: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub history_origin: String,
    pub current_projection_frame_index: u64,
    pub current_particle_frame_index: u64,
    pub current_particle_key_sha256: String,
    pub current_particle_frame_canonical_sha256: String,
    pub current_projection_frame_canonical_sha256: String,
    pub current_projection_socket_transform_inventory_sha256: String,
    pub current_projection_socket_transform_readback_sha256: String,
    pub previous_projection_frame_index: u64,
    pub previous_particle_frame_index: u64,
    pub previous_particle_sequence_frame_canonical_sha256: String,
    pub previous_projection_frame_canonical_sha256: String,
    pub previous_projection_socket_transform_inventory_sha256: String,
    pub previous_projection_socket_transform_readback_sha256: String,
    pub projection_sample_set_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub history_samples: Vec<FictionalEnergyVfxAnimatedSocketTrailsHistorySample>,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub trails: Vec<FictionalEnergyVfxAnimatedSocketTrail>,
    pub trail_key_sha256: String,
    pub trail_seed_sha256: String,
    pub trail_inventory_sha256: String,
    pub trail_id_encoding_sha256: String,
    pub emitter_binding_sha256: String,
    pub trail_color_object_sha256: String,
    pub trail_id_object_sha256: String,
    pub trail_depth_object_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}
/// Caller-supplied source bindings; history and outputs are Runtime-derived.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput {
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub history_origin: String,
    pub current_projection_frame_index: u64,
    pub current_particle_frame_index: u64,
    pub current_particle_key_sha256: String,
    pub current_particle_frame_canonical_sha256: String,
    pub current_projection_frame_canonical_sha256: String,
    pub current_projection_socket_transform_inventory_sha256: String,
    pub current_projection_socket_transform_readback_sha256: String,
    pub previous_projection_frame_index: u64,
    pub previous_particle_frame_index: u64,
    pub previous_particle_sequence_frame_canonical_sha256: String,
    pub previous_projection_frame_canonical_sha256: String,
    pub previous_projection_socket_transform_inventory_sha256: String,
    pub previous_projection_socket_transform_readback_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
}
/// Runtime-owned structural animated socket trail sequence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequence {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub source_artifact_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub trails_sequence_policy: String,
    pub history_policy: String,
    pub history_pre_roll_policy: String,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsSequenceFrame>,
    pub sequence_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}
/// Bounded trail prepare request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub source_artifact_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub trails_sequence_policy: String,
    pub history_policy: String,
    pub history_pre_roll_policy: String,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsSequenceFrameInput>,
    pub input_sha256: String,
    pub idempotency_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequencePrepareResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketTrailsSequence,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceGetRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceGetResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketTrailsSequence,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// One closed history sample for Trails@2.  Frame zero is retained as the
/// explicit pre-roll input; output frame ordinals remain `0..=14` while their
/// current Projection@2/Particles@2 sources are frames `1..=15`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsV2HistorySample {
    pub history_ordinal: u64,
    pub projection_key_sha256: String,
    pub projection_frame_index: u64,
    pub projection_frame_canonical_sha256: String,
    pub projection_socket_transform_inventory_sha256: String,
    pub projection_socket_transform_readback_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_frame_index: u64,
    pub particle_key_sha256: String,
    pub particle_frame_canonical_sha256: String,
    pub sample_time_ticks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsV2Point {
    pub source_frame_index: u64,
    pub sample_time_ticks: u64,
    pub source_particle_key_sha256: String,
    pub source_particle_frame_index: u64,
    pub source_particle_id: u64,
    pub local_offset_micrometers: [i64; 3],
    pub world_position_micrometers: [i64; 3],
    pub depth_micrometers: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsV2Trail {
    pub emitter_role: String,
    pub trail_id: u64,
    pub points: Vec<FictionalEnergyVfxAnimatedSocketTrailsV2Point>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame {
    pub schema_version: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub history_origin: String,
    pub current_projection_frame_index: u64,
    pub current_particle_frame_index: u64,
    pub current_particle_key_sha256: String,
    pub current_particle_frame_canonical_sha256: String,
    pub current_projection_frame_canonical_sha256: String,
    pub current_projection_socket_transform_inventory_sha256: String,
    pub current_projection_socket_transform_readback_sha256: String,
    pub previous_projection_frame_index: u64,
    pub previous_particle_frame_index: u64,
    pub previous_particle_sequence_frame_canonical_sha256: String,
    pub previous_projection_frame_canonical_sha256: String,
    pub previous_projection_socket_transform_inventory_sha256: String,
    pub previous_projection_socket_transform_readback_sha256: String,
    pub projection_sample_set_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub history_samples: Vec<FictionalEnergyVfxAnimatedSocketTrailsV2HistorySample>,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub trails: Vec<FictionalEnergyVfxAnimatedSocketTrailsV2Trail>,
    pub trail_key_sha256: String,
    pub trail_seed_sha256: String,
    pub trail_inventory_sha256: String,
    pub trail_id_encoding_sha256: String,
    pub emitter_binding_sha256: String,
    pub trail_color_object_sha256: String,
    pub trail_id_object_sha256: String,
    pub trail_depth_object_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceV2FrameInput {
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub history_origin: String,
    pub current_projection_frame_index: u64,
    pub current_particle_frame_index: u64,
    pub current_particle_key_sha256: String,
    pub current_particle_frame_canonical_sha256: String,
    pub current_projection_frame_canonical_sha256: String,
    pub current_projection_socket_transform_inventory_sha256: String,
    pub current_projection_socket_transform_readback_sha256: String,
    pub previous_projection_frame_index: u64,
    pub previous_particle_frame_index: u64,
    pub previous_particle_sequence_frame_canonical_sha256: String,
    pub previous_projection_frame_canonical_sha256: String,
    pub previous_projection_socket_transform_inventory_sha256: String,
    pub previous_projection_socket_transform_readback_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceV2 {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub geometry_preservation_status: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub anchor_binding_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub trails_sequence_policy: String,
    pub history_policy: String,
    pub history_pre_roll_policy: String,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsSequenceV2Frame>,
    pub sequence_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceV2PrepareRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub trails_sequence_policy: String,
    pub history_policy: String,
    pub history_pre_roll_policy: String,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsSequenceV2FrameInput>,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceV2PrepareResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceV2GetRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub appearance_candidate_id: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsSequenceV2GetResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketTrailsSequenceV2,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// One of the two fixed trail-specific Bloom contributions.  The contribution
/// digest is a canonical projection, not a fabricated particle CAS object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomV2Contribution {
    pub emitter_role: String,
    pub trail_id: u64,
    pub trail_key_sha256: String,
    pub trail_frame_canonical_sha256: String,
    pub trail_bloom_contribution_sha256: String,
}

/// One immutable Trails@2-bound animated socket Bloom frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Frame {
    pub schema_version: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub trail_frame_index: u64,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_frame_canonical_sha256: String,
    pub trail_key_sha256: String,
    pub trail_inventory_sha256: String,
    pub trail_id_encoding_sha256: String,
    pub emitter_binding_sha256: String,
    pub trail_color_object_sha256: String,
    pub trail_id_object_sha256: String,
    pub trail_depth_object_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_frame_canonical_sha256: String,
    pub current_projection_frame_index: u64,
    pub current_particle_frame_index: u64,
    pub current_projection_frame_canonical_sha256: String,
    pub current_projection_socket_transform_inventory_sha256: String,
    pub current_projection_socket_transform_readback_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub trail_bloom_profile_sha256: String,
    pub base_opaque_depth_object_sha256: String,
    pub base_aov_byte_exact_verified: bool,
    pub base_opaque_depth_byte_exact_reused: bool,
    pub bloom_pass_byte_exact_reused: bool,
    pub particle_passes_byte_exact_reused: bool,
    pub trail_passes_byte_exact_reused: bool,
    pub base_bloom_mutated: bool,
    pub particle_passes_mutated: bool,
    pub trail_passes_mutated: bool,
    pub trail_bloom_input: bool,
    pub trail_emissive_source_rendered: bool,
    pub trail_bloom_contribution_rendered: bool,
    pub trail_bloom_rendered: bool,
    pub trail_bloom_key_sha256: String,
    pub trail_bloom_seed_sha256: String,
    pub trail_bloom_contributions: Vec<FictionalEnergyVfxAnimatedSocketTrailsBloomV2Contribution>,
    pub trail_emissive_source_object_sha256: String,
    pub trail_bloom_contribution_object_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Caller-supplied source bindings.  Derived Bloom passes, contribution rows,
/// render-set/receipt hashes and canonical output are Runtime-owned.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2FrameInput {
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub trail_frame_index: u64,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_frame_canonical_sha256: String,
    pub trail_key_sha256: String,
    pub trail_inventory_sha256: String,
    pub trail_id_encoding_sha256: String,
    pub emitter_binding_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_frame_canonical_sha256: String,
    pub current_projection_frame_index: u64,
    pub current_particle_frame_index: u64,
    pub current_projection_frame_canonical_sha256: String,
    pub current_projection_socket_transform_inventory_sha256: String,
    pub current_projection_socket_transform_readback_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
}

/// Runtime-owned structural TrailsBloom@2 sequence.  The parent repeats the
/// complete dual-candidate/material lineage so a restart read never depends
/// on V1's single-candidate semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2 {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub geometry_preservation_projection_sha256: String,
    pub geometry_preservation_status: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub anchor_binding_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub trails_bloom_sequence_policy: String,
    pub history_policy: String,
    pub history_pre_roll_policy: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_key_scope: String,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub trail_bloom_profile_sha256: String,
    pub trail_bloom_profile: Value,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2Frame>,
    pub sequence_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub geometry_candidate_state_sha256: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub geometry_artifact_sha256: String,
    pub appearance_candidate_id: String,
    pub appearance_candidate_state_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
    pub appearance_artifact_sha256: String,
    pub material_surface_quality_id: String,
    pub material_surface_quality_report_object_sha256: String,
    pub material_surface_quality_canonical_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub particle_sequence_key_sha256: String,
    pub particle_sequence_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub appearance_anchor_set_object_sha256: String,
    pub appearance_anchor_set_canonical_sha256: String,
    pub anchor_binding_policy: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub trails_bloom_sequence_policy: String,
    pub history_policy: String,
    pub history_pre_roll_policy: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_key_scope: String,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub trail_bloom_profile_sha256: String,
    pub trail_bloom_profile: Value,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2FrameInput>,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2PrepareResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2GetRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub geometry_candidate_id: String,
    pub appearance_candidate_id: String,
    pub geometry_delivery_manifest_object_sha256: String,
    pub appearance_delivery_manifest_object_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2GetResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceV2,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// One animated trail Bloom frame with exact upstream passes and two new additive outputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame {
    pub schema_version: String,
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_frame_canonical_sha256: String,
    pub trail_color_object_sha256: String,
    pub trail_id_object_sha256: String,
    pub trail_depth_object_sha256: String,
    pub particle_sequence_frame_canonical_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub trail_bloom_profile_sha256: String,
    pub base_opaque_depth_object_sha256: String,
    pub base_aov_byte_exact_verified: bool,
    pub base_opaque_depth_byte_exact_reused: bool,
    pub bloom_pass_byte_exact_reused: bool,
    pub particle_passes_byte_exact_reused: bool,
    pub trail_passes_byte_exact_reused: bool,
    pub base_bloom_mutated: bool,
    pub particle_passes_mutated: bool,
    pub trail_passes_mutated: bool,
    pub trail_bloom_input: bool,
    pub trail_emissive_source_rendered: bool,
    pub trail_bloom_contribution_rendered: bool,
    pub trail_bloom_rendered: bool,
    pub trail_bloom_key_sha256: String,
    pub trail_bloom_seed_sha256: String,
    pub trail_emissive_source_object_sha256: String,
    pub trail_bloom_contribution_object_sha256: String,
    pub render_set_object_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}
/// Caller-supplied trail/particle/base/Bloom and render bindings only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput {
    pub frame_index: u64,
    pub sample_time_ticks: u64,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_frame_canonical_sha256: String,
    pub particle_sequence_frame_canonical_sha256: String,
    pub base_frame_key_sha256: String,
    pub bloom_key_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
}
/// Runtime-owned structural animated trail Bloom sequence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequence {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub source_artifact_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub trails_bloom_sequence_policy: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_key_scope: String,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub trail_bloom_profile_sha256: String,
    pub trail_bloom_profile: Value,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrame>,
    pub sequence_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub restart_hash_verified: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub input_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}
/// Bounded trail Bloom prepare request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub delivery_manifest_object_sha256: String,
    pub source_artifact_sha256: String,
    pub projection_key_sha256: String,
    pub projection_object_sha256: String,
    pub projection_canonical_sha256: String,
    pub animated_socket_materialization_key_sha256: String,
    pub animated_artifact_sha256: String,
    pub animated_socket_anchor_set_object_sha256: String,
    pub animated_socket_anchor_set_canonical_sha256: String,
    pub animation_clip_id: String,
    pub animation_clip_object_sha256: String,
    pub animation_clip_canonical_sha256: String,
    pub animation_receipt_object_sha256: String,
    pub animation_receipt_canonical_sha256: String,
    pub vfx_profile_object_sha256: String,
    pub vfx_profile_canonical_sha256: String,
    pub socket_node_id_encoding_sha256: String,
    pub socket_roles_sha256: String,
    pub camera_object_sha256: String,
    pub camera_identity_sha256: String,
    pub render_profile_sha256: String,
    pub render_worker_build_cohort_sha256: String,
    pub sample_schedule_sha256: String,
    pub sample_count: u64,
    pub sample_time_ticks: Vec<u64>,
    pub frame_scope: String,
    pub trails_bloom_sequence_policy: String,
    pub trail_sequence_key_sha256: String,
    pub trail_sequence_canonical_sha256: String,
    pub trail_key_scope: String,
    pub trail_count: u64,
    pub trail_emitter_roles: Vec<String>,
    pub trail_bloom_profile_sha256: String,
    pub trail_bloom_profile: Value,
    pub frames: Vec<FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceFrameInput>,
    pub input_sha256: String,
    pub idempotency_key: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequencePrepareResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketTrailsBloomSequence,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetRequest {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub project_id: String,
    pub candidate_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FictionalEnergyVfxAnimatedSocketTrailsBloomSequenceGetResult {
    pub schema_version: String,
    pub sequence_key_sha256: String,
    pub sequence: FictionalEnergyVfxAnimatedSocketTrailsBloomSequence,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub commercial_fps_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub actual_engine_roundtrip: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// Compact SQLite index for one immutable candidate-bound Appearance source
/// lineage sidecar.  The complete source program, MaterialPack manifest,
/// TextureBuild receipt and the per-LOD binding inventory remain in CAS;
/// SQLite keeps the exact hashes needed for restart lookup, reachability and
/// conflict detection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppearanceSourceLineageLinkRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub source_replay_worker_cohort_sha256: String,
    pub appearance_program_schema_version: String,
    pub appearance_program_object_sha256: String,
    pub appearance_program_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub material_layer_stack_sha256: Option<String>,
    pub material_pack_id: String,
    pub material_pack_version: String,
    pub material_pack_license_spdx: String,
    pub material_pack_provenance_sha256: String,
    pub material_pack_manifest_object_sha256: String,
    pub material_pack_manifest_sha256: String,
    pub texture_build_receipt_object_sha256: String,
    pub texture_build_receipt_sha256: String,
    pub candidate_surface_bake_receipt_object_sha256: Option<String>,
    pub candidate_surface_bake_receipt_sha256: Option<String>,
    pub uv_binding_sha256: String,
    pub lod_candidate_ids: Vec<String>,
    pub lod_candidate_state_sha256s: Vec<String>,
    pub lod_artifact_sha256s: Vec<String>,
    pub lod_artifact_readback_sha256s: Vec<String>,
    pub lod_artifact_readback_object_sha256s: Vec<String>,
    pub lod_part_binding_inventory_sha256s: Vec<String>,
    pub request_sha256: String,
    pub sidecar_object_sha256: String,
    pub materialization_status: String,
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
