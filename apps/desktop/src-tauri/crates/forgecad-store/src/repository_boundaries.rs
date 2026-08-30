//! First-phase ownership directory for the Store repositories.
//!
//! This is an inventory and routing boundary, not a new wire contract.  It
//! intentionally contains no SQL, schema or manifest data.  The existing
//! `Store` implementation remains the compatibility façade while the
//! inventory gives the next extraction pass a stable home for each record and
//! CAS-root policy.
//!
//! Migration ownership is deliberately singular: every domain continues to
//! use `Store::migrate` and the same `0001_runtime.sql` source.  Domain entries
//! describe ownership only; they must not be interpreted as permission for a
//! domain to run a second migration sequence or to collect CAS roots outside
//! the existing transaction/reachability rules.

pub use forgecad_contracts::weaponry_domain_map::{
    MappingStatus as StoreMappingStatus, PersistenceKind,
    WeaponryCapabilityMapping as StoreCapabilityMapping,
    WeaponryServiceDomain as StoreRepositoryDomain, KNIFE_CAPABILITY_MAPPINGS,
};

/// Version of the logical Store ownership directory.
pub const STORE_REPOSITORY_DIRECTORY_SCHEMA_VERSION: &str = "StoreRepositoryDirectory@1";

/// The only migration source currently owned by the Runtime Store.
pub const STORE_MIGRATION_SOURCE: &str = "migrations-runtime-v1/0001_runtime.sql";

/// The only migration entry point.  Additive domain tables remain bootstrapped
/// by the existing Store transaction after this script; this constant records
/// the ownership rule without changing that order.
pub const STORE_MIGRATION_OWNER: &str = "forgecad-store::Store::migrate";

/// Migration order is intentionally a one-item sequence until a planned
/// versioned migration is reviewed and landed.
pub const STORE_MIGRATION_SEQUENCE: &[&str] = &[STORE_MIGRATION_SOURCE];

/// Snapshot of the current knife profile used by this ownership audit.  The
/// profile has eleven public façades, 125 distinct underlying operations and
/// 125 façade operation occurrences; every operation has exactly one façade
/// owner.  These values are audit facts, not a second MCP manifest.
pub const KNIFE_PROFILE_FACADE_COUNT: usize = 11;
pub const KNIFE_PROFILE_UNIQUE_OPERATION_COUNT: usize = 125;
pub const KNIFE_PROFILE_OPERATION_OCCURRENCE_COUNT: usize = 125;
pub const KNIFE_PROFILE_CROSS_FACADE_OPERATION_NAME_COUNT: usize = 0;
pub const KNIFE_PROFILE_DUPLICATE_OWNER_COUNT: usize = 0;

/// One logical repository boundary.  `implementation_modules` records the
/// current source locations and therefore makes remaining `lib.rs`
/// concentration visible instead of pretending extraction is complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreRepositoryBoundary {
    pub domain: StoreRepositoryDomain,
    pub logical_module: &'static str,
    pub implementation_modules: &'static [&'static str],
    pub record_types: &'static [&'static str],
    pub table_names: &'static [&'static str],
    pub migration_owner: &'static str,
    pub migration_source: &'static str,
    pub gc_root_policy: &'static str,
    pub extraction_status: &'static str,
}

/// Ownership of one of the eleven default knife façades.  Multiple façades
/// may call one repository, but no façade is allowed to create a second Store
/// writer for the same record family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreFacadeOwnership {
    pub facade: &'static str,
    pub logical_module: &'static str,
    pub underlying_operation_count: usize,
    /// This is an ownership-seam label only; it does not claim physical
    /// extraction from `Store`/`lib.rs`.
    pub ownership_status: &'static str,
}

impl StoreFacadeOwnership {
    /// Resolve the façade's domain from the central Contract map.
    pub fn domain(&self) -> StoreRepositoryDomain {
        forgecad_contracts::weaponry_domain_map::knife_facade_binding(self.facade)
            .expect("every Store façade must have a Contract binding")
            .domain
    }
}

