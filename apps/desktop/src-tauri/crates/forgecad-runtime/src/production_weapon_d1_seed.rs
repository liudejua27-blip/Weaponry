use crate::{canonical_json_hash, sha256_hex, Runtime, RuntimeError};
use serde_json::{json, Value};

const SEED_ID: &str = "fps-form-04a-d1";
const SEED_REVISION: &str = "production-weapon-d1-seed-v1";
const SEED_BYTES: &[u8] = include_bytes!("../assets/production-weapon-d1-seed-v1.json");

/// Materialize the closed first-party D1 authoring seed for one Runtime-owned
/// project. The caller can select only the project binding; topology,
/// operators, Part/MaterialZone bindings and budgets are compiled into the
/// Runtime and cannot be supplied through MCP or an arbitrary file path.
pub(crate) fn materialize(project_id: &str) -> Result<Value, RuntimeError> {
    if project_id.is_empty()
        || project_id.len() > 128
        || !project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Err(RuntimeError::InvalidInput(
            "PRODUCTION_WEAPON_D1_SEED_PROJECT_ID_INVALID".to_owned(),
        ));
    }
    let mut program: Value = serde_json::from_slice(SEED_BYTES).map_err(|error| {
        RuntimeError::InvalidInput(format!(
            "PRODUCTION_WEAPON_D1_SEED_EMBEDDED_JSON_INVALID: {error}"
        ))
    })?;
    program["project_id"] = Value::String(project_id.to_owned());
    program["operator_catalog_sha256"] =
        Value::String(forgecad_worker_protocol::operator_catalog_sha256());
    program
        .as_object_mut()
        .expect("embedded D1 seed root is checked above")
        .remove("canonical_sha256");
    // GeometryProgram@2 draft hashing is canonical JSON hashing. The ordinary
    // prepare path still sends the complete program through the fixed Worker,
    // which performs the authoritative schema/operator/budget validation
    // before any candidate or CAS root can be committed.
    let canonical_sha256 = canonical_json_hash(&program);
    program["canonical_sha256"] = Value::String(canonical_sha256);
    Ok(program)
}

