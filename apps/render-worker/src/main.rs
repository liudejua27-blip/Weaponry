use base64::Engine;
use forgecad_render_core::{
    render_fixed_glb, render_perspective_glb, render_perspective_glb_fit_at_resolution,
    render_perspective_glb_with_emissive_overrides, render_perspective_glb_with_hdr_bloom,
    render_typed_particles_with_glb, render_typed_trails_bloom_with_glb,
    render_typed_trails_with_glb, EmissiveMaterialOverride, HdrBloomProfile, RenderPass,
    TypedParticle, TypedTrail, TypedTrailBloomProfile,
};
use forgecad_worker_protocol::{
    build_cohort_sha256, canonical_json_sha256, render_profile, validate_request, WorkerError,
    WorkerRequest, WorkerResponse, MAX_WORKER_REQUEST_BYTES, MAX_WORKER_RESPONSE_BYTES,
    RENDER_TYPED_ANIMATED_SOCKET_PARTICLES_OPERATION,
    RENDER_TYPED_ANIMATED_SOCKET_TRAILS_BLOOM_OPERATION,
    RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION, WORKER_PROTOCOL,
};
use serde_json::{json, Map, Value};
use std::io::{self, Read, Write};

const RENDER_OPERATIONS: &[&str] = &[
    "render_fixed",
    "render_glb",
    "render_glb_vfx_frame",
    "render_glb_vfx_bloom_frame",
    "render_typed_particles",
    RENDER_TYPED_ANIMATED_SOCKET_PARTICLES_OPERATION,
    RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION,
    RENDER_TYPED_ANIMATED_SOCKET_TRAILS_BLOOM_OPERATION,
    "render_typed_trails",
    "render_typed_trails_bloom",
    "render_glb_fit_batch",
];

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--build-identity"] {
        println!(
            "{}",
            serde_json::json!({
                "schema_version": "ForgeCADDevBuildIdentity@1",
                "component": "forgecad-render-worker",
                "build_cohort_sha256": build_cohort_sha256()
            })
        );
        return;
    }
    if args != ["--isolated-once"] {
        eprintln!("usage: forgecad-render-worker --isolated-once");
        std::process::exit(2);
    }
    std::process::exit(run_isolated_once());
}

/// Render is deliberately a one-request child. Runtime closes stdin after
/// writing one bounded request; reading to EOF makes a second JSONL request
/// impossible to sneak into the same process and keeps this lifecycle aligned
/// with the Geometry Worker isolation contract.
fn run_isolated_once() -> i32 {
    let request_bytes = match read_bounded_stdin() {
        Ok(bytes) => bytes,
        Err(message) => {
            let mut stdout = io::BufWriter::new(io::stdout());
            let _ = emit(
                &mut stdout,
                error_response("invalid-request", "WORKER_PROTOCOL", message),
            );
            return 1;
        }
    };
    let response = match serde_json::from_slice::<WorkerRequest>(&request_bytes) {
        Ok(request) => handle_request(request),
        Err(error) => WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "unknown".to_owned(),
            build_cohort_sha256: build_cohort_sha256(),
            ok: false,
            result: None,
            error: Some(forgecad_worker_protocol::WorkerError {
                code: "PARSE_ERROR".to_owned(),
                message: error.to_string(),
            }),
        },
    };
    let ok = response.ok;
    let mut stdout = io::BufWriter::new(io::stdout());
    if !emit(&mut stdout, response) {
        return 1;
    }
    if ok {
        0
    } else {
        1
    }
}

fn read_bounded_stdin() -> Result<Vec<u8>, String> {
    let mut input = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut stdin = io::stdin().lock();
    loop {
        let read = stdin
            .read(&mut buffer)
            .map_err(|error| format!("cannot read render request: {error}"))?;
        if read == 0 {
            break;
        }
        if input.len().saturating_add(read) > MAX_WORKER_REQUEST_BYTES {
            return Err("request exceeds the bounded render input".to_owned());
        }
        input.extend_from_slice(&buffer[..read]);
    }
    if input.is_empty() {
        return Err("render request is empty".to_owned());
    }
    Ok(input)
}

fn handle_request(request: WorkerRequest) -> WorkerResponse {
    let request_id = request.request_id.clone();
    if let Err(message) = validate_request(&request) {
        return error_response(&request_id, "WORKER_PROTOCOL", message);
    }
    if !RENDER_OPERATIONS.contains(&request.operation.as_str()) {
        return error_response(
            &request_id,
            "RENDER_WORKER_OPERATION_NOT_ALLOWED",
            "render worker accepts only render operations",
        );
    }
    match render_worker_result(&request) {
        Ok(result) => WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id,
            build_cohort_sha256: build_cohort_sha256(),
            ok: true,
            result: Some(result),
            error: None,
        },
        Err(error) => error_response(&request_id, "RENDER_REJECTED", error.to_string()),
    }
}

fn render_worker_result(request: &WorkerRequest) -> Result<Value, String> {
    let payload = request
        .payload
        .as_object()
        .ok_or_else(|| "payload is required".to_owned())?;
    match request.operation.as_str() {
        "render_fixed" => {
            require_closed_payload(payload, &["glb_base64"])?;
            let glb = decode_render_glb(payload)?;
            let passes = render_fixed_glb(&glb).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerResult@1",
                "passes":serialize_passes(&passes)
            }))
        }
        "render_glb" => {
            require_closed_payload(payload, &["glb_base64", "camera"])?;
            let glb = decode_render_glb(payload)?;
            let camera = payload
                .get("camera")
                .ok_or_else(|| "camera is required".to_owned())?;
            let passes = render_perspective_glb(&glb, camera).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerResult@2",
                "width":512,
                "height":512,
                "renderer_revision":"forgecad-renderer-2",
                "render_profile":render_profile(),
                "passes":serialize_passes(&passes)
            }))
        }
        "render_glb_vfx_frame" => {
            require_closed_payload(payload, &["glb_base64", "camera", "emissive_overrides"])?;
            let glb = decode_render_glb(payload)?;
            let camera = payload
                .get("camera")
                .ok_or_else(|| "camera is required".to_owned())?;
            let overrides = decode_emissive_overrides(payload.get("emissive_overrides"))?;
            let (passes, applied) =
                render_perspective_glb_with_emissive_overrides(&glb, camera, &overrides)
                    .map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerVfxFrameResult@1",
                "width":512,
                "height":512,
                "renderer_revision":"forgecad-renderer-2",
                "render_profile":render_profile(),
                "applied_emissive_overrides":applied.into_iter().map(|value| serde_json::json!({
                    "material_zone_id":value.material_zone_id,
                    "material_id":value.material_id,
                    "glb_material_index":value.glb_material_index
                })).collect::<Vec<_>>(),
                "passes":serialize_passes(&passes)
            }))
        }
        "render_glb_vfx_bloom_frame" => {
            require_closed_payload(
                payload,
                &[
                    "glb_base64",
                    "camera",
                    "emissive_overrides",
                    "bloom_profile",
                ],
            )?;
            let glb = decode_render_glb(payload)?;
            let camera = payload
                .get("camera")
                .ok_or_else(|| "camera is required".to_owned())?;
            let overrides = decode_emissive_overrides(payload.get("emissive_overrides"))?;
            let bloom_profile = decode_hdr_bloom_profile(payload.get("bloom_profile"))?;
            let (bloom_passes, applied) =
                render_perspective_glb_with_hdr_bloom(&glb, camera, &overrides, bloom_profile)
                    .map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerVfxBloomFrameResult@1",
                "width":512,
                "height":512,
                "renderer_revision":"forgecad-renderer-2",
                "render_profile":render_profile(),
                "bloom_profile":{
                    "threshold":bloom_profile.threshold,
                    "radius_px":bloom_profile.radius_px,
                    "intensity":bloom_profile.intensity,
                    "hdr_clamp":bloom_profile.hdr_clamp,
                    "blur_passes":HdrBloomProfile::BLUR_PASSES
                },
                "applied_emissive_overrides":applied.into_iter().map(|value| serde_json::json!({
                    "material_zone_id":value.material_zone_id,
                    "material_id":value.material_id,
                    "glb_material_index":value.glb_material_index
                })).collect::<Vec<_>>(),
                "bloom_passes":serialize_passes(&bloom_passes)
            }))
        }
        RENDER_TYPED_ANIMATED_SOCKET_PARTICLES_OPERATION => {
            render_typed_animated_socket_particles(payload)
        }
        RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION => {
            render_typed_animated_socket_trails(payload, false)
        }
        RENDER_TYPED_ANIMATED_SOCKET_TRAILS_BLOOM_OPERATION => {
            render_typed_animated_socket_trails(payload, true)
        }
        "render_typed_particles" => {
            require_closed_payload(
                payload,
                &["glb_base64", "camera", "particles", "seed_sha256"],
            )?;
            let glb = decode_render_glb(payload)?;
            let camera = payload
                .get("camera")
                .ok_or_else(|| "camera is required".to_owned())?;
            let seed_sha256 = payload
                .get("seed_sha256")
                .and_then(Value::as_str)
                .filter(|value| {
                    value.len() == 64
                        && value
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                })
                .ok_or_else(|| "seed_sha256 must be lowercase sha256".to_owned())?;
            let particles = decode_typed_particles(payload.get("particles"))?;
            let passes = render_typed_particles_with_glb(&glb, camera, &particles)
                .map_err(|error| error.to_string())?;
            let mut emitter_counts = serde_json::Map::new();
            for emitter in ["muzzle-burst", "energy-core-sparks"] {
                emitter_counts.insert(
                    emitter.to_owned(),
                    Value::from(
                        particles
                            .iter()
                            .filter(|particle| particle.emitter_id == emitter)
                            .count() as u64,
                    ),
                );
            }
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerVfxParticlesFrameResult@1",
                "width":512,
                "height":512,
                "renderer_revision":"forgecad-renderer-2",
                "render_profile":render_profile(),
                "seed_sha256":seed_sha256,
                "particle_count":particles.len(),
                "emitter_counts":emitter_counts,
                "particle_passes":serialize_passes(&passes)
            }))
        }
        "render_typed_trails" => {
            require_closed_payload(payload, &["glb_base64", "camera", "trails", "seed_sha256"])?;
            let glb = decode_render_glb(payload)?;
            let camera = payload
                .get("camera")
                .ok_or_else(|| "camera is required".to_owned())?;
            let seed_sha256 = payload
                .get("seed_sha256")
                .and_then(Value::as_str)
                .filter(|value| {
                    value.len() == 64
                        && value
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                })
                .ok_or_else(|| "seed_sha256 must be lowercase sha256".to_owned())?;
            let trails = decode_typed_trails(payload.get("trails"))?;
            let passes = render_typed_trails_with_glb(&glb, camera, &trails)
                .map_err(|error| error.to_string())?;
            let mut emitter_counts = serde_json::Map::new();
            for emitter in ["muzzle-trail", "energy-core-trail"] {
                emitter_counts.insert(
                    emitter.to_owned(),
                    Value::from(
                        trails
                            .iter()
                            .filter(|trail| trail.emitter_id == emitter)
                            .count() as u64,
                    ),
                );
            }
            let segment_count = trails
                .iter()
                .map(|trail| trail.points.len().saturating_sub(1))
                .sum::<usize>();
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerVfxTrailsFrameResult@1",
                "width":512,
                "height":512,
                "renderer_revision":"forgecad-renderer-2",
                "render_profile":render_profile(),
                "seed_sha256":seed_sha256,
                "trail_count":trails.len(),
                "segment_count":segment_count,
                "emitter_counts":emitter_counts,
                "trail_passes":serialize_passes(&passes)
            }))
        }
        "render_typed_trails_bloom" => {
            require_closed_payload(
                payload,
                &[
                    "glb_base64",
                    "camera",
                    "trails",
                    "trail_bloom_profile",
                    "seed_sha256",
                ],
            )?;
            let glb = decode_render_glb(payload)?;
            let camera = payload
                .get("camera")
                .ok_or_else(|| "camera is required".to_owned())?;
            let seed_sha256 = payload
                .get("seed_sha256")
                .and_then(Value::as_str)
                .filter(|value| {
                    value.len() == 64
                        && value
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                })
                .ok_or_else(|| "seed_sha256 must be lowercase sha256".to_owned())?;
            let trails = decode_typed_trails(payload.get("trails"))?;
            let profile = decode_typed_trail_bloom_profile(payload.get("trail_bloom_profile"))?;
            let passes = render_typed_trails_bloom_with_glb(&glb, camera, &trails, profile)
                .map_err(|error| error.to_string())?;
            let mut emitter_counts = serde_json::Map::new();
            for emitter in ["muzzle-trail", "energy-core-trail"] {
                emitter_counts.insert(
                    emitter.to_owned(),
                    Value::from(
                        trails
                            .iter()
                            .filter(|trail| trail.emitter_id == emitter)
                            .count() as u64,
                    ),
                );
            }
            let segment_count = trails
                .iter()
                .map(|trail| trail.points.len().saturating_sub(1))
                .sum::<usize>();
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerVfxTrailsBloomFrameResult@1",
                "width":512,
                "height":512,
                "renderer_revision":"forgecad-renderer-2",
                "render_profile":render_profile(),
                "trail_bloom_profile":{
                    "threshold":profile.threshold,
                    "radius_px":profile.radius_px,
                    "intensity":profile.intensity,
                    "hdr_clamp":profile.hdr_clamp,
                    "source_gain":profile.source_gain,
                    "blur_passes":TypedTrailBloomProfile::BLUR_PASSES
                },
                "seed_sha256":seed_sha256,
                "trail_count":trails.len(),
                "segment_count":segment_count,
                "emitter_counts":emitter_counts,
                "trail_bloom_passes":serialize_passes(&passes)
            }))
        }
        "render_glb_fit_batch" => {
            require_closed_payload(payload, &["glb_base64", "cameras", "resolution"])?;
            let glb = decode_render_glb(payload)?;
            let resolution = payload
                .get("resolution")
                .and_then(Value::as_u64)
                .filter(|value| matches!(*value, 128 | 256 | 512))
                .ok_or_else(|| "fit resolution must be 128, 256 or 512".to_owned())?
                as u32;
            let cameras = payload
                .get("cameras")
                .and_then(Value::as_array)
                .filter(|values| !values.is_empty() && values.len() <= 64)
                .ok_or_else(|| "fit cameras are outside the bounded range".to_owned())?;
            let mut renders = Vec::with_capacity(cameras.len());
            for (index, camera) in cameras.iter().enumerate() {
                let passes = render_perspective_glb_fit_at_resolution(&glb, camera, resolution)
                    .map_err(|error| error.to_string())?;
                renders.push(serde_json::json!({
                    "index":index,
                    "passes":serialize_passes(&passes)
                }));
            }
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerFitBatchResult@1",
                "width":resolution,
                "height":resolution,
                "renderer_revision":"forgecad-renderer-2",
                "renders":renders
            }))
        }
        _ => Err("render worker operation is not allowlisted".to_owned()),
    }
}

