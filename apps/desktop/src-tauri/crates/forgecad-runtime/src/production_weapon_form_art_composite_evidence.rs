//! Runtime-owned six-view evidence attachment for an immutable composite
//! FormArt proposal.  This adapter evaluates the already prepared candidate,
//! stores CrossView/FormArt evidence, and appends a separate typed sidecar.
//! It never mutates the parent proposal or promotes the candidate.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, now_string, Runtime,
    RuntimeError,
};
use forgecad_store::{
    production_weapon_form_art_composite_evidence_record_canonical_sha256,
    ProductionWeaponFormArtCompositeEvidenceRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const PREPARE_SCHEMA: &str = "ProductionWeaponFormArtCompositeEvidencePrepareRequest@1";
const GET_SCHEMA: &str = "ProductionWeaponFormArtCompositeEvidenceGetRequest@1";
const PREPARE_OPERATION: &str = "forgecad.production.weapon.form-art-composite-evidence-prepare@1";
const GET_OPERATION: &str = "forgecad.production.weapon.form-art-composite-evidence-get@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PrepareRequest {
    schema_version: String,
    operation: String,
    composite_evidence_id: String,
    proposal_id: String,
    session_id: String,
    project_id: String,
    composite_proposal_record_canonical_sha256: String,
    composite_proposal_receipt_object_sha256: String,
    original_fresh_baseline_id: String,
    original_fresh_baseline_canonical_sha256: String,
    source_form_art_evidence_id: String,
    source_form_art_evidence_object_sha256: String,
    source_form_art_evidence_canonical_sha256: String,
    proposal_candidate_id: String,
    proposal_candidate_state_sha256: String,
    proposal_artifact_id: String,
    proposal_artifact_sha256: String,
    proposal_artifact_readback_object_sha256: String,
    proposal_artifact_readback_sha256: String,
    idempotency_key: String,
    max_response_bytes: u64,
    runtime_write_performed: bool,
    writer_policy: String,
    canonicalization_policy: String,
    input_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GetRequest {
    schema_version: String,
    operation: String,
    composite_evidence_id: String,
    proposal_id: String,
    session_id: String,
    project_id: String,
    composite_proposal_record_canonical_sha256: String,
    composite_proposal_receipt_object_sha256: String,
    original_fresh_baseline_id: String,
    original_fresh_baseline_canonical_sha256: String,
    source_form_art_evidence_id: String,
    source_form_art_evidence_object_sha256: String,
    source_form_art_evidence_canonical_sha256: String,
    proposal_candidate_id: String,
    proposal_candidate_state_sha256: String,
    proposal_artifact_id: String,
    proposal_artifact_sha256: String,
    proposal_artifact_readback_object_sha256: String,
    proposal_artifact_readback_sha256: String,
    max_response_bytes: u64,
    runtime_write_performed: bool,
    writer_policy: String,
    canonicalization_policy: String,
    input_sha256: String,
}

fn invalid(reason: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_FORM_ART_COMPOSITE_EVIDENCE_INVALID: {}",
        reason.into()
    ))
}

fn request_input_sha256<T: Serialize>(request: &T) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(request).map_err(|error| invalid(error.to_string()))?;
    value
        .as_object_mut()
        .ok_or_else(|| invalid("request object is unavailable"))?
        .remove("input_sha256");
    Ok(canonical_json_hash(&value))
}

fn validate_common(
    ids: &[&str],
    hashes: &[&str],
    max_response_bytes: u64,
    runtime_write_performed: bool,
    writer_policy: &str,
    canonicalization_policy: &str,
) -> Result<(), RuntimeError> {
    if ids.iter().any(|value| !is_opaque_id(value))
        || hashes.iter().any(|value| !is_sha256(value))
        || max_response_bytes != MAX_RESPONSE_BYTES
        || runtime_write_performed
        || writer_policy != WRITER_POLICY
        || canonicalization_policy != CANONICALIZATION_POLICY
    {
        return Err(invalid(
            "request identity, hash or transport policy differs",
        ));
    }
    Ok(())
}

