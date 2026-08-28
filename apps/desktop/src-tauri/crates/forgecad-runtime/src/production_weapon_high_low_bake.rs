//! Runtime-owned public seam for the formal High/Low/HeroUV/Cage bake gate.
//!
//! The Store transaction is intentionally kept behind this seam. Existing
//! formal receipts can be replayed/read through the Store adapter. A brand-new
//! prepare still fails closed until Runtime can materialize all seven typed
//! records from independently verified High/Low/HeroUV/Cage Worker outputs;
//! it never falls back to diagnostic preflight or a Worker-only result.

use super::{canonical_json_hash, is_opaque_id, sha256_hex, Runtime, RuntimeError};
use forgecad_contracts::{
    ProductionStageHeadV3Record, ProductionStageTransitionV3Record,
    ProductionWeaponHighLowBakeGetRequest, ProductionWeaponHighLowBakePrepareRequest,
    ProductionWeaponHighLowBakePrepareResult, ProductionWeaponHighLowBakeReceiptRecord,
    PRODUCTION_STAGE_V3_STAGES, PRODUCTION_WEAPON_HIGH_LOW_BAKE_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_HIGH_LOW_BAKE_POLICY,
    PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREPARE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_HIGH_LOW_GATE_SCOPES, PRODUCTION_WEAPON_HIGH_LOW_SOURCE_STAGES,
    PRODUCTION_WEAPON_HIGH_LOW_TARGET_STAGES,
};
use forgecad_worker_protocol::{
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION, PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY,
    PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESULT_SCHEMA_VERSION,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "bake_receipt_id",
    "session_id",
    "project_id",
    "gate_scope",
    "source_stage",
    "target_stage",
    "source_stage_head_transition_id",
    "source_stage_head_transition_sha256",
    "source_stage_head_canonical_sha256",
    "source_stage_head_stage",
    "high_candidate_id",
    "high_candidate_state_sha256",
    "high_artifact_id",
    "high_artifact_sha256",
    "high_artifact_readback_sha256",
    "low_candidate_id",
    "low_candidate_state_sha256",
    "low_artifact_id",
    "low_artifact_sha256",
    "low_artifact_readback_sha256",
    "cage_artifact_id",
    "cage_artifact_sha256",
    "cage_artifact_readback_sha256",
    "correspondence_id",
    "correspondence_object_sha256",
    "correspondence_canonical_sha256",
    "bake_plan_id",
    "bake_plan_object_sha256",
    "bake_plan_canonical_sha256",
    "bake_policy",
    "bake_policy_sha256",
    "input_sha256",
    "idempotency_key",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "bake_receipt_id",
    "session_id",
    "project_id",
    "gate_scope",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    kind: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{kind} request must be an object")))?;
    if let Some(field) = object
        .keys()
        .find(|field| !fields.contains(&field.as_str()))
    {
        return Err(invalid(format!(
            "{kind} request contains unsupported field {field}"
        )));
    }
    if let Some(field) = fields.iter().find(|field| !object.contains_key(**field)) {
        return Err(invalid(format!("{kind} request is missing {field}")));
    }
    Ok(object)
}

fn require_ids(values: &[(&str, &str)]) -> Result<(), RuntimeError> {
    for (field, value) in values {
        if !is_opaque_id(value) {
            return Err(invalid(format!(
                "ProductionWeaponHighLowBake {field} identity is invalid"
            )));
        }
    }
    Ok(())
}

fn require_hashes(values: &[(&str, &str)]) -> Result<(), RuntimeError> {
    for (field, value) in values {
        if !forgecad_contracts::is_sha256(value) {
            return Err(invalid(format!(
                "ProductionWeaponHighLowBake {field} SHA-256 is invalid"
            )));
        }
    }
    Ok(())
}

