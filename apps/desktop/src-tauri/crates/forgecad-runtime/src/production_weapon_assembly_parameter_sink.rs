//! Read-only Runtime projection of the closed FPS weapon assembly parameter
//! sinks.
//!
//! This module binds a request to the existing candidate, artifact,
//! GeometryProgram and five-group AssemblyDecisionRegistry.  It only reads
//! Runtime-owned state and CAS objects; it never writes SQLite/CAS, invokes a
//! Worker, creates a candidate, or exposes a caller-selected path/expression.

use super::production_weapon_art_decision_proposal::build_registry;
use super::production_weapon_assembly_parameter_mutator::{
    production_weapon_assembly_parameter_descriptors, ProductionWeaponAssemblyParameterDescriptor,
};
use super::{canonical_json_hash, is_opaque_id, is_sha256, Runtime, RuntimeError};
use forgecad_contracts::{
    ProductionWeaponAssemblyParameterSink, ProductionWeaponAssemblyParameterSinkGetRequest,
    ProductionWeaponAssemblyParameterSinkGetResult, ProductionWeaponAssemblyParameterSinkRegistry,
    PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_PROFILE_ID,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_ENGINE_STATUS,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_HUMAN_STATUS,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_APPLICATION_STATUS,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_POLICY,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_SCHEMA_VERSION,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_STATUS,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_SUPPORTED_GROUP_IDS,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_SUPPORTED_PARAMETER_IDS,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_UNAVAILABLE_PARAMETER_IDS,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_STRUCTURAL_STATUS,
    PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_VISUAL_STATUS,
};
use forgecad_worker_protocol::operator_catalog_sha256;
use serde_json::Value;

const SINK_REGISTRY_ID: &str = "fps-weapon-assembly-parameter-sink-registry";
const REQUEST_FIELDS: [&str; 13] = [
    "schema_version",
    "sink_registry_id",
    "session_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "artifact_id",
    "artifact_sha256",
    "geometry_program_sha256",
    "geometry_program_canonical_sha256",
    "operator_catalog_sha256",
    "assembly_registry_id",
    "assembly_registry_canonical_sha256",
];
const MAX_PROGRAM_BYTES: u64 = 1024 * 1024;
const MAX_READBACK_BYTES: u64 = 1024 * 1024;

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn parse_request(
    value: &Value,
) -> Result<ProductionWeaponAssemblyParameterSinkGetRequest, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SINK_REQUEST_NOT_OBJECT"))?;
    if object.len() != REQUEST_FIELDS.len()
        || object
            .keys()
            .any(|field| !REQUEST_FIELDS.contains(&field.as_str()))
        || REQUEST_FIELDS
            .iter()
            .any(|field| !object.contains_key(*field))
    {
        return Err(invalid("ASSEMBLY_PARAMETER_SINK_REQUEST_FIELDS_INVALID"));
    }
    let request: ProductionWeaponAssemblyParameterSinkGetRequest =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!("ASSEMBLY_PARAMETER_SINK_REQUEST_INVALID: {error}"))
        })?;
    if request.schema_version
        != PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_GET_REQUEST_SCHEMA_VERSION
    {
        return Err(invalid("ASSEMBLY_PARAMETER_SINK_REQUEST_SCHEMA_INVALID"));
    }
    if request.sink_registry_id != SINK_REGISTRY_ID {
        return Err(invalid("ASSEMBLY_PARAMETER_SINK_REGISTRY_ID_INVALID"));
    }
    for id in [
        &request.sink_registry_id,
        &request.session_id,
        &request.project_id,
        &request.candidate_id,
        &request.artifact_id,
        &request.assembly_registry_id,
    ] {
        if !is_opaque_id(id) {
            return Err(invalid("ASSEMBLY_PARAMETER_SINK_REQUEST_ID_INVALID"));
        }
    }
    for hash in [
        &request.candidate_state_sha256,
        &request.artifact_sha256,
        &request.geometry_program_sha256,
        &request.geometry_program_canonical_sha256,
        &request.operator_catalog_sha256,
        &request.assembly_registry_canonical_sha256,
    ] {
        if !is_sha256(hash) {
            return Err(invalid("ASSEMBLY_PARAMETER_SINK_REQUEST_HASH_INVALID"));
        }
    }
    Ok(request)
}

