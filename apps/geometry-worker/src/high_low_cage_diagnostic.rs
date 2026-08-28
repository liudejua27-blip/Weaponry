//! Independent, bounded High/Low/Cage artifact production and diagnostics.
//!
//! This module is intentionally not a bake implementation.  It consumes three
//! already-produced, strict ForgeCAD GLBs, checks their semantic/topology
//! correspondence, and performs a deterministic two-sided ray probe.  The
//! result contains counts and hashes only: no mesh, PNG, normal map, texture,
//! or surface-bake field is produced.

use base64::Engine;
use forgecad_worker_protocol::{
    PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION,
    PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use crate::integrity::{self, DiagnosticMesh, GlbIntegrity};
use crate::GeometryError;

pub const REQUEST_SCHEMA_VERSION: &str = "ProductionWeaponHighLowCageDiagnosticRequest@1";
pub const RESULT_SCHEMA_VERSION: &str = "ProductionWeaponHighLowCageDiagnosticResult@1";
pub const PRODUCER_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowCageArtifactProducerRequest@1";
pub const PRODUCER_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponHighLowCageArtifactProducerResult@1";
pub const DIAGNOSTIC_POLICY: &str = "production-weapon-high-low-cage-ray-diagnostic@1";
pub const PRODUCER_POLICY: &str = "production-weapon-independent-high-low-cage-program-producer@1";
pub const BUDGET_PROFILE: &str = "fixture-high-low-cage-diagnostic@1";
pub const PRODUCER_BUDGET_PROFILE: &str = "source-high-low-cage-artifact-producer@1";
pub const RAY_SAMPLE_POLICY: &str = "cage-triangle-centroid-two-sided-same-part@1";
pub const BAKE_MODE: &str = "independent-high-low-cage-ray-bake@1";
pub const BAKE_POLICY: &str = "production-weapon-high-low-cage-ray-diagnostic-plan@1";
pub const HIGH_LOW_BAKE_STATUS: &str = "DIAGNOSTIC_ONLY";
pub const MAX_HIGH_TRIANGLES: usize = 8_000;
pub const MAX_LOW_CAGE_TRIANGLES: usize = 1_024;
pub const MAX_RAY_SAMPLES: usize = 2_048;
pub const MAX_DIAGNOSTIC_GLB_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DIAGNOSTIC_TOTAL_BYTES: usize = MAX_DIAGNOSTIC_GLB_BYTES * 3;
const MAX_PRODUCER_PROGRAM_BYTES: usize = 1024 * 1024;

const PRODUCER_REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "producer_policy",
    "producer_policy_sha256",
    "budget_profile",
    "max_high_triangles",
    "max_low_triangles",
    "max_cage_triangles",
    "max_glb_bytes",
    "high_geometry_program",
    "low_geometry_program",
    "cage_geometry_program",
    "surface_bake_reuse_allowed",
    "canonical_sha256",
];

const MAX_RAY_DISTANCE_M: f64 = 10.0;
const RAY_EPSILON_M: f64 = 1.0e-7;
const CAGE_CONTAINMENT_EPSILON_M: f32 = 1.0e-4;
const HISTOGRAM_BINS: usize = 8;
const NORMAL_ALIGNMENT_MIN: f32 = 0.25;

/// The request is deliberately closed.  The base64 fields are the only
/// accepted artifact transport; paths, URLs, scripts, material stacks and
/// caller-provided bake outputs are not part of this operation.
const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "diagnostic_policy",
    "diagnostic_policy_sha256",
    "budget_profile",
    "max_high_triangles",
    "max_low_triangles",
    "max_cage_triangles",
    "max_ray_samples",
    "max_ray_distance_m",
    "ray_sample_policy",
    "high_artifact_sha256",
    "low_artifact_sha256",
    "cage_artifact_sha256",
    "high_glb_base64",
    "low_glb_base64",
    "cage_glb_base64",
    "surface_bake_reuse_allowed",
    "canonical_sha256",
];

#[derive(Debug, Clone)]
struct Request {
    max_ray_distance_m: f64,
    high_artifact_sha256: String,
    low_artifact_sha256: String,
    cage_artifact_sha256: String,
    high_glb_base64: String,
    low_glb_base64: String,
    cage_glb_base64: String,
}

#[derive(Debug, Clone)]
struct ProducerRequest {
    high_geometry_program: Value,
    low_geometry_program: Value,
    cage_geometry_program: Value,
}

#[derive(Debug, Clone)]
struct ProducedArtifact {
    artifact: crate::GeometryArtifact,
    integrity: GlbIntegrity,
    mesh: DiagnosticMesh,
}