fn parse_prepare(value: &Value) -> Result<PrepareRequest, RuntimeError> {
    let request: PrepareRequest =
        serde_json::from_value(value.clone()).map_err(|error| invalid(error.to_string()))?;
    if request.schema_version != PREPARE_SCHEMA || request.operation != PREPARE_OPERATION {
        return Err(invalid("prepare schema or operation differs"));
    }
    validate_common(
        &[
            &request.composite_evidence_id,
            &request.proposal_id,
            &request.session_id,
            &request.project_id,
            &request.original_fresh_baseline_id,
            &request.source_form_art_evidence_id,
            &request.proposal_candidate_id,
            &request.proposal_artifact_id,
            &request.idempotency_key,
        ],
        &[
            &request.composite_proposal_record_canonical_sha256,
            &request.composite_proposal_receipt_object_sha256,
            &request.original_fresh_baseline_canonical_sha256,
            &request.source_form_art_evidence_object_sha256,
            &request.source_form_art_evidence_canonical_sha256,
            &request.proposal_candidate_state_sha256,
            &request.proposal_artifact_sha256,
            &request.proposal_artifact_readback_object_sha256,
            &request.proposal_artifact_readback_sha256,
            &request.input_sha256,
        ],
        request.max_response_bytes,
        request.runtime_write_performed,
        &request.writer_policy,
        &request.canonicalization_policy,
    )?;
    if request.input_sha256 != request_input_sha256(&request)? {
        return Err(invalid("prepare input hash differs"));
    }
    Ok(request)
}

fn parse_get(value: &Value) -> Result<GetRequest, RuntimeError> {
    let request: GetRequest =
        serde_json::from_value(value.clone()).map_err(|error| invalid(error.to_string()))?;
    if request.schema_version != GET_SCHEMA || request.operation != GET_OPERATION {
        return Err(invalid("get schema or operation differs"));
    }
    validate_common(
        &[
            &request.composite_evidence_id,
            &request.proposal_id,
            &request.session_id,
            &request.project_id,
            &request.original_fresh_baseline_id,
            &request.source_form_art_evidence_id,
            &request.proposal_candidate_id,
            &request.proposal_artifact_id,
        ],
        &[
            &request.composite_proposal_record_canonical_sha256,
            &request.composite_proposal_receipt_object_sha256,
            &request.original_fresh_baseline_canonical_sha256,
            &request.source_form_art_evidence_object_sha256,
            &request.source_form_art_evidence_canonical_sha256,
            &request.proposal_candidate_state_sha256,
            &request.proposal_artifact_sha256,
            &request.proposal_artifact_readback_object_sha256,
            &request.proposal_artifact_readback_sha256,
            &request.input_sha256,
        ],
        request.max_response_bytes,
        request.runtime_write_performed,
        &request.writer_policy,
        &request.canonicalization_policy,
    )?;
    if request.input_sha256 != request_input_sha256(&request)? {
        return Err(invalid("get input hash differs"));
    }
    Ok(request)
}

fn validate_parent_scope(
    parent: &forgecad_store::ProductionWeaponFormArtCompositeProposalStoreRecord,
    project_id: &str,
    proposal_id: &str,
    session_id: &str,
    parent_canonical_sha256: &str,
    parent_receipt_object_sha256: &str,
    proposal_candidate_id: &str,
    proposal_candidate_state_sha256: &str,
    proposal_artifact_id: &str,
    proposal_artifact_sha256: &str,
    proposal_artifact_readback_object_sha256: &str,
    proposal_artifact_readback_sha256: &str,
) -> Result<(), RuntimeError> {
    if parent.project_id != project_id
        || parent.proposal_id != proposal_id
        || parent.session_id != session_id
        || parent.canonical_sha256 != parent_canonical_sha256
        || parent.receipt_object_sha256 != parent_receipt_object_sha256
        || parent.proposal_candidate_id != proposal_candidate_id
        || parent.proposal_candidate_state_sha256 != proposal_candidate_state_sha256
        || parent.proposal_artifact_sha256 != proposal_artifact_id
        || parent.proposal_artifact_sha256 != proposal_artifact_sha256
        || parent.proposal_artifact_readback_object_sha256
            != proposal_artifact_readback_object_sha256
        || parent.proposal_artifact_readback_sha256 != proposal_artifact_readback_sha256
    {
        return Err(invalid("composite parent or proposal scope differs"));
    }
    Ok(())
}

