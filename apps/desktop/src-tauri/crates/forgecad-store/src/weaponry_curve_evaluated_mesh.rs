//! Store-local durability for the bounded knife curve evaluated-mesh slice.
//!
//! This module is deliberately a persistence boundary.  The curve evaluator
//! lives in `forgecad-core`; Runtime stages its four immutable JSON outputs in
//! CAS and this module verifies their metadata, canonical bytes and lineage
//! before installing one SQLite binding.  The evaluated mesh is disposable and
//! never replaces the structural Curve/ModifierGraph record or AuthoringMesh.

use super::{
    CasObjectRecord, CasStore, Store, StoreError, WEAPONRY_CURVE_MODIFIER_GRAPH_JSON_MIME,
    WEAPONRY_CURVE_SET_OBJECT_KIND, WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND,
    WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND, WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
    WEAPONRY_SAMPLE_SET_OBJECT_KIND, canonical_json_bytes, canonical_json_hash, is_opaque_id,
    is_sha256, mark_reachable_in_transaction,
};
use forgecad_core::weaponry_dcc::{
    EvaluatedMeshGeometry, EvaluatedMeshIdentity, EvaluatedMeshLink, Sha256Hash,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const WEAPONRY_CURVE_EVALUATED_MESH_RECORD_SCHEMA: &str =
    "KnifeCurveEvaluatedMeshDurableRecord@1";
pub const WEAPONRY_CURVE_EVALUATED_MESH_STATUS: &str =
    "runtime-owned-store-weaponry-curve-evaluated-mesh@1";
pub const WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND: &str = "weaponry-curve-evaluation-plan";
pub const WEAPONRY_EVALUATED_MESH_OBJECT_KIND: &str = "weaponry-evaluated-mesh";
pub const WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND: &str = "weaponry-evaluated-mesh-identity";
pub const WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND: &str = "weaponry-evaluated-mesh-link";
pub const WEAPONRY_CURVE_EVALUATED_MESH_JSON_MIME: &str = WEAPONRY_CURVE_MODIFIER_GRAPH_JSON_MIME;
pub const WEAPONRY_CURVE_EVALUATED_MESH_MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;

/// Store-local index for a disposable evaluated mesh.  The structural source
/// record is referenced by `curve_graph_lookup_key_sha256`; the evaluated
/// result has its own lookup key and idempotency namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryCurveEvaluatedMeshDurableRecord {
    pub schema_version: String,
    pub project_id: String,
    pub curve_graph_lookup_key_sha256: String,
    pub source_candidate_id: String,
    pub source_candidate_state_sha256: String,
    pub source_authoring_mesh_id: String,
    pub source_authoring_mesh_lineage_id: String,
    pub source_authoring_mesh_revision_id: String,
    pub source_authoring_mesh_revision_index: u64,
    pub source_authoring_mesh_revision_sha256: String,
    pub source_authoring_mesh_identity_sha256: String,
    pub source_modifier_graph_id: String,
    pub source_modifier_graph_sha256: String,
    pub curve_set_semantic_sha256: String,
    pub curve_set_object_sha256: String,
    pub sample_set_semantic_sha256: String,
    pub sample_set_object_sha256: String,
    pub modifier_graph_semantic_sha256: String,
    pub modifier_graph_object_sha256: String,
    pub dependency_graph_semantic_sha256: String,
    pub dependency_graph_object_sha256: String,
    pub recompute_plan_semantic_sha256: String,
    pub recompute_plan_object_sha256: String,
    pub evaluation_id: String,
    pub evaluation_plan_semantic_sha256: String,
    pub evaluation_plan_object_sha256: String,
    pub evaluated_mesh_id: String,
    pub evaluated_mesh_semantic_sha256: String,
    pub evaluated_mesh_object_sha256: String,
    pub evaluated_mesh_identity_sha256: String,
    pub evaluated_mesh_identity_object_sha256: String,
    pub evaluated_mesh_link_sha256: String,
    pub evaluated_mesh_link_object_sha256: String,
    pub vertex_count: u64,
    pub triangle_count: u64,
    pub closed_two_manifold: bool,
    pub zero_degenerate_triangles: bool,
    pub evaluated_mesh_lookup_key_sha256: String,
    pub idempotency_key: String,
    pub input_sha256: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// The four immutable JSON CAS objects staged by Runtime before the Store
/// transaction.  Store never creates or deletes these objects.
#[derive(Debug, Clone)]
pub struct WeaponryCurveEvaluatedMeshCasBundle {
    pub evaluation_plan: CasObjectRecord,
    pub evaluated_mesh: CasObjectRecord,
    pub evaluated_mesh_identity: CasObjectRecord,
    pub evaluated_mesh_link: CasObjectRecord,
}

#[derive(Debug, Clone)]
pub struct WeaponryCurveEvaluatedMeshCommit {
    pub record: WeaponryCurveEvaluatedMeshDurableRecord,
    pub cas: WeaponryCurveEvaluatedMeshCasBundle,
}

// Naming aliases keep the Store interface usable by the knife-facing Runtime
// while preserving the Weaponry prefix used by the structural module.
pub type KnifeCurveEvaluatedMeshDurableRecord = WeaponryCurveEvaluatedMeshDurableRecord;
pub type KnifeCurveEvaluatedMeshCasBundle = WeaponryCurveEvaluatedMeshCasBundle;
pub type KnifeCurveEvaluatedMeshCommit = WeaponryCurveEvaluatedMeshCommit;

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn record_value(record: &WeaponryCurveEvaluatedMeshDurableRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn canonical_record_sha256(
    record: &WeaponryCurveEvaluatedMeshDurableRecord,
) -> Result<String, StoreError> {
    let mut value = record_value(record)?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

/// Public for Runtime record construction.  This hash excludes only the
/// record's self-referential `canonical_sha256`; CAS object hashes remain
/// content hashes and are never substituted by this identity hash.
pub fn record_canonical_sha256(
    record: &WeaponryCurveEvaluatedMeshDurableRecord,
) -> Result<String, StoreError> {
    canonical_record_sha256(record)
}

fn canonical_record_bytes(
    record: &WeaponryCurveEvaluatedMeshDurableRecord,
) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&record_value(record)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn validate_record(record: &WeaponryCurveEvaluatedMeshDurableRecord) -> Result<(), StoreError> {
    let ids = [
        record.project_id.as_str(),
        record.source_candidate_id.as_str(),
        record.source_authoring_mesh_id.as_str(),
        record.source_authoring_mesh_lineage_id.as_str(),
        record.source_authoring_mesh_revision_id.as_str(),
        record.source_modifier_graph_id.as_str(),
        record.evaluation_id.as_str(),
        record.evaluated_mesh_id.as_str(),
        record.idempotency_key.as_str(),
    ];
    let hashes = [
        record.curve_graph_lookup_key_sha256.as_str(),
        record.source_candidate_state_sha256.as_str(),
        record.source_authoring_mesh_revision_sha256.as_str(),
        record.source_authoring_mesh_identity_sha256.as_str(),
        record.source_modifier_graph_sha256.as_str(),
        record.curve_set_semantic_sha256.as_str(),
        record.curve_set_object_sha256.as_str(),
        record.sample_set_semantic_sha256.as_str(),
        record.sample_set_object_sha256.as_str(),
        record.modifier_graph_semantic_sha256.as_str(),
        record.modifier_graph_object_sha256.as_str(),
        record.dependency_graph_semantic_sha256.as_str(),
        record.dependency_graph_object_sha256.as_str(),
        record.recompute_plan_semantic_sha256.as_str(),
        record.recompute_plan_object_sha256.as_str(),
        record.evaluation_plan_semantic_sha256.as_str(),
        record.evaluation_plan_object_sha256.as_str(),
        record.evaluated_mesh_semantic_sha256.as_str(),
        record.evaluated_mesh_object_sha256.as_str(),
        record.evaluated_mesh_identity_sha256.as_str(),
        record.evaluated_mesh_identity_object_sha256.as_str(),
        record.evaluated_mesh_link_sha256.as_str(),
        record.evaluated_mesh_link_object_sha256.as_str(),
        record.evaluated_mesh_lookup_key_sha256.as_str(),
        record.input_sha256.as_str(),
        record.canonical_sha256.as_str(),
    ];
    if record.schema_version != WEAPONRY_CURVE_EVALUATED_MESH_RECORD_SCHEMA
        || ids.iter().any(|value| !is_opaque_id(value))
        || hashes.iter().any(|value| !is_sha256(value))
        || record.source_authoring_mesh_revision_index > 1_000_000
        || !(1..=2_000_000).contains(&record.vertex_count)
        || !(1..=4_000_000).contains(&record.triangle_count)
        || !record.closed_two_manifold
        || !record.zero_degenerate_triangles
        || record.materialization_status != WEAPONRY_CURVE_EVALUATED_MESH_STATUS
        || record.idempotency_key.len() > 128
        || record.created_at.is_empty()
        || record.created_at.len() > 128
    {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_RECORD_INVALID",
            "evaluated mesh durable identity, truth or hash is malformed",
        ));
    }
    if canonical_record_sha256(record)? != record.canonical_sha256 {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CANONICAL_MISMATCH",
            "evaluated mesh durable record canonical hash differs",
        ));
    }
    Ok(())
}

