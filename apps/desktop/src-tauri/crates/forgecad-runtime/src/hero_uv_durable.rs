//! Typed, source-only Hero UV durable core.
//!
//! This module deliberately stops one boundary above the shared Store and
//! Runtime/MCP dispatch tables.  `HeroUvDurablePersistence` is the narrow
//! adapter root must implement with Runtime-owned CAS reservation plus one
//! SQLite transaction.  Keeping that adapter local to this file makes the
//! contract and replay rules testable without touching the high-conflict
//! `forgecad-store/src/lib.rs`, Runtime `lib.rs`, or MCP `main.rs`.
//!
//! The source Low object is never supplied inline by MCP.  Runtime reads the
//! candidate-bound Low GLB and Low readback from CAS, passes them as
//! `HeroUvSourceInput`, and this core builds the Worker request.  The Worker
//! is replayed twice; only the exact result/cohort pair is admitted to the
//! persistence adapter.  The resulting diagnostic is structural-only: it is
//! not artist unwrap, visual likeness, engine tangent round-trip, or a stage
//! promotion.

use base64::Engine;
use forgecad_contracts::{
    LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND, LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION,
    LOW_QUAD_DRAFT_DURABLE_READBACK_KIND,
};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use forgecad_store::hero_uv_durable::{
    HeroUvDurableCasPayload as StoreHeroUvDurableCasPayload,
    HeroUvDurableReadback as StoreHeroUvDurableReadback,
    HeroUvDurableRecord as StoreHeroUvDurableRecord,
};
use forgecad_store::{CasObject, CasReservation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fmt;

use super::{geometry_worker, now_string, Runtime, RuntimeError, MAX_GEOMETRY_ARTIFACT_BYTES};

pub const HERO_UV_DURABLE_PREPARE_SCHEMA: &str = "HeroUvDurablePrepareRequest@1";
pub const HERO_UV_DURABLE_GET_SCHEMA: &str = "HeroUvDurableGetRequest@1";
pub const HERO_UV_DURABLE_PREPARE_RESULT_SCHEMA: &str = "HeroUvDurablePrepareResult@1";
pub const HERO_UV_DURABLE_GET_RESULT_SCHEMA: &str = "HeroUvDurableGetResult@1";
pub const HERO_UV_DURABLE_RECORD_SCHEMA: &str = "HeroUvDurableRecord@1";
pub const HERO_UV_DURABLE_LINK_SCHEMA: &str = "HeroUvDurableLink@1";
pub const HERO_UV_LAYOUT_SCHEMA: &str = "HeroUvLayout@1";
pub const HERO_UV_LAYOUT_OPERATION: &str = "production_weapon_hero_uv_layout";
pub const HERO_UV_LAYOUT_REQUEST_SCHEMA: &str = "HeroUvLayoutRequest@1";
pub const HERO_UV_LAYOUT_POLICY: &str = "production-weapon-hero-uv-layout-first-person-weighted@1";
pub const HERO_UV_DURABLE_PREPARE_OPERATION: &str = "forgecad.production.hero-uv-durable-prepare@1";
pub const HERO_UV_DURABLE_GET_OPERATION: &str = "forgecad.production.hero-uv-durable-get@1";
pub const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
pub const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
pub const HERO_UV_LINK_POLICY: &str = "low-artifact-to-hero-uv-layout-diagnostic@1";
pub const HERO_UV_MATERIALIZATION_STATUS: &str = "runtime-owned-durable-hero-uv-source-only@1";
pub const HERO_UV_IDEMPOTENCY_POLICY: &str = "same-input-hash-replays-without-new-record@1";
pub const HERO_UV_LAYOUT_CAS_KIND: &str = "production-weapon-hero-uv-layout";
pub const HERO_UV_LINK_CAS_KIND: &str = "production-weapon-hero-uv-durable-link";
pub const LOW_ARTIFACT_CAS_KIND: &str = LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND;
pub const LOW_READBACK_CAS_KIND: &str = LOW_QUAD_DRAFT_DURABLE_READBACK_KIND;
pub const JSON_MIME: &str = "application/json";
pub const GLB_MIME: &str = "model/gltf-binary";
// Keep Runtime's bound equal to the decoded GLB bound enforced by the
// dedicated geometry Worker.  The Low durable Store has a wider historical
// object bound, but Hero UV must never advertise or forward more than the
// Worker can decode.
pub const MAX_GLB_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TRIANGLES: usize = 8_192;

pub const LIMITATIONS: &[&str] = &[
    "RUNTIME_SOLE_WRITER",
    "NO_STAGE_ADVANCEMENT",
    "NO_CANDIDATE_CONFIRM",
    "NO_VERSION_CREATED",
    "NO_EXPORT",
    "STRUCTURAL_DIAGNOSTIC_NOT_ARTIST_UNWRAP",
    "STRUCTURAL_ONLY_NOT_VISUAL_OR_ENGINE_PASS",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroUvDurableError(pub String);

impl fmt::Display for HeroUvDurableError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&self.0)
    }
}

impl std::error::Error for HeroUvDurableError {}

