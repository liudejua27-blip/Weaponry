-- U004: a captured candidate is a live, non-terminal Agent Turn. SQLite
-- CHECK constraints require a table rebuild; preserve legacy provider-context
-- columns and every existing Turn exactly.
PRAGMA foreign_keys = OFF;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE agent_turns_u004_capture_new (
  turn_id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES agent_threads(thread_id) ON DELETE CASCADE,
  request_text TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN (
    'queued', 'running', 'waiting_for_capture', 'waiting_for_approval',
    'waiting_for_clarification', 'completed', 'failed', 'cancelled'
  )),
  error_code TEXT,
  error_message TEXT,
  usage_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(usage_json)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  context_hash TEXT,
  prompt_contract_version TEXT,
  provider_request_fingerprint TEXT
);

INSERT INTO agent_turns_u004_capture_new(
  turn_id, thread_id, request_text, status, error_code, error_message,
  usage_json, created_at, updated_at, context_hash, prompt_contract_version,
  provider_request_fingerprint
)
SELECT
  turn_id, thread_id, request_text, status, error_code, error_message,
  usage_json, created_at, updated_at, context_hash, prompt_contract_version,
  provider_request_fingerprint
FROM agent_turns;

DROP TABLE agent_turns;
ALTER TABLE agent_turns_u004_capture_new RENAME TO agent_turns;
CREATE INDEX idx_agent_turns_thread_created
  ON agent_turns(thread_id, created_at ASC, turn_id ASC);

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0048', 'agent_turn_waiting_for_capture', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0048', 'agent_turn_waiting_for_capture');

COMMIT;
PRAGMA foreign_keys = ON;
