//! Atomic Store binding for the distinct derived High candidate.
//!
//! NativeHighDurable remains owned by the Stage/source candidate.  This
//! module creates the separate prepared High candidate and its formal
//! `ProductionWeaponHighArtifact@1` row in one SQLite transaction.  It never
//! advances ProductionStage@3, confirms a candidate, creates a version or
//! exports an artifact.

use forgecad_contracts::{
    CandidateRecord, CasObjectRecord, PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND,
    PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY, PRODUCTION_WEAPON_HIGH_ARTIFACT_SCHEMA_VERSION,
    ProductionWeaponHighArtifactRecord, is_opaque_id, is_sha256,
};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use super::{Store, StoreError};

const TABLE: &str = "production_weapon_formal_high_links";
const JSON_MIME: &str = "application/json";
const GLB_MIME: &str = "model/gltf-binary";
const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ProductionWeaponFormalHighCommitBundle {
    pub candidate: CandidateRecord,
    pub high: ProductionWeaponHighArtifactRecord,
    pub idempotency_key: String,
    pub high_artifact_object: CasObjectRecord,
    pub high_artifact_readback_object: CasObjectRecord,
    pub high_geometry_program_object: CasObjectRecord,
    pub high_detail_graph_object: CasObjectRecord,
    pub receipt_object: CasObjectRecord,
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

pub(crate) fn ensure_table(connection: &rusqlite::Connection) -> Result<(), StoreError> {
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
            high_artifact_id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            source_stage_head_transition_id TEXT NOT NULL,
            source_candidate_id TEXT NOT NULL,
            high_candidate_id TEXT NOT NULL UNIQUE,
            high_candidate_state_sha256 TEXT NOT NULL,
            high_artifact_sha256 TEXT NOT NULL UNIQUE,
            high_artifact_readback_object_sha256 TEXT NOT NULL,
            receipt_object_sha256 TEXT NOT NULL UNIQUE,
            idempotency_key TEXT NOT NULL,
            canonical_sha256 TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE (project_id, session_id, idempotency_key)
        );
        CREATE INDEX IF NOT EXISTS production_weapon_formal_high_scope_idx
            ON {TABLE}(project_id, session_id, source_candidate_id, created_at DESC);"
    ))?;
    let has_idempotency_key: Option<String> = connection
        .query_row(
            "SELECT name FROM pragma_table_info(?1) WHERE name = 'idempotency_key'",
            params![TABLE],
            |row| row.get(0),
        )
        .optional()?;
    if has_idempotency_key.is_none() {
        // This table first existed on the unshipped Formal High source path.
        // Preserve any developer-cohort rows instead of dropping them: older
        // rows receive a deterministic, artifact-scoped legacy key, while all
        // new public writes carry the caller's exact key through Runtime.
        connection.execute(
            &format!("ALTER TABLE {TABLE} ADD COLUMN idempotency_key TEXT"),
            [],
        )?;
        connection.execute(
            &format!(
                "UPDATE {TABLE} SET idempotency_key = 'legacy:' || high_artifact_id \
                 WHERE idempotency_key IS NULL OR idempotency_key = ''"
            ),
            [],
        )?;
    }
    connection.execute_batch(&format!(
        "CREATE UNIQUE INDEX IF NOT EXISTS production_weapon_formal_high_idempotency_idx
            ON {TABLE}(project_id, session_id, idempotency_key);"
    ))?;
    Ok(())
}

fn candidate_hash(candidate: &CandidateRecord) -> Result<String, StoreError> {
    let mut value = serde_json::to_value(candidate)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn same_candidate(left: &CandidateRecord, right: &CandidateRecord) -> bool {
    let left = serde_json::to_value(left)
        .ok()
        .and_then(|value| canonical_json_bytes(&value).ok());
    let right = serde_json::to_value(right)
        .ok()
        .and_then(|value| canonical_json_bytes(&value).ok());
    left.is_some() && left == right
}

fn validate_candidate_shape(candidate: &CandidateRecord) -> Result<(), StoreError> {
    super::validate_candidate(candidate)?;
    if candidate.schema_version != "Candidate@1"
        || candidate.state != "prepared"
        || candidate.prepared_object_id.is_none()
        || candidate.prepared_object_sha256.is_none()
        || candidate.quality_report_id.is_some()
        || candidate.quality_hard_gate_passed
        || candidate.error_code.is_some()
        || candidate_hash(candidate)? != candidate.canonical_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_CANDIDATE_INVALID",
            "derived High candidate is not the closed prepared candidate",
        ));
    }
    Ok(())
}

fn payload_value(high: &ProductionWeaponHighArtifactRecord) -> Result<Value, StoreError> {
    let value = super::production_weapon_high_low_payload_value(high, "formal high")?;
    if high.schema_version != PRODUCTION_WEAPON_HIGH_ARTIFACT_SCHEMA_VERSION
        || high.high_policy != PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY
        || high.high_policy_sha256 != sha256_hex(PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY.as_bytes())
        || high.high_artifact_kind != PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND
        || high.high_mime != GLB_MIME
        || high.high_size_bytes == 0
        || high.high_size_bytes > MAX_GLB_BYTES
        || high.high_worker_replay_count != 2
        || !high.high_replay_byte_exact
        || !high.hard_gate_passed
        || high.structural_status != "PASS_SOURCE_STRUCTURAL"
        || high.visual_status != "NOT_RUN"
        || high.human_status != "NOT_RUN"
        || high.engine_status != "NOT_RUN"
        || high.distribution_status != "NOT_RUN"
        || high.high_part_ids.is_empty()
        || high.high_material_zone_ids.is_empty()
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_RECORD_INVALID",
            "formal High record does not satisfy the structural-only contract",
        ));
    }
    for value in high
        .high_part_ids
        .iter()
        .chain(high.high_material_zone_ids.iter())
    {
        if !is_opaque_id(value) {
            return Err(contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_RECORD_INVALID",
                "formal High inventory contains an invalid identity",
            ));
        }
    }
    Ok(value)
}

