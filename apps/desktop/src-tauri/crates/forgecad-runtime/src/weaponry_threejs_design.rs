//! Runtime-owned durable intake for the lightweight Three.js knife workbench.
//!
//! The first vertical slice stores a closed `KnifeSceneProgram@1`. It does
//! not run TypeScript: later build/preview/export operations must cross a
//! fixed typed worker boundary and bind their output back to this identity.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, Runtime, RuntimeError,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_store::{
    WeaponryThreeJsDesignCommit, WeaponryThreeJsDesignStoreRecord, WeaponryThreeJsExecutionCommit,
    WeaponryThreeJsExecutionStoreRecord, WeaponryThreeJsPreviewCommit,
    WeaponryThreeJsPreviewStoreRecord, WEAPONRY_THREEJS_DESIGN_RECORD_SCHEMA,
    WEAPONRY_THREEJS_EXECUTION_RECORD_SCHEMA, WEAPONRY_THREEJS_GLB_KIND, WEAPONRY_THREEJS_GLB_MIME,
    WEAPONRY_THREEJS_MAX_GLB_BYTES, WEAPONRY_THREEJS_MAX_PROGRAM_BYTES,
    WEAPONRY_THREEJS_PREVIEW_AOVS_PER_VIEW, WEAPONRY_THREEJS_PREVIEW_AOV_COUNT,
    WEAPONRY_THREEJS_PREVIEW_AOV_KIND, WEAPONRY_THREEJS_PREVIEW_AOV_MIME,
    WEAPONRY_THREEJS_PREVIEW_MAX_AOV_BYTES, WEAPONRY_THREEJS_PREVIEW_RECEIPT_KIND,
    WEAPONRY_THREEJS_PREVIEW_RECEIPT_MIME, WEAPONRY_THREEJS_PREVIEW_RECORD_SCHEMA,
    WEAPONRY_THREEJS_PREVIEW_RUNTIME_ID, WEAPONRY_THREEJS_PREVIEW_VIEW_COUNT,
    WEAPONRY_THREEJS_PROGRAM_MIME, WEAPONRY_THREEJS_PROGRAM_OBJECT_KIND,
    WEAPONRY_THREEJS_PROGRAM_SCHEMA, WEAPONRY_THREEJS_WORKER_RESULT_KIND,
    WEAPONRY_THREEJS_WORKER_RESULT_MIME,
};
use image::ImageDecoder;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub(crate) const PREPARE_OPERATION: &str = "weaponry_threejs_knife_design_prepare";
pub(crate) const GET_OPERATION: &str = "weaponry_threejs_knife_design_get";
pub(crate) const EXECUTE_OPERATION: &str = "weaponry_threejs_knife_design_execute";
const PREPARE_SCHEMA: &str = "WeaponryThreeJsKnifeDesignPrepareRequest@1";
const GET_SCHEMA: &str = "WeaponryThreeJsKnifeDesignGetRequest@1";
const RESULT_SCHEMA: &str = "WeaponryThreeJsKnifeDesignResult@1";
const EXECUTE_SCHEMA: &str = "WeaponryThreeJsKnifeDesignExecuteRequest@1";
const EXECUTION_RESULT_SCHEMA: &str = "WeaponryThreeJsKnifeDesignExecutionResult@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const INPUT_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
const RESULT_CANONICALIZATION: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "program",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "design_id",
    "program_sha256",
    "program_object_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];
const EXECUTE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "action",
    "project_id",
    "design_id",
    "program_sha256",
    "program_object_sha256",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];
const FIXED_WORKER_REQUEST_SCHEMA: &str = "WeaponryThreeJsFixedWorkerRequest@1";
const FIXED_WORKER_RESULT_SCHEMA: &str = "WeaponryThreeJsFixedWorkerResult@1";
const FIXED_WORKER_ID: &str = "weaponry-threejs-fixed-knife-worker@1";
const FIXED_WORKER_MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
const PREVIEW_RECEIPT_OPERATION: &str = "weaponry_threejs_knife_design_preview";
const PREVIEW_RECEIPT_SCHEMA: &str = "WeaponryThreeJsPreviewReceipt@1";
const PREVIEW_VIEW_IDS: [&str; 8] = [
    "FRONT",
    "BACK",
    "TOP",
    "BOTTOM",
    "LEFT",
    "RIGHT",
    "REAR_THREE_QUARTER",
    "FPS_HOLD",
];
const PREVIEW_AOV_IDS: [&str; 6] = [
    "beauty",
    "alpha-silhouette",
    "semantic-id",
    "depth",
    "normal",
    "roughness-material-id",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "WEAPONRY_THREEJS_DESIGN_INVALID: {}",
        message.into()
    ))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} must be an object")))?;
    let actual: BTreeSet<&str> = object.keys().map(String::as_str).collect();
    let expected: BTreeSet<&str> = fields.iter().copied().collect();
    if actual != expected {
        return Err(invalid(format!("{label} fields are not closed")));
    }
    Ok(object)
}

fn text(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} must be an opaque identifier")))
}

fn hash(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} must be a SHA-256")))
}

fn validate_header(
    request: &Value,
    object: &Map<String, Value>,
    schema: &str,
    operation: &str,
    read_only: bool,
) -> Result<(), RuntimeError> {
    if object.get("schema_version").and_then(Value::as_str) != Some(schema)
        || object.get("operation").and_then(Value::as_str) != Some(operation)
        || object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES)
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("writer_policy").and_then(Value::as_str) != Some(WRITER_POLICY)
        || object
            .get("canonicalization_policy")
            .and_then(Value::as_str)
            != Some(INPUT_CANONICALIZATION)
        || (read_only
            && object
                .get("persistent_user_data_touched")
                .and_then(Value::as_bool)
                != Some(false))
    {
        return Err(invalid("request header or fixed policy differs"));
    }
    let supplied = hash(object, "input_sha256")?;
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(invalid("input_sha256 differs from the canonical request"));
    }
    Ok(())
}

fn safe_tree(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            text.len() <= 256
                && !text.chars().any(char::is_control)
                && !text.starts_with('/')
                && !text.starts_with('\\')
                && !lower.starts_with("file:")
                && !lower.starts_with("data:")
                && !lower.starts_with("http:")
                && !lower.starts_with("https:")
                && !lower.contains("api_key")
                && !lower.contains("password")
                && !lower.contains("secret:")
                && !lower.contains("token:")
        }
        Value::Array(values) => values.len() <= 256 && values.iter().all(safe_tree),
        Value::Object(object) => {
            object.len() <= 64
                && object.keys().all(|key| {
                    !matches!(
                        key.to_ascii_lowercase().as_str(),
                        "url" | "path" | "file_path" | "script" | "javascript" | "python"
                    )
                })
                && object.values().all(safe_tree)
        }
        _ => true,
    }
}

struct ProgramBinding {
    value: Value,
    asset_id: String,
    family: String,
    semantic_sha256: String,
    part_count: u64,
    material_zone_count: u64,
}

