//! Runtime adoption boundary for the sealed Blender knife provider.
//!
//! Blender remains a disposable compute provider.  This module discovers only
//! the application-packaged bundle, reads the source GLB from Runtime CAS,
//! invokes the closed Rust launcher, independently rechecks every returned
//! byte, and adopts those bytes into CAS.  It intentionally exposes no caller
//! selected executable, script, path, URL, add-on, Python environment, or
//! Blender session.

use super::{
    canonical_json_bytes, canonical_json_hash, now_string, sha256_hex, Runtime, RuntimeError,
};
use forgecad_blender_worker::{
    KnifeBlenderInstall, KnifeBlenderWorker, KnifeWorkerRequest, KNIFE_BLENDER_REVISION,
    KNIFE_BLENDER_VERSION, KNIFE_OPERATION, KNIFE_WORKER_ID,
};
use forgecad_store::{
    weaponry_blender_artifact_set_sha256, weaponry_blender_execution_record_canonical_sha256,
    WeaponryBlenderArtifactRef, WeaponryBlenderExecutionCasBundle, WeaponryBlenderExecutionCommit,
    WeaponryBlenderExecutionStoreRecord, WeaponryBlenderPackageIdentity,
    WEAPONRY_BLENDER_EXECUTION_RECORD_SCHEMA, WEAPONRY_BLENDER_EXECUTION_STATUS,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const PACKAGED_MANIFEST: &str = "weaponry-blender-worker-manifest.json";
const PACKAGED_SCHEMA: &str = "WeaponryBlenderPackagedWorkerManifest@1";
const RELEASE_ELIGIBILITY_RELATIVE: &str = "compliance/release-eligibility.json";
const RELEASE_ELIGIBILITY_SCHEMA: &str = "WeaponryBlenderDistributionEligibility@1";
const JSON_MIME: &str = "application/json";
const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

struct DiscoveredInstall {
    install: KnifeBlenderInstall,
    package_manifest: Value,
    package_manifest_bytes: Vec<u8>,
    release_eligibility: Value,
    release_eligibility_bytes: Vec<u8>,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "WEAPONRY_BLENDER_PROVIDER_INVALID: {}",
        message.into()
    ))
}

fn candidate_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.join("../Resources/weaponry-blender-worker"));
            roots.push(parent.join("weaponry-blender-worker"));
        }
    }
    roots.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/weaponry-blender-worker"),
    );
    roots
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, RuntimeError> {
    let metadata = fs::metadata(path)
        .map_err(|error| invalid(format!("packaged resource is unavailable: {error}")))?;
    if metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(invalid("packaged resource size is outside its bound"));
    }
    fs::read(path).map_err(|error| invalid(format!("packaged resource could not be read: {error}")))
}

fn string<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, RuntimeError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("packaged manifest {pointer} is missing")))
}

fn sha(value: &Value, pointer: &str) -> Result<String, RuntimeError> {
    let value = string(value, pointer)?;
    if !forgecad_contracts::is_sha256(value) {
        return Err(invalid(format!(
            "packaged manifest {pointer} is not SHA-256"
        )));
    }
    Ok(value.to_owned())
}

fn verify_file(root: &Path, relative: &str, expected: &str) -> Result<(), RuntimeError> {
    let path = root.join(relative);
    let canonical_root = root
        .canonicalize()
        .map_err(|error| invalid(format!("packaged root is unavailable: {error}")))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|error| invalid(format!("packaged file is unavailable: {error}")))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(invalid("packaged file escaped the sealed resource root"));
    }
    let bytes = fs::read(&canonical_path)
        .map_err(|error| invalid(format!("packaged file could not be read: {error}")))?;
    if sha256_hex(&bytes) != expected {
        return Err(invalid("packaged file hash drifted"));
    }
    Ok(())
}

fn install_from_root(root: &Path) -> Result<DiscoveredInstall, RuntimeError> {
    let manifest_bytes = read_bounded(&root.join(PACKAGED_MANIFEST), MAX_MANIFEST_BYTES)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| invalid(format!("packaged manifest is invalid JSON: {error}")))?;
    if string(&manifest, "/schema_version")? != PACKAGED_SCHEMA
        || string(&manifest, "/worker/worker_id")? != KNIFE_WORKER_ID
        || string(&manifest, "/worker/operation")? != KNIFE_OPERATION
        || string(&manifest, "/blender/build_hash")? != KNIFE_BLENDER_REVISION
        || string(&manifest, "/policy/network")? != "disabled"
        || string(&manifest, "/policy/filesystem")? != "runtime_scratch_only"
        || string(&manifest, "/policy/script")? != "frozen_bundle_only"
        || string(&manifest, "/policy/blender_autoexec")? != "disabled"
        || manifest
            .pointer("/policy/runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid(
            "packaged Blender/Worker identity or policy drifted",
        ));
    }
    let version = string(&manifest, "/blender/version")?;
    if version != KNIFE_BLENDER_VERSION && version != format!("{KNIFE_BLENDER_VERSION} LTS") {
        return Err(invalid("packaged Blender version drifted"));
    }
    let canonical = sha(&manifest, "/canonical_sha256")?;
    let mut preimage = manifest.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical {
        return Err(invalid("packaged manifest canonical hash drifted"));
    }
    if !cfg!(debug_assertions)
        && manifest
            .pointer("/distribution_gates/release_eligible")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err(invalid(
            "packaged Blender provider is not release eligible; GPL/source/SBOM/legal gates remain open",
        ));
    }

    let executable_relative = string(&manifest, "/blender/executable_path")?;
    let executable_sha = sha(&manifest, "/blender/executable_sha256")?;
    let entrypoint_relative = string(&manifest, "/worker/entrypoint_path")?;
    let entrypoint_sha = sha(&manifest, "/worker/entrypoint_sha256")?;
    let source_manifest_relative = string(&manifest, "/worker/source_manifest_path")?;
    let source_manifest_sha = sha(&manifest, "/worker/source_manifest_sha256")?;
    verify_file(root, executable_relative, &executable_sha)?;
    verify_file(root, entrypoint_relative, &entrypoint_sha)?;
    verify_file(root, source_manifest_relative, &source_manifest_sha)?;

    let source_manifest_bytes =
        read_bounded(&root.join(source_manifest_relative), MAX_MANIFEST_BYTES)?;
    let source_manifest: Value = serde_json::from_slice(&source_manifest_bytes)
        .map_err(|error| invalid(format!("source manifest is invalid JSON: {error}")))?;
    if string(&source_manifest, "/worker_id")? != KNIFE_WORKER_ID
        || string(&source_manifest, "/operation")? != KNIFE_OPERATION
        || string(&source_manifest, "/host/blender_version")? != KNIFE_BLENDER_VERSION
        || string(&source_manifest, "/host/source_revision")? != KNIFE_BLENDER_REVISION
        || string(&source_manifest, "/entrypoint_hash_policy")?
            != "DERIVED_FROM_STAGED_ENTRYPOINT_BYTES"
    {
        return Err(invalid("source manifest identity drifted"));
    }
    if source_manifest
        .pointer("/entrypoint_sha256")
        .and_then(Value::as_str)
        .is_some_and(|hash| hash != entrypoint_sha)
    {
        return Err(invalid("source manifest entrypoint hash drifted"));
    }
    let _dependency_lock_sha256 = sha(&source_manifest, "/dependency_lock_sha256")?;
    let _resource_tree_sha256 = sha(&manifest, "/resource_tree_sha256")?;
    let release_eligibility_bytes =
        read_bounded(&root.join(RELEASE_ELIGIBILITY_RELATIVE), MAX_MANIFEST_BYTES)?;
    let release_eligibility: Value = serde_json::from_slice(&release_eligibility_bytes)
        .map_err(|error| invalid(format!("release eligibility is invalid JSON: {error}")))?;
    if string(&release_eligibility, "/schema_version")? != RELEASE_ELIGIBILITY_SCHEMA
        || string(&release_eligibility, "/worker/worker_id")? != KNIFE_WORKER_ID
        || string(&release_eligibility, "/worker/entrypoint_sha256")? != entrypoint_sha
        || release_eligibility
            .pointer("/release_eligible")
            .and_then(Value::as_bool)
            != manifest
                .pointer("/distribution_gates/release_eligible")
                .and_then(Value::as_bool)
    {
        return Err(invalid(
            "release eligibility identity differs from the packaged Worker",
        ));
    }
    let release_canonical = sha(&release_eligibility, "/canonical_sha256")?;
    let mut release_preimage = release_eligibility.clone();
    release_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&release_preimage) != release_canonical {
        return Err(invalid("release eligibility canonical hash drifted"));
    }
    let install = KnifeBlenderInstall::from_packaged_manifest(root)
        .map_err(|error| invalid(error.to_string()))?;
    Ok(DiscoveredInstall {
        install,
        package_manifest: manifest,
        package_manifest_bytes: manifest_bytes,
        release_eligibility,
        release_eligibility_bytes,
    })
}

fn discover_install() -> Result<DiscoveredInstall, RuntimeError> {
    let mut errors = Vec::new();
    for root in candidate_roots() {
        if !root.join(PACKAGED_MANIFEST).is_file() {
            continue;
        }
        match install_from_root(&root) {
            Ok(install) => return Ok(install),
            Err(error) => errors.push(error.to_string()),
        }
    }
    Err(invalid(if errors.is_empty() {
        "fixed Blender provider is not installed in an application resource root".to_owned()
    } else {
        format!(
            "fixed Blender provider failed validation: {}",
            errors.join(" | ")
        )
    }))
}

fn cas_kind(output_kind: &str) -> Result<String, RuntimeError> {
    let allowed = [
        "high_glb",
        "low_glb",
        "cage_glb",
        "normal_map",
        "ao_map",
        "curvature_map",
        "thickness_map",
        "material_id_map",
        "worker_manifest",
    ];
    if !allowed.contains(&output_kind) {
        return Err(invalid(format!(
            "Worker output kind {output_kind} is not allowlisted"
        )));
    }
    Ok(format!("weaponry-blender-{output_kind}@1"))
}

// This is deliberately an internal adoption gate.  It is not a public
// contract and is not exposed through MCP.  The fixed Blender worker's Rust
// wire validates the shape of the response, while this gate validates the
// bytes that the response claims to describe before any of them enter CAS.
const STRICT_MANIFEST_SCHEMA: &str = "WeaponryBlenderKnifeWorkerManifest@1";
const STRICT_MANIFEST_POLICY: &str =
    "fixed-built-in-bevel-weighted-normal-decimate-smart-uv-cycles-bake@1";
const STRICT_MANIFEST_NORMALIZATION: &str = "standard-position-normal-uv-only-scratch-copy@1";
const STRICT_SURFACE_SIGNAL_SCHEMA: &str = "weaponry.surface-signals@1";
const STRICT_SURFACE_SIGNAL_STORAGE: &str =
    "glb-material-slots-object-extras-and-bounded-attributes@1";
const STRICT_CAGE_STORAGE: &str = "temporary-bake-participant-not-product-truth";

#[derive(Debug, Clone)]
struct StrictPartBinding {
    part_id: String,
    source_object: String,
    role: String,
    high_object: String,
    low_object: String,
    material_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct StrictOutputRecord {
    kind: String,
    relative_path: String,
    mime: String,
    sha256: String,
}

#[derive(Debug, Clone)]
struct StrictAccessor {
    start: usize,
    stride: usize,
    count: usize,
    component_type: u64,
    component_size: usize,
    component_count: usize,
    type_name: String,
}

struct StrictGlb<'a> {
    json: Value,
    bin: &'a [u8],
}

fn strict_invalid(message: impl Into<String>) -> RuntimeError {
    invalid(format!("strict readback failed: {}", message.into()))
}

fn strict_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<&'a Value, RuntimeError> {
    object
        .get(key)
        .ok_or_else(|| strict_invalid(format!("{label}.{key} is missing")))
}

fn strict_object<'a>(
    value: &'a Value,
    label: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    value
        .as_object()
        .ok_or_else(|| strict_invalid(format!("{label} is not an object")))
}

fn strict_array<'a>(value: &'a Value, label: &str) -> Result<&'a Vec<Value>, RuntimeError> {
    value
        .as_array()
        .ok_or_else(|| strict_invalid(format!("{label} is not an array")))
}

fn strict_string_value(value: &Value, label: &str) -> Result<String, RuntimeError> {
    let text = value
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| strict_invalid(format!("{label} is not a non-empty string")))?;
    if text.len() > 1024 {
        return Err(strict_invalid(format!("{label} exceeds its bound")));
    }
    Ok(text.to_owned())
}

fn strict_string_field(
    object: &Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<String, RuntimeError> {
    strict_string_value(strict_field(object, key, label)?, &format!("{label}.{key}"))
}

fn strict_u64_value(value: &Value, label: &str) -> Result<u64, RuntimeError> {
    value
        .as_u64()
        .ok_or_else(|| strict_invalid(format!("{label} is not an unsigned integer")))
}

fn strict_u64_field(
    object: &Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<u64, RuntimeError> {
    strict_u64_value(strict_field(object, key, label)?, &format!("{label}.{key}"))
}

fn strict_bool_field(
    object: &Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<bool, RuntimeError> {
    strict_field(object, key, label)?
        .as_bool()
        .ok_or_else(|| strict_invalid(format!("{label}.{key} is not a boolean")))
}

fn strict_sha_value(value: &Value, label: &str) -> Result<String, RuntimeError> {
    let text = strict_string_value(value, label)?;
    if !forgecad_contracts::is_sha256(&text) {
        return Err(strict_invalid(format!("{label} is not SHA-256")));
    }
    Ok(text)
}

fn strict_sha_field(
    object: &Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<String, RuntimeError> {
    strict_sha_value(strict_field(object, key, label)?, &format!("{label}.{key}"))
}

fn strict_require_exact_fields(
    object: &Map<String, Value>,
    expected: &[&str],
    label: &str,
) -> Result<(), RuntimeError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(strict_invalid(format!("{label} fields are not closed")));
    }
    Ok(())
}

fn strict_id(value: &Value, label: &str) -> Result<String, RuntimeError> {
    let text = strict_string_value(value, label)?;
    if text.len() > 128
        || !text.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && b"_.-".contains(&byte))
        })
    {
        return Err(strict_invalid(format!(
            "{label} is not a closed identifier"
        )));
    }
    Ok(text)
}

fn strict_finite_number(value: &Value, label: &str) -> Result<f64, RuntimeError> {
    let number = value
        .as_f64()
        .ok_or_else(|| strict_invalid(format!("{label} is not a number")))?;
    if !number.is_finite() {
        return Err(strict_invalid(format!("{label} is not finite")));
    }
    Ok(number)
}

fn strict_optional_u64(
    object: &Map<String, Value>,
    key: &str,
    label: &str,
) -> Result<u64, RuntimeError> {
    object
        .get(key)
        .map(|value| strict_u64_value(value, &format!("{label}.{key}")))
        .transpose()
        .map(|value| value.unwrap_or(0))
}

