//! Durable Store/CAS boundary for the fixed Blender knife Worker.
//!
//! The Blender process is an isolated compute provider.  It never owns the
//! Runtime database or CAS.  Runtime adopts the provider's temporary bytes and
//! supplies this repository with the complete, already hashed bundle.  This
//! module is deliberately additive: it owns one immutable execution index and
//! an explicit root set, while the existing candidate/version/stage records
//! remain untouched.
//!
//! The index is intentionally more verbose than the Worker result.  A Worker
//! result can be a useful observation, but it is not enough to prove that the
//! source object, the fixed Worker identity, every High/Low/map byte, and the
//! package/release identity are still the same after a Runtime restart.  The
//! Store record repeats those identities so a later reader can fail closed
//! without trusting a Blender session, a path, or a mutable manifest.

use forgecad_contracts::{is_opaque_id, is_sha256, CasObjectRecord};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

use super::{mark_reachable_in_transaction, Store, StoreError};

pub const WEAPONRY_BLENDER_EXECUTION_RECORD_SCHEMA: &str = "WeaponryBlenderExecutionStoreRecord@1";
pub const WEAPONRY_BLENDER_EXECUTION_RECORD_SCHEMA_VERSION: &str =
    WEAPONRY_BLENDER_EXECUTION_RECORD_SCHEMA;
pub const WEAPONRY_BLENDER_EXECUTION_SCHEMA_VERSION: &str = "WeaponryBlenderKnifeExecution@1";
pub const WEAPONRY_BLENDER_EXECUTION_OPERATION: &str = "knife_high_low_uv_bake@1";
pub const WEAPONRY_BLENDER_EXECUTION_STATUS: &str =
    "runtime-owned-store-blender-worker-execution@1";
pub const WEAPONRY_BLENDER_EXECUTION_MATERIALIZATION_STATUS: &str =
    WEAPONRY_BLENDER_EXECUTION_STATUS;
pub const WEAPONRY_BLENDER_EXECUTION_JSON_MIME: &str = "application/json";
pub const WEAPONRY_BLENDER_EXECUTION_GLB_MIME: &str = "model/gltf-binary";
pub const WEAPONRY_BLENDER_EXECUTION_PNG_MIME: &str = "image/png";
pub const WEAPONRY_BLENDER_WORKER_RESULT_KIND: &str = "weaponry-blender-worker-result@1";
pub const WEAPONRY_BLENDER_WORKER_IDENTITY_KIND: &str = "weaponry-blender-worker-identity@1";
pub const WEAPONRY_BLENDER_RECEIPT_KIND: &str = "weaponry-blender-runtime-adoption-receipt@1";
pub const WEAPONRY_BLENDER_WORKER_MANIFEST_KIND: &str = "weaponry-blender-worker_manifest@1";
pub const WEAPONRY_BLENDER_HIGH_GLB_KIND: &str = "weaponry-blender-high_glb@1";
pub const WEAPONRY_BLENDER_LOW_GLB_KIND: &str = "weaponry-blender-low_glb@1";
pub const WEAPONRY_BLENDER_NORMAL_MAP_KIND: &str = "weaponry-blender-normal_map@1";
pub const WEAPONRY_BLENDER_AO_MAP_KIND: &str = "weaponry-blender-ao_map@1";
pub const WEAPONRY_BLENDER_WORKER_RESULT_SCHEMA: &str = "WeaponryBlenderKnifeWorkerResult@1";
pub const WEAPONRY_BLENDER_WORKER_IDENTITY_SCHEMA: &str = "WeaponryBlenderKnifeWorkerIdentity@1";
pub const WEAPONRY_BLENDER_RECEIPT_SCHEMA: &str = "WeaponryBlenderKnifeRuntimeAdoptionReceipt@1";
pub const WEAPONRY_BLENDER_WORKER_MANIFEST_SCHEMA: &str = "WeaponryBlenderKnifeWorkerManifest@1";
pub const WEAPONRY_BLENDER_MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
pub const WEAPONRY_BLENDER_MAX_GLB_BYTES: u64 = 256 * 1024 * 1024;
pub const WEAPONRY_BLENDER_MAX_MAP_BYTES: u64 = 32 * 1024 * 1024;
pub const WEAPONRY_BLENDER_MAX_ARTIFACTS: usize = 512;
pub const WEAPONRY_BLENDER_MAX_ROOTS: usize = WEAPONRY_BLENDER_MAX_ARTIFACTS + 16;
pub const WEAPONRY_BLENDER_MAX_IDEMPOTENCY_BYTES: usize = 128;

const TABLE: &str = "weaponry_blender_execution_records";
const ROOT_TABLE: &str = "weaponry_blender_execution_roots";
const CAS_SCHEMA: &str = "CasObject@1";
const WORKER_ID: &str = "weaponry-blender-knife-worker@1";
const WORKER_VERSION: &str = "0.1.0";
const BLENDER_VERSION: &str = "5.2.1";
const BLENDER_REVISION: &str = "9e2066aef7ef";
const WORKER_POLICY: &str = "fixed-built-in-bevel-weighted-normal-decimate-smart-uv-cycles-bake@1";
const WORKER_PROTOCOL: &str = "weaponry-fixed-worker-stdio-json@1";

/// One output byte adopted from the fixed Worker.
///
/// `semantic_sha256` is the hash declared by the Worker output record.  The
/// current fixed provider emits raw bytes for GLB/PNG outputs, so it equals
/// `object_sha256`; the two fields remain separate to prevent a future
/// provider from silently collapsing semantic and CAS identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryBlenderArtifactRef {
    pub relative_path: String,
    pub kind: String,
    pub mime: String,
    pub semantic_sha256: String,
    pub object_sha256: String,
    pub byte_size: u64,
}

/// Package and release identity is metadata, not a release authorization.
/// The fixed development package is expected to carry `release_eligible=false`.
/// Optional CAS hashes are allowed for a later package layer, but this Store
/// does not infer release eligibility from their presence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryBlenderPackageIdentity {
    pub packaged: bool,
    pub package_manifest_sha256: Option<String>,
    pub resource_tree_sha256: Option<String>,
    pub blender_bundle_tree_sha256: Option<String>,
    pub release_eligibility_sha256: Option<String>,
    pub package_status: String,
    pub release_eligible: bool,
}

/// Immutable Store-local index for one fixed Blender Worker execution.
///
/// No field is a filesystem path or a Blender object handle.  The only
/// durable references are opaque IDs and content hashes.  The normal/AO
/// vectors are ordered by `relative_path` and are also represented by the
/// explicit root table below, so SQLite reachability does not depend on JSON
/// substring matching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponryBlenderExecutionStoreRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub execution_id: String,
    pub request_id: String,
    pub operation: String,
    pub source_object_sha256: String,
    pub source_object_size_bytes: u64,
    pub worker_id: String,
    pub worker_version: String,
    pub blender_version: String,
    pub blender_revision: String,
    pub worker_entrypoint_sha256: String,
    pub worker_bundle_sha256: String,
    pub dependency_lock_sha256: String,
    pub worker_identity_sha256: String,
    pub worker_identity_object_sha256: String,
    pub worker_result_sha256: String,
    pub worker_result_object_sha256: String,
    pub receipt_sha256: String,
    pub receipt_object_sha256: String,
    pub worker_manifest_sha256: String,
    pub worker_manifest_object_sha256: String,
    pub worker_manifest_relative_path: String,
    pub high_glb_sha256: String,
    pub high_glb_object_sha256: String,
    pub high_glb_bytes: u64,
    pub high_glb_relative_path: String,
    pub low_glb_sha256: String,
    pub low_glb_object_sha256: String,
    pub low_glb_bytes: u64,
    pub low_glb_relative_path: String,
    pub normal_maps: Vec<WeaponryBlenderArtifactRef>,
    pub ao_maps: Vec<WeaponryBlenderArtifactRef>,
    pub normal_map_set_sha256: String,
    pub ao_map_set_sha256: String,
    pub all_artifact_set_sha256: String,
    pub package_identity: WeaponryBlenderPackageIdentity,
    pub materialization_status: String,
    pub quality_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub commercial_status: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub request_sha256: String,
    pub idempotency_key: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// CAS objects must be staged and registered by Runtime before calling the
/// Store.  This bundle contains exactly the bytes referenced by the record.
/// Package/release files are optional because the current development package
/// keeps those identity hashes as metadata rather than product CAS roots.
#[derive(Debug, Clone)]
pub struct WeaponryBlenderExecutionCasBundle {
    pub source: CasObjectRecord,
    pub worker_identity: CasObjectRecord,
    pub worker_result: CasObjectRecord,
    pub receipt: CasObjectRecord,
    pub worker_manifest: CasObjectRecord,
    pub high_glb: CasObjectRecord,
    pub low_glb: CasObjectRecord,
    pub normal_maps: Vec<CasObjectRecord>,
    pub ao_maps: Vec<CasObjectRecord>,
    pub package_manifest: Option<CasObjectRecord>,
    pub release_eligibility: Option<CasObjectRecord>,
}

#[derive(Debug, Clone)]
pub struct WeaponryBlenderExecutionCommit {
    pub record: WeaponryBlenderExecutionStoreRecord,
    pub cas: WeaponryBlenderExecutionCasBundle,
}

// Names used by the Worker-facing runtime slice are retained as aliases so a
// caller does not need a second persistence type merely because it calls the
// operation a "Worker execution" rather than a "Blender execution".
pub type WeaponryBlenderWorkerExecutionStoreRecord = WeaponryBlenderExecutionStoreRecord;
pub type WeaponryBlenderWorkerExecutionCasBundle = WeaponryBlenderExecutionCasBundle;
pub type WeaponryBlenderWorkerExecutionCommit = WeaponryBlenderExecutionCommit;
pub type WeaponryBlenderWorkerArtifactRef = WeaponryBlenderArtifactRef;
pub type WeaponryBlenderWorkerPackageIdentity = WeaponryBlenderPackageIdentity;

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn is_valid_hash(value: &str) -> bool {
    is_sha256(value)
}

fn is_safe_kind(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
}

fn record_value(record: &WeaponryBlenderExecutionStoreRecord) -> Result<Value, StoreError> {
    serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))
}

/// Return the canonical hash of a Store record with its self-hash blanked.
pub fn weaponry_blender_execution_record_canonical_sha256(
    record: &WeaponryBlenderExecutionStoreRecord,
) -> Result<String, StoreError> {
    let mut value = record_value(record)?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

/// Hash an ordered artifact vector.  Runtime should sort map references by
/// `relative_path` before populating the record; the validator enforces that
/// ordering and therefore the hash is replay-stable.
pub fn weaponry_blender_artifact_set_sha256(
    artifacts: &[WeaponryBlenderArtifactRef],
) -> Result<String, StoreError> {
    let value = serde_json::to_value(artifacts)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    Ok(canonical_json_hash(&value))
}

fn output_artifacts(
    record: &WeaponryBlenderExecutionStoreRecord,
) -> Vec<WeaponryBlenderArtifactRef> {
    let mut outputs = Vec::with_capacity(2 + record.normal_maps.len() + record.ao_maps.len() + 1);
    outputs.push(WeaponryBlenderArtifactRef {
        relative_path: record.high_glb_relative_path.clone(),
        kind: "high_glb".to_owned(),
        mime: WEAPONRY_BLENDER_EXECUTION_GLB_MIME.to_owned(),
        semantic_sha256: record.high_glb_sha256.clone(),
        object_sha256: record.high_glb_object_sha256.clone(),
        byte_size: record.high_glb_bytes,
    });
    outputs.push(WeaponryBlenderArtifactRef {
        relative_path: record.low_glb_relative_path.clone(),
        kind: "low_glb".to_owned(),
        mime: WEAPONRY_BLENDER_EXECUTION_GLB_MIME.to_owned(),
        semantic_sha256: record.low_glb_sha256.clone(),
        object_sha256: record.low_glb_object_sha256.clone(),
        byte_size: record.low_glb_bytes,
    });
    outputs.extend(record.normal_maps.clone());
    outputs.extend(record.ao_maps.clone());
    outputs.push(WeaponryBlenderArtifactRef {
        relative_path: record.worker_manifest_relative_path.clone(),
        kind: "worker_manifest".to_owned(),
        mime: WEAPONRY_BLENDER_EXECUTION_JSON_MIME.to_owned(),
        semantic_sha256: record.worker_manifest_sha256.clone(),
        object_sha256: record.worker_manifest_object_sha256.clone(),
        byte_size: 0,
    });
    outputs
}

fn validate_id(value: &str, label: &str) -> Result<(), StoreError> {
    if !is_opaque_id(value) || value.len() > 256 {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_ID_INVALID",
            format!("{label} is not a bounded opaque id"),
        ));
    }
    Ok(())
}