/// Validate the closed High/Low/Cage geometric-bake Worker projection before
/// any future CAS/receipt producer can consume it.  A Worker may emit maps for
/// diagnostic inspection even when a ray had no valid same-Part hit; that is
/// never an admissible bake result.  In particular, a nearest-surface
/// fallback, miss, foreign-Part hit, backface hit, or any cage/UV diagnostic
/// failure is a hard rejection here rather than a softer status.
#[allow(dead_code)]
pub fn validate_production_weapon_geometric_bake_result(
    result: &Value,
    high_artifact_sha256: &str,
    low_artifact_sha256: &str,
    cage_artifact_sha256: &str,
) -> Result<(), RuntimeError> {
    if !forgecad_contracts::is_sha256(high_artifact_sha256)
        || !forgecad_contracts::is_sha256(low_artifact_sha256)
        || !forgecad_contracts::is_sha256(cage_artifact_sha256)
        || high_artifact_sha256 == low_artifact_sha256
        || high_artifact_sha256 == cage_artifact_sha256
        || low_artifact_sha256 == cage_artifact_sha256
    {
        return Err(invalid(
            "geometric bake artifact bindings are not distinct SHA-256 values",
        ));
    }
    let object = result
        .as_object()
        .ok_or_else(|| invalid("geometric bake Worker result is not an object"))?;
    for (field, expected) in [
        (
            "schema_version",
            PRODUCTION_WEAPON_GEOMETRIC_BAKE_RESULT_SCHEMA_VERSION,
        ),
        ("operation", PRODUCTION_WEAPON_GEOMETRIC_BAKE_OPERATION),
        ("bake_policy", PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY),
        ("normal_convention", "OpenGL+Y"),
    ] {
        if object.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(invalid(format!("geometric bake Worker {field} differs")));
        }
    }
    if object.get("bake_policy_sha256").and_then(Value::as_str)
        != Some(sha256_hex(PRODUCTION_WEAPON_GEOMETRIC_BAKE_POLICY.as_bytes()).as_str())
        || object.get("high_artifact_sha256").and_then(Value::as_str) != Some(high_artifact_sha256)
        || object.get("low_artifact_sha256").and_then(Value::as_str) != Some(low_artifact_sha256)
        || object.get("cage_artifact_sha256").and_then(Value::as_str) != Some(cage_artifact_sha256)
        || object
            .get("surface_bake_reuse_allowed")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("formal_quality_gate").and_then(Value::as_str) != Some("NOT_RUN")
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object
            .get("production_stage_advanced")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid(
            "geometric bake Worker binding/status is not fail-closed",
        ));
    }
    let diagnostic = object
        .get("diagnostic")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("geometric bake diagnostic is missing"))?;
    if diagnostic.get("status").and_then(Value::as_str) != Some("PASS_SOURCE_STRUCTURAL") {
        return Err(invalid(
            "geometric bake diagnostic status is not an exact structural pass",
        ));
    }
    for field in [
        "ray_miss_count",
        "nearest_surface_fallback_count",
        "cross_part_hit_count",
        "backface_hit_count",
        "skew_count",
        "penetration_count",
        "cage_intersection_count",
        "overlap_count",
        "out_of_range_count",
        "thickness_miss_count",
        "uv_overlap_count",
    ] {
        if diagnostic.get(field).and_then(Value::as_u64) != Some(0) {
            return Err(invalid(format!(
                "geometric bake diagnostic {field} prevents PASS"
            )));
        }
    }
    let ray_sample_count = diagnostic
        .get("ray_sample_count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .ok_or_else(|| invalid("geometric bake ray sample count is empty"))?;
    if diagnostic.get("ray_hit_count").and_then(Value::as_u64) != Some(ray_sample_count) {
        return Err(invalid("geometric bake ray hits do not cover every sample"));
    }
    let coverage = object
        .get("coverage")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("geometric bake coverage is missing"))?;
    if coverage
        .get("primary_covered_pixels")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .is_none()
    {
        return Err(invalid("geometric bake has no primary covered pixels"));
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| invalid("geometric bake Worker canonical hash is missing"))?;
    let mut normalized = result.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&normalized) != canonical {
        return Err(invalid("geometric bake Worker canonical hash differs"));
    }
    Ok(())
}

