//! Read-only readiness projection for the formal High/Low/Cage/Bake chain.
//!
//! This deliberately does not call the source-bundle getter because that
//! getter replays Geometry Workers. It reads only the current Stage@3 head and
//! Store-owned formal row/CAS metadata, emits blockers, and never reserves or
//! writes CAS/SQLite state.

use super::{canonical_json_hash, Runtime, RuntimeError};
use forgecad_contracts::{
    ProductionWeaponHighLowBakePreflightCheck, ProductionWeaponHighLowBakePreflightGetRequest,
    ProductionWeaponHighLowBakePreflightGetResult, PRODUCTION_STAGE_V3_STAGES,
    PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREFLIGHT_GET_RESULT_SCHEMA_VERSION,
};
use forgecad_store::ProductionWeaponHighLowBakePreflightSourceSummary;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "preflight_id",
    "session_id",
    "project_id",
    "candidate_id",
    "expected_head_stage",
    "expected_head_transition_id",
    "expected_head_transition_sha256",
    "expected_head_canonical_sha256",
    "input_sha256",
];

const PREFLIGHT_STAGES: &[&str] = &[
    "camera-calibrated",
    "blockout-reviewed",
    "primary-form-approved",
    "secondary-form-approved",
    "high-poly-approved",
    "low-poly-approved",
    "uv-approved",
    "cage-approved",
    "bake-approved",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn is_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn exact_object<'a>(value: &'a Value) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("HighLowBake preflight request must be an object"))?;
    if let Some(field) = object
        .keys()
        .find(|field| !REQUEST_FIELDS.contains(&field.as_str()))
    {
        return Err(invalid(format!(
            "HighLowBake preflight contains unsupported field {field}"
        )));
    }
    if let Some(field) = REQUEST_FIELDS
        .iter()
        .find(|field| !object.contains_key(**field))
    {
        return Err(invalid(format!("HighLowBake preflight is missing {field}")));
    }
    Ok(object)
}

fn parse_request(
    value: &Value,
) -> Result<ProductionWeaponHighLowBakePreflightGetRequest, RuntimeError> {
    let object = exact_object(value)?;
    let request: ProductionWeaponHighLowBakePreflightGetRequest =
        serde_json::from_value(value.clone())
            .map_err(|error| invalid(format!("invalid HighLowBake preflight request: {error}")))?;
    if request.schema_version
        != PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION
    {
        return Err(invalid("HighLowBake preflight schema differs"));
    }
    for value in [
        &request.preflight_id,
        &request.session_id,
        &request.project_id,
        &request.candidate_id,
        &request.expected_head_transition_id,
    ] {
        if !is_id(value) {
            return Err(invalid("HighLowBake preflight identity is invalid"));
        }
    }
    for value in [
        &request.expected_head_transition_sha256,
        &request.expected_head_canonical_sha256,
        &request.input_sha256,
    ] {
        if !is_sha(value) {
            return Err(invalid("HighLowBake preflight SHA-256 is invalid"));
        }
    }
    if !PREFLIGHT_STAGES.contains(&request.expected_head_stage.as_str())
        || !PRODUCTION_STAGE_V3_STAGES.contains(&request.expected_head_stage.as_str())
    {
        return Err(invalid("HighLowBake preflight stage is invalid"));
    }
    let mut preimage = object.clone();
    preimage.remove("input_sha256");
    if canonical_json_hash(&Value::Object(preimage)) != request.input_sha256 {
        return Err(invalid("HighLowBake preflight input hash differs"));
    }
    Ok(request)
}

fn stage_at_least(stage: &str, required: &str) -> bool {
    let index = PRODUCTION_STAGE_V3_STAGES
        .iter()
        .position(|value| *value == stage);
    let required = PRODUCTION_STAGE_V3_STAGES
        .iter()
        .position(|value| *value == required);
    matches!((index, required), (Some(index), Some(required)) if index >= required)
}

fn check(
    status: &str,
    reason_code: &str,
    object_sha256: Option<String>,
    canonical_sha256: Option<String>,
) -> ProductionWeaponHighLowBakePreflightCheck {
    ProductionWeaponHighLowBakePreflightCheck {
        status: status.to_owned(),
        reason_code: reason_code.to_owned(),
        object_sha256,
        canonical_sha256,
    }
}

fn summary<'a>(
    links: &'a [ProductionWeaponHighLowBakePreflightSourceSummary],
) -> Option<&'a ProductionWeaponHighLowBakePreflightSourceSummary> {
    links.first()
}

