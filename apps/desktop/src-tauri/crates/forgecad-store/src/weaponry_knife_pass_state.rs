//! Durable Store/CAS ownership for one closed `KnifePassState@1` pass.
//!
//! The Runtime supplies a validated Main payload and a CAS registration.  The
//! Store re-verifies the canonical bytes, resolves every upstream identity
//! from its durable row, and commits one immutable index row together with
//! the reachable CAS roots.  No path, URL, script, secret or caller-provided
//! object hash can become product truth here.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    mark_reachable_in_transaction, CasObjectRecord, CasStore,
    KnifeReferenceIntentBundleStoreRecord, KnifeSourceBindingStoreRecord, Store, StoreError,
    WeaponryCurveEvaluatedMeshDurableRecord, WeaponryCurveModifierGraphDurableRecord,
    WeaponryKnifeProductionBriefStoreRecord, AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
    WEAPONRY_CURVE_EVALUATED_MESH_JSON_MIME, WEAPONRY_CURVE_EVALUATED_MESH_MAX_JSON_BYTES,
    WEAPONRY_CURVE_EVALUATED_MESH_RECORD_SCHEMA, WEAPONRY_CURVE_EVALUATED_MESH_STATUS,
    WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND, WEAPONRY_CURVE_MODIFIER_GRAPH_JSON_MIME,
    WEAPONRY_CURVE_MODIFIER_GRAPH_MAX_JSON_BYTES, WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_SCHEMA,
    WEAPONRY_CURVE_MODIFIER_GRAPH_STATUS, WEAPONRY_CURVE_SET_OBJECT_KIND,
    WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND, WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND,
    WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND, WEAPONRY_EVALUATED_MESH_OBJECT_KIND,
    WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND, WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
    WEAPONRY_SAMPLE_SET_OBJECT_KIND,
};
use forgecad_contracts::AuthoringMeshRevision;
use forgecad_core::sha256_hex;
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const KNIFE_PASS_STATE_RECORD_SCHEMA_VERSION: &str = "KnifePassStateStoreRecord@1";
pub const KNIFE_PASS_STATE_SCHEMA_VERSION: &str = "KnifePassState@1";
pub const KNIFE_PASS_STATE_STATUS: &str = "runtime-owned-store-knife-pass-state@1";
pub const KNIFE_PASS_STATE_OBJECT_KIND: &str = "knife-pass-state";
pub const KNIFE_PASS_STATE_JSON_MIME: &str = "application/json";
pub const KNIFE_PASS_STATE_MAX_JSON_BYTES: u64 = 1024 * 1024;
pub const KNIFE_PASS_STATE_CANONICALIZATION_POLICY: &str =
    "canonical-json-sha256-excluding-canonical-sha256@1";
pub const KNIFE_PASS_STATE_EVIDENCE_BUNDLE_SCHEMA_VERSION: &str = "KnifeEvidenceBundle@1";
const MAX_IDEMPOTENCY_BYTES: usize = 128;
const MAX_TIMESTAMP_BYTES: usize = 128;
const MAX_LINEAGE_JSON_BYTES: u64 = 8 * 1024 * 1024;

/// Store-local immutable projection of one `KnifePassState@1` Main object.
///
/// `pass_state_object_sha256` and `idempotency_key` are Store metadata only;
/// `main_value` removes both before writing the canonical Main CAS object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnifePassStateStoreRecord {
    pub schema_version: String,
    pub pass_id: String,
    pub parent_pass_id: Option<String>,
    pub parent_pass_sha256: Option<String>,
    pub project_id: String,
    pub stage: String,
    pub source_binding_id: String,
    pub source_binding_sha256: String,
    pub source_binding_object_sha256: String,
    pub intent_bundle_id: String,
    pub intent_bundle_sha256: String,
    pub intent_bundle_object_sha256: String,
    pub brief_id: String,
    pub brief_sha256: String,
    pub brief_object_sha256: String,
    pub reference_id: String,
    pub reference_object_sha256: String,
    pub reference_evidence_sha256: String,
    pub source_candidate_id: String,
    pub source_candidate_state_sha256: String,
    pub baseline_candidate_id: String,
    pub baseline_candidate_state_sha256: String,
    pub baseline_artifact_sha256: String,
    /// Canonical GeometryProgram semantic/object identity for the baseline
    /// candidate.  These are explicit so a later correction cannot be
    /// confused with the source candidate merely because its state hash is
    /// similar.
    pub baseline_geometry_program_sha256: String,
    pub baseline_geometry_program_object_sha256: String,
    pub baseline_artifact_readback_object_sha256: String,
    pub baseline_representation_plan_sha256: String,
    pub attempt_candidate_id: String,
    pub attempt_candidate_state_sha256: String,
    pub attempt_artifact_sha256: String,
    /// Exact materializer lineage for the candidate used by this pass.
    pub attempt_geometry_program_sha256: String,
    pub attempt_geometry_program_object_sha256: String,
    pub attempt_artifact_readback_object_sha256: String,
    pub attempt_representation_plan_sha256: String,
    pub authoring_mesh_id: String,
    pub authoring_mesh_lineage_id: String,
    pub authoring_mesh_revision_id: String,
    pub authoring_mesh_revision_index: u64,
    pub authoring_mesh_revision_sha256: String,
    pub authoring_mesh_revision_object_sha256: String,
    pub authoring_mesh_identity_sha256: String,
    pub authoring_mesh_sha256: String,
    pub modifier_graph_id: Option<String>,
    pub modifier_graph_sha256: Option<String>,
    pub evaluated_mesh_id: Option<String>,
    pub evaluated_mesh_sha256: Option<String>,
    pub high_artifact_id: Option<String>,
    pub high_artifact_sha256: Option<String>,
    pub fixed_view: Value,
    pub camera_set_sha256: String,
    pub render_set_id: String,
    pub render_set_sha256: String,
    pub render_set_object_sha256: String,
    pub reference_comparison_id: String,
    pub reference_comparison_sha256: String,
    pub reference_comparison_object_sha256: String,
    pub quality_report_id: String,
    pub quality_report_sha256: String,
    pub quality_report_object_sha256: String,
    pub evidence_bundle_sha256: String,
    pub hard_gate_status: String,
    pub visual_gate_status: String,
    pub quality_status: String,
    pub high_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub unknowns: Vec<Value>,
    pub unlocked_successor: String,
    pub high_mesh_created: bool,
    pub high_stage_unlocked: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
    pub pass_state_object_sha256: String,
    pub idempotency_key: String,
    pub created_at: String,
}

/// The Main object staged in CAS before the Store transaction.
#[derive(Debug, Clone)]
pub struct KnifePassStateCasBundle {
    pub pass_state: CasObjectRecord,
}

#[derive(Debug, Clone)]
pub struct KnifePassStateCommit {
    pub record: KnifePassStateStoreRecord,
    pub cas: KnifePassStateCasBundle,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn record_value(record: &KnifePassStateStoreRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn record_bytes(record: &KnifePassStateStoreRecord) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&record_value(record)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

/// Convert the Store projection to the exact closed Main object.  The
/// external CAS object hash and request idempotency are intentionally absent.
pub fn main_value(record: &KnifePassStateStoreRecord) -> Result<Value, StoreError> {
    let mut object = record_value(record)?.as_object().cloned().ok_or_else(|| {
        StoreError::InvalidData("KnifePassState record is not an object".to_owned())
    })?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(KNIFE_PASS_STATE_SCHEMA_VERSION.to_owned()),
    );
    object.remove("pass_state_object_sha256");
    object.remove("idempotency_key");
    Ok(Value::Object(object))
}

/// Build a Store projection from a Main value supplied by Runtime.  The
/// caller still must provide a registered CAS object; this helper never writes
/// or invents that object.
pub fn record_from_main_value(
    value: Value,
    pass_state_object_sha256: impl Into<String>,
    idempotency_key: impl Into<String>,
) -> Result<KnifePassStateStoreRecord, StoreError> {
    if !value.is_object()
        || value.get("pass_state_object_sha256").is_some()
        || value.get("idempotency_key").is_some()
    {
        return Err(contract(
            "KNIFE_PASS_STATE_MAIN_FIELDS_INVALID",
            "Main value must be closed and must not contain Store metadata",
        ));
    }
    let mut object = value.as_object().cloned().ok_or_else(|| {
        StoreError::InvalidData("KnifePassState Main is not an object".to_owned())
    })?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(KNIFE_PASS_STATE_RECORD_SCHEMA_VERSION.to_owned()),
    );
    object.insert(
        "pass_state_object_sha256".to_owned(),
        Value::String(pass_state_object_sha256.into()),
    );
    object.insert(
        "idempotency_key".to_owned(),
        Value::String(idempotency_key.into()),
    );
    let record: KnifePassStateStoreRecord = serde_json::from_value(Value::Object(object))
        .map_err(|error| contract("KNIFE_PASS_STATE_RECORD_INVALID", error.to_string()))?;
    validate_record(&record)?;
    Ok(record)
}

fn validate_identifier(value: &str) -> bool {
    is_opaque_id(value)
}

fn validate_optional_identifier(value: Option<&str>) -> bool {
    value.is_none_or(validate_identifier)
}

fn validate_optional_hash(value: Option<&str>) -> bool {
    value.is_none_or(is_sha256)
}

fn validate_fixed_view(record: &KnifePassStateStoreRecord) -> Result<(), StoreError> {
    let object = record.fixed_view.as_object().ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_FIXED_VIEW_INVALID",
            "fixed_view must be one closed object",
        )
    })?;
    const REQUIRED: &[&str] = &[
        "view_id",
        "view_kind",
        "comparison_role",
        "reference_required",
        "camera_id",
        "camera_sha256",
        "reference_view_id",
        "reference_view_sha256",
        "fixed_view_policy",
    ];
    if object.len() != REQUIRED.len() || REQUIRED.iter().any(|key| !object.contains_key(*key)) {
        return Err(contract(
            "KNIFE_PASS_STATE_FIXED_VIEW_INVALID",
            "fixed_view must contain exactly the closed nine-field view shape",
        ));
    }
    if !validate_identifier(object["view_id"].as_str().unwrap_or_default())
        || !matches!(
            object["view_kind"].as_str(),
            Some(
                "front"
                    | "back"
                    | "left"
                    | "right"
                    | "front-three-quarter"
                    | "rear-three-quarter"
                    | "top"
                    | "bottom"
                    | "fps-inspect"
            )
        )
        || object["comparison_role"].as_str() != Some("primary-reference")
        || object["reference_required"].as_bool() != Some(true)
        || !validate_identifier(object["camera_id"].as_str().unwrap_or_default())
        || !is_sha256(object["camera_sha256"].as_str().unwrap_or_default())
        || !validate_identifier(object["reference_view_id"].as_str().unwrap_or_default())
        || !is_sha256(object["reference_view_sha256"].as_str().unwrap_or_default())
        || object["fixed_view_policy"].as_str()
            != Some("single-runtime-bound-primary-reference-view@1")
    {
        return Err(contract(
            "KNIFE_PASS_STATE_FIXED_VIEW_BINDING_MISMATCH",
            "fixed_view is not bound to the single runtime reference",
        ));
    }
    let camera_set = json!({
        "schema_version": "KnifeCameraSet@1",
        "fixed_views": [record.fixed_view.clone()],
        "fixed_view_count": 1,
    });
    if canonical_json_hash(&camera_set) != record.camera_set_sha256 {
        return Err(contract(
            "KNIFE_PASS_STATE_CAMERA_SET_MISMATCH",
            "camera_set_sha256 does not match the one bounded fixed view",
        ));
    }
    Ok(())
}

fn validate_unknowns(record: &KnifePassStateStoreRecord) -> Result<(), StoreError> {
    if record.unknowns.is_empty() || record.unknowns.len() > 16 {
        return Err(contract(
            "KNIFE_PASS_STATE_UNKNOWNS_INVALID",
            "a pass must preserve one to sixteen blocking unknowns",
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for value in &record.unknowns {
        let object = value.as_object().ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_UNKNOWNS_INVALID",
                "unknown is not an object",
            )
        })?;
        const REQUIRED: &[&str] = &[
            "unknown_id",
            "category",
            "view_kind",
            "description",
            "impact",
            "status",
        ];
        if object.len() != REQUIRED.len() || REQUIRED.iter().any(|key| !object.contains_key(*key)) {
            return Err(contract(
                "KNIFE_PASS_STATE_UNKNOWNS_INVALID",
                "unknown fields are not closed",
            ));
        }
        let id = object["unknown_id"].as_str().unwrap_or_default();
        let description = object["description"].as_str().unwrap_or_default();
        if !validate_identifier(id)
            || !ids.insert(id.to_owned())
            || !matches!(
                object["category"].as_str(),
                Some(
                    "reference-coverage"
                        | "silhouette"
                        | "proportion"
                        | "material"
                        | "topology"
                        | "lineage"
                )
            )
            || !matches!(
                object["view_kind"].as_str(),
                Some(
                    "front"
                        | "back"
                        | "left"
                        | "right"
                        | "front-three-quarter"
                        | "rear-three-quarter"
                        | "top"
                        | "bottom"
                        | "fps-inspect"
                )
            )
            || description.is_empty()
            || description.len() > 1024
            || description.chars().any(|character| character.is_control())
            || object["impact"].as_str() != Some("blocking")
            || object["status"].as_str() != Some("open")
        {
            return Err(contract(
                "KNIFE_PASS_STATE_UNKNOWNS_INVALID",
                "unknown is not a unique open blocking item",
            ));
        }
    }

    Ok(())
}

fn validate_record(record: &KnifePassStateStoreRecord) -> Result<(), StoreError> {
    let ids = [
        record.pass_id.as_str(),
        record.project_id.as_str(),
        record.source_binding_id.as_str(),
        record.intent_bundle_id.as_str(),
        record.brief_id.as_str(),
        record.reference_id.as_str(),
        record.source_candidate_id.as_str(),
        record.baseline_candidate_id.as_str(),
        record.attempt_candidate_id.as_str(),
        record.authoring_mesh_id.as_str(),
        record.authoring_mesh_lineage_id.as_str(),
        record.authoring_mesh_revision_id.as_str(),
        record.render_set_id.as_str(),
        record.reference_comparison_id.as_str(),
        record.quality_report_id.as_str(),
        record.idempotency_key.as_str(),
    ];
    let hashes = [
        record.source_binding_sha256.as_str(),
        record.source_binding_object_sha256.as_str(),
        record.intent_bundle_sha256.as_str(),
        record.intent_bundle_object_sha256.as_str(),
        record.brief_sha256.as_str(),
        record.brief_object_sha256.as_str(),
        record.reference_object_sha256.as_str(),
        record.reference_evidence_sha256.as_str(),
        record.source_candidate_state_sha256.as_str(),
        record.baseline_candidate_state_sha256.as_str(),
        record.baseline_artifact_sha256.as_str(),
        record.baseline_geometry_program_sha256.as_str(),
        record.baseline_geometry_program_object_sha256.as_str(),
        record.baseline_artifact_readback_object_sha256.as_str(),
        record.baseline_representation_plan_sha256.as_str(),
        record.attempt_candidate_state_sha256.as_str(),
        record.attempt_artifact_sha256.as_str(),
        record.attempt_geometry_program_sha256.as_str(),
        record.attempt_geometry_program_object_sha256.as_str(),
        record.attempt_artifact_readback_object_sha256.as_str(),
        record.attempt_representation_plan_sha256.as_str(),
        record.authoring_mesh_revision_sha256.as_str(),
        record.authoring_mesh_revision_object_sha256.as_str(),
        record.authoring_mesh_identity_sha256.as_str(),
        record.authoring_mesh_sha256.as_str(),
        record.camera_set_sha256.as_str(),
        record.render_set_sha256.as_str(),
        record.render_set_object_sha256.as_str(),
        record.reference_comparison_sha256.as_str(),
        record.reference_comparison_object_sha256.as_str(),
        record.quality_report_sha256.as_str(),
        record.quality_report_object_sha256.as_str(),
        record.evidence_bundle_sha256.as_str(),
        record.canonical_sha256.as_str(),
        record.pass_state_object_sha256.as_str(),
    ];
    if record.schema_version != KNIFE_PASS_STATE_RECORD_SCHEMA_VERSION
        || ids.iter().any(|value| !validate_identifier(value))
        || hashes.iter().any(|value| !is_sha256(value))
        || !validate_optional_identifier(record.parent_pass_id.as_deref())
        || !validate_optional_hash(record.parent_pass_sha256.as_deref())
        || (record.parent_pass_id.is_some() != record.parent_pass_sha256.is_some())
        || !validate_optional_identifier(record.modifier_graph_id.as_deref())
        || !validate_optional_hash(record.modifier_graph_sha256.as_deref())
        || !validate_optional_identifier(record.evaluated_mesh_id.as_deref())
        || !validate_optional_hash(record.evaluated_mesh_sha256.as_deref())
        || !validate_optional_identifier(record.high_artifact_id.as_deref())
        || !validate_optional_hash(record.high_artifact_sha256.as_deref())
        || record.authoring_mesh_revision_index > 1_000_000
        || record.idempotency_key.len() > MAX_IDEMPOTENCY_BYTES
        || record.created_at.is_empty()
        || record.created_at.len() > MAX_TIMESTAMP_BYTES
        || record.created_at.contains('/')
        || record.created_at.contains('\\')
        || !matches!(
            record.stage.as_str(),
            "camera-lock"
                | "silhouette-blockout"
                | "structural-form"
                | "secondary-form"
                | "high-geometry"
        )
        || record.canonicalization_policy != KNIFE_PASS_STATE_CANONICALIZATION_POLICY
        || !matches!(
            record.hard_gate_status.as_str(),
            "NOT_RUN" | "BLOCKED" | "FAIL" | "PASS_SOURCE_STRUCTURAL"
        )
        || !matches!(
            record.visual_gate_status.as_str(),
            "NOT_RUN" | "QUALITY_TARGET_NOT_MET" | "BLOCKED_REFERENCE_COVERAGE"
        )
        || !matches!(
            record.quality_status.as_str(),
            "NOT_RUN" | "QUALITY_TARGET_NOT_MET" | "BLOCKED_REFERENCE_COVERAGE"
        )
        || !matches!(record.high_status.as_str(), "NOT_RUN" | "BLOCKED")
        || record.human_status != "NOT_RUN"
        || record.engine_status != "NOT_RUN"
        || record.unlocked_successor != "none"
        || record.high_mesh_created
        || record.high_stage_unlocked
        || record.production_stage_advanced
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
    {
        return Err(contract(
            "KNIFE_PASS_STATE_RECORD_INVALID",
            "pass state identity, status, timestamp or promotion fields are invalid",
        ));
    }
    if record.modifier_graph_id.is_some() != record.modifier_graph_sha256.is_some()
        || record.evaluated_mesh_id.is_some() != record.evaluated_mesh_sha256.is_some()
        || record.high_artifact_id.is_some() != record.high_artifact_sha256.is_some()
    {
        return Err(contract(
            "KNIFE_PASS_STATE_OPTIONAL_BINDING_INVALID",
            "optional modifier/evaluation/High fields must be paired",
        ));
    }
    validate_fixed_view(record)?;
    validate_unknowns(record)?;

    let mut main = main_value(record)?;
    main["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&main) != record.canonical_sha256 {
        return Err(contract(
            "KNIFE_PASS_STATE_CANONICAL_MISMATCH",
            "Main canonical hash differs from its closed fields",
        ));
    }
    // Cross-record derivation (optional curve identities, candidate evidence,
    // materializer and visual status) is deliberately performed only after a
    // transaction has resolved the durable rows.  `validate_record` remains a
    // closed-shape check and never invents synthetic upstream hashes.
    Ok(())
}

fn read_object_record(
    transaction: &Transaction<'_>,
    sha256: &str,
) -> Result<CasObjectRecord, StoreError> {
    transaction
        .query_row(
            "SELECT sha256, size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
            params![sha256],
            |row| {
                let size: i64 = row.get(1)?;
                Ok(CasObjectRecord {
                    schema_version: "CasObject@1".to_owned(),
                    sha256: row.get(0)?,
                    size_bytes: u64::try_from(size).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    mime: row.get(2)?,
                    kind: row.get(3)?,
                    reachability: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(StoreError::from)
}

fn validate_registered_cas(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    hash: &str,
    expected_kinds: &[&str],
    expected_schemas: &[&str],
    expected_semantic: Option<&str>,
    require_reachable: bool,
    role: &str,
) -> Result<Value, StoreError> {
    if !is_sha256(hash) {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_METADATA_INVALID",
            format!("{role} object hash is invalid"),
        ));
    }
    let object = read_object_record(transaction, hash).map_err(|error| match error {
        StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
            "KNIFE_PASS_STATE_CAS_MISSING",
            format!("{role} object is not registered"),
        ),
        other => other,
    })?;
    if object.schema_version != "CasObject@1"
        || object.sha256 != hash
        || object.mime != KNIFE_PASS_STATE_JSON_MIME
        || !expected_kinds.iter().any(|kind| object.kind == *kind)
        || object.size_bytes == 0
        || object.size_bytes > MAX_LINEAGE_JSON_BYTES
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && object.reachability != "reachable")
    {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_METADATA_INVALID",
            format!("{role} object metadata is outside the bounded allowlist"),
        ));
    }
    let bytes = cas
        .read_verified_bounded(hash, MAX_LINEAGE_JSON_BYTES)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != hash {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_HASH_MISMATCH",
            format!("{role} bytes do not match their registered object hash"),
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        contract(
            "KNIFE_PASS_STATE_CAS_JSON_INVALID",
            format!("{role} CAS JSON is invalid: {error}"),
        )
    })?;
    if !expected_schemas.is_empty()
        && !expected_schemas
            .iter()
            .any(|schema| value.get("schema_version").and_then(Value::as_str) == Some(*schema))
    {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_SCHEMA_INVALID",
            format!("{role} CAS schema is not an allowed evidence schema"),
        ));
    }
    if canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?
        != bytes
    {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_CANONICAL_MISMATCH",
            format!("{role} CAS JSON is not canonical"),
        ));
    }
    if let Some(expected) = expected_semantic {
        let supplied = value
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .ok_or_else(|| {
                contract(
                    "KNIFE_PASS_STATE_EVIDENCE_CANONICAL_MISSING",
                    format!("{role} semantic canonical hash is missing"),
                )
            })?;
        let mut preimage = value.clone();
        preimage["canonical_sha256"] = Value::String(String::new());
        if supplied != expected || canonical_json_hash(&preimage) != expected {
            return Err(contract(
                "KNIFE_PASS_STATE_EVIDENCE_CANONICAL_MISMATCH",
                format!("{role} semantic hash differs from its Store binding"),
            ));
        }
    }
    Ok(value)
}

/// Validate a non-semantic CAS root whose identity is the exact object hash
/// stored by an upstream durable row.  Geometry GLB/readback, camera
/// calibration and reference masks all use this path: unlike a Main or
/// report object, their bytes must not be re-hashed into a synthetic semantic
/// identity by PassState.
fn validate_registered_bytes(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    hash: &str,
    expected_kinds: &[&str],
    expected_mimes: &[&str],
    max_bytes: u64,
    require_reachable: bool,
    role: &str,
) -> Result<(CasObjectRecord, Vec<u8>), StoreError> {
    if !is_sha256(hash) {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_METADATA_INVALID",
            format!("{role} object hash is invalid"),
        ));
    }
    let object = read_object_record(transaction, hash).map_err(|error| match error {
        StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
            "KNIFE_PASS_STATE_CAS_MISSING",
            format!("{role} object is not registered"),
        ),
        other => other,
    })?;
    if object.schema_version != "CasObject@1"
        || object.sha256 != hash
        || !expected_mimes.iter().any(|mime| object.mime == *mime)
        || !expected_kinds.iter().any(|kind| object.kind == *kind)
        || object.size_bytes == 0
        || object.size_bytes > max_bytes
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && object.reachability != "reachable")
    {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_METADATA_INVALID",
            format!("{role} object metadata is outside the bounded allowlist"),
        ));
    }
    let bytes = cas
        .read_verified_bounded(hash, max_bytes)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != hash {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_HASH_MISMATCH",
            format!("{role} bytes do not match their registered object hash"),
        ));
    }
    Ok((object, bytes))
}

fn canonical_json_value(bytes: &[u8], role: &str) -> Result<Value, StoreError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "KNIFE_PASS_STATE_CAS_JSON_INVALID",
            format!("{role} CAS JSON is invalid: {error}"),
        )
    })?;
    if canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?
        != bytes
    {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_CANONICAL_MISMATCH",
            format!("{role} CAS JSON is not canonical"),
        ));
    }
    Ok(value)
}

