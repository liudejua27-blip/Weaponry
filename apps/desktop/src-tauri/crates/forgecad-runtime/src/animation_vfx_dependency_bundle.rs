//! Request-scoped, read-only verification primitives for the Animation/VFX
//! dependency chain.
//!
//! These values deliberately do not implement `Serialize` and are never
//! stored on `Runtime`.  They are a small seam for a future top-level request
//! to share already-verified inputs between nested getters without turning a
//! verification result into a cross-request cache or a second source of
//! truth.  The module is intentionally not wired into Attachment@3 or
//! Quality@2 yet.

#![allow(dead_code)]

use super::{
    canonical_json_bytes, canonical_json_hash, is_sha256, sha256_hex, Runtime, RuntimeError,
};
use forgecad_contracts::CasObjectRecord;
use serde_json::Value;

const CAS_SCHEMA_VERSION: &str = "CasObject@1";
const MAX_RUNTIME_CAS_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ANIMATION_VFX_OUTPUT_FRAMES: u64 = 15;

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "ANIMATION_VFX_DEPENDENCY_BUNDLE: {}",
        message.into()
    ))
}

/// A CAS object whose database metadata and content hash were checked in the
/// current request.
///
/// The fields are private on purpose.  Callers can only obtain an instance
/// through one of the validating constructors and can only borrow the
/// validated bytes/metadata.  In particular this type has no `Serialize`
/// implementation and cannot become a durable request shortcut by accident.
pub(crate) struct VerifiedCasObject {
    record: CasObjectRecord,
    bytes: Vec<u8>,
}

impl std::fmt::Debug for VerifiedCasObject {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedCasObject")
            .field("record", &self.record)
            .field("bytes_len", &self.bytes.len())
            .finish()
    }
}

impl VerifiedCasObject {
    /// Read and verify an object through the existing Runtime read path.
    ///
    /// This method performs no CAS or SQLite write.  A missing row, metadata
    /// mismatch, bounded-read failure, or byte-hash mismatch fails closed.
    pub(crate) fn read(
        runtime: &Runtime,
        expected_sha256: &str,
        expected_mime: &str,
        expected_kind: &str,
        max_bytes: u64,
    ) -> Result<Self, RuntimeError> {
        let record = runtime
            .store
            .get_object(expected_sha256)?
            .ok_or_else(|| invalid("verified CAS object metadata is missing"))?;
        if record.sha256 != expected_sha256 {
            return Err(invalid("CAS metadata sha256 is not the requested object"));
        }
        let bytes = runtime.cas_read_bounded(expected_sha256, max_bytes)?;
        Self::from_parts(record, bytes, expected_mime, expected_kind, max_bytes)
    }

    /// Construct a verified value from already-read bytes.  This remains
    /// crate-visible rather than public API so future bundle builders cannot
    /// bypass the checks from outside Runtime.
    pub(crate) fn from_parts(
        record: CasObjectRecord,
        bytes: Vec<u8>,
        expected_mime: &str,
        expected_kind: &str,
        max_bytes: u64,
    ) -> Result<Self, RuntimeError> {
        if !is_sha256(&record.sha256) {
            return Err(invalid("CAS metadata contains an invalid sha256"));
        }
        if record.schema_version != CAS_SCHEMA_VERSION {
            return Err(invalid("CAS metadata schema_version mismatch"));
        }
        if expected_mime.is_empty() || expected_kind.is_empty() {
            return Err(invalid("expected CAS metadata is incomplete"));
        }
        if max_bytes == 0 || max_bytes > MAX_RUNTIME_CAS_BYTES {
            return Err(invalid("CAS read budget is outside the Runtime bound"));
        }
        if record.mime != expected_mime {
            return Err(invalid("CAS MIME does not match the typed dependency"));
        }
        if record.kind != expected_kind {
            return Err(invalid("CAS kind does not match the typed dependency"));
        }
        if record.size_bytes > max_bytes {
            return Err(invalid("CAS metadata exceeds the typed read budget"));
        }
        if usize::try_from(record.size_bytes).ok() != Some(bytes.len()) {
            return Err(invalid("CAS metadata size does not match the bytes"));
        }
        let actual_sha256 = sha256_hex(&bytes);
        if record.sha256 != actual_sha256 {
            return Err(invalid("CAS metadata sha256 does not match the bytes"));
        }
        Ok(Self { record, bytes })
    }

