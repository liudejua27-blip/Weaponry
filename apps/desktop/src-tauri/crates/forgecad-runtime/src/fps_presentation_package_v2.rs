//! Runtime-owned editable composite FPS package materialization.

use forgecad_contracts::*;
use forgecad_core::{canonical_json_bytes, canonical_json_hash};
use forgecad_store::{
    foundation_authoring_mesh_v2_materialization::FoundationAuthoringMeshV2MaterializationDescriptor,
    fps_presentation_package_v2 as package_store, FpsPresentationPackageV2StoreRecord,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use super::{now_string, Runtime, RuntimeError};

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "FPS_PRESENTATION_PACKAGE_V2_REJECTED: {}",
        message.into()
    ))
}

fn canonical_hash_without<T: serde::Serialize>(
    value: &T,
    field: &str,
) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(value).map_err(|error| invalid(error.to_string()))?;
    value
        .as_object_mut()
        .ok_or_else(|| invalid("canonical payload must be an object"))?
        .insert(field.to_owned(), Value::String(String::new()));
    Ok(canonical_json_hash(&value))
}

fn parse_prepare(value: &Value) -> Result<FpsPresentationPackageV2PrepareRequest, RuntimeError> {
    let request: FpsPresentationPackageV2PrepareRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("prepare request is not closed: {error}")))?;
    if request.schema_version != FPS_PRESENTATION_PACKAGE_V2_PREPARE_REQUEST_SCHEMA_VERSION
        || !super::is_opaque_id(&request.project_id)
        || !super::is_opaque_id(&request.weapon_materialization_id)
        || !super::is_opaque_id(&request.arms_materialization_id)
        || !super::is_opaque_id(&request.animation_materialization_id)
        || !super::is_opaque_id(&request.idempotency_key)
        || !super::is_sha256(&request.weapon_descriptor_sha256)
        || !super::is_sha256(&request.arms_descriptor_sha256)
        || !super::is_sha256(&request.animation_descriptor_sha256)
        || !super::is_sha256(&request.input_sha256)
        || request.package_policy != FPS_PRESENTATION_PACKAGE_V2_POLICY
        || request.max_response_bytes != FPS_PRESENTATION_PACKAGE_V2_MAX_RESPONSE_BYTES
        || request.runtime_write_performed
        || request.writer_policy != FPS_PRESENTATION_PACKAGE_V2_WRITER_POLICY
        || request.canonicalization_policy != FPS_PRESENTATION_PACKAGE_V2_CANONICALIZATION_POLICY
        || canonical_hash_without(&request, "input_sha256")? != request.input_sha256
    {
        return Err(invalid(
            "prepare request identity, policy, or hash is invalid",
        ));
    }
    if BTreeSet::from([
        request.weapon_materialization_id.as_str(),
        request.arms_materialization_id.as_str(),
        request.animation_materialization_id.as_str(),
    ])
    .len()
        != 3
    {
        return Err(invalid(
            "weapon, arms, and animation materializations must be distinct",
        ));
    }
    Ok(request)
}

fn parse_get(value: &Value) -> Result<FpsPresentationPackageV2GetRequest, RuntimeError> {
    let request: FpsPresentationPackageV2GetRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("get request is not closed: {error}")))?;
    if request.schema_version != FPS_PRESENTATION_PACKAGE_V2_GET_REQUEST_SCHEMA_VERSION
        || !super::is_opaque_id(&request.project_id)
        || !super::is_opaque_id(&request.package_id)
        || request
            .package_sha256
            .as_deref()
            .is_some_and(|value| !super::is_sha256(value))
        || request.runtime_write_performed
        || request.persistent_user_data_touched
        || !super::is_sha256(&request.input_sha256)
        || canonical_hash_without(&request, "input_sha256")? != request.input_sha256
    {
        return Err(invalid("get request identity or hash is invalid"));
    }
    Ok(request)
}

struct Source {
    aggregate: forgecad_store::FoundationAuthoringMeshV2MaterializationRecord,
    descriptor: FoundationAuthoringMeshV2MaterializationDescriptor,
    foundation: forgecad_store::weapon_foundation_import::WeaponFoundationImportRecord,
}