fn validate_optional_hash(value: &Option<String>, label: &str) -> Result<(), StoreError> {
    if value.as_deref().is_some_and(|value| !is_valid_hash(value)) {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_PACKAGE_IDENTITY_INVALID",
            format!("{label} is not a SHA-256 hash"),
        ));
    }
    Ok(())
}

fn validate_package_identity(identity: &WeaponryBlenderPackageIdentity) -> Result<(), StoreError> {
    validate_optional_hash(&identity.package_manifest_sha256, "package_manifest_sha256")?;
    validate_optional_hash(&identity.resource_tree_sha256, "resource_tree_sha256")?;
    validate_optional_hash(
        &identity.blender_bundle_tree_sha256,
        "blender_bundle_tree_sha256",
    )?;
    validate_optional_hash(
        &identity.release_eligibility_sha256,
        "release_eligibility_sha256",
    )?;
    if identity.package_status.is_empty()
        || identity.package_status.len() > 128
        || identity.package_status.contains('/')
        || identity.package_status.contains('\\')
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_PACKAGE_IDENTITY_INVALID",
            "package_status is empty or contains a path separator",
        ));
    }
    if identity.release_eligible && identity.release_eligibility_sha256.is_none() {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_PACKAGE_IDENTITY_INVALID",
            "release-eligible execution must carry a release eligibility identity",
        ));
    }
    Ok(())
}

fn validate_artifact_ref(
    artifact: &WeaponryBlenderArtifactRef,
    expected_kind: &str,
    expected_mime: &str,
    max_bytes: u64,
) -> Result<(), StoreError> {
    if artifact.relative_path.is_empty()
        || artifact.relative_path.len() > 256
        || artifact.relative_path.starts_with('/')
        || artifact.relative_path.contains("..")
        || artifact.relative_path.contains('\\')
        || artifact.kind != expected_kind
        || artifact.mime != expected_mime
        || !is_valid_hash(&artifact.semantic_sha256)
        || !is_valid_hash(&artifact.object_sha256)
        || artifact.byte_size == 0
        || artifact.byte_size > max_bytes
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_ARTIFACT_REF_INVALID",
            "fixed Worker artifact reference is outside its closed kind/path/size policy",
        ));
    }
    Ok(())
}

fn validate_sorted_unique_paths(
    artifacts: &[WeaponryBlenderArtifactRef],
) -> Result<(), StoreError> {
    let mut previous = None;
    for artifact in artifacts {
        if previous.is_some_and(|path: &str| path >= artifact.relative_path.as_str()) {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_ARTIFACT_ORDER_INVALID",
                "normal/AO artifacts must be strictly ordered by relative_path",
            ));
        }
        previous = Some(artifact.relative_path.as_str());
    }
    Ok(())
}

fn validate_record_shape(record: &WeaponryBlenderExecutionStoreRecord) -> Result<(), StoreError> {
    let ids = [
        ("project_id", record.project_id.as_str()),
        ("candidate_id", record.candidate_id.as_str()),
        ("execution_id", record.execution_id.as_str()),
        ("request_id", record.request_id.as_str()),
        ("idempotency_key", record.idempotency_key.as_str()),
    ];
    for (label, value) in ids {
        validate_id(value, label)?;
    }
    let hashes = [
        ("source_object_sha256", record.source_object_sha256.as_str()),
        (
            "worker_entrypoint_sha256",
            record.worker_entrypoint_sha256.as_str(),
        ),
        ("worker_bundle_sha256", record.worker_bundle_sha256.as_str()),
        (
            "dependency_lock_sha256",
            record.dependency_lock_sha256.as_str(),
        ),
        (
            "worker_identity_sha256",
            record.worker_identity_sha256.as_str(),
        ),
        (
            "worker_identity_object_sha256",
            record.worker_identity_object_sha256.as_str(),
        ),
        ("worker_result_sha256", record.worker_result_sha256.as_str()),
        (
            "worker_result_object_sha256",
            record.worker_result_object_sha256.as_str(),
        ),
        ("receipt_sha256", record.receipt_sha256.as_str()),
        (
            "receipt_object_sha256",
            record.receipt_object_sha256.as_str(),
        ),
        (
            "worker_manifest_sha256",
            record.worker_manifest_sha256.as_str(),
        ),
        (
            "worker_manifest_object_sha256",
            record.worker_manifest_object_sha256.as_str(),
        ),
        ("high_glb_sha256", record.high_glb_sha256.as_str()),
        (
            "high_glb_object_sha256",
            record.high_glb_object_sha256.as_str(),
        ),
        ("low_glb_sha256", record.low_glb_sha256.as_str()),
        (
            "low_glb_object_sha256",
            record.low_glb_object_sha256.as_str(),
        ),
        (
            "normal_map_set_sha256",
            record.normal_map_set_sha256.as_str(),
        ),
        ("ao_map_set_sha256", record.ao_map_set_sha256.as_str()),
        (
            "all_artifact_set_sha256",
            record.all_artifact_set_sha256.as_str(),
        ),
        ("request_sha256", record.request_sha256.as_str()),
        ("canonical_sha256", record.canonical_sha256.as_str()),
    ];
    if hashes.iter().any(|(_, value)| !is_valid_hash(value)) {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_HASH_INVALID",
            "fixed Blender execution contains a non-SHA-256 identity",
        ));
    }
    if record.schema_version != WEAPONRY_BLENDER_EXECUTION_RECORD_SCHEMA
        || record.operation != WEAPONRY_BLENDER_EXECUTION_OPERATION
        || record.idempotency_key.len() > WEAPONRY_BLENDER_MAX_IDEMPOTENCY_BYTES
        || record.source_object_size_bytes == 0
        || record.source_object_size_bytes > WEAPONRY_BLENDER_MAX_GLB_BYTES
        || record.high_glb_bytes == 0
        || record.high_glb_bytes > WEAPONRY_BLENDER_MAX_GLB_BYTES
        || record.low_glb_bytes == 0
        || record.low_glb_bytes > WEAPONRY_BLENDER_MAX_GLB_BYTES
        || record.high_glb_sha256 != record.high_glb_object_sha256
        || record.low_glb_sha256 != record.low_glb_object_sha256
        || record.worker_manifest_sha256 != record.worker_manifest_object_sha256
        || record.worker_id != WORKER_ID
        || record.worker_version != WORKER_VERSION
        || record.blender_version != BLENDER_VERSION
        || record.blender_revision != BLENDER_REVISION
        || record.materialization_status != WEAPONRY_BLENDER_EXECUTION_STATUS
        || record.quality_status != "structural_only"
        || record.visual_status != "NOT_RUN"
        || record.human_status != "NOT_RUN"
        || record.engine_status != "NOT_RUN"
        || record.commercial_status != "NOT_RUN"
        || !record.runtime_write_performed
        || !record.persistent_user_data_touched
        || record.production_stage_advanced
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
        || record.created_at.is_empty()
        || record.created_at.len() > 128
        || record.created_at.contains('/')
        || record.created_at.contains('\\')
        || record.normal_maps.is_empty()
        || record.normal_maps.len() != record.ao_maps.len()
        || record.normal_maps.len() > WEAPONRY_BLENDER_MAX_ARTIFACTS
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_RECORD_INVALID",
            "fixed Blender execution identity, status, count or policy is invalid",
        ));
    }
    validate_package_identity(&record.package_identity)?;
    for (label, path) in [
        (
            "high_glb_relative_path",
            record.high_glb_relative_path.as_str(),
        ),
        (
            "low_glb_relative_path",
            record.low_glb_relative_path.as_str(),
        ),
        (
            "worker_manifest_relative_path",
            record.worker_manifest_relative_path.as_str(),
        ),
    ] {
        if path.is_empty()
            || path.len() > 256
            || path.starts_with('/')
            || path.contains("..")
            || path.contains('\\')
        {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_ARTIFACT_PATH_INVALID",
                format!("{label} is outside the closed relative-path policy"),
            ));
        }
    }
    if record.high_glb_relative_path == record.low_glb_relative_path
        || record.high_glb_relative_path == record.worker_manifest_relative_path
        || record.low_glb_relative_path == record.worker_manifest_relative_path
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_ARTIFACT_PATH_INVALID",
            "primary Worker artifact paths must be distinct",
        ));
    }
    validate_sorted_unique_paths(&record.normal_maps)?;
    validate_sorted_unique_paths(&record.ao_maps)?;
    for artifact in &record.normal_maps {
        validate_artifact_ref(
            artifact,
            "normal_map",
            WEAPONRY_BLENDER_EXECUTION_PNG_MIME,
            WEAPONRY_BLENDER_MAX_MAP_BYTES,
        )?;
    }
    for artifact in &record.ao_maps {
        validate_artifact_ref(
            artifact,
            "ao_map",
            WEAPONRY_BLENDER_EXECUTION_PNG_MIME,
            WEAPONRY_BLENDER_MAX_MAP_BYTES,
        )?;
    }
    if weaponry_blender_artifact_set_sha256(&record.normal_maps)? != record.normal_map_set_sha256
        || weaponry_blender_artifact_set_sha256(&record.ao_maps)? != record.ao_map_set_sha256
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_ARTIFACT_SET_MISMATCH",
            "normal/AO artifact-set hash differs from the ordered references",
        ));
    }
    let all = record
        .normal_maps
        .iter()
        .chain(record.ao_maps.iter())
        .cloned()
        .collect::<Vec<_>>();
    // The all-artifact set includes the map collection plus explicit High,
    // Low and manifest refs in a fixed order.  Keep this preimage independent
    // from Worker output ordering.
    let mut all_with_primary = Vec::with_capacity(all.len() + 3);
    all_with_primary.push(WeaponryBlenderArtifactRef {
        relative_path: record.high_glb_relative_path.clone(),
        kind: "high_glb".to_owned(),
        mime: WEAPONRY_BLENDER_EXECUTION_GLB_MIME.to_owned(),
        semantic_sha256: record.high_glb_sha256.clone(),
        object_sha256: record.high_glb_object_sha256.clone(),
        byte_size: record.high_glb_bytes,
    });
    all_with_primary.push(WeaponryBlenderArtifactRef {
        relative_path: record.low_glb_relative_path.clone(),
        kind: "low_glb".to_owned(),
        mime: WEAPONRY_BLENDER_EXECUTION_GLB_MIME.to_owned(),
        semantic_sha256: record.low_glb_sha256.clone(),
        object_sha256: record.low_glb_object_sha256.clone(),
        byte_size: record.low_glb_bytes,
    });
    all_with_primary.extend(all);
    all_with_primary.push(WeaponryBlenderArtifactRef {
        relative_path: record.worker_manifest_relative_path.clone(),
        kind: "worker_manifest".to_owned(),
        mime: WEAPONRY_BLENDER_EXECUTION_JSON_MIME.to_owned(),
        semantic_sha256: record.worker_manifest_sha256.clone(),
        object_sha256: record.worker_manifest_object_sha256.clone(),
        // The manifest's byte size is checked from CAS, so this marker is not
        // part of the caller-facing record.  Set zero only for this hash
        // preimage; no zero-sized CAS object is ever accepted below.
        byte_size: 0,
    });
    if weaponry_blender_artifact_set_sha256(&all_with_primary)? != record.all_artifact_set_sha256 {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_ARTIFACT_SET_MISMATCH",
            "all artifact-set hash differs from the ordered output collection",
        ));
    }
    if weaponry_blender_execution_record_canonical_sha256(record)? != record.canonical_sha256 {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_CANONICAL_MISMATCH",
            "fixed Blender execution record canonical hash differs",
        ));
    }
    Ok(())
}

