use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rusqlite::{params, types::ValueRef, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    canonical::sha256_bytes, migration::open_connection, semantic_sha256, verify_forgecad_glb,
    ContentAddressedObjectStore, CoreError, CoreResult, ForgeCadGlbReadback, StoredObject,
    WriterLease,
};

const MIGRATION_SCHEMA_VERSION: &str = "ForgeCADArtifactMigrationRun@1";
const ASSET_STATUS_SCHEMA_VERSION: &str = "ForgeCADAssetMigrationStatus@1";
const MAX_INLINE_GLB_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMPORTED_GLB_BYTES: usize = 32 * 1024 * 1024;
const MAX_IMPORTED_GLB_TRIANGLES: u64 = 250_000;

const SOURCE_CANDIDATE: &str = "agent_candidate_inline_glb";
const SOURCE_IMPORTED: &str = "agent_imported_glb";
const ROLE_LEGACY_INTERACTIVE: &str = "legacy_interactive_preview_glb";
const ROLE_EXTERNAL_REFERENCE: &str = "external_reference_glb";

/// Aggregate result of one resumable historical-artifact adoption pass.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactMigrationReport {
    pub run_id: String,
    pub inventory_sha256: String,
    pub semantic_sha256_before: String,
    pub semantic_sha256_after: String,
    pub total_items: u64,
    pub migrated_items: u64,
    pub legacy_read_only_items: u64,
    pub retryable_error_items: u64,
    pub recovered_pending_objects: u64,
}

/// Offline, writer-fenced migration of historical inline/reference artifacts.
#[derive(Debug, Clone)]
pub struct ArtifactMigrationRunner {
    db_path: PathBuf,
    library_root: PathBuf,
    max_inline_glb_bytes: usize,
}

impl ArtifactMigrationRunner {
    pub fn new(db_path: impl AsRef<Path>, library_root: impl AsRef<Path>) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
            library_root: library_root.as_ref().to_path_buf(),
            max_inline_glb_bytes: MAX_INLINE_GLB_BYTES,
        }
    }

    pub fn run(&self, lease: &WriterLease) -> CoreResult<ArtifactMigrationReport> {
        self.run_internal(lease, MigrationFault::None)
    }

    fn run_internal(
        &self,
        lease: &WriterLease,
        mut fault: MigrationFault,
    ) -> CoreResult<ArtifactMigrationReport> {
        let canonical_db = self.db_path.canonicalize()?;
        if canonical_db != lease.db_path() {
            return Err(CoreError::conflict(
                "ARTIFACT_MIGRATION_WRITER_MISMATCH",
                "Artifact migration must use the writer lease for the same SQLite library.",
            ));
        }
        let store = ContentAddressedObjectStore::new(&self.library_root)?;
        let mut connection = open_connection(&canonical_db)?;
        lease.assert_current(&connection)?;
        require_migration_schema(&connection)?;

        let indexed = indexed_object_sha256(&connection)?;
        let recovered = store.recover_pending(&indexed)?;
        let inventory = inventory_legacy_artifacts(&connection)?;
        let inventory_identity = inventory
            .iter()
            .map(MigrationItem::identity)
            .collect::<Vec<_>>();
        let inventory_sha256 = semantic_sha256(&inventory_identity)?;
        if let Some(mut report) = ready_report_for_inventory(&connection, &inventory_sha256)? {
            report.recovered_pending_objects = recovered.len() as u64;
            return Ok(report);
        }
        let semantic_before = legacy_semantic_sha256(&connection)?;
        let run_identity = json!({
            "schema_version": MIGRATION_SCHEMA_VERSION,
            "inventory_sha256": inventory_sha256,
        });
        let run_id = format!("artifact_migration_{}", semantic_sha256(&run_identity)?);
        let started_at = system_timestamp();
        begin_run(
            &mut connection,
            lease,
            &run_id,
            &inventory_sha256,
            &semantic_before,
            inventory.len() as u64,
            &started_at,
        )?;
        capture_initial_asset_cohort(
            &mut connection,
            lease,
            &run_id,
            &semantic_before,
            &started_at,
        )?;

        let migration_result = (|| -> CoreResult<(u64, u64, u64)> {
            let mut migrated = 0_u64;
            let mut legacy_read_only = 0_u64;
            let retryable = 0_u64;
            for item in &inventory {
                let outcome =
                    self.process_item(&mut connection, lease, &store, &run_id, item, &mut fault)?;
                match outcome {
                    ItemOutcome::Migrated => migrated += 1,
                    ItemOutcome::LegacyReadOnly => legacy_read_only += 1,
                }
            }
            classify_asset_versions(
                &mut connection,
                lease,
                &store,
                &run_id,
                &semantic_before,
                &started_at,
            )?;
            Ok((migrated, legacy_read_only, retryable))
        })();

        let (migrated, legacy_read_only, retryable) = match migration_result {
            Ok(counts) => counts,
            Err(error) => {
                if error.code() != "ARTIFACT_MIGRATION_INTERRUPTED" {
                    let _ = mark_run_error(&mut connection, lease, &run_id, error.code());
                }
                return Err(error);
            }
        };

        let semantic_after = legacy_semantic_sha256(&connection)?;
        if semantic_after != semantic_before {
            mark_run_error(
                &mut connection,
                lease,
                &run_id,
                "LEGACY_SEMANTIC_STATE_CHANGED",
            )?;
            return Err(CoreError::conflict(
                "LEGACY_SEMANTIC_STATE_CHANGED",
                "Legacy product-state rows changed while artifact adoption was running.",
            ));
        }
        finish_run(
            &mut connection,
            lease,
            &run_id,
            &semantic_after,
            migrated,
            legacy_read_only,
            retryable,
        )?;
        Ok(ArtifactMigrationReport {
            run_id,
            inventory_sha256,
            semantic_sha256_before: semantic_before,
            semantic_sha256_after: semantic_after,
            total_items: inventory.len() as u64,
            migrated_items: migrated,
            legacy_read_only_items: legacy_read_only,
            retryable_error_items: retryable,
            recovered_pending_objects: recovered.len() as u64,
        })
    }

    fn process_item(
        &self,
        connection: &mut Connection,
        lease: &WriterLease,
        store: &ContentAddressedObjectStore,
        run_id: &str,
        item: &MigrationItem,
        fault: &mut MigrationFault,
    ) -> CoreResult<ItemOutcome> {
        assert_existing_item_compatible(connection, item)?;
        match &item.payload {
            ItemPayload::Candidate { glb_base64 } => {
                let bytes = match decode_inline_glb(glb_base64, self.max_inline_glb_bytes) {
                    Ok(bytes) => bytes,
                    Err(reason) => {
                        record_rejection(connection, lease, run_id, item, reason)?;
                        return Ok(ItemOutcome::LegacyReadOnly);
                    }
                };
                let validation = match validate_interactive_candidate_glb(&bytes) {
                    Ok(validation) => validation,
                    Err(reason) => {
                        record_rejection(connection, lease, run_id, item, reason)?;
                        return Ok(ItemOutcome::LegacyReadOnly);
                    }
                };
                let mut promoted = store.stage(&bytes, "glb")?.promote()?;
                if fault.fire(MigrationFault::AfterPromoteBeforeDatabase) {
                    std::mem::forget(promoted);
                    return Err(interrupted_error());
                }
                let stored = promoted.metadata().clone();
                if let Err(error) = commit_migrated_item(
                    connection,
                    lease,
                    run_id,
                    item,
                    &stored,
                    "interactive_preview",
                    &validation.readback_sha256,
                ) {
                    promoted.cleanup_after_rollback();
                    return Err(error);
                }
                if fault.fire(MigrationFault::AfterDatabaseBeforeFinalize) {
                    std::mem::forget(promoted);
                    return Err(interrupted_error());
                }
                if promoted.finalize_commit().is_err() {
                    let indexed = indexed_object_sha256(connection)?;
                    store.recover_pending(&indexed)?;
                }
                Ok(ItemOutcome::Migrated)
            }
            ItemPayload::Imported(imported) => {
                let stored = match store.adopt_existing_legacy_object(
                    &imported.object_path,
                    &imported.sha256,
                    imported.byte_size,
                    "glb",
                ) {
                    Ok(stored) => stored,
                    Err(error) => {
                        let reason = match error.code() {
                            "LEGACY_OBJECT_PATH_INVALID" => "LEGACY_OBJECT_PATH_INVALID",
                            "LEGACY_OBJECT_MISSING" => "LEGACY_OBJECT_MISSING",
                            _ => "LEGACY_OBJECT_CORRUPT",
                        };
                        record_rejection(connection, lease, run_id, item, reason)?;
                        return Ok(ItemOutcome::LegacyReadOnly);
                    }
                };
                let inspection = match inspect_external_glb(&store.read(&stored)?) {
                    Ok(inspection) => inspection,
                    Err(reason) => {
                        record_rejection(connection, lease, run_id, item, reason)?;
                        return Ok(ItemOutcome::LegacyReadOnly);
                    }
                };
                if !imported.matches(&inspection) {
                    record_rejection(
                        connection,
                        lease,
                        run_id,
                        item,
                        "LEGACY_EXTERNAL_READBACK_MISMATCH",
                    )?;
                    return Ok(ItemOutcome::LegacyReadOnly);
                }
                let readback_sha256 = semantic_sha256(&inspection)?;
                commit_migrated_item(
                    connection,
                    lease,
                    run_id,
                    item,
                    &stored,
                    "external_reference",
                    &readback_sha256,
                )?;
                Ok(ItemOutcome::Migrated)
            }
        }
    }

    #[cfg(test)]
    fn with_max_inline_glb_bytes(mut self, value: usize) -> Self {
        self.max_inline_glb_bytes = value;
        self
    }

    #[cfg(test)]
    fn run_with_fault(
        &self,
        lease: &WriterLease,
        fault: MigrationFault,
    ) -> CoreResult<ArtifactMigrationReport> {
        self.run_internal(lease, fault)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemOutcome {
    Migrated,
    LegacyReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MigrationFault {
    None,
    AfterPromoteBeforeDatabase,
    AfterDatabaseBeforeFinalize,
    Fired,
}

impl MigrationFault {
    fn fire(&mut self, requested: Self) -> bool {
        if *self == requested {
            *self = Self::Fired;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct MigrationItemIdentity<'a> {
    migration_key: &'a str,
    source_kind: &'a str,
    source_id: &'a str,
    source_fingerprint_sha256: &'a str,
    target_reference_kind: &'a str,
    target_owner_id: &'a str,
    target_role: &'a str,
}

#[derive(Debug, Clone)]
struct MigrationItem {
    migration_key: String,
    source_kind: &'static str,
    source_id: String,
    source_fingerprint_sha256: String,
    target_reference_kind: &'static str,
    target_owner_id: String,
    target_role: &'static str,
    created_at: String,
    payload: ItemPayload,
}

impl MigrationItem {
    fn identity(&self) -> MigrationItemIdentity<'_> {
        MigrationItemIdentity {
            migration_key: &self.migration_key,
            source_kind: self.source_kind,
            source_id: &self.source_id,
            source_fingerprint_sha256: &self.source_fingerprint_sha256,
            target_reference_kind: self.target_reference_kind,
            target_owner_id: &self.target_owner_id,
            target_role: self.target_role,
        }
    }
}

#[derive(Debug, Clone)]
enum ItemPayload {
    Candidate { glb_base64: String },
    Imported(ImportedSource),
}

#[derive(Debug, Clone, Serialize)]
struct ImportedSource {
    import_id: String,
    asset_version_id: String,
    object_path: String,
    sha256: String,
    byte_size: u64,
    triangle_count: u64,
    bounds_mm_json: String,
    mesh_count: u64,
    primitive_count: u64,
    material_count: u64,
    node_count: u64,
}

impl ImportedSource {
    fn matches(&self, inspection: &ExternalGlbInspection) -> bool {
        let stored_bounds = serde_json::from_str::<Vec<f64>>(&self.bounds_mm_json).ok();
        self.byte_size == inspection.byte_size
            && self.triangle_count == inspection.triangle_count
            && self.mesh_count == inspection.mesh_count
            && self.primitive_count == inspection.primitive_count
            && self.material_count == inspection.material_count
            && self.node_count == inspection.node_count
            && stored_bounds.as_ref().is_some_and(|bounds| {
                bounds.len() == 3
                    && bounds
                        .iter()
                        .zip(&inspection.bounds_mm)
                        .all(|(left, right)| (left - right).abs() <= 0.0001)
            })
    }
}

#[derive(Debug, Clone, Serialize)]
struct ExternalGlbInspection {
    byte_size: u64,
    triangle_count: u64,
    bounds_mm: [f64; 3],
    mesh_count: u64,
    primitive_count: u64,
    material_count: u64,
    node_count: u64,
}

#[derive(Debug)]
struct CandidateValidation {
    readback_sha256: String,
}

fn require_migration_schema(connection: &Connection) -> CoreResult<()> {
    let ready: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM forgecad_core_schema_migrations WHERE version='0036')",
        [],
        |row| row.get(0),
    )?;
    if ready {
        Ok(())
    } else {
        Err(CoreError::conflict(
            "ARTIFACT_MIGRATION_SCHEMA_REQUIRED",
            "K003 artifact migration schema 0036 is not installed.",
        ))
    }
}

fn inventory_legacy_artifacts(connection: &Connection) -> CoreResult<Vec<MigrationItem>> {
    let mut items = Vec::new();
    {
        let mut statement = connection.prepare(
            "SELECT artifact_id, glb_base64, shape_program_json, created_at FROM agent_blockout_candidates WHERE length(glb_base64)>0 ORDER BY artifact_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (artifact_id, glb_base64, shape_program_json, created_at) = row?;
            let fingerprint = candidate_source_fingerprint(
                &artifact_id,
                &glb_base64,
                &shape_program_json,
                &created_at,
            )?;
            let migration_key = migration_key(
                SOURCE_CANDIDATE,
                &artifact_id,
                "candidate",
                &artifact_id,
                ROLE_LEGACY_INTERACTIVE,
            )?;
            items.push(MigrationItem {
                migration_key,
                source_kind: SOURCE_CANDIDATE,
                source_id: artifact_id.clone(),
                source_fingerprint_sha256: fingerprint,
                target_reference_kind: "candidate",
                target_owner_id: artifact_id,
                target_role: ROLE_LEGACY_INTERACTIVE,
                created_at,
                payload: ItemPayload::Candidate { glb_base64 },
            });
        }
    }
    {
        let mut statement = connection.prepare(
            "SELECT import_id, asset_version_id, object_path, sha256, byte_size, triangle_count, bounds_mm_json, mesh_count, primitive_count, material_count, node_count, created_at FROM agent_imported_glbs ORDER BY import_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                ImportedSource {
                    import_id: row.get(0)?,
                    asset_version_id: row.get(1)?,
                    object_path: row.get(2)?,
                    sha256: row.get(3)?,
                    byte_size: row.get::<_, u64>(4)?,
                    triangle_count: row.get::<_, u64>(5)?,
                    bounds_mm_json: row.get(6)?,
                    mesh_count: row.get::<_, u64>(7)?,
                    primitive_count: row.get::<_, u64>(8)?,
                    material_count: row.get::<_, u64>(9)?,
                    node_count: row.get::<_, u64>(10)?,
                },
                row.get::<_, String>(11)?,
            ))
        })?;
        for row in rows {
            let (imported, created_at) = row?;
            let fingerprint = semantic_sha256(&json!({
                "source_kind": SOURCE_IMPORTED,
                "source": imported,
                "created_at": created_at,
            }))?;
            let migration_key = migration_key(
                SOURCE_IMPORTED,
                &imported.import_id,
                "asset_version",
                &imported.asset_version_id,
                ROLE_EXTERNAL_REFERENCE,
            )?;
            items.push(MigrationItem {
                migration_key,
                source_kind: SOURCE_IMPORTED,
                source_id: imported.import_id.clone(),
                source_fingerprint_sha256: fingerprint,
                target_reference_kind: "asset_version",
                target_owner_id: imported.asset_version_id.clone(),
                target_role: ROLE_EXTERNAL_REFERENCE,
                created_at,
                payload: ItemPayload::Imported(imported),
            });
        }
    }
    items.sort_by(|left, right| left.migration_key.cmp(&right.migration_key));
    Ok(items)
}

