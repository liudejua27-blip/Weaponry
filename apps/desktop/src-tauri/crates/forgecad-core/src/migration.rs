use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{
    filesystem_permissions::{
        ensure_private_directory_tree, ensure_private_file, secure_sqlite_files,
    },
    CoreError, CoreResult,
};

pub const CURRENT_LEGACY_MIGRATION: &str = "0034";
const BUSY_TIMEOUT_MS: u64 = 5_000;

struct Migration {
    version: &'static str,
    name: &'static str,
    sql: &'static str,
}

macro_rules! legacy_migration {
    ($version:literal, $name:literal, $file:literal) => {
        Migration {
            version: $version,
            name: $name,
            sql: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../migrations/",
                $file
            )),
        }
    };
}

const LEGACY_MIGRATIONS: &[Migration] = &[
    legacy_migration!("0001", "init", "0001_init.sql"),
    legacy_migration!(
        "0002",
        "m4_idempotency_records",
        "0002_m4_idempotency_records.sql"
    ),
    legacy_migration!(
        "0003",
        "m4_concept_patch_role",
        "0003_m4_concept_patch_role.sql"
    ),
    legacy_migration!(
        "0004",
        "m4_asset_file_content_reuse",
        "0004_m4_asset_file_content_reuse.sql"
    ),
    legacy_migration!("0005", "p0_job_actions", "0005_p0_job_actions.sql"),
    legacy_migration!(
        "0006",
        "p0_runtime_recovery",
        "0006_p0_runtime_recovery.sql"
    ),
    legacy_migration!(
        "0007",
        "p0_job_history_indexes",
        "0007_p0_job_history_indexes.sql"
    ),
    legacy_migration!(
        "0008",
        "m6_structure_recast",
        "0008_m6_structure_recast.sql"
    ),
    legacy_migration!("0009", "r2_concept_domain", "0009_r2_concept_domain.sql"),
    legacy_migration!(
        "0010",
        "r2_change_set_preview",
        "0010_r2_change_set_preview.sql"
    ),
    legacy_migration!("0011", "r2_concept_jobs", "0011_r2_concept_jobs.sql"),
    legacy_migration!(
        "0012",
        "r3_change_set_audit",
        "0012_r3_change_set_audit.sql"
    ),
    legacy_migration!(
        "0013",
        "quality_geometry_refs",
        "0013_quality_geometry_refs.sql"
    ),
    legacy_migration!(
        "0014",
        "concept_planner_provenance",
        "0014_concept_planner_provenance.sql"
    ),
    legacy_migration!(
        "0015",
        "change_planner_provenance",
        "0015_change_planner_provenance.sql"
    ),
    legacy_migration!(
        "0016",
        "change_set_audit_exports",
        "0016_change_set_audit_exports.sql"
    ),
    legacy_migration!(
        "0017",
        "module_asset_catalog_metadata",
        "0017_module_asset_catalog_metadata.sql"
    ),
    legacy_migration!(
        "0018",
        "concept_quality_job_queue",
        "0018_concept_quality_job_queue.sql"
    ),
    legacy_migration!("0019", "agent_kernel", "0019_agent_kernel.sql"),
    legacy_migration!(
        "0020",
        "agent_asset_editing",
        "0020_agent_asset_editing.sql"
    ),
    legacy_migration!(
        "0021",
        "agent_component_registry",
        "0021_agent_component_registry.sql"
    ),
    legacy_migration!(
        "0022",
        "agent_external_glb_import",
        "0022_agent_external_glb_import.sql"
    ),
    legacy_migration!(
        "0023",
        "active_design_snapshots",
        "0023_active_design_snapshots.sql"
    ),
    legacy_migration!(
        "0024",
        "legacy_agent_conversion_intents",
        "0024_legacy_agent_conversion_intents.sql"
    ),
    legacy_migration!(
        "0025",
        "agent_asset_quality_reports",
        "0025_agent_asset_quality_reports.sql"
    ),
    legacy_migration!(
        "0026",
        "agent_asset_navigation_frames",
        "0026_agent_asset_navigation_frames.sql"
    ),
    legacy_migration!(
        "0027",
        "agent_clarification_items",
        "0027_agent_clarification_items.sql"
    ),
    legacy_migration!(
        "0028",
        "active_design_render_presets",
        "0028_active_design_render_presets.sql"
    ),
    legacy_migration!(
        "0029",
        "agent_material_texture_objects",
        "0029_agent_material_texture_objects.sql"
    ),
    legacy_migration!(
        "0030",
        "active_design_selected_material_zone",
        "0030_active_design_selected_material_zone.sql"
    ),
    legacy_migration!(
        "0031",
        "active_design_part_display",
        "0031_active_design_part_display.sql"
    ),
    legacy_migration!(
        "0032",
        "agent_provider_conversations",
        "0032_agent_provider_conversations.sql"
    ),
    legacy_migration!(
        "0033",
        "agent_provider_budget",
        "0033_agent_provider_budget.sql"
    ),
    legacy_migration!(
        "0034",
        "expand_material_texture_roles",
        "0034_expand_material_texture_roles.sql"
    ),
];

