mod cas;

pub use cas::{CasError, CasObject, CasStore};
use forgecad_contracts::{
    is_opaque_id, is_sha256, ApprovalReceiptRecord, AuditEventRecord, CandidateConfirmRequest,
    CandidateConfirmResult, CandidateRecord, CandidateRejectRequest, CandidateRejectResult,
    CasObjectRecord, DesignAssetVersionRecord, ExportConfirmRequest, ExportConfirmResult,
    ExportManifestRecord, ExportPrepareRequest, ExportPrepareResult,
    GeometryCandidateEvidenceRecord, JobEventRecord, JobRecord, JobSummary, ProjectRecord,
    ProjectSummary, ReferenceAuthorization, ReferenceEvidenceRecord, RestoreConfirmRequest,
    RestoreConfirmResult, RestorePrepareRequest, RestorePrepareResult, SnapshotRecord,
    SnapshotSummary,
};
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use uuid::Uuid;

const MIGRATION_SQL: &str =
    include_str!("../../../../../../migrations-runtime-v1/0001_runtime.sql");
const RUNTIME_SCHEMA_VERSION: &str = "1";
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("CAS error: {0}")]
    Cas(#[from] CasError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid runtime data: {0}")]
    InvalidData(String),
    #[error("{code}: {message}")]
    Contract { code: String, message: String },
    #[error("database backup is unavailable for an in-memory store")]
    BackupUnavailable,
    #[error("database migration version is unsupported")]
    MigrationVersionUnsupported,
    #[error("legacy database is not a ForgeCAD Runtime V1 database")]
    LegacyDatabaseRejected,
    #[error("store mutex is poisoned")]
    LockPoisoned,
}

#[derive(Clone)]
pub struct Store {
    connection: Arc<Mutex<Connection>>,
    cas: CasStore,
    database_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualEvidenceRecord {
    pub candidate_id: String,
    pub project_id: String,
    pub reference_id: String,
    pub target_sha256: Option<String>,
    pub render_set_object_sha256: String,
    pub comparison_report_object_sha256: Option<String>,
    pub visual_review_object_sha256: Option<String>,
    pub quality_report_object_sha256: String,
    pub human_receipt_object_sha256: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// One candidate-bound visual comparison for an authored ReferenceCanvas
/// view.  The legacy `visual_evidence` row remains the single-view compatibility
/// projection; this table is the durable view-keyed source for multi-view
/// evidence and prevents the last rendered view from overwriting the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualEvidenceViewRecord {
    pub candidate_id: String,
    pub project_id: String,
    pub view_id: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub render_set_object_sha256: String,
    pub comparison_report_object_sha256: Option<String>,
    pub quality_report_object_sha256: String,
    pub quality_status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Index row for a canonical `CrossViewEvidenceBundle@1` CAS object.  The
/// bundle is immutable; the row only makes the latest candidate/session
/// binding queryable without moving JSON truth into SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossViewEvidenceRecord {
    pub bundle_object_sha256: String,
    pub candidate_id: String,
    pub project_id: String,
    pub session_id: String,
    pub reference_canvas_sha256: String,
    pub aggregate_status: String,
    pub hard_gate_passed: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Runtime-owned durable state for the Agentic Design Runtime.  The full
/// record is also stored as a canonical JSON object in CAS; these fields are
/// repeated in SQLite so binding checks and project-scoped reads stay inside
/// the Store transaction boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgenticSessionRecord {
    pub schema_version: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub revision: i64,
    pub candidate_state_sha256: String,
    pub design_spec_id: String,
    pub design_spec_sha256: String,
    pub reference_canvas_id: String,
    pub reference_canvas_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub evidence_sha256: String,
    pub observation_sha256: String,
    pub current_version_id: Option<String>,
    pub current_version_sha256: Option<String>,
    pub current_stage: String,
    pub quality_status: String,
    pub status: String,
    pub stage_gate: Value,
    pub next_actions: Vec<Value>,
    pub rollback: Value,
    pub current_checkpoint_id: Option<String>,
    pub current_checkpoint_sha256: Option<String>,
    pub checkpoint_ids: Vec<String>,
    pub lineage: Value,
    /// SQLite/CAS object containing the canonical record.  It is omitted from
    /// the CAS payload itself to avoid a content-hash cycle.
    pub object_sha256: Option<String>,
    pub canonical_sha256: String,
    pub created_at: String,
    pub updated_at: String,
}

/// An immutable checkpoint snapshot.  The explicit uncertainty/action lists
/// are intentionally part of the durable record rather than derived by the
/// Viewer, so a later readback cannot silently change what was known or
/// unlocked at the time of the checkpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgenticCheckpointRecord {
    pub schema_version: String,
    pub checkpoint_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub revision: i64,
    pub stage: String,
    pub checkpoint_type: String,
    pub candidate_state_sha256: String,
    pub artifact_sha256: String,
    pub reference_id: String,
    pub reference_sha256: String,
    pub camera_hash: String,
    pub input_sha256: String,
    pub evidence_sha256: String,
    pub observation_sha256: String,
    pub version_id: Option<String>,
    pub version_sha256: Option<String>,
    pub parent_checkpoint_id: Option<String>,
    pub parent_checkpoint_sha256: Option<String>,
    pub stage_gate: Value,
    pub rollback: Value,
    pub observed: Vec<String>,
    pub inferred: Vec<String>,
    pub unknown: Vec<String>,
    pub failed_gates: Vec<String>,
    pub allowed_actions: Vec<String>,
    pub locked_actions: Vec<String>,
    pub immutable: bool,
    pub runtime_write: bool,
    pub object_sha256: Option<String>,
    pub canonical_sha256: String,
    pub created_at: String,
}