fn migration_key(
    source_kind: &str,
    source_id: &str,
    target_reference_kind: &str,
    target_owner_id: &str,
    target_role: &str,
) -> CoreResult<String> {
    semantic_sha256(&json!({
        "source_kind": source_kind,
        "source_id": source_id,
        "target_reference_kind": target_reference_kind,
        "target_owner_id": target_owner_id,
        "target_role": target_role,
    }))
}

fn candidate_source_fingerprint(
    artifact_id: &str,
    glb_base64: &str,
    shape_program_json: &str,
    created_at: &str,
) -> CoreResult<String> {
    semantic_sha256(&json!({
        "source_kind": SOURCE_CANDIDATE,
        "artifact_id": artifact_id,
        "glb_base64": glb_base64,
        "shape_program_json": shape_program_json,
        "created_at": created_at,
    }))
}

fn current_source_fingerprint(connection: &Connection, item: &MigrationItem) -> CoreResult<String> {
    match item.source_kind {
        SOURCE_CANDIDATE => connection
            .query_row(
                "SELECT artifact_id, glb_base64, shape_program_json, created_at FROM agent_blockout_candidates WHERE artifact_id=?",
                [&item.source_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::not_found("Historical blockout candidate"))
            .and_then(|row| candidate_source_fingerprint(&row.0, &row.1, &row.2, &row.3)),
        SOURCE_IMPORTED => connection
            .query_row(
                "SELECT import_id, asset_version_id, object_path, sha256, byte_size, triangle_count, bounds_mm_json, mesh_count, primitive_count, material_count, node_count, created_at FROM agent_imported_glbs WHERE import_id=?",
                [&item.source_id],
                |row| {
                    Ok((
                        ImportedSource {
                            import_id: row.get(0)?,
                            asset_version_id: row.get(1)?,
                            object_path: row.get(2)?,
                            sha256: row.get(3)?,
                            byte_size: row.get(4)?,
                            triangle_count: row.get(5)?,
                            bounds_mm_json: row.get(6)?,
                            mesh_count: row.get(7)?,
                            primitive_count: row.get(8)?,
                            material_count: row.get(9)?,
                            node_count: row.get(10)?,
                        },
                        row.get::<_, String>(11)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::not_found("Historical imported GLB"))
            .and_then(|(source, created_at)| {
                semantic_sha256(&json!({
                    "source_kind": SOURCE_IMPORTED,
                    "source": source,
                    "created_at": created_at,
                }))
            }),
        _ => Err(CoreError::invalid_data(
            "ARTIFACT_MIGRATION_SOURCE_INVALID",
            "Artifact migration source kind is unsupported.",
        )),
    }
}

fn assert_source_unchanged(connection: &Connection, item: &MigrationItem) -> CoreResult<()> {
    if current_source_fingerprint(connection, item)? == item.source_fingerprint_sha256 {
        Ok(())
    } else {
        Err(CoreError::conflict(
            "LEGACY_ARTIFACT_SOURCE_MUTATED",
            "Historical artifact source changed after migration inventory was captured.",
        ))
    }
}

fn assert_existing_item_compatible(
    connection: &Connection,
    item: &MigrationItem,
) -> CoreResult<()> {
    let existing: Option<String> = connection
        .query_row(
            "SELECT source_fingerprint_sha256 FROM forgecad_core_artifact_migration_items WHERE migration_key=?",
            [&item.migration_key],
            |row| row.get(0),
        )
        .optional()?;
    if existing
        .as_deref()
        .is_some_and(|value| value != item.source_fingerprint_sha256)
    {
        return Err(CoreError::conflict(
            "LEGACY_ARTIFACT_SOURCE_MUTATED",
            "A previously classified historical artifact source was modified.",
        ));
    }
    Ok(())
}

fn ready_report_for_inventory(
    connection: &Connection,
    inventory_sha256: &str,
) -> CoreResult<Option<ArtifactMigrationReport>> {
    connection
        .query_row(
            "SELECT run_id, inventory_sha256, semantic_sha256_before, semantic_sha256_after, total_items, migrated_items, legacy_read_only_items, retryable_error_items FROM forgecad_core_artifact_migration_runs WHERE inventory_sha256=? AND state='ready' ORDER BY completed_at DESC, run_id DESC LIMIT 1",
            [inventory_sha256],
            |row| {
                Ok(ArtifactMigrationReport {
                    run_id: row.get(0)?,
                    inventory_sha256: row.get(1)?,
                    semantic_sha256_before: row.get(2)?,
                    semantic_sha256_after: row.get(3)?,
                    total_items: row.get(4)?,
                    migrated_items: row.get(5)?,
                    legacy_read_only_items: row.get(6)?,
                    retryable_error_items: row.get(7)?,
                    recovered_pending_objects: 0,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn begin_run(
    connection: &mut Connection,
    lease: &WriterLease,
    run_id: &str,
    inventory_sha256: &str,
    semantic_before: &str,
    total_items: u64,
    timestamp: &str,
) -> CoreResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    lease.assert_current(&transaction)?;
    transaction.execute(
        "INSERT INTO forgecad_core_artifact_migration_runs(run_id, schema_version, writer_epoch, state, inventory_sha256, semantic_sha256_before, semantic_sha256_after, total_items, migrated_items, legacy_read_only_items, retryable_error_items, error_code, started_at, updated_at, completed_at) VALUES (?, ?, ?, 'migrating', ?, ?, NULL, ?, 0, 0, 0, NULL, ?, ?, NULL) ON CONFLICT(run_id) DO UPDATE SET writer_epoch=excluded.writer_epoch, state='migrating', inventory_sha256=excluded.inventory_sha256, semantic_sha256_before=excluded.semantic_sha256_before, semantic_sha256_after=NULL, total_items=excluded.total_items, migrated_items=0, legacy_read_only_items=0, retryable_error_items=0, error_code=NULL, updated_at=excluded.updated_at, completed_at=NULL",
        params![
            run_id,
            MIGRATION_SCHEMA_VERSION,
            lease.epoch(),
            inventory_sha256,
            semantic_before,
            total_items,
            timestamp,
            timestamp,
        ],
    )?;
    transaction.commit()?;
    Ok(())
}

fn capture_initial_asset_cohort(
    connection: &mut Connection,
    lease: &WriterLease,
    run_id: &str,
    semantic_before: &str,
    timestamp: &str,
) -> CoreResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    lease.assert_current(&transaction)?;
    let captured: Option<String> = transaction.query_row(
        "SELECT cohort_captured_run_id FROM forgecad_core_artifact_migration_state WHERE singleton=1",
        [],
        |row| row.get(0),
    )?;
    if captured.is_some() {
        transaction.commit()?;
        return Ok(());
    }
    if legacy_semantic_sha256(&transaction)? != semantic_before {
        return Err(CoreError::conflict(
            "LEGACY_SEMANTIC_STATE_CHANGED",
            "Legacy product-state rows changed before the migration cohort was captured.",
        ));
    }
    let versions = {
        let mut statement = transaction.prepare(
            "SELECT asset_version_id, shape_program_json FROM agent_asset_versions ORDER BY asset_version_id",
        )?;
        let values = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        values
    };
    for (asset_version_id, shape_program_json) in versions {
        let shape_program: Value = serde_json::from_str(&shape_program_json).map_err(|_| {
            CoreError::invalid_data(
                "LEGACY_SHAPE_PROGRAM_INVALID",
                "Historical asset ShapeProgram JSON is invalid.",
            )
        })?;
        transaction.execute(
            "INSERT INTO forgecad_core_asset_migration_cohort(asset_version_id, captured_run_id, source_shape_program_sha256, captured_at) VALUES (?, ?, ?, ?)",
            params![
                asset_version_id,
                run_id,
                semantic_sha256(&shape_program)?,
                timestamp,
            ],
        )?;
    }
    let changed = transaction.execute(
        "UPDATE forgecad_core_artifact_migration_state SET cohort_captured_run_id=?, cohort_captured_at=? WHERE singleton=1 AND cohort_captured_run_id IS NULL",
        params![run_id, timestamp],
    )?;
    if changed != 1 {
        return Err(CoreError::conflict(
            "ARTIFACT_MIGRATION_COHORT_STALE",
            "Historical asset migration cohort was captured by another writer.",
        ));
    }
    transaction.commit()?;
    Ok(())
}

fn commit_migrated_item(
    connection: &mut Connection,
    lease: &WriterLease,
    run_id: &str,
    item: &MigrationItem,
    stored: &StoredObject,
    artifact_profile_id: &str,
    readback_sha256: &str,
) -> CoreResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    lease.assert_current(&transaction)?;
    assert_source_unchanged(&transaction, item)?;
    assert_existing_item_compatible(&transaction, item)?;
    let existing_item: Option<(String, Option<String>)> = transaction
        .query_row(
            "SELECT outcome, object_sha256 FROM forgecad_core_artifact_migration_items WHERE migration_key=?",
            [&item.migration_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((outcome, Some(existing_sha))) = existing_item {
        if outcome == "migrated" && existing_sha != stored.sha256 {
            return Err(CoreError::conflict(
                "ARTIFACT_MIGRATION_OBJECT_CONFLICT",
                "Historical artifact already references a different immutable object.",
            ));
        }
    }
    transaction.execute(
        "INSERT OR IGNORE INTO forgecad_core_objects(sha256, object_path, extension, byte_size, ref_count, created_at, updated_at) VALUES (?, ?, ?, ?, 0, ?, ?)",
        params![
            stored.sha256,
            stored.relative_path,
            stored.extension,
            stored.byte_size,
            item.created_at,
            item.created_at,
        ],
    )?;
    let metadata: (String, String, u64) = transaction.query_row(
        "SELECT object_path, extension, byte_size FROM forgecad_core_objects WHERE sha256=?",
        [&stored.sha256],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if metadata
        != (
            stored.relative_path.clone(),
            stored.extension.clone(),
            stored.byte_size,
        )
    {
        return Err(CoreError::conflict(
            "CONTENT_OBJECT_METADATA_CONFLICT",
            "Existing content object metadata differs from the verified historical bytes.",
        ));
    }
    transaction.execute(
        "INSERT OR IGNORE INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES (?, ?, ?, ?, ?)",
        params![
            item.target_reference_kind,
            item.target_owner_id,
            item.target_role,
            stored.sha256,
            item.created_at,
        ],
    )?;
    let referenced_sha: String = transaction.query_row(
        "SELECT sha256 FROM forgecad_core_object_references WHERE reference_kind=? AND owner_id=? AND role=?",
        params![
            item.target_reference_kind,
            item.target_owner_id,
            item.target_role,
        ],
        |row| row.get(0),
    )?;
    if referenced_sha != stored.sha256 {
        return Err(CoreError::conflict(
            "ARTIFACT_MIGRATION_REFERENCE_CONFLICT",
            "Historical artifact reference already points at a different immutable object.",
        ));
    }
    let timestamp = system_timestamp();
    transaction.execute(
        "INSERT INTO forgecad_core_artifact_migration_items(migration_key, run_id, source_kind, source_id, source_fingerprint_sha256, target_reference_kind, target_owner_id, target_role, outcome, object_sha256, artifact_profile_id, readback_sha256, reason_code, attempt_count, updated_at, completed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'migrated', ?, ?, ?, NULL, 1, ?, ?) ON CONFLICT(migration_key) DO UPDATE SET run_id=excluded.run_id, outcome='migrated', object_sha256=excluded.object_sha256, artifact_profile_id=excluded.artifact_profile_id, readback_sha256=excluded.readback_sha256, reason_code=NULL, attempt_count=forgecad_core_artifact_migration_items.attempt_count+1, updated_at=excluded.updated_at, completed_at=excluded.completed_at",
        params![
            item.migration_key,
            run_id,
            item.source_kind,
            item.source_id,
            item.source_fingerprint_sha256,
            item.target_reference_kind,
            item.target_owner_id,
            item.target_role,
            stored.sha256,
            artifact_profile_id,
            readback_sha256,
            timestamp,
            timestamp,
        ],
    )?;
    transaction.commit()?;
    Ok(())
}

fn record_rejection(
    connection: &mut Connection,
    lease: &WriterLease,
    run_id: &str,
    item: &MigrationItem,
    reason_code: &'static str,
) -> CoreResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    lease.assert_current(&transaction)?;
    assert_source_unchanged(&transaction, item)?;
    assert_existing_item_compatible(&transaction, item)?;
    let existing_outcome: Option<String> = transaction
        .query_row(
            "SELECT outcome FROM forgecad_core_artifact_migration_items WHERE migration_key=?",
            [&item.migration_key],
            |row| row.get(0),
        )
        .optional()?;
    if existing_outcome.as_deref() == Some("migrated") {
        return Err(CoreError::conflict(
            "MIGRATED_ARTIFACT_BECAME_INVALID",
            "A previously migrated historical artifact no longer passes strict verification.",
        ));
    }
    let timestamp = system_timestamp();
    transaction.execute(
        "INSERT INTO forgecad_core_artifact_migration_items(migration_key, run_id, source_kind, source_id, source_fingerprint_sha256, target_reference_kind, target_owner_id, target_role, outcome, object_sha256, artifact_profile_id, readback_sha256, reason_code, attempt_count, updated_at, completed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'legacy_read_only', NULL, NULL, NULL, ?, 1, ?, ?) ON CONFLICT(migration_key) DO UPDATE SET run_id=excluded.run_id, outcome='legacy_read_only', object_sha256=NULL, artifact_profile_id=NULL, readback_sha256=NULL, reason_code=excluded.reason_code, attempt_count=forgecad_core_artifact_migration_items.attempt_count+1, updated_at=excluded.updated_at, completed_at=excluded.completed_at",
        params![
            item.migration_key,
            run_id,
            item.source_kind,
            item.source_id,
            item.source_fingerprint_sha256,
            item.target_reference_kind,
            item.target_owner_id,
            item.target_role,
            reason_code,
            timestamp,
            timestamp,
        ],
    )?;
    transaction.commit()?;
    Ok(())
}

fn classify_asset_versions(
    connection: &mut Connection,
    lease: &WriterLease,
    store: &ContentAddressedObjectStore,
    _run_id: &str,
    semantic_before: &str,
    timestamp: &str,
) -> CoreResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    lease.assert_current(&transaction)?;
    if legacy_semantic_sha256(&transaction)? != semantic_before {
        return Err(CoreError::conflict(
            "LEGACY_SEMANTIC_STATE_CHANGED",
            "Legacy product-state rows changed before asset migration status was committed.",
        ));
    }
    let versions = {
        let mut statement = transaction.prepare(
            "SELECT v.asset_version_id, v.project_id, v.shape_program_json, c.source_shape_program_sha256 FROM forgecad_core_asset_migration_cohort c JOIN agent_asset_versions v ON v.asset_version_id=c.asset_version_id ORDER BY v.asset_version_id",
        )?;
        let values = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        values
    };
    for (asset_version_id, project_id, shape_program_json, captured_shape_program_sha256) in
        versions
    {
        let shape_program: Value = serde_json::from_str(&shape_program_json).map_err(|_| {
            CoreError::invalid_data(
                "LEGACY_SHAPE_PROGRAM_INVALID",
                "Historical asset ShapeProgram JSON is invalid.",
            )
        })?;
        let shape_program_sha256 = semantic_sha256(&shape_program)?;
        let imported: Option<(String, String, Option<String>)> = transaction
            .query_row(
                "SELECT i.import_id, COALESCE(m.outcome, 'legacy_read_only'), m.reason_code FROM agent_imported_glbs i LEFT JOIN forgecad_core_artifact_migration_items m ON m.source_kind='agent_imported_glb' AND m.source_id=i.import_id WHERE i.asset_version_id=?",
                [&asset_version_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (state, reason_code): (&str, String) = if let Some((_import_id, outcome, reason)) =
            imported
        {
            if outcome == "migrated" {
                (
                    "external_reference_read_only",
                    "EXTERNAL_REFERENCE_ONLY".to_string(),
                )
            } else {
                (
                    "legacy_read_only",
                    reason.unwrap_or_else(|| "LEGACY_EXTERNAL_REFERENCE_UNAVAILABLE".to_string()),
                )
            }
        } else {
            let exact_current = shape_program_sha256 == captured_shape_program_sha256
                && asset_has_exact_current_dual_profile(
                    &transaction,
                    store,
                    &asset_version_id,
                    &project_id,
                    &shape_program_sha256,
                )?;
            if exact_current {
                ("ready", String::new())
            } else {
                (
                    "artifact_recompile_required",
                    "LEGACY_DUAL_PROFILE_RECOMPILE_REQUIRED".to_string(),
                )
            }
        };
        transaction.execute(
            "INSERT INTO forgecad_core_asset_migration_status(asset_version_id, schema_version, shape_program_sha256, state, reason_code, updated_at) VALUES (?, ?, ?, ?, NULLIF(?, ''), ?) ON CONFLICT(asset_version_id) DO UPDATE SET shape_program_sha256=excluded.shape_program_sha256, state=excluded.state, reason_code=excluded.reason_code, updated_at=excluded.updated_at",
            params![
                asset_version_id,
                ASSET_STATUS_SCHEMA_VERSION,
                shape_program_sha256,
                state,
                reason_code,
                timestamp,
            ],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn asset_has_exact_current_dual_profile(
    connection: &Connection,
    store: &ContentAddressedObjectStore,
    asset_version_id: &str,
    project_id: &str,
    shape_program_sha256: &str,
) -> CoreResult<bool> {
    let Some(production) = asset_object_for_role(connection, asset_version_id, "production_glb")?
    else {
        return Ok(false);
    };
    let Some(interactive) =
        asset_object_for_role(connection, asset_version_id, "interactive_preview_glb")?
    else {
        return Ok(false);
    };
    if production.sha256 == interactive.sha256 {
        return Ok(false);
    }
    let production_bytes = match store.read(&production) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(false),
    };
    let interactive_bytes = match store.read(&interactive) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(false),
    };
    let production_verified =
        match verify_forgecad_glb(&production_bytes, Some("production_concept")) {
            Ok(readback) => readback,
            Err(_) => return Ok(false),
        };
    let interactive_verified =
        match verify_forgecad_glb(&interactive_bytes, Some("interactive_preview")) {
            Ok(readback) => readback,
            Err(_) => return Ok(false),
        };
    if production_verified.glb_sha256 != production.sha256
        || production_verified.glb_byte_size != production.byte_size
        || interactive_verified.glb_sha256 != interactive.sha256
        || interactive_verified.glb_byte_size != interactive.byte_size
    {
        return Ok(false);
    }

    let quality_reports = {
        let mut statement = connection.prepare(
            "SELECT quality_report_id, project_id, report_json, status FROM agent_asset_quality_reports WHERE asset_version_id=? ORDER BY created_at DESC, quality_report_id DESC",
        )?;
        let values = statement
            .query_map([asset_version_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        values
    };
    let mut production_attested = false;
    for (quality_report_id, quality_project_id, report_json, status) in quality_reports {
        if status != "passed" || quality_project_id != project_id {
            continue;
        }
        let Ok(report) = serde_json::from_str::<Value>(&report_json) else {
            continue;
        };
        if exact_production_quality_matches(
            &report,
            &quality_report_id,
            asset_version_id,
            shape_program_sha256,
            &production_verified,
        ) {
            production_attested = true;
            break;
        }
    }
    if !production_attested {
        return Ok(false);
    }

    let interactive_readbacks = {
        let mut statement = connection.prepare(
            "SELECT preview_json FROM agent_asset_change_sets WHERE resulting_asset_version_id=? AND status='confirmed' AND preview_json IS NOT NULL ORDER BY updated_at DESC, change_set_id DESC",
        )?;
        let values = statement
            .query_map([asset_version_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        values
    };
    for preview_json in interactive_readbacks {
        let Ok(seal) = serde_json::from_str::<Value>(&preview_json) else {
            continue;
        };
        if seal.get("schema_version").and_then(Value::as_str) != Some("ChangeSetConfirmSeal@1") {
            continue;
        }
        if seal
            .get("resulting_asset_version_id")
            .and_then(Value::as_str)
            != Some(asset_version_id)
        {
            continue;
        }
        if seal.get("interactive_readback").is_some_and(|readback| {
            exact_restricted_readback_matches(
                readback,
                "interactive_preview",
                shape_program_sha256,
                &interactive_verified,
            )
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn asset_object_for_role(
    connection: &Connection,
    asset_version_id: &str,
    role: &str,
) -> CoreResult<Option<StoredObject>> {
    connection
        .query_row(
            "SELECT o.sha256, o.object_path, o.extension, o.byte_size FROM forgecad_core_object_references r JOIN forgecad_core_objects o ON o.sha256=r.sha256 WHERE r.reference_kind='asset_version' AND r.owner_id=? AND r.role=?",
            params![asset_version_id, role],
            |row| {
                Ok(StoredObject {
                    sha256: row.get(0)?,
                    relative_path: row.get(1)?,
                    extension: row.get(2)?,
                    byte_size: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn exact_production_quality_matches(
    report: &Value,
    quality_report_id: &str,
    asset_version_id: &str,
    shape_program_sha256: &str,
    verified: &ForgeCadGlbReadback,
) -> bool {
    report.get("schema_version").and_then(Value::as_str) == Some("AgentAssetQualityReport@1")
        && report.get("quality_report_id").and_then(Value::as_str) == Some(quality_report_id)
        && report.get("asset_version_id").and_then(Value::as_str) == Some(asset_version_id)
        && report.get("status").and_then(Value::as_str) == Some("passed")
        && report.get("evidence_source").and_then(Value::as_str)
            == Some("geometry_compile_readback")
        && report.get("triangle_count").and_then(Value::as_u64) == Some(verified.triangle_count)
        && report.get("bounds_mm") == Some(&json!(verified.bounds_mm))
        && report.get("compile_readback").is_some_and(|readback| {
            readback.get("schema_version").and_then(Value::as_str)
                == Some("GeometryCompileReadback@2")
                && readback.get("artifact_profile") == Some(&verified.artifact_profile)
                && exact_restricted_readback_matches(
                    readback,
                    "production_concept",
                    shape_program_sha256,
                    verified,
                )
                && readback.get("uv0_primitive_count").and_then(Value::as_u64)
                    == Some(verified.uv0_primitive_count)
                && readback
                    .get("normal_primitive_count")
                    .and_then(Value::as_u64)
                    == Some(verified.normal_primitive_count)
                && readback
                    .get("tangent_primitive_count")
                    .and_then(Value::as_u64)
                    == Some(verified.tangent_primitive_count)
                && readback
                    .get("visual_texture_set_count")
                    .and_then(Value::as_u64)
                    == Some(verified.visual_texture_set_count)
                && readback
                    .get("visual_texture_map_count")
                    .and_then(Value::as_u64)
                    == Some(verified.visual_texture_map_count)
        })
}

fn exact_restricted_readback_matches(
    readback: &Value,
    expected_profile_id: &str,
    shape_program_sha256: &str,
    verified: &ForgeCadGlbReadback,
) -> bool {
    let profile_id = readback
        .get("artifact_profile_id")
        .and_then(Value::as_str)
        .or_else(|| {
            readback
                .get("artifact_profile")
                .and_then(|profile| profile.get("artifact_profile_id"))
                .and_then(Value::as_str)
        });
    readback.is_object()
        && profile_id == Some(expected_profile_id)
        && readback
            .get("runtime_manifest_version")
            .and_then(Value::as_str)
            == Some(verified.runtime_manifest_version.as_str())
        && readback.get("shape_program_sha256").and_then(Value::as_str)
            == Some(shape_program_sha256)
        && readback.get("glb_sha256").and_then(Value::as_str) == Some(verified.glb_sha256.as_str())
        && readback.get("glb_byte_size").and_then(Value::as_u64) == Some(verified.glb_byte_size)
        && readback.get("triangle_count").and_then(Value::as_u64) == Some(verified.triangle_count)
        && readback.get("bounds_mm") == Some(&json!(verified.bounds_mm))
        && readback.get("mesh_count").and_then(Value::as_u64) == Some(verified.mesh_count)
        && readback.get("primitive_count").and_then(Value::as_u64) == Some(verified.primitive_count)
        && readback.get("material_count").and_then(Value::as_u64) == Some(verified.material_count)
        && readback.get("closed_manifold").and_then(Value::as_bool)
            == Some(verified.closed_manifold)
        && readback
            .get("surface_provenance_present")
            .and_then(Value::as_bool)
            == Some(verified.surface_provenance_present)
}

fn finish_run(
    connection: &mut Connection,
    lease: &WriterLease,
    run_id: &str,
    semantic_after: &str,
    migrated: u64,
    legacy_read_only: u64,
    retryable: u64,
) -> CoreResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    lease.assert_current(&transaction)?;
    let timestamp = system_timestamp();
    let changed = transaction.execute(
        "UPDATE forgecad_core_artifact_migration_runs SET state='ready', semantic_sha256_after=?, migrated_items=?, legacy_read_only_items=?, retryable_error_items=?, error_code=NULL, updated_at=?, completed_at=? WHERE run_id=? AND state='migrating'",
        params![
            semantic_after,
            migrated,
            legacy_read_only,
            retryable,
            timestamp,
            timestamp,
            run_id,
        ],
    )?;
    if changed != 1 {
        return Err(CoreError::conflict(
            "ARTIFACT_MIGRATION_RUN_STALE",
            "Artifact migration run was no longer in the resumable state.",
        ));
    }
    transaction.commit()?;
    Ok(())
}

fn mark_run_error(
    connection: &mut Connection,
    lease: &WriterLease,
    run_id: &str,
    error_code: &str,
) -> CoreResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    lease.assert_current(&transaction)?;
    let timestamp = system_timestamp();
    let state = if matches!(
        error_code,
        "LEGACY_ARTIFACT_SOURCE_MUTATED" | "LEGACY_SEMANTIC_STATE_CHANGED"
    ) {
        "blocked"
    } else {
        "failed"
    };
    transaction.execute(
        "UPDATE forgecad_core_artifact_migration_runs SET state=?, error_code=?, updated_at=?, completed_at=? WHERE run_id=?",
        params![state, error_code, timestamp, timestamp, run_id],
    )?;
    transaction.commit()?;
    Ok(())
}

fn indexed_object_sha256(connection: &Connection) -> CoreResult<BTreeSet<String>> {
    let mut statement = connection.prepare("SELECT sha256 FROM forgecad_core_objects")?;
    let values = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(values)
}

fn legacy_semantic_sha256(connection: &Connection) -> CoreResult<String> {
    let mut hasher = Sha256::new();
    for (label, sql) in [
        (
            "projects",
            "SELECT project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at FROM projects ORDER BY project_id",
        ),
        (
            "agent_blockout_candidates",
            "SELECT artifact_id, project_id, plan_id, direction_id, domain_pack_id, status, candidate_json, shape_program_json, assembly_graph_json, material_bindings_json, glb_base64, created_at, updated_at FROM agent_blockout_candidates ORDER BY artifact_id",
        ),
        (
            "agent_asset_versions",
            "SELECT asset_version_id, project_id, parent_asset_version_id, version_no, status, summary, stage, plan_id, direction_id, domain_pack_id, artifact_id, parts_json, shape_program_json, assembly_graph_json, material_bindings_json, created_at FROM agent_asset_versions ORDER BY asset_version_id",
        ),
        (
            "agent_asset_heads",
            "SELECT project_id, asset_version_id, updated_at FROM agent_asset_heads ORDER BY project_id",
        ),
        (
            "agent_asset_change_sets",
            "SELECT change_set_id, project_id, base_asset_version_id, summary, operations_json, protected_part_ids_json, preview_json, status, resulting_asset_version_id, created_at, updated_at FROM agent_asset_change_sets ORDER BY change_set_id",
        ),
        (
            "agent_imported_glbs",
            "SELECT import_id, project_id, asset_version_id, domain_pack_id, file_name, object_path, sha256, byte_size, triangle_count, bounds_mm_json, mesh_count, primitive_count, material_count, node_count, created_at FROM agent_imported_glbs ORDER BY import_id",
        ),
        (
            "active_design_snapshots",
            "SELECT project_id, source, active_asset_version_id, active_assembly_graph_id, legacy_version_id, legacy_module_graph_id, selected_part_id, preview_change_set_id, preview_base_asset_version_id, quality_report_id, quality_asset_version_id, export_source, export_source_version_id, revision, updated_at, render_preset_json, selected_material_zone_id, part_display_json FROM active_design_snapshots ORDER BY project_id",
        ),
        (
            "agent_asset_quality_reports",
            "SELECT quality_report_id, project_id, asset_version_id, report_json, status, created_at FROM agent_asset_quality_reports ORDER BY quality_report_id",
        ),
    ] {
        hasher.update((label.len() as u64).to_le_bytes());
        hasher.update(label.as_bytes());
        update_query_hash(connection, sql, &mut hasher)?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn update_query_hash(connection: &Connection, sql: &str, hasher: &mut Sha256) -> CoreResult<()> {
    let mut statement = connection.prepare(sql)?;
    let column_count = statement.column_count();
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        hasher.update(b"row");
        for index in 0..column_count {
            match row.get_ref(index)? {
                ValueRef::Null => hasher.update([0]),
                ValueRef::Integer(value) => {
                    hasher.update([1]);
                    hasher.update(value.to_le_bytes());
                }
                ValueRef::Real(value) => {
                    hasher.update([2]);
                    hasher.update(value.to_bits().to_le_bytes());
                }
                ValueRef::Text(value) => {
                    hasher.update([3]);
                    hasher.update((value.len() as u64).to_le_bytes());
                    hasher.update(value);
                }
                ValueRef::Blob(value) => {
                    hasher.update([4]);
                    hasher.update((value.len() as u64).to_le_bytes());
                    hasher.update(value);
                }
            }
        }
    }
    Ok(())
}

fn decode_inline_glb(value: &str, max_bytes: usize) -> Result<Vec<u8>, &'static str> {
    let maximum_encoded = max_bytes.saturating_add(2) / 3 * 4;
    if value.len() > maximum_encoded {
        return Err("LEGACY_GLB_BASE64_OVERSIZED");
    }
    let bytes = BASE64_STANDARD
        .decode(value)
        .map_err(|_| "LEGACY_GLB_BASE64_INVALID")?;
    if bytes.len() > max_bytes {
        return Err("LEGACY_GLB_BASE64_OVERSIZED");
    }
    Ok(bytes)
}

fn validate_interactive_candidate_glb(bytes: &[u8]) -> Result<CandidateValidation, &'static str> {
    let (document, _) = parse_glb_chunks(bytes, MAX_INLINE_GLB_BYTES)?;
    let profile = document
        .get("extras")
        .and_then(|value| value.get("forgecad_geometry_artifact_profile"))
        .ok_or("LEGACY_GLB_PROFILE_MISSING")?;
    let profile_id = profile
        .get("artifact_profile_id")
        .and_then(Value::as_str)
        .ok_or("LEGACY_GLB_PROFILE_STALE")?;
    if profile_id == "production_concept" {
        return Err("LEGACY_CANDIDATE_PRODUCTION_UNPROVEN");
    }
    if profile_id != "interactive_preview" {
        return Err("LEGACY_GLB_PROFILE_STALE");
    }
    let mut expected = interactive_profile_contract();
    let profile_sha = semantic_sha256(&expected).map_err(|_| "LEGACY_GLB_PROFILE_STALE")?;
    expected["profile_sha256"] = Value::String(profile_sha);
    if profile != &expected {
        return Err("LEGACY_GLB_PROFILE_STALE");
    }
    let inspection = inspect_external_document(bytes, &document)?;
    let readback_sha256 = semantic_sha256(&json!({
        "artifact_profile": profile,
        "inspection": inspection,
        "glb_sha256": sha256_bytes(bytes),
    }))
    .map_err(|_| "LEGACY_GLB_READBACK_INVALID")?;
    Ok(CandidateValidation { readback_sha256 })
}

fn interactive_profile_contract() -> Value {
    json!({
        "schema_version": "GeometryArtifactProfile@1",
        "artifact_profile_id": "interactive_preview",
        "radial_segments": 24,
        "capsule_hemisphere_segments": 5,
        "smooth_loft_normals": false,
        "texture_width": 128,
        "texture_height": 128,
        "texture_mime_type": "image/png",
        "texture_compression": "png_deflate",
        "delivery": "interactive",
        "triangle_budget_multiplier": 1,
        "max_triangle_count": 100_000,
    })
}

fn inspect_external_glb(bytes: &[u8]) -> Result<ExternalGlbInspection, &'static str> {
    let (document, _) = parse_glb_chunks(bytes, MAX_IMPORTED_GLB_BYTES)?;
    inspect_external_document(bytes, &document)
}

fn inspect_external_document(
    bytes: &[u8],
    document: &Value,
) -> Result<ExternalGlbInspection, &'static str> {
    let (_, binary) = parse_glb_chunks(bytes, MAX_IMPORTED_GLB_BYTES.max(bytes.len()))?;
    if document
        .get("asset")
        .and_then(|asset| asset.get("version"))
        .and_then(Value::as_str)
        != Some("2.0")
    {
        return Err("LEGACY_EXTERNAL_GLB_INVALID");
    }
    if document
        .get("extensionsUsed")
        .and_then(Value::as_array)
        .is_some_and(|extensions| {
            extensions.iter().any(|extension| {
                matches!(
                    extension.as_str(),
                    Some("KHR_draco_mesh_compression" | "EXT_meshopt_compression")
                )
            })
        })
    {
        return Err("LEGACY_EXTERNAL_GLB_UNSUPPORTED_COMPRESSION");
    }
    let buffers = document
        .get("buffers")
        .and_then(Value::as_array)
        .filter(|buffers| buffers.len() == 1)
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    let buffer = buffers[0]
        .as_object()
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    if buffer.get("uri").is_some_and(|value| !value.is_null())
        || buffer
            .get("byteLength")
            .and_then(Value::as_u64)
            .is_none_or(|length| length > binary.len() as u64)
    {
        return Err("LEGACY_EXTERNAL_GLB_NOT_SELF_CONTAINED");
    }
    if document
        .get("images")
        .and_then(Value::as_array)
        .is_some_and(|images| images.iter().any(|image| image.get("uri").is_some()))
    {
        return Err("LEGACY_EXTERNAL_GLB_NOT_SELF_CONTAINED");
    }
    let accessors = document
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    let views = document
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    let meshes = document
        .get("meshes")
        .and_then(Value::as_array)
        .filter(|meshes| !meshes.is_empty())
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    let mut triangle_count = 0_u64;
    let mut primitive_count = 0_u64;
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
        for primitive in primitives {
            if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                return Err("LEGACY_EXTERNAL_GLB_INVALID");
            }
            let position_index = primitive
                .get("attributes")
                .and_then(|value| value.get("POSITION"))
                .and_then(Value::as_u64)
                .ok_or("LEGACY_EXTERNAL_GLB_INVALID")? as usize;
            let position = validate_accessor(accessors, views, &binary, position_index)?;
            if position.get("componentType").and_then(Value::as_u64) != Some(5126)
                || position.get("type").and_then(Value::as_str) != Some("VEC3")
            {
                return Err("LEGACY_EXTERNAL_GLB_INVALID");
            }
            let lower = finite_vec3(position.get("min"))?;
            let upper = finite_vec3(position.get("max"))?;
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(lower[axis]);
                maximum[axis] = maximum[axis].max(upper[axis]);
            }
            let index_count = if let Some(index) = primitive.get("indices") {
                let index = index.as_u64().ok_or("LEGACY_EXTERNAL_GLB_INVALID")? as usize;
                let accessor = validate_accessor(accessors, views, &binary, index)?;
                if accessor.get("type").and_then(Value::as_str) != Some("SCALAR")
                    || !matches!(
                        accessor.get("componentType").and_then(Value::as_u64),
                        Some(5121 | 5123 | 5125)
                    )
                {
                    return Err("LEGACY_EXTERNAL_GLB_INVALID");
                }
                accessor
                    .get("count")
                    .and_then(Value::as_u64)
                    .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?
            } else {
                position
                    .get("count")
                    .and_then(Value::as_u64)
                    .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?
            };
            if index_count == 0 || index_count % 3 != 0 {
                return Err("LEGACY_EXTERNAL_GLB_INVALID");
            }
            triangle_count = triangle_count
                .checked_add(index_count / 3)
                .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
            primitive_count += 1;
        }
    }
    if triangle_count == 0 || triangle_count > MAX_IMPORTED_GLB_TRIANGLES {
        return Err("LEGACY_EXTERNAL_GLB_BUDGET_EXCEEDED");
    }
    let bounds_mm = std::array::from_fn(|axis| {
        let value = (maximum[axis] - minimum[axis]) * 1000.0;
        (value * 10_000.0).round() / 10_000.0
    });
    Ok(ExternalGlbInspection {
        byte_size: bytes.len() as u64,
        triangle_count,
        bounds_mm,
        mesh_count: meshes.len() as u64,
        primitive_count,
        material_count: document
            .get("materials")
            .and_then(Value::as_array)
            .map_or(0, |items| items.len() as u64),
        node_count: document
            .get("nodes")
            .and_then(Value::as_array)
            .map_or(0, |items| items.len() as u64),
    })
}

fn parse_glb_chunks(bytes: &[u8], max_bytes: usize) -> Result<(Value, Vec<u8>), &'static str> {
    if bytes.len() < 20 || bytes.len() > max_bytes || bytes.get(..4) != Some(b"glTF") {
        return Err("LEGACY_GLB_CONTAINER_INVALID");
    }
    let version = read_u32(bytes, 4)?;
    let declared = read_u32(bytes, 8)? as usize;
    if version != 2 || declared != bytes.len() {
        return Err("LEGACY_GLB_CONTAINER_INVALID");
    }
    let mut cursor = 12_usize;
    let mut document = None;
    let mut binary = None;
    while cursor < bytes.len() {
        if cursor + 8 > bytes.len() {
            return Err("LEGACY_GLB_CONTAINER_INVALID");
        }
        let length = read_u32(bytes, cursor)? as usize;
        let kind = read_u32(bytes, cursor + 4)?;
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or("LEGACY_GLB_CONTAINER_INVALID")?;
        match kind {
            0x4e4f534a if document.is_none() => {
                let json_bytes = trim_glb_json_padding(&bytes[start..end]);
                document = Some(
                    serde_json::from_slice::<Value>(json_bytes)
                        .map_err(|_| "LEGACY_GLB_CONTAINER_INVALID")?,
                );
            }
            0x004e4942 if binary.is_none() => binary = Some(bytes[start..end].to_vec()),
            0x4e4f534a | 0x004e4942 => return Err("LEGACY_GLB_CONTAINER_INVALID"),
            _ => {}
        }
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err("LEGACY_GLB_CONTAINER_INVALID");
    }
    let document = document
        .filter(Value::is_object)
        .ok_or("LEGACY_GLB_CONTAINER_INVALID")?;
    let binary = binary
        .filter(|value| !value.is_empty())
        .ok_or("LEGACY_GLB_CONTAINER_INVALID")?;
    Ok((document, binary))
}

fn trim_glb_json_padding(mut bytes: &[u8]) -> &[u8] {
    while bytes.last().is_some_and(|byte| matches!(*byte, b' ' | 0)) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .ok_or("LEGACY_GLB_CONTAINER_INVALID")?;
    Ok(u32::from_le_bytes(raw))
}

fn validate_accessor<'a>(
    accessors: &'a [Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
) -> Result<&'a Value, &'static str> {
    let accessor = accessors
        .get(accessor_index)
        .filter(|value| value.is_object())
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    if accessor.get("sparse").is_some() {
        return Err("LEGACY_EXTERNAL_GLB_INVALID");
    }
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")? as usize;
    let view = views
        .get(view_index)
        .filter(|value| value.is_object())
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    if view.get("buffer").and_then(Value::as_u64).unwrap_or(0) != 0 {
        return Err("LEGACY_EXTERNAL_GLB_INVALID");
    }
    let component_type = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    let component_size = match component_type {
        5120 | 5121 => 1_u64,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return Err("LEGACY_EXTERNAL_GLB_INVALID"),
    };
    let component_count = match accessor.get("type").and_then(Value::as_str) {
        Some("SCALAR") => 1_u64,
        Some("VEC2") => 2,
        Some("VEC3") => 3,
        Some("VEC4") => 4,
        _ => return Err("LEGACY_EXTERNAL_GLB_INVALID"),
    };
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    let element_size = component_size * component_count;
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .unwrap_or(element_size);
    if stride < element_size {
        return Err("LEGACY_EXTERNAL_GLB_INVALID");
    }
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0);
    let view_length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    let required = accessor_offset
        .checked_add(
            count
                .saturating_sub(1)
                .checked_mul(stride)
                .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?,
        )
        .and_then(|value| value.checked_add(element_size))
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    if required > view_length
        || view_offset
            .checked_add(required)
            .is_none_or(|value| value > binary.len() as u64)
    {
        return Err("LEGACY_EXTERNAL_GLB_INVALID");
    }
    Ok(accessor)
}

fn finite_vec3(value: Option<&Value>) -> Result<[f64; 3], &'static str> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or("LEGACY_EXTERNAL_GLB_INVALID")?;
    let result = std::array::from_fn(|index| values[index].as_f64().unwrap_or(f64::NAN));
    if result.iter().all(|value| value.is_finite()) {
        Ok(result)
    } else {
        Err("LEGACY_EXTERNAL_GLB_INVALID")
    }
}

fn interrupted_error() -> CoreError {
    CoreError::conflict(
        "ARTIFACT_MIGRATION_INTERRUPTED",
        "Injected interruption left the durable migration ledger resumable.",
    )
}

fn system_timestamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("unix_ms_{millis}")
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc};

    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
    use rusqlite::{params, Connection};
    use tempfile::TempDir;

    use crate::{MigrationRunner, StateOwner};

    use super::*;

    struct Fixture {
        root: TempDir,
        db: PathBuf,
        lease: Arc<WriterLease>,
    }

    impl Fixture {
        fn new(instance_id: &str) -> Self {
            let root = tempfile::tempdir().unwrap();
            let db = root.path().join("library.db");
            MigrationRunner::new(&db).run().unwrap();
            let lease = WriterLease::acquire(
                &db,
                root.path(),
                instance_id,
                StateOwner::PythonCompatibilityAdapter,
            )
            .unwrap();
            Self { root, db, lease }
        }

        fn runner(&self) -> ArtifactMigrationRunner {
            ArtifactMigrationRunner::new(&self.db, self.root.path())
        }
    }

    #[test]
    fn empty_inventory_is_a_ready_noop_with_unchanged_semantics() {
        let fixture = Fixture::new("artifact_empty");
        let report = fixture.runner().run(&fixture.lease).unwrap();
        assert_eq!(report.total_items, 0);
        assert_eq!(report.migrated_items, 0);
        assert_eq!(report.legacy_read_only_items, 0);
        assert_eq!(report.semantic_sha256_before, report.semantic_sha256_after);
        let connection = Connection::open(&fixture.db).unwrap();
        let state: String = connection
            .query_row(
                "SELECT state FROM forgecad_core_artifact_migration_runs WHERE run_id=?",
                [&report.run_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "ready");
    }

    #[test]
    fn ready_inventory_fast_path_never_reclassifies_later_rust_native_versions() {
        let fixture = Fixture::new("artifact_native_after_cutover");
        let first = fixture.runner().run(&fixture.lease).unwrap();
        insert_version(
            &fixture.db,
            "asset_native_after_cutover_v1",
            "project_native_after_cutover",
            "artifact_native_after_cutover",
        );

        let second = fixture.runner().run(&fixture.lease).unwrap();
        assert_eq!(first, second);
        let connection = Connection::open(&fixture.db).unwrap();
        let counts: (i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM forgecad_core_artifact_migration_runs), (SELECT COUNT(*) FROM forgecad_core_asset_migration_cohort), (SELECT COUNT(*) FROM forgecad_core_asset_migration_status)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 0, 0));
    }

    #[test]
    fn inline_interactive_candidate_is_adopted_but_never_promoted_to_production() {
        let fixture = Fixture::new("artifact_interactive");
        let glb = test_glb(true, "interactive-a");
        let encoded = BASE64_STANDARD.encode(&glb);
        insert_candidate(&fixture.db, "artifact_interactive", &encoded);
        insert_version(
            &fixture.db,
            "asset_interactive_v1",
            "project_interactive",
            "artifact_interactive",
        );

        let report = fixture.runner().run(&fixture.lease).unwrap();
        assert_eq!(report.migrated_items, 1);
        assert_eq!(report.legacy_read_only_items, 0);
        let connection = Connection::open(&fixture.db).unwrap();
        let roles = connection
            .prepare(
                "SELECT reference_kind, role FROM forgecad_core_object_references ORDER BY role",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            roles,
            vec![("candidate".to_string(), ROLE_LEGACY_INTERACTIVE.to_string())]
        );
        let production_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM forgecad_core_object_references WHERE role='production_glb'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let candidate_object_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM forgecad_core_candidate_objects",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(production_count, 0);
        assert_eq!(candidate_object_count, 0);
        let status: (String, String) = connection
            .query_row(
                "SELECT state, reason_code FROM forgecad_core_asset_migration_status WHERE asset_version_id='asset_interactive_v1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            status,
            (
                "artifact_recompile_required".to_string(),
                "LEGACY_DUAL_PROFILE_RECOMPILE_REQUIRED".to_string(),
            )
        );
        let legacy_value: String = connection
            .query_row(
                "SELECT glb_base64 FROM agent_blockout_candidates WHERE artifact_id='artifact_interactive'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy_value, encoded);
    }

    #[test]
    fn imported_glb_is_adopted_in_place_as_explicit_read_only_reference() {
        let fixture = Fixture::new("artifact_external");
        insert_version(
            &fixture.db,
            "asset_external_v1",
            "project_external",
            "artifact_external",
        );
        let glb = test_glb(false, "external-a");
        let store = ContentAddressedObjectStore::new(fixture.root.path()).unwrap();
        let mut promoted = store.stage(&glb, "glb").unwrap().promote().unwrap();
        let stored = promoted.metadata().clone();
        promoted.finalize_commit().unwrap();
        insert_imported_glb(
            &fixture.db,
            "import_external",
            "asset_external_v1",
            &stored,
            &glb,
        );

        let report = fixture.runner().run(&fixture.lease).unwrap();
        assert_eq!(report.migrated_items, 1);
        let connection = Connection::open(&fixture.db).unwrap();
        let reference: (String, String, String) = connection
            .query_row(
                "SELECT reference_kind, role, sha256 FROM forgecad_core_object_references",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(reference.0, "asset_version");
        assert_eq!(reference.1, ROLE_EXTERNAL_REFERENCE);
        assert_eq!(reference.2, stored.sha256);
        let status: (String, String) = connection
            .query_row(
                "SELECT state, reason_code FROM forgecad_core_asset_migration_status WHERE asset_version_id='asset_external_v1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            status,
            (
                "external_reference_read_only".to_string(),
                "EXTERNAL_REFERENCE_ONLY".to_string(),
            )
        );
        let production_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM forgecad_core_object_references WHERE role='production_glb'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(production_count, 0);
    }

    #[test]
    fn wrong_profile_dual_references_are_never_classified_ready() {
        let fixture = Fixture::new("artifact_wrong_profile");
        insert_version(
            &fixture.db,
            "asset_wrong_profile_v1",
            "project_wrong_profile",
            "artifact_wrong_profile",
        );
        insert_asset_object_reference(
            &fixture,
            "asset_wrong_profile_v1",
            "production_glb",
            &test_glb(true, "wrong-production-profile"),
        );
        insert_asset_object_reference(
            &fixture,
            "asset_wrong_profile_v1",
            "interactive_preview_glb",
            &test_glb(true, "interactive-profile"),
        );

        fixture.runner().run(&fixture.lease).unwrap();
        assert_asset_recompile_required(&fixture.db, "asset_wrong_profile_v1");
    }

    #[test]
    fn tampered_dual_profile_object_is_never_classified_ready() {
        let fixture = Fixture::new("artifact_tampered_profile");
        insert_version(
            &fixture.db,
            "asset_tampered_profile_v1",
            "project_tampered_profile",
            "artifact_tampered_profile",
        );
        let production = insert_asset_object_reference(
            &fixture,
            "asset_tampered_profile_v1",
            "production_glb",
            &test_glb(false, "tampered-production"),
        );
        insert_asset_object_reference(
            &fixture,
            "asset_tampered_profile_v1",
            "interactive_preview_glb",
            &test_glb(true, "tampered-interactive"),
        );
        fs::write(
            fixture
                .root
                .path()
                .join("objects/sha256")
                .join(&production.relative_path),
            b"tampered",
        )
        .unwrap();

        fixture.runner().run(&fixture.lease).unwrap();
        assert_asset_recompile_required(&fixture.db, "asset_tampered_profile_v1");
    }

    #[test]
    fn invalid_and_oversized_inline_base64_are_explicit_read_only_outcomes() {
        let fixture = Fixture::new("artifact_invalid");
        insert_candidate(&fixture.db, "artifact_invalid", "%%%not-base64%%%");
        let oversized = BASE64_STANDARD.encode(vec![0_u8; 20]);
        insert_candidate(&fixture.db, "artifact_oversized", &oversized);
        let runner = fixture.runner().with_max_inline_glb_bytes(16);
        let report = runner.run(&fixture.lease).unwrap();
        assert_eq!(report.total_items, 2);
        assert_eq!(report.migrated_items, 0);
        assert_eq!(report.legacy_read_only_items, 2);
        let connection = Connection::open(&fixture.db).unwrap();
        let reasons = connection
            .prepare(
                "SELECT reason_code FROM forgecad_core_artifact_migration_items ORDER BY reason_code",
            )
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            reasons,
            vec![
                "LEGACY_GLB_BASE64_INVALID".to_string(),
                "LEGACY_GLB_BASE64_OVERSIZED".to_string(),
            ]
        );
        let reference_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM forgecad_core_object_references",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reference_count, 0);
    }

    #[test]
    fn repeated_run_is_idempotent_and_keeps_one_object_reference() {
        let fixture = Fixture::new("artifact_replay");
        let encoded = BASE64_STANDARD.encode(test_glb(true, "interactive-replay"));
        insert_candidate(&fixture.db, "artifact_replay", &encoded);
        let first = fixture.runner().run(&fixture.lease).unwrap();
        let second = fixture.runner().run(&fixture.lease).unwrap();
        assert_eq!(first.run_id, second.run_id);
        assert_eq!(first.inventory_sha256, second.inventory_sha256);
        let connection = Connection::open(&fixture.db).unwrap();
        let counts: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM forgecad_core_objects), (SELECT COUNT(*) FROM forgecad_core_object_references), (SELECT ref_count FROM forgecad_core_objects), (SELECT attempt_count FROM forgecad_core_artifact_migration_items)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 1, 1, 1));
    }

    #[test]
    fn changed_source_after_classification_is_blocked_without_repointing_reference() {
        let fixture = Fixture::new("artifact_mutation");
        let first_glb = test_glb(true, "interactive-before");
        insert_candidate(
            &fixture.db,
            "artifact_mutation",
            &BASE64_STANDARD.encode(&first_glb),
        );
        fixture.runner().run(&fixture.lease).unwrap();
        let first_sha = sha256_bytes(&first_glb);
        let changed = BASE64_STANDARD.encode(test_glb(true, "interactive-after"));
        Connection::open(&fixture.db)
            .unwrap()
            .execute(
                "UPDATE agent_blockout_candidates SET glb_base64=? WHERE artifact_id='artifact_mutation'",
                [changed],
            )
            .unwrap();
        let error = fixture.runner().run(&fixture.lease).unwrap_err();
        assert_eq!(error.code(), "LEGACY_ARTIFACT_SOURCE_MUTATED");
        let connection = Connection::open(&fixture.db).unwrap();
        let referenced_sha: String = connection
            .query_row(
                "SELECT sha256 FROM forgecad_core_object_references",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(referenced_sha, first_sha);
        let blocked: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM forgecad_core_artifact_migration_runs WHERE state='blocked' AND error_code='LEGACY_ARTIFACT_SOURCE_MUTATED'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(blocked, 1);
    }

    #[test]
    fn interruption_before_database_commit_removes_orphan_and_resumes() {
        let fixture = Fixture::new("artifact_interrupt_pre_db");
        let encoded = BASE64_STANDARD.encode(test_glb(true, "interactive-interrupt-pre"));
        insert_candidate(&fixture.db, "artifact_interrupt_pre", &encoded);
        let runner = fixture.runner();
        let error = runner
            .run_with_fault(&fixture.lease, MigrationFault::AfterPromoteBeforeDatabase)
            .unwrap_err();
        assert_eq!(error.code(), "ARTIFACT_MIGRATION_INTERRUPTED");
        assert_eq!(
            regular_file_count(&fixture.root.path().join("objects/.pending")),
            1
        );

        let resumed = runner.run(&fixture.lease).unwrap();
        assert_eq!(resumed.recovered_pending_objects, 1);
        assert_eq!(resumed.migrated_items, 1);
        assert_eq!(
            regular_file_count(&fixture.root.path().join("objects/.pending")),
            0
        );
        let connection = Connection::open(&fixture.db).unwrap();
        let counts: (i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM forgecad_core_objects), (SELECT COUNT(*) FROM forgecad_core_object_references)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 1));
    }

    #[test]
    fn interruption_after_database_commit_keeps_verified_object_and_clears_journal() {
        let fixture = Fixture::new("artifact_interrupt_post_db");
        let encoded = BASE64_STANDARD.encode(test_glb(true, "interactive-interrupt-post"));
        insert_candidate(&fixture.db, "artifact_interrupt_post", &encoded);
        let runner = fixture.runner();
        let error = runner
            .run_with_fault(&fixture.lease, MigrationFault::AfterDatabaseBeforeFinalize)
            .unwrap_err();
        assert_eq!(error.code(), "ARTIFACT_MIGRATION_INTERRUPTED");
        assert_eq!(
            regular_file_count(&fixture.root.path().join("objects/.pending")),
            1
        );

        let resumed = runner.run(&fixture.lease).unwrap();
        assert_eq!(resumed.migrated_items, 1);
        assert_eq!(
            regular_file_count(&fixture.root.path().join("objects/.pending")),
            0
        );
        let connection = Connection::open(&fixture.db).unwrap();
        let counts: (i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM forgecad_core_objects), (SELECT COUNT(*) FROM forgecad_core_object_references), (SELECT ref_count FROM forgecad_core_objects)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts, (1, 1, 1));
    }

    fn insert_candidate(db: &Path, artifact_id: &str, glb_base64: &str) {
        Connection::open(db)
            .unwrap()
            .execute(
                "INSERT INTO agent_blockout_candidates(artifact_id, project_id, plan_id, direction_id, domain_pack_id, status, candidate_json, shape_program_json, assembly_graph_json, material_bindings_json, glb_base64, created_at, updated_at) VALUES (?, NULL, 'plan_legacy', 'direction_legacy', 'pack_weapon_concept_v1', 'candidate', '{}', ?, '{}', '{}', ?, '2026-07-17T00:00:00Z', '2026-07-17T00:00:00Z')",
                params![
                    artifact_id,
                    serde_json::to_string(&json!({
                        "schema_version":"ShapeProgram@1",
                        "program_id":format!("shape_{artifact_id}"),
                    }))
                    .unwrap(),
                    glb_base64,
                ],
            )
            .unwrap();
    }

    fn insert_version(db: &Path, asset_version_id: &str, project_id: &str, artifact_id: &str) {
        Connection::open(db)
            .unwrap()
            .execute(
                "INSERT INTO agent_asset_versions(asset_version_id, project_id, parent_asset_version_id, version_no, status, summary, stage, plan_id, direction_id, domain_pack_id, artifact_id, parts_json, shape_program_json, assembly_graph_json, material_bindings_json, created_at) VALUES (?, ?, NULL, 1, 'committed', 'legacy asset', 'segmented_concept', 'plan_legacy', 'direction_legacy', 'pack_weapon_concept_v1', ?, '[]', ?, '{}', '{}', '2026-07-17T00:00:01Z')",
                params![
                    asset_version_id,
                    project_id,
                    artifact_id,
                    serde_json::to_string(&json!({
                        "schema_version":"ShapeProgram@1",
                        "program_id":format!("shape_{asset_version_id}"),
                    }))
                    .unwrap(),
                ],
            )
            .unwrap();
    }

    fn insert_imported_glb(
        db: &Path,
        import_id: &str,
        asset_version_id: &str,
        stored: &StoredObject,
        glb: &[u8],
    ) {
        let inspection = inspect_external_glb(glb).unwrap();
        Connection::open(db)
            .unwrap()
            .execute(
                "INSERT INTO agent_imported_glbs(import_id, project_id, asset_version_id, domain_pack_id, file_name, object_path, sha256, byte_size, triangle_count, bounds_mm_json, mesh_count, primitive_count, material_count, node_count, created_at) VALUES (?, 'project_external', ?, 'pack_weapon_concept_v1', 'reference.glb', ?, ?, ?, ?, ?, ?, ?, ?, ?, '2026-07-17T00:00:02Z')",
                params![
                    import_id,
                    asset_version_id,
                    format!("objects/sha256/{}", stored.relative_path),
                    stored.sha256,
                    stored.byte_size,
                    inspection.triangle_count,
                    serde_json::to_string(&inspection.bounds_mm).unwrap(),
                    inspection.mesh_count,
                    inspection.primitive_count,
                    inspection.material_count,
                    inspection.node_count,
                ],
            )
            .unwrap();
    }

    fn insert_asset_object_reference(
        fixture: &Fixture,
        asset_version_id: &str,
        role: &str,
        glb: &[u8],
    ) -> StoredObject {
        let store = ContentAddressedObjectStore::new(fixture.root.path()).unwrap();
        let mut promoted = store.stage(glb, "glb").unwrap().promote().unwrap();
        let stored = promoted.metadata().clone();
        promoted.finalize_commit().unwrap();
        let connection = Connection::open(&fixture.db).unwrap();
        connection
            .execute(
                "INSERT OR IGNORE INTO forgecad_core_objects(sha256, object_path, extension, byte_size, ref_count, created_at, updated_at) VALUES (?, ?, ?, ?, 0, '2026-07-17T00:00:03Z', '2026-07-17T00:00:03Z')",
                params![
                    stored.sha256,
                    stored.relative_path,
                    stored.extension,
                    stored.byte_size,
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('asset_version', ?, ?, ?, '2026-07-17T00:00:03Z')",
                params![asset_version_id, role, stored.sha256],
            )
            .unwrap();
        stored
    }

    fn assert_asset_recompile_required(db: &Path, asset_version_id: &str) {
        let status: (String, String) = Connection::open(db)
            .unwrap()
            .query_row(
                "SELECT state, reason_code FROM forgecad_core_asset_migration_status WHERE asset_version_id=?",
                [asset_version_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            status,
            (
                "artifact_recompile_required".to_string(),
                "LEGACY_DUAL_PROFILE_RECOMPILE_REQUIRED".to_string(),
            )
        );
    }

    fn test_glb(with_interactive_profile: bool, label: &str) -> Vec<u8> {
        let positions = [
            0.0_f32, 0.0, 0.0, //
            1.0, 0.0, 0.0, //
            0.0, 1.0, 0.0,
        ];
        let indices = [0_u16, 1, 2];
        let mut binary = Vec::new();
        for value in positions {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        let index_offset = binary.len();
        for value in indices {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let mut document = json!({
            "asset":{"version":"2.0","generator":label},
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"mesh":0}],
            "meshes":[{"primitives":[{
                "attributes":{"POSITION":0},
                "indices":1,
                "material":0,
                "mode":4
            }]}],
            "materials":[{}],
            "buffers":[{"byteLength":binary.len()}],
            "bufferViews":[
                {"buffer":0,"byteOffset":0,"byteLength":index_offset},
                {"buffer":0,"byteOffset":index_offset,"byteLength":6,"target":34963}
            ],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},
                {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}
            ]
        });
        if with_interactive_profile {
            let mut profile = interactive_profile_contract();
            profile["profile_sha256"] = Value::String(semantic_sha256(&profile).unwrap());
            document["extras"] = json!({
                "forgecad_geometry_artifact_profile": profile,
            });
        }
        let mut json_chunk = serde_json::to_vec(&document).unwrap();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(b' ');
        }
        let total_length = 12 + 8 + json_chunk.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json_chunk);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(&binary);
        glb
    }

    fn regular_file_count(path: &Path) -> usize {
        if !path.exists() {
            return 0;
        }
        fs::read_dir(path)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .count()
    }
}