#[derive(Debug, Clone)]
struct Triangle {
    part_id: String,
    positions: [[f32; 3]; 3],
    normal: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
struct Hit {
    distance_m: f64,
    normal: [f32; 3],
}

#[derive(Debug, Clone)]
struct RaySummary {
    samples: usize,
    hits: usize,
    misses: usize,
    cross_part_hits: usize,
    backface_hits: usize,
    normal_mismatch_count: usize,
    skew_count: usize,
    max_distance_m: f64,
    histogram: [u64; HISTOGRAM_BINS],
}

impl Default for RaySummary {
    fn default() -> Self {
        Self {
            samples: 0,
            hits: 0,
            misses: 0,
            cross_part_hits: 0,
            backface_hits: 0,
            normal_mismatch_count: 0,
            skew_count: 0,
            max_distance_m: 0.0,
            histogram: [0; HISTOGRAM_BINS],
        }
    }
}

pub fn diagnose(payload: &Map<String, Value>) -> Result<Value, GeometryError> {
    require_closed_payload(payload)?;
    let request = Request::parse(payload)?;
    let (high_bytes, low_bytes, cage_bytes) = request.decode_glbs()?;
    let total_bytes = high_bytes
        .len()
        .checked_add(low_bytes.len())
        .and_then(|value| value.checked_add(cage_bytes.len()))
        .ok_or_else(|| GeometryError::Invalid("diagnostic GLB byte count overflows".to_owned()))?;
    if total_bytes > MAX_DIAGNOSTIC_TOTAL_BYTES {
        return Err(GeometryError::Invalid(
            "high/low/cage diagnostic input exceeds the bounded total byte budget".to_owned(),
        ));
    }

    // Strict readback is deliberately performed independently for all three
    // artifacts.  No one artifact is treated as a source or fallback for the
    // others, and no CandidateSurfaceBake path is reachable from this module.
    let high_integrity = strict_integrity(&high_bytes, "high", MAX_HIGH_TRIANGLES)?;
    let low_integrity = strict_integrity(&low_bytes, "low", MAX_LOW_CAGE_TRIANGLES)?;
    let cage_integrity = strict_integrity(&cage_bytes, "cage", MAX_LOW_CAGE_TRIANGLES)?;
    let high = integrity::extract_diagnostic_mesh(&high_bytes, MAX_HIGH_TRIANGLES)?;
    let low = integrity::extract_diagnostic_mesh(&low_bytes, MAX_LOW_CAGE_TRIANGLES)?;
    let cage = integrity::extract_diagnostic_mesh(&cage_bytes, MAX_LOW_CAGE_TRIANGLES)?;

    if high.triangle_count != high_integrity.triangle_count as usize
        || low.triangle_count != low_integrity.triangle_count as usize
        || cage.triangle_count != cage_integrity.triangle_count as usize
    {
        return Err(GeometryError::Invalid(
            "diagnostic triangle count disagrees with strict GLB readback".to_owned(),
        ));
    }
    let part_pairs = compare_low_cage(&low, &cage)?;
    compare_high_parts(&high, &low, &cage)?;
    let high_by_part = high_triangles(&high);
    let low_triangles = diagnostic_triangles(&low);
    let cage_triangles = diagnostic_triangles(&cage);
    let containment = cage_containment(&low, &cage);
    let ray = trace_rays(
        &low_triangles,
        &cage_triangles,
        &high_by_part,
        request.max_ray_distance_m,
    )?;

    if ray.samples > MAX_RAY_SAMPLES {
        return Err(GeometryError::Invalid(
            "diagnostic ray sample budget exceeded".to_owned(),
        ));
    }
    let distance_histogram = Value::Array(
        ray.histogram
            .iter()
            .map(|value| Value::from(*value))
            .collect(),
    );
    let distance_histogram_sha256 = crate::canonical_hash(&distance_histogram);
    let part_pairs_value = Value::Array(part_pairs);
    let part_pairs_sha256 = crate::canonical_hash(&part_pairs_value);
    let heatmap_preimage = json!({
        "part_pairs_sha256":part_pairs_sha256,
        "distance_histogram_sha256":distance_histogram_sha256,
        "ray_sample_count":ray.samples,
        "ray_hit_count":ray.hits,
        "ray_miss_count":ray.misses,
        "cross_part_hit_count":ray.cross_part_hits,
        "skew_count":ray.skew_count,
        "cage_intersection_count":containment.intersection_count,
        "overlap_count":containment.overlap_count,
        "out_of_range_count":containment.out_of_range_count
    });
    let heatmap_sha256 = crate::canonical_hash(&heatmap_preimage);
    let diagnostic_status = if ray.misses == 0
        && ray.cross_part_hits == 0
        && ray.skew_count == 0
        && containment.intersection_count == 0
        && containment.overlap_count == 0
        && containment.out_of_range_count == 0
    {
        "PASS_SOURCE_STRUCTURAL"
    } else {
        "FAILED"
    };
    let mut result = json!({
        "schema_version":RESULT_SCHEMA_VERSION,
        "operation":PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION,
        "diagnostic_policy":DIAGNOSTIC_POLICY,
        "diagnostic_policy_sha256":sha256_hex(DIAGNOSTIC_POLICY.as_bytes()),
        "budget_profile":BUDGET_PROFILE,
        "bake_mode":BAKE_MODE,
        "bake_policy":BAKE_POLICY,
        "normal_convention":"OpenGL+Y",
        "ray_origin_policy":"cage-triangle-centroid-plus-epsilon@1",
        "ray_direction_policy":"cage-face-normal-front-and-back@1",
        "ray_distance_policy":"bounded-positive-nearest-hit@1",
        "front_back_policy":"two-sided-front-back@1",
        "per_part_isolation_policy":"same-semantic-part-only@1",
        "anti_cross_hit_policy":"reject-nearer-foreign-part@1",
        "max_high_triangles":MAX_HIGH_TRIANGLES,
        "max_low_triangles":MAX_LOW_CAGE_TRIANGLES,
        "max_cage_triangles":MAX_LOW_CAGE_TRIANGLES,
        "max_ray_samples":MAX_RAY_SAMPLES,
        "max_ray_distance_m":request.max_ray_distance_m,
        "ray_sample_policy":RAY_SAMPLE_POLICY,
        "high_artifact_sha256":request.high_artifact_sha256,
        "low_artifact_sha256":request.low_artifact_sha256,
        "cage_artifact_sha256":request.cage_artifact_sha256,
        "high_triangle_count":high.triangle_count,
        "low_triangle_count":low.triangle_count,
        "cage_triangle_count":cage.triangle_count,
        "part_count":low.primitives.len(),
        "part_ids":low.primitives.iter().map(|primitive| Value::String(primitive.part_id.clone())).collect::<Vec<_>>(),
        "part_pairs":part_pairs_value,
        "part_pairs_sha256":part_pairs_sha256,
        "ray_sample_count":ray.samples,
        "ray_hit_count":ray.hits,
        "ray_miss_count":ray.misses,
        "cross_part_hit_count":ray.cross_part_hits,
        "backface_hit_count":ray.backface_hits,
        "normal_mismatch_count":ray.normal_mismatch_count,
        "skew_count":ray.skew_count,
        "cage_intersection_count":containment.intersection_count,
        "cage_containment_violation_count":containment.intersection_count,
        "overlap_count":containment.overlap_count,
        "out_of_range_count":containment.out_of_range_count,
        "max_observed_distance_m":ray.max_distance_m,
        "distance_histogram_sha256":distance_histogram_sha256,
        "distance_histogram_object_sha256":distance_histogram_sha256,
        "distance_histogram_canonical_sha256":distance_histogram_sha256,
        "diagnostic_heatmap_sha256":heatmap_sha256,
        "diagnostic_heatmap_object_sha256":heatmap_sha256,
        "diagnostic_heatmap_canonical_sha256":heatmap_sha256,
        "validator_status":if diagnostic_status == "PASS_SOURCE_STRUCTURAL" {"passed"} else {"failed"},
        "hard_gate_passed":diagnostic_status == "PASS_SOURCE_STRUCTURAL",
        "diagnostic_status":diagnostic_status,
        "mapping_status":if diagnostic_status == "PASS_SOURCE_STRUCTURAL" {"PASS_SOURCE_STRUCTURAL"} else {"FAILED"},
        "correspondence_status":if diagnostic_status == "PASS_SOURCE_STRUCTURAL" {"PASS_SOURCE_STRUCTURAL"} else {"FAILED"},
        "high_low_bake_status":HIGH_LOW_BAKE_STATUS,
        "surface_bake_reuse_allowed":false,
        "raw_media_emitted":false,
        "bake_output_object_sha256s":[],
        "runtime_write_performed":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(crate::canonical_hash(&result));
    Ok(result)
}

/// Produce three independent, typed-program GLBs and immediately run the
/// bounded correspondence diagnostic over their strict readbacks.  This is a
/// source-only producer: it does not derive retopology or a cage from another
/// artifact, does not access CAS, and does not emit bake maps.  Callers must
/// provide three distinct GeometryProgram@2 inputs, so the Worker never
/// disguises a copied candidate as an independent production artifact.
pub fn produce(payload: &Map<String, Value>) -> Result<Value, GeometryError> {
    require_closed_producer_payload(payload)?;
    let request = ProducerRequest::parse(payload)?;
    let first = compile_producer_set(&request)?;
    let second = compile_producer_set(&request)?;
    ensure_replay_byte_exact(&first, &second)?;

    let first_diagnostic_payload = diagnostic_payload_for_artifacts(
        &first[0].artifact.glb,
        &first[1].artifact.glb,
        &first[2].artifact.glb,
    );
    let second_diagnostic_payload = diagnostic_payload_for_artifacts(
        &second[0].artifact.glb,
        &second[1].artifact.glb,
        &second[2].artifact.glb,
    );
    let first_diagnostic = diagnose(
        first_diagnostic_payload
            .as_object()
            .expect("diagnostic payload object"),
    )?;
    let second_diagnostic = diagnose(
        second_diagnostic_payload
            .as_object()
            .expect("diagnostic payload object"),
    )?;
    if first_diagnostic != second_diagnostic {
        return Err(GeometryError::Invalid(
            "high/low/cage diagnostic replay is not byte-deterministic".to_owned(),
        ));
    }

    let total_bytes = first
        .iter()
        .map(|produced| produced.artifact.glb.len())
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(|| GeometryError::Invalid("producer output byte count overflows".to_owned()))?;
    if total_bytes > MAX_DIAGNOSTIC_TOTAL_BYTES {
        return Err(GeometryError::Invalid(
            "high/low/cage producer output exceeds the bounded total byte budget".to_owned(),
        ));
    }

    let high = produced_artifact_value("high", &first[0]);
    let low = produced_artifact_value("low", &first[1]);
    let cage = produced_artifact_value("cage", &first[2]);
    let mut result = json!({
        "schema_version":PRODUCER_RESULT_SCHEMA_VERSION,
        "operation":PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION,
        "producer_policy":PRODUCER_POLICY,
        "producer_policy_sha256":sha256_hex(PRODUCER_POLICY.as_bytes()),
        "budget_profile":PRODUCER_BUDGET_PROFILE,
        "generated_glb_count":3,
        "distinct_artifact_bindings":true,
        "artifact_semantics":"independent-typed-programs-only@1",
        "retopology_derived":false,
        "cage_offset_field_derived":false,
        "limitations":[
            "LOW_IS_NOT_DERIVED_RETOPOLOGY_FROM_HIGH",
            "CAGE_IS_NOT_DERIVED_OFFSET_FIELD_FROM_LOW",
            "NO_UV_OR_FORMAL_BAKE_MAP_OUTPUT"
        ],
        "high":high,
        "low":low,
        "cage":cage,
        "diagnostic":first_diagnostic,
        "diagnostic_status":first_diagnostic["diagnostic_status"],
        "high_low_bake_status":HIGH_LOW_BAKE_STATUS,
        "surface_bake_reuse_allowed":false,
        "formal_bake_performed":false,
        "png_emitted":false,
        "worker_replay_count":2,
        "replay_byte_exact":true,
        "runtime_write_performed":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(crate::canonical_hash(&result));
    Ok(result)
}

fn require_closed_producer_payload(payload: &Map<String, Value>) -> Result<(), GeometryError> {
    if payload.len() != PRODUCER_REQUEST_FIELDS.len()
        || payload
            .keys()
            .any(|key| !PRODUCER_REQUEST_FIELDS.contains(&key.as_str()))
    {
        return Err(GeometryError::Invalid(
            "high/low/cage producer payload is not the closed request shape".to_owned(),
        ));
    }
    Ok(())
}

impl ProducerRequest {
    fn parse(payload: &Map<String, Value>) -> Result<Self, GeometryError> {
        expect_string(payload, "schema_version", PRODUCER_REQUEST_SCHEMA_VERSION)?;
        expect_string(payload, "producer_policy", PRODUCER_POLICY)?;
        expect_string(payload, "budget_profile", PRODUCER_BUDGET_PROFILE)?;
        let policy_hash = expect_sha256(payload, "producer_policy_sha256")?;
        if policy_hash != sha256_hex(PRODUCER_POLICY.as_bytes()) {
            return Err(GeometryError::Invalid(
                "producer_policy_sha256 does not match the fixed policy".to_owned(),
            ));
        }
        for (field, expected) in [
            ("max_high_triangles", MAX_HIGH_TRIANGLES),
            ("max_low_triangles", MAX_LOW_CAGE_TRIANGLES),
            ("max_cage_triangles", MAX_LOW_CAGE_TRIANGLES),
            ("max_glb_bytes", MAX_DIAGNOSTIC_GLB_BYTES),
        ] {
            let actual = payload
                .get(field)
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| GeometryError::Invalid(format!("{field} is invalid")))?;
            if actual != expected {
                return Err(GeometryError::Invalid(format!(
                    "{field} does not match the fixed producer budget"
                )));
            }
        }
        if payload.get("surface_bake_reuse_allowed") != Some(&Value::Bool(false)) {
            return Err(GeometryError::Invalid(
                "surface_bake_reuse_allowed must be false".to_owned(),
            ));
        }
        let high_geometry_program = payload
            .get("high_geometry_program")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or_else(|| {
                GeometryError::Invalid("high_geometry_program is required".to_owned())
            })?;
        let low_geometry_program = payload
            .get("low_geometry_program")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or_else(|| GeometryError::Invalid("low_geometry_program is required".to_owned()))?;
        let cage_geometry_program = payload
            .get("cage_geometry_program")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or_else(|| {
                GeometryError::Invalid("cage_geometry_program is required".to_owned())
            })?;
        for (label, program) in [
            ("high", &high_geometry_program),
            ("low", &low_geometry_program),
            ("cage", &cage_geometry_program),
        ] {
            if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2") {
                return Err(GeometryError::Invalid(format!(
                    "{label}_geometry_program must be GeometryProgram@2"
                )));
            }
            let program_bytes = serde_json::to_vec(program).map_err(|_| {
                GeometryError::Invalid(format!("{label}_geometry_program cannot be encoded"))
            })?;
            if program_bytes.len() > MAX_PRODUCER_PROGRAM_BYTES {
                return Err(GeometryError::Invalid(format!(
                    "{label}_geometry_program exceeds the bounded producer input"
                )));
            }
        }
        let program_hashes = [
            program_hash(&high_geometry_program)?,
            program_hash(&low_geometry_program)?,
            program_hash(&cage_geometry_program)?,
        ];
        if program_hashes[0] == program_hashes[1]
            || program_hashes[0] == program_hashes[2]
            || program_hashes[1] == program_hashes[2]
        {
            return Err(GeometryError::Invalid(
                "high, low and cage GeometryProgram hashes must be distinct".to_owned(),
            ));
        }
        let canonical_sha256 = expect_sha256(payload, "canonical_sha256")?;
        let mut without_hash = payload.clone();
        without_hash.remove("canonical_sha256");
        if canonical_sha256 != crate::canonical_hash(&Value::Object(without_hash)) {
            return Err(GeometryError::Invalid(
                "producer request canonical_sha256 does not match".to_owned(),
            ));
        }
        Ok(Self {
            high_geometry_program,
            low_geometry_program,
            cage_geometry_program,
        })
    }
}

fn program_hash(program: &Value) -> Result<String, GeometryError> {
    let hash = program
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            GeometryError::Invalid("GeometryProgram canonical_sha256 is invalid".to_owned())
        })?;
    let mut without_hash = program.as_object().ok_or(GeometryError::NotObject)?.clone();
    without_hash.remove("canonical_sha256");
    if hash != crate::canonical_hash(&Value::Object(without_hash)) {
        return Err(GeometryError::Invalid(
            "GeometryProgram canonical_sha256 does not match".to_owned(),
        ));
    }
    Ok(hash.to_owned())
}

fn compile_producer_set(request: &ProducerRequest) -> Result<[ProducedArtifact; 3], GeometryError> {
    let high =
        compile_produced_artifact("high", &request.high_geometry_program, MAX_HIGH_TRIANGLES)?;
    let low =
        compile_produced_artifact("low", &request.low_geometry_program, MAX_LOW_CAGE_TRIANGLES)?;
    let cage = compile_produced_artifact(
        "cage",
        &request.cage_geometry_program,
        MAX_LOW_CAGE_TRIANGLES,
    )?;
    let hashes = [
        sha256_hex(&high.artifact.glb),
        sha256_hex(&low.artifact.glb),
        sha256_hex(&cage.artifact.glb),
    ];
    if hashes[0] == hashes[1] || hashes[0] == hashes[2] || hashes[1] == hashes[2] {
        return Err(GeometryError::Invalid(
            "high, low and cage producer outputs must be distinct GLBs".to_owned(),
        ));
    }
    Ok([high, low, cage])
}

fn compile_produced_artifact(
    label: &str,
    program: &Value,
    max_triangles: usize,
) -> Result<ProducedArtifact, GeometryError> {
    let artifact = crate::compile_geometry_program(program)?;
    if artifact.glb.len() > MAX_DIAGNOSTIC_GLB_BYTES
        || artifact.triangle_count == 0
        || artifact.triangle_count as usize > max_triangles
    {
        return Err(GeometryError::Invalid(format!(
            "{label} producer artifact exceeds its fixed budget"
        )));
    }
    let integrity = strict_integrity(&artifact.glb, label, max_triangles)?;
    if integrity.program_sha256 != artifact.program_sha256
        || integrity.triangle_count != artifact.triangle_count
    {
        return Err(GeometryError::Invalid(format!(
            "{label} producer strict readback does not match the compiled artifact"
        )));
    }
    let mesh = integrity::extract_diagnostic_mesh(&artifact.glb, max_triangles)?;
    if mesh.triangle_count != artifact.triangle_count as usize {
        return Err(GeometryError::Invalid(format!(
            "{label} producer diagnostic mesh count does not match the compiled artifact"
        )));
    }
    Ok(ProducedArtifact {
        artifact,
        integrity,
        mesh,
    })
}

fn ensure_replay_byte_exact(
    first: &[ProducedArtifact; 3],
    second: &[ProducedArtifact; 3],
) -> Result<(), GeometryError> {
    if first
        .iter()
        .zip(second)
        .any(|(left, right)| left.artifact.glb != right.artifact.glb)
    {
        return Err(GeometryError::Invalid(
            "high/low/cage producer replay is not byte-exact".to_owned(),
        ));
    }
    Ok(())
}

fn produced_artifact_value(label: &str, produced: &ProducedArtifact) -> Value {
    let artifact_hash = sha256_hex(&produced.artifact.glb);
    let readback_hash = crate::canonical_hash(&json!({
        "artifact_sha256":artifact_hash,
        "schema_version":produced.integrity.artifact_schema_version,
        "triangle_count":produced.integrity.triangle_count,
        "validator_status":produced.integrity.validator_status,
        "hard_gate_passed":produced.integrity.hard_gate_passed,
    }));
    json!({
        "artifact_kind":format!("production-weapon-{label}-artifact-glb"),
        "mime":"model/gltf-binary",
        "glb_base64":base64::engine::general_purpose::STANDARD.encode(&produced.artifact.glb),
        "artifact_sha256":artifact_hash,
        "artifact_readback_sha256":readback_hash,
        "readback_sha256":readback_hash,
        "program_sha256":produced.artifact.program_sha256,
        "triangle_count":produced.artifact.triangle_count,
        "size_bytes":produced.artifact.glb.len(),
        "part_ids":produced.artifact.part_ids,
        "material_zone_ids":produced.artifact.material_zone_ids,
        "validator_status":produced.integrity.validator_status,
        "hard_gate_passed":produced.integrity.hard_gate_passed,
        "diagnostic_mesh_triangle_count":produced.mesh.triangle_count,
    })
}

fn require_closed_payload(payload: &Map<String, Value>) -> Result<(), GeometryError> {
    if payload.len() != REQUEST_FIELDS.len()
        || payload
            .keys()
            .any(|key| !REQUEST_FIELDS.contains(&key.as_str()))
    {
        return Err(GeometryError::Invalid(
            "high/low/cage diagnostic payload is not the closed request shape".to_owned(),
        ));
    }
    Ok(())
}

impl Request {
    fn parse(payload: &Map<String, Value>) -> Result<Self, GeometryError> {
        expect_string(payload, "schema_version", REQUEST_SCHEMA_VERSION)?;
        expect_string(payload, "diagnostic_policy", DIAGNOSTIC_POLICY)?;
        expect_string(payload, "budget_profile", BUDGET_PROFILE)?;
        expect_string(payload, "ray_sample_policy", RAY_SAMPLE_POLICY)?;
        let policy_hash = expect_sha256(payload, "diagnostic_policy_sha256")?;
        if policy_hash != sha256_hex(DIAGNOSTIC_POLICY.as_bytes()) {
            return Err(GeometryError::Invalid(
                "diagnostic_policy_sha256 does not match the fixed policy".to_owned(),
            ));
        }
        for (field, expected) in [
            ("max_high_triangles", MAX_HIGH_TRIANGLES),
            ("max_low_triangles", MAX_LOW_CAGE_TRIANGLES),
            ("max_cage_triangles", MAX_LOW_CAGE_TRIANGLES),
            ("max_ray_samples", MAX_RAY_SAMPLES),
        ] {
            let actual = payload
                .get(field)
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| GeometryError::Invalid(format!("{field} is invalid")))?;
            if actual != expected {
                return Err(GeometryError::Invalid(format!(
                    "{field} does not match the fixed diagnostic budget"
                )));
            }
        }
        let max_ray_distance_m = payload
            .get("max_ray_distance_m")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value > 0.0 && *value <= MAX_RAY_DISTANCE_M)
            .ok_or_else(|| GeometryError::Invalid("max_ray_distance_m is invalid".to_owned()))?;
        if (max_ray_distance_m - MAX_RAY_DISTANCE_M).abs() > f64::EPSILON {
            return Err(GeometryError::Invalid(
                "max_ray_distance_m does not match the fixed diagnostic budget".to_owned(),
            ));
        }
        if payload.get("surface_bake_reuse_allowed") != Some(&Value::Bool(false)) {
            return Err(GeometryError::Invalid(
                "surface_bake_reuse_allowed must be false".to_owned(),
            ));
        }
        let high_artifact_sha256 = expect_sha256(payload, "high_artifact_sha256")?.to_owned();
        let low_artifact_sha256 = expect_sha256(payload, "low_artifact_sha256")?.to_owned();
        let cage_artifact_sha256 = expect_sha256(payload, "cage_artifact_sha256")?.to_owned();
        if high_artifact_sha256 == low_artifact_sha256
            || high_artifact_sha256 == cage_artifact_sha256
            || low_artifact_sha256 == cage_artifact_sha256
        {
            return Err(GeometryError::Invalid(
                "high, low and cage artifact hashes must be distinct".to_owned(),
            ));
        }
        let high_glb_base64 = expect_base64(payload, "high_glb_base64")?.to_owned();
        let low_glb_base64 = expect_base64(payload, "low_glb_base64")?.to_owned();
        let cage_glb_base64 = expect_base64(payload, "cage_glb_base64")?.to_owned();
        let canonical_sha256 = expect_sha256(payload, "canonical_sha256")?;
        let mut without_hash = payload.clone();
        without_hash.remove("canonical_sha256");
        if canonical_sha256 != crate::canonical_hash(&Value::Object(without_hash)) {
            return Err(GeometryError::Invalid(
                "diagnostic request canonical_sha256 does not match".to_owned(),
            ));
        }
        Ok(Self {
            max_ray_distance_m,
            high_artifact_sha256,
            low_artifact_sha256,
            cage_artifact_sha256,
            high_glb_base64,
            low_glb_base64,
            cage_glb_base64,
        })
    }

    fn decode_glbs(&self) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), GeometryError> {
        let high = decode_glb(&self.high_glb_base64, "high")?;
        let low = decode_glb(&self.low_glb_base64, "low")?;
        let cage = decode_glb(&self.cage_glb_base64, "cage")?;
        if sha256_hex(&high) != self.high_artifact_sha256
            || sha256_hex(&low) != self.low_artifact_sha256
            || sha256_hex(&cage) != self.cage_artifact_sha256
        {
            return Err(GeometryError::Invalid(
                "diagnostic GLB bytes do not match the declared artifact hashes".to_owned(),
            ));
        }
        Ok((high, low, cage))
    }
}

