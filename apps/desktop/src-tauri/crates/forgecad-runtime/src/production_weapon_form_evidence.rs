//! Runtime-owned six-view weapon form evidence.
//!
//! This producer consumes existing immutable camera/render evidence. It never
//! renders, compiles geometry, advances ProductionStage, confirms, versions or
//! exports. The first contract version deliberately keeps artistic quality at
//! NOT_PROVEN while making the source observations durable and restart-safe.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, CasObject,
    Runtime, RuntimeError,
};
use forgecad_contracts::{
    ProductionWeaponFormEvidenceGetRequest, ProductionWeaponFormEvidenceLineFlow,
    ProductionWeaponFormEvidenceNegativeSpace, ProductionWeaponFormEvidenceObservation,
    ProductionWeaponFormEvidencePartId, ProductionWeaponFormEvidencePrepareRequest,
    ProductionWeaponFormEvidenceRecord, ProductionWeaponFormEvidenceViewInput,
    ProductionWeaponFormEvidenceViewRecord,
    PRODUCTION_WEAPON_FORM_EVIDENCE_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_EVIDENCE_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_EVIDENCE_PARENT_RECEIPT_KIND, PRODUCTION_WEAPON_FORM_EVIDENCE_POLICY,
    PRODUCTION_WEAPON_FORM_EVIDENCE_PREPARE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_EVIDENCE_PREPARE_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_EVIDENCE_QUALITY_STATUS, PRODUCTION_WEAPON_FORM_EVIDENCE_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_KINDS, PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_RECEIPT_KIND,
    PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_SCHEMA_VERSION,
};
use forgecad_store::CasReservation;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const JSON_MIME: &str = "application/json";
const MAX_JSON_BYTES: usize = 1024 * 1024;
const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "form_evidence_id",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "reference_canvas_object_sha256",
    "reference_canvas_canonical_sha256",
    "design_spec_object_sha256",
    "design_spec_canonical_sha256",
    "camera_lock_id",
    "camera_lock_canonical_sha256",
    "camera_rig_object_sha256",
    "camera_rig_canonical_sha256",
    "camera_lock_receipt_object_sha256",
    "camera_lock_source_transition_id",
    "camera_lock_source_transition_sha256",
    "camera_lock_source_head_canonical_sha256",
    "view_kinds",
    "views",
    "evidence_policy",
    "evidence_policy_sha256",
    "input_sha256",
    "idempotency_key",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "form_evidence_id",
    "session_id",
    "project_id",
    "candidate_id",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} must be an object")))?;
    if let Some(field) = object
        .keys()
        .find(|field| !fields.contains(&field.as_str()))
    {
        return Err(invalid(format!(
            "{label} contains unsupported field {field}"
        )));
    }
    if let Some(field) = fields.iter().find(|field| !object.contains_key(**field)) {
        return Err(invalid(format!("{label} is missing {field}")));
    }
    Ok(object)
}

fn canonical_document(value: &Value, schema: &str, label: &str) -> Result<String, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} is not an object")))?;
    if object.get("schema_version").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(format!("{label} schema differs")));
    }
    let mut normalized = value.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    let canonical = canonical_json_hash(&normalized);
    if object.get("canonical_sha256").and_then(Value::as_str) != Some(canonical.as_str()) {
        return Err(invalid(format!("{label} canonical differs")));
    }
    Ok(canonical)
}