const CORE_MIGRATIONS: &[Migration] = &[
    legacy_migration!(
        "0035",
        "k003_rust_core_ownership",
        "0035_k003_rust_core_ownership.sql"
    ),
    legacy_migration!(
        "0036",
        "k003_legacy_artifact_adoption",
        "0036_k003_legacy_artifact_adoption.sql"
    ),
    legacy_migration!(
        "0037",
        "k003_cas_deletion_journal",
        "0037_k003_cas_deletion_journal.sql"
    ),
    legacy_migration!("0038", "agent_skills", "0038_agent_skills.sql"),
    legacy_migration!("0039", "reference_evidence", "0039_reference_evidence.sql"),
    legacy_migration!(
        "0040",
        "reference_surface_pairs",
        "0040_reference_surface_pairs.sql"
    ),
    legacy_migration!(
        "0041",
        "reference_evidence_class",
        "0041_reference_evidence_class.sql"
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub applied_legacy_versions: Vec<String>,
    pub current_legacy_version: String,
    pub core_schema_applied: bool,
    pub journal_mode: String,
}

#[derive(Debug, Clone)]
pub struct MigrationRunner {
    db_path: PathBuf,
}

impl MigrationRunner {
    pub fn new(db_path: impl AsRef<Path>) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
        }
    }

    pub fn run(&self) -> CoreResult<MigrationReport> {
        self.run_internal(None, None)
    }

    fn run_internal(
        &self,
        stop_after: Option<&str>,
        interrupt_after_sql: Option<&str>,
    ) -> CoreResult<MigrationReport> {
        let mut connection = open_connection(&self.db_path)?;
        let journal_mode = set_and_read_wal(&connection, &self.db_path)?;
        let mut applied_now = Vec::new();

        for migration in LEGACY_MIGRATIONS {
            if let Some(stop) = stop_after {
                if migration.version > stop {
                    break;
                }
            }
            let applied = applied_legacy_versions(&connection)?;
            if applied.contains(migration.version) {
                continue;
            }
            let body = migration_body(migration.sql);
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let result = (|| -> CoreResult<()> {
                transaction.execute_batch(&body)?;
                if interrupt_after_sql == Some(migration.version) {
                    return Err(CoreError::conflict(
                        "MIGRATION_INTERRUPTED",
                        "Injected interruption before migration commit.",
                    ));
                }
                let marker: Option<String> = transaction
                    .query_row(
                        "SELECT version FROM schema_migrations WHERE version = ?",
                        [migration.version],
                        |row| row.get(0),
                    )
                    .optional()?;
                if marker.is_none() {
                    transaction.execute(
                        "INSERT INTO schema_migrations(version, name) VALUES (?, ?)",
                        params![migration.version, migration.name],
                    )?;
                }
                Ok(())
            })();
            if let Err(source) = result {
                return Err(CoreError::Migration {
                    version: migration.version.to_string(),
                    source: Box::new(source),
                });
            }
            transaction
                .commit()
                .map_err(|source| CoreError::Migration {
                    version: migration.version.to_string(),
                    source: Box::new(CoreError::Sqlite(source)),
                })?;
            applied_now.push(migration.version.to_string());
        }

        let reached_current =
            applied_legacy_versions(&connection)?.contains(CURRENT_LEGACY_MIGRATION);
        let core_schema_applied = if reached_current && stop_after.is_none() {
            apply_core_schema(&mut connection)?
        } else {
            false
        };
        secure_sqlite_files(&self.db_path)?;
        Ok(MigrationReport {
            applied_legacy_versions: applied_now,
            current_legacy_version: CURRENT_LEGACY_MIGRATION.to_string(),
            core_schema_applied,
            journal_mode,
        })
    }

    #[cfg(test)]
    fn run_to(&self, stop_after: &str) -> CoreResult<MigrationReport> {
        self.run_internal(Some(stop_after), None)
    }

    #[cfg(test)]
    fn run_with_interruption(&self, version: &str) -> CoreResult<MigrationReport> {
        self.run_internal(None, Some(version))
    }
}

pub(crate) fn open_connection(db_path: &Path) -> CoreResult<Connection> {
    if let Some(parent) = db_path.parent() {
        ensure_private_directory_tree(parent)?;
    }
    let database = ensure_private_file(db_path)?;
    database.sync_all()?;
    drop(database);
    let connection = Connection::open(db_path)?;
    connection.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    secure_sqlite_files(db_path)?;
    Ok(connection)
}