fn roots(record: &WeaponryCurveEvaluatedMeshDurableRecord) -> Vec<String> {
    let mut roots = vec![
        record.evaluation_plan_object_sha256.clone(),
        record.evaluated_mesh_object_sha256.clone(),
        record.evaluated_mesh_identity_object_sha256.clone(),
        record.evaluated_mesh_link_object_sha256.clone(),
    ];
    roots.sort();
    roots.dedup();
    roots
}

fn canonical_hash_without_field(value: &Value, field: &str) -> Result<String, StoreError> {
    let mut value = value.clone();
    value
        .as_object_mut()
        .ok_or_else(|| {
            contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_CAS_PAYLOAD_INVALID",
                "canonical hash input must be a JSON object",
            )
        })?
        .remove(field);
    Ok(canonical_json_hash(&value))
}

fn core_hash(value: &str, field: &str) -> Result<Sha256Hash, StoreError> {
    Sha256Hash::new(value).map_err(|error| {
        contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CORE_PAYLOAD_INVALID",
            format!("{field} is not a valid Core SHA-256 hash: {error}"),
        )
    })
}

fn validate_identity_payload(
    value: &Value,
    expected_semantic_sha256: Option<&str>,
) -> Result<(), StoreError> {
    let identity: EvaluatedMeshIdentity =
        serde_json::from_value(value.clone()).map_err(|error| {
            contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_IDENTITY_INVALID",
                format!("EvaluatedMeshIdentity is not typed Core data: {error}"),
            )
        })?;
    let rebuilt = EvaluatedMeshIdentity::new(
        core_hash(
            identity.source_revision_sha256.as_str(),
            "source_revision_sha256",
        )?,
        core_hash(
            identity.modifier_graph_sha256.as_str(),
            "modifier_graph_sha256",
        )?,
        identity
            .input_evaluation_sha256
            .iter()
            .map(|value| core_hash(value.as_str(), "input_evaluation_sha256"))
            .collect::<Result<Vec<_>, _>>()?,
        core_hash(identity.output_mesh_sha256.as_str(), "output_mesh_sha256")?,
    )
    .map_err(|error| {
        contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_IDENTITY_INVALID",
            format!("EvaluatedMeshIdentity failed Core validation: {error}"),
        )
    })?;
    if rebuilt != identity {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_IDENTITY_INVALID",
            "EvaluatedMeshIdentity input hashes are not canonical and unique",
        ));
    }
    let typed_value = serde_json::to_value(&identity).map_err(|error| {
        StoreError::InvalidData(format!(
            "EvaluatedMeshIdentity serialization failed: {error}"
        ))
    })?;
    if typed_value != *value {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_IDENTITY_INVALID",
            "EvaluatedMeshIdentity payload has fields outside the Core type",
        ));
    }
    let semantic = identity.canonical_sha256().map_err(|error| {
        contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_IDENTITY_INVALID",
            format!("EvaluatedMeshIdentity canonical hash failed: {error}"),
        )
    })?;
    if expected_semantic_sha256.is_some_and(|expected| expected != semantic.as_str()) {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CAS_SEMANTIC_MISMATCH",
            "EvaluatedMeshIdentity canonical hash differs from its durable binding",
        ));
    }
    Ok(())
}

fn validate_link_payload(
    value: &Value,
    expected_semantic_sha256: Option<&str>,
) -> Result<(), StoreError> {
    let link: EvaluatedMeshLink = serde_json::from_value(value.clone()).map_err(|error| {
        contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_LINK_INVALID",
            format!("EvaluatedMeshLink is not typed Core data: {error}"),
        )
    })?;
    let typed_value = serde_json::to_value(&link).map_err(|error| {
        StoreError::InvalidData(format!("EvaluatedMeshLink serialization failed: {error}"))
    })?;
    if typed_value != *value {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_LINK_INVALID",
            "EvaluatedMeshLink payload has fields outside the Core type",
        ));
    }
    let identity_value = serde_json::to_value(&link.identity).map_err(|error| {
        StoreError::InvalidData(format!(
            "EvaluatedMeshIdentity serialization failed: {error}"
        ))
    })?;
    validate_identity_payload(&identity_value, None)?;
    let semantic = link.canonical_sha256().map_err(|error| {
        contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_LINK_INVALID",
            format!("EvaluatedMeshLink canonical hash failed: {error}"),
        )
    })?;
    if expected_semantic_sha256.is_some_and(|expected| expected != semantic.as_str()) {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CAS_SEMANTIC_MISMATCH",
            "EvaluatedMeshLink canonical hash differs from its durable binding",
        ));
    }
    Ok(())
}

fn same_json_shape(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) | (Value::Bool(_), Value::Bool(_)) => true,
        (Value::Number(_), Value::Number(_)) | (Value::String(_), Value::String(_)) => true,
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same_json_shape(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| same_json_shape(left, right))
                })
        }
        _ => false,
    }
}