fn strict_manifest_parts(
    manifest: &Map<String, Value>,
    result: &Map<String, Value>,
) -> Result<Vec<StrictPartBinding>, RuntimeError> {
    let parts = strict_array(
        strict_field(manifest, "part_operations", "worker manifest")?,
        "worker manifest.part_operations",
    )?;
    if parts.is_empty() || parts.len() > 128 {
        return Err(strict_invalid(
            "worker manifest part count is outside its bound",
        ));
    }
    let expected_fields = [
        "high_bevel_width_m",
        "high_object",
        "high_subdivision",
        "high_surface_pass",
        "high_uv_loop_count",
        "low_bevel_applied",
        "low_bevel_width_m",
        "low_decimate_ratio",
        "low_object",
        "low_surface_pass",
        "low_uv_loop_count",
        "material_ids",
        "part_id",
        "role",
        "source_object",
        "source_triangle_count",
        "uv_quantization_grid_denominator",
    ];
    let mut seen = BTreeSet::new();
    let mut output = Vec::with_capacity(parts.len());
    for (index, value) in parts.iter().enumerate() {
        let object = strict_object(value, &format!("worker manifest.part_operations[{index}]"))?;
        strict_require_exact_fields(object, &expected_fields, "worker manifest.part_operation")?;
        let part_id = strict_id(
            strict_field(object, "part_id", "worker manifest part")?,
            "worker manifest part_id",
        )?;
        if !seen.insert(part_id.clone()) {
            return Err(strict_invalid("worker manifest contains duplicate part_id"));
        }
        let source_object = strict_string_field(object, "source_object", "worker manifest part")?;
        let role = strict_id(
            strict_field(object, "role", "worker manifest part")?,
            "worker manifest role",
        )?;
        let high_object = strict_string_field(object, "high_object", "worker manifest part")?;
        let low_object = strict_string_field(object, "low_object", "worker manifest part")?;
        let material_values = strict_array(
            strict_field(object, "material_ids", "worker manifest part")?,
            "worker manifest part.material_ids",
        )?;
        if material_values.is_empty() || material_values.len() > 64 {
            return Err(strict_invalid(
                "worker manifest material id count is outside its bound",
            ));
        }
        let mut material_ids = Vec::with_capacity(material_values.len());
        for (material_index, value) in material_values.iter().enumerate() {
            let material_id = strict_id(
                value,
                &format!("worker manifest part.material_ids[{material_index}]"),
            )?;
            if material_ids.contains(&material_id) {
                return Err(strict_invalid(
                    "worker manifest contains duplicate material_id",
                ));
            }
            material_ids.push(material_id);
        }
        let source_triangle_count =
            strict_u64_field(object, "source_triangle_count", "worker manifest part")?;
        if source_triangle_count == 0 {
            return Err(strict_invalid(
                "worker manifest source triangle count is zero",
            ));
        }
        let high_uv_loop_count =
            strict_u64_field(object, "high_uv_loop_count", "worker manifest part")?;
        let low_uv_loop_count =
            strict_u64_field(object, "low_uv_loop_count", "worker manifest part")?;
        if high_uv_loop_count == 0 || low_uv_loop_count == 0 {
            return Err(strict_invalid("worker manifest UV loop count is zero"));
        }
        if strict_u64_field(
            object,
            "uv_quantization_grid_denominator",
            "worker manifest part",
        )? != 65_536
        {
            return Err(strict_invalid("worker manifest UV quantization drifted"));
        }
        for key in [
            "high_bevel_width_m",
            "low_bevel_width_m",
            "low_decimate_ratio",
        ] {
            let number = strict_finite_number(
                strict_field(object, key, "worker manifest part")?,
                &format!("worker manifest part.{key}"),
            )?;
            if number < 0.0 || (key == "low_decimate_ratio" && number > 1.0) {
                return Err(strict_invalid(format!(
                    "worker manifest part.{key} is outside its bound"
                )));
            }
        }
        let _ = strict_bool_field(object, "high_subdivision", "worker manifest part")?;
        let _ = strict_bool_field(object, "low_bevel_applied", "worker manifest part")?;
        strict_object(
            strict_field(object, "high_surface_pass", "worker manifest part")?,
            "worker manifest part.high_surface_pass",
        )?;
        strict_object(
            strict_field(object, "low_surface_pass", "worker manifest part")?,
            "worker manifest part.low_surface_pass",
        )?;
        output.push(StrictPartBinding {
            part_id,
            source_object,
            role,
            high_object,
            low_object,
            material_ids,
        });
    }

    let stats = strict_object(
        strict_field(result, "stats", "worker result")?,
        "worker result.stats",
    )?;
    for key in [
        "source_object_count",
        "high_object_count",
        "low_object_count",
    ] {
        if strict_u64_field(stats, key, "worker result.stats")? != output.len() as u64 {
            return Err(strict_invalid(format!(
                "worker result.stats.{key} is not bound to manifest parts"
            )));
        }
    }
    Ok(output)
}

fn strict_validate_worker_manifest(
    request: &KnifeWorkerRequest,
    result: &Map<String, Value>,
    result_outputs: &[Value],
    records: &[StrictOutputRecord],
    manifest_bytes: &[u8],
) -> Result<(Value, Vec<StrictPartBinding>), RuntimeError> {
    let manifest: Value = serde_json::from_slice(manifest_bytes)
        .map_err(|error| strict_invalid(format!("worker manifest JSON is invalid: {error}")))?;
    let manifest_object = strict_object(&manifest, "worker manifest")?;
    strict_require_exact_fields(
        manifest_object,
        &[
            "blender_build_hash",
            "blender_import_normalization",
            "blender_revision",
            "blender_version",
            "cages",
            "candidate_confirmed",
            "candidate_id",
            "canonical_sha256",
            "checks",
            "dependency_lock_sha256",
            "export_performed",
            "implementation_profile",
            "maps",
            "operation",
            "outputs",
            "part_operations",
            "policy",
            "project_id",
            "recipe_sha256",
            "request_id",
            "runtime_write_performed",
            "schema_version",
            "source_authoring_mesh_sha256",
            "stage_advanced",
            "surface_signals",
            "stats",
            "version_created",
            "worker_entrypoint_sha256",
            "worker_id",
            "worker_version",
        ],
        "worker manifest",
    )?;
    if strict_string_field(manifest_object, "schema_version", "worker manifest")?
        != STRICT_MANIFEST_SCHEMA
        || strict_string_field(manifest_object, "operation", "worker manifest")?
            != request.operation
        || strict_string_field(manifest_object, "request_id", "worker manifest")?
            != request.request_id
        || strict_string_field(manifest_object, "project_id", "worker manifest")?
            != request.project_id
        || strict_string_field(manifest_object, "candidate_id", "worker manifest")?
            != request.candidate_id
        || strict_string_field(
            manifest_object,
            "source_authoring_mesh_sha256",
            "worker manifest",
        )? != request.input_glb.sha256
        || strict_string_field(manifest_object, "recipe_sha256", "worker manifest")?
            != request.recipe_sha256
        || strict_string_field(manifest_object, "policy", "worker manifest")?
            != STRICT_MANIFEST_POLICY
        || strict_string_field(manifest_object, "worker_id", "worker manifest")? != KNIFE_WORKER_ID
        || strict_string_field(manifest_object, "worker_version", "worker manifest")?
            != forgecad_blender_worker::KNIFE_WORKER_VERSION
        || strict_string_field(manifest_object, "blender_version", "worker manifest")?
            != KNIFE_BLENDER_VERSION
        || strict_string_field(manifest_object, "blender_revision", "worker manifest")?
            != KNIFE_BLENDER_REVISION
        || strict_string_field(manifest_object, "blender_build_hash", "worker manifest")?
            != KNIFE_BLENDER_REVISION
        || strict_string_field(
            manifest_object,
            "blender_import_normalization",
            "worker manifest",
        )? != STRICT_MANIFEST_NORMALIZATION
        || strict_sha_field(
            manifest_object,
            "worker_entrypoint_sha256",
            "worker manifest",
        )? != strict_string_field(result, "worker_entrypoint_sha256", "worker result")?
        || strict_sha_field(manifest_object, "dependency_lock_sha256", "worker manifest")?
            != strict_string_field(result, "dependency_lock_sha256", "worker result")?
    {
        return Err(strict_invalid(
            "worker manifest identity is not bound to the request/result",
        ));
    }
    for key in [
        "runtime_write_performed",
        "stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        if strict_bool_field(manifest_object, key, "worker manifest")? {
            return Err(strict_invalid(format!(
                "worker manifest.{key} unexpectedly promotes state"
            )));
        }
    }
    let supplied_hash = strict_sha_field(manifest_object, "canonical_sha256", "worker manifest")?;
    let mut preimage = manifest.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    let canonical_bytes = canonical_json_bytes(&manifest).map_err(|error| {
        strict_invalid(format!("worker manifest canonicalization failed: {error}"))
    })?;
    if canonical_bytes != manifest_bytes || canonical_json_hash(&preimage) != supplied_hash {
        return Err(strict_invalid(
            "worker manifest canonical bytes/hash drifted",
        ));
    }
    let implementation_profile = strict_object(
        strict_field(manifest_object, "implementation_profile", "worker manifest")?,
        "worker manifest.implementation_profile",
    )?;
    if strict_string_field(
        implementation_profile,
        "profile_id",
        "implementation profile",
    )? != "weaponry.knife.blender.high-low-uv-bake-enhanced@1"
        || strict_string_field(
            implementation_profile,
            "wire_compatibility",
            "implementation profile",
        )? != request.recipe_id
    {
        return Err(strict_invalid(
            "worker manifest implementation profile drifted",
        ));
    }
    if strict_field(manifest_object, "stats", "worker manifest")?
        != strict_field(result, "stats", "worker result")?
        || strict_field(manifest_object, "checks", "worker manifest")?
            != strict_field(result, "checks", "worker result")?
    {
        return Err(strict_invalid(
            "worker manifest stats/checks are not result-bound",
        ));
    }
    let internal_outputs = strict_array(
        strict_field(manifest_object, "outputs", "worker manifest")?,
        "worker manifest.outputs",
    )?;
    if records.len() < 3 || internal_outputs.len() != result_outputs.len() - 1 {
        return Err(strict_invalid(
            "worker manifest output count is not result-bound",
        ));
    }
    if internal_outputs != &result_outputs[..result_outputs.len() - 1] {
        return Err(strict_invalid(
            "worker manifest.outputs differs from worker result.outputs",
        ));
    }
    if records.last().map(|record| record.kind.as_str()) != Some("worker_manifest") {
        return Err(strict_invalid(
            "worker manifest record is not the final worker output",
        ));
    }

    let parts = strict_manifest_parts(manifest_object, result)?;
    strict_validate_cages(manifest_object, &parts)?;
    strict_validate_surface_signals(manifest_object, &parts)?;
    Ok((manifest, parts))
}

fn strict_validate_cages(
    manifest: &Map<String, Value>,
    parts: &[StrictPartBinding],
) -> Result<(), RuntimeError> {
    let cages = strict_array(
        strict_field(manifest, "cages", "worker manifest")?,
        "worker manifest.cages",
    )?;
    if cages.len() != parts.len() {
        return Err(strict_invalid(
            "worker manifest cage count is not bound to parts",
        ));
    }
    let expected_fields = [
        "applied_offset_m",
        "cage_object",
        "cage_topology_sha256",
        "independent",
        "low_object",
        "low_topology_sha256",
        "moved_vertex_count",
        "part_id",
        "policy",
        "polygon_count",
        "requested_offset_m",
        "role",
        "storage",
        "topology_preserved",
        "vertex_count",
    ];
    let mut seen = BTreeSet::new();
    for (index, value) in cages.iter().enumerate() {
        let object = strict_object(value, &format!("worker manifest.cages[{index}]"))?;
        strict_require_exact_fields(object, &expected_fields, "worker manifest.cage")?;
        let part_id = strict_id(
            strict_field(object, "part_id", "worker manifest cage")?,
            "worker manifest cage.part_id",
        )?;
        let part = parts
            .iter()
            .find(|part| part.part_id == part_id)
            .ok_or_else(|| strict_invalid("worker manifest cage references an unknown part"))?;
        if !seen.insert(part_id)
            || strict_string_field(object, "low_object", "worker manifest cage")? != part.low_object
            || strict_string_field(object, "role", "worker manifest cage")? != part.role
            || strict_string_field(object, "policy", "worker manifest cage")?
                != "independent-low-derived-normal-offset@1"
            || strict_string_field(object, "storage", "worker manifest cage")?
                != STRICT_CAGE_STORAGE
            || !strict_bool_field(object, "independent", "worker manifest cage")?
            || !strict_bool_field(object, "topology_preserved", "worker manifest cage")?
        {
            return Err(strict_invalid("worker manifest cage binding drifted"));
        }
        let low_hash = strict_sha_field(object, "low_topology_sha256", "worker manifest cage")?;
        let cage_hash = strict_sha_field(object, "cage_topology_sha256", "worker manifest cage")?;
        if low_hash != cage_hash {
            return Err(strict_invalid(
                "worker manifest cage topology hash differs from Low",
            ));
        }
        let vertex_count = strict_u64_field(object, "vertex_count", "worker manifest cage")?;
        let polygon_count = strict_u64_field(object, "polygon_count", "worker manifest cage")?;
        let moved_count = strict_u64_field(object, "moved_vertex_count", "worker manifest cage")?;
        if vertex_count == 0 || polygon_count == 0 || moved_count > vertex_count {
            return Err(strict_invalid("worker manifest cage counts are invalid"));
        }
        for key in ["applied_offset_m", "requested_offset_m"] {
            if strict_finite_number(
                strict_field(object, key, "worker manifest cage")?,
                &format!("worker manifest cage.{key}"),
            )? < 0.0
            {
                return Err(strict_invalid("worker manifest cage offset is negative"));
            }
        }
    }
    Ok(())
}

