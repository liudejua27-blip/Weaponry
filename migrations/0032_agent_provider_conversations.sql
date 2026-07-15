PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

ALTER TABLE agent_turns ADD COLUMN context_hash TEXT;
ALTER TABLE agent_turns ADD COLUMN prompt_contract_version TEXT;
ALTER TABLE agent_turns ADD COLUMN provider_request_fingerprint TEXT;

CREATE TABLE agent_thread_memory_summaries (
  summary_id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES agent_threads(thread_id) ON DELETE CASCADE,
  up_to_sequence INTEGER NOT NULL CHECK (up_to_sequence > 0),
  summary_text TEXT NOT NULL CHECK (length(summary_text) <= 4000),
  domain_pack_id TEXT,
  snapshot_fingerprint TEXT,
  prompt_contract_version TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(thread_id, up_to_sequence)
);

CREATE INDEX idx_agent_thread_memory_latest
  ON agent_thread_memory_summaries(thread_id, up_to_sequence DESC, summary_id DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0032', 'agent_provider_conversations');

COMMIT;