const ANIMATED_SOCKET_PARTICLE_SCHEMA: &str = "RenderWorkerAnimatedSocketParticlesFrameResult@1";
const ANIMATED_SOCKET_PARTICLE_WORLD_INVENTORY_SCHEMA: &str =
    "RenderWorkerAnimatedSocketParticleWorldInventory@1";
const ANIMATED_SOCKET_EMITTER_BINDING_SCHEMA: &str = "RenderWorkerAnimatedSocketEmitterBindings@1";
const ANIMATED_SOCKET_PARTICLE_COUNT: usize = 56;
const ANIMATED_SOCKET_MUZZLE_COUNT: usize = 24;
const ANIMATED_SOCKET_CORE_COUNT: usize = 32;

#[derive(Debug, Clone)]
struct AnimatedSocketEmitterBinding {
    emitter_id: String,
    socket_node_id: String,
    anchor_id: String,
    role: String,
    owner_part_id: String,
    translation_m: [f32; 3],
    rotation_quat_xyzw: [f32; 4],
    scale_xyz: [f32; 3],
}

fn render_typed_animated_socket_particles(payload: &Map<String, Value>) -> Result<Value, String> {
    require_closed_payload(
        payload,
        &[
            "glb_base64",
            "camera",
            "projection_key_sha256",
            "frame_index",
            "sample_time_ticks",
            "projection_input_sha256",
            "projection_socket_transform_inventory_sha256",
            "projection_socket_transform_readback_sha256",
            "emitter_bindings",
            "particles",
            "seed_sha256",
        ],
    )?;
    let glb = decode_render_glb(payload)?;
    let camera = payload
        .get("camera")
        .ok_or_else(|| "camera is required".to_owned())?;
    let projection_key_sha256 = required_sha256(
        payload.get("projection_key_sha256"),
        "projection_key_sha256",
    )?;
    let frame_index = payload
        .get("frame_index")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 15)
        .ok_or_else(|| "frame_index must be in the bounded 0..15 range".to_owned())?;
    let sample_time_ticks = payload
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| "sample_time_ticks is outside the bounded range".to_owned())?;
    let projection_input_sha256 = required_sha256(
        payload.get("projection_input_sha256"),
        "projection_input_sha256",
    )?;
    let projection_socket_transform_inventory_sha256 = required_sha256(
        payload.get("projection_socket_transform_inventory_sha256"),
        "projection_socket_transform_inventory_sha256",
    )?;
    let projection_socket_transform_readback_sha256 = required_sha256(
        payload.get("projection_socket_transform_readback_sha256"),
        "projection_socket_transform_readback_sha256",
    )?;
    let seed_sha256 = required_sha256(payload.get("seed_sha256"), "seed_sha256")?;
    let emitters = decode_animated_socket_emitters(payload.get("emitter_bindings"))?;
    let (particles, world_values) =
        decode_animated_socket_particles(payload.get("particles"), &emitters, camera)?;
    let passes = render_typed_particles_with_glb(&glb, camera, &particles)
        .map_err(|error| error.to_string())?;
    let emitter_binding_value = json!({
        "schema_version":ANIMATED_SOCKET_EMITTER_BINDING_SCHEMA,
        "emitters":emitters.iter().map(animated_socket_emitter_value).collect::<Vec<_>>()
    });
    let emitter_binding_sha256 = canonical_json_sha256(&emitter_binding_value);
    let expected_seed_sha256 = animated_socket_particle_seed_sha256(
        &projection_key_sha256,
        frame_index,
        sample_time_ticks,
        &projection_input_sha256,
        &projection_socket_transform_inventory_sha256,
        &projection_socket_transform_readback_sha256,
        &emitter_binding_sha256,
        &world_values,
    );
    if seed_sha256 != expected_seed_sha256 {
        return Err(
            "seed_sha256 does not bind the projection, emitter bindings and local particle inventory"
                .to_owned(),
        );
    }
    let world_inventory = json!({
        "schema_version":ANIMATED_SOCKET_PARTICLE_WORLD_INVENTORY_SCHEMA,
        "projection_key_sha256":projection_key_sha256,
        "frame_index":frame_index,
        "sample_time_ticks":sample_time_ticks,
        "seed_sha256":seed_sha256,
        "particle_count":particles.len(),
        "particles":world_values,
        "canonical_sha256":""
    });
    // Hash the exact JSON number representation that crosses stdout and is
    // independently revalidated by Runtime. In-memory `f32` Numbers can have
    // a different serde representation after transport even when their
    // numeric value is unchanged.
    let world_inventory_bytes = serde_json::to_vec(&world_inventory)
        .map_err(|_| "animated socket world inventory serialization failed".to_owned())?;
    let mut world_inventory: Value = serde_json::from_slice(&world_inventory_bytes)
        .map_err(|_| "animated socket world inventory normalization failed".to_owned())?;
    let mut world_inventory_preimage = world_inventory
        .as_object()
        .expect("animated socket world inventory is an object")
        .clone();
    world_inventory_preimage.remove("canonical_sha256");
    let world_particle_inventory_sha256 =
        canonical_json_sha256(&Value::Object(world_inventory_preimage));
    world_inventory["canonical_sha256"] = Value::String(world_particle_inventory_sha256.clone());
    let mut emitter_counts = serde_json::Map::new();
    emitter_counts.insert(
        "muzzle-burst".to_owned(),
        Value::from(ANIMATED_SOCKET_MUZZLE_COUNT as u64),
    );
    emitter_counts.insert(
        "energy-core-sparks".to_owned(),
        Value::from(ANIMATED_SOCKET_CORE_COUNT as u64),
    );
    Ok(json!({
        "schema_version":ANIMATED_SOCKET_PARTICLE_SCHEMA,
        "width":512,
        "height":512,
        "renderer_revision":"forgecad-renderer-2",
        "render_profile":render_profile(),
        "projection_key_sha256":projection_key_sha256,
        "frame_index":frame_index,
        "sample_time_ticks":sample_time_ticks,
        "projection_input_sha256":projection_input_sha256,
        "projection_socket_transform_inventory_sha256":projection_socket_transform_inventory_sha256,
        "projection_socket_transform_readback_sha256":projection_socket_transform_readback_sha256,
        "seed_sha256":seed_sha256,
        "emitter_binding_sha256":emitter_binding_sha256,
        "world_particle_inventory_sha256":world_particle_inventory_sha256,
        "world_particle_inventory":world_inventory,
        "particle_count":particles.len(),
        "emitter_counts":emitter_counts,
        "particle_passes":serialize_passes(&passes)
    }))
}

const ANIMATED_SOCKET_TRAIL_SCHEMA: &str = "RenderWorkerAnimatedSocketTrailsFrameResult@1";
const ANIMATED_SOCKET_TRAIL_BLOOM_SCHEMA: &str =
    "RenderWorkerAnimatedSocketTrailsBloomFrameResult@1";
const ANIMATED_SOCKET_TRAIL_SAMPLE_SET_SCHEMA: &str =
    "RenderWorkerAnimatedSocketTrailProjectionSamples@1";
const ANIMATED_SOCKET_TRAIL_EMITTER_SCHEMA: &str =
    "RenderWorkerAnimatedSocketTrailEmitterBindings@1";
const ANIMATED_SOCKET_TRAIL_INVENTORY_SCHEMA: &str = "RenderWorkerAnimatedSocketTrailInventory@1";
const ANIMATED_SOCKET_TRAIL_SEED_SCHEMA: &str = "RenderWorkerAnimatedSocketTrailSeed@1";

#[derive(Debug, Clone)]
struct AnimatedSocketTrailSample {
    frame_index: u64,
    sample_time_ticks: u64,
    projection_frame_canonical_sha256: String,
    projection_socket_transform_inventory_sha256: String,
    projection_socket_transform_readback_sha256: String,
    emitters: [AnimatedSocketEmitterBinding; 2],
}

#[derive(Debug, Clone)]
struct AnimatedSocketTrailPoint {
    frame_index: u64,
    sample_time_ticks: u64,
    source_particle_key_sha256: String,
    source_particle_id: u32,
    local_offset_m: [f32; 3],
}

#[derive(Debug, Clone)]
struct AnimatedSocketTrailDefinition {
    emitter_id: String,
    id: u32,
    points: Vec<AnimatedSocketTrailPoint>,
    radius_px: f32,
    color_linear_rgb: [f32; 3],
    alpha: f32,
    lifetime_ticks: u64,
}

struct AnimatedSocketTrailInput {
    projection_key_sha256: String,
    current_frame_index: u64,
    current_sample_time_ticks: u64,
    projection_input_sha256: String,
    trails: Vec<AnimatedSocketTrailDefinition>,
    typed_trails: Vec<TypedTrail>,
    projection_sample_set_sha256: String,
    emitter_binding_sha256: String,
    trail_inventory_sha256: String,
    trail_inventory: Value,
    emitter_counts: Map<String, Value>,
    segment_count: usize,
}