fn projection(
    request: &GetRequest,
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
    parent: &forgecad_store::ProductionWeaponFormArtCompositeProposalStoreRecord,
    replayed: bool,
    runtime_write_performed: bool,
) -> Result<Value, RuntimeError> {
    let baseline = request.original_fresh_baseline_id.as_str();
    let form_art = request.source_form_art_evidence_id.as_str();
    let proposal_evidence = runtime_projection_placeholder(record);
    Ok(json!({
        "schema_version":if runtime_write_performed {"ProductionWeaponFormArtCompositeEvidencePrepareResult@1"} else {"ProductionWeaponFormArtCompositeEvidenceGetResult@1"},
        "operation":if runtime_write_performed {PREPARE_OPERATION} else {GET_OPERATION},
        "composite_evidence_id":record.attachment_id,
        "proposal_id":record.proposal_id,
        "session_id":parent.session_id,
        "project_id":record.project_id,
        "composite_proposal_record_canonical_sha256":record.parent_record_canonical_sha256,
        "composite_proposal_receipt_object_sha256":record.parent_receipt_object_sha256,
        "original_fresh_baseline_id":baseline,
        "original_fresh_baseline_canonical_sha256":request.original_fresh_baseline_canonical_sha256,
        "source_form_art_evidence_id":form_art,
        "source_form_art_evidence_object_sha256":request.source_form_art_evidence_object_sha256,
        "source_form_art_evidence_canonical_sha256":request.source_form_art_evidence_canonical_sha256,
        "proposal_candidate_id":parent.proposal_candidate_id,
        "proposal_candidate_state_sha256":parent.proposal_candidate_state_sha256,
        "proposal_artifact_id":parent.proposal_artifact_sha256,
        "proposal_artifact_sha256":parent.proposal_artifact_sha256,
        "proposal_artifact_readback_object_sha256":parent.proposal_artifact_readback_object_sha256,
        "proposal_artifact_readback_sha256":parent.proposal_artifact_readback_sha256,
        "proposal_form_art_evidence_receipt_object_sha256":record.proposal_form_art_evidence_receipt_object_sha256,
        "cross_view_evidence_bundle_sha256":record.cross_view_evidence_bundle_sha256,
        "proposal_form_art_evidence":proposal_evidence,
        "view_kinds":["front","back","left","right","top","rear-three-quarter"],
        "aov_count_per_view":9,
        "aov_count":54,
        "status":record.status,
        "quality_status":record.quality_status,
        "candidate_confirm_allowed":false,
        "secondary_form_approved":"NOT_CREATED",
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "receipt_object_sha256":record.attachment_receipt_object_sha256,
        "record_canonical_sha256":record.canonical_sha256,
        "replayed":replayed,
        "restart_hash_verified":!runtime_write_performed,
        "runtime_write_performed":runtime_write_performed,
        "persistent_user_data_touched":runtime_write_performed
    }))
}

fn runtime_projection_placeholder(
    record: &ProductionWeaponFormArtCompositeEvidenceRecord,
) -> Value {
    json!({
        "receipt_object_sha256":record.proposal_form_art_evidence_receipt_object_sha256,
        "status":"DURABLE_HASH_BOUND_READBACK",
        "aov_bytes_in_summary":false
    })
}