fn validate_program(value: Value) -> Result<ProgramBinding, RuntimeError> {
    if !safe_tree(&value) {
        return Err(invalid(
            "KnifeSceneProgram contains an unbounded path, URL, script or secret-like value",
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid("program must be an object"))?;
    let required = [
        "asset_id",
        "blade_surface",
        "budgets",
        "canonical_sha256",
        "coordinate_convention",
        "design_basis",
        "family",
        "material_zones",
        "parts",
        "presentation",
        "schema_version",
        "unknowns",
    ];
    let allowed: BTreeSet<&str> = required
        .iter()
        .copied()
        .chain(std::iter::once("assembly"))
        .collect();
    if required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !allowed.contains(key.as_str()))
        || object.get("schema_version").and_then(Value::as_str)
            != Some(WEAPONRY_THREEJS_PROGRAM_SCHEMA)
        || object.get("coordinate_convention").and_then(Value::as_str)
            != Some("weapon-front-z-up-right-handed@1")
        || !matches!(
            object.get("design_basis").and_then(Value::as_str),
            Some("authorized-reference-inspired" | "original-design")
        )
    {
        return Err(invalid("KnifeSceneProgram root contract is invalid"));
    }
    let asset_id = text(object, "asset_id")?;
    let family = text(object, "family")?;
    if !matches!(
        family.as_str(),
        "kukri" | "tanto" | "karambit" | "bayonet" | "machete" | "original-knife"
    ) {
        return Err(invalid("knife family is outside the closed allowlist"));
    }
    let parts = object
        .get("parts")
        .and_then(Value::as_array)
        .filter(|parts| (2..=64).contains(&parts.len()))
        .ok_or_else(|| invalid("parts must contain 2..=64 entries"))?;
    let zones = object
        .get("material_zones")
        .and_then(Value::as_array)
        .filter(|zones| (1..=32).contains(&zones.len()))
        .ok_or_else(|| invalid("material_zones must contain 1..=32 entries"))?;
    let mut zone_ids = BTreeSet::new();
    for zone in zones {
        let zone = zone
            .as_object()
            .ok_or_else(|| invalid("material zone must be an object"))?;
        let id = text(zone, "material_zone_id")?;
        if !zone_ids.insert(id) {
            return Err(invalid("material_zone_id values must be unique"));
        }
    }
    let mut part_ids = BTreeSet::new();
    for part in parts {
        let part = part
            .as_object()
            .ok_or_else(|| invalid("part must be an object"))?;
        let id = text(part, "part_id")?;
        let material = text(part, "material_zone_id")?;
        if !part_ids.insert(id) || !zone_ids.contains(&material) {
            return Err(invalid(
                "part IDs must be unique and every part must bind a declared material zone",
            ));
        }
    }
    let presentation = object
        .get("presentation")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("presentation must be an object"))?;
    if presentation.get("camera_set").and_then(Value::as_str) != Some("knife-fixed-eight-view@1")
        || presentation.get("renderer").and_then(Value::as_str)
            != Some("threejs-browser-authority@1")
    {
        return Err(invalid(
            "presentation must bind the fixed knife camera/renderer",
        ));
    }
    let semantic = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("program canonical_sha256 is missing"))?
        .to_owned();
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != semantic {
        return Err(invalid(
            "program canonical_sha256 differs from its semantic preimage",
        ));
    }
    let part_count = parts.len() as u64;
    let material_zone_count = zones.len() as u64;
    Ok(ProgramBinding {
        value,
        asset_id,
        family,
        semantic_sha256: semantic,
        part_count,
        material_zone_count,
    })
}

fn result_value(
    record: &WeaponryThreeJsDesignStoreRecord,
    program: Value,
    request_kind: &str,
    replayed: bool,
) -> Result<Value, RuntimeError> {
    let mut result = json!({
        "schema_version": RESULT_SCHEMA,
        "operation": if request_kind == "prepare" { PREPARE_OPERATION } else { GET_OPERATION },
        "request_kind": request_kind,
        "status": if replayed { "replayed" } else if request_kind == "prepare" { "prepared" } else { "found" },
        "project_id": record.project_id,
        "design_id": record.design_id,
        "asset_id": record.asset_id,
        "family": record.family,
        "program_sha256": record.program_sha256,
        "program_object_sha256": record.program_object_sha256,
        "program": program,
        "part_count": record.part_count,
        "material_zone_count": record.material_zone_count,
        "idempotency_key": if request_kind == "prepare" { Value::String(record.idempotency_key.clone()) } else { Value::Null },
        "replayed": replayed,
        "store_effect": if request_kind == "prepare" && !replayed { "inserted" } else { "not-touched" },
        "cas_effect": if request_kind == "prepare" && !replayed { "inserted" } else { "not-touched" },
        "runtime_write_performed": request_kind == "prepare" && !replayed,
        "persistent_user_data_touched": request_kind == "prepare" && !replayed,
        "worker_execution_status": record.execution_status,
        "glb_created": false,
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "commercial_status": "NOT_RUN",
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": RESULT_CANONICALIZATION,
        "canonical_sha256": ""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    let bytes = canonical_json_bytes(&result).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(invalid("result exceeds max_response_bytes"));
    }
    Ok(result)
}

fn source_fixed_worker_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../packages/weaponry-threejs/scripts/fixed-worker.mjs")
}

struct FixedWorkerLaunch {
    runtime: PathBuf,
    entry: PathBuf,
    packaged: bool,
}

fn packaged_worker_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.join("../Resources/weaponry-threejs-worker"));
            roots.push(parent.join("weaponry-threejs-worker"));
        }
    }
    roots.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/weaponry-threejs-worker"),
    );
    roots
}

fn file_sha256(path: &std::path::Path) -> Result<String, RuntimeError> {
    let bytes = std::fs::read(path)
        .map_err(|error| invalid(format!("packaged resource could not be read: {error}")))?;
    Ok(super::sha256_hex(&bytes))
}

fn tree_sha256(root: &std::path::Path, excluded: &[&str]) -> Result<String, RuntimeError> {
    fn visit(
        root: &std::path::Path,
        current: &std::path::Path,
        files: &mut Vec<PathBuf>,
    ) -> Result<(), RuntimeError> {
        for entry in std::fs::read_dir(current).map_err(|error| {
            invalid(format!("packaged resource tree could not be read: {error}"))
        })? {
            let path = entry
                .map_err(|error| invalid(format!("packaged resource entry is invalid: {error}")))?
                .path();
            if path.is_dir() {
                visit(root, &path, files)?;
            } else if path.is_file() {
                files.push(path);
            }
        }
        let _ = root;
        Ok(())
    }
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort_by_key(|path| {
        path.strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    });
    let mut digest = Sha256::new();
    for path in files {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| invalid("packaged resource escaped its root"))?
            .to_string_lossy()
            .replace('\\', "/");
        if excluded.contains(&relative.as_str()) {
            continue;
        }
        let bytes = std::fs::read(&path)
            .map_err(|error| invalid(format!("packaged resource could not be hashed: {error}")))?;
        digest.update(relative.as_bytes());
        digest.update(b"\0");
        digest.update(bytes.len().to_string().as_bytes());
        digest.update(b"\0");
        digest.update(&bytes);
        digest.update(b"\0");
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn manifest_hash(object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let supplied = object
        .get("manifest_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("packaged Worker manifest hash is invalid"))?;
    let mut preimage = Value::Object(object.clone());
    preimage["manifest_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(invalid("packaged Worker manifest canonical hash differs"));
    }
    Ok(supplied.to_owned())
}

fn verify_packaged_worker(
    root: &std::path::Path,
    object: &Map<String, Value>,
    action: &str,
) -> Result<(), RuntimeError> {
    manifest_hash(object)?;
    for (field, relative_root) in [
        ("worker_source_tree_sha256", "worker"),
        ("npm_dependency_tree_sha256", "worker/node_modules"),
        ("three_dependency_tree_sha256", "worker/node_modules/three"),
        ("runtime_tree_sha256", "runtime"),
    ] {
        let expected = object
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid(format!("packaged Worker {field} is invalid")))?;
        if tree_sha256(&root.join(relative_root), &[])? != expected {
            return Err(invalid(format!("packaged Worker {field} drifted")));
        }
    }
    let runtime_expected = object
        .get("runtime_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("packaged Worker runtime hash is invalid"))?;
    if file_sha256(&root.join("runtime/node"))? != runtime_expected {
        return Err(invalid("packaged Worker runtime bytes drifted"));
    }
    let resource_expected = object
        .get("resource_tree_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("packaged Worker resource tree hash is invalid"))?;
    if tree_sha256(root, &["weaponry-threejs-worker-manifest.json"])? != resource_expected {
        return Err(invalid("packaged Worker resource tree drifted"));
    }
    if object.get("status").and_then(Value::as_str) == Some("PACKAGED_RELOCATABLE") {
        let cohort = object
            .get("build_cohort_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("packaged Worker build cohort is invalid"))?;
        if super::build_cohort_sha256().as_deref() != Some(cohort) {
            return Err(invalid("packaged Worker build cohort differs from Runtime"));
        }
    }
    if action == "preview" {
        let preview = object
            .get("preview_runtime")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("packaged preview runtime is missing"))?;
        for (path_field, hash_field) in [
            ("executable_path", "executable_sha256"),
            ("license_path", "license_sha256"),
        ] {
            let relative = preview
                .get(path_field)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty() && !value.contains(".."))
                .ok_or_else(|| invalid(format!("packaged preview {path_field} is invalid")))?;
            let expected = preview
                .get(hash_field)
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid(format!("packaged preview {hash_field} is invalid")))?;
            if file_sha256(&root.join(relative))? != expected {
                return Err(invalid(format!("packaged preview {path_field} drifted")));
            }
        }
    }
    Ok(())
}

fn fixed_worker_launch(action: &str) -> Result<FixedWorkerLaunch, RuntimeError> {
    for root in packaged_worker_roots() {
        let manifest_path = root.join("weaponry-threejs-worker-manifest.json");
        if !manifest_path.is_file() {
            continue;
        }
        let manifest_bytes = std::fs::read(&manifest_path).map_err(|error| {
            invalid(format!(
                "packaged Worker manifest could not be read: {error}"
            ))
        })?;
        let manifest: Value = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| invalid(format!("packaged Worker manifest is invalid: {error}")))?;
        let object = manifest
            .as_object()
            .ok_or_else(|| invalid("packaged Worker manifest must be an object"))?;
        let status = object.get("status").and_then(Value::as_str);
        let runtime_entry = object.get("runtime_entry").and_then(Value::as_str);
        let worker_entry = object.get("worker_entry").and_then(Value::as_str);
        let preview_packaged = object
            .get("preview_runtime")
            .and_then(Value::as_object)
            .and_then(|preview| preview.get("packaged"))
            .and_then(Value::as_bool);
        if object.get("schema_version").and_then(Value::as_str)
            != Some("WeaponryThreeJsPackagedWorkerManifest@1")
            || runtime_entry != Some("runtime/node")
            || worker_entry != Some("worker/scripts/fixed-worker.mjs")
        {
            return Err(invalid("packaged Worker manifest identity drifted"));
        }
        if action == "preview"
            && (status != Some("PACKAGED_RELOCATABLE") || preview_packaged != Some(true))
        {
            continue;
        }
        verify_packaged_worker(&root, object, action)?;
        let runtime = root.join(runtime_entry.expect("checked runtime entry"));
        let entry = root.join(worker_entry.expect("checked worker entry"));
        if !runtime.is_file() || !entry.is_file() {
            return Err(invalid("packaged Worker runtime or entry is missing"));
        }
        return Ok(FixedWorkerLaunch {
            runtime,
            entry,
            packaged: true,
        });
    }
    if action == "preview" && std::env::var("WPN_THREEJS_PREVIEW_SOURCE_LIVE").as_deref() != Ok("1")
    {
        return Err(invalid(
            "packaged preview runtime is unavailable; explicit source-live mode was not enabled",
        ));
    }
    let entry = source_fixed_worker_path();
    if !entry.is_file() {
        return Err(invalid("fixed Three.js Worker is not installed"));
    }
    Ok(FixedWorkerLaunch {
        runtime: PathBuf::from("node"),
        entry,
        packaged: false,
    })
}