fn strict_validate_surface_signals(
    manifest: &Map<String, Value>,
    parts: &[StrictPartBinding],
) -> Result<(), RuntimeError> {
    let signals = strict_array(
        strict_field(manifest, "surface_signals", "worker manifest")?,
        "worker manifest.surface_signals",
    )?;
    if signals.len() != parts.len() * 2 {
        return Err(strict_invalid(
            "worker manifest surface signal count is not bound to parts",
        ));
    }
    let expected_fields = [
        "attributes",
        "color_attribute",
        "curvature",
        "material_id_encoding",
        "material_ids",
        "object",
        "part_id",
        "role",
        "schema",
        "signal_sha256",
        "thickness",
    ];
    let mut seen = BTreeSet::new();
    for (index, value) in signals.iter().enumerate() {
        let object = strict_object(value, &format!("worker manifest.surface_signals[{index}]"))?;
        strict_require_exact_fields(object, &expected_fields, "worker manifest.surface_signal")?;
        let part_id = strict_id(
            strict_field(object, "part_id", "worker manifest surface signal")?,
            "worker manifest surface signal.part_id",
        )?;
        let part = parts
            .iter()
            .find(|part| part.part_id == part_id)
            .ok_or_else(|| {
                strict_invalid("worker manifest surface signal references an unknown part")
            })?;
        let object_name = strict_string_field(object, "object", "worker manifest surface signal")?;
        let side = if object_name == part.high_object {
            "high"
        } else if object_name == part.low_object {
            "low"
        } else {
            return Err(strict_invalid(
                "worker manifest surface signal object is not High/Low bound",
            ));
        };
        if !seen.insert((part_id.clone(), side.to_owned()))
            || strict_string_field(object, "role", "worker manifest surface signal")? != part.role
            || strict_string_field(object, "schema", "worker manifest surface signal")?
                != STRICT_SURFACE_SIGNAL_SCHEMA
            || strict_string_field(
                object,
                "material_id_encoding",
                "worker manifest surface signal",
            )? != "sha256-first-24-bits-rgb@1"
            || strict_string_field(object, "color_attribute", "worker manifest surface signal")?
                != "WPN_SurfaceSignals"
        {
            return Err(strict_invalid(
                "worker manifest surface signal binding drifted",
            ));
        }
        let material_ids = strict_string_array(
            strict_field(object, "material_ids", "worker manifest surface signal")?,
            "worker manifest surface signal.material_ids",
        )?;
        if material_ids != part.material_ids {
            return Err(strict_invalid(
                "worker manifest surface signal materials drifted",
            ));
        }
        let attrs = strict_string_array(
            strict_field(object, "attributes", "worker manifest surface signal")?,
            "worker manifest surface signal.attributes",
        )?;
        if attrs
            != [
                "WPN_Curvature".to_owned(),
                "WPN_Thickness".to_owned(),
                "WPN_MaterialID".to_owned(),
            ]
        {
            return Err(strict_invalid(
                "worker manifest surface signal attributes drifted",
            ));
        }
        let signal_hash =
            strict_sha_field(object, "signal_sha256", "worker manifest surface signal")?;
        if signal_hash.is_empty() {
            return Err(strict_invalid(
                "worker manifest surface signal hash is empty",
            ));
        }
        for key in ["curvature", "thickness"] {
            strict_object(
                strict_field(object, key, "worker manifest surface signal")?,
                &format!("worker manifest surface signal.{key}"),
            )?;
        }
    }
    Ok(())
}

fn strict_string_array(value: &Value, label: &str) -> Result<Vec<String>, RuntimeError> {
    let values = strict_array(value, label)?;
    let mut output = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        output.push(strict_string_value(value, &format!("{label}[{index}]"))?);
    }
    Ok(output)
}

fn strict_output_records(
    result: &Map<String, Value>,
    artifacts: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<StrictOutputRecord>, RuntimeError> {
    let result_outputs = strict_array(
        strict_field(result, "outputs", "worker result")?,
        "worker result.outputs",
    )?;
    if result_outputs.len() < 3 || result_outputs.len() > 512 {
        return Err(strict_invalid(
            "worker result output count is outside its bound",
        ));
    }
    let expected_fields = [
        "kind",
        "relative_path",
        "mime",
        "byte_size",
        "sha256",
        "cas_owner",
        "durability",
    ];
    let mut seen = BTreeSet::new();
    let mut records = Vec::with_capacity(result_outputs.len());
    for (index, value) in result_outputs.iter().enumerate() {
        let object = strict_object(value, &format!("worker result.outputs[{index}]"))?;
        strict_require_exact_fields(object, &expected_fields, "worker result.output")?;
        let kind = strict_string_field(object, "kind", "worker result output")?;
        let relative_path = strict_string_field(object, "relative_path", "worker result output")?;
        if relative_path != relative_path.trim()
            || relative_path.is_empty()
            || relative_path.starts_with('/')
            || relative_path.contains("..")
            || !seen.insert(relative_path.clone())
        {
            return Err(strict_invalid(
                "worker result output path is unsafe or duplicated",
            ));
        }
        let mime = strict_string_field(object, "mime", "worker result output")?;
        let expected_mime = match kind.as_str() {
            "high_glb" | "low_glb" => "model/gltf-binary",
            "normal_map" | "ao_map" => "image/png",
            "worker_manifest" => "application/json",
            _ => {
                return Err(strict_invalid(format!(
                    "worker result output kind {kind} is not allowlisted"
                )))
            }
        };
        let expected_path = match kind.as_str() {
            "high_glb" => Some("output/dragonfang-high.blend.glb"),
            "low_glb" => Some("output/dragonfang-low.blend.glb"),
            "worker_manifest" => Some("output/manifest.json"),
            _ => None,
        };
        if expected_path.is_some_and(|expected| relative_path != expected) {
            return Err(strict_invalid(
                "worker result fixed artifact path differs from its kind",
            ));
        }
        if mime != expected_mime
            || strict_string_field(object, "cas_owner", "worker result output")? != "runtime"
            || strict_string_field(object, "durability", "worker result output")?
                != "pending_runtime_adoption"
        {
            return Err(strict_invalid(
                "worker result output MIME/ownership drifted",
            ));
        }
        let byte_size = strict_u64_field(object, "byte_size", "worker result output")?;
        let sha256 = strict_sha_field(object, "sha256", "worker result output")?;
        if byte_size == 0 || byte_size > forgecad_blender_worker::KNIFE_MAX_OUTPUT_BYTES as u64 {
            return Err(strict_invalid(
                "worker result output size is outside its bound",
            ));
        }
        let bytes = artifacts
            .get(&relative_path)
            .ok_or_else(|| strict_invalid("worker result output has no returned bytes"))?;
        if bytes.len() as u64 != byte_size || sha256_hex(bytes) != sha256 {
            return Err(strict_invalid(
                "worker result output hash/size differs from returned bytes",
            ));
        }
        records.push(StrictOutputRecord {
            kind,
            relative_path,
            mime,
            sha256,
        });
    }
    if artifacts.len() != records.len() || artifacts.keys().any(|path| !seen.contains(path)) {
        return Err(strict_invalid(
            "returned bytes are not exactly the result output list",
        ));
    }
    let kind_count = |kind: &str| records.iter().filter(|record| record.kind == kind).count();
    if kind_count("high_glb") != 1
        || kind_count("low_glb") != 1
        || kind_count("worker_manifest") != 1
        || records.last().map(|record| record.kind.as_str()) != Some("worker_manifest")
    {
        return Err(strict_invalid(
            "High/Low/manifest output kinds are not exact",
        ));
    }
    Ok(records)
}

fn strict_validate_map_records(
    manifest: &Map<String, Value>,
    parts: &[StrictPartBinding],
    records: &[StrictOutputRecord],
    artifacts: &BTreeMap<String, Vec<u8>>,
    texture_size: u32,
) -> Result<(), RuntimeError> {
    let maps = strict_array(
        strict_field(manifest, "maps", "worker manifest")?,
        "worker manifest.maps",
    )?;
    if maps.len() != parts.len() * 2 {
        return Err(strict_invalid(
            "worker manifest map count is not exactly two per part",
        ));
    }
    let expected_fields = [
        "kind",
        "part_id",
        "part_index",
        "relative_path",
        "role",
        "sha256",
    ];
    let records_by_path = records
        .iter()
        .map(|record| (record.relative_path.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for (index, value) in maps.iter().enumerate() {
        let object = strict_object(value, &format!("worker manifest.maps[{index}]"))?;
        strict_require_exact_fields(object, &expected_fields, "worker manifest.map")?;
        let part_index = strict_u64_field(object, "part_index", "worker manifest map")? as usize;
        let part = parts
            .get(part_index)
            .ok_or_else(|| strict_invalid("worker manifest map part_index is out of range"))?;
        let part_id = strict_id(
            strict_field(object, "part_id", "worker manifest map")?,
            "worker manifest map.part_id",
        )?;
        let role = strict_id(
            strict_field(object, "role", "worker manifest map")?,
            "worker manifest map.role",
        )?;
        let kind = strict_string_field(object, "kind", "worker manifest map")?;
        if part_id != part.part_id
            || role != part.role
            || (kind != "normal_map" && kind != "ao_map")
            || !seen.insert((part_index, kind.clone()))
        {
            return Err(strict_invalid(
                "worker manifest map part/role/kind binding drifted",
            ));
        }
        let relative_path = strict_string_field(object, "relative_path", "worker manifest map")?;
        let expected_suffix = if kind == "normal_map" {
            "-normal.png"
        } else {
            "-ao.png"
        };
        if !relative_path.starts_with("output/maps/") || !relative_path.ends_with(expected_suffix) {
            return Err(strict_invalid(
                "worker manifest map path is not a fixed PNG path",
            ));
        }
        let semantic_sha256 = strict_sha_field(object, "sha256", "worker manifest map")?;
        let record = records_by_path.get(relative_path.as_str()).ok_or_else(|| {
            strict_invalid("worker manifest map is absent from worker result outputs")
        })?;
        if record.kind != kind || record.mime != "image/png" || record.sha256 != semantic_sha256 {
            return Err(strict_invalid(
                "worker manifest map differs from its output record",
            ));
        }
        let bytes = artifacts
            .get(relative_path.as_str())
            .ok_or_else(|| strict_invalid("worker manifest map bytes are absent"))?;
        strict_validate_png_map(bytes, texture_size)?;
    }
    if seen.len() != maps.len() {
        return Err(strict_invalid("worker manifest has duplicate map binding"));
    }
    for index in 0..parts.len() {
        for kind in ["normal_map", "ao_map"] {
            if !seen.contains(&(index, kind.to_owned())) {
                return Err(strict_invalid(
                    "worker manifest is missing a per-part normal/AO map",
                ));
            }
        }
    }
    Ok(())
}

fn strict_validate_png_map(bytes: &[u8], texture_size: u32) -> Result<(), RuntimeError> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 33 || !bytes.starts_with(PNG_SIGNATURE) {
        return Err(strict_invalid("map is not a PNG with a valid signature"));
    }
    let mut cursor = 8usize;
    let mut saw_ihdr = false;
    let mut saw_idat = false;
    let mut saw_iend = false;
    while cursor < bytes.len() {
        if bytes.len() - cursor < 12 {
            return Err(strict_invalid("PNG chunk header/trailer is truncated"));
        }
        let length = u32::from_be_bytes(
            bytes[cursor..cursor + 4]
                .try_into()
                .map_err(|_| strict_invalid("PNG chunk length is invalid"))?,
        ) as usize;
        let chunk_type = &bytes[cursor + 4..cursor + 8];
        let end = cursor
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .ok_or_else(|| strict_invalid("PNG chunk length overflowed"))?;
        if end > bytes.len() {
            return Err(strict_invalid("PNG chunk exceeds map bytes"));
        }
        let chunk_data = &bytes[cursor + 8..cursor + 8 + length];
        if !saw_ihdr {
            if chunk_type != b"IHDR" || length != 13 {
                return Err(strict_invalid("PNG does not start with a 13-byte IHDR"));
            }
            let width = u32::from_be_bytes(chunk_data[0..4].try_into().unwrap());
            let height = u32::from_be_bytes(chunk_data[4..8].try_into().unwrap());
            if width != texture_size || height != texture_size {
                return Err(strict_invalid(format!(
                    "PNG dimensions {width}x{height} differ from {texture_size}x{texture_size}"
                )));
            }
            if chunk_data[8] != 8
                || (chunk_data[9] != 2 && chunk_data[9] != 6)
                || chunk_data[10] != 0
                || chunk_data[11] != 0
                || chunk_data[12] != 0
            {
                return Err(strict_invalid(
                    "PNG IHDR encoding is not the fixed 8-bit RGB/RGBA form",
                ));
            }
            saw_ihdr = true;
        } else if chunk_type == b"IHDR" {
            return Err(strict_invalid("PNG contains a duplicate IHDR"));
        } else if chunk_type == b"IDAT" {
            saw_idat = true;
        } else if chunk_type == b"IEND" {
            if length != 0 || saw_iend || end != bytes.len() {
                return Err(strict_invalid("PNG IEND is malformed or not final"));
            }
            saw_iend = true;
        }
        cursor = end;
    }
    if !saw_ihdr || !saw_idat || !saw_iend {
        return Err(strict_invalid("PNG is missing IHDR, IDAT, or IEND"));
    }
    Ok(())
}

fn strict_parse_glb(bytes: &[u8]) -> Result<StrictGlb<'_>, RuntimeError> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return Err(strict_invalid("GLB header is missing"));
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let declared_length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if version != 2 || declared_length != bytes.len() {
        return Err(strict_invalid(
            "GLB is not a version-2 file with an exact length",
        ));
    }
    let mut cursor = 12usize;
    let mut json_chunk: Option<&[u8]> = None;
    let mut bin_chunk: Option<&[u8]> = None;
    while cursor < bytes.len() {
        if bytes.len() - cursor < 8 {
            return Err(strict_invalid("GLB chunk header is truncated"));
        }
        let length = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        let chunk_type = u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap());
        if length % 4 != 0 {
            return Err(strict_invalid("GLB chunk length is not 4-byte aligned"));
        }
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .ok_or_else(|| strict_invalid("GLB chunk length overflowed"))?;
        if end > bytes.len() || length == 0 {
            return Err(strict_invalid("GLB chunk is empty or exceeds the file"));
        }
        match chunk_type {
            0x4E4F534A => {
                if json_chunk.is_some() || bin_chunk.is_some() {
                    return Err(strict_invalid(
                        "GLB JSON chunk is duplicated or out of order",
                    ));
                }
                json_chunk = Some(&bytes[start..end]);
            }
            0x004E4942 => {
                if json_chunk.is_none() || bin_chunk.is_some() {
                    return Err(strict_invalid(
                        "GLB BIN chunk is duplicated or out of order",
                    ));
                }
                bin_chunk = Some(&bytes[start..end]);
            }
            _ => return Err(strict_invalid("GLB contains an unallowlisted chunk type")),
        }
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(strict_invalid("GLB has trailing bytes"));
    }
    let json_chunk = json_chunk.ok_or_else(|| strict_invalid("GLB JSON chunk is missing"))?;
    let bin = bin_chunk.ok_or_else(|| strict_invalid("GLB BIN chunk is missing"))?;
    let mut json_end = json_chunk.len();
    while json_end > 0 && matches!(json_chunk[json_end - 1], b' ' | b'\t' | b'\r' | b'\n' | 0) {
        json_end -= 1;
    }
    let json_chunk = &json_chunk[..json_end];
    let json: Value = serde_json::from_slice(json_chunk)
        .map_err(|error| strict_invalid(format!("GLB JSON chunk is invalid: {error}")))?;
    let object = strict_object(&json, "GLB JSON")?;
    let asset = strict_object(
        object
            .get("asset")
            .ok_or_else(|| strict_invalid("GLB asset object is missing"))?,
        "GLB asset",
    )?;
    if strict_string_field(asset, "version", "GLB asset")? != "2.0" {
        return Err(strict_invalid("GLB asset.version is not 2.0"));
    }
    if bin.is_empty() {
        return Err(strict_invalid("GLB BIN chunk is empty"));
    }
    Ok(StrictGlb { json, bin })
}