pub(crate) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    if let Some(existing) = runtime
        .store
        .get_production_weapon_form_art_composite_evidence_by_idempotency(
            &request.project_id,
            &request.idempotency_key,
        )?
    {
        let get_request = GetRequest {
            schema_version: GET_SCHEMA.to_owned(),
            operation: GET_OPERATION.to_owned(),
            composite_evidence_id: request.composite_evidence_id,
            proposal_id: request.proposal_id,
            session_id: request.session_id,
            project_id: request.project_id,
            composite_proposal_record_canonical_sha256: request
                .composite_proposal_record_canonical_sha256,
            composite_proposal_receipt_object_sha256: request
                .composite_proposal_receipt_object_sha256,
            original_fresh_baseline_id: request.original_fresh_baseline_id,
            original_fresh_baseline_canonical_sha256: request
                .original_fresh_baseline_canonical_sha256,
            source_form_art_evidence_id: request.source_form_art_evidence_id,
            source_form_art_evidence_object_sha256: request.source_form_art_evidence_object_sha256,
            source_form_art_evidence_canonical_sha256: request
                .source_form_art_evidence_canonical_sha256,
            proposal_candidate_id: request.proposal_candidate_id,
            proposal_candidate_state_sha256: request.proposal_candidate_state_sha256,
            proposal_artifact_id: request.proposal_artifact_id,
            proposal_artifact_sha256: request.proposal_artifact_sha256,
            proposal_artifact_readback_object_sha256: request
                .proposal_artifact_readback_object_sha256,
            proposal_artifact_readback_sha256: request.proposal_artifact_readback_sha256,
            max_response_bytes: MAX_RESPONSE_BYTES,
            runtime_write_performed: false,
            writer_policy: WRITER_POLICY.to_owned(),
            canonicalization_policy: CANONICALIZATION_POLICY.to_owned(),
            input_sha256: String::new(),
        };
        let parent = runtime
            .store
            .get_production_weapon_form_art_composite_proposal(
                &existing.project_id,
                &existing.proposal_id,
            )?
            .ok_or_else(|| invalid("composite parent disappeared on replay"))?;
        return projection(&get_request, &existing, &parent, true, false);
    }
    let parent = runtime
        .store
        .get_production_weapon_form_art_composite_proposal(
            &request.project_id,
            &request.proposal_id,
        )?
        .ok_or_else(|| invalid("composite parent is unavailable"))?;
    validate_parent_scope(
        &parent,
        &request.project_id,
        &request.proposal_id,
        &request.session_id,
        &request.composite_proposal_record_canonical_sha256,
        &request.composite_proposal_receipt_object_sha256,
        &request.proposal_candidate_id,
        &request.proposal_candidate_state_sha256,
        &request.proposal_artifact_id,
        &request.proposal_artifact_sha256,
        &request.proposal_artifact_readback_object_sha256,
        &request.proposal_artifact_readback_sha256,
    )?;
    let baseline = runtime
        .store
        .get_production_weapon_form_art_baseline_by_id(&request.original_fresh_baseline_id)?
        .ok_or_else(|| invalid("fresh baseline is unavailable"))?;
    if baseline.canonical_sha256 != request.original_fresh_baseline_canonical_sha256 {
        return Err(invalid("fresh baseline canonical hash differs"));
    }
    let evaluated =
        super::production_weapon_form_art_mesh_proposal::evaluate_existing_composite_candidate(
            runtime,
            &parent,
            &request.original_fresh_baseline_id,
            &request.source_form_art_evidence_id,
            &request.source_form_art_evidence_object_sha256,
            &request.source_form_art_evidence_canonical_sha256,
        )?;
    let cross_view_sha256 = evaluated
        .get("cross_view_evidence_bundle_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("evaluated CrossView hash is unavailable"))?;
    let proposal_form_art = evaluated
        .get("proposal_form_art_evidence")
        .ok_or_else(|| invalid("evaluated proposal FormArt is unavailable"))?;
    let proposal_form_art_sha256 = proposal_form_art
        .get("receipt_object_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("proposal FormArt receipt hash is unavailable"))?;
    let timestamp = now_string();
    let request_value =
        serde_json::to_value(&request).map_err(|error| invalid(error.to_string()))?;
    let mut record = ProductionWeaponFormArtCompositeEvidenceRecord {
        schema_version: "ProductionWeaponFormArtCompositeEvidenceRecord@1".to_owned(),
        attachment_id: request.composite_evidence_id.clone(),
        project_id: request.project_id.clone(),
        proposal_id: request.proposal_id.clone(),
        parent_record_canonical_sha256: parent.canonical_sha256.clone(),
        parent_receipt_object_sha256: parent.receipt_object_sha256.clone(),
        cross_view_evidence_bundle_sha256: cross_view_sha256.to_owned(),
        proposal_form_art_evidence_receipt_object_sha256: proposal_form_art_sha256.to_owned(),
        attachment_receipt_object_sha256: "0".repeat(64),
        request_sha256: canonical_json_hash(&request_value),
        input_sha256: request.input_sha256.clone(),
        idempotency_key: request.idempotency_key.clone(),
        status: "SIX_VIEW_EVIDENCE_BOUND_NOT_PROMOTED".to_owned(),
        quality_status: "QUALITY_TARGET_NOT_MET".to_owned(),
        candidate_confirm_allowed: false,
        canonical_sha256: String::new(),
        created_at: timestamp,
    };
    record.canonical_sha256 =
        production_weapon_form_art_composite_evidence_record_canonical_sha256(&record)?;
    let receipt = json!({
        "schema_version":"ProductionWeaponFormArtCompositeEvidenceReceipt@1",
        "attachment_id":record.attachment_id,
        "project_id":record.project_id,
        "proposal_id":record.proposal_id,
        "parent_record_canonical_sha256":record.parent_record_canonical_sha256,
        "parent_receipt_object_sha256":record.parent_receipt_object_sha256,
        "cross_view_evidence_bundle_sha256":record.cross_view_evidence_bundle_sha256,
        "proposal_form_art_evidence_receipt_object_sha256":record.proposal_form_art_evidence_receipt_object_sha256,
        "record_canonical_sha256":record.canonical_sha256,
        "status":record.status,
        "quality_status":"QUALITY_TARGET_NOT_MET",
        "candidate_confirm_allowed":false
    });
    let receipt_bytes =
        canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
    let receipt_object = runtime.put_object(
        &receipt_bytes,
        None,
        "application/json",
        "production-weapon-form-art-composite-evidence-receipt",
    )?;
    record.attachment_receipt_object_sha256 = receipt_object.record.sha256.clone();
    let (stored, replayed) = runtime
        .store
        .record_production_weapon_form_art_composite_evidence_with_replay(
            &record,
            &receipt_object.record,
        )?;
    let mut get_request = GetRequest {
        schema_version: GET_SCHEMA.to_owned(),
        operation: GET_OPERATION.to_owned(),
        composite_evidence_id: request.composite_evidence_id,
        proposal_id: request.proposal_id,
        session_id: request.session_id,
        project_id: request.project_id,
        composite_proposal_record_canonical_sha256: request
            .composite_proposal_record_canonical_sha256,
        composite_proposal_receipt_object_sha256: request.composite_proposal_receipt_object_sha256,
        original_fresh_baseline_id: request.original_fresh_baseline_id,
        original_fresh_baseline_canonical_sha256: request.original_fresh_baseline_canonical_sha256,
        source_form_art_evidence_id: request.source_form_art_evidence_id,
        source_form_art_evidence_object_sha256: request.source_form_art_evidence_object_sha256,
        source_form_art_evidence_canonical_sha256: request
            .source_form_art_evidence_canonical_sha256,
        proposal_candidate_id: request.proposal_candidate_id,
        proposal_candidate_state_sha256: request.proposal_candidate_state_sha256,
        proposal_artifact_id: request.proposal_artifact_id,
        proposal_artifact_sha256: request.proposal_artifact_sha256,
        proposal_artifact_readback_object_sha256: request.proposal_artifact_readback_object_sha256,
        proposal_artifact_readback_sha256: request.proposal_artifact_readback_sha256,
        max_response_bytes: MAX_RESPONSE_BYTES,
        runtime_write_performed: false,
        writer_policy: WRITER_POLICY.to_owned(),
        canonicalization_policy: CANONICALIZATION_POLICY.to_owned(),
        input_sha256: String::new(),
    };
    get_request.input_sha256 = request_input_sha256(&get_request)?;
    let mut result = projection(&get_request, &stored, &parent, replayed, !replayed)?;
    result["evaluation"] = evaluated;
    Ok(result)
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let record = runtime
        .store
        .get_production_weapon_form_art_composite_evidence(
            &request.project_id,
            &request.proposal_id,
        )?
        .ok_or_else(|| invalid("composite evidence sidecar is unavailable"))?;
    if record.attachment_id != request.composite_evidence_id {
        return Err(invalid("composite evidence identity differs"));
    }
    let parent = runtime
        .store
        .get_production_weapon_form_art_composite_proposal(
            &request.project_id,
            &request.proposal_id,
        )?
        .ok_or_else(|| invalid("composite parent is unavailable"))?;
    validate_parent_scope(
        &parent,
        &request.project_id,
        &request.proposal_id,
        &request.session_id,
        &request.composite_proposal_record_canonical_sha256,
        &request.composite_proposal_receipt_object_sha256,
        &request.proposal_candidate_id,
        &request.proposal_candidate_state_sha256,
        &request.proposal_artifact_id,
        &request.proposal_artifact_sha256,
        &request.proposal_artifact_readback_object_sha256,
        &request.proposal_artifact_readback_sha256,
    )?;
    let baseline = runtime
        .store
        .get_production_weapon_form_art_baseline_by_id(&request.original_fresh_baseline_id)?
        .ok_or_else(|| invalid("fresh baseline is unavailable"))?;
    if baseline.canonical_sha256 != request.original_fresh_baseline_canonical_sha256 {
        return Err(invalid("fresh baseline canonical hash differs"));
    }
    projection(&request, &record, &parent, true, false)
}
