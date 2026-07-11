PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS concept_jobs (
  job_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  version_id TEXT REFERENCES project_versions(version_id) ON DELETE SET NULL,
  job_type TEXT NOT NULL CHECK (job_type IN (
    'interpret_brief', 'generate_variants', 'validate_graph',
    'quality_run', 'export_package'
  )),
  status TEXT NOT NULL CHECK (status IN (
    'created', 'queued', 'running', 'waiting_provider', 'waiting_user',
    'retrying', 'succeeded', 'failed', 'cancelled', 'partial_succeeded'
  )),
  current_step TEXT,
  input_hash TEXT NOT NULL CHECK (length(input_hash) = 64),
  input_json TEXT NOT NULL CHECK (json_valid(input_json)),
  output_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(output_json)),
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  finished_at TEXT
);

CREATE TABLE IF NOT EXISTS concept_job_events (
  event_id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES concept_jobs(job_id) ON DELETE CASCADE,
  seq INTEGER NOT NULL CHECK (seq >= 1),
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  version_id TEXT REFERENCES project_versions(version_id) ON DELETE SET NULL,
  step TEXT NOT NULL,
  level TEXT NOT NULL CHECK (level IN ('info', 'warning', 'error')),
  status TEXT NOT NULL CHECK (status IN (
    'created', 'queued', 'running', 'waiting_provider', 'waiting_user',
    'retrying', 'succeeded', 'failed', 'cancelled', 'partial_succeeded'
  )),
  message TEXT NOT NULL,
  progress REAL NOT NULL CHECK (progress >= 0 AND progress <= 1),
  artifact_asset_id TEXT REFERENCES concept_assets(asset_id) ON DELETE SET NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
  created_at TEXT NOT NULL,
  UNIQUE (job_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_concept_jobs_project_status
  ON concept_jobs(project_id, status, updated_at);
CREATE INDEX IF NOT EXISTS idx_concept_jobs_version_type
  ON concept_jobs(version_id, job_type, created_at);
CREATE INDEX IF NOT EXISTS idx_concept_job_events_job_seq
  ON concept_job_events(job_id, seq);

INSERT INTO schema_migrations(version, name)
VALUES ('0011', 'r2_concept_jobs');

COMMIT;