fn expect_string<'a>(
    payload: &'a Map<String, Value>,
    key: &str,
    expected: &str,
) -> Result<&'a str, GeometryError> {
    let value = payload
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| GeometryError::Invalid(format!("{key} is required")))?;
    if value != expected {
        return Err(GeometryError::Invalid(format!(
            "{key} is not the fixed value"
        )));
    }
    Ok(value)
}

fn expect_sha256<'a>(payload: &'a Map<String, Value>, key: &str) -> Result<&'a str, GeometryError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| GeometryError::Invalid(format!("{key} is not a SHA-256")))
}

fn expect_base64<'a>(payload: &'a Map<String, Value>, key: &str) -> Result<&'a str, GeometryError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| GeometryError::Invalid(format!("{key} is required")))
}

fn decode_glb(value: &str, label: &str) -> Result<Vec<u8>, GeometryError> {
    let encoded_limit = MAX_DIAGNOSTIC_GLB_BYTES
        .checked_mul(4)
        .and_then(|value| value.checked_div(3))
        .and_then(|value| value.checked_add(8))
        .ok_or_else(|| GeometryError::Invalid("diagnostic base64 budget overflows".to_owned()))?;
    if value.len() > encoded_limit {
        return Err(GeometryError::Invalid(format!(
            "{label} GLB base64 exceeds the bounded input size"
        )));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| GeometryError::Invalid(format!("{label} GLB base64 is invalid")))?;
    if bytes.is_empty() || bytes.len() > MAX_DIAGNOSTIC_GLB_BYTES {
        return Err(GeometryError::Invalid(format!(
            "{label} GLB exceeds the bounded input size"
        )));
    }
    Ok(bytes)
}

