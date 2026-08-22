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
            | "render_glb_fit_batch"
            | "geometry_program_hash"
    ) {
        return Err("worker operation is not allowlisted".to_owned());
    }
    if !request.payload.is_object() {
        return Err("worker payload must be an object".to_owned());
    }
    Ok(())
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
    pub fn unavailable(request_id: String, worker: &str) -> Self {
        Self {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id,
            build_cohort_sha256: build_cohort_sha256(),
            ok: false,
            result: None,
            error: Some(WorkerError {
                code: "CAPABILITY_UNAVAILABLE".to_owned(),
                message: format!("{worker} worker is not enabled in MCP001"),
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

fn canonical_hash(value: &Value) -> String {
    let mut bytes = Vec::new();
    write_canonical(value, &mut bytes);
    let digest = Sha256::digest(bytes);
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
        for operation in [
            RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION,
            RENDER_TYPED_ANIMATED_SOCKET_TRAILS_BLOOM_OPERATION,
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
}
