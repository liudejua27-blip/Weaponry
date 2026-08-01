//! Rust-owned ForgeCAD product-state core.
//!
//! This crate deliberately owns no Provider credentials, desktop window state
//! or geometry implementation. It reuses the existing SQLite schema and keeps
//! the Python boundary suitable for a restricted geometry executor that only
//! returns validated bytes/readback to Rust.

mod arm_design_intent;
mod arm_geometry_family;
mod artifact_migration;
mod artifact_readback;
mod assembly_delta;
mod c111_structural_detail;
mod c111_visual_fixture;
mod candidate_pbr_capture;
mod canonical;
mod component_recipes;
mod e005_formal_batch;
mod e005_provider_budget;
mod e005_visual_patch_v1;
mod e005_visual_review_checkpoint;
mod error;
mod expanded_visual_dag_v2;
mod external_glb;
mod filesystem_permissions;
mod forge_visual_author_source_v1;
mod forge_visual_authoring_intent;
mod forge_visual_program;
mod forge_visual_program_v2;
mod game_asset_delivery;
mod game_asset_lod;
mod game_asset_profile;
mod generation_gate_profile;
mod geometry_invariant_binding;
mod high_level_visual_geometry_v2;
mod legacy_conversion;
mod lifecycle;
mod migration;
mod models;
mod multimodal_design;
mod neural_visual_generation;
mod object_store;
mod ownership;
mod projection_camera_binding;
mod reference_appearance_binding;
mod reference_camera_fit;
mod reference_camera_uv_bake;
mod reference_evidence;
mod repository;
mod semantic_proportions;
mod shape_program;
mod single_generation;
mod skills;
mod surface_layers;
mod universal_asset_source;
mod universal_authoring;
mod visual_convergence;
mod visual_geometry_patch_v2;
mod visual_program_authoring_session_v2;
mod visual_reference_budget;