fn canonical_preimage_hash(value: &Value, field: &str) -> Result<String, StoreError> {
    let mut preimage = value.clone();
    let object = preimage.as_object_mut().ok_or_else(|| {
        contract(
            "WEAPONRY_BLENDER_EXECUTION_JSON_INVALID",
            format!("{field} must be a JSON object"),
        )
    })?;
    object.insert(field.to_owned(), Value::String(String::new()));
    Ok(canonical_json_hash(&preimage))
}

fn require_exact_fields(
    object: &Map<String, Value>,
    expected: &[&str],
    label: &str,
) -> Result<(), StoreError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_JSON_FIELDS_INVALID",
            format!("{label} fields are not closed"),
        ));
    }
    Ok(())
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<&'a str, StoreError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_JSON_FIELD_INVALID",
                format!("{label}.{field} is missing"),
            )
        })
}

fn validate_glb_bytes(bytes: &[u8], expected_size: u64, role: &str) -> Result<(), StoreError> {
    if bytes.len() as u64 != expected_size
        || bytes.len() < 12
        || &bytes[0..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().expect("GLB version bytes")) != 2
        || u64::from(u32::from_le_bytes(
            bytes[8..12].try_into().expect("GLB length bytes"),
        )) != bytes.len() as u64
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_GLB_INVALID",
            format!("{role} is not an exact bounded GLB v2 object"),
        ));
    }
    Ok(())
}

fn validate_png_bytes(bytes: &[u8], role: &str) -> Result<(), StoreError> {
    if bytes.len() < 8 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_MAP_INVALID",
            format!("{role} is not a PNG object"),
        ));
    }
    Ok(())
}

fn parse_json_object(bytes: &[u8], max_bytes: u64, role: &str) -> Result<Value, StoreError> {
    if bytes.is_empty() || bytes.len() as u64 > max_bytes {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_JSON_BYTES_INVALID",
            format!("{role} is empty or exceeds its bounded JSON capacity"),
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        contract(
            "WEAPONRY_BLENDER_EXECUTION_JSON_INVALID",
            format!("{role} is not valid JSON: {error}"),
        )
    })?;
    if !value.is_object() {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_JSON_INVALID",
            format!("{role} must be a JSON object"),
        ));
    }
    let canonical =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if canonical != bytes {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_JSON_NOT_CANONICAL",
            format!("{role} must use canonical JSON encoding"),
        ));
    }
    Ok(value)
}

fn validate_worker_identity(
    record: &WeaponryBlenderExecutionStoreRecord,
    bytes: &[u8],
) -> Result<Value, StoreError> {
    let value = parse_json_object(bytes, WEAPONRY_BLENDER_MAX_JSON_BYTES, "Worker identity")?;
    let object = value.as_object().expect("object checked");
    require_exact_fields(
        object,
        &[
            "schema_version",
            "protocol",
            "worker_id",
            "blender_version",
            "blender_revision",
            "worker_bundle_sha256",
            "dependency_lock_sha256",
            "input_sha256",
            "stdout_sha256",
            "outputs",
            "runtime_write_performed",
            "persistent_user_data_touched",
            "canonical_sha256",
        ],
        "Worker identity",
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(WEAPONRY_BLENDER_WORKER_IDENTITY_SCHEMA)
        || object.get("protocol").and_then(Value::as_str) != Some(WORKER_PROTOCOL)
        || object.get("worker_id").and_then(Value::as_str) != Some(record.worker_id.as_str())
        || object.get("blender_version").and_then(Value::as_str)
            != Some(record.blender_version.as_str())
        || object.get("blender_revision").and_then(Value::as_str)
            != Some(record.blender_revision.as_str())
        || object.get("worker_bundle_sha256").and_then(Value::as_str)
            != Some(record.worker_bundle_sha256.as_str())
        || object.get("dependency_lock_sha256").and_then(Value::as_str)
            != Some(record.dependency_lock_sha256.as_str())
        || object.get("input_sha256").and_then(Value::as_str)
            != Some(record.source_object_sha256.as_str())
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object
            .get("persistent_user_data_touched")
            .and_then(Value::as_bool)
            != Some(false)
        || object
            .get("stdout_sha256")
            .and_then(Value::as_str)
            .is_none_or(|value| !is_valid_hash(value))
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_IDENTITY_BINDING_MISMATCH",
            "Worker identity differs from the fixed execution record",
        ));
    }
    let supplied = string_field(object, "canonical_sha256", "Worker identity")?;
    if !is_valid_hash(supplied) || canonical_preimage_hash(&value, "canonical_sha256")? != supplied
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_IDENTITY_CANONICAL_MISMATCH",
            "Worker identity canonical hash differs",
        ));
    }
    if sha256_hex(bytes) != record.worker_identity_object_sha256
        || supplied != record.worker_identity_sha256
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_IDENTITY_HASH_MISMATCH",
            "Worker identity semantic or object hash differs",
        ));
    }
    Ok(value)
}

fn output_record_map(value: &Value, role: &str) -> Result<WeaponryBlenderArtifactRef, StoreError> {
    let object = value.as_object().ok_or_else(|| {
        contract(
            "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
            format!("{role} output is not an object"),
        )
    })?;
    require_exact_fields(
        object,
        &[
            "kind",
            "relative_path",
            "mime",
            "byte_size",
            "sha256",
            "cas_owner",
            "durability",
        ],
        role,
    )?;
    let kind = string_field(object, "kind", role)?.to_owned();
    let mime = string_field(object, "mime", role)?.to_owned();
    let relative_path = string_field(object, "relative_path", role)?.to_owned();
    let semantic_sha256 = string_field(object, "sha256", role)?.to_owned();
    let byte_size = object
        .get("byte_size")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
                format!("{role}.byte_size is not an integer"),
            )
        })?;
    let cas_owner = string_field(object, "cas_owner", role)?;
    let durability = string_field(object, "durability", role)?;
    if !is_valid_hash(&semantic_sha256)
        || !is_opaque_id(cas_owner)
        || cas_owner != "runtime"
        || !is_opaque_id(durability)
        || durability != "pending_runtime_adoption"
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
            format!("{role} output ownership/durability/hash is invalid"),
        ));
    }
    Ok(WeaponryBlenderArtifactRef {
        relative_path,
        kind,
        mime,
        semantic_sha256,
        object_sha256: String::new(),
        byte_size,
    })
}

fn validate_worker_identity_outputs(
    identity: &Value,
    result_refs: &[WeaponryBlenderArtifactRef],
) -> Result<(), StoreError> {
    let outputs = identity
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_IDENTITY_OUTPUTS_INVALID",
                "Worker identity outputs are missing",
            )
        })?;
    if outputs.len() != result_refs.len() || outputs.len() > WEAPONRY_BLENDER_MAX_ARTIFACTS {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_IDENTITY_OUTPUTS_INVALID",
            "Worker identity output count differs from the Worker result",
        ));
    }
    let mut identity_by_path = BTreeMap::new();
    for (index, output) in outputs.iter().enumerate() {
        let object = output.as_object().ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_IDENTITY_OUTPUTS_INVALID",
                format!("Worker identity output {index} is not an object"),
            )
        })?;
        require_exact_fields(
            object,
            &["relative_path", "sha256", "byte_size"],
            &format!("Worker identity output {index}"),
        )?;
        let relative_path = string_field(object, "relative_path", "Worker identity output")?;
        let sha256 = string_field(object, "sha256", "Worker identity output")?;
        let byte_size = object
            .get("byte_size")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                contract(
                    "WEAPONRY_BLENDER_EXECUTION_IDENTITY_OUTPUTS_INVALID",
                    format!("Worker identity output {index}.byte_size is not an integer"),
                )
            })?;
        if relative_path.starts_with('/')
            || relative_path.contains("..")
            || relative_path.contains('\\')
            || !is_valid_hash(sha256)
            || byte_size == 0
            || byte_size > WEAPONRY_BLENDER_MAX_GLB_BYTES
            || identity_by_path
                .insert(relative_path.to_owned(), (sha256.to_owned(), byte_size))
                .is_some()
        {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_IDENTITY_OUTPUTS_INVALID",
                "Worker identity output path/hash/size is unsafe or duplicated",
            ));
        }
    }
    for reference in result_refs {
        let Some((sha256, byte_size)) = identity_by_path.get(&reference.relative_path) else {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_IDENTITY_OUTPUTS_MISMATCH",
                "Worker identity omits a Worker result output",
            ));
        };
        if sha256 != &reference.semantic_sha256 || *byte_size != reference.byte_size {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_IDENTITY_OUTPUTS_MISMATCH",
                "Worker identity output hash or size differs from Worker result",
            ));
        }
    }
    Ok(())
}