fn validate_typed_payload(
    value: &Value,
    bytes: &[u8],
    expected_kind: &str,
    expected_semantic_sha256: Option<&str>,
) -> Result<(), StoreError> {
    match expected_kind {
        WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND => {
            let declared = value
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    contract(
                        "WEAPONRY_CURVE_EVALUATED_MESH_PLAN_INVALID",
                        "evaluation plan canonical_sha256 is missing or malformed",
                    )
                })?;
            let semantic = canonical_hash_without_field(value, "canonical_sha256")?;
            if declared != semantic
                || expected_semantic_sha256.is_some_and(|expected| expected != semantic)
            {
                return Err(contract(
                    "WEAPONRY_CURVE_EVALUATED_MESH_CAS_SEMANTIC_MISMATCH",
                    "evaluation plan semantic hash differs from its canonical payload",
                ));
            }
            Ok(())
        }
        WEAPONRY_EVALUATED_MESH_OBJECT_KIND => {
            let declared = value
                .get("semantic_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| {
                    contract(
                        "WEAPONRY_CURVE_EVALUATED_MESH_CORE_PAYLOAD_INVALID",
                        "evaluated mesh semantic_sha256 is missing or malformed",
                    )
                })?;
            let mesh: EvaluatedMeshGeometry = serde_json::from_slice(bytes).map_err(|error| {
                contract(
                    "WEAPONRY_CURVE_EVALUATED_MESH_CORE_PAYLOAD_INVALID",
                    format!("evaluated mesh is not typed Core geometry: {error}"),
                )
            })?;
            let typed_value = serde_json::to_value(&mesh).map_err(|error| {
                StoreError::InvalidData(format!("evaluated mesh serialization failed: {error}"))
            })?;
            if !same_json_shape(&typed_value, value) {
                return Err(contract(
                    "WEAPONRY_CURVE_EVALUATED_MESH_CORE_PAYLOAD_INVALID",
                    "evaluated mesh payload has fields outside the Core type",
                ));
            }
            mesh.validate().map_err(|error| {
                contract(
                    "WEAPONRY_CURVE_EVALUATED_MESH_CORE_PAYLOAD_INVALID",
                    format!("evaluated mesh failed Core validation: {error}"),
                )
            })?;
            if expected_semantic_sha256
                .is_some_and(|expected| expected != mesh.semantic_hash().as_str())
                || declared != mesh.semantic_hash().as_str()
            {
                return Err(contract(
                    "WEAPONRY_CURVE_EVALUATED_MESH_CAS_SEMANTIC_MISMATCH",
                    "evaluated mesh semantic hash differs from the validated Core payload",
                ));
            }
            Ok(())
        }
        WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND => {
            validate_identity_payload(value, expected_semantic_sha256)
        }
        WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND => {
            validate_link_payload(value, expected_semantic_sha256)
        }
        _ => {
            let semantic = canonical_json_hash(value);
            if expected_semantic_sha256.is_some_and(|expected| expected != semantic) {
                return Err(contract(
                    "WEAPONRY_CURVE_EVALUATED_MESH_CAS_SEMANTIC_MISMATCH",
                    "source CAS semantic hash differs from its canonical payload",
                ));
            }
            Ok(())
        }
    }
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
                let size_bytes: i64 = row.get(1)?;
                Ok(CasObjectRecord {
                    schema_version: "CasObject@1".to_owned(),
                    sha256: row.get(0)?,
                    size_bytes: u64::try_from(size_bytes)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    mime: row.get(2)?,
                    kind: row.get(3)?,
                    reachability: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(StoreError::from)
}

fn validate_json_object(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    object: &CasObjectRecord,
    expected_sha256: &str,
    expected_kind: &str,
    expected_semantic_sha256: Option<&str>,
    require_reachable: bool,
) -> Result<Value, StoreError> {
    if object.schema_version != "CasObject@1"
        || object.sha256 != expected_sha256
        || !is_sha256(&object.sha256)
        || object.mime != WEAPONRY_CURVE_EVALUATED_MESH_JSON_MIME
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > WEAPONRY_CURVE_EVALUATED_MESH_MAX_JSON_BYTES
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || (require_reachable && object.reachability != "reachable")
        || object.created_at.is_empty()
        || object.created_at.len() > 128
    {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CAS_METADATA_INVALID",
            "evaluated mesh CAS metadata is outside the bounded allowlist",
        ));
    }
    let stored: Option<(i64, String, String, String, String)> = transaction
        .query_row(
            "SELECT size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
            params![object.sha256],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((size, mime, kind, reachability, created_at)) = stored else {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CAS_MISSING",
            "evaluated mesh CAS object is not registered",
        ));
    };
    let reachability_matches = object.reachability == reachability
        || (object.reachability == "temporary" && reachability == "reachable");
    if size != i64::try_from(object.size_bytes).unwrap_or(i64::MAX)
        || mime != object.mime
        || kind != object.kind
        || !reachability_matches
        || created_at != object.created_at
        || (require_reachable && reachability != "reachable")
    {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CAS_METADATA_MISMATCH",
            "evaluated mesh CAS metadata differs from SQLite",
        ));
    }
    let bytes = cas
        .read_verified_bounded(&object.sha256, WEAPONRY_CURVE_EVALUATED_MESH_MAX_JSON_BYTES)
        .map_err(StoreError::from)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
        contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CAS_JSON_INVALID",
            format!("evaluated mesh CAS object is not JSON: {error}"),
        )
    })?;
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if object.size_bytes != bytes.len() as u64 {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CAS_SIZE_MISMATCH",
            "evaluated mesh CAS metadata size differs from canonical JSON bytes",
        ));
    }
    // Every staged JSON root is required to use the same canonical encoding
    // whose bytes are content-addressed by CAS. Core mesh generation quantizes
    // its emitted f64 values at the producer boundary so deserialize/validate
    // remains stable across Store reopen and Runtime readback.
    if canonical != bytes {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_CAS_NOT_CANONICAL",
            "evaluated mesh CAS JSON must use canonical encoding",
        ));
    }
    validate_typed_payload(&value, &bytes, expected_kind, expected_semantic_sha256)?;
    Ok(value)
}

fn read_structural_parent(
    transaction: &Transaction<'_>,
    project_id: &str,
    lookup_key: &str,
) -> Result<super::WeaponryCurveModifierGraphDurableRecord, StoreError> {
    let payload: Option<String> = transaction
        .query_row(
            "SELECT record_json FROM weaponry_curve_modifier_graph_records WHERE project_id = ?1 AND lookup_key_sha256 = ?2",
            params![project_id, lookup_key],
            |row| row.get(0),
        )
        .optional()?;
    let Some(payload) = payload else {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_STRUCTURAL_RECORD_MISSING",
            "evaluated mesh source curve/modifier record is unavailable",
        ));
    };
    serde_json::from_str(&payload).map_err(|error| {
        contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_STRUCTURAL_RECORD_INVALID",
            format!("source curve/modifier record is invalid: {error}"),
        )
    })
}

fn validate_structural_parent(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &WeaponryCurveEvaluatedMeshDurableRecord,
    require_reachable: bool,
) -> Result<(), StoreError> {
    let parent = read_structural_parent(
        transaction,
        &record.project_id,
        &record.curve_graph_lookup_key_sha256,
    )?;
    let mut parent_value = serde_json::to_value(&parent)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    let parent_canonical = parent.canonical_sha256.clone();
    parent_value["canonical_sha256"] = Value::String(String::new());
    if parent.schema_version != super::WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_SCHEMA
        || parent.project_id != record.project_id
        || parent.lookup_key_sha256 != record.curve_graph_lookup_key_sha256
        || canonical_json_hash(&parent_value) != parent_canonical
        || parent.source_candidate_id != record.source_candidate_id
        || parent.source_candidate_state_sha256 != record.source_candidate_state_sha256
        || parent.source_authoring_mesh_id != record.source_authoring_mesh_id
        || parent.source_authoring_mesh_lineage_id != record.source_authoring_mesh_lineage_id
        || parent.source_revision_id != record.source_authoring_mesh_revision_id
        || parent.source_revision_sha256 != record.source_authoring_mesh_revision_sha256
        || parent.source_authoring_mesh_revision_index
            != record.source_authoring_mesh_revision_index
        || parent.source_authoring_mesh_identity_sha256
            != record.source_authoring_mesh_identity_sha256
        || parent.modifier_graph_id != record.source_modifier_graph_id
        || parent.modifier_graph_sha256 != record.source_modifier_graph_sha256
        || parent.curve_set_sha256 != record.curve_set_semantic_sha256
        || parent.curve_set_object_sha256 != record.curve_set_object_sha256
        || parent.sample_set_sha256 != record.sample_set_semantic_sha256
        || parent.sample_set_object_sha256 != record.sample_set_object_sha256
        || parent.modifier_graph_sha256 != record.modifier_graph_semantic_sha256
        || parent.modifier_graph_object_sha256 != record.modifier_graph_object_sha256
        || parent.dependency_graph_sha256 != record.dependency_graph_semantic_sha256
        || parent.dependency_graph_object_sha256 != record.dependency_graph_object_sha256
        || parent.recompute_plan_sha256 != record.recompute_plan_semantic_sha256
        || parent.recompute_plan_object_sha256 != record.recompute_plan_object_sha256
    {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_STRUCTURAL_BINDING_MISMATCH",
            "evaluated mesh record does not bind the selected structural source",
        ));
    }
    let expected = [
        (
            &parent.curve_set_object_sha256,
            WEAPONRY_CURVE_SET_OBJECT_KIND,
            &parent.curve_set_sha256,
        ),
        (
            &parent.sample_set_object_sha256,
            WEAPONRY_SAMPLE_SET_OBJECT_KIND,
            &parent.sample_set_sha256,
        ),
        (
            &parent.modifier_graph_object_sha256,
            WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND,
            &parent.modifier_graph_sha256,
        ),
        (
            &parent.dependency_graph_object_sha256,
            WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND,
            &parent.dependency_graph_sha256,
        ),
        (
            &parent.recompute_plan_object_sha256,
            WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
            &parent.recompute_plan_sha256,
        ),
    ];
    for (sha256, kind, semantic) in expected {
        let object = read_object_record(transaction, sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_STRUCTURAL_CAS_MISSING",
                "structural source CAS object disappeared before evaluation",
            ),
            other => other,
        })?;
        validate_json_object(
            transaction,
            cas,
            &object,
            sha256,
            kind,
            Some(semantic.as_str()),
            require_reachable,
        )?;
    }
    Ok(())
}