fn validate_stage_binding(
    gate_scope: &str,
    source_stage: &str,
    target_stage: &str,
    source_stage_head_stage: &str,
) -> Result<(), RuntimeError> {
    if !PRODUCTION_WEAPON_HIGH_LOW_GATE_SCOPES.contains(&gate_scope) {
        return Err(invalid("ProductionWeaponHighLowBake gate_scope is invalid"));
    }
    if !PRODUCTION_STAGE_V3_STAGES.contains(&source_stage)
        || !PRODUCTION_STAGE_V3_STAGES.contains(&target_stage)
        || !PRODUCTION_STAGE_V3_STAGES.contains(&source_stage_head_stage)
    {
        return Err(invalid("ProductionWeaponHighLowBake stage is invalid"));
    }
    let Some(index) = PRODUCTION_WEAPON_HIGH_LOW_GATE_SCOPES
        .iter()
        .position(|value| *value == gate_scope)
    else {
        return Err(invalid("ProductionWeaponHighLowBake gate_scope is invalid"));
    };
    if PRODUCTION_WEAPON_HIGH_LOW_SOURCE_STAGES[index] != source_stage
        || PRODUCTION_WEAPON_HIGH_LOW_TARGET_STAGES[index] != target_stage
    {
        return Err(invalid(
            "ProductionWeaponHighLowBake gate_scope/stage pair differs",
        ));
    }
    if source_stage_head_stage != source_stage {
        return Err(invalid(
            "ProductionWeaponHighLowBake source-stage head stage differs",
        ));
    }
    Ok(())
}

fn validate_distinct_bindings(
    request: &ProductionWeaponHighLowBakePrepareRequest,
) -> Result<(), RuntimeError> {
    let mut candidate_ids = BTreeSet::new();
    if !candidate_ids.insert(request.high_candidate_id.as_str())
        || !candidate_ids.insert(request.low_candidate_id.as_str())
    {
        return Err(invalid(
            "ProductionWeaponHighLowBake high and low candidates must be distinct",
        ));
    }
    let mut artifact_ids = BTreeSet::new();
    for artifact_id in [
        request.high_artifact_id.as_str(),
        request.low_artifact_id.as_str(),
        request.cage_artifact_id.as_str(),
    ] {
        if !artifact_ids.insert(artifact_id) {
            return Err(invalid(
                "ProductionWeaponHighLowBake High/Low/Cage artifacts must be distinct",
            ));
        }
    }
    Ok(())
}