fn invalid(message: impl Into<String>) -> HeroUvDurableError {
    HeroUvDurableError(format!("HERO_UV_DURABLE_INVALID: {}", message.into()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HeroUvVisibilityWeight {
    pub part_id: String,
    pub first_person: f64,
    pub world: f64,
    pub hidden: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HeroUvDurablePrepareRequest {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub source_low_artifact_id: String,
    pub source_low_artifact_object_sha256: String,
    pub source_low_artifact_sha256: String,
    pub source_low_artifact_readback_object_sha256: String,
    pub source_low_artifact_readback_sha256: String,
    pub resolution: u64,
    pub padding_texels: u64,
    pub min_mip_level: u64,
    pub hard_edge_angle_deg: f64,
    pub stretch_threshold: f64,
    pub visibility_weights: Vec<HeroUvVisibilityWeight>,
    pub idempotency_key: String,
    pub max_response_bytes: u64,
    pub source_only: bool,
    pub runtime_write_performed: bool,
    pub writer_policy: String,
    pub canonicalization_policy: String,
    pub input_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HeroUvDurableGetRequest {
    pub schema_version: String,
    pub operation: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub source_low_artifact_id: String,
    pub source_low_artifact_sha256: String,
    pub layout_object_sha256: String,
    pub layout_canonical_sha256: String,
    pub link_id: String,
    pub link_object_sha256: String,
    pub resolution: u64,
    pub padding_texels: u64,
    pub min_mip_level: u64,
    pub hard_edge_angle_deg: f64,
    pub stretch_threshold: f64,
    pub visibility_weights_sha256: String,
    pub idempotency_key: String,
    pub source_only: bool,
    pub writer_policy: String,
    pub runtime_write_performed: bool,
    pub persistent_user_data_touched: bool,
    pub input_sha256: String,
}

/// The exact candidate-bound source identity that Runtime must resolve from
/// its current Low candidate before calling the pure Worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HeroUvSourceBinding {
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub source_low_artifact_id: String,
    pub source_low_artifact_object_sha256: String,
    pub source_low_artifact_sha256: String,
    pub source_low_artifact_readback_object_sha256: String,
    pub source_low_artifact_readback_sha256: String,
}

/// CAS bytes read by Runtime after candidate/object metadata checks.  The
/// durable core rechecks byte hashes and low readback lineage so a caller
/// cannot smuggle a different Low mesh under a valid-looking request.
#[derive(Debug, Clone, PartialEq)]
pub struct HeroUvSourceInput {
    pub binding: HeroUvSourceBinding,
    pub glb_bytes: Vec<u8>,
    pub glb_mime: String,
    pub glb_kind: String,
    pub readback_bytes: Vec<u8>,
    pub readback_mime: String,
    pub readback_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HeroUvDurableRecord {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub base_version_id: Option<String>,
    pub source_low_artifact_id: String,
    pub source_low_artifact_object_sha256: String,
    pub source_low_artifact_sha256: String,
    pub source_low_artifact_readback_object_sha256: String,
    pub source_low_artifact_readback_sha256: String,
    pub resolution: u64,
    pub padding_texels: u64,
    pub min_mip_level: u64,
    pub hard_edge_angle_deg: f64,
    pub stretch_threshold: f64,
    pub visibility_weights_sha256: String,
    pub layout_object_sha256: String,
    pub layout_canonical_sha256: String,
    pub worker_build_cohort_sha256: String,
    pub link_id: String,
    pub link_object_sha256: String,
    pub request_sha256: String,
    pub input_sha256: String,
    pub idempotency_key: String,
    pub materialization_status: String,
    pub canonical_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroUvDurableCasPayload {
    pub bytes: Vec<u8>,
    pub object_sha256: String,
    pub mime: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroUvDurableReadback {
    pub layout_bytes: Vec<u8>,
    pub link_bytes: Vec<u8>,
}

/// Root implements this trait with `Store::begin_cas_reservation`, two
/// `put_object_reserved` calls, and one SQLite row transaction.  The trait is
/// intentionally small: no Runtime state may be written by the MCP adapter.
pub trait HeroUvDurablePersistence {
    fn get_hero_uv(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<HeroUvDurableRecord>, String>;

    fn commit_hero_uv(
        &self,
        record: &HeroUvDurableRecord,
        layout: &HeroUvDurableCasPayload,
        link: &HeroUvDurableCasPayload,
    ) -> Result<(HeroUvDurableRecord, bool), String>;

    fn read_hero_uv_bundle(
        &self,
        record: &HeroUvDurableRecord,
    ) -> Result<HeroUvDurableReadback, String>;
}

fn is_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn exact_id(value: &str, field: &str) -> Result<(), HeroUvDurableError> {
    if is_id(value) {
        Ok(())
    } else {
        Err(invalid(format!("{field} is not an opaque id")))
    }
}

fn exact_hash(value: &str, field: &str) -> Result<(), HeroUvDurableError> {
    if is_hash(value) {
        Ok(())
    } else {
        Err(invalid(format!("{field} is not a SHA-256")))
    }
}

fn validate_weights(weights: &[HeroUvVisibilityWeight]) -> Result<(), HeroUvDurableError> {
    if weights.is_empty() || weights.len() > 4096 {
        return Err(invalid("visibility_weights count is outside the bound"));
    }
    let mut ids = BTreeSet::new();
    for weight in weights {
        exact_id(&weight.part_id, "visibility_weights.part_id")?;
        if !ids.insert(weight.part_id.as_str()) {
            return Err(invalid("visibility_weights contains a duplicate part"));
        }
        for (field, value) in [
            ("first_person", weight.first_person),
            ("world", weight.world),
            ("hidden", weight.hidden),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(invalid(format!(
                    "visibility_weights.{field} is outside 0..=1"
                )));
            }
        }
    }
    Ok(())
}

fn validate_common_config(
    resolution: u64,
    padding_texels: u64,
    min_mip_level: u64,
    hard_edge_angle_deg: f64,
    stretch_threshold: f64,
) -> Result<(), HeroUvDurableError> {
    if !matches!(resolution, 2048 | 4096) {
        return Err(invalid("resolution must be 2048 or 4096"));
    }
    if !(1..=128).contains(&padding_texels) {
        return Err(invalid("padding_texels is outside the bound"));
    }
    if min_mip_level > 12 {
        return Err(invalid("min_mip_level is outside the bound"));
    }
    if !hard_edge_angle_deg.is_finite() || hard_edge_angle_deg <= 0.1 || hard_edge_angle_deg >= 89.9
    {
        return Err(invalid("hard_edge_angle_deg is outside the bound"));
    }
    if !stretch_threshold.is_finite() || !(1.0..=100.0).contains(&stretch_threshold) {
        return Err(invalid("stretch_threshold is outside the bound"));
    }
    Ok(())
}

fn request_input_hash(value: &Value) -> Result<String, HeroUvDurableError> {
    let mut preimage = value.clone();
    let object = preimage
        .as_object_mut()
        .ok_or_else(|| invalid("prepare request must be an object"))?;
    if let Some(weights) = object.get("visibility_weights").cloned() {
        object.insert(
            "visibility_weights".to_owned(),
            normalized_visibility_weights_value(&weights)?,
        );
    }
    object.remove("input_sha256");
    object.remove("idempotency_key");
    Ok(canonical_json_hash(&preimage))
}

fn get_input_hash(value: &Value) -> Result<String, HeroUvDurableError> {
    let mut preimage = value.clone();
    let object = preimage
        .as_object_mut()
        .ok_or_else(|| invalid("get request must be an object"))?;
    object.insert("input_sha256".to_owned(), Value::String(String::new()));
    Ok(canonical_json_hash(&preimage))
}

fn value_from<T: for<'de> Deserialize<'de>>(
    value: Value,
    context: &str,
) -> Result<T, HeroUvDurableError> {
    serde_json::from_value(value)
        .map_err(|error| invalid(format!("{context} is not closed: {error}")))
}

fn validate_prepare_request(
    value: &Value,
    request: &HeroUvDurablePrepareRequest,
) -> Result<(), HeroUvDurableError> {
    require_base_version_key(value, "prepare")?;
    if request.schema_version != HERO_UV_DURABLE_PREPARE_SCHEMA
        || request.max_response_bytes != MAX_RESPONSE_BYTES as u64
        || !request.source_only
        || request.runtime_write_performed
        || request.writer_policy != WRITER_POLICY
        || request.canonicalization_policy != CANONICALIZATION_POLICY
    {
        return Err(invalid("prepare policy fields differ"));
    }
    for (field, value) in [
        ("project_id", &request.project_id),
        ("candidate_id", &request.candidate_id),
        ("source_low_artifact_id", &request.source_low_artifact_id),
        ("idempotency_key", &request.idempotency_key),
    ] {
        exact_id(value, field)?;
    }
    for (field, value) in [
        ("candidate_state_sha256", &request.candidate_state_sha256),
        (
            "source_low_artifact_object_sha256",
            &request.source_low_artifact_object_sha256,
        ),
        (
            "source_low_artifact_sha256",
            &request.source_low_artifact_sha256,
        ),
        (
            "source_low_artifact_readback_object_sha256",
            &request.source_low_artifact_readback_object_sha256,
        ),
        (
            "source_low_artifact_readback_sha256",
            &request.source_low_artifact_readback_sha256,
        ),
        ("input_sha256", &request.input_sha256),
    ] {
        exact_hash(value, field)?;
    }
    if request.source_low_artifact_id != request.source_low_artifact_object_sha256 {
        return Err(invalid(
            "source_low_artifact_id must equal the candidate-bound Low artifact object hash",
        ));
    }
    if let Some(version) = &request.base_version_id {
        exact_id(version, "base_version_id")?;
    }
    validate_common_config(
        request.resolution,
        request.padding_texels,
        request.min_mip_level,
        request.hard_edge_angle_deg,
        request.stretch_threshold,
    )?;
    validate_weights(&request.visibility_weights)?;
    if request_input_hash(value)? != request.input_sha256 {
        return Err(invalid("input_sha256 does not bind the prepare request"));
    }
    Ok(())
}

fn validate_get_request(
    value: &Value,
    request: &HeroUvDurableGetRequest,
) -> Result<(), HeroUvDurableError> {
    require_base_version_key(value, "get")?;
    if request.schema_version != HERO_UV_DURABLE_GET_SCHEMA
        || request.operation != HERO_UV_DURABLE_GET_OPERATION
        || !request.source_only
        || request.runtime_write_performed
        || request.persistent_user_data_touched
        || request.writer_policy != WRITER_POLICY
    {
        return Err(invalid("get policy fields differ"));
    }
    for (field, value) in [
        ("project_id", &request.project_id),
        ("candidate_id", &request.candidate_id),
        ("source_low_artifact_id", &request.source_low_artifact_id),
        ("link_id", &request.link_id),
        ("idempotency_key", &request.idempotency_key),
    ] {
        exact_id(value, field)?;
    }
    for (field, value) in [
        ("candidate_state_sha256", &request.candidate_state_sha256),
        (
            "source_low_artifact_sha256",
            &request.source_low_artifact_sha256,
        ),
        ("layout_object_sha256", &request.layout_object_sha256),
        ("layout_canonical_sha256", &request.layout_canonical_sha256),
        ("link_object_sha256", &request.link_object_sha256),
        (
            "visibility_weights_sha256",
            &request.visibility_weights_sha256,
        ),
        ("input_sha256", &request.input_sha256),
    ] {
        exact_hash(value, field)?;
    }
    if let Some(version) = &request.base_version_id {
        exact_id(version, "base_version_id")?;
    }
    validate_common_config(
        request.resolution,
        request.padding_texels,
        request.min_mip_level,
        request.hard_edge_angle_deg,
        request.stretch_threshold,
    )?;
    if get_input_hash(value)? != request.input_sha256 {
        return Err(invalid("input_sha256 does not bind the get request"));
    }
    Ok(())
}

fn require_base_version_key(value: &Value, context: &str) -> Result<(), HeroUvDurableError> {
    if value
        .as_object()
        .is_some_and(|object| object.contains_key("base_version_id"))
    {
        Ok(())
    } else {
        Err(invalid(format!(
            "{context} request must include base_version_id"
        )))
    }
}

fn normalized_visibility_weights(weights: &[HeroUvVisibilityWeight]) -> Value {
    let mut sorted = weights.to_vec();
    sorted.sort_by(|left, right| left.part_id.cmp(&right.part_id));
    for weight in &mut sorted {
        // The Worker decodes these bounded weights as f32.  Canonicalize the
        // Runtime preimage to that same representation so 0.1 does not hash
        // differently from the Worker-returned 0.10000000149011612.
        weight.first_person = f64::from(weight.first_person as f32);
        weight.world = f64::from(weight.world as f32);
        weight.hidden = f64::from(weight.hidden as f32);
    }
    serde_json::to_value(sorted).expect("Hero UV visibility weights are serializable")
}

fn normalized_visibility_weights_value(value: &Value) -> Result<Value, HeroUvDurableError> {
    let weights: Vec<HeroUvVisibilityWeight> = value_from(value.clone(), "visibility_weights")?;
    validate_weights(&weights)?;
    Ok(normalized_visibility_weights(&weights))
}

pub fn parse_prepare(value: Value) -> Result<HeroUvDurablePrepareRequest, HeroUvDurableError> {
    let request: HeroUvDurablePrepareRequest =
        value_from(value.clone(), HERO_UV_DURABLE_PREPARE_SCHEMA)?;
    validate_prepare_request(&value, &request)?;
    Ok(request)
}

pub fn parse_get(value: Value) -> Result<HeroUvDurableGetRequest, HeroUvDurableError> {
    let request: HeroUvDurableGetRequest = value_from(value.clone(), HERO_UV_DURABLE_GET_SCHEMA)?;
    validate_get_request(&value, &request)?;
    Ok(request)
}

fn binding_from_prepare(request: &HeroUvDurablePrepareRequest) -> HeroUvSourceBinding {
    HeroUvSourceBinding {
        project_id: request.project_id.clone(),
        candidate_id: request.candidate_id.clone(),
        candidate_state_sha256: request.candidate_state_sha256.clone(),
        base_version_id: request.base_version_id.clone(),
        source_low_artifact_id: request.source_low_artifact_id.clone(),
        source_low_artifact_object_sha256: request.source_low_artifact_object_sha256.clone(),
        source_low_artifact_sha256: request.source_low_artifact_sha256.clone(),
        source_low_artifact_readback_object_sha256: request
            .source_low_artifact_readback_object_sha256
            .clone(),
        source_low_artifact_readback_sha256: request.source_low_artifact_readback_sha256.clone(),
    }
}

fn validate_source(
    request: &HeroUvDurablePrepareRequest,
    source: &HeroUvSourceInput,
) -> Result<(), HeroUvDurableError> {
    let expected = binding_from_prepare(request);
    if source.binding != expected {
        return Err(invalid("Low source candidate/artifact binding differs"));
    }
    if request.source_low_artifact_id != request.source_low_artifact_object_sha256 {
        return Err(invalid(
            "Low source artifact identity is not bound to its CAS object hash",
        ));
    }
    if source.glb_mime != GLB_MIME
        || source.glb_kind != LOW_ARTIFACT_CAS_KIND
        || source.readback_mime != JSON_MIME
        || source.readback_kind != LOW_READBACK_CAS_KIND
    {
        return Err(invalid("Low source CAS metadata differs"));
    }
    if source.glb_bytes.is_empty()
        || source.glb_bytes.len() > MAX_GLB_BYTES
        || sha256_hex(&source.glb_bytes) != request.source_low_artifact_object_sha256
        || sha256_hex(&source.glb_bytes) != request.source_low_artifact_sha256
    {
        return Err(invalid(
            "Low GLB bytes do not match both object and artifact hashes",
        ));
    }
    if source.readback_bytes.is_empty()
        || source.readback_bytes.len() > MAX_JSON_BYTES
        || sha256_hex(&source.readback_bytes) != request.source_low_artifact_readback_object_sha256
    {
        return Err(invalid(
            "Low readback bytes do not match its CAS object hash",
        ));
    }
    let readback: Value = serde_json::from_slice(&source.readback_bytes)
        .map_err(|_| invalid("Low readback JSON is invalid"))?;
    if readback.get("schema_version").and_then(Value::as_str)
        != Some(LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION)
        || readback.get("artifact_sha256").and_then(Value::as_str)
            != Some(request.source_low_artifact_sha256.as_str())
        || readback
            .get("artifact_object_sha256")
            .and_then(Value::as_str)
            != Some(request.source_low_artifact_object_sha256.as_str())
        || readback.get("validator_status").and_then(Value::as_str) != Some("passed")
        || readback.get("hard_gate_passed") != Some(&Value::Bool(true))
        || readback.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || readback.get("edge_flow_status").and_then(Value::as_str) != Some("DRAFT_UNREVIEWED")
        || readback.get("promotion_eligible") != Some(&Value::Bool(false))
        || readback.get("production_stage_advanced") != Some(&Value::Bool(false))
        || readback.get("candidate_confirmed") != Some(&Value::Bool(false))
        || readback.get("version_created") != Some(&Value::Bool(false))
        || readback.get("export_performed") != Some(&Value::Bool(false))
    {
        return Err(invalid(
            "Low quad readback binding or strict status differs",
        ));
    }
    let canonical = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Low readback canonical hash is missing"))?;
    exact_hash(canonical, "Low readback canonical_sha256")?;
    let mut preimage = readback.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical
        || canonical != request.source_low_artifact_readback_sha256
    {
        return Err(invalid("Low readback canonical binding differs"));
    }
    Ok(())
}

pub fn build_worker_request(
    request: &HeroUvDurablePrepareRequest,
    glb_bytes: &[u8],
) -> Result<Value, HeroUvDurableError> {
    if glb_bytes.is_empty() || glb_bytes.len() > MAX_GLB_BYTES {
        return Err(invalid("Low GLB exceeds the bounded Worker input"));
    }
    let weights = serde_json::to_value(&request.visibility_weights)
        .map_err(|error| invalid(error.to_string()))?;
    let mut worker = json!({
        "schema_version": HERO_UV_LAYOUT_REQUEST_SCHEMA,
        "low_artifact_sha256": request.source_low_artifact_sha256,
        "low_glb_base64": base64::engine::general_purpose::STANDARD.encode(glb_bytes),
        "resolution": request.resolution,
        "padding_texels": request.padding_texels,
        "min_mip_level": request.min_mip_level,
        "hard_edge_angle_deg": request.hard_edge_angle_deg,
        "stretch_threshold": request.stretch_threshold,
        "visibility_weights": weights,
        "canonical_sha256": ""
    });
    let mut preimage = worker.clone();
    preimage
        .as_object_mut()
        .expect("Hero UV Worker request object")
        .remove("canonical_sha256");
    worker["canonical_sha256"] = Value::String(canonical_json_hash(&preimage));
    Ok(worker)
}

fn exact_worker_keys(object: &Map<String, Value>) -> bool {
    const FIELDS: &[&str] = &[
        "schema_version",
        "operation",
        "policy",
        "policy_sha256",
        "low_artifact_sha256",
        "resolution",
        "uv0_semantic",
        "uv1_semantic",
        "visibility_weight_policy",
        "mip_padding_policy",
        "seam_policy",
        "hard_edge_policy",
        "uv0_corners",
        "uv1_corners",
        "visibility_weights",
        "islands",
        "metrics",
        "mikk_replay",
        "source_only",
        "quality_status",
        "structural_status",
        "visual_status",
        "human_status",
        "engine_status",
        "distribution_status",
        "runtime_write_performed",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
        "promotion_eligible",
        "canonical_sha256",
    ];
    object.len() == FIELDS.len() && object.keys().all(|key| FIELDS.contains(&key.as_str()))
}

fn verify_canonical(value: &Value, field: &str) -> Result<String, HeroUvDurableError> {
    let supplied = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} is missing")))?;
    exact_hash(supplied, field)?;
    let mut preimage = value.clone();
    preimage[field] = Value::String(String::new());
    if canonical_json_hash(&preimage) != supplied {
        return Err(invalid(format!("{field} does not match payload")));
    }
    Ok(supplied.to_owned())
}

fn validate_worker_result(
    value: &Value,
    request: &HeroUvDurablePrepareRequest,
) -> Result<String, HeroUvDurableError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("Hero UV Worker result is not an object"))?;
    if !exact_worker_keys(object) {
        return Err(invalid("Hero UV Worker result fields are not closed"));
    }
    for (field, expected) in [
        ("schema_version", HERO_UV_LAYOUT_SCHEMA),
        ("operation", HERO_UV_LAYOUT_OPERATION),
        ("policy", HERO_UV_LAYOUT_POLICY),
        (
            "low_artifact_sha256",
            request.source_low_artifact_sha256.as_str(),
        ),
        ("uv0_semantic", "game-material-hero-channel@1"),
        ("uv1_semantic", "lightmap-bake-channel@1"),
        (
            "visibility_weight_policy",
            "first-person-world-hidden-per-part@1",
        ),
        (
            "mip_padding_policy",
            "base-padding-at-least-2^min-mip-level@1",
        ),
        (
            "seam_policy",
            "uv-seam-or-material-boundary-and-hard-edge-congruence@1",
        ),
        ("hard_edge_policy", "face-normal-angle-threshold@1"),
        ("quality_status", "structural_only"),
        ("structural_status", "PASS_SOURCE_STRUCTURAL"),
        ("visual_status", "NOT_PROVEN"),
        ("human_status", "NOT_RUN"),
        ("engine_status", "NOT_RUN"),
        ("distribution_status", "NOT_RUN"),
    ] {
        if object.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(invalid(format!("Worker result {field} binding differs")));
        }
    }
    if object.get("resolution").and_then(Value::as_u64) != Some(request.resolution)
        || object.get("source_only") != Some(&Value::Bool(true))
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("production_stage_advanced") != Some(&Value::Bool(false))
        || object.get("candidate_confirmed") != Some(&Value::Bool(false))
        || object.get("version_created") != Some(&Value::Bool(false))
        || object.get("export_performed") != Some(&Value::Bool(false))
        || object.get("promotion_eligible") != Some(&Value::Bool(false))
    {
        return Err(invalid("Worker result contains a promoting flag"));
    }
    let metrics = object
        .get("metrics")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Hero UV metrics are missing"))?;
    if metrics.get("padding_texels").and_then(Value::as_u64) != Some(request.padding_texels)
        || metrics
            .get("required_mip_padding_texels")
            .and_then(Value::as_u64)
            != Some(1u64 << request.min_mip_level)
        || metrics.get("first_person_weighting_applied") != Some(&Value::Bool(true))
    {
        return Err(invalid(
            "Hero UV mip or first-person metric binding differs",
        ));
    }
    let expected_weights = normalized_visibility_weights(&request.visibility_weights);
    let returned_weights = object
        .get("visibility_weights")
        .ok_or_else(|| invalid("Worker visibility weights are missing"))?;
    let returned_weights = normalized_visibility_weights_value(returned_weights)?;
    if returned_weights != expected_weights {
        return Err(invalid(format!(
            "Worker visibility weights differ from the normalized request: expected={expected_weights} actual={returned_weights}"
        )));
    }
    let _ = verify_canonical(value, "canonical_sha256")?;
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(canonical.to_owned())
}

fn worker_replay<F>(
    request: &HeroUvDurablePrepareRequest,
    glb: &[u8],
    runner: &mut F,
) -> Result<(Value, String, Value), HeroUvDurableError>
where
    F: FnMut(&Value) -> Result<(Value, String), String>,
{
    let worker_request = build_worker_request(request, glb)?;
    let (first, cohort_first) = runner(&worker_request)
        .map_err(|error| invalid(format!("Hero UV Worker failed: {error}")))?;
    let (second, cohort_second) = runner(&worker_request)
        .map_err(|error| invalid(format!("Hero UV Worker replay failed: {error}")))?;
    if first != second || cohort_first != cohort_second {
        return Err(invalid("Hero UV Worker replay or build cohort differs"));
    }
    if !is_hash(&cohort_first) {
        return Err(invalid("Hero UV Worker build cohort is invalid"));
    }
    validate_worker_result(&first, request)?;
    Ok((first, cohort_first, worker_request))
}

fn normalized_payload(
    value: &Value,
    context: &str,
) -> Result<(Value, Vec<u8>, String), HeroUvDurableError> {
    let mut payload = value.clone();
    payload["canonical_sha256"] = Value::String(String::new());
    let canonical = canonical_json_hash(&payload);
    payload["canonical_sha256"] = Value::String(canonical);
    let bytes = canonical_json_bytes(&payload)
        .map_err(|error| invalid(format!("{context} serialization failed: {error}")))?;
    if bytes.is_empty() || bytes.len() > MAX_JSON_BYTES {
        return Err(invalid(format!(
            "{context} exceeds the bounded JSON CAS size"
        )));
    }
    let object_sha256 = sha256_hex(&bytes);
    Ok((payload, bytes, object_sha256))
}

fn build_link(
    request: &HeroUvDurablePrepareRequest,
    layout: &Value,
    layout_object_sha256: &str,
    layout_canonical_sha256: &str,
    worker_cohort: &str,
    request_sha256: &str,
    weights_sha256: &str,
    created_at: &str,
) -> Result<Value, HeroUvDurableError> {
    let seed = canonical_json_hash(
        &json!({"project_id": request.project_id, "candidate_id": request.candidate_id, "candidate_state_sha256": request.candidate_state_sha256, "source_low_artifact_sha256": request.source_low_artifact_sha256, "layout_canonical_sha256": layout_canonical_sha256, "input_sha256": request.input_sha256}),
    );
    let link = json!({
        "schema_version": HERO_UV_DURABLE_LINK_SCHEMA,
        "link_id": format!("hero-uv-link-{}", &seed[..32]),
        "project_id": request.project_id, "candidate_id": request.candidate_id, "candidate_state_sha256": request.candidate_state_sha256, "base_version_id": request.base_version_id,
        "source_low_artifact_id": request.source_low_artifact_id, "source_low_artifact_object_sha256": request.source_low_artifact_object_sha256, "source_low_artifact_sha256": request.source_low_artifact_sha256, "source_low_artifact_readback_object_sha256": request.source_low_artifact_readback_object_sha256, "source_low_artifact_readback_sha256": request.source_low_artifact_readback_sha256,
        "resolution": request.resolution, "padding_texels": request.padding_texels, "min_mip_level": request.min_mip_level, "hard_edge_angle_deg": request.hard_edge_angle_deg, "stretch_threshold": request.stretch_threshold, "visibility_weights_sha256": weights_sha256,
        "layout_object_sha256": layout_object_sha256, "layout_canonical_sha256": layout_canonical_sha256, "worker_build_cohort_sha256": worker_cohort, "request_sha256": request_sha256, "input_sha256": request.input_sha256, "idempotency_key": request.idempotency_key,
        "replay_count": 2, "replay_byte_exact": true, "link_policy": HERO_UV_LINK_POLICY, "writer_policy": WRITER_POLICY, "materialization_status": HERO_UV_MATERIALIZATION_STATUS, "idempotency_policy": HERO_UV_IDEMPOTENCY_POLICY, "source_only": true, "runtime_write_performed": true, "persistent_user_data_touched": true, "production_stage_advanced": false, "candidate_confirmed": false, "version_created": false, "export_performed": false, "quality_status": "structural_only", "visual_status": "NOT_PROVEN", "human_status": "NOT_RUN", "engine_status": "NOT_RUN", "distribution_status": "NOT_RUN", "canonicalization_policy": CANONICALIZATION_POLICY, "canonical_sha256": "", "created_at": created_at,
    });
    let _ = layout;
    Ok(normalized_payload(&link, HERO_UV_DURABLE_LINK_SCHEMA)?.0)
}

fn record_from_parts(
    request: &HeroUvDurablePrepareRequest,
    layout: &Value,
    layout_object_sha256: &str,
    link: &Value,
    link_object_sha256: &str,
    worker_cohort: &str,
    request_sha256: &str,
    weights_sha256: &str,
    created_at: &str,
) -> Result<HeroUvDurableRecord, HeroUvDurableError> {
    let mut record = HeroUvDurableRecord {
        schema_version: HERO_UV_DURABLE_RECORD_SCHEMA.to_owned(),
        project_id: request.project_id.clone(),
        candidate_id: request.candidate_id.clone(),
        candidate_state_sha256: request.candidate_state_sha256.clone(),
        base_version_id: request.base_version_id.clone(),
        source_low_artifact_id: request.source_low_artifact_id.clone(),
        source_low_artifact_object_sha256: request.source_low_artifact_object_sha256.clone(),
        source_low_artifact_sha256: request.source_low_artifact_sha256.clone(),
        source_low_artifact_readback_object_sha256: request
            .source_low_artifact_readback_object_sha256
            .clone(),
        source_low_artifact_readback_sha256: request.source_low_artifact_readback_sha256.clone(),
        resolution: request.resolution,
        padding_texels: request.padding_texels,
        min_mip_level: request.min_mip_level,
        hard_edge_angle_deg: request.hard_edge_angle_deg,
        stretch_threshold: request.stretch_threshold,
        visibility_weights_sha256: weights_sha256.to_owned(),
        layout_object_sha256: layout_object_sha256.to_owned(),
        layout_canonical_sha256: layout["canonical_sha256"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        worker_build_cohort_sha256: worker_cohort.to_owned(),
        link_id: link["link_id"].as_str().unwrap_or_default().to_owned(),
        link_object_sha256: link_object_sha256.to_owned(),
        request_sha256: request_sha256.to_owned(),
        input_sha256: request.input_sha256.clone(),
        idempotency_key: request.idempotency_key.clone(),
        materialization_status: HERO_UV_MATERIALIZATION_STATUS.to_owned(),
        canonical_sha256: String::new(),
        created_at: created_at.to_owned(),
    };
    let value = serde_json::to_value(&record).map_err(|error| invalid(error.to_string()))?;
    record.canonical_sha256 = canonical_json_hash(&value);
    Ok(record)
}

fn verify_record(record: &HeroUvDurableRecord) -> Result<(), HeroUvDurableError> {
    if record.schema_version != HERO_UV_DURABLE_RECORD_SCHEMA
        || record.materialization_status != HERO_UV_MATERIALIZATION_STATUS
    {
        return Err(invalid("durable record policy differs"));
    }
    for (field, value) in [
        ("project_id", &record.project_id),
        ("candidate_id", &record.candidate_id),
        ("source_low_artifact_id", &record.source_low_artifact_id),
        ("link_id", &record.link_id),
        ("idempotency_key", &record.idempotency_key),
    ] {
        exact_id(value, field)?;
    }
    for (field, value) in [
        ("candidate_state_sha256", &record.candidate_state_sha256),
        (
            "source_low_artifact_object_sha256",
            &record.source_low_artifact_object_sha256,
        ),
        (
            "source_low_artifact_sha256",
            &record.source_low_artifact_sha256,
        ),
        (
            "source_low_artifact_readback_object_sha256",
            &record.source_low_artifact_readback_object_sha256,
        ),
        (
            "source_low_artifact_readback_sha256",
            &record.source_low_artifact_readback_sha256,
        ),
        (
            "visibility_weights_sha256",
            &record.visibility_weights_sha256,
        ),
        ("layout_object_sha256", &record.layout_object_sha256),
        ("layout_canonical_sha256", &record.layout_canonical_sha256),
        (
            "worker_build_cohort_sha256",
            &record.worker_build_cohort_sha256,
        ),
        ("link_object_sha256", &record.link_object_sha256),
        ("request_sha256", &record.request_sha256),
        ("input_sha256", &record.input_sha256),
        ("canonical_sha256", &record.canonical_sha256),
    ] {
        exact_hash(value, field)?;
    }
    validate_common_config(
        record.resolution,
        record.padding_texels,
        record.min_mip_level,
        record.hard_edge_angle_deg,
        record.stretch_threshold,
    )?;
    if let Some(version) = &record.base_version_id {
        exact_id(version, "base_version_id")?;
    }
    let mut value = serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    value["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&value) != record.canonical_sha256 {
        return Err(invalid("durable record canonical hash differs"));
    }
    Ok(())
}

fn verify_bundle(
    record: &HeroUvDurableRecord,
    bundle: &HeroUvDurableReadback,
) -> Result<(Value, Value), HeroUvDurableError> {
    verify_record(record)?;
    if bundle.layout_bytes.len() > MAX_JSON_BYTES
        || bundle.link_bytes.len() > MAX_JSON_BYTES
        || sha256_hex(&bundle.layout_bytes) != record.layout_object_sha256
        || sha256_hex(&bundle.link_bytes) != record.link_object_sha256
    {
        return Err(invalid("durable CAS object hash readback differs"));
    }
    let layout: Value = serde_json::from_slice(&bundle.layout_bytes)
        .map_err(|_| invalid("durable Hero UV layout JSON is invalid"))?;
    let link: Value = serde_json::from_slice(&bundle.link_bytes)
        .map_err(|_| invalid("durable Hero UV link JSON is invalid"))?;
    let layout_hash = verify_canonical(&layout, "canonical_sha256")?;
    let link_hash = verify_canonical(&link, "canonical_sha256")?;
    if layout_hash != record.layout_canonical_sha256
        || layout.get("schema_version").and_then(Value::as_str) != Some(HERO_UV_LAYOUT_SCHEMA)
        || link.get("schema_version").and_then(Value::as_str) != Some(HERO_UV_DURABLE_LINK_SCHEMA)
        || link.get("link_id").and_then(Value::as_str) != Some(record.link_id.as_str())
        || link_hash
            != link
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .unwrap_or_default()
    {
        return Err(invalid("durable layout/link lineage differs"));
    }
    if canonical_json_bytes(&layout).map_err(|error| invalid(error.to_string()))?
        != bundle.layout_bytes
        || canonical_json_bytes(&link).map_err(|error| invalid(error.to_string()))?
            != bundle.link_bytes
    {
        return Err(invalid("durable CAS bytes are not canonical JSON"));
    }
    Ok((layout, link))
}

fn output_value(
    record: &HeroUvDurableRecord,
    layout: Value,
    link: Value,
    schema: &str,
    operation: &str,
    runtime_write: bool,
    replayed: bool,
) -> Result<Value, HeroUvDurableError> {
    let weights = layout
        .get("visibility_weights")
        .cloned()
        .ok_or_else(|| invalid("layout visibility weights are missing"))?;
    let mut output = json!({
        "schema_version": schema, "operation": operation, "project_id": record.project_id, "candidate_id": record.candidate_id, "candidate_state_sha256": record.candidate_state_sha256, "base_version_id": record.base_version_id,
        "source_low_artifact_id": record.source_low_artifact_id, "source_low_artifact_object_sha256": record.source_low_artifact_object_sha256, "source_low_artifact_sha256": record.source_low_artifact_sha256, "source_low_artifact_readback_object_sha256": record.source_low_artifact_readback_object_sha256, "source_low_artifact_readback_sha256": record.source_low_artifact_readback_sha256,
        "resolution": record.resolution, "padding_texels": record.padding_texels, "min_mip_level": record.min_mip_level, "hard_edge_angle_deg": record.hard_edge_angle_deg, "stretch_threshold": record.stretch_threshold, "visibility_weights": weights, "visibility_weights_sha256": record.visibility_weights_sha256,
        "layout": layout, "layout_object_sha256": record.layout_object_sha256, "layout_canonical_sha256": record.layout_canonical_sha256, "worker_build_cohort_sha256": record.worker_build_cohort_sha256, "replay_count": 2, "replay_byte_exact": true, "request_sha256": record.request_sha256, "request_input_sha256": record.input_sha256, "idempotency_key": record.idempotency_key, "replayed": replayed, "restart_hash_verified": true,
        "link_id": record.link_id, "link_object_sha256": record.link_object_sha256, "durable_link": link, "source_only": true, "writer_policy": WRITER_POLICY, "runtime_write_performed": runtime_write, "persistent_user_data_touched": runtime_write, "production_stage_advanced": false, "candidate_confirmed": false, "version_created": false, "export_performed": false, "quality_status": "structural_only", "visual_status": "NOT_PROVEN", "human_status": "NOT_RUN", "engine_status": "NOT_RUN", "distribution_status": "NOT_RUN", "limitations": LIMITATIONS, "canonicalization_policy": CANONICALIZATION_POLICY, "canonical_sha256": ""
    });
    if canonical_json_bytes(&output)
        .map_err(|error| invalid(error.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("Hero UV durable response exceeds its bound"));
    }
    output["canonical_sha256"] = Value::String(canonical_json_hash(&output));
    Ok(output)
}

pub fn prepare<F, P>(
    value: Value,
    source: HeroUvSourceInput,
    mut runner: F,
    persistence: &P,
    created_at: &str,
) -> Result<Value, HeroUvDurableError>
where
    F: FnMut(&Value) -> Result<(Value, String), String>,
    P: HeroUvDurablePersistence,
{
    let request = parse_prepare(value)?;
    if created_at.is_empty() {
        return Err(invalid("created_at is empty"));
    }
    if let Some(existing) = persistence
        .get_hero_uv(&request.project_id, &request.idempotency_key)
        .map_err(invalid)?
    {
        if existing.input_sha256 != request.input_sha256 {
            return Err(invalid(
                "HERO_UV_DURABLE_RECORD_CONFLICT: idempotency key is bound to another input",
            ));
        }
        let bundle = persistence
            .read_hero_uv_bundle(&existing)
            .map_err(invalid)?;
        let (layout, link) = verify_bundle(&existing, &bundle)?;
        return output_value(
            &existing,
            layout,
            link,
            HERO_UV_DURABLE_PREPARE_RESULT_SCHEMA,
            HERO_UV_DURABLE_PREPARE_OPERATION,
            true,
            true,
        );
    }
    validate_source(&request, &source)?;
    let (layout, worker_cohort, worker_request) =
        worker_replay(&request, &source.glb_bytes, &mut runner)?;
    let layout_canonical_sha256 = layout
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Hero UV layout canonical hash is missing"))?
        .to_owned();
    let layout_bytes = canonical_json_bytes(&layout).map_err(|error| invalid(error.to_string()))?;
    if layout_bytes.len() > MAX_JSON_BYTES {
        return Err(invalid("Hero UV layout exceeds its CAS bound"));
    }
    let layout_object_sha256 = sha256_hex(&layout_bytes);
    let request_sha256 = worker_request
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Worker request canonical hash is missing"))?
        .to_owned();
    let weights_sha256 = canonical_json_hash(
        layout
            .get("visibility_weights")
            .ok_or_else(|| invalid("Hero UV layout visibility weights are missing"))?,
    );
    let link = build_link(
        &request,
        &layout,
        &layout_object_sha256,
        &layout_canonical_sha256,
        &worker_cohort,
        &request_sha256,
        &weights_sha256,
        created_at,
    )?;
    let link_bytes = canonical_json_bytes(&link).map_err(|error| invalid(error.to_string()))?;
    let link_object_sha256 = sha256_hex(&link_bytes);
    let record = record_from_parts(
        &request,
        &layout,
        &layout_object_sha256,
        &link,
        &link_object_sha256,
        &worker_cohort,
        &request_sha256,
        &weights_sha256,
        created_at,
    )?;
    let layout_payload = HeroUvDurableCasPayload {
        bytes: layout_bytes,
        object_sha256: layout_object_sha256,
        mime: JSON_MIME.to_owned(),
        kind: HERO_UV_LAYOUT_CAS_KIND.to_owned(),
    };
    let link_payload = HeroUvDurableCasPayload {
        bytes: link_bytes,
        object_sha256: link_object_sha256,
        mime: JSON_MIME.to_owned(),
        kind: HERO_UV_LINK_CAS_KIND.to_owned(),
    };
    let (stored, replayed) = persistence
        .commit_hero_uv(&record, &layout_payload, &link_payload)
        .map_err(invalid)?;
    let bundle = persistence.read_hero_uv_bundle(&stored).map_err(invalid)?;
    let (stored_layout, stored_link) = verify_bundle(&stored, &bundle)?;
    output_value(
        &stored,
        stored_layout,
        stored_link,
        HERO_UV_DURABLE_PREPARE_RESULT_SCHEMA,
        HERO_UV_DURABLE_PREPARE_OPERATION,
        true,
        replayed,
    )
}

pub fn get<P>(value: Value, persistence: &P) -> Result<Value, HeroUvDurableError>
where
    P: HeroUvDurablePersistence,
{
    let request = parse_get(value)?;
    let record = persistence
        .get_hero_uv(&request.project_id, &request.idempotency_key)
        .map_err(invalid)?
        .ok_or_else(|| invalid("Hero UV durable record is unavailable"))?;
    if record.candidate_id != request.candidate_id
        || record.candidate_state_sha256 != request.candidate_state_sha256
        || record.base_version_id != request.base_version_id
        || record.source_low_artifact_id != request.source_low_artifact_id
        || record.source_low_artifact_sha256 != request.source_low_artifact_sha256
        || record.layout_object_sha256 != request.layout_object_sha256
        || record.layout_canonical_sha256 != request.layout_canonical_sha256
        || record.link_id != request.link_id
        || record.link_object_sha256 != request.link_object_sha256
        || record.resolution != request.resolution
        || record.padding_texels != request.padding_texels
        || record.min_mip_level != request.min_mip_level
        || record.hard_edge_angle_deg != request.hard_edge_angle_deg
        || record.stretch_threshold != request.stretch_threshold
        || record.visibility_weights_sha256 != request.visibility_weights_sha256
    {
        return Err(invalid("Hero UV durable get binding differs"));
    }
    let bundle = persistence.read_hero_uv_bundle(&record).map_err(invalid)?;
    let (layout, link) = verify_bundle(&record, &bundle)?;
    output_value(
        &record,
        layout,
        link,
        HERO_UV_DURABLE_GET_RESULT_SCHEMA,
        HERO_UV_DURABLE_GET_OPERATION,
        false,
        true,
    )
}

fn store_record(record: &HeroUvDurableRecord) -> Result<StoreHeroUvDurableRecord, String> {
    serde_json::from_value(
        serde_json::to_value(record)
            .map_err(|error| format!("record serialization failed: {error}"))?,
    )
    .map_err(|error| format!("Store Hero UV record conversion failed: {error}"))
}

fn runtime_record(record: StoreHeroUvDurableRecord) -> Result<HeroUvDurableRecord, String> {
    serde_json::from_value(
        serde_json::to_value(record)
            .map_err(|error| format!("record serialization failed: {error}"))?,
    )
    .map_err(|error| format!("Runtime Hero UV record conversion failed: {error}"))
}

fn store_payload(payload: &HeroUvDurableCasPayload) -> StoreHeroUvDurableCasPayload {
    StoreHeroUvDurableCasPayload {
        bytes: payload.bytes.clone(),
        object_sha256: payload.object_sha256.clone(),
        mime: payload.mime.clone(),
        kind: payload.kind.clone(),
    }
}

fn runtime_readback(readback: StoreHeroUvDurableReadback) -> HeroUvDurableReadback {
    HeroUvDurableReadback {
        layout_bytes: readback.layout_bytes,
        link_bytes: readback.link_bytes,
    }
}

struct StoreHeroUvPersistence<'a> {
    runtime: &'a Runtime,
}

fn release_reserved(
    runtime: &Runtime,
    reservation: &CasReservation,
    objects: &[CasObject],
    cleanup: bool,
) {
    for object in objects.iter().rev() {
        let _ = runtime
            .store
            .release_cas_reservation_object(reservation, object, cleanup);
    }
}

impl HeroUvDurablePersistence for StoreHeroUvPersistence<'_> {
    fn get_hero_uv(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<HeroUvDurableRecord>, String> {
        self.runtime
            .store
            .get_hero_uv(project_id, idempotency_key)
            .map_err(|error| error.to_string())?
            .map(runtime_record)
            .transpose()
    }

    fn commit_hero_uv(
        &self,
        record: &HeroUvDurableRecord,
        layout: &HeroUvDurableCasPayload,
        link: &HeroUvDurableCasPayload,
    ) -> Result<(HeroUvDurableRecord, bool), String> {
        let store_record = store_record(record)?;
        let store_layout = store_payload(layout);
        let store_link = store_payload(link);
        let reservation = self.runtime.store.begin_cas_reservation();
        let created_at = now_string();
        let mut objects = Vec::new();
        let result = (|| -> Result<(StoreHeroUvDurableRecord, bool), String> {
            let layout_object = self
                .runtime
                .store
                .put_object_reserved(
                    &reservation,
                    &store_layout.bytes,
                    Some(&store_layout.object_sha256),
                    &store_layout.mime,
                    &store_layout.kind,
                    &created_at,
                )
                .map_err(|error| error.to_string())?;
            objects.push(layout_object);
            let link_object = self
                .runtime
                .store
                .put_object_reserved(
                    &reservation,
                    &store_link.bytes,
                    Some(&store_link.object_sha256),
                    &store_link.mime,
                    &store_link.kind,
                    &created_at,
                )
                .map_err(|error| error.to_string())?;
            objects.push(link_object);
            self.runtime
                .store
                .commit_hero_uv(&store_record, &store_layout, &store_link)
                .map_err(|error| error.to_string())
        })();
        let cleanup = result.is_err();
        release_reserved(self.runtime, &reservation, &objects, cleanup);
        result
            .and_then(|(record, replayed)| runtime_record(record).map(|record| (record, replayed)))
    }

    fn read_hero_uv_bundle(
        &self,
        record: &HeroUvDurableRecord,
    ) -> Result<HeroUvDurableReadback, String> {
        let record = store_record(record)?;
        self.runtime
            .store
            .read_hero_uv_bundle(&record)
            .map(runtime_readback)
            .map_err(|error| error.to_string())
    }
}

fn resolve_candidate_bound_low_source(
    runtime: &Runtime,
    request: &HeroUvDurablePrepareRequest,
) -> Result<HeroUvSourceInput, RuntimeError> {
    let candidate = runtime
        .candidate(&request.candidate_id)?
        .ok_or_else(|| RuntimeError::InvalidInput("candidate is unavailable".to_owned()))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.candidate_state_sha256
        || candidate.base_version_id != request.base_version_id
    {
        return Err(RuntimeError::InvalidInput(
            "candidate project/state/base-version binding differs".to_owned(),
        ));
    }
    if request.source_low_artifact_id != request.source_low_artifact_object_sha256 {
        return Err(RuntimeError::InvalidInput(
            "source_low_artifact_id must equal the Low artifact CAS object hash".to_owned(),
        ));
    }
    let low_record = runtime
        .store
        .get_low_quad_draft_durable_by_candidate_artifact(
            &request.candidate_id,
            &request.source_low_artifact_sha256,
        )?
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "candidate-bound Low durable provenance is unavailable".to_owned(),
            )
        })?;
    if low_record.project_id != request.project_id
        || low_record.candidate_id != request.candidate_id
        || low_record.candidate_state_sha256 != request.candidate_state_sha256
        || low_record.base_version_id != request.base_version_id
        || low_record.artifact_object_sha256 != request.source_low_artifact_object_sha256
        || low_record.artifact_sha256 != request.source_low_artifact_sha256
        || low_record.readback_object_sha256 != request.source_low_artifact_readback_object_sha256
        || low_record.readback_sha256 != request.source_low_artifact_readback_sha256
    {
        return Err(RuntimeError::InvalidInput(
            "candidate-bound Low durable provenance differs".to_owned(),
        ));
    }
    let artifact_object = runtime
        .store
        .get_object(&request.source_low_artifact_object_sha256)?
        .ok_or_else(|| {
            RuntimeError::InvalidInput("source Low artifact is unavailable".to_owned())
        })?;
    if artifact_object.mime != GLB_MIME
        || artifact_object.kind != LOW_ARTIFACT_CAS_KIND
        || artifact_object.size_bytes == 0
        || artifact_object.size_bytes > MAX_GLB_BYTES as u64
    {
        return Err(RuntimeError::InvalidInput(
            "source Low artifact CAS metadata differs".to_owned(),
        ));
    }
    let glb_bytes = runtime.cas_read_bounded(
        &request.source_low_artifact_object_sha256,
        (MAX_GLB_BYTES as u64).min(MAX_GEOMETRY_ARTIFACT_BYTES),
    )?;
    if sha256_hex(&glb_bytes) != request.source_low_artifact_object_sha256
        || sha256_hex(&glb_bytes) != request.source_low_artifact_sha256
    {
        return Err(RuntimeError::InvalidInput(
            "source Low artifact CAS hash differs".to_owned(),
        ));
    }
    if !super::strict_glb_inspection(&glb_bytes)?.hard_gate_passed {
        return Err(RuntimeError::InvalidInput(
            "source Low artifact strict readback failed".to_owned(),
        ));
    }
    let readback_object = runtime
        .store
        .get_object(&request.source_low_artifact_readback_object_sha256)?
        .ok_or_else(|| {
            RuntimeError::InvalidInput("source Low readback is unavailable".to_owned())
        })?;
    if readback_object.mime != JSON_MIME
        || readback_object.kind != LOW_READBACK_CAS_KIND
        || readback_object.size_bytes == 0
        || readback_object.size_bytes > MAX_JSON_BYTES as u64
    {
        return Err(RuntimeError::InvalidInput(
            "source Low readback CAS metadata differs".to_owned(),
        ));
    }
    let readback_bytes = runtime.cas_read_bounded(
        &request.source_low_artifact_readback_object_sha256,
        MAX_JSON_BYTES as u64,
    )?;
    if sha256_hex(&readback_bytes) != request.source_low_artifact_readback_object_sha256 {
        return Err(RuntimeError::InvalidInput(
            "source Low readback CAS hash differs".to_owned(),
        ));
    }
    Ok(HeroUvSourceInput {
        binding: binding_from_prepare(request),
        glb_bytes,
        glb_mime: artifact_object.mime,
        glb_kind: artifact_object.kind,
        readback_bytes,
        readback_mime: readback_object.mime,
        readback_kind: readback_object.kind,
    })
}