fn strict_component_shape(component_type: u64, type_name: &str) -> Option<(usize, usize)> {
    let component_size = match component_type {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return None,
    };
    let component_count = match type_name {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        "MAT2" => 4,
        "MAT3" => 9,
        "MAT4" => 16,
        _ => return None,
    };
    Some((component_size, component_count))
}

fn strict_parse_accessors(glb: &StrictGlb<'_>) -> Result<Vec<StrictAccessor>, RuntimeError> {
    let object = strict_object(&glb.json, "GLB JSON")?;
    let buffers = strict_array(
        object
            .get("buffers")
            .ok_or_else(|| strict_invalid("GLB buffers are missing"))?,
        "GLB buffers",
    )?;
    if buffers.len() != 1 {
        return Err(strict_invalid("GLB must contain exactly one buffer"));
    }
    let buffer = strict_object(&buffers[0], "GLB buffer")?;
    if strict_u64_field(buffer, "byteLength", "GLB buffer")? > glb.bin.len() as u64 {
        return Err(strict_invalid("GLB buffer.byteLength exceeds BIN bytes"));
    }
    let buffer_views = strict_array(
        object
            .get("bufferViews")
            .ok_or_else(|| strict_invalid("GLB bufferViews are missing"))?,
        "GLB bufferViews",
    )?;
    if buffer_views.is_empty() || buffer_views.len() > 16_384 {
        return Err(strict_invalid("GLB bufferView count is outside its bound"));
    }
    let mut views = Vec::with_capacity(buffer_views.len());
    for (index, value) in buffer_views.iter().enumerate() {
        let view = strict_object(value, &format!("GLB bufferViews[{index}]"))?;
        if strict_u64_field(view, "buffer", "GLB bufferView")? != 0 {
            return Err(strict_invalid(
                "GLB bufferView references an unknown buffer",
            ));
        }
        let offset = strict_optional_u64(view, "byteOffset", "GLB bufferView")? as usize;
        let length = strict_u64_field(view, "byteLength", "GLB bufferView")? as usize;
        if length == 0
            || offset > glb.bin.len()
            || offset
                .checked_add(length)
                .is_none_or(|end| end > glb.bin.len())
        {
            return Err(strict_invalid("GLB bufferView range is invalid"));
        }
        let stride = view
            .get("byteStride")
            .map(|value| strict_u64_value(value, "GLB bufferView.byteStride"))
            .transpose()?
            .map(|value| value as usize);
        if let Some(stride) = stride {
            if !(4..=252).contains(&stride) || stride % 4 != 0 {
                return Err(strict_invalid("GLB bufferView byteStride is invalid"));
            }
        }
        views.push((offset, length, stride));
    }
    let accessors = strict_array(
        object
            .get("accessors")
            .ok_or_else(|| strict_invalid("GLB accessors are missing"))?,
        "GLB accessors",
    )?;
    if accessors.is_empty() || accessors.len() > 16_384 {
        return Err(strict_invalid("GLB accessor count is outside its bound"));
    }
    let mut output = Vec::with_capacity(accessors.len());
    for (index, value) in accessors.iter().enumerate() {
        let accessor = strict_object(value, &format!("GLB accessors[{index}]"))?;
        let view_index = strict_u64_field(accessor, "bufferView", "GLB accessor")? as usize;
        let (view_offset, view_length, view_stride) = views
            .get(view_index)
            .copied()
            .ok_or_else(|| strict_invalid("GLB accessor bufferView index is invalid"))?;
        let component_type = strict_u64_field(accessor, "componentType", "GLB accessor")?;
        let type_name = strict_string_field(accessor, "type", "GLB accessor")?;
        let (component_size, component_count) = strict_component_shape(component_type, &type_name)
            .ok_or_else(|| strict_invalid("GLB accessor componentType/type is unsupported"))?;
        let count = strict_u64_field(accessor, "count", "GLB accessor")? as usize;
        if count == 0 {
            return Err(strict_invalid("GLB accessor count is zero"));
        }
        let element_size = component_size
            .checked_mul(component_count)
            .ok_or_else(|| strict_invalid("GLB accessor element size overflowed"))?;
        let stride = view_stride.unwrap_or(element_size);
        if stride < element_size {
            return Err(strict_invalid(
                "GLB accessor stride is smaller than its element",
            ));
        }
        let byte_offset = strict_optional_u64(accessor, "byteOffset", "GLB accessor")? as usize;
        let start = view_offset
            .checked_add(byte_offset)
            .ok_or_else(|| strict_invalid("GLB accessor offset overflowed"))?;
        let required = stride
            .checked_mul(count - 1)
            .and_then(|value| value.checked_add(element_size))
            .ok_or_else(|| strict_invalid("GLB accessor range overflowed"))?;
        if start > view_offset + view_length
            || start
                .checked_add(required)
                .is_none_or(|end| end > view_offset + view_length)
            || start % component_size != 0
        {
            return Err(strict_invalid(
                "GLB accessor range is outside its bufferView",
            ));
        }
        if component_type == 5126
            && accessor
                .get("normalized")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            return Err(strict_invalid(
                "GLB floating-point accessor is unexpectedly normalized",
            ));
        }
        if let Some(minimum) = accessor.get("min") {
            let minimum = strict_array(minimum, "GLB accessor.min")?;
            if minimum.len() != component_count {
                return Err(strict_invalid(
                    "GLB accessor.min shape differs from accessor.type",
                ));
            }
            for (component, value) in minimum.iter().enumerate() {
                strict_finite_number(value, &format!("GLB accessor.min[{component}]"))?;
            }
        }
        if let Some(maximum) = accessor.get("max") {
            let maximum = strict_array(maximum, "GLB accessor.max")?;
            if maximum.len() != component_count {
                return Err(strict_invalid(
                    "GLB accessor.max shape differs from accessor.type",
                ));
            }
            for (component, value) in maximum.iter().enumerate() {
                strict_finite_number(value, &format!("GLB accessor.max[{component}]"))?;
            }
        }
        if accessor.contains_key("sparse") {
            return Err(strict_invalid(
                "GLB sparse accessors are not accepted by the closed worker",
            ));
        }
        output.push(StrictAccessor {
            start,
            stride,
            count,
            component_type,
            component_size,
            component_count,
            type_name,
        });
    }
    Ok(output)
}

fn strict_accessor<'a>(
    accessors: &'a [StrictAccessor],
    value: &Value,
    label: &str,
) -> Result<&'a StrictAccessor, RuntimeError> {
    let index = strict_u64_value(value, label)? as usize;
    accessors
        .get(index)
        .ok_or_else(|| strict_invalid(format!("{label} index is out of range")))
}

fn strict_read_f32(
    bin: &[u8],
    accessor: &StrictAccessor,
    element: usize,
    component: usize,
) -> Result<f32, RuntimeError> {
    if accessor.component_type != 5126
        || element >= accessor.count
        || component >= accessor.component_count
    {
        return Err(strict_invalid(
            "GLB floating-point accessor read is invalid",
        ));
    }
    let offset = accessor
        .start
        .checked_add(
            element
                .checked_mul(accessor.stride)
                .ok_or_else(|| strict_invalid("GLB accessor offset overflowed"))?,
        )
        .and_then(|value| {
            value.checked_add(
                component
                    .checked_mul(accessor.component_size)
                    .unwrap_or(usize::MAX),
            )
        })
        .ok_or_else(|| strict_invalid("GLB accessor component offset overflowed"))?;
    let bytes = bin
        .get(offset..offset + 4)
        .ok_or_else(|| strict_invalid("GLB floating-point accessor read exceeds BIN"))?;
    let number = f32::from_le_bytes(bytes.try_into().unwrap());
    if !number.is_finite() {
        return Err(strict_invalid("GLB geometry contains a non-finite value"));
    }
    Ok(number)
}

fn strict_read_index(
    bin: &[u8],
    accessor: &StrictAccessor,
    element: usize,
) -> Result<u32, RuntimeError> {
    if accessor.type_name != "SCALAR" || element >= accessor.count {
        return Err(strict_invalid("GLB index accessor shape is invalid"));
    }
    let offset = accessor
        .start
        .checked_add(
            element
                .checked_mul(accessor.stride)
                .ok_or_else(|| strict_invalid("GLB index offset overflowed"))?,
        )
        .ok_or_else(|| strict_invalid("GLB index offset overflowed"))?;
    match accessor.component_type {
        5121 => bin
            .get(offset)
            .copied()
            .map(u32::from)
            .ok_or_else(|| strict_invalid("GLB UNSIGNED_BYTE index exceeds BIN")),
        5123 => {
            let bytes = bin
                .get(offset..offset + 2)
                .ok_or_else(|| strict_invalid("GLB UNSIGNED_SHORT index exceeds BIN"))?;
            Ok(u16::from_le_bytes(bytes.try_into().unwrap()) as u32)
        }
        5125 => {
            let bytes = bin
                .get(offset..offset + 4)
                .ok_or_else(|| strict_invalid("GLB UNSIGNED_INT index exceeds BIN"))?;
            Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
        }
        _ => Err(strict_invalid("GLB index componentType is not unsigned")),
    }
}

fn strict_material_name_matches(name: &str, material_id: &str) -> bool {
    if name == material_id {
        return true;
    }
    let Some(suffix) = name.strip_prefix(&format!("{material_id}.")) else {
        return false;
    };
    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
}

fn strict_material_part_aliases(source_object: &str, part_id: &str) -> BTreeSet<String> {
    let mut aliases = BTreeSet::from([part_id.to_owned(), source_object.to_owned()]);
    if let Some((_, suffix)) = source_object.rsplit_once(':') {
        aliases.insert(suffix.to_owned());
    }
    aliases
}

fn strict_validate_mesh(
    glb: &StrictGlb<'_>,
    accessors: &[StrictAccessor],
    mesh: &Map<String, Value>,
    part: &StrictPartBinding,
    materials: &[Value],
    material_bindings: &BTreeMap<String, BTreeSet<String>>,
    material_owners: &mut BTreeMap<usize, String>,
) -> Result<u64, RuntimeError> {
    let primitives = strict_array(
        mesh.get("primitives")
            .ok_or_else(|| strict_invalid("GLB mesh.primitives is missing"))?,
        "GLB mesh.primitives",
    )?;
    if primitives.is_empty() {
        return Err(strict_invalid("GLB mesh has no primitives"));
    }
    let expected_material_ids = part.material_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut observed_material_ids = BTreeSet::new();
    let mut triangle_count = 0u64;
    for (primitive_index, primitive_value) in primitives.iter().enumerate() {
        let primitive = strict_object(
            primitive_value,
            &format!("GLB primitive[{primitive_index}]"),
        )?;
        if primitive
            .get("mode")
            .map(|value| strict_u64_value(value, "GLB primitive.mode"))
            .transpose()?
            .unwrap_or(4)
            != 4
        {
            return Err(strict_invalid("GLB primitive mode is not TRIANGLES"));
        }
        let attributes = strict_object(
            primitive
                .get("attributes")
                .ok_or_else(|| strict_invalid("GLB primitive.attributes is missing"))?,
            "GLB primitive.attributes",
        )?;
        let position = strict_accessor(
            accessors,
            attributes
                .get("POSITION")
                .ok_or_else(|| strict_invalid("GLB primitive POSITION is missing"))?,
            "GLB primitive POSITION",
        )?;
        let normal = strict_accessor(
            accessors,
            attributes
                .get("NORMAL")
                .ok_or_else(|| strict_invalid("GLB primitive NORMAL is missing"))?,
            "GLB primitive NORMAL",
        )?;
        let uv = strict_accessor(
            accessors,
            attributes
                .get("TEXCOORD_0")
                .ok_or_else(|| strict_invalid("GLB primitive TEXCOORD_0 is missing"))?,
            "GLB primitive TEXCOORD_0",
        )?;
        if position.component_type != 5126
            || position.type_name != "VEC3"
            || normal.component_type != 5126
            || normal.type_name != "VEC3"
            || uv.component_type != 5126
            || uv.type_name != "VEC2"
            || normal.count != position.count
            || uv.count != position.count
        {
            return Err(strict_invalid(
                "GLB primitive POSITION/NORMAL/TEXCOORD_0 shape is invalid",
            ));
        }
        for vertex in 0..position.count {
            for component in 0..3 {
                strict_read_f32(glb.bin, position, vertex, component)?;
                strict_read_f32(glb.bin, normal, vertex, component)?;
            }
            for component in 0..2 {
                strict_read_f32(glb.bin, uv, vertex, component)?;
            }
        }
        for (attribute_name, accessor_value) in attributes {
            let accessor = strict_accessor(
                accessors,
                accessor_value,
                &format!("GLB primitive attribute {attribute_name}"),
            )?;
            if accessor.count != position.count {
                return Err(strict_invalid(format!(
                    "GLB primitive attribute {attribute_name} count differs from POSITION"
                )));
            }
            if accessor.component_type == 5126 {
                for vertex in 0..accessor.count {
                    for component in 0..accessor.component_count {
                        strict_read_f32(glb.bin, accessor, vertex, component)?;
                    }
                }
            }
        }
        let indices = strict_accessor(
            accessors,
            primitive
                .get("indices")
                .ok_or_else(|| strict_invalid("GLB primitive indices are missing"))?,
            "GLB primitive indices",
        )?;
        if indices.component_count != 1
            || indices.type_name != "SCALAR"
            || indices.count == 0
            || indices.count % 3 != 0
        {
            return Err(strict_invalid(
                "GLB primitive indices are not a non-empty triangle index stream",
            ));
        }
        for index in 0..indices.count {
            if strict_read_index(glb.bin, indices, index)? >= position.count as u32 {
                return Err(strict_invalid("GLB primitive index is outside POSITION"));
            }
        }
        let material_index = strict_u64_value(
            primitive
                .get("material")
                .ok_or_else(|| strict_invalid("GLB primitive material is missing"))?,
            "GLB primitive material",
        )? as usize;
        let material = materials
            .get(material_index)
            .ok_or_else(|| strict_invalid("GLB primitive material index is out of range"))?;
        let material_object = strict_object(material, "GLB material")?;
        let material_name = strict_string_field(material_object, "name", "GLB material")?;
        let material_extras = strict_object(
            material_object
                .get("extras")
                .ok_or_else(|| strict_invalid("GLB material extras are missing"))?,
            "GLB material extras",
        )?;
        let material_zone_id = strict_id(
            strict_field(material_extras, "material_zone_id", "GLB material extras")?,
            "GLB material_zone_id",
        )?;
        let material_part_id =
            strict_string_field(material_extras, "part_id", "GLB material extras")?;
        let material_part_bound = material_bindings
            .get(&material_zone_id)
            .is_some_and(|aliases| aliases.contains(&material_part_id));
        if !expected_material_ids.contains(&material_zone_id)
            || !strict_material_name_matches(&material_name, &material_zone_id)
            || !material_part_bound
        {
            return Err(strict_invalid(
                "GLB material is not bound to the node part/material declaration",
            ));
        }
        if let Some(owner) = material_owners.get(&material_index) {
            if owner != &part.part_id {
                return Err(strict_invalid(
                    "GLB material index is shared across semantic parts",
                ));
            }
        } else {
            material_owners.insert(material_index, part.part_id.clone());
        }
        observed_material_ids.insert(material_zone_id);
        triangle_count = triangle_count
            .checked_add((indices.count / 3) as u64)
            .ok_or_else(|| strict_invalid("GLB triangle count overflowed"))?;
    }
    if observed_material_ids != expected_material_ids {
        return Err(strict_invalid(
            "GLB primitive materials do not cover the manifest material_ids",
        ));
    }
    Ok(triangle_count)
}