fn validate_worker_result(
    record: &WeaponryBlenderExecutionStoreRecord,
    bytes: &[u8],
) -> Result<(Value, Vec<WeaponryBlenderArtifactRef>), StoreError> {
    let value = parse_json_object(bytes, WEAPONRY_BLENDER_MAX_JSON_BYTES, "Worker result")?;
    let object = value.as_object().expect("object checked");
    require_exact_fields(
        object,
        &[
            "schema_version",
            "operation",
            "request_id",
            "project_id",
            "candidate_id",
            "source_authoring_mesh_sha256",
            "recipe_sha256",
            "policy",
            "worker_id",
            "worker_version",
            "blender_version",
            "blender_revision",
            "blender_build_hash",
            "worker_entrypoint_sha256",
            "dependency_lock_sha256",
            "input_canonical_sha256",
            "outputs",
            "stats",
            "checks",
            "runtime_write_performed",
            "stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "canonical_sha256",
        ],
        "Worker result",
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(WEAPONRY_BLENDER_WORKER_RESULT_SCHEMA)
        || object.get("operation").and_then(Value::as_str) != Some(record.operation.as_str())
        || object.get("request_id").and_then(Value::as_str) != Some(record.request_id.as_str())
        || object.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || object.get("candidate_id").and_then(Value::as_str) != Some(record.candidate_id.as_str())
        || object
            .get("source_authoring_mesh_sha256")
            .and_then(Value::as_str)
            != Some(record.source_object_sha256.as_str())
        || object.get("policy").and_then(Value::as_str) != Some(WORKER_POLICY)
        || object.get("worker_id").and_then(Value::as_str) != Some(record.worker_id.as_str())
        || object.get("worker_version").and_then(Value::as_str)
            != Some(record.worker_version.as_str())
        || object.get("blender_version").and_then(Value::as_str)
            != Some(record.blender_version.as_str())
        || object.get("blender_revision").and_then(Value::as_str)
            != Some(record.blender_revision.as_str())
        || object.get("blender_build_hash").and_then(Value::as_str)
            != Some(record.blender_revision.as_str())
        || object
            .get("worker_entrypoint_sha256")
            .and_then(Value::as_str)
            != Some(record.worker_entrypoint_sha256.as_str())
        || object.get("dependency_lock_sha256").and_then(Value::as_str)
            != Some(record.dependency_lock_sha256.as_str())
        || object.get("input_canonical_sha256").and_then(Value::as_str)
            != Some(record.request_sha256.as_str())
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("stage_advanced").and_then(Value::as_bool) != Some(false)
        || object.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || object.get("version_created").and_then(Value::as_bool) != Some(false)
        || object.get("export_performed").and_then(Value::as_bool) != Some(false)
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_RESULT_BINDING_MISMATCH",
            "fixed Worker result differs from the execution record",
        ));
    }
    let supplied = string_field(object, "canonical_sha256", "Worker result")?;
    if !is_valid_hash(supplied)
        || canonical_preimage_hash(&value, "canonical_sha256")? != supplied
        || supplied != record.worker_result_sha256
        || sha256_hex(bytes) != record.worker_result_object_sha256
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_RESULT_CANONICAL_MISMATCH",
            "Worker result semantic or object hash differs",
        ));
    }
    let outputs = object
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
                "Worker result outputs are missing",
            )
        })?;
    if outputs.len() < 5 || outputs.len() > WEAPONRY_BLENDER_MAX_ARTIFACTS {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
            "Worker result output count is outside its bound",
        ));
    }
    let mut refs = outputs
        .iter()
        .enumerate()
        .map(|(index, output)| output_record_map(output, &format!("Worker output {index}")))
        .collect::<Result<Vec<_>, _>>()?;
    for reference in &refs {
        let (expected_mime, max_bytes) = match reference.kind.as_str() {
            "high_glb" | "low_glb" => (
                WEAPONRY_BLENDER_EXECUTION_GLB_MIME,
                WEAPONRY_BLENDER_MAX_GLB_BYTES,
            ),
            "normal_map" | "ao_map" => (
                WEAPONRY_BLENDER_EXECUTION_PNG_MIME,
                WEAPONRY_BLENDER_MAX_MAP_BYTES,
            ),
            "worker_manifest" => (
                WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
                WEAPONRY_BLENDER_MAX_JSON_BYTES,
            ),
            _ => {
                return Err(contract(
                    "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
                    "Worker output kind is not allowlisted",
                ))
            }
        };
        if reference.mime != expected_mime || reference.byte_size > max_bytes {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
                "Worker output MIME or size differs from its fixed kind",
            ));
        }
    }
    let mut paths = BTreeSet::new();
    for reference in &refs {
        if !paths.insert(reference.relative_path.clone()) {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
                "Worker output paths are duplicated",
            ));
        }
        if reference.relative_path.starts_with('/')
            || reference.relative_path.contains("..")
            || reference.relative_path.contains('\\')
        {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
                "Worker output path is unsafe",
            ));
        }
    }
    refs.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    // The result's declared output set must contain both primary GLBs, the
    // manifest, and a balanced normal/AO collection.  Exact byte bindings are
    // checked against the record below after CAS metadata is available.
    let high = refs.iter().filter(|item| item.kind == "high_glb").count();
    let low = refs.iter().filter(|item| item.kind == "low_glb").count();
    let manifest = refs
        .iter()
        .filter(|item| item.kind == "worker_manifest")
        .count();
    let normal = refs.iter().filter(|item| item.kind == "normal_map").count();
    let ao = refs.iter().filter(|item| item.kind == "ao_map").count();
    if high != 1 || low != 1 || manifest != 1 || normal == 0 || normal != ao {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_WORKER_OUTPUT_INVALID",
            "Worker output set lacks exactly one High/Low/manifest or balanced maps",
        ));
    }
    Ok((value, refs))
}

fn validate_worker_manifest(
    record: &WeaponryBlenderExecutionStoreRecord,
    bytes: &[u8],
) -> Result<Value, StoreError> {
    let value = parse_json_object(bytes, WEAPONRY_BLENDER_MAX_JSON_BYTES, "Worker manifest")?;
    let object = value.as_object().expect("object checked");
    let schema = string_field(object, "schema_version", "Worker manifest")?;
    if schema != WEAPONRY_BLENDER_WORKER_MANIFEST_SCHEMA
        || string_field(object, "worker_id", "Worker manifest")? != record.worker_id
        || string_field(object, "worker_version", "Worker manifest")? != record.worker_version
        || string_field(object, "blender_version", "Worker manifest")? != record.blender_version
        || string_field(object, "blender_revision", "Worker manifest")? != record.blender_revision
        || string_field(object, "source_authoring_mesh_sha256", "Worker manifest")?
            != record.source_object_sha256
        || string_field(object, "operation", "Worker manifest")? != record.operation
        || string_field(object, "policy", "Worker manifest")? != WORKER_POLICY
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_MANIFEST_BINDING_MISMATCH",
            "Worker manifest differs from the fixed execution identity",
        ));
    }
    let supplied = string_field(object, "canonical_sha256", "Worker manifest")?;
    if !is_valid_hash(supplied) || canonical_preimage_hash(&value, "canonical_sha256")? != supplied
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_MANIFEST_CANONICAL_MISMATCH",
            "Worker manifest canonical hash differs",
        ));
    }
    // The manifest itself is one raw output object.  Its semantic output hash
    // is the raw bytes hash, while the inner canonical hash remains an
    // independently checked manifest identity.
    if sha256_hex(bytes) != record.worker_manifest_object_sha256
        || record.worker_manifest_sha256 != record.worker_manifest_object_sha256
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_MANIFEST_HASH_MISMATCH",
            "Worker manifest object hash differs",
        ));
    }
    Ok(value)
}

fn validate_receipt(
    record: &WeaponryBlenderExecutionStoreRecord,
    bytes: &[u8],
) -> Result<Value, StoreError> {
    let value = parse_json_object(bytes, WEAPONRY_BLENDER_MAX_JSON_BYTES, "adoption receipt")?;
    let object = value.as_object().expect("object checked");
    require_exact_fields(
        object,
        &[
            "schema_version",
            "operation",
            "request_id",
            "project_id",
            "candidate_id",
            "source_object_sha256",
            "worker_result_sha256",
            "worker_result_object_sha256",
            "worker_identity_sha256",
            "worker_identity_object_sha256",
            "artifacts",
            "runtime_write_performed",
            "persistent_user_data_touched",
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "visual_status",
            "human_status",
            "engine_status",
            "commercial_status",
            "durable_record_status",
            "canonical_sha256",
        ],
        "adoption receipt",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some(WEAPONRY_BLENDER_RECEIPT_SCHEMA)
        || object.get("operation").and_then(Value::as_str) != Some(record.operation.as_str())
        || object.get("request_id").and_then(Value::as_str) != Some(record.request_id.as_str())
        || object.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || object.get("candidate_id").and_then(Value::as_str) != Some(record.candidate_id.as_str())
        || object.get("source_object_sha256").and_then(Value::as_str)
            != Some(record.source_object_sha256.as_str())
        || object.get("worker_result_sha256").and_then(Value::as_str)
            != Some(record.worker_result_sha256.as_str())
        || object
            .get("worker_result_object_sha256")
            .and_then(Value::as_str)
            != Some(record.worker_result_object_sha256.as_str())
        || object.get("worker_identity_sha256").and_then(Value::as_str)
            != Some(record.worker_identity_sha256.as_str())
        || object
            .get("worker_identity_object_sha256")
            .and_then(Value::as_str)
            != Some(record.worker_identity_object_sha256.as_str())
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(true)
        || object
            .get("persistent_user_data_touched")
            .and_then(Value::as_bool)
            != Some(true)
        || object
            .get("production_stage_advanced")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || object.get("version_created").and_then(Value::as_bool) != Some(false)
        || object.get("export_performed").and_then(Value::as_bool) != Some(false)
        || object.get("visual_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("human_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("engine_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("commercial_status").and_then(Value::as_str) != Some("NOT_RUN")
        || object.get("durable_record_status").and_then(Value::as_str)
            != Some("CAS_ADOPTED_NO_LOOKUP_ROW")
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_RECEIPT_BINDING_MISMATCH",
            "adoption receipt differs from the fixed execution record",
        ));
    }
    let supplied = string_field(object, "canonical_sha256", "adoption receipt")?;
    if !is_valid_hash(supplied)
        || canonical_preimage_hash(&value, "canonical_sha256")? != supplied
        || supplied != record.receipt_sha256
        || sha256_hex(bytes) != record.receipt_object_sha256
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_RECEIPT_CANONICAL_MISMATCH",
            "adoption receipt semantic or object hash differs",
        ));
    }
    let artifacts = object
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_INVALID",
                "adoption receipt artifacts are missing",
            )
        })?;
    if artifacts.len() != 2 + record.normal_maps.len() + record.ao_maps.len() + 1 {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_INVALID",
            "adoption receipt artifact count differs",
        ));
    }
    Ok(value)
}

fn output_object_sha256<'a>(
    record: &'a WeaponryBlenderExecutionStoreRecord,
    reference: &WeaponryBlenderArtifactRef,
) -> Option<&'a str> {
    if reference.kind == "high_glb" {
        return Some(record.high_glb_object_sha256.as_str());
    }
    if reference.kind == "low_glb" {
        return Some(record.low_glb_object_sha256.as_str());
    }
    if reference.kind == "worker_manifest" {
        return Some(record.worker_manifest_object_sha256.as_str());
    }
    record
        .normal_maps
        .iter()
        .chain(record.ao_maps.iter())
        .find(|item| item.kind == reference.kind && item.relative_path == reference.relative_path)
        .map(|item| item.object_sha256.as_str())
}

fn validate_receipt_artifacts(
    record: &WeaponryBlenderExecutionStoreRecord,
    receipt: &Value,
    worker_refs: &[WeaponryBlenderArtifactRef],
) -> Result<(), StoreError> {
    let artifacts = receipt
        .get("artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_INVALID",
                "adoption receipt artifacts are missing",
            )
        })?;
    let expected = worker_refs
        .iter()
        .map(|reference| (reference.relative_path.as_str(), reference))
        .collect::<BTreeMap<_, _>>();
    let mut paths = BTreeSet::new();
    if artifacts.len() != expected.len() {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_INVALID",
            "adoption receipt artifact count differs from Worker result",
        ));
    }
    for (index, artifact) in artifacts.iter().enumerate() {
        let object = artifact.as_object().ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_INVALID",
                format!("adoption receipt artifact {index} is not an object"),
            )
        })?;
        require_exact_fields(
            object,
            &[
                "relative_path",
                "output_kind",
                "mime",
                "semantic_sha256",
                "object_sha256",
                "byte_size",
            ],
            &format!("adoption receipt artifact {index}"),
        )?;
        let relative_path = string_field(object, "relative_path", "adoption receipt artifact")?;
        if !paths.insert(relative_path.to_owned()) {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_INVALID",
                "adoption receipt artifact paths are duplicated",
            ));
        }
        let Some(expected_ref) = expected.get(relative_path) else {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_MISMATCH",
                "adoption receipt references an output absent from Worker result",
            ));
        };
        let output_kind = string_field(object, "output_kind", "adoption receipt artifact")?;
        let mime = string_field(object, "mime", "adoption receipt artifact")?;
        let semantic_sha256 = string_field(object, "semantic_sha256", "adoption receipt artifact")?;
        let object_sha256 = string_field(object, "object_sha256", "adoption receipt artifact")?;
        let byte_size = object
            .get("byte_size")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                contract(
                    "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_INVALID",
                    format!("adoption receipt artifact {index}.byte_size is not an integer"),
                )
            })?;
        let expected_object_sha256 =
            output_object_sha256(record, expected_ref).ok_or_else(|| {
                contract(
                    "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_MISMATCH",
                    "adoption receipt output kind/path is absent from durable record",
                )
            })?;
        if output_kind != expected_ref.kind
            || mime != expected_ref.mime
            || semantic_sha256 != expected_ref.semantic_sha256
            || object_sha256 != expected_object_sha256
            || byte_size != expected_ref.byte_size
        {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_RECEIPT_ARTIFACTS_MISMATCH",
                "adoption receipt artifact differs from Worker result or durable record",
            ));
        }
    }
    Ok(())
}