    pub(crate) fn record(&self) -> &CasObjectRecord {
        &self.record
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Canonical JSON verified from its exact canonical bytes and digest.
///
/// `T` is deliberately not bounded by `Serialize`: the wrapper is not a wire
/// type.  The current constructor returns `Value`; a future typed consumer
/// can attach a separately parsed contract value without making that value
/// serializable through this wrapper.
pub(crate) struct VerifiedCanonicalJson<T> {
    value: T,
    canonical_sha256: String,
    bytes_sha256: String,
    bytes: Vec<u8>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for VerifiedCanonicalJson<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedCanonicalJson")
            .field("value", &self.value)
            .field("canonical_sha256", &self.canonical_sha256)
            .field("bytes_sha256", &self.bytes_sha256)
            .field("bytes_len", &self.bytes.len())
            .finish()
    }
}

impl VerifiedCanonicalJson<Value> {
    /// Parse a JSON CAS payload and verify canonical bytes, content SHA, the
    /// normalized canonical digest, and (when present) its schema marker.
    pub(crate) fn from_json_bytes(
        bytes: Vec<u8>,
        expected_object_sha256: Option<&str>,
        expected_canonical_sha256: &str,
        expected_schema_version: Option<&str>,
    ) -> Result<Self, RuntimeError> {
        if bytes.is_empty() || bytes.len() > 1024 * 1024 {
            return Err(invalid("canonical JSON payload exceeds the bounded size"));
        }
        if !is_sha256(expected_canonical_sha256) {
            return Err(invalid("canonical JSON digest is not a SHA-256"));
        }
        if let Some(expected_object_sha256) = expected_object_sha256 {
            if !is_sha256(expected_object_sha256) {
                return Err(invalid("JSON object digest is not a SHA-256"));
            }
            if sha256_hex(&bytes) != expected_object_sha256 {
                return Err(invalid("JSON object digest does not match the bytes"));
            }
        }

        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("canonical JSON parse failed: {error}")))?;
        if let Some(expected_schema_version) = expected_schema_version {
            if value.get("schema_version").and_then(Value::as_str) != Some(expected_schema_version)
            {
                return Err(invalid("canonical JSON schema_version mismatch"));
            }
        }
        let canonical_bytes = canonical_json_bytes(&value)
            .map_err(|error| invalid(format!("canonical JSON encoding failed: {error}")))?;
        if canonical_bytes != bytes {
            return Err(invalid("canonical JSON bytes are not canonical"));
        }

        let preimage = canonical_json_preimage(&value);
        let actual_canonical_sha256 = canonical_json_hash(&preimage);
        if actual_canonical_sha256 != expected_canonical_sha256 {
            return Err(invalid(
                "canonical JSON digest does not match the normalized value",
            ));
        }
        if let Some(declared) = value.get("canonical_sha256") {
            let declared = declared
                .as_str()
                .ok_or_else(|| invalid("canonical_sha256 must be a string"))?;
            if !declared.is_empty() && declared != expected_canonical_sha256 {
                return Err(invalid("canonical_sha256 field does not match the digest"));
            }
        }

        Ok(Self {
            value,
            canonical_sha256: actual_canonical_sha256,
            bytes_sha256: sha256_hex(&bytes),
            bytes,
        })
    }
}

impl<T> VerifiedCanonicalJson<T> {
    pub(crate) fn value(&self) -> &T {
        &self.value
    }

    pub(crate) fn canonical_sha256(&self) -> &str {
        &self.canonical_sha256
    }