/// First-phase Store ownership of the default knife profile.  The operation
/// counts are a checked-in audit snapshot of
/// `packages/forgecad-contracts/profiles/weaponry-knife-p0.json`; operation
/// registration itself remains in MCP and is intentionally not duplicated.
pub const STORE_FACADE_OWNERSHIPS: &[StoreFacadeOwnership] = &[
    StoreFacadeOwnership {
        facade: "weapon_preflight",
        logical_module: "forgecad_store::repositories::authoring",
        underlying_operation_count: 6,
        ownership_status: "ownership_seam_only:read_only_capability_and_skill_projection",
    },
    StoreFacadeOwnership {
        facade: "reference_intake",
        logical_module: "forgecad_store::repositories::authoring",
        underlying_operation_count: 5,
        ownership_status: "ownership_seam_only:authoring_reference_and_import_owner",
    },
    StoreFacadeOwnership {
        facade: "observe",
        logical_module: "forgecad_store::repositories::evaluation",
        underlying_operation_count: 10,
        ownership_status: "ownership_seam_only:read_only_evaluation_projection",
    },
    StoreFacadeOwnership {
        facade: "authoring_transaction",
        logical_module: "forgecad_store::repositories::authoring",
        underlying_operation_count: 15,
        ownership_status: "ownership_seam_only:authoring_command_and_readback_owner",
    },
    StoreFacadeOwnership {
        facade: "surface_pipeline",
        logical_module: "forgecad_store::repositories::surface",
        underlying_operation_count: 15,
        ownership_status: "ownership_seam_only:surface_artifact_and_bake_owner",
    },
    StoreFacadeOwnership {
        facade: "fps_presentation",
        logical_module: "forgecad_store::repositories::presentation",
        underlying_operation_count: 21,
        ownership_status: "ownership_seam_only:package_and_mechanical_animation_clip_owner;remaining_presentation_records_not_extracted",
    },
    StoreFacadeOwnership {
        facade: "quality_review",
        logical_module: "forgecad_store::repositories::evaluation",
        underlying_operation_count: 23,
        ownership_status: "ownership_seam_only:quality_evidence_evaluation_owner;strict_mapping_gap",
    },
    StoreFacadeOwnership {
        facade: "delivery",
        logical_module: "forgecad_store::repositories::delivery",
        underlying_operation_count: 6,
        ownership_status: "ownership_seam_only:delivery_and_export_owner;strict_mapping_gap",
    },
    StoreFacadeOwnership {
        facade: "approval",
        logical_module: "forgecad_store::repositories::delivery",
        underlying_operation_count: 5,
        ownership_status: "ownership_seam_only:approval_and_immutable_version_owner;strict_mapping_gap",
    },
    StoreFacadeOwnership {
        facade: "recovery",
        logical_module: "forgecad_store::repositories::authoring",
        underlying_operation_count: 11,
        ownership_status: "ownership_seam_only:checkpoint_restore_repair_authoring_owner;strict_mapping_gap",
    },
    StoreFacadeOwnership {
        facade: "job",
        logical_module: "forgecad_store::repositories::evaluation",
        underlying_operation_count: 8,
        ownership_status: "ownership_seam_only:typed_job_aggregate_extracted;subcontracts_partial",
    },
];

const AUTHORING_IMPLEMENTATION_MODULES: &[&str] = &[
    "src/authoring_repository.rs (borrowed Store façade for the first physical slice)",
    "src/lib.rs (legacy/base records)",
    "src/authoring_mesh_v2_transaction.rs",
    "src/foundation_authoring_mesh_v2_materialization.rs",
    "src/weapon_foundation_import.rs",
    "src/weaponry_curve_modifier_graph.rs",
    "src/weaponry_curve_evaluated_mesh.rs",
    "src/lib.rs (Agentic session/checkpoint/action compatibility)",
];

const AUTHORING_RECORD_TYPES: &[&str] = &[
    "AuthoringMeshProjectionIndexRecord",
    "AuthoringMeshDurableRecord",
    "AuthoringMeshV2DurableRecord",
    "AuthoringMeshV2TransactionDurableRecord",
    "AuthoringMeshIdentityLineageDurableRecord",
    "FoundationAuthoringMeshV2MaterializationRecord",
    "WeaponFoundationImportRecord",
    "WeaponryCurveModifierGraphDurableRecord",
    "WeaponryCurveEvaluatedMeshDurableRecord",
    "AgenticSessionRecord",
    "AgenticCheckpointRecord",
    "AgenticActionRunRecord",
];

const AUTHORING_TABLE_NAMES: &[&str] = &[
    "authoring_mesh_projection_indexes",
    "authoring_mesh_durable_records",
    "authoring_mesh_v2_durable_records",
    "authoring_mesh_v2_transactions",
    "authoring_mesh_identity_lineage_durable_records",
    "foundation_authoring_mesh_v2_materializations",
    "weapon_foundation_imports",
    "weaponry_curve_modifier_graph_records",
    "weaponry_curve_evaluated_mesh_records",
    "agentic_design_sessions",
    "agentic_design_checkpoints",
    "agentic_action_runs",
];

/// Record families whose durable lookup/record entry points are now exposed
/// through the borrowed `AuthoringRepository`.  The underlying compatibility
/// methods remain available until their implementations are moved in a later
/// atom; this list describes the public physical boundary, not a new schema.
pub const AUTHORING_REPOSITORY_EXTRACTED_RECORD_FAMILIES: &[&str] = &[
    "AuthoringMeshV2TransactionDurableRecord (including its revision chain)",
    "WeaponryCurveModifierGraphDurableRecord",
    "WeaponryCurveEvaluatedMeshDurableRecord",
];

/// Authoring record families still owned by the broad Store implementation.
/// Keeping this list explicit prevents the first repository façade from being
/// mistaken for a complete five-domain Store extraction.
pub const AUTHORING_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES: &[&str] = &[
    "AuthoringMeshProjectionIndexRecord",
    "AuthoringMeshDurableRecord",
    "AuthoringMeshIdentityLineageDurableRecord",
    "NativeHighDurableRecord",
    "FoundationAuthoringMeshV2MaterializationRecord",
    "WeaponFoundationImportRecord",
    "AgenticSessionRecord",
    "AgenticCheckpointRecord",
    "AgenticActionRunRecord",
];

