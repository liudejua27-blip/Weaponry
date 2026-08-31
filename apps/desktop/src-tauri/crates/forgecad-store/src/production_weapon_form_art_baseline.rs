//! Store-owned durable seam for a Runtime-produced six-view FormArt baseline.
//!
//! The public contract record is the durable value.  This module only adds a
//! SQLite index and verifies the immutable CAS objects supplied by Runtime; it
//! does not introduce a second Link contract or perform a stage/candidate
//! side effect.  A commit inserts the row and marks the lineage receipt,
//! RegisteredCameraRigCalibration@2, baseline parent receipt and all six view
//! receipts reachable in one SQLite transaction.

use forgecad_contracts::{
    is_opaque_id, is_sha256, CasObjectRecord, ProductionWeaponFormArtBaselineRecord,
    ProductionWeaponFormArtBaselineView, PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_CANONICALIZATION_POLICY,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_IDEMPOTENCY_POLICY,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_MATERIALIZATION_STATUS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_POLICY, PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_SCHEMA_VERSION,
    PRODUCTION_WEAPON_FORM_ART_BASELINE_WRITER_POLICY,
};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

use super::{Store, StoreError};

pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_PARENT_OBJECT_KIND: &str =
    "production-weapon-form-art-baseline-v1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_OBJECT_KIND: &str =
    "production-weapon-form-art-baseline-view-v1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_LINEAGE_RECEIPT_OBJECT_KIND: &str =
    "production-camera-lock-registration-lineage-receipt";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_RIG_V2_OBJECT_KIND: &str =
    "registered-camera-rig-calibration-v2";

/// Store-local owner identity for one fresh FormArt baseline prepare.  This
/// is deliberately not a public contract: it is a bounded recovery index that
/// lets startup distinguish this transaction's temporary CAS objects from
/// another producer's content-addressed objects.
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_SCHEMA_VERSION: &str =
    "ProductionWeaponFormArtBaselineCasBatch@1";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_KIND: &str =
    "fresh-form-art-baseline-prepare";
pub const PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_TIMEOUT_SECS: u64 = 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionWeaponFormArtBaselineCasBatchOwner {
    pub schema_version: String,
    pub owner_kind: String,
    pub batch_id: String,
    pub baseline_id: String,
    pub registration_lineage_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub request_sha256: String,
    pub input_sha256: String,
    pub runtime_build_cohort_sha256: String,
    pub created_at: String,
    pub expires_at: String,
}

/// A live Store reservation plus its durable, typed batch owner.  The
/// reservation remains in-process only; the owner row is what makes a crash
/// recoverable after the reservation is dropped.
///
/// The Store handle is retained so dropping a batch can close an abandoned
/// owner immediately. This is deliberately best-effort in `Drop`; startup
/// reconciliation remains the crash-only fallback.
pub struct ProductionWeaponFormArtBaselineCasBatch {
    owner: ProductionWeaponFormArtBaselineCasBatchOwner,
    reservation: super::CasReservation,
    store: Store,
}

impl std::fmt::Debug for ProductionWeaponFormArtBaselineCasBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionWeaponFormArtBaselineCasBatch")
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl Drop for ProductionWeaponFormArtBaselineCasBatch {
    fn drop(&mut self) {
        // Runtime normally calls the explicit abort/complete seam before the
        // handle goes out of scope. Keep the drop path as a bounded safety net
        // for early returns and panics; it never deletes CAS bytes.
        let store = self.store.clone();
        let _ = store.abort_production_weapon_form_art_baseline_cas_batch(self);
    }
}

impl ProductionWeaponFormArtBaselineCasBatch {
    pub fn owner(&self) -> &ProductionWeaponFormArtBaselineCasBatchOwner {
        &self.owner
    }

    pub fn batch_id(&self) -> &str {
        &self.owner.batch_id
    }

    pub fn reservation(&self) -> &super::CasReservation {
        &self.reservation
    }
}

const TABLE: &str = "production_weapon_form_art_baselines";
const CAS_BATCH_TABLE: &str = "production_weapon_form_art_baseline_cas_batches";
const CAS_BATCH_OBJECT_TABLE: &str = "production_weapon_form_art_baseline_cas_batch_objects";
const CAS_BATCH_OPEN: &str = "open";
const CAS_BATCH_COMMITTED: &str = "committed";
const CAS_BATCH_QUARANTINED: &str = "quarantined";
const JSON_MIME: &str = "application/json";
const MAX_JSON_BYTES: u64 = 1_048_576;

// A crashed FormArt prepare can leave its batch-owned objects registered as
// temporary after the in-process reservation has disappeared.  Keep opening
// repair deliberately narrow and metadata-only.  Unknown timestamps,
// metadata, links, reservations, or ownership are retained fail-closed.
const RECONCILIATION_MIN_AGE_SECS: u64 = 60 * 60;
const RECONCILIATION_MAX_OBJECTS: i64 = 64;
const RECONCILIATION_MAX_VERIFY_BYTES: u64 = 64 * 1024 * 1024;

/// CAS inputs and the public baseline record committed as one durable unit.
#[derive(Debug, Clone)]
pub struct ProductionWeaponFormArtBaselineCommitBundle {
    pub baseline: ProductionWeaponFormArtBaselineRecord,
    pub baseline_parent_receipt_object: CasObjectRecord,
    pub baseline_view_receipt_objects: Vec<CasObjectRecord>,
    pub registration_lineage_receipt_object: CasObjectRecord,
    pub registered_rig_v2_object: CasObjectRecord,
}

impl ProductionWeaponFormArtBaselineCommitBundle {
    pub fn new(
        baseline: ProductionWeaponFormArtBaselineRecord,
        baseline_parent_receipt_object: CasObjectRecord,
        baseline_view_receipt_objects: Vec<CasObjectRecord>,
        registration_lineage_receipt_object: CasObjectRecord,
        registered_rig_v2_object: CasObjectRecord,
    ) -> Self {
        Self {
            baseline,
            baseline_parent_receipt_object,
            baseline_view_receipt_objects,
            registration_lineage_receipt_object,
            registered_rig_v2_object,
        }
    }
}

fn contract(code: &str, message: impl Into<String>) -> StoreError {
    StoreError::Contract {
        code: code.to_owned(),
        message: message.into(),
    }
}

pub(crate) fn ensure_table(connection: &rusqlite::Connection) -> Result<(), StoreError> {
    let existing_table_sql: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![TABLE],
            |row| row.get(0),
        )
        .optional()?;
    let legacy_inline_identity = existing_table_sql.as_ref().is_some_and(|sql| {
        let normalized = sql.to_ascii_lowercase();
        normalized.contains("unique (registration_lineage_id, candidate_id, artifact_sha256)")
            && !normalized.contains(
                "unique (registration_lineage_id, candidate_id, artifact_sha256, runtime_build_cohort_sha256)",
            )
    });
    if legacy_inline_identity {
        // A fresh baseline is immutable evidence for one exact Runtime/Worker
        // source cohort. The first development schema accidentally locked the
        // source identity across all future cohorts, making truthful refresh
        // impossible after a source build changed. Rebuild only the owner
        // table and preserve every public record byte-for-byte.
        connection.execute_batch(&format!(
            "DROP TRIGGER IF EXISTS production_weapon_form_art_baselines_identity_guard;
             DROP INDEX IF EXISTS production_weapon_form_art_baselines_scope_idx;
             DROP INDEX IF EXISTS production_weapon_form_art_baselines_object_idx;
             DROP INDEX IF EXISTS production_weapon_form_art_baselines_identity_lookup_idx;
             DROP INDEX IF EXISTS production_weapon_form_art_baselines_identity_unique_idx;
             ALTER TABLE {TABLE} RENAME TO production_weapon_form_art_baselines_legacy_identity;
             CREATE TABLE {TABLE} (
                 baseline_id TEXT PRIMARY KEY,
                 schema_version TEXT NOT NULL CHECK (schema_version = 'ProductionWeaponFormArtBaseline@1'),
                 session_id TEXT NOT NULL,
                 project_id TEXT NOT NULL,
                 candidate_id TEXT NOT NULL,
                 candidate_state_sha256 TEXT NOT NULL,
                 artifact_id TEXT NOT NULL,
                 artifact_sha256 TEXT NOT NULL,
                 registration_lineage_id TEXT NOT NULL,
                 registration_lineage_canonical_sha256 TEXT NOT NULL,
                 registration_lineage_receipt_object_sha256 TEXT NOT NULL,
                 registered_rig_v2_id TEXT NOT NULL,
                 registered_rig_v2_object_sha256 TEXT NOT NULL,
                 registered_rig_v2_canonical_sha256 TEXT NOT NULL,
                 receipt_object_sha256 TEXT NOT NULL,
                 view_kinds_json TEXT NOT NULL,
                 view_receipt_object_sha256_json TEXT NOT NULL,
                 runtime_build_cohort_sha256 TEXT NOT NULL,
                 baseline_policy TEXT NOT NULL,
                 materialization_status TEXT NOT NULL,
                 request_sha256 TEXT NOT NULL,
                 input_sha256 TEXT NOT NULL,
                 idempotency_key TEXT NOT NULL,
                 canonical_sha256 TEXT NOT NULL,
                 created_at TEXT NOT NULL,
                 record_json TEXT NOT NULL,
                 UNIQUE (project_id, idempotency_key)
             );
             INSERT INTO {TABLE} (
                 baseline_id, schema_version, session_id, project_id, candidate_id,
                 candidate_state_sha256, artifact_id, artifact_sha256,
                 registration_lineage_id, registration_lineage_canonical_sha256,
                 registration_lineage_receipt_object_sha256, registered_rig_v2_id,
                 registered_rig_v2_object_sha256, registered_rig_v2_canonical_sha256,
                 receipt_object_sha256, view_kinds_json, view_receipt_object_sha256_json,
                 runtime_build_cohort_sha256, baseline_policy, materialization_status,
                 request_sha256, input_sha256, idempotency_key, canonical_sha256,
                 created_at, record_json
             )
             SELECT
                 baseline_id, schema_version, session_id, project_id, candidate_id,
                 candidate_state_sha256, artifact_id, artifact_sha256,
                 registration_lineage_id, registration_lineage_canonical_sha256,
                 registration_lineage_receipt_object_sha256, registered_rig_v2_id,
                 registered_rig_v2_object_sha256, registered_rig_v2_canonical_sha256,
                 receipt_object_sha256, view_kinds_json, view_receipt_object_sha256_json,
                 runtime_build_cohort_sha256, baseline_policy, materialization_status,
                 request_sha256, input_sha256, idempotency_key, canonical_sha256,
                 created_at, record_json
               FROM production_weapon_form_art_baselines_legacy_identity;
             DROP TABLE production_weapon_form_art_baselines_legacy_identity;"
        ))?;
    }
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {TABLE} (
             baseline_id TEXT PRIMARY KEY,
             schema_version TEXT NOT NULL CHECK (schema_version = 'ProductionWeaponFormArtBaseline@1'),
             session_id TEXT NOT NULL,
             project_id TEXT NOT NULL,
             candidate_id TEXT NOT NULL,
             candidate_state_sha256 TEXT NOT NULL,
             artifact_id TEXT NOT NULL,
             artifact_sha256 TEXT NOT NULL,
             registration_lineage_id TEXT NOT NULL,
             registration_lineage_canonical_sha256 TEXT NOT NULL,
             registration_lineage_receipt_object_sha256 TEXT NOT NULL,
             registered_rig_v2_id TEXT NOT NULL,
             registered_rig_v2_object_sha256 TEXT NOT NULL,
             registered_rig_v2_canonical_sha256 TEXT NOT NULL,
             receipt_object_sha256 TEXT NOT NULL,
             view_kinds_json TEXT NOT NULL,
             view_receipt_object_sha256_json TEXT NOT NULL,
             runtime_build_cohort_sha256 TEXT NOT NULL,
             baseline_policy TEXT NOT NULL,
             materialization_status TEXT NOT NULL,
             request_sha256 TEXT NOT NULL,
             input_sha256 TEXT NOT NULL,
             idempotency_key TEXT NOT NULL,
             canonical_sha256 TEXT NOT NULL,
             created_at TEXT NOT NULL,
             record_json TEXT NOT NULL,
             UNIQUE (project_id, idempotency_key)
         );
         CREATE INDEX IF NOT EXISTS production_weapon_form_art_baselines_scope_idx
             ON {TABLE}(project_id, candidate_id, created_at DESC, baseline_id ASC);
         CREATE INDEX IF NOT EXISTS production_weapon_form_art_baselines_object_idx
             ON {TABLE}(registration_lineage_receipt_object_sha256, registered_rig_v2_object_sha256, receipt_object_sha256);
         CREATE INDEX IF NOT EXISTS production_weapon_form_art_baselines_identity_lookup_idx
             ON {TABLE}(registration_lineage_id, candidate_id, artifact_sha256);
         DROP TRIGGER IF EXISTS production_weapon_form_art_baselines_identity_guard;
         CREATE TRIGGER production_weapon_form_art_baselines_identity_guard
             BEFORE INSERT ON {TABLE}
             FOR EACH ROW
             WHEN EXISTS (
                 SELECT 1 FROM {TABLE}
                  WHERE registration_lineage_id = NEW.registration_lineage_id
                    AND candidate_id = NEW.candidate_id
                    AND artifact_sha256 = NEW.artifact_sha256
                    AND runtime_build_cohort_sha256 = NEW.runtime_build_cohort_sha256
             )
             BEGIN
                 SELECT RAISE(ABORT, 'PRODUCTION_WEAPON_FORM_ART_BASELINE_IDENTITY_CONFLICT');
             END;"
    ))?;

    // Older development stores may already contain duplicate source/cohort
    // rows from before this invariant existed.  Do not delete or rewrite
    // those immutable rows, and do not make opening such a store fail.  A
    // unique index is installed when the existing data permits it; the
    // trigger above still prevents any new duplicate insert when legacy
    // duplicates are present.  The commit transaction performs the same
    // identity query and returns a typed conflict before attempting INSERT.
    let has_identity_duplicate: bool = connection.query_row(
        &format!(
            "SELECT EXISTS (
                 SELECT 1 FROM {TABLE}
                  GROUP BY registration_lineage_id, candidate_id, artifact_sha256,
                           runtime_build_cohort_sha256
                  HAVING COUNT(*) > 1
             )"
        ),
        [],
        |row| row.get(0),
    )?;
    connection.execute_batch(
        "DROP INDEX IF EXISTS production_weapon_form_art_baselines_identity_unique_idx;",
    )?;
    if !has_identity_duplicate {
        connection.execute_batch(&format!(
            "CREATE UNIQUE INDEX IF NOT EXISTS production_weapon_form_art_baselines_identity_unique_idx
            ON {TABLE}(registration_lineage_id, candidate_id, artifact_sha256, runtime_build_cohort_sha256);"
        ))?;
    }

    // This index is Store-local and intentionally separate from the public
    // baseline record.  The owner row is written before any derived object;
    // object rows are added only after the bounded CAS metadata has been
    // registered, so a crash leaves enough information for fail-closed
    // startup reconciliation without putting a second durable link in the
    // contract graph.
    connection.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {CAS_BATCH_TABLE} (
             batch_id TEXT PRIMARY KEY,
             schema_version TEXT NOT NULL CHECK (schema_version = 'ProductionWeaponFormArtBaselineCasBatch@1'),
             owner_kind TEXT NOT NULL CHECK (owner_kind = 'fresh-form-art-baseline-prepare'),
             baseline_id TEXT NOT NULL,
             registration_lineage_id TEXT NOT NULL,
             session_id TEXT NOT NULL,
             project_id TEXT NOT NULL,
             candidate_id TEXT NOT NULL,
             candidate_state_sha256 TEXT NOT NULL,
             artifact_id TEXT NOT NULL,
             artifact_sha256 TEXT NOT NULL,
             request_sha256 TEXT NOT NULL,
             input_sha256 TEXT NOT NULL,
             runtime_build_cohort_sha256 TEXT NOT NULL,
             created_at TEXT NOT NULL,
             expires_at TEXT NOT NULL,
             status TEXT NOT NULL CHECK (status IN ('open', 'committed', 'quarantined')),
             completed_at TEXT
         );
         CREATE INDEX IF NOT EXISTS production_weapon_form_art_baseline_cas_batches_expiry_idx
             ON {CAS_BATCH_TABLE}(status, expires_at, created_at, batch_id);
         CREATE INDEX IF NOT EXISTS production_weapon_form_art_baseline_cas_batches_scope_idx
             ON {CAS_BATCH_TABLE}(project_id, candidate_id, baseline_id);
         CREATE TABLE IF NOT EXISTS {CAS_BATCH_OBJECT_TABLE} (
             batch_id TEXT NOT NULL REFERENCES {CAS_BATCH_TABLE}(batch_id),
             object_sha256 TEXT NOT NULL,
             size_bytes INTEGER NOT NULL,
             mime TEXT NOT NULL,
             kind TEXT NOT NULL,
             status TEXT NOT NULL CHECK (status IN ('temporary', 'reachable', 'quarantined', 'linked', 'missing', 'corrupt')),
             created_at TEXT NOT NULL,
             PRIMARY KEY (batch_id, object_sha256)
         );
         CREATE INDEX IF NOT EXISTS production_weapon_form_art_baseline_cas_batch_objects_lookup_idx
             ON {CAS_BATCH_OBJECT_TABLE}(object_sha256, status, batch_id);"
    ))?;
    Ok(())
}

