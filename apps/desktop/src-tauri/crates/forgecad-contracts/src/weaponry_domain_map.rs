//! Compile-time ownership map for the Weaponry knife profile.
//!
//! This is the only authoritative Contract -> Runtime service -> Store
//! persistence -> MCP façade directory.  Runtime and Store expose typed views
//! of this data; they must not maintain a second façade/domain table.  The map
//! describes ownership, not quality or completion.  In particular, a read
//! model may be a projection and a pure evaluation may intentionally have no
//! durable Store record.

/// Version of the compile-time ownership directory.
pub const WEAPONRY_DOMAIN_MAP_SCHEMA_VERSION: &str = "WeaponryDomainMap@1";

/// Stable identity of the five implementation-facing Weaponry services.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum WeaponryServiceDomain {
    Authoring,
    Evaluation,
    Surface,
    Presentation,
    Delivery,
}

impl WeaponryServiceDomain {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Authoring => "authoring",
            Self::Evaluation => "evaluation",
            Self::Surface => "surface",
            Self::Presentation => "presentation",
            Self::Delivery => "delivery",
        }
    }

    pub const fn id(self) -> &'static str {
        self.as_str()
    }

    pub const fn all() -> [Self; 5] {
        [
            Self::Authoring,
            Self::Evaluation,
            Self::Surface,
            Self::Presentation,
            Self::Delivery,
        ]
    }
}

/// Persistence semantics for a mapped capability.
///
/// `None` is intentional for pure computation/readback that is recomputed
/// from immutable inputs.  `Projection` means a Store read model may exist,
/// but it is not the source of truth for the underlying artifact.  Only
/// `DurableTransaction` claims a durable Runtime-owned transaction record.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PersistenceKind {
    None,
    Projection,
    DurableTransaction,
}

impl PersistenceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Projection => "projection",
            Self::DurableTransaction => "durable_transaction",
        }
    }
}

/// Whether the four-layer mapping is complete at the ownership seam.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MappingStatus {
    Complete,
    Partial,
    Gap,
}

/// One of the eleven public knife façades and its single Runtime service.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct KnifeFacadeBinding {
    pub facade_name: &'static str,
    pub domain: WeaponryServiceDomain,
}

/// The process boundary that executes an operation after its public façade
/// and service domain have been validated.
///
/// Almost every Knife operation is Runtime-owned. `McpAdapter` is reserved
/// for bounded transport/control-plane projections that cannot truthfully be
/// produced by Runtime design state. Keeping this exception here prevents
/// MCP and Runtime from growing separate ownership rules.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum WeaponryOperationExecutionTarget {
    Runtime,
    McpAdapter,
}

impl WeaponryOperationExecutionTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::McpAdapter => "mcp_adapter",
        }
    }
}

const MCP_ADAPTER_OPERATIONS: &[&str] = &["doctor", "runtime_status"];

/// Return the centrally declared execution target for a validated Knife
/// operation. Unknown-operation rejection remains the profile/router's job;
/// this function only distinguishes the bounded MCP-local control plane from
/// normal Runtime execution.
pub fn knife_operation_execution_target(operation: &str) -> WeaponryOperationExecutionTarget {
    if MCP_ADAPTER_OPERATIONS.contains(&operation) {
        WeaponryOperationExecutionTarget::McpAdapter
    } else {
        WeaponryOperationExecutionTarget::Runtime
    }
}

pub const KNIFE_FACADE_BINDINGS: [KnifeFacadeBinding; 11] = [
    KnifeFacadeBinding {
        facade_name: "weapon_preflight",
        domain: WeaponryServiceDomain::Authoring,
    },
    KnifeFacadeBinding {
        facade_name: "reference_intake",
        domain: WeaponryServiceDomain::Authoring,
    },
    KnifeFacadeBinding {
        facade_name: "observe",
        domain: WeaponryServiceDomain::Evaluation,
    },
    KnifeFacadeBinding {
        facade_name: "authoring_transaction",
        domain: WeaponryServiceDomain::Authoring,
    },
    KnifeFacadeBinding {
        facade_name: "surface_pipeline",
        domain: WeaponryServiceDomain::Surface,
    },
    KnifeFacadeBinding {
        facade_name: "fps_presentation",
        domain: WeaponryServiceDomain::Presentation,
    },
    KnifeFacadeBinding {
        facade_name: "quality_review",
        domain: WeaponryServiceDomain::Evaluation,
    },
    KnifeFacadeBinding {
        facade_name: "delivery",
        domain: WeaponryServiceDomain::Delivery,
    },
    KnifeFacadeBinding {
        facade_name: "approval",
        domain: WeaponryServiceDomain::Delivery,
    },
    KnifeFacadeBinding {
        facade_name: "recovery",
        domain: WeaponryServiceDomain::Authoring,
    },
    KnifeFacadeBinding {
        facade_name: "job",
        domain: WeaponryServiceDomain::Evaluation,
    },
];

const AUTHORING_FACADES: &[&str] = &[
    "weapon_preflight",
    "reference_intake",
    "authoring_transaction",
    "recovery",
];
const EVALUATION_FACADES: &[&str] = &["observe", "quality_review", "job"];
const SURFACE_FACADES: &[&str] = &["surface_pipeline"];
const PRESENTATION_FACADES: &[&str] = &["fps_presentation"];
const DELIVERY_FACADES: &[&str] = &["delivery", "approval"];

/// Return the authoritative façade names for a service domain.
pub const fn knife_facades_for_domain(domain: WeaponryServiceDomain) -> &'static [&'static str] {
    match domain {
        WeaponryServiceDomain::Authoring => AUTHORING_FACADES,
        WeaponryServiceDomain::Evaluation => EVALUATION_FACADES,
        WeaponryServiceDomain::Surface => SURFACE_FACADES,
        WeaponryServiceDomain::Presentation => PRESENTATION_FACADES,
        WeaponryServiceDomain::Delivery => DELIVERY_FACADES,
    }
}

/// Look up the sole Runtime owner of a public façade.
pub fn knife_facade_binding(facade_name: &str) -> Option<&'static KnifeFacadeBinding> {
    KNIFE_FACADE_BINDINGS
        .iter()
        .find(|binding| binding.facade_name == facade_name)
}

/// One capability-level Contract -> Runtime -> Store -> MCP mapping.
///
/// The `store_record` field is only populated when `persistence` is
/// `DurableTransaction` or `Projection`.  A `None` persistence kind must not
/// be represented as a fake durable record.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct WeaponryCapabilityMapping {
    pub capability: &'static str,
    pub domain: WeaponryServiceDomain,
    pub contract: Option<&'static str>,
    pub runtime_service: Option<&'static str>,
    pub store_record: Option<&'static str>,
    pub persistence: PersistenceKind,
    pub mcp_facade: Option<&'static str>,
    pub mcp_operations: &'static [&'static str],
    pub status: MappingStatus,
}

const AUTHORING_TRANSACTION_WRITE_OPERATIONS: &[&str] = &["authoring_mesh_transaction_prepare"];
const AUTHORING_TRANSACTION_READBACK_OPERATIONS: &[&str] = &["authoring_mesh_transaction_get"];
const AUTHORING_MESH_V2_CANDIDATE_MATERIALIZE_OPERATIONS: &[&str] =
    &["authoring_mesh_v2_candidate_materialize"];