fn sink_from_descriptor(
    descriptor: ProductionWeaponAssemblyParameterDescriptor,
) -> ProductionWeaponAssemblyParameterSink {
    ProductionWeaponAssemblyParameterSink {
        parameter_id: descriptor.parameter_id,
        group_id: descriptor.group_id,
        mutator_id: descriptor.mutator_id,
        current: descriptor.current,
        min: descriptor.min,
        max: descriptor.max,
        step: descriptor.step,
        unit: descriptor.unit,
        application_status: PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_APPLICATION_STATUS
            .to_owned(),
        blocker_codes: Vec::new(),
        target_part_ids: descriptor.target_part_ids,
        source_node_ids: descriptor.source_node_ids,
        operator_ids: descriptor.operator_ids,
        evidence_requirements: descriptor.evidence_requirements,
    }
}

fn unavailable_parameter_ids(unavailable_supported: &[String]) -> Vec<String> {
    let mut ids = PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_UNAVAILABLE_PARAMETER_IDS
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    ids.extend(unavailable_supported.iter().cloned());
    ids
}

impl Runtime {
    /// Recompute the closed aggregate sink registry from the exact bound
    /// GeometryProgram and AssemblyDecisionRegistry.  The result is an
    /// in-memory projection with a canonical hash; it is not persisted.
    pub fn production_weapon_assembly_parameter_sink_get(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_request(&value)?;
        let active_catalog_sha256 = operator_catalog_sha256();
        if request.operator_catalog_sha256 != active_catalog_sha256 {
            return Err(invalid("ASSEMBLY_PARAMETER_SINK_OPERATOR_CATALOG_MISMATCH"));
        }

        let project = self
            .project(&request.project_id)?
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SINK_PROJECT_NOT_FOUND"))?;
        if project.project_id != request.project_id {
            return Err(invalid("ASSEMBLY_PARAMETER_SINK_PROJECT_MISMATCH"));
        }
        let session = self
            .store
            .get_agentic_session(&request.session_id)?
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SINK_SESSION_NOT_FOUND"))?;
        if session.project_id != request.project_id
            || session.candidate_id != request.candidate_id
            || session.candidate_state_sha256 != request.candidate_state_sha256
        {
            return Err(invalid("ASSEMBLY_PARAMETER_SINK_SESSION_LINEAGE_MISMATCH"));
        }

        let candidate = self
            .candidate(&request.candidate_id)?
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SINK_CANDIDATE_NOT_FOUND"))?;
        if candidate.project_id != request.project_id
            || candidate.canonical_sha256 != request.candidate_state_sha256
            || candidate.prepared_object_id.as_deref() != Some(request.artifact_id.as_str())
            || candidate.prepared_object_sha256.as_deref() != Some(request.artifact_sha256.as_str())
            || candidate
                .manifest_hash
                .as_deref()
                .is_some_and(|hash| hash != request.artifact_sha256)
            || !matches!(candidate.state.as_str(), "reviewable" | "confirmed")
        {
            return Err(invalid(
                "ASSEMBLY_PARAMETER_SINK_CANDIDATE_LINEAGE_MISMATCH",
            ));
        }

        let geometry = self
            .store
            .get_geometry_candidate_evidence(&request.candidate_id)?
            .ok_or_else(|| invalid("ASSEMBLY_PARAMETER_SINK_GEOMETRY_EVIDENCE_NOT_FOUND"))?;
        if geometry.candidate_id != request.candidate_id
            || geometry.project_id != request.project_id
            || geometry.artifact_object_sha256 != request.artifact_sha256
            || geometry.geometry_program_object_sha256 != request.geometry_program_sha256
            || geometry.geometry_program_sha256 != request.geometry_program_canonical_sha256
            || geometry.operator_catalog_sha256 != request.operator_catalog_sha256
        {
            return Err(invalid("ASSEMBLY_PARAMETER_SINK_GEOMETRY_LINEAGE_MISMATCH"));
        }

        // The artifact is read back through the existing Runtime-owned
        // candidate binding.  This is a read-only integrity check; it does
        // not create a receipt or touch reachability.
        let readback = self.artifact_readback(&request.artifact_sha256, &request.candidate_id)?;
        let stored_readback_bytes = self.cas_read_bounded(
            &geometry.artifact_readback_object_sha256,
            MAX_READBACK_BYTES,
        )?;
        let stored_readback: Value =
            serde_json::from_slice(&stored_readback_bytes).map_err(|error| {
                invalid(format!(
                    "ASSEMBLY_PARAMETER_SINK_ARTIFACT_READBACK_INVALID: {error}"
                ))
            })?;
        if stored_readback != readback
            || readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
            || readback.get("artifact_id").and_then(Value::as_str)
                != Some(request.artifact_sha256.as_str())
            || readback.get("object_sha256").and_then(Value::as_str)
                != Some(request.artifact_sha256.as_str())
            || readback.get("candidate_id").and_then(Value::as_str)
                != Some(request.candidate_id.as_str())
            || readback.get("program_sha256").and_then(Value::as_str)
                != Some(request.geometry_program_canonical_sha256.as_str())
            || readback
                .get("operator_catalog_sha256")
                .and_then(Value::as_str)
                != Some(request.operator_catalog_sha256.as_str())
            || readback
                .get("readback_config_sha256")
                .and_then(Value::as_str)
                != Some(geometry.readback_config_sha256.as_str())
            || readback.get("validator_status").and_then(Value::as_str) != Some("passed")
            || readback.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        {
            return Err(invalid(
                "ASSEMBLY_PARAMETER_SINK_ARTIFACT_READBACK_LINEAGE_MISMATCH",
            ));
        }

        let program_bytes =
            self.cas_read_bounded(&request.geometry_program_sha256, MAX_PROGRAM_BYTES)?;
        let program: Value = serde_json::from_slice(&program_bytes).map_err(|error| {
            invalid(format!(
                "ASSEMBLY_PARAMETER_SINK_GEOMETRY_PROGRAM_INVALID: {error}"
            ))
        })?;
        if program.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str()) {
            return Err(invalid("ASSEMBLY_PARAMETER_SINK_GEOMETRY_PROJECT_MISMATCH"));
        }