fn payload_json(value: &Value) -> Result<String, StoreError> {
    super::production_weapon_high_low_payload_json(value)
}

fn validate_idempotency_key(idempotency_key: &str) -> Result<(), StoreError> {
    if !is_opaque_id(idempotency_key) {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_IDEMPOTENCY_KEY_INVALID",
            "formal High idempotency key is not an opaque identity",
        ));
    }
    Ok(())
}

fn validate_binding(
    candidate: &CandidateRecord,
    high: &ProductionWeaponHighArtifactRecord,
) -> Result<(), StoreError> {
    if candidate.project_id != high.project_id
        || candidate.candidate_id != high.high_candidate_id
        || candidate.canonical_sha256 != high.high_candidate_state_sha256
        || candidate.prepared_object_id.as_deref() != Some(high.high_artifact_id.as_str())
        || candidate.prepared_object_sha256.as_deref() != Some(high.high_artifact_sha256.as_str())
        || candidate.request_sha256 != high.request_sha256
        || candidate.candidate_id == high.source_candidate_id
        || high.source_artifact_sha256 == high.high_artifact_sha256
        || high.source_stage_head_stage != "secondary-form-approved"
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_BINDING_MISMATCH",
            "derived candidate, source lineage and formal High record differ",
        ));
    }
    Ok(())
}

fn string_array(value: &Value, field: &str) -> Result<Vec<String>, StoreError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_NATIVE_PAYLOAD_INVALID",
                format!("HighMeshArtifact.{field} is missing"),
            )
        })?
        .iter()
        .map(|item| {
            item.as_str().map(str::to_owned).ok_or_else(|| {
                contract(
                    "PRODUCTION_WEAPON_FORMAL_HIGH_NATIVE_PAYLOAD_INVALID",
                    format!("HighMeshArtifact.{field} contains a non-string"),
                )
            })
        })
        .collect()
}