/// This is the Store-side copy of the closed Runtime projection used by
/// `authoring_mesh_v2_candidate_materializer`.  Keeping the projection
/// preimage here lets a restart validate the materializer output without
/// treating a caller-provided representation-plan hash as authority.
fn authoring_mesh_geometry_parameters(
    revision: &AuthoringMeshRevision,
    position_m: [f64; 3],
    rotation_rad: [f64; 3],
) -> Result<Value, StoreError> {
    if position_m
        .iter()
        .chain(rotation_rad.iter())
        .any(|value| !value.is_finite())
    {
        return Err(contract(
            "KNIFE_PASS_STATE_MATERIALIZER_PROJECTION_INVALID",
            "materializer transform contains a non-finite component",
        ));
    }
    let edges_by_id = revision
        .original
        .edges
        .iter()
        .map(|edge| (edge.edge_id.0.as_str(), edge))
        .collect::<BTreeMap<_, _>>();
    let half_edges_by_id = revision
        .original
        .half_edges
        .iter()
        .map(|half_edge| (half_edge.half_edge_id.0.as_str(), half_edge))
        .collect::<BTreeMap<_, _>>();
    let mut loops = Vec::new();
    let mut faces = Vec::new();
    let mut referenced_vertex_ids = BTreeSet::new();
    let mut referenced_edge_ids = BTreeSet::new();
    for face in &revision.original.faces {
        if !(3..=4).contains(&face.half_edge_ids.len()) {
            return Err(contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PROJECTION_UNSUPPORTED",
                format!(
                    "face {} is not triangle/quad Worker-compatible",
                    face.face_id.0
                ),
            ));
        }
        let mut ordered = face
            .half_edge_ids
            .iter()
            .map(|id| {
                half_edges_by_id.get(id.0.as_str()).copied().ok_or_else(|| {
                    contract(
                        "KNIFE_PASS_STATE_MATERIALIZER_PROJECTION_INVALID",
                        "face references an unknown half-edge",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let first = ordered
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.corner_id.0.cmp(&right.corner_id.0))
            .map(|(index, _)| index)
            .ok_or_else(|| {
                contract(
                    "KNIFE_PASS_STATE_MATERIALIZER_PROJECTION_INVALID",
                    "face has no half-edges",
                )
            })?;
        ordered.rotate_left(first);
        let mut face_loop_ids = Vec::with_capacity(ordered.len());
        for (ordinal, half_edge) in ordered.iter().enumerate() {
            let next = ordered[(ordinal + 1) % ordered.len()];
            let edge = edges_by_id
                .get(half_edge.edge_id.0.as_str())
                .copied()
                .ok_or_else(|| {
                    contract(
                        "KNIFE_PASS_STATE_MATERIALIZER_PROJECTION_INVALID",
                        "half-edge references an unknown edge",
                    )
                })?;
            let mut endpoints = [edge.vertex_ids[0].0.as_str(), edge.vertex_ids[1].0.as_str()];
            endpoints.sort();
            let origin = half_edge.origin_vertex_id.0.as_str();
            let target = next.origin_vertex_id.0.as_str();
            let edge_forward = if (origin == endpoints[0] && target == endpoints[1])
                || (origin == endpoints[1] && target == endpoints[0])
            {
                origin == endpoints[0] && target == endpoints[1]
            } else {
                return Err(contract(
                    "KNIFE_PASS_STATE_MATERIALIZER_PROJECTION_INVALID",
                    "half-edge direction differs from its edge endpoints",
                ));
            };
            referenced_vertex_ids.insert(half_edge.origin_vertex_id.0.as_str());
            referenced_edge_ids.insert(half_edge.edge_id.0.as_str());
            face_loop_ids.push(half_edge.corner_id.0.clone());
            loops.push(json!({
                "element_id":half_edge.corner_id.0,
                "face_id":face.face_id.0,
                "ordinal":ordinal,
                "vertex_id":half_edge.origin_vertex_id.0,
                "edge_id":half_edge.edge_id.0,
                "edge_forward":edge_forward,
            }));
        }
        faces.push(json!({"element_id":face.face_id.0,"loop_ids":face_loop_ids}));
    }
    loops.sort_by(|left, right| {
        left["element_id"]
            .as_str()
            .cmp(&right["element_id"].as_str())
    });
    let vertices = revision
        .original
        .vertices
        .iter()
        .filter(|vertex| referenced_vertex_ids.contains(vertex.vertex_id.0.as_str()))
        .map(|vertex| json!({"element_id":vertex.vertex_id.0,"position_m":vertex.position_m}))
        .collect::<Vec<_>>();
    let edges = revision
        .original
        .edges
        .iter()
        .filter(|edge| referenced_edge_ids.contains(edge.edge_id.0.as_str()))
        .map(|edge| {
            let mut endpoints = [edge.vertex_ids[0].0.clone(), edge.vertex_ids[1].0.clone()];
            endpoints.sort();
            json!({"element_id":edge.edge_id.0,"vertex_ids":endpoints})
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "shape":"authoring-mesh",
        "topology_policy":"triangle-quad-manifold-with-boundary@1",
        "vertices":vertices,
        "edges":edges,
        "loops":loops,
        "faces":faces,
        "position_m":position_m,
        "rotation_rad":rotation_rad,
    }))
}

#[derive(Debug, Clone)]
struct SourceMaterializationInputs {
    source_program: Value,
    source_candidate_id: String,
    source_candidate_state_sha256: String,
    source_artifact_sha256: String,
    source_artifact_readback_object_sha256: String,
    source_artifact_readback_sha256: String,
    source_quality_report_object_sha256: String,
    source_program_sha256: String,
    source_program_object_sha256: String,
    source_binding_id: String,
    source_binding_sha256: String,
    source_binding_object_sha256: String,
    source_node_id: String,
    source_part_id: String,
    source_material_zone_id: String,
    source_solid: bool,
    source_position_m: [f64; 3],
    source_rotation_rad: [f64; 3],
    source_part_output_sha256: String,
}

fn source_materialization_inputs(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
    revision: &AuthoringMeshRevision,
) -> Result<SourceMaterializationInputs, StoreError> {
    let binding = revision.source_binding.as_ref().ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_MATERIALIZER_SOURCE_BINDING_MISSING",
            "source-bound materializer plan requires an embedded AuthoringMesh source binding",
        )
    })?;
    if binding.schema_version != "AuthoringMeshV2SourceBinding@1"
        || binding.project_id != record.project_id
        || binding.candidate_id != record.source_candidate_id
        || binding.candidate_state_sha256 != record.source_candidate_state_sha256
        || binding.geometry_program_sha256.is_empty()
        || binding.artifact_sha256.is_empty()
        || binding.artifact_readback_sha256.is_empty()
        || binding.source_node_id.is_empty()
        || binding.part_id.is_empty()
        || binding.material_zone_id.is_empty()
        || !is_sha256(&binding.source_parameters_sha256)
        || !is_sha256(&binding.part_output_sha256)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_MATERIALIZER_SOURCE_BINDING_INVALID",
            "embedded source binding is not a valid source-bound materializer input",
        ));
    }
    let candidate: Option<(String, String, Option<String>, String, i64)> = transaction
        .query_row(
            "SELECT project_id, canonical_sha256, prepared_object_sha256, state, quality_hard_gate_passed FROM candidates WHERE candidate_id = ?1",
            params![record.source_candidate_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((candidate_project, candidate_state, prepared_object, candidate_status, hard_gate)) =
        candidate
    else {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_CANDIDATE_MISSING",
            "source candidate is not durably registered",
        ));
    };
    if candidate_project != record.project_id
        || candidate_state != record.source_candidate_state_sha256
        || candidate_status != "reviewable"
        || hard_gate == 0
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_CANDIDATE_MISMATCH",
            "source candidate is not the exact reviewable structural source",
        ));
    }
    let evidence: Option<(String, Option<String>, Option<String>, String, String, String, String, String, String)> = transaction
        .query_row(
            "SELECT project_id, reference_id, reference_sha256, geometry_program_sha256, geometry_program_object_sha256, operator_catalog_sha256, artifact_object_sha256, artifact_readback_object_sha256, quality_report_object_sha256 FROM geometry_candidate_evidence WHERE candidate_id = ?1",
            params![record.source_candidate_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?)),
        )
        .optional()?;
    let Some((
        evidence_project,
        evidence_reference_id,
        evidence_reference_sha256,
        program_sha256,
        program_object_sha256,
        operator_catalog_sha256,
        artifact_sha256,
        readback_object_sha256,
        quality_object_sha256,
    )) = evidence
    else {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_GEOMETRY_EVIDENCE_MISSING",
            "source candidate GeometryCandidateEvidence is not durably registered",
        ));
    };
    if evidence_project != record.project_id
        || evidence_reference_id.as_deref() != Some(record.reference_id.as_str())
        || evidence_reference_sha256.as_deref() != Some(record.reference_object_sha256.as_str())
        || prepared_object.as_deref() != Some(artifact_sha256.as_str())
        || binding.artifact_sha256 != artifact_sha256
        || binding.geometry_program_sha256 != program_sha256
        || !is_sha256(&operator_catalog_sha256)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_GEOMETRY_EVIDENCE_MISMATCH",
            "source candidate artifact/program differs from embedded source binding",
        ));
    }
    let (_source_program_object, source_program_bytes) = validate_registered_bytes(
        transaction,
        cas,
        &program_object_sha256,
        &["geometry-program-v2"],
        &[KNIFE_PASS_STATE_JSON_MIME],
        MAX_LINEAGE_JSON_BYTES,
        true,
        "source GeometryProgram",
    )?;
    let source_program = canonical_json_value(&source_program_bytes, "source GeometryProgram")?;
    if source_program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || source_program.get("canonical_sha256").is_some()
        || canonical_json_hash(&source_program) != program_sha256
        || source_program.get("project_id").and_then(Value::as_str)
            != Some(record.project_id.as_str())
        || source_program
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(operator_catalog_sha256.as_str())
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_GEOMETRY_PROGRAM_INVALID",
            "source GeometryProgram is not the canonical Runtime draft",
        ));
    }
    let (_artifact_object, _artifact_bytes) = validate_registered_bytes(
        transaction,
        cas,
        &artifact_sha256,
        &[
            "geometry-glb",
            "geometry-artifact",
            "appearance-glb",
            "appearance-v2-glb",
        ],
        &["model/gltf-binary"],
        MAX_LINEAGE_JSON_BYTES * 8,
        true,
        "source geometry artifact",
    )?;
    let (_readback_object, readback_bytes) = validate_registered_bytes(
        transaction,
        cas,
        &readback_object_sha256,
        &[
            "geometry-artifact-readback-v2",
            "appearance-v2-artifact-readback",
            "artifact-readback",
        ],
        &[KNIFE_PASS_STATE_JSON_MIME],
        MAX_LINEAGE_JSON_BYTES,
        true,
        "source artifact readback",
    )?;
    let readback = canonical_json_value(&readback_bytes, "source artifact readback")?;
    let readback_canonical = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_SOURCE_READBACK_INVALID",
                "source artifact readback semantic hash is missing",
            )
        })?;
    if readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
        || readback.get("candidate_id").and_then(Value::as_str)
            != Some(record.source_candidate_id.as_str())
        || readback.get("object_sha256").and_then(Value::as_str) != Some(artifact_sha256.as_str())
        || readback.get("program_sha256").and_then(Value::as_str) != Some(program_sha256.as_str())
        || binding.artifact_readback_sha256 != readback_canonical
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_READBACK_MISMATCH",
            "source artifact readback differs from embedded source binding",
        ));
    }
    let (_quality_object, quality_bytes) = validate_registered_bytes(
        transaction,
        cas,
        &quality_object_sha256,
        &[
            "geometry-quality-report",
            "quality-report",
            "appearance-quality-report",
            "appearance-v2-quality-report",
        ],
        &[KNIFE_PASS_STATE_JSON_MIME],
        MAX_LINEAGE_JSON_BYTES,
        true,
        "source geometry quality report",
    )?;
    let quality = canonical_json_value(&quality_bytes, "source geometry quality report")?;
    if quality.get("candidate_id").and_then(Value::as_str)
        != Some(record.source_candidate_id.as_str())
        || quality.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256.as_str())
        || quality.get("program_sha256").and_then(Value::as_str) != Some(program_sha256.as_str())
        || quality.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_GEOMETRY_QUALITY_MISMATCH",
            "source candidate geometry quality report differs from its evidence",
        ));
    }
    let nodes = source_program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_SOURCE_GEOMETRY_PROGRAM_INVALID",
                "source GeometryProgram nodes are missing",
            )
        })?;
    let source_nodes = nodes
        .iter()
        .filter(|node| {
            node.get("node_id").and_then(Value::as_str) == Some(binding.source_node_id.as_str())
        })
        .collect::<Vec<_>>();
    if source_nodes.len() != 1
        || source_nodes[0].get("operator_id").and_then(Value::as_str)
            != Some(binding.source_operator_id.as_str())
        || source_nodes[0]
            .get("inputs")
            .and_then(Value::as_array)
            .is_none_or(|inputs| !inputs.is_empty())
        || source_nodes[0].get("parameters").is_none()
        || canonical_json_hash(source_nodes[0].get("parameters").expect("checked above"))
            != binding.source_parameters_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_GEOMETRY_NODE_MISMATCH",
            "source GeometryProgram node differs from the source binding",
        ));
    }
    if nodes.iter().any(|node| {
        node.get("inputs")
            .and_then(Value::as_array)
            .is_some_and(|inputs| {
                inputs
                    .iter()
                    .any(|input| input.as_str() == Some(binding.source_node_id.as_str()))
            })
    }) {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_GEOMETRY_NODE_MISMATCH",
            "source node has a downstream consumer and is not replaceable",
        ));
    }
    let outputs = source_program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_SOURCE_GEOMETRY_PROGRAM_INVALID",
                "source GeometryProgram part outputs are missing",
            )
        })?;
    let source_parts = outputs
        .iter()
        .filter(|part| {
            part.get("part_id").and_then(Value::as_str) == Some(binding.part_id.as_str())
        })
        .collect::<Vec<_>>();
    if source_parts.len() != 1
        || source_parts[0]
            .get("input_node_ids")
            .and_then(Value::as_array)
            .is_none_or(|inputs| {
                inputs.len() != 1 || inputs[0].as_str() != Some(binding.source_node_id.as_str())
            })
        || source_parts[0]
            .get("material_zone_id")
            .and_then(Value::as_str)
            != Some(binding.material_zone_id.as_str())
        || source_parts[0].get("solid").and_then(Value::as_bool) != Some(binding.solid)
        || canonical_json_hash(source_parts[0]) != binding.part_output_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_GEOMETRY_PART_MISMATCH",
            "source GeometryProgram part output differs from the source binding",
        ));
    }
    Ok(SourceMaterializationInputs {
        source_program,
        source_candidate_id: record.source_candidate_id.clone(),
        source_candidate_state_sha256: record.source_candidate_state_sha256.clone(),
        source_artifact_sha256: artifact_sha256,
        source_artifact_readback_object_sha256: readback_object_sha256,
        source_artifact_readback_sha256: readback_canonical.to_owned(),
        source_quality_report_object_sha256: quality_object_sha256,
        source_program_sha256: program_sha256,
        source_program_object_sha256: program_object_sha256,
        source_binding_id: record.source_binding_id.clone(),
        source_binding_sha256: record.source_binding_sha256.clone(),
        source_binding_object_sha256: record.source_binding_object_sha256.clone(),
        source_node_id: binding.source_node_id.clone(),
        source_part_id: binding.part_id.clone(),
        source_material_zone_id: binding.material_zone_id.clone(),
        source_solid: binding.solid,
        source_position_m: binding.position_m,
        source_rotation_rad: binding.rotation_rad,
        source_part_output_sha256: binding.part_output_sha256.clone(),
    })
}

fn expected_representation_plan_sha256(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
    revision: &AuthoringMeshRevision,
) -> Result<String, StoreError> {
    let source = source_materialization_inputs(transaction, cas, record, revision)?;
    let parameters = authoring_mesh_geometry_parameters(
        revision,
        source.source_position_m,
        source.source_rotation_rad,
    )?;
    let projection_sha256 = canonical_json_hash(&json!({
        "schema_version":"AuthoringMeshV2GeometryProjection@1",
        "revision_id":revision.revision_id.0,
        "revision_sha256":revision.canonical_sha256,
        "operator_id":"forgecad.geometry.authoring-mesh@1",
        "parameters":parameters,
    }));
    let materialization_mode = "source_binding_part_replacement";
    let replacement_identity = json!({
        "schema_version":"AuthoringMeshV2CandidateReplacementIdentity@1",
        "project_id":record.project_id,
        "mesh_id":record.authoring_mesh_id,
        "lineage_id":record.authoring_mesh_lineage_id,
        "materialization_mode":materialization_mode,
        "revision_id":record.authoring_mesh_revision_id,
        "revision_sha256":record.authoring_mesh_revision_sha256,
        "revision_object_sha256":record.authoring_mesh_revision_object_sha256,
        "projection_sha256":projection_sha256,
        "source_binding_id":Some(source.source_binding_id.clone()),
        "source_binding_sha256":Some(source.source_binding_sha256.clone()),
        "source_node_id":Some(source.source_node_id.clone()),
        "source_part_id":Some(source.source_part_id.clone()),
    });
    let replacement_node_id = format!(
        "authoring-mesh-v2-{}",
        &canonical_json_hash(&replacement_identity)[..32]
    );
    Ok(canonical_json_hash(&json!({
        "schema_version":"AuthoringMeshV2CandidateMaterializationRepresentationPlan@1",
        "project_id":record.project_id,
        "mesh_id":record.authoring_mesh_id,
        "lineage_id":record.authoring_mesh_lineage_id,
        "materialization_mode":materialization_mode,
        "revision_id":record.authoring_mesh_revision_id,
        "revision_index":record.authoring_mesh_revision_index,
        "revision_sha256":record.authoring_mesh_revision_sha256,
        "revision_object_sha256":record.authoring_mesh_revision_object_sha256,
        "replacement_revision_id":record.authoring_mesh_revision_id,
        "replacement_revision_sha256":record.authoring_mesh_revision_sha256,
        "replacement_revision_object_sha256":record.authoring_mesh_revision_object_sha256,
        "replacement_projection_sha256":projection_sha256,
        "replacement_node_id":replacement_node_id,
        "source_candidate_id":Some(source.source_candidate_id),
        "source_candidate_state_sha256":Some(source.source_candidate_state_sha256),
        "source_artifact_sha256":Some(source.source_artifact_sha256),
        "source_artifact_readback_sha256":Some(source.source_artifact_readback_sha256),
        "source_program_sha256":Some(source.source_program_sha256),
        "source_program_object_sha256":Some(source.source_program_object_sha256),
        "source_binding_id":Some(source.source_binding_id),
        "source_binding_sha256":Some(source.source_binding_sha256),
        "source_binding_object_sha256":Some(source.source_binding_object_sha256),
        "source_node_id":Some(source.source_node_id),
        "source_part_id":Some(source.source_part_id),
        "source_material_zone_id":Some(source.source_material_zone_id),
        "source_solid":Some(source.source_solid),
        "source_part_output_sha256":Some(source.source_part_output_sha256),
    })))
}

fn expected_standalone_representation_plan_sha256(
    record: &KnifePassStateStoreRecord,
    revision: &AuthoringMeshRevision,
) -> Result<String, StoreError> {
    let parameters = authoring_mesh_geometry_parameters(revision, [0.0; 3], [0.0; 3])?;
    let projection_sha256 = canonical_json_hash(&json!({
        "schema_version":"AuthoringMeshV2GeometryProjection@1",
        "revision_id":revision.revision_id.0,
        "revision_sha256":revision.canonical_sha256,
        "operator_id":"forgecad.geometry.authoring-mesh@1",
        "parameters":parameters,
    }));
    let replacement_identity = json!({
        "schema_version":"AuthoringMeshV2CandidateReplacementIdentity@1",
        "project_id":record.project_id,
        "mesh_id":record.authoring_mesh_id,
        "lineage_id":record.authoring_mesh_lineage_id,
        "materialization_mode":"standalone_revision",
        "revision_id":record.authoring_mesh_revision_id,
        "revision_sha256":record.authoring_mesh_revision_sha256,
        "revision_object_sha256":record.authoring_mesh_revision_object_sha256,
        "projection_sha256":projection_sha256,
        "source_binding_id":Value::Null,
        "source_binding_sha256":Value::Null,
        "source_node_id":Value::Null,
        "source_part_id":Value::Null,
    });
    let replacement_node_id = format!(
        "authoring-mesh-v2-{}",
        &canonical_json_hash(&replacement_identity)[..32]
    );
    Ok(canonical_json_hash(&json!({
        "schema_version":"AuthoringMeshV2CandidateMaterializationRepresentationPlan@1",
        "project_id":record.project_id,
        "mesh_id":record.authoring_mesh_id,
        "lineage_id":record.authoring_mesh_lineage_id,
        "materialization_mode":"standalone_revision",
        "revision_id":record.authoring_mesh_revision_id,
        "revision_index":record.authoring_mesh_revision_index,
        "revision_sha256":record.authoring_mesh_revision_sha256,
        "revision_object_sha256":record.authoring_mesh_revision_object_sha256,
        "replacement_revision_id":record.authoring_mesh_revision_id,
        "replacement_revision_sha256":record.authoring_mesh_revision_sha256,
        "replacement_revision_object_sha256":record.authoring_mesh_revision_object_sha256,
        "replacement_projection_sha256":projection_sha256,
        "replacement_node_id":replacement_node_id,
        "source_candidate_id":Value::Null,
        "source_candidate_state_sha256":Value::Null,
        "source_artifact_sha256":Value::Null,
        "source_artifact_readback_sha256":Value::Null,
        "source_program_sha256":Value::Null,
        "source_program_object_sha256":Value::Null,
        "source_binding_id":Value::Null,
        "source_binding_sha256":Value::Null,
        "source_binding_object_sha256":Value::Null,
        "source_node_id":Value::Null,
        "source_part_id":Value::Null,
        "source_material_zone_id":Value::Null,
        "source_solid":Value::Null,
        "source_part_output_sha256":Value::Null,
    })))
}

fn validate_standalone_materialized_program(
    record: &KnifePassStateStoreRecord,
    revision: &AuthoringMeshRevision,
    operator_catalog_sha256: &str,
    program: &Value,
) -> Result<(), StoreError> {
    let parameters = authoring_mesh_geometry_parameters(revision, [0.0; 3], [0.0; 3])?;
    let projection_sha256 = canonical_json_hash(&json!({
        "schema_version":"AuthoringMeshV2GeometryProjection@1",
        "revision_id":revision.revision_id.0,
        "revision_sha256":revision.canonical_sha256,
        "operator_id":"forgecad.geometry.authoring-mesh@1",
        "parameters":parameters,
    }));
    let replacement_identity = json!({
        "schema_version":"AuthoringMeshV2CandidateReplacementIdentity@1",
        "project_id":record.project_id,
        "mesh_id":record.authoring_mesh_id,
        "lineage_id":record.authoring_mesh_lineage_id,
        "materialization_mode":"standalone_revision",
        "revision_id":record.authoring_mesh_revision_id,
        "revision_sha256":record.authoring_mesh_revision_sha256,
        "revision_object_sha256":record.authoring_mesh_revision_object_sha256,
        "projection_sha256":projection_sha256,
        "source_binding_id":Value::Null,
        "source_binding_sha256":Value::Null,
        "source_node_id":Value::Null,
        "source_part_id":Value::Null,
    });
    let replacement_node_id = format!(
        "authoring-mesh-v2-{}",
        &canonical_json_hash(&replacement_identity)[..32]
    );
    let expected_node = json!({
        "node_id":replacement_node_id,
        "operator_id":"forgecad.geometry.authoring-mesh@1",
        "inputs":[],
        "parameters":parameters,
    });
    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
                "standalone GeometryProgram nodes are missing",
            )
        })?;
    if nodes != &[expected_node] {
        return Err(contract(
            "KNIFE_PASS_STATE_MATERIALIZER_OPERATOR_MISMATCH",
            "standalone GeometryProgram is not the exact AMV2 Worker projection",
        ));
    }
    let face_count = parameters
        .get("faces")
        .and_then(Value::as_array)
        .map(Vec::len)
        .filter(|count| *count > 0)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PROJECTION_INVALID",
                "standalone Worker projection contains no faces",
            )
        })?;
    let expected_output = json!({
        "part_id":format!("authoring-part-{}", revision.revision_id.0),
        "input_node_ids":[replacement_node_id],
        "material_zone_id":"weaponry-authoring-mesh",
        "solid":false,
    });
    if program.get("part_outputs").and_then(Value::as_array) != Some(&vec![expected_output])
        || program
            .get("budgets")
            .and_then(Value::as_object)
            .and_then(|budgets| budgets.get("max_triangles"))
            .and_then(Value::as_u64)
            != u64::try_from(face_count)
                .ok()
                .and_then(|count| count.checked_mul(2))
    {
        return Err(contract(
            "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
            "standalone GeometryProgram output or triangle budget differs",
        ));
    }
    let units = json!({
        "length":"meter",
        "angle":"radian",
        "coordinate_system":"right-handed-y-up",
    });
    let budgets = json!({
        "max_nodes":1,
        "max_triangles":u64::try_from(face_count).ok().and_then(|count| count.checked_mul(2)).unwrap_or(0),
        "max_glb_bytes":67108864,
        "max_worker_memory_bytes":536870912,
        "max_runtime_ms":10000,
    });
    let expected_keys = [
        "schema_version",
        "project_id",
        "representation_plan_sha256",
        "operator_catalog_sha256",
        "units",
        "budgets",
        "nodes",
        "part_outputs",
    ];
    let object = program.as_object().ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
            "standalone GeometryProgram is not an object",
        )
    })?;
    if object.len() != expected_keys.len()
        || expected_keys.iter().any(|key| !object.contains_key(*key))
        || object["schema_version"] != Value::String("GeometryProgram@2".to_owned())
        || object["project_id"] != Value::String(record.project_id.clone())
        || object["operator_catalog_sha256"] != Value::String(operator_catalog_sha256.to_owned())
        || object["units"] != units
        || object["budgets"] != budgets
    {
        return Err(contract(
            "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
            "standalone GeometryProgram top-level policy differs",
        ));
    }
    Ok(())
}

fn validate_source_materialized_program(
    record: &KnifePassStateStoreRecord,
    revision: &AuthoringMeshRevision,
    source: &SourceMaterializationInputs,
    program: &Value,
) -> Result<(), StoreError> {
    let parameters = authoring_mesh_geometry_parameters(
        revision,
        source.source_position_m,
        source.source_rotation_rad,
    )?;
    let projection_sha256 = canonical_json_hash(&json!({
        "schema_version":"AuthoringMeshV2GeometryProjection@1",
        "revision_id":revision.revision_id.0,
        "revision_sha256":revision.canonical_sha256,
        "operator_id":"forgecad.geometry.authoring-mesh@1",
        "parameters":parameters,
    }));
    let replacement_identity = json!({
        "schema_version":"AuthoringMeshV2CandidateReplacementIdentity@1",
        "project_id":record.project_id,
        "mesh_id":record.authoring_mesh_id,
        "lineage_id":record.authoring_mesh_lineage_id,
        "materialization_mode":"source_binding_part_replacement",
        "revision_id":record.authoring_mesh_revision_id,
        "revision_sha256":record.authoring_mesh_revision_sha256,
        "revision_object_sha256":record.authoring_mesh_revision_object_sha256,
        "projection_sha256":projection_sha256,
        "source_binding_id":Some(source.source_binding_id.clone()),
        "source_binding_sha256":Some(source.source_binding_sha256.clone()),
        "source_node_id":Some(source.source_node_id.clone()),
        "source_part_id":Some(source.source_part_id.clone()),
    });
    let replacement_node_id = format!(
        "authoring-mesh-v2-{}",
        &canonical_json_hash(&replacement_identity)[..32]
    );
    let expected_nodes = source
        .source_program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_SOURCE_GEOMETRY_PROGRAM_INVALID",
                "source GeometryProgram nodes are missing",
            )
        })?;
    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
                "materialized GeometryProgram nodes are missing",
            )
        })?;
    if nodes.len() != expected_nodes.len() || nodes.is_empty() {
        return Err(contract(
            "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
            "materialized GeometryProgram changed the source node inventory",
        ));
    }
    for (index, (expected, actual)) in expected_nodes.iter().zip(nodes).enumerate() {
        let expected_id = expected
            .get("node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                contract(
                    "KNIFE_PASS_STATE_SOURCE_GEOMETRY_NODE_MISMATCH",
                    "source GeometryProgram node id is missing",
                )
            })?;
        if expected_id == source.source_node_id {
            let replacement = json!({
                "node_id":replacement_node_id,
                "operator_id":"forgecad.geometry.authoring-mesh@1",
                "inputs":[],
                "parameters":parameters,
            });
            if actual != &replacement {
                return Err(contract(
                    "KNIFE_PASS_STATE_MATERIALIZER_OPERATOR_MISMATCH",
                    format!("materialized replacement node differs at source node index {index}"),
                ));
            }
        } else if actual != expected {
            return Err(contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
                format!("materializer changed preserved source node at index {index}"),
            ));
        }
    }

    let expected_outputs = source
        .source_program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_SOURCE_GEOMETRY_PROGRAM_INVALID",
                "source GeometryProgram part outputs are missing",
            )
        })?;
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
                "materialized GeometryProgram part outputs are missing",
            )
        })?;
    if outputs.len() != expected_outputs.len() || outputs.is_empty() {
        return Err(contract(
            "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
            "materialized GeometryProgram changed the source part inventory",
        ));
    }
    for (index, (expected, actual)) in expected_outputs.iter().zip(outputs).enumerate() {
        let part_id = expected
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                contract(
                    "KNIFE_PASS_STATE_SOURCE_GEOMETRY_PART_MISMATCH",
                    "source part output id is missing",
                )
            })?;
        if part_id == source.source_part_id {
            let mut replacement = expected.clone();
            replacement["input_node_ids"] = json!([replacement_node_id]);
            if actual != &replacement {
                return Err(contract(
                    "KNIFE_PASS_STATE_MATERIALIZER_PART_MISMATCH",
                    format!("materialized replacement part differs at source part index {index}"),
                ));
            }
        } else if actual != expected {
            return Err(contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
                format!("materializer changed preserved source part at index {index}"),
            ));
        }
    }

    // Runtime clones the source program and changes only the replacement plan
    // plus the two inventories above.  Compare every other top-level field so
    // a forged candidate cannot hide a different operator catalogue/budget.
    let source_object = source.source_program.as_object().ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_SOURCE_GEOMETRY_PROGRAM_INVALID",
            "source GeometryProgram is not an object",
        )
    })?;
    let actual_object = program.as_object().ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
            "materialized GeometryProgram is not an object",
        )
    })?;
    for (key, expected) in source_object {
        if matches!(
            key.as_str(),
            "representation_plan_sha256" | "nodes" | "part_outputs"
        ) {
            continue;
        }
        if actual_object.get(key) != Some(expected) {
            return Err(contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
                format!("materialized GeometryProgram changed top-level field {key}"),
            ));
        }
    }
    for key in actual_object.keys() {
        if matches!(
            key.as_str(),
            "representation_plan_sha256" | "nodes" | "part_outputs"
        ) {
            continue;
        }
        if !source_object.contains_key(key) {
            return Err(contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PROGRAM_INVALID",
                format!("materialized GeometryProgram added top-level field {key}"),
            ));
        }
    }
    Ok(())
}

fn validate_pass_state_cas_object(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    supplied: &CasObjectRecord,
    record: &KnifePassStateStoreRecord,
    require_reachable: bool,
) -> Result<Vec<u8>, StoreError> {
    if supplied.schema_version != "CasObject@1"
        || supplied.sha256 != record.pass_state_object_sha256
        || !is_sha256(&supplied.sha256)
        || supplied.mime != KNIFE_PASS_STATE_JSON_MIME
        || supplied.kind != KNIFE_PASS_STATE_OBJECT_KIND
        || supplied.size_bytes == 0
        || supplied.size_bytes > KNIFE_PASS_STATE_MAX_JSON_BYTES
        || !matches!(supplied.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && supplied.reachability != "reachable")
        || supplied.created_at.is_empty()
        || supplied.created_at.len() > MAX_TIMESTAMP_BYTES
    {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_METADATA_INVALID",
            "pass state CAS metadata is outside the bounded allowlist",
        ));
    }
    let registered =
        read_object_record(transaction, &supplied.sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "KNIFE_PASS_STATE_CAS_MISSING",
                "pass state CAS object is not registered",
            ),
            other => other,
        })?;
    let reachability_matches = supplied.reachability == registered.reachability
        || (supplied.reachability == "temporary" && registered.reachability == "reachable");
    if registered.size_bytes != supplied.size_bytes
        || registered.mime != supplied.mime
        || registered.kind != supplied.kind
        || !reachability_matches
        || (require_reachable && registered.reachability != "reachable")
    {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_METADATA_MISMATCH",
            "pass state CAS metadata differs from SQLite registration",
        ));
    }
    let bytes = cas
        .read_verified_bounded(&supplied.sha256, KNIFE_PASS_STATE_MAX_JSON_BYTES)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != supplied.size_bytes || sha256_hex(&bytes) != supplied.sha256 {
        return Err(contract(
            "KNIFE_PASS_STATE_CAS_HASH_MISMATCH",
            "pass state CAS bytes do not match their content hash",
        ));
    }
    Ok(bytes)
}