const AUTHORING_V2_DURABLE_WRITE_OPERATIONS: &[&str] = &["authoring_mesh_v2_durable_prepare"];
const AUTHORING_V2_DURABLE_READBACK_OPERATIONS: &[&str] = &["authoring_mesh_v2_durable_get"];
const AUTHORING_MESH_V2_SOURCE_OPERATIONS: &[&str] =
    &["production_weapon_authoring_mesh_v2_source_prepare"];
const AUTHORING_MESH_V2_HIGH_BRIDGE_OPERATIONS: &[&str] = &[
    "authoring_mesh_v2_high_bridge_get",
    "authoring_mesh_v2_high_bridge_prepare",
];
const AUTHORING_MESH_V2_HIGH_ARTIFACT_OPERATIONS: &[&str] = &[
    "authoring_mesh_v2_high_artifact_get",
    "authoring_mesh_v2_high_artifact_prepare",
];
const FOUNDATION_IMPORT_OPERATIONS: &[&str] = &[
    "weapon_foundation_asset_get",
    "weapon_foundation_asset_prepare",
];
const KNIFE_PRODUCTION_BRIEF_OPERATIONS: &[&str] = &[
    "weaponry_knife_production_brief_get",
    "weaponry_knife_production_brief_prepare",
];
const KNIFE_REFERENCE_INTENT_BUNDLE_OPERATIONS: &[&str] = &[
    "knife_reference_intent_bundle_get",
    "knife_reference_intent_bundle_prepare",
];
const KNIFE_SOURCE_BINDING_OPERATIONS: &[&str] =
    &["knife_source_binding_get", "knife_source_binding_prepare"];
const FOUNDATION_MATERIALIZATION_OPERATIONS: &[&str] = &[
    "weapon_foundation_authoring_materialization_get",
    "weapon_foundation_authoring_materialization_prepare",
];
const FORMAL_HIGH_OPERATIONS: &[&str] = &[
    "production_weapon_formal_high_get",
    "production_weapon_formal_high_prepare",
];
const CURVE_GRAPH_OPERATIONS: &[&str] = &[
    "knife_curve_modifier_graph_get",
    "knife_curve_modifier_graph_prepare",
];
const EVALUATED_MESH_OPERATIONS: &[&str] = &[
    "knife_curve_evaluated_mesh_get",
    "knife_curve_evaluated_mesh_prepare",
];
const LOW_QUAD_OPERATIONS: &[&str] = &[
    "low_quad_draft_durable_get",
    "low_quad_draft_durable_prepare",
];
const HERO_UV_OPERATIONS: &[&str] = &["hero_uv_durable_get", "hero_uv_durable_prepare"];
const KNIFE_UV_BAKE_V2_OPERATIONS: &[&str] = &[
    "production_knife_uv_bake_v2_get",
    "production_knife_uv_bake_v2_prepare",
];
const HIGH_LOW_BAKE_OPERATIONS: &[&str] = &[
    "production_weapon_high_low_bake_get",
    "production_weapon_high_low_bake_preflight_get",
    "production_weapon_high_low_bake_prepare",
];
const FPS_PACKAGE_OPERATIONS: &[&str] = &[
    "fps_presentation_package_v2_get",
    "fps_presentation_package_v2_prepare",
    "fps_presentation_package_v2_production_preflight_get",
];
const FPS_CANDIDATE_OPERATIONS: &[&str] = &[
    "fps_presentation_package_v2_candidate_get",
    "fps_presentation_package_v2_candidate_prepare",
];
const FPS_ANCHOR_OPERATIONS: &[&str] = &["game_weapon_anchor_get", "game_weapon_anchor_prepare"];
const FPS_ANIMATED_SOCKET_OPERATIONS: &[&str] = &[
    "game_weapon_animated_glb_socket_get",
    "game_weapon_animated_glb_socket_prepare",
];
const FPS_SOCKET_TRANSFORM_PROJECTION_OPERATIONS: &[&str] = &[
    "game_weapon_animated_glb_socket_transform_projection_get",
    "game_weapon_animated_glb_socket_transform_projection_prepare",
];
const FPS_SOCKET_TRANSFORM_PROJECTION_V2_OPERATIONS: &[&str] = &[
    "game_weapon_animated_glb_socket_transform_projection_v2_get",
    "game_weapon_animated_glb_socket_transform_projection_v2_prepare",
];
const FPS_ANIMATION_CLIP_OPERATIONS: &[&str] = &[
    "mechanical_animation_clip_get",
    "mechanical_animation_clip_prepare",
    "mechanical_animation_clip_preview_get",
];
const FPS_ANIMATION_CLIP_V2_OPERATIONS: &[&str] = &[
    "mechanical_animation_clip_v2_get",
    "mechanical_animation_clip_v2_prepare",
    "mechanical_animation_clip_v2_preview",
];
const FPS_ANIMATION_GLB_V2_OPERATIONS: &[&str] = &[
    "mechanical_animation_glb_v2_get",
    "mechanical_animation_glb_v2_prepare",
];

const OBSERVE_OPERATIONS: &[&str] = &["candidate_get", "snapshot_get", "scene_observe_get"];
const SILHOUETTE_OPERATIONS: &[&str] = &[
    "silhouette_evaluation_objective_prepare",
    "silhouette_fit_prepare",
];
const KNIFE_PASS_STATE_OPERATIONS: &[&str] = &["knife_pass_state_get", "knife_pass_state_prepare"];
const HIGH_ARTIFACT_REFERENCE_COMPARE_OPERATIONS: &[&str] =
    &["high_artifact_reference_compare_prepare"];
const JOB_OPERATIONS: &[&str] = &[
    "job_cancel",
    "job_events_read",
    "job_get",
    "job_result_get",
    "optimization_job_get",
    "optimization_job_prepare",
    "optimization_job_resume",
    "primary_form_repair_job_prepare",
];

// Delivery and approval are intentionally kept as separate façade bindings:
// both belong to the Delivery domain, but approval is the explicit user
// decision surface.  These operation families mirror the checked-in knife
// profile; the rows below add the missing Contract-layer ownership entries
// without changing the profile or the compatibility registry.
// The active MCP profile now consumes package-owned closed request schemas for
// all eleven Delivery operations.  `MappingStatus::Partial` below therefore
// records remaining Runtime/Store/CAS capability gaps, not request-schema debt.
const GAME_ASSET_DELIVERY_OPERATIONS: &[&str] =
    &["game_asset_delivery_get", "game_asset_delivery_prepare"];
const GAME_ASSET_LOD_OPERATIONS: &[&str] = &["game_asset_lod_derive"];
const GAME_WEAPON_GLB_SOCKET_OPERATIONS: &[&str] = &[
    "game_weapon_glb_socket_get",
    "game_weapon_glb_socket_prepare",
];
const EXPORT_PREPARE_OPERATIONS: &[&str] = &["export_prepare"];
const VERSION_DIFF_OPERATIONS: &[&str] = &["version_diff"];
const CANDIDATE_CONFIRM_OPERATIONS: &[&str] = &["candidate_confirm"];
const CANDIDATE_REJECT_OPERATIONS: &[&str] = &["candidate_reject"];
const CROSS_VIEW_PROMOTION_OPERATIONS: &[&str] = &["cross_view_promotion_confirm"];
const EXPORT_CONFIRM_OPERATIONS: &[&str] = &["export_confirm"];