/// An immutable Runtime-owned record for one bounded Agentic action run.
/// Stage results are metadata-only and must point at CAS objects by hash; the
/// Store never executes or interprets an action from this record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgenticActionRunRecord {
    pub schema_version: String,
    pub run_id: String,
    pub session_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub input_sha256: String,
    pub status: String,
    pub completed_stage: String,
    pub stage_results: Value,
    pub checkpoint_id: Option<String>,
    pub checkpoint_sha256: Option<String>,
    pub immutable: bool,
    /// SQLite/CAS object containing the canonical record.  It is omitted from
    /// the CAS payload itself to avoid a content-hash cycle.
    pub object_sha256: Option<String>,
    pub canonical_sha256: String,
    pub created_at: String,
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

    pub fn insert_candidate(&self, candidate: &CandidateRecord) -> Result<(), StoreError> {
        validate_candidate(candidate)?;
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO candidates (candidate_id, project_id, base_version_id, source_version_id, prepared_object_id, prepared_object_sha256, state, request_sha256, manifest_hash, quality_report_id, quality_hard_gate_passed, canonical_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                candidate.candidate_id,
                candidate.project_id,
                candidate.base_version_id,
                candidate.source_version_id,
                candidate.prepared_object_id,
                candidate.prepared_object_sha256,
                candidate.state,
                candidate.request_sha256,
                candidate.manifest_hash,
                candidate.quality_report_id,
                candidate.quality_hard_gate_passed,
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
                "SELECT candidate_id, project_id, base_version_id, source_version_id, prepared_object_id, prepared_object_sha256, state, request_sha256, manifest_hash, quality_report_id, quality_hard_gate_passed, canonical_sha256, error_code, created_at, updated_at FROM candidates WHERE candidate_id = ?1",
                params![candidate_id],
                |row| {
                    Ok(CandidateRecord {
                        schema_version: "Candidate@1".to_owned(),
                        candidate_id: row.get(0)?,
                        project_id: row.get(1)?,
                        base_version_id: row.get(2)?,
                        source_version_id: row.get(3)?,
                        prepared_object_id: row.get(4)?,
                        prepared_object_sha256: row.get(5)?,
                        state: row.get(6)?,
                        request_sha256: row.get(7)?,
                        manifest_hash: row.get(8)?,
                        quality_report_id: row.get(9)?,
                        quality_hard_gate_passed: row.get::<_, i64>(10)? != 0,
                        canonical_sha256: row.get(11)?,
                        error_code: row.get(12)?,
                        created_at: row.get(13)?,
                        updated_at: row.get(14)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn list_candidates(&self, project_id: &str) -> Result<Vec<CandidateRecord>, StoreError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT candidate_id, project_id, base_version_id, source_version_id, prepared_object_id, prepared_object_sha256, state, request_sha256, manifest_hash, quality_report_id, quality_hard_gate_passed, canonical_sha256, error_code, created_at, updated_at FROM candidates WHERE project_id = ?1 ORDER BY created_at DESC, candidate_id ASC",
        )?;
        let rows = statement.query_map(params![project_id], |row| {
            Ok(CandidateRecord {
                schema_version: "Candidate@1".to_owned(),
                candidate_id: row.get(0)?,
                project_id: row.get(1)?,
                base_version_id: row.get(2)?,
                source_version_id: row.get(3)?,
                prepared_object_id: row.get(4)?,
                prepared_object_sha256: row.get(5)?,
                state: row.get(6)?,
                request_sha256: row.get(7)?,
                manifest_hash: row.get(8)?,
                quality_report_id: row.get(9)?,
                quality_hard_gate_passed: row.get::<_, i64>(10)? != 0,
                canonical_sha256: row.get(11)?,
                error_code: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn insert_candidate_and_job(
        &self,
        candidate: &CandidateRecord,
        job: &JobRecord,
        event: &JobEventRecord,
        audit: &AuditEventRecord,
    ) -> Result<(), StoreError> {
        validate_candidate(candidate)?;
        validate_job(job)?;
        validate_job_event(event)?;
        validate_audit(audit)?;
        if job.project_id != candidate.project_id || event.job_id != job.job_id {
            return Err(StoreError::InvalidData(
                "candidate/job transaction scope mismatch".to_owned(),
            ));
        }
        let payload = serde_json::to_string(&event.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let audit_payload = serde_json::to_string(&audit.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO candidates (candidate_id, project_id, base_version_id, source_version_id, prepared_object_id, prepared_object_sha256, state, request_sha256, manifest_hash, quality_report_id, quality_hard_gate_passed, canonical_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                candidate.candidate_id,
                candidate.project_id,
                candidate.base_version_id,
                candidate.source_version_id,
                candidate.prepared_object_id,
                candidate.prepared_object_sha256,
                candidate.state,
                candidate.request_sha256,
                candidate.manifest_hash,
                candidate.quality_report_id,
                candidate.quality_hard_gate_passed,
                candidate.canonical_sha256,
                candidate.error_code,
                candidate.created_at,
                candidate.updated_at,
            ],
        )?;
        transaction.execute(
            "INSERT INTO runtime_jobs (job_id, project_id, kind, status, progress, request_sha256, checkpoint_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                job.job_id,
                job.project_id,
                job.kind,
                job.status,
                i64::from(job.progress),
                job.request_sha256,
                job.checkpoint_sha256,
                job.error_code,
                job.created_at,
                job.updated_at,
            ],
        )?;
        transaction.execute(
            "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![event.job_id, event.sequence, event.kind, payload, event.created_at],
        )?;
        transaction.execute(
            "INSERT INTO audit_events (audit_id, project_id, kind, object_id, request_sha256, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                audit.audit_id,
                audit.project_id,
                audit.kind,
                audit.object_id,
                audit.request_sha256,
                audit_payload,
                audit.created_at,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_candidate_quality(
        &self,
        candidate_id: &str,
        quality_report_id: &str,
        hard_gate_passed: bool,
        updated_at: &str,
    ) -> Result<CandidateRecord, StoreError> {
        if !is_opaque_id(candidate_id) || !is_opaque_id(quality_report_id) {
            return Err(StoreError::InvalidData(
                "invalid quality result identity".to_owned(),
            ));
        }
        let state = if hard_gate_passed {
            "reviewable"
        } else {
            "failed"
        };
        let error_code = if hard_gate_passed {
            None
        } else {
            Some("QUALITY_HARD_GATE_FAILED")
        };
        let connection = self.lock_connection()?;
        let updated = connection.execute(
            "UPDATE candidates SET state = ?1, quality_report_id = ?2, quality_hard_gate_passed = ?3, error_code = ?4, updated_at = ?5 WHERE candidate_id = ?6 AND state IN ('prepared', 'compiling', 'evaluating', 'reviewable')",
            params![state, quality_report_id, hard_gate_passed, error_code, updated_at, candidate_id],
        )?;
        if updated == 0 {
            return Err(StoreError::Contract {
                code: "CANDIDATE_STATE_INVALID".to_owned(),
                message: "candidate is not awaiting quality evaluation".to_owned(),
            });
        }
        drop(connection);
        self.get_candidate(candidate_id)?.ok_or_else(|| {
            StoreError::InvalidData("candidate disappeared after quality update".to_owned())
        })
    }

    /// Persist the V2 geometry provenance before exposing a candidate as
    /// reviewable. This is deliberately one transaction: a candidate may not
    /// become passing without the exact program/readback/quality evidence that
    /// confirmation re-reads later.
    pub fn record_geometry_candidate_evidence_and_mark_quality(
        &self,
        evidence: &GeometryCandidateEvidenceRecord,
        hard_gate_passed: bool,
        updated_at: &str,
    ) -> Result<CandidateRecord, StoreError> {
        validate_geometry_candidate_evidence(evidence)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let candidate = read_candidate_for_transaction(&transaction, &evidence.candidate_id)?
            .ok_or_else(|| StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "candidate not found for geometry evidence".to_owned(),
            })?;
        if candidate.project_id != evidence.project_id
            || candidate.prepared_object_sha256.as_deref()
                != Some(evidence.artifact_object_sha256.as_str())
        {
            return Err(StoreError::Contract {
                code: "GEOMETRY_EVIDENCE_CANDIDATE_MISMATCH".to_owned(),
                message: "geometry evidence does not bind this candidate artifact and project"
                    .to_owned(),
            });
        }
        if let Some(reference_id) = evidence.reference_id.as_deref() {
            let reference = transaction
                .query_row(
                    "SELECT project_id, object_sha256 FROM reference_evidence WHERE reference_id = ?1",
                    params![reference_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or_else(|| StoreError::Contract {
                    code: "REFERENCE_SCOPE_DENIED".to_owned(),
                    message: "geometry evidence reference is unavailable".to_owned(),
                })?;
            if reference.0 != evidence.project_id
                || evidence.reference_sha256.as_deref() != Some(reference.1.as_str())
            {
                return Err(StoreError::Contract {
                    code: "REFERENCE_SCOPE_DENIED".to_owned(),
                    message: "geometry evidence reference is outside the candidate project"
                        .to_owned(),
                });
            }
        }
        for hash in [
            &evidence.geometry_program_object_sha256,
            &evidence.artifact_object_sha256,
            &evidence.artifact_readback_object_sha256,
            &evidence.quality_report_object_sha256,
        ] {
            let exists = transaction
                .query_row(
                    "SELECT 1 FROM objects WHERE sha256 = ?1",
                    params![hash],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !exists {
                return Err(StoreError::Contract {
                    code: "GEOMETRY_EVIDENCE_OBJECT_UNAVAILABLE".to_owned(),
                    message: "geometry evidence references an unavailable CAS object".to_owned(),
                });
            }
        }
        transaction.execute(
            "INSERT INTO geometry_candidate_evidence (candidate_id, project_id, reference_id, reference_sha256, geometry_program_sha256, geometry_program_object_sha256, operator_catalog_sha256, readback_config_sha256, artifact_object_sha256, artifact_readback_object_sha256, quality_report_object_sha256, quality_report_id, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &evidence.candidate_id,
                &evidence.project_id,
                &evidence.reference_id,
                &evidence.reference_sha256,
                &evidence.geometry_program_sha256,
                &evidence.geometry_program_object_sha256,
                &evidence.operator_catalog_sha256,
                &evidence.readback_config_sha256,
                &evidence.artifact_object_sha256,
                &evidence.artifact_readback_object_sha256,
                &evidence.quality_report_object_sha256,
                &evidence.quality_report_id,
                &evidence.canonical_sha256,
                &evidence.created_at,
            ],
        )?;
        let state = if hard_gate_passed {
            "reviewable"
        } else {
            "failed"
        };
        let error_code = if hard_gate_passed {
            None
        } else {
            Some("QUALITY_HARD_GATE_FAILED")
        };
        let updated = transaction.execute(
            "UPDATE candidates SET state = ?1, quality_report_id = ?2, quality_hard_gate_passed = ?3, error_code = ?4, updated_at = ?5 WHERE candidate_id = ?6 AND state IN ('prepared', 'compiling', 'evaluating')",
            params![state, &evidence.quality_report_id, hard_gate_passed, error_code, updated_at, &evidence.candidate_id],
        )?;
        if updated != 1 {
            return Err(StoreError::Contract {
                code: "CANDIDATE_STATE_INVALID".to_owned(),
                message: "candidate is not awaiting geometry quality evaluation".to_owned(),
            });
        }
        transaction.commit()?;
        drop(connection);
        self.get_candidate(&evidence.candidate_id)?.ok_or_else(|| {
            StoreError::InvalidData(
                "candidate disappeared after geometry evidence write".to_owned(),
            )
        })
    }

    pub fn get_geometry_candidate_evidence(
        &self,
        candidate_id: &str,
    ) -> Result<Option<GeometryCandidateEvidenceRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT candidate_id, project_id, reference_id, reference_sha256, geometry_program_sha256, geometry_program_object_sha256, operator_catalog_sha256, readback_config_sha256, artifact_object_sha256, artifact_readback_object_sha256, quality_report_object_sha256, quality_report_id, canonical_sha256, created_at FROM geometry_candidate_evidence WHERE candidate_id = ?1",
                params![candidate_id],
                |row| {
                    Ok(GeometryCandidateEvidenceRecord {
                        schema_version: "GeometryCandidateEvidence@1".to_owned(),
                        candidate_id: row.get(0)?,
                        project_id: row.get(1)?,
                        reference_id: row.get(2)?,
                        reference_sha256: row.get(3)?,
                        geometry_program_sha256: row.get(4)?,
                        geometry_program_object_sha256: row.get(5)?,
                        operator_catalog_sha256: row.get(6)?,
                        readback_config_sha256: row.get(7)?,
                        artifact_object_sha256: row.get(8)?,
                        artifact_readback_object_sha256: row.get(9)?,
                        quality_report_object_sha256: row.get(10)?,
                        quality_report_id: row.get(11)?,
                        canonical_sha256: row.get(12)?,
                        created_at: row.get(13)?,
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

    pub fn latest_version_for_project(
        &self,
        project_id: &str,
    ) -> Result<Option<DesignAssetVersionRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT v.version_id, v.project_id, v.parent_version_id, v.candidate_id, v.manifest_hash, v.canonical_sha256, v.created_at FROM projects p JOIN snapshots s ON s.snapshot_id = p.head_snapshot_id JOIN design_asset_versions v ON v.candidate_id = s.candidate_id WHERE p.project_id = ?1 LIMIT 1",
                params![project_id],
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

    pub fn get_job_record(&self, job_id: &str) -> Result<Option<JobRecord>, StoreError> {
        if !is_opaque_id(job_id) {
            return Err(StoreError::InvalidData("invalid job id".to_owned()));
        }
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT job_id, project_id, kind, status, progress, request_sha256, checkpoint_sha256, error_code, created_at, updated_at FROM runtime_jobs WHERE job_id = ?1",
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
                    Ok(JobRecord {
                        schema_version: "RuntimeJob@1".to_owned(),
                        job_id: row.get(0)?,
                        project_id: row.get(1)?,
                        kind: row.get(2)?,
                        status: row.get(3)?,
                        progress,
                        request_sha256: row.get(5)?,
                        checkpoint_sha256: row.get(6)?,
                        error_code: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                    })
                },
            )
            .optional()?)
    }

    /// Atomically create one durable Job and its first event.  Optimization
    /// is intentionally built on this existing RuntimeJob table rather than
    /// introducing a second job state machine.  A repeated job id is an
    /// idempotent read only when its project/kind/request binding is exact.
    pub fn insert_job_with_event(
        &self,
        job: &JobRecord,
        event: &JobEventRecord,
    ) -> Result<(), StoreError> {
        self.insert_job_with_event_if_absent(job, event, &[])?;
        Ok(())
    }

    /// Compatibility terminal update for Runtime-owned bounded jobs.  The
    /// newer store path also records reachable CAS hashes; this wrapper keeps
    /// the older Primary Form worker contract on the same durable state table.
    pub fn finish_job_with_event(
        &self,
        job_id: &str,
        status: &str,
        progress: u8,
        error_code: Option<&str>,
        event_kind: &str,
        payload: &Value,
        updated_at: &str,
    ) -> Result<JobSummary, StoreError> {
        let mut job = self
            .get_job_record(job_id)?
            .ok_or_else(|| StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "job not found".to_owned(),
            })?;
        if matches!(job.status.as_str(), "succeeded" | "failed" | "cancelled") {
            return self
                .get_job(job_id)?
                .ok_or_else(|| StoreError::InvalidData("job disappeared".to_owned()));
        }
        job.status = status.to_owned();
        job.progress = progress;
        job.error_code = error_code.map(str::to_owned);
        job.updated_at = updated_at.to_owned();
        self.update_job_with_event(&job, event_kind, payload, &[])?;
        self.get_job(job_id)?
            .ok_or_else(|| StoreError::InvalidData("job disappeared after update".to_owned()))
    }

    pub fn insert_job_with_event_if_absent(
        &self,
        job: &JobRecord,
        event: &JobEventRecord,
        reachable_hashes: &[String],
    ) -> Result<JobRecord, StoreError> {
        validate_job(job)?;
        validate_job_event(event)?;
        if event.job_id != job.job_id || event.sequence != 1 {
            return Err(StoreError::InvalidData(
                "initial job event does not bind to job".to_owned(),
            ));
        }
        validate_reachable_hashes(reachable_hashes)?;
        let payload = serde_json::to_string(&event.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let existing = read_job_record_from_transaction(&transaction, &job.job_id)?;
        if let Some(existing) = existing {
            if existing.project_id != job.project_id
                || existing.kind != job.kind
                || existing.request_sha256 != job.request_sha256
            {
                return Err(StoreError::Contract {
                    code: "JOB_IDEMPOTENCY_CONFLICT".to_owned(),
                    message: "job id is already bound to another optimization request".to_owned(),
                });
            }
            transaction.commit()?;
            return Ok(existing);
        }
        transaction.execute(
            "INSERT INTO runtime_jobs (job_id, project_id, kind, status, progress, request_sha256, checkpoint_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                job.job_id,
                job.project_id,
                job.kind,
                job.status,
                i64::from(job.progress),
                job.request_sha256,
                job.checkpoint_sha256,
                job.error_code,
                job.created_at,
                job.updated_at,
            ],
        )?;
        transaction.execute(
            "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![event.job_id, event.sequence, event.kind, payload, event.created_at],
        )?;
        mark_reachable_in_transaction(&transaction, reachable_hashes)?;
        transaction.commit()?;
        drop(connection);
        self.get_job_record(&job.job_id)?.ok_or_else(|| {
            StoreError::InvalidData("job disappeared after atomic create".to_owned())
        })
    }

    /// Advance a durable Job and append the event/checkpoint in one SQLite
    /// transaction.  Every CAS object referenced by the event becomes
    /// reachable before the new state is visible to a reader.
    pub fn update_job_with_event(
        &self,
        job: &JobRecord,
        event_kind: &str,
        event_payload: &Value,
        reachable_hashes: &[String],
    ) -> Result<JobRecord, StoreError> {
        validate_job(job)?;
        if event_kind.trim().is_empty() || !event_payload.is_object() {
            return Err(StoreError::InvalidData(
                "job event is not a bounded object".to_owned(),
            ));
        }
        validate_reachable_hashes(reachable_hashes)?;
        let payload = serde_json::to_string(event_payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let existing =
            read_job_record_from_transaction(&transaction, &job.job_id)?.ok_or_else(|| {
                StoreError::Contract {
                    code: "NOT_FOUND".to_owned(),
                    message: "job not found".to_owned(),
                }
            })?;
        if existing.project_id != job.project_id
            || existing.kind != job.kind
            || existing.request_sha256 != job.request_sha256
        {
            return Err(StoreError::Contract {
                code: "JOB_BINDING_MISMATCH".to_owned(),
                message: "job update does not bind to the original request".to_owned(),
            });
        }
        if matches!(
            existing.status.as_str(),
            "succeeded" | "failed" | "cancelled"
        ) {
            return Err(StoreError::Contract {
                code: "JOB_TERMINAL".to_owned(),
                message: "job is already terminal".to_owned(),
            });
        }
        let next_sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM runtime_job_events WHERE job_id = ?1",
            params![job.job_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE runtime_jobs SET status = ?1, progress = ?2, checkpoint_sha256 = ?3, error_code = ?4, updated_at = ?5 WHERE job_id = ?6",
            params![
                job.status,
                i64::from(job.progress),
                job.checkpoint_sha256,
                job.error_code,
                job.updated_at,
                job.job_id,
            ],
        )?;
        transaction.execute(
            "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![job.job_id, next_sequence, event_kind, payload, job.updated_at],
        )?;
        if let Some(checkpoint_sha256) = job.checkpoint_sha256.as_deref() {
            transaction.execute(
                "INSERT OR REPLACE INTO runtime_job_checkpoints (job_id, sequence, checkpoint_sha256, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![job.job_id, next_sequence, checkpoint_sha256, job.updated_at],
            )?;
        }
        mark_reachable_in_transaction(&transaction, reachable_hashes)?;
        transaction.commit()?;
        drop(connection);
        self.get_job_record(&job.job_id)?.ok_or_else(|| {
            StoreError::InvalidData("job disappeared after atomic update".to_owned())
        })
    }

    /// Claim a queued job exactly once.  This closes the small race between
    /// two identical MCP reads both deciding to start the same detached
    /// worker thread.
    pub fn claim_job_running(
        &self,
        job_id: &str,
        updated_at: &str,
        payload: &Value,
    ) -> Result<Option<JobRecord>, StoreError> {
        if !is_opaque_id(job_id) || updated_at.is_empty() || !payload.is_object() {
            return Err(StoreError::InvalidData(
                "invalid job claim envelope".to_owned(),
            ));
        }
        let payload = serde_json::to_string(payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let current = read_job_record_from_transaction(&transaction, job_id)?;
        let Some(current) = current else {
            return Err(StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "job not found".to_owned(),
            });
        };
        if current.status != "queued" {
            transaction.commit()?;
            return Ok(None);
        }
        let updated = transaction.execute(
            "UPDATE runtime_jobs SET status = 'running', error_code = NULL, updated_at = ?1 WHERE job_id = ?2 AND status = 'queued'",
            params![updated_at, job_id],
        )?;
        if updated != 1 {
            transaction.commit()?;
            return Ok(None);
        }
        let sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM runtime_job_events WHERE job_id = ?1",
            params![job_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, 'optimization_started', ?3, ?4)",
            params![job_id, sequence, payload, updated_at],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_job_record(job_id)
    }

    /// Requeue a non-terminal optimization after a user-approved recovery
    /// request.  The previous CAS checkpoint remains immutable/reachable; a
    /// resumed run starts from the same intent and records a new event.
    pub fn requeue_job(
        &self,
        job_id: &str,
        updated_at: &str,
        payload: &Value,
    ) -> Result<JobRecord, StoreError> {
        if !is_opaque_id(job_id) || updated_at.is_empty() || !payload.is_object() {
            return Err(StoreError::InvalidData(
                "invalid job recovery envelope".to_owned(),
            ));
        }
        let payload = serde_json::to_string(payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let current = read_job_record_from_transaction(&transaction, job_id)?.ok_or_else(|| {
            StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "job not found".to_owned(),
            }
        })?;
        if !matches!(current.status.as_str(), "running" | "failed" | "cancelled") {
            return Err(StoreError::Contract {
                code: "JOB_NOT_RECOVERABLE".to_owned(),
                message: "only running, failed or cancelled jobs can be recovered".to_owned(),
            });
        }
        let next_sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM runtime_job_events WHERE job_id = ?1",
            params![job_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "UPDATE runtime_jobs SET status = 'queued', error_code = NULL, updated_at = ?1 WHERE job_id = ?2",
            params![updated_at, job_id],
        )?;
        transaction.execute(
            "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, 'optimization_requeued', ?3, ?4)",
            params![job_id, next_sequence, payload, updated_at],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_job_record(job_id)?
            .ok_or_else(|| StoreError::InvalidData("job disappeared after recovery".to_owned()))
    }

    pub fn insert_job(&self, job: &JobRecord) -> Result<(), StoreError> {
        validate_job(job)?;
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO runtime_jobs (job_id, project_id, kind, status, progress, request_sha256, checkpoint_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                job.job_id,
                job.project_id,
                job.kind,
                job.status,
                i64::from(job.progress),
                job.request_sha256,
                job.checkpoint_sha256,
                job.error_code,
                job.created_at,
                job.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn cancel_job(&self, job_id: &str, updated_at: &str) -> Result<JobSummary, StoreError> {
        if !is_opaque_id(job_id) {
            return Err(StoreError::InvalidData("invalid job id".to_owned()));
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let status: Option<String> = transaction
            .query_row(
                "SELECT status FROM runtime_jobs WHERE job_id = ?1",
                params![job_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(status) = status else {
            return Err(StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "job not found".to_owned(),
            });
        };
        if matches!(status.as_str(), "succeeded" | "failed" | "cancelled") {
            return Err(StoreError::Contract {
                code: "JOB_NOT_CANCELLABLE".to_owned(),
                message: "job is already terminal".to_owned(),
            });
        }
        transaction.execute(
            "UPDATE runtime_jobs SET status = 'cancelled', progress = 0, error_code = 'JOB_CANCELLED', updated_at = ?1 WHERE job_id = ?2",
            params![updated_at, job_id],
        )?;
        let next_sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM runtime_job_events WHERE job_id = ?1",
            params![job_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, 'cancelled', '{}', ?3)",
            params![job_id, next_sequence, updated_at],
        )?;
        let job = read_job_summary(&transaction, job_id)?;
        transaction.commit()?;
        Ok(job)
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

    pub fn insert_reference_evidence(
        &self,
        reference: &ReferenceEvidenceRecord,
    ) -> Result<(), StoreError> {
        validate_reference_evidence(reference)?;
        let authorization_json = serde_json::to_string(&reference.authorization)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let project_exists: Option<String> = transaction
            .query_row(
                "SELECT project_id FROM projects WHERE project_id = ?1",
                params![reference.project_id],
                |row| row.get(0),
            )
            .optional()?;
        if project_exists.is_none() {
            return Err(StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "project does not exist".to_owned(),
            });
        }
        let object_exists: Option<String> = transaction
            .query_row(
                "SELECT sha256 FROM objects WHERE sha256 = ?1",
                params![reference.object_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if object_exists.is_none() {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "reference CAS object is unavailable".to_owned(),
            });
        }
        if let Some(derived) = reference.derived_object_sha256.as_deref() {
            let derived_exists: Option<String> = transaction
                .query_row(
                    "SELECT sha256 FROM objects WHERE sha256 = ?1",
                    params![derived],
                    |row| row.get(0),
                )
                .optional()?;
            if derived_exists.is_none() {
                return Err(StoreError::Contract {
                    code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                    message: "reference derived CAS object is unavailable".to_owned(),
                });
            }
        }
        transaction.execute(
            "INSERT INTO reference_evidence (reference_id, project_id, object_sha256, mime, size_bytes, width, height, frame_count, import_mode, authorization_json, derived_object_sha256, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                reference.reference_id,
                reference.project_id,
                reference.object_sha256,
                reference.mime,
                i64::try_from(reference.size_bytes)
                    .map_err(|_| StoreError::InvalidData("reference is too large".to_owned()))?,
                i64::from(reference.width),
                i64::from(reference.height),
                i64::from(reference.frame_count),
                reference.import_mode,
                authorization_json,
                reference.derived_object_sha256,
                reference.canonical_sha256,
                reference.created_at,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_reference_evidence(
        &self,
        reference_id: &str,
    ) -> Result<Option<ReferenceEvidenceRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT reference_id, project_id, object_sha256, mime, size_bytes, width, height, frame_count, import_mode, authorization_json, derived_object_sha256, canonical_sha256, created_at FROM reference_evidence WHERE reference_id = ?1",
                params![reference_id],
                |row| read_reference_evidence(row),
            )
            .optional()?)
    }

    pub fn list_reference_evidence(
        &self,
        project_id: &str,
    ) -> Result<Vec<ReferenceEvidenceRecord>, StoreError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT reference_id, project_id, object_sha256, mime, size_bytes, width, height, frame_count, import_mode, authorization_json, derived_object_sha256, canonical_sha256, created_at FROM reference_evidence WHERE project_id = ?1 ORDER BY created_at DESC, reference_id ASC",
        )?;
        let rows = statement.query_map(params![project_id], read_reference_evidence)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
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

    pub fn upsert_visual_evidence(
        &self,
        evidence: &VisualEvidenceRecord,
    ) -> Result<(), StoreError> {
        if !is_opaque_id(&evidence.candidate_id)
            || !is_opaque_id(&evidence.project_id)
            || !is_opaque_id(&evidence.reference_id)
            || evidence
                .target_sha256
                .as_deref()
                .is_some_and(|value| !is_sha256(value))
            || !is_sha256(&evidence.render_set_object_sha256)
            || !is_sha256(&evidence.quality_report_object_sha256)
            || evidence
                .comparison_report_object_sha256
                .as_deref()
                .is_some_and(|value| !is_sha256(value))
            || evidence
                .visual_review_object_sha256
                .as_deref()
                .is_some_and(|value| !is_sha256(value))
            || evidence
                .human_receipt_object_sha256
                .as_deref()
                .is_some_and(|value| !is_sha256(value))
            || evidence.created_at.is_empty()
            || evidence.updated_at.is_empty()
        {
            return Err(StoreError::InvalidData(
                "visual evidence identity or hash is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO visual_evidence (candidate_id, project_id, reference_id, target_sha256, render_set_object_sha256, comparison_report_object_sha256, visual_review_object_sha256, quality_report_object_sha256, human_receipt_object_sha256, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) ON CONFLICT(candidate_id) DO UPDATE SET project_id=excluded.project_id, reference_id=excluded.reference_id, target_sha256=excluded.target_sha256, render_set_object_sha256=excluded.render_set_object_sha256, comparison_report_object_sha256=excluded.comparison_report_object_sha256, visual_review_object_sha256=excluded.visual_review_object_sha256, quality_report_object_sha256=excluded.quality_report_object_sha256, human_receipt_object_sha256=excluded.human_receipt_object_sha256, updated_at=excluded.updated_at",
            params![
                evidence.candidate_id,
                evidence.project_id,
                evidence.reference_id,
                evidence.target_sha256,
                evidence.render_set_object_sha256,
                evidence.comparison_report_object_sha256,
                evidence.visual_review_object_sha256,
                evidence.quality_report_object_sha256,
                evidence.human_receipt_object_sha256,
                evidence.created_at,
                evidence.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_visual_evidence(
        &self,
        candidate_id: &str,
    ) -> Result<Option<VisualEvidenceRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT candidate_id, project_id, reference_id, target_sha256, render_set_object_sha256, comparison_report_object_sha256, visual_review_object_sha256, quality_report_object_sha256, human_receipt_object_sha256, created_at, updated_at FROM visual_evidence WHERE candidate_id = ?1",
                params![candidate_id],
                |row| {
                    Ok(VisualEvidenceRecord {
                        candidate_id: row.get(0)?,
                        project_id: row.get(1)?,
                        reference_id: row.get(2)?,
                        target_sha256: row.get(3)?,
                        render_set_object_sha256: row.get(4)?,
                        comparison_report_object_sha256: row.get(5)?,
                        visual_review_object_sha256: row.get(6)?,
                        quality_report_object_sha256: row.get(7)?,
                        human_receipt_object_sha256: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn upsert_visual_evidence_view(
        &self,
        evidence: &VisualEvidenceViewRecord,
    ) -> Result<(), StoreError> {
        if !is_opaque_id(&evidence.candidate_id)
            || !is_opaque_id(&evidence.project_id)
            || !is_opaque_id(&evidence.view_id)
            || !is_opaque_id(&evidence.reference_id)
            || !is_sha256(&evidence.reference_sha256)
            || !is_sha256(&evidence.camera_hash)
            || !is_sha256(&evidence.render_set_object_sha256)
            || !is_sha256(&evidence.quality_report_object_sha256)
            || evidence
                .comparison_report_object_sha256
                .as_deref()
                .is_some_and(|value| !is_sha256(value))
            || !matches!(
                evidence.quality_status.as_str(),
                "PARTIAL_VISIBLE_VIEW_PASS"
                    | "QUALITY_TARGET_NOT_MET"
                    | "BLOCKED_REFERENCE_COVERAGE"
                    | "not-run"
            )
            || evidence.created_at.is_empty()
            || evidence.updated_at.is_empty()
        {
            return Err(StoreError::InvalidData(
                "visual evidence view identity or hash is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO visual_evidence_views (candidate_id, project_id, view_id, reference_id, reference_sha256, camera_hash, render_set_object_sha256, comparison_report_object_sha256, quality_report_object_sha256, quality_status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) ON CONFLICT(candidate_id, view_id) DO UPDATE SET project_id=excluded.project_id, reference_id=excluded.reference_id, reference_sha256=excluded.reference_sha256, camera_hash=excluded.camera_hash, render_set_object_sha256=excluded.render_set_object_sha256, comparison_report_object_sha256=excluded.comparison_report_object_sha256, quality_report_object_sha256=excluded.quality_report_object_sha256, quality_status=excluded.quality_status, updated_at=excluded.updated_at",
            params![
                evidence.candidate_id,
                evidence.project_id,
                evidence.view_id,
                evidence.reference_id,
                evidence.reference_sha256,
                evidence.camera_hash,
                evidence.render_set_object_sha256,
                evidence.comparison_report_object_sha256,
                evidence.quality_report_object_sha256,
                evidence.quality_status,
                evidence.created_at,
                evidence.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_visual_evidence_views(
        &self,
        candidate_id: &str,
    ) -> Result<Vec<VisualEvidenceViewRecord>, StoreError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT candidate_id, project_id, view_id, reference_id, reference_sha256, camera_hash, render_set_object_sha256, comparison_report_object_sha256, quality_report_object_sha256, quality_status, created_at, updated_at FROM visual_evidence_views WHERE candidate_id = ?1 ORDER BY view_id ASC",
        )?;
        let rows = statement.query_map(params![candidate_id], |row| {
            Ok(VisualEvidenceViewRecord {
                candidate_id: row.get(0)?,
                project_id: row.get(1)?,
                view_id: row.get(2)?,
                reference_id: row.get(3)?,
                reference_sha256: row.get(4)?,
                camera_hash: row.get(5)?,
                render_set_object_sha256: row.get(6)?,
                comparison_report_object_sha256: row.get(7)?,
                quality_report_object_sha256: row.get(8)?,
                quality_status: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn insert_cross_view_evidence(
        &self,
        evidence: &CrossViewEvidenceRecord,
    ) -> Result<(), StoreError> {
        if !is_sha256(&evidence.bundle_object_sha256)
            || !is_opaque_id(&evidence.candidate_id)
            || !is_opaque_id(&evidence.project_id)
            || !is_opaque_id(&evidence.session_id)
            || !is_sha256(&evidence.reference_canvas_sha256)
            || !matches!(
                evidence.aggregate_status.as_str(),
                "PARTIAL_VISIBLE_VIEW_PASS"
                    | "QUALITY_TARGET_NOT_MET"
                    | "BLOCKED_REFERENCE_COVERAGE"
            )
            || evidence.created_at.is_empty()
            || evidence.updated_at.is_empty()
        {
            return Err(StoreError::InvalidData(
                "cross-view evidence identity or status is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO cross_view_evidence (bundle_object_sha256, candidate_id, project_id, session_id, reference_canvas_sha256, aggregate_status, hard_gate_passed, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) ON CONFLICT(bundle_object_sha256) DO UPDATE SET candidate_id=excluded.candidate_id, project_id=excluded.project_id, session_id=excluded.session_id, reference_canvas_sha256=excluded.reference_canvas_sha256, aggregate_status=excluded.aggregate_status, hard_gate_passed=excluded.hard_gate_passed, updated_at=excluded.updated_at",
            params![
                evidence.bundle_object_sha256,
                evidence.candidate_id,
                evidence.project_id,
                evidence.session_id,
                evidence.reference_canvas_sha256,
                evidence.aggregate_status,
                evidence.hard_gate_passed,
                evidence.created_at,
                evidence.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_latest_cross_view_evidence(
        &self,
        candidate_id: &str,
    ) -> Result<Option<CrossViewEvidenceRecord>, StoreError> {
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT bundle_object_sha256, candidate_id, project_id, session_id, reference_canvas_sha256, aggregate_status, hard_gate_passed, created_at, updated_at FROM cross_view_evidence WHERE candidate_id = ?1 ORDER BY updated_at DESC, bundle_object_sha256 ASC LIMIT 1",
                params![candidate_id],
                |row| {
                    Ok(CrossViewEvidenceRecord {
                        bundle_object_sha256: row.get(0)?,
                        candidate_id: row.get(1)?,
                        project_id: row.get(2)?,
                        session_id: row.get(3)?,
                        reference_canvas_sha256: row.get(4)?,
                        aggregate_status: row.get(5)?,
                        hard_gate_passed: row.get::<_, i64>(6)? != 0,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn get_cross_view_evidence(
        &self,
        bundle_object_sha256: &str,
    ) -> Result<Option<CrossViewEvidenceRecord>, StoreError> {
        if !is_sha256(bundle_object_sha256) {
            return Err(StoreError::InvalidData(
                "cross-view bundle hash is invalid".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        Ok(connection
            .query_row(
                "SELECT bundle_object_sha256, candidate_id, project_id, session_id, reference_canvas_sha256, aggregate_status, hard_gate_passed, created_at, updated_at FROM cross_view_evidence WHERE bundle_object_sha256 = ?1",
                params![bundle_object_sha256],
                |row| {
                    Ok(CrossViewEvidenceRecord {
                        bundle_object_sha256: row.get(0)?,
                        candidate_id: row.get(1)?,
                        project_id: row.get(2)?,
                        session_id: row.get(3)?,
                        reference_canvas_sha256: row.get(4)?,
                        aggregate_status: row.get(5)?,
                        hard_gate_passed: row.get::<_, i64>(6)? != 0,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )
            .optional()?)
    }

    /// Insert or update one Runtime-owned DesignSession.  An existing session
    /// may only be resumed when its project/candidate/reference binding is
    /// identical; the Store never lets a caller retarget a session.
    pub fn agentic_session_create_or_resume(
        &self,
        session: &AgenticSessionRecord,
        object: &CasObjectRecord,
    ) -> Result<AgenticSessionRecord, StoreError> {
        validate_agentic_session(session)?;
        let payload = agentic_payload_json(session)?;
        ensure_agentic_object(
            &self.cas,
            object,
            session.object_sha256.as_deref(),
            &payload,
        )?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_agentic_binding_rows(
            &transaction,
            &session.project_id,
            &session.candidate_id,
            &session.reference_id,
            &session.reference_sha256,
        )?;
        let existing: Option<(String, String, i64, String, String, String, String)> = transaction
            .query_row(
                "SELECT project_id, candidate_id, revision, reference_id, reference_sha256, camera_hash, evidence_sha256 FROM agentic_design_sessions WHERE session_id = ?1",
                params![session.session_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .optional()?;
        if let Some(existing) = existing {
            let matches = existing.0 == session.project_id
                && existing.1 == session.candidate_id
                && existing.2 == session.revision
                && existing.3 == session.reference_id
                && existing.4 == session.reference_sha256
                && existing.5 == session.camera_hash
                && existing.6 == session.evidence_sha256;
            if !matches {
                return Err(StoreError::Contract {
                    code: "AGENTIC_SESSION_BINDING_MISMATCH".to_owned(),
                    message: "session cannot be resumed with another project, candidate, revision, reference, camera or evidence".to_owned(),
                });
            }
        }
        transaction.execute(
            "INSERT INTO agentic_design_sessions (session_id, project_id, candidate_id, revision, candidate_state_sha256, reference_id, reference_sha256, camera_hash, evidence_sha256, current_checkpoint_id, current_checkpoint_sha256, object_sha256, canonical_sha256, payload_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16) ON CONFLICT(session_id) DO UPDATE SET project_id=excluded.project_id, candidate_id=excluded.candidate_id, revision=excluded.revision, candidate_state_sha256=excluded.candidate_state_sha256, reference_id=excluded.reference_id, reference_sha256=excluded.reference_sha256, camera_hash=excluded.camera_hash, evidence_sha256=excluded.evidence_sha256, current_checkpoint_id=excluded.current_checkpoint_id, current_checkpoint_sha256=excluded.current_checkpoint_sha256, object_sha256=excluded.object_sha256, canonical_sha256=excluded.canonical_sha256, payload_json=excluded.payload_json, updated_at=excluded.updated_at",
            params![
                session.session_id,
                session.project_id,
                session.candidate_id,
                session.revision,
                session.candidate_state_sha256,
                session.reference_id,
                session.reference_sha256,
                session.camera_hash,
                session.evidence_sha256,
                session.current_checkpoint_id,
                session.current_checkpoint_sha256,
                object.sha256,
                session.canonical_sha256,
                payload,
                session.created_at,
                session.updated_at,
            ],
        )?;
        transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
            params![object.sha256],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_agentic_session(&session.session_id)?
            .ok_or_else(|| {
                StoreError::InvalidData("agentic session disappeared after write".to_owned())
            })
    }

    /// Atomically persist an immutable checkpoint and advance the session's
    /// current pointer.  The parent pointer is checked against the stored
    /// session to reject stale writers before either row changes.
    pub fn agentic_checkpoint_prepare(
        &self,
        checkpoint: &AgenticCheckpointRecord,
        session: &AgenticSessionRecord,
        checkpoint_object: &CasObjectRecord,
        session_object: &CasObjectRecord,
    ) -> Result<(), StoreError> {
        validate_agentic_checkpoint(checkpoint)?;
        validate_agentic_session(session)?;
        let checkpoint_payload = agentic_payload_json(checkpoint)?;
        let session_payload = agentic_payload_json(session)?;
        ensure_agentic_object(
            &self.cas,
            checkpoint_object,
            checkpoint.object_sha256.as_deref(),
            &checkpoint_payload,
        )?;
        ensure_agentic_object(
            &self.cas,
            session_object,
            session.object_sha256.as_deref(),
            &session_payload,
        )?;
        if checkpoint.session_id != session.session_id
            || checkpoint.project_id != session.project_id
            || checkpoint.candidate_id != session.candidate_id
            || checkpoint.revision != session.revision
            || checkpoint.candidate_state_sha256 != session.candidate_state_sha256
            || checkpoint.observation_sha256 != session.observation_sha256
        {
            return Err(StoreError::Contract {
                code: "AGENTIC_CHECKPOINT_BINDING_MISMATCH".to_owned(),
                message: "checkpoint does not bind to the session state".to_owned(),
            });
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_agentic_binding_rows(
            &transaction,
            &session.project_id,
            &session.candidate_id,
            &session.reference_id,
            &session.reference_sha256,
        )?;
        let stored_session: Option<(String, String, String, Option<String>, Option<String>)> = transaction
            .query_row(
                "SELECT project_id, candidate_id, evidence_sha256, current_checkpoint_id, current_checkpoint_sha256 FROM agentic_design_sessions WHERE session_id = ?1",
                params![session.session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?;
        let Some(stored_session) = stored_session else {
            return Err(StoreError::Contract {
                code: "AGENTIC_SESSION_NOT_FOUND".to_owned(),
                message: "checkpoint requires a persisted session".to_owned(),
            });
        };
        if stored_session.0 != session.project_id
            || stored_session.1 != session.candidate_id
            || stored_session.2 != session.evidence_sha256
            || stored_session.3 != checkpoint.parent_checkpoint_id
            || stored_session.4 != checkpoint.parent_checkpoint_sha256
        {
            return Err(StoreError::Contract {
                code: "AGENTIC_SESSION_STALE".to_owned(),
                message: "checkpoint parent or session evidence is stale".to_owned(),
            });
        }
        if let Some(parent_id) = checkpoint.parent_checkpoint_id.as_deref() {
            let parent: Option<(String, String, String, String)> = transaction
                .query_row(
                    "SELECT session_id, project_id, candidate_id, canonical_sha256 FROM agentic_design_checkpoints WHERE checkpoint_id = ?1",
                    params![parent_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?;
            if parent.as_ref().map(|row| {
                row.0 != checkpoint.session_id
                    || row.1 != checkpoint.project_id
                    || row.2 != checkpoint.candidate_id
                    || row.3
                        != checkpoint
                            .parent_checkpoint_sha256
                            .clone()
                            .unwrap_or_default()
            }) != Some(false)
            {
                return Err(StoreError::Contract {
                    code: "AGENTIC_PARENT_BINDING_MISMATCH".to_owned(),
                    message: "checkpoint parent is not part of the same session binding".to_owned(),
                });
            }
        }
        transaction.execute(
            "INSERT INTO agentic_design_checkpoints (checkpoint_id, session_id, project_id, candidate_id, revision, candidate_state_sha256, artifact_sha256, reference_id, reference_sha256, camera_hash, input_sha256, evidence_sha256, parent_checkpoint_id, parent_checkpoint_sha256, object_sha256, canonical_sha256, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                checkpoint.checkpoint_id,
                checkpoint.session_id,
                checkpoint.project_id,
                checkpoint.candidate_id,
                checkpoint.revision,
                checkpoint.candidate_state_sha256,
                checkpoint.artifact_sha256,
                checkpoint.reference_id,
                checkpoint.reference_sha256,
                checkpoint.camera_hash,
                checkpoint.input_sha256,
                checkpoint.evidence_sha256,
                checkpoint.parent_checkpoint_id,
                checkpoint.parent_checkpoint_sha256,
                checkpoint_object.sha256,
                checkpoint.canonical_sha256,
                checkpoint_payload,
                checkpoint.created_at,
            ],
        )?;
        transaction.execute(
            "UPDATE agentic_design_sessions SET project_id=?1, candidate_id=?2, revision=?3, candidate_state_sha256=?4, reference_id=?5, reference_sha256=?6, camera_hash=?7, evidence_sha256=?8, current_checkpoint_id=?9, current_checkpoint_sha256=?10, object_sha256=?11, canonical_sha256=?12, payload_json=?13, updated_at=?14 WHERE session_id=?15",
            params![
                session.project_id,
                session.candidate_id,
                session.revision,
                session.candidate_state_sha256,
                session.reference_id,
                session.reference_sha256,
                session.camera_hash,
                session.evidence_sha256,
                session.current_checkpoint_id,
                session.current_checkpoint_sha256,
                session_object.sha256,
                session.canonical_sha256,
                session_payload,
                session.updated_at,
                session.session_id,
            ],
        )?;
        transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 IN (?1, ?2)",
            params![checkpoint_object.sha256, session_object.sha256],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_agentic_session(
        &self,
        session_id: &str,
    ) -> Result<Option<AgenticSessionRecord>, StoreError> {
        let connection = self.lock_connection()?;
        let row: Option<(String, String, String, String, String)> = connection
            .query_row(
                "SELECT payload_json, object_sha256, canonical_sha256, project_id, candidate_id FROM agentic_design_sessions WHERE session_id = ?1",
                params![session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?;
        row.map(
            |(payload, object_sha256, canonical_sha256, project_id, candidate_id)| {
                let mut record: AgenticSessionRecord =
                    serde_json::from_str(&payload).map_err(|error| {
                        StoreError::InvalidData(format!("invalid agentic session payload: {error}"))
                    })?;
                if record.project_id != project_id
                    || record.candidate_id != candidate_id
                    || record.session_id != session_id
                {
                    return Err(StoreError::Contract {
                        code: "AGENTIC_SESSION_PAYLOAD_BINDING_MISMATCH".to_owned(),
                        message: "session payload does not match its SQLite binding".to_owned(),
                    });
                }
                record.object_sha256 = Some(object_sha256);
                record.canonical_sha256 = canonical_sha256;
                validate_agentic_session(&record)?;
                Ok(record)
            },
        )
        .transpose()
    }

    pub fn get_agentic_session_for_binding(
        &self,
        project_id: &str,
        candidate_id: &str,
    ) -> Result<Option<AgenticSessionRecord>, StoreError> {
        let connection = self.lock_connection()?;
        let session_id: Option<String> = connection
            .query_row(
                "SELECT session_id FROM agentic_design_sessions WHERE project_id = ?1 AND candidate_id = ?2 ORDER BY updated_at DESC, session_id DESC LIMIT 1",
                params![project_id, candidate_id],
                |row| row.get(0),
            )
            .optional()?;
        drop(connection);
        session_id
            .as_deref()
            .map(|session_id| self.get_agentic_session(session_id))
            .transpose()
            .map(|value| value.flatten())
    }

    pub fn get_agentic_checkpoint(
        &self,
        checkpoint_id: &str,
    ) -> Result<Option<AgenticCheckpointRecord>, StoreError> {
        let connection = self.lock_connection()?;
        let row: Option<(String, String, String, String, String, String)> = connection
            .query_row(
                "SELECT payload_json, object_sha256, canonical_sha256, session_id, project_id, candidate_id FROM agentic_design_checkpoints WHERE checkpoint_id = ?1",
                params![checkpoint_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .optional()?;
        row.map(
            |(payload, object_sha256, canonical_sha256, session_id, project_id, candidate_id)| {
                let mut record: AgenticCheckpointRecord =
                    serde_json::from_str(&payload).map_err(|error| {
                        StoreError::InvalidData(format!(
                            "invalid agentic checkpoint payload: {error}"
                        ))
                    })?;
                if record.project_id != project_id
                    || record.candidate_id != candidate_id
                    || record.session_id != session_id
                    || record.checkpoint_id != checkpoint_id
                {
                    return Err(StoreError::Contract {
                        code: "AGENTIC_CHECKPOINT_PAYLOAD_BINDING_MISMATCH".to_owned(),
                        message: "checkpoint payload does not match its SQLite binding".to_owned(),
                    });
                }
                record.object_sha256 = Some(object_sha256);
                record.canonical_sha256 = canonical_sha256;
                validate_agentic_checkpoint(&record)?;
                Ok(record)
            },
        )
        .transpose()
    }

    /// Persist one immutable Runtime-owned action-run record.  Resuming means
    /// reading the existing immutable record: a repeated run_id may not update
    /// its input, stage results, status, checkpoint pointer, or canonical data.
    pub fn agentic_action_run_create_or_resume(
        &self,
        run: &AgenticActionRunRecord,
        object: &CasObjectRecord,
    ) -> Result<AgenticActionRunRecord, StoreError> {
        validate_agentic_action_run(run)?;
        validate_agentic_action_run_object(object)?;
        let payload = agentic_action_run_payload_json(run)?;
        let stage_results_json = canonical_json_bytes(&run.stage_results)
            .map_err(|error| StoreError::InvalidData(error.to_string()))
            .and_then(|bytes| {
                String::from_utf8(bytes).map_err(|error| StoreError::InvalidData(error.to_string()))
            })?;
        ensure_agentic_object(&self.cas, object, run.object_sha256.as_deref(), &payload)?;

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_agentic_action_run_binding_rows(&transaction, run)?;
        ensure_agentic_action_run_object_row(&transaction, object)?;

        let existing: Option<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        )> =
            transaction
                .query_row(
                    "SELECT payload_json, stage_results_json, object_sha256, input_sha256, canonical_sha256, session_id, project_id, candidate_id FROM agentic_action_runs WHERE run_id = ?1",
                    params![run.run_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                            row.get(7)?,
                        ))
                    },
                )
                .optional()?;
        if let Some(existing) = existing {
            if existing.3 != run.input_sha256 {
                return Err(StoreError::Contract {
                    code: "AGENTIC_ACTION_RUN_INPUT_MISMATCH".to_owned(),
                    message: "run_id is already bound to another input hash".to_owned(),
                });
            }
            if existing.0 != payload
                || existing.1 != stage_results_json
                || existing.2 != object.sha256
                || existing.4 != run.canonical_sha256
                || existing.5 != run.session_id
                || existing.6 != run.project_id
                || existing.7 != run.candidate_id
            {
                return Err(StoreError::Contract {
                    code: "AGENTIC_ACTION_RUN_IMMUTABLE_CONFLICT".to_owned(),
                    message: "an existing run cannot be silently changed while resuming".to_owned(),
                });
            }
            transaction.commit()?;
            drop(connection);
            return self.get_agentic_action_run(&run.run_id)?.ok_or_else(|| {
                StoreError::InvalidData("action run disappeared after resume".to_owned())
            });
        }

        transaction.execute(
            "INSERT INTO agentic_action_runs (run_id, session_id, project_id, candidate_id, input_sha256, status, completed_stage, stage_results_json, checkpoint_id, checkpoint_sha256, immutable, object_sha256, canonical_sha256, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                run.run_id,
                run.session_id,
                run.project_id,
                run.candidate_id,
                run.input_sha256,
                run.status,
                run.completed_stage,
                stage_results_json,
                run.checkpoint_id,
                run.checkpoint_sha256,
                run.immutable,
                object.sha256,
                run.canonical_sha256,
                payload,
                run.created_at,
            ],
        )?;
        let marked_reachable = transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
            params![object.sha256],
        )?;
        if marked_reachable != 1 {
            return Err(StoreError::Contract {
                code: "AGENTIC_ACTION_RUN_CAS_OBJECT_UNAVAILABLE".to_owned(),
                message: "action run CAS metadata disappeared during the write".to_owned(),
            });
        }
        transaction.commit()?;
        drop(connection);
        self.get_agentic_action_run(&run.run_id)?.ok_or_else(|| {
            StoreError::InvalidData("action run disappeared after create".to_owned())
        })
    }

    pub fn get_agentic_action_run(
        &self,
        run_id: &str,
    ) -> Result<Option<AgenticActionRunRecord>, StoreError> {
        if !is_opaque_id(run_id) {
            return Err(StoreError::InvalidData(
                "invalid agentic action run id".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        let row: Option<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        )> = connection
            .query_row(
                "SELECT payload_json, stage_results_json, object_sha256, canonical_sha256, session_id, project_id, candidate_id, input_sha256 FROM agentic_action_runs WHERE run_id = ?1",
                params![run_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .optional()?;
        drop(connection);

        let Some((
            payload,
            stage_results_json,
            object_sha256,
            canonical_sha256,
            session_id,
            project_id,
            candidate_id,
            input_sha256,
        )) = row
        else {
            return Ok(None);
        };
        let object = self
            .get_object(&object_sha256)?
            .ok_or_else(|| StoreError::Contract {
                code: "AGENTIC_ACTION_RUN_CAS_OBJECT_UNAVAILABLE".to_owned(),
                message: "action run CAS metadata is missing".to_owned(),
            })?;
        validate_agentic_action_run_object(&object)?;
        let cas_payload = self.cas.read_verified(&object.sha256)?;
        if cas_payload != payload.as_bytes() {
            return Err(StoreError::Contract {
                code: "AGENTIC_ACTION_RUN_CAS_PAYLOAD_MISMATCH".to_owned(),
                message: "SQLite action run payload differs from its CAS object".to_owned(),
            });
        }

        let mut record: AgenticActionRunRecord =
            serde_json::from_str(&payload).map_err(|error| {
                StoreError::InvalidData(format!("invalid agentic action run payload: {error}"))
            })?;
        let record_stage_results_json = canonical_json_bytes(&record.stage_results)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if record.object_sha256.is_some()
            || record.run_id != run_id
            || record.session_id != session_id
            || record.project_id != project_id
            || record.candidate_id != candidate_id
            || record.input_sha256 != input_sha256
            || record.canonical_sha256 != canonical_sha256
            || record_stage_results_json != stage_results_json.as_bytes()
        {
            return Err(StoreError::Contract {
                code: "AGENTIC_ACTION_RUN_PAYLOAD_BINDING_MISMATCH".to_owned(),
                message: "action run payload does not match its SQLite binding".to_owned(),
            });
        }
        record.object_sha256 = Some(object_sha256);
        validate_agentic_action_run(&record)?;
        Ok(Some(record))
    }

    pub fn agentic_action_run_get(
        &self,
        run_id: &str,
    ) -> Result<Option<AgenticActionRunRecord>, StoreError> {
        self.get_agentic_action_run(run_id)
    }

    /// Persist one immutable DesignActionRun@1 contract alongside the older
    /// AgenticActionRun@1 envelope.  The table columns retain only the
    /// binding/index fields; the complete contract is the canonical CAS
    /// payload and is read back byte-for-byte after a Runtime restart.
    pub fn design_action_run_create_or_resume(
        &self,
        run: &Value,
        object: &CasObjectRecord,
        created_at: &str,
    ) -> Result<Value, StoreError> {
        validate_design_action_run_value(run)?;
        let payload_bytes = canonical_json_bytes(run)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let payload = String::from_utf8(payload_bytes)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if payload.len() > 256 * 1024 {
            return Err(StoreError::InvalidData(
                "DesignActionRun payload exceeds bounded metadata capacity".to_owned(),
            ));
        }
        ensure_agentic_object(&self.cas, object, Some(object.sha256.as_str()), &payload)?;

        let object_value = run.as_object().expect("validated DesignActionRun object");
        let run_id = object_value["run_id"].as_str().expect("validated run_id");
        let session_id = object_value["session_id"]
            .as_str()
            .expect("validated session_id");
        let project_id = object_value["project_id"]
            .as_str()
            .expect("validated project_id");
        let candidate_id = object_value["candidate_id"]
            .as_str()
            .expect("validated candidate_id");
        let input_sha256 = object_value["input_sha256"]
            .as_str()
            .expect("validated input_sha256");
        let status = object_value["status"].as_str().expect("validated status");
        let completed_stage = object_value
            .get("completed_stage")
            .and_then(Value::as_str)
            .unwrap_or("");
        let stage_results_json = canonical_json_bytes(&object_value["stage_results"])
            .map_err(|error| StoreError::InvalidData(error.to_string()))
            .and_then(|bytes| {
                String::from_utf8(bytes).map_err(|error| StoreError::InvalidData(error.to_string()))
            })?;
        let checkpoint_id = object_value.get("checkpoint_id").and_then(Value::as_str);
        let checkpoint_hash = object_value.get("checkpoint_hash").and_then(Value::as_str);
        let canonical_sha256 = object_value["canonical_sha256"]
            .as_str()
            .expect("validated canonical_sha256");
        if created_at.is_empty() || created_at.len() > 64 {
            return Err(StoreError::InvalidData(
                "DesignActionRun created_at is malformed".to_owned(),
            ));
        }

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_design_action_run_binding_rows(&transaction, run)?;
        ensure_agentic_action_run_object_row(&transaction, object)?;
        let existing: Option<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        )> = transaction
            .query_row(
                "SELECT payload_json, stage_results_json, object_sha256, input_sha256, canonical_sha256, session_id, project_id, candidate_id FROM agentic_action_runs WHERE run_id = ?1",
                params![run_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing.3 != input_sha256 {
                return Err(StoreError::Contract {
                    code: "DESIGN_ACTION_RUN_INPUT_MISMATCH".to_owned(),
                    message: "run_id is already bound to another input hash".to_owned(),
                });
            }
            if existing.0 != payload
                || existing.1 != stage_results_json
                || existing.2 != object.sha256
                || existing.4 != canonical_sha256
                || existing.5 != session_id
                || existing.6 != project_id
                || existing.7 != candidate_id
            {
                return Err(StoreError::Contract {
                    code: "DESIGN_ACTION_RUN_IMMUTABLE_CONFLICT".to_owned(),
                    message: "an existing DesignActionRun cannot be changed while resuming"
                        .to_owned(),
                });
            }
            transaction.commit()?;
            drop(connection);
            return self.get_design_action_run(run_id)?.ok_or_else(|| {
                StoreError::InvalidData("DesignActionRun disappeared after resume".to_owned())
            });
        }

        transaction.execute(
            "INSERT INTO agentic_action_runs (run_id, session_id, project_id, candidate_id, input_sha256, status, completed_stage, stage_results_json, checkpoint_id, checkpoint_sha256, immutable, object_sha256, canonical_sha256, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?12, ?13, ?14)",
            params![
                run_id,
                session_id,
                project_id,
                candidate_id,
                input_sha256,
                status,
                completed_stage,
                stage_results_json,
                checkpoint_id,
                checkpoint_hash,
                object.sha256,
                canonical_sha256,
                payload,
                created_at,
            ],
        )?;
        let marked_reachable = transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
            params![object.sha256],
        )?;
        if marked_reachable != 1 {
            return Err(StoreError::Contract {
                code: "DESIGN_ACTION_RUN_CAS_OBJECT_UNAVAILABLE".to_owned(),
                message: "DesignActionRun CAS metadata disappeared during the write".to_owned(),
            });
        }
        transaction.commit()?;
        drop(connection);
        self.get_design_action_run(run_id)?.ok_or_else(|| {
            StoreError::InvalidData("DesignActionRun disappeared after create".to_owned())
        })
    }

    pub fn get_design_action_run(&self, run_id: &str) -> Result<Option<Value>, StoreError> {
        if !is_opaque_id(run_id) {
            return Err(StoreError::InvalidData(
                "invalid DesignActionRun id".to_owned(),
            ));
        }
        let connection = self.lock_connection()?;
        let row: Option<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        )> = connection
            .query_row(
                "SELECT payload_json, stage_results_json, object_sha256, canonical_sha256, session_id, project_id, candidate_id, input_sha256 FROM agentic_action_runs WHERE run_id = ?1",
                params![run_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .optional()?;
        drop(connection);
        let Some((
            payload,
            stage_results_json,
            object_sha256,
            canonical_sha256,
            session_id,
            project_id,
            candidate_id,
            input_sha256,
        )) = row
        else {
            return Ok(None);
        };
        let object = self
            .get_object(&object_sha256)?
            .ok_or_else(|| StoreError::Contract {
                code: "DESIGN_ACTION_RUN_CAS_OBJECT_UNAVAILABLE".to_owned(),
                message: "DesignActionRun CAS metadata is missing".to_owned(),
            })?;
        validate_agentic_action_run_object(&object)?;
        let cas_payload = self.cas.read_verified(&object.sha256)?;
        if cas_payload != payload.as_bytes() {
            return Err(StoreError::Contract {
                code: "DESIGN_ACTION_RUN_CAS_PAYLOAD_MISMATCH".to_owned(),
                message: "SQLite DesignActionRun payload differs from its CAS object".to_owned(),
            });
        }
        let value: Value = serde_json::from_str(&payload).map_err(|error| {
            StoreError::InvalidData(format!("invalid DesignActionRun payload: {error}"))
        })?;
        validate_design_action_run_value(&value)?;
        let object_value = value.as_object().expect("validated DesignActionRun object");
        let stored_stage_results = canonical_json_bytes(&object_value["stage_results"])
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if object_value.get("run_id").and_then(Value::as_str) != Some(run_id)
            || object_value.get("session_id").and_then(Value::as_str) != Some(session_id.as_str())
            || object_value.get("project_id").and_then(Value::as_str) != Some(project_id.as_str())
            || object_value.get("candidate_id").and_then(Value::as_str)
                != Some(candidate_id.as_str())
            || object_value.get("input_sha256").and_then(Value::as_str)
                != Some(input_sha256.as_str())
            || object_value.get("canonical_sha256").and_then(Value::as_str)
                != Some(canonical_sha256.as_str())
            || stored_stage_results != stage_results_json.as_bytes()
        {
            return Err(StoreError::Contract {
                code: "DESIGN_ACTION_RUN_PAYLOAD_BINDING_MISMATCH".to_owned(),
                message: "DesignActionRun payload does not match its SQLite binding".to_owned(),
            });
        }
        Ok(Some(value))
    }

    pub fn design_action_run_get(&self, run_id: &str) -> Result<Option<Value>, StoreError> {
        self.get_design_action_run(run_id)
    }

    pub fn prepare_restore_candidate(
        &self,
        request: &RestorePrepareRequest,
        now: &str,
    ) -> Result<RestorePrepareResult, StoreError> {
        validate_restore_prepare_request(request)?;
        let request_value = serde_json::to_value(request)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let request_sha256 = canonical_json_hash(&request_value);
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let _project = transaction
            .query_row(
                "SELECT active_snapshot_revision, head_snapshot_id FROM projects WHERE project_id = ?1",
                params![request.project_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?
            .ok_or_else(|| StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "project does not exist".to_owned(),
            })?;
        let current_head: Option<String> = transaction
            .query_row(
                "SELECT v.version_id FROM projects p JOIN snapshots s ON s.snapshot_id = p.head_snapshot_id JOIN design_asset_versions v ON v.candidate_id = s.candidate_id WHERE p.project_id = ?1 LIMIT 1",
                params![request.project_id],
                |row| row.get(0),
            )
            .optional()?;
        let bound_base_version_id = request
            .base_version_id
            .clone()
            .or_else(|| current_head.clone());
        if bound_base_version_id != current_head {
            return Err(StoreError::Contract {
                code: "STALE_BASE_VERSION".to_owned(),
                message: "project head changed before restore prepare".to_owned(),
            });
        }
        let source = transaction
            .query_row(
                "SELECT version_id, candidate_id, manifest_hash FROM design_asset_versions WHERE version_id = ?1 AND project_id = ?2",
                params![request.source_version_id, request.project_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            )
            .optional()?
            .ok_or_else(|| StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "restore source version not found in project".to_owned(),
            })?;
        let source_candidate = read_candidate_for_transaction(&transaction, &source.1)?
            .ok_or_else(|| StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "restore source candidate not found".to_owned(),
            })?;
        if source_candidate.state != "confirmed" || !source_candidate.quality_hard_gate_passed {
            return Err(StoreError::Contract {
                code: "RESTORE_SOURCE_UNCONFIRMED".to_owned(),
                message: "restore source must be a confirmed quality-passing candidate".to_owned(),
            });
        }
        let prepared_object_id =
            source_candidate
                .prepared_object_id
                .clone()
                .ok_or_else(|| StoreError::Contract {
                    code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                    message: "restore source has no prepared object".to_owned(),
                })?;
        let prepared_object_sha256 =
            source_candidate
                .prepared_object_sha256
                .clone()
                .ok_or_else(|| StoreError::Contract {
                    code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                    message: "restore source has no prepared object hash".to_owned(),
                })?;
        if prepared_object_sha256 != source.2
            || source_candidate.manifest_hash.as_deref() != Some(source.2.as_str())
        {
            return Err(StoreError::Contract {
                code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                message: "restore source manifest is not bound to its candidate object".to_owned(),
            });
        }
        let object_exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM objects WHERE sha256 = ?1",
                params![prepared_object_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if object_exists.is_none() {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "restore source CAS object is unavailable".to_owned(),
            });
        }
        // The source must still have passed a quality gate, but a restored
        // candidate must not inherit that passing state directly. Runtime
        // creates candidate-bound V2 readback/quality/evidence before it
        // transitions this newly prepared candidate to `reviewable`.
        source_candidate
            .quality_report_id
            .as_deref()
            .ok_or_else(|| StoreError::Contract {
                code: "QUALITY_HARD_GATE_FAILED".to_owned(),
                message: "restore source has no quality report".to_owned(),
            })?;
        let candidate_id = format!("candidate-{}", Uuid::new_v4().simple());
        let job_id = format!("job-{}", Uuid::new_v4().simple());
        let canonical_sha256 = canonical_json_hash(&serde_json::json!({
            "schema_version": "Candidate@1",
            "candidate_id": candidate_id,
            "project_id": request.project_id,
            "base_version_id": bound_base_version_id,
            "source_version_id": request.source_version_id,
            "prepared_object_id": prepared_object_id,
            "prepared_object_sha256": prepared_object_sha256,
            "state": "prepared",
            "request_sha256": request_sha256,
            "manifest_hash": source.2,
            "quality_report_id": null,
            "quality_hard_gate_passed": false,
            "created_at": now,
            "updated_at": now,
        }));
        let candidate = CandidateRecord {
            schema_version: "Candidate@1".to_owned(),
            candidate_id: candidate_id.clone(),
            project_id: request.project_id.clone(),
            base_version_id: bound_base_version_id,
            source_version_id: Some(request.source_version_id.clone()),
            prepared_object_id: Some(prepared_object_id.clone()),
            prepared_object_sha256: Some(prepared_object_sha256.clone()),
            state: "prepared".to_owned(),
            request_sha256: request_sha256.clone(),
            manifest_hash: Some(source.2.clone()),
            quality_report_id: None,
            quality_hard_gate_passed: false,
            canonical_sha256,
            error_code: None,
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
        };
        let job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: job_id.clone(),
            project_id: request.project_id.clone(),
            kind: "restore_prepare".to_owned(),
            status: "succeeded".to_owned(),
            progress: 100,
            request_sha256: request_sha256.clone(),
            checkpoint_sha256: None,
            error_code: None,
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
        };
        let event = JobEventRecord {
            schema_version: "RuntimeJobEvent@1".to_owned(),
            job_id: job_id.clone(),
            sequence: 1,
            kind: "restore_prepared".to_owned(),
            payload: serde_json::json!({
                "candidate_id": candidate_id,
                "source_version_id": request.source_version_id,
                "prepared_object_sha256": prepared_object_sha256,
            }),
            created_at: now.to_owned(),
        };
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: "restore_prepared".to_owned(),
            object_id: Some(candidate.candidate_id.clone()),
            request_sha256: Some(request_sha256),
            payload: serde_json::json!({
                "candidate_id": candidate.candidate_id,
                "job_id": job_id,
                "source_version_id": request.source_version_id,
            }),
            created_at: now.to_owned(),
        };
        validate_candidate(&candidate)?;
        validate_job(&job)?;
        validate_job_event(&event)?;
        validate_audit(&audit)?;
        let event_payload = serde_json::to_string(&event.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let audit_payload = serde_json::to_string(&audit.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        transaction.execute(
            "INSERT INTO candidates (candidate_id, project_id, base_version_id, source_version_id, prepared_object_id, prepared_object_sha256, state, request_sha256, manifest_hash, quality_report_id, quality_hard_gate_passed, canonical_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![candidate.candidate_id, candidate.project_id, candidate.base_version_id, candidate.source_version_id, candidate.prepared_object_id, candidate.prepared_object_sha256, candidate.state, candidate.request_sha256, candidate.manifest_hash, candidate.quality_report_id, candidate.quality_hard_gate_passed, candidate.canonical_sha256, candidate.error_code, candidate.created_at, candidate.updated_at],
        )?;
        transaction.execute(
            "INSERT INTO runtime_jobs (job_id, project_id, kind, status, progress, request_sha256, checkpoint_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![job.job_id, job.project_id, job.kind, job.status, i64::from(job.progress), job.request_sha256, job.checkpoint_sha256, job.error_code, job.created_at, job.updated_at],
        )?;
        transaction.execute(
            "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![event.job_id, event.sequence, event.kind, event_payload, event.created_at],
        )?;
        transaction.execute(
            "INSERT INTO audit_events (audit_id, project_id, kind, object_id, request_sha256, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![audit.audit_id, audit.project_id, audit.kind, audit.object_id, audit.request_sha256, audit_payload, audit.created_at],
        )?;
        transaction.commit()?;
        drop(connection);
        let job = self
            .get_job(&job.job_id)?
            .ok_or_else(|| StoreError::InvalidData("restore job disappeared".to_owned()))?;
        Ok(RestorePrepareResult {
            schema_version: "RestorePrepareResult@1".to_owned(),
            candidate,
            job,
            source_version_id: request.source_version_id.clone(),
        })
    }

    pub fn confirm_candidate(
        &self,
        request: &CandidateConfirmRequest,
        now: &str,
    ) -> Result<CandidateConfirmResult, StoreError> {
        self.confirm_candidate_with_tool(request, now, "candidate_confirm", None, None)
    }

    pub fn confirm_cross_view_candidate(
        &self,
        request: &CandidateConfirmRequest,
        now: &str,
        request_sha256: &str,
    ) -> Result<CandidateConfirmResult, StoreError> {
        if !is_sha256(request_sha256) {
            return Err(StoreError::InvalidData(
                "cross-view promotion request hash is invalid".to_owned(),
            ));
        }
        self.confirm_candidate_with_tool(
            request,
            now,
            "cross_view_promotion_confirm",
            None,
            Some(request_sha256),
        )
    }

    pub fn confirm_repair_apply_candidate(
        &self,
        request: &CandidateConfirmRequest,
        now: &str,
        request_sha256: &str,
    ) -> Result<CandidateConfirmResult, StoreError> {
        if !is_sha256(request_sha256) {
            return Err(StoreError::InvalidData(
                "repair apply request hash is invalid".to_owned(),
            ));
        }
        self.confirm_candidate_with_tool(
            request,
            now,
            "repair_apply_confirm",
            None,
            Some(request_sha256),
        )
    }

    pub fn restore_confirm(
        &self,
        request: &RestoreConfirmRequest,
        now: &str,
    ) -> Result<RestoreConfirmResult, StoreError> {
        validate_restore_confirm_request(request)?;
        let candidate_request = CandidateConfirmRequest {
            project_id: request.project_id.clone(),
            candidate_id: request.candidate_id.clone(),
            base_version_id: request.base_version_id.clone(),
            prepared_object_id: request.prepared_object_id.clone(),
            prepared_object_sha256: request.prepared_object_sha256.clone(),
            quality_report_id: request.quality_report_id.clone(),
            approval_receipt_id: request.approval_receipt_id.clone(),
            approval_summary: request.approval_summary.clone(),
            approval_session_id: request.approval_session_id.clone(),
            approval_expires_at: request.approval_expires_at.clone(),
            idempotency_key: request.idempotency_key.clone(),
        };
        let result = self.confirm_candidate_with_tool(
            &candidate_request,
            now,
            "restore_confirm",
            Some(request.source_version_id.as_str()),
            Some(&canonical_json_hash(
                &serde_json::to_value(request)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
            )),
        )?;
        Ok(RestoreConfirmResult {
            schema_version: "RestoreConfirmResult@1".to_owned(),
            candidate_id: result.candidate_id,
            project_id: result.project_id,
            source_version_id: request.source_version_id.clone(),
            version_id: result.version_id,
            snapshot_id: result.snapshot_id,
            approval_receipt_id: result.approval_receipt_id,
            request_sha256: result.request_sha256,
            replayed: result.replayed,
        })
    }

    fn confirm_candidate_with_tool(
        &self,
        request: &CandidateConfirmRequest,
        now: &str,
        approval_tool: &str,
        expected_source_version_id: Option<&str>,
        request_sha256_override: Option<&str>,
    ) -> Result<CandidateConfirmResult, StoreError> {
        validate_confirm_request(request)?;
        let request_value = serde_json::to_value(request)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let request_sha256 = request_sha256_override
            .map(str::to_owned)
            .unwrap_or_else(|| canonical_json_hash(&request_value));
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;

        if let Some((project_id, tool, stored_hash, response_json)) = transaction
            .query_row(
                "SELECT project_id, tool, request_sha256, response_json FROM write_idempotency WHERE idempotency_key = ?1",
                params![request.idempotency_key],
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
        {
            if project_id != request.project_id
                || tool != approval_tool
                || stored_hash != request_sha256
            {
                return Err(StoreError::Contract {
                    code: "IDEMPOTENCY_KEY_REUSED".to_owned(),
                    message: "idempotency key is bound to a different request".to_owned(),
                });
            }
            let mut result: CandidateConfirmResult = serde_json::from_str(&response_json)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            result.replayed = true;
            return Ok(result);
        }

        let project = transaction
            .query_row(
                "SELECT active_snapshot_revision, head_snapshot_id FROM projects WHERE project_id = ?1",
                params![request.project_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?
            .ok_or_else(|| StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "project does not exist".to_owned(),
            })?;
        let candidate = read_candidate_for_transaction(&transaction, &request.candidate_id)?
            .ok_or_else(|| StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "candidate not found".to_owned(),
            })?;
        if candidate.project_id != request.project_id {
            return Err(StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "candidate is outside the requested project".to_owned(),
            });
        }
        if expected_source_version_id.is_some() {
            if candidate.source_version_id.as_deref() != expected_source_version_id {
                return Err(StoreError::Contract {
                    code: "RESTORE_SOURCE_MISMATCH".to_owned(),
                    message: "restore candidate is not bound to the requested source version"
                        .to_owned(),
                });
            }
        } else if candidate.source_version_id.is_some() {
            return Err(StoreError::Contract {
                code: "CANDIDATE_OPERATION_MISMATCH".to_owned(),
                message: "restore candidate requires restore_confirm".to_owned(),
            });
        }
        if candidate.state != "reviewable" || !candidate.quality_hard_gate_passed {
            return Err(StoreError::Contract {
                code: "QUALITY_HARD_GATE_FAILED".to_owned(),
                message: "candidate is not reviewable with a passing hard quality gate".to_owned(),
            });
        }
        if candidate.base_version_id != request.base_version_id {
            return Err(StoreError::Contract {
                code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                message: "confirm base does not match the prepared candidate".to_owned(),
            });
        }
        if candidate.prepared_object_id.as_deref() != Some(request.prepared_object_id.as_str())
            || candidate.prepared_object_sha256.as_deref()
                != Some(request.prepared_object_sha256.as_str())
            || candidate.quality_report_id.as_deref() != Some(request.quality_report_id.as_str())
        {
            return Err(StoreError::Contract {
                code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                message: "prepared object or quality binding does not match the candidate"
                    .to_owned(),
            });
        }
        let current_head: Option<String> = transaction
            .query_row(
                "SELECT v.version_id FROM projects p JOIN snapshots s ON s.snapshot_id = p.head_snapshot_id JOIN design_asset_versions v ON v.candidate_id = s.candidate_id WHERE p.project_id = ?1 LIMIT 1",
                params![request.project_id],
                |row| row.get(0),
            )
            .optional()?;
        if current_head != request.base_version_id {
            return Err(StoreError::Contract {
                code: "STALE_BASE_VERSION".to_owned(),
                message: "project head changed after the candidate was prepared".to_owned(),
            });
        }
        let object_exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM objects WHERE sha256 = ?1",
                params![request.prepared_object_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if object_exists.is_none() {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "prepared CAS object is unavailable".to_owned(),
            });
        }
        // The caller supplies approval context, but the durable receipt ID is
        // always minted by Runtime inside this transaction.
        let approval_receipt_id = generated_approval_receipt_id();
        if is_expired(now, &request.approval_expires_at)? {
            let approval = approval_record(
                request.project_id.as_str(),
                approval_tool,
                approval_receipt_id.as_str(),
                candidate.base_version_id.as_deref(),
                request.prepared_object_id.as_str(),
                request.prepared_object_sha256.as_str(),
                Some(request.quality_report_id.as_str()),
                request.approval_summary.as_str(),
                "expired",
                request.approval_expires_at.as_str(),
                request.approval_session_id.as_str(),
                now,
            )?;
            insert_approval(&transaction, &approval)?;
            transaction.commit()?;
            return Err(StoreError::Contract {
                code: "APPROVAL_EXPIRED".to_owned(),
                message: "approval receipt expired before confirm".to_owned(),
            });
        }
        let approval = approval_record(
            request.project_id.as_str(),
            approval_tool,
            approval_receipt_id.as_str(),
            candidate.base_version_id.as_deref(),
            request.prepared_object_id.as_str(),
            request.prepared_object_sha256.as_str(),
            Some(request.quality_report_id.as_str()),
            request.approval_summary.as_str(),
            "approved",
            request.approval_expires_at.as_str(),
            request.approval_session_id.as_str(),
            now,
        )?;
        insert_approval(&transaction, &approval)?;
        let marked_reachable = transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
            params![request.prepared_object_sha256],
        )?;
        if marked_reachable != 1 {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "prepared CAS object disappeared before confirm".to_owned(),
            });
        }

        let manifest_hash = candidate
            .manifest_hash
            .clone()
            .or_else(|| candidate.prepared_object_sha256.clone())
            .ok_or_else(|| StoreError::Contract {
                code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                message: "candidate has no manifest hash".to_owned(),
            })?;
        if !is_sha256(&manifest_hash) {
            return Err(StoreError::Contract {
                code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                message: "candidate manifest hash is invalid".to_owned(),
            });
        }
        let version_id = format!("version-{}", Uuid::new_v4().simple());
        let version_created_at = now.to_owned();
        let version_canonical_sha256 = canonical_json_hash(&serde_json::json!({
            "schema_version": "DesignAssetVersion@1",
            "version_id": version_id,
            "project_id": request.project_id,
            "parent_version_id": request.base_version_id,
            "candidate_id": request.candidate_id,
            "manifest_hash": manifest_hash,
            "created_at": version_created_at,
        }));
        transaction.execute(
            "INSERT INTO design_asset_versions (version_id, project_id, parent_version_id, candidate_id, manifest_hash, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                version_id,
                request.project_id,
                request.base_version_id,
                request.candidate_id,
                manifest_hash,
                version_canonical_sha256,
                version_created_at,
            ],
        )?;

        let snapshot_id = format!("snapshot-{}", Uuid::new_v4().simple());
        let snapshot_revision = project.0 + 1;
        let snapshot_canonical_sha256 = canonical_json_hash(&serde_json::json!({
            "schema_version": "ActiveDesignSnapshot@1",
            "snapshot_id": snapshot_id,
            "project_id": request.project_id,
            "parent_snapshot_id": project.1,
            "candidate_id": request.candidate_id,
            "revision": snapshot_revision,
            "status": "confirmed",
            "manifest_hash": manifest_hash,
            "created_at": now,
        }));
        transaction.execute(
            "INSERT INTO snapshots (snapshot_id, project_id, parent_snapshot_id, candidate_id, revision, status, manifest_hash, canonical_sha256, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'confirmed', ?6, ?7, ?8)",
            params![
                snapshot_id,
                request.project_id,
                project.1,
                request.candidate_id,
                snapshot_revision,
                manifest_hash,
                snapshot_canonical_sha256,
                now,
            ],
        )?;
        let updated_project = transaction.execute(
            "UPDATE projects SET active_snapshot_revision = ?1, head_snapshot_id = ?2, updated_at = ?3 WHERE project_id = ?4 AND active_snapshot_revision = ?5",
            params![snapshot_revision, snapshot_id, now, request.project_id, project.0],
        )?;
        if updated_project != 1 {
            return Err(StoreError::Contract {
                code: "STALE_BASE_VERSION".to_owned(),
                message: "project head changed during confirm".to_owned(),
            });
        }
        transaction.execute(
            "UPDATE candidates SET state = 'confirmed', error_code = NULL, updated_at = ?1 WHERE candidate_id = ?2 AND state = 'reviewable'",
            params![now, request.candidate_id],
        )?;
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: match approval_tool {
                "restore_confirm" => "restore_confirmed",
                "repair_apply_confirm" => "repair_apply_confirmed",
                "cross_view_promotion_confirm" => "cross_view_candidate_confirmed",
                _ => "candidate_confirmed",
            }
            .to_owned(),
            object_id: Some(request.candidate_id.clone()),
            request_sha256: Some(request_sha256.clone()),
            payload: serde_json::json!({
                "candidate_id": request.candidate_id,
                "version_id": version_id,
                "snapshot_id": snapshot_id,
                "approval_receipt_id": approval_receipt_id,
                "prepared_object_sha256": request.prepared_object_sha256,
                "quality_report_id": request.quality_report_id,
                "source_version_id": candidate.source_version_id,
            }),
            created_at: now.to_owned(),
        };
        insert_audit(&transaction, &audit)?;
        let result = CandidateConfirmResult {
            schema_version: "CandidateConfirmResult@1".to_owned(),
            candidate_id: request.candidate_id.clone(),
            project_id: request.project_id.clone(),
            version_id,
            snapshot_id,
            approval_receipt_id,
            request_sha256: request_sha256.clone(),
            replayed: false,
        };
        insert_idempotency(
            &transaction,
            &request.idempotency_key,
            &request.project_id,
            approval_tool,
            &request_sha256,
            &result,
            now,
        )?;
        transaction.commit()?;
        Ok(result)
    }

    pub fn prepare_export(
        &self,
        request: &ExportPrepareRequest,
        now: &str,
    ) -> Result<ExportPrepareResult, StoreError> {
        validate_export_prepare_request(request)?;
        let version =
            self.get_version(&request.version_id)?
                .ok_or_else(|| StoreError::Contract {
                    code: "NOT_FOUND".to_owned(),
                    message: "export version not found".to_owned(),
                })?;
        if version.project_id != request.project_id {
            return Err(StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "export version is outside the requested project".to_owned(),
            });
        }
        let source_candidate =
            self.get_candidate(&version.candidate_id)?
                .ok_or_else(|| StoreError::Contract {
                    code: "NOT_FOUND".to_owned(),
                    message: "export source candidate not found".to_owned(),
                })?;
        if source_candidate.state != "confirmed" || !source_candidate.quality_hard_gate_passed {
            return Err(StoreError::Contract {
                code: "EXPORT_SOURCE_UNCONFIRMED".to_owned(),
                message: "export requires a confirmed quality-passing version".to_owned(),
            });
        }
        let source_object = self.get_object(&version.manifest_hash)?;
        if source_object.is_none() {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "export source manifest object is unavailable".to_owned(),
            });
        }
        if request.format == "glb"
            && source_object
                .as_ref()
                .map(|object| object.mime != "model/gltf-binary")
                .unwrap_or(true)
        {
            return Err(StoreError::Contract {
                code: "EXPORT_FORMAT_UNAVAILABLE".to_owned(),
                message: "mvp-glb export requires a Runtime GLB artifact".to_owned(),
            });
        }
        let export_id = format!("export-{}", Uuid::new_v4().simple());
        let artifact_hashes = vec![version.manifest_hash.clone()];
        let output_kind = if request.format == "glb" {
            "mvp-glb"
        } else {
            "diagnostic-manifest"
        };
        let manifest_payload = serde_json::json!({
            "schema_version": "ExportPayload@1",
            "export_id": export_id,
            "project_id": request.project_id,
            "version_id": request.version_id,
            "format": request.format,
            "profile": request.profile,
            "artifact_hashes": artifact_hashes,
            "license_provenance": {
                "status": if request.format == "glb" { "procedural-mvp" } else { "diagnostic_fixture_unavailable" },
                "absolute_paths": false,
                "source": "runtime-contract-core"
            },
            "toolchain": output_kind
        });
        let manifest_bytes = serde_json::to_vec(&manifest_payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let manifest_object = self.put_object(
            &manifest_bytes,
            None,
            "application/json",
            "export-manifest",
            now,
        )?;
        let request_sha256 = canonical_json_hash(
            &serde_json::to_value(request)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        );
        let manifest = ExportManifestRecord {
            schema_version: "ExportManifest@1".to_owned(),
            export_id: export_id.clone(),
            project_id: request.project_id.clone(),
            version_id: request.version_id.clone(),
            format: request.format.clone(),
            profile: request.profile.clone(),
            manifest_sha256: manifest_object.record.sha256.clone(),
            artifact_hashes: vec![version.manifest_hash.clone()],
            state: "prepared".to_owned(),
            approval_receipt_id: None,
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
        };
        validate_export_manifest(&manifest)?;
        let job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: format!("job-{}", Uuid::new_v4().simple()),
            project_id: request.project_id.clone(),
            kind: "export_prepare".to_owned(),
            status: "succeeded".to_owned(),
            progress: 100,
            request_sha256: request_sha256.clone(),
            checkpoint_sha256: None,
            error_code: None,
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
        };
        let event = JobEventRecord {
            schema_version: "RuntimeJobEvent@1".to_owned(),
            job_id: job.job_id.clone(),
            sequence: 1,
            kind: "export_prepared".to_owned(),
            payload: serde_json::json!({
                "export_id": export_id,
                "version_id": request.version_id,
                "manifest_sha256": manifest.manifest_sha256,
            }),
            created_at: now.to_owned(),
        };
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: "export_prepared".to_owned(),
            object_id: Some(manifest.export_id.clone()),
            request_sha256: Some(request_sha256),
            payload: serde_json::json!({
                "export_id": manifest.export_id,
                "version_id": manifest.version_id,
                "manifest_sha256": manifest.manifest_sha256,
            }),
            created_at: now.to_owned(),
        };
        validate_job(&job)?;
        validate_job_event(&event)?;
        validate_audit(&audit)?;
        let artifact_hashes_json = serde_json::to_string(&manifest.artifact_hashes)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let event_payload = serde_json::to_string(&event.payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO export_manifests (export_id, project_id, version_id, format, profile, manifest_sha256, artifact_hashes_json, state, approval_receipt_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![manifest.export_id, manifest.project_id, manifest.version_id, manifest.format, manifest.profile, manifest.manifest_sha256, artifact_hashes_json, manifest.state, manifest.approval_receipt_id, manifest.created_at, manifest.updated_at],
        )?;
        transaction.execute(
            "INSERT INTO runtime_jobs (job_id, project_id, kind, status, progress, request_sha256, checkpoint_sha256, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![job.job_id, job.project_id, job.kind, job.status, i64::from(job.progress), job.request_sha256, job.checkpoint_sha256, job.error_code, job.created_at, job.updated_at],
        )?;
        transaction.execute(
            "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![event.job_id, event.sequence, event.kind, event_payload, event.created_at],
        )?;
        insert_audit(&transaction, &audit)?;
        transaction.commit()?;
        drop(connection);
        let job_summary = self
            .get_job(&job.job_id)?
            .ok_or_else(|| StoreError::InvalidData("export job disappeared".to_owned()))?;
        Ok(ExportPrepareResult {
            schema_version: "ExportPrepareResult@1".to_owned(),
            manifest,
            job: job_summary,
        })
    }

    pub fn confirm_export(
        &self,
        request: &ExportConfirmRequest,
        now: &str,
    ) -> Result<ExportConfirmResult, StoreError> {
        validate_export_confirm_request(request)?;
        let request_sha256 = canonical_json_hash(
            &serde_json::to_value(request)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        );
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        if let Some((project_id, tool, stored_hash, response_json)) = transaction
            .query_row(
                "SELECT project_id, tool, request_sha256, response_json FROM write_idempotency WHERE idempotency_key = ?1",
                params![request.idempotency_key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)),
            )
            .optional()?
        {
            if project_id != request.project_id
                || tool != "export_confirm"
                || stored_hash != request_sha256
            {
                return Err(StoreError::Contract {
                    code: "IDEMPOTENCY_KEY_REUSED".to_owned(),
                    message: "idempotency key is bound to a different request".to_owned(),
                });
            }
            let mut result: ExportConfirmResult = serde_json::from_str(&response_json)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            result.replayed = true;
            return Ok(result);
        }
        let manifest =
            read_export_for_transaction(&transaction, &request.export_id)?.ok_or_else(|| {
                StoreError::Contract {
                    code: "NOT_FOUND".to_owned(),
                    message: "export manifest not found".to_owned(),
                }
            })?;
        if manifest.project_id != request.project_id
            || manifest.version_id != request.version_id
            || manifest.format != request.format
            || manifest.profile != request.profile
        {
            return Err(StoreError::Contract {
                code: "EXPORT_HASH_MISMATCH".to_owned(),
                message: "export request does not match prepared manifest".to_owned(),
            });
        }
        if manifest.state != "prepared" {
            return Err(StoreError::Contract {
                code: "EXPORT_STATE_INVALID".to_owned(),
                message: "export manifest is not awaiting confirmation".to_owned(),
            });
        }
        let object_exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM objects WHERE sha256 = ?1",
                params![manifest.manifest_sha256],
                |row| row.get(0),
            )
            .optional()?;
        if object_exists.is_none() {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "prepared export manifest object is unavailable".to_owned(),
            });
        }
        let approval_receipt_id = generated_approval_receipt_id();
        if is_expired(now, &request.approval_expires_at)? {
            let approval = approval_record(
                request.project_id.as_str(),
                "export_confirm",
                approval_receipt_id.as_str(),
                Some(request.version_id.as_str()),
                request.export_id.as_str(),
                manifest.manifest_sha256.as_str(),
                None,
                request.approval_summary.as_str(),
                "expired",
                request.approval_expires_at.as_str(),
                request.approval_session_id.as_str(),
                now,
            )?;
            insert_approval(&transaction, &approval)?;
            transaction.commit()?;
            return Err(StoreError::Contract {
                code: "APPROVAL_EXPIRED".to_owned(),
                message: "approval receipt expired before export confirm".to_owned(),
            });
        }
        let approval = approval_record(
            request.project_id.as_str(),
            "export_confirm",
            approval_receipt_id.as_str(),
            Some(request.version_id.as_str()),
            request.export_id.as_str(),
            manifest.manifest_sha256.as_str(),
            None,
            request.approval_summary.as_str(),
            "approved",
            request.approval_expires_at.as_str(),
            request.approval_session_id.as_str(),
            now,
        )?;
        insert_approval(&transaction, &approval)?;
        for artifact_hash in &manifest.artifact_hashes {
            let marked_reachable = transaction.execute(
                "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
                params![artifact_hash],
            )?;
            if marked_reachable != 1 {
                return Err(StoreError::Contract {
                    code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                    message: "export artifact disappeared before confirm".to_owned(),
                });
            }
        }
        let marked_manifest_reachable = transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
            params![manifest.manifest_sha256],
        )?;
        if marked_manifest_reachable != 1 {
            return Err(StoreError::Contract {
                code: "REFERENCE_TRANSFER_UNAVAILABLE".to_owned(),
                message: "prepared export manifest disappeared before confirm".to_owned(),
            });
        }
        transaction.execute(
            "UPDATE export_manifests SET state = 'confirmed', approval_receipt_id = ?1, updated_at = ?2 WHERE export_id = ?3 AND state = 'prepared'",
            params![approval_receipt_id, now, request.export_id],
        )?;
        let output_sha256 = if manifest.format == "glb" {
            manifest
                .artifact_hashes
                .first()
                .cloned()
                .unwrap_or_else(|| manifest.manifest_sha256.clone())
        } else {
            manifest.manifest_sha256.clone()
        };
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: "export_confirmed".to_owned(),
            object_id: Some(request.export_id.clone()),
            request_sha256: Some(request_sha256.clone()),
            payload: serde_json::json!({
                "export_id": request.export_id,
                "version_id": request.version_id,
                "manifest_sha256": manifest.manifest_sha256,
                "output_sha256": output_sha256.clone(),
                "approval_receipt_id": approval_receipt_id,
            }),
            created_at: now.to_owned(),
        };
        insert_audit(&transaction, &audit)?;
        let result = ExportConfirmResult {
            schema_version: "ExportConfirmResult@1".to_owned(),
            export_id: request.export_id.clone(),
            project_id: request.project_id.clone(),
            version_id: request.version_id.clone(),
            manifest_sha256: manifest.manifest_sha256.clone(),
            output_sha256,
            approval_receipt_id,
            request_sha256: request_sha256.clone(),
            replayed: false,
        };
        insert_idempotency(
            &transaction,
            &request.idempotency_key,
            &request.project_id,
            "export_confirm",
            &request_sha256,
            &result,
            now,
        )?;
        transaction.commit()?;
        Ok(result)
    }

    pub fn reject_candidate(
        &self,
        request: &CandidateRejectRequest,
        now: &str,
    ) -> Result<CandidateRejectResult, StoreError> {
        validate_reject_request(request)?;
        let request_value = serde_json::to_value(request)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let request_sha256 = canonical_json_hash(&request_value);
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        if let Some((project_id, tool, stored_hash, response_json)) = transaction
            .query_row(
                "SELECT project_id, tool, request_sha256, response_json FROM write_idempotency WHERE idempotency_key = ?1",
                params![request.idempotency_key],
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
        {
            if project_id != request.project_id
                || tool != "candidate_reject"
                || stored_hash != request_sha256
            {
                return Err(StoreError::Contract {
                    code: "IDEMPOTENCY_KEY_REUSED".to_owned(),
                    message: "idempotency key is bound to a different request".to_owned(),
                });
            }
            let mut result: CandidateRejectResult = serde_json::from_str(&response_json)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            result.replayed = true;
            return Ok(result);
        }
        let candidate = read_candidate_for_transaction(&transaction, &request.candidate_id)?
            .ok_or_else(|| StoreError::Contract {
                code: "NOT_FOUND".to_owned(),
                message: "candidate not found".to_owned(),
            })?;
        if candidate.project_id != request.project_id {
            return Err(StoreError::Contract {
                code: "PROJECT_SCOPE_DENIED".to_owned(),
                message: "candidate is outside the requested project".to_owned(),
            });
        }
        if candidate.state == "confirmed" {
            return Err(StoreError::Contract {
                code: "CANDIDATE_ALREADY_CONFIRMED".to_owned(),
                message: "confirmed candidate cannot be rejected".to_owned(),
            });
        }
        let prepared_object_id =
            candidate
                .prepared_object_id
                .clone()
                .ok_or_else(|| StoreError::Contract {
                    code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                    message: "candidate has no prepared object".to_owned(),
                })?;
        let prepared_object_sha256 =
            candidate
                .prepared_object_sha256
                .clone()
                .ok_or_else(|| StoreError::Contract {
                    code: "CANDIDATE_HASH_MISMATCH".to_owned(),
                    message: "candidate has no prepared object hash".to_owned(),
                })?;
        let approval_receipt_id = generated_approval_receipt_id();
        let approval = approval_record(
            request.project_id.as_str(),
            "candidate_reject",
            approval_receipt_id.as_str(),
            candidate.base_version_id.as_deref(),
            prepared_object_id.as_str(),
            prepared_object_sha256.as_str(),
            candidate.quality_report_id.as_deref(),
            request.approval_summary.as_str(),
            "rejected",
            request.approval_expires_at.as_str(),
            request.approval_session_id.as_str(),
            now,
        )?;
        insert_approval(&transaction, &approval)?;
        transaction.execute(
            "UPDATE candidates SET state = 'rejected', error_code = NULL, updated_at = ?1 WHERE candidate_id = ?2",
            params![now, request.candidate_id],
        )?;
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(request.project_id.clone()),
            kind: "candidate_rejected".to_owned(),
            object_id: Some(request.candidate_id.clone()),
            request_sha256: Some(request_sha256.clone()),
            payload: serde_json::json!({
                "candidate_id": request.candidate_id,
                "approval_receipt_id": approval_receipt_id,
            }),
            created_at: now.to_owned(),
        };
        insert_audit(&transaction, &audit)?;
        let result = CandidateRejectResult {
            schema_version: "CandidateRejectResult@1".to_owned(),
            candidate_id: request.candidate_id.clone(),
            project_id: request.project_id.clone(),
            state: "rejected".to_owned(),
            approval_receipt_id,
            request_sha256: request_sha256.clone(),
            replayed: false,
        };
        insert_idempotency(
            &transaction,
            &request.idempotency_key,
            &request.project_id,
            "candidate_reject",
            &request_sha256,
            &result,
            now,
        )?;
        transaction.commit()?;
        Ok(result)
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
    ensure_column(&transaction, "candidates", "prepared_object_id", "TEXT")?;
    ensure_column(&transaction, "candidates", "prepared_object_sha256", "TEXT")?;
    ensure_column(&transaction, "candidates", "source_version_id", "TEXT")?;
    ensure_column(&transaction, "candidates", "quality_report_id", "TEXT")?;
    ensure_column(
        &transaction,
        "candidates",
        "quality_hard_gate_passed",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        &transaction,
        "approval_receipts",
        "tool",
        "TEXT NOT NULL DEFAULT 'candidate_confirm'",
    )?;
    ensure_column(&transaction, "approval_receipts", "base_version_id", "TEXT")?;
    ensure_column(
        &transaction,
        "approval_receipts",
        "summary_sha256",
        &format!("TEXT NOT NULL DEFAULT '{}'", "0".repeat(64)),
    )?;
    ensure_column(
        &transaction,
        "approval_receipts",
        "session_id",
        "TEXT NOT NULL DEFAULT 'legacy-migration'",
    )?;
    ensure_approval_tools_schema(&transaction)?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS geometry_candidate_evidence (
             candidate_id TEXT PRIMARY KEY REFERENCES candidates(candidate_id),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             reference_id TEXT REFERENCES reference_evidence(reference_id),
             reference_sha256 TEXT,
             geometry_program_sha256 TEXT NOT NULL,
             geometry_program_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             operator_catalog_sha256 TEXT NOT NULL,
             readback_config_sha256 TEXT NOT NULL,
             artifact_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             artifact_readback_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             quality_report_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             quality_report_id TEXT NOT NULL,
             canonical_sha256 TEXT NOT NULL,
             created_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS geometry_candidate_evidence_project_idx
             ON geometry_candidate_evidence(project_id, candidate_id);",
    )?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS visual_evidence (
             candidate_id TEXT PRIMARY KEY REFERENCES candidates(candidate_id),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             reference_id TEXT NOT NULL REFERENCES reference_evidence(reference_id),
             target_sha256 TEXT REFERENCES objects(sha256),
             render_set_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             comparison_report_object_sha256 TEXT,
             visual_review_object_sha256 TEXT REFERENCES objects(sha256),
             quality_report_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             human_receipt_object_sha256 TEXT REFERENCES objects(sha256),
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS visual_evidence_project_idx
             ON visual_evidence(project_id, candidate_id);",
    )?;
    ensure_column(
        &transaction,
        "visual_evidence",
        "visual_review_object_sha256",
        "TEXT",
    )?;
    ensure_column(&transaction, "visual_evidence", "target_sha256", "TEXT")?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS visual_evidence_views (
             candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             view_id TEXT NOT NULL,
             reference_id TEXT NOT NULL REFERENCES reference_evidence(reference_id),
             reference_sha256 TEXT NOT NULL,
             camera_hash TEXT NOT NULL,
             render_set_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             comparison_report_object_sha256 TEXT REFERENCES objects(sha256),
             quality_report_object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             quality_status TEXT NOT NULL,
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL,
             PRIMARY KEY(candidate_id, view_id)
         );
         CREATE INDEX IF NOT EXISTS visual_evidence_views_project_idx
             ON visual_evidence_views(project_id, candidate_id, view_id);
         CREATE TABLE IF NOT EXISTS cross_view_evidence (
             bundle_object_sha256 TEXT PRIMARY KEY REFERENCES objects(sha256),
             candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             session_id TEXT NOT NULL,
             reference_canvas_sha256 TEXT NOT NULL,
             aggregate_status TEXT NOT NULL,
             hard_gate_passed INTEGER NOT NULL CHECK (hard_gate_passed IN (0, 1)),
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS cross_view_evidence_candidate_idx
             ON cross_view_evidence(candidate_id, updated_at DESC, bundle_object_sha256 ASC);",
    )?;
    // Agentic session/checkpoint state is an additive Runtime V1 extension.
    // It creates no rows from existing projects and never opens or migrates a
    // legacy database (that gate ran before this transaction).
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS agentic_design_sessions (
             session_id TEXT PRIMARY KEY,
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             revision INTEGER NOT NULL CHECK (revision >= 0),
             candidate_state_sha256 TEXT NOT NULL,
             reference_id TEXT NOT NULL REFERENCES reference_evidence(reference_id),
             reference_sha256 TEXT NOT NULL,
             camera_hash TEXT NOT NULL,
             evidence_sha256 TEXT NOT NULL,
             current_checkpoint_id TEXT,
             current_checkpoint_sha256 TEXT,
             object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             canonical_sha256 TEXT NOT NULL,
             payload_json TEXT NOT NULL,
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS agentic_design_checkpoints (
             checkpoint_id TEXT PRIMARY KEY,
             session_id TEXT NOT NULL REFERENCES agentic_design_sessions(session_id),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             revision INTEGER NOT NULL CHECK (revision >= 0),
             candidate_state_sha256 TEXT NOT NULL,
             artifact_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             reference_id TEXT NOT NULL REFERENCES reference_evidence(reference_id),
             reference_sha256 TEXT NOT NULL,
             camera_hash TEXT NOT NULL,
             input_sha256 TEXT NOT NULL,
             evidence_sha256 TEXT NOT NULL,
             parent_checkpoint_id TEXT,
             parent_checkpoint_sha256 TEXT,
             object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             canonical_sha256 TEXT NOT NULL,
             payload_json TEXT NOT NULL,
             created_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS agentic_sessions_project_idx
             ON agentic_design_sessions(project_id, updated_at DESC, session_id ASC);
         CREATE INDEX IF NOT EXISTS agentic_checkpoints_session_idx
             ON agentic_design_checkpoints(session_id, created_at ASC, checkpoint_id ASC);
         CREATE INDEX IF NOT EXISTS agentic_checkpoints_binding_idx
             ON agentic_design_checkpoints(project_id, candidate_id, revision ASC, checkpoint_id ASC);
         CREATE TABLE IF NOT EXISTS agentic_action_runs (
             run_id TEXT PRIMARY KEY,
             session_id TEXT NOT NULL REFERENCES agentic_design_sessions(session_id),
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             candidate_id TEXT NOT NULL REFERENCES candidates(candidate_id),
             input_sha256 TEXT NOT NULL,
             status TEXT NOT NULL,
             completed_stage TEXT NOT NULL,
             stage_results_json TEXT NOT NULL,
             checkpoint_id TEXT,
             checkpoint_sha256 TEXT,
             immutable INTEGER NOT NULL CHECK (immutable = 1),
             object_sha256 TEXT NOT NULL REFERENCES objects(sha256),
             canonical_sha256 TEXT NOT NULL,
             payload_json TEXT NOT NULL,
             created_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS agentic_action_runs_binding_idx
             ON agentic_action_runs(session_id, project_id, candidate_id, created_at ASC);
         CREATE INDEX IF NOT EXISTS agentic_action_runs_input_idx
             ON agentic_action_runs(input_sha256, run_id);",
    )?;
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

pub fn agentic_session_payload_bytes(record: &AgenticSessionRecord) -> Result<Vec<u8>, StoreError> {
    let mut value =
        serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    value["object_sha256"] = Value::Null;
    canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))
}

pub fn agentic_checkpoint_payload_bytes(
    record: &AgenticCheckpointRecord,
) -> Result<Vec<u8>, StoreError> {
    let mut value =
        serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    value["object_sha256"] = Value::Null;
    canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))
}

pub fn agentic_action_run_payload_bytes(
    record: &AgenticActionRunRecord,
) -> Result<Vec<u8>, StoreError> {
    let mut value =
        serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    value["object_sha256"] = Value::Null;
    canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))
}

pub fn agentic_action_run_canonical_sha256(
    record: &AgenticActionRunRecord,
) -> Result<String, StoreError> {
    let mut value =
        serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    let object = value.as_object_mut().ok_or_else(|| {
        StoreError::InvalidData("agentic action run record is not a JSON object".to_owned())
    })?;
    object.insert("object_sha256".to_owned(), Value::Null);
    object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    Ok(canonical_json_hash(&value))
}

fn agentic_payload_json<T: Serialize>(record: &T) -> Result<String, StoreError> {
    let mut value =
        serde_json::to_value(record).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if let Some(object) = value.as_object_mut() {
        object.insert("object_sha256".to_owned(), Value::Null);
    }
    validate_agentic_metadata(&value, 0)?;
    let bytes =
        canonical_json_bytes(&value).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    if bytes.len() > 256 * 1024 {
        return Err(StoreError::InvalidData(
            "agentic payload exceeds bounded metadata capacity".to_owned(),
        ));
    }
    String::from_utf8(bytes).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn agentic_action_run_payload_json(record: &AgenticActionRunRecord) -> Result<String, StoreError> {
    let bytes = agentic_action_run_payload_bytes(record)?;
    if bytes.len() > 256 * 1024 {
        return Err(StoreError::InvalidData(
            "agentic action run payload exceeds bounded metadata capacity".to_owned(),
        ));
    }
    String::from_utf8(bytes).map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn ensure_agentic_object(
    cas: &CasStore,
    object: &CasObjectRecord,
    expected_sha256: Option<&str>,
    payload: &str,
) -> Result<(), StoreError> {
    if expected_sha256 != Some(object.sha256.as_str())
        || sha256_hex(payload.as_bytes()) != object.sha256
    {
        return Err(StoreError::InvalidData(
            "agentic CAS object hash does not match canonical payload".to_owned(),
        ));
    }
    cas.verify(&object.sha256, object.size_bytes)?;
    Ok(())
}

fn validate_agentic_action_run_object(object: &CasObjectRecord) -> Result<(), StoreError> {
    if object.schema_version != "CasObject@1"
        || !is_sha256(&object.sha256)
        || i64::try_from(object.size_bytes).is_err()
        || object.mime != "application/json"
        || !is_opaque_id(&object.kind)
        || !matches!(object.reachability.as_str(), "temporary" | "reachable")
        || object.created_at.is_empty()
        || object.created_at.len() > 64
    {
        return Err(StoreError::InvalidData(
            "agentic action run CAS metadata is malformed".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_agentic_action_run_object_row(
    transaction: &rusqlite::Transaction<'_>,
    object: &CasObjectRecord,
) -> Result<(), StoreError> {
    let stored: Option<(i64, String, String, String)> = transaction
        .query_row(
            "SELECT size_bytes, mime, kind, reachability FROM objects WHERE sha256 = ?1",
            params![object.sha256],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((size_bytes, mime, kind, reachability)) = stored else {
        return Err(StoreError::Contract {
            code: "AGENTIC_ACTION_RUN_CAS_OBJECT_UNAVAILABLE".to_owned(),
            message: "action run CAS object is not registered in SQLite".to_owned(),
        });
    };
    if size_bytes != i64::try_from(object.size_bytes).unwrap_or(i64::MAX)
        || mime != object.mime
        || kind != object.kind
        || !matches!(reachability.as_str(), "temporary" | "reachable")
    {
        return Err(StoreError::InvalidData(
            "action run CAS metadata differs from SQLite".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_agentic_binding_rows(
    transaction: &rusqlite::Transaction<'_>,
    project_id: &str,
    candidate_id: &str,
    reference_id: &str,
    reference_sha256: &str,
) -> Result<(), StoreError> {
    let candidate_project: Option<String> = transaction
        .query_row(
            "SELECT project_id FROM candidates WHERE candidate_id = ?1",
            params![candidate_id],
            |row| row.get(0),
        )
        .optional()?;
    if candidate_project.as_deref() != Some(project_id) {
        return Err(StoreError::Contract {
            code: "AGENTIC_CANDIDATE_SCOPE_DENIED".to_owned(),
            message: "candidate is not in the requested project".to_owned(),
        });
    }
    let reference: Option<(String, String)> = transaction
        .query_row(
            "SELECT project_id, object_sha256 FROM reference_evidence WHERE reference_id = ?1",
            params![reference_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if reference.as_ref().map(|value| value.0.as_str()) != Some(project_id)
        || reference.as_ref().map(|value| value.1.as_str()) != Some(reference_sha256)
    {
        return Err(StoreError::Contract {
            code: "AGENTIC_REFERENCE_SCOPE_DENIED".to_owned(),
            message: "reference is not bound to the requested project/hash".to_owned(),
        });
    }
    Ok(())
}

fn ensure_agentic_action_run_binding_rows(
    transaction: &rusqlite::Transaction<'_>,
    run: &AgenticActionRunRecord,
) -> Result<(), StoreError> {
    let candidate_project: Option<String> = transaction
        .query_row(
            "SELECT project_id FROM candidates WHERE candidate_id = ?1",
            params![run.candidate_id],
            |row| row.get(0),
        )
        .optional()?;
    if candidate_project.as_deref() != Some(run.project_id.as_str()) {
        return Err(StoreError::Contract {
            code: "AGENTIC_ACTION_RUN_CANDIDATE_SCOPE_DENIED".to_owned(),
            message: "action run candidate is not in the requested project".to_owned(),
        });
    }

    let session: Option<(String, String)> = transaction
        .query_row(
            "SELECT project_id, candidate_id FROM agentic_design_sessions WHERE session_id = ?1",
            params![run.session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((session_project, session_candidate)) = session else {
        return Err(StoreError::Contract {
            code: "AGENTIC_ACTION_RUN_SESSION_NOT_FOUND".to_owned(),
            message: "action run requires a persisted DesignSession".to_owned(),
        });
    };
    if session_project != run.project_id || session_candidate != run.candidate_id {
        return Err(StoreError::Contract {
            code: "AGENTIC_ACTION_RUN_BINDING_MISMATCH".to_owned(),
            message: "action run session, project and candidate bindings differ".to_owned(),
        });
    }

    if let Some(checkpoint_id) = run.checkpoint_id.as_deref() {
        let checkpoint: Option<(String, String, String, String)> = transaction
            .query_row(
                "SELECT session_id, project_id, candidate_id, canonical_sha256 FROM agentic_design_checkpoints WHERE checkpoint_id = ?1",
                params![checkpoint_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        if checkpoint.as_ref().map(|value| {
            value.0 != run.session_id
                || value.1 != run.project_id
                || value.2 != run.candidate_id
                || Some(value.3.as_str()) != run.checkpoint_sha256.as_deref()
        }) != Some(false)
        {
            return Err(StoreError::Contract {
                code: "AGENTIC_ACTION_RUN_CHECKPOINT_MISMATCH".to_owned(),
                message: "action run checkpoint pointer is unavailable or out of scope".to_owned(),
            });
        }
    }
    Ok(())
}

fn ensure_design_action_run_binding_rows(
    transaction: &rusqlite::Transaction<'_>,
    run: &Value,
) -> Result<(), StoreError> {
    let object = run
        .as_object()
        .ok_or_else(|| StoreError::InvalidData("DesignActionRun is not an object".to_owned()))?;
    let project_id = object["project_id"].as_str().ok_or_else(|| {
        StoreError::InvalidData("DesignActionRun project_id is missing".to_owned())
    })?;
    let candidate_id = object["candidate_id"].as_str().ok_or_else(|| {
        StoreError::InvalidData("DesignActionRun candidate_id is missing".to_owned())
    })?;
    let session_id = object["session_id"].as_str().ok_or_else(|| {
        StoreError::InvalidData("DesignActionRun session_id is missing".to_owned())
    })?;
    let candidate_project: Option<String> = transaction
        .query_row(
            "SELECT project_id FROM candidates WHERE candidate_id = ?1",
            params![candidate_id],
            |row| row.get(0),
        )
        .optional()?;
    if candidate_project.as_deref() != Some(project_id) {
        return Err(StoreError::Contract {
            code: "DESIGN_ACTION_RUN_CANDIDATE_SCOPE_DENIED".to_owned(),
            message: "DesignActionRun candidate is not in the requested project".to_owned(),
        });
    }
    let session: Option<(String, String)> = transaction
        .query_row(
            "SELECT project_id, candidate_id FROM agentic_design_sessions WHERE session_id = ?1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((session_project, session_candidate)) = session else {
        return Err(StoreError::Contract {
            code: "DESIGN_ACTION_RUN_SESSION_NOT_FOUND".to_owned(),
            message: "DesignActionRun requires a persisted DesignSession".to_owned(),
        });
    };
    if session_project != project_id || session_candidate != candidate_id {
        return Err(StoreError::Contract {
            code: "DESIGN_ACTION_RUN_BINDING_MISMATCH".to_owned(),
            message: "DesignActionRun session, project and candidate bindings differ".to_owned(),
        });
    }
    if let Some(checkpoint_id) = object.get("checkpoint_id").and_then(Value::as_str) {
        let checkpoint_hash = object
            .get("checkpoint_hash")
            .and_then(Value::as_str)
            .ok_or_else(|| StoreError::Contract {
                code: "DESIGN_ACTION_RUN_CHECKPOINT_MISMATCH".to_owned(),
                message: "DesignActionRun checkpoint hash is required".to_owned(),
            })?;
        let checkpoint: Option<(String, String, String, String)> = transaction
            .query_row(
                "SELECT session_id, project_id, candidate_id, canonical_sha256 FROM agentic_design_checkpoints WHERE checkpoint_id = ?1",
                params![checkpoint_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        if checkpoint.as_ref().map(|value| {
            value.0 != session_id
                || value.1 != project_id
                || value.2 != candidate_id
                || value.3 != checkpoint_hash
        }) != Some(false)
        {
            return Err(StoreError::Contract {
                code: "DESIGN_ACTION_RUN_CHECKPOINT_MISMATCH".to_owned(),
                message: "DesignActionRun checkpoint pointer is unavailable or out of scope"
                    .to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_agentic_metadata(value: &Value, depth: usize) -> Result<(), StoreError> {
    if depth > 12 {
        return Err(StoreError::InvalidData(
            "agentic metadata nesting exceeds the bounded limit".to_owned(),
        ));
    }
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        Value::String(text) => {
            if text.len() > 4096
                || text.starts_with('/')
                || text.starts_with('\\')
                || text.starts_with("file:")
                || text.starts_with("data:")
                || text.starts_with("http:")
                || text.starts_with("https:")
                || text.contains("-----BEGIN")
            {
                return Err(StoreError::InvalidData(
                    "agentic metadata contains a path, URL, secret or oversized string".to_owned(),
                ));
            }
            Ok(())
        }
        Value::Array(values) => {
            if values.len() > 256 {
                return Err(StoreError::InvalidData(
                    "agentic metadata array exceeds the bounded limit".to_owned(),
                ));
            }
            for value in values {
                validate_agentic_metadata(value, depth + 1)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for (key, value) in values {
                let lowered = key.to_ascii_lowercase();
                let forbidden_field = lowered.split(['_', '-', '.']).any(|part| {
                    matches!(
                        part,
                        "path"
                            | "url"
                            | "secret"
                            | "token"
                            | "password"
                            | "script"
                            | "command"
                            | "shell"
                            | "bytes"
                            | "base64"
                            | "prompt"
                    )
                }) || lowered.contains("api_key")
                    || lowered.contains("apikey");
                if forbidden_field {
                    return Err(StoreError::InvalidData(format!(
                        "agentic metadata contains a forbidden field: {key}"
                    )));
                }
                validate_agentic_metadata(value, depth + 1)?;
            }
            Ok(())
        }
    }
}

fn validate_hash_pair(
    id: &Option<String>,
    hash: &Option<String>,
    label: &str,
) -> Result<(), StoreError> {
    match (id, hash) {
        (None, None) => Ok(()),
        (Some(id), Some(hash)) if is_opaque_id(id) && is_sha256(hash) => Ok(()),
        _ => Err(StoreError::InvalidData(format!(
            "agentic {label} id/hash pair is malformed"
        ))),
    }
}

fn validate_agentic_session(session: &AgenticSessionRecord) -> Result<(), StoreError> {
    if session.schema_version != "DesignSession@1"
        || !is_opaque_id(&session.session_id)
        || !is_opaque_id(&session.project_id)
        || !is_opaque_id(&session.candidate_id)
        || session.revision < 0
        || !is_sha256(&session.candidate_state_sha256)
        || !is_opaque_id(&session.design_spec_id)
        || !is_sha256(&session.design_spec_sha256)
        || !is_opaque_id(&session.reference_canvas_id)
        || !is_sha256(&session.reference_canvas_sha256)
        || !is_opaque_id(&session.reference_id)
        || !is_sha256(&session.reference_sha256)
        || !is_sha256(&session.camera_hash)
        || !is_sha256(&session.evidence_sha256)
        || !is_sha256(&session.observation_sha256)
        || !is_opaque_id(&session.current_stage)
        || !is_opaque_id(&session.status)
        || !is_opaque_id(&session.quality_status)
        || session
            .object_sha256
            .as_deref()
            .is_none_or(|hash| !is_sha256(hash))
        || !is_sha256(&session.canonical_sha256)
        || session.created_at.is_empty()
        || session.updated_at.is_empty()
        || !session.stage_gate.is_object()
        || !session.rollback.is_object()
        || session.next_actions.is_empty()
        || session.next_actions.len() > 8
        || session.checkpoint_ids.len() > 128
        || session.checkpoint_ids.iter().any(|id| !is_opaque_id(id))
        || session
            .checkpoint_ids
            .windows(2)
            .any(|pair| pair[0] == pair[1])
    {
        return Err(StoreError::InvalidData(
            "agentic DesignSession record is malformed".to_owned(),
        ));
    }
    validate_hash_pair(
        &session.current_version_id,
        &session.current_version_sha256,
        "current version",
    )?;
    validate_hash_pair(
        &session.current_checkpoint_id,
        &session.current_checkpoint_sha256,
        "current checkpoint",
    )?;
    if let Some(checkpoint_id) = session.current_checkpoint_id.as_deref() {
        if !session.checkpoint_ids.iter().any(|id| id == checkpoint_id) {
            return Err(StoreError::InvalidData(
                "current checkpoint is absent from session history".to_owned(),
            ));
        }
    }
    validate_agentic_metadata(
        &serde_json::to_value(session)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        0,
    )
}

fn validate_agentic_checkpoint(checkpoint: &AgenticCheckpointRecord) -> Result<(), StoreError> {
    if checkpoint.schema_version != "DesignCheckpoint@1"
        || !is_opaque_id(&checkpoint.checkpoint_id)
        || !is_opaque_id(&checkpoint.session_id)
        || !is_opaque_id(&checkpoint.project_id)
        || !is_opaque_id(&checkpoint.candidate_id)
        || checkpoint.revision < 0
        || !is_opaque_id(&checkpoint.stage)
        || !is_opaque_id(&checkpoint.checkpoint_type)
        || !is_sha256(&checkpoint.candidate_state_sha256)
        || !is_sha256(&checkpoint.artifact_sha256)
        || !is_opaque_id(&checkpoint.reference_id)
        || !is_sha256(&checkpoint.reference_sha256)
        || !is_sha256(&checkpoint.camera_hash)
        || !is_sha256(&checkpoint.input_sha256)
        || !is_sha256(&checkpoint.evidence_sha256)
        || !is_sha256(&checkpoint.observation_sha256)
        || !is_sha256(&checkpoint.canonical_sha256)
        || checkpoint
            .object_sha256
            .as_deref()
            .is_none_or(|hash| !is_sha256(hash))
        || !checkpoint.immutable
        || checkpoint.runtime_write
        || !checkpoint.stage_gate.is_object()
        || !checkpoint.rollback.is_object()
        || checkpoint.created_at.is_empty()
        || checkpoint.observed.iter().any(|value| !is_opaque_id(value))
        || checkpoint.inferred.iter().any(|value| !is_opaque_id(value))
        || checkpoint.unknown.iter().any(|value| !is_opaque_id(value))
        || checkpoint
            .failed_gates
            .iter()
            .any(|value| !is_opaque_id(value))
        || checkpoint
            .allowed_actions
            .iter()
            .any(|value| !is_opaque_id(value))
        || checkpoint
            .locked_actions
            .iter()
            .any(|value| !is_opaque_id(value))
    {
        return Err(StoreError::InvalidData(
            "agentic DesignCheckpoint record is malformed".to_owned(),
        ));
    }
    validate_hash_pair(
        &checkpoint.version_id,
        &checkpoint.version_sha256,
        "checkpoint version",
    )?;
    validate_hash_pair(
        &checkpoint.parent_checkpoint_id,
        &checkpoint.parent_checkpoint_sha256,
        "checkpoint parent",
    )?;
    validate_agentic_metadata(
        &serde_json::to_value(checkpoint)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        0,
    )
}

fn validate_agentic_action_run(run: &AgenticActionRunRecord) -> Result<(), StoreError> {
    if run.schema_version != "AgenticActionRun@1"
        || !is_opaque_id(&run.run_id)
        || !is_opaque_id(&run.session_id)
        || !is_opaque_id(&run.project_id)
        || !is_opaque_id(&run.candidate_id)
        || !is_sha256(&run.input_sha256)
        || !matches!(
            run.status.as_str(),
            "queued"
                | "running"
                | "waiting_for_input"
                | "succeeded"
                | "completed"
                | "failed"
                | "cancelled"
                | "blocked"
        )
        || (run.completed_stage != "none" && !is_opaque_id(&run.completed_stage))
        || !run.immutable
        || run
            .object_sha256
            .as_deref()
            .is_some_and(|hash| !is_sha256(hash))
        || !is_sha256(&run.canonical_sha256)
        || run.created_at.is_empty()
        || run.created_at.len() > 64
    {
        return Err(StoreError::InvalidData(
            "agentic action run record is malformed".to_owned(),
        ));
    }
    validate_hash_pair(
        &run.checkpoint_id,
        &run.checkpoint_sha256,
        "action run checkpoint",
    )?;
    validate_agentic_action_stage_results(&run.stage_results)?;
    let value =
        serde_json::to_value(run).map_err(|error| StoreError::InvalidData(error.to_string()))?;
    validate_agentic_metadata(&value, 0)?;
    validate_agentic_action_run_text(&value, 0)?;
    let expected_canonical_sha256 = agentic_action_run_canonical_sha256(run)?;
    if run.canonical_sha256 != expected_canonical_sha256 {
        return Err(StoreError::Contract {
            code: "AGENTIC_ACTION_RUN_CANONICAL_HASH_MISMATCH".to_owned(),
            message: "action run canonical hash does not match its immutable fields".to_owned(),
        });
    }
    Ok(())
}

fn validate_design_action_run_value(value: &Value) -> Result<(), StoreError> {
    const REQUIRED: &[&str] = &[
        "schema_version",
        "run_id",
        "session_id",
        "project_id",
        "candidate_id",
        "reference_id",
        "reference_sha256",
        "camera_hash",
        "input_sha256",
        "observation_sha256",
        "action",
        "requested_stage",
        "status",
        "completed_stage",
        "stage_results",
        "quality_status",
        "failed_gates",
        "allowed_actions",
        "locked_actions",
        "checkpoint_id",
        "checkpoint_hash",
        "optimization_job_id",
        "optimization_intent_sha256",
        "runtime_write",
        "persistent_user_data_touched",
        "canonical_sha256",
    ];
    let object = value
        .as_object()
        .ok_or_else(|| StoreError::InvalidData("DesignActionRun must be an object".to_owned()))?;
    if object
        .keys()
        .any(|key| !REQUIRED.contains(&key.as_str()) && key != "proposal")
        || REQUIRED.iter().any(|key| !object.contains_key(*key))
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun has missing or unknown fields".to_owned(),
        ));
    }
    if object.get("schema_version").and_then(Value::as_str) != Some("DesignActionRun@1") {
        return Err(StoreError::InvalidData(
            "DesignActionRun schema version is unsupported".to_owned(),
        ));
    }
    for key in [
        "run_id",
        "session_id",
        "project_id",
        "candidate_id",
        "reference_id",
    ] {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| !is_opaque_id(value))
        {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun {key} is malformed"
            )));
        }
    }
    for key in [
        "reference_sha256",
        "camera_hash",
        "input_sha256",
        "observation_sha256",
        "canonical_sha256",
    ] {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun {key} is not a SHA-256"
            )));
        }
    }
    if object.get("runtime_write") != Some(&Value::Bool(false))
        || object.get("persistent_user_data_touched") != Some(&Value::Bool(false))
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun must remain Runtime-write and persistent-data false".to_owned(),
        ));
    }
    if object
        .get("requested_stage")
        .and_then(Value::as_str)
        .is_none_or(|stage| {
            !matches!(
                stage,
                "reference-canvas"
                    | "primary-form"
                    | "secondary-structure"
                    | "tertiary-detail"
                    | "uv-pbr"
                    | "final-review"
            )
        })
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun requested_stage is unsupported".to_owned(),
        ));
    }
    if object
        .get("status")
        .and_then(Value::as_str)
        .is_none_or(|status| {
            !matches!(
                status,
                "prepared" | "running" | "completed" | "failed" | "blocked"
            )
        })
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun status is unsupported".to_owned(),
        ));
    }
    if let Some(stage) = object.get("completed_stage").and_then(Value::as_str) {
        if !matches!(
            stage,
            "prepare" | "compile" | "readback" | "render" | "evaluate"
        ) {
            return Err(StoreError::InvalidData(
                "DesignActionRun completed_stage is unsupported".to_owned(),
            ));
        }
    } else if !object.get("completed_stage").is_some_and(Value::is_null) {
        return Err(StoreError::InvalidData(
            "DesignActionRun completed_stage must be null or a pipeline stage".to_owned(),
        ));
    }
    if object
        .get("quality_status")
        .and_then(Value::as_str)
        .is_none_or(|status| {
            !matches!(
                status,
                "PARTIAL_VISIBLE_VIEW_PASS"
                    | "QUALITY_TARGET_NOT_MET"
                    | "BLOCKED_REFERENCE_COVERAGE"
                    | "not-run"
                    | "unknown"
            )
        })
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun quality_status is unsupported".to_owned(),
        ));
    }
    validate_design_action_array(object, "failed_gates", true)?;
    validate_design_action_array(object, "allowed_actions", true)?;
    validate_design_action_array(object, "locked_actions", true)?;
    let allowed = object["allowed_actions"]
        .as_array()
        .expect("validated action array");
    let locked = object["locked_actions"]
        .as_array()
        .expect("validated action array");
    if allowed
        .iter()
        .any(|value| matches!(value.as_str(), Some("confirm" | "export")))
        || !locked.iter().any(|value| value.as_str() == Some("confirm"))
        || !locked.iter().any(|value| value.as_str() == Some("export"))
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun approval locks are not fail-closed".to_owned(),
        ));
    }
    let checkpoint_id = object.get("checkpoint_id").expect("required checkpoint_id");
    let checkpoint_hash = object
        .get("checkpoint_hash")
        .expect("required checkpoint_hash");
    if checkpoint_id.is_null() != checkpoint_hash.is_null()
        || checkpoint_hash
            .as_str()
            .is_some_and(|value| !is_sha256(value))
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun checkpoint id/hash must be paired".to_owned(),
        ));
    }
    let optimization_job_id = object
        .get("optimization_job_id")
        .expect("required optimization_job_id");
    let optimization_intent_sha256 = object
        .get("optimization_intent_sha256")
        .expect("required optimization_intent_sha256");
    if optimization_job_id.is_null() != optimization_intent_sha256.is_null()
        || (!optimization_job_id.is_null()
            && !optimization_job_id.as_str().is_some_and(is_opaque_id))
        || (!optimization_intent_sha256.is_null()
            && !optimization_intent_sha256.as_str().is_some_and(is_sha256))
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun optimization job/id hash must be paired".to_owned(),
        ));
    }
    let action = object
        .get("action")
        .and_then(Value::as_object)
        .ok_or_else(|| StoreError::InvalidData("DesignActionRun action is missing".to_owned()))?;
    if action
        .get("action_id")
        .and_then(Value::as_str)
        .is_none_or(|value| !is_opaque_id(value))
        || action.get("bounded") != Some(&Value::Bool(true))
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun action is not bounded".to_owned(),
        ));
    }
    let stages = object
        .get("stage_results")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            StoreError::InvalidData("DesignActionRun stage_results is missing".to_owned())
        })?;
    const PIPELINE: &[&str] = &["prepare", "compile", "readback", "render", "evaluate"];
    if stages.len() != PIPELINE.len() || PIPELINE.iter().any(|stage| !stages.contains_key(*stage)) {
        return Err(StoreError::InvalidData(
            "DesignActionRun stage_results must contain exactly five pipeline stages".to_owned(),
        ));
    }
    for stage in PIPELINE {
        let result = stages[*stage].as_object().ok_or_else(|| {
            StoreError::InvalidData(format!("DesignActionRun {stage} result is malformed"))
        })?;
        if result
            .keys()
            .any(|key| !matches!(key.as_str(), "status" | "hash" | "reason"))
            || !result.contains_key("status")
            || !result.contains_key("hash")
            || !result.contains_key("reason")
        {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun {stage} result fields are invalid"
            )));
        }
        let status = result["status"].as_str().ok_or_else(|| {
            StoreError::InvalidData(format!("DesignActionRun {stage} status is missing"))
        })?;
        if !matches!(
            status,
            "not-run" | "running" | "completed" | "failed" | "blocked"
        ) {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun {stage} status is unsupported"
            )));
        }
        if result["hash"].as_str().is_some_and(|hash| !is_sha256(hash))
            || (!result["hash"].is_null() && result["hash"].as_str().is_none())
        {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun {stage} hash is malformed"
            )));
        }
        let reason = result["reason"].as_str();
        if matches!(status, "not-run" | "running" | "failed" | "blocked")
            && reason.is_none_or(str::is_empty)
        {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun {stage} reason is required"
            )));
        }
        if status == "completed" && result["hash"].is_null() {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun {stage} completed result needs a hash"
            )));
        }
        if let Some(reason) = reason {
            if reason.len() > 512 || reason.contains("://") || reason.starts_with('/') {
                return Err(StoreError::InvalidData(format!(
                    "DesignActionRun {stage} reason contains unsafe text"
                )));
            }
        } else if !result["reason"].is_null() {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun {stage} reason must be text or null"
            )));
        }
    }
    validate_agentic_metadata(value, 0)?;
    validate_agentic_action_run_text(value, 0)?;
    if let Some(proposal) = object.get("proposal") {
        validate_design_action_proposal_value(proposal)?;
    }
    let mut canonical = value.clone();
    canonical["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical) != object["canonical_sha256"].as_str().unwrap_or_default() {
        return Err(StoreError::Contract {
            code: "DESIGN_ACTION_RUN_CANONICAL_HASH_MISMATCH".to_owned(),
            message: "DesignActionRun canonical hash does not match immutable fields".to_owned(),
        });
    }
    Ok(())
}

fn validate_design_action_proposal_value(value: &Value) -> Result<(), StoreError> {
    const REQUIRED: &[&str] = &[
        "intent_sha256",
        "intent_object_sha256",
        "candidate_id",
        "candidate_state_sha256",
        "artifact_sha256",
        "geometry_program_sha256",
        "render_set_sha256",
        "comparison_report_sha256",
        "quality_report_sha256",
        "visual_status",
        "baseline_score",
        "proposal_score",
        "strict_improvement",
        "non_regressing",
        "promotion",
        "confirm_allowed",
    ];
    let object = value.as_object().ok_or_else(|| {
        StoreError::InvalidData("DesignActionRun proposal must be an object".to_owned())
    })?;
    if object
        .keys()
        .any(|key| !REQUIRED.contains(&key.as_str()) && key != "cross_view_evidence_sha256")
        || REQUIRED.iter().any(|key| !object.contains_key(*key))
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun proposal has missing or unknown fields".to_owned(),
        ));
    }
    for key in ["candidate_id"] {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| !is_opaque_id(value))
        {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun proposal {key} is malformed"
            )));
        }
    }
    for key in [
        "intent_sha256",
        "intent_object_sha256",
        "candidate_state_sha256",
        "artifact_sha256",
        "geometry_program_sha256",
        "render_set_sha256",
        "comparison_report_sha256",
        "quality_report_sha256",
    ] {
        if object
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| !is_sha256(value))
        {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun proposal {key} is not a SHA-256"
            )));
        }
    }
    if !matches!(
        object.get("visual_status").and_then(Value::as_str),
        Some("PARTIAL_VISIBLE_VIEW_PASS" | "QUALITY_TARGET_NOT_MET" | "BLOCKED_REFERENCE_COVERAGE")
    ) || !matches!(
        object.get("promotion").and_then(Value::as_str),
        Some("reviewable" | "not-improved" | "rejected-regression")
    ) || object.get("confirm_allowed") != Some(&Value::Bool(false))
    {
        return Err(StoreError::InvalidData(
            "DesignActionRun proposal gate fields are invalid".to_owned(),
        ));
    }
    for key in ["baseline_score", "proposal_score"] {
        let score = object.get(key).and_then(Value::as_f64).ok_or_else(|| {
            StoreError::InvalidData(format!("DesignActionRun proposal {key} is invalid"))
        })?;
        if !score.is_finite() || !(0.0..=8.0).contains(&score) {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun proposal {key} is out of bounds"
            )));
        }
    }
    for key in ["strict_improvement", "non_regressing"] {
        if !object.get(key).is_some_and(Value::is_boolean) {
            return Err(StoreError::InvalidData(format!(
                "DesignActionRun proposal {key} must be boolean"
            )));
        }
    }
    if let Some(value) = object.get("cross_view_evidence_sha256") {
        if !value.is_null() && value.as_str().is_none_or(|value| !is_sha256(value)) {
            return Err(StoreError::InvalidData(
                "DesignActionRun proposal cross_view_evidence_sha256 is invalid".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_design_action_array(
    object: &serde_json::Map<String, Value>,
    key: &str,
    non_empty: bool,
) -> Result<(), StoreError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| StoreError::InvalidData(format!("DesignActionRun {key} is not an array")))?;
    if non_empty && values.is_empty() && key == "locked_actions" {
        return Err(StoreError::InvalidData(format!(
            "DesignActionRun {key} is empty"
        )));
    }
    if values.len() > 24
        || values.iter().any(|value| {
            value
                .as_str()
                .is_none_or(|value| !is_opaque_id(value) || value.len() > 64)
        })
    {
        return Err(StoreError::InvalidData(format!(
            "DesignActionRun {key} contains an invalid capability"
        )));
    }
    Ok(())
}

fn validate_agentic_action_stage_results(value: &Value) -> Result<(), StoreError> {
    match value {
        Value::Object(stages) => {
            if stages.len() > 32 {
                return Err(StoreError::InvalidData(
                    "agentic action run has too many stage results".to_owned(),
                ));
            }
            for (stage, result) in stages {
                if !is_opaque_id(stage) {
                    return Err(StoreError::InvalidData(
                        "agentic action run stage id is malformed".to_owned(),
                    ));
                }
                validate_agentic_action_stage_result(result, false)?;
            }
            Ok(())
        }
        Value::Array(results) => {
            if results.len() > 32 {
                return Err(StoreError::InvalidData(
                    "agentic action run has too many stage results".to_owned(),
                ));
            }
            let mut stage_ids = std::collections::BTreeSet::new();
            for result in results {
                let stage = result
                    .as_object()
                    .and_then(|object| object.get("stage"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        StoreError::InvalidData(
                            "array action stage result must include a stage id".to_owned(),
                        )
                    })?;
                if !stage_ids.insert(stage.to_owned()) {
                    return Err(StoreError::InvalidData(
                        "agentic action stage results contain a duplicate stage".to_owned(),
                    ));
                }
                validate_agentic_action_stage_result(result, true)?;
            }
            Ok(())
        }
        _ => Err(StoreError::InvalidData(
            "agentic action stage_results must be an object or array".to_owned(),
        )),
    }
}

fn validate_agentic_action_stage_result(
    value: &Value,
    requires_stage: bool,
) -> Result<(), StoreError> {
    const ALLOWED_FIELDS: &[&str] = &[
        "stage",
        "status",
        "result_sha256",
        "output_sha256",
        "summary_sha256",
        "error_code",
        "checkpoint_id",
        "checkpoint_sha256",
        "canonical_sha256",
        "completed_at",
    ];
    let object = value.as_object().ok_or_else(|| {
        StoreError::InvalidData("agentic action stage result must be an object".to_owned())
    })?;
    for key in object.keys() {
        if !ALLOWED_FIELDS.contains(&key.as_str()) {
            return Err(StoreError::InvalidData(format!(
                "unknown agentic action stage result field: {key}"
            )));
        }
    }
    if requires_stage {
        let stage = object
            .get("stage")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| {
                StoreError::InvalidData("agentic action stage id is malformed".to_owned())
            })?;
        if stage.is_empty() {
            return Err(StoreError::InvalidData(
                "agentic action stage id is empty".to_owned(),
            ));
        }
    } else if object.contains_key("stage") {
        return Err(StoreError::InvalidData(
            "object action stage result must not repeat its stage id".to_owned(),
        ));
    }
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            StoreError::InvalidData("agentic action stage result status is required".to_owned())
        })?;
    if !matches!(
        status,
        "queued"
            | "running"
            | "waiting_for_input"
            | "succeeded"
            | "completed"
            | "failed"
            | "cancelled"
            | "blocked"
            | "skipped"
    ) {
        return Err(StoreError::InvalidData(
            "agentic action stage result status is unsupported".to_owned(),
        ));
    }
    for field in [
        "result_sha256",
        "output_sha256",
        "summary_sha256",
        "checkpoint_sha256",
        "canonical_sha256",
    ] {
        if let Some(value) = object.get(field) {
            if value.as_str().is_none_or(|hash| !is_sha256(hash)) {
                return Err(StoreError::InvalidData(format!(
                    "agentic action stage result field {field} is not a SHA-256"
                )));
            }
        }
    }
    if let Some(value) = object.get("checkpoint_id") {
        if value.as_str().is_none_or(|id| !is_opaque_id(id)) {
            return Err(StoreError::InvalidData(
                "agentic action stage checkpoint id is malformed".to_owned(),
            ));
        }
        if object.get("checkpoint_sha256").is_none() {
            return Err(StoreError::InvalidData(
                "agentic action stage checkpoint hash is required with its id".to_owned(),
            ));
        }
    } else if object.contains_key("checkpoint_sha256") {
        return Err(StoreError::InvalidData(
            "agentic action stage checkpoint id is required with its hash".to_owned(),
        ));
    }
    if let Some(value) = object.get("error_code") {
        if value.as_str().is_none_or(|code| !is_opaque_id(code)) {
            return Err(StoreError::InvalidData(
                "agentic action stage error code is malformed".to_owned(),
            ));
        }
    }
    if let Some(value) = object.get("completed_at") {
        if value
            .as_str()
            .is_none_or(|timestamp| timestamp.is_empty() || timestamp.len() > 64)
        {
            return Err(StoreError::InvalidData(
                "agentic action stage completed_at is malformed".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_agentic_action_run_text(value: &Value, depth: usize) -> Result<(), StoreError> {
    if depth > 12 {
        return Err(StoreError::InvalidData(
            "agentic action metadata nesting exceeds the bounded limit".to_owned(),
        ));
    }
    match value {
        Value::String(text) => {
            let lowered = text.to_ascii_lowercase();
            if lowered.contains("://")
                || lowered.starts_with("javascript:")
                || lowered.starts_with("ftp:")
                || lowered.starts_with("bearer ")
                || lowered.starts_with("sk-")
                || lowered.contains("-----begin")
            {
                return Err(StoreError::InvalidData(
                    "agentic action metadata contains a URL, secret or executable reference"
                        .to_owned(),
                ));
            }
            Ok(())
        }
        Value::Array(values) => {
            for value in values {
                validate_agentic_action_run_text(value, depth + 1)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for value in values.values() {
                validate_agentic_action_run_text(value, depth + 1)?;
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
    }
}

fn validate_geometry_candidate_evidence(
    evidence: &GeometryCandidateEvidenceRecord,
) -> Result<(), StoreError> {
    if evidence.schema_version != "GeometryCandidateEvidence@1"
        || !is_opaque_id(&evidence.candidate_id)
        || !is_opaque_id(&evidence.project_id)
        || !is_opaque_id(&evidence.quality_report_id)
        || evidence.created_at.is_empty()
        || evidence.created_at.len() > 64
        || !is_sha256(&evidence.geometry_program_sha256)
        || !is_sha256(&evidence.geometry_program_object_sha256)
        || !is_sha256(&evidence.operator_catalog_sha256)
        || !is_sha256(&evidence.readback_config_sha256)
        || !is_sha256(&evidence.artifact_object_sha256)
        || !is_sha256(&evidence.artifact_readback_object_sha256)
        || !is_sha256(&evidence.quality_report_object_sha256)
        || !is_sha256(&evidence.canonical_sha256)
    {
        return Err(StoreError::InvalidData(
            "geometry candidate evidence is malformed".to_owned(),
        ));
    }
    match (&evidence.reference_id, &evidence.reference_sha256) {
        (None, None) => Ok(()),
        (Some(reference_id), Some(reference_sha256))
            if is_opaque_id(reference_id) && is_sha256(reference_sha256) =>
        {
            Ok(())
        }
        _ => Err(StoreError::InvalidData(
            "geometry candidate reference evidence is malformed".to_owned(),
        )),
    }
}

fn ensure_column(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), StoreError> {
    let exists: Option<String> = transaction
        .query_row(
            "SELECT name FROM pragma_table_info(?1) WHERE name = ?2",
            params![table, column],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        let statement = format!(
            "ALTER TABLE {} ADD COLUMN {} {}",
            quote_identifier(table),
            quote_identifier(column),
            definition
        );
        transaction.execute(&statement, [])?;
    }
    Ok(())
}

fn ensure_approval_tools_schema(transaction: &rusqlite::Transaction<'_>) -> Result<(), StoreError> {
    let sql: Option<String> = transaction
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'approval_receipts'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(sql) = sql else {
        return Err(StoreError::InvalidData(
            "approval_receipts table is missing after migration".to_owned(),
        ));
    };
    let normalized = sql.to_ascii_lowercase();
    if normalized.contains("restore_confirm")
        && normalized.contains("export_confirm")
        && normalized.contains("repair_apply_confirm")
        && normalized.contains("cross_view_promotion_confirm")
    {
        return Ok(());
    }
    transaction.execute_batch(
        "ALTER TABLE approval_receipts RENAME TO approval_receipts_legacy;
         CREATE TABLE approval_receipts (
             approval_receipt_id TEXT PRIMARY KEY,
             project_id TEXT NOT NULL REFERENCES projects(project_id),
             tool TEXT NOT NULL CHECK (tool IN ('candidate_confirm', 'candidate_reject', 'restore_confirm', 'export_confirm', 'repair_apply_confirm', 'cross_view_promotion_confirm')),
             base_version_id TEXT,
             prepared_object_id TEXT NOT NULL,
             prepared_object_sha256 TEXT NOT NULL,
             quality_report_id TEXT,
             summary_sha256 TEXT NOT NULL,
             decision TEXT NOT NULL CHECK (decision IN ('approved', 'rejected', 'expired')),
             expires_at TEXT NOT NULL,
             session_id TEXT NOT NULL,
             created_at TEXT NOT NULL
         );
         INSERT INTO approval_receipts (approval_receipt_id, project_id, tool, base_version_id, prepared_object_id, prepared_object_sha256, quality_report_id, summary_sha256, decision, expires_at, session_id, created_at)
         SELECT approval_receipt_id, project_id, tool, base_version_id, prepared_object_id, prepared_object_sha256, quality_report_id, summary_sha256, decision, expires_at, session_id, created_at
         FROM approval_receipts_legacy;
         DROP TABLE approval_receipts_legacy;",
    )?;
    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn read_candidate_for_transaction(
    transaction: &rusqlite::Transaction<'_>,
    candidate_id: &str,
) -> Result<Option<CandidateRecord>, StoreError> {
    Ok(transaction
        .query_row(
            "SELECT candidate_id, project_id, base_version_id, source_version_id, prepared_object_id, prepared_object_sha256, state, request_sha256, manifest_hash, quality_report_id, quality_hard_gate_passed, canonical_sha256, error_code, created_at, updated_at FROM candidates WHERE candidate_id = ?1",
            params![candidate_id],
            |row| {
                Ok(CandidateRecord {
                    schema_version: "Candidate@1".to_owned(),
                    candidate_id: row.get(0)?,
                    project_id: row.get(1)?,
                    base_version_id: row.get(2)?,
                    source_version_id: row.get(3)?,
                    prepared_object_id: row.get(4)?,
                    prepared_object_sha256: row.get(5)?,
                    state: row.get(6)?,
                    request_sha256: row.get(7)?,
                    manifest_hash: row.get(8)?,
                    quality_report_id: row.get(9)?,
                    quality_hard_gate_passed: row.get::<_, i64>(10)? != 0,
                    canonical_sha256: row.get(11)?,
                    error_code: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            },
        )
        .optional()?)
}

fn read_job_summary(
    transaction: &rusqlite::Transaction<'_>,
    job_id: &str,
) -> Result<JobSummary, StoreError> {
    let (job_id, project_id, kind, status, progress, error_code, created_at, updated_at) =
        transaction.query_row(
            "SELECT job_id, project_id, kind, status, progress, error_code, created_at, updated_at FROM runtime_jobs WHERE job_id = ?1",
            params![job_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )?;
    Ok(JobSummary {
        job_id,
        project_id,
        kind,
        status,
        progress: u8::try_from(progress)
            .map_err(|_| StoreError::InvalidData("job progress outside u8".to_owned()))?,
        error_code,
        created_at,
        updated_at,
    })
}

fn read_job_record_from_transaction(
    transaction: &rusqlite::Transaction<'_>,
    job_id: &str,
) -> Result<Option<JobRecord>, StoreError> {
    Ok(transaction
        .query_row(
            "SELECT job_id, project_id, kind, status, progress, request_sha256, checkpoint_sha256, error_code, created_at, updated_at FROM runtime_jobs WHERE job_id = ?1",
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
                Ok(JobRecord {
                    schema_version: "RuntimeJob@1".to_owned(),
                    job_id: row.get(0)?,
                    project_id: row.get(1)?,
                    kind: row.get(2)?,
                    status: row.get(3)?,
                    progress,
                    request_sha256: row.get(5)?,
                    checkpoint_sha256: row.get(6)?,
                    error_code: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )
        .optional()?)
}

// A bounded optimization checkpoint can reference the coarse candidate
// programs/artifacts, one evaluation object per real geometry evaluation, all
// final AOV objects, the intent/result objects, and the prior checkpoint.  The
// v4 funnel (48 coarse + 8 mid + 2 proposal finalists + baseline) stays below
// this ceiling while preserving the generic Job reachability guard.
const MAX_JOB_REACHABILITY_HASHES: usize = 256;

fn validate_reachable_hashes(hashes: &[String]) -> Result<(), StoreError> {
    if hashes.len() > MAX_JOB_REACHABILITY_HASHES || hashes.iter().any(|hash| !is_sha256(hash)) {
        return Err(StoreError::InvalidData(
            "job reachability list is invalid or exceeds its bound".to_owned(),
        ));
    }
    Ok(())
}

fn mark_reachable_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    hashes: &[String],
) -> Result<(), StoreError> {
    for hash in hashes {
        let updated = transaction.execute(
            "UPDATE objects SET reachability = 'reachable' WHERE sha256 = ?1",
            params![hash],
        )?;
        if updated != 1 {
            return Err(StoreError::Contract {
                code: "JOB_CAS_OBJECT_UNAVAILABLE".to_owned(),
                message: "job references a CAS object that is not registered".to_owned(),
            });
        }
    }
    Ok(())
}

fn read_export_for_transaction(
    transaction: &rusqlite::Transaction<'_>,
    export_id: &str,
) -> Result<Option<ExportManifestRecord>, StoreError> {
    let record = transaction
        .query_row(
            "SELECT export_id, project_id, version_id, format, profile, manifest_sha256, artifact_hashes_json, state, approval_receipt_id, created_at, updated_at FROM export_manifests WHERE export_id = ?1",
            params![export_id],
            |row| {
                let artifact_hashes_json: String = row.get(6)?;
                let artifact_hashes = serde_json::from_str(&artifact_hashes_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(ExportManifestRecord {
                    schema_version: "ExportManifest@1".to_owned(),
                    export_id: row.get(0)?,
                    project_id: row.get(1)?,
                    version_id: row.get(2)?,
                    format: row.get(3)?,
                    profile: row.get(4)?,
                    manifest_sha256: row.get(5)?,
                    artifact_hashes,
                    state: row.get(7)?,
                    approval_receipt_id: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            },
        )
        .optional()?;
    if let Some(record) = record.as_ref() {
        validate_export_manifest(record)?;
    }
    Ok(record)
}

fn generated_approval_receipt_id() -> String {
    format!("receipt-{}", Uuid::new_v4().simple())
}

fn approval_record(
    project_id: &str,
    tool: &str,
    approval_receipt_id: &str,
    base_version_id: Option<&str>,
    prepared_object_id: &str,
    prepared_object_sha256: &str,
    quality_report_id: Option<&str>,
    summary: &str,
    decision: &str,
    expires_at: &str,
    session_id: &str,
    created_at: &str,
) -> Result<ApprovalReceiptRecord, StoreError> {
    let record = ApprovalReceiptRecord {
        schema_version: "ApprovalReceipt@1".to_owned(),
        approval_receipt_id: approval_receipt_id.to_owned(),
        project_id: project_id.to_owned(),
        tool: tool.to_owned(),
        base_version_id: base_version_id.map(str::to_owned),
        prepared_object_id: prepared_object_id.to_owned(),
        prepared_object_sha256: prepared_object_sha256.to_owned(),
        quality_report_id: quality_report_id.map(str::to_owned),
        summary_sha256: canonical_json_hash(&serde_json::json!(summary)),
        decision: decision.to_owned(),
        expires_at: expires_at.to_owned(),
        session_id: session_id.to_owned(),
        created_at: created_at.to_owned(),
    };
    validate_approval(&record)?;
    Ok(record)
}

fn insert_approval(
    transaction: &rusqlite::Transaction<'_>,
    approval: &ApprovalReceiptRecord,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO approval_receipts (approval_receipt_id, project_id, tool, base_version_id, prepared_object_id, prepared_object_sha256, quality_report_id, summary_sha256, decision, expires_at, session_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            approval.approval_receipt_id,
            approval.project_id,
            approval.tool,
            approval.base_version_id,
            approval.prepared_object_id,
            approval.prepared_object_sha256,
            approval.quality_report_id,
            approval.summary_sha256,
            approval.decision,
            approval.expires_at,
            approval.session_id,
            approval.created_at,
        ],
    )?;
    Ok(())
}

fn insert_audit(
    transaction: &rusqlite::Transaction<'_>,
    audit: &AuditEventRecord,
) -> Result<(), StoreError> {
    validate_audit(audit)?;
    let payload = serde_json::to_string(&audit.payload)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    transaction.execute(
        "INSERT INTO audit_events (audit_id, project_id, kind, object_id, request_sha256, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            audit.audit_id,
            audit.project_id,
            audit.kind,
            audit.object_id,
            audit.request_sha256,
            payload,
            audit.created_at,
        ],
    )?;
    Ok(())
}

fn insert_idempotency<T: serde::Serialize>(
    transaction: &rusqlite::Transaction<'_>,
    key: &str,
    project_id: &str,
    tool: &str,
    request_sha256: &str,
    response: &T,
    created_at: &str,
) -> Result<(), StoreError> {
    let response_json = serde_json::to_string(response)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    transaction.execute(
        "INSERT INTO write_idempotency (idempotency_key, project_id, tool, request_sha256, response_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![key, project_id, tool, request_sha256, response_json, created_at],
    )?;
    Ok(())
}

fn is_expired(now: &str, expires_at: &str) -> Result<bool, StoreError> {
    let now = now.parse::<i64>().map_err(|_| {
        StoreError::InvalidData(
            "approval timestamps must use UTC epoch seconds in Runtime V1".to_owned(),
        )
    })?;
    let expires_at = expires_at.parse::<i64>().map_err(|_| {
        StoreError::InvalidData(
            "approval expiry must use UTC epoch seconds in Runtime V1".to_owned(),
        )
    })?;
    Ok(expires_at <= now)
}

fn read_reference_evidence(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReferenceEvidenceRecord> {
    let size_bytes: i64 = row.get(4)?;
    let width: i64 = row.get(5)?;
    let height: i64 = row.get(6)?;
    let frame_count: i64 = row.get(7)?;
    let authorization_json: String = row.get(9)?;
    let authorization: ReferenceAuthorization =
        serde_json::from_str(&authorization_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                9,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    Ok(ReferenceEvidenceRecord {
        schema_version: "ReferenceEvidence@1".to_owned(),
        reference_id: row.get(0)?,
        project_id: row.get(1)?,
        object_sha256: row.get(2)?,
        mime: row.get(3)?,
        size_bytes: u64::try_from(size_bytes).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Integer,
                "negative reference size".into(),
            )
        })?,
        width: u32::try_from(width).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Integer,
                "invalid reference width".into(),
            )
        })?,
        height: u32::try_from(height).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Integer,
                "invalid reference height".into(),
            )
        })?,
        frame_count: u32::try_from(frame_count).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Integer,
                "invalid reference frame count".into(),
            )
        })?,
        import_mode: row.get(8)?,
        authorization,
        derived_object_sha256: row.get(10)?,
        canonical_sha256: row.get(11)?,
        created_at: row.get(12)?,
    })
}

fn validate_reference_evidence(reference: &ReferenceEvidenceRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&reference.reference_id)
        || !is_opaque_id(&reference.project_id)
        || !is_sha256(&reference.object_sha256)
        || !matches!(reference.mime.as_str(), "image/png" | "image/jpeg")
        || reference.size_bytes == 0
        || reference.width == 0
        || reference.height == 0
        || reference.frame_count != 1
        || !matches!(
            reference.import_mode.as_str(),
            "inline_content" | "codex_local_file"
        )
        || !reference.authorization.user_authorized
        || reference.authorization.declaration.trim().is_empty()
        || reference.authorization.declaration.len() > 512
        || reference
            .derived_object_sha256
            .as_deref()
            .is_some_and(|value| !is_sha256(value))
        || !is_sha256(&reference.canonical_sha256)
    {
        return Err(StoreError::InvalidData(
            "invalid reference evidence record".to_owned(),
        ));
    }
    Ok(())
}

fn validate_confirm_request(request: &CandidateConfirmRequest) -> Result<(), StoreError> {
    if !is_opaque_id(&request.project_id)
        || !is_opaque_id(&request.candidate_id)
        || request
            .base_version_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || !is_opaque_id(&request.prepared_object_id)
        || !is_sha256(&request.prepared_object_sha256)
        || !is_opaque_id(&request.quality_report_id)
        || !is_opaque_id(&request.approval_receipt_id)
        || request.approval_summary.trim().is_empty()
        || request.approval_summary.len() > 4096
        || !is_opaque_id(&request.approval_session_id)
        || !is_opaque_id(&request.idempotency_key)
    {
        return Err(StoreError::InvalidData(
            "invalid candidate confirm request".to_owned(),
        ));
    }
    Ok(())
}

fn validate_reject_request(request: &CandidateRejectRequest) -> Result<(), StoreError> {
    if !is_opaque_id(&request.project_id)
        || !is_opaque_id(&request.candidate_id)
        || !is_opaque_id(&request.approval_receipt_id)
        || request.approval_summary.trim().is_empty()
        || request.approval_summary.len() > 4096
        || !is_opaque_id(&request.approval_session_id)
        || !is_opaque_id(&request.idempotency_key)
    {
        return Err(StoreError::InvalidData(
            "invalid candidate reject request".to_owned(),
        ));
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

fn validate_candidate(candidate: &CandidateRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&candidate.candidate_id)
        || !is_opaque_id(&candidate.project_id)
        || !is_sha256(&candidate.request_sha256)
        || candidate
            .prepared_object_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || candidate
            .prepared_object_sha256
            .as_deref()
            .is_some_and(|value| !is_sha256(value))
        || candidate
            .quality_report_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || candidate
            .base_version_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || candidate
            .source_version_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || candidate
            .manifest_hash
            .as_deref()
            .is_some_and(|value| !is_sha256(value))
        || !is_sha256(&candidate.canonical_sha256)
    {
        return Err(StoreError::InvalidData(
            "invalid candidate record".to_owned(),
        ));
    }
    Ok(())
}

fn validate_job(job: &JobRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&job.job_id)
        || !is_opaque_id(&job.project_id)
        || job.kind.trim().is_empty()
        || !matches!(
            job.status.as_str(),
            "queued" | "running" | "waiting_for_input" | "succeeded" | "failed" | "cancelled"
        )
        || job.progress > 100
        || !is_sha256(&job.request_sha256)
    {
        return Err(StoreError::InvalidData("invalid job record".to_owned()));
    }
    Ok(())
}

fn validate_job_event(event: &JobEventRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&event.job_id)
        || event.sequence <= 0
        || event.kind.trim().is_empty()
        || !event.payload.is_object()
    {
        return Err(StoreError::InvalidData("invalid job event".to_owned()));
    }
    Ok(())
}

fn validate_audit(event: &AuditEventRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&event.audit_id)
        || event.kind.trim().is_empty()
        || event
            .project_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || event
            .object_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || event
            .request_sha256
            .as_deref()
            .is_some_and(|value| !is_sha256(value))
    {
        return Err(StoreError::InvalidData("invalid audit event".to_owned()));
    }
    Ok(())
}