fn set_and_read_wal(connection: &Connection, db_path: &Path) -> CoreResult<String> {
    let mode: String = connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(CoreError::conflict(
            "SQLITE_WAL_REQUIRED",
            "ForgeCAD product-state database did not enter WAL mode.",
        ));
    }
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "busy_timeout", BUSY_TIMEOUT_MS)?;
    secure_sqlite_files(db_path)?;
    Ok(mode.to_ascii_lowercase())
}

fn applied_legacy_versions(connection: &Connection) -> CoreResult<BTreeSet<String>> {
    let has_table: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
        [],
        |row| row.get(0),
    )?;
    if !has_table {
        return Ok(BTreeSet::new());
    }
    let mut statement = connection.prepare("SELECT version FROM schema_migrations")?;
    let versions = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(versions)
}

fn migration_body(sql: &str) -> String {
    sql.lines()
        .filter(|line| {
            let normalized = line.trim().to_ascii_uppercase().replace(' ', "");
            normalized != "BEGINIMMEDIATE;"
                && normalized != "COMMIT;"
                && normalized != "PRAGMAJOURNAL_MODE=WAL;"
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn apply_core_schema(connection: &mut Connection) -> CoreResult<bool> {
    let mut applied_any = false;
    for migration in CORE_MIGRATIONS {
        let has_ledger: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='forgecad_core_schema_migrations')",
            [],
            |row| row.get(0),
        )?;
        let already_applied = has_ledger
            && connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM forgecad_core_schema_migrations WHERE version = ?)",
                [migration.version],
                |row| row.get(0),
            )?;
        if already_applied {
            continue;
        }
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(&migration_body(migration.sql))?;
        transaction.commit()?;
        applied_any = true;
    }
    Ok(applied_any)
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use std::fs;
    use tempfile::tempdir;

    use crate::semantic_sha256;

    use super::*;

    #[test]
    fn empty_database_reaches_current_schema_in_wal_mode() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        let report = MigrationRunner::new(&db).run().unwrap();
        assert_eq!(report.journal_mode, "wal");
        assert_eq!(
            report.applied_legacy_versions.first().map(String::as_str),
            Some("0001")
        );
        assert_eq!(
            report.applied_legacy_versions.last().map(String::as_str),
            Some("0034")
        );
        assert!(report.core_schema_applied);

        let connection = open_connection(&db).unwrap();
        let versions = applied_legacy_versions(&connection).unwrap();
        assert_eq!(
            versions.len(),
            LEGACY_MIGRATIONS.len() + CORE_MIGRATIONS.len()
        );
        assert!(versions.contains("0026"));
        assert!(versions.contains("0035"));
        assert!(versions.contains("0039"));
        assert!(versions.contains("0040"));
        assert!(versions.contains("0041"));
        for table in [
            "projects",
            "agent_asset_versions",
            "active_design_snapshots",
            "forgecad_core_ownership",
            "forgecad_core_objects",
            "forgecad_core_artifact_migration_runs",
            "forgecad_core_artifact_migration_items",
            "forgecad_core_artifact_migration_state",
            "forgecad_core_asset_migration_cohort",
            "forgecad_core_asset_migration_status",
            "forgecad_core_object_deletion_journal",
            "agent_skill_versions",
            "agent_skill_activations",
            "agent_skill_eval_reports",
            "agent_asset_skill_references",
            "reference_evidence",
            "reference_guided_rebuild_plans",
            "reference_surface_analyses",
            "reference_rebuild_result_lineage",
        ] {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "missing table {table}");
        }
        let reference_class_column_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('reference_evidence') WHERE name='reference_class'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reference_class_column_count, 1);
    }

    #[test]
    fn historical_legacy_rows_keep_semantic_hash_after_upgrade() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        let runner = MigrationRunner::new(&db);
        runner.run_to("0023").unwrap();
        let connection = open_connection(&db).unwrap();
        let profile_json = r#"{"schema_version":"DesignDomainProfile@1","roles":["core_shell"]}"#;
        let spec_json = r#"{"schema_version":"WeaponConceptSpec@1","name":"legacy"}"#;
        let graph_json =
            r#"{"schema_version":"ModuleGraph@1","root_node_id":"node_root","nodes":[]}"#;
        let profile_sha = crate::canonical::sha256_bytes(profile_json.as_bytes());
        let spec_sha = crate::canonical::sha256_bytes(spec_json.as_bytes());
        let graph_sha = crate::canonical::sha256_bytes(graph_json.as_bytes());
        connection.execute(
            "INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at) VALUES (?, 'weapon_concept', 'DesignDomainProfile@1', 'pack_legacy', 'Legacy', ?, ?, 'active', ?, ?)",
            params!["profile_legacy", profile_json, profile_sha, "2026-07-17T00:00:00Z", "2026-07-17T00:00:00Z"],
        ).unwrap();
        connection.execute(
            "INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at) VALUES (?, ?, 'weapon_concept', 'Legacy Project', 'active', ?, ?, ?)",
            params!["project_legacy", "profile_legacy", "version_legacy", "2026-07-17T00:00:00Z", "2026-07-17T00:00:00Z"],
        ).unwrap();
        connection.execute(
            "INSERT INTO project_versions(version_id, project_id, parent_version_id, version_no, status, summary, spec_schema_version, spec_json, spec_sha256, module_graph_id, change_set_id, created_at) VALUES (?, ?, NULL, 1, 'committed', 'legacy', 'WeaponConceptSpec@1', ?, ?, ?, NULL, ?)",
            params!["version_legacy", "project_legacy", spec_json, spec_sha, "graph_legacy", "2026-07-17T00:00:00Z"],
        ).unwrap();
        connection.execute(
            "INSERT INTO module_graphs(graph_id, project_id, version_id, root_node_id, schema_version, graph_json, graph_sha256, validation_status, created_at, updated_at) VALUES (?, ?, ?, 'node_root', 'ModuleGraph@1', ?, ?, 'valid', ?, ?)",
            params!["graph_legacy", "project_legacy", "version_legacy", graph_json, graph_sha, "2026-07-17T00:00:00Z", "2026-07-17T00:00:00Z"],
        ).unwrap();
        drop(connection);
        let before = semantic_sha256(&serde_json::json!({
            "profile": profile_json,
            "spec": spec_json,
            "graph": graph_json,
        }))
        .unwrap();

        runner.run().unwrap();
        let connection = open_connection(&db).unwrap();
        let after_profile: String = connection
            .query_row(
                "SELECT profile_json FROM domain_profiles WHERE profile_id='profile_legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let after_spec: String = connection
            .query_row(
                "SELECT spec_json FROM project_versions WHERE version_id='version_legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let after_graph: String = connection
            .query_row(
                "SELECT graph_json FROM module_graphs WHERE graph_id='graph_legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let after = semantic_sha256(&serde_json::json!({
            "profile": after_profile,
            "spec": after_spec,
            "graph": after_graph,
        }))
        .unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn interruption_rolls_back_schema_and_marker_then_retries_cleanly() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        let runner = MigrationRunner::new(&db);
        runner.run_to("0022").unwrap();
        let error = runner.run_with_interruption("0023").unwrap_err();
        assert_eq!(error.code(), "SQLITE_MIGRATION_FAILED");
        let connection = open_connection(&db).unwrap();
        let table_exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='active_design_snapshots')",
            [], |row| row.get(0),
        ).unwrap();
        let marker_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version='0023')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!table_exists);
        assert!(!marker_exists);
        drop(connection);
        runner.run().unwrap();
    }

    #[test]
    fn missing_0026_ledger_marker_is_repaired_without_rewriting_the_table() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        let runner = MigrationRunner::new(&db);
        runner.run().unwrap();
        let connection = open_connection(&db).unwrap();
        connection
            .execute("DELETE FROM schema_migrations WHERE version='0026'", [])
            .unwrap();
        drop(connection);
        runner.run().unwrap();
        let connection = open_connection(&db).unwrap();
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version='0026'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[cfg(unix)]
    #[test]
    fn existing_library_and_sqlite_sidecars_are_tightened_idempotently() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        let runner = MigrationRunner::new(&db);
        runner.run().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&db, fs::Permissions::from_mode(0o644)).unwrap();
        runner.run().unwrap();

        let wal = PathBuf::from(format!("{}-wal", db.display()));
        let shm = PathBuf::from(format!("{}-shm", db.display()));
        fs::write(&wal, b"legacy wal marker").unwrap();
        fs::write(&shm, b"legacy shm marker").unwrap();
        fs::set_permissions(&wal, fs::Permissions::from_mode(0o644)).unwrap();
        fs::set_permissions(&shm, fs::Permissions::from_mode(0o644)).unwrap();

        secure_sqlite_files(&db).unwrap();
        assert_eq!(
            fs::metadata(root.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&db).unwrap().permissions().mode() & 0o777,
            0o600
        );
        for sidecar in [&wal, &shm] {
            assert_eq!(
                fs::metadata(sidecar).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        fs::remove_file(&wal).unwrap();
        fs::remove_file(&shm).unwrap();
        runner.run().unwrap();
        assert_eq!(
            fs::metadata(root.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&db).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