fn diagnostic_payload_for_artifacts(high: &[u8], low: &[u8], cage: &[u8]) -> Value {
    let mut payload = json!({
        "schema_version":REQUEST_SCHEMA_VERSION,
        "diagnostic_policy":DIAGNOSTIC_POLICY,
        "diagnostic_policy_sha256":sha256_hex(DIAGNOSTIC_POLICY.as_bytes()),
        "budget_profile":BUDGET_PROFILE,
        "max_high_triangles":MAX_HIGH_TRIANGLES,
        "max_low_triangles":MAX_LOW_CAGE_TRIANGLES,
        "max_cage_triangles":MAX_LOW_CAGE_TRIANGLES,
        "max_ray_samples":MAX_RAY_SAMPLES,
        "max_ray_distance_m":MAX_RAY_DISTANCE_M,
        "ray_sample_policy":RAY_SAMPLE_POLICY,
        "high_artifact_sha256":sha256_hex(high),
        "low_artifact_sha256":sha256_hex(low),
        "cage_artifact_sha256":sha256_hex(cage),
        "high_glb_base64":base64::engine::general_purpose::STANDARD.encode(high),
        "low_glb_base64":base64::engine::general_purpose::STANDARD.encode(low),
        "cage_glb_base64":base64::engine::general_purpose::STANDARD.encode(cage),
        "surface_bake_reuse_allowed":false,
        "canonical_sha256":""
    });
    payload["canonical_sha256"] =
        Value::String(crate::canonical_hash(&payload_without_hash(&payload)));
    payload
}

