//! Runtime-owned orchestration for one formal derived High candidate.
//!
//! This is the Runtime-owned implementation behind the closed public MCP
//! `get/prepare` seam.  Public exposure remains source/compile evidence until
//! an isolated positive/replay/restart/cleanup fixture proves the full path.
//! The operation writes two new JSON roots (formal readback + receipt) and one
//! distinct prepared candidate/formal High row.  It cannot advance a Stage,
//! confirm, version or export.

use super::production_weapon_formal_high_factory::{
    build_formal_high_artifact, VerifiedFormalHighFactoryInput,
};
use super::{canonical_json_bytes, canonical_json_hash, Runtime, RuntimeError};
use forgecad_contracts::{
    is_opaque_id, is_sha256, ProductionWeaponHighArtifactRecord,
    PRODUCTION_WEAPON_HIGH_ARTIFACT_RECEIPT_KIND,
};
use forgecad_store::{CasObject, CasReservation, ProductionWeaponFormalHighCommitBundle};
use serde_json::{json, Value};

const JSON_MIME: &str = "application/json";
const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;

/// Closed High-only input. Runtime derives every output hash, including the
/// prepared candidate state and formal readback. Keeping those outputs out of
/// the input preimage avoids the hash cycle in the monolithic High/Low/Bake
/// request (`input_sha256 -> candidate state -> input_sha256`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormalHighMaterializeInput {
    pub source_stage_head_transition_id: String,
    pub source_stage_head_transition_sha256: String,
    pub source_stage_head_canonical_sha256: String,
    pub high_candidate_id: String,
    pub idempotency_key: String,
    pub request_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn required_string(value: &Value, field: &str) -> Result<String, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("HighMeshArtifact.{field} is missing")))
}

fn required_string_array(value: &Value, field: &str) -> Result<Vec<String>, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("HighMeshArtifact.{field} is missing")))?
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| invalid(format!("HighMeshArtifact.{field} is invalid")))
        })
        .collect()
}

fn read_json(runtime: &Runtime, hash: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read_bounded(hash, MAX_JSON_BYTES)?;
    serde_json::from_slice(&bytes).map_err(|error| invalid(format!("CAS JSON is invalid: {error}")))
}