fn validate_main_payload(
    bytes: &[u8],
    record: &KnifePassStateStoreRecord,
) -> Result<(), StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > KNIFE_PASS_STATE_MAX_JSON_BYTES {
        return Err(contract(
            "KNIFE_PASS_STATE_PAYLOAD_BYTES_INVALID",
            "pass state Main CAS JSON is empty or exceeds its bound",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "KNIFE_PASS_STATE_PAYLOAD_JSON_INVALID",
            format!("pass state Main CAS JSON is invalid: {error}"),
        )
    })?;
    let expected = main_value(record)?;
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes || value != expected {
        return Err(contract(
            "KNIFE_PASS_STATE_PAYLOAD_BINDING_MISMATCH",
            "pass state Main CAS JSON is not the exact closed Main projection",
        ));
    }
    if value.get("pass_state_object_sha256").is_some() || value.get("idempotency_key").is_some() {
        return Err(contract(
            "KNIFE_PASS_STATE_PAYLOAD_EXTERNAL_HASH_EMBEDDED",
            "Main CAS JSON embeds Store-only object or idempotency metadata",
        ));
    }
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != record.canonical_sha256 {
        return Err(contract(
            "KNIFE_PASS_STATE_PAYLOAD_CANONICAL_MISMATCH",
            "pass state Main CAS canonical hash differs from its Store binding",
        ));
    }
    Ok(())
}

fn validate_source_binding_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let source_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM knife_source_binding_records WHERE project_id = ?1 AND source_binding_id = ?2 AND source_binding_sha256 = ?3",
            params![record.project_id, record.source_binding_id, record.source_binding_sha256],
            |row| row.get(0),
        )
        .optional()?;
    let source_json = source_json.ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_SOURCE_BINDING_MISSING",
            "source binding is not durably registered",
        )
    })?;
    let source: KnifeSourceBindingStoreRecord =
        serde_json::from_str(&source_json).map_err(|error| {
            contract(
                "KNIFE_PASS_STATE_SOURCE_BINDING_INVALID",
                format!("source binding Store row is invalid: {error}"),
            )
        })?;
    if source.project_id != record.project_id
        || source.source_binding_id != record.source_binding_id
        || source.source_binding_sha256 != record.source_binding_sha256
        || source.source_binding_object_sha256 != record.source_binding_object_sha256
        || source.intent_bundle_id != record.intent_bundle_id
        || source.intent_bundle_sha256 != record.intent_bundle_sha256
        || source.intent_bundle_object_sha256 != record.intent_bundle_object_sha256
        || source.brief_id != record.brief_id
        || source.brief_sha256 != record.brief_sha256
        || source.brief_object_sha256 != record.brief_object_sha256
        || source.reference_id != record.reference_id
        || source.reference_object_sha256 != record.reference_object_sha256
        || source.reference_evidence_sha256 != record.reference_evidence_sha256
        || source.source_candidate_id != record.source_candidate_id
        || source.source_candidate_state_sha256 != record.source_candidate_state_sha256
        || source.authoring_mesh_id != record.authoring_mesh_id
        || source.authoring_mesh_lineage_id != record.authoring_mesh_lineage_id
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_BINDING_MISMATCH",
            "source binding fields differ from the exact pass lineage",
        ));
    }
    let value = validate_registered_cas(
        transaction,
        cas,
        &record.source_binding_object_sha256,
        &["knife-source-binding"],
        &["KnifeSourceBinding@1"],
        Some(&record.source_binding_sha256),
        true,
        "source binding",
    )?;
    if value.get("source_binding_id").and_then(Value::as_str)
        != Some(record.source_binding_id.as_str())
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_BINDING_PAYLOAD_MISMATCH",
            "source binding CAS payload has a different identity",
        ));
    }
    let quality_contract = validate_registered_cas(
        transaction,
        cas,
        &source.quality_contract_object_sha256,
        &["knife-quality-contract"],
        &["KnifeQualityContract@1"],
        Some(&source.quality_contract_sha256),
        true,
        "source quality contract",
    )?;
    if quality_contract.get("contract_id").and_then(Value::as_str)
        != Some(source.quality_contract_id.as_str())
    {
        return Err(contract(
            "KNIFE_PASS_STATE_QUALITY_CONTRACT_BINDING_MISMATCH",
            "source quality contract CAS payload has a different identity",
        ));
    }
    Ok(vec![
        record.source_binding_object_sha256.clone(),
        source.quality_contract_object_sha256,
    ])
}

fn validate_intent_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let intent_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM knife_reference_intent_bundle_records WHERE project_id = ?1 AND intent_bundle_id = ?2 AND intent_bundle_sha256 = ?3",
            params![record.project_id, record.intent_bundle_id, record.intent_bundle_sha256],
            |row| row.get(0),
        )
        .optional()?;
    let intent_json = intent_json.ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_INTENT_MISSING",
            "reference intent bundle is not durably registered",
        )
    })?;
    let intent: KnifeReferenceIntentBundleStoreRecord = serde_json::from_str(&intent_json)
        .map_err(|error| {
            contract(
                "KNIFE_PASS_STATE_INTENT_INVALID",
                format!("intent Store row is invalid: {error}"),
            )
        })?;
    if intent.project_id != record.project_id
        || intent.intent_bundle_id != record.intent_bundle_id
        || intent.intent_bundle_sha256 != record.intent_bundle_sha256
        || intent.intent_bundle_object_sha256 != record.intent_bundle_object_sha256
        || intent.brief_id != record.brief_id
        || intent.brief_sha256 != record.brief_sha256
        || intent.brief_object_sha256 != record.brief_object_sha256
        || intent.reference_id != record.reference_id
        || intent.reference_object_sha256 != record.reference_object_sha256
        || intent.reference_evidence_sha256 != record.reference_evidence_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_INTENT_BINDING_MISMATCH",
            "intent bundle fields differ from the exact pass lineage",
        ));
    }
    let _ = validate_registered_cas(
        transaction,
        cas,
        &record.intent_bundle_object_sha256,
        &["knife-reference-intent-bundle"],
        &["KnifeReferenceIntentBundle@1"],
        Some(&record.intent_bundle_sha256),
        true,
        "intent bundle",
    )?;
    let intent_quality_contract = validate_registered_cas(
        transaction,
        cas,
        &intent.quality_contract_object_sha256,
        &["knife-quality-contract"],
        &["KnifeQualityContract@1"],
        Some(&intent.quality_contract_sha256),
        true,
        "intent quality contract",
    )?;
    let source_quality_contract_id: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM knife_source_binding_records WHERE project_id = ?1 AND source_binding_id = ?2 AND source_binding_sha256 = ?3",
            params![record.project_id, record.source_binding_id, record.source_binding_sha256],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .and_then(|json| serde_json::from_str::<KnifeSourceBindingStoreRecord>(&json).ok())
        .map(|source| source.quality_contract_id);
    if intent_quality_contract
        .get("contract_id")
        .and_then(Value::as_str)
        .is_none_or(|contract_id| source_quality_contract_id.as_deref() != Some(contract_id))
    {
        return Err(contract(
            "KNIFE_PASS_STATE_QUALITY_CONTRACT_BINDING_MISMATCH",
            "intent quality contract CAS payload has a different identity",
        ));
    }
    Ok(vec![
        record.intent_bundle_object_sha256.clone(),
        intent.quality_contract_object_sha256,
    ])
}

fn validate_brief_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let brief_json: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM weaponry_knife_production_brief_records WHERE project_id = ?1 AND brief_id = ?2 AND brief_canonical_sha256 = ?3",
            params![record.project_id, record.brief_id, record.brief_sha256],
            |row| row.get(0),
        )
        .optional()?;
    let brief_json = brief_json.ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_BRIEF_MISSING",
            "production Brief is not durably registered",
        )
    })?;
    let brief: WeaponryKnifeProductionBriefStoreRecord = serde_json::from_str(&brief_json)
        .map_err(|error| {
            contract(
                "KNIFE_PASS_STATE_BRIEF_INVALID",
                format!("Brief Store row is invalid: {error}"),
            )
        })?;
    if brief.project_id != record.project_id
        || brief.brief_id != record.brief_id
        || brief.brief_canonical_sha256 != record.brief_sha256
        || brief.brief_object_sha256 != record.brief_object_sha256
        || brief.reference_id != record.reference_id
        || brief.reference_object_sha256 != record.reference_object_sha256
        || brief.reference_evidence_sha256 != record.reference_evidence_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_BRIEF_BINDING_MISMATCH",
            "Brief fields differ from the exact pass lineage",
        ));
    }
    let _ = validate_registered_cas(
        transaction,
        cas,
        &record.brief_object_sha256,
        &["weaponry-knife-production-brief"],
        &["WeaponryKnifeProductionBrief@1"],
        Some(&record.brief_sha256),
        true,
        "Brief",
    )?;
    Ok(vec![record.brief_object_sha256.clone()])
}

fn validate_reference_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let source: Option<(String, String, String, i64, String)> = transaction
        .query_row(
            "SELECT project_id, object_sha256, canonical_sha256, size_bytes, mime FROM reference_evidence WHERE reference_id = ?1",
            params![record.reference_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((project_id, object_sha256, evidence_sha256, size_bytes, mime)) = source else {
        return Err(contract(
            "KNIFE_PASS_STATE_REFERENCE_MISSING",
            "ReferenceEvidence is not durably registered",
        ));
    };
    if project_id != record.project_id
        || object_sha256 != record.reference_object_sha256
        || evidence_sha256 != record.reference_evidence_sha256
        || size_bytes <= 0
        || !matches!(mime.as_str(), "image/png" | "image/jpeg")
    {
        return Err(contract(
            "KNIFE_PASS_STATE_REFERENCE_BINDING_MISMATCH",
            "ReferenceEvidence fields differ from the exact pass lineage",
        ));
    }
    let object =
        read_object_record(transaction, &record.reference_object_sha256).map_err(|error| {
            match error {
                StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                    "KNIFE_PASS_STATE_REFERENCE_CAS_MISSING",
                    "reference image CAS object is not registered",
                ),
                other => other,
            }
        })?;
    if object.sha256 != record.reference_object_sha256
        || object.kind != "reference-image"
        || object.mime != mime
        || object.size_bytes != u64::try_from(size_bytes).unwrap_or(u64::MAX)
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || object.size_bytes == 0
    {
        return Err(contract(
            "KNIFE_PASS_STATE_REFERENCE_CAS_METADATA_MISMATCH",
            "ReferenceEvidence CAS metadata differs",
        ));
    }
    cas.verify(&record.reference_object_sha256, object.size_bytes)
        .map_err(StoreError::from)?;
    Ok(vec![record.reference_object_sha256.clone()])
}

#[derive(Debug, Clone)]
struct CandidateLineageValidation {
    roots: Vec<String>,
    attempt_hard_gate_passed: bool,
}

fn validate_candidate_geometry_evidence(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
    revision: &AuthoringMeshRevision,
    role: &str,
    candidate_id: &str,
    _candidate_state_sha256: &str,
    geometry_program_sha256: &str,
    geometry_program_object_sha256: &str,
    artifact_sha256: &str,
    artifact_readback_object_sha256: &str,
    declared_representation_plan_sha256: &str,
    quality_hard_gate_passed: bool,
) -> Result<Vec<String>, StoreError> {
    let evidence: Option<(
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    )> = transaction
        .query_row(
            "SELECT project_id, reference_id, reference_sha256, geometry_program_sha256, geometry_program_object_sha256, operator_catalog_sha256, readback_config_sha256, artifact_object_sha256, artifact_readback_object_sha256, quality_report_object_sha256, quality_report_id, canonical_sha256 FROM geometry_candidate_evidence WHERE candidate_id = ?1",
            params![candidate_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                ))
            },
        )
        .optional()?;
    let Some((
        evidence_project,
        evidence_reference_id,
        evidence_reference_sha256,
        evidence_program_sha256,
        evidence_program_object_sha256,
        operator_catalog_sha256,
        readback_config_sha256,
        evidence_artifact_sha256,
        evidence_readback_object_sha256,
        quality_object_sha256,
        quality_report_id,
        evidence_canonical_sha256,
    )) = evidence
    else {
        return Err(contract(
            "KNIFE_PASS_STATE_GEOMETRY_EVIDENCE_MISSING",
            format!("{role} candidate geometry evidence is not durably registered"),
        ));
    };
    if evidence_project != record.project_id
        || evidence_reference_id.as_deref() != Some(record.reference_id.as_str())
        || evidence_reference_sha256.as_deref() != Some(record.reference_object_sha256.as_str())
        || evidence_program_sha256 != geometry_program_sha256
        || evidence_program_object_sha256 != geometry_program_object_sha256
        || evidence_artifact_sha256 != artifact_sha256
        || evidence_readback_object_sha256 != artifact_readback_object_sha256
        || !is_sha256(&operator_catalog_sha256)
        || !is_sha256(&readback_config_sha256)
        || !is_sha256(&quality_object_sha256)
        || !is_opaque_id(&quality_report_id)
        || !is_sha256(&evidence_canonical_sha256)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_GEOMETRY_EVIDENCE_BINDING_MISMATCH",
            format!("{role} geometry evidence differs from the exact candidate lineage"),
        ));
    }

    let (program_object, program_bytes) = validate_registered_bytes(
        transaction,
        cas,
        geometry_program_object_sha256,
        &["geometry-program-v2"],
        &[KNIFE_PASS_STATE_JSON_MIME],
        MAX_LINEAGE_JSON_BYTES,
        true,
        &format!("{role} GeometryProgram"),
    )?;
    let program = canonical_json_value(&program_bytes, &format!("{role} GeometryProgram"))?;
    let source = if revision.source_binding.is_some() {
        Some(source_materialization_inputs(
            transaction,
            cas,
            record,
            revision,
        )?)
    } else {
        None
    };
    let mut roots = Vec::new();
    let actual_plan = program
        .get("representation_plan_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_MATERIALIZER_PLAN_MISSING",
                format!("{role} GeometryProgram representation plan is missing"),
            )
        })?;
    let source_plan = source
        .as_ref()
        .map(|_| expected_representation_plan_sha256(transaction, cas, record, revision))
        .transpose()?;
    let standalone_plan = expected_standalone_representation_plan_sha256(record, revision)?;
    if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || program.get("canonical_sha256").is_some()
        || canonical_json_hash(&program) != geometry_program_sha256
        || program.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || program
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(operator_catalog_sha256.as_str())
        || actual_plan != declared_representation_plan_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_MATERIALIZER_PLAN_MISMATCH",
            format!(
                "{role} GeometryProgram is not materialized from the exact AMV2 revision (actual={actual_plan}, source={source_plan:?}, declared={})",
                declared_representation_plan_sha256
            ),
        ));
    }
    if source_plan.as_deref() == Some(actual_plan) {
        validate_source_materialized_program(
            record,
            revision,
            source.as_ref().expect("source plan implies source inputs"),
            &program,
        )?;
    } else if source.is_none() && standalone_plan == actual_plan {
        validate_standalone_materialized_program(
            record,
            revision,
            &operator_catalog_sha256,
            &program,
        )?;
    } else {
        return Err(contract(
            "KNIFE_PASS_STATE_MATERIALIZER_PLAN_MISMATCH",
            format!("{role} GeometryProgram plan is not an exact supported materialization mode"),
        ));
    }

    // A source-bound replacement is only replayable while the original
    // program, artifact readback and structural quality evidence remain
    // available.  Keep those upstream objects in the durable root set rather
    // than rooting only the derived candidate; otherwise a later restart
    // could no longer prove which source Part was replaced.
    if let Some(source) = source.as_ref() {
        // `source_materialization_inputs` has already verified all three
        // identities against the source candidate's evidence row/CAS.
        roots.extend([
            source.source_program_object_sha256.clone(),
            source.source_artifact_sha256.clone(),
            source.source_artifact_readback_object_sha256.clone(),
            source.source_quality_report_object_sha256.clone(),
        ]);
    }

    let (artifact_object, _artifact_bytes) = validate_registered_bytes(
        transaction,
        cas,
        artifact_sha256,
        &[
            "geometry-glb",
            "geometry-artifact",
            "appearance-glb",
            "appearance-v2-glb",
        ],
        &["model/gltf-binary"],
        MAX_LINEAGE_JSON_BYTES * 8,
        true,
        &format!("{role} geometry artifact"),
    )?;
    let (readback_object, readback_bytes) = validate_registered_bytes(
        transaction,
        cas,
        artifact_readback_object_sha256,
        &[
            "geometry-artifact-readback-v2",
            "appearance-v2-artifact-readback",
            "artifact-readback",
        ],
        &[KNIFE_PASS_STATE_JSON_MIME],
        MAX_LINEAGE_JSON_BYTES,
        true,
        &format!("{role} artifact readback"),
    )?;
    let readback = canonical_json_value(&readback_bytes, &format!("{role} artifact readback"))?;
    for (field, expected) in [
        ("candidate_id", candidate_id),
        ("object_sha256", artifact_sha256),
        ("program_sha256", geometry_program_sha256),
    ] {
        if readback.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(contract(
                "KNIFE_PASS_STATE_ARTIFACT_READBACK_BINDING_MISMATCH",
                format!("{role} artifact readback {field} differs"),
            ));
        }
    }
    if let Some(hard_gate) = readback.get("hard_gate_passed").and_then(Value::as_bool) {
        if hard_gate != quality_hard_gate_passed {
            return Err(contract(
                "KNIFE_PASS_STATE_ARTIFACT_READBACK_GATE_MISMATCH",
                format!("{role} artifact readback hard gate differs from Candidate"),
            ));
        }
    }

    let (quality_object, quality_bytes) = validate_registered_bytes(
        transaction,
        cas,
        &quality_object_sha256,
        &[
            "geometry-quality-report",
            "quality-report",
            "appearance-quality-report",
            "appearance-v2-quality-report",
        ],
        &[KNIFE_PASS_STATE_JSON_MIME],
        MAX_LINEAGE_JSON_BYTES,
        true,
        &format!("{role} geometry quality report"),
    )?;
    let quality = canonical_json_value(&quality_bytes, &format!("{role} geometry quality report"))?;
    if quality.get("quality_report_id").and_then(Value::as_str) != Some(quality_report_id.as_str())
        || quality.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || quality.get("artifact_sha256").and_then(Value::as_str) != Some(artifact_sha256)
        || quality.get("program_sha256").and_then(Value::as_str) != Some(geometry_program_sha256)
        || quality
            .get("hard_gate_passed")
            .and_then(Value::as_bool)
            .is_some_and(|value| value != quality_hard_gate_passed)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_GEOMETRY_QUALITY_BINDING_MISMATCH",
            format!("{role} geometry quality report differs from evidence"),
        ));
    }
    roots.extend([
        program_object.sha256,
        artifact_object.sha256,
        readback_object.sha256,
        quality_object.sha256,
    ]);
    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn validate_candidate_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
    revision: &AuthoringMeshRevision,
) -> Result<CandidateLineageValidation, StoreError> {
    let mut roots = Vec::new();
    let mut attempt_hard_gate_passed = false;
    let source_candidate: Option<(String, String)> = transaction
        .query_row(
            "SELECT project_id, canonical_sha256 FROM candidates WHERE candidate_id = ?1",
            params![record.source_candidate_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((source_project_id, source_state_sha256)) = source_candidate else {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_CANDIDATE_MISSING",
            "source candidate is not durably registered",
        ));
    };
    if source_project_id != record.project_id
        || source_state_sha256 != record.source_candidate_state_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_SOURCE_CANDIDATE_MISMATCH",
            "source candidate is not bound to the source binding lineage",
        ));
    }
    // The root pass compares the immutable SourceBinding source candidate
    // against the first source-bound materialization.  Its baseline is not
    // itself a materializer output, so validating it with the selected AMV2
    // representation-plan formula is a category error.  Re-prove the source
    // candidate through the embedded binding/CAS lineage instead; child-pass
    // baselines remain the exact materialized attempt from their parent.
    let root_source = if record.parent_pass_id.is_none() {
        Some(source_materialization_inputs(
            transaction,
            cas,
            record,
            revision,
        )?)
    } else {
        None
    };
    for (
        role,
        candidate_id,
        state_sha256,
        program_sha256,
        program_object_sha256,
        artifact_sha256,
        readback_object_sha256,
        declared_representation_plan_sha256,
    ) in [
        (
            "baseline",
            &record.baseline_candidate_id,
            &record.baseline_candidate_state_sha256,
            &record.baseline_geometry_program_sha256,
            &record.baseline_geometry_program_object_sha256,
            &record.baseline_artifact_sha256,
            &record.baseline_artifact_readback_object_sha256,
            &record.baseline_representation_plan_sha256,
        ),
        (
            "attempt",
            &record.attempt_candidate_id,
            &record.attempt_candidate_state_sha256,
            &record.attempt_geometry_program_sha256,
            &record.attempt_geometry_program_object_sha256,
            &record.attempt_artifact_sha256,
            &record.attempt_artifact_readback_object_sha256,
            &record.attempt_representation_plan_sha256,
        ),
    ] {
        if role == "baseline" {
            if let Some(source) = root_source.as_ref() {
                let source_plan = source
                    .source_program
                    .get("representation_plan_sha256")
                    .and_then(Value::as_str)
                    .filter(|value| is_sha256(value))
                    .ok_or_else(|| {
                        contract(
                            "KNIFE_PASS_STATE_SOURCE_GEOMETRY_PROGRAM_INVALID",
                            "root source GeometryProgram representation plan is missing",
                        )
                    })?;
                if candidate_id != &source.source_candidate_id
                    || state_sha256 != &source.source_candidate_state_sha256
                    || program_sha256 != &source.source_program_sha256
                    || program_object_sha256 != &source.source_program_object_sha256
                    || artifact_sha256 != &source.source_artifact_sha256
                    || readback_object_sha256 != &source.source_artifact_readback_object_sha256
                    || record.baseline_representation_plan_sha256 != source_plan
                {
                    return Err(contract(
                        "KNIFE_PASS_STATE_ROOT_BASELINE_MISMATCH",
                        "root baseline is not the exact SourceBinding source candidate",
                    ));
                }
                roots.extend([
                    source.source_program_object_sha256.clone(),
                    source.source_artifact_sha256.clone(),
                    source.source_artifact_readback_object_sha256.clone(),
                    source.source_quality_report_object_sha256.clone(),
                ]);
                continue;
            }
        }
        // A correction's baseline is the exact attempt artifact from its
        // parent PassState.  Validate that candidate against the parent's
        // AuthoringMesh revision (the current attempt is allowed to use the
        // newer descendant revision), otherwise a legitimate correction is
        // rejected as if its inherited baseline had been materialized from
        // the new revision.
        let (candidate_record, candidate_revision) = if role == "baseline"
            && record.parent_pass_id.is_some()
        {
            let (parent_id, parent_sha256) = (
                record.parent_pass_id.as_deref().expect("checked parent id"),
                record
                    .parent_pass_sha256
                    .as_deref()
                    .expect("checked parent hash"),
            );
            let parent_json: String = transaction
                .query_row(
                    "SELECT record_json FROM knife_pass_state_records WHERE project_id = ?1 AND pass_id = ?2 AND canonical_sha256 = ?3",
                    params![record.project_id, parent_id, parent_sha256],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| {
                    contract(
                        "KNIFE_PASS_STATE_PARENT_MISSING",
                        "correction baseline parent is not durably registered",
                    )
                })?;
            let parent_record: KnifePassStateStoreRecord = serde_json::from_str(&parent_json)
                .map_err(|error| {
                    contract(
                        "KNIFE_PASS_STATE_PARENT_INVALID",
                        format!("correction baseline parent is invalid: {error}"),
                    )
                })?;
            let parent_revision = load_authoring_mesh_revision(transaction, cas, &parent_record)?;
            (parent_record, parent_revision)
        } else {
            (record.clone(), revision.clone())
        };
        let candidate: Option<(String, String, Option<String>, String, i64)> = transaction
            .query_row(
                "SELECT project_id, canonical_sha256, prepared_object_sha256, state, quality_hard_gate_passed FROM candidates WHERE candidate_id = ?1",
                params![candidate_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?;
        let Some((project_id, state, prepared_object_sha256, candidate_state, hard_gate)) =
            candidate
        else {
            return Err(contract(
                "KNIFE_PASS_STATE_CANDIDATE_MISSING",
                format!("{role} candidate is not durably registered"),
            ));
        };
        if project_id != record.project_id || state != *state_sha256 {
            return Err(contract(
                "KNIFE_PASS_STATE_CANDIDATE_BINDING_MISMATCH",
                format!("{role} candidate state differs from the pass lineage"),
            ));
        }
        let quality_hard_gate_passed = hard_gate != 0;
        if prepared_object_sha256.as_deref() != Some(artifact_sha256.as_str()) {
            return Err(contract(
                "KNIFE_PASS_STATE_CANDIDATE_ARTIFACT_MISMATCH",
                format!("{role} candidate prepared object differs from geometry evidence"),
            ));
        }
        let candidate_roots = validate_candidate_geometry_evidence(
            transaction,
            cas,
            &candidate_record,
            &candidate_revision,
            role,
            candidate_id,
            state_sha256,
            program_sha256,
            program_object_sha256,
            artifact_sha256,
            readback_object_sha256,
            declared_representation_plan_sha256,
            quality_hard_gate_passed,
        )?;
        roots.extend(candidate_roots);
        if role == "attempt" {
            attempt_hard_gate_passed = quality_hard_gate_passed;
        }
        let _ = candidate_state;
    }
    roots.sort();
    roots.dedup();
    Ok(CandidateLineageValidation {
        roots,
        attempt_hard_gate_passed,
    })
}

fn validate_authoring_mesh_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let revision: Option<(String, String, i64, String, String)> = transaction
        .query_row(
            "SELECT lineage_id, revision_id, revision_index, revision_object_sha256, revision_sha256 FROM authoring_mesh_v2_durable_records WHERE project_id = ?1 AND mesh_id = ?2 AND revision_id = ?3",
            params![record.project_id, record.authoring_mesh_id, record.authoring_mesh_revision_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((lineage_id, revision_id, revision_index, object_sha256, revision_sha256)) = revision
    else {
        return Err(contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_REVISION_MISSING",
            "AuthoringMesh V2 revision is not durably registered",
        ));
    };
    if lineage_id != record.authoring_mesh_lineage_id
        || revision_id != record.authoring_mesh_revision_id
        || revision_index < 0
        || u64::try_from(revision_index).unwrap_or(u64::MAX) != record.authoring_mesh_revision_index
        || object_sha256 != record.authoring_mesh_revision_object_sha256
        || revision_sha256 != record.authoring_mesh_revision_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_REVISION_MISMATCH",
            "AuthoringMesh V2 revision identity differs",
        ));
    }
    let value = validate_registered_cas(
        transaction,
        cas,
        &record.authoring_mesh_revision_object_sha256,
        &[AUTHORING_MESH_V2_REVISION_OBJECT_KIND],
        &["AuthoringMeshRevision@2"],
        Some(&record.authoring_mesh_revision_sha256),
        true,
        "AuthoringMesh revision",
    )?;
    let revision: AuthoringMeshRevision = serde_json::from_value(value).map_err(|error| {
        contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_REVISION_INVALID",
            format!("AuthoringMesh revision payload is invalid: {error}"),
        )
    })?;
    if revision.mesh_id.0 != record.authoring_mesh_id
        || revision.lineage_id.0 != record.authoring_mesh_lineage_id
        || revision.revision_id.0 != record.authoring_mesh_revision_id
        || revision.revision_index != record.authoring_mesh_revision_index
        || revision.canonical_sha256 != record.authoring_mesh_revision_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_PAYLOAD_MISMATCH",
            "AuthoringMesh revision payload differs from its durable identity",
        ));
    }
    validate_source_revision_ancestry(transaction, cas, record, &revision)?;
    Ok(vec![record.authoring_mesh_revision_object_sha256.clone()])
}

const MAX_SOURCE_REVISION_ANCESTRY_HOPS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DurableAuthoringRevisionIdentity {
    mesh_id: String,
    lineage_id: String,
    revision_id: String,
    parent_revision_ids: Vec<String>,
    revision_index: u64,
    revision_object_sha256: String,
    revision_sha256: String,
}

