//! Candidate/AOV-bound artistic form evidence for FPS weapon production.
//!
//! This is deliberately additive to FormEvidence@1.  FormEvidence@1 records
//! the durable six-view source chain; this module independently observes the
//! candidate against those views.  It never renders, changes a candidate, or
//! advances a production stage.  In particular, reference annotations are
//! never copied into the candidate observation: if the target is not a
//! user-confirmed, source=user_refined target, the derived rows remain
//! unknown.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, CasObject,
    Runtime, RuntimeError,
};
use forgecad_contracts::{
    ProductionWeaponFormArtEvidenceGetRequest, ProductionWeaponFormArtEvidenceLineFlowRow,
    ProductionWeaponFormArtEvidenceNegativeSpaceRow,
    ProductionWeaponFormArtEvidencePartIdAggregate, ProductionWeaponFormArtEvidencePrepareRequest,
    ProductionWeaponFormArtEvidenceRecord, ProductionWeaponFormArtEvidenceViewRecord,
    ProductionWeaponOwnerReviewedVoidCalibrationProjection,
    ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest,
    ProductionWeaponOwnerReviewedVoidCalibrationProjectionView,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PARENT_RECEIPT_KIND,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_POLICY,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PREPARE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PREPARE_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_QUALITY_STATUS,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_RECEIPT_KIND,
    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_SCHEMA_VERSION,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_CANONICALIZATION_POLICY,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_DEPTH_POLICY,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_OPERATION,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_OWNER_PART_ID,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_POLICY,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_QUALITY_STATUS,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_REVIEWED_VOID_POLICY,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_SCHEMA_VERSION,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_THRESHOLD_POLICY,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_TRANSFORM_POLICY,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_VIEW_KINDS,
    PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_VIEW_SCHEMA_VERSION,
};
use forgecad_store::CasReservation;
use image::imageops;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const JSON_MIME: &str = "application/json";
const MAX_JSON_BYTES: usize = 1024 * 1024;
const MAX_LINE_FLOWS_PER_VIEW: usize = 128;
const MAX_LINE_POINTS_PER_FLOW: usize = 256;
const MAX_SAMPLES_PER_FLOW: usize = 256;
const MAX_TOTAL_LINE_SAMPLES_PER_VIEW: usize = 8192;
const MAX_NEGATIVE_REGIONS_PER_VIEW: usize = 32;
const MAX_NEGATIVE_POINTS_PER_VIEW: usize = 4096;
const VIEW_KINDS: [&str; 6] = [
    "front",
    "back",
    "left",
    "right",
    "top",
    "rear-three-quarter",
];
const UNKNOWN_STRUCTURE_HASH_SEED: &[u8] =
    b"forgecad-production-weapon-form-art-visual-structure-unknown@1";
const RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_GET_REQUEST_SCHEMA_VERSION: &str =
    "ProductionWeaponRasterSourceAttributionDiagnosticGetRequest@1";
const RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_GET_RESULT_SCHEMA_VERSION: &str =
    "ProductionWeaponRasterSourceAttributionDiagnosticGetResult@1";
const RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_POLICY: &str =
    "production-weapon-raster-source-attribution-single-candidate-diagnostic@1";

#[derive(Debug, Clone, Deserialize)]
struct RasterSourceAttributionDiagnosticGetRequest {
    schema_version: String,
    diagnostic_id: String,
    session_id: String,
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    artifact_id: String,
    artifact_sha256: String,
    reference_id: String,
    reference_sha256: String,
    form_art_evidence_object_sha256: String,
    form_art_evidence_canonical_sha256: String,
    view_kind: String,
    view_id: String,
    camera_hash: String,
    camera_canonical_sha256: String,
    input_sha256: String,
}

const RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_GET_FIELDS: &[&str] = &[
    "schema_version",
    "diagnostic_id",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "reference_id",
    "reference_sha256",
    "form_art_evidence_object_sha256",
    "form_art_evidence_canonical_sha256",
    "view_kind",
    "view_id",
    "camera_hash",
    "camera_canonical_sha256",
    "input_sha256",
];

fn parse_raster_source_attribution_diagnostic_get(
    value: &Value,
) -> Result<(RasterSourceAttributionDiagnosticGetRequest, String), RuntimeError> {
    let object = exact_object(
        value,
        RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_GET_FIELDS,
        "ProductionWeaponRasterSourceAttributionDiagnosticGetRequest@1",
    )?;
    let request: RasterSourceAttributionDiagnosticGetRequest =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!(
                "raster source attribution diagnostic request is malformed: {error}"
            ))
        })?;
    if request.schema_version != RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_GET_REQUEST_SCHEMA_VERSION {
        return Err(invalid(
            "raster source attribution diagnostic request schema differs",
        ));
    }
    for id in [
        &request.diagnostic_id,
        &request.session_id,
        &request.project_id,
        &request.candidate_id,
        &request.artifact_id,
        &request.reference_id,
        &request.view_id,
    ] {
        if !is_opaque_id(id) {
            return Err(invalid(
                "raster source attribution diagnostic identifier is invalid",
            ));
        }
    }
    for hash in [
        &request.candidate_state_sha256,
        &request.artifact_sha256,
        &request.reference_sha256,
        &request.form_art_evidence_object_sha256,
        &request.form_art_evidence_canonical_sha256,
        &request.camera_hash,
        &request.camera_canonical_sha256,
        &request.input_sha256,
    ] {
        if !is_sha256(hash) {
            return Err(invalid(
                "raster source attribution diagnostic hash is invalid",
            ));
        }
    }
    if !matches!(
        request.view_kind.as_str(),
        "left" | "right" | "rear-three-quarter"
    ) {
        return Err(invalid(
            "raster source attribution diagnostic view is not a reviewed open-stock view",
        ));
    }
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    let request_sha256 = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != request_sha256 {
        return Err(invalid(
            "raster source attribution diagnostic input hash differs",
        ));
    }
    Ok((request, request_sha256))
}

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "art_evidence_id",
    "session_id",
    "project_id",
    "candidate_id",
    "form_evidence_object_sha256",
    "form_evidence_canonical_sha256",
    "art_evidence_policy",
    "art_evidence_policy_sha256",
    "input_sha256",
    "idempotency_key",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "art_evidence_id",
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
    if normalized.get("canonical_sha256").is_some() {
        normalized["canonical_sha256"] = Value::String(String::new());
    }
    let canonical = canonical_json_hash(&normalized);
    if object.get("canonical_sha256").and_then(Value::as_str) != Some(canonical.as_str()) {
        return Err(invalid(format!("{label} canonical differs")));
    }
    Ok(canonical)
}

fn parse_prepare(
    value: &Value,
) -> Result<(ProductionWeaponFormArtEvidencePrepareRequest, String), RuntimeError> {
    let object = exact_object(
        value,
        PREPARE_FIELDS,
        "ProductionWeaponFormArtEvidencePrepareRequest@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PREPARE_REQUEST_SCHEMA_VERSION)
    {
        return Err(invalid("form art evidence prepare schema differs"));
    }
    let request: ProductionWeaponFormArtEvidencePrepareRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("form art evidence prepare is malformed: {error}")))?;
    for id in [
        &request.art_evidence_id,
        &request.session_id,
        &request.project_id,
        &request.candidate_id,
        &request.idempotency_key,
    ] {
        if !is_opaque_id(id) {
            return Err(invalid("form art evidence identifier is invalid"));
        }
    }
    for hash in [
        &request.form_evidence_object_sha256,
        &request.form_evidence_canonical_sha256,
        &request.art_evidence_policy_sha256,
        &request.input_sha256,
    ] {
        if !is_sha256(hash) {
            return Err(invalid("form art evidence hash is invalid"));
        }
    }
    if request.art_evidence_policy != PRODUCTION_WEAPON_FORM_ART_EVIDENCE_POLICY
        || request.art_evidence_policy_sha256
            != sha256_hex(PRODUCTION_WEAPON_FORM_ART_EVIDENCE_POLICY.as_bytes())
    {
        return Err(invalid("form art evidence policy differs"));
    }
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    preimage.remove("idempotency_key");
    let request_sha256 = canonical_json_hash(&Value::Object(preimage));
    if request.input_sha256 != request_sha256 {
        return Err(invalid("form art evidence input hash differs"));
    }
    Ok((request, request_sha256))
}

fn parse_get(value: &Value) -> Result<ProductionWeaponFormArtEvidenceGetRequest, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("ProductionWeaponFormArtEvidenceGetRequest@1 must be an object"))?;
    if let Some(field) = object.keys().find(|field| {
        !GET_FIELDS.contains(&field.as_str()) && *field != "raster_source_attribution_diagnostic"
    }) {
        return Err(invalid(format!(
            "ProductionWeaponFormArtEvidenceGetRequest@1 contains unsupported field {field}"
        )));
    }
    if let Some(field) = GET_FIELDS
        .iter()
        .find(|field| !object.contains_key(**field))
    {
        return Err(invalid(format!(
            "ProductionWeaponFormArtEvidenceGetRequest@1 is missing {field}"
        )));
    }
    if object.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_FORM_ART_EVIDENCE_GET_REQUEST_SCHEMA_VERSION)
    {
        return Err(invalid("form art evidence get schema differs"));
    }
    let request: ProductionWeaponFormArtEvidenceGetRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("form art evidence get is malformed: {error}")))?;
    for id in [
        &request.art_evidence_id,
        &request.session_id,
        &request.project_id,
        &request.candidate_id,
    ] {
        if !is_opaque_id(id) {
            return Err(invalid("form art evidence get scope is invalid"));
        }
    }
    Ok(request)
}

fn normalized_view(
    view: &ProductionWeaponFormArtEvidenceViewRecord,
) -> Result<Value, RuntimeError> {
    let mut value = serde_json::to_value(view).map_err(|error| invalid(error.to_string()))?;
    value["receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

fn normalized_record(
    record: &ProductionWeaponFormArtEvidenceRecord,
) -> Result<Value, RuntimeError> {
    let mut value = serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    value["receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

fn read_json(runtime: &Runtime, sha256: &str, label: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(sha256, MAX_JSON_BYTES as u64)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{label} is invalid JSON: {error}")))
}

/// Read immutable FormArt evidence for a projection that must survive Runtime
/// upgrades.  Historical RenderSet objects remain fully hash/profile/binding
/// validated, but their recorded Worker cohort is not relabelled as the
/// current cohort.  Callers that intend to render or compare new evidence must
/// separately require current-cohort compatibility before performing writes.
pub(crate) fn read_persisted_form_art_for_projection(
    runtime: &Runtime,
    art_evidence_id: &str,
    project_id: &str,
    candidate_id: &str,
    object_sha256: &str,
    canonical_sha256: &str,
) -> Result<
    (
        ProductionWeaponFormArtEvidenceRecord,
        BTreeSet<Option<String>>,
    ),
    RuntimeError,
> {
    let record = runtime
        .store
        .get_production_weapon_form_art_evidence(art_evidence_id)
        .map_err(RuntimeError::from)?
        .ok_or_else(|| invalid("FormArt evidence is unavailable"))?;
    if record.art_evidence_id != art_evidence_id
        || record.project_id != project_id
        || record.candidate_id != candidate_id
        || record.receipt_object_sha256 != object_sha256
        || record.canonical_sha256 != canonical_sha256
    {
        return Err(invalid("persisted FormArt evidence scope differs"));
    }

    let mut worker_cohorts = BTreeSet::new();
    for view in &record.views {
        if canonical_json_hash(&normalized_view(view)?) != view.canonical_sha256 {
            return Err(invalid("persisted FormArt evidence view canonical differs"));
        }
        let bytes = runtime.cas_read(&view.receipt_object_sha256)?;
        let mut expected =
            serde_json::to_value(view).map_err(|error| invalid(error.to_string()))?;
        expected["receipt_object_sha256"] = Value::String(String::new());
        if bytes != canonical_json_bytes(&expected).map_err(|error| invalid(error.to_string()))? {
            return Err(invalid(
                "persisted FormArt evidence view receipt bytes differ",
            ));
        }

        let source_view_receipt = read_json(
            runtime,
            &view.form_evidence_view_receipt_object_sha256,
            "persisted FormEvidence view receipt",
        )?;
        if source_view_receipt
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(view.form_evidence_view_receipt_canonical_sha256.as_str())
            || source_view_receipt
                .get("candidate_id")
                .and_then(Value::as_str)
                != Some(record.candidate_id.as_str())
            || source_view_receipt.get("view_id").and_then(Value::as_str)
                != Some(view.view_id.as_str())
        {
            return Err(invalid("persisted FormEvidence view binding differs"));
        }
        let render_set_object_sha256 = source_view_receipt
            .get("render_set_object_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("persisted FormEvidence RenderSet object hash is invalid"))?;
        let render_set_canonical_sha256 = source_view_receipt
            .get("render_set_canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("persisted FormEvidence RenderSet canonical is invalid"))?;
        let render_set = read_json(
            runtime,
            render_set_object_sha256,
            "persisted FormArt RenderSet",
        )?;
        super::validate_persisted_render_set_v2_output(&render_set)?;
        if canonical_document(&render_set, "RenderSet@2", "persisted FormArt RenderSet")?
            != render_set_canonical_sha256
            || render_set.get("candidate_id").and_then(Value::as_str)
                != Some(record.candidate_id.as_str())
            || render_set.get("artifact_sha256").and_then(Value::as_str)
                != Some(record.artifact_sha256.as_str())
            || render_set.get("reference_id").and_then(Value::as_str)
                != Some(view.reference_id.as_str())
            || render_set.get("camera_hash").and_then(Value::as_str)
                != Some(view.camera_hash.as_str())
            || render_set.get("view_id").and_then(Value::as_str) != Some(view.view_id.as_str())
        {
            return Err(invalid("persisted FormArt RenderSet binding differs"));
        }
        worker_cohorts.insert(
            render_set
                .get("render_worker_build_cohort_sha256")
                .and_then(Value::as_str)
                .map(str::to_owned),
        );
    }

    if canonical_json_hash(&normalized_record(&record)?) != record.canonical_sha256 {
        return Err(invalid(
            "persisted FormArt evidence parent canonical differs",
        ));
    }
    let bytes = runtime.cas_read(&record.receipt_object_sha256)?;
    let mut expected = serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
    expected["receipt_object_sha256"] = Value::String(String::new());
    if bytes != canonical_json_bytes(&expected).map_err(|error| invalid(error.to_string()))? {
        return Err(invalid(
            "persisted FormArt evidence parent receipt bytes differ",
        ));
    }
    Ok((record, worker_cohorts))
}

fn source_form_evidence(
    runtime: &Runtime,
    request: &ProductionWeaponFormArtEvidencePrepareRequest,
) -> Result<forgecad_contracts::ProductionWeaponFormEvidenceRecord, RuntimeError> {
    // The form evidence id is intentionally carried in the parent receipt, not
    // in this request.  Its id is discovered by scanning the immutable CAS
    // payload binding from the requested object.
    let payload = read_json(
        runtime,
        &request.form_evidence_object_sha256,
        "FormEvidence receipt",
    )?;
    let canonical = canonical_document(&payload, "ProductionWeaponFormEvidence@1", "FormEvidence")?;
    if canonical != request.form_evidence_canonical_sha256 {
        return Err(invalid("FormEvidence canonical differs"));
    }
    let mut source: forgecad_contracts::ProductionWeaponFormEvidenceRecord =
        serde_json::from_value(payload.clone())
            .map_err(|error| invalid(format!("FormEvidence receipt is malformed: {error}")))?;
    // The receipt projection deliberately clears its self-reference.  The
    // Store row is authoritative for the actual receipt hash and full record.
    let source_id = source.form_evidence_id.clone();
    let stored = runtime
        .store
        .get_production_weapon_form_evidence(&source_id)
        .map_err(RuntimeError::from)?
        .ok_or_else(|| invalid("FormEvidence durable link is unavailable"))?;
    source = stored;
    if source.form_evidence_id != source_id
        || source.project_id != request.project_id
        || source.session_id != request.session_id
        || source.candidate_id != request.candidate_id
        || source.canonical_sha256 != request.form_evidence_canonical_sha256
        || source.receipt_object_sha256 != request.form_evidence_object_sha256
    {
        return Err(invalid("FormEvidence binding differs"));
    }
    Ok(source)
}

fn validate_canvas_and_spec(
    runtime: &Runtime,
    form: &forgecad_contracts::ProductionWeaponFormEvidenceRecord,
) -> Result<Value, RuntimeError> {
    let canvas = read_json(
        runtime,
        &form.reference_canvas_object_sha256,
        "ReferenceCanvas",
    )?;
    if canonical_document(&canvas, "ReferenceCanvas@1", "ReferenceCanvas")?
        != form.reference_canvas_canonical_sha256
        || canvas.get("project_id").and_then(Value::as_str) != Some(form.project_id.as_str())
    {
        return Err(invalid("ReferenceCanvas binding differs"));
    }
    let spec = read_json(runtime, &form.design_spec_object_sha256, "DesignSpec")?;
    if canonical_document(&spec, "DesignSpec@1", "DesignSpec")? != form.design_spec_canonical_sha256
        || spec.get("project_id").and_then(Value::as_str) != Some(form.project_id.as_str())
        || spec.get("reference_canvas_sha256").and_then(Value::as_str)
            != Some(form.reference_canvas_object_sha256.as_str())
    {
        return Err(invalid("DesignSpec binding differs"));
    }
    Ok(canvas)
}

fn canvas_views(canvas: &Value) -> Result<BTreeMap<String, Value>, RuntimeError> {
    let views = canvas
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ReferenceCanvas views are missing"))?;
    let mut result = BTreeMap::new();
    for view in views {
        let id = view
            .get("view_id")
            .and_then(Value::as_str)
            .filter(|id| is_opaque_id(id))
            .ok_or_else(|| invalid("ReferenceCanvas view_id is invalid"))?;
        if result.insert(id.to_owned(), view.clone()).is_some() {
            return Err(invalid("ReferenceCanvas view_id is duplicated"));
        }
    }
    Ok(result)
}

fn rig_views(rig: &Value) -> Result<BTreeMap<String, Value>, RuntimeError> {
    let views = rig
        .get("renderer_views")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("registered CameraRig views are missing"))?;
    let mut result = BTreeMap::new();
    for view in views {
        let kind = view
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("CameraRig view kind is missing"))?;
        if result.insert(kind.to_owned(), view.clone()).is_some() {
            return Err(invalid("CameraRig view kind is duplicated"));
        }
    }
    Ok(result)
}

fn metrics(mask_a: &[bool], mask_b: &[bool]) -> (u64, u64, u64, u64, bool) {
    if mask_a.len() != 512 * 512 || mask_b.len() != 512 * 512 {
        return (0, 0, 0, 0, false);
    }
    let mut intersection = 0_u64;
    let mut a_count = 0_u64;
    let mut b_count = 0_u64;
    let mut ac = (0_f64, 0_f64);
    let mut bc = (0_f64, 0_f64);
    for (index, (&a, &b)) in mask_a.iter().zip(mask_b.iter()).enumerate() {
        if a {
            a_count += 1;
            ac.0 += (index % 512) as f64;
            ac.1 += (index / 512) as f64;
        }
        if b {
            b_count += 1;
            bc.0 += (index % 512) as f64;
            bc.1 += (index / 512) as f64;
        }
        if a && b {
            intersection += 1;
        }
    }
    let union = a_count + b_count - intersection;
    let iou = if union == 0 {
        0
    } else {
        intersection * 1000 / union
    };
    let boundary_a = boundary_mask(mask_a);
    let boundary_b = boundary_mask(mask_b);
    let precision = if boundary_b.iter().filter(|v| **v).count() == 0 {
        0.0
    } else {
        boundary_overlap(&boundary_b, &boundary_a) as f64
            / boundary_b.iter().filter(|v| **v).count() as f64
    };
    let recall = if boundary_a.iter().filter(|v| **v).count() == 0 {
        0.0
    } else {
        boundary_overlap(&boundary_a, &boundary_b) as f64
            / boundary_a.iter().filter(|v| **v).count() as f64
    };
    let f1 = if precision + recall <= f64::EPSILON {
        0
    } else {
        ((2.0 * precision * recall / (precision + recall)) * 1000.0).round() as u64
    };
    let ratio = if a_count == 0 {
        0
    } else {
        ((b_count as f64 / a_count as f64) * 1000.0)
            .round()
            .clamp(0.0, 10000.0) as u64
    };
    let centroid = if a_count == 0 || b_count == 0 {
        100000
    } else {
        let dx = ac.0 / a_count as f64 - bc.0 / b_count as f64;
        let dy = ac.1 / a_count as f64 - bc.1 / b_count as f64;
        ((dx.mul_add(dx, dy * dy).sqrt() / 512.0) * 100000.0).round() as u64
    };
    (
        iou.min(1000),
        f1.min(1000),
        ratio,
        centroid.min(100000),
        a_count > 0 && b_count > 0,
    )
}

fn boundary_mask(mask: &[bool]) -> Vec<bool> {
    let mut edge = vec![false; mask.len()];
    for y in 0..512usize {
        for x in 0..512usize {
            let i = y * 512 + x;
            if !mask[i] {
                continue;
            }
            edge[i] = [
                (x > 0).then(|| i - 1),
                (x < 511).then(|| i + 1),
                (y > 0).then(|| i - 512),
                (y < 511).then(|| i + 512),
            ]
            .into_iter()
            .flatten()
            .any(|n| !mask[n]);
        }
    }
    edge
}

fn boundary_overlap(source: &[bool], target: &[bool]) -> u64 {
    let mut count = 0_u64;
    for y in 0..512usize {
        for x in 0..512usize {
            let i = y * 512 + x;
            if !source[i] {
                continue;
            }
            let mut hit = false;
            for dy in -2_i32..=2 {
                for dx in -2_i32..=2 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0
                        && ny >= 0
                        && nx < 512
                        && ny < 512
                        && target[ny as usize * 512 + nx as usize]
                    {
                        hit = true;
                    }
                }
            }
            if hit {
                count += 1;
            }
        }
    }
    count
}

/// A read-only, candidate-local owner-mask audit used by the private 04U
/// profile diagnostics.  The masks remain semantic Part-ID masks; this helper
/// deliberately does not infer source triangles or mutate Runtime truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct OwnerMaskHammingDiagnostic {
    pub expected_void_pixel_count: u64,
    pub expected_boundary_pixel_count: u64,
    pub baseline_owner_pixel_count: u64,
    pub trial_owner_pixel_count: u64,
    pub owner_mask_changed_pixel_count: u64,
    pub owner_mask_changed_inside_expected_void_pixel_count: u64,
    pub owner_mask_changed_inside_region_outside_expected_void_pixel_count: u64,
    pub owner_mask_changed_outside_reviewed_region_pixel_count: u64,
    pub baseline_owner_expected_void_overlap_pixel_count: u64,
    pub trial_owner_expected_void_overlap_pixel_count: u64,
    pub owner_expected_void_overlap_delta_pixel_count: i64,
    pub changed_expected_boundary_pixel_count: u64,
    pub changed_expected_boundary_band_r1_pixel_count: u64,
    pub changed_expected_boundary_band_r2_pixel_count: u64,
    pub changed_expected_boundary_band_r4_pixel_count: u64,
    pub changed_bbox_px: Option<[u64; 4]>,
    pub changed_centroid_milli_px: Option<[i64; 2]>,
    pub classification: String,
}

/// The Runtime path now binds the deterministic triangle/source map to the
/// exact durable FormArt record, candidate artifact, reference target and
/// registered camera. A real D1 execution receipt is still independently
/// required before this capability can be promoted beyond source/fixture truth.
pub(crate) const OWNER_MASK_TRIANGLE_ATTRIBUTION_STATUS: &str =
    "AVAILABLE_RUNTIME_FORM_ART_HASH_BOUND_REAL_D1_NOT_RUN";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct RasterSourceAttributionRow {
    pub semantic_part_id: String,
    pub source_node_id: String,
    pub lineage_source_node_ids: Vec<String>,
    pub material_zone_ids: Vec<String>,
    pub mesh_indices: Vec<u32>,
    pub primitive_indices: Vec<u32>,
    pub triangle_count: u64,
    pub visible_pixel_count: u64,
    pub reviewed_region_pixel_count: u64,
    pub expected_void_pixel_count: u64,
    pub owner_changed_pixel_count: u64,
}

/// Read-only projection of the isolated Render Worker's exact
/// pixel -> triangle -> source table. The diagnostic is intentionally
/// transient: callers must bind candidate/reference/camera/evidence hashes in
/// their enclosing receipt, and this function cannot persist or promote it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct RasterSourceAttributionDiagnostic {
    pub width: u32,
    pub height: u32,
    pub visible_pixel_count: u64,
    pub background_pixel_count: u64,
    pub reviewed_region_attributed_pixel_count: u64,
    pub expected_void_attributed_pixel_count: u64,
    pub owner_changed_attributed_pixel_count: u64,
    pub triangle_ids_sha256: String,
    pub source_table_sha256: String,
    pub sources: Vec<RasterSourceAttributionRow>,
    /// Runtime-selected source with the largest actionable raster overlap.
    /// Identifier ordering is never used to break an impact tie: ambiguous
    /// ownership fails closed before a RepairIntent can be proposed.
    pub highest_impact_source: RasterSourceAttributionRow,
    pub highest_impact_basis: String,
    pub highest_impact_pixel_count: u64,
    /// Semantic Parts that are allowed to own the reviewed art region. This
    /// closes the 180-degree silhouette ambiguity: a muzzle raster cannot be
    /// promoted into a stock RepairIntent merely because it overlaps more
    /// pixels in a mirrored camera projection.
    pub expected_semantic_part_ids: Vec<String>,
    pub highest_impact_semantic_match: bool,
    pub repair_target_status: String,
    pub render_worker_build_cohort_sha256: Option<String>,
    pub status: String,
    pub diagnostic_only: bool,
    pub promotable: bool,
    pub runtime_write: bool,
    pub production_stage_advanced: bool,
}

fn select_unique_highest_impact_source(
    sources: &[RasterSourceAttributionRow],
) -> Result<(RasterSourceAttributionRow, &'static str, u64), RuntimeError> {
    let owner_changed_max = sources
        .iter()
        .map(|source| source.owner_changed_pixel_count)
        .max()
        .unwrap_or(0);
    let (basis, maximum) = if owner_changed_max > 0 {
        ("owner-changed-pixels", owner_changed_max)
    } else {
        (
            "expected-void-pixels",
            sources
                .iter()
                .map(|source| source.expected_void_pixel_count)
                .max()
                .unwrap_or(0),
        )
    };
    if maximum == 0 {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_NO_ACTIONABLE_SOURCE_PIXELS",
        ));
    }
    let selected = sources
        .iter()
        .filter(|source| match basis {
            "owner-changed-pixels" => source.owner_changed_pixel_count == maximum,
            _ => source.expected_void_pixel_count == maximum,
        })
        .collect::<Vec<_>>();
    if selected.len() != 1 {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_AMBIGUOUS_HIGHEST_IMPACT_SOURCE",
        ));
    }
    Ok((selected[0].clone(), basis, maximum))
}

#[derive(Default)]
struct RasterSourceAttributionAccumulator {
    lineage_source_node_ids: BTreeSet<String>,
    material_zone_ids: BTreeSet<String>,
    mesh_indices: BTreeSet<u32>,
    primitive_indices: BTreeSet<u32>,
    triangle_ids: BTreeSet<u32>,
    visible_pixel_count: u64,
    reviewed_region_pixel_count: u64,
    expected_void_pixel_count: u64,
    owner_changed_pixel_count: u64,
}

pub(crate) fn raster_source_attribution_diagnostic(
    attribution: &super::render_worker::RenderWorkerRasterAttribution,
    reviewed_region_mask: &[bool],
    expected_void_mask: &[bool],
    owner_changed_mask: &[bool],
    expected_semantic_part_ids: &[&str],
) -> Result<RasterSourceAttributionDiagnostic, RuntimeError> {
    let pixel_count = attribution.width as usize * attribution.height as usize;
    if attribution.width != 512
        || attribution.height != 512
        || attribution.triangle_ids_le.len() != pixel_count * 4
        || [reviewed_region_mask, expected_void_mask, owner_changed_mask]
            .iter()
            .any(|mask| mask.len() != pixel_count)
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_INVALID: attribution and masks must be fixed 512x512",
        ));
    }

    let mut accumulators = BTreeMap::<(String, String), RasterSourceAttributionAccumulator>::new();
    let mut visible_pixel_count = 0_u64;
    let mut background_pixel_count = 0_u64;
    let mut reviewed_region_attributed_pixel_count = 0_u64;
    let mut expected_void_attributed_pixel_count = 0_u64;
    let mut owner_changed_attributed_pixel_count = 0_u64;
    for (pixel_index, bytes) in attribution.triangle_ids_le.chunks_exact(4).enumerate() {
        let triangle_id = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if triangle_id == u32::MAX {
            background_pixel_count += 1;
            continue;
        }
        let source = attribution
            .sources
            .get(triangle_id as usize)
            .filter(|source| source.triangle_index == triangle_id)
            .ok_or_else(|| {
                invalid("RASTER_SOURCE_ATTRIBUTION_INVALID: pixel triangle source is missing")
            })?;
        visible_pixel_count += 1;
        reviewed_region_attributed_pixel_count += u64::from(reviewed_region_mask[pixel_index]);
        expected_void_attributed_pixel_count += u64::from(expected_void_mask[pixel_index]);
        owner_changed_attributed_pixel_count += u64::from(owner_changed_mask[pixel_index]);
        let entry = accumulators
            .entry((
                source.semantic_part_id.clone(),
                source.source_node_id.clone(),
            ))
            .or_default();
        entry.triangle_ids.insert(triangle_id);
        entry
            .lineage_source_node_ids
            .extend(source.lineage_source_node_ids.iter().cloned());
        entry
            .material_zone_ids
            .insert(source.material_zone_id.clone());
        entry.mesh_indices.insert(source.mesh_index);
        entry.primitive_indices.insert(source.primitive_index);
        entry.visible_pixel_count += 1;
        entry.reviewed_region_pixel_count += u64::from(reviewed_region_mask[pixel_index]);
        entry.expected_void_pixel_count += u64::from(expected_void_mask[pixel_index]);
        entry.owner_changed_pixel_count += u64::from(owner_changed_mask[pixel_index]);
    }

    let mut sources = accumulators
        .into_iter()
        .map(
            |((semantic_part_id, source_node_id), accumulator)| RasterSourceAttributionRow {
                semantic_part_id,
                source_node_id,
                lineage_source_node_ids: accumulator.lineage_source_node_ids.into_iter().collect(),
                material_zone_ids: accumulator.material_zone_ids.into_iter().collect(),
                mesh_indices: accumulator.mesh_indices.into_iter().collect(),
                primitive_indices: accumulator.primitive_indices.into_iter().collect(),
                triangle_count: accumulator.triangle_ids.len() as u64,
                visible_pixel_count: accumulator.visible_pixel_count,
                reviewed_region_pixel_count: accumulator.reviewed_region_pixel_count,
                expected_void_pixel_count: accumulator.expected_void_pixel_count,
                owner_changed_pixel_count: accumulator.owner_changed_pixel_count,
            },
        )
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| {
        right
            .owner_changed_pixel_count
            .cmp(&left.owner_changed_pixel_count)
            .then_with(|| {
                right
                    .expected_void_pixel_count
                    .cmp(&left.expected_void_pixel_count)
            })
            .then_with(|| left.semantic_part_id.cmp(&right.semantic_part_id))
            .then_with(|| left.source_node_id.cmp(&right.source_node_id))
    });
    let (highest_impact_source, highest_impact_basis, highest_impact_pixel_count) =
        select_unique_highest_impact_source(&sources)?;
    let highest_impact_semantic_match = expected_semantic_part_ids.is_empty()
        || expected_semantic_part_ids
            .iter()
            .any(|part_id| *part_id == highest_impact_source.semantic_part_id);
    let repair_target_status = if highest_impact_semantic_match {
        "UNIQUE_HIGHEST_IMPACT_SOURCE_OBSERVED"
    } else {
        "BLOCKED_REVIEW_REGION_SEMANTIC_MISMATCH"
    };

    Ok(RasterSourceAttributionDiagnostic {
        width: attribution.width,
        height: attribution.height,
        visible_pixel_count,
        background_pixel_count,
        reviewed_region_attributed_pixel_count,
        expected_void_attributed_pixel_count,
        owner_changed_attributed_pixel_count,
        triangle_ids_sha256: attribution.triangle_ids_sha256.clone(),
        source_table_sha256: attribution.source_table_sha256.clone(),
        sources,
        highest_impact_source,
        highest_impact_basis: highest_impact_basis.to_owned(),
        highest_impact_pixel_count,
        expected_semantic_part_ids: expected_semantic_part_ids
            .iter()
            .map(|part_id| (*part_id).to_owned())
            .collect(),
        highest_impact_semantic_match,
        repair_target_status: repair_target_status.to_owned(),
        render_worker_build_cohort_sha256: attribution.build_cohort_sha256.clone(),
        status: "TRANSIENT_SOURCE_ATTRIBUTION_OBSERVED".to_owned(),
        diagnostic_only: true,
        promotable: false,
        runtime_write: false,
        production_stage_advanced: false,
    })
}

