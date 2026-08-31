//! Runtime-owned source binding for the knife authoring path.
//!
//! This boundary is intentionally boring: it re-reads every upstream object,
//! proves the candidate-owned AuthoringMesh@2 revision carries the same source
//! binding, derives one closed `KnifeSourceBinding@1` payload, and only then
//! asks Store to commit the staged CAS object.  In particular, this module
//! does not create an identity-lineage V1 row, High mesh, visual evidence,
//! approval, version, or export.

use super::{
    authoring_mesh_v2::{self, AuthoringMeshV2Revision},
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, Runtime,
    RuntimeError,
};
use forgecad_contracts::AuthoringMeshRevision;
use forgecad_store::{
    CasObject, KnifeSourceBindingCasBundle, KnifeSourceBindingCommit,
    KnifeSourceBindingStoreRecord, AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
    KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND, KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY,
    KNIFE_SOURCE_BINDING_BINDING_STATUS, KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY,
    KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY, KNIFE_SOURCE_BINDING_JSON_MIME,
    KNIFE_SOURCE_BINDING_MAX_JSON_BYTES, KNIFE_SOURCE_BINDING_OBJECT_KIND,
    KNIFE_SOURCE_BINDING_POLICY, KNIFE_SOURCE_BINDING_RECORD_SCHEMA_VERSION,
    KNIFE_SOURCE_BINDING_SCHEMA_VERSION,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub(crate) const PREPARE_SCHEMA: &str = "KnifeSourceBindingPrepareRequest@1";
pub(crate) const GET_SCHEMA: &str = "KnifeSourceBindingGetRequest@1";
pub(crate) const RESULT_SCHEMA: &str = "KnifeSourceBindingResult@1";
pub(crate) const PREPARE_OPERATION: &str = "knife_source_binding_prepare";
pub(crate) const GET_OPERATION: &str = "knife_source_binding_get";
pub(crate) const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
pub(crate) const REQUEST_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
pub(crate) const MAX_RESPONSE_BYTES: u64 = 1_048_576;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "source_binding",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "source_binding_id",
    "source_binding_sha256",
    "source_binding_object_sha256",
    "intent_bundle_id",
    "intent_bundle_sha256",
    "intent_bundle_object_sha256",
    "brief_id",
    "brief_sha256",
    "brief_object_sha256",
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "quality_contract_id",
    "quality_contract_sha256",
    "quality_contract_object_sha256",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "authoring_mesh_id",
    "authoring_mesh_lineage_id",
    "authoring_mesh_revision_id",
    "authoring_mesh_revision_index",
    "authoring_mesh_revision_sha256",
    "authoring_mesh_revision_object_sha256",
    "authoring_mesh_identity_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];
const SOURCE_BINDING_FIELDS: &[&str] = &[
    "schema_version",
    "source_binding_id",
    "project_id",
    "binding_status",
    "authoring_eligibility",
    "intent_bundle_id",
    "intent_bundle_sha256",
    "intent_bundle_object_sha256",
    "brief_id",
    "brief_sha256",
    "brief_object_sha256",
    "reference_id",
    "reference_object_sha256",
    "reference_evidence_sha256",
    "quality_contract_id",
    "quality_contract_sha256",
    "quality_contract_object_sha256",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "authoring_mesh_id",
    "authoring_mesh_lineage_id",
    "authoring_mesh_revision_id",
    "authoring_mesh_revision_index",
    "authoring_mesh_revision_sha256",
    "authoring_mesh_revision_object_sha256",
    "authoring_mesh_identity_sha256",
    "downstream_binding_requirements",
    "high_mesh_created",
    "high_stage_unlocked",
    "production_stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "visual_status",
    "human_status",
    "engine_status",
    "binding_policy",
    "canonicalization_policy",
    "canonical_sha256",
    "created_at",
];
const DOWNSTREAM_FIELDS: &[&str] = &[
    "curve_modifier_graph",
    "curve_evaluated_mesh",
    "high",
    "render",
];

#[derive(Debug, Clone)]
struct Selectors {
    source_binding_id: String,
    source_binding_sha256: String,
    project_id: String,
    intent_bundle_id: String,
    intent_bundle_sha256: String,
    intent_bundle_object_sha256: String,
    brief_id: String,
    brief_sha256: String,
    brief_object_sha256: String,
    reference_id: String,
    reference_object_sha256: String,
    reference_evidence_sha256: String,
    quality_contract_id: String,
    quality_contract_sha256: String,
    quality_contract_object_sha256: String,
    source_candidate_id: String,
    source_candidate_state_sha256: String,
    authoring_mesh_id: String,
    authoring_mesh_lineage_id: String,
    authoring_mesh_revision_id: String,
    authoring_mesh_revision_index: u64,
    authoring_mesh_revision_sha256: String,
    authoring_mesh_revision_object_sha256: String,
    authoring_mesh_identity_sha256: String,
    created_at: String,
}

#[cfg(test)]
pub(crate) fn test_source_binding_request(runtime: &Runtime, suffix: &str) -> Value {
    tests::source_binding_request(runtime, suffix)
}

/// Test-only source fixture with one additional preserved Part.  The High
/// bridge is intentionally single-revision-part, but its upstream materialized
/// candidate must prove that non-target Parts survive the source-bound splice.
/// Keep this constructor behind `cfg(test)` so production code cannot acquire
/// a second fixture-specific authoring path.
#[cfg(test)]
pub(crate) fn test_multi_part_source_binding_request(runtime: &Runtime, suffix: &str) -> Value {
    tests::source_binding_request_multi_part(runtime, suffix)
}

#[derive(Debug, Clone)]
struct Upstream {
    selectors: Selectors,
}

#[derive(Debug, Clone)]
struct SourceTruth {
    candidate_state_sha256: String,
    revision: AuthoringMeshRevision,
    durable: forgecad_store::AuthoringMeshV2DurableRecord,
    identity_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("KNIFE_SOURCE_BINDING_INVALID: {}", message.into()))
}

fn mismatch(code: &str, message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("{code}: {}", message.into()))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(invalid(format!(
            "{context} fields differ from the closed contract"
        )));
    }
    Ok(object)
}

fn text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    context: &str,
) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{context}.{field} must be a string")))
}

fn id(object: &Map<String, Value>, field: &str, context: &str) -> Result<String, RuntimeError> {
    let value = text(object, field, context)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!(
            "{context}.{field} must be an opaque identifier"
        )));
    }
    Ok(value.to_owned())
}

fn hash(object: &Map<String, Value>, field: &str, context: &str) -> Result<String, RuntimeError> {
    let value = text(object, field, context)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{context}.{field} must be a SHA-256")));
    }
    Ok(value.to_owned())
}

fn bool_exact(
    object: &Map<String, Value>,
    field: &str,
    expected: bool,
    context: &str,
) -> Result<(), RuntimeError> {
    if object.get(field).and_then(Value::as_bool) != Some(expected) {
        return Err(invalid(format!("{context}.{field} must be {expected}")));
    }
    Ok(())
}

fn timestamp(value: &str, context: &str) -> Result<String, RuntimeError> {
    // Existing Runtime records use epoch seconds while the public fixtures
    // use RFC3339.  Both are bounded, path-free, and immutable metadata.
    if value.is_empty()
        || value.len() > 128
        || value.contains('/')
        || value.contains('\\')
        || value.chars().any(|c| c.is_control())
        || (!value.chars().all(|c| c.is_ascii_digit())
            && !(value.ends_with('Z') && value.contains('T')))
    {
        return Err(invalid(format!("{context} must be a bounded timestamp")));
    }
    Ok(value.to_owned())
}

fn request_header(
    object: &Map<String, Value>,
    schema: &str,
    operation: &str,
    context: &str,
) -> Result<(), RuntimeError> {
    if text(object, "schema_version", context)? != schema
        || text(object, "operation", context)? != operation
        || text(object, "writer_policy", context)? != WRITER_POLICY
        || text(object, "canonicalization_policy", context)? != REQUEST_CANONICALIZATION
        || object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES)
    {
        return Err(invalid(format!(
            "{context} policy or response budget differs"
        )));
    }
    bool_exact(object, "runtime_write_performed", false, context)
}

fn request_hash(
    request: &Value,
    object: &Map<String, Value>,
    context: &str,
) -> Result<String, RuntimeError> {
    let supplied = hash(object, "input_sha256", context)?;
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_INPUT_HASH_MISMATCH",
            format!("{context}.input_sha256 differs from its closed request"),
        ));
    }
    Ok(supplied)
}