pub use arm_design_intent::{
    lower_arm_design_intent, ArmDesignIntent, ArmRecipeLowering, ARM_DESIGN_INTENT_SCHEMA_VERSION,
    ARM_RECIPE_LOWERING_SCHEMA_VERSION,
};
pub use arm_geometry_family::{
    apply_arm_geometry_family, apply_serial_chain_geometry_family, ArmGeometryFamilyBinding,
    ARM_GEOMETRY_FAMILY_SCHEMA_VERSION,
};
pub use artifact_migration::{ArtifactMigrationReport, ArtifactMigrationRunner};
pub use artifact_readback::{normalized_geometry_sha256, verify_forgecad_glb, ForgeCadGlbReadback};
pub use assembly_delta::{
    lower_assembly_delta, materialize_assembly_delta, AssemblyDeltaLowering,
    AssemblyDeltaOperation, AssemblyDeltaProgram, DeltaJointPose, DeltaTransform,
    ASSEMBLY_DELTA_LOWERING_SCHEMA_VERSION, ASSEMBLY_DELTA_PROGRAM_SCHEMA_VERSION,
};
pub use c111_structural_detail::{
    build_c111_structural_detail_contract, C111StructuralDetailContract,
    C111StructuralDetailLineage, C111_STRUCTURAL_DETAIL_SCHEMA_VERSION,
};
pub use c111_visual_fixture::{
    build_c111_forge_visual_program_fixture, c111b_visual_reference_acceptance_policy,
    c111b_visual_reference_acceptance_policy_for_domain, reviewed_c111_draft_visual_program,
    C111ForgeVisualProgramFixture, C111_FORGE_VISUAL_PROGRAM_FIXTURE_SCHEMA_VERSION,
};
pub use candidate_pbr_capture::{
    CandidatePbrCaptureEvidence, CandidatePbrCaptureSession, CandidatePbrCaptureSubmission,
    CandidatePbrCapturedView, CANDIDATE_PBR_CAPTURE_EVIDENCE_SCHEMA_VERSION,
    CANDIDATE_PBR_CAPTURE_SESSION_SCHEMA_VERSION, CANDIDATE_PBR_RENDERER_ID,
    MAX_CAPTURE_AUXILIARY_VIEW_BYTES, MAX_CAPTURE_TOTAL_BYTES, MAX_CAPTURE_TTL_MS,
    MAX_CAPTURE_VIEW_BYTES, TURN_TABLE_EIGHT_VIEW_IDS, WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX,
    WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX, WORKBENCH_PBR_AUXILIARY_PASS_HEIGHT_PX,
    WORKBENCH_PBR_AUXILIARY_PASS_WIDTH_PX, WORKBENCH_PBR_CAPTURE_HEIGHT_PX,
    WORKBENCH_PBR_CAPTURE_WIDTH_PX, WORKBENCH_PBR_RENDER_MANIFEST_SHA256,
    WORKBENCH_PBR_RENDERER_ID, WORKBENCH_PBR_VISUAL_ENVIRONMENT_ID,
    WORKBENCH_PBR_VISUAL_ENVIRONMENT_SHA256,
};
pub use canonical::{canonical_json, semantic_sha256};
pub use component_recipes::{
    ComponentRecipeInstanceProvenance, ComponentRecipeRef, EditableComponentRecipe,
    ExpandedComponentCandidate, ExpandedComponentInstance, RecipeConnector, RecipeExpander,
    RecipeExpansionPolicy, RecipeFrame, RecipeInstantiationRequest, RecipeMaterialZoneOverride,
    RecipeParameterValue, RecipeRegistry, RecipeSlotBinding, RecipeSurfaceAdornmentSlot,
    RecipeTransform, RecipeValidator,
};
pub use e005_formal_batch::{
    E005FormalBatchCheckpoint, E005FormalBatchStatus, E005FormalBatchTaskCheckpoint,
    E005FormalBatchTaskClaim, E005FormalBatchTaskState, E005_FORMAL_BATCH_SCHEMA_VERSION,
    E005_FORMAL_BATCH_TASK_SCHEMA_VERSION,
};
pub use e005_provider_budget::{
    E005ProviderBudgetEvidence, E005ProviderBudgetLedger, E005ProviderCallKind,
    E005ProviderCallOutcome, E005ProviderCallReservation, E005ProviderCallReservationRequest,
    E005ProviderCallSettlement, E005ProviderRunAuthorizationContract, E005_FORMAL_TASK_SET_SHA256,
    E005_MAXIMUM_AUTHOR_CALLS, E005_MAXIMUM_PATCH_CALLS, E005_MAXIMUM_TOTAL_CALLS,
    E005_PROVIDER_BUDGET_EVIDENCE_SCHEMA_VERSION, E005_PROVIDER_LEDGER_SCHEMA_VERSION,
    E005_PROVIDER_RESERVATION_SCHEMA_VERSION, E005_PROVIDER_RUN_AUTHORIZATION_SCHEMA_VERSION,
    E005_TASK_COUNT,
};
pub use e005_visual_patch_v1::{
    apply_e005_visual_patch_v1, seal_e005_visual_patch_proposal_v1,
    validate_e005_visual_patch_against_comparison_v1, E005VisualDecisionKindV1,
    E005VisualPatchOperationV1, E005VisualPatchProposalV1, E005VisualPatchResultV1,
    E005VisualPatchV1, E005_VISUAL_PATCH_PROPOSAL_SCHEMA_VERSION,
    E005_VISUAL_PATCH_RESULT_SCHEMA_VERSION, E005_VISUAL_PATCH_SCHEMA_VERSION,
};
pub use e005_visual_review_checkpoint::{
    E005ProviderUsageCheckpoint, E005VisualReviewCheckpoint, E005VisualReviewCheckpointState,
    E005_VISUAL_REVIEW_CHECKPOINT_SCHEMA_VERSION,
};
pub use error::{CoreError, CoreResult};
pub use expanded_visual_dag_v2::{
    expand_and_lower_forge_visual_composition_v2, expand_forge_visual_composition_v2,
    ExpandedVisualDagV2, ExpandedVisualProgramLoweringV2, EXPANDED_VISUAL_DAG_SCHEMA_VERSION,
    FORGE_VISUAL_COMPOSITION_SCHEMA_VERSION, VP202_COMPILER_VERSION,
};
pub use external_glb::{
    inspect_external_glb, is_external_glb_reference, ExternalGlbImportBundleReadback,
    ImportExternalGlbRequest, ImportExternalGlbResponse, ImportedGlbInspection, ImportedGlbRecord,
    EXTERNAL_GLB_ARTIFACT_PROFILE_ID, EXTERNAL_GLB_REFERENCE_ROLE, MAX_IMPORTED_GLB_BYTES,
    MAX_IMPORTED_GLB_TRIANGLES,
};
pub use forge_visual_author_source_v1::{
    lower_forge_visual_author_source_v1, lower_visual_runtime_source_v1, AuthorScalarV1,
    AuthorSurfaceProfileV1, ForgeVisualAuthorLoweringV1, ForgeVisualAuthorSourceLineageV1,
    ForgeVisualAuthorSourceV1, ForgeVisualAuthorSurfacePlanV1,
    ForgeVisualSemanticDensityEvidenceV1, VisualRuntimeSourceLoweringV1,
    FORGE_VISUAL_AUTHOR_LOWERING_SCHEMA_VERSION, FORGE_VISUAL_AUTHOR_SOURCE_SCHEMA_VERSION,
};
pub use forge_visual_authoring_intent::{
    lower_forge_visual_authoring_intent, ForgeVisualAuthoringIntent,
    FORGE_VISUAL_AUTHORING_INTENT_SCHEMA_VERSION,
};
pub use forge_visual_program::{
    compiled_visual_base_material_id, lower_forge_visual_program, ForgeVisualDesignToken,
    ForgeVisualExportProfile, ForgeVisualInspectionView, ForgeVisualMaterialBinding,
    ForgeVisualPart, ForgeVisualPatch, ForgeVisualPatchOperation, ForgeVisualProgram,
    ForgeVisualProgramInspection, ForgeVisualProgramLowering, ForgeVisualProgramRevision,
    ForgeVisualProgramStage, ForgeVisualSurfaceBinding, VisualDetailBinding,
    VisualDetailBindingKind, VisualDetailInventoryItem, VisualDetailLevel, VisualDetailStatus,
    COMPILED_VISUAL_MATERIAL_IDS, FORGE_VISUAL_PATCH_SCHEMA_VERSION,
    FORGE_VISUAL_PROGRAM_INSPECTION_SCHEMA_VERSION, FORGE_VISUAL_PROGRAM_LOWERING_SCHEMA_VERSION,
    FORGE_VISUAL_PROGRAM_REVISION_SCHEMA_VERSION, FORGE_VISUAL_PROGRAM_SCHEMA_VERSION,
};
pub use forge_visual_program_v2::{
    lower_forge_visual_program_v2, ForgeVisualMaterialV2, ForgeVisualNodeV2, ForgeVisualOutputV2,
    ForgeVisualParameterKindV2, ForgeVisualParameterUnitV2, ForgeVisualParameterV2,
    ForgeVisualProgramBudgetV2, ForgeVisualProgramLoweringV2, ForgeVisualProgramV2,
    ForgeVisualScalarV2, ForgeVisualSourceMapEntryV2, ForgeVisualSourceMapV2,
    ForgeVisualUnitSystemV2, FORGE_VISUAL_PROGRAM_V2_LOWERING_SCHEMA_VERSION,
    FORGE_VISUAL_PROGRAM_V2_SCHEMA_VERSION, FORGE_VISUAL_SOURCE_MAP_SCHEMA_VERSION,
};
pub use game_asset_delivery::{
    compile_game_asset_delivery, compile_game_asset_lod_delivery,
    derive_game_asset_delivery_bindings, verify_game_asset_delivery_glb,
    verify_game_asset_lod_delivery_glb, GameAssetCollisionProxyReadback, GameAssetDeliveryArtifact,
    GameAssetDeliveryBindings, GameAssetDeliveryPartBinding, GameAssetDeliveryReadback,
    GameAssetLodDelivery, GameAssetLodDeliveryReadback, GameAssetLodLevelReadback,
    GameAssetMaterialTexelDensityReadback, GameAssetSocketReadback, GameAssetTexelDensityReadback,
    GAME_ASSET_DELIVERY_BINDINGS_SCHEMA_VERSION, GAME_ASSET_DELIVERY_RECEIPT_SCHEMA_VERSION,
    GAME_ASSET_LOD_RECEIPT_SCHEMA_VERSION,
};
pub use game_asset_lod::{
    simplify_game_asset_lod, simplify_game_asset_lod_with_global_error, GameAssetLodMesh,
    GameAssetLodVertex, GAME_ASSET_LOD_TARGET_ERROR,
};
pub use game_asset_profile::{
    GameAssetDeliveryRequest, GameAssetProfile, GameAssetSocket,
    GAME_ASSET_DELIVERY_REQUEST_SCHEMA_VERSION, GAME_ASSET_PROFILE_SCHEMA_VERSION,
};
pub use generation_gate_profile::{
    evaluate_native_v003_gate_profile_v2, native_v003_gate_profile_sha256,
    NativeGateEvidenceSource, NativeGenerationGateBinding, NativeGenerationGateEvaluation,
    NativeGenerationGateEvidence, NATIVE_GENERATION_GATE_EVALUATION_SCHEMA_VERSION,
    NATIVE_GENERATION_GATE_EVIDENCE_SCHEMA_VERSION, NATIVE_V003_GATE_IDS,
    NATIVE_V003_GATE_PROFILE_CANONICAL, NATIVE_V003_GATE_PROFILE_ID,
    NATIVE_V003_GATE_PROFILE_SHA256, NATIVE_V003_GATE_PROFILE_VERSION,
};
pub use geometry_invariant_binding::{
    derive_geometry_invariant_binding, GeometryInvariantBinding,
    GEOMETRY_INVARIANT_BINDING_SCHEMA_VERSION,
};
pub use high_level_visual_geometry_v2::{
    lower_forge_visual_geometry_program_v2, ExpandedVisualGeometryDagV2,
    ForgeVisualGeometryLoweringV2, ForgeVisualGeometryProgramV2, GeometryAxisV2,
    GeometryCapPolicyV2, GeometryProfileV2, GeometrySectionSetV2, GeometrySectionV2,
    HighLevelGeometryBudgetV2, HighLevelGeometryNodeV2, HighLevelGeometryOutputV2,
    VisualGeometryBudgetEvidenceV2, VisualGeometrySourceMapEntryV2,
    EXPANDED_VISUAL_GEOMETRY_DAG_SCHEMA_VERSION, FORGE_VISUAL_GEOMETRY_LOWERING_SCHEMA_VERSION,
    FORGE_VISUAL_GEOMETRY_PROGRAM_SCHEMA_VERSION, VP203_COMPILER_VERSION,
};
pub use legacy_conversion::{
    LegacyActiveDesignConversionResponse, LegacyActiveDesignSource, LegacyAgentConversionIntent,
    LEGACY_CONVERSION_READY,
};
pub use lifecycle::LifecycleStore;
pub use migration::{MigrationReport, MigrationRunner, CURRENT_LEGACY_MIGRATION};
pub use models::{
    ActiveDesign, ActiveDesignSnapshot, AgentAssetChangeSet, AgentAssetVersion,
    AgentComponentCandidate, AgentComponentCompatibility, AgentComponentRecord,
    AgentStructureSuggestion, AgentStructureSuggestionList, AssetStage, AssetVersionStatus,
    BlockoutCandidate, CandidateBundleReadback, CandidateStatus, ChangeSetConfirmBundleReadback,
    ChangeSetPreviewBundleReadback, ChangeSetStatus, ConfirmedAsset, DraftArtifactReference,
    DraftCandidate, DraftCandidateBundleReadback, DraftCandidateStatus, ExportReference,
    GameDeliveryCandidateBundleReadback,
    MaterialTextureLicense, MaterialTextureObject, MaterialTextureQuery, MaterialTextureRole,
    MaterialTextureSource, MaterialTextureSummary, NavigationAction, NavigationAvailability,
    NavigationResult, ObjectRecord, ObjectReference, PartDisplay, PreviewReference, Project,
    ProjectStatus, QualityReference, QualityReport, QualityStatus, RegisterMaterialTextureRequest,
    RenderPreset, Selection, SnapshotEtag,
};
pub use multimodal_design::{
    MultimodalDesignLocks, MultimodalDesignRequest, MultimodalProgramEvidenceBinding,
    MultimodalReferenceInput, MultimodalSelectionScope, NormalizedEvidenceRegion, ReferenceRole,
    VisionEvidenceProviderProvenance, VisualClaimDisposition, VisualClaimDispositionKind,
    VisualClaimStatus, VisualClaimTarget, VisualEvidenceClaim, VisualEvidenceGraph,
    VisualReferenceAcceptancePolicy, VisualReferenceCandidateViewProfile,
    VisualReferenceClaimAssessment, VisualReferenceComparisonInput,
    VisualReferenceComparisonReport, VisualReferenceMatchOutcome, VisualReferenceRenderContract,
    VisualReferenceSourceFingerprint,
    MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION, MULTIMODAL_PROGRAM_EVIDENCE_BINDING_SCHEMA_VERSION,
    VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION, VISUAL_REFERENCE_ACCEPTANCE_POLICY_SCHEMA_VERSION,
    VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION,
    VISUAL_REFERENCE_COMPARISON_REPORT_SCHEMA_VERSION,
    VISUAL_REFERENCE_RENDER_CONTRACT_SCHEMA_VERSION,
};
pub use neural_visual_generation::{
    inspect_concept_png, inspect_neural_visual_glb, ConceptImageBackend,
    ConceptImageGenerationRequest, ConceptImageResumeBinding, ConceptPngInspection,
    ConceptReferenceArtifact, ForgeAssetPackage, ForgeAssetPackageFile, HiddenSurfacePolicy,
    Neural3DAdditionalView, Neural3DAdditionalViewRole, Neural3DBackend, Neural3DGenerationRequest,
    Neural3DResumeBinding, NeuralVisualArtifact, NeuralVisualGenerationJob,
    NeuralVisualGlbInspection, NeuralVisualStage, PbrChannel, VisualDesignBrief,
    VisualInputEvidence, VisualInputKind, VisualQualityTier, VisualRemoteJobRecord,
    VisualRemoteJobState, CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION,
    CONCEPT_REFERENCE_ARTIFACT_SCHEMA_VERSION, CONCEPT_REFERENCE_HEIGHT, CONCEPT_REFERENCE_WIDTH,
    FORGE_ASSET_PACKAGE_SCHEMA_VERSION, MAX_CONCEPT_REFERENCE_PNG_BYTES,
    MAX_NEURAL_VISUAL_GLB_BYTES, MAX_NEURAL_VISUAL_TRIANGLES,
    NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION, NEURAL_VISUAL_ARTIFACT_SCHEMA_VERSION,
    NEURAL_VISUAL_GENERATION_JOB_SCHEMA_VERSION, REQUIRED_MULTIVIEW_RENDER_COUNT,
    VISUAL_DESIGN_BRIEF_SCHEMA_VERSION, VISUAL_REMOTE_JOB_RECORD_SCHEMA_VERSION,
};
pub use object_store::{ContentAddressedObjectStore, PromotedObject, StagedObject, StoredObject};
pub use ownership::{
    read_ownership_marker, BootstrapLease, OwnershipMarker, StateOwner, WriterLease,
    WriterLeaseRecovery,
};
pub use projection_camera_binding::{
    derive_geometry_projection_camera_binding, derive_projection_camera_binding,
    GeometryProjectionCameraBinding, ProjectionCameraBinding,
    GEOMETRY_PROJECTION_CAMERA_BINDING_SCHEMA_VERSION, PROJECTION_CAMERA_BINDING_ALGORITHM_ID,
    PROJECTION_CAMERA_BINDING_ALGORITHM_VERSION, PROJECTION_CAMERA_BINDING_SCHEMA_VERSION,
    PROJECTION_CAMERA_FRAME_TARGET_NDC, PROJECTION_CAMERA_VERTICAL_FOV_MILLIDEGREES,
};
pub use reference_appearance_binding::{
    derive_reference_appearance_bindings, ReferenceAppearanceBinding,
    REFERENCE_APPEARANCE_BINDING_SCHEMA_VERSION,
};
pub use reference_camera_fit::{
    fit_reference_camera_from_view_regions, CandidateCameraSilhouette, ReferenceViewRegion,
    MAX_SILHOUETTE_CENTER_ERROR_PER_MILLE, MAX_SILHOUETTE_FIT_CONFIDENCE_BPS,
    MAX_SILHOUETTE_PROFILE_ERROR_PER_MILLE, MIN_SILHOUETTE_IOU_BPS,
    SILHOUETTE_PROFILE_BUCKET_COUNT,
};
pub use reference_camera_uv_bake::{
    build_reference_camera_uv_raster_bake, build_reference_camera_uv_raster_bake_from_geometry,
    ReferenceCameraUvRasterBake, ReferenceCameraUvRasterTextureProfile,
    MAX_REFERENCE_CAMERA_UV_RASTER_SOURCE_BYTES, REFERENCE_CAMERA_UV_RASTER_ALGORITHM_ID,
    REFERENCE_CAMERA_UV_RASTER_ALGORITHM_VERSION, REFERENCE_CAMERA_UV_RASTER_BAKE_SCHEMA_VERSION,
};
pub use reference_evidence::{
    analyze_reference_image_bytes, derive_reference_silhouette_profile,
    reference_rebuild_plan_id_for_change_set,
    validate_reference_surface_analysis_for_plan, CreateReferenceEvidenceRequest, ReferenceClass,
    ReferenceEvidence, ReferenceEvidenceKind, ReferenceEvidenceObservations,
    ReferenceGuidedRebuildPlan, ReferenceGuidedRebuildPlanStatus, ReferenceImageBrightnessBucket,
    ReferenceImageColorBucket, ReferenceImageEdgeDensityBucket, ReferenceImageForegroundConfidence,
    ReferenceImageSurfaceFacts, ReferenceSurfaceAnalysis, ReferenceSurfaceBinding,
    ReferenceSurfaceFidelityCeiling, ReferenceSurfaceGlbReadbackFacts,
    ReferenceSurfaceIntentionalChange, ReferenceSurfaceObservationKind, ReferenceSurfaceUnresolved,
    VisiblePartHypothesis, REFERENCE_EVIDENCE_SCHEMA_VERSION, REFERENCE_EVIDENCE_SOURCE_ROLE,
    REFERENCE_GUIDED_REBUILD_PLAN_SCHEMA_VERSION, REFERENCE_SURFACE_ANALYSIS_SCHEMA_VERSION,
};
pub use repository::{CoreRepository, LegacyModuleGlb, ReferenceGuidedRebuildFrozenPair};
pub use semantic_proportions::{
    resolve_semantic_proportions, MechanicalStyleToken, ResolvedSemanticProportionOption,
    ResolvedSemanticProportionOptions,
};
pub use shape_program::normalize_persisted_shape_program;
pub use single_generation::{
    GenerationAttemptKind, GenerationCancel, GenerationFailure, GenerationGateCheck,
    GenerationGateReport, GenerationPreview, RepairAttempt, SingleGenerationAttempt,
    SingleGenerationSession, SingleGenerationSessionState, SingleResultDecision,
    SingleResultOutcome, SingleResultState, VerificationOutcome,
    GENERATION_GATE_REPORT_SCHEMA_VERSION, MAX_SAME_INTENT_REPAIR_ATTEMPTS,
    REPAIR_ATTEMPT_SCHEMA_VERSION, SINGLE_GENERATION_ATTEMPT_SCHEMA_VERSION,
    SINGLE_RESULT_DECISION_SCHEMA_VERSION,
};
pub use skills::{
    builtin_surface_adornment_manifest, builtin_surface_adornment_manifest_v2,
    builtin_surface_adornment_manifest_v3, c111_golden_surface_adornment_programs,
    c111_golden_surface_layer_program, c111_link_finish_surface_layer, AgentSkillActivation,
    AgentSkillDryRun, AgentSkillEvalReport, AgentSkillManifest, SkillEvalStatus, SkillExample,
    SkillLicense, SkillProvenance, SurfaceAdornmentProgram,
};
pub use surface_layers::{
    DecalLayer, EmissiveMask, NormalReliefLayer, RetainedSurfaceLayers, RoughnessMask,
    SurfaceLayerLowering, SurfaceLayerProgram, SurfaceSymmetry, UvFrame, VectorPath,
    VectorPathCommand,
};
pub use universal_asset_source::{
    AppearanceEvidenceArtifact, AppearanceEvidenceArtifactKind, AppearanceEvidenceBundle,
    AppearanceEvidenceReference, AppearanceProjectionLayer, CameraParameterSource,
    MaterialZoneAppearance, PbrTextureChannel, ReferenceCameraHypothesis, ReferenceProjectionType,
    ReferenceAppearanceProjectionReceipt,
    UniversalAssetSource, UniversalAssetSourceState, UniversalAssetSourceV2,
    UniversalCompiledArtifactBinding, UniversalComponentSource, UniversalDetailBinding,
    UniversalDetailBindingKind, UniversalLatticeDeformationBindingV2,
    UniversalLocalHardSurfaceHybridSourceV2, UniversalLocalLatticeDeformSourceV2,
    UniversalLocalMeshPatchBindingV2, UniversalLocalMeshPatchSourceV2,
    UniversalProceduralPartBindingV2,
    UniversalProceduralSourceV2, UniversalRepresentationSourceV2,
    VisualDetailClaimV2,
    GenericHardSurfaceAppearanceCompilation, GenericHardSurfaceAppearanceZone,
    ReferenceSurfaceAppearanceBinding,
    APPEARANCE_EVIDENCE_BUNDLE_SCHEMA_VERSION, MATERIAL_ZONE_APPEARANCE_SCHEMA_VERSION,
    REFERENCE_SURFACE_APPEARANCE_BINDING_SCHEMA_VERSION,
    REFERENCE_APPEARANCE_PROJECTION_RECEIPT_SCHEMA_VERSION,
    REFERENCE_CAMERA_HYPOTHESIS_SCHEMA_VERSION, UNIVERSAL_ASSET_SOURCE_SCHEMA_VERSION,
    UNIVERSAL_ASSET_SOURCE_V2_SCHEMA_VERSION, VISUAL_DETAIL_CLAIM_V2_SCHEMA_VERSION,
};
pub use universal_authoring::{
    representation_capability_manifest, representation_capability_manifest_sha256,
    AppearanceChannel, CapabilityAvailability, EvidenceStatus, PartRepresentationPlan,
    RepresentationCapability, RepresentationCapabilityManifest, RepresentationKind,
    RepresentationLimitation, RepresentationLimitationCode, RepresentationPlan, SubjectFeature,
    SubjectMaterial, SubjectPart, SubjectProfile, UniversalActiveAssetBinding,
    UniversalAuthorOutcome, UniversalAuthorRequest, UniversalDesignLocks, UniversalEvidenceClaim,
    UniversalInputMode, UniversalReferenceInput, UniversalSelectionScope, VisualEvidenceGraphV2,
    VisualFeatureContract, VisualFeatureEvidenceRegion, VisualFeatureLevel,
    VisualFeatureRequirement, GENERIC_HARD_SURFACE_PROCEDURAL_CAPABILITY_ID,
    GENERIC_VISUAL_EXTERIOR_PROCEDURAL_CAPABILITY_ID,
    LOCAL_LATTICE_DEFORMABLE_CAPABILITY_ID, LOCAL_MESH_PATCH_CAPABILITY_ID,
    REPRESENTATION_CAPABILITY_MANIFEST_SCHEMA_VERSION,
    REPRESENTATION_LIMITATION_SCHEMA_VERSION, REPRESENTATION_PLAN_SCHEMA_VERSION,
    ROBOTIC_ARM_PROCEDURAL_CAPABILITY_ID, SUBJECT_PROFILE_SCHEMA_VERSION,
    UNIVERSAL_AUTHOR_OUTCOME_SCHEMA_VERSION, UNIVERSAL_AUTHOR_REQUEST_SCHEMA_VERSION,
    VISUAL_EVIDENCE_GRAPH_V2_SCHEMA_VERSION, VISUAL_FEATURE_CONTRACT_SCHEMA_VERSION,
};
pub use visual_convergence::{
    DesignBuildLedger, VisualBuildPass, VisualBuildStage, VisualConvergenceInput,
    VisualConvergenceReport, VisualDetailCoverage, VisualFixedViewEvidence, VisualFixedViewProfile,
    VisualGlbReadbackEvidence, VisualReferenceConvergenceEvidence, VisualRepairEvidence,
    DESIGN_BUILD_LEDGER_SCHEMA_VERSION, MAX_VISUAL_REPAIR_ATTEMPTS, REQUIRED_VISUAL_VIEW_IDS,
    TURN_TABLE_VISUAL_VIEW_IDS, VISUAL_CONVERGENCE_INPUT_SCHEMA_VERSION,
    VISUAL_CONVERGENCE_REPORT_SCHEMA_VERSION,
};
pub use visual_geometry_patch_v2::{
    apply_forge_visual_geometry_patch_v2, ForgeVisualGeometryPatchOperationV2,
    ForgeVisualGeometryPatchV2, GeometryIncrementalPlanV2, PatchedVisualGeometryProgramV2,
    FORGE_VISUAL_GEOMETRY_PATCH_SCHEMA_VERSION, GEOMETRY_INCREMENTAL_PLAN_SCHEMA_VERSION,
};
pub use visual_program_authoring_session_v2::{
    VisualProgramAuthoringSessionV2, VisualProgramAuthoringStateV2,
    VisualProgramCacheDispositionV2, VisualProgramExecutionReceiptV2, VisualProgramGateOutcomeV2,
    VisualProgramGateVerdictV2, VisualProgramPhaseReceiptV2, VisualProgramPhaseV2,
    VisualProgramUsageV2, VISUAL_PROGRAM_AUTHORING_SESSION_SCHEMA_VERSION,
    VISUAL_PROGRAM_EXECUTION_RECEIPT_SCHEMA_VERSION, VISUAL_PROGRAM_GATE_OUTCOME_SCHEMA_VERSION,
};
pub use visual_reference_budget::{
    visual_reference_authorization_binding_sha256, VisualReferenceComparisonAuthorization,
    VisualReferenceComparisonBudgetEvidence, VisualReferenceComparisonReservation,
    VISUAL_REFERENCE_COMPARISON_AUTHORIZATION_LIFETIME_MS,
    VISUAL_REFERENCE_COMPARISON_AUTHORIZATION_SCHEMA_VERSION,
    VISUAL_REFERENCE_COMPARISON_BUDGET_EVIDENCE_SCHEMA_VERSION,
    VISUAL_REFERENCE_COMPARISON_MAXIMUM_CALLS,
    VISUAL_REFERENCE_COMPARISON_MAXIMUM_VARIABLE_COST_MICROUSD,
    VISUAL_REFERENCE_COMPARISON_RESERVATION_SCHEMA_VERSION,
};