pub(crate) fn render_glb_raster_source_attribution_diagnostic(
    glb: &[u8],
    camera: &Value,
    reviewed_region_mask: &[bool],
    expected_void_mask: &[bool],
    owner_changed_mask: &[bool],
    expected_semantic_part_ids: &[&str],
) -> Result<RasterSourceAttributionDiagnostic, RuntimeError> {
    let attribution = super::render_worker::render_glb_raster_attribution(glb, camera)
        .map_err(|_| invalid("RASTER_SOURCE_ATTRIBUTION_WORKER_FAILED"))?;
    raster_source_attribution_diagnostic(
        &attribution,
        reviewed_region_mask,
        expected_void_mask,
        owner_changed_mask,
        expected_semantic_part_ids,
    )
}

fn dilated_boundary_band(boundary: &[bool], radius: usize) -> Vec<bool> {
    let mut band = vec![false; boundary.len()];
    for (index, is_boundary) in boundary.iter().enumerate() {
        if !*is_boundary {
            continue;
        }
        let x = index % 512;
        let y = index / 512;
        let x0 = x.saturating_sub(radius);
        let x1 = (x + radius).min(511);
        let y0 = y.saturating_sub(radius);
        let y1 = (y + radius).min(511);
        for yy in y0..=y1 {
            for xx in x0..=x1 {
                band[yy * 512 + xx] = true;
            }
        }
    }
    band
}

fn mask_centroid_milli(mask: &[bool]) -> Option<[i64; 2]> {
    let (_, centroid) = mask_bbox_and_centroid(mask)?;
    Some([
        (centroid[0] * 1000.0).round() as i64,
        (centroid[1] * 1000.0).round() as i64,
    ])
}

/// Compare two exact semantic Part-ID masks against the same reviewed
/// subtract-region masks.  This is intentionally a pure diagnostic: it does
/// not read Runtime state, write CAS/SQLite, or promote any candidate.
pub(crate) fn owner_mask_hamming_diagnostic(
    baseline_owner_mask: &[bool],
    trial_owner_mask: &[bool],
    region_mask: &[bool],
    expected_void: &[bool],
) -> Result<OwnerMaskHammingDiagnostic, RuntimeError> {
    let expected_len = 512 * 512;
    if [
        baseline_owner_mask,
        trial_owner_mask,
        region_mask,
        expected_void,
    ]
    .iter()
    .any(|mask| mask.len() != expected_len)
    {
        return Err(invalid(
            "OWNER_MASK_HAMMING_DIAGNOSTIC_INVALID: masks must be 512x512",
        ));
    }
    let expected_boundary = boundary_mask(expected_void);
    let expected_boundary_band_r1 = dilated_boundary_band(&expected_boundary, 1);
    let expected_boundary_band_r2 = dilated_boundary_band(&expected_boundary, 2);
    let expected_boundary_band_r4 = dilated_boundary_band(&expected_boundary, 4);
    let mut changed_mask = vec![false; expected_len];
    let mut baseline_owner_pixel_count = 0_u64;
    let mut trial_owner_pixel_count = 0_u64;
    let mut owner_mask_changed_pixel_count = 0_u64;
    let mut owner_mask_changed_inside_expected_void_pixel_count = 0_u64;
    let mut owner_mask_changed_inside_region_outside_expected_void_pixel_count = 0_u64;
    let mut owner_mask_changed_outside_reviewed_region_pixel_count = 0_u64;
    let mut baseline_owner_expected_void_overlap_pixel_count = 0_u64;
    let mut trial_owner_expected_void_overlap_pixel_count = 0_u64;
    let mut changed_expected_boundary_pixel_count = 0_u64;
    let mut changed_expected_boundary_band_r1_pixel_count = 0_u64;
    let mut changed_expected_boundary_band_r2_pixel_count = 0_u64;
    for index in 0..expected_len {
        let baseline_owner = baseline_owner_mask[index];
        let trial_owner = trial_owner_mask[index];
        let changed = baseline_owner != trial_owner;
        changed_mask[index] = changed;
        baseline_owner_pixel_count += u64::from(baseline_owner);
        trial_owner_pixel_count += u64::from(trial_owner);
        if baseline_owner && expected_void[index] {
            baseline_owner_expected_void_overlap_pixel_count += 1;
        }
        if trial_owner && expected_void[index] {
            trial_owner_expected_void_overlap_pixel_count += 1;
        }
        if !changed {
            continue;
        }
        owner_mask_changed_pixel_count += 1;
        if expected_void[index] {
            owner_mask_changed_inside_expected_void_pixel_count += 1;
        } else if region_mask[index] {
            owner_mask_changed_inside_region_outside_expected_void_pixel_count += 1;
        } else {
            owner_mask_changed_outside_reviewed_region_pixel_count += 1;
        }
        if expected_boundary[index] {
            changed_expected_boundary_pixel_count += 1;
        }
        if expected_boundary_band_r1[index] {
            changed_expected_boundary_band_r1_pixel_count += 1;
        }
        if expected_boundary_band_r2[index] {
            changed_expected_boundary_band_r2_pixel_count += 1;
        }
    }
    let changed_expected_boundary_band_r4_pixel_count = changed_mask
        .iter()
        .zip(expected_boundary_band_r4.iter())
        .filter(|(changed, band)| **changed && **band)
        .count() as u64;
    let classification = if owner_mask_changed_pixel_count == 0 {
        "MASK_UNCHANGED"
    } else if owner_mask_changed_inside_expected_void_pixel_count == 0
        && owner_mask_changed_inside_region_outside_expected_void_pixel_count == 0
    {
        "VISIBLE_CHANGE_OUTSIDE_REVIEWED_REGION"
    } else if owner_mask_changed_inside_expected_void_pixel_count == 0 {
        "VISIBLE_CHANGE_INSIDE_REGION_OUTSIDE_EXPECTED_VOID"
    } else if changed_expected_boundary_band_r4_pixel_count == 0 {
        "BOUNDARY_BAND_UNTOUCHED"
    } else {
        "OWNER_PIXEL_CHANGE_INSIDE_EXPECTED_VOID"
    };
    Ok(OwnerMaskHammingDiagnostic {
        expected_void_pixel_count: expected_void.iter().filter(|pixel| **pixel).count() as u64,
        expected_boundary_pixel_count: expected_boundary.iter().filter(|pixel| **pixel).count()
            as u64,
        baseline_owner_pixel_count,
        trial_owner_pixel_count,
        owner_mask_changed_pixel_count,
        owner_mask_changed_inside_expected_void_pixel_count,
        owner_mask_changed_inside_region_outside_expected_void_pixel_count,
        owner_mask_changed_outside_reviewed_region_pixel_count,
        baseline_owner_expected_void_overlap_pixel_count,
        trial_owner_expected_void_overlap_pixel_count,
        owner_expected_void_overlap_delta_pixel_count: trial_owner_expected_void_overlap_pixel_count
            as i64
            - baseline_owner_expected_void_overlap_pixel_count as i64,
        changed_expected_boundary_pixel_count,
        changed_expected_boundary_band_r1_pixel_count,
        changed_expected_boundary_band_r2_pixel_count,
        changed_expected_boundary_band_r4_pixel_count,
        changed_bbox_px: mask_bbox_and_centroid(&changed_mask).map(|(bbox, _)| bbox),
        changed_centroid_milli_px: mask_centroid_milli(&changed_mask),
        classification: classification.to_owned(),
    })
}

/// Decode one fixed 512x512 RGBA AOV and return an exact changed-pixel mask.
/// Callers may label the result as unavailable when a render set omits the
/// requested pass; no renderer schema extension is needed for 04U.
pub(crate) fn fixed_aov_changed_mask(
    baseline_png: &[u8],
    trial_png: &[u8],
    label: &str,
) -> Result<Vec<bool>, RuntimeError> {
    let baseline = decode_image(baseline_png, label)?;
    let trial = decode_image(trial_png, label)?;
    Ok(baseline
        .pixels()
        .zip(trial.pixels())
        .map(|(baseline, trial)| baseline.0 != trial.0)
        .collect())
}

/// Expose the canonical reviewed-region masks to a private, read-only
/// diagnostic without exposing contour rasterization as a Runtime write path.
pub(crate) fn reviewed_region_owner_audit_masks_with_rotation(
    structure: &Value,
    target_mask: &[bool],
    crop: [f64; 4],
    rotation_degrees: f64,
    structure_id: &str,
) -> Result<(String, Vec<bool>, Vec<bool>, Vec<bool>), RuntimeError> {
    let (region_hash, region_mask, expected_void) =
        reviewed_region_expected_void_mask_with_rotation(
            structure,
            target_mask,
            crop,
            rotation_degrees,
            structure_id,
        )?;
    let expected_boundary = boundary_mask(&expected_void);
    Ok((region_hash, region_mask, expected_void, expected_boundary))
}

fn contour_from_value(value: Option<&Value>) -> Option<Vec<[f64; 2]>> {
    let points = value?.as_array()?;
    let mut result = Vec::with_capacity(points.len());
    for point in points {
        let array = point.as_array()?;
        if array.len() != 2 {
            return None;
        }
        result.push([array[0].as_f64()?, array[1].as_f64()?]);
    }
    (result.len() >= 3).then_some(result)
}

fn project_point_to_view(
    point: [f64; 2],
    crop: [f64; 4],
    rotation_degrees: f64,
    label: &str,
) -> Result<[f64; 2], RuntimeError> {
    let [crop_x, crop_y, crop_width, crop_height] = crop;
    let epsilon = 1e-9;
    if point.iter().any(|value| !value.is_finite())
        || point[0] < crop_x - epsilon
        || point[0] > crop_x + crop_width + epsilon
        || point[1] < crop_y - epsilon
        || point[1] > crop_y + crop_height + epsilon
    {
        return Err(invalid(format!(
            "FORM_ART_EVIDENCE_VIEW_CROP_MISMATCH: {label} lies outside the bound crop"
        )));
    }
    let local = [
        ((point[0] - crop_x) / crop_width).clamp(0.0, 1.0),
        ((point[1] - crop_y) / crop_height).clamp(0.0, 1.0),
    ];
    Ok(super::rotate_reference_view_point(local, rotation_degrees))
}

fn decode_image(bytes: &[u8], label: &str) -> Result<image::RgbaImage, RuntimeError> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| invalid(format!("{label} image is invalid: {error}")))?
        .to_rgba8();
    if image.width() != 512 || image.height() != 512 {
        return Err(invalid(format!("{label} must be 512x512")));
    }
    Ok(image)
}

fn edge_from_aovs(
    silhouette: &[bool],
    part_png: &[u8],
    depth_png: &[u8],
    normal_png: &[u8],
    expected: &[String],
) -> Result<Vec<bool>, RuntimeError> {
    let mut edge = boundary_mask(silhouette);
    let part = decode_image(part_png, "part-id")?;
    let depth = decode_image(depth_png, "depth")?;
    let normal = decode_image(normal_png, "normal")?;
    let mut known_parts = 0_u64;
    for id in expected {
        if super::decode_part_mask(part_png, id, expected).is_some() {
            known_parts += 1;
        }
    }
    if known_parts == 0 {
        return Ok(edge);
    }
    for y in 0..512u32 {
        for x in 0..512u32 {
            let i = y as usize * 512 + x as usize;
            let p = part.get_pixel(x, y).0;
            for (nx, ny) in [
                (x.saturating_sub(1), y),
                ((x + 1).min(511), y),
                (x, y.saturating_sub(1)),
                (x, (y + 1).min(511)),
            ] {
                let n = part.get_pixel(nx, ny).0;
                let d = depth.get_pixel(x, y).0;
                let dn = depth.get_pixel(nx, ny).0;
                let q = normal.get_pixel(x, y).0;
                let qn = normal.get_pixel(nx, ny).0;
                if p != n
                    || d.iter()
                        .zip(dn.iter())
                        .map(|(a, b)| a.abs_diff(*b) as u32)
                        .sum::<u32>()
                        > 48
                    || q.iter()
                        .zip(qn.iter())
                        .map(|(a, b)| a.abs_diff(*b) as u32)
                        .sum::<u32>()
                        > 96
                {
                    edge[i] = true;
                    break;
                }
            }
        }
    }
    Ok(edge)
}

fn resample_polyline(vertices: &[(f64, f64)]) -> Option<Vec<(f64, f64)>> {
    if vertices.len() < 2 {
        return None;
    }
    let mut cumulative = Vec::with_capacity(vertices.len());
    cumulative.push(0.0);
    for segment in vertices.windows(2) {
        let distance = (segment[1].0 - segment[0].0).hypot(segment[1].1 - segment[0].1);
        if !distance.is_finite() {
            return None;
        }
        cumulative.push(cumulative.last().copied().unwrap_or(0.0) + distance);
    }
    let total = cumulative.last().copied().unwrap_or(0.0);
    if !total.is_finite() || total <= f64::EPSILON {
        return None;
    }
    let sample_count = (total.ceil() as usize + 1).clamp(2, MAX_SAMPLES_PER_FLOW);
    let mut samples = Vec::with_capacity(sample_count);
    for sample_index in 0..sample_count {
        let distance = total * sample_index as f64 / (sample_count - 1) as f64;
        let mut segment_index = cumulative.partition_point(|value| *value <= distance);
        segment_index = segment_index.saturating_sub(1).min(vertices.len() - 2);
        let start_distance = cumulative[segment_index];
        let segment_distance = cumulative[segment_index + 1] - start_distance;
        let t = if segment_distance <= f64::EPSILON {
            0.0
        } else {
            ((distance - start_distance) / segment_distance).clamp(0.0, 1.0)
        };
        samples.push((
            vertices[segment_index].0
                + (vertices[segment_index + 1].0 - vertices[segment_index].0) * t,
            vertices[segment_index].1
                + (vertices[segment_index + 1].1 - vertices[segment_index].1) * t,
        ));
    }
    Some(samples)
}

fn strict_segment_intersection(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let cross = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    ((ab_c > 1e-6 && ab_d < -1e-6) || (ab_c < -1e-6 && ab_d > 1e-6))
        && ((cd_a > 1e-6 && cd_b < -1e-6) || (cd_a < -1e-6 && cd_b > 1e-6))
}

fn trace_crossing_count(trace: &[(f64, f64)]) -> u64 {
    let mut crossings = 0usize;
    for first in 0..trace.len().saturating_sub(1) {
        for second in (first + 2)..trace.len().saturating_sub(1) {
            if strict_segment_intersection(
                trace[first],
                trace[first + 1],
                trace[second],
                trace[second + 1],
            ) {
                crossings += 1;
            }
        }
    }
    for first in 0..trace.len() {
        for second in (first + 2)..trace.len() {
            if (trace[first].0 - trace[second].0).abs() <= 0.5
                && (trace[first].1 - trace[second].1).abs() <= 0.5
            {
                crossings += 1;
            }
        }
    }
    crossings.min(512) as u64
}

fn line_rows_with_rotation(
    structure: Option<&Value>,
    visual_confirmed: bool,
    edge: &[bool],
    crop: [f64; 4],
    rotation_degrees: f64,
) -> Result<(String, Vec<ProductionWeaponFormArtEvidenceLineFlowRow>), RuntimeError> {
    let Some(structure) = structure else {
        return Ok(("unknown".into(), Vec::new()));
    };
    let flows = structure
        .get("line_flows")
        .and_then(Value::as_array)
        .map_or(&[][..], |values| values.as_slice());
    if !visual_confirmed {
        return Ok(("unknown".into(), Vec::new()));
    }
    if flows.is_empty() {
        return Ok(("not-applicable".into(), Vec::new()));
    }
    if flows.len() > MAX_LINE_FLOWS_PER_VIEW {
        return Err(invalid(
            "FORM_ART_EVIDENCE_BUDGET_EXCEEDED: line flow count",
        ));
    }

    let mut plans = Vec::with_capacity(flows.len());
    let mut total_samples = 0usize;
    for flow in flows {
        let points = flow
            .get("points")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("FORM_ART_EVIDENCE_LINE_FLOW_INVALID: points"))?;
        if points.len() > MAX_LINE_POINTS_PER_FLOW {
            return Err(invalid(
                "FORM_ART_EVIDENCE_BUDGET_EXCEEDED: line flow points",
            ));
        }
        let mut vertices = Vec::with_capacity(points.len());
        for point in points {
            let point = point
                .as_array()
                .ok_or_else(|| invalid("FORM_ART_EVIDENCE_LINE_FLOW_INVALID: point"))?;
            let (Some(x), Some(y)) = (
                point.first().and_then(Value::as_f64),
                point.get(1).and_then(Value::as_f64),
            ) else {
                return Err(invalid(
                    "FORM_ART_EVIDENCE_LINE_FLOW_INVALID: point coordinates",
                ));
            };
            if !x.is_finite() || !y.is_finite() {
                return Err(invalid(
                    "FORM_ART_EVIDENCE_LINE_FLOW_INVALID: non-finite point",
                ));
            }
            let projected =
                project_point_to_view([x, y], crop, rotation_degrees, "line-flow point")?;
            vertices.push((projected[0] * 511.0, projected[1] * 511.0));
        }
        let samples = resample_polyline(&vertices);
        total_samples = total_samples.saturating_add(samples.as_ref().map_or(0, Vec::len));
        if total_samples > MAX_TOTAL_LINE_SAMPLES_PER_VIEW {
            return Err(invalid(
                "FORM_ART_EVIDENCE_BUDGET_EXCEEDED: line flow samples",
            ));
        }
        plans.push((vertices, samples));
    }
    let mut rows = Vec::new();
    for (flow, (vertices, samples)) in flows.iter().zip(plans) {
        let id = flow
            .get("line_flow_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown-line")
            .to_owned();
        let expected_hash = canonical_json_hash(flow);
        let flow_visibility = flow
            .get("visibility")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let mut cumulative = Vec::with_capacity(vertices.len().max(1));
        cumulative.push(0.0);
        for segment in vertices.windows(2) {
            let distance = (segment[1].0 - segment[0].0).hypot(segment[1].1 - segment[0].1);
            cumulative.push(cumulative.last().copied().unwrap_or(0.0) + distance);
        }
        let total_length = cumulative.last().copied().unwrap_or(0.0);
        let sample_points = samples.as_deref().unwrap_or(&[]);
        let sample_count = sample_points.len();
        let project_on_path = |point: (f64, f64)| -> f64 {
            if vertices.len() < 2 || total_length <= f64::EPSILON {
                return 0.0;
            }
            let mut best_distance = f64::INFINITY;
            let mut best_position = 0.0;
            for (index, segment) in vertices.windows(2).enumerate() {
                let (a, b) = (segment[0], segment[1]);
                let dx = b.0 - a.0;
                let dy = b.1 - a.1;
                let length_sq = dx.mul_add(dx, dy * dy);
                let t = if length_sq <= f64::EPSILON {
                    0.0
                } else {
                    ((point.0 - a.0).mul_add(dx, (point.1 - a.1) * dy) / length_sq).clamp(0.0, 1.0)
                };
                let projected = (a.0 + dx * t, a.1 + dy * t);
                let distance = (point.0 - projected.0).hypot(point.1 - projected.1);
                if distance < best_distance {
                    best_distance = distance;
                    best_position = cumulative[index] + length_sq.sqrt() * t;
                }
            }
            best_position / total_length
        };
        let mut hits = 0usize;
        let mut distances = Vec::new();
        let mut trace_positions = Vec::new();
        let mut trace_points = Vec::new();
        for (x, y) in sample_points {
            let mut best = 1000.0f64;
            let mut best_point = None;
            for yy in (y.round() as i32 - 12)..=(y.round() as i32 + 12) {
                for xx in (x.round() as i32 - 12)..=(x.round() as i32 + 12) {
                    if xx >= 0
                        && yy >= 0
                        && xx < 512
                        && yy < 512
                        && edge[yy as usize * 512 + xx as usize]
                    {
                        let distance = (xx as f64 - x).hypot(yy as f64 - y);
                        if distance < best {
                            best = distance;
                            best_point = Some((xx as f64, yy as f64));
                        }
                    }
                }
            }
            if best <= 12.0 {
                hits += 1;
                if let Some(point) = best_point {
                    trace_points.push(point);
                    trace_positions.push(project_on_path(point));
                }
            }
            distances.push(best);
        }
        let coverage = if sample_count == 0 {
            0
        } else {
            (hits as u64 * 1000 / sample_count as u64).min(1000)
        };
        let mut longest_run = 0usize;
        let mut current_run = 0usize;
        for distance in &distances {
            if *distance <= 12.0 {
                current_run += 1;
                longest_run = longest_run.max(current_run);
            } else {
                current_run = 0;
            }
        }
        let continuity = if sample_count == 0 {
            0
        } else {
            (longest_run as u64 * 1000 / sample_count as u64).min(1000)
        };
        let chamfer = if distances.is_empty() {
            100000
        } else {
            ((distances.iter().sum::<f64>() / distances.len() as f64 / 512.0) * 100000.0).round()
                as u64
        };
        let max_deviation = distances.iter().copied().fold(0.0, f64::max);
        let max_deviation = ((max_deviation / 512.0) * 100000.0).round() as u64;
        let mut reversals = 0usize;
        for pair in trace_positions.windows(2) {
            if pair[1] + 0.002 < pair[0] {
                reversals += 1;
            }
        }
        let direction_order = if trace_positions.len() < 2 {
            0
        } else {
            ((trace_positions.len() - 1 - reversals) as u64 * 1000
                / (trace_positions.len() - 1) as u64)
                .min(1000)
        };
        let status = if !matches!(flow_visibility, "observed" | "inferred") {
            "unknown"
        } else if sample_count < 2 || hits == 0 {
            "unknown"
        } else if flow_visibility == "inferred" || hits < sample_count {
            "inferred"
        } else {
            "observed"
        };
        let crossing_count = trace_crossing_count(&trace_points);
        rows.push(ProductionWeaponFormArtEvidenceLineFlowRow {
            line_flow_id: id,
            expected_line_canonical_sha256: expected_hash,
            coverage_milli: coverage,
            continuity_milli: continuity,
            symmetric_chamfer_milli: chamfer.min(100000),
            max_deviation_milli: max_deviation.min(100000),
            direction_order_milli: direction_order,
            duplicate_crossing_count: crossing_count,
            status: status.into(),
        });
    }
    let status = if rows.is_empty() {
        "unknown"
    } else if rows.iter().any(|row| row.status == "unknown") {
        "unknown"
    } else if rows.iter().any(|row| row.status == "inferred") {
        "inferred"
    } else {
        "observed"
    };
    Ok((status.into(), rows))
}

fn line_rows(
    structure: Option<&Value>,
    visual_confirmed: bool,
    edge: &[bool],
    crop: [f64; 4],
) -> Result<(String, Vec<ProductionWeaponFormArtEvidenceLineFlowRow>), RuntimeError> {
    line_rows_with_rotation(structure, visual_confirmed, edge, crop, 0.0)
}