fn validate_durable_source(
    store: &Store,
    high: &ProductionWeaponHighArtifactRecord,
) -> Result<(), StoreError> {
    let transition = store
        .get_production_stage_transition_v3(&high.source_stage_head_transition_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_UNAVAILABLE",
                "source ProductionStage@3 transition is missing",
            )
        })?;
    let head = store
        .get_production_stage_head_v3(
            &high.session_id,
            &high.project_id,
            &transition.root_candidate_id,
        )?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_UNAVAILABLE",
                "source ProductionStage@3 head is missing",
            )
        })?;
    if transition.session_id != high.session_id
        || transition.project_id != high.project_id
        || transition.canonical_sha256 != high.source_stage_head_transition_sha256
        || transition.head_candidate_id != high.source_candidate_id
        || transition.head_candidate_state_sha256 != high.source_candidate_state_sha256
        || transition.output_artifact_id != high.source_artifact_id
        || transition.head_artifact_sha256 != high.source_artifact_sha256
        || transition.to_stage != high.source_stage_head_stage
        || head.head_transition_id != transition.transition_id
        || head.head_transition_sha256 != transition.canonical_sha256
        || head.canonical_sha256 != high.source_stage_head_canonical_sha256
        || head.head_candidate_id != high.source_candidate_id
        || head.head_candidate_state_sha256 != high.source_candidate_state_sha256
        || head.output_artifact_id != high.source_artifact_id
        || head.head_artifact_sha256 != high.source_artifact_sha256
        || head.head_stage != high.source_stage_head_stage
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_LINEAGE_MISMATCH",
            "source transition/head differ from the formal High record",
        ));
    }

    let native = store
        .get_native_high_durable_by_candidate(&high.source_candidate_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_NATIVE_HIGH_UNAVAILABLE",
                "source candidate has no durable Native High",
            )
        })?;
    let authoring = store
        .get_authoring_mesh_durable_record_by_mesh(
            &high.source_candidate_id,
            &native.source_canonical_mesh_id,
        )?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_AUTHORING_MESH_UNAVAILABLE",
                "source Native High has no durable AuthoringMesh",
            )
        })?;
    let evidence = store
        .get_geometry_candidate_evidence(&high.source_candidate_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_GEOMETRY_EVIDENCE_UNAVAILABLE",
                "source candidate has no geometry evidence",
            )
        })?;
    if native.project_id != high.project_id
        || native.candidate_state_sha256 != high.source_candidate_state_sha256
        || native.high_artifact_id != high.high_artifact_id
        || native.high_artifact_object_sha256 != high.high_artifact_sha256
        || native.high_artifact_sha256 != high.high_artifact_sha256
        || native.high_artifact_size_bytes != high.high_size_bytes
        || native.detail_graph_object_sha256 != high.high_detail_graph_object_sha256
        || native.detail_graph_canonical_sha256 != high.high_detail_graph_canonical_sha256
        || native.high_worker_build_cohort_sha256 != high.high_worker_build_cohort_sha256
        || authoring.project_id != high.project_id
        || authoring.candidate_state_sha256 != high.source_candidate_state_sha256
        || authoring.source_artifact_object_sha256 != high.source_artifact_sha256
        || authoring.source_artifact_sha256 != high.source_artifact_sha256
        || authoring.source_artifact_readback_sha256 != high.source_artifact_readback_sha256
        || authoring.source_program_object_sha256 != high.high_geometry_program_object_sha256
        || authoring.source_program_sha256 != high.high_geometry_program_sha256
        || evidence.project_id != high.project_id
        || evidence.geometry_program_object_sha256 != high.high_geometry_program_object_sha256
        || evidence.geometry_program_sha256 != high.high_geometry_program_sha256
        || evidence.canonical_sha256 != high.high_geometry_candidate_evidence_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_DURABLE_SOURCE_MISMATCH",
            "Native High, AuthoringMesh or geometry evidence differs",
        ));
    }

    let formal_readback_bytes = store
        .cas
        .read_verified_bounded(&high.high_artifact_readback_object_sha256, MAX_JSON_BYTES)?;
    let formal_readback: Value =
        serde_json::from_slice(&formal_readback_bytes).map_err(|error| {
            contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_READBACK_INVALID",
                error.to_string(),
            )
        })?;
    if formal_readback
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("ProductionWeaponHighArtifactReadback@1")
        || formal_readback
            .get("native_readback_object_sha256")
            .and_then(Value::as_str)
            != Some(native.high_artifact_readback_object_sha256.as_str())
        || formal_readback
            .get("native_readback_sha256")
            .and_then(Value::as_str)
            != Some(native.high_artifact_readback_sha256.as_str())
        || formal_readback
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(high.high_artifact_readback_sha256.as_str())
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_READBACK_MISMATCH",
            "formal readback does not bind the Native High readback",
        ));
    }

    let high_mesh_bytes = store
        .cas
        .read_verified_bounded(&native.high_mesh_artifact_object_sha256, MAX_JSON_BYTES)?;
    let high_mesh: Value = serde_json::from_slice(&high_mesh_bytes).map_err(|error| {
        contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_NATIVE_PAYLOAD_INVALID",
            error.to_string(),
        )
    })?;
    let part_ids = string_array(&high_mesh, "part_ids")?;
    let material_zone_ids = string_array(&high_mesh, "material_zone_ids")?;
    let inventory_sha256 = canonical_json_hash(&serde_json::json!({
        "part_ids":part_ids,
        "material_zone_ids":material_zone_ids
    }));
    if high_mesh.get("schema_version").and_then(Value::as_str) != Some("HighMeshArtifact@1")
        || high_mesh.get("artifact_sha256").and_then(Value::as_str)
            != Some(native.high_mesh_artifact_sha256.as_str())
        || high_mesh
            .get("high_worker_algorithm_sha256")
            .and_then(Value::as_str)
            != Some(high.high_worker_algorithm_sha256.as_str())
        || high_mesh
            .get("high_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(high.high_worker_build_cohort_sha256.as_str())
        || high_mesh
            .get("high_topology_status")
            .and_then(Value::as_str)
            != Some(high.high_topology_status.as_str())
        || high_mesh
            .get("high_authoring_topology_status")
            .and_then(Value::as_str)
            != Some("source-preserved")
        || high.high_authoring_topology_status != "partial"
        || high_mesh.get("uv_status").and_then(Value::as_str) != Some(high.high_uv_status.as_str())
        || high_mesh.get("tangent_status").and_then(Value::as_str)
            != Some(high.high_tangent_status.as_str())
        || part_ids != high.high_part_ids
        || material_zone_ids != high.high_material_zone_ids
        || inventory_sha256 != high.high_part_inventory_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_NATIVE_PAYLOAD_MISMATCH",
            "HighMeshArtifact inventory, topology, algorithm or cohort differs",
        ));
    }
    Ok(())
}

fn validate_object(
    store: &Store,
    object: &CasObjectRecord,
    expected_hash: &str,
    expected_mime: &str,
    expected_kind: Option<&str>,
    max_bytes: u64,
    require_reachable: bool,
) -> Result<(), StoreError> {
    if object.schema_version != "CasObject@1"
        || object.sha256 != expected_hash
        || !is_sha256(expected_hash)
        || object.mime != expected_mime
        || expected_kind.is_some_and(|kind| object.kind != kind)
        || object.size_bytes == 0
        || object.size_bytes > max_bytes
        || if require_reachable {
            object.reachability != "reachable"
        } else {
            !matches!(object.reachability.as_str(), "temporary" | "reachable")
        }
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_CAS_METADATA_INVALID",
            "formal High CAS metadata differs from its closed role",
        ));
    }
    let registered = store.get_object(expected_hash)?.ok_or_else(|| {
        contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_CAS_MISSING",
            "formal High CAS root is not registered",
        )
    })?;
    if registered.size_bytes != object.size_bytes
        || registered.mime != object.mime
        || registered.kind != object.kind
        || (require_reachable && registered.reachability != "reachable")
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_CAS_METADATA_INVALID",
            "registered formal High CAS metadata differs",
        ));
    }
    store.cas.verify(expected_hash, object.size_bytes)?;
    Ok(())
}