fn parse_prepare(value: &Value) -> Result<ProductionWeaponHighLowBakePrepareRequest, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, "ProductionWeaponHighLowBake prepare")?;
    let request: ProductionWeaponHighLowBakePrepareRequest = serde_json::from_value(value.clone())
        .map_err(|error| {
            invalid(format!(
                "invalid ProductionWeaponHighLowBake prepare request: {error}"
            ))
        })?;

    if request.schema_version != PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREPARE_REQUEST_SCHEMA_VERSION {
        return Err(invalid(
            "ProductionWeaponHighLowBake prepare schema differs",
        ));
    }
    require_ids(&[
        ("bake_receipt_id", &request.bake_receipt_id),
        ("session_id", &request.session_id),
        ("project_id", &request.project_id),
        (
            "source_stage_head_transition_id",
            &request.source_stage_head_transition_id,
        ),
        ("high_candidate_id", &request.high_candidate_id),
        ("high_artifact_id", &request.high_artifact_id),
        ("low_candidate_id", &request.low_candidate_id),
        ("low_artifact_id", &request.low_artifact_id),
        ("cage_artifact_id", &request.cage_artifact_id),
        ("correspondence_id", &request.correspondence_id),
        ("bake_plan_id", &request.bake_plan_id),
        ("idempotency_key", &request.idempotency_key),
    ])?;
    require_hashes(&[
        (
            "source_stage_head_transition",
            &request.source_stage_head_transition_sha256,
        ),
        (
            "source_stage_head_canonical",
            &request.source_stage_head_canonical_sha256,
        ),
        ("high_candidate_state", &request.high_candidate_state_sha256),
        ("high_artifact", &request.high_artifact_sha256),
        (
            "high_artifact_readback",
            &request.high_artifact_readback_sha256,
        ),
        ("low_candidate_state", &request.low_candidate_state_sha256),
        ("low_artifact", &request.low_artifact_sha256),
        (
            "low_artifact_readback",
            &request.low_artifact_readback_sha256,
        ),
        ("cage_artifact", &request.cage_artifact_sha256),
        (
            "cage_artifact_readback",
            &request.cage_artifact_readback_sha256,
        ),
        (
            "correspondence_object",
            &request.correspondence_object_sha256,
        ),
        (
            "correspondence_canonical",
            &request.correspondence_canonical_sha256,
        ),
        ("bake_plan_object", &request.bake_plan_object_sha256),
        ("bake_plan_canonical", &request.bake_plan_canonical_sha256),
        ("bake_policy", &request.bake_policy_sha256),
        ("input", &request.input_sha256),
    ])?;
    if request.bake_policy != PRODUCTION_WEAPON_HIGH_LOW_BAKE_POLICY {
        return Err(invalid("ProductionWeaponHighLowBake policy differs"));
    }
    validate_stage_binding(
        &request.gate_scope,
        &request.source_stage,
        &request.target_stage,
        &request.source_stage_head_stage,
    )?;
    validate_distinct_bindings(&request)?;
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    if canonical_json_hash(&Value::Object(preimage)) != request.input_sha256 {
        return Err(invalid("ProductionWeaponHighLowBake input hash differs"));
    }
    Ok(request)
}

fn parse_get(value: &Value) -> Result<ProductionWeaponHighLowBakeGetRequest, RuntimeError> {
    exact_object(value, GET_FIELDS, "ProductionWeaponHighLowBake get")?;
    let request: ProductionWeaponHighLowBakeGetRequest = serde_json::from_value(value.clone())
        .map_err(|error| {
            invalid(format!(
                "invalid ProductionWeaponHighLowBake get request: {error}"
            ))
        })?;
    if request.schema_version != PRODUCTION_WEAPON_HIGH_LOW_BAKE_GET_REQUEST_SCHEMA_VERSION {
        return Err(invalid("ProductionWeaponHighLowBake get schema differs"));
    }
    require_ids(&[
        ("bake_receipt_id", &request.bake_receipt_id),
        ("session_id", &request.session_id),
        ("project_id", &request.project_id),
    ])?;
    if !PRODUCTION_WEAPON_HIGH_LOW_GATE_SCOPES.contains(&request.gate_scope.as_str()) {
        return Err(invalid(
            "ProductionWeaponHighLowBake get gate_scope is invalid",
        ));
    }
    Ok(request)
}

fn validate_current_stage_head(
    runtime: &Runtime,
    request: &ProductionWeaponHighLowBakePrepareRequest,
) -> Result<(), RuntimeError> {
    let transition: ProductionStageTransitionV3Record = runtime
        .store
        .get_production_stage_transition_v3(&request.source_stage_head_transition_id)?
        .ok_or_else(|| invalid("PRODUCTION_WEAPON_HIGH_LOW_BAKE_CURRENT_STAGE_HEAD_MISSING"))?;
    if transition.session_id != request.session_id
        || transition.project_id != request.project_id
        || transition.transition_id != request.source_stage_head_transition_id
        || transition.canonical_sha256 != request.source_stage_head_transition_sha256
        || transition.to_stage != request.source_stage
        || transition.to_stage != request.source_stage_head_stage
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_HIGH_LOW_BAKE_CURRENT_STAGE_HEAD_MISMATCH",
        ));
    }
    let head: ProductionStageHeadV3Record = runtime
        .store
        .get_production_stage_head_v3(
            &request.session_id,
            &request.project_id,
            &transition.root_candidate_id,
        )?
        .ok_or_else(|| invalid("PRODUCTION_WEAPON_HIGH_LOW_BAKE_CURRENT_STAGE_HEAD_MISSING"))?;
    if head.session_id != request.session_id
        || head.project_id != request.project_id
        || head.root_candidate_id != transition.root_candidate_id
        || head.head_stage != request.source_stage
        || head.head_transition_id != transition.transition_id
        || head.head_transition_sha256 != transition.canonical_sha256
        || head.canonical_sha256 != request.source_stage_head_canonical_sha256
    {
        return Err(invalid(
            "PRODUCTION_WEAPON_HIGH_LOW_BAKE_CURRENT_STAGE_HEAD_MISMATCH",
        ));
    }
    Ok(())
}

