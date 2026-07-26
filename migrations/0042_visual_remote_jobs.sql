PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- Prompt-free recovery journal for user-started remote concept-image and
-- neural-3D work. Provider credentials, URLs, full prompts and raw responses
-- are intentionally excluded from the Rust-owned record JSON.
CREATE TABLE IF NOT EXISTS visual_remote_jobs (
  client_request_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  turn_id TEXT NOT NULL,
  stage TEXT NOT NULL CHECK (stage IN (
    'concept_submitted',
    'neural_submitted',
    'completed',
    'failed',
    'cancelled'
  )),
  record_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_visual_remote_jobs_recovery
  ON visual_remote_jobs(stage, project_id, updated_at, client_request_id);

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0042', 'visual_remote_jobs', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0042', 'visual_remote_jobs');

COMMIT;