pub(crate) fn negative_rows_with_rotation(
    structure: Option<&Value>,
    visual_confirmed: bool,
    target_mask: &[bool],
    model_mask: &[bool],
    crop: [f64; 4],
    rotation_degrees: f64,
) -> Result<(String, Vec<ProductionWeaponFormArtEvidenceNegativeSpaceRow>), RuntimeError> {
    let Some(structure) = structure else {
        return Ok(("unknown".into(), Vec::new()));
    };
    let regions = structure
        .get("regions")
        .and_then(Value::as_array)
        .map_or(&[][..], |values| values.as_slice());
    let subtracts = regions
        .iter()
        .filter(|region| region.get("mask_operation").and_then(Value::as_str) == Some("subtract"))
        .collect::<Vec<_>>();
    if !visual_confirmed {
        return Ok(("unknown".into(), Vec::new()));
    }
    if subtracts.is_empty()
        && regions.iter().any(|region| {
            region.get("visual_role").and_then(Value::as_str) == Some("open-frame")
                && region.get("boundary_relationship").and_then(Value::as_str) == Some("enclosed")
                && region.get("visibility").and_then(Value::as_str) == Some("observed")
        })
    {
        // A reviewed bbox/open-frame region is useful visual evidence but is
        // not a pixel-exact hole. Treating it as `not-applicable` would erase
        // a known review obligation, while treating it as `subtract` would
        // fabricate a cutout. Keep the gate explicitly unknown until an exact
        // enclosed subtract contour is separately user-confirmed.
        return Ok(("unknown".into(), Vec::new()));
    }
    if subtracts.is_empty() {
        return Ok(("not-applicable".into(), Vec::new()));
    }
    if subtracts.len() > MAX_NEGATIVE_REGIONS_PER_VIEW {
        return Err(invalid(
            "FORM_ART_EVIDENCE_BUDGET_EXCEEDED: negative-space region count",
        ));
    }
    let total_points = subtracts
        .iter()
        .map(|region| {
            region
                .get("contour_points")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        })
        .sum::<usize>();
    if total_points > MAX_NEGATIVE_POINTS_PER_VIEW {
        return Err(invalid(
            "FORM_ART_EVIDENCE_BUDGET_EXCEEDED: negative-space points",
        ));
    }
    let mut rows = Vec::new();
    for region in subtracts {
        let id = region
            .get("structure_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown-structure")
            .to_owned();
        let hash = canonical_json_hash(region);
        let contour = contour_from_value(region.get("contour_points"));
        let Some(contour) = contour else {
            rows.push(ProductionWeaponFormArtEvidenceNegativeSpaceRow {
                structure_id: id,
                expected_region_canonical_sha256: hash,
                iou_milli: 0,
                boundary_f1_milli: 0,
                area_ratio_milli: 0,
                centroid_error_milli: 100000,
                sealed: false,
                missing: true,
                status: "unknown".into(),
            });
            continue;
        };
        let contour = contour
            .into_iter()
            .map(|point| {
                project_point_to_view(
                    point,
                    crop,
                    rotation_degrees,
                    "negative-space contour point",
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let region_mask = super::rasterize_contour(&contour);
        let expected: Vec<bool> = region_mask
            .iter()
            .zip(target_mask.iter())
            .map(|(region, target)| *region && !*target)
            .collect();
        let actual: Vec<bool> = region_mask
            .iter()
            .zip(model_mask.iter())
            .map(|(region, model)| *region && !*model)
            .collect();
        let (iou, f1, ratio, centroid, nonempty) = metrics(&expected, &actual);
        // A reviewed subtract contour with no visible opening in the model is
        // sealed, not missing: the expected region is present and hash-bound.
        // `missing` is reserved for an unavailable/invalid expected contour,
        // which is handled by the branch above. Store intentionally rejects
        // the contradictory `sealed && missing` state.
        let sealed = expected.iter().any(|v| *v) && actual.iter().all(|v| !*v);
        let missing = false;
        rows.push(ProductionWeaponFormArtEvidenceNegativeSpaceRow {
            structure_id: id,
            expected_region_canonical_sha256: hash,
            iou_milli: iou,
            boundary_f1_milli: f1,
            area_ratio_milli: ratio,
            centroid_error_milli: centroid,
            sealed,
            missing,
            status: if nonempty { "observed" } else { "unknown" }.into(),
        });
    }
    let status = if rows.is_empty() {
        "unknown"
    } else if rows.iter().any(|row| row.status == "unknown") {
        "unknown"
    } else if rows.iter().any(|row| row.status == "inferred") {
        "inferred"
    } else {
        "observed"
    };
    Ok((status.into(), rows))
}

pub(crate) fn negative_rows(
    structure: Option<&Value>,
    visual_confirmed: bool,
    target_mask: &[bool],
    model_mask: &[bool],
    crop: [f64; 4],
) -> Result<(String, Vec<ProductionWeaponFormArtEvidenceNegativeSpaceRow>), RuntimeError> {
    negative_rows_with_rotation(
        structure,
        visual_confirmed,
        target_mask,
        model_mask,
        crop,
        0.0,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PartOwnedNegativeSpaceDiagnostic {
    pub structure_id: String,
    pub expected_region_canonical_sha256: String,
    pub owner_part_id: String,
    pub expected_void_pixel_count: u64,
    pub owner_pixel_count: u64,
    pub owner_region_pixel_count: u64,
    pub owner_expected_void_overlap_pixel_count: u64,
    pub owner_expected_void_overlap_milli: u64,
    pub owner_boundary_adjacency_pixel_count: u64,
    pub owner_boundary_adjacency_milli: u64,
    pub expected_void_bbox_px: [u64; 4],
    pub owner_bbox_px: [u64; 4],
    pub owner_minus_expected_bbox_edge_delta_px: [i64; 4],
    pub owner_minus_expected_centroid_delta_milli_px: [i64; 2],
    pub status: String,
}

fn mask_bbox_and_centroid(mask: &[bool]) -> Option<([u64; 4], [f64; 2])> {
    let mut min_x = 512_u64;
    let mut min_y = 512_u64;
    let mut max_x = 0_u64;
    let mut max_y = 0_u64;
    let mut sum_x = 0_u64;
    let mut sum_y = 0_u64;
    let mut count = 0_u64;
    for (index, pixel) in mask.iter().enumerate() {
        if !*pixel {
            continue;
        }
        let x = (index % 512) as u64;
        let y = (index / 512) as u64;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        sum_x += x;
        sum_y += y;
        count += 1;
    }
    (count > 0).then_some((
        [min_x, min_y, max_x, max_y],
        [sum_x as f64 / count as f64, sum_y as f64 / count as f64],
    ))
}

/// Decode one exact semantic Part from the fixed Part-ID AOV.
///
/// Unlike the general fitting decoder, this helper deliberately does not use
/// bilateral/rig aliases.  A production negative-space binding must name the
/// semantic ArtifactReadback Part (`rear-stock`), never one of its source
/// nodes (`rear-stock-lower-beam`).  Unknown palette colors and indices beyond
/// the exact ArtifactReadback vocabulary fail closed.
pub(crate) fn exact_part_id_mask(
    part_png: &[u8],
    part_ids: &[String],
    owner_part_id: &str,
) -> Result<Vec<bool>, RuntimeError> {
    if part_ids.is_empty()
        || part_ids.iter().collect::<BTreeSet<_>>().len() != part_ids.len()
        || part_ids
            .iter()
            .filter(|part_id| part_id.as_str() == owner_part_id)
            .count()
            != 1
    {
        return Err(invalid(
            "PART_OWNED_NEGATIVE_SPACE_BINDING_INVALID: exact owner vocabulary",
        ));
    }
    let image = decode_image(part_png, "part-id")?;
    let background = [8_u8, 12_u8, 18_u8, 255_u8];
    let mut mask = vec![false; 512 * 512];
    for (index, pixel) in image.pixels().enumerate() {
        let rgba = pixel.0;
        if rgba == background {
            continue;
        }
        let palette_index = super::part_color_index(rgba).ok_or_else(|| {
            invalid("PART_OWNED_NEGATIVE_SPACE_PALETTE_INVALID: unknown Part-ID color")
        })?;
        let observed_part_id = part_ids.get(palette_index).ok_or_else(|| {
            invalid("PART_OWNED_NEGATIVE_SPACE_PALETTE_INVALID: index outside ArtifactReadback")
        })?;
        mask[index] = observed_part_id == owner_part_id;
    }
    if !mask.iter().any(|pixel| *pixel) {
        return Err(invalid(
            "PART_OWNED_NEGATIVE_SPACE_BINDING_MISSING: owner Part is not visible",
        ));
    }
    Ok(mask)
}

/// Produce a supplemental, non-promoting Part-owned diagnostic for one exact
/// reviewed subtract contour.  This does not alter the canonical FormArt row:
/// the absolute negative-space gate continues to compare the reviewed target
/// against the complete model silhouette.  The Part-ID result only proves
/// whether the named semantic Part is spatially bound to that reviewed void.
pub(crate) fn part_owned_negative_space_diagnostic_with_rotation(
    structure: &Value,
    target_mask: &[bool],
    part_png: &[u8],
    part_ids: &[String],
    crop: [f64; 4],
    rotation_degrees: f64,
    structure_id: &str,
    owner_part_id: &str,
) -> Result<PartOwnedNegativeSpaceDiagnostic, RuntimeError> {
    if target_mask.len() != 512 * 512 {
        return Err(invalid(
            "PART_OWNED_NEGATIVE_SPACE_BINDING_INVALID: target mask dimensions",
        ));
    }
    let matches = structure
        .get("regions")
        .and_then(Value::as_array)
        .map_or(&[][..], |regions| regions.as_slice())
        .iter()
        .filter(|region| {
            region.get("structure_id").and_then(Value::as_str) == Some(structure_id)
                && region.get("mask_operation").and_then(Value::as_str) == Some("subtract")
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(invalid(
            "PART_OWNED_NEGATIVE_SPACE_BINDING_INVALID: exact subtract contour",
        ));
    }
    let region = matches[0];
    let region_hash = canonical_json_hash(region);
    let contour = contour_from_value(region.get("contour_points"))
        .ok_or_else(|| invalid("PART_OWNED_NEGATIVE_SPACE_BINDING_INVALID: contour unavailable"))?;
    let contour = contour
        .into_iter()
        .map(|point| {
            project_point_to_view(
                point,
                crop,
                rotation_degrees,
                "part-owned negative-space contour point",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let region_mask = super::rasterize_contour(&contour);
    let expected_void = region_mask
        .iter()
        .zip(target_mask.iter())
        .map(|(region, target)| *region && !*target)
        .collect::<Vec<_>>();
    let expected_void_pixel_count = expected_void.iter().filter(|pixel| **pixel).count() as u64;
    if expected_void_pixel_count == 0 {
        return Err(invalid(
            "PART_OWNED_NEGATIVE_SPACE_BINDING_INVALID: expected void is empty",
        ));
    }
    let owner_mask = exact_part_id_mask(part_png, part_ids, owner_part_id)?;
    let owner_pixel_count = owner_mask.iter().filter(|pixel| **pixel).count() as u64;
    let owner_region_pixel_count = owner_mask
        .iter()
        .zip(region_mask.iter())
        .filter(|(owner, region)| **owner && **region)
        .count() as u64;
    let owner_expected_void_overlap_pixel_count = owner_mask
        .iter()
        .zip(expected_void.iter())
        .filter(|(owner, expected)| **owner && **expected)
        .count() as u64;
    let owner_expected_void_overlap_milli =
        owner_expected_void_overlap_pixel_count * 1000 / expected_void_pixel_count;
    let expected_boundary = boundary_mask(&expected_void);
    let expected_boundary_pixel_count = expected_boundary.iter().filter(|pixel| **pixel).count();
    let mut owner_boundary_adjacency_pixel_count = 0_u64;
    for (index, boundary) in expected_boundary.iter().enumerate() {
        if !*boundary {
            continue;
        }
        let x = (index % 512) as i32;
        let y = (index / 512) as i32;
        let adjacent = (-2..=2).any(|dy| {
            (-2..=2).any(|dx| {
                let xx = x + dx;
                let yy = y + dy;
                xx >= 0
                    && yy >= 0
                    && xx < 512
                    && yy < 512
                    && owner_mask[yy as usize * 512 + xx as usize]
            })
        });
        if adjacent {
            owner_boundary_adjacency_pixel_count += 1;
        }
    }
    let owner_boundary_adjacency_milli = if expected_boundary_pixel_count == 0 {
        0
    } else {
        owner_boundary_adjacency_pixel_count * 1000 / expected_boundary_pixel_count as u64
    };
    let (expected_void_bbox_px, expected_void_centroid) = mask_bbox_and_centroid(&expected_void)
        .ok_or_else(|| {
            invalid("PART_OWNED_NEGATIVE_SPACE_BINDING_INVALID: expected void geometry")
        })?;
    let (owner_bbox_px, owner_centroid) = mask_bbox_and_centroid(&owner_mask)
        .ok_or_else(|| invalid("PART_OWNED_NEGATIVE_SPACE_BINDING_MISSING: owner Part geometry"))?;
    let owner_minus_expected_bbox_edge_delta_px = [
        owner_bbox_px[0] as i64 - expected_void_bbox_px[0] as i64,
        owner_bbox_px[1] as i64 - expected_void_bbox_px[1] as i64,
        owner_bbox_px[2] as i64 - expected_void_bbox_px[2] as i64,
        owner_bbox_px[3] as i64 - expected_void_bbox_px[3] as i64,
    ];
    let owner_minus_expected_centroid_delta_milli_px = [
        ((owner_centroid[0] - expected_void_centroid[0]) * 1000.0).round() as i64,
        ((owner_centroid[1] - expected_void_centroid[1]) * 1000.0).round() as i64,
    ];
    Ok(PartOwnedNegativeSpaceDiagnostic {
        structure_id: structure_id.to_owned(),
        expected_region_canonical_sha256: region_hash,
        owner_part_id: owner_part_id.to_owned(),
        expected_void_pixel_count,
        owner_pixel_count,
        owner_region_pixel_count,
        owner_expected_void_overlap_pixel_count,
        owner_expected_void_overlap_milli,
        owner_boundary_adjacency_pixel_count,
        owner_boundary_adjacency_milli,
        expected_void_bbox_px,
        owner_bbox_px,
        owner_minus_expected_bbox_edge_delta_px,
        owner_minus_expected_centroid_delta_milli_px,
        status: if owner_boundary_adjacency_pixel_count > 0 {
            "bound"
        } else {
            "unbound"
        }
        .to_owned(),
    })
}

pub(crate) fn part_owned_negative_space_diagnostic(
    structure: &Value,
    target_mask: &[bool],
    part_png: &[u8],
    part_ids: &[String],
    crop: [f64; 4],
    structure_id: &str,
    owner_part_id: &str,
) -> Result<PartOwnedNegativeSpaceDiagnostic, RuntimeError> {
    part_owned_negative_space_diagnostic_with_rotation(
        structure,
        target_mask,
        part_png,
        part_ids,
        crop,
        0.0,
        structure_id,
        owner_part_id,
    )
}

/// Read-only source attribution for a transient split Part-ID AOV.
///
/// The split IDs are diagnostic vocabulary only.  They do not change the
/// canonical `rear-stock` PartOutput, do not create a candidate, and cannot be
/// promoted to a production owner binding.  The semantic owner is deliberately
/// fixed to the union Part (`rear-stock`) so a source projection cannot be
/// mistaken for a different production Part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct SourcePartIdAttributionDiagnostic {
    pub structure_id: String,
    pub expected_region_canonical_sha256: String,
    pub semantic_owner_part_id: String,
    pub source_diagnostic_part_id: String,
    pub expected_void_pixel_count: u64,
    pub source_pixel_count: u64,
    pub source_region_pixel_count: u64,
    pub source_expected_void_overlap_pixel_count: u64,
    pub source_expected_void_overlap_milli: u64,
    pub source_boundary_adjacency_pixel_count: u64,
    pub source_boundary_adjacency_milli: u64,
    pub expected_void_bbox_px: [u64; 4],
    pub source_bbox_px: [u64; 4],
    pub source_minus_expected_void_bbox_edge_delta_px: [i64; 4],
    pub source_minus_expected_void_centroid_delta_milli_px: [i64; 2],
    pub diagnostic_only: bool,
    pub promotable: bool,
    pub status: String,
}

pub(crate) fn source_part_id_attribution_diagnostic_with_rotation(
    structure: &Value,
    target_mask: &[bool],
    diagnostic_part_png: &[u8],
    diagnostic_part_ids: &[String],
    crop: [f64; 4],
    rotation_degrees: f64,
    structure_id: &str,
    semantic_owner_part_id: &str,
    source_diagnostic_part_id: &str,
) -> Result<SourcePartIdAttributionDiagnostic, RuntimeError> {
    if semantic_owner_part_id != "rear-stock" {
        return Err(invalid(
            "SOURCE_PART_ID_ATTRIBUTION_BLOCKED: semantic owner must be rear-stock",
        ));
    }
    if !matches!(
        source_diagnostic_part_id,
        "rear-stock-upper-diagnostic" | "rear-stock-lower-diagnostic"
    ) {
        return Err(invalid(
            "SOURCE_PART_ID_ATTRIBUTION_BLOCKED: unknown source diagnostic Part-ID",
        ));
    }

    // This delegates the canonical expected-void projection and exact Part-ID
    // palette decoding to the existing read-only helpers.  An absent/empty
    // source mask therefore fails closed instead of producing zero metrics.
    let diagnostic = part_owned_negative_space_diagnostic_with_rotation(
        structure,
        target_mask,
        diagnostic_part_png,
        diagnostic_part_ids,
        crop,
        rotation_degrees,
        structure_id,
        source_diagnostic_part_id,
    )?;

    Ok(SourcePartIdAttributionDiagnostic {
        structure_id: diagnostic.structure_id,
        expected_region_canonical_sha256: diagnostic.expected_region_canonical_sha256,
        semantic_owner_part_id: semantic_owner_part_id.to_owned(),
        source_diagnostic_part_id: source_diagnostic_part_id.to_owned(),
        expected_void_pixel_count: diagnostic.expected_void_pixel_count,
        source_pixel_count: diagnostic.owner_pixel_count,
        source_region_pixel_count: diagnostic.owner_region_pixel_count,
        source_expected_void_overlap_pixel_count: diagnostic
            .owner_expected_void_overlap_pixel_count,
        source_expected_void_overlap_milli: diagnostic.owner_expected_void_overlap_milli,
        source_boundary_adjacency_pixel_count: diagnostic.owner_boundary_adjacency_pixel_count,
        source_boundary_adjacency_milli: diagnostic.owner_boundary_adjacency_milli,
        expected_void_bbox_px: diagnostic.expected_void_bbox_px,
        source_bbox_px: diagnostic.owner_bbox_px,
        source_minus_expected_void_bbox_edge_delta_px: diagnostic
            .owner_minus_expected_bbox_edge_delta_px,
        source_minus_expected_void_centroid_delta_milli_px: diagnostic
            .owner_minus_expected_centroid_delta_milli_px,
        diagnostic_only: true,
        promotable: false,
        status: "diagnostic-only".to_owned(),
    })
}

/// The only view transforms that the reviewed-region calibration is allowed
/// to consider.  The crop itself is never fitted: it is supplied by the
/// canonical ReferenceViewSpec crop and applied through
/// `project_point_to_view` below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReviewedRegionPartBindingTransform {
    Identity,
    HorizontalFlip,
    VerticalFlip,
    Rotate180,
}

pub(crate) fn reviewed_region_part_binding_transform_name(
    transform: ReviewedRegionPartBindingTransform,
) -> &'static str {
    match transform {
        ReviewedRegionPartBindingTransform::Identity => "identity",
        ReviewedRegionPartBindingTransform::HorizontalFlip => "horizontal-flip",
        ReviewedRegionPartBindingTransform::VerticalFlip => "vertical-flip",
        ReviewedRegionPartBindingTransform::Rotate180 => "rotate-180",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReviewedRegionPartBindingThresholds {
    pub min_owner_region_pixels: Option<u64>,
    pub min_boundary_adjacency_pixels: Option<u64>,
    pub max_owner_expected_void_overlap_milli: Option<u64>,
}

/// Depth validation for the exact semantic owner pixels in a candidate-bound
/// fixed 512x512 render.  The renderer uses the non-gray
/// `[8, 12, 18, 255]` background for all of its ordinary AOVs, so the
/// foreground domain is deliberately the Part-ID `rear-stock` mask rather
/// than every pixel in the depth image.  This keeps a background palette
/// detail from being mistaken for a malformed foreground depth sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DepthAovOwnerPixelValidation {
    pub depth_png_sha256: String,
    pub owner_pixel_count: u64,
    pub owner_depth_valid_pixel_count: u64,
    pub owner_depth_code_min: u8,
    pub owner_depth_code_max: u8,
    pub owner_depth_code_mean_milli: u64,
}

/// Read-only depth-aware owner-to-reviewed-void calibration.  This is a
/// loose identity observation and a separate strict zero-intrusion result:
/// callers may retain the useful owner/void offsets when strict quality is
/// blocked, but neither result is promotable or a camera/stage write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DepthAwareReviewedRegionPartBindingCalibration {
    pub structure_id: String,
    pub expected_region_canonical_sha256: String,
    pub owner_part_id: String,
    pub policy: String,
    pub depth_encoding: String,
    pub expected_void_mask_sha256: String,
    pub owner_mask_sha256: String,
    pub owner_region_mask_sha256: String,
    pub expected_boundary_mask_sha256: String,
    pub depth_png_sha256: String,
    pub expected_void_bbox_px: [u64; 4],
    pub owner_bbox_px: [u64; 4],
    pub owner_minus_expected_void_bbox_edge_delta_px: [i64; 4],
    pub owner_minus_expected_void_centroid_delta_milli_px: [i64; 2],
    pub expected_void_pixel_count: u64,
    pub expected_boundary_pixel_count: u64,
    pub owner_pixel_count: u64,
    pub owner_depth_valid_pixel_count: u64,
    pub owner_depth_code_min: u8,
    pub owner_depth_code_max: u8,
    pub owner_depth_code_mean_milli: u64,
    pub depth_boundary_sample_count: u64,
    pub depth_ordering_milli: i64,
    pub owner_region_pixel_count: u64,
    pub owner_expected_void_overlap_pixel_count: u64,
    pub owner_expected_void_overlap_milli: u64,
    pub owner_boundary_adjacency_pixel_count: u64,
    pub owner_boundary_adjacency_milli: u64,
    pub identity_transform_unique: bool,
    pub eligible_transform_count: u64,
    pub transform_rank_tie: bool,
    pub loose_identity_calibrated: bool,
    pub strict_zero_intrusion: bool,
    pub depth_status: String,
    pub quality_status: String,
    pub promotable: bool,
    pub status: String,
}

pub(crate) const DEPTH_AWARE_OWNER_VOID_POLICY: &str =
    "registered-camera-direct-part-id-depth-owner-void-calibration@1";
pub(crate) const DEPTH_AWARE_OWNER_VOID_DEPTH_ENCODING: &str =
    "render-worker-depth-v1|512x512|rgba8|rgb=1-depth-u8|alpha=255";

const STRICT_OWNER_VOID_MIN_EXPECTED_VOID_PIXELS: u64 = 256;
const STRICT_OWNER_VOID_MIN_EXPECTED_BOUNDARY_PIXELS: u64 = 64;
const STRICT_OWNER_VOID_MIN_OWNER_REGION_PIXELS: u64 = 128;
const STRICT_OWNER_VOID_MIN_BOUNDARY_ADJACENCY_PIXELS: u64 = 32;
const STRICT_OWNER_VOID_MIN_BOUNDARY_ADJACENCY_MILLI: u64 = 250;

/// Validate only the depth samples whose foreground ownership was established
/// by the exact Part-ID mask.  Depth is encoded by the fixed renderer as one
/// reversed-z u8 copied to RGB; alpha is opaque.  A malformed non-owner
/// background pixel is intentionally outside this helper's foreground
/// contract and cannot make an owner binding pass or fail.
pub(crate) fn validate_depth_aov_owner_pixels(
    depth_png: &[u8],
    owner_mask: &[bool],
) -> Result<DepthAovOwnerPixelValidation, RuntimeError> {
    if owner_mask.len() != 512 * 512 {
        return Err(invalid(
            "DEPTH_AWARE_OWNER_VOID_BLOCKED: owner mask must be 512x512",
        ));
    }
    let image = decode_image(depth_png, "depth")?;
    let mut owner_pixel_count = 0_u64;
    let mut owner_depth_valid_pixel_count = 0_u64;
    let mut owner_depth_code_min = u8::MAX;
    let mut owner_depth_code_max = 0_u8;
    let mut owner_depth_code_sum = 0_u64;
    for (index, pixel) in image.pixels().enumerate() {
        if !owner_mask[index] {
            continue;
        }
        owner_pixel_count += 1;
        let [red, green, blue, alpha] = pixel.0;
        if red != green || green != blue || alpha != 255 {
            return Err(invalid(
                "DEPTH_AWARE_OWNER_VOID_BLOCKED: owner depth pixel is not RGB-equal opaque RGBA",
            ));
        }
        owner_depth_valid_pixel_count += 1;
        owner_depth_code_min = owner_depth_code_min.min(red);
        owner_depth_code_max = owner_depth_code_max.max(red);
        owner_depth_code_sum += u64::from(red);
    }
    if owner_pixel_count == 0 {
        return Err(invalid(
            "DEPTH_AWARE_OWNER_VOID_BLOCKED: owner Part has no foreground pixels",
        ));
    }
    if owner_depth_valid_pixel_count != owner_pixel_count {
        return Err(invalid(
            "DEPTH_AWARE_OWNER_VOID_BLOCKED: owner depth foreground is incomplete",
        ));
    }
    Ok(DepthAovOwnerPixelValidation {
        depth_png_sha256: sha256_hex(depth_png),
        owner_pixel_count,
        owner_depth_valid_pixel_count,
        owner_depth_code_min,
        owner_depth_code_max,
        owner_depth_code_mean_milli: owner_depth_code_sum * 1000 / owner_pixel_count,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReviewedRegionPartBindingCandidate {
    pub transform: ReviewedRegionPartBindingTransform,
    pub overlap_iou_milli: u64,
    pub boundary_f1_milli: u64,
    pub bbox_edge_error_px: u64,
    pub centroid_error_px: u64,
    pub expected_void_bbox_px: [u64; 4],
    pub owner_bbox_px: [u64; 4],
    pub owner_minus_expected_void_bbox_edge_delta_px: [i64; 4],
    pub owner_minus_expected_void_centroid_delta_milli_px: [i64; 2],
    pub owner_region_pixel_count: u64,
    pub owner_expected_void_overlap_pixel_count: u64,
    pub owner_expected_void_overlap_milli: u64,
    pub owner_boundary_adjacency_pixel_count: u64,
    pub owner_boundary_adjacency_milli: u64,
    pub passes_thresholds: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReviewedRegionPartBindingCalibration {
    pub structure_id: String,
    pub expected_region_canonical_sha256: String,
    pub owner_part_id: String,
    pub expected_void_pixel_count: u64,
    pub expected_boundary_pixel_count: u64,
    pub candidates: Vec<ReviewedRegionPartBindingCandidate>,
    pub selected_transform: ReviewedRegionPartBindingTransform,
    pub authored_transform: Option<ReviewedRegionPartBindingTransform>,
    pub promotable: bool,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReviewedRegionPartBindingDiagnostic {
    pub structure_id: String,
    pub expected_region_canonical_sha256: String,
    pub owner_part_id: String,
    pub expected_void_pixel_count: u64,
    pub expected_boundary_pixel_count: u64,
    pub candidates: Vec<ReviewedRegionPartBindingCandidate>,
    pub ranked_transform: ReviewedRegionPartBindingTransform,
    pub ranked_transform_unique: bool,
    pub eligible_transform_count: u64,
}

pub(crate) const STRICT_OWNER_VOID_POLICY: &str =
    "registered-camera-direct-part-id-owner-void-zero-intrusion@1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StrictReviewedRegionPartBindingAssessment {
    pub structure_id: String,
    pub owner_part_id: String,
    pub policy: String,
    pub expected_region_canonical_sha256: String,
    pub expected_void_pixel_count: u64,
    pub expected_boundary_pixel_count: u64,
    pub owner_region_pixel_count: u64,
    pub owner_boundary_adjacency_pixel_count: u64,
    pub owner_boundary_adjacency_milli: u64,
    pub owner_expected_void_overlap_pixel_count: u64,
    pub owner_expected_void_overlap_milli: u64,
    pub status: String,
    pub promotable: bool,
    pub quality_status: String,
    pub depth_status: String,
}

fn reviewed_region_expected_void_mask_with_rotation(
    structure: &Value,
    target_mask: &[bool],
    crop: [f64; 4],
    rotation_degrees: f64,
    structure_id: &str,
) -> Result<(String, Vec<bool>, Vec<bool>), RuntimeError> {
    if target_mask.len() != 512 * 512 {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: target mask must be 512x512",
        ));
    }
    let [crop_x, crop_y, crop_width, crop_height] = crop;
    if !crop.iter().all(|value| value.is_finite())
        || crop_x < 0.0
        || crop_y < 0.0
        || crop_width <= 0.0
        || crop_height <= 0.0
        || crop_x + crop_width > 1.0 + f64::EPSILON
        || crop_y + crop_height > 1.0 + f64::EPSILON
    {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: canonical crop mapping is invalid",
        ));
    }
    if structure.get("review_status").and_then(Value::as_str) != Some("user_confirmed") {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: visual structure is not reviewed",
        ));
    }
    if !matches!(
        structure_id,
        "left.open-stock-void"
            | "right.open-stock-void"
            | "rear3q.open-stock-void"
            | "left.trigger-void"
            | "right.trigger-void"
            | "rear3q.trigger-void"
    ) {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: structure id is not an exact reviewed negative-space id",
        ));
    }
    let regions = structure
        .get("regions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid("PART_BINDING_CALIBRATION_BLOCKED: reviewed regions are unavailable")
        })?;
    let matches = regions
        .iter()
        .filter(|region| {
            region.get("structure_id").and_then(Value::as_str) == Some(structure_id)
                && region.get("mask_operation").and_then(Value::as_str) == Some("subtract")
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: exact reviewed subtract region is not unique",
        ));
    }
    let region = matches[0];
    let region_hash = canonical_json_hash(region);
    let contour = contour_from_value(region.get("contour_points")).ok_or_else(|| {
        invalid("PART_BINDING_CALIBRATION_BLOCKED: reviewed contour is unavailable")
    })?;
    let contour = contour
        .into_iter()
        .map(|point| {
            project_point_to_view(
                point,
                crop,
                rotation_degrees,
                "reviewed binding contour point",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let region_mask = super::rasterize_contour(&contour);
    let expected_void = region_mask
        .iter()
        .zip(target_mask.iter())
        .map(|(region, target)| *region && !*target)
        .collect::<Vec<_>>();
    if !expected_void.iter().any(|pixel| *pixel) {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: reviewed expected void is empty",
        ));
    }
    Ok((region_hash, region_mask, expected_void))
}

fn reviewed_region_expected_void_mask(
    structure: &Value,
    target_mask: &[bool],
    crop: [f64; 4],
    structure_id: &str,
) -> Result<(String, Vec<bool>, Vec<bool>), RuntimeError> {
    reviewed_region_expected_void_mask_with_rotation(
        structure,
        target_mask,
        crop,
        0.0,
        structure_id,
    )
}

fn transform_part_binding_mask(
    source: &[bool],
    transform: ReviewedRegionPartBindingTransform,
) -> Result<Vec<bool>, RuntimeError> {
    if source.len() != 512 * 512 {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: Part-ID mask must be 512x512",
        ));
    }
    let mut result = vec![false; 512 * 512];
    for y in 0..512usize {
        for x in 0..512usize {
            let (source_x, source_y) = match transform {
                ReviewedRegionPartBindingTransform::Identity => (x, y),
                ReviewedRegionPartBindingTransform::HorizontalFlip => (511 - x, y),
                ReviewedRegionPartBindingTransform::VerticalFlip => (x, 511 - y),
                ReviewedRegionPartBindingTransform::Rotate180 => (511 - x, 511 - y),
            };
            result[y * 512 + x] = source[source_y * 512 + source_x];
        }
    }
    Ok(result)
}

fn validate_reviewed_region_part_binding_thresholds(
    thresholds: &ReviewedRegionPartBindingThresholds,
) -> Result<(u64, u64, u64), RuntimeError> {
    let min_owner_region = thresholds.min_owner_region_pixels.ok_or_else(|| {
        invalid("PART_BINDING_CALIBRATION_BLOCKED: owner-region threshold is unavailable")
    })?;
    let min_boundary_adjacency = thresholds.min_boundary_adjacency_pixels.ok_or_else(|| {
        invalid("PART_BINDING_CALIBRATION_BLOCKED: boundary-adjacency threshold is unavailable")
    })?;
    let max_owner_void_overlap = thresholds
        .max_owner_expected_void_overlap_milli
        .ok_or_else(|| {
            invalid("PART_BINDING_CALIBRATION_BLOCKED: owner-void threshold is unavailable")
        })?;
    if min_owner_region > 512 * 512
        || min_boundary_adjacency > 512 * 512
        || max_owner_void_overlap > 1000
    {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: binding thresholds are out of bounds",
        ));
    }
    Ok((
        min_owner_region,
        min_boundary_adjacency,
        max_owner_void_overlap,
    ))
}

fn reviewed_region_part_binding_score_cmp(
    left: &ReviewedRegionPartBindingCandidate,
    right: &ReviewedRegionPartBindingCandidate,
) -> std::cmp::Ordering {
    left.owner_boundary_adjacency_pixel_count
        .cmp(&right.owner_boundary_adjacency_pixel_count)
        .then_with(|| {
            left.owner_region_pixel_count
                .cmp(&right.owner_region_pixel_count)
        })
        .then_with(|| {
            right
                .owner_expected_void_overlap_pixel_count
                .cmp(&left.owner_expected_void_overlap_pixel_count)
        })
        .then_with(|| right.bbox_edge_error_px.cmp(&left.bbox_edge_error_px))
        .then_with(|| right.centroid_error_px.cmp(&left.centroid_error_px))
}

/// Hash-only mask encoding used by the 04Y registration preflight.  The
/// payload is a domain tag followed by row-major foreground bytes; no source
/// image or decoded AOV bytes are included in a receipt.
pub(crate) const REGISTRATION_PREFLIGHT_MASK_ENCODING: &str =
    "forgecad-registration-preflight-mask-v1|512x512|row-major-u8|foreground=true";

pub(crate) fn registration_preflight_mask_sha256(mask: &[bool]) -> Result<String, RuntimeError> {
    if mask.len() != 512 * 512 {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: mask must be 512x512",
        ));
    }
    let mut bytes = Vec::with_capacity(REGISTRATION_PREFLIGHT_MASK_ENCODING.len() + 1 + mask.len());
    bytes.extend_from_slice(REGISTRATION_PREFLIGHT_MASK_ENCODING.as_bytes());
    bytes.push(0);
    bytes.extend(mask.iter().map(|pixel| u8::from(*pixel)));
    Ok(sha256_hex(&bytes))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct RegistrationPreflightBinding {
    pub view_kind: String,
    pub structure_id: String,
    pub crop_canonical_sha256: String,
    pub target_object_sha256: String,
    pub target_mask_source_sha256: String,
    pub part_id_pass_sha256: String,
    pub registered_camera_hash: String,
    pub worker_binding_canonical_sha256: String,
    pub lineage_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct RegistrationPreflightProjection {
    pub binding: RegistrationPreflightBinding,
    pub owner_part_id: String,
    pub rotation_degrees_milli: i64,
    pub expected_region_canonical_sha256: String,
    pub projected_target_mask_sha256: String,
    pub projected_region_mask_sha256: String,
    pub expected_void_mask_sha256: String,
    pub owner_mask_sha256: String,
    pub projected_region_bbox_px: [u64; 4],
    pub projected_region_centroid_milli_px: [i64; 2],
    pub expected_void_bbox_px: [u64; 4],
    pub expected_void_centroid_milli_px: [i64; 2],
    pub owner_bbox_px: [u64; 4],
    pub owner_centroid_milli_px: [i64; 2],
    pub owner_minus_expected_void_bbox_edge_delta_px: [i64; 4],
    pub owner_minus_expected_void_centroid_delta_milli_px: [i64; 2],
    pub expected_void_pixel_count: u64,
    pub expected_boundary_pixel_count: u64,
    pub owner_pixel_count: u64,
    pub owner_region_pixel_count: u64,
    pub identity_overlap_iou_milli: u64,
    pub identity_boundary_f1_milli: u64,
    pub identity_bbox_edge_error_px: u64,
    pub identity_centroid_error_px: u64,
    pub identity_owner_expected_void_overlap_pixel_count: u64,
    pub identity_owner_expected_void_overlap_milli: u64,
    pub identity_owner_boundary_adjacency_pixel_count: u64,
    pub identity_owner_boundary_adjacency_milli: u64,
    /// Registration-only eligibility: identity is the unique winner across
    /// the closed four-transform ranking. This ignores formal owner-zero and
    /// quality thresholds by design.
    pub registration_identity_eligible: bool,
    /// Independent strict owner gate; false here does not invalidate the
    /// registration observation and never implies visual/quality approval.
    pub formal_owner_gate_eligible: bool,
    pub strict_owner_zero_intrusion: bool,
    pub unique_ranked_registration_identity: bool,
    pub formal_eligible_transform_count: u64,
    pub registration_rank_tie: bool,
    pub promotable: bool,
    pub quality_status: String,
    pub depth_status: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct RegistrationPreflightGate {
    pub current_view_count: u64,
    pub current_unique_registration_identity_all_views: bool,
    pub formal_owner_gate_all_views: bool,
    pub negative_fixture_improves: bool,
    pub negative_fixture_tie: bool,
    pub lineage_stable: bool,
    pub left_right_projection_hashes_stable: bool,
    /// Registration-only result; never a quality or stage advancement gate.
    pub pass: bool,
    pub promotable: bool,
    pub quality_status: String,
    pub status: String,
}

fn validate_registration_preflight_binding(
    binding: &RegistrationPreflightBinding,
) -> Result<(), RuntimeError> {
    if !matches!(
        binding.view_kind.as_str(),
        "left" | "right" | "rear-three-quarter"
    ) {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: unexpected view kind",
        ));
    }
    let expected_structure = match binding.view_kind.as_str() {
        "left" => "left.open-stock-void",
        "right" => "right.open-stock-void",
        "rear-three-quarter" => "rear3q.open-stock-void",
        _ => unreachable!(),
    };
    if binding.structure_id != expected_structure || binding.target_object_sha256.is_empty() {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: view/structure/target binding differs",
        ));
    }
    for (name, hash) in [
        ("crop", &binding.crop_canonical_sha256),
        ("target object", &binding.target_object_sha256),
        ("target mask source", &binding.target_mask_source_sha256),
        ("Part-ID pass", &binding.part_id_pass_sha256),
        ("registered camera", &binding.registered_camera_hash),
        ("worker binding", &binding.worker_binding_canonical_sha256),
        ("lineage", &binding.lineage_sha256),
    ] {
        if !is_sha256(hash) {
            return Err(invalid(format!(
                "REGISTRATION_PREFLIGHT_BLOCKED: {name} binding is not a SHA-256",
            )));
        }
    }
    Ok(())
}

