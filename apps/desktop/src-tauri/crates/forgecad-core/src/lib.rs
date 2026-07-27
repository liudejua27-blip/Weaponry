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
mod canonical;
mod component_recipes;
mod error;
mod external_glb;
mod filesystem_permissions;
mod forge_visual_authoring_intent;
mod forge_visual_program;
mod generation_gate_profile;
mod legacy_conversion;
mod lifecycle;
mod migration;
mod models;
mod multimodal_design;
mod neural_visual_generation;
mod object_store;
mod ownership;
mod reference_evidence;
mod repository;
mod semantic_proportions;
mod shape_program;
mod single_generation;
mod skills;
mod surface_layers;
mod visual_convergence;

pub use arm_design_intent::{
    lower_arm_design_intent, ArmDesignIntent, ArmRecipeLowering, ARM_DESIGN_INTENT_SCHEMA_VERSION,
    ARM_RECIPE_LOWERING_SCHEMA_VERSION,
};
pub use arm_geometry_family::{
    apply_arm_geometry_family, apply_serial_chain_geometry_family, ArmGeometryFamilyBinding,
    ARM_GEOMETRY_FAMILY_SCHEMA_VERSION,
};
pub use artifact_migration::{ArtifactMigrationReport, ArtifactMigrationRunner};
pub use artifact_readback::{verify_forgecad_glb, ForgeCadGlbReadback};
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
    build_c111_forge_visual_program_fixture, reviewed_c111_draft_visual_program,
    C111ForgeVisualProgramFixture, C111_FORGE_VISUAL_PROGRAM_FIXTURE_SCHEMA_VERSION,
};
pub use canonical::{canonical_json, semantic_sha256};
pub use component_recipes::{
    ComponentRecipeInstanceProvenance, ComponentRecipeRef, EditableComponentRecipe,
    ExpandedComponentCandidate, ExpandedComponentInstance, RecipeConnector, RecipeExpander,
    RecipeExpansionPolicy, RecipeFrame, RecipeInstantiationRequest, RecipeMaterialZoneOverride,
    RecipeParameterValue, RecipeRegistry, RecipeSlotBinding, RecipeSurfaceAdornmentSlot,
    RecipeTransform, RecipeValidator,
};
pub use error::{CoreError, CoreResult};
pub use external_glb::{
    inspect_external_glb, is_external_glb_reference, ExternalGlbImportBundleReadback,
    ImportExternalGlbRequest, ImportExternalGlbResponse, ImportedGlbInspection, ImportedGlbRecord,
    EXTERNAL_GLB_ARTIFACT_PROFILE_ID, EXTERNAL_GLB_REFERENCE_ROLE, MAX_IMPORTED_GLB_BYTES,
    MAX_IMPORTED_GLB_TRIANGLES,
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
pub use generation_gate_profile::{
    evaluate_native_v003_gate_profile_v2, native_v003_gate_profile_sha256,
    NativeGateEvidenceSource, NativeGenerationGateBinding, NativeGenerationGateEvaluation,
    NativeGenerationGateEvidence, NATIVE_GENERATION_GATE_EVALUATION_SCHEMA_VERSION,
    NATIVE_GENERATION_GATE_EVIDENCE_SCHEMA_VERSION, NATIVE_V003_GATE_IDS,
    NATIVE_V003_GATE_PROFILE_CANONICAL, NATIVE_V003_GATE_PROFILE_ID,
    NATIVE_V003_GATE_PROFILE_SHA256, NATIVE_V003_GATE_PROFILE_VERSION,
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
    VisualReferenceClaimAssessment, VisualReferenceComparisonInput,
    VisualReferenceComparisonReport, VisualReferenceMatchOutcome, VisualReferenceSourceFingerprint,
    MULTIMODAL_DESIGN_REQUEST_SCHEMA_VERSION, MULTIMODAL_PROGRAM_EVIDENCE_BINDING_SCHEMA_VERSION,
    VISUAL_EVIDENCE_GRAPH_SCHEMA_VERSION, VISUAL_REFERENCE_COMPARISON_INPUT_SCHEMA_VERSION,
    VISUAL_REFERENCE_COMPARISON_REPORT_SCHEMA_VERSION,
};
pub use neural_visual_generation::{
    inspect_concept_png, inspect_neural_visual_glb, ConceptImageBackend,
    ConceptImageGenerationRequest, ConceptImageResumeBinding, ConceptPngInspection,
    ConceptReferenceArtifact, ForgeAssetPackage, ForgeAssetPackageFile, HiddenSurfacePolicy,
    Neural3DBackend, Neural3DGenerationRequest, Neural3DResumeBinding, NeuralVisualArtifact,
    NeuralVisualGenerationJob, NeuralVisualGlbInspection, NeuralVisualStage, PbrChannel,
    VisualDesignBrief, VisualInputEvidence, VisualInputKind, VisualQualityTier,
    VisualRemoteJobRecord, VisualRemoteJobState, CONCEPT_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION,
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
pub use reference_evidence::{
    analyze_reference_image_bytes, reference_rebuild_plan_id_for_change_set,
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
    c111_golden_surface_layer_program, c111_link_finish_surface_layer,
    AgentSkillActivation, AgentSkillDryRun, AgentSkillEvalReport, AgentSkillManifest,
    SkillEvalStatus, SkillExample, SkillLicense, SkillProvenance, SurfaceAdornmentProgram,
};
pub use surface_layers::{
    DecalLayer, EmissiveMask, NormalReliefLayer, RetainedSurfaceLayers, RoughnessMask,
    SurfaceLayerLowering, SurfaceLayerProgram, SurfaceSymmetry, UvFrame, VectorPath,
    VectorPathCommand,
};
pub use visual_convergence::{
    DesignBuildLedger, VisualBuildPass, VisualBuildStage, VisualConvergenceInput,
    VisualConvergenceReport, VisualDetailCoverage, VisualFixedViewEvidence,
    VisualGlbReadbackEvidence, VisualReferenceConvergenceEvidence, VisualRepairEvidence,
    DESIGN_BUILD_LEDGER_SCHEMA_VERSION, MAX_VISUAL_REPAIR_ATTEMPTS, REQUIRED_VISUAL_VIEW_IDS,
    VISUAL_CONVERGENCE_INPUT_SCHEMA_VERSION, VISUAL_CONVERGENCE_REPORT_SCHEMA_VERSION,
};