fn strict_validate_scene_reachability(glb: &StrictGlb<'_>) -> Result<(), RuntimeError> {
    let object = strict_object(&glb.json, "GLB JSON")?;
    let nodes = strict_array(
        object
            .get("nodes")
            .ok_or_else(|| strict_invalid("GLB nodes are missing"))?,
        "GLB nodes",
    )?;
    let scenes = strict_array(
        object
            .get("scenes")
            .ok_or_else(|| strict_invalid("GLB scenes are missing"))?,
        "GLB scenes",
    )?;
    let scene_index = strict_u64_value(
        object
            .get("scene")
            .ok_or_else(|| strict_invalid("GLB default scene is missing"))?,
        "GLB scene",
    )? as usize;
    let scene = strict_object(
        scenes
            .get(scene_index)
            .ok_or_else(|| strict_invalid("GLB default scene index is invalid"))?,
        "GLB default scene",
    )?;
    let roots = strict_array(
        scene
            .get("nodes")
            .ok_or_else(|| strict_invalid("GLB default scene nodes are missing"))?,
        "GLB default scene.nodes",
    )?;
    let mut visited = BTreeSet::new();
    let mut stack = roots
        .iter()
        .map(|value| strict_u64_value(value, "GLB scene node").map(|index| index as usize))
        .collect::<Result<Vec<_>, _>>()?;
    while let Some(index) = stack.pop() {
        if !visited.insert(index) {
            continue;
        }
        let node = nodes
            .get(index)
            .ok_or_else(|| strict_invalid("GLB scene references an invalid node"))?;
        let node = strict_object(node, "GLB node")?;
        if let Some(children) = node.get("children") {
            for child in strict_array(children, "GLB node.children")? {
                stack.push(strict_u64_value(child, "GLB child node")? as usize);
            }
        }
    }
    for (index, node) in nodes.iter().enumerate() {
        if strict_object(node, "GLB node")?.contains_key("mesh") && !visited.contains(&index) {
            return Err(strict_invalid(
                "GLB mesh node is not reachable from the default scene",
            ));
        }
    }
    Ok(())
}

fn strict_validate_glb(
    bytes: &[u8],
    parts: &[StrictPartBinding],
    variant: &str,
    expected_triangles: u64,
) -> Result<(), RuntimeError> {
    let glb = strict_parse_glb(bytes)?;
    strict_validate_scene_reachability(&glb)?;
    let accessors = strict_parse_accessors(&glb)?;
    let object = strict_object(&glb.json, "GLB JSON")?;
    let nodes = strict_array(
        object
            .get("nodes")
            .ok_or_else(|| strict_invalid("GLB nodes are missing"))?,
        "GLB nodes",
    )?;
    let meshes = strict_array(
        object
            .get("meshes")
            .ok_or_else(|| strict_invalid("GLB meshes are missing"))?,
        "GLB meshes",
    )?;
    let materials = strict_array(
        object
            .get("materials")
            .ok_or_else(|| strict_invalid("GLB materials are missing"))?,
        "GLB materials",
    )?;
    if nodes.len() > 128 || meshes.len() != parts.len() || materials.is_empty() {
        return Err(strict_invalid(
            "GLB High/Low object/material count is outside the closed binding",
        ));
    }
    let mut part_nodes = BTreeMap::new();
    let mut used_meshes = BTreeSet::new();
    for (node_index, value) in nodes.iter().enumerate() {
        let node = strict_object(value, &format!("GLB nodes[{node_index}]"))?;
        let Some(mesh_value) = node.get("mesh") else {
            continue;
        };
        let mesh_index = strict_u64_value(mesh_value, "GLB node.mesh")? as usize;
        if mesh_index >= meshes.len() || !used_meshes.insert(mesh_index) {
            return Err(strict_invalid(
                "GLB mesh node index is invalid or duplicated",
            ));
        }
        let extras = strict_object(
            node.get("extras")
                .ok_or_else(|| strict_invalid("GLB part node extras are missing"))?,
            "GLB part node extras",
        )?;
        let part_id = strict_id(
            strict_field(extras, "weaponry_part_id", "GLB part node extras")?,
            "GLB part node part_id",
        )?;
        let part = parts
            .iter()
            .find(|part| part.part_id == part_id)
            .ok_or_else(|| strict_invalid("GLB part node references an unknown manifest part"))?;
        let expected_object = if variant == "high" {
            &part.high_object
        } else {
            &part.low_object
        };
        let expected_role = if variant == "high" { "high" } else { "low" };
        if node.get("name").and_then(Value::as_str) != Some(expected_object.as_str())
            || strict_string_field(extras, "weaponry_semantic_role", "GLB part node extras")?
                != part.role
            || strict_string_field(extras, "weaponry_source_object", "GLB part node extras")?
                != part.source_object
            || strict_string_field(extras, "weaponry_part_role", "GLB part node extras")?
                != expected_role
            || strict_string_field(extras, "weaponry_operation_scope", "GLB part node extras")?
                != "part-local-fixed-role-policy@1"
            || strict_string_field(
                extras,
                "weaponry_surface_signal_schema",
                "GLB part node extras",
            )? != STRICT_SURFACE_SIGNAL_SCHEMA
            || strict_string_field(
                extras,
                "weaponry_surface_signal_storage",
                "GLB part node extras",
            )? != STRICT_SURFACE_SIGNAL_STORAGE
        {
            return Err(strict_invalid("GLB part/role/source node binding drifted"));
        }
        let material_ids_json =
            strict_string_field(extras, "weaponry_material_ids_json", "GLB part node extras")?;
        let material_ids_value: Value =
            serde_json::from_str(&material_ids_json).map_err(|error| {
                strict_invalid(format!("GLB node material_ids_json is invalid: {error}"))
            })?;
        let material_ids = strict_string_array(&material_ids_value, "GLB node material_ids_json")?;
        if material_ids != part.material_ids
            || part_nodes
                .insert(part_id, (node_index, mesh_index))
                .is_some()
        {
            return Err(strict_invalid(
                "GLB part node material/part binding is duplicated or drifted",
            ));
        }
    }
    if part_nodes.len() != parts.len() || used_meshes.len() != parts.len() {
        return Err(strict_invalid(
            "GLB does not contain exactly one High/Low node per manifest part",
        ));
    }
    let mut material_owners = BTreeMap::new();
    let mut material_bindings = BTreeMap::<String, BTreeSet<String>>::new();
    for part in parts {
        for material_id in &part.material_ids {
            material_bindings
                .entry(material_id.clone())
                .or_default()
                .extend(strict_material_part_aliases(
                    &part.source_object,
                    &part.part_id,
                ));
        }
    }
    let mut observed_triangles = 0u64;
    for part in parts {
        let (_, mesh_index) = part_nodes
            .get(&part.part_id)
            .copied()
            .ok_or_else(|| strict_invalid("GLB part node lookup failed"))?;
        observed_triangles = observed_triangles
            .checked_add(strict_validate_mesh(
                &glb,
                &accessors,
                strict_object(&meshes[mesh_index], "GLB mesh")?,
                part,
                materials,
                &material_bindings,
                &mut material_owners,
            )?)
            .ok_or_else(|| strict_invalid("GLB triangle count overflowed"))?;
    }
    if material_owners.len() != materials.len() {
        return Err(strict_invalid("GLB contains an unbound material entry"));
    }
    if observed_triangles != expected_triangles {
        return Err(strict_invalid(format!(
            "GLB {variant} triangle count {observed_triangles} differs from manifest/result {expected_triangles}"
        )));
    }
    Ok(())
}

fn strict_validate_worker_artifacts(
    request: &KnifeWorkerRequest,
    result: &Value,
    artifacts: &BTreeMap<String, Vec<u8>>,
) -> Result<(), RuntimeError> {
    let result_object = strict_object(result, "worker result")?;
    let result_outputs = strict_array(
        strict_field(result_object, "outputs", "worker result")?,
        "worker result.outputs",
    )?;
    let records = strict_output_records(result_object, artifacts)?;
    let manifest_record = records
        .last()
        .ok_or_else(|| strict_invalid("worker manifest output record is missing"))?;
    let manifest_bytes = artifacts
        .get(&manifest_record.relative_path)
        .ok_or_else(|| strict_invalid("worker manifest bytes are missing"))?;
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(strict_invalid("worker manifest exceeds its bounded size"));
    }
    let (manifest, parts) = strict_validate_worker_manifest(
        request,
        result_object,
        result_outputs,
        &records,
        manifest_bytes,
    )?;
    let manifest_object = strict_object(&manifest, "worker manifest")?;
    let stats = strict_object(
        strict_field(result_object, "stats", "worker result")?,
        "worker result.stats",
    )?;
    let high_triangles = strict_u64_field(stats, "high_triangle_count", "worker result.stats")?;
    let low_triangles = strict_u64_field(stats, "low_triangle_count", "worker result.stats")?;
    let texture_size = strict_u64_field(stats, "texture_size", "worker result.stats")?;
    if texture_size != forgecad_blender_worker::KNIFE_TEXTURE_SIZE as u64 {
        return Err(strict_invalid("worker result texture_size drifted"));
    }
    let high_record = records
        .iter()
        .find(|record| record.kind == "high_glb")
        .ok_or_else(|| strict_invalid("High GLB output record is missing"))?;
    let low_record = records
        .iter()
        .find(|record| record.kind == "low_glb")
        .ok_or_else(|| strict_invalid("Low GLB output record is missing"))?;
    strict_validate_glb(
        artifacts.get(&high_record.relative_path).unwrap(),
        &parts,
        "high",
        high_triangles,
    )?;
    strict_validate_glb(
        artifacts.get(&low_record.relative_path).unwrap(),
        &parts,
        "low",
        low_triangles,
    )?;
    strict_validate_map_records(
        manifest_object,
        &parts,
        &records,
        artifacts,
        texture_size as u32,
    )?;
    let map_records = records
        .iter()
        .filter(|record| record.kind == "normal_map" || record.kind == "ao_map")
        .count();
    if map_records != parts.len() * 2
        || strict_u64_field(stats, "bake_map_count", "worker result.stats")? != map_records as u64
    {
        return Err(strict_invalid(
            "worker result map count is not exactly manifest-bound",
        ));
    }
    Ok(())
}

fn strict_validate_adoption_receipt(
    receipt: &Value,
    result: &Map<String, Value>,
    adopted: &[Value],
    request: &KnifeWorkerRequest,
) -> Result<(), RuntimeError> {
    let object = strict_object(receipt, "Runtime adoption receipt")?;
    strict_require_exact_fields(
        object,
        &[
            "artifacts",
            "candidate_confirmed",
            "candidate_id",
            "canonical_sha256",
            "commercial_status",
            "durable_record_status",
            "engine_status",
            "export_performed",
            "human_status",
            "operation",
            "persistent_user_data_touched",
            "production_stage_advanced",
            "project_id",
            "request_id",
            "runtime_write_performed",
            "schema_version",
            "source_object_sha256",
            "version_created",
            "visual_status",
            "worker_identity_object_sha256",
            "worker_identity_sha256",
            "worker_result_object_sha256",
            "worker_result_sha256",
        ],
        "Runtime adoption receipt",
    )?;
    if strict_string_field(object, "schema_version", "Runtime adoption receipt")?
        != "WeaponryBlenderKnifeRuntimeAdoptionReceipt@1"
        || strict_string_field(object, "operation", "Runtime adoption receipt")?
            != request.operation
        || strict_string_field(object, "request_id", "Runtime adoption receipt")?
            != request.request_id
        || strict_string_field(object, "project_id", "Runtime adoption receipt")?
            != request.project_id
        || strict_string_field(object, "candidate_id", "Runtime adoption receipt")?
            != request.candidate_id
        || strict_string_field(object, "source_object_sha256", "Runtime adoption receipt")?
            != request.input_glb.sha256
        || strict_bool_field(
            object,
            "runtime_write_performed",
            "Runtime adoption receipt",
        )? != true
        || strict_bool_field(
            object,
            "persistent_user_data_touched",
            "Runtime adoption receipt",
        )? != true
        || strict_bool_field(
            object,
            "production_stage_advanced",
            "Runtime adoption receipt",
        )?
        || strict_bool_field(object, "candidate_confirmed", "Runtime adoption receipt")?
        || strict_bool_field(object, "version_created", "Runtime adoption receipt")?
        || strict_bool_field(object, "export_performed", "Runtime adoption receipt")?
        || strict_string_field(object, "durable_record_status", "Runtime adoption receipt")?
            != "CAS_ADOPTED_NO_LOOKUP_ROW"
        || strict_string_field(object, "visual_status", "Runtime adoption receipt")? != "NOT_RUN"
        || strict_string_field(object, "human_status", "Runtime adoption receipt")? != "NOT_RUN"
        || strict_string_field(object, "engine_status", "Runtime adoption receipt")? != "NOT_RUN"
        || strict_string_field(object, "commercial_status", "Runtime adoption receipt")?
            != "NOT_RUN"
        || strict_string_field(object, "worker_result_sha256", "Runtime adoption receipt")?
            != strict_string_field(result, "canonical_sha256", "worker result")?
    {
        return Err(strict_invalid(
            "Runtime adoption receipt identity/flags drifted",
        ));
    }
    let receipt_artifacts = strict_array(
        strict_field(object, "artifacts", "Runtime adoption receipt")?,
        "Runtime adoption receipt.artifacts",
    )?;
    if receipt_artifacts.len() != adopted.len()
        || adopted.len()
            != strict_array(
                strict_field(result, "outputs", "worker result")?,
                "worker result.outputs",
            )?
            .len()
    {
        return Err(strict_invalid(
            "Runtime adoption receipt artifact count drifted",
        ));
    }
    let expected_fields = [
        "relative_path",
        "output_kind",
        "mime",
        "semantic_sha256",
        "object_sha256",
        "byte_size",
    ];
    let result_outputs = strict_array(
        strict_field(result, "outputs", "worker result")?,
        "worker result.outputs",
    )?;
    for (index, (receipt_value, adopted_value)) in receipt_artifacts.iter().zip(adopted).enumerate()
    {
        let receipt_artifact = strict_object(
            receipt_value,
            &format!("Runtime adoption receipt.artifacts[{index}]"),
        )?;
        strict_require_exact_fields(
            receipt_artifact,
            &expected_fields,
            "Runtime adoption receipt artifact",
        )?;
        let adopted_object = strict_object(adopted_value, "adopted artifact")?;
        let output = strict_object(&result_outputs[index], "worker result output")?;
        for (receipt_key, output_key) in [
            ("relative_path", "relative_path"),
            ("output_kind", "kind"),
            ("mime", "mime"),
            ("semantic_sha256", "sha256"),
            ("byte_size", "byte_size"),
        ] {
            if receipt_artifact.get(receipt_key) != output.get(output_key) {
                return Err(strict_invalid(
                    "Runtime adoption receipt artifact differs from worker output",
                ));
            }
        }
        if receipt_artifact.get("object_sha256") != adopted_object.get("object_sha256")
            || receipt_artifact.get("semantic_sha256") != adopted_object.get("semantic_sha256")
            || receipt_artifact.get("relative_path") != adopted_object.get("relative_path")
        {
            return Err(strict_invalid(
                "Runtime adoption receipt artifact differs from CAS adoption",
            ));
        }
        let object_sha256 = strict_sha_field(
            receipt_artifact,
            "object_sha256",
            "Runtime adoption receipt artifact",
        )?;
        let semantic_sha256 = strict_sha_field(
            receipt_artifact,
            "semantic_sha256",
            "Runtime adoption receipt artifact",
        )?;
        if object_sha256 != semantic_sha256 {
            return Err(strict_invalid(
                "Runtime adoption receipt semantic/object hash diverged",
            ));
        }
    }
    let supplied_hash = strict_sha_field(object, "canonical_sha256", "Runtime adoption receipt")?;
    let mut preimage = receipt.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied_hash {
        return Err(strict_invalid(
            "Runtime adoption receipt canonical hash drifted",
        ));
    }
    Ok(())
}