fn baseline_insert_constraint_error(error: &rusqlite::Error) -> Option<StoreError> {
    let rusqlite::Error::SqliteFailure(failure, message) = error else {
        return None;
    };
    if failure.code != rusqlite::ErrorCode::ConstraintViolation {
        return None;
    }
    let detail = message.as_deref().unwrap_or_default();
    if detail.contains("PRODUCTION_WEAPON_FORM_ART_BASELINE_IDENTITY_CONFLICT") {
        return Some(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_IDENTITY_CONFLICT",
            "registration lineage, candidate, artifact and Runtime cohort are already bound to a durable baseline",
        ));
    }
    Some(contract(
        "PRODUCTION_WEAPON_FORM_ART_BASELINE_CONFLICT",
        "baseline identity or idempotency key is already bound",
    ))
}

fn reconciliation_object_is_old_enough(created_at: &str, now_secs: u64) -> bool {
    let Some(created_secs) = created_at.parse::<u64>().ok() else {
        // Runtime timestamps are unix seconds today.  Unknown formats are
        // retained rather than guessed, so opening a Store is fail-closed.
        return false;
    };
    created_secs <= now_secs.saturating_sub(RECONCILIATION_MIN_AGE_SECS)
}

fn unix_now_secs() -> Result<u64, StoreError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| {
            StoreError::InvalidData(format!("system clock before unix epoch: {error}"))
        })
}

fn batch_object_metadata_is_allowed(mime: &str, kind: &str, size_bytes: u64) -> bool {
    if size_bytes == 0 {
        return false;
    }
    if mime == JSON_MIME
        && matches!(
            kind,
            "camera-calibration"
                | "render-set-v2"
                | "reference-comparison-report"
                | "quality-report-v2"
                | PRODUCTION_WEAPON_FORM_ART_BASELINE_PARENT_OBJECT_KIND
                | PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_OBJECT_KIND
        )
    {
        return size_bytes <= MAX_JSON_BYTES;
    }
    mime == "image/png"
        && (kind.starts_with("render-pass-") || kind == "reference-silhouette-mask-v1")
        && size_bytes <= RECONCILIATION_MAX_VERIFY_BYTES
}

fn validate_batch_owner(
    owner: &ProductionWeaponFormArtBaselineCasBatchOwner,
) -> Result<(), StoreError> {
    if owner.schema_version != PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_SCHEMA_VERSION
        || owner.owner_kind != PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_KIND
        || !is_opaque_id(&owner.batch_id)
        || !is_opaque_id(&owner.baseline_id)
        || !is_opaque_id(&owner.registration_lineage_id)
        || !is_opaque_id(&owner.session_id)
        || !is_opaque_id(&owner.project_id)
        || !is_opaque_id(&owner.candidate_id)
        || !is_sha256(&owner.candidate_state_sha256)
        || !is_opaque_id(&owner.artifact_id)
        || !is_sha256(&owner.artifact_sha256)
        || !is_sha256(&owner.request_sha256)
        || !is_sha256(&owner.input_sha256)
        || !is_sha256(&owner.runtime_build_cohort_sha256)
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_INVALID",
            "baseline CAS batch owner identity or hash is invalid",
        ));
    }
    let created_at = owner.created_at.parse::<u64>().map_err(|_| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_INVALID",
            "baseline CAS batch created_at must be unix seconds",
        )
    })?;
    let expires_at = owner.expires_at.parse::<u64>().map_err(|_| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_INVALID",
            "baseline CAS batch expires_at must be unix seconds",
        )
    })?;
    if expires_at < created_at
        || expires_at.saturating_sub(created_at)
            > PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_TIMEOUT_SECS
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_INVALID",
            "baseline CAS batch expiry exceeds its bounded timeout",
        ));
    }
    Ok(())
}

fn fresh_batch_owner(
    baseline_id: &str,
    registration_lineage_id: &str,
    session_id: &str,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    artifact_id: &str,
    artifact_sha256: &str,
    request_sha256: &str,
    input_sha256: &str,
    runtime_build_cohort_sha256: &str,
) -> Result<ProductionWeaponFormArtBaselineCasBatchOwner, StoreError> {
    let created_at = unix_now_secs()?;
    let owner = ProductionWeaponFormArtBaselineCasBatchOwner {
        schema_version: PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_SCHEMA_VERSION.to_owned(),
        owner_kind: PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_KIND.to_owned(),
        batch_id: format!("form-art-baseline-batch-{}", Uuid::new_v4().simple()),
        baseline_id: baseline_id.to_owned(),
        registration_lineage_id: registration_lineage_id.to_owned(),
        session_id: session_id.to_owned(),
        project_id: project_id.to_owned(),
        candidate_id: candidate_id.to_owned(),
        candidate_state_sha256: candidate_state_sha256.to_owned(),
        artifact_id: artifact_id.to_owned(),
        artifact_sha256: artifact_sha256.to_owned(),
        request_sha256: request_sha256.to_owned(),
        input_sha256: input_sha256.to_owned(),
        runtime_build_cohort_sha256: runtime_build_cohort_sha256.to_owned(),
        created_at: created_at.to_string(),
        expires_at: created_at
            .saturating_add(PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_TIMEOUT_SECS)
            .to_string(),
    };
    validate_batch_owner(&owner)?;
    Ok(owner)
}

fn read_batch_owner(
    row: &Row<'_>,
) -> rusqlite::Result<(ProductionWeaponFormArtBaselineCasBatchOwner, String)> {
    Ok((
        ProductionWeaponFormArtBaselineCasBatchOwner {
            schema_version: row.get(0)?,
            owner_kind: row.get(1)?,
            batch_id: row.get(2)?,
            baseline_id: row.get(3)?,
            registration_lineage_id: row.get(4)?,
            session_id: row.get(5)?,
            project_id: row.get(6)?,
            candidate_id: row.get(7)?,
            candidate_state_sha256: row.get(8)?,
            artifact_id: row.get(9)?,
            artifact_sha256: row.get(10)?,
            request_sha256: row.get(11)?,
            input_sha256: row.get(12)?,
            runtime_build_cohort_sha256: row.get(13)?,
            created_at: row.get(14)?,
            expires_at: row.get(15)?,
        },
        row.get(16)?,
    ))
}

fn batch_owner_from_transaction(
    transaction: &rusqlite::Transaction<'_>,
    batch_id: &str,
) -> Result<Option<(ProductionWeaponFormArtBaselineCasBatchOwner, String)>, StoreError> {
    Ok(transaction
        .query_row(
            &format!(
                "SELECT schema_version, owner_kind, batch_id, baseline_id,
                        registration_lineage_id, session_id, project_id, candidate_id,
                        candidate_state_sha256, artifact_id, artifact_sha256, request_sha256,
                        input_sha256, runtime_build_cohort_sha256, created_at, expires_at, status
                   FROM {CAS_BATCH_TABLE}
                  WHERE batch_id = ?1"
            ),
            params![batch_id],
            read_batch_owner,
        )
        .optional()?)
}

fn batch_is_expired(owner: &ProductionWeaponFormArtBaselineCasBatchOwner, now_secs: u64) -> bool {
    let Some(created_at) = owner.created_at.parse::<u64>().ok() else {
        return false;
    };
    let Some(expires_at) = owner.expires_at.parse::<u64>().ok() else {
        return false;
    };
    expires_at <= now_secs && created_at <= now_secs
}