fn validate_derived_bindings(
    record: &WeaponryCurveEvaluatedMeshDurableRecord,
    plan: &Value,
    mesh: &Value,
    identity: &Value,
    link: &Value,
) -> Result<(), StoreError> {
    if plan.get("evaluation_id").and_then(Value::as_str) != Some(record.evaluation_id.as_str()) {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_PLAN_BINDING_MISMATCH",
            "evaluation plan does not bind evaluation_id",
        ));
    }
    if mesh.get("semantic_sha256").and_then(Value::as_str)
        != Some(record.evaluated_mesh_semantic_sha256.as_str())
    {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_OUTPUT_BINDING_MISMATCH",
            "evaluated mesh output semantic hash is not declared by the mesh",
        ));
    }
    let vertices = mesh
        .get("vertices")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_PAYLOAD_INVALID",
                "evaluated mesh vertices are missing",
            )
        })?;
    let triangles = mesh
        .get("triangles")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_PAYLOAD_INVALID",
                "evaluated mesh triangles are missing",
            )
        })?;
    if vertices.len() != record.vertex_count as usize
        || triangles.len() != record.triangle_count as usize
    {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_COUNT_MISMATCH",
            "evaluated mesh counts differ from its durable truth",
        ));
    }
    let identity_object = identity.as_object().ok_or_else(|| {
        contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_IDENTITY_INVALID",
            "evaluated mesh identity is not an object",
        )
    })?;
    if identity_object
        .get("source_revision_sha256")
        .and_then(Value::as_str)
        != Some(record.source_authoring_mesh_revision_sha256.as_str())
        || identity_object
            .get("modifier_graph_sha256")
            .and_then(Value::as_str)
            != Some(record.source_modifier_graph_sha256.as_str())
        || identity_object
            .get("output_mesh_sha256")
            .and_then(Value::as_str)
            != Some(record.evaluated_mesh_semantic_sha256.as_str())
    {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_IDENTITY_BINDING_MISMATCH",
            "EvaluatedMeshIdentity does not bind source revision, ModifierGraph and output mesh",
        ));
    }
    let link_identity = link.get("identity").ok_or_else(|| {
        contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_LINK_INVALID",
            "EvaluatedMeshLink identity is missing",
        )
    })?;
    if link_identity != identity {
        return Err(contract(
            "WEAPONRY_CURVE_EVALUATED_MESH_LINK_BINDING_MISMATCH",
            "EvaluatedMeshLink does not exactly embed EvaluatedMeshIdentity",
        ));
    }
    Ok(())
}

fn validate_cas_bundle(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &WeaponryCurveEvaluatedMeshDurableRecord,
    bundle: &WeaponryCurveEvaluatedMeshCasBundle,
    require_reachable: bool,
) -> Result<(), StoreError> {
    let expected = [
        (
            &bundle.evaluation_plan,
            &record.evaluation_plan_object_sha256,
            WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND,
            &record.evaluation_plan_semantic_sha256,
        ),
        (
            &bundle.evaluated_mesh,
            &record.evaluated_mesh_object_sha256,
            WEAPONRY_EVALUATED_MESH_OBJECT_KIND,
            &record.evaluated_mesh_semantic_sha256,
        ),
        (
            &bundle.evaluated_mesh_identity,
            &record.evaluated_mesh_identity_object_sha256,
            WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND,
            &record.evaluated_mesh_identity_sha256,
        ),
        (
            &bundle.evaluated_mesh_link,
            &record.evaluated_mesh_link_object_sha256,
            WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND,
            &record.evaluated_mesh_link_sha256,
        ),
    ];
    let mut values = Vec::with_capacity(expected.len());
    for (object, expected_sha256, expected_kind, expected_semantic) in expected {
        values.push(validate_json_object(
            transaction,
            cas,
            object,
            expected_sha256,
            expected_kind,
            Some(expected_semantic.as_str()),
            require_reachable,
        )?);
    }
    validate_derived_bindings(record, &values[0], &values[1], &values[2], &values[3])
}

fn same_record(
    left: &WeaponryCurveEvaluatedMeshDurableRecord,
    right: &WeaponryCurveEvaluatedMeshDurableRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.created_at.clear();
    right.created_at.clear();
    left.canonical_sha256.clear();
    right.canonical_sha256.clear();
    left == right
}

fn read_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<WeaponryCurveEvaluatedMeshDurableRecord> {
    let payload: String = row.get(0)?;
    serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

pub(crate) fn ensure_table(connection: &Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS weaponry_curve_evaluated_mesh_records (
             schema_version TEXT NOT NULL CHECK (schema_version = 'KnifeCurveEvaluatedMeshDurableRecord@1'),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             curve_graph_lookup_key_sha256 TEXT NOT NULL,
             source_candidate_id TEXT NOT NULL,
             source_candidate_state_sha256 TEXT NOT NULL,
             source_authoring_mesh_id TEXT NOT NULL,
             source_authoring_mesh_lineage_id TEXT NOT NULL,
             source_authoring_mesh_revision_id TEXT NOT NULL,
             source_authoring_mesh_revision_index INTEGER NOT NULL CHECK (source_authoring_mesh_revision_index BETWEEN 0 AND 1000000),
             source_authoring_mesh_revision_sha256 TEXT NOT NULL,
             source_authoring_mesh_identity_sha256 TEXT NOT NULL,
             source_modifier_graph_id TEXT NOT NULL,
             source_modifier_graph_sha256 TEXT NOT NULL,
             curve_set_semantic_sha256 TEXT NOT NULL,
             curve_set_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             sample_set_semantic_sha256 TEXT NOT NULL,
             sample_set_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             modifier_graph_semantic_sha256 TEXT NOT NULL,
             modifier_graph_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             dependency_graph_semantic_sha256 TEXT NOT NULL,
             dependency_graph_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             recompute_plan_semantic_sha256 TEXT NOT NULL,
             recompute_plan_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             evaluation_id TEXT NOT NULL,
             evaluation_plan_semantic_sha256 TEXT NOT NULL,
             evaluation_plan_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             evaluated_mesh_id TEXT NOT NULL,
             evaluated_mesh_semantic_sha256 TEXT NOT NULL,
             evaluated_mesh_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             evaluated_mesh_identity_sha256 TEXT NOT NULL,
             evaluated_mesh_identity_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             evaluated_mesh_link_sha256 TEXT NOT NULL,
             evaluated_mesh_link_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             vertex_count INTEGER NOT NULL CHECK (vertex_count BETWEEN 1 AND 2000000),
             triangle_count INTEGER NOT NULL CHECK (triangle_count BETWEEN 1 AND 4000000),
             closed_two_manifold INTEGER NOT NULL CHECK (closed_two_manifold = 1),
             zero_degenerate_triangles INTEGER NOT NULL CHECK (zero_degenerate_triangles = 1),
             evaluated_mesh_lookup_key_sha256 TEXT NOT NULL,
             idempotency_key TEXT NOT NULL,
             input_sha256 TEXT NOT NULL,
             materialization_status TEXT NOT NULL CHECK (materialization_status = 'runtime-owned-store-weaponry-curve-evaluated-mesh@1'),
             canonical_sha256 TEXT NOT NULL,
             created_at TEXT NOT NULL,
             record_json TEXT NOT NULL,
             PRIMARY KEY (project_id, evaluated_mesh_lookup_key_sha256),
             UNIQUE (project_id, idempotency_key)
         );
         CREATE INDEX IF NOT EXISTS weaponry_curve_evaluated_mesh_source_idx
             ON weaponry_curve_evaluated_mesh_records(project_id, curve_graph_lookup_key_sha256, created_at DESC);
         CREATE INDEX IF NOT EXISTS weaponry_curve_evaluated_mesh_object_idx
             ON weaponry_curve_evaluated_mesh_records(evaluation_plan_object_sha256,
                                                       evaluated_mesh_object_sha256,
                                                       evaluated_mesh_identity_object_sha256,
                                                       evaluated_mesh_link_object_sha256);",
    )?;
    Ok(())
}