fn source(
    runtime: &Runtime,
    project_id: &str,
    id: &str,
    expected_descriptor: &str,
) -> Result<Source, RuntimeError> {
    let aggregate = runtime
        .store
        .get_foundation_authoring_mesh_v2_materialization(project_id, id)?
        .ok_or_else(|| invalid(format!("materialization {id} is not durable")))?;
    if aggregate.descriptor_canonical_sha256 != expected_descriptor {
        return Err(invalid("materialization descriptor hash differs"));
    }
    let bytes = runtime.cas_read_bounded(
        &aggregate.descriptor_object_sha256,
        package_store::MAX_JSON_BYTES,
    )?;
    let descriptor: FoundationAuthoringMeshV2MaterializationDescriptor =
        serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("materialization descriptor is invalid: {error}")))?;
    if descriptor.canonical_sha256 != expected_descriptor || descriptor.project_id != project_id {
        return Err(invalid("materialization descriptor CAS binding differs"));
    }
    let foundation = runtime
        .store
        .get_weapon_foundation_import(
            &aggregate.foundation_request_id,
            Some(&aggregate.foundation_request_sha256),
        )?
        .ok_or_else(|| invalid("foundation import is not durable"))?;
    Ok(Source {
        aggregate,
        descriptor,
        foundation,
    })
}

fn component(
    role: &str,
    source: &Source,
) -> Result<FpsPresentationPackageV2ComponentBinding, RuntimeError> {
    let mut part_ids = source
        .descriptor
        .part_revisions
        .iter()
        .map(|part| part.part_id.clone())
        .collect::<Vec<_>>();
    let mut revision_hashes = source
        .descriptor
        .part_revisions
        .iter()
        .map(|part| part.revision_object_sha256.clone())
        .collect::<Vec<_>>();
    part_ids.sort();
    revision_hashes.sort();
    let mut value = FpsPresentationPackageV2ComponentBinding {
        schema_version: FPS_PRESENTATION_PACKAGE_V2_COMPONENT_SCHEMA_VERSION.to_owned(),
        component_role: role.to_owned(),
        source_asset_id: source.foundation.asset_id.clone(),
        source_asset_sha256: source.foundation.asset_sha256.clone(),
        source_asset_role: source.foundation.asset_role.clone(),
        materialization_id: source.aggregate.idempotency_key.clone(),
        materialization_descriptor_object_sha256: source.aggregate.descriptor_object_sha256.clone(),
        materialization_descriptor_sha256: source.aggregate.descriptor_canonical_sha256.clone(),
        foundation_package_object_sha256: source
            .foundation
            .fps_presentation_package_object_sha256
            .clone(),
        socket_map_object_sha256: source.foundation.socket_map_object_sha256.clone(),
        rig_map_object_sha256: source.foundation.rig_map_object_sha256.clone(),
        part_ids,
        part_revision_object_sha256s: revision_hashes,
        part_revision_summary_sha256: source.aggregate.part_revision_summary_sha256.clone(),
        part_count: source.aggregate.part_count,
        vertex_count: source.aggregate.vertex_count,
        face_count: source.aggregate.face_count,
        editable_authoring_mesh_v2: true,
        canonical_sha256: String::new(),
    };
    value.canonical_sha256 = canonical_hash_without(&value, "canonical_sha256")?;
    Ok(value)
}