fn reservation_owns_hash(
    store: &Store,
    reservation: &super::CasReservation,
    sha256: &str,
) -> Result<bool, StoreError> {
    if !std::sync::Arc::ptr_eq(&reservation.reservations, &store.cas_reservations) {
        return Ok(false);
    }
    Ok(reservation
        .reservations
        .lock()
        .map_err(|_| StoreError::LockPoisoned)?
        .get(sha256)
        .is_some_and(|owners| owners.contains(&reservation.token)))
}

fn batch_has_live_peer_owner(
    transaction: &rusqlite::Transaction<'_>,
    sha256: &str,
    batch_id: &str,
    now_secs: u64,
) -> Result<bool, StoreError> {
    let mut statement = transaction.prepare(&format!(
        "SELECT b.created_at, b.expires_at
           FROM {CAS_BATCH_OBJECT_TABLE} o
           JOIN {CAS_BATCH_TABLE} b ON b.batch_id = o.batch_id
          WHERE o.object_sha256 = ?1
            AND o.batch_id != ?2
            AND o.status = 'temporary'
            AND b.status = 'open'"
    ))?;
    let rows = statement.query_map(params![sha256, batch_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (created_at, expires_at) = row?;
        let (Some(created_at), Some(expires_at)) = (
            created_at.parse::<u64>().ok(),
            expires_at.parse::<u64>().ok(),
        ) else {
            // Unknown timestamps are a live owner for safety.
            return Ok(true);
        };
        if created_at > now_secs || expires_at > now_secs {
            return Ok(true);
        }
    }
    Ok(false)
}

fn update_batch_object_status(
    transaction: &rusqlite::Transaction<'_>,
    batch_id: &str,
    sha256: &str,
    status: &str,
) -> Result<(), StoreError> {
    transaction.execute(
        &format!(
            "UPDATE {CAS_BATCH_OBJECT_TABLE}
                SET status = ?1
              WHERE batch_id = ?2 AND object_sha256 = ?3 AND status = 'temporary'"
        ),
        params![status, batch_id, sha256],
    )?;
    Ok(())
}

fn set_batch_object_status(
    transaction: &rusqlite::Transaction<'_>,
    batch_id: &str,
    sha256: &str,
    status: &str,
) -> Result<(), StoreError> {
    transaction.execute(
        &format!(
            "UPDATE {CAS_BATCH_OBJECT_TABLE}
                SET status = ?1
              WHERE batch_id = ?2 AND object_sha256 = ?3"
        ),
        params![status, batch_id, sha256],
    )?;
    Ok(())
}

fn batch_has_temporary_objects(
    transaction: &rusqlite::Transaction<'_>,
    batch_id: &str,
) -> Result<bool, StoreError> {
    Ok(transaction.query_row(
        &format!(
            "SELECT EXISTS (
                    SELECT 1 FROM {CAS_BATCH_OBJECT_TABLE}
                     WHERE batch_id = ?1 AND status = 'temporary'
                )"
        ),
        params![batch_id],
        |row| row.get(0),
    )?)
}

/// Return whether a CAS object is indexed by any FormArt batch.
///
/// This is intentionally conservative: a batch-local index is enough to
/// protect the content-addressed bytes from generic temporary-object cleanup,
/// regardless of the batch's current lifecycle status.  The query does not
/// create tables, so it is safe to use from the generic Store cleanup path
/// before the FormArt feature has ever been opened in a database.
pub(crate) fn cas_object_claimed_by_production_weapon_form_art_baseline_batch(
    store: &Store,
    sha256: &str,
) -> Result<bool, StoreError> {
    let connection = store.lock_connection()?;
    let object_table_exists: bool = connection.query_row(
        "SELECT EXISTS (
                SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = ?1
            )",
        params![CAS_BATCH_OBJECT_TABLE],
        |row| row.get(0),
    )?;
    if !object_table_exists {
        return Ok(false);
    }
    Ok(connection.query_row(
        &format!(
            "SELECT EXISTS (
                    SELECT 1 FROM {CAS_BATCH_OBJECT_TABLE}
                     WHERE object_sha256 = ?1
                )"
        ),
        params![sha256],
        |row| row.get(0),
    )?)
}

fn batch_has_live_peer_reservation(
    store: &Store,
    reservation_token: &str,
    sha256: &str,
) -> Result<bool, StoreError> {
    Ok(store
        .cas_reservations
        .lock()
        .map_err(|_| StoreError::LockPoisoned)?
        .get(sha256)
        .is_some_and(|owners| {
            owners
                .iter()
                .any(|owner| owner.as_str() != reservation_token)
        }))
}

/// Remove one batch's in-process token without touching any other operation's
/// reservation.  The durable batch/object rows remain as the audit trail.
fn release_batch_reservation_token(
    store: &Store,
    reservation_token: &str,
) -> Result<(), StoreError> {
    let mut reservations = store
        .cas_reservations
        .lock()
        .map_err(|_| StoreError::LockPoisoned)?;
    reservations.retain(|_, owners| {
        owners.remove(reservation_token);
        !owners.is_empty()
    });
    Ok(())
}

fn batch_owner_scope_matches_record(
    owner: &ProductionWeaponFormArtBaselineCasBatchOwner,
    record: &ProductionWeaponFormArtBaselineRecord,
) -> bool {
    owner.baseline_id == record.baseline_id
        && owner.registration_lineage_id == record.registration_lineage_id
        && owner.session_id == record.session_id
        && owner.project_id == record.project_id
        && owner.candidate_id == record.candidate_id
        && owner.candidate_state_sha256 == record.candidate_state_sha256
        && owner.artifact_id == record.artifact_id
        && owner.artifact_sha256 == record.artifact_sha256
        && owner.request_sha256 == record.request_sha256
        && owner.input_sha256 == record.input_sha256
        && owner.runtime_build_cohort_sha256 == record.runtime_build_cohort_sha256
}

fn preclaim_batch_object(
    store: &Store,
    batch: &ProductionWeaponFormArtBaselineCasBatch,
    sha256: &str,
    size_bytes: u64,
    mime: &str,
    kind: &str,
    created_at: &str,
) -> Result<CasObjectRecord, StoreError> {
    if !std::sync::Arc::ptr_eq(&batch.reservation.reservations, &store.cas_reservations) {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_SCOPE_DENIED",
            "baseline CAS batch belongs to a different Runtime Store",
        ));
    }
    if !reservation_owns_hash(store, &batch.reservation, sha256)? {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_RESERVATION_MISSING",
            "baseline CAS batch object is not held by its live reservation",
        ));
    }
    let _cas_mutation_guard = store
        .cas_mutation_lock
        .lock()
        .map_err(|_| StoreError::LockPoisoned)?;
    let mut connection = store.lock_connection()?;
    ensure_table(&connection)?;
    let transaction = connection.transaction()?;
    let Some((stored_owner, status)) =
        batch_owner_from_transaction(&transaction, batch.batch_id())?
    else {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_MISSING",
            "baseline CAS batch owner is not registered",
        ));
    };
    validate_batch_owner(&stored_owner)?;
    if stored_owner != *batch.owner() || status != CAS_BATCH_OPEN {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_MISMATCH",
            "baseline CAS batch owner or lifecycle status differs",
        ));
    }
    let registered: Option<(i64, String, String, String, String)> = transaction
        .query_row(
            "SELECT size_bytes, mime, kind, reachability, created_at
               FROM objects WHERE sha256 = ?1",
            params![sha256],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    let canonical_object = if let Some((
        registered_size,
        registered_mime,
        registered_kind,
        reachability,
        registered_created_at,
    )) = registered
    {
        if registered_size != i64::try_from(size_bytes).unwrap_or(i64::MAX)
            || registered_mime != mime
            || registered_kind != kind
        {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_MISMATCH",
                "registered CAS metadata differs from the batch object",
            ));
        }
        if !matches!(reachability.as_str(), "temporary" | "reachable") {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_MISMATCH",
                "only temporary or reachable CAS objects may enter a batch",
            ));
        }
        CasObjectRecord {
            schema_version: "CasObject@1".to_owned(),
            sha256: sha256.to_owned(),
            size_bytes: u64::try_from(registered_size).map_err(|_| {
                contract(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_MISMATCH",
                    "registered CAS object size is invalid",
                )
            })?,
            mime: registered_mime,
            kind: registered_kind,
            reachability,
            created_at: registered_created_at,
        }
    } else {
        CasObjectRecord {
            schema_version: "CasObject@1".to_owned(),
            sha256: sha256.to_owned(),
            size_bytes,
            mime: mime.to_owned(),
            kind: kind.to_owned(),
            reachability: "temporary".to_owned(),
            created_at: created_at.to_owned(),
        }
    };
    if canonical_object.reachability == "reachable" {
        // Already durable content needs no temporary batch claim. The normal
        // put path receives this exact registered metadata and releases this
        // operation's reservation after it adopts the durable object.
        return Ok(canonical_object);
    }
    let existing: Option<(i64, String, String, String, String)> = transaction
        .query_row(
            &format!(
                "SELECT size_bytes, mime, kind, status, created_at
                   FROM {CAS_BATCH_OBJECT_TABLE}
                  WHERE batch_id = ?1 AND object_sha256 = ?2"
            ),
            params![batch.batch_id(), sha256],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    if let Some((
        existing_size,
        existing_mime,
        existing_kind,
        existing_status,
        existing_created_at,
    )) = existing
    {
        if existing_size == i64::try_from(canonical_object.size_bytes).unwrap_or(i64::MAX)
            && existing_mime == canonical_object.mime
            && existing_kind == canonical_object.kind
            && existing_created_at == canonical_object.created_at
            && existing_status == "temporary"
        {
            return Ok(canonical_object);
        }
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_CONFLICT",
            "baseline CAS batch hash is already associated with different metadata",
        ));
    }
    transaction.execute(
        &format!(
            "INSERT INTO {CAS_BATCH_OBJECT_TABLE}
                (batch_id, object_sha256, size_bytes, mime, kind, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'temporary', ?6)"
        ),
        params![
            batch.batch_id(),
            sha256,
            i64::try_from(canonical_object.size_bytes).map_err(|_| {
                StoreError::InvalidData("baseline CAS batch object is too large".to_owned())
            })?,
            &canonical_object.mime,
            &canonical_object.kind,
            &canonical_object.created_at,
        ],
    )?;
    transaction.commit()?;
    Ok(canonical_object)
}