/// The animated-socket trail operations deliberately share one decoder.  A
/// Bloom request can therefore not silently use a different transform,
/// history window, or seed projection from its non-Bloom sibling.
fn render_typed_animated_socket_trails(
    payload: &Map<String, Value>,
    bloom: bool,
) -> Result<Value, String> {
    let mut fields = vec![
        "glb_base64",
        "camera",
        "projection_key_sha256",
        "current_frame_index",
        "current_sample_time_ticks",
        "projection_input_sha256",
        "projection_samples",
        "trails",
        "seed_sha256",
    ];
    if bloom {
        fields.push("trail_bloom_profile");
    }
    require_closed_payload(payload, &fields)?;
    let glb = decode_render_glb(payload)?;
    let camera = payload
        .get("camera")
        .ok_or_else(|| "camera is required".to_owned())?;
    // Parse and fully validate all projection/trail data before invoking the
    // renderer.  This keeps malformed history and retargeted bindings from
    // causing even a transient render side effect in future adapters.
    let input = decode_animated_socket_trail_input(payload, camera)?;
    let seed_sha256 = required_sha256(payload.get("seed_sha256"), "seed_sha256")?;
    let expected_seed_sha256 = animated_socket_trail_seed_sha256(&input);
    if seed_sha256 != expected_seed_sha256 {
        return Err(
            "seed_sha256 does not bind projection samples, emitters and local trail inventory"
                .to_owned(),
        );
    }
    let profile = if bloom {
        Some(decode_typed_trail_bloom_profile(
            payload.get("trail_bloom_profile"),
        )?)
    } else {
        None
    };
    let passes = if let Some(profile) = profile {
        render_typed_trails_bloom_with_glb(&glb, camera, &input.typed_trails, profile)
            .map_err(|error| error.to_string())?
    } else {
        render_typed_trails_with_glb(&glb, camera, &input.typed_trails)
            .map_err(|error| error.to_string())?
    };
    let mut inventory = input.trail_inventory;
    inventory["seed_sha256"] = Value::String(seed_sha256.clone());
    let mut inventory_preimage = inventory
        .as_object()
        .expect("animated trail inventory is an object")
        .clone();
    inventory_preimage.remove("canonical_sha256");
    inventory_preimage.remove("seed_sha256");
    let inventory_sha256 = canonical_json_sha256(&Value::Object(inventory_preimage));
    if inventory_sha256 != input.trail_inventory_sha256 {
        return Err("animated trail inventory hash is not stable".to_owned());
    }
    inventory["canonical_sha256"] = Value::String(inventory_sha256);

    let mut output = json!({
        "schema_version":if bloom { ANIMATED_SOCKET_TRAIL_BLOOM_SCHEMA } else { ANIMATED_SOCKET_TRAIL_SCHEMA },
        "width":512,
        "height":512,
        "renderer_revision":"forgecad-renderer-2",
        "render_profile":render_profile(),
        "projection_key_sha256":input.projection_key_sha256,
        "current_frame_index":input.current_frame_index,
        "current_sample_time_ticks":input.current_sample_time_ticks,
        "projection_input_sha256":input.projection_input_sha256,
        "projection_sample_set_sha256":input.projection_sample_set_sha256,
        "emitter_binding_sha256":input.emitter_binding_sha256,
        "trail_inventory_sha256":input.trail_inventory_sha256,
        "trail_inventory":inventory,
        "seed_sha256":seed_sha256,
        "trail_count":input.trails.len(),
        "segment_count":input.segment_count,
        "emitter_counts":input.emitter_counts,
    });
    if bloom {
        let profile = profile.expect("validated trail Bloom profile");
        output["trail_bloom_profile"] = json!({
            "threshold":profile.threshold,
            "radius_px":profile.radius_px,
            "intensity":profile.intensity,
            "hdr_clamp":profile.hdr_clamp,
            "source_gain":profile.source_gain,
            "blur_passes":TypedTrailBloomProfile::BLUR_PASSES
        });
        output["trail_bloom_passes"] = Value::Array(serialize_passes(&passes));
    } else {
        output["trail_passes"] = Value::Array(serialize_passes(&passes));
    }
    Ok(output)
}

fn decode_animated_socket_trail_input(
    payload: &Map<String, Value>,
    camera: &Value,
) -> Result<AnimatedSocketTrailInput, String> {
    let projection_key_sha256 = required_sha256(
        payload.get("projection_key_sha256"),
        "projection_key_sha256",
    )?;
    let current_frame_index = payload
        .get("current_frame_index")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 15)
        .ok_or_else(|| "current_frame_index is outside 0..15".to_owned())?;
    let current_sample_time_ticks = payload
        .get("current_sample_time_ticks")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| "current_sample_time_ticks is outside the bounded range".to_owned())?;
    let projection_input_sha256 = required_sha256(
        payload.get("projection_input_sha256"),
        "projection_input_sha256",
    )?;
    let sample_values = payload
        .get("projection_samples")
        .and_then(Value::as_array)
        .filter(|values| (2..=9).contains(&values.len()))
        .ok_or_else(|| "projection_samples must contain 2..9 samples".to_owned())?;
    let mut samples = Vec::with_capacity(sample_values.len());
    let mut previous_frame = None;
    let mut previous_ticks = None;
    for (index, value) in sample_values.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| "projection sample must be an object".to_owned())?;
        require_closed_payload(
            object,
            &[
                "frame_index",
                "sample_time_ticks",
                "projection_frame_canonical_sha256",
                "projection_socket_transform_inventory_sha256",
                "projection_socket_transform_readback_sha256",
                "emitters",
            ],
        )?;
        if object.len() != 6 {
            return Err("projection sample is missing a required field".to_owned());
        }
        let frame_index = object
            .get("frame_index")
            .and_then(Value::as_u64)
            .filter(|value| *value <= 15)
            .ok_or_else(|| "projection sample frame_index is invalid".to_owned())?;
        let sample_time_ticks = object
            .get("sample_time_ticks")
            .and_then(Value::as_u64)
            .filter(|value| *value <= 1_000_000)
            .ok_or_else(|| "projection sample tick is invalid".to_owned())?;
        if let Some(previous) = previous_frame {
            if frame_index <= previous {
                return Err(
                    "projection sample frame indices must be strictly increasing".to_owned(),
                );
            }
        }
        if let Some(previous) = previous_ticks {
            if sample_time_ticks <= previous {
                return Err("projection sample ticks must be strictly increasing".to_owned());
            }
        }
        previous_frame = Some(frame_index);
        previous_ticks = Some(sample_time_ticks);
        let projection_frame_canonical_sha256 = required_sha256(
            object.get("projection_frame_canonical_sha256"),
            "projection_frame_canonical_sha256",
        )?;
        let projection_socket_transform_inventory_sha256 = required_sha256(
            object.get("projection_socket_transform_inventory_sha256"),
            "projection_socket_transform_inventory_sha256",
        )?;
        let projection_socket_transform_readback_sha256 = required_sha256(
            object.get("projection_socket_transform_readback_sha256"),
            "projection_socket_transform_readback_sha256",
        )?;
        let emitters = decode_animated_socket_trail_emitters(object.get("emitters"))?;
        if index + 1 == sample_values.len()
            && (frame_index != current_frame_index
                || sample_time_ticks != current_sample_time_ticks)
        {
            return Err("current projection sample does not match current frame".to_owned());
        }
        samples.push(AnimatedSocketTrailSample {
            frame_index,
            sample_time_ticks,
            projection_frame_canonical_sha256,
            projection_socket_transform_inventory_sha256,
            projection_socket_transform_readback_sha256,
            emitters,
        });
    }
    let projection_sample_values = samples
        .iter()
        .map(|sample| {
            json!({
                "frame_index":sample.frame_index,
                "sample_time_ticks":sample.sample_time_ticks,
                "projection_frame_canonical_sha256":sample.projection_frame_canonical_sha256,
                "projection_socket_transform_inventory_sha256":sample.projection_socket_transform_inventory_sha256,
                "projection_socket_transform_readback_sha256":sample.projection_socket_transform_readback_sha256,
                "emitter_binding":sample.emitters.iter().map(animated_socket_emitter_value).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let emitter_binding_value = json!({
        "schema_version":ANIMATED_SOCKET_TRAIL_EMITTER_SCHEMA,
        "projection_key_sha256":projection_key_sha256,
        "samples":samples.iter().map(|sample| json!({
            "frame_index":sample.frame_index,
            "sample_time_ticks":sample.sample_time_ticks,
            "emitters":sample.emitters.iter().map(animated_socket_emitter_value).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    });
    let emitter_binding_sha256 = canonical_json_sha256(&emitter_binding_value);
    let projection_sample_set_sha256 = canonical_json_sha256(&json!({
        "schema_version":ANIMATED_SOCKET_TRAIL_SAMPLE_SET_SCHEMA,
        "projection_key_sha256":projection_key_sha256,
        "current_frame_index":current_frame_index,
        "current_sample_time_ticks":current_sample_time_ticks,
        "samples":projection_sample_values
    }));

    let trail_values = payload
        .get("trails")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or_else(|| "trails must contain exactly two fixed trails".to_owned())?;
    let mut trails = Vec::with_capacity(2);
    for (index, value) in trail_values.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| "animated socket trail must be an object".to_owned())?;
        require_closed_payload(
            object,
            &[
                "emitter_id",
                "id",
                "local_points",
                "radius_px",
                "color_linear_rgb",
                "alpha",
                "lifetime_ticks",
            ],
        )?;
        if object.len() != 7 {
            return Err("animated socket trail is missing a required field".to_owned());
        }
        let (expected_emitter, expected_id) = if index == 0 {
            ("muzzle-trail", 30_000_u32)
        } else {
            ("energy-core-trail", 31_000_u32)
        };
        let emitter_id = bounded_identifier(object.get("emitter_id"), "emitter_id")?;
        let id = object
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value == expected_id)
            .ok_or_else(|| "animated socket trail id is outside the fixed encoding".to_owned())?;
        if emitter_id != expected_emitter {
            return Err("animated socket trail emitter order differs".to_owned());
        }
        let local_values = object
            .get("local_points")
            .and_then(Value::as_array)
            .filter(|values| values.len() == samples.len())
            .ok_or_else(|| "local_points must match projection sample count".to_owned())?;
        let mut points = Vec::with_capacity(local_values.len());
        for (sample_index, point_value) in local_values.iter().enumerate() {
            let point = point_value
                .as_object()
                .ok_or_else(|| "local trail point must be an object".to_owned())?;
            require_closed_payload(
                point,
                &[
                    "frame_index",
                    "sample_time_ticks",
                    "source_particle_key_sha256",
                    "source_particle_id",
                    "local_offset_m",
                ],
            )?;
            if point.len() != 5 {
                return Err("local trail point is missing a required field".to_owned());
            }
            let sample = &samples[sample_index];
            let frame_index = point
                .get("frame_index")
                .and_then(Value::as_u64)
                .filter(|value| *value == sample.frame_index)
                .ok_or_else(|| "trail point frame does not match projection sample".to_owned())?;
            let sample_time_ticks = point
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                .filter(|value| *value == sample.sample_time_ticks)
                .ok_or_else(|| "trail point tick does not match projection sample".to_owned())?;
            let source_particle_key_sha256 = required_sha256(
                point.get("source_particle_key_sha256"),
                "source_particle_key_sha256",
            )?;
            let source_particle_id = point
                .get("source_particle_id")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value == if index == 0 { 10_000 } else { 20_000 })
                .ok_or_else(|| "source_particle_id is outside the fixed encoding".to_owned())?;
            let local =
                decode_f32_array(point.get("local_offset_m"), 3, 10.0, "local trail offset")?;
            let local_offset_m = [local[0], local[1], local[2]];
            let position =
                animated_socket_transform_point(&sample.emitters[index], local_offset_m)?;
            let _ = animated_socket_camera_depth(camera, position)?;
            points.push(AnimatedSocketTrailPoint {
                frame_index,
                sample_time_ticks,
                source_particle_key_sha256,
                source_particle_id,
                local_offset_m,
            });
        }
        let radius_px = object
            .get("radius_px")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (1.0..=8.0).contains(value))
            .map(|value| value as f32)
            .ok_or_else(|| "animated socket trail radius is invalid".to_owned())?;
        let color_values = decode_f32_array(object.get("color_linear_rgb"), 3, 1.0, "trail color")?;
        let color_linear_rgb = [color_values[0], color_values[1], color_values[2]];
        let alpha = object
            .get("alpha")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .map(|value| value as f32)
            .ok_or_else(|| "animated socket trail alpha is invalid".to_owned())?;
        let lifetime_ticks = object
            .get("lifetime_ticks")
            .and_then(Value::as_u64)
            .filter(|value| (1..=1_000_000).contains(value))
            .ok_or_else(|| "animated socket trail lifetime is invalid".to_owned())?;
        trails.push(AnimatedSocketTrailDefinition {
            emitter_id,
            id,
            points,
            radius_px,
            color_linear_rgb,
            alpha,
            lifetime_ticks,
        });
    }
    let mut typed_trails = Vec::with_capacity(2);
    let mut inventory_trails = Vec::with_capacity(2);
    for (index, trail) in trails.iter().enumerate() {
        let mut world_points = Vec::with_capacity(trail.points.len());
        for point in &trail.points {
            let position = animated_socket_transform_point(
                &samples
                    .iter()
                    .find(|sample| sample.frame_index == point.frame_index)
                    .ok_or_else(|| "trail point sample is missing".to_owned())?
                    .emitters[index],
                point.local_offset_m,
            )?;
            let depth = animated_socket_camera_depth(camera, position)?;
            world_points.push(position);
            // Keep the source/local/world mapping in the hash inventory.  It
            // is intentionally not reduced to a caller-provided world point.
            let _ = depth;
        }
        typed_trails.push(TypedTrail {
            emitter_id: trail.emitter_id.clone(),
            id: trail.id,
            points: world_points.clone(),
            radius_px: trail.radius_px,
            color_linear_rgb: trail.color_linear_rgb,
            alpha: trail.alpha,
            lifetime_ticks: trail.lifetime_ticks,
        });
        inventory_trails.push(json!({
            "emitter_id":trail.emitter_id,
            "id":trail.id,
            "radius_px":trail.radius_px,
            "color_linear_rgb":trail.color_linear_rgb,
            "alpha":trail.alpha,
            "lifetime_ticks":trail.lifetime_ticks,
            "points":trail.points.iter().zip(world_points).map(|(point, position)| {
                let sample = samples.iter().find(|sample| sample.frame_index == point.frame_index).expect("validated sample");
                let depth = animated_socket_camera_depth(camera, position).expect("validated depth");
                json!({
                    "frame_index":point.frame_index,
                    "sample_time_ticks":point.sample_time_ticks,
                    "source_particle_key_sha256":point.source_particle_key_sha256,
                    "source_particle_id":point.source_particle_id,
                    "local_offset_m":point.local_offset_m,
                    "world_position_m":position,
                    "camera_depth":depth,
                    "projection_frame_canonical_sha256":sample.projection_frame_canonical_sha256
                })
            }).collect::<Vec<_>>()
        }));
    }
    let inventory = json!({
        "schema_version":ANIMATED_SOCKET_TRAIL_INVENTORY_SCHEMA,
        "projection_key_sha256":projection_key_sha256,
        "current_frame_index":current_frame_index,
        "current_sample_time_ticks":current_sample_time_ticks,
        "sample_count":samples.len(),
        "seed_sha256":"",
        "trails":inventory_trails,
        "canonical_sha256":""
    });
    let mut inventory_preimage = inventory
        .as_object()
        .expect("animated trail inventory is an object")
        .clone();
    inventory_preimage.remove("canonical_sha256");
    inventory_preimage.remove("seed_sha256");
    let trail_inventory_sha256 = canonical_json_sha256(&Value::Object(inventory_preimage));
    let segment_count = trails
        .iter()
        .map(|trail| trail.points.len().saturating_sub(1))
        .sum::<usize>();
    if segment_count == 0 || segment_count > 16 {
        return Err("animated trail segment count is outside 1..16".to_owned());
    }
    let mut emitter_counts = Map::new();
    emitter_counts.insert("muzzle-trail".to_owned(), Value::from(1_u64));
    emitter_counts.insert("energy-core-trail".to_owned(), Value::from(1_u64));
    Ok(AnimatedSocketTrailInput {
        projection_key_sha256,
        current_frame_index,
        current_sample_time_ticks,
        projection_input_sha256,
        trails,
        typed_trails,
        projection_sample_set_sha256,
        emitter_binding_sha256,
        trail_inventory_sha256,
        trail_inventory: inventory,
        emitter_counts,
        segment_count,
    })
}

fn decode_animated_socket_trail_emitters(
    value: Option<&Value>,
) -> Result<[AnimatedSocketEmitterBinding; 2], String> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or_else(|| "projection sample must contain exactly two emitters".to_owned())?;
    let expected = [
        (
            "muzzle-trail",
            "socket-muzzle-vfx",
            "muzzle-vfx",
            "barrel-assembly",
        ),
        (
            "energy-core-trail",
            "socket-energy-core-vfx",
            "energy-core-vfx",
            "energy-core",
        ),
    ];
    let mut decoded = Vec::with_capacity(2);
    for (index, value) in values.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| "trail emitter must be an object".to_owned())?;
        require_closed_payload(
            object,
            &[
                "emitter_id",
                "socket_node_id",
                "anchor_id",
                "role",
                "owner_part_id",
                "composed_world_transform",
            ],
        )?;
        if object.len() != 6 {
            return Err("trail emitter is missing a required field".to_owned());
        }
        let (expected_emitter, expected_anchor, expected_role, expected_owner) = expected[index];
        let emitter_id = bounded_identifier(object.get("emitter_id"), "emitter_id")?;
        let socket_node_id = bounded_identifier(object.get("socket_node_id"), "socket_node_id")?;
        let anchor_id = bounded_identifier(object.get("anchor_id"), "anchor_id")?;
        let role = bounded_identifier(object.get("role"), "role")?;
        let owner_part_id = bounded_identifier(object.get("owner_part_id"), "owner_part_id")?;
        if emitter_id != expected_emitter
            || socket_node_id != expected_anchor
            || anchor_id != expected_anchor
            || role != expected_role
            || owner_part_id != expected_owner
        {
            return Err("trail emitter binding is outside the fixed role mapping".to_owned());
        }
        let transform = object
            .get("composed_world_transform")
            .and_then(Value::as_object)
            .ok_or_else(|| "trail emitter transform must be an object".to_owned())?;
        require_closed_payload(
            transform,
            &["translation_m", "rotation_quat_xyzw", "scale_xyz"],
        )?;
        if transform.len() != 3 {
            return Err("trail emitter transform is missing a required field".to_owned());
        }
        let translation = decode_f32_array(
            transform.get("translation_m"),
            3,
            1_000.0,
            "trail emitter translation",
        )?;
        let rotation = decode_f32_array(
            transform.get("rotation_quat_xyzw"),
            4,
            1.0,
            "trail emitter rotation",
        )?;
        let length = rotation
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if !length.is_finite() || (length - 1.0).abs() > 1.0e-5 {
            return Err("trail emitter rotation must be unit length".to_owned());
        }
        let scale = decode_f32_array(transform.get("scale_xyz"), 3, 1.0, "trail emitter scale")?;
        if scale
            .iter()
            .any(|value| (*value - 1.0).abs() > f32::EPSILON)
        {
            return Err("trail emitter scale must be unit scale".to_owned());
        }
        decoded.push(AnimatedSocketEmitterBinding {
            emitter_id,
            socket_node_id,
            anchor_id,
            role,
            owner_part_id,
            translation_m: [translation[0], translation[1], translation[2]],
            rotation_quat_xyzw: [rotation[0], rotation[1], rotation[2], rotation[3]],
            scale_xyz: [scale[0], scale[1], scale[2]],
        });
    }
    Ok(decoded
        .try_into()
        .expect("animated trail emitter count is exactly two"))
}