fn run_hero_worker(payload: &Value) -> Result<(Value, String), String> {
    let worker = geometry_worker::production_weapon_hero_uv_layout(payload.clone())
        .map_err(|error| error.to_string())?;
    let cohort = worker
        .build_cohort_sha256
        .ok_or_else(|| "Hero UV Worker build cohort is unavailable".to_owned())?;
    Ok((worker.result, cohort))
}

fn restart_revalidate(runtime: &Runtime, record: &HeroUvDurableRecord) -> Result<(), RuntimeError> {
    let persistence = StoreHeroUvPersistence { runtime };
    let bundle = persistence
        .read_hero_uv_bundle(record)
        .map_err(|error| RuntimeError::InvalidInput(error.to_owned()))?;
    let layout: Value = serde_json::from_slice(&bundle.layout_bytes).map_err(|error| {
        RuntimeError::InvalidInput(format!("stored Hero UV layout JSON is invalid: {error}"))
    })?;
    let visibility_weights = layout.get("visibility_weights").cloned().ok_or_else(|| {
        RuntimeError::InvalidInput("stored Hero UV visibility weights are missing".to_owned())
    })?;
    let request_value = json!({
        "schema_version": HERO_UV_DURABLE_PREPARE_SCHEMA,
        "project_id": record.project_id,
        "candidate_id": record.candidate_id,
        "candidate_state_sha256": record.candidate_state_sha256,
        "base_version_id": record.base_version_id,
        "source_low_artifact_id": record.source_low_artifact_id,
        "source_low_artifact_object_sha256": record.source_low_artifact_object_sha256,
        "source_low_artifact_sha256": record.source_low_artifact_sha256,
        "source_low_artifact_readback_object_sha256": record.source_low_artifact_readback_object_sha256,
        "source_low_artifact_readback_sha256": record.source_low_artifact_readback_sha256,
        "resolution": record.resolution,
        "padding_texels": record.padding_texels,
        "min_mip_level": record.min_mip_level,
        "hard_edge_angle_deg": record.hard_edge_angle_deg,
        "stretch_threshold": record.stretch_threshold,
        "visibility_weights": visibility_weights,
        "idempotency_key": record.idempotency_key,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "source_only": true,
        "runtime_write_performed": false,
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "input_sha256": record.input_sha256
    });
    let request = parse_prepare(request_value)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let source = resolve_candidate_bound_low_source(runtime, &request)?;
    validate_source(&request, &source)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let mut runner = run_hero_worker;
    let (replayed_layout, worker_cohort, _) =
        worker_replay(&request, &source.glb_bytes, &mut runner)
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    if replayed_layout != layout || worker_cohort != record.worker_build_cohort_sha256 {
        return Err(RuntimeError::InvalidInput(
            "stored Hero UV Worker replay differs after Runtime restart".to_owned(),
        ));
    }
    Ok(())
}

