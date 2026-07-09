PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE INDEX IF NOT EXISTS idx_jobs_updated_cursor
ON generation_jobs(updated_at DESC, job_id DESC);

CREATE INDEX IF NOT EXISTS idx_jobs_status_updated_cursor
ON generation_jobs(status, updated_at DESC, job_id DESC);

CREATE INDEX IF NOT EXISTS idx_jobs_type_updated_cursor
ON generation_jobs(job_type, updated_at DESC, job_id DESC);

CREATE INDEX IF NOT EXISTS idx_jobs_error_updated_cursor
ON generation_jobs(error_code, updated_at DESC, job_id DESC)
WHERE error_code IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_job_actions_created_cursor
ON job_actions(created_at DESC, action_id DESC);

CREATE INDEX IF NOT EXISTS idx_job_actions_job_created_cursor
ON job_actions(job_id, created_at DESC, action_id DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0007', 'p0_job_history_indexes');

COMMIT;