fn payload_without_hash(value: &Value) -> Value {
    let mut object = value.as_object().expect("hash preimage object").clone();
    object.remove("canonical_sha256");
    Value::Object(object)
}

fn strict_integrity(
    bytes: &[u8],
    label: &str,
    max_triangles: usize,
) -> Result<GlbIntegrity, GeometryError> {
    let integrity = integrity::inspect_glb(bytes)?;
    if integrity.artifact_schema_version != "ArtifactReadback@2"
        || !integrity.hard_gate_passed
        || integrity.validator_status != "passed"
        || integrity.external_uri_count != 0
        || integrity.triangle_count == 0
        || integrity.triangle_count as usize > max_triangles
    {
        return Err(GeometryError::Invalid(format!(
            "{label} GLB is not a strict bounded ArtifactReadback@2"
        )));
    }
    Ok(integrity)
}

fn compare_high_parts(
    high: &DiagnosticMesh,
    low: &DiagnosticMesh,
    cage: &DiagnosticMesh,
) -> Result<(), GeometryError> {
    let low_parts = part_materials(low);
    let cage_parts = part_materials(cage);
    let high_parts = part_materials(high);
    if low_parts != cage_parts || low_parts != high_parts {
        return Err(GeometryError::Invalid(
            "PART_CORRESPONDENCE_MISMATCH: high/low/cage semantic Part or material-zone sets differ"
                .to_owned(),
        ));
    }
    Ok(())
}