    pub(crate) fn bytes_sha256(&self) -> &str {
        &self.bytes_sha256
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn canonical_json_preimage(value: &Value) -> Value {
    let mut preimage = value.clone();
    if let Some(object) = preimage.as_object_mut() {
        if object.contains_key("canonical_sha256") {
            object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        }
    }
    preimage
}

/// One named worker output pass with a self-checked content digest.
pub(crate) struct WorkerPass {
    name: String,
    bytes: Vec<u8>,
    sha256: String,
}

impl std::fmt::Debug for WorkerPass {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerPass")
            .field("name", &self.name)
            .field("sha256", &self.sha256)
            .field("bytes_len", &self.bytes.len())
            .finish()
    }
}

impl WorkerPass {
    pub(crate) fn new(
        name: impl Into<String>,
        bytes: Vec<u8>,
        declared_sha256: impl Into<String>,
    ) -> Result<Self, RuntimeError> {
        let pass = Self {
            name: name.into(),
            bytes,
            sha256: declared_sha256.into(),
        };
        pass.validate()?;
        Ok(pass)
    }

    fn validate(&self) -> Result<(), RuntimeError> {
        if self.name.is_empty() {
            return Err(invalid("worker pass name is empty"));
        }
        if !is_sha256(&self.sha256) {
            return Err(invalid("worker pass digest is not a SHA-256"));
        }
        if sha256_hex(&self.bytes) != self.sha256 {
            return Err(invalid("worker pass digest does not match the bytes"));
        }
        Ok(())
    }
}

/// Proof that two worker executions were equivalent for one typed frame.
///
/// The proof retains only the first typed payload plus digest-level evidence
/// for both executions.  The second payload must compare equal at
/// construction time; no second unverified payload is exposed later.
pub(crate) struct WorkerReplayProof<T> {
    first: T,
    frame_index: u64,
    sample_time_ticks: u64,
    cohort_sha256: String,
    pass_order: Vec<String>,
    pass_sha256: Vec<String>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for WorkerReplayProof<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerReplayProof")
            .field("first", &self.first)
            .field("frame_index", &self.frame_index)
            .field("sample_time_ticks", &self.sample_time_ticks)
            .field("cohort_sha256", &self.cohort_sha256)
            .field("pass_order", &self.pass_order)
            .field("pass_sha256", &self.pass_sha256)
            .finish()
    }
}

impl<T: PartialEq> WorkerReplayProof<T> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_pair(
        first: T,
        repeat: T,
        first_frame_index: u64,
        repeat_frame_index: u64,
        first_sample_time_ticks: u64,
        repeat_sample_time_ticks: u64,
        expected_frame_index: u64,
        expected_sample_time_ticks: u64,
        first_cohort_sha256: Option<&str>,
        repeat_cohort_sha256: Option<&str>,
        expected_cohort_sha256: &str,
        first_passes: &[WorkerPass],
        repeat_passes: &[WorkerPass],
        expected_pass_order: &[&str],
    ) -> Result<Self, RuntimeError> {
        if !is_sha256(expected_cohort_sha256) {
            return Err(invalid("worker cohort digest is not a SHA-256"));
        }
        if first != repeat {
            return Err(invalid("worker first/repeat typed payload differs"));
        }
        if first_frame_index != repeat_frame_index {
            return Err(invalid("worker first/repeat frame index differs"));
        }
        if first_sample_time_ticks != repeat_sample_time_ticks {
            return Err(invalid("worker first/repeat sample tick differs"));
        }
        if first_frame_index != expected_frame_index {
            return Err(invalid("worker frame index does not match the dependency"));
        }
        if first_sample_time_ticks != expected_sample_time_ticks {
            return Err(invalid("worker sample tick does not match the dependency"));
        }
        let first_cohort_sha256 = first_cohort_sha256
            .filter(|cohort| is_sha256(cohort))
            .ok_or_else(|| invalid("worker first cohort is missing or invalid"))?;
        let repeat_cohort_sha256 = repeat_cohort_sha256
            .filter(|cohort| is_sha256(cohort))
            .ok_or_else(|| invalid("worker repeat cohort is missing or invalid"))?;
        if first_cohort_sha256 != expected_cohort_sha256
            || repeat_cohort_sha256 != expected_cohort_sha256
        {
            return Err(invalid("worker cohort does not match the expected cohort"));
        }
        if expected_pass_order.is_empty()
            || expected_pass_order.iter().any(|pass| pass.is_empty())
            || has_duplicates(expected_pass_order.iter().copied())
        {
            return Err(invalid("worker pass order is not a closed unique list"));
        }
        validate_passes(first_passes, expected_pass_order)?;
        validate_passes(repeat_passes, expected_pass_order)?;
        for (first_pass, repeat_pass) in first_passes.iter().zip(repeat_passes) {
            if first_pass.name != repeat_pass.name
                || first_pass.sha256 != repeat_pass.sha256
                || first_pass.bytes != repeat_pass.bytes
            {
                return Err(invalid("worker first/repeat pass differs"));
            }
        }

        Ok(Self {
            first,
            frame_index: first_frame_index,
            sample_time_ticks: first_sample_time_ticks,
            cohort_sha256: expected_cohort_sha256.to_owned(),
            pass_order: first_passes.iter().map(|pass| pass.name.clone()).collect(),
            pass_sha256: first_passes
                .iter()
                .map(|pass| pass.sha256.clone())
                .collect(),
        })
    }

    pub(crate) fn first(&self) -> &T {
        &self.first
    }

    pub(crate) fn frame_index(&self) -> u64 {
        self.frame_index
    }

    pub(crate) fn sample_time_ticks(&self) -> u64 {
        self.sample_time_ticks
    }

    pub(crate) fn cohort_sha256(&self) -> &str {
        &self.cohort_sha256
    }

    pub(crate) fn pass_order(&self) -> &[String] {
        &self.pass_order
    }

    pub(crate) fn pass_sha256(&self) -> &[String] {
        &self.pass_sha256
    }
}