fn invoke_fixed_worker(
    action: &str,
    record: &WeaponryThreeJsDesignStoreRecord,
    program: &Value,
) -> Result<Value, RuntimeError> {
    let request = json!({
        "schema_version": FIXED_WORKER_REQUEST_SCHEMA,
        "operation": action,
        "program_sha256": record.program_sha256,
        "program_object_sha256": record.program_object_sha256,
        "program": program,
        "max_response_bytes": FIXED_WORKER_MAX_RESPONSE_BYTES
    });
    let request_bytes =
        canonical_json_bytes(&request).map_err(|error| invalid(error.to_string()))?;
    let launch = fixed_worker_launch(action)?;
    let mut command = Command::new(&launch.runtime);
    command
        .arg("--experimental-strip-types")
        .arg(&launch.entry)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if launch.packaged {
        command.env_remove("WPN_THREEJS_PREVIEW_SOURCE_LIVE");
        command.env_remove("WPN_CHROME_EXECUTABLE");
    }
    let mut child = command
        .spawn()
        .map_err(|error| invalid(format!("fixed Worker could not start: {error}")))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| invalid("fixed Worker stdin is unavailable"))?
        .write_all(&request_bytes)
        .map_err(|error| invalid(format!("fixed Worker request failed: {error}")))?;
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .map_err(|error| invalid(format!("fixed Worker wait failed: {error}")))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(invalid(format!(
            "fixed Worker rejected the request: {}",
            detail.chars().take(512).collect::<String>()
        )));
    }
    if output.stdout.is_empty() || output.stdout.len() as u64 > FIXED_WORKER_MAX_RESPONSE_BYTES {
        return Err(invalid(
            "fixed Worker response exceeds its bounded envelope",
        ));
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| invalid(format!("fixed Worker result is invalid JSON: {error}")))?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("fixed Worker result must be an object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some(FIXED_WORKER_RESULT_SCHEMA)
        || object.get("worker_id").and_then(Value::as_str) != Some(FIXED_WORKER_ID)
        || object.get("operation").and_then(Value::as_str) != Some(action)
        || object.get("program_sha256").and_then(Value::as_str)
            != Some(record.program_sha256.as_str())
        || object.get("program_object_sha256").and_then(Value::as_str)
            != Some(record.program_object_sha256.as_str())
        || object.get("renderer_invoked").and_then(Value::as_bool) != Some(action == "preview")
        || object.get("visual_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("human_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("commercial_status").and_then(Value::as_str) != Some("NOT_RUN")
    {
        return Err(invalid(
            "fixed Worker result identity or quality flags drifted",
        ));
    }
    let supplied = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("fixed Worker canonical_sha256 is invalid"))?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(invalid("fixed Worker result canonical hash differs"));
    }
    Ok(value)
}

struct PreviewTransport {
    durable_worker_result: Value,
    pngs: Vec<Vec<u8>>,
}

fn validate_preview_transport(mut raw: Value) -> Result<PreviewTransport, RuntimeError> {
    let object = raw
        .as_object()
        .ok_or_else(|| invalid("preview Worker result must be an object"))?;
    let payloads = object
        .get("preview_payloads")
        .and_then(Value::as_array)
        .filter(|items| items.len() == WEAPONRY_THREEJS_PREVIEW_AOV_COUNT)
        .ok_or_else(|| invalid("preview transport must contain exactly 48 PNG payloads"))?;
    let views = object
        .get("preview_views")
        .and_then(Value::as_array)
        .filter(|items| items.len() == WEAPONRY_THREEJS_PREVIEW_VIEW_COUNT)
        .ok_or_else(|| invalid("preview Worker must contain exactly eight views"))?;
    if object.get("preview_runtime_id").and_then(Value::as_str)
        != Some(WEAPONRY_THREEJS_PREVIEW_RUNTIME_ID)
        || object
            .get("preview_runtime_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || object
            .get("preview_dependency_lock_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || object
            .get("preview_worker_cohort_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        || object.get("preview_view_count").and_then(Value::as_u64)
            != Some(WEAPONRY_THREEJS_PREVIEW_VIEW_COUNT as u64)
        || object.get("preview_aov_count").and_then(Value::as_u64)
            != Some(WEAPONRY_THREEJS_PREVIEW_AOV_COUNT as u64)
    {
        return Err(invalid("preview runtime identity or fixed counts drifted"));
    }
    let cohort = object["preview_worker_cohort_sha256"]
        .as_str()
        .expect("validated cohort");
    let mut pngs = Vec::with_capacity(WEAPONRY_THREEJS_PREVIEW_AOV_COUNT);
    for (view_index, view) in views.iter().enumerate() {
        let view = view
            .as_object()
            .ok_or_else(|| invalid("preview view must be an object"))?;
        let passes = view
            .get("passes")
            .and_then(Value::as_array)
            .filter(|items| items.len() == WEAPONRY_THREEJS_PREVIEW_AOVS_PER_VIEW)
            .ok_or_else(|| invalid("preview view must contain six AOV passes"))?;
        if view.get("view_id").and_then(Value::as_str) != Some(PREVIEW_VIEW_IDS[view_index])
            || view
                .get("camera_sha256")
                .and_then(Value::as_str)
                .is_none_or(|value| !is_sha256(value))
            || view.get("worker_cohort_sha256").and_then(Value::as_str) != Some(cohort)
            || view.get("width").and_then(Value::as_u64) != Some(512)
            || view.get("height").and_then(Value::as_u64) != Some(512)
        {
            return Err(invalid("preview fixed view or camera binding drifted"));
        }
        for (pass_index, pass) in passes.iter().enumerate() {
            let flat_index = view_index * WEAPONRY_THREEJS_PREVIEW_AOVS_PER_VIEW + pass_index;
            let pass = pass
                .as_object()
                .ok_or_else(|| invalid("preview pass must be an object"))?;
            let payload = payloads[flat_index]
                .as_object()
                .ok_or_else(|| invalid("preview PNG payload must be an object"))?;
            if pass.get("aov_id").and_then(Value::as_str) != Some(PREVIEW_AOV_IDS[pass_index])
                || pass.get("mime").and_then(Value::as_str)
                    != Some(WEAPONRY_THREEJS_PREVIEW_AOV_MIME)
                || payload.get("view_id").and_then(Value::as_str)
                    != Some(PREVIEW_VIEW_IDS[view_index])
                || payload.get("aov_id").and_then(Value::as_str)
                    != Some(PREVIEW_AOV_IDS[pass_index])
                || payload.get("mime_type").and_then(Value::as_str)
                    != Some(WEAPONRY_THREEJS_PREVIEW_AOV_MIME)
            {
                return Err(invalid("preview PNG payload order or identity drifted"));
            }
            let encoded = payload
                .get("base64")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("preview PNG payload is missing base64 bytes"))?;
            let bytes = BASE64_STANDARD
                .decode(encoded)
                .map_err(|error| invalid(format!("preview PNG base64 is invalid: {error}")))?;
            if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_THREEJS_PREVIEW_MAX_AOV_BYTES {
                return Err(invalid("preview PNG exceeds its bounded capacity"));
            }
            let decoder = image::codecs::png::PngDecoder::new(Cursor::new(&bytes))
                .map_err(|error| invalid(format!("preview PNG is invalid: {error}")))?;
            if decoder.dimensions() != (512, 512) {
                return Err(invalid("preview PNG dimensions differ from fixed 512x512"));
            }
            let digest = super::sha256_hex(&bytes);
            if pass.get("sha256").and_then(Value::as_str) != Some(digest.as_str())
                || pass.get("object_sha256").and_then(Value::as_str) != Some(digest.as_str())
                || pass.get("bytes").and_then(Value::as_u64) != Some(bytes.len() as u64)
            {
                return Err(invalid("preview PNG hash or byte count differs"));
            }
            pngs.push(bytes);
        }
    }
    raw.as_object_mut()
        .expect("preview object")
        .remove("preview_payloads");
    raw["canonical_sha256"] = Value::String(String::new());
    raw["canonical_sha256"] = Value::String(canonical_json_hash(&raw));
    Ok(PreviewTransport {
        durable_worker_result: raw,
        pngs,
    })
}

fn glb_u32(bytes: &[u8], offset: usize, label: &str) -> Result<u32, RuntimeError> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| invalid(format!("Three.js GLB {label} is truncated")))?;
    Ok(u32::from_le_bytes(
        slice.try_into().expect("bounded four-byte slice"),
    ))
}