fn durable_authoring_revision_identity(
    transaction: &Transaction<'_>,
    project_id: &str,
    revision_id: &str,
) -> Result<Option<DurableAuthoringRevisionIdentity>, StoreError> {
    let row: Option<(String, String, String, String, i64, String, String)> = transaction
        .query_row(
            "SELECT mesh_id, lineage_id, revision_id, parent_revision_ids_json, revision_index, revision_object_sha256, revision_sha256 FROM authoring_mesh_v2_durable_records WHERE project_id = ?1 AND revision_id = ?2",
            params![project_id, revision_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?;
    let Some((
        mesh_id,
        lineage_id,
        revision_id,
        parents_json,
        revision_index,
        revision_object_sha256,
        revision_sha256,
    )) = row
    else {
        return Ok(None);
    };
    if revision_index < 0 {
        return Err(contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_INVALID",
            "AuthoringMesh revision index is negative",
        ));
    }
    let parent_revision_ids: Vec<String> =
        serde_json::from_str(&parents_json).map_err(|error| {
            contract(
                "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_INVALID",
                format!("AuthoringMesh parent revision list is invalid: {error}"),
            )
        })?;
    if !parent_revision_ids.iter().all(|id| validate_identifier(id)) {
        return Err(contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_INVALID",
            "AuthoringMesh parent revision identity is invalid",
        ));
    }
    Ok(Some(DurableAuthoringRevisionIdentity {
        mesh_id,
        lineage_id,
        revision_id,
        parent_revision_ids,
        revision_index: u64::try_from(revision_index).map_err(|_| {
            contract(
                "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_INVALID",
                "AuthoringMesh revision index is outside the bounded range",
            )
        })?,
        revision_object_sha256,
        revision_sha256,
    }))
}

fn validate_authoring_revision_identity(
    durable: &DurableAuthoringRevisionIdentity,
    revision: &AuthoringMeshRevision,
) -> Result<(), StoreError> {
    let revision_parents = revision
        .parent_revision_ids
        .iter()
        .map(|parent| parent.0.clone())
        .collect::<Vec<_>>();
    if durable.mesh_id != revision.mesh_id.0
        || durable.lineage_id != revision.lineage_id.0
        || durable.revision_id != revision.revision_id.0
        || durable.parent_revision_ids != revision_parents
        || durable.revision_index != revision.revision_index
        || durable.revision_object_sha256.is_empty()
        || durable.revision_sha256 != revision.canonical_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_MISMATCH",
            "durable and CAS AuthoringMesh revision identities differ",
        ));
    }
    Ok(())
}

/// Treat the immutable SourceBinding revision as a root anchor.  A PassState
/// may use that exact revision for its first pass, or a bounded single-parent
/// AuthoringMesh descendant for a later correction.  Every hop is resolved
/// through the durable Store row and canonical CAS object; caller-supplied
/// revision IDs cannot manufacture ancestry.
fn validate_source_revision_ancestry(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
    current_revision: &AuthoringMeshRevision,
) -> Result<(), StoreError> {
    let source_json: String = transaction
        .query_row(
            "SELECT record_json FROM knife_source_binding_records WHERE project_id = ?1 AND source_binding_id = ?2 AND source_binding_sha256 = ?3",
            params![record.project_id, record.source_binding_id, record.source_binding_sha256],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_SOURCE_BINDING_MISSING",
                "source binding is not durably registered for AuthoringMesh ancestry",
            )
        })?;
    let source: KnifeSourceBindingStoreRecord =
        serde_json::from_str(&source_json).map_err(|error| {
            contract(
                "KNIFE_PASS_STATE_SOURCE_BINDING_INVALID",
                format!("source binding Store row is invalid: {error}"),
            )
        })?;
    let anchor = durable_authoring_revision_identity(
        transaction,
        &record.project_id,
        &source.authoring_mesh_revision_id,
    )?
    .ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_ANCHOR_MISSING",
            "SourceBinding anchor revision is not durably registered",
        )
    })?;
    if anchor.mesh_id != source.authoring_mesh_id
        || anchor.lineage_id != source.authoring_mesh_lineage_id
        || anchor.revision_id != source.authoring_mesh_revision_id
        || anchor.revision_index != source.authoring_mesh_revision_index
        || anchor.revision_sha256 != source.authoring_mesh_revision_sha256
        || anchor.revision_object_sha256 != source.authoring_mesh_revision_object_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_ANCHOR_MISMATCH",
            "SourceBinding anchor identity differs from its durable AuthoringMesh row",
        ));
    }
    let anchor_value = validate_registered_cas(
        transaction,
        cas,
        &anchor.revision_object_sha256,
        &[AUTHORING_MESH_V2_REVISION_OBJECT_KIND],
        &["AuthoringMeshRevision@2"],
        Some(&anchor.revision_sha256),
        true,
        "SourceBinding anchor revision",
    )?;
    let anchor_revision: AuthoringMeshRevision =
        serde_json::from_value(anchor_value).map_err(|error| {
            contract(
                "KNIFE_PASS_STATE_AUTHORING_MESH_ANCHOR_INVALID",
                format!("SourceBinding anchor revision payload is invalid: {error}"),
            )
        })?;
    validate_authoring_revision_identity(&anchor, &anchor_revision)?;
    let anchor_binding = anchor_revision.source_binding.clone().ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_ANCHOR_INVALID",
            "SourceBinding anchor has no embedded source binding",
        )
    })?;
    if anchor_binding.project_id != record.project_id
        || anchor_binding.candidate_id != source.source_candidate_id
        || anchor_binding.candidate_state_sha256 != source.source_candidate_state_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_ANCHOR_MISMATCH",
            "SourceBinding anchor embedded source identity differs",
        ));
    }

    let mut durable = durable_authoring_revision_identity(
        transaction,
        &record.project_id,
        &record.authoring_mesh_revision_id,
    )?
    .ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_REVISION_MISSING",
            "selected AuthoringMesh revision is not durably registered",
        )
    })?;
    validate_authoring_revision_identity(&durable, current_revision)?;
    if current_revision.mesh_id.0 != source.authoring_mesh_id
        || current_revision.lineage_id.0 != source.authoring_mesh_lineage_id
        || current_revision.revision_index < anchor.revision_index
    {
        return Err(contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_MISMATCH",
            "selected revision is outside the immutable SourceBinding mesh lineage",
        ));
    }
    if record.parent_pass_id.is_none() && durable != anchor {
        return Err(contract(
            "KNIFE_PASS_STATE_ROOT_REVISION_MISMATCH",
            "a root PassState must use the exact immutable SourceBinding anchor revision",
        ));
    }
    let mut revision = current_revision.clone();
    for _ in 0..=MAX_SOURCE_REVISION_ANCESTRY_HOPS {
        if revision.source_binding.as_ref() != Some(&anchor_binding) {
            return Err(contract(
                "KNIFE_PASS_STATE_AUTHORING_MESH_SOURCE_BINDING_MISMATCH",
                "AuthoringMesh descendant changed the immutable SourceBinding anchor",
            ));
        }
        if durable.revision_id == anchor.revision_id {
            if durable != anchor
                || revision.canonical_sha256 != source.authoring_mesh_revision_sha256
            {
                return Err(contract(
                    "KNIFE_PASS_STATE_AUTHORING_MESH_ANCHOR_MISMATCH",
                    "AuthoringMesh ancestry terminated at a drifted SourceBinding anchor",
                ));
            }
            return Ok(());
        }
        if durable.revision_index <= anchor.revision_index
            || durable.parent_revision_ids.len() != 1
            || revision.parent_revision_ids.len() != 1
            || revision.parent_revision_ids[0].0 != durable.parent_revision_ids[0]
        {
            return Err(contract(
                "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_INVALID",
                "SourceBinding descendant must have a bounded single parent",
            ));
        }
        let parent_id = durable.parent_revision_ids[0].clone();
        let parent =
            durable_authoring_revision_identity(transaction, &record.project_id, &parent_id)?
                .ok_or_else(|| {
                    contract(
                        "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_MISSING",
                        "SourceBinding descendant parent is not durable",
                    )
                })?;
        if parent.mesh_id != source.authoring_mesh_id
            || parent.lineage_id != source.authoring_mesh_lineage_id
            || parent.revision_index.checked_add(1) != Some(durable.revision_index)
        {
            return Err(contract(
                "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_MISMATCH",
                "SourceBinding descendant parent lineage/index differs",
            ));
        }
        let parent_value = validate_registered_cas(
            transaction,
            cas,
            &parent.revision_object_sha256,
            &[AUTHORING_MESH_V2_REVISION_OBJECT_KIND],
            &["AuthoringMeshRevision@2"],
            Some(&parent.revision_sha256),
            true,
            "SourceBinding ancestor revision",
        )?;
        revision = serde_json::from_value(parent_value).map_err(|error| {
            contract(
                "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_INVALID",
                format!("SourceBinding ancestor revision payload is invalid: {error}"),
            )
        })?;
        validate_authoring_revision_identity(&parent, &revision)?;
        durable = parent;
    }
    Err(contract(
        "KNIFE_PASS_STATE_AUTHORING_MESH_ANCESTRY_TOO_DEEP",
        "SourceBinding descendant ancestry exceeds the bounded correction budget",
    ))
}

fn validate_optional_curve_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let mut roots = Vec::new();
    if let (Some(modifier_graph_id), Some(modifier_graph_sha256)) = (
        record.modifier_graph_id.as_deref(),
        record.modifier_graph_sha256.as_deref(),
    ) {
        let mut statement = transaction.prepare(
            "SELECT record_json FROM weaponry_curve_modifier_graph_records WHERE project_id = ?1 AND source_revision_sha256 = ?2 AND modifier_graph_id = ?3 AND modifier_graph_sha256 = ?4 ORDER BY created_at ASC, lookup_key_sha256 ASC",
        )?;
        let rows = statement
            .query_map(
                params![
                    record.project_id,
                    record.authoring_mesh_revision_sha256,
                    modifier_graph_id,
                    modifier_graph_sha256,
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        if rows.len() != 1 {
            return Err(contract(
                "KNIFE_PASS_STATE_MODIFIER_LINEAGE_MISSING",
                "optional modifier graph does not resolve to exactly one durable Curve row",
            ));
        }
        let modifier: WeaponryCurveModifierGraphDurableRecord = serde_json::from_str(&rows[0])
            .map_err(|error| {
                contract(
                    "KNIFE_PASS_STATE_MODIFIER_LINEAGE_INVALID",
                    format!("modifier graph Store row is invalid: {error}"),
                )
            })?;
        if modifier.schema_version != WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_SCHEMA
            || modifier.materialization_status != WEAPONRY_CURVE_MODIFIER_GRAPH_STATUS
            || modifier.project_id != record.project_id
            || modifier.source_revision_id != record.authoring_mesh_revision_id
            || modifier.source_revision_sha256 != record.authoring_mesh_revision_sha256
            || modifier.source_candidate_id != record.source_candidate_id
            || modifier.source_candidate_state_sha256 != record.source_candidate_state_sha256
            || modifier.source_authoring_mesh_id != record.authoring_mesh_id
            || modifier.source_authoring_mesh_lineage_id != record.authoring_mesh_lineage_id
            || modifier.source_authoring_mesh_revision_index != record.authoring_mesh_revision_index
            || modifier.source_authoring_mesh_identity_sha256
                != record.authoring_mesh_identity_sha256
            || modifier.modifier_graph_id != modifier_graph_id
            || modifier.modifier_graph_sha256 != modifier_graph_sha256
        {
            return Err(contract(
                "KNIFE_PASS_STATE_MODIFIER_LINEAGE_MISMATCH",
                "optional modifier graph is not bound to the exact source revision",
            ));
        }
        for (hash, kinds) in [
            (
                &modifier.curve_set_object_sha256,
                &[WEAPONRY_CURVE_SET_OBJECT_KIND][..],
            ),
            (
                &modifier.sample_set_object_sha256,
                &[WEAPONRY_SAMPLE_SET_OBJECT_KIND][..],
            ),
            (
                &modifier.modifier_graph_object_sha256,
                &[WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND][..],
            ),
            (
                &modifier.dependency_graph_object_sha256,
                &[WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND][..],
            ),
            (
                &modifier.recompute_plan_object_sha256,
                &[WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND][..],
            ),
        ] {
            roots.push(
                validate_registered_bytes(
                    transaction,
                    cas,
                    hash,
                    kinds,
                    &[WEAPONRY_CURVE_MODIFIER_GRAPH_JSON_MIME],
                    WEAPONRY_CURVE_MODIFIER_GRAPH_MAX_JSON_BYTES,
                    true,
                    "Curve modifier lineage",
                )?
                .0
                .sha256,
            );
        }
    }
    if let (Some(evaluated_mesh_id), Some(evaluated_mesh_sha256)) = (
        record.evaluated_mesh_id.as_deref(),
        record.evaluated_mesh_sha256.as_deref(),
    ) {
        let mut statement = transaction.prepare(
            "SELECT record_json FROM weaponry_curve_evaluated_mesh_records WHERE project_id = ?1 AND source_authoring_mesh_revision_sha256 = ?2 AND evaluated_mesh_id = ?3 AND evaluated_mesh_semantic_sha256 = ?4 ORDER BY created_at ASC, evaluated_mesh_lookup_key_sha256 ASC",
        )?;
        let rows = statement
            .query_map(
                params![
                    record.project_id,
                    record.authoring_mesh_revision_sha256,
                    evaluated_mesh_id,
                    evaluated_mesh_sha256,
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        if rows.len() != 1 {
            return Err(contract(
                "KNIFE_PASS_STATE_EVALUATED_LINEAGE_MISSING",
                "optional evaluated mesh does not resolve to exactly one durable Curve row",
            ));
        }
        let evaluated: WeaponryCurveEvaluatedMeshDurableRecord = serde_json::from_str(&rows[0])
            .map_err(|error| {
                contract(
                    "KNIFE_PASS_STATE_EVALUATED_LINEAGE_INVALID",
                    format!("evaluated mesh Store row is invalid: {error}"),
                )
            })?;
        if evaluated.schema_version != WEAPONRY_CURVE_EVALUATED_MESH_RECORD_SCHEMA
            || evaluated.materialization_status != WEAPONRY_CURVE_EVALUATED_MESH_STATUS
            || evaluated.project_id != record.project_id
            || evaluated.source_candidate_id != record.source_candidate_id
            || evaluated.source_candidate_state_sha256 != record.source_candidate_state_sha256
            || evaluated.source_authoring_mesh_id != record.authoring_mesh_id
            || evaluated.source_authoring_mesh_lineage_id != record.authoring_mesh_lineage_id
            || evaluated.source_authoring_mesh_revision_id != record.authoring_mesh_revision_id
            || evaluated.source_authoring_mesh_revision_index
                != record.authoring_mesh_revision_index
            || evaluated.source_authoring_mesh_revision_sha256
                != record.authoring_mesh_revision_sha256
            || evaluated.source_authoring_mesh_identity_sha256
                != record.authoring_mesh_identity_sha256
            || evaluated.source_modifier_graph_id
                != record.modifier_graph_id.clone().unwrap_or_default()
            || evaluated.source_modifier_graph_sha256
                != record.modifier_graph_sha256.clone().unwrap_or_default()
            || evaluated.evaluated_mesh_id != evaluated_mesh_id
            || evaluated.evaluated_mesh_semantic_sha256 != evaluated_mesh_sha256
        {
            return Err(contract(
                "KNIFE_PASS_STATE_EVALUATED_LINEAGE_MISMATCH",
                "optional evaluated mesh is not bound to the exact modifier/source revision",
            ));
        }
        for (hash, kinds) in [
            (
                &evaluated.evaluation_plan_object_sha256,
                &[WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND][..],
            ),
            (
                &evaluated.evaluated_mesh_object_sha256,
                &[WEAPONRY_EVALUATED_MESH_OBJECT_KIND][..],
            ),
            (
                &evaluated.evaluated_mesh_identity_object_sha256,
                &[WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND][..],
            ),
            (
                &evaluated.evaluated_mesh_link_object_sha256,
                &[WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND][..],
            ),
        ] {
            roots.push(
                validate_registered_bytes(
                    transaction,
                    cas,
                    hash,
                    kinds,
                    &[WEAPONRY_CURVE_EVALUATED_MESH_JSON_MIME],
                    WEAPONRY_CURVE_EVALUATED_MESH_MAX_JSON_BYTES,
                    true,
                    "Curve evaluated-mesh lineage",
                )?
                .0
                .sha256,
            );
        }
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn value_identity(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        value
            .get(*field)
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .map(str::to_owned)
    })
}

fn validate_visual_object_binding(
    value: &Value,
    record: &KnifePassStateStoreRecord,
    expected_id: &str,
    role: &str,
) -> Result<(), StoreError> {
    let id = value_identity(
        value,
        match role {
            "RenderSet" => &["render_set_id", "id", "report_id"],
            "ReferenceComparison" => &[
                "report_id",
                "comparison_id",
                "reference_comparison_id",
                "id",
            ],
            "QualityReport" => &["quality_report_id", "report_id", "id"],
            _ => &["id"],
        },
    )
    .ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_EVIDENCE_ID_MISSING",
            format!("{role} id is missing"),
        )
    })?;
    if id != expected_id {
        return Err(contract(
            "KNIFE_PASS_STATE_EVIDENCE_ID_MISMATCH",
            format!("{role} id differs from the pass state"),
        ));
    }
    for (field, expected) in [
        (
            "source_binding_sha256",
            Some(record.source_binding_sha256.as_str()),
        ),
        ("camera_set_sha256", Some(record.camera_set_sha256.as_str())),
        ("reference_id", Some(record.reference_id.as_str())),
        ("candidate_id", Some(record.attempt_candidate_id.as_str())),
    ] {
        if let Some(actual) = value.get(field).and_then(Value::as_str) {
            if Some(actual) != expected {
                return Err(contract(
                    "KNIFE_PASS_STATE_EVIDENCE_BINDING_MISMATCH",
                    format!("{role} field {field} differs from the exact pass lineage"),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct VisualEvidenceValidation {
    roots: Vec<String>,
    camera_hash: String,
    visual_status: String,
}

fn validate_visual_evidence(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
    require_reachable: bool,
) -> Result<VisualEvidenceValidation, StoreError> {
    let evidence: Option<(String, String, String, Option<String>, String)> = transaction
        .query_row(
            "SELECT project_id, reference_id, render_set_object_sha256, comparison_report_object_sha256, quality_report_object_sha256 FROM visual_evidence WHERE candidate_id = ?1",
            params![record.attempt_candidate_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let (evidence_project, evidence_reference, comparison_object) = if let Some((
        project,
        reference,
        render,
        comparison,
        quality,
    )) = evidence
    {
        if render != record.render_set_object_sha256
            || comparison.as_deref() != Some(record.reference_comparison_object_sha256.as_str())
            || quality != record.quality_report_object_sha256
        {
            return Err(contract(
                "KNIFE_PASS_STATE_VISUAL_EVIDENCE_BINDING_MISMATCH",
                "visual_evidence object hashes differ from the pass state",
            ));
        }
        (project, reference, comparison)
    } else {
        let view: Option<(String, String, String, String, String, Option<String>, String)> = transaction
            .query_row(
                "SELECT project_id, view_id, reference_id, reference_sha256, camera_hash, comparison_report_object_sha256, quality_status FROM visual_evidence_views WHERE candidate_id = ?1 AND render_set_object_sha256 = ?2 AND quality_report_object_sha256 = ?3",
                params![record.attempt_candidate_id, record.render_set_object_sha256, record.quality_report_object_sha256],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
            )
            .optional()?;
        let Some((
            project,
            view_id,
            reference,
            reference_sha256,
            camera_hash,
            comparison,
            quality_status,
        )) = view
        else {
            return Err(contract(
                "KNIFE_PASS_STATE_VISUAL_EVIDENCE_MISSING",
                "attempt candidate has no durable visual evidence row",
            ));
        };
        let fixed_view_id = record
            .fixed_view
            .get("view_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let fixed_camera_hash = record
            .fixed_view
            .get("camera_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if view_id != fixed_view_id
            || reference != record.reference_id
            || reference_sha256 != record.reference_object_sha256
            || camera_hash != fixed_camera_hash
            || !matches!(
                quality_status.as_str(),
                "QUALITY_TARGET_NOT_MET"
                    | "BLOCKED_REFERENCE_COVERAGE"
                    | "not-run"
                    | "PARTIAL_VISIBLE_VIEW_PASS"
            )
        {
            return Err(contract(
                "KNIFE_PASS_STATE_VISUAL_EVIDENCE_BINDING_MISMATCH",
                "visual_evidence_view is not the exact fixed reference/camera identity",
            ));
        }
        if comparison.as_deref() != Some(record.reference_comparison_object_sha256.as_str()) {
            return Err(contract(
                "KNIFE_PASS_STATE_VISUAL_EVIDENCE_BINDING_MISMATCH",
                "visual_evidence_view comparison object differs from the pass state",
            ));
        }
        (project, reference, comparison)
    };
    if evidence_project != record.project_id || evidence_reference != record.reference_id {
        return Err(contract(
            "KNIFE_PASS_STATE_VISUAL_EVIDENCE_BINDING_MISMATCH",
            "visual evidence project or reference differs",
        ));
    }
    let comparison_object = comparison_object.ok_or_else(|| {
        contract(
            "KNIFE_PASS_STATE_VISUAL_EVIDENCE_MISSING",
            "reference comparison object is missing",
        )
    })?;
    let render = validate_registered_cas(
        transaction,
        cas,
        &record.render_set_object_sha256,
        &[
            "appearance-v2-render-set",
            "render-set-v2",
            "render-set",
            "knife-render-set",
        ],
        &["RenderSet@2", "RenderSet@1", "KnifeRenderSet@1"],
        Some(&record.render_set_sha256),
        require_reachable,
        "RenderSet",
    )?;
    validate_visual_object_binding(&render, record, &record.render_set_id, "RenderSet")?;
    if render.get("candidate_id").and_then(Value::as_str)
        != Some(record.attempt_candidate_id.as_str())
        || render.get("artifact_sha256").and_then(Value::as_str)
            != Some(record.attempt_artifact_sha256.as_str())
        || render.get("program_sha256").and_then(Value::as_str)
            != Some(record.attempt_geometry_program_sha256.as_str())
        || render.get("reference_id").and_then(Value::as_str) != Some(record.reference_id.as_str())
    {
        return Err(contract(
            "KNIFE_PASS_STATE_EVIDENCE_RENDER_BINDING_MISMATCH",
            "RenderSet is not bound to the exact attempt geometry/reference",
        ));
    }
    let render_view_id = render
        .get("view_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_EVIDENCE_VIEW_MISSING",
                "RenderSet must carry the fixed view identity",
            )
        })?;
    let fixed_view_id = record
        .fixed_view
        .get("view_id")
        .and_then(Value::as_str)
        .ok_or_else(|| contract("KNIFE_PASS_STATE_FIXED_VIEW_INVALID", "view_id is missing"))?;
    let reference_view_id = record
        .fixed_view
        .get("reference_view_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_FIXED_VIEW_INVALID",
                "reference_view_id is missing",
            )
        })?;
    if render_view_id != fixed_view_id || render_view_id != reference_view_id {
        return Err(contract(
            "KNIFE_PASS_STATE_EVIDENCE_VIEW_MISMATCH",
            "RenderSet view is not the one bounded fixed view",
        ));
    }
    let camera_hash = render
        .get("camera_hash")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_EVIDENCE_CAMERA_MISSING",
                "RenderSet camera_hash is missing",
            )
        })?;
    if record
        .fixed_view
        .get("camera_sha256")
        .and_then(Value::as_str)
        != Some(camera_hash)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_EVIDENCE_CAMERA_MISMATCH",
            "RenderSet camera hash differs from the fixed view",
        ));
    }
    let camera_object_sha256 = render
        .get("camera_object_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_EVIDENCE_CAMERA_OBJECT_MISSING",
                "RenderSet camera_object_sha256 is missing",
            )
        })?;
    let (camera_object, camera_bytes) = validate_registered_bytes(
        transaction,
        cas,
        camera_object_sha256,
        &["camera-calibration"],
        &[KNIFE_PASS_STATE_JSON_MIME],
        MAX_LINEAGE_JSON_BYTES,
        require_reachable,
        "camera calibration",
    )?;
    let camera = canonical_json_value(&camera_bytes, "camera calibration")?;
    if camera.get("camera_hash").and_then(Value::as_str) != Some(camera_hash)
        || camera
            .get("camera_id")
            .and_then(Value::as_str)
            .is_some_and(|camera_id| {
                camera_id != record.fixed_view["camera_id"].as_str().unwrap_or_default()
            })
    {
        return Err(contract(
            "KNIFE_PASS_STATE_EVIDENCE_CAMERA_MISMATCH",
            "camera calibration object differs from RenderSet camera_hash",
        ));
    }
    let comparison = validate_registered_cas(
        transaction,
        cas,
        &comparison_object,
        &[
            "reference-comparison-report",
            "comparison-report",
            "knife-reference-comparison",
        ],
        &[
            "ReferenceComparisonReport@1",
            "ReferenceComparison@1",
            "KnifeReferenceComparison@1",
        ],
        Some(&record.reference_comparison_sha256),
        require_reachable,
        "ReferenceComparison",
    )?;
    validate_visual_object_binding(
        &comparison,
        record,
        &record.reference_comparison_id,
        "ReferenceComparison",
    )?;
    let comparison_view_id = comparison
        .get("view_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_EVIDENCE_VIEW_MISSING",
                "ReferenceComparisonReport must carry the fixed view identity",
            )
        })?;
    if comparison_view_id != fixed_view_id
        || comparison_view_id != reference_view_id
        || comparison.get("camera_hash").and_then(Value::as_str) != Some(camera_hash)
        || comparison.get("render_set_hash").and_then(Value::as_str)
            != Some(record.render_set_object_sha256.as_str())
        || comparison.get("reference_sha256").and_then(Value::as_str)
            != Some(record.reference_object_sha256.as_str())
        || comparison.get("candidate_id").and_then(Value::as_str)
            != Some(record.attempt_candidate_id.as_str())
        || comparison.get("artifact_sha256").and_then(Value::as_str)
            != Some(record.attempt_artifact_sha256.as_str())
    {
        return Err(contract(
            "KNIFE_PASS_STATE_EVIDENCE_COMPARISON_BINDING_MISMATCH",
            "ReferenceComparisonReport is not bound to the fixed RenderSet/reference",
        ));
    }
    let mask_sha256 = comparison
        .get("mask")
        .and_then(|mask| mask.get("sha256"))
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| {
            contract(
                "KNIFE_PASS_STATE_EVIDENCE_REFERENCE_VIEW_MISSING",
                "ReferenceComparisonReport mask identity is missing",
            )
        })?;
    if record
        .fixed_view
        .get("reference_view_sha256")
        .and_then(Value::as_str)
        != Some(mask_sha256)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_EVIDENCE_REFERENCE_VIEW_MISMATCH",
            "fixed_view reference view is not the durable comparison mask",
        ));
    }
    let (mask_object, _mask_bytes) = validate_registered_bytes(
        transaction,
        cas,
        mask_sha256,
        &["reference-silhouette-mask-v1"],
        &["image/png"],
        MAX_LINEAGE_JSON_BYTES,
        require_reachable,
        "reference silhouette mask",
    )?;
    let quality = validate_registered_cas(
        transaction,
        cas,
        &record.quality_report_object_sha256,
        &[
            "appearance-v2-quality-report",
            "quality-report-v2",
            "quality-report",
            "knife-quality-report",
        ],
        &["QualityReport@2", "QualityReport@1", "KnifeQualityReport@1"],
        Some(&record.quality_report_sha256),
        require_reachable,
        "QualityReport",
    )?;
    validate_visual_object_binding(&quality, record, &record.quality_report_id, "QualityReport")?;
    if quality.get("candidate_id").and_then(Value::as_str)
        != Some(record.attempt_candidate_id.as_str())
        || quality.get("artifact_sha256").and_then(Value::as_str)
            != Some(record.attempt_artifact_sha256.as_str())
        || quality.get("program_sha256").and_then(Value::as_str)
            != Some(record.attempt_geometry_program_sha256.as_str())
        || quality.get("reference_id").and_then(Value::as_str) != Some(record.reference_id.as_str())
        || quality.get("reference_sha256").and_then(Value::as_str)
            != Some(record.reference_object_sha256.as_str())
        || quality.get("render_set_hash").and_then(Value::as_str)
            != Some(record.render_set_object_sha256.as_str())
        || quality
            .get("comparison_report_hash")
            .and_then(Value::as_str)
            != Some(comparison_object.as_str())
        || quality.get("view_id").and_then(Value::as_str) != Some(fixed_view_id)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_EVIDENCE_QUALITY_BINDING_MISMATCH",
            "QualityReport is not bound to the exact candidate/view evidence",
        ));
    }
    let visual_status = match quality.get("visual_status").and_then(Value::as_str) {
        Some("QUALITY_TARGET_NOT_MET") => "QUALITY_TARGET_NOT_MET",
        Some("BLOCKED_REFERENCE_COVERAGE") => "BLOCKED_REFERENCE_COVERAGE",
        Some("not-run") => "NOT_RUN",
        Some("PARTIAL_VISIBLE_VIEW_PASS") => {
            return Err(contract(
                "KNIFE_PASS_STATE_QUALITY_PROMOTION_FORBIDDEN",
                "a PassState cannot promote a visual-only partial view to PASS",
            ));
        }
        _ => {
            return Err(contract(
                "KNIFE_PASS_STATE_EVIDENCE_QUALITY_STATUS_INVALID",
                "QualityReport visual_status is not a conservative PassState status",
            ));
        }
    };
    if record.visual_gate_status != visual_status || record.quality_status != visual_status {
        return Err(contract(
            "KNIFE_PASS_STATE_QUALITY_STATUS_MISMATCH",
            "PassState visual/quality status differs from the durable QualityReport",
        ));
    }
    let mut roots = vec![
        record.render_set_object_sha256.clone(),
        comparison_object,
        record.quality_report_object_sha256.clone(),
        camera_object.sha256,
        mask_object.sha256,
    ];
    roots.sort();
    roots.dedup();
    Ok(VisualEvidenceValidation {
        roots,
        camera_hash: camera_hash.to_owned(),
        visual_status: visual_status.to_owned(),
    })
}