/// Close a failed FormArt batch immediately while preserving every byte and
/// every durable/peer owner.  The batch row is Store-local metadata: it is
/// allowed to become `quarantined`, but CAS objects are only ever moved from
/// `temporary` to `quarantined` here.  No filesystem deletion is performed.
fn abort_production_weapon_form_art_baseline_cas_batch(
    store: &Store,
    batch: &ProductionWeaponFormArtBaselineCasBatch,
) -> Result<(), StoreError> {
    if !std::sync::Arc::ptr_eq(&batch.reservation.reservations, &store.cas_reservations) {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_SCOPE_DENIED",
            "baseline CAS batch belongs to a different Runtime Store",
        ));
    }
    validate_batch_owner(batch.owner())?;
    let now_secs = unix_now_secs()?;
    let _cas_mutation_guard = store
        .cas_mutation_lock
        .lock()
        .map_err(|_| StoreError::LockPoisoned)?;
    let mut connection = store.lock_connection()?;
    ensure_table(&connection)?;
    let transaction = connection.transaction()?;
    let Some((stored_owner, lifecycle)) =
        batch_owner_from_transaction(&transaction, batch.batch_id())?
    else {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_MISSING",
            "baseline CAS batch owner is not registered",
        ));
    };
    validate_batch_owner(&stored_owner)?;
    if stored_owner != *batch.owner() {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_MISMATCH",
            "baseline CAS batch owner differs from persisted identity",
        ));
    }

    // A completed or previously quarantined owner is terminal.  Releasing
    // this handle's in-process token is safe because committed/reachable
    // content is never eligible for temporary cleanup.
    if lifecycle != CAS_BATCH_OPEN {
        transaction.commit()?;
        release_batch_reservation_token(store, &batch.reservation.token)?;
        return Ok(());
    }

    let objects = {
        let mut statement = transaction.prepare(&format!(
            "SELECT object_sha256, size_bytes, mime, kind, status, created_at
               FROM {CAS_BATCH_OBJECT_TABLE}
              WHERE batch_id = ?1
              ORDER BY created_at ASC, object_sha256 ASC"
        ))?;
        let rows = statement.query_map(params![batch.batch_id()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    for (sha256, size_bytes, mime, kind, status, created_at) in objects {
        // Terminal object metadata is never demoted.  In particular, a
        // linked/reachable row can be observed while a failed caller is
        // unwinding and must remain durable.
        if status != "temporary" {
            continue;
        }

        let Ok(size_bytes_u64) = u64::try_from(size_bytes) else {
            update_batch_object_status(&transaction, batch.batch_id(), &sha256, "corrupt")?;
            continue;
        };
        if !is_sha256(&sha256)
            || !batch_object_metadata_is_allowed(&mime, &kind, size_bytes_u64)
            || created_at.parse::<u64>().is_err()
        {
            update_batch_object_status(&transaction, batch.batch_id(), &sha256, "corrupt")?;
            continue;
        }

        let registered: Option<(i64, String, String, String, String)> = transaction
            .query_row(
                "SELECT size_bytes, mime, kind, reachability, created_at
                   FROM objects WHERE sha256 = ?1",
                params![sha256],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            registered_size,
            registered_mime,
            registered_kind,
            reachability,
            registered_created_at,
        )) = registered
        else {
            update_batch_object_status(&transaction, batch.batch_id(), &sha256, "missing")?;
            continue;
        };
        if registered_size != size_bytes
            || registered_mime != mime
            || registered_kind != kind
            || registered_created_at != created_at
        {
            update_batch_object_status(&transaction, batch.batch_id(), &sha256, "corrupt")?;
            continue;
        }

        if reachability == "reachable" {
            // A durable object wins over this failed batch, even if the
            // shared link query has not yet observed its owning row.
            update_batch_object_status(&transaction, batch.batch_id(), &sha256, "linked")?;
            continue;
        }
        if reachability == "quarantined" {
            update_batch_object_status(&transaction, batch.batch_id(), &sha256, "quarantined")?;
            continue;
        }
        if reachability != "temporary" {
            update_batch_object_status(&transaction, batch.batch_id(), &sha256, "corrupt")?;
            continue;
        }

        // A durable link may have won after the batch claim.  Promote only
        // that exact object row to reachable; never demote a link or delete
        // its bytes.
        if super::authoring_mesh_edit_object_is_linked(&transaction, &sha256)? {
            transaction.execute(
                "UPDATE objects
                    SET reachability = 'reachable'
                  WHERE sha256 = ?1
                    AND size_bytes = ?2
                    AND mime = ?3
                    AND kind = ?4
                    AND created_at = ?5
                    AND reachability = 'temporary'",
                params![sha256, size_bytes, mime, kind, created_at],
            )?;
            update_batch_object_status(&transaction, batch.batch_id(), &sha256, "linked")?;
            continue;
        }

        // Keep a peer's live reservation or durable batch owner untouched.
        // This batch's claim is closed, but the shared object remains
        // temporary until the surviving owner commits or explicitly aborts.
        if batch_has_live_peer_reservation(store, &batch.reservation.token, &sha256)?
            || batch_has_live_peer_owner(&transaction, &sha256, batch.batch_id(), now_secs)?
        {
            update_batch_object_status(&transaction, batch.batch_id(), &sha256, "quarantined")?;
            continue;
        }

        // This is the only object-state transition made by abort: metadata
        // quarantine. The CAS file is intentionally left in place for
        // diagnosis/replay and is never removed here.
        transaction.execute(
            "UPDATE objects
                SET reachability = 'quarantined'
              WHERE sha256 = ?1
                AND size_bytes = ?2
                AND mime = ?3
                AND kind = ?4
                AND created_at = ?5
                AND reachability = 'temporary'",
            params![sha256, size_bytes, mime, kind, created_at],
        )?;
        update_batch_object_status(&transaction, batch.batch_id(), &sha256, "quarantined")?;
    }

    // Closing the failed owner does not close or mutate another batch that may
    // still own one of these hashes. The surviving owner remains queryable and
    // can finish its own commit path.
    transaction.execute(
        &format!(
            "UPDATE {CAS_BATCH_TABLE}
                SET status = ?1, completed_at = ?2
              WHERE batch_id = ?3 AND status = 'open'"
        ),
        params![
            CAS_BATCH_QUARANTINED,
            now_secs.to_string(),
            batch.batch_id()
        ],
    )?;
    transaction.commit()?;
    release_batch_reservation_token(store, &batch.reservation.token)?;
    Ok(())
}

/// Quarantine stale, unlinked FormArt objects left by a crashed prepare. This
/// is metadata-only reconciliation: it never walks the CAS filesystem and
/// never removes bytes. Only objects explicitly indexed by this Store-local
/// owner/batch are considered; unowned temporary rows are retained.
pub(crate) fn reconcile_stale_cas_receipts(store: &Store) -> Result<(), StoreError> {
    let now_secs = unix_now_secs()?;

    let _cas_mutation_guard = store
        .cas_mutation_lock
        .lock()
        .map_err(|_| StoreError::LockPoisoned)?;
    let mut connection = store.lock_connection()?;
    ensure_table(&connection)?;
    let transaction = connection.transaction()?;
    let batches = {
        let mut statement = transaction.prepare(&format!(
            "SELECT schema_version, owner_kind, batch_id, baseline_id,
                    registration_lineage_id, session_id, project_id, candidate_id,
                    candidate_state_sha256, artifact_id, artifact_sha256, request_sha256,
                    input_sha256, runtime_build_cohort_sha256, created_at, expires_at, status
               FROM {CAS_BATCH_TABLE}
              WHERE status = 'open'
                AND schema_version = ?1
                AND owner_kind = ?2
              ORDER BY expires_at ASC, batch_id ASC
              LIMIT ?3"
        ))?;
        let rows = statement.query_map(
            params![
                PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_SCHEMA_VERSION,
                PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_KIND,
                RECONCILIATION_MAX_OBJECTS,
            ],
            read_batch_owner,
        )?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut reconciled_objects = 0i64;
    for (owner, status) in batches {
        if status != CAS_BATCH_OPEN
            || validate_batch_owner(&owner).is_err()
            || !batch_is_expired(&owner, now_secs)
            || !reconciliation_object_is_old_enough(&owner.created_at, now_secs)
        {
            continue;
        }
        let candidates = {
            let mut statement = transaction.prepare(&format!(
                "SELECT object_sha256, size_bytes, mime, kind, created_at
                   FROM {CAS_BATCH_OBJECT_TABLE}
                  WHERE batch_id = ?1 AND status = 'temporary'
                  ORDER BY created_at ASC, object_sha256 ASC
                  LIMIT ?2"
            ))?;
            let rows = statement.query_map(
                params![
                    owner.batch_id,
                    RECONCILIATION_MAX_OBJECTS - reconciled_objects
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for (sha256, size_bytes, mime, kind, created_at) in candidates {
            if reconciled_objects >= RECONCILIATION_MAX_OBJECTS {
                break;
            }
            let Ok(size_bytes) = u64::try_from(size_bytes) else {
                continue;
            };
            if !is_sha256(&sha256)
                || !batch_object_metadata_is_allowed(&mime, &kind, size_bytes)
                || !reconciliation_object_is_old_enough(&created_at, now_secs)
                || store.has_cas_reservation_locked(&sha256)?
                || batch_has_live_peer_owner(&transaction, &sha256, &owner.batch_id, now_secs)?
            {
                continue;
            }

            // A receipt can become linked after it was indexed but before a
            // process dies. The shared query covers every durable Store link,
            // including a newly committed FormArt baseline.
            if super::authoring_mesh_edit_object_is_linked(&transaction, &sha256)? {
                update_batch_object_status(&transaction, &owner.batch_id, &sha256, "linked")?;
                continue;
            }
            let metadata: Option<(i64, String, String, String, String)> = transaction
                .query_row(
                    "SELECT size_bytes, mime, kind, reachability, created_at
                       FROM objects WHERE sha256 = ?1",
                    params![sha256],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?;
            let Some((
                registered_size,
                registered_mime,
                registered_kind,
                reachability,
                registered_created_at,
            )) = metadata
            else {
                update_batch_object_status(&transaction, &owner.batch_id, &sha256, "missing")?;
                continue;
            };
            if registered_size != i64::try_from(size_bytes).unwrap_or(i64::MAX)
                || registered_mime != mime
                || registered_kind != kind
                || registered_created_at != created_at
            {
                update_batch_object_status(&transaction, &owner.batch_id, &sha256, "corrupt")?;
                continue;
            }
            if reachability != "temporary" {
                if reachability == "reachable" {
                    // The durable link won the race after the claim was
                    // written. Keep the bytes durable and close only the
                    // Store-local claim.
                    update_batch_object_status(&transaction, &owner.batch_id, &sha256, "linked")?;
                }
                continue;
            }
            if store.cas.verify(&sha256, size_bytes).is_err() {
                // Corrupt/missing bytes are retained for diagnosis; this
                // repair is fail-closed.
                update_batch_object_status(&transaction, &owner.batch_id, &sha256, "corrupt")?;
                continue;
            }
            let updated = transaction.execute(
                "UPDATE objects
                    SET reachability = 'quarantined'
                  WHERE sha256 = ?1
                    AND size_bytes = ?2
                    AND mime = ?3
                    AND kind = ?4
                    AND created_at = ?5
                    AND reachability = 'temporary'",
                params![
                    sha256,
                    i64::try_from(size_bytes).map_err(|_| {
                        StoreError::InvalidData("reconciliation object size overflow".to_owned())
                    })?,
                    mime,
                    kind,
                    created_at,
                ],
            )?;
            if updated == 1 {
                update_batch_object_status(&transaction, &owner.batch_id, &sha256, "quarantined")?;
                reconciled_objects += 1;
            }
        }
        if !batch_has_temporary_objects(&transaction, &owner.batch_id)? {
            transaction.execute(
                &format!(
                    "UPDATE {CAS_BATCH_TABLE}
                        SET status = ?1, completed_at = ?2
                      WHERE batch_id = ?3 AND status = 'open'"
                ),
                params![CAS_BATCH_QUARANTINED, now_secs.to_string(), owner.batch_id],
            )?;
        }
    }
    transaction.commit()?;
    Ok(())
}

fn value<T: Serialize>(value: &T) -> Result<Value, StoreError> {
    serde_json::to_value(value).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn canonical_bytes<T: Serialize>(input: &T) -> Result<Vec<u8>, StoreError> {
    canonical_json_bytes(&value(input)?).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn canonical_json<T: Serialize>(input: &T) -> Result<String, StoreError> {
    String::from_utf8(canonical_bytes(input)?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn normalized_view_value(view: &ProductionWeaponFormArtBaselineView) -> Result<Value, StoreError> {
    let mut value = value(view)?;
    value["receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

fn normalized_record_value(
    record: &ProductionWeaponFormArtBaselineRecord,
) -> Result<Value, StoreError> {
    let mut value = value(record)?;
    value["receipt_object_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    if let Some(views) = value.get_mut("views").and_then(Value::as_array_mut) {
        for view in views {
            view["receipt_object_sha256"] = Value::String(String::new());
            view["canonical_sha256"] = Value::String(String::new());
        }
    }
    Ok(value)
}

fn parent_payload_bytes(
    record: &ProductionWeaponFormArtBaselineRecord,
) -> Result<Vec<u8>, StoreError> {
    let mut value = value(record)?;
    // The receipt hash points at this object, so it is intentionally empty in
    // the immutable parent payload.  Child receipt hashes remain explicit.
    value["receipt_object_sha256"] = Value::String(String::new());
    let bytes =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_PARENT_TOO_LARGE",
            "baseline parent receipt exceeds its bounded JSON size",
        ));
    }
    Ok(bytes)
}

fn view_payload_bytes(view: &ProductionWeaponFormArtBaselineView) -> Result<Vec<u8>, StoreError> {
    let mut value = value(view)?;
    value["receipt_object_sha256"] = Value::String(String::new());
    let bytes =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_TOO_LARGE",
            "baseline view receipt exceeds its bounded JSON size",
        ));
    }
    Ok(bytes)
}

fn validate_view(
    record: &ProductionWeaponFormArtBaselineRecord,
    view: &ProductionWeaponFormArtBaselineView,
    expected_kind: &str,
) -> Result<(), StoreError> {
    if view.schema_version != PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_SCHEMA_VERSION
        || view.view_kind != expected_kind
        || !is_opaque_id(&view.view_id)
        || !is_opaque_id(&view.reference_id)
        || !is_sha256(&view.reference_sha256)
        || !is_sha256(&view.camera_hash)
        || !is_sha256(&view.camera_canonical_sha256)
        || !is_sha256(&view.camera_object_sha256)
        || !is_opaque_id(&view.render_set_id)
        || !is_sha256(&view.render_set_object_sha256)
        || !is_sha256(&view.render_set_canonical_sha256)
        || !is_opaque_id(&view.render_set_view_id)
        || view.pass_artifact_object_sha256.len()
            != PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS.len()
        || view
            .pass_artifact_object_sha256
            .iter()
            .any(|hash| !is_sha256(hash))
        || !is_sha256(&view.reference_mask_object_sha256)
        || !is_sha256(&view.comparison_report_object_sha256)
        || !is_sha256(&view.quality_report_object_sha256)
        || !is_sha256(&view.render_worker_build_cohort_sha256)
        || view.render_worker_build_cohort_sha256 != record.runtime_build_cohort_sha256
        || view.quality_status != PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS
        || !is_sha256(&view.receipt_object_sha256)
        || !is_sha256(&view.canonical_sha256)
        || view.created_at.is_empty()
        || view.created_at.len() > 128
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_INVALID",
            format!("baseline view {expected_kind} is malformed or out of scope"),
        ));
    }
    if canonical_json_hash(&normalized_view_value(view)?) != view.canonical_sha256 {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_CANONICAL_MISMATCH",
            format!("baseline view {expected_kind} canonical hash is not reproducible"),
        ));
    }
    Ok(())
}

fn validate_record_shape(record: &ProductionWeaponFormArtBaselineRecord) -> Result<(), StoreError> {
    let ids = [
        record.baseline_id.as_str(),
        record.registration_lineage_id.as_str(),
        record.registered_rig_v2_id.as_str(),
        record.session_id.as_str(),
        record.project_id.as_str(),
        record.candidate_id.as_str(),
        record.artifact_id.as_str(),
        record.idempotency_key.as_str(),
    ];
    let hashes = [
        record.registration_lineage_canonical_sha256.as_str(),
        record.registration_lineage_receipt_object_sha256.as_str(),
        record.registered_rig_v2_object_sha256.as_str(),
        record.registered_rig_v2_canonical_sha256.as_str(),
        record.candidate_state_sha256.as_str(),
        record.artifact_sha256.as_str(),
        record.runtime_build_cohort_sha256.as_str(),
        record.request_sha256.as_str(),
        record.input_sha256.as_str(),
        record.receipt_object_sha256.as_str(),
        record.canonical_sha256.as_str(),
    ];
    if record.schema_version != PRODUCTION_WEAPON_FORM_ART_BASELINE_SCHEMA_VERSION
        || ids.iter().any(|value| !is_opaque_id(value))
        || hashes.iter().any(|value| !is_sha256(value))
        || record
            .base_version_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || record.view_kinds.len() != PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS.len()
        || record.views.len() != PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS.len()
        || record.runtime_build_cohort_sha256.is_empty()
        || record.baseline_policy != PRODUCTION_WEAPON_FORM_ART_BASELINE_POLICY
        || record.materialization_status
            != PRODUCTION_WEAPON_FORM_ART_BASELINE_MATERIALIZATION_STATUS
        || record.historical_form_art_reused
        || !record.worker_started
        || !record.worker_cohort_verified
        || record.quality_status != PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_STATUS
        || record.visual_status != "NOT_PROVEN"
        || record.human_status != "NOT_RUN"
        || record.engine_status != "NOT_RUN"
        || record.distribution_status != "NOT_RUN"
        || record.promotion_eligible
        || !record.runtime_write_performed
        || !record.persistent_user_data_touched
        || record.production_stage_advanced
        || record.candidate_confirmed
        || record.version_created
        || record.export_performed
        || record.idempotency_policy != PRODUCTION_WEAPON_FORM_ART_BASELINE_IDEMPOTENCY_POLICY
        || record.writer_policy != PRODUCTION_WEAPON_FORM_ART_BASELINE_WRITER_POLICY
        || record.canonicalization_policy
            != PRODUCTION_WEAPON_FORM_ART_BASELINE_CANONICALIZATION_POLICY
        || record.created_at.is_empty()
        || record.created_at.len() > 128
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_RECORD_INVALID",
            "baseline record identity, status, policy, hash or six-view shape is invalid",
        ));
    }
    if record.view_kinds.iter().map(String::as_str).ne(
        PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS
            .iter()
            .copied(),
    ) {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_SET_INVALID",
            "baseline views must use the fixed six-view order",
        ));
    }
    let mut view_ids = HashSet::new();
    for (view, expected_kind) in record
        .views
        .iter()
        .zip(PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_KINDS.iter())
    {
        if !view_ids.insert(view.view_id.as_str()) {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_DUPLICATE",
                "baseline view_id is duplicated",
            ));
        }
        validate_view(record, view, expected_kind)?;
    }
    if canonical_json_hash(&normalized_record_value(record)?) != record.canonical_sha256 {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CANONICAL_MISMATCH",
            "baseline record canonical hash is not reproducible",
        ));
    }
    Ok(())
}