fn validate_objects(
    store: &Store,
    bundle: &ProductionWeaponFormalHighCommitBundle,
    payload: &str,
    require_reachable: bool,
) -> Result<(), StoreError> {
    let high = &bundle.high;
    let source_artifact = object_record(store, &high.source_artifact_sha256)?;
    validate_object(
        store,
        &source_artifact,
        &high.source_artifact_sha256,
        GLB_MIME,
        None,
        MAX_GLB_BYTES,
        true,
    )?;
    validate_object(
        store,
        &bundle.high_artifact_object,
        &high.high_artifact_sha256,
        GLB_MIME,
        Some(PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND),
        MAX_GLB_BYTES,
        true,
    )?;
    if bundle.high_artifact_object.size_bytes != high.high_size_bytes {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_GLB_SIZE_MISMATCH",
            "formal High GLB byte length differs from CAS metadata",
        ));
    }
    validate_object(
        store,
        &bundle.high_artifact_readback_object,
        &high.high_artifact_readback_object_sha256,
        JSON_MIME,
        Some(super::PRODUCTION_WEAPON_HIGH_ARTIFACT_RECEIPT_KIND),
        MAX_JSON_BYTES,
        require_reachable,
    )?;
    validate_object(
        store,
        &bundle.high_geometry_program_object,
        &high.high_geometry_program_object_sha256,
        JSON_MIME,
        None,
        MAX_JSON_BYTES,
        true,
    )?;
    validate_object(
        store,
        &bundle.high_detail_graph_object,
        &high.high_detail_graph_object_sha256,
        JSON_MIME,
        Some(super::NATIVE_HIGH_DETAIL_GRAPH_OBJECT_KIND),
        MAX_JSON_BYTES,
        true,
    )?;
    validate_object(
        store,
        &bundle.receipt_object,
        &high.receipt_object_sha256,
        JSON_MIME,
        Some(super::PRODUCTION_WEAPON_HIGH_ARTIFACT_RECEIPT_KIND),
        MAX_JSON_BYTES,
        require_reachable,
    )?;
    super::high_low_payload_matches_object(
        &store.cas,
        &bundle.receipt_object,
        payload,
        "formal high",
    )?;
    super::high_low_canonical_json_object_matches(
        &store.cas,
        &bundle.high_artifact_readback_object,
        &high.high_artifact_readback_sha256,
        "formal High readback",
    )?;
    super::high_low_canonical_json_object_matches(
        &store.cas,
        &bundle.high_geometry_program_object,
        &high.high_geometry_program_sha256,
        "formal High geometry program",
    )?;
    super::high_low_canonical_json_object_matches(
        &store.cas,
        &bundle.high_detail_graph_object,
        &high.high_detail_graph_canonical_sha256,
        "formal High detail graph",
    )?;
    Ok(())
}