fn parse_prepare(
    value: &Value,
) -> Result<(ProductionWeaponFormEvidencePrepareRequest, String), RuntimeError> {
    let object = exact_object(
        value,
        PREPARE_FIELDS,
        "ProductionWeaponFormEvidencePrepareRequest@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_FORM_EVIDENCE_PREPARE_REQUEST_SCHEMA_VERSION)
    {
        return Err(invalid("form evidence prepare schema differs"));
    }
    let request: ProductionWeaponFormEvidencePrepareRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("form evidence prepare is malformed: {error}")))?;
    for identifier in [
        &request.form_evidence_id,
        &request.session_id,
        &request.project_id,
        &request.candidate_id,
        &request.artifact_id,
        &request.camera_lock_id,
        &request.camera_lock_source_transition_id,
        &request.idempotency_key,
    ] {
        if !is_opaque_id(identifier) {
            return Err(invalid("form evidence identifier is invalid"));
        }
    }
    for hash in [
        &request.candidate_state_sha256,
        &request.artifact_sha256,
        &request.reference_canvas_object_sha256,
        &request.reference_canvas_canonical_sha256,
        &request.design_spec_object_sha256,
        &request.design_spec_canonical_sha256,
        &request.camera_lock_canonical_sha256,
        &request.camera_rig_object_sha256,
        &request.camera_rig_canonical_sha256,
        &request.camera_lock_receipt_object_sha256,
        &request.camera_lock_source_transition_sha256,
        &request.camera_lock_source_head_canonical_sha256,
        &request.evidence_policy_sha256,
        &request.input_sha256,
    ] {
        if !is_sha256(hash) {
            return Err(invalid("form evidence hash is invalid"));
        }
    }
    if request.view_kinds
        != PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_KINDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
        || request.views.len() != PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_KINDS.len()
        || request
            .views
            .iter()
            .map(|view| view.view_kind.as_str())
            .collect::<Vec<_>>()
            != PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_KINDS
    {
        return Err(invalid("form evidence requires the six ordered views"));
    }
    let mut view_ids = BTreeSet::new();
    for view in &request.views {
        if !is_opaque_id(&view.view_id)
            || !is_opaque_id(&view.reference_id)
            || !is_opaque_id(&view.render_set_view_id)
            || !view_ids.insert(view.view_id.as_str())
            || [
                &view.reference_sha256,
                &view.camera_hash,
                &view.camera_canonical_sha256,
                &view.render_set_object_sha256,
                &view.render_set_canonical_sha256,
            ]
            .iter()
            .any(|hash| !is_sha256(hash))
            || view.render_set_view_id != view.view_id
        {
            return Err(invalid("form evidence view binding is invalid"));
        }
    }
    if request.evidence_policy != PRODUCTION_WEAPON_FORM_EVIDENCE_POLICY
        || request.evidence_policy_sha256
            != sha256_hex(PRODUCTION_WEAPON_FORM_EVIDENCE_POLICY.as_bytes())
    {
        return Err(invalid("form evidence policy differs"));
    }
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let request_sha256 = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != request_sha256 {
        return Err(invalid("form evidence input hash differs"));
    }
    Ok((request, request_sha256))
}

fn parse_get(value: &Value) -> Result<ProductionWeaponFormEvidenceGetRequest, RuntimeError> {
    let object = exact_object(
        value,
        GET_FIELDS,
        "ProductionWeaponFormEvidenceGetRequest@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_FORM_EVIDENCE_GET_REQUEST_SCHEMA_VERSION)
    {
        return Err(invalid("form evidence get schema differs"));
    }
    let request: ProductionWeaponFormEvidenceGetRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("form evidence get is malformed: {error}")))?;
    if [
        &request.form_evidence_id,
        &request.session_id,
        &request.project_id,
        &request.candidate_id,
    ]
    .iter()
    .any(|value| !is_opaque_id(value))
    {
        return Err(invalid("form evidence get scope is invalid"));
    }
    Ok(request)
}