fn roots(record: &KnifePassStateStoreRecord) -> Vec<String> {
    let mut roots = vec![
        record.pass_state_object_sha256.clone(),
        record.source_binding_object_sha256.clone(),
        record.intent_bundle_object_sha256.clone(),
        record.brief_object_sha256.clone(),
        record.reference_object_sha256.clone(),
        record.authoring_mesh_revision_object_sha256.clone(),
        record.baseline_geometry_program_object_sha256.clone(),
        record.baseline_artifact_readback_object_sha256.clone(),
        record.baseline_artifact_sha256.clone(),
        record.attempt_geometry_program_object_sha256.clone(),
        record.attempt_artifact_readback_object_sha256.clone(),
        record.attempt_artifact_sha256.clone(),
        record.render_set_object_sha256.clone(),
        record.reference_comparison_object_sha256.clone(),
        record.quality_report_object_sha256.clone(),
    ];
    roots.sort();
    roots.dedup();
    roots
}

fn load_authoring_mesh_revision(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
) -> Result<AuthoringMeshRevision, StoreError> {
    let value = validate_registered_cas(
        transaction,
        cas,
        &record.authoring_mesh_revision_object_sha256,
        &[AUTHORING_MESH_V2_REVISION_OBJECT_KIND],
        &["AuthoringMeshRevision@2"],
        Some(&record.authoring_mesh_revision_sha256),
        true,
        "AuthoringMesh revision",
    )?;
    serde_json::from_value(value).map_err(|error| {
        contract(
            "KNIFE_PASS_STATE_AUTHORING_MESH_REVISION_INVALID",
            format!("AuthoringMesh revision payload is invalid: {error}"),
        )
    })
}

fn validate_parent_lineage(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let (Some(parent_id), Some(parent_sha256)) = (
        record.parent_pass_id.as_deref(),
        record.parent_pass_sha256.as_deref(),
    ) else {
        if record.parent_pass_id.is_some() || record.parent_pass_sha256.is_some() {
            return Err(contract(
                "KNIFE_PASS_STATE_PARENT_BINDING_INVALID",
                "parent pass identity must be all-null or all-present",
            ));
        }
        return Ok(Vec::new());
    };
    let parent: Option<(String, String)> = transaction
        .query_row(
            "SELECT record_json, pass_state_object_sha256 FROM knife_pass_state_records WHERE project_id = ?1 AND pass_id = ?2 AND canonical_sha256 = ?3",
            params![record.project_id, parent_id, parent_sha256],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((parent_json, parent_object_sha256)) = parent else {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_MISSING",
            "parent pass state is not durably registered",
        ));
    };
    if parent_id == record.pass_id || !is_sha256(&parent_object_sha256) {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_BINDING_INVALID",
            "parent pass state identity is malformed or self-referential",
        ));
    }
    let parent_record: KnifePassStateStoreRecord =
        serde_json::from_str(&parent_json).map_err(|error| {
            contract(
                "KNIFE_PASS_STATE_PARENT_INVALID",
                format!("parent Store record is invalid: {error}"),
            )
        })?;
    if parent_record.project_id != record.project_id
        || parent_record.pass_id != parent_id
        || parent_record.canonical_sha256 != parent_sha256
        || parent_record.pass_state_object_sha256 != parent_object_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_BINDING_MISMATCH",
            "parent Store record differs from the exact parent selector",
        ));
    }
    let parent_object =
        read_object_record(transaction, &parent_object_sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "KNIFE_PASS_STATE_PARENT_CAS_MISSING",
                "parent pass-state CAS object is not registered",
            ),
            other => other,
        })?;
    let parent_payload =
        validate_pass_state_cas_object(transaction, cas, &parent_object, &parent_record, true)?;
    validate_main_payload(&parent_payload, &parent_record)?;
    validate_parent_successor(transaction, cas, &parent_record, record)?;
    Ok(vec![parent_object_sha256])
}

fn knife_pass_stage_rank(stage: &str) -> u8 {
    match stage {
        "camera-lock" => 0,
        "silhouette-blockout" => 1,
        "structural-form" => 2,
        "secondary-form" => 3,
        "high-geometry" => 4,
        _ => u8::MAX,
    }
}

/// Validate the immutable successor relationship after the parent object has
/// itself passed Main/CAS validation.  A child is a correction of exactly the
/// parent attempt: its baseline must copy every candidate artifact/program/
/// readback/plan identity from that attempt, while its new attempt must use a
/// different candidate and a genuinely different program or representation
/// plan.  Shared source/reference/intent/Brief/view identities and monotonic
/// stage/revision constraints prevent a fresh unrelated pass from masquerading
/// as a correction.
fn validate_parent_successor(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    parent: &KnifePassStateStoreRecord,
    record: &KnifePassStateStoreRecord,
) -> Result<(), StoreError> {
    let exact_shared = [
        (
            "project_id",
            parent.project_id.as_str(),
            record.project_id.as_str(),
        ),
        (
            "source_binding_id",
            parent.source_binding_id.as_str(),
            record.source_binding_id.as_str(),
        ),
        (
            "source_binding_sha256",
            parent.source_binding_sha256.as_str(),
            record.source_binding_sha256.as_str(),
        ),
        (
            "source_binding_object_sha256",
            parent.source_binding_object_sha256.as_str(),
            record.source_binding_object_sha256.as_str(),
        ),
        (
            "intent_bundle_id",
            parent.intent_bundle_id.as_str(),
            record.intent_bundle_id.as_str(),
        ),
        (
            "intent_bundle_sha256",
            parent.intent_bundle_sha256.as_str(),
            record.intent_bundle_sha256.as_str(),
        ),
        (
            "intent_bundle_object_sha256",
            parent.intent_bundle_object_sha256.as_str(),
            record.intent_bundle_object_sha256.as_str(),
        ),
        (
            "brief_id",
            parent.brief_id.as_str(),
            record.brief_id.as_str(),
        ),
        (
            "brief_sha256",
            parent.brief_sha256.as_str(),
            record.brief_sha256.as_str(),
        ),
        (
            "brief_object_sha256",
            parent.brief_object_sha256.as_str(),
            record.brief_object_sha256.as_str(),
        ),
        (
            "reference_id",
            parent.reference_id.as_str(),
            record.reference_id.as_str(),
        ),
        (
            "reference_object_sha256",
            parent.reference_object_sha256.as_str(),
            record.reference_object_sha256.as_str(),
        ),
        (
            "reference_evidence_sha256",
            parent.reference_evidence_sha256.as_str(),
            record.reference_evidence_sha256.as_str(),
        ),
        (
            "source_candidate_id",
            parent.source_candidate_id.as_str(),
            record.source_candidate_id.as_str(),
        ),
        (
            "source_candidate_state_sha256",
            parent.source_candidate_state_sha256.as_str(),
            record.source_candidate_state_sha256.as_str(),
        ),
        (
            "authoring_mesh_id",
            parent.authoring_mesh_id.as_str(),
            record.authoring_mesh_id.as_str(),
        ),
        (
            "authoring_mesh_lineage_id",
            parent.authoring_mesh_lineage_id.as_str(),
            record.authoring_mesh_lineage_id.as_str(),
        ),
        (
            "authoring_mesh_identity_sha256",
            parent.authoring_mesh_identity_sha256.as_str(),
            record.authoring_mesh_identity_sha256.as_str(),
        ),
    ];
    if let Some((field, _, _)) = exact_shared
        .iter()
        .find(|(_, parent_value, child_value)| parent_value != child_value)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_SUCCESSOR_BINDING_MISMATCH",
            format!("parent and correction differ in {field}"),
        ));
    }
    if parent.fixed_view != record.fixed_view
        || parent.camera_set_sha256 != record.camera_set_sha256
    {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_SUCCESSOR_VIEW_MISMATCH",
            "correction must preserve the exact fixed view and camera set",
        ));
    }
    if knife_pass_stage_rank(record.stage.as_str()) < knife_pass_stage_rank(parent.stage.as_str()) {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_SUCCESSOR_STAGE_REGRESSION",
            "correction stage cannot move backwards",
        ));
    }
    if record.authoring_mesh_revision_index <= parent.authoring_mesh_revision_index {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_SUCCESSOR_REVISION_REGRESSION",
            "correction must use a higher AuthoringMesh revision index",
        ));
    }
    let current_revision = load_authoring_mesh_revision(transaction, cas, record)?;
    if current_revision.parent_revision_ids.len() != 1
        || current_revision.parent_revision_ids[0].0 != parent.authoring_mesh_revision_id
    {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_SUCCESSOR_REVISION_PARENT_MISMATCH",
            "correction AuthoringMesh revision must directly descend from the parent pass revision",
        ));
    }

    let parent_attempt = [
        (
            "candidate_id",
            parent.attempt_candidate_id.as_str(),
            record.baseline_candidate_id.as_str(),
        ),
        (
            "candidate_state_sha256",
            parent.attempt_candidate_state_sha256.as_str(),
            record.baseline_candidate_state_sha256.as_str(),
        ),
        (
            "artifact_sha256",
            parent.attempt_artifact_sha256.as_str(),
            record.baseline_artifact_sha256.as_str(),
        ),
        (
            "geometry_program_sha256",
            parent.attempt_geometry_program_sha256.as_str(),
            record.baseline_geometry_program_sha256.as_str(),
        ),
        (
            "geometry_program_object_sha256",
            parent.attempt_geometry_program_object_sha256.as_str(),
            record.baseline_geometry_program_object_sha256.as_str(),
        ),
        (
            "artifact_readback_object_sha256",
            parent.attempt_artifact_readback_object_sha256.as_str(),
            record.baseline_artifact_readback_object_sha256.as_str(),
        ),
        (
            "representation_plan_sha256",
            parent.attempt_representation_plan_sha256.as_str(),
            record.baseline_representation_plan_sha256.as_str(),
        ),
    ];
    if let Some((field, _, _)) = parent_attempt
        .iter()
        .find(|(_, parent_value, child_value)| parent_value != child_value)
    {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_SUCCESSOR_BASELINE_MISMATCH",
            format!("correction baseline differs from parent attempt {field}"),
        ));
    }
    if record.attempt_candidate_id == record.baseline_candidate_id {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_SUCCESSOR_ATTEMPT_NOT_NEW",
            "correction attempt candidate must differ from its baseline candidate",
        ));
    }
    let changed_geometry = record.attempt_geometry_program_sha256
        != record.baseline_geometry_program_sha256
        || record.attempt_geometry_program_object_sha256
            != record.baseline_geometry_program_object_sha256
        || record.attempt_representation_plan_sha256 != record.baseline_representation_plan_sha256;
    if !changed_geometry {
        return Err(contract(
            "KNIFE_PASS_STATE_PARENT_SUCCESSOR_GEOMETRY_UNCHANGED",
            "correction attempt must change its GeometryProgram or representation plan",
        ));
    }
    Ok(())
}

fn same_record(left: &KnifePassStateStoreRecord, right: &KnifePassStateStoreRecord) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

fn read_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnifePassStateStoreRecord> {
    let record_json: String = row.get(0)?;
    serde_json::from_str(&record_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

/// Additive migration for the pass-state index.  Main truth remains in CAS;
/// SQLite only indexes exact lineage and object roots for restart/GC queries.
pub(crate) fn ensure_table(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS knife_pass_state_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'KnifePassStateStoreRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             pass_id TEXT NOT NULL,
             canonical_sha256 TEXT NOT NULL,
             pass_state_object_sha256 TEXT NOT NULL UNIQUE REFERENCES objects(sha256),
             idempotency_key TEXT NOT NULL,
             source_binding_id TEXT NOT NULL,
             source_binding_sha256 TEXT NOT NULL,
             source_binding_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             intent_bundle_id TEXT NOT NULL,
             intent_bundle_sha256 TEXT NOT NULL,
             intent_bundle_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             brief_id TEXT NOT NULL,
             brief_sha256 TEXT NOT NULL,
             brief_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             reference_id TEXT NOT NULL REFERENCES reference_evidence(reference_id),
             reference_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             reference_evidence_sha256 TEXT NOT NULL,
             source_candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             source_candidate_state_sha256 TEXT NOT NULL,
             baseline_candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             baseline_candidate_state_sha256 TEXT NOT NULL,
             baseline_artifact_sha256 TEXT NOT NULL,
             baseline_geometry_program_sha256 TEXT NOT NULL,
             baseline_geometry_program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             baseline_artifact_readback_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             baseline_representation_plan_sha256 TEXT NOT NULL,
             attempt_candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             attempt_candidate_state_sha256 TEXT NOT NULL,
             attempt_artifact_sha256 TEXT NOT NULL,
             attempt_geometry_program_sha256 TEXT NOT NULL,
             attempt_geometry_program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             attempt_artifact_readback_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             attempt_representation_plan_sha256 TEXT NOT NULL,
             authoring_mesh_id TEXT NOT NULL,
             authoring_mesh_lineage_id TEXT NOT NULL,
             authoring_mesh_revision_id TEXT NOT NULL,
             authoring_mesh_revision_index INTEGER NOT NULL CHECK (authoring_mesh_revision_index BETWEEN 0 AND 1000000),
             authoring_mesh_revision_sha256 TEXT NOT NULL,
             authoring_mesh_revision_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             authoring_mesh_identity_sha256 TEXT NOT NULL,
             authoring_mesh_sha256 TEXT NOT NULL,
             camera_set_sha256 TEXT NOT NULL,
             render_set_id TEXT NOT NULL,
             render_set_sha256 TEXT NOT NULL,
             render_set_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             reference_comparison_id TEXT NOT NULL,
             reference_comparison_sha256 TEXT NOT NULL,
             reference_comparison_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             quality_report_id TEXT NOT NULL,
             quality_report_sha256 TEXT NOT NULL,
             quality_report_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             evidence_bundle_sha256 TEXT NOT NULL,
             record_json TEXT NOT NULL,
             created_at TEXT NOT NULL,
             PRIMARY KEY (project_id, pass_id),
             UNIQUE (project_id, idempotency_key),
             UNIQUE (project_id, canonical_sha256)
         );
         CREATE INDEX IF NOT EXISTS knife_pass_state_project_idx
             ON knife_pass_state_records(project_id, created_at DESC, pass_id ASC);
         CREATE INDEX IF NOT EXISTS knife_pass_state_lineage_idx
             ON knife_pass_state_records(source_binding_id, source_candidate_id, authoring_mesh_revision_id);
         CREATE INDEX IF NOT EXISTS knife_pass_state_object_idx
             ON knife_pass_state_records(pass_state_object_sha256, source_binding_object_sha256, render_set_object_sha256, reference_comparison_object_sha256, quality_report_object_sha256);",
    )?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS knife_pass_state_roots (
             project_id TEXT NOT NULL,
             pass_id TEXT NOT NULL,
             object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             root_kind TEXT NOT NULL,
             PRIMARY KEY (project_id, pass_id, object_sha256),
             FOREIGN KEY (project_id, pass_id) REFERENCES knife_pass_state_records(project_id, pass_id)
         );
         CREATE INDEX IF NOT EXISTS knife_pass_state_roots_object_idx
             ON knife_pass_state_roots(object_sha256);",
    )?;
    for (name, definition) in [
        (
            "baseline_geometry_program_sha256",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "baseline_geometry_program_object_sha256",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "baseline_artifact_readback_object_sha256",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "baseline_representation_plan_sha256",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "attempt_geometry_program_sha256",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "attempt_geometry_program_object_sha256",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "attempt_artifact_readback_object_sha256",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "attempt_representation_plan_sha256",
            "TEXT NOT NULL DEFAULT ''",
        ),
    ] {
        ensure_pass_state_column(transaction, name, definition)?;
    }
    transaction.execute_batch(
        "CREATE INDEX IF NOT EXISTS knife_pass_state_lineage_objects_idx
             ON knife_pass_state_records(baseline_geometry_program_object_sha256, baseline_artifact_readback_object_sha256, attempt_geometry_program_object_sha256, attempt_artifact_readback_object_sha256);",
    )?;
    Ok(())
}

fn ensure_pass_state_column(
    transaction: &Transaction<'_>,
    name: &str,
    definition: &str,
) -> Result<(), StoreError> {
    let exists: Option<String> = transaction
        .query_row(
            "SELECT name FROM pragma_table_info('knife_pass_state_records') WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        transaction.execute(
            &format!("ALTER TABLE knife_pass_state_records ADD COLUMN {name} {definition}"),
            [],
        )?;
    }
    Ok(())
}

fn persist_root_index(
    transaction: &Transaction<'_>,
    record: &KnifePassStateStoreRecord,
    resolved_roots: &[String],
) -> Result<(), StoreError> {
    for object_sha256 in resolved_roots {
        transaction.execute(
            "INSERT OR IGNORE INTO knife_pass_state_roots (project_id, pass_id, object_sha256, root_kind) VALUES (?1, ?2, ?3, ?4)",
            params![record.project_id, record.pass_id, object_sha256, "resolved-lineage"],
        )?;
    }
    Ok(())
}

fn read_by_identity(
    transaction: &Transaction<'_>,
    project_id: &str,
    pass_id: &str,
    canonical_sha256: &str,
) -> Result<Option<KnifePassStateStoreRecord>, StoreError> {
    Ok(transaction
        .query_row(
            "SELECT record_json FROM knife_pass_state_records WHERE project_id = ?1 AND pass_id = ?2 AND canonical_sha256 = ?3",
            params![project_id, pass_id, canonical_sha256],
            read_record,
        )
        .optional()?)
}

fn validate_full_record(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &KnifePassStateStoreRecord,
    payload: Option<&[u8]>,
    require_reachable: bool,
) -> Result<Vec<String>, StoreError> {
    validate_record(record)?;
    if let Some(payload) = payload {
        validate_main_payload(payload, record)?;
    }
    let mut resolved_roots = Vec::new();
    resolved_roots.extend(validate_source_binding_lineage(transaction, cas, record)?);
    resolved_roots.extend(validate_intent_lineage(transaction, cas, record)?);
    resolved_roots.extend(validate_brief_lineage(transaction, cas, record)?);
    resolved_roots.extend(validate_reference_lineage(transaction, cas, record)?);
    let mesh_roots = validate_authoring_mesh_lineage(transaction, cas, record)?;
    resolved_roots.extend(mesh_roots);
    resolved_roots.extend(validate_optional_curve_lineage(transaction, cas, record)?);
    // Establish the parent/successor role mapping before interpreting the
    // candidate fields.  Otherwise a forged child baseline can fail later as
    // a materializer-plan mismatch and obscure the stable parent-lineage
    // contract that it actually violated.
    let mut parent_roots = validate_parent_lineage(transaction, cas, record)?;
    resolved_roots.append(&mut parent_roots);
    let candidate_lineage = validate_candidate_lineage(
        transaction,
        cas,
        record,
        &load_authoring_mesh_revision(transaction, cas, record)?,
    )?;
    resolved_roots.extend(candidate_lineage.roots);
    let visual = validate_visual_evidence(transaction, cas, record, require_reachable)?;
    resolved_roots.extend(visual.roots);
    let expected_hard_gate = if candidate_lineage.attempt_hard_gate_passed {
        "PASS_SOURCE_STRUCTURAL"
    } else {
        "BLOCKED"
    };
    if record.hard_gate_status != expected_hard_gate {
        return Err(contract(
            "KNIFE_PASS_STATE_HARD_GATE_STATUS_MISMATCH",
            "PassState hard_gate_status differs from candidate readback evidence",
        ));
    }
    if record.visual_gate_status != visual.visual_status
        || record.quality_status != visual.visual_status
    {
        return Err(contract(
            "KNIFE_PASS_STATE_STATUS_DERIVATION_MISMATCH",
            "PassState visual/quality status differs from durable evidence",
        ));
    }
    let evidence_bundle = json!({
        "schema_version":KNIFE_PASS_STATE_EVIDENCE_BUNDLE_SCHEMA_VERSION,
        "render_set_sha256":record.render_set_sha256,
        "reference_comparison_sha256":record.reference_comparison_sha256,
        "quality_report_sha256":record.quality_report_sha256,
        "camera_set_sha256":record.camera_set_sha256,
    });
    if canonical_json_hash(&evidence_bundle) != record.evidence_bundle_sha256 {
        return Err(contract(
            "KNIFE_PASS_STATE_EVIDENCE_BUNDLE_MISMATCH",
            "evidence_bundle_sha256 is not the exact RenderSet/Comparison/Quality/camera-set bundle",
        ));
    }
    if record
        .fixed_view
        .get("camera_sha256")
        .and_then(Value::as_str)
        != Some(visual.camera_hash.as_str())
    {
        return Err(contract(
            "KNIFE_PASS_STATE_CAMERA_BINDING_MISMATCH",
            "PassState camera identity differs from durable RenderSet evidence",
        ));
    }
    resolved_roots.sort();
    resolved_roots.dedup();
    if require_reachable {
        for hash in &resolved_roots {
            let object = read_object_record(transaction, &hash)?;
            if object.reachability != "reachable" {
                return Err(contract(
                    "KNIFE_PASS_STATE_CAS_NOT_REACHABLE",
                    "a committed pass-state root is not reachable",
                ));
            }
        }
    }
    Ok(resolved_roots)
}