fn animated_socket_trail_seed_sha256(input: &AnimatedSocketTrailInput) -> String {
    let local_inventory = input
        .trail_inventory
        .get("trails")
        .cloned()
        .unwrap_or(Value::Null);
    canonical_json_sha256(&json!({
        "schema_version":ANIMATED_SOCKET_TRAIL_SEED_SCHEMA,
        "projection_key_sha256":input.projection_key_sha256,
        "current_frame_index":input.current_frame_index,
        "current_sample_time_ticks":input.current_sample_time_ticks,
        "projection_input_sha256":input.projection_input_sha256,
        "projection_sample_set_sha256":input.projection_sample_set_sha256,
        "emitter_binding_sha256":input.emitter_binding_sha256,
        "local_trail_inventory":local_inventory
    }))
}

fn animated_socket_particle_seed_sha256(
    projection_key_sha256: &str,
    frame_index: u64,
    sample_time_ticks: u64,
    projection_input_sha256: &str,
    projection_socket_transform_inventory_sha256: &str,
    projection_socket_transform_readback_sha256: &str,
    emitter_binding_sha256: &str,
    world_values: &[Value],
) -> String {
    canonical_json_sha256(&json!({
        "schema_version":"RenderWorkerAnimatedSocketParticleSeed@1",
        "projection_key_sha256":projection_key_sha256,
        "frame_index":frame_index,
        "sample_time_ticks":sample_time_ticks,
        "projection_input_sha256":projection_input_sha256,
        "projection_socket_transform_inventory_sha256":projection_socket_transform_inventory_sha256,
        "projection_socket_transform_readback_sha256":projection_socket_transform_readback_sha256,
        "emitter_binding_sha256":emitter_binding_sha256,
        "local_particle_inventory":world_values
    }))
}

fn decode_animated_socket_emitters(
    value: Option<&Value>,
) -> Result<[AnimatedSocketEmitterBinding; 2], String> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or_else(|| "emitter_bindings must contain exactly two sockets".to_owned())?;
    let expected = [
        (
            "muzzle-burst",
            "socket-muzzle-vfx",
            "muzzle-vfx",
            "barrel-assembly",
        ),
        (
            "energy-core-sparks",
            "socket-energy-core-vfx",
            "energy-core-vfx",
            "energy-core",
        ),
    ];
    let mut decoded = Vec::with_capacity(2);
    for (index, value) in values.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| "animated socket emitter must be an object".to_owned())?;
        require_closed_payload(
            object,
            &[
                "emitter_id",
                "socket_node_id",
                "anchor_id",
                "role",
                "owner_part_id",
                "composed_world_transform",
            ],
        )?;
        if object.len() != 6 {
            return Err("animated socket emitter is missing a required field".to_owned());
        }
        let (expected_emitter, expected_anchor, expected_role, expected_owner) = expected[index];
        let emitter_id = bounded_identifier(object.get("emitter_id"), "emitter_id")?;
        let socket_node_id = bounded_identifier(object.get("socket_node_id"), "socket_node_id")?;
        let anchor_id = bounded_identifier(object.get("anchor_id"), "anchor_id")?;
        let role = bounded_identifier(object.get("role"), "role")?;
        let owner_part_id = bounded_identifier(object.get("owner_part_id"), "owner_part_id")?;
        if emitter_id != expected_emitter
            || anchor_id != expected_anchor
            || role != expected_role
            || owner_part_id != expected_owner
        {
            return Err(
                "animated socket emitter binding is outside the fixed role mapping".to_owned(),
            );
        }
        let transform = object
            .get("composed_world_transform")
            .and_then(Value::as_object)
            .ok_or_else(|| "composed_world_transform must be an object".to_owned())?;
        require_closed_payload(
            transform,
            &["translation_m", "rotation_quat_xyzw", "scale_xyz"],
        )?;
        if transform.len() != 3 {
            return Err("composed_world_transform is missing a required field".to_owned());
        }
        let translation_m = decode_f32_array(
            transform.get("translation_m"),
            3,
            1_000.0,
            "composed translation",
        )?;
        let rotation_quat_xyzw = decode_f32_array(
            transform.get("rotation_quat_xyzw"),
            4,
            1.0,
            "composed rotation",
        )?;
        let quaternion_length = rotation_quat_xyzw
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if !quaternion_length.is_finite() || (quaternion_length - 1.0).abs() > 1.0e-5 {
            return Err("composed rotation quaternion must be unit length".to_owned());
        }
        let scale_xyz = decode_f32_array(transform.get("scale_xyz"), 3, 1.0, "composed scale")?;
        if scale_xyz
            .iter()
            .any(|value| (*value - 1.0).abs() > f32::EPSILON)
        {
            return Err("composed socket scale must be exactly unit scale".to_owned());
        }
        decoded.push(AnimatedSocketEmitterBinding {
            emitter_id,
            socket_node_id,
            anchor_id,
            role,
            owner_part_id,
            translation_m: [translation_m[0], translation_m[1], translation_m[2]],
            rotation_quat_xyzw: [
                rotation_quat_xyzw[0],
                rotation_quat_xyzw[1],
                rotation_quat_xyzw[2],
                rotation_quat_xyzw[3],
            ],
            scale_xyz: [scale_xyz[0], scale_xyz[1], scale_xyz[2]],
        });
    }
    Ok(decoded
        .try_into()
        .expect("animated socket emitter count is exactly two"))
}