fn part_materials(mesh: &DiagnosticMesh) -> BTreeMap<String, BTreeSet<String>> {
    let mut result = BTreeMap::new();
    for primitive in &mesh.primitives {
        result
            .entry(primitive.part_id.clone())
            .or_insert_with(BTreeSet::new)
            .insert(primitive.material_zone_id.clone());
    }
    result
}

fn compare_low_cage(
    low: &DiagnosticMesh,
    cage: &DiagnosticMesh,
) -> Result<Vec<Value>, GeometryError> {
    if low.primitives.len() != cage.primitives.len() || low.triangle_count != cage.triangle_count {
        return Err(GeometryError::Invalid(
            "CAGE_TOPOLOGY_MISMATCH: low/cage primitive or triangle count differs".to_owned(),
        ));
    }
    let mut pairs = Vec::with_capacity(low.primitives.len());
    for (ordinal, (low_primitive, cage_primitive)) in
        low.primitives.iter().zip(&cage.primitives).enumerate()
    {
        if low_primitive.part_id != cage_primitive.part_id
            || low_primitive.source_node_id != cage_primitive.source_node_id
            || low_primitive.material_zone_id != cage_primitive.material_zone_id
            || low_primitive.solid != cage_primitive.solid
            || low_primitive.positions.len() != cage_primitive.positions.len()
            || low_primitive.indices != cage_primitive.indices
        {
            return Err(GeometryError::Invalid(
                "CAGE_TOPOLOGY_MISMATCH: low/cage Part order or index topology differs".to_owned(),
            ));
        }
        let max_position_delta_m = low_primitive
            .positions
            .iter()
            .zip(&cage_primitive.positions)
            .map(|(low, cage)| distance3(*low, *cage))
            .fold(0.0_f32, f32::max);
        if !max_position_delta_m.is_finite() {
            return Err(GeometryError::Invalid(
                "CAGE_TOPOLOGY_MISMATCH: non-finite cage displacement".to_owned(),
            ));
        }
        let topology_preimage = json!({
            "part_id":low_primitive.part_id,
            "source_node_id":low_primitive.source_node_id,
            "material_zone_id":low_primitive.material_zone_id,
            "solid":low_primitive.solid,
            "vertex_count":low_primitive.positions.len(),
            "indices":low_primitive.indices,
        });
        pairs.push(json!({
            "ordinal":ordinal,
            "part_id":low_primitive.part_id,
            "source_node_id":low_primitive.source_node_id,
            "triangle_count":low_primitive.indices.len() / 3,
            "vertex_count":low_primitive.positions.len(),
            "topology_sha256":crate::canonical_hash(&topology_preimage),
            "max_position_delta_m":f64::from(max_position_delta_m),
        }));
    }
    Ok(pairs)
}

fn diagnostic_triangles(mesh: &DiagnosticMesh) -> Vec<Triangle> {
    let mut triangles = Vec::with_capacity(mesh.triangle_count);
    for primitive in &mesh.primitives {
        for indices in primitive.indices.chunks_exact(3) {
            let positions = [
                primitive.positions[indices[0] as usize],
                primitive.positions[indices[1] as usize],
                primitive.positions[indices[2] as usize],
            ];
            let normal = normalize3(cross3(
                sub3(positions[1], positions[0]),
                sub3(positions[2], positions[0]),
            ));
            triangles.push(Triangle {
                part_id: primitive.part_id.clone(),
                positions,
                normal,
            });
        }
    }
    triangles
}

fn high_triangles(mesh: &DiagnosticMesh) -> BTreeMap<String, Vec<Triangle>> {
    let mut by_part = BTreeMap::new();
    for triangle in diagnostic_triangles(mesh) {
        by_part
            .entry(triangle.part_id.clone())
            .or_insert_with(Vec::new)
            .push(triangle);
    }
    by_part
}

fn trace_rays(
    low: &[Triangle],
    cage: &[Triangle],
    high_by_part: &BTreeMap<String, Vec<Triangle>>,
    max_distance_m: f64,
) -> Result<RaySummary, GeometryError> {
    if low.len() != cage.len() {
        return Err(GeometryError::Invalid(
            "CAGE_TOPOLOGY_MISMATCH: low/cage triangle order differs".to_owned(),
        ));
    }
    let mut summary = RaySummary::default();
    for (low_triangle, cage_triangle) in low.iter().zip(cage) {
        if low_triangle.part_id != cage_triangle.part_id {
            return Err(GeometryError::Invalid(
                "CAGE_TOPOLOGY_MISMATCH: low/cage triangle Part differs".to_owned(),
            ));
        }
        if dot3(low_triangle.normal, cage_triangle.normal).abs() < NORMAL_ALIGNMENT_MIN {
            summary.skew_count += 1;
        }
        let directions = [cage_triangle.normal, negate3(cage_triangle.normal)];
        let centroid = centroid(cage_triangle.positions);
        for direction_f32 in directions {
            summary.samples += 1;
            if summary.samples > MAX_RAY_SAMPLES {
                return Err(GeometryError::Invalid(
                    "diagnostic ray sample budget exceeded".to_owned(),
                ));
            }
            let direction = [
                f64::from(direction_f32[0]),
                f64::from(direction_f32[1]),
                f64::from(direction_f32[2]),
            ];
            let origin = [
                f64::from(centroid[0]) + direction[0] * RAY_EPSILON_M,
                f64::from(centroid[1]) + direction[1] * RAY_EPSILON_M,
                f64::from(centroid[2]) + direction[2] * RAY_EPSILON_M,
            ];
            let same = high_by_part
                .get(&low_triangle.part_id)
                .and_then(|triangles| nearest_hit(origin, direction, triangles, max_distance_m));
            let foreign = high_by_part
                .iter()
                .filter(|(part_id, _)| *part_id != &low_triangle.part_id)
                .filter_map(|(_, triangles)| {
                    nearest_hit(origin, direction, triangles, max_distance_m)
                })
                .min_by(|left, right| left.distance_m.total_cmp(&right.distance_m));
            if foreign.is_some_and(|foreign| {
                same.is_none_or(|same| foreign.distance_m <= same.distance_m + RAY_EPSILON_M)
            }) {
                summary.cross_part_hits += 1;
                continue;
            }
            if let Some(hit) = same {
                summary.hits += 1;
                summary.max_distance_m = summary.max_distance_m.max(hit.distance_m);
                let bin = ((hit.distance_m / max_distance_m) * HISTOGRAM_BINS as f64)
                    .floor()
                    .clamp(0.0, (HISTOGRAM_BINS - 1) as f64) as usize;
                summary.histogram[bin] += 1;
                if dot3(hit.normal, direction_f32) >= 0.0 {
                    summary.backface_hits += 1;
                }
                if dot3(hit.normal, low_triangle.normal).abs() < NORMAL_ALIGNMENT_MIN {
                    summary.normal_mismatch_count += 1;
                }
            } else {
                summary.misses += 1;
            }
        }
    }
    Ok(summary)
}