fn validate_approval(approval: &ApprovalReceiptRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&approval.approval_receipt_id)
        || !is_opaque_id(&approval.project_id)
        || !matches!(
            approval.tool.as_str(),
            "candidate_confirm"
                | "candidate_reject"
                | "restore_confirm"
                | "export_confirm"
                | "repair_apply_confirm"
                | "cross_view_promotion_confirm"
        )
        || approval
            .base_version_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || !is_opaque_id(&approval.prepared_object_id)
        || !is_sha256(&approval.prepared_object_sha256)
        || approval
            .quality_report_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || !is_sha256(&approval.summary_sha256)
        || !matches!(
            approval.decision.as_str(),
            "approved" | "rejected" | "expired"
        )
        || !is_opaque_id(&approval.session_id)
    {
        return Err(StoreError::InvalidData(
            "invalid approval receipt".to_owned(),
        ));
    }
    Ok(())
}

fn validate_restore_prepare_request(request: &RestorePrepareRequest) -> Result<(), StoreError> {
    if !is_opaque_id(&request.project_id)
        || request
            .base_version_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || !is_opaque_id(&request.source_version_id)
        || !request.request.is_object()
    {
        return Err(StoreError::InvalidData(
            "invalid restore prepare request".to_owned(),
        ));
    }
    Ok(())
}