fn decode_animated_socket_particles(
    value: Option<&Value>,
    emitters: &[AnimatedSocketEmitterBinding; 2],
    camera: &Value,
) -> Result<(Vec<TypedParticle>, Vec<Value>), String> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == ANIMATED_SOCKET_PARTICLE_COUNT)
        .ok_or_else(|| "particles must contain exactly 56 local values".to_owned())?;
    let mut typed = Vec::with_capacity(values.len());
    let mut world_values = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| "animated socket particle must be an object".to_owned())?;
        require_closed_payload(
            object,
            &[
                "emitter_id",
                "id",
                "local_offset_m",
                "radius_px",
                "color_linear_rgb",
                "alpha",
                "lifetime_ticks",
            ],
        )?;
        if object.len() != 7 {
            return Err("animated socket particle is missing a required field".to_owned());
        }
        let (emitter_index, expected_id) = if index < ANIMATED_SOCKET_MUZZLE_COUNT {
            (0, 10_000_u32 + index as u32)
        } else {
            (
                1,
                20_000_u32 + (index - ANIMATED_SOCKET_MUZZLE_COUNT) as u32,
            )
        };
        let emitter_id = bounded_identifier(object.get("emitter_id"), "emitter_id")?;
        if emitter_id != emitters[emitter_index].emitter_id {
            return Err("animated socket particle emitter order differs".to_owned());
        }
        let id = object
            .get("id")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value == expected_id)
            .ok_or_else(|| {
                "animated socket particle id is outside the fixed encoding".to_owned()
            })?;
        let local_offset_m = decode_f32_array(
            object.get("local_offset_m"),
            3,
            10.0,
            "particle local offset",
        )?;
        let local_offset_m = [local_offset_m[0], local_offset_m[1], local_offset_m[2]];
        let position = animated_socket_transform_point(&emitters[emitter_index], local_offset_m)?;
        let depth = animated_socket_camera_depth(camera, position)?;
        let radius_px = object
            .get("radius_px")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (1.0..=8.0).contains(value))
            .map(|value| value as f32)
            .ok_or_else(|| "animated socket particle radius is invalid".to_owned())?;
        let color_values =
            decode_f32_array(object.get("color_linear_rgb"), 3, 1.0, "particle color")?;
        if color_values.iter().any(|value| *value < 0.0) {
            return Err("animated socket particle color is outside 0..1".to_owned());
        }
        let color_linear_rgb = [color_values[0], color_values[1], color_values[2]];
        let alpha = object
            .get("alpha")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .map(|value| value as f32)
            .ok_or_else(|| "animated socket particle alpha is invalid".to_owned())?;
        let lifetime_ticks = object
            .get("lifetime_ticks")
            .and_then(Value::as_u64)
            .filter(|value| (1..=1_000_000).contains(value))
            .ok_or_else(|| "animated socket particle lifetime is invalid".to_owned())?;
        typed.push(TypedParticle {
            emitter_id: emitter_id.clone(),
            id,
            position,
            radius_px,
            color_linear_rgb,
            alpha,
            lifetime_ticks,
            depth,
        });
        world_values.push(json!({
            "emitter_id":emitter_id,
            "id":id,
            "local_offset_m":local_offset_m,
            "position":position,
            "radius_px":radius_px,
            "color_linear_rgb":color_linear_rgb,
            "alpha":alpha,
            "lifetime_ticks":lifetime_ticks,
            "depth":depth
        }));
    }
    Ok((typed, world_values))
}

fn animated_socket_emitter_value(binding: &AnimatedSocketEmitterBinding) -> Value {
    json!({
        "emitter_id":binding.emitter_id,
        "socket_node_id":binding.socket_node_id,
        "anchor_id":binding.anchor_id,
        "role":binding.role,
        "owner_part_id":binding.owner_part_id,
        "composed_world_transform":{
            "translation_m":binding.translation_m,
            "rotation_quat_xyzw":binding.rotation_quat_xyzw,
            "scale_xyz":binding.scale_xyz
        }
    })
}

fn animated_socket_transform_point(
    binding: &AnimatedSocketEmitterBinding,
    local_offset_m: [f32; 3],
) -> Result<[f32; 3], String> {
    let scaled = [
        local_offset_m[0] * binding.scale_xyz[0],
        local_offset_m[1] * binding.scale_xyz[1],
        local_offset_m[2] * binding.scale_xyz[2],
    ];
    let [x, y, z, w] = binding.rotation_quat_xyzw;
    let q = [x, y, z];
    let twice_cross = animated_socket_scale3(animated_socket_cross3(q, scaled), 2.0);
    let rotated = animated_socket_add3(
        animated_socket_add3(scaled, animated_socket_scale3(twice_cross, w)),
        animated_socket_cross3(q, twice_cross),
    );
    let position = animated_socket_add3(binding.translation_m, rotated);
    if position
        .iter()
        .any(|value| !value.is_finite() || value.abs() > 10.0)
    {
        return Err(
            "animated socket world particle position is outside the bounded domain".to_owned(),
        );
    }
    Ok(position)
}

fn animated_socket_camera_depth(camera: &Value, position: [f32; 3]) -> Result<f32, String> {
    let object = camera
        .as_object()
        .ok_or_else(|| "camera must be an object".to_owned())?;
    let schema_version = object.get("schema_version").and_then(Value::as_str);
    let projection = object.get("projection").and_then(Value::as_str);
    if !matches!(
        schema_version,
        Some("CameraCalibration@1" | "CameraCalibration@2")
    ) || !matches!(projection, Some("perspective" | "orthographic"))
        || (schema_version == Some("CameraCalibration@1") && projection != Some("perspective"))
        || object.get("coordinate_system").and_then(Value::as_str)
            != Some("right-handed-y-up-meter")
        || object
            .get("resolution")
            .and_then(|value| value.get("width"))
            .and_then(Value::as_u64)
            != Some(512)
        || object
            .get("resolution")
            .and_then(|value| value.get("height"))
            .and_then(Value::as_u64)
            != Some(512)
    {
        return Err("CameraCalibration is not the fixed bounded camera contract".to_owned());
    }
    let transform = object
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| "camera transform is missing".to_owned())?;
    let position_camera =
        decode_f32_array(transform.get("position_m"), 3, 1_000.0, "camera position")?;
    let target = decode_f32_array(transform.get("target_m"), 3, 1_000.0, "camera target")?;
    let up_input = decode_f32_array(transform.get("up"), 3, 1_000.0, "camera up")?;
    let position_camera = [position_camera[0], position_camera[1], position_camera[2]];
    let target = [target[0], target[1], target[2]];
    let up_input = [up_input[0], up_input[1], up_input[2]];
    let forward = animated_socket_normalize(animated_socket_subtract3(target, position_camera))?;
    let right = animated_socket_normalize(animated_socket_cross3(forward, up_input))?;
    let _up = animated_socket_normalize(animated_socket_cross3(right, forward))?;
    let near = object
        .get("near_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .map(|value| value as f32)
        .ok_or_else(|| "camera near is invalid".to_owned())?;
    let far = object
        .get("far_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .map(|value| value as f32)
        .ok_or_else(|| "camera far is invalid".to_owned())?;
    if !(near > 0.0 && far > near) {
        return Err("camera clipping limits are invalid".to_owned());
    }
    match projection {
        Some("perspective") => {
            let fov = object
                .get("fov_y_degrees")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value > 1.0 && *value < 179.0)
                .ok_or_else(|| "camera perspective limits are invalid".to_owned())?;
            let _ = fov;
        }
        Some("orthographic") => {
            let scale = object
                .get("ortho_scale")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value > 0.0 && *value <= 100.0)
                .ok_or_else(|| "camera orthographic limits are invalid".to_owned())?;
            let _ = scale;
        }
        _ => return Err("camera projection is invalid".to_owned()),
    }
    let relative = animated_socket_subtract3(position, position_camera);
    let z = animated_socket_dot3(relative, forward);
    if !z.is_finite() || z <= near || z >= far {
        return Err("animated socket particle is outside the camera clip range".to_owned());
    }
    let depth = (z - near) / (far - near);
    if !depth.is_finite() || !(0.0..=1.0).contains(&depth) {
        return Err("animated socket particle camera depth is invalid".to_owned());
    }
    Ok(depth)
}

fn decode_f32_array(
    value: Option<&Value>,
    expected_len: usize,
    absolute_max: f32,
    field: &str,
) -> Result<Vec<f32>, String> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == expected_len)
        .ok_or_else(|| format!("{field} must contain exactly {expected_len} values"))?;
    values
        .iter()
        .map(|value| {
            let number = value
                .as_f64()
                .filter(|value| value.is_finite())
                .map(|value| value as f32)
                .filter(|value| value.is_finite() && value.abs() <= absolute_max)
                .ok_or_else(|| format!("{field} contains a non-finite or out-of-range value"))?;
            Ok(number)
        })
        .collect()
}

fn required_sha256(value: Option<&Value>, field: &str) -> Result<String, String> {
    let value = value
        .and_then(Value::as_str)
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        })
        .ok_or_else(|| format!("{field} must be lowercase sha256"))?;
    Ok(value.to_owned())
}

fn animated_socket_subtract3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn animated_socket_add3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn animated_socket_scale3(value: [f32; 3], scalar: f32) -> [f32; 3] {
    [value[0] * scalar, value[1] * scalar, value[2] * scalar]
}

fn animated_socket_dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn animated_socket_cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn animated_socket_normalize(value: [f32; 3]) -> Result<[f32; 3], String> {
    let length = animated_socket_dot3(value, value).sqrt();
    if !length.is_finite() || length <= f32::EPSILON {
        return Err("camera basis is degenerate".to_owned());
    }
    Ok([value[0] / length, value[1] / length, value[2] / length])
}

fn decode_emissive_overrides(
    value: Option<&Value>,
) -> Result<Vec<EmissiveMaterialOverride>, String> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 8)
        .ok_or_else(|| "emissive_overrides must contain 1 to 8 items".to_owned())?;
    values
        .iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| "emissive override must be an object".to_owned())?;
            require_closed_payload(
                object,
                &[
                    "material_zone_id",
                    "material_id",
                    "color_linear_rgb",
                    "emissive_strength",
                ],
            )?;
            if object.len() != 4 {
                return Err("emissive override is missing a required field".to_owned());
            }
            let material_zone_id =
                bounded_identifier(object.get("material_zone_id"), "material_zone_id")?;
            let material_id = bounded_identifier(object.get("material_id"), "material_id")?;
            let color = object
                .get("color_linear_rgb")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 3)
                .ok_or_else(|| "color_linear_rgb must contain exactly three channels".to_owned())?;
            let mut color_linear_rgb = [0.0_f32; 3];
            for (index, channel) in color.iter().enumerate() {
                color_linear_rgb[index] = channel
                    .as_f64()
                    .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                    .ok_or_else(|| "color_linear_rgb is outside 0 to 1".to_owned())?
                    as f32;
            }
            let emissive_strength = object
                .get("emissive_strength")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && (0.0..=16.0).contains(value))
                .ok_or_else(|| "emissive_strength is outside 0 to 16".to_owned())?
                as f32;
            Ok(EmissiveMaterialOverride {
                material_zone_id,
                material_id,
                color_linear_rgb,
                emissive_strength,
            })
        })
        .collect()
}

