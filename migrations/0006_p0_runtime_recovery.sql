PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

ALTER TABLE generation_jobs ADD COLUMN runner_id TEXT;
ALTER TABLE generation_jobs ADD COLUMN lease_expires_at TEXT;
ALTER TABLE generation_jobs ADD COLUMN checkpoint_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE generation_jobs ADD COLUMN cancel_requested_at TEXT;
ALTER TABLE generation_jobs ADD COLUMN cancel_provider_attempted_at TEXT;

ALTER TABLE job_steps ADD COLUMN provider_task_id TEXT;
ALTER TABLE job_steps ADD COLUMN checkpoint_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE job_steps ADD COLUMN resumable_after_restart INTEGER NOT NULL DEFAULT 0 CHECK (resumable_after_restart IN (0, 1));
ALTER TABLE job_steps ADD COLUMN cancel_state TEXT
  CHECK (cancel_state IS NULL OR cancel_state IN (
    'none', 'cancel_requested', 'provider_cancel_attempted',
    'provider_cancelled', 'provider_cancel_unsupported'
  ));

CREATE TABLE IF NOT EXISTS provider_tasks (
  task_record_id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES generation_jobs(job_id) ON DELETE CASCADE,
  step_name TEXT NOT NULL,
  attempt INTEGER NOT NULL DEFAULT 1 CHECK (attempt >= 1),
  provider_kind TEXT NOT NULL
    CHECK (provider_kind IN ('llm', 'image', 'three_d', 'asset_store', 'quality_checker', 'unity')),
  provider_id TEXT NOT NULL,
  provider_task_id TEXT,
  status TEXT NOT NULL
    CHECK (status IN ('submitted', 'polling', 'cancel_requested', 'cancelled', 'succeeded', 'failed', 'unknown')),
  cancel_requested_at TEXT,
  last_seen_at TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (job_id, step_name, attempt, provider_task_id)
);

CREATE TABLE IF NOT EXISTS job_checkpoints (
  checkpoint_id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES generation_jobs(job_id) ON DELETE CASCADE,
  step_name TEXT NOT NULL,
  attempt INTEGER NOT NULL DEFAULT 1 CHECK (attempt >= 1),
  status TEXT NOT NULL
    CHECK (status IN ('ready', 'leased', 'completed', 'cancelled', 'superseded')),
  resume_policy TEXT NOT NULL
    CHECK (resume_policy IN ('restart_step', 'skip_completed', 'manual_review')),
  provider_task_record_id TEXT REFERENCES provider_tasks(task_record_id) ON DELETE SET NULL,
  state_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (job_id, step_name, attempt)
);

CREATE INDEX IF NOT EXISTS idx_provider_tasks_job ON provider_tasks(job_id, step_name, attempt);
CREATE INDEX IF NOT EXISTS idx_provider_tasks_external ON provider_tasks(provider_id, provider_task_id);
CREATE INDEX IF NOT EXISTS idx_provider_tasks_status ON provider_tasks(status, updated_at);
CREATE INDEX IF NOT EXISTS idx_job_checkpoints_job ON job_checkpoints(job_id, step_name, attempt);
CREATE INDEX IF NOT EXISTS idx_job_checkpoints_status ON job_checkpoints(status, updated_at);

INSERT INTO schema_migrations(version, name)
VALUES ('0006', 'p0_runtime_recovery');

COMMIT;