const EVALUATION_IMPLEMENTATION_MODULES: &[&str] = &[
    "src/evaluation_repository.rs (borrowed Job/Event/Checkpoint aggregate façade)",
    "src/lib.rs (subdivision, quality, observe, visual and remaining evaluation compatibility)",
];

const EVALUATION_RECORD_TYPES: &[&str] = &[
    "VisualEvidenceRecord",
    "VisualEvidenceViewRecord",
    "CrossViewEvidenceRecord",
    "JobRecord",
    "JobSummary",
    "JobEventRecord",
];

const EVALUATION_TABLE_NAMES: &[&str] = &[
    "subdivision_artifact_lineage_links",
    "visual_evidence",
    "visual_evidence_views",
    "cross_view_evidence",
    "runtime_jobs",
    "runtime_job_events",
    "runtime_job_checkpoints",
];

/// Record families whose complete durable Job/Event/Checkpoint entry points
/// now live behind the borrowed `EvaluationRepository`.  The tables and
/// their transaction/replay semantics remain owned by `Store`.
pub const EVALUATION_REPOSITORY_EXTRACTED_RECORD_FAMILIES: &[&str] = &[
    "JobRecord / JobSummary (runtime_jobs)",
    "JobEventRecord (runtime_job_events)",
    "Job checkpoint bindings (runtime_job_checkpoints)",
];

/// Evaluation families intentionally left in the broad Store implementation
/// for later ReadModel/QualityEvidence extraction atoms.
pub const EVALUATION_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES: &[&str] = &[
    "VisualEvidenceRecord",
    "VisualEvidenceViewRecord",
    "CrossViewEvidenceRecord",
    "ReadModel project/candidate/snapshot projections",
    "QualityEvidence form/topology/material/animation records",
];

const SURFACE_IMPLEMENTATION_MODULES: &[&str] = &[
    "src/surface_repository.rs (physical formal High/Low/Cage/Bake aggregate boundary)",
    "src/production_weapon_formal_high.rs",
    "src/low_quad_durable.rs",
    "src/hero_uv_durable.rs",
    "src/lib.rs (remaining surface compatibility and aggregate codecs)",
];

const SURFACE_RECORD_TYPES: &[&str] = &[
    "ProductionWeaponHighArtifactRecord",
    "LowQuadDraftDurableRecord",
    "HeroUvDurableRecord",
    "ProductionWeaponHighLowBakeCommitBundle",
    "ProductionWeaponHighLowBakePreflightSourceSummary",
    "ProductionWeaponHighLowBakePreflightSources",
];

const SURFACE_TABLE_NAMES: &[&str] = &[
    "production_weapon_formal_high_links",
    "low_quad_draft_durable_links",
    "hero_uv_durable_links",
    "production_weapon_high_low_bake_links",
    "production_weapon_high_low_bake_high",
    "production_weapon_high_low_bake_low",
    "production_weapon_high_low_bake_cage",
    "production_weapon_high_low_bake_correspondence",
    "production_weapon_high_low_bake_plan",
    "production_weapon_high_low_bake_diagnostic",
    "production_weapon_high_low_bake_receipts",
];

/// Record/query façades physically moved into the borrowed Surface
/// repository.  Child rows remain the durable representation of the one
/// formal High/Low/Cage/Bake aggregate, so this list names the aggregate API
/// rather than inventing a second table or migration.
pub const SURFACE_REPOSITORY_EXTRACTED_RECORD_FAMILIES: &[&str] = &[
    "ProductionWeaponHighLowBakeCommitBundle (seven-row formal aggregate)",
    "ProductionWeaponHighLowBakePreflightSourceSummary",
    "ProductionWeaponHighLowBakePreflightSources",
];

/// Surface families intentionally left in their existing modules or in the
/// Store compatibility implementation for later extraction atoms.
pub const SURFACE_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES: &[&str] = &[
    "ProductionWeaponHighArtifactRecord (standalone formal High adapter)",
    "LowQuadDraftDurableRecord",
    "HeroUvDurableRecord",
    "ProductionWeaponRetopologyCageSourceBundle (source-only Low/Cage bundle)",
    "formal High/Low/Cage/Bake aggregate codecs and validation helpers",
];

const PRESENTATION_IMPLEMENTATION_MODULES: &[&str] = &[
    "src/fps_presentation_package_v2.rs",
    "src/presentation_repository.rs (physical MechanicalAnimationClip@1 aggregate)",
    "src/production_weapon_form_art_baseline.rs",
    "src/production_weapon_form_art_composite_proposal.rs",
    "src/production_weapon_form_art_composite_evidence.rs",
    "src/lib.rs (V2 animation/GLB/socket, visual evidence and Agentic projection compatibility)",
];

const PRESENTATION_RECORD_TYPES: &[&str] = &[
    "FpsPresentationPackageV2StoreRecord",
    "FpsPresentationPackageV2CandidateStoreRecord",
    "MechanicalAnimationClipLinkRecord",
    "ProductionWeaponFormArtProposalEvidenceRecord",
];