fn producer_unavailable(
    runtime: &Runtime,
    request: &ProductionWeaponHighLowBakePrepareRequest,
) -> Result<RuntimeError, RuntimeError> {
    let transition = runtime
        .store
        .get_production_stage_transition_v3(&request.source_stage_head_transition_id)?
        .ok_or_else(|| invalid("PRODUCTION_WEAPON_HIGH_LOW_BAKE_CURRENT_STAGE_HEAD_MISSING"))?;

    // NativeHighDurable is intentionally materialized on the Stage source
    // candidate: its AuthoringMesh still points at the source GLB.  The formal
    // High candidate is a distinct derived candidate whose prepared artifact
    // must be the Native High GLB.  Treating the derived candidate as the
    // NativeHigh owner creates an impossible requirement that one candidate
    // simultaneously point at both source and High artifacts.
    let source_high = runtime
        .store
        .get_native_high_durable_by_candidate(&transition.head_candidate_id)?;
    let source_authoring = match &source_high {
        Some(high) => runtime.store.get_authoring_mesh_durable_record_by_mesh(
            &transition.head_candidate_id,
            &high.source_canonical_mesh_id,
        )?,
        None => None,
    };
    let formal_high_candidate = runtime.candidate(&request.high_candidate_id)?;
    let formal_high = runtime.store.get_production_weapon_formal_high(
        &request.project_id,
        &request.session_id,
        &request.high_artifact_id,
    )?;

    let formal_high_lineage_ready = match (
        &source_high,
        &source_authoring,
        &formal_high_candidate,
        &formal_high,
    ) {
        (Some(high), Some(authoring), Some(candidate), Some(formal)) => {
            transition.head_candidate_id != request.high_candidate_id
                && transition.project_id == request.project_id
                && transition.head_candidate_state_sha256 == high.candidate_state_sha256
                && transition.head_candidate_state_sha256 == authoring.candidate_state_sha256
                && transition.output_artifact_id == authoring.source_artifact_object_sha256
                && transition.head_artifact_sha256 == authoring.source_artifact_object_sha256
                && authoring.project_id == request.project_id
                && authoring.candidate_id == transition.head_candidate_id
                && authoring.source_artifact_sha256 == authoring.source_artifact_object_sha256
                && high.project_id == request.project_id
                && high.candidate_id == transition.head_candidate_id
                && high.source_canonical_mesh_id == authoring.canonical_mesh_id
                && high.source_canonical_mesh_object_sha256
                    == authoring.canonical_mesh_object_sha256
                && high.source_canonical_mesh_sha256 == authoring.canonical_mesh_sha256
                && high.high_artifact_id == request.high_artifact_id
                && high.high_artifact_sha256 == request.high_artifact_sha256
                && candidate.project_id == request.project_id
                && candidate.candidate_id == request.high_candidate_id
                && candidate.canonical_sha256 == request.high_candidate_state_sha256
                && candidate.prepared_object_id.as_deref()
                    == Some(request.high_artifact_id.as_str())
                && candidate.prepared_object_sha256.as_deref()
                    == Some(request.high_artifact_sha256.as_str())
                && formal.source_stage_head_transition_id == transition.transition_id
                && formal.source_candidate_id == transition.head_candidate_id
                && formal.high_candidate_id == request.high_candidate_id
                && formal.high_candidate_state_sha256 == request.high_candidate_state_sha256
                && formal.high_artifact_id == request.high_artifact_id
                && formal.high_artifact_sha256 == request.high_artifact_sha256
                && formal.high_artifact_readback_sha256 == request.high_artifact_readback_sha256
        }
        _ => false,
    };

    let mut blockers = Vec::new();
    if !formal_high_lineage_ready {
        blockers.push("FORMAL_HIGH_STAGE_SOURCE_LINEAGE_UNAVAILABLE".to_owned());
    }

    match &source_high {
        None => blockers.push("NATIVE_HIGH_DURABLE_UNAVAILABLE".to_owned()),
        Some(high)
            if high.project_id != request.project_id
                || high.candidate_id != transition.head_candidate_id
                || high.candidate_state_sha256 != transition.head_candidate_state_sha256
                || high.high_artifact_id != request.high_artifact_id
                || high.high_artifact_sha256 != request.high_artifact_sha256 =>
        {
            blockers.push("NATIVE_HIGH_DURABLE_BINDING_MISMATCH".to_owned())
        }
        Some(_) => {}
    }

    let low = runtime
        .store
        .get_low_quad_draft_durable_by_candidate_artifact(
            &request.low_candidate_id,
            &request.low_artifact_sha256,
        )?;
    match (&source_high, &low) {
        (_, None) => blockers.push("LOW_QUAD_DURABLE_UNAVAILABLE".to_owned()),
        (Some(high), Some(low))
            if low.project_id != request.project_id
                || low.candidate_state_sha256 != request.low_candidate_state_sha256
                || low.source_high_artifact_id != request.high_artifact_id
                || low.source_high_artifact_sha256 != request.high_artifact_sha256
                || low.source_high_artifact_readback_sha256
                    != request.high_artifact_readback_sha256
                || low.artifact_sha256 != request.low_artifact_sha256
                || low.readback_sha256 != request.low_artifact_readback_sha256
                || high.high_artifact_sha256 != low.source_high_artifact_sha256 =>
        {
            blockers.push("LOW_QUAD_DURABLE_BINDING_MISMATCH".to_owned())
        }
        (None, Some(_)) => blockers.push("LOW_HIGH_LINEAGE_UNWITNESSED".to_owned()),
        (Some(_), Some(_)) => {}
    }

    if let Some(low) = &low {
        let hero_uv = runtime.store.get_hero_uv_by_candidate_source_artifact(
            &request.project_id,
            &request.low_candidate_id,
            &request.low_artifact_sha256,
        )?;
        match hero_uv {
            None => blockers.push("HERO_UV_DURABLE_UNAVAILABLE".to_owned()),
            Some(hero_uv)
                if hero_uv.candidate_state_sha256 != request.low_candidate_state_sha256
                    || hero_uv.source_low_artifact_id != request.low_artifact_id
                    || hero_uv.source_low_artifact_object_sha256 != low.artifact_object_sha256
                    || hero_uv.source_low_artifact_sha256 != request.low_artifact_sha256
                    || hero_uv.source_low_artifact_readback_object_sha256
                        != low.readback_object_sha256
                    || hero_uv.source_low_artifact_readback_sha256
                        != request.low_artifact_readback_sha256 =>
            {
                blockers.push("HERO_UV_DURABLE_BINDING_MISMATCH".to_owned())
            }
            Some(_) => {}
        }
    } else {
        blockers.push("HERO_UV_LOW_SOURCE_UNAVAILABLE".to_owned());
    }

    let detail = serde_json::json!({
        "code":"PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE",
        "blockers":blockers,
        "runtime_write":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    Ok(invalid(format!(
        "PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE: {detail}"
    )))
}

fn receipt_matches_prepare(
    receipt: &ProductionWeaponHighLowBakeReceiptRecord,
    request: &ProductionWeaponHighLowBakePrepareRequest,
) -> bool {
    receipt.bake_receipt_id == request.bake_receipt_id
        && receipt.session_id == request.session_id
        && receipt.project_id == request.project_id
        && receipt.gate_scope == request.gate_scope
        && receipt.source_stage == request.source_stage
        && receipt.target_stage == request.target_stage
        && receipt.source_stage_head_transition_id == request.source_stage_head_transition_id
        && receipt.source_stage_head_transition_sha256
            == request.source_stage_head_transition_sha256
        && receipt.source_stage_head_canonical_sha256 == request.source_stage_head_canonical_sha256
        && receipt.source_stage_head_stage == request.source_stage_head_stage
        && receipt.high_candidate_id == request.high_candidate_id
        && receipt.high_candidate_state_sha256 == request.high_candidate_state_sha256
        && receipt.high_artifact_id == request.high_artifact_id
        && receipt.high_artifact_sha256 == request.high_artifact_sha256
        && receipt.high_artifact_readback_sha256 == request.high_artifact_readback_sha256
        && receipt.low_candidate_id == request.low_candidate_id
        && receipt.low_candidate_state_sha256 == request.low_candidate_state_sha256
        && receipt.low_artifact_id == request.low_artifact_id
        && receipt.low_artifact_sha256 == request.low_artifact_sha256
        && receipt.low_artifact_readback_sha256 == request.low_artifact_readback_sha256
        && receipt.cage_artifact_id == request.cage_artifact_id
        && receipt.cage_artifact_sha256 == request.cage_artifact_sha256
        && receipt.cage_artifact_readback_sha256 == request.cage_artifact_readback_sha256
        && receipt.correspondence_id == request.correspondence_id
        && receipt.correspondence_object_sha256 == request.correspondence_object_sha256
        && receipt.correspondence_canonical_sha256 == request.correspondence_canonical_sha256
        && receipt.bake_plan_id == request.bake_plan_id
        && receipt.bake_plan_object_sha256 == request.bake_plan_object_sha256
        && receipt.bake_plan_canonical_sha256 == request.bake_plan_canonical_sha256
        && receipt.bake_policy == request.bake_policy
        && receipt.bake_policy_sha256 == request.bake_policy_sha256
        && receipt.input_sha256 == request.input_sha256
}

impl Runtime {
    /// Validate the formal request and current Stage@3 head. Exact durable
    /// receipts can be replayed without invoking a Worker or writing CAS. A
    /// new materialization remains fail-closed until its producer is wired.
    pub fn production_weapon_high_low_bake_prepare(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_prepare(&value)?;
        validate_current_stage_head(self, &request)?;
        if let Some(existing) = self.store.get_production_weapon_high_low_bake(
            &request.project_id,
            &request.session_id,
            &request.bake_receipt_id,
            &request.gate_scope,
        )? {
            if !receipt_matches_prepare(&existing.bake_receipt, &request) {
                return Err(invalid("PRODUCTION_WEAPON_HIGH_LOW_BAKE_REPLAY_CONFLICT"));
            }
            let result = ProductionWeaponHighLowBakePrepareResult {
                schema_version: "ProductionWeaponHighLowBakePrepareResult@1".to_owned(),
                bake_receipt_id: existing.bake_receipt_id,
                bake_receipt_object_sha256: existing.bake_receipt_object_sha256,
                bake_receipt: existing.bake_receipt,
                replayed: true,
                restart_hash_verified: existing.restart_hash_verified,
                runtime_write: false,
                production_stage_advanced: false,
                candidate_confirmed: false,
                version_created: false,
                export_performed: false,
            };
            return serde_json::to_value(result)
                .map_err(|error| invalid(format!("formal bake replay encode failed: {error}")));
        }
        Err(producer_unavailable(self, &request)?)
    }

    /// Read and re-verify the exact receipt from Store/CAS. It never
    /// substitutes the read-only preflight projection.
    pub fn production_weapon_high_low_bake_get(&self, value: Value) -> Result<Value, RuntimeError> {
        let request = parse_get(&value)?;
        let result = self
            .store
            .get_production_weapon_high_low_bake(
                &request.project_id,
                &request.session_id,
                &request.bake_receipt_id,
                &request.gate_scope,
            )?
            .ok_or_else(|| invalid("PRODUCTION_WEAPON_HIGH_LOW_BAKE_NOT_FOUND"))?;
        serde_json::to_value(result)
            .map_err(|error| invalid(format!("formal bake get encode failed: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn prepare_request() -> Value {
        let mut value = json!({
            "schema_version":PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREPARE_REQUEST_SCHEMA_VERSION,
            "bake_receipt_id":"bake-receipt-1",
            "session_id":"session-1",
            "project_id":"project-1",
            "gate_scope":"secondary-form-approved",
            "source_stage":"secondary-form-approved",
            "target_stage":"high-poly-approved",
            "source_stage_head_transition_id":"transition-1",
            "source_stage_head_transition_sha256":"a".repeat(64),
            "source_stage_head_canonical_sha256":"b".repeat(64),
            "source_stage_head_stage":"secondary-form-approved",
            "high_candidate_id":"high-candidate-1",
            "high_candidate_state_sha256":"c".repeat(64),
            "high_artifact_id":"high-artifact-1",
            "high_artifact_sha256":"d".repeat(64),
            "high_artifact_readback_sha256":"e".repeat(64),
            "low_candidate_id":"low-candidate-1",
            "low_candidate_state_sha256":"f".repeat(64),
            "low_artifact_id":"low-artifact-1",
            "low_artifact_sha256":"0".repeat(64),
            "low_artifact_readback_sha256":"1".repeat(64),
            "cage_artifact_id":"cage-artifact-1",
            "cage_artifact_sha256":"2".repeat(64),
            "cage_artifact_readback_sha256":"3".repeat(64),
            "correspondence_id":"correspondence-1",
            "correspondence_object_sha256":"4".repeat(64),
            "correspondence_canonical_sha256":"5".repeat(64),
            "bake_plan_id":"bake-plan-1",
            "bake_plan_object_sha256":"6".repeat(64),
            "bake_plan_canonical_sha256":"7".repeat(64),
            "bake_policy":PRODUCTION_WEAPON_HIGH_LOW_BAKE_POLICY,
            "bake_policy_sha256":"8".repeat(64),
            "input_sha256":"",
            "idempotency_key":"bake-idempotency-1"
        });
        let mut preimage = value.as_object().expect("request").clone();
        preimage.remove("input_sha256");
        value["input_sha256"] = Value::String(canonical_json_hash(&Value::Object(preimage)));
        value
    }

    #[test]
    fn prepare_request_is_closed_hash_and_stage_bound() {
        // high-artifact is the first formal gate; secondary-form-approved is
        // intentionally not a valid gate scope and must fail closed.
        assert!(parse_prepare(&prepare_request()).is_err());
        let mut valid = prepare_request();
        valid["gate_scope"] = Value::String("high-artifact".to_owned());
        let mut preimage = valid.as_object().expect("request").clone();
        preimage.remove("input_sha256");
        valid["input_sha256"] = Value::String(canonical_json_hash(&Value::Object(preimage)));
        parse_prepare(&valid).expect("valid closed request");
        valid["unknown"] = Value::Bool(true);
        assert!(parse_prepare(&valid).is_err());
    }

    #[test]
    fn empty_runtime_prepare_never_writes_without_current_head_or_store_adapter() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let result = runtime.production_weapon_high_low_bake_prepare({
            let mut request = prepare_request();
            request["gate_scope"] = Value::String("high-artifact".to_owned());
            let mut preimage = request.as_object().expect("request").clone();
            preimage.remove("input_sha256");
            request["input_sha256"] = Value::String(canonical_json_hash(&Value::Object(preimage)));
            request
        });
        assert!(result.is_err());
        assert!(result
            .expect_err("empty Runtime must fail closed")
            .to_string()
            .contains("CURRENT_STAGE_HEAD_MISSING"));
    }
}