fn read_candidate(
    transaction: &Transaction<'_>,
    candidate_id: &str,
) -> Result<Option<CandidateRecord>, StoreError> {
    Ok(transaction
        .query_row(
            "SELECT candidate_id, project_id, base_version_id, source_version_id, prepared_object_id, prepared_object_sha256, state, request_sha256, manifest_hash, quality_report_id, quality_hard_gate_passed, canonical_sha256, error_code, created_at, updated_at FROM candidates WHERE candidate_id = ?1",
            params![candidate_id],
            |row| {
                Ok(CandidateRecord {
                    schema_version: "Candidate@1".to_owned(),
                    candidate_id: row.get(0)?,
                    project_id: row.get(1)?,
                    base_version_id: row.get(2)?,
                    source_version_id: row.get(3)?,
                    prepared_object_id: row.get(4)?,
                    prepared_object_sha256: row.get(5)?,
                    state: row.get(6)?,
                    request_sha256: row.get(7)?,
                    manifest_hash: row.get(8)?,
                    quality_report_id: row.get(9)?,
                    quality_hard_gate_passed: row.get::<_, i64>(10)? != 0,
                    canonical_sha256: row.get(11)?,
                    error_code: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            },
        )
        .optional()?)
}

fn read_high_with_idempotency(
    transaction: &Transaction<'_>,
    high_artifact_id: &str,
) -> Result<Option<(String, ProductionWeaponHighArtifactRecord)>, StoreError> {
    let row: Option<(String, String)> = transaction
        .query_row(
            &format!(
                "SELECT idempotency_key, payload_json FROM {TABLE} WHERE high_artifact_id = ?1"
            ),
            params![high_artifact_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    row.map(|(idempotency_key, payload)| {
        validate_idempotency_key(&idempotency_key)?;
        let high = serde_json::from_str(&payload).map_err(|error| {
            contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_PAYLOAD_INVALID",
                error.to_string(),
            )
        })?;
        Ok((idempotency_key, high))
    })
    .transpose()
}

fn read_high_by_scope_idempotency(
    transaction: &Transaction<'_>,
    project_id: &str,
    session_id: &str,
    idempotency_key: &str,
) -> Result<Option<(String, ProductionWeaponHighArtifactRecord)>, StoreError> {
    let high_artifact_id: Option<String> = transaction
        .query_row(
            &format!(
                "SELECT high_artifact_id FROM {TABLE} WHERE project_id = ?1 AND session_id = ?2 AND idempotency_key = ?3"
            ),
            params![project_id, session_id, idempotency_key],
            |row| row.get(0),
        )
        .optional()?;
    let Some(high_artifact_id) = high_artifact_id else {
        return Ok(None);
    };
    read_high_with_idempotency(transaction, &high_artifact_id)
}

fn validate_source_lineage(
    transaction: &Transaction<'_>,
    high: &ProductionWeaponHighArtifactRecord,
) -> Result<(), StoreError> {
    let source = read_candidate(transaction, &high.source_candidate_id)?.ok_or_else(|| {
        contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_UNAVAILABLE",
            "formal High source candidate is missing",
        )
    })?;
    if source.project_id != high.project_id
        || source.canonical_sha256 != high.source_candidate_state_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_MISMATCH",
            "formal High source candidate binding differs",
        ));
    }
    let transition_match: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM agentic_production_stage_transitions_v3 WHERE transition_id = ?1 AND session_id = ?2 AND project_id = ?3 AND head_candidate_id = ?4 AND head_candidate_state_sha256 = ?5 AND output_artifact_id = ?6 AND head_artifact_sha256 = ?7 AND to_stage = ?8 AND canonical_sha256 = ?9",
        params![high.source_stage_head_transition_id, high.session_id, high.project_id, high.source_candidate_id, high.source_candidate_state_sha256, high.source_artifact_id, high.source_artifact_sha256, high.source_stage_head_stage, high.source_stage_head_transition_sha256],
        |row| row.get(0),
    )?;
    let head_match: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM agentic_production_stage_heads_v3 WHERE session_id = ?1 AND project_id = ?2 AND head_candidate_id = ?3 AND head_candidate_state_sha256 = ?4 AND output_artifact_id = ?5 AND head_artifact_sha256 = ?6 AND head_stage = ?7 AND head_transition_id = ?8 AND canonical_sha256 = ?9",
        params![high.session_id, high.project_id, high.source_candidate_id, high.source_candidate_state_sha256, high.source_artifact_id, high.source_artifact_sha256, high.source_stage_head_stage, high.source_stage_head_transition_id, high.source_stage_head_canonical_sha256],
        |row| row.get(0),
    )?;
    let native_high_match: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM native_high_durable_links WHERE project_id = ?1 AND candidate_id = ?2 AND candidate_state_sha256 = ?3 AND high_artifact_id = ?4 AND high_artifact_object_sha256 = ?5 AND high_artifact_sha256 = ?5 AND detail_graph_object_sha256 = ?6 AND detail_graph_canonical_sha256 = ?7",
        params![high.project_id, high.source_candidate_id, high.source_candidate_state_sha256, high.high_artifact_id, high.high_artifact_sha256, high.high_detail_graph_object_sha256, high.high_detail_graph_canonical_sha256],
        |row| row.get(0),
    )?;
    if transition_match != 1 || head_match != 1 || native_high_match != 1 {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_LINEAGE_MISMATCH",
            "Stage head, Native High and source candidate do not form one exact lineage",
        ));
    }
    Ok(())
}

fn insert_candidate(
    transaction: &Transaction<'_>,
    candidate: &CandidateRecord,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO candidates (candidate_id, project_id, base_version_id, source_version_id, prepared_object_id, prepared_object_sha256, state, request_sha256, manifest_hash, quality_report_id, quality_hard_gate_passed, canonical_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![candidate.candidate_id, candidate.project_id, candidate.base_version_id, candidate.source_version_id, candidate.prepared_object_id, candidate.prepared_object_sha256, candidate.state, candidate.request_sha256, candidate.manifest_hash, candidate.quality_report_id, candidate.quality_hard_gate_passed, candidate.canonical_sha256, candidate.error_code, candidate.created_at, candidate.updated_at],
    )?;
    Ok(())
}

fn object_record(store: &Store, hash: &str) -> Result<CasObjectRecord, StoreError> {
    store.get_object(hash)?.ok_or_else(|| {
        contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_CAS_MISSING",
            format!("formal High CAS root {hash} is missing"),
        )
    })
}