fn validate_object(
    store: &Store,
    object: &CasObjectRecord,
    expected_sha256: &str,
    expected_kind: &str,
    expected_payload: Option<&[u8]>,
) -> Result<(), StoreError> {
    if object.schema_version != "CasObject@1"
        || object.sha256 != expected_sha256
        || !is_sha256(expected_sha256)
        || object.mime != JSON_MIME
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_JSON_BYTES
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_METADATA_MISMATCH",
            "baseline CAS object metadata differs from its binding",
        ));
    }
    let current = store.get_object(expected_sha256)?.ok_or_else(|| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_MISSING",
            "baseline CAS object is not registered",
        )
    })?;
    if current.sha256 != object.sha256
        || current.size_bytes != object.size_bytes
        || current.mime != object.mime
        || current.kind != object.kind
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_METADATA_MISMATCH",
            "registered baseline CAS metadata differs from the supplied object",
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(expected_sha256, MAX_JSON_BYTES)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != expected_sha256 {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_HASH_MISMATCH",
            "baseline CAS bytes do not match their content hash",
        ));
    }
    serde_json::from_slice::<Value>(&bytes).map_err(|error| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_JSON_INVALID",
            format!("baseline CAS object is not valid JSON: {error}"),
        )
    })?;
    if let Some(expected_payload) = expected_payload {
        if bytes != expected_payload {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_RECEIPT_BINDING_MISMATCH",
                "baseline receipt bytes differ from their canonical contract payload",
            ));
        }
    }
    Ok(())
}

fn read_bound_json_object(
    store: &Store,
    sha256: &str,
    expected_kind: &str,
) -> Result<Value, StoreError> {
    let object = store.get_object(sha256)?.ok_or_else(|| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_DERIVED_CAS_MISSING",
            format!("baseline derived object {expected_kind} is not registered"),
        )
    })?;
    if object.sha256 != sha256
        || object.mime != JSON_MIME
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_JSON_BYTES
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_DERIVED_CAS_METADATA_MISMATCH",
            format!("baseline derived object {expected_kind} metadata is invalid"),
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(sha256, MAX_JSON_BYTES)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != sha256 {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_DERIVED_CAS_HASH_MISMATCH",
            format!("baseline derived object {expected_kind} bytes do not match its hash"),
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_DERIVED_CAS_JSON_INVALID",
            format!("baseline derived object {expected_kind} is not JSON: {error}"),
        )
    })
}

fn validate_bound_png_object(
    store: &Store,
    sha256: &str,
    expected_kind: &str,
) -> Result<(), StoreError> {
    let object = store.get_object(sha256)?.ok_or_else(|| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_DERIVED_CAS_MISSING",
            format!("baseline derived object {expected_kind} is not registered"),
        )
    })?;
    if object.sha256 != sha256
        || object.mime != "image/png"
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > 64 * 1024 * 1024
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_DERIVED_CAS_METADATA_MISMATCH",
            format!("baseline derived object {expected_kind} metadata is invalid"),
        ));
    }
    let bytes = store
        .cas
        .read_verified_bounded(sha256, 64 * 1024 * 1024)
        .map_err(StoreError::from)?;
    if bytes.len() as u64 != object.size_bytes || sha256_hex(&bytes) != sha256 {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_DERIVED_CAS_HASH_MISMATCH",
            format!("baseline derived object {expected_kind} bytes do not match its hash"),
        ));
    }
    Ok(())
}

fn required_json_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, StoreError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_DERIVED_BINDING_MISMATCH",
            format!("baseline derived object is missing {field}"),
        )
    })
}

fn validate_view_evidence(
    store: &Store,
    baseline: &ProductionWeaponFormArtBaselineRecord,
    view: &ProductionWeaponFormArtBaselineView,
) -> Result<(), StoreError> {
    let camera = read_bound_json_object(store, &view.camera_object_sha256, "camera-calibration")?;
    if required_json_str(&camera, "camera_hash")? != view.camera_hash
        || required_json_str(&camera, "canonical_sha256")? != view.camera_canonical_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAMERA_BINDING_MISMATCH",
            "baseline view camera object differs from its public hashes",
        ));
    }

    let render_set =
        read_bound_json_object(store, &view.render_set_object_sha256, "render-set-v2")?;
    if required_json_str(&render_set, "candidate_id")? != baseline.candidate_id
        || required_json_str(&render_set, "artifact_sha256")? != baseline.artifact_sha256
        || required_json_str(&render_set, "render_set_id")? != view.render_set_id
        || required_json_str(&render_set, "canonical_sha256")? != view.render_set_canonical_sha256
        || required_json_str(&render_set, "view_id")? != view.render_set_view_id
        || required_json_str(&render_set, "camera_hash")? != view.camera_hash
        || required_json_str(&render_set, "camera_object_sha256")? != view.camera_object_sha256
        || required_json_str(&render_set, "reference_id")? != view.reference_id
        || required_json_str(&render_set, "render_worker_build_cohort_sha256")?
            != view.render_worker_build_cohort_sha256
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_RENDER_SET_BINDING_MISMATCH",
            "baseline view RenderSet differs from its public hashes or scope",
        ));
    }
    let passes = render_set
        .get("passes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_BINDING_MISMATCH",
                "baseline RenderSet has no fixed pass list",
            )
        })?;
    let artifacts = render_set
        .get("pass_artifacts")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_BINDING_MISMATCH",
                "baseline RenderSet has no pass artifact map",
            )
        })?;
    if passes.len() != PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS.len() {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_BINDING_MISMATCH",
            "baseline RenderSet does not contain exactly nine AOVs",
        ));
    }
    for ((pass, expected_kind), expected_sha256) in passes
        .iter()
        .zip(PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_KINDS.iter())
        .zip(view.pass_artifact_object_sha256.iter())
    {
        if pass.as_str() != Some(expected_kind)
            || artifacts
                .get(*expected_kind)
                .and_then(|artifact| artifact.get("sha256"))
                .and_then(Value::as_str)
                != Some(expected_sha256)
        {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_AOV_BINDING_MISMATCH",
                format!("baseline AOV {expected_kind} differs from its public hash"),
            ));
        }
        validate_bound_png_object(
            store,
            expected_sha256,
            &format!("render-pass-{expected_kind}"),
        )?;
    }
    validate_bound_png_object(
        store,
        &view.reference_mask_object_sha256,
        "reference-silhouette-mask-v1",
    )?;

    let comparison = read_bound_json_object(
        store,
        &view.comparison_report_object_sha256,
        "reference-comparison-report",
    )?;
    if required_json_str(&comparison, "candidate_id")? != baseline.candidate_id
        || required_json_str(&comparison, "artifact_sha256")? != baseline.artifact_sha256
        || required_json_str(&comparison, "render_set_hash")? != view.render_set_object_sha256
        || required_json_str(&comparison, "reference_id")? != view.reference_id
        || required_json_str(&comparison, "reference_sha256")? != view.reference_sha256
        || required_json_str(&comparison, "camera_hash")? != view.camera_hash
        || required_json_str(&comparison, "view_id")? != view.view_id
        || comparison
            .get("mask")
            .and_then(|mask| mask.get("sha256"))
            .and_then(Value::as_str)
            != Some(view.reference_mask_object_sha256.as_str())
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_COMPARISON_BINDING_MISMATCH",
            "baseline comparison report differs from its public hashes or scope",
        ));
    }
    let quality = read_bound_json_object(
        store,
        &view.quality_report_object_sha256,
        "quality-report-v2",
    )?;
    if required_json_str(&quality, "candidate_id")? != baseline.candidate_id
        || required_json_str(&quality, "artifact_sha256")? != baseline.artifact_sha256
        || required_json_str(&quality, "render_set_hash")? != view.render_set_object_sha256
        || required_json_str(&quality, "comparison_report_hash")?
            != view.comparison_report_object_sha256
        || required_json_str(&quality, "reference_id")? != view.reference_id
        || required_json_str(&quality, "reference_sha256")? != view.reference_sha256
        || required_json_str(&quality, "view_id")? != view.view_id
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_QUALITY_BINDING_MISMATCH",
            "baseline quality report differs from its public hashes or scope",
        ));
    }
    Ok(())
}

