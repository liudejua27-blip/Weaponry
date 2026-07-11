PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- The first durable Concept worker slice is intentionally limited to the
-- immutable geometry inspector. Other Concept endpoints remain synchronous
-- compatibility flows until their side effects are split into resumable steps.
CREATE TABLE concept_job_work_items (
  job_id TEXT PRIMARY KEY REFERENCES concept_jobs(job_id) ON DELETE CASCADE,
  task_type TEXT NOT NULL CHECK (task_type = 'inspect_quality'),
  payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
  retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
  lease_owner TEXT,
  lease_expires_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_concept_job_work_items_lease
  ON concept_job_work_items(lease_expires_at, updated_at);

INSERT INTO schema_migrations(version, name)
VALUES ('0018', 'concept_quality_job_queue');

COMMIT;