impl Runtime {
    pub fn hero_uv_durable_prepare(&self, value: Value) -> Result<Value, RuntimeError> {
        let request = parse_prepare(value.clone())
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let source = resolve_candidate_bound_low_source(self, &request)?;
        if let Some(existing) = self
            .store
            .get_hero_uv(&request.project_id, &request.idempotency_key)?
        {
            let existing = runtime_record(existing)
                .map_err(|error| RuntimeError::InvalidInput(error.to_owned()))?;
            if existing.input_sha256 != request.input_sha256 {
                return Err(RuntimeError::InvalidInput(
                    "idempotency key is bound to another input".to_owned(),
                ));
            }
            restart_revalidate(self, &existing)?;
        }
        let persistence = StoreHeroUvPersistence { runtime: self };
        prepare(value, source, run_hero_worker, &persistence, &now_string())
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
    }

    pub fn hero_uv_durable_get(&self, value: Value) -> Result<Value, RuntimeError> {
        let request = parse_get(value.clone())
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
        let record = self
            .store
            .get_hero_uv(&request.project_id, &request.idempotency_key)?
            .ok_or_else(|| {
                RuntimeError::InvalidInput("Hero UV durable record is unavailable".to_owned())
            })?;
        let record =
            runtime_record(record).map_err(|error| RuntimeError::InvalidInput(error.to_owned()))?;
        restart_revalidate(self, &record)?;
        let persistence = StoreHeroUvPersistence { runtime: self };
        get(value, &persistence).map_err(|error| RuntimeError::InvalidInput(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{
        LOW_QUAD_DRAFT_DURABLE_CANONICALIZATION_POLICY, LOW_QUAD_DRAFT_DURABLE_MAX_RESPONSE_BYTES,
        LOW_QUAD_DRAFT_DURABLE_PREPARE_SCHEMA_VERSION, LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY,
    };
    use forgecad_worker_protocol::{
        PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM, PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY,
        PRODUCTION_WEAPON_LOW_QUAD_DRAFT_REQUEST_SCHEMA_VERSION,
    };
    use std::cell::RefCell;
    use std::fs;
    use uuid::Uuid;

    fn authoring_program(project_id: &str) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project_id,
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":crate::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":1,
                "max_triangles":32,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"hero-uv-restart-panel",
                "operator_id":"forgecad.geometry.authoring-mesh@1",
                "inputs":[],
                "parameters":{
                    "shape":"authoring-mesh",
                    "topology_policy":"triangle-quad-manifold-with-boundary@1",
                    "vertices":[
                        {"element_id":"v0","position_m":[-1.0,-1.0,0.0]},
                        {"element_id":"v1","position_m":[1.0,-1.0,0.0]},
                        {"element_id":"v2","position_m":[1.0,1.0,0.0]},
                        {"element_id":"v3","position_m":[-1.0,1.0,0.0]}
                    ],
                    "edges":[
                        {"element_id":"e01","vertex_ids":["v0","v1"]},
                        {"element_id":"e03","vertex_ids":["v0","v3"]},
                        {"element_id":"e12","vertex_ids":["v1","v2"]},
                        {"element_id":"e23","vertex_ids":["v2","v3"]}
                    ],
                    "loops":[
                        {"element_id":"l0","face_id":"f0","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
                        {"element_id":"l1","face_id":"f0","ordinal":1,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
                        {"element_id":"l2","face_id":"f0","ordinal":2,"vertex_id":"v2","edge_id":"e23","edge_forward":true},
                        {"element_id":"l3","face_id":"f0","ordinal":3,"vertex_id":"v3","edge_id":"e03","edge_forward":false}
                    ],
                    "faces":[{"element_id":"f0","loop_ids":["l0","l1","l2","l3"]}],
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"hero-uv-restart-panel",
                "input_node_ids":["hero-uv-restart-panel"],
                "material_zone_id":"zone-hero-uv-shell",
                "solid":false
            }]
        });
        let hash = crate::hash_geometry_program_with_runtime_worker(&program)
            .expect("GeometryProgram hash");
        program["canonical_sha256"] = hash["canonical_sha256"].clone();
        program
    }

    fn expected_canonical_mesh(
        projection: &Value,
        project_id: &str,
        candidate_id: &str,
        candidate_state_sha256: &str,
        base_version_id: Value,
        source_program_object_sha256: &str,
        source_program_sha256: &str,
        source_artifact_object_sha256: &str,
        source_artifact_sha256: &str,
        source_artifact_readback_object_sha256: &str,
        source_artifact_readback_sha256: &str,
        source_lineage_sha256: &str,
    ) -> Value {
        let canonical_mesh_id = projection["mesh_id"].as_str().expect("projection mesh id");
        let mesh_sha256 = projection["mesh_sha256"]
            .as_str()
            .expect("projection mesh sha");
        let original_id = projection["original_identity"]["identity_id"]
            .as_str()
            .expect("projection original identity");
        let evaluated_id = projection["evaluated_identity"]["identity_id"]
            .as_str()
            .expect("projection evaluated identity");
        let mut canonical = json!({
            "schema_version":"AuthoringMeshCanonical@1",
            "canonical_mesh_id":canonical_mesh_id,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "base_version_id":base_version_id,
            "authoring_node_id":"hero-uv-restart-panel",
            "part_id":"hero-uv-restart-panel",
            "source_program_object_sha256":source_program_object_sha256,
            "source_program_sha256":source_program_sha256,
            "source_artifact_object_sha256":source_artifact_object_sha256,
            "source_artifact_sha256":source_artifact_sha256,
            "source_artifact_readback_object_sha256":source_artifact_readback_object_sha256,
            "source_artifact_readback_sha256":source_artifact_readback_sha256,
            "source_lineage_sha256":source_lineage_sha256,
            "representation":"runtime-owned-original-half-edge@1",
            "storage_policy":"runtime-owned-sqlite-cas-canonical-authoring-mesh@1",
            "writer_policy":"forgecad-runtime-only-state-writer@1",
            "original_identity":{
                "identity_id":original_id,
                "namespace":"original",
                "identity_kind":"runtime-owned-original-authoring@1",
                "element_id_policy":"lineage-scoped-opaque-not-cross-version-stable@1",
                "topology_sha256":mesh_sha256,
                "source_lineage_sha256":source_lineage_sha256,
                "stability_scope":"same-canonical-mesh-lineage-only@1"
            },
            "evaluated_identity":{
                "identity_id":evaluated_id,
                "namespace":"evaluated",
                "identity_kind":"runtime-derived-evaluated-artifact-readback@1",
                "element_id_policy":"artifact-local-no-authoring-bijection@1",
                "correspondence_policy":"non-bijective-derived-only@1",
                "artifact_object_sha256":source_artifact_object_sha256,
                "artifact_readback_sha256":source_artifact_readback_sha256,
                "source_lineage_sha256":source_lineage_sha256,
                "cross_version_stable":false
            },
            "cross_version_stable":false,
            "cross_version_stability":{
                "status":"not-proven@1",
                "scope":"same-canonical-mesh-lineage-only@1",
                "stable_id_claim":"none-across-revisions@1",
                "deleted_id_reuse_policy":"not-proven-and-not-a-contract@1",
                "new_id_policy":"lineage-operation-parent-derived-draft-only@1",
                "evaluated_id_policy":"artifact-local-unstable-derived-only@1"
            },
            "counts":projection["counts"],
            "vertices":projection["vertices"],
            "edges":projection["edges"],
            "half_edges":projection["half_edges"],
            "corners":projection["corners"],
            "faces":projection["faces"],
            "loops":projection["loops"],
            "rings":projection["rings"],
            "topology":projection["topology"],
            "canonicalization_policy":"canonical-json-sha256-excluding-canonical-sha256@1",
            "runtime_write_performed":true,
            "persistent_user_data_touched":true,
            "stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "quality_status":"structural_only",
            "canonical_sha256":""
        });
        canonical["canonical_sha256"] = Value::String(canonical_json_hash(&canonical));
        canonical
    }

    fn high_request(
        canonical_mesh: Value,
        candidate_id: &str,
        candidate_state_sha256: &str,
    ) -> Value {
        let source_mesh_sha256 = canonical_mesh["canonical_sha256"]
            .as_str()
            .expect("canonical mesh hash")
            .to_owned();
        let source_authoring_mesh = json!({
            "schema_version":"HighWorkerAuthoringMeshAdapter@1",
            "canonical_mesh":canonical_mesh,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "head_candidate_id":candidate_id,
            "head_candidate_state_sha256":candidate_state_sha256,
            "source_mesh_sha256":source_mesh_sha256
        });
        let detail_graph = json!({
            "schema_version":"DetailGraph@1",
            "nodes":[{
                "node_id":"hero-uv-floating-detail",
                "kind":"floating_detail",
                "parent_part_id":"hero-uv-restart-panel",
                "parent_node_id":null,
                "source_edge":null,
                "width_m":null,
                "count":null,
                "sharpness":null,
                "center_m":[0.0,0.0,2.0],
                "size_m":[1.0,1.0,1.0]
            }]
        });
        let mut request = json!({
            "schema_version":"HighMeshWorkerRequest@1",
            "operation":"forgecad.production.high-mesh-prepare@1",
            "source_authoring_mesh":source_authoring_mesh,
            "source_authoring_mesh_sha256":"",
            "detail_graph":detail_graph,
            "detail_graph_canonical_sha256":"",
            "budgets":{
                "max_detail_nodes":16,
                "max_output_vertices":1024,
                "max_output_triangles":2048
            },
            "canonical_sha256":""
        });
        request["source_authoring_mesh_sha256"] =
            Value::String(canonical_json_hash(&request["source_authoring_mesh"]));
        request["detail_graph_canonical_sha256"] =
            Value::String(canonical_json_hash(&request["detail_graph"]));
        request["canonical_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn native_prepare_request(
        project_id: &str,
        candidate_id: &str,
        candidate_state_sha256: &str,
        base_version_id: Value,
        source_mesh_id: &str,
        source_mesh_object_sha256: &str,
        source_mesh_sha256: &str,
        high_mesh_request: Value,
        idempotency_key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version":"NativeHighDurablePrepareRequest@1",
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "base_version_id":base_version_id,
            "source_authoring_mesh_id":source_mesh_id,
            "source_authoring_mesh_object_sha256":source_mesh_object_sha256,
            "source_authoring_mesh_sha256":source_mesh_sha256,
            "high_mesh_request":high_mesh_request,
            "high_mesh_request_sha256":"",
            "idempotency_key":idempotency_key,
            "max_response_bytes":1048576,
            "source_only":true,
            "runtime_write_performed":false,
            "writer_policy":"forgecad-runtime-only-state-writer@1",
            "canonicalization_policy":"canonical-json-sha256-excluding-canonical-sha256@1",
            "input_sha256":""
        });
        request["high_mesh_request_sha256"] =
            request["high_mesh_request"]["canonical_sha256"].clone();
        request["input_sha256"] = Value::String({
            let mut preimage = request.clone();
            let object = preimage
                .as_object_mut()
                .expect("Native High request object");
            object.remove("input_sha256");
            object.remove("idempotency_key");
            canonical_json_hash(&preimage)
        });
        request
    }

