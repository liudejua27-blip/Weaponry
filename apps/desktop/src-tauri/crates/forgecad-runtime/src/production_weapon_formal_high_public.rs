//! Closed public Runtime adapter for one formal derived High candidate.
//!
//! The adapter accepts only source Stage identity plus a caller-selected new
//! candidate identity. Session/project, candidate state, artifact/readback and
//! receipt hashes are always derived by Runtime. This keeps the public request
//! acyclic and prevents the monolithic High/Low/Bake path from partially
//! writing a formal High result.

use super::production_weapon_formal_high::FormalHighMaterializeInput;
use super::{canonical_json_hash, is_opaque_id, is_sha256, Runtime, RuntimeError};
use forgecad_contracts::{
    ProductionWeaponFormalHighGetResult, ProductionWeaponFormalHighPrepareResult,
    PRODUCTION_WEAPON_FORMAL_HIGH_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORMAL_HIGH_GET_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORMAL_HIGH_MAX_RESPONSE_BYTES,
    PRODUCTION_WEAPON_FORMAL_HIGH_PREPARE_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORMAL_HIGH_PREPARE_RESULT_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORMAL_HIGH_WRITER_POLICY,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "source_stage_head_transition_id",
    "source_stage_head_transition_sha256",
    "source_stage_head_canonical_sha256",
    "high_candidate_id",
    "idempotency_key",
    "max_response_bytes",
    "writer_policy",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "session_id",
    "high_artifact_id",
    "high_candidate_id",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_FORMAL_HIGH_PUBLIC_INVALID: {}",
        message.into()
    ))
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
    if actual != expected {
        return Err(invalid(format!(
            "{context} contains an unknown or missing field"
        )));
    }
    Ok(object)
}

fn text(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} is not an opaque identity")))
}

fn hash(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} is not a SHA-256")))
}

fn request_hash(value: &Value) -> Result<String, RuntimeError> {
    let mut preimage = value.clone();
    let object = preimage
        .as_object_mut()
        .ok_or_else(|| invalid("request must be an object"))?;
    object.remove("input_sha256");
    Ok(canonical_json_hash(&preimage))
}

fn validate_prepare_policy(object: &Map<String, Value>) -> Result<(), RuntimeError> {
    if object.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_FORMAL_HIGH_PREPARE_REQUEST_SCHEMA_VERSION)
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(PRODUCTION_WEAPON_FORMAL_HIGH_MAX_RESPONSE_BYTES)
        || object.get("writer_policy").and_then(Value::as_str)
            != Some(PRODUCTION_WEAPON_FORMAL_HIGH_WRITER_POLICY)
    {
        return Err(invalid("request policy fields differ"));
    }
    Ok(())
}

fn bounded_result<T: serde::Serialize>(result: &T) -> Result<Value, RuntimeError> {
    let value = serde_json::to_value(result)
        .map_err(|error| invalid(format!("result serialization failed: {error}")))?;
    let size = serde_json::to_vec(&value)
        .map_err(|error| invalid(format!("result serialization failed: {error}")))?
        .len() as u64;
    if size > PRODUCTION_WEAPON_FORMAL_HIGH_MAX_RESPONSE_BYTES {
        return Err(invalid("result exceeds max_response_bytes"));
    }
    Ok(value)
}

impl Runtime {
    pub fn production_weapon_formal_high_prepare(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let object = exact_object(
            &value,
            PREPARE_FIELDS,
            PRODUCTION_WEAPON_FORMAL_HIGH_PREPARE_REQUEST_SCHEMA_VERSION,
        )?;
        validate_prepare_policy(object)?;
        let input_sha256 = hash(object, "input_sha256")?;
        if request_hash(&value)? != input_sha256 {
            return Err(invalid("input_sha256 does not bind the prepare request"));
        }
        let input = FormalHighMaterializeInput {
            source_stage_head_transition_id: text(object, "source_stage_head_transition_id")?,
            source_stage_head_transition_sha256: hash(
                object,
                "source_stage_head_transition_sha256",
            )?,
            source_stage_head_canonical_sha256: hash(object, "source_stage_head_canonical_sha256")?,
            high_candidate_id: text(object, "high_candidate_id")?,
            idempotency_key: text(object, "idempotency_key")?,
            request_sha256: input_sha256.clone(),
        };
        let (high, replayed) = self.materialize_production_weapon_formal_high(&input)?;
        let candidate = self
            .candidate(&high.high_candidate_id)?
            .ok_or_else(|| invalid("Runtime-derived High candidate is missing"))?;
        if candidate.canonical_sha256 != high.high_candidate_state_sha256 {
            return Err(invalid("Runtime-derived High candidate differs"));
        }
        bounded_result(&ProductionWeaponFormalHighPrepareResult {
            schema_version: PRODUCTION_WEAPON_FORMAL_HIGH_PREPARE_RESULT_SCHEMA_VERSION.to_owned(),
            candidate,
            high,
            replayed,
            runtime_write: !replayed,
            restart_hash_verified: true,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
        })
    }

    pub fn production_weapon_formal_high_get(&self, value: Value) -> Result<Value, RuntimeError> {
        let object = exact_object(
            &value,
            GET_FIELDS,
            PRODUCTION_WEAPON_FORMAL_HIGH_GET_REQUEST_SCHEMA_VERSION,
        )?;
        if object.get("schema_version").and_then(Value::as_str)
            != Some(PRODUCTION_WEAPON_FORMAL_HIGH_GET_REQUEST_SCHEMA_VERSION)
        {
            return Err(invalid("get schema differs"));
        }
        let project_id = text(object, "project_id")?;
        let session_id = text(object, "session_id")?;
        let high_artifact_id = text(object, "high_artifact_id")?;
        let high_candidate_id = text(object, "high_candidate_id")?;
        let high = self
            .store
            .get_production_weapon_formal_high(&project_id, &session_id, &high_artifact_id)?
            .ok_or_else(|| invalid("formal High record is missing"))?;
        if high.high_candidate_id != high_candidate_id {
            return Err(invalid("formal High candidate binding differs"));
        }
        let candidate = self
            .candidate(&high.high_candidate_id)?
            .ok_or_else(|| invalid("formal High candidate is missing"))?;
        if candidate.canonical_sha256 != high.high_candidate_state_sha256 {
            return Err(invalid("formal High candidate state differs"));
        }
        bounded_result(&ProductionWeaponFormalHighGetResult {
            schema_version: PRODUCTION_WEAPON_FORMAL_HIGH_GET_RESULT_SCHEMA_VERSION.to_owned(),
            candidate,
            high,
            replayed: true,
            runtime_write: false,
            restart_hash_verified: true,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
        })
    }
}
