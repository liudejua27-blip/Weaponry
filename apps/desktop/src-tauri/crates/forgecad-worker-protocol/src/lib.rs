use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const WORKER_PROTOCOL: &str = "forgecad-worker-protocol@1";
pub const MATERIAL_PACK_ID: &str = "forgecad-hard-surface-robot";
pub const FICTIONAL_ENERGY_WEAPON_MATERIAL_PACK_ID: &str = "forgecad-fictional-energy-weapon";
pub const FICTIONAL_ENERGY_WEAPON_2K_MATERIAL_PACK_ID: &str = "forgecad-fictional-energy-weapon-2k";
const MATERIAL_PACK_MANIFEST_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../..//packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/manifest.json"
));
const FICTIONAL_ENERGY_WEAPON_MATERIAL_PACK_MANIFEST_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../..//packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/manifest.json"
));
const FICTIONAL_ENERGY_WEAPON_2K_MATERIAL_PACK_MANIFEST_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../..//packages/forgecad-assets/forgecad-fictional-energy-weapon-2k/1.0.0/manifest.json"
));
/// The isolated Worker accepts a single bounded JSON request. Geometry
/// programs are small, but `render_glb` carries the self-contained candidate
/// (including up to 64 MiB of embedded PNGs) as base64. Keep the framing
/// bounded while leaving room for that documented product maximum.
pub const MAX_WORKER_REQUEST_BYTES: usize = 96 * 1024 * 1024;
/// A 64 MiB GLB is base64 encoded in the internal response, so the response
/// envelope needs modest headroom while still being decisively bounded.
pub const MAX_WORKER_RESPONSE_BYTES: usize = 96 * 1024 * 1024;
pub const MAX_WORKER_STDERR_BYTES: usize = 64 * 1024;
/// Versioned render operation that consumes a bounded animated socket frame.
/// The operation name is part of the closed Worker protocol; the existing
/// typed-particle @1 operation remains unchanged for historical receipts.
pub const RENDER_TYPED_ANIMATED_SOCKET_PARTICLES_OPERATION: &str =
    "render_typed_animated_socket_particles";
/// Versioned animated-socket trail operation.  The payload is closed and
/// carries projection samples rather than caller-computed world points.
pub const RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION: &str =
    "render_typed_animated_socket_trails";
/// Versioned animated-socket trail + Bloom operation.  Its first three passes
/// are byte-identical to `RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION`.
pub const RENDER_TYPED_ANIMATED_SOCKET_TRAILS_BLOOM_OPERATION: &str =
    "render_typed_animated_socket_trails_bloom";
/// Closed read-only fixed-renderer projection from output pixels to exact GLB
/// triangles and primitive source lineage. The Worker returns bounded encoded
/// ids plus a semantic source table; it never persists media or advances a
/// candidate/Stage.
pub const RENDER_RASTER_ATTRIBUTION_OPERATION: &str = "render_glb_raster_attribution";
/// Closed, hash-only High/Low/Cage correspondence and ray diagnostic.  This
/// operation consumes three already-produced GLBs; it never compiles a
/// candidate and never emits a bake map or other media.
pub const PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION: &str =
    "production_weapon_high_low_cage_diagnostic";
/// Dedicated one-shot entry point for the bounded High/Low/Cage diagnostic.
/// The executable rejects other operations when launched through this entry.
pub const PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_ENTRY: &str = "--isolated-once-high-low-cage";
/// Closed producer for three independently compiled High/Low/Cage GLBs.  It
/// accepts typed GeometryProgram@2 inputs only; it does not read CAS or emit a
/// bake map.
pub const PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION: &str =
    "production_weapon_high_low_cage_artifact_producer";
/// Dedicated one-shot entry point for the bounded High/Low/Cage artifact
/// producer.  It is intentionally separate from the diagnostic entry.
pub const PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_ENTRY: &str =
    "--isolated-once-high-low-cage-producer";
/// Closed operation for the standalone Native High sibling. The protocol
/// crate owns only this boundary; the High worker keeps its mesh contracts
/// private and crosses the boundary as opaque JSON.
pub const NATIVE_HIGH_WORKER_OPERATION: &str = "forgecad.production.high-mesh-prepare@1";
/// Short alias retained for callers that refer to the operation without the
/// worker suffix.
pub const NATIVE_HIGH_OPERATION: &str = NATIVE_HIGH_WORKER_OPERATION;
/// Dedicated one-shot entry point for the fixed Native High sibling.
pub const NATIVE_HIGH_WORKER_ENTRY: &str = "--isolated-once-native-high";
/// Closed sibling operation that materializes one already hash-bound Native
/// High artifact into an embedded-only GLB. It never writes Runtime state.
pub const NATIVE_HIGH_GLB_MATERIALIZE_OPERATION: &str =
    "forgecad.production.high-mesh-glb-materialize@1";
pub const NATIVE_HIGH_GLB_MATERIALIZE_ENTRY: &str = "--isolated-once-native-high-glb";
pub const NATIVE_HIGH_GLB_REQUEST_SCHEMA_VERSION: &str = "NativeHighGlbMaterializeRequest@1";
pub const NATIVE_HIGH_GLB_RESULT_SCHEMA_VERSION: &str = "NativeHighGlbMaterializeResult@1";
pub const NATIVE_HIGH_GLB_MAX_INPUT_BYTES: usize = NATIVE_HIGH_MAX_PAYLOAD_BYTES;
pub const NATIVE_HIGH_GLB_MAX_RESULT_BYTES: usize = MAX_WORKER_RESPONSE_BYTES;
pub const NATIVE_HIGH_REQUEST_ENVELOPE_SCHEMA_VERSION: &str = "NativeHighWorkerRequestEnvelope@1";
pub const NATIVE_HIGH_RESPONSE_ENVELOPE_SCHEMA_VERSION: &str = "NativeHighWorkerResponseEnvelope@1";
pub const NATIVE_HIGH_PAYLOAD_SCHEMA_VERSION: &str = "HighMeshWorkerRequest@1";
pub const NATIVE_HIGH_RESULT_SCHEMA_VERSION: &str = "HighMeshArtifact@1";
pub const NATIVE_HIGH_MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
pub const NATIVE_HIGH_MAX_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
pub const NATIVE_HIGH_MAX_RESPONSE_BYTES: usize = MAX_WORKER_RESPONSE_BYTES;
pub const NATIVE_HIGH_MAX_RESULT_BYTES: usize = MAX_WORKER_RESPONSE_BYTES;
pub const NATIVE_HIGH_MAX_RUNTIME_MS: u64 = 10_000;
pub const NATIVE_HIGH_MAX_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
/// Dedicated Native High evaluator seam.  It is intentionally transport-only
/// and is not wired into Runtime/MCP stage or persistence paths.
pub const NATIVE_HIGH_EVALUATOR_OPERATION: &str = "forgecad.production.high-evaluator@1";
pub const NATIVE_HIGH_EVALUATOR_ENTRY: &str = "--isolated-once-native-high-evaluator";
pub const NATIVE_HIGH_EVALUATOR_REQUEST_SCHEMA_VERSION: &str = "HighEvaluatorRequest@1";
pub const NATIVE_HIGH_EVALUATOR_RESULT_SCHEMA_VERSION: &str = "HighEvaluatorResult@1";

const NATIVE_HIGH_ERROR_CODES: &[&str] = &[
    "CAPABILITY_UNAVAILABLE",
    "WORKER_PROTOCOL",
    "WORKER_TIMEOUT",
    "WORKER_CRASHED",
    "WORKER_RESOURCE_LIMIT",
    "WORKER_HASH_MISMATCH",
    "WORKER_COHORT_MISMATCH",
    "WORKER_DETERMINISM_MISMATCH",
    "HIGH_WORKER_TIMEOUT",
    "HIGH_WORKER_BUDGET_INVALID",
    "HIGH_WORKER_REQUEST_TOO_LARGE",
    "HIGH_WORKER_REQUEST_CANONICAL_MISMATCH",
    "HIGH_WORKER_OPERATION_NOT_ALLOWED",
    "HIGH_WORKER_REQUEST_SCHEMA_MISMATCH",
    "HIGH_WORKER_JSON_INVALID",
    "HIGH_WORKER_FAILED",
    "HIGH_GLB_READBACK_REJECTED",
    "HIGH_GLB_REQUEST_INVALID",
    "HIGH_GLB_BASE64_INVALID",
];
/// Closed, source-only automatic triangulated retopology operation.  It
/// derives one bounded Low mesh projection and an explicit High-to-Low
/// correspondence from one admitted High GLB. GLB lowering/CAS ownership are
/// intentionally separate. It is not an artist-authored quad topology or a
/// production-stage transition.
pub const PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION: &str = "production_weapon_low_retopology";
pub const PRODUCTION_WEAPON_LOW_RETOPOLOGY_ENTRY: &str =
    "--isolated-once-production-low-retopology";
/// Closed, source-only explicit quad draft producer.  The caller supplies an
/// already-authored `authoring-mesh@1` draft; the Worker validates that every
/// face is a quad, compiles a derived render artifact, and returns a
/// hash-bound edge-flow projection.  It never auto-retopologizes, claims
/// artist approval, or advances a production stage.
pub const PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION: &str = "production_weapon_low_quad_draft";
pub const PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ENTRY: &str =
    "--isolated-once-production-low-quad-draft";
pub const PRODUCTION_WEAPON_LOW_QUAD_DRAFT_REQUEST_SCHEMA_VERSION: &str =
    "LowQuadDraftWorkerRequest@1";
pub const PRODUCTION_WEAPON_LOW_QUAD_DRAFT_RESULT_SCHEMA_VERSION: &str =
    "LowQuadDraftWorkerResult@1";
pub const PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY: &str =
    "explicit-artist-editable-quad-draft-source-only@1";
pub const PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM: &str =
    "deterministic-explicit-quad-compile-edge-flow@1";
/// Closed, source-only Cage derivation operation. It preserves the admitted
/// Low primitive/vertex/index/face order and emits a Cage mesh projection plus
/// per-vertex offset field; GLB lowering/CAS ownership are separate. It never
/// performs a bake or advances a production stage.
pub const PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION: &str = "production_weapon_cage_offset";
pub const PRODUCTION_WEAPON_CAGE_OFFSET_ENTRY: &str = "--isolated-once-production-cage-offset";
/// Closed Worker-only High/Low/Cage geometric bake. It consumes three
/// independently hash-bound GLBs and emits deterministic 2048 Normal/AO/
/// Curvature PNG bytes. It never selects a Runtime candidate, writes CAS, or
/// advances a production stage.
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION: &str = "production_weapon_geometric_bake_2k";
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_ENTRY: &str =
    "--isolated-once-production-geometric-bake-2k";
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponGeometricBakeRequest@1";
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponGeometricBakeResult@1";
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESOLUTION: u64 = 2048;
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_NORMAL_CONVENTION: &str = "OpenGL+Y";
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_AO_SAMPLE_COUNT: u64 = 8;
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY: &str =
    "production-weapon-high-low-cage-geometric-bake@1";
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_BUDGET_PROFILE: &str =
    "source-geometric-bake-2k-bounded@1";
pub const PRODUCTION_WEAPON_GEOMETRIC_BAKE_ATLAS_POLICY: &str =
    "low-TEXCOORD_0-fixed-2048-raster@1";
/// Closed Worker-only Hero material assembly. It binds one admitted Low GLB
/// to hash-bound outputs from the geometric bake, derives the remaining fixed
/// 2K PBR channels, and emits one embedded-only GLB. It does not write CAS,
/// select a candidate, or advance a production stage.
pub const PRODUCTION_WEAPON_HERO_MATERIAL_OPERATION: &str = "production_weapon_hero_material_2k";
pub const PRODUCTION_WEAPON_HERO_MATERIAL_ENTRY: &str =
    "--isolated-once-production-hero-material-2k";
pub const PRODUCTION_WEAPON_HERO_MATERIAL_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponHeroMaterialRequest@1";
pub const PRODUCTION_WEAPON_HERO_MATERIAL_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponHeroMaterialResult@1";
pub const PRODUCTION_WEAPON_HERO_MATERIAL_POLICY: &str =
    "production-weapon-embedded-geometric-bake-hero-material@1";
/// Closed Worker-only Hero UV layout producer. It consumes one hash-bound Low
/// GLB and emits UV0/UV1, visibility-weighted density, seam/hard-edge,
/// stretch/overlap and mip-padding diagnostics. It never writes Runtime/CAS.
pub const PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION: &str = "production_weapon_hero_uv_layout";
pub const PRODUCTION_WEAPON_HERO_UV_LAYOUT_ENTRY: &str =
    "--isolated-once-production-hero-uv-layout";
pub const PRODUCTION_WEAPON_HERO_UV_LAYOUT_REQUEST_SCHEMA_VERSION: &str = "HeroUvLayoutRequest@1";
pub const PRODUCTION_WEAPON_HERO_UV_LAYOUT_RESULT_SCHEMA_VERSION: &str = "HeroUvLayout@1";
pub const PRODUCTION_WEAPON_HERO_UV_LAYOUT_POLICY: &str =
    "production-weapon-hero-uv-layout-first-person-weighted@1";
/// Closed, read-only execution-plan validator for MaterialLayerGraph@1. It
/// validates a deterministic DAG but does not evaluate textures or write CAS.
pub const PRODUCTION_WEAPON_MATERIAL_LAYER_GRAPH_PLAN_OPERATION: &str =
    "production_weapon_material_layer_graph_plan";
pub const PRODUCTION_WEAPON_MATERIAL_LAYER_GRAPH_PLAN_ENTRY: &str =
    "--isolated-once-production-material-layer-graph-plan";
/// Approved-for-evaluation Blender task identities. This operation remains
/// outside `validate_request` until its Runtime binary and sandbox gates are
/// accepted; these constants only make the closed payload contract explicit.
pub const BLENDER_RENDER_FIXED_OPERATION: &str = "blender.render_fixed@1";
pub const BLENDER_TASK_REQUEST_SCHEMA_VERSION: &str = "BlenderTaskRequest@1";
pub const BLENDER_TASK_RESULT_SCHEMA_VERSION: &str = "BlenderTaskResult@1";
pub const BLENDER_TASK_ERROR_SCHEMA_VERSION: &str = "BlenderTaskError@1";
pub const BLENDER_RENDER_FIXED_RECIPE_ID: &str = "forgecad-blender-render-fixed@1";
pub const BLENDER_RENDER_FIXED_RECIPE_VERSION: &str = "1.0.0";
pub const BLENDER_NETWORK_POLICY: &str = "disabled";
pub const BLENDER_FILESYSTEM_POLICY: &str = "runtime_scratch_only";
pub const BLENDER_SCRIPT_POLICY: &str = "frozen_bundle_only";
pub const BLENDER_OUTPUT_POLICY: &str = "runtime_cas_after_readback";
pub const BLENDER_ENVELOPE_MAX_BYTES: u64 = 100_663_296;
pub const BLENDER_STDERR_MAX_BYTES: u64 = 65_536;
pub const BLENDER_RENDER_MAX_RUNTIME_MS: u64 = 120_000;
pub const BLENDER_RENDER_MAX_CPU_SECONDS: u64 = 120;
pub const BLENDER_WORKER_MAX_MEMORY_BYTES: u64 = 536_870_912;
pub const BLENDER_GPU_MAX_BYTES: u64 = 0;
pub const BLENDER_MAX_TRIANGLES: u64 = 250_000;
pub const BLENDER_MAX_TEXTURE_BYTES: u64 = 67_108_864;

