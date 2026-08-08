mod cas;

pub use cas::{CasError, CasObject, CasStore};
use forgecad_contracts::{
    is_opaque_id, is_sha256, AuditEventRecord, CandidateRecord, CasObjectRecord,
    DesignAssetVersionRecord, JobEventRecord, JobSummary, ProjectRecord, ProjectSummary,
    SnapshotRecord, SnapshotSummary,
};
use forgecad_core::sha256_hex;
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MIGRATION_SQL: &str =
    include_str!("../../../../../../migrations-runtime-v1/0001_runtime.sql");
const RUNTIME_SCHEMA_VERSION: &str = "1";
const DEFAULT_LEASE_TTL_MS: i64 = 30_000;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("CAS error: {0}")]
    Cas(#[from] CasError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("writer lease is already held")]
    WriterLeaseHeld,
    #[error("writer lease is not held by this owner")]
    LeaseNotHeld,
    #[error("invalid runtime data: {0}")]
    InvalidData(String),
    #[error("database backup is unavailable for an in-memory store")]
    BackupUnavailable,
    #[error("database migration version is unsupported")]
    MigrationVersionUnsupported,
    #[error("legacy database is not a ForgeCAD Runtime V1 database")]
    LegacyDatabaseRejected,
    #[error("store mutex is poisoned")]
    LockPoisoned,
}

#[derive(Debug, Clone)]
pub struct LeaseGrant {
    pub owner: String,
    pub token: String,
    pub acquired_at_ms: i64,
}