fn validate_bundle(
    store: &Store,
    bundle: &ProductionWeaponFormArtBaselineCommitBundle,
) -> Result<(), StoreError> {
    let record = &bundle.baseline;
    validate_record_shape(record)?;
    if bundle.baseline_view_receipt_objects.len() != record.views.len() {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_COUNT_MISMATCH",
            "baseline commit must supply exactly six view receipt objects",
        ));
    }
    let parent_payload = parent_payload_bytes(record)?;
    validate_object(
        store,
        &bundle.baseline_parent_receipt_object,
        &record.receipt_object_sha256,
        PRODUCTION_WEAPON_FORM_ART_BASELINE_PARENT_OBJECT_KIND,
        Some(&parent_payload),
    )?;
    for (view, object) in record
        .views
        .iter()
        .zip(bundle.baseline_view_receipt_objects.iter())
    {
        let payload = view_payload_bytes(view)?;
        validate_object(
            store,
            object,
            &view.receipt_object_sha256,
            PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_OBJECT_KIND,
            Some(&payload),
        )?;
    }
    validate_object(
        store,
        &bundle.registration_lineage_receipt_object,
        &record.registration_lineage_receipt_object_sha256,
        PRODUCTION_WEAPON_FORM_ART_BASELINE_LINEAGE_RECEIPT_OBJECT_KIND,
        None,
    )?;
    validate_object(
        store,
        &bundle.registered_rig_v2_object,
        &record.registered_rig_v2_object_sha256,
        PRODUCTION_WEAPON_FORM_ART_BASELINE_RIG_V2_OBJECT_KIND,
        None,
    )?;
    for view in &record.views {
        validate_view_evidence(store, record, view)?;
    }
    Ok(())
}

fn read_record(row: &Row<'_>) -> rusqlite::Result<ProductionWeaponFormArtBaselineRecord> {
    let payload: String = row.get(0)?;
    serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn roots(record: &ProductionWeaponFormArtBaselineRecord) -> Vec<String> {
    let mut roots = vec![
        record.registration_lineage_receipt_object_sha256.clone(),
        record.registered_rig_v2_object_sha256.clone(),
        record.receipt_object_sha256.clone(),
    ];
    roots.extend(
        record
            .views
            .iter()
            .map(|view| view.receipt_object_sha256.clone()),
    );
    for view in &record.views {
        roots.extend([
            view.camera_object_sha256.clone(),
            view.render_set_object_sha256.clone(),
            view.reference_mask_object_sha256.clone(),
            view.comparison_report_object_sha256.clone(),
            view.quality_report_object_sha256.clone(),
        ]);
        roots.extend(view.pass_artifact_object_sha256.iter().cloned());
    }
    roots.sort();
    roots.dedup();
    roots
}

fn require_reachable_roots(
    store: &Store,
    record: &ProductionWeaponFormArtBaselineRecord,
) -> Result<(), StoreError> {
    for sha256 in roots(record) {
        let object = store.get_object(&sha256)?.ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_MISSING",
                "persisted baseline root is not registered",
            )
        })?;
        if object.reachability != "reachable" {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_ROOT_NOT_REACHABLE",
                format!("persisted baseline root {sha256} is not reachable"),
            ));
        }
    }
    Ok(())
}

fn validate_persisted_objects(
    store: &Store,
    record: &ProductionWeaponFormArtBaselineRecord,
) -> Result<(), StoreError> {
    require_reachable_roots(store, record)?;
    let parent_payload = parent_payload_bytes(record)?;
    let parent = store
        .get_object(&record.receipt_object_sha256)?
        .ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_MISSING",
                "persisted baseline parent receipt is not registered",
            )
        })?;
    validate_object(
        store,
        &parent,
        &record.receipt_object_sha256,
        PRODUCTION_WEAPON_FORM_ART_BASELINE_PARENT_OBJECT_KIND,
        Some(&parent_payload),
    )?;
    for view in &record.views {
        let object = store
            .get_object(&view.receipt_object_sha256)?
            .ok_or_else(|| {
                contract(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_MISSING",
                    "persisted baseline view receipt is not registered",
                )
            })?;
        let payload = view_payload_bytes(view)?;
        validate_object(
            store,
            &object,
            &view.receipt_object_sha256,
            PRODUCTION_WEAPON_FORM_ART_BASELINE_VIEW_OBJECT_KIND,
            Some(&payload),
        )?;
        validate_view_evidence(store, record, view)?;
    }
    for (hash, kind) in [
        (
            record.registration_lineage_receipt_object_sha256.as_str(),
            PRODUCTION_WEAPON_FORM_ART_BASELINE_LINEAGE_RECEIPT_OBJECT_KIND,
        ),
        (
            record.registered_rig_v2_object_sha256.as_str(),
            PRODUCTION_WEAPON_FORM_ART_BASELINE_RIG_V2_OBJECT_KIND,
        ),
    ] {
        let object = store.get_object(hash)?.ok_or_else(|| {
            contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_MISSING",
                "persisted baseline lineage CAS object is not registered",
            )
        })?;
        validate_object(store, &object, hash, kind, None)?;
    }
    Ok(())
}