struct RuntimeBlenderScratch {
    path: PathBuf,
}

impl RuntimeBlenderScratch {
    fn create() -> Result<Self, RuntimeError> {
        let path =
            std::env::temp_dir().join(format!("weaponry-blender-runtime-{}", Uuid::new_v4()));
        fs::create_dir(&path).map_err(|error| {
            invalid(format!(
                "Runtime Blender scratch root could not be created: {error}"
            ))
        })?;
        let path = fs::canonicalize(&path).map_err(|error| {
            invalid(format!(
                "Runtime Blender scratch root could not be resolved: {error}"
            ))
        })?;
        fs::create_dir(path.join("input")).map_err(|error| {
            invalid(format!(
                "Runtime Blender scratch input directory could not be created: {error}"
            ))
        })?;
        Ok(Self { path })
    }

    fn stage_input(&self, source_glb: &[u8]) -> Result<(), RuntimeError> {
        let input_path = self
            .path
            .join(forgecad_blender_worker::KNIFE_INPUT_RELATIVE_PATH);
        fs::write(input_path, source_glb).map_err(|error| {
            invalid(format!(
                "Runtime Blender scratch input could not be written: {error}"
            ))
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for RuntimeBlenderScratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn durable_artifact_ref(value: &Value) -> Result<WeaponryBlenderArtifactRef, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("durable artifact reference is not an object"))?;
    strict_require_exact_fields(
        object,
        &[
            "byte_size",
            "mime",
            "object_sha256",
            "output_kind",
            "relative_path",
            "semantic_sha256",
        ],
        "durable artifact reference",
    )?;
    Ok(WeaponryBlenderArtifactRef {
        relative_path: strict_string_field(object, "relative_path", "durable artifact")?,
        kind: strict_string_field(object, "output_kind", "durable artifact")?,
        mime: strict_string_field(object, "mime", "durable artifact")?,
        semantic_sha256: strict_sha_field(object, "semantic_sha256", "durable artifact")?,
        object_sha256: strict_sha_field(object, "object_sha256", "durable artifact")?,
        byte_size: strict_u64_field(object, "byte_size", "durable artifact")?,
    })
}

fn unique_artifact<'a>(
    artifacts: &'a [WeaponryBlenderArtifactRef],
    kind: &str,
) -> Result<&'a WeaponryBlenderArtifactRef, RuntimeError> {
    let mut matches = artifacts.iter().filter(|artifact| artifact.kind == kind);
    let artifact = matches
        .next()
        .ok_or_else(|| invalid(format!("durable {kind} artifact is missing")))?;
    if matches.next().is_some() {
        return Err(invalid(format!("durable {kind} artifact is duplicated")));
    }
    Ok(artifact)
}

