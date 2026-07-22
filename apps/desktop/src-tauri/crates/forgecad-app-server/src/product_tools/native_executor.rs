//! Rust-owned execution of the immutable ForgeCAD Product Tool registry.
//!
//! This module deliberately keeps the high-level Agent state on the Rust side.
//! The only boundary that may invoke the restricted geometry worker contains
//! an already-expanded ShapeProgram/Profile/SectionSet and a code-owned quality
//! profile.  It has no lifecycle identity, Provider authority, persistence
//! capability, machine location, or URL.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use forgecad_app_server_protocol::{
    ProductToolExecutionRequest, ProductToolExecutionResult, ProductToolExecutionStatus,
    ProductToolFailureCategory, ValidatedProductToolPayload,
    PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION, PRODUCT_TOOL_REGISTRY_SCHEMA_VERSION,
};
use forgecad_core::{
    evaluate_native_v003_gate_profile_v2, lower_assembly_delta, normalize_persisted_shape_program,
    ComponentRecipeInstanceProvenance, ComponentRecipeRef, GenerationAttemptKind,
    GenerationGateReport, GenerationPreview, NativeGateEvidenceSource, NativeGenerationGateBinding,
    NativeGenerationGateEvidence, RecipeExpander, RecipeExpansionPolicy,
    RecipeInstantiationRequest, RecipeRegistry, RecipeValidator, RepairAttempt,
    SingleGenerationAttempt, SingleResultDecision, SurfaceAdornmentProgram, SurfaceLayerLowering,
    SurfaceLayerProgram, VerificationOutcome, MAX_SAME_INTENT_REPAIR_ATTEMPTS,
    NATIVE_GENERATION_GATE_EVIDENCE_SCHEMA_VERSION, REPAIR_ATTEMPT_SCHEMA_VERSION,
    SINGLE_GENERATION_ATTEMPT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::{
    validate_json_schema, GenerationSourceBinding, GenerationSourceKind, ProductToolCancelFuture,
    ProductToolExecutorPort, ProductToolPortError, ProductToolPortErrorKind, ProductToolPortFuture,
    ProductToolRegistry, MAX_PRODUCT_TOOL_CALLS,
};
use crate::{
    canonical::{canonical_json, sha256_hex},
    CancellationToken,
};

pub const RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION: &str = "RestrictedGeometryInput@1";
pub const RESTRICTED_GEOMETRY_OUTPUT_SCHEMA_VERSION: &str = "RestrictedGeometryOutput@1";
pub const RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION: &str = "ShapeProgramRuntimeManifest@1";
pub const NATIVE_PREVIEW_ARTIFACT_SCHEMA_VERSION: &str = "NativePreviewArtifact@1";
pub const NATIVE_PREVIEW_ASSEMBLY_SCHEMA_VERSION: &str = "NativePreviewAssemblyFacts@1";
pub const NATIVE_SINGLE_RESULT_PROVENANCE_SCHEMA_VERSION: &str = "NativeSingleResultProvenance@1";
const MAX_GLTF_BYTES: usize = 64 * 1024 * 1024;
// ShapeProgram@1 caps the requested tessellation budget at 100k. Production
// readback can legitimately exceed that request after bevel/profile
// refinement, so the output envelope is reviewed separately below.
const MAX_SHAPE_PROGRAM_TRIANGLE_BUDGET: u32 = 100_000;
const MAX_VIEW_BYTES: usize = 16 * 1024 * 1024;
const MAX_PREVIEW_ARTIFACT_BYTES_HARD: usize = 512 * 1024 * 1024;
const REQUIRED_VIEWS: [&str; 4] = ["front", "iso", "side", "top"];
const BOUNDED_REPAIR_PATCH_SCHEMA_VERSION: &str = "BoundedGeometryRepairPatch@1";
const G819_OPERATIONS: [&str; 16] = [
    "box",
    "cylinder",
    "capsule",
    "wedge",
    "profile",
    "extrude",
    "revolve",
    "loft",
    "sweep",
    "mirror",
    "array",
    "radial_array",
    "union",
    "subtract",
    "bevel_approx",
    "surface_panel",
];
const FORBIDDEN_KEYS: [&str; 33] = [
    "provider_key",
    "api_key",
    "authorization",
    "bearer_token",
    "access_token",
    "client_secret",
    "provider_url",
    "base_url",
    "endpoint_url",
    "reasoning",
    "reasoning_content",
    "raw_reasoning",
    "hidden_reasoning",
    "chain_of_thought",
    "database_path",
    "sqlite_path",
    "object_store_path",
    "object_path",
    "filesystem_path",
    "file_path",
    "db_path",
    "repository_path",
    "snapshot_write_token",
    "snapshot_token",
    "asset_write_token",
    "asset_token",
    "thread_id",
    "session",
    "session_id",
    "history",
    "messages",
    "url",
    "uri",
];

pub type RestrictedGeometryFuture = Pin<
    Box<
        dyn Future<Output = Result<RestrictedGeometryOutput, RestrictedGeometryError>>
            + Send
            + 'static,
    >,
>;

/// The only Python/worker-facing K003 geometry capability.
pub trait RestrictedGeometryPort: Send + Sync + 'static {
    fn build_compile_render(
        &self,
        input: RestrictedGeometryInput,
        cancellation: CancellationToken,
    ) -> RestrictedGeometryFuture;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RestrictedGeometryErrorKind {
    InvalidInput,
    Unsupported,
    Cancelled,
    Timeout,
    Execution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RestrictedGeometryError {
    pub code: String,
    pub kind: RestrictedGeometryErrorKind,
    pub message: String,
    pub recoverable: bool,
}

impl RestrictedGeometryError {
    pub fn execution(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            kind: RestrictedGeometryErrorKind::Execution,
            message: message.into(),
            recoverable: false,
        }
    }

    pub fn cancelled() -> Self {
        Self {
            code: "RESTRICTED_GEOMETRY_CANCELLED".into(),
            kind: RestrictedGeometryErrorKind::Cancelled,
            message: "Restricted geometry work was cancelled.".into(),
            recoverable: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RestrictedQualityProfile {
    pub profile_id: String,
    pub runtime_manifest_version: String,
    pub max_triangle_count: u32,
    pub render_width: u16,
    pub render_height: u16,
    pub require_closed_manifold: bool,
    pub require_surface_provenance: bool,
}

impl RestrictedQualityProfile {
    fn for_presentation(value: &str) -> Result<Self, NativeToolFailure> {
        match value {
            "quick_sketch" => Ok(Self {
                profile_id: "interactive_preview".into(),
                runtime_manifest_version: RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION.into(),
                max_triangle_count: 7_000,
                render_width: 320,
                render_height: 320,
                require_closed_manifold: true,
                require_surface_provenance: true,
            }),
            "showcase" => Ok(Self {
                profile_id: "production_concept".into(),
                runtime_manifest_version: RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION.into(),
                max_triangle_count: 150_000,
                // The production GLB itself is displayed by the workbench's
                // single Three.js renderer. These software-raster images are
                // deterministic audit thumbnails, not the visual-quality
                // source of truth; bounding them avoids blocking on four
                // redundant full-size CPU renders of a 100k-triangle asset.
                render_width: 128,
                render_height: 128,
                require_closed_manifold: true,
                require_surface_provenance: true,
            }),
            _ => Err(NativeToolFailure::schema(
                "PRESENTATION_PROFILE_INVALID",
                "Presentation profile is outside the code-owned quality profiles.",
            )),
        }
    }

    fn validate(&self) -> Result<(), NativeToolFailure> {
        if !matches!(
            self.profile_id.as_str(),
            "interactive_preview" | "production_concept"
        ) || self.runtime_manifest_version != RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION
            || !(100..=150_000).contains(&self.max_triangle_count)
            || !(64..=2048).contains(&self.render_width)
            || !(64..=2048).contains(&self.render_height)
            || !self.require_closed_manifold
            || !self.require_surface_provenance
        {
            return Err(NativeToolFailure::schema(
                "RESTRICTED_QUALITY_PROFILE_INVALID",
                "Restricted geometry quality profile is not code-owned or bounded.",
            ));
        }
        Ok(())
    }
}

/// Strict worker request.  It intentionally has no execution or product ID.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestrictedGeometryInput {
    pub schema_version: String,
    pub shape_program: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_sketch: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_set: Option<Value>,
    /// Bounded visual-only texture programs. They carry no lifecycle or write
    /// capability; Rust validates the active immutable Skill before this
    /// restricted compiler input is constructed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_adornment_programs: Vec<SurfaceAdornmentProgram>,
    /// A private-construction Rust seal around one `SurfaceLayerProgram@1`
    /// lowering. Python receives only this reviewed DTO and cannot provide a
    /// second authoring language or select a different A005 program list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_layer_input: Option<RestrictedSurfaceLayerInput>,
    pub quality_profile: RestrictedQualityProfile,
}

/// Exact Rust-owned retained-surface payload for the RestrictedGeometryPort.
/// Its fields are private so normal callers can only construct it by lowering
/// a validated `SurfaceLayerProgram` through `from_program`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestrictedSurfaceLayerInput {
    schema_version: String,
    lowering: SurfaceLayerLowering,
    lowering_sha256: String,
}

impl RestrictedSurfaceLayerInput {
    pub fn from_program(program: &SurfaceLayerProgram) -> Result<Self, RestrictedGeometryError> {
        let lowering = program
            .lower()
            .map_err(|error| restricted_input_error(error.to_string()))?;
        // The new retained PBR compiler currently shares the existing A005
        // base-material channel. A Design Surface with no reviewed relief may
        // remain stored/previewed at the Core layer, but it cannot cross this
        // v1 executor boundary until its base-material contract is explicit.
        if lowering.adornments().is_empty() {
            return Err(restricted_input_error(
                "Restricted surface lowering requires at least one reviewed A005 normal-relief layer.",
            ));
        }
        let lowering_sha256 = restricted_surface_lowering_sha256(&lowering)?;
        let sealed = Self {
            schema_version: "RestrictedSurfaceLayerInput@1".into(),
            lowering,
            lowering_sha256,
        };
        sealed.validate()?;
        Ok(sealed)
    }

    pub fn lowering(&self) -> &SurfaceLayerLowering {
        &self.lowering
    }

    pub fn lowering_sha256(&self) -> &str {
        &self.lowering_sha256
    }

    fn validate(&self) -> Result<(), RestrictedGeometryError> {
        if self.schema_version != "RestrictedSurfaceLayerInput@1" {
            return Err(restricted_input_error(
                "Restricted surface lowering schema version is unsupported.",
            ));
        }
        self.lowering
            .validate()
            .map_err(|error| restricted_input_error(error.to_string()))?;
        if self.lowering.adornments().is_empty()
            || !is_sha256(&self.lowering_sha256)
            || self.lowering_sha256 != restricted_surface_lowering_sha256(&self.lowering)?
        {
            return Err(restricted_input_error(
                "Restricted surface lowering is not an exact Rust-owned sealed payload.",
            ));
        }
        Ok(())
    }
}

fn restricted_surface_lowering_sha256(
    lowering: &SurfaceLayerLowering,
) -> Result<String, RestrictedGeometryError> {
    let value = serde_json::to_value(lowering).map_err(|_| {
        restricted_input_error("Restricted surface lowering could not be serialized.")
    })?;
    Ok(sha256_hex(canonical_json(&value).as_bytes()))
}

impl RestrictedGeometryInput {
    pub fn validate(&self) -> Result<(), RestrictedGeometryError> {
        if self.schema_version != RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION {
            return Err(restricted_input_error(
                "Restricted geometry input version is unsupported.",
            ));
        }
        let serialized = serde_json::to_value(self).map_err(|_| {
            restricted_input_error("Restricted geometry input could not be inspected.")
        })?;
        reject_forbidden_json(&serialized)
            .map_err(|failure| restricted_input_error(failure.message))?;
        reject_high_level_geometry_context(&serialized)
            .map_err(|failure| restricted_input_error(failure.message))?;
        self.quality_profile
            .validate()
            .map_err(|failure| restricted_input_error(failure.message))?;
        validate_shape_program_value(&self.shape_program)
            .map_err(|failure| restricted_input_error(failure.message))?;
        let program_budget = self
            .shape_program
            .get("triangle_budget")
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX);
        if program_budget
            > u64::from(
                self.quality_profile
                    .max_triangle_count
                    .min(MAX_SHAPE_PROGRAM_TRIANGLE_BUDGET),
            )
        {
            return Err(restricted_input_error(
                "ShapeProgram triangle budget exceeds its code-owned quality profile.",
            ));
        }
        if let Some(profile) = self.profile_sketch.as_ref() {
            validate_profile_sketch_value(profile)
                .map_err(|failure| restricted_input_error(failure.message))?;
        }
        if let Some(section_set) = self.section_set.as_ref() {
            validate_section_set_value(section_set)
                .map_err(|failure| restricted_input_error(failure.message))?;
        }
        if self.surface_adornment_programs.len() > 8 {
            return Err(restricted_input_error(
                "Restricted geometry accepts at most eight visual adornments.",
            ));
        }
        for program in &self.surface_adornment_programs {
            program
                .validate()
                .map_err(|failure| restricted_input_error(failure.to_string()))?;
        }
        if let Some(surface_layer_input) = self.surface_layer_input.as_ref() {
            surface_layer_input.validate()?;
            if self.surface_adornment_programs.as_slice()
                != surface_layer_input.lowering().adornments()
            {
                return Err(restricted_input_error(
                    "Restricted surface layers must carry the exact A005 list emitted by their Rust lowering.",
                ));
            }
        }
        validate_geometry_companion_binding(
            &self.shape_program,
            self.profile_sketch.as_ref(),
            self.section_set.as_ref(),
        )?;
        Ok(())
    }

    /// Attaches a reviewed Design Surface atomically. The A005 list is copied
    /// only from the same sealed lowering; callers cannot pair arbitrary
    /// retained layers with unrelated texture-bake programs.
    pub fn with_surface_layer_program(
        mut self,
        program: &SurfaceLayerProgram,
    ) -> Result<Self, RestrictedGeometryError> {
        if self.surface_layer_input.is_some() || !self.surface_adornment_programs.is_empty() {
            return Err(restricted_input_error(
                "Restricted geometry accepts one sealed Design Surface instead of independently supplied adornments.",
            ));
        }
        let surface_layer_input = RestrictedSurfaceLayerInput::from_program(program)?;
        self.surface_adornment_programs = surface_layer_input.lowering().adornments().to_vec();
        self.surface_layer_input = Some(surface_layer_input);
        self.validate()?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestrictedGeometryReadback {
    pub runtime_manifest_version: String,
    pub artifact_profile_id: String,
    pub shape_program_sha256: String,
    pub glb_sha256: String,
    pub glb_byte_size: u64,
    pub triangle_count: u32,
    pub bounds_mm: [f64; 3],
    pub mesh_count: u32,
    pub primitive_count: u32,
    pub material_count: u32,
    pub closed_manifold: bool,
    pub surface_provenance_present: bool,
    /// Canonical digest of the exact `GeometryCompileReadback@2` value
    /// accepted by the Rust↔Python boundary.  It lets the V003 evidence
    /// profile cite real compiler facts without retaining the full report in
    /// the transient Product Tool state.
    pub compile_readback_sha256: String,
    /// Bounded summaries copied only after the bridge validates the worker's
    /// material-zone and visual-texture evidence.
    pub material_zone_count: u32,
    pub visual_texture_set_count: u32,
    pub visual_texture_map_count: u32,
    pub visual_texture_provenance_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RestrictedGeometryOutput {
    pub schema_version: String,
    pub glb_bytes: Vec<u8>,
    pub glb_sha256: String,
    pub topology_hash: String,
    pub readback: RestrictedGeometryReadback,
    pub views: BTreeMap<String, Vec<u8>>,
    pub view_sha256: BTreeMap<String, String>,
    pub renderer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativePreviewPartFact {
    pub part_id: String,
    pub output_id: String,
    pub operation_id: String,
    pub part_role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_zone_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativePreviewAssemblyFacts {
    pub schema_version: String,
    pub assembly_id: String,
    pub root_part_id: String,
    pub parts: Vec<NativePreviewPartFact>,
}

/// Trusted V003 identity retained beside transient bytes. This is created only
/// when the native Thread lifecycle has already bound the execution to a
/// Project. Legacy compatibility builds deliberately retain `None` here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativeSingleResultProvenance {
    pub schema_version: String,
    pub project_id: String,
    pub plan_id: String,
    pub direction_id: String,
    pub domain_pack_id: String,
    pub decision: SingleResultDecision,
    pub decision_sha256: String,
}

/// Ephemeral candidate bytes owned only by the Rust app-server.
///
/// It deliberately excludes the Python artifact handle, Provider/session
/// state, paths, and persistence authority.  Confirmation may consume this
/// value and transfer its immutable facts into Rust product core.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NativePreviewArtifact {
    pub schema_version: String,
    pub preview_id: String,
    pub turn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formal_provenance: Option<NativeSingleResultProvenance>,
    pub shape_program: Value,
    pub assembly: NativePreviewAssemblyFacts,
    pub recipe_assembly_graph: Option<Value>,
    pub recipe_component_instances: Option<Value>,
    pub recipe_candidate_sha256: Option<String>,
    pub recipe_expanded_shape_program_sha256: Option<String>,
    pub glb_bytes: Vec<u8>,
    pub glb_sha256: String,
    pub readback: RestrictedGeometryReadback,
    pub views: BTreeMap<String, Vec<u8>>,
    pub view_sha256: BTreeMap<String, String>,
    pub renderer_id: String,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl NativePreviewArtifact {
    pub fn validate(&self) -> Result<(), ProductToolPortError> {
        if self.schema_version != NATIVE_PREVIEW_ARTIFACT_SCHEMA_VERSION
            || !is_stable_id(&self.preview_id)
            || !self.preview_id.starts_with("preview_")
            || !is_stable_id(&self.turn_id)
            || self.created_at_unix_ms >= self.expires_at_unix_ms
            || self.glb_bytes.is_empty()
            || self.glb_bytes.len() > MAX_GLTF_BYTES
            || self.glb_sha256 != sha256_hex(&self.glb_bytes)
            || self.readback.glb_sha256 != self.glb_sha256
            || self.readback.glb_byte_size != self.glb_bytes.len() as u64
            || self.renderer_id.is_empty()
            || self.renderer_id.len() > 120
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Native preview artifact failed identity or byte integrity validation.",
            ));
        }
        validate_shape_program_value(&self.shape_program).map_err(|failure| {
            preview_port_error("NATIVE_PREVIEW_ARTIFACT_INVALID", failure.message)
        })?;
        reject_high_level_geometry_context(&self.shape_program).map_err(|failure| {
            preview_port_error("NATIVE_PREVIEW_ARTIFACT_INVALID", failure.message)
        })?;
        let expected_program_sha256 = sha256_hex(canonical_json(&self.shape_program).as_bytes());
        if self.readback.shape_program_sha256 != expected_program_sha256 {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Native preview ShapeProgram identity does not match readback.",
            ));
        }
        let required = REQUIRED_VIEWS.into_iter().collect::<BTreeSet<_>>();
        if self
            .views
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != required
            || self
                .view_sha256
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != required
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Native preview must contain exactly four views and hashes.",
            ));
        }
        for (view_id, bytes) in &self.views {
            if bytes.is_empty()
                || bytes.len() > MAX_VIEW_BYTES
                || self.view_sha256.get(view_id) != Some(&sha256_hex(bytes))
            {
                return Err(preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Native preview view bytes and hashes do not match.",
                ));
            }
        }
        validate_preview_assembly(&self.shape_program, &self.assembly)?;
        if let Some(provenance) = &self.formal_provenance {
            provenance.decision.validate().map_err(|error| {
                preview_port_error("NATIVE_PREVIEW_ARTIFACT_INVALID", error.to_string())
            })?;
            let decision_value = serde_json::to_value(&provenance.decision).map_err(|_| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Formal preview decision could not be inspected.",
                )
            })?;
            let preview = provenance.decision.preview.as_ref().ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Formal preview provenance requires a passed preview decision.",
                )
            })?;
            if provenance.schema_version != NATIVE_SINGLE_RESULT_PROVENANCE_SCHEMA_VERSION
                || !is_stable_id(&provenance.project_id)
                || !is_stable_id(&provenance.plan_id)
                || !is_stable_id(&provenance.direction_id)
                || !supported_domain(&provenance.domain_pack_id)
                || provenance.decision.project_id != provenance.project_id
                || provenance.decision.turn_id != self.turn_id
                || preview.preview_id != self.preview_id
                || preview.artifact_sha256 != self.glb_sha256
                || preview.artifact_profile_id != self.readback.artifact_profile_id
                || provenance.decision_sha256
                    != sha256_hex(canonical_json(&decision_value).as_bytes())
            {
                return Err(preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Formal preview provenance does not match its trusted Project, Turn, decision, or GLB.",
                ));
            }
        }
        if self.recipe_assembly_graph.is_some() != self.recipe_component_instances.is_some()
            || self.recipe_assembly_graph.is_some() != self.recipe_candidate_sha256.is_some()
            || self.recipe_assembly_graph.is_some()
                != self.recipe_expanded_shape_program_sha256.is_some()
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe preview evidence must be complete or absent.",
            ));
        }
        if let (Some(graph), Some(instances), Some(candidate_sha256), Some(shape_program_sha256)) = (
            self.recipe_assembly_graph.as_ref(),
            self.recipe_component_instances.as_ref(),
            self.recipe_candidate_sha256.as_deref(),
            self.recipe_expanded_shape_program_sha256.as_deref(),
        ) {
            validate_recipe_preview_evidence(
                &self.shape_program,
                &self.assembly,
                graph,
                instances,
                candidate_sha256,
                shape_program_sha256,
            )?;
        }
        Ok(())
    }

    fn retained_bytes(&self) -> usize {
        self.glb_bytes.len()
            + self.views.values().map(Vec::len).sum::<usize>()
            + canonical_json(&self.shape_program).len()
            + self
                .formal_provenance
                .as_ref()
                .and_then(|value| serde_json::to_value(value).ok())
                .map(|value| canonical_json(&value).len())
                .unwrap_or_default()
            + self
                .recipe_assembly_graph
                .as_ref()
                .map(|value| canonical_json(value).len())
                .unwrap_or_default()
            + self
                .recipe_component_instances
                .as_ref()
                .map(|value| canonical_json(value).len())
                .unwrap_or_default()
            + self
                .recipe_candidate_sha256
                .as_ref()
                .map(String::len)
                .unwrap_or_default()
            + self
                .recipe_expanded_shape_program_sha256
                .as_ref()
                .map(String::len)
                .unwrap_or_default()
    }
}

fn build_native_preview_artifact(
    turn_id: &str,
    state: &NativeToolState,
    ttl: Duration,
    formal_v003_preview: bool,
) -> Result<NativePreviewArtifact, NativeToolFailure> {
    let preview = state.preview.as_ref().ok_or_else(|| {
        NativeToolFailure::conflict(
            "NATIVE_PREVIEW_DESCRIPTOR_REQUIRED",
            "A validated preview descriptor is required before retaining preview bytes.",
        )
    })?;
    let preview_id = preview
        .get("preview_id")
        .and_then(Value::as_str)
        .filter(|value| value.starts_with("preview_") && is_stable_id(value))
        .ok_or_else(|| {
            NativeToolFailure::schema(
                "NATIVE_PREVIEW_DESCRIPTOR_INVALID",
                "Preview descriptor identity is missing or invalid.",
            )
        })?;
    let expanded = state.expanded_geometry.as_ref().ok_or_else(|| {
        NativeToolFailure::conflict(
            "ACTION_LOOP_SHAPE_PROGRAM_REQUIRED",
            "Expanded ShapeProgram is required before retaining preview bytes.",
        )
    })?;
    expanded.validate().map_err(native_failure_from_geometry)?;
    let geometry = state.geometry.as_ref().ok_or_else(|| {
        NativeToolFailure::conflict(
            "ACTION_LOOP_BUILD_REQUIRED",
            "Restricted geometry output is required before retaining preview bytes.",
        )
    })?;
    geometry
        .validate(expanded)
        .map_err(native_failure_from_geometry)?;

    let created_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| {
            NativeToolFailure::new(
                ProductToolFailureCategory::Execution,
                "NATIVE_PREVIEW_CLOCK_INVALID",
                "System time cannot establish the bounded preview lifetime.",
            )
        })?
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let ttl_ms = ttl.as_millis().min(u128::from(u64::MAX)) as u64;
    let expires_at_unix_ms = created_at_unix_ms.checked_add(ttl_ms).ok_or_else(|| {
        NativeToolFailure::new(
            ProductToolFailureCategory::Execution,
            "NATIVE_PREVIEW_CLOCK_INVALID",
            "Preview expiration exceeds the bounded clock range.",
        )
    })?;
    let formal_provenance = match (formal_v003_preview, state.project_id.as_deref()) {
        (false, _) => None,
        (true, Some(project_id)) => {
            let plan = state.plan.as_ref().ok_or_else(|| {
                NativeToolFailure::conflict(
                    "NATIVE_FORMAL_PREVIEW_PLAN_REQUIRED",
                    "A trusted formal preview requires its accepted concept plan.",
                )
            })?;
            let plan_id = plan.get("plan_id").and_then(Value::as_str).ok_or_else(|| {
                NativeToolFailure::schema(
                    "NATIVE_FORMAL_PREVIEW_PLAN_INVALID",
                    "A trusted formal preview requires a stable plan identity.",
                )
            })?;
            let domain_pack_id = plan
                .get("domain_pack_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    NativeToolFailure::schema(
                        "NATIVE_FORMAL_PREVIEW_PLAN_INVALID",
                        "A trusted formal preview requires its domain identity.",
                    )
                })?;
            let direction_id = state
                .build
                .as_ref()
                .and_then(|value| value.get("direction_id"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    NativeToolFailure::conflict(
                        "NATIVE_FORMAL_PREVIEW_DIRECTION_REQUIRED",
                        "A trusted formal preview requires its single build direction.",
                    )
                })?;
            let decision_value = preview
                .get("single_result_decision")
                .filter(|value| !value.is_null())
                .cloned()
                .ok_or_else(|| {
                    NativeToolFailure::conflict(
                        "NATIVE_FORMAL_PREVIEW_DECISION_REQUIRED",
                        "A trusted formal preview requires a passed single-result decision.",
                    )
                })?;
            let decision: SingleResultDecision = serde_json::from_value(decision_value.clone())
                .map_err(|_| {
                    NativeToolFailure::schema(
                        "NATIVE_FORMAL_PREVIEW_DECISION_INVALID",
                        "The formal single-result decision is invalid.",
                    )
                })?;
            Some(NativeSingleResultProvenance {
                schema_version: NATIVE_SINGLE_RESULT_PROVENANCE_SCHEMA_VERSION.into(),
                project_id: project_id.to_string(),
                plan_id: plan_id.to_string(),
                direction_id: direction_id.to_string(),
                domain_pack_id: domain_pack_id.to_string(),
                decision_sha256: sha256_hex(canonical_json(&decision_value).as_bytes()),
                decision,
            })
        }
        (true, None) => None,
    };
    let artifact = NativePreviewArtifact {
        schema_version: NATIVE_PREVIEW_ARTIFACT_SCHEMA_VERSION.into(),
        preview_id: preview_id.to_string(),
        turn_id: turn_id.to_string(),
        formal_provenance,
        shape_program: expanded.shape_program.clone(),
        assembly: preview_assembly_from_shape_program(&expanded.shape_program)?,
        recipe_assembly_graph: state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.expanded_assembly_graph.clone()),
        recipe_component_instances: state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.component_recipe_instances.clone()),
        recipe_candidate_sha256: state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.candidate_sha256.clone()),
        recipe_expanded_shape_program_sha256: state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.expanded_shape_program_sha256.clone()),
        glb_bytes: geometry.glb_bytes.clone(),
        glb_sha256: geometry.glb_sha256.clone(),
        readback: geometry.readback.clone(),
        views: geometry.views.clone(),
        view_sha256: geometry.view_sha256.clone(),
        renderer_id: geometry.renderer_id.clone(),
        created_at_unix_ms,
        expires_at_unix_ms,
    };
    artifact.validate().map_err(|error| {
        NativeToolFailure::new(
            ProductToolFailureCategory::Execution,
            error.code,
            error.message,
        )
    })?;
    Ok(artifact)
}

fn preview_assembly_from_shape_program(
    shape_program: &Value,
) -> Result<NativePreviewAssemblyFacts, NativeToolFailure> {
    validate_shape_program_value(shape_program)?;
    let program = shape_program.as_object().ok_or_else(|| {
        NativeToolFailure::schema(
            "NATIVE_PREVIEW_ASSEMBLY_INVALID",
            "Preview ShapeProgram is not an object.",
        )
    })?;
    let operations = program
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeToolFailure::schema(
                "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                "Preview ShapeProgram has no operations.",
            )
        })?;
    let mut operation_args = BTreeMap::new();
    for operation in operations {
        let operation = operation.as_object().ok_or_else(|| {
            NativeToolFailure::schema(
                "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                "Preview operation is not an object.",
            )
        })?;
        let operation_id = operation
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeToolFailure::schema(
                    "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                    "Preview operation has no identity.",
                )
            })?;
        let args = operation
            .get("args")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                NativeToolFailure::schema(
                    "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                    "Preview operation has no bounded arguments.",
                )
            })?;
        operation_args.insert(operation_id.to_string(), args);
    }

    let outputs = program
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            NativeToolFailure::schema(
                "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                "Preview ShapeProgram has no outputs.",
            )
        })?;
    let mut parts = Vec::with_capacity(outputs.len());
    for output in outputs {
        let output = output.as_object().ok_or_else(|| {
            NativeToolFailure::schema(
                "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                "Preview output is not an object.",
            )
        })?;
        let output_id = output
            .get("output_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeToolFailure::schema(
                    "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                    "Preview output has no identity.",
                )
            })?;
        let operation_id = output
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeToolFailure::schema(
                    "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                    "Preview output has no operation identity.",
                )
            })?;
        let part_role = output
            .get("part_role")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeToolFailure::schema(
                    "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                    "Preview output has no part role.",
                )
            })?;
        let args = operation_args.get(operation_id).ok_or_else(|| {
            NativeToolFailure::schema(
                "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                "Preview output references no operation facts.",
            )
        })?;
        parts.push(NativePreviewPartFact {
            part_id: format!("part_{}", &sha256_hex(output_id.as_bytes())[..24]),
            output_id: output_id.to_string(),
            operation_id: operation_id.to_string(),
            part_role: part_role.to_string(),
            material_zone_id: args
                .get("zone_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            material_id: args
                .get("material_id")
                .and_then(Value::as_str)
                .map(str::to_string),
        });
    }
    let root_part_id = parts
        .first()
        .map(|part| part.part_id.clone())
        .ok_or_else(|| {
            NativeToolFailure::schema(
                "NATIVE_PREVIEW_ASSEMBLY_INVALID",
                "Preview assembly must contain at least one part.",
            )
        })?;
    Ok(NativePreviewAssemblyFacts {
        schema_version: NATIVE_PREVIEW_ASSEMBLY_SCHEMA_VERSION.into(),
        assembly_id: format!(
            "assembly_{}",
            &sha256_hex(canonical_json(shape_program).as_bytes())[..24]
        ),
        root_part_id,
        parts,
    })
}

fn validate_preview_assembly(
    shape_program: &Value,
    assembly: &NativePreviewAssemblyFacts,
) -> Result<(), ProductToolPortError> {
    let expected = preview_assembly_from_shape_program(shape_program).map_err(|failure| {
        preview_port_error("NATIVE_PREVIEW_ASSEMBLY_INVALID", failure.message)
    })?;
    if assembly != &expected {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ASSEMBLY_INVALID",
            "Preview assembly facts do not match the validated ShapeProgram outputs.",
        ));
    }
    Ok(())
}

/// The rendered-output contract is selected only after immutable Recipe
/// provenance has been classified.  In particular, the robotic-arm Domain Pack
/// is shared by the frozen C105 catalog and C106, so the domain string alone is
/// never sufficient authority for C106's semantic-component/multi-output rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipePreviewOutputContract {
    C105OneToOne,
    C106ArmSemanticComponents,
    C110GParallelLinkComponents,
}

pub fn recipe_preview_shape_program_role<'a>(
    contract: RecipePreviewOutputContract,
    component_role: &'a str,
) -> &'a str {
    match (contract, component_role) {
        (RecipePreviewOutputContract::C106ArmSemanticComponents, "link_armor") => "upper_link_form",
        (RecipePreviewOutputContract::C106ArmSemanticComponents, "joint_housing" | "turntable") => {
            "secondary_form"
        }
        (RecipePreviewOutputContract::C106ArmSemanticComponents, "cable_harness") => {
            "visual_detail"
        }
        (RecipePreviewOutputContract::C106ArmSemanticComponents, "surface_trim") => "trim",
        _ => component_role,
    }
}

/// Classify a complete Recipe provenance carrier without allowing a malformed
/// C106-looking carrier to fall back to C105.  The `recipe_c106_arm_` prefix is
/// used only as a rejection marker; authorization always requires the exact
/// registry hash and exact reviewed Recipe allow-lists below.
pub fn recipe_preview_output_contract(
    instances: &Value,
) -> Result<RecipePreviewOutputContract, ProductToolPortError> {
    let typed: Vec<ComponentRecipeInstanceProvenance> = serde_json::from_value(instances.clone())
        .map_err(|_| {
        preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe preview instance provenance does not match its closed typed contract.",
        )
    })?;
    if typed.is_empty() {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe preview instance provenance is empty.",
        ));
    }

    let c110g_registry = RecipeRegistry::from_embedded_c110g_parallel_link().map_err(|error| {
        preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            format!("The reviewed C110G registry is unavailable: {error}"),
        )
    })?;
    let c110g_registry_sha256 = c110g_registry.registry_sha256();
    const C110G_RECIPE_IDS: [&str; 4] = [
        "recipe_c110g_parallel_rail",
        "recipe_c110g_parallel_carriage",
        "recipe_c110g_parallel_link",
        "recipe_c110g_parallel_end_effector",
    ];
    let c110g_recipe_ids = [
        "recipe_c110g_parallel_link_root",
        C110G_RECIPE_IDS[0],
        C110G_RECIPE_IDS[1],
        C110G_RECIPE_IDS[2],
        C110G_RECIPE_IDS[3],
    ];
    let has_c110g_marker = typed.iter().any(|instance| {
        instance.registry_sha256 == c110g_registry_sha256
            || c110g_recipe_ids.contains(&instance.recipe.recipe_id.as_str())
    });
    if has_c110g_marker {
        let mut instance_ids = BTreeSet::new();
        let mut instance_paths = BTreeSet::new();
        let mut recipe_counts = BTreeMap::<&str, usize>::new();
        let mut root_recipe_ids = Vec::new();
        for instance in &typed {
            let recipe_id = instance.recipe.recipe_id.as_str();
            let reviewed_recipe = c110g_registry.recipe(recipe_id).ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "C110G provenance names a Recipe outside the exact reviewed allow-list.",
                )
            })?;
            let reviewed_recipe_sha256 =
                RecipeValidator::recipe_sha256(reviewed_recipe).map_err(|error| {
                    preview_port_error(
                        "NATIVE_PREVIEW_ARTIFACT_INVALID",
                        format!("The reviewed C110G Recipe identity is unavailable: {error}"),
                    )
                })?;
            let source_valid = instance.source.get("source_kind").and_then(Value::as_str)
                == Some("forgecad_first_party")
                && instance.source.get("source_id").and_then(Value::as_str)
                    == Some("source_c110g_parallel_link");
            let license_valid = instance.license.get("license_id").and_then(Value::as_str)
                == Some("ForgeCAD-Internal-Visual-Only")
                && instance
                    .license
                    .get("redistributable")
                    .and_then(Value::as_bool)
                    == Some(false);
            let review_valid = instance
                .review_state
                .get("reviewer_kind")
                .and_then(Value::as_str)
                == Some("forgecad_internal");
            if instance.schema_version != "ComponentRecipeInstanceProvenance@1"
                || instance.policy_version != "ComponentRecipePolicy@1"
                || instance.registry_sha256 != c110g_registry_sha256
                || instance.domain_pack_id != "pack_robotic_arm_concept"
                || !c110g_recipe_ids.contains(&recipe_id)
                || instance.recipe.version != reviewed_recipe.version
                || instance.recipe.recipe_sha256 != reviewed_recipe_sha256
                || !source_valid
                || !license_valid
                || !review_valid
                || instance.quality_status != "passed"
                || !instance.non_functional_only
                || !instance_ids.insert(instance.instance_id.as_str())
                || !instance_paths.insert(instance.instance_path.as_str())
            {
                return Err(preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "C110G provenance is mixed, stale, unreviewed, or outside its visual-only registry contract.",
                ));
            }
            *recipe_counts.entry(recipe_id).or_default() += 1;
            if instance.instance_path == "root"
                || instance.parent_instance_id.is_none()
                || instance.parent_slot_id.is_none()
            {
                if instance.instance_path != "root"
                    || instance.parent_instance_id.is_some()
                    || instance.parent_slot_id.is_some()
                {
                    return Err(preview_port_error(
                        "NATIVE_PREVIEW_ARTIFACT_INVALID",
                        "C110G provenance has an ambiguous root or orphan component.",
                    ));
                }
                root_recipe_ids.push(recipe_id);
            }
        }
        if typed.len() != 6
            || root_recipe_ids != vec!["recipe_c110g_parallel_link_root"]
            || recipe_counts.get("recipe_c110g_parallel_link_root") != Some(&1)
            || recipe_counts.get("recipe_c110g_parallel_rail") != Some(&2)
            || recipe_counts.get("recipe_c110g_parallel_carriage") != Some(&1)
            || recipe_counts.get("recipe_c110g_parallel_link") != Some(&1)
            || recipe_counts.get("recipe_c110g_parallel_end_effector") != Some(&1)
            || recipe_counts.len() != 5
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "C110G provenance does not contain exactly one reviewed six-instance parallel-link assembly.",
            ));
        }
        return Ok(RecipePreviewOutputContract::C110GParallelLinkComponents);
    }

    let registry = RecipeRegistry::from_embedded_c106_robotic_arm().map_err(|error| {
        preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            format!("The reviewed C106 registry is unavailable: {error}"),
        )
    })?;
    let registry_sha256 = registry.registry_sha256();
    let has_c106_marker = typed.iter().any(|instance| {
        instance.registry_sha256 == registry_sha256
            || C106_ARM_RECIPE_IDS.contains(&instance.recipe.recipe_id.as_str())
            || instance.recipe.recipe_id.starts_with("recipe_c106_arm_")
    });
    if !has_c106_marker {
        return Ok(RecipePreviewOutputContract::C105OneToOne);
    }

    let mut instance_ids = BTreeSet::new();
    let mut instance_paths = BTreeSet::new();
    let mut recipe_counts = BTreeMap::<&str, usize>::new();
    let mut root_recipe_ids = Vec::new();
    for instance in &typed {
        let recipe_id = instance.recipe.recipe_id.as_str();
        let reviewed_recipe = registry.recipe(recipe_id).ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "C106 provenance names a Recipe outside the exact reviewed allow-list.",
            )
        })?;
        let reviewed_recipe_sha256 =
            RecipeValidator::recipe_sha256(reviewed_recipe).map_err(|error| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    format!("The reviewed C106 Recipe identity is unavailable: {error}"),
                )
            })?;
        let source_valid = instance.source.get("source_kind").and_then(Value::as_str)
            == Some("forgecad_first_party")
            && instance.source.get("source_id").and_then(Value::as_str) == Some("source_c106_arm");
        let license_valid = instance.license.get("license_id").and_then(Value::as_str)
            == Some("ForgeCAD-Internal-Visual-Only")
            && instance
                .license
                .get("redistributable")
                .and_then(Value::as_bool)
                == Some(false);
        let review_valid = instance
            .review_state
            .get("reviewer_kind")
            .and_then(Value::as_str)
            == Some("forgecad_internal");
        if instance.schema_version != "ComponentRecipeInstanceProvenance@1"
            || instance.policy_version != "ComponentRecipePolicy@1"
            || instance.registry_sha256 != registry_sha256
            || instance.domain_pack_id != "pack_robotic_arm_concept"
            || !C106_ARM_RECIPE_IDS.contains(&recipe_id)
            || instance.recipe.version != reviewed_recipe.version
            || instance.recipe.recipe_sha256 != reviewed_recipe_sha256
            || !source_valid
            || !license_valid
            || !review_valid
            || instance.quality_status != "passed"
            || !instance.non_functional_only
            || !instance_ids.insert(instance.instance_id.as_str())
            || !instance_paths.insert(instance.instance_path.as_str())
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "C106 provenance is mixed, stale, unreviewed, or outside its visual-only registry contract.",
            ));
        }
        *recipe_counts.entry(recipe_id).or_default() += 1;
        if instance.instance_path == "root"
            || instance.parent_instance_id.is_none()
            || instance.parent_slot_id.is_none()
        {
            if instance.instance_path != "root"
                || instance.parent_instance_id.is_some()
                || instance.parent_slot_id.is_some()
            {
                return Err(preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "C106 provenance has an ambiguous root or orphan component.",
                ));
            }
            root_recipe_ids.push(recipe_id);
        }
    }

    if typed.len() != 10
        || root_recipe_ids.len() != 1
        || !C106_ARM_ROOT_RECIPE_IDS.contains(&root_recipe_ids[0])
        || recipe_counts.get(root_recipe_ids[0]) != Some(&1)
        || recipe_counts.get("recipe_c106_arm_turntable") != Some(&1)
        || recipe_counts.get("recipe_c106_arm_joint_housing") != Some(&3)
        || recipe_counts.get("recipe_c106_arm_link_armor") != Some(&2)
        || recipe_counts.get("recipe_c106_arm_cable_harness") != Some(&1)
        || recipe_counts.get("recipe_c106_arm_gripper") != Some(&1)
        || recipe_counts.get("recipe_c106_arm_surface_trim") != Some(&1)
        || recipe_counts.len() != 7
    {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "C106 provenance does not contain exactly one reviewed ten-component root assembly.",
        ));
    }
    Ok(RecipePreviewOutputContract::C106ArmSemanticComponents)
}

/// Validate the Rust-only C105 carrier before it is retained for confirmation.
///
/// The AssemblyGraph uses its own `part_id` namespace, so identity is bound by
/// the exact ShapeProgram `operation_id` + `output_id` pair rather than by a
/// coincidental preview part hash.  This rejects a graph that merely looks
/// schema-valid but is disconnected from the rendered candidate.
fn validate_recipe_preview_evidence(
    shape_program: &Value,
    assembly: &NativePreviewAssemblyFacts,
    graph: &Value,
    instances: &Value,
    candidate_sha256: &str,
    expanded_shape_program_sha256: &str,
) -> Result<(), ProductToolPortError> {
    if !is_sha256(candidate_sha256) || !is_sha256(expanded_shape_program_sha256) {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe candidate and ShapeProgram identities must be lowercase SHA-256 digests.",
        ));
    }
    if expanded_shape_program_sha256 != sha256_hex(canonical_json(shape_program).as_bytes()) {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe ShapeProgram identity does not match the compiled preview.",
        ));
    }
    let graph = graph.as_object().ok_or_else(|| {
        preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe assembly evidence is not an AssemblyGraph object.",
        )
    })?;
    if graph.get("schema_version").and_then(Value::as_str) != Some("AssemblyGraph@1") {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe assembly evidence is not an AssemblyGraph.",
        ));
    }
    let graph_parts = graph
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe AssemblyGraph has no bounded parts array.",
            )
        })?;
    let graph_instances = graph
        .get("component_recipe_instances")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe AssemblyGraph has no instance provenance.",
            )
        })?;
    let output_contract = recipe_preview_output_contract(instances)?;
    let c106_arm_semantic_components =
        output_contract == RecipePreviewOutputContract::C106ArmSemanticComponents;
    let instances = instances.as_array().ok_or_else(|| {
        preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe preview instance provenance is not an array.",
        )
    })?;
    let instance_domains = graph_instances
        .iter()
        .map(|instance| instance.get("domain_pack_id").and_then(Value::as_str))
        .collect::<Option<BTreeSet<_>>>()
        .ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe instance provenance has no domain identity.",
            )
        })?;
    if instance_domains.len() != 1 {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe preview may contain only one reviewed Domain Pack.",
        ));
    }
    if graph_parts.is_empty()
        || graph_instances.len() != graph_parts.len()
        || instances != graph_instances
        || (!c106_arm_semantic_components && graph_parts.len() != assembly.parts.len())
    {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe graph parts and instance provenance do not match the preview assembly.",
        ));
    }

    let mut preview_by_binding = BTreeMap::new();
    for part in &assembly.parts {
        if preview_by_binding
            .insert((part.operation_id.as_str(), part.output_id.as_str()), part)
            .is_some()
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Preview assembly has duplicate ShapeProgram output bindings.",
            ));
        }
    }
    let root_binding = assembly
        .parts
        .first()
        .map(|part| (part.operation_id.as_str(), part.output_id.as_str()))
        .ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe preview assembly has no root output.",
            )
        })?;
    let root_part_id = graph
        .get("root_part_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe AssemblyGraph has no root part identity.",
            )
        })?;
    let mut graph_bindings = BTreeSet::new();
    let mut graph_part_ids = BTreeSet::new();
    let mut semantic_component_outputs = Vec::new();
    let mut root_matches_first_output = false;
    for graph_part in graph_parts {
        let graph_part = graph_part.as_object().ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe AssemblyGraph contains a non-object part.",
            )
        })?;
        let part_id = graph_part
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Recipe graph part has no identity.",
                )
            })?;
        let operation_id = graph_part
            .get("operation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Recipe graph part has no operation binding.",
                )
            })?;
        let output_id = graph_part
            .get("output_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Recipe graph part has no output binding.",
                )
            })?;
        let binding = (operation_id, output_id);
        let preview_part = preview_by_binding.get(&binding).ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe graph part does not map to a rendered ShapeProgram output.",
            )
        })?;
        let graph_role = graph_part
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Recipe graph part has no semantic role.",
                )
            })?;
        if !graph_bindings.insert(binding)
            || !graph_part_ids.insert(part_id)
            || recipe_preview_shape_program_role(output_contract, graph_role)
                != preview_part.part_role.as_str()
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe graph binding or role is not one-to-one with the preview assembly.",
            ));
        }
        let material_zones = graph_part
            .get("material_zones")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Recipe graph part has no material zones.",
                )
            })?;
        let material_zone_ids = graph_part
            .get("material_zone_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Recipe graph part has no material zone IDs.",
                )
            })?;
        if material_zones != material_zone_ids {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe graph material zone aliases disagree.",
            ));
        }
        let zones = material_zone_ids
            .iter()
            .map(|value| value.as_str())
            .collect::<Option<BTreeSet<_>>>()
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Recipe graph material zone is invalid.",
                )
            })?;
        if zones.len() != material_zone_ids.len()
            || preview_part
                .material_zone_id
                .as_deref()
                .is_none_or(|zone| !zones.contains(zone))
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe graph material zones do not include the rendered output zone.",
            ));
        }
        if c106_arm_semantic_components {
            let recipe_instance_id = graph_part
                .get("recipe_instance_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    preview_port_error(
                        "NATIVE_PREVIEW_ARTIFACT_INVALID",
                        "C106 Recipe graph part has no instance identity.",
                    )
                })?;
            let instance_suffix =
                recipe_instance_id
                    .strip_prefix("recipeinst_")
                    .ok_or_else(|| {
                        preview_port_error(
                            "NATIVE_PREVIEW_ARTIFACT_INVALID",
                            "C106 Recipe graph part has an invalid instance identity.",
                        )
                    })?;
            semantic_component_outputs.push((
                format!("op_{instance_suffix}_"),
                recipe_preview_shape_program_role(output_contract, graph_role).to_string(),
                zones,
            ));
        }
        if binding == root_binding && part_id == root_part_id {
            root_matches_first_output = true;
        }
    }
    let every_preview_output_has_c106_component = !c106_arm_semantic_components
        || assembly.parts.iter().all(|preview_part| {
            semantic_component_outputs
                .iter()
                .any(|(operation_prefix, output_role, zones)| {
                    preview_part.operation_id.starts_with(operation_prefix)
                        && preview_part.part_role == *output_role
                        && preview_part
                            .material_zone_id
                            .as_deref()
                            .is_some_and(|zone| zones.contains(zone))
                })
        });
    if (!c106_arm_semantic_components && graph_bindings.len() != preview_by_binding.len())
        || !every_preview_output_has_c106_component
        || !root_matches_first_output
    {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ARTIFACT_INVALID",
            "Recipe graph root or output bindings do not match the rendered preview.",
        ));
    }
    let mut instance_ids = BTreeSet::new();
    for instance in instances {
        let instance = instance.as_object().ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe instance provenance is not an object.",
            )
        })?;
        if instance.get("schema_version").and_then(Value::as_str)
            != Some("ComponentRecipeInstanceProvenance@1")
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe instance provenance has an unsupported schema version.",
            ));
        }
        let instance_id = instance
            .get("instance_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_INVALID",
                    "Recipe instance provenance has no identity.",
                )
            })?;
        let suffix = instance_id.strip_prefix("recipeinst_").ok_or_else(|| {
            preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe instance identity is invalid.",
            )
        })?;
        let expected_part_id = format!("part_{suffix}");
        if !instance_ids.insert(instance_id) || !graph_part_ids.contains(expected_part_id.as_str())
        {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_ARTIFACT_INVALID",
                "Recipe instance provenance is not represented by an AssemblyGraph part.",
            ));
        }
    }
    Ok(())
}

fn preview_port_error(code: impl Into<String>, message: impl Into<String>) -> ProductToolPortError {
    port_error(
        code,
        ProductToolPortErrorKind::InvalidResponse,
        message,
        false,
    )
}

fn validate_preview_id(preview_id: &str) -> Result<(), ProductToolPortError> {
    if !preview_id.starts_with("preview_") || !is_stable_id(preview_id) {
        return Err(preview_port_error(
            "NATIVE_PREVIEW_ID_INVALID",
            "Preview identity must be one bounded stable preview ID.",
        ));
    }
    Ok(())
}

impl RestrictedGeometryOutput {
    pub fn validate(&self, input: &RestrictedGeometryInput) -> Result<(), RestrictedGeometryError> {
        let quality = &input.quality_profile;
        let expected_program_sha256 = sha256_hex(canonical_json(&input.shape_program).as_bytes());
        if self.schema_version != RESTRICTED_GEOMETRY_OUTPUT_SCHEMA_VERSION
            || self.glb_bytes.is_empty()
            || self.glb_bytes.len() > MAX_GLTF_BYTES
            || self.glb_sha256 != sha256_hex(&self.glb_bytes)
            || !is_sha256(&self.topology_hash)
            || self.readback.runtime_manifest_version
                != RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION
            || self.readback.artifact_profile_id != quality.profile_id
            || self.readback.shape_program_sha256 != expected_program_sha256
            || self.readback.glb_sha256 != self.glb_sha256
            || self.readback.glb_byte_size != self.glb_bytes.len() as u64
            || self.readback.triangle_count == 0
            || self.readback.triangle_count > quality.max_triangle_count
            || self.readback.mesh_count == 0
            || self.readback.primitive_count == 0
            || self.readback.material_count == 0
            || !is_sha256(&self.readback.compile_readback_sha256)
            || self.readback.material_zone_count == 0
            || self.readback.material_zone_count > 512
            || self.readback.visual_texture_set_count > 64
            || self.readback.visual_texture_map_count > 320
            || !self
                .readback
                .bounds_mm
                .iter()
                .all(|value| value.is_finite() && *value > 0.0)
            || self.renderer_id.is_empty()
            || self.renderer_id.len() > 120
        {
            return Err(restricted_output_error(
                "Restricted geometry output failed GLB/readback integrity validation.",
            ));
        }
        // `closed_manifold` and `surface_provenance_present` are quality
        // facts, not byte-integrity facts. V003 must retain a structurally
        // valid failed candidate long enough to issue a typed hard-gate
        // report and, only when authorized, run one bounded in-place repair.
        // They are still mandatory in `evaluate_candidate` and a failed fact
        // can never reach `prepare_candidate_preview` or persistence.
        let view_names = self
            .views
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let hash_names = self
            .view_sha256
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let required = REQUIRED_VIEWS.into_iter().collect::<BTreeSet<_>>();
        if view_names != required || hash_names != required {
            return Err(restricted_output_error(
                "Restricted geometry output must contain exactly four deterministic views.",
            ));
        }
        for (name, bytes) in &self.views {
            if bytes.is_empty()
                || bytes.len() > MAX_VIEW_BYTES
                || self.view_sha256.get(name) != Some(&sha256_hex(bytes))
            {
                return Err(restricted_output_error(
                    "Restricted geometry view bytes and hashes do not match.",
                ));
            }
        }
        Ok(())
    }
}

fn restricted_input_error(message: impl Into<String>) -> RestrictedGeometryError {
    RestrictedGeometryError {
        code: "RESTRICTED_GEOMETRY_INPUT_INVALID".into(),
        kind: RestrictedGeometryErrorKind::InvalidInput,
        message: message.into(),
        recoverable: false,
    }
}

fn restricted_output_error(message: impl Into<String>) -> RestrictedGeometryError {
    RestrictedGeometryError {
        code: "RESTRICTED_GEOMETRY_OUTPUT_INVALID".into(),
        kind: RestrictedGeometryErrorKind::Execution,
        message: message.into(),
        recoverable: false,
    }
}

fn validate_geometry_companion_binding(
    shape_program: &Value,
    profile_sketch: Option<&Value>,
    section_set: Option<&Value>,
) -> Result<(), RestrictedGeometryError> {
    let profile_inputs = shape_program
        .get("profile_inputs")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for (companion, kind, contract) in [
        (profile_sketch, "profile_sketch", "ProfileSketch@1"),
        (section_set, "profile_section_set", "ProfileSectionSet@1"),
    ] {
        let Some(companion) = companion else {
            continue;
        };
        let digest = sha256_hex(canonical_json(companion).as_bytes());
        let bound = profile_inputs.iter().any(|entry| {
            entry.get("input_kind").and_then(Value::as_str) == Some(kind)
                && entry.get("contract_version").and_then(Value::as_str) == Some(contract)
                && entry.get("input_sha256").and_then(Value::as_str) == Some(&digest)
                && entry.get("canonical_payload") == Some(companion)
        });
        if !bound {
            return Err(RestrictedGeometryError {
                code: "GEOMETRY_PROFILE_COMPANION_UNBOUND".into(),
                kind: RestrictedGeometryErrorKind::InvalidInput,
                message: "Geometry profile companion is not canonically bound by ShapeProgram.profile_inputs.".into(),
                recoverable: false,
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewedCatalogRequest {
    pub domain_pack_id: String,
    pub direction_id: String,
    pub variant_id: Option<String>,
    pub presentation_profile: String,
    pub plan: Value,
    pub style_recipe: Option<Value>,
    pub authored_profile_sketch: Option<Value>,
    pub authored_shape_program: Option<Value>,
    /// This is selected by the Rust plan normalizer, never inferred from the
    /// incidental presence of a prior `author_shape_program` tool call.
    pub geometry_strategy: GeometryStrategy,
}

/// The bounded geometry route for one native Product Tool execution.
///
/// Recipe-backed expansion is the default.  An authored ShapeProgram is only
/// allowed when the reviewed plan explicitly sets `shape_program_ready=true`;
/// this keeps the normal Action Loop from accidentally bypassing C105 merely
/// because it validated a draft ShapeProgram before planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryStrategy {
    ComponentRecipe,
    AuthoredShapeProgram,
}

impl Default for GeometryStrategy {
    fn default() -> Self {
        Self::ComponentRecipe
    }
}

/// Code-owned expansion hook.  Implementations run in Rust and may never
/// delegate a MechanicalConceptPlan, Style Token, or Recipe to the worker.
pub trait ReviewedShapeProgramCatalog: Send + Sync + 'static {
    fn expand(
        &self,
        request: &ReviewedCatalogRequest,
    ) -> Result<ReviewedCatalogExpansion, ReviewedCatalogError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewedCatalogExpansion {
    pub geometry_input: RestrictedGeometryInput,
    pub expanded_assembly_graph: Option<Value>,
    pub component_recipe_instances: Option<Value>,
    pub candidate_sha256: Option<String>,
    /// Hash of the exact, quality-derived ShapeProgram delivered to Python.
    /// This prevents a Recipe candidate identity from silently covering the
    /// pre-derivation program while the worker compiles a different budget.
    pub expanded_shape_program_sha256: Option<String>,
    /// Code-owned proof that the selected semantic proportion recipe changed
    /// the exact ShapeProgram delivered to the restricted compiler.  This is
    /// kept outside the worker DTO and is rechecked by V003 before preview.
    semantic_proportion_binding: Option<SemanticProportionBinding>,
    /// Code-owned proof that an ArmDesignIntent changed a reviewed serial
    /// chain ShapeProgram and AssemblyGraph together.
    arm_geometry_binding: Option<forgecad_core::ArmGeometryFamilyBinding>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct SemanticProportionBinding {
    style_recipe_sha256: String,
    role_selector: String,
    path: String,
    step_delta: i32,
    shape_program_sha256: String,
}

impl ReviewedCatalogExpansion {
    fn validate(&self) -> Result<(), ReviewedCatalogError> {
        self.geometry_input
            .validate()
            .map_err(|error| ReviewedCatalogError::new(error.code, error.message))?;
        let recipe_evidence_present = self.expanded_assembly_graph.is_some()
            || self.component_recipe_instances.is_some()
            || self.candidate_sha256.is_some()
            || self.expanded_shape_program_sha256.is_some();
        if !recipe_evidence_present {
            if self.semantic_proportion_binding.is_some() {
                return Err(ReviewedCatalogError::new(
                    "REVIEWED_SEMANTIC_BINDING_WITHOUT_RECIPE",
                    "Semantic proportion evidence requires a reviewed Recipe expansion.",
                ));
            }
            return Ok(());
        }
        let (Some(_), Some(_), Some(candidate_sha256), Some(shape_program_sha256)) = (
            self.expanded_assembly_graph.as_ref(),
            self.component_recipe_instances.as_ref(),
            self.candidate_sha256.as_deref(),
            self.expanded_shape_program_sha256.as_deref(),
        ) else {
            return Err(ReviewedCatalogError::new(
                "REVIEWED_RECIPE_EVIDENCE_INCOMPLETE",
                "Recipe expansion evidence must be complete or absent.",
            ));
        };
        if !is_sha256(candidate_sha256) || !is_sha256(shape_program_sha256) {
            return Err(ReviewedCatalogError::new(
                "REVIEWED_RECIPE_EVIDENCE_INVALID",
                "Recipe candidate and ShapeProgram identities must be lowercase SHA-256 digests.",
            ));
        }
        let actual_shape_program_sha256 =
            sha256_hex(canonical_json(&self.geometry_input.shape_program).as_bytes());
        if shape_program_sha256 != actual_shape_program_sha256 {
            return Err(ReviewedCatalogError::new(
                "REVIEWED_RECIPE_SHAPE_IDENTITY_MISMATCH",
                "Recipe expansion ShapeProgram identity does not match the worker input.",
            ));
        }
        if let Some(binding) = self.semantic_proportion_binding.as_ref() {
            if !is_sha256(&binding.style_recipe_sha256)
                || !is_sha256(&binding.shape_program_sha256)
                || binding.shape_program_sha256 != actual_shape_program_sha256
                || !is_role_id(&binding.role_selector)
                || !matches!(
                    binding.path.as_str(),
                    "transform.scale.x" | "transform.scale.y" | "transform.scale.z"
                )
                || !(-1..=1).contains(&binding.step_delta)
                || binding.step_delta == 0
            {
                return Err(ReviewedCatalogError::new(
                    "REVIEWED_SEMANTIC_BINDING_INVALID",
                    "Semantic proportion evidence did not bind the exact expanded ShapeProgram.",
                ));
            }
        }
        if let Some(binding) = self.arm_geometry_binding.as_ref() {
            let family_architecture = match binding.family_id.as_str() {
                "robotic_arm.serial_chain.reviewed_v1" => Some("serial_chain"),
                "robotic_arm.parallel_link.c110g_v1" => Some("parallel_link"),
                _ => None,
            };
            if binding.schema_version != forgecad_core::ARM_GEOMETRY_FAMILY_SCHEMA_VERSION
                || !matches!(
                    binding.family_id.as_str(),
                    "robotic_arm.serial_chain.reviewed_v1" | "robotic_arm.parallel_link.c110g_v1"
                )
                || family_architecture != Some(binding.architecture.as_str())
                || !is_sha256(&binding.intent_sha256)
                || binding.changed_operation_count == 0
                || binding.changed_part_count == 0
                || binding.shape_program_sha256.as_deref()
                    != Some(actual_shape_program_sha256.as_str())
            {
                return Err(ReviewedCatalogError::new(
                    "REVIEWED_ARM_GEOMETRY_BINDING_INVALID",
                    "Arm geometry-family evidence did not bind the exact expanded ShapeProgram.",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedCatalogError {
    pub code: String,
    pub message: String,
}

impl ReviewedCatalogError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Minimal embedded fixture hook used until C105 installs recipe-backed
/// catalogs.  It emits a valid, non-functional exterior blockout only.
#[derive(Debug, Clone, Default)]
pub struct EmbeddedReviewedShapeProgramCatalog;

/// C105 adapter: the app-server selects one reviewed root recipe and asks the
/// Rust core to expand it.  The worker only receives the expanded IR.
#[derive(Debug, Clone, Default)]
pub struct RecipeBackedReviewedShapeProgramCatalog;

const C106_ARM_DESKTOP_ROOT_RECIPE_ID: &str = "recipe_c106_arm_desktop_assistant";
const C106_ARM_GALLERY_ROOT_RECIPE_ID: &str = "recipe_c106_arm_gallery_industrial";
const C106_ARM_SERVICE_ROOT_RECIPE_ID: &str = "recipe_c106_arm_service_display";
const C106_ARM_ROOT_RECIPE_IDS: [&str; 3] = [
    C106_ARM_DESKTOP_ROOT_RECIPE_ID,
    C106_ARM_GALLERY_ROOT_RECIPE_ID,
    C106_ARM_SERVICE_ROOT_RECIPE_ID,
];
const C106_ARM_RECIPE_IDS: [&str; 9] = [
    C106_ARM_DESKTOP_ROOT_RECIPE_ID,
    C106_ARM_GALLERY_ROOT_RECIPE_ID,
    C106_ARM_SERVICE_ROOT_RECIPE_ID,
    "recipe_c106_arm_turntable",
    "recipe_c106_arm_joint_housing",
    "recipe_c106_arm_link_armor",
    "recipe_c106_arm_cable_harness",
    "recipe_c106_arm_gripper",
    "recipe_c106_arm_surface_trim",
];
const C110G_PARALLEL_ROOT_RECIPE_ID: &str = "recipe_c110g_parallel_link_root";

/// A mechanical-arm Style Token selects one reviewed C106 root *before* the
/// single Recipe expansion.  This is deliberately a catalog lookup, not a
/// proposal fan-out or a visual ranking pass: V003 still synthesizes exactly
/// one candidate for one user turn.
fn c106_robotic_arm_root_recipe_id(
    style_recipe: Option<&Value>,
) -> Result<&'static str, ReviewedCatalogError> {
    let Some(style_recipe) = style_recipe else {
        // The bounded style selector has the same compact default.  Keeping
        // this explicit makes a direct catalog caller deterministic as well.
        return Ok(C106_ARM_DESKTOP_ROOT_RECIPE_ID);
    };
    let choice = validated_style_choice(style_recipe, "pack_robotic_arm_concept")?;
    match choice.token_id {
        "style_compact_rounded" | "style_clean_balanced" => Ok(C106_ARM_DESKTOP_ROOT_RECIPE_ID),
        "style_industrial_substantial" => Ok(C106_ARM_GALLERY_ROOT_RECIPE_ID),
        "style_aerodynamic_sleek" => Ok(C106_ARM_SERVICE_ROOT_RECIPE_ID),
        _ => Err(ReviewedCatalogError::new(
            "REVIEWED_CATALOG_STYLE_UNAVAILABLE",
            "The reviewed C106 robotic-arm catalog has no root for this Style Token.",
        )),
    }
}

impl ReviewedShapeProgramCatalog for RecipeBackedReviewedShapeProgramCatalog {
    fn expand(
        &self,
        request: &ReviewedCatalogRequest,
    ) -> Result<ReviewedCatalogExpansion, ReviewedCatalogError> {
        let quality_profile =
            RestrictedQualityProfile::for_presentation(&request.presentation_profile)
                .map_err(|failure| ReviewedCatalogError::new(failure.code, failure.message))?;
        if request.geometry_strategy == GeometryStrategy::AuthoredShapeProgram {
            let authored = request.authored_shape_program.clone().ok_or_else(|| {
                ReviewedCatalogError::new(
                    "AUTHORED_SHAPE_PROGRAM_REQUIRED",
                    "The explicit authored geometry strategy requires a validated ShapeProgram.",
                )
            })?;
            let shape_program = normalize_persisted_shape_program(&authored)
                .map_err(|error| ReviewedCatalogError::new(error.code(), error.to_string()))?;
            let input = RestrictedGeometryInput {
                schema_version: RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION.into(),
                shape_program,
                profile_sketch: request.authored_profile_sketch.clone(),
                section_set: None,
                surface_adornment_programs: Vec::new(),
                surface_layer_input: None,
                quality_profile,
            };
            input
                .validate()
                .map_err(|error| ReviewedCatalogError::new(error.code, error.message))?;
            return Ok(ReviewedCatalogExpansion {
                geometry_input: input,
                expanded_assembly_graph: None,
                component_recipe_instances: None,
                candidate_sha256: None,
                expanded_shape_program_sha256: None,
                semantic_proportion_binding: None,
                arm_geometry_binding: None,
            });
        }
        // C105 remains the catalog for every existing domain.  C106 is a
        // separate immutable mechanical-arm production catalog so persisted
        // C105 identities never drift as the arm golden path becomes richer.
        let (registry, root_recipe_id) = if request.domain_pack_id == "pack_robotic_arm_concept" {
            let architecture = request
                .plan
                .get("arm_design_intent")
                .and_then(Value::as_object)
                .and_then(|intent| intent.get("architecture"))
                .and_then(Value::as_str);
            if architecture == Some("parallel_link") {
                (
                    RecipeRegistry::from_embedded_c110g_parallel_link().map_err(|error| {
                        ReviewedCatalogError::new(
                            "REVIEWED_RECIPE_REGISTRY_INVALID",
                            error.to_string(),
                        )
                    })?,
                    C110G_PARALLEL_ROOT_RECIPE_ID.to_string(),
                )
            } else {
                (
                    RecipeRegistry::from_embedded_c106_robotic_arm().map_err(|error| {
                        ReviewedCatalogError::new(
                            "REVIEWED_RECIPE_REGISTRY_INVALID",
                            error.to_string(),
                        )
                    })?,
                    c106_robotic_arm_root_recipe_id(request.style_recipe.as_ref())?.to_string(),
                )
            }
        } else {
            let registry = RecipeRegistry::from_embedded().map_err(|error| {
                ReviewedCatalogError::new("REVIEWED_RECIPE_REGISTRY_INVALID", error.to_string())
            })?;
            let root_recipe_id = {
                let roots: Vec<_> = registry
                    .recipes()
                    .filter(|recipe| {
                        recipe
                            .allowed_domains
                            .iter()
                            .any(|domain| domain == &request.domain_pack_id)
                            && recipe.component_role != "visual_detail"
                            && !recipe.child_slots.is_empty()
                    })
                    .collect();
                if roots.len() != 1 {
                    return Err(ReviewedCatalogError::new(
                        "REVIEWED_CATALOG_DOMAIN_UNAVAILABLE",
                        "The domain does not have exactly one reviewed root recipe.",
                    ));
                }
                roots[0].recipe_id.clone()
            };
            (registry, root_recipe_id)
        };
        let root = registry.recipe(&root_recipe_id).ok_or_else(|| {
            ReviewedCatalogError::new(
                "REVIEWED_CATALOG_DOMAIN_UNAVAILABLE",
                "The selected reviewed root recipe is unavailable in its immutable catalog.",
            )
        })?;
        let recipe = ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: root.recipe_id.clone(),
            version: root.version,
            recipe_sha256: RecipeValidator::recipe_sha256(root).map_err(|error| {
                ReviewedCatalogError::new("REVIEWED_RECIPE_HASH_INVALID", error.to_string())
            })?,
        };
        let mut candidate = RecipeExpander::expand(
            &registry,
            &RecipeInstantiationRequest {
                schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                context_mode: "initial_candidate".into(),
                request_id: format!(
                    "recipereq_{}",
                    stable_seed(&format!(
                        "{}:{}",
                        request.domain_pack_id, request.direction_id
                    ))
                ),
                project_id: None,
                base_asset_version_id: None,
                snapshot_revision: None,
                domain_pack_id: request.domain_pack_id.clone(),
                recipe_registry_sha256: registry.registry_sha256().into(),
                recipe,
                target_part_id: None,
                slot_bindings: vec![],
                parameter_values: vec![],
                material_zone_overrides: vec![],
            },
            &RecipeExpansionPolicy::default(),
        )
        .map_err(|error| {
            ReviewedCatalogError::new("REVIEWED_RECIPE_EXPANSION_FAILED", error.to_string())
        })?;
        let mut shape_program = candidate.expanded_shape_program.clone();
        shape_program["triangle_budget"] = json!(quality_profile
            .max_triangle_count
            .min(MAX_SHAPE_PROGRAM_TRIANGLE_BUDGET));
        let arm_geometry_binding = if request.domain_pack_id == "pack_robotic_arm_concept" {
            request
                .plan
                .get("arm_design_intent")
                .map(|intent| {
                    forgecad_core::apply_arm_geometry_family(
                        intent,
                        &mut shape_program,
                        &mut candidate.expanded_assembly_graph,
                    )
                    .map_err(|error| {
                        ReviewedCatalogError::new("ARM_GEOMETRY_FAMILY_INVALID", error.to_string())
                    })
                })
                .transpose()?
        } else {
            None
        };
        let semantic_proportion_binding = request
            .style_recipe
            .as_ref()
            .map(|style_recipe| {
                apply_semantic_proportion_recipe(
                    &mut shape_program,
                    &mut candidate.expanded_assembly_graph,
                    style_recipe,
                    &request.domain_pack_id,
                )
            })
            .transpose()?;
        let shape_program = normalize_persisted_shape_program(&shape_program)
            .map_err(|error| ReviewedCatalogError::new(error.code(), error.to_string()))?;
        let expanded_shape_program_sha256 = sha256_hex(canonical_json(&shape_program).as_bytes());
        let semantic_proportion_binding = semantic_proportion_binding.map(|mut binding| {
            binding.shape_program_sha256 = expanded_shape_program_sha256.clone();
            binding
        });
        let arm_geometry_binding = arm_geometry_binding.map(|mut binding| {
            binding.shape_program_sha256 = Some(expanded_shape_program_sha256.clone());
            binding
        });
        candidate.expanded_shape_program = shape_program.clone();
        candidate.quality_profile = quality_profile.profile_id.clone();
        candidate.candidate_sha256 =
            RecipeExpander::candidate_sha256(&candidate).map_err(|error| {
                ReviewedCatalogError::new("REVIEWED_RECIPE_HASH_INVALID", error.to_string())
            })?;
        let input = RestrictedGeometryInput {
            schema_version: RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION.into(),
            shape_program,
            profile_sketch: None,
            section_set: None,
            surface_adornment_programs: Vec::new(),
            surface_layer_input: None,
            quality_profile,
        };
        input
            .validate()
            .map_err(|error| ReviewedCatalogError::new(error.code, error.message))?;
        let expansion = ReviewedCatalogExpansion {
            geometry_input: input,
            expanded_assembly_graph: Some(candidate.expanded_assembly_graph),
            component_recipe_instances: Some(
                serde_json::to_value(candidate.component_recipe_instances).map_err(|_| {
                    ReviewedCatalogError::new(
                        "REVIEWED_RECIPE_EXPANSION_FAILED",
                        "Recipe provenance could not be serialized.",
                    )
                })?,
            ),
            candidate_sha256: Some(candidate.candidate_sha256),
            expanded_shape_program_sha256: Some(expanded_shape_program_sha256),
            semantic_proportion_binding,
            arm_geometry_binding,
        };
        expansion.validate()?;
        Ok(expansion)
    }
}

impl ReviewedShapeProgramCatalog for EmbeddedReviewedShapeProgramCatalog {
    fn expand(
        &self,
        request: &ReviewedCatalogRequest,
    ) -> Result<ReviewedCatalogExpansion, ReviewedCatalogError> {
        if !supported_domain(&request.domain_pack_id) {
            return Err(ReviewedCatalogError::new(
                "REVIEWED_CATALOG_DOMAIN_UNAVAILABLE",
                "No reviewed embedded ShapeProgram exists for this domain.",
            ));
        }
        let quality_profile =
            RestrictedQualityProfile::for_presentation(&request.presentation_profile)
                .map_err(|failure| ReviewedCatalogError::new(failure.code, failure.message))?;
        let profile_sketch = request.authored_profile_sketch.clone();
        let shape_program = request.authored_shape_program.clone().unwrap_or_else(|| {
            let size = match request.domain_pack_id.as_str() {
                "pack_future_weapon_prop" => [180.0, 56.0, 34.0],
                "pack_vehicle_concept" => [260.0, 72.0, 110.0],
                "pack_aircraft_concept" => [300.0, 54.0, 180.0],
                "pack_robotic_arm_concept" => [84.0, 230.0, 84.0],
                _ => unreachable!("domain checked above"),
            };
            let seed = stable_seed(&format!(
                "{}:{}:{}",
                request.domain_pack_id,
                request.direction_id,
                request.variant_id.as_deref().unwrap_or("default")
            ));
            json!({
                "schema_version": "ShapeProgram@1",
                "program_id": format!("shape_reviewed_{seed}"),
                "units": "millimeter",
                "seed": seed,
                "triangle_budget": quality_profile
                    .max_triangle_count
                    .min(MAX_SHAPE_PROGRAM_TRIANGLE_BUDGET),
                "parameters": [],
                "operations": [{
                    "operation_id": "op_primary_shell",
                    "op": "box",
                    "inputs": [],
                    "args": {
                        "size": size,
                        "position": [0.0, 0.0, 0.0],
                        "rotation": [0.0, 0.0, 0.0],
                        "part_role": "primary_form",
                        "zone_id": "zone_primary",
                        "material_id": "mat_graphite"
                    }
                }],
                "outputs": [{
                    "output_id": "output_primary_shell",
                    "operation_id": "op_primary_shell",
                    "kind": "mesh",
                    "part_role": "primary_form"
                }],
                "non_functional_only": true
            })
        });
        let shape_program = normalize_persisted_shape_program(&shape_program)
            .map_err(|error| ReviewedCatalogError::new(error.code(), error.to_string()))?;
        let input = RestrictedGeometryInput {
            schema_version: RESTRICTED_GEOMETRY_INPUT_SCHEMA_VERSION.into(),
            shape_program,
            profile_sketch,
            section_set: None,
            surface_adornment_programs: Vec::new(),
            surface_layer_input: None,
            quality_profile,
        };
        input
            .validate()
            .map_err(|error| ReviewedCatalogError::new(error.code, error.message))?;
        Ok(ReviewedCatalogExpansion {
            geometry_input: input,
            expanded_assembly_graph: None,
            component_recipe_instances: None,
            candidate_sha256: None,
            expanded_shape_program_sha256: None,
            semantic_proportion_binding: None,
            arm_geometry_binding: None,
        })
    }
}

fn stable_seed(value: &str) -> u32 {
    u32::from_str_radix(&sha256_hex(value.as_bytes())[..8], 16).unwrap_or(0) & 0x7fff_ffff
}

/// Applies the reviewed Style Token's one bounded proportion adjustment to
/// the expanded recipe before the worker sees it.  The style payload was
/// produced by `select_style_recipe`; we still re-derive its complete meaning
/// from the static choice table so a model-authored JSON object cannot create
/// a new scale path or ratio.
fn apply_semantic_proportion_recipe(
    shape_program: &mut Value,
    assembly_graph: &mut Value,
    style_recipe: &Value,
    domain_pack_id: &str,
) -> Result<SemanticProportionBinding, ReviewedCatalogError> {
    let choice = validated_style_choice(style_recipe, domain_pack_id)?;
    let role_selector = semantic_role_selector(domain_pack_id, choice.role_selector);
    let shape_program_role_selector =
        shape_program_role_selector_for_program(domain_pack_id, role_selector, shape_program);
    let axis = match choice.path {
        "transform.scale.x" => 0,
        "transform.scale.y" => 1,
        "transform.scale.z" => 2,
        _ => unreachable!("style choices are code-owned"),
    };
    let factor = 1.0 + 0.1 * f64::from(choice.step_delta);
    let operations = shape_program
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            ReviewedCatalogError::new(
                "SEMANTIC_PROPORTION_SHAPE_INVALID",
                "Expanded recipe ShapeProgram has no operations.",
            )
        })?;
    let mut changed_operations = 0_usize;
    for operation in operations {
        let Some(args) = operation.get_mut("args").and_then(Value::as_object_mut) else {
            continue;
        };
        if args.get("part_role").and_then(Value::as_str) != Some(shape_program_role_selector) {
            continue;
        }
        let mut changed = false;
        for key in ["size", "profile_scale", "cross_section_scale"] {
            let Some(values) = args.get_mut(key).and_then(Value::as_array_mut) else {
                continue;
            };
            if let Some(value) = values.get_mut(axis).and_then(|value| value.as_f64()) {
                let adjusted = value * factor;
                if !adjusted.is_finite() || adjusted <= 0.0 {
                    return Err(ReviewedCatalogError::new(
                        "SEMANTIC_PROPORTION_ADJUSTMENT_INVALID",
                        "Reviewed semantic proportion adjustment left the bounded geometry range.",
                    ));
                }
                values[axis] = json!(adjusted);
                changed = true;
            }
        }
        // C106's long link is a capsule recipe.  Its code-owned editable
        // geometry contract is `height` on the primary axis and `radius` on
        // the two cross-section axes; it intentionally does not pretend that
        // a nonexistent generic size vector exists.  Other domain packs keep
        // the original vector-only adjustment contract.
        if !changed && domain_pack_id == "pack_robotic_arm_concept" && role_selector == "link_armor"
        {
            let scalar_key = match axis {
                0 => "height",
                1 | 2 => "radius",
                _ => unreachable!("style axes are code-owned"),
            };
            if let Some(value) = args.get(scalar_key).and_then(Value::as_f64) {
                let adjusted = value * factor;
                if !adjusted.is_finite() || adjusted <= 0.0 {
                    return Err(ReviewedCatalogError::new(
                        "SEMANTIC_PROPORTION_ADJUSTMENT_INVALID",
                        "Reviewed semantic proportion adjustment left the bounded geometry range.",
                    ));
                }
                args.insert(scalar_key.into(), json!(adjusted));
                changed = true;
            }
        }
        if changed {
            changed_operations += 1;
        }
    }
    if changed_operations == 0 {
        return Err(ReviewedCatalogError::new(
            "SEMANTIC_PROPORTION_TARGET_UNAVAILABLE",
            "The selected reviewed Style Token has no compatible expanded geometry target.",
        ));
    }

    let graph_parts = assembly_graph
        .get_mut("parts")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            ReviewedCatalogError::new(
                "SEMANTIC_PROPORTION_GRAPH_INVALID",
                "Expanded Recipe AssemblyGraph has no parts.",
            )
        })?;
    let mut changed_parts = 0_usize;
    for part in graph_parts {
        if part.get("role").and_then(Value::as_str) != Some(role_selector) {
            continue;
        }
        let Some(scale) = part
            .get_mut("transform")
            .and_then(Value::as_object_mut)
            .and_then(|transform| transform.get_mut("scale"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let Some(value) = scale.get_mut(axis).and_then(|value| value.as_f64()) else {
            continue;
        };
        let adjusted = value * factor;
        if !adjusted.is_finite() || !(0.6..=1.4).contains(&adjusted) {
            return Err(ReviewedCatalogError::new(
                "SEMANTIC_PROPORTION_ADJUSTMENT_INVALID",
                "Reviewed semantic proportion adjustment exceeded the assembly display range.",
            ));
        }
        scale[axis] = json!(adjusted);
        changed_parts += 1;
    }
    if changed_parts == 0 {
        return Err(ReviewedCatalogError::new(
            "SEMANTIC_PROPORTION_TARGET_UNAVAILABLE",
            "The selected reviewed Style Token has no compatible editable assembly part.",
        ));
    }
    Ok(SemanticProportionBinding {
        style_recipe_sha256: sha256_hex(canonical_json(style_recipe).as_bytes()),
        role_selector: role_selector.into(),
        path: choice.path.into(),
        step_delta: choice.step_delta,
        // Filled only after canonical ShapeProgram normalization.
        shape_program_sha256: String::new(),
    })
}

fn validated_style_choice(
    style_recipe: &Value,
    domain_pack_id: &str,
) -> Result<StyleChoice, ReviewedCatalogError> {
    let token = style_recipe
        .get("style_token")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            ReviewedCatalogError::new(
                "SEMANTIC_PROPORTION_STYLE_INVALID",
                "Style Token is unavailable.",
            )
        })?;
    let recipe = style_recipe
        .get("recipe")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            ReviewedCatalogError::new(
                "SEMANTIC_PROPORTION_STYLE_INVALID",
                "Semantic proportion recipe is unavailable.",
            )
        })?;
    let token_id = token.get("token_id").and_then(Value::as_str);
    let choice = STYLE_CHOICES
        .iter()
        .copied()
        .find(|choice| Some(choice.token_id) == token_id)
        .ok_or_else(|| {
            ReviewedCatalogError::new(
                "SEMANTIC_PROPORTION_STYLE_INVALID",
                "Style Token is not code-owned.",
            )
        })?;
    let adjustment = recipe
        .get("adjustments")
        .and_then(Value::as_array)
        .filter(|items| items.len() == 1)
        .and_then(|items| items.first())
        .and_then(Value::as_object);
    if token.get("schema_version").and_then(Value::as_str) != Some("MechanicalStyleToken@1")
        || recipe.get("schema_version").and_then(Value::as_str)
            != Some("DomainSemanticProportionRecipe@1")
        || recipe.get("domain_pack_id").and_then(Value::as_str) != Some(domain_pack_id)
        || recipe.get("style_token_id").and_then(Value::as_str) != Some(choice.token_id)
        || token
            .get("allowed_domains")
            .and_then(Value::as_array)
            .is_none_or(|domains| {
                domains
                    .iter()
                    .any(|domain| domain.as_str() == Some(domain_pack_id))
                    == false
            })
        || adjustment
            .and_then(|item| item.get("role_selector"))
            .and_then(Value::as_str)
            != Some(semantic_role_selector(domain_pack_id, choice.role_selector))
        || adjustment
            .and_then(|item| item.get("path"))
            .and_then(Value::as_str)
            != Some(choice.path)
        || adjustment
            .and_then(|item| item.get("step_delta"))
            .and_then(Value::as_i64)
            != Some(i64::from(choice.step_delta))
    {
        return Err(ReviewedCatalogError::new(
            "SEMANTIC_PROPORTION_STYLE_INVALID",
            "Style Token and semantic proportion recipe did not match the code-owned selection.",
        ));
    }
    Ok(choice)
}

/// Domain packs retain their own role vocabulary.  A generic primary-form
/// style token therefore resolves to the arm pack's actual editable link role
/// before it reaches the ShapeProgram; this is code-owned mapping, not model
/// supplied geometry meaning.
fn semantic_role_selector(domain_pack_id: &str, generic_role: &'static str) -> &'static str {
    match (domain_pack_id, generic_role) {
        // C106 exposes the exterior link armor as the bounded, visibly
        // editable shape target.  Both generic semantic roles intentionally
        // resolve to that real production Recipe role; the Style Token still
        // controls a distinct axis/delta and never introduces a new geometry
        // operation or arbitrary parameter path.
        ("pack_robotic_arm_concept", "primary_form" | "secondary_form") => "link_armor",
        _ => generic_role,
    }
}

/// Component Recipes expose semantic part roles while their ShapeProgram
/// operations preserve a visual mesh role.  Resolve that narrow compiler
/// distinction in one code-owned place so a Style Token cannot choose an
/// arbitrary operation target.
fn shape_program_role_selector<'a>(domain_pack_id: &str, component_role: &'a str) -> &'a str {
    match (domain_pack_id, component_role) {
        ("pack_robotic_arm_concept", "link_armor") => "upper_link_form",
        ("pack_robotic_arm_concept", "joint_housing" | "turntable") => "secondary_form",
        ("pack_robotic_arm_concept", "cable_harness") => "visual_detail",
        ("pack_robotic_arm_concept", "surface_trim") => "trim",
        _ => component_role,
    }
}

/// C106's legacy arm ShapeProgram uses `upper_link_form` for the link armor,
/// while independent C110G Recipes deliberately preserve their reviewed
/// component role as `link_armor`.  Resolve the distinction from the actual
/// expanded program instead of forcing the new family through a C106 target.
fn shape_program_role_selector_for_program<'a>(
    domain_pack_id: &str,
    component_role: &'a str,
    shape_program: &Value,
) -> &'a str {
    if domain_pack_id == "pack_robotic_arm_concept"
        && component_role == "link_armor"
        && shape_program
            .get("operations")
            .and_then(Value::as_array)
            .is_some_and(|operations| {
                operations.iter().any(|operation| {
                    operation
                        .get("args")
                        .and_then(Value::as_object)
                        .and_then(|args| args.get("part_role"))
                        .and_then(Value::as_str)
                        == Some("link_armor")
                })
            })
    {
        return "link_armor";
    }
    shape_program_role_selector(domain_pack_id, component_role)
}

#[derive(Debug, Clone)]
pub struct NativeProductToolExecutorConfig {
    pub max_active_executions: usize,
    pub max_tool_calls_per_execution: u32,
    pub max_wall_time: Duration,
    pub max_cancel_tombstones: usize,
    pub max_preview_artifacts: usize,
    pub max_preview_retained_bytes: usize,
    pub preview_artifact_ttl: Duration,
}

impl Default for NativeProductToolExecutorConfig {
    fn default() -> Self {
        Self {
            max_active_executions: 64,
            max_tool_calls_per_execution: MAX_PRODUCT_TOOL_CALLS,
            // A production geometry Tool performs a bounded compile followed
            // by the fixed concept-view render. M109A's 1K/80k-150k profile
            // needs more than the old 60-second combined allowance on a cold
            // packaged arm64 launch, while the enclosing Action Loop still
            // owns its independent 280-second deadline and cancellation.
            max_wall_time: Duration::from_secs(250),
            max_cancel_tombstones: 256,
            max_preview_artifacts: 16,
            max_preview_retained_bytes: 256 * 1024 * 1024,
            preview_artifact_ttl: Duration::from_secs(5 * 60),
        }
    }
}

impl NativeProductToolExecutorConfig {
    fn validate(&self) -> Result<(), ProductToolPortError> {
        if !(1..=128).contains(&self.max_active_executions)
            || !(1..=MAX_PRODUCT_TOOL_CALLS).contains(&self.max_tool_calls_per_execution)
            || !(Duration::from_millis(1)..=Duration::from_secs(300)).contains(&self.max_wall_time)
            || !(1..=1024).contains(&self.max_cancel_tombstones)
            || !(1..=64).contains(&self.max_preview_artifacts)
            || !(1..=MAX_PREVIEW_ARTIFACT_BYTES_HARD).contains(&self.max_preview_retained_bytes)
            || !(Duration::from_millis(1)..=Duration::from_secs(30 * 60))
                .contains(&self.preview_artifact_ttl)
        {
            return Err(port_error(
                "NATIVE_PRODUCT_TOOL_CONFIG_INVALID",
                ProductToolPortErrorKind::InvalidResponse,
                "Native Product Tool executor configuration is outside hard bounds.",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct NativeProductToolExecutor {
    registry: Arc<ProductToolRegistry>,
    geometry: Arc<dyn RestrictedGeometryPort>,
    catalog: Arc<dyn ReviewedShapeProgramCatalog>,
    config: NativeProductToolExecutorConfig,
    inner: Arc<Mutex<NativeExecutorInner>>,
    active_snapshot_reader: Arc<Mutex<Option<Arc<dyn ActiveDesignSnapshotReader>>>>,
}

/// Rust-owned, read-only projection of the current product Snapshot.  The
/// app-server deliberately knows nothing about SQLite or the CAS; the desktop
/// adapter supplies this capability from `RustCoreRuntime` at construction.
/// Provider input can therefore describe an edit relative to the actual head,
/// while all writes still flow through the existing ChangeSet routes.
pub trait ActiveDesignSnapshotReader: Send + Sync + 'static {
    fn read_active_design_snapshot(
        &self,
        project_id: &str,
    ) -> Result<Option<Value>, ProductToolPortError>;
}

impl NativeProductToolExecutor {
    pub fn new(
        registry: Arc<ProductToolRegistry>,
        geometry: Arc<dyn RestrictedGeometryPort>,
        catalog: Arc<dyn ReviewedShapeProgramCatalog>,
        config: NativeProductToolExecutorConfig,
    ) -> Result<Self, ProductToolPortError> {
        config.validate()?;
        // Construction also proves the immutable fixture and its exact hash
        // remain valid; no second registry is accepted from the worker.
        if registry.definitions().count() != 13 {
            return Err(port_error(
                "NATIVE_PRODUCT_TOOL_REGISTRY_INVALID",
                ProductToolPortErrorKind::InvalidResponse,
                "Native Product Tool executor requires the exact thirteen-tool registry.",
                false,
            ));
        }
        Ok(Self {
            registry,
            geometry,
            catalog,
            config,
            inner: Arc::new(Mutex::new(NativeExecutorInner::default())),
            active_snapshot_reader: Arc::new(Mutex::new(None)),
        })
    }

    /// Attaches the single Rust product-state reader after construction.  The
    /// setter is intentionally one-way and idempotent for the same reader
    /// lifetime; replacing a reader while an app-server is live could make a
    /// Turn observe a different Project store.
    pub fn attach_active_snapshot_reader(
        &self,
        reader: Arc<dyn ActiveDesignSnapshotReader>,
    ) -> Result<(), ProductToolPortError> {
        let mut slot = self.active_snapshot_reader.lock().map_err(|_| {
            ProductToolPortError::invalid_response("ActiveDesignSnapshot reader mutex is poisoned.")
        })?;
        if slot.is_some() {
            return Err(ProductToolPortError::invalid_response(
                "ActiveDesignSnapshot reader is already attached.",
            ));
        }
        *slot = Some(reader);
        Ok(())
    }

    pub fn with_embedded_catalog(
        registry: Arc<ProductToolRegistry>,
        geometry: Arc<dyn RestrictedGeometryPort>,
        config: NativeProductToolExecutorConfig,
    ) -> Result<Self, ProductToolPortError> {
        Self::new(
            registry,
            geometry,
            Arc::new(RecipeBackedReviewedShapeProgramCatalog),
            config,
        )
    }

    /// Retains a legacy blockout compatibility preview after the bounded
    /// compiler and four-view readback have completed.  This path is
    /// intentionally separate from V003: it creates no `SingleResultDecision`
    /// and no formal provenance, because a later ChangeSet owns preview and
    /// confirmation semantics for the editable base asset.
    pub fn retain_compatibility_preview(
        &self,
        execution_id: &str,
        turn_id: &str,
    ) -> Result<NativePreviewArtifact, ProductToolPortError> {
        if !is_stable_id(execution_id) || !is_stable_id(turn_id) {
            return Err(preview_port_error(
                "NATIVE_COMPATIBILITY_PREVIEW_IDENTITY_INVALID",
                "Compatibility preview requires bounded execution and Turn identities.",
            ));
        }
        let mut inner = self.lock_inner()?;
        inner.prune_expired_previews(Instant::now());
        let mut local_state = inner
            .runs
            .get(execution_id)
            .filter(|run| run.turn_id == turn_id && run.in_flight.is_empty())
            .map(|run| run.state.clone())
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_COMPATIBILITY_PREVIEW_STATE_UNAVAILABLE",
                    "Compatibility preview has no completed native execution state.",
                )
            })?;
        if local_state.preview.is_some() || local_state.generation_gate_report.is_some() {
            return Err(preview_port_error(
                "NATIVE_COMPATIBILITY_PREVIEW_CONFLICT",
                "A compatibility preview cannot reuse a formal V003 preview state.",
            ));
        }
        let topology_hash = local_state
            .geometry
            .as_ref()
            .map(|geometry| geometry.topology_hash.clone())
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_COMPATIBILITY_PREVIEW_BUILD_REQUIRED",
                    "Compatibility preview requires completed restricted geometry readback.",
                )
            })?;
        let preview_id = format!(
            "preview_{}",
            &sha256_hex(format!("compatibility:{turn_id}:{topology_hash}").as_bytes())[..24]
        );
        local_state.preview = Some(json!({
            "preview_id": preview_id,
            "topology_hash": topology_hash,
            "requires_user_confirmation": true,
            "permanent_side_effects": 0,
            "compatibility_preview": true,
            "single_result_decision": Value::Null,
        }));
        let artifact = build_native_preview_artifact(
            turn_id,
            &local_state,
            self.config.preview_artifact_ttl,
            false,
        )
        .map_err(|failure| {
            port_error(
                failure.code,
                ProductToolPortErrorKind::InvalidResponse,
                failure.message,
                false,
            )
        })?;
        inner.insert_preview(artifact.clone(), &self.config)?;
        let run = inner.runs.get_mut(execution_id).ok_or_else(|| {
            internal_port_error(
                "Compatibility preview execution disappeared before state promotion.",
            )
        })?;
        run.state = local_state;
        Ok(artifact)
    }

    /// Returns an immutable clone of a live transient preview and refreshes
    /// only its LRU position. The absolute TTL is never extended.
    pub fn preview_artifact(
        &self,
        preview_id: &str,
    ) -> Result<Option<NativePreviewArtifact>, ProductToolPortError> {
        validate_preview_id(preview_id)?;
        let mut inner = self.lock_inner()?;
        inner.prune_expired_previews(Instant::now());
        let artifact = inner
            .preview_artifacts
            .get(preview_id)
            .map(|record| record.artifact.clone());
        if artifact.is_some() {
            inner.touch_preview(preview_id);
        }
        Ok(artifact)
    }

    /// Reads only a V003 preview whose trusted native Project/Turn binding and
    /// sealed GLB identity exactly match the caller's route preconditions.
    pub fn formal_preview_artifact(
        &self,
        project_id: &str,
        turn_id: &str,
        preview_id: &str,
        artifact_sha256: &str,
    ) -> Result<Option<NativePreviewArtifact>, ProductToolPortError> {
        validate_preview_id(preview_id)?;
        if !is_stable_id(project_id)
            || !is_stable_id(turn_id)
            || artifact_sha256.len() != 64
            || !artifact_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(preview_port_error(
                "NATIVE_FORMAL_PREVIEW_IDENTITY_INVALID",
                "Formal preview identity is outside the bounded contract.",
            ));
        }
        let mut inner = self.lock_inner()?;
        inner.prune_expired_previews(Instant::now());
        let Some(record) = inner.preview_artifacts.get(preview_id) else {
            return Ok(None);
        };
        validate_formal_preview_binding(&record.artifact, project_id, turn_id, artifact_sha256)?;
        let artifact = record.artifact.clone();
        inner.touch_preview(preview_id);
        Ok(Some(artifact))
    }

    /// Idempotently drops one formally bound preview. A missing/expired value
    /// is already rejected, while an existing mismatched value is never
    /// discarded.
    pub fn reject_formal_preview(
        &self,
        project_id: &str,
        turn_id: &str,
        preview_id: &str,
        artifact_sha256: &str,
    ) -> Result<bool, ProductToolPortError> {
        validate_preview_id(preview_id)?;
        let mut inner = self.lock_inner()?;
        inner.prune_expired_previews(Instant::now());
        let Some(record) = inner.preview_artifacts.get(preview_id) else {
            return Ok(false);
        };
        validate_formal_preview_binding(&record.artifact, project_id, turn_id, artifact_sha256)?;
        Ok(inner.remove_preview_record(preview_id).is_some())
    }

    /// Atomically transfers a live preview out of the transient registry.
    /// The Turn binding is checked before any bytes are removed.
    pub fn consume_preview(
        &self,
        preview_id: &str,
        turn_id: &str,
    ) -> Result<NativePreviewArtifact, ProductToolPortError> {
        validate_preview_id(preview_id)?;
        if !is_stable_id(turn_id) {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_TURN_INVALID",
                "Preview consumption requires one bounded stable Turn identity.",
            ));
        }
        let mut inner = self.lock_inner()?;
        inner.prune_expired_previews(Instant::now());
        let artifact_turn_id = inner
            .preview_artifacts
            .get(preview_id)
            .map(|record| record.artifact.turn_id.as_str())
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_ARTIFACT_NOT_FOUND",
                    "Native preview is missing, expired, discarded, or already consumed.",
                )
            })?;
        if artifact_turn_id != turn_id {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_TURN_MISMATCH",
                "Native preview cannot be consumed by another Turn.",
            ));
        }
        inner.remove_preview_record(preview_id).ok_or_else(|| {
            internal_port_error("Preview disappeared during atomic transient consumption.")
        })
    }

    /// Explicitly drops transient preview bytes. Missing or expired previews
    /// are an idempotent `false` result.
    pub fn discard_preview(&self, preview_id: &str) -> Result<bool, ProductToolPortError> {
        validate_preview_id(preview_id)?;
        let mut inner = self.lock_inner()?;
        inner.prune_expired_previews(Instant::now());
        Ok(inner.remove_preview_record(preview_id).is_some())
    }

    fn bind_execution_project_native(
        &self,
        execution_id: &str,
        turn_id: &str,
        project_id: Option<&str>,
    ) -> Result<(), ProductToolPortError> {
        if !is_stable_id(execution_id) || !is_stable_id(turn_id) {
            return Err(preview_port_error(
                "NATIVE_PROJECT_BINDING_INVALID",
                "Native Project binding requires bounded execution and Turn identities.",
            ));
        }
        if let Some(project_id) = project_id {
            if !is_stable_id(project_id) {
                return Err(preview_port_error(
                    "NATIVE_PROJECT_BINDING_INVALID",
                    "Native Project binding contains an invalid Project identity.",
                ));
            }
        }
        let mut inner = self.lock_inner()?;
        if let Some(run) = inner.runs.get(execution_id) {
            if run.turn_id != turn_id || run.project_id.as_deref() != project_id {
                return Err(preview_port_error(
                    "NATIVE_PROJECT_BINDING_CONFLICT",
                    "A native Product Tool execution cannot be rebound to another Turn or Project.",
                ));
            }
            return Ok(());
        }
        if let Some(existing) = inner.project_bindings.get(execution_id) {
            if existing.turn_id != turn_id || existing.project_id.as_deref() != project_id {
                return Err(preview_port_error(
                    "NATIVE_PROJECT_BINDING_CONFLICT",
                    "A native Product Tool execution cannot be rebound to another Turn or Project.",
                ));
            }
            return Ok(());
        }
        inner.project_bindings.insert(
            execution_id.to_string(),
            NativeProjectBinding {
                turn_id: turn_id.to_string(),
                project_id: project_id.map(str::to_string),
            },
        );
        // Direct native invocations are deterministic local fixtures unless
        // NativeRuntime replaces this marker after Provider preflight. This
        // prevents an unmarked source from silently passing V003 in tests or
        // compatibility adapters while preserving a truthful offline mode.
        inner
            .source_bindings
            .entry(execution_id.to_string())
            .or_insert_with(|| NativeSourceBinding {
                turn_id: turn_id.to_string(),
                source: GenerationSourceBinding {
                    provider_id: "offline_deterministic".into(),
                    source_kind: GenerationSourceKind::OfflineDeterministic,
                },
            });
        Ok(())
    }

    fn bind_execution_generation_source_native(
        &self,
        execution_id: &str,
        turn_id: &str,
        source: GenerationSourceBinding,
    ) -> Result<(), ProductToolPortError> {
        validate_generation_source_binding(&source)?;
        if !is_stable_id(execution_id) || !is_stable_id(turn_id) {
            return Err(preview_port_error(
                "NATIVE_GENERATION_SOURCE_BINDING_INVALID",
                "Generation source binding requires bounded execution and Turn identities.",
            ));
        }
        let mut inner = self.lock_inner()?;
        if let Some(run) = inner.runs.get(execution_id) {
            if run.turn_id != turn_id || run.state.generation_source.as_ref() != Some(&source) {
                return Err(preview_port_error(
                    "NATIVE_GENERATION_SOURCE_BINDING_CONFLICT",
                    "Generation source cannot be rebound after Product Tool execution begins.",
                ));
            }
            return Ok(());
        }
        if let Some(existing) = inner.source_bindings.get(execution_id) {
            if existing.turn_id != turn_id {
                return Err(preview_port_error(
                    "NATIVE_GENERATION_SOURCE_BINDING_CONFLICT",
                    "Generation source binding Turn does not match the active execution.",
                ));
            }
            if existing.source == source {
                return Ok(());
            }
            if existing.source.source_kind != GenerationSourceKind::OfflineDeterministic {
                return Err(preview_port_error(
                    "NATIVE_GENERATION_SOURCE_BINDING_CONFLICT",
                    "A non-offline generation source cannot be rebound.",
                ));
            }
        }
        inner.source_bindings.insert(
            execution_id.to_string(),
            NativeSourceBinding {
                turn_id: turn_id.to_string(),
                source,
            },
        );
        Ok(())
    }

    async fn execute_native(
        self,
        request: ProductToolExecutionRequest,
        caller_cancellation: CancellationToken,
    ) -> Result<ProductToolExecutionResult, ProductToolPortError> {
        self.validate_request(&request)?;
        let started = Instant::now();

        let (mut local_state, generation, run_cancellation) = {
            let mut inner = self.lock_inner()?;
            inner.prune_expired_previews(Instant::now());
            let execution_id = request.execution_id.clone();
            if !inner.runs.contains_key(&execution_id) {
                self.create_run(&mut inner, &request)?;
            }
            inner.touch(&execution_id);
            let run = inner.runs.get_mut(&execution_id).ok_or_else(|| {
                internal_port_error("Native Product Tool execution state disappeared.")
            })?;
            if run.turn_id != request.turn_id
                || run.cancellation_id != request.cancellation_id
                || run.cancellation_token != request.cancellation_token
            {
                return Err(port_error(
                    "NATIVE_PRODUCT_TOOL_EXECUTION_IDENTITY_CONFLICT",
                    ProductToolPortErrorKind::InvalidResponse,
                    "Execution identity cannot be rebound across Turn or cancellation scope.",
                    false,
                ));
            }
            if let Some(cached) = run.cached.get(&request.call_id) {
                if cached.idempotency_key != request.idempotency_key {
                    return Err(port_error(
                        "NATIVE_PRODUCT_TOOL_CALL_ID_CONFLICT",
                        ProductToolPortErrorKind::InvalidResponse,
                        "A Product Tool call ID cannot be reused with another decision.",
                        false,
                    ));
                }
                return Ok(cached.result.clone());
            }
            if let Some(previous_call) = run.idempotency_to_call.get(&request.idempotency_key) {
                if previous_call != &request.call_id {
                    return Err(port_error(
                        "NATIVE_PRODUCT_TOOL_IDEMPOTENCY_CONFLICT",
                        ProductToolPortErrorKind::InvalidResponse,
                        "An idempotency key cannot identify two Product Tool calls.",
                        false,
                    ));
                }
            }
            if run.in_flight.contains(&request.call_id) {
                return Err(port_error(
                    "NATIVE_PRODUCT_TOOL_CALL_IN_FLIGHT",
                    ProductToolPortErrorKind::Unavailable,
                    "The same Product Tool call is already in flight.",
                    true,
                ));
            }
            if run.calls_started >= self.config.max_tool_calls_per_execution {
                return Err(port_error(
                    "NATIVE_PRODUCT_TOOL_CALL_LIMIT",
                    ProductToolPortErrorKind::InvalidResponse,
                    "The bounded Product Tool execution exceeded its call limit.",
                    false,
                ));
            }
            run.calls_started += 1;
            let is_repair_call = request.tool_name == "build_candidate_geometry"
                && request.validated_arguments.value.contains_key("repair");
            let repair_attempts_started_before = run.repair_calls_started;
            if is_repair_call && run.repair_calls_started < MAX_SAME_INTENT_REPAIR_ATTEMPTS {
                // Reserve before invoking transactional local state. A
                // schema-valid repair request counts even when its parent is
                // stale, its patch is inapplicable, or worker execution
                // fails; otherwise failures could bypass the two-attempt
                // V003 limit.
                run.repair_calls_started += 1;
            }
            run.in_flight.insert(request.call_id.clone());
            run.idempotency_to_call
                .insert(request.idempotency_key.clone(), request.call_id.clone());
            if caller_cancellation.is_cancelled() {
                run.generation = run.generation.saturating_add(1);
                run.run_cancellation.cancel();
            }
            let mut local_state = run.state.clone();
            local_state.repair_attempts_started = repair_attempts_started_before;
            (local_state, run.generation, run.run_cancellation.clone())
        };

        let invocation = self.invoke_tool(&request, &mut local_state, run_cancellation.clone());
        let mut outcome = match tokio::time::timeout(
            self.config.max_wall_time,
            race_cancellation(invocation, caller_cancellation),
        )
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(())) => {
                run_cancellation.cancel();
                Err(NativeToolFailure::cancelled())
            }
            Err(_) => {
                run_cancellation.cancel();
                Err(NativeToolFailure::timeout())
            }
        };

        let pending_preview = if request.tool_name == "prepare_candidate_preview" && outcome.is_ok()
        {
            match build_native_preview_artifact(
                &request.turn_id,
                &local_state,
                self.config.preview_artifact_ttl,
                true,
            ) {
                Ok(artifact) => Some(artifact),
                Err(failure) => {
                    outcome = Err(failure);
                    None
                }
            }
        } else {
            None
        };

        let duration_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let mut inner = self.lock_inner()?;
        let late_result = {
            let run = inner.runs.get_mut(&request.execution_id).ok_or_else(|| {
                internal_port_error("Native Product Tool execution was evicted while in flight.")
            })?;
            run.in_flight.remove(&request.call_id);
            run.touched_at = Instant::now();
            run.generation != generation
        };
        let result = if late_result {
            inner.discard_preview_for_turn(&request.turn_id);
            cancelled_result(&request, duration_ms)
        } else {
            match outcome {
                Ok(output) => {
                    let completed =
                        completed_result(&self.registry, &request, output, duration_ms)?;
                    let insertion = pending_preview
                        .map(|artifact| inner.insert_preview(artifact, &self.config))
                        .transpose();
                    match insertion {
                        Ok(_) => {
                            let run = inner.runs.get_mut(&request.execution_id).ok_or_else(|| {
                                internal_port_error(
                                    "Native Product Tool execution disappeared before state promotion.",
                                )
                            })?;
                            run.state = local_state;
                            completed
                        }
                        Err(error) => {
                            inner.discard_preview_for_turn(&request.turn_id);
                            failed_result(
                                &request,
                                NativeToolFailure::new(
                                    ProductToolFailureCategory::Execution,
                                    error.code,
                                    error.message,
                                ),
                                duration_ms,
                            )
                        }
                    }
                }
                Err(failure) => {
                    inner.discard_preview_for_turn(&request.turn_id);
                    failed_result(&request, failure, duration_ms)
                }
            }
        };
        if let Err(error) = self.registry.validate_result(&request, &result) {
            inner.discard_preview_for_turn(&request.turn_id);
            return Err(port_error(
                "NATIVE_PRODUCT_TOOL_RESULT_INVALID",
                ProductToolPortErrorKind::InvalidResponse,
                error.message,
                false,
            ));
        }
        let run = inner.runs.get_mut(&request.execution_id).ok_or_else(|| {
            internal_port_error("Native Product Tool execution disappeared before caching.")
        })?;
        run.cached.insert(
            request.call_id.clone(),
            CachedNativeResult {
                idempotency_key: request.idempotency_key,
                result: result.clone(),
            },
        );
        Ok(result)
    }

    fn validate_request(
        &self,
        request: &ProductToolExecutionRequest,
    ) -> Result<(), ProductToolPortError> {
        request.validate().map_err(|error| {
            port_error(
                "NATIVE_PRODUCT_TOOL_REQUEST_INVALID",
                ProductToolPortErrorKind::InvalidResponse,
                error.message,
                false,
            )
        })?;
        if request.registry_schema_version != PRODUCT_TOOL_REGISTRY_SCHEMA_VERSION {
            return Err(internal_port_error(
                "Product Tool registry version does not match the native executor.",
            ));
        }
        let definition = self
            .registry
            .definition(&request.tool_name)
            .map_err(|error| {
                port_error(
                    "NATIVE_PRODUCT_TOOL_NOT_CODE_OWNED",
                    ProductToolPortErrorKind::InvalidResponse,
                    error.message,
                    false,
                )
            })?;
        if definition.tool_id != request.tool_id
            || definition.approval_policy != request.approval_policy
            || definition.input_schema_sha256 != request.validated_arguments.schema_sha256
        {
            return Err(port_error(
                "NATIVE_PRODUCT_TOOL_IDENTITY_DRIFT",
                ProductToolPortErrorKind::InvalidResponse,
                "Product Tool identity, policy, or Schema digest drifted from the code-owned registry.",
                false,
            ));
        }
        let arguments = Value::Object(
            request
                .validated_arguments
                .value
                .clone()
                .into_iter()
                .collect::<Map<_, _>>(),
        );
        validate_json_schema(&definition.input_schema, &arguments).map_err(|message| {
            port_error(
                "NATIVE_PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID",
                ProductToolPortErrorKind::InvalidResponse,
                message,
                false,
            )
        })?;
        reject_forbidden_json(&arguments).map_err(|failure| {
            port_error(
                &failure.code,
                ProductToolPortErrorKind::InvalidResponse,
                failure.message,
                false,
            )
        })?;
        let expected = sha256_hex(
            canonical_json(&json!({
                "turn_id": request.turn_id,
                "call_id": request.call_id,
                "tool_id": request.tool_id,
                "arguments": request.validated_arguments.value,
            }))
            .as_bytes(),
        );
        if request.idempotency_key != expected {
            return Err(port_error(
                "NATIVE_PRODUCT_TOOL_IDEMPOTENCY_DRIFT",
                ProductToolPortErrorKind::InvalidResponse,
                "Product Tool idempotency is not bound to the sealed Rust decision.",
                false,
            ));
        }
        Ok(())
    }

    fn create_run(
        &self,
        inner: &mut NativeExecutorInner,
        request: &ProductToolExecutionRequest,
    ) -> Result<(), ProductToolPortError> {
        if let Some(execution_id) = inner
            .execution_by_cancellation
            .get(&request.cancellation_id)
        {
            if execution_id != &request.execution_id {
                return Err(port_error(
                    "NATIVE_PRODUCT_TOOL_CANCELLATION_ID_CONFLICT",
                    ProductToolPortErrorKind::InvalidResponse,
                    "Cancellation ID is already bound to another execution.",
                    false,
                ));
            }
        }
        while inner.runs.len() >= self.config.max_active_executions {
            let removable = inner.order.iter().find_map(|execution_id| {
                inner
                    .runs
                    .get(execution_id)
                    .filter(|run| run.in_flight.is_empty())
                    .map(|_| execution_id.clone())
            });
            let Some(removable) = removable else {
                return Err(port_error(
                    "NATIVE_PRODUCT_TOOL_BACKPRESSURE",
                    ProductToolPortErrorKind::Unavailable,
                    "All bounded native Product Tool executions are active.",
                    true,
                ));
            };
            inner.remove_run(&removable);
        }
        let project_id = inner
            .project_bindings
            .remove(&request.execution_id)
            .map(|binding| {
                if binding.turn_id != request.turn_id {
                    return Err(port_error(
                        "NATIVE_PROJECT_BINDING_CONFLICT",
                        ProductToolPortErrorKind::InvalidResponse,
                        "A native Product Tool execution Project binding did not match its Turn.",
                        false,
                    ));
                }
                Ok(binding.project_id)
            })
            .transpose()?
            .flatten();
        let state_project_id = project_id.clone();
        let active_snapshot = if let Some(project_id) = project_id.as_deref() {
            let reader = self
                .active_snapshot_reader
                .lock()
                .map_err(|_| {
                    port_error(
                        "ACTIVE_DESIGN_SNAPSHOT_READER_UNAVAILABLE",
                        ProductToolPortErrorKind::Unavailable,
                        "ActiveDesignSnapshot reader lock is unavailable.",
                        true,
                    )
                })?
                .clone();
            reader
                .map(|reader| reader.read_active_design_snapshot(project_id))
                .transpose()?
                .flatten()
        } else {
            None
        };
        let generation_source = inner
            .source_bindings
            .remove(&request.execution_id)
            .map(|binding| {
                if binding.turn_id != request.turn_id {
                    return Err(port_error(
                        "NATIVE_GENERATION_SOURCE_BINDING_CONFLICT",
                        ProductToolPortErrorKind::InvalidResponse,
                        "Generation source binding did not match the execution Turn.",
                        false,
                    ));
                }
                validate_generation_source_binding(&binding.source)?;
                Ok(binding.source)
            })
            .transpose()?;
        let run_cancellation = CancellationToken::new();
        if let Some(token) = inner.cancel_tombstones.get(&request.cancellation_id) {
            if token != &request.cancellation_token {
                return Err(port_error(
                    "NATIVE_PRODUCT_TOOL_CANCELLATION_TOKEN_MISMATCH",
                    ProductToolPortErrorKind::InvalidResponse,
                    "Cancellation token conflicts with cancel-before-start state.",
                    false,
                ));
            }
            run_cancellation.cancel();
        }
        inner.execution_by_cancellation.insert(
            request.cancellation_id.clone(),
            request.execution_id.clone(),
        );
        inner.order.push_back(request.execution_id.clone());
        inner.runs.insert(
            request.execution_id.clone(),
            NativeExecutionRun {
                turn_id: request.turn_id.clone(),
                project_id,
                cancellation_id: request.cancellation_id.clone(),
                cancellation_token: request.cancellation_token.clone(),
                run_cancellation,
                generation: usize::from(
                    inner
                        .cancel_tombstones
                        .contains_key(&request.cancellation_id),
                ),
                calls_started: 0,
                repair_calls_started: 0,
                state: NativeToolState {
                    project_id: state_project_id,
                    turn_id: request.turn_id.clone(),
                    generation_source,
                    active_snapshot,
                    ..NativeToolState::default()
                },
                in_flight: HashSet::new(),
                cached: HashMap::new(),
                idempotency_to_call: HashMap::new(),
                touched_at: Instant::now(),
            },
        );
        Ok(())
    }

    fn lock_inner(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, NativeExecutorInner>, ProductToolPortError> {
        self.inner.lock().map_err(|_| {
            port_error(
                "NATIVE_PRODUCT_TOOL_STATE_UNAVAILABLE",
                ProductToolPortErrorKind::Unavailable,
                "Native Product Tool state lock is unavailable.",
                true,
            )
        })
    }

    async fn invoke_tool(
        &self,
        request: &ProductToolExecutionRequest,
        state: &mut NativeToolState,
        run_cancellation: CancellationToken,
    ) -> Result<BTreeMap<String, Value>, NativeToolFailure> {
        if run_cancellation.is_cancelled() {
            return Err(NativeToolFailure::cancelled());
        }
        let arguments = &request.validated_arguments.value;
        let output = match request.tool_name.as_str() {
            "infer_product_domain" => infer_product_domain(arguments, state)?,
            "research_approved_references" => research_approved_references(arguments),
            "select_style_recipe" => select_style_recipe(arguments, state)?,
            "author_profile_sketch" | "validate_profile_sketch" => {
                author_profile_sketch(arguments, state)?
            }
            "author_shape_program" | "validate_shape_program" => {
                author_shape_program(arguments, state)?
            }
            "plan_complete_concept" => plan_complete_concept(arguments, state)?,
            "build_candidate_geometry" => {
                self.build_candidate_geometry(arguments, state, run_cancellation)
                    .await?
            }
            "compile_readback_candidate" => compile_readback_candidate(state)?,
            "render_candidate_views" => render_candidate_views(state)?,
            "evaluate_candidate" => evaluate_candidate(state)?,
            "prepare_candidate_preview" => prepare_candidate_preview(&request.turn_id, state)?,
            _ => {
                return Err(NativeToolFailure::unsupported(
                    "NATIVE_PRODUCT_TOOL_UNSUPPORTED",
                    "Product Tool has no native executor.",
                ))
            }
        };
        value_object(output)
    }

    async fn build_candidate_geometry(
        &self,
        arguments: &BTreeMap<String, Value>,
        state: &mut NativeToolState,
        run_cancellation: CancellationToken,
    ) -> Result<Value, NativeToolFailure> {
        if arguments.get("repair").is_some() {
            return self
                .repair_candidate_geometry(arguments, state, run_cancellation)
                .await;
        }
        if state.preview.is_some() {
            return Err(NativeToolFailure::conflict(
                "NATIVE_PREVIEW_ALREADY_PREPARED",
                "A completed execution cannot replace its single preview descriptor.",
            ));
        }
        if state.build.is_some() {
            return Err(NativeToolFailure::conflict(
                "SINGLE_GENERATION_FULL_BUILD_ALREADY_USED",
                "One Turn may create only one complete synthesis; later builds require an authorized bounded repair patch.",
            ));
        }
        let plan = state.plan.clone().ok_or_else(|| {
            NativeToolFailure::conflict(
                "ACTION_LOOP_PLAN_REQUIRED",
                "A bound concept plan is required before geometry build.",
            )
        })?;
        let direction_id = string_argument(arguments, "direction_id")?;
        let direction_exists = plan
            .get("directions")
            .and_then(Value::as_array)
            .is_some_and(|directions| {
                directions.iter().any(|direction| {
                    direction.get("direction_id").and_then(Value::as_str) == Some(direction_id)
                })
            });
        if !direction_exists {
            return Err(NativeToolFailure::conflict(
                "ACTION_LOOP_DIRECTION_CONFLICT",
                "Geometry direction is not present in the bound concept plan.",
            ));
        }
        let domain_pack_id = plan
            .get("domain_pack_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NativeToolFailure::schema(
                    "ACTION_LOOP_PLAN_DOMAIN_MISSING",
                    "Bound concept plan has no domain.",
                )
            })?
            .to_string();
        let presentation_profile = string_argument(arguments, "presentation_profile")?.to_string();
        let catalog_request = ReviewedCatalogRequest {
            domain_pack_id: domain_pack_id.clone(),
            direction_id: direction_id.to_string(),
            variant_id: arguments
                .get("variant_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            presentation_profile: presentation_profile.clone(),
            plan: plan.clone(),
            style_recipe: state.style_recipe.clone(),
            authored_profile_sketch: state.profile_sketch.clone(),
            authored_shape_program: state.shape_program.clone(),
            geometry_strategy: state.geometry_strategy,
        };
        let catalog_expansion = self
            .catalog
            .expand(&catalog_request)
            .map_err(|error| NativeToolFailure::unsupported(error.code, error.message))?;
        catalog_expansion
            .validate()
            .map_err(|error| NativeToolFailure::unsupported(error.code, error.message))?;
        let expanded = catalog_expansion.geometry_input.clone();
        expanded.validate().map_err(native_failure_from_geometry)?;
        let work = self
            .geometry
            .build_compile_render(expanded.clone(), run_cancellation.clone());
        let geometry = race_cancellation(work, run_cancellation)
            .await
            .map_err(|()| NativeToolFailure::cancelled())?
            .map_err(native_failure_from_geometry)?;
        geometry
            .validate(&expanded)
            .map_err(native_failure_from_geometry)?;
        let output = json!({
            "direction_id": direction_id,
            "topology_hash": geometry.topology_hash,
            "triangle_count": geometry.readback.triangle_count,
            "bounds_mm": geometry.readback.bounds_mm,
            "candidate_only": true
        });
        state.expanded_geometry = Some(expanded);
        state.recipe_expansion = Some(catalog_expansion);
        state.geometry = Some(geometry);
        state.build = Some(output.clone());
        state.evaluation = None;
        state.initial_build_identity = Some(InitialBuildIdentity {
            direction_id: direction_id.to_string(),
            variant_id: arguments
                .get("variant_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            presentation_profile: presentation_profile.to_string(),
            plan_sha256: sha256_hex(canonical_json(&plan).as_bytes()),
            style_recipe_sha256: state
                .style_recipe
                .as_ref()
                .map(|value| sha256_hex(canonical_json(value).as_bytes())),
            profile_sketch_sha256: state
                .profile_sketch
                .as_ref()
                .map(|value| sha256_hex(canonical_json(value).as_bytes())),
            domain_pack_id,
        });
        Ok(output)
    }

    async fn repair_candidate_geometry(
        &self,
        arguments: &BTreeMap<String, Value>,
        state: &mut NativeToolState,
        run_cancellation: CancellationToken,
    ) -> Result<Value, NativeToolFailure> {
        if state.preview.is_some() {
            return Err(NativeToolFailure::conflict(
                "NATIVE_PREVIEW_ALREADY_PREPARED",
                "A completed execution cannot repair its prepared preview.",
            ));
        }
        if state.repair_attempts_started >= MAX_SAME_INTENT_REPAIR_ATTEMPTS {
            return Err(NativeToolFailure::conflict(
                "REPAIR_ATTEMPT_LIMIT_REACHED",
                "One Turn permits at most two same-intent in-place repairs.",
            ));
        }
        let identity = state.initial_build_identity.as_ref().ok_or_else(|| {
            NativeToolFailure::conflict(
                "REPAIR_INITIAL_BUILD_REQUIRED",
                "A bounded repair requires one prior complete synthesis in the same execution.",
            )
        })?;
        let direction_id = string_argument(arguments, "direction_id")?;
        let presentation_profile = string_argument(arguments, "presentation_profile")?;
        let variant_id = arguments
            .get("variant_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let current_plan = state.plan.as_ref().ok_or_else(|| {
            NativeToolFailure::conflict(
                "REPAIR_INTENT_UNAVAILABLE",
                "The original reviewed plan is unavailable for repair identity verification.",
            )
        })?;
        let current_plan_sha256 = sha256_hex(canonical_json(current_plan).as_bytes());
        let current_style_sha256 = state
            .style_recipe
            .as_ref()
            .map(|value| sha256_hex(canonical_json(value).as_bytes()));
        let current_profile_sha256 = state
            .profile_sketch
            .as_ref()
            .map(|value| sha256_hex(canonical_json(value).as_bytes()));
        if direction_id != identity.direction_id
            || variant_id != identity.variant_id
            || presentation_profile != identity.presentation_profile
            || current_plan_sha256 != identity.plan_sha256
            || current_style_sha256 != identity.style_recipe_sha256
            || current_profile_sha256 != identity.profile_sketch_sha256
            || state.domain_pack_id.as_deref() != Some(identity.domain_pack_id.as_str())
        {
            return Err(NativeToolFailure::conflict(
                "REPAIR_ATTEMPT_INTENT_DRIFT",
                "Repair must preserve the initial direction, profile, plan, Style Token, profile sketch, and Domain Pack.",
            ));
        }
        let patch: BoundedGeometryRepairPatch =
            serde_json::from_value(arguments.get("repair").cloned().ok_or_else(|| {
                NativeToolFailure::schema(
                    "BOUNDED_REPAIR_PATCH_REQUIRED",
                    "A repair build requires one bounded repair patch.",
                )
            })?)
            .map_err(|_| {
                NativeToolFailure::schema(
                    "BOUNDED_REPAIR_PATCH_INVALID",
                    "Bounded repair patch does not match its code-owned schema.",
                )
            })?;
        if patch.schema_version != BOUNDED_REPAIR_PATCH_SCHEMA_VERSION {
            return Err(NativeToolFailure::schema(
                "BOUNDED_REPAIR_PATCH_INVALID",
                "Bounded repair patch must use its exact v1 schema.",
            ));
        }
        let parent_attempt = state.generation_attempt.clone().ok_or_else(|| {
            NativeToolFailure::conflict(
                "REPAIR_PARENT_ATTEMPT_REQUIRED",
                "A repair requires the evaluated parent generation attempt.",
            )
        })?;
        let parent_report = state.generation_gate_report.clone().ok_or_else(|| {
            NativeToolFailure::conflict(
                "REPAIR_PARENT_GATE_REPORT_REQUIRED",
                "A repair requires the evaluated parent hard-gate report.",
            )
        })?;
        if patch.parent_attempt_id != parent_attempt.attempt_id
            || patch.parent_gate_report_id != parent_report.gate_report_id
        {
            return Err(NativeToolFailure::conflict(
                "REPAIR_PARENT_PROVENANCE_MISMATCH",
                "Repair parent attempt and gate report must exactly match the current failed attempt.",
            ));
        }
        let current_core_intent = state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.candidate_sha256.as_deref())
            .map(str::to_string)
            .or_else(|| state.profile_sketch.as_ref().map(canonical_json))
            .unwrap_or_else(|| "reviewed_component_recipe".into());
        let current_recipe_provenance = state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.candidate_sha256.clone());
        let parent_expanded = state.expanded_geometry.as_ref().ok_or_else(|| {
            NativeToolFailure::conflict(
                "REPAIR_PARENT_GEOMETRY_REQUIRED",
                "The parent restricted geometry input is unavailable.",
            )
        })?;
        if sha256_hex(state.brief.as_deref().unwrap_or_default().as_bytes())
            != parent_attempt.brief_sha256
            || sha256_hex(identity.domain_pack_id.as_bytes()) != parent_attempt.domain_pack_sha256
            || sha256_hex(current_core_intent.as_bytes())
                != parent_attempt.core_recipe_or_profile_sha256
            || sha256_hex(
                parent_expanded
                    .quality_profile
                    .runtime_manifest_version
                    .as_bytes(),
            ) != parent_attempt.runtime_manifest_sha256
            || current_recipe_provenance != parent_attempt.recipe_provenance_sha256
        {
            return Err(NativeToolFailure::conflict(
                "REPAIR_ATTEMPT_INTENT_DRIFT",
                "Repair must preserve the parent Brief, Domain Pack, Recipe/profile, runtime manifest, and provenance.",
            ));
        }
        if !parent_report.allows_repair() {
            return Err(NativeToolFailure::conflict(
                "REPAIR_ATTEMPT_NOT_ALLOWED",
                "Undetermined or non-repairable hard-gate failures cannot be repaired in place.",
            ));
        }
        if !parent_report
            .repairable_gate_ids()
            .contains(patch.gate_id.as_str())
        {
            return Err(NativeToolFailure::conflict(
                "REPAIR_ATTEMPT_SCOPE_INVALID",
                "Repair may target only a concrete repairable failure in the parent gate report.",
            ));
        }

        // Advance the transactional lineage to the reservation made before
        // invocation. Failed calls still consume the run-owned budget even
        // though this local state is promoted only on success.
        state.repair_attempts_started += 1;
        let mut expanded = state.expanded_geometry.clone().ok_or_else(|| {
            NativeToolFailure::conflict(
                "REPAIR_PARENT_GEOMETRY_REQUIRED",
                "The parent restricted geometry input is unavailable.",
            )
        })?;
        let recipe_graph = state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.expanded_assembly_graph.as_ref())
            .cloned();
        let applied = apply_bounded_geometry_repair(
            &mut expanded.shape_program,
            recipe_graph.as_ref(),
            &patch,
        )?;
        expanded.shape_program = normalize_persisted_shape_program(&expanded.shape_program)
            .map_err(|error| NativeToolFailure::schema(error.code(), error.to_string()))?;
        expanded.validate().map_err(native_failure_from_geometry)?;
        let work = self
            .geometry
            .build_compile_render(expanded.clone(), run_cancellation.clone());
        let geometry = race_cancellation(work, run_cancellation)
            .await
            .map_err(|()| NativeToolFailure::cancelled())?
            .map_err(native_failure_from_geometry)?;
        geometry
            .validate(&expanded)
            .map_err(native_failure_from_geometry)?;

        let repair_number = state.repair_attempts_started;
        let repaired_attempt = SingleGenerationAttempt {
            schema_version: SINGLE_GENERATION_ATTEMPT_SCHEMA_VERSION.into(),
            attempt_id: format!(
                "attempt_{}",
                &sha256_hex(
                    format!(
                        "{}:{}:{}:{}",
                        parent_attempt.attempt_id,
                        repair_number,
                        geometry.glb_sha256,
                        geometry.topology_hash
                    )
                    .as_bytes()
                )[..24]
            ),
            turn_id: parent_attempt.turn_id.clone(),
            project_id: parent_attempt.project_id.clone(),
            attempt_kind: GenerationAttemptKind::Repair,
            parent_attempt_id: Some(parent_attempt.attempt_id.clone()),
            brief_sha256: parent_attempt.brief_sha256.clone(),
            domain_pack_id: parent_attempt.domain_pack_id.clone(),
            domain_pack_sha256: parent_attempt.domain_pack_sha256.clone(),
            core_recipe_or_profile_sha256: parent_attempt.core_recipe_or_profile_sha256.clone(),
            runtime_manifest_sha256: parent_attempt.runtime_manifest_sha256.clone(),
            shape_program_sha256: geometry.readback.shape_program_sha256.clone(),
            recipe_provenance_sha256: parent_attempt.recipe_provenance_sha256.clone(),
        };
        let repair = RepairAttempt {
            schema_version: REPAIR_ATTEMPT_SCHEMA_VERSION.into(),
            repair_id: format!(
                "repair_{}",
                &sha256_hex(
                    format!(
                        "{}:{}:{}:{}",
                        parent_attempt.attempt_id,
                        parent_report.gate_report_id,
                        applied.patch_id,
                        repair_number
                    )
                    .as_bytes()
                )[..24]
            ),
            parent_attempt_id: parent_attempt.attempt_id.clone(),
            parent_gate_report_id: parent_report.gate_report_id.clone(),
            repaired_gate_ids: vec![applied.gate_id.clone()],
            repaired_attempt: repaired_attempt.clone(),
        };
        repair
            .validate_against(&parent_attempt, &parent_report)
            .map_err(|error| {
                NativeToolFailure::new(
                    ProductToolFailureCategory::Execution,
                    error.code(),
                    error.to_string(),
                )
            })?;

        let repaired_recipe_expansion = state
            .recipe_expansion
            .clone()
            .map(|mut recipe_expansion| {
                recipe_expansion.geometry_input = expanded.clone();
                if recipe_expansion.expanded_assembly_graph.is_some() {
                    recipe_expansion.expanded_shape_program_sha256 =
                        Some(geometry.readback.shape_program_sha256.clone());
                    if let Some(binding) = recipe_expansion.semantic_proportion_binding.as_mut() {
                        binding.shape_program_sha256 =
                            geometry.readback.shape_program_sha256.clone();
                    }
                }
                recipe_expansion.validate().map_err(|error| {
                    NativeToolFailure::new(
                        ProductToolFailureCategory::Execution,
                        error.code,
                        error.message,
                    )
                })?;
                Ok::<_, NativeToolFailure>(recipe_expansion)
            })
            .transpose()?;
        let output = json!({
            "direction_id": direction_id,
            "topology_hash": geometry.topology_hash,
            "triangle_count": geometry.readback.triangle_count,
            "bounds_mm": geometry.readback.bounds_mm,
            "candidate_only": true,
            "repair": {
                "schema_version": BOUNDED_REPAIR_PATCH_SCHEMA_VERSION,
                "repair_id": repair.repair_id,
                "repair_number": repair_number,
                "parent_attempt_id": repair.parent_attempt_id,
                "parent_gate_report_id": repair.parent_gate_report_id,
                "repaired_attempt_id": repaired_attempt.attempt_id,
                "gate_id": applied.gate_id,
                "patch_id": applied.patch_id,
                "patched_fields": applied.patched_fields
            }
        });
        state.expanded_geometry = Some(expanded);
        state.recipe_expansion = repaired_recipe_expansion;
        state.geometry = Some(geometry);
        state.build = Some(output.clone());
        state.evaluation = None;
        state.generation_attempt = Some(repaired_attempt);
        state.generation_gate_report = None;
        state.repair_attempts.push(repair);
        Ok(output)
    }
}

fn apply_bounded_geometry_repair(
    shape_program: &mut Value,
    recipe_assembly_graph: Option<&Value>,
    patch: &BoundedGeometryRepairPatch,
) -> Result<AppliedBoundedRepairPatch, NativeToolFailure> {
    let program = shape_program.as_object_mut().ok_or_else(|| {
        NativeToolFailure::schema(
            "BOUNDED_REPAIR_SHAPE_PROGRAM_INVALID",
            "Repair target is not a validated ShapeProgram object.",
        )
    })?;
    let outputs_snapshot = program.get("outputs").cloned();
    let operations = program
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            NativeToolFailure::schema(
                "BOUNDED_REPAIR_SHAPE_PROGRAM_INVALID",
                "Repair target has no bounded operations array.",
            )
        })?;
    let mut patched_fields = Vec::new();
    match (patch.gate_id.as_str(), patch.patch_id.as_str()) {
        ("closed_manifold", "seal_open_sweep_caps") => {
            for operation in operations.iter_mut() {
                let Some(operation) = operation.as_object_mut() else {
                    continue;
                };
                if operation.get("op").and_then(Value::as_str) != Some("sweep") {
                    continue;
                }
                let operation_id = operation
                    .get("operation_id")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let Some(args) = operation.get_mut("args").and_then(Value::as_object_mut) else {
                    continue;
                };
                for field in ["cap_start", "cap_end"] {
                    if args.get(field) != Some(&Value::Bool(true)) {
                        args.insert(field.into(), Value::Bool(true));
                        patched_fields.push(format!(
                            "shape_program.operations[{operation_id}].args.{field}"
                        ));
                    }
                }
            }
        }
        ("surface_provenance_present", "restore_output_material_zones") => {
            let outputs = outputs_snapshot
                .as_ref()
                .and_then(Value::as_array)
                .cloned()
                .ok_or_else(|| {
                    NativeToolFailure::schema(
                        "BOUNDED_REPAIR_SHAPE_PROGRAM_INVALID",
                        "Repair target has no bounded outputs array.",
                    )
                })?;
            let recipe_zones = recipe_assembly_graph
                .and_then(|graph| graph.get("parts"))
                .and_then(Value::as_array)
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| {
                            Some((
                                (
                                    part.get("operation_id")?.as_str()?.to_string(),
                                    part.get("output_id")?.as_str()?.to_string(),
                                ),
                                part.get("material_zone_ids")?
                                    .as_array()?
                                    .first()?
                                    .as_str()?
                                    .to_string(),
                            ))
                        })
                        .collect::<BTreeMap<_, _>>()
                })
                .unwrap_or_default();
            for output in outputs {
                let Some(output) = output.as_object() else {
                    continue;
                };
                let Some(operation_id) = output.get("operation_id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(output_id) = output.get("output_id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(operation) = operations.iter_mut().find(|operation| {
                    operation.get("operation_id").and_then(Value::as_str) == Some(operation_id)
                }) else {
                    continue;
                };
                let Some(args) = operation.get_mut("args").and_then(Value::as_object_mut) else {
                    continue;
                };
                if !args.get("zone_id").is_some_and(Value::is_string) {
                    let zone_id = recipe_zones
                        .get(&(operation_id.to_string(), output_id.to_string()))
                        .cloned()
                        .unwrap_or_else(|| {
                            format!("zone_repair_{}", &sha256_hex(output_id.as_bytes())[..16])
                        });
                    args.insert("zone_id".into(), Value::String(zone_id));
                    patched_fields.push(format!(
                        "shape_program.operations[{operation_id}].args.zone_id"
                    ));
                }
                if !args.get("material_id").is_some_and(Value::is_string) {
                    args.insert("material_id".into(), Value::String("mat_graphite".into()));
                    patched_fields.push(format!(
                        "shape_program.operations[{operation_id}].args.material_id"
                    ));
                }
            }
        }
        ("closed_manifold", _) | ("surface_provenance_present", _) => {
            return Err(NativeToolFailure::conflict(
                "REPAIR_PATCH_GATE_MISMATCH",
                "Repair patch is not code-owned for the requested failed hard gate.",
            ))
        }
        _ => {
            return Err(NativeToolFailure::conflict(
                "REPAIR_ATTEMPT_SCOPE_INVALID",
                "The requested hard gate has no bounded in-place repair implementation.",
            ))
        }
    }
    if patched_fields.is_empty() {
        return Err(NativeToolFailure::conflict(
            "REPAIR_PATCH_NO_APPLICABLE_FIELD",
            "The bounded repair patch found no authorized failed field to change.",
        ));
    }
    patched_fields.sort();
    patched_fields.dedup();
    Ok(AppliedBoundedRepairPatch {
        gate_id: patch.gate_id.clone(),
        patch_id: patch.patch_id.clone(),
        patched_fields,
    })
}

fn validate_formal_preview_binding(
    artifact: &NativePreviewArtifact,
    project_id: &str,
    turn_id: &str,
    artifact_sha256: &str,
) -> Result<(), ProductToolPortError> {
    artifact.validate()?;
    let provenance = artifact.formal_provenance.as_ref().ok_or_else(|| {
        preview_port_error(
            "NATIVE_FORMAL_PREVIEW_REQUIRED",
            "Legacy compatibility previews cannot use the formal single-result route.",
        )
    })?;
    if provenance.project_id != project_id
        || artifact.turn_id != turn_id
        || artifact.glb_sha256 != artifact_sha256
    {
        return Err(preview_port_error(
            "NATIVE_FORMAL_PREVIEW_IDENTITY_MISMATCH",
            "Formal preview Project, Turn, or GLB identity does not match.",
        ));
    }
    Ok(())
}

impl ProductToolExecutorPort for NativeProductToolExecutor {
    fn read_active_design_snapshot(
        &self,
        project_id: &str,
    ) -> Result<Option<Value>, ProductToolPortError> {
        if project_id.is_empty() {
            return Err(ProductToolPortError::invalid_response(
                "ActiveDesignSnapshot lookup requires a bound Project identity.",
            ));
        }
        let reader = self
            .active_snapshot_reader
            .lock()
            .map_err(|_| {
                ProductToolPortError::invalid_response(
                    "ActiveDesignSnapshot reader mutex is poisoned.",
                )
            })?
            .clone();
        match reader {
            Some(reader) => reader.read_active_design_snapshot(project_id),
            None => Ok(None),
        }
    }

    fn bind_execution_project(
        &self,
        execution_id: &str,
        turn_id: &str,
        project_id: Option<&str>,
    ) -> Result<(), ProductToolPortError> {
        self.bind_execution_project_native(execution_id, turn_id, project_id)
    }

    fn bind_execution_generation_source(
        &self,
        execution_id: &str,
        turn_id: &str,
        source: GenerationSourceBinding,
    ) -> Result<(), ProductToolPortError> {
        self.bind_execution_generation_source_native(execution_id, turn_id, source)
    }

    fn execute(
        &self,
        request: ProductToolExecutionRequest,
        cancellation: CancellationToken,
    ) -> ProductToolPortFuture {
        let executor = self.clone();
        Box::pin(async move { executor.execute_native(request, cancellation).await })
    }

    fn cancel(
        &self,
        cancellation_id: String,
        cancellation_token: String,
    ) -> ProductToolCancelFuture {
        let executor = self.clone();
        Box::pin(async move {
            if !is_stable_id(&cancellation_id) || !is_stable_id(&cancellation_token) {
                return Err(port_error(
                    "NATIVE_PRODUCT_TOOL_CANCELLATION_ID_INVALID",
                    ProductToolPortErrorKind::InvalidResponse,
                    "Cancellation identity must be one bounded stable ID pair.",
                    false,
                ));
            }
            let mut inner = executor.lock_inner()?;
            if let Some(execution_id) = inner
                .execution_by_cancellation
                .get(&cancellation_id)
                .cloned()
            {
                let turn_id = {
                    let run = inner.runs.get_mut(&execution_id).ok_or_else(|| {
                        internal_port_error("Cancellation index referenced no active execution.")
                    })?;
                    if run.cancellation_token != cancellation_token {
                        return Err(port_error(
                            "NATIVE_PRODUCT_TOOL_CANCELLATION_TOKEN_MISMATCH",
                            ProductToolPortErrorKind::InvalidResponse,
                            "Cancellation token does not own this execution.",
                            false,
                        ));
                    }
                    run.generation = run.generation.saturating_add(1);
                    run.run_cancellation.cancel();
                    run.touched_at = Instant::now();
                    run.turn_id.clone()
                };
                inner.discard_preview_for_turn(&turn_id);
                return Ok(true);
            }
            if let Some(previous) = inner.cancel_tombstones.get(&cancellation_id) {
                if previous != &cancellation_token {
                    return Err(port_error(
                        "NATIVE_PRODUCT_TOOL_CANCELLATION_TOKEN_MISMATCH",
                        ProductToolPortErrorKind::InvalidResponse,
                        "Cancellation token conflicts with an earlier tombstone.",
                        false,
                    ));
                }
                return Ok(true);
            }
            inner
                .cancel_tombstones
                .insert(cancellation_id.clone(), cancellation_token);
            inner.tombstone_order.push_back(cancellation_id);
            while inner.cancel_tombstones.len() > executor.config.max_cancel_tombstones {
                if let Some(expired) = inner.tombstone_order.pop_front() {
                    inner.cancel_tombstones.remove(&expired);
                }
            }
            Ok(false)
        })
    }
}

#[derive(Default)]
struct NativeExecutorInner {
    runs: HashMap<String, NativeExecutionRun>,
    order: VecDeque<String>,
    execution_by_cancellation: HashMap<String, String>,
    project_bindings: HashMap<String, NativeProjectBinding>,
    source_bindings: HashMap<String, NativeSourceBinding>,
    cancel_tombstones: HashMap<String, String>,
    tombstone_order: VecDeque<String>,
    preview_artifacts: HashMap<String, NativePreviewArtifactRecord>,
    preview_lru: VecDeque<String>,
    preview_by_turn: HashMap<String, String>,
    preview_retained_bytes: usize,
}

#[derive(Debug, Clone)]
struct NativeProjectBinding {
    turn_id: String,
    project_id: Option<String>,
}

#[derive(Debug, Clone)]
struct NativeSourceBinding {
    turn_id: String,
    source: GenerationSourceBinding,
}

impl NativeExecutorInner {
    fn touch(&mut self, execution_id: &str) {
        if let Some(index) = self.order.iter().position(|item| item == execution_id) {
            self.order.remove(index);
        }
        self.order.push_back(execution_id.to_string());
    }

    fn remove_run(&mut self, execution_id: &str) {
        if let Some(run) = self.runs.remove(execution_id) {
            self.execution_by_cancellation.remove(&run.cancellation_id);
            self.discard_preview_for_turn(&run.turn_id);
        }
        if let Some(index) = self.order.iter().position(|item| item == execution_id) {
            self.order.remove(index);
        }
    }

    fn prune_expired_previews(&mut self, now: Instant) {
        let expired = self
            .preview_artifacts
            .iter()
            .filter_map(|(preview_id, record)| {
                (record.expires_at <= now).then(|| preview_id.clone())
            })
            .collect::<Vec<_>>();
        for preview_id in expired {
            self.remove_preview_record(&preview_id);
        }
    }

    fn insert_preview(
        &mut self,
        artifact: NativePreviewArtifact,
        config: &NativeProductToolExecutorConfig,
    ) -> Result<(), ProductToolPortError> {
        artifact.validate()?;
        self.prune_expired_previews(Instant::now());
        let retained_bytes = artifact.retained_bytes();
        if retained_bytes > config.max_preview_retained_bytes {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_CAPACITY_EXCEEDED",
                "Native preview exceeds its configured transient byte budget.",
            ));
        }
        if let Some(existing) = self.preview_artifacts.get(&artifact.preview_id) {
            if existing.artifact.turn_id != artifact.turn_id {
                return Err(preview_port_error(
                    "NATIVE_PREVIEW_ID_CONFLICT",
                    "Preview identity is already bound to another Turn.",
                ));
            }
        }
        if let Some(previous_id) = self.preview_by_turn.get(&artifact.turn_id).cloned() {
            self.remove_preview_record(&previous_id);
        }
        self.remove_preview_record(&artifact.preview_id);

        let expires_at = Instant::now()
            .checked_add(config.preview_artifact_ttl)
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_CLOCK_INVALID",
                    "Native preview monotonic expiration exceeds its bounded range.",
                )
            })?;
        let preview_id = artifact.preview_id.clone();
        let turn_id = artifact.turn_id.clone();
        self.preview_retained_bytes = self
            .preview_retained_bytes
            .checked_add(retained_bytes)
            .ok_or_else(|| {
                preview_port_error(
                    "NATIVE_PREVIEW_CAPACITY_EXCEEDED",
                    "Native preview byte accounting exceeded its hard bound.",
                )
            })?;
        self.preview_artifacts.insert(
            preview_id.clone(),
            NativePreviewArtifactRecord {
                artifact,
                expires_at,
                retained_bytes,
            },
        );
        self.preview_by_turn.insert(turn_id, preview_id.clone());
        self.preview_lru.push_back(preview_id.clone());

        while self.preview_artifacts.len() > config.max_preview_artifacts
            || self.preview_retained_bytes > config.max_preview_retained_bytes
        {
            let Some(evicted) = self.preview_lru.front().cloned() else {
                break;
            };
            self.remove_preview_record(&evicted);
        }
        if !self.preview_artifacts.contains_key(&preview_id) {
            return Err(preview_port_error(
                "NATIVE_PREVIEW_CAPACITY_EXCEEDED",
                "Native preview could not fit inside the transient LRU registry.",
            ));
        }
        Ok(())
    }

    fn touch_preview(&mut self, preview_id: &str) {
        if let Some(index) = self.preview_lru.iter().position(|item| item == preview_id) {
            self.preview_lru.remove(index);
        }
        self.preview_lru.push_back(preview_id.to_string());
    }

    fn remove_preview_record(&mut self, preview_id: &str) -> Option<NativePreviewArtifact> {
        let record = self.preview_artifacts.remove(preview_id)?;
        self.preview_retained_bytes = self
            .preview_retained_bytes
            .saturating_sub(record.retained_bytes);
        if let Some(index) = self.preview_lru.iter().position(|item| item == preview_id) {
            self.preview_lru.remove(index);
        }
        if self
            .preview_by_turn
            .get(&record.artifact.turn_id)
            .map(String::as_str)
            == Some(preview_id)
        {
            self.preview_by_turn.remove(&record.artifact.turn_id);
        }
        for run in self
            .runs
            .values_mut()
            .filter(|run| run.turn_id == record.artifact.turn_id)
        {
            if run
                .state
                .preview
                .as_ref()
                .and_then(|preview| preview.get("preview_id"))
                .and_then(Value::as_str)
                == Some(preview_id)
            {
                run.state.preview = None;
            }
        }
        Some(record.artifact)
    }

    fn discard_preview_for_turn(&mut self, turn_id: &str) -> bool {
        let Some(preview_id) = self.preview_by_turn.get(turn_id).cloned() else {
            return false;
        };
        self.remove_preview_record(&preview_id).is_some()
    }
}

struct NativePreviewArtifactRecord {
    artifact: NativePreviewArtifact,
    expires_at: Instant,
    retained_bytes: usize,
}

struct NativeExecutionRun {
    turn_id: String,
    project_id: Option<String>,
    cancellation_id: String,
    cancellation_token: String,
    run_cancellation: CancellationToken,
    generation: usize,
    calls_started: u32,
    repair_calls_started: usize,
    state: NativeToolState,
    in_flight: HashSet<String>,
    cached: HashMap<String, CachedNativeResult>,
    idempotency_to_call: HashMap<String, String>,
    touched_at: Instant,
}

#[derive(Clone)]
struct CachedNativeResult {
    idempotency_key: String,
    result: ProductToolExecutionResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InitialBuildIdentity {
    direction_id: String,
    variant_id: Option<String>,
    presentation_profile: String,
    plan_sha256: String,
    style_recipe_sha256: Option<String>,
    profile_sketch_sha256: Option<String>,
    domain_pack_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundedGeometryRepairPatch {
    schema_version: String,
    parent_attempt_id: String,
    parent_gate_report_id: String,
    gate_id: String,
    patch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedBoundedRepairPatch {
    gate_id: String,
    patch_id: String,
    patched_fields: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct NativeToolState {
    project_id: Option<String>,
    turn_id: String,
    generation_source: Option<GenerationSourceBinding>,
    brief: Option<String>,
    domain_pack_id: Option<String>,
    domain_inference: Option<Value>,
    /// Rust-owned, read-only state captured at execution start. It is used
    /// only to bind an AssemblyDelta candidate to the actual active head.
    active_snapshot: Option<Value>,
    style_recipe: Option<Value>,
    profile_sketch: Option<Value>,
    shape_program: Option<Value>,
    plan: Option<Value>,
    geometry_strategy: GeometryStrategy,
    expanded_geometry: Option<RestrictedGeometryInput>,
    recipe_expansion: Option<ReviewedCatalogExpansion>,
    geometry: Option<RestrictedGeometryOutput>,
    build: Option<Value>,
    evaluation: Option<Value>,
    preview: Option<Value>,
    generation_attempt: Option<SingleGenerationAttempt>,
    generation_gate_report: Option<GenerationGateReport>,
    initial_build_identity: Option<InitialBuildIdentity>,
    repair_attempts: Vec<RepairAttempt>,
    repair_attempts_started: usize,
}

#[derive(Debug, Clone)]
struct NativeToolFailure {
    category: ProductToolFailureCategory,
    code: String,
    message: String,
}

impl NativeToolFailure {
    fn new(
        category: ProductToolFailureCategory,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let code = code.into();
        let code = if is_stable_id(&code) {
            code
        } else {
            "NATIVE_PRODUCT_TOOL_FAILED".into()
        };
        let mut message = message.into();
        let folded = message.to_ascii_lowercase();
        if [
            "http://",
            "https://",
            "file://",
            "bearer ",
            "sk-",
            "/users/",
            "/private/",
            "/tmp/",
            "\\users\\",
        ]
        .iter()
        .any(|token| folded.contains(token))
        {
            message = "Native Product Tool execution failed at a restricted boundary.".into();
        }
        if message.chars().count() > 500 {
            message = message.chars().take(500).collect();
        }
        Self {
            category,
            code,
            message,
        }
    }

    fn schema(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ProductToolFailureCategory::Schema, code, message)
    }

    fn conflict(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ProductToolFailureCategory::Conflict, code, message)
    }

    fn unsupported(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ProductToolFailureCategory::Unsupported, code, message)
    }

    fn cancelled() -> Self {
        Self::new(
            ProductToolFailureCategory::Cancelled,
            "PRODUCT_TOOL_CANCELLED",
            "Product Tool result was cancelled; any late result was discarded.",
        )
    }

    fn timeout() -> Self {
        Self::new(
            ProductToolFailureCategory::Timeout,
            "PRODUCT_TOOL_TIMEOUT",
            "Product Tool exceeded the native executor wall-time budget.",
        )
    }
}

fn native_failure_from_geometry(error: RestrictedGeometryError) -> NativeToolFailure {
    let category = match error.kind {
        RestrictedGeometryErrorKind::InvalidInput => ProductToolFailureCategory::Schema,
        RestrictedGeometryErrorKind::Unsupported => ProductToolFailureCategory::Unsupported,
        RestrictedGeometryErrorKind::Cancelled => ProductToolFailureCategory::Cancelled,
        RestrictedGeometryErrorKind::Timeout => ProductToolFailureCategory::Timeout,
        RestrictedGeometryErrorKind::Execution => ProductToolFailureCategory::Execution,
    };
    NativeToolFailure::new(category, error.code, error.message)
}

async fn race_cancellation<F, T>(future: F, cancellation: CancellationToken) -> Result<T, ()>
where
    F: Future<Output = T>,
{
    let mut work = Box::pin(future);
    let mut cancelled = Box::pin(cancellation.cancelled_owned());
    std::future::poll_fn(move |context| {
        if let std::task::Poll::Ready(output) = work.as_mut().poll(context) {
            return std::task::Poll::Ready(Ok(output));
        }
        if cancelled.as_mut().poll(context).is_ready() {
            return std::task::Poll::Ready(Err(()));
        }
        std::task::Poll::Pending
    })
    .await
}

fn completed_result(
    registry: &ProductToolRegistry,
    request: &ProductToolExecutionRequest,
    output: BTreeMap<String, Value>,
    duration_ms: u64,
) -> Result<ProductToolExecutionResult, ProductToolPortError> {
    let definition = registry.definition(&request.tool_name).map_err(|error| {
        port_error(
            "NATIVE_PRODUCT_TOOL_NOT_CODE_OWNED",
            ProductToolPortErrorKind::InvalidResponse,
            error.message,
            false,
        )
    })?;
    Ok(ProductToolExecutionResult {
        schema_version: PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION.into(),
        execution_id: request.execution_id.clone(),
        turn_id: request.turn_id.clone(),
        call_id: request.call_id.clone(),
        tool_id: request.tool_id.clone(),
        cancellation_id: request.cancellation_id.clone(),
        status: ProductToolExecutionStatus::Completed,
        validated_output: Some(ValidatedProductToolPayload {
            schema_id: format!("{}:output", request.tool_id),
            schema_sha256: definition.output_schema_sha256.clone(),
            value: output,
        }),
        failure_category: None,
        error_code: None,
        message: None,
        duration_ms,
        permanent_side_effects: 0,
    })
}

fn failed_result(
    request: &ProductToolExecutionRequest,
    failure: NativeToolFailure,
    duration_ms: u64,
) -> ProductToolExecutionResult {
    let status = if failure.category == ProductToolFailureCategory::Cancelled {
        ProductToolExecutionStatus::Cancelled
    } else {
        ProductToolExecutionStatus::Failed
    };
    ProductToolExecutionResult {
        schema_version: PRODUCT_TOOL_EXECUTION_RESULT_SCHEMA_VERSION.into(),
        execution_id: request.execution_id.clone(),
        turn_id: request.turn_id.clone(),
        call_id: request.call_id.clone(),
        tool_id: request.tool_id.clone(),
        cancellation_id: request.cancellation_id.clone(),
        status,
        validated_output: None,
        failure_category: Some(failure.category),
        error_code: Some(failure.code),
        message: Some(failure.message),
        duration_ms,
        permanent_side_effects: 0,
    }
}

fn cancelled_result(
    request: &ProductToolExecutionRequest,
    duration_ms: u64,
) -> ProductToolExecutionResult {
    failed_result(request, NativeToolFailure::cancelled(), duration_ms)
}

fn port_error(
    code: impl Into<String>,
    kind: ProductToolPortErrorKind,
    message: impl Into<String>,
    recoverable: bool,
) -> ProductToolPortError {
    ProductToolPortError {
        code: code.into(),
        kind,
        message: message.into(),
        recoverable,
    }
}

fn internal_port_error(message: impl Into<String>) -> ProductToolPortError {
    port_error(
        "NATIVE_PRODUCT_TOOL_INTERNAL_STATE_INVALID",
        ProductToolPortErrorKind::InvalidResponse,
        message,
        false,
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainKeywordFixture {
    schema_version: String,
    purpose: String,
    packs: Vec<DomainKeywordPack>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainKeywordPack {
    domain_pack_id: String,
    keywords: Vec<String>,
    synonyms: Vec<String>,
}

fn infer_product_domain(
    arguments: &BTreeMap<String, Value>,
    state: &mut NativeToolState,
) -> Result<Value, NativeToolFailure> {
    let brief = string_argument(arguments, "brief")?.trim().to_string();
    let fixture: DomainKeywordFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../packages/concept-spec/fixtures/domain-inference-keywords.json"
    )))
    .map_err(|_| {
        NativeToolFailure::schema(
            "DOMAIN_INFERENCE_FIXTURE_INVALID",
            "Bundled domain inference evidence is invalid.",
        )
    })?;
    if fixture.schema_version != "DomainInferenceKeywordFixture@1"
        || fixture.purpose.is_empty()
        || fixture.packs.len() != 4
    {
        return Err(NativeToolFailure::schema(
            "DOMAIN_INFERENCE_FIXTURE_INVALID",
            "Bundled domain inference evidence has an unsupported shape.",
        ));
    }
    let normalized = brief.to_lowercase();
    let mut candidates = Vec::new();
    let mut matched_terms = Vec::new();
    let mut seen_packs = BTreeSet::new();
    for pack in fixture.packs {
        if !supported_domain(&pack.domain_pack_id)
            || !seen_packs.insert(pack.domain_pack_id.clone())
        {
            return Err(NativeToolFailure::schema(
                "DOMAIN_INFERENCE_FIXTURE_INVALID",
                "Bundled domain inference evidence contains an invalid pack.",
            ));
        }
        let mut terms = pack.keywords;
        terms.extend(pack.synonyms);
        let matches = terms
            .into_iter()
            .filter(|term| !term.is_empty() && normalized.contains(&term.to_lowercase()))
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            candidates.push(pack.domain_pack_id);
            matched_terms.extend(matches);
        }
    }
    matched_terms.sort();
    matched_terms.dedup();
    let (status, domain_pack_id) = match candidates.len() {
        1 => ("recognized", Some(candidates[0].clone())),
        2.. => ("ambiguous", None),
        _ => ("unsupported", None),
    };
    let payload = json!({
        "schema_version": "DomainInferenceResult@1",
        "status": status,
        "domain_pack_id": domain_pack_id,
        "candidate_domain_pack_ids": candidates,
        "matched_terms": matched_terms
    });
    state.brief = Some(brief);
    state.domain_pack_id = domain_pack_id;
    state.domain_inference = Some(payload.clone());
    Ok(payload)
}

fn research_approved_references(arguments: &BTreeMap<String, Value>) -> Value {
    json!({
        "source_scope": arguments.get("source_scope").cloned().unwrap_or(Value::Null),
        "matches": [],
        "network_call_made": false,
        "message": "No reviewed bundled reference matched; no network or URL was accessed."
    })
}

#[derive(Clone, Copy)]
struct StyleChoice {
    suffix: &'static str,
    token_id: &'static str,
    display_name: &'static str,
    proportion_profile: &'static str,
    edge_language: &'static str,
    surface_tension: &'static str,
    detail_density: &'static str,
    symmetry: &'static str,
    material_palette: &'static str,
    lighting_profile: &'static str,
    phrases: &'static [&'static str],
    role_selector: &'static str,
    path: &'static str,
    step_delta: i32,
}

const STYLE_CHOICES: [StyleChoice; 4] = [
    StyleChoice {
        suffix: "compact",
        token_id: "style_compact_rounded",
        display_name: "compact rounded",
        proportion_profile: "compact",
        edge_language: "soft",
        surface_tension: "relaxed",
        detail_density: "low",
        symmetry: "bilateral",
        material_palette: "clean_coating",
        lighting_profile: "soft_studio",
        phrases: &["紧凑", "短一些", "compact", "rounded"],
        role_selector: "primary_form",
        path: "transform.scale.x",
        step_delta: -1,
    },
    StyleChoice {
        suffix: "sleek",
        token_id: "style_aerodynamic_sleek",
        display_name: "aerodynamic sleek",
        proportion_profile: "elongated",
        edge_language: "controlled",
        surface_tension: "taut",
        detail_density: "low",
        symmetry: "bilateral",
        material_palette: "technical_composite",
        lighting_profile: "concept_contrast",
        phrases: &["修长", "流线", "sleek", "aerodynamic"],
        role_selector: "primary_form",
        path: "transform.scale.x",
        step_delta: 1,
    },
    StyleChoice {
        suffix: "substantial",
        token_id: "style_industrial_substantial",
        display_name: "industrial substantial",
        proportion_profile: "substantial",
        edge_language: "crisp",
        surface_tension: "neutral",
        detail_density: "medium",
        symmetry: "assembly_driven",
        material_palette: "mixed_industrial",
        lighting_profile: "cad_neutral",
        phrases: &["厚重", "稳重", "industrial", "substantial"],
        role_selector: "primary_form",
        path: "transform.scale.y",
        step_delta: 1,
    },
    StyleChoice {
        suffix: "clean",
        token_id: "style_clean_balanced",
        display_name: "clean balanced",
        proportion_profile: "balanced",
        edge_language: "controlled",
        surface_tension: "neutral",
        detail_density: "low",
        symmetry: "assembly_driven",
        material_palette: "dark_metal",
        lighting_profile: "cad_neutral",
        phrases: &["简洁", "协调", "clean", "balanced"],
        role_selector: "secondary_form",
        path: "transform.scale.z",
        step_delta: -1,
    },
];

fn select_style_recipe(
    arguments: &BTreeMap<String, Value>,
    state: &mut NativeToolState,
) -> Result<Value, NativeToolFailure> {
    let domain = string_argument(arguments, "domain_pack_id")?;
    if !supported_domain(domain) {
        return Err(NativeToolFailure::unsupported(
            "STYLE_RECIPE_DOMAIN_UNAVAILABLE",
            "No reviewed semantic proportion recipe exists for this domain.",
        ));
    }
    if state
        .domain_pack_id
        .as_deref()
        .is_some_and(|bound| bound != domain)
    {
        return Err(NativeToolFailure::conflict(
            "ACTION_LOOP_DOMAIN_CONFLICT",
            "Style recipe changed the domain bound to this execution.",
        ));
    }
    let intent = string_argument(arguments, "intent")?.to_lowercase();
    let matched = STYLE_CHOICES.iter().copied().find(|choice| {
        choice
            .phrases
            .iter()
            .any(|phrase| intent.contains(&phrase.to_lowercase()))
    });
    let choice = matched.unwrap_or(STYLE_CHOICES[0]);
    let role_selector = semantic_role_selector(domain, choice.role_selector);
    let payload = json!({
        "style_token": {
            "schema_version": "MechanicalStyleToken@1",
            "token_id": choice.token_id,
            "display_name": choice.display_name,
            "description": "Bounded non-engineering exterior visual language.",
            "proportion_profile": choice.proportion_profile,
            "edge_language": choice.edge_language,
            "surface_tension": choice.surface_tension,
            "detail_density": choice.detail_density,
            "symmetry": choice.symmetry,
            "material_palette": choice.material_palette,
            "lighting_profile": choice.lighting_profile,
            "allowed_domains": [
                "pack_future_weapon_prop",
                "pack_vehicle_concept",
                "pack_aircraft_concept",
                "pack_robotic_arm_concept"
            ]
        },
        "recipe": {
            "schema_version": "DomainSemanticProportionRecipe@1",
            "recipe_id": format!("proportion_{}_{}", domain_suffix(domain), choice.suffix),
            "domain_pack_id": domain,
            "style_token_id": choice.token_id,
            "display_name": choice.display_name,
            "description": "One bounded conceptual proportion adjustment; no manufacturing dimension.",
            "intent_phrases": choice.phrases,
            "adjustments": [{
                "role_selector": role_selector,
                "path": choice.path,
                "step_delta": choice.step_delta
            }]
        },
        "fallback_used": matched.is_none()
    });
    state
        .domain_pack_id
        .get_or_insert_with(|| domain.to_string());
    state.style_recipe = Some(payload.clone());
    Ok(payload)
}

fn author_profile_sketch(
    arguments: &BTreeMap<String, Value>,
    state: &mut NativeToolState,
) -> Result<Value, NativeToolFailure> {
    let profile = arguments.get("profile_sketch").cloned().ok_or_else(|| {
        NativeToolFailure::schema("PROFILE_SKETCH_MISSING", "ProfileSketch is missing.")
    })?;
    validate_profile_sketch_value(&profile)?;
    state.profile_sketch = Some(profile.clone());
    Ok(json!({"profile_sketch": profile, "validated": true}))
}

fn author_shape_program(
    arguments: &BTreeMap<String, Value>,
    state: &mut NativeToolState,
) -> Result<Value, NativeToolFailure> {
    let program = arguments.get("shape_program").cloned().ok_or_else(|| {
        NativeToolFailure::schema("SHAPE_PROGRAM_MISSING", "ShapeProgram is missing.")
    })?;
    validate_shape_program_value(&program)?;
    state.shape_program = Some(program.clone());
    Ok(json!({"shape_program": program, "validated": true}))
}

fn plan_complete_concept(
    arguments: &BTreeMap<String, Value>,
    state: &mut NativeToolState,
) -> Result<Value, NativeToolFailure> {
    if state.domain_inference.as_ref().is_some_and(|inference| {
        inference.get("status").and_then(Value::as_str) != Some("recognized")
    }) {
        return Err(NativeToolFailure::conflict(
            "ACTION_LOOP_DOMAIN_UNRESOLVED",
            "Ambiguous or unsupported domain inference must be resolved before planning.",
        ));
    }
    let mut plan = arguments
        .get("plan")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| {
            NativeToolFailure::schema("CONCEPT_PLAN_MISSING", "Concept plan is missing.")
        })?;
    let plan_domain = plan
        .get("domain_pack_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            NativeToolFailure::schema(
                "CONCEPT_PLAN_DOMAIN_MISSING",
                "Concept plan domain is missing.",
            )
        })?;
    if !supported_domain(plan_domain) {
        return Err(NativeToolFailure::unsupported(
            "CONCEPT_PLAN_DOMAIN_UNSUPPORTED",
            "Concept plan domain is not enabled.",
        ));
    }
    if state
        .domain_pack_id
        .as_deref()
        .is_some_and(|bound| bound != plan_domain)
    {
        return Err(NativeToolFailure::conflict(
            "ACTION_LOOP_DOMAIN_CONFLICT",
            "Concept plan changed the domain bound to this execution.",
        ));
    }
    let plan_domain = plan_domain.to_string();
    let plan_brief = plan
        .get("brief")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|brief| !brief.is_empty())
        .ok_or_else(|| {
            NativeToolFailure::schema(
                "CONCEPT_PLAN_BRIEF_MISSING",
                "Concept plan must preserve one non-empty bounded brief.",
            )
        })?
        .to_string();
    if let Some(brief) = state.brief.as_ref() {
        plan.insert("brief".into(), Value::String(brief.clone()));
    } else {
        // Deterministic/offline fixtures may start directly at reviewed plan
        // normalization.  The plan brief is then the only Rust-validated
        // intent available, and becomes the immutable execution brief.
        state.brief = Some(plan_brief);
    }
    // Provider provenance belongs to the Rust Turn/trace, not Product Tool
    // state or the restricted geometry boundary.
    plan.remove("provider_id");
    plan.remove("model");
    // `shape_program_ready` is the existing, schema-owned reviewed-plan
    // switch for the exceptional authored route.  Its absence is normalized
    // to the C105 component-recipe default; merely having called
    // `author_shape_program` never changes this route.
    let geometry_strategy = match plan.get("shape_program_ready") {
        Some(Value::Bool(true)) => GeometryStrategy::AuthoredShapeProgram,
        Some(Value::Bool(false)) | None => GeometryStrategy::ComponentRecipe,
        Some(_) => {
            return Err(NativeToolFailure::schema(
                "CONCEPT_PLAN_GEOMETRY_STRATEGY_INVALID",
                "Concept plan shape_program_ready must be a boolean.",
            ))
        }
    };
    plan.insert(
        "shape_program_ready".into(),
        Value::Bool(geometry_strategy == GeometryStrategy::AuthoredShapeProgram),
    );
    if let Some(delta) = plan.get("assembly_delta").cloned() {
        if plan_domain != "pack_robotic_arm_concept" {
            return Err(NativeToolFailure::unsupported(
                "ASSEMBLY_DELTA_DOMAIN_UNSUPPORTED",
                "AssemblyDelta continuation is currently reviewed for robotic arms only.",
            ));
        }
        let active_version = state.active_snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .get("active_design")
                .and_then(Value::as_object)
                .filter(|design| {
                    design.get("source").and_then(Value::as_str) == Some("agent_asset")
                })
                .and_then(|design| design.get("asset_version_id"))
                .and_then(Value::as_str)
        });
        if active_version.is_none() {
            return Err(NativeToolFailure::conflict(
                "ASSEMBLY_DELTA_NOT_ALLOWED_ON_INITIAL_SYNTHESIS",
                "AssemblyDelta is only valid when a Rust-owned active Agent asset exists.",
            ));
        }
        lower_assembly_delta(&delta).map_err(|error| {
            NativeToolFailure::schema(
                "ASSEMBLY_DELTA_INVALID",
                format!("AssemblyDeltaProgram validation failed ({})", error.code()),
            )
        })?;
        if let Some(active_version) = active_version {
            let requested_base = delta.get("base_asset_version_id").and_then(Value::as_str);
            if requested_base != Some(active_version) {
                return Err(NativeToolFailure::conflict(
                    "ASSEMBLY_DELTA_BASE_STALE",
                    "AssemblyDelta must target the Rust-owned active Agent asset version.",
                ));
            }
        }
    }
    // C110B: a robotic-arm intent is a bounded visual vocabulary. Rust owns
    // its validation and chooses only from the reviewed C106 recipe family.
    // Offline deterministic fixtures may still omit this field for backwards
    // compatibility, but a real DeepSeek execution must provide the intent;
    // otherwise the Provider would silently fall back to one fixed showcase.
    if plan_domain == "pack_robotic_arm_concept" {
        let requires_provider_intent = state.generation_source.as_ref().is_some_and(|source| {
            matches!(
                source.source_kind,
                GenerationSourceKind::DeepseekNetworkAttempted
            )
        });
        if requires_provider_intent && !plan.get("arm_design_intent").is_some_and(Value::is_object)
        {
            return Err(NativeToolFailure::schema(
                "ARM_DESIGN_INTENT_REQUIRED",
                "DeepSeek robotic-arm plans must include a bounded ArmDesignIntent@1 object.",
            ));
        }
        if let Some(intent) = plan.get("arm_design_intent").cloned() {
            let lowering = forgecad_core::lower_arm_design_intent(&intent).map_err(|error| {
                NativeToolFailure::schema(
                    "ARM_DESIGN_INTENT_INVALID",
                    format!("ArmDesignIntent validation failed ({})", error.code()),
                )
            })?;
            if lowering.status == "unsupported" {
                return Err(NativeToolFailure::unsupported(
                    "ARM_INTENT_ARCHITECTURE_UNSUPPORTED",
                    "This arm architecture has no reviewed recipe family yet.",
                ));
            }
            let lowering = serde_json::to_value(&lowering).map_err(|_| {
                NativeToolFailure::schema(
                    "ARM_RECIPE_LOWERING_SERIALIZATION_FAILED",
                    "Arm recipe lowering could not be recorded.",
                )
            })?;
            plan.insert("arm_recipe_lowering".into(), lowering);

            // The provider's visual intent must affect the actual reviewed
            // Recipe expansion, not remain metadata on the plan.  The action
            // loop normally calls select_style_recipe first; this fallback
            // keeps direct/native and older provider loops honest by deriving
            // one code-owned Style Token from the bounded proportion profile.
            if state.style_recipe.is_none() {
                let proportion_profile = intent
                    .get("proportion_profile")
                    .and_then(Value::as_str)
                    .unwrap_or("balanced");
                let intent_text = match proportion_profile {
                    "compact" => "compact rounded",
                    "long_reach" | "slender" => "aerodynamic sleek",
                    "heavy_base" => "industrial substantial",
                    _ => "clean balanced",
                };
                let mut style_arguments = BTreeMap::new();
                style_arguments.insert("domain_pack_id".into(), Value::String(plan_domain.clone()));
                style_arguments.insert("intent".into(), Value::String(intent_text.into()));
                select_style_recipe(&style_arguments, state)?;
            }
        }
    }
    let normalized = Value::Object(plan);
    reject_forbidden_json(&normalized)?;
    state.domain_pack_id = Some(plan_domain);
    state.geometry_strategy = geometry_strategy;
    state.plan = Some(normalized.clone());
    Ok(json!({"plan": normalized, "accepted": true}))
}

fn compile_readback_candidate(state: &NativeToolState) -> Result<Value, NativeToolFailure> {
    let geometry = state.geometry.as_ref().ok_or_else(|| {
        NativeToolFailure::conflict(
            "ACTION_LOOP_SHAPE_PROGRAM_REQUIRED",
            "A restricted geometry build is required before compile/readback.",
        )
    })?;
    Ok(json!({
        "runtime_manifest_version": geometry.readback.runtime_manifest_version,
        "artifact_profile_id": geometry.readback.artifact_profile_id,
        "shape_program_sha256": geometry.readback.shape_program_sha256,
        "glb_byte_size": geometry.readback.glb_byte_size,
        "triangle_count": geometry.readback.triangle_count,
        "bounds_mm": geometry.readback.bounds_mm,
        "mesh_count": geometry.readback.mesh_count,
        "primitive_count": geometry.readback.primitive_count,
        "material_count": geometry.readback.material_count,
        "closed_manifold": geometry.readback.closed_manifold,
        "surface_provenance_present": geometry.readback.surface_provenance_present,
        "glb_sha256": geometry.glb_sha256,
        "evidence_source": "restricted_geometry_glb_readback"
    }))
}

fn render_candidate_views(state: &NativeToolState) -> Result<Value, NativeToolFailure> {
    let geometry = state.geometry.as_ref().ok_or_else(|| {
        NativeToolFailure::conflict(
            "ACTION_LOOP_COMPILE_READBACK_REQUIRED",
            "Actual restricted geometry readback is required before rendering.",
        )
    })?;
    Ok(json!({
        "view_ids": geometry.views.keys().cloned().collect::<Vec<_>>(),
        "view_sha256": geometry.view_sha256,
        "renderer_id": geometry.renderer_id
    }))
}

fn evaluate_candidate(state: &mut NativeToolState) -> Result<Value, NativeToolFailure> {
    let geometry = state.geometry.as_ref().ok_or_else(|| {
        NativeToolFailure::conflict(
            "ACTION_LOOP_COMPILE_READBACK_REQUIRED",
            "Actual restricted geometry readback is required before evaluation.",
        )
    })?;
    let four_views_read_back = geometry
        .views
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        == REQUIRED_VIEWS.into_iter().collect::<BTreeSet<_>>();
    let maybe_attempt = state.project_id.as_deref().map(|project_id| {
        if let Some(existing) = state.generation_attempt.as_ref() {
            return existing.clone();
        }
        let plan = state.plan.as_ref().expect("geometry build requires a plan");
        let domain_pack_id = plan
            .get("domain_pack_id")
            .and_then(Value::as_str)
            .expect("plan domain validated before geometry build");
        let core_intent = state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.candidate_sha256.as_deref())
            .map(str::to_string)
            .or_else(|| state.profile_sketch.as_ref().map(canonical_json))
            .unwrap_or_else(|| "reviewed_component_recipe".into());
        let recipe_provenance_sha256 = state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.candidate_sha256.clone());
        SingleGenerationAttempt {
            schema_version: SINGLE_GENERATION_ATTEMPT_SCHEMA_VERSION.into(),
            attempt_id: format!(
                "attempt_{}",
                &sha256_hex(
                    format!(
                        "{}:{project_id}:{}",
                        geometry.glb_sha256, geometry.topology_hash
                    )
                    .as_bytes()
                )[..24]
            ),
            turn_id: state.turn_id.clone(),
            project_id: project_id.to_string(),
            attempt_kind: forgecad_core::GenerationAttemptKind::Initial,
            parent_attempt_id: None,
            brief_sha256: sha256_hex(state.brief.as_deref().unwrap_or_default().as_bytes()),
            domain_pack_id: domain_pack_id.to_string(),
            domain_pack_sha256: sha256_hex(domain_pack_id.as_bytes()),
            core_recipe_or_profile_sha256: sha256_hex(core_intent.as_bytes()),
            runtime_manifest_sha256: sha256_hex(
                geometry.readback.runtime_manifest_version.as_bytes(),
            ),
            shape_program_sha256: geometry.readback.shape_program_sha256.clone(),
            recipe_provenance_sha256,
        }
    });
    let report = maybe_attempt
        .as_ref()
        .map(|attempt| evaluate_native_v003_gate(state, attempt, geometry, four_views_read_back))
        .transpose()?;
    let checks = report.as_ref().map_or_else(
        || json!({"v003_native_evidence": "unavailable"}),
        |report| serde_json::to_value(&report.checks).unwrap_or(Value::Null),
    );
    let hard_gate_passed = report.as_ref().is_some_and(GenerationGateReport::is_passed);
    let payload = json!({
        "hard_gate_passed": hard_gate_passed,
        "checks": checks,
        "generation_gate_report": report,
        "evidence_source": "native_v003_gate_profile_v2"
    });
    state.generation_attempt = maybe_attempt;
    state.generation_gate_report = report;
    state.evaluation = Some(payload.clone());
    Ok(payload)
}

fn evaluate_native_v003_gate(
    state: &NativeToolState,
    attempt: &SingleGenerationAttempt,
    geometry: &RestrictedGeometryOutput,
    four_views_read_back: bool,
) -> Result<GenerationGateReport, NativeToolFailure> {
    let plan = state.plan.as_ref().ok_or_else(|| {
        NativeToolFailure::conflict(
            "NATIVE_V003_PLAN_EVIDENCE_MISSING",
            "A formal V003 evaluation requires the reviewed plan bound to this execution.",
        )
    })?;
    let direction_id = state
        .initial_build_identity
        .as_ref()
        .map(|identity| identity.direction_id.as_str())
        .ok_or_else(|| {
            NativeToolFailure::conflict(
                "NATIVE_V003_DIRECTION_EVIDENCE_MISSING",
                "A formal V003 evaluation requires the initial selected direction identity.",
            )
        })?;
    let direction = plan
        .get("directions")
        .and_then(Value::as_array)
        .and_then(|directions| {
            directions.iter().find(|direction| {
                direction.get("direction_id").and_then(Value::as_str) == Some(direction_id)
            })
        });
    let render_source = sha256_hex(
        canonical_json(&json!({
            "renderer_id": geometry.renderer_id,
            "glb_sha256": geometry.glb_sha256,
            "views": geometry.view_sha256,
        }))
        .as_bytes(),
    );
    let compile_source = geometry.readback.compile_readback_sha256.clone();
    let brief_source = sha256_hex(
        canonical_json(&json!({
            "brief": state.brief,
            "plan_brief": plan.get("brief"),
            "plan_id": plan.get("plan_id"),
            "direction_id": direction_id,
        }))
        .as_bytes(),
    );
    let semantic_source = sha256_hex(
        canonical_json(&json!({
            "binding": state.recipe_expansion.as_ref().and_then(|value| value.semantic_proportion_binding.as_ref()),
            "shape_program_sha256": geometry.readback.shape_program_sha256,
            "style_recipe": state.style_recipe,
        }))
        .as_bytes(),
    );
    let role_source = sha256_hex(
        canonical_json(&json!({
            "domain_pack_id": attempt.domain_pack_id,
            "direction_id": direction_id,
            "assembly_graph": state.recipe_expansion.as_ref().and_then(|value| value.expanded_assembly_graph.as_ref()),
            "component_recipe_instances": state.recipe_expansion.as_ref().and_then(|value| value.component_recipe_instances.as_ref()),
            "actual_outputs": state.expanded_geometry.as_ref().map(|value| &value.shape_program),
        }))
        .as_bytes(),
    );
    let material_source = sha256_hex(
        canonical_json(&json!({
            "compile_readback_sha256": compile_source,
            "artifact_profile_id": geometry.readback.artifact_profile_id,
            "material_zone_count": geometry.readback.material_zone_count,
            "visual_texture_set_count": geometry.readback.visual_texture_set_count,
            "visual_texture_map_count": geometry.readback.visual_texture_map_count,
            "visual_texture_provenance_verified": geometry.readback.visual_texture_provenance_verified,
        }))
        .as_bytes(),
    );
    let editability_source = sha256_hex(
        canonical_json(&json!({
            "candidate_sha256": state.recipe_expansion.as_ref().and_then(|value| value.candidate_sha256.as_ref()),
            "shape_program_sha256": state.recipe_expansion.as_ref().and_then(|value| value.expanded_shape_program_sha256.as_ref()),
            "assembly_graph": state.recipe_expansion.as_ref().and_then(|value| value.expanded_assembly_graph.as_ref()),
            "instances": state.recipe_expansion.as_ref().and_then(|value| value.component_recipe_instances.as_ref()),
        }))
        .as_bytes(),
    );
    let (execution_source, execution_outcome, execution_summary) = match state.generation_source.as_ref() {
        Some(source) => (
            sha256_hex(canonical_json(&json!({
                "provider_id": source.provider_id,
                "source_kind": source.source_kind,
                "project_id": attempt.project_id,
                "turn_id": attempt.turn_id,
                "attempt_id": attempt.attempt_id,
            })).as_bytes()),
            VerificationOutcome::Pass,
            match source.source_kind {
                GenerationSourceKind::OfflineDeterministic => "Trusted deterministic offline generation source is bound by the native lifecycle.",
                GenerationSourceKind::DeepseekNetworkAttempted => "Trusted DeepSeek network-attempt source is bound by the native lifecycle, not by model tool arguments.",
            },
        ),
        None => (
            sha256_hex(canonical_json(&json!({
                "project_id": attempt.project_id,
                "turn_id": attempt.turn_id,
                "attempt_id": attempt.attempt_id,
                "source": "unbound",
            })).as_bytes()),
            VerificationOutcome::Undetermined,
            "No trusted generation source is bound to this execution.",
        ),
    };

    let brief_outcome = match (
        state.brief.as_deref(),
        plan.get("brief").and_then(Value::as_str),
        direction,
    ) {
        (Some(brief), Some(plan_brief), Some(direction))
            if !brief.trim().is_empty()
                && brief == plan_brief
                && direction
                    .get("title")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
                && direction
                    .get("summary")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
                && direction
                    .get("silhouette")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
                && direction
                    .get("material_direction")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
                && direction
                    .get("primary_part_roles")
                    .and_then(Value::as_array)
                    .is_some_and(|roles| !roles.is_empty()) =>
        {
            VerificationOutcome::Pass
        }
        (Some(_), Some(_), Some(_)) => VerificationOutcome::Fail,
        _ => VerificationOutcome::Undetermined,
    };
    let semantic_outcome = state
        .recipe_expansion
        .as_ref()
        .and_then(|value| value.semantic_proportion_binding.as_ref())
        .map_or(VerificationOutcome::Undetermined, |binding| {
            if binding.shape_program_sha256 == geometry.readback.shape_program_sha256
                && state.style_recipe.as_ref().is_some_and(|style| {
                    binding.style_recipe_sha256 == sha256_hex(canonical_json(style).as_bytes())
                })
            {
                VerificationOutcome::Pass
            } else {
                VerificationOutcome::Fail
            }
        });
    let role_outcome = match (
        state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.expanded_assembly_graph.as_ref()),
        state
            .recipe_expansion
            .as_ref()
            .and_then(|value| value.component_recipe_instances.as_ref()),
        state
            .expanded_geometry
            .as_ref()
            .map(|value| &value.shape_program),
    ) {
        (Some(graph), Some(instances), Some(shape_program)) => {
            domain_role_coverage_outcome(graph, instances, shape_program, &attempt.domain_pack_id)
        }
        _ => VerificationOutcome::Undetermined,
    };
    let material_outcome = if geometry.readback.artifact_profile_id != "production_concept" {
        VerificationOutcome::Fail
    } else if geometry.readback.visual_texture_provenance_verified
        && geometry.readback.material_zone_count > 0
        && geometry.readback.visual_texture_set_count > 0
        && geometry.readback.visual_texture_map_count
            >= geometry.readback.visual_texture_set_count.saturating_mul(5)
    {
        VerificationOutcome::Pass
    } else {
        VerificationOutcome::Fail
    };
    let editability_outcome =
        state
            .recipe_expansion
            .as_ref()
            .map_or(VerificationOutcome::Undetermined, |expansion| {
                if expansion.expanded_assembly_graph.is_some()
                    && expansion.component_recipe_instances.is_some()
                    && expansion.candidate_sha256.is_some()
                    && expansion.expanded_shape_program_sha256.as_deref()
                        == Some(geometry.readback.shape_program_sha256.as_str())
                {
                    VerificationOutcome::Pass
                } else {
                    VerificationOutcome::Fail
                }
            });
    let r006_outcome = if four_views_read_back
        && geometry
            .views
            .iter()
            .all(|(id, bytes)| geometry.view_sha256.get(id) == Some(&sha256_hex(bytes)))
    {
        VerificationOutcome::Pass
    } else {
        VerificationOutcome::Undetermined
    };
    let evidence = vec![
        native_gate_evidence("has_triangles", if geometry.readback.triangle_count > 0 { VerificationOutcome::Pass } else { VerificationOutcome::Fail }, NativeGateEvidenceSource::RestrictedGeometryGlbReadback, &compile_source, "GLB compiler readback contains triangles."),
        native_gate_evidence("has_meshes", if geometry.readback.mesh_count > 0 { VerificationOutcome::Pass } else { VerificationOutcome::Fail }, NativeGateEvidenceSource::RestrictedGeometryGlbReadback, &compile_source, "GLB compiler readback contains a mesh."),
        native_gate_evidence("four_views_read_back", if four_views_read_back { VerificationOutcome::Pass } else { VerificationOutcome::Undetermined }, NativeGateEvidenceSource::DeterministicConceptRenderReadback, &render_source, "Deterministic renderer returned the required four views."),
        native_gate_evidence("closed_manifold", if geometry.readback.closed_manifold { VerificationOutcome::Pass } else { VerificationOutcome::Fail }, NativeGateEvidenceSource::RestrictedGeometryGlbReadback, &compile_source, "Compiler readback verified closed manifold state."),
        native_gate_evidence("surface_provenance_present", if geometry.readback.surface_provenance_present { VerificationOutcome::Pass } else { VerificationOutcome::Fail }, NativeGateEvidenceSource::RestrictedGeometryGlbReadback, &compile_source, "Compiler readback verified surface provenance."),
        native_gate_evidence("glb_hash_verified", if geometry.glb_sha256 == sha256_hex(&geometry.glb_bytes) { VerificationOutcome::Pass } else { VerificationOutcome::Fail }, NativeGateEvidenceSource::RestrictedGeometryGlbReadback, &compile_source, "Returned GLB bytes match the compiler-bound SHA-256."),
        native_gate_evidence("brief_coverage", brief_outcome, NativeGateEvidenceSource::BriefCoverageResolver, &brief_source, "Reviewed plan preserves the exact bounded user brief and selected exterior coverage fields."),
        native_gate_evidence("semantic_proportion_bound", semantic_outcome, NativeGateEvidenceSource::SemanticProportionResolver, &semantic_source, "Reviewed Style Token proportion adjustment is bound to the compiled Recipe ShapeProgram."),
        native_gate_evidence("domain_role_coverage", role_outcome, NativeGateEvidenceSource::DomainRoleResolver, &role_source, "Every persistent Recipe AssemblyGraph part is bound to an exact ShapeProgram operation/output/role triple and a same-domain Recipe instance."),
        native_gate_evidence("material_texture_provenance", material_outcome, NativeGateEvidenceSource::ProductionMaterialReadback, &material_source, "Production compiler readback binds material zones to complete built-in PBR texture sets."),
        native_gate_evidence("editability_evidence", editability_outcome, NativeGateEvidenceSource::RecipeAssemblyReadback, &editability_source, "Recipe assembly graph, instance provenance, candidate and exact compiled ShapeProgram are bound together."),
        native_gate_evidence("r006_same_source_views", r006_outcome, NativeGateEvidenceSource::DeterministicConceptRenderReadback, &render_source, "All four concept views are fingerprinted with the same compiled GLB and renderer identity."),
        native_gate_evidence("generation_source_marked", execution_outcome, NativeGateEvidenceSource::GenerationExecutionTrace, &execution_source, execution_summary),
    ];
    let evaluation = evaluate_native_v003_gate_profile_v2(
        NativeGenerationGateBinding {
            gate_report_id: format!("gate_{}", &sha256_hex(format!("{}:{}", attempt.attempt_id, geometry.glb_sha256).as_bytes())[..24]),
            attempt_id: attempt.attempt_id.clone(),
            glb_sha256: geometry.glb_sha256.clone(),
            compile_readback_id: format!("readback_{}", &geometry.glb_sha256[..24]),
            render_fingerprint: format!("render_{}", &render_source[..24]),
            summary: "Rust-owned V003 profile v2 evaluated trusted compiler, renderer, plan, Recipe, material, and execution evidence.".into(),
        },
        evidence,
    )
    .map_err(|error| NativeToolFailure::new(ProductToolFailureCategory::Execution, error.code(), error.to_string()))?;
    Ok(evaluation.report)
}

fn native_gate_evidence(
    gate_id: &str,
    outcome: VerificationOutcome,
    source: NativeGateEvidenceSource,
    source_sha256: &str,
    summary: &str,
) -> NativeGenerationGateEvidence {
    NativeGenerationGateEvidence {
        schema_version: NATIVE_GENERATION_GATE_EVIDENCE_SCHEMA_VERSION.into(),
        gate_id: gate_id.into(),
        outcome,
        source,
        source_sha256: source_sha256.into(),
        summary: summary.into(),
    }
}

fn domain_role_coverage_outcome(
    graph: &Value,
    instances: &Value,
    shape_program: &Value,
    expected_domain_pack_id: &str,
) -> VerificationOutcome {
    let Some(parts) = graph
        .get("parts")
        .and_then(Value::as_array)
        .filter(|parts| !parts.is_empty())
    else {
        return VerificationOutcome::Fail;
    };
    let Some(recipe_instances) = instances.as_array().filter(|items| !items.is_empty()) else {
        return VerificationOutcome::Fail;
    };
    if recipe_instances.iter().any(|instance| {
        instance.get("domain_pack_id").and_then(Value::as_str) != Some(expected_domain_pack_id)
            || instance
                .get("instance_id")
                .and_then(Value::as_str)
                .is_none()
    }) {
        return VerificationOutcome::Fail;
    }
    let Some(outputs) = shape_program.get("outputs").and_then(Value::as_array) else {
        return VerificationOutcome::Fail;
    };
    let output_bindings = outputs
        .iter()
        .filter_map(|output| {
            Some((
                output.get("operation_id")?.as_str()?,
                output.get("output_id")?.as_str()?,
                output.get("part_role")?.as_str()?,
            ))
        })
        .collect::<BTreeSet<_>>();
    if output_bindings.is_empty() {
        return VerificationOutcome::Fail;
    }
    for part in parts {
        let Some(operation_id) = part.get("operation_id").and_then(Value::as_str) else {
            return VerificationOutcome::Fail;
        };
        let Some(output_id) = part.get("output_id").and_then(Value::as_str) else {
            return VerificationOutcome::Fail;
        };
        let Some(role) = part.get("role").and_then(Value::as_str) else {
            return VerificationOutcome::Fail;
        };
        let Some(instance_id) = part.get("recipe_instance_id").and_then(Value::as_str) else {
            return VerificationOutcome::Fail;
        };
        let shape_role =
            shape_program_role_selector_for_program(expected_domain_pack_id, role, shape_program);
        if !output_bindings.contains(&(operation_id, output_id, shape_role))
            || !recipe_instances.iter().any(|instance| {
                instance.get("instance_id").and_then(Value::as_str) == Some(instance_id)
            })
        {
            return VerificationOutcome::Fail;
        }
    }
    VerificationOutcome::Pass
}

fn prepare_candidate_preview(
    turn_id: &str,
    state: &mut NativeToolState,
) -> Result<Value, NativeToolFailure> {
    if state.preview.is_some() {
        return Err(NativeToolFailure::conflict(
            "NATIVE_PREVIEW_ALREADY_PREPARED",
            "This execution already has its single preview descriptor.",
        ));
    }
    let evaluation = state.evaluation.as_ref().ok_or_else(|| {
        NativeToolFailure::conflict(
            "ACTION_LOOP_EVALUATION_REQUIRED",
            "Hard evidence evaluation is required before preview.",
        )
    })?;
    if evaluation.get("hard_gate_passed") != Some(&Value::Bool(true)) {
        return Err(NativeToolFailure::new(
            ProductToolFailureCategory::Execution,
            "CANDIDATE_HARD_GATE_FAILED",
            "Candidate failed a readback or render hard gate.",
        ));
    }
    let geometry = state.geometry.as_ref().ok_or_else(|| {
        NativeToolFailure::conflict(
            "ACTION_LOOP_BUILD_REQUIRED",
            "Restricted geometry build is required before preview.",
        )
    })?;
    let preview_id = format!(
        "preview_{}",
        &sha256_hex(format!("{turn_id}:{}", geometry.topology_hash).as_bytes())[..24]
    );
    let decision = match (
        state.generation_attempt.as_ref(),
        state.generation_gate_report.as_ref(),
    ) {
        (Some(attempt), Some(report)) => Some(
            SingleResultDecision::passed(
                format!(
                    "decision_{}",
                    &sha256_hex(format!("{}:{preview_id}", attempt.attempt_id).as_bytes())[..24]
                ),
                attempt,
                report,
                "One complete concept is ready for preview and explicit confirmation.".into(),
                GenerationPreview {
                    preview_id: preview_id.clone(),
                    artifact_sha256: geometry.glb_sha256.clone(),
                    artifact_profile_id: geometry.readback.artifact_profile_id.clone(),
                    expires_at: None,
                },
            )
            .map_err(|error| {
                NativeToolFailure::new(
                    ProductToolFailureCategory::Execution,
                    error.code(),
                    error.to_string(),
                )
            })?,
        ),
        (None, None) => None,
        _ => {
            return Err(NativeToolFailure::conflict(
                "SINGLE_GENERATION_EVIDENCE_INCOMPLETE",
                "Native V003 preview evidence was incomplete.",
            ))
        }
    };
    let payload = json!({
        "preview_id": preview_id,
        "topology_hash": geometry.topology_hash,
        "view_sha256": geometry.view_sha256,
        "requires_user_confirmation": true,
        "permanent_side_effects": 0,
        "single_result_decision": decision
    });
    state.preview = Some(payload.clone());
    Ok(payload)
}

fn value_object(value: Value) -> Result<BTreeMap<String, Value>, NativeToolFailure> {
    let object = value.as_object().ok_or_else(|| {
        NativeToolFailure::schema(
            "NATIVE_PRODUCT_TOOL_OUTPUT_NOT_OBJECT",
            "Native Product Tool output must be an object.",
        )
    })?;
    reject_forbidden_json(&value)?;
    Ok(object
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect())
}

fn string_argument<'a>(
    arguments: &'a BTreeMap<String, Value>,
    key: &str,
) -> Result<&'a str, NativeToolFailure> {
    arguments.get(key).and_then(Value::as_str).ok_or_else(|| {
        NativeToolFailure::schema(
            "NATIVE_PRODUCT_TOOL_ARGUMENT_INVALID",
            format!("Required argument {key} is missing or invalid."),
        )
    })
}

fn supported_domain(value: &str) -> bool {
    matches!(
        value,
        "pack_future_weapon_prop"
            | "pack_vehicle_concept"
            | "pack_aircraft_concept"
            | "pack_robotic_arm_concept"
    )
}

fn domain_suffix(value: &str) -> &'static str {
    match value {
        "pack_future_weapon_prop" => "prop",
        "pack_vehicle_concept" => "vehicle",
        "pack_aircraft_concept" => "aircraft",
        "pack_robotic_arm_concept" => "arm",
        _ => "unsupported",
    }
}

fn reject_forbidden_json(value: &Value) -> Result<(), NativeToolFailure> {
    fn visit(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), NativeToolFailure> {
        *nodes = nodes.saturating_add(1);
        if depth > 32 || *nodes > 20_000 {
            return Err(NativeToolFailure::schema(
                "RESTRICTED_JSON_BUDGET_EXCEEDED",
                "Restricted JSON exceeded its structural budget.",
            ));
        }
        match value {
            Value::Object(object) => {
                if object.len() > 512 {
                    return Err(NativeToolFailure::schema(
                        "RESTRICTED_JSON_BUDGET_EXCEEDED",
                        "Restricted JSON object exceeded its field budget.",
                    ));
                }
                for (key, child) in object {
                    let folded = key.to_ascii_lowercase();
                    if key.len() > 128 || FORBIDDEN_KEYS.contains(&folded.as_str()) {
                        return Err(NativeToolFailure::new(
                            ProductToolFailureCategory::Permission,
                            "RESTRICTED_JSON_FORBIDDEN_FIELD",
                            "Restricted JSON contained a forbidden authority field.",
                        ));
                    }
                    visit(child, depth + 1, nodes)?;
                }
            }
            Value::Array(items) => {
                if items.len() > 2048 {
                    return Err(NativeToolFailure::schema(
                        "RESTRICTED_JSON_BUDGET_EXCEEDED",
                        "Restricted JSON array exceeded its item budget.",
                    ));
                }
                for child in items {
                    visit(child, depth + 1, nodes)?;
                }
            }
            Value::String(text) => {
                if text.chars().count() > 8_000 || is_machine_location(text) {
                    return Err(NativeToolFailure::new(
                        ProductToolFailureCategory::Permission,
                        "RESTRICTED_JSON_MACHINE_LOCATION_FORBIDDEN",
                        "Restricted JSON cannot contain a URL or machine location.",
                    ));
                }
                let folded = text.to_ascii_lowercase();
                if folded.starts_with("bearer ") || folded.starts_with("sk-") {
                    return Err(NativeToolFailure::new(
                        ProductToolFailureCategory::Permission,
                        "RESTRICTED_JSON_SECRET_FORBIDDEN",
                        "Restricted JSON cannot contain a credential-like value.",
                    ));
                }
            }
            Value::Number(number) => {
                if number.as_f64().is_some_and(|value| !value.is_finite()) {
                    return Err(NativeToolFailure::schema(
                        "RESTRICTED_JSON_NON_FINITE",
                        "Restricted JSON cannot contain a non-finite number.",
                    ));
                }
            }
            Value::Null | Value::Bool(_) => {}
        }
        Ok(())
    }

    visit(value, 0, &mut 0)
}

fn reject_high_level_geometry_context(value: &Value) -> Result<(), NativeToolFailure> {
    const HIGH_LEVEL_KEYS: [&str; 16] = [
        "brief",
        "directions",
        "domain_pack_id",
        "mechanical_concept_plan",
        "model",
        "plan",
        "provider",
        "provider_id",
        "provider_name",
        "recipe",
        "style_recipe",
        "style_token",
        "thread",
        "turn",
        "snapshot",
        "project",
    ];
    fn visit(value: &Value) -> Result<(), NativeToolFailure> {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    if HIGH_LEVEL_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
                        return Err(NativeToolFailure::new(
                            ProductToolFailureCategory::Permission,
                            "RESTRICTED_GEOMETRY_HIGH_LEVEL_CONTEXT_FORBIDDEN",
                            "Restricted geometry input contained high-level Agent or product state.",
                        ));
                    }
                    visit(child)?;
                }
            }
            Value::Array(items) => {
                for child in items {
                    visit(child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    visit(value)
}

fn is_machine_location(value: &str) -> bool {
    let folded = value.to_ascii_lowercase();
    value.starts_with('/')
        || value.starts_with("~/")
        || folded.starts_with("file://")
        || folded.starts_with("http://")
        || folded.starts_with("https://")
        || (value.len() >= 3
            && value.as_bytes()[0].is_ascii_alphabetic()
            && value.as_bytes()[1] == b':'
            && matches!(value.as_bytes()[2], b'/' | b'\\'))
}

fn validate_profile_sketch_value(value: &Value) -> Result<(), NativeToolFailure> {
    reject_forbidden_json(value)?;
    let object = require_object(value, "PROFILE_SKETCH_SCHEMA_INVALID")?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "sketch_id",
            "version",
            "plane",
            "closed",
            "winding",
            "start",
            "segments",
            "holes",
            "normalized_bounds",
            "symmetry",
            "continuity_hint",
            "resample_count",
            "provenance",
        ],
        &[],
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    require_const_string(
        object,
        "schema_version",
        "ProfileSketch@1",
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    require_prefixed_id(
        object,
        "sketch_id",
        "sketch_",
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    require_const_u64(object, "version", 1, "PROFILE_SKETCH_SCHEMA_INVALID")?;
    require_enum_string(
        object,
        "plane",
        &["front", "side", "top", "cross_section"],
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    let closed = object
        .get("closed")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            schema_failure(
                "PROFILE_SKETCH_SCHEMA_INVALID",
                "ProfileSketch.closed must be boolean.",
            )
        })?;
    let winding = require_enum_string(
        object,
        "winding",
        &["open", "clockwise", "counter_clockwise"],
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    if closed == (winding == "open") {
        return Err(schema_failure(
            "PROFILE_SKETCH_WINDING_INVALID",
            "Closed ProfileSketch requires a directed winding; open profiles require open winding.",
        ));
    }
    validate_number_tuple(
        object.get("start"),
        2,
        -1.0,
        1.0,
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    let segments = require_array(object, "segments", 1, 64, "PROFILE_SKETCH_SCHEMA_INVALID")?;
    for segment in segments {
        validate_profile_segment(segment, "PROFILE_SKETCH_SCHEMA_INVALID")?;
    }
    let holes = require_array(object, "holes", 0, 8, "PROFILE_SKETCH_SCHEMA_INVALID")?;
    let mut hole_ids = BTreeSet::new();
    for hole in holes {
        let hole = require_object(hole, "PROFILE_SKETCH_SCHEMA_INVALID")?;
        require_exact_keys(
            hole,
            &["hole_id", "winding", "start", "segments"],
            &[],
            "PROFILE_SKETCH_SCHEMA_INVALID",
        )?;
        let hole_id =
            require_prefixed_id(hole, "hole_id", "hole_", "PROFILE_SKETCH_SCHEMA_INVALID")?;
        if !hole_ids.insert(hole_id) {
            return Err(schema_failure(
                "PROFILE_SKETCH_DUPLICATE_HOLE",
                "ProfileSketch hole IDs must be unique.",
            ));
        }
        require_enum_string(
            hole,
            "winding",
            &["clockwise", "counter_clockwise"],
            "PROFILE_SKETCH_SCHEMA_INVALID",
        )?;
        validate_number_tuple(
            hole.get("start"),
            2,
            -1.0,
            1.0,
            "PROFILE_SKETCH_SCHEMA_INVALID",
        )?;
        for segment in require_array(hole, "segments", 3, 64, "PROFILE_SKETCH_SCHEMA_INVALID")? {
            validate_profile_segment(segment, "PROFILE_SKETCH_SCHEMA_INVALID")?;
        }
    }
    let bounds = require_object(
        object.get("normalized_bounds").unwrap_or(&Value::Null),
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    require_exact_keys(
        bounds,
        &["min", "max"],
        &[],
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    let minimum = number_tuple(
        bounds.get("min"),
        2,
        -1.0,
        1.0,
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    let maximum = number_tuple(
        bounds.get("max"),
        2,
        -1.0,
        1.0,
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    if minimum.iter().zip(&maximum).any(|(min, max)| min >= max) {
        return Err(schema_failure(
            "PROFILE_SKETCH_BOUNDS_INVALID",
            "ProfileSketch normalized bounds must have positive extent.",
        ));
    }
    require_enum_string(
        object,
        "symmetry",
        &["none", "horizontal", "vertical", "radial"],
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    require_enum_string(
        object,
        "continuity_hint",
        &["linear", "tangent", "smooth"],
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    require_u64_range(
        object,
        "resample_count",
        8,
        256,
        "PROFILE_SKETCH_SCHEMA_INVALID",
    )?;
    validate_provenance(object.get("provenance"), "PROFILE_SKETCH_SCHEMA_INVALID")?;
    Ok(())
}

fn validate_profile_segment(value: &Value, code: &str) -> Result<(), NativeToolFailure> {
    let segment = require_object(value, code)?;
    let kind = segment
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| schema_failure(code, "Profile segment kind must be a string."))?;
    match kind {
        "line" => require_exact_keys(segment, &["kind", "to"], &[], code)?,
        "quadratic" => require_exact_keys(segment, &["kind", "control", "to"], &[], code)?,
        "cubic" => require_exact_keys(
            segment,
            &["kind", "control_1", "control_2", "to"],
            &[],
            code,
        )?,
        _ => return Err(schema_failure(code, "Profile segment kind is unsupported.")),
    }
    validate_number_tuple(segment.get("to"), 2, -1.0, 1.0, code)?;
    for key in ["control", "control_1", "control_2"] {
        if segment.contains_key(key) {
            validate_number_tuple(segment.get(key), 2, -1.0, 1.0, code)?;
        }
    }
    Ok(())
}

fn validate_section_set_value(value: &Value) -> Result<(), NativeToolFailure> {
    reject_forbidden_json(value)?;
    let object = require_object(value, "PROFILE_SECTION_SET_SCHEMA_INVALID")?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "section_set_id",
            "version",
            "main_axis",
            "profiles",
            "sections",
            "resample_policy",
            "symmetry",
            "provenance",
        ],
        &[],
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    require_const_string(
        object,
        "schema_version",
        "ProfileSectionSet@1",
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    require_prefixed_id(
        object,
        "section_set_id",
        "sectionset_",
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    require_const_u64(object, "version", 1, "PROFILE_SECTION_SET_SCHEMA_INVALID")?;
    require_enum_string(
        object,
        "main_axis",
        &["x", "y", "z"],
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    let profiles = require_array(
        object,
        "profiles",
        1,
        12,
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    let mut profile_ids = BTreeSet::new();
    for profile in profiles {
        validate_profile_sketch_value(profile)?;
        let id = profile
            .get("sketch_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !profile_ids.insert(id.to_string()) {
            return Err(schema_failure(
                "PROFILE_SECTION_SET_DUPLICATE_PROFILE",
                "ProfileSectionSet profile IDs must be unique.",
            ));
        }
    }
    let sections = require_array(
        object,
        "sections",
        2,
        12,
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    let mut section_ids = BTreeSet::new();
    let mut previous_position = f64::NEG_INFINITY;
    for section in sections {
        let section = require_object(section, "PROFILE_SECTION_SET_SCHEMA_INVALID")?;
        require_exact_keys(
            section,
            &[
                "section_id",
                "position",
                "profile_sketch_id",
                "scale",
                "twist_degrees",
                "cap_policy",
            ],
            &[],
            "PROFILE_SECTION_SET_SCHEMA_INVALID",
        )?;
        let section_id = require_prefixed_id(
            section,
            "section_id",
            "section_",
            "PROFILE_SECTION_SET_SCHEMA_INVALID",
        )?;
        if !section_ids.insert(section_id) {
            return Err(schema_failure(
                "PROFILE_SECTION_SET_DUPLICATE_SECTION",
                "ProfileSectionSet section IDs must be unique.",
            ));
        }
        let position = require_number_range(
            section,
            "position",
            -1.0,
            1.0,
            "PROFILE_SECTION_SET_SCHEMA_INVALID",
        )?;
        if position <= previous_position {
            return Err(schema_failure(
                "PROFILE_SECTION_SET_ORDER_INVALID",
                "ProfileSectionSet section positions must be strictly ordered.",
            ));
        }
        previous_position = position;
        let profile_id = require_prefixed_id(
            section,
            "profile_sketch_id",
            "sketch_",
            "PROFILE_SECTION_SET_SCHEMA_INVALID",
        )?;
        if !profile_ids.contains(&profile_id) {
            return Err(schema_failure(
                "PROFILE_SECTION_SET_PROFILE_MISSING",
                "ProfileSectionSet section references an unknown profile.",
            ));
        }
        require_number_range(
            section,
            "scale",
            0.25,
            4.0,
            "PROFILE_SECTION_SET_SCHEMA_INVALID",
        )?;
        require_number_range(
            section,
            "twist_degrees",
            -45.0,
            45.0,
            "PROFILE_SECTION_SET_SCHEMA_INVALID",
        )?;
        require_enum_string(
            section,
            "cap_policy",
            &["none", "start", "end"],
            "PROFILE_SECTION_SET_SCHEMA_INVALID",
        )?;
    }
    let policy = require_object(
        object.get("resample_policy").unwrap_or(&Value::Null),
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    require_exact_keys(
        policy,
        &["mode", "count"],
        &[],
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    require_const_string(
        policy,
        "mode",
        "uniform_count",
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    require_u64_range(
        policy,
        "count",
        8,
        256,
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    require_enum_string(
        object,
        "symmetry",
        &["none", "horizontal", "vertical", "radial"],
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    validate_provenance(
        object.get("provenance"),
        "PROFILE_SECTION_SET_SCHEMA_INVALID",
    )?;
    Ok(())
}

fn validate_shape_program_value(value: &Value) -> Result<(), NativeToolFailure> {
    reject_forbidden_json(value)?;
    let object = require_object(value, "SHAPE_PROGRAM_SCHEMA_INVALID")?;
    require_exact_keys(
        object,
        &[
            "schema_version",
            "program_id",
            "units",
            "seed",
            "triangle_budget",
            "parameters",
            "operations",
            "outputs",
            "non_functional_only",
        ],
        &["profile_inputs"],
        "SHAPE_PROGRAM_SCHEMA_INVALID",
    )?;
    require_const_string(
        object,
        "schema_version",
        "ShapeProgram@1",
        "SHAPE_PROGRAM_SCHEMA_INVALID",
    )?;
    require_prefixed_id(
        object,
        "program_id",
        "shape_",
        "SHAPE_PROGRAM_SCHEMA_INVALID",
    )?;
    require_const_string(
        object,
        "units",
        "millimeter",
        "SHAPE_PROGRAM_SCHEMA_INVALID",
    )?;
    require_u64_range(
        object,
        "seed",
        0,
        2_147_483_647,
        "SHAPE_PROGRAM_SCHEMA_INVALID",
    )?;
    require_u64_range(
        object,
        "triangle_budget",
        100,
        100_000,
        "SHAPE_PROGRAM_SCHEMA_INVALID",
    )?;
    if object.get("non_functional_only") != Some(&Value::Bool(true)) {
        return Err(schema_failure(
            "SHAPE_PROGRAM_FUNCTIONAL_FORBIDDEN",
            "ShapeProgram must remain a non-functional concept asset.",
        ));
    }
    let parameters = require_array(object, "parameters", 0, 64, "SHAPE_PROGRAM_SCHEMA_INVALID")?;
    let mut parameter_ids = BTreeSet::new();
    for parameter in parameters {
        let parameter = require_object(parameter, "SHAPE_PROGRAM_SCHEMA_INVALID")?;
        require_exact_keys(
            parameter,
            &["parameter_id", "default", "min", "max"],
            &[],
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        let parameter_id = require_prefixed_id(
            parameter,
            "parameter_id",
            "param_",
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        if !parameter_ids.insert(parameter_id) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_DUPLICATE_PARAMETER",
                "ShapeProgram parameter IDs must be unique.",
            ));
        }
        let default = require_finite_number(parameter, "default", "SHAPE_PROGRAM_SCHEMA_INVALID")?;
        let minimum = require_finite_number(parameter, "min", "SHAPE_PROGRAM_SCHEMA_INVALID")?;
        let maximum = require_finite_number(parameter, "max", "SHAPE_PROGRAM_SCHEMA_INVALID")?;
        if minimum > maximum || !(minimum..=maximum).contains(&default) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_PARAMETER_RANGE",
                "ShapeProgram parameter default must be inside its range.",
            ));
        }
    }

    let mut profile_input_ids = BTreeSet::new();
    if let Some(profile_inputs) = object.get("profile_inputs") {
        let profile_inputs = profile_inputs
            .as_array()
            .filter(|items| items.len() <= 16)
            .ok_or_else(|| {
                schema_failure(
                    "SHAPE_PROGRAM_SCHEMA_INVALID",
                    "ShapeProgram profile_inputs must be a bounded array.",
                )
            })?;
        for input in profile_inputs {
            let input = require_object(input, "SHAPE_PROGRAM_SCHEMA_INVALID")?;
            require_exact_keys(
                input,
                &[
                    "input_id",
                    "input_kind",
                    "contract_version",
                    "input_sha256",
                    "canonical_payload",
                ],
                &[],
                "SHAPE_PROGRAM_SCHEMA_INVALID",
            )?;
            let input_id = require_prefixed_id(
                input,
                "input_id",
                "profileinput_",
                "SHAPE_PROGRAM_SCHEMA_INVALID",
            )?;
            if !profile_input_ids.insert(input_id) {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_DUPLICATE_PROFILE_INPUT",
                    "ShapeProgram profile input IDs must be unique.",
                ));
            }
            let kind = require_enum_string(
                input,
                "input_kind",
                &["profile_sketch", "profile_section_set"],
                "SHAPE_PROGRAM_SCHEMA_INVALID",
            )?;
            let version = require_enum_string(
                input,
                "contract_version",
                &["ProfileSketch@1", "ProfileSectionSet@1"],
                "SHAPE_PROGRAM_SCHEMA_INVALID",
            )?;
            if (kind == "profile_sketch") != (version == "ProfileSketch@1") {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_PROFILE_INPUT_KIND_MISMATCH",
                    "ShapeProgram profile input kind and contract version disagree.",
                ));
            }
            let payload = input.get("canonical_payload").ok_or_else(|| {
                schema_failure(
                    "SHAPE_PROGRAM_SCHEMA_INVALID",
                    "ShapeProgram profile payload is missing.",
                )
            })?;
            if kind == "profile_sketch" {
                validate_profile_sketch_value(payload)?;
            } else {
                validate_section_set_value(payload)?;
            }
            let expected = sha256_hex(canonical_json(payload).as_bytes());
            if input.get("input_sha256").and_then(Value::as_str) != Some(&expected) {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_PROFILE_INPUT_HASH_MISMATCH",
                    "ShapeProgram profile input hash does not match its canonical payload.",
                ));
            }
        }
    }

    let operations = require_array(object, "operations", 1, 256, "SHAPE_PROGRAM_SCHEMA_INVALID")?;
    let mut seen_operations = BTreeSet::new();
    let mut operation_kinds = BTreeMap::new();
    for operation in operations {
        let operation = require_object(operation, "SHAPE_PROGRAM_SCHEMA_INVALID")?;
        require_exact_keys(
            operation,
            &["operation_id", "op", "inputs", "args"],
            &[],
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        let operation_id = require_prefixed_id(
            operation,
            "operation_id",
            "op_",
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        if seen_operations.contains(&operation_id) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_DUPLICATE_OPERATION",
                "ShapeProgram operation IDs must be unique.",
            ));
        }
        let op = operation.get("op").and_then(Value::as_str).ok_or_else(|| {
            schema_failure(
                "SHAPE_PROGRAM_SCHEMA_INVALID",
                "ShapeProgram operation name is missing.",
            )
        })?;
        if !G819_OPERATIONS.contains(&op) {
            return Err(NativeToolFailure::unsupported(
                "UNSUPPORTED_RUNTIME_OPERATION",
                "ShapeProgram operation is not declared by ShapeProgramRuntimeManifest@1.",
            ));
        }
        let inputs = require_array(operation, "inputs", 0, 8, "SHAPE_PROGRAM_SCHEMA_INVALID")?;
        let input_ids = inputs
            .iter()
            .map(|value| {
                value.as_str().map(str::to_string).ok_or_else(|| {
                    schema_failure(
                        "SHAPE_PROGRAM_SCHEMA_INVALID",
                        "ShapeProgram operation input must be an operation ID.",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if input_ids
            .iter()
            .any(|input| !seen_operations.contains(input))
        {
            return Err(schema_failure(
                "SHAPE_PROGRAM_FORWARD_OR_MISSING_REFERENCE",
                "ShapeProgram operation references a missing or future operation.",
            ));
        }
        let args = require_object(
            operation.get("args").unwrap_or(&Value::Null),
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        validate_shape_operation_args(
            op,
            args,
            &input_ids,
            &operation_kinds,
            &profile_input_ids,
            &parameter_ids,
        )?;
        seen_operations.insert(operation_id.clone());
        operation_kinds.insert(operation_id, op.to_string());
    }

    // A reviewed assembly delta may append a small visual Recipe to an
    // already production-sized arm. Keep the contract bounded, but do not
    // make the original 48-output showcase envelope an accidental ceiling for
    // continued design and assembly.
    let outputs = require_array(object, "outputs", 1, 96, "SHAPE_PROGRAM_SCHEMA_INVALID")?;
    let mut output_ids = BTreeSet::new();
    for output in outputs {
        let output = require_object(output, "SHAPE_PROGRAM_SCHEMA_INVALID")?;
        require_exact_keys(
            output,
            &["output_id", "operation_id", "kind", "part_role"],
            &[],
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        let output_id = require_prefixed_id(
            output,
            "output_id",
            "output_",
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        if !output_ids.insert(output_id) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_DUPLICATE_OUTPUT",
                "ShapeProgram output IDs must be unique.",
            ));
        }
        let operation_id = require_prefixed_id(
            output,
            "operation_id",
            "op_",
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        if !seen_operations.contains(&operation_id) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_OUTPUT_REFERENCE_MISSING",
                "ShapeProgram output references an unknown operation.",
            ));
        }
        require_enum_string(
            output,
            "kind",
            &["mesh", "assembly_graph"],
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        require_role_id(output, "part_role", "SHAPE_PROGRAM_SCHEMA_INVALID")?;
    }
    Ok(())
}

fn validate_shape_operation_args(
    op: &str,
    args: &Map<String, Value>,
    inputs: &[String],
    operation_kinds: &BTreeMap<String, String>,
    profile_inputs: &BTreeSet<String>,
    parameter_ids: &BTreeSet<String>,
) -> Result<(), NativeToolFailure> {
    const ALLOWED_ARGS: [&str; 26] = [
        "size",
        "radius",
        "height",
        "angle",
        "spacing",
        "position",
        "rotation",
        "axis",
        "count",
        "segments",
        "points",
        "profile_input_id",
        "profile_scale",
        "cap_start",
        "cap_end",
        "radial_segments",
        "section_set_input_id",
        "cross_section_scale",
        "axis_length",
        "continuity",
        "path_points",
        "path_closed",
        "path_twist_degrees",
        "parameter_id",
        "part_role",
        "zone_id",
    ];
    const EXTRA_ALLOWED_ARGS: [&str; 3] = ["connector_kind", "joint_kind", "material_id"];
    for key in args.keys() {
        if !ALLOWED_ARGS.contains(&key.as_str()) && !EXTRA_ALLOWED_ARGS.contains(&key.as_str()) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_SCHEMA_INVALID",
                "ShapeProgram operation contains an unknown argument.",
            ));
        }
    }
    if let Some(parameter_id) = args.get("parameter_id").and_then(Value::as_str) {
        if !parameter_ids.contains(parameter_id) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_UNKNOWN_PARAMETER",
                "ShapeProgram operation references an unknown parameter.",
            ));
        }
    }
    for key in ["position", "rotation", "axis"] {
        if args.contains_key(key) {
            validate_number_tuple(
                args.get(key),
                3,
                -100_000.0,
                100_000.0,
                "SHAPE_PROGRAM_SCHEMA_INVALID",
            )?;
        }
    }
    if args.contains_key("size") {
        let size = number_tuple(
            args.get("size"),
            3,
            f64::MIN,
            f64::MAX,
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        if size.iter().any(|value| *value <= 0.0 || *value > 100_000.0) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_SCHEMA_INVALID",
                "ShapeProgram size must be positive and bounded.",
            ));
        }
    }
    for key in ["radius", "height", "spacing", "axis_length"] {
        if let Some(value) = args.get(key) {
            let value = value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    schema_failure(
                        "SHAPE_PROGRAM_SCHEMA_INVALID",
                        "ShapeProgram scalar must be finite.",
                    )
                })?;
            if value <= 0.0 || value > 100_000.0 {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_SCHEMA_INVALID",
                    "ShapeProgram scalar must be positive and bounded.",
                ));
            }
        }
    }
    if let Some(value) = args.get("angle") {
        let value = value.as_f64().unwrap_or(f64::NAN);
        if !value.is_finite() || value <= 0.0 || value > std::f64::consts::TAU {
            return Err(schema_failure(
                "SHAPE_PROGRAM_SCHEMA_INVALID",
                "ShapeProgram angle is outside bounds.",
            ));
        }
    }
    for (key, minimum, maximum) in [
        ("count", 1_u64, 64_u64),
        ("segments", 1_u64, 3_u64),
        ("radial_segments", 8_u64, 64_u64),
    ] {
        if let Some(value) = args.get(key) {
            if value
                .as_u64()
                .filter(|value| (minimum..=maximum).contains(value))
                .is_none()
            {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_SCHEMA_INVALID",
                    "ShapeProgram integer argument is outside bounds.",
                ));
            }
        }
    }
    for key in ["profile_scale", "cross_section_scale"] {
        if args.contains_key(key) {
            let values = number_tuple(
                args.get(key),
                2,
                f64::MIN_POSITIVE,
                100_000.0,
                "SHAPE_PROGRAM_SCHEMA_INVALID",
            )?;
            if values.iter().any(|value| *value <= 0.0) {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_SCHEMA_INVALID",
                    "ShapeProgram scale must be positive.",
                ));
            }
        }
    }
    for key in ["cap_start", "cap_end", "path_closed"] {
        if args.contains_key(key) && !args.get(key).is_some_and(Value::is_boolean) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_SCHEMA_INVALID",
                "ShapeProgram flag argument must be boolean.",
            ));
        }
    }
    if args
        .get("continuity")
        .is_some_and(|value| value.as_str() != Some("linear"))
    {
        return Err(schema_failure(
            "SHAPE_PROGRAM_SCHEMA_INVALID",
            "ShapeProgram continuity is outside its enum.",
        ));
    }
    if let Some(value) = args.get("path_twist_degrees") {
        let value = value.as_f64().unwrap_or(f64::NAN);
        if !value.is_finite() || !(-90.0..=90.0).contains(&value) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_SCHEMA_INVALID",
                "ShapeProgram path twist is outside bounds.",
            ));
        }
    }
    for (key, prefix) in [("zone_id", "zone_"), ("material_id", "mat_")] {
        if let Some(value) = args.get(key) {
            let value = value.as_str().unwrap_or_default();
            if !value.starts_with(prefix) || !is_lower_stable_id(value) {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_SCHEMA_INVALID",
                    "ShapeProgram visual identity argument is invalid.",
                ));
            }
        }
    }
    for key in ["part_role", "connector_kind"] {
        if let Some(value) = args.get(key) {
            if !value.as_str().is_some_and(is_role_id) {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_SCHEMA_INVALID",
                    "ShapeProgram semantic role argument is invalid.",
                ));
            }
        }
    }
    if args.get("joint_kind").is_some_and(|value| {
        !matches!(
            value.as_str(),
            Some("fixed" | "hinge" | "slider" | "ball" | "continuous")
        )
    }) {
        return Err(schema_failure(
            "SHAPE_PROGRAM_SCHEMA_INVALID",
            "ShapeProgram joint kind is outside its enum.",
        ));
    }
    if let Some(points) = args.get("points") {
        let points = points
            .as_array()
            .filter(|points| (3..=32).contains(&points.len()))
            .ok_or_else(|| {
                schema_failure(
                    "SHAPE_PROGRAM_SCHEMA_INVALID",
                    "ShapeProgram points must be a bounded polygon.",
                )
            })?;
        for point in points {
            validate_number_tuple(
                Some(point),
                2,
                -100_000.0,
                100_000.0,
                "SHAPE_PROGRAM_SCHEMA_INVALID",
            )?;
        }
    }
    if let Some(points) = args.get("path_points") {
        let points = points
            .as_array()
            .filter(|points| (2..=32).contains(&points.len()))
            .ok_or_else(|| {
                schema_failure(
                    "SHAPE_PROGRAM_SCHEMA_INVALID",
                    "ShapeProgram path must be bounded.",
                )
            })?;
        for point in points {
            validate_number_tuple(
                Some(point),
                3,
                -100_000.0,
                100_000.0,
                "SHAPE_PROGRAM_SCHEMA_INVALID",
            )?;
        }
    }
    if let Some(profile_id) = args.get("profile_input_id").and_then(Value::as_str) {
        if !profile_inputs.contains(profile_id) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_PROFILE_INPUT_MISSING",
                "ShapeProgram references an unknown profile input.",
            ));
        }
    }
    if let Some(section_id) = args.get("section_set_input_id").and_then(Value::as_str) {
        if !profile_inputs.contains(section_id) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_SECTION_SET_MISSING",
                "ShapeProgram references an unknown section set.",
            ));
        }
    }
    match op {
        "box" | "wedge" if !inputs.is_empty() || !args.contains_key("size") => {
            return Err(schema_failure(
                "SHAPE_PROGRAM_PRIMITIVE_INVALID",
                "Box/wedge require size and no inputs.",
            ));
        }
        "cylinder" | "capsule"
            if !inputs.is_empty()
                || !args.contains_key("radius")
                || !args.contains_key("height") =>
        {
            return Err(schema_failure(
                "SHAPE_PROGRAM_PRIMITIVE_INVALID",
                "Cylinder/capsule require radius, height, and no inputs.",
            ));
        }
        "profile" => {
            let sources = usize::from(args.contains_key("points"))
                + usize::from(args.contains_key("profile_input_id"));
            if !inputs.is_empty() || sources != 1 {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_PROFILE_SOURCE_INVALID",
                    "Profile requires exactly one bounded source and no operation input.",
                ));
            }
        }
        "extrude" | "revolve" => {
            if inputs.len() != 1
                || operation_kinds.get(&inputs[0]).map(String::as_str) != Some("profile")
            {
                return Err(schema_failure(
                    "SHAPE_PROGRAM_PROFILE_OPERATION_INPUT_INVALID",
                    "Extrude/revolve require one earlier profile operation.",
                ));
            }
        }
        "loft" if !inputs.is_empty() || !args.contains_key("section_set_input_id") => {
            return Err(schema_failure(
                "SHAPE_PROGRAM_LOFT_INPUT_INVALID",
                "Loft requires one expanded section set and no mesh input.",
            ));
        }
        "sweep"
            if !inputs.is_empty()
                || !args.contains_key("profile_input_id")
                || !args.contains_key("path_points") =>
        {
            return Err(schema_failure(
                "SHAPE_PROGRAM_SWEEP_INPUT_INVALID",
                "Sweep requires one reviewed profile input and a bounded path.",
            ));
        }
        "mirror" | "array" | "radial_array" | "bevel_approx" | "surface_panel"
            if inputs.len() != 1 =>
        {
            return Err(schema_failure(
                "SHAPE_PROGRAM_OPERATION_INPUT_INVALID",
                "Transform/detail operation requires one prior mesh input.",
            ));
        }
        "union" | "subtract" if inputs.len() < 2 => {
            return Err(schema_failure(
                "SHAPE_PROGRAM_BOOLEAN_INPUT_INVALID",
                "Boolean operation requires at least two prior mesh inputs.",
            ));
        }
        _ => {}
    }
    if matches!(op, "mirror" | "array" | "radial_array") {
        let axis = number_tuple(
            args.get("axis"),
            3,
            -100_000.0,
            100_000.0,
            "SHAPE_PROGRAM_SCHEMA_INVALID",
        )?;
        if axis.iter().all(|value| value.abs() <= f64::EPSILON) {
            return Err(schema_failure(
                "SHAPE_PROGRAM_AXIS_INVALID",
                "ShapeProgram transform axis must be non-zero.",
            ));
        }
    }
    if op == "array"
        && (args.get("count").and_then(Value::as_u64).unwrap_or(0) < 2
            || !args.contains_key("spacing"))
    {
        return Err(schema_failure(
            "SHAPE_PROGRAM_ARRAY_BUDGET",
            "ShapeProgram array requires count >= 2 and positive spacing.",
        ));
    }
    if op == "radial_array"
        && (args.get("count").and_then(Value::as_u64).unwrap_or(0) < 2
            || !args.contains_key("radius"))
    {
        return Err(schema_failure(
            "SHAPE_PROGRAM_RADIAL_ARRAY_BUDGET",
            "ShapeProgram radial array requires count >= 2 and positive radius.",
        ));
    }
    Ok(())
}

fn validate_provenance(value: Option<&Value>, code: &str) -> Result<(), NativeToolFailure> {
    let provenance = require_object(value.unwrap_or(&Value::Null), code)?;
    require_exact_keys(provenance, &["source"], &["source_ref"], code)?;
    require_enum_string(
        provenance,
        "source",
        &[
            "agent",
            "svg_editor",
            "component_recipe",
            "reference_rebuild",
        ],
        code,
    )?;
    if let Some(source_ref) = provenance.get("source_ref") {
        let source_ref = source_ref
            .as_str()
            .filter(|value| is_role_id(value) && value.len() <= 120)
            .ok_or_else(|| schema_failure(code, "Provenance source_ref is invalid."))?;
        let _ = source_ref;
    }
    Ok(())
}

fn require_object<'a>(
    value: &'a Value,
    code: &str,
) -> Result<&'a Map<String, Value>, NativeToolFailure> {
    value
        .as_object()
        .ok_or_else(|| schema_failure(code, "Expected a JSON object."))
}

fn require_exact_keys(
    object: &Map<String, Value>,
    required: &[&str],
    optional: &[&str],
    code: &str,
) -> Result<(), NativeToolFailure> {
    if required.iter().any(|key| !object.contains_key(*key))
        || object
            .keys()
            .any(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
    {
        return Err(schema_failure(
            code,
            "Object keys do not match the code-owned Schema.",
        ));
    }
    Ok(())
}

fn require_const_string(
    object: &Map<String, Value>,
    key: &str,
    expected: &str,
    code: &str,
) -> Result<(), NativeToolFailure> {
    if object.get(key).and_then(Value::as_str) != Some(expected) {
        return Err(schema_failure(
            code,
            format!("{key} does not match its Schema constant."),
        ));
    }
    Ok(())
}

fn require_const_u64(
    object: &Map<String, Value>,
    key: &str,
    expected: u64,
    code: &str,
) -> Result<(), NativeToolFailure> {
    if object.get(key).and_then(Value::as_u64) != Some(expected) {
        return Err(schema_failure(
            code,
            format!("{key} does not match its Schema constant."),
        ));
    }
    Ok(())
}

fn require_prefixed_id(
    object: &Map<String, Value>,
    key: &str,
    prefix: &str,
    code: &str,
) -> Result<String, NativeToolFailure> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| schema_failure(code, format!("{key} must be a string.")))?;
    if !value.starts_with(prefix) || !is_lower_stable_id(value) {
        return Err(schema_failure(
            code,
            format!("{key} is outside its stable ID pattern."),
        ));
    }
    Ok(value.to_string())
}

fn require_role_id(
    object: &Map<String, Value>,
    key: &str,
    code: &str,
) -> Result<String, NativeToolFailure> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_role_id(value))
        .ok_or_else(|| {
            schema_failure(code, format!("{key} is outside its stable role pattern."))
        })?;
    Ok(value.to_string())
}

fn require_enum_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    allowed: &[&str],
    code: &str,
) -> Result<&'a str, NativeToolFailure> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| schema_failure(code, format!("{key} must be a string.")))?;
    if !allowed.contains(&value) {
        return Err(schema_failure(code, format!("{key} is outside its enum.")));
    }
    Ok(value)
}

fn require_array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    minimum: usize,
    maximum: usize,
    code: &str,
) -> Result<&'a Vec<Value>, NativeToolFailure> {
    object
        .get(key)
        .and_then(Value::as_array)
        .filter(|items| (minimum..=maximum).contains(&items.len()))
        .ok_or_else(|| schema_failure(code, format!("{key} must be a bounded array.")))
}

fn require_u64_range(
    object: &Map<String, Value>,
    key: &str,
    minimum: u64,
    maximum: u64,
    code: &str,
) -> Result<u64, NativeToolFailure> {
    let value = object
        .get(key)
        .and_then(Value::as_u64)
        .filter(|value| (minimum..=maximum).contains(value))
        .ok_or_else(|| schema_failure(code, format!("{key} is outside integer bounds.")))?;
    Ok(value)
}

fn require_finite_number(
    object: &Map<String, Value>,
    key: &str,
    code: &str,
) -> Result<f64, NativeToolFailure> {
    object
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| schema_failure(code, format!("{key} must be a finite number.")))
}

fn require_number_range(
    object: &Map<String, Value>,
    key: &str,
    minimum: f64,
    maximum: f64,
    code: &str,
) -> Result<f64, NativeToolFailure> {
    let value = require_finite_number(object, key, code)?;
    if !(minimum..=maximum).contains(&value) {
        return Err(schema_failure(
            code,
            format!("{key} is outside numeric bounds."),
        ));
    }
    Ok(value)
}

fn validate_number_tuple(
    value: Option<&Value>,
    length: usize,
    minimum: f64,
    maximum: f64,
    code: &str,
) -> Result<(), NativeToolFailure> {
    number_tuple(value, length, minimum, maximum, code).map(|_| ())
}

fn number_tuple(
    value: Option<&Value>,
    length: usize,
    minimum: f64,
    maximum: f64,
    code: &str,
) -> Result<Vec<f64>, NativeToolFailure> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == length)
        .ok_or_else(|| schema_failure(code, "Numeric tuple has the wrong dimension."))?;
    values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite() && (minimum..=maximum).contains(value))
                .ok_or_else(|| schema_failure(code, "Numeric tuple contains an invalid value."))
        })
        .collect()
}

fn schema_failure(code: impl Into<String>, message: impl Into<String>) -> NativeToolFailure {
    NativeToolFailure::schema(code, message)
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_generation_source_binding(
    source: &GenerationSourceBinding,
) -> Result<(), ProductToolPortError> {
    if !is_stable_id(&source.provider_id) {
        return Err(preview_port_error(
            "NATIVE_GENERATION_SOURCE_BINDING_INVALID",
            "Generation source provider identity must be a bounded stable ID.",
        ));
    }
    match (&source.source_kind, source.provider_id.as_str()) {
        (GenerationSourceKind::OfflineDeterministic, "offline_deterministic")
        | (GenerationSourceKind::DeepseekNetworkAttempted, "deepseek") => Ok(()),
        _ => Err(preview_port_error(
            "NATIVE_GENERATION_SOURCE_BINDING_INVALID",
            "Generation source kind must match a code-owned Provider origin.",
        )),
    }
}

fn is_stable_id(value: &str) -> bool {
    (1..=160).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_lower_stable_id(value: &str) -> bool {
    (2..=160).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn is_role_id(value: &str) -> bool {
    (2..=64).contains(&value.len())
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::ProviderToolCall;

    fn surface_layer_program() -> SurfaceLayerProgram {
        serde_json::from_value(json!({
            "schema_version": "SurfaceLayerProgram@1",
            "program_id": "surface_layer_native_executor",
            "target_part_id": "part_native_shell",
            "target_zone_id": "zone_native_shell",
            "target_part_role": "link_armor",
            "material_zone_id": "zone_native_shell",
            "base_material": "mat_graphite",
            "vector_paths": [{
                "path_id": "path_native_spine",
                "closed": false,
                "commands": [
                    {"kind": "move", "points": [[0.1, 0.2]]},
                    {"kind": "line", "points": [[0.8, 0.7]]}
                ]
            }],
            "decal_layers": [],
            "normal_relief_layers": [{
                "layer_id": "relief_native_groove",
                "motif": "parallel_groove",
                "intensity": "subtle",
                "coverage": "center_band",
                "seed": 7
            }],
            "roughness_masks": [],
            "emissive_masks": [],
            "symmetry": {"mode": "none", "center_uv": [0.5, 0.5]},
            "uv_frame": {
                "frame_id": "uvframe_native_shell",
                "u_min": 0.0,
                "u_max": 1.0,
                "v_min": 0.0,
                "v_max": 1.0,
                "rotation_degrees": 0.0
            },
            "quality_profile": "interactive_preview",
            "execution": "lower_to_a005_and_retain",
            "skill_id": "skill_first_party_surface_adornment",
            "skill_version": 2,
            "skill_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "generator": "surface_layer_v1",
            "non_functional_only": true
        }))
        .unwrap()
    }

    #[derive(Clone, Default)]
    struct SuccessGeometryPort {
        captured: Arc<Mutex<Vec<RestrictedGeometryInput>>>,
        calls: Arc<AtomicUsize>,
    }

    impl RestrictedGeometryPort for SuccessGeometryPort {
        fn build_compile_render(
            &self,
            input: RestrictedGeometryInput,
            cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            self.captured.lock().unwrap().push(input.clone());
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if cancellation.is_cancelled() {
                    return Err(RestrictedGeometryError::cancelled());
                }
                Ok(success_geometry_output(&input))
            })
        }
    }

    #[derive(Clone)]
    struct FailingGeometryPort;

    impl RestrictedGeometryPort for FailingGeometryPort {
        fn build_compile_render(
            &self,
            _input: RestrictedGeometryInput,
            _cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            Box::pin(async {
                Err(RestrictedGeometryError::execution(
                    "GEOMETRY_EXECUTOR_CRASHED",
                    "Restricted geometry worker exited without a result.",
                ))
            })
        }
    }

    #[derive(Clone)]
    struct PendingGeometryPort {
        started: Arc<tokio::sync::Notify>,
    }

    impl RestrictedGeometryPort for PendingGeometryPort {
        fn build_compile_render(
            &self,
            _input: RestrictedGeometryInput,
            _cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            let started = self.started.clone();
            Box::pin(async move {
                started.notify_one();
                std::future::pending::<Result<RestrictedGeometryOutput, RestrictedGeometryError>>()
                    .await
            })
        }
    }

    fn success_geometry_output(input: &RestrictedGeometryInput) -> RestrictedGeometryOutput {
        let glb_bytes = b"glTF ForgeCAD bounded native fixture bytes".to_vec();
        let glb_sha256 = sha256_hex(&glb_bytes);
        let glb_byte_size = glb_bytes.len() as u64;
        let views = REQUIRED_VIEWS
            .into_iter()
            .map(|name| {
                (
                    name.to_string(),
                    format!("PNG ForgeCAD deterministic {name} fixture").into_bytes(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let view_sha256 = views
            .iter()
            .map(|(name, bytes)| (name.clone(), sha256_hex(bytes)))
            .collect();
        RestrictedGeometryOutput {
            schema_version: RESTRICTED_GEOMETRY_OUTPUT_SCHEMA_VERSION.into(),
            glb_sha256: glb_sha256.clone(),
            topology_hash: sha256_hex(canonical_json(&input.shape_program).as_bytes()),
            glb_bytes,
            readback: RestrictedGeometryReadback {
                runtime_manifest_version: RESTRICTED_GEOMETRY_RUNTIME_MANIFEST_VERSION.into(),
                artifact_profile_id: input.quality_profile.profile_id.clone(),
                shape_program_sha256: sha256_hex(canonical_json(&input.shape_program).as_bytes()),
                glb_sha256,
                glb_byte_size,
                triangle_count: 12,
                bounds_mm: [180.0, 56.0, 34.0],
                mesh_count: 1,
                primitive_count: 1,
                material_count: 1,
                closed_manifold: true,
                surface_provenance_present: true,
                compile_readback_sha256: sha256_hex(b"native_success_geometry_readback_v2"),
                material_zone_count: 1,
                visual_texture_set_count: 1,
                visual_texture_map_count: 5,
                visual_texture_provenance_verified: true,
            },
            views,
            view_sha256,
            renderer_id: "forgecad-agent-software-raster@1".into(),
        }
    }

    #[derive(Clone, Default)]
    struct SurfaceRepairGeometryPort;

    impl RestrictedGeometryPort for SurfaceRepairGeometryPort {
        fn build_compile_render(
            &self,
            input: RestrictedGeometryInput,
            _cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            Box::pin(async move {
                let mut output = success_geometry_output(&input);
                let operations = input
                    .shape_program
                    .get("operations")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_object)
                    .filter_map(|operation| {
                        Some((
                            operation.get("operation_id")?.as_str()?,
                            operation.get("args")?.as_object()?,
                        ))
                    })
                    .collect::<BTreeMap<_, _>>();
                output.readback.surface_provenance_present = input
                    .shape_program
                    .get("outputs")
                    .and_then(Value::as_array)
                    .is_some_and(|outputs| {
                        !outputs.is_empty()
                            && outputs.iter().all(|output| {
                                output
                                    .get("operation_id")
                                    .and_then(Value::as_str)
                                    .and_then(|operation_id| operations.get(operation_id))
                                    .is_some_and(|args| {
                                        args.get("zone_id").is_some_and(Value::is_string)
                                            && args.get("material_id").is_some_and(Value::is_string)
                                    })
                            })
                    });
                Ok(output)
            })
        }
    }

    #[derive(Clone, Default)]
    struct AlwaysMissingSurfaceGeometryPort;

    impl RestrictedGeometryPort for AlwaysMissingSurfaceGeometryPort {
        fn build_compile_render(
            &self,
            input: RestrictedGeometryInput,
            _cancellation: CancellationToken,
        ) -> RestrictedGeometryFuture {
            Box::pin(async move {
                let mut output = success_geometry_output(&input);
                output.readback.surface_provenance_present = false;
                Ok(output)
            })
        }
    }

    /// Test-only reviewed catalog that models a compiler-visible surface
    /// provenance omission while preserving Recipe ownership and every other
    /// V003 evidence source.  The bounded repair must restore this exact
    /// omission in place; authored geometry is intentionally not used to
    /// prove the production V003 repair path.
    #[derive(Clone, Default)]
    struct MissingSurfaceRecipeCatalog;

    impl ReviewedShapeProgramCatalog for MissingSurfaceRecipeCatalog {
        fn expand(
            &self,
            request: &ReviewedCatalogRequest,
        ) -> Result<ReviewedCatalogExpansion, ReviewedCatalogError> {
            let mut expansion = RecipeBackedReviewedShapeProgramCatalog.expand(request)?;
            if request.geometry_strategy == GeometryStrategy::ComponentRecipe {
                for operation in expansion
                    .geometry_input
                    .shape_program
                    .get_mut("operations")
                    .and_then(Value::as_array_mut)
                    .into_iter()
                    .flatten()
                {
                    if let Some(args) = operation.get_mut("args").and_then(Value::as_object_mut) {
                        args.remove("zone_id");
                        args.remove("material_id");
                    }
                }
                let shape_sha256 =
                    sha256_hex(canonical_json(&expansion.geometry_input.shape_program).as_bytes());
                expansion.expanded_shape_program_sha256 = Some(shape_sha256.clone());
                if let Some(binding) = expansion.semantic_proportion_binding.as_mut() {
                    binding.shape_program_sha256 = shape_sha256;
                }
            }
            expansion.validate()?;
            Ok(expansion)
        }
    }

    fn executor_with(
        geometry: Arc<dyn RestrictedGeometryPort>,
        config: NativeProductToolExecutorConfig,
    ) -> NativeProductToolExecutor {
        NativeProductToolExecutor::with_embedded_catalog(
            Arc::new(ProductToolRegistry::default()),
            geometry,
            config,
        )
        .unwrap()
    }

    fn executor_with_missing_surface_catalog(
        geometry: Arc<dyn RestrictedGeometryPort>,
        config: NativeProductToolExecutorConfig,
    ) -> NativeProductToolExecutor {
        NativeProductToolExecutor::new(
            Arc::new(ProductToolRegistry::default()),
            geometry,
            Arc::new(MissingSurfaceRecipeCatalog),
            config,
        )
        .unwrap()
    }

    #[derive(Clone)]
    struct SnapshotReaderFixture;

    impl ActiveDesignSnapshotReader for SnapshotReaderFixture {
        fn read_active_design_snapshot(
            &self,
            project_id: &str,
        ) -> Result<Option<Value>, ProductToolPortError> {
            Ok(Some(json!({
                "schema_version": "ActiveDesignSnapshot@1",
                "project_id": project_id,
                "active_design": {
                    "source": "agent_asset",
                    "project_id": project_id,
                    "asset_version_id": "assetver_current",
                    "assembly_graph_id": "assembly_current"
                },
                "revision": 7
            })))
        }
    }

    #[test]
    fn native_executor_reads_snapshot_only_from_attached_rust_port() {
        let executor = executor_with(
            Arc::new(SuccessGeometryPort::default()),
            NativeProductToolExecutorConfig::default(),
        );
        assert_eq!(
            executor.read_active_design_snapshot("project_arm").unwrap(),
            None
        );
        executor
            .attach_active_snapshot_reader(Arc::new(SnapshotReaderFixture))
            .unwrap();
        let snapshot = executor
            .read_active_design_snapshot("project_arm")
            .unwrap()
            .unwrap();
        assert_eq!(
            snapshot["active_design"]["asset_version_id"],
            "assetver_current"
        );
        assert!(executor
            .attach_active_snapshot_reader(Arc::new(SnapshotReaderFixture))
            .is_err());
    }

    fn request(
        registry: &ProductToolRegistry,
        name: &str,
        arguments: Value,
        call_id: &str,
        execution_id: &str,
        turn_id: &str,
        cancellation_id: &str,
        cancellation_token: &str,
    ) -> ProductToolExecutionRequest {
        registry
            .build_execution_request(
                turn_id,
                &ProviderToolCall {
                    call_id: call_id.into(),
                    name: name.into(),
                    arguments,
                },
                execution_id,
                cancellation_id,
                cancellation_token,
            )
            .unwrap_or_else(|error| panic!("{name} request invalid: {error:?}"))
    }

    fn standard_request(
        registry: &ProductToolRegistry,
        name: &str,
        arguments: Value,
        call_id: &str,
    ) -> ProductToolExecutionRequest {
        request(
            registry,
            name,
            arguments,
            call_id,
            "execution_native",
            "turn_native",
            "cancel_native",
            "cancel_token_native",
        )
    }

    fn concept_plan() -> Value {
        let direction = |id: &str, title: &str| {
            json!({
                "direction_id": id,
                "title": title,
                "summary": "Complete non-functional exterior concept.",
                "silhouette": "compact",
                "primary_part_roles": ["primary_form", "secondary_form"],
                "material_direction": "dark metal and bounded visual coating"
            })
        };
        json!({
            "schema_version": "MechanicalConceptPlan@1",
            "plan_id": "plan_native",
            "domain_pack_id": "pack_future_weapon_prop",
            "brief": "non-functional future game prop",
            "generation_stage": "blockout",
            "spec": {},
            "directions": [
                direction("direction_primary", "Primary")
            ],
            "provider_id": "rust_app_server",
            "shape_program_ready": false
        })
    }

    fn concept_plan_for(domain_pack_id: &str) -> Value {
        let (plan_id, brief, title, summary, silhouette, roles, material_direction) =
            match domain_pack_id {
                "pack_future_weapon_prop" => (
                    "plan_future_prop",
                    "Create a non-functional future game prop with a compact silhouette, layered visual trim, and game-ready PBR exterior.",
                    "Future game prop exterior",
                    "A non-functional display prop with a compact forward silhouette and layered exterior detail.",
                    "compact",
                    json!(["primary_form", "visual_detail"]),
                    "dark graphite shell, restrained signal accents, and production PBR texture zones",
                ),
                "pack_vehicle_concept" => (
                    "plan_vehicle_concept",
                    "Create a non-functional future electric coupe concept with a continuous aerodynamic body, lamp trim, and production PBR exterior.",
                    "Future electric coupe concept",
                    "A display-only electric coupe with a low flowing cabin, continuous body volume, and integrated light trim.",
                    "extended",
                    json!(["primary_form", "visual_detail"]),
                    "automotive paint, dark glass-like contrast, signal lighting trim, and production PBR texture zones",
                ),
                "pack_aircraft_concept" => (
                    "plan_aircraft_concept",
                    "Create a non-functional future personal aircraft concept with a streamlined fuselage, controlled trim, and production PBR exterior.",
                    "Future personal aircraft concept",
                    "A display-only personal aircraft with a streamlined fuselage, clean sectional transitions, and exterior trim.",
                    "extended",
                    json!(["primary_form", "visual_detail"]),
                    "brushed alloy body, technical composite contrast, restrained trim, and production PBR texture zones",
                ),
                "pack_robotic_arm_concept" => (
                    "plan_robotic_arm",
                    "Create a non-functional desktop robotic arm concept with articulated exterior links, visible detail trim, and production PBR exterior.",
                "Desktop robotic arm concept",
                "A display-only articulated robotic arm with a strong upper link, compact joint housing, and visual exterior detail.",
                "industrial",
                // C106 validates the actual editable component roles emitted
                // by its production Recipe, rather than the legacy mesh-only
                // `upper_link_form`/`visual_detail` vocabulary.
                json!(["link_armor", "surface_trim"]),
                "anodized metal link shell, dark joint contrast, accent trim, and production PBR texture zones",
            ),
                _ => panic!("unsupported fixture domain: {domain_pack_id}"),
            };
        let direction = |id: &str, direction_title: String| {
            json!({
                "direction_id": id,
                "title": direction_title,
                "summary": summary,
                "silhouette": silhouette,
                "primary_part_roles": roles.clone(),
                "material_direction": material_direction
            })
        };
        json!({
            "schema_version": "MechanicalConceptPlan@1",
            "plan_id": plan_id,
            "domain_pack_id": domain_pack_id,
            "brief": brief,
            "generation_stage": "blockout",
            "spec": {},
            "directions": [
                direction("direction_primary", title.to_string())
            ],
            "provider_id": "rust_app_server",
            "shape_program_ready": false
        })
    }

    fn profile_sketch() -> Value {
        json!({
            "schema_version": "ProfileSketch@1",
            "sketch_id": "sketch_native",
            "version": 1,
            "plane": "cross_section",
            "closed": true,
            "winding": "counter_clockwise",
            "start": [-1.0, -1.0],
            "segments": [
                {"kind": "line", "to": [1.0, -1.0]},
                {"kind": "line", "to": [1.0, 1.0]},
                {"kind": "line", "to": [-1.0, 1.0]},
                {"kind": "line", "to": [-1.0, -1.0]}
            ],
            "holes": [],
            "normalized_bounds": {"min": [-1.0, -1.0], "max": [1.0, 1.0]},
            "symmetry": "horizontal",
            "continuity_hint": "linear",
            "resample_count": 16,
            "provenance": {"source": "agent"}
        })
    }

    fn shape_program(op: &str) -> Value {
        json!({
            "schema_version": "ShapeProgram@1",
            "program_id": "shape_native",
            "units": "millimeter",
            "seed": 7,
            "triangle_budget": 7000,
            "parameters": [],
            "operations": [{
                "operation_id": "op_primary",
                "op": op,
                "inputs": [],
                "args": {
                    "size": [180.0, 56.0, 34.0],
                    "position": [0.0, 0.0, 0.0],
                    "rotation": [0.0, 0.0, 0.0],
                    "part_role": "primary_form",
                    "zone_id": "zone_primary",
                    "material_id": "mat_graphite"
                }
            }],
            "outputs": [{
                "output_id": "output_primary",
                "operation_id": "op_primary",
                "kind": "mesh",
                "part_role": "primary_form"
            }],
            "non_functional_only": true
        })
    }

    fn contains_json_key(value: &Value, expected: &str) -> bool {
        match value {
            Value::Object(object) => object
                .iter()
                .any(|(key, value)| key == expected || contains_json_key(value, expected)),
            Value::Array(items) => items.iter().any(|value| contains_json_key(value, expected)),
            _ => false,
        }
    }

    async fn run_tool(
        executor: &NativeProductToolExecutor,
        request: ProductToolExecutionRequest,
    ) -> ProductToolExecutionResult {
        executor
            .execute(request, CancellationToken::new())
            .await
            .unwrap()
    }

    async fn prepare_preview_for(
        executor: &NativeProductToolExecutor,
        suffix: &str,
    ) -> (ProductToolExecutionResult, String) {
        prepare_preview_for_plan(executor, suffix, concept_plan()).await
    }

    async fn prepare_preview_for_plan(
        executor: &NativeProductToolExecutor,
        suffix: &str,
        plan: Value,
    ) -> (ProductToolExecutionResult, String) {
        let (_, preview, preview_id) = prepare_v003_preview_for_plan(executor, suffix, plan).await;
        (preview, preview_id)
    }

    async fn prepare_v003_preview_for_plan(
        executor: &NativeProductToolExecutor,
        suffix: &str,
        plan: Value,
    ) -> (Value, ProductToolExecutionResult, String) {
        let registry = ProductToolRegistry::default();
        let execution_id = format!("execution_{suffix}");
        let turn_id = format!("turn_{suffix}");
        let cancellation_id = format!("cancel_{suffix}");
        let cancellation_token = format!("token_{suffix}");
        executor
            .bind_execution_project_native(&execution_id, &turn_id, Some("project_native_test"))
            .unwrap();
        let domain_pack_id = plan
            .get("domain_pack_id")
            .and_then(Value::as_str)
            .expect("fixture plan has a domain")
            .to_string();
        let calls = [
            ("plan_complete_concept", json!({"plan": plan})),
            (
                "select_style_recipe",
                json!({"domain_pack_id": domain_pack_id, "intent": "紧凑"}),
            ),
            (
                "build_candidate_geometry",
                json!({"direction_id": "direction_primary", "variant_id": null, "presentation_profile": "showcase"}),
            ),
            ("compile_readback_candidate", json!({})),
            ("render_candidate_views", json!({})),
            ("evaluate_candidate", json!({})),
            ("prepare_candidate_preview", json!({})),
        ];
        let mut last = None;
        let mut evaluation = None;
        for (index, (name, arguments)) in calls.into_iter().enumerate() {
            let result = run_tool(
                executor,
                request(
                    &registry,
                    name,
                    arguments,
                    &format!("call_{suffix}_{index}"),
                    &execution_id,
                    &turn_id,
                    &cancellation_id,
                    &cancellation_token,
                ),
            )
            .await;
            assert_eq!(
                result.status,
                ProductToolExecutionStatus::Completed,
                "{name} failed while preparing {suffix}: {:?}",
                result.error_code
            );
            if name == "evaluate_candidate" {
                evaluation = result.validated_output.as_ref().map(|output| {
                    serde_json::to_value(&output.value).expect("tool output serializes")
                });
            }
            last = Some(result);
        }
        let result = last.unwrap();
        let preview_id = result
            .validated_output
            .as_ref()
            .and_then(|output| output.value.get("preview_id"))
            .and_then(Value::as_str)
            .unwrap()
            .to_string();
        (
            evaluation.expect("formal V003 evaluation exists"),
            result,
            preview_id,
        )
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future)
    }

    #[test]
    fn native_success_chain_returns_one_preview_and_no_high_level_geometry_context() {
        block_on(async {
            let geometry = SuccessGeometryPort::default();
            let captured = geometry.captured.clone();
            let calls = geometry.calls.clone();
            let executor = executor_with(
                Arc::new(geometry),
                NativeProductToolExecutorConfig::default(),
            );
            executor
                .bind_execution_project_native(
                    "execution_native",
                    "turn_native",
                    Some("project_native_test"),
                )
                .unwrap();
            let registry = ProductToolRegistry::default();
            let chain = [
                standard_request(
                    &registry,
                    "infer_product_domain",
                    json!({"brief": "设计一个非功能性的未来武器游戏道具"}),
                    "call_1",
                ),
                standard_request(
                    &registry,
                    "select_style_recipe",
                    json!({"domain_pack_id": "pack_future_weapon_prop", "intent": "紧凑流线"}),
                    "call_2",
                ),
                standard_request(
                    &registry,
                    "plan_complete_concept",
                    json!({"plan": concept_plan()}),
                    "call_3",
                ),
                standard_request(
                    &registry,
                    "build_candidate_geometry",
                    json!({"direction_id": "direction_primary", "variant_id": null, "presentation_profile": "showcase"}),
                    "call_4",
                ),
                standard_request(&registry, "compile_readback_candidate", json!({}), "call_5"),
                standard_request(&registry, "render_candidate_views", json!({}), "call_6"),
                standard_request(&registry, "evaluate_candidate", json!({}), "call_7"),
                standard_request(&registry, "prepare_candidate_preview", json!({}), "call_8"),
            ];
            let mut results = Vec::new();
            for call in chain {
                results.push(run_tool(&executor, call).await);
            }
            assert!(
                results
                    .iter()
                    .all(|result| result.status == ProductToolExecutionStatus::Completed),
                "native recipe chain failures: {:?}",
                results
                    .iter()
                    .map(|result| (&result.status, &result.error_code))
                    .collect::<Vec<_>>()
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            let gate_report =
                &results[6].validated_output.as_ref().unwrap().value["generation_gate_report"];
            assert_eq!(
                gate_report["gate_profile_version"],
                json!("native_v003_gate_v2")
            );
            let gate_ids = gate_report["checks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|check| check["gate_id"].as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                gate_ids,
                BTreeSet::from([
                    "brief_coverage",
                    "closed_manifold",
                    "domain_role_coverage",
                    "editability_evidence",
                    "four_views_read_back",
                    "generation_source_marked",
                    "glb_hash_verified",
                    "has_meshes",
                    "has_triangles",
                    "material_texture_provenance",
                    "r006_same_source_views",
                    "semantic_proportion_bound",
                    "surface_provenance_present",
                ])
            );
            let input = captured.lock().unwrap().first().cloned().unwrap();
            input.validate().unwrap();
            let wire = serde_json::to_string(&input).unwrap();
            for forbidden in [
                "thread_id",
                "session_id",
                "provider_id",
                "database_path",
                "object_store_path",
                "http://",
                "https://",
                "directions",
                "style_token",
                "recipe_id",
            ] {
                assert!(
                    !wire.contains(forbidden),
                    "restricted input leaked {forbidden}"
                );
            }
            let preview = results.last().unwrap().validated_output.as_ref().unwrap();
            assert_eq!(preview.value["requires_user_confirmation"], json!(true));
            assert_eq!(preview.value["permanent_side_effects"], json!(0));
            let preview_id = preview.value["preview_id"].as_str().unwrap().to_string();
            let decision = preview.value["single_result_decision"].as_object().unwrap();
            assert_eq!(decision["schema_version"], json!("SingleResultDecision@1"));
            assert_eq!(decision["project_id"], json!("project_native_test"));
            assert_eq!(decision["state"], json!("ready_for_preview"));
            let artifact = executor.preview_artifact(&preview_id).unwrap().unwrap();
            assert_eq!(
                decision["preview"]["artifact_sha256"],
                json!(artifact.glb_sha256)
            );

            let second_preview = run_tool(
                &executor,
                standard_request(&registry, "prepare_candidate_preview", json!({}), "call_9"),
            )
            .await;
            assert_eq!(second_preview.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                second_preview.error_code.as_deref(),
                Some("NATIVE_PREVIEW_ALREADY_PREPARED")
            );
            assert!(executor.preview_artifact(&preview_id).unwrap().is_none());
        });
    }

    #[test]
    fn native_v003_project_binding_is_trusted_once_and_decision_is_bound_to_gate_glb() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let (result, preview_id) = prepare_preview_for(&executor, "v003_project_binding").await;
            let output = result.validated_output.unwrap().value;
            let decision = output["single_result_decision"].as_object().unwrap();
            assert_eq!(decision["project_id"], json!("project_native_test"));
            assert_eq!(
                decision["preview"]["artifact_sha256"],
                output["single_result_decision"]["preview"]["artifact_sha256"]
            );
            assert!(
                output["single_result_decision"]["preview"]["artifact_sha256"]
                    .as_str()
                    .unwrap()
                    .len()
                    == 64
            );
            let artifact_sha256 = decision["preview"]["artifact_sha256"].as_str().unwrap();
            let artifact = executor
                .formal_preview_artifact(
                    "project_native_test",
                    "turn_v003_project_binding",
                    &preview_id,
                    artifact_sha256,
                )
                .unwrap()
                .unwrap();
            let provenance = artifact.formal_provenance.as_ref().unwrap();
            assert_eq!(provenance.project_id, "project_native_test");
            assert_eq!(provenance.plan_id, "plan_native");
            assert_eq!(provenance.direction_id, "direction_primary");
            assert_eq!(
                provenance.decision.preview.as_ref().unwrap().preview_id,
                preview_id
            );
            assert_eq!(provenance.decision_sha256.len(), 64);
            let mismatch = executor
                .formal_preview_artifact(
                    "project_other",
                    "turn_v003_project_binding",
                    &preview_id,
                    artifact_sha256,
                )
                .unwrap_err();
            assert_eq!(mismatch.code, "NATIVE_FORMAL_PREVIEW_IDENTITY_MISMATCH");
            assert!(executor.preview_artifact(&preview_id).unwrap().is_some());
            assert!(executor
                .reject_formal_preview(
                    "project_native_test",
                    "turn_v003_project_binding",
                    &preview_id,
                    artifact_sha256,
                )
                .unwrap());
            assert!(!executor
                .reject_formal_preview(
                    "project_native_test",
                    "turn_v003_project_binding",
                    &preview_id,
                    artifact_sha256,
                )
                .unwrap());
            let error = executor
                .bind_execution_project_native(
                    "execution_v003_project_binding",
                    "turn_v003_project_binding",
                    Some("project_rebound"),
                )
                .unwrap_err();
            assert_eq!(error.code, "NATIVE_PROJECT_BINDING_CONFLICT");
        });
    }

    #[test]
    fn native_v003_generation_source_is_trusted_once_and_fails_closed_on_turn_or_late_drift() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            executor
                .bind_execution_project_native(
                    "execution_source",
                    "turn_source",
                    Some("project_source"),
                )
                .unwrap();
            executor
                .bind_execution_generation_source_native(
                    "execution_source",
                    "turn_source",
                    GenerationSourceBinding {
                        provider_id: "deepseek".into(),
                        source_kind: GenerationSourceKind::DeepseekNetworkAttempted,
                    },
                )
                .unwrap();
            let source_drift = executor
                .bind_execution_generation_source_native(
                    "execution_source",
                    "turn_source",
                    GenerationSourceBinding {
                        provider_id: "offline_deterministic".into(),
                        source_kind: GenerationSourceKind::OfflineDeterministic,
                    },
                )
                .unwrap_err();
            assert_eq!(
                source_drift.code,
                "NATIVE_GENERATION_SOURCE_BINDING_CONFLICT"
            );
            let turn_drift = executor
                .bind_execution_generation_source_native(
                    "execution_source",
                    "turn_other",
                    GenerationSourceBinding {
                        provider_id: "deepseek".into(),
                        source_kind: GenerationSourceKind::DeepseekNetworkAttempted,
                    },
                )
                .unwrap_err();
            assert_eq!(turn_drift.code, "NATIVE_GENERATION_SOURCE_BINDING_CONFLICT");

            let registry = ProductToolRegistry::default();
            let first = run_tool(
                &executor,
                request(
                    &registry,
                    "infer_product_domain",
                    json!({"brief": "non-functional game prop"}),
                    "call_source_first",
                    "execution_source",
                    "turn_source",
                    "cancel_source",
                    "token_source",
                ),
            )
            .await;
            assert_eq!(first.status, ProductToolExecutionStatus::Completed);
            let late_bind = executor
                .bind_execution_generation_source_native(
                    "execution_source",
                    "turn_source",
                    GenerationSourceBinding {
                        provider_id: "deepseek".into(),
                        source_kind: GenerationSourceKind::DeepseekNetworkAttempted,
                    },
                )
                .unwrap();
            // Exact source rebind after execution start is idempotent; a
            // different origin cannot replace it.
            let conflicting_late_bind = executor
                .bind_execution_generation_source_native(
                    "execution_source",
                    "turn_source",
                    GenerationSourceBinding {
                        provider_id: "offline_deterministic".into(),
                        source_kind: GenerationSourceKind::OfflineDeterministic,
                    },
                )
                .unwrap_err();
            assert_eq!(
                conflicting_late_bind.code,
                "NATIVE_GENERATION_SOURCE_BINDING_CONFLICT"
            );
            assert_eq!(late_bind, ());
        });
    }

    #[test]
    fn native_v003_domain_role_gate_rejects_a_persistent_child_without_exact_output_binding() {
        let graph = json!({
            "parts": [
                {"operation_id": "op_root", "output_id": "output_root", "role": "primary_form", "recipe_instance_id": "instance_root"},
                {"operation_id": "op_child", "output_id": "output_child", "role": "visual_detail", "recipe_instance_id": "instance_child"}
            ]
        });
        let instances = json!([
            {"instance_id": "instance_root", "domain_pack_id": "pack_future_weapon_prop"},
            {"instance_id": "instance_child", "domain_pack_id": "pack_future_weapon_prop"}
        ]);
        let root_only = json!({
            "outputs": [{"operation_id": "op_root", "output_id": "output_root", "part_role": "primary_form"}]
        });
        assert_eq!(
            domain_role_coverage_outcome(&graph, &instances, &root_only, "pack_future_weapon_prop",),
            VerificationOutcome::Fail
        );
        let complete = json!({
            "outputs": [
                {"operation_id": "op_root", "output_id": "output_root", "part_role": "primary_form"},
                {"operation_id": "op_child", "output_id": "output_child", "part_role": "visual_detail"}
            ]
        });
        assert_eq!(
            domain_role_coverage_outcome(&graph, &instances, &complete, "pack_future_weapon_prop",),
            VerificationOutcome::Pass
        );
        assert_eq!(
            domain_role_coverage_outcome(&graph, &instances, &complete, "pack_vehicle_concept"),
            VerificationOutcome::Fail
        );
    }

    #[test]
    fn native_v003_runs_one_code_owned_surface_repair_with_exact_parent_lineage() {
        block_on(async {
            let executor = executor_with_missing_surface_catalog(
                Arc::new(SurfaceRepairGeometryPort),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let execution_id = "execution_surface_repair";
            let turn_id = "turn_surface_repair";
            let cancellation_id = "cancel_surface_repair";
            let cancellation_token = "token_surface_repair";
            executor
                .bind_execution_project_native(
                    execution_id,
                    turn_id,
                    Some("project_surface_repair"),
                )
                .unwrap();
            let plan = concept_plan();
            let initial_calls = [
                ("plan_complete_concept", json!({"plan": plan})),
                (
                    "select_style_recipe",
                    json!({"domain_pack_id": "pack_future_weapon_prop", "intent": "紧凑"}),
                ),
                (
                    "build_candidate_geometry",
                    json!({"direction_id": "direction_primary", "variant_id": null, "presentation_profile": "showcase"}),
                ),
                ("evaluate_candidate", json!({})),
            ];
            let mut evaluation = None;
            for (index, (name, arguments)) in initial_calls.into_iter().enumerate() {
                let result = run_tool(
                    &executor,
                    request(
                        &registry,
                        name,
                        arguments,
                        &format!("call_surface_initial_{index}"),
                        execution_id,
                        turn_id,
                        cancellation_id,
                        cancellation_token,
                    ),
                )
                .await;
                assert_eq!(
                    result.status,
                    ProductToolExecutionStatus::Completed,
                    "{name}: {:?}",
                    result.error_code
                );
                if name == "evaluate_candidate" {
                    evaluation = result.validated_output.map(|output| output.value);
                }
            }
            let evaluation = evaluation.unwrap();
            assert_eq!(evaluation["hard_gate_passed"], json!(false));
            let report = &evaluation["generation_gate_report"];
            let parent_attempt_id = report["attempt_id"].as_str().unwrap();
            let parent_gate_report_id = report["gate_report_id"].as_str().unwrap();
            let repair_arguments = json!({
                "direction_id": "direction_primary",
                "variant_id": null,
                "presentation_profile": "showcase",
                "repair": {
                    "schema_version": "BoundedGeometryRepairPatch@1",
                    "parent_attempt_id": parent_attempt_id,
                    "parent_gate_report_id": parent_gate_report_id,
                    "gate_id": "surface_provenance_present",
                    "patch_id": "restore_output_material_zones"
                }
            });
            let mut wrong_parent_arguments = repair_arguments.clone();
            wrong_parent_arguments["repair"]["parent_attempt_id"] = json!("attempt_wrong_parent");
            let wrong_parent = run_tool(
                &executor,
                request(
                    &registry,
                    "build_candidate_geometry",
                    wrong_parent_arguments,
                    "call_surface_wrong_parent",
                    execution_id,
                    turn_id,
                    cancellation_id,
                    cancellation_token,
                ),
            )
            .await;
            assert_eq!(wrong_parent.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                wrong_parent.error_code.as_deref(),
                Some("REPAIR_PARENT_PROVENANCE_MISMATCH")
            );
            let repaired = run_tool(
                &executor,
                request(
                    &registry,
                    "build_candidate_geometry",
                    repair_arguments,
                    "call_surface_repair",
                    execution_id,
                    turn_id,
                    cancellation_id,
                    cancellation_token,
                ),
            )
            .await;
            assert_eq!(
                repaired.status,
                ProductToolExecutionStatus::Completed,
                "repair failed: {:?} {:?}",
                repaired.error_code,
                repaired.message
            );
            let repair = &repaired.validated_output.as_ref().unwrap().value["repair"];
            assert_eq!(repair["repair_number"], json!(2));
            assert_eq!(repair["parent_attempt_id"], json!(parent_attempt_id));
            assert_eq!(
                repair["parent_gate_report_id"],
                json!(parent_gate_report_id)
            );
            let patched_fields = repair["patched_fields"].as_array().unwrap();
            assert_eq!(patched_fields.len(), 4);
            assert!(patched_fields.iter().all(|field| {
                field.as_str().is_some_and(|field| {
                    field.starts_with("shape_program.operations[op_")
                        && (field.ends_with(".args.material_id")
                            || field.ends_with(".args.zone_id"))
                })
            }));
            let repaired_attempt_id = repair["repaired_attempt_id"].as_str().unwrap().to_string();
            assert_ne!(repaired_attempt_id, parent_attempt_id);

            let reevaluated = run_tool(
                &executor,
                request(
                    &registry,
                    "evaluate_candidate",
                    json!({}),
                    "call_surface_reevaluate",
                    execution_id,
                    turn_id,
                    cancellation_id,
                    cancellation_token,
                ),
            )
            .await;
            assert_eq!(reevaluated.status, ProductToolExecutionStatus::Completed);
            assert_eq!(
                reevaluated.validated_output.as_ref().unwrap().value["hard_gate_passed"],
                json!(true)
            );
            assert_eq!(
                reevaluated.validated_output.as_ref().unwrap().value["generation_gate_report"]
                    ["attempt_id"],
                json!(repaired_attempt_id)
            );
            let repaired_gate_report_id = reevaluated.validated_output.as_ref().unwrap().value
                ["generation_gate_report"]["gate_report_id"]
                .as_str()
                .unwrap()
                .to_string();
            let third = run_tool(
                &executor,
                request(
                    &registry,
                    "build_candidate_geometry",
                    json!({
                        "direction_id": "direction_primary",
                        "variant_id": null,
                        "presentation_profile": "showcase",
                        "repair": {
                            "schema_version": "BoundedGeometryRepairPatch@1",
                            "parent_attempt_id": repaired_attempt_id,
                            "parent_gate_report_id": repaired_gate_report_id,
                            "gate_id": "surface_provenance_present",
                            "patch_id": "restore_output_material_zones"
                        }
                    }),
                    "call_surface_third_repair",
                    execution_id,
                    turn_id,
                    cancellation_id,
                    cancellation_token,
                ),
            )
            .await;
            assert_eq!(third.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                third.error_code.as_deref(),
                Some("REPAIR_ATTEMPT_LIMIT_REACHED")
            );
            let preview = run_tool(
                &executor,
                request(
                    &registry,
                    "prepare_candidate_preview",
                    json!({}),
                    "call_surface_preview",
                    execution_id,
                    turn_id,
                    cancellation_id,
                    cancellation_token,
                ),
            )
            .await;
            assert_eq!(preview.status, ProductToolExecutionStatus::Completed);
            assert_eq!(
                preview.validated_output.as_ref().unwrap().value["single_result_decision"]
                    ["attempt_id"],
                json!(repaired_attempt_id)
            );
        });
    }

    #[test]
    fn native_v003_repair_rejects_plan_intent_drift_before_geometry_recompile() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SurfaceRepairGeometryPort),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let execution_id = "execution_repair_drift";
            let turn_id = "turn_repair_drift";
            let cancellation_id = "cancel_repair_drift";
            let cancellation_token = "token_repair_drift";
            executor
                .bind_execution_project_native(execution_id, turn_id, Some("project_repair_drift"))
                .unwrap();
            let mut authored = shape_program("box");
            authored["operations"][0]["args"]
                .as_object_mut()
                .unwrap()
                .remove("zone_id");
            authored["operations"][0]["args"]
                .as_object_mut()
                .unwrap()
                .remove("material_id");
            let mut plan = concept_plan();
            plan["shape_program_ready"] = json!(true);
            for (index, (name, arguments)) in [
                ("author_shape_program", json!({"shape_program": authored})),
                ("plan_complete_concept", json!({"plan": plan.clone()})),
                (
                    "build_candidate_geometry",
                    json!({"direction_id": "direction_primary", "variant_id": null, "presentation_profile": "quick_sketch"}),
                ),
                ("evaluate_candidate", json!({})),
            ]
            .into_iter()
            .enumerate()
            {
                let result = run_tool(
                    &executor,
                    request(
                        &registry,
                        name,
                        arguments,
                        &format!("call_repair_drift_initial_{index}"),
                        execution_id,
                        turn_id,
                        cancellation_id,
                        cancellation_token,
                    ),
                )
                .await;
                assert_eq!(result.status, ProductToolExecutionStatus::Completed);
            }
            let (parent_attempt_id, parent_gate_report_id) = {
                let inner = executor.lock_inner().unwrap();
                let state = &inner.runs.get(execution_id).unwrap().state;
                (
                    state
                        .generation_attempt
                        .as_ref()
                        .unwrap()
                        .attempt_id
                        .clone(),
                    state
                        .generation_gate_report
                        .as_ref()
                        .unwrap()
                        .gate_report_id
                        .clone(),
                )
            };
            plan["directions"][0]["summary"] =
                json!("A different full-model intent must start a new Turn.");
            let changed_plan = run_tool(
                &executor,
                request(
                    &registry,
                    "plan_complete_concept",
                    json!({"plan": plan}),
                    "call_repair_drift_plan",
                    execution_id,
                    turn_id,
                    cancellation_id,
                    cancellation_token,
                ),
            )
            .await;
            assert_eq!(changed_plan.status, ProductToolExecutionStatus::Completed);
            let repair = run_tool(
                &executor,
                request(
                    &registry,
                    "build_candidate_geometry",
                    json!({
                        "direction_id": "direction_primary",
                        "variant_id": null,
                        "presentation_profile": "quick_sketch",
                        "repair": {
                            "schema_version": "BoundedGeometryRepairPatch@1",
                            "parent_attempt_id": parent_attempt_id,
                            "parent_gate_report_id": parent_gate_report_id,
                            "gate_id": "surface_provenance_present",
                            "patch_id": "restore_output_material_zones"
                        }
                    }),
                    "call_repair_drift_repair",
                    execution_id,
                    turn_id,
                    cancellation_id,
                    cancellation_token,
                ),
            )
            .await;
            assert_eq!(repair.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                repair.error_code.as_deref(),
                Some("REPAIR_ATTEMPT_INTENT_DRIFT")
            );
        });
    }

    #[test]
    fn native_v003_counts_authorized_failed_repairs_toward_the_two_attempt_limit() {
        block_on(async {
            let executor = executor_with(
                Arc::new(AlwaysMissingSurfaceGeometryPort),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let execution_id = "execution_failed_repairs";
            let turn_id = "turn_failed_repairs";
            let cancellation_id = "cancel_failed_repairs";
            let cancellation_token = "token_failed_repairs";
            executor
                .bind_execution_project_native(
                    execution_id,
                    turn_id,
                    Some("project_failed_repairs"),
                )
                .unwrap();
            for (index, (name, arguments)) in [
                ("plan_complete_concept", json!({"plan": concept_plan()})),
                ("select_style_recipe", json!({"domain_pack_id": "pack_future_weapon_prop", "intent": "紧凑"})),
                (
                    "build_candidate_geometry",
                    json!({"direction_id": "direction_primary", "variant_id": null, "presentation_profile": "showcase"}),
                ),
                ("evaluate_candidate", json!({})),
            ]
            .into_iter()
            .enumerate()
            {
                let result = run_tool(
                    &executor,
                    request(
                        &registry,
                        name,
                        arguments,
                        &format!("call_failed_repairs_initial_{index}"),
                        execution_id,
                        turn_id,
                        cancellation_id,
                        cancellation_token,
                    ),
                )
                .await;
                assert_eq!(result.status, ProductToolExecutionStatus::Completed);
            }
            let (attempt_id, report_id) = {
                let inner = executor.lock_inner().unwrap();
                let state = &inner.runs.get(execution_id).unwrap().state;
                (
                    state
                        .generation_attempt
                        .as_ref()
                        .unwrap()
                        .attempt_id
                        .clone(),
                    state
                        .generation_gate_report
                        .as_ref()
                        .unwrap()
                        .gate_report_id
                        .clone(),
                )
            };
            let repair_arguments = json!({
                "direction_id": "direction_primary",
                "variant_id": null,
                "presentation_profile": "showcase",
                "repair": {
                    "schema_version": "BoundedGeometryRepairPatch@1",
                    "parent_attempt_id": attempt_id,
                    "parent_gate_report_id": report_id,
                    "gate_id": "surface_provenance_present",
                    "patch_id": "restore_output_material_zones"
                }
            });
            for attempt in 1..=3 {
                let result = run_tool(
                    &executor,
                    request(
                        &registry,
                        "build_candidate_geometry",
                        repair_arguments.clone(),
                        &format!("call_failed_repair_{attempt}"),
                        execution_id,
                        turn_id,
                        cancellation_id,
                        cancellation_token,
                    ),
                )
                .await;
                assert_eq!(result.status, ProductToolExecutionStatus::Failed);
                assert_eq!(
                    result.error_code.as_deref(),
                    Some(if attempt < 3 {
                        "REPAIR_PATCH_NO_APPLICABLE_FIELD"
                    } else {
                        "REPAIR_ATTEMPT_LIMIT_REACHED"
                    })
                );
            }
        });
    }

    #[test]
    fn native_v003_rejects_a_second_complete_build_in_one_turn() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            executor
                .bind_execution_project_native(
                    "execution_one_build",
                    "turn_one_build",
                    Some("project_one_build"),
                )
                .unwrap();
            let plan = run_tool(
                &executor,
                request(
                    &registry,
                    "plan_complete_concept",
                    json!({"plan": concept_plan()}),
                    "call_plan",
                    "execution_one_build",
                    "turn_one_build",
                    "cancel_one_build",
                    "token_one_build",
                ),
            )
            .await;
            assert_eq!(plan.status, ProductToolExecutionStatus::Completed);
            let build_arguments = json!({
                "direction_id": "direction_primary",
                "variant_id": null,
                "presentation_profile": "quick_sketch"
            });
            let first = run_tool(
                &executor,
                request(
                    &registry,
                    "build_candidate_geometry",
                    build_arguments.clone(),
                    "call_build_first",
                    "execution_one_build",
                    "turn_one_build",
                    "cancel_one_build",
                    "token_one_build",
                ),
            )
            .await;
            assert_eq!(first.status, ProductToolExecutionStatus::Completed);
            let second = run_tool(
                &executor,
                request(
                    &registry,
                    "build_candidate_geometry",
                    build_arguments,
                    "call_build_second",
                    "execution_one_build",
                    "turn_one_build",
                    "cancel_one_build",
                    "token_one_build",
                ),
            )
            .await;
            assert_eq!(second.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                second.error_code.as_deref(),
                Some("SINGLE_GENERATION_FULL_BUILD_ALREADY_USED")
            );
        });
    }

    #[test]
    fn component_recipe_preview_carrier_survives_serialization_and_never_crosses_python_boundary() {
        block_on(async {
            let geometry = SuccessGeometryPort::default();
            let captured = geometry.captured.clone();
            let calls = geometry.calls.clone();
            let executor = executor_with(
                Arc::new(geometry),
                NativeProductToolExecutorConfig::default(),
            );
            for (suffix, domain_pack_id, fixed_brief) in [
                ("recipe_prop", "pack_future_weapon_prop", "Create a non-functional future game prop with a compact silhouette, layered visual trim, and game-ready PBR exterior."),
                ("recipe_vehicle", "pack_vehicle_concept", "Create a non-functional future electric coupe concept with a continuous aerodynamic body, lamp trim, and production PBR exterior."),
                ("recipe_aircraft", "pack_aircraft_concept", "Create a non-functional future personal aircraft concept with a streamlined fuselage, controlled trim, and production PBR exterior."),
                ("recipe_arm", "pack_robotic_arm_concept", "Create a non-functional desktop robotic arm concept with articulated exterior links, visible detail trim, and production PBR exterior."),
            ] {
                let plan = concept_plan_for(domain_pack_id);
                assert_eq!(plan["brief"], json!(fixed_brief));
                assert!(plan["directions"].as_array().is_some_and(|directions| {
                    directions.iter().all(|direction| {
                        direction["summary"].as_str().is_some_and(|value| !value.is_empty())
                            && direction["silhouette"].as_str().is_some_and(|value| !value.is_empty())
                            && direction["material_direction"].as_str().is_some_and(|value| !value.is_empty())
                            && direction["primary_part_roles"].as_array().is_some_and(|roles| !roles.is_empty())
                    })
                }));
                let (evaluation, preview, preview_id) =
                    prepare_v003_preview_for_plan(&executor, suffix, plan).await;
                assert_eq!(evaluation["hard_gate_passed"], json!(true));
                let checks = evaluation["generation_gate_report"]["checks"].as_array().unwrap();
                assert_eq!(checks.len(), 13);
                assert!(checks.iter().all(|check| check["outcome"] == "pass"));
                assert_eq!(
                    checks
                        .iter()
                        .map(|check| check["gate_id"].as_str().unwrap())
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from([
                        "brief_coverage", "closed_manifold", "domain_role_coverage",
                        "editability_evidence", "four_views_read_back", "generation_source_marked",
                        "glb_hash_verified", "has_meshes", "has_triangles",
                        "material_texture_provenance", "r006_same_source_views",
                        "semantic_proportion_bound", "surface_provenance_present",
                    ])
                );
                assert_eq!(preview.validated_output.as_ref().unwrap().value["permanent_side_effects"], json!(0));
                assert_eq!(preview.permanent_side_effects, 0);
                assert_eq!(preview.validated_output.as_ref().unwrap().value["single_result_decision"]["state"], json!("ready_for_preview"));
                let artifact = executor.preview_artifact(&preview_id).unwrap().unwrap();
                artifact.validate().unwrap();
                assert_eq!(artifact.readback.artifact_profile_id, "production_concept");
                assert!(artifact.formal_provenance.is_some());
                let inner = executor.lock_inner().unwrap();
                let state = &inner.runs.get(&format!("execution_{suffix}")).unwrap().state;
                assert_eq!(state.repair_attempts.len(), 0);
                assert_eq!(state.repair_attempts_started, 0);
                drop(inner);
                let wire = serde_json::to_value(&artifact).unwrap();
                assert_eq!(
                    wire["recipe_assembly_graph"],
                    artifact.recipe_assembly_graph.clone().unwrap()
                );
                assert_eq!(
                    wire["recipe_component_instances"],
                    artifact.recipe_component_instances.clone().unwrap()
                );
                assert_eq!(
                    wire["recipe_candidate_sha256"],
                    Value::String(artifact.recipe_candidate_sha256.clone().unwrap())
                );
                assert_eq!(
                    wire["recipe_expanded_shape_program_sha256"],
                    Value::String(
                        artifact
                            .recipe_expanded_shape_program_sha256
                            .clone()
                            .unwrap(),
                    )
                );

                let mut disconnected = artifact.clone();
                disconnected.recipe_assembly_graph.as_mut().unwrap()["parts"][0]["output_id"] =
                    Value::String("output_disconnected".into());
                assert_eq!(
                    disconnected.validate().unwrap_err().code,
                    "NATIVE_PREVIEW_ARTIFACT_INVALID"
                );
                let mut stale_shape_identity = artifact.clone();
                stale_shape_identity.recipe_expanded_shape_program_sha256 = Some("0".repeat(64));
                assert_eq!(
                    stale_shape_identity.validate().unwrap_err().code,
                    "NATIVE_PREVIEW_ARTIFACT_INVALID"
                );
                let mut stale_budget = artifact.clone();
                stale_budget.shape_program["triangle_budget"] = json!(6_999);
                assert_eq!(
                    stale_budget.validate().unwrap_err().code,
                    "NATIVE_PREVIEW_ARTIFACT_INVALID"
                );
            }
            assert_eq!(
                calls.load(Ordering::SeqCst),
                4,
                "each domain performs exactly one complete synthesis"
            );
            for input in captured.lock().unwrap().iter() {
                let wire = serde_json::to_value(input).unwrap();
                reject_high_level_geometry_context(&wire).unwrap();
                for forbidden in [
                    "expanded_assembly_graph",
                    "component_recipe_instances",
                    "candidate_sha256",
                    "recipe_assembly_graph",
                ] {
                    assert!(
                        !contains_json_key(&wire, forbidden),
                        "restricted worker request leaked {forbidden}"
                    );
                }
            }
        });
    }

    #[test]
    fn v003_fixed_brief_and_domain_mismatch_fail_before_any_synthesis() {
        block_on(async {
            let geometry = SuccessGeometryPort::default();
            let calls = geometry.calls.clone();
            let executor = executor_with(
                Arc::new(geometry),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let execution_id = "execution_fixed_brief_negative";
            let turn_id = "turn_fixed_brief_negative";
            executor
                .bind_execution_project_native(execution_id, turn_id, Some("project_fixed_brief"))
                .unwrap();

            let mut missing_brief = concept_plan_for("pack_vehicle_concept");
            missing_brief.as_object_mut().unwrap().remove("brief");
            let missing = registry
                .build_execution_request(
                    turn_id,
                    &ProviderToolCall {
                        call_id: "call_missing_brief".into(),
                        name: "plan_complete_concept".into(),
                        arguments: json!({"plan": missing_brief}),
                    },
                    execution_id,
                    "cancel_fixed_brief",
                    "token_fixed_brief",
                )
                .unwrap_err();
            assert_eq!(missing.code, "PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID");

            let mut empty_field = concept_plan_for("pack_vehicle_concept");
            empty_field["directions"][0]["material_direction"] = json!("");
            let empty = registry
                .build_execution_request(
                    turn_id,
                    &ProviderToolCall {
                        call_id: "call_empty_required_field".into(),
                        name: "plan_complete_concept".into(),
                        arguments: json!({"plan": empty_field}),
                    },
                    execution_id,
                    "cancel_fixed_brief",
                    "token_fixed_brief",
                )
                .unwrap_err();
            assert_eq!(empty.code, "PRODUCT_TOOL_ARGUMENT_SCHEMA_INVALID");

            let planned = run_tool(
                &executor,
                request(
                    &registry,
                    "plan_complete_concept",
                    json!({"plan": concept_plan_for("pack_vehicle_concept")}),
                    "call_vehicle_plan",
                    execution_id,
                    turn_id,
                    "cancel_fixed_brief",
                    "token_fixed_brief",
                ),
            )
            .await;
            assert_eq!(planned.status, ProductToolExecutionStatus::Completed);
            let mismatched_domain = run_tool(
                &executor,
                request(
                    &registry,
                    "select_style_recipe",
                    json!({"domain_pack_id": "pack_robotic_arm_concept", "intent": "紧凑"}),
                    "call_cross_domain_style",
                    execution_id,
                    turn_id,
                    "cancel_fixed_brief",
                    "token_fixed_brief",
                ),
            )
            .await;
            assert_eq!(mismatched_domain.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                mismatched_domain.error_code.as_deref(),
                Some("ACTION_LOOP_DOMAIN_CONFLICT")
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        });
    }

    #[test]
    fn authored_strategy_requires_explicit_plan_override_and_drops_recipe_carrier() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let mut plan = concept_plan();
            plan["shape_program_ready"] = Value::Bool(true);
            let calls = [
                (
                    "author_shape_program",
                    json!({"shape_program": shape_program("box")}),
                ),
                ("plan_complete_concept", json!({"plan": plan})),
                (
                    "build_candidate_geometry",
                    json!({"direction_id": "direction_primary", "variant_id": null, "presentation_profile": "quick_sketch"}),
                ),
                ("compile_readback_candidate", json!({})),
                ("render_candidate_views", json!({})),
                ("evaluate_candidate", json!({})),
            ];
            let mut evaluation = None;
            for (index, (name, arguments)) in calls.into_iter().enumerate() {
                let result = run_tool(
                    &executor,
                    request(
                        &registry,
                        name,
                        arguments,
                        &format!("call_authored_{index}"),
                        "execution_authored",
                        "turn_authored",
                        "cancel_authored",
                        "token_authored",
                    ),
                )
                .await;
                assert_eq!(
                    result.status,
                    ProductToolExecutionStatus::Completed,
                    "{name}: {:?}",
                    result.error_code
                );
                if name == "evaluate_candidate" {
                    evaluation = result.validated_output.map(|output| output.value);
                }
            }
            let evaluation = evaluation.unwrap();
            assert_eq!(evaluation["hard_gate_passed"], json!(false));
            assert!(evaluation["generation_gate_report"].is_null());
        });
    }

    #[test]
    fn authored_draft_does_not_bypass_default_component_recipe_strategy() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let calls = [
                (
                    "author_shape_program",
                    json!({"shape_program": shape_program("box")}),
                ),
                ("plan_complete_concept", json!({"plan": concept_plan()})),
                (
                    "select_style_recipe",
                    json!({"domain_pack_id": "pack_future_weapon_prop", "intent": "紧凑"}),
                ),
                (
                    "build_candidate_geometry",
                    json!({"direction_id": "direction_primary", "variant_id": null, "presentation_profile": "showcase"}),
                ),
                ("compile_readback_candidate", json!({})),
                ("render_candidate_views", json!({})),
                ("evaluate_candidate", json!({})),
                ("prepare_candidate_preview", json!({})),
            ];
            let mut last = None;
            for (index, (name, arguments)) in calls.into_iter().enumerate() {
                let result = run_tool(
                    &executor,
                    request(
                        &registry,
                        name,
                        arguments,
                        &format!("call_default_recipe_{index}"),
                        "execution_default_recipe",
                        "turn_default_recipe",
                        "cancel_default_recipe",
                        "token_default_recipe",
                    ),
                )
                .await;
                if name == "prepare_candidate_preview" {
                    assert_eq!(result.status, ProductToolExecutionStatus::Failed);
                    assert_eq!(
                        result.error_code.as_deref(),
                        Some("CANDIDATE_HARD_GATE_FAILED")
                    );
                    break;
                }
                assert_eq!(
                    result.status,
                    ProductToolExecutionStatus::Completed,
                    "{name}: {:?}",
                    result.error_code
                );
                last = Some(result);
            }
            let evaluation = last.unwrap().validated_output.unwrap().value;
            assert_eq!(evaluation["hard_gate_passed"], json!(false));
            let inner = executor.lock_inner().unwrap();
            let state = &inner.runs.get("execution_default_recipe").unwrap().state;
            assert!(state.recipe_expansion.is_some());
            assert!(state
                .recipe_expansion
                .as_ref()
                .unwrap()
                .expanded_assembly_graph
                .is_some());
        });
    }

    #[test]
    fn sweep_requires_a_profile_input_and_rejects_section_set_only_contract() {
        let catalog = RecipeBackedReviewedShapeProgramCatalog;
        let mut input = catalog
            .expand(&ReviewedCatalogRequest {
                domain_pack_id: "pack_future_weapon_prop".into(),
                direction_id: "direction_primary".into(),
                variant_id: None,
                presentation_profile: "quick_sketch".into(),
                plan: concept_plan(),
                style_recipe: None,
                authored_profile_sketch: None,
                authored_shape_program: None,
                geometry_strategy: GeometryStrategy::ComponentRecipe,
            })
            .unwrap()
            .geometry_input;
        let sweep_args = input.shape_program["operations"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|operation| operation["op"] == "sweep")
            .unwrap()["args"]
            .as_object_mut()
            .unwrap();
        let profile_input_id = sweep_args.remove("profile_input_id").unwrap();
        sweep_args.insert("section_set_input_id".into(), profile_input_id);
        let error = input.validate().unwrap_err();
        assert_eq!(error.code, "RESTRICTED_GEOMETRY_INPUT_INVALID");
        assert!(error
            .message
            .contains("Sweep requires one reviewed profile input"));
    }

    #[test]
    fn recipe_candidate_hash_binds_the_exact_quality_derived_shape_program_for_every_domain() {
        let catalog = RecipeBackedReviewedShapeProgramCatalog;
        let candidate_schema: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/schemas/component-recipe-candidate.schema.json"
        )))
        .unwrap();
        let legal_quality_profiles = candidate_schema["properties"]["quality_profile"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(
            legal_quality_profiles,
            &vec![json!("interactive_preview"), json!("production_concept")],
            "ComponentRecipeCandidate@1 must accept exactly the code-owned preview and production artifact profiles"
        );
        for domain_pack_id in [
            "pack_future_weapon_prop",
            "pack_vehicle_concept",
            "pack_aircraft_concept",
            "pack_robotic_arm_concept",
        ] {
            let request = |presentation_profile: &str| ReviewedCatalogRequest {
                domain_pack_id: domain_pack_id.into(),
                direction_id: "direction_primary".into(),
                variant_id: None,
                presentation_profile: presentation_profile.into(),
                plan: concept_plan_for(domain_pack_id),
                style_recipe: None,
                authored_profile_sketch: None,
                authored_shape_program: None,
                geometry_strategy: GeometryStrategy::ComponentRecipe,
            };
            let quick = catalog.expand(&request("quick_sketch")).unwrap();
            let showcase = catalog.expand(&request("showcase")).unwrap();
            quick.validate().unwrap();
            showcase.validate().unwrap();
            assert!(legal_quality_profiles.contains(&Value::String(
                quick.geometry_input.quality_profile.profile_id.clone(),
            )));
            assert!(legal_quality_profiles.contains(&Value::String(
                showcase.geometry_input.quality_profile.profile_id.clone(),
            )));
            assert_eq!(
                quick.expanded_shape_program_sha256.as_deref(),
                Some(
                    sha256_hex(canonical_json(&quick.geometry_input.shape_program).as_bytes())
                        .as_str()
                )
            );
            assert_eq!(
                showcase.expanded_shape_program_sha256.as_deref(),
                Some(
                    sha256_hex(canonical_json(&showcase.geometry_input.shape_program).as_bytes())
                        .as_str()
                )
            );
            assert_ne!(
                quick.candidate_sha256, showcase.candidate_sha256,
                "{domain_pack_id}: quality-derived ShapeProgram must receive a new candidate hash"
            );
            let mut stale_shape = quick.clone();
            stale_shape.geometry_input.shape_program["triangle_budget"] = json!(6_999);
            assert_eq!(
                stale_shape.validate().unwrap_err().code,
                "REVIEWED_RECIPE_SHAPE_IDENTITY_MISMATCH"
            );
            let mut stale_hash = quick.clone();
            stale_hash.expanded_shape_program_sha256 = Some("f".repeat(64));
            assert_eq!(
                stale_hash.validate().unwrap_err().code,
                "REVIEWED_RECIPE_SHAPE_IDENTITY_MISMATCH"
            );
        }
    }

    #[test]
    fn c106_arm_style_routes_one_deterministic_root_without_changing_c105_catalogs() {
        let catalog = RecipeBackedReviewedShapeProgramCatalog;
        let arm_request = |style_recipe: Option<Value>| ReviewedCatalogRequest {
            domain_pack_id: "pack_robotic_arm_concept".into(),
            direction_id: "direction_primary".into(),
            variant_id: None,
            presentation_profile: "showcase".into(),
            plan: concept_plan_for("pack_robotic_arm_concept"),
            style_recipe,
            authored_profile_sketch: None,
            authored_shape_program: None,
            geometry_strategy: GeometryStrategy::ComponentRecipe,
        };
        let selected_style = |intent: &str| {
            let mut state = NativeToolState::default();
            let arguments = BTreeMap::from([
                (
                    "domain_pack_id".into(),
                    Value::String("pack_robotic_arm_concept".into()),
                ),
                ("intent".into(), Value::String(intent.into())),
            ]);
            select_style_recipe(&arguments, &mut state).unwrap()
        };
        for (intent, expected_root) in [
            ("紧凑", C106_ARM_DESKTOP_ROOT_RECIPE_ID),
            ("简洁", C106_ARM_DESKTOP_ROOT_RECIPE_ID),
            ("厚重", C106_ARM_GALLERY_ROOT_RECIPE_ID),
            ("流线", C106_ARM_SERVICE_ROOT_RECIPE_ID),
        ] {
            let request = arm_request(Some(selected_style(intent)));
            let first = catalog.expand(&request).unwrap();
            let second = catalog.expand(&request).unwrap();
            let first_instances = first.component_recipe_instances.as_ref().unwrap();
            assert_eq!(
                first_instances[0]["recipe"]["recipe_id"],
                json!(expected_root),
                "{intent} must select one code-owned C106 root before expansion"
            );
            assert_eq!(
                first.candidate_sha256, second.candidate_sha256,
                "{intent} must produce one deterministic candidate, not a ranked fan-out"
            );
            assert_eq!(
                first.expanded_shape_program_sha256, second.expanded_shape_program_sha256,
                "{intent} must expand the same selected root deterministically"
            );
            assert!(
                first.expanded_assembly_graph.as_ref().unwrap()["parts"]
                    .as_array()
                    .is_some_and(|parts| !parts.is_empty()),
                "the selected root must yield the one editable assembly carried into V003"
            );
        }

        // No Style Token means the catalog's code-owned compact default, not
        // an implicit root-order selection.
        let default_arm = catalog.expand(&arm_request(None)).unwrap();
        assert_eq!(
            default_arm.component_recipe_instances.as_ref().unwrap()[0]["recipe"]["recipe_id"],
            json!(C106_ARM_DESKTOP_ROOT_RECIPE_ID)
        );

        // Existing non-arm domains still select their root from the frozen
        // C105 registry. C106 therefore cannot become a hidden global
        // registry replacement.
        let c105 = RecipeRegistry::from_embedded().unwrap();
        let c105_root = c105
            .recipes()
            .find(|recipe| {
                recipe
                    .allowed_domains
                    .iter()
                    .any(|domain| domain == "pack_future_weapon_prop")
                    && recipe.component_role != "visual_detail"
                    && !recipe.child_slots.is_empty()
            })
            .unwrap();
        let prop = catalog
            .expand(&ReviewedCatalogRequest {
                domain_pack_id: "pack_future_weapon_prop".into(),
                direction_id: "direction_primary".into(),
                variant_id: None,
                presentation_profile: "showcase".into(),
                plan: concept_plan_for("pack_future_weapon_prop"),
                style_recipe: None,
                authored_profile_sketch: None,
                authored_shape_program: None,
                geometry_strategy: GeometryStrategy::ComponentRecipe,
            })
            .unwrap();
        assert_eq!(
            prop.component_recipe_instances.as_ref().unwrap()[0]["recipe"]["recipe_id"],
            json!(c105_root.recipe_id)
        );
        assert_eq!(
            prop.component_recipe_instances.as_ref().unwrap()[0]["registry_sha256"],
            json!(c105.registry_sha256())
        );
    }

    #[test]
    fn c110e_arm_intent_changes_reviewed_shape_and_assembly_together() {
        let catalog = RecipeBackedReviewedShapeProgramCatalog;
        let arm_intent = |link_language: &str| {
            json!({
                "schema_version": "ArmDesignIntent@1",
                "domain_pack_id": "pack_robotic_arm_concept",
                "architecture": "serial_chain",
                "joint_language": "exposed_ring",
                "link_language": link_language,
                "base_language": "hex_platform",
                "wrist_language": "fork_wrist",
                "end_effector_language": "adaptive_claw",
                "cable_language": "braided_external",
                "surface_language": ["panel_seams", "flowline"],
                "material_palette": "white_aluminum",
                "detail_density": "dense",
                "pose": "extended",
                "proportion_profile": "long_reach",
                "style_keywords": ["precision", "mechanical"],
                "source": "user_brief",
                "visual_only": true
            })
        };
        let request = |link_language: &str| {
            let mut plan = concept_plan_for("pack_robotic_arm_concept");
            plan["arm_design_intent"] = arm_intent(link_language);
            ReviewedCatalogRequest {
                domain_pack_id: "pack_robotic_arm_concept".into(),
                direction_id: "direction_primary".into(),
                variant_id: None,
                presentation_profile: "showcase".into(),
                plan,
                style_recipe: None,
                authored_profile_sketch: None,
                authored_shape_program: None,
                geometry_strategy: GeometryStrategy::ComponentRecipe,
            }
        };
        let closed = catalog.expand(&request("closed_shell")).unwrap();
        let rails = catalog.expand(&request("twin_rail")).unwrap();
        closed.validate().unwrap();
        rails.validate().unwrap();
        assert_ne!(
            closed.expanded_shape_program_sha256, rails.expanded_shape_program_sha256,
            "link language must change the exact compiled ShapeProgram"
        );
        assert_ne!(closed.candidate_sha256, rails.candidate_sha256);
        assert!(closed.arm_geometry_binding.is_some());
        assert!(rails.arm_geometry_binding.is_some());
        let closed_parts = closed.expanded_assembly_graph.as_ref().unwrap()["parts"]
            .as_array()
            .unwrap();
        let rails_parts = rails.expanded_assembly_graph.as_ref().unwrap()["parts"]
            .as_array()
            .unwrap();
        assert_ne!(closed_parts, rails_parts);
    }

    #[test]
    fn c110g_parallel_link_uses_distinct_reviewed_geometry_family() {
        let mut plan = concept_plan_for("pack_robotic_arm_concept");
        plan["arm_design_intent"] = json!({
            "schema_version": "ArmDesignIntent@1",
            "domain_pack_id": "pack_robotic_arm_concept",
            "architecture": "parallel_link",
            "joint_language": "armored_bearing",
            "link_language": "twin_rail",
            "base_language": "industrial_deck",
            "wrist_language": "fork_wrist",
            "end_effector_language": "parallel_gripper",
            "cable_language": "armored_harness",
            "surface_language": ["panel_seams", "flowline"],
            "material_palette": "graphite_blue",
            "detail_density": "dense",
            "pose": "grounded",
            "proportion_profile": "balanced",
            "style_keywords": ["parallel", "industrial"],
            "source": "user_brief",
            "visual_only": true
        });
        let expansion = RecipeBackedReviewedShapeProgramCatalog
            .expand(&ReviewedCatalogRequest {
                domain_pack_id: "pack_robotic_arm_concept".into(),
                direction_id: "direction_parallel_link".into(),
                variant_id: None,
                presentation_profile: "showcase".into(),
                plan,
                style_recipe: None,
                authored_profile_sketch: None,
                authored_shape_program: None,
                geometry_strategy: GeometryStrategy::ComponentRecipe,
            })
            .unwrap();
        expansion.validate().unwrap();
        assert_eq!(
            domain_role_coverage_outcome(
                expansion.expanded_assembly_graph.as_ref().unwrap(),
                expansion.component_recipe_instances.as_ref().unwrap(),
                &expansion.geometry_input.shape_program,
                "pack_robotic_arm_concept",
            ),
            VerificationOutcome::Pass,
            "C110G graph roles must bind to the independent ShapeProgram outputs"
        );
        assert_eq!(
            expansion.arm_geometry_binding.as_ref().unwrap().family_id,
            "robotic_arm.parallel_link.c110g_v1"
        );
        assert_ne!(expansion.expanded_shape_program_sha256, Some("".into()));

        let c110g_registry = RecipeRegistry::from_embedded_c110g_parallel_link().unwrap();
        let root = c110g_registry
            .recipe(C110G_PARALLEL_ROOT_RECIPE_ID)
            .unwrap();
        let c110g_candidate = RecipeExpander::expand(
            &c110g_registry,
            &RecipeInstantiationRequest {
                schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                context_mode: "initial_candidate".into(),
                request_id: "recipereq_c110g_output_contract".into(),
                project_id: None,
                base_asset_version_id: None,
                snapshot_revision: None,
                domain_pack_id: "pack_robotic_arm_concept".into(),
                recipe_registry_sha256: c110g_registry.registry_sha256().into(),
                recipe: ComponentRecipeRef {
                    schema_version: "ComponentRecipeRef@1".into(),
                    recipe_id: root.recipe_id.clone(),
                    version: root.version,
                    recipe_sha256: RecipeValidator::recipe_sha256(root).unwrap(),
                },
                target_part_id: None,
                slot_bindings: vec![],
                parameter_values: vec![],
                material_zone_overrides: vec![],
            },
            &RecipeExpansionPolicy::default(),
        )
        .unwrap();
        let instances = serde_json::to_value(&c110g_candidate.component_recipe_instances).unwrap();
        assert_eq!(
            recipe_preview_output_contract(&instances).unwrap(),
            RecipePreviewOutputContract::C110GParallelLinkComponents
        );
    }

    #[test]
    fn c110b_plan_lowers_arm_intent_and_rejects_unreviewed_architecture() {
        let arm_intent = |architecture: &str| {
            json!({
                "schema_version": "ArmDesignIntent@1",
                "domain_pack_id": "pack_robotic_arm_concept",
                "architecture": architecture,
                "joint_language": "armored_bearing",
                "link_language": "closed_shell",
                "base_language": "round_turntable",
                "wrist_language": "layered_wrist",
                "end_effector_language": "parallel_gripper",
                "cable_language": "armored_harness",
                "surface_language": ["panel_seams", "flowline"],
                "material_palette": "graphite_blue",
                "detail_density": "dense",
                "pose": "grounded",
                "proportion_profile": "long_reach",
                "style_keywords": ["precision"],
                "source": "user_brief",
                "visual_only": true
            })
        };
        let mut state = NativeToolState::default();
        let mut valid_plan = concept_plan_for("pack_robotic_arm_concept");
        valid_plan["arm_design_intent"] = arm_intent("serial_chain");
        let arguments = BTreeMap::from([("plan".into(), valid_plan)]);
        let accepted = plan_complete_concept(&arguments, &mut state).unwrap();
        assert_eq!(
            accepted["plan"]["arm_recipe_lowering"]["status"],
            json!("lowered")
        );
        assert_eq!(
            accepted["plan"]["arm_recipe_lowering"]["root_recipe_id"],
            json!("recipe_c106_arm_service_display")
        );
        assert_eq!(
            state.style_recipe.as_ref().unwrap()["style_token"]["token_id"],
            json!("style_aerodynamic_sleek"),
            "ArmDesignIntent proportion must bind a reviewed Style Token before geometry build"
        );

        let mut unsupported_plan = concept_plan_for("pack_robotic_arm_concept");
        unsupported_plan["arm_design_intent"] = arm_intent("scara");
        let error = plan_complete_concept(
            &BTreeMap::from([("plan".into(), unsupported_plan)]),
            &mut NativeToolState::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "ARM_INTENT_ARCHITECTURE_UNSUPPORTED");
    }

    #[test]
    fn c106_output_contract_requires_exact_registry_and_recipe_provenance() {
        let expand = |registry: &RecipeRegistry, recipe_id: &str| {
            let recipe = registry.recipe(recipe_id).unwrap();
            RecipeExpander::expand(
                registry,
                &RecipeInstantiationRequest {
                    schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                    context_mode: "initial_candidate".into(),
                    request_id: format!("recipereq_contract_{recipe_id}"),
                    project_id: None,
                    base_asset_version_id: None,
                    snapshot_revision: None,
                    domain_pack_id: "pack_robotic_arm_concept".into(),
                    recipe_registry_sha256: registry.registry_sha256().into(),
                    recipe: ComponentRecipeRef {
                        schema_version: "ComponentRecipeRef@1".into(),
                        recipe_id: recipe.recipe_id.clone(),
                        version: recipe.version,
                        recipe_sha256: RecipeValidator::recipe_sha256(recipe).unwrap(),
                    },
                    target_part_id: None,
                    slot_bindings: Vec::new(),
                    parameter_values: Vec::new(),
                    material_zone_overrides: Vec::new(),
                },
                &RecipeExpansionPolicy::default(),
            )
            .unwrap()
        };

        let c106_registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
        let c106_candidate = expand(&c106_registry, C106_ARM_DESKTOP_ROOT_RECIPE_ID);
        let c106_instances =
            serde_json::to_value(&c106_candidate.component_recipe_instances).unwrap();
        assert_eq!(
            recipe_preview_output_contract(&c106_instances).unwrap(),
            RecipePreviewOutputContract::C106ArmSemanticComponents
        );

        let c105_registry = RecipeRegistry::from_embedded().unwrap();
        let c105_root = c105_registry
            .recipes()
            .find(|recipe| {
                recipe
                    .allowed_domains
                    .iter()
                    .any(|domain| domain == "pack_robotic_arm_concept")
                    && !recipe.child_slots.is_empty()
            })
            .unwrap();
        let c105_candidate = expand(&c105_registry, &c105_root.recipe_id);
        let c105_instances =
            serde_json::to_value(&c105_candidate.component_recipe_instances).unwrap();
        assert_eq!(
            recipe_preview_output_contract(&c105_instances).unwrap(),
            RecipePreviewOutputContract::C105OneToOne,
            "the shared robotic-arm domain must not relax frozen C105 output cardinality"
        );

        let assert_c106_rejected = |mutated: Value| {
            assert_eq!(
                recipe_preview_output_contract(&mutated).unwrap_err().code,
                "NATIVE_PREVIEW_ARTIFACT_INVALID"
            );
        };

        let mut wrong_hash = c106_instances.clone();
        wrong_hash[0]["registry_sha256"] = json!("f".repeat(64));
        assert_c106_rejected(wrong_hash);

        let mut mixed = c106_instances.clone();
        mixed.as_array_mut().unwrap()[9] = c105_instances[1].clone();
        assert_c106_rejected(mixed);

        let mut unknown_recipe = c106_instances.clone();
        unknown_recipe[9]["recipe"]["recipe_id"] = json!("recipe_c106_arm_unknown");
        assert_c106_rejected(unknown_recipe);

        let mut wrong_root = c106_instances;
        let child_recipe = wrong_root
            .as_array()
            .unwrap()
            .iter()
            .find(|instance| instance["recipe"]["recipe_id"] == "recipe_c106_arm_link_armor")
            .unwrap()["recipe"]
            .clone();
        wrong_root[0]["recipe"] = child_recipe;
        assert_c106_rejected(wrong_root);
    }

    #[test]
    fn native_preview_accessor_clones_validated_bytes_and_consumes_only_for_owning_turn() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let (preview_result, preview_id) = prepare_preview_for(&executor, "accessor").await;
            assert_eq!(preview_result.permanent_side_effects, 0);

            let artifact = executor.preview_artifact(&preview_id).unwrap().unwrap();
            artifact.validate().unwrap();
            assert_eq!(artifact.preview_id, preview_id);
            assert_eq!(artifact.turn_id, "turn_accessor");
            assert_eq!(artifact.glb_sha256, sha256_hex(&artifact.glb_bytes));
            assert_eq!(
                artifact.readback.glb_byte_size,
                artifact.glb_bytes.len() as u64
            );
            assert_eq!(artifact.assembly.parts.len(), 2);
            assert_eq!(artifact.assembly.parts[0].part_role, "primary_form");
            assert_eq!(
                artifact.assembly.parts[0].material_zone_id.as_deref(),
                Some("zone_prop_shell")
            );
            assert_eq!(
                artifact.assembly.parts[0].material_id.as_deref(),
                Some("mat_graphite")
            );
            assert!(artifact.recipe_assembly_graph.is_some());
            assert!(artifact.recipe_component_instances.is_some());
            assert!(artifact.recipe_candidate_sha256.is_some());
            assert_eq!(
                artifact
                    .views
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                REQUIRED_VIEWS.into_iter().collect::<BTreeSet<_>>()
            );
            assert_eq!(
                executor.preview_artifact(&preview_id).unwrap().unwrap(),
                artifact
            );

            let wire = serde_json::to_string(&artifact)
                .unwrap()
                .to_ascii_lowercase();
            for forbidden in [
                "artifact_handle",
                "python_handle",
                "provider_key",
                "provider_url",
                "thread_id",
                "history",
                "database_path",
                "object_store_path",
                "file_path",
            ] {
                assert!(!wire.contains(forbidden), "preview leaked {forbidden}");
            }

            let mismatch = executor
                .consume_preview(&preview_id, "turn_other")
                .unwrap_err();
            assert_eq!(mismatch.code, "NATIVE_PREVIEW_TURN_MISMATCH");
            assert!(executor.preview_artifact(&preview_id).unwrap().is_some());

            let consumed = executor
                .consume_preview(&preview_id, "turn_accessor")
                .unwrap();
            assert_eq!(consumed, artifact);
            assert!(executor.preview_artifact(&preview_id).unwrap().is_none());
            assert_eq!(
                executor
                    .consume_preview(&preview_id, "turn_accessor")
                    .unwrap_err()
                    .code,
                "NATIVE_PREVIEW_ARTIFACT_NOT_FOUND"
            );
        });
    }

    #[test]
    fn native_preview_registry_enforces_absolute_ttl_and_lru_capacity() {
        block_on(async {
            let ttl_executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig {
                    preview_artifact_ttl: Duration::from_millis(5),
                    ..NativeProductToolExecutorConfig::default()
                },
            );
            let (_, expiring_id) = prepare_preview_for(&ttl_executor, "ttl").await;
            tokio::time::sleep(Duration::from_millis(10)).await;
            assert!(ttl_executor
                .preview_artifact(&expiring_id)
                .unwrap()
                .is_none());
            assert_eq!(ttl_executor.lock_inner().unwrap().preview_retained_bytes, 0);

            let lru_executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig {
                    max_preview_artifacts: 2,
                    ..NativeProductToolExecutorConfig::default()
                },
            );
            let (_, preview_a) = prepare_preview_for(&lru_executor, "lru_a").await;
            let (_, preview_b) = prepare_preview_for(&lru_executor, "lru_b").await;
            assert!(lru_executor.preview_artifact(&preview_a).unwrap().is_some());
            let (_, preview_c) = prepare_preview_for(&lru_executor, "lru_c").await;
            assert!(lru_executor.preview_artifact(&preview_a).unwrap().is_some());
            assert!(lru_executor.preview_artifact(&preview_b).unwrap().is_none());
            assert!(lru_executor.preview_artifact(&preview_c).unwrap().is_some());
            assert_eq!(
                lru_executor.lock_inner().unwrap().preview_artifacts.len(),
                2
            );
        });
    }

    #[test]
    fn native_preview_cancel_and_discard_remove_all_transient_bytes() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let (_, cancelled_id) = prepare_preview_for(&executor, "cancel_preview").await;
            assert!(executor.preview_artifact(&cancelled_id).unwrap().is_some());
            assert!(executor
                .cancel(
                    "cancel_cancel_preview".into(),
                    "token_cancel_preview".into()
                )
                .await
                .unwrap());
            assert!(executor.preview_artifact(&cancelled_id).unwrap().is_none());
            assert_eq!(executor.lock_inner().unwrap().preview_retained_bytes, 0);

            let fresh_executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let (_, discarded_id) = prepare_preview_for(&fresh_executor, "discard").await;
            assert!(fresh_executor.discard_preview(&discarded_id).unwrap());
            assert!(!fresh_executor.discard_preview(&discarded_id).unwrap());
            assert!(fresh_executor
                .preview_artifact(&discarded_id)
                .unwrap()
                .is_none());
            assert_eq!(
                fresh_executor.lock_inner().unwrap().preview_retained_bytes,
                0
            );
        });
    }

    #[test]
    fn native_preview_config_rejects_unbounded_or_zero_registry_limits() {
        for config in [
            NativeProductToolExecutorConfig {
                max_preview_artifacts: 0,
                ..NativeProductToolExecutorConfig::default()
            },
            NativeProductToolExecutorConfig {
                max_preview_retained_bytes: 0,
                ..NativeProductToolExecutorConfig::default()
            },
            NativeProductToolExecutorConfig {
                preview_artifact_ttl: Duration::ZERO,
                ..NativeProductToolExecutorConfig::default()
            },
        ] {
            let error = NativeProductToolExecutor::with_embedded_catalog(
                Arc::new(ProductToolRegistry::default()),
                Arc::new(SuccessGeometryPort::default()),
                config,
            )
            .err()
            .unwrap();
            assert_eq!(error.code, "NATIVE_PRODUCT_TOOL_CONFIG_INVALID");
        }
    }

    #[test]
    fn native_profile_and_shape_validators_enforce_schema_and_g819_manifest() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            for call in [
                standard_request(
                    &registry,
                    "author_profile_sketch",
                    json!({"profile_sketch": profile_sketch()}),
                    "call_profile",
                ),
                standard_request(
                    &registry,
                    "author_shape_program",
                    json!({"shape_program": shape_program("box")}),
                    "call_shape",
                ),
            ] {
                assert_eq!(
                    run_tool(&executor, call).await.status,
                    ProductToolExecutionStatus::Completed
                );
            }

            let unknown = run_tool(
                &executor,
                standard_request(
                    &registry,
                    "validate_shape_program",
                    json!({"shape_program": shape_program("arbitrary_script")}),
                    "call_unknown",
                ),
            )
            .await;
            assert_eq!(unknown.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                unknown.failure_category,
                Some(ProductToolFailureCategory::Unsupported)
            );
            assert_eq!(
                unknown.error_code.as_deref(),
                Some("UNSUPPORTED_RUNTIME_OPERATION")
            );
        });
    }

    #[test]
    fn native_executor_rejects_missing_state_and_preserves_tool_order() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let missing = run_tool(
                &executor,
                standard_request(
                    &registry,
                    "compile_readback_candidate",
                    json!({}),
                    "call_missing",
                ),
            )
            .await;
            assert_eq!(missing.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                missing.failure_category,
                Some(ProductToolFailureCategory::Conflict)
            );
            assert_eq!(
                missing.error_code.as_deref(),
                Some("ACTION_LOOP_SHAPE_PROGRAM_REQUIRED")
            );

            let build = run_tool(
            &executor,
            standard_request(
                &registry,
                "build_candidate_geometry",
                json!({"direction_id": "direction_primary", "presentation_profile": "quick_sketch"}),
                "call_build_without_plan",
            ),
        )
        .await;
            assert_eq!(build.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                build.error_code.as_deref(),
                Some("ACTION_LOOP_PLAN_REQUIRED")
            );
        });
    }

    #[test]
    fn native_execution_identity_cannot_cross_turn_or_cancellation_scope() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let first = request(
                &registry,
                "infer_product_domain",
                json!({"brief": "汽车"}),
                "call_first",
                "execution_first",
                "turn_first",
                "cancel_shared",
                "token_shared",
            );
            assert_eq!(
                run_tool(&executor, first).await.status,
                ProductToolExecutionStatus::Completed
            );
            let cross_execution = request(
                &registry,
                "infer_product_domain",
                json!({"brief": "飞机"}),
                "call_second",
                "execution_second",
                "turn_second",
                "cancel_shared",
                "token_shared",
            );
            let error = executor
                .execute(cross_execution, CancellationToken::new())
                .await
                .unwrap_err();
            assert_eq!(error.code, "NATIVE_PRODUCT_TOOL_CANCELLATION_ID_CONFLICT");

            let cross_turn = request(
                &registry,
                "infer_product_domain",
                json!({"brief": "汽车"}),
                "call_third",
                "execution_first",
                "turn_other",
                "cancel_shared",
                "token_shared",
            );
            let error = executor
                .execute(cross_turn, CancellationToken::new())
                .await
                .unwrap_err();
            assert_eq!(
                error.code,
                "NATIVE_PRODUCT_TOOL_EXECUTION_IDENTITY_CONFLICT"
            );
        });
    }

    #[test]
    fn native_idempotency_replays_exact_result_and_rejects_conflicts() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let first = standard_request(
                &registry,
                "infer_product_domain",
                json!({"brief": "汽车"}),
                "call_idempotent",
            );
            let first_result = run_tool(&executor, first.clone()).await;
            let replay = run_tool(&executor, first).await;
            assert_eq!(first_result, replay);

            let changed = standard_request(
                &registry,
                "infer_product_domain",
                json!({"brief": "飞机"}),
                "call_idempotent",
            );
            let error = executor
                .execute(changed, CancellationToken::new())
                .await
                .unwrap_err();
            assert_eq!(error.code, "NATIVE_PRODUCT_TOOL_CALL_ID_CONFLICT");

            let mut drifted = standard_request(
                &registry,
                "infer_product_domain",
                json!({"brief": "机械臂"}),
                "call_drifted",
            );
            drifted.idempotency_key = "a".repeat(64);
            let error = executor
                .execute(drifted, CancellationToken::new())
                .await
                .unwrap_err();
            assert_eq!(error.code, "NATIVE_PRODUCT_TOOL_IDEMPOTENCY_DRIFT");
        });
    }

    #[test]
    fn cancel_before_start_is_tombstoned_and_token_mismatch_fails_closed() {
        block_on(async {
            let executor = executor_with(
                Arc::new(SuccessGeometryPort::default()),
                NativeProductToolExecutorConfig::default(),
            );
            assert!(!executor
                .cancel("cancel_native".into(), "cancel_token_native".into())
                .await
                .unwrap());
            let registry = ProductToolRegistry::default();
            let cancelled = run_tool(
                &executor,
                standard_request(
                    &registry,
                    "infer_product_domain",
                    json!({"brief": "汽车"}),
                    "call_cancelled",
                ),
            )
            .await;
            assert_eq!(cancelled.status, ProductToolExecutionStatus::Cancelled);
            assert_eq!(cancelled.permanent_side_effects, 0);
            let error = executor
                .cancel("cancel_native".into(), "wrong_token".into())
                .await
                .unwrap_err();
            assert_eq!(
                error.code,
                "NATIVE_PRODUCT_TOOL_CANCELLATION_TOKEN_MISMATCH"
            );
        });
    }

    #[test]
    fn in_flight_cancel_discards_late_geometry_and_never_promotes_state() {
        block_on(async {
            let started = Arc::new(tokio::sync::Notify::new());
            let executor = executor_with(
                Arc::new(PendingGeometryPort {
                    started: started.clone(),
                }),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let plan = standard_request(
                &registry,
                "plan_complete_concept",
                json!({"plan": concept_plan()}),
                "call_plan",
            );
            assert_eq!(
                run_tool(&executor, plan).await.status,
                ProductToolExecutionStatus::Completed
            );
            let build = standard_request(
                &registry,
                "build_candidate_geometry",
                json!({"direction_id": "direction_primary", "presentation_profile": "quick_sketch"}),
                "call_build",
            );
            let task_executor = executor.clone();
            let task = tokio::spawn(async move {
                task_executor
                    .execute(build, CancellationToken::new())
                    .await
                    .unwrap()
            });
            started.notified().await;
            assert!(executor
                .cancel("cancel_native".into(), "cancel_token_native".into())
                .await
                .unwrap());
            let result = task.await.unwrap();
            assert_eq!(result.status, ProductToolExecutionStatus::Cancelled);
            assert_eq!(result.permanent_side_effects, 0);
            assert!(executor.lock_inner().unwrap().preview_artifacts.is_empty());
            assert_eq!(executor.lock_inner().unwrap().preview_retained_bytes, 0);

            let compile = run_tool(
                &executor,
                standard_request(
                    &registry,
                    "compile_readback_candidate",
                    json!({}),
                    "call_compile_late",
                ),
            )
            .await;
            assert_eq!(compile.status, ProductToolExecutionStatus::Cancelled);
            assert!(executor.lock_inner().unwrap().preview_artifacts.is_empty());
        });
    }

    #[test]
    fn geometry_timeout_and_failure_are_terminal_without_partial_candidate() {
        block_on(async {
            let registry = ProductToolRegistry::default();
            let timeout_executor = executor_with(
                Arc::new(PendingGeometryPort {
                    started: Arc::new(tokio::sync::Notify::new()),
                }),
                NativeProductToolExecutorConfig {
                    max_wall_time: Duration::from_millis(10),
                    ..NativeProductToolExecutorConfig::default()
                },
            );
            assert_eq!(
                run_tool(
                    &timeout_executor,
                    standard_request(
                        &registry,
                        "plan_complete_concept",
                        json!({"plan": concept_plan()}),
                        "call_timeout_plan",
                    ),
                )
                .await
                .status,
                ProductToolExecutionStatus::Completed
            );
            let timeout = run_tool(
            &timeout_executor,
            standard_request(
                &registry,
                "build_candidate_geometry",
                json!({"direction_id": "direction_primary", "presentation_profile": "quick_sketch"}),
                "call_timeout_build",
            ),
        )
        .await;
            assert_eq!(timeout.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                timeout.failure_category,
                Some(ProductToolFailureCategory::Timeout)
            );

            let failure_executor = executor_with(
                Arc::new(FailingGeometryPort),
                NativeProductToolExecutorConfig::default(),
            );
            assert_eq!(
                run_tool(
                    &failure_executor,
                    standard_request(
                        &registry,
                        "plan_complete_concept",
                        json!({"plan": concept_plan()}),
                        "call_failure_plan",
                    ),
                )
                .await
                .status,
                ProductToolExecutionStatus::Completed
            );
            let failure = run_tool(
            &failure_executor,
            standard_request(
                &registry,
                "build_candidate_geometry",
                json!({"direction_id": "direction_primary", "presentation_profile": "quick_sketch"}),
                "call_failure_build",
            ),
        )
        .await;
            assert_eq!(failure.status, ProductToolExecutionStatus::Failed);
            assert_eq!(
                failure.failure_category,
                Some(ProductToolFailureCategory::Execution)
            );
            assert_eq!(
                failure.error_code.as_deref(),
                Some("GEOMETRY_EXECUTOR_CRASHED")
            );
        });
    }

    #[test]
    fn malicious_field_and_machine_location_never_reach_geometry_port() {
        block_on(async {
            let geometry = SuccessGeometryPort::default();
            let calls = geometry.calls.clone();
            let executor = executor_with(
                Arc::new(geometry),
                NativeProductToolExecutorConfig::default(),
            );
            let registry = ProductToolRegistry::default();
            let mut malicious_program = shape_program("box");
            malicious_program
                .as_object_mut()
                .unwrap()
                .insert("database_path".into(), json!("/tmp/forbidden.sqlite"));
            let mut malicious = standard_request(
                &registry,
                "author_shape_program",
                json!({"shape_program": shape_program("box")}),
                "call_malicious",
            );
            malicious
                .validated_arguments
                .value
                .insert("shape_program".into(), malicious_program);
            let error = executor
                .execute(malicious, CancellationToken::new())
                .await
                .unwrap_err();
            assert_eq!(error.code, "NATIVE_PRODUCT_TOOL_REQUEST_INVALID");
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        });
    }

    #[test]
    fn restricted_geometry_dto_rejects_high_level_plan_even_when_shape_is_present() {
        let mut input = EmbeddedReviewedShapeProgramCatalog
            .expand(&ReviewedCatalogRequest {
                domain_pack_id: "pack_vehicle_concept".into(),
                direction_id: "direction_primary".into(),
                variant_id: None,
                presentation_profile: "quick_sketch".into(),
                plan: concept_plan(),
                style_recipe: None,
                authored_profile_sketch: None,
                authored_shape_program: None,
                geometry_strategy: GeometryStrategy::ComponentRecipe,
            })
            .unwrap()
            .geometry_input;
        input
            .shape_program
            .as_object_mut()
            .unwrap()
            .insert("plan".into(), concept_plan());
        let error = input.validate().unwrap_err();
        assert_eq!(error.code, "RESTRICTED_GEOMETRY_INPUT_INVALID");
    }

    #[test]
    fn restricted_surface_layer_input_is_hash_sealed_and_carries_only_rust_lowered_a005() {
        let base = EmbeddedReviewedShapeProgramCatalog
            .expand(&ReviewedCatalogRequest {
                domain_pack_id: "pack_vehicle_concept".into(),
                direction_id: "direction_primary".into(),
                variant_id: None,
                presentation_profile: "quick_sketch".into(),
                plan: concept_plan(),
                style_recipe: None,
                authored_profile_sketch: None,
                authored_shape_program: None,
                geometry_strategy: GeometryStrategy::ComponentRecipe,
            })
            .unwrap()
            .geometry_input;
        let input = base
            .with_surface_layer_program(&surface_layer_program())
            .unwrap();
        input.validate().unwrap();
        let sealed = input.surface_layer_input.as_ref().unwrap();
        assert_eq!(
            input.surface_adornment_programs,
            sealed.lowering().adornments()
        );
        let serialized = serde_json::to_value(sealed).unwrap();
        assert_eq!(
            serialized["schema_version"],
            json!("RestrictedSurfaceLayerInput@1")
        );
        assert_eq!(
            serialized["lowering_sha256"],
            json!(sealed.lowering_sha256())
        );
        assert_eq!(
            serialized["lowering"]["retained_layers_sha256"],
            json!(sealed.lowering().retained_layers_sha256)
        );

        let mut forged = input.clone();
        forged.surface_layer_input.as_mut().unwrap().lowering_sha256 = "0".repeat(64);
        assert_eq!(
            forged.validate().unwrap_err().code,
            "RESTRICTED_GEOMETRY_INPUT_INVALID"
        );

        let mut mismatched_a005 = input;
        mismatched_a005.surface_adornment_programs.clear();
        assert_eq!(
            mismatched_a005.validate().unwrap_err().code,
            "RESTRICTED_GEOMETRY_INPUT_INVALID"
        );
    }

    #[test]
    fn profile_companion_requires_exact_shape_program_profile_input_binding() {
        let catalog = EmbeddedReviewedShapeProgramCatalog;
        let unbound = catalog.expand(&ReviewedCatalogRequest {
            domain_pack_id: "pack_vehicle_concept".into(),
            direction_id: "direction_primary".into(),
            variant_id: None,
            presentation_profile: "quick_sketch".into(),
            plan: concept_plan(),
            style_recipe: None,
            authored_profile_sketch: Some(profile_sketch()),
            authored_shape_program: Some(shape_program("box")),
            geometry_strategy: GeometryStrategy::AuthoredShapeProgram,
        });
        assert_eq!(
            unbound.unwrap_err().code,
            "GEOMETRY_PROFILE_COMPANION_UNBOUND"
        );

        let profile = profile_sketch();
        let mut program = shape_program("box");
        program.as_object_mut().unwrap().insert(
            "profile_inputs".into(),
            json!([{
                "input_id": "profileinput_native",
                "input_kind": "profile_sketch",
                "contract_version": "ProfileSketch@1",
                "input_sha256": sha256_hex(canonical_json(&profile).as_bytes()),
                "canonical_payload": profile
            }]),
        );
        let bound_profile = program["profile_inputs"][0]["canonical_payload"].clone();
        let bound = catalog
            .expand(&ReviewedCatalogRequest {
                domain_pack_id: "pack_vehicle_concept".into(),
                direction_id: "direction_primary".into(),
                variant_id: None,
                presentation_profile: "quick_sketch".into(),
                plan: concept_plan(),
                style_recipe: None,
                authored_profile_sketch: Some(bound_profile),
                authored_shape_program: Some(program),
                geometry_strategy: GeometryStrategy::AuthoredShapeProgram,
            })
            .unwrap();
        bound.geometry_input.validate().unwrap();
    }
}