fn build_result(
    runtime: &Runtime,
    request: &ProductionWeaponHighLowBakePreflightGetRequest,
) -> Result<ProductionWeaponHighLowBakePreflightGetResult, RuntimeError> {
    let sources = runtime
        .store
        .get_production_weapon_high_low_bake_preflight_sources(
            &request.project_id,
            &request.session_id,
            &request.candidate_id,
        )?;
    if let Some(head) = &sources.head {
        if head.head_stage != request.expected_head_stage
            || head.head_transition_id != request.expected_head_transition_id
            || head.head_transition_sha256 != request.expected_head_transition_sha256
            || head.canonical_sha256 != request.expected_head_canonical_sha256
        {
            return Err(invalid(
                "HighLowBake preflight expected head binding differs",
            ));
        }
    }
    let formal = summary(&sources.formal_bake_links);
    let head_passed = sources
        .head
        .as_ref()
        .is_some_and(|head| stage_at_least(&head.head_stage, "secondary-form-approved"));
    let mut checks = BTreeMap::new();
    checks.insert(
        "secondary_form_head".to_owned(),
        if head_passed {
            let head = sources.head.as_ref().expect("checked head");
            check(
                "passed",
                "SECONDARY_FORM_APPROVED_HEAD_PRESENT",
                Some(head.head_transition_sha256.clone()),
                Some(head.canonical_sha256.clone()),
            )
        } else {
            check(
                "missing",
                "SECONDARY_FORM_APPROVED_HEAD_MISSING",
                None,
                None,
            )
        },
    );
    let high_observed = formal.is_some_and(|value| {
        value.high_exists
            && value.high_artifact_sha256.is_some()
            && value.high_artifact_readback_object_sha256.is_some()
    });
    checks.insert(
        "formal_high_artifact".to_owned(),
        if high_observed {
            let formal = formal.expect("checked observed formal high");
            check(
                "blocked",
                "FORMAL_HIGH_ARTIFACT_UNVERIFIED",
                formal.high_artifact_sha256.clone(),
                None,
            )
        } else {
            check("missing", "FORMAL_HIGH_ARTIFACT_MISSING", None, None)
        },
    );
    checks.insert(
        "authoring_low_topology".to_owned(),
        check(
            "blocked",
            "AUTHORING_QUAD_TOPOLOGY_NOT_PROVEN",
            formal.and_then(|value| value.low_artifact_sha256.clone()),
            None,
        ),
    );
    checks.insert(
        "hero_uv_layout".to_owned(),
        check("missing", "HERO_UV_NOT_RUN", None, None),
    );
    let cage_observed = formal.is_some_and(|value| {
        value.cage_exists && value.cage_artifact_readback_object_sha256.is_some()
    });
    checks.insert(
        "formal_cage_artifact".to_owned(),
        if cage_observed {
            let formal = formal.expect("checked observed formal cage");
            check(
                "blocked",
                "FORMAL_CAGE_ARTIFACT_UNVERIFIED",
                Some(formal.cage_artifact_sha256.clone()),
                None,
            )
        } else {
            check("missing", "FORMAL_CAGE_ARTIFACT_MISSING", None, None)
        },
    );
    let correspondence_observed = formal.is_some_and(|value| value.correspondence_exists);
    checks.insert(
        "high_low_correspondence".to_owned(),
        if correspondence_observed {
            let formal = formal.expect("checked observed formal correspondence");
            check(
                "blocked",
                "HIGH_LOW_CORRESPONDENCE_UNVERIFIED",
                formal.correspondence_object_sha256.clone(),
                None,
            )
        } else {
            check("missing", "HIGH_LOW_CORRESPONDENCE_MISSING", None, None)
        },
    );
    let diagnostic_observed = formal.is_some_and(|value| value.diagnostic_exists);
    checks.insert(
        "ray_diagnostic".to_owned(),
        if diagnostic_observed {
            let formal = formal.expect("checked observed diagnostic");
            check(
                "blocked",
                "RAY_DIAGNOSTIC_UNVERIFIED",
                formal.diagnostic_object_sha256.clone(),
                None,
            )
        } else {
            check("missing", "RAY_DIAGNOSTIC_NOT_RUN", None, None)
        },
    );
    let bake_observed = formal.is_some_and(|value| value.receipt_exists);
    checks.insert(
        "formal_bake".to_owned(),
        if bake_observed {
            let formal = formal.expect("checked observed formal bake");
            check(
                "blocked",
                "FORMAL_BAKE_UNVERIFIED",
                Some(formal.receipt_object_sha256.clone()),
                Some(formal.canonical_sha256.clone()),
            )
        } else {
            check("missing", "FORMAL_BAKE_NOT_REACHED", None, None)
        },
    );

    let blocking_reasons = checks
        .iter()
        .filter(|(name, check)| name.as_str() != "formal_bake" && check.status != "passed")
        .map(|(_, check)| check.reason_code.clone())
        .collect::<Vec<_>>();
    let mut result = ProductionWeaponHighLowBakePreflightGetResult {
        schema_version: PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREFLIGHT_GET_RESULT_SCHEMA_VERSION
            .to_owned(),
        preflight_id: request.preflight_id.clone(),
        session_id: request.session_id.clone(),
        project_id: request.project_id.clone(),
        candidate_id: request.candidate_id.clone(),
        expected_head_stage: request.expected_head_stage.clone(),
        observed_head_stage: sources.head.as_ref().map(|head| head.head_stage.clone()),
        observed_head_transition_id: sources
            .head
            .as_ref()
            .map(|head| head.head_transition_id.clone()),
        observed_head_transition_sha256: sources
            .head
            .as_ref()
            .map(|head| head.head_transition_sha256.clone()),
        observed_head_canonical_sha256: sources
            .head
            .as_ref()
            .map(|head| head.canonical_sha256.clone()),
        ready_for_formal_bake: blocking_reasons.is_empty(),
        blocking_reasons,
        checks,
        quality_status: "structural_only".to_owned(),
        visual_quality_status: "NOT_PROVEN".to_owned(),
        human_review_status: "NOT_RUN".to_owned(),
        commercial_engine_status: "NOT_RUN".to_owned(),
        distribution_status: "NOT_RUN".to_owned(),
        runtime_write: false,
        worker_started: false,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        restart_hash_verified: true,
        readiness_sha256: String::new(),
    };
    let mut preimage = serde_json::to_value(&result).map_err(|error| {
        invalid(format!(
            "HighLowBake preflight serialization failed: {error}"
        ))
    })?;
    preimage["readiness_sha256"] = Value::String(String::new());
    result.readiness_sha256 = canonical_json_hash(&preimage);
    Ok(result)
}