fn source_selectors(value: &Value) -> Result<Selectors, RuntimeError> {
    let object = exact_object(value, SOURCE_BINDING_FIELDS, "source_binding")?;
    if text(object, "schema_version", "source_binding")? != KNIFE_SOURCE_BINDING_SCHEMA_VERSION
        || text(object, "binding_status", "source_binding")? != KNIFE_SOURCE_BINDING_BINDING_STATUS
        || text(object, "authoring_eligibility", "source_binding")?
            != KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY
        || text(object, "quality_status", "source_binding")? != "source_binding_only"
        || text(object, "visual_status", "source_binding")? != "NOT_RUN"
        || text(object, "human_status", "source_binding")? != "NOT_RUN"
        || text(object, "engine_status", "source_binding")? != "NOT_RUN"
        || text(object, "binding_policy", "source_binding")? != KNIFE_SOURCE_BINDING_POLICY
        || text(object, "canonicalization_policy", "source_binding")?
            != KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY
    {
        return Err(invalid("source_binding status or policy differs"));
    }
    for field in [
        "high_mesh_created",
        "high_stage_unlocked",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        bool_exact(object, field, false, "source_binding")?;
    }
    let downstream = exact_object(
        object
            .get("downstream_binding_requirements")
            .ok_or_else(|| invalid("source_binding.downstream_binding_requirements missing"))?,
        DOWNSTREAM_FIELDS,
        "source_binding.downstream_binding_requirements",
    )?;
    for field in DOWNSTREAM_FIELDS {
        if text(downstream, field, "downstream_binding_requirements")?
            != KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY
        {
            return Err(invalid("downstream binding policy differs"));
        }
    }
    let canonical = hash(object, "canonical_sha256", "source_binding")?;
    let mut preimage = value_clone_object(object);
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_CANONICAL_MISMATCH",
            "source_binding canonical_sha256 differs from its payload",
        ));
    }
    Ok(Selectors {
        source_binding_id: id(object, "source_binding_id", "source_binding")?,
        source_binding_sha256: canonical,
        project_id: id(object, "project_id", "source_binding")?,
        intent_bundle_id: id(object, "intent_bundle_id", "source_binding")?,
        intent_bundle_sha256: hash(object, "intent_bundle_sha256", "source_binding")?,
        intent_bundle_object_sha256: hash(object, "intent_bundle_object_sha256", "source_binding")?,
        brief_id: id(object, "brief_id", "source_binding")?,
        brief_sha256: hash(object, "brief_sha256", "source_binding")?,
        brief_object_sha256: hash(object, "brief_object_sha256", "source_binding")?,
        reference_id: id(object, "reference_id", "source_binding")?,
        reference_object_sha256: hash(object, "reference_object_sha256", "source_binding")?,
        reference_evidence_sha256: hash(object, "reference_evidence_sha256", "source_binding")?,
        quality_contract_id: id(object, "quality_contract_id", "source_binding")?,
        quality_contract_sha256: hash(object, "quality_contract_sha256", "source_binding")?,
        quality_contract_object_sha256: hash(
            object,
            "quality_contract_object_sha256",
            "source_binding",
        )?,
        source_candidate_id: id(object, "source_candidate_id", "source_binding")?,
        source_candidate_state_sha256: hash(
            object,
            "source_candidate_state_sha256",
            "source_binding",
        )?,
        authoring_mesh_id: id(object, "authoring_mesh_id", "source_binding")?,
        authoring_mesh_lineage_id: id(object, "authoring_mesh_lineage_id", "source_binding")?,
        authoring_mesh_revision_id: id(object, "authoring_mesh_revision_id", "source_binding")?,
        authoring_mesh_revision_index: object
            .get("authoring_mesh_revision_index")
            .and_then(Value::as_u64)
            .filter(|value| *value <= 1_000_000)
            .ok_or_else(|| invalid("source_binding.authoring_mesh_revision_index is invalid"))?,
        authoring_mesh_revision_sha256: hash(
            object,
            "authoring_mesh_revision_sha256",
            "source_binding",
        )?,
        authoring_mesh_revision_object_sha256: hash(
            object,
            "authoring_mesh_revision_object_sha256",
            "source_binding",
        )?,
        authoring_mesh_identity_sha256: hash(
            object,
            "authoring_mesh_identity_sha256",
            "source_binding",
        )?,
        created_at: timestamp(
            text(object, "created_at", "source_binding")?,
            "source_binding.created_at",
        )?,
    })
}

fn value_clone_object(object: &Map<String, Value>) -> Value {
    Value::Object(object.clone())
}

fn reference_canonical_hash(
    reference: &forgecad_contracts::ReferenceEvidenceRecord,
) -> Result<String, RuntimeError> {
    let authorization = serde_json::to_value(&reference.authorization)
        .map_err(|error| invalid(error.to_string()))?;
    Ok(canonical_json_hash(&serde_json::json!({
        "schema_version": "ReferenceEvidence@1",
        "reference_id": reference.reference_id,
        "project_id": reference.project_id,
        "object_sha256": reference.object_sha256,
        "mime": reference.mime,
        "size_bytes": reference.size_bytes,
        "width": reference.width,
        "height": reference.height,
        "frame_count": reference.frame_count,
        "import_mode": reference.import_mode,
        "authorization": authorization,
        "derived_object_sha256": reference.derived_object_sha256,
        "created_at": reference.created_at,
    })))
}

fn source_identity_hash(
    mesh_id: &str,
    lineage_id: &str,
    revision_id: &str,
    revision_index: u64,
    revision_sha256: &str,
) -> String {
    canonical_json_hash(&serde_json::json!({
        "schema_version": "AuthoringMeshSourceIdentity@1",
        "mesh_id": mesh_id,
        "lineage_id": lineage_id,
        "revision_id": revision_id,
        "revision_index": revision_index,
        "revision_sha256": revision_sha256,
    }))
}

fn read_canonical_json(
    runtime: &Runtime,
    sha256: &str,
    max_bytes: u64,
    context: &str,
) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(sha256, max_bytes)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{context} CAS JSON is invalid: {error}")))?;
    let canonical = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    if canonical != bytes || sha256_hex(&bytes) != sha256 {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_CAS_MISMATCH",
            format!("{context} CAS bytes are not canonical or have the wrong object hash"),
        ));
    }
    Ok(value)
}

fn validate_cas_metadata(
    runtime: &Runtime,
    sha256: &str,
    kind: &str,
    context: &str,
) -> Result<(), RuntimeError> {
    validate_cas_metadata_with_budget(runtime, sha256, kind, 8 * 1024 * 1024, context)
}

fn validate_cas_metadata_with_budget(
    runtime: &Runtime,
    sha256: &str,
    kind: &str,
    max_bytes: u64,
    context: &str,
) -> Result<(), RuntimeError> {
    let object = runtime.store.get_object(sha256)?.ok_or_else(|| {
        mismatch(
            "KNIFE_SOURCE_BINDING_CAS_MISSING",
            format!("{context} object is absent"),
        )
    })?;
    let expected_mime = if kind == "geometry-glb" {
        "model/gltf-binary"
    } else {
        KNIFE_SOURCE_BINDING_JSON_MIME
    };
    if object.sha256 != sha256
        || object.mime != expected_mime
        || object.kind != kind
        || object.size_bytes == 0
        || object.size_bytes > max_bytes
        || object.reachability != "reachable"
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_CAS_METADATA_INVALID",
            format!("{context} metadata is not an exact reachable JSON object"),
        ));
    }
    Ok(())
}