pub(crate) fn manifest() -> Value {
    let mut value = json!({
        "schema_version":"ProductionWeaponD1SeedManifest@1",
        "seed_id":SEED_ID,
        "seed_revision":SEED_REVISION,
        "seed_bytes_sha256":sha256_hex(SEED_BYTES),
        "geometry_program_schema_version":"GeometryProgram@2",
        "part_count":23,
        "source_node_count":24,
        "materialization_policy":"runtime-owned-closed-first-party",
        "quality_status":"NOT_PROVEN",
        "promotion_eligible":false,
        "candidate_confirmed":false,
        "version_created":false,
        "production_stage_advanced":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    value
}

impl Runtime {
    /// Read-only description of the exact first-party seed compiled into this
    /// Runtime cohort. It does not imply candidate creation or visual quality.
    pub fn production_weapon_d1_seed_manifest(&self) -> Value {
        manifest()
    }

    /// Compile the closed seed through the ordinary Geometry candidate
    /// transaction. The only caller-selected bindings are an existing project,
    /// an optional existing base version, and an authorized Runtime reference.
    pub fn prepare_production_weapon_d1_seed_candidate(
        &self,
        project_id: &str,
        base_version_id: Option<&str>,
        reference_id: &str,
    ) -> Result<Value, RuntimeError> {
        if reference_id.is_empty() {
            return Err(RuntimeError::InvalidInput(
                "PRODUCTION_WEAPON_D1_SEED_REFERENCE_ID_INVALID".to_owned(),
            ));
        }
        let program = materialize(project_id)?;
        self.prepare_geometry_candidate(
            project_id,
            base_version_id,
            json!({
                "typed":"geometry",
                "reference_id":reference_id,
                "geometry_program":program
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    #[test]
    fn first_party_d1_seed_is_closed_hash_stable_and_compilable() {
        let first = materialize("project-d1-seed").expect("closed D1 seed");
        let second = materialize("project-d1-seed").expect("stable closed D1 seed");
        assert_eq!(first, second);
        assert_eq!(first["nodes"].as_array().map(Vec::len), Some(24));
        assert_eq!(first["part_outputs"].as_array().map(Vec::len), Some(23));
        assert_eq!(
            first["part_outputs"][3]["input_node_ids"],
            json!(["rear-stock", "rear-stock-lower-beam"])
        );
        let artifact = forgecad_geometry_worker::compile_geometry_program(&first)
            .expect("embedded D1 seed compiles through the fixed Worker");
        assert_eq!(artifact.part_ids.len(), 23);
        assert!(artifact.triangle_count > 0);
        let manifest = manifest();
        assert_eq!(manifest["quality_status"], "NOT_PROVEN");
        assert_eq!(manifest["promotion_eligible"], false);
    }

    #[test]
    #[ignore = "requires explicitly supplied authorized reference and fresh durable Runtime database"]
    fn first_party_d1_seed_candidate_is_durable_and_restart_safe() {
        let reference_path = std::env::var_os("FORGECAD_REAL_WEAPON_REFERENCE_PATH")
            .map(std::path::PathBuf::from)
            .expect("FORGECAD_REAL_WEAPON_REFERENCE_PATH must identify the authorized reference");
        let database_path = std::env::var_os("FORGECAD_REAL_WEAPON_RUNTIME_DATABASE_PATH")
            .map(std::path::PathBuf::from)
            .expect("FORGECAD_REAL_WEAPON_RUNTIME_DATABASE_PATH must identify a fresh database");
        assert!(
            !database_path.exists(),
            "D1 seed durable probe requires a fresh database path"
        );
        if let Some(parent) = database_path.parent() {
            std::fs::create_dir_all(parent).expect("D1 seed database parent");
        }
        let reference_bytes = std::fs::read(reference_path).expect("authorized reference bytes");
        let reference_sha256 = sha256_hex(&reference_bytes);
        assert_eq!(
            reference_sha256,
            "1964704a62ed7a841b4d49c370b8d46f4626e201daad29092a9c39a40b4c4109"
        );

        let runtime = Runtime::open(&database_path).expect("durable D1 seed Runtime");
        let project = runtime
            .create_project("FPS FORM 04AX durable D1 seed", json!({"profile":"mvp"}))
            .expect("D1 seed project");
        let reference = runtime
            .import_reference(&crate::ReferenceImportRequest {
                project_id: project.project_id.clone(),
                source: crate::ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: base64::engine::general_purpose::STANDARD
                        .encode(&reference_bytes),
                },
                authorization: crate::ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "User-authorized fictional FPS weapon concept board for D1 seed"
                        .to_owned(),
                },
                expected_sha256: Some(reference_sha256.clone()),
            })
            .expect("D1 seed reference import")
            .reference;
        let prepared = runtime
            .prepare_production_weapon_d1_seed_candidate(
                &project.project_id,
                None,
                &reference.reference_id,
            )
            .expect("closed D1 seed candidate");
        let candidate = prepared["candidate"]
            .as_object()
            .expect("D1 seed candidate object");
        let candidate_id = candidate["candidate_id"]
            .as_str()
            .expect("D1 seed candidate id")
            .to_owned();
        let candidate_state_sha256 = candidate["canonical_sha256"]
            .as_str()
            .expect("D1 seed candidate state")
            .to_owned();
        let artifact_sha256 = candidate["prepared_object_sha256"]
            .as_str()
            .expect("D1 seed artifact hash")
            .to_owned();
        let readback = runtime
            .artifact_readback(&artifact_sha256, &candidate_id)
            .expect("D1 seed ArtifactReadback");
        assert_eq!(readback["schema_version"], "ArtifactReadback@2");
        assert_eq!(readback["hard_gate_passed"], true);
        assert_eq!(readback["validator_status"], "passed");
        assert_eq!(readback["part_ids"].as_array().map(Vec::len), Some(23));
        assert_eq!(
            readback["source_node_ids"].as_array().map(Vec::len),
            Some(24)
        );
        let geometry_program_sha256 = readback["program_sha256"]
            .as_str()
            .expect("D1 seed GeometryProgram hash")
            .to_owned();
        let triangle_count = readback["triangle_count"]
            .as_u64()
            .expect("D1 seed triangle count");
        let manifest_before = runtime.production_weapon_d1_seed_manifest();
        drop(runtime);

        let reopened = Runtime::open(&database_path).expect("reopened D1 seed Runtime");
        let replay = reopened
            .artifact_readback(&artifact_sha256, &candidate_id)
            .expect("restarted D1 seed ArtifactReadback");
        assert_eq!(replay, readback);
        assert_eq!(
            reopened.production_weapon_d1_seed_manifest(),
            manifest_before
        );
        assert!(reopened
            .reference(&reference.reference_id)
            .expect("restarted D1 seed reference lookup")
            .is_some());

        println!(
            "FPS_FORM_04AX_D1_SEED={}",
            serde_json::to_string(&json!({
                "receipt_format":"forgecad-supplemental-evidence-v1",
                "receipt_kind":"FPS-FORM-04AX-DURABLE-D1-SEED",
                "seed_manifest":manifest_before,
                "project_id":project.project_id,
                "reference_id":reference.reference_id,
                "reference_sha256":reference_sha256,
                "candidate_id":candidate_id,
                "candidate_state_sha256":candidate_state_sha256,
                "artifact_sha256":artifact_sha256,
                "geometry_program_sha256":geometry_program_sha256,
                "part_count":23,
                "source_node_count":24,
                "triangle_count":triangle_count,
                "validator_status":"passed",
                "restart_hash_verified":true,
                "quality_status":"NOT_PROVEN",
                "promotion_eligible":false,
                "candidate_confirmed":false,
                "version_created":false,
                "production_stage_advanced":false,
                "export_performed":false
            }))
            .expect("04AX D1 seed receipt JSON")
        );
    }
}