impl Runtime {
    pub fn production_weapon_high_low_bake_preflight_get(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_request(&value)?;
        serde_json::to_value(build_result(self, &request)?).map_err(|error| {
            invalid(format!(
                "HighLowBake preflight result serialization failed: {error}"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> Value {
        let mut value = json!({
            "schema_version":PRODUCTION_WEAPON_HIGH_LOW_BAKE_PREFLIGHT_GET_REQUEST_SCHEMA_VERSION,
            "preflight_id":"high-low-preflight-1",
            "session_id":"session-1",
            "project_id":"project-1",
            "candidate_id":"candidate-1",
            "expected_head_stage":"camera-calibrated",
            "expected_head_transition_id":"transition-1",
            "expected_head_transition_sha256":"a".repeat(64),
            "expected_head_canonical_sha256":"b".repeat(64),
            "input_sha256":""
        });
        let mut preimage = value.as_object().expect("request").clone();
        preimage.remove("input_sha256");
        value["input_sha256"] = Value::String(canonical_json_hash(&Value::Object(preimage)));
        value
    }

    #[test]
    fn request_is_closed_hash_bound_and_stage_bound() {
        parse_request(&request()).expect("valid request");
        let mut unknown = request();
        unknown["script"] = Value::String("forbidden".to_owned());
        assert!(parse_request(&unknown).is_err());
        let mut tampered = request();
        tampered["candidate_id"] = Value::String("candidate-2".to_owned());
        assert!(parse_request(&tampered).is_err());
        let mut stage = request();
        stage["expected_head_stage"] = Value::String("material-approved".to_owned());
        let mut preimage = stage.as_object().expect("request").clone();
        preimage.remove("input_sha256");
        stage["input_sha256"] = Value::String(canonical_json_hash(&Value::Object(preimage)));
        assert!(parse_request(&stage).is_err());
    }

    #[test]
    fn empty_runtime_returns_blockers_without_cas_or_sqlite_writes() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let projects_before = runtime.store.list_projects().expect("projects before");
        let objects_before = runtime.store.cas().list_objects().expect("objects before");
        let result = runtime
            .production_weapon_high_low_bake_preflight_get(request())
            .expect("preflight projection");
        assert_eq!(result["ready_for_formal_bake"], false);
        assert_eq!(result["runtime_write"], false);
        assert_eq!(result["worker_started"], false);
        assert_eq!(result["production_stage_advanced"], false);
        assert_eq!(result["restart_hash_verified"], true);
        assert!(result["blocking_reasons"]
            .as_array()
            .expect("blockers")
            .iter()
            .any(|reason| reason == "SECONDARY_FORM_APPROVED_HEAD_MISSING"));
        assert_eq!(
            runtime.store.list_projects().expect("projects after").len(),
            projects_before.len()
        );
        assert_eq!(
            runtime.store.cas().list_objects().expect("objects after"),
            objects_before
        );
    }

    #[test]
    fn empty_runtime_projection_is_restart_stable() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let database = std::env::temp_dir().join(format!(
            "forgecad-high-low-preflight-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let first_runtime = Runtime::open(&database).expect("first runtime");
        let first = first_runtime
            .production_weapon_high_low_bake_preflight_get(request())
            .expect("first projection");
        drop(first_runtime);
        let second_runtime = Runtime::open(&database).expect("reopened runtime");
        let second = second_runtime
            .production_weapon_high_low_bake_preflight_get(request())
            .expect("restart projection");
        assert_eq!(first, second);
        drop(second_runtime);
        let _ = std::fs::remove_file(&database);
        let _ = std::fs::remove_dir_all(database.with_extension("cas"));
    }

    #[test]
    fn ipc_dispatch_is_read_only_and_returns_the_same_projection() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let expected = runtime
            .production_weapon_high_low_bake_preflight_get(request())
            .expect("direct projection");
        let actual = runtime
            .dispatch_ipc("production_weapon_high_low_bake_preflight_get", &request())
            .expect("IPC projection");
        assert_eq!(actual, expected);
        assert_eq!(actual["runtime_write"], false);
        assert_eq!(actual["worker_started"], false);
    }
}
