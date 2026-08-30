use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

mod authoring_mesh_v2;
mod fps_presentation_package_v2;
mod fps_presentation_package_v2_candidate;
mod low_quad_durable;
mod production_weapon_form_art_baseline;
mod production_weapon_formal_high;
mod production_weapon_owner_reviewed_void_calibration;
mod weapon_foundation_authoring_materialization;
mod weapon_foundation_typed_importer;
pub mod weaponry_domain_map;
pub use authoring_mesh_v2::*;
pub use fps_presentation_package_v2::*;
pub use fps_presentation_package_v2_candidate::*;
pub use low_quad_durable::*;
pub use production_weapon_form_art_baseline::*;
pub use production_weapon_formal_high::*;
pub use production_weapon_owner_reviewed_void_calibration::*;
pub use weapon_foundation_authoring_materialization::*;
pub use weapon_foundation_typed_importer::*;
pub use weaponry_domain_map::*;

pub const CONTRACT_SET: &str = "forgecad-runtime-contracts@1";
pub const AUTHORING_MESH_EDIT_PREVIEW_REQUEST_SCHEMA_VERSION: &str =
    "AuthoringMeshEditPreviewRequest@1";
pub const AUTHORING_MESH_EDIT_PREVIEW_SCHEMA_VERSION: &str = "AuthoringMeshEditPreview@1";
pub const AUTHORING_MESH_EDIT_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "AuthoringMeshEditPrepareRequest@1";
pub const AUTHORING_MESH_EDIT_PREPARE_SCHEMA_VERSION: &str = "AuthoringMeshEditPrepare@1";
pub const AUTHORING_MESH_TOPOLOGY_EDIT_OPERATIONS: &[&str] =
    &["split_edge", "collapse_edge", "dissolve_edge"];
pub const AUTHORING_MESH_TOPOLOGY_CORRESPONDENCE_KINDS: &[&str] = &["one-to-many", "many-to-one"];
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

/// Closed correspondence emitted for one typed topology edit.  A generic
/// mesh delta, script, selection history or command text is deliberately not
/// representable here: the Runtime must state the exact authoring identities
/// retired and created by the bounded operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshTopologyCorrespondence {
    pub kind: String,
    pub parent_source_element_ids: Vec<String>,
    pub child_source_element_ids: Vec<String>,
    pub operation_lineage_sha256: String,
    pub identity_namespace_status: String,
}

/// Closed monotonic tombstone carried by a typed topology edit binding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshTopologyTombstone {
    pub source_element_id: String,
    pub element_kind: String,
    pub retired_revision_index: u64,
    pub operation_lineage_sha256: String,
    pub reason: String,
}

/// Closed typed operation proof emitted in `edited_element_ids` for the three
/// topology operations.  Its namespace is deliberately source-element-only;
/// it is not an AuthoringMesh IdentityLineage materialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshTopologyOperationProof {
    pub schema_version: String,
    pub operation: String,
    pub parent_revision: u64,
    pub child_revision: u64,
    pub operation_lineage_sha256: String,
    pub source_vertex_ids: Vec<String>,
    pub source_edge_ids: Vec<String>,
    pub source_face_ids: Vec<String>,
    pub generated_vertex_ids: Vec<String>,
    pub generated_edge_ids: Vec<String>,
    pub generated_loop_ids: Vec<String>,
    pub generated_face_ids: Vec<String>,
    pub retired_vertex_ids: Vec<String>,
    pub retired_edge_ids: Vec<String>,
    pub retired_loop_ids: Vec<String>,
    pub retired_face_ids: Vec<String>,
    pub tombstones: Vec<AuthoringMeshTopologyTombstone>,
    pub correspondence: Vec<AuthoringMeshTopologyCorrespondence>,
    pub identity_namespace_status: String,
    pub canonical_sha256: String,
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

/// ProductionStage@3 is an additive, fine-grained production axis.  The
/// historical V1/V2 stage enums remain frozen; Runtime/Store implementations
/// must use these constants for the V3 closed set instead of widening the old
/// contracts in place.
pub const PRODUCTION_STAGE_V3_SCHEMA_VERSION: &str = "ProductionStageTransition@3";
pub const PRODUCTION_STAGE_HEAD_V3_SCHEMA_VERSION: &str = "ProductionStageHead@3";
pub const PRODUCTION_STAGE_COMPATIBILITY_PROJECTION_V3_SCHEMA_VERSION: &str =
    "ProductionStageCompatibilityProjection@3";
pub const PRODUCTION_STAGE_V3_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionStageTransitionPrepareRequest@3";
pub const PRODUCTION_STAGE_V3_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionStageTransitionPrepareResult@3";
pub const PRODUCTION_STAGE_V3_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionStageTransitionGetRequest@3";
pub const PRODUCTION_STAGE_V3_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionStageTransitionGetResult@3";
pub const PRODUCTION_STAGE_V3_FIRST_FROM_STAGE: &str = "reference-intake";
pub const PRODUCTION_STAGE_V3_FIRST_TO_STAGE: &str = "reference-coverage-reviewed";
pub const PRODUCTION_STAGE_V3_CAMERA_FROM_STAGE: &str = "reference-coverage-reviewed";
pub const PRODUCTION_STAGE_V3_CAMERA_TO_STAGE: &str = "camera-calibrated";
pub const PRODUCTION_STAGE_V3_FORM_EDGES: &[(&str, &str)] = &[
    ("camera-calibrated", "blockout-reviewed"),
    ("blockout-reviewed", "primary-form-approved"),
    ("primary-form-approved", "secondary-form-approved"),
];
pub const PRODUCTION_STAGE_V3_FORM_QUALITY_RECEIPT_KIND: &str = "ProductionWeaponFormQuality@2";
pub const PRODUCTION_STAGE_V3_FORM_ART_RECEIPT_KIND: &str = "ProductionWeaponFormArtEvidence@1";
pub const PRODUCTION_STAGE_V3_CAMERA_LOCK_SCHEMA_VERSION: &str = "ProductionCameraLock@1";
pub const PRODUCTION_STAGE_V3_CAMERA_LOCK_POLICY: &str =
    "fps-weapon-reviewed-six-reference-seven-camera-lock@1";
pub const PRODUCTION_STAGE_V3_CAMERA_BINDING_FIELDS: &[&str] = &[
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "camera_rig_object_sha256",
    "camera_rig_canonical_sha256",
    "camera_lock_receipt_object_sha256",
    "camera_lock_source_transition_id",
    "camera_lock_source_transition_sha256",
    "camera_lock_source_head_canonical_sha256",
];
pub const PRODUCTION_STAGE_V3_FIRST_EDGE_NULL_EVIDENCE_FIELDS: &[&str] = &[
    "quality_report_object_sha256",
    "comparison_report_object_sha256",
    "visual_receipt_object_sha256",
    "human_review_receipt_object_sha256",
    "engine_validation_receipt_object_sha256",
    "distribution_receipt_object_sha256",
];

/// Additive structural form-quality receipt for the three independent
/// blockout/primary/secondary form gates.  The same contract is intentionally
/// reused for each fine-grained Stage@3 edge; it never advances the stage head.
pub const PRODUCTION_WEAPON_FORM_QUALITY_SCHEMA_VERSION: &str = "ProductionWeaponFormQuality@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityPrepareRequest@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityPrepareResult@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityGetRequest@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityGetResult@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_POLICY: &str =
    "production-weapon-form-quality-six-view-no-regression@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_THRESHOLD_POLICY: &str =
    "production-weapon-form-view-thresholds@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_FORM_STAGES: &[&str] =
    &["blockout", "primary", "secondary"];