fn stored_objects(
    transaction: &Transaction<'_>,
    cas: &CasStore,
    record: &WeaponryCurveEvaluatedMeshDurableRecord,
    require_reachable: bool,
) -> Result<(), StoreError> {
    let objects = [
        (
            &record.evaluation_plan_object_sha256,
            WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND,
            &record.evaluation_plan_semantic_sha256,
        ),
        (
            &record.evaluated_mesh_object_sha256,
            WEAPONRY_EVALUATED_MESH_OBJECT_KIND,
            &record.evaluated_mesh_semantic_sha256,
        ),
        (
            &record.evaluated_mesh_identity_object_sha256,
            WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND,
            &record.evaluated_mesh_identity_sha256,
        ),
        (
            &record.evaluated_mesh_link_object_sha256,
            WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND,
            &record.evaluated_mesh_link_sha256,
        ),
    ];
    let mut values = Vec::with_capacity(objects.len());
    for (sha256, kind, semantic) in objects {
        let object = read_object_record(transaction, sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_CAS_MISSING",
                "evaluated mesh CAS root disappeared before readback",
            ),
            other => other,
        })?;
        values.push(validate_json_object(
            transaction,
            cas,
            &object,
            sha256,
            kind,
            Some(semantic.as_str()),
            require_reachable,
        )?);
    }
    validate_derived_bindings(record, &values[0], &values[1], &values[2], &values[3])
}