fn validate_threejs_glb(bytes: &[u8], part_ids: &[String]) -> Result<(), RuntimeError> {
    if bytes.len() < 28 || bytes.get(..4) != Some(b"glTF") {
        return Err(invalid("fixed Worker output is not a GLB"));
    }
    if glb_u32(bytes, 4, "version")? != 2
        || glb_u32(bytes, 8, "declared length")? as usize != bytes.len()
    {
        return Err(invalid("Three.js GLB header version or length differs"));
    }
    let json_len = glb_u32(bytes, 12, "JSON chunk length")? as usize;
    let json_type = glb_u32(bytes, 16, "JSON chunk type")?;
    let json_end = 20usize
        .checked_add(json_len)
        .ok_or_else(|| invalid("Three.js GLB JSON chunk length overflowed"))?;
    if json_type != 0x4E4F_534A || json_len == 0 || json_len % 4 != 0 || json_end + 8 > bytes.len()
    {
        return Err(invalid("Three.js GLB JSON chunk is invalid"));
    }
    let bin_len = glb_u32(bytes, json_end, "BIN chunk length")? as usize;
    let bin_type = glb_u32(bytes, json_end + 4, "BIN chunk type")?;
    let bin_end = json_end
        .checked_add(8)
        .and_then(|offset| offset.checked_add(bin_len))
        .ok_or_else(|| invalid("Three.js GLB BIN chunk length overflowed"))?;
    if bin_type != 0x004E_4942 || bin_len == 0 || bin_len % 4 != 0 || bin_end != bytes.len() {
        return Err(invalid("Three.js GLB BIN chunk is invalid"));
    }
    let document: Value = serde_json::from_slice(&bytes[20..json_end])
        .map_err(|error| invalid(format!("Three.js GLB JSON is invalid: {error}")))?;
    let root = document
        .as_object()
        .ok_or_else(|| invalid("Three.js GLB JSON root must be an object"))?;
    let asset = root
        .get("asset")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Three.js GLB asset metadata is missing"))?;
    if asset.get("version").and_then(Value::as_str) != Some("2.0")
        || asset.get("generator").and_then(Value::as_str) != Some("THREE.GLTFExporter r185")
        || root.get("scene").and_then(Value::as_u64) != Some(0)
        || root.get("scenes").and_then(Value::as_array).map(Vec::len) != Some(1)
    {
        return Err(invalid(
            "Three.js GLB asset, generator or scene authority differs",
        ));
    }
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Three.js GLB nodes are missing"))?;
    let meshes = root
        .get("meshes")
        .and_then(Value::as_array)
        .filter(|meshes| meshes.len() == part_ids.len())
        .ok_or_else(|| invalid("Three.js GLB mesh count differs from the Worker result"))?;
    let expected_parts: BTreeSet<&str> = part_ids.iter().map(String::as_str).collect();
    if expected_parts.len() != part_ids.len() {
        return Err(invalid("fixed Worker part_ids must be unique"));
    }
    let mut bound_parts = BTreeSet::new();
    let mut bound_meshes = BTreeSet::new();
    for node in nodes {
        let Some(node) = node.as_object() else {
            return Err(invalid("Three.js GLB node must be an object"));
        };
        let Some(name) = node.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(part_id) = name.strip_prefix("knife-part:") else {
            continue;
        };
        let mesh_index = node
            .get("mesh")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .filter(|index| *index < meshes.len())
            .ok_or_else(|| invalid("Three.js GLB Part node has no valid mesh"))?;
        if !expected_parts.contains(part_id)
            || !bound_parts.insert(part_id)
            || !bound_meshes.insert(mesh_index)
        {
            return Err(invalid(
                "Three.js GLB Part-to-mesh lineage is duplicate or unknown",
            ));
        }
        let primitives = meshes[mesh_index]
            .get("primitives")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .ok_or_else(|| invalid("Three.js GLB mesh has no primitive"))?;
        if primitives.iter().any(|primitive| {
            primitive
                .get("attributes")
                .and_then(Value::as_object)
                .and_then(|attributes| attributes.get("POSITION"))
                .and_then(Value::as_u64)
                .is_none()
        }) {
            return Err(invalid(
                "Three.js GLB primitive is missing a POSITION accessor",
            ));
        }
    }
    if bound_parts != expected_parts || bound_meshes.len() != meshes.len() {
        return Err(invalid(
            "Three.js GLB does not bind every Worker Part to one mesh",
        ));
    }
    if root
        .get("buffers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|buffer| buffer.get("uri").is_some())
        || root
            .get("images")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|image| image.get("uri").is_some())
    {
        return Err(invalid("Three.js GLB contains an external URI"));
    }
    Ok(())
}

fn execution_result_value(
    record: &WeaponryThreeJsExecutionStoreRecord,
    worker_result: &Value,
    replayed: bool,
) -> Result<Value, RuntimeError> {
    let mut result = json!({
        "schema_version": EXECUTION_RESULT_SCHEMA,
        "operation": EXECUTE_OPERATION,
        "request_kind": "execute",
        "status": if replayed { "replayed" } else { "executed" },
        "execution_status": worker_result["status"],
        "action": record.action,
        "project_id": record.project_id,
        "execution_id": record.execution_id,
        "design_id": record.design_id,
        "program_sha256": record.program_sha256,
        "program_object_sha256": record.program_object_sha256,
        "worker_id": FIXED_WORKER_ID,
        "worker_result_sha256": record.worker_result_sha256,
        "worker_result_object_sha256": record.worker_result_object_sha256,
        "deterministic_fingerprint": worker_result["deterministic_fingerprint"],
        "triangle_count": record.triangle_count,
        "part_ids": worker_result["part_ids"],
        "preview_manifest": worker_result["preview_manifest"],
        "glb_sha256": record.glb_sha256,
        "glb_object_sha256": record.glb_object_sha256,
        "glb_bytes": record.glb_bytes,
        "idempotency_key": record.idempotency_key,
        "replayed": replayed,
        "worker_invoked": !replayed,
        "store_effect": if replayed { "not-touched" } else { "inserted" },
        "cas_effect": if replayed { "not-touched" } else { "inserted" },
        "runtime_write_performed": !replayed,
        "persistent_user_data_touched": !replayed,
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "commercial_status": "NOT_RUN",
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": RESULT_CANONICALIZATION,
        "canonical_sha256": ""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    if canonical_json_bytes(&result)
        .map_err(|error| invalid(error.to_string()))?
        .len() as u64
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("execution result exceeds max_response_bytes"));
    }
    Ok(result)
}