fn load_upstream(runtime: &Runtime, selectors: &Selectors) -> Result<Upstream, RuntimeError> {
    let intent = runtime
        .store
        .get_knife_reference_intent_bundle_exact(
            &selectors.project_id,
            &selectors.brief_id,
            &selectors.brief_sha256,
            &selectors.brief_object_sha256,
            &selectors.reference_id,
            &selectors.reference_object_sha256,
            &selectors.reference_evidence_sha256,
            &selectors.intent_bundle_id,
            &selectors.intent_bundle_sha256,
            &selectors.intent_bundle_object_sha256,
        )?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_SOURCE_BINDING_INTENT_MISSING",
                "exact intent bundle is absent",
            )
        })?;
    let bundle = runtime
        .store
        .read_knife_reference_intent_bundle_json(
            &selectors.project_id,
            &selectors.brief_id,
            &selectors.intent_bundle_id,
            &selectors.intent_bundle_sha256,
        )?
        .ok_or_else(|| invalid("intent bundle disappeared before Runtime readback"))?;
    validate_cas_metadata(
        runtime,
        &selectors.intent_bundle_object_sha256,
        KNIFE_REFERENCE_INTENT_BUNDLE_OBJECT_KIND,
        "intent bundle",
    )?;
    let bundle_cas = read_canonical_json(
        runtime,
        &selectors.intent_bundle_object_sha256,
        8 * 1024 * 1024,
        "intent bundle",
    )?;
    if bundle_cas != bundle {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_INTENT_CAS_MISMATCH",
            "intent bundle Store readback differs from its exact CAS object",
        ));
    }
    if bundle.get("schema_version").and_then(Value::as_str) != Some("KnifeReferenceIntentBundle@1")
        || bundle.get("intent_bundle_id").and_then(Value::as_str)
            != Some(selectors.intent_bundle_id.as_str())
        || bundle.get("project_id").and_then(Value::as_str) != Some(selectors.project_id.as_str())
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_INTENT_BINDING_MISMATCH",
            "intent bundle identity differs",
        ));
    }
    let bundle_canonical = bundle
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("intent bundle canonical_sha256 is invalid"))?;
    let mut bundle_preimage = bundle.clone();
    bundle_preimage["canonical_sha256"] = Value::String(String::new());
    if bundle_canonical != selectors.intent_bundle_sha256
        || canonical_json_hash(&bundle_preimage) != selectors.intent_bundle_sha256
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_INTENT_CANONICAL_MISMATCH",
            "intent bundle semantic hash differs from its canonical CAS payload",
        ));
    }
    if intent.brief_id != selectors.brief_id
        || intent.brief_sha256 != selectors.brief_sha256
        || intent.brief_object_sha256 != selectors.brief_object_sha256
        || intent.reference_id != selectors.reference_id
        || intent.reference_object_sha256 != selectors.reference_object_sha256
        || intent.reference_evidence_sha256 != selectors.reference_evidence_sha256
        || intent.quality_contract_sha256 != selectors.quality_contract_sha256
        || intent.quality_contract_object_sha256 != selectors.quality_contract_object_sha256
        || intent.intent_bundle_object_sha256 != selectors.intent_bundle_object_sha256
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_INTENT_BINDING_MISMATCH",
            "intent Store fields differ",
        ));
    }
    let brief_binding = bundle
        .get("brief_binding")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("intent bundle Brief binding is missing"))?;
    if brief_binding.get("brief_id").and_then(Value::as_str) != Some(selectors.brief_id.as_str())
        || brief_binding.get("brief_sha256").and_then(Value::as_str)
            != Some(selectors.brief_sha256.as_str())
        || brief_binding
            .get("brief_object_sha256")
            .and_then(Value::as_str)
            != Some(selectors.brief_object_sha256.as_str())
        || brief_binding
            .get("authoring_eligibility")
            .and_then(Value::as_str)
            != Some("ELIGIBLE")
        || brief_binding
            .get("authorization_binding_status")
            .and_then(Value::as_str)
            != Some("runtime-bound")
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_BRIEF_BINDING_MISMATCH",
            "intent Brief binding differs",
        ));
    }
    let reference_binding = bundle
        .get("reference_binding")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("intent bundle Reference binding is missing"))?;
    if reference_binding
        .get("reference_id")
        .and_then(Value::as_str)
        != Some(selectors.reference_id.as_str())
        || reference_binding
            .get("reference_object_sha256")
            .and_then(Value::as_str)
            != Some(selectors.reference_object_sha256.as_str())
        || reference_binding
            .get("reference_evidence_sha256")
            .and_then(Value::as_str)
            != Some(selectors.reference_evidence_sha256.as_str())
        || reference_binding
            .get("binding_status")
            .and_then(Value::as_str)
            != Some("runtime-bound")
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_REFERENCE_BINDING_MISMATCH",
            "intent Reference binding differs",
        ));
    }
    let quality = bundle
        .get("quality_contract")
        .ok_or_else(|| invalid("intent quality contract child is missing"))?;
    let quality_id = quality
        .get("contract_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("intent quality contract id is invalid"))?;
    if quality_id != selectors.quality_contract_id {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_QUALITY_BINDING_MISMATCH",
            "quality id differs",
        ));
    }
    let quality_semantic = quality
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("intent quality contract canonical_sha256 is invalid"))?;
    let mut quality_preimage = quality.clone();
    quality_preimage["canonical_sha256"] = Value::String(String::new());
    if quality_semantic != selectors.quality_contract_sha256
        || canonical_json_hash(&quality_preimage) != selectors.quality_contract_sha256
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_QUALITY_CANONICAL_MISMATCH",
            "intent quality child semantic hash differs from its canonical payload",
        ));
    }
    validate_cas_metadata(
        runtime,
        &selectors.quality_contract_object_sha256,
        "knife-quality-contract",
        "quality contract",
    )?;
    let quality_cas = read_canonical_json(
        runtime,
        &selectors.quality_contract_object_sha256,
        8 * 1024 * 1024,
        "quality contract",
    )?;
    if quality_cas.get("schema_version").and_then(Value::as_str) != Some("KnifeQualityContract@1")
        || quality_cas.get("contract_id").and_then(Value::as_str)
            != Some(selectors.quality_contract_id.as_str())
        || quality_cas.get("canonical_sha256").and_then(Value::as_str)
            != Some(selectors.quality_contract_sha256.as_str())
        || quality_cas != *quality
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_QUALITY_BINDING_MISMATCH",
            "quality CAS child differs",
        ));
    }
    let mut quality_preimage = quality_cas.clone();
    quality_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&quality_preimage) != selectors.quality_contract_sha256 {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_QUALITY_CANONICAL_MISMATCH",
            "quality CAS semantic hash differs",
        ));
    }

    let brief_record = runtime
        .store
        .get_weaponry_knife_production_brief_exact(
            &selectors.project_id,
            &selectors.reference_id,
            &selectors.reference_object_sha256,
            &selectors.reference_evidence_sha256,
            &selectors.brief_id,
            &selectors.brief_sha256,
            &selectors.brief_object_sha256,
        )?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_SOURCE_BINDING_BRIEF_MISSING",
                "exact Brief is absent",
            )
        })?;
    let brief = runtime
        .store
        .read_weaponry_knife_production_brief_json(
            &selectors.project_id,
            &selectors.brief_id,
            &selectors.brief_sha256,
        )?
        .ok_or_else(|| invalid("Brief disappeared before Runtime readback"))?;
    validate_cas_metadata(
        runtime,
        &selectors.brief_object_sha256,
        "weaponry-knife-production-brief",
        "Brief",
    )?;
    let brief_cas = read_canonical_json(
        runtime,
        &selectors.brief_object_sha256,
        8 * 1024 * 1024,
        "Brief",
    )?;
    if brief_cas != brief {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_BRIEF_CAS_MISMATCH",
            "Brief Store readback differs from its exact CAS object",
        ));
    }
    let validation = super::weaponry_knife_production_brief::validate_brief(brief.clone())?;
    if brief_record.project_id != selectors.project_id
        || brief_record.brief_id != selectors.brief_id
        || brief_record.brief_canonical_sha256 != selectors.brief_sha256
        || brief_record.brief_object_sha256 != selectors.brief_object_sha256
        || validation.project_id != selectors.project_id
        || validation.brief_id != selectors.brief_id
        || validation.brief_sha256 != selectors.brief_sha256
        || validation.authoring_eligibility != KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY
        || validation.authorization_binding_status != "runtime-bound"
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_BRIEF_BINDING_MISMATCH",
            "Brief Runtime validation differs",
        ));
    }
    let reference = runtime.reference(&selectors.reference_id)?.ok_or_else(|| {
        mismatch(
            "KNIFE_SOURCE_BINDING_REFERENCE_MISSING",
            "ReferenceEvidence is absent",
        )
    })?;
    if reference.schema_version != "ReferenceEvidence@1"
        || reference.project_id != selectors.project_id
        || reference.reference_id != selectors.reference_id
        || reference.object_sha256 != selectors.reference_object_sha256
        || reference.canonical_sha256 != selectors.reference_evidence_sha256
        || reference_canonical_hash(&reference)? != selectors.reference_evidence_sha256
        || !reference.authorization.user_authorized
        || !matches!(reference.mime.as_str(), "image/png" | "image/jpeg")
        || reference.size_bytes == 0
        || reference.size_bytes > 8 * 1024 * 1024
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_REFERENCE_BINDING_MISMATCH",
            "ReferenceEvidence fields differ",
        ));
    }
    let reference_bytes = runtime.cas_read_bounded(&reference.object_sha256, 8 * 1024 * 1024)?;
    if reference_bytes.len() as u64 != reference.size_bytes
        || sha256_hex(&reference_bytes) != reference.object_sha256
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_REFERENCE_CAS_MISMATCH",
            "Reference CAS bytes differ",
        ));
    }
    Ok(Upstream {
        selectors: Selectors {
            quality_contract_id: quality_id.to_owned(),
            ..selectors.clone()
        },
    })
}