fn validate_candidate_binding_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    record: &ProductionWeaponFormArtBaselineRecord,
) -> Result<(), StoreError> {
    let candidate: Option<(
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = transaction
        .query_row(
            "SELECT project_id, canonical_sha256, base_version_id, prepared_object_id,
                    prepared_object_sha256, manifest_hash
               FROM candidates WHERE candidate_id = ?1",
            params![record.candidate_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()?;
    let Some((project_id, state_sha256, base_version_id, artifact_id, artifact_sha256, manifest)) =
        candidate
    else {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CANDIDATE_BINDING_MISSING",
            "baseline candidate disappeared before durable commit",
        ));
    };
    if project_id != record.project_id
        || state_sha256 != record.candidate_state_sha256
        || base_version_id != record.base_version_id
        || artifact_id.as_deref() != Some(record.artifact_id.as_str())
        || artifact_sha256.as_deref() != Some(record.artifact_sha256.as_str())
        || manifest
            .as_deref()
            .is_some_and(|value| value != record.artifact_sha256)
    {
        return Err(contract(
            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CANDIDATE_BINDING_MISMATCH",
            "baseline candidate, state, version or artifact drifted before durable commit",
        ));
    }
    Ok(())
}

impl Store {
    /// Begin a fresh baseline CAS batch. The owner is persisted before the
    /// first derived object is written; the returned reservation is scoped to
    /// this Store and disappears automatically when the process drops it.
    pub fn begin_production_weapon_form_art_baseline_cas_batch(
        &self,
        owner: ProductionWeaponFormArtBaselineCasBatchOwner,
    ) -> Result<ProductionWeaponFormArtBaselineCasBatch, StoreError> {
        validate_batch_owner(&owner)?;
        let reservation = self.begin_cas_reservation();
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        connection.execute(
            &format!(
                "INSERT INTO {CAS_BATCH_TABLE} (
                    batch_id, schema_version, owner_kind, baseline_id,
                    registration_lineage_id, session_id, project_id, candidate_id,
                    candidate_state_sha256, artifact_id, artifact_sha256,
                    request_sha256, input_sha256, runtime_build_cohort_sha256,
                    created_at, expires_at, status, completed_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                          ?12, ?13, ?14, ?15, ?16, ?17, NULL)"
            ),
            params![
                owner.batch_id,
                owner.schema_version,
                owner.owner_kind,
                owner.baseline_id,
                owner.registration_lineage_id,
                owner.session_id,
                owner.project_id,
                owner.candidate_id,
                owner.candidate_state_sha256,
                owner.artifact_id,
                owner.artifact_sha256,
                owner.request_sha256,
                owner.input_sha256,
                owner.runtime_build_cohort_sha256,
                owner.created_at,
                owner.expires_at,
                CAS_BATCH_OPEN,
            ],
        )?;
        Ok(ProductionWeaponFormArtBaselineCasBatch {
            owner,
            reservation,
            store: self.clone(),
        })
    }

    /// Convenience constructor for Runtime. It keeps timestamp and batch-id
    /// generation inside Store while retaining the same typed owner fields.
    #[allow(clippy::too_many_arguments)]
    pub fn begin_production_weapon_form_art_baseline_cas_batch_for_prepare(
        &self,
        baseline_id: &str,
        registration_lineage_id: &str,
        session_id: &str,
        project_id: &str,
        candidate_id: &str,
        candidate_state_sha256: &str,
        artifact_id: &str,
        artifact_sha256: &str,
        request_sha256: &str,
        input_sha256: &str,
        runtime_build_cohort_sha256: &str,
    ) -> Result<ProductionWeaponFormArtBaselineCasBatch, StoreError> {
        let owner = fresh_batch_owner(
            baseline_id,
            registration_lineage_id,
            session_id,
            project_id,
            candidate_id,
            candidate_state_sha256,
            artifact_id,
            artifact_sha256,
            request_sha256,
            input_sha256,
            runtime_build_cohort_sha256,
        )?;
        self.begin_production_weapon_form_art_baseline_cas_batch(owner)
    }

    /// Associate one bounded temporary object with its typed batch. This is a
    /// metadata-only index operation; the content hash and registered object
    /// metadata are rechecked while the CAS mutation lock is held.
    pub fn associate_production_weapon_form_art_baseline_cas_object(
        &self,
        batch: &ProductionWeaponFormArtBaselineCasBatch,
        object: &CasObjectRecord,
    ) -> Result<(), StoreError> {
        if !std::sync::Arc::ptr_eq(&batch.reservation.reservations, &self.cas_reservations) {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_SCOPE_DENIED",
                "baseline CAS batch belongs to a different Runtime Store",
            ));
        }
        if !reservation_owns_hash(self, &batch.reservation, &object.sha256)? {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_RESERVATION_MISSING",
                "baseline CAS batch object is not held by its live reservation",
            ));
        }
        if object.schema_version != "CasObject@1"
            || object.reachability != "temporary"
            || !is_sha256(&object.sha256)
            || object.created_at.parse::<u64>().is_err()
            || !batch_object_metadata_is_allowed(&object.mime, &object.kind, object.size_bytes)
        {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_INVALID",
                "baseline CAS batch object metadata is outside the bounded temporary set",
            ));
        }
        let _cas_mutation_guard = self
            .cas_mutation_lock
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?;
        self.cas.verify(&object.sha256, object.size_bytes)?;
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let Some((stored_owner, status)) =
            batch_owner_from_transaction(&transaction, batch.batch_id())?
        else {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_MISSING",
                "baseline CAS batch owner is not registered",
            ));
        };
        validate_batch_owner(&stored_owner)?;
        if stored_owner != *batch.owner() || status != CAS_BATCH_OPEN {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_MISMATCH",
                "baseline CAS batch owner or lifecycle status differs",
            ));
        }
        let registered: Option<(i64, String, String, String, String)> = transaction
            .query_row(
                "SELECT size_bytes, mime, kind, reachability, created_at
                   FROM objects WHERE sha256 = ?1",
                params![object.sha256],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((size_bytes, mime, kind, reachability, created_at)) = registered else {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_MISSING",
                "baseline CAS batch object is not registered",
            ));
        };
        if size_bytes != i64::try_from(object.size_bytes).unwrap_or(i64::MAX)
            || mime != object.mime
            || kind != object.kind
            || created_at != object.created_at
        {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_MISMATCH",
                "registered CAS metadata or created_at differs from the batch object",
            ));
        }
        if reachability == "reachable" {
            // Content-addressed bytes may already be durable through another
            // link. They need no second temporary owner; the caller still
            // holds and can release this operation's reservation token.
            return Ok(());
        }
        if reachability != "temporary" {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_MISMATCH",
                "only temporary or already reachable CAS objects may enter a batch",
            ));
        }
        let existing: Option<(i64, String, String, String, String)> = transaction
            .query_row(
                &format!(
                    "SELECT size_bytes, mime, kind, status, created_at
                       FROM {CAS_BATCH_OBJECT_TABLE}
                      WHERE batch_id = ?1 AND object_sha256 = ?2"
                ),
                params![batch.batch_id(), object.sha256],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;
        if let Some((size, existing_mime, existing_kind, existing_status, existing_created_at)) =
            existing
        {
            if size == size_bytes
                && existing_mime == mime
                && existing_kind == kind
                && existing_status == "temporary"
            {
                // If another producer won registration between preclaim and
                // put, the registered CAS row owns the canonical created_at.
                // Repair only this batch-local metadata row; mismatched
                // content metadata remains a hard conflict.
                if existing_created_at != created_at {
                    let updated = transaction.execute(
                        &format!(
                            "UPDATE {CAS_BATCH_OBJECT_TABLE}
                                SET size_bytes = ?1, mime = ?2, kind = ?3, created_at = ?4
                              WHERE batch_id = ?5
                                AND object_sha256 = ?6
                                AND status = 'temporary'"
                        ),
                        params![
                            size_bytes,
                            &mime,
                            &kind,
                            &created_at,
                            batch.batch_id(),
                            object.sha256,
                        ],
                    )?;
                    if updated != 1 {
                        return Err(contract(
                            "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_CONFLICT",
                            "baseline CAS batch object status changed during association",
                        ));
                    }
                }
                return Ok(());
            }
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_CONFLICT",
                "baseline CAS batch hash is already associated with different metadata",
            ));
        }
        transaction.execute(
            &format!(
                "INSERT INTO {CAS_BATCH_OBJECT_TABLE}
                    (batch_id, object_sha256, size_bytes, mime, kind, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'temporary', ?6)"
            ),
            params![
                batch.batch_id(),
                object.sha256,
                i64::try_from(object.size_bytes).map_err(|_| {
                    StoreError::InvalidData("baseline CAS batch object is too large".to_owned())
                })?,
                object.mime,
                object.kind,
                created_at,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Put and associate one of the bounded FormArt derived objects. Runtime
    /// can use this seam for camera, RenderSet, AOV/mask, comparison, quality,
    /// and baseline parent/view receipts without broadening generic CAS APIs.
    pub fn put_production_weapon_form_art_baseline_cas_object(
        &self,
        batch: &ProductionWeaponFormArtBaselineCasBatch,
        bytes: &[u8],
        expected_sha256: Option<&str>,
        mime: &str,
        kind: &str,
        created_at: &str,
    ) -> Result<super::CasObject, StoreError> {
        if !batch_object_metadata_is_allowed(mime, kind, bytes.len() as u64) {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_INVALID",
                "baseline CAS batch object kind or size is outside the bounded temporary set",
            ));
        }
        if created_at.parse::<u64>().is_err() {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_INVALID",
                "baseline CAS batch object created_at must be unix seconds",
            ));
        }
        let actual_sha256 = sha256_hex(bytes);
        if let Some(expected_sha256) = expected_sha256 {
            if !is_sha256(expected_sha256) || expected_sha256 != actual_sha256 {
                return Err(StoreError::InvalidData(
                    "baseline CAS batch object hash does not match content".to_owned(),
                ));
            }
        }
        {
            let mut reservations = self
                .cas_reservations
                .lock()
                .map_err(|_| StoreError::LockPoisoned)?;
            reservations
                .entry(actual_sha256.clone())
                .or_default()
                .insert(batch.reservation.token.clone());
        }
        let canonical_object = match preclaim_batch_object(
            self,
            batch,
            &actual_sha256,
            bytes.len() as u64,
            mime,
            kind,
            created_at,
        ) {
            Ok(object) => object,
            Err(error) => {
                self.remove_cas_reservation_locked(&batch.reservation.token, &actual_sha256)?;
                return Err(error);
            }
        };
        let mut object = self.put_object_reserved(
            batch.reservation(),
            bytes,
            expected_sha256,
            &canonical_object.mime,
            &canonical_object.kind,
            &canonical_object.created_at,
        )?;
        if let Some(existing) = self.get_object(&object.record.sha256)? {
            if existing.size_bytes == object.record.size_bytes
                && existing.mime == object.record.mime
                && existing.kind == object.record.kind
                && matches!(existing.reachability.as_str(), "temporary" | "reachable")
            {
                // A peer may have registered the same hash after preclaim
                // observed no object.  The registered Store row is the
                // canonical metadata source for both the returned object and
                // the batch association, including its created_at.
                object.record.size_bytes = existing.size_bytes;
                object.record.mime = existing.mime.clone();
                object.record.kind = existing.kind.clone();
                object.record.reachability = existing.reachability.clone();
                object.record.created_at = existing.created_at.clone();
                if existing.reachability == "reachable" {
                    self.mark_production_weapon_form_art_baseline_cas_object_status(
                        batch,
                        &object.record.sha256,
                        "linked",
                    )?;
                    self.release_cas_reservation_object(batch.reservation(), &object, false)?;
                    return Ok(object);
                }
            }
        }
        if let Err(error) =
            self.associate_production_weapon_form_art_baseline_cas_object(batch, &object.record)
        {
            // Keep this helper metadata-only on association failure. The
            // reservation is released, but CAS bytes are left for diagnosis;
            // startup reconciliation only handles objects that were indexed.
            let _ = self.release_cas_reservation_object(batch.reservation(), &object, false);
            return Err(error);
        }
        Ok(object)
    }

    fn mark_production_weapon_form_art_baseline_cas_object_status(
        &self,
        batch: &ProductionWeaponFormArtBaselineCasBatch,
        sha256: &str,
        status: &str,
    ) -> Result<(), StoreError> {
        if !std::sync::Arc::ptr_eq(&batch.reservation.reservations, &self.cas_reservations) {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_SCOPE_DENIED",
                "baseline CAS batch belongs to a different Runtime Store",
            ));
        }
        let _cas_mutation_guard = self
            .cas_mutation_lock
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?;
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let Some((stored_owner, lifecycle)) =
            batch_owner_from_transaction(&transaction, batch.batch_id())?
        else {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_MISSING",
                "baseline CAS batch owner is not registered",
            ));
        };
        if stored_owner != *batch.owner() || lifecycle != CAS_BATCH_OPEN {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_MISMATCH",
                "baseline CAS batch owner or lifecycle status differs",
            ));
        }
        set_batch_object_status(&transaction, batch.batch_id(), sha256, status)?;
        transaction.commit()?;
        Ok(())
    }

    /// Abort a failed or replayed batch without waiting for expiry or Store
    /// restart. This closes only the supplied owner and quarantines only its
    /// temporary metadata; linked/reachable objects, peer reservations, and
    /// all CAS bytes are preserved. Repeated calls are idempotent once the
    /// owner is terminal.
    pub fn abort_production_weapon_form_art_baseline_cas_batch(
        &self,
        batch: &ProductionWeaponFormArtBaselineCasBatch,
    ) -> Result<(), StoreError> {
        abort_production_weapon_form_art_baseline_cas_batch(self, batch)
    }

    /// Complete the owner lifecycle after the public baseline commit. The
    /// durable baseline row is checked first; only objects recorded by this
    /// batch may then be promoted to reachable. A repeated completion is
    /// idempotent, while an owner with no matching durable baseline fails
    /// closed.
    pub fn complete_production_weapon_form_art_baseline_cas_batch(
        &self,
        batch: &ProductionWeaponFormArtBaselineCasBatch,
        record: &ProductionWeaponFormArtBaselineRecord,
    ) -> Result<(), StoreError> {
        validate_batch_owner(batch.owner())?;
        validate_record_shape(record)?;
        if !batch_owner_scope_matches_record(batch.owner(), record) {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_MISMATCH",
                "baseline CAS batch owner does not match the committed baseline",
            ));
        }
        let _cas_mutation_guard = self
            .cas_mutation_lock
            .lock()
            .map_err(|_| StoreError::LockPoisoned)?;
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        let Some((stored_owner, status)) =
            batch_owner_from_transaction(&transaction, batch.batch_id())?
        else {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_MISSING",
                "baseline CAS batch owner is not registered",
            ));
        };
        if stored_owner != *batch.owner() {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_MISMATCH",
                "baseline CAS batch owner differs from persisted identity",
            ));
        }
        let stored: Option<ProductionWeaponFormArtBaselineRecord> = transaction
            .query_row(
                &format!("SELECT record_json FROM {TABLE} WHERE baseline_id = ?1"),
                params![record.baseline_id],
                read_record,
            )
            .optional()?;
        let Some(stored) = stored else {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_DURABLE_LINK_MISSING",
                "baseline CAS batch cannot complete before its durable baseline row",
            ));
        };
        if stored != *record {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OWNER_MISMATCH",
                "durable baseline row differs from completion record",
            ));
        }
        if status == CAS_BATCH_COMMITTED {
            transaction.commit()?;
            return Ok(());
        }
        if status != CAS_BATCH_OPEN {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_NOT_OPEN",
                "quarantined baseline CAS batch cannot be completed",
            ));
        }
        let roots = roots(record);
        let mut statement = transaction.prepare(&format!(
            "SELECT object_sha256, size_bytes, mime, kind, status, created_at
               FROM {CAS_BATCH_OBJECT_TABLE}
              WHERE batch_id = ?1
              ORDER BY object_sha256 ASC"
        ))?;
        let objects = statement
            .query_map(params![batch.batch_id()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (sha256, size_bytes, mime, kind, status, created_at) in objects {
            if !roots.iter().any(|root| root == &sha256) {
                return Err(contract(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_NOT_ROOT",
                    "baseline CAS batch contains an object outside the durable baseline roots",
                ));
            }
            let registered: Option<(i64, String, String, String, String)> = transaction
                .query_row(
                    "SELECT size_bytes, mime, kind, reachability, created_at
                       FROM objects WHERE sha256 = ?1",
                    params![sha256],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?;
            let Some((
                registered_size,
                registered_mime,
                registered_kind,
                reachability,
                registered_created_at,
            )) = registered
            else {
                return Err(contract(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_MISSING",
                    "baseline CAS batch object disappeared before completion",
                ));
            };
            if registered_size != size_bytes
                || registered_mime != mime
                || registered_kind != kind
                || registered_created_at != created_at
                || !matches!(status.as_str(), "temporary" | "reachable" | "linked")
                || !matches!(
                    reachability.as_str(),
                    "temporary" | "reachable" | "quarantined"
                )
            {
                return Err(contract(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_CAS_BATCH_OBJECT_MISMATCH",
                    "baseline CAS batch object metadata or reachability is invalid",
                ));
            }
            transaction.execute(
                "UPDATE objects SET reachability = 'reachable'
                  WHERE sha256 = ?1 AND reachability IN ('temporary', 'quarantined')",
                params![sha256],
            )?;
            transaction.execute(
                &format!(
                    "UPDATE {CAS_BATCH_OBJECT_TABLE}
                        SET status = 'reachable'
                      WHERE batch_id = ?1 AND object_sha256 = ?2"
                ),
                params![batch.batch_id(), sha256],
            )?;
        }
        transaction.execute(
            &format!(
                "UPDATE {CAS_BATCH_TABLE}
                    SET status = ?1, completed_at = ?2
                  WHERE batch_id = ?3 AND status = 'open'"
            ),
            params![
                CAS_BATCH_COMMITTED,
                unix_now_secs()?.to_string(),
                batch.batch_id()
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Atomically persist the public baseline record and all of its required
    /// CAS roots.  A byte-identical project/idempotency request replays; any
    /// different record under that key or baseline_id conflicts.
    pub fn commit_production_weapon_form_art_baseline_with_replay(
        &self,
        bundle: &ProductionWeaponFormArtBaselineCommitBundle,
    ) -> Result<(ProductionWeaponFormArtBaselineRecord, bool), StoreError> {
        validate_bundle(self, bundle)?;
        let record = &bundle.baseline;
        let payload_json = canonical_json(record)?;
        let mut connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let transaction = connection.transaction()?;
        validate_candidate_binding_in_transaction(&transaction, record)?;
        let existing = transaction
            .query_row(
                &format!(
                    "SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"
                ),
                params![record.project_id, record.idempotency_key],
                read_record,
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing.baseline_id != record.baseline_id
                || existing.registration_lineage_id != record.registration_lineage_id
                || existing.registration_lineage_canonical_sha256
                    != record.registration_lineage_canonical_sha256
                || existing.session_id != record.session_id
                || existing.project_id != record.project_id
                || existing.candidate_id != record.candidate_id
                || existing.candidate_state_sha256 != record.candidate_state_sha256
                || existing.artifact_id != record.artifact_id
                || existing.artifact_sha256 != record.artifact_sha256
                || existing.base_version_id != record.base_version_id
                || existing.request_sha256 != record.request_sha256
                || existing.input_sha256 != record.input_sha256
                || existing.idempotency_key != record.idempotency_key
            {
                return Err(contract(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_CONFLICT",
                    "project/idempotency key is bound to different baseline input or scope",
                ));
            }
            super::mark_reachable_in_transaction(&transaction, &roots(&existing))?;
            transaction.commit()?;
            return Ok((existing, true));
        }
        let reachable = roots(record);
        let source_identity_conflict: Option<(String, String)> = transaction
            .query_row(
                &format!(
                    "SELECT baseline_id, idempotency_key FROM {TABLE}
                     WHERE registration_lineage_id = ?1
                       AND candidate_id = ?2
                       AND artifact_sha256 = ?3
                       AND runtime_build_cohort_sha256 = ?4
                     ORDER BY baseline_id ASC
                     LIMIT 1"
                ),
                params![
                    record.registration_lineage_id,
                    record.candidate_id,
                    record.artifact_sha256,
                    record.runtime_build_cohort_sha256,
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((baseline_id, idempotency_key)) = source_identity_conflict {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_IDENTITY_CONFLICT",
                format!(
                    "registration lineage/candidate/artifact/Runtime cohort is already bound to baseline {baseline_id} (idempotency key {idempotency_key})"
                ),
            ));
        }
        let identity_conflict: Option<String> = transaction
            .query_row(
                &format!(
                    "SELECT baseline_id FROM {TABLE}
                     WHERE baseline_id = ?1 OR receipt_object_sha256 = ?2"
                ),
                params![record.baseline_id, record.receipt_object_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if identity_conflict.is_some() {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CONFLICT",
                "baseline_id or baseline parent receipt is already bound",
            ));
        }
        let view_kinds_json = canonical_json(&record.view_kinds)?;
        let view_receipts_json = canonical_json(
            &record
                .views
                .iter()
                .map(|view| view.receipt_object_sha256.clone())
                .collect::<Vec<_>>(),
        )?;
        let insert_result = transaction.execute(
            &format!(
                "INSERT INTO {TABLE} (
                    baseline_id, schema_version, session_id, project_id, candidate_id,
                    candidate_state_sha256, artifact_id, artifact_sha256,
                    registration_lineage_id, registration_lineage_canonical_sha256,
                    registration_lineage_receipt_object_sha256, registered_rig_v2_id,
                    registered_rig_v2_object_sha256, registered_rig_v2_canonical_sha256,
                    receipt_object_sha256, view_kinds_json, view_receipt_object_sha256_json,
                    runtime_build_cohort_sha256, baseline_policy, materialization_status,
                    request_sha256, input_sha256, idempotency_key, canonical_sha256,
                    created_at, record_json
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                    ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)"
            ),
            params![
                record.baseline_id,
                record.schema_version,
                record.session_id,
                record.project_id,
                record.candidate_id,
                record.candidate_state_sha256,
                record.artifact_id,
                record.artifact_sha256,
                record.registration_lineage_id,
                record.registration_lineage_canonical_sha256,
                record.registration_lineage_receipt_object_sha256,
                record.registered_rig_v2_id,
                record.registered_rig_v2_object_sha256,
                record.registered_rig_v2_canonical_sha256,
                record.receipt_object_sha256,
                view_kinds_json,
                view_receipts_json,
                record.runtime_build_cohort_sha256,
                record.baseline_policy,
                record.materialization_status,
                record.request_sha256,
                record.input_sha256,
                record.idempotency_key,
                record.canonical_sha256,
                record.created_at,
                payload_json,
            ],
        );
        if let Err(error) = insert_result {
            if let Some(conflict) = baseline_insert_constraint_error(&error) {
                return Err(conflict);
            }
            return Err(error.into());
        }
        super::mark_reachable_in_transaction(&transaction, &reachable)?;
        let stored = transaction.query_row(
            &format!(
                "SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"
            ),
            params![record.project_id, record.idempotency_key],
            read_record,
        )?;
        transaction.commit()?;
        Ok((stored, false))
    }

    pub fn record_production_weapon_form_art_baseline_with_replay(
        &self,
        bundle: &ProductionWeaponFormArtBaselineCommitBundle,
    ) -> Result<(ProductionWeaponFormArtBaselineRecord, bool), StoreError> {
        self.commit_production_weapon_form_art_baseline_with_replay(bundle)
    }

    pub fn get_production_weapon_form_art_baseline(
        &self,
        project_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<ProductionWeaponFormArtBaselineRecord>, StoreError> {
        if !is_opaque_id(project_id) || !is_opaque_id(idempotency_key) {
            return Err(StoreError::InvalidData(
                "baseline lookup identity is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let record = connection
            .query_row(
                &format!(
                    "SELECT record_json FROM {TABLE} WHERE project_id = ?1 AND idempotency_key = ?2"
                ),
                params![project_id, idempotency_key],
                read_record,
            )
            .optional()?;
        drop(connection);
        let Some(record) = record else {
            return Ok(None);
        };
        if record.project_id != project_id || record.idempotency_key != idempotency_key {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_SCOPE_MISMATCH",
                "stored baseline record scope differs from lookup",
            ));
        }
        validate_record_shape(&record)?;
        validate_persisted_objects(self, &record)?;
        Ok(Some(record))
    }

    pub fn get_production_weapon_form_art_baseline_by_baseline_id(
        &self,
        baseline_id: &str,
    ) -> Result<Option<ProductionWeaponFormArtBaselineRecord>, StoreError> {
        if !is_opaque_id(baseline_id) {
            return Err(StoreError::InvalidData("baseline_id is invalid".to_owned()));
        }
        let connection = self.lock_connection()?;
        ensure_table(&connection)?;
        let identity: Option<(String, String)> = connection
            .query_row(
                &format!("SELECT project_id, idempotency_key FROM {TABLE} WHERE baseline_id = ?1"),
                params![baseline_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        drop(connection);
        let Some((project_id, idempotency_key)) = identity else {
            return Ok(None);
        };
        self.get_production_weapon_form_art_baseline(&project_id, &idempotency_key)
    }

    pub fn get_production_weapon_form_art_baseline_by_id(
        &self,
        baseline_id: &str,
    ) -> Result<Option<ProductionWeaponFormArtBaselineRecord>, StoreError> {
        self.get_production_weapon_form_art_baseline_by_baseline_id(baseline_id)
    }

    /// Resolve the one fresh baseline that can authorize a same-cohort
    /// source-side FormArt repair.  A caller cannot choose between multiple
    /// camera lineages implicitly: ambiguous current-cohort baselines fail
    /// closed and require an explicit higher-level binding instead.
    pub fn get_production_weapon_form_art_baseline_for_current_source(
        &self,
        project_id: &str,
        candidate_id: &str,
        artifact_sha256: &str,
        runtime_build_cohort_sha256: &str,
    ) -> Result<Option<ProductionWeaponFormArtBaselineRecord>, StoreError> {
        if !is_opaque_id(project_id)
            || !is_opaque_id(candidate_id)
            || !is_sha256(artifact_sha256)
            || !is_sha256(runtime_build_cohort_sha256)
        {
            return Err(StoreError::InvalidData(
                "baseline current-source lookup identity is invalid".to_owned(),
            ));
        }
        let baseline_ids = {
            let connection = self.lock_connection()?;
            ensure_table(&connection)?;
            let mut statement = connection.prepare(&format!(
                "SELECT baseline_id FROM {TABLE} WHERE project_id = ?1 AND candidate_id = ?2 AND artifact_sha256 = ?3 AND runtime_build_cohort_sha256 = ?4 ORDER BY created_at DESC, baseline_id ASC LIMIT 2"
            ))?;
            let rows = statement.query_map(
                params![
                    project_id,
                    candidate_id,
                    artifact_sha256,
                    runtime_build_cohort_sha256
                ],
                |row| row.get::<_, String>(0),
            )?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        if baseline_ids.len() > 1 {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CURRENT_SOURCE_AMBIGUOUS",
                "multiple current-cohort baselines match the source candidate and artifact",
            ));
        }
        let Some(baseline_id) = baseline_ids.into_iter().next() else {
            return Ok(None);
        };
        let record = self
            .get_production_weapon_form_art_baseline_by_baseline_id(&baseline_id)?
            .ok_or_else(|| {
                contract(
                    "PRODUCTION_WEAPON_FORM_ART_BASELINE_CURRENT_SOURCE_MISSING",
                    "current-source baseline disappeared during strict readback",
                )
            })?;
        if record.project_id != project_id
            || record.candidate_id != candidate_id
            || record.artifact_sha256 != artifact_sha256
            || record.runtime_build_cohort_sha256 != runtime_build_cohort_sha256
        {
            return Err(contract(
                "PRODUCTION_WEAPON_FORM_ART_BASELINE_CURRENT_SOURCE_SCOPE_MISMATCH",
                "current-source baseline scope differs after strict readback",
            ));
        }
        Ok(Some(record))
    }
}

#[cfg(test)]
mod cas_batch_tests {
    use super::*;

    #[test]
    fn temporary_same_hash_reuse_returns_registered_metadata() {
        let store = Store::memory().expect("store");
        let bytes = br#"{"camera_hash":"test"}"#;
        let registered = store
            .put_object(bytes, None, JSON_MIME, "camera-calibration", "11")
            .expect("registered object");
        let batch = store
            .begin_production_weapon_form_art_baseline_cas_batch_for_prepare(
                "baseline-test",
                "lineage-test",
                "session-test",
                "project-test",
                "candidate-test",
                &"1".repeat(64),
                "artifact-test",
                &"2".repeat(64),
                &"3".repeat(64),
                &"4".repeat(64),
                &"5".repeat(64),
            )
            .expect("batch");
        let reused = store
            .put_production_weapon_form_art_baseline_cas_object(
                &batch,
                bytes,
                None,
                JSON_MIME,
                "camera-calibration",
                "22",
            )
            .expect("reused object");

        assert_eq!(
            reused.record.schema_version,
            registered.record.schema_version
        );
        assert_eq!(reused.record.sha256, registered.record.sha256);
        assert_eq!(reused.record.size_bytes, registered.record.size_bytes);
        assert_eq!(reused.record.mime, registered.record.mime);
        assert_eq!(reused.record.kind, registered.record.kind);
        assert_eq!(reused.record.reachability, registered.record.reachability);
        assert_eq!(reused.record.created_at, registered.record.created_at);
    }

    #[test]
    fn stale_batch_recovery_is_reservation_and_owner_scoped_without_cas_delete() {
        let store = Store::memory().expect("store");
        let batch = store
            .begin_production_weapon_form_art_baseline_cas_batch_for_prepare(
                "baseline-test",
                "lineage-test",
                "session-test",
                "project-test",
                "candidate-test",
                &"1".repeat(64),
                "artifact-test",
                &"2".repeat(64),
                &"3".repeat(64),
                &"4".repeat(64),
                &"5".repeat(64),
            )
            .expect("batch");
        let object = store
            .put_production_weapon_form_art_baseline_cas_object(
                &batch,
                br#"{"camera_hash":"test"}"#,
                None,
                JSON_MIME,
                "camera-calibration",
                "1",
            )
            .expect("object");
        let unowned = store
            .put_object(
                br#"{"unowned":true}"#,
                None,
                JSON_MIME,
                PRODUCTION_WEAPON_FORM_ART_BASELINE_PARENT_OBJECT_KIND,
                "1",
            )
            .expect("unowned object");
        let expired =
            (unix_now_secs().expect("clock") - RECONCILIATION_MIN_AGE_SECS - 1).to_string();
        let connection = store.lock_connection().expect("connection");
        connection
            .execute(
                &format!(
                    "UPDATE {CAS_BATCH_TABLE}
                        SET created_at = ?1, expires_at = ?1
                      WHERE batch_id = ?2"
                ),
                params![expired, batch.batch_id()],
            )
            .expect("expire batch");
        drop(connection);

        // A live reservation keeps an expired owner from being quarantined.
        reconcile_stale_cas_receipts(&store).expect("live reconciliation");
        assert_eq!(
            store
                .get_object(&object.record.sha256)
                .expect("object lookup")
                .expect("object")
                .reachability,
            "temporary"
        );

        drop(batch);
        reconcile_stale_cas_receipts(&store).expect("stale reconciliation");
        assert_eq!(
            store
                .get_object(&object.record.sha256)
                .expect("object lookup")
                .expect("object")
                .reachability,
            "quarantined"
        );
        assert!(store.cas.read_verified(&object.record.sha256).is_ok());
        assert_eq!(
            store
                .get_object(&unowned.record.sha256)
                .expect("unowned lookup")
                .expect("unowned object")
                .reachability,
            "temporary"
        );
    }
}