fn nearest_hit(
    origin: [f64; 3],
    direction: [f64; 3],
    triangles: &[Triangle],
    max_distance_m: f64,
) -> Option<Hit> {
    triangles
        .iter()
        .filter_map(|triangle| ray_triangle(origin, direction, triangle))
        .filter(|hit| hit.distance_m <= max_distance_m)
        .min_by(|left, right| left.distance_m.total_cmp(&right.distance_m))
}

fn ray_triangle(origin: [f64; 3], direction: [f64; 3], triangle: &Triangle) -> Option<Hit> {
    let a = to_f64(triangle.positions[0]);
    let b = to_f64(triangle.positions[1]);
    let c = to_f64(triangle.positions[2]);
    let edge_1 = sub3_f64(b, a);
    let edge_2 = sub3_f64(c, a);
    let p = cross3_f64(direction, edge_2);
    let determinant = dot3_f64(edge_1, p);
    if determinant.abs() <= RAY_EPSILON_M {
        return None;
    }
    let inverse = 1.0 / determinant;
    let tvec = sub3_f64(origin, a);
    let u = dot3_f64(tvec, p) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross3_f64(tvec, edge_1);
    let v = dot3_f64(direction, q) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance_m = dot3_f64(edge_2, q) * inverse;
    if !distance_m.is_finite() || distance_m <= RAY_EPSILON_M {
        return None;
    }
    Some(Hit {
        distance_m,
        normal: triangle.normal,
    })
}

#[derive(Debug, Clone, Copy, Default)]
struct ContainmentSummary {
    intersection_count: usize,
    overlap_count: usize,
    out_of_range_count: usize,
}

fn cage_containment(low: &DiagnosticMesh, cage: &DiagnosticMesh) -> ContainmentSummary {
    let mut summary = ContainmentSummary::default();
    for (low_primitive, cage_primitive) in low.primitives.iter().zip(&cage.primitives) {
        let (low_min, low_max) = bounds(&low_primitive.positions);
        let (cage_min, cage_max) = bounds(&cage_primitive.positions);
        for point in &low_primitive.positions {
            if (0..3).any(|axis| {
                point[axis] < cage_min[axis] - CAGE_CONTAINMENT_EPSILON_M
                    || point[axis] > cage_max[axis] + CAGE_CONTAINMENT_EPSILON_M
            }) {
                summary.out_of_range_count += 1;
                summary.intersection_count += 1;
            }
        }
        for point in &cage_primitive.positions {
            if (0..3).all(|axis| {
                point[axis] > low_min[axis] + CAGE_CONTAINMENT_EPSILON_M
                    && point[axis] < low_max[axis] - CAGE_CONTAINMENT_EPSILON_M
            }) {
                summary.overlap_count += 1;
                summary.intersection_count += 1;
            }
        }
    }
    summary
}

fn bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    (min, max)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn centroid(positions: [[f32; 3]; 3]) -> [f32; 3] {
    [
        (positions[0][0] + positions[1][0] + positions[2][0]) / 3.0,
        (positions[0][1] + positions[1][1] + positions[2][1]) / 3.0,
        (positions[0][2] + positions[1][2] + positions[2][2]) / 3.0,
    ]
}

fn distance3(left: [f32; 3], right: [f32; 3]) -> f32 {
    let delta = sub3(left, right);
    dot3(delta, delta).sqrt()
}

fn sub3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn normalize3(value: [f32; 3]) -> [f32; 3] {
    let length = dot3(value, value).sqrt();
    if !length.is_finite() || length <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [value[0] / length, value[1] / length, value[2] / length]
    }
}

fn negate3(value: [f32; 3]) -> [f32; 3] {
    [-value[0], -value[1], -value[2]]
}

fn to_f64(value: [f32; 3]) -> [f64; 3] {
    [
        f64::from(value[0]),
        f64::from(value[1]),
        f64::from(value[2]),
    ]
}