fn load_source(runtime: &Runtime, selectors: &Selectors) -> Result<SourceTruth, RuntimeError> {
    let candidate = runtime
        .candidate(&selectors.source_candidate_id)?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_SOURCE_BINDING_CANDIDATE_MISSING",
                "source candidate is absent",
            )
        })?;
    if candidate.project_id != selectors.project_id
        || candidate.canonical_sha256 != selectors.source_candidate_state_sha256
        || candidate.state != "reviewable"
        || !candidate.quality_hard_gate_passed
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_CANDIDATE_BINDING_MISMATCH",
            "candidate project, state hash, or source readiness differs",
        ));
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&selectors.source_candidate_id)?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_MISSING",
                "candidate geometry evidence is absent",
            )
        })?;
    if evidence.project_id != selectors.project_id
        || evidence.candidate_id != selectors.source_candidate_id
        || evidence.reference_id.as_deref() != Some(selectors.reference_id.as_str())
        || evidence.reference_sha256.as_deref() != Some(selectors.reference_object_sha256.as_str())
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_GEOMETRY_LINEAGE_MISMATCH",
            "geometry evidence project/candidate differs",
        ));
    }
    let durable = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(
            &selectors.project_id,
            &selectors.authoring_mesh_revision_id,
        )?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_SOURCE_BINDING_REVISION_MISSING",
                "AuthoringMesh@2 revision is absent",
            )
        })?;
    if durable.mesh_id != selectors.authoring_mesh_id
        || durable.lineage_id != selectors.authoring_mesh_lineage_id
        || durable.revision_id != selectors.authoring_mesh_revision_id
        || durable.revision_index != selectors.authoring_mesh_revision_index
        || durable.revision_sha256 != selectors.authoring_mesh_revision_sha256
        || durable.revision_object_sha256 != selectors.authoring_mesh_revision_object_sha256
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_REVISION_BINDING_MISMATCH",
            "durable revision identity differs",
        ));
    }
    validate_cas_metadata(
        runtime,
        &durable.revision_object_sha256,
        AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
        "AuthoringMesh revision",
    )?;
    let revision_value = read_canonical_json(
        runtime,
        &durable.revision_object_sha256,
        8 * 1024 * 1024,
        "AuthoringMesh revision",
    )?;
    let revision: AuthoringMeshRevision = serde_json::from_value(revision_value)
        .map_err(|error| invalid(format!("AuthoringMesh revision JSON is invalid: {error}")))?;
    let kernel = AuthoringMeshV2Revision::from_record(revision.clone())?;
    if kernel.record().canonical_sha256 != selectors.authoring_mesh_revision_sha256
        || kernel.record().mesh_id.0 != selectors.authoring_mesh_id
        || kernel.record().lineage_id.0 != selectors.authoring_mesh_lineage_id
        || kernel.record().revision_id.0 != selectors.authoring_mesh_revision_id
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_REVISION_BINDING_MISMATCH",
            "typed revision differs from durable row",
        ));
    }
    let embedded = revision.source_binding.as_ref().ok_or_else(|| {
        mismatch(
            "KNIFE_SOURCE_BINDING_REVISION_SOURCE_MISSING",
            "V2 revision has no embedded source binding",
        )
    })?;
    authoring_mesh_v2::validate_source_binding(embedded)?;
    if embedded.project_id != selectors.project_id
        || embedded.candidate_id != selectors.source_candidate_id
        || embedded.candidate_state_sha256 != selectors.source_candidate_state_sha256
        || embedded.geometry_program_sha256 != evidence.geometry_program_sha256
        || embedded.artifact_sha256 != evidence.artifact_object_sha256
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_REVISION_SOURCE_BINDING_MISMATCH",
            "embedded V2 source binding differs from candidate evidence",
        ));
    }
    validate_cas_metadata_with_budget(
        runtime,
        &evidence.geometry_program_object_sha256,
        "geometry-program-v2",
        64 * 1024 * 1024,
        "GeometryProgram",
    )?;
    let program = read_canonical_json(
        runtime,
        &evidence.geometry_program_object_sha256,
        64 * 1024 * 1024,
        "GeometryProgram",
    )?;
    let mut program_preimage = program.clone();
    let program_semantic = program_preimage
        .as_object_mut()
        .and_then(|object| object.remove("canonical_sha256"));
    if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || program.get("project_id").and_then(Value::as_str) != Some(selectors.project_id.as_str())
        || (program_semantic.is_some()
            && canonical_json_hash(&program_preimage) != evidence.geometry_program_sha256)
        || (program_semantic.is_none()
            && canonical_json_hash(&program) != evidence.geometry_program_sha256)
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_GEOMETRY_PROGRAM_MISMATCH",
            "candidate GeometryProgram CAS differs from its semantic evidence",
        ));
    }
    validate_cas_metadata_with_budget(
        runtime,
        &evidence.artifact_object_sha256,
        "geometry-glb",
        64 * 1024 * 1024,
        "source artifact",
    )?;
    let artifact_object = runtime
        .store
        .get_object(&evidence.artifact_object_sha256)?
        .ok_or_else(|| invalid("source artifact metadata disappeared"))?;
    let artifact_bytes = runtime.cas_read_bounded(&artifact_object.sha256, 64 * 1024 * 1024)?;
    if artifact_bytes.len() as u64 != artifact_object.size_bytes {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_ARTIFACT_CAS_MISMATCH",
            "source artifact CAS size differs from metadata",
        ));
    }
    validate_cas_metadata(
        runtime,
        &evidence.artifact_readback_object_sha256,
        "geometry-artifact-readback-v2",
        "source artifact readback",
    )?;
    let artifact_readback = read_canonical_json(
        runtime,
        &evidence.artifact_readback_object_sha256,
        8 * 1024 * 1024,
        "source artifact readback",
    )?;
    let mut artifact_readback_preimage = artifact_readback.clone();
    let artifact_readback_semantic = artifact_readback_preimage
        .as_object_mut()
        .and_then(|object| object.get("canonical_sha256").cloned());
    artifact_readback_preimage["canonical_sha256"] = Value::String(String::new());
    if artifact_readback
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("ArtifactReadback@2")
        || artifact_readback.get("artifact_id").and_then(Value::as_str)
            != Some(evidence.artifact_object_sha256.as_str())
        || artifact_readback
            .get("object_sha256")
            .and_then(Value::as_str)
            != Some(evidence.artifact_object_sha256.as_str())
        || artifact_readback
            .get("candidate_id")
            .and_then(Value::as_str)
            != Some(selectors.source_candidate_id.as_str())
        || artifact_readback
            .get("program_sha256")
            .and_then(Value::as_str)
            != Some(evidence.geometry_program_sha256.as_str())
        || artifact_readback_semantic.as_ref().and_then(Value::as_str)
            != Some(embedded.artifact_readback_sha256.as_str())
        || canonical_json_hash(&artifact_readback_preimage) != embedded.artifact_readback_sha256
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_ARTIFACT_READBACK_MISMATCH",
            "candidate ArtifactReadback semantic/object lineage differs",
        ));
    }
    validate_cas_metadata(
        runtime,
        &evidence.quality_report_object_sha256,
        "geometry-quality-report",
        "source geometry quality report",
    )?;
    let quality_report = read_canonical_json(
        runtime,
        &evidence.quality_report_object_sha256,
        8 * 1024 * 1024,
        "source geometry quality report",
    )?;
    if quality_report.get("schema_version").and_then(Value::as_str)
        != Some("GeometryQualityReport@2")
        || quality_report
            .get("quality_report_id")
            .and_then(Value::as_str)
            != Some(evidence.quality_report_id.as_str())
        || quality_report.get("candidate_id").and_then(Value::as_str)
            != Some(selectors.source_candidate_id.as_str())
        || quality_report
            .get("artifact_sha256")
            .and_then(Value::as_str)
            != Some(evidence.artifact_object_sha256.as_str())
        || quality_report.get("program_sha256").and_then(Value::as_str)
            != Some(evidence.geometry_program_sha256.as_str())
        || quality_report
            .get("artifact_readback_object_sha256")
            .and_then(Value::as_str)
            != Some(evidence.artifact_readback_object_sha256.as_str())
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_GEOMETRY_QUALITY_MISMATCH",
            "candidate GeometryQualityReport lineage differs",
        ));
    }
    if candidate.prepared_object_sha256.as_deref() != Some(embedded.artifact_sha256.as_str()) {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_CANDIDATE_ARTIFACT_MISMATCH",
            "embedded artifact is not candidate prepared object",
        ));
    }
    let identity_sha256 = source_identity_hash(
        &durable.mesh_id,
        &durable.lineage_id,
        &durable.revision_id,
        durable.revision_index,
        &durable.revision_sha256,
    );
    if selectors.authoring_mesh_identity_sha256 != identity_sha256 {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_IDENTITY_MISMATCH",
            "AuthoringMeshSourceIdentity@1 differs",
        ));
    }
    Ok(SourceTruth {
        candidate_state_sha256: candidate.canonical_sha256,
        revision,
        durable,
        identity_sha256,
    })
}

fn derive_binding(upstream: &Upstream, source: &SourceTruth) -> Result<Value, RuntimeError> {
    let s = &upstream.selectors;
    let mut value = serde_json::json!({
        "schema_version": KNIFE_SOURCE_BINDING_SCHEMA_VERSION,
        "source_binding_id": s.source_binding_id,
        "project_id": s.project_id,
        "binding_status": KNIFE_SOURCE_BINDING_BINDING_STATUS,
        "authoring_eligibility": KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY,
        "intent_bundle_id": s.intent_bundle_id,
        "intent_bundle_sha256": s.intent_bundle_sha256,
        "intent_bundle_object_sha256": s.intent_bundle_object_sha256,
        "brief_id": s.brief_id,
        "brief_sha256": s.brief_sha256,
        "brief_object_sha256": s.brief_object_sha256,
        "reference_id": s.reference_id,
        "reference_object_sha256": s.reference_object_sha256,
        "reference_evidence_sha256": s.reference_evidence_sha256,
        "quality_contract_id": s.quality_contract_id,
        "quality_contract_sha256": s.quality_contract_sha256,
        "quality_contract_object_sha256": s.quality_contract_object_sha256,
        "source_candidate_id": source.revision.source_binding.as_ref().map(|b| b.candidate_id.clone()).unwrap_or_default(),
        "source_candidate_state_sha256": source.candidate_state_sha256,
        "authoring_mesh_id": source.durable.mesh_id,
        "authoring_mesh_lineage_id": source.durable.lineage_id,
        "authoring_mesh_revision_id": source.durable.revision_id,
        "authoring_mesh_revision_index": source.durable.revision_index,
        "authoring_mesh_revision_sha256": source.durable.revision_sha256,
        "authoring_mesh_revision_object_sha256": source.durable.revision_object_sha256,
        "authoring_mesh_identity_sha256": source.identity_sha256,
        "downstream_binding_requirements": {
            "curve_modifier_graph": KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY,
            "curve_evaluated_mesh": KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY,
            "high": KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY,
            "render": KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY,
        },
        "high_mesh_created": false,
        "high_stage_unlocked": false,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "source_binding_only",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "binding_policy": KNIFE_SOURCE_BINDING_POLICY,
        "canonicalization_policy": KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY,
        "canonical_sha256": "",
        "created_at": s.created_at,
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    Ok(value)
}

fn store_record(
    main: &Value,
    object: &CasObject,
    idempotency_key: &str,
) -> Result<KnifeSourceBindingStoreRecord, RuntimeError> {
    let mut record = main
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("derived source binding is not an object"))?;
    let source_binding_sha256 = record
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("derived source binding canonical hash is invalid"))?
        .to_owned();
    record.insert(
        "schema_version".to_owned(),
        Value::String(KNIFE_SOURCE_BINDING_RECORD_SCHEMA_VERSION.to_owned()),
    );
    record.remove("canonical_sha256");
    record.insert(
        "source_binding_sha256".to_owned(),
        Value::String(source_binding_sha256),
    );
    record.insert(
        "source_binding_object_sha256".to_owned(),
        Value::String(object.record.sha256.clone()),
    );
    record.insert(
        "idempotency_key".to_owned(),
        Value::String(idempotency_key.to_owned()),
    );
    serde_json::from_value(Value::Object(record))
        .map_err(|error| invalid(format!("derived Store record is invalid: {error}")))
}