fn preview_execution_result_value(
    record: &WeaponryThreeJsPreviewStoreRecord,
    worker_result: &Value,
    receipt: &Value,
    replayed: bool,
) -> Result<Value, RuntimeError> {
    let mut result = json!({
        "schema_version": EXECUTION_RESULT_SCHEMA,
        "operation": EXECUTE_OPERATION,
        "request_kind": "execute",
        "status": if replayed { "replayed" } else { "executed" },
        "execution_status": "preview-ready",
        "action": "preview",
        "project_id": record.project_id,
        "execution_id": record.execution_id,
        "design_id": record.design_id,
        "program_sha256": record.program_sha256,
        "program_object_sha256": record.program_object_sha256,
        "worker_id": record.worker_id,
        "worker_result_sha256": record.worker_result_sha256,
        "worker_result_object_sha256": record.worker_result_object_sha256,
        "deterministic_fingerprint": worker_result["deterministic_fingerprint"],
        "triangle_count": worker_result["triangle_count"],
        "part_ids": worker_result["part_ids"],
        "preview_manifest": worker_result["preview_manifest"],
        "preview_runtime_id": record.preview_runtime_id,
        "preview_runtime_sha256": record.preview_runtime_sha256,
        "preview_dependency_lock_sha256": record.preview_dependency_lock_sha256,
        "preview_worker_cohort_sha256": record.preview_worker_cohort_sha256,
        "preview_receipt_sha256": record.preview_receipt_sha256,
        "preview_receipt_object_sha256": record.preview_receipt_object_sha256,
        "preview_view_count": record.view_count,
        "preview_aov_count": record.aov_count,
        "preview_views": receipt["views"],
        "renderer_invoked": true,
        "glb_sha256": Value::Null,
        "glb_object_sha256": Value::Null,
        "glb_bytes": 0,
        "idempotency_key": record.idempotency_key,
        "replayed": replayed,
        "worker_invoked": !replayed,
        "store_effect": if replayed { "not-touched" } else { "inserted" },
        "cas_effect": if replayed { "not-touched" } else { "inserted" },
        "runtime_write_performed": !replayed,
        "persistent_user_data_touched": !replayed,
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "commercial_status": "NOT_RUN",
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": RESULT_CANONICALIZATION,
        "canonical_sha256": ""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    if canonical_json_bytes(&result)
        .map_err(|error| invalid(error.to_string()))?
        .len() as u64
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("preview result exceeds max_response_bytes"));
    }
    Ok(result)
}

fn release_preview_reservation(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    objects: &[forgecad_store::CasObject],
    delete_if_unreachable: bool,
) {
    let mut released = BTreeSet::new();
    for object in objects {
        if released.insert(object.record.sha256.as_str()) {
            let _ = runtime.store.release_cas_reservation_object(
                reservation,
                object,
                delete_if_unreachable,
            );
        }
    }
}

fn persist_preview(
    runtime: &Runtime,
    design: &WeaponryThreeJsDesignStoreRecord,
    request_sha256: &str,
    idempotency_key: &str,
    raw_worker_result: Value,
) -> Result<Value, RuntimeError> {
    let transport = validate_preview_transport(raw_worker_result)?;
    let worker = transport
        .durable_worker_result
        .as_object()
        .ok_or_else(|| invalid("durable preview Worker result must be an object"))?;
    let worker_result_sha256 = hash(worker, "canonical_sha256")?;
    let preview_runtime_sha256 = hash(worker, "preview_runtime_sha256")?;
    let preview_dependency_lock_sha256 = hash(worker, "preview_dependency_lock_sha256")?;
    let preview_worker_cohort_sha256 = hash(worker, "preview_worker_cohort_sha256")?;
    let execution_id = format!("three-preview-{}", &request_sha256[..40]);
    let worker_bytes = canonical_json_bytes(&transport.durable_worker_result)
        .map_err(|error| invalid(error.to_string()))?;
    let reservation = runtime.store.begin_cas_reservation();
    let worker_cas = runtime.store.put_object_reserved(
        &reservation,
        &worker_bytes,
        None,
        WEAPONRY_THREEJS_WORKER_RESULT_MIME,
        WEAPONRY_THREEJS_WORKER_RESULT_KIND,
        &super::now_string(),
    )?;
    let mut reserved = vec![worker_cas.clone()];
    let mut aov_objects = Vec::with_capacity(transport.pngs.len());
    for png in &transport.pngs {
        let object = match runtime.store.put_object_reserved(
            &reservation,
            png,
            None,
            WEAPONRY_THREEJS_PREVIEW_AOV_MIME,
            WEAPONRY_THREEJS_PREVIEW_AOV_KIND,
            &super::now_string(),
        ) {
            Ok(value) => value,
            Err(error) => {
                release_preview_reservation(runtime, &reservation, &reserved, true);
                return Err(error.into());
            }
        };
        reserved.push(object.clone());
        aov_objects.push(object.record.clone());
    }
    let views = transport.durable_worker_result["preview_views"].clone();
    let mut receipt = json!({
        "schema_version": PREVIEW_RECEIPT_SCHEMA,
        "operation": PREVIEW_RECEIPT_OPERATION,
        "project_id": design.project_id,
        "execution_id": execution_id,
        "design_id": design.design_id,
        "program_sha256": design.program_sha256,
        "program_object_sha256": design.program_object_sha256,
        "worker_result_sha256": worker_result_sha256,
        "worker_result_object_sha256": worker_cas.record.sha256,
        "preview_runtime_id": WEAPONRY_THREEJS_PREVIEW_RUNTIME_ID,
        "preview_runtime_sha256": preview_runtime_sha256,
        "preview_dependency_lock_sha256": preview_dependency_lock_sha256,
        "preview_worker_cohort_sha256": preview_worker_cohort_sha256,
        "view_count": WEAPONRY_THREEJS_PREVIEW_VIEW_COUNT,
        "aov_count": WEAPONRY_THREEJS_PREVIEW_AOV_COUNT,
        "views": views,
        "canonical_sha256": ""
    });
    receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
    let receipt_sha256 = receipt["canonical_sha256"]
        .as_str()
        .expect("sealed receipt")
        .to_owned();
    let receipt_bytes =
        canonical_json_bytes(&receipt).map_err(|error| invalid(error.to_string()))?;
    let receipt_cas = match runtime.store.put_object_reserved(
        &reservation,
        &receipt_bytes,
        None,
        WEAPONRY_THREEJS_PREVIEW_RECEIPT_MIME,
        WEAPONRY_THREEJS_PREVIEW_RECEIPT_KIND,
        &super::now_string(),
    ) {
        Ok(value) => value,
        Err(error) => {
            release_preview_reservation(runtime, &reservation, &reserved, true);
            return Err(error.into());
        }
    };
    reserved.push(receipt_cas.clone());
    let record = WeaponryThreeJsPreviewStoreRecord {
        schema_version: WEAPONRY_THREEJS_PREVIEW_RECORD_SCHEMA.to_owned(),
        project_id: design.project_id.clone(),
        execution_id,
        design_id: design.design_id.clone(),
        operation: EXECUTE_OPERATION.to_owned(),
        action: "preview".to_owned(),
        program_sha256: design.program_sha256.clone(),
        program_object_sha256: design.program_object_sha256.clone(),
        worker_id: FIXED_WORKER_ID.to_owned(),
        preview_runtime_id: WEAPONRY_THREEJS_PREVIEW_RUNTIME_ID.to_owned(),
        preview_runtime_sha256,
        preview_dependency_lock_sha256,
        preview_worker_cohort_sha256,
        worker_result_sha256,
        worker_result_object_sha256: worker_cas.record.sha256.clone(),
        preview_receipt_sha256: receipt_sha256,
        preview_receipt_object_sha256: receipt_cas.record.sha256.clone(),
        view_count: WEAPONRY_THREEJS_PREVIEW_VIEW_COUNT as u64,
        aov_count: WEAPONRY_THREEJS_PREVIEW_AOV_COUNT as u64,
        request_sha256: request_sha256.to_owned(),
        idempotency_key: idempotency_key.to_owned(),
        created_at: super::now_string(),
    };
    let commit = WeaponryThreeJsPreviewCommit {
        record,
        worker_result: worker_cas.record.clone(),
        receipt: receipt_cas.record.clone(),
        aov_objects,
    };
    let stored = runtime
        .store
        .record_weaponry_threejs_preview_with_replay(&commit);
    let (record, replayed) = match stored {
        Ok(value) => value,
        Err(error) => {
            release_preview_reservation(runtime, &reservation, &reserved, true);
            return Err(error.into());
        }
    };
    release_preview_reservation(runtime, &reservation, &reserved, false);
    let worker = runtime
        .store
        .read_weaponry_threejs_preview_worker_result_json(&record)?;
    let receipt = runtime
        .store
        .read_weaponry_threejs_preview_receipt_json(&record)?;
    preview_execution_result_value(&record, &worker, &receipt, replayed)
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    validate_header(request, object, PREPARE_SCHEMA, PREPARE_OPERATION, false)?;
    let project_id = text(object, "project_id")?;
    let idempotency_key = text(object, "idempotency_key")?;
    let request_sha256 = hash(object, "input_sha256")?;
    if runtime.project(&project_id)?.is_none() {
        return Err(invalid("project does not exist"));
    }
    let program = validate_program(
        object
            .get("program")
            .cloned()
            .ok_or_else(|| invalid("program is missing"))?,
    )?;
    let program_bytes =
        canonical_json_bytes(&program.value).map_err(|error| invalid(error.to_string()))?;
    if program_bytes.len() as u64 > WEAPONRY_THREEJS_MAX_PROGRAM_BYTES {
        return Err(invalid(
            "KnifeSceneProgram exceeds the bounded CAS capacity",
        ));
    }
    let design_id = format!("three-design-{}", &program.semantic_sha256[..40]);
    let reservation = runtime.store.begin_cas_reservation();
    let cas = runtime.store.put_object_reserved(
        &reservation,
        &program_bytes,
        None,
        WEAPONRY_THREEJS_PROGRAM_MIME,
        WEAPONRY_THREEJS_PROGRAM_OBJECT_KIND,
        &super::now_string(),
    )?;
    let commit = WeaponryThreeJsDesignCommit {
        record: WeaponryThreeJsDesignStoreRecord {
            schema_version: WEAPONRY_THREEJS_DESIGN_RECORD_SCHEMA.to_owned(),
            project_id,
            design_id,
            asset_id: program.asset_id,
            family: program.family,
            program_sha256: program.semantic_sha256,
            program_object_sha256: cas.record.sha256.clone(),
            part_count: program.part_count,
            material_zone_count: program.material_zone_count,
            request_sha256,
            idempotency_key,
            execution_status: "NOT_RUN_FIXED_WORKER".to_owned(),
            created_at: super::now_string(),
        },
        program: cas.record.clone(),
    };
    let stored = runtime
        .store
        .record_weaponry_threejs_design_with_replay(&commit);
    let (record, replayed) = match stored {
        Ok(value) => {
            let _ = runtime
                .store
                .release_cas_reservation_object(&reservation, &cas, false);
            value
        }
        Err(error) => {
            let _ = runtime
                .store
                .release_cas_reservation_object(&reservation, &cas, true);
            return Err(error.into());
        }
    };
    let stored_program = runtime.store.read_weaponry_threejs_program_json(&record)?;
    result_value(&record, stored_program, "prepare", replayed)
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_FIELDS, GET_SCHEMA)?;
    validate_header(request, object, GET_SCHEMA, GET_OPERATION, true)?;
    let project_id = text(object, "project_id")?;
    let design_id = text(object, "design_id")?;
    let program_sha256 = hash(object, "program_sha256")?;
    let program_object_sha256 = hash(object, "program_object_sha256")?;
    let record = runtime
        .store
        .get_weaponry_threejs_design_exact(
            &project_id,
            &design_id,
            &program_sha256,
            &program_object_sha256,
        )?
        .ok_or_else(|| invalid("exact Three.js design binding was not found"))?;
    let program = runtime.store.read_weaponry_threejs_program_json(&record)?;
    result_value(&record, program, "get", false)
}