fn validate_passes(passes: &[WorkerPass], expected_order: &[&str]) -> Result<(), RuntimeError> {
    if passes.len() != expected_order.len() {
        return Err(invalid("worker pass count differs from the closed order"));
    }
    for (pass, expected_name) in passes.iter().zip(expected_order) {
        pass.validate()?;
        if pass.name != *expected_name {
            return Err(invalid("worker pass order differs from the closed order"));
        }
    }
    Ok(())
}

fn has_duplicates<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    values.into_iter().any(|value| !seen.insert(value))
}

/// The explicit index/tick join used by a top-level Animation/VFX request.
///
/// The output frame is ordinal 0..14, while Projection/Particles are the
/// current source frame 1..15 and Trails/TrailsBloom are the durable output
/// frame 0..14.  This object only records the verified join; it does not hold
/// a mutable cache or any raw worker/CAS write handle.
pub(crate) struct VerifiedFrameDependency {
    output_frame_index: u64,
    sample_time_ticks: u64,
    projection_frame_index: u64,
    particle_frame_index: u64,
    trails_frame_index: u64,
    trails_bloom_frame_index: u64,
    projection_frame_canonical_sha256: String,
    particle_frame_canonical_sha256: String,
    trails_frame_canonical_sha256: String,
    trails_bloom_frame_canonical_sha256: String,
}

impl std::fmt::Debug for VerifiedFrameDependency {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedFrameDependency")
            .field("output_frame_index", &self.output_frame_index)
            .field("sample_time_ticks", &self.sample_time_ticks)
            .field("projection_frame_index", &self.projection_frame_index)
            .field("particle_frame_index", &self.particle_frame_index)
            .field("trails_frame_index", &self.trails_frame_index)
            .field("trails_bloom_frame_index", &self.trails_bloom_frame_index)
            .field(
                "projection_frame_canonical_sha256",
                &self.projection_frame_canonical_sha256,
            )
            .field(
                "particle_frame_canonical_sha256",
                &self.particle_frame_canonical_sha256,
            )
            .field(
                "trails_frame_canonical_sha256",
                &self.trails_frame_canonical_sha256,
            )
            .field(
                "trails_bloom_frame_canonical_sha256",
                &self.trails_bloom_frame_canonical_sha256,
            )
            .finish()
    }
}