const PRESENTATION_TABLE_NAMES: &[&str] = &[
    "fps_presentation_packages_v2",
    "fps_presentation_package_v2_candidates",
    "mechanical_animation_clip_links",
    "production_weapon_form_art_baselines",
    "production_weapon_form_art_composite_proposal_links",
    "production_weapon_form_art_composite_evidence_links",
    "agentic_design_sessions",
    "agentic_design_checkpoints",
];

/// Presentation record families with a physical repository implementation.
/// The package family was already physically isolated; this atom moves the
/// remaining V1 mechanical clip write/read/list/rollback implementation and
/// its table bootstrap behind the same borrowed Store repository.
pub const PRESENTATION_REPOSITORY_EXTRACTED_RECORD_FAMILIES: &[&str] = &[
    "FpsPresentationPackageV2StoreRecord / FpsPresentationPackageV2CandidateStoreRecord (fps_presentation_package_v2.rs)",
    "MechanicalAnimationClipLinkRecord (record/get/list/discard; presentation_repository.rs)",
];

/// Presentation families intentionally still owned by their existing modules
/// or the broad Store compatibility implementation.  This prevents the V1
/// clip extraction from being mistaken for a complete 21-operation façade.
pub const PRESENTATION_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES: &[&str] = &[
    "MechanicalAnimationClipV2LinkRecord / MechanicalAnimationClipV2Record",
    "MechanicalAnimationGlbV2LinkRecord / MechanicalAnimationGlbV2ReceiptRecord",
    "GameWeaponAnchorLinkRecord and game weapon socket materialization records",
    "ProductionWeaponFormArtProposalEvidenceRecord",
    "VisualEvidenceRecord / VisualEvidenceViewRecord / CrossViewEvidenceRecord",
    "AgenticSessionRecord / AgenticCheckpointRecord / AgenticActionRunRecord",
];

const DELIVERY_IMPLEMENTATION_MODULES: &[&str] = &[
    "src/delivery_repository.rs (borrowed GameAssetDeliveryLinkRecord aggregate: record/get/list/commit)",
    "src/approval_repository.rs (physical Delivery-owned ApprovalLifecycle aggregate)",
    "src/cas.rs",
    "src/lib.rs (CAS and socket delivery compatibility)",
];

const DELIVERY_RECORD_TYPES: &[&str] = &[
    "GameAssetDeliveryLinkRecord",
    "CasObjectRecord",
    "DesignAssetVersionRecord",
    "ApprovalReceiptRecord",
    "ExportManifestRecord",
    "CandidateRecord",
];

const DELIVERY_TABLE_NAMES: &[&str] = &[
    "objects",
    "design_asset_versions",
    "approval_receipts",
    "export_manifests",
    "game_asset_delivery_links",
    "game_weapon_anchor_links",
    "game_weapon_glb_socket_materialization_links",
    "game_weapon_animated_glb_socket_materialization_links",
];

/// Delivery record families whose real Store transactions now live in the
/// borrowed `DeliveryRepository`.  The compatibility methods on `Store`
/// delegate to this implementation and do not own a second write path.
pub const DELIVERY_REPOSITORY_EXTRACTED_RECORD_FAMILIES: &[&str] =
    &[
        "GameAssetDeliveryLinkRecord (record/get/list/commit; game_asset_delivery_links)",
        "ApprovalReceiptRecord / DesignAssetVersionRecord / ExportManifestRecord (ApprovalLifecycle; approval_repository.rs)",
    ];

/// Delivery families intentionally retained in the Store compatibility root
/// for later atoms. Socket-sidecar tables remain named here so the ApprovalLifecycle
/// extraction is not overclaimed as a complete game-delivery extraction.
pub const DELIVERY_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES: &[&str] = &[
    "GameWeaponAnchorLinkRecord",
    "GameWeaponGlbSocketMaterializationLinkRecord and child LOD records",
    "GameWeaponAnimatedGlbSocketMaterializationLinkRecord and child LOD records",
    "CandidateRecord and Store CAS compatibility helpers",
];