fn result_value(
    record: &KnifeSourceBindingStoreRecord,
    source_binding: Value,
    request_kind: &str,
    replayed: bool,
    store_effect: &str,
    cas_effect: &str,
    max_response_bytes: u64,
) -> Result<Value, RuntimeError> {
    let mut result = serde_json::json!({
        "schema_version": RESULT_SCHEMA,
        "operation": if request_kind == "get" { GET_OPERATION } else { PREPARE_OPERATION },
        "request_kind": request_kind,
        "status": if request_kind == "get" { "found" } else if replayed { "replayed" } else { "prepared" },
        "project_id": record.project_id,
        "source_binding_id": record.source_binding_id,
        "source_binding_sha256": record.source_binding_sha256,
        "source_binding_object_sha256": record.source_binding_object_sha256,
        "binding_status": record.binding_status,
        "authoring_eligibility": record.authoring_eligibility,
        "intent_bundle_id": record.intent_bundle_id,
        "intent_bundle_sha256": record.intent_bundle_sha256,
        "intent_bundle_object_sha256": record.intent_bundle_object_sha256,
        "brief_id": record.brief_id,
        "brief_sha256": record.brief_sha256,
        "brief_object_sha256": record.brief_object_sha256,
        "reference_id": record.reference_id,
        "reference_object_sha256": record.reference_object_sha256,
        "reference_evidence_sha256": record.reference_evidence_sha256,
        "quality_contract_id": record.quality_contract_id,
        "quality_contract_sha256": record.quality_contract_sha256,
        "quality_contract_object_sha256": record.quality_contract_object_sha256,
        "source_candidate_id": record.source_candidate_id,
        "source_candidate_state_sha256": record.source_candidate_state_sha256,
        "authoring_mesh_id": record.authoring_mesh_id,
        "authoring_mesh_lineage_id": record.authoring_mesh_lineage_id,
        "authoring_mesh_revision_id": record.authoring_mesh_revision_id,
        "authoring_mesh_revision_index": record.authoring_mesh_revision_index,
        "authoring_mesh_revision_sha256": record.authoring_mesh_revision_sha256,
        "authoring_mesh_revision_object_sha256": record.authoring_mesh_revision_object_sha256,
        "authoring_mesh_identity_sha256": record.authoring_mesh_identity_sha256,
        "downstream_binding_requirements": record.downstream_binding_requirements,
        "source_binding": source_binding,
        "idempotency_key": if request_kind == "get" { Value::Null } else { Value::String(record.idempotency_key.clone()) },
        "replayed": replayed,
        "store_effect": store_effect,
        "cas_effect": cas_effect,
        "atomicity_status": if request_kind == "get" || replayed { "not-touched" } else { "committed" },
        "store_commit_status": if request_kind == "get" || replayed { "not-touched" } else { "committed" },
        "cas_commit_status": if request_kind == "get" || replayed { "not-touched" } else { "committed" },
        "runtime_write_performed": request_kind == "prepare" && !replayed,
        "persistent_user_data_touched": request_kind == "prepare" && !replayed,
        "partial_result_exposed": false,
        "high_mesh_created": false,
        "high_stage_unlocked": false,
        "production_stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "source_binding_only",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY,
        "canonical_sha256": "",
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    let bytes = canonical_json_bytes(&result).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > max_response_bytes || bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(invalid("result exceeds max_response_bytes"));
    }
    Ok(result)
}

fn commit_readback(
    runtime: &Runtime,
    record: &KnifeSourceBindingStoreRecord,
) -> Result<Value, RuntimeError> {
    runtime
        .store
        .read_knife_source_binding_json(
            &record.project_id,
            &record.source_binding_id,
            &record.source_binding_sha256,
        )?
        .ok_or_else(|| invalid("source binding disappeared before readback"))
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    request_header(object, PREPARE_SCHEMA, PREPARE_OPERATION, "prepare")?;
    request_hash(request, object, "prepare")?;
    let project_id = id(object, "project_id", "prepare")?;
    let idempotency_key = id(object, "idempotency_key", "prepare")?;
    if runtime.project(&project_id)?.is_none() {
        return Err(mismatch(
            "PROJECT_SCOPE_DENIED",
            "source binding project does not exist",
        ));
    }
    let selectors = source_selectors(
        object
            .get("source_binding")
            .ok_or_else(|| invalid("prepare.source_binding is missing"))?,
    )?;
    if selectors.project_id != project_id {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_PROJECT_SCOPE_MISMATCH",
            "request and source binding project differ",
        ));
    }
    let upstream = load_upstream(runtime, &selectors)?;
    let source = load_source(runtime, &selectors)?;
    let derived = derive_binding(&upstream, &source)?;
    if derived
        != *object
            .get("source_binding")
            .ok_or_else(|| invalid("prepare.source_binding is missing"))?
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_DERIVED_MISMATCH",
            "caller source binding is not the Runtime-derived truth",
        ));
    }

    if let Some(existing) = runtime
        .store
        .get_knife_source_binding_by_idempotency(&project_id, &idempotency_key)?
    {
        let expected_object = runtime
            .store
            .get_object(&existing.source_binding_object_sha256)?
            .ok_or_else(|| invalid("existing source binding CAS metadata disappeared"))?;
        let expected = runtime
            .cas_read_bounded(&expected_object.sha256, KNIFE_SOURCE_BINDING_MAX_JSON_BYTES)?;
        if sha256_hex(&expected) != expected_object.sha256 {
            return Err(invalid("existing source binding CAS hash differs"));
        }
        let expected_value: Value = serde_json::from_slice(&expected).map_err(|error| {
            invalid(format!(
                "existing source binding CAS JSON is invalid: {error}"
            ))
        })?;
        if expected_value != derived {
            return Err(mismatch(
                "KNIFE_SOURCE_BINDING_IDEMPOTENCY_CONFLICT",
                "idempotency key is bound to different source metadata",
            ));
        }
        let stored = commit_readback(runtime, &existing)?;
        return result_value(
            &existing,
            stored,
            "prepare",
            true,
            "not-touched",
            "not-touched",
            MAX_RESPONSE_BYTES,
        );
    }

    let bytes = canonical_json_bytes(&derived).map_err(|error| invalid(error.to_string()))?;
    if bytes.is_empty() || bytes.len() as u64 > KNIFE_SOURCE_BINDING_MAX_JSON_BYTES {
        return Err(invalid("derived source binding exceeds CAS budget"));
    }
    let reservation = runtime.store.begin_cas_reservation();
    let mut staged: Option<CasObject> = None;
    let result = (|| {
        let object = runtime.store.put_object_reserved(
            &reservation,
            &bytes,
            None,
            KNIFE_SOURCE_BINDING_JSON_MIME,
            KNIFE_SOURCE_BINDING_OBJECT_KIND,
            &selectors.created_at,
        )?;
        staged = Some(object.clone());
        let record = store_record(&derived, &object, &idempotency_key)?;
        let commit = KnifeSourceBindingCommit {
            record,
            cas: KnifeSourceBindingCasBundle {
                source_binding: object.record.clone(),
            },
        };
        let (stored, replayed) = runtime
            .store
            .record_knife_source_binding_with_replay(&commit)?;
        let _ = runtime
            .store
            .release_cas_reservation_object(&reservation, &object, false);
        staged = None;
        let readback = commit_readback(runtime, &stored)?;
        let readback_object =
            exact_object(&readback, SOURCE_BINDING_FIELDS, "source binding readback")?;
        if readback_object
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(stored.source_binding_sha256.as_str())
        {
            return Err(invalid("source binding readback semantic hash differs"));
        }
        result_value(
            &stored,
            readback,
            "prepare",
            replayed,
            if replayed { "not-touched" } else { "inserted" },
            if replayed { "not-touched" } else { "inserted" },
            MAX_RESPONSE_BYTES,
        )
    })();
    if result.is_err() {
        if let Some(object) = staged.as_ref() {
            let _ = runtime
                .store
                .release_cas_reservation_object(&reservation, object, true);
        }
    }
    result
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_FIELDS, GET_SCHEMA)?;
    request_header(object, GET_SCHEMA, GET_OPERATION, "get")?;
    request_hash(request, object, "get")?;
    bool_exact(object, "persistent_user_data_touched", false, "get")?;
    let project_id = id(object, "project_id", "get")?;
    let source_binding_id = id(object, "source_binding_id", "get")?;
    let source_binding_sha256 = hash(object, "source_binding_sha256", "get")?;
    let source_binding_object_sha256 = hash(object, "source_binding_object_sha256", "get")?;
    let intent_bundle_id = id(object, "intent_bundle_id", "get")?;
    let intent_bundle_sha256 = hash(object, "intent_bundle_sha256", "get")?;
    let intent_bundle_object_sha256 = hash(object, "intent_bundle_object_sha256", "get")?;
    let brief_id = id(object, "brief_id", "get")?;
    let brief_sha256 = hash(object, "brief_sha256", "get")?;
    let brief_object_sha256 = hash(object, "brief_object_sha256", "get")?;
    let reference_id = id(object, "reference_id", "get")?;
    let reference_object_sha256 = hash(object, "reference_object_sha256", "get")?;
    let reference_evidence_sha256 = hash(object, "reference_evidence_sha256", "get")?;
    let quality_contract_id = id(object, "quality_contract_id", "get")?;
    let quality_contract_sha256 = hash(object, "quality_contract_sha256", "get")?;
    let quality_contract_object_sha256 = hash(object, "quality_contract_object_sha256", "get")?;
    let source_candidate_id = id(object, "source_candidate_id", "get")?;
    let source_candidate_state_sha256 = hash(object, "source_candidate_state_sha256", "get")?;
    let authoring_mesh_id = id(object, "authoring_mesh_id", "get")?;
    let authoring_mesh_lineage_id = id(object, "authoring_mesh_lineage_id", "get")?;
    let authoring_mesh_revision_id = id(object, "authoring_mesh_revision_id", "get")?;
    let authoring_mesh_revision_index = object
        .get("authoring_mesh_revision_index")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| invalid("get.authoring_mesh_revision_index is invalid"))?;
    let authoring_mesh_revision_sha256 = hash(object, "authoring_mesh_revision_sha256", "get")?;
    let authoring_mesh_revision_object_sha256 =
        hash(object, "authoring_mesh_revision_object_sha256", "get")?;
    let authoring_mesh_identity_sha256 = hash(object, "authoring_mesh_identity_sha256", "get")?;
    let max_response_bytes = object
        .get("max_response_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("get.max_response_bytes is invalid"))?;
    let selectors = Selectors {
        source_binding_id,
        source_binding_sha256,
        project_id: project_id.clone(),
        intent_bundle_id,
        intent_bundle_sha256,
        intent_bundle_object_sha256,
        brief_id,
        brief_sha256,
        brief_object_sha256,
        reference_id,
        reference_object_sha256,
        reference_evidence_sha256,
        quality_contract_id,
        quality_contract_sha256,
        quality_contract_object_sha256,
        source_candidate_id,
        source_candidate_state_sha256,
        authoring_mesh_id,
        authoring_mesh_lineage_id,
        authoring_mesh_revision_id,
        authoring_mesh_revision_index,
        authoring_mesh_revision_sha256,
        authoring_mesh_revision_object_sha256,
        authoring_mesh_identity_sha256,
        created_at: String::new(),
    };
    let record = runtime
        .store
        .get_knife_source_binding_exact(
            &selectors.project_id,
            &selectors.source_binding_id,
            &selectors.source_binding_sha256,
            &source_binding_object_sha256,
            &selectors.intent_bundle_id,
            &selectors.intent_bundle_sha256,
            &selectors.intent_bundle_object_sha256,
            &selectors.brief_id,
            &selectors.brief_sha256,
            &selectors.brief_object_sha256,
            &selectors.reference_id,
            &selectors.reference_object_sha256,
            &selectors.reference_evidence_sha256,
            &selectors.quality_contract_id,
            &selectors.quality_contract_sha256,
            &selectors.quality_contract_object_sha256,
            &selectors.source_candidate_id,
            &selectors.source_candidate_state_sha256,
            &selectors.authoring_mesh_id,
            &selectors.authoring_mesh_lineage_id,
            &selectors.authoring_mesh_revision_id,
            selectors.authoring_mesh_revision_index,
            &selectors.authoring_mesh_revision_sha256,
            &selectors.authoring_mesh_revision_object_sha256,
            &selectors.authoring_mesh_identity_sha256,
        )?
        .ok_or_else(|| {
            mismatch(
                "KNIFE_SOURCE_BINDING_NOT_FOUND",
                "no exact source binding exists",
            )
        })?;
    let main = commit_readback(runtime, &record)?;
    let main_selectors = source_selectors(&main)?;
    if main_selectors.project_id != project_id
        || main_selectors.source_binding_id != selectors.source_binding_id
        || main_selectors.intent_bundle_id != selectors.intent_bundle_id
        || main_selectors.source_candidate_id != selectors.source_candidate_id
    {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_READBACK_MISMATCH",
            "source binding readback differs from get identity",
        ));
    }
    let upstream = load_upstream(runtime, &main_selectors)?;
    let source = load_source(runtime, &main_selectors)?;
    let derived = derive_binding(&upstream, &source)?;
    if derived != main {
        return Err(mismatch(
            "KNIFE_SOURCE_BINDING_READBACK_MISMATCH",
            "Runtime-derived source binding differs from CAS",
        ));
    }
    result_value(
        &record,
        main,
        "get",
        false,
        "not-touched",
        "not-touched",
        max_response_bytes,
    )
}