fn validate_registered_object(
    store: &Store,
    supplied: &CasObjectRecord,
    expected_sha256: &str,
    expected_mime: &str,
    expected_kind: &str,
    max_bytes: u64,
    role: &str,
) -> Result<Vec<u8>, StoreError> {
    if supplied.schema_version != CAS_SCHEMA
        || supplied.sha256 != expected_sha256
        || !is_valid_hash(expected_sha256)
        || supplied.mime != expected_mime
        || supplied.kind != expected_kind
        || supplied.size_bytes == 0
        || supplied.size_bytes > max_bytes
        || !matches!(supplied.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_CAS_METADATA_INVALID",
            format!("{role} CAS metadata differs from its durable binding"),
        ));
    }
    let current = store.get_object(expected_sha256)?.ok_or_else(|| {
        contract(
            "WEAPONRY_BLENDER_EXECUTION_CAS_MISSING",
            format!("{role} CAS object is not registered"),
        )
    })?;
    if current.schema_version != CAS_SCHEMA
        || current.sha256 != supplied.sha256
        || current.size_bytes != supplied.size_bytes
        || current.mime != supplied.mime
        || current.kind != supplied.kind
        || !matches!(current.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_CAS_METADATA_INVALID",
            format!("registered {role} CAS metadata differs"),
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(expected_sha256, max_bytes)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != supplied.size_bytes || sha256_hex(&bytes) != expected_sha256 {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_CAS_HASH_MISMATCH",
            format!("{role} CAS bytes do not match their content hash"),
        ));
    }
    Ok(bytes)
}

fn validate_cas_bundle(
    store: &Store,
    record: &WeaponryBlenderExecutionStoreRecord,
    cas: &WeaponryBlenderExecutionCasBundle,
) -> Result<Vec<WeaponryBlenderArtifactRef>, StoreError> {
    let source_bytes = validate_registered_object(
        store,
        &cas.source,
        &record.source_object_sha256,
        WEAPONRY_BLENDER_EXECUTION_GLB_MIME,
        // Source GLB kind belongs to the upstream authoring path and is
        // intentionally not hard-coded here.  Validate it below as a safe
        // opaque kind rather than allowing an arbitrary MIME/path.
        &cas.source.kind,
        WEAPONRY_BLENDER_MAX_GLB_BYTES,
        "source",
    )?;
    if !is_safe_kind(&cas.source.kind) || cas.source.size_bytes != record.source_object_size_bytes {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_SOURCE_BINDING_MISMATCH",
            "source object size or kind differs",
        ));
    }
    validate_glb_bytes(&source_bytes, record.source_object_size_bytes, "source")?;
    let identity_bytes = validate_registered_object(
        store,
        &cas.worker_identity,
        &record.worker_identity_object_sha256,
        WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
        WEAPONRY_BLENDER_WORKER_IDENTITY_KIND,
        WEAPONRY_BLENDER_MAX_JSON_BYTES,
        "Worker identity",
    )?;
    let identity_value = validate_worker_identity(record, &identity_bytes)?;
    let result_bytes = validate_registered_object(
        store,
        &cas.worker_result,
        &record.worker_result_object_sha256,
        WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
        WEAPONRY_BLENDER_WORKER_RESULT_KIND,
        WEAPONRY_BLENDER_MAX_JSON_BYTES,
        "Worker result",
    )?;
    let (_, worker_refs) = validate_worker_result(record, &result_bytes)?;
    validate_worker_identity_outputs(&identity_value, &worker_refs)?;
    let receipt_bytes = validate_registered_object(
        store,
        &cas.receipt,
        &record.receipt_object_sha256,
        WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
        WEAPONRY_BLENDER_RECEIPT_KIND,
        WEAPONRY_BLENDER_MAX_JSON_BYTES,
        "adoption receipt",
    )?;
    let receipt_value = validate_receipt(record, &receipt_bytes)?;
    validate_receipt_artifacts(record, &receipt_value, &worker_refs)?;
    let manifest_bytes = validate_registered_object(
        store,
        &cas.worker_manifest,
        &record.worker_manifest_object_sha256,
        WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
        WEAPONRY_BLENDER_WORKER_MANIFEST_KIND,
        WEAPONRY_BLENDER_MAX_JSON_BYTES,
        "Worker manifest",
    )?;
    validate_worker_manifest(record, &manifest_bytes)?;
    let high_bytes = validate_registered_object(
        store,
        &cas.high_glb,
        &record.high_glb_object_sha256,
        WEAPONRY_BLENDER_EXECUTION_GLB_MIME,
        WEAPONRY_BLENDER_HIGH_GLB_KIND,
        WEAPONRY_BLENDER_MAX_GLB_BYTES,
        "High GLB",
    )?;
    validate_glb_bytes(&high_bytes, record.high_glb_bytes, "High GLB")?;
    let low_bytes = validate_registered_object(
        store,
        &cas.low_glb,
        &record.low_glb_object_sha256,
        WEAPONRY_BLENDER_EXECUTION_GLB_MIME,
        WEAPONRY_BLENDER_LOW_GLB_KIND,
        WEAPONRY_BLENDER_MAX_GLB_BYTES,
        "Low GLB",
    )?;
    validate_glb_bytes(&low_bytes, record.low_glb_bytes, "Low GLB")?;
    if cas.normal_maps.len() != record.normal_maps.len()
        || cas.ao_maps.len() != record.ao_maps.len()
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_MAP_BINDING_MISMATCH",
            "normal/AO CAS bundle counts differ from the durable record",
        ));
    }
    for (index, (supplied, reference)) in cas
        .normal_maps
        .iter()
        .zip(record.normal_maps.iter())
        .enumerate()
    {
        let bytes = validate_registered_object(
            store,
            supplied,
            &reference.object_sha256,
            WEAPONRY_BLENDER_EXECUTION_PNG_MIME,
            WEAPONRY_BLENDER_NORMAL_MAP_KIND,
            WEAPONRY_BLENDER_MAX_MAP_BYTES,
            &format!("normal map {index}"),
        )?;
        if supplied.size_bytes != reference.byte_size
            || reference.semantic_sha256 != reference.object_sha256
        {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_MAP_BINDING_MISMATCH",
                "normal map semantic/object/size identity differs",
            ));
        }
        validate_png_bytes(&bytes, &format!("normal map {index}"))?;
    }
    for (index, (supplied, reference)) in cas.ao_maps.iter().zip(record.ao_maps.iter()).enumerate()
    {
        let bytes = validate_registered_object(
            store,
            supplied,
            &reference.object_sha256,
            WEAPONRY_BLENDER_EXECUTION_PNG_MIME,
            WEAPONRY_BLENDER_AO_MAP_KIND,
            WEAPONRY_BLENDER_MAX_MAP_BYTES,
            &format!("AO map {index}"),
        )?;
        if supplied.size_bytes != reference.byte_size
            || reference.semantic_sha256 != reference.object_sha256
        {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_MAP_BINDING_MISMATCH",
                "AO map semantic/object/size identity differs",
            ));
        }
        validate_png_bytes(&bytes, &format!("AO map {index}"))?;
    }
    let output_by_kind_path = worker_refs
        .into_iter()
        .map(|item| ((item.kind.clone(), item.relative_path.clone()), item))
        .collect::<BTreeMap<_, _>>();
    let high_key = ("high_glb".to_owned(), record.high_glb_relative_path.clone());
    let low_key = ("low_glb".to_owned(), record.low_glb_relative_path.clone());
    if output_by_kind_path
        .get(&high_key)
        .is_none_or(|item| item.semantic_sha256 != record.high_glb_sha256)
        || output_by_kind_path
            .get(&low_key)
            .is_none_or(|item| item.semantic_sha256 != record.low_glb_sha256)
    {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_RESULT_OUTPUT_BINDING_MISMATCH",
            "Worker result High/Low output differs from the durable record",
        ));
    }
    for reference in record.normal_maps.iter().chain(record.ao_maps.iter()) {
        let key = (reference.kind.clone(), reference.relative_path.clone());
        if output_by_kind_path.get(&key).is_none_or(|item| {
            item.semantic_sha256 != reference.semantic_sha256
                || item.byte_size != reference.byte_size
        }) {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_RESULT_OUTPUT_BINDING_MISMATCH",
                "Worker result map output differs from the durable record",
            ));
        }
    }
    let manifest = output_by_kind_path
        .iter()
        .find(|((kind, _), _)| kind == "worker_manifest")
        .map(|(_, value)| value)
        .ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_RESULT_OUTPUT_BINDING_MISMATCH",
                "Worker result worker_manifest output is missing",
            )
        })?;
    if manifest.semantic_sha256 != record.worker_manifest_object_sha256 {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_RESULT_OUTPUT_BINDING_MISMATCH",
            "Worker result manifest output differs from the durable record",
        ));
    }
    if let Some(package_manifest) = &cas.package_manifest {
        let expected = record
            .package_identity
            .package_manifest_sha256
            .as_deref()
            .ok_or_else(|| {
                contract(
                    "WEAPONRY_BLENDER_EXECUTION_PACKAGE_IDENTITY_INVALID",
                    "package manifest CAS was supplied without a record identity",
                )
            })?;
        validate_registered_object(
            store,
            package_manifest,
            expected,
            WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
            "weaponry-blender-package-manifest@1",
            WEAPONRY_BLENDER_MAX_JSON_BYTES,
            "package manifest",
        )?;
    } else if record.package_identity.package_manifest_sha256.is_some() {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_PACKAGE_IDENTITY_INVALID",
            "record package manifest identity lacks its CAS object",
        ));
    }
    if let Some(release) = &cas.release_eligibility {
        let expected = record
            .package_identity
            .release_eligibility_sha256
            .as_deref()
            .ok_or_else(|| {
                contract(
                    "WEAPONRY_BLENDER_EXECUTION_PACKAGE_IDENTITY_INVALID",
                    "release eligibility CAS was supplied without a record identity",
                )
            })?;
        validate_registered_object(
            store,
            release,
            expected,
            WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
            "weaponry-blender-release-eligibility@1",
            WEAPONRY_BLENDER_MAX_JSON_BYTES,
            "release eligibility",
        )?;
    } else if record.package_identity.release_eligibility_sha256.is_some() {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_PACKAGE_IDENTITY_INVALID",
            "record release eligibility identity lacks its CAS object",
        ));
    }
    Ok(output_artifacts(record))
}

fn same_record(
    left: &WeaponryBlenderExecutionStoreRecord,
    right: &WeaponryBlenderExecutionStoreRecord,
) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    // A timestamp is operation metadata, not idempotency input.  Every
    // content-bearing field and its canonical hash still has to match.
    left.created_at.clear();
    right.created_at.clear();
    left == right
}