fn sub3_f64(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn cross3_f64(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn dot3_f64(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile_geometry_program;
    use serde_json::json;

    fn fixture_program(part_id: &str, size: [f64; 3], position: [f64; 3]) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"high-low-cage-diagnostic-fixture",
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":crate::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":4,"max_triangles":1000,"max_glb_bytes":4194304,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{"node_id":"weapon-body","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":size,"position_m":position,"rotation_rad":[0.0,0.0,0.0]}}],
            "part_outputs":[{"part_id":part_id,"input_node_ids":["weapon-body"],"material_zone_id":"zone-white-shell","solid":true}]
        });
        program["canonical_sha256"] = Value::String(crate::canonical_hash(&without_hash(&program)));
        program
    }

    fn fixture_payload() -> Value {
        let high = compile_geometry_program(&fixture_program(
            "weapon-body",
            [4.0, 4.0, 4.0],
            [0.0, 0.0, 0.0],
        ))
        .expect("high fixture");
        let low = compile_geometry_program(&fixture_program(
            "weapon-body",
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
        ))
        .expect("low fixture");
        let cage = compile_geometry_program(&fixture_program(
            "weapon-body",
            [1.5, 1.5, 1.5],
            [0.0, 0.0, 0.0],
        ))
        .expect("cage fixture");
        diagnostic_payload_for_artifacts(&high.glb, &low.glb, &cage.glb)
    }

    fn fixture_producer_payload() -> Value {
        let mut payload = json!({
            "schema_version":PRODUCER_REQUEST_SCHEMA_VERSION,
            "producer_policy":PRODUCER_POLICY,
            "producer_policy_sha256":sha256_hex(PRODUCER_POLICY.as_bytes()),
            "budget_profile":PRODUCER_BUDGET_PROFILE,
            "max_high_triangles":MAX_HIGH_TRIANGLES,
            "max_low_triangles":MAX_LOW_CAGE_TRIANGLES,
            "max_cage_triangles":MAX_LOW_CAGE_TRIANGLES,
            "max_glb_bytes":MAX_DIAGNOSTIC_GLB_BYTES,
            "high_geometry_program":fixture_program("weapon-body", [4.0, 4.0, 4.0], [0.0, 0.0, 0.0]),
            "low_geometry_program":fixture_program("weapon-body", [1.0, 1.0, 1.0], [0.0, 0.0, 0.0]),
            "cage_geometry_program":fixture_program("weapon-body", [1.5, 1.5, 1.5], [0.0, 0.0, 0.0]),
            "surface_bake_reuse_allowed":false,
            "canonical_sha256":""
        });
        payload["canonical_sha256"] = Value::String(crate::canonical_hash(&without_hash(&payload)));
        payload
    }

    #[test]
    fn positive_is_deterministic_hash_only_and_not_a_bake() {
        let payload = fixture_payload();
        let first = diagnose(payload.as_object().expect("payload object")).expect("diagnostic");
        let second = diagnose(payload.as_object().expect("payload object")).expect("diagnostic");
        assert_eq!(first, second);
        assert_eq!(first["diagnostic_status"], "PASS_SOURCE_STRUCTURAL");
        assert_eq!(first["low_triangle_count"], 12);
        assert_eq!(first["cage_triangle_count"], 12);
        assert_eq!(first["ray_miss_count"], 0);
        assert_eq!(first["surface_bake_reuse_allowed"], false);
        assert_eq!(first["raw_media_emitted"], false);
        assert!(first["bake_output_object_sha256s"]
            .as_array()
            .is_some_and(Vec::is_empty));
        assert!(first.get("png_base64").is_none());
        assert!(first.get("bake_map_base64").is_none());
        assert_eq!(first["canonical_sha256"], second["canonical_sha256"]);
    }

    #[test]
    fn allowlisted_worker_operation_routes_to_the_independent_diagnostic() {
        let request = json!({
            "operation": PRODUCTION_WEAPON_HIGH_LOW_CAGE_DIAGNOSTIC_OPERATION,
            "payload": fixture_payload(),
        });
        let result = crate::worker_result(&request).expect("worker dispatch");
        assert_eq!(result["schema_version"], RESULT_SCHEMA_VERSION);
        assert_eq!(result["diagnostic_status"], "PASS_SOURCE_STRUCTURAL");
        assert_eq!(result["runtime_write_performed"], false);
    }

    #[test]
    fn producer_generates_three_independent_glbs_and_replays_byte_exactly() {
        let request = json!({
            "operation": PRODUCTION_WEAPON_HIGH_LOW_CAGE_ARTIFACT_PRODUCER_OPERATION,
            "payload": fixture_producer_payload(),
        });
        let result = crate::worker_result(&request).expect("producer dispatch");
        let replay = crate::worker_result(&request).expect("producer replay dispatch");
        assert_eq!(result, replay);
        assert_eq!(result["schema_version"], PRODUCER_RESULT_SCHEMA_VERSION);
        assert_eq!(result["generated_glb_count"], 3);
        assert_eq!(result["distinct_artifact_bindings"], true);
        assert_eq!(result["worker_replay_count"], 2);
        assert_eq!(result["replay_byte_exact"], true);
        assert_eq!(result["diagnostic_status"], "PASS_SOURCE_STRUCTURAL");
        let high = result["high"]["artifact_sha256"]
            .as_str()
            .expect("high hash");
        let low = result["low"]["artifact_sha256"].as_str().expect("low hash");
        let cage = result["cage"]["artifact_sha256"]
            .as_str()
            .expect("cage hash");
        assert_ne!(high, low);
        assert_ne!(high, cage);
        assert_ne!(low, cage);
        assert_eq!(result["formal_bake_performed"], false);
        assert_eq!(result["png_emitted"], false);
        assert_eq!(result["runtime_write_performed"], false);
    }

    #[test]
    fn producer_rejects_unknown_duplicate_and_oversize_inputs() {
        let mut unknown = fixture_producer_payload();
        unknown["unknown"] = Value::Bool(true);
        assert!(produce(unknown.as_object().expect("producer payload")).is_err());

        let mut oversize = fixture_producer_payload();
        oversize["max_glb_bytes"] = Value::from(MAX_DIAGNOSTIC_GLB_BYTES + 1);
        oversize["canonical_sha256"] =
            Value::String(crate::canonical_hash(&without_hash(&oversize)));
        let oversize_error = produce(oversize.as_object().expect("producer payload"))
            .expect_err("oversize producer budget");
        assert!(oversize_error.to_string().contains("fixed producer budget"));

        let mut duplicate = fixture_producer_payload();
        duplicate["low_geometry_program"] = duplicate["high_geometry_program"].clone();
        duplicate["canonical_sha256"] =
            Value::String(crate::canonical_hash(&without_hash(&duplicate)));
        let duplicate_error = produce(duplicate.as_object().expect("producer payload"))
            .expect_err("duplicate source programs");
        assert!(duplicate_error
            .to_string()
            .contains("hashes must be distinct"));
    }

    #[test]
    fn corrupt_artifact_is_rejected_before_diagnostics() {
        let mut payload = fixture_payload();
        payload["high_glb_base64"] = Value::String("AAAA".to_owned());
        payload["high_artifact_sha256"] = Value::String(sha256_hex(b"bad"));
        payload["canonical_sha256"] = Value::String(crate::canonical_hash(&without_hash(&payload)));
        let error =
            diagnose(payload.as_object().expect("payload object")).expect_err("corrupt GLB");
        assert!(error.to_string().contains("GLB") || error.to_string().contains("base64"));
    }

    #[test]
    fn unknown_or_oversize_request_is_rejected_closed() {
        let mut unknown = fixture_payload();
        unknown["unknown"] = Value::Bool(true);
        assert!(diagnose(unknown.as_object().expect("payload object")).is_err());

        let mut oversize = fixture_payload();
        oversize["max_high_triangles"] = Value::from(MAX_HIGH_TRIANGLES + 1);
        oversize["canonical_sha256"] =
            Value::String(crate::canonical_hash(&without_hash(&oversize)));
        let error = diagnose(oversize.as_object().expect("payload object")).expect_err("budget");
        assert!(error.to_string().contains("fixed diagnostic budget"));
    }

    #[test]
    fn low_cage_retarget_is_rejected_closed() {
        let mut payload = fixture_payload();
        let foreign_cage = compile_geometry_program(&fixture_program(
            "foreign-part",
            [1.5, 1.5, 1.5],
            [0.0, 0.0, 0.0],
        ))
        .expect("foreign cage fixture");
        payload["cage_artifact_sha256"] = Value::String(sha256_hex(&foreign_cage.glb));
        payload["cage_glb_base64"] =
            Value::String(base64::engine::general_purpose::STANDARD.encode(foreign_cage.glb));
        payload["canonical_sha256"] = Value::String(crate::canonical_hash(&without_hash(&payload)));
        let error = diagnose(payload.as_object().expect("payload object")).expect_err("retarget");
        assert!(
            error.to_string().contains("PART_CORRESPONDENCE")
                || error.to_string().contains("CAGE_TOPOLOGY")
        );
    }

    fn without_hash(value: &Value) -> Value {
        let mut object = value.as_object().expect("hash preimage object").clone();
        object.remove("canonical_sha256");
        Value::Object(object)
    }
}