impl VerifiedFrameDependency {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        output_frame_index: u64,
        sample_time_ticks: u64,
        projection_frame_index: u64,
        projection_sample_time_ticks: u64,
        particle_frame_index: u64,
        particle_sample_time_ticks: u64,
        trails_frame_index: u64,
        trails_sample_time_ticks: u64,
        trails_bloom_frame_index: u64,
        trails_bloom_sample_time_ticks: u64,
        projection_frame_canonical_sha256: &str,
        particle_frame_canonical_sha256: &str,
        trails_frame_canonical_sha256: &str,
        trails_bloom_frame_canonical_sha256: &str,
    ) -> Result<Self, RuntimeError> {
        if output_frame_index >= MAX_ANIMATION_VFX_OUTPUT_FRAMES {
            return Err(invalid("output frame index is outside 0..14"));
        }
        if projection_frame_index != output_frame_index + 1
            || particle_frame_index != output_frame_index + 1
            || trails_frame_index != output_frame_index
            || trails_bloom_frame_index != output_frame_index
        {
            return Err(invalid("Animation/VFX frame index mapping is inconsistent"));
        }
        if projection_sample_time_ticks != sample_time_ticks
            || particle_sample_time_ticks != sample_time_ticks
            || trails_sample_time_ticks != sample_time_ticks
            || trails_bloom_sample_time_ticks != sample_time_ticks
        {
            return Err(invalid("Animation/VFX frame sample tick is inconsistent"));
        }
        for digest in [
            projection_frame_canonical_sha256,
            particle_frame_canonical_sha256,
            trails_frame_canonical_sha256,
            trails_bloom_frame_canonical_sha256,
        ] {
            if !is_sha256(digest) {
                return Err(invalid("frame canonical digest is not a SHA-256"));
            }
        }
        Ok(Self {
            output_frame_index,
            sample_time_ticks,
            projection_frame_index,
            particle_frame_index,
            trails_frame_index,
            trails_bloom_frame_index,
            projection_frame_canonical_sha256: projection_frame_canonical_sha256.to_owned(),
            particle_frame_canonical_sha256: particle_frame_canonical_sha256.to_owned(),
            trails_frame_canonical_sha256: trails_frame_canonical_sha256.to_owned(),
            trails_bloom_frame_canonical_sha256: trails_bloom_frame_canonical_sha256.to_owned(),
        })
    }

    pub(crate) fn output_frame_index(&self) -> u64 {
        self.output_frame_index
    }

    pub(crate) fn sample_time_ticks(&self) -> u64 {
        self.sample_time_ticks
    }

    pub(crate) fn projection_frame_index(&self) -> u64 {
        self.projection_frame_index
    }

    pub(crate) fn particle_frame_index(&self) -> u64 {
        self.particle_frame_index
    }

    pub(crate) fn trails_frame_index(&self) -> u64 {
        self.trails_frame_index
    }

    pub(crate) fn trails_bloom_frame_index(&self) -> u64 {
        self.trails_bloom_frame_index
    }

    pub(crate) fn projection_frame_canonical_sha256(&self) -> &str {
        &self.projection_frame_canonical_sha256
    }

    pub(crate) fn particle_frame_canonical_sha256(&self) -> &str {
        &self.particle_frame_canonical_sha256
    }

    pub(crate) fn trails_frame_canonical_sha256(&self) -> &str {
        &self.trails_frame_canonical_sha256
    }

    pub(crate) fn trails_bloom_frame_canonical_sha256(&self) -> &str {
        &self.trails_bloom_frame_canonical_sha256
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const MIME: &str = "application/octet-stream";
    const KIND: &str = "verified-test-object";
    const PASS_ORDER: &[&str] = &["color", "id", "depth"];

    fn cas_record(bytes: &[u8]) -> CasObjectRecord {
        CasObjectRecord {
            schema_version: CAS_SCHEMA_VERSION.to_owned(),
            sha256: sha256_hex(bytes),
            size_bytes: bytes.len() as u64,
            mime: MIME.to_owned(),
            kind: KIND.to_owned(),
            reachability: "temporary".to_owned(),
            created_at: "test".to_owned(),
        }
    }

    fn pass(name: &str, bytes: &[u8]) -> WorkerPass {
        WorkerPass::new(name, bytes.to_vec(), sha256_hex(bytes)).expect("valid pass")
    }

    fn proof(
        first: u64,
        repeat: u64,
        first_frame_index: u64,
        repeat_frame_index: u64,
        first_tick: u64,
        repeat_tick: u64,
        first_cohort: Option<&str>,
        repeat_cohort: Option<&str>,
        first_passes: &[WorkerPass],
        repeat_passes: &[WorkerPass],
    ) -> Result<WorkerReplayProof<u64>, RuntimeError> {
        WorkerReplayProof::from_pair(
            first,
            repeat,
            first_frame_index,
            repeat_frame_index,
            first_tick,
            repeat_tick,
            first_frame_index,
            first_tick,
            first_cohort,
            repeat_cohort,
            SHA_A,
            first_passes,
            repeat_passes,
            PASS_ORDER,
        )
    }

    #[test]
    fn verified_cas_object_requires_exact_metadata_size_and_sha() {
        let bytes = b"verified-cas".to_vec();
        let record = cas_record(&bytes);
        let verified =
            VerifiedCasObject::from_parts(record.clone(), bytes.clone(), MIME, KIND, 1024)
                .expect("metadata and bytes are valid");
        assert_eq!(verified.record().sha256, sha256_hex(&bytes));
        assert_eq!(verified.bytes(), bytes.as_slice());

        let mut wrong_kind = record.clone();
        wrong_kind.kind = "other-kind".to_owned();
        assert!(
            VerifiedCasObject::from_parts(wrong_kind, bytes.clone(), MIME, KIND, 1024).is_err()
        );

        let mut wrong_size = record.clone();
        wrong_size.size_bytes += 1;
        assert!(
            VerifiedCasObject::from_parts(wrong_size, bytes.clone(), MIME, KIND, 1024).is_err()
        );

        let mut wrong_sha = record;
        wrong_sha.sha256 = SHA_B.to_owned();
        assert!(VerifiedCasObject::from_parts(wrong_sha, bytes, MIME, KIND, 1024).is_err());
    }

    #[test]
    fn verified_canonical_json_requires_exact_canonical_bytes_and_digest() {
        let mut value = json!({"schema_version":"DependencyReceipt@1", "canonical_sha256":""});
        let expected_canonical = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(expected_canonical.clone());
        let bytes = canonical_json_bytes(&value).expect("canonical bytes");
        let object_sha256 = sha256_hex(&bytes);
        let verified = VerifiedCanonicalJson::from_json_bytes(
            bytes.clone(),
            Some(&object_sha256),
            &expected_canonical,
            Some("DependencyReceipt@1"),
        )
        .expect("canonical JSON is valid");
        assert_eq!(verified.canonical_sha256(), expected_canonical);
        assert_eq!(verified.bytes_sha256(), object_sha256);

        let mut noncanonical = bytes.clone();
        noncanonical.push(b' ');
        assert!(VerifiedCanonicalJson::from_json_bytes(
            noncanonical,
            None,
            &expected_canonical,
            Some("DependencyReceipt@1"),
        )
        .is_err());
        assert!(VerifiedCanonicalJson::from_json_bytes(
            bytes.clone(),
            Some(SHA_B),
            &expected_canonical,
            Some("DependencyReceipt@1"),
        )
        .is_err());
        assert!(VerifiedCanonicalJson::from_json_bytes(
            bytes,
            None,
            SHA_B,
            Some("DependencyReceipt@1"),
        )
        .is_err());
    }

    #[test]
    fn worker_replay_proof_requires_payload_cohort_pass_order_and_frame_tick_equality() {
        let first_passes = [pass("color", b"c"), pass("id", b"i"), pass("depth", b"d")];
        let repeat_passes = [pass("color", b"c"), pass("id", b"i"), pass("depth", b"d")];
        let valid = proof(
            7,
            7,
            3,
            3,
            42,
            42,
            Some(SHA_A),
            Some(SHA_A),
            &first_passes,
            &repeat_passes,
        )
        .expect("matching replays");
        assert_eq!(valid.frame_index(), 3);
        assert_eq!(valid.sample_time_ticks(), 42);
        assert_eq!(valid.pass_order(), ["color", "id", "depth"]);

        assert!(proof(
            7,
            8,
            3,
            3,
            42,
            42,
            Some(SHA_A),
            Some(SHA_A),
            &first_passes,
            &repeat_passes,
        )
        .is_err());
        assert!(proof(
            7,
            7,
            3,
            4,
            42,
            42,
            Some(SHA_A),
            Some(SHA_A),
            &first_passes,
            &repeat_passes,
        )
        .is_err());
        assert!(proof(
            7,
            7,
            3,
            3,
            42,
            43,
            Some(SHA_A),
            Some(SHA_A),
            &first_passes,
            &repeat_passes,
        )
        .is_err());
        assert!(proof(
            7,
            7,
            3,
            3,
            42,
            42,
            Some(SHA_A),
            Some(SHA_C),
            &first_passes,
            &repeat_passes,
        )
        .is_err());

        let wrong_order = [pass("id", b"i"), pass("color", b"c"), pass("depth", b"d")];
        assert!(proof(
            7,
            7,
            3,
            3,
            42,
            42,
            Some(SHA_A),
            Some(SHA_A),
            &wrong_order,
            &repeat_passes,
        )
        .is_err());

        let wrong_repeat = [
            pass("color", b"c"),
            pass("id", b"different"),
            pass("depth", b"d"),
        ];
        assert!(proof(
            7,
            7,
            3,
            3,
            42,
            42,
            Some(SHA_A),
            Some(SHA_A),
            &first_passes,
            &wrong_repeat,
        )
        .is_err());

        assert!(WorkerReplayProof::from_pair(
            7,
            7,
            3,
            3,
            42,
            42,
            4,
            42,
            Some(SHA_A),
            Some(SHA_A),
            SHA_A,
            &first_passes,
            &repeat_passes,
            PASS_ORDER,
        )
        .is_err());
        assert!(WorkerReplayProof::from_pair(
            7,
            7,
            3,
            3,
            42,
            42,
            3,
            43,
            Some(SHA_A),
            Some(SHA_A),
            SHA_A,
            &first_passes,
            &repeat_passes,
            PASS_ORDER,
        )
        .is_err());
    }

    #[test]
    fn verified_frame_dependency_requires_exact_index_tick_and_canonical_bindings() {
        let valid = VerifiedFrameDependency::new(
            0, 100, 1, 100, 1, 100, 0, 100, 0, 100, SHA_A, SHA_B, SHA_C, SHA_A,
        )
        .expect("frame join is valid");
        assert_eq!(valid.output_frame_index(), 0);
        assert_eq!(valid.projection_frame_index(), 1);
        assert_eq!(valid.trails_bloom_frame_index(), 0);

        assert!(VerifiedFrameDependency::new(
            0, 100, 0, 100, 1, 100, 0, 100, 0, 100, SHA_A, SHA_B, SHA_C, SHA_A,
        )
        .is_err());
        assert!(VerifiedFrameDependency::new(
            0, 100, 1, 101, 1, 100, 0, 100, 0, 100, SHA_A, SHA_B, SHA_C, SHA_A,
        )
        .is_err());
        assert!(VerifiedFrameDependency::new(
            0,
            100,
            1,
            100,
            1,
            100,
            0,
            100,
            0,
            100,
            SHA_A,
            "not-a-sha",
            SHA_C,
            SHA_A,
        )
        .is_err());
        assert!(VerifiedFrameDependency::new(
            MAX_ANIMATION_VFX_OUTPUT_FRAMES,
            100,
            MAX_ANIMATION_VFX_OUTPUT_FRAMES + 1,
            100,
            MAX_ANIMATION_VFX_OUTPUT_FRAMES + 1,
            100,
            MAX_ANIMATION_VFX_OUTPUT_FRAMES,
            100,
            MAX_ANIMATION_VFX_OUTPUT_FRAMES,
            100,
            SHA_A,
            SHA_B,
            SHA_C,
            SHA_A,
        )
        .is_err());
    }
}