/// Compute one hash-bound, owner/contour-only registration projection. The
/// target mask is already projected into the authored view; contour rotation
/// is applied by the existing reviewed-region projector. Registration ranks
/// every member of the closed four-transform set; formal owner-zero/quality
/// eligibility is reported independently and never promotes a candidate.
/// This function has no Runtime handle and cannot write CAS/SQLite or create a
/// candidate.
pub(crate) fn registration_preflight_projection_with_rotation(
    binding: RegistrationPreflightBinding,
    structure: &Value,
    target_mask: &[bool],
    owner_mask: &[bool],
    crop: [f64; 4],
    rotation_degrees: f64,
    thresholds: &ReviewedRegionPartBindingThresholds,
) -> Result<RegistrationPreflightProjection, RuntimeError> {
    validate_registration_preflight_binding(&binding)?;
    if !rotation_degrees.is_finite() || !(-180.0..=180.0).contains(&rotation_degrees) {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: rotation is out of range",
        ));
    }
    if target_mask.len() != 512 * 512 || owner_mask.len() != 512 * 512 {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: masks must be 512x512",
        ));
    }
    let thresholds = validate_reviewed_region_part_binding_thresholds(thresholds)?;
    let structure_id = binding.structure_id.as_str();
    let (region_hash, region_mask, expected_void) =
        reviewed_region_expected_void_mask_with_rotation(
            structure,
            target_mask,
            crop,
            rotation_degrees,
            structure_id,
        )?;
    let projected_region_bbox_px = mask_bbox_and_centroid(&region_mask)
        .ok_or_else(|| invalid("REGISTRATION_PREFLIGHT_BLOCKED: projected region is empty"))?;
    let projected_region_centroid_milli_px =
        mask_centroid_milli(&region_mask).ok_or_else(|| {
            invalid("REGISTRATION_PREFLIGHT_BLOCKED: projected region centroid is unavailable")
        })?;
    let expected_void_bbox_px = mask_bbox_and_centroid(&expected_void)
        .ok_or_else(|| invalid("REGISTRATION_PREFLIGHT_BLOCKED: expected void is empty"))?;
    let expected_void_centroid_milli_px = mask_centroid_milli(&expected_void).ok_or_else(|| {
        invalid("REGISTRATION_PREFLIGHT_BLOCKED: expected void centroid is unavailable")
    })?;
    let owner_bbox_px = mask_bbox_and_centroid(owner_mask)
        .ok_or_else(|| invalid("REGISTRATION_PREFLIGHT_BLOCKED: owner mask is empty"))?;
    let owner_centroid_milli_px = mask_centroid_milli(owner_mask)
        .ok_or_else(|| invalid("REGISTRATION_PREFLIGHT_BLOCKED: owner centroid is unavailable"))?;
    let transforms = [
        ReviewedRegionPartBindingTransform::Identity,
        ReviewedRegionPartBindingTransform::HorizontalFlip,
        ReviewedRegionPartBindingTransform::VerticalFlip,
        ReviewedRegionPartBindingTransform::Rotate180,
    ];
    let candidates = transforms
        .into_iter()
        .map(|transform| {
            let transformed = transform_part_binding_mask(owner_mask, transform)?;
            reviewed_region_part_binding_candidate_metrics(
                &region_mask,
                &expected_void,
                &transformed,
                transform,
                thresholds,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let identity = candidates
        .iter()
        .find(|candidate| candidate.transform == ReviewedRegionPartBindingTransform::Identity)
        .expect("identity transform is part of the closed transform set");
    let best = candidates
        .iter()
        .max_by(|left, right| reviewed_region_part_binding_score_cmp(left, right))
        .expect("closed four-transform ranking is non-empty");
    let ties = candidates
        .iter()
        .filter(|candidate| {
            reviewed_region_part_binding_score_cmp(candidate, best) == std::cmp::Ordering::Equal
        })
        .count();
    let registration_rank_tie = ties != 1;
    let unique_ranked_registration_identity = !registration_rank_tie;
    let registration_identity_eligible = unique_ranked_registration_identity
        && best.transform == ReviewedRegionPartBindingTransform::Identity;
    let strict_owner_zero_intrusion = identity.owner_expected_void_overlap_pixel_count == 0
        && identity.owner_expected_void_overlap_milli == 0;
    let formal_owner_gate_eligible = identity.passes_thresholds && strict_owner_zero_intrusion;
    let status = if registration_rank_tie {
        "REGISTRATION_RANK_TIE"
    } else if registration_identity_eligible {
        "REGISTRATION_IDENTITY_UNIQUE"
    } else {
        "REGISTRATION_NON_IDENTITY_UNIQUE"
    }
    .to_owned();
    let rotation_degrees_milli = (rotation_degrees * 1000.0).round() as i64;
    Ok(RegistrationPreflightProjection {
        binding,
        owner_part_id: "rear-stock".to_owned(),
        rotation_degrees_milli,
        expected_region_canonical_sha256: region_hash,
        projected_target_mask_sha256: registration_preflight_mask_sha256(target_mask)?,
        projected_region_mask_sha256: registration_preflight_mask_sha256(&region_mask)?,
        expected_void_mask_sha256: registration_preflight_mask_sha256(&expected_void)?,
        owner_mask_sha256: registration_preflight_mask_sha256(owner_mask)?,
        projected_region_bbox_px: projected_region_bbox_px.0,
        projected_region_centroid_milli_px,
        expected_void_bbox_px: expected_void_bbox_px.0,
        expected_void_centroid_milli_px,
        owner_bbox_px: owner_bbox_px.0,
        owner_centroid_milli_px,
        owner_minus_expected_void_bbox_edge_delta_px: identity
            .owner_minus_expected_void_bbox_edge_delta_px,
        owner_minus_expected_void_centroid_delta_milli_px: identity
            .owner_minus_expected_void_centroid_delta_milli_px,
        expected_void_pixel_count: expected_void.iter().filter(|pixel| **pixel).count() as u64,
        expected_boundary_pixel_count: boundary_mask(&expected_void)
            .iter()
            .filter(|pixel| **pixel)
            .count() as u64,
        owner_pixel_count: owner_mask.iter().filter(|pixel| **pixel).count() as u64,
        owner_region_pixel_count: identity.owner_region_pixel_count,
        identity_overlap_iou_milli: identity.overlap_iou_milli,
        identity_boundary_f1_milli: identity.boundary_f1_milli,
        identity_bbox_edge_error_px: identity.bbox_edge_error_px,
        identity_centroid_error_px: identity.centroid_error_px,
        identity_owner_expected_void_overlap_pixel_count: identity
            .owner_expected_void_overlap_pixel_count,
        identity_owner_expected_void_overlap_milli: identity.owner_expected_void_overlap_milli,
        identity_owner_boundary_adjacency_pixel_count: identity
            .owner_boundary_adjacency_pixel_count,
        identity_owner_boundary_adjacency_milli: identity.owner_boundary_adjacency_milli,
        registration_identity_eligible,
        formal_owner_gate_eligible,
        strict_owner_zero_intrusion,
        unique_ranked_registration_identity,
        formal_eligible_transform_count: candidates
            .iter()
            .filter(|candidate| {
                candidate.passes_thresholds
                    && candidate.owner_expected_void_overlap_pixel_count == 0
                    && candidate.owner_expected_void_overlap_milli == 0
            })
            .count() as u64,
        registration_rank_tie,
        promotable: false,
        quality_status: "NOT_PROVEN".to_owned(),
        depth_status: "UNKNOWN".to_owned(),
        status,
    })
}

fn registration_preflight_identity_score_cmp(
    left: &RegistrationPreflightProjection,
    right: &RegistrationPreflightProjection,
) -> std::cmp::Ordering {
    left.identity_owner_boundary_adjacency_pixel_count
        .cmp(&right.identity_owner_boundary_adjacency_pixel_count)
        .then_with(|| {
            left.owner_region_pixel_count
                .cmp(&right.owner_region_pixel_count)
        })
        .then_with(|| {
            right
                .identity_owner_expected_void_overlap_pixel_count
                .cmp(&left.identity_owner_expected_void_overlap_pixel_count)
        })
        .then_with(|| {
            right
                .identity_bbox_edge_error_px
                .cmp(&left.identity_bbox_edge_error_px)
        })
        .then_with(|| {
            right
                .identity_centroid_error_px
                .cmp(&left.identity_centroid_error_px)
        })
}

fn registration_preflight_view_map<'a>(
    views: &'a [RegistrationPreflightProjection],
) -> Result<BTreeMap<&'a str, &'a RegistrationPreflightProjection>, RuntimeError> {
    let mut map = BTreeMap::new();
    for view in views {
        validate_registration_preflight_binding(&view.binding)?;
        if map.insert(view.binding.view_kind.as_str(), view).is_some() {
            return Err(invalid(
                "REGISTRATION_PREFLIGHT_BLOCKED: duplicate view projection",
            ));
        }
    }
    for kind in ["left", "right", "rear-three-quarter"] {
        if !map.contains_key(kind) {
            return Err(invalid(
                "REGISTRATION_PREFLIGHT_BLOCKED: closed three-view set is incomplete",
            ));
        }
    }
    Ok(map)
}

/// Root gate for the closed 04Y three-view screen. It accepts only the
/// current L/R@0 and rear3q@180 projections when all three are uniquely
/// registration-identity eligible and the negative rear3q@0 fixture does not
/// improve the same score. Formal owner/quality eligibility is deliberately
/// outside this registration gate. Any hash/lineage drift or rank tie is an
/// error, never a pass.
pub(crate) fn registration_preflight_gate(
    current: &[RegistrationPreflightProjection],
    negative_fixture: &[RegistrationPreflightProjection],
) -> Result<RegistrationPreflightGate, RuntimeError> {
    let current_map = registration_preflight_view_map(current)?;
    let negative_map = registration_preflight_view_map(negative_fixture)?;
    let mut lineage_stable = true;
    let mut left_right_projection_hashes_stable = true;
    for kind in ["left", "right", "rear-three-quarter"] {
        let current_view = current_map[kind];
        let negative_view = negative_map[kind];
        lineage_stable &= current_view.binding == negative_view.binding;
        if kind == "left" || kind == "right" {
            left_right_projection_hashes_stable &= current_view == negative_view;
        }
    }
    if !lineage_stable {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: current/negative lineage or hash binding drifted",
        ));
    }
    if !left_right_projection_hashes_stable {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: unchanged L/R projection hashes drifted",
        ));
    }
    let current_rotation_ok = current_map["left"].rotation_degrees_milli == 0
        && current_map["right"].rotation_degrees_milli == 0
        && current_map["rear-three-quarter"].rotation_degrees_milli == 180_000;
    let negative_rotation_ok = negative_map
        .values()
        .all(|view| view.rotation_degrees_milli == 0);
    if !current_rotation_ok || !negative_rotation_ok {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: current/negative rotation fixture differs",
        ));
    }
    if current_map
        .values()
        .chain(negative_map.values())
        .any(|view| view.registration_rank_tie)
    {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: registration identity ranking tie",
        ));
    }
    let current_unique_registration_identity_all_views = current_map.values().all(|view| {
        view.registration_identity_eligible
            && view.unique_ranked_registration_identity
            && !view.registration_rank_tie
    });
    let formal_owner_gate_all_views = current_map
        .values()
        .all(|view| view.formal_owner_gate_eligible);
    let current_rear = current_map["rear-three-quarter"];
    let negative_rear = negative_map["rear-three-quarter"];
    let negative_score_order =
        registration_preflight_identity_score_cmp(negative_rear, current_rear);
    let negative_fixture_tie = negative_score_order == std::cmp::Ordering::Equal;
    if negative_fixture_tie {
        return Err(invalid(
            "REGISTRATION_PREFLIGHT_BLOCKED: negative fixture ties current identity score",
        ));
    }
    let negative_fixture_improves = negative_score_order == std::cmp::Ordering::Greater;
    let pass = current_unique_registration_identity_all_views
        && !negative_fixture_improves
        && lineage_stable
        && left_right_projection_hashes_stable;
    Ok(RegistrationPreflightGate {
        current_view_count: current.len() as u64,
        current_unique_registration_identity_all_views,
        formal_owner_gate_all_views,
        negative_fixture_improves,
        negative_fixture_tie,
        lineage_stable,
        left_right_projection_hashes_stable,
        pass,
        promotable: false,
        quality_status: "NOT_PROVEN".to_owned(),
        status: if pass {
            "CURRENT_UNIQUE_REGISTRATION_ELIGIBLE".to_owned()
        } else if negative_fixture_improves {
            "BLOCKED_NEGATIVE_FIXTURE_IMPROVES".to_owned()
        } else {
            "BLOCKED_CURRENT_IDENTITY_NOT_UNIQUE_OR_INELIGIBLE".to_owned()
        },
    })
}

fn reviewed_region_part_binding_candidate_metrics(
    region_mask: &[bool],
    expected_void: &[bool],
    owner_mask: &[bool],
    transform: ReviewedRegionPartBindingTransform,
    thresholds: (u64, u64, u64),
) -> Result<ReviewedRegionPartBindingCandidate, RuntimeError> {
    if region_mask.len() != 512 * 512
        || expected_void.len() != 512 * 512
        || owner_mask.len() != 512 * 512
    {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: binding masks must be 512x512",
        ));
    }
    let (overlap_iou_milli, boundary_f1_milli, _, _, nonempty) = metrics(region_mask, owner_mask);
    if !nonempty {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: binding masks are empty",
        ));
    }
    let (expected_bbox, expected_centroid) =
        mask_bbox_and_centroid(region_mask).ok_or_else(|| {
            invalid("PART_BINDING_CALIBRATION_BLOCKED: reviewed region geometry is unavailable")
        })?;
    let (owner_bbox, owner_centroid) = mask_bbox_and_centroid(owner_mask).ok_or_else(|| {
        invalid("PART_BINDING_CALIBRATION_BLOCKED: owner Part geometry is unavailable")
    })?;
    let bbox_edge_error_px = expected_bbox
        .iter()
        .zip(owner_bbox.iter())
        .map(|(expected, owner)| expected.abs_diff(*owner))
        .max()
        .unwrap_or(0);
    let centroid_distance =
        (owner_centroid[0] - expected_centroid[0]).hypot(owner_centroid[1] - expected_centroid[1]);
    if !centroid_distance.is_finite() {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: centroid metric is not finite",
        ));
    }
    let centroid_error_px = centroid_distance.round() as u64;
    let owner_region_pixel_count = owner_mask
        .iter()
        .zip(region_mask.iter())
        .filter(|(owner, region)| **owner && **region)
        .count() as u64;
    let owner_expected_void_overlap_pixel_count = owner_mask
        .iter()
        .zip(expected_void.iter())
        .filter(|(owner, expected)| **owner && **expected)
        .count() as u64;
    let expected_void_pixel_count = expected_void.iter().filter(|pixel| **pixel).count() as u64;
    if expected_void_pixel_count == 0 {
        return Err(invalid(
            "PART_BINDING_CALIBRATION_BLOCKED: expected void geometry is unavailable",
        ));
    }
    let (expected_void_bbox_px, expected_void_centroid) = mask_bbox_and_centroid(expected_void)
        .ok_or_else(|| {
            invalid("PART_BINDING_CALIBRATION_BLOCKED: expected void geometry is unavailable")
        })?;
    let owner_minus_expected_void_bbox_edge_delta_px = [
        owner_bbox[0] as i64 - expected_void_bbox_px[0] as i64,
        owner_bbox[1] as i64 - expected_void_bbox_px[1] as i64,
        owner_bbox[2] as i64 - expected_void_bbox_px[2] as i64,
        owner_bbox[3] as i64 - expected_void_bbox_px[3] as i64,
    ];
    let owner_minus_expected_void_centroid_delta_milli_px = [
        ((owner_centroid[0] - expected_void_centroid[0]) * 1000.0).round() as i64,
        ((owner_centroid[1] - expected_void_centroid[1]) * 1000.0).round() as i64,
    ];
    let owner_expected_void_overlap_milli =
        owner_expected_void_overlap_pixel_count * 1000 / expected_void_pixel_count;
    let expected_boundary = boundary_mask(expected_void);
    let expected_boundary_pixel_count = expected_boundary.iter().filter(|pixel| **pixel).count();
    let mut owner_boundary_adjacency_pixel_count = 0_u64;
    for (index, boundary) in expected_boundary.iter().enumerate() {
        if !*boundary {
            continue;
        }
        let x = (index % 512) as i32;
        let y = (index / 512) as i32;
        let adjacent = (-2..=2).any(|dy| {
            (-2..=2).any(|dx| {
                let xx = x + dx;
                let yy = y + dy;
                xx >= 0
                    && yy >= 0
                    && xx < 512
                    && yy < 512
                    && owner_mask[yy as usize * 512 + xx as usize]
            })
        });
        if adjacent {
            owner_boundary_adjacency_pixel_count += 1;
        }
    }
    let owner_boundary_adjacency_milli = if expected_boundary_pixel_count == 0 {
        0
    } else {
        owner_boundary_adjacency_pixel_count * 1000 / expected_boundary_pixel_count as u64
    };
    let (min_owner_region, min_boundary_adjacency, max_owner_void_overlap) = thresholds;
    Ok(ReviewedRegionPartBindingCandidate {
        transform,
        overlap_iou_milli,
        boundary_f1_milli,
        bbox_edge_error_px,
        centroid_error_px,
        expected_void_bbox_px,
        owner_bbox_px: owner_bbox,
        owner_minus_expected_void_bbox_edge_delta_px,
        owner_minus_expected_void_centroid_delta_milli_px,
        owner_region_pixel_count,
        owner_expected_void_overlap_pixel_count,
        owner_expected_void_overlap_milli,
        owner_boundary_adjacency_pixel_count,
        owner_boundary_adjacency_milli,
        passes_thresholds: owner_region_pixel_count >= min_owner_region
            && owner_boundary_adjacency_pixel_count >= min_boundary_adjacency
            && owner_expected_void_overlap_milli <= max_owner_void_overlap,
    })
}

