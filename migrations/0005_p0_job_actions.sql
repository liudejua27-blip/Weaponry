PRAGMA foreign_keys = OFF;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS agent_events_new (
  event_id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES generation_jobs(job_id) ON DELETE CASCADE,
  seq INTEGER NOT NULL CHECK (seq >= 1),
  weapon_id TEXT REFERENCES weapons(weapon_id) ON DELETE SET NULL,
  step TEXT NOT NULL,
  level TEXT NOT NULL CHECK (level IN ('info', 'warning', 'error')),
  status TEXT NOT NULL CHECK (status IN (
    'created', 'queued', 'started', 'progress', 'waiting_provider',
    'waiting_user', 'retrying', 'succeeded', 'failed', 'cancelled',
    'partial_succeeded', 'skipped'
  )),
  message TEXT NOT NULL,
  artifact_asset_id TEXT REFERENCES asset_files(file_id) ON DELETE SET NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (job_id, seq)
);

INSERT INTO agent_events_new (
  event_id, job_id, seq, weapon_id, step, level, status, message,
  artifact_asset_id, metadata_json, created_at
)
SELECT
  event_id, job_id, seq, weapon_id, step, level, status, message,
  artifact_asset_id, metadata_json, created_at
FROM agent_events;

DROP TABLE agent_events;
ALTER TABLE agent_events_new RENAME TO agent_events;

CREATE TABLE IF NOT EXISTS job_actions (
  action_id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES generation_jobs(job_id) ON DELETE CASCADE,
  action_type TEXT NOT NULL CHECK (action_type IN ('cancel', 'retry', 'retry_from_step')),
  requested_step TEXT,
  status TEXT NOT NULL CHECK (status IN ('accepted', 'rejected', 'noop')),
  previous_job_status TEXT NOT NULL,
  resulting_job_status TEXT NOT NULL,
  event_id TEXT REFERENCES agent_events(event_id) ON DELETE SET NULL,
  message TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_job_actions_job ON job_actions(job_id, created_at);

INSERT INTO schema_migrations(version, name)
VALUES ('0005', 'p0_job_actions');

COMMIT;

PRAGMA foreign_keys = ON;