const BLENDER_MAX_INPUT_OBJECTS: usize = 16;
const BLENDER_MAX_OUTPUTS: usize = 16;
const BLENDER_MAX_BASE64_CHARS: usize = BLENDER_ENVELOPE_MAX_BYTES as usize;

const BLENDER_TASK_ERROR_CODES: [&str; 12] = [
    "CAPABILITY_UNAVAILABLE",
    "WORKER_PROTOCOL",
    "WORKER_TIMEOUT",
    "WORKER_CRASHED",
    "WORKER_RESOURCE_LIMIT",
    "WORKER_HASH_MISMATCH",
    "WORKER_READBACK_REJECTED",
    "WORKER_DETERMINISM_MISMATCH",
    "WORKER_COHORT_MISMATCH",
    "WORKER_SANDBOX_VIOLATION",
    "WORKER_LICENSE_UNAVAILABLE",
    "WORKER_PACKAGE_UNVERIFIED",
];

/// The lineage preimage used by the Runtime-owned Blender adoption gate. This
/// is an internal hash recipe, not a public JSON schema field. It makes the
/// returned media depend on the independently verified source/readback,
/// camera, material, recipe, Python bundle, cohort, and request identity.
pub const BLENDER_TASK_OUTPUT_LINEAGE_SCHEMA_VERSION: &str = "BlenderTaskOutputLineage@1";

/// One output admitted by a fixed Blender recipe. `expected_byte_size` is
/// optional because compressed PNG sizes may be bounded rather than exactly
/// known; when present it is an exact byte-size gate. This type is Runtime
/// authority data and never crosses the public BlenderTask@1 JSON boundary.
#[derive(Debug, Clone, Copy)]
pub struct BlenderTaskExpectedOutput<'a> {
    pub kind: &'a str,
    pub mime: &'a str,
    pub expected_byte_size: Option<u64>,
    pub max_byte_size: u64,
}

/// Hash-only authority assembled by Runtime after it has independently read
/// and verified the candidate, source artifact, ArtifactReadback, camera
/// profile, and material profile from Store/CAS. The worker protocol crate
/// deliberately does not depend on Store/CAS types; these hashes are the
/// hand-off token that prevents a caller from retargeting the wire payload.
#[derive(Debug, Clone, Copy)]
pub struct BlenderTaskExchangeAuthority<'a> {
    pub project_id: &'a str,
    pub candidate_id: &'a str,
    pub source_candidate_sha256: &'a str,
    pub source_artifact_sha256: &'a str,
    pub source_artifact_canonical_sha256: &'a str,
    pub source_artifact_readback_sha256: &'a str,
    pub source_artifact_readback_object_sha256: &'a str,
    pub camera_profile_sha256: &'a str,
    pub camera_profile_object_sha256: &'a str,
    pub camera_profile_canonical_sha256: &'a str,
    pub material_profile_sha256: &'a str,
    pub material_profile_object_sha256: &'a str,
    pub material_profile_canonical_sha256: &'a str,
    pub recipe_sha256: &'a str,
    pub python_bundle_sha256: &'a str,
    pub expected_build_cohort_sha256: &'a str,
    pub expected_outputs: &'a [BlenderTaskExpectedOutput<'a>],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerRequest {
    pub protocol: String,
    pub request_id: String,
    pub operation: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerResponse {
    pub protocol: String,
    pub request_id: String,
    /// The Runtime and the fixed sibling worker must be built in the same
    /// development cohort when a cohort is present. `null` is valid for
    /// ordinary source builds that intentionally omit a cohort.
    pub build_cohort_sha256: Option<String>,
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<WorkerError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerError {
    pub code: String,
    pub message: String,
}

/// Resource budget owned by the Native High transport boundary. It describes
/// envelope/input/output resources only; High mesh/detail types intentionally
/// do not appear in this crate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NativeHighWorkerBudget {
    pub max_runtime_ms: u64,
    pub max_memory_bytes: u64,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
}

/// Closed Native High sibling request envelope. `payload` remains an opaque
/// canonical JSON object so the standalone High worker is free to own its
/// internal mesh representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHighWorkerRequestEnvelope {
    pub schema_version: String,
    pub protocol: String,
    pub request_id: String,
    pub operation: String,
    pub build_cohort_sha256: Option<String>,
    pub payload: Value,
    pub payload_sha256: String,
    pub payload_bytes: u64,
    pub budget: NativeHighWorkerBudget,
    pub timeout_ms: u64,
}

/// Closed Native High sibling response envelope. The result is opaque JSON
/// for the same reason as the request payload; only its schema marker and
/// canonical transport hash/size are owned by this crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHighWorkerResponseEnvelope {
    pub schema_version: String,
    pub protocol: String,
    pub request_id: String,
    pub operation: String,
    pub build_cohort_sha256: Option<String>,
    pub ok: bool,
    pub result: Option<Value>,
    pub result_sha256: Option<String>,
    pub result_bytes: Option<u64>,
    pub error: Option<NativeHighWorkerError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeHighWorkerError {
    pub code: String,
    pub message: String,
}