pub(crate) fn execute(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, EXECUTE_FIELDS, EXECUTE_SCHEMA)?;
    validate_header(request, object, EXECUTE_SCHEMA, EXECUTE_OPERATION, false)?;
    let action = text(object, "action")?;
    if !matches!(action.as_str(), "build" | "preview" | "export") {
        return Err(invalid("action must be build, preview or export"));
    }
    let project_id = text(object, "project_id")?;
    let design_id = text(object, "design_id")?;
    let program_sha256 = hash(object, "program_sha256")?;
    let program_object_sha256 = hash(object, "program_object_sha256")?;
    let idempotency_key = text(object, "idempotency_key")?;
    let request_sha256 = hash(object, "input_sha256")?;
    let design = runtime
        .store
        .get_weaponry_threejs_design_exact(
            &project_id,
            &design_id,
            &program_sha256,
            &program_object_sha256,
        )?
        .ok_or_else(|| invalid("exact durable Three.js design was not found"))?;
    if let Some(existing) = runtime
        .store
        .get_weaponry_threejs_preview(&project_id, &idempotency_key)?
    {
        if action != "preview"
            || existing.request_sha256 != request_sha256
            || existing.design_id != design_id
        {
            return Err(invalid(
                "execution idempotency key conflicts with a durable preview",
            ));
        }
        let worker = runtime
            .store
            .read_weaponry_threejs_preview_worker_result_json(&existing)?;
        let receipt = runtime
            .store
            .read_weaponry_threejs_preview_receipt_json(&existing)?;
        return preview_execution_result_value(&existing, &worker, &receipt, true);
    }
    if let Some(existing) = runtime
        .store
        .get_weaponry_threejs_execution(&project_id, &idempotency_key)?
    {
        if action == "preview"
            || existing.request_sha256 != request_sha256
            || existing.design_id != design_id
            || existing.action != action
        {
            return Err(invalid(
                "execution idempotency key conflicts with another request",
            ));
        }
        let worker_result = runtime
            .store
            .read_weaponry_threejs_worker_result_json(&existing)?;
        return execution_result_value(&existing, &worker_result, true);
    }

    let program = runtime.store.read_weaponry_threejs_program_json(&design)?;
    let worker_result = invoke_fixed_worker(&action, &design, &program)?;
    if action == "preview" {
        return persist_preview(
            runtime,
            &design,
            &request_sha256,
            &idempotency_key,
            worker_result,
        );
    }
    let worker_object = worker_result
        .as_object()
        .ok_or_else(|| invalid("fixed Worker result must be an object"))?;
    let worker_result_sha256 = hash(worker_object, "canonical_sha256")?;
    let triangle_count = worker_object
        .get("triangle_count")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid("fixed Worker triangle_count is invalid"))?;
    let part_ids = worker_object
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("fixed Worker part_ids are invalid"))?
        .iter()
        .map(|part| {
            part.as_str()
                .filter(|part| is_opaque_id(part))
                .map(str::to_owned)
                .ok_or_else(|| invalid("fixed Worker part_id is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let part_count = u64::try_from(part_ids.len())
        .ok()
        .filter(|count| (2..=64).contains(count))
        .ok_or_else(|| invalid("fixed Worker part_ids are invalid"))?;
    let (glb_sha256, glb_bytes) = match worker_object.get("glb_base64") {
        Some(Value::String(encoded)) => {
            let bytes = BASE64_STANDARD
                .decode(encoded)
                .map_err(|error| invalid(format!("fixed Worker GLB base64 is invalid: {error}")))?;
            if bytes.is_empty() || bytes.len() as u64 > WEAPONRY_THREEJS_MAX_GLB_BYTES {
                return Err(invalid("fixed Worker GLB exceeds its bounded capacity"));
            }
            let supplied = worker_object
                .get("glb_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("fixed Worker glb_sha256 is invalid"))?;
            if super::sha256_hex(&bytes) != supplied {
                return Err(invalid("fixed Worker GLB hash differs from its bytes"));
            }
            validate_threejs_glb(&bytes, &part_ids)?;
            (Some(supplied.to_owned()), Some(bytes))
        }
        Some(Value::Null) if action == "preview" => (None, None),
        _ => return Err(invalid("fixed Worker GLB presence differs from action")),
    };
    let worker_bytes =
        canonical_json_bytes(&worker_result).map_err(|error| invalid(error.to_string()))?;
    let reservation = runtime.store.begin_cas_reservation();
    let worker_cas = runtime.store.put_object_reserved(
        &reservation,
        &worker_bytes,
        None,
        WEAPONRY_THREEJS_WORKER_RESULT_MIME,
        WEAPONRY_THREEJS_WORKER_RESULT_KIND,
        &super::now_string(),
    )?;
    let glb_cas = if let Some(bytes) = &glb_bytes {
        Some(runtime.store.put_object_reserved(
            &reservation,
            bytes,
            None,
            WEAPONRY_THREEJS_GLB_MIME,
            WEAPONRY_THREEJS_GLB_KIND,
            &super::now_string(),
        )?)
    } else {
        None
    };
    let execution_id = format!("three-exec-{}", &request_sha256[..40]);
    let record = WeaponryThreeJsExecutionStoreRecord {
        schema_version: WEAPONRY_THREEJS_EXECUTION_RECORD_SCHEMA.to_owned(),
        project_id,
        execution_id,
        design_id,
        operation: EXECUTE_OPERATION.to_owned(),
        action,
        program_sha256,
        program_object_sha256,
        worker_result_sha256,
        worker_result_object_sha256: worker_cas.record.sha256.clone(),
        glb_sha256,
        glb_object_sha256: glb_cas.as_ref().map(|value| value.record.sha256.clone()),
        glb_bytes: glb_bytes.as_ref().map_or(0, |bytes| bytes.len() as u64),
        triangle_count,
        part_count,
        request_sha256,
        idempotency_key,
        created_at: super::now_string(),
    };
    let commit = WeaponryThreeJsExecutionCommit {
        record,
        worker_result: worker_cas.record.clone(),
        glb: glb_cas.as_ref().map(|value| value.record.clone()),
    };
    let stored = runtime
        .store
        .record_weaponry_threejs_execution_with_replay(&commit);
    let (record, replayed) = match stored {
        Ok(value) => value,
        Err(error) => {
            let _ = runtime
                .store
                .release_cas_reservation_object(&reservation, &worker_cas, true);
            if let Some(glb) = &glb_cas {
                let _ = runtime
                    .store
                    .release_cas_reservation_object(&reservation, glb, true);
            }
            return Err(error.into());
        }
    };
    let _ = runtime
        .store
        .release_cas_reservation_object(&reservation, &worker_cas, false);
    if let Some(glb) = &glb_cas {
        let _ = runtime
            .store
            .release_cas_reservation_object(&reservation, glb, false);
    }
    execution_result_value(&record, &worker_result, replayed)
}

impl Runtime {
    pub fn weaponry_threejs_knife_design_prepare(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        prepare(self, request)
    }

    pub fn weaponry_threejs_knife_design_get(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        get(self, request)
    }

    pub fn weaponry_threejs_knife_design_execute(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        execute(self, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{
        ReferenceAuthorization, ReferenceImportRequest, ReferenceImportSource,
    };
    use std::fs;
    use uuid::Uuid;

    fn seal_input(mut value: Value) -> Value {
        value["input_sha256"] = Value::String(String::new());
        let digest = canonical_json_hash(&value);
        value["input_sha256"] = Value::String(digest);
        value
    }

    #[test]
    fn dragonfang_r7_prepare_exact_replay_reopen_get() {
        let root =
            std::env::temp_dir().join(format!("weaponry-threejs-dragonfang-r7-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create live probe root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let program: Value = serde_json::from_str(include_str!(
            "../../../../../../skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r7.json"
        ))
        .expect("checked-in Dragonfang r7 program");

        let (project_id, prepared, replayed) = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("initial Runtime");
            let project = runtime
                .create_project(
                    "Dragonfang r7 Three.js Studio live probe",
                    json!({"profile":"weaponry-knife-p0@1"}),
                )
                .expect("create project");
            let request = seal_input(json!({
                "schema_version": PREPARE_SCHEMA,
                "operation": PREPARE_OPERATION,
                "project_id": project.project_id,
                "program": program,
                "idempotency_key": "dragonfang-r7-threejs-design-prepare-001",
                "max_response_bytes": MAX_RESPONSE_BYTES,
                "runtime_write_performed": false,
                "writer_policy": WRITER_POLICY,
                "canonicalization_policy": INPUT_CANONICALIZATION,
                "input_sha256": ""
            }));
            let prepared = runtime
                .weaponry_threejs_knife_design_prepare(&request)
                .expect("prepare Dragonfang r7");
            let replayed = runtime
                .weaponry_threejs_knife_design_prepare(&request)
                .expect("exact replay Dragonfang r7");
            assert_eq!(prepared["status"], "prepared");
            assert_eq!(replayed["status"], "replayed");
            assert_eq!(prepared["program_sha256"], replayed["program_sha256"]);
            assert_eq!(
                prepared["program_object_sha256"],
                replayed["program_object_sha256"]
            );
            (project.project_id, prepared, replayed)
        };

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen Runtime");
        let get_request = seal_input(json!({
            "schema_version": GET_SCHEMA,
            "operation": GET_OPERATION,
            "project_id": project_id,
            "design_id": prepared["design_id"],
            "program_sha256": prepared["program_sha256"],
            "program_object_sha256": prepared["program_object_sha256"],
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "persistent_user_data_touched": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": INPUT_CANONICALIZATION,
            "input_sha256": ""
        }));
        let found = reopened
            .weaponry_threejs_knife_design_get(&get_request)
            .expect("get Dragonfang r7 after reopen");
        assert_eq!(found["status"], "found");
        assert_eq!(found["program"], prepared["program"]);
        assert_eq!(found["program_sha256"], replayed["program_sha256"]);
        assert_eq!(
            found["program_object_sha256"],
            replayed["program_object_sha256"]
        );
        assert_eq!(found["worker_execution_status"], "NOT_RUN_FIXED_WORKER");
        println!(
            "DRAGONFANG_R7_THREEJS_PERSISTED program_sha256={} program_object_sha256={} design_id={}",
            found["program_sha256"].as_str().expect("semantic hash"),
            found["program_object_sha256"].as_str().expect("object hash"),
            found["design_id"].as_str().expect("design id")
        );

        let execute_request = |action: &str| {
            seal_input(json!({
                "schema_version": EXECUTE_SCHEMA,
                "operation": EXECUTE_OPERATION,
                "action": action,
                "project_id": project_id,
                "design_id": found["design_id"],
                "program_sha256": found["program_sha256"],
                "program_object_sha256": found["program_object_sha256"],
                "idempotency_key": format!("dragonfang-r7-threejs-{action}-001"),
                "max_response_bytes": MAX_RESPONSE_BYTES,
                "runtime_write_performed": false,
                "writer_policy": WRITER_POLICY,
                "canonicalization_policy": INPUT_CANONICALIZATION,
                "input_sha256": ""
            }))
        };
        let build_request = execute_request("build");
        let built = reopened
            .weaponry_threejs_knife_design_execute(&build_request)
            .expect("fixed Worker build");
        let build_replay = reopened
            .weaponry_threejs_knife_design_execute(&build_request)
            .expect("fixed Worker build exact replay");
        let preview_request = execute_request("preview");
        let preview = reopened
            .weaponry_threejs_knife_design_execute(&preview_request)
            .expect("fixed Worker preview");
        let preview_replay = reopened
            .weaponry_threejs_knife_design_execute(&preview_request)
            .expect("fixed Worker preview exact replay");
        let exported = reopened
            .weaponry_threejs_knife_design_execute(&execute_request("export"))
            .expect("fixed Worker export");
        assert_eq!(built["status"], "executed");
        assert_eq!(build_replay["status"], "replayed");
        assert_eq!(build_replay["worker_invoked"], false);
        assert_eq!(preview["execution_status"], "preview-ready");
        assert_eq!(preview["renderer_invoked"], true);
        assert_eq!(preview["preview_view_count"], 8);
        assert_eq!(preview["preview_aov_count"], 48);
        assert_eq!(preview_replay["status"], "replayed");
        assert_eq!(preview_replay["worker_invoked"], false);
        assert_eq!(
            preview_replay["preview_receipt_object_sha256"],
            preview["preview_receipt_object_sha256"]
        );
        assert!(preview["glb_sha256"].is_null());
        assert_eq!(exported["execution_status"], "exported");
        assert_eq!(built["glb_sha256"], exported["glb_sha256"]);
        assert_eq!(built["triangle_count"], 4598);
        assert_eq!(built["part_ids"].as_array().map(Vec::len), Some(13));

        // WPN-THREE-COMPARE-006 live vertical slice.  The checked-in FRONT
        // crop is placed back at its frozen sheet coordinates so the Runtime
        // consumes a normal ReferenceEvidence image rather than a test-only
        // mask.  Only semantic ids 1/2 participate in the measurement.
        let front_crop = image::load_from_memory(include_bytes!(
            "../../../../../../packages/weaponry-threejs/evidence/reference-crops/dragonfang-front.png"
        ))
        .expect("checked-in Dragonfang FRONT crop")
        .to_rgba8();
        let mut reference_canvas =
            image::RgbaImage::from_pixel(1536, 1024, image::Rgba([31, 31, 31, 255]));
        image::imageops::overlay(&mut reference_canvas, &front_crop, 10, 20);
        let mut reference_png = Vec::new();
        image::DynamicImage::ImageRgba8(reference_canvas)
            .write_to(
                &mut std::io::Cursor::new(&mut reference_png),
                image::ImageFormat::Png,
            )
            .expect("encode authorized reference fixture");
        let reference = reopened
            .import_reference(&ReferenceImportRequest {
                project_id: project_id.clone(),
                source: ReferenceImportSource::InlineContent {
                    mime: "image/png".to_owned(),
                    content_base64: BASE64_STANDARD.encode(reference_png),
                },
                authorization: ReferenceAuthorization {
                    user_authorized: true,
                    declaration: "User supplied Dragonfang reference for local comparison"
                        .to_owned(),
                },
                expected_sha256: None,
            })
            .expect("import authorized Dragonfang reference")
            .reference;
        let semantic_pass = preview["preview_views"]
            .as_array()
            .and_then(|views| {
                views
                    .iter()
                    .find(|view| view["view_id"].as_str() == Some("FRONT"))
            })
            .and_then(|view| view["passes"].as_array())
            .and_then(|passes| {
                passes
                    .iter()
                    .find(|pass| pass["aov_id"].as_str() == Some("semantic-id"))
            })
            .expect("FRONT semantic-id pass");
        let comparison_request = seal_input(json!({
            "schema_version":"WeaponryThreeJsKnifeComparisonPrepareRequest@1",
            "operation":"weaponry_threejs_knife_comparison_prepare",
            "project_id":project_id,
            "preview_execution_id":preview["execution_id"],
            "preview_program_sha256":preview["program_sha256"],
            "preview_program_object_sha256":preview["program_object_sha256"],
            "preview_worker_cohort_sha256":preview["preview_worker_cohort_sha256"],
            "preview_receipt_sha256":preview["preview_receipt_sha256"],
            "preview_receipt_object_sha256":preview["preview_receipt_object_sha256"],
            "preview_aov_sha256":semantic_pass["sha256"],
            "preview_aov_object_sha256":semantic_pass["object_sha256"],
            "reference_id":reference.reference_id,
            "reference_object_sha256":reference.object_sha256,
            "reference_evidence_sha256":reference.canonical_sha256,
            "view_id":"FRONT",
            "reference_crop":{"x":10,"y":20,"width":650,"height":200},
            "semantic_part_ids":{"blade-body":1,"cutting-edge":2},
            "editable_part_ids":["blade-body","cutting-edge"],
            "frozen_part_ids":[
                "fastener-grip-a","fastener-grip-b","fastener-grip-c","fastener-grip-d",
                "gem-guard-eye","gem-pommel","grip","guard","pommel",
                "relief-dragon-belly","relief-dragon-spine"
            ],
            "idempotency_key":"dragonfang-r7-threejs-comparison-001",
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "runtime_write_performed":false,
            "writer_policy":WRITER_POLICY,
            "canonicalization_policy":INPUT_CANONICALIZATION,
            "input_sha256":""
        }));
        let comparison = reopened
            .weaponry_threejs_knife_comparison_prepare(&comparison_request)
            .expect("persist Dragonfang FRONT comparison");
        let comparison_replay = reopened
            .weaponry_threejs_knife_comparison_prepare(&comparison_request)
            .expect("exact replay Dragonfang FRONT comparison");
        assert_eq!(comparison["status"], "stored");
        assert_eq!(comparison_replay["status"], "replayed");
        assert_eq!(comparison["comparison_status"], "MEASURED_NOT_APPROVED");
        assert_eq!(comparison["visual_status"], "NOT_RUN");
        assert_eq!(comparison["parent_retained"], true);
        let comparison_get_request = seal_input(json!({
            "schema_version":"WeaponryThreeJsKnifeComparisonGetRequest@1",
            "operation":"weaponry_threejs_knife_comparison_get",
            "project_id":project_id,
            "comparison_id":comparison["comparison_id"],
            "comparison_sha256":comparison["comparison_sha256"],
            "comparison_object_sha256":comparison["comparison_object_sha256"],
            "preview_execution_id":preview["execution_id"],
            "preview_program_sha256":preview["program_sha256"],
            "preview_program_object_sha256":preview["program_object_sha256"],
            "preview_worker_cohort_sha256":preview["preview_worker_cohort_sha256"],
            "preview_receipt_sha256":preview["preview_receipt_sha256"],
            "preview_receipt_object_sha256":preview["preview_receipt_object_sha256"],
            "preview_aov_sha256":semantic_pass["sha256"],
            "preview_aov_object_sha256":semantic_pass["object_sha256"],
            "reference_id":reference.reference_id,
            "reference_object_sha256":reference.object_sha256,
            "reference_evidence_sha256":reference.canonical_sha256,
            "view_id":"FRONT",
            "reference_crop":{"x":10,"y":20,"width":650,"height":200},
            "semantic_part_ids":{"blade-body":1,"cutting-edge":2},
            "editable_part_ids":["blade-body","cutting-edge"],
            "frozen_part_ids":[
                "fastener-grip-a","fastener-grip-b","fastener-grip-c","fastener-grip-d",
                "gem-guard-eye","gem-pommel","grip","guard","pommel",
                "relief-dragon-belly","relief-dragon-spine"
            ],
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "runtime_write_performed":false,
            "persistent_user_data_touched":false,
            "writer_policy":WRITER_POLICY,
            "canonicalization_policy":INPUT_CANONICALIZATION,
            "input_sha256":""
        }));
        drop(reopened);
        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen after comparison");
        let comparison_found = reopened
            .weaponry_threejs_knife_comparison_get(&comparison_get_request)
            .expect("get Dragonfang comparison after reopen");
        assert_eq!(comparison_found["status"], "found");
        assert_eq!(comparison_found["metrics"], comparison["metrics"]);
        println!(
            "DRAGONFANG_R7_THREEJS_COMPARISON comparison_id={} comparison_sha256={} comparison_object_sha256={} semantic_aov_sha256={} metrics={} comparison_status={} visual_status={}",
            comparison["comparison_id"].as_str().expect("comparison id"),
            comparison["comparison_sha256"].as_str().expect("comparison semantic hash"),
            comparison["comparison_object_sha256"].as_str().expect("comparison object hash"),
            semantic_pass["sha256"].as_str().expect("semantic AOV hash"),
            comparison["metrics"],
            comparison["comparison_status"].as_str().expect("comparison status"),
            comparison["visual_status"].as_str().expect("visual status"),
        );
        println!(
            "DRAGONFANG_R7_THREEJS_FIXED_WORKER glb_sha256={} glb_object_sha256={} glb_bytes={} triangle_count={} preview_views={}",
            built["glb_sha256"].as_str().expect("GLB semantic hash"),
            built["glb_object_sha256"].as_str().expect("GLB CAS hash"),
            built["glb_bytes"].as_u64().expect("GLB bytes"),
            built["triangle_count"].as_u64().expect("triangle count"),
            preview["preview_manifest"]["view_ids"].as_array().map(Vec::len).expect("preview views")
        );
        println!(
            "DRAGONFANG_R7_THREEJS_PREVIEW_RECEIPT receipt_sha256={} receipt_object_sha256={} worker_result_sha256={} worker_result_object_sha256={} runtime_sha256={} dependency_lock_sha256={} cohort_sha256={} views={} aovs={}",
            preview["preview_receipt_sha256"].as_str().expect("receipt semantic hash"),
            preview["preview_receipt_object_sha256"].as_str().expect("receipt object hash"),
            preview["worker_result_sha256"].as_str().expect("worker semantic hash"),
            preview["worker_result_object_sha256"].as_str().expect("worker object hash"),
            preview["preview_runtime_sha256"].as_str().expect("runtime hash"),
            preview["preview_dependency_lock_sha256"].as_str().expect("dependency hash"),
            preview["preview_worker_cohort_sha256"].as_str().expect("cohort hash"),
            preview["preview_view_count"].as_u64().expect("view count"),
            preview["preview_aov_count"].as_u64().expect("AOV count")
        );
        drop(reopened);
        fs::remove_dir_all(&root).expect("remove live probe root");
    }
}