/// First-phase repository ownership directory.  No entry changes migration
/// execution or CAS reachability; it is a static extraction map for the next
/// refactor cohort.
pub const STORE_REPOSITORY_BOUNDARIES: &[StoreRepositoryBoundary] = &[
    StoreRepositoryBoundary {
        domain: StoreRepositoryDomain::Authoring,
        logical_module: "forgecad_store::repositories::authoring",
        implementation_modules: AUTHORING_IMPLEMENTATION_MODULES,
        record_types: AUTHORING_RECORD_TYPES,
        table_names: AUTHORING_TABLE_NAMES,
        migration_owner: STORE_MIGRATION_OWNER,
        migration_source: STORE_MIGRATION_SOURCE,
        gc_root_policy: "authoring rows own canonical/source roots; parent revision and lineage ancestors remain reachable for evaluation",
        extraction_status: "physical_first_slice_authoring_repository_plus_logical_remaining_records",
    },
    StoreRepositoryBoundary {
        domain: StoreRepositoryDomain::Evaluation,
        logical_module: "forgecad_store::repositories::evaluation",
        implementation_modules: EVALUATION_IMPLEMENTATION_MODULES,
        record_types: EVALUATION_RECORD_TYPES,
        table_names: EVALUATION_TABLE_NAMES,
        migration_owner: STORE_MIGRATION_OWNER,
        migration_source: STORE_MIGRATION_SOURCE,
        gc_root_policy: "evaluation rows own graph/evaluated roots and retain exact source authoring lineage",
        extraction_status: "physical_first_slice_job_repository_plus_logical_remaining_evaluation_records",
    },
    StoreRepositoryBoundary {
        domain: StoreRepositoryDomain::Surface,
        logical_module: "forgecad_store::repositories::surface",
        implementation_modules: SURFACE_IMPLEMENTATION_MODULES,
        record_types: SURFACE_RECORD_TYPES,
        table_names: SURFACE_TABLE_NAMES,
        migration_owner: STORE_MIGRATION_OWNER,
        migration_source: STORE_MIGRATION_SOURCE,
        gc_root_policy: "surface rows own High/Low/UV/Cage/Bake roots and preserve linked source/candidate ancestors",
        extraction_status: "physical_first_slice_formal_high_low_bake_repository_plus_logical_remaining_surface_records",
    },
    StoreRepositoryBoundary {
        domain: StoreRepositoryDomain::Presentation,
        logical_module: "forgecad_store::repositories::presentation",
        implementation_modules: PRESENTATION_IMPLEMENTATION_MODULES,
        record_types: PRESENTATION_RECORD_TYPES,
        table_names: PRESENTATION_TABLE_NAMES,
        migration_owner: STORE_MIGRATION_OWNER,
        migration_source: STORE_MIGRATION_SOURCE,
        gc_root_policy: "presentation rows own package/render/clip roots; MechanicalAnimationClip@1 marks its canonical clip CAS object reachable in the same link transaction; presentation never infers approval from an evaluation projection",
        extraction_status: "physical_first_slice_presentation_repository_mechanical_animation_clip;fps_package_module_existing;remaining_presentation_records_not_extracted",
    },
    StoreRepositoryBoundary {
        domain: StoreRepositoryDomain::Delivery,
        logical_module: "forgecad_store::repositories::delivery",
        implementation_modules: DELIVERY_IMPLEMENTATION_MODULES,
        record_types: DELIVERY_RECORD_TYPES,
        table_names: DELIVERY_TABLE_NAMES,
        migration_owner: STORE_MIGRATION_OWNER,
        migration_source: STORE_MIGRATION_SOURCE,
        gc_root_policy: "game-delivery rows own all LOD/artifact and JSON sidecar roots; confirmed version/export/approval roots retain their complete manifest lineage; temporary CAS is reclaimed only by existing root walk",
        extraction_status: "physical_first_slice_delivery_repository_game_asset_delivery_link_and_approval_lifecycle;socket_records_not_extracted",
    },
];

/// A missing layer in the strict one-to-one mapping rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreMappingLayer {
    Contract,
    RuntimeService,
    StoreRecord,
    McpFacade,
}

/// Mapping data is owned by `forgecad-contracts`; this alias preserves the
/// Store inventory API without maintaining a second capability directory.
pub const STORE_CAPABILITY_MAPPINGS: &[StoreCapabilityMapping] = KNIFE_CAPABILITY_MAPPINGS;

/// A documented gap in the strict one-to-one mapping.  These are facts about
/// the current source tree, not failure statuses of a candidate or quality
/// gate.  They provide the extraction queue while preserving compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreMappingGap {
    pub capability: &'static str,
    pub domain: StoreRepositoryDomain,
    pub missing_layers: &'static [StoreMappingLayer],
    pub current_surface: &'static str,
    pub target_repository: &'static str,
    pub reason: &'static str,
}

const GAP_CONTRACT: &[StoreMappingLayer] = &[StoreMappingLayer::Contract];
const GAP_CONTRACT_AND_STORE: &[StoreMappingLayer] =
    &[StoreMappingLayer::Contract, StoreMappingLayer::StoreRecord];
const GAP_REPOSITORY_FACADE: &[StoreMappingLayer] = &[
    StoreMappingLayer::RuntimeService,
    StoreMappingLayer::StoreRecord,
];
const GAP_RUNTIME_SERVICE_ONLY: &[StoreMappingLayer] = &[StoreMappingLayer::RuntimeService];

