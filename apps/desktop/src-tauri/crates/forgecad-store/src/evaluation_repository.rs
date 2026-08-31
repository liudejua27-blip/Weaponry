//! Physical Store boundary for the Evaluation-owned Job aggregate.
//!
//! A Job is a durable row in `runtime_jobs` together with its ordered event
//! stream and optional checkpoint rows.  This module moves the public Job
//! read/write API behind one borrowed repository without changing the SQL
//! schema, transaction ordering, CAS reachability, replay rules, or the
//! existing `Store` method surface.
//!
//! `EvaluationRepository` deliberately borrows [`Store`].  It owns no
//! connection, migration sequence, CAS root, or independent recovery state.
//! The `Store` methods at the bottom are compatibility shims for existing
//! Runtime callers; new Evaluation services can use the repository directly.

use super::{validate_job, validate_job_event, validate_reachable_hashes, Store, StoreError};
use forgecad_contracts::is_opaque_id;
pub use forgecad_contracts::{JobEventRecord, JobRecord, JobSummary};
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

/// Borrowed Evaluation repository for the coherent Runtime Job aggregate.
#[derive(Clone, Copy)]
pub struct EvaluationRepository<'store> {
    store: &'store Store,
}

/// Compatibility name for callers that want to identify the extracted
/// aggregate directly. Both names refer to the same borrowed repository.
pub type JobRepository<'store> = EvaluationRepository<'store>;

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

/// Install a Job row and its first event into a caller-owned transaction.
///
/// Cross-domain Store operations use this narrow primitive when a candidate,
/// evidence, export, or restore row must be committed atomically with its Job.
/// The caller performs contract validation and event serialization before
/// entering the transaction; this helper only centralizes the Job-owned SQL
/// while preserving the caller's connection, transaction, and rollback scope.
pub(crate) fn insert_job_and_event_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    job: &JobRecord,
    event: &JobEventRecord,
    payload_json: &str,
    updated_at: &str,
) -> Result<(), StoreError> {
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
            updated_at,
        ],
    )?;
    transaction.execute(
        "INSERT INTO runtime_job_events (job_id, sequence, kind, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![event.job_id, event.sequence, event.kind, payload_json, event.created_at],
    )?;
    Ok(())
}

impl<'store> EvaluationRepository<'store> {
    pub(crate) fn new(store: &'store Store) -> Self {
        Self { store }
    }

    /// Read the compact Job projection used by list/status callers.
    pub fn get_job(&self, job_id: &str) -> Result<Option<JobSummary>, StoreError> {
        let connection = self.store.lock_connection()?;
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

    /// Read the full durable Job row.
    pub fn get_job_record(&self, job_id: &str) -> Result<Option<JobRecord>, StoreError> {
        if !is_opaque_id(job_id) {
            return Err(StoreError::InvalidData("invalid job id".to_owned()));
        }
        let connection = self.store.lock_connection()?;
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

    /// Atomically create one durable Job and its first event. A repeated Job
    /// id is an idempotent read only when its project/kind/request binding is
    /// exact.
    pub fn insert_job_with_event(
        &self,
        job: &JobRecord,
        event: &JobEventRecord,
    ) -> Result<(), StoreError> {
        self.insert_job_with_event_if_absent(job, event, &[])?;
        Ok(())
    }

    /// Compatibility terminal update for Runtime-owned bounded jobs.
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

    /// Create a Job/event pair and mark any supplied CAS roots reachable in
    /// the same SQLite transaction.
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
        let mut connection = self.store.lock_connection()?;
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
        super::mark_reachable_in_transaction(&transaction, reachable_hashes)?;
        transaction.commit()?;
        drop(connection);
        self.get_job_record(&job.job_id)?.ok_or_else(|| {
            StoreError::InvalidData("job disappeared after atomic create".to_owned())
        })
    }

    /// Advance a durable Job and append its event/checkpoint in one SQLite
    /// transaction. Every referenced CAS object becomes reachable before the
    /// new state is visible to readers.
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
        let mut connection = self.store.lock_connection()?;
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
        super::mark_reachable_in_transaction(&transaction, reachable_hashes)?;
        transaction.commit()?;
        drop(connection);
        self.get_job_record(&job.job_id)?.ok_or_else(|| {
            StoreError::InvalidData("job disappeared after atomic update".to_owned())
        })
    }

    /// Claim a queued Job exactly once and append the started event.
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
        let mut connection = self.store.lock_connection()?;
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

    /// Requeue a recoverable Job while preserving its prior checkpoint and
    /// appending a new recovery event.
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
        let mut connection = self.store.lock_connection()?;
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