        let (assembly_registry, _) = build_registry(&request.operator_catalog_sha256, &program)?;
        if request.assembly_registry_id != assembly_registry.registry_id
            || request.assembly_registry_canonical_sha256 != assembly_registry.canonical_sha256
            || assembly_registry.operator_catalog_sha256 != request.operator_catalog_sha256
        {
            return Err(invalid("ASSEMBLY_PARAMETER_SINK_REGISTRY_LINEAGE_MISMATCH"));
        }

        let descriptor_report = production_weapon_assembly_parameter_descriptors(
            &program,
            &request.geometry_program_canonical_sha256,
        )?;
        let sinks = descriptor_report
            .available
            .into_iter()
            .map(sink_from_descriptor)
            .collect::<Vec<_>>();
        let unavailable_parameter_ids =
            unavailable_parameter_ids(&descriptor_report.unavailable_parameter_ids);
        let status = if sinks.len()
            == PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_SUPPORTED_PARAMETER_IDS.len()
            && descriptor_report.unavailable_parameter_ids.is_empty()
        {
            PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_STATUS[1]
        } else {
            PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_STATUS[0]
        };
        let mut registry = ProductionWeaponAssemblyParameterSinkRegistry {
            schema_version: PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_SCHEMA_VERSION
                .to_owned(),
            sink_registry_id: SINK_REGISTRY_ID.to_owned(),
            profile_id: PRODUCTION_WEAPON_ASSEMBLY_DECISION_REGISTRY_PROFILE_ID.to_owned(),
            sink_policy: PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_POLICY.to_owned(),
            session_id: request.session_id,
            project_id: request.project_id,
            candidate_id: request.candidate_id,
            candidate_state_sha256: request.candidate_state_sha256,
            artifact_id: request.artifact_id,
            artifact_sha256: request.artifact_sha256,
            geometry_program_sha256: request.geometry_program_sha256,
            geometry_program_canonical_sha256: request.geometry_program_canonical_sha256,
            operator_catalog_sha256: request.operator_catalog_sha256,
            assembly_registry_id: assembly_registry.registry_id,
            assembly_registry_canonical_sha256: assembly_registry.canonical_sha256,
            supported_group_ids:
                PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_SUPPORTED_GROUP_IDS
                    .iter()
                    .map(|id| (*id).to_owned())
                    .collect(),
            sinks,
            unavailable_parameter_ids,
            status: status.to_owned(),
            read_only: true,
            runtime_write_performed: false,
            worker_invoked: false,
            candidate_generated: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            canonical_sha256: String::new(),
        };
        registry.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&registry).map_err(|error| invalid(error.to_string()))?,
        );
        let registry_canonical_sha256 = registry.canonical_sha256.clone();
        let result = ProductionWeaponAssemblyParameterSinkGetResult {
            schema_version: PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_GET_RESULT_SCHEMA_VERSION
                .to_owned(),
            registry,
            registry_canonical_sha256,
            recomputed: true,
            restart_hash_verified: true,
            read_only: true,
            structural_status: PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_STRUCTURAL_STATUS
                .to_owned(),
            quality_status: PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_STRUCTURAL_STATUS.to_owned(),
            visual_quality_status: PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_VISUAL_STATUS
                .to_owned(),
            human_review_status: PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_HUMAN_STATUS.to_owned(),
            commercial_engine_status: PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_ENGINE_STATUS
                .to_owned(),
            runtime_write_performed: false,
            worker_invoked: false,
            candidate_generated: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
        };
        serde_json::to_value(result).map_err(|error| invalid(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production_weapon_assembly_parameter_mutator::production_weapon_assembly_parameter_test_fixture as fixture;

    fn request_shape() -> Value {
        serde_json::json!({
            "schema_version":"ProductionWeaponAssemblyParameterSinkGetRequest@1",
            "sink_registry_id":SINK_REGISTRY_ID,
            "session_id":"session-a",
            "project_id":"project-a",
            "candidate_id":"candidate-a",
            "candidate_state_sha256":"a".repeat(64),
            "artifact_id":"artifact-a",
            "artifact_sha256":"b".repeat(64),
            "geometry_program_sha256":"c".repeat(64),
            "geometry_program_canonical_sha256":"d".repeat(64),
            "operator_catalog_sha256":"e".repeat(64),
            "assembly_registry_id":"registry-a",
            "assembly_registry_canonical_sha256":"f".repeat(64)
        })
    }

    #[test]
    fn request_is_exactly_thirteen_fields_and_rejects_retarget_or_tamper_shape() {
        let mut request = request_shape();
        assert!(parse_request(&request).is_ok());
        request["unexpected"] = Value::Bool(true);
        assert!(parse_request(&request).is_err());
        let mut request = request_shape();
        request["project_id"] = Value::String("project/retargeted".to_owned());
        assert!(parse_request(&request).is_err());
        let mut request = request_shape();
        request["assembly_registry_canonical_sha256"] = Value::String("0".repeat(63));
        assert!(parse_request(&request).is_err());
    }

    #[test]
    fn descriptor_report_is_closed_and_exactly_eight_for_d1_shape() {
        let program = fixture();
        let canonical = program["canonical_sha256"].as_str().unwrap();
        let report = production_weapon_assembly_parameter_descriptors(&program, canonical)
            .expect("descriptor report");
        assert_eq!(report.available.len(), 8);
        assert!(report.unavailable_parameter_ids.is_empty());
        assert_eq!(
            report.available[0].target_part_ids,
            vec!["receiver-main", "receiver-upper", "receiver-lower"]
        );
        assert_eq!(report.available[0].current, 1.0);
        assert_eq!(report.available[2].unit, "meter");
        assert_eq!(report.available[2].current, 0.0);
        assert_eq!(report.available[5].target_part_ids, vec!["muzzle-core"]);
        assert_eq!(report.available[6].target_part_ids, vec!["rear-stock"]);
        assert_eq!(report.available[6].unit, "meter");
        assert_eq!(report.available[7].unit, "radian");
    }

    #[test]
    fn unavailable_ids_are_exact_and_sink_registry_hash_is_deterministic() {
        let ids = unavailable_parameter_ids(&[]);
        assert_eq!(
            ids,
            PRODUCTION_WEAPON_ASSEMBLY_PARAMETER_SINK_REGISTRY_UNAVAILABLE_PARAMETER_IDS
                .iter()
                .map(|id| (*id).to_owned())
                .collect::<Vec<_>>()
        );
        let program = fixture();
        let canonical = program["canonical_sha256"].as_str().unwrap();
        let report = production_weapon_assembly_parameter_descriptors(&program, canonical)
            .expect("descriptor report");
        let sinks = report
            .available
            .into_iter()
            .map(sink_from_descriptor)
            .collect::<Vec<_>>();
        assert_eq!(sinks.len(), 8);
        assert!(sinks.iter().all(|sink| sink.blocker_codes.is_empty()));
        assert_eq!(
            sinks[0].evidence_requirements,
            vec![
                "assembly-registry",
                "geometry-program",
                "operator-catalog",
                "artifact-readback",
                "candidate-state"
            ]
        );
    }
}