impl Runtime {
    /// Execute the fixed Blender knife provider and adopt its temporary bytes
    /// into Runtime CAS.  This is an internal Rust capability, not an MCP tool
    /// or a generic Blender/Python execution surface.
    pub fn weaponry_blender_knife_worker_execute_internal(
        &self,
        request: &Value,
        source_object_sha256: &str,
    ) -> Result<Value, RuntimeError> {
        let request: KnifeWorkerRequest = serde_json::from_value(request.clone())
            .map_err(|error| invalid(format!("closed Worker request is invalid: {error}")))?;
        if request.input_glb.sha256 != source_object_sha256 {
            return Err(invalid("source CAS identity differs from Worker request"));
        }
        let source_object = self
            .store
            .get_object(source_object_sha256)?
            .ok_or_else(|| invalid("source GLB CAS object is unavailable"))?;
        if source_object.mime != "model/gltf-binary"
            || source_object.size_bytes != request.input_glb.byte_size
            || source_object.size_bytes == 0
            || source_object.size_bytes > MAX_SOURCE_BYTES
        {
            return Err(invalid("source GLB CAS metadata differs"));
        }
        let source_glb = self.cas_read_bounded(source_object_sha256, MAX_SOURCE_BYTES)?;
        if sha256_hex(&source_glb) != source_object_sha256 {
            return Err(invalid("source GLB CAS bytes drifted"));
        }

        let discovered = discover_install()?;
        let package_status = string(&discovered.package_manifest, "/status")?.to_owned();
        let package_manifest_canonical = sha(&discovered.package_manifest, "/canonical_sha256")?;
        let resource_tree_sha256 = sha(&discovered.package_manifest, "/resource_tree_sha256")?;
        let blender_bundle_tree_sha256 =
            sha(&discovered.package_manifest, "/blender/bundle_tree_sha256")?;
        let release_eligible = discovered
            .release_eligibility
            .get("release_eligible")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("release eligibility boolean is missing"))?;
        let release_eligibility_canonical =
            sha(&discovered.release_eligibility, "/canonical_sha256")?;
        let worker = KnifeBlenderWorker::new(discovered.install)
            .map_err(|error| invalid(error.to_string()))?;
        let scratch = RuntimeBlenderScratch::create()?;
        scratch.stage_input(&source_glb)?;
        let run = worker
            .run_from_staged_root(&request, scratch.path())
            .map_err(|error| invalid(error.to_string()))?;
        let result = run
            .response
            .result
            .as_ref()
            .ok_or_else(|| invalid("validated Worker response lacks result"))?;
        let output_records = result
            .get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("validated Worker result lacks outputs"))?;
        strict_validate_worker_artifacts(&request, result, &run.artifacts)?;
        let mut expected_by_path = BTreeMap::new();
        for output in output_records {
            let path = output
                .get("relative_path")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("Worker output path is missing"))?;
            expected_by_path.insert(path.to_owned(), output.clone());
        }

        let mut adopted = Vec::with_capacity(run.artifacts.len());
        // Preserve the Worker result order in the adoption receipt.  The
        // result list is the closed ordering authority; BTreeMap ordering is
        // useful for deterministic identity but must not silently reorder the
        // receipt/output binding.
        for output in output_records {
            let path = output
                .get("relative_path")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("Worker output path is missing"))?;
            let bytes = run
                .artifacts
                .get(path)
                .ok_or_else(|| invalid("Worker output bytes are absent during adoption"))?;
            let output = expected_by_path
                .get(path)
                .ok_or_else(|| invalid("Worker bytes are absent from its output manifest"))?;
            let expected_sha = string(output, "/sha256")?;
            let mime = string(output, "/mime")?;
            let output_kind = string(output, "/kind")?;
            if output.get("byte_size").and_then(Value::as_u64) != Some(bytes.len() as u64)
                || sha256_hex(bytes) != expected_sha
            {
                return Err(invalid(
                    "Worker artifact hash or size drifted during adoption",
                ));
            }
            let object =
                self.put_object(bytes, Some(expected_sha), mime, &cas_kind(output_kind)?)?;
            adopted.push(json!({
                "relative_path": path,
                "output_kind": output_kind,
                "mime": mime,
                "semantic_sha256": expected_sha,
                "object_sha256": object.record.sha256,
                "byte_size": object.record.size_bytes,
            }));
        }

        let result_bytes =
            canonical_json_bytes(result).map_err(|error| invalid(error.to_string()))?;
        let result_object = self.put_object(
            &result_bytes,
            None,
            JSON_MIME,
            "weaponry-blender-worker-result@1",
        )?;
        let mut identity = run.identity_projection();
        identity["canonical_sha256"] = Value::String(canonical_json_hash(&identity));
        let identity_bytes =
            canonical_json_bytes(&identity).map_err(|error| invalid(error.to_string()))?;
        let identity_object = self.put_object(
            &identity_bytes,
            None,
            JSON_MIME,
            "weaponry-blender-worker-identity@1",
        )?;
        let package_manifest_object = self.put_object(
            &discovered.package_manifest_bytes,
            None,
            JSON_MIME,
            "weaponry-blender-package-manifest@1",
        )?;
        let release_eligibility_object = self.put_object(
            &discovered.release_eligibility_bytes,
            None,
            JSON_MIME,
            "weaponry-blender-release-eligibility@1",
        )?;
        let mut receipt = json!({
            "schema_version":"WeaponryBlenderKnifeRuntimeAdoptionReceipt@1",
            "operation":KNIFE_OPERATION,
            "request_id":request.request_id,
            "project_id":request.project_id,
            "candidate_id":request.candidate_id,
            "source_object_sha256":source_object_sha256,
            "worker_result_sha256":result.get("canonical_sha256"),
            "worker_result_object_sha256":result_object.record.sha256,
            "worker_identity_sha256":identity.get("canonical_sha256"),
            "worker_identity_object_sha256":identity_object.record.sha256,
            "artifacts":adopted,
            "runtime_write_performed":true,
            "persistent_user_data_touched":true,
            "production_stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "visual_status":"NOT_RUN",
            "human_status":"NOT_RUN",
            "engine_status":"NOT_RUN",
            "commercial_status":"NOT_RUN",
            "durable_record_status":"CAS_ADOPTED_NO_LOOKUP_ROW",
            "canonical_sha256":""
        });
        receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
        strict_validate_adoption_receipt(
            &receipt,
            result.as_object().unwrap(),
            &adopted,
            &request,
        )?;
        let receipt_bytes =
            canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
        let receipt_object = self.put_object(
            &receipt_bytes,
            None,
            JSON_MIME,
            "weaponry-blender-runtime-adoption-receipt@1",
        )?;
        Ok(json!({
            "schema_version":"WeaponryBlenderKnifeRuntimeAdoptionResult@1",
            "receipt":receipt,
            "receipt_object_sha256":receipt_object.record.sha256,
            "package_identity":{
                "packaged":true,
                "package_manifest_canonical_sha256":package_manifest_canonical,
                "package_manifest_object_sha256":package_manifest_object.record.sha256,
                "resource_tree_sha256":resource_tree_sha256,
                "blender_bundle_tree_sha256":blender_bundle_tree_sha256,
                "release_eligibility_canonical_sha256":release_eligibility_canonical,
                "release_eligibility_object_sha256":release_eligibility_object.record.sha256,
                "package_status":package_status,
                "release_eligible":release_eligible
            },
            "runtime_write_performed":true,
            "persistent_user_data_touched":true,
            "production_stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false
        }))
    }

    /// Execute or exactly replay the sealed Blender knife job and persist one
    /// Runtime-owned lookup row. The request id is the idempotency key; a
    /// replay never starts Blender again. This remains an internal capability
    /// until a closed public Contract/MCP facade is intentionally introduced.
    pub fn weaponry_blender_knife_worker_execute_durable_internal(
        &self,
        request: &Value,
        source_object_sha256: &str,
    ) -> Result<Value, RuntimeError> {
        let typed: KnifeWorkerRequest = serde_json::from_value(request.clone())
            .map_err(|error| invalid(format!("closed Worker request is invalid: {error}")))?;
        if typed.input_glb.sha256 != source_object_sha256 {
            return Err(invalid("source CAS identity differs from Worker request"));
        }
        if let Some(existing) = self
            .store
            .get_weaponry_blender_execution(&typed.project_id, &typed.request_id)?
        {
            if existing.request_sha256 != typed.canonical_sha256
                || existing.source_object_sha256 != source_object_sha256
                || existing.candidate_id != typed.candidate_id
            {
                return Err(invalid(
                    "durable Worker idempotency key is bound to a different request",
                ));
            }
            return self.weaponry_blender_durable_projection(&existing, "replayed", true);
        }

        let adoption =
            self.weaponry_blender_knife_worker_execute_internal(request, source_object_sha256)?;
        let receipt = adoption
            .get("receipt")
            .ok_or_else(|| invalid("Runtime adoption receipt is missing"))?;
        let receipt_object_sha256 = strict_sha_value(
            adoption
                .get("receipt_object_sha256")
                .ok_or_else(|| invalid("Runtime adoption receipt object is missing"))?,
            "receipt_object_sha256",
        )?;
        let artifact_values = receipt
            .get("artifacts")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("Runtime adoption artifact list is missing"))?;
        let artifacts = artifact_values
            .iter()
            .map(durable_artifact_ref)
            .collect::<Result<Vec<_>, _>>()?;
        let high = unique_artifact(&artifacts, "high_glb")?.clone();
        let low = unique_artifact(&artifacts, "low_glb")?.clone();
        let manifest = unique_artifact(&artifacts, "worker_manifest")?.clone();
        let mut normal_maps = artifacts
            .iter()
            .filter(|artifact| artifact.kind == "normal_map")
            .cloned()
            .collect::<Vec<_>>();
        let mut ao_maps = artifacts
            .iter()
            .filter(|artifact| artifact.kind == "ao_map")
            .cloned()
            .collect::<Vec<_>>();
        normal_maps.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        ao_maps.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        let worker_result_sha256 = strict_sha_value(
            receipt
                .get("worker_result_sha256")
                .ok_or_else(|| invalid("Worker result semantic hash is missing"))?,
            "worker_result_sha256",
        )?;
        let worker_result_object_sha256 = strict_sha_value(
            receipt
                .get("worker_result_object_sha256")
                .ok_or_else(|| invalid("Worker result object hash is missing"))?,
            "worker_result_object_sha256",
        )?;
        let worker_identity_sha256 = strict_sha_value(
            receipt
                .get("worker_identity_sha256")
                .ok_or_else(|| invalid("Worker identity semantic hash is missing"))?,
            "worker_identity_sha256",
        )?;
        let worker_identity_object_sha256 = strict_sha_value(
            receipt
                .get("worker_identity_object_sha256")
                .ok_or_else(|| invalid("Worker identity object hash is missing"))?,
            "worker_identity_object_sha256",
        )?;
        let receipt_sha256 = strict_sha_value(
            receipt
                .get("canonical_sha256")
                .ok_or_else(|| invalid("Runtime receipt semantic hash is missing"))?,
            "receipt.canonical_sha256",
        )?;
        let worker_result_bytes =
            self.cas_read_bounded(&worker_result_object_sha256, MAX_MANIFEST_BYTES * 8)?;
        let worker_result: Value = serde_json::from_slice(&worker_result_bytes)
            .map_err(|error| invalid(format!("stored Worker result JSON is invalid: {error}")))?;
        let worker_identity_bytes =
            self.cas_read_bounded(&worker_identity_object_sha256, MAX_MANIFEST_BYTES * 8)?;
        let worker_identity: Value = serde_json::from_slice(&worker_identity_bytes)
            .map_err(|error| invalid(format!("stored Worker identity JSON is invalid: {error}")))?;
        let package = adoption
            .get("package_identity")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("packaged Worker identity is missing"))?;
        let package_manifest_object_sha256 = strict_sha_field(
            package,
            "package_manifest_object_sha256",
            "package identity",
        )?;
        let release_eligibility_object_sha256 = strict_sha_field(
            package,
            "release_eligibility_object_sha256",
            "package identity",
        )?;
        let source = self
            .store
            .get_object(source_object_sha256)?
            .ok_or_else(|| invalid("source CAS object disappeared before durable commit"))?;
        let object = |hash: &str| -> Result<_, RuntimeError> {
            self.store
                .get_object(hash)?
                .ok_or_else(|| invalid(format!("durable CAS object {hash} is missing")))
        };
        let mut record = WeaponryBlenderExecutionStoreRecord {
            schema_version: WEAPONRY_BLENDER_EXECUTION_RECORD_SCHEMA.to_owned(),
            project_id: typed.project_id.clone(),
            candidate_id: typed.candidate_id.clone(),
            execution_id: format!("blender-{}", &worker_result_sha256[..24]),
            request_id: typed.request_id.clone(),
            operation: KNIFE_OPERATION.to_owned(),
            source_object_sha256: source_object_sha256.to_owned(),
            source_object_size_bytes: source.size_bytes,
            worker_id: strict_string_field(
                worker_result
                    .as_object()
                    .ok_or_else(|| invalid("Worker result is not an object"))?,
                "worker_id",
                "Worker result",
            )?,
            worker_version: strict_string_field(
                worker_result.as_object().unwrap(),
                "worker_version",
                "Worker result",
            )?,
            blender_version: strict_string_field(
                worker_result.as_object().unwrap(),
                "blender_version",
                "Worker result",
            )?,
            blender_revision: strict_string_field(
                worker_result.as_object().unwrap(),
                "blender_revision",
                "Worker result",
            )?,
            worker_entrypoint_sha256: strict_sha_field(
                worker_result.as_object().unwrap(),
                "worker_entrypoint_sha256",
                "Worker result",
            )?,
            worker_bundle_sha256: strict_sha_field(
                worker_identity
                    .as_object()
                    .ok_or_else(|| invalid("Worker identity is not an object"))?,
                "worker_bundle_sha256",
                "Worker identity",
            )?,
            dependency_lock_sha256: strict_sha_field(
                worker_result.as_object().unwrap(),
                "dependency_lock_sha256",
                "Worker result",
            )?,
            worker_identity_sha256,
            worker_identity_object_sha256: worker_identity_object_sha256.clone(),
            worker_result_sha256: worker_result_sha256.clone(),
            worker_result_object_sha256: worker_result_object_sha256.clone(),
            receipt_sha256,
            receipt_object_sha256: receipt_object_sha256.clone(),
            worker_manifest_sha256: manifest.semantic_sha256.clone(),
            worker_manifest_object_sha256: manifest.object_sha256.clone(),
            worker_manifest_relative_path: manifest.relative_path.clone(),
            high_glb_sha256: high.semantic_sha256.clone(),
            high_glb_object_sha256: high.object_sha256.clone(),
            high_glb_bytes: high.byte_size,
            high_glb_relative_path: high.relative_path.clone(),
            low_glb_sha256: low.semantic_sha256.clone(),
            low_glb_object_sha256: low.object_sha256.clone(),
            low_glb_bytes: low.byte_size,
            low_glb_relative_path: low.relative_path.clone(),
            normal_maps: normal_maps.clone(),
            ao_maps: ao_maps.clone(),
            normal_map_set_sha256: weaponry_blender_artifact_set_sha256(&normal_maps)?,
            ao_map_set_sha256: weaponry_blender_artifact_set_sha256(&ao_maps)?,
            all_artifact_set_sha256: String::new(),
            package_identity: WeaponryBlenderPackageIdentity {
                packaged: true,
                package_manifest_sha256: Some(package_manifest_object_sha256.clone()),
                resource_tree_sha256: Some(strict_sha_field(
                    package,
                    "resource_tree_sha256",
                    "package identity",
                )?),
                blender_bundle_tree_sha256: Some(strict_sha_field(
                    package,
                    "blender_bundle_tree_sha256",
                    "package identity",
                )?),
                release_eligibility_sha256: Some(release_eligibility_object_sha256.clone()),
                package_status: strict_string_field(package, "package_status", "package identity")?,
                release_eligible: strict_bool_field(
                    package,
                    "release_eligible",
                    "package identity",
                )?,
            },
            materialization_status: WEAPONRY_BLENDER_EXECUTION_STATUS.to_owned(),
            quality_status: "structural_only".to_owned(),
            visual_status: "NOT_RUN".to_owned(),
            human_status: "NOT_RUN".to_owned(),
            engine_status: "NOT_RUN".to_owned(),
            commercial_status: "NOT_RUN".to_owned(),
            runtime_write_performed: true,
            persistent_user_data_touched: true,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            request_sha256: typed.canonical_sha256.clone(),
            idempotency_key: typed.request_id.clone(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        let mut all = vec![high.clone(), low.clone()];
        all.extend(normal_maps.clone());
        all.extend(ao_maps.clone());
        let mut manifest_for_set = manifest.clone();
        manifest_for_set.byte_size = 0;
        all.push(manifest_for_set);
        record.all_artifact_set_sha256 = weaponry_blender_artifact_set_sha256(&all)?;
        record.canonical_sha256 = weaponry_blender_execution_record_canonical_sha256(&record)?;
        let commit = WeaponryBlenderExecutionCommit {
            record,
            cas: WeaponryBlenderExecutionCasBundle {
                source,
                worker_identity: object(&worker_identity_object_sha256)?,
                worker_result: object(&worker_result_object_sha256)?,
                receipt: object(&receipt_object_sha256)?,
                worker_manifest: object(&manifest.object_sha256)?,
                high_glb: object(&high.object_sha256)?,
                low_glb: object(&low.object_sha256)?,
                normal_maps: normal_maps
                    .iter()
                    .map(|artifact| object(&artifact.object_sha256))
                    .collect::<Result<Vec<_>, _>>()?,
                ao_maps: ao_maps
                    .iter()
                    .map(|artifact| object(&artifact.object_sha256))
                    .collect::<Result<Vec<_>, _>>()?,
                package_manifest: Some(object(&package_manifest_object_sha256)?),
                release_eligibility: Some(object(&release_eligibility_object_sha256)?),
            },
        };
        let (persisted, replayed) = self
            .store
            .record_weaponry_blender_execution_with_replay(&commit)?;
        self.weaponry_blender_durable_projection(
            &persisted,
            if replayed { "replayed" } else { "prepared" },
            replayed,
        )
    }

    pub fn weaponry_blender_knife_worker_execution_get_internal(
        &self,
        project_id: &str,
        execution_id: &str,
        source_object_sha256: &str,
        worker_result_object_sha256: &str,
        receipt_object_sha256: &str,
    ) -> Result<Value, RuntimeError> {
        let record = self
            .store
            .get_weaponry_blender_execution_exact(
                project_id,
                execution_id,
                source_object_sha256,
                worker_result_object_sha256,
                receipt_object_sha256,
            )?
            .ok_or_else(|| invalid("durable Blender execution was not found"))?;
        self.weaponry_blender_durable_projection(&record, "found", false)
    }

    fn weaponry_blender_durable_projection(
        &self,
        record: &WeaponryBlenderExecutionStoreRecord,
        status: &str,
        replayed: bool,
    ) -> Result<Value, RuntimeError> {
        let worker_result = self
            .store
            .read_weaponry_blender_worker_result_json(record)?;
        let worker_identity = self
            .store
            .read_weaponry_blender_worker_identity_json(record)?;
        let worker_manifest = self
            .store
            .read_weaponry_blender_worker_manifest_json(record)?;
        let receipt = self.store.read_weaponry_blender_receipt_json(record)?;
        Ok(json!({
            "schema_version":"WeaponryBlenderKnifeDurableExecutionResult@1",
            "operation":KNIFE_OPERATION,
            "status":status,
            "project_id":record.project_id,
            "candidate_id":record.candidate_id,
            "execution_id":record.execution_id,
            "source_object_sha256":record.source_object_sha256,
            "worker_result_sha256":record.worker_result_sha256,
            "worker_result_object_sha256":record.worker_result_object_sha256,
            "receipt_sha256":record.receipt_sha256,
            "receipt_object_sha256":record.receipt_object_sha256,
            "high_glb_sha256":record.high_glb_sha256,
            "high_glb_object_sha256":record.high_glb_object_sha256,
            "low_glb_sha256":record.low_glb_sha256,
            "low_glb_object_sha256":record.low_glb_object_sha256,
            "normal_map_set_sha256":record.normal_map_set_sha256,
            "ao_map_set_sha256":record.ao_map_set_sha256,
            "all_artifact_set_sha256":record.all_artifact_set_sha256,
            "record":serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?,
            "worker_result":worker_result,
            "worker_identity":worker_identity,
            "worker_manifest":worker_manifest,
            "receipt":receipt,
            "replayed":replayed,
            "store_effect":if status == "prepared" { "inserted" } else { "not-touched" },
            "cas_effect":"not-touched",
            "runtime_write_performed":status == "prepared",
            "persistent_user_data_touched":status == "prepared",
            "production_stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "quality_status":"structural_only",
            "visual_status":"NOT_RUN",
            "human_status":"NOT_RUN",
            "engine_status":"NOT_RUN",
            "commercial_status":"NOT_RUN"
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_blender_worker::{
        KNIFE_MAX_INPUT_BYTES, KNIFE_MAX_MEMORY_BYTES, KNIFE_MAX_OUTPUT_BYTES,
        KNIFE_MAX_RUNTIME_MS, KNIFE_MAX_TRIANGLES, KNIFE_RECIPE_ID, KNIFE_RECIPE_SHA256,
        KNIFE_REQUEST_SCHEMA, KNIFE_TEXTURE_SIZE,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn png_fixture(width: u32, height: u32) -> Vec<u8> {
        fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
            output.extend_from_slice(&(data.len() as u32).to_be_bytes());
            output.extend_from_slice(kind);
            output.extend_from_slice(data);
            // The strict map gate checks the closed PNG envelope and payload
            // shape.  CRC calculation is intentionally unnecessary for this
            // parser fixture because the production Blender bytes are also
            // checked by the fixed worker hash before this gate.
            output.extend_from_slice(&[0, 0, 0, 0]);
        }

        let mut output = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut output, b"IHDR", &ihdr);
        chunk(&mut output, b"IDAT", &[0]);
        chunk(&mut output, b"IEND", &[]);
        output
    }

    fn glb_fixture(asset_version: &str) -> Vec<u8> {
        let mut json = serde_json::to_vec(&json!({
            "asset": { "version": asset_version }
        }))
        .expect("GLB fixture JSON");
        json.resize((json.len() + 3) & !3, b' ');
        let bin = [0, 0, 0, 0];
        let total_length = 12 + 8 + json.len() + 8 + bin.len();
        let mut output = Vec::with_capacity(total_length);
        output.extend_from_slice(b"glTF");
        output.extend_from_slice(&2u32.to_le_bytes());
        output.extend_from_slice(&(total_length as u32).to_le_bytes());
        output.extend_from_slice(&(json.len() as u32).to_le_bytes());
        output.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
        output.extend_from_slice(&json);
        output.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        output.extend_from_slice(&0x004E4942u32.to_le_bytes());
        output.extend_from_slice(&bin);
        output
    }

    #[test]
    fn strict_png_map_enforces_signature_and_dimensions() {
        let valid = png_fixture(KNIFE_TEXTURE_SIZE, KNIFE_TEXTURE_SIZE);
        strict_validate_png_map(&valid, KNIFE_TEXTURE_SIZE).expect("valid PNG envelope");

        let mut bad_signature = valid.clone();
        bad_signature[0] = 0;
        assert!(strict_validate_png_map(&bad_signature, KNIFE_TEXTURE_SIZE).is_err());

        let mut bad_dimensions = valid;
        bad_dimensions[16..20].copy_from_slice(&511u32.to_be_bytes());
        assert!(strict_validate_png_map(&bad_dimensions, KNIFE_TEXTURE_SIZE).is_err());
    }

    #[test]
    fn strict_glb_parser_enforces_glb2_header_and_asset_version() {
        let valid = glb_fixture("2.0");
        strict_parse_glb(&valid).expect("valid glTF 2 GLB");

        let mut bad_version = valid.clone();
        bad_version[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(strict_parse_glb(&bad_version).is_err());

        let bad_asset_version = glb_fixture("1.0");
        assert!(strict_parse_glb(&bad_asset_version).is_err());
    }

    #[test]
    fn strict_output_records_bind_exact_returned_bytes_and_kinds() {
        let mut artifacts = BTreeMap::new();
        let mut output_values = Vec::new();
        for (path, kind, mime, bytes) in [
            (
                "output/dragonfang-high.blend.glb",
                "high_glb",
                "model/gltf-binary",
                b"high".as_slice(),
            ),
            (
                "output/dragonfang-low.blend.glb",
                "low_glb",
                "model/gltf-binary",
                b"low".as_slice(),
            ),
            (
                "output/manifest.json",
                "worker_manifest",
                "application/json",
                b"manifest".as_slice(),
            ),
        ] {
            artifacts.insert(path.to_owned(), bytes.to_vec());
            output_values.push(json!({
                "kind": kind,
                "relative_path": path,
                "mime": mime,
                "byte_size": bytes.len(),
                "sha256": sha256_hex(bytes),
                "cas_owner": "runtime",
                "durability": "pending_runtime_adoption"
            }));
        }
        let result = json!({ "outputs": output_values });
        let records = strict_output_records(result.as_object().unwrap(), &artifacts)
            .expect("closed output list");
        assert_eq!(records.len(), 3);

        let mut malformed = result;
        malformed["outputs"][0]["cas_owner"] = Value::String("caller".to_owned());
        assert!(strict_output_records(malformed.as_object().unwrap(), &artifacts).is_err());
    }

    /// This is deliberately opt-in because it launches the fixed Blender
    /// sidecar and performs real High/Low/UV/Bake work.  The default Runtime
    /// test suite remains offline and never starts Blender.
    #[test]
    fn dragonfang_r8_blender_runtime_cas_adoption_live() {
        if std::env::var("FORGECAD_RUN_WEAPONRY_BLENDER_RUNTIME_LIVE").as_deref() != Ok("1") {
            return;
        }

        let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../packages/weaponry-threejs/deliveries/dragonfang-r8/dragonfang-kukri-r8-action-ready.glb",
        );
        let source_glb = fs::read(&source_path).expect("Dragonfang r8 source GLB");
        assert!(!source_glb.is_empty());
        assert!(source_glb.len() <= KNIFE_MAX_INPUT_BYTES);
        let source_sha256 = sha256_hex(&source_glb);

        let root = std::env::temp_dir().join(format!(
            "weaponry-blender-dragonfang-live-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("create live Runtime root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let runtime = Runtime::open_with_cas(&database, &cas).expect("file-backed Runtime");
        let project = runtime
            .create_project(
                "Dragonfang fixed Blender Worker live",
                json!({"profile":"weaponry-knife-p0"}),
            )
            .expect("create live project");
        let source_object = runtime
            .put_object(
                &source_glb,
                Some(&source_sha256),
                "model/gltf-binary",
                "dragonfang-r8-source-glb@1",
            )
            .expect("store Dragonfang r8 source GLB in CAS");
        assert_eq!(source_object.record.sha256, source_sha256);
        assert_eq!(
            runtime
                .cas_read(&source_sha256)
                .expect("source CAS readback"),
            source_glb
        );

        let mut request = json!({
            "schema_version": KNIFE_REQUEST_SCHEMA,
            "operation": KNIFE_OPERATION,
            "request_id": "dragonfang-r8-runtime-blender-live",
            "project_id": project.project_id,
            "candidate_id": "dragonfang-kukri-r8",
            "input_glb": {
                "kind": "authoring_mesh_glb",
                "relative_path": forgecad_blender_worker::KNIFE_INPUT_RELATIVE_PATH,
                "sha256": source_sha256,
                "byte_size": source_glb.len(),
                "mime": "model/gltf-binary"
            },
            "recipe_id": KNIFE_RECIPE_ID,
            "recipe_sha256": KNIFE_RECIPE_SHA256,
            "budgets": {
                "max_runtime_ms": KNIFE_MAX_RUNTIME_MS,
                "max_memory_bytes": KNIFE_MAX_MEMORY_BYTES,
                "max_input_bytes": KNIFE_MAX_INPUT_BYTES,
                "max_output_bytes": KNIFE_MAX_OUTPUT_BYTES,
                "max_triangles": KNIFE_MAX_TRIANGLES,
                "texture_size": KNIFE_TEXTURE_SIZE
            },
            "policies": {
                "network_policy": "disabled",
                "filesystem_policy": "runtime_scratch_only",
                "script_policy": "frozen_bundle_only",
                "output_policy": "temporary_observation_runtime_adoption"
            },
            "canonical_sha256": ""
        });
        request["canonical_sha256"] = Value::String(canonical_json_hash(&request));

        let adopted = runtime
            .weaponry_blender_knife_worker_execute_durable_internal(&request, &source_sha256)
            .expect("execute and persist fixed Blender Worker output");
        assert_eq!(adopted["status"], "prepared");
        assert_eq!(adopted["replayed"], false);
        assert_eq!(adopted["store_effect"], "inserted");
        assert_eq!(adopted["runtime_write_performed"], true);
        assert_eq!(adopted["persistent_user_data_touched"], true);
        assert_eq!(adopted["production_stage_advanced"], false);
        assert_eq!(adopted["candidate_confirmed"], false);
        assert_eq!(adopted["version_created"], false);
        assert_eq!(adopted["export_performed"], false);

        let replayed = runtime
            .weaponry_blender_knife_worker_execute_durable_internal(&request, &source_sha256)
            .expect("exactly replay fixed Blender Worker execution");
        assert_eq!(replayed["status"], "replayed");
        assert_eq!(replayed["replayed"], true);
        assert_eq!(replayed["store_effect"], "not-touched");
        assert_eq!(replayed["cas_effect"], "not-touched");
        assert_eq!(replayed["runtime_write_performed"], false);
        assert_eq!(replayed["persistent_user_data_touched"], false);
        for field in [
            "execution_id",
            "source_object_sha256",
            "worker_result_sha256",
            "worker_result_object_sha256",
            "receipt_sha256",
            "receipt_object_sha256",
            "high_glb_sha256",
            "high_glb_object_sha256",
            "low_glb_sha256",
            "low_glb_object_sha256",
            "normal_map_set_sha256",
            "ao_map_set_sha256",
            "all_artifact_set_sha256",
        ] {
            assert_eq!(replayed[field], adopted[field], "replay drifted at {field}");
        }

        let receipt = adopted["receipt"].as_object().expect("adoption receipt");
        let receipt_object_sha256 = adopted["receipt_object_sha256"]
            .as_str()
            .expect("receipt object hash");
        let receipt_bytes = runtime
            .cas_read(receipt_object_sha256)
            .expect("receipt CAS readback");
        assert_eq!(sha256_hex(&receipt_bytes), receipt_object_sha256);
        let stored_receipt: Value = serde_json::from_slice(&receipt_bytes).expect("receipt JSON");
        assert_eq!(stored_receipt.as_object(), Some(receipt));
        assert_eq!(
            stored_receipt["canonical_sha256"],
            receipt["canonical_sha256"]
        );

        let worker_result_sha256 = receipt["worker_result_sha256"]
            .as_str()
            .expect("Worker result semantic hash");
        let worker_result_object_sha256 = receipt["worker_result_object_sha256"]
            .as_str()
            .expect("Worker result object hash");
        let worker_result_bytes = runtime
            .cas_read(worker_result_object_sha256)
            .expect("Worker result CAS readback");
        assert_eq!(
            sha256_hex(&worker_result_bytes),
            worker_result_object_sha256
        );
        let worker_result: Value =
            serde_json::from_slice(&worker_result_bytes).expect("Worker result JSON");
        assert_eq!(worker_result["canonical_sha256"], worker_result_sha256);
        let mut worker_result_preimage = worker_result.clone();
        worker_result_preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            canonical_json_hash(&worker_result_preimage),
            worker_result_sha256
        );

        let worker_identity_sha256 = receipt["worker_identity_sha256"]
            .as_str()
            .expect("Worker identity semantic hash");
        let worker_identity_object_sha256 = receipt["worker_identity_object_sha256"]
            .as_str()
            .expect("Worker identity object hash");
        let worker_identity_bytes = runtime
            .cas_read(worker_identity_object_sha256)
            .expect("Worker identity CAS readback");
        assert_eq!(
            sha256_hex(&worker_identity_bytes),
            worker_identity_object_sha256
        );
        let worker_identity: Value =
            serde_json::from_slice(&worker_identity_bytes).expect("Worker identity JSON");
        assert_eq!(worker_identity["canonical_sha256"], worker_identity_sha256);
        let mut worker_identity_preimage = worker_identity.clone();
        worker_identity_preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            canonical_json_hash(&worker_identity_preimage),
            worker_identity_sha256
        );
        assert_eq!(worker_identity["worker_id"], KNIFE_WORKER_ID);
        assert_eq!(worker_identity["blender_version"], KNIFE_BLENDER_VERSION);
        assert_eq!(worker_identity["blender_revision"], KNIFE_BLENDER_REVISION);

        let artifacts = receipt["artifacts"].as_array().expect("adopted artifacts");
        let result_outputs = worker_result["outputs"].as_array().expect("Worker outputs");
        assert_eq!(artifacts.len(), result_outputs.len());
        assert!(!artifacts.is_empty());

        let mut result_by_path = BTreeMap::new();
        for output in result_outputs {
            result_by_path.insert(
                output["relative_path"]
                    .as_str()
                    .expect("Worker output path"),
                output,
            );
        }
        let mut identity_by_path = BTreeMap::new();
        for output in worker_identity["outputs"]
            .as_array()
            .expect("identity outputs")
        {
            identity_by_path.insert(
                output["relative_path"]
                    .as_str()
                    .expect("identity output path"),
                output,
            );
        }
        assert_eq!(identity_by_path.len(), artifacts.len());

        let mut kinds = BTreeSet::new();
        for artifact in artifacts {
            let path = artifact["relative_path"].as_str().expect("adopted path");
            let output_kind = artifact["output_kind"]
                .as_str()
                .expect("adopted output kind");
            let semantic_sha256 = artifact["semantic_sha256"].as_str().expect("artifact hash");
            let object_sha256 = artifact["object_sha256"]
                .as_str()
                .expect("artifact object hash");
            let bytes = runtime
                .cas_read(object_sha256)
                .expect("artifact CAS readback");
            assert_eq!(sha256_hex(&bytes), object_sha256);
            assert_eq!(semantic_sha256, object_sha256);
            assert_eq!(artifact["byte_size"].as_u64(), Some(bytes.len() as u64));

            let result_output = result_by_path.get(path).expect("result output path");
            assert_eq!(result_output["sha256"].as_str(), Some(semantic_sha256));
            assert_eq!(
                result_output["byte_size"].as_u64(),
                Some(bytes.len() as u64)
            );
            let identity_output = identity_by_path.get(path).expect("identity output path");
            assert_eq!(identity_output["sha256"].as_str(), Some(semantic_sha256));
            assert_eq!(
                identity_output["byte_size"].as_u64(),
                Some(bytes.len() as u64)
            );
            kinds.insert(output_kind);
        }

        assert!(kinds.contains("high_glb"), "High GLB was not adopted");
        assert!(kinds.contains("low_glb"), "Low GLB was not adopted");
        assert!(kinds.contains("normal_map"), "normal maps were not adopted");
        assert!(kinds.contains("ao_map"), "AO maps were not adopted");
        assert!(
            kinds.contains("worker_manifest"),
            "Worker manifest was not adopted"
        );

        let execution_id = adopted["execution_id"]
            .as_str()
            .expect("execution id")
            .to_owned();
        let worker_result_object_sha256 = adopted["worker_result_object_sha256"]
            .as_str()
            .expect("Worker result object hash")
            .to_owned();
        let durable_receipt_object_sha256 = adopted["receipt_object_sha256"]
            .as_str()
            .expect("receipt object hash")
            .to_owned();
        drop(runtime);

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopened Runtime");
        let found = reopened
            .weaponry_blender_knife_worker_execution_get_internal(
                request["project_id"].as_str().expect("project id"),
                &execution_id,
                &source_sha256,
                &worker_result_object_sha256,
                &durable_receipt_object_sha256,
            )
            .expect("exact durable Worker get after Runtime restart");
        assert_eq!(found["status"], "found");
        assert_eq!(found["replayed"], false);
        assert_eq!(found["store_effect"], "not-touched");
        assert_eq!(found["cas_effect"], "not-touched");
        assert_eq!(found["runtime_write_performed"], false);
        assert_eq!(found["persistent_user_data_touched"], false);
        for field in [
            "execution_id",
            "source_object_sha256",
            "worker_result_sha256",
            "worker_result_object_sha256",
            "receipt_sha256",
            "receipt_object_sha256",
            "high_glb_sha256",
            "high_glb_object_sha256",
            "low_glb_sha256",
            "low_glb_object_sha256",
            "normal_map_set_sha256",
            "ao_map_set_sha256",
            "all_artifact_set_sha256",
        ] {
            assert_eq!(
                found[field], adopted[field],
                "reopen get drifted at {field}"
            );
        }

        // An exact replay proves that Runtime does not execute Blender twice;
        // it does not prove Blender process determinism. Run one independent
        // closed request against the same source and compare only asset bytes.
        // Request/project-bound manifests and receipts are expected to differ.
        let independent_project = reopened
            .create_project(
                "Dragonfang fixed Blender Worker determinism witness",
                json!({"profile":"weaponry-knife-p0"}),
            )
            .expect("create independent determinism project");
        let mut independent_request = request.clone();
        independent_request["request_id"] =
            Value::String("dragonfang-r8-runtime-blender-live-independent".to_owned());
        independent_request["project_id"] = Value::String(independent_project.project_id);
        independent_request["canonical_sha256"] = Value::String(String::new());
        independent_request["canonical_sha256"] =
            Value::String(canonical_json_hash(&independent_request));
        let independent = reopened
            .weaponry_blender_knife_worker_execute_durable_internal(
                &independent_request,
                &source_sha256,
            )
            .expect("execute independent determinism witness");
        assert_eq!(independent["status"], "prepared");
        for field in [
            "high_glb_sha256",
            "high_glb_object_sha256",
            "low_glb_sha256",
            "low_glb_object_sha256",
            "normal_map_set_sha256",
            "ao_map_set_sha256",
        ] {
            assert_eq!(
                independent[field], adopted[field],
                "independent Worker execution drifted at {field}"
            );
        }

        println!(
            "DRAGONFANG_R8_BLENDER_RUNTIME_DURABLE source={} execution={} worker_identity={} worker_identity_object={} worker_result={} worker_result_object={} receipt={} receipt_object={} high={} low={} normal_set={} ao_set={} all_set={} artifacts={}",
            source_sha256,
            execution_id,
            worker_identity_sha256,
            worker_identity_object_sha256,
            worker_result_sha256,
            worker_result_object_sha256,
            receipt["canonical_sha256"].as_str().expect("receipt hash"),
            receipt_object_sha256,
            adopted["high_glb_object_sha256"].as_str().expect("High object hash"),
            adopted["low_glb_object_sha256"].as_str().expect("Low object hash"),
            adopted["normal_map_set_sha256"].as_str().expect("normal set hash"),
            adopted["ao_map_set_sha256"].as_str().expect("AO set hash"),
            adopted["all_artifact_set_sha256"].as_str().expect("all artifact set hash"),
            artifacts
                .iter()
                .map(|artifact| format!(
                    "{}={}",
                    artifact["output_kind"].as_str().expect("artifact kind"),
                    artifact["object_sha256"].as_str().expect("artifact object hash")
                ))
                .collect::<Vec<_>>()
                .join(","),
        );
        drop(reopened);
        fs::remove_dir_all(&root).expect("remove live Runtime root");
    }
}