    fn low_quad_worker_request(
        project_id: &str,
        source_high_artifact_sha256: &str,
        source_high_artifact_readback_sha256: &str,
    ) -> Value {
        let vertices = vec![
            ("v0", [-1.0, -1.0, -1.0]),
            ("v1", [1.0, -1.0, -1.0]),
            ("v2", [1.0, 1.0, -1.0]),
            ("v3", [-1.0, 1.0, -1.0]),
            ("v4", [-1.0, -1.0, 1.0]),
            ("v5", [1.0, -1.0, 1.0]),
            ("v6", [1.0, 1.0, 1.0]),
            ("v7", [-1.0, 1.0, 1.0]),
        ];
        let faces = vec![
            ("f0", vec!["v0", "v3", "v2", "v1"]),
            ("f1", vec!["v4", "v5", "v6", "v7"]),
            ("f2", vec!["v0", "v4", "v7", "v3"]),
            ("f3", vec!["v1", "v2", "v6", "v5"]),
            ("f4", vec!["v0", "v1", "v5", "v4"]),
            ("f5", vec!["v3", "v7", "v6", "v2"]),
        ];
        let mut edge_ids = BTreeSet::<(String, String)>::new();
        for (_, face) in &faces {
            for index in 0..face.len() {
                let first = face[index].to_owned();
                let second = face[(index + 1) % face.len()].to_owned();
                edge_ids.insert(if first < second {
                    (first, second)
                } else {
                    (second, first)
                });
            }
        }
        let edges = edge_ids
            .iter()
            .map(|(first, second)| {
                json!({
                    "element_id":format!("e-{first}-{second}"),
                    "vertex_ids":[first,second]
                })
            })
            .collect::<Vec<_>>();
        let mut loops = Vec::new();
        let mut face_values = Vec::new();
        for (face_id, face) in &faces {
            let mut loop_ids = Vec::new();
            for ordinal in 0..face.len() {
                let first = face[ordinal];
                let second = face[(ordinal + 1) % face.len()];
                let (left, right) = if first < second {
                    (first, second)
                } else {
                    (second, first)
                };
                let loop_id = format!("l-{face_id}-{ordinal}");
                loops.push(json!({
                    "element_id":loop_id,
                    "face_id":face_id,
                    "ordinal":ordinal,
                    "vertex_id":first,
                    "edge_id":format!("e-{left}-{right}"),
                    "edge_forward":first == left
                }));
                loop_ids.push(Value::String(loop_id));
            }
            face_values.push(json!({"element_id":face_id,"loop_ids":loop_ids}));
        }
        let authoring_mesh = json!({
            "shape":"authoring-mesh",
            "topology_policy":"triangle-quad-manifold-with-boundary@1",
            "vertices":vertices
                .iter()
                .map(|(id, position)| json!({"element_id":id,"position_m":position}))
                .collect::<Vec<_>>(),
            "edges":edges,
            "loops":loops,
            "faces":face_values,
            "position_m":[0.0,0.0,0.0],
            "rotation_rad":[0.0,0.0,0.0]
        });
        let source_lineage = json!({
            "source_high_artifact_sha256":source_high_artifact_sha256,
            "source_high_artifact_readback_sha256":source_high_artifact_readback_sha256,
            "source_high_part_id":"hero-uv-restart-panel",
            "source_high_node_id":"hero-uv-restart-panel",
            "source_high_material_zone_id":"zone-hero-uv-shell"
        });
        let mut request = json!({
            "schema_version":PRODUCTION_WEAPON_LOW_QUAD_DRAFT_REQUEST_SCHEMA_VERSION,
            "preview_only":true,
            "project_id":project_id,
            "source_high_artifact_sha256":source_high_artifact_sha256,
            "source_high_artifact_readback_sha256":source_high_artifact_readback_sha256,
            "source_high_part_id":"hero-uv-restart-panel",
            "source_high_node_id":"hero-uv-restart-panel",
            "source_high_material_zone_id":"zone-hero-uv-shell",
            "draft":{
                "schema_version":"LowQuadRetopologyDraft@1",
                "source_lineage":source_lineage,
                "authoring_mesh":authoring_mesh
            },
            "max_vertices":128,
            "max_edges":128,
            "max_faces":64,
            "low_retopology_policy":PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY,
            "algorithm":PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM,
            "canonical_sha256":""
        });
        request["canonical_sha256"] = Value::String({
            let mut preimage = request.clone();
            preimage
                .as_object_mut()
                .expect("Low quad Worker request object")
                .remove("canonical_sha256");
            canonical_json_hash(&preimage)
        });
        request
    }