#[derive(Clone)]
pub struct Store {
    connection: Arc<Mutex<Connection>>,
    cas: CasStore,
    database_path: Option<PathBuf>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        let cas_root = path.with_extension("cas");
        Self::open_with_cas(path, cas_root)
    }

    pub fn open_with_cas(
        path: impl AsRef<Path>,
        cas_root: impl AsRef<Path>,
    ) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        reject_legacy_database(&path)?;
        let mut connection = Connection::open(&path)?;
        configure_connection(&mut connection)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            cas: CasStore::new(cas_root)?,
            database_path: Some(path),
        })
    }

    pub fn memory() -> Result<Self, StoreError> {
        let mut connection = Connection::open_in_memory()?;
        configure_connection(&mut connection)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            cas: CasStore::ephemeral()?,
            database_path: None,
        })
    }

    pub fn cas(&self) -> &CasStore {
        &self.cas
    }

    pub fn acquire_writer_lease(&self, owner: &str) -> Result<LeaseGrant, StoreError> {
        let token = Uuid::new_v4().simple().to_string();
        self.acquire_writer_lease_with_token(owner, &token, now_ms(), DEFAULT_LEASE_TTL_MS)
    }

    pub fn acquire_writer_lease_with_token(
        &self,
        owner: &str,
        token: &str,
        now: i64,
        ttl_ms: i64,
    ) -> Result<LeaseGrant, StoreError> {
        validate_owner(owner)?;
        if token.is_empty() || token.len() > 256 {
            return Err(StoreError::InvalidData(
                "invalid writer lease token".to_owned(),
            ));
        }
        if ttl_ms < 0 {
            return Err(StoreError::InvalidData("negative lease ttl".to_owned()));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let existing = transaction
            .query_row(
                "SELECT owner, heartbeat_at FROM writer_lease WHERE lease_id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let token_hash = sha256_hex(token.as_bytes());
        match existing {
            None => {
                transaction.execute(
                    "INSERT INTO writer_lease (lease_id, owner, lease_token_hash, acquired_at, heartbeat_at) VALUES (1, ?1, ?2, ?3, ?3)",
                    params![owner, token_hash, now],
                )?;
            }
            Some((existing_owner, heartbeat)) if existing_owner == owner => {
                transaction.execute(
                    "UPDATE writer_lease SET lease_token_hash = ?1, heartbeat_at = ?2 WHERE lease_id = 1 AND owner = ?3",
                    params![token_hash, now, owner],
                )?;
                transaction.commit()?;
                return Ok(LeaseGrant {
                    owner: owner.to_owned(),
                    token: token.to_owned(),
                    acquired_at_ms: heartbeat,
                });
            }
            Some((_, heartbeat)) if now.saturating_sub(heartbeat) >= ttl_ms && ttl_ms >= 0 => {
                transaction.execute(
                    "UPDATE writer_lease SET owner = ?1, lease_token_hash = ?2, acquired_at = ?3, heartbeat_at = ?3 WHERE lease_id = 1",
                    params![owner, token_hash, now],
                )?;
            }
            Some(_) => return Err(StoreError::WriterLeaseHeld),
        }
        transaction.commit()?;
        Ok(LeaseGrant {
            owner: owner.to_owned(),
            token: token.to_owned(),
            acquired_at_ms: now,
        })
    }

    pub fn heartbeat_writer_lease(
        &self,
        owner: &str,
        token: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let connection = self.lock_connection()?;
        let token_hash = sha256_hex(token.as_bytes());
        let updated = connection.execute(
            "UPDATE writer_lease SET heartbeat_at = ?1 WHERE lease_id = 1 AND owner = ?2 AND lease_token_hash = ?3",
            params![now, owner, token_hash],
        )?;
        if updated == 0 {
            return Err(StoreError::LeaseNotHeld);
        }
        Ok(())
    }

    pub fn release_writer_lease(&self, owner: &str, token: &str) -> Result<(), StoreError> {
        let connection = self.lock_connection()?;
        let token_hash = sha256_hex(token.as_bytes());
        let updated = connection.execute(
            "DELETE FROM writer_lease WHERE lease_id = 1 AND owner = ?1 AND lease_token_hash = ?2",
            params![owner, token_hash],
        )?;
        if updated == 0 {
            return Err(StoreError::LeaseNotHeld);
        }
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>, StoreError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT project_id, name, updated_at, head_snapshot_id FROM projects ORDER BY updated_at DESC, project_id ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ProjectSummary {
                project_id: row.get(0)?,
                name: row.get(1)?,
                updated_at: row.get(2)?,
                head_snapshot_id: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_project(&self, project_id: &str) -> Result<Option<ProjectRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT project_id, name, policy_json, created_at, updated_at, active_snapshot_revision, head_snapshot_id, canonical_sha256 FROM projects WHERE project_id = ?1",
                params![project_id],
                |row| {
                    let policy_json: String = row.get(2)?;
                    let policy = serde_json::from_str(&policy_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    Ok(ProjectRecord {
                        schema_version: "Project@1".to_owned(),
                        project_id: row.get(0)?,
                        name: row.get(1)?,
                        policy,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        active_snapshot_revision: row.get(5)?,
                        head_snapshot_id: row.get(6)?,
                        canonical_sha256: row.get(7)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn insert_project(&self, project: &ProjectRecord) -> Result<(), StoreError> {
        validate_project(project)?;
        let policy_json = serde_json::to_string(&project.policy)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO projects (project_id, name, policy_json, created_at, updated_at, active_snapshot_revision, head_snapshot_id, canonical_sha256) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                project.project_id,
                project.name,
                policy_json,
                project.created_at,
                project.updated_at,
                project.active_snapshot_revision,
                project.head_snapshot_id,
                project.canonical_sha256,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_snapshot(&self, snapshot_id: &str) -> Result<Option<SnapshotSummary>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT snapshot_id, project_id, parent_snapshot_id, status, manifest_hash, created_at FROM snapshots WHERE snapshot_id = ?1",
                params![snapshot_id],
                |row| {
                    Ok(SnapshotSummary {
                        snapshot_id: row.get(0)?,
                        project_id: row.get(1)?,
                        parent_snapshot_id: row.get(2)?,
                        status: row.get(3)?,
                        manifest_hash: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn get_snapshot_record(
        &self,
        snapshot_id: &str,
    ) -> Result<Option<SnapshotRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT snapshot_id, project_id, parent_snapshot_id, candidate_id, revision, status, manifest_hash, canonical_sha256, created_at FROM snapshots WHERE snapshot_id = ?1",
                params![snapshot_id],
                |row| {
                    Ok(SnapshotRecord {
                        schema_version: "ActiveDesignSnapshot@1".to_owned(),
                        snapshot_id: row.get(0)?,
                        project_id: row.get(1)?,
                        parent_snapshot_id: row.get(2)?,
                        candidate_id: row.get(3)?,
                        revision: row.get(4)?,
                        status: row.get(5)?,
                        manifest_hash: row.get(6)?,
                        canonical_sha256: row.get(7)?,
                        created_at: row.get(8)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn insert_snapshot(&self, snapshot: &SnapshotRecord) -> Result<(), StoreError> {
        validate_snapshot(snapshot)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO snapshots (snapshot_id, project_id, parent_snapshot_id, candidate_id, revision, status, manifest_hash, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                snapshot.snapshot_id,
                snapshot.project_id,
                snapshot.parent_snapshot_id,
                snapshot.candidate_id,
                snapshot.revision,
                snapshot.status,
                snapshot.manifest_hash,
                snapshot.canonical_sha256,
                snapshot.created_at,
            ],
        )?;
        transaction.execute(
            "UPDATE projects SET active_snapshot_revision = ?1, head_snapshot_id = ?2, updated_at = ?3 WHERE project_id = ?4",
            params![snapshot.revision, snapshot.snapshot_id, snapshot.created_at, snapshot.project_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_candidate(&self, candidate: &CandidateRecord) -> Result<(), StoreError> {
        validate_candidate(candidate)?;
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO candidates (candidate_id, project_id, base_version_id, state, request_sha256, manifest_hash, canonical_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                candidate.candidate_id,
                candidate.project_id,
                candidate.base_version_id,
                candidate.state,
                candidate.request_sha256,
                candidate.manifest_hash,
                candidate.canonical_sha256,
                candidate.error_code,
                candidate.created_at,
                candidate.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_candidate(&self, candidate_id: &str) -> Result<Option<CandidateRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT candidate_id, project_id, base_version_id, state, request_sha256, manifest_hash, canonical_sha256, error_code, created_at, updated_at FROM candidates WHERE candidate_id = ?1",
                params![candidate_id],
                |row| {
                    Ok(CandidateRecord {
                        schema_version: "Candidate@1".to_owned(),
                        candidate_id: row.get(0)?,
                        project_id: row.get(1)?,
                        base_version_id: row.get(2)?,
                        state: row.get(3)?,
                        request_sha256: row.get(4)?,
                        manifest_hash: row.get(5)?,
                        canonical_sha256: row.get(6)?,
                        error_code: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn insert_version(&self, version: &DesignAssetVersionRecord) -> Result<(), StoreError> {
        validate_version(version)?;
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO design_asset_versions (version_id, project_id, parent_version_id, candidate_id, manifest_hash, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                version.version_id,
                version.project_id,
                version.parent_version_id,
                version.candidate_id,
                version.manifest_hash,
                version.canonical_sha256,
                version.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_version(
        &self,
        version_id: &str,
    ) -> Result<Option<DesignAssetVersionRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT version_id, project_id, parent_version_id, candidate_id, manifest_hash, canonical_sha256, created_at FROM design_asset_versions WHERE version_id = ?1",
                params![version_id],
                |row| {
                    Ok(DesignAssetVersionRecord {
                        schema_version: "DesignAssetVersion@1".to_owned(),
                        version_id: row.get(0)?,
                        project_id: row.get(1)?,
                        parent_version_id: row.get(2)?,
                        candidate_id: row.get(3)?,
                        manifest_hash: row.get(4)?,
                        canonical_sha256: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn list_versions(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<DesignAssetVersionRecord>, StoreError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT version_id, project_id, parent_version_id, candidate_id, manifest_hash, canonical_sha256, created_at FROM design_asset_versions WHERE (?1 IS NULL OR project_id = ?1) ORDER BY created_at DESC, version_id ASC",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(DesignAssetVersionRecord {
                schema_version: "DesignAssetVersion@1".to_owned(),
                version_id: row.get(0)?,
                project_id: row.get(1)?,
                parent_version_id: row.get(2)?,
                candidate_id: row.get(3)?,
                manifest_hash: row.get(4)?,
                canonical_sha256: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get_job(&self, job_id: &str) -> Result<Option<JobSummary>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT job_id, project_id, kind, status, progress, error_code, created_at, updated_at FROM runtime_jobs WHERE job_id = ?1",
                params![job_id],
                |row| {
                    let progress: i64 = row.get(4)?;
                    let progress = u8::try_from(progress).map_err(|_| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Integer,
                            "job progress outside u8".into(),
                        )
                    })?;
                    Ok(JobSummary {
                        job_id: row.get(0)?,
                        project_id: row.get(1)?,
                        kind: row.get(2)?,
                        status: row.get(3)?,
                        progress,
                        error_code: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn insert_job_event(&self, event: &JobEventRecord) -> Result<(), StoreError> {
        if !is_opaque_id(&event.job_id) || event.sequence <= 0 {
            return Err(StoreError::InvalidData(
                "invalid job event identity".to_owned(),
            ));
        }
        let payload = serde_json::to_string(&event.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![event.job_id, event.sequence, event.kind, payload, event.created_at],
        )?;
        Ok(())
    }

    pub fn list_job_events(
        &self,
        job_id: &str,
        after_sequence: i64,
    ) -> Result<Vec<JobEventRecord>, StoreError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT sequence, kind, payload_json, created_at FROM runtime_job_events WHERE job_id = ?1 AND sequence > ?2 ORDER BY sequence ASC",
        )?;
        let rows = statement.query_map(params![job_id, after_sequence], |row| {
            let payload_json: String = row.get(2)?;
            let payload = serde_json::from_str(&payload_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(JobEventRecord {
                schema_version: "RuntimeJobEvent@1".to_owned(),
                job_id: job_id.to_owned(),
                sequence: row.get(0)?,
                kind: row.get(1)?,
                payload,
                created_at: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn register_object(&self, object: &CasObjectRecord) -> Result<(), StoreError> {
        self.cas.verify(&object.sha256, object.size_bytes)?;
        let connection = self.lock_connection()?;
        let existing: Option<(i64, String, String)> = connection
            .query_row(
                "SELECT size_bytes, mime, kind FROM objects WHERE sha256 = ?1",
                params![object.sha256],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((size, mime, kind)) = existing {
            if size != i64::try_from(object.size_bytes).unwrap_or(i64::MAX)
                || mime != object.mime
                || kind != object.kind
            {
                return Err(StoreError::InvalidData("CAS metadata mismatch".to_owned()));
            }
            return Ok(());
        }
        connection.execute(
            "INSERT INTO objects (sha256, size_bytes, mime, kind, reachability, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                object.sha256,
                i64::try_from(object.size_bytes).map_err(|_| StoreError::InvalidData("object too large".to_owned()))?,
                object.mime,
                object.kind,
                object.reachability,
                object.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_object(&self, sha256: &str) -> Result<Option<CasObjectRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT sha256, size_bytes, mime, kind, reachability, created_at FROM objects WHERE sha256 = ?1",
                params![sha256],
                |row| {
                    let size_bytes: i64 = row.get(1)?;
                    Ok(CasObjectRecord {
                        schema_version: "CasObject@1".to_owned(),
                        sha256: row.get(0)?,
                        size_bytes: u64::try_from(size_bytes).map_err(|_| {
                            rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Integer,
                                "negative object size".into(),
                            )
                        })?,
                        mime: row.get(2)?,
                        kind: row.get(3)?,
                        reachability: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn put_object(
        &self,
        bytes: &[u8],
        expected_sha256: Option<&str>,
        mime: &str,
        kind: &str,
        created_at: &str,
    ) -> Result<CasObject, StoreError> {
        let object = self
            .cas
            .put(bytes, expected_sha256, mime, kind, created_at)?;
        self.register_object(&object.record)?;
        Ok(object)
    }

    pub fn append_audit(&self, event: &AuditEventRecord) -> Result<(), StoreError> {
        if !is_opaque_id(&event.audit_id) || event.kind.is_empty() {
            return Err(StoreError::InvalidData("invalid audit event".to_owned()));
        }
        let payload = serde_json::to_string(&event.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO audit_events (audit_id, project_id, kind, object_id, request_sha256, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![event.audit_id, event.project_id, event.kind, event.object_id, event.request_sha256, payload, event.created_at],
        )?;
        Ok(())
    }

    pub fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), StoreError> {
        let Some(database_path) = self.database_path.as_ref() else {
            return Err(StoreError::BackupUnavailable);
        };
        let destination = destination.as_ref();
        fs::create_dir_all(destination)?;
        let destination_database = destination.join("runtime.sqlite");
        if destination_database.exists() {
            return Err(StoreError::InvalidData(
                "backup destination already contains a database".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        let temporary_database =
            destination.join(format!("runtime.sqlite.tmp-{}", Uuid::new_v4().simple()));
        fs::copy(database_path, &temporary_database)?;
        fs::File::open(&temporary_database)?.sync_all()?;
        fs::rename(&temporary_database, &destination_database)?;
        self.cas.copy_objects_to(destination.join("cas"))?;
        let digest = sha256_file(&destination_database)?;
        fs::write(
            destination.join("manifest.sha256"),
            format!("{digest}  runtime.sqlite\n"),
        )?;
        let object_manifest = self
            .cas
            .list_objects()?
            .into_iter()
            .filter_map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();
        fs::write(
            destination.join("objects.manifest"),
            object_manifest.join("\n") + if object_manifest.is_empty() { "" } else { "\n" },
        )?;
        Ok(())
    }

    pub fn restore_from(
        backup: impl AsRef<Path>,
        database_path: impl AsRef<Path>,
        cas_root: impl AsRef<Path>,
    ) -> Result<Self, StoreError> {
        let backup = backup.as_ref();
        let database_path = database_path.as_ref();
        if database_path.exists() {
            return Err(StoreError::InvalidData(
                "restore target database exists".to_owned(),
            ));
        }
        if let Some(parent) = database_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let source_database = backup.join("runtime.sqlite");
        if let Ok(manifest) = fs::read_to_string(backup.join("manifest.sha256")) {
            let expected = manifest.split_whitespace().next().unwrap_or_default();
            if expected != sha256_file(&source_database)? {
                return Err(StoreError::InvalidData(
                    "backup database hash mismatch".to_owned(),
                ));
            }
        }
        fs::copy(source_database, database_path)?;
        let cas_root = cas_root.as_ref();
        if cas_root.exists() {
            return Err(StoreError::InvalidData(
                "restore target CAS exists".to_owned(),
            ));
        }
        copy_directory(&backup.join("cas"), cas_root)?;
        let restored = Self::open_with_cas(database_path, cas_root)?;
        let expected_objects = fs::read_to_string(backup.join("objects.manifest"))
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>();
        for object_path in restored.cas.list_objects()? {
            let object_hash = object_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| StoreError::InvalidData("invalid backup object name".to_owned()))?;
            restored.cas.read_verified(object_hash)?;
        }
        let actual_objects = restored
            .cas
            .list_objects()?
            .into_iter()
            .filter_map(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
            })
            .collect::<std::collections::BTreeSet<_>>();
        if expected_objects != actual_objects {
            return Err(StoreError::InvalidData(
                "backup CAS manifest mismatch".to_owned(),
            ));
        }
        Ok(restored)
    }

    pub fn database_path(&self) -> Option<&Path> {
        self.database_path.as_deref()
    }

    fn lock_connection(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.connection.lock().map_err(|_| StoreError::LockPoisoned)
    }
}

fn configure_connection(connection: &mut Connection) -> Result<(), StoreError> {
    connection.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA synchronous = FULL;",
    )?;
    let _ = connection.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()));
    Ok(())
}

fn reject_legacy_database(path: &Path) -> Result<(), StoreError> {
    if !path.exists() || fs::metadata(path)?.len() == 0 {
        return Ok(());
    }
    let connection = Connection::open(path)?;
    let has_runtime_meta = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_meta' LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some();
    if !has_runtime_meta {
        return Err(StoreError::LegacyDatabaseRejected);
    }
    Ok(())
}

fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(MIGRATION_SQL)?;
    let version: String = transaction.query_row(
        "SELECT value FROM schema_meta WHERE key = 'runtime_schema_version'",
        [],
        |row| row.get(0),
    )?;
    if version != RUNTIME_SCHEMA_VERSION {
        return Err(StoreError::MigrationVersionUnsupported);
    }
    transaction.commit()?;
    Ok(())
}

fn validate_owner(owner: &str) -> Result<(), StoreError> {
    if !is_opaque_id(owner) {
        return Err(StoreError::InvalidData("invalid writer owner".to_owned()));
    }
    Ok(())
}

fn validate_project(project: &ProjectRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&project.project_id)
        || project.name.trim().is_empty()
        || project.active_snapshot_revision < 0
        || !is_sha256(&project.canonical_sha256)
    {
        return Err(StoreError::InvalidData("invalid project record".to_owned()));
    }
    Ok(())
}

fn validate_snapshot(snapshot: &SnapshotRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&snapshot.snapshot_id)
        || !is_opaque_id(&snapshot.project_id)
        || snapshot.revision < 0
        || !is_sha256(&snapshot.manifest_hash)
        || !is_sha256(&snapshot.canonical_sha256)
    {
        return Err(StoreError::InvalidData(
            "invalid snapshot record".to_owned(),
        ));
    }
    Ok(())
}

fn validate_candidate(candidate: &CandidateRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&candidate.candidate_id)
        || !is_opaque_id(&candidate.project_id)
        || !is_sha256(&candidate.request_sha256)
        || !is_sha256(&candidate.canonical_sha256)
    {
        return Err(StoreError::InvalidData(
            "invalid candidate record".to_owned(),
        ));
    }
    Ok(())
}

fn validate_version(version: &DesignAssetVersionRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&version.version_id)
        || !is_opaque_id(&version.project_id)
        || !is_opaque_id(&version.candidate_id)
        || !is_sha256(&version.manifest_hash)
        || !is_sha256(&version.canonical_sha256)
    {
        return Err(StoreError::InvalidData(
            "invalid asset version record".to_owned(),
        ));
    }
    Ok(())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_millis() as i64
}

fn sha256_file(path: &Path) -> Result<String, StoreError> {
    let bytes = fs::read(path)?;
    Ok(sha256_hex(&bytes))
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.is_dir() {
        return Err(StoreError::InvalidData(
            "backup CAS is not a directory".to_owned(),
        ));
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(StoreError::InvalidData("symlink in backup".to_owned()));
        }
        if file_type.is_dir() {
            copy_directory(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn test_root(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("forgecad-store-test-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test root");
        root
    }

    fn project(id: &str) -> ProjectRecord {
        ProjectRecord {
            schema_version: "Project@1".to_owned(),
            project_id: id.to_owned(),
            name: "Test project".to_owned(),
            policy: serde_json::json!({"scope":"test"}),
            created_at: "1".to_owned(),
            updated_at: "1".to_owned(),
            active_snapshot_revision: 0,
            head_snapshot_id: None,
            canonical_sha256: "a".repeat(64),
        }
    }

    #[test]
    fn writer_lease_is_single_owner_across_connections() {
        let root = test_root("lease");
        let db = root.join("runtime.sqlite");
        let first = Store::open(&db).expect("first store");
        let second = Store::open(&db).expect("second store");
        assert!(first.acquire_writer_lease("runtime-a").is_ok());
        assert!(matches!(
            second.acquire_writer_lease("runtime-b"),
            Err(StoreError::WriterLeaseHeld)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stale_lease_can_be_recovered_after_crash_window() {
        let store = Store::memory().expect("store");
        assert!(store
            .acquire_writer_lease_with_token("runtime-a", "token-a", 100, 10)
            .is_ok());
        assert!(store
            .acquire_writer_lease_with_token("runtime-b", "token-b", 111, 10)
            .is_ok());
        let _ = store.release_writer_lease("runtime-b", "token-b");
    }

    #[test]
    fn migration_and_restart_preserve_runtime_records() {
        let root = test_root("restart");
        let db = root.join("runtime.sqlite");
        {
            let store = Store::open(&db).expect("store");
            store
                .insert_project(&project("project-restart"))
                .expect("insert");
        }
        let reopened = Store::open(&db).expect("reopen");
        assert_eq!(
            reopened
                .get_project("project-restart")
                .expect("read")
                .expect("project")
                .name,
            "Test project"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_database_is_not_opened_or_migrated() {
        let root = test_root("legacy");
        let db = root.join("legacy.sqlite");
        let connection = Connection::open(&db).expect("legacy db");
        connection
            .execute_batch(
                "CREATE TABLE projects (project_id TEXT PRIMARY KEY, name TEXT NOT NULL);",
            )
            .expect("legacy table");
        drop(connection);
        assert!(matches!(
            Store::open(&db),
            Err(StoreError::LegacyDatabaseRejected)
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn transaction_rolls_back_on_constraint_failure() {
        let store = Store::memory().expect("store");
        let mut connection = store.lock_connection().expect("lock");
        let transaction = connection.transaction().expect("transaction");
        transaction
            .execute(
                "INSERT INTO projects (project_id, name, policy_json, created_at, updated_at, active_snapshot_revision, canonical_sha256) VALUES ('p', 'P', '{}', '1', '1', 0, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa')",
                [],
            )
            .expect("first insert");
        assert!(transaction
            .execute(
                "INSERT INTO projects (project_id, name, policy_json, created_at, updated_at, active_snapshot_revision, canonical_sha256) VALUES ('p', 'P2', '{}', '1', '1', 0, 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb')",
                [],
            )
            .is_err());
        drop(transaction);
        drop(connection);
        assert!(store.list_projects().expect("list").is_empty());
    }

    #[test]
    fn backup_and_restore_preserve_db_and_cas() {
        let root = test_root("backup");
        let db = root.join("runtime.sqlite");
        let store = Store::open(&db).expect("store");
        store
            .insert_project(&project("project-backup"))
            .expect("insert");
        let object = store
            .put_object(
                b"backup-object",
                None,
                "application/octet-stream",
                "fixture",
                "1",
            )
            .expect("object");
        let backup = root.join("backup");
        store.backup_to(&backup).expect("backup");
        let restored_db = root.join("restored.sqlite");
        let restored_cas = root.join("restored.cas");
        let restored = Store::restore_from(&backup, &restored_db, &restored_cas).expect("restore");
        assert!(restored
            .get_project("project-backup")
            .expect("read")
            .is_some());
        assert_eq!(
            restored
                .cas()
                .read_verified(&object.record.sha256)
                .expect("cas"),
            b"backup-object"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_readers_do_not_create_second_writer() {
        let root = test_root("concurrent");
        let db = root.join("runtime.sqlite");
        let first = Store::open(&db).expect("first");
        first.acquire_writer_lease("owner-a").expect("lease");
        let db_for_thread = db.clone();
        let handle = thread::spawn(move || {
            Store::open(db_for_thread)
                .expect("second")
                .acquire_writer_lease("owner-b")
        });
        assert!(matches!(
            handle.join().expect("thread"),
            Err(StoreError::WriterLeaseHeld)
        ));
        let _ = fs::remove_dir_all(root);
    }
}