impl Runtime {
    pub fn knife_source_binding_prepare(&self, request: &Value) -> Result<Value, RuntimeError> {
        prepare(self, request)
    }

    pub fn knife_source_binding_get(&self, request: &Value) -> Result<Value, RuntimeError> {
        get(self, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{
        ReferenceAuthorization, ReferenceImportRequest, ReferenceImportSource,
    };
    use serde_json::json;
    use std::path::PathBuf;

    const MAX_BYTES: u64 = 1_048_576;
    const NOW: &str = "2026-08-30T00:00:00Z";

    fn contract_fixture() -> Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/fixtures/knife-source-binding/positive/dragonfang-source-binding.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).expect("source binding fixture"))
            .expect("source binding fixture JSON")
    }

    fn bundle_fixture() -> Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/fixtures/weaponry-knife-reference-intent-bundle/positive/dragonfang-reference-intent-bundle.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).expect("intent fixture"))
            .expect("intent fixture JSON")
    }

    fn brief_fixture(name: &str) -> Value {
        let path = format!(
            "{}/../../../../../packages/forgecad-contracts/fixtures/weaponry-knife-production-brief/positive/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(path).expect("Brief fixture"))
            .expect("Brief fixture JSON")
    }

    fn refill_hash(value: &mut Value) {
        value["canonical_sha256"] = Value::String(String::new());
        value["canonical_sha256"] = Value::String(canonical_json_hash(value));
    }

    fn import_reference(
        runtime: &Runtime,
        project_id: &str,
    ) -> forgecad_contracts::ReferenceEvidenceRecord {
        runtime
            .import_reference(&ReferenceImportRequest {
                project_id: project_id.to_owned(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_owned(),
                },
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "Runtime knife source binding fixture".to_owned(),
                },
                expected_sha256: None,
            })
            .expect("reference import")
            .reference
    }

    fn runtime_brief(
        name: &str,
        project_id: &str,
        reference: &forgecad_contracts::ReferenceEvidenceRecord,
    ) -> Value {
        let mut brief = brief_fixture(name);
        brief["project_id"] = Value::String(project_id.to_owned());
        brief["authorization"]["source_reference_sha256"] =
            Value::String(reference.object_sha256.clone());
        brief["reference_coverage"]["source_reference_sha256"] =
            Value::String(reference.object_sha256.clone());
        brief["reference_coverage"]["source_dimensions"] =
            json!({"width": reference.width, "height": reference.height});
        if name == "dragonfang-kukri-brief-resolved-001.json" {
            brief["authorization"]["evidence_status"] = Value::String("runtime-bound".to_owned());
            brief["acceptance_constraints"]["gate_statuses"][0]["status"] =
                Value::String("pass".to_owned());
            if let Some(reasons) =
                brief["acceptance_constraints"]["blocking_reasons"].as_array_mut()
            {
                reasons.retain(|value| value.as_str() != Some("authorization-not-runtime-bound"));
            }
        }
        refill_hash(&mut brief);
        brief
    }

    fn brief_request(
        brief: Value,
        reference: &forgecad_contracts::ReferenceEvidenceRecord,
        key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version": "WeaponryKnifeProductionBriefPrepareRequest@1",
            "operation": "weaponry_knife_production_brief_prepare",
            "project_id": brief["project_id"],
            "brief": brief,
            "reference_id": reference.reference_id,
            "reference_object_sha256": reference.object_sha256,
            "reference_evidence_sha256": reference.canonical_sha256,
            "idempotency_key": key,
            "max_response_bytes": MAX_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn prepare_brief(
        runtime: &Runtime,
        project_id: &str,
        reference: &forgecad_contracts::ReferenceEvidenceRecord,
    ) -> Value {
        let parent = runtime_brief("dragonfang-kukri-brief.json", project_id, reference);
        let parent_result = runtime
            .weaponry_knife_production_brief_prepare(&brief_request(
                parent,
                reference,
                "source-binding-parent-brief",
            ))
            .expect("parent Brief prepare");
        let mut eligible = runtime_brief(
            "dragonfang-kukri-brief-resolved-001.json",
            project_id,
            reference,
        );
        eligible["parent_brief_id"] = parent_result["brief_id"].clone();
        eligible["parent_brief_sha256"] = parent_result["brief_sha256"].clone();
        refill_hash(&mut eligible);
        runtime
            .weaponry_knife_production_brief_prepare(&brief_request(
                eligible,
                reference,
                "source-binding-eligible-brief",
            ))
            .expect("eligible Brief prepare")
    }

    fn rewrite_reference_bindings(
        value: &mut Value,
        reference: &forgecad_contracts::ReferenceEvidenceRecord,
    ) {
        match value {
            Value::Object(object) => {
                for (key, child) in object.iter_mut() {
                    match key.as_str() {
                        "reference_id" => *child = Value::String(reference.reference_id.clone()),
                        "reference_object_sha256" => {
                            *child = Value::String(reference.object_sha256.clone())
                        }
                        "reference_evidence_sha256" => {
                            *child = Value::String(reference.canonical_sha256.clone())
                        }
                        _ => rewrite_reference_bindings(child, reference),
                    }
                }
            }
            Value::Array(values) => {
                for child in values {
                    rewrite_reference_bindings(child, reference);
                }
            }
            _ => {}
        }
    }

    fn runtime_bundle(
        brief_result: &Value,
        reference: &forgecad_contracts::ReferenceEvidenceRecord,
        id: &str,
    ) -> Value {
        let mut bundle = bundle_fixture();
        bundle["intent_bundle_id"] = Value::String(id.to_owned());
        bundle["project_id"] = brief_result["project_id"].clone();
        bundle["brief_binding"]["brief_id"] = brief_result["brief_id"].clone();
        bundle["brief_binding"]["brief_sha256"] = brief_result["brief_sha256"].clone();
        bundle["brief_binding"]["brief_object_sha256"] =
            brief_result["brief_object_sha256"].clone();
        rewrite_reference_bindings(&mut bundle, reference);
        if let Some(features) = bundle["quality_contract"]["critical_features"].as_array_mut() {
            for feature in features {
                if let Some(regions) = feature["evidence_region_ids"].as_array_mut() {
                    for region in regions {
                        if let Some((_, view)) =
                            region.as_str().and_then(|value| value.split_once(':'))
                        {
                            *region = Value::String(format!("{}:{view}", reference.reference_id));
                        }
                    }
                }
            }
        }
        bundle["intake_manifest"]["records"][0]["resolution"] =
            json!({"width": reference.width, "height": reference.height});
        refill_hash(&mut bundle["intake_manifest"]);
        refill_hash(&mut bundle["detail_inventory"]);
        refill_hash(&mut bundle["quality_contract"]);
        refill_hash(&mut bundle);
        bundle
    }

    fn intent_request(bundle: Value, key: &str) -> Value {
        let mut request = json!({
            "schema_version": "KnifeReferenceIntentBundlePrepareRequest@1",
            "operation": "knife_reference_intent_bundle_prepare",
            "project_id": bundle["project_id"],
            "brief_id": bundle["brief_binding"]["brief_id"],
            "brief_sha256": bundle["brief_binding"]["brief_sha256"],
            "brief_object_sha256": bundle["brief_binding"]["brief_object_sha256"],
            "reference_id": bundle["reference_binding"]["reference_id"],
            "reference_object_sha256": bundle["reference_binding"]["reference_object_sha256"],
            "reference_evidence_sha256": bundle["reference_binding"]["reference_evidence_sha256"],
            "brief_authoring_eligibility": "ELIGIBLE",
            "intent_bundle": bundle,
            "idempotency_key": key,
            "max_response_bytes": MAX_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn source_program(project_id: &str, include_preserved_part: bool) -> Value {
        let mut program = json!({
            "schema_version": "GeometryProgram@2",
            "project_id": project_id,
            "representation_plan_sha256": "9".repeat(64),
            "operator_catalog_sha256": super::super::operator_catalog_sha256(),
            "units": {"length": "meter", "angle": "radian", "coordinate_system": "right-handed-y-up"},
            "budgets": {"max_nodes": if include_preserved_part { 2 } else { 1 }, "max_triangles": 1000, "max_glb_bytes": 1048576, "max_worker_memory_bytes": 536870912, "max_runtime_ms": 10000},
            "nodes": [{"node_id": "blade", "operator_id": "forgecad.geometry.profile-extrude@1", "inputs": [], "parameters": {
                "shape": "profile-extrude",
                "profile": [[-1.18, -0.10], [-0.92, -0.17], [-0.52, -0.24], [-0.08, -0.30], [0.42, -0.31], [0.92, -0.24], [1.38, -0.12], [1.80, 0.05], [2.02, 0.18], [1.72, 0.25], [1.28, 0.34], [0.78, 0.42], [0.24, 0.40], [-0.30, 0.31], [-0.78, 0.17], [-1.10, 0.05]],
                "depth_m": 0.06,
                "position_m": [0.0, 0.0, 0.0],
                "rotation_rad": [0.0, 0.0, 0.0]
            }}],
            "part_outputs": [{"part_id": "blade", "input_node_ids": ["blade"], "material_zone_id": "blade-zone", "solid": true}]
        });
        if include_preserved_part {
            program["nodes"]
                .as_array_mut()
                .expect("source nodes")
                .push(json!({
                    "node_id": "guard",
                    "operator_id": "forgecad.geometry.profile-extrude@1",
                    "inputs": [],
                    "parameters": {
                        "shape": "profile-extrude",
                        "profile": [[-0.24, -0.08], [0.24, -0.08], [0.24, 0.08], [-0.24, 0.08]],
                        "depth_m": 0.12,
                        "position_m": [-1.28, 0.0, 0.0],
                        "rotation_rad": [0.0, 0.0, 0.0]
                    }
                }));
            program["part_outputs"]
                .as_array_mut()
                .expect("source outputs")
                .push(json!({
                    "part_id": "guard",
                    "input_node_ids": ["guard"],
                    "material_zone_id": "guard-zone",
                    "solid": true
                }));
        }
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        program
    }

    pub(super) fn source_binding_request(runtime: &Runtime, suffix: &str) -> Value {
        source_binding_request_with_program(runtime, suffix, false)
    }

    pub(super) fn source_binding_request_multi_part(runtime: &Runtime, suffix: &str) -> Value {
        source_binding_request_with_program(runtime, suffix, true)
    }

    fn source_binding_request_with_program(
        runtime: &Runtime,
        suffix: &str,
        include_preserved_part: bool,
    ) -> Value {
        let project = runtime
            .create_project(
                "knife source binding integration",
                json!({"profile": "knife"}),
            )
            .expect("project");
        let reference = import_reference(runtime, &project.project_id);
        let brief = prepare_brief(runtime, &project.project_id, &reference);
        let bundle = runtime_bundle(
            &brief,
            &reference,
            &format!("source-binding-intent-{suffix}"),
        );
        let intent = runtime
            .knife_reference_intent_bundle_prepare(&intent_request(
                bundle,
                &format!("source-binding-intent-key-{suffix}"),
            ))
            .expect("intent bundle prepare");
        let intent_record = runtime
            .store
            .get_knife_reference_intent_bundle(
                intent["project_id"].as_str().expect("intent project"),
                intent["brief_id"].as_str().expect("intent brief"),
                intent["intent_bundle_id"].as_str().expect("intent id"),
                intent["intent_bundle_sha256"]
                    .as_str()
                    .expect("intent hash"),
            )
            .expect("intent record lookup")
            .expect("intent record");
        let geometry = runtime
            .prepare_geometry_candidate_exact(
                &project.project_id,
                None,
                &format!("source-binding-geometry-key-{suffix}"),
                json!({
                    "typed": "geometry",
                    "reference_id": reference.reference_id,
                    "geometry_program": source_program(&project.project_id, include_preserved_part)
                }),
            )
            .expect("geometry candidate prepare");
        let candidate = &geometry["candidate"];
        let artifact = &geometry["artifact"];
        let source = runtime
            .production_weapon_authoring_mesh_v2_source_prepare(&{
                let mut request = json!({
                    "schema_version": "ProductionWeaponAuthoringMeshV2SourcePrepareRequest@1",
                    "project_id": project.project_id,
                    "candidate_id": candidate["candidate_id"],
                    "candidate_state_sha256": candidate["canonical_sha256"],
                    "geometry_program_sha256": artifact["program_sha256"],
                    "artifact_sha256": candidate["prepared_object_sha256"],
                    "artifact_readback_sha256": artifact["canonical_sha256"],
                    "part_id": "blade",
                    "source_node_id": "blade",
                    "idempotency_key": format!("source-binding-mesh-key-{suffix}"),
                    "max_response_bytes": MAX_BYTES,
                    "runtime_write_performed": false,
                    "writer_policy": WRITER_POLICY,
                    "canonicalization_policy": KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY,
                    "input_sha256": ""
                });
                request["input_sha256"] = Value::String(canonical_json_hash(&request));
                request
            })
            .expect("AuthoringMesh V2 source prepare");
        let durable = &source["authoring_mesh_v2"];
        let mesh_id = durable["mesh_id"].as_str().expect("mesh id");
        let lineage_id = durable["lineage_id"].as_str().expect("lineage id");
        let revision_id = durable["revision_id"].as_str().expect("revision id");
        let revision_index = durable["revision_index"].as_u64().expect("revision index");
        let revision_sha256 = durable["revision_sha256"].as_str().expect("revision hash");
        let identity_sha256 = source_identity_hash(
            mesh_id,
            lineage_id,
            revision_id,
            revision_index,
            revision_sha256,
        );
        let mut binding = json!({
            "schema_version": KNIFE_SOURCE_BINDING_SCHEMA_VERSION,
            "source_binding_id": format!("source-binding-{suffix}"),
            "project_id": project.project_id,
            "binding_status": KNIFE_SOURCE_BINDING_BINDING_STATUS,
            "authoring_eligibility": KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY,
            "intent_bundle_id": intent["intent_bundle_id"],
            "intent_bundle_sha256": intent["intent_bundle_sha256"],
            "intent_bundle_object_sha256": intent["intent_bundle_object_sha256"],
            "brief_id": intent["brief_id"],
            "brief_sha256": intent["brief_sha256"],
            "brief_object_sha256": intent["brief_object_sha256"],
            "reference_id": intent["reference_id"],
            "reference_object_sha256": intent["reference_object_sha256"],
            "reference_evidence_sha256": intent["reference_evidence_sha256"],
            "quality_contract_id": intent["intent_bundle"]["quality_contract"]["contract_id"],
            "quality_contract_sha256": intent["intent_bundle"]["quality_contract"]["canonical_sha256"],
            "quality_contract_object_sha256": intent_record.quality_contract_object_sha256,
            "source_candidate_id": candidate["candidate_id"],
            "source_candidate_state_sha256": candidate["canonical_sha256"],
            "authoring_mesh_id": mesh_id,
            "authoring_mesh_lineage_id": lineage_id,
            "authoring_mesh_revision_id": revision_id,
            "authoring_mesh_revision_index": revision_index,
            "authoring_mesh_revision_sha256": revision_sha256,
            "authoring_mesh_revision_object_sha256": durable["revision_object_sha256"],
            "authoring_mesh_identity_sha256": identity_sha256,
            "downstream_binding_requirements": {
                "curve_modifier_graph": KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY,
                "curve_evaluated_mesh": KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY,
                "high": KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY,
                "render": KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY
            },
            "high_mesh_created": false,
            "high_stage_unlocked": false,
            "production_stage_advanced": false,
            "candidate_confirmed": false,
            "version_created": false,
            "export_performed": false,
            "quality_status": "source_binding_only",
            "visual_status": "NOT_RUN",
            "human_status": "NOT_RUN",
            "engine_status": "NOT_RUN",
            "binding_policy": KNIFE_SOURCE_BINDING_POLICY,
            "canonicalization_policy": KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY,
            "canonical_sha256": "",
            "created_at": NOW
        });
        binding["canonical_sha256"] = Value::String(canonical_json_hash(&binding));
        let mut request = json!({
            "schema_version": PREPARE_SCHEMA,
            "operation": PREPARE_OPERATION,
            "project_id": binding["project_id"],
            "source_binding": binding,
            "idempotency_key": format!("source-binding-key-{suffix}"),
            "max_response_bytes": MAX_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn get_request(result: &Value) -> Value {
        let mut request = json!({
            "schema_version": GET_SCHEMA,
            "operation": GET_OPERATION,
            "project_id": result["project_id"],
            "source_binding_id": result["source_binding_id"],
            "source_binding_sha256": result["source_binding_sha256"],
            "source_binding_object_sha256": result["source_binding_object_sha256"],
            "intent_bundle_id": result["intent_bundle_id"],
            "intent_bundle_sha256": result["intent_bundle_sha256"],
            "intent_bundle_object_sha256": result["intent_bundle_object_sha256"],
            "brief_id": result["brief_id"],
            "brief_sha256": result["brief_sha256"],
            "brief_object_sha256": result["brief_object_sha256"],
            "reference_id": result["reference_id"],
            "reference_object_sha256": result["reference_object_sha256"],
            "reference_evidence_sha256": result["reference_evidence_sha256"],
            "quality_contract_id": result["quality_contract_id"],
            "quality_contract_sha256": result["quality_contract_sha256"],
            "quality_contract_object_sha256": result["quality_contract_object_sha256"],
            "source_candidate_id": result["source_candidate_id"],
            "source_candidate_state_sha256": result["source_candidate_state_sha256"],
            "authoring_mesh_id": result["authoring_mesh_id"],
            "authoring_mesh_lineage_id": result["authoring_mesh_lineage_id"],
            "authoring_mesh_revision_id": result["authoring_mesh_revision_id"],
            "authoring_mesh_revision_index": result["authoring_mesh_revision_index"],
            "authoring_mesh_revision_sha256": result["authoring_mesh_revision_sha256"],
            "authoring_mesh_revision_object_sha256": result["authoring_mesh_revision_object_sha256"],
            "authoring_mesh_identity_sha256": result["authoring_mesh_identity_sha256"],
            "max_response_bytes": MAX_BYTES,
            "runtime_write_performed": false,
            "persistent_user_data_touched": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn file_runtime(label: &str) -> (Runtime, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "forgecad-source-binding-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("source binding test root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        (
            Runtime::open_with_cas(&database, &cas).expect("runtime"),
            database,
            cas,
        )
    }

    #[test]
    fn source_binding_positive_main_parser_is_closed_and_canonical() {
        let value = contract_fixture();
        let selectors = source_selectors(&value).expect("positive Main fixture");
        assert_eq!(selectors.source_binding_sha256, value["canonical_sha256"]);
        assert_eq!(selectors.project_id, value["project_id"]);
        assert_eq!(
            selectors.authoring_mesh_revision_index,
            value["authoring_mesh_revision_index"]
        );
    }

    #[test]
    fn source_binding_identity_hash_is_deterministic_and_revision_bound() {
        let first = source_identity_hash("mesh-a", "lineage-a", "revision-a", 0, &"a".repeat(64));
        let repeat = source_identity_hash("mesh-a", "lineage-a", "revision-a", 0, &"a".repeat(64));
        let changed = source_identity_hash("mesh-a", "lineage-a", "revision-a", 1, &"a".repeat(64));
        assert_eq!(first, repeat);
        assert_ne!(first, changed);
        assert_ne!(
            first,
            canonical_json_hash(&json!({
                "schema_version": "AuthoringMeshIdentity@1",
                "mesh_id": "mesh-a",
                "lineage_id": "lineage-a",
                "revision_id": "revision-a",
                "revision_index": 0
            }))
        );
    }

    #[test]
    fn source_binding_prepare_replay_get_and_restart_are_exact() {
        if super::super::build_cohort_sha256().is_none() {
            eprintln!("source binding live test requires FORGECAD_BUILD_COHORT_SHA256; parser tests remain active");
            return;
        }
        let (runtime, database, cas) = file_runtime("replay");
        let request = source_binding_request(&runtime, "replay");
        let first = runtime
            .knife_source_binding_prepare(&request)
            .expect("source binding prepare");
        assert_eq!(first["status"], "prepared");
        assert_eq!(first["runtime_write_performed"], true);
        assert_eq!(first["persistent_user_data_touched"], true);
        assert_eq!(first["high_mesh_created"], false);
        assert_eq!(first["visual_status"], "NOT_RUN");

        let replay = runtime
            .knife_source_binding_prepare(&request)
            .expect("source binding replay");
        assert_eq!(replay["status"], "replayed");
        assert_eq!(replay["replayed"], true);
        assert_eq!(replay["runtime_write_performed"], false);
        assert_eq!(replay["store_effect"], "not-touched");
        assert_eq!(replay["cas_effect"], "not-touched");

        let get = runtime
            .knife_source_binding_get(&get_request(&first))
            .expect("source binding exact get");
        assert_eq!(get["status"], "found");
        assert_eq!(get["idempotency_key"], Value::Null);
        assert_eq!(get["runtime_write_performed"], false);
        assert_eq!(get["source_binding"], first["source_binding"]);

        drop(runtime);
        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen Runtime");
        let restarted = reopened
            .knife_source_binding_get(&get_request(&first))
            .expect("source binding restart get");
        assert_eq!(restarted["status"], "found");
        assert_eq!(
            restarted["source_binding_sha256"],
            first["source_binding_sha256"]
        );
        assert_eq!(
            restarted["source_binding_object_sha256"],
            first["source_binding_object_sha256"]
        );
        drop(reopened);
        let _ = std::fs::remove_dir_all(database.parent().expect("source binding root"));
    }

    #[test]
    fn source_binding_same_key_conflict_and_late_lineage_rejection_leave_no_new_row_or_cas() {
        if super::super::build_cohort_sha256().is_none() {
            eprintln!("source binding live test requires FORGECAD_BUILD_COHORT_SHA256; parser tests remain active");
            return;
        }
        let runtime = Runtime::ephemeral().expect("runtime");
        let request = source_binding_request(&runtime, "conflict");
        let first = runtime
            .knife_source_binding_prepare(&request)
            .expect("initial source binding prepare");

        let mut same_key = request.clone();
        same_key["source_binding"]["source_binding_id"] =
            Value::String("source-binding-conflict-retarget".to_owned());
        same_key["source_binding"]["canonical_sha256"] = Value::String(String::new());
        same_key["source_binding"]["canonical_sha256"] =
            Value::String(canonical_json_hash(&same_key["source_binding"]));
        same_key["input_sha256"] = Value::String(String::new());
        same_key["input_sha256"] = Value::String(canonical_json_hash(&same_key));
        let error = runtime
            .knife_source_binding_prepare(&same_key)
            .expect_err("same idempotency key must conflict");
        assert!(error
            .to_string()
            .contains("KNIFE_SOURCE_BINDING_IDEMPOTENCY_CONFLICT"));

        let mut late = request;
        late["source_binding"]["source_binding_id"] =
            Value::String("source-binding-late-conflict".to_owned());
        late["source_binding"]["canonical_sha256"] = Value::String(String::new());
        late["source_binding"]["canonical_sha256"] =
            Value::String(canonical_json_hash(&late["source_binding"]));
        late["idempotency_key"] = Value::String("source-binding-late-key".to_owned());
        late["input_sha256"] = Value::String(String::new());
        late["input_sha256"] = Value::String(canonical_json_hash(&late));
        let staged_main_hash =
            sha256_hex(&canonical_json_bytes(&late["source_binding"]).expect("late Main bytes"));
        let error = runtime
            .knife_source_binding_prepare(&late)
            .expect_err("duplicate intent lineage must reject after staging");
        assert!(error
            .to_string()
            .contains("KNIFE_SOURCE_BINDING_INTENT_CONFLICT"));
        let late_semantic = late["source_binding"]["canonical_sha256"]
            .as_str()
            .expect("late semantic hash");
        assert!(runtime
            .store
            .get_knife_source_binding(
                late["project_id"].as_str().expect("project"),
                late["source_binding"]["source_binding_id"]
                    .as_str()
                    .expect("late id"),
                late_semantic,
            )
            .expect("late Store lookup")
            .is_none());
        assert!(runtime
            .store
            .get_object(&staged_main_hash)
            .expect("late CAS lookup")
            .is_none());
        assert_eq!(first["status"], "prepared");
    }
}
