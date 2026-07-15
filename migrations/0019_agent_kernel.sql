PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE agent_threads (
  thread_id TEXT PRIMARY KEY,
  project_id TEXT,
  title TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('idle', 'active', 'error', 'archived')),
  summary TEXT NOT NULL DEFAULT '',
  provider_id TEXT NOT NULL DEFAULT 'deterministic_kernel',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_turn_id TEXT
);

CREATE INDEX idx_agent_threads_project_updated
  ON agent_threads(project_id, updated_at DESC, thread_id DESC);

CREATE TABLE agent_turns (
  turn_id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES agent_threads(thread_id) ON DELETE CASCADE,
  request_text TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN (
    'queued', 'running', 'waiting_for_approval',
    'completed', 'failed', 'cancelled'
  )),
  error_code TEXT,
  error_message TEXT,
  usage_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(usage_json)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_agent_turns_thread_created
  ON agent_turns(thread_id, created_at ASC, turn_id ASC);

CREATE TABLE agent_items (
  item_id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES agent_threads(thread_id) ON DELETE CASCADE,
  turn_id TEXT NOT NULL REFERENCES agent_turns(turn_id) ON DELETE CASCADE,
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  item_type TEXT NOT NULL CHECK (item_type IN (
    'user_message', 'assistant_message', 'plan', 'tool_call',
    'tool_result', 'preview', 'approval_request', 'artifact'
  )),
  status TEXT NOT NULL CHECK (status IN ('pending', 'completed', 'failed', 'cancelled')),
  payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
  created_at TEXT NOT NULL,
  UNIQUE(thread_id, sequence)
);

CREATE INDEX idx_agent_items_thread_sequence
  ON agent_items(thread_id, sequence ASC);

CREATE TABLE agent_approvals (
  approval_id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES agent_threads(thread_id) ON DELETE CASCADE,
  turn_id TEXT NOT NULL REFERENCES agent_turns(turn_id) ON DELETE CASCADE,
  item_id TEXT NOT NULL REFERENCES agent_items(item_id) ON DELETE CASCADE,
  action TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected')),
  payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
  created_at TEXT NOT NULL,
  resolved_at TEXT
);

CREATE INDEX idx_agent_approvals_thread_status
  ON agent_approvals(thread_id, status, created_at ASC);

INSERT INTO schema_migrations(version, name)
VALUES ('0019', 'agent_kernel');

COMMIT;
