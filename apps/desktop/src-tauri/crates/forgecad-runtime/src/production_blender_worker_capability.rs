//! Read-only projection for the isolated Blender headless Worker evaluation.
//!
//! The evaluation candidate is deliberately represented as capability data,
//! not as an executable Worker.  This module has no Store, CAS, filesystem,
//! environment, process, or Worker dependency.  Runtime owns the projection
//! and recomputes its canonical hash from the typed result.

use super::{canonical_json_hash, Runtime, RuntimeError};
use forgecad_contracts::{
    BlenderWorkerCapability, BlenderWorkerCapabilityGetRequest, BlenderWorkerCapabilityGetResult,
    BLENDER_WORKER_CAPABILITY_ADOPTION_STATUS,
    BLENDER_WORKER_CAPABILITY_GET_REQUEST_SCHEMA_VERSION,
    BLENDER_WORKER_CAPABILITY_GET_RESULT_SCHEMA_VERSION, BLENDER_WORKER_CAPABILITY_ID,
    BLENDER_WORKER_CAPABILITY_LICENSE_SPDX, BLENDER_WORKER_CAPABILITY_SCHEMA_VERSION,
    BLENDER_WORKER_CAPABILITY_SOURCE_IDENTITY, BLENDER_WORKER_CAPABILITY_SOURCE_REVISION,
    BLENDER_WORKER_CAPABILITY_WORKER_ID, BLENDER_WORKER_CAPABILITY_WORKER_KIND,
};
use serde_json::{Map, Value};

const REQUEST_FIELDS: &[&str] = &["schema_version", "capability_id"];
const NOT_RUN: &str = "not-run";
const LICENSE_NAME: &str = "GNU General Public License version 2 or later";

const LIMITATIONS: &[&str] = &[
    "ADR-0028: approved-for-evaluation only; Blender is not accepted, integrated, executed, or packaged.",
    "ADR-0028: fixed binary, Recipe, frozen Python bundle, GPL/source-offer/legal, sandbox, resource, determinism, CAS-readback, rollback/restart, packaging, and fallback gates remain not-run/pending.",
    "Blender remains reference-only; arbitrary Python, addons, .blend state, paths, URLs, scripts, secrets, and network input are unavailable.",
    "Runtime remains the sole writer; no Blender Worker can write CAS or SQLite, advance Stage, confirm a candidate, create a version, or export.",
    "Capability unavailable falls back to the ForgeCAD Native Rust Worker; Runtime does not probe for a user Blender installation.",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(message.into())
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value.as_object().ok_or_else(|| {
        invalid("production_blender_worker_capability_get request must be an object")
    })?;
    if let Some(field) = object
        .keys()
        .find(|field| !fields.contains(&field.as_str()))
    {
        return Err(invalid(format!(
            "production_blender_worker_capability_get contains unsupported field {field}"
        )));
    }
    for field in fields {
        if !object.contains_key(*field) {
            return Err(invalid(format!(
                "production_blender_worker_capability_get missing {field}"
            )));
        }
    }
    Ok(object)
}

fn parse_request(value: &Value) -> Result<BlenderWorkerCapabilityGetRequest, RuntimeError> {
    exact_object(value, REQUEST_FIELDS)?;
    let request: BlenderWorkerCapabilityGetRequest = serde_json::from_value(value.clone())
        .map_err(|error| {
            invalid(format!(
                "invalid BlenderWorkerCapabilityGetRequest: {error}"
            ))
        })?;
    if request.schema_version != BLENDER_WORKER_CAPABILITY_GET_REQUEST_SCHEMA_VERSION {
        return Err(invalid(format!(
            "unsupported BlenderWorkerCapabilityGetRequest schema {}",
            request.schema_version
        )));
    }
    if request.capability_id != BLENDER_WORKER_CAPABILITY_ID {
        return Err(invalid(format!(
            "unknown Blender worker capability {}",
            request.capability_id
        )));
    }
    Ok(request)
}

fn capability() -> BlenderWorkerCapability {
    BlenderWorkerCapability {
        schema_version: BLENDER_WORKER_CAPABILITY_SCHEMA_VERSION.to_owned(),
        capability_id: BLENDER_WORKER_CAPABILITY_ID.to_owned(),
        worker_id: BLENDER_WORKER_CAPABILITY_WORKER_ID.to_owned(),
        worker_kind: BLENDER_WORKER_CAPABILITY_WORKER_KIND.to_owned(),
        source_identity: BLENDER_WORKER_CAPABILITY_SOURCE_IDENTITY.to_owned(),
        source_revision: BLENDER_WORKER_CAPABILITY_SOURCE_REVISION.to_owned(),
        adoption_status: BLENDER_WORKER_CAPABILITY_ADOPTION_STATUS.to_owned(),
        capability_status: "unavailable".to_owned(),
        binary_status: NOT_RUN.to_owned(),
        binary_sha256: None,
        recipe_id: None,
        recipe_version: None,
        recipe_status: NOT_RUN.to_owned(),
        recipe_sha256: None,
        python_bundle_status: NOT_RUN.to_owned(),
        python_bundle_sha256: None,
        license_name: LICENSE_NAME.to_owned(),
        license_spdx: BLENDER_WORKER_CAPABILITY_LICENSE_SPDX.to_owned(),
        license_status: NOT_RUN.to_owned(),
        license_file_sha256: None,
        license_full_text_sha256: None,
        sandbox_status: NOT_RUN.to_owned(),
        sandbox_sha256: None,
        determinism_status: NOT_RUN.to_owned(),
        determinism_sha256: None,
        package_gate_status: NOT_RUN.to_owned(),
        package_sha256: None,
        read_only: true,
        runtime_write_performed: false,
        worker_invoked: false,
        candidate_generated: false,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        limitations: LIMITATIONS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        canonical_sha256: String::new(),
    }
}