pub const PRODUCTION_WEAPON_FORM_QUALITY_SOURCE_STAGES: &[&str] = &[
    "camera-calibrated",
    "blockout-reviewed",
    "primary-form-approved",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_TARGET_STAGES: &[&str] = &[
    "blockout-reviewed",
    "primary-form-approved",
    "secondary-form-approved",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_REVIEWED_REFERENCE_VIEW_KINDS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_FIXED_CAMERA_VIEW_KINDS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "top",
    "bottom",
    "rear-three-quarter",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_CAMERA_CALIBRATED_HEAD_FIELDS: &[&str] = &[
    "camera_calibrated_head_transition_id",
    "camera_calibrated_head_transition_sha256",
    "camera_calibrated_head_canonical_sha256",
    "camera_calibrated_head_candidate_id",
    "camera_calibrated_head_candidate_state_sha256",
    "camera_calibrated_head_artifact_id",
    "camera_calibrated_head_artifact_sha256",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_CROSS_VIEW_EVIDENCE_FIELDS: &[&str] = &[
    "cross_view_evidence_object_sha256",
    "cross_view_evidence_canonical_sha256",
    "cross_view_evidence_view_kinds",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_TYPED_EVIDENCE_FIELDS: &[&str] = &[
    "form_evidence_object_sha256",
    "form_evidence_canonical_sha256",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_EVIDENCE_SOURCE_KINDS: &[&str] =
    &["cross-view-evidence-bundle", "design-spec", "not-proven"];

/// Additive @2 form gate.  @1 remains the immutable legacy CrossView/metrics
/// report; @2 consumes that report plus ProductionWeaponFormArtEvidence@1 and
/// records only a passing, structure-only decision for one Stage@3 form edge.
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_SCHEMA_VERSION: &str = "ProductionWeaponFormQuality@2";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityView@2";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityPrepareRequest@2";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityPrepareResult@2";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityGetRequest@2";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityGetResult@2";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_POLICY: &str =
    "production-weapon-form-quality-six-view-art-evidence-gate@2";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_THRESHOLD_POLICY: &str =
    "production-weapon-form-view-thresholds@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_FORM_STAGES: &[&str] =
    &["blockout", "primary", "secondary"];
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_SOURCE_STAGES: &[&str] = &[
    "camera-calibrated",
    "blockout-reviewed",
    "primary-form-approved",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_TARGET_STAGES: &[&str] = &[
    "blockout-reviewed",
    "primary-form-approved",
    "secondary-form-approved",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_VIEW_KINDS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_FIXED_CAMERA_VIEW_KINDS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "top",
    "bottom",
    "rear-three-quarter",
];
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_VALIDATOR_STATUS: &str = "passed";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_STRUCTURAL_STATUS: &str = "PASS_SOURCE_STRUCTURAL";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_VISUAL_STATUS: &str =
    "PASS_STAGE_VISUAL_STRUCTURE_ONLY";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_HUMAN_STATUS: &str = "NOT_RUN";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_ENGINE_STATUS: &str = "NOT_RUN";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_DISTRIBUTION_STATUS: &str = "NOT_RUN";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_QUALITY_STATUS: &str = "PASS_FORM_GATE";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityV2PreflightGetRequest@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormQualityV2PreflightGetResult@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_POLICY: &str =
    "production-weapon-form-quality-v2-preflight-readiness-gate@1";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_QUALITY_STATUS: &str = "NOT_PROVEN";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_VISUAL_QUALITY_STATUS: &str = "NOT_PROVEN";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_HUMAN_REVIEW_STATUS: &str = "NOT_RUN";
pub const PRODUCTION_WEAPON_FORM_QUALITY_V2_PREFLIGHT_COMMERCIAL_ENGINE_STATUS: &str = "NOT_RUN";

/// Runtime-owned per-view artistic evidence used by the non-promoting form
/// quality receipt.  One parent contains exactly the six reviewed views and
/// each child carries all three independent typed observations.  The parent
/// and every child remain candidate/artifact/reference/camera/render-set
/// bound; this contract never compiles geometry or advances ProductionStage.
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_SCHEMA_VERSION: &str = "ProductionWeaponFormEvidence@1";
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_SCHEMA_VERSION: &str =
    "ProductionWeaponFormEvidenceView@1";
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormEvidencePrepareRequest@1";
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormEvidencePrepareResult@1";
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormEvidenceGetRequest@1";
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormEvidenceGetResult@1";
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_POLICY: &str =
    "production-weapon-form-evidence-six-view-typed-observation@1";
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_KINDS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_KINDS: &[&str] =
    &["part-id", "negative-space", "line-flow"];
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_OBSERVATION_STATUSES: &[&str] =
    &["observed", "inferred", "unknown"];
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_QUALITY_STATUS: &str = "NOT_PROVEN";
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_PARENT_RECEIPT_KIND: &str =
    "production-weapon-form-evidence-receipt";
pub const PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_RECEIPT_KIND: &str =
    "production-weapon-form-evidence-view-receipt";

/// Additive Runtime-owned art-observation evidence for the six-view form
/// gate.  This family intentionally does not change FormEvidence@1,
/// FormQuality@1 or ProductionStage@3 semantics: it records the typed
/// target/AOV comparison needed by a future quality consumer, while quality
/// remains NOT_PROVEN and no stage side effect is permitted.
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtEvidence@1";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtEvidenceView@1";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtEvidencePrepareRequest@1";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtEvidencePrepareResult@1";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtEvidenceGetRequest@1";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtEvidenceGetResult@1";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_POLICY: &str =
    "production-weapon-form-art-evidence-six-view-typed-observation@1";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_KINDS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_QUALITY_STATUS: &str = "NOT_PROVEN";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PARENT_RECEIPT_KIND: &str =
    "production-weapon-form-art-evidence-receipt";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_RECEIPT_KIND: &str =
    "production-weapon-form-art-evidence-view-receipt";
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_IOU_MIN_MILLI: u64 = 850;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_BOUNDARY_F1_MIN_MILLI: u64 = 800;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_AREA_RATIO_MIN_MILLI: u64 = 850;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_AREA_RATIO_MAX_MILLI: u64 = 1150;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VOID_CENTROID_MAX_MILLI: u64 = 3000;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_COVERAGE_MIN_MILLI: u64 = 900;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_CONTINUITY_MIN_MILLI: u64 = 900;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_CHAMFER_MAX_MILLI: u64 = 3000;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_DEVIATION_MAX_MILLI: u64 = 5000;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_DIRECTION_MIN_MILLI: u64 = 950;
pub const PRODUCTION_WEAPON_FORM_ART_EVIDENCE_LINE_DUPLICATE_CROSSING_MAX: u64 = 0;

/// Closed assembly-level decision vocabulary for the first art-decision
/// projection.  The registry is deliberately smaller than the 23-part
/// structural fixture: it names only the coupled form groups that can be
/// searched without turning a single-Part optimizer into an implicit
/// assembly editor.
pub const PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_SCHEMA_VERSION: &str =
    "ProductionWeaponAssemblyDecisionRegistry@1";
pub const PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_POLICY: &str =
    "fps-weapon-closed-assembly-form-decision-registry@1";
pub const PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_PROFILE_ID: &str =
    "fps-weapon-form-assembly@1";
pub const PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_GROUP_IDS: &[&str] = &[
    "receiver-envelope",
    "muzzle-axis",
    "stock-open-frame",
    "trigger-void",
    "rail-spine",
];
pub const PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_COUPLING_MODES: &[&str] =
    &["independent", "linked", "mirror"];
pub const PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_INVARIANTS: &[&str] = &[
    "shared-axis",
    "coaxial",
    "mirror-symmetric",
    "clearance-min",
    "enclosed-void",
    "continuous-spine",
];
pub const PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_VIEW_KINDS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];

/// Runtime-owned, product-defined aggregate mutators for the first parameter
/// sink slice.  These are identifiers for implemented typed Runtime code, not
/// a user supplied path or executable descriptor.
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_SCHEMA_VERSION: &str =
    "ProductionWeaponAssemblyParameterSinkRegistry@1";
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponAssemblyParameterSinkGetRequest@1";
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponAssemblyParameterSinkGetResult@1";
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_POLICY: &str =
    "fps-weapon-product-owned-aggregate-parameter-sink-registry@1";
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_SUPPORTED_GROUP_IDS: &[&str] =
    &["receiver-envelope", "muzzle-axis", "stock-open-frame"];
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_SUPPORTED_PARAMETER_IDS: &[&str] = &[
    "receiver-envelope-width",
    "receiver-envelope-height",
    "receiver-envelope-shoulder",
    "muzzle-axis-shroud-envelope",
    "muzzle-axis-emitter-envelope",
    "muzzle-axis-core-aperture",
    "stock-open-frame-clearance",
    "stock-open-frame-angle",
];
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_UNAVAILABLE_PARAMETER_IDS: &[&str] = &[
    "trigger-void-clearance",
    "trigger-void-centroid",
    "rail-spine-continuity",
    "rail-spine-offset",
];
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_MUTATOR_IDS: &[&str] = &[
    "forgecad.assembly.mutator.receiver-envelope@1",
    "forgecad.assembly.mutator.muzzle-axis@1",
    "forgecad.assembly.mutator.stock-open-frame@1",
];
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_STATUS: &[&str] =
    &["PARTIAL_TYPED_SINKS", "READY"];
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_UNITS: &[&str] =
    &["meter", "radian", "ratio"];
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_APPLICATION_STATUS: &str = "AVAILABLE";
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_EVIDENCE_REQUIREMENTS: &[&str] = &[
    "assembly-registry",
    "geometry-program",
    "operator-catalog",
    "artifact-readback",
    "candidate-state",
];
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_STRUCTURAL_STATUS: &str = "structural_only";
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_VISUAL_STATUS: &str = "NOT_PROVEN";
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_HUMAN_STATUS: &str = "NOT_RUN";
pub const PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_ENGINE_STATUS: &str = "NOT_RUN";

/// Read-only art-decision proposal projection.  The request is intentionally
/// hash/id-only (apart from nullable first-person evidence); the result may
/// explain blockers but cannot create geometry, invoke a Worker, or promote a
/// candidate.  This is a proposal surface, not a Runtime mutation API.
pub const PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponArtDecisionProposalGetRequest@1";
pub const PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponArtDecisionProposalGetResult@1";
pub const PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_OBJECTIVE_POLICY: &str =
    "assembly-form-search-negative-space-line-flow-first-person@1";
pub const PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GATE_IDS: &[&str] = &[
    "lineage",
    "reference-annotation",
    "camera",
    "assembly-registry",
    "parameter-sink",
    "negative-space",
    "line-flow",
    "first-person-readability",
    "candidate-search-critic",
    "surface-scope",
];
pub const PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_GATE_STATUSES: &[&str] =
    &["PASS", "BLOCKED", "NOT_RUN", "LOCKED"];
pub const PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_STATUSES: &[&str] = &[
    "READY_ASSEMBLY_FORM_SEARCH",
    "BLOCKED_LINEAGE",
    "BLOCKED_REFERENCE_ANNOTATION",
    "BLOCKED_CAMERA",
    "BLOCKED_NEGATIVE_SPACE",
    "BLOCKED_LINE_FLOW",
    "BLOCKED_FIRST_PERSON_PROFILE",
    "BLOCKED_ASSEMBLY_REGISTRY",
    "BLOCKED_PARAMETER_SINK",
    "NO_STRICT_MULTI_VIEW_IMPROVEMENT",
];
pub const PRODUCTION_WEAPON_ART_DECISION_PROPOSAL_BLOCKER_CODES: &[&str] = &[
    "BLOCKED_LINEAGE",
    "BLOCKED_REFERENCE_ANNOTATION",
    "BLOCKED_CAMERA",
    "BLOCKED_NEGATIVE_SPACE",
    "BLOCKED_LINE_FLOW",
    "BLOCKED_FIRST_PERSON_PROFILE",
    "BLOCKED_ASSEMBLY_REGISTRY",
    "BLOCKED_PARAMETER_SINK",
    "NO_STRICT_MULTI_VIEW_IMPROVEMENT",
];

pub const PRODUCTION_STAGE_V3_STAGES: &[&str] = &[
    "reference-intake",
    "reference-coverage-reviewed",
    "camera-calibrated",
    "blockout-reviewed",
    "primary-form-approved",
    "secondary-form-approved",
    "high-poly-approved",
    "low-poly-approved",
    "uv-approved",
    "cage-approved",
    "bake-approved",
    "material-approved",
    "rig-socket-approved",
    "animation-approved",
    "vfx-approved",
    "lod-collision-approved",
    "hero-art-review-approved",
    "engine-validated",
    "export-confirmed",
];

pub const PRODUCTION_STAGE_V3_STRUCTURAL_STATUSES: &[&str] =
    &["NOT_RUN", "BLOCKED", "PASS_SOURCE_STRUCTURAL"];
pub const PRODUCTION_STAGE_V3_VISUAL_STATUSES: &[&str] = &[
    "NOT_RUN",
    "BLOCKED",
    "QUALITY_TARGET_NOT_MET",
    "PASS_STAGE_VISUAL",
    "PASS_STAGE_VISUAL_STRUCTURE_ONLY",
];
pub const PRODUCTION_STAGE_V3_HUMAN_STATUSES: &[&str] =
    &["NOT_RUN", "BLOCKED", "REJECTED", "PASS_HUMAN_ART_REVIEW"];
pub const PRODUCTION_STAGE_V3_ENGINE_STATUSES: &[&str] =
    &["NOT_RUN", "BLOCKED", "FAILED", "PASS_ENGINE_VALIDATION"];
pub const PRODUCTION_STAGE_V3_DISTRIBUTION_STATUSES: &[&str] =
    &["NOT_RUN", "BLOCKED", "FAILED", "PASS_DISTRIBUTION"];

pub fn production_stage_v3_index(stage: &str) -> Option<usize> {
    PRODUCTION_STAGE_V3_STAGES
        .iter()
        .position(|value| *value == stage)
}

pub fn is_production_stage_v3_stage(stage: &str) -> bool {
    production_stage_v3_index(stage).is_some()
}

pub fn production_stage_v3_is_adjacent(from_stage: &str, to_stage: &str) -> bool {
    matches!(
        (production_stage_v3_index(from_stage), production_stage_v3_index(to_stage)),
        (Some(from), Some(to)) if to == from + 1
    )
}

pub fn production_stage_v3_is_first_public_edge(from_stage: &str, to_stage: &str) -> bool {
    from_stage == PRODUCTION_STAGE_V3_FIRST_FROM_STAGE
        && to_stage == PRODUCTION_STAGE_V3_FIRST_TO_STAGE
}

pub fn production_stage_v3_is_camera_calibration_edge(from_stage: &str, to_stage: &str) -> bool {
    from_stage == PRODUCTION_STAGE_V3_CAMERA_FROM_STAGE
        && to_stage == PRODUCTION_STAGE_V3_CAMERA_TO_STAGE
}

pub fn production_stage_v3_is_form_edge(from_stage: &str, to_stage: &str) -> bool {
    PRODUCTION_STAGE_V3_FORM_EDGES
        .iter()
        .any(|(from, to)| *from == from_stage && *to == to_stage)
}

/// The V3 head is intentionally a projection object.  It records lossy
/// coarse-stage views without mutating either historical V1 or dual-candidate
/// V2 heads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageCompatibilityProjectionV3 {
    pub schema_version: String,
    pub source_schema_version: String,
    pub v3_stage: Option<String>,
    pub v3_stage_complete: bool,
    pub v1_projection_stage: Option<String>,
    pub v1_projection_complete: bool,
    pub v2_projection_stage: Option<String>,
    pub v2_projection_complete: bool,
    pub projection_status: String,
    pub legacy_head_transition_id: Option<String>,
    pub legacy_head_transition_sha256: Option<String>,
    pub projection_policy_sha256: String,
}

/// Fine-grained immutable production transition.  The fields deliberately
/// mirror the V2 lineage shape while adding the 19-stage edge, five status
/// dimensions and receipt bindings.  `parent_transition_id` is nullable for
/// the first executable edge; no synthetic seed transition is required.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV3Record {
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
    pub candidate_binding_status: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub camera_lock_id: Option<String>,
    pub camera_lock_canonical_sha256: Option<String>,
    pub camera_rig_object_sha256: Option<String>,
    pub camera_rig_canonical_sha256: Option<String>,
    pub camera_lock_receipt_object_sha256: Option<String>,
    pub camera_lock_source_transition_id: Option<String>,
    pub camera_lock_source_transition_sha256: Option<String>,
    pub camera_lock_source_head_canonical_sha256: Option<String>,
    pub evidence_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub quality_report_object_sha256: Option<String>,
    pub comparison_report_object_sha256: Option<String>,
    pub design_spec_object_sha256: String,
    pub visual_receipt_object_sha256: Option<String>,
    pub human_review_receipt_object_sha256: Option<String>,
    pub engine_validation_receipt_object_sha256: Option<String>,
    pub distribution_receipt_object_sha256: Option<String>,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub approval_receipt_id: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub approval_summary_sha256: String,
    pub request_key_sha256: String,
    pub parent_transition_id: Option<String>,
    pub parent_transition_sha256: Option<String>,
    pub parent_transition_schema_version: Option<String>,
    pub gate_status: String,
    pub status: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Durable V3 head projection.  It is separate from the V1 single-candidate
/// head and V2 topology/material-surface head; those records remain immutable
/// and are exposed only through `compatibility_projection`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageHeadV3Record {
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
    pub candidate_binding_status: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub camera_lock_id: Option<String>,
    pub camera_lock_canonical_sha256: Option<String>,
    pub camera_rig_object_sha256: Option<String>,
    pub camera_rig_canonical_sha256: Option<String>,
    pub camera_lock_receipt_object_sha256: Option<String>,
    pub camera_lock_source_transition_id: Option<String>,
    pub camera_lock_source_transition_sha256: Option<String>,
    pub camera_lock_source_head_canonical_sha256: Option<String>,
    pub evidence_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub quality_report_object_sha256: Option<String>,
    pub comparison_report_object_sha256: Option<String>,
    pub design_spec_object_sha256: String,
    pub visual_receipt_object_sha256: Option<String>,
    pub human_review_receipt_object_sha256: Option<String>,
    pub engine_validation_receipt_object_sha256: Option<String>,
    pub distribution_receipt_object_sha256: Option<String>,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub approval_receipt_id: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub approval_summary_sha256: String,
    pub head_transition_id: String,
    pub head_transition_sha256: String,
    pub compatibility_projection: ProductionStageCompatibilityProjectionV3,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub payload_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV3PrepareRequest {
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
    pub candidate_binding_status: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub camera_lock_id: Option<String>,
    pub camera_lock_canonical_sha256: Option<String>,
    pub camera_rig_object_sha256: Option<String>,
    pub camera_rig_canonical_sha256: Option<String>,
    pub camera_lock_receipt_object_sha256: Option<String>,
    pub camera_lock_source_transition_id: Option<String>,
    pub camera_lock_source_transition_sha256: Option<String>,
    pub camera_lock_source_head_canonical_sha256: Option<String>,
    pub evidence_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub quality_report_object_sha256: Option<String>,
    pub comparison_report_object_sha256: Option<String>,
    pub design_spec_object_sha256: String,
    pub visual_receipt_object_sha256: Option<String>,
    pub human_review_receipt_object_sha256: Option<String>,
    pub engine_validation_receipt_object_sha256: Option<String>,
    pub distribution_receipt_object_sha256: Option<String>,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub approval_receipt_id: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub parent_transition_id: Option<String>,
    pub parent_transition_sha256: Option<String>,
    pub parent_transition_schema_version: Option<String>,
    pub input_sha256: String,
    pub approved: bool,
    pub approval_summary: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV3PrepareResult {
    pub schema_version: String,
    pub transition: ProductionStageTransitionV3Record,
    pub production_stage_head: ProductionStageHeadV3Record,
    pub compatibility_projection: ProductionStageCompatibilityProjectionV3,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV3GetRequest {
    pub schema_version: String,
    pub transition_id: String,
    pub session_id: String,
    pub project_id: String,
    pub root_candidate_id: String,
    pub head_candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionStageTransitionV3GetResult {
    pub schema_version: String,
    pub transition: ProductionStageTransitionV3Record,
    pub production_stage_head: ProductionStageHeadV3Record,
    pub compatibility_projection: ProductionStageCompatibilityProjectionV3,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

/// ProductionCameraLock@1 is an independent, candidate-bound prerequisite
/// for the V3 `reference-coverage-reviewed -> camera-calibrated` edge.  It
/// records reviewed reference coverage and the complete calibration rig, but
/// deliberately never advances the ProductionStage head.
pub const PRODUCTION_CAMERA_LOCK_SCHEMA_VERSION: &str = "ProductionCameraLock@1";
pub const PRODUCTION_CAMERA_LOCK_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionCameraLockPrepareRequest@1";
pub const PRODUCTION_CAMERA_LOCK_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionCameraLockPrepareResult@1";
pub const PRODUCTION_CAMERA_LOCK_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionCameraLockGetRequest@1";
pub const PRODUCTION_CAMERA_LOCK_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionCameraLockGetResult@1";
pub const PRODUCTION_CAMERA_LOCK_REFERENCE_VIEW_KINDS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
pub const PRODUCTION_CAMERA_LOCK_CAMERA_VIEW_KINDS: &[&str] = &[
    "front",
    "back",
    "left",
    "right",
    "top",
    "bottom",
    "rear-three-quarter",
];
pub const PRODUCTION_CAMERA_LOCK_PRIMARY_VIEW_KIND: &str = "left";
pub const PRODUCTION_CAMERA_LOCK_CALIBRATION_POLICY: &str =
    "fps-weapon-reviewed-six-reference-seven-camera-lock@1";
pub const PRODUCTION_CAMERA_LOCK_REVIEW_STATUS: &str = "user-approved-reference-coverage";
pub const PRODUCTION_CAMERA_LOCK_CALIBRATION_STATUS: &str = "passed";
pub const PRODUCTION_CAMERA_LOCK_STRUCTURAL_STATUS: &str = "PASS_SOURCE_STRUCTURAL";
pub const PRODUCTION_CAMERA_LOCK_VISUAL_STATUS: &str = "QUALITY_TARGET_NOT_MET";
pub const PRODUCTION_CAMERA_LOCK_HUMAN_STATUS: &str = "NOT_RUN";
pub const PRODUCTION_CAMERA_LOCK_ENGINE_STATUS: &str = "NOT_RUN";
pub const PRODUCTION_CAMERA_LOCK_DISTRIBUTION_STATUS: &str = "NOT_RUN";

/// Closed public lineage contracts for mapping one canonical subject-space
/// camera rig onto a GeometryProgram whose exact semantic anchor frame may
/// predate SubjectCoordinateFrame@1.  The registered rig remains read-only
/// and carries no quality, stage, confirmation, version or export authority.
pub const PRODUCTION_WEAPON_SUBJECT_FRAME_REGISTRATION_SCHEMA_VERSION: &str =
    "ProductionWeaponSubjectFrameRegistration@1";
pub const REGISTERED_CAMERA_RIG_CALIBRATION_SCHEMA_VERSION: &str =
    "RegisteredCameraRigCalibration@1";
pub const PRODUCTION_WEAPON_SEMANTIC_LANDMARK_ORDERING_SCHEMA_VERSION: &str =
    "ProductionWeaponSemanticLandmarkOrdering@1";
pub const PRODUCTION_WEAPON_AUTHORED_VIEW_ORIENTATION_SCHEMA_VERSION: &str =
    "ProductionWeaponAuthoredViewOrientation@1";
pub const REGISTERED_CAMERA_RIG_CALIBRATION_V2_SCHEMA_VERSION: &str =
    "RegisteredCameraRigCalibration@2";
pub const PRODUCTION_WEAPON_SUBJECT_FRAME_REGISTRATION_POLICY: &str =
    "exact-semantic-anchor-axis-registration@1";
pub const REGISTERED_CAMERA_RIG_QUALITY_STATUS: &str = "NOT_EVALUATED";
pub const PRODUCTION_WEAPON_SEMANTIC_ORDERING_POLICY: &str =
    "exact-subject-axis-source-order-no-2d-landmarks@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponSubjectFrameRegistrationRecord {
    pub schema_version: String,
    pub registration_id: String,
    pub geometry_program_sha256: String,
    pub subject_coordinate_frame_sha256: String,
    pub derivation_policy: String,
    pub geometry_semantic_axes: Value,
    pub subject_semantic_axes: Value,
    pub anchor_evidence: Value,
    pub transform: Value,
    pub read_only: bool,
    pub geometry_program_modified: bool,
    pub depth_modified: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RegisteredCameraRigCalibrationRecord {
    pub schema_version: String,
    pub registered_rig_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub operator_catalog_sha256: String,
    pub subject_camera_rig: Value,
    pub subject_camera_rig_object_sha256: String,
    pub subject_camera_rig_canonical_sha256: String,
    pub subject_frame_registration: ProductionWeaponSubjectFrameRegistrationRecord,
    pub subject_frame_registration_canonical_sha256: String,
    pub renderer_views: Vec<Value>,
    pub read_only: bool,
    pub runtime_write: bool,
    pub depth_status: String,
    pub quality_status: String,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponSemanticLandmarkOrderingRecord {
    pub schema_version: String,
    pub ordering_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_sha256: String,
    pub subject_camera_rig_object_sha256: String,
    pub subject_camera_rig_canonical_sha256: String,
    pub registered_camera_rig_canonical_sha256: String,
    pub ordering_policy: String,
    pub identity_view_kinds: Vec<String>,
    pub camera_view_kinds: Vec<String>,
    pub primary_view_kind: String,
    pub subject_longitudinal_order: Vec<String>,
    pub anchors: Vec<Value>,
    pub target_landmark_arrays_present: bool,
    pub target_landmark_metrics_status: String,
    pub ordering_status: String,
    pub authored_orientation_status: String,
    pub read_only: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponAuthoredViewOrientationRecord {
    pub schema_version: String,
    pub orientation_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub view_kind: String,
    pub source_view: String,
    pub reference_view_spec_canonical_sha256: String,
    pub source_crop: Value,
    pub reference_to_subject_view: Value,
    pub post_render_transform: String,
    pub target_landmark_status: String,
    pub orientation_provenance: Value,
    pub status: String,
    pub promotable: bool,
    pub read_only: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RegisteredCameraRigCalibrationV2Record {
    pub schema_version: String,
    pub registered_rig_v2_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub registered_rig_v1: RegisteredCameraRigCalibrationRecord,
    pub registered_rig_v1_canonical_sha256: String,
    pub semantic_landmark_ordering: ProductionWeaponSemanticLandmarkOrderingRecord,
    pub semantic_landmark_ordering_object_sha256: String,
    pub semantic_landmark_ordering_canonical_sha256: String,
    pub rear_three_quarter_authored_orientation: ProductionWeaponAuthoredViewOrientationRecord,
    pub rear_three_quarter_authored_orientation_object_sha256: String,
    pub rear_three_quarter_authored_orientation_canonical_sha256: String,
    pub renderer_views: Vec<Value>,
    pub read_only: bool,
    pub runtime_write: bool,
    pub depth_status: String,
    pub quality_status: String,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionCameraLockRecord {
    pub schema_version: String,
    pub camera_lock_id: String,
    pub session_id: String,
    pub project_id: String,
    pub source_transition_id: String,
    pub source_transition_sha256: String,
    pub source_head_canonical_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_object_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub camera_rig_object_sha256: String,
    pub camera_rig_canonical_sha256: String,
    pub required_reference_view_kinds: Vec<String>,
    pub required_camera_view_kinds: Vec<String>,
    pub primary_view_kind: String,
    pub calibration_policy: String,
    pub review_status: String,
    pub calibration_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub approval_receipt_id: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub approval_summary_sha256: String,
    pub input_sha256: String,
    pub request_key_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionCameraLockPrepareRequest {
    pub schema_version: String,
    pub camera_lock_id: String,
    pub session_id: String,
    pub project_id: String,
    pub source_transition_id: String,
    pub source_transition_sha256: String,
    pub source_head_canonical_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub required_reference_view_kinds: Vec<String>,
    pub required_camera_view_kinds: Vec<String>,
    pub primary_view_kind: String,
    pub calibration_policy: String,
    pub input_sha256: String,
    pub approved: bool,
    pub camera_rig: Value,
    pub approval_receipt_id: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub approval_summary: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionCameraLockPrepareResult {
    pub schema_version: String,
    pub camera_lock: ProductionCameraLockRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub restart_hash_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionCameraLockGetRequest {
    pub schema_version: String,
    pub camera_lock_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionCameraLockGetResult {
    pub schema_version: String,
    pub camera_lock: ProductionCameraLockRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub restart_hash_verified: bool,
}

/// A success-only additive child of ProductionCameraLock@1.  This compact
/// lineage binds the exact source objects needed for registered semantic
/// camera materialization without copying or upgrading the legacy CameraLock
/// record.  A diagnostic or blocked authored orientation is intentionally not
/// representable as a successful lineage.
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_SCHEMA_VERSION: &str =
    "ProductionCameraLockRegistrationLineage@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionCameraLockRegistrationLineagePrepareRequest@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionCameraLockRegistrationLineagePrepareResult@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionCameraLockRegistrationLineageGetRequest@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionCameraLockRegistrationLineageGetResult@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionCameraLockRegistrationLineagePreflightGetRequest@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionCameraLockRegistrationLineagePreflightGetResult@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionCameraLockRegistrationLineagePreflightProjectionGetRequest@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionCameraLockRegistrationLineagePreflightProjectionGetResult@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREPARE_OPERATION: &str =
    "forgecad.production.camera-lock-registration-lineage-prepare@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_GET_OPERATION: &str =
    "forgecad.production.camera-lock-registration-lineage-get@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_GET_OPERATION: &str =
    "forgecad.production.camera-lock-registration-lineage-preflight-get@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_GET_OPERATION: &str =
    "forgecad.production.camera-lock-registration-lineage-preflight-projection-get@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_POLICY: &str =
    "camera-lock-promotable-authored-orientation-lineage@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_POLICY: &str =
    "camera-lock-user-authority-preflight@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_PREFLIGHT_PROJECTION_POLICY: &str =
    "runtime-derived-semantic-camera-preflight-projection@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_WRITER_POLICY: &str =
    "forgecad-runtime-only-state-writer@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_GEOMETRY_PROGRAM_SCHEMA_VERSION: &str =
    "GeometryProgram@2";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_SEMANTIC_ORDERING_SCHEMA_VERSION: &str =
    "ProductionWeaponSemanticLandmarkOrdering@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_AUTHORED_ORIENTATION_SCHEMA_VERSION: &str =
    "ProductionWeaponAuthoredViewOrientation@1";
pub const PRODUCTION_CAMERA_LOCK_REGISTRATION_LINEAGE_REGISTERED_RIG_V2_SCHEMA_VERSION: &str =
    "RegisteredCameraRigCalibration@2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineageRecord {
    pub schema_version: String,
    pub registration_lineage_id: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub camera_lock_receipt_object_sha256: String,
    pub session_id: String,
    pub project_id: String,
    pub source_transition_id: String,
    pub source_transition_sha256: String,
    pub source_head_canonical_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_object_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub subject_camera_rig_object_sha256: String,
    pub subject_camera_rig_canonical_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub semantic_landmark_ordering_object_sha256: String,
    pub semantic_landmark_ordering_canonical_sha256: String,
    pub authored_orientation_object_sha256: String,
    pub authored_orientation_canonical_sha256: String,
    pub authored_orientation_approval_receipt_object_sha256: Option<String>,
    pub registered_rig_v2_object_sha256: String,
    pub registered_rig_v2_canonical_sha256: String,
    pub lineage_policy: String,
    pub promotable: bool,
    pub input_sha256: String,
    pub request_key_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Alias without the Rust implementation suffix for callers that use the
/// contract title as the record type name.
pub type ProductionCameraLockRegistrationLineage = ProductionCameraLockRegistrationLineageRecord;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineagePrepareRequest {
    pub schema_version: String,
    pub operation: String,
    pub registration_lineage_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub semantic_landmark_ordering_id: String,
    pub authored_orientation_id: String,
    pub registered_rig_v2_id: String,
    pub rear_three_quarter_rotation_degrees: i64,
    pub rear_three_quarter_subject_screen_order: String,
    pub rear_three_quarter_camera_orbit_degrees: i64,
    pub approval_receipt_id: String,
    pub approval_session_id: String,
    pub approval_expires_at: String,
    pub approval_summary: String,
    pub approved: bool,
    pub idempotency_key: String,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineagePrepareResult {
    pub schema_version: String,
    pub operation: String,
    pub registration_lineage_id: String,
    pub registration_lineage: ProductionCameraLockRegistrationLineageRecord,
    pub session_id: String,
    pub project_id: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub camera_lock_receipt_object_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub semantic_landmark_ordering_id: String,
    pub semantic_landmark_ordering_object_sha256: String,
    pub semantic_landmark_ordering_canonical_sha256: String,
    pub authored_orientation_id: String,
    pub authored_orientation_object_sha256: String,
    pub authored_orientation_canonical_sha256: String,
    pub authored_orientation_approval_receipt_object_sha256: Option<String>,
    pub authored_orientation_status: String,
    pub registered_rig_v2_id: String,
    pub registered_rig_v2_object_sha256: String,
    pub registered_rig_v2_canonical_sha256: String,
    pub lineage_policy: String,
    pub promotable: bool,
    pub request_sha256: String,
    pub request_input_sha256: String,
    pub input_sha256: String,
    pub request_key_sha256: String,
    pub receipt_object_sha256: String,
    pub idempotency_key: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub depth_status: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineageGetRequest {
    pub schema_version: String,
    pub operation: String,
    pub registration_lineage_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub max_response_bytes: u64,
    pub writer_policy: String,
    pub input_sha256: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineageGetResult {
    pub schema_version: String,
    pub operation: String,
    pub registration_lineage_id: String,
    pub registration_lineage: ProductionCameraLockRegistrationLineageRecord,
    pub session_id: String,
    pub project_id: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub camera_lock_receipt_object_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub geometry_program_object_sha256: String,
    pub geometry_program_sha256: String,
    pub semantic_landmark_ordering_id: String,
    pub semantic_landmark_ordering_object_sha256: String,
    pub semantic_landmark_ordering_canonical_sha256: String,
    pub authored_orientation_id: String,
    pub authored_orientation_object_sha256: String,
    pub authored_orientation_canonical_sha256: String,
    pub authored_orientation_approval_receipt_object_sha256: Option<String>,
    pub authored_orientation_status: String,
    pub registered_rig_v2_id: String,
    pub registered_rig_v2_object_sha256: String,
    pub registered_rig_v2_canonical_sha256: String,
    pub lineage_policy: String,
    pub promotable: bool,
    pub request_sha256: String,
    pub request_input_sha256: String,
    pub input_sha256: String,
    pub request_key_sha256: String,
    pub receipt_object_sha256: String,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub depth_status: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

/// Read-only authority preflight for the success-only registration child.
/// `diagnostic_inferred_rotation_degrees` is deliberately caller-labelled
/// diagnostic input; it is never an approval receipt and cannot make the
/// lineage promotable.  A durable child is the only source this preflight
/// treats as a prior user-approved orientation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineagePreflightGetRequest {
    pub schema_version: String,
    pub operation: String,
    pub preflight_id: String,
    pub registration_lineage_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub diagnostic_inferred_rotation_degrees: i64,
    pub max_response_bytes: u64,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineagePreflightGetResult {
    pub schema_version: String,
    pub operation: String,
    pub preflight_id: String,
    pub registration_lineage_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub parent_camera_lock_status: String,
    pub parent_camera_lock_receipt_object_sha256: Option<String>,
    pub durable_lineage_status: String,
    pub existing_promotable_lineage_present: bool,
    pub user_approved_orientation_present: bool,
    pub user_approved_orientation_source: String,
    pub diagnostic_inferred_orientation_present: bool,
    pub diagnostic_inferred_rotation_degrees: i64,
    pub diagnostic_orientation_source: String,
    pub orientation_authority_status: String,
    pub ready_for_promotable_lineage: bool,
    pub blocking_reasons: Vec<String>,
    pub policy: String,
    pub writer_policy: String,
    pub runtime_write: bool,
    pub persistent_user_data_touched: bool,
    pub worker_started: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub restart_hash_verified: bool,
    pub readiness_sha256: String,
}

/// Successor read-only projection used to review the exact semantic camera
/// before an approval receipt exists. The caller supplies no camera orbit,
/// camera matrix, semantic anchors or geometry; Runtime derives all of them
/// from the immutable CameraLock and candidate-owned source truth.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineagePreflightProjectionGetRequest {
    pub schema_version: String,
    pub operation: String,
    pub preflight_id: String,
    pub registration_lineage_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub proposed_board_rotation_degrees: i64,
    pub proposed_subject_screen_order: String,
    pub max_response_bytes: u64,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineagePreflightProjectionProof {
    pub policy: String,
    pub camera_hash: String,
    pub expected_subject_screen_order: String,
    pub projected_subject_screen_order: String,
    pub stock_minus_muzzle_screen_x_milli: i64,
    pub world_y_screen_up_dot_milli: i64,
    pub screen_up: String,
    pub passed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCameraLockRegistrationLineagePreflightProjectionGetResult {
    pub schema_version: String,
    pub operation: String,
    pub preflight_id: String,
    pub registration_lineage_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub parent_camera_lock_status: String,
    pub parent_camera_lock_receipt_object_sha256: Option<String>,
    pub proposed_board_rotation_degrees: i64,
    pub proposed_subject_screen_order: String,
    pub derived_camera_orbit_degrees: Option<i64>,
    pub derived_camera_hash: Option<String>,
    pub derived_camera_canonical_sha256: Option<String>,
    pub upright_proof: Option<ProductionCameraLockRegistrationLineagePreflightProjectionProof>,
    pub projection_status: String,
    pub projection_input_sha256: Option<String>,
    pub projection_ready_for_user_review: bool,
    pub existing_lineage_status: String,
    pub existing_promotable_lineage_present: bool,
    pub existing_lineage_matches_proposal: bool,
    pub orientation_authority_status: String,
    pub ready_for_promotable_lineage: bool,
    pub blocking_reasons: Vec<String>,
    pub policy: String,
    pub writer_policy: String,
    pub runtime_write: bool,
    pub persistent_user_data_touched: bool,
    pub worker_started: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub readiness_sha256: String,
}

/// A source binding for one independently persisted per-view observation.  It
/// is deliberately narrower than a quality gate: the producer may report an
/// observed, inferred or unknown observation, but the quality status remains
/// NOT_PROVEN until a later FORM gate consumes the evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidenceObservation {
    pub evidence_kind: String,
    pub observation_status: String,
    pub quality_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidencePartId {
    pub observation: ProductionWeaponFormEvidenceObservation,
    pub expected_part_ids: Vec<String>,
    pub observed_part_ids: Vec<String>,
    pub missing_part_ids: Vec<String>,
    pub unexpected_part_ids: Vec<String>,
    pub coverage_milli: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidenceNegativeSpace {
    pub observation: ProductionWeaponFormEvidenceObservation,
    pub expected_count: u64,
    pub observed_count: u64,
    pub missing_count: u64,
    pub sealed_count: u64,
    pub coverage_milli: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidenceLineFlow {
    pub observation: ProductionWeaponFormEvidenceObservation,
    pub expected_count: u64,
    pub observed_count: u64,
    pub coverage_milli: u64,
    pub continuity_milli: u64,
    pub deviation_milli: u64,
}

/// Input binding for one of the six reviewed views.  The Runtime derives the
/// three observations from this already-existing RenderSet and never accepts
/// geometry, image bytes, paths or an externally supplied quality PASS.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidenceViewInput {
    pub view_kind: String,
    pub view_id: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub camera_canonical_sha256: String,
    pub render_set_object_sha256: String,
    pub render_set_canonical_sha256: String,
    pub render_set_view_id: String,
}

/// One independently persisted child projection.  It repeats the candidate
/// and source hashes intentionally so a Store child row is auditable without
/// trusting an array position or a parent-only payload projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidenceViewRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub view_kind: String,
    pub view_id: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub camera_canonical_sha256: String,
    pub render_set_object_sha256: String,
    pub render_set_canonical_sha256: String,
    pub render_set_view_id: String,
    pub part_id_evidence: ProductionWeaponFormEvidencePartId,
    pub negative_space_evidence: ProductionWeaponFormEvidenceNegativeSpace,
    pub line_flow_evidence: ProductionWeaponFormEvidenceLineFlow,
    pub view_observation_status: String,
    pub quality_status: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// Immutable parent for exactly six independently persisted per-view
/// Part-ID/negative-space/line-flow evidence children.  This is evidence only:
/// it cannot create a candidate, advance a stage, confirm, version or export.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidenceRecord {
    pub schema_version: String,
    pub form_evidence_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_object_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub camera_rig_object_sha256: String,
    pub camera_rig_canonical_sha256: String,
    pub camera_lock_receipt_object_sha256: String,
    pub camera_lock_source_transition_id: String,
    pub camera_lock_source_transition_sha256: String,
    pub camera_lock_source_head_canonical_sha256: String,
    pub view_kinds: Vec<String>,
    pub views: Vec<ProductionWeaponFormEvidenceViewRecord>,
    pub evidence_policy: String,
    pub evidence_policy_sha256: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidencePrepareRequest {
    pub schema_version: String,
    pub form_evidence_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_object_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub camera_rig_object_sha256: String,
    pub camera_rig_canonical_sha256: String,
    pub camera_lock_receipt_object_sha256: String,
    pub camera_lock_source_transition_id: String,
    pub camera_lock_source_transition_sha256: String,
    pub camera_lock_source_head_canonical_sha256: String,
    pub view_kinds: Vec<String>,
    pub views: Vec<ProductionWeaponFormEvidenceViewInput>,
    pub evidence_policy: String,
    pub evidence_policy_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidencePrepareResult {
    pub schema_version: String,
    pub form_evidence: ProductionWeaponFormEvidenceRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidenceGetRequest {
    pub schema_version: String,
    pub form_evidence_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormEvidenceGetResult {
    pub schema_version: String,
    pub form_evidence: ProductionWeaponFormEvidenceRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub restart_hash_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormArtEvidencePartIdAggregate {
    pub status: String,
    pub expected_count: u64,
    pub observed_count: u64,
    pub missing_count: u64,
    pub unexpected_count: u64,
    pub coverage_milli: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormArtEvidenceNegativeSpaceRow {
    pub structure_id: String,
    pub expected_region_canonical_sha256: String,
    pub iou_milli: u64,
    pub boundary_f1_milli: u64,
    pub area_ratio_milli: u64,
    pub centroid_error_milli: u64,
    pub sealed: bool,
    pub missing: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormArtEvidenceLineFlowRow {
    pub line_flow_id: String,
    pub expected_line_canonical_sha256: String,
    pub coverage_milli: u64,
    pub continuity_milli: u64,
    pub symmetric_chamfer_milli: u64,
    pub max_deviation_milli: u64,
    pub direction_order_milli: u64,
    pub duplicate_crossing_count: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormArtEvidenceViewRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub view_kind: String,
    pub view_id: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub camera_canonical_sha256: String,
    pub form_evidence_view_receipt_object_sha256: String,
    pub form_evidence_view_receipt_canonical_sha256: String,
    pub target_object_sha256: String,
    pub target_canonical_sha256: String,
    pub visual_structure_canonical_sha256: String,
    pub visual_structure_review_status: String,
    pub silhouette_pass_object_sha256: String,
    pub part_id_pass_object_sha256: String,
    pub depth_pass_object_sha256: String,
    pub normal_pass_object_sha256: String,
    pub part_id_status: String,
    pub part_id_expected_count: u64,
    pub part_id_observed_count: u64,
    pub part_id_missing_count: u64,
    pub part_id_unexpected_count: u64,
    pub part_id_coverage_milli: u64,
    pub negative_space_status: String,
    pub negative_space_rows: Vec<ProductionWeaponFormArtEvidenceNegativeSpaceRow>,
    pub line_flow_status: String,
    pub line_flow_rows: Vec<ProductionWeaponFormArtEvidenceLineFlowRow>,
    pub view_observation_status: String,
    pub quality_status: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormArtEvidenceRecord {
    pub schema_version: String,
    pub art_evidence_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_object_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub camera_rig_object_sha256: String,
    pub camera_rig_canonical_sha256: String,
    pub camera_lock_receipt_object_sha256: String,
    pub camera_lock_source_transition_id: String,
    pub camera_lock_source_transition_sha256: String,
    pub camera_lock_source_head_canonical_sha256: String,
    pub form_evidence_object_sha256: String,
    pub form_evidence_canonical_sha256: String,
    pub view_kinds: Vec<String>,
    pub views: Vec<ProductionWeaponFormArtEvidenceViewRecord>,
    pub part_id_aggregate: ProductionWeaponFormArtEvidencePartIdAggregate,
    pub art_evidence_policy: String,
    pub art_evidence_policy_sha256: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormArtEvidencePrepareRequest {
    pub schema_version: String,
    pub art_evidence_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub form_evidence_object_sha256: String,
    pub form_evidence_canonical_sha256: String,
    pub art_evidence_policy: String,
    pub art_evidence_policy_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormArtEvidencePrepareResult {
    pub schema_version: String,
    pub art_evidence: ProductionWeaponFormArtEvidenceRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormArtEvidenceGetRequest {
    pub schema_version: String,
    pub art_evidence_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormArtEvidenceGetResult {
    pub schema_version: String,
    pub art_evidence: ProductionWeaponFormArtEvidenceRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub restart_hash_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponAssemblyDecisionRegistryGroup {
    pub group_id: String,
    pub intent_kind: String,
    pub part_ids: Vec<String>,
    pub source_node_ids: Vec<String>,
    pub parameter_ids: Vec<String>,
    pub allowed_operator_ids: Vec<String>,
    pub coupling_mode: String,
    pub invariants: Vec<String>,
    pub affected_view_kinds: Vec<String>,
    pub priority: u64,
}

/// Immutable, closed assembly vocabulary consumed by the read-only art
/// decision projection.  It is not a GeometryProgram and does not itself
/// authorize a Worker or a Runtime write.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponAssemblyDecisionRegistry {
    pub schema_version: String,
    pub registry_id: String,
    pub profile_id: String,
    pub operator_catalog_sha256: String,
    pub registry_policy: String,
    pub groups: Vec<ProductionWeaponAssemblyDecisionRegistryGroup>,
    pub canonical_sha256: String,
}

/// A single Runtime-owned aggregate parameter sink.  The target is expressed
/// only through a product-owned mutator ID plus verified Part/node/operator
/// bindings; callers never provide JSON pointers, parameter keys, components,
/// expressions, or scripts. Unavailable parameters are kept in the registry's
/// closed `unavailable_parameter_ids` list rather than faked as sink rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponAssemblyParameterSink {
    pub parameter_id: String,
    pub group_id: String,
    pub mutator_id: String,
    pub current: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub unit: String,
    pub application_status: String,
    pub blocker_codes: Vec<String>,
    pub target_part_ids: Vec<String>,
    pub source_node_ids: Vec<String>,
    pub operator_ids: Vec<String>,
    pub evidence_requirements: Vec<String>,
}

/// Read-only diagnostic projection of the typed assembly parameter sink. The
/// The current slice emits only real AVAILABLE receiver/muzzle/open-stock
/// rows. Its status is `PARTIAL_TYPED_SINKS` when fewer than eight are
/// available; the closed unavailable list carries trigger/rail plus any
/// missing supported IDs and prevents a false twelve-parameter claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponAssemblyParameterSinkRegistry {
    pub schema_version: String,
    pub sink_registry_id: String,
    pub profile_id: String,
    pub sink_policy: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_program_canonical_sha256: String,
    pub operator_catalog_sha256: String,
    pub assembly_registry_id: String,
    pub assembly_registry_canonical_sha256: String,
    pub supported_group_ids: Vec<String>,
    pub sinks: Vec<ProductionWeaponAssemblyParameterSink>,
    pub unavailable_parameter_ids: Vec<String>,
    pub status: String,
    pub read_only: bool,
    pub runtime_write_performed: bool,
    pub worker_invoked: bool,
    pub candidate_generated: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponAssemblyParameterSinkGetRequest {
    pub schema_version: String,
    pub sink_registry_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_program_canonical_sha256: String,
    pub operator_catalog_sha256: String,
    pub assembly_registry_id: String,
    pub assembly_registry_canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponAssemblyParameterSinkGetResult {
    pub schema_version: String,
    pub registry: ProductionWeaponAssemblyParameterSinkRegistry,
    pub registry_canonical_sha256: String,
    pub recomputed: bool,
    pub restart_hash_verified: bool,
    pub read_only: bool,
    pub structural_status: String,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write_performed: bool,
    pub worker_invoked: bool,
    pub candidate_generated: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponArtDecisionProposalViewBinding {
    pub view_kind: String,
    pub view_id: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub camera_canonical_sha256: String,
    pub render_set_object_sha256: String,
    pub render_set_canonical_sha256: String,
    pub form_evidence_view_receipt_object_sha256: String,
    pub form_evidence_view_receipt_canonical_sha256: String,
    pub form_art_evidence_view_receipt_object_sha256: String,
    pub form_art_evidence_view_receipt_canonical_sha256: String,
    pub target_sha256: String,
    pub visual_structure_canonical_sha256: String,
    pub part_id_status: String,
    pub negative_space_status: String,
    pub line_flow_status: String,
    pub view_observation_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponArtDecisionProposalAssemblyGroupDecision {
    pub group_id: String,
    pub status: String,
    pub part_ids: Vec<String>,
    pub source_node_ids: Vec<String>,
    pub parameter_ids: Vec<String>,
    pub allowed_operator_ids: Vec<String>,
    pub coupling_mode: String,
    pub invariants: Vec<String>,
    pub affected_view_kinds: Vec<String>,
    pub blocker_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponArtDecisionProposalGateResult {
    pub gate_id: String,
    pub status: String,
    pub evidence_sha256: Option<String>,
    pub blocker_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponArtDecisionProposalBlocker {
    pub blocker_code: String,
    pub scope: String,
    pub group_id: Option<String>,
    pub view_kind: Option<String>,
    pub evidence_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponArtDecisionProposalGetRequest {
    pub schema_version: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_program_canonical_sha256: String,
    pub operator_catalog_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub form_evidence_id: String,
    pub form_evidence_object_sha256: String,
    pub form_evidence_canonical_sha256: String,
    pub form_art_evidence_id: String,
    pub form_art_evidence_object_sha256: String,
    pub form_art_evidence_canonical_sha256: String,
    pub first_person_profile_id: Option<String>,
    pub first_person_profile_sha256: Option<String>,
}

/// Read-only proposal result.  Blockers are first-class output so the current
/// real six-view fixture can be projected without pretending that unknown
/// negative-space/line-flow or absent first-person evidence has passed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponArtDecisionProposalGetResult {
    pub schema_version: String,
    pub proposal_projection_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub geometry_program_sha256: String,
    pub geometry_program_canonical_sha256: String,
    pub operator_catalog_sha256: String,
    pub assembly_registry_id: String,
    pub assembly_registry_canonical_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub form_evidence_id: String,
    pub form_evidence_object_sha256: String,
    pub form_evidence_canonical_sha256: String,
    pub form_art_evidence_id: String,
    pub form_art_evidence_object_sha256: String,
    pub form_art_evidence_canonical_sha256: String,
    pub first_person_profile_id: Option<String>,
    pub first_person_profile_sha256: Option<String>,
    pub view_bindings: Vec<ProductionWeaponArtDecisionProposalViewBinding>,
    pub assembly_group_decisions: Vec<ProductionWeaponArtDecisionProposalAssemblyGroupDecision>,
    pub objective_policy: String,
    pub gate_results: Vec<ProductionWeaponArtDecisionProposalGateResult>,
    pub blockers: Vec<ProductionWeaponArtDecisionProposalBlocker>,
    pub proposal_status: String,
    pub read_only: bool,
    pub runtime_write_performed: bool,
    pub worker_invoked: bool,
    pub candidate_generated: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub canonical_sha256: String,
}

/// The CrossViewEvidenceBundle owns every RenderSet, ComparisonReport and
/// QualityReport hash plus the per-view metrics/no-regression result.  Form
/// quality binds that immutable parent object instead of accepting a caller
/// supplied copy of those values.  These three evidence records are the only
/// form-stage-specific artistic projections; their source must be a Runtime-
/// verified bundle/design-spec object or remain NOT_PROVEN.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityEvidenceBinding {
    pub source_kind: String,
    pub source_object_sha256: Option<String>,
    pub evidence_object_sha256: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityPartIdEvidence {
    pub source: ProductionWeaponFormQualityEvidenceBinding,
    pub expected_part_ids: Vec<String>,
    pub observed_part_ids: Vec<String>,
    pub missing_part_ids: Vec<String>,
    pub unexpected_part_ids: Vec<String>,
    pub coverage_milli: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityNegativeSpaceEvidence {
    pub source: ProductionWeaponFormQualityEvidenceBinding,
    pub expected_count: u64,
    pub observed_count: u64,
    pub missing_count: u64,
    pub sealed_count: u64,
    pub coverage_milli: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityLineFlowEvidence {
    pub source: ProductionWeaponFormQualityEvidenceBinding,
    pub expected_count: u64,
    pub observed_count: u64,
    pub coverage_milli: u64,
    pub continuity_milli: u64,
    pub deviation_milli: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityNoRegression {
    pub status: String,
    pub metrics_not_regressed: bool,
    pub part_id_not_regressed: bool,
    pub negative_space_not_regressed: bool,
    pub line_flow_not_regressed: bool,
}

/// One SQL-child-like projection of a CrossViewEvidenceBundle view.  The
/// `view_id` must resolve to the parent bundle; RenderSet/ComparisonReport/
/// QualityReport hashes and metrics are intentionally not copied here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityViewRecord {
    pub view_kind: String,
    pub view_id: String,
    pub part_id_evidence: ProductionWeaponFormQualityPartIdEvidence,
    pub negative_space_evidence: ProductionWeaponFormQualityNegativeSpaceEvidence,
    pub line_flow_evidence: ProductionWeaponFormQualityLineFlowEvidence,
    pub no_regression: ProductionWeaponFormQualityNoRegression,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityHardGate {
    pub stage_head_binding: bool,
    pub camera_lock_binding: bool,
    pub same_candidate_artifact: bool,
    pub reviewed_reference_views: bool,
    pub fixed_camera_views: bool,
    pub cross_view_evidence_binding: bool,
    pub form_view_evaluations: bool,
    pub part_id_evidence: bool,
    pub negative_space_evidence: bool,
    pub line_flow_evidence: bool,
    pub threshold_policy_binding: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityFormGate {
    pub layer_status: String,
    pub all_view_thresholds: bool,
    pub all_view_no_regression: bool,
    pub previous_form_quality_binding: bool,
}

/// Immutable structural form evidence. One record is produced for exactly
/// one form edge (blockout, primary or secondary); later edges bind the
/// previous record instead of compensating for a failed earlier layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityRecord {
    pub schema_version: String,
    pub form_quality_id: String,
    pub session_id: String,
    pub project_id: String,
    pub form_stage: String,
    pub source_stage: String,
    pub target_stage: String,
    pub camera_calibrated_head_transition_id: String,
    pub camera_calibrated_head_transition_sha256: String,
    pub camera_calibrated_head_canonical_sha256: String,
    pub camera_calibrated_head_candidate_id: String,
    pub camera_calibrated_head_candidate_state_sha256: String,
    pub camera_calibrated_head_artifact_id: String,
    pub camera_calibrated_head_artifact_sha256: String,
    pub camera_calibrated_head_stage: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_object_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub camera_rig_object_sha256: String,
    pub camera_rig_canonical_sha256: String,
    pub camera_lock_receipt_object_sha256: String,
    pub camera_lock_source_transition_id: String,
    pub camera_lock_source_transition_sha256: String,
    pub camera_lock_source_head_canonical_sha256: String,
    pub reviewed_reference_view_kinds: Vec<String>,
    pub fixed_camera_view_kinds: Vec<String>,
    pub cross_view_evidence_object_sha256: String,
    pub cross_view_evidence_canonical_sha256: String,
    pub cross_view_evidence_view_kinds: Vec<String>,
    pub form_evidence_object_sha256: String,
    pub form_evidence_canonical_sha256: String,
    pub form_view_evaluations: Vec<ProductionWeaponFormQualityViewRecord>,
    pub previous_form_quality_id: Option<String>,
    pub previous_form_quality_report_object_sha256: Option<String>,
    pub previous_form_quality_canonical_sha256: Option<String>,
    pub form_quality_policy: String,
    pub form_quality_policy_sha256: String,
    pub threshold_policy: String,
    pub threshold_policy_sha256: String,
    pub layer_status: String,
    pub hard_gate: ProductionWeaponFormQualityHardGate,
    pub hard_gate_passed: bool,
    pub form_gate: ProductionWeaponFormQualityFormGate,
    pub form_gate_passed: bool,
    pub validator_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityPrepareRequest {
    pub schema_version: String,
    pub form_quality_id: String,
    pub session_id: String,
    pub project_id: String,
    pub form_stage: String,
    pub source_stage: String,
    pub target_stage: String,
    pub camera_calibrated_head_transition_id: String,
    pub camera_calibrated_head_transition_sha256: String,
    pub camera_calibrated_head_canonical_sha256: String,
    pub camera_calibrated_head_candidate_id: String,
    pub camera_calibrated_head_candidate_state_sha256: String,
    pub camera_calibrated_head_artifact_id: String,
    pub camera_calibrated_head_artifact_sha256: String,
    pub camera_calibrated_head_stage: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_object_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub camera_rig_object_sha256: String,
    pub camera_rig_canonical_sha256: String,
    pub camera_lock_receipt_object_sha256: String,
    pub camera_lock_source_transition_id: String,
    pub camera_lock_source_transition_sha256: String,
    pub camera_lock_source_head_canonical_sha256: String,
    pub reviewed_reference_view_kinds: Vec<String>,
    pub fixed_camera_view_kinds: Vec<String>,
    pub cross_view_evidence_object_sha256: String,
    pub cross_view_evidence_canonical_sha256: String,
    pub cross_view_evidence_view_kinds: Vec<String>,
    pub form_evidence_object_sha256: String,
    pub form_evidence_canonical_sha256: String,
    pub form_view_evaluations: Vec<ProductionWeaponFormQualityViewRecord>,
    pub previous_form_quality_id: Option<String>,
    pub previous_form_quality_report_object_sha256: Option<String>,
    pub previous_form_quality_canonical_sha256: Option<String>,
    pub form_quality_policy: String,
    pub form_quality_policy_sha256: String,
    pub threshold_policy: String,
    pub threshold_policy_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityPrepareResult {
    pub schema_version: String,
    pub form_quality: ProductionWeaponFormQualityRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityGetRequest {
    pub schema_version: String,
    pub form_quality_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub form_stage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityGetResult {
    pub schema_version: String,
    pub form_quality: ProductionWeaponFormQualityRecord,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2ViewDecision {
    pub view_kind: String,
    pub legacy_form_quality_view_id: String,
    pub legacy_form_quality_view_canonical_sha256: String,
    pub form_art_view_id: String,
    pub form_art_view_canonical_sha256: String,
    pub form_art_view_receipt_object_sha256: String,
    pub target_object_sha256: String,
    pub target_canonical_sha256: String,
    pub silhouette_pass_object_sha256: String,
    pub part_id_pass_object_sha256: String,
    pub depth_pass_object_sha256: String,
    pub normal_pass_object_sha256: String,
    pub cross_view_thresholds_passed: bool,
    pub no_regression_passed: bool,
    pub part_id_passed: bool,
    pub negative_space_passed: bool,
    pub line_flow_passed: bool,
    pub view_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2Aggregate {
    pub view_count: u64,
    pub all_cross_view_thresholds_passed: bool,
    pub all_no_regression_passed: bool,
    pub all_part_id_passed: bool,
    pub all_negative_space_passed: bool,
    pub all_line_flow_passed: bool,
    pub all_view_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2Record {
    pub schema_version: String,
    pub form_quality_id: String,
    pub session_id: String,
    pub project_id: String,
    pub form_stage: String,
    pub source_stage: String,
    pub target_stage: String,
    pub current_source_head_transition_id: String,
    pub current_source_head_transition_sha256: String,
    pub current_source_head_canonical_sha256: String,
    pub current_source_head_stage: String,
    pub current_source_head_candidate_id: String,
    pub current_source_head_candidate_state_sha256: String,
    pub current_source_head_artifact_id: String,
    pub current_source_head_artifact_sha256: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub reference_canvas_object_sha256: String,
    pub reference_canvas_canonical_sha256: String,
    pub design_spec_object_sha256: String,
    pub design_spec_canonical_sha256: String,
    pub camera_hash: String,
    /// `legacy-source` keeps historical FormQuality/FormArt joins readable;
    /// `fresh-baseline-proposal` requires every source/proposal scope below.
    pub evidence_source_kind: String,
    pub source_candidate_id: Option<String>,
    pub source_candidate_state_sha256: Option<String>,
    pub source_artifact_id: Option<String>,
    pub source_artifact_sha256: Option<String>,
    pub source_fresh_baseline_id: Option<String>,
    pub source_fresh_baseline_canonical_sha256: Option<String>,
    pub source_fresh_baseline_receipt_object_sha256: Option<String>,
    pub source_registration_lineage_id: Option<String>,
    pub source_registration_lineage_canonical_sha256: Option<String>,
    pub source_registration_lineage_receipt_object_sha256: Option<String>,
    pub source_registered_rig_v2_id: Option<String>,
    pub source_registered_rig_v2_object_sha256: Option<String>,
    pub source_registered_rig_v2_canonical_sha256: Option<String>,
    pub source_runtime_build_cohort_sha256: Option<String>,
    pub proposal_candidate_id: Option<String>,
    pub proposal_candidate_state_sha256: Option<String>,
    pub proposal_artifact_id: Option<String>,
    pub proposal_artifact_sha256: Option<String>,
    pub proposal_artifact_readback_sha256: Option<String>,
    pub proposal_worker_build_cohort_sha256: Option<String>,
    pub cross_view_evidence_bundle_sha256: Option<String>,
    pub proposal_form_art_evidence_id: Option<String>,
    pub proposal_form_art_evidence_object_sha256: Option<String>,
    pub proposal_form_art_evidence_canonical_sha256: Option<String>,
    pub proposal_part_id_evidence_sha256: Option<String>,
    pub proposal_negative_space_evidence_sha256: Option<String>,
    pub proposal_line_flow_evidence_sha256: Option<String>,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub camera_rig_object_sha256: String,
    pub camera_rig_canonical_sha256: String,
    pub camera_lock_receipt_object_sha256: String,
    pub camera_lock_source_transition_id: String,
    pub camera_lock_source_transition_sha256: String,
    pub camera_lock_source_head_canonical_sha256: String,
    pub reviewed_reference_view_kinds: Vec<String>,
    pub fixed_camera_view_kinds: Vec<String>,
    pub legacy_form_quality_object_sha256: String,
    pub legacy_form_quality_canonical_sha256: String,
    pub form_art_evidence_object_sha256: String,
    pub form_art_evidence_canonical_sha256: String,
    pub view_decisions: Vec<ProductionWeaponFormQualityV2ViewDecision>,
    pub aggregate: ProductionWeaponFormQualityV2Aggregate,
    pub previous_form_quality_id: Option<String>,
    pub previous_form_quality_report_object_sha256: Option<String>,
    pub previous_form_quality_canonical_sha256: Option<String>,
    pub form_quality_policy: String,
    pub form_quality_policy_sha256: String,
    pub threshold_policy: String,
    pub threshold_policy_sha256: String,
    pub hard_gate_passed: bool,
    pub form_gate_passed: bool,
    pub validator_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2PrepareRequest {
    pub schema_version: String,
    pub form_quality_id: String,
    pub session_id: String,
    pub project_id: String,
    pub form_stage: String,
    pub source_stage: String,
    pub target_stage: String,
    pub legacy_form_quality_object_sha256: String,
    pub legacy_form_quality_canonical_sha256: String,
    pub form_art_evidence_object_sha256: String,
    pub form_art_evidence_canonical_sha256: String,
    pub evidence_source_kind: String,
    pub source_candidate_id: Option<String>,
    pub source_candidate_state_sha256: Option<String>,
    pub source_artifact_id: Option<String>,
    pub source_artifact_sha256: Option<String>,
    pub source_fresh_baseline_id: Option<String>,
    pub source_fresh_baseline_canonical_sha256: Option<String>,
    pub source_fresh_baseline_receipt_object_sha256: Option<String>,
    pub source_registration_lineage_id: Option<String>,
    pub source_registration_lineage_canonical_sha256: Option<String>,
    pub source_registration_lineage_receipt_object_sha256: Option<String>,
    pub source_registered_rig_v2_id: Option<String>,
    pub source_registered_rig_v2_object_sha256: Option<String>,
    pub source_registered_rig_v2_canonical_sha256: Option<String>,
    pub source_runtime_build_cohort_sha256: Option<String>,
    pub proposal_candidate_id: Option<String>,
    pub proposal_candidate_state_sha256: Option<String>,
    pub proposal_artifact_id: Option<String>,
    pub proposal_artifact_sha256: Option<String>,
    pub proposal_artifact_readback_sha256: Option<String>,
    pub proposal_worker_build_cohort_sha256: Option<String>,
    pub cross_view_evidence_bundle_sha256: Option<String>,
    pub proposal_form_art_evidence_id: Option<String>,
    pub proposal_form_art_evidence_object_sha256: Option<String>,
    pub proposal_form_art_evidence_canonical_sha256: Option<String>,
    pub proposal_part_id_evidence_sha256: Option<String>,
    pub proposal_negative_space_evidence_sha256: Option<String>,
    pub proposal_line_flow_evidence_sha256: Option<String>,
    pub current_source_head_transition_id: String,
    pub current_source_head_transition_sha256: String,
    pub current_source_head_canonical_sha256: String,
    pub previous_form_quality_id: Option<String>,
    pub previous_form_quality_report_object_sha256: Option<String>,
    pub previous_form_quality_canonical_sha256: Option<String>,
    pub form_quality_policy: String,
    pub form_quality_policy_sha256: String,
    pub threshold_policy: String,
    pub threshold_policy_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2PrepareResult {
    pub schema_version: String,
    pub form_quality: ProductionWeaponFormQualityV2Record,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2GetRequest {
    pub schema_version: String,
    pub form_quality_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub form_stage: String,
    pub evidence_source_kind: String,
    pub source_candidate_id: Option<String>,
    pub source_candidate_state_sha256: Option<String>,
    pub source_artifact_id: Option<String>,
    pub source_artifact_sha256: Option<String>,
    pub source_fresh_baseline_id: Option<String>,
    pub source_fresh_baseline_canonical_sha256: Option<String>,
    pub source_fresh_baseline_receipt_object_sha256: Option<String>,
    pub source_registration_lineage_id: Option<String>,
    pub source_registration_lineage_canonical_sha256: Option<String>,
    pub source_registration_lineage_receipt_object_sha256: Option<String>,
    pub source_registered_rig_v2_id: Option<String>,
    pub source_registered_rig_v2_object_sha256: Option<String>,
    pub source_registered_rig_v2_canonical_sha256: Option<String>,
    pub source_runtime_build_cohort_sha256: Option<String>,
    pub proposal_candidate_id: Option<String>,
    pub proposal_candidate_state_sha256: Option<String>,
    pub proposal_artifact_id: Option<String>,
    pub proposal_artifact_sha256: Option<String>,
    pub proposal_artifact_readback_sha256: Option<String>,
    pub proposal_worker_build_cohort_sha256: Option<String>,
    pub cross_view_evidence_bundle_sha256: Option<String>,
    pub proposal_form_art_evidence_id: Option<String>,
    pub proposal_form_art_evidence_object_sha256: Option<String>,
    pub proposal_form_art_evidence_canonical_sha256: Option<String>,
    pub proposal_part_id_evidence_sha256: Option<String>,
    pub proposal_negative_space_evidence_sha256: Option<String>,
    pub proposal_line_flow_evidence_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2GetResult {
    pub schema_version: String,
    pub form_quality: ProductionWeaponFormQualityV2Record,
    pub replayed: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub restart_hash_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2PreflightCheck {
    pub status: String,
    pub reason_code: String,
    pub object_sha256: Option<String>,
    pub canonical_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2PreflightGetRequest {
    pub schema_version: String,
    pub preflight_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub form_stage: String,
    pub legacy_form_quality_object_sha256: String,
    pub legacy_form_quality_canonical_sha256: String,
    pub form_art_evidence_object_sha256: String,
    pub form_art_evidence_canonical_sha256: String,
    pub current_source_head_transition_id: String,
    pub current_source_head_transition_sha256: String,
    pub current_source_head_canonical_sha256: String,
    pub input_sha256: String,
    pub evidence_source_kind: String,
    pub source_candidate_id: Option<String>,
    pub source_candidate_state_sha256: Option<String>,
    pub source_artifact_id: Option<String>,
    pub source_artifact_sha256: Option<String>,
    pub source_fresh_baseline_id: Option<String>,
    pub source_fresh_baseline_canonical_sha256: Option<String>,
    pub source_fresh_baseline_receipt_object_sha256: Option<String>,
    pub source_registration_lineage_id: Option<String>,
    pub source_registration_lineage_canonical_sha256: Option<String>,
    pub source_registration_lineage_receipt_object_sha256: Option<String>,
    pub source_registered_rig_v2_id: Option<String>,
    pub source_registered_rig_v2_object_sha256: Option<String>,
    pub source_registered_rig_v2_canonical_sha256: Option<String>,
    pub source_runtime_build_cohort_sha256: Option<String>,
    pub proposal_candidate_id: Option<String>,
    pub proposal_candidate_state_sha256: Option<String>,
    pub proposal_artifact_id: Option<String>,
    pub proposal_artifact_sha256: Option<String>,
    pub proposal_artifact_readback_sha256: Option<String>,
    pub proposal_worker_build_cohort_sha256: Option<String>,
    pub cross_view_evidence_bundle_sha256: Option<String>,
    pub proposal_form_art_evidence_id: Option<String>,
    pub proposal_form_art_evidence_object_sha256: Option<String>,
    pub proposal_form_art_evidence_canonical_sha256: Option<String>,
    pub proposal_part_id_evidence_sha256: Option<String>,
    pub proposal_negative_space_evidence_sha256: Option<String>,
    pub proposal_line_flow_evidence_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponFormQualityV2PreflightGetResult {
    pub schema_version: String,
    pub preflight_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub form_stage: String,
    pub evidence_source_kind: String,
    pub source_candidate_id: Option<String>,
    pub source_candidate_state_sha256: Option<String>,
    pub source_artifact_id: Option<String>,
    pub source_artifact_sha256: Option<String>,
    pub source_fresh_baseline_id: Option<String>,
    pub source_fresh_baseline_canonical_sha256: Option<String>,
    pub source_fresh_baseline_receipt_object_sha256: Option<String>,
    pub source_registration_lineage_id: Option<String>,
    pub source_registration_lineage_canonical_sha256: Option<String>,
    pub source_registration_lineage_receipt_object_sha256: Option<String>,
    pub source_registered_rig_v2_id: Option<String>,
    pub source_registered_rig_v2_object_sha256: Option<String>,
    pub source_registered_rig_v2_canonical_sha256: Option<String>,
    pub source_runtime_build_cohort_sha256: Option<String>,
    pub proposal_candidate_id: Option<String>,
    pub proposal_candidate_state_sha256: Option<String>,
    pub proposal_artifact_id: Option<String>,
    pub proposal_artifact_sha256: Option<String>,
    pub proposal_artifact_readback_sha256: Option<String>,
    pub proposal_worker_build_cohort_sha256: Option<String>,
    pub cross_view_evidence_bundle_sha256: Option<String>,
    pub proposal_form_art_evidence_id: Option<String>,
    pub proposal_form_art_evidence_object_sha256: Option<String>,
    pub proposal_form_art_evidence_canonical_sha256: Option<String>,
    pub proposal_part_id_evidence_sha256: Option<String>,
    pub proposal_negative_space_evidence_sha256: Option<String>,
    pub proposal_line_flow_evidence_sha256: Option<String>,
    pub checks: BTreeMap<String, ProductionWeaponFormQualityV2PreflightCheck>,
    pub ready_for_v2_prepare: bool,
    pub blocking_reasons: Vec<String>,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub runtime_write: bool,
    pub worker_started: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub restart_hash_verified: bool,
    pub readiness_sha256: String,
}

// FPS-HIGH-LOW-CAGE-05 is intentionally additive.  These contracts describe
// independent high, low and cage artifacts plus correspondence and bounded
// ray diagnostics; they do not widen CandidateSurfaceBake@1 or any historical
// LOD receipt, and they never advance ProductionStage@3 by themselves.
pub const PRODUCTION_WEAPON_HIGH_ARTIFACT_SCHEMA_VERSION: &str = "ProductionWeaponHighArtifact@1";
pub const PRODUCTION_WEAPON_LOW_ARTIFACT_SCHEMA_VERSION: &str = "ProductionWeaponLowArtifact@1";
pub const PRODUCTION_WEAPON_CAGE_ARTIFACT_SCHEMA_VERSION: &str = "ProductionWeaponCageArtifact@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowCorrespondence@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_PLAN_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowBakePlan@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_DIAGNOSTIC_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowDiagnostic@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_RECEIPT_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowBakeReceipt@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREPARE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowBakePrepareRequest@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREPARE_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowBakePrepareResult@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowBakeGetRequest@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowBakeGetResult@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowBakePreflightGetRequest@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREFLIGHT_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowBakePreflightGetResult@1";

pub const PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY: &str =
    "production-weapon-independent-high-detail-graph@1";
pub const PRODUCTION_WEAPON_LOW_ARTIFACT_POLICY: &str =
    "production-weapon-independent-low-retopology@1";
pub const PRODUCTION_WEAPON_CAGE_ARTIFACT_POLICY: &str =
    "production-weapon-low-bound-cage-offset-field@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_POLICY: &str =
    "production-weapon-high-low-cage-part-face-corner-correspondence@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_PLAN_POLICY: &str =
    "production-weapon-high-low-cage-ray-diagnostic-plan@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_DIAGNOSTIC_POLICY: &str =
    "production-weapon-high-low-cage-ray-diagnostic@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_POLICY: &str =
    "production-weapon-high-low-cage-bake-gate@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_MODE: &str = "independent-high-low-cage-ray-bake@1";
pub const PRODUCTION_WEAPON_HIGH_LOW_NORMAL_CONVENTION: &str = "OpenGL+Y";

pub const PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND: &str = "production-weapon-high-artifact-glb";
pub const PRODUCTION_WEAPON_LOW_ARTIFACT_KIND: &str = "production-weapon-low-artifact-glb";
pub const PRODUCTION_WEAPON_CAGE_ARTIFACT_KIND: &str = "production-weapon-cage-artifact-glb";
pub const PRODUCTION_WEAPON_HIGH_ARTIFACT_RECEIPT_KIND: &str =
    "production-weapon-high-artifact-receipt";
pub const PRODUCTION_WEAPON_LOW_ARTIFACT_RECEIPT_KIND: &str =
    "production-weapon-low-artifact-receipt";
pub const PRODUCTION_WEAPON_CAGE_ARTIFACT_RECEIPT_KIND: &str =
    "production-weapon-cage-artifact-receipt";
pub const PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_KIND: &str =
    "production-weapon-high-low-correspondence";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_PLAN_KIND: &str = "production-weapon-high-low-bake-plan";
pub const PRODUCTION_WEAPON_HIGH_LOW_DIAGNOSTIC_KIND: &str =
    "production-weapon-high-low-diagnostic";
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_RECEIPT_KIND: &str =
    "production-weapon-high-low-bake-receipt";

pub const PRODUCTION_WEAPON_HIGH_LOW_STRUCTURAL_STATUSES: &[&str] =
    &["NOT_RUN", "BLOCKED", "PASS_SOURCE_STRUCTURAL"];
pub const PRODUCTION_WEAPON_HIGH_LOW_VISUAL_STATUSES: &[&str] =
    &["NOT_RUN", "BLOCKED", "QUALITY_TARGET_NOT_MET", "NOT_PROVEN"];
pub const PRODUCTION_WEAPON_HIGH_LOW_HUMAN_STATUSES: &[&str] =
    &["NOT_RUN", "BLOCKED", "REJECTED", "PASS_HUMAN_ART_REVIEW"];
pub const PRODUCTION_WEAPON_HIGH_LOW_ENGINE_STATUSES: &[&str] =
    &["NOT_RUN", "BLOCKED", "FAILED", "PASS_ENGINE_VALIDATION"];
pub const PRODUCTION_WEAPON_HIGH_LOW_DISTRIBUTION_STATUSES: &[&str] =
    &["NOT_RUN", "BLOCKED", "FAILED", "PASS_DISTRIBUTION"];
pub const PRODUCTION_WEAPON_HIGH_LOW_BAKE_STATUSES: &[&str] = &[
    "NOT_HIGH_LOW_BAKE",
    "DIAGNOSTIC_ONLY",
    "PASS_SOURCE_STRUCTURAL",
];
pub const PRODUCTION_WEAPON_HIGH_LOW_GATE_SCOPES: &[&str] = &[
    "high-artifact",
    "low-artifact",
    "cage-artifact",
    "high-low-bake",
];
pub const PRODUCTION_WEAPON_HIGH_LOW_SOURCE_STAGES: &[&str] = &[
    "secondary-form-approved",
    "high-poly-approved",
    "low-poly-approved",
    "cage-approved",
];
pub const PRODUCTION_WEAPON_HIGH_LOW_TARGET_STAGES: &[&str] = &[
    "high-poly-approved",
    "low-poly-approved",
    "cage-approved",
    "bake-approved",
];
pub const PRODUCTION_WEAPON_HIGH_LOW_OUTPUT_SEMANTICS: &[&str] = &[
    "tangent-normal",
    "ao",
    "curvature",
    "thickness",
    "position",
    "object-id",
    "material-id",
    "part-id",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponHighArtifactRecord {
    pub schema_version: String,
    pub high_artifact_id: String,
    pub session_id: String,
    pub project_id: String,
    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub source_stage_head_stage: String,
    pub source_candidate_id: String,
    pub source_candidate_state_sha256: String,
    pub source_artifact_id: String,
    pub source_artifact_sha256: String,
    pub source_artifact_readback_sha256: String,
    pub high_candidate_id: String,
    pub high_candidate_state_sha256: String,
    pub high_artifact_sha256: String,
    pub high_artifact_readback_sha256: String,
    pub high_artifact_readback_object_sha256: String,
    pub high_geometry_program_sha256: String,
    pub high_geometry_program_object_sha256: String,
    pub high_geometry_candidate_evidence_sha256: String,
    pub high_detail_graph_object_sha256: String,
    pub high_detail_graph_canonical_sha256: String,
    pub high_part_inventory_sha256: String,
    pub high_part_ids: Vec<String>,
    pub high_material_zone_ids: Vec<String>,
    pub high_policy: String,
    pub high_policy_sha256: String,
    pub high_artifact_kind: String,
    pub high_mime: String,
    pub high_size_bytes: u64,
    pub high_worker_algorithm_sha256: String,
    pub high_worker_build_cohort_sha256: String,
    pub high_worker_replay_count: u64,
    pub high_replay_byte_exact: bool,
    pub high_topology_status: String,
    pub high_authoring_topology_status: String,
    pub high_uv_status: String,
    pub high_tangent_status: String,
    pub validator_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub quality_status: String,
    pub hard_gate_passed: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponLowArtifactRecord {
    pub schema_version: String,
    pub low_artifact_id: String,
    pub session_id: String,
    pub project_id: String,
    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub source_stage_head_stage: String,
    pub source_high_candidate_id: String,
    pub source_high_candidate_state_sha256: String,
    pub source_high_artifact_id: String,
    pub source_high_artifact_sha256: String,
    pub source_high_artifact_readback_sha256: String,
    pub low_candidate_id: String,
    pub low_candidate_state_sha256: String,
    pub low_artifact_sha256: String,
    pub low_artifact_readback_sha256: String,
    pub low_artifact_readback_object_sha256: String,
    pub low_geometry_program_sha256: String,
    pub low_geometry_program_object_sha256: String,
    pub low_geometry_candidate_evidence_sha256: String,
    pub low_part_inventory_sha256: String,
    pub low_part_ids: Vec<String>,
    pub low_material_zone_ids: Vec<String>,
    pub low_retopology_policy: String,
    pub low_retopology_policy_sha256: String,
    pub low_triangle_budget_sha256: String,
    pub low_triangle_count: u64,
    pub low_part_triangle_counts_sha256: String,
    pub low_authoring_topology_status: String,
    pub low_authoring_topology_object_sha256: String,
    pub low_authoring_topology_canonical_sha256: String,
    pub low_uv_binding_sha256: String,
    pub low_tangent_input_sha256: String,
    pub low_artifact_kind: String,
    pub low_mime: String,
    pub low_size_bytes: u64,
    pub low_worker_algorithm_sha256: String,
    pub low_worker_build_cohort_sha256: String,
    pub low_worker_replay_count: u64,
    pub low_replay_byte_exact: bool,
    pub low_topology_status: String,
    pub low_uv_status: String,
    pub low_tangent_status: String,
    pub validator_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub quality_status: String,
    pub hard_gate_passed: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponCageArtifactRecord {
    pub schema_version: String,
    pub cage_artifact_id: String,
    pub session_id: String,
    pub project_id: String,
    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub source_stage_head_stage: String,
    pub source_high_candidate_id: String,
    pub source_high_candidate_state_sha256: String,
    pub source_high_artifact_id: String,
    pub source_high_artifact_sha256: String,
    pub source_high_artifact_readback_sha256: String,
    pub source_low_candidate_id: String,
    pub source_low_candidate_state_sha256: String,
    pub source_low_artifact_id: String,
    pub source_low_artifact_sha256: String,
    pub source_low_artifact_readback_sha256: String,
    pub cage_artifact_sha256: String,
    pub cage_artifact_readback_sha256: String,
    pub cage_artifact_readback_object_sha256: String,
    pub cage_geometry_program_sha256: String,
    pub cage_geometry_program_object_sha256: String,
    pub cage_geometry_candidate_evidence_sha256: String,
    pub cage_part_inventory_sha256: String,
    pub cage_part_ids: Vec<String>,
    pub cage_material_zone_ids: Vec<String>,
    pub cage_policy: String,
    pub cage_policy_sha256: String,
    pub cage_topology_correspondence_sha256: String,
    pub cage_offset_field_object_sha256: String,
    pub cage_offset_field_canonical_sha256: String,
    pub cage_offset_min_m: f64,
    pub cage_offset_max_m: f64,
    pub cage_offset_space: String,
    pub cage_artifact_kind: String,
    pub cage_mime: String,
    pub cage_size_bytes: u64,
    pub cage_self_intersection_count: u64,
    pub cage_cross_part_count: u64,
    pub cage_out_of_range_count: u64,
    pub cage_skew_count: u64,
    pub cage_worker_algorithm_sha256: String,
    pub cage_worker_build_cohort_sha256: String,
    pub cage_worker_replay_count: u64,
    pub cage_replay_byte_exact: bool,
    pub cage_topology_status: String,
    pub validator_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub quality_status: String,
    pub hard_gate_passed: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponHighLowPartPair {
    pub part_id: String,
    pub high_part_id: String,
    pub low_part_id: String,
    pub cage_part_id: String,
    pub material_zone_id: String,
    pub high_source_node_id: String,
    pub low_source_node_id: String,
    pub cage_source_node_id: String,
    pub high_face_count: u64,
    pub low_face_count: u64,
    pub cage_face_count: u64,
    pub vertex_map_sha256: String,
    pub face_map_sha256: String,
    pub mapping_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponHighLowCorrespondenceRecord {
    pub schema_version: String,
    pub correspondence_id: String,
    pub session_id: String,
    pub project_id: String,
    pub high_candidate_id: String,
    pub high_candidate_state_sha256: String,
    pub high_artifact_id: String,
    pub high_artifact_sha256: String,
    pub high_artifact_readback_sha256: String,
    pub low_candidate_id: String,
    pub low_candidate_state_sha256: String,
    pub low_artifact_id: String,
    pub low_artifact_sha256: String,
    pub low_artifact_readback_sha256: String,
    pub cage_artifact_id: String,
    pub cage_artifact_sha256: String,
    pub cage_artifact_readback_sha256: String,
    pub part_inventory_sha256: String,
    pub part_ids: Vec<String>,
    pub material_zone_ids: Vec<String>,
    pub correspondence_policy: String,
    pub correspondence_policy_sha256: String,
    pub part_pairs: Vec<ProductionWeaponHighLowPartPair>,
    pub mapping_object_sha256: String,
    pub mapping_canonical_sha256: String,
    pub unmapped_count: u64,
    pub ambiguous_count: u64,
    pub cross_part_count: u64,
    pub cross_material_zone_count: u64,
    pub stable_identity_policy: String,
    pub worker_algorithm_sha256: String,
    pub worker_build_cohort_sha256: String,
    pub worker_replay_count: u64,
    pub replay_byte_exact: bool,
    pub mapping_status: String,
    pub validator_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub quality_status: String,
    pub hard_gate_passed: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponHighLowBakePlanRecord {
    pub schema_version: String,
    pub bake_plan_id: String,
    pub session_id: String,
    pub project_id: String,
    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub source_stage_head_stage: String,
    pub high_candidate_id: String,
    pub high_candidate_state_sha256: String,
    pub high_artifact_id: String,
    pub high_artifact_sha256: String,
    pub high_artifact_readback_sha256: String,
    pub low_candidate_id: String,
    pub low_candidate_state_sha256: String,
    pub low_artifact_id: String,
    pub low_artifact_sha256: String,
    pub low_artifact_readback_sha256: String,
    pub cage_artifact_id: String,
    pub cage_artifact_sha256: String,
    pub cage_artifact_readback_sha256: String,
    pub correspondence_id: String,
    pub correspondence_object_sha256: String,
    pub correspondence_canonical_sha256: String,
    pub low_uv_binding_sha256: String,
    pub low_tangent_binding_sha256: String,
    pub material_zone_binding_sha256: String,
    pub normal_convention: String,
    pub ray_origin_policy: String,
    pub ray_direction_policy: String,
    pub ray_distance_policy: String,
    pub front_back_policy: String,
    pub per_part_isolation_policy: String,
    pub anti_cross_hit_policy: String,
    pub max_ray_distance_m: f64,
    pub output_semantics: Vec<String>,
    pub diagnostic_required: bool,
    pub surface_bake_reuse_allowed: bool,
    pub bake_mode: String,
    pub bake_policy: String,
    pub bake_policy_sha256: String,
    pub worker_build_cohort_sha256: String,
    pub worker_replay_count: u64,
    pub replay_byte_exact: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponHighLowDiagnosticRecord {
    pub schema_version: String,
    pub diagnostic_id: String,
    pub session_id: String,
    pub project_id: String,
    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub source_stage_head_stage: String,
    pub high_artifact_id: String,
    pub high_artifact_sha256: String,
    pub high_artifact_readback_sha256: String,
    pub low_artifact_id: String,
    pub low_artifact_sha256: String,
    pub low_artifact_readback_sha256: String,
    pub cage_artifact_id: String,
    pub cage_artifact_sha256: String,
    pub cage_artifact_readback_sha256: String,
    pub correspondence_id: String,
    pub correspondence_object_sha256: String,
    pub correspondence_canonical_sha256: String,
    pub bake_plan_id: String,
    pub bake_plan_object_sha256: String,
    pub bake_plan_canonical_sha256: String,
    pub low_uv_binding_sha256: String,
    pub low_tangent_binding_sha256: String,
    pub material_zone_binding_sha256: String,
    pub normal_convention: String,
    pub ray_origin_policy: String,
    pub ray_direction_policy: String,
    pub ray_distance_policy: String,
    pub front_back_policy: String,
    pub per_part_isolation_policy: String,
    pub anti_cross_hit_policy: String,
    pub max_ray_distance_m: f64,
    pub max_observed_distance_m: f64,
    pub ray_sample_count: u64,
    pub ray_hit_count: u64,
    pub ray_miss_count: u64,
    pub backface_hit_count: u64,
    pub skew_count: u64,
    pub cross_part_hit_count: u64,
    pub cage_intersection_count: u64,
    pub overlap_count: u64,
    pub out_of_range_count: u64,
    pub distance_histogram_object_sha256: String,
    pub distance_histogram_canonical_sha256: String,
    pub diagnostic_heatmap_object_sha256: String,
    pub diagnostic_heatmap_canonical_sha256: String,
    pub diagnostic_policy: String,
    pub diagnostic_policy_sha256: String,
    pub bake_mode: String,
    pub surface_bake_reuse_allowed: bool,
    pub diagnostic_status: String,
    pub high_low_bake_status: String,
    pub worker_algorithm_sha256: String,
    pub worker_build_cohort_sha256: String,
    pub worker_replay_count: u64,
    pub replay_byte_exact: bool,
    pub validator_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub quality_status: String,
    pub hard_gate_passed: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponHighLowHardGate {
    pub distinct_high_low_cage_bindings: bool,
    pub high_readback_verified: bool,
    pub low_readback_verified: bool,
    pub cage_readback_verified: bool,
    pub low_authoring_topology_verified: bool,
    pub correspondence_verified: bool,
    pub uv_tangent_binding_verified: bool,
    pub ray_diagnostic_verified: bool,
    pub no_candidate_surface_bake_reuse: bool,
    pub same_cohort_replay_verified: bool,
    pub output_byte_exact: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponHighLowBakeReceiptRecord {
    pub schema_version: String,
    pub bake_receipt_id: String,
    pub session_id: String,
    pub project_id: String,
    pub gate_scope: String,
    pub source_stage: String,
    pub target_stage: String,
    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub source_stage_head_stage: String,
    pub high_candidate_id: String,
    pub high_candidate_state_sha256: String,
    pub high_artifact_id: String,
    pub high_artifact_sha256: String,
    pub high_artifact_readback_sha256: String,
    pub low_candidate_id: String,
    pub low_candidate_state_sha256: String,
    pub low_artifact_id: String,
    pub low_artifact_sha256: String,
    pub low_artifact_readback_sha256: String,
    pub cage_artifact_id: String,
    pub cage_artifact_sha256: String,
    pub cage_artifact_readback_sha256: String,
    pub correspondence_id: String,
    pub correspondence_object_sha256: String,
    pub correspondence_canonical_sha256: String,
    pub bake_plan_id: String,
    pub bake_plan_object_sha256: String,
    pub bake_plan_canonical_sha256: String,
    pub diagnostic_id: String,
    pub diagnostic_object_sha256: String,
    pub diagnostic_canonical_sha256: String,
    pub bake_policy: String,
    pub bake_policy_sha256: String,
    pub high_status: String,
    pub low_status: String,
    pub cage_status: String,
    pub correspondence_status: String,
    pub diagnostic_status: String,
    pub high_low_bake_status: String,
    pub bake_output_object_sha256s: Vec<String>,
    pub hard_gate: ProductionWeaponHighLowHardGate,
    pub hard_gate_passed: bool,
    pub validator_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub stage_advance_allowed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub limitations: Vec<String>,
    pub request_sha256: String,
    pub input_sha256: String,
    pub receipt_object_sha256: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponHighLowBakePrepareRequest {
    pub schema_version: String,
    pub bake_receipt_id: String,
    pub session_id: String,
    pub project_id: String,
    pub gate_scope: String,
    pub source_stage: String,
    pub target_stage: String,
    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub source_stage_head_stage: String,
    pub high_candidate_id: String,
    pub high_candidate_state_sha256: String,
    pub high_artifact_id: String,
    pub high_artifact_sha256: String,
    pub high_artifact_readback_sha256: String,
    pub low_candidate_id: String,
    pub low_candidate_state_sha256: String,
    pub low_artifact_id: String,
    pub low_artifact_sha256: String,
    pub low_artifact_readback_sha256: String,
    pub cage_artifact_id: String,
    pub cage_artifact_sha256: String,
    pub cage_artifact_readback_sha256: String,
    pub correspondence_id: String,
    pub correspondence_object_sha256: String,
    pub correspondence_canonical_sha256: String,
    pub bake_plan_id: String,
    pub bake_plan_object_sha256: String,
    pub bake_plan_canonical_sha256: String,
    pub bake_policy: String,
    pub bake_policy_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponHighLowBakePrepareResult {
    pub schema_version: String,
    pub bake_receipt_id: String,
    pub bake_receipt_object_sha256: String,
    pub bake_receipt: ProductionWeaponHighLowBakeReceiptRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponHighLowBakeGetRequest {
    pub schema_version: String,
    pub bake_receipt_id: String,
    pub session_id: String,
    pub project_id: String,
    pub gate_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductionWeaponHighLowBakeGetResult {
    pub schema_version: String,
    pub bake_receipt_id: String,
    pub bake_receipt_object_sha256: String,
    pub bake_receipt: ProductionWeaponHighLowBakeReceiptRecord,
    pub replayed: bool,
    pub restart_hash_verified: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponHighLowBakePreflightCheck {
    pub status: String,
    pub reason_code: String,
    pub object_sha256: Option<String>,
    pub canonical_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponHighLowBakePreflightGetRequest {
    pub schema_version: String,
    pub preflight_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub expected_head_stage: String,
    pub expected_head_transition_id: String,
    pub expected_head_transition_sha256: String,
    pub expected_head_canonical_sha256: String,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionWeaponHighLowBakePreflightGetResult {
    pub schema_version: String,
    pub preflight_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub expected_head_stage: String,
    pub observed_head_stage: Option<String>,
    pub observed_head_transition_id: Option<String>,
    pub observed_head_transition_sha256: Option<String>,
    pub observed_head_canonical_sha256: Option<String>,
    pub checks: BTreeMap<String, ProductionWeaponHighLowBakePreflightCheck>,
    pub ready_for_formal_bake: bool,
    pub blocking_reasons: Vec<String>,
    pub quality_status: String,
    pub visual_quality_status: String,
    pub human_review_status: String,
    pub commercial_engine_status: String,
    pub distribution_status: String,
    pub runtime_write: bool,
    pub worker_started: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub restart_hash_verified: bool,
    pub readiness_sha256: String,
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
    /// Optional typed context carried by ApprovalReceipt@1.  Existing
    /// approval flows omit this field; the production CameraLock lineage
    /// uses it to bind the exact authored orientation to its scope and the
    /// Runtime-derived camera proof without adding another top-level receipt
    /// contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_context: Option<ApprovalReceiptContextRecord>,
}

/// Closed ApprovalReceipt@1 context for the production CameraLock authored
/// orientation gate.  This is intentionally nested under the existing
/// receipt contract so generic approvals keep their historical wire shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalReceiptContextRecord {
    pub schema_version: String,
    pub policy: String,
    pub scope: ApprovalReceiptContextScope,
    pub orientation: ApprovalReceiptOrientation,
    pub binding_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalReceiptContextScope {
    pub project_id: String,
    pub session_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub registration_lineage_id: String,
    pub camera_lock_id: String,
    pub camera_lock_canonical_sha256: String,
    pub authored_orientation_id: String,
    pub registered_rig_v2_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalReceiptOrientation {
    pub rotation_degrees: i64,
    pub subject_screen_order: String,
    pub upright: bool,
    pub screen_up: String,
    pub derived_camera_orbit_degrees: i64,
    pub derived_camera_hash: String,
    pub derived_camera_canonical_sha256: String,
    pub semantic_orientation_proof_sha256: String,
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