fn enclosed_void_count(mask: &[bool]) -> u64 {
    if mask.len() != 512 * 512 {
        return 0;
    }
    let mut visited = vec![false; mask.len()];
    let mut enclosed = 0_u64;
    for start in 0..mask.len() {
        if mask[start] || visited[start] {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        visited[start] = true;
        let mut touches_border = false;
        while let Some(index) = queue.pop_front() {
            let x = index % 512;
            let y = index / 512;
            touches_border |= x == 0 || y == 0 || x == 511 || y == 511;
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if nx >= 512 || ny >= 512 {
                    continue;
                }
                let next = ny * 512 + nx;
                if !mask[next] && !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        if !touches_border {
            enclosed += 1;
        }
    }
    enclosed.min(512)
}

fn observation(kind: &str, status: &str) -> ProductionWeaponFormEvidenceObservation {
    ProductionWeaponFormEvidenceObservation {
        evidence_kind: kind.into(),
        observation_status: status.into(),
        quality_status: PRODUCTION_WEAPON_FORM_EVIDENCE_QUALITY_STATUS.into(),
    }
}

fn normalized_view_value(
    view: &ProductionWeaponFormEvidenceViewRecord,
) -> Result<Value, RuntimeError> {
    let mut value = serde_json::to_value(view)
        .map_err(|error| invalid(format!("form evidence view serialize failed: {error}")))?;
    value["receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

fn canonical_record_value(
    record: &ProductionWeaponFormEvidenceRecord,
) -> Result<Value, RuntimeError> {
    let mut value = serde_json::to_value(record)
        .map_err(|error| invalid(format!("form evidence serialize failed: {error}")))?;
    value["receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

fn normalized_record_value(
    record: &ProductionWeaponFormEvidenceRecord,
) -> Result<Value, RuntimeError> {
    let mut value = canonical_record_value(record)?;
    if let Some(views) = value.get_mut("views").and_then(Value::as_array_mut) {
        for view in views {
            view["receipt_object_sha256"] = Value::String(String::new());
            view["canonical_sha256"] = Value::String(String::new());
        }
    }
    Ok(value)
}

fn validate_lock_binding(
    runtime: &Runtime,
    request: &ProductionWeaponFormEvidencePrepareRequest,
) -> Result<Value, RuntimeError> {
    let lock = runtime
        .store
        .get_production_camera_lock(&request.camera_lock_id)?
        .ok_or_else(|| invalid("form evidence CameraLock is unavailable"))?;
    super::agentic_session::validate_production_camera_lock_record(runtime, &lock)?;
    if lock.session_id != request.session_id
        || lock.project_id != request.project_id
        || lock.candidate_id != request.candidate_id
        || lock.candidate_state_sha256 != request.candidate_state_sha256
        || lock.artifact_id != request.artifact_id
        || lock.artifact_sha256 != request.artifact_sha256
        || lock.reference_canvas_object_sha256 != request.reference_canvas_object_sha256
        || lock.reference_canvas_canonical_sha256 != request.reference_canvas_canonical_sha256
        || lock.design_spec_object_sha256 != request.design_spec_object_sha256
        || lock.design_spec_canonical_sha256 != request.design_spec_canonical_sha256
        || lock.canonical_sha256 != request.camera_lock_canonical_sha256
        || lock.camera_rig_object_sha256 != request.camera_rig_object_sha256
        || lock.camera_rig_canonical_sha256 != request.camera_rig_canonical_sha256
        || lock.receipt_object_sha256 != request.camera_lock_receipt_object_sha256
        || lock.source_transition_id != request.camera_lock_source_transition_id
        || lock.source_transition_sha256 != request.camera_lock_source_transition_sha256
        || lock.source_head_canonical_sha256 != request.camera_lock_source_head_canonical_sha256
    {
        return Err(invalid("form evidence CameraLock binding differs"));
    }
    let rig_bytes = runtime.cas_read(&request.camera_rig_object_sha256)?;
    let subject_rig: Value = serde_json::from_slice(&rig_bytes)
        .map_err(|error| invalid(format!("form evidence camera rig is invalid: {error}")))?;
    super::agentic_session::materialize_production_camera_lock_registered_rig(
        runtime,
        &lock.project_id,
        &lock.candidate_id,
        &lock.candidate_state_sha256,
        &lock.artifact_id,
        &lock.artifact_sha256,
        &subject_rig,
        &lock.camera_rig_object_sha256,
    )
}

fn reference_canvas_views(
    runtime: &Runtime,
    request: &ProductionWeaponFormEvidencePrepareRequest,
) -> Result<BTreeMap<String, Value>, RuntimeError> {
    let bytes = runtime.cas_read(&request.reference_canvas_object_sha256)?;
    let canvas: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("form evidence ReferenceCanvas is invalid: {error}")))?;
    if canonical_document(&canvas, "ReferenceCanvas@1", "ReferenceCanvas")?
        != request.reference_canvas_canonical_sha256
    {
        return Err(invalid("form evidence ReferenceCanvas canonical differs"));
    }
    let views = canvas
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("form evidence ReferenceCanvas views are missing"))?;
    let mut indexed = BTreeMap::new();
    for view in views {
        let view_id = view
            .get("view_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("form evidence ReferenceCanvas view id is missing"))?;
        if indexed.insert(view_id.to_owned(), view.clone()).is_some() {
            return Err(invalid(
                "form evidence ReferenceCanvas view id is duplicated",
            ));
        }
    }
    Ok(indexed)
}

fn rig_views(rig: &Value) -> Result<BTreeMap<String, Value>, RuntimeError> {
    let views = rig
        .get("renderer_views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("form evidence registered camera rig views are missing"))?;
    let mut indexed = BTreeMap::new();
    for view in views {
        let kind = view
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("form evidence camera rig kind is missing"))?;
        if indexed.insert(kind.to_owned(), view.clone()).is_some() {
            return Err(invalid("form evidence camera rig kind is duplicated"));
        }
    }
    Ok(indexed)
}

fn derive_views(
    runtime: &Runtime,
    request: &ProductionWeaponFormEvidencePrepareRequest,
    created_at: &str,
) -> Result<Vec<ProductionWeaponFormEvidenceViewRecord>, RuntimeError> {
    let rig = validate_lock_binding(runtime, request)?;
    let rig_by_kind = rig_views(&rig)?;
    let canvas_by_id = reference_canvas_views(runtime, request)?;
    let readback = runtime.artifact_readback(&request.artifact_sha256, &request.candidate_id)?;
    let expected_part_ids = readback
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("form evidence ArtifactReadback part_ids are missing"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("form evidence ArtifactReadback part id is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut records = Vec::new();
    for view in &request.views {
        let canvas_view = canvas_by_id
            .get(&view.view_id)
            .ok_or_else(|| invalid("form evidence view is absent from ReferenceCanvas"))?;
        if canvas_view.get("kind").and_then(Value::as_str) != Some(view.view_kind.as_str())
            || canvas_view.get("reference_id").and_then(Value::as_str)
                != Some(view.reference_id.as_str())
            || canvas_view.get("reference_sha256").and_then(Value::as_str)
                != Some(view.reference_sha256.as_str())
        {
            return Err(invalid(
                "form evidence ReferenceCanvas view binding differs",
            ));
        }
        let reference = runtime
            .reference(&view.reference_id)?
            .ok_or_else(|| invalid("form evidence reference is unavailable"))?;
        if reference.project_id != request.project_id
            || reference.object_sha256 != view.reference_sha256
        {
            return Err(invalid("form evidence reference binding differs"));
        }
        let rig_view = rig_by_kind
            .get(&view.view_kind)
            .ok_or_else(|| invalid("form evidence camera rig view is unavailable"))?;
        if rig_view
            .get("registered_camera_hash")
            .and_then(Value::as_str)
            != Some(view.camera_hash.as_str())
            || rig_view
                .get("registered_camera")
                .and_then(|camera| camera.get("canonical_sha256"))
                .and_then(Value::as_str)
                != Some(view.camera_canonical_sha256.as_str())
        {
            return Err(invalid("form evidence camera binding differs"));
        }
        let render_bytes = runtime.cas_read(&view.render_set_object_sha256)?;
        let render: Value = serde_json::from_slice(&render_bytes)
            .map_err(|error| invalid(format!("form evidence RenderSet is invalid: {error}")))?;
        super::validate_render_set_v2_output(&render)?;
        if canonical_document(&render, "RenderSet@2", "form evidence RenderSet")?
            != view.render_set_canonical_sha256
            || render.get("candidate_id").and_then(Value::as_str)
                != Some(request.candidate_id.as_str())
            || render.get("artifact_sha256").and_then(Value::as_str)
                != Some(request.artifact_sha256.as_str())
            || render.get("reference_id").and_then(Value::as_str)
                != Some(view.reference_id.as_str())
            || render.get("camera_hash").and_then(Value::as_str) != Some(view.camera_hash.as_str())
            || render.get("view_id").and_then(Value::as_str) != Some(view.view_id.as_str())
        {
            return Err(invalid("form evidence RenderSet binding differs"));
        }
        let part_png = runtime.render_pass_bytes(&render, "part-id")?;
        let silhouette_png = runtime.render_pass_bytes(&render, "silhouette")?;
        let observed_part_ids = expected_part_ids
            .iter()
            .filter(|part_id| {
                super::decode_part_mask(&part_png, part_id, &expected_part_ids).is_some()
            })
            .cloned()
            .collect::<Vec<_>>();
        let observed_set = observed_part_ids.iter().collect::<BTreeSet<_>>();
        let missing_part_ids = expected_part_ids
            .iter()
            .filter(|part_id| !observed_set.contains(part_id))
            .cloned()
            .collect::<Vec<_>>();
        let coverage_milli = if expected_part_ids.is_empty() {
            0
        } else {
            (observed_part_ids.len() as u64 * 1000 / expected_part_ids.len() as u64).min(1000)
        };
        let silhouette = super::decode_binary_mask(&silhouette_png)?;
        let void_count = enclosed_void_count(&silhouette);
        let negative_status = if void_count == 0 {
            "unknown"
        } else {
            "inferred"
        };
        let line_status = "unknown";
        let view_observation_status = if negative_status == "unknown" || line_status == "unknown" {
            "unknown"
        } else if negative_status == "inferred" || line_status == "inferred" {
            "inferred"
        } else {
            "observed"
        };
        let mut record = ProductionWeaponFormEvidenceViewRecord {
            schema_version: PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_SCHEMA_VERSION.into(),
            project_id: request.project_id.clone(),
            candidate_id: request.candidate_id.clone(),
            candidate_state_sha256: request.candidate_state_sha256.clone(),
            artifact_id: request.artifact_id.clone(),
            artifact_sha256: request.artifact_sha256.clone(),
            view_kind: view.view_kind.clone(),
            view_id: view.view_id.clone(),
            reference_id: view.reference_id.clone(),
            reference_sha256: view.reference_sha256.clone(),
            camera_hash: view.camera_hash.clone(),
            camera_canonical_sha256: view.camera_canonical_sha256.clone(),
            render_set_object_sha256: view.render_set_object_sha256.clone(),
            render_set_canonical_sha256: view.render_set_canonical_sha256.clone(),
            render_set_view_id: view.render_set_view_id.clone(),
            part_id_evidence: ProductionWeaponFormEvidencePartId {
                observation: observation("part-id", "observed"),
                expected_part_ids: expected_part_ids.clone(),
                observed_part_ids,
                missing_part_ids,
                unexpected_part_ids: Vec::new(),
                coverage_milli,
            },
            negative_space_evidence: ProductionWeaponFormEvidenceNegativeSpace {
                observation: observation("negative-space", negative_status),
                expected_count: 0,
                observed_count: void_count,
                missing_count: 0,
                sealed_count: 0,
                coverage_milli: 0,
            },
            line_flow_evidence: ProductionWeaponFormEvidenceLineFlow {
                observation: observation("line-flow", line_status),
                expected_count: 0,
                observed_count: 0,
                coverage_milli: 0,
                continuity_milli: 0,
                deviation_milli: 0,
            },
            view_observation_status: view_observation_status.into(),
            quality_status: PRODUCTION_WEAPON_FORM_EVIDENCE_QUALITY_STATUS.into(),
            receipt_object_sha256: String::new(),
            canonical_sha256: String::new(),
            created_at: created_at.into(),
        };
        record.canonical_sha256 = canonical_json_hash(&normalized_view_value(&record)?);
        records.push(record);
    }
    Ok(records)
}

fn record_from_request(
    runtime: &Runtime,
    request: &ProductionWeaponFormEvidencePrepareRequest,
    request_sha256: &str,
) -> Result<ProductionWeaponFormEvidenceRecord, RuntimeError> {
    let candidate = runtime
        .candidate(&request.candidate_id)?
        .ok_or_else(|| invalid("form evidence candidate is unavailable"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.candidate_state_sha256
        || candidate.prepared_object_id.as_deref() != Some(request.artifact_id.as_str())
        || candidate.prepared_object_sha256.as_deref() != Some(request.artifact_sha256.as_str())
    {
        return Err(invalid("form evidence candidate/artifact binding differs"));
    }
    let views = derive_views(runtime, request, &candidate.updated_at)?;
    let record = ProductionWeaponFormEvidenceRecord {
        schema_version: PRODUCTION_WEAPON_FORM_EVIDENCE_SCHEMA_VERSION.into(),
        form_evidence_id: request.form_evidence_id.clone(),
        session_id: request.session_id.clone(),
        project_id: request.project_id.clone(),
        candidate_id: request.candidate_id.clone(),
        candidate_state_sha256: request.candidate_state_sha256.clone(),
        artifact_id: request.artifact_id.clone(),
        artifact_sha256: request.artifact_sha256.clone(),
        reference_canvas_object_sha256: request.reference_canvas_object_sha256.clone(),
        reference_canvas_canonical_sha256: request.reference_canvas_canonical_sha256.clone(),
        design_spec_object_sha256: request.design_spec_object_sha256.clone(),
        design_spec_canonical_sha256: request.design_spec_canonical_sha256.clone(),
        camera_lock_id: request.camera_lock_id.clone(),
        camera_lock_canonical_sha256: request.camera_lock_canonical_sha256.clone(),
        camera_rig_object_sha256: request.camera_rig_object_sha256.clone(),
        camera_rig_canonical_sha256: request.camera_rig_canonical_sha256.clone(),
        camera_lock_receipt_object_sha256: request.camera_lock_receipt_object_sha256.clone(),
        camera_lock_source_transition_id: request.camera_lock_source_transition_id.clone(),
        camera_lock_source_transition_sha256: request.camera_lock_source_transition_sha256.clone(),
        camera_lock_source_head_canonical_sha256: request
            .camera_lock_source_head_canonical_sha256
            .clone(),
        view_kinds: request.view_kinds.clone(),
        views,
        evidence_policy: request.evidence_policy.clone(),
        evidence_policy_sha256: request.evidence_policy_sha256.clone(),
        quality_status: PRODUCTION_WEAPON_FORM_EVIDENCE_QUALITY_STATUS.into(),
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256: request_sha256.into(),
        input_sha256: request.input_sha256.clone(),
        receipt_object_sha256: String::new(),
        canonical_sha256: String::new(),
        created_at: candidate.updated_at,
    };
    // Parent canonical is finalized only after child receipt hashes are known.
    Ok(record)
}

fn release(runtime: &Runtime, reservation: &CasReservation, objects: &[CasObject], cleanup: bool) {
    for object in objects {
        let _ = runtime.store.release_cas_reservation_object(
            reservation,
            object,
            cleanup && object.created_new,
        );
    }
}

fn result_value(
    record: &ProductionWeaponFormEvidenceRecord,
    replayed: bool,
    schema: &str,
    runtime_write: bool,
    restart_hash_verified: Option<bool>,
) -> Result<Value, RuntimeError> {
    let mut value = serde_json::json!({
        "schema_version":schema,
        "form_evidence":serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?,
        "replayed":replayed,
        "runtime_write":runtime_write,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false
    });
    if let Some(verified) = restart_hash_verified {
        value["restart_hash_verified"] = Value::Bool(verified);
    }
    Ok(value)
}

impl Runtime {
    pub fn production_weapon_form_evidence_prepare(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let (request, request_sha256) = parse_prepare(&value)?;
        let mut record = record_from_request(self, &request, &request_sha256)?;
        let reservation = self.store.begin_cas_reservation();
        let mut objects = Vec::new();
        for view in &mut record.views {
            let mut receipt_value = serde_json::to_value(&*view).map_err(|error| {
                invalid(format!("form evidence view serialize failed: {error}"))
            })?;
            receipt_value["receipt_object_sha256"] = Value::String(String::new());
            let bytes = canonical_json_bytes(&receipt_value)
                .map_err(|error| invalid(format!("form evidence view bytes failed: {error}")))?;
            if bytes.len() > MAX_JSON_BYTES {
                release(self, &reservation, &objects, true);
                return Err(invalid("form evidence view receipt exceeds 1 MiB"));
            }
            let object = match self.store.put_object_reserved(
                &reservation,
                &bytes,
                None,
                JSON_MIME,
                PRODUCTION_WEAPON_FORM_EVIDENCE_VIEW_RECEIPT_KIND,
                &record.created_at,
            ) {
                Ok(object) => object,
                Err(error) => {
                    release(self, &reservation, &objects, true);
                    return Err(error.into());
                }
            };
            view.receipt_object_sha256 = object.record.sha256.clone();
            objects.push(object);
        }
        record.canonical_sha256 = canonical_json_hash(&canonical_record_value(&record)?);
        let mut receipt_value = serde_json::to_value(&record)
            .map_err(|error| invalid(format!("form evidence serialize failed: {error}")))?;
        receipt_value["receipt_object_sha256"] = Value::String(String::new());
        let bytes = canonical_json_bytes(&receipt_value)
            .map_err(|error| invalid(format!("form evidence bytes failed: {error}")))?;
        if bytes.len() > MAX_JSON_BYTES {
            release(self, &reservation, &objects, true);
            return Err(invalid("form evidence parent receipt exceeds 1 MiB"));
        }
        let parent = match self.store.put_object_reserved(
            &reservation,
            &bytes,
            None,
            JSON_MIME,
            PRODUCTION_WEAPON_FORM_EVIDENCE_PARENT_RECEIPT_KIND,
            &record.created_at,
        ) {
            Ok(object) => object,
            Err(error) => {
                release(self, &reservation, &objects, true);
                return Err(error.into());
            }
        };
        record.receipt_object_sha256 = parent.record.sha256.clone();
        objects.push(parent);
        let child_objects = &objects[..objects.len() - 1];
        let parent_object = &objects[objects.len() - 1];
        match self
            .store
            .record_production_weapon_form_evidence_with_replay(
                &record,
                &child_objects
                    .iter()
                    .map(|object| object.record.clone())
                    .collect::<Vec<_>>(),
                &parent_object.record,
            ) {
            Ok((stored, replayed)) => {
                release(self, &reservation, &objects, false);
                result_value(
                    &stored,
                    replayed,
                    PRODUCTION_WEAPON_FORM_EVIDENCE_PREPARE_RESULT_SCHEMA_VERSION,
                    true,
                    None,
                )
            }
            Err(error) => {
                release(self, &reservation, &objects, true);
                Err(error.into())
            }
        }
    }

    pub fn production_weapon_form_evidence_get(&self, value: Value) -> Result<Value, RuntimeError> {
        let request = parse_get(&value)?;
        let record = self
            .store
            .get_production_weapon_form_evidence(&request.form_evidence_id)?
            .ok_or_else(|| invalid("form evidence is unavailable"))?;
        if record.session_id != request.session_id
            || record.project_id != request.project_id
            || record.candidate_id != request.candidate_id
        {
            return Err(invalid("form evidence get scope differs"));
        }
        for view in &record.views {
            if canonical_json_hash(&normalized_view_value(view)?) != view.canonical_sha256 {
                return Err(invalid("form evidence view canonical differs"));
            }
            let bytes = self.cas_read(&view.receipt_object_sha256)?;
            let mut expected = serde_json::to_value(view).map_err(|error| {
                invalid(format!("form evidence view serialize failed: {error}"))
            })?;
            expected["receipt_object_sha256"] = Value::String(String::new());
            if bytes
                != canonical_json_bytes(&expected)
                    .map_err(|error| invalid(format!("form evidence view bytes failed: {error}")))?
            {
                return Err(invalid("form evidence view receipt bytes differ"));
            }
        }
        if canonical_json_hash(&canonical_record_value(&record)?) != record.canonical_sha256 {
            return Err(invalid("form evidence parent canonical differs"));
        }
        let bytes = self.cas_read(&record.receipt_object_sha256)?;
        let mut expected = serde_json::to_value(&record)
            .map_err(|error| invalid(format!("form evidence serialize failed: {error}")))?;
        expected["receipt_object_sha256"] = Value::String(String::new());
        if bytes
            != canonical_json_bytes(&expected)
                .map_err(|error| invalid(format!("form evidence bytes failed: {error}")))?
        {
            return Err(invalid("form evidence parent receipt bytes differ"));
        }
        let prepare = ProductionWeaponFormEvidencePrepareRequest {
            schema_version: PRODUCTION_WEAPON_FORM_EVIDENCE_PREPARE_REQUEST_SCHEMA_VERSION.into(),
            form_evidence_id: record.form_evidence_id.clone(),
            session_id: record.session_id.clone(),
            project_id: record.project_id.clone(),
            candidate_id: record.candidate_id.clone(),
            candidate_state_sha256: record.candidate_state_sha256.clone(),
            artifact_id: record.artifact_id.clone(),
            artifact_sha256: record.artifact_sha256.clone(),
            reference_canvas_object_sha256: record.reference_canvas_object_sha256.clone(),
            reference_canvas_canonical_sha256: record.reference_canvas_canonical_sha256.clone(),
            design_spec_object_sha256: record.design_spec_object_sha256.clone(),
            design_spec_canonical_sha256: record.design_spec_canonical_sha256.clone(),
            camera_lock_id: record.camera_lock_id.clone(),
            camera_lock_canonical_sha256: record.camera_lock_canonical_sha256.clone(),
            camera_rig_object_sha256: record.camera_rig_object_sha256.clone(),
            camera_rig_canonical_sha256: record.camera_rig_canonical_sha256.clone(),
            camera_lock_receipt_object_sha256: record.camera_lock_receipt_object_sha256.clone(),
            camera_lock_source_transition_id: record.camera_lock_source_transition_id.clone(),
            camera_lock_source_transition_sha256: record
                .camera_lock_source_transition_sha256
                .clone(),
            camera_lock_source_head_canonical_sha256: record
                .camera_lock_source_head_canonical_sha256
                .clone(),
            view_kinds: record.view_kinds.clone(),
            views: record
                .views
                .iter()
                .map(|view| ProductionWeaponFormEvidenceViewInput {
                    view_kind: view.view_kind.clone(),
                    view_id: view.view_id.clone(),
                    reference_id: view.reference_id.clone(),
                    reference_sha256: view.reference_sha256.clone(),
                    camera_hash: view.camera_hash.clone(),
                    camera_canonical_sha256: view.camera_canonical_sha256.clone(),
                    render_set_object_sha256: view.render_set_object_sha256.clone(),
                    render_set_canonical_sha256: view.render_set_canonical_sha256.clone(),
                    render_set_view_id: view.render_set_view_id.clone(),
                })
                .collect(),
            evidence_policy: record.evidence_policy.clone(),
            evidence_policy_sha256: record.evidence_policy_sha256.clone(),
            input_sha256: record.input_sha256.clone(),
            idempotency_key: record.form_evidence_id.clone(),
        };
        let prepare_value = serde_json::to_value(&prepare)
            .map_err(|error| invalid(format!("form evidence prepare serialize failed: {error}")))?;
        let (parsed, request_sha256) = parse_prepare(&prepare_value)?;
        if request_sha256 != record.request_sha256 || parsed.input_sha256 != record.input_sha256 {
            return Err(invalid("form evidence request hash differs after restart"));
        }
        let rebuilt = record_from_request(self, &parsed, &request_sha256)?;
        if normalized_record_value(&rebuilt)? != normalized_record_value(&record)? {
            return Err(invalid("form evidence restart projection differs"));
        }
        result_value(
            &record,
            true,
            PRODUCTION_WEAPON_FORM_EVIDENCE_GET_RESULT_SCHEMA_VERSION,
            false,
            Some(true),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enclosed_void_count_distinguishes_background_from_hole() {
        let mut mask = vec![false; 512 * 512];
        for y in 100..200 {
            for x in 100..200 {
                mask[y * 512 + x] = true;
            }
        }
        for y in 130..170 {
            for x in 130..170 {
                mask[y * 512 + x] = false;
            }
        }
        assert_eq!(enclosed_void_count(&mask), 1);
    }

    #[test]
    fn closed_get_rejects_unknown_fields() {
        let request = serde_json::json!({
            "schema_version":PRODUCTION_WEAPON_FORM_EVIDENCE_GET_REQUEST_SCHEMA_VERSION,
            "form_evidence_id":"form-evidence-1",
            "session_id":"session-1",
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "raw_png_bytes":"forbidden"
        });
        assert!(parse_get(&request).is_err());
    }
}