fn commit_validated_formal_high_rows(
    transaction: &Transaction<'_>,
    bundle: &ProductionWeaponFormalHighCommitBundle,
    payload: &str,
) -> Result<(ProductionWeaponHighArtifactRecord, bool), StoreError> {
    validate_idempotency_key(&bundle.idempotency_key)?;

    let existing_by_artifact =
        read_high_with_idempotency(transaction, &bundle.high.high_artifact_id)?;
    let existing_by_scope = read_high_by_scope_idempotency(
        transaction,
        &bundle.high.project_id,
        &bundle.high.session_id,
        &bundle.idempotency_key,
    )?;

    if let Some((existing_idempotency_key, existing)) = existing_by_artifact {
        let existing_candidate = read_candidate(transaction, &existing.high_candidate_id)?
            .ok_or_else(|| {
                contract(
                    "PRODUCTION_WEAPON_FORMAL_HIGH_RESTART_READBACK_FAILED",
                    "formal High derived candidate disappeared",
                )
            })?;
        if existing_idempotency_key != bundle.idempotency_key
            || existing != bundle.high
            || !same_candidate(&existing_candidate, &bundle.candidate)
        {
            return Err(contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_REPLAY_CONFLICT",
                "formal High identity is already bound to different bytes",
            ));
        }
        super::mark_reachable_in_transaction(
            transaction,
            &[
                bundle.high.high_artifact_readback_object_sha256.clone(),
                bundle.high.receipt_object_sha256.clone(),
            ],
        )?;
        return Ok((existing, true));
    }

    if let Some((existing_idempotency_key, existing)) = existing_by_scope {
        let existing_candidate = read_candidate(transaction, &existing.high_candidate_id)?
            .ok_or_else(|| {
                contract(
                    "PRODUCTION_WEAPON_FORMAL_HIGH_RESTART_READBACK_FAILED",
                    "formal High derived candidate disappeared",
                )
            })?;
        if existing_idempotency_key != bundle.idempotency_key
            || existing != bundle.high
            || !same_candidate(&existing_candidate, &bundle.candidate)
        {
            return Err(contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_REPLAY_CONFLICT",
                "project/session idempotency key is already bound to different bytes",
            ));
        }
        super::mark_reachable_in_transaction(
            transaction,
            &[
                bundle.high.high_artifact_readback_object_sha256.clone(),
                bundle.high.receipt_object_sha256.clone(),
            ],
        )?;
        return Ok((existing, true));
    }

    if read_candidate(transaction, &bundle.candidate.candidate_id)?.is_some() {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_ORPHAN_CANDIDATE_CONFLICT",
            "derived High candidate exists without the exact formal High link",
        ));
    }
    insert_candidate(transaction, &bundle.candidate)?;
    transaction.execute(
        &format!("INSERT INTO {TABLE} (high_artifact_id, session_id, project_id, source_stage_head_transition_id, source_candidate_id, high_candidate_id, high_candidate_state_sha256, high_artifact_sha256, high_artifact_readback_object_sha256, receipt_object_sha256, idempotency_key, canonical_sha256, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"),
        params![bundle.high.high_artifact_id, bundle.high.session_id, bundle.high.project_id, bundle.high.source_stage_head_transition_id, bundle.high.source_candidate_id, bundle.high.high_candidate_id, bundle.high.high_candidate_state_sha256, bundle.high.high_artifact_sha256, bundle.high.high_artifact_readback_object_sha256, bundle.high.receipt_object_sha256, bundle.idempotency_key, bundle.high.canonical_sha256, payload, bundle.high.created_at],
    )?;
    super::mark_reachable_in_transaction(
        transaction,
        &[
            bundle.high.high_artifact_readback_object_sha256.clone(),
            bundle.high.receipt_object_sha256.clone(),
        ],
    )?;
    let (stored_idempotency_key, stored) =
        read_high_with_idempotency(transaction, &bundle.high.high_artifact_id)?.ok_or_else(
            || {
                contract(
                    "PRODUCTION_WEAPON_FORMAL_HIGH_RESTART_READBACK_FAILED",
                    "formal High row disappeared before commit",
                )
            },
        )?;
    let stored_candidate = read_candidate(transaction, &bundle.candidate.candidate_id)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORMAL_HIGH_RESTART_READBACK_FAILED",
                "derived High candidate disappeared before commit",
            )
        })?;
    if stored_idempotency_key != bundle.idempotency_key
        || stored != bundle.high
        || !same_candidate(&stored_candidate, &bundle.candidate)
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORMAL_HIGH_RESTART_READBACK_FAILED",
            "formal High write-after-readback differs",
        ));
    }
    Ok((stored, false))
}

fn record_production_weapon_formal_high_after_validation(
    store: &Store,
    bundle: &ProductionWeaponFormalHighCommitBundle,
    payload: &str,
) -> Result<(ProductionWeaponHighArtifactRecord, bool), StoreError> {
    let mut connection = store.lock_connection()?;
    ensure_table(&connection)?;
    let transaction = connection.transaction()?;
    validate_source_lineage(&transaction, &bundle.high)?;
    let result = commit_validated_formal_high_rows(&transaction, bundle, payload)?;
    transaction.commit()?;
    Ok(result)
}

impl Store {
    pub fn record_production_weapon_formal_high_with_replay(
        &self,
        bundle: &ProductionWeaponFormalHighCommitBundle,
    ) -> Result<(ProductionWeaponHighArtifactRecord, bool), StoreError> {
        validate_idempotency_key(&bundle.idempotency_key)?;
        validate_candidate_shape(&bundle.candidate)?;
        let value = payload_value(&bundle.high)?;
        validate_binding(&bundle.candidate, &bundle.high)?;
        validate_durable_source(self, &bundle.high)?;
        let payload = payload_json(&value)?;
        validate_objects(self, bundle, &payload, false)?;
        record_production_weapon_formal_high_after_validation(self, bundle, &payload)
    }