/// Calibrate one reviewed open-stock region against the exact semantic
/// `rear-stock` Part-ID mask.  This is a pure read-only diagnostic: it takes
/// no Runtime handle, never writes FormArt, and never edits the reviewed
/// contour, target mask, or any depth evidence.
pub(crate) fn diagnose_reviewed_region_part_binding_with_rotation(
    structure: &Value,
    target_mask: &[bool],
    part_png: &[u8],
    part_ids: &[String],
    crop: [f64; 4],
    rotation_degrees: f64,
    structure_id: &str,
    thresholds: &ReviewedRegionPartBindingThresholds,
) -> Result<ReviewedRegionPartBindingDiagnostic, RuntimeError> {
    let thresholds = validate_reviewed_region_part_binding_thresholds(thresholds)?;
    let owner_part_id = "rear-stock";
    let (region_hash, region_mask, expected_void) =
        reviewed_region_expected_void_mask_with_rotation(
            structure,
            target_mask,
            crop,
            rotation_degrees,
            structure_id,
        )?;
    let owner_mask = exact_part_id_mask(part_png, part_ids, owner_part_id)?;
    let transforms = [
        ReviewedRegionPartBindingTransform::Identity,
        ReviewedRegionPartBindingTransform::HorizontalFlip,
        ReviewedRegionPartBindingTransform::VerticalFlip,
        ReviewedRegionPartBindingTransform::Rotate180,
    ];
    let candidates = transforms
        .into_iter()
        .map(|transform| {
            let transformed = transform_part_binding_mask(&owner_mask, transform)?;
            reviewed_region_part_binding_candidate_metrics(
                &region_mask,
                &expected_void,
                &transformed,
                transform,
                thresholds,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ranked = candidates
        .iter()
        .max_by(|left, right| reviewed_region_part_binding_score_cmp(left, right))
        .expect("closed transform set is non-empty");
    let ranked_tie_count = candidates
        .iter()
        .filter(|candidate| {
            reviewed_region_part_binding_score_cmp(candidate, ranked) == std::cmp::Ordering::Equal
        })
        .count();
    let ranked_transform = ranked.transform;
    let eligible_transform_count = candidates
        .iter()
        .filter(|candidate| candidate.passes_thresholds)
        .count() as u64;
    Ok(ReviewedRegionPartBindingDiagnostic {
        structure_id: structure_id.to_owned(),
        expected_region_canonical_sha256: region_hash,
        owner_part_id: owner_part_id.to_owned(),
        expected_void_pixel_count: expected_void.iter().filter(|pixel| **pixel).count() as u64,
        expected_boundary_pixel_count: boundary_mask(&expected_void)
            .iter()
            .filter(|pixel| **pixel)
            .count() as u64,
        candidates,
        ranked_transform,
        ranked_transform_unique: ranked_tie_count == 1,
        eligible_transform_count,
    })
}

fn calibrate_reviewed_region_part_binding_diagnostic(
    diagnostic: ReviewedRegionPartBindingDiagnostic,
    authored_transform: Option<ReviewedRegionPartBindingTransform>,
) -> Result<ReviewedRegionPartBindingCalibration, RuntimeError> {
    let eligible = diagnostic
        .candidates
        .iter()
        .filter(|candidate| candidate.passes_thresholds)
        .collect::<Vec<_>>();
    let selected_transform = {
        let best = eligible
            .iter()
            .copied()
            .max_by(|left, right| reviewed_region_part_binding_score_cmp(left, right))
            .ok_or_else(|| {
                invalid("PART_BINDING_CALIBRATION_BLOCKED: no transform passes all thresholds")
            })?;
        let ties = eligible
            .iter()
            .filter(|candidate| {
                reviewed_region_part_binding_score_cmp(candidate, best) == std::cmp::Ordering::Equal
            })
            .count();
        if ties != 1 {
            return Err(invalid(
                "PART_BINDING_CALIBRATION_BLOCKED: transform winner is not unique",
            ));
        }
        if let Some(authored) = authored_transform {
            if authored != best.transform {
                return Err(invalid(
                    "PART_BINDING_CALIBRATION_BLOCKED: authored transform differs from unique winner",
                ));
            }
        }
        best.transform
    };
    Ok(ReviewedRegionPartBindingCalibration {
        structure_id: diagnostic.structure_id,
        expected_region_canonical_sha256: diagnostic.expected_region_canonical_sha256,
        owner_part_id: diagnostic.owner_part_id,
        expected_void_pixel_count: diagnostic.expected_void_pixel_count,
        expected_boundary_pixel_count: diagnostic.expected_boundary_pixel_count,
        candidates: diagnostic.candidates,
        selected_transform,
        authored_transform,
        promotable: false,
        status: if authored_transform.is_some() {
            "bound-diagnostic-only".to_owned()
        } else {
            "ephemeral-transform-candidate".to_owned()
        },
    })
}

pub(crate) fn calibrate_reviewed_region_part_binding_with_rotation(
    structure: &Value,
    target_mask: &[bool],
    part_png: &[u8],
    part_ids: &[String],
    crop: [f64; 4],
    rotation_degrees: f64,
    structure_id: &str,
    authored_transform: Option<ReviewedRegionPartBindingTransform>,
    thresholds: &ReviewedRegionPartBindingThresholds,
) -> Result<ReviewedRegionPartBindingCalibration, RuntimeError> {
    let diagnostic = diagnose_reviewed_region_part_binding_with_rotation(
        structure,
        target_mask,
        part_png,
        part_ids,
        crop,
        rotation_degrees,
        structure_id,
        thresholds,
    )?;
    calibrate_reviewed_region_part_binding_diagnostic(diagnostic, authored_transform)
}

pub(crate) fn calibrate_reviewed_region_part_binding(
    structure: &Value,
    target_mask: &[bool],
    part_png: &[u8],
    part_ids: &[String],
    crop: [f64; 4],
    structure_id: &str,
    authored_transform: Option<ReviewedRegionPartBindingTransform>,
    thresholds: &ReviewedRegionPartBindingThresholds,
) -> Result<ReviewedRegionPartBindingCalibration, RuntimeError> {
    calibrate_reviewed_region_part_binding_with_rotation(
        structure,
        target_mask,
        part_png,
        part_ids,
        crop,
        0.0,
        structure_id,
        authored_transform,
        thresholds,
    )
}

/// Calibrate one exact `rear-stock` owner mask against a reviewed open-stock
/// void while validating the candidate-bound depth AOV.  This is intentionally
/// identity-only: the caller must have already bound the Part-ID/depth passes
/// to the approved registered camera.  The loose result ignores void
/// intrusion so it can report useful offsets; the strict result additionally
/// requires the production zero-intrusion thresholds.  Neither result applies
/// an image transform, changes a contour, writes CAS/SQLite, or promotes a
/// candidate.
pub(crate) fn calibrate_depth_aware_reviewed_region_part_binding_with_rotation(
    structure: &Value,
    target_mask: &[bool],
    part_png: &[u8],
    depth_png: &[u8],
    part_ids: &[String],
    crop: [f64; 4],
    rotation_degrees: f64,
    structure_id: &str,
    thresholds: &ReviewedRegionPartBindingThresholds,
) -> Result<DepthAwareReviewedRegionPartBindingCalibration, RuntimeError> {
    let thresholds = validate_reviewed_region_part_binding_thresholds(thresholds)?;
    let loose_thresholds = ReviewedRegionPartBindingThresholds {
        min_owner_region_pixels: Some(thresholds.0),
        min_boundary_adjacency_pixels: Some(thresholds.1),
        max_owner_expected_void_overlap_milli: Some(1000),
    };
    let diagnostic = diagnose_reviewed_region_part_binding_with_rotation(
        structure,
        target_mask,
        part_png,
        part_ids,
        crop,
        rotation_degrees,
        structure_id,
        &loose_thresholds,
    )?;
    let identity_transform_unique = diagnostic.ranked_transform_unique
        && diagnostic.ranked_transform == ReviewedRegionPartBindingTransform::Identity;
    let transform_rank_tie = !diagnostic.ranked_transform_unique;
    let eligible_transform_count = diagnostic.eligible_transform_count;
    let (region_hash, region_mask, expected_void) =
        reviewed_region_expected_void_mask_with_rotation(
            structure,
            target_mask,
            crop,
            rotation_degrees,
            structure_id,
        )?;
    let owner_part_id = "rear-stock";
    let owner_mask = exact_part_id_mask(part_png, part_ids, owner_part_id)?;
    let depth_validation = validate_depth_aov_owner_pixels(depth_png, &owner_mask)?;
    let expected_void_pixel_count = expected_void.iter().filter(|pixel| **pixel).count() as u64;
    let expected_boundary = boundary_mask(&expected_void);
    let expected_boundary_pixel_count =
        expected_boundary.iter().filter(|pixel| **pixel).count() as u64;
    let owner_region_pixel_count = owner_mask
        .iter()
        .zip(region_mask.iter())
        .filter(|(owner, region)| **owner && **region)
        .count() as u64;
    let owner_region_mask = owner_mask
        .iter()
        .zip(region_mask.iter())
        .map(|(owner, region)| *owner && *region)
        .collect::<Vec<_>>();
    let owner_expected_void_overlap_pixel_count = owner_mask
        .iter()
        .zip(expected_void.iter())
        .filter(|(owner, expected)| **owner && **expected)
        .count() as u64;
    let owner_expected_void_overlap_milli = if expected_void_pixel_count == 0 {
        0
    } else {
        owner_expected_void_overlap_pixel_count * 1000 / expected_void_pixel_count
    };
    let mut owner_boundary_adjacency_pixel_count = 0_u64;
    for (index, boundary) in expected_boundary.iter().enumerate() {
        if !*boundary {
            continue;
        }
        let x = (index % 512) as i32;
        let y = (index / 512) as i32;
        let adjacent = (-2..=2).any(|dy| {
            (-2..=2).any(|dx| {
                let xx = x + dx;
                let yy = y + dy;
                xx >= 0
                    && yy >= 0
                    && xx < 512
                    && yy < 512
                    && owner_mask[yy as usize * 512 + xx as usize]
            })
        });
        if adjacent {
            owner_boundary_adjacency_pixel_count += 1;
        }
    }
    let owner_boundary_adjacency_milli = if expected_boundary_pixel_count == 0 {
        0
    } else {
        owner_boundary_adjacency_pixel_count * 1000 / expected_boundary_pixel_count
    };
    let depth_image = decode_image(depth_png, "depth")?;
    let mut boundary_depth_sample_count = 0_u64;
    let mut boundary_depth_code_sum = 0_u64;
    for (index, owner) in owner_mask.iter().enumerate() {
        if !*owner {
            continue;
        }
        let x = (index % 512) as i32;
        let y = (index / 512) as i32;
        let supports_boundary = (-2..=2).any(|dy| {
            (-2..=2).any(|dx| {
                let xx = x + dx;
                let yy = y + dy;
                xx >= 0
                    && yy >= 0
                    && xx < 512
                    && yy < 512
                    && expected_boundary[yy as usize * 512 + xx as usize]
            })
        });
        if supports_boundary {
            boundary_depth_sample_count += 1;
            boundary_depth_code_sum += u64::from(depth_image.get_pixel(x as u32, y as u32).0[0]);
        }
    }
    let boundary_depth_code_mean_milli = if boundary_depth_sample_count == 0 {
        0
    } else {
        boundary_depth_code_sum * 1000 / boundary_depth_sample_count
    };
    let depth_ordering_milli =
        boundary_depth_code_mean_milli as i64 - depth_validation.owner_depth_code_mean_milli as i64;
    let (expected_void_bbox_px, expected_void_centroid) = mask_bbox_and_centroid(&expected_void)
        .ok_or_else(|| {
            invalid("DEPTH_AWARE_OWNER_VOID_BLOCKED: expected void geometry is unavailable")
        })?;
    let (owner_bbox_px, owner_centroid) = mask_bbox_and_centroid(&owner_mask).ok_or_else(|| {
        invalid("DEPTH_AWARE_OWNER_VOID_BLOCKED: owner Part geometry is unavailable")
    })?;
    let owner_minus_expected_void_bbox_edge_delta_px = [
        owner_bbox_px[0] as i64 - expected_void_bbox_px[0] as i64,
        owner_bbox_px[1] as i64 - expected_void_bbox_px[1] as i64,
        owner_bbox_px[2] as i64 - expected_void_bbox_px[2] as i64,
        owner_bbox_px[3] as i64 - expected_void_bbox_px[3] as i64,
    ];
    let owner_minus_expected_void_centroid_delta_milli_px = [
        ((owner_centroid[0] - expected_void_centroid[0]) * 1000.0).round() as i64,
        ((owner_centroid[1] - expected_void_centroid[1]) * 1000.0).round() as i64,
    ];
    // Loose identity calibration answers only whether the exact semantic
    // owner is sufficiently present and touches the reviewed boundary.  It
    // deliberately does not turn a non-zero intrusion into a pass.
    let loose_identity_calibrated = depth_validation.owner_depth_valid_pixel_count
        == depth_validation.owner_pixel_count
        && identity_transform_unique
        && owner_region_pixel_count >= thresholds.0
        && owner_boundary_adjacency_pixel_count >= thresholds.1
        && boundary_depth_sample_count >= thresholds.1;
    // Strict zero-intrusion remains an independent hard gate.  The pixel
    // overlap check is exact even when the milli ratio rounds to zero.
    let strict_zero_intrusion = loose_identity_calibrated
        && expected_void_pixel_count >= STRICT_OWNER_VOID_MIN_EXPECTED_VOID_PIXELS
        && expected_boundary_pixel_count >= STRICT_OWNER_VOID_MIN_EXPECTED_BOUNDARY_PIXELS
        && owner_region_pixel_count >= STRICT_OWNER_VOID_MIN_OWNER_REGION_PIXELS
        && owner_boundary_adjacency_pixel_count >= STRICT_OWNER_VOID_MIN_BOUNDARY_ADJACENCY_PIXELS
        && owner_boundary_adjacency_milli >= STRICT_OWNER_VOID_MIN_BOUNDARY_ADJACENCY_MILLI
        && owner_expected_void_overlap_pixel_count == 0
        && owner_expected_void_overlap_milli == 0
        && owner_expected_void_overlap_milli <= thresholds.2;
    let status = if strict_zero_intrusion {
        "STRICT_ZERO_INTRUSION_METRIC_ELIGIBLE"
    } else if loose_identity_calibrated {
        "LOOSE_IDENTITY_CALIBRATED"
    } else {
        "LOOSE_IDENTITY_NOT_ELIGIBLE"
    };
    Ok(DepthAwareReviewedRegionPartBindingCalibration {
        structure_id: structure_id.to_owned(),
        expected_region_canonical_sha256: region_hash,
        owner_part_id: owner_part_id.to_owned(),
        policy: DEPTH_AWARE_OWNER_VOID_POLICY.to_owned(),
        depth_encoding: DEPTH_AWARE_OWNER_VOID_DEPTH_ENCODING.to_owned(),
        expected_void_mask_sha256: registration_preflight_mask_sha256(&expected_void)?,
        owner_mask_sha256: registration_preflight_mask_sha256(&owner_mask)?,
        owner_region_mask_sha256: registration_preflight_mask_sha256(&owner_region_mask)?,
        expected_boundary_mask_sha256: registration_preflight_mask_sha256(&expected_boundary)?,
        depth_png_sha256: depth_validation.depth_png_sha256.clone(),
        expected_void_bbox_px,
        owner_bbox_px,
        owner_minus_expected_void_bbox_edge_delta_px,
        owner_minus_expected_void_centroid_delta_milli_px,
        expected_void_pixel_count,
        expected_boundary_pixel_count,
        owner_pixel_count: depth_validation.owner_pixel_count,
        owner_depth_valid_pixel_count: depth_validation.owner_depth_valid_pixel_count,
        owner_depth_code_min: depth_validation.owner_depth_code_min,
        owner_depth_code_max: depth_validation.owner_depth_code_max,
        owner_depth_code_mean_milli: depth_validation.owner_depth_code_mean_milli,
        depth_boundary_sample_count: boundary_depth_sample_count,
        depth_ordering_milli,
        owner_region_pixel_count,
        owner_expected_void_overlap_pixel_count,
        owner_expected_void_overlap_milli,
        owner_boundary_adjacency_pixel_count,
        owner_boundary_adjacency_milli,
        identity_transform_unique,
        eligible_transform_count,
        transform_rank_tie,
        loose_identity_calibrated,
        strict_zero_intrusion,
        depth_status: "OWNER_DEPTH_VALIDATED".to_owned(),
        quality_status: "NOT_PROVEN".to_owned(),
        promotable: false,
        status: status.to_owned(),
    })
}

pub(crate) fn calibrate_depth_aware_reviewed_region_part_binding(
    structure: &Value,
    target_mask: &[bool],
    part_png: &[u8],
    depth_png: &[u8],
    part_ids: &[String],
    crop: [f64; 4],
    structure_id: &str,
    thresholds: &ReviewedRegionPartBindingThresholds,
) -> Result<DepthAwareReviewedRegionPartBindingCalibration, RuntimeError> {
    calibrate_depth_aware_reviewed_region_part_binding_with_rotation(
        structure,
        target_mask,
        part_png,
        depth_png,
        part_ids,
        crop,
        0.0,
        structure_id,
        thresholds,
    )
}

/// Build the public, zero-write calibration projection from durable Runtime
/// truth.  The caller supplies only identities and hashes; every target,
/// camera, RenderSet, Part-ID mask and depth sample is resolved from Store/CAS.
/// `eligible` means the direct registered-camera owner binding is reliable
/// enough to author one bounded repair.  Strict zero-intrusion remains a
/// separate result and is never inferred from calibration eligibility.
pub(crate) fn build_owner_reviewed_void_calibration_projection(
    runtime: &Runtime,
    request: &ProductionWeaponOwnerReviewedVoidCalibrationProjectionGetRequest,
) -> Result<ProductionWeaponOwnerReviewedVoidCalibrationProjection, RuntimeError> {
    let candidate = runtime
        .candidate(&request.candidate_id)?
        .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_CANDIDATE_MISSING"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.candidate_state_sha256
        || candidate.prepared_object_id.as_deref() != Some(request.artifact_id.as_str())
        || candidate.prepared_object_sha256.as_deref() != Some(request.artifact_sha256.as_str())
    {
        return Err(invalid(
            "OWNER_REVIEWED_VOID_CALIBRATION_CANDIDATE_SCOPE_MISMATCH",
        ));
    }

    let readback = runtime.artifact_readback(&request.artifact_sha256, &request.candidate_id)?;
    if readback.get("canonical_sha256").and_then(Value::as_str)
        != Some(request.artifact_readback_sha256.as_str())
    {
        return Err(invalid(
            "OWNER_REVIEWED_VOID_CALIBRATION_ARTIFACT_READBACK_MISMATCH",
        ));
    }
    let part_ids = readback
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_PART_IDS_MISSING"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| is_opaque_id(value))
                .map(str::to_owned)
                .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_PART_ID_INVALID"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if part_ids
        .iter()
        .filter(|part_id| {
            part_id.as_str()
                == PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_OWNER_PART_ID
        })
        .count()
        != 1
        || part_ids.iter().collect::<BTreeSet<_>>().len() != part_ids.len()
    {
        return Err(invalid(
            "OWNER_REVIEWED_VOID_CALIBRATION_OWNER_VOCABULARY_INVALID",
        ));
    }

    let (art, worker_cohorts) = read_persisted_form_art_for_projection(
        runtime,
        &request.form_art_evidence_id,
        &request.project_id,
        &request.candidate_id,
        &request.form_art_evidence_object_sha256,
        &request.form_art_evidence_canonical_sha256,
    )?;
    if art.session_id != request.session_id
        || art.candidate_state_sha256 != request.candidate_state_sha256
        || art.artifact_id != request.artifact_id
        || art.artifact_sha256 != request.artifact_sha256
    {
        return Err(invalid(
            "OWNER_REVIEWED_VOID_CALIBRATION_FORM_ART_SCOPE_MISMATCH",
        ));
    }

    let baseline = runtime
        .store
        .get_production_weapon_form_art_baseline_by_baseline_id(&request.fresh_baseline_id)?
        .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_BASELINE_MISSING"))?;
    if baseline.session_id != request.session_id
        || baseline.project_id != request.project_id
        || baseline.candidate_id != request.candidate_id
        || baseline.candidate_state_sha256 != request.candidate_state_sha256
        || baseline.artifact_id != request.artifact_id
        || baseline.artifact_sha256 != request.artifact_sha256
        || baseline.canonical_sha256 != request.fresh_baseline_canonical_sha256
        || baseline.receipt_object_sha256 != request.fresh_baseline_receipt_object_sha256
        || baseline.registration_lineage_id != request.registration_lineage_id
        || baseline.registration_lineage_canonical_sha256
            != request.registration_lineage_canonical_sha256
        || baseline.registration_lineage_receipt_object_sha256
            != request.registration_lineage_receipt_object_sha256
        || baseline.registered_rig_v2_id != request.registered_rig_v2_id
        || baseline.registered_rig_v2_object_sha256 != request.registered_rig_v2_object_sha256
        || baseline.registered_rig_v2_canonical_sha256 != request.registered_rig_v2_canonical_sha256
        || !baseline.worker_cohort_verified
        || baseline.materialization_status
            != forgecad_contracts::PRODUCTION_WEAPON_FORM_ART_BASELINE_MATERIALIZATION_STATUS
    {
        return Err(invalid(
            "OWNER_REVIEWED_VOID_CALIBRATION_BASELINE_SCOPE_MISMATCH",
        ));
    }

    let lineage = runtime
        .store
        .get_production_camera_lock_registration_lineage(&request.registration_lineage_id)?
        .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_LINEAGE_MISSING"))?;
    if lineage.canonical_sha256 != request.registration_lineage_canonical_sha256
        || lineage.receipt_object_sha256 != request.registration_lineage_receipt_object_sha256
        || lineage.session_id != request.session_id
        || lineage.project_id != request.project_id
        || lineage.candidate_id != request.candidate_id
        || lineage.candidate_state_sha256 != request.candidate_state_sha256
        || lineage.artifact_id != request.artifact_id
        || lineage.artifact_sha256 != request.artifact_sha256
        || lineage.registered_rig_v2_object_sha256 != request.registered_rig_v2_object_sha256
        || lineage.registered_rig_v2_canonical_sha256 != request.registered_rig_v2_canonical_sha256
        || !lineage.promotable
    {
        return Err(invalid(
            "OWNER_REVIEWED_VOID_CALIBRATION_LINEAGE_SCOPE_MISMATCH",
        ));
    }

    let rig_v2 = read_json(
        runtime,
        &request.registered_rig_v2_object_sha256,
        "RegisteredCameraRigCalibrationV2",
    )?;
    if canonical_document(
        &rig_v2,
        "RegisteredCameraRigCalibration@2",
        "RegisteredCameraRigCalibrationV2",
    )? != request.registered_rig_v2_canonical_sha256
    {
        return Err(invalid("OWNER_REVIEWED_VOID_CALIBRATION_RIG_V2_MISMATCH"));
    }

    let canvas = read_json(
        runtime,
        &art.reference_canvas_object_sha256,
        "ReferenceCanvas",
    )?;
    if canonical_document(&canvas, "ReferenceCanvas@1", "ReferenceCanvas")?
        != art.reference_canvas_canonical_sha256
    {
        return Err(invalid(
            "OWNER_REVIEWED_VOID_CALIBRATION_REFERENCE_CANVAS_MISMATCH",
        ));
    }
    let canvas_by_id = canvas_views(&canvas)?;
    let thresholds = ReviewedRegionPartBindingThresholds {
        min_owner_region_pixels: Some(STRICT_OWNER_VOID_MIN_OWNER_REGION_PIXELS),
        min_boundary_adjacency_pixels: Some(STRICT_OWNER_VOID_MIN_BOUNDARY_ADJACENCY_PIXELS),
        max_owner_expected_void_overlap_milli: Some(0),
    };
    let mut views = Vec::with_capacity(3);
    for view_kind in PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_VIEW_KINDS {
        let art_view = art
            .views
            .iter()
            .find(|view| view.view_kind == view_kind)
            .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_FORM_ART_VIEW_MISSING"))?;
        let baseline_view = baseline
            .views
            .iter()
            .find(|view| view.view_kind == view_kind)
            .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_BASELINE_VIEW_MISSING"))?;
        if baseline_view.view_id != art_view.view_id
            || baseline_view.reference_id != art_view.reference_id
            || baseline_view.reference_sha256 != art_view.reference_sha256
            || baseline_view.camera_hash != art_view.camera_hash
            || baseline_view.camera_canonical_sha256 != art_view.camera_canonical_sha256
        {
            return Err(invalid(
                "OWNER_REVIEWED_VOID_CALIBRATION_VIEW_COHORT_MISMATCH",
            ));
        }
        let canvas_view = canvas_by_id
            .get(&art_view.view_id)
            .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_CANVAS_VIEW_MISSING"))?;
        let view_spec = canvas_view
            .get("view_spec")
            .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_VIEW_SPEC_MISSING"))?;
        let crop = super::reference_view_crop(view_spec)?;
        let rotation_degrees = super::reference_view_rotation_degrees(view_spec)?;
        let target = runtime.read_silhouette_target(&art_view.target_object_sha256)?;
        if target.get("canonical_sha256").and_then(Value::as_str)
            != Some(art_view.target_canonical_sha256.as_str())
            || target.get("reference_id").and_then(Value::as_str)
                != Some(art_view.reference_id.as_str())
            || target.get("reference_sha256").and_then(Value::as_str)
                != Some(art_view.reference_sha256.as_str())
            || target.get("source").and_then(Value::as_str) != Some("user_refined")
            || target.get("annotation_status").and_then(Value::as_str) != Some("user_confirmed")
        {
            return Err(invalid(
                "OWNER_REVIEWED_VOID_CALIBRATION_REVIEWED_TARGET_MISMATCH",
            ));
        }
        let visual_structure = target
            .get("visual_structure")
            .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_STRUCTURE_MISSING"))?;
        super::validate_reference_visual_structure(visual_structure)?;
        if visual_structure
            .get("review_status")
            .and_then(Value::as_str)
            != Some("user_confirmed")
            || visual_structure
                .get("canonical_sha256")
                .and_then(Value::as_str)
                != Some(art_view.visual_structure_canonical_sha256.as_str())
        {
            return Err(invalid(
                "OWNER_REVIEWED_VOID_CALIBRATION_STRUCTURE_NOT_USER_CONFIRMED",
            ));
        }
        let target_mask = super::project_reference_mask_to_view(
            &runtime
                .target_mask(&art_view.target_object_sha256, &target)?
                .mask,
            view_spec,
            true,
        )?;
        let source_view_receipt = read_json(
            runtime,
            &art_view.form_evidence_view_receipt_object_sha256,
            "FormEvidence view receipt",
        )?;
        let render_set_object_sha256 = source_view_receipt
            .get("render_set_object_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_RENDER_SET_MISSING"))?;
        let render_set_canonical_sha256 = source_view_receipt
            .get("render_set_canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("OWNER_REVIEWED_VOID_CALIBRATION_RENDER_SET_INVALID"))?;
        if baseline_view.render_set_object_sha256 != render_set_object_sha256
            || baseline_view.render_set_canonical_sha256 != render_set_canonical_sha256
        {
            return Err(invalid(
                "OWNER_REVIEWED_VOID_CALIBRATION_BASELINE_RENDER_SET_MISMATCH",
            ));
        }
        let render_set = read_json(runtime, render_set_object_sha256, "RenderSet")?;
        super::validate_persisted_render_set_v2_output(&render_set)?;
        if canonical_document(&render_set, "RenderSet@2", "RenderSet")?
            != render_set_canonical_sha256
            || render_set.get("candidate_id").and_then(Value::as_str)
                != Some(request.candidate_id.as_str())
            || render_set.get("artifact_sha256").and_then(Value::as_str)
                != Some(request.artifact_sha256.as_str())
            || render_set.get("view_id").and_then(Value::as_str) != Some(art_view.view_id.as_str())
            || render_set.get("camera_hash").and_then(Value::as_str)
                != Some(art_view.camera_hash.as_str())
        {
            return Err(invalid(
                "OWNER_REVIEWED_VOID_CALIBRATION_RENDER_SET_SCOPE_MISMATCH",
            ));
        }
        let silhouette_pass = pass_hash(&render_set, "silhouette")?;
        let part_id_pass = pass_hash(&render_set, "part-id")?;
        let depth_pass = pass_hash(&render_set, "depth")?;
        if silhouette_pass != art_view.silhouette_pass_object_sha256
            || part_id_pass != art_view.part_id_pass_object_sha256
            || depth_pass != art_view.depth_pass_object_sha256
        {
            return Err(invalid(
                "OWNER_REVIEWED_VOID_CALIBRATION_AOV_BINDING_MISMATCH",
            ));
        }
        let part_png = ensure_png(runtime, &part_id_pass, "owner Part-ID")?;
        let depth_png = ensure_png(runtime, &depth_pass, "owner depth")?;
        let structure_id = match view_kind {
            "left" => "left.open-stock-void",
            "right" => "right.open-stock-void",
            "rear-three-quarter" => "rear3q.open-stock-void",
            _ => unreachable!("contract closes owner view kinds"),
        };
        let calibration = calibrate_depth_aware_reviewed_region_part_binding_with_rotation(
            visual_structure,
            &target_mask,
            &part_png,
            &depth_png,
            &part_ids,
            crop,
            rotation_degrees,
            structure_id,
            &thresholds,
        )?;
        let strict_depth_passed = calibration.owner_depth_valid_pixel_count
            == calibration.owner_pixel_count
            && calibration.depth_boundary_sample_count
                >= STRICT_OWNER_VOID_MIN_BOUNDARY_ADJACENCY_PIXELS;
        let view_binding_eligible = calibration.loose_identity_calibrated && strict_depth_passed;
        let strict_owner_void_passed = calibration.strict_zero_intrusion;
        let mut blocker_codes = Vec::new();
        if !calibration.identity_transform_unique {
            blocker_codes.push("REGISTERED_CAMERA_IDENTITY_NOT_UNIQUE".to_owned());
        }
        if calibration.owner_region_pixel_count < STRICT_OWNER_VOID_MIN_OWNER_REGION_PIXELS {
            blocker_codes.push("OWNER_REGION_TOO_SMALL".to_owned());
        }
        if calibration.owner_boundary_adjacency_pixel_count
            < STRICT_OWNER_VOID_MIN_BOUNDARY_ADJACENCY_PIXELS
        {
            blocker_codes.push("OWNER_BOUNDARY_ADJACENCY_TOO_SMALL".to_owned());
        }
        if !strict_depth_passed {
            blocker_codes.push("OWNER_DEPTH_BINDING_INCOMPLETE".to_owned());
        }
        if !strict_owner_void_passed {
            blocker_codes.push("STRICT_OWNER_VOID_ZERO_INTRUSION_NOT_MET".to_owned());
        }
        let mut projection_view = ProductionWeaponOwnerReviewedVoidCalibrationProjectionView {
            schema_version:
                PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_VIEW_SCHEMA_VERSION
                    .to_owned(),
            project_id: request.project_id.clone(),
            candidate_id: request.candidate_id.clone(),
            candidate_state_sha256: request.candidate_state_sha256.clone(),
            artifact_id: request.artifact_id.clone(),
            artifact_sha256: request.artifact_sha256.clone(),
            artifact_readback_sha256: request.artifact_readback_sha256.clone(),
            view_kind: view_kind.to_owned(),
            view_id: art_view.view_id.clone(),
            reviewed_structure_id: structure_id.to_owned(),
            reference_id: art_view.reference_id.clone(),
            reference_sha256: art_view.reference_sha256.clone(),
            camera_hash: art_view.camera_hash.clone(),
            camera_canonical_sha256: art_view.camera_canonical_sha256.clone(),
            camera_object_sha256: baseline_view.camera_object_sha256.clone(),
            render_set_object_sha256: render_set_object_sha256.to_owned(),
            render_set_canonical_sha256: render_set_canonical_sha256.to_owned(),
            render_set_view_id: baseline_view.render_set_view_id.clone(),
            form_art_view_receipt_object_sha256: art_view.receipt_object_sha256.clone(),
            form_art_view_receipt_canonical_sha256: art_view.canonical_sha256.clone(),
            baseline_view_receipt_object_sha256: baseline_view.receipt_object_sha256.clone(),
            target_object_sha256: art_view.target_object_sha256.clone(),
            target_canonical_sha256: art_view.target_canonical_sha256.clone(),
            visual_structure_canonical_sha256: art_view.visual_structure_canonical_sha256.clone(),
            silhouette_pass_object_sha256: silhouette_pass,
            part_id_pass_object_sha256: part_id_pass,
            depth_pass_object_sha256: depth_pass,
            owner_part_id:
                PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_OWNER_PART_ID
                    .to_owned(),
            derived_owner_region_sha256: calibration.owner_region_mask_sha256,
            derived_reviewed_void_region_sha256: calibration.expected_void_mask_sha256,
            derived_void_boundary_sha256: calibration.expected_boundary_mask_sha256,
            registered_camera_lineage_verified: true,
            derived_transform_kind: "identity".to_owned(),
            identity_transform_unique: calibration.identity_transform_unique,
            eligible_transform_count: calibration.eligible_transform_count,
            transform_rank_tie: calibration.transform_rank_tie,
            expected_void_pixel_count: calibration.expected_void_pixel_count,
            owner_region_pixel_count: calibration.owner_region_pixel_count,
            owner_expected_void_overlap_pixel_count: calibration
                .owner_expected_void_overlap_pixel_count,
            owner_expected_void_overlap_milli: calibration.owner_expected_void_overlap_milli,
            boundary_pixel_count: calibration.expected_boundary_pixel_count,
            owner_boundary_adjacency_pixel_count: calibration.owner_boundary_adjacency_pixel_count,
            owner_boundary_adjacency_milli: calibration.owner_boundary_adjacency_milli,
            depth_valid_pixel_count: calibration.owner_depth_valid_pixel_count,
            depth_owner_sample_count: calibration.owner_pixel_count,
            depth_boundary_sample_count: calibration.depth_boundary_sample_count,
            depth_invalid_sample_count: calibration
                .owner_pixel_count
                .saturating_sub(calibration.owner_depth_valid_pixel_count),
            depth_ordering_milli: calibration.depth_ordering_milli,
            depth_status: if strict_depth_passed {
                "OBSERVED"
            } else {
                "UNKNOWN"
            }
            .to_owned(),
            owner_void_status: if view_binding_eligible {
                "BOUND"
            } else {
                "BLOCKED"
            }
            .to_owned(),
            strict_owner_void_passed,
            strict_depth_passed,
            view_status: if view_binding_eligible {
                "ELIGIBLE"
            } else {
                "BLOCKED"
            }
            .to_owned(),
            view_passed: view_binding_eligible,
            blocker_codes,
            quality_status:
                PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_QUALITY_STATUS
                    .to_owned(),
            canonical_sha256: String::new(),
            created_at: baseline_view.created_at.clone(),
        };
        projection_view.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&projection_view).map_err(|error| invalid(error.to_string()))?,
        );
        views.push(projection_view);
    }

    let binding_all_views = views.iter().all(|view| view.view_status == "ELIGIBLE");
    let strict_owner_void_all_views_passed = views.iter().all(|view| view.strict_owner_void_passed);
    let strict_depth_all_views_passed = views.iter().all(|view| view.strict_depth_passed);
    let identity_transform_all_views_unique =
        views.iter().all(|view| view.identity_transform_unique);
    let all_views_passed = binding_all_views;
    let calibration_status = if binding_all_views {
        "ELIGIBLE"
    } else {
        "BLOCKED"
    };
    let mut blocker_codes = views
        .iter()
        .flat_map(|view| view.blocker_codes.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if binding_all_views {
        blocker_codes.retain(|code| code == "STRICT_OWNER_VOID_ZERO_INTRUSION_NOT_MET");
    }
    let observed_worker_cohorts = worker_cohorts.iter().flatten().collect::<BTreeSet<_>>();
    if worker_cohorts.iter().any(Option::is_none)
        || observed_worker_cohorts.len() != 1
        || observed_worker_cohorts
            .iter()
            .next()
            .map(|value| value.as_str())
            != Some(baseline.runtime_build_cohort_sha256.as_str())
    {
        return Err(invalid(
            "OWNER_REVIEWED_VOID_CALIBRATION_WORKER_COHORT_MISMATCH",
        ));
    }
    let runtime_build_cohort_sha256 = baseline.runtime_build_cohort_sha256.clone();
    let mut projection = ProductionWeaponOwnerReviewedVoidCalibrationProjection {
        schema_version: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_SCHEMA_VERSION
            .to_owned(),
        projection_id: request.projection_id.clone(),
        operation: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_GET_OPERATION
            .to_owned(),
        session_id: request.session_id.clone(),
        project_id: request.project_id.clone(),
        candidate_id: request.candidate_id.clone(),
        candidate_state_sha256: request.candidate_state_sha256.clone(),
        artifact_id: request.artifact_id.clone(),
        artifact_sha256: request.artifact_sha256.clone(),
        artifact_readback_sha256: request.artifact_readback_sha256.clone(),
        form_art_evidence_id: request.form_art_evidence_id.clone(),
        form_art_evidence_object_sha256: request.form_art_evidence_object_sha256.clone(),
        form_art_evidence_canonical_sha256: request.form_art_evidence_canonical_sha256.clone(),
        fresh_baseline_id: request.fresh_baseline_id.clone(),
        fresh_baseline_canonical_sha256: request.fresh_baseline_canonical_sha256.clone(),
        fresh_baseline_receipt_object_sha256: request.fresh_baseline_receipt_object_sha256.clone(),
        registration_lineage_id: request.registration_lineage_id.clone(),
        registration_lineage_canonical_sha256: request
            .registration_lineage_canonical_sha256
            .clone(),
        registration_lineage_receipt_object_sha256: request
            .registration_lineage_receipt_object_sha256
            .clone(),
        registered_rig_v2_id: request.registered_rig_v2_id.clone(),
        registered_rig_v2_object_sha256: request.registered_rig_v2_object_sha256.clone(),
        registered_rig_v2_canonical_sha256: request.registered_rig_v2_canonical_sha256.clone(),
        runtime_build_cohort_sha256,
        owner_part_id: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_OWNER_PART_ID
            .to_owned(),
        view_kinds: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_VIEW_KINDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        views,
        calibration_policy: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_POLICY
            .to_owned(),
        calibration_policy_sha256: sha256_hex(
            PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_POLICY.as_bytes(),
        ),
        transform_policy:
            PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_TRANSFORM_POLICY.to_owned(),
        reviewed_void_policy:
            PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_REVIEWED_VOID_POLICY
                .to_owned(),
        depth_policy: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_DEPTH_POLICY
            .to_owned(),
        depth_policy_sha256: sha256_hex(
            PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_DEPTH_POLICY.as_bytes(),
        ),
        threshold_policy:
            PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_THRESHOLD_POLICY.to_owned(),
        threshold_policy_sha256: sha256_hex(
            PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_THRESHOLD_POLICY
                .as_bytes(),
        ),
        calibration_status: calibration_status.to_owned(),
        blocker_codes,
        strict_owner_void_all_views_passed,
        strict_depth_all_views_passed,
        identity_transform_all_views_unique,
        all_views_passed,
        eligible: binding_all_views,
        promotable: false,
        quality_status: PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_QUALITY_STATUS
            .to_owned(),
        depth_status: if strict_depth_all_views_passed {
            "OBSERVED"
        } else {
            "UNKNOWN"
        }
        .to_owned(),
        runtime_write_performed: false,
        persistent_user_data_touched: false,
        worker_started: false,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256: String::new(),
        input_sha256: request.input_sha256.clone(),
        canonicalization_policy:
            PRODUCTION_WEAPON_OWNER_REVIEWED_VOID_CALIBRATION_PROJECTION_CANONICALIZATION_POLICY
                .to_owned(),
        canonical_sha256: String::new(),
        created_at: baseline.created_at.clone(),
    };
    projection.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&projection).map_err(|error| invalid(error.to_string()))?,
    );
    Ok(projection)
}

/// Evaluate one directly rendered, registered-camera Part-ID binding against
/// the strict owner-void policy. Discovery-time image transforms are
/// deliberately forbidden here: a production acceptance input must already
/// be aligned in the registered camera coordinate space and therefore select
/// the identity transform without post-processing the Part-ID mask.
pub(crate) fn strict_reviewed_region_part_binding_assessment(
    calibration: &ReviewedRegionPartBindingCalibration,
    registered_camera_lineage_verified: bool,
) -> Result<StrictReviewedRegionPartBindingAssessment, RuntimeError> {
    if !registered_camera_lineage_verified {
        return Err(invalid(
            "OWNER_VOID_ACCEPTANCE_BLOCKED: registered camera lineage is unavailable",
        ));
    }
    if calibration.owner_part_id != "rear-stock"
        || !matches!(
            calibration.structure_id.as_str(),
            "left.open-stock-void" | "right.open-stock-void" | "rear3q.open-stock-void"
        )
    {
        return Err(invalid(
            "OWNER_VOID_ACCEPTANCE_BLOCKED: exact structure owner binding differs",
        ));
    }
    if calibration.authored_transform != Some(ReviewedRegionPartBindingTransform::Identity)
        || calibration.selected_transform != ReviewedRegionPartBindingTransform::Identity
        || calibration.status != "bound-diagnostic-only"
    {
        return Err(invalid(
            "OWNER_VOID_ACCEPTANCE_BLOCKED: Part-ID was not produced directly in the registered camera frame",
        ));
    }
    let eligible = calibration
        .candidates
        .iter()
        .filter(|candidate| candidate.passes_thresholds)
        .collect::<Vec<_>>();
    let Some(best) = eligible
        .iter()
        .copied()
        .max_by(|left, right| reviewed_region_part_binding_score_cmp(left, right))
    else {
        return Err(invalid(
            "OWNER_VOID_ACCEPTANCE_BLOCKED: no registered-camera binding candidate is eligible",
        ));
    };
    let best_tie_count = eligible
        .iter()
        .filter(|candidate| {
            reviewed_region_part_binding_score_cmp(candidate, best) == std::cmp::Ordering::Equal
        })
        .count();
    if best_tie_count != 1
        || best.transform != ReviewedRegionPartBindingTransform::Identity
        || calibration.selected_transform != best.transform
    {
        return Err(invalid(
            "OWNER_VOID_ACCEPTANCE_BLOCKED: identity binding is not the unique ranked candidate",
        ));
    }
    let selected = best;
    if calibration.expected_void_pixel_count < 256
        || calibration.expected_boundary_pixel_count < 64
        || selected.owner_region_pixel_count < 128
        || selected.owner_boundary_adjacency_pixel_count < 32
        || selected.owner_boundary_adjacency_milli < 250
        || selected.owner_expected_void_overlap_pixel_count != 0
        || selected.owner_expected_void_overlap_milli != 0
    {
        return Err(invalid(
            "OWNER_VOID_ACCEPTANCE_BLOCKED: strict owner-void thresholds are not met",
        ));
    }
    Ok(StrictReviewedRegionPartBindingAssessment {
        structure_id: calibration.structure_id.clone(),
        owner_part_id: calibration.owner_part_id.clone(),
        policy: STRICT_OWNER_VOID_POLICY.to_owned(),
        expected_region_canonical_sha256: calibration.expected_region_canonical_sha256.clone(),
        expected_void_pixel_count: calibration.expected_void_pixel_count,
        expected_boundary_pixel_count: calibration.expected_boundary_pixel_count,
        owner_region_pixel_count: selected.owner_region_pixel_count,
        owner_boundary_adjacency_pixel_count: selected.owner_boundary_adjacency_pixel_count,
        owner_boundary_adjacency_milli: selected.owner_boundary_adjacency_milli,
        owner_expected_void_overlap_pixel_count: selected.owner_expected_void_overlap_pixel_count,
        owner_expected_void_overlap_milli: selected.owner_expected_void_overlap_milli,
        status: "PASS_BOUND_DIAGNOSTIC".to_owned(),
        promotable: false,
        quality_status: "NOT_PROVEN".to_owned(),
        depth_status: "UNKNOWN".to_owned(),
    })
}