/// The catalog is shared protocol data rather than a Runtime-owned mirror of
/// executable Worker state. The Worker validates against this closed value and
/// the Runtime exposes the exact same canonical JSON through its read path.
///
/// The Boolean operator is product-owned and is compiled through the fixed
/// Manifold C API bridge in the isolated Geometry Worker.  Only the bounded
/// union/difference/intersection same-Part slice is active in P0; no arbitrary script or
/// plugin can add an operator at runtime.
pub fn operator_catalog() -> Value {
    let mut catalog = json!({
        "schema_version":"OperatorCatalog@1",
        "catalog_id":"forgecad-mcp010d-hard-surface",
        "geometry_program_schema_version":"GeometryProgram@2",
        "operators":[
            {"operator_id":"forgecad.geometry.primitive@2","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"GeometryPrimitiveParameters@2","part_output_required":true,"supported_shapes":["box","cylinder","ellipsoid","sphere"]},
            {"operator_id":"forgecad.geometry.profile-extrude@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"ProfileExtrudeParameters@1","part_output_required":true,"supported_shapes":["profile-extrude"]},
            {"operator_id":"forgecad.geometry.profile-loft@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"ProfileLoftParameters@1","part_output_required":true,"supported_shapes":["profile-loft"]},
            {"operator_id":"forgecad.geometry.profile-loft@2","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"ProfileLoftParameters@2","part_output_required":true,"supported_shapes":["profile-loft-v2"]},
            {"operator_id":"forgecad.geometry.multi-loop-profile-loft@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"MultiLoopProfileLoftParameters@1","part_output_required":true,"supported_shapes":["multi-loop-profile-loft"]},
            {"operator_id":"forgecad.geometry.longitudinal-section-loft@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"LongitudinalSectionLoftParameters@1","part_output_required":true,"supported_shapes":["longitudinal-section-loft"]},
            {"operator_id":"forgecad.geometry.subd-cage@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"SubdCageParameters@1","part_output_required":true,"supported_shapes":["subd-cage"]},
            {"operator_id":"forgecad.geometry.subd-cage@2","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"SubdCageCreaseParameters@2","part_output_required":true,"supported_shapes":["subd-cage"]},
            {"operator_id":"forgecad.geometry.authoring-mesh@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"AuthoringMeshParameters@1","part_output_required":true,"supported_shapes":["authoring-mesh"]},
            {"operator_id":"forgecad.geometry.surface-patch@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"SurfacePatchParameters@1","part_output_required":true,"supported_shapes":["surface-patch"]},
            {"operator_id":"forgecad.geometry.surface-shell@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"SurfaceShellParameters@1","part_output_required":true,"supported_shapes":["surface-shell"]},
            {"operator_id":"forgecad.geometry.revolve@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"RevolveParameters@1","part_output_required":true,"supported_shapes":["revolve"]},
            {"operator_id":"forgecad.geometry.tube-sweep@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"TubeSweepParameters@1","part_output_required":true,"supported_shapes":["tube-sweep"]},
            {"operator_id":"forgecad.geometry.transform@2","status":"active","input_arity":{"min":1,"max":1},"output_kind":"triangle-mesh","parameter_schema":"TransformParameters@2","part_output_required":true,"supported_shapes":["transform"]},
            {"operator_id":"forgecad.geometry.mirror@1","status":"active","input_arity":{"min":1,"max":1},"output_kind":"triangle-mesh","parameter_schema":"MirrorParameters@1","part_output_required":true,"supported_shapes":["mirror"]},
            {"operator_id":"forgecad.geometry.array@1","status":"active","input_arity":{"min":1,"max":1},"output_kind":"triangle-mesh","parameter_schema":"ArrayParameters@1","part_output_required":true,"supported_shapes":["array"]},
            {"operator_id":"forgecad.geometry.bevel@1","status":"active","input_arity":{"min":1,"max":1},"output_kind":"triangle-mesh","parameter_schema":"BevelParameters@1","part_output_required":true,"supported_shapes":["bevel"]},
            {"operator_id":"forgecad.geometry.bevel@2","status":"active","input_arity":{"min":1,"max":1},"output_kind":"triangle-mesh","parameter_schema":"BevelParameters@2","part_output_required":true,"supported_shapes":["bevel"]},
            {"operator_id":"forgecad.geometry.normal-policy@1","status":"active","input_arity":{"min":1,"max":1},"output_kind":"triangle-mesh","parameter_schema":"NormalPolicyParameters@1","part_output_required":true,"supported_shapes":["normal-policy"]},
            {"operator_id":"forgecad.geometry.panel@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"PanelParameters@1","part_output_required":true,"supported_shapes":["panel"]},
            {"operator_id":"forgecad.geometry.panel@2","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"PanelParameters@2","part_output_required":true,"supported_shapes":["panel"]},
            {"operator_id":"forgecad.geometry.vent-array@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"VentArrayParameters@1","part_output_required":true,"supported_shapes":["vent-array"]},
            {"operator_id":"forgecad.geometry.vent-array@2","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"VentArrayParameters@2","part_output_required":true,"supported_shapes":["vent-array"]},
            {"operator_id":"forgecad.geometry.recessed-channel@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"RecessedChannelParameters@1","part_output_required":true,"supported_shapes":["recessed-channel"]},
            {"operator_id":"forgecad.geometry.energy-core@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"EnergyCoreParameters@1","part_output_required":true,"supported_shapes":["energy-core"]},
            {"operator_id":"forgecad.geometry.joint-stack@1","status":"active","input_arity":{"min":0,"max":0},"output_kind":"triangle-mesh","parameter_schema":"JointStackParameters@1","part_output_required":true,"supported_shapes":["joint-stack"]},
            {"operator_id":"forgecad.geometry.part-output@1","status":"active","input_arity":{"min":1,"max":64},"output_kind":"triangle-mesh","parameter_schema":"PartOutputParameters@1","part_output_required":true,"supported_shapes":["part-output"]},
            {"operator_id":"forgecad.geometry.boolean@1","status":"active","input_arity":{"min":2,"max":2},"output_kind":"triangle-mesh","parameter_schema":"BooleanParameters@1","part_output_required":true,"supported_shapes":["union","difference","intersection"]}
        ],
        "canonical_sha256":""
    });
    let mut without_hash = catalog
        .as_object()
        .expect("operator catalog is an object")
        .clone();
    without_hash.remove("canonical_sha256");
    catalog["canonical_sha256"] = Value::String(canonical_hash(&Value::Object(without_hash)));
    catalog
}

pub fn operator_catalog_sha256() -> String {
    operator_catalog()["canonical_sha256"]
        .as_str()
        .expect("operator catalog has a canonical hash")
        .to_owned()
}

/// Product-owned fixed render semantics shared by the isolated Render Worker
/// and Runtime. This is a clean-room contract inspired by the separation of
/// display-color and data passes in DCC render pipelines; it contains no
/// Blender/EEVEE/Cycles code, configuration or runtime dependency.
pub fn render_profile() -> Value {
    let color_pipeline = json!({
        "scene_color_space":"linear-rec709-d65",
        "display_device":"srgb",
        "view_transform":"fixed-linear-to-srgb@1",
        "look":"none",
        "exposure_stops":0,
        "gamma":1,
        "ocio_config_sha256":null
    });
    let id_palette = json!({
        "part_id":"mesh-index-part-color-v1",
        "material_id":"material-index-color-v1",
        "index_domain":{"min":0,"max":255},
        "overflow_policy":"reject-render-input"
    });
    let id_palette_definition_sha256 = canonical_hash(&id_palette);
    let aovs = json!([
        {"pass_id":"beauty","semantic_kind":"color","storage":"image/png;rgba8","encoding":"srgb-u8","source_value_range":"bounded-display-rgb-0-1","color_transform":"fixed-linear-to-srgb@1","filter":"triangle","alpha_semantics":"opaque-1","background_encoding":"rgba8:8,12,18,255","units":"display-relative","palette_definition_sha256":null,"metric_safe":false,"source_definition":"fixed-ggx-material-shading@1"},
        {"pass_id":"silhouette","semantic_kind":"mask","storage":"image/png;rgba8","encoding":"binary-mask-palette-u8","source_value_range":"background-or-foreground-palette","color_transform":"none","filter":"nearest","alpha_semantics":"opaque-1","background_encoding":"rgba8:8,12,18,255","units":"categorical","palette_definition_sha256":null,"metric_safe":true,"source_definition":"visible-fragment-mask@1"},
        {"pass_id":"depth","semantic_kind":"depth","storage":"image/png;rgba8","encoding":"reversed-normalized-depth-u8","source_value_range":"near-1-far-0-clamped","color_transform":"none","filter":"nearest","alpha_semantics":"opaque-1","background_encoding":"rgba8:8,12,18,255","units":"normalized-camera-depth-not-meters","palette_definition_sha256":null,"metric_safe":false,"source_definition":"camera-near-far-depth@1"},
        {"pass_id":"normal","semantic_kind":"normal-vector","storage":"image/png;rgba8","encoding":"signed-unit-vector-to-unorm8","source_value_range":"xyz-minus-1-to-1-mapped-0-to-255","color_transform":"none","filter":"triangle","alpha_semantics":"opaque-1","background_encoding":"rgba8:8,12,18,255","units":"world-direction","palette_definition_sha256":null,"metric_safe":false,"source_definition":"interpolated-world-normal@1"},
        {"pass_id":"ao","semantic_kind":"scalar","storage":"image/png;rgba8","encoding":"normalized-scalar-u8","source_value_range":"occlusion-0-to-1","color_transform":"none","filter":"triangle","alpha_semantics":"opaque-1","background_encoding":"rgba8:8,12,18,255","units":"unitless","palette_definition_sha256":null,"metric_safe":false,"source_definition":"screen-neighborhood-ao@1"},
        {"pass_id":"part-id","semantic_kind":"id","storage":"image/png;rgba8","encoding":"index-palette-u8","source_value_range":"categorical-mesh-index-0-255","color_transform":"none","filter":"nearest","alpha_semantics":"opaque-1","background_encoding":"rgba8:8,12,18,255","units":"categorical","palette_definition_sha256":id_palette_definition_sha256,"metric_safe":true,"source_definition":"mesh-index-part-color-v1"},
        {"pass_id":"material-id","semantic_kind":"id","storage":"image/png;rgba8","encoding":"index-palette-u8","source_value_range":"categorical-material-index-0-255","color_transform":"none","filter":"nearest","alpha_semantics":"opaque-1","background_encoding":"rgba8:8,12,18,255","units":"categorical","palette_definition_sha256":id_palette_definition_sha256,"metric_safe":true,"source_definition":"material-index-color-v1"},
        {"pass_id":"wireframe","semantic_kind":"diagnostic","storage":"image/png;rgba8","encoding":"edge-diagnostic-palette-u8","source_value_range":"edge-or-background-palette","color_transform":"none","filter":"triangle","alpha_semantics":"opaque-1","background_encoding":"rgba8:8,12,18,255","units":"categorical","palette_definition_sha256":null,"metric_safe":false,"source_definition":"barycentric-edge-diagnostic@1"},
        {"pass_id":"uv-stretch","semantic_kind":"diagnostic","storage":"image/png;rgba8","encoding":"uv-stretch-heatmap-u8","source_value_range":"bounded-relative-stretch-heatmap","color_transform":"none","filter":"triangle","alpha_semantics":"opaque-1","background_encoding":"rgba8:8,12,18,255","units":"relative-diagnostic","palette_definition_sha256":null,"metric_safe":false,"source_definition":"triangle-uv-stretch-diagnostic@1"}
    ]);
    let aov_definition_sha256 = canonical_hash(&aovs);
    let color_pipeline_sha256 = canonical_hash(&color_pipeline);
    let mut profile = json!({
        "schema_version":"RenderProfile@1",
        "profile_id":"forgecad-fixed-software-render-profile",
        "engine_id":"forgecad-fixed-software@2",
        "backend_id":"cpu-raster@1",
        "renderer_revision":"forgecad-renderer-2",
        "resolution":{"width":512,"height":512},
        "sampling":{"mode":"deterministic-raster","supersample_axis":2,"seed_policy":"not-applicable-no-rng","adaptive":false,"temporal":false,"motion_blur":false},
        "color_pipeline":color_pipeline,
        "alpha":{"background":"opaque-fixed","alpha_mode":"opaque-1","transparent_film":false},
        "aovs":aovs,
        "aov_definition_sha256":aov_definition_sha256,
        "color_pipeline_sha256":color_pipeline_sha256,
        "id_palette_definition_sha256":id_palette_definition_sha256,
        "canonical_sha256":""
    });
    let mut preimage = profile
        .as_object()
        .expect("render profile is an object")
        .clone();
    preimage.remove("canonical_sha256");
    profile["canonical_sha256"] = Value::String(canonical_hash(&Value::Object(preimage)));
    profile
}

pub fn render_profile_sha256() -> String {
    render_profile()["canonical_sha256"]
        .as_str()
        .expect("render profile canonical hash")
        .to_owned()
}

/// Runtime-owned read-only material manifest.  The manifest is compiled into
/// the same source cohort as the Worker; it is never fetched from a URL or
/// resolved through a user path.  The checked-in canonical field is verified
/// against the same canonical JSON function used by the Worker.
fn verified_material_pack_manifest(manifest_json: &str, expected_pack_id: &str) -> Value {
    let manifest: Value =
        serde_json::from_str(manifest_json).expect("ForgeCAD material pack manifest is valid JSON");
    let expected = manifest
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .expect("ForgeCAD material pack manifest has a canonical hash");
    let mut without_hash = manifest
        .as_object()
        .expect("ForgeCAD material pack manifest is an object")
        .clone();
    without_hash.remove("canonical_sha256");
    let actual = canonical_hash(&Value::Object(without_hash));
    assert_eq!(
        expected, actual,
        "ForgeCAD material pack manifest hash drifted"
    );
    assert_eq!(
        manifest.get("pack_id").and_then(Value::as_str),
        Some(expected_pack_id)
    );
    manifest
}

pub fn material_pack_manifest_by_id(pack_id: &str) -> Option<Value> {
    match pack_id {
        MATERIAL_PACK_ID => Some(verified_material_pack_manifest(
            MATERIAL_PACK_MANIFEST_JSON,
            MATERIAL_PACK_ID,
        )),
        FICTIONAL_ENERGY_WEAPON_MATERIAL_PACK_ID => Some(verified_material_pack_manifest(
            FICTIONAL_ENERGY_WEAPON_MATERIAL_PACK_MANIFEST_JSON,
            FICTIONAL_ENERGY_WEAPON_MATERIAL_PACK_ID,
        )),
        FICTIONAL_ENERGY_WEAPON_2K_MATERIAL_PACK_ID => Some(verified_material_pack_manifest(
            FICTIONAL_ENERGY_WEAPON_2K_MATERIAL_PACK_MANIFEST_JSON,
            FICTIONAL_ENERGY_WEAPON_2K_MATERIAL_PACK_ID,
        )),
        _ => None,
    }
}

pub fn material_pack_catalog() -> Value {
    let mut catalog = json!({
        "schema_version":"MaterialPackCatalog@1",
        "packs":[
            {
                "pack_id":MATERIAL_PACK_ID,
                "version":"1.0.0",
                "manifest_sha256":material_pack_manifest_by_id(MATERIAL_PACK_ID)
                    .expect("default material pack")
                    ["canonical_sha256"],
                "status":"development-only"
            },
            {
                "pack_id":FICTIONAL_ENERGY_WEAPON_MATERIAL_PACK_ID,
                "version":"1.0.0",
                "manifest_sha256":material_pack_manifest_by_id(FICTIONAL_ENERGY_WEAPON_MATERIAL_PACK_ID)
                    .expect("fictional energy weapon material pack")
                    ["canonical_sha256"],
                "status":"development-only"
            },
            {
                "pack_id":FICTIONAL_ENERGY_WEAPON_2K_MATERIAL_PACK_ID,
                "version":"1.0.0",
                "manifest_sha256":material_pack_manifest_by_id(FICTIONAL_ENERGY_WEAPON_2K_MATERIAL_PACK_ID)
                    .expect("fictional energy weapon 2K material pack")
                    ["canonical_sha256"],
                "status":"development-only"
            }
        ],
        "runtime_network":false,
        "caller_paths":false,
        "canonical_sha256":""
    });
    let mut preimage = catalog
        .as_object()
        .expect("material pack catalog is an object")
        .clone();
    preimage.remove("canonical_sha256");
    catalog["canonical_sha256"] = Value::String(canonical_hash(&Value::Object(preimage)));
    catalog
}

pub fn material_pack_manifest() -> Value {
    material_pack_manifest_by_id(MATERIAL_PACK_ID).expect("default material pack is compiled in")
}

pub fn material_pack_manifest_sha256() -> String {
    material_pack_manifest()["canonical_sha256"]
        .as_str()
        .expect("ForgeCAD material pack manifest hash")
        .to_owned()
}

pub fn material_pack_manifest_sha256_by_id(pack_id: &str) -> Option<String> {
    material_pack_manifest_by_id(pack_id)
        .and_then(|manifest| manifest["canonical_sha256"].as_str().map(str::to_owned))
}

pub fn build_cohort_sha256() -> Option<String> {
    option_env!("FORGECAD_BUILD_COHORT_SHA256")
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
}

pub fn validate_request(request: &WorkerRequest) -> Result<(), String> {
    if request.protocol != WORKER_PROTOCOL {
        return Err("worker protocol version is invalid".to_owned());
    }
    if !is_opaque_id(&request.request_id) {
        return Err("worker request_id is invalid".to_owned());
    }
    if !matches!(
        request.operation.as_str(),
        "compile_geometry"
            | "boolean_operand_lineage"
            | "subdivision_topology_lineage"
            | "render_fixed"
            | "render_glb"
            | "render_glb_vfx_frame"
            | "render_glb_vfx_bloom_frame"
            | "render_typed_particles"
            | "render_typed_trails"
            | "render_typed_trails_bloom"
            | RENDER_TYPED_ANIMATED_SOCKET_PARTICLES_OPERATION
            | RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION
            | RENDER_TYPED_ANIMATED_SOCKET_TRAILS_BLOOM_OPERATION
            | RENDER_RASTER_ATTRIBUTION_OPERATION
            | NATIVE_HIGH_WORKER_OPERATION
            | NATIVE_HIGH_GLB_MATERIALIZE_OPERATION
            | NATIVE_HIGH_EVALUATOR_OPERATION
            | PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION
            | PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION
            | PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION
            | PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION
            | PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION
            | PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION
            | PRODUCTION_WEAPON_HERO_MATERIAL_OPERATION
            | PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION
            | PRODUCTION_WEAPON_MATERIAL_LAYER_GRAPH_PLAN_OPERATION
            | "render_glb_fit_batch"
            | "geometry_program_hash"
    ) {
        return Err("worker operation is not allowlisted".to_owned());
    }
    if !request.payload.is_object() {
        return Err("worker payload must be an object".to_owned());
    }
    if request.operation == NATIVE_HIGH_WORKER_OPERATION {
        validate_native_high_payload_marker(&request.payload)?;
    } else if request.operation == NATIVE_HIGH_GLB_MATERIALIZE_OPERATION {
        validate_native_high_glb_materialize_payload(&request.payload)?;
    }
    Ok(())
}

/// Validate the protocol-owned portion of the opaque Native High payload.
/// High's nested fields are deliberately not deserialized here; the sibling
/// owns that contract and must perform its own strict validation.
pub fn validate_native_high_payload_marker(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "Native High payload must be an object".to_owned())?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(NATIVE_HIGH_PAYLOAD_SCHEMA_VERSION)
    {
        return Err("Native High payload schema marker is invalid".to_owned());
    }
    let bytes = canonical_json_bytes(value);
    if bytes.is_empty() || bytes.len() > NATIVE_HIGH_MAX_PAYLOAD_BYTES {
        return Err("Native High payload exceeds its byte bound".to_owned());
    }
    Ok(())
}

/// Validate the closed High-artifact-to-GLB payload. The artifact itself is
/// still owned by the High worker crate; this boundary binds only its schema
/// marker, canonical input hash and exact envelope fields.
pub fn validate_native_high_glb_materialize_payload(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "Native High GLB payload must be an object".to_owned())?;
    exact_value_fields(
        object,
        &[
            "schema_version",
            "artifact",
            "input_canonical_sha256",
            "canonical_sha256",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(NATIVE_HIGH_GLB_REQUEST_SCHEMA_VERSION)
    {
        return Err("Native High GLB request schema marker is invalid".to_owned());
    }
    let artifact = object
        .get("artifact")
        .and_then(Value::as_object)
        .ok_or_else(|| "Native High GLB artifact must be an object".to_owned())?;
    if artifact.get("schema_version").and_then(Value::as_str) != Some("HighMeshArtifact@1") {
        return Err("Native High GLB artifact schema marker is invalid".to_owned());
    }
    let input_hash = object
        .get("input_canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "Native High GLB input canonical hash is missing".to_owned())?;
    if !is_sha256(input_hash)
        || artifact.get("canonical_sha256").and_then(Value::as_str) != Some(input_hash)
    {
        return Err("Native High GLB input canonical hash does not bind artifact".to_owned());
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "Native High GLB request canonical hash is missing".to_owned())?;
    if !is_sha256(canonical) {
        return Err("Native High GLB request canonical hash is invalid".to_owned());
    }
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical != canonical_json_sha256(&preimage) {
        return Err("Native High GLB request canonical hash does not match".to_owned());
    }
    let bytes = canonical_json_bytes(value);
    if bytes.is_empty() || bytes.len() > NATIVE_HIGH_GLB_MAX_INPUT_BYTES {
        return Err("Native High GLB request exceeds its byte bound".to_owned());
    }
    Ok(())
}

/// Validate the closed GLB materialization result, including canonical
/// result hash and exact embedded base64 byte hash/size bindings.
pub fn validate_native_high_glb_materialize_result(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "Native High GLB result must be an object".to_owned())?;
    exact_value_fields(
        object,
        &[
            "schema_version",
            "glb_base64",
            "glb_sha256",
            "strict_readback",
            "runtime_write_performed",
            "canonical_sha256",
        ],
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(NATIVE_HIGH_GLB_RESULT_SCHEMA_VERSION)
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
    {
        return Err("Native High GLB result schema or write flag is invalid".to_owned());
    }
    let encoded = object
        .get("glb_base64")
        .and_then(Value::as_str)
        .ok_or_else(|| "Native High GLB base64 is missing".to_owned())?;
    let glb = decode_base64_strict(encoded, NATIVE_HIGH_GLB_MAX_RESULT_BYTES)
        .map_err(|_| "Native High GLB base64 is invalid".to_owned())?;
    let glb_hash = object
        .get("glb_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "Native High GLB hash is missing".to_owned())?;
    if !is_sha256(glb_hash) || glb_hash != hex_sha256(&glb) {
        return Err("Native High GLB hash does not match bytes".to_owned());
    }
    let readback = object
        .get("strict_readback")
        .and_then(Value::as_object)
        .ok_or_else(|| "Native High GLB strict readback is missing".to_owned())?;
    exact_value_fields(
        readback,
        &[
            "glb_sha256",
            "source_artifact_id",
            "source_artifact_sha256",
            "part_ids",
            "base_primitive_count",
            "detail_primitive_count",
            "base_triangle_count",
            "detail_triangle_count",
            "triangle_count",
            "byte_length",
        ],
    )?;
    if readback.get("glb_sha256").and_then(Value::as_str) != Some(glb_hash)
        || readback.get("byte_length").and_then(Value::as_u64) != Some(glb.len() as u64)
        || readback
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            .map_or(true, |value| !is_sha256(value))
        || readback.get("part_ids").and_then(Value::as_array).is_none()
    {
        return Err("Native High GLB strict readback binding is invalid".to_owned());
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "Native High GLB result canonical hash is missing".to_owned())?;
    if !is_sha256(canonical) {
        return Err("Native High GLB result canonical hash is invalid".to_owned());
    }
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical != canonical_json_sha256(&preimage) {
        return Err("Native High GLB result canonical hash does not match".to_owned());
    }
    let bytes = canonical_json_bytes(value);
    if bytes.len() > NATIVE_HIGH_GLB_MAX_RESULT_BYTES {
        return Err("Native High GLB result exceeds its byte bound".to_owned());
    }
    Ok(())
}

fn exact_value_fields(
    object: &serde_json::Map<String, Value>,
    fields: &[&str],
) -> Result<(), String> {
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err("closed Native High GLB object fields drifted".to_owned());
    }
    Ok(())
}

fn decode_base64_strict(value: &str, max_bytes: usize) -> Result<Vec<u8>, ()> {
    if value.is_empty()
        || value.len() % 4 != 0
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(());
    }
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for (chunk_index, chunk) in bytes.chunks_exact(4).enumerate() {
        let padding = usize::from(chunk[2] == b'=') + usize::from(chunk[3] == b'=');
        let is_last = chunk_index + 1 == bytes.len() / 4;
        if padding != 0 && !is_last {
            return Err(());
        }
        if chunk[2] == b'=' && chunk[3] != b'=' {
            return Err(());
        }
        let a = base64_digit(chunk[0]).ok_or(())? as u32;
        let b = base64_digit(chunk[1]).ok_or(())? as u32;
        let c = if chunk[2] == b'=' {
            0
        } else {
            base64_digit(chunk[2]).ok_or(())? as u32
        };
        let d = if chunk[3] == b'=' {
            0
        } else {
            base64_digit(chunk[3]).ok_or(())? as u32
        };
        let value = (a << 18) | (b << 12) | (c << 6) | d;
        if padding == 2 && (c != 0 || (b & 0x0f) != 0) {
            return Err(());
        }
        if padding == 1 && (d != 0 || (c & 0x03) != 0) {
            return Err(());
        }
        output.push((value >> 16) as u8);
        if padding < 2 {
            output.push((value >> 8) as u8);
        }
        if padding == 0 {
            output.push(value as u8);
        }
    }
    if output.len() > max_bytes {
        return Err(());
    }
    Ok(output)
}

fn base64_digit(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Validate a closed Native High request envelope, including its opaque
/// payload's canonical hash/size, cohort binding, resource budget and timeout.
pub fn validate_native_high_request(
    request: &NativeHighWorkerRequestEnvelope,
) -> Result<(), String> {
    if request.schema_version != NATIVE_HIGH_REQUEST_ENVELOPE_SCHEMA_VERSION {
        return Err("Native High request envelope schema is invalid".to_owned());
    }
    if request.protocol != WORKER_PROTOCOL {
        return Err("Native High request protocol is invalid".to_owned());
    }
    if !is_opaque_id(&request.request_id) {
        return Err("Native High request_id is invalid".to_owned());
    }
    if request.operation != NATIVE_HIGH_WORKER_OPERATION {
        return Err("Native High operation is not allowlisted".to_owned());
    }
    if request
        .build_cohort_sha256
        .as_deref()
        .is_some_and(|value| !is_sha256(value))
    {
        return Err("Native High request cohort is invalid".to_owned());
    }
    validate_native_high_payload_marker(&request.payload)?;
    if !is_sha256(&request.payload_sha256) {
        return Err("Native High payload hash is invalid".to_owned());
    }
    let payload_bytes = canonical_json_bytes(&request.payload);
    if request.payload_bytes != payload_bytes.len() as u64 {
        return Err("Native High payload byte size does not match canonical JSON".to_owned());
    }
    if request.payload_sha256 != canonical_json_sha256(&request.payload) {
        return Err("Native High payload hash does not match canonical JSON".to_owned());
    }
    validate_native_high_budget(&request.budget)?;
    if request.payload_bytes > request.budget.max_input_bytes {
        return Err("Native High payload exceeds max_input_bytes".to_owned());
    }
    if request.timeout_ms == 0 || request.timeout_ms > request.budget.max_runtime_ms {
        return Err("Native High timeout is outside the requested budget".to_owned());
    }
    let envelope_bytes = serde_json::to_vec(request)
        .map_err(|_| "Native High request envelope cannot be serialized".to_owned())?;
    if envelope_bytes.len() > NATIVE_HIGH_MAX_REQUEST_BYTES {
        return Err("Native High request envelope exceeds its byte bound".to_owned());
    }
    Ok(())
}

/// Parse and validate a Native High request before dispatch. Parsing through
/// this function makes `deny_unknown_fields` part of the wire boundary rather
/// than an optional caller convention.
pub fn parse_native_high_request(bytes: &[u8]) -> Result<NativeHighWorkerRequestEnvelope, String> {
    if bytes.len() > NATIVE_HIGH_MAX_REQUEST_BYTES {
        return Err("Native High request envelope exceeds its byte bound".to_owned());
    }
    let request =
        serde_json::from_slice::<NativeHighWorkerRequestEnvelope>(bytes).map_err(|_| {
            "Native High request envelope JSON is invalid or contains unknown fields".to_owned()
        })?;
    validate_native_high_request(&request)?;
    Ok(request)
}

/// Validate a closed Native High error. Timeout is an explicit typed failure,
/// never a successful response with an absent result.
pub fn validate_native_high_error(error: &NativeHighWorkerError) -> Result<(), String> {
    if !NATIVE_HIGH_ERROR_CODES.contains(&error.code.as_str()) {
        return Err("Native High error code is not allowlisted".to_owned());
    }
    if error.message.is_empty()
        || error.message.len() > 256
        || !error
            .message
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        return Err("Native High error message is invalid".to_owned());
    }
    if matches!(
        error.code.as_str(),
        "WORKER_TIMEOUT" | "HIGH_WORKER_TIMEOUT"
    ) && error.message != "timeout"
    {
        return Err("Native High timeout error must use the fixed message".to_owned());
    }
    Ok(())
}

/// Validate a Native High response against its exact request envelope.
pub fn validate_native_high_response(
    response: &NativeHighWorkerResponseEnvelope,
    request: &NativeHighWorkerRequestEnvelope,
) -> Result<(), String> {
    validate_native_high_request(request)?;
    if response.schema_version != NATIVE_HIGH_RESPONSE_ENVELOPE_SCHEMA_VERSION {
        return Err("Native High response envelope schema is invalid".to_owned());
    }
    if response.protocol != WORKER_PROTOCOL
        || response.request_id != request.request_id
        || response.operation != request.operation
    {
        return Err("Native High response envelope binding is invalid".to_owned());
    }
    if response.build_cohort_sha256 != request.build_cohort_sha256 {
        return Err("Native High response cohort differs from the request".to_owned());
    }
    if response.ok {
        if response.result.is_none()
            || response.error.is_some()
            || response.result_sha256.as_deref().is_none()
            || response.result_bytes.is_none()
        {
            return Err("Native High success response has an invalid result envelope".to_owned());
        }
        let result = response.result.as_ref().expect("checked above");
        let result_object = result
            .as_object()
            .ok_or_else(|| "Native High result must be an object".to_owned())?;
        if result_object.get("schema_version").and_then(Value::as_str)
            != Some(NATIVE_HIGH_RESULT_SCHEMA_VERSION)
        {
            return Err("Native High result schema marker is invalid".to_owned());
        }
        let result_bytes = canonical_json_bytes(result);
        if result_bytes.is_empty()
            || result_bytes.len() > NATIVE_HIGH_MAX_RESULT_BYTES
            || result_bytes.len() as u64 > request.budget.max_output_bytes
            || response.result_bytes != Some(result_bytes.len() as u64)
        {
            return Err("Native High result exceeds or disagrees with its byte budget".to_owned());
        }
        let result_sha256 = response.result_sha256.as_deref().expect("checked above");
        if !is_sha256(result_sha256) || result_sha256 != canonical_json_sha256(result) {
            return Err("Native High result hash does not match canonical JSON".to_owned());
        }
    } else {
        if response.result.is_some()
            || response.result_sha256.is_some()
            || response.result_bytes.is_some()
        {
            return Err("Native High failed response must not carry a result".to_owned());
        }
        let error = response
            .error
            .as_ref()
            .ok_or_else(|| "Native High failed response lacks an error".to_owned())?;
        validate_native_high_error(error)?;
    }
    let envelope_bytes = serde_json::to_vec(response)
        .map_err(|_| "Native High response envelope cannot be serialized".to_owned())?;
    if envelope_bytes.len() > NATIVE_HIGH_MAX_RESPONSE_BYTES {
        return Err("Native High response envelope exceeds its byte bound".to_owned());
    }
    Ok(())
}

pub fn parse_native_high_response(
    bytes: &[u8],
    request: &NativeHighWorkerRequestEnvelope,
) -> Result<NativeHighWorkerResponseEnvelope, String> {
    if bytes.len() > NATIVE_HIGH_MAX_RESPONSE_BYTES {
        return Err("Native High response envelope exceeds its byte bound".to_owned());
    }
    let response =
        serde_json::from_slice::<NativeHighWorkerResponseEnvelope>(bytes).map_err(|_| {
            "Native High response envelope JSON is invalid or contains unknown fields".to_owned()
        })?;
    validate_native_high_response(&response, request)?;
    Ok(response)
}

fn validate_native_high_budget(budget: &NativeHighWorkerBudget) -> Result<(), String> {
    if !(1..=NATIVE_HIGH_MAX_RUNTIME_MS).contains(&budget.max_runtime_ms)
        || !(1..=NATIVE_HIGH_MAX_MEMORY_BYTES).contains(&budget.max_memory_bytes)
        || !(1..=NATIVE_HIGH_MAX_PAYLOAD_BYTES as u64).contains(&budget.max_input_bytes)
        || !(1..=NATIVE_HIGH_MAX_RESULT_BYTES as u64).contains(&budget.max_output_bytes)
    {
        return Err("Native High budget is outside its fixed bounds".to_owned());
    }
    Ok(())
}

/// Validate the closed `BlenderTaskRequest@1` payload without selecting or
/// launching a Blender process. This takes JSON rather than a Rust contract
/// type so unknown fields are rejected before typed deserialization can omit
/// them at the Worker boundary.
pub fn validate_blender_task_request_value(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "schema_version",
        "project_id",
        "candidate_id",
        "source_candidate_sha256",
        "recipe_id",
        "recipe_version",
        "recipe_sha256",
        "python_bundle_sha256",
        "input_objects",
        "camera_profile_sha256",
        "material_profile_sha256",
        "budgets",
        "network_policy",
        "filesystem_policy",
        "script_policy",
        "output_policy",
        "canonical_sha256",
    ];
    let object = blender_require_object(value, FIELDS, "BlenderTaskRequest")?;

    blender_require_const_string(
        object,
        "schema_version",
        BLENDER_TASK_REQUEST_SCHEMA_VERSION,
    )?;
    blender_validate_id(object, "project_id")?;
    blender_validate_id(object, "candidate_id")?;
    blender_validate_sha256(object, "source_candidate_sha256")?;
    blender_require_const_string(object, "recipe_id", BLENDER_RENDER_FIXED_RECIPE_ID)?;
    blender_require_const_string(
        object,
        "recipe_version",
        BLENDER_RENDER_FIXED_RECIPE_VERSION,
    )?;
    blender_validate_sha256(object, "recipe_sha256")?;
    blender_validate_sha256(object, "python_bundle_sha256")?;
    blender_validate_sha256(object, "camera_profile_sha256")?;
    blender_validate_sha256(object, "material_profile_sha256")?;
    blender_require_const_string(object, "network_policy", BLENDER_NETWORK_POLICY)?;
    blender_require_const_string(object, "filesystem_policy", BLENDER_FILESYSTEM_POLICY)?;
    blender_require_const_string(object, "script_policy", BLENDER_SCRIPT_POLICY)?;
    blender_require_const_string(object, "output_policy", BLENDER_OUTPUT_POLICY)?;

    let budgets = blender_validate_budgets(blender_required(object, "budgets")?)?;
    let input_values = blender_required(object, "input_objects")?
        .as_array()
        .ok_or_else(|| "BlenderTaskRequest.input_objects must be an array".to_owned())?;
    if input_values.is_empty() || input_values.len() > BLENDER_MAX_INPUT_OBJECTS {
        return Err("BlenderTaskRequest.input_objects count is outside its bound".to_owned());
    }
    let mut input_bytes = 0_u64;
    for (index, input) in input_values.iter().enumerate() {
        let byte_size = validate_blender_input_object(input, index)?;
        input_bytes = input_bytes
            .checked_add(byte_size)
            .ok_or_else(|| "BlenderTaskRequest input byte budget overflowed".to_owned())?;
    }
    if input_bytes > budgets.max_input_bytes {
        return Err("BlenderTaskRequest input bytes exceed max_input_bytes".to_owned());
    }

    let canonical = blender_required_string(object, "canonical_sha256")?;
    let expected = blender_canonical_without_field(value, "canonical_sha256")?;
    if canonical != expected {
        return Err("BlenderTaskRequest canonical_sha256 does not match".to_owned());
    }
    Ok(())
}

/// Validate the closed, non-promoting `BlenderTaskResult@1` payload. Output
/// bytes are permitted only as bounded internal base64 transport; the result
/// never authorizes CAS, Stage, candidate, version, or export writes.
pub fn validate_blender_task_result_value(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "schema_version",
        "project_id",
        "candidate_id",
        "recipe_sha256",
        "python_bundle_sha256",
        "build_cohort_sha256",
        "input_canonical_sha256",
        "outputs",
        "checks",
        "runtime_write",
        "worker_started",
        "stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "canonical_sha256",
    ];
    let object = blender_require_object(value, FIELDS, "BlenderTaskResult")?;

    blender_require_const_string(object, "schema_version", BLENDER_TASK_RESULT_SCHEMA_VERSION)?;
    blender_validate_id(object, "project_id")?;
    blender_validate_id(object, "candidate_id")?;
    blender_validate_sha256(object, "recipe_sha256")?;
    blender_validate_sha256(object, "python_bundle_sha256")?;
    blender_validate_sha256(object, "build_cohort_sha256")?;
    blender_validate_sha256(object, "input_canonical_sha256")?;

    let output_values = blender_required(object, "outputs")?
        .as_array()
        .ok_or_else(|| "BlenderTaskResult.outputs must be an array".to_owned())?;
    if output_values.is_empty() || output_values.len() > BLENDER_MAX_OUTPUTS {
        return Err("BlenderTaskResult.outputs count is outside its bound".to_owned());
    }
    let mut output_bytes = 0_u64;
    for (index, output) in output_values.iter().enumerate() {
        let byte_size = validate_blender_output_object(output, index)?;
        output_bytes = output_bytes
            .checked_add(byte_size)
            .ok_or_else(|| "BlenderTaskResult output byte budget overflowed".to_owned())?;
    }
    if output_bytes > BLENDER_ENVELOPE_MAX_BYTES {
        return Err("BlenderTaskResult output bytes exceed the response ceiling".to_owned());
    }

    validate_blender_task_checks(blender_required(object, "checks")?)?;
    for field in [
        "runtime_write",
        "stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        if blender_required(object, field)? != &Value::Bool(false) {
            return Err(format!("BlenderTaskResult.{field} must be false"));
        }
    }
    if blender_required(object, "worker_started")?
        .as_bool()
        .is_none()
    {
        return Err("BlenderTaskResult.worker_started must be a boolean".to_owned());
    }

    let canonical = blender_required_string(object, "canonical_sha256")?;
    let expected = blender_canonical_without_field(value, "canonical_sha256")?;
    if canonical != expected {
        return Err("BlenderTaskResult canonical_sha256 does not match".to_owned());
    }
    Ok(())
}

/// Validate one successful Blender evaluation exchange as a single immutable
/// unit.  The individual request/result validators intentionally remain useful
/// at their transport boundaries; this validator adds the cross-message
/// bindings required before Runtime may consider adopting any returned bytes.
///
/// It does not write CAS, advance a Stage, or make a visual-quality claim.
pub fn validate_blender_task_exchange(
    request: &Value,
    result: &Value,
    expected_build_cohort_sha256: &str,
) -> Result<(), String> {
    validate_blender_task_request_value(request)?;
    validate_blender_task_result_value(result)?;
    if !is_sha256(expected_build_cohort_sha256) {
        return Err("Blender task expected build cohort is not a SHA-256".to_owned());
    }

    let request_object = request
        .as_object()
        .ok_or_else(|| "BlenderTaskRequest must be an object".to_owned())?;
    let result_object = result
        .as_object()
        .ok_or_else(|| "BlenderTaskResult must be an object".to_owned())?;

    for (request_field, result_field) in [
        ("project_id", "project_id"),
        ("candidate_id", "candidate_id"),
        ("recipe_sha256", "recipe_sha256"),
        ("python_bundle_sha256", "python_bundle_sha256"),
    ] {
        if blender_required(request_object, request_field)?
            != blender_required(result_object, result_field)?
        {
            return Err(format!(
                "Blender task exchange {result_field} differs from the request"
            ));
        }
    }

    if blender_required_string(result_object, "input_canonical_sha256")?
        != blender_required_string(request_object, "canonical_sha256")?
    {
        return Err("Blender task result input canonical hash differs from the request".to_owned());
    }
    if blender_required_string(result_object, "build_cohort_sha256")?
        != expected_build_cohort_sha256
    {
        return Err("Blender task result build cohort differs".to_owned());
    }
    if blender_required(result_object, "worker_started")? != &Value::Bool(true) {
        return Err("Blender task successful result must record worker_started=true".to_owned());
    }

    let budgets = blender_validate_budgets(blender_required(request_object, "budgets")?)?;
    let outputs = blender_required(result_object, "outputs")?
        .as_array()
        .ok_or_else(|| "BlenderTaskResult.outputs must be an array".to_owned())?;
    let mut output_bytes = 0_u64;
    let mut output_kinds = Vec::with_capacity(outputs.len());
    for (index, output) in outputs.iter().enumerate() {
        let output_object = output
            .as_object()
            .ok_or_else(|| format!("BlenderTaskResult.outputs[{index}] must be an object"))?;
        let kind = blender_required_string(output_object, "kind")?;
        if output_kinds.contains(&kind) {
            return Err("Blender task result contains duplicate output kinds".to_owned());
        }
        output_kinds.push(kind);
        output_bytes = output_bytes
            .checked_add(blender_required_u64(output_object, "byte_size")?)
            .ok_or_else(|| "Blender task result output byte budget overflowed".to_owned())?;
    }
    if output_bytes > budgets.max_output_bytes {
        return Err("Blender task result exceeds request max_output_bytes".to_owned());
    }

    let checks = blender_required(result_object, "checks")?
        .as_object()
        .ok_or_else(|| "BlenderTaskResult.checks must be an object".to_owned())?;
    for field in [
        "validator_status",
        "readback_status",
        "deterministic_replay_status",
    ] {
        if blender_required_string(checks, field)? != "passed" {
            return Err(format!(
                "Blender task accepted exchange requires checks.{field}=passed"
            ));
        }
    }
    Ok(())
}

/// Validate the fixed recipe's Runtime authority before it is compared with
/// an untrusted Worker exchange. This is intentionally hash-only: Runtime is
/// responsible for obtaining these values from Store/CAS after independent
/// byte/readback validation, while this crate prevents retargeting at the
/// protocol boundary.
fn validate_blender_task_exchange_authority(
    authority: &BlenderTaskExchangeAuthority<'_>,
) -> Result<(), String> {
    for (field, value) in [
        ("project_id", authority.project_id),
        ("candidate_id", authority.candidate_id),
    ] {
        if !is_opaque_id(value) {
            return Err(format!("Blender task authority {field} is invalid"));
        }
    }
    for (field, value) in [
        ("source_candidate_sha256", authority.source_candidate_sha256),
        ("source_artifact_sha256", authority.source_artifact_sha256),
        (
            "source_artifact_canonical_sha256",
            authority.source_artifact_canonical_sha256,
        ),
        (
            "source_artifact_readback_sha256",
            authority.source_artifact_readback_sha256,
        ),
        (
            "source_artifact_readback_object_sha256",
            authority.source_artifact_readback_object_sha256,
        ),
        ("camera_profile_sha256", authority.camera_profile_sha256),
        (
            "camera_profile_object_sha256",
            authority.camera_profile_object_sha256,
        ),
        (
            "camera_profile_canonical_sha256",
            authority.camera_profile_canonical_sha256,
        ),
        ("material_profile_sha256", authority.material_profile_sha256),
        (
            "material_profile_object_sha256",
            authority.material_profile_object_sha256,
        ),
        (
            "material_profile_canonical_sha256",
            authority.material_profile_canonical_sha256,
        ),
        ("recipe_sha256", authority.recipe_sha256),
        ("python_bundle_sha256", authority.python_bundle_sha256),
        (
            "expected_build_cohort_sha256",
            authority.expected_build_cohort_sha256,
        ),
    ] {
        if !is_sha256(value) {
            return Err(format!("Blender task authority {field} is not a SHA-256"));
        }
    }

    if authority.expected_outputs.is_empty()
        || authority.expected_outputs.len() > BLENDER_MAX_OUTPUTS
    {
        return Err("Blender task authority output set is outside its bound".to_owned());
    }
    let mut kinds = Vec::with_capacity(authority.expected_outputs.len());
    for expected in authority.expected_outputs {
        if !matches!(
            expected.kind,
            "beauty"
                | "silhouette"
                | "depth"
                | "normal"
                | "ao"
                | "part-id"
                | "material-id"
                | "wireframe"
                | "uv-stretch"
        ) {
            return Err(format!(
                "Blender task authority output kind {} is not allowlisted",
                expected.kind
            ));
        }
        if kinds.iter().any(|kind| *kind == expected.kind) {
            return Err("Blender task authority output kinds must be unique".to_owned());
        }
        kinds.push(expected.kind);
        if !matches!(
            expected.mime,
            "image/png" | "model/gltf-binary" | "application/json"
        ) {
            return Err(format!(
                "Blender task authority output MIME {} is not allowlisted",
                expected.mime
            ));
        }
        if expected.max_byte_size == 0 || expected.max_byte_size > BLENDER_ENVELOPE_MAX_BYTES {
            return Err("Blender task authority output size is outside its bound".to_owned());
        }
        if expected
            .expected_byte_size
            .is_some_and(|size| size == 0 || size > expected.max_byte_size)
        {
            return Err("Blender task authority exact output size is invalid".to_owned());
        }
    }
    Ok(())
}

fn blender_authority_request_field(
    request_object: &serde_json::Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), String> {
    if blender_required_string(request_object, field)? != expected {
        return Err(format!("Blender task authority {field} differs"));
    }
    Ok(())
}

fn validate_blender_task_authority_inputs(
    request_object: &serde_json::Map<String, Value>,
    authority: &BlenderTaskExchangeAuthority<'_>,
) -> Result<(), String> {
    blender_authority_request_field(request_object, "project_id", authority.project_id)?;
    blender_authority_request_field(request_object, "candidate_id", authority.candidate_id)?;
    blender_authority_request_field(
        request_object,
        "source_candidate_sha256",
        authority.source_candidate_sha256,
    )?;
    blender_authority_request_field(
        request_object,
        "camera_profile_sha256",
        authority.camera_profile_sha256,
    )?;
    blender_authority_request_field(
        request_object,
        "material_profile_sha256",
        authority.material_profile_sha256,
    )?;
    blender_authority_request_field(request_object, "recipe_sha256", authority.recipe_sha256)?;
    blender_authority_request_field(
        request_object,
        "python_bundle_sha256",
        authority.python_bundle_sha256,
    )?;

    let input_values = blender_required(request_object, "input_objects")?
        .as_array()
        .ok_or_else(|| "BlenderTaskRequest.input_objects must be an array".to_owned())?;
    let mut source_count = 0_u8;
    let mut material_count = 0_u8;
    for input in input_values {
        let input_object = input
            .as_object()
            .ok_or_else(|| "Blender task authority input must be an object".to_owned())?;
        let kind = blender_required_string(input_object, "kind")?;
        match kind {
            "glb" => {
                source_count = source_count
                    .checked_add(1)
                    .ok_or_else(|| "Blender task source GLB count overflowed".to_owned())?;
                if blender_required_string(input_object, "sha256")?
                    != authority.source_artifact_sha256
                {
                    return Err(
                        "Blender task source GLB differs from authority artifact".to_owned()
                    );
                }
                if blender_required_string(input_object, "canonical_sha256")?
                    != authority.source_artifact_canonical_sha256
                {
                    return Err(
                        "Blender task source GLB canonical hash differs from authority".to_owned(),
                    );
                }
                if blender_required_string(input_object, "mime")? != "model/gltf-binary" {
                    return Err("Blender task source GLB MIME is invalid".to_owned());
                }
            }
            "material_profile" => {
                material_count = material_count
                    .checked_add(1)
                    .ok_or_else(|| "Blender task material profile count overflowed".to_owned())?;
                if blender_required_string(input_object, "sha256")?
                    != authority.material_profile_object_sha256
                {
                    return Err(
                        "Blender task material object differs from authority object".to_owned()
                    );
                }
                if blender_required_string(input_object, "canonical_sha256")?
                    != authority.material_profile_canonical_sha256
                {
                    return Err(
                        "Blender task material canonical hash differs from authority".to_owned(),
                    );
                }
                if blender_required_string(input_object, "mime")? != "application/json" {
                    return Err("Blender task material profile MIME is invalid".to_owned());
                }
            }
            "reference_image" => {}
            other => {
                return Err(format!(
                    "Blender task authority input kind {other} is not allowlisted"
                ));
            }
        }
    }
    if source_count != 1 {
        return Err("Blender task requires exactly one source GLB".to_owned());
    }
    if material_count != 1 {
        return Err("Blender task requires exactly one material profile".to_owned());
    }
    Ok(())
}

/// Recompute the Runtime-owned lineage hash for one output. The worker's
/// lineage field is never trusted; all fields that identify the source and
/// evaluation context come from the authority created by Runtime.
pub fn blender_task_output_lineage_sha256(
    request: &Value,
    authority: &BlenderTaskExchangeAuthority<'_>,
    output: &Value,
) -> Result<String, String> {
    validate_blender_task_exchange_authority(authority)?;
    let request_object = request
        .as_object()
        .ok_or_else(|| "Blender task lineage request must be an object".to_owned())?;
    let output_object = output
        .as_object()
        .ok_or_else(|| "Blender task lineage output must be an object".to_owned())?;
    let preimage = json!({
        "schema_version": BLENDER_TASK_OUTPUT_LINEAGE_SCHEMA_VERSION,
        "project_id": authority.project_id,
        "candidate_id": authority.candidate_id,
        "source_candidate_sha256": authority.source_candidate_sha256,
        "source_artifact_sha256": authority.source_artifact_sha256,
        "source_artifact_canonical_sha256": authority.source_artifact_canonical_sha256,
        "source_artifact_readback_sha256": authority.source_artifact_readback_sha256,
        "source_artifact_readback_object_sha256": authority.source_artifact_readback_object_sha256,
        "camera_profile_sha256": authority.camera_profile_sha256,
        "camera_profile_object_sha256": authority.camera_profile_object_sha256,
        "camera_profile_canonical_sha256": authority.camera_profile_canonical_sha256,
        "material_profile_sha256": authority.material_profile_sha256,
        "material_profile_object_sha256": authority.material_profile_object_sha256,
        "material_profile_canonical_sha256": authority.material_profile_canonical_sha256,
        "recipe_sha256": authority.recipe_sha256,
        "python_bundle_sha256": authority.python_bundle_sha256,
        "build_cohort_sha256": authority.expected_build_cohort_sha256,
        "request_canonical_sha256": blender_required_string(request_object, "canonical_sha256")?,
        "output_kind": blender_required_string(output_object, "kind")?,
        "output_mime": blender_required_string(output_object, "mime")?,
        "output_byte_size": blender_required_u64(output_object, "byte_size")?,
        "output_sha256": blender_required_string(output_object, "sha256")?,
    });
    Ok(canonical_hash(&preimage))
}

/// Validate an inner Blender request/result against Runtime's verified hash
/// authority. This does not launch a process or write CAS/SQLite.
pub fn validate_blender_task_exchange_against_authority(
    request: &Value,
    result: &Value,
    authority: &BlenderTaskExchangeAuthority<'_>,
) -> Result<(), String> {
    validate_blender_task_exchange_authority(authority)?;
    validate_blender_task_exchange(request, result, authority.expected_build_cohort_sha256)?;
    let request_object = request
        .as_object()
        .ok_or_else(|| "BlenderTaskRequest must be an object".to_owned())?;
    let result_object = result
        .as_object()
        .ok_or_else(|| "BlenderTaskResult must be an object".to_owned())?;
    validate_blender_task_authority_inputs(request_object, authority)?;
    for (field, expected) in [
        ("project_id", authority.project_id),
        ("candidate_id", authority.candidate_id),
        ("recipe_sha256", authority.recipe_sha256),
        ("python_bundle_sha256", authority.python_bundle_sha256),
    ] {
        if blender_required_string(result_object, field)? != expected {
            return Err(format!("Blender task result authority {field} differs"));
        }
    }
    if blender_required_string(result_object, "build_cohort_sha256")?
        != authority.expected_build_cohort_sha256
    {
        return Err("Blender task result authority cohort differs".to_owned());
    }

    let outputs = blender_required(result_object, "outputs")?
        .as_array()
        .ok_or_else(|| "BlenderTaskResult.outputs must be an array".to_owned())?;
    if outputs.len() != authority.expected_outputs.len() {
        return Err("Blender task result output count differs from authority".to_owned());
    }
    for (index, (output, expected)) in outputs
        .iter()
        .zip(authority.expected_outputs.iter())
        .enumerate()
    {
        let output_object = output
            .as_object()
            .ok_or_else(|| format!("BlenderTaskResult.outputs[{index}] must be an object"))?;
        if blender_required_string(output_object, "kind")? != expected.kind {
            return Err(format!(
                "Blender task output[{index}] kind differs from authority"
            ));
        }
        if blender_required_string(output_object, "mime")? != expected.mime {
            return Err(format!(
                "Blender task output[{index}] MIME differs from authority"
            ));
        }
        let byte_size = blender_required_u64(output_object, "byte_size")?;
        if byte_size > expected.max_byte_size {
            return Err(format!(
                "Blender task output[{index}] exceeds authority size"
            ));
        }
        if expected
            .expected_byte_size
            .is_some_and(|size| size != byte_size)
        {
            return Err(format!(
                "Blender task output[{index}] exact size differs from authority"
            ));
        }
        let expected_lineage = blender_task_output_lineage_sha256(request, authority, output)?;
        if blender_required_string(output_object, "lineage_sha256")? != expected_lineage {
            return Err(format!(
                "Blender task output[{index}] lineage differs from authority"
            ));
        }
    }
    Ok(())
}

/// Validate a complete fixed Blender Worker envelope. Blender intentionally
/// remains outside the generic `validate_request` allowlist while it is only
/// approved for evaluation, so this function checks the fixed operation
/// explicitly without enabling generic Worker dispatch.
pub fn validate_blender_task_envelope_exchange(
    request: &WorkerRequest,
    response: &WorkerResponse,
    authority: &BlenderTaskExchangeAuthority<'_>,
) -> Result<(), String> {
    validate_blender_task_exchange_authority(authority)?;
    if request.protocol != WORKER_PROTOCOL || !is_opaque_id(&request.request_id) {
        return Err("Blender task request envelope is invalid".to_owned());
    }
    if request.operation != BLENDER_RENDER_FIXED_OPERATION {
        return Err("Blender task operation is not the fixed render operation".to_owned());
    }
    if response.protocol != WORKER_PROTOCOL
        || response.request_id != request.request_id
        || response.build_cohort_sha256.as_deref() != Some(authority.expected_build_cohort_sha256)
    {
        return Err("Blender task response envelope binding is invalid".to_owned());
    }
    if !response.ok || response.error.is_some() {
        if let Some(error) = &response.error {
            let value = json!({"code": error.code, "message": error.message});
            validate_blender_task_error_value_scrubbed(&value)?;
        }
        return Err("Blender task response is not a successful result".to_owned());
    }
    let result = response
        .result
        .as_ref()
        .ok_or_else(|| "Blender task response lacks a result".to_owned())?;
    validate_blender_task_exchange_against_authority(&request.payload, result, authority)
}

/// Validate one of the closed error codes from `BlenderTaskError@1`.
pub fn validate_blender_task_error_code(code: &str) -> Result<(), String> {
    if BLENDER_TASK_ERROR_CODES.contains(&code) {
        Ok(())
    } else {
        Err("BlenderTaskError code is not allowlisted".to_owned())
    }
}

pub fn is_blender_task_error_code(code: &str) -> bool {
    validate_blender_task_error_code(code).is_ok()
}

/// JSON-facing form of the error-code validator for callers that have not
/// deserialized the Worker envelope yet.
pub fn validate_blender_task_error_code_value(value: &Value) -> Result<(), String> {
    let code = value
        .as_str()
        .ok_or_else(|| "BlenderTaskError code must be a string".to_owned())?;
    validate_blender_task_error_code(code)
}

/// Validate the strict nested `BlenderTaskError@1` object as well as its code.
pub fn validate_blender_task_error_value(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &["code", "message"];
    let object = blender_require_object(value, FIELDS, "BlenderTaskError")?;
    validate_blender_task_error_code_value(blender_required(object, "code")?)?;
    let message = blender_required_string(object, "message")?;
    if message.is_empty() || message.len() > 512 {
        return Err("BlenderTaskError.message length is outside its bound".to_owned());
    }
    Ok(())
}

fn blender_safe_error_message(code: &str) -> Option<&'static str> {
    Some(match code {
        "CAPABILITY_UNAVAILABLE" => "capability_unavailable",
        "WORKER_PROTOCOL" => "protocol_rejected",
        "WORKER_TIMEOUT" => "timeout",
        "WORKER_CRASHED" => "crashed",
        "WORKER_RESOURCE_LIMIT" => "resource_limit",
        "WORKER_HASH_MISMATCH" => "hash_mismatch",
        "WORKER_READBACK_REJECTED" => "readback_rejected",
        "WORKER_DETERMINISM_MISMATCH" => "determinism_mismatch",
        "WORKER_COHORT_MISMATCH" => "cohort_mismatch",
        "WORKER_SANDBOX_VIOLATION" => "sandbox_violation",
        "WORKER_LICENSE_UNAVAILABLE" => "license_unavailable",
        "WORKER_PACKAGE_UNVERIFIED" => "package_unverified",
        _ => return None,
    })
}

/// Strict error validator for the Blender fixed-worker boundary. The public
/// BlenderTask@1 schema keeps a human-readable bounded string for compatibility,
/// but this adoption gate only accepts a code-specific machine token. This
/// prevents paths, URLs, usernames, commands, secrets, and control characters
/// from reaching Runtime logs or MCP output.
pub fn validate_blender_task_error_value_scrubbed(value: &Value) -> Result<(), String> {
    validate_blender_task_error_value(value)?;
    let object = value
        .as_object()
        .ok_or_else(|| "BlenderTaskError must be an object".to_owned())?;
    let code = blender_required_string(object, "code")?;
    let expected = blender_safe_error_message(code)
        .ok_or_else(|| "BlenderTaskError code has no safe machine message".to_owned())?;
    if blender_required_string(object, "message")? != expected {
        return Err("BlenderTaskError.message is not the fixed scrubbed token".to_owned());
    }
    Ok(())
}

fn blender_require_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(format!("{context} has missing or unknown fields"));
    }
    Ok(object)
}

fn blender_required<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a Value, String> {
    object
        .get(field)
        .ok_or_else(|| format!("Blender task field {field} is missing"))
}

fn blender_required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, String> {
    blender_required(object, field)?
        .as_str()
        .ok_or_else(|| format!("Blender task field {field} must be a string"))
}

fn blender_require_const_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), String> {
    if blender_required_string(object, field)? != expected {
        return Err(format!("Blender task field {field} has an invalid value"));
    }
    Ok(())
}

fn blender_validate_id(object: &serde_json::Map<String, Value>, field: &str) -> Result<(), String> {
    let value = blender_required_string(object, field)?;
    if !is_opaque_id(value) {
        return Err(format!("Blender task field {field} is an invalid id"));
    }
    Ok(())
}

fn blender_validate_sha256(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), String> {
    if !is_sha256(blender_required_string(object, field)?) {
        return Err(format!("Blender task field {field} is not a SHA-256"));
    }
    Ok(())
}

fn blender_required_u64(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<u64, String> {
    blender_required(object, field)?
        .as_u64()
        .ok_or_else(|| format!("Blender task budget {field} must be an integer"))
}

#[derive(Debug, Clone, Copy)]
struct BlenderValidatedBudgets {
    max_input_bytes: u64,
    max_output_bytes: u64,
}

fn blender_validate_budgets(value: &Value) -> Result<BlenderValidatedBudgets, String> {
    const FIELDS: &[&str] = &[
        "max_runtime_ms",
        "max_cpu_seconds",
        "max_memory_bytes",
        "max_gpu_bytes",
        "max_input_bytes",
        "max_output_bytes",
        "max_triangles",
        "max_texture_bytes",
        "max_stdout_bytes",
        "max_stderr_bytes",
    ];
    let object = blender_require_object(value, FIELDS, "BlenderTaskBudgets")?;
    blender_require_range(object, "max_runtime_ms", 1, BLENDER_RENDER_MAX_RUNTIME_MS)?;
    blender_require_range(object, "max_cpu_seconds", 1, BLENDER_RENDER_MAX_CPU_SECONDS)?;
    blender_require_range(
        object,
        "max_memory_bytes",
        1,
        BLENDER_WORKER_MAX_MEMORY_BYTES,
    )?;
    if blender_required_u64(object, "max_gpu_bytes")? != BLENDER_GPU_MAX_BYTES {
        return Err("BlenderTaskBudgets.max_gpu_bytes must be zero".to_owned());
    }
    let max_input_bytes =
        blender_require_range(object, "max_input_bytes", 1, BLENDER_ENVELOPE_MAX_BYTES)?;
    let max_output_bytes =
        blender_require_range(object, "max_output_bytes", 1, BLENDER_ENVELOPE_MAX_BYTES)?;
    blender_require_range(object, "max_triangles", 1, BLENDER_MAX_TRIANGLES)?;
    blender_require_range(object, "max_texture_bytes", 1, BLENDER_MAX_TEXTURE_BYTES)?;
    blender_require_range(object, "max_stdout_bytes", 1, BLENDER_ENVELOPE_MAX_BYTES)?;
    blender_require_range(object, "max_stderr_bytes", 1, BLENDER_STDERR_MAX_BYTES)?;
    Ok(BlenderValidatedBudgets {
        max_input_bytes,
        max_output_bytes,
    })
}

fn blender_require_range(
    object: &serde_json::Map<String, Value>,
    field: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let value = blender_required_u64(object, field)?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("Blender task budget {field} is outside its bound"));
    }
    Ok(value)
}

fn validate_blender_input_object(value: &Value, index: usize) -> Result<u64, String> {
    const FIELDS: &[&str] = &[
        "kind",
        "sha256",
        "canonical_sha256",
        "byte_size",
        "mime",
        "bytes_base64",
    ];
    let context = format!("BlenderTaskRequest.input_objects[{index}]");
    let object = blender_require_object(value, FIELDS, &context)?;
    let kind = blender_required_string(object, "kind")?;
    if !matches!(kind, "glb" | "reference_image" | "material_profile") {
        return Err(format!("{context}.kind is not allowlisted"));
    }
    let mime = blender_required_string(object, "mime")?;
    if !matches!(
        mime,
        "model/gltf-binary" | "image/png" | "image/jpeg" | "application/json"
    ) {
        return Err(format!("{context}.mime is not allowlisted"));
    }
    if !is_sha256(blender_required_string(object, "sha256")?)
        || !is_sha256(blender_required_string(object, "canonical_sha256")?)
    {
        return Err(format!("{context} contains an invalid SHA-256"));
    }
    let byte_size = blender_required_u64(object, "byte_size")?;
    if !(1..=BLENDER_ENVELOPE_MAX_BYTES).contains(&byte_size) {
        return Err(format!("{context}.byte_size is outside its bound"));
    }
    let encoded = blender_required_string(object, "bytes_base64")?;
    let bytes = blender_decode_base64(encoded, &context)?;
    if bytes.len() as u64 != byte_size {
        return Err(format!(
            "{context}.byte_size does not match transport bytes"
        ));
    }
    if blender_required_string(object, "sha256")? != hex_sha256(&bytes) {
        return Err(format!("{context}.sha256 does not match transport bytes"));
    }
    let expected_canonical = blender_canonical_without_field(value, "canonical_sha256")?;
    if blender_required_string(object, "canonical_sha256")? != expected_canonical {
        return Err(format!("{context}.canonical_sha256 does not match"));
    }
    Ok(byte_size)
}

fn validate_blender_output_object(value: &Value, index: usize) -> Result<u64, String> {
    const FIELDS: &[&str] = &[
        "kind",
        "mime",
        "byte_size",
        "sha256",
        "canonical_sha256",
        "lineage_sha256",
        "transport_bytes_base64",
        "cas_owner",
        "durability",
    ];
    let context = format!("BlenderTaskResult.outputs[{index}]");
    let object = blender_require_object(value, FIELDS, &context)?;
    let kind = blender_required_string(object, "kind")?;
    if !matches!(
        kind,
        "beauty"
            | "silhouette"
            | "depth"
            | "normal"
            | "ao"
            | "part-id"
            | "material-id"
            | "wireframe"
            | "uv-stretch"
    ) {
        return Err(format!("{context}.kind is not allowlisted"));
    }
    let mime = blender_required_string(object, "mime")?;
    if !matches!(mime, "image/png" | "model/gltf-binary" | "application/json") {
        return Err(format!("{context}.mime is not allowlisted"));
    }
    if !is_sha256(blender_required_string(object, "sha256")?)
        || !is_sha256(blender_required_string(object, "canonical_sha256")?)
        || !is_sha256(blender_required_string(object, "lineage_sha256")?)
    {
        return Err(format!("{context} contains an invalid SHA-256"));
    }
    let byte_size = blender_required_u64(object, "byte_size")?;
    if !(1..=BLENDER_ENVELOPE_MAX_BYTES).contains(&byte_size) {
        return Err(format!("{context}.byte_size is outside its bound"));
    }
    let encoded = blender_required_string(object, "transport_bytes_base64")?;
    let bytes = blender_decode_base64(encoded, &context)?;
    if bytes.len() as u64 != byte_size {
        return Err(format!(
            "{context}.byte_size does not match transport bytes"
        ));
    }
    if blender_required_string(object, "sha256")? != hex_sha256(&bytes) {
        return Err(format!("{context}.sha256 does not match transport bytes"));
    }
    if blender_required_string(object, "cas_owner")? != "runtime" {
        return Err(format!("{context}.cas_owner must be runtime"));
    }
    if blender_required_string(object, "durability")? != "pending_runtime_adoption" {
        return Err(format!("{context}.durability is not internal-pending"));
    }
    let expected_canonical = blender_canonical_without_field(value, "canonical_sha256")?;
    if blender_required_string(object, "canonical_sha256")? != expected_canonical {
        return Err(format!("{context}.canonical_sha256 does not match"));
    }
    Ok(byte_size)
}

fn validate_blender_task_checks(value: &Value) -> Result<(), String> {
    const FIELDS: &[&str] = &[
        "validator_status",
        "readback_status",
        "deterministic_replay_status",
        "stage_eligibility",
    ];
    let object = blender_require_object(value, FIELDS, "BlenderTaskResult.checks")?;
    for field in [
        "validator_status",
        "readback_status",
        "deterministic_replay_status",
    ] {
        let status = blender_required_string(object, field)?;
        if !matches!(status, "passed" | "failed" | "not-run") {
            return Err(format!("BlenderTaskResult.checks.{field} is invalid"));
        }
    }
    blender_require_const_string(object, "stage_eligibility", "non-promoting")
}

fn blender_canonical_without_field(value: &Value, field: &str) -> Result<String, String> {
    let mut preimage = value
        .as_object()
        .ok_or_else(|| "Blender canonical preimage must be an object".to_owned())?
        .clone();
    preimage.remove(field);
    Ok(canonical_hash(&Value::Object(preimage)))
}

fn blender_decode_base64(encoded: &str, context: &str) -> Result<Vec<u8>, String> {
    if encoded.is_empty() || encoded.len() > BLENDER_MAX_BASE64_CHARS || encoded.len() % 4 != 0 {
        return Err(format!("{context} base64 transport is outside its bound"));
    }
    let mut output = Vec::with_capacity(encoded.len() / 4 * 3);
    let bytes = encoded.as_bytes();
    for (chunk_index, chunk) in bytes.chunks_exact(4).enumerate() {
        let is_last = chunk_index + 1 == bytes.len() / 4;
        let first = blender_base64_digit(chunk[0])
            .ok_or_else(|| format!("{context} base64 transport has an invalid character"))?;
        let second = blender_base64_digit(chunk[1])
            .ok_or_else(|| format!("{context} base64 transport has an invalid character"))?;
        if chunk[2] == b'=' {
            if !is_last || chunk[3] != b'=' || second & 0x0f != 0 {
                return Err(format!("{context} base64 padding is invalid"));
            }
            output.push((first << 2) | (second >> 4));
            continue;
        }
        let third = blender_base64_digit(chunk[2])
            .ok_or_else(|| format!("{context} base64 transport has an invalid character"))?;
        output.push((first << 2) | (second >> 4));
        output.push((second << 4) | (third >> 2));
        if chunk[3] == b'=' {
            if !is_last || third & 0x03 != 0 {
                return Err(format!("{context} base64 padding is invalid"));
            }
            continue;
        }
        let fourth = blender_base64_digit(chunk[3])
            .ok_or_else(|| format!("{context} base64 transport has an invalid character"))?;
        output.push((third << 6) | fourth);
    }
    if output.is_empty() || output.len() as u64 > BLENDER_ENVELOPE_MAX_BYTES {
        return Err(format!(
            "{context} base64 transport is outside its byte bound"
        ));
    }
    Ok(output)
}

fn blender_base64_digit(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn validate_response(
    response: &WorkerResponse,
    expected_request_id: &str,
) -> Result<(), String> {
    if response.protocol != WORKER_PROTOCOL {
        return Err("worker response protocol is invalid".to_owned());
    }
    if response.request_id != expected_request_id || !is_opaque_id(&response.request_id) {
        return Err("worker response request_id is invalid".to_owned());
    }
    if response.ok {
        if response.result.is_none() || response.error.is_some() {
            return Err("successful worker response has an invalid result envelope".to_owned());
        }
    } else {
        let error = response
            .error
            .as_ref()
            .ok_or_else(|| "failed worker response lacks an error".to_owned())?;
        if response.result.is_some()
            || !is_opaque_id(&error.code)
            || error.message.is_empty()
            || error.message.len() > 512
        {
            return Err("failed worker response has an invalid error envelope".to_owned());
        }
    }
    if response
        .build_cohort_sha256
        .as_deref()
        .is_some_and(|value| !is_sha256(value))
    {
        return Err("worker response cohort is invalid".to_owned());
    }
    Ok(())
}

impl WorkerResponse {
    pub fn unavailable(request_id: String, _worker: &str) -> Self {
        Self {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id,
            build_cohort_sha256: build_cohort_sha256(),
            ok: false,
            result: None,
            error: Some(WorkerError {
                code: "CAPABILITY_UNAVAILABLE".to_owned(),
                message: blender_safe_error_message("CAPABILITY_UNAVAILABLE")
                    .expect("allowlisted Blender error")
                    .to_owned(),
            }),
        }
    }
}

fn is_opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Hash one JSON value using ForgeCAD's deterministic sorted-key canonical
/// encoding. Runtime producers use this same helper for Worker readback
/// digests; it is independent of serde's object insertion order.
pub fn canonical_json_sha256(value: &Value) -> String {
    canonical_hash(value)
}

/// Return the same deterministic bytes used by `canonical_json_sha256`.
/// Native High uses this opaque representation for transport byte accounting;
/// no internal mesh type is needed here.
pub fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_canonical(value, &mut bytes);
    bytes
}

fn canonical_hash(value: &Value) -> String {
    let digest = Sha256::digest(canonical_json_bytes(value));
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_canonical(value: &Value, output: &mut Vec<u8>) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => {
            serde_json::to_writer(&mut *output, value).expect("string serializes")
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_canonical(value, output);
            }
            output.push(b']');
        }
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            output.push(b'{');
            for (index, key) in keys.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key).expect("object key serializes");
                output.push(b':');
                write_canonical(&values[*key], output);
            }
            output.push(b'}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_hash_is_stable() {
        let catalog = operator_catalog();
        assert_eq!(catalog["canonical_sha256"], operator_catalog_sha256());
        assert_eq!(operator_catalog(), catalog);
    }

    #[test]
    fn material_pack_catalog_is_closed_hash_bound_and_contains_the_weapon_pack() {
        let catalog = material_pack_catalog();
        assert_eq!(catalog, material_pack_catalog());
        assert_eq!(catalog["schema_version"], "MaterialPackCatalog@1");
        assert_eq!(catalog["runtime_network"], false);
        assert_eq!(catalog["caller_paths"], false);
        let packs = catalog["packs"].as_array().expect("material pack list");
        assert_eq!(packs.len(), 3);
        assert_eq!(packs[0]["pack_id"], MATERIAL_PACK_ID);
        assert_eq!(
            packs[1]["pack_id"],
            FICTIONAL_ENERGY_WEAPON_MATERIAL_PACK_ID
        );
        assert_eq!(
            packs[1]["manifest_sha256"],
            material_pack_manifest_sha256_by_id(FICTIONAL_ENERGY_WEAPON_MATERIAL_PACK_ID)
                .expect("weapon manifest hash")
        );
        assert_eq!(
            packs[2]["pack_id"],
            FICTIONAL_ENERGY_WEAPON_2K_MATERIAL_PACK_ID
        );
        assert_eq!(
            packs[2]["manifest_sha256"],
            material_pack_manifest_sha256_by_id(FICTIONAL_ENERGY_WEAPON_2K_MATERIAL_PACK_ID)
                .expect("weapon 2K manifest hash")
        );
        assert!(material_pack_manifest_by_id("unknown-pack").is_none());
        let mut preimage = catalog.clone();
        preimage.as_object_mut().unwrap().remove("canonical_sha256");
        assert_eq!(catalog["canonical_sha256"], canonical_hash(&preimage));
    }

    #[test]
    fn render_profile_hashes_and_color_data_semantics_are_stable() {
        let profile = render_profile();
        assert_eq!(profile, render_profile());
        assert_eq!(profile["canonical_sha256"], render_profile_sha256());
        assert_eq!(profile["schema_version"], "RenderProfile@1");
        let aovs = profile["aovs"].as_array().expect("AOV array");
        assert_eq!(aovs.len(), 9);
        assert_eq!(aovs[0]["pass_id"], "beauty");
        assert_eq!(aovs[0]["color_transform"], "fixed-linear-to-srgb@1");
        assert!(aovs[1..].iter().all(|aov| aov["color_transform"] == "none"));
        assert_eq!(aovs[2]["encoding"], "reversed-normalized-depth-u8");
        assert_eq!(aovs[3]["semantic_kind"], "normal-vector");
        assert!(aovs[5]["palette_definition_sha256"].is_string());
        assert_eq!(
            aovs[5]["source_value_range"],
            "categorical-mesh-index-0-255"
        );
        assert_eq!(
            aovs[6]["source_value_range"],
            "categorical-material-index-0-255"
        );
        assert_eq!(
            aovs[5]["palette_definition_sha256"],
            profile["id_palette_definition_sha256"]
        );
        assert_eq!(
            aovs[6]["palette_definition_sha256"],
            profile["id_palette_definition_sha256"]
        );

        let mut aov_preimage = profile["aovs"].clone();
        assert_eq!(
            canonical_hash(&aov_preimage),
            profile["aov_definition_sha256"]
        );
        aov_preimage[2]["encoding"] = Value::String("tampered".to_owned());
        assert_ne!(
            canonical_hash(&aov_preimage),
            profile["aov_definition_sha256"]
        );
        assert_eq!(
            canonical_hash(&profile["color_pipeline"]),
            profile["color_pipeline_sha256"]
        );
    }

    #[test]
    fn strict_request_and_response_envelopes_reject_drift() {
        let request = WorkerRequest {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "request-1".to_owned(),
            operation: "compile_geometry".to_owned(),
            payload: json!({"geometry_program":{}}),
        };
        assert!(validate_request(&request).is_ok());
        let bloom_request = WorkerRequest {
            operation: "render_glb_vfx_bloom_frame".to_owned(),
            payload: json!({}),
            ..request.clone()
        };
        assert!(validate_request(&bloom_request).is_ok());
        let animated_particles_request = WorkerRequest {
            operation: RENDER_TYPED_ANIMATED_SOCKET_PARTICLES_OPERATION.to_owned(),
            payload: json!({}),
            ..request.clone()
        };
        assert!(validate_request(&animated_particles_request).is_ok());
        let native_high_request = WorkerRequest {
            operation: NATIVE_HIGH_WORKER_OPERATION.to_owned(),
            payload: json!({"schema_version":NATIVE_HIGH_PAYLOAD_SCHEMA_VERSION}),
            ..request.clone()
        };
        assert!(validate_request(&native_high_request).is_ok());
        for operation in [
            RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION,
            RENDER_TYPED_ANIMATED_SOCKET_TRAILS_BLOOM_OPERATION,
            PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION,
            PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION,
            PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION,
            PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION,
            PRODUCTION_WEAPON_CAGE_OFFSET_OPERATION,
            PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION,
            PRODUCTION_WEAPON_HERO_MATERIAL_OPERATION,
            PRODUCTION_WEAPON_HERO_UV_LAYOUT_OPERATION,
        ] {
            let request = WorkerRequest {
                operation: operation.to_owned(),
                ..request.clone()
            };
            assert!(validate_request(&request).is_ok());
        }
        let unknown_operation = WorkerRequest {
            operation: "render_typed_animated_socket_trails_v2".to_owned(),
            ..request.clone()
        };
        assert!(validate_request(&unknown_operation).is_err());
        let response = WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: request.request_id.clone(),
            build_cohort_sha256: None,
            ok: true,
            result: Some(json!({"schema_version":"GeometryWorkerResult@1"})),
            error: None,
        };
        assert!(validate_response(&response, &request.request_id).is_ok());
        let mut invalid = response;
        invalid.error = Some(WorkerError {
            code: "BAD".to_owned(),
            message: "drift".to_owned(),
        });
        assert!(validate_response(&invalid, "request-1").is_err());
    }

    fn native_high_request_fixture() -> NativeHighWorkerRequestEnvelope {
        let payload = json!({
            "schema_version":NATIVE_HIGH_PAYLOAD_SCHEMA_VERSION,
            "opaque_detail_graph":{"node_count":1}
        });
        NativeHighWorkerRequestEnvelope {
            schema_version: NATIVE_HIGH_REQUEST_ENVELOPE_SCHEMA_VERSION.to_owned(),
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "native-high-request-1".to_owned(),
            operation: NATIVE_HIGH_WORKER_OPERATION.to_owned(),
            build_cohort_sha256: Some("a".repeat(64)),
            payload_sha256: canonical_json_sha256(&payload),
            payload_bytes: canonical_json_bytes(&payload).len() as u64,
            payload,
            budget: NativeHighWorkerBudget {
                max_runtime_ms: 2_000,
                max_memory_bytes: 64 * 1024 * 1024,
                max_input_bytes: NATIVE_HIGH_MAX_PAYLOAD_BYTES as u64,
                max_output_bytes: 4 * 1024 * 1024,
            },
            timeout_ms: 1_000,
        }
    }

    #[test]
    fn native_high_request_is_closed_hash_bound_and_budgeted() {
        let request = native_high_request_fixture();
        assert!(validate_native_high_request(&request).is_ok());
        let bytes = serde_json::to_vec(&request).expect("request JSON");
        assert!(parse_native_high_request(&bytes).is_ok());

        let mut invalid_operation = request.clone();
        invalid_operation.operation = "forgecad.production.high-mesh-prepare@2".to_owned();
        assert!(validate_native_high_request(&invalid_operation).is_err());

        let mut invalid_marker = request.clone();
        invalid_marker.payload["schema_version"] = Value::String("Unknown@1".to_owned());
        assert!(validate_native_high_request(&invalid_marker).is_err());

        let mut invalid_hash = request.clone();
        invalid_hash.payload_sha256 = "b".repeat(64);
        assert!(validate_native_high_request(&invalid_hash).is_err());

        let mut invalid_timeout = request.clone();
        invalid_timeout.timeout_ms = invalid_timeout.budget.max_runtime_ms + 1;
        assert!(validate_native_high_request(&invalid_timeout).is_err());

        let mut unknown_field = serde_json::to_value(&request).expect("request value");
        unknown_field["unexpected"] = json!(true);
        assert!(parse_native_high_request(&serde_json::to_vec(&unknown_field).unwrap()).is_err());
    }

    #[test]
    fn native_high_response_binds_result_hash_cohort_and_timeout_errors() {
        let request = native_high_request_fixture();
        let result = json!({
            "schema_version":NATIVE_HIGH_RESULT_SCHEMA_VERSION,
            "opaque_artifact":{"triangle_count":1}
        });
        let response = NativeHighWorkerResponseEnvelope {
            schema_version: NATIVE_HIGH_RESPONSE_ENVELOPE_SCHEMA_VERSION.to_owned(),
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: request.request_id.clone(),
            operation: request.operation.clone(),
            build_cohort_sha256: request.build_cohort_sha256.clone(),
            ok: true,
            result_sha256: Some(canonical_json_sha256(&result)),
            result_bytes: Some(canonical_json_bytes(&result).len() as u64),
            result: Some(result),
            error: None,
        };
        assert!(validate_native_high_response(&response, &request).is_ok());

        let timeout = NativeHighWorkerResponseEnvelope {
            schema_version: NATIVE_HIGH_RESPONSE_ENVELOPE_SCHEMA_VERSION.to_owned(),
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: request.request_id.clone(),
            operation: request.operation.clone(),
            build_cohort_sha256: request.build_cohort_sha256.clone(),
            ok: false,
            result: None,
            result_sha256: None,
            result_bytes: None,
            error: Some(NativeHighWorkerError {
                code: "WORKER_TIMEOUT".to_owned(),
                message: "timeout".to_owned(),
            }),
        };
        assert!(validate_native_high_response(&timeout, &request).is_ok());

        let mut invalid_error = timeout.clone();
        invalid_error.error.as_mut().unwrap().code = "UNKNOWN".to_owned();
        assert!(validate_native_high_response(&invalid_error, &request).is_err());

        let mut invalid_cohort = response.clone();
        invalid_cohort.build_cohort_sha256 = Some("c".repeat(64));
        assert!(validate_native_high_response(&invalid_cohort, &request).is_err());
    }

    fn blender_hashed_request_fixture() -> Value {
        let bytes = [1_u8, 2, 3];
        let mut input = json!({
            "kind":"glb",
            "sha256":hex_sha256(&bytes),
            "canonical_sha256":"",
            "byte_size":bytes.len(),
            "mime":"model/gltf-binary",
            "bytes_base64":"AQID"
        });
        input["canonical_sha256"] =
            Value::String(blender_canonical_without_field(&input, "canonical_sha256").unwrap());
        let mut request = json!({
            "schema_version":BLENDER_TASK_REQUEST_SCHEMA_VERSION,
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "source_candidate_sha256":"a".repeat(64),
            "recipe_id":BLENDER_RENDER_FIXED_RECIPE_ID,
            "recipe_version":BLENDER_RENDER_FIXED_RECIPE_VERSION,
            "recipe_sha256":"b".repeat(64),
            "python_bundle_sha256":"c".repeat(64),
            "input_objects":[input],
            "camera_profile_sha256":"d".repeat(64),
            "material_profile_sha256":"e".repeat(64),
            "budgets":{
                "max_runtime_ms":1,
                "max_cpu_seconds":1,
                "max_memory_bytes":1,
                "max_gpu_bytes":0,
                "max_input_bytes":3,
                "max_output_bytes":1,
                "max_triangles":1,
                "max_texture_bytes":1,
                "max_stdout_bytes":1,
                "max_stderr_bytes":1
            },
            "network_policy":BLENDER_NETWORK_POLICY,
            "filesystem_policy":BLENDER_FILESYSTEM_POLICY,
            "script_policy":BLENDER_SCRIPT_POLICY,
            "output_policy":BLENDER_OUTPUT_POLICY,
            "canonical_sha256":""
        });
        request["canonical_sha256"] =
            Value::String(blender_canonical_without_field(&request, "canonical_sha256").unwrap());
        request
    }

    fn blender_hashed_result_fixture() -> Value {
        let bytes = [4_u8, 5, 6];
        let mut output = json!({
            "kind":"beauty",
            "mime":"image/png",
            "byte_size":bytes.len(),
            "sha256":hex_sha256(&bytes),
            "canonical_sha256":"",
            "lineage_sha256":"f".repeat(64),
            "transport_bytes_base64":"BAUG",
            "cas_owner":"runtime",
            "durability":"pending_runtime_adoption"
        });
        output["canonical_sha256"] =
            Value::String(blender_canonical_without_field(&output, "canonical_sha256").unwrap());
        let mut result = json!({
            "schema_version":BLENDER_TASK_RESULT_SCHEMA_VERSION,
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "recipe_sha256":"b".repeat(64),
            "python_bundle_sha256":"c".repeat(64),
            "build_cohort_sha256":"1".repeat(64),
            "input_canonical_sha256":"2".repeat(64),
            "outputs":[output],
            "checks":{
                "validator_status":"passed",
                "readback_status":"not-run",
                "deterministic_replay_status":"not-run",
                "stage_eligibility":"non-promoting"
            },
            "runtime_write":false,
            "worker_started":true,
            "stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "canonical_sha256":""
        });
        result["canonical_sha256"] =
            Value::String(blender_canonical_without_field(&result, "canonical_sha256").unwrap());
        result
    }

    fn blender_rehash(value: &mut Value) {
        value["canonical_sha256"] = Value::String(
            blender_canonical_without_field(value, "canonical_sha256").expect("canonical hash"),
        );
    }

    fn blender_hashed_exchange_fixture() -> (Value, Value) {
        let mut request = blender_hashed_request_fixture();
        request["budgets"]["max_output_bytes"] = Value::Number(3.into());
        blender_rehash(&mut request);

        let mut result = blender_hashed_result_fixture();
        result["input_canonical_sha256"] = request["canonical_sha256"].clone();
        result["checks"]["readback_status"] = Value::String("passed".to_owned());
        result["checks"]["deterministic_replay_status"] = Value::String("passed".to_owned());
        blender_rehash(&mut result);
        (request, result)
    }

    fn blender_authoritative_exchange_fixture() -> (Value, Value) {
        let mut request = blender_hashed_request_fixture();
        let material_bytes = [7_u8, 8, 9];
        let mut material = json!({
            "kind":"material_profile",
            "sha256":hex_sha256(&material_bytes),
            "canonical_sha256":"",
            "byte_size":material_bytes.len(),
            "mime":"application/json",
            "bytes_base64":"BwgJ"
        });
        material["canonical_sha256"] =
            Value::String(blender_canonical_without_field(&material, "canonical_sha256").unwrap());
        let source = request["input_objects"][0].clone();
        request["input_objects"] = json!([source, material]);
        request["budgets"]["max_input_bytes"] = Value::Number(6.into());
        request["budgets"]["max_output_bytes"] = Value::Number(3.into());
        blender_rehash(&mut request);

        let mut result = blender_hashed_result_fixture();
        result["input_canonical_sha256"] = request["canonical_sha256"].clone();
        result["checks"]["readback_status"] = Value::String("passed".to_owned());
        result["checks"]["deterministic_replay_status"] = Value::String("passed".to_owned());
        blender_rehash(&mut result);
        (request, result)
    }

    fn blender_authority_for<'a>(
        request: &'a Value,
        expected_outputs: &'a [BlenderTaskExpectedOutput<'a>],
    ) -> BlenderTaskExchangeAuthority<'a> {
        BlenderTaskExchangeAuthority {
            project_id: request["project_id"].as_str().expect("project id"),
            candidate_id: request["candidate_id"].as_str().expect("candidate id"),
            source_candidate_sha256: request["source_candidate_sha256"]
                .as_str()
                .expect("source candidate"),
            source_artifact_sha256: request["input_objects"][0]["sha256"]
                .as_str()
                .expect("source artifact"),
            source_artifact_canonical_sha256: request["input_objects"][0]["canonical_sha256"]
                .as_str()
                .expect("source artifact canonical"),
            source_artifact_readback_sha256: "f".repeat(64).leak(),
            source_artifact_readback_object_sha256: "0".repeat(64).leak(),
            camera_profile_sha256: request["camera_profile_sha256"]
                .as_str()
                .expect("camera profile"),
            camera_profile_object_sha256: "1".repeat(64).leak(),
            camera_profile_canonical_sha256: "2".repeat(64).leak(),
            material_profile_sha256: request["material_profile_sha256"]
                .as_str()
                .expect("material profile"),
            material_profile_object_sha256: request["input_objects"][1]["sha256"]
                .as_str()
                .expect("material object"),
            material_profile_canonical_sha256: request["input_objects"][1]["canonical_sha256"]
                .as_str()
                .expect("material canonical"),
            recipe_sha256: request["recipe_sha256"].as_str().expect("recipe"),
            python_bundle_sha256: request["python_bundle_sha256"].as_str().expect("python"),
            expected_build_cohort_sha256: "1".repeat(64).leak(),
            expected_outputs,
        }
    }

    #[test]
    fn blender_validators_accept_hashed_closed_fixtures() {
        let request = blender_hashed_request_fixture();
        assert!(validate_blender_task_request_value(&request).is_ok());
        let result = blender_hashed_result_fixture();
        assert!(validate_blender_task_result_value(&result).is_ok());
        for code in BLENDER_TASK_ERROR_CODES {
            assert!(validate_blender_task_error_code(code).is_ok());
            assert!(is_blender_task_error_code(code));
        }
        assert!(validate_blender_task_error_value(&json!({
            "code":"WORKER_READBACK_REJECTED",
            "message":"bounded readback rejected"
        }))
        .is_ok());
    }

    #[test]
    fn blender_validators_reject_unknown_fields_hash_drift_budget_and_write_flags() {
        let mut request = blender_hashed_request_fixture();
        request["dynamic"] = Value::Bool(true);
        assert!(validate_blender_task_request_value(&request).is_err());

        let mut request = blender_hashed_request_fixture();
        request["source_candidate_sha256"] = Value::String("0".repeat(64));
        assert!(validate_blender_task_request_value(&request).is_err());

        let mut request = blender_hashed_request_fixture();
        request["budgets"]["max_gpu_bytes"] = Value::Number(1.into());
        assert!(validate_blender_task_request_value(&request).is_err());

        let mut result = blender_hashed_result_fixture();
        result["runtime_write"] = Value::Bool(true);
        assert!(validate_blender_task_result_value(&result).is_err());

        let mut result = blender_hashed_result_fixture();
        result["outputs"][0]["cas_owner"] = Value::String("worker".to_owned());
        assert!(validate_blender_task_result_value(&result).is_err());

        assert!(validate_blender_task_error_code("WORKER_EXEC").is_err());
        assert!(validate_blender_task_error_code_value(&json!(3)).is_err());
        assert!(validate_blender_task_error_value(&json!({
            "code":"WORKER_TIMEOUT",
            "message":"ok",
            "path":"/tmp/secret"
        }))
        .is_err());
    }

    #[test]
    fn blender_exchange_binds_request_budget_identity_cohort_and_checks() {
        let (request, result) = blender_hashed_exchange_fixture();
        assert!(validate_blender_task_exchange(&request, &result, &"1".repeat(64)).is_ok());

        let mut retargeted = result.clone();
        retargeted["candidate_id"] = Value::String("candidate-2".to_owned());
        blender_rehash(&mut retargeted);
        assert!(
            validate_blender_task_exchange(&request, &retargeted, &"1".repeat(64))
                .unwrap_err()
                .contains("candidate_id differs")
        );

        let mut wrong_input = result.clone();
        wrong_input["input_canonical_sha256"] = Value::String("9".repeat(64));
        blender_rehash(&mut wrong_input);
        assert!(
            validate_blender_task_exchange(&request, &wrong_input, &"1".repeat(64))
                .unwrap_err()
                .contains("input canonical hash differs")
        );

        assert!(
            validate_blender_task_exchange(&request, &result, &"2".repeat(64))
                .unwrap_err()
                .contains("build cohort differs")
        );

        let mut over_budget_request = request.clone();
        over_budget_request["budgets"]["max_output_bytes"] = Value::Number(2.into());
        blender_rehash(&mut over_budget_request);
        let mut over_budget_result = result.clone();
        over_budget_result["input_canonical_sha256"] =
            over_budget_request["canonical_sha256"].clone();
        blender_rehash(&mut over_budget_result);
        assert!(validate_blender_task_exchange(
            &over_budget_request,
            &over_budget_result,
            &"1".repeat(64)
        )
        .unwrap_err()
        .contains("max_output_bytes"));

        let mut incomplete_checks = result.clone();
        incomplete_checks["checks"]["deterministic_replay_status"] =
            Value::String("not-run".to_owned());
        blender_rehash(&mut incomplete_checks);
        assert!(
            validate_blender_task_exchange(&request, &incomplete_checks, &"1".repeat(64))
                .unwrap_err()
                .contains("deterministic_replay_status=passed")
        );
    }

    #[test]
    fn blender_authority_envelope_binds_assets_outputs_lineage_and_headers() {
        let (request, mut result) = blender_authoritative_exchange_fixture();
        let expected_outputs = [BlenderTaskExpectedOutput {
            kind: "beauty",
            mime: "image/png",
            expected_byte_size: Some(3),
            max_byte_size: 3,
        }];
        let authority = blender_authority_for(&request, &expected_outputs);
        let lineage =
            blender_task_output_lineage_sha256(&request, &authority, &result["outputs"][0])
                .expect("lineage");
        result["outputs"][0]["lineage_sha256"] = Value::String(lineage);
        result["outputs"][0]["canonical_sha256"] = Value::String(
            blender_canonical_without_field(&result["outputs"][0], "canonical_sha256")
                .expect("output canonical"),
        );
        blender_rehash(&mut result);

        let request_envelope = WorkerRequest {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "blender-request-1".to_owned(),
            operation: BLENDER_RENDER_FIXED_OPERATION.to_owned(),
            payload: request.clone(),
        };
        let response_envelope = WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: request_envelope.request_id.clone(),
            build_cohort_sha256: Some("1".repeat(64)),
            ok: true,
            result: Some(result.clone()),
            error: None,
        };
        let valid_result = validate_blender_task_envelope_exchange(
            &request_envelope,
            &response_envelope,
            &authority,
        );
        assert!(valid_result.is_ok(), "{valid_result:?}");

        let mut bad_source_authority = authority;
        bad_source_authority.source_artifact_sha256 = Box::leak("9".repeat(64).into_boxed_str());
        assert!(validate_blender_task_envelope_exchange(
            &request_envelope,
            &response_envelope,
            &bad_source_authority
        )
        .unwrap_err()
        .contains("source GLB"));

        let mut bad_material_authority = authority;
        bad_material_authority.material_profile_object_sha256 =
            Box::leak("8".repeat(64).into_boxed_str());
        assert!(validate_blender_task_envelope_exchange(
            &request_envelope,
            &response_envelope,
            &bad_material_authority
        )
        .unwrap_err()
        .contains("material object"));

        let mut bad_material_canonical_authority = authority;
        bad_material_canonical_authority.material_profile_canonical_sha256 =
            Box::leak("9".repeat(64).into_boxed_str());
        assert!(validate_blender_task_envelope_exchange(
            &request_envelope,
            &response_envelope,
            &bad_material_canonical_authority
        )
        .unwrap_err()
        .contains("material canonical"));

        let mut bad_camera_request = request.clone();
        bad_camera_request["camera_profile_sha256"] = Value::String("9".repeat(64));
        blender_rehash(&mut bad_camera_request);
        let mut bad_camera_result = result.clone();
        bad_camera_result["input_canonical_sha256"] =
            bad_camera_request["canonical_sha256"].clone();
        blender_rehash(&mut bad_camera_result);
        let bad_camera_envelope = WorkerRequest {
            payload: bad_camera_request,
            ..request_envelope.clone()
        };
        let bad_camera_response = WorkerResponse {
            result: Some(bad_camera_result),
            ..response_envelope.clone()
        };
        assert!(validate_blender_task_envelope_exchange(
            &bad_camera_envelope,
            &bad_camera_response,
            &authority
        )
        .unwrap_err()
        .contains("camera_profile_sha256"));

        let mut bad_lineage_result = result.clone();
        bad_lineage_result["outputs"][0]["lineage_sha256"] = Value::String("0".repeat(64));
        bad_lineage_result["outputs"][0]["canonical_sha256"] = Value::String(
            blender_canonical_without_field(&bad_lineage_result["outputs"][0], "canonical_sha256")
                .expect("tampered output canonical"),
        );
        blender_rehash(&mut bad_lineage_result);
        let bad_lineage_response = WorkerResponse {
            result: Some(bad_lineage_result),
            ..response_envelope.clone()
        };
        assert!(validate_blender_task_envelope_exchange(
            &request_envelope,
            &bad_lineage_response,
            &authority
        )
        .unwrap_err()
        .contains("lineage"));

        let mut bad_operation = request_envelope.clone();
        bad_operation.operation = "render_glb".to_owned();
        assert!(validate_blender_task_envelope_exchange(
            &bad_operation,
            &response_envelope,
            &authority
        )
        .unwrap_err()
        .contains("operation"));

        let mut bad_outer_cohort = response_envelope.clone();
        bad_outer_cohort.build_cohort_sha256 = Some("2".repeat(64));
        assert!(validate_blender_task_envelope_exchange(
            &request_envelope,
            &bad_outer_cohort,
            &authority
        )
        .unwrap_err()
        .contains("response envelope"));
    }

    #[test]
    fn blender_error_scrub_rejects_paths_urls_secrets_commands_and_controls() {
        assert!(validate_blender_task_error_value_scrubbed(&json!({
            "code":"WORKER_READBACK_REJECTED",
            "message":"readback_rejected"
        }))
        .is_ok());
        for message in [
            "/Users/alice/secret.glb",
            "https://example.invalid/token",
            "token=super-secret",
            "blender --background --python /tmp/x.py",
            "line\nbreak",
            "\u{0000}",
            "bounded readback rejected",
        ] {
            assert!(
                validate_blender_task_error_value_scrubbed(&json!({
                    "code":"WORKER_READBACK_REJECTED",
                    "message":message
                }))
                .is_err(),
                "message must be scrubbed: {message:?}"
            );
        }
        let unavailable = WorkerResponse::unavailable("request-1".to_owned(), "/tmp/blender");
        let error = unavailable.error.expect("unavailable error");
        assert_eq!(error.message, "capability_unavailable");
        assert!(validate_blender_task_error_value_scrubbed(&json!({
            "code":error.code,
            "message":error.message
        }))
        .is_ok());
    }

    #[test]
    fn blender_operation_is_not_in_the_generic_worker_allowlist() {
        let request = WorkerRequest {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "request-blender".to_owned(),
            operation: BLENDER_RENDER_FIXED_OPERATION.to_owned(),
            payload: blender_hashed_request_fixture(),
        };
        assert!(validate_request(&request).is_err());
    }
}