    pub fn get_production_weapon_formal_high(
        &self,
        project_id: &str,
        session_id: &str,
        high_artifact_id: &str,
    ) -> Result<Option<ProductionWeaponHighArtifactRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(session_id) || !is_opaque_id(high_artifact_id)
        {
            return Err(StoreError::InvalidData(
                "formal High lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let stored = {
            let transaction = connection.unchecked_transaction()?;
            let high = read_high_with_idempotency(&transaction, high_artifact_id)?;
            transaction.commit()?;
            high
        };
        drop(connection);
        let Some((idempotency_key, high)) = stored else {
            return Ok(None);
        };
        if high.project_id != project_id || high.session_id != session_id {
            return Ok(None);
        }
        let candidate = self
            .get_candidate(&high.high_candidate_id)?
            .ok_or_else(|| {
                contract(
                    "PRODUCTION_WEAPON_FORMAL_HIGH_RESTART_READBACK_FAILED",
                    "formal High derived candidate is missing",
                )
            })?;
        validate_candidate_shape(&candidate)?;
        payload_value(&high)?;
        validate_binding(&candidate, &high)?;
        validate_durable_source(self, &high)?;
        let bundle = ProductionWeaponFormalHighCommitBundle {
            candidate,
            high: high.clone(),
            idempotency_key,
            high_artifact_object: object_record(self, &high.high_artifact_sha256)?,
            high_artifact_readback_object: object_record(
                self,
                &high.high_artifact_readback_object_sha256,
            )?,
            high_geometry_program_object: object_record(
                self,
                &high.high_geometry_program_object_sha256,
            )?,
            high_detail_graph_object: object_record(self, &high.high_detail_graph_object_sha256)?,
            receipt_object: object_record(self, &high.receipt_object_sha256)?,
        };
        let value = payload_value(&high)?;
        validate_objects(self, &bundle, &payload_json(&value)?, true)?;
        Ok(Some(high))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::ProjectRecord;
    use uuid::Uuid;

    fn hash(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn row_bundle(store: &Store) -> ProductionWeaponFormalHighCommitBundle {
        let readback = store
            .put_object(
                br#"{"schema_version":"ProductionWeaponHighArtifactReadback@1"}"#,
                None,
                JSON_MIME,
                super::super::PRODUCTION_WEAPON_HIGH_ARTIFACT_RECEIPT_KIND,
                "1",
            )
            .expect("readback object");
        let receipt = store
            .put_object(
                br#"{"schema_version":"ProductionWeaponHighArtifact@1"}"#,
                None,
                JSON_MIME,
                super::super::PRODUCTION_WEAPON_HIGH_ARTIFACT_RECEIPT_KIND,
                "1",
            )
            .expect("receipt object");
        let mut candidate = CandidateRecord {
            schema_version: "Candidate@1".to_owned(),
            candidate_id: "candidate-formal-high".to_owned(),
            project_id: "project-formal-high".to_owned(),
            base_version_id: None,
            source_version_id: None,
            prepared_object_id: Some("artifact-formal-high".to_owned()),
            prepared_object_sha256: Some(hash('f')),
            state: "prepared".to_owned(),
            request_sha256: hash('a'),
            manifest_hash: None,
            quality_report_id: None,
            quality_hard_gate_passed: false,
            canonical_sha256: String::new(),
            error_code: None,
            created_at: "1".to_owned(),
            updated_at: "1".to_owned(),
        };
        candidate.canonical_sha256 = candidate_hash(&candidate).expect("candidate hash");
        let high = ProductionWeaponHighArtifactRecord {
            schema_version: PRODUCTION_WEAPON_HIGH_ARTIFACT_SCHEMA_VERSION.to_owned(),
            high_artifact_id: "artifact-formal-high".to_owned(),
            session_id: "session-formal-high".to_owned(),
            project_id: candidate.project_id.clone(),
            source_stage_head_transition_id: "transition-secondary".to_owned(),
            source_stage_head_transition_sha256: hash('b'),
            source_stage_head_canonical_sha256: hash('c'),
            source_stage_head_stage: "secondary-form-approved".to_owned(),
            source_candidate_id: "candidate-source".to_owned(),
            source_candidate_state_sha256: hash('d'),
            source_artifact_id: "artifact-source".to_owned(),
            source_artifact_sha256: hash('e'),
            source_artifact_readback_sha256: hash('1'),
            high_candidate_id: candidate.candidate_id.clone(),
            high_candidate_state_sha256: candidate.canonical_sha256.clone(),
            high_artifact_sha256: hash('f'),
            high_artifact_readback_sha256: hash('2'),
            high_artifact_readback_object_sha256: readback.record.sha256.clone(),
            high_geometry_program_sha256: hash('3'),
            high_geometry_program_object_sha256: hash('4'),
            high_geometry_candidate_evidence_sha256: hash('5'),
            high_detail_graph_object_sha256: hash('6'),
            high_detail_graph_canonical_sha256: hash('7'),
            high_part_inventory_sha256: hash('8'),
            high_part_ids: vec!["receiver".to_owned(), "muzzle".to_owned()],
            high_material_zone_ids: vec!["outer-shell".to_owned()],
            high_policy: PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY.to_owned(),
            high_policy_sha256: sha256_hex(PRODUCTION_WEAPON_HIGH_ARTIFACT_POLICY.as_bytes()),
            high_artifact_kind: PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND.to_owned(),
            high_mime: GLB_MIME.to_owned(),
            high_size_bytes: 1024,
            high_worker_algorithm_sha256: hash('9'),
            high_worker_build_cohort_sha256: hash('a'),
            high_worker_replay_count: 2,
            high_replay_byte_exact: true,
            high_topology_status: "structural-readback".to_owned(),
            high_authoring_topology_status: "partial".to_owned(),
            high_uv_status: "NOT_RUN".to_owned(),
            high_tangent_status: "NOT_RUN".to_owned(),
            validator_status: "passed".to_owned(),
            structural_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
            visual_status: "NOT_RUN".to_owned(),
            human_status: "NOT_RUN".to_owned(),
            engine_status: "NOT_RUN".to_owned(),
            distribution_status: "NOT_RUN".to_owned(),
            quality_status: "structural_only".to_owned(),
            hard_gate_passed: true,
            runtime_write_performed: true,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            request_sha256: candidate.request_sha256.clone(),
            input_sha256: hash('b'),
            receipt_object_sha256: receipt.record.sha256.clone(),
            canonical_sha256: hash('c'),
            created_at: "1".to_owned(),
        };
        ProductionWeaponFormalHighCommitBundle {
            candidate,
            high,
            idempotency_key: "idempotency-formal-high".to_owned(),
            high_artifact_object: readback.record.clone(),
            high_artifact_readback_object: readback.record.clone(),
            high_geometry_program_object: readback.record.clone(),
            high_detail_graph_object: readback.record,
            receipt_object: receipt.record,
        }
    }

    #[test]
    fn table_is_additive_and_closed() {
        let store = Store::memory().expect("store");
        let connection = store.lock_connection().expect("connection");
        ensure_table(&connection).expect("table");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                params![TABLE],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(count, 1);
        let idempotency_column: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name='idempotency_key'",
                params![TABLE],
                |row| row.get(0),
            )
            .expect("idempotency column");
        assert_eq!(idempotency_column, 1);
        let schema: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
                params![TABLE],
                |row| row.get(0),
            )
            .expect("table schema");
        assert!(schema.contains("UNIQUE (project_id, session_id, idempotency_key)"));
    }

    #[test]
    fn candidate_hash_is_not_caller_controlled() {
        let candidate = CandidateRecord {
            schema_version: "Candidate@1".to_owned(),
            candidate_id: "candidate-high".to_owned(),
            project_id: "project-high".to_owned(),
            base_version_id: None,
            source_version_id: None,
            prepared_object_id: Some("artifact-high".to_owned()),
            prepared_object_sha256: Some("a".repeat(64)),
            state: "prepared".to_owned(),
            request_sha256: "b".repeat(64),
            manifest_hash: None,
            quality_report_id: None,
            quality_hard_gate_passed: false,
            canonical_sha256: "c".repeat(64),
            error_code: None,
            created_at: "1".to_owned(),
            updated_at: "1".to_owned(),
        };
        assert!(validate_candidate_shape(&candidate).is_err());
    }

    #[test]
    fn validated_rows_commit_replay_conflict_tamper_and_reopen() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-formal-high-row-restart-{}",
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("fixture root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let bundle = {
            let store = Store::open_with_cas(&database, &cas).expect("store");
            store
                .insert_project(&ProjectRecord {
                    schema_version: "Project@1".to_owned(),
                    project_id: "project-formal-high".to_owned(),
                    name: "Formal High row fixture".to_owned(),
                    policy: serde_json::json!({"scope":"isolated-test"}),
                    created_at: "1".to_owned(),
                    updated_at: "1".to_owned(),
                    active_snapshot_revision: 0,
                    head_snapshot_id: None,
                    canonical_sha256: hash('a'),
                })
                .expect("project");
            let bundle = row_bundle(&store);
            let payload = serde_json::to_string(&bundle.high).expect("payload");
            let mut connection = store.lock_connection().expect("connection");
            ensure_table(&connection).expect("table");
            let transaction = connection.transaction().expect("transaction");
            let (stored, replayed) =
                commit_validated_formal_high_rows(&transaction, &bundle, &payload)
                    .expect("first row commit");
            assert!(!replayed);
            assert_eq!(stored, bundle.high);
            transaction.commit().expect("first commit");
            bundle
        };

        let store = Store::open_with_cas(&database, &cas).expect("reopened store");
        let payload = serde_json::to_string(&bundle.high).expect("payload");
        let mut connection = store.lock_connection().expect("connection");
        let transaction = connection.transaction().expect("replay transaction");
        let (stored, replayed) = commit_validated_formal_high_rows(&transaction, &bundle, &payload)
            .expect("exact replay");
        assert!(replayed);
        assert_eq!(stored, bundle.high);
        transaction.commit().expect("replay commit");

        let mut conflict = bundle.clone();
        conflict.high.request_sha256 = hash('f');
        let conflict_payload = serde_json::to_string(&conflict.high).expect("conflict payload");
        let transaction = connection.transaction().expect("conflict transaction");
        let error = commit_validated_formal_high_rows(&transaction, &conflict, &conflict_payload)
            .expect_err("same identity with different payload must fail");
        assert!(matches!(
            error,
            StoreError::Contract { code, .. }
                if code == "PRODUCTION_WEAPON_FORMAL_HIGH_REPLAY_CONFLICT"
        ));
        transaction.rollback().expect("conflict rollback");

        connection
            .execute(
                &format!("UPDATE {TABLE} SET payload_json='{{}}' WHERE high_artifact_id=?1"),
                params![bundle.high.high_artifact_id],
            )
            .expect("tamper payload");
        let transaction = connection.transaction().expect("tamper transaction");
        assert!(read_high_with_idempotency(&transaction, &bundle.high.high_artifact_id).is_err());
        transaction.rollback().expect("tamper rollback");
        drop(connection);
        drop(store);
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }
}