/// Current mapping gaps, ordered by the knife workflow and then by the
/// compatibility/archive cleanup that must follow it.
pub const STORE_MAPPING_GAPS: &[StoreMappingGap] = &[
    StoreMappingGap {
        capability: "authoring_mesh_transaction_contract_source",
        domain: StoreRepositoryDomain::Authoring,
        missing_layers: GAP_CONTRACT,
        current_surface: "AuthoringMeshTransaction@1 is currently a package schema consumed by the Runtime/MCP lane; Store record and runtime bridge are present",
        target_repository: "forgecad-contracts::authoring_mesh_transaction",
        reason: "the four-layer mapping is operationally named but the contract is not yet exported from the central Rust contracts crate; do not add or rename it in this Store-only slice",
    },
    StoreMappingGap {
        capability: "knife_native_curve_contract_source",
        domain: StoreRepositoryDomain::Authoring,
        missing_layers: GAP_CONTRACT,
        current_surface: "KnifeCurveModifierGraph@1 and KnifeCurveEvaluatedMesh@1 request/result schemas are package-local MCP includes",
        target_repository: "forgecad-contracts::weaponry_curve",
        reason: "curve graph and EvaluatedMesh already have separate Store records and one Authoring repository owner, but their contract source is not centralized with forgecad-contracts",
    },
    StoreMappingGap {
        capability: "hero_uv_contract_source",
        domain: StoreRepositoryDomain::Surface,
        missing_layers: GAP_CONTRACT,
        current_surface: "HeroUvDurable@1 request/result/link schemas are package-local while hero_uv_durable.rs owns one Store record",
        target_repository: "forgecad-contracts::hero_uv_durable",
        reason: "the Store and Runtime persistence adapter are named, but the Contract source must be centralized in a later contracts-only cohort; this task does not create it",
    },
    StoreMappingGap {
        capability: "form_quality_and_evidence",
        domain: StoreRepositoryDomain::Evaluation,
        missing_layers: GAP_CONTRACT_AND_STORE,
        current_surface: "production_weapon_form_quality(_v2), form_evidence, form_art_evidence, visual_evidence and cross_view_evidence methods in lib.rs",
        target_repository: "forgecad_store::repositories::evaluation::QualityEvidenceRepository",
        reason: "the quality_review façade exists, but its operations bind several contracts, rows and CAS projections without one central capability mapping or Store aggregate owner",
    },
    StoreMappingGap {
        capability: "delivery_and_game_socket_materialization",
        domain: StoreRepositoryDomain::Delivery,
        missing_layers: GAP_RUNTIME_SERVICE_ONLY,
        current_surface: "GameAssetDeliveryLinkRecord record/get/list/commit is physically implemented in delivery_repository.rs; game_weapon_* socket materialization remains behind Store compatibility shims",
        target_repository: "forgecad_store::delivery_repository::DeliveryRepository",
        reason: "the first Store aggregate seam is closed for the immutable game-delivery link; Runtime direct typed Delivery service alignment and later socket aggregate extraction remain separate atoms",
    },
    StoreMappingGap {
        capability: "approval_confirm_and_export",
        domain: StoreRepositoryDomain::Delivery,
        missing_layers: GAP_RUNTIME_SERVICE_ONLY,
        current_surface: "candidate confirm/reject, approval_receipts, design_asset_versions and export_manifests in approval_repository.rs; Runtime service alignment remains pending",
        target_repository: "forgecad_store::approval_repository::ApprovalLifecycle",
        reason: "ApprovalLifecycle is physically isolated as a Delivery-owned Store module; one public workflow still updates several lifecycle tables and the Runtime service mapping remains a later atom",
    },
    StoreMappingGap {
        capability: "agentic_recovery_checkpoint",
        domain: StoreRepositoryDomain::Authoring,
        missing_layers: GAP_REPOSITORY_FACADE,
        current_surface: "AgenticSessionRecord, AgenticCheckpointRecord, restore and repair persistence methods in lib.rs",
        target_repository: "forgecad_store::repositories::authoring::RecoveryRepository",
        reason: "session/checkpoint/restore/repair records are present but do not yet have an independent Store repository façade",
    },
    StoreMappingGap {
        capability: "observe_read_model",
        domain: StoreRepositoryDomain::Evaluation,
        missing_layers: &[StoreMappingLayer::StoreRecord],
        current_surface: "project/candidate/snapshot/quality/visual read methods and MCP observe façade",
        target_repository: "forgecad_store::repositories::evaluation::ReadModelRepository",
        reason: "observe is intentionally a read projection over multiple records; a projection record must be defined before claiming one-to-one persistence",
    },
    StoreMappingGap {
        capability: "legacy_fps_and_fictional_vfx_compatibility",
        domain: StoreRepositoryDomain::Delivery,
        missing_layers: &[StoreMappingLayer::McpFacade],
        current_surface: "mechanical_animation, game socket and fictional_energy_vfx compatibility tables and methods in lib.rs",
        target_repository: "forgecad_store::repositories::archive::CompatibilityRepository",
        reason: "historical compatibility records still share the active Store root; they must move behind explicit archive/compatibility ownership without being registered by the knife Runtime root",
    },
];

/// Find one capability mapping without introducing a Runtime dependency.
pub fn mapping_for(capability: &str) -> Option<&'static StoreCapabilityMapping> {
    STORE_CAPABILITY_MAPPINGS
        .iter()
        .find(|mapping| mapping.capability == capability)
}