    fn low_prepare_request(
        project_id: &str,
        candidate_id: &str,
        candidate_state_sha256: &str,
        base_version_id: Value,
        source_high_artifact_id: &str,
        source_high_artifact_object_sha256: &str,
        source_high_artifact_sha256: &str,
        source_high_artifact_readback_object_sha256: &str,
        source_high_artifact_readback_sha256: &str,
        worker_request: Value,
        idempotency_key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version":LOW_QUAD_DRAFT_DURABLE_PREPARE_SCHEMA_VERSION,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "base_version_id":base_version_id,
            "source_high_artifact_id":source_high_artifact_id,
            "source_high_artifact_object_sha256":source_high_artifact_object_sha256,
            "source_high_artifact_sha256":source_high_artifact_sha256,
            "source_high_artifact_readback_object_sha256":source_high_artifact_readback_object_sha256,
            "source_high_artifact_readback_sha256":source_high_artifact_readback_sha256,
            "low_quad_draft_worker_request":worker_request,
            "low_quad_draft_worker_request_sha256":"",
            "idempotency_key":idempotency_key,
            "max_response_bytes":LOW_QUAD_DRAFT_DURABLE_MAX_RESPONSE_BYTES,
            "source_only":true,
            "runtime_write_performed":false,
            "writer_policy":LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY,
            "canonicalization_policy":LOW_QUAD_DRAFT_DURABLE_CANONICALIZATION_POLICY,
            "input_sha256":""
        });
        request["low_quad_draft_worker_request_sha256"] =
            request["low_quad_draft_worker_request"]["canonical_sha256"].clone();
        request["input_sha256"] = Value::String({
            let mut preimage = request.clone();
            let object = preimage
                .as_object_mut()
                .expect("Low durable request object");
            object.remove("input_sha256");
            object.remove("idempotency_key");
            canonical_json_hash(&preimage)
        });
        request
    }

    fn hero_prepare_request(first: &Value, low_request: &Value) -> Value {
        let mut request = json!({
            "schema_version":HERO_UV_DURABLE_PREPARE_SCHEMA,
            "project_id":low_request["project_id"],
            "candidate_id":low_request["candidate_id"],
            "candidate_state_sha256":low_request["candidate_state_sha256"],
            "base_version_id":low_request["base_version_id"],
            "source_low_artifact_id":first["artifact_object_sha256"],
            "source_low_artifact_object_sha256":first["artifact_object_sha256"],
            "source_low_artifact_sha256":first["artifact_sha256"],
            "source_low_artifact_readback_object_sha256":first["readback_object_sha256"],
            "source_low_artifact_readback_sha256":first["readback_sha256"],
            "resolution":2048,
            "padding_texels":8,
            "min_mip_level":3,
            "hard_edge_angle_deg":45.0,
            "stretch_threshold":4.0,
            "visibility_weights":[{"part_id":"hero-uv-restart-panel","first_person":1.0,"world":0.5,"hidden":0.1}],
            "idempotency_key":"hero-uv-durable-once",
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "source_only":true,
            "runtime_write_performed":false,
            "writer_policy":WRITER_POLICY,
            "canonicalization_policy":CANONICALIZATION_POLICY,
            "input_sha256":""
        });
        request["input_sha256"] =
            Value::String(request_input_hash(&request).expect("Hero UV input hash"));
        request
    }

    fn hero_get_request(first: &Value, request: &Value) -> Value {
        let mut get = json!({
            "schema_version":HERO_UV_DURABLE_GET_SCHEMA,
            "operation":HERO_UV_DURABLE_GET_OPERATION,
            "project_id":request["project_id"],
            "candidate_id":request["candidate_id"],
            "candidate_state_sha256":request["candidate_state_sha256"],
            "base_version_id":request["base_version_id"],
            "source_low_artifact_id":first["source_low_artifact_id"],
            "source_low_artifact_sha256":first["source_low_artifact_sha256"],
            "layout_object_sha256":first["layout_object_sha256"],
            "layout_canonical_sha256":first["layout_canonical_sha256"],
            "link_id":first["link_id"],
            "link_object_sha256":first["link_object_sha256"],
            "resolution":first["resolution"],
            "padding_texels":first["padding_texels"],
            "min_mip_level":first["min_mip_level"],
            "hard_edge_angle_deg":first["hard_edge_angle_deg"],
            "stretch_threshold":first["stretch_threshold"],
            "visibility_weights_sha256":first["visibility_weights_sha256"],
            "idempotency_key":first["idempotency_key"],
            "source_only":true,
            "writer_policy":WRITER_POLICY,
            "runtime_write_performed":false,
            "persistent_user_data_touched":false,
            "input_sha256":""
        });
        get["input_sha256"] = Value::String({
            let mut preimage = get.clone();
            preimage["input_sha256"] = Value::String(String::new());
            canonical_json_hash(&preimage)
        });
        get
    }

    fn request_value() -> Value {
        let mut value = json!({
            "schema_version": HERO_UV_DURABLE_PREPARE_SCHEMA, "project_id": "project-test", "candidate_id": "candidate-test", "candidate_state_sha256": "a".repeat(64), "base_version_id": Value::Null,
            "source_low_artifact_id": "b".repeat(64), "source_low_artifact_object_sha256": "b".repeat(64), "source_low_artifact_sha256": "b".repeat(64), "source_low_artifact_readback_object_sha256": "c".repeat(64), "source_low_artifact_readback_sha256": "d".repeat(64),
            "resolution": 2048, "padding_texels": 8, "min_mip_level": 3, "hard_edge_angle_deg": 45.0, "stretch_threshold": 4.0,
            "visibility_weights": [{"part_id":"receiver", "first_person":1.0, "world":0.5, "hidden":0.1}], "idempotency_key":"hero-uv-test", "max_response_bytes": MAX_RESPONSE_BYTES, "source_only": true, "runtime_write_performed": false, "writer_policy": WRITER_POLICY, "canonicalization_policy": CANONICALIZATION_POLICY, "input_sha256":""
        });
        let hash = request_input_hash(&value).expect("request hash");
        value["input_sha256"] = Value::String(hash);
        value
    }

    fn source(request: &Value, glb: &[u8], readback: &[u8]) -> HeroUvSourceInput {
        let req: HeroUvDurablePrepareRequest =
            serde_json::from_value(request.clone()).expect("typed request");
        HeroUvSourceInput {
            binding: binding_from_prepare(&req),
            glb_bytes: glb.to_vec(),
            glb_mime: GLB_MIME.to_owned(),
            glb_kind: LOW_ARTIFACT_CAS_KIND.to_owned(),
            readback_bytes: readback.to_vec(),
            readback_mime: JSON_MIME.to_owned(),
            readback_kind: LOW_READBACK_CAS_KIND.to_owned(),
        }
    }

    fn fake_layout(request: &Value) -> Value {
        let req: HeroUvDurablePrepareRequest =
            serde_json::from_value(request.clone()).expect("typed request");
        let mut value = json!({
            "schema_version": HERO_UV_LAYOUT_SCHEMA, "operation": HERO_UV_LAYOUT_OPERATION, "policy": HERO_UV_LAYOUT_POLICY, "policy_sha256": sha256_hex(HERO_UV_LAYOUT_POLICY.as_bytes()), "low_artifact_sha256": req.source_low_artifact_sha256, "resolution": req.resolution, "uv0_semantic":"game-material-hero-channel@1", "uv1_semantic":"lightmap-bake-channel@1", "visibility_weight_policy":"first-person-world-hidden-per-part@1", "mip_padding_policy":"base-padding-at-least-2^min-mip-level@1", "seam_policy":"uv-seam-or-material-boundary-and-hard-edge-congruence@1", "hard_edge_policy":"face-normal-angle-threshold@1", "uv0_corners":[], "uv1_corners":[], "visibility_weights":req.visibility_weights, "islands":[], "metrics":{"triangle_count":1,"uv0_island_count":1,"uv1_chart_count":1,"uv0_overlap_count":0,"uv1_overlap_count":0,"uv0_out_of_bounds_triangle_count":0,"uv1_out_of_bounds_triangle_count":0,"uv0_zero_area_triangle_count":0,"uv0_inverted_triangle_count":0,"stretch_exceeded_triangle_count":0,"max_stretch_ratio":1.0,"first_person_weighted_texel_density":1.0,"all_surface_texel_density":1.0,"boundary_edge_count":0,"non_manifold_edge_count":0,"hard_edge_count":0,"uv_seam_count":0,"material_boundary_count":0,"hard_edge_without_seam_count":0,"seam_hard_edge_congruence":true,"padding_texels":req.padding_texels,"required_mip_padding_texels":1u64<<req.min_mip_level,"mip_padding_passed":true,"first_person_weighting_applied":true,"uv0_structural_gate":true,"uv1_structural_gate":true}, "mikk_replay":{"algorithm":"MikkTSpace@0.3.0","status":"PASS_SOURCE_STRUCTURAL","triangle_corner_count":3,"non_finite_count":0,"input_frame_mismatch_count":0,"tangent_semantics":"UV0-derived-tangent-input-replay@1","normal_convention":"OpenGL+Y"}, "source_only":true,"quality_status":"structural_only","structural_status":"PASS_SOURCE_STRUCTURAL","visual_status":"NOT_PROVEN","human_status":"NOT_RUN","engine_status":"NOT_RUN","distribution_status":"NOT_RUN","runtime_write_performed":false,"production_stage_advanced":false,"candidate_confirmed":false,"version_created":false,"export_performed":false,"promotion_eligible":false,"canonical_sha256":""
        });
        value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
        value
    }

    #[test]
    fn prepare_request_is_closed_and_hash_bound() {
        let value = request_value();
        assert!(parse_prepare(value.clone()).is_ok());
        let mut extra = value;
        extra["unexpected"] = Value::Bool(true);
        assert!(parse_prepare(extra).is_err());
    }

    #[test]
    fn worker_request_preserves_2k_4k_and_source_hash() {
        let value = request_value();
        let request = parse_prepare(value).expect("request");
        let payload = build_worker_request(&request, b"glb").expect("worker request");
        assert_eq!(payload["schema_version"], HERO_UV_LAYOUT_REQUEST_SCHEMA);
        assert_eq!(
            payload["low_artifact_sha256"],
            request.source_low_artifact_sha256
        );
        assert!(payload["low_glb_base64"].is_string());
    }

    #[test]
    fn worker_request_canonical_hash_excludes_the_hash_field() {
        let value = request_value();
        let request = parse_prepare(value).expect("request");
        let payload = build_worker_request(&request, b"glb").expect("worker request");
        let supplied = payload["canonical_sha256"]
            .as_str()
            .expect("request hash")
            .to_owned();
        let mut preimage = payload;
        preimage
            .as_object_mut()
            .expect("worker request object")
            .remove("canonical_sha256");
        assert_eq!(canonical_json_hash(&preimage), supplied);
    }

    #[test]
    fn replay_rejects_non_deterministic_worker() {
        let value = request_value();
        let request = parse_prepare(value).expect("request");
        let mut count = 0u8;
        let mut runner = |payload: &Value| {
            let _ = payload;
            count += 1;
            let mut output = fake_layout(&request_value());
            output["canonical_sha256"] = Value::String(canonical_json_hash(&output));
            if count == 2 {
                output["resolution"] = Value::from(4096u64);
                output["canonical_sha256"] = Value::String(canonical_json_hash(&output));
            }
            Ok((output, "e".repeat(64)))
        };
        let result = worker_replay(&request, b"glb", &mut runner);
        assert!(result.is_err());
    }

    #[test]
    fn hero_uv_durable_prepare_replay_drop_reopen_get_preserves_low_lineage_exactly() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-hero-uv-durable-restart-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("restart root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");

        let (
            hero_request,
            hero_first,
            candidate_json,
            candidates_before,
            versions_before,
            project_id,
        ) = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("initial Runtime");
            let project = runtime
                .create_project("Hero UV durable restart", json!({"profile":"test"}))
                .expect("project");
            let prepared = runtime
                .prepare_geometry_candidate(
                    &project.project_id,
                    None,
                    json!({
                        "typed":"geometry",
                        "geometry_program":authoring_program(&project.project_id)
                    }),
                )
                .expect("source GeometryProgram candidate");
            let candidate_id = prepared["candidate"]["candidate_id"]
                .as_str()
                .expect("candidate id")
                .to_owned();
            let candidate = runtime
                .candidate(&candidate_id)
                .expect("candidate query")
                .expect("candidate");
            let candidate_json = serde_json::to_value(&candidate).expect("candidate JSON");
            let candidates_before = serde_json::to_value(
                runtime
                    .candidates(&project.project_id)
                    .expect("candidates before"),
            )
            .expect("candidates JSON");
            let versions_before = serde_json::to_value(
                runtime
                    .versions(Some(&project.project_id))
                    .expect("versions before"),
            )
            .expect("versions JSON");
            let evidence = runtime
                .store
                .get_geometry_candidate_evidence(&candidate_id)
                .expect("evidence query")
                .expect("geometry evidence");
            let source_artifact_id = candidate
                .prepared_object_id
                .clone()
                .expect("source artifact id");
            let source_artifact_object_sha256 = candidate
                .prepared_object_sha256
                .clone()
                .expect("source artifact object SHA");
            let readback = runtime
                .artifact_readback(&source_artifact_object_sha256, &candidate_id)
                .expect("source ArtifactReadback");
            let source_artifact_readback_sha256 = readback["canonical_sha256"]
                .as_str()
                .expect("source ArtifactReadback SHA")
                .to_owned();
            let projection_request = json!({
                "schema_version":"AuthoringMeshRequest@1",
                "project_id":project.project_id,
                "candidate_id":candidate_id,
                "artifact_id":source_artifact_object_sha256,
                "artifact_readback_sha256":source_artifact_readback_sha256,
                "program_sha256":evidence.geometry_program_sha256,
                "operator_catalog_sha256":evidence.operator_catalog_sha256,
                "readback_config_sha256":evidence.readback_config_sha256,
                "authoring_node_id":"hero-uv-restart-panel",
                "part_id":"hero-uv-restart-panel",
                "authoring_mesh_policy_sha256":"aa72cadabba90ddb43dd0014cfa434ab9b13f4e072b09258072f37334c72e709",
                "max_response_bytes":1048576
            });
            let projection = crate::authoring_mesh::get(&runtime, &projection_request)
                .expect("source AuthoringMesh projection");
            let source_lineage_sha256 = projection["lineage"]["lineage_sha256"]
                .as_str()
                .expect("source lineage SHA")
                .to_owned();
            let base_version_id = candidate
                .base_version_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null);
            let expected_canonical = expected_canonical_mesh(
                &projection,
                &project.project_id,
                &candidate_id,
                &candidate.canonical_sha256,
                base_version_id.clone(),
                &evidence.geometry_program_object_sha256,
                &evidence.geometry_program_sha256,
                &source_artifact_object_sha256,
                &source_artifact_object_sha256,
                &evidence.artifact_readback_object_sha256,
                &source_artifact_readback_sha256,
                &source_lineage_sha256,
            );
            let mut source_request = json!({
                "schema_version":"AuthoringMeshPrepareRequest@1",
                "project_id":project.project_id,
                "source_candidate_id":candidate_id,
                "source_candidate_state_sha256":candidate.canonical_sha256,
                "base_version_id":base_version_id,
                "authoring_node_id":"hero-uv-restart-panel",
                "part_id":"hero-uv-restart-panel",
                "source_program_object_sha256":evidence.geometry_program_object_sha256,
                "source_program_sha256":evidence.geometry_program_sha256,
                "source_artifact_id":source_artifact_id,
                "source_artifact_object_sha256":source_artifact_object_sha256,
                "source_artifact_sha256":source_artifact_object_sha256,
                "source_artifact_readback_object_sha256":evidence.artifact_readback_object_sha256,
                "source_artifact_readback_sha256":source_artifact_readback_sha256,
                "source_lineage_sha256":source_lineage_sha256,
                "expected_canonical_mesh_sha256":expected_canonical["canonical_sha256"],
                "idempotency_key":"hero-uv-authoring-mesh-once",
                "max_response_bytes":1048576,
                "runtime_write_performed":false,
                "writer_policy":"forgecad-runtime-only-state-writer@1",
                "canonicalization_policy":"canonical-json-sha256-excluding-canonical-sha256@1",
                "input_sha256":""
            });
            source_request["input_sha256"] = Value::String(canonical_json_hash(&source_request));
            let source = runtime
                .authoring_mesh_durable_prepare(&source_request)
                .expect("durable AuthoringMesh source");
            assert_eq!(source["canonical_mesh"], expected_canonical);

            let native_request = native_prepare_request(
                &project.project_id,
                &candidate_id,
                &candidate.canonical_sha256,
                source["base_version_id"].clone(),
                source["canonical_mesh_id"]
                    .as_str()
                    .expect("source mesh id"),
                source["canonical_mesh_object_sha256"]
                    .as_str()
                    .expect("source mesh object SHA"),
                source["canonical_mesh_sha256"]
                    .as_str()
                    .expect("source mesh SHA"),
                high_request(
                    source["canonical_mesh"].clone(),
                    &candidate_id,
                    &candidate.canonical_sha256,
                ),
                "hero-uv-native-high-once",
            );
            let high = runtime
                .native_high_durable_prepare(native_request)
                .expect("Native High source");
            assert_eq!(high["replayed"], false);
            assert_eq!(high["restart_hash_verified"], true);
            for field in [
                "production_stage_advanced",
                "candidate_confirmed",
                "version_created",
                "export_performed",
            ] {
                assert_eq!(high[field], false, "High {field}");
            }

            let source_high_artifact_id = high["artifact_id"].as_str().expect("High artifact id");
            let source_high_artifact_object_sha256 = high["glb_object_sha256"]
                .as_str()
                .expect("High GLB object SHA");
            let source_high_artifact_sha256 = high["glb_sha256"].as_str().expect("High GLB SHA");
            let source_high_artifact_readback_object_sha256 = high["glb_readback_object_sha256"]
                .as_str()
                .expect("High GLB readback object SHA");
            let source_high_artifact_readback_sha256 = high["glb_readback_sha256"]
                .as_str()
                .expect("High GLB readback SHA");
            let low_request = low_prepare_request(
                &project.project_id,
                &candidate_id,
                &candidate.canonical_sha256,
                candidate
                    .base_version_id
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
                source_high_artifact_id,
                source_high_artifact_object_sha256,
                source_high_artifact_sha256,
                source_high_artifact_readback_object_sha256,
                source_high_artifact_readback_sha256,
                low_quad_worker_request(
                    &project.project_id,
                    source_high_artifact_sha256,
                    source_high_artifact_readback_sha256,
                ),
                "hero-uv-low-quad-once",
            );
            let low_first = runtime
                .low_quad_draft_durable_prepare(low_request.clone())
                .expect("Low quad durable source");
            assert_eq!(low_first["replayed"], false);
            assert_eq!(low_first["restart_hash_verified"], true);
            for field in [
                "production_stage_advanced",
                "candidate_confirmed",
                "version_created",
                "export_performed",
            ] {
                assert_eq!(low_first[field], false, "Low {field}");
            }
            let hero_request = hero_prepare_request(&low_first, &low_request);
            let hero_first = runtime
                .hero_uv_durable_prepare(hero_request.clone())
                .expect("Hero UV durable prepare");
            assert_eq!(hero_first["replayed"], false);
            assert_eq!(hero_first["restart_hash_verified"], true);
            assert_eq!(hero_first["runtime_write_performed"], true);
            assert_eq!(hero_first["persistent_user_data_touched"], true);
            assert_eq!(hero_first["quality_status"], "structural_only");
            for field in [
                "production_stage_advanced",
                "candidate_confirmed",
                "version_created",
                "export_performed",
            ] {
                assert_eq!(hero_first[field], false, "Hero {field}");
            }
            for field in [
                "source_low_artifact_object_sha256",
                "source_low_artifact_sha256",
                "source_low_artifact_readback_object_sha256",
                "source_low_artifact_readback_sha256",
                "layout_object_sha256",
                "layout_canonical_sha256",
                "link_object_sha256",
                "request_sha256",
                "worker_build_cohort_sha256",
            ] {
                assert!(hero_first[field].as_str().is_some(), "Hero hash {field}");
            }
            let objects_after_prepare = runtime.store.cas().list_objects().expect("CAS after Hero");
            let hero_replay = runtime
                .hero_uv_durable_prepare(hero_request.clone())
                .expect("same-key Hero UV replay");
            assert_eq!(hero_replay["replayed"], true);
            assert_eq!(hero_replay["restart_hash_verified"], true);
            for field in [
                "source_low_artifact_object_sha256",
                "source_low_artifact_sha256",
                "source_low_artifact_readback_object_sha256",
                "source_low_artifact_readback_sha256",
                "layout_object_sha256",
                "layout_canonical_sha256",
                "link_id",
                "link_object_sha256",
                "request_sha256",
                "worker_build_cohort_sha256",
            ] {
                assert_eq!(hero_replay[field], hero_first[field], "Hero replay {field}");
            }
            assert_eq!(hero_replay["layout"], hero_first["layout"]);
            assert_eq!(hero_replay["durable_link"], hero_first["durable_link"]);
            assert_eq!(
                runtime
                    .store
                    .cas()
                    .list_objects()
                    .expect("CAS after replay"),
                objects_after_prepare
            );
            assert_eq!(
                serde_json::to_value(
                    runtime
                        .candidate(&candidate_id)
                        .expect("candidate after Hero")
                        .expect("candidate"),
                )
                .expect("candidate JSON after Hero"),
                candidate_json
            );
            assert_eq!(
                serde_json::to_value(
                    runtime
                        .candidates(&project.project_id)
                        .expect("candidates after Hero"),
                )
                .expect("candidates JSON after Hero"),
                candidates_before
            );
            assert_eq!(
                serde_json::to_value(
                    runtime
                        .versions(Some(&project.project_id))
                        .expect("versions after Hero"),
                )
                .expect("versions JSON after Hero"),
                versions_before
            );
            drop(runtime);
            (
                hero_request,
                hero_first,
                candidate_json,
                candidates_before,
                versions_before,
                project.project_id,
            )
        };

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopened Runtime");
        let hero_get = hero_get_request(&hero_first, &hero_request);
        let objects_before_get = reopened.store.cas().list_objects().expect("CAS before get");
        let hero_get_result = reopened
            .hero_uv_durable_get(hero_get)
            .expect("Hero UV durable get after restart");
        assert_eq!(hero_get_result["replayed"], true);
        assert_eq!(hero_get_result["restart_hash_verified"], true);
        assert_eq!(hero_get_result["runtime_write_performed"], false);
        assert_eq!(hero_get_result["persistent_user_data_touched"], false);
        for field in [
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "base_version_id",
            "source_low_artifact_id",
            "source_low_artifact_object_sha256",
            "source_low_artifact_sha256",
            "source_low_artifact_readback_object_sha256",
            "source_low_artifact_readback_sha256",
            "layout_object_sha256",
            "layout_canonical_sha256",
            "link_id",
            "link_object_sha256",
            "request_sha256",
            "request_input_sha256",
            "worker_build_cohort_sha256",
            "idempotency_key",
        ] {
            assert_eq!(
                hero_get_result[field], hero_first[field],
                "Hero get {field}"
            );
        }
        assert_eq!(hero_get_result["layout"], hero_first["layout"]);
        assert_eq!(hero_get_result["durable_link"], hero_first["durable_link"]);
        for field in [
            "production_stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ] {
            assert_eq!(hero_get_result[field], false, "Hero get {field}");
        }
        assert_eq!(
            reopened.store.cas().list_objects().expect("CAS after get"),
            objects_before_get
        );
        assert_eq!(
            serde_json::to_value(
                reopened
                    .candidate(hero_request["candidate_id"].as_str().expect("candidate id"))
                    .expect("candidate after restart")
                    .expect("candidate"),
            )
            .expect("candidate JSON after restart"),
            candidate_json
        );
        assert_eq!(
            serde_json::to_value(
                reopened
                    .candidates(&project_id)
                    .expect("candidates after restart"),
            )
            .expect("candidates JSON after restart"),
            candidates_before
        );
        assert_eq!(
            serde_json::to_value(
                reopened
                    .versions(Some(&project_id))
                    .expect("versions after restart"),
            )
            .expect("versions JSON after restart"),
            versions_before
        );
        let hero_record = reopened
            .store
            .get_hero_uv(&project_id, "hero-uv-durable-once")
            .expect("Hero durable record after restart")
            .expect("Hero durable record");
        assert_eq!(
            hero_record.layout_object_sha256,
            hero_first["layout_object_sha256"]
        );
        assert_eq!(
            hero_record.layout_canonical_sha256,
            hero_first["layout_canonical_sha256"]
        );
        assert_eq!(
            hero_record.link_object_sha256,
            hero_first["link_object_sha256"]
        );
        assert_eq!(
            hero_record.source_low_artifact_sha256,
            hero_first["source_low_artifact_sha256"]
        );
        drop(reopened);
        fs::remove_dir_all(root).expect("restart fixture cleanup");
    }

    #[derive(Default)]
    struct MemoryPersistence {
        record: RefCell<Option<HeroUvDurableRecord>>,
        layout: RefCell<Vec<u8>>,
        link: RefCell<Vec<u8>>,
    }

    impl HeroUvDurablePersistence for MemoryPersistence {
        fn get_hero_uv(
            &self,
            project_id: &str,
            idempotency_key: &str,
        ) -> Result<Option<HeroUvDurableRecord>, String> {
            Ok(self
                .record
                .borrow()
                .as_ref()
                .filter(|record| {
                    record.project_id == project_id && record.idempotency_key == idempotency_key
                })
                .cloned())
        }
        fn commit_hero_uv(
            &self,
            record: &HeroUvDurableRecord,
            layout: &HeroUvDurableCasPayload,
            link: &HeroUvDurableCasPayload,
        ) -> Result<(HeroUvDurableRecord, bool), String> {
            if let Some(existing) = self.record.borrow().as_ref() {
                if existing.input_sha256 != record.input_sha256 {
                    return Err("conflict".to_owned());
                }
                return Ok((existing.clone(), true));
            }
            *self.record.borrow_mut() = Some(record.clone());
            *self.layout.borrow_mut() = layout.bytes.clone();
            *self.link.borrow_mut() = link.bytes.clone();
            Ok((record.clone(), false))
        }
        fn read_hero_uv_bundle(
            &self,
            _record: &HeroUvDurableRecord,
        ) -> Result<HeroUvDurableReadback, String> {
            Ok(HeroUvDurableReadback {
                layout_bytes: self.layout.borrow().clone(),
                link_bytes: self.link.borrow().clone(),
            })
        }
    }

    #[test]
    fn source_binding_rejects_wrong_low_bytes_and_readback() {
        let value = request_value();
        let req: HeroUvDurablePrepareRequest =
            serde_json::from_value(value.clone()).expect("request");
        let readback = json!({"schema_version":"ProductionWeaponLowArtifactReadback@1","artifact_sha256":req.source_low_artifact_sha256,"canonical_sha256":""});
        let mut readback = readback;
        readback["canonical_sha256"] = Value::String(canonical_json_hash(&readback));
        let readback_bytes = canonical_json_bytes(&readback).expect("readback");
        let mut source = source(&value, b"glb", &readback_bytes);
        assert!(validate_source(&req, &source).is_err());
        source.glb_bytes = b"glb".to_vec();
        assert!(validate_source(&req, &source).is_err());
    }
}