fn decode_hdr_bloom_profile(value: Option<&Value>) -> Result<HdrBloomProfile, String> {
    let object = value
        .and_then(Value::as_object)
        .ok_or_else(|| "bloom_profile must be an object".to_owned())?;
    require_closed_payload(
        object,
        &["threshold", "radius_px", "intensity", "hdr_clamp"],
    )?;
    if object.len() != 4 {
        return Err("bloom_profile is missing a required field".to_owned());
    }
    let threshold = object
        .get("threshold")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && (0.0..=16.0).contains(value))
        .ok_or_else(|| "bloom threshold is outside 0 to 16".to_owned())? as f32;
    let radius_px = object
        .get("radius_px")
        .and_then(Value::as_u64)
        .filter(|value| (1..=8).contains(value))
        .ok_or_else(|| "bloom radius is outside 1 to 8".to_owned())? as u32;
    let intensity = object
        .get("intensity")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && (0.0..=4.0).contains(value))
        .ok_or_else(|| "bloom intensity is outside 0 to 4".to_owned())? as f32;
    let hdr_clamp = object
        .get("hdr_clamp")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && (1.0..=16.0).contains(value))
        .ok_or_else(|| "bloom HDR clamp is outside 1 to 16".to_owned())? as f32;
    HdrBloomProfile {
        threshold,
        radius_px,
        intensity,
        hdr_clamp,
    }
    .validate()
    .map_err(|error| error.to_string())
}

fn decode_typed_trail_bloom_profile(
    value: Option<&Value>,
) -> Result<TypedTrailBloomProfile, String> {
    let object = value
        .and_then(Value::as_object)
        .ok_or_else(|| "trail_bloom_profile must be an object".to_owned())?;
    require_closed_payload(
        object,
        &[
            "threshold",
            "radius_px",
            "intensity",
            "hdr_clamp",
            "source_gain",
        ],
    )?;
    if object.len() != 5 {
        return Err("trail_bloom_profile is missing a required field".to_owned());
    }
    let threshold = object
        .get("threshold")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value == 1.0)
        .ok_or_else(|| "trail Bloom threshold must be the fixed value 1".to_owned())?
        as f32;
    let radius_px = object
        .get("radius_px")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value == 8)
        .ok_or_else(|| "trail Bloom radius is invalid".to_owned())?;
    let intensity = object
        .get("intensity")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value == 4.0)
        .ok_or_else(|| "trail Bloom intensity must be the fixed value 4".to_owned())?
        as f32;
    let hdr_clamp = object
        .get("hdr_clamp")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value == 16.0)
        .ok_or_else(|| "trail Bloom HDR clamp must be the fixed value 16".to_owned())?
        as f32;
    let source_gain = object
        .get("source_gain")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value == 8.0)
        .ok_or_else(|| "trail Bloom source gain must be the fixed value 8".to_owned())?
        as f32;
    TypedTrailBloomProfile {
        threshold,
        radius_px,
        intensity,
        hdr_clamp,
        source_gain,
    }
    .validate_fixed()
    .map_err(|error| error.to_string())
}

fn decode_typed_particles(value: Option<&Value>) -> Result<Vec<TypedParticle>, String> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 128)
        .ok_or_else(|| "particles must contain 1 to 128 typed values".to_owned())?;
    let mut ids = std::collections::HashSet::new();
    values
        .iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| "particle must be an object".to_owned())?;
            require_closed_payload(
                object,
                &[
                    "emitter_id",
                    "id",
                    "position",
                    "radius_px",
                    "color_linear_rgb",
                    "alpha",
                    "lifetime_ticks",
                    "depth",
                ],
            )?;
            if object.len() != 8 {
                return Err("particle is missing a required field".to_owned());
            }
            let emitter_id = bounded_identifier(object.get("emitter_id"), "emitter_id")?;
            if emitter_id != "muzzle-burst" && emitter_id != "energy-core-sparks" {
                return Err("particle emitter_id is outside the closed set".to_owned());
            }
            let id = object
                .get("id")
                .and_then(Value::as_u64)
                .filter(|value| (1..=65_535).contains(value))
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| "particle id is invalid".to_owned())?;
            if !ids.insert(id) {
                return Err("particle ids must be unique".to_owned());
            }
            let position = object
                .get("position")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 3)
                .ok_or_else(|| "particle position must have three values".to_owned())?
                .iter()
                .map(|value| {
                    value
                        .as_f64()
                        .filter(|value| value.is_finite() && value.abs() <= 10.0)
                        .map(|value| value as f32)
                        .ok_or_else(|| "particle position is invalid".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let color = object
                .get("color_linear_rgb")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 3)
                .ok_or_else(|| "particle color must have three values".to_owned())?
                .iter()
                .map(|value| {
                    value
                        .as_f64()
                        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                        .map(|value| value as f32)
                        .ok_or_else(|| "particle color is invalid".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let radius_px = object
                .get("radius_px")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && (1.0..=8.0).contains(value))
                .map(|value| value as f32)
                .ok_or_else(|| "particle radius is invalid".to_owned())?;
            let alpha = object
                .get("alpha")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .map(|value| value as f32)
                .ok_or_else(|| "particle alpha is invalid".to_owned())?;
            let lifetime_ticks = object
                .get("lifetime_ticks")
                .and_then(Value::as_u64)
                .filter(|value| (1..=1_000_000).contains(value))
                .ok_or_else(|| "particle lifetime_ticks is invalid".to_owned())?;
            let depth = object
                .get("depth")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .map(|value| value as f32)
                .ok_or_else(|| "particle depth is invalid".to_owned())?;
            Ok(TypedParticle {
                emitter_id,
                id,
                position: [position[0], position[1], position[2]],
                radius_px,
                color_linear_rgb: [color[0], color[1], color[2]],
                alpha,
                lifetime_ticks,
                depth,
            })
        })
        .collect()
}

fn decode_typed_trails(value: Option<&Value>) -> Result<Vec<TypedTrail>, String> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 16)
        .ok_or_else(|| "trails must contain 1 to 16 typed values".to_owned())?;
    let mut ids = std::collections::HashSet::new();
    let mut segment_count = 0usize;
    values
        .iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| "trail must be an object".to_owned())?;
            require_closed_payload(
                object,
                &[
                    "emitter_id",
                    "id",
                    "points",
                    "radius_px",
                    "color_linear_rgb",
                    "alpha",
                    "lifetime_ticks",
                ],
            )?;
            if object.len() != 7 {
                return Err("trail is missing a required field".to_owned());
            }
            let emitter_id = bounded_identifier(object.get("emitter_id"), "emitter_id")?;
            if emitter_id != "muzzle-trail" && emitter_id != "energy-core-trail" {
                return Err("trail emitter_id is outside the closed set".to_owned());
            }
            let id = object
                .get("id")
                .and_then(Value::as_u64)
                .filter(|value| (1..=65_535).contains(value))
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| "trail id is invalid".to_owned())?;
            if !ids.insert(id) {
                return Err("trail ids must be unique".to_owned());
            }
            let points_value = object
                .get("points")
                .and_then(Value::as_array)
                .filter(|values| (2..=32).contains(&values.len()))
                .ok_or_else(|| "trail points must contain 2 to 32 values".to_owned())?;
            segment_count = segment_count.saturating_add(points_value.len() - 1);
            if segment_count > 128 {
                return Err("trail segment count exceeds the fixed limit".to_owned());
            }
            let points = points_value
                .iter()
                .map(|point| {
                    point
                        .as_array()
                        .filter(|values| values.len() == 3)
                        .ok_or_else(|| "trail point must have three values".to_owned())?
                        .iter()
                        .map(|value| {
                            value
                                .as_f64()
                                .filter(|value| value.is_finite() && value.abs() <= 10.0)
                                .map(|value| value as f32)
                                .ok_or_else(|| "trail point is invalid".to_owned())
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map(|point| [point[0], point[1], point[2]])
                })
                .collect::<Result<Vec<_>, _>>()?;
            let color = object
                .get("color_linear_rgb")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 3)
                .ok_or_else(|| "trail color must have three values".to_owned())?
                .iter()
                .map(|value| {
                    value
                        .as_f64()
                        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                        .map(|value| value as f32)
                        .ok_or_else(|| "trail color is invalid".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let radius_px = object
                .get("radius_px")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && (1.0..=8.0).contains(value))
                .map(|value| value as f32)
                .ok_or_else(|| "trail radius is invalid".to_owned())?;
            let alpha = object
                .get("alpha")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .map(|value| value as f32)
                .ok_or_else(|| "trail alpha is invalid".to_owned())?;
            let lifetime_ticks = object
                .get("lifetime_ticks")
                .and_then(Value::as_u64)
                .filter(|value| (1..=1_000_000).contains(value))
                .ok_or_else(|| "trail lifetime_ticks is invalid".to_owned())?;
            Ok(TypedTrail {
                emitter_id,
                id,
                points,
                radius_px,
                color_linear_rgb: [color[0], color[1], color[2]],
                alpha,
                lifetime_ticks,
            })
        })
        .collect()
}

fn bounded_identifier(value: Option<&Value>, field: &str) -> Result<String, String> {
    value
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
        .map(str::to_owned)
        .ok_or_else(|| format!("{field} is invalid"))
}

fn require_closed_payload(payload: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    if payload.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("worker payload contains an unknown field".to_owned());
    }
    Ok(())
}

fn decode_render_glb(payload: &Map<String, Value>) -> Result<Vec<u8>, String> {
    let encoded = payload
        .get("glb_base64")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "glb_base64 is required".to_owned())?;
    let glb = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| "glb_base64 is invalid".to_owned())?;
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 {
        return Err("GLB exceeds the bounded render input".to_owned());
    }
    Ok(glb)
}

fn serialize_passes(passes: &[RenderPass]) -> Vec<Value> {
    passes
        .iter()
        .map(|pass| {
            serde_json::json!({
                "pass":pass.pass,
                "mime":"image/png",
                "width":pass.width,
                "height":pass.height,
                "png_base64":base64::engine::general_purpose::STANDARD.encode(&pass.png)
            })
        })
        .collect()
}

fn error_response(request_id: &str, code: &str, message: impl Into<String>) -> WorkerResponse {
    WorkerResponse {
        protocol: WORKER_PROTOCOL.to_owned(),
        request_id: request_id.to_owned(),
        build_cohort_sha256: build_cohort_sha256(),
        ok: false,
        result: None,
        error: Some(WorkerError {
            code: code.to_owned(),
            message: message.into(),
        }),
    }
}