fn canonicalize_capability(
    mut value: BlenderWorkerCapability,
) -> Result<BlenderWorkerCapability, RuntimeError> {
    let mut preimage = serde_json::to_value(&value)
        .map_err(|error| invalid(format!("Blender capability serialization failed: {error}")))?;
    preimage["canonical_sha256"] = Value::String(String::new());
    value.canonical_sha256 = canonical_json_hash(&preimage);
    Ok(value)
}

fn projection(value: Value) -> Result<Value, RuntimeError> {
    let _request = parse_request(&value)?;
    let capability = canonicalize_capability(capability())?;
    let result = BlenderWorkerCapabilityGetResult {
        schema_version: BLENDER_WORKER_CAPABILITY_GET_RESULT_SCHEMA_VERSION.to_owned(),
        capability,
        read_only: true,
        runtime_write_performed: false,
        worker_invoked: false,
        candidate_generated: false,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
    };
    serde_json::to_value(result).map_err(|error| {
        invalid(format!(
            "Blender capability result serialization failed: {error}"
        ))
    })
}

impl Runtime {
    /// Return the fixed, unavailable Blender evaluation capability.  This
    /// route is intentionally pure: it never reads or writes Runtime state,
    /// starts a Worker, probes the host, or inspects a path/environment.
    pub fn production_blender_worker_capability_get(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let _ = self;
        projection(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_core::canonical_json_hash;
    use serde_json::json;

    fn request() -> Value {
        json!({
            "schema_version": BLENDER_WORKER_CAPABILITY_GET_REQUEST_SCHEMA_VERSION,
            "capability_id": BLENDER_WORKER_CAPABILITY_ID,
        })
    }

    #[test]
    fn request_is_closed_and_rejects_raw_execution_fields() {
        for field in ["path", "url", "script", "secret", "bytes_base64"] {
            let mut value = request();
            value[field] = Value::String("forbidden".to_owned());
            let error = projection(value).expect_err("closed request must reject extra fields");
            assert!(error.to_string().contains("unsupported field"));
        }
    }

    #[test]
    fn projection_is_unavailable_read_only_and_runtime_hashed() {
        let result = projection(request()).expect("capability projection");
        assert_eq!(
            result["schema_version"],
            BLENDER_WORKER_CAPABILITY_GET_RESULT_SCHEMA_VERSION
        );
        assert_eq!(
            result["capability"]["adoption_status"],
            "approved-for-evaluation"
        );
        assert_eq!(result["capability"]["capability_status"], "unavailable");
        for field in [
            "binary_sha256",
            "recipe_id",
            "recipe_version",
            "recipe_sha256",
            "python_bundle_sha256",
            "license_file_sha256",
            "license_full_text_sha256",
            "sandbox_sha256",
            "determinism_sha256",
            "package_sha256",
        ] {
            assert!(
                result["capability"][field].is_null(),
                "{field} must be null"
            );
        }
        for field in [
            "binary_status",
            "recipe_status",
            "python_bundle_status",
            "license_status",
            "sandbox_status",
            "determinism_status",
            "package_gate_status",
        ] {
            assert_eq!(result["capability"][field], NOT_RUN, "{field} status");
        }
        for field in [
            "runtime_write_performed",
            "worker_invoked",
            "candidate_generated",
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ] {
            assert_eq!(result["capability"][field], false, "capability {field}");
            assert_eq!(result[field], false, "result {field}");
        }
        assert_eq!(result["read_only"], true);
        let mut preimage = result["capability"].clone();
        preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            result["capability"]["canonical_sha256"],
            canonical_json_hash(&preimage)
        );
        assert!(result["capability"]["limitations"]
            .as_array()
            .expect("limitations")
            .iter()
            .any(|value| value
                .as_str()
                .is_some_and(|value| value.contains("ADR-0028"))));
    }

    #[test]
    fn projection_is_restart_stable_and_does_not_use_runtime_state() {
        let first_runtime = Runtime::ephemeral().expect("first runtime");
        let second_runtime = Runtime::ephemeral().expect("restarted runtime");
        let first = first_runtime
            .production_blender_worker_capability_get(request())
            .expect("first projection");
        let second = second_runtime
            .production_blender_worker_capability_get(request())
            .expect("replayed projection");
        assert_eq!(first, second);
    }

    #[test]
    fn projection_never_reports_worker_or_writes() {
        let result = projection(request()).expect("capability projection");
        assert_eq!(result["capability"]["worker_invoked"], false);
        assert_eq!(result["capability"]["runtime_write_performed"], false);
        assert_eq!(result["worker_invoked"], false);
        assert_eq!(result["runtime_write_performed"], false);
    }

    #[test]
    fn ipc_dispatch_is_zero_cas_and_sqlite_write() {
        let runtime = Runtime::ephemeral().expect("ephemeral runtime");
        let before_projects = runtime.store.list_projects().expect("projects before");
        let before_objects = runtime.store.cas().list_objects().expect("objects before");
        let result = runtime
            .dispatch_ipc("production_blender_worker_capability_get", &request())
            .expect("capability IPC dispatch");
        let after_projects = runtime.store.list_projects().expect("projects after");
        let after_objects = runtime.store.cas().list_objects().expect("objects after");
        assert_eq!(result, projection(request()).expect("direct projection"));
        assert_eq!(
            serde_json::to_value(before_projects).expect("projects before JSON"),
            serde_json::to_value(after_projects).expect("projects after JSON")
        );
        assert_eq!(before_objects, after_objects);
        assert_eq!(result["worker_invoked"], false);
        assert_eq!(result["runtime_write_performed"], false);
    }
}