fn record_json(record: &WeaponryBlenderExecutionStoreRecord) -> Result<String, StoreError> {
    let bytes = canonical_json_bytes(&record_value(record)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    String::from_utf8(bytes).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn read_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<WeaponryBlenderExecutionStoreRecord> {
    let payload: String = row.get(0)?;
    serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn current_object(
    transaction: &Transaction<'_>,
    sha256: &str,
) -> Result<CasObjectRecord, StoreError> {
    transaction
        .query_row(
            "SELECT sha256, size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
            params![sha256],
            |row| {
                let size: i64 = row.get(1)?;
                Ok(CasObjectRecord {
                    schema_version: CAS_SCHEMA.to_owned(),
                    sha256: row.get(0)?,
                    size_bytes: u64::try_from(size).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    mime: row.get(2)?,
                    kind: row.get(3)?,
                    reachability: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .map_err(StoreError::from)
}

fn root_pairs(record: &WeaponryBlenderExecutionStoreRecord) -> Vec<(String, String)> {
    let mut roots = vec![
        ("source".to_owned(), record.source_object_sha256.clone()),
        (
            "worker_identity".to_owned(),
            record.worker_identity_object_sha256.clone(),
        ),
        (
            "worker_result".to_owned(),
            record.worker_result_object_sha256.clone(),
        ),
        ("receipt".to_owned(), record.receipt_object_sha256.clone()),
        (
            "worker_manifest".to_owned(),
            record.worker_manifest_object_sha256.clone(),
        ),
        ("high_glb".to_owned(), record.high_glb_object_sha256.clone()),
        ("low_glb".to_owned(), record.low_glb_object_sha256.clone()),
    ];
    for reference in &record.normal_maps {
        roots.push((
            format!("normal_map:{}", reference.relative_path),
            reference.object_sha256.clone(),
        ));
    }
    for reference in &record.ao_maps {
        roots.push((
            format!("ao_map:{}", reference.relative_path),
            reference.object_sha256.clone(),
        ));
    }
    if let Some(hash) = &record.package_identity.package_manifest_sha256 {
        roots.push(("package_manifest".to_owned(), hash.clone()));
    }
    if let Some(hash) = &record.package_identity.release_eligibility_sha256 {
        roots.push(("release_eligibility".to_owned(), hash.clone()));
    }
    roots.sort_by(|left, right| left.0.cmp(&right.0));
    roots
}

fn read_roots(
    transaction: &Transaction<'_>,
    record: &WeaponryBlenderExecutionStoreRecord,
) -> Result<Vec<(String, String)>, StoreError> {
    let mut statement = transaction.prepare(
        "SELECT role, object_sha256 FROM weaponry_blender_execution_roots WHERE project_id = ?1 AND execution_id = ?2 ORDER BY role ASC",
    )?;
    let rows = statement.query_map(params![record.project_id, record.execution_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn ensure_persisted_roots(
    transaction: &Transaction<'_>,
    record: &WeaponryBlenderExecutionStoreRecord,
) -> Result<Vec<String>, StoreError> {
    let expected = root_pairs(record);
    if expected.len() > WEAPONRY_BLENDER_MAX_ROOTS {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_ROOT_COUNT_EXCEEDED",
            "fixed Blender execution root set exceeds its bound",
        ));
    }
    let actual = read_roots(transaction, record)?;
    if actual != expected {
        return Err(contract(
            "WEAPONRY_BLENDER_EXECUTION_ROOTS_MISMATCH",
            "durable Blender execution root set differs from its record",
        ));
    }
    Ok(actual.into_iter().map(|(_, hash)| hash).collect())
}

pub(crate) fn ensure_table(transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
            schema_version TEXT NOT NULL CHECK (schema_version = '{WEAPONRY_BLENDER_EXECUTION_RECORD_SCHEMA}'),
            project_id TEXT NOT NULL REFERENCES projects(project_id),
            candidate_id TEXT NOT NULL,
            execution_id TEXT NOT NULL,
            request_id TEXT NOT NULL,
            operation TEXT NOT NULL CHECK (operation = '{WEAPONRY_BLENDER_EXECUTION_OPERATION}'),
            source_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            worker_identity_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            worker_result_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            receipt_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            worker_manifest_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            high_glb_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            low_glb_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            request_sha256 TEXT NOT NULL,
            idempotency_key TEXT NOT NULL UNIQUE,
            canonical_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL,
            record_json TEXT NOT NULL,
            PRIMARY KEY (project_id, execution_id)
        );
        CREATE INDEX IF NOT EXISTS weaponry_blender_execution_candidate_idx
            ON {TABLE}(project_id, candidate_id, created_at DESC, execution_id ASC);
        CREATE INDEX IF NOT EXISTS weaponry_blender_execution_source_idx
            ON {TABLE}(source_object_sha256, project_id, execution_id);
        CREATE TABLE IF NOT EXISTS {ROOT_TABLE} (
            project_id TEXT NOT NULL,
            execution_id TEXT NOT NULL,
            role TEXT NOT NULL,
            object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
            PRIMARY KEY (project_id, execution_id, role),
            FOREIGN KEY (project_id, execution_id)
                REFERENCES {TABLE}(project_id, execution_id)
                ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS weaponry_blender_execution_roots_object_idx
            ON {ROOT_TABLE}(object_sha256, project_id, execution_id);"
    ))?;
    Ok(())
}

impl Store {
    /// Return the complete explicit CAS root set for one fixed Worker
    /// execution.  The order is stable by root role and includes every
    /// normal/AO map plus optional package/release identity objects.
    pub fn weaponry_blender_execution_cas_roots(
        record: &WeaponryBlenderExecutionStoreRecord,
    ) -> Vec<String> {
        root_pairs(record)
            .into_iter()
            .map(|(_, hash)| hash)
            .collect()
    }

    /// Persist one complete fixed Worker execution, or return the exact
    /// existing row for a replay of the same idempotency input.
    pub fn record_weaponry_blender_execution_with_replay(
        &self,
        commit: &WeaponryBlenderExecutionCommit,
    ) -> Result<(WeaponryBlenderExecutionStoreRecord, bool), StoreError> {
        validate_record_shape(&commit.record)?;
        let _ = validate_cas_bundle(self, &commit.record, &commit.cas)?;

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        if let Some(existing) = transaction
            .query_row(
                &format!("SELECT record_json FROM {TABLE} WHERE idempotency_key = ?1"),
                params![commit.record.idempotency_key],
                read_record,
            )
            .optional()?
        {
            validate_record_shape(&existing)?;
            if !same_record(&existing, &commit.record) {
                return Err(contract(
                    "WEAPONRY_BLENDER_EXECUTION_IDEMPOTENCY_CONFLICT",
                    "idempotency key is already bound to a different fixed Worker execution",
                ));
            }
            let hashes = ensure_persisted_roots(&transaction, &existing)?;
            mark_reachable_in_transaction(&transaction, &hashes)?;
            transaction.commit()?;
            return Ok((existing, true));
        }
        let project_exists: Option<String> = transaction
            .query_row(
                "SELECT project_id FROM projects WHERE project_id = ?1",
                params![commit.record.project_id],
                |row| row.get(0),
            )
            .optional()?;
        if project_exists.is_none() {
            return Err(contract(
                "PROJECT_SCOPE_DENIED",
                "fixed Blender execution project does not exist",
            ));
        }
        let record_json = record_json(&commit.record)?;
        transaction.execute(
            &format!(
                "INSERT INTO {TABLE} (schema_version, project_id, candidate_id, execution_id, request_id, operation, source_object_sha256, worker_identity_object_sha256, worker_result_object_sha256, receipt_object_sha256, worker_manifest_object_sha256, high_glb_object_sha256, low_glb_object_sha256, request_sha256, idempotency_key, canonical_sha256, created_at, record_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)"
            ),
            params![
                commit.record.schema_version,
                commit.record.project_id,
                commit.record.candidate_id,
                commit.record.execution_id,
                commit.record.request_id,
                commit.record.operation,
                commit.record.source_object_sha256,
                commit.record.worker_identity_object_sha256,
                commit.record.worker_result_object_sha256,
                commit.record.receipt_object_sha256,
                commit.record.worker_manifest_object_sha256,
                commit.record.high_glb_object_sha256,
                commit.record.low_glb_object_sha256,
                commit.record.request_sha256,
                commit.record.idempotency_key,
                commit.record.canonical_sha256,
                commit.record.created_at,
                record_json,
            ],
        )?;
        let roots = root_pairs(&commit.record);
        if roots.len() > WEAPONRY_BLENDER_MAX_ROOTS {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_ROOT_COUNT_EXCEEDED",
                "fixed Blender execution root set exceeds its bound",
            ));
        }
        for (role, object_sha256) in &roots {
            transaction.execute(
                &format!(
                    "INSERT INTO {ROOT_TABLE} (project_id, execution_id, role, object_sha256) VALUES (?1, ?2, ?3, ?4)"
                ),
                params![
                    commit.record.project_id,
                    commit.record.execution_id,
                    role,
                    object_sha256,
                ],
            )?;
        }
        mark_reachable_in_transaction(
            &transaction,
            &roots
                .iter()
                .map(|(_, hash)| hash.clone())
                .collect::<Vec<_>>(),
        )?;
        transaction.commit()?;
        Ok((commit.record.clone(), false))
    }

    /// Lookup by project and idempotency key and revalidate all CAS bytes and
    /// root rows.  A SQLite row without its exact CAS graph is not a valid
    /// successful read.
    pub fn get_weaponry_blender_execution(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<WeaponryBlenderExecutionStoreRecord>, StoreError> {
        validate_id(project_id, "project_id")?;
        validate_id(idempotency_key, "idempotency_key")?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let record = transaction
            .query_row(
                &format!(
                    "SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"
                ),
                params![project_id, idempotency_key],
                read_record,
            )
            .optional()?;
        let Some(record) = record else {
            transaction.rollback()?;
            return Ok(None);
        };
        validate_record_shape(&record)?;
        let roots = ensure_persisted_roots(&transaction, &record)?;
        for hash in &roots {
            let object = current_object(&transaction, hash)?;
            let _ = self
                .cas
                .read_verified_bounded(hash, object.size_bytes.max(1))
                .map_err(StoreError::from)?;
        }
        // Ensure the semantic records are still parseable and hash-bound.  A
        // full object bundle is reconstructed from SQLite metadata below.
        drop(transaction);
        drop(connection);
        self.revalidate_persisted_record(&record)?;
        Ok(Some(record))
    }

    /// Exact lookup by all stable execution identities.  This is stricter than
    /// an idempotency lookup and is intended for Runtime reopen/readback.
    pub fn get_weaponry_blender_execution_exact(
        &self,
        project_id: &str,
        execution_id: &str,
        source_object_sha256: &str,
        worker_result_object_sha256: &str,
        receipt_object_sha256: &str,
    ) -> Result<Option<WeaponryBlenderExecutionStoreRecord>, StoreError> {
        validate_id(project_id, "project_id")?;
        validate_id(execution_id, "execution_id")?;
        for (label, hash) in [
            ("source_object_sha256", source_object_sha256),
            ("worker_result_object_sha256", worker_result_object_sha256),
            ("receipt_object_sha256", receipt_object_sha256),
        ] {
            if !is_valid_hash(hash) {
                return Err(contract(
                    "WEAPONRY_BLENDER_EXECUTION_EXACT_LOOKUP_INVALID",
                    format!("{label} is not a SHA-256 hash"),
                ));
            }
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let record = transaction
            .query_row(
                &format!(
                    "SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND execution_id = ?2"
                ),
                params![project_id, execution_id,],
                read_record,
            )
            .optional()?;
        let Some(record) = record else {
            transaction.rollback()?;
            return Ok(None);
        };
        validate_record_shape(&record)?;
        let _ = ensure_persisted_roots(&transaction, &record)?;
        drop(transaction);
        drop(connection);
        self.revalidate_persisted_record(&record)?;
        if record.source_object_sha256 != source_object_sha256
            || record.worker_result_object_sha256 != worker_result_object_sha256
            || record.receipt_object_sha256 != receipt_object_sha256
        {
            return Err(contract(
                "WEAPONRY_BLENDER_EXECUTION_EXACT_LOOKUP_MISMATCH",
                "exact lookup hashes differ from the durable Blender execution record",
            ));
        }
        Ok(Some(record))
    }

    /// Read and validate the fixed Worker result JSON from CAS.
    pub fn read_weaponry_blender_worker_result_json(
        &self,
        record: &WeaponryBlenderExecutionStoreRecord,
    ) -> Result<Value, StoreError> {
        self.revalidate_persisted_record(record)?;
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.worker_result_object_sha256,
                WEAPONRY_BLENDER_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        Ok(validate_worker_result(record, &bytes)?.0)
    }

    /// Read and validate the Runtime adoption receipt JSON from CAS.
    pub fn read_weaponry_blender_receipt_json(
        &self,
        record: &WeaponryBlenderExecutionStoreRecord,
    ) -> Result<Value, StoreError> {
        self.revalidate_persisted_record(record)?;
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.receipt_object_sha256,
                WEAPONRY_BLENDER_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_receipt(record, &bytes)
    }

    pub fn read_weaponry_blender_worker_identity_json(
        &self,
        record: &WeaponryBlenderExecutionStoreRecord,
    ) -> Result<Value, StoreError> {
        self.revalidate_persisted_record(record)?;
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.worker_identity_object_sha256,
                WEAPONRY_BLENDER_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_worker_identity(record, &bytes)
    }

    pub fn read_weaponry_blender_worker_manifest_json(
        &self,
        record: &WeaponryBlenderExecutionStoreRecord,
    ) -> Result<Value, StoreError> {
        self.revalidate_persisted_record(record)?;
        let bytes = self
            .cas
            .read_verified_bounded(
                &record.worker_manifest_object_sha256,
                WEAPONRY_BLENDER_MAX_JSON_BYTES,
            )
            .map_err(StoreError::from)?;
        validate_worker_manifest(record, &bytes)
    }

    /// Generic JSON read alias for callers that use the Worker result as the
    /// primary execution payload.
    pub fn read_weaponry_blender_execution_json(
        &self,
        record: &WeaponryBlenderExecutionStoreRecord,
    ) -> Result<Value, StoreError> {
        self.read_weaponry_blender_worker_result_json(record)
    }

    fn revalidate_persisted_record(
        &self,
        record: &WeaponryBlenderExecutionStoreRecord,
    ) -> Result<(), StoreError> {
        validate_record_shape(record)?;
        let source = self.get_object_for_record(&record.source_object_sha256)?;
        let identity = self.get_object_for_record(&record.worker_identity_object_sha256)?;
        let result = self.get_object_for_record(&record.worker_result_object_sha256)?;
        let receipt = self.get_object_for_record(&record.receipt_object_sha256)?;
        let manifest = self.get_object_for_record(&record.worker_manifest_object_sha256)?;
        let high = self.get_object_for_record(&record.high_glb_object_sha256)?;
        let low = self.get_object_for_record(&record.low_glb_object_sha256)?;
        let mut normal = Vec::with_capacity(record.normal_maps.len());
        for item in &record.normal_maps {
            normal.push(self.get_object_for_record(&item.object_sha256)?);
        }
        let mut ao = Vec::with_capacity(record.ao_maps.len());
        for item in &record.ao_maps {
            ao.push(self.get_object_for_record(&item.object_sha256)?);
        }
        let package_manifest = match &record.package_identity.package_manifest_sha256 {
            Some(hash) => Some(self.get_object_for_record(hash)?),
            None => None,
        };
        let release_eligibility = match &record.package_identity.release_eligibility_sha256 {
            Some(hash) => Some(self.get_object_for_record(hash)?),
            None => None,
        };
        let bundle = WeaponryBlenderExecutionCasBundle {
            source,
            worker_identity: identity,
            worker_result: result,
            receipt,
            worker_manifest: manifest,
            high_glb: high,
            low_glb: low,
            normal_maps: normal,
            ao_maps: ao,
            package_manifest,
            release_eligibility,
        };
        let _ = validate_cas_bundle(self, record, &bundle)?;
        Ok(())
    }

    fn get_object_for_record(&self, sha256: &str) -> Result<CasObjectRecord, StoreError> {
        self.get_object(sha256)?.ok_or_else(|| {
            contract(
                "WEAPONRY_BLENDER_EXECUTION_CAS_MISSING",
                "durable Blender execution references a missing CAS object",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectRecord;
    use forgecad_core::sha256_hex;
    use serde_json::json;
    use std::fs;
    use uuid::Uuid;

    fn valid_glb() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&13_u32.to_le_bytes());
        bytes.push(0);
        bytes
    }

    fn valid_glb_with(marker: u8) -> Vec<u8> {
        let mut bytes = valid_glb();
        bytes[12] = marker;
        bytes
    }

    fn valid_png() -> Vec<u8> {
        b"\x89PNG\r\n\x1a\nfixture".to_vec()
    }

    fn valid_png_with(marker: u8) -> Vec<u8> {
        let mut bytes = valid_png();
        bytes.push(marker);
        bytes
    }

    fn object(store: &Store, bytes: &[u8], kind: &str, mime: &str) -> CasObjectRecord {
        store
            .put_object(bytes, None, mime, kind, "2026-09-01T00:00:00Z")
            .expect("CAS fixture")
            .record
    }

    fn put_project(store: &Store, project_id: &str) {
        store
            .insert_project(&ProjectRecord {
                schema_version: "Project@1".to_owned(),
                project_id: project_id.to_owned(),
                name: "Blender durable test".to_owned(),
                policy: json!({"scope":"weaponry-blender-test"}),
                created_at: "2026-09-01T00:00:00Z".to_owned(),
                updated_at: "2026-09-01T00:00:00Z".to_owned(),
                active_snapshot_revision: 0,
                head_snapshot_id: None,
                canonical_sha256: "a".repeat(64),
            })
            .expect("project");
    }

    struct Fixture {
        commit: WeaponryBlenderExecutionCommit,
        result_json: Value,
        receipt_json: Value,
    }

    fn fixture(store: &Store) -> Fixture {
        const PROJECT: &str = "blender-durable-project";
        const CANDIDATE: &str = "blender-durable-candidate";
        const EXECUTION: &str = "blender-durable-execution";
        const REQUEST: &str = "blender-durable-request";
        const IDEMPOTENCY: &str = "blender-durable-idempotency";
        put_project(store, PROJECT);

        let source_bytes = valid_glb_with(1);
        let source = object(
            store,
            &source_bytes,
            "authoring-mesh-glb@1",
            WEAPONRY_BLENDER_EXECUTION_GLB_MIME,
        );
        let high_bytes = valid_glb_with(2);
        let high = object(
            store,
            &high_bytes,
            WEAPONRY_BLENDER_HIGH_GLB_KIND,
            WEAPONRY_BLENDER_EXECUTION_GLB_MIME,
        );
        let low_bytes = valid_glb_with(3);
        let low = object(
            store,
            &low_bytes,
            WEAPONRY_BLENDER_LOW_GLB_KIND,
            WEAPONRY_BLENDER_EXECUTION_GLB_MIME,
        );
        let normal_bytes = valid_png_with(1);
        let normal = object(
            store,
            &normal_bytes,
            WEAPONRY_BLENDER_NORMAL_MAP_KIND,
            WEAPONRY_BLENDER_EXECUTION_PNG_MIME,
        );
        let ao_bytes = valid_png_with(2);
        let ao = object(
            store,
            &ao_bytes,
            WEAPONRY_BLENDER_AO_MAP_KIND,
            WEAPONRY_BLENDER_EXECUTION_PNG_MIME,
        );
        let request_sha256 = sha256_hex(b"fixed-request");
        let worker_entrypoint_sha256 = sha256_hex(b"entrypoint");
        let worker_bundle_sha256 = sha256_hex(b"bundle");
        let dependency_lock_sha256 = sha256_hex(b"dependency-lock");
        let normal_ref = WeaponryBlenderArtifactRef {
            relative_path: "output/maps/000-blade-normal.png".to_owned(),
            kind: "normal_map".to_owned(),
            mime: WEAPONRY_BLENDER_EXECUTION_PNG_MIME.to_owned(),
            semantic_sha256: normal.sha256.clone(),
            object_sha256: normal.sha256.clone(),
            byte_size: normal.size_bytes,
        };
        let ao_ref = WeaponryBlenderArtifactRef {
            relative_path: "output/maps/000-blade-ao.png".to_owned(),
            kind: "ao_map".to_owned(),
            mime: WEAPONRY_BLENDER_EXECUTION_PNG_MIME.to_owned(),
            semantic_sha256: ao.sha256.clone(),
            object_sha256: ao.sha256.clone(),
            byte_size: ao.size_bytes,
        };

        let output = |kind: &str, path: &str, mime: &str, hash: &str, bytes: u64| {
            json!({
                "kind":kind,
                "relative_path":path,
                "mime":mime,
                "byte_size":bytes,
                "sha256":hash,
                "cas_owner":"runtime",
                "durability":"pending_runtime_adoption"
            })
        };
        let high_output = output(
            "high_glb",
            "output/dragonfang-high.blend.glb",
            WEAPONRY_BLENDER_EXECUTION_GLB_MIME,
            &high.sha256,
            high.size_bytes,
        );
        let low_output = output(
            "low_glb",
            "output/dragonfang-low.blend.glb",
            WEAPONRY_BLENDER_EXECUTION_GLB_MIME,
            &low.sha256,
            low.size_bytes,
        );
        let normal_output = output(
            "normal_map",
            &normal_ref.relative_path,
            WEAPONRY_BLENDER_EXECUTION_PNG_MIME,
            &normal.sha256,
            normal.size_bytes,
        );
        let ao_output = output(
            "ao_map",
            &ao_ref.relative_path,
            WEAPONRY_BLENDER_EXECUTION_PNG_MIME,
            &ao.sha256,
            ao.size_bytes,
        );
        let mut manifest = json!({
            "schema_version":WEAPONRY_BLENDER_WORKER_MANIFEST_SCHEMA,
            "worker_id":"weaponry-blender-knife-worker@1",
            "worker_version":"0.1.0",
            "blender_version":"5.2.1",
            "blender_revision":"9e2066aef7ef",
            "blender_build_hash":"9e2066aef7ef",
            "worker_entrypoint_sha256":worker_entrypoint_sha256,
            "dependency_lock_sha256":dependency_lock_sha256,
            "operation":WEAPONRY_BLENDER_EXECUTION_OPERATION,
            "policy":"fixed-built-in-bevel-weighted-normal-decimate-smart-uv-cycles-bake@1",
            "request_id":REQUEST,
            "project_id":PROJECT,
            "candidate_id":CANDIDATE,
            "source_authoring_mesh_sha256":source.sha256,
            "outputs":[high_output,low_output,normal_output,ao_output],
            "canonical_sha256":""
        });
        manifest["canonical_sha256"] = Value::String(canonical_json_hash(&manifest));
        let manifest_bytes = canonical_json_bytes(&manifest).expect("manifest bytes");
        let manifest_object = object(
            store,
            &manifest_bytes,
            WEAPONRY_BLENDER_WORKER_MANIFEST_KIND,
            WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
        );
        let manifest_output = output(
            "worker_manifest",
            "output/manifest.json",
            WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
            &manifest_object.sha256,
            manifest_object.size_bytes,
        );
        let result_outputs = vec![
            high_output,
            low_output,
            normal_output,
            ao_output,
            manifest_output,
        ];
        let mut result = json!({
            "schema_version":WEAPONRY_BLENDER_WORKER_RESULT_SCHEMA,
            "operation":WEAPONRY_BLENDER_EXECUTION_OPERATION,
            "request_id":REQUEST,
            "project_id":PROJECT,
            "candidate_id":CANDIDATE,
            "source_authoring_mesh_sha256":source.sha256,
            "recipe_sha256":sha256_hex(b"recipe"),
            "policy":"fixed-built-in-bevel-weighted-normal-decimate-smart-uv-cycles-bake@1",
            "worker_id":"weaponry-blender-knife-worker@1",
            "worker_version":"0.1.0",
            "blender_version":"5.2.1",
            "blender_revision":"9e2066aef7ef",
            "blender_build_hash":"9e2066aef7ef",
            "worker_entrypoint_sha256":worker_entrypoint_sha256,
            "dependency_lock_sha256":dependency_lock_sha256,
            "input_canonical_sha256":request_sha256,
            "outputs":result_outputs,
            "stats":{
                "source_object_count":1,
                "high_object_count":1,
                "low_object_count":1,
                "source_triangle_count":1,
                "high_triangle_count":1,
                "low_triangle_count":1,
                "bake_map_count":2,
                "texture_size":512
            },
            "checks":{
                "validator_status":"prototype_pending_runtime_readback",
                "readback_status":"prototype_pending_runtime_readback",
                "deterministic_replay_status":"not_run",
                "stage_eligibility":"non_promoting_prototype",
                "human_status":"NOT_RUN",
                "engine_status":"NOT_RUN"
            },
            "runtime_write_performed":false,
            "stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "canonical_sha256":""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        let result_bytes = canonical_json_bytes(&result).expect("result bytes");
        let result_object = object(
            store,
            &result_bytes,
            WEAPONRY_BLENDER_WORKER_RESULT_KIND,
            WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
        );

        let identity_outputs = result["outputs"]
            .as_array()
            .expect("result outputs")
            .iter()
            .map(|output| {
                json!({
                    "relative_path":output["relative_path"],
                    "sha256":output["sha256"],
                    "byte_size":output["byte_size"]
                })
            })
            .collect::<Vec<_>>();
        let mut identity = json!({
            "schema_version":WEAPONRY_BLENDER_WORKER_IDENTITY_SCHEMA,
            "protocol":"weaponry-fixed-worker-stdio-json@1",
            "worker_id":"weaponry-blender-knife-worker@1",
            "blender_version":"5.2.1",
            "blender_revision":"9e2066aef7ef",
            "worker_bundle_sha256":worker_bundle_sha256,
            "dependency_lock_sha256":dependency_lock_sha256,
            "input_sha256":source.sha256,
            "stdout_sha256":sha256_hex(b"stdout"),
            "outputs":identity_outputs,
            "runtime_write_performed":false,
            "persistent_user_data_touched":false,
            "canonical_sha256":""
        });
        identity["canonical_sha256"] = Value::String(canonical_json_hash(&identity));
        let identity_bytes = canonical_json_bytes(&identity).expect("identity bytes");
        let identity_object = object(
            store,
            &identity_bytes,
            WEAPONRY_BLENDER_WORKER_IDENTITY_KIND,
            WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
        );

        let receipt_artifact = |output: &Value, object_sha256: &str| {
            json!({
                "relative_path":output["relative_path"],
                "output_kind":output["kind"],
                "mime":output["mime"],
                "semantic_sha256":output["sha256"],
                "object_sha256":object_sha256,
                "byte_size":output["byte_size"]
            })
        };
        let receipt_artifacts = vec![
            receipt_artifact(&result["outputs"][0], &high.sha256),
            receipt_artifact(&result["outputs"][1], &low.sha256),
            receipt_artifact(&result["outputs"][2], &normal.sha256),
            receipt_artifact(&result["outputs"][3], &ao.sha256),
            receipt_artifact(&result["outputs"][4], &manifest_object.sha256),
        ];
        let mut receipt = json!({
            "schema_version":WEAPONRY_BLENDER_RECEIPT_SCHEMA,
            "operation":WEAPONRY_BLENDER_EXECUTION_OPERATION,
            "request_id":REQUEST,
            "project_id":PROJECT,
            "candidate_id":CANDIDATE,
            "source_object_sha256":source.sha256,
            "worker_result_sha256":result["canonical_sha256"],
            "worker_result_object_sha256":result_object.sha256,
            "worker_identity_sha256":identity["canonical_sha256"],
            "worker_identity_object_sha256":identity_object.sha256,
            "artifacts":receipt_artifacts,
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
        let receipt_bytes = canonical_json_bytes(&receipt).expect("receipt bytes");
        let receipt_object = object(
            store,
            &receipt_bytes,
            WEAPONRY_BLENDER_RECEIPT_KIND,
            WEAPONRY_BLENDER_EXECUTION_JSON_MIME,
        );

        let mut record = WeaponryBlenderExecutionStoreRecord {
            schema_version: WEAPONRY_BLENDER_EXECUTION_RECORD_SCHEMA.to_owned(),
            project_id: PROJECT.to_owned(),
            candidate_id: CANDIDATE.to_owned(),
            execution_id: EXECUTION.to_owned(),
            request_id: REQUEST.to_owned(),
            operation: WEAPONRY_BLENDER_EXECUTION_OPERATION.to_owned(),
            source_object_sha256: source.sha256.clone(),
            source_object_size_bytes: source.size_bytes,
            worker_id: "weaponry-blender-knife-worker@1".to_owned(),
            worker_version: "0.1.0".to_owned(),
            blender_version: "5.2.1".to_owned(),
            blender_revision: "9e2066aef7ef".to_owned(),
            worker_entrypoint_sha256,
            worker_bundle_sha256,
            dependency_lock_sha256,
            worker_identity_sha256: identity["canonical_sha256"].as_str().unwrap().to_owned(),
            worker_identity_object_sha256: identity_object.sha256.clone(),
            worker_result_sha256: result["canonical_sha256"].as_str().unwrap().to_owned(),
            worker_result_object_sha256: result_object.sha256.clone(),
            receipt_sha256: receipt["canonical_sha256"].as_str().unwrap().to_owned(),
            receipt_object_sha256: receipt_object.sha256.clone(),
            worker_manifest_sha256: manifest_object.sha256.clone(),
            worker_manifest_object_sha256: manifest_object.sha256.clone(),
            worker_manifest_relative_path: "output/manifest.json".to_owned(),
            high_glb_sha256: high.sha256.clone(),
            high_glb_object_sha256: high.sha256.clone(),
            high_glb_bytes: high.size_bytes,
            high_glb_relative_path: "output/dragonfang-high.blend.glb".to_owned(),
            low_glb_sha256: low.sha256.clone(),
            low_glb_object_sha256: low.sha256.clone(),
            low_glb_bytes: low.size_bytes,
            low_glb_relative_path: "output/dragonfang-low.blend.glb".to_owned(),
            normal_maps: vec![normal_ref],
            ao_maps: vec![ao_ref],
            normal_map_set_sha256: String::new(),
            ao_map_set_sha256: String::new(),
            all_artifact_set_sha256: String::new(),
            package_identity: WeaponryBlenderPackageIdentity {
                packaged: false,
                package_manifest_sha256: None,
                resource_tree_sha256: None,
                blender_bundle_tree_sha256: None,
                release_eligibility_sha256: None,
                package_status: "development-fixed-worker@1".to_owned(),
                release_eligible: false,
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
            request_sha256,
            idempotency_key: IDEMPOTENCY.to_owned(),
            canonical_sha256: String::new(),
            created_at: "2026-09-01T00:00:00Z".to_owned(),
        };
        record.normal_map_set_sha256 =
            weaponry_blender_artifact_set_sha256(&record.normal_maps).expect("normal set");
        record.ao_map_set_sha256 =
            weaponry_blender_artifact_set_sha256(&record.ao_maps).expect("AO set");
        let mut all = vec![
            WeaponryBlenderArtifactRef {
                relative_path: record.high_glb_relative_path.clone(),
                kind: "high_glb".to_owned(),
                mime: WEAPONRY_BLENDER_EXECUTION_GLB_MIME.to_owned(),
                semantic_sha256: high.sha256.clone(),
                object_sha256: high.sha256.clone(),
                byte_size: high.size_bytes,
            },
            WeaponryBlenderArtifactRef {
                relative_path: record.low_glb_relative_path.clone(),
                kind: "low_glb".to_owned(),
                mime: WEAPONRY_BLENDER_EXECUTION_GLB_MIME.to_owned(),
                semantic_sha256: low.sha256.clone(),
                object_sha256: low.sha256.clone(),
                byte_size: low.size_bytes,
            },
        ];
        all.extend(record.normal_maps.clone());
        all.extend(record.ao_maps.clone());
        all.push(WeaponryBlenderArtifactRef {
            relative_path: record.worker_manifest_relative_path.clone(),
            kind: "worker_manifest".to_owned(),
            mime: WEAPONRY_BLENDER_EXECUTION_JSON_MIME.to_owned(),
            semantic_sha256: manifest_object.sha256.clone(),
            object_sha256: manifest_object.sha256.clone(),
            byte_size: 0,
        });
        record.all_artifact_set_sha256 =
            weaponry_blender_artifact_set_sha256(&all).expect("all set");
        record.canonical_sha256 =
            weaponry_blender_execution_record_canonical_sha256(&record).expect("record hash");

        Fixture {
            commit: WeaponryBlenderExecutionCommit {
                record,
                cas: WeaponryBlenderExecutionCasBundle {
                    source,
                    worker_identity: identity_object,
                    worker_result: result_object,
                    receipt: receipt_object,
                    worker_manifest: manifest_object,
                    high_glb: high,
                    low_glb: low,
                    normal_maps: vec![normal],
                    ao_maps: vec![ao],
                    package_manifest: None,
                    release_eligibility: None,
                },
            },
            result_json: result,
            receipt_json: receipt,
        }
    }

    #[test]
    fn durable_blender_commit_replay_exact_read_json_gc_and_reopen() {
        let root =
            std::env::temp_dir().join(format!("forgecad-blender-durable-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test root");
        let database = root.join("runtime.sqlite");
        let cas_root = root.join("cas");
        let commit = {
            let store = Store::open_with_cas(&database, &cas_root).expect("open");
            let fixture = fixture(&store);
            let (stored, replayed) = store
                .record_weaponry_blender_execution_with_replay(&fixture.commit)
                .expect("first commit");
            assert!(!replayed);
            assert_eq!(stored, fixture.commit.record);
            let (replayed_record, replayed) = store
                .record_weaponry_blender_execution_with_replay(&fixture.commit)
                .expect("exact replay");
            assert!(replayed);
            assert_eq!(replayed_record, stored);
            assert_eq!(
                store
                    .get_weaponry_blender_execution(&stored.project_id, &stored.idempotency_key,)
                    .expect("get"),
                Some(stored.clone())
            );
            assert_eq!(
                store
                    .read_weaponry_blender_worker_result_json(&stored)
                    .expect("result"),
                fixture.result_json
            );
            assert_eq!(
                store
                    .read_weaponry_blender_receipt_json(&stored)
                    .expect("receipt"),
                fixture.receipt_json
            );
            for hash in Store::weaponry_blender_execution_cas_roots(&stored) {
                assert_eq!(
                    store
                        .get_object(&hash)
                        .expect("object")
                        .expect("metadata")
                        .reachability,
                    "reachable"
                );
                let mut connection = store.lock_connection().expect("connection");
                let transaction = connection.transaction().expect("transaction");
                assert!(
                    super::super::authoring_mesh_edit_object_is_linked(&transaction, &hash)
                        .expect("GC root")
                );
                transaction.commit().expect("commit");
            }
            assert!(store
                .get_weaponry_blender_execution_exact(
                    &stored.project_id,
                    &stored.execution_id,
                    &stored.source_object_sha256,
                    &stored.worker_result_object_sha256,
                    &stored.receipt_object_sha256,
                )
                .expect("exact")
                .is_some());
            let error = store
                .get_weaponry_blender_execution_exact(
                    &stored.project_id,
                    &stored.execution_id,
                    &"f".repeat(64),
                    &stored.worker_result_object_sha256,
                    &stored.receipt_object_sha256,
                )
                .expect_err("exact drift");
            assert!(
                matches!(error, StoreError::Contract { code, .. } if code == "WEAPONRY_BLENDER_EXECUTION_EXACT_LOOKUP_MISMATCH")
            );
            let mut conflict = fixture.commit.clone();
            conflict.record.package_identity.package_status = "different-package@1".to_owned();
            conflict.record.canonical_sha256 =
                weaponry_blender_execution_record_canonical_sha256(&conflict.record)
                    .expect("conflict hash");
            let error = store
                .record_weaponry_blender_execution_with_replay(&conflict)
                .expect_err("idempotency conflict");
            assert!(
                matches!(error, StoreError::Contract { code, .. } if code == "WEAPONRY_BLENDER_EXECUTION_IDEMPOTENCY_CONFLICT")
            );
            fixture.commit
        };
        let reopened = Store::open_with_cas(&database, &cas_root).expect("reopen");
        let record = reopened
            .get_weaponry_blender_execution(
                &commit.record.project_id,
                &commit.record.idempotency_key,
            )
            .expect("reopen get")
            .expect("reopened record");
        assert_eq!(record, commit.record);
        assert_eq!(
            reopened
                .read_weaponry_blender_execution_json(&record)
                .expect("reopen JSON")["request_id"],
            Value::String(record.request_id.clone())
        );
        let _ = fs::remove_dir_all(root);
    }
}