fn pass_hash(render: &Value, pass: &str) -> Result<String, RuntimeError> {
    render
        .get("pass_artifacts")
        .and_then(|value| value.get(pass))
        .and_then(|value| value.get("sha256"))
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("RenderSet {pass} pass hash is unavailable")))
}

fn ensure_png(runtime: &Runtime, hash: &str, label: &str) -> Result<Vec<u8>, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)
        .map_err(RuntimeError::from)?
        .ok_or_else(|| invalid(format!("{label} CAS object is unavailable")))?;
    if object.mime != "image/png" || object.size_bytes == 0 || object.size_bytes > 16 * 1024 * 1024
    {
        return Err(invalid(format!("{label} CAS metadata is invalid")));
    }
    let bytes = runtime.cas_read_bounded(hash, 16 * 1024 * 1024)?;
    let _ = decode_image(&bytes, label)?;
    Ok(bytes)
}

/// Return the Part IDs that are actually visible in one candidate-bound
/// Part-ID AOV.  The ArtifactReadback list remains the authority for the
/// palette vocabulary; a palette entry outside that list is not silently
/// treated as background or as an unknown part.
pub(crate) fn visible_part_ids(
    part_png: &[u8],
    part_ids: &[String],
) -> Result<Vec<String>, RuntimeError> {
    if part_ids.is_empty() {
        return Ok(Vec::new());
    }
    let image = image::load_from_memory(part_png)
        .map_err(|error| invalid(format!("part-id image is invalid: {error}")))?
        .resize_exact(512, 512, imageops::FilterType::Nearest)
        .to_rgba8();
    let mut visible_indices = BTreeSet::new();
    for pixel in image.pixels() {
        let Some(index) = super::part_color_index(pixel.0) else {
            continue;
        };
        if index >= part_ids.len() {
            return Err(invalid(format!(
                "part-id image contains palette index {index} outside ArtifactReadback part_ids"
            )));
        }
        visible_indices.insert(index);
    }
    Ok(part_ids
        .iter()
        .enumerate()
        .filter(|(index, _)| visible_indices.contains(index))
        .map(|(_, part_id)| part_id.clone())
        .collect())
}

/// Project the durable FormEvidence observation into the expected inventory
/// for one view.  FormEvidence's `expected_part_ids` is intentionally not
/// used here: it is the complete artifact vocabulary and therefore includes
/// Parts that are occluded or outside this view.  Only the source view's
/// candidate-bound `observed_part_ids` may become this view's expected set.
fn per_view_expected_part_ids(
    artifact_part_ids: &[String],
    source_observed_part_ids: &[String],
    view_kind: &str,
) -> Result<Vec<String>, RuntimeError> {
    let artifact_set = artifact_part_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for part_id in source_observed_part_ids {
        if !seen.insert(part_id.as_str()) {
            return Err(invalid(format!(
                "FormEvidence {view_kind} visible Part-ID inventory is duplicated"
            )));
        }
        if !artifact_set.contains(part_id.as_str()) {
            return Err(invalid(format!(
                "FormEvidence {view_kind} visible Part-ID is outside ArtifactReadback"
            )));
        }
    }
    // Preserve the canonical ArtifactReadback order.  This makes the derived
    // expected set deterministic across restarts and prevents a caller from
    // changing only array order to create a different receipt.
    Ok(artifact_part_ids
        .iter()
        .filter(|part_id| seen.contains(part_id.as_str()))
        .cloned()
        .collect())
}

fn validate_visible_part_inventory(
    expected_part_ids: &[String],
    observed_part_ids: &[String],
    view_kind: &str,
) -> Result<(u64, u64, u64, u64), RuntimeError> {
    let expected = expected_part_ids.iter().collect::<BTreeSet<_>>();
    let observed = observed_part_ids.iter().collect::<BTreeSet<_>>();
    let missing = expected
        .difference(&observed)
        .map(|part_id| (*part_id).clone())
        .collect::<Vec<_>>();
    let unexpected = observed
        .difference(&expected)
        .map(|part_id| (*part_id).clone())
        .collect::<Vec<_>>();
    if !missing.is_empty() || !unexpected.is_empty() {
        return Err(invalid(format!(
            "FormArt {view_kind} Part-ID visible inventory differs: missing={missing:?}, unexpected={unexpected:?}"
        )));
    }
    let expected_count = expected.len() as u64;
    let observed_count = observed.len() as u64;
    // An empty expected set means that no Part is visible in this view.  It is
    // complete by definition, so coverage remains 1000 rather than turning a
    // valid occlusion-only view into a false quality failure.
    let coverage = if expected_count == 0 {
        1000
    } else {
        (observed_count * 1000 / expected_count).min(1000)
    };
    Ok((expected_count, observed_count, 0, coverage))
}

/// Derive one proposal-candidate FormArt observation from its own fixed AOVs.
/// The caller supplies only source-reviewed annotation/camera-frame truth and
/// the source view's expected visible Part set; no source candidate pixels or
/// FormEvidence receipt are reused as proposal evidence.
pub(crate) fn derive_proposal_form_art_observation(
    visual_structure: Option<&Value>,
    visual_confirmed: bool,
    target_mask: &[bool],
    silhouette_png: &[u8],
    part_png: &[u8],
    depth_png: &[u8],
    normal_png: &[u8],
    artifact_part_ids: &[String],
    expected_visible_part_ids: &[String],
    crop: [f64; 4],
    rotation_degrees: f64,
) -> Result<Value, RuntimeError> {
    if target_mask.len() != 512 * 512
        || artifact_part_ids.is_empty()
        || artifact_part_ids.iter().collect::<BTreeSet<_>>().len() != artifact_part_ids.len()
    {
        return Err(invalid(
            "PROPOSAL_FORM_ART_OBSERVATION_BLOCKED: invalid target or Part vocabulary",
        ));
    }
    let artifact_set = artifact_part_ids.iter().collect::<BTreeSet<_>>();
    if expected_visible_part_ids
        .iter()
        .any(|part_id| !artifact_set.contains(part_id))
    {
        return Err(invalid(
            "PROPOSAL_FORM_ART_OBSERVATION_BLOCKED: source expected Part is absent from proposal ArtifactReadback",
        ));
    }
    let model_mask = super::decode_binary_mask(silhouette_png)?;
    let observed_part_ids = visible_part_ids(part_png, artifact_part_ids)?;
    let expected = expected_visible_part_ids.iter().collect::<BTreeSet<_>>();
    let observed = observed_part_ids.iter().collect::<BTreeSet<_>>();
    let missing_part_ids = expected
        .difference(&observed)
        .map(|value| (*value).clone())
        .collect::<Vec<_>>();
    let unexpected_part_ids = observed
        .difference(&expected)
        .map(|value| (*value).clone())
        .collect::<Vec<_>>();
    // A proposal is expected to change visibility. Missing or newly revealed
    // Parts are quality evidence, not a malformed transport. Preserve them in
    // the blocked observation instead of aborting before the review receipt.
    let expected_count = expected.len() as u64;
    let observed_count = observed.len() as u64;
    let matched_count = expected.intersection(&observed).count() as u64;
    let unexpected_count = unexpected_part_ids.len() as u64;
    let coverage_milli = if expected_count == 0 {
        if observed_count == 0 {
            1000
        } else {
            0
        }
    } else {
        matched_count * 1000 / expected_count
    };
    let part_id_status = if missing_part_ids.is_empty() && unexpected_part_ids.is_empty() {
        "observed"
    } else {
        "unknown"
    };
    let (negative_space_status, negative_space_rows) = negative_rows_with_rotation(
        visual_structure,
        visual_confirmed,
        target_mask,
        &model_mask,
        crop,
        rotation_degrees,
    )?;
    let edge = edge_from_aovs(
        &model_mask,
        part_png,
        depth_png,
        normal_png,
        artifact_part_ids,
    )?;
    let (line_flow_status, line_flow_rows) = line_rows_with_rotation(
        visual_structure,
        visual_confirmed,
        &edge,
        crop,
        rotation_degrees,
    )?;
    let view_observation_status = if part_id_status == "observed"
        && matches!(
            negative_space_status.as_str(),
            "observed" | "not-applicable"
        )
        && matches!(line_flow_status.as_str(), "observed" | "not-applicable")
    {
        "observed"
    } else if visual_confirmed {
        "inferred"
    } else {
        "unknown"
    };
    Ok(json!({
        "part_id_status":part_id_status,
        "part_id_expected_count":expected_count,
        "part_id_observed_count":observed_count,
        "part_id_missing_count":missing_part_ids.len(),
        "part_id_unexpected_count":unexpected_count,
        "part_id_coverage_milli":coverage_milli,
        "expected_visible_part_ids":expected_visible_part_ids,
        "observed_part_ids":observed_part_ids,
        "missing_part_ids":missing_part_ids,
        "unexpected_part_ids":unexpected_part_ids,
        "negative_space_status":negative_space_status,
        "negative_space_rows":negative_space_rows,
        "line_flow_status":line_flow_status,
        "line_flow_rows":line_flow_rows,
        "view_observation_status":view_observation_status,
        "quality_status":"NOT_PROVEN"
    }))
}

fn derive_views(
    runtime: &Runtime,
    request: &ProductionWeaponFormArtEvidencePrepareRequest,
    form: &forgecad_contracts::ProductionWeaponFormEvidenceRecord,
    canvas: &Value,
    rig: &Value,
    candidate_state_sha256: &str,
    artifact_id: &str,
    artifact_sha256: &str,
    created_at: &str,
) -> Result<
    (
        Vec<ProductionWeaponFormArtEvidenceViewRecord>,
        ProductionWeaponFormArtEvidencePartIdAggregate,
    ),
    RuntimeError,
> {
    let canvas_by_id = canvas_views(canvas)?;
    let rig_by_kind = rig_views(rig)?;
    let readback = runtime.artifact_readback(artifact_sha256, &request.candidate_id)?;
    let expected_ids = readback
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ArtifactReadback part_ids are missing"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("ArtifactReadback part id is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut per_view_expected = Vec::with_capacity(VIEW_KINDS.len());
    let mut expected_visible_union = BTreeSet::new();
    for (ordinal, kind) in VIEW_KINDS.iter().enumerate() {
        let source_view = form
            .views
            .get(ordinal)
            .ok_or_else(|| invalid("FormEvidence view ordering is incomplete"))?;
        if source_view.view_kind != *kind {
            return Err(invalid("FormEvidence view ordering differs"));
        }
        if source_view.part_id_evidence.expected_part_ids != expected_ids {
            return Err(invalid(format!(
                "FormEvidence {kind} Part-ID vocabulary differs from ArtifactReadback"
            )));
        }
        let view_expected = per_view_expected_part_ids(
            &expected_ids,
            &source_view.part_id_evidence.observed_part_ids,
            kind,
        )?;
        expected_visible_union.extend(view_expected.iter().cloned());
        per_view_expected.push(view_expected);
    }
    if expected_visible_union.is_empty() {
        return Err(invalid(
            "FormEvidence contains no candidate-visible Part-ID evidence across six views",
        ));
    }
    let mut aggregate_observed = BTreeSet::new();
    let mut views = Vec::new();
    for (ordinal, kind) in VIEW_KINDS.iter().enumerate() {
        let source_view = form
            .views
            .get(ordinal)
            .ok_or_else(|| invalid("FormEvidence view ordering is incomplete"))?;
        if source_view.view_kind != *kind {
            return Err(invalid("FormEvidence view ordering differs"));
        }
        let canvas_view = canvas_by_id
            .get(&source_view.view_id)
            .ok_or_else(|| invalid("ReferenceCanvas view is missing"))?;
        if canvas_view.get("kind").and_then(Value::as_str) != Some(*kind)
            || canvas_view.get("reference_id").and_then(Value::as_str)
                != Some(source_view.reference_id.as_str())
            || canvas_view.get("reference_sha256").and_then(Value::as_str)
                != Some(source_view.reference_sha256.as_str())
        {
            return Err(invalid("ReferenceCanvas view binding differs"));
        }
        let target_hash = canvas_view
            .get("target_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| invalid("FORM_ART_EVIDENCE_TARGET_UNAVAILABLE"))?;
        let target = runtime.read_silhouette_target(target_hash)?;
        if target.get("reference_id").and_then(Value::as_str)
            != Some(source_view.reference_id.as_str())
            || target.get("reference_sha256").and_then(Value::as_str)
                != Some(source_view.reference_sha256.as_str())
        {
            return Err(invalid("SilhouetteTarget reference binding differs"));
        }
        let target_canonical = target
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| invalid("SilhouetteTarget canonical is unavailable"))?
            .to_owned();
        let target_annotation_confirmed = target.get("source").and_then(Value::as_str)
            == Some("user_refined")
            && target.get("annotation_status").and_then(Value::as_str) == Some("user_confirmed");
        let visual_structure = target.get("visual_structure");
        if let Some(structure) = visual_structure {
            super::validate_reference_visual_structure(structure)?;
        }
        let structure_review_status = visual_structure
            .and_then(|value| value.get("review_status"))
            .and_then(Value::as_str);
        // `unreviewed` is valid in the source target contract but is not a
        // valid output status for this evidence contract.  It maps to
        // `unknown`; only the full target + visual-structure confirmation
        // permits observed negative/line observations.
        let visual_confirmed =
            target_annotation_confirmed && structure_review_status == Some("user_confirmed");
        let visual_status = if visual_confirmed {
            "user_confirmed"
        } else if structure_review_status == Some("user_confirmed") {
            "inferred"
        } else {
            "unknown"
        };
        let visual_hash = visual_structure
            .and_then(|value| value.get("canonical_sha256"))
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .map(str::to_owned)
            .unwrap_or_else(|| sha256_hex(UNKNOWN_STRUCTURE_HASH_SEED));
        let rig_view = rig_by_kind
            .get(*kind)
            .ok_or_else(|| invalid("CameraRig view is missing"))?;
        if rig_view
            .get("registered_camera_hash")
            .and_then(Value::as_str)
            != Some(source_view.camera_hash.as_str())
            || rig_view
                .get("registered_camera")
                .and_then(|camera| camera.get("canonical_sha256"))
                .and_then(Value::as_str)
                != Some(source_view.camera_canonical_sha256.as_str())
        {
            return Err(invalid("CameraRig view binding differs"));
        }
        let render: Value = read_json(runtime, &source_view.render_set_object_sha256, "RenderSet")?;
        super::validate_render_set_v2_output(&render)?;
        if canonical_document(&render, "RenderSet@2", "RenderSet")?
            != source_view.render_set_canonical_sha256
            || render.get("candidate_id").and_then(Value::as_str)
                != Some(request.candidate_id.as_str())
            || render.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256)
            || render.get("reference_id").and_then(Value::as_str)
                != Some(source_view.reference_id.as_str())
            || render.get("camera_hash").and_then(Value::as_str)
                != Some(source_view.camera_hash.as_str())
            || render.get("view_id").and_then(Value::as_str) != Some(source_view.view_id.as_str())
        {
            return Err(invalid("RenderSet binding differs"));
        }
        let silhouette_hash = pass_hash(&render, "silhouette")?;
        let part_hash = pass_hash(&render, "part-id")?;
        let depth_hash = pass_hash(&render, "depth")?;
        let normal_hash = pass_hash(&render, "normal")?;
        let silhouette_png = ensure_png(runtime, &silhouette_hash, "silhouette")?;
        let part_png = ensure_png(runtime, &part_hash, "part-id")?;
        let depth_png = ensure_png(runtime, &depth_hash, "depth")?;
        let normal_png = ensure_png(runtime, &normal_hash, "normal")?;
        let model_mask = super::decode_binary_mask(&silhouette_png)?;
        let view_spec = canvas_view
            .get("view_spec")
            .ok_or_else(|| invalid("ReferenceCanvas view_spec is missing"))?;
        let crop = super::reference_view_crop(view_spec)?;
        let rotation_degrees = super::reference_view_rotation_degrees(view_spec)?;
        let target_mask = super::project_reference_mask_to_view(
            &runtime.target_mask(target_hash, &target)?.mask,
            view_spec,
            true,
        )?;
        let view_expected_ids = &per_view_expected[ordinal];
        let observed_ids = visible_part_ids(&part_png, &expected_ids)?;
        let (expected_count, observed_count, unexpected_count, coverage_milli) =
            validate_visible_part_inventory(view_expected_ids, &observed_ids, kind)?;
        for id in &observed_ids {
            aggregate_observed.insert(id.clone());
        }
        let aggregate = ProductionWeaponFormArtEvidencePartIdAggregate {
            status: "observed".into(),
            expected_count,
            observed_count,
            missing_count: 0,
            unexpected_count,
            coverage_milli,
        };
        let (negative_status, negative_rows) = negative_rows_with_rotation(
            visual_structure,
            visual_confirmed,
            &target_mask,
            &model_mask,
            crop,
            rotation_degrees,
        )?;
        let edge = edge_from_aovs(
            &model_mask,
            &part_png,
            &depth_png,
            &normal_png,
            &expected_ids,
        )?;
        let (line_status, line_rows) = line_rows_with_rotation(
            visual_structure,
            visual_confirmed,
            &edge,
            crop,
            rotation_degrees,
        )?;
        let observation_status = if aggregate.status == "observed"
            && (negative_status == "observed" || negative_status == "not-applicable")
            && (line_status == "observed" || line_status == "not-applicable")
        {
            "observed"
        } else if visual_confirmed {
            "inferred"
        } else {
            "unknown"
        };
        let mut view = ProductionWeaponFormArtEvidenceViewRecord {
            schema_version: PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_SCHEMA_VERSION.into(),
            project_id: request.project_id.clone(),
            candidate_id: request.candidate_id.clone(),
            candidate_state_sha256: candidate_state_sha256.into(),
            artifact_id: artifact_id.into(),
            artifact_sha256: artifact_sha256.into(),
            view_kind: (*kind).into(),
            view_id: source_view.view_id.clone(),
            reference_id: source_view.reference_id.clone(),
            reference_sha256: source_view.reference_sha256.clone(),
            camera_hash: source_view.camera_hash.clone(),
            camera_canonical_sha256: source_view.camera_canonical_sha256.clone(),
            form_evidence_view_receipt_object_sha256: source_view.receipt_object_sha256.clone(),
            form_evidence_view_receipt_canonical_sha256: source_view.canonical_sha256.clone(),
            target_object_sha256: target_hash.into(),
            target_canonical_sha256: target_canonical,
            visual_structure_canonical_sha256: visual_hash,
            visual_structure_review_status: visual_status.into(),
            silhouette_pass_object_sha256: silhouette_hash,
            part_id_pass_object_sha256: part_hash,
            depth_pass_object_sha256: depth_hash,
            normal_pass_object_sha256: normal_hash,
            part_id_status: aggregate.status.clone(),
            part_id_expected_count: aggregate.expected_count,
            part_id_observed_count: aggregate.observed_count,
            part_id_missing_count: aggregate.missing_count,
            part_id_unexpected_count: aggregate.unexpected_count,
            part_id_coverage_milli: aggregate.coverage_milli,
            negative_space_status: negative_status,
            negative_space_rows: negative_rows,
            line_flow_status: line_status,
            line_flow_rows: line_rows,
            view_observation_status: observation_status.into(),
            quality_status: PRODUCTION_WEAPON_FORM_ART_EVIDENCE_QUALITY_STATUS.into(),
            receipt_object_sha256: String::new(),
            canonical_sha256: String::new(),
            created_at: created_at.into(),
        };
        view.canonical_sha256 = canonical_json_hash(&normalized_view(&view)?);
        views.push(view);
    }
    let status = if aggregate_observed.len() == expected_visible_union.len() {
        "observed"
    } else {
        "inferred"
    };
    let part_id_aggregate = ProductionWeaponFormArtEvidencePartIdAggregate {
        status: status.into(),
        expected_count: expected_visible_union.len() as u64,
        observed_count: aggregate_observed.len() as u64,
        missing_count: expected_visible_union
            .difference(&aggregate_observed)
            .count() as u64,
        unexpected_count: aggregate_observed
            .difference(&expected_visible_union)
            .count() as u64,
        coverage_milli: if expected_visible_union.is_empty() {
            1000
        } else {
            (aggregate_observed.len() as u64 * 1000 / expected_visible_union.len() as u64).min(1000)
        },
    };
    Ok((views, part_id_aggregate))
}

fn validate_source(
    runtime: &Runtime,
    request: &ProductionWeaponFormArtEvidencePrepareRequest,
) -> Result<
    (
        forgecad_contracts::ProductionWeaponFormEvidenceRecord,
        Value,
        Value,
        Value,
    ),
    RuntimeError,
> {
    let form = source_form_evidence(runtime, request)?;
    let candidate = runtime
        .candidate(&request.candidate_id)?
        .ok_or_else(|| invalid("candidate is unavailable"))?;
    if candidate.project_id != request.project_id || candidate.canonical_sha256.is_empty() {
        return Err(invalid("candidate binding differs"));
    }
    if candidate.canonical_sha256 != form.candidate_state_sha256
        || candidate.prepared_object_sha256.as_deref() != Some(form.artifact_sha256.as_str())
        || candidate.prepared_object_id.as_deref() != Some(form.artifact_id.as_str())
    {
        return Err(invalid("candidate/artifact binding differs"));
    }
    let _readback = runtime.artifact_readback(&form.artifact_sha256, &request.candidate_id)?;
    let canvas = validate_canvas_and_spec(runtime, &form)?;
    let lock = runtime
        .store
        .get_production_camera_lock(&form.camera_lock_id)
        .map_err(RuntimeError::from)?
        .ok_or_else(|| invalid("CameraLock is unavailable"))?;
    super::agentic_session::validate_production_camera_lock_record(runtime, &lock)?;
    if lock.session_id != form.session_id
        || lock.project_id != form.project_id
        || lock.candidate_id != form.candidate_id
        || lock.candidate_state_sha256 != form.candidate_state_sha256
        || lock.artifact_id != form.artifact_id
        || lock.artifact_sha256 != form.artifact_sha256
        || lock.reference_canvas_object_sha256 != form.reference_canvas_object_sha256
        || lock.design_spec_object_sha256 != form.design_spec_object_sha256
    {
        return Err(invalid("CameraLock binding differs"));
    }
    let subject_rig = read_json(runtime, &form.camera_rig_object_sha256, "CameraRig")?;
    let registered_rig = super::agentic_session::materialize_production_camera_lock_registered_rig(
        runtime,
        &lock.project_id,
        &lock.candidate_id,
        &lock.candidate_state_sha256,
        &lock.artifact_id,
        &lock.artifact_sha256,
        &subject_rig,
        &lock.camera_rig_object_sha256,
    )?;
    Ok((form, canvas, registered_rig, Value::Null))
}

fn record_from_source(
    runtime: &Runtime,
    request: &ProductionWeaponFormArtEvidencePrepareRequest,
    request_sha256: &str,
) -> Result<ProductionWeaponFormArtEvidenceRecord, RuntimeError> {
    let (form, canvas, rig, _) = validate_source(runtime, request)?;
    let (views, part_id_aggregate) = derive_views(
        runtime,
        request,
        &form,
        &canvas,
        &rig,
        &form.candidate_state_sha256,
        &form.artifact_id,
        &form.artifact_sha256,
        &form.created_at,
    )?;
    Ok(ProductionWeaponFormArtEvidenceRecord {
        schema_version: "ProductionWeaponFormArtEvidence@1".into(),
        art_evidence_id: request.art_evidence_id.clone(),
        session_id: request.session_id.clone(),
        project_id: request.project_id.clone(),
        candidate_id: request.candidate_id.clone(),
        candidate_state_sha256: form.candidate_state_sha256,
        artifact_id: form.artifact_id,
        artifact_sha256: form.artifact_sha256,
        reference_canvas_object_sha256: form.reference_canvas_object_sha256,
        reference_canvas_canonical_sha256: form.reference_canvas_canonical_sha256,
        design_spec_object_sha256: form.design_spec_object_sha256,
        design_spec_canonical_sha256: form.design_spec_canonical_sha256,
        camera_lock_id: form.camera_lock_id,
        camera_lock_canonical_sha256: form.camera_lock_canonical_sha256,
        camera_rig_object_sha256: form.camera_rig_object_sha256,
        camera_rig_canonical_sha256: form.camera_rig_canonical_sha256,
        camera_lock_receipt_object_sha256: form.camera_lock_receipt_object_sha256,
        camera_lock_source_transition_id: form.camera_lock_source_transition_id,
        camera_lock_source_transition_sha256: form.camera_lock_source_transition_sha256,
        camera_lock_source_head_canonical_sha256: form.camera_lock_source_head_canonical_sha256,
        form_evidence_object_sha256: request.form_evidence_object_sha256.clone(),
        form_evidence_canonical_sha256: request.form_evidence_canonical_sha256.clone(),
        view_kinds: VIEW_KINDS.iter().map(|value| (*value).into()).collect(),
        views,
        part_id_aggregate,
        art_evidence_policy: request.art_evidence_policy.clone(),
        art_evidence_policy_sha256: request.art_evidence_policy_sha256.clone(),
        quality_status: PRODUCTION_WEAPON_FORM_ART_EVIDENCE_QUALITY_STATUS.into(),
        runtime_write_performed: true,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256: request_sha256.into(),
        input_sha256: request.input_sha256.clone(),
        receipt_object_sha256: String::new(),
        canonical_sha256: String::new(),
        created_at: form.created_at,
    })
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
    record: &ProductionWeaponFormArtEvidenceRecord,
    replayed: bool,
    schema: &str,
    runtime_write: bool,
    restart_hash_verified: Option<bool>,
) -> Result<Value, RuntimeError> {
    let mut value = serde_json::json!({"schema_version":schema,"art_evidence":serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?,"replayed":replayed,"runtime_write":runtime_write,"production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false});
    if let Some(verified) = restart_hash_verified {
        value["restart_hash_verified"] = Value::Bool(verified);
    }
    Ok(value)
}

/// Run the existing fixed Render Worker triangle/source attribution against
/// one durable FormArt view.  All renderer inputs are derived from Runtime
/// state: the candidate artifact, ProductionCameraLock rig, ReferenceCanvas
/// view, SilhouetteTarget and the immutable FormArt receipt.  The operation is
/// deliberately a transient projection: it returns a canonical hash for the
/// caller's evidence sheet but creates no CAS object, SQLite row, candidate,
/// stage edge or confirmation.  A single candidate has no baseline owner
/// delta, so the owner-change mask is explicitly all false and reported as
/// not applicable rather than being accepted from the caller.
fn build_raster_source_attribution_diagnostic(
    runtime: &Runtime,
    request: &RasterSourceAttributionDiagnosticGetRequest,
    expected_record: &ProductionWeaponFormArtEvidenceRecord,
) -> Result<Value, RuntimeError> {
    if request.form_art_evidence_object_sha256 != expected_record.receipt_object_sha256
        || request.form_art_evidence_canonical_sha256 != expected_record.canonical_sha256
        || request.session_id != expected_record.session_id
        || request.project_id != expected_record.project_id
        || request.candidate_id != expected_record.candidate_id
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_OUTER_RECORD_BINDING_MISMATCH",
        ));
    }
    let art = super::agentic_session::read_form_art_evidence(
        runtime,
        &request.form_art_evidence_object_sha256,
    )?;
    if art.session_id != request.session_id
        || art.project_id != request.project_id
        || art.candidate_id != request.candidate_id
        || art.candidate_state_sha256 != request.candidate_state_sha256
        || art.artifact_id != request.artifact_id
        || art.artifact_sha256 != request.artifact_sha256
        || art.canonical_sha256.is_empty()
        || art.canonical_sha256 != request.form_art_evidence_canonical_sha256
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_EVIDENCE_BINDING_MISMATCH",
        ));
    }
    let view = art
        .views
        .iter()
        .find(|view| view.view_kind == request.view_kind && view.view_id == request.view_id)
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_VIEW_BINDING_MISSING"))?;
    if view.candidate_state_sha256 != request.candidate_state_sha256
        || view.artifact_id != request.artifact_id
        || view.artifact_sha256 != request.artifact_sha256
        || view.reference_id != request.reference_id
        || view.reference_sha256 != request.reference_sha256
        || view.camera_hash != request.camera_hash
        || view.camera_canonical_sha256 != request.camera_canonical_sha256
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_VIEW_BINDING_MISMATCH",
        ));
    }

    let candidate = runtime
        .candidate(&request.candidate_id)?
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CANDIDATE_MISSING"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.candidate_state_sha256
        || candidate.prepared_object_id.as_deref() != Some(request.artifact_id.as_str())
        || candidate.prepared_object_sha256.as_deref() != Some(request.artifact_sha256.as_str())
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CANDIDATE_BINDING_MISMATCH",
        ));
    }
    let readback = runtime.artifact_readback(&request.artifact_sha256, &request.candidate_id)?;
    let artifact_readback_canonical_sha256 = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_READBACK_INVALID"))?;
    let glb = runtime.cas_read_bounded(&request.artifact_sha256, 64 * 1024 * 1024)?;

    let canvas = read_json(
        runtime,
        &art.reference_canvas_object_sha256,
        "ReferenceCanvas",
    )?;
    if canonical_document(&canvas, "ReferenceCanvas@1", "ReferenceCanvas")?
        != art.reference_canvas_canonical_sha256
        || canvas.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CANVAS_BINDING_MISMATCH",
        ));
    }
    let spec = read_json(runtime, &art.design_spec_object_sha256, "DesignSpec")?;
    if canonical_document(&spec, "DesignSpec@1", "DesignSpec")? != art.design_spec_canonical_sha256
        || spec.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || spec.get("reference_canvas_sha256").and_then(Value::as_str)
            != Some(art.reference_canvas_object_sha256.as_str())
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_DESIGN_SPEC_BINDING_MISMATCH",
        ));
    }
    let canvas_view = canvas_views(&canvas)?
        .get(&request.view_id)
        .cloned()
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CANVAS_VIEW_MISSING"))?;
    if canvas_view.get("kind").and_then(Value::as_str) != Some(request.view_kind.as_str())
        || canvas_view.get("reference_id").and_then(Value::as_str)
            != Some(request.reference_id.as_str())
        || canvas_view.get("reference_sha256").and_then(Value::as_str)
            != Some(request.reference_sha256.as_str())
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_REFERENCE_BINDING_MISMATCH",
        ));
    }
    let target_hash = canvas_view
        .get("target_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_TARGET_MISSING"))?;
    if target_hash != view.target_object_sha256 {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_TARGET_HASH_MISMATCH",
        ));
    }
    let target = runtime.read_silhouette_target(target_hash)?;
    if target.get("reference_id").and_then(Value::as_str) != Some(request.reference_id.as_str())
        || target.get("reference_sha256").and_then(Value::as_str)
            != Some(request.reference_sha256.as_str())
        || target.get("canonical_sha256").and_then(Value::as_str)
            != Some(view.target_canonical_sha256.as_str())
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_TARGET_BINDING_MISMATCH",
        ));
    }
    let target_mask_source_sha256 = target
        .get("mask_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_TARGET_MASK_INVALID"))?;
    let view_spec = canvas_view
        .get("view_spec")
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_VIEW_SPEC_MISSING"))?;
    let crop = super::reference_view_crop(view_spec)?;
    let rotation_degrees = super::reference_view_rotation_degrees(view_spec)?;
    let projected_target_mask = super::project_reference_mask_to_view(
        &runtime.target_mask(target_hash, &target)?.mask,
        view_spec,
        true,
    )?;
    let visual_structure = target
        .get("visual_structure")
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_STRUCTURE_MISSING"))?;
    let structure_id = match request.view_kind.as_str() {
        "left" => "left.open-stock-void",
        "right" => "right.open-stock-void",
        "rear-three-quarter" => "rear3q.open-stock-void",
        _ => unreachable!("request parser closes raster attribution view kinds"),
    };
    let (reviewed_region_canonical_sha256, reviewed_region_mask, expected_void_mask, _) =
        reviewed_region_owner_audit_masks_with_rotation(
            visual_structure,
            &projected_target_mask,
            crop,
            rotation_degrees,
            structure_id,
        )?;
    let projected_target_mask_sha256 = registration_preflight_mask_sha256(&projected_target_mask)?;
    let reviewed_region_mask_sha256 = registration_preflight_mask_sha256(&reviewed_region_mask)?;
    let expected_void_mask_sha256 = registration_preflight_mask_sha256(&expected_void_mask)?;

    let form_evidence_view = read_json(
        runtime,
        &view.form_evidence_view_receipt_object_sha256,
        "FormEvidence view receipt",
    )?;
    if form_evidence_view
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(view.form_evidence_view_receipt_canonical_sha256.as_str())
        || form_evidence_view
            .get("candidate_id")
            .and_then(Value::as_str)
            != Some(request.candidate_id.as_str())
        || form_evidence_view.get("view_id").and_then(Value::as_str)
            != Some(request.view_id.as_str())
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_FORM_EVIDENCE_VIEW_BINDING_MISMATCH",
        ));
    }
    let render_set_object_sha256 = form_evidence_view
        .get("render_set_object_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_RENDER_SET_MISSING"))?;
    let render_set_canonical_sha256 = form_evidence_view
        .get("render_set_canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_RENDER_SET_INVALID"))?;
    let render_set = read_json(runtime, render_set_object_sha256, "RenderSet")?;
    super::validate_render_set_v2_output(&render_set)?;
    if canonical_document(&render_set, "RenderSet@2", "RenderSet")? != render_set_canonical_sha256
        || render_set.get("candidate_id").and_then(Value::as_str)
            != Some(request.candidate_id.as_str())
        || render_set.get("artifact_sha256").and_then(Value::as_str)
            != Some(request.artifact_sha256.as_str())
        || render_set.get("reference_id").and_then(Value::as_str)
            != Some(request.reference_id.as_str())
        || render_set.get("view_id").and_then(Value::as_str) != Some(request.view_id.as_str())
        || render_set.get("camera_hash").and_then(Value::as_str)
            != Some(request.camera_hash.as_str())
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_RENDER_SET_BINDING_MISMATCH",
        ));
    }
    let camera_object_sha256 = render_set
        .get("camera_object_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CAMERA_OBJECT_MISSING"))?;

    let lock = runtime
        .store
        .get_production_camera_lock(&art.camera_lock_id)
        .map_err(RuntimeError::from)?
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CAMERA_LOCK_MISSING"))?;
    super::agentic_session::validate_production_camera_lock_record(runtime, &lock)?;
    if lock.session_id != request.session_id
        || lock.project_id != request.project_id
        || lock.candidate_id != request.candidate_id
        || lock.candidate_state_sha256 != request.candidate_state_sha256
        || lock.artifact_id != request.artifact_id
        || lock.artifact_sha256 != request.artifact_sha256
        || lock.reference_canvas_object_sha256 != art.reference_canvas_object_sha256
        || lock.reference_canvas_canonical_sha256 != art.reference_canvas_canonical_sha256
        || lock.design_spec_object_sha256 != art.design_spec_object_sha256
        || lock.design_spec_canonical_sha256 != art.design_spec_canonical_sha256
        || lock.camera_rig_object_sha256 != art.camera_rig_object_sha256
        || lock.camera_rig_canonical_sha256 != art.camera_rig_canonical_sha256
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CAMERA_LOCK_BINDING_MISMATCH",
        ));
    }
    let subject_rig = read_json(runtime, &lock.camera_rig_object_sha256, "CameraRig")?;
    let registered_rig = super::agentic_session::materialize_production_camera_lock_registered_rig(
        runtime,
        &lock.project_id,
        &lock.candidate_id,
        &lock.candidate_state_sha256,
        &lock.artifact_id,
        &lock.artifact_sha256,
        &subject_rig,
        &lock.camera_rig_object_sha256,
    )?;
    let rig_view = registered_rig
        .get("renderer_views")
        .and_then(Value::as_array)
        .and_then(|views| {
            views.iter().find(|view| {
                view.get("kind").and_then(Value::as_str) == Some(request.view_kind.as_str())
            })
        })
        .cloned()
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CAMERA_VIEW_MISSING"))?;
    if rig_view
        .get("registered_camera_hash")
        .and_then(Value::as_str)
        != Some(request.camera_hash.as_str())
        || rig_view
            .get("registered_camera")
            .and_then(|camera| camera.get("canonical_sha256"))
            .and_then(Value::as_str)
            != Some(request.camera_canonical_sha256.as_str())
    {
        return Err(invalid(
            "RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CAMERA_BINDING_MISMATCH",
        ));
    }
    let camera = rig_view
        .get("registered_camera")
        .cloned()
        .ok_or_else(|| invalid("RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_CAMERA_MISSING"))?;

    // There is intentionally no caller-provided baseline or mask.  This
    // single-candidate action reports source ownership in the reviewed region
    // and expected void; owner-delta attribution is explicitly not applicable.
    let owner_changed_mask = vec![false; 512 * 512];
    let attribution = render_glb_raster_source_attribution_diagnostic(
        &glb,
        &camera,
        &reviewed_region_mask,
        &expected_void_mask,
        &owner_changed_mask,
        &["rear-stock"],
    )?;
    let mut result = serde_json::json!({
        "schema_version": RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_GET_RESULT_SCHEMA_VERSION,
        "diagnostic_id": request.diagnostic_id.clone(),
        "session_id": request.session_id.clone(),
        "project_id": request.project_id.clone(),
        "candidate_id": request.candidate_id.clone(),
        "candidate_state_sha256": request.candidate_state_sha256.clone(),
        "artifact_id": request.artifact_id.clone(),
        "artifact_sha256": request.artifact_sha256.clone(),
        "artifact_readback_canonical_sha256": artifact_readback_canonical_sha256,
        "reference_id": request.reference_id.clone(),
        "reference_sha256": request.reference_sha256.clone(),
        "view_kind": request.view_kind.clone(),
        "view_id": request.view_id.clone(),
        "camera_hash": request.camera_hash.clone(),
        "camera_canonical_sha256": request.camera_canonical_sha256.clone(),
        "camera_object_sha256": camera_object_sha256,
        "camera_rig_object_sha256": art.camera_rig_object_sha256.clone(),
        "camera_rig_canonical_sha256": art.camera_rig_canonical_sha256.clone(),
        "form_art_evidence_object_sha256": request.form_art_evidence_object_sha256.clone(),
        "form_art_evidence_canonical_sha256": request.form_art_evidence_canonical_sha256.clone(),
        "form_art_view_receipt_object_sha256": view.receipt_object_sha256.clone(),
        "form_art_view_receipt_canonical_sha256": view.canonical_sha256.clone(),
        "target_object_sha256": view.target_object_sha256.clone(),
        "target_canonical_sha256": view.target_canonical_sha256.clone(),
        "target_mask_source_sha256": target_mask_source_sha256,
        "projected_target_mask_sha256": projected_target_mask_sha256,
        "reviewed_region_mask_sha256": reviewed_region_mask_sha256,
        "expected_void_mask_sha256": expected_void_mask_sha256,
        "render_set_object_sha256": render_set_object_sha256,
        "render_set_canonical_sha256": render_set_canonical_sha256,
        "reviewed_region_structure_id": structure_id,
        "reviewed_region_canonical_sha256": reviewed_region_canonical_sha256,
        "owner_changed_status": "NOT_APPLICABLE_SINGLE_CANDIDATE",
        "policy": RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_POLICY,
        "diagnostic": serde_json::to_value(attribution)
            .map_err(|error| invalid(error.to_string()))?,
        "quality_status": "NOT_PROVEN",
        "runtime_write": false,
        "worker_started": true,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "input_sha256": request.input_sha256.clone(),
        "diagnostic_canonical_sha256": ""
    });
    let canonical = canonical_json_hash(&result);
    result["diagnostic_canonical_sha256"] = Value::String(canonical);
    Ok(result)
}