impl Store {
    /// Atomically install one immutable pass state.  The return flag is true
    /// only for exact idempotency replay; conflicts never touch a new row or
    /// mark any new CAS root reachable.
    pub fn record_knife_pass_state_with_replay(
        &self,
        commit: &KnifePassStateCommit,
    ) -> Result<(KnifePassStateStoreRecord, bool), StoreError> {
        validate_record(&commit.record)?;
        if commit.cas.pass_state.sha256 != commit.record.pass_state_object_sha256 {
            return Err(contract(
                "KNIFE_PASS_STATE_CAS_BINDING_MISMATCH",
                "pass state CAS object hash differs from its Store binding",
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let payload = validate_pass_state_cas_object(
            &transaction,
            &self.cas,
            &commit.cas.pass_state,
            &commit.record,
            false,
        )?;
        validate_main_payload(&payload, &commit.record)?;

        let existing = transaction
            .query_row(
                "SELECT record_json FROM knife_pass_state_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![commit.record.project_id, commit.record.idempotency_key],
                read_record,
            )
        .optional()?;
        if let Some(existing) = existing {
            let existing_roots =
                validate_full_record(&transaction, &self.cas, &existing, None, true)?;
            persist_root_index(&transaction, &existing, &existing_roots)?;
            if !same_record(&existing, &commit.record) {
                return Err(contract(
                    "KNIFE_PASS_STATE_IDEMPOTENCY_CONFLICT",
                    "project and idempotency key are already bound to different pass metadata",
                ));
            }
            let existing_payload = self
                .cas
                .read_verified_bounded(
                    &existing.pass_state_object_sha256,
                    KNIFE_PASS_STATE_MAX_JSON_BYTES,
                )
                .map_err(StoreError::from)?;
            validate_main_payload(&existing_payload, &existing)?;
            if payload != existing_payload {
                return Err(contract(
                    "KNIFE_PASS_STATE_REPLAY_PAYLOAD_MISMATCH",
                    "idempotency replay payload differs from the immutable Main object",
                ));
            }
            transaction.commit()?;
            return Ok((existing, true));
        }
        let pass_conflict: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM knife_pass_state_records WHERE project_id = ?1 AND pass_id = ?2",
                params![commit.record.project_id, commit.record.pass_id],
                |row| row.get(0),
            )
            .optional()?;
        if pass_conflict.is_some() {
            return Err(contract(
                "KNIFE_PASS_STATE_PASS_CONFLICT",
                "pass id is already bound to a different immutable pass state",
            ));
        }

        let resolved_roots = validate_full_record(
            &transaction,
            &self.cas,
            &commit.record,
            Some(&payload),
            false,
        )?;
        let mut resolved_roots = resolved_roots;
        resolved_roots.push(commit.record.pass_state_object_sha256.clone());
        resolved_roots.sort();
        resolved_roots.dedup();
        let record_json = String::from_utf8(record_bytes(&commit.record)?).map_err(|error| {
            StoreError::InvalidData(format!("pass state Store record is not UTF-8: {error}"))
        })?;
        transaction.execute(
            "INSERT INTO knife_pass_state_records (schema_version, project_id, pass_id, canonical_sha256, pass_state_object_sha256, idempotency_key, source_binding_id, source_binding_sha256, source_binding_object_sha256, intent_bundle_id, intent_bundle_sha256, intent_bundle_object_sha256, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, source_candidate_id, source_candidate_state_sha256, baseline_candidate_id, baseline_candidate_state_sha256, baseline_artifact_sha256, baseline_geometry_program_sha256, baseline_geometry_program_object_sha256, baseline_artifact_readback_object_sha256, baseline_representation_plan_sha256, attempt_candidate_id, attempt_candidate_state_sha256, attempt_artifact_sha256, attempt_geometry_program_sha256, attempt_geometry_program_object_sha256, attempt_artifact_readback_object_sha256, attempt_representation_plan_sha256, authoring_mesh_id, authoring_mesh_lineage_id, authoring_mesh_revision_id, authoring_mesh_revision_index, authoring_mesh_revision_sha256, authoring_mesh_revision_object_sha256, authoring_mesh_identity_sha256, authoring_mesh_sha256, camera_set_sha256, render_set_id, render_set_sha256, render_set_object_sha256, reference_comparison_id, reference_comparison_sha256, reference_comparison_object_sha256, quality_report_id, quality_report_sha256, quality_report_object_sha256, evidence_bundle_sha256, record_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54, ?55)",
            params![
                commit.record.schema_version,
                commit.record.project_id,
                commit.record.pass_id,
                commit.record.canonical_sha256,
                commit.record.pass_state_object_sha256,
                commit.record.idempotency_key,
                commit.record.source_binding_id,
                commit.record.source_binding_sha256,
                commit.record.source_binding_object_sha256,
                commit.record.intent_bundle_id,
                commit.record.intent_bundle_sha256,
                commit.record.intent_bundle_object_sha256,
                commit.record.brief_id,
                commit.record.brief_sha256,
                commit.record.brief_object_sha256,
                commit.record.reference_id,
                commit.record.reference_object_sha256,
                commit.record.reference_evidence_sha256,
                commit.record.source_candidate_id,
                commit.record.source_candidate_state_sha256,
                commit.record.baseline_candidate_id,
                commit.record.baseline_candidate_state_sha256,
                commit.record.baseline_artifact_sha256,
                commit.record.baseline_geometry_program_sha256,
                commit.record.baseline_geometry_program_object_sha256,
                commit.record.baseline_artifact_readback_object_sha256,
                commit.record.baseline_representation_plan_sha256,
                commit.record.attempt_candidate_id,
                commit.record.attempt_candidate_state_sha256,
                commit.record.attempt_artifact_sha256,
                commit.record.attempt_geometry_program_sha256,
                commit.record.attempt_geometry_program_object_sha256,
                commit.record.attempt_artifact_readback_object_sha256,
                commit.record.attempt_representation_plan_sha256,
                commit.record.authoring_mesh_id,
                commit.record.authoring_mesh_lineage_id,
                commit.record.authoring_mesh_revision_id,
                i64::try_from(commit.record.authoring_mesh_revision_index).map_err(|_| StoreError::InvalidData("pass state revision index is too large".to_owned()))?,
                commit.record.authoring_mesh_revision_sha256,
                commit.record.authoring_mesh_revision_object_sha256,
                commit.record.authoring_mesh_identity_sha256,
                commit.record.authoring_mesh_sha256,
                commit.record.camera_set_sha256,
                commit.record.render_set_id,
                commit.record.render_set_sha256,
                commit.record.render_set_object_sha256,
                commit.record.reference_comparison_id,
                commit.record.reference_comparison_sha256,
                commit.record.reference_comparison_object_sha256,
                commit.record.quality_report_id,
                commit.record.quality_report_sha256,
                commit.record.quality_report_object_sha256,
                commit.record.evidence_bundle_sha256,
                record_json,
                commit.record.created_at,
            ],
        )?;
        persist_root_index(&transaction, &commit.record, &resolved_roots)?;
        mark_reachable_in_transaction(&transaction, &resolved_roots)?;
        let stored = transaction
            .query_row(
                "SELECT record_json FROM knife_pass_state_records WHERE project_id = ?1 AND pass_id = ?2",
                params![commit.record.project_id, commit.record.pass_id],
                read_record,
            )?;
        let _ = validate_full_record(&transaction, &self.cas, &stored, None, true)?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn get_knife_pass_state(
        &self,
        project_id: &str,
        pass_id: &str,
        canonical_sha256: &str,
    ) -> Result<Option<KnifePassStateStoreRecord>, StoreError> {
        if !validate_identifier(project_id)
            || !validate_identifier(pass_id)
            || !is_sha256(canonical_sha256)
        {
            return Err(StoreError::InvalidData(
                "pass state lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let Some(record) = read_by_identity(&transaction, project_id, pass_id, canonical_sha256)?
        else {
            transaction.commit()?;
            return Ok(None);
        };
        let payload = self
            .cas
            .read_verified_bounded(
                &record.pass_state_object_sha256,
                KNIFE_PASS_STATE_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        let _ = validate_full_record(&transaction, &self.cas, &record, Some(&payload), true)?;
        transaction.commit()?;
        Ok(Some(record))
    }

    pub fn get_knife_pass_state_by_idempotency(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<KnifePassStateStoreRecord>, StoreError> {
        if !validate_identifier(project_id) || !validate_identifier(idempotency_key) {
            return Err(StoreError::InvalidData(
                "pass state idempotency identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_table(&transaction)?;
        let record = transaction
            .query_row(
                "SELECT record_json FROM knife_pass_state_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![project_id, idempotency_key],
                read_record,
            )
            .optional()?;
        let Some(record) = record else {
            transaction.commit()?;
            return Ok(None);
        };
        let payload = self
            .cas
            .read_verified_bounded(
                &record.pass_state_object_sha256,
                KNIFE_PASS_STATE_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        let _ = validate_full_record(&transaction, &self.cas, &record, Some(&payload), true)?;
        transaction.commit()?;
        Ok(Some(record))
    }

    pub fn read_knife_pass_state_json(
        &self,
        record: &KnifePassStateStoreRecord,
    ) -> Result<Vec<u8>, StoreError> {
        validate_record(record)?;
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.pass_state_object_sha256,
                KNIFE_PASS_STATE_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_main_payload(&bytes, record)?;
        Ok(bytes)
    }

    pub fn knife_pass_state_cas_roots(record: &KnifePassStateStoreRecord) -> Vec<String> {
        roots(record)
    }

    /// Exact-record lookup for Runtime readback callers that already decoded
    /// every required identity from a Get request.  The closed Store record is
    /// intentionally the argument so no four-hash compatibility shortcut can
    /// accidentally omit candidate, AMV2, fixed-view or status identities.
    pub fn get_knife_pass_state_exact_record(
        &self,
        expected: &KnifePassStateStoreRecord,
    ) -> Result<Option<KnifePassStateStoreRecord>, StoreError> {
        validate_record(expected)?;
        let Some(record) = self.get_knife_pass_state(
            &expected.project_id,
            &expected.pass_id,
            &expected.canonical_sha256,
        )?
        else {
            return Ok(None);
        };
        if !same_record(&record, expected) {
            return Err(contract(
                "KNIFE_PASS_STATE_EXACT_LOOKUP_MISMATCH",
                "pass state exact record identity differs",
            ));
        }
        Ok(Some(record))
    }

    /// Full-identity alias retained for callers that prefer an explicit name.
    pub fn get_knife_pass_state_exact_full(
        &self,
        expected: &KnifePassStateStoreRecord,
    ) -> Result<Option<KnifePassStateStoreRecord>, StoreError> {
        self.get_knife_pass_state_exact_record(expected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        KnifeSourceBindingDownstreamBindingRequirements, ProjectRecord, ReferenceAuthorization,
        ReferenceEvidenceRecord, KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY,
        KNIFE_SOURCE_BINDING_BINDING_STATUS, KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY,
        KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY, KNIFE_SOURCE_BINDING_POLICY,
    };
    use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
    use rusqlite::params;
    use serde_json::json;
    use std::fs;
    use uuid::Uuid;

    const PROJECT: &str = "knife-pass-state-project";
    const PASS: &str = "knife-pass-state-pass";
    const SOURCE_BINDING: &str = "knife-pass-state-source-binding";
    const INTENT: &str = "knife-pass-state-intent";
    const BRIEF: &str = "knife-pass-state-brief";
    const REFERENCE: &str = "knife-pass-state-reference";
    const SOURCE_CANDIDATE: &str = "knife-pass-state-source-candidate";
    const BASELINE_CANDIDATE: &str = "knife-pass-state-baseline-candidate";
    const ATTEMPT_CANDIDATE: &str = "knife-pass-state-attempt-candidate";
    const MESH: &str = "knife-pass-state-mesh";
    const LINEAGE: &str = "knife-pass-state-lineage";
    const REVISION: &str = "knife-pass-state-revision";
    const SOURCE_NODE: &str = "knife-pass-state-source-node";
    const PART: &str = "knife-pass-state-blade";
    const ZONE: &str = "knife-pass-state-blade-zone";
    const SOURCE_QUALITY: &str = "knife-pass-state-source-quality";
    const BASELINE_QUALITY: &str = "knife-pass-state-baseline-quality";
    const ATTEMPT_QUALITY: &str = "knife-pass-state-attempt-quality";
    const RENDER_SET: &str = "knife-pass-state-render-set";
    const COMPARISON: &str = "knife-pass-state-comparison";
    const QUALITY: &str = "knife-pass-state-quality";
    const VIEW: &str = "knife-pass-state-view";
    const CAMERA: &str = "knife-pass-state-camera";
    const QUALITY_CONTRACT: &str = "knife-pass-state-quality-contract";
    const CHILD_PASS: &str = "knife-pass-state-child-pass";
    const CHILD_REVISION: &str = "knife-pass-state-child-revision";
    const CHILD_ATTEMPT_CANDIDATE: &str = "knife-pass-state-child-attempt-candidate";
    const CHILD_ATTEMPT_QUALITY: &str = "knife-pass-state-child-attempt-quality";
    const CHILD_RENDER_SET: &str = "knife-pass-state-child-render-set";
    const CHILD_COMPARISON: &str = "knife-pass-state-child-comparison";
    const CHILD_QUALITY: &str = "knife-pass-state-child-quality";
    const NOW: &str = "2026-08-31T00:00:00Z";

    fn h(byte: char) -> String {
        // Keep the fixture shorthand readable while always producing a real
        // hexadecimal SHA-256 identity (the Store must reject non-hex fake
        // hashes before it reaches any lineage lookup).
        sha256_hex(byte.to_string().as_bytes())
    }

    fn put_canonical_json(
        store: &Store,
        mut value: Value,
        kind: &str,
    ) -> (CasObjectRecord, String, Value) {
        value["canonical_sha256"] = Value::String(String::new());
        let semantic = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(semantic.clone());
        let bytes = canonical_json_bytes(&value).expect("canonical JSON");
        let object = store
            .put_object(&bytes, None, KNIFE_PASS_STATE_JSON_MIME, kind, NOW)
            .expect("canonical CAS object");
        (object.record, semantic, value)
    }

    fn put_draft_json(store: &Store, value: Value, kind: &str) -> (CasObjectRecord, String) {
        let bytes = canonical_json_bytes(&value).expect("draft JSON");
        let semantic = sha256_hex(&bytes);
        let object = store
            .put_object(&bytes, None, KNIFE_PASS_STATE_JSON_MIME, kind, NOW)
            .expect("draft CAS object");
        assert_eq!(object.record.sha256, semantic);
        (object.record, semantic)
    }

    fn insert_project(store: &Store) {
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: PROJECT.to_owned(),
                name: "Knife PassState Store fixture".to_owned(),
                policy: json!({"scope":"knife-pass-state-test"}),
                created_at: NOW.to_owned(),
                updated_at: NOW.to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: h('p'),
            })
            .expect("project");
    }

    fn insert_candidate(
        store: &Store,
        candidate_id: &str,
        state_sha256: &str,
        artifact_sha256: &str,
        quality_report_id: &str,
        hard_gate_passed: bool,
    ) {
        let connection = store.connection.lock().expect("connection");
        connection
            .execute(
                "INSERT INTO candidates (candidate_id, project_id, base_version_id, source_version_id, prepared_object_id, prepared_object_sha256, state, request_sha256, manifest_hash, quality_report_id, quality_hard_gate_passed, canonical_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, NULL, NULL, ?3, ?4, 'reviewable', ?5, NULL, ?6, ?7, ?8, NULL, ?9, ?9)",
                params![
                    candidate_id,
                    PROJECT,
                    format!("{candidate_id}-artifact"),
                    artifact_sha256,
                    h('r'),
                    quality_report_id,
                    if hard_gate_passed { 1 } else { 0 },
                    state_sha256,
                    NOW,
                ],
            )
            .expect("candidate row");
    }

    fn insert_geometry_evidence(
        store: &Store,
        candidate_id: &str,
        reference_sha256: &str,
        program_sha256: &str,
        program_object_sha256: &str,
        artifact_sha256: &str,
        readback_object_sha256: &str,
        quality_object_sha256: &str,
        quality_report_id: &str,
        operator_catalog_sha256: &str,
        readback_config_sha256: &str,
    ) {
        let row_identity = json!({
            "candidate_id": candidate_id,
            "project_id": PROJECT,
            "reference_id": REFERENCE,
            "reference_sha256": reference_sha256,
            "geometry_program_sha256": program_sha256,
            "geometry_program_object_sha256": program_object_sha256,
            "operator_catalog_sha256": operator_catalog_sha256,
            "readback_config_sha256": readback_config_sha256,
            "artifact_object_sha256": artifact_sha256,
            "artifact_readback_object_sha256": readback_object_sha256,
            "quality_report_object_sha256": quality_object_sha256,
            "quality_report_id": quality_report_id,
        });
        let canonical = canonical_json_hash(&row_identity);
        let connection = store.connection.lock().expect("connection");
        connection
            .execute(
                "INSERT INTO geometry_candidate_evidence (candidate_id, project_id, reference_id, reference_sha256, geometry_program_sha256, geometry_program_object_sha256, operator_catalog_sha256, readback_config_sha256, artifact_object_sha256, artifact_readback_object_sha256, quality_report_object_sha256, quality_report_id, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    candidate_id,
                    PROJECT,
                    REFERENCE,
                    reference_sha256,
                    program_sha256,
                    program_object_sha256,
                    operator_catalog_sha256,
                    readback_config_sha256,
                    artifact_sha256,
                    readback_object_sha256,
                    quality_object_sha256,
                    quality_report_id,
                    canonical,
                    NOW,
                ],
            )
            .expect("geometry evidence row");
    }

    fn mark_all_reachable(store: &Store) {
        store
            .connection
            .lock()
            .expect("connection")
            .execute("UPDATE objects SET reachability = 'reachable'", [])
            .expect("mark fixture roots");
    }

    fn source_public_payload(record: &KnifeSourceBindingStoreRecord) -> Value {
        let mut object = serde_json::to_value(record)
            .expect("source Store record")
            .as_object()
            .expect("source Store object")
            .clone();
        object.insert(
            "schema_version".to_owned(),
            Value::String("KnifeSourceBinding@1".to_owned()),
        );
        object.remove("source_binding_sha256");
        object.remove("source_binding_object_sha256");
        object.remove("idempotency_key");
        object.insert(
            "canonical_sha256".to_owned(),
            Value::String(record.source_binding_sha256.clone()),
        );
        Value::Object(object)
    }

    fn revision_value(source_binding: Value) -> (Value, String) {
        let mut original = json!({
            "namespace": "original",
            "lineage_id": LINEAGE,
            "vertices": [],
            "edges": [],
            "half_edges": [],
            "corners": [],
            "faces": [],
            "loops": [],
            "rings": [],
            "tombstones": [],
            "canonical_sha256": ""
        });
        original["canonical_sha256"] = Value::String(canonical_json_hash(&original));
        let mut revision = json!({
            "schema_version": "AuthoringMeshRevision@2",
            "mesh_id": MESH,
            "lineage_id": LINEAGE,
            "revision_id": REVISION,
            "parent_revision_ids": [],
            "revision_index": 0,
            "operation": null,
            "original": original,
            "evaluated": null,
            "source_binding": source_binding,
            "id_policy": "runtime-derived-lineage-operation-parent-stable-no-reuse@2",
            "canonical_sha256": ""
        });
        let revision_sha256 = canonical_json_hash(&revision);
        revision["canonical_sha256"] = Value::String(revision_sha256.clone());
        (revision, revision_sha256)
    }

    struct PassFixture {
        commit: KnifePassStateCommit,
        camera_object_sha256: String,
        mask_object_sha256: String,
        source_program_object_sha256: String,
        source_artifact_readback_object_sha256: String,
        source_quality_object_sha256: String,
        quality_contract_object_sha256: String,
    }

    fn setup_fixture(store: &Store) -> PassFixture {
        insert_project(store);

        let reference_object = store
            .put_object(
                b"knife-pass-state-reference",
                None,
                "image/png",
                "reference-image",
                NOW,
            )
            .expect("reference object");
        let authorization = ReferenceAuthorization {
            user_authorized: true,
            declaration: "authorized Store fixture source".to_owned(),
        };
        let reference_evidence_sha256 = canonical_json_hash(&json!({
            "schema_version": "ReferenceEvidence@1",
            "reference_id": REFERENCE,
            "project_id": PROJECT,
            "object_sha256": reference_object.record.sha256,
            "mime": "image/png",
            "size_bytes": reference_object.record.size_bytes,
            "width": 1,
            "height": 1,
            "frame_count": 1,
            "import_mode": "inline_content",
            "authorization": authorization,
            "derived_object_sha256": Value::Null,
            "created_at": NOW,
        }));
        store
            .insert_reference_evidence(&ReferenceEvidenceRecord {
                schema_version: "ReferenceEvidence@1".to_owned(),
                reference_id: REFERENCE.to_owned(),
                project_id: PROJECT.to_owned(),
                object_sha256: reference_object.record.sha256.clone(),
                mime: "image/png".to_owned(),
                size_bytes: reference_object.record.size_bytes,
                width: 1,
                height: 1,
                frame_count: 1,
                import_mode: "inline_content".to_owned(),
                authorization,
                derived_object_sha256: None,
                canonical_sha256: reference_evidence_sha256.clone(),
                created_at: NOW.to_owned(),
            })
            .expect("reference evidence");

        let mut brief_value = json!({
            "schema_version": "WeaponryKnifeProductionBrief@1",
            "brief_id": BRIEF,
            "project_id": PROJECT,
            "parent_brief_id": null,
            "parent_brief_sha256": null,
            "freeze_policy": "initial-intake-no-parent@1",
            "authorization": {"source_reference_sha256": reference_object.record.sha256},
            "reference_coverage": {"source_reference_sha256": reference_object.record.sha256},
            "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
            "canonical_sha256": ""
        });
        brief_value["canonical_sha256"] = Value::String(canonical_json_hash(&brief_value));
        let brief_bytes = canonical_json_bytes(&brief_value).expect("brief bytes");
        let brief_object = store
            .put_object(
                &brief_bytes,
                None,
                KNIFE_PASS_STATE_JSON_MIME,
                "weaponry-knife-production-brief",
                NOW,
            )
            .expect("brief object");
        let brief_sha256 = brief_value["canonical_sha256"]
            .as_str()
            .expect("brief semantic")
            .to_owned();
        let brief_record = WeaponryKnifeProductionBriefStoreRecord {
            schema_version: "WeaponryKnifeProductionBriefStoreRecord@1".to_owned(),
            project_id: PROJECT.to_owned(),
            brief_id: BRIEF.to_owned(),
            brief_object_sha256: brief_object.record.sha256.clone(),
            brief_canonical_sha256: brief_sha256.clone(),
            reference_id: REFERENCE.to_owned(),
            reference_object_sha256: reference_object.record.sha256.clone(),
            reference_evidence_sha256: reference_evidence_sha256.clone(),
            parent_brief_id: None,
            parent_brief_sha256: None,
            freeze_policy: "initial-intake-no-parent@1".to_owned(),
            source_reference_hashes: vec![reference_object.record.sha256.clone()],
            status: "eligible".to_owned(),
            conflict_freeze_state: "resolved".to_owned(),
            idempotency_key: "knife-pass-state-brief-key".to_owned(),
            created_at: NOW.to_owned(),
        };
        {
            let connection = store.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO weaponry_knife_production_brief_records (schema_version, project_id, brief_id, brief_object_sha256, brief_canonical_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, parent_brief_id, parent_brief_sha256, freeze_policy, source_reference_hashes_json, status, conflict_freeze_state, idempotency_key, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    params![
                        brief_record.schema_version,
                        brief_record.project_id,
                        brief_record.brief_id,
                        brief_record.brief_object_sha256,
                        brief_record.brief_canonical_sha256,
                        brief_record.reference_id,
                        brief_record.reference_object_sha256,
                        brief_record.reference_evidence_sha256,
                        brief_record.freeze_policy,
                        serde_json::to_string(&brief_record.source_reference_hashes).expect("refs"),
                        brief_record.status,
                        brief_record.conflict_freeze_state,
                        brief_record.idempotency_key,
                        brief_record.created_at,
                        serde_json::to_string(&brief_record).expect("brief row"),
                    ],
                )
                .expect("brief row");
        }

        let (quality_contract_object, quality_contract_sha256, quality_contract_value) =
            put_canonical_json(
                store,
                json!({
                    "schema_version": "KnifeQualityContract@1",
                    "contract_id": QUALITY_CONTRACT,
                    "stage_order": ["camera-lock", "silhouette-blockout"],
                    "threshold_status": "CALIBRATION_PENDING"
                }),
                "knife-quality-contract",
            );
        let (intake_object, intake_sha256, intake_value) = put_canonical_json(
            store,
            json!({"schema_version":"KnifeIntakeManifest@1","manifest_id":"knife-pass-state-intake"}),
            "knife-intake-manifest",
        );
        let (detail_object, detail_sha256, detail_value) = put_canonical_json(
            store,
            json!({"schema_version":"KnifeDetailInventory@1","inventory_id":"knife-pass-state-details"}),
            "knife-detail-inventory",
        );
        let mut intent_value = json!({
            "schema_version": "KnifeReferenceIntentBundle@1",
            "intent_bundle_id": INTENT,
            "project_id": PROJECT,
            "brief_binding": {
                "brief_schema_version": "WeaponryKnifeProductionBrief@1",
                "brief_id": BRIEF,
                "brief_sha256": brief_sha256,
                "brief_object_sha256": brief_object.record.sha256,
                "authoring_eligibility": "ELIGIBLE",
                "authorization_binding_status": "runtime-bound"
            },
            "reference_binding": {
                "reference_id": REFERENCE,
                "reference_object_sha256": reference_object.record.sha256,
                "reference_evidence_sha256": reference_evidence_sha256,
                "binding_status": "runtime-bound"
            },
            "route": "reference-projection",
            "exactness": "image-only",
            "intake_manifest": intake_value,
            "detail_inventory": detail_value,
            "quality_contract": quality_contract_value,
            "unknowns": [],
            "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
            "canonical_sha256": ""
        });
        intent_value["canonical_sha256"] = Value::String(canonical_json_hash(&intent_value));
        let intent_bytes = canonical_json_bytes(&intent_value).expect("intent bytes");
        let intent_object = store
            .put_object(
                &intent_bytes,
                None,
                KNIFE_PASS_STATE_JSON_MIME,
                "knife-reference-intent-bundle",
                NOW,
            )
            .expect("intent object");
        let intent_sha256 = intent_value["canonical_sha256"]
            .as_str()
            .expect("intent semantic")
            .to_owned();
        let intent_record = KnifeReferenceIntentBundleStoreRecord {
            schema_version: "KnifeReferenceIntentBundleStoreRecord@1".to_owned(),
            intent_bundle_id: INTENT.to_owned(),
            project_id: PROJECT.to_owned(),
            brief_id: BRIEF.to_owned(),
            brief_sha256: brief_sha256.clone(),
            brief_object_sha256: brief_object.record.sha256.clone(),
            reference_id: REFERENCE.to_owned(),
            reference_object_sha256: reference_object.record.sha256.clone(),
            reference_evidence_sha256: reference_evidence_sha256.clone(),
            intake_manifest_sha256: intake_sha256,
            intake_manifest_object_sha256: intake_object.sha256.clone(),
            detail_inventory_sha256: detail_sha256,
            detail_inventory_object_sha256: detail_object.sha256.clone(),
            quality_contract_sha256: quality_contract_sha256.clone(),
            quality_contract_object_sha256: quality_contract_object.sha256.clone(),
            intent_bundle_sha256: intent_sha256.clone(),
            intent_bundle_object_sha256: intent_object.record.sha256.clone(),
            idempotency_key: "knife-pass-state-intent-key".to_owned(),
            created_at: NOW.to_owned(),
        };
        {
            let connection = store.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO knife_reference_intent_bundle_records (schema_version, intent_bundle_id, project_id, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, intake_manifest_sha256, intake_manifest_object_sha256, detail_inventory_sha256, detail_inventory_object_sha256, quality_contract_sha256, quality_contract_object_sha256, intent_bundle_sha256, intent_bundle_object_sha256, idempotency_key, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
                    params![
                        intent_record.schema_version,
                        intent_record.intent_bundle_id,
                        intent_record.project_id,
                        intent_record.brief_id,
                        intent_record.brief_sha256,
                        intent_record.brief_object_sha256,
                        intent_record.reference_id,
                        intent_record.reference_object_sha256,
                        intent_record.reference_evidence_sha256,
                        intent_record.intake_manifest_sha256,
                        intent_record.intake_manifest_object_sha256,
                        intent_record.detail_inventory_sha256,
                        intent_record.detail_inventory_object_sha256,
                        intent_record.quality_contract_sha256,
                        intent_record.quality_contract_object_sha256,
                        intent_record.intent_bundle_sha256,
                        intent_record.intent_bundle_object_sha256,
                        intent_record.idempotency_key,
                        intent_record.created_at,
                        serde_json::to_string(&intent_record).expect("intent row"),
                    ],
                )
                .expect("intent row");
        }

        let operator_catalog_sha256 = h('o');
        let readback_config_sha256 = h('r');
        let source_parameters = json!({
            "primitive": "box",
            "size_m": [1.0, 0.1, 0.1],
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0]
        });
        let source_part_output = json!({
            "part_id": PART,
            "input_node_ids": [SOURCE_NODE],
            "material_zone_id": ZONE,
            "solid": true
        });
        let source_program_value = json!({
            "schema_version": "GeometryProgram@2",
            "project_id": PROJECT,
            "representation_plan_sha256": h('z'),
            "operator_catalog_sha256": operator_catalog_sha256,
            "units": {"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets": {"max_nodes":4,"max_triangles":250000,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes": [{"node_id":SOURCE_NODE,"operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":source_parameters}],
            "part_outputs": [source_part_output]
        });
        let (source_program_object, source_program_sha256) =
            put_draft_json(store, source_program_value.clone(), "geometry-program-v2");
        let source_artifact = store
            .put_object(
                b"source-geometry-glb",
                None,
                "model/gltf-binary",
                "geometry-glb",
                NOW,
            )
            .expect("source artifact");
        let source_readback_value = {
            let mut value = json!({
                "schema_version": "ArtifactReadback@2",
                "artifact_id": "knife-pass-state-source-artifact",
                "candidate_id": SOURCE_CANDIDATE,
                "object_sha256": source_artifact.record.sha256,
                "mime": "model/gltf-binary",
                "size_bytes": source_artifact.record.size_bytes,
                "program_sha256": source_program_sha256,
                "operator_catalog_sha256": operator_catalog_sha256,
                "readback_config_sha256": readback_config_sha256,
                "hard_gate_passed": true,
                "canonical_sha256": ""
            });
            value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
            value
        };
        let (source_readback_object, source_readback_sha256, _) = put_canonical_json(
            store,
            source_readback_value,
            "geometry-artifact-readback-v2",
        );
        let source_quality_value = {
            let mut value = json!({
                "schema_version": "GeometryQualityReport@2",
                "quality_report_id": SOURCE_QUALITY,
                "candidate_id": SOURCE_CANDIDATE,
                "artifact_sha256": source_artifact.record.sha256,
                "program_sha256": source_program_sha256,
                "hard_gate_passed": true,
                "status": "structural_only",
                "canonical_sha256": ""
            });
            value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
            value
        };
        let (source_quality_object, _source_quality_sha256, _) =
            put_canonical_json(store, source_quality_value, "geometry-quality-report");
        let source_state_sha256 = h('s');
        insert_candidate(
            store,
            SOURCE_CANDIDATE,
            &source_state_sha256,
            &source_artifact.record.sha256,
            SOURCE_QUALITY,
            true,
        );
        insert_geometry_evidence(
            store,
            SOURCE_CANDIDATE,
            &reference_object.record.sha256,
            &source_program_sha256,
            &source_program_object.sha256,
            &source_artifact.record.sha256,
            &source_readback_object.sha256,
            &source_quality_object.sha256,
            SOURCE_QUALITY,
            &operator_catalog_sha256,
            &readback_config_sha256,
        );

        let mut embedded_source_binding = json!({
            "schema_version": "AuthoringMeshV2SourceBinding@1",
            "project_id": PROJECT,
            "candidate_id": SOURCE_CANDIDATE,
            "candidate_state_sha256": source_state_sha256,
            "artifact_id": "knife-pass-state-source-artifact",
            "artifact_sha256": source_artifact.record.sha256,
            "artifact_readback_sha256": source_readback_sha256,
            "geometry_program_sha256": source_program_sha256,
            "source_node_id": SOURCE_NODE,
            "part_id": PART,
            "material_zone_id": ZONE,
            "solid": true,
            "source_operator_id": "forgecad.geometry.primitive@2",
            "source_parameters_sha256": canonical_json_hash(&source_parameters),
            "part_output_sha256": canonical_json_hash(&source_part_output),
            "position_m": [0.0, 0.0, 0.0],
            "rotation_rad": [0.0, 0.0, 0.0],
            "canonical_sha256": ""
        });
        embedded_source_binding["canonical_sha256"] =
            Value::String(canonical_json_hash(&embedded_source_binding));
        let (revision_payload, revision_sha256) = revision_value(embedded_source_binding);
        let revision_bytes = canonical_json_bytes(&revision_payload).expect("revision bytes");
        let revision_object = store
            .put_object(
                &revision_bytes,
                None,
                KNIFE_PASS_STATE_JSON_MIME,
                AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
                NOW,
            )
            .expect("revision object");
        let authoring_mesh_identity_sha256 = canonical_json_hash(&json!({
            "schema_version": "AuthoringMeshSourceIdentity@1",
            "mesh_id": MESH,
            "lineage_id": LINEAGE,
            "revision_id": REVISION,
            "revision_index": 0,
            "revision_sha256": revision_sha256
        }));
        {
            let connection = store.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO authoring_mesh_v2_durable_records (schema_version, project_id, mesh_id, lineage_id, revision_id, parent_revision_ids_json, revision_index, revision_object_sha256, revision_sha256, operation_id, operation_kind, operation_lineage_sha256, request_input_sha256, idempotency_key, materialization_status, canonical_sha256, created_at) VALUES ('AuthoringMeshV2DurableRecord@1', ?1, ?2, ?3, ?4, '[]', 0, ?5, ?6, NULL, NULL, NULL, ?7, ?8, 'runtime-owned-store-authoring-mesh-v2-durable-record@1', ?9, ?10)",
                    params![
                        PROJECT,
                        MESH,
                        LINEAGE,
                        REVISION,
                        revision_object.record.sha256,
                        revision_sha256,
                        h('q'),
                        "knife-pass-state-revision-key",
                        h('w'),
                        NOW,
                    ],
                )
                .expect("revision row");
        }

        let mut source_record = KnifeSourceBindingStoreRecord {
            schema_version: "KnifeSourceBindingStoreRecord@1".to_owned(),
            source_binding_id: SOURCE_BINDING.to_owned(),
            project_id: PROJECT.to_owned(),
            binding_status: KNIFE_SOURCE_BINDING_BINDING_STATUS.to_owned(),
            authoring_eligibility: KNIFE_SOURCE_BINDING_AUTHORING_ELIGIBILITY.to_owned(),
            intent_bundle_id: INTENT.to_owned(),
            intent_bundle_sha256: intent_sha256.clone(),
            intent_bundle_object_sha256: intent_object.record.sha256.clone(),
            brief_id: BRIEF.to_owned(),
            brief_sha256: brief_sha256.clone(),
            brief_object_sha256: brief_object.record.sha256.clone(),
            reference_id: REFERENCE.to_owned(),
            reference_object_sha256: reference_object.record.sha256.clone(),
            reference_evidence_sha256: reference_evidence_sha256.clone(),
            quality_contract_id: QUALITY_CONTRACT.to_owned(),
            quality_contract_sha256: quality_contract_sha256.clone(),
            quality_contract_object_sha256: quality_contract_object.sha256.clone(),
            source_candidate_id: SOURCE_CANDIDATE.to_owned(),
            source_candidate_state_sha256: source_state_sha256.clone(),
            authoring_mesh_id: MESH.to_owned(),
            authoring_mesh_lineage_id: LINEAGE.to_owned(),
            authoring_mesh_revision_id: REVISION.to_owned(),
            authoring_mesh_revision_index: 0,
            authoring_mesh_revision_sha256: revision_sha256.clone(),
            authoring_mesh_revision_object_sha256: revision_object.record.sha256.clone(),
            authoring_mesh_identity_sha256: authoring_mesh_identity_sha256.clone(),
            downstream_binding_requirements: KnifeSourceBindingDownstreamBindingRequirements {
                curve_modifier_graph: KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY.to_owned(),
                curve_evaluated_mesh: KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY.to_owned(),
                high: KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY.to_owned(),
                render: KNIFE_SOURCE_BINDING_DOWNSTREAM_POLICY.to_owned(),
            },
            high_mesh_created: false,
            high_stage_unlocked: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            quality_status: "source_binding_only".to_owned(),
            visual_status: "NOT_RUN".to_owned(),
            human_status: "NOT_RUN".to_owned(),
            engine_status: "NOT_RUN".to_owned(),
            binding_policy: KNIFE_SOURCE_BINDING_POLICY.to_owned(),
            canonicalization_policy: KNIFE_SOURCE_BINDING_CANONICALIZATION_POLICY.to_owned(),
            source_binding_sha256: h('x'),
            source_binding_object_sha256: h('y'),
            idempotency_key: "knife-pass-state-source-key".to_owned(),
            created_at: NOW.to_owned(),
        };
        let mut source_payload = source_public_payload(&source_record);
        source_payload["canonical_sha256"] = Value::String(String::new());
        source_record.source_binding_sha256 = canonical_json_hash(&source_payload);
        source_payload = source_public_payload(&source_record);
        let source_payload_bytes = canonical_json_bytes(&source_payload).expect("source bytes");
        let source_object = store
            .put_object(
                &source_payload_bytes,
                None,
                KNIFE_PASS_STATE_JSON_MIME,
                crate::KNIFE_SOURCE_BINDING_OBJECT_KIND,
                NOW,
            )
            .expect("source binding object");
        source_record.source_binding_object_sha256 = source_object.record.sha256.clone();
        {
            let connection = store.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO knife_source_binding_records (schema_version, source_binding_id, project_id, binding_status, authoring_eligibility, intent_bundle_id, intent_bundle_sha256, intent_bundle_object_sha256, brief_id, brief_sha256, brief_object_sha256, reference_id, reference_object_sha256, reference_evidence_sha256, quality_contract_id, quality_contract_sha256, quality_contract_object_sha256, source_candidate_id, source_candidate_state_sha256, authoring_mesh_id, authoring_mesh_lineage_id, authoring_mesh_revision_id, authoring_mesh_revision_index, authoring_mesh_revision_sha256, authoring_mesh_revision_object_sha256, authoring_mesh_identity_sha256, downstream_binding_requirements_json, high_mesh_created, high_stage_unlocked, production_stage_advanced, candidate_confirmed, version_created, export_performed, quality_status, visual_status, human_status, engine_status, binding_policy, canonicalization_policy, source_binding_sha256, source_binding_object_sha256, idempotency_key, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44)",
                    params![
                        source_record.schema_version,
                        source_record.source_binding_id,
                        source_record.project_id,
                        source_record.binding_status,
                        source_record.authoring_eligibility,
                        source_record.intent_bundle_id,
                        source_record.intent_bundle_sha256,
                        source_record.intent_bundle_object_sha256,
                        source_record.brief_id,
                        source_record.brief_sha256,
                        source_record.brief_object_sha256,
                        source_record.reference_id,
                        source_record.reference_object_sha256,
                        source_record.reference_evidence_sha256,
                        source_record.quality_contract_id,
                        source_record.quality_contract_sha256,
                        source_record.quality_contract_object_sha256,
                        source_record.source_candidate_id,
                        source_record.source_candidate_state_sha256,
                        source_record.authoring_mesh_id,
                        source_record.authoring_mesh_lineage_id,
                        source_record.authoring_mesh_revision_id,
                        0_i64,
                        source_record.authoring_mesh_revision_sha256,
                        source_record.authoring_mesh_revision_object_sha256,
                        source_record.authoring_mesh_identity_sha256,
                        serde_json::to_string(&source_record.downstream_binding_requirements).expect("requirements"),
                        0_i64,
                        0_i64,
                        0_i64,
                        0_i64,
                        0_i64,
                        0_i64,
                        source_record.quality_status,
                        source_record.visual_status,
                        source_record.human_status,
                        source_record.engine_status,
                        source_record.binding_policy,
                        source_record.canonicalization_policy,
                        source_record.source_binding_sha256,
                        source_record.source_binding_object_sha256,
                        source_record.idempotency_key,
                        source_record.created_at,
                        serde_json::to_string(&source_record).expect("source row"),
                    ],
                )
                .expect("source binding row");
        }

        let mut fixed_view = json!({
            "view_id": VIEW,
            "view_kind": "front",
            "comparison_role": "primary-reference",
            "reference_required": true,
            "camera_id": CAMERA,
            "camera_sha256": h('c'),
            "reference_view_id": VIEW,
            "reference_view_sha256": h('m'),
            "fixed_view_policy": "single-runtime-bound-primary-reference-view@1"
        });
        let camera_object_value = json!({
            "schema_version": "CameraCalibration@1",
            "camera_id": CAMERA,
            "camera_hash": fixed_view["camera_sha256"],
            "projection": "perspective",
            "position_m": [0.0, 0.0, 2.0],
            "target_m": [0.0, 0.0, 0.0],
            "up": [0.0, 1.0, 0.0],
            "fov_y_deg": 45.0
        });
        let camera_bytes = canonical_json_bytes(&camera_object_value).expect("camera bytes");
        let camera_object = store
            .put_object(
                &camera_bytes,
                None,
                KNIFE_PASS_STATE_JSON_MIME,
                "camera-calibration",
                NOW,
            )
            .expect("camera object");
        let mask_object = store
            .put_object(
                b"reference-silhouette-mask",
                None,
                "image/png",
                "reference-silhouette-mask-v1",
                NOW,
            )
            .expect("mask object");
        fixed_view["reference_view_sha256"] = Value::String(mask_object.record.sha256.clone());
        let camera_set_sha256 = canonical_json_hash(&json!({
            "schema_version": "KnifeCameraSet@1",
            "fixed_views": [fixed_view.clone()],
            "fixed_view_count": 1
        }));

        let mut skeleton_main: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/fixtures/knife-pass-state/positive/dragonfang-pass-state.json"
        )))
        .expect("positive pass fixture");
        let object = skeleton_main.as_object_mut().expect("main object");
        object.insert("pass_id".to_owned(), Value::String(PASS.to_owned()));
        object.insert("project_id".to_owned(), Value::String(PROJECT.to_owned()));
        object.insert(
            "stage".to_owned(),
            Value::String("silhouette-blockout".to_owned()),
        );
        object.insert(
            "source_binding_id".to_owned(),
            Value::String(SOURCE_BINDING.to_owned()),
        );
        object.insert(
            "source_binding_sha256".to_owned(),
            Value::String(source_record.source_binding_sha256.clone()),
        );
        object.insert(
            "source_binding_object_sha256".to_owned(),
            Value::String(source_record.source_binding_object_sha256.clone()),
        );
        object.insert(
            "intent_bundle_id".to_owned(),
            Value::String(INTENT.to_owned()),
        );
        object.insert(
            "intent_bundle_sha256".to_owned(),
            Value::String(intent_sha256.clone()),
        );
        object.insert(
            "intent_bundle_object_sha256".to_owned(),
            Value::String(intent_object.record.sha256.clone()),
        );
        object.insert("brief_id".to_owned(), Value::String(BRIEF.to_owned()));
        object.insert(
            "brief_sha256".to_owned(),
            Value::String(brief_sha256.clone()),
        );
        object.insert(
            "brief_object_sha256".to_owned(),
            Value::String(brief_object.record.sha256.clone()),
        );
        object.insert(
            "reference_id".to_owned(),
            Value::String(REFERENCE.to_owned()),
        );
        object.insert(
            "reference_object_sha256".to_owned(),
            Value::String(reference_object.record.sha256.clone()),
        );
        object.insert(
            "reference_evidence_sha256".to_owned(),
            Value::String(reference_evidence_sha256.clone()),
        );
        object.insert(
            "source_candidate_id".to_owned(),
            Value::String(SOURCE_CANDIDATE.to_owned()),
        );
        object.insert(
            "source_candidate_state_sha256".to_owned(),
            Value::String(source_state_sha256.clone()),
        );
        object.insert(
            "authoring_mesh_id".to_owned(),
            Value::String(MESH.to_owned()),
        );
        object.insert(
            "authoring_mesh_lineage_id".to_owned(),
            Value::String(LINEAGE.to_owned()),
        );
        object.insert(
            "authoring_mesh_revision_id".to_owned(),
            Value::String(REVISION.to_owned()),
        );
        object.insert(
            "authoring_mesh_revision_sha256".to_owned(),
            Value::String(revision_sha256.clone()),
        );
        object.insert(
            "authoring_mesh_revision_object_sha256".to_owned(),
            Value::String(revision_object.record.sha256.clone()),
        );
        object.insert(
            "authoring_mesh_identity_sha256".to_owned(),
            Value::String(authoring_mesh_identity_sha256),
        );
        object.insert(
            "authoring_mesh_sha256".to_owned(),
            Value::String(revision_sha256.clone()),
        );
        object.insert("fixed_view".to_owned(), fixed_view.clone());
        object.insert(
            "camera_set_sha256".to_owned(),
            Value::String(camera_set_sha256.clone()),
        );
        object.insert(
            "baseline_candidate_id".to_owned(),
            Value::String(SOURCE_CANDIDATE.to_owned()),
        );
        object.insert(
            "baseline_candidate_state_sha256".to_owned(),
            Value::String(source_state_sha256.clone()),
        );
        object.insert(
            "attempt_candidate_id".to_owned(),
            Value::String(ATTEMPT_CANDIDATE.to_owned()),
        );
        object.insert(
            "attempt_candidate_state_sha256".to_owned(),
            Value::String(h('t')),
        );
        let _ = object;

        // Build two real candidate artifacts/readbacks before deriving the
        // source-bound materialization plan.  Both candidates may share the
        // same AMV2 revision/program; their artifact objects remain distinct
        // identities so correction replay cannot collapse them by state hash.
        let revision: AuthoringMeshRevision =
            serde_json::from_value(revision_payload.clone()).expect("revision contract");
        // The lineage resolver only accepts durable/reachable upstream
        // objects.  This fixture is intentionally assembled in dependency
        // order, so make the already-created source objects visible before
        // deriving the materializer plan.
        mark_all_reachable(store);
        let mut temporary_main = skeleton_main.clone();
        temporary_main["canonical_sha256"] = Value::String(String::new());
        temporary_main["canonical_sha256"] = Value::String(canonical_json_hash(&temporary_main));
        let temporary_record =
            record_from_main_value(temporary_main, h('v'), "knife-pass-state-temporary-key")
                .expect("temporary pass record");
        let plan_sha256 = {
            let mut connection = store.connection.lock().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            let plan = expected_representation_plan_sha256(
                &transaction,
                store.cas(),
                &temporary_record,
                &revision,
            )
            .expect("source-bound representation plan");
            transaction.commit().expect("plan transaction");
            plan
        };
        let object = skeleton_main.as_object_mut().expect("main object");
        object.insert(
            "baseline_representation_plan_sha256".to_owned(),
            Value::String(h('z')),
        );
        object.insert(
            "attempt_representation_plan_sha256".to_owned(),
            Value::String(plan_sha256.clone()),
        );

        let parameters = authoring_mesh_geometry_parameters(&revision, [0.0; 3], [0.0; 3])
            .expect("projection parameters");
        let projection_sha256 = canonical_json_hash(&json!({
            "schema_version":"AuthoringMeshV2GeometryProjection@1",
            "revision_id":REVISION,
            "revision_sha256":revision_sha256,
            "operator_id":"forgecad.geometry.authoring-mesh@1",
            "parameters":parameters,
        }));
        let replacement_identity = json!({
            "schema_version":"AuthoringMeshV2CandidateReplacementIdentity@1",
            "project_id":PROJECT,
            "mesh_id":MESH,
            "lineage_id":LINEAGE,
            "materialization_mode":"source_binding_part_replacement",
            "revision_id":REVISION,
            "revision_sha256":revision_sha256,
            "revision_object_sha256":revision_object.record.sha256,
            "projection_sha256":projection_sha256,
            "source_binding_id":SOURCE_BINDING,
            "source_binding_sha256":source_record.source_binding_sha256,
            "source_node_id":SOURCE_NODE,
            "source_part_id":PART,
        });
        let replacement_node_id = format!(
            "authoring-mesh-v2-{}",
            &canonical_json_hash(&replacement_identity)[..32]
        );
        let materialized_program = json!({
            "schema_version": "GeometryProgram@2",
            "project_id": PROJECT,
            "representation_plan_sha256": plan_sha256,
            "operator_catalog_sha256": operator_catalog_sha256,
            "units": {"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets": {"max_nodes":4,"max_triangles":250000,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes": [{"node_id":replacement_node_id,"operator_id":"forgecad.geometry.authoring-mesh@1","inputs":[],"parameters":parameters}],
            "part_outputs": [{"part_id":PART,"input_node_ids":[replacement_node_id],"material_zone_id":ZONE,"solid":true}]
        });
        let (materialized_program_object, materialized_program_sha256) =
            put_draft_json(store, materialized_program, "geometry-program-v2");

        let baseline_artifact = store
            .put_object(
                b"baseline-geometry-glb",
                None,
                "model/gltf-binary",
                "geometry-glb",
                NOW,
            )
            .expect("baseline artifact");
        let attempt_artifact = store
            .put_object(
                b"attempt-geometry-glb",
                None,
                "model/gltf-binary",
                "geometry-glb",
                NOW,
            )
            .expect("attempt artifact");
        let candidate_artifacts = [
            (
                BASELINE_CANDIDATE,
                h('b'),
                BASELINE_QUALITY,
                &baseline_artifact.record,
            ),
            (
                ATTEMPT_CANDIDATE,
                h('t'),
                ATTEMPT_QUALITY,
                &attempt_artifact.record,
            ),
        ];
        let mut candidate_roots = Vec::new();
        for (candidate_id, state_sha256, quality_id, artifact) in candidate_artifacts {
            let mut readback = json!({
                "schema_version":"ArtifactReadback@2",
                "artifact_id":format!("{candidate_id}-artifact"),
                "candidate_id":candidate_id,
                "object_sha256":artifact.sha256,
                "mime":"model/gltf-binary",
                "size_bytes":artifact.size_bytes,
                "program_sha256":materialized_program_sha256,
                "operator_catalog_sha256":operator_catalog_sha256,
                "readback_config_sha256":readback_config_sha256,
                "hard_gate_passed":true,
                "canonical_sha256":""
            });
            readback["canonical_sha256"] = Value::String(canonical_json_hash(&readback));
            let (readback_object, _readback_sha256, _) =
                put_canonical_json(store, readback, "geometry-artifact-readback-v2");
            let mut quality = json!({
                "schema_version":"GeometryQualityReport@2",
                "quality_report_id":quality_id,
                "candidate_id":candidate_id,
                "artifact_sha256":artifact.sha256,
                "program_sha256":materialized_program_sha256,
                "hard_gate_passed":true,
                "status":"structural_only",
                "canonical_sha256":""
            });
            quality["canonical_sha256"] = Value::String(canonical_json_hash(&quality));
            let (quality_object, _quality_sha256, _) =
                put_canonical_json(store, quality, "geometry-quality-report");
            insert_candidate(
                store,
                candidate_id,
                &state_sha256,
                &artifact.sha256,
                quality_id,
                true,
            );
            insert_geometry_evidence(
                store,
                candidate_id,
                &reference_object.record.sha256,
                &materialized_program_sha256,
                &materialized_program_object.sha256,
                &artifact.sha256,
                &readback_object.sha256,
                &quality_object.sha256,
                quality_id,
                &operator_catalog_sha256,
                &readback_config_sha256,
            );
            candidate_roots.push((
                candidate_id,
                artifact.clone(),
                readback_object,
                quality_object,
            ));
        }
        let attempt_root = candidate_roots
            .iter()
            .find(|(candidate_id, _, _, _)| *candidate_id == ATTEMPT_CANDIDATE)
            .expect("attempt roots");
        object.insert(
            "baseline_artifact_sha256".to_owned(),
            Value::String(source_artifact.record.sha256.clone()),
        );
        object.insert(
            "baseline_geometry_program_sha256".to_owned(),
            Value::String(source_program_sha256.clone()),
        );
        object.insert(
            "baseline_geometry_program_object_sha256".to_owned(),
            Value::String(source_program_object.sha256.clone()),
        );
        object.insert(
            "baseline_artifact_readback_object_sha256".to_owned(),
            Value::String(source_readback_object.sha256.clone()),
        );
        object.insert(
            "attempt_artifact_sha256".to_owned(),
            Value::String(attempt_root.1.sha256.clone()),
        );
        object.insert(
            "attempt_geometry_program_sha256".to_owned(),
            Value::String(materialized_program_sha256.clone()),
        );
        object.insert(
            "attempt_geometry_program_object_sha256".to_owned(),
            Value::String(materialized_program_object.sha256.clone()),
        );
        object.insert(
            "attempt_artifact_readback_object_sha256".to_owned(),
            Value::String(attempt_root.2.sha256.clone()),
        );

        let render_passes = [
            "beauty",
            "silhouette",
            "depth",
            "normal",
            "ao",
            "part-id",
            "material-id",
            "wireframe",
            "uv-stretch",
        ];
        let mut pass_artifacts = serde_json::Map::new();
        for pass in render_passes {
            let png = store
                .put_object(
                    format!("{pass}-png").as_bytes(),
                    None,
                    "image/png",
                    &format!("render-pass-{pass}"),
                    NOW,
                )
                .expect("render pass");
            pass_artifacts.insert(
                pass.to_owned(),
                json!({
                    "sha256":png.record.sha256,
                    "mime":"image/png",
                    "size_bytes":png.record.size_bytes,
                    "width":512,
                    "height":512,
                    "channels":"rgba8",
                    "color_space":if pass == "beauty" {"srgb"} else {"data"}
                }),
            );
        }
        let mut render_value = json!({
            "schema_version":"RenderSet@2",
            "render_set_id":RENDER_SET,
            "candidate_id":ATTEMPT_CANDIDATE,
            "artifact_sha256":attempt_root.1.sha256,
            "program_sha256":materialized_program_sha256,
            "reference_id":REFERENCE,
            "camera_hash":fixed_view["camera_sha256"],
            "camera_object_sha256":camera_object.record.sha256,
            "renderer_hash":h('d'),
            "render_profile":{"projection":"perspective","fixed_view":VIEW},
            "render_profile_sha256":h('e'),
            "aov_definition_sha256":h('f'),
            "color_pipeline_sha256":h('g'),
            "id_palette_definition_sha256":h('h'),
            "render_worker_build_cohort_sha256":h('i'),
            "render_worker_binding_status":"fixed-worker",
            "width":512,
            "height":512,
            "view_id":VIEW,
            "source_binding_sha256":source_record.source_binding_sha256,
            "camera_set_sha256":camera_set_sha256,
            "passes":render_passes,
            "pass_artifacts":Value::Object(pass_artifacts),
            "canonical_sha256":""
        });
        render_value["canonical_sha256"] = Value::String(canonical_json_hash(&render_value));
        let (render_object, render_sha256, _) =
            put_canonical_json(store, render_value, "render-set-v2");

        let mut comparison_value = json!({
            "schema_version":"ReferenceComparisonReport@1",
            "report_id":COMPARISON,
            "candidate_id":ATTEMPT_CANDIDATE,
            "artifact_sha256":attempt_root.1.sha256,
            "reference_id":REFERENCE,
            "reference_sha256":reference_object.record.sha256,
            "render_set_hash":render_object.sha256,
            "camera_hash":fixed_view["camera_sha256"],
            "view_id":VIEW,
            "mask":{"method":"silhouette-target","revision":"mask-v1","sha256":mask_object.record.sha256,"width":512,"height":512},
            "metrics":{"silhouette_iou":0.2,"boundary_f1_4px":0.2,"bbox_edge_error":0.2,"centroid_error":0.2,"landmark_coverage":0.2,"landmark_nme":0.2,"region_median_iou":0.2,"critical_region_min_iou":0.2},
            "status":"BLOCKED_REFERENCE_COVERAGE",
            "canonical_sha256":""
        });
        comparison_value["canonical_sha256"] =
            Value::String(canonical_json_hash(&comparison_value));
        let (comparison_object, comparison_sha256, _) =
            put_canonical_json(store, comparison_value, "reference-comparison-report");
        let mut quality_value = json!({
            "schema_version":"QualityReport@2",
            "quality_report_id":QUALITY,
            "candidate_id":ATTEMPT_CANDIDATE,
            "artifact_sha256":attempt_root.1.sha256,
            "program_sha256":materialized_program_sha256,
            "reference_id":REFERENCE,
            "reference_sha256":reference_object.record.sha256,
            "render_set_hash":render_object.sha256,
            "comparison_report_hash":comparison_object.sha256,
            "human_receipt_hash":Value::Null,
            "structural_status":"passed",
            "visual_status":"BLOCKED_REFERENCE_COVERAGE",
            "hard_gate_passed":false,
            "threshold_revision":"knife-threshold-v1",
            "threshold_policy_sha256":h('j'),
            "threshold_source":"fixture",
            "metric_gate_results":[],
            "limitations":["single bounded fixed view"],
            "view_id":VIEW,
            "canonical_sha256":""
        });
        quality_value["canonical_sha256"] = Value::String(canonical_json_hash(&quality_value));
        let (quality_object, quality_sha256, _) =
            put_canonical_json(store, quality_value, "quality-report-v2");
        {
            let connection = store.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO visual_evidence (candidate_id, project_id, reference_id, target_sha256, render_set_object_sha256, comparison_report_object_sha256, visual_review_object_sha256, quality_report_object_sha256, human_receipt_object_sha256, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, NULL, ?8, ?8)",
                    params![
                        ATTEMPT_CANDIDATE,
                        PROJECT,
                        REFERENCE,
                        reference_object.record.sha256,
                        render_object.sha256,
                        comparison_object.sha256,
                        quality_object.sha256,
                        NOW,
                    ],
                )
                .expect("visual evidence row");
        }
        let evidence_bundle_sha256 = canonical_json_hash(&json!({
            "schema_version": KNIFE_PASS_STATE_EVIDENCE_BUNDLE_SCHEMA_VERSION,
            "render_set_sha256": render_sha256,
            "reference_comparison_sha256": comparison_sha256,
            "quality_report_sha256": quality_sha256,
            "camera_set_sha256": camera_set_sha256,
        }));
        object.insert(
            "render_set_id".to_owned(),
            Value::String(RENDER_SET.to_owned()),
        );
        object.insert("render_set_sha256".to_owned(), Value::String(render_sha256));
        object.insert(
            "render_set_object_sha256".to_owned(),
            Value::String(render_object.sha256.clone()),
        );
        object.insert(
            "reference_comparison_id".to_owned(),
            Value::String(COMPARISON.to_owned()),
        );
        object.insert(
            "reference_comparison_sha256".to_owned(),
            Value::String(comparison_sha256),
        );
        object.insert(
            "reference_comparison_object_sha256".to_owned(),
            Value::String(comparison_object.sha256.clone()),
        );
        object.insert(
            "quality_report_id".to_owned(),
            Value::String(QUALITY.to_owned()),
        );
        object.insert(
            "quality_report_sha256".to_owned(),
            Value::String(quality_sha256),
        );
        object.insert(
            "quality_report_object_sha256".to_owned(),
            Value::String(quality_object.sha256.clone()),
        );
        object.insert(
            "evidence_bundle_sha256".to_owned(),
            Value::String(evidence_bundle_sha256),
        );
        object.insert(
            "hard_gate_status".to_owned(),
            Value::String("PASS_SOURCE_STRUCTURAL".to_owned()),
        );
        object.insert(
            "visual_gate_status".to_owned(),
            Value::String("BLOCKED_REFERENCE_COVERAGE".to_owned()),
        );
        object.insert(
            "quality_status".to_owned(),
            Value::String("BLOCKED_REFERENCE_COVERAGE".to_owned()),
        );
        object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        let _ = object;
        let canonical_sha256 = canonical_json_hash(&skeleton_main);
        let object = skeleton_main.as_object_mut().expect("main object");
        object.insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical_sha256),
        );
        let main_bytes = canonical_json_bytes(&skeleton_main).expect("main bytes");
        let pass_object = store
            .put_object(
                &main_bytes,
                None,
                KNIFE_PASS_STATE_JSON_MIME,
                KNIFE_PASS_STATE_OBJECT_KIND,
                NOW,
            )
            .expect("pass object");
        let record = record_from_main_value(
            skeleton_main,
            pass_object.record.sha256.clone(),
            "knife-pass-state-idempotency",
        )
        .expect("pass record");
        mark_all_reachable(store);
        PassFixture {
            commit: KnifePassStateCommit {
                record,
                cas: KnifePassStateCasBundle {
                    pass_state: pass_object.record,
                },
            },
            camera_object_sha256: camera_object.record.sha256,
            mask_object_sha256: mask_object.record.sha256,
            source_program_object_sha256: source_program_object.sha256,
            source_artifact_readback_object_sha256: source_readback_object.sha256,
            source_quality_object_sha256: source_quality_object.sha256,
            quality_contract_object_sha256: quality_contract_object.sha256,
        }
    }

    fn put_child_evidence_object<F>(
        store: &Store,
        parent_object_sha256: &str,
        kind: &str,
        edit: F,
    ) -> (CasObjectRecord, String, Value)
    where
        F: FnOnce(&mut Value),
    {
        let bytes = store
            .cas()
            .read_verified_bounded(parent_object_sha256, MAX_LINEAGE_JSON_BYTES)
            .expect("parent evidence bytes");
        let mut value: Value = serde_json::from_slice(&bytes).expect("parent evidence JSON");
        edit(&mut value);
        put_canonical_json(store, value, kind)
    }

    fn setup_child_fixture(store: &Store, parent: &PassFixture) -> KnifePassStateCommit {
        let parent_revision_bytes = store
            .cas()
            .read_verified_bounded(
                &parent.commit.record.authoring_mesh_revision_object_sha256,
                MAX_LINEAGE_JSON_BYTES,
            )
            .expect("parent revision bytes");
        let mut child_revision_value: Value =
            serde_json::from_slice(&parent_revision_bytes).expect("parent revision JSON");
        child_revision_value["revision_id"] = Value::String(CHILD_REVISION.to_owned());
        child_revision_value["parent_revision_ids"] = json!([REVISION]);
        child_revision_value["revision_index"] = Value::Number(1.into());
        child_revision_value["original"]["vertices"] = json!([{
            "vertex_id":"knife-pass-state-child-vertex",
            "position_m":[0.0, 0.0, 0.0]
        }]);
        child_revision_value["original"]["canonical_sha256"] = Value::String(String::new());
        child_revision_value["original"]["canonical_sha256"] =
            Value::String(canonical_json_hash(&child_revision_value["original"]));
        let mut operation = json!({
            "schema_version":"AuthoringMeshTopologyOperation@2",
            "operation_id":"knife-pass-state-child-operation",
            "kind":"move_vertices",
            "parent_revision_id":REVISION,
            "operation_lineage_sha256":h('o'),
            "source_elements":[{"kind":"vertex","id":"knife-pass-state-child-vertex"}],
            "generated_elements":[],
            "retired_elements":[],
            "tombstones":[],
            "locality_policy":"bounded-authoring-mesh-operation@1",
            "canonical_sha256":""
        });
        operation["canonical_sha256"] = Value::String(canonical_json_hash(&operation));
        child_revision_value["operation"] = operation;
        child_revision_value["canonical_sha256"] = Value::String(String::new());
        let child_revision_sha256 = canonical_json_hash(&child_revision_value);
        child_revision_value["canonical_sha256"] = Value::String(child_revision_sha256.clone());
        let child_revision_bytes =
            canonical_json_bytes(&child_revision_value).expect("child revision bytes");
        let child_revision_object = store
            .put_object(
                &child_revision_bytes,
                None,
                KNIFE_PASS_STATE_JSON_MIME,
                AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
                NOW,
            )
            .expect("child revision object");
        {
            let connection = store.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO authoring_mesh_v2_durable_records (schema_version, project_id, mesh_id, lineage_id, revision_id, parent_revision_ids_json, revision_index, revision_object_sha256, revision_sha256, operation_id, operation_kind, operation_lineage_sha256, request_input_sha256, idempotency_key, materialization_status, canonical_sha256, created_at) VALUES ('AuthoringMeshV2DurableRecord@1', ?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, 'move_vertices', ?9, ?10, ?11, 'runtime-owned-store-authoring-mesh-v2-durable-record@1', ?12, ?13)",
                    params![
                        PROJECT,
                        MESH,
                        LINEAGE,
                        CHILD_REVISION,
                        serde_json::to_string(&vec![REVISION]).expect("child parents"),
                        child_revision_object.record.sha256,
                        child_revision_sha256,
                        "knife-pass-state-child-operation",
                        h('o'),
                        h('l'),
                        "knife-pass-state-child-revision-key",
                        h('w'),
                        NOW,
                    ],
                )
                .expect("child revision row");
        }
        mark_all_reachable(store);
        let child_revision: AuthoringMeshRevision =
            serde_json::from_value(child_revision_value).expect("child revision contract");
        let mut planning_record = parent.commit.record.clone();
        planning_record.authoring_mesh_revision_id = CHILD_REVISION.to_owned();
        planning_record.authoring_mesh_revision_index = 1;
        planning_record.authoring_mesh_revision_sha256 = child_revision_sha256.clone();
        planning_record.authoring_mesh_revision_object_sha256 =
            child_revision_object.record.sha256.clone();
        planning_record.authoring_mesh_sha256 = child_revision_sha256.clone();
        let child_plan_sha256 = {
            let mut connection = store.connection.lock().expect("connection");
            let transaction = connection.transaction().expect("child plan transaction");
            let plan = expected_representation_plan_sha256(
                &transaction,
                store.cas(),
                &planning_record,
                &child_revision,
            )
            .expect("child representation plan");
            transaction.commit().expect("child plan commit");
            plan
        };
        let binding = child_revision
            .source_binding
            .as_ref()
            .expect("child source binding");
        let parameters = authoring_mesh_geometry_parameters(
            &child_revision,
            binding.position_m,
            binding.rotation_rad,
        )
        .expect("child projection parameters");
        let projection_sha256 = canonical_json_hash(&json!({
            "schema_version":"AuthoringMeshV2GeometryProjection@1",
            "revision_id":CHILD_REVISION,
            "revision_sha256":child_revision_sha256,
            "operator_id":"forgecad.geometry.authoring-mesh@1",
            "parameters":parameters,
        }));
        let replacement_identity = json!({
            "schema_version":"AuthoringMeshV2CandidateReplacementIdentity@1",
            "project_id":PROJECT,
            "mesh_id":MESH,
            "lineage_id":LINEAGE,
            "materialization_mode":"source_binding_part_replacement",
            "revision_id":CHILD_REVISION,
            "revision_sha256":child_revision_sha256,
            "revision_object_sha256":child_revision_object.record.sha256,
            "projection_sha256":projection_sha256,
            "source_binding_id":SOURCE_BINDING,
            "source_binding_sha256":parent.commit.record.source_binding_sha256,
            "source_node_id":SOURCE_NODE,
            "source_part_id":PART,
        });
        let replacement_node_id = format!(
            "authoring-mesh-v2-{}",
            &canonical_json_hash(&replacement_identity)[..32]
        );
        let child_program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":PROJECT,
            "representation_plan_sha256":child_plan_sha256,
            "operator_catalog_sha256":h('o'),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":4,"max_triangles":250000,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{"node_id":replacement_node_id,"operator_id":"forgecad.geometry.authoring-mesh@1","inputs":[],"parameters":parameters}],
            "part_outputs":[{"part_id":PART,"input_node_ids":[replacement_node_id],"material_zone_id":ZONE,"solid":true}]
        });
        let (child_program_object, child_program_sha256) =
            put_draft_json(store, child_program, "geometry-program-v2");
        let child_artifact = store
            .put_object(
                b"child-geometry-glb",
                None,
                "model/gltf-binary",
                "geometry-glb",
                NOW,
            )
            .expect("child artifact");
        let mut child_readback = json!({
            "schema_version":"ArtifactReadback@2",
            "artifact_id":"knife-pass-state-child-artifact",
            "candidate_id":CHILD_ATTEMPT_CANDIDATE,
            "object_sha256":child_artifact.record.sha256,
            "mime":"model/gltf-binary",
            "size_bytes":child_artifact.record.size_bytes,
            "program_sha256":child_program_sha256,
            "operator_catalog_sha256":h('o'),
            "readback_config_sha256":h('r'),
            "hard_gate_passed":true,
            "canonical_sha256":""
        });
        child_readback["canonical_sha256"] = Value::String(canonical_json_hash(&child_readback));
        let (child_readback_object, _, _) =
            put_canonical_json(store, child_readback, "geometry-artifact-readback-v2");
        let mut child_quality = json!({
            "schema_version":"GeometryQualityReport@2",
            "quality_report_id":CHILD_ATTEMPT_QUALITY,
            "candidate_id":CHILD_ATTEMPT_CANDIDATE,
            "artifact_sha256":child_artifact.record.sha256,
            "program_sha256":child_program_sha256,
            "hard_gate_passed":true,
            "status":"structural_only",
            "canonical_sha256":""
        });
        child_quality["canonical_sha256"] = Value::String(canonical_json_hash(&child_quality));
        let (child_quality_object, _, _) =
            put_canonical_json(store, child_quality, "geometry-quality-report");
        let child_state_sha256 = h('u');
        insert_candidate(
            store,
            CHILD_ATTEMPT_CANDIDATE,
            &child_state_sha256,
            &child_artifact.record.sha256,
            CHILD_ATTEMPT_QUALITY,
            true,
        );
        insert_geometry_evidence(
            store,
            CHILD_ATTEMPT_CANDIDATE,
            &parent.commit.record.reference_object_sha256,
            &child_program_sha256,
            &child_program_object.sha256,
            &child_artifact.record.sha256,
            &child_readback_object.sha256,
            &child_quality_object.sha256,
            CHILD_ATTEMPT_QUALITY,
            &h('o'),
            &h('r'),
        );

        let (child_render_object, child_render_sha256, _) = put_child_evidence_object(
            store,
            &parent.commit.record.render_set_object_sha256,
            "render-set-v2",
            |render| {
                render["render_set_id"] = Value::String(CHILD_RENDER_SET.to_owned());
                render["candidate_id"] = Value::String(CHILD_ATTEMPT_CANDIDATE.to_owned());
                render["artifact_sha256"] = Value::String(child_artifact.record.sha256.clone());
                render["program_sha256"] = Value::String(child_program_sha256.clone());
            },
        );
        let (child_comparison_object, child_comparison_sha256, _) = put_child_evidence_object(
            store,
            &parent.commit.record.reference_comparison_object_sha256,
            "reference-comparison-report",
            |comparison| {
                comparison["report_id"] = Value::String(CHILD_COMPARISON.to_owned());
                comparison["candidate_id"] = Value::String(CHILD_ATTEMPT_CANDIDATE.to_owned());
                comparison["artifact_sha256"] = Value::String(child_artifact.record.sha256.clone());
                comparison["render_set_hash"] = Value::String(child_render_object.sha256.clone());
            },
        );
        let (child_quality_evidence_object, child_quality_evidence_sha256, _) =
            put_child_evidence_object(
                store,
                &parent.commit.record.quality_report_object_sha256,
                "quality-report-v2",
                |quality| {
                    quality["quality_report_id"] = Value::String(CHILD_QUALITY.to_owned());
                    quality["candidate_id"] = Value::String(CHILD_ATTEMPT_CANDIDATE.to_owned());
                    quality["artifact_sha256"] =
                        Value::String(child_artifact.record.sha256.clone());
                    quality["program_sha256"] = Value::String(child_program_sha256.clone());
                    quality["render_set_hash"] = Value::String(child_render_object.sha256.clone());
                    quality["comparison_report_hash"] =
                        Value::String(child_comparison_object.sha256.clone());
                },
            );
        {
            let connection = store.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO visual_evidence (candidate_id, project_id, reference_id, target_sha256, render_set_object_sha256, comparison_report_object_sha256, visual_review_object_sha256, quality_report_object_sha256, human_receipt_object_sha256, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, NULL, ?8, ?8)",
                    params![
                        CHILD_ATTEMPT_CANDIDATE,
                        PROJECT,
                        REFERENCE,
                        parent.commit.record.reference_object_sha256,
                        child_render_object.sha256,
                        child_comparison_object.sha256,
                        child_quality_evidence_object.sha256,
                        NOW,
                    ],
                )
                .expect("child visual evidence row");
        }
        let evidence_bundle_sha256 = canonical_json_hash(&json!({
            "schema_version":KNIFE_PASS_STATE_EVIDENCE_BUNDLE_SCHEMA_VERSION,
            "render_set_sha256":child_render_sha256,
            "reference_comparison_sha256":child_comparison_sha256,
            "quality_report_sha256":child_quality_evidence_sha256,
            "camera_set_sha256":parent.commit.record.camera_set_sha256,
        }));
        let mut child_main = main_value(&parent.commit.record).expect("parent Main");
        let object = child_main.as_object_mut().expect("child Main object");
        object.insert("pass_id".to_owned(), Value::String(CHILD_PASS.to_owned()));
        object.insert("parent_pass_id".to_owned(), Value::String(PASS.to_owned()));
        object.insert(
            "parent_pass_sha256".to_owned(),
            Value::String(parent.commit.record.canonical_sha256.clone()),
        );
        object.insert(
            "stage".to_owned(),
            Value::String("structural-form".to_owned()),
        );
        object.insert(
            "authoring_mesh_revision_id".to_owned(),
            Value::String(CHILD_REVISION.to_owned()),
        );
        object.insert(
            "authoring_mesh_revision_index".to_owned(),
            Value::Number(1.into()),
        );
        object.insert(
            "authoring_mesh_revision_sha256".to_owned(),
            Value::String(child_revision_sha256.clone()),
        );
        object.insert(
            "authoring_mesh_revision_object_sha256".to_owned(),
            Value::String(child_revision_object.record.sha256.clone()),
        );
        object.insert(
            "authoring_mesh_sha256".to_owned(),
            Value::String(child_revision_sha256),
        );
        for (field, value) in [
            (
                "baseline_candidate_id",
                parent.commit.record.attempt_candidate_id.clone(),
            ),
            (
                "baseline_candidate_state_sha256",
                parent.commit.record.attempt_candidate_state_sha256.clone(),
            ),
            (
                "baseline_artifact_sha256",
                parent.commit.record.attempt_artifact_sha256.clone(),
            ),
            (
                "baseline_geometry_program_sha256",
                parent.commit.record.attempt_geometry_program_sha256.clone(),
            ),
            (
                "baseline_geometry_program_object_sha256",
                parent
                    .commit
                    .record
                    .attempt_geometry_program_object_sha256
                    .clone(),
            ),
            (
                "baseline_artifact_readback_object_sha256",
                parent
                    .commit
                    .record
                    .attempt_artifact_readback_object_sha256
                    .clone(),
            ),
            (
                "baseline_representation_plan_sha256",
                parent
                    .commit
                    .record
                    .attempt_representation_plan_sha256
                    .clone(),
            ),
            ("attempt_candidate_id", CHILD_ATTEMPT_CANDIDATE.to_owned()),
            ("attempt_candidate_state_sha256", child_state_sha256),
            (
                "attempt_artifact_sha256",
                child_artifact.record.sha256.clone(),
            ),
            (
                "attempt_geometry_program_sha256",
                child_program_sha256.clone(),
            ),
            (
                "attempt_geometry_program_object_sha256",
                child_program_object.sha256.clone(),
            ),
            (
                "attempt_artifact_readback_object_sha256",
                child_readback_object.sha256.clone(),
            ),
            ("attempt_representation_plan_sha256", child_plan_sha256),
            ("render_set_id", CHILD_RENDER_SET.to_owned()),
            ("render_set_sha256", child_render_sha256),
            (
                "render_set_object_sha256",
                child_render_object.sha256.clone(),
            ),
            ("reference_comparison_id", CHILD_COMPARISON.to_owned()),
            ("reference_comparison_sha256", child_comparison_sha256),
            (
                "reference_comparison_object_sha256",
                child_comparison_object.sha256.clone(),
            ),
            ("quality_report_id", CHILD_QUALITY.to_owned()),
            ("quality_report_sha256", child_quality_evidence_sha256),
            (
                "quality_report_object_sha256",
                child_quality_evidence_object.sha256.clone(),
            ),
            ("evidence_bundle_sha256", evidence_bundle_sha256),
        ] {
            object.insert(field.to_owned(), Value::String(value));
        }
        object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        let _ = object;
        child_main["canonical_sha256"] = Value::String(canonical_json_hash(&child_main));
        let child_main_bytes = canonical_json_bytes(&child_main).expect("child Main bytes");
        let child_pass_object = store
            .put_object(
                &child_main_bytes,
                None,
                KNIFE_PASS_STATE_JSON_MIME,
                KNIFE_PASS_STATE_OBJECT_KIND,
                NOW,
            )
            .expect("child pass object");
        let child_record = record_from_main_value(
            child_main,
            child_pass_object.record.sha256.clone(),
            "knife-pass-state-child-idempotency",
        )
        .expect("child pass record");
        mark_all_reachable(store);
        KnifePassStateCommit {
            record: child_record,
            cas: KnifePassStateCasBundle {
                pass_state: child_pass_object.record,
            },
        }
    }

    fn commit_with_main_edit<F>(
        store: &Store,
        record: &KnifePassStateStoreRecord,
        edit: F,
    ) -> KnifePassStateCommit
    where
        F: FnOnce(&mut Value),
    {
        let mut main = main_value(record).expect("Main projection");
        edit(&mut main);
        main["canonical_sha256"] = Value::String(String::new());
        main["canonical_sha256"] = Value::String(canonical_json_hash(&main));
        let bytes = canonical_json_bytes(&main).expect("Main bytes");
        let pass_object = store
            .put_object(
                &bytes,
                None,
                KNIFE_PASS_STATE_JSON_MIME,
                KNIFE_PASS_STATE_OBJECT_KIND,
                NOW,
            )
            .expect("pass object");
        let record = record_from_main_value(
            main,
            pass_object.record.sha256.clone(),
            record.idempotency_key.clone(),
        )
        .expect("edited pass record");
        KnifePassStateCommit {
            record,
            cas: KnifePassStateCasBundle {
                pass_state: pass_object.record,
            },
        }
    }

    fn contract_code(error: &StoreError) -> Option<&str> {
        match error {
            StoreError::Contract { code, .. } => Some(code.as_str()),
            _ => None,
        }
    }

    fn pass_row_count(store: &Store) -> i64 {
        store
            .connection
            .lock()
            .expect("connection")
            .query_row("SELECT COUNT(*) FROM knife_pass_state_records", [], |row| {
                row.get(0)
            })
            .expect("pass row count")
    }

    #[test]
    fn knife_pass_state_store_atomically_promotes_temporary_visual_roots() {
        let store = Store::memory().expect("memory Store");
        let fixture = setup_fixture(&store);
        let visual_roots = [
            fixture.commit.record.render_set_object_sha256.as_str(),
            fixture
                .commit
                .record
                .reference_comparison_object_sha256
                .as_str(),
            fixture.commit.record.quality_report_object_sha256.as_str(),
            fixture.camera_object_sha256.as_str(),
            fixture.mask_object_sha256.as_str(),
        ];
        {
            let connection = store.connection.lock().expect("connection");
            for hash in visual_roots {
                connection
                    .execute(
                        "UPDATE objects SET reachability = 'temporary' WHERE sha256 = ?1",
                        params![hash],
                    )
                    .expect("visual root temporary");
            }
        }

        store
            .record_knife_pass_state_with_replay(&fixture.commit)
            .expect("temporary visual roots commit atomically");

        let connection = store.connection.lock().expect("connection");
        for hash in visual_roots {
            let reachability: String = connection
                .query_row(
                    "SELECT reachability FROM objects WHERE sha256 = ?1",
                    params![hash],
                    |row| row.get(0),
                )
                .expect("visual root reachability");
            assert_eq!(reachability, "reachable");
        }
    }

    #[test]
    fn knife_pass_state_store_accepts_source_bound_correction_and_rejects_fake_successor() {
        let store = Store::memory().expect("memory Store");
        let parent = setup_fixture(&store);
        store
            .record_knife_pass_state_with_replay(&parent.commit)
            .expect("root pass commit");
        let child = setup_child_fixture(&store, &parent);
        let (stored, replayed) = store
            .record_knife_pass_state_with_replay(&child)
            .expect("source-bound correction commit");
        assert!(!replayed);
        assert_eq!(stored, child.record);
        assert_eq!(pass_row_count(&store), 2);

        let mut fake = commit_with_main_edit(&store, &child.record, |main| {
            main["pass_id"] = Value::String("knife-pass-state-fake-child".to_owned());
            main["parent_pass_id"] = Value::String(PASS.to_owned());
            main["parent_pass_sha256"] =
                Value::String(parent.commit.record.canonical_sha256.clone());
            for (field, value) in [
                (
                    "baseline_candidate_id",
                    parent.commit.record.baseline_candidate_id.clone(),
                ),
                (
                    "baseline_candidate_state_sha256",
                    parent.commit.record.baseline_candidate_state_sha256.clone(),
                ),
                (
                    "baseline_artifact_sha256",
                    parent.commit.record.baseline_artifact_sha256.clone(),
                ),
                (
                    "baseline_geometry_program_sha256",
                    parent
                        .commit
                        .record
                        .baseline_geometry_program_sha256
                        .clone(),
                ),
                (
                    "baseline_geometry_program_object_sha256",
                    parent
                        .commit
                        .record
                        .baseline_geometry_program_object_sha256
                        .clone(),
                ),
                (
                    "baseline_artifact_readback_object_sha256",
                    parent
                        .commit
                        .record
                        .baseline_artifact_readback_object_sha256
                        .clone(),
                ),
                (
                    "baseline_representation_plan_sha256",
                    parent
                        .commit
                        .record
                        .baseline_representation_plan_sha256
                        .clone(),
                ),
            ] {
                main[field] = Value::String(value);
            }
        });
        fake.record.idempotency_key = "knife-pass-state-fake-idempotency".to_owned();
        let error = store
            .record_knife_pass_state_with_replay(&fake)
            .expect_err("unchanged correction must fail closed");
        assert_eq!(
            contract_code(&error),
            Some("KNIFE_PASS_STATE_PARENT_SUCCESSOR_BASELINE_MISMATCH")
        );
        assert_eq!(pass_row_count(&store), 2);

        let mut program_drift = commit_with_main_edit(&store, &child.record, |main| {
            main["pass_id"] = Value::String("knife-pass-state-program-drift".to_owned());
            main["attempt_geometry_program_sha256"] = Value::String(h('z'));
        });
        program_drift.record.idempotency_key =
            "knife-pass-state-program-drift-idempotency".to_owned();
        let error = store
            .record_knife_pass_state_with_replay(&program_drift)
            .expect_err("attempt program drift must fail closed");
        assert_eq!(
            contract_code(&error),
            Some("KNIFE_PASS_STATE_GEOMETRY_EVIDENCE_BINDING_MISMATCH")
        );
        assert_eq!(pass_row_count(&store), 2);

        let mut embedded_binding_drift = commit_with_main_edit(&store, &child.record, |main| {
            main["pass_id"] = Value::String("knife-pass-state-binding-drift".to_owned());
            main["source_binding_sha256"] = Value::String(h('z'));
        });
        embedded_binding_drift.record.idempotency_key =
            "knife-pass-state-binding-drift-idempotency".to_owned();
        let error = store
            .record_knife_pass_state_with_replay(&embedded_binding_drift)
            .expect_err("embedded source binding drift must fail closed");
        assert!(
            contract_code(&error).is_some_and(|code| code.contains("SOURCE_BINDING")),
            "unexpected binding drift error: {error:?}"
        );
        assert_eq!(pass_row_count(&store), 2);

        let mut stage_drift = commit_with_main_edit(&store, &child.record, |main| {
            main["pass_id"] = Value::String("knife-pass-state-stage-drift".to_owned());
            main["stage"] = Value::String("camera-lock".to_owned());
        });
        stage_drift.record.idempotency_key = "knife-pass-state-stage-drift-idempotency".to_owned();
        let error = store
            .record_knife_pass_state_with_replay(&stage_drift)
            .expect_err("parent stage regression must fail closed");
        assert_eq!(
            contract_code(&error),
            Some("KNIFE_PASS_STATE_PARENT_SUCCESSOR_STAGE_REGRESSION")
        );
        assert_eq!(pass_row_count(&store), 2);
    }

    #[test]
    fn knife_pass_state_store_replay_conflict_restart_and_exact_get() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-knife-pass-state-store-{}",
            Uuid::new_v4()
        ));
        let database = root.join("store.sqlite3");
        let cas_root = root.join("cas");
        let store = Store::open_with_cas(&database, &cas_root).expect("open Store");
        let fixture = setup_fixture(&store);

        let (stored, replayed) = store
            .record_knife_pass_state_with_replay(&fixture.commit)
            .expect("first pass commit");
        assert!(!replayed);
        assert_eq!(stored, fixture.commit.record);
        assert_eq!(pass_row_count(&store), 1);
        assert_eq!(
            store
                .get_knife_pass_state_exact_record(&fixture.commit.record)
                .expect("exact get")
                .as_ref(),
            Some(&fixture.commit.record)
        );

        let (replayed_record, replayed) = store
            .record_knife_pass_state_with_replay(&fixture.commit)
            .expect("exact replay");
        assert!(replayed);
        assert_eq!(replayed_record, fixture.commit.record);
        assert_eq!(pass_row_count(&store), 1);

        let conflicting = commit_with_main_edit(&store, &fixture.commit.record, |main| {
            main["stage"] = Value::String("structural-form".to_owned());
        });
        let error = store
            .record_knife_pass_state_with_replay(&conflicting)
            .expect_err("same idempotency must conflict");
        assert_eq!(
            contract_code(&error),
            Some("KNIFE_PASS_STATE_IDEMPOTENCY_CONFLICT")
        );
        assert_eq!(pass_row_count(&store), 1);
        assert_eq!(
            store
                .get_knife_pass_state_by_idempotency(
                    PROJECT,
                    &fixture.commit.record.idempotency_key,
                )
                .expect("idempotency lookup")
                .as_ref(),
            Some(&fixture.commit.record)
        );
        let bytes = store
            .read_knife_pass_state_json(&fixture.commit.record)
            .expect("exact Main readback");
        let main: Value = serde_json::from_slice(&bytes).expect("Main JSON");
        assert!(main.get("pass_state_object_sha256").is_none());
        assert!(main.get("idempotency_key").is_none());
        assert_eq!(
            main["canonical_sha256"],
            fixture.commit.record.canonical_sha256
        );

        drop(store);
        let reopened = Store::open_with_cas(&database, &cas_root).expect("reopen Store");
        assert_eq!(
            reopened
                .get_knife_pass_state_exact_record(&fixture.commit.record)
                .expect("restart exact get")
                .as_ref(),
            Some(&fixture.commit.record)
        );
        assert_eq!(pass_row_count(&reopened), 1);
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn knife_pass_state_store_evidence_and_lineage_drift_leave_zero_rows() {
        let store = Store::memory().expect("memory Store");
        let fixture = setup_fixture(&store);
        let evidence_drift = commit_with_main_edit(&store, &fixture.commit.record, |main| {
            main["evidence_bundle_sha256"] = Value::String(h('e'));
        });
        let error = store
            .record_knife_pass_state_with_replay(&evidence_drift)
            .expect_err("evidence drift must fail closed");
        assert_eq!(
            contract_code(&error),
            Some("KNIFE_PASS_STATE_EVIDENCE_BUNDLE_MISMATCH")
        );
        assert_eq!(pass_row_count(&store), 0);

        let store = Store::memory().expect("memory Store");
        let fixture = setup_fixture(&store);
        let plan_drift = commit_with_main_edit(&store, &fixture.commit.record, |main| {
            main["attempt_representation_plan_sha256"] = Value::String(h('d'));
        });
        let error = store
            .record_knife_pass_state_with_replay(&plan_drift)
            .expect_err("materializer plan drift must fail closed");
        assert_eq!(
            contract_code(&error),
            Some("KNIFE_PASS_STATE_MATERIALIZER_PLAN_MISMATCH")
        );
        assert_eq!(pass_row_count(&store), 0);

        let store = Store::memory().expect("memory Store");
        let fixture = setup_fixture(&store);
        let source_program_drift = commit_with_main_edit(&store, &fixture.commit.record, |main| {
            main["attempt_geometry_program_sha256"] = Value::String(h('c'));
        });
        let error = store
            .record_knife_pass_state_with_replay(&source_program_drift)
            .expect_err("source program drift must fail closed");
        assert_eq!(
            contract_code(&error),
            Some("KNIFE_PASS_STATE_GEOMETRY_EVIDENCE_BINDING_MISMATCH")
        );
        assert_eq!(pass_row_count(&store), 0);

        let store = Store::memory().expect("memory Store");
        let fixture = setup_fixture(&store);
        let program_bytes = store
            .cas()
            .read_verified_bounded(
                &fixture.source_program_object_sha256,
                MAX_LINEAGE_JSON_BYTES,
            )
            .expect("source program bytes");
        let mut program: Value = serde_json::from_slice(&program_bytes).expect("program JSON");
        program["part_outputs"] = json!([]);
        let tampered_bytes = canonical_json_bytes(&program).expect("tampered program bytes");
        let program_path = store
            .cas()
            .root()
            .join("objects")
            .join(&fixture.source_program_object_sha256[..2])
            .join(&fixture.source_program_object_sha256);
        fs::write(program_path, tampered_bytes).expect("tamper source program");
        let error = store
            .record_knife_pass_state_with_replay(&fixture.commit)
            .expect_err("source part drift must fail closed");
        assert!(
            matches!(error, StoreError::Cas(_))
                || contract_code(&error)
                    .is_some_and(|code| code.contains("CAS") || code.contains("GEOMETRY")),
            "unexpected error: {error:?}"
        );
        assert_eq!(pass_row_count(&store), 0);
    }

    #[test]
    fn knife_pass_state_store_cas_tamper_and_gc_roots() {
        let root =
            std::env::temp_dir().join(format!("forgecad-knife-pass-state-gc-{}", Uuid::new_v4()));
        let database = root.join("store.sqlite3");
        let cas_root = root.join("cas");
        let store = Store::open_with_cas(&database, &cas_root).expect("open Store");
        let fixture = setup_fixture(&store);
        store
            .record_knife_pass_state_with_replay(&fixture.commit)
            .expect("pass commit");

        let roots = {
            let connection = store.connection.lock().expect("connection");
            let mut statement = connection
                .prepare(
                    "SELECT object_sha256 FROM knife_pass_state_roots WHERE project_id = ?1 AND pass_id = ?2",
                )
                .expect("root query");
            statement
                .query_map(params![PROJECT, PASS], |row| row.get::<_, String>(0))
                .expect("root rows")
                .collect::<Result<Vec<_>, _>>()
                .expect("root values")
        };
        for expected in [
            fixture.commit.record.pass_state_object_sha256.as_str(),
            fixture.camera_object_sha256.as_str(),
            fixture.mask_object_sha256.as_str(),
            fixture.source_quality_object_sha256.as_str(),
            fixture.quality_contract_object_sha256.as_str(),
            fixture.commit.record.source_binding_object_sha256.as_str(),
            fixture.commit.record.intent_bundle_object_sha256.as_str(),
            fixture.commit.record.brief_object_sha256.as_str(),
            fixture
                .commit
                .record
                .attempt_geometry_program_object_sha256
                .as_str(),
            fixture
                .commit
                .record
                .attempt_artifact_readback_object_sha256
                .as_str(),
            fixture.source_artifact_readback_object_sha256.as_str(),
        ] {
            assert!(
                roots.iter().any(|root| root == expected),
                "missing root {expected}; actual roots={roots:?}; fixture camera={} mask={} source_quality={} quality_contract={}",
                fixture.camera_object_sha256,
                fixture.mask_object_sha256,
                fixture.source_quality_object_sha256,
                fixture.quality_contract_object_sha256
            );
        }
        assert!(roots.len() >= 10);

        let pass_path = store
            .cas()
            .root()
            .join("objects")
            .join(&fixture.commit.record.pass_state_object_sha256[..2])
            .join(&fixture.commit.record.pass_state_object_sha256);
        fs::write(pass_path, b"tampered pass state").expect("tamper pass CAS");
        let error = store
            .get_knife_pass_state(PROJECT, PASS, &fixture.commit.record.canonical_sha256)
            .expect_err("tampered pass CAS must fail closed");
        assert!(
            error.to_string().contains("CAS")
                || contract_code(&error).is_some_and(|code| code.contains("CAS")),
            "unexpected tamper error: {error:?}"
        );
        assert_eq!(pass_row_count(&store), 1);
        drop(store);
        let _ = fs::remove_dir_all(root);
    }
}