impl Store {
    /// Read a reachable evaluated-mesh JSON root with exact kind and CAS
    /// integrity checks.  Runtime uses this to include the canonical plan in
    /// a result without opening the CAS directly.
    pub fn read_weaponry_curve_evaluated_mesh_json(
        &self,
        sha256: &str,
        expected_kind: &str,
    ) -> Result<Value, StoreError> {
        if !is_sha256(sha256)
            || !matches!(
                expected_kind,
                WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND
                    | WEAPONRY_EVALUATED_MESH_OBJECT_KIND
                    | WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND
                    | WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND
            )
        {
            return Err(StoreError::InvalidData(
                "evaluated mesh JSON root identity or kind is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let object = read_object_record(&transaction, sha256).map_err(|error| match error {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_CAS_MISSING",
                "evaluated mesh JSON root is not registered",
            ),
            other => other,
        })?;
        let bytes = self
            .cas
            .read_verified_bounded(&object.sha256, WEAPONRY_CURVE_EVALUATED_MESH_MAX_JSON_BYTES)
            .map_err(StoreError::from)?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
            contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_CAS_JSON_INVALID",
                format!("evaluated mesh JSON root is invalid: {error}"),
            )
        })?;
        validate_json_object(
            &transaction,
            &self.cas,
            &object,
            sha256,
            expected_kind,
            None,
            true,
        )?;
        transaction.commit()?;
        Ok(value)
    }

    /// Atomically install the evaluated-mesh sidecar after validating its
    /// existing structural parent and four staged CAS roots.  A same-key
    /// exact replay returns `(record, true)`; key reuse with any changed
    /// binding fails closed.
    pub fn record_weaponry_curve_evaluated_mesh_with_replay(
        &self,
        commit: &WeaponryCurveEvaluatedMeshCommit,
    ) -> Result<(WeaponryCurveEvaluatedMeshDurableRecord, bool), StoreError> {
        validate_record(&commit.record)?;
        if commit.cas.evaluation_plan.sha256 != commit.record.evaluation_plan_object_sha256
            || commit.cas.evaluated_mesh.sha256 != commit.record.evaluated_mesh_object_sha256
            || commit.cas.evaluated_mesh_identity.sha256
                != commit.record.evaluated_mesh_identity_object_sha256
            || commit.cas.evaluated_mesh_link.sha256
                != commit.record.evaluated_mesh_link_object_sha256
        {
            return Err(contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_CAS_BINDING_MISMATCH",
                "CAS object hash differs from evaluated mesh durable binding",
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        validate_structural_parent(&transaction, &self.cas, &commit.record, true)?;
        validate_cas_bundle(&transaction, &self.cas, &commit.record, &commit.cas, false)?;

        let existing = transaction
            .query_row(
                "SELECT record_json FROM weaponry_curve_evaluated_mesh_records WHERE project_id = ?1 AND idempotency_key = ?2",
                params![commit.record.project_id, commit.record.idempotency_key],
                read_record,
            )
            .optional()?;
        if let Some(existing) = existing {
            validate_record(&existing)?;
            validate_structural_parent(&transaction, &self.cas, &existing, true)?;
            stored_objects(&transaction, &self.cas, &existing, true)?;
            if !same_record(&existing, &commit.record) {
                return Err(contract(
                    "WEAPONRY_CURVE_EVALUATED_MESH_IDEMPOTENCY_CONFLICT",
                    "idempotency key is already bound to different evaluated mesh input",
                ));
            }
            mark_reachable_in_transaction(&transaction, &roots(&existing))?;
            transaction.commit()?;
            return Ok((existing, true));
        }

        let duplicate: Option<String> = transaction
            .query_row(
                "SELECT idempotency_key FROM weaponry_curve_evaluated_mesh_records WHERE project_id = ?1 AND evaluated_mesh_lookup_key_sha256 = ?2",
                params![commit.record.project_id, commit.record.evaluated_mesh_lookup_key_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if duplicate.is_some() {
            return Err(contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_LOOKUP_CONFLICT",
                "evaluated mesh lookup key is already bound to another input",
            ));
        }

        let record_json =
            String::from_utf8(canonical_record_bytes(&commit.record)?).map_err(|error| {
                StoreError::InvalidData(format!(
                    "evaluated mesh durable record is not UTF-8: {error}"
                ))
            })?;
        transaction.execute(
            "INSERT INTO weaponry_curve_evaluated_mesh_records (schema_version, project_id, curve_graph_lookup_key_sha256, source_candidate_id, source_candidate_state_sha256, source_authoring_mesh_id, source_authoring_mesh_lineage_id, source_authoring_mesh_revision_id, source_authoring_mesh_revision_index, source_authoring_mesh_revision_sha256, source_authoring_mesh_identity_sha256, source_modifier_graph_id, source_modifier_graph_sha256, curve_set_semantic_sha256, curve_set_object_sha256, sample_set_semantic_sha256, sample_set_object_sha256, modifier_graph_semantic_sha256, modifier_graph_object_sha256, dependency_graph_semantic_sha256, dependency_graph_object_sha256, recompute_plan_semantic_sha256, recompute_plan_object_sha256, evaluation_id, evaluation_plan_semantic_sha256, evaluation_plan_object_sha256, evaluated_mesh_id, evaluated_mesh_semantic_sha256, evaluated_mesh_object_sha256, evaluated_mesh_identity_sha256, evaluated_mesh_identity_object_sha256, evaluated_mesh_link_sha256, evaluated_mesh_link_object_sha256, vertex_count, triangle_count, closed_two_manifold, zero_degenerate_triangles, evaluated_mesh_lookup_key_sha256, idempotency_key, input_sha256, materialization_status, canonical_sha256, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44)",
            params![
                commit.record.schema_version,
                commit.record.project_id,
                commit.record.curve_graph_lookup_key_sha256,
                commit.record.source_candidate_id,
                commit.record.source_candidate_state_sha256,
                commit.record.source_authoring_mesh_id,
                commit.record.source_authoring_mesh_lineage_id,
                commit.record.source_authoring_mesh_revision_id,
                i64::try_from(commit.record.source_authoring_mesh_revision_index).map_err(|_| StoreError::InvalidData("authoring mesh revision index is too large".to_owned()))?,
                commit.record.source_authoring_mesh_revision_sha256,
                commit.record.source_authoring_mesh_identity_sha256,
                commit.record.source_modifier_graph_id,
                commit.record.source_modifier_graph_sha256,
                commit.record.curve_set_semantic_sha256,
                commit.record.curve_set_object_sha256,
                commit.record.sample_set_semantic_sha256,
                commit.record.sample_set_object_sha256,
                commit.record.modifier_graph_semantic_sha256,
                commit.record.modifier_graph_object_sha256,
                commit.record.dependency_graph_semantic_sha256,
                commit.record.dependency_graph_object_sha256,
                commit.record.recompute_plan_semantic_sha256,
                commit.record.recompute_plan_object_sha256,
                commit.record.evaluation_id,
                commit.record.evaluation_plan_semantic_sha256,
                commit.record.evaluation_plan_object_sha256,
                commit.record.evaluated_mesh_id,
                commit.record.evaluated_mesh_semantic_sha256,
                commit.record.evaluated_mesh_object_sha256,
                commit.record.evaluated_mesh_identity_sha256,
                commit.record.evaluated_mesh_identity_object_sha256,
                commit.record.evaluated_mesh_link_sha256,
                commit.record.evaluated_mesh_link_object_sha256,
                i64::try_from(commit.record.vertex_count).map_err(|_| StoreError::InvalidData("vertex count is too large".to_owned()))?,
                i64::try_from(commit.record.triangle_count).map_err(|_| StoreError::InvalidData("triangle count is too large".to_owned()))?,
                if commit.record.closed_two_manifold { 1_i64 } else { 0_i64 },
                if commit.record.zero_degenerate_triangles { 1_i64 } else { 0_i64 },
                commit.record.evaluated_mesh_lookup_key_sha256,
                commit.record.idempotency_key,
                commit.record.input_sha256,
                commit.record.materialization_status,
                commit.record.canonical_sha256,
                commit.record.created_at,
                record_json,
            ],
        )?;
        mark_reachable_in_transaction(&transaction, &roots(&commit.record))?;
        let stored = transaction.query_row(
            "SELECT record_json FROM weaponry_curve_evaluated_mesh_records WHERE project_id = ?1 AND evaluated_mesh_lookup_key_sha256 = ?2",
            params![commit.record.project_id, commit.record.evaluated_mesh_lookup_key_sha256],
            read_record,
        )?;
        validate_record(&stored)?;
        stored_objects(&transaction, &self.cas, &stored, true)?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn get_weaponry_curve_evaluated_mesh(
        &self,
        project_id: &str,
        evaluated_mesh_lookup_key_sha256: &str,
    ) -> Result<Option<WeaponryCurveEvaluatedMeshDurableRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_sha256(evaluated_mesh_lookup_key_sha256) {
            return Err(StoreError::InvalidData(
                "evaluated mesh lookup identity is invalid".to_owned(),
            ));
        }
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let record = transaction
            .query_row(
                "SELECT record_json FROM weaponry_curve_evaluated_mesh_records WHERE project_id = ?1 AND evaluated_mesh_lookup_key_sha256 = ?2",
                params![project_id, evaluated_mesh_lookup_key_sha256],
                read_record,
            )
            .optional()?;
        let Some(record) = record else {
            transaction.commit()?;
            return Ok(None);
        };
        if record.project_id != project_id
            || record.evaluated_mesh_lookup_key_sha256 != evaluated_mesh_lookup_key_sha256
        {
            return Err(contract(
                "WEAPONRY_CURVE_EVALUATED_MESH_SCOPE_MISMATCH",
                "stored evaluated mesh record scope differs",
            ));
        }
        validate_record(&record)?;
        validate_structural_parent(&transaction, &self.cas, &record, true)?;
        stored_objects(&transaction, &self.cas, &record, true)?;
        transaction.commit()?;
        Ok(Some(record))
    }

    pub fn weaponry_curve_evaluated_mesh_cas_roots(
        record: &WeaponryCurveEvaluatedMeshDurableRecord,
    ) -> Vec<String> {
        roots(record)
    }

    pub fn record_knife_curve_evaluated_mesh_with_replay(
        &self,
        commit: &KnifeCurveEvaluatedMeshCommit,
    ) -> Result<(KnifeCurveEvaluatedMeshDurableRecord, bool), StoreError> {
        self.record_weaponry_curve_evaluated_mesh_with_replay(commit)
    }

    pub fn get_knife_curve_evaluated_mesh(
        &self,
        project_id: &str,
        evaluated_mesh_lookup_key_sha256: &str,
    ) -> Result<Option<KnifeCurveEvaluatedMeshDurableRecord>, StoreError> {
        self.get_weaponry_curve_evaluated_mesh(project_id, evaluated_mesh_lookup_key_sha256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weaponry_curve_modifier_graph::{
        WeaponryCurveModifierGraphCasBundle, WeaponryCurveModifierGraphCommit,
        WeaponryCurveModifierGraphDurableRecord,
    };
    use forgecad_core::weaponry_dcc::{
        KnifeBladeSweepPlan, KnifeCurve, KnifeCurveBasis, KnifeCurveRole, Sha256Hash,
    };
    use forgecad_core::{canonical_json_hash, sha256_hex};
    use std::fs;
    use std::path::PathBuf;

    fn hash(seed: &str) -> String {
        canonical_json_hash(&serde_json::json!({"seed": seed}))
    }

    fn project(store: &Store) {
        store
            .insert_project(&super::super::ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: "weaponry".to_owned(),
                name: "Weaponry test".to_owned(),
                policy: serde_json::json!({"scope":"test"}),
                created_at: "1".to_owned(),
                updated_at: "1".to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: "a".repeat(64),
            })
            .expect("project");
    }

    fn object(store: &Store, kind: &str, value: Value) -> CasObjectRecord {
        let bytes = canonical_json_bytes(&value).expect("canonical JSON");
        store
            .put_object(
                &bytes,
                None,
                WEAPONRY_CURVE_EVALUATED_MESH_JSON_MIME,
                kind,
                "1",
            )
            .expect("object")
            .record
    }

    fn structural(store: &Store) -> WeaponryCurveModifierGraphDurableRecord {
        let values = [
            (
                WEAPONRY_CURVE_SET_OBJECT_KIND,
                serde_json::json!({"curves":[]}),
            ),
            (
                WEAPONRY_SAMPLE_SET_OBJECT_KIND,
                serde_json::json!({"samples":[]}),
            ),
            (
                WEAPONRY_MODIFIER_GRAPH_OBJECT_KIND,
                serde_json::json!({"nodes":[]}),
            ),
            (
                WEAPONRY_DEPENDENCY_GRAPH_OBJECT_KIND,
                serde_json::json!({"dependency_nodes":[]}),
            ),
            (
                WEAPONRY_RECOMPUTE_PLAN_OBJECT_KIND,
                serde_json::json!({"dirty_nodes":[]}),
            ),
        ];
        let mut objects = Vec::new();
        let mut semantics = Vec::new();
        for (kind, value) in values {
            semantics.push(canonical_json_hash(&value));
            objects.push(object(store, kind, value));
        }
        let mut record = WeaponryCurveModifierGraphDurableRecord {
            schema_version: super::super::WEAPONRY_CURVE_MODIFIER_GRAPH_RECORD_SCHEMA.to_owned(),
            project_id: "weaponry".to_owned(),
            source_revision_id: "revision-r1".to_owned(),
            source_revision_sha256: hash("revision"),
            source_candidate_id: "candidate-r1".to_owned(),
            source_candidate_state_sha256: hash("candidate-state"),
            source_authoring_mesh_id: "mesh-r1".to_owned(),
            source_authoring_mesh_lineage_id: "lineage-r1".to_owned(),
            source_authoring_mesh_revision_index: 1,
            source_authoring_mesh_identity_sha256: hash("mesh-identity"),
            curve_set_id: "curve-set".to_owned(),
            curve_set_sha256: semantics[0].clone(),
            curve_set_object_sha256: objects[0].sha256.clone(),
            sample_set_id: "sample-set".to_owned(),
            sample_set_sha256: semantics[1].clone(),
            sample_set_object_sha256: objects[1].sha256.clone(),
            modifier_graph_id: "graph-r1".to_owned(),
            modifier_graph_sha256: semantics[2].clone(),
            modifier_graph_object_sha256: objects[2].sha256.clone(),
            dependency_graph_sha256: semantics[3].clone(),
            dependency_graph_object_sha256: objects[3].sha256.clone(),
            recompute_plan_sha256: semantics[4].clone(),
            recompute_plan_object_sha256: objects[4].sha256.clone(),
            lookup_key_sha256: hash("structural-lookup"),
            idempotency_key: "structural-idem".to_owned(),
            input_sha256: hash("structural-input"),
            materialization_status: super::super::WEAPONRY_CURVE_MODIFIER_GRAPH_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: "1".to_owned(),
        };
        let mut value = serde_json::to_value(&record).expect("record value");
        value["canonical_sha256"] = Value::String(String::new());
        record.canonical_sha256 = canonical_json_hash(&value);
        store
            .record_weaponry_curve_modifier_graph_with_replay(&WeaponryCurveModifierGraphCommit {
                record: record.clone(),
                cas: WeaponryCurveModifierGraphCasBundle {
                    curve_set: objects[0].clone(),
                    sample_set: objects[1].clone(),
                    modifier_graph: objects[2].clone(),
                    dependency_graph: objects[3].clone(),
                    recompute_plan: objects[4].clone(),
                },
            })
            .expect("structural commit");
        record
    }

    fn evaluated(
        store: &Store,
        parent: &WeaponryCurveModifierGraphDurableRecord,
    ) -> WeaponryCurveEvaluatedMeshCommit {
        let spine = KnifeCurve::new(
            "blade-spine",
            KnifeCurveRole::BladeSpine,
            KnifeCurveBasis::Bezier,
            3,
            vec![
                [0.0, 0.0, 0.0],
                [0.0, 0.2, 0.4],
                [0.0, 0.6, 0.8],
                [0.0, 1.0, 1.0],
            ],
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("valid blade spine");
        let edge = KnifeCurve::new(
            "blade-edge",
            KnifeCurveRole::BladeEdge,
            KnifeCurveBasis::Bezier,
            3,
            vec![
                [0.42, 0.0, 0.0],
                [0.42, 0.2, 0.0],
                [0.34, 0.65, 0.0],
                [0.0, 1.0, 0.0],
            ],
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("valid blade edge");
        let typed_plan =
            KnifeBladeSweepPlan::from_curves(&spine, &edge, 32, 0.01).expect("valid blade plan");
        let mesh_core = typed_plan
            .evaluate(&spine, &edge)
            .expect("valid blade mesh");
        let source_revision =
            Sha256Hash::new(parent.source_revision_sha256.as_str()).expect("source revision hash");
        let source_modifier_graph = Sha256Hash::new(parent.modifier_graph_sha256.as_str())
            .expect("source modifier graph hash");
        let edge_hash = edge.canonical_sha256().expect("edge hash");
        let identity = mesh_core
            .evaluated_mesh_identity(source_revision, source_modifier_graph, vec![edge_hash])
            .expect("evaluated mesh identity");
        let link = EvaluatedMeshLink::new(identity.clone());
        let mesh = serde_json::to_value(&mesh_core).expect("mesh JSON");
        let canonical_mesh_bytes = canonical_json_bytes(&mesh).expect("mesh canonical JSON");
        let canonical_mesh: EvaluatedMeshGeometry =
            serde_json::from_slice(&canonical_mesh_bytes).expect("canonical mesh deserialize");
        canonical_mesh
            .validate()
            .expect("canonical mesh Core validation");
        let identity_value = serde_json::to_value(&identity).expect("identity JSON");
        let link_value = serde_json::to_value(&link).expect("link JSON");
        let spine_hash = spine.canonical_sha256().expect("spine hash");
        let edge_hash = edge.canonical_sha256().expect("edge hash");
        let plan_without_hash = serde_json::json!({
            "schema_version":"KnifeBladeProfileSweepLoftPlan@1",
            "evaluation_id":"evaluation-r1",
            "spine_curve_id":"blade-spine",
            "spine_curve_sha256":spine_hash,
            "edge_curve_id":"blade-edge",
            "edge_curve_sha256":edge_hash,
            "station_count":32,
            "thickness_axis":"local_normal",
            "thickness_m":0.01,
            "root_cap":true,
            "tip_cap":true,
            "stable_triangulation":"station-ring-fixed-diagonal@1",
            "stable_lineage_policy":"source-curve-modifier-graph-evaluated-mesh@1"
        });
        let plan_semantic = canonical_json_hash(&plan_without_hash);
        let mut plan = plan_without_hash;
        plan["canonical_sha256"] = Value::String(plan_semantic.clone());
        let plan_object = object(store, WEAPONRY_CURVE_EVALUATION_PLAN_OBJECT_KIND, plan);

        let mesh_object = object(store, WEAPONRY_EVALUATED_MESH_OBJECT_KIND, mesh);
        let mesh_semantic = mesh_core.semantic_sha256.as_str().to_owned();
        let identity_semantic = identity
            .canonical_sha256()
            .expect("identity hash")
            .as_str()
            .to_owned();
        let identity_object = object(
            store,
            WEAPONRY_EVALUATED_MESH_IDENTITY_OBJECT_KIND,
            identity_value,
        );
        let link_semantic = link
            .canonical_sha256()
            .expect("link hash")
            .as_str()
            .to_owned();
        let link_object = object(store, WEAPONRY_EVALUATED_MESH_LINK_OBJECT_KIND, link_value);

        let mut record = WeaponryCurveEvaluatedMeshDurableRecord {
            schema_version: WEAPONRY_CURVE_EVALUATED_MESH_RECORD_SCHEMA.to_owned(),
            project_id: parent.project_id.clone(),
            curve_graph_lookup_key_sha256: parent.lookup_key_sha256.clone(),
            source_candidate_id: parent.source_candidate_id.clone(),
            source_candidate_state_sha256: parent.source_candidate_state_sha256.clone(),
            source_authoring_mesh_id: parent.source_authoring_mesh_id.clone(),
            source_authoring_mesh_lineage_id: parent.source_authoring_mesh_lineage_id.clone(),
            source_authoring_mesh_revision_id: parent.source_revision_id.clone(),
            source_authoring_mesh_revision_index: parent.source_authoring_mesh_revision_index,
            source_authoring_mesh_revision_sha256: parent.source_revision_sha256.clone(),
            source_authoring_mesh_identity_sha256: parent
                .source_authoring_mesh_identity_sha256
                .clone(),
            source_modifier_graph_id: parent.modifier_graph_id.clone(),
            source_modifier_graph_sha256: parent.modifier_graph_sha256.clone(),
            curve_set_semantic_sha256: parent.curve_set_sha256.clone(),
            curve_set_object_sha256: parent.curve_set_object_sha256.clone(),
            sample_set_semantic_sha256: parent.sample_set_sha256.clone(),
            sample_set_object_sha256: parent.sample_set_object_sha256.clone(),
            modifier_graph_semantic_sha256: parent.modifier_graph_sha256.clone(),
            modifier_graph_object_sha256: parent.modifier_graph_object_sha256.clone(),
            dependency_graph_semantic_sha256: parent.dependency_graph_sha256.clone(),
            dependency_graph_object_sha256: parent.dependency_graph_object_sha256.clone(),
            recompute_plan_semantic_sha256: parent.recompute_plan_sha256.clone(),
            recompute_plan_object_sha256: parent.recompute_plan_object_sha256.clone(),
            evaluation_id: "evaluation-r1".to_owned(),
            evaluation_plan_semantic_sha256: plan_semantic,
            evaluation_plan_object_sha256: plan_object.sha256.clone(),
            evaluated_mesh_id: "evaluated-mesh-r1".to_owned(),
            evaluated_mesh_semantic_sha256: mesh_semantic,
            evaluated_mesh_object_sha256: mesh_object.sha256.clone(),
            evaluated_mesh_identity_sha256: identity_semantic,
            evaluated_mesh_identity_object_sha256: identity_object.sha256.clone(),
            evaluated_mesh_link_sha256: link_semantic,
            evaluated_mesh_link_object_sha256: link_object.sha256.clone(),
            vertex_count: mesh_core.vertices.len() as u64,
            triangle_count: mesh_core.triangles.len() as u64,
            closed_two_manifold: true,
            zero_degenerate_triangles: true,
            evaluated_mesh_lookup_key_sha256: hash("evaluated-lookup"),
            idempotency_key: "evaluated-idem".to_owned(),
            input_sha256: hash("evaluated-input"),
            materialization_status: WEAPONRY_CURVE_EVALUATED_MESH_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: "1".to_owned(),
        };
        record.canonical_sha256 = canonical_record_sha256(&record).expect("record canonical");
        WeaponryCurveEvaluatedMeshCommit {
            record,
            cas: WeaponryCurveEvaluatedMeshCasBundle {
                evaluation_plan: plan_object,
                evaluated_mesh: mesh_object,
                evaluated_mesh_identity: identity_object,
                evaluated_mesh_link: link_object,
            },
        }
    }

    fn fixture(store: &Store) -> WeaponryCurveEvaluatedMeshCommit {
        project(store);
        let parent = structural(store);
        evaluated(store, &parent)
    }

    #[test]
    fn commit_read_replay_and_gc_roots_are_exact() {
        let store = Store::memory().expect("store");
        let commit = fixture(&store);
        let (stored, replayed) = store
            .record_weaponry_curve_evaluated_mesh_with_replay(&commit)
            .expect("commit");
        assert!(!replayed);
        assert_eq!(stored, commit.record);
        let (replayed_record, replayed) = store
            .record_weaponry_curve_evaluated_mesh_with_replay(&commit)
            .expect("replay");
        assert!(replayed);
        assert_eq!(replayed_record, stored);
        assert_eq!(
            store
                .get_weaponry_curve_evaluated_mesh(
                    "weaponry",
                    &commit.record.evaluated_mesh_lookup_key_sha256,
                )
                .expect("get"),
            Some(stored.clone())
        );
        for root in Store::weaponry_curve_evaluated_mesh_cas_roots(&stored) {
            assert_eq!(
                store
                    .get_object(&root)
                    .expect("object")
                    .expect("root")
                    .reachability,
                "reachable"
            );
        }
    }

    #[test]
    fn same_idempotency_key_conflict_does_not_replace_row() {
        let store = Store::memory().expect("store");
        let commit = fixture(&store);
        store
            .record_weaponry_curve_evaluated_mesh_with_replay(&commit)
            .expect("first");
        let mut conflict = commit.record.clone();
        conflict.input_sha256 = hash("different-input");
        conflict.canonical_sha256 = canonical_record_sha256(&conflict).expect("canonical");
        let error = store
            .record_weaponry_curve_evaluated_mesh_with_replay(&WeaponryCurveEvaluatedMeshCommit {
                record: conflict,
                cas: commit.cas,
            })
            .expect_err("conflict");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "WEAPONRY_CURVE_EVALUATED_MESH_IDEMPOTENCY_CONFLICT")
        );
    }

    #[test]
    fn missing_or_mismatched_cas_leaves_zero_rows() {
        let store = Store::memory().expect("store");
        let mut missing = fixture(&store);
        missing.record.evaluation_plan_object_sha256 = "f".repeat(64);
        missing.record.canonical_sha256 =
            canonical_record_sha256(&missing.record).expect("canonical");
        let error = store
            .record_weaponry_curve_evaluated_mesh_with_replay(&missing)
            .expect_err("missing binding");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "WEAPONRY_CURVE_EVALUATED_MESH_CAS_BINDING_MISMATCH")
        );
        assert_eq!(
            store
                .get_weaponry_curve_evaluated_mesh("weaponry", &hash("evaluated-lookup"))
                .expect("get"),
            None
        );

        let mut mismatch = missing;
        mismatch.record.evaluation_plan_object_sha256 = mismatch.cas.evaluation_plan.sha256.clone();
        mismatch.record.canonical_sha256 =
            canonical_record_sha256(&mismatch.record).expect("canonical");
        mismatch.cas.evaluation_plan.size_bytes += 1;
        let error = store
            .record_weaponry_curve_evaluated_mesh_with_replay(&mismatch)
            .expect_err("metadata mismatch");
        assert!(
            matches!(error, StoreError::Contract { code, .. } if code == "WEAPONRY_CURVE_EVALUATED_MESH_CAS_METADATA_MISMATCH")
        );
        assert_eq!(
            store
                .get_weaponry_curve_evaluated_mesh("weaponry", &hash("evaluated-lookup"))
                .expect("get"),
            None
        );
    }

    #[test]
    fn file_drop_reopen_get_preserves_binding() {
        let root =
            std::env::temp_dir().join(format!("forgecad-evaluated-mesh-{}", uuid::Uuid::new_v4()));
        let db = root.join("runtime.sqlite");
        let cas = root.join("cas");
        fs::create_dir_all(&root).expect("root");
        let commit = {
            let store = Store::open_with_cas(&db, &cas).expect("open");
            let commit = fixture(&store);
            store
                .record_weaponry_curve_evaluated_mesh_with_replay(&commit)
                .expect("commit");
            commit
        };
        let reopened = Store::open_with_cas(&db, &cas).expect("reopen");
        assert_eq!(
            reopened
                .get_weaponry_curve_evaluated_mesh(
                    "weaponry",
                    &commit.record.evaluated_mesh_lookup_key_sha256
                )
                .expect("get"),
            Some(commit.record)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gc_linkage_marks_each_evaluated_mesh_root() {
        let store = Store::memory().expect("store");
        let commit = fixture(&store);
        store
            .record_weaponry_curve_evaluated_mesh_with_replay(&commit)
            .expect("commit");
        let mut connection = store.lock_connection().expect("connection");
        let transaction = connection.transaction().expect("transaction");
        for root in Store::weaponry_curve_evaluated_mesh_cas_roots(&commit.record) {
            assert!(
                super::super::authoring_mesh_edit_object_is_linked(&transaction, &root)
                    .expect("linked")
            );
        }
        transaction.commit().expect("commit transaction");
    }

    #[test]
    fn cas_object_sha256_is_content_addressed() {
        let store = Store::memory().expect("store");
        let commit = fixture(&store);
        let bytes = store
            .cas()
            .read_verified(&commit.cas.evaluated_mesh.sha256)
            .expect("read");
        assert_eq!(sha256_hex(&bytes), commit.cas.evaluated_mesh.sha256);
        let _: PathBuf = store.cas().root().to_path_buf();
    }
}