impl Runtime {
    pub fn production_weapon_form_art_evidence_prepare(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let (request, request_sha256) = parse_prepare(&value)?;
        let mut record = record_from_source(self, &request, &request_sha256)?;
        let reservation = self.store.begin_cas_reservation();
        let mut objects = Vec::new();
        for view in &mut record.views {
            let mut receipt =
                serde_json::to_value(&*view).map_err(|error| invalid(error.to_string()))?;
            receipt["receipt_object_sha256"] = Value::String(String::new());
            let bytes =
                canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
            if bytes.len() > MAX_JSON_BYTES {
                release(self, &reservation, &objects, true);
                return Err(invalid("form art evidence view receipt exceeds 1 MiB"));
            }
            let object = match self.store.put_object_reserved(
                &reservation,
                &bytes,
                None,
                JSON_MIME,
                PRODUCTION_WEAPON_FORM_ART_EVIDENCE_VIEW_RECEIPT_KIND,
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
        record.canonical_sha256 = canonical_json_hash(&normalized_record(&record)?);
        let mut receipt =
            serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
        receipt["receipt_object_sha256"] = Value::String(String::new());
        let bytes = canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
        if bytes.len() > MAX_JSON_BYTES {
            release(self, &reservation, &objects, true);
            return Err(invalid("form art evidence parent receipt exceeds 1 MiB"));
        }
        let parent = match self.store.put_object_reserved(
            &reservation,
            &bytes,
            None,
            JSON_MIME,
            PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PARENT_RECEIPT_KIND,
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
        let child_records = objects[..objects.len() - 1]
            .iter()
            .map(|object| object.record.clone())
            .collect::<Vec<_>>();
        let parent_record = objects.last().expect("parent receipt");
        match self
            .store
            .record_production_weapon_form_art_evidence_with_replay(
                &record,
                &child_records,
                &parent_record.record,
            ) {
            Ok((stored, replayed)) => {
                release(self, &reservation, &objects, false);
                result_value(
                    &stored,
                    replayed,
                    PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PREPARE_RESULT_SCHEMA_VERSION,
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

    pub fn production_weapon_form_art_evidence_get(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_get(&value)?;
        let raster_diagnostic_request = value
            .get("raster_source_attribution_diagnostic")
            .map(parse_raster_source_attribution_diagnostic_get)
            .transpose()?
            .map(|(request, _request_sha256)| request);
        let record = self
            .store
            .get_production_weapon_form_art_evidence(&request.art_evidence_id)
            .map_err(RuntimeError::from)?
            .ok_or_else(|| invalid("form art evidence is unavailable"))?;
        if record.session_id != request.session_id
            || record.project_id != request.project_id
            || record.candidate_id != request.candidate_id
        {
            return Err(invalid("form art evidence get scope differs"));
        }
        for view in &record.views {
            if canonical_json_hash(&normalized_view(view)?) != view.canonical_sha256 {
                return Err(invalid("form art evidence view canonical differs"));
            }
            let bytes = self.cas_read(&view.receipt_object_sha256)?;
            let mut expected =
                serde_json::to_value(view).map_err(|error| invalid(error.to_string()))?;
            expected["receipt_object_sha256"] = Value::String(String::new());
            if bytes
                != canonical_json_bytes(&expected).map_err(|error| invalid(error.to_string()))?
            {
                return Err(invalid("form art evidence view receipt bytes differ"));
            }
        }
        if canonical_json_hash(&normalized_record(&record)?) != record.canonical_sha256 {
            return Err(invalid("form art evidence parent canonical differs"));
        }
        let bytes = self.cas_read(&record.receipt_object_sha256)?;
        let mut expected =
            serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
        expected["receipt_object_sha256"] = Value::String(String::new());
        if bytes != canonical_json_bytes(&expected).map_err(|error| invalid(error.to_string()))? {
            return Err(invalid("form art evidence parent receipt bytes differ"));
        }
        let prepare = serde_json::json!({"schema_version":PRODUCTION_WEAPON_FORM_ART_EVIDENCE_PREPARE_REQUEST_SCHEMA_VERSION,"art_evidence_id":record.art_evidence_id,"session_id":record.session_id,"project_id":record.project_id,"candidate_id":record.candidate_id,"form_evidence_object_sha256":record.form_evidence_object_sha256,"form_evidence_canonical_sha256":record.form_evidence_canonical_sha256,"art_evidence_policy":record.art_evidence_policy,"art_evidence_policy_sha256":record.art_evidence_policy_sha256,"input_sha256":record.input_sha256,"idempotency_key":record.art_evidence_id});
        let (parsed, request_sha256) = parse_prepare(&prepare)?;
        if request_sha256 != record.request_sha256 {
            return Err(invalid(
                "form art evidence request hash differs after restart",
            ));
        }
        let mut rebuilt = record_from_source(self, &parsed, &request_sha256)?;
        if rebuilt.views.len() != record.views.len() {
            return Err(invalid("form art evidence restart view count differs"));
        }
        // The source projection deterministically rebuilds each child before
        // its durable receipt object exists.  The Store get above has already
        // verified those immutable child receipts, so restore only their CAS
        // identities before comparing the complete parent projection.  The
        // per-view canonical hashes and every source/AOV binding remain part
        // of the strict comparison.
        for (rebuilt_view, stored_view) in rebuilt.views.iter_mut().zip(record.views.iter()) {
            rebuilt_view.receipt_object_sha256 = stored_view.receipt_object_sha256.clone();
        }
        if normalized_record(&rebuilt)? != normalized_record(&record)? {
            return Err(invalid("form art evidence restart projection differs"));
        }
        let mut result = result_value(
            &record,
            true,
            PRODUCTION_WEAPON_FORM_ART_EVIDENCE_GET_RESULT_SCHEMA_VERSION,
            false,
            Some(true),
        )?;
        if let Some(diagnostic_request) = raster_diagnostic_request {
            result["raster_source_attribution_diagnostic"] =
                build_raster_source_attribution_diagnostic(self, &diagnostic_request, &record)?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_source_attribution_get_request_is_closed_and_hash_bound() {
        let hash = "a".repeat(64);
        let mut request = serde_json::json!({
            "schema_version": RASTER_SOURCE_ATTRIBUTION_DIAGNOSTIC_GET_REQUEST_SCHEMA_VERSION,
            "diagnostic_id": "diagnostic-1",
            "session_id": "session-1",
            "project_id": "project-1",
            "candidate_id": "candidate-1",
            "candidate_state_sha256": hash,
            "artifact_id": "artifact-1",
            "artifact_sha256": "b".repeat(64),
            "reference_id": "reference-left",
            "reference_sha256": "c".repeat(64),
            "form_art_evidence_object_sha256": "d".repeat(64),
            "form_art_evidence_canonical_sha256": "e".repeat(64),
            "view_kind": "left",
            "view_id": "view-left",
            "camera_hash": "f".repeat(64),
            "camera_canonical_sha256": "1".repeat(64),
            "input_sha256": ""
        });
        let mut preimage = request.as_object().expect("request object").clone();
        preimage.remove("input_sha256");
        request["input_sha256"] = Value::String(canonical_json_hash(&Value::Object(preimage)));
        assert!(parse_raster_source_attribution_diagnostic_get(&request).is_ok());

        let mut retargeted = request.clone();
        retargeted["candidate_id"] = Value::String("candidate-foreign".to_owned());
        assert!(parse_raster_source_attribution_diagnostic_get(&retargeted).is_err());
        let mut unknown = request;
        unknown["camera"] = serde_json::json!({"forbidden":"caller-provided"});
        assert!(parse_raster_source_attribution_diagnostic_get(&unknown).is_err());
    }

    #[test]
    fn reviewed_reference_points_apply_authored_rotation_before_aov_comparison() {
        let rotated =
            project_point_to_view([0.2, 0.3], [0.0, 0.0, 1.0, 1.0], 180.0, "reviewed point")
                .expect("180-degree reference registration");
        assert!((rotated[0] - 0.8).abs() < 1e-9);
        assert!((rotated[1] - 0.7).abs() < 1e-9);

        let cropped = project_point_to_view(
            [0.3, 0.4],
            [0.2, 0.3, 0.2, 0.2],
            0.0,
            "cropped reviewed point",
        )
        .expect("zero-rotation crop projection");
        assert!((cropped[0] - 0.5).abs() < 1e-9);
        assert!((cropped[1] - 0.5).abs() < 1e-9);
    }

    fn test_part_color(index: usize) -> [u8; 4] {
        [
            (((index.wrapping_mul(97) + 53) % 220 + 20) as u8),
            (((index.wrapping_mul(53) + 79) % 170 + 40) as u8),
            (((index.wrapping_mul(31) + 131) % 120 + 80) as u8),
            255,
        ]
    }

    fn test_part_png(pixels: &[(usize, usize, usize)]) -> Vec<u8> {
        use image::ImageEncoder;
        let mut image = image::RgbaImage::from_pixel(512, 512, image::Rgba([8, 12, 18, 255]));
        for (x, y, palette_index) in pixels {
            image.put_pixel(
                *x as u32,
                *y as u32,
                image::Rgba(test_part_color(*palette_index)),
            );
        }
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(image.as_raw(), 512, 512, image::ExtendedColorType::Rgba8)
            .expect("test Part-ID PNG");
        png
    }

    #[test]
    fn closed_get_rejects_unknown_fields() {
        let value = serde_json::json!({"schema_version":PRODUCTION_WEAPON_FORM_ART_EVIDENCE_GET_REQUEST_SCHEMA_VERSION,"art_evidence_id":"art-1","session_id":"session-1","project_id":"project-1","candidate_id":"candidate-1","raw_png_bytes":"forbidden"});
        assert!(parse_get(&value).is_err());
    }
    #[test]
    fn metrics_are_deterministic_and_bounded() {
        let mut left = vec![false; 512 * 512];
        let mut right = vec![false; 512 * 512];
        for y in 20..80 {
            for x in 30..90 {
                left[y * 512 + x] = true;
                right[y * 512 + x] = true;
            }
        }
        assert_eq!(metrics(&left, &right), metrics(&left, &right));
        let (iou, f1, ratio, centroid, nonempty) = metrics(&left, &right);
        assert_eq!(
            (iou, f1, ratio, centroid, nonempty),
            (1000, 1000, 1000, 0, true)
        );
    }

    #[test]
    fn owner_mask_hamming_audit_separates_expected_void_and_boundary_bands() {
        let mut region = vec![false; 512 * 512];
        let mut expected_void = vec![false; 512 * 512];
        for y in 100..200 {
            for x in 100..200 {
                region[y * 512 + x] = true;
            }
        }
        for y in 120..160 {
            for x in 120..160 {
                expected_void[y * 512 + x] = true;
            }
        }
        let mut baseline_owner = vec![false; 512 * 512];
        let mut trial_owner = vec![false; 512 * 512];
        baseline_owner[110 * 512 + 110] = true;
        trial_owner[110 * 512 + 110] = true;
        // One changed pixel is on the reviewed expected-void boundary; the
        // other is outside the reviewed region entirely.
        trial_owner[120 * 512 + 120] = true;
        trial_owner[250 * 512 + 250] = true;
        let diagnostic =
            owner_mask_hamming_diagnostic(&baseline_owner, &trial_owner, &region, &expected_void)
                .expect("owner-mask Hamming diagnostic");
        assert_eq!(diagnostic.owner_mask_changed_pixel_count, 2);
        assert_eq!(
            diagnostic.owner_mask_changed_inside_expected_void_pixel_count,
            1
        );
        assert_eq!(
            diagnostic.owner_mask_changed_inside_region_outside_expected_void_pixel_count,
            0
        );
        assert_eq!(
            diagnostic.owner_mask_changed_outside_reviewed_region_pixel_count,
            1
        );
        assert_eq!(diagnostic.changed_expected_boundary_pixel_count, 1);
        assert!(diagnostic.changed_expected_boundary_band_r1_pixel_count >= 1);
        assert!(diagnostic.changed_expected_boundary_band_r2_pixel_count >= 1);
        assert!(diagnostic.changed_expected_boundary_band_r4_pixel_count >= 1);
        assert_eq!(diagnostic.changed_bbox_px, Some([120, 120, 250, 250]));
        assert_eq!(
            diagnostic.classification,
            "OWNER_PIXEL_CHANGE_INSIDE_EXPECTED_VOID"
        );
    }

    #[test]
    fn owner_mask_hamming_audit_rejects_non_512_masks() {
        let valid = vec![false; 512 * 512];
        let invalid_mask = vec![false; 512 * 512 - 1];
        let error = owner_mask_hamming_diagnostic(&valid, &invalid_mask, &valid, &valid)
            .expect_err("invalid owner-mask dimensions must fail closed");
        assert!(error.to_string().contains("masks must be 512x512"));
    }

    #[test]
    fn exact_part_id_mask_uses_semantic_part_and_rejects_source_node_or_palette_escape() {
        let part_ids = vec![
            "receiver-main".to_owned(),
            "receiver-upper".to_owned(),
            "receiver-lower".to_owned(),
            "rear-stock".to_owned(),
        ];
        let pixels = (100..110)
            .flat_map(|y| (120..130).map(move |x| (x, y, 3)))
            .collect::<Vec<_>>();
        let png = test_part_png(&pixels);
        let mask = exact_part_id_mask(&png, &part_ids, "rear-stock")
            .expect("exact semantic rear-stock mask");
        assert_eq!(mask.iter().filter(|pixel| **pixel).count(), 100);
        assert!(exact_part_id_mask(&png, &part_ids, "rear-stock-lower-beam").is_err());

        let escaped = test_part_png(&[(120, 100, 4)]);
        assert!(exact_part_id_mask(&escaped, &part_ids, "rear-stock").is_err());
    }

    fn binding_thresholds() -> ReviewedRegionPartBindingThresholds {
        ReviewedRegionPartBindingThresholds {
            min_owner_region_pixels: Some(1),
            min_boundary_adjacency_pixels: Some(1),
            max_owner_expected_void_overlap_milli: Some(0),
        }
    }

    fn binding_structure(contour_points: [[f64; 2]; 4]) -> Value {
        serde_json::json!({
            "review_status":"user_confirmed",
            "regions":[{
                "structure_id":"left.open-stock-void",
                "mask_operation":"subtract",
                "contour_points":contour_points
            }]
        })
    }

    fn binding_part_ids() -> Vec<String> {
        vec![
            "receiver-main".to_owned(),
            "receiver-upper".to_owned(),
            "receiver-lower".to_owned(),
            "rear-stock".to_owned(),
        ]
    }

    fn binding_pixels_from_mask(
        mask: &[bool],
        map: impl Fn(usize, usize) -> (usize, usize),
    ) -> Vec<(usize, usize, usize)> {
        mask.iter()
            .enumerate()
            .filter_map(|(index, pixel)| {
                (*pixel).then(|| {
                    let x = index % 512;
                    let y = index / 512;
                    let (mapped_x, mapped_y) = map(x, y);
                    (mapped_x, mapped_y, 3)
                })
            })
            .collect()
    }

    fn binding_target_and_owner_masks(
        structure: &Value,
        hole_x: (usize, usize),
        hole_y: (usize, usize),
    ) -> (Vec<bool>, Vec<bool>) {
        let empty_target = vec![false; 512 * 512];
        let (_, region_mask, _) = reviewed_region_expected_void_mask(
            structure,
            &empty_target,
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
        )
        .expect("reviewed region mask");
        let mut target_mask = region_mask.clone();
        for y in hole_y.0..hole_y.1 {
            for x in hole_x.0..hole_x.1 {
                target_mask[y * 512 + x] = false;
            }
        }
        let (_, _, expected_void) = reviewed_region_expected_void_mask(
            structure,
            &target_mask,
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
        )
        .expect("reviewed expected void");
        let mut owner_mask = vec![false; 512 * 512];
        let x0 = hole_x.0.saturating_sub(2);
        let x1 = (hole_x.1 + 2).min(512);
        let y0 = hole_y.0.saturating_sub(2);
        let y1 = (hole_y.1 + 2).min(512);
        for y in y0..hole_y.0 {
            for x in x0..x1 {
                let index = y * 512 + x;
                owner_mask[index] = region_mask[index] && !expected_void[index];
            }
        }
        for y in hole_y.1..y1 {
            for x in x0..x1 {
                let index = y * 512 + x;
                owner_mask[index] = region_mask[index] && !expected_void[index];
            }
        }
        for y in hole_y.0..hole_y.1 {
            for x in x0..hole_x.0 {
                let index = y * 512 + x;
                owner_mask[index] = region_mask[index] && !expected_void[index];
            }
            for x in hole_x.1..x1 {
                let index = y * 512 + x;
                owner_mask[index] = region_mask[index] && !expected_void[index];
            }
        }
        (target_mask, owner_mask)
    }

    fn preflight_structure(structure_id: &str) -> Value {
        serde_json::json!({
            "review_status":"user_confirmed",
            "regions":[{
                "structure_id":structure_id,
                "mask_operation":"subtract",
                "contour_points":[[0.15,0.2],[0.45,0.2],[0.45,0.4],[0.15,0.4]]
            }]
        })
    }

    fn preflight_masks(
        structure: &Value,
        structure_id: &str,
        hole_x: (usize, usize),
        hole_y: (usize, usize),
    ) -> (Vec<bool>, Vec<bool>) {
        let empty_target = vec![false; 512 * 512];
        let (_, region_mask, _) = reviewed_region_expected_void_mask_with_rotation(
            structure,
            &empty_target,
            [0.0, 0.0, 1.0, 1.0],
            0.0,
            structure_id,
        )
        .expect("preflight region mask");
        let mut target_mask = region_mask.clone();
        for y in hole_y.0..hole_y.1 {
            for x in hole_x.0..hole_x.1 {
                target_mask[y * 512 + x] = false;
            }
        }
        let (_, _, expected_void) = reviewed_region_expected_void_mask_with_rotation(
            structure,
            &target_mask,
            [0.0, 0.0, 1.0, 1.0],
            0.0,
            structure_id,
        )
        .expect("preflight expected void");
        let mut owner_mask = vec![false; 512 * 512];
        let x0 = hole_x.0.saturating_sub(2);
        let x1 = (hole_x.1 + 2).min(512);
        let y0 = hole_y.0.saturating_sub(2);
        let y1 = (hole_y.1 + 2).min(512);
        for y in y0..hole_y.0 {
            for x in x0..x1 {
                let index = y * 512 + x;
                owner_mask[index] = region_mask[index] && !expected_void[index];
            }
        }
        for y in hole_y.1..y1 {
            for x in x0..x1 {
                let index = y * 512 + x;
                owner_mask[index] = region_mask[index] && !expected_void[index];
            }
        }
        for y in hole_y.0..hole_y.1 {
            for x in x0..hole_x.0 {
                let index = y * 512 + x;
                owner_mask[index] = region_mask[index] && !expected_void[index];
            }
            for x in hole_x.1..x1 {
                let index = y * 512 + x;
                owner_mask[index] = region_mask[index] && !expected_void[index];
            }
        }
        (target_mask, owner_mask)
    }

    fn preflight_binding(view_kind: &str) -> RegistrationPreflightBinding {
        let hash = |label: &str| sha256_hex(label.as_bytes());
        RegistrationPreflightBinding {
            view_kind: view_kind.to_owned(),
            structure_id: match view_kind {
                "left" => "left.open-stock-void",
                "right" => "right.open-stock-void",
                "rear-three-quarter" => "rear3q.open-stock-void",
                _ => panic!("closed preflight view set"),
            }
            .to_owned(),
            crop_canonical_sha256: hash("crop-v1"),
            target_object_sha256: hash("target-object-v1"),
            target_mask_source_sha256: hash("target-mask-source-v1"),
            part_id_pass_sha256: hash(&format!("{view_kind}-part-id-v1")),
            registered_camera_hash: hash("registered-camera-v1"),
            worker_binding_canonical_sha256: hash("worker-binding-v1"),
            lineage_sha256: hash(&format!("lineage-{view_kind}-v1")),
        }
    }

    fn preflight_fixture() -> (
        Vec<RegistrationPreflightProjection>,
        Vec<RegistrationPreflightProjection>,
    ) {
        let thresholds = ReviewedRegionPartBindingThresholds {
            min_owner_region_pixels: Some(1),
            min_boundary_adjacency_pixels: Some(1),
            max_owner_expected_void_overlap_milli: Some(0),
        };
        let mut current = Vec::new();
        let mut negative = Vec::new();
        for view_kind in ["left", "right", "rear-three-quarter"] {
            let structure_id = match view_kind {
                "left" => "left.open-stock-void",
                "right" => "right.open-stock-void",
                "rear-three-quarter" => "rear3q.open-stock-void",
                _ => unreachable!(),
            };
            let structure = preflight_structure(structure_id);
            let (base_target, base_owner) =
                preflight_masks(&structure, structure_id, (120, 150), (130, 165));
            let (target_mask, mut owner_mask) = if view_kind == "rear-three-quarter" {
                let mut rotated_owner = vec![false; 512 * 512];
                for (index, pixel) in base_owner.iter().enumerate() {
                    if *pixel {
                        let x = index % 512;
                        let y = index / 512;
                        rotated_owner[(511 - y) * 512 + (511 - x)] = true;
                    }
                }
                let mut target = base_target.clone();
                let (_, region_180, _) = reviewed_region_expected_void_mask_with_rotation(
                    &structure,
                    &vec![false; 512 * 512],
                    [0.0, 0.0, 1.0, 1.0],
                    180.0,
                    structure_id,
                )
                .expect("preflight rear rotated region");
                for (index, pixel) in region_180.iter().enumerate() {
                    if *pixel {
                        target[index] = true;
                    }
                }
                for y in 130..165 {
                    for x in 120..150 {
                        target[(511 - y) * 512 + (511 - x)] = false;
                    }
                }
                (target, rotated_owner)
            } else {
                (base_target, base_owner)
            };
            // Keep registration useful in the presence of the known owner
            // intrusion: this pixel must make the independent formal gate
            // false without changing the closed-transform identity winner.
            if view_kind == "left" {
                owner_mask[145 * 512 + 135] = true;
            }
            let binding = preflight_binding(view_kind);
            let rotation = if view_kind == "rear-three-quarter" {
                180.0
            } else {
                0.0
            };
            let current_view = registration_preflight_projection_with_rotation(
                binding.clone(),
                &structure,
                &target_mask,
                &owner_mask,
                [0.0, 0.0, 1.0, 1.0],
                rotation,
                &thresholds,
            )
            .expect("current preflight projection");
            let negative_view = registration_preflight_projection_with_rotation(
                binding,
                &structure,
                &target_mask,
                &owner_mask,
                [0.0, 0.0, 1.0, 1.0],
                if view_kind == "rear-three-quarter" {
                    0.0
                } else {
                    rotation
                },
                &thresholds,
            )
            .expect("negative preflight projection");
            current.push(current_view);
            negative.push(negative_view);
        }
        (current, negative)
    }

    #[test]
    fn registration_preflight_hash_only_positive_and_negative_rear_fixture() {
        let (current, negative) = preflight_fixture();
        let gate = registration_preflight_gate(&current, &negative)
            .expect("current unique identity and negative rear fixture is worse");
        assert!(gate.pass);
        assert_eq!(gate.status, "CURRENT_UNIQUE_REGISTRATION_ELIGIBLE");
        assert!(gate.current_unique_registration_identity_all_views);
        assert!(!gate.formal_owner_gate_all_views);
        assert!(!gate.negative_fixture_improves);
        assert!(!gate.negative_fixture_tie);
        assert!(gate.lineage_stable);
        assert!(!gate.promotable);
        assert_eq!(gate.quality_status, "NOT_PROVEN");
        assert_eq!(gate.current_view_count, 3);
        for view in &current {
            assert!(view.registration_identity_eligible);
            assert!(view.unique_ranked_registration_identity);
            assert!(!view.registration_rank_tie);
            assert!(!view.promotable);
            assert_eq!(view.quality_status, "NOT_PROVEN");
            assert!(!view.projected_region_mask_sha256.is_empty());
            assert!(!view.expected_void_mask_sha256.is_empty());
            assert!(view.expected_void_bbox_px[2] >= view.expected_void_bbox_px[0]);
            assert!(view.expected_void_bbox_px[3] >= view.expected_void_bbox_px[1]);
        }
        assert!(!current[0].formal_owner_gate_eligible);
        assert!(!current[0].strict_owner_zero_intrusion);
        assert!(current[0].identity_owner_expected_void_overlap_pixel_count > 0);
        assert_ne!(
            current[2].expected_void_mask_sha256,
            negative[2].expected_void_mask_sha256
        );
        assert_eq!(current[0], negative[0]);
        assert_eq!(current[1], negative[1]);
    }

    #[test]
    fn registration_preflight_fails_closed_on_negative_score_improvement_and_tie() {
        let (current, mut negative) = preflight_fixture();
        negative[2].identity_owner_boundary_adjacency_pixel_count = current[2]
            .identity_owner_boundary_adjacency_pixel_count
            .saturating_add(1);
        let gate = registration_preflight_gate(&current, &negative)
            .expect("a real negative improvement is a blocked gate, not a panic");
        assert!(!gate.pass);
        assert!(gate.negative_fixture_improves);
        assert_eq!(gate.status, "BLOCKED_NEGATIVE_FIXTURE_IMPROVES");

        let (current, mut negative) = preflight_fixture();
        negative[2].identity_owner_boundary_adjacency_pixel_count =
            current[2].identity_owner_boundary_adjacency_pixel_count;
        negative[2].owner_region_pixel_count = current[2].owner_region_pixel_count;
        negative[2].identity_owner_expected_void_overlap_pixel_count =
            current[2].identity_owner_expected_void_overlap_pixel_count;
        negative[2].identity_bbox_edge_error_px = current[2].identity_bbox_edge_error_px;
        negative[2].identity_centroid_error_px = current[2].identity_centroid_error_px;
        let tie = registration_preflight_gate(&current, &negative)
            .expect_err("equal current/negative score must fail closed");
        assert!(tie.to_string().contains("identity score"));

        let (mut current, mut negative) = preflight_fixture();
        current[0].registration_rank_tie = true;
        negative[0].registration_rank_tie = true;
        let registration_tie = registration_preflight_gate(&current, &negative)
            .expect_err("registration transform tie must fail closed");
        assert!(registration_tie
            .to_string()
            .contains("registration identity ranking tie"));
    }

    #[test]
    fn registration_preflight_fails_closed_on_lineage_drift_and_bad_mask_hash_input() {
        let (current, mut negative) = preflight_fixture();
        negative[2].binding.lineage_sha256 = sha256_hex(b"drifted-lineage");
        let drift = registration_preflight_gate(&current, &negative)
            .expect_err("lineage drift must fail closed");
        assert!(drift
            .to_string()
            .contains("lineage or hash binding drifted"));
        let bad = registration_preflight_mask_sha256(&[true, false])
            .expect_err("non-512 mask hash must fail closed");
        assert!(bad.to_string().contains("mask must be 512x512"));
    }

    #[test]
    fn reviewed_region_part_binding_selects_unique_identity_transform() {
        let structure = binding_structure([[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]]);
        let (target_mask, owner_mask) =
            binding_target_and_owner_masks(&structure, (145, 170), (145, 170));
        let calibration = calibrate_reviewed_region_part_binding(
            &structure,
            &target_mask,
            &test_part_png(&binding_pixels_from_mask(&owner_mask, |x, y| (x, y))),
            &binding_part_ids(),
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            None,
            &binding_thresholds(),
        )
        .expect("unique identity binding");
        assert_eq!(calibration.status, "ephemeral-transform-candidate");
        assert!(!calibration.promotable);
        assert_eq!(
            calibration.selected_transform,
            ReviewedRegionPartBindingTransform::Identity
        );
        assert_eq!(calibration.candidates.len(), 4);
        assert!(
            calibration
                .candidates
                .iter()
                .filter(|candidate| candidate.passes_thresholds)
                .count()
                == 1
        );
        let mut authored = calibration;
        authored.authored_transform = Some(ReviewedRegionPartBindingTransform::Identity);
        authored.status = "bound-diagnostic-only".to_owned();
        let strict = strict_reviewed_region_part_binding_assessment(&authored, true)
            .expect("direct registered-camera identity binding passes strict diagnostic");
        assert_eq!(strict.status, "PASS_BOUND_DIAGNOSTIC");
        assert_eq!(strict.policy, STRICT_OWNER_VOID_POLICY);
        assert_eq!(strict.owner_expected_void_overlap_pixel_count, 0);
        assert!(!strict.promotable);
        assert_eq!(strict.quality_status, "NOT_PROVEN");
        assert_eq!(strict.depth_status, "UNKNOWN");
    }

    #[test]
    fn reviewed_region_part_binding_records_signed_owner_void_offset() {
        let structure = binding_structure([[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]]);
        let (target_mask, owner_mask) =
            binding_target_and_owner_masks(&structure, (145, 170), (145, 170));
        let calibration = calibrate_reviewed_region_part_binding(
            &structure,
            &target_mask,
            &test_part_png(&binding_pixels_from_mask(&owner_mask, |x, y| (x + 5, y))),
            &binding_part_ids(),
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            None,
            &ReviewedRegionPartBindingThresholds {
                min_owner_region_pixels: Some(1),
                min_boundary_adjacency_pixels: Some(1),
                max_owner_expected_void_overlap_milli: Some(1000),
            },
        )
        .expect("shifted owner binding diagnostic");
        let identity = calibration
            .candidates
            .iter()
            .find(|candidate| candidate.transform == ReviewedRegionPartBindingTransform::Identity)
            .expect("identity diagnostic");
        assert_eq!(identity.expected_void_bbox_px, [145, 145, 169, 169]);
        assert_eq!(identity.owner_bbox_px, [148, 143, 176, 171]);
        assert_eq!(
            identity.owner_minus_expected_void_bbox_edge_delta_px,
            [3, -2, 7, 2]
        );
        assert_eq!(
            identity.owner_minus_expected_void_centroid_delta_milli_px,
            [5000, 0]
        );
    }

    #[test]
    fn strict_owner_void_requires_unique_ranked_identity_not_single_discovery_eligible() {
        let structure = binding_structure([[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]]);
        let (target_mask, owner_mask) =
            binding_target_and_owner_masks(&structure, (145, 170), (145, 170));
        let mut calibration = calibrate_reviewed_region_part_binding(
            &structure,
            &target_mask,
            &test_part_png(&binding_pixels_from_mask(&owner_mask, |x, y| (x, y))),
            &binding_part_ids(),
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            Some(ReviewedRegionPartBindingTransform::Identity),
            &binding_thresholds(),
        )
        .expect("unique ranked identity binding");
        let identity = calibration
            .candidates
            .iter()
            .find(|candidate| candidate.transform == ReviewedRegionPartBindingTransform::Identity)
            .expect("identity candidate")
            .clone();
        let secondary = calibration
            .candidates
            .iter_mut()
            .find(|candidate| {
                candidate.transform == ReviewedRegionPartBindingTransform::HorizontalFlip
            })
            .expect("secondary transform candidate");
        secondary.passes_thresholds = true;
        secondary.owner_boundary_adjacency_pixel_count = identity
            .owner_boundary_adjacency_pixel_count
            .saturating_sub(1);
        secondary.owner_boundary_adjacency_milli =
            identity.owner_boundary_adjacency_milli.saturating_sub(1);
        let strict = strict_reviewed_region_part_binding_assessment(&calibration, true)
            .expect("lower-ranked discovery candidate must not obscure direct identity acceptance");
        assert_eq!(strict.owner_expected_void_overlap_pixel_count, 0);

        let tied = calibration
            .candidates
            .iter_mut()
            .find(|candidate| {
                candidate.transform == ReviewedRegionPartBindingTransform::HorizontalFlip
            })
            .expect("secondary transform candidate");
        *tied = ReviewedRegionPartBindingCandidate {
            transform: ReviewedRegionPartBindingTransform::HorizontalFlip,
            ..identity
        };
        let tie = strict_reviewed_region_part_binding_assessment(&calibration, true)
            .expect_err("a tied identity ranking must remain fail-closed");
        assert!(tie
            .to_string()
            .contains("identity binding is not the unique ranked candidate"));
    }

    #[test]
    fn reviewed_region_part_binding_selects_only_explicit_horizontal_flip() {
        let structure = binding_structure([[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]]);
        let (target_mask, owner_mask) =
            binding_target_and_owner_masks(&structure, (145, 170), (145, 170));
        let calibration = calibrate_reviewed_region_part_binding(
            &structure,
            &target_mask,
            &test_part_png(&binding_pixels_from_mask(&owner_mask, |x, y| (511 - x, y))),
            &binding_part_ids(),
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            Some(ReviewedRegionPartBindingTransform::HorizontalFlip),
            &binding_thresholds(),
        )
        .expect("explicit horizontal-flip binding");
        assert_eq!(calibration.status, "bound-diagnostic-only");
        assert!(!calibration.promotable);
        assert_eq!(
            calibration.selected_transform,
            ReviewedRegionPartBindingTransform::HorizontalFlip
        );
        assert_eq!(
            calibration
                .candidates
                .iter()
                .filter(|candidate| candidate.passes_thresholds)
                .count(),
            1
        );
        let mismatch = calibrate_reviewed_region_part_binding(
            &structure,
            &target_mask,
            &test_part_png(&binding_pixels_from_mask(&owner_mask, |x, y| (511 - x, y))),
            &binding_part_ids(),
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            Some(ReviewedRegionPartBindingTransform::Identity),
            &binding_thresholds(),
        )
        .expect_err("authored transform must agree with unique winner");
        assert!(mismatch
            .to_string()
            .contains("authored transform differs from unique winner"));
        let strict_error = strict_reviewed_region_part_binding_assessment(&calibration, true)
            .expect_err("post-render horizontal flip cannot pass strict owner-void acceptance");
        assert!(strict_error
            .to_string()
            .contains("not produced directly in the registered camera frame"));
    }

    #[test]
    fn strict_owner_void_rejects_missing_lineage_and_one_raw_intrusion_pixel() {
        let structure = binding_structure([[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]]);
        let (target_mask, owner_mask) =
            binding_target_and_owner_masks(&structure, (145, 170), (145, 170));
        let mut pixels = binding_pixels_from_mask(&owner_mask, |x, y| (x, y));
        pixels.push((150, 150, 3));
        let calibration = calibrate_reviewed_region_part_binding(
            &structure,
            &target_mask,
            &test_part_png(&pixels),
            &binding_part_ids(),
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            Some(ReviewedRegionPartBindingTransform::Identity),
            &ReviewedRegionPartBindingThresholds {
                min_owner_region_pixels: Some(1),
                min_boundary_adjacency_pixels: Some(1),
                max_owner_expected_void_overlap_milli: Some(1000),
            },
        )
        .expect("discovery calibration intentionally permits one intrusion pixel");
        assert_eq!(
            calibration
                .candidates
                .iter()
                .find(|candidate| {
                    candidate.transform == ReviewedRegionPartBindingTransform::Identity
                })
                .map(|candidate| candidate.owner_expected_void_overlap_pixel_count),
            Some(1)
        );
        let missing = strict_reviewed_region_part_binding_assessment(&calibration, false)
            .expect_err("missing registered camera lineage must fail closed");
        assert!(missing
            .to_string()
            .contains("registered camera lineage is unavailable"));
        let intrusion = strict_reviewed_region_part_binding_assessment(&calibration, true)
            .expect_err("one raw intrusion pixel must fail even when milli rounds to zero");
        assert!(intrusion
            .to_string()
            .contains("strict owner-void thresholds are not met"));
    }

    #[test]
    fn reviewed_region_part_binding_blocks_tie_and_missing_threshold() {
        let symmetric = binding_structure([[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75]]);
        let (target_mask, owner_mask) =
            binding_target_and_owner_masks(&symmetric, (236, 276), (236, 276));
        let png = test_part_png(&binding_pixels_from_mask(&owner_mask, |x, y| (x, y)));
        let tie_error = calibrate_reviewed_region_part_binding(
            &symmetric,
            &target_mask,
            &png,
            &binding_part_ids(),
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            None,
            &binding_thresholds(),
        )
        .expect_err("symmetric transforms must be blocked as a tie");
        assert!(tie_error
            .to_string()
            .contains("transform winner is not unique"));

        let threshold_error = calibrate_reviewed_region_part_binding(
            &symmetric,
            &target_mask,
            &png,
            &binding_part_ids(),
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            None,
            &ReviewedRegionPartBindingThresholds {
                min_owner_region_pixels: None,
                ..binding_thresholds()
            },
        )
        .expect_err("missing threshold must fail closed");
        assert!(threshold_error
            .to_string()
            .contains("owner-region threshold is unavailable"));
    }

    #[test]
    fn reviewed_region_part_binding_blocks_noncanonical_crop_and_unreviewed_structure() {
        let structure = binding_structure([[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]]);
        let target_mask = vec![false; 512 * 512];
        let part_ids = binding_part_ids();
        let png = test_part_png(&[(110, 110, 3)]);
        let crop_error = calibrate_reviewed_region_part_binding(
            &structure,
            &target_mask,
            &png,
            &part_ids,
            [0.0, 0.0, 0.0, 1.0],
            "left.open-stock-void",
            None,
            &binding_thresholds(),
        )
        .expect_err("zero-width crop must be blocked");
        assert!(crop_error
            .to_string()
            .contains("canonical crop mapping is invalid"));

        let mut unreviewed = structure;
        unreviewed["review_status"] = Value::String("unreviewed".to_owned());
        let review_error = calibrate_reviewed_region_part_binding(
            &unreviewed,
            &target_mask,
            &png,
            &part_ids,
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            None,
            &binding_thresholds(),
        )
        .expect_err("unreviewed structure must be blocked");
        assert!(review_error
            .to_string()
            .contains("visual structure is not reviewed"));
    }

    #[test]
    fn part_owned_void_changes_when_silhouette_row_is_identical_but_owner_mask_moves() {
        let structure = serde_json::json!({"regions":[{
            "structure_id":"left.open-stock-void",
            "mask_operation":"subtract",
            "contour_points":[[0.2,0.2],[0.8,0.2],[0.8,0.8],[0.2,0.8]]
        }]});
        let target_mask = vec![false; 512 * 512];
        let part_ids = vec![
            "receiver-main".to_owned(),
            "receiver-upper".to_owned(),
            "receiver-lower".to_owned(),
            "rear-stock".to_owned(),
        ];
        let boundary_pixels = (102..108)
            .flat_map(|y| (102..108).map(move |x| (x, y, 3)))
            .collect::<Vec<_>>();
        let interior_pixels = (250..256)
            .flat_map(|y| (250..256).map(move |x| (x, y, 3)))
            .collect::<Vec<_>>();
        let baseline = part_owned_negative_space_diagnostic(
            &structure,
            &target_mask,
            &test_part_png(&boundary_pixels),
            &part_ids,
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            "rear-stock",
        )
        .expect("baseline Part-owned diagnostic");
        let trial = part_owned_negative_space_diagnostic(
            &structure,
            &target_mask,
            &test_part_png(&interior_pixels),
            &part_ids,
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            "rear-stock",
        )
        .expect("trial Part-owned diagnostic");
        assert_eq!(baseline.status, "bound");
        assert_eq!(trial.status, "unbound");
        assert_ne!(
            baseline.owner_boundary_adjacency_pixel_count,
            trial.owner_boundary_adjacency_pixel_count
        );
        assert_eq!(
            baseline.expected_region_canonical_sha256,
            trial.expected_region_canonical_sha256
        );
    }

    #[test]
    fn source_part_id_attribution_reports_split_metrics_without_promotion() {
        let structure = binding_structure([[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]]);
        let (target_mask, owner_mask) =
            binding_target_and_owner_masks(&structure, (145, 170), (145, 170));
        let source_pixels = binding_pixels_from_mask(&owner_mask, |x, y| (x + 5, y))
            .into_iter()
            .map(|(x, y, _)| (x, y, 0))
            .collect::<Vec<_>>();
        let diagnostic = source_part_id_attribution_diagnostic_with_rotation(
            &structure,
            &target_mask,
            &test_part_png(&source_pixels),
            &[
                "rear-stock-upper-diagnostic".to_owned(),
                "rear-stock-lower-diagnostic".to_owned(),
            ],
            [0.0, 0.0, 1.0, 1.0],
            0.0,
            "left.open-stock-void",
            "rear-stock",
            "rear-stock-upper-diagnostic",
        )
        .expect("source attribution diagnostic");

        assert!(diagnostic.diagnostic_only);
        assert!(!diagnostic.promotable);
        assert_eq!(diagnostic.status, "diagnostic-only");
        assert_eq!(diagnostic.semantic_owner_part_id, "rear-stock");
        assert_eq!(
            diagnostic.source_diagnostic_part_id,
            "rear-stock-upper-diagnostic"
        );
        assert_eq!(diagnostic.source_expected_void_overlap_pixel_count, 50);
        assert_eq!(diagnostic.source_expected_void_overlap_milli, 80);
        assert!(diagnostic.source_boundary_adjacency_pixel_count > 0);
        assert!(diagnostic.source_boundary_adjacency_milli > 0);
        assert_eq!(
            diagnostic.source_minus_expected_void_bbox_edge_delta_px,
            [3, -2, 7, 2]
        );
        assert_eq!(
            diagnostic.source_minus_expected_void_centroid_delta_milli_px,
            [5000, 0]
        );
    }

    #[test]
    fn part_owned_void_fails_closed_for_duplicate_region_empty_target_or_missing_owner() {
        let region = serde_json::json!({
            "structure_id":"left.open-stock-void",
            "mask_operation":"subtract",
            "contour_points":[[0.2,0.2],[0.8,0.2],[0.8,0.8],[0.2,0.8]]
        });
        let duplicate = serde_json::json!({"regions":[region.clone(),region]});
        let target_mask = vec![false; 512 * 512];
        let part_ids = vec!["rear-stock".to_owned()];
        let png = test_part_png(&[(102, 102, 0)]);
        assert!(part_owned_negative_space_diagnostic(
            &duplicate,
            &target_mask,
            &png,
            &part_ids,
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            "rear-stock",
        )
        .is_err());

        let structure = serde_json::json!({"regions":[{
            "structure_id":"left.open-stock-void",
            "mask_operation":"subtract",
            "contour_points":[[0.2,0.2],[0.8,0.2],[0.8,0.8],[0.2,0.8]]
        }]});
        let target_filled = vec![true; 512 * 512];
        assert!(part_owned_negative_space_diagnostic(
            &structure,
            &target_filled,
            &png,
            &part_ids,
            [0.0, 0.0, 1.0, 1.0],
            "left.open-stock-void",
            "rear-stock",
        )
        .is_err());
        assert!(exact_part_id_mask(&test_part_png(&[]), &part_ids, "rear-stock").is_err());
    }

    #[test]
    fn source_part_id_attribution_fails_closed_for_owner_source_or_empty_mask() {
        let structure = binding_structure([[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]]);
        let (target_mask, owner_mask) =
            binding_target_and_owner_masks(&structure, (145, 170), (145, 170));
        let part_ids = vec![
            "rear-stock-upper-diagnostic".to_owned(),
            "rear-stock-lower-diagnostic".to_owned(),
        ];
        let source_png = test_part_png(&binding_pixels_from_mask(&owner_mask, |x, y| (x, y)));

        let owner_error = source_part_id_attribution_diagnostic_with_rotation(
            &structure,
            &target_mask,
            &source_png,
            &part_ids,
            [0.0, 0.0, 1.0, 1.0],
            0.0,
            "left.open-stock-void",
            "rear-cap",
            "rear-stock-upper-diagnostic",
        )
        .expect_err("non-rear-stock semantic owner must fail closed");
        assert!(owner_error
            .to_string()
            .contains("semantic owner must be rear-stock"));

        let source_error = source_part_id_attribution_diagnostic_with_rotation(
            &structure,
            &target_mask,
            &source_png,
            &part_ids,
            [0.0, 0.0, 1.0, 1.0],
            0.0,
            "left.open-stock-void",
            "rear-stock",
            "rear-stock-lower-beam",
        )
        .expect_err("production source node must not masquerade as diagnostic Part-ID");
        assert!(source_error
            .to_string()
            .contains("unknown source diagnostic Part-ID"));

        let empty_error = source_part_id_attribution_diagnostic_with_rotation(
            &structure,
            &target_mask,
            &test_part_png(&[]),
            &part_ids,
            [0.0, 0.0, 1.0, 1.0],
            0.0,
            "left.open-stock-void",
            "rear-stock",
            "rear-stock-upper-diagnostic",
        )
        .expect_err("empty source mask must fail closed");
        assert!(empty_error
            .to_string()
            .contains("owner Part is not visible"));
    }

    #[test]
    fn metrics_centroid_uses_contract_normalized_scale() {
        let mut left = vec![false; 512 * 512];
        let mut shift_15 = vec![false; 512 * 512];
        let mut shift_16 = vec![false; 512 * 512];
        for y in 20..80 {
            for x in 30..90 {
                left[y * 512 + x] = true;
                shift_15[y * 512 + x + 15] = true;
                shift_16[y * 512 + x + 16] = true;
            }
        }
        let centroid_15 = metrics(&left, &shift_15).3;
        let centroid_16 = metrics(&left, &shift_16).3;
        assert!(
            (2929..=2930).contains(&centroid_15),
            "15 px shift must be about 0.03: {centroid_15}"
        );
        assert!(
            (3124..=3125).contains(&centroid_16),
            "16 px shift must exceed 0.03: {centroid_16}"
        );
        assert!(centroid_15 < 3000 && centroid_16 > 3000);
    }
    #[test]
    fn line_trace_derives_order_and_crossings_from_edge() {
        let mut edge = vec![false; 512 * 512];
        for x in 51..=460 {
            edge[256 * 512 + x] = true;
        }
        let structure = serde_json::json!({"line_flows":[{"line_flow_id":"ridge-1","visibility":"observed","points":[[0.1,0.5],[0.9,0.5]]}]});
        let (status, rows) = line_rows(Some(&structure), true, &edge, [0.0, 0.0, 1.0, 1.0])
            .expect("bounded line trace");
        assert_eq!(status, "observed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].direction_order_milli, 1000);
        assert_eq!(rows[0].duplicate_crossing_count, 0);
        assert!(rows[0].symmetric_chamfer_milli <= 300);
        assert!(rows[0].max_deviation_milli <= 300);
    }
    #[test]
    fn form_art_points_project_from_reference_crop_and_reject_escape() {
        let mut edge = vec![false; 512 * 512];
        for x in 51..=460 {
            edge[256 * 512 + x] = true;
        }
        let crop = [0.25, 0.25, 0.5, 0.5];
        let structure = serde_json::json!({"line_flows":[{
            "line_flow_id":"crop-ridge",
            "visibility":"observed",
            "points":[[0.3,0.5],[0.7,0.5]]
        }]});
        let (status, rows) = line_rows(Some(&structure), true, &edge, crop)
            .expect("reference-space line projects into crop space");
        assert_eq!(status, "observed");
        assert_eq!(rows[0].coverage_milli, 1000);

        let escaped = serde_json::json!({"line_flows":[{
            "line_flow_id":"escaped-ridge",
            "visibility":"observed",
            "points":[[0.2,0.5],[0.7,0.5]]
        }]});
        let error = line_rows(Some(&escaped), true, &edge, crop)
            .expect_err("line outside its bound crop must fail closed");
        assert!(error
            .to_string()
            .contains("FORM_ART_EVIDENCE_VIEW_CROP_MISMATCH"));
    }
    #[test]
    fn line_sampling_budget_rejects_128_max_length_flows_deterministically() {
        let mut flows = Vec::new();
        for index in 0..MAX_LINE_FLOWS_PER_VIEW {
            let points = (0..MAX_LINE_POINTS_PER_FLOW)
                .map(|point| serde_json::json!([point as f64 / 255.0, index as f64 / 128.0]))
                .collect::<Vec<_>>();
            flows.push(serde_json::json!({
                "line_flow_id": format!("flow-{index}"),
                "visibility": "observed",
                "points": points
            }));
        }
        let structure = serde_json::json!({"line_flows": flows});
        let edge = vec![false; 512 * 512];
        assert!(line_rows(Some(&structure), true, &edge, [0.0, 0.0, 1.0, 1.0]).is_err());
        assert!(line_rows(Some(&structure), true, &edge, [0.0, 0.0, 1.0, 1.0]).is_err());
    }
    #[test]
    fn negative_space_budget_rejects_more_than_32_regions() {
        let regions = (0..(MAX_NEGATIVE_REGIONS_PER_VIEW + 1))
            .map(|index| {
                serde_json::json!({
                    "structure_id": format!("void-{index}"),
                    "mask_operation": "subtract",
                    "contour_points": [[0.1, 0.1], [0.2, 0.1], [0.2, 0.2]]
                })
            })
            .collect::<Vec<_>>();
        let structure = serde_json::json!({"regions": regions});
        let mask = vec![false; 512 * 512];
        assert!(negative_rows(Some(&structure), true, &mask, &mask, [0.0, 0.0, 1.0, 1.0]).is_err());
    }

    #[test]
    fn per_view_expected_inventory_excludes_occluded_parts_but_preserves_order() {
        let artifact_ids = vec![
            "receiver".to_owned(),
            "grip".to_owned(),
            "rear-stock".to_owned(),
        ];
        let expected = per_view_expected_part_ids(
            &artifact_ids,
            &["rear-stock".to_owned(), "receiver".to_owned()],
            "left",
        )
        .expect("valid visible source inventory");
        assert_eq!(expected, vec!["receiver", "rear-stock"]);
        let (expected_count, observed_count, unexpected_count, coverage) =
            validate_visible_part_inventory(&expected, &expected, "left")
                .expect("occluded grip is not expected in left view");
        assert_eq!(
            (expected_count, observed_count, unexpected_count, coverage),
            (2, 2, 0, 1000)
        );
    }

    #[test]
    fn per_view_expected_inventory_rejects_missing_or_unexpected_visible_parts() {
        let expected = vec!["receiver".to_owned(), "grip".to_owned()];
        let missing = validate_visible_part_inventory(&expected, &["receiver".to_owned()], "front");
        assert!(missing.is_err(), "a missing visible Part must fail closed");
        let unexpected = validate_visible_part_inventory(
            &["receiver".to_owned()],
            &["receiver".to_owned(), "rear-stock".to_owned()],
            "front",
        );
        assert!(
            unexpected.is_err(),
            "an unexpected visible Part must fail closed"
        );
    }

    #[test]
    fn per_view_expected_inventory_rejects_duplicate_or_foreign_source_ids() {
        let artifact_ids = vec!["receiver".to_owned(), "grip".to_owned()];
        assert!(per_view_expected_part_ids(
            &artifact_ids,
            &["receiver".to_owned(), "receiver".to_owned()],
            "left"
        )
        .is_err());
        assert!(per_view_expected_part_ids(
            &artifact_ids,
            &["receiver".to_owned(), "foreign".to_owned()],
            "left"
        )
        .is_err());
    }

    #[test]
    fn empty_per_view_expected_inventory_is_vacuously_complete() {
        let empty = Vec::<String>::new();
        assert_eq!(
            validate_visible_part_inventory(&empty, &empty, "back")
                .expect("fully occluded view is valid"),
            (0, 0, 0, 1000)
        );
    }

    #[test]
    fn raster_source_attribution_ranks_exact_changed_source_without_writes() {
        let mut triangle_ids_le = vec![u8::MAX; 512 * 512 * 4];
        triangle_ids_le[0..4].copy_from_slice(&0_u32.to_le_bytes());
        triangle_ids_le[4..8].copy_from_slice(&1_u32.to_le_bytes());
        triangle_ids_le[8..12].copy_from_slice(&1_u32.to_le_bytes());
        let attribution = super::super::render_worker::RenderWorkerRasterAttribution {
            width: 512,
            height: 512,
            raster_width: 1024,
            raster_height: 1024,
            triangle_ids_le,
            triangle_ids_sha256: "b".repeat(64),
            source_table_sha256: "c".repeat(64),
            sources: vec![
                super::super::render_worker::RasterAttributionSource {
                    triangle_index: 0,
                    mesh_index: 0,
                    primitive_index: 0,
                    triangle_index_in_primitive: 0,
                    semantic_part_id: "receiver".to_owned(),
                    source_node_id: "receiver-shell".to_owned(),
                    lineage_source_node_ids: vec!["receiver-shell".to_owned()],
                    material_zone_id: "receiver-metal".to_owned(),
                },
                super::super::render_worker::RasterAttributionSource {
                    triangle_index: 1,
                    mesh_index: 1,
                    primitive_index: 0,
                    triangle_index_in_primitive: 0,
                    semantic_part_id: "rear-stock".to_owned(),
                    source_node_id: "rear-stock-upper".to_owned(),
                    lineage_source_node_ids: vec![
                        "rear-stock".to_owned(),
                        "rear-stock-upper".to_owned(),
                    ],
                    material_zone_id: "stock-polymer".to_owned(),
                },
            ],
            build_cohort_sha256: Some("a".repeat(64)),
        };
        let mut reviewed = vec![false; 512 * 512];
        let mut expected_void = reviewed.clone();
        let mut changed = reviewed.clone();
        reviewed[0] = true;
        reviewed[1] = true;
        reviewed[2] = true;
        expected_void[1] = true;
        expected_void[2] = true;
        changed[2] = true;

        let diagnostic = raster_source_attribution_diagnostic(
            &attribution,
            &reviewed,
            &expected_void,
            &changed,
            &[],
        )
        .expect("closed transient attribution");
        assert_eq!(diagnostic.visible_pixel_count, 3);
        assert_eq!(diagnostic.background_pixel_count, 512 * 512 - 3);
        assert_eq!(diagnostic.sources[0].source_node_id, "rear-stock-upper");
        assert_eq!(diagnostic.sources[0].material_zone_ids, ["stock-polymer"]);
        assert_eq!(diagnostic.sources[0].mesh_indices, [1]);
        assert_eq!(diagnostic.sources[0].primitive_indices, [0]);
        assert_eq!(diagnostic.sources[0].owner_changed_pixel_count, 1);
        assert_eq!(diagnostic.sources[0].expected_void_pixel_count, 2);
        assert_eq!(
            diagnostic.highest_impact_source.source_node_id,
            "rear-stock-upper"
        );
        assert_eq!(diagnostic.highest_impact_basis, "owner-changed-pixels");
        assert_eq!(diagnostic.highest_impact_pixel_count, 1);
        assert_eq!(
            diagnostic.repair_target_status,
            "UNIQUE_HIGHEST_IMPACT_SOURCE_OBSERVED"
        );
        assert!(diagnostic.diagnostic_only);
        assert!(!diagnostic.promotable);
        assert!(!diagnostic.runtime_write);
        assert!(!diagnostic.production_stage_advanced);
    }
}