fn validate_restore_confirm_request(request: &RestoreConfirmRequest) -> Result<(), StoreError> {
    if !is_opaque_id(&request.source_version_id)
        || !is_opaque_id(&request.project_id)
        || !is_opaque_id(&request.candidate_id)
        || request
            .base_version_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
        || !is_opaque_id(&request.prepared_object_id)
        || !is_sha256(&request.prepared_object_sha256)
        || !is_opaque_id(&request.quality_report_id)
        || !is_opaque_id(&request.approval_receipt_id)
        || request.approval_summary.trim().is_empty()
        || request.approval_summary.len() > 4096
        || !is_opaque_id(&request.approval_session_id)
        || !is_opaque_id(&request.idempotency_key)
    {
        return Err(StoreError::InvalidData(
            "invalid restore confirm request".to_owned(),
        ));
    }
    Ok(())
}

fn validate_export_prepare_request(request: &ExportPrepareRequest) -> Result<(), StoreError> {
    if !is_opaque_id(&request.project_id)
        || !is_opaque_id(&request.version_id)
        || !matches!(request.format.as_str(), "manifest-json" | "glb")
        || !is_opaque_id(&request.profile)
        || !((request.format == "manifest-json" && request.profile == "diagnostic")
            || (request.format == "glb" && request.profile == "mvp-glb"))
        || !request.request.is_object()
    {
        return Err(StoreError::InvalidData(
            "only the diagnostic manifest-json export profile is available".to_owned(),
        ));
    }
    Ok(())
}

