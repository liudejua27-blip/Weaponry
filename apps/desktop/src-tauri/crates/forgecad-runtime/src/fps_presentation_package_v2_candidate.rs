//! Runtime-owned package weapon -> reviewable candidate derivation.

use super::{authoring_mesh_v2::AuthoringMeshV2Revision, now_string, Runtime, RuntimeError};
use forgecad_contracts::*;
use forgecad_core::{canonical_json_bytes, canonical_json_hash};
use forgecad_store::{
    fps_presentation_package_v2 as package_store, FpsPresentationPackageV2CandidateStoreRecord,
};
use serde_json::{json, Value};

const INTERNAL_MAX_BYTES: usize = 64 * 1024 * 1024;
fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_REJECTED: {}",
        message.into()
    ))
}
fn hash_without<T: serde::Serialize>(value: &T, field: &str) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(value).map_err(|e| invalid(e.to_string()))?;
    value
        .as_object_mut()
        .ok_or_else(|| invalid("canonical payload must be an object"))?
        .insert(field.to_owned(), Value::String(String::new()));
    Ok(canonical_json_hash(&value))
}
fn parse_prepare(
    value: &Value,
) -> Result<FpsPresentationPackageV2CandidatePrepareRequest, RuntimeError> {
    let request: FpsPresentationPackageV2CandidatePrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|e| invalid(format!("prepare request is not closed: {e}")))?;
    if request.schema_version
        != FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_PREPARE_REQUEST_SCHEMA_VERSION
        || !is_opaque_id(&request.project_id)
        || !is_opaque_id(&request.package_id)
        || !is_opaque_id(&request.idempotency_key)
        || !is_sha256(&request.package_sha256)
        || !is_sha256(&request.input_sha256)
        || request.policy != FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_POLICY
        || request.max_response_bytes != FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_MAX_RESPONSE_BYTES
        || request.runtime_write_performed
        || request.writer_policy != FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_WRITER_POLICY
        || request.canonicalization_policy
            != FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_CANONICALIZATION_POLICY
        || hash_without(&request, "input_sha256")? != request.input_sha256
    {
        return Err(invalid(
            "prepare request identity, policy, or hash is invalid",
        ));
    }
    Ok(request)
}
fn parse_get(value: &Value) -> Result<FpsPresentationPackageV2CandidateGetRequest, RuntimeError> {
    let request: FpsPresentationPackageV2CandidateGetRequest =
        serde_json::from_value(value.clone())
            .map_err(|e| invalid(format!("get request is not closed: {e}")))?;
    if request.schema_version != FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_GET_REQUEST_SCHEMA_VERSION
        || !is_opaque_id(&request.project_id)
        || !is_opaque_id(&request.package_id)
        || request
            .binding_sha256
            .as_deref()
            .is_some_and(|v| !is_sha256(v))
        || request.runtime_write_performed
        || request.persistent_user_data_touched
        || !is_sha256(&request.input_sha256)
        || hash_without(&request, "input_sha256")? != request.input_sha256
    {
        return Err(invalid("get request identity or hash is invalid"));
    }
    Ok(request)
}
fn load_binding(
    runtime: &Runtime,
    record: &FpsPresentationPackageV2CandidateStoreRecord,
) -> Result<FpsPresentationPackageV2CandidateBinding, RuntimeError> {
    let bytes =
        runtime.cas_read_bounded(&record.binding_object_sha256, package_store::MAX_JSON_BYTES)?;
    let binding: FpsPresentationPackageV2CandidateBinding = serde_json::from_slice(&bytes)
        .map_err(|e| invalid(format!("binding CAS JSON is invalid: {e}")))?;
    if binding.canonical_sha256 != record.binding_canonical_sha256
        || hash_without(&binding, "canonical_sha256")? != binding.canonical_sha256
    {
        return Err(invalid("binding restart hash differs"));
    }
    let candidate = runtime
        .candidate(&binding.candidate_id)?
        .ok_or_else(|| invalid("bound candidate disappeared"))?;
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&binding.candidate_id)?
        .ok_or_else(|| invalid("bound geometry evidence disappeared"))?;
    if candidate.canonical_sha256 != binding.candidate_state_sha256
        || candidate.state != "reviewable"
        || candidate.prepared_object_sha256.as_deref()
            != Some(binding.candidate_artifact_sha256.as_str())
        || evidence.canonical_sha256 != binding.geometry_candidate_evidence_sha256
        || evidence.geometry_program_sha256 != binding.geometry_program_sha256
        || evidence.geometry_program_object_sha256 != binding.geometry_program_object_sha256
    {
        return Err(invalid(
            "candidate or geometry evidence restart binding differs",
        ));
    }
    Ok(binding)
}
fn prepare_result(
    record: &FpsPresentationPackageV2CandidateStoreRecord,
    binding: FpsPresentationPackageV2CandidateBinding,
    replayed: bool,
    write: bool,
) -> Result<Value, RuntimeError> {
    let mut result = FpsPresentationPackageV2CandidatePrepareResult {
        schema_version: FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_PREPARE_RESULT_SCHEMA_VERSION
            .to_owned(),
        binding_object_sha256: record.binding_object_sha256.clone(),
        binding,
        request_input_sha256: record.request_input_sha256.clone(),
        idempotency_key: record.idempotency_key.clone(),
        replayed,
        restart_hash_verified: true,
        runtime_write_performed: write,
        persistent_user_data_touched: write,
        canonical_sha256: String::new(),
    };
    result.canonical_sha256 = hash_without(&result, "canonical_sha256")?;
    let value = serde_json::to_value(result).map_err(|e| invalid(e.to_string()))?;
    if canonical_json_bytes(&value)
        .map_err(|e| invalid(e.to_string()))?
        .len() as u64
        >= FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_MAX_RESPONSE_BYTES
    {
        return Err(invalid("prepare response exceeds 1 MiB"));
    }
    Ok(value)
}
fn geometry_program(
    project_id: &str,
    package_sha256: &str,
    part_id: &str,
    zone: &str,
    revision: &AuthoringMeshRevision,
) -> Result<Value, RuntimeError> {
    let parameters =
        super::authoring_mesh_v2_geometry::authoring_mesh_v2_welded_geometry_parameters(
            revision, [0.0; 3], [0.0; 3],
        )?;
    let triangles = parameters
        .get("faces")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("welded projection omitted faces"))?
        .iter()
        .try_fold(0u64, |sum, face| {
            let count = u64::try_from(
                face.get("loop_ids")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid("welded face omitted loops"))?
                    .len(),
            )
            .map_err(|_| invalid("face count overflow"))?;
            sum.checked_add(count.saturating_sub(2))
                .ok_or_else(|| invalid("triangle count overflow"))
        })?;
    if triangles == 0 || triangles > 250_000 {
        return Err(invalid(
            "foundation weapon triangle count exceeds production budget",
        ));
    }
    let plan = canonical_json_hash(
        &json!({"schema_version":"FpsPresentationPackageV2CandidateRepresentationPlan@1","package_sha256":package_sha256,"revision_id":revision.revision_id.0,"revision_sha256":revision.canonical_sha256,"part_id":part_id,"material_zone_id":zone}),
    );
    let node_id = format!("package-weapon-{}", &revision.canonical_sha256[..16]);
    let mut program = json!({"schema_version":"GeometryProgram@2","project_id":project_id,"representation_plan_sha256":plan,"operator_catalog_sha256":super::operator_catalog_sha256(),"units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},"budgets":{"max_nodes":1,"max_triangles":triangles,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},"nodes":[{"node_id":node_id,"operator_id":"forgecad.geometry.authoring-mesh@1","inputs":[],"parameters":parameters}],"part_outputs":[{"part_id":part_id,"input_node_ids":[node_id],"material_zone_id":zone,"solid":false}]});
    program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
    Ok(program)
}
pub(crate) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    if let Some(record) = runtime
        .store
        .get_fps_presentation_package_v2_candidate(&request.project_id, &request.package_id)?
    {
        if record.request_input_sha256 != request.input_sha256
            || record.idempotency_key != request.idempotency_key
            || record.package_sha256 != request.package_sha256
        {
            return Err(invalid("idempotency key or package hash differs"));
        }
        let binding = load_binding(runtime, &record)?;
        return prepare_result(&record, binding, true, false);
    }
    let package = runtime
        .store
        .get_fps_presentation_package_v2(&request.project_id, &request.package_id)?
        .ok_or_else(|| invalid("composite package is not durable"))?;
    if package.package_canonical_sha256 != request.package_sha256 {
        return Err(invalid("package hash differs"));
    }
    let aggregate = runtime
        .store
        .get_foundation_authoring_mesh_v2_materialization(
            &request.project_id,
            &package.weapon_materialization_id,
        )?
        .ok_or_else(|| invalid("weapon materialization disappeared"))?;
    if aggregate.descriptor_canonical_sha256 != package.weapon_descriptor_sha256 {
        return Err(invalid("weapon descriptor hash differs"));
    }
    let descriptor_bytes = runtime.cas_read_bounded(
        &aggregate.descriptor_object_sha256,
        package_store::MAX_JSON_BYTES,
    )?;
    let descriptor:forgecad_store::foundation_authoring_mesh_v2_materialization::FoundationAuthoringMeshV2MaterializationDescriptor=serde_json::from_slice(&descriptor_bytes).map_err(|e|invalid(format!("weapon descriptor is invalid: {e}")))?;
    if descriptor.canonical_sha256 != package.weapon_descriptor_sha256
        || descriptor.part_revisions.len() != 1
    {
        return Err(invalid("candidate bridge requires one exact weapon Part"));
    }
    let part = &descriptor.part_revisions[0];
    let revision_bytes =
        runtime.cas_read_bounded(&part.revision_object_sha256, INTERNAL_MAX_BYTES as u64)?;
    let revision: AuthoringMeshRevision = serde_json::from_slice(&revision_bytes)
        .map_err(|e| invalid(format!("weapon AuthoringMesh revision is invalid: {e}")))?;
    AuthoringMeshV2Revision::from_record(revision.clone())?;
    if revision.revision_id.0 != part.revision_id
        || revision.canonical_sha256 != part.revision_sha256
    {
        return Err(invalid("weapon revision descriptor binding differs"));
    }
    let source = revision
        .foundation_source_binding
        .as_ref()
        .ok_or_else(|| invalid("weapon revision has no foundation provenance"))?;
    if source.project_id != request.project_id
        || source.materialization_id != package.weapon_materialization_id
        || source.part_id != part.part_id
        || !source.source_only
        || source.review_status != "DRAFT_UNREVIEWED"
    {
        return Err(invalid("weapon foundation provenance differs"));
    }
    let program = geometry_program(
        &request.project_id,
        &request.package_sha256,
        &source.part_id,
        &source.material_zone_id,
        &revision,
    )?;
    let base = runtime
        .store
        .latest_version_for_project(&request.project_id)?
        .map(|v| v.version_id);
    let geometry_key = format!("fpspkg-candidate-{}", &request.input_sha256[..32]);
    let prepared = runtime.prepare_geometry_candidate_exact_bounded(
        &request.project_id,
        base.as_deref(),
        &geometry_key,
        json!({"typed":"geometry","geometry_program":program}),
        INTERNAL_MAX_BYTES,
    )?;
    let candidate: CandidateRecord = serde_json::from_value(
        prepared
            .get("candidate")
            .cloned()
            .ok_or_else(|| invalid("geometry prepare omitted candidate"))?,
    )
    .map_err(|e| invalid(e.to_string()))?;
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&candidate.candidate_id)?
        .ok_or_else(|| invalid("geometry prepare omitted durable evidence"))?;
    if candidate.state != "reviewable"
        || !candidate.quality_hard_gate_passed
        || candidate.prepared_object_sha256.as_deref()
            != Some(evidence.artifact_object_sha256.as_str())
    {
        return Err(invalid("geometry candidate is not structurally reviewable"));
    }
    let mut binding = FpsPresentationPackageV2CandidateBinding {
        schema_version: FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_BINDING_SCHEMA_VERSION.to_owned(),
        project_id: request.project_id.clone(),
        package_id: request.package_id.clone(),
        package_object_sha256: package.package_object_sha256.clone(),
        package_sha256: request.package_sha256.clone(),
        weapon_materialization_id: package.weapon_materialization_id.clone(),
        weapon_materialization_descriptor_sha256: package.weapon_descriptor_sha256.clone(),
        weapon_part_id: source.part_id.clone(),
        weapon_material_zone_id: source.material_zone_id.clone(),
        weapon_authoring_mesh_revision_id: revision.revision_id.0.clone(),
        weapon_authoring_mesh_revision_object_sha256: part.revision_object_sha256.clone(),
        weapon_authoring_mesh_revision_sha256: revision.canonical_sha256.clone(),
        candidate_id: candidate.candidate_id.clone(),
        candidate_state_sha256: candidate.canonical_sha256.clone(),
        candidate_state: "reviewable".to_owned(),
        candidate_artifact_sha256: evidence.artifact_object_sha256.clone(),
        geometry_program_object_sha256: evidence.geometry_program_object_sha256.clone(),
        geometry_program_sha256: evidence.geometry_program_sha256.clone(),
        geometry_candidate_evidence_sha256: evidence.canonical_sha256.clone(),
        geometry_integrity_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
        form_stage: "candidate-reviewable".to_owned(),
        secondary_form_approved: false,
        formal_high_status: "BLOCKED_SECONDARY_FORM_APPROVAL".to_owned(),
        quality_status: "structural_only".to_owned(),
        visual_review_status: "NOT_RUN".to_owned(),
        engine_validation_status: "NOT_RUN".to_owned(),
        human_review_status: "NOT_RUN".to_owned(),
        promotion_eligible: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        policy: FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_POLICY.to_owned(),
        canonicalization_policy: FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_CANONICALIZATION_POLICY
            .to_owned(),
        canonical_sha256: String::new(),
    };
    binding.canonical_sha256 = hash_without(&binding, "canonical_sha256")?;
    let bytes =
        canonical_json_bytes(&serde_json::to_value(&binding).map_err(|e| invalid(e.to_string()))?)
            .map_err(|e| invalid(e.to_string()))?;
    let reservation = runtime.store.begin_cas_reservation();
    let object = runtime.store.put_object_reserved(
        &reservation,
        &bytes,
        None,
        package_store::JSON_MIME,
        package_store::CANDIDATE_OBJECT_KIND,
        &now_string(),
    )?;
    let mut record = FpsPresentationPackageV2CandidateStoreRecord {
        schema_version: package_store::CANDIDATE_RECORD_SCHEMA_VERSION.to_owned(),
        project_id: request.project_id,
        package_id: request.package_id,
        package_sha256: request.package_sha256,
        candidate_id: candidate.candidate_id,
        candidate_state_sha256: candidate.canonical_sha256,
        binding_object_sha256: object.record.sha256.clone(),
        binding_canonical_sha256: binding.canonical_sha256.clone(),
        idempotency_key: request.idempotency_key,
        request_input_sha256: request.input_sha256,
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    record.canonical_sha256 = hash_without(&record, "canonical_sha256")?;
    match runtime
        .store
        .record_fps_presentation_package_v2_candidate_with_replay(&record, &binding, &object.record)
    {
        Ok((stored, replayed)) => {
            runtime
                .store
                .release_cas_reservation_object(&reservation, &object, false)?;
            let stored_binding = load_binding(runtime, &stored)?;
            prepare_result(&stored, stored_binding, replayed, !replayed)
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
        .get_fps_presentation_package_v2_candidate(&request.project_id, &request.package_id)?
        .ok_or_else(|| invalid("candidate binding is not durable"))?;
    if request
        .binding_sha256
        .as_deref()
        .is_some_and(|hash| hash != record.binding_canonical_sha256)
    {
        return Err(invalid("binding hash differs"));
    }
    let binding = load_binding(runtime, &record)?;
    let mut result = FpsPresentationPackageV2CandidateGetResult {
        schema_version: FPS_PRESENTATION_PACKAGE_V2_CANDIDATE_GET_RESULT_SCHEMA_VERSION.to_owned(),
        binding_object_sha256: record.binding_object_sha256,
        binding,
        request_input_sha256: request.input_sha256,
        replayed: true,
        restart_hash_verified: true,
        runtime_write_performed: false,
        persistent_user_data_touched: false,
        canonical_sha256: String::new(),
    };
    result.canonical_sha256 = hash_without(&result, "canonical_sha256")?;
    serde_json::to_value(result).map_err(|e| invalid(e.to_string()))
}