    /// Insert a standalone Job row for legacy callers. New code should use
    /// `insert_job_with_event_if_absent` so the initial event is atomic.
    pub fn insert_job(&self, job: &JobRecord) -> Result<(), StoreError> {
        validate_job(job)?;
        let connection = self.store.lock_connection()?;
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

    /// Cancel a non-terminal Job and append the cancellation event.
    pub fn cancel_job(&self, job_id: &str, updated_at: &str) -> Result<JobSummary, StoreError> {
        if !is_opaque_id(job_id) {
            return Err(StoreError::InvalidData("invalid job id".to_owned()));
        }
        let mut connection = self.store.lock_connection()?;
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

    /// Read the ordered event stream after an optional sequence cursor.
    pub fn list_job_events(
        &self,
        job_id: &str,
        after_sequence: i64,
    ) -> Result<Vec<JobEventRecord>, StoreError> {
        let connection = self.store.lock_connection()?;
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
}

impl Store {
    /// Borrow the Evaluation-owned Job repository.
    ///
    /// Construction is side-effect free; `Store::migrate` remains the single
    /// migration owner and the repository uses this Store's connection/CAS.
    pub fn evaluation_repository(&self) -> EvaluationRepository<'_> {
        EvaluationRepository::new(self)
    }

    /// Explicit Job-named alias for the extracted aggregate.
    pub fn job_repository(&self) -> JobRepository<'_> {
        self.evaluation_repository()
    }

    pub fn get_job(&self, job_id: &str) -> Result<Option<JobSummary>, StoreError> {
        self.evaluation_repository().get_job(job_id)
    }

    pub fn get_job_record(&self, job_id: &str) -> Result<Option<JobRecord>, StoreError> {
        self.evaluation_repository().get_job_record(job_id)
    }

    pub fn insert_job_with_event(
        &self,
        job: &JobRecord,
        event: &JobEventRecord,
    ) -> Result<(), StoreError> {
        self.evaluation_repository()
            .insert_job_with_event(job, event)
    }

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
        self.evaluation_repository().finish_job_with_event(
            job_id, status, progress, error_code, event_kind, payload, updated_at,
        )
    }

    pub fn insert_job_with_event_if_absent(
        &self,
        job: &JobRecord,
        event: &JobEventRecord,
        reachable_hashes: &[String],
    ) -> Result<JobRecord, StoreError> {
        self.evaluation_repository()
            .insert_job_with_event_if_absent(job, event, reachable_hashes)
    }

    pub fn update_job_with_event(
        &self,
        job: &JobRecord,
        event_kind: &str,
        event_payload: &Value,
        reachable_hashes: &[String],
    ) -> Result<JobRecord, StoreError> {
        self.evaluation_repository().update_job_with_event(
            job,
            event_kind,
            event_payload,
            reachable_hashes,
        )
    }

    pub fn claim_job_running(
        &self,
        job_id: &str,
        updated_at: &str,
        payload: &Value,
    ) -> Result<Option<JobRecord>, StoreError> {
        self.evaluation_repository()
            .claim_job_running(job_id, updated_at, payload)
    }

    pub fn requeue_job(
        &self,
        job_id: &str,
        updated_at: &str,
        payload: &Value,
    ) -> Result<JobRecord, StoreError> {
        self.evaluation_repository()
            .requeue_job(job_id, updated_at, payload)
    }

    pub fn insert_job(&self, job: &JobRecord) -> Result<(), StoreError> {
        self.evaluation_repository().insert_job(job)
    }

    pub fn cancel_job(&self, job_id: &str, updated_at: &str) -> Result<JobSummary, StoreError> {
        self.evaluation_repository().cancel_job(job_id, updated_at)
    }

    pub fn list_job_events(
        &self,
        job_id: &str,
        after_sequence: i64,
    ) -> Result<Vec<JobEventRecord>, StoreError> {
        self.evaluation_repository()
            .list_job_events(job_id, after_sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_is_borrowed_and_empty_job_lookup_is_side_effect_free() {
        let store = Store::memory().expect("store");
        let repository = store.evaluation_repository();
        assert!(std::ptr::eq(repository.store, &store));
        assert!(repository.get_job("job-missing").expect("lookup").is_none());
        assert!(repository
            .get_job_record("job-missing")
            .expect("record lookup")
            .is_none());
        assert!(repository
            .list_job_events("job-missing", 0)
            .expect("event lookup")
            .is_empty());
    }

    #[test]
    fn repository_rejects_invalid_job_before_writing() {
        let store = Store::memory().expect("store");
        let repository = store.job_repository();
        let error = repository
            .insert_job(&JobRecord {
                schema_version: "RuntimeJob@1".to_owned(),
                job_id: String::new(),
                project_id: "project".to_owned(),
                kind: "test".to_owned(),
                status: "queued".to_owned(),
                progress: 0,
                request_sha256: "a".repeat(64),
                checkpoint_sha256: None,
                error_code: None,
                created_at: "1".to_owned(),
                updated_at: "1".to_owned(),
            })
            .expect_err("invalid id");
        assert!(matches!(error, StoreError::InvalidData(_)));
        let connection = store.lock_connection().expect("connection");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM runtime_jobs", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 0);
    }
}