fn validate_export_confirm_request(request: &ExportConfirmRequest) -> Result<(), StoreError> {
    if !is_opaque_id(&request.project_id)
        || !is_opaque_id(&request.export_id)
        || !is_opaque_id(&request.version_id)
        || !matches!(request.format.as_str(), "manifest-json" | "glb")
        || !((request.format == "manifest-json" && request.profile == "diagnostic")
            || (request.format == "glb" && request.profile == "mvp-glb"))
        || !is_opaque_id(&request.approval_receipt_id)
        || request.approval_summary.trim().is_empty()
        || request.approval_summary.len() > 4096
        || !is_opaque_id(&request.approval_session_id)
        || !is_opaque_id(&request.idempotency_key)
    {
        return Err(StoreError::InvalidData(
            "invalid export confirm request".to_owned(),
        ));
    }
    Ok(())
}

fn validate_export_manifest(manifest: &ExportManifestRecord) -> Result<(), StoreError> {
    if !is_opaque_id(&manifest.export_id)
        || !is_opaque_id(&manifest.project_id)
        || !is_opaque_id(&manifest.version_id)
        || !matches!(manifest.format.as_str(), "manifest-json" | "glb")
        || !is_opaque_id(&manifest.profile)
        || !((manifest.format == "manifest-json" && manifest.profile == "diagnostic")
            || (manifest.format == "glb" && manifest.profile == "mvp-glb"))
        || !is_sha256(&manifest.manifest_sha256)
        || manifest.artifact_hashes.is_empty()
        || manifest.artifact_hashes.iter().any(|hash| !is_sha256(hash))
        || !matches!(
            manifest.state.as_str(),
            "prepared" | "confirmed" | "rejected" | "failed"
        )
        || manifest
            .approval_receipt_id
            .as_deref()
            .is_some_and(|value| !is_opaque_id(value))
    {
        return Err(StoreError::InvalidData(
            "invalid export manifest record".to_owned(),
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
    fn cancelling_a_non_terminal_job_is_durable_and_has_no_version_side_effect() {
        let store = Store::memory().expect("store");
        store
            .insert_project(&project("project-job-cancel"))
            .expect("project");
        let job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: "job-cancel".to_owned(),
            project_id: "project-job-cancel".to_owned(),
            kind: "candidate_compile".to_owned(),
            status: "queued".to_owned(),
            progress: 0,
            request_sha256: "a".repeat(64),
            checkpoint_sha256: None,
            error_code: None,
            created_at: "1".to_owned(),
            updated_at: "1".to_owned(),
        };
        store.insert_job(&job).expect("job");
        let cancelled = store.cancel_job("job-cancel", "2").expect("cancel");
        assert_eq!(cancelled.status, "cancelled");
        assert_eq!(cancelled.error_code.as_deref(), Some("JOB_CANCELLED"));
        assert_eq!(store.list_job_events("job-cancel", 0).unwrap().len(), 1);
        assert!(store
            .list_versions(Some("project-job-cancel"))
            .unwrap()
            .is_empty());
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
}