fn release_all(
    runtime: &Runtime,
    reservation: &CasReservation,
    objects: &[CasObject],
    cleanup: bool,
) -> Result<(), RuntimeError> {
    let mut first_error = None;
    for object in objects.iter().rev() {
        if let Err(error) =
            runtime
                .store
                .release_cas_reservation_object(reservation, object, cleanup)
        {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }
    first_error.map_or(Ok(()), |error| Err(RuntimeError::Store(error)))
}

fn put_reserved(
    runtime: &Runtime,
    reservation: &CasReservation,
    objects: &mut Vec<CasObject>,
    bytes: &[u8],
    expected_hash: &str,
    created_at: &str,
) -> Result<CasObject, RuntimeError> {
    let object = runtime.store.put_object_reserved(
        reservation,
        bytes,
        Some(expected_hash),
        JSON_MIME,
        PRODUCTION_WEAPON_HIGH_ARTIFACT_RECEIPT_KIND,
        created_at,
    )?;
    objects.push(object.clone());
    Ok(object)
}

fn formal_readback(
    input: &FormalHighMaterializeInput,
    session_id: &str,
    project_id: &str,
    high_artifact_id: &str,
    high_artifact_sha256: &str,
    native_readback_object_sha256: &str,
    native_readback_sha256: &str,
    part_ids: &[String],
    material_zone_ids: &[String],
) -> Result<(Value, Vec<u8>), RuntimeError> {
    let mut value = json!({
        "schema_version":"ProductionWeaponHighArtifactReadback@1",
        "project_id":project_id,
        "session_id":session_id,
        "high_candidate_id":input.high_candidate_id,
        "high_artifact_id":high_artifact_id,
        "high_artifact_sha256":high_artifact_sha256,
        "native_readback_object_sha256":native_readback_object_sha256,
        "native_readback_sha256":native_readback_sha256,
        "part_ids":part_ids,
        "material_zone_ids":material_zone_ids,
        "validator_status":"passed",
        "structural_status":"PASS_SOURCE_STRUCTURAL",
        "visual_status":"NOT_RUN",
        "human_status":"NOT_RUN",
        "engine_status":"NOT_RUN",
        "distribution_status":"NOT_RUN",
        "quality_status":"structural_only",
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    let bytes = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    Ok((value, bytes))
}

impl Runtime {
    /// Materialize the High-only durable slice from an already validated
    /// High-only input. The public adapter is intentionally thin; this method
    /// owns all source revalidation, CAS reservation and Store commit work.
    pub(crate) fn materialize_production_weapon_formal_high(
        &self,
        input: &FormalHighMaterializeInput,
    ) -> Result<(ProductionWeaponHighArtifactRecord, bool), RuntimeError> {
        if !is_opaque_id(&input.source_stage_head_transition_id)
            || !is_opaque_id(&input.high_candidate_id)
            || !is_opaque_id(&input.idempotency_key)
            || !is_sha256(&input.source_stage_head_transition_sha256)
            || !is_sha256(&input.source_stage_head_canonical_sha256)
            || !is_sha256(&input.request_sha256)
        {
            return Err(invalid("PRODUCTION_WEAPON_FORMAL_HIGH_INPUT_INVALID"));
        }

        let transition = self
            .store
            .get_production_stage_transition_v3(&input.source_stage_head_transition_id)?
            .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_TRANSITION_MISSING"))?;
        let head = self
            .store
            .get_production_stage_head_v3(
                &transition.session_id,
                &transition.project_id,
                &transition.root_candidate_id,
            )?
            .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_HEAD_MISSING"))?;
        if transition.canonical_sha256 != input.source_stage_head_transition_sha256
            || head.canonical_sha256 != input.source_stage_head_canonical_sha256
            || transition.to_stage != "secondary-form-approved"
            || head.head_stage != "secondary-form-approved"
            || head.head_transition_id != transition.transition_id
            || head.head_transition_sha256 != transition.canonical_sha256
        {
            return Err(invalid(
                "PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_HEAD_MISMATCH",
            ));
        }
        let source_candidate = self
            .candidate(&transition.head_candidate_id)?
            .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORMAL_HIGH_SOURCE_CANDIDATE_MISSING"))?;
        let native = self
            .store
            .get_native_high_durable_by_candidate(&transition.head_candidate_id)?
            .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORMAL_HIGH_NATIVE_HIGH_MISSING"))?;
        let authoring = self
            .store
            .get_authoring_mesh_durable_record_by_mesh(
                &transition.head_candidate_id,
                &native.source_canonical_mesh_id,
            )?
            .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORMAL_HIGH_AUTHORING_MESH_MISSING"))?;
        let evidence = self
            .store
            .get_geometry_candidate_evidence(&transition.head_candidate_id)?
            .ok_or_else(|| invalid("PRODUCTION_WEAPON_FORMAL_HIGH_GEOMETRY_EVIDENCE_MISSING"))?;
        if source_candidate.canonical_sha256 != transition.head_candidate_state_sha256 {
            return Err(invalid(
                "PRODUCTION_WEAPON_FORMAL_HIGH_NATIVE_BINDING_MISMATCH",
            ));
        }

        if let Some(existing) = self.store.get_production_weapon_formal_high(
            &transition.project_id,
            &transition.session_id,
            &native.high_artifact_id,
        )? {
            if existing.high_candidate_id != input.high_candidate_id
                || existing.source_stage_head_transition_id != input.source_stage_head_transition_id
                || existing.source_stage_head_transition_sha256
                    != input.source_stage_head_transition_sha256
                || existing.source_stage_head_canonical_sha256
                    != input.source_stage_head_canonical_sha256
                || existing.request_sha256 != input.request_sha256
            {
                return Err(invalid("PRODUCTION_WEAPON_FORMAL_HIGH_REPLAY_CONFLICT"));
            }
            return Ok((existing, true));
        }

        let high_mesh = read_json(self, &native.high_mesh_artifact_object_sha256)?;
        if high_mesh.get("schema_version").and_then(Value::as_str) != Some("HighMeshArtifact@1")
            || high_mesh.get("artifact_sha256").and_then(Value::as_str)
                != Some(native.high_mesh_artifact_sha256.as_str())
        {
            return Err(invalid("PRODUCTION_WEAPON_FORMAL_HIGH_HIGH_MESH_MISMATCH"));
        }
        let part_ids = required_string_array(&high_mesh, "part_ids")?;
        let material_zone_ids = required_string_array(&high_mesh, "material_zone_ids")?;
        let worker_algorithm_sha256 = required_string(&high_mesh, "high_worker_algorithm_sha256")?;
        let worker_cohort_sha256 = required_string(&high_mesh, "high_worker_build_cohort_sha256")?;
        let high_topology_status = required_string(&high_mesh, "high_topology_status")?;
        if !is_sha256(&worker_algorithm_sha256)
            || worker_cohort_sha256 != native.high_worker_build_cohort_sha256
        {
            return Err(invalid(
                "PRODUCTION_WEAPON_FORMAL_HIGH_WORKER_BINDING_MISMATCH",
            ));
        }

        let (readback_value, readback_bytes) = formal_readback(
            input,
            &transition.session_id,
            &transition.project_id,
            &native.high_artifact_id,
            &native.high_artifact_sha256,
            &native.high_artifact_readback_object_sha256,
            &native.high_artifact_readback_sha256,
            &part_ids,
            &material_zone_ids,
        )?;
        let readback_hash = super::sha256_hex(&readback_bytes);
        let readback_canonical_sha256 = required_string(&readback_value, "canonical_sha256")?;
        let factory_input = VerifiedFormalHighFactoryInput {
            session_id: transition.session_id.clone(),
            project_id: transition.project_id.clone(),
            source_stage_head_transition_id: transition.transition_id.clone(),
            source_stage_head_transition_sha256: transition.canonical_sha256.clone(),
            source_stage_head_canonical_sha256: head.canonical_sha256.clone(),
            source_stage_head_stage: head.head_stage.clone(),
            source_candidate_id: source_candidate.candidate_id.clone(),
            source_candidate_state_sha256: source_candidate.canonical_sha256.clone(),
            source_artifact_id: transition.output_artifact_id.clone(),
            source_artifact_sha256: transition.head_artifact_sha256.clone(),
            source_artifact_readback_sha256: authoring.source_artifact_readback_sha256.clone(),
            high_candidate_id: input.high_candidate_id.clone(),
            high_artifact_id: native.high_artifact_id.clone(),
            high_artifact_sha256: native.high_artifact_sha256.clone(),
            high_artifact_readback_sha256: readback_canonical_sha256,
            high_artifact_readback_object_sha256: readback_hash.clone(),
            high_geometry_program_sha256: evidence.geometry_program_sha256.clone(),
            high_geometry_program_object_sha256: evidence.geometry_program_object_sha256.clone(),
            high_geometry_candidate_evidence_sha256: evidence.canonical_sha256.clone(),
            high_detail_graph_object_sha256: native.detail_graph_object_sha256.clone(),
            high_detail_graph_canonical_sha256: native.detail_graph_canonical_sha256.clone(),
            high_part_ids: part_ids,
            high_material_zone_ids: material_zone_ids,
            high_size_bytes: native.high_artifact_size_bytes,
            high_worker_algorithm_sha256: worker_algorithm_sha256,
            high_worker_build_cohort_sha256: worker_cohort_sha256,
            high_topology_status,
            base_version_id: source_candidate.base_version_id.clone(),
            source_version_id: source_candidate.source_version_id.clone(),
            candidate_manifest_hash: source_candidate.manifest_hash.clone(),
            request_sha256: input.request_sha256.clone(),
            created_at: transition.created_at.clone(),
            updated_at: transition.created_at.clone(),
        };
        let output = build_formal_high_artifact(factory_input)
            .map_err(|error| invalid(format!("formal High factory failed: {error}")))?;
        if readback_value
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(output.high.high_artifact_readback_sha256.as_str())
        {
            return Err(invalid(
                "PRODUCTION_WEAPON_FORMAL_HIGH_READBACK_CANONICAL_MISMATCH",
            ));
        }

        let reservation = self.store.begin_cas_reservation();
        let mut reserved = Vec::new();
        let operation: Result<(ProductionWeaponHighArtifactRecord, bool), RuntimeError> = (|| {
            let readback_object = put_reserved(
                self,
                &reservation,
                &mut reserved,
                &readback_bytes,
                &readback_hash,
                &transition.created_at,
            )?;
            let receipt_object = put_reserved(
                self,
                &reservation,
                &mut reserved,
                &output.receipt_json_bytes,
                &output.receipt_object_sha256,
                &transition.created_at,
            )?;
            let high_artifact_object = self
                .store
                .get_object(&output.high.high_artifact_sha256)?
                .ok_or_else(|| invalid("formal High GLB CAS root is missing"))?;
            let high_geometry_program_object = self
                .store
                .get_object(&output.high.high_geometry_program_object_sha256)?
                .ok_or_else(|| invalid("formal High geometry program CAS root is missing"))?;
            let high_detail_graph_object = self
                .store
                .get_object(&output.high.high_detail_graph_object_sha256)?
                .ok_or_else(|| invalid("formal High detail graph CAS root is missing"))?;
            Ok(self
                .store
                .record_production_weapon_formal_high_with_replay(
                    &ProductionWeaponFormalHighCommitBundle {
                        candidate: output.candidate.clone(),
                        high: output.high.clone(),
                        idempotency_key: input.idempotency_key.clone(),
                        high_artifact_object,
                        high_artifact_readback_object: readback_object.record,
                        high_geometry_program_object,
                        high_detail_graph_object,
                        receipt_object: receipt_object.record,
                    },
                )?)
        })(
        );
        match operation {
            Ok((stored, replayed)) => {
                release_all(self, &reservation, &reserved, false)?;
                let restart = self
                    .store
                    .get_production_weapon_formal_high(
                        &stored.project_id,
                        &stored.session_id,
                        &stored.high_artifact_id,
                    )?
                    .ok_or_else(|| invalid("formal High restart readback is missing"))?;
                if restart != stored {
                    return Err(invalid("formal High restart readback differs"));
                }
                Ok((restart, replayed))
            }
            Err(error) => {
                if let Err(cleanup) = release_all(self, &reservation, &reserved, true) {
                    return Err(invalid(format!(
                        "formal High failed: {error}; CAS cleanup failed: {cleanup}"
                    )));
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn input() -> FormalHighMaterializeInput {
        FormalHighMaterializeInput {
            source_stage_head_transition_id: "transition-secondary".to_owned(),
            source_stage_head_transition_sha256: hash('a'),
            source_stage_head_canonical_sha256: hash('b'),
            high_candidate_id: "candidate-formal-high".to_owned(),
            idempotency_key: "idempotency-formal-high".to_owned(),
            request_sha256: hash('c'),
        }
    }

    #[test]
    fn high_only_input_excludes_runtime_derived_output_hashes() {
        let value = serde_json::to_value(json!({
            "source_stage_head_transition_id":input().source_stage_head_transition_id,
            "source_stage_head_transition_sha256":input().source_stage_head_transition_sha256,
            "source_stage_head_canonical_sha256":input().source_stage_head_canonical_sha256,
            "high_candidate_id":input().high_candidate_id,
            "request_sha256":input().request_sha256
        }))
        .expect("input value");
        assert!(value.get("high_candidate_state_sha256").is_none());
        assert!(value.get("high_artifact_readback_sha256").is_none());
        assert!(value.get("receipt_object_sha256").is_none());
    }

    #[test]
    fn formal_readback_is_runtime_derived_and_canonical() {
        let (value, bytes) = formal_readback(
            &input(),
            "session-formal-high",
            "project-formal-high",
            "high-artifact-1",
            &hash('d'),
            &hash('e'),
            &hash('f'),
            &["receiver".to_owned(), "muzzle".to_owned()],
            &["outer-shell".to_owned()],
        )
        .expect("formal readback");
        assert_eq!(
            value["canonical_sha256"],
            canonical_json_hash(&{
                let mut preimage = value.clone();
                preimage["canonical_sha256"] = Value::String(String::new());
                preimage
            })
        );
        assert_eq!(
            bytes,
            canonical_json_bytes(&value).expect("canonical bytes")
        );
        assert!(value.get("high_candidate_state_sha256").is_none());
        assert_eq!(value["production_stage_advanced"], false);
        assert_eq!(value["candidate_confirmed"], false);
        assert_eq!(value["version_created"], false);
        assert_eq!(value["export_performed"], false);
    }
}
