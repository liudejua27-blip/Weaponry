PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE concept_jobs_0015_new (
  job_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  version_id TEXT REFERENCES project_versions(version_id) ON DELETE SET NULL,
  job_type TEXT NOT NULL CHECK (job_type IN (
    'interpret_brief', 'generate_variants', 'validate_graph',
    'quality_run', 'export_package', 'concept_change_plan'
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

INSERT INTO concept_jobs_0015_new (
  job_id, project_id, version_id, job_type, status, current_step,
  input_hash, input_json, output_json, error_code, error_message,
  created_at, updated_at, finished_at
)
SELECT
  job_id, project_id, version_id, job_type, status, current_step,
  input_hash, input_json, output_json, error_code, error_message,
  created_at, updated_at, finished_at
FROM concept_jobs;

CREATE TABLE concept_job_events_0015_new (
  event_id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES concept_jobs_0015_new(job_id) ON DELETE CASCADE,
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

INSERT INTO concept_job_events_0015_new (
  event_id, job_id, seq, project_id, version_id, step, level, status,
  message, progress, artifact_asset_id, metadata_json, created_at
)
SELECT
  event_id, job_id, seq, project_id, version_id, step, level, status,
  message, progress, artifact_asset_id, metadata_json, created_at
FROM concept_job_events;

DROP TABLE concept_job_events;
DROP TABLE concept_jobs;
ALTER TABLE concept_jobs_0015_new RENAME TO concept_jobs;
ALTER TABLE concept_job_events_0015_new RENAME TO concept_job_events;

CREATE INDEX idx_concept_jobs_project_status
  ON concept_jobs(project_id, status, updated_at);
CREATE INDEX idx_concept_jobs_version_type
  ON concept_jobs(version_id, job_type, created_at);
CREATE INDEX idx_concept_job_events_job_seq
  ON concept_job_events(job_id, seq);

ALTER TABLE design_change_sets
  ADD COLUMN actor_type TEXT NOT NULL DEFAULT 'user'
  CHECK (actor_type IN ('user', 'planner'));

ALTER TABLE design_change_sets
  ADD COLUMN planner_instruction TEXT;

ALTER TABLE design_change_sets
  ADD COLUMN planner_rationale_json TEXT
  CHECK (planner_rationale_json IS NULL OR json_valid(planner_rationale_json));

ALTER TABLE design_change_sets
  ADD COLUMN planner_provenance_json TEXT
  CHECK (planner_provenance_json IS NULL OR json_valid(planner_provenance_json));

ALTER TABLE design_change_sets
  ADD COLUMN planner_job_id TEXT
  REFERENCES concept_jobs(job_id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_change_sets_project_actor_updated
  ON design_change_sets(project_id, actor_type, updated_at DESC, change_set_id DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0015', 'change_planner_provenance');

COMMIT;
