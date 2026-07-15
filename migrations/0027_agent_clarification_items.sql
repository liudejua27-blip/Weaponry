-- D003: persist one plain-language clarification without creating a plan or asset.
-- SQLite CHECK constraints require a table rebuild; preserve all existing rows.
PRAGMA foreign_keys = OFF;
BEGIN IMMEDIATE;

CREATE TABLE agent_turns_d003_new (
  turn_id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES agent_threads(thread_id) ON DELETE CASCADE,
  request_text TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN (
    'queued', 'running', 'waiting_for_approval', 'waiting_for_clarification',
    'completed', 'failed', 'cancelled'
  )),
  error_code TEXT,
  error_message TEXT,
  usage_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(usage_json)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT INTO agent_turns_d003_new(
  turn_id, thread_id, request_text, status, error_code, error_message,
  usage_json, created_at, updated_at
)
SELECT
  turn_id, thread_id, request_text, status, error_code, error_message,
  usage_json, created_at, updated_at
FROM agent_turns;

DROP TABLE agent_turns;
ALTER TABLE agent_turns_d003_new RENAME TO agent_turns;
CREATE INDEX idx_agent_turns_thread_created
  ON agent_turns(thread_id, created_at ASC, turn_id ASC);

CREATE TABLE agent_items_d003_new (
  item_id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES agent_threads(thread_id) ON DELETE CASCADE,
  turn_id TEXT NOT NULL REFERENCES agent_turns(turn_id) ON DELETE CASCADE,
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  item_type TEXT NOT NULL CHECK (item_type IN (
    'user_message', 'assistant_message', 'plan', 'tool_call',
    'tool_result', 'preview', 'approval_request', 'clarification', 'artifact'
  )),
  status TEXT NOT NULL CHECK (status IN ('pending', 'completed', 'failed', 'cancelled')),
  payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
  created_at TEXT NOT NULL,
  UNIQUE(thread_id, sequence)
);

INSERT INTO agent_items_d003_new(
  item_id, thread_id, turn_id, sequence, item_type, status, payload_json, created_at
)
SELECT
  item_id, thread_id, turn_id, sequence, item_type, status, payload_json, created_at
FROM agent_items;

DROP TABLE agent_items;
ALTER TABLE agent_items_d003_new RENAME TO agent_items;
CREATE INDEX idx_agent_items_thread_sequence
  ON agent_items(thread_id, sequence ASC);

INSERT INTO schema_migrations(version, name)
VALUES ('0027', 'agent_clarification_items');

COMMIT;
PRAGMA foreign_keys = ON;