/// The capability mappings currently backed by explicit Runtime/Store
/// modules.  This is deliberately smaller than the compatibility operation
/// registry; unlisted legacy operations do not acquire active knife ownership
/// merely by existing in the old registry.
pub const KNIFE_CAPABILITY_MAPPINGS: &[WeaponryCapabilityMapping] = &[
    WeaponryCapabilityMapping {
        capability: "knife_source_binding",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("KnifeSourceBinding@1"),
        runtime_service: Some("authoring_service::knife_source_binding::{prepare,get}"),
        store_record: Some("KnifeSourceBindingStoreRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("authoring_transaction"),
        mcp_operations: KNIFE_SOURCE_BINDING_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "knife_reference_intent_bundle",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("KnifeReferenceIntentBundle@1"),
        runtime_service: Some("authoring_service::knife_reference_intent_bundle::{prepare,get}"),
        store_record: Some("KnifeReferenceIntentBundleStoreRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("reference_intake"),
        mcp_operations: KNIFE_REFERENCE_INTENT_BUNDLE_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "weaponry_knife_production_brief",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("WeaponryKnifeProductionBrief@1"),
        runtime_service: Some("authoring_service::weaponry_knife_production_brief::{prepare,get}"),
        store_record: Some("WeaponryKnifeProductionBriefStoreRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("reference_intake"),
        mcp_operations: KNIFE_PRODUCTION_BRIEF_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "authoring_mesh_transaction",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("AuthoringMeshTransaction@1"),
        runtime_service: Some("authoring_service::authoring_mesh_transaction_prepare"),
        store_record: Some("AuthoringMeshV2TransactionDurableRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("authoring_transaction"),
        mcp_operations: AUTHORING_TRANSACTION_WRITE_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "authoring_mesh_transaction_readback",
        domain: WeaponryServiceDomain::Evaluation,
        contract: Some("AuthoringMeshTransactionResult@1"),
        runtime_service: Some("evaluation_service::authoring_mesh_transaction_get"),
        store_record: Some("AuthoringMeshV2TransactionDurableRecord"),
        persistence: PersistenceKind::Projection,
        mcp_facade: Some("observe"),
        mcp_operations: AUTHORING_TRANSACTION_READBACK_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "authoring_mesh_v2_candidate_materialization",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("AuthoringMeshV2CandidateMaterializeResult@1"),
        runtime_service: Some("authoring_service::authoring_mesh_v2_candidate_materialize"),
        // This operation intentionally has no second materializer Main row;
        // it atomically reuses the existing candidate/evidence/Job records.
        store_record: Some("CandidateRecord + GeometryCandidateEvidenceRecord + JobRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("authoring_transaction"),
        mcp_operations: AUTHORING_MESH_V2_CANDIDATE_MATERIALIZE_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "authoring_mesh_v2_durable",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("AuthoringMesh@2"),
        runtime_service: Some("authoring_service::authoring_mesh_v2_durable_prepare"),
        store_record: Some("AuthoringMeshV2DurableRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("authoring_transaction"),
        mcp_operations: AUTHORING_V2_DURABLE_WRITE_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "authoring_mesh_v2_source",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("ProductionWeaponAuthoringMeshV2SourcePrepareResult@1"),
        runtime_service: Some("Runtime::production_weapon_authoring_mesh_v2_source_prepare"),
        // The source bridge materializes the Runtime-derived genesis through
        // the existing V2 durable writer; there is no separate producer row.
        store_record: Some("AuthoringMeshV2DurableRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("authoring_transaction"),
        mcp_operations: AUTHORING_MESH_V2_SOURCE_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "authoring_mesh_v2_high_bridge",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("AuthoringMeshV2HighBridge@1"),
        runtime_service: Some("authoring_service::authoring_mesh_v2_high_bridge::{prepare,get}"),
        store_record: Some("AuthoringMeshV2HighBridgeStoreRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("authoring_transaction"),
        mcp_operations: AUTHORING_MESH_V2_HIGH_BRIDGE_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "authoring_mesh_v2_durable_readback",
        domain: WeaponryServiceDomain::Evaluation,
        contract: Some("AuthoringMesh@2"),
        runtime_service: Some("evaluation_service::authoring_mesh_v2_durable_get"),
        store_record: Some("AuthoringMeshV2DurableRecord"),
        persistence: PersistenceKind::Projection,
        mcp_facade: Some("observe"),
        mcp_operations: AUTHORING_V2_DURABLE_READBACK_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "weapon_foundation_import",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("WeaponFoundationImportRecord@1"),
        runtime_service: Some("weapon_foundation_runtime::{prepare,get}"),
        store_record: Some("WeaponFoundationImportRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("reference_intake"),
        mcp_operations: FOUNDATION_IMPORT_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "foundation_authoring_materialization",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("WeaponFoundationAuthoringMaterializationRecord@1"),
        runtime_service: Some("weapon_foundation_authoring_materialization::{prepare,get}"),
        store_record: Some("FoundationAuthoringMeshV2MaterializationRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("authoring_transaction"),
        mcp_operations: FOUNDATION_MATERIALIZATION_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "knife_curve_modifier_graph",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("KnifeCurveModifierGraph@1"),
        runtime_service: Some("knife_curve_modifier_graph::{prepare,get}"),
        store_record: Some("WeaponryCurveModifierGraphDurableRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("authoring_transaction"),
        mcp_operations: CURVE_GRAPH_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "knife_curve_evaluated_mesh",
        domain: WeaponryServiceDomain::Authoring,
        contract: Some("KnifeCurveEvaluatedMesh@1"),
        runtime_service: Some("knife_curve_evaluated_mesh::{prepare,get}"),
        store_record: Some("WeaponryCurveEvaluatedMeshDurableRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("authoring_transaction"),
        mcp_operations: EVALUATED_MESH_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "low_quad_draft",
        domain: WeaponryServiceDomain::Surface,
        contract: Some("LowQuadDraftDurableLink@1"),
        runtime_service: Some("low_quad_durable::{prepare,get}"),
        store_record: Some("LowQuadDraftDurableRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("surface_pipeline"),
        mcp_operations: LOW_QUAD_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "hero_uv",
        domain: WeaponryServiceDomain::Surface,
        contract: Some("HeroUvDurableLink@1"),
        runtime_service: Some("hero_uv_durable::{prepare,get}"),
        store_record: Some("HeroUvDurableRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("surface_pipeline"),
        mcp_operations: HERO_UV_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "knife_uv_bake_v2_aggregate",
        domain: WeaponryServiceDomain::Surface,
        contract: Some("WeaponryKnifeUvBakeV2Aggregate@1"),
        runtime_service: Some("production_knife_uv_bake_v2::{prepare,get}"),
        store_record: Some("WeaponryKnifeUvBakeV2AggregateStoreRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("surface_pipeline"),
        mcp_operations: KNIFE_UV_BAKE_V2_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "formal_high",
        domain: WeaponryServiceDomain::Surface,
        contract: Some("ProductionWeaponHighArtifact@1"),
        runtime_service: Some("production_weapon_formal_high::{prepare,get}"),
        store_record: Some("ProductionWeaponHighArtifactRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("surface_pipeline"),
        mcp_operations: FORMAL_HIGH_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "authoring_mesh_v2_high_artifact",
        domain: WeaponryServiceDomain::Surface,
        contract: Some("AuthoringMeshV2HighArtifact@1"),
        runtime_service: Some(
            "surface_service::authoring_mesh_v2_high_artifact::{prepare,get}",
        ),
        store_record: Some("AuthoringMeshV2HighArtifactStoreRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("surface_pipeline"),
        mcp_operations: AUTHORING_MESH_V2_HIGH_ARTIFACT_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "formal_high_low_cage_bake",
        domain: WeaponryServiceDomain::Surface,
        contract: Some("ProductionWeaponHighLowBakeReceipt@1"),
        runtime_service: Some(
            "surface_service::production_weapon_high_low_bake::{prepare,get,preflight}",
        ),
        store_record: Some("ProductionWeaponHighLowBakeCommitBundle"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("surface_pipeline"),
        mcp_operations: HIGH_LOW_BAKE_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "fps_presentation_package",
        domain: WeaponryServiceDomain::Presentation,
        contract: Some("FpsPresentationPackage@2"),
        runtime_service: Some("fps_presentation_package_v2::{prepare,get}"),
        store_record: Some("FpsPresentationPackageV2StoreRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("fps_presentation"),
        mcp_operations: FPS_PACKAGE_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "fps_presentation_candidate",
        domain: WeaponryServiceDomain::Presentation,
        contract: Some("FpsPresentationPackageV2CandidateBinding@1"),
        runtime_service: Some("fps_presentation_package_v2_candidate::{prepare,get}"),
        store_record: Some("FpsPresentationPackageV2CandidateStoreRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("fps_presentation"),
        mcp_operations: FPS_CANDIDATE_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "fps_presentation_anchor",
        domain: WeaponryServiceDomain::Presentation,
        contract: Some("GameWeaponAnchorSet@1"),
        runtime_service: Some("game_asset_delivery::weapon_anchor::{prepare,get}"),
        store_record: Some("GameWeaponAnchorLinkRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("fps_presentation"),
        mcp_operations: FPS_ANCHOR_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "fps_presentation_animated_socket",
        domain: WeaponryServiceDomain::Presentation,
        contract: Some("GameWeaponAnimatedGlbSocketMaterializationReceipt@1"),
        runtime_service: Some("rigid_animation_glb::weapon_animated_glb_socket::{prepare,get}"),
        store_record: Some("GameWeaponAnimatedGlbSocketMaterializationLinkRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("fps_presentation"),
        mcp_operations: FPS_ANIMATED_SOCKET_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "fps_presentation_socket_transform_projection",
        domain: WeaponryServiceDomain::Presentation,
        contract: Some("GameWeaponAnimatedGlbSocketTransformProjection@1"),
        runtime_service: Some(
            "rigid_animation_glb::game_weapon_animated_glb_socket_transform_projection::{prepare,get}",
        ),
        store_record: Some("GameWeaponAnimatedGlbSocketTransformProjection"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("fps_presentation"),
        mcp_operations: FPS_SOCKET_TRANSFORM_PROJECTION_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "fps_presentation_socket_transform_projection_v2",
        domain: WeaponryServiceDomain::Presentation,
        contract: Some("GameWeaponAnimatedGlbSocketTransformProjection@2"),
        runtime_service: Some(
            "game_weapon_animated_glb_socket_transform_projection_v2::{prepare,get}",
        ),
        store_record: Some("GameWeaponAnimatedGlbSocketTransformProjectionV2"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("fps_presentation"),
        mcp_operations: FPS_SOCKET_TRANSFORM_PROJECTION_V2_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "fps_presentation_animation_clip",
        domain: WeaponryServiceDomain::Presentation,
        contract: Some("MechanicalAnimationClip@1"),
        runtime_service: Some("mechanical_pose::animation_clip::{prepare,get,preview_get}"),
        store_record: Some("MechanicalAnimationClipLinkRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("fps_presentation"),
        mcp_operations: FPS_ANIMATION_CLIP_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "fps_presentation_animation_clip_v2",
        domain: WeaponryServiceDomain::Presentation,
        contract: Some("MechanicalAnimationClip@2"),
        runtime_service: Some("mechanical_animation_clip_v2::{prepare,get,preview}"),
        store_record: Some("MechanicalAnimationClipV2LinkRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("fps_presentation"),
        mcp_operations: FPS_ANIMATION_CLIP_V2_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "fps_presentation_animation_glb_v2",
        domain: WeaponryServiceDomain::Presentation,
        contract: Some("MechanicalAnimationGlbReceipt@2"),
        runtime_service: Some("mechanical_animation_glb_v2::{prepare,get}"),
        store_record: Some("MechanicalAnimationGlbV2LinkRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("fps_presentation"),
        mcp_operations: FPS_ANIMATION_GLB_V2_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "game_asset_delivery",
        domain: WeaponryServiceDomain::Delivery,
        contract: Some("GameAssetDeliveryLink@1"),
        runtime_service: Some("game_asset_delivery::{prepare,get}"),
        store_record: Some("GameAssetDeliveryLinkRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("delivery"),
        mcp_operations: GAME_ASSET_DELIVERY_OPERATIONS,
        // Runtime and Store still use the shared compatibility implementation;
        // this is an ownership seam, not a claim that GameDeliveryRepository
        // extraction or a direct Delivery service is complete.
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "game_asset_lod",
        domain: WeaponryServiceDomain::Delivery,
        contract: Some("GameAssetLodDeriveResult@1"),
        runtime_service: Some("game_asset_delivery::derive_lods"),
        // LOD derive is a bounded, zero-write preview.  It reads immutable
        // candidate/evidence/CAS inputs and returns a transient result; do not
        // invent a durable Store record for this operation.
        store_record: None,
        persistence: PersistenceKind::None,
        mcp_facade: Some("delivery"),
        mcp_operations: GAME_ASSET_LOD_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "game_weapon_glb_socket",
        domain: WeaponryServiceDomain::Delivery,
        contract: Some("GameWeaponGlbSocketMaterializationLink@1"),
        runtime_service: Some("game_asset_delivery::weapon_glb_socket::{prepare,get}"),
        // The parent Link owns the associated per-LOD child rows and receipt.
        store_record: Some("GameWeaponGlbSocketMaterializationLinkRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("delivery"),
        mcp_operations: GAME_WEAPON_GLB_SOCKET_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "export_prepare",
        domain: WeaponryServiceDomain::Delivery,
        contract: Some("ExportManifest@1"),
        runtime_service: Some("Runtime::prepare_export"),
        store_record: Some("ExportManifestRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("delivery"),
        mcp_operations: EXPORT_PREPARE_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "version_diff",
        domain: WeaponryServiceDomain::Delivery,
        contract: Some("VersionDiff@1"),
        runtime_service: Some("Runtime::version_diff"),
        // Runtime emits VersionDiff@1, but no package result schema or row is
        // currently checked in.  The result is recomputed from immutable
        // version/candidate/CAS inputs.
        store_record: None,
        persistence: PersistenceKind::None,
        mcp_facade: Some("approval"),
        mcp_operations: VERSION_DIFF_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "candidate_confirm",
        domain: WeaponryServiceDomain::Delivery,
        contract: Some("CandidateConfirmResult@1"),
        runtime_service: Some("Runtime::confirm_candidate"),
        store_record: Some(
            "CandidateRecord + DesignAssetVersionRecord + SnapshotRecord + ApprovalReceiptRecord",
        ),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("approval"),
        mcp_operations: CANDIDATE_CONFIRM_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "candidate_reject",
        domain: WeaponryServiceDomain::Delivery,
        contract: Some("CandidateRejectResult@1"),
        runtime_service: Some("Runtime::reject_candidate"),
        store_record: Some("CandidateRecord + ApprovalReceiptRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("approval"),
        mcp_operations: CANDIDATE_REJECT_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "cross_view_promotion",
        domain: WeaponryServiceDomain::Delivery,
        contract: Some("CrossViewPromotionResult@1"),
        runtime_service: Some("Runtime::cross_view_promotion_confirm"),
        // CrossViewEvidenceRecord is an Evaluation-owned read dependency;
        // it is deliberately not claimed as Delivery persistence here.
        store_record: Some(
            "CandidateRecord + DesignAssetVersionRecord + SnapshotRecord + ApprovalReceiptRecord",
        ),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("approval"),
        mcp_operations: CROSS_VIEW_PROMOTION_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "export_confirm",
        domain: WeaponryServiceDomain::Delivery,
        contract: Some("ExportConfirmResult@1"),
        runtime_service: Some("Runtime::confirm_export"),
        store_record: Some("ExportManifestRecord + ApprovalReceiptRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("approval"),
        mcp_operations: EXPORT_CONFIRM_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "observe_read_model",
        domain: WeaponryServiceDomain::Evaluation,
        contract: None,
        runtime_service: Some("observe::{read_model}"),
        store_record: Some("EvaluationReadModelProjection"),
        persistence: PersistenceKind::Projection,
        mcp_facade: Some("observe"),
        mcp_operations: OBSERVE_OPERATIONS,
        status: MappingStatus::Partial,
    },
    WeaponryCapabilityMapping {
        capability: "silhouette_evaluation",
        domain: WeaponryServiceDomain::Evaluation,
        contract: Some("SilhouetteEvaluationObjective@1"),
        runtime_service: Some("silhouette::{objective,fit}"),
        store_record: None,
        persistence: PersistenceKind::None,
        mcp_facade: Some("quality_review"),
        mcp_operations: SILHOUETTE_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "knife_pass_state",
        domain: WeaponryServiceDomain::Evaluation,
        contract: Some("KnifePassState@1"),
        runtime_service: Some("evaluation_service::knife_pass_state::{prepare,get}"),
        store_record: Some("KnifePassStateStoreRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("quality_review"),
        mcp_operations: KNIFE_PASS_STATE_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "high_artifact_reference_comparison",
        domain: WeaponryServiceDomain::Evaluation,
        contract: Some("HighArtifactReferenceComparison@1"),
        runtime_service: Some("evaluation_service::high_artifact_reference_comparison"),
        store_record: Some("CAS-only HighArtifactRenderSet + HighArtifactReferenceComparison"),
        persistence: PersistenceKind::Projection,
        mcp_facade: Some("quality_review"),
        mcp_operations: HIGH_ARTIFACT_REFERENCE_COMPARE_OPERATIONS,
        status: MappingStatus::Complete,
    },
    WeaponryCapabilityMapping {
        capability: "runtime_job_lifecycle",
        domain: WeaponryServiceDomain::Evaluation,
        contract: Some("RuntimeJob@1"),
        runtime_service: Some("evaluation_service::runtime_job_lifecycle"),
        store_record: Some("JobRecord"),
        persistence: PersistenceKind::DurableTransaction,
        mcp_facade: Some("job"),
        mcp_operations: JOB_OPERATIONS,
        status: MappingStatus::Partial,
    },
];

pub fn capability_mapping_for(capability: &str) -> Option<&'static WeaponryCapabilityMapping> {
    KNIFE_CAPABILITY_MAPPINGS
        .iter()
        .find(|mapping| mapping.capability == capability)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Exact `fps_presentation` operation inventory copied from the closed
    /// Knife profile for a source-level map audit.  This remains test-only so
    /// the Contract map does not become a second runtime/MCP registry.
    const FPS_PRESENTATION_READ_OPERATIONS: &[&str] = &[
        "fps_presentation_package_v2_candidate_get",
        "fps_presentation_package_v2_get",
        "fps_presentation_package_v2_production_preflight_get",
        "game_weapon_anchor_get",
        "game_weapon_animated_glb_socket_get",
        "game_weapon_animated_glb_socket_transform_projection_get",
        "game_weapon_animated_glb_socket_transform_projection_v2_get",
        "mechanical_animation_clip_get",
        "mechanical_animation_clip_preview_get",
        "mechanical_animation_clip_v2_get",
        "mechanical_animation_clip_v2_preview",
        "mechanical_animation_glb_v2_get",
    ];
    const FPS_PRESENTATION_WRITE_OPERATIONS: &[&str] = &[
        "fps_presentation_package_v2_candidate_prepare",
        "fps_presentation_package_v2_prepare",
        "game_weapon_anchor_prepare",
        "game_weapon_animated_glb_socket_prepare",
        "game_weapon_animated_glb_socket_transform_projection_prepare",
        "game_weapon_animated_glb_socket_transform_projection_v2_prepare",
        "mechanical_animation_clip_prepare",
        "mechanical_animation_clip_v2_prepare",
        "mechanical_animation_glb_v2_prepare",
    ];

    /// Exact Delivery/Approval operation inventory copied from the closed
    /// Knife profile for this source-level mapping audit.
    const DELIVERY_PROFILE_READ_OPERATIONS: &[&str] = &[
        "game_asset_delivery_get",
        "game_asset_lod_derive",
        "game_weapon_glb_socket_get",
    ];
    const DELIVERY_PROFILE_WRITE_OPERATIONS: &[&str] = &[
        "export_prepare",
        "game_asset_delivery_prepare",
        "game_weapon_glb_socket_prepare",
    ];
    const APPROVAL_PROFILE_READ_OPERATIONS: &[&str] = &["version_diff"];
    const APPROVAL_PROFILE_WRITE_OPERATIONS: &[&str] = &[
        "candidate_confirm",
        "candidate_reject",
        "cross_view_promotion_confirm",
        "export_confirm",
    ];

    #[test]
    fn knife_facades_have_one_owner_in_five_domains() {
        assert_eq!(KNIFE_FACADE_BINDINGS.len(), 11);
        let names = KNIFE_FACADE_BINDINGS
            .iter()
            .map(|binding| binding.facade_name)
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 11);
        for domain in WeaponryServiceDomain::all() {
            for facade in knife_facades_for_domain(domain) {
                let binding = knife_facade_binding(facade).expect("facade binding");
                assert_eq!(binding.domain, domain);
            }
        }
    }

    #[test]
    fn persistence_kind_never_claims_record_for_none() {
        for mapping in KNIFE_CAPABILITY_MAPPINGS {
            if mapping.persistence == PersistenceKind::None {
                assert!(mapping.store_record.is_none());
            } else {
                assert!(mapping.store_record.is_some());
            }
        }
        assert_eq!(
            capability_mapping_for("silhouette_evaluation")
                .expect("silhouette")
                .persistence,
            PersistenceKind::None
        );
        assert_eq!(
            capability_mapping_for("observe_read_model")
                .expect("observe")
                .persistence,
            PersistenceKind::Projection
        );
    }

    #[test]
    fn capability_mappings_resolve_to_their_single_facade_domain() {
        let mut capabilities = BTreeSet::new();
        let mut operations = BTreeSet::new();
        for mapping in KNIFE_CAPABILITY_MAPPINGS {
            assert!(capabilities.insert(mapping.capability));
            assert!(!mapping.mcp_operations.is_empty());
            for operation in mapping.mcp_operations {
                assert!(
                    operations.insert(*operation),
                    "operation {operation} has more than one capability owner"
                );
            }
            if let Some(facade) = mapping.mcp_facade {
                assert_eq!(
                    knife_facade_binding(facade)
                        .expect("mapped capability façade")
                        .domain,
                    mapping.domain
                );
            }
            if mapping.persistence == PersistenceKind::None {
                assert!(mapping.store_record.is_none());
            }
        }
    }

    #[test]
    fn authoring_writes_and_observe_readbacks_have_distinct_domain_owners() {
        let transaction_write = capability_mapping_for("authoring_mesh_transaction")
            .expect("transaction write mapping");
        assert_eq!(transaction_write.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(transaction_write.mcp_facade, Some("authoring_transaction"));
        assert_eq!(
            transaction_write.mcp_operations,
            &["authoring_mesh_transaction_prepare"]
        );

        let transaction_read = capability_mapping_for("authoring_mesh_transaction_readback")
            .expect("transaction readback mapping");
        assert_eq!(transaction_read.domain, WeaponryServiceDomain::Evaluation);
        assert_eq!(transaction_read.mcp_facade, Some("observe"));
        assert_eq!(
            transaction_read.mcp_operations,
            &["authoring_mesh_transaction_get"]
        );
        assert_eq!(transaction_read.persistence, PersistenceKind::Projection);

        let mesh_write =
            capability_mapping_for("authoring_mesh_v2_durable").expect("mesh write mapping");
        assert_eq!(mesh_write.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(mesh_write.mcp_facade, Some("authoring_transaction"));
        assert_eq!(
            mesh_write.mcp_operations,
            &["authoring_mesh_v2_durable_prepare"]
        );

        let mesh_read = capability_mapping_for("authoring_mesh_v2_durable_readback")
            .expect("mesh readback mapping");
        assert_eq!(mesh_read.domain, WeaponryServiceDomain::Evaluation);
        assert_eq!(mesh_read.mcp_facade, Some("observe"));
        assert_eq!(mesh_read.mcp_operations, &["authoring_mesh_v2_durable_get"]);
        assert_eq!(mesh_read.persistence, PersistenceKind::Projection);

        let materialization = capability_mapping_for("authoring_mesh_v2_candidate_materialization")
            .expect("AuthoringMeshV2 candidate materialization mapping");
        assert_eq!(materialization.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(
            materialization.contract,
            Some("AuthoringMeshV2CandidateMaterializeResult@1")
        );
        assert_eq!(
            materialization.runtime_service,
            Some("authoring_service::authoring_mesh_v2_candidate_materialize")
        );
        assert_eq!(
            materialization.store_record,
            Some("CandidateRecord + GeometryCandidateEvidenceRecord + JobRecord")
        );
        assert_eq!(
            materialization.mcp_operations,
            AUTHORING_MESH_V2_CANDIDATE_MATERIALIZE_OPERATIONS
        );
        assert_eq!(materialization.status, MappingStatus::Partial);
    }

    #[test]
    fn authoring_mesh_v2_source_uses_the_existing_durable_record_seam() {
        let source = capability_mapping_for("authoring_mesh_v2_source")
            .expect("AuthoringMeshV2 source mapping");
        assert_eq!(source.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(
            source.contract,
            Some("ProductionWeaponAuthoringMeshV2SourcePrepareResult@1")
        );
        assert_eq!(
            source.runtime_service,
            Some("Runtime::production_weapon_authoring_mesh_v2_source_prepare")
        );
        assert_eq!(source.store_record, Some("AuthoringMeshV2DurableRecord"));
        assert_eq!(source.persistence, PersistenceKind::DurableTransaction);
        assert_eq!(source.mcp_facade, Some("authoring_transaction"));
        assert_eq!(source.mcp_operations, AUTHORING_MESH_V2_SOURCE_OPERATIONS);
        assert_eq!(source.status, MappingStatus::Complete);
    }

    #[test]
    fn authoring_mesh_v2_high_bridge_uses_the_authoring_transaction_seam() {
        let bridge = capability_mapping_for("authoring_mesh_v2_high_bridge")
            .expect("AuthoringMeshV2 High bridge mapping");
        assert_eq!(bridge.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(bridge.contract, Some("AuthoringMeshV2HighBridge@1"));
        assert_eq!(
            bridge.runtime_service,
            Some("authoring_service::authoring_mesh_v2_high_bridge::{prepare,get}")
        );
        assert_eq!(
            bridge.store_record,
            Some("AuthoringMeshV2HighBridgeStoreRecord")
        );
        assert_eq!(bridge.persistence, PersistenceKind::DurableTransaction);
        assert_eq!(bridge.mcp_facade, Some("authoring_transaction"));
        assert_eq!(
            bridge.mcp_operations,
            &[
                "authoring_mesh_v2_high_bridge_get",
                "authoring_mesh_v2_high_bridge_prepare"
            ]
        );
        assert_eq!(bridge.status, MappingStatus::Complete);
    }

    #[test]
    fn authoring_mesh_v2_high_artifact_uses_the_surface_pipeline_seam() {
        let artifact = capability_mapping_for("authoring_mesh_v2_high_artifact")
            .expect("AuthoringMeshV2 High artifact mapping");
        assert_eq!(artifact.domain, WeaponryServiceDomain::Surface);
        assert_eq!(artifact.contract, Some("AuthoringMeshV2HighArtifact@1"));
        assert_eq!(
            artifact.runtime_service,
            Some("surface_service::authoring_mesh_v2_high_artifact::{prepare,get}")
        );
        assert_eq!(
            artifact.store_record,
            Some("AuthoringMeshV2HighArtifactStoreRecord")
        );
        assert_eq!(artifact.persistence, PersistenceKind::DurableTransaction);
        assert_eq!(artifact.mcp_facade, Some("surface_pipeline"));
        assert_eq!(
            artifact.mcp_operations,
            AUTHORING_MESH_V2_HIGH_ARTIFACT_OPERATIONS
        );
        assert_eq!(artifact.status, MappingStatus::Complete);
    }

    #[test]
    fn formal_high_low_bake_has_one_complete_surface_ownership_seam() {
        let mapping = capability_mapping_for("formal_high_low_cage_bake")
            .expect("formal High/Low/Cage/Bake mapping");
        assert_eq!(mapping.domain, WeaponryServiceDomain::Surface);
        assert_eq!(
            mapping.contract,
            Some("ProductionWeaponHighLowBakeReceipt@1")
        );
        assert_eq!(
            mapping.store_record,
            Some("ProductionWeaponHighLowBakeCommitBundle")
        );
        assert_eq!(mapping.mcp_facade, Some("surface_pipeline"));
        assert_eq!(
            mapping.mcp_operations,
            &[
                "production_weapon_high_low_bake_get",
                "production_weapon_high_low_bake_preflight_get",
                "production_weapon_high_low_bake_prepare",
            ]
        );
    }

    #[test]
    fn fps_presentation_profile_has_exactly_one_presentation_owner_per_operation() {
        assert_eq!(FPS_PRESENTATION_READ_OPERATIONS.len(), 12);
        assert_eq!(FPS_PRESENTATION_WRITE_OPERATIONS.len(), 9);

        let profile_operations = FPS_PRESENTATION_READ_OPERATIONS
            .iter()
            .chain(FPS_PRESENTATION_WRITE_OPERATIONS.iter())
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(profile_operations.len(), 21);

        let presentation_mappings = KNIFE_CAPABILITY_MAPPINGS
            .iter()
            .filter(|mapping| mapping.domain == WeaponryServiceDomain::Presentation)
            .collect::<Vec<_>>();
        assert_eq!(presentation_mappings.len(), 9);

        let mut mapped_operations = BTreeSet::new();
        for mapping in presentation_mappings {
            assert_eq!(mapping.mcp_facade, Some("fps_presentation"));
            assert_eq!(mapping.persistence, PersistenceKind::DurableTransaction);
            assert!(mapping.contract.is_some());
            assert!(mapping.runtime_service.is_some());
            assert!(mapping.store_record.is_some());
            for operation in mapping.mcp_operations {
                assert!(
                    profile_operations.contains(operation),
                    "unlisted fps_presentation operation: {operation}"
                );
                assert!(
                    mapped_operations.insert(*operation),
                    "operation {operation} has more than one Presentation capability owner"
                );
            }
        }
        assert_eq!(mapped_operations, profile_operations);
    }

    #[test]
    fn fps_presentation_capability_layers_match_the_typed_families() {
        let expected = [
            (
                "fps_presentation_package",
                "FpsPresentationPackage@2",
                "fps_presentation_package_v2::{prepare,get}",
                "FpsPresentationPackageV2StoreRecord",
                FPS_PACKAGE_OPERATIONS,
            ),
            (
                "fps_presentation_candidate",
                "FpsPresentationPackageV2CandidateBinding@1",
                "fps_presentation_package_v2_candidate::{prepare,get}",
                "FpsPresentationPackageV2CandidateStoreRecord",
                FPS_CANDIDATE_OPERATIONS,
            ),
            (
                "fps_presentation_anchor",
                "GameWeaponAnchorSet@1",
                "game_asset_delivery::weapon_anchor::{prepare,get}",
                "GameWeaponAnchorLinkRecord",
                FPS_ANCHOR_OPERATIONS,
            ),
            (
                "fps_presentation_animated_socket",
                "GameWeaponAnimatedGlbSocketMaterializationReceipt@1",
                "rigid_animation_glb::weapon_animated_glb_socket::{prepare,get}",
                "GameWeaponAnimatedGlbSocketMaterializationLinkRecord",
                FPS_ANIMATED_SOCKET_OPERATIONS,
            ),
            (
                "fps_presentation_socket_transform_projection",
                "GameWeaponAnimatedGlbSocketTransformProjection@1",
                "rigid_animation_glb::game_weapon_animated_glb_socket_transform_projection::{prepare,get}",
                "GameWeaponAnimatedGlbSocketTransformProjection",
                FPS_SOCKET_TRANSFORM_PROJECTION_OPERATIONS,
            ),
            (
                "fps_presentation_socket_transform_projection_v2",
                "GameWeaponAnimatedGlbSocketTransformProjection@2",
                "game_weapon_animated_glb_socket_transform_projection_v2::{prepare,get}",
                "GameWeaponAnimatedGlbSocketTransformProjectionV2",
                FPS_SOCKET_TRANSFORM_PROJECTION_V2_OPERATIONS,
            ),
            (
                "fps_presentation_animation_clip",
                "MechanicalAnimationClip@1",
                "mechanical_pose::animation_clip::{prepare,get,preview_get}",
                "MechanicalAnimationClipLinkRecord",
                FPS_ANIMATION_CLIP_OPERATIONS,
            ),
            (
                "fps_presentation_animation_clip_v2",
                "MechanicalAnimationClip@2",
                "mechanical_animation_clip_v2::{prepare,get,preview}",
                "MechanicalAnimationClipV2LinkRecord",
                FPS_ANIMATION_CLIP_V2_OPERATIONS,
            ),
            (
                "fps_presentation_animation_glb_v2",
                "MechanicalAnimationGlbReceipt@2",
                "mechanical_animation_glb_v2::{prepare,get}",
                "MechanicalAnimationGlbV2LinkRecord",
                FPS_ANIMATION_GLB_V2_OPERATIONS,
            ),
        ];

        for (capability, contract, runtime_service, store_record, operations) in expected {
            let mapping = capability_mapping_for(capability).expect("Presentation capability");
            assert_eq!(mapping.domain, WeaponryServiceDomain::Presentation);
            assert_eq!(mapping.contract, Some(contract));
            assert_eq!(mapping.runtime_service, Some(runtime_service));
            assert_eq!(mapping.store_record, Some(store_record));
            assert_eq!(mapping.mcp_facade, Some("fps_presentation"));
            assert_eq!(mapping.mcp_operations, operations);
        }
    }

    #[test]
    fn delivery_and_approval_profile_inventory_has_one_owner_per_operation() {
        assert_eq!(DELIVERY_PROFILE_READ_OPERATIONS.len(), 3);
        assert_eq!(DELIVERY_PROFILE_WRITE_OPERATIONS.len(), 3);
        assert_eq!(APPROVAL_PROFILE_READ_OPERATIONS.len(), 1);
        assert_eq!(APPROVAL_PROFILE_WRITE_OPERATIONS.len(), 4);

        let delivery_operations = DELIVERY_PROFILE_READ_OPERATIONS
            .iter()
            .chain(DELIVERY_PROFILE_WRITE_OPERATIONS.iter())
            .copied()
            .collect::<BTreeSet<_>>();
        let approval_operations = APPROVAL_PROFILE_READ_OPERATIONS
            .iter()
            .chain(APPROVAL_PROFILE_WRITE_OPERATIONS.iter())
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(delivery_operations.len(), 6);
        assert_eq!(approval_operations.len(), 5);
        assert!(delivery_operations.is_disjoint(&approval_operations));

        let delivery_mappings = KNIFE_CAPABILITY_MAPPINGS
            .iter()
            .filter(|mapping| mapping.domain == WeaponryServiceDomain::Delivery)
            .filter(|mapping| mapping.mcp_facade == Some("delivery"))
            .collect::<Vec<_>>();
        let approval_mappings = KNIFE_CAPABILITY_MAPPINGS
            .iter()
            .filter(|mapping| mapping.domain == WeaponryServiceDomain::Delivery)
            .filter(|mapping| mapping.mcp_facade == Some("approval"))
            .collect::<Vec<_>>();
        assert_eq!(delivery_mappings.len(), 4);
        assert_eq!(approval_mappings.len(), 5);

        let mapped_delivery_operations = delivery_mappings
            .iter()
            .flat_map(|mapping| mapping.mcp_operations.iter().copied())
            .collect::<BTreeSet<_>>();
        let mapped_approval_operations = approval_mappings
            .iter()
            .flat_map(|mapping| mapping.mcp_operations.iter().copied())
            .collect::<BTreeSet<_>>();
        assert_eq!(mapped_delivery_operations, delivery_operations);
        assert_eq!(mapped_approval_operations, approval_operations);
        for mapping in delivery_mappings
            .into_iter()
            .chain(approval_mappings.into_iter())
        {
            assert_eq!(mapping.status, MappingStatus::Partial);
        }
    }

    #[test]
    fn delivery_and_approval_layers_match_current_runtime_and_store_seams() {
        let expected = [
            (
                "game_asset_delivery",
                "GameAssetDeliveryLink@1",
                "game_asset_delivery::{prepare,get}",
                Some("GameAssetDeliveryLinkRecord"),
                PersistenceKind::DurableTransaction,
                "delivery",
                GAME_ASSET_DELIVERY_OPERATIONS,
            ),
            (
                "game_asset_lod",
                "GameAssetLodDeriveResult@1",
                "game_asset_delivery::derive_lods",
                None,
                PersistenceKind::None,
                "delivery",
                GAME_ASSET_LOD_OPERATIONS,
            ),
            (
                "game_weapon_glb_socket",
                "GameWeaponGlbSocketMaterializationLink@1",
                "game_asset_delivery::weapon_glb_socket::{prepare,get}",
                Some("GameWeaponGlbSocketMaterializationLinkRecord"),
                PersistenceKind::DurableTransaction,
                "delivery",
                GAME_WEAPON_GLB_SOCKET_OPERATIONS,
            ),
            (
                "export_prepare",
                "ExportManifest@1",
                "Runtime::prepare_export",
                Some("ExportManifestRecord"),
                PersistenceKind::DurableTransaction,
                "delivery",
                EXPORT_PREPARE_OPERATIONS,
            ),
            (
                "version_diff",
                "VersionDiff@1",
                "Runtime::version_diff",
                None,
                PersistenceKind::None,
                "approval",
                VERSION_DIFF_OPERATIONS,
            ),
            (
                "candidate_confirm",
                "CandidateConfirmResult@1",
                "Runtime::confirm_candidate",
                Some(
                    "CandidateRecord + DesignAssetVersionRecord + SnapshotRecord + ApprovalReceiptRecord",
                ),
                PersistenceKind::DurableTransaction,
                "approval",
                CANDIDATE_CONFIRM_OPERATIONS,
            ),
            (
                "candidate_reject",
                "CandidateRejectResult@1",
                "Runtime::reject_candidate",
                Some("CandidateRecord + ApprovalReceiptRecord"),
                PersistenceKind::DurableTransaction,
                "approval",
                CANDIDATE_REJECT_OPERATIONS,
            ),
            (
                "cross_view_promotion",
                "CrossViewPromotionResult@1",
                "Runtime::cross_view_promotion_confirm",
                Some(
                    "CandidateRecord + DesignAssetVersionRecord + SnapshotRecord + ApprovalReceiptRecord",
                ),
                PersistenceKind::DurableTransaction,
                "approval",
                CROSS_VIEW_PROMOTION_OPERATIONS,
            ),
            (
                "export_confirm",
                "ExportConfirmResult@1",
                "Runtime::confirm_export",
                Some("ExportManifestRecord + ApprovalReceiptRecord"),
                PersistenceKind::DurableTransaction,
                "approval",
                EXPORT_CONFIRM_OPERATIONS,
            ),
        ];

        for (
            capability,
            contract,
            runtime_service,
            store_record,
            persistence,
            facade,
            operations,
        ) in expected
        {
            let mapping = capability_mapping_for(capability).expect("Delivery capability");
            assert_eq!(mapping.domain, WeaponryServiceDomain::Delivery);
            assert_eq!(mapping.contract, Some(contract));
            assert_eq!(mapping.runtime_service, Some(runtime_service));
            assert_eq!(mapping.store_record, store_record);
            assert_eq!(mapping.persistence, persistence);
            assert_eq!(mapping.mcp_facade, Some(facade));
            assert_eq!(mapping.mcp_operations, operations);
            assert_eq!(mapping.status, MappingStatus::Partial);
        }
    }

    #[test]
    fn execution_target_has_two_explicit_mcp_control_plane_exceptions() {
        assert_eq!(MCP_ADAPTER_OPERATIONS, &["doctor", "runtime_status"]);
        assert_eq!(
            knife_operation_execution_target("doctor"),
            WeaponryOperationExecutionTarget::McpAdapter
        );
        assert_eq!(
            knife_operation_execution_target("runtime_status"),
            WeaponryOperationExecutionTarget::McpAdapter
        );
        assert_eq!(
            knife_operation_execution_target("authoring_mesh_transaction_prepare"),
            WeaponryOperationExecutionTarget::Runtime
        );
        assert_eq!(
            knife_facade_binding("weapon_preflight")
                .expect("weapon preflight façade")
                .domain,
            WeaponryServiceDomain::Authoring
        );
    }

    #[test]
    fn knife_production_brief_has_one_complete_reference_intake_seam() {
        let mapping = capability_mapping_for("weaponry_knife_production_brief")
            .expect("knife production brief mapping");
        assert_eq!(mapping.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(mapping.contract, Some("WeaponryKnifeProductionBrief@1"));
        assert_eq!(
            mapping.runtime_service,
            Some("authoring_service::weaponry_knife_production_brief::{prepare,get}")
        );
        assert_eq!(
            mapping.store_record,
            Some("WeaponryKnifeProductionBriefStoreRecord")
        );
        assert_eq!(mapping.persistence, PersistenceKind::DurableTransaction);
        assert_eq!(mapping.mcp_facade, Some("reference_intake"));
        assert_eq!(mapping.mcp_operations, KNIFE_PRODUCTION_BRIEF_OPERATIONS);
        assert_eq!(mapping.status, MappingStatus::Complete);
    }

    #[test]
    fn knife_reference_intent_bundle_has_one_reference_intake_seam() {
        let mapping = capability_mapping_for("knife_reference_intent_bundle")
            .expect("knife reference intent bundle mapping");
        assert_eq!(mapping.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(mapping.contract, Some("KnifeReferenceIntentBundle@1"));
        assert_eq!(
            mapping.runtime_service,
            Some("authoring_service::knife_reference_intent_bundle::{prepare,get}")
        );
        assert_eq!(
            mapping.store_record,
            Some("KnifeReferenceIntentBundleStoreRecord")
        );
        assert_eq!(mapping.persistence, PersistenceKind::DurableTransaction);
        assert_eq!(mapping.mcp_facade, Some("reference_intake"));
        assert_eq!(
            mapping.mcp_operations,
            KNIFE_REFERENCE_INTENT_BUNDLE_OPERATIONS
        );
        assert_eq!(mapping.status, MappingStatus::Partial);
    }

    #[test]
    fn knife_source_binding_has_one_complete_authoring_seam() {
        let mapping =
            capability_mapping_for("knife_source_binding").expect("knife source binding mapping");
        assert_eq!(mapping.domain, WeaponryServiceDomain::Authoring);
        assert_eq!(mapping.contract, Some("KnifeSourceBinding@1"));
        assert_eq!(
            mapping.runtime_service,
            Some("authoring_service::knife_source_binding::{prepare,get}")
        );
        assert_eq!(mapping.store_record, Some("KnifeSourceBindingStoreRecord"));
        assert_eq!(mapping.persistence, PersistenceKind::DurableTransaction);
        assert_eq!(mapping.mcp_facade, Some("authoring_transaction"));
        assert_eq!(mapping.mcp_operations, KNIFE_SOURCE_BINDING_OPERATIONS);
        assert_eq!(mapping.status, MappingStatus::Complete);
    }

    #[test]
    fn silhouette_evaluation_is_owned_by_quality_review() {
        let mapping =
            capability_mapping_for("silhouette_evaluation").expect("silhouette evaluation mapping");
        assert_eq!(mapping.domain, WeaponryServiceDomain::Evaluation);
        assert_eq!(mapping.mcp_facade, Some("quality_review"));
        assert_eq!(
            mapping.mcp_operations,
            &[
                "silhouette_evaluation_objective_prepare",
                "silhouette_fit_prepare",
            ]
        );
    }

    #[test]
    fn runtime_job_lifecycle_has_one_evaluation_aggregate_seam() {
        let mapping =
            capability_mapping_for("runtime_job_lifecycle").expect("runtime Job lifecycle mapping");
        assert_eq!(mapping.domain, WeaponryServiceDomain::Evaluation);
        assert_eq!(mapping.contract, Some("RuntimeJob@1"));
        assert_eq!(mapping.store_record, Some("JobRecord"));
        assert_eq!(mapping.mcp_facade, Some("job"));
        assert_eq!(mapping.persistence, PersistenceKind::DurableTransaction);
        assert_eq!(mapping.mcp_operations, JOB_OPERATIONS);
        assert_eq!(mapping.status, MappingStatus::Partial);
    }
}