fn animation_clips(runtime: &Runtime, source: &Source) -> Result<Vec<String>, RuntimeError> {
    let bytes = runtime.cas_read_bounded(&source.foundation.rig_map_object_sha256, 1_048_576)?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
    let mut clips = value
        .get("source_animation_clips")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("animation rig map has no source_animation_clips"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("animation clip id is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    clips.sort();
    clips.dedup();
    if clips.is_empty() {
        return Err(invalid("animation source contains no typed clips"));
    }
    Ok(clips)
}

fn pipeline() -> FpsPresentationProductionPipelineBinding {
    FpsPresentationProductionPipelineBinding {
        formal_high_entrypoint: "production_weapon_formal_high_prepare".to_owned(),
        low_cage_bake_entrypoint: "production_weapon_high_low_bake_prepare".to_owned(),
        hero_uv_entrypoint: "hero_uv_durable_prepare".to_owned(),
        fps_validation_entrypoint: "game_asset_delivery_prepare".to_owned(),
        engine_validation_entrypoint: "commercial_engine_import_prepare".to_owned(),
        human_hero_review_entrypoint: "human_visual_review_submit".to_owned(),
        formal_high_status: "BLOCKED_SECONDARY_FORM_APPROVAL_AND_CANDIDATE_BINDING".to_owned(),
        low_status: "NOT_RUN".to_owned(),
        hero_uv_status: "NOT_RUN".to_owned(),
        cage_bake_status: "NOT_RUN".to_owned(),
        fps_validation_status: "NOT_RUN".to_owned(),
        engine_validation_status: "NOT_RUN".to_owned(),
        human_hero_review_status: "NOT_RUN".to_owned(),
        blocker: "COMPOSITE_PACKAGE_IS_SOURCE_ONLY_NO_PROMOTABLE_CANDIDATE".to_owned(),
    }
}

fn load_package(
    runtime: &Runtime,
    record: &FpsPresentationPackageV2StoreRecord,
) -> Result<FpsPresentationPackageV2, RuntimeError> {
    let bytes =
        runtime.cas_read_bounded(&record.package_object_sha256, package_store::MAX_JSON_BYTES)?;
    let package: FpsPresentationPackageV2 =
        serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
    if package.canonical_sha256 != record.package_canonical_sha256
        || canonical_hash_without(&package, "canonical_sha256")? != package.canonical_sha256
    {
        return Err(invalid("package restart hash differs"));
    }
    Ok(package)
}

fn prepare_result(
    record: &FpsPresentationPackageV2StoreRecord,
    package: FpsPresentationPackageV2,
    replayed: bool,
    write: bool,
) -> Result<Value, RuntimeError> {
    let mut result = FpsPresentationPackageV2PrepareResult {
        schema_version: FPS_PRESENTATION_PACKAGE_V2_PREPARE_RESULT_SCHEMA_VERSION.to_owned(),
        project_id: record.project_id.clone(),
        package_id: record.package_id.clone(),
        package_object_sha256: record.package_object_sha256.clone(),
        package_sha256: record.package_canonical_sha256.clone(),
        package,
        request_input_sha256: record.request_input_sha256.clone(),
        idempotency_key: record.idempotency_key.clone(),
        replayed,
        restart_hash_verified: true,
        runtime_write_performed: write,
        persistent_user_data_touched: write,
        canonical_sha256: String::new(),
    };
    result.canonical_sha256 = canonical_hash_without(&result, "canonical_sha256")?;
    let value = serde_json::to_value(result).map_err(|error| invalid(error.to_string()))?;
    if canonical_json_bytes(&value)
        .map_err(|error| invalid(error.to_string()))?
        .len() as u64
        >= FPS_PRESENTATION_PACKAGE_V2_MAX_RESPONSE_BYTES
    {
        return Err(invalid("response exceeds 1 MiB"));
    }
    Ok(value)
}

pub(crate) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    if let Some(record) = runtime
        .store
        .get_fps_presentation_package_v2(&request.project_id, &request.idempotency_key)?
    {
        if record.request_input_sha256 != request.input_sha256 {
            return Err(invalid("idempotency key is bound to another request"));
        }
        let package = load_package(runtime, &record)?;
        return prepare_result(&record, package, true, false);
    }
    let weapon = source(
        runtime,
        &request.project_id,
        &request.weapon_materialization_id,
        &request.weapon_descriptor_sha256,
    )?;
    let arms = source(
        runtime,
        &request.project_id,
        &request.arms_materialization_id,
        &request.arms_descriptor_sha256,
    )?;
    let animation = source(
        runtime,
        &request.project_id,
        &request.animation_materialization_id,
        &request.animation_descriptor_sha256,
    )?;
    if weapon.foundation.asset_id != "pichuliru-weapon-west"
        || weapon.foundation.asset_role != "rigged-weapon-semantic-source"
        || arms.foundation.asset_id != "wrad-arms"
        || arms.foundation.asset_role != "first-person-armature-source"
        || animation.foundation.asset_id != "lightning-low-pbr"
        || animation.foundation.asset_role != "high-low-bake-pbr-animation-benchmark"
    {
        return Err(invalid(
            "component roles do not match the closed production foundation",
        ));
    }
    if weapon.foundation.coordinate_spec_sha256 != arms.foundation.coordinate_spec_sha256
        || weapon.foundation.coordinate_spec_sha256 != animation.foundation.coordinate_spec_sha256
    {
        return Err(invalid("component coordinate specifications differ"));
    }
    let weapon_component = component("weapon", &weapon)?;
    let arms_component = component("first-person-arms", &arms)?;
    let animation_component = component("animation-source", &animation)?;
    let clips = animation_clips(runtime, &animation)?;
    let required = FPS_PRESENTATION_PACKAGE_V2_REQUIRED_CLIPS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    let available = clips.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let missing = required
        .iter()
        .filter(|value| !available.contains(value.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let summary = canonical_json_hash(&json!([
        weapon_component.part_revision_summary_sha256,
        arms_component.part_revision_summary_sha256,
        animation_component.part_revision_summary_sha256,
    ]));
    let package_id = request.idempotency_key.clone();
    let mut package = FpsPresentationPackageV2 {
        schema_version: FPS_PRESENTATION_PACKAGE_V2_SCHEMA_VERSION.to_owned(),
        package_id: package_id.clone(),
        project_id: request.project_id.clone(),
        weapon: weapon_component,
        first_person_arms: arms_component,
        animation_source: animation_component,
        coordinate_spec_sha256: weapon.foundation.coordinate_spec_sha256.clone(),
        weapon_socket_map_object_sha256: weapon.foundation.socket_map_object_sha256.clone(),
        weapon_rig_map_object_sha256: weapon.foundation.rig_map_object_sha256.clone(),
        arms_rig_map_object_sha256: arms.foundation.rig_map_object_sha256.clone(),
        animation_rig_map_object_sha256: animation.foundation.rig_map_object_sha256.clone(),
        source_package_object_sha256s: vec![
            weapon
                .foundation
                .fps_presentation_package_object_sha256
                .clone(),
            arms.foundation
                .fps_presentation_package_object_sha256
                .clone(),
            animation
                .foundation
                .fps_presentation_package_object_sha256
                .clone(),
        ],
        aggregate_part_revision_summary_sha256: summary,
        aggregate_part_count: weapon.aggregate.part_count
            + arms.aggregate.part_count
            + animation.aggregate.part_count,
        aggregate_vertex_count: weapon.aggregate.vertex_count
            + arms.aggregate.vertex_count
            + animation.aggregate.vertex_count,
        aggregate_face_count: weapon.aggregate.face_count
            + arms.aggregate.face_count
            + animation.aggregate.face_count,
        source_animation_clip_ids: clips,
        required_clip_ids: required,
        missing_required_clip_ids: missing,
        animation_binding_status: "SOURCE_CLIPS_BOUND_REQUIRED_CLIPS_INCOMPLETE".to_owned(),
        socket_binding_status: "WEAPON_SOCKET_MAP_BOUND_ARMS_GRIP_MAPPING_BOUND".to_owned(),
        rig_binding_status: "MULTI_SOURCE_RIG_MAPS_BOUND_REST_POSE_NOT_VALIDATED".to_owned(),
        authoring_status: FPS_PRESENTATION_PACKAGE_V2_STATUS.to_owned(),
        production_pipeline: pipeline(),
        package_policy: FPS_PRESENTATION_PACKAGE_V2_POLICY.to_owned(),
        status: FPS_PRESENTATION_PACKAGE_V2_STATUS.to_owned(),
        quality_status: FPS_PRESENTATION_PACKAGE_V2_QUALITY_STATUS.to_owned(),
        review_status: FPS_PRESENTATION_PACKAGE_V2_REVIEW_STATUS.to_owned(),
        promotion_eligible: false,
        candidate_created: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        actual_engine_roundtrip: false,
        human_review_performed: false,
        canonicalization_policy: FPS_PRESENTATION_PACKAGE_V2_CANONICALIZATION_POLICY.to_owned(),
        canonical_sha256: String::new(),
    };
    package.canonical_sha256 = canonical_hash_without(&package, "canonical_sha256")?;
    let bytes = canonical_json_bytes(
        &serde_json::to_value(&package).map_err(|error| invalid(error.to_string()))?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let reservation = runtime.store.begin_cas_reservation();
    let object = runtime.store.put_object_reserved(
        &reservation,
        &bytes,
        None,
        package_store::JSON_MIME,
        package_store::OBJECT_KIND,
        &now_string(),
    )?;
    let mut record = FpsPresentationPackageV2StoreRecord {
        schema_version: package_store::RECORD_SCHEMA_VERSION.to_owned(),
        project_id: request.project_id.clone(),
        package_id,
        idempotency_key: request.idempotency_key.clone(),
        package_object_sha256: object.record.sha256.clone(),
        package_canonical_sha256: package.canonical_sha256.clone(),
        weapon_materialization_id: request.weapon_materialization_id,
        weapon_descriptor_sha256: request.weapon_descriptor_sha256,
        arms_materialization_id: request.arms_materialization_id,
        arms_descriptor_sha256: request.arms_descriptor_sha256,
        animation_materialization_id: request.animation_materialization_id,
        animation_descriptor_sha256: request.animation_descriptor_sha256,
        request_input_sha256: request.input_sha256,
        status: FPS_PRESENTATION_PACKAGE_V2_STATUS.to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    record.canonical_sha256 = canonical_hash_without(&record, "canonical_sha256")?;
    match runtime
        .store
        .record_fps_presentation_package_v2_with_replay(&record, &package, &object.record)
    {
        Ok((stored, replayed)) => {
            runtime
                .store
                .release_cas_reservation_object(&reservation, &object, false)?;
            let stored_package = load_package(runtime, &stored)?;
            prepare_result(&stored, stored_package, replayed, !replayed)
        }
        Err(error) => {
            let _ = runtime
                .store
                .release_cas_reservation_object(&reservation, &object, true);
            Err(error.into())
        }
    }
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let record = runtime
        .store
        .get_fps_presentation_package_v2(&request.project_id, &request.package_id)?
        .ok_or_else(|| invalid("composite package is not durable"))?;
    let package = load_package(runtime, &record)?;
    if request
        .package_sha256
        .as_deref()
        .is_some_and(|hash| hash != package.canonical_sha256)
    {
        return Err(invalid("package hash differs"));
    }
    let mut result = FpsPresentationPackageV2GetResult {
        schema_version: FPS_PRESENTATION_PACKAGE_V2_GET_RESULT_SCHEMA_VERSION.to_owned(),
        project_id: record.project_id,
        package_id: record.package_id,
        package_object_sha256: record.package_object_sha256,
        package_sha256: record.package_canonical_sha256,
        package,
        request_input_sha256: request.input_sha256,
        replayed: true,
        restart_hash_verified: true,
        runtime_write_performed: false,
        persistent_user_data_touched: false,
        canonical_sha256: String::new(),
    };
    result.canonical_sha256 = canonical_hash_without(&result, "canonical_sha256")?;
    serde_json::to_value(result).map_err(|error| invalid(error.to_string()))
}

pub(crate) fn production_preflight_get(
    runtime: &Runtime,
    value: &Value,
) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let record = runtime
        .store
        .get_fps_presentation_package_v2(&request.project_id, &request.package_id)?
        .ok_or_else(|| invalid("composite package is not durable"))?;
    let package = load_package(runtime, &record)?;
    if request
        .package_sha256
        .as_deref()
        .is_some_and(|hash| hash != package.canonical_sha256)
    {
        return Err(invalid("package hash differs"));
    }
    let mut gates = BTreeMap::new();
    gates.insert("editable_composite".to_owned(), "PASS".to_owned());
    gates.insert(
        "formal_high".to_owned(),
        package.production_pipeline.formal_high_status.clone(),
    );
    gates.insert(
        "low".to_owned(),
        package.production_pipeline.low_status.clone(),
    );
    gates.insert(
        "hero_uv".to_owned(),
        package.production_pipeline.hero_uv_status.clone(),
    );
    gates.insert(
        "cage_bake".to_owned(),
        package.production_pipeline.cage_bake_status.clone(),
    );
    gates.insert(
        "fps_validation".to_owned(),
        package.production_pipeline.fps_validation_status.clone(),
    );
    gates.insert(
        "engine_validation".to_owned(),
        package.production_pipeline.engine_validation_status.clone(),
    );
    gates.insert(
        "human_hero_review".to_owned(),
        package.production_pipeline.human_hero_review_status.clone(),
    );
    let mut result = FpsPresentationPackageV2ProductionPreflightResult {
        schema_version: FPS_PRESENTATION_PACKAGE_V2_PRODUCTION_PREFLIGHT_RESULT_SCHEMA_VERSION.to_owned(),
        project_id: package.project_id.clone(), package_id: package.package_id.clone(), package_object_sha256: record.package_object_sha256,
        package_sha256: package.canonical_sha256, editable_composite_ready: true, gates,
        next_action: "BIND_COMPOSITE_TO_REVIEWABLE_CANDIDATE_THEN_REQUIRE_SECONDARY_FORM_APPROVAL_BEFORE_FORMAL_HIGH".to_owned(),
        runtime_write_performed: false, persistent_user_data_touched: false,
        canonicalization_policy: FPS_PRESENTATION_PACKAGE_V2_CANONICALIZATION_POLICY.to_owned(), canonical_sha256: String::new(),
    };
    result.canonical_sha256 = canonical_hash_without(&result, "canonical_sha256")?;
    serde_json::to_value(result).map_err(|error| invalid(error.to_string()))
}