fn emit(stdout: &mut impl Write, response: WorkerResponse) -> bool {
    let bytes = serde_json::to_vec(&response).expect("worker response serializes");
    if bytes.len() > MAX_WORKER_RESPONSE_BYTES {
        let fallback = error_response(
            &response.request_id,
            "WORKER_RESPONSE_TOO_LARGE",
            "render response exceeds the bounded worker response",
        );
        let fallback_bytes = match serde_json::to_vec(&fallback) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        if stdout.write_all(&fallback_bytes).is_err() {
            return false;
        }
    } else {
        if stdout.write_all(&bytes).is_err() {
            return false;
        }
    }
    stdout.write_all(b"\n").is_ok() && stdout.flush().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_worker_rejects_geometry_compile_payload() {
        let request = WorkerRequest {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "render-boundary-test-1".to_owned(),
            operation: "render_fixed".to_owned(),
            payload: serde_json::json!({
                "geometry_program": {},
                "appearance_program": {}
            }),
        };
        let error =
            render_worker_result(&request).expect_err("render boundary must reject compiler input");
        assert!(error.contains("unknown field"));
    }

    #[test]
    fn vfx_frame_payload_rejects_script_and_out_of_range_strength() {
        let payload = serde_json::json!({
            "glb_base64":"AA==",
            "camera":{},
            "emissive_overrides":[{
                "material_zone_id":"zone-core-emissive",
                "material_id":"energy-cyan-emissive",
                "color_linear_rgb":[0.0,0.82,1.0],
                "emissive_strength":16.01,
                "script":"forbidden"
            }]
        });
        let error = decode_emissive_overrides(payload.get("emissive_overrides"))
            .expect_err("unknown executable field must fail closed");
        assert!(error.contains("unknown field"));

        let payload = serde_json::json!([{
            "material_zone_id":"zone-core-emissive",
            "material_id":"energy-cyan-emissive",
            "color_linear_rgb":[0.0,0.82,1.0],
            "emissive_strength":16.01
        }]);
        assert!(decode_emissive_overrides(Some(&payload)).is_err());
    }

    #[test]
    fn bloom_profile_is_closed_and_bounded() {
        let profile = serde_json::json!({
            "threshold":1.0,
            "radius_px":8,
            "intensity":4.0,
            "hdr_clamp":16.0
        });
        assert_eq!(
            decode_hdr_bloom_profile(Some(&profile)).unwrap().radius_px,
            8
        );
        let invalid = serde_json::json!({
            "threshold":1.0,
            "radius_px":9,
            "intensity":4.0,
            "hdr_clamp":16.0
        });
        assert!(decode_hdr_bloom_profile(Some(&invalid)).is_err());
        let executable = serde_json::json!({
            "threshold":1.0,
            "radius_px":4,
            "intensity":2.0,
            "hdr_clamp":8.0,
            "shader":"forbidden"
        });
        assert!(decode_hdr_bloom_profile(Some(&executable)).is_err());
    }

    #[test]
    fn typed_particle_payload_is_closed_bounded_and_unique() {
        let particle = serde_json::json!({
            "emitter_id":"muzzle-burst",
            "id":10000,
            "position":[0.0,0.0,0.0],
            "radius_px":4.0,
            "color_linear_rgb":[0.0,0.82,1.0],
            "alpha":0.8,
            "lifetime_ticks":120,
            "depth":0.5
        });
        assert_eq!(
            decode_typed_particles(Some(&serde_json::json!([particle.clone()])))
                .expect("closed particle")
                .len(),
            1
        );
        let mut executable = particle.clone();
        executable["shader"] = Value::String("forbidden".to_owned());
        assert!(decode_typed_particles(Some(&serde_json::json!([executable]))).is_err());

        let duplicate = serde_json::json!([particle.clone(), particle.clone()]);
        assert!(decode_typed_particles(Some(&duplicate)).is_err());

        let mut reserved_background_id = particle.clone();
        reserved_background_id["id"] = Value::from(0);
        assert!(
            decode_typed_particles(Some(&serde_json::json!([reserved_background_id]))).is_err()
        );

        let mut excessive = particle;
        excessive["radius_px"] = Value::from(8.01);
        assert!(decode_typed_particles(Some(&serde_json::json!([excessive]))).is_err());
    }

    #[test]
    fn typed_trail_payload_is_closed_bounded_and_rejects_reserved_id() {
        let trail = serde_json::json!({
            "emitter_id":"muzzle-trail",
            "id":30000,
            "points":[[0.0,0.0,0.0],[0.1,0.0,0.0],[0.2,0.01,0.0]],
            "radius_px":3.0,
            "color_linear_rgb":[0.0,0.82,1.0],
            "alpha":0.75,
            "lifetime_ticks":180
        });
        assert_eq!(
            decode_typed_trails(Some(&serde_json::json!([trail.clone()])))
                .expect("closed trail")
                .len(),
            1
        );
        let mut executable = trail.clone();
        executable["shader"] = Value::String("forbidden".to_owned());
        assert!(decode_typed_trails(Some(&serde_json::json!([executable]))).is_err());

        let mut reserved_background_id = trail.clone();
        reserved_background_id["id"] = Value::from(0);
        assert!(decode_typed_trails(Some(&serde_json::json!([reserved_background_id]))).is_err());

        let duplicate = serde_json::json!([trail.clone(), trail.clone()]);
        assert!(decode_typed_trails(Some(&duplicate)).is_err());

        let mut excessive = trail;
        excessive["points"] = Value::Array(
            (0..33)
                .map(|index| {
                    Value::Array(vec![
                        Value::from(index as f64 / 100.0),
                        Value::from(0.0),
                        Value::from(0.0),
                    ])
                })
                .collect(),
        );
        assert!(decode_typed_trails(Some(&serde_json::json!([excessive]))).is_err());
    }

    #[test]
    fn typed_trail_bloom_profile_is_fixed_and_closed() {
        let profile = serde_json::json!({
            "threshold":1.0,
            "radius_px":8,
            "intensity":4.0,
            "hdr_clamp":16.0,
            "source_gain":8.0
        });
        let decoded =
            decode_typed_trail_bloom_profile(Some(&profile)).expect("fixed trail Bloom profile");
        assert_eq!(decoded, TypedTrailBloomProfile::FIXED);

        let mut executable = profile.clone();
        executable["kernel"] = Value::String("caller-kernel".to_owned());
        assert!(decode_typed_trail_bloom_profile(Some(&executable)).is_err());

        let mut mutable_gain = profile;
        mutable_gain["source_gain"] = Value::from(7.0);
        assert!(decode_typed_trail_bloom_profile(Some(&mutable_gain)).is_err());
    }

    fn animated_socket_test_camera() -> Value {
        json!({
            "schema_version":"CameraCalibration@2",
            "projection":"perspective",
            "coordinate_system":"right-handed-y-up-meter",
            "resolution":{"width":512,"height":512},
            "transform":{
                "position_m":[0.0,0.0,0.0],
                "target_m":[0.0,0.0,5.0],
                "up":[0.0,1.0,0.0]
            },
            "near_m":0.05,
            "far_m":20.0,
            "fov_y_degrees":45.0
        })
    }

    fn push_f32_bytes(output: &mut Vec<u8>, value: f32) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32_bytes(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn animated_socket_test_glb() -> Vec<u8> {
        let mut binary = Vec::new();
        for vertex in [[-1.0_f32, -1.0, 5.0], [1.0, -1.0, 5.0], [0.0, 1.0, 5.0]] {
            for value in vertex {
                push_f32_bytes(&mut binary, value);
            }
        }
        for _ in 0..3 {
            for value in [0.0_f32, 0.0, -1.0] {
                push_f32_bytes(&mut binary, value);
            }
        }
        for uv in [[0.0_f32, 0.0], [1.0, 0.0], [0.5, 1.0]] {
            for value in uv {
                push_f32_bytes(&mut binary, value);
            }
        }
        for value in [0_u32, 1, 2] {
            push_u32_bytes(&mut binary, value);
        }
        assert_eq!(binary.len(), 108);
        let root = json!({
            "asset":{"version":"2.0"},
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"mesh":0}],
            "materials":[{"name":"test-material"}],
            "meshes":[{"primitives":[{
                "attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2},
                "indices":3,
                "material":0
            }]}],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},
                {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"},
                {"bufferView":2,"componentType":5126,"count":3,"type":"VEC2"},
                {"bufferView":3,"componentType":5125,"count":3,"type":"SCALAR"}
            ],
            "bufferViews":[
                {"buffer":0,"byteOffset":0,"byteLength":36},
                {"buffer":0,"byteOffset":36,"byteLength":36},
                {"buffer":0,"byteOffset":72,"byteLength":24},
                {"buffer":0,"byteOffset":96,"byteLength":12}
            ],
            "buffers":[{"byteLength":108}]
        });
        let mut json_bytes = serde_json::to_vec(&root).expect("test GLB JSON serializes");
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }
        let total_length = 12 + 8 + json_bytes.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json_bytes);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(&binary);
        glb
    }

    fn animated_socket_test_payload(glb: &[u8]) -> Value {
        let camera = animated_socket_test_camera();
        let mut particles = Vec::with_capacity(ANIMATED_SOCKET_PARTICLE_COUNT);
        for index in 0..ANIMATED_SOCKET_MUZZLE_COUNT {
            let x = (index % 6) as f64 * 0.015 - 0.0375;
            let y = (index / 6) as f64 * 0.02 - 0.06;
            particles.push(json!({
                "emitter_id":"muzzle-burst",
                "id":10000 + index,
                "local_offset_m":[x,y,0.0],
                "radius_px":2.0,
                "color_linear_rgb":[0.0,0.82,1.0],
                "alpha":0.8,
                "lifetime_ticks":120
            }));
        }
        for index in 0..ANIMATED_SOCKET_CORE_COUNT {
            let x = (index % 8) as f64 * 0.015 - 0.0525;
            let y = (index / 8) as f64 * 0.02 - 0.03;
            particles.push(json!({
                "emitter_id":"energy-core-sparks",
                "id":20000 + index,
                "local_offset_m":[x,y,0.0],
                "radius_px":2.0,
                "color_linear_rgb":[1.0,0.4,0.05],
                "alpha":0.75,
                "lifetime_ticks":160
            }));
        }
        let mut payload = json!({
            "glb_base64":base64::engine::general_purpose::STANDARD.encode(glb),
            "camera":camera,
            "projection_key_sha256":"1".repeat(64),
            "frame_index":3,
            "sample_time_ticks":240,
            "projection_input_sha256":"2".repeat(64),
            "projection_socket_transform_inventory_sha256":"3".repeat(64),
            "projection_socket_transform_readback_sha256":"4".repeat(64),
            "emitter_bindings":[
                {
                    "emitter_id":"muzzle-burst",
                    "socket_node_id":"socket-node-muzzle",
                    "anchor_id":"socket-muzzle-vfx",
                    "role":"muzzle-vfx",
                    "owner_part_id":"barrel-assembly",
                    "composed_world_transform":{
                        "translation_m":[0.0,0.0,4.0],
                        "rotation_quat_xyzw":[0.0,0.0,0.0,1.0],
                        "scale_xyz":[1.0,1.0,1.0]
                    }
                },
                {
                    "emitter_id":"energy-core-sparks",
                    "socket_node_id":"socket-node-core",
                    "anchor_id":"socket-energy-core-vfx",
                    "role":"energy-core-vfx",
                    "owner_part_id":"energy-core",
                    "composed_world_transform":{
                        "translation_m":[0.25,0.0,4.0],
                        "rotation_quat_xyzw":[0.0,0.0,0.70710677,0.70710677],
                        "scale_xyz":[1.0,1.0,1.0]
                    }
                }
            ],
            "particles":particles,
            "seed_sha256":"0".repeat(64)
        });
        let payload_object = payload.as_object().expect("animated payload object");
        let emitters = decode_animated_socket_emitters(payload_object.get("emitter_bindings"))
            .expect("animated fixture emitters");
        let (_, world_values) = decode_animated_socket_particles(
            payload_object.get("particles"),
            &emitters,
            payload_object
                .get("camera")
                .expect("animated fixture camera"),
        )
        .expect("animated fixture particles");
        let emitter_binding_value = json!({
            "schema_version":ANIMATED_SOCKET_EMITTER_BINDING_SCHEMA,
            "emitters":emitters.iter().map(animated_socket_emitter_value).collect::<Vec<_>>()
        });
        let emitter_binding_sha256 = canonical_json_sha256(&emitter_binding_value);
        let seed_sha256 = animated_socket_particle_seed_sha256(
            payload_object
                .get("projection_key_sha256")
                .and_then(Value::as_str)
                .expect("projection key"),
            payload_object
                .get("frame_index")
                .and_then(Value::as_u64)
                .expect("frame index"),
            payload_object
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                .expect("sample tick"),
            payload_object
                .get("projection_input_sha256")
                .and_then(Value::as_str)
                .expect("projection input"),
            payload_object
                .get("projection_socket_transform_inventory_sha256")
                .and_then(Value::as_str)
                .expect("projection inventory"),
            payload_object
                .get("projection_socket_transform_readback_sha256")
                .and_then(Value::as_str)
                .expect("projection readback"),
            &emitter_binding_sha256,
            &world_values,
        );
        payload["seed_sha256"] = Value::String(seed_sha256);
        payload
    }

    fn animated_socket_test_request(payload: Value) -> WorkerRequest {
        WorkerRequest {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "animated-socket-particles-test".to_owned(),
            operation: RENDER_TYPED_ANIMATED_SOCKET_PARTICLES_OPERATION.to_owned(),
            payload,
        }
    }

    fn animated_socket_trail_test_payload(glb: &[u8]) -> Value {
        let samples = (0..3_u64)
            .map(|index| {
                let frame_index = index + 1;
                let sample_time_ticks = (index + 1) * 80;
                json!({
                    "frame_index":frame_index,
                    "sample_time_ticks":sample_time_ticks,
                    "projection_frame_canonical_sha256":format!("{}{}", "1", frame_index).chars().cycle().take(64).collect::<String>(),
                    "projection_socket_transform_inventory_sha256":"2".repeat(64),
                    "projection_socket_transform_readback_sha256":"3".repeat(64),
                    "emitters":[
                        {
                            "emitter_id":"muzzle-trail",
                            "socket_node_id":"socket-muzzle-vfx",
                            "anchor_id":"socket-muzzle-vfx",
                            "role":"muzzle-vfx",
                            "owner_part_id":"barrel-assembly",
                            "composed_world_transform":{
                                "translation_m":[0.0,0.0,4.0 + index as f64 * 0.15],
                                "rotation_quat_xyzw":[0.0,0.0,0.0,1.0],
                                "scale_xyz":[1.0,1.0,1.0]
                            }
                        },
                        {
                            "emitter_id":"energy-core-trail",
                            "socket_node_id":"socket-energy-core-vfx",
                            "anchor_id":"socket-energy-core-vfx",
                            "role":"energy-core-vfx",
                            "owner_part_id":"energy-core",
                            "composed_world_transform":{
                                "translation_m":[0.25,0.0,4.0 + index as f64 * 0.15],
                                "rotation_quat_xyzw":[0.0,0.0,0.0,1.0],
                                "scale_xyz":[1.0,1.0,1.0]
                            }
                        }
                    ]
                })
            })
            .collect::<Vec<_>>();
        let trails = json!([
            {
                "emitter_id":"muzzle-trail",
                "id":30000,
                "local_points":[
                    {"frame_index":1,"sample_time_ticks":80,"source_particle_key_sha256":"4".repeat(64),"source_particle_id":10000,"local_offset_m":[0.0,0.0,0.0]},
                    {"frame_index":2,"sample_time_ticks":160,"source_particle_key_sha256":"5".repeat(64),"source_particle_id":10000,"local_offset_m":[0.02,0.0,0.0]},
                    {"frame_index":3,"sample_time_ticks":240,"source_particle_key_sha256":"6".repeat(64),"source_particle_id":10000,"local_offset_m":[0.04,0.01,0.0]}
                ],
                "radius_px":3.0,
                "color_linear_rgb":[0.0,0.82,1.0],
                "alpha":0.8,
                "lifetime_ticks":180
            },
            {
                "emitter_id":"energy-core-trail",
                "id":31000,
                "local_points":[
                    {"frame_index":1,"sample_time_ticks":80,"source_particle_key_sha256":"7".repeat(64),"source_particle_id":20000,"local_offset_m":[0.0,0.0,0.0]},
                    {"frame_index":2,"sample_time_ticks":160,"source_particle_key_sha256":"8".repeat(64),"source_particle_id":20000,"local_offset_m":[0.02,0.0,0.0]},
                    {"frame_index":3,"sample_time_ticks":240,"source_particle_key_sha256":"9".repeat(64),"source_particle_id":20000,"local_offset_m":[0.04,-0.01,0.0]}
                ],
                "radius_px":2.5,
                "color_linear_rgb":[1.0,0.4,0.05],
                "alpha":0.75,
                "lifetime_ticks":180
            }
        ]);
        let mut payload = json!({
            "glb_base64":base64::engine::general_purpose::STANDARD.encode(glb),
            "camera":animated_socket_test_camera(),
            "projection_key_sha256":"a".repeat(64),
            "current_frame_index":3,
            "current_sample_time_ticks":240,
            "projection_input_sha256":"b".repeat(64),
            "projection_samples":samples,
            "trails":trails,
            "seed_sha256":"0".repeat(64)
        });
        let input = decode_animated_socket_trail_input(
            payload.as_object().expect("trail payload object"),
            payload.get("camera").expect("trail camera"),
        )
        .expect("trail fixture input");
        payload["seed_sha256"] = Value::String(animated_socket_trail_seed_sha256(&input));
        payload
    }

    fn animated_socket_trail_test_request(payload: Value) -> WorkerRequest {
        WorkerRequest {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "animated-socket-trails-test".to_owned(),
            operation: RENDER_TYPED_ANIMATED_SOCKET_TRAILS_OPERATION.to_owned(),
            payload,
        }
    }

    #[test]
    fn animated_socket_particles_are_trs_bound_and_byte_exact_on_replay() {
        let glb = animated_socket_test_glb();
        let payload = animated_socket_test_payload(&glb);
        let request = animated_socket_test_request(payload.clone());
        let first = render_worker_result(&request).expect("animated socket particle frame");
        let second = render_worker_result(&request).expect("animated socket particle replay");
        assert_eq!(first, second);
        let result = first.as_object().expect("animated result object");
        assert_eq!(result["schema_version"], ANIMATED_SOCKET_PARTICLE_SCHEMA);
        assert_eq!(result["particle_count"], 56);
        assert_eq!(result["particle_passes"].as_array().unwrap().len(), 3);
        assert_eq!(
            result["world_particle_inventory"]["canonical_sha256"],
            result["world_particle_inventory_sha256"]
        );
        assert_eq!(
            result["seed_sha256"], payload["seed_sha256"],
            "the seed is part of the returned inventory binding"
        );
        let inventory = result["world_particle_inventory"]["particles"]
            .as_array()
            .expect("world particle inventory");
        assert_eq!(inventory.len(), ANIMATED_SOCKET_PARTICLE_COUNT);
        let first_position = inventory[0]["position"]
            .as_array()
            .expect("first world position");
        assert!((first_position[2].as_f64().unwrap() - 4.0).abs() < 1.0e-6);
        let core_position = inventory[ANIMATED_SOCKET_MUZZLE_COUNT]["position"]
            .as_array()
            .expect("core world position");
        assert!((core_position[0].as_f64().unwrap() - 0.28).abs() < 1.0e-4);
        assert!((core_position[1].as_f64().unwrap() + 0.0525).abs() < 1.0e-4);
    }

    #[test]
    fn animated_socket_particles_reject_unknown_and_retargeted_input() {
        let glb = animated_socket_test_glb();
        let base = animated_socket_test_payload(&glb);

        let mut unknown = base.clone();
        unknown["unknown"] = Value::String("forbidden".to_owned());
        let error = render_worker_result(&animated_socket_test_request(unknown))
            .expect_err("unknown operation field must fail closed");
        assert!(error.contains("unknown field"));

        let mut wrong_role = base.clone();
        wrong_role["emitter_bindings"][0]["role"] = Value::String("energy-core-vfx".to_owned());
        assert!(render_worker_result(&animated_socket_test_request(wrong_role)).is_err());

        let mut wrong_owner = base.clone();
        wrong_owner["emitter_bindings"][1]["owner_part_id"] =
            Value::String("barrel-assembly".to_owned());
        assert!(render_worker_result(&animated_socket_test_request(wrong_owner)).is_err());

        let mut retargeted_projection = base.clone();
        retargeted_projection["projection_key_sha256"] = Value::String("f".repeat(64));
        let error = render_worker_result(&animated_socket_test_request(retargeted_projection))
            .expect_err("projection retarget must invalidate the seed binding");
        assert!(
            error.contains("seed_sha256"),
            "unexpected projection retarget error: {error}"
        );

        let mut local_retarget = base;
        local_retarget["particles"][0]["local_offset_m"] = json!([0.3, 0.0, 0.0]);
        let error = render_worker_result(&animated_socket_test_request(local_retarget))
            .expect_err("local inventory retarget must invalidate the seed binding");
        assert!(
            error.contains("seed_sha256"),
            "unexpected local retarget error: {error}"
        );
    }

    #[test]
    fn animated_socket_particles_reject_non_unit_trs_and_wrong_particle_shape() {
        let glb = animated_socket_test_glb();
        let base = animated_socket_test_payload(&glb);

        let mut non_unit_quaternion = base.clone();
        non_unit_quaternion["emitter_bindings"][0]["composed_world_transform"]
            ["rotation_quat_xyzw"] = json!([0.0, 0.0, 0.0, 0.0]);
        assert!(render_worker_result(&animated_socket_test_request(non_unit_quaternion)).is_err());

        let mut non_unit_scale = base.clone();
        non_unit_scale["emitter_bindings"][1]["composed_world_transform"]["scale_xyz"] =
            json!([1.0, 2.0, 1.0]);
        assert!(render_worker_result(&animated_socket_test_request(non_unit_scale)).is_err());

        let mut wrong_count = base.clone();
        wrong_count["particles"]
            .as_array_mut()
            .expect("particle array")
            .pop();
        assert!(render_worker_result(&animated_socket_test_request(wrong_count)).is_err());

        let mut wrong_id = base;
        wrong_id["particles"][0]["id"] = Value::from(65_535_u64);
        assert!(render_worker_result(&animated_socket_test_request(wrong_id)).is_err());
    }

    #[test]
    fn animated_socket_trails_are_history_trs_bound_and_bloom_prefix_is_exact() {
        let glb = animated_socket_test_glb();
        let payload = animated_socket_trail_test_payload(&glb);
        let request = animated_socket_trail_test_request(payload.clone());
        let first = render_worker_result(&request).expect("animated socket trail frame");
        let second = render_worker_result(&request).expect("animated socket trail replay");
        assert_eq!(first, second);
        let first_object = first.as_object().expect("trail result object");
        assert_eq!(first_object["schema_version"], ANIMATED_SOCKET_TRAIL_SCHEMA);
        assert_eq!(first_object["trail_count"], 2);
        assert_eq!(first_object["segment_count"], 4);
        assert_eq!(first_object["trail_passes"].as_array().unwrap().len(), 3);
        assert_eq!(
            first_object["trail_inventory"]["canonical_sha256"],
            first_object["trail_inventory_sha256"]
        );

        let mut bloom_payload = payload;
        bloom_payload["trail_bloom_profile"] = json!({
            "threshold":1.0,
            "radius_px":8,
            "intensity":4.0,
            "hdr_clamp":16.0,
            "source_gain":8.0
        });
        let bloom_request = WorkerRequest {
            operation: RENDER_TYPED_ANIMATED_SOCKET_TRAILS_BLOOM_OPERATION.to_owned(),
            ..animated_socket_trail_test_request(bloom_payload)
        };
        let bloom = render_worker_result(&bloom_request).expect("animated socket trail Bloom");
        let bloom_object = bloom.as_object().expect("trail Bloom result object");
        assert_eq!(
            bloom_object["schema_version"],
            ANIMATED_SOCKET_TRAIL_BLOOM_SCHEMA
        );
        assert_eq!(
            bloom_object["trail_bloom_passes"].as_array().unwrap().len(),
            5
        );
        for (base, bloom) in first_object["trail_passes"].as_array().unwrap().iter().zip(
            bloom_object["trail_bloom_passes"]
                .as_array()
                .unwrap()
                .iter(),
        ) {
            assert_eq!(base["png_base64"], bloom["png_base64"]);
        }
    }

    #[test]
    fn animated_socket_trails_reject_history_retarget_shape_and_trs_drift() {
        let glb = animated_socket_test_glb();
        let base = animated_socket_trail_test_payload(&glb);

        let mut unknown = base.clone();
        unknown["unknown"] = Value::Bool(true);
        assert!(render_worker_result(&animated_socket_trail_test_request(unknown)).is_err());

        let mut too_short = base.clone();
        too_short["projection_samples"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(render_worker_result(&animated_socket_trail_test_request(too_short)).is_err());

        let mut wrong_role = base.clone();
        wrong_role["projection_samples"][0]["emitters"][1]["role"] =
            Value::String("muzzle-vfx".to_owned());
        assert!(render_worker_result(&animated_socket_trail_test_request(wrong_role)).is_err());

        let mut wrong_owner = base.clone();
        wrong_owner["projection_samples"][1]["emitters"][0]["owner_part_id"] =
            Value::String("energy-core".to_owned());
        assert!(render_worker_result(&animated_socket_trail_test_request(wrong_owner)).is_err());

        let mut non_unit = base.clone();
        non_unit["projection_samples"][2]["emitters"][0]["composed_world_transform"]
            ["rotation_quat_xyzw"] = json!([0.0, 0.0, 0.0, 0.0]);
        assert!(render_worker_result(&animated_socket_trail_test_request(non_unit)).is_err());

        let mut wrong_source_id = base.clone();
        wrong_source_id["trails"][0]["local_points"][1]["source_particle_id"] =
            Value::from(10001_u64);
        assert!(
            render_worker_result(&animated_socket_trail_test_request(wrong_source_id)).is_err()
        );

        let mut retarget = base;
        retarget["projection_key_sha256"] = Value::String("f".repeat(64));
        let error = render_worker_result(&animated_socket_trail_test_request(retarget))
            .expect_err("projection retarget must invalidate trail seed");
        assert!(error.contains("seed_sha256"));
    }
}