/// Find one repository boundary by its stable domain id.
pub fn boundary_for(domain: StoreRepositoryDomain) -> Option<&'static StoreRepositoryBoundary> {
    STORE_REPOSITORY_BOUNDARIES
        .iter()
        .find(|boundary| boundary.domain == domain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn directory_has_exactly_five_domains_in_dependency_order() {
        let domains = STORE_REPOSITORY_BOUNDARIES
            .iter()
            .map(|boundary| boundary.domain)
            .collect::<Vec<_>>();
        assert_eq!(domains, StoreRepositoryDomain::all());
        assert_eq!(domains.len(), 5);
        assert_eq!(
            STORE_REPOSITORY_BOUNDARIES
                .iter()
                .map(|boundary| boundary.logical_module)
                .collect::<BTreeSet<_>>()
                .len(),
            5
        );
    }

    #[test]
    fn knife_facade_ownership_is_closed_and_snapshot_counts_reconcile() {
        let facades = STORE_FACADE_OWNERSHIPS
            .iter()
            .map(|ownership| ownership.facade)
            .collect::<BTreeSet<_>>();
        assert_eq!(STORE_FACADE_OWNERSHIPS.len(), KNIFE_PROFILE_FACADE_COUNT);
        assert_eq!(facades.len(), KNIFE_PROFILE_FACADE_COUNT);
        assert_eq!(
            STORE_FACADE_OWNERSHIPS
                .iter()
                .map(|ownership| ownership.underlying_operation_count)
                .sum::<usize>(),
            KNIFE_PROFILE_OPERATION_OCCURRENCE_COUNT
        );
        assert_eq!(
            KNIFE_PROFILE_OPERATION_OCCURRENCE_COUNT,
            KNIFE_PROFILE_UNIQUE_OPERATION_COUNT
        );
        assert_eq!(KNIFE_PROFILE_CROSS_FACADE_OPERATION_NAME_COUNT, 0);
        assert_eq!(KNIFE_PROFILE_DUPLICATE_OWNER_COUNT, 0);
        let authoring = boundary_for(StoreRepositoryDomain::Authoring).expect("authoring");
        assert!(authoring
            .record_types
            .contains(&"WeaponryCurveModifierGraphDurableRecord"));
        assert!(authoring
            .record_types
            .contains(&"WeaponryCurveEvaluatedMeshDurableRecord"));
    }

    #[test]
    fn runtime_facade_domain_alignment_is_explicit() {
        assert_eq!(
            STORE_FACADE_OWNERSHIPS
                .iter()
                .map(|ownership| ownership.facade)
                .collect::<BTreeSet<_>>(),
            forgecad_contracts::weaponry_domain_map::KNIFE_FACADE_BINDINGS
                .iter()
                .map(|binding| binding.facade_name)
                .collect::<BTreeSet<_>>()
        );
        for ownership in STORE_FACADE_OWNERSHIPS {
            assert_eq!(
                ownership.domain(),
                forgecad_contracts::weaponry_domain_map::knife_facade_binding(ownership.facade)
                    .expect("Contract façade binding")
                    .domain
            );
        }
        assert!(STORE_FACADE_OWNERSHIPS.iter().all(|ownership| {
            ownership
                .ownership_status
                .starts_with("ownership_seam_only:")
        }));
    }

    #[test]
    fn every_domain_keeps_one_migration_owner_and_source() {
        assert_eq!(STORE_MIGRATION_SEQUENCE, &[STORE_MIGRATION_SOURCE]);
        for boundary in STORE_REPOSITORY_BOUNDARIES {
            assert_eq!(boundary.migration_owner, STORE_MIGRATION_OWNER);
            assert_eq!(boundary.migration_source, STORE_MIGRATION_SOURCE);
            assert!(!boundary.table_names.is_empty());
            assert!(!boundary.record_types.is_empty());
            assert!(boundary.gc_root_policy.contains("root"));
            assert!(
                boundary
                    .extraction_status
                    .starts_with("logical_boundary_only_")
                    || boundary
                        .extraction_status
                        .starts_with("physical_first_slice_")
            );
        }
    }

    #[test]
    fn authoring_repository_extraction_report_is_explicit_and_bounded() {
        assert_eq!(
            AUTHORING_REPOSITORY_EXTRACTED_RECORD_FAMILIES,
            &[
                "AuthoringMeshV2TransactionDurableRecord (including its revision chain)",
                "WeaponryCurveModifierGraphDurableRecord",
                "WeaponryCurveEvaluatedMeshDurableRecord",
            ]
        );
        assert!(!AUTHORING_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES.is_empty());
        assert!(AUTHORING_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES
            .iter()
            .all(|family| !family.is_empty()));
        let authoring = boundary_for(StoreRepositoryDomain::Authoring).expect("authoring");
        assert!(authoring
            .extraction_status
            .starts_with("physical_first_slice_"));
    }

    #[test]
    fn surface_repository_extraction_report_is_explicit_and_bounded() {
        assert_eq!(
            SURFACE_REPOSITORY_EXTRACTED_RECORD_FAMILIES,
            &[
                "ProductionWeaponHighLowBakeCommitBundle (seven-row formal aggregate)",
                "ProductionWeaponHighLowBakePreflightSourceSummary",
                "ProductionWeaponHighLowBakePreflightSources",
            ]
        );
        assert!(!SURFACE_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES.is_empty());
        assert!(SURFACE_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES
            .iter()
            .all(|family| !family.is_empty()));
        let surface = boundary_for(StoreRepositoryDomain::Surface).expect("surface");
        assert!(surface
            .implementation_modules
            .iter()
            .any(|module| module.starts_with("src/surface_repository.rs")));
        assert!(surface
            .extraction_status
            .starts_with("physical_first_slice_"));
        assert!(surface
            .record_types
            .contains(&"ProductionWeaponHighLowBakePreflightSources"));
    }

    #[test]
    fn evaluation_repository_extraction_report_is_explicit_and_bounded() {
        assert_eq!(
            EVALUATION_REPOSITORY_EXTRACTED_RECORD_FAMILIES,
            &[
                "JobRecord / JobSummary (runtime_jobs)",
                "JobEventRecord (runtime_job_events)",
                "Job checkpoint bindings (runtime_job_checkpoints)",
            ]
        );
        assert!(!EVALUATION_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES.is_empty());
        assert!(EVALUATION_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES
            .iter()
            .all(|family| !family.is_empty()));
        let evaluation = boundary_for(StoreRepositoryDomain::Evaluation).expect("evaluation");
        assert!(evaluation
            .implementation_modules
            .iter()
            .any(|module| module.starts_with("src/evaluation_repository.rs")));
        assert!(evaluation
            .extraction_status
            .starts_with("physical_first_slice_"));
        assert!(evaluation.record_types.contains(&"JobRecord"));
        assert!(evaluation.record_types.contains(&"JobEventRecord"));
        assert!(evaluation.table_names.contains(&"runtime_job_checkpoints"));
    }

    #[test]
    fn presentation_repository_extraction_report_is_explicit_and_bounded() {
        assert_eq!(
            PRESENTATION_REPOSITORY_EXTRACTED_RECORD_FAMILIES,
            &[
                "FpsPresentationPackageV2StoreRecord / FpsPresentationPackageV2CandidateStoreRecord (fps_presentation_package_v2.rs)",
                "MechanicalAnimationClipLinkRecord (record/get/list/discard; presentation_repository.rs)",
            ]
        );
        assert!(!PRESENTATION_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES.is_empty());
        assert!(PRESENTATION_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES
            .iter()
            .all(|family| !family.is_empty()));
        let presentation = boundary_for(StoreRepositoryDomain::Presentation).expect("presentation");
        assert!(presentation
            .implementation_modules
            .iter()
            .any(|module| module.starts_with("src/presentation_repository.rs")));
        assert!(presentation
            .extraction_status
            .starts_with("physical_first_slice_"));
        assert!(presentation
            .record_types
            .contains(&"MechanicalAnimationClipLinkRecord"));
        assert!(presentation
            .table_names
            .contains(&"mechanical_animation_clip_links"));
    }

    #[test]
    fn delivery_repository_extraction_report_is_explicit_and_bounded() {
        assert_eq!(
            DELIVERY_REPOSITORY_EXTRACTED_RECORD_FAMILIES,
            &[
                "GameAssetDeliveryLinkRecord (record/get/list/commit; game_asset_delivery_links)",
                "ApprovalReceiptRecord / DesignAssetVersionRecord / ExportManifestRecord (ApprovalLifecycle; approval_repository.rs)",
            ]
        );
        assert!(!DELIVERY_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES.is_empty());
        assert!(DELIVERY_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES
            .iter()
            .all(|family| !family.is_empty()));
        let delivery = boundary_for(StoreRepositoryDomain::Delivery).expect("delivery");
        assert!(delivery
            .implementation_modules
            .iter()
            .any(|module| module.starts_with("src/delivery_repository.rs")));
        assert!(delivery
            .extraction_status
            .starts_with("physical_first_slice_"));
        assert!(delivery
            .record_types
            .contains(&"GameAssetDeliveryLinkRecord"));
        assert!(delivery.table_names.contains(&"game_asset_delivery_links"));
    }

    #[test]
    fn complete_mappings_have_one_owner_per_layer() {
        let mut capabilities = BTreeSet::new();
        for mapping in STORE_CAPABILITY_MAPPINGS {
            assert!(capabilities.insert(mapping.capability));
            assert!(!mapping.mcp_operations.is_empty());
            assert!(mapping.runtime_service.is_some());
            assert!(mapping.mcp_facade.is_some());
            match mapping.persistence {
                PersistenceKind::None => {
                    assert!(mapping.contract.is_some());
                    assert!(mapping.store_record.is_none());
                }
                PersistenceKind::Projection | PersistenceKind::DurableTransaction => {
                    assert!(
                        mapping.contract.is_some()
                            || mapping.persistence == PersistenceKind::Projection
                    );
                    assert!(mapping.store_record.is_some());
                }
            }
            if mapping.status == StoreMappingStatus::Complete {
                assert!(mapping.mcp_operations.len() <= 3);
            }
        }
    }

    #[test]
    fn every_gap_explains_its_target_and_missing_layer() {
        let mut capabilities = BTreeSet::new();
        for gap in STORE_MAPPING_GAPS {
            assert!(capabilities.insert(gap.capability));
            assert!(!gap.missing_layers.is_empty());
            assert!(!gap.current_surface.is_empty());
            assert!(!gap.target_repository.is_empty());
            assert!(!gap.reason.is_empty());
        }
    }

    #[test]
    fn lookup_is_stable() {
        assert_eq!(
            mapping_for("hero_uv").map(|value| value.domain),
            Some(StoreRepositoryDomain::Surface)
        );
        assert_eq!(mapping_for("missing"), None);
        assert_eq!(
            boundary_for(StoreRepositoryDomain::Delivery).map(|value| value.domain),
            Some(StoreRepositoryDomain::Delivery)
        );
    }
}
