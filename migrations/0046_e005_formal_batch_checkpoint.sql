PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- Rust-owned E005 batch/checkpoint state. Provider payloads, credentials and
-- GLB bytes are excluded. Receipts are small canonical evidence envelopes;
-- geometry remains in its existing content-addressed product stores.
CREATE TABLE IF NOT EXISTS e005_formal_batches (
  batch_id TEXT PRIMARY KEY,
  authorization_id TEXT NOT NULL UNIQUE,
  task_set_sha256 TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('ready', 'running', 'reconciliation_required', 'completed', 'cancelled')),
  total_task_count INTEGER NOT NULL CHECK (total_task_count = 30),
  sealed_receipt_count INTEGER NOT NULL DEFAULT 0 CHECK (sealed_receipt_count BETWEEN 0 AND total_task_count),
  created_at_unix_ms INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL,
  FOREIGN KEY(authorization_id) REFERENCES e005_provider_run_authorizations(authorization_id),
  UNIQUE(batch_id, authorization_id),
  CHECK (updated_at_unix_ms >= created_at_unix_ms),
  CHECK ((status = 'completed' AND sealed_receipt_count = total_task_count) OR status != 'completed')
);

CREATE TABLE IF NOT EXISTS e005_formal_batch_tasks (
  batch_id TEXT NOT NULL,
  authorization_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  task_payload_sha256 TEXT NOT NULL,
  task_ordinal INTEGER NOT NULL CHECK (task_ordinal BETWEEN 1 AND 30),
  state TEXT NOT NULL CHECK (state IN ('pending', 'running', 'receipt_sealed', 'reconciliation_required')),
  receipt_json TEXT,
  receipt_sha256 TEXT,
  started_at_unix_ms INTEGER,
  sealed_at_unix_ms INTEGER,
  PRIMARY KEY(batch_id, task_id),
  UNIQUE(batch_id, task_ordinal),
  FOREIGN KEY(batch_id, authorization_id) REFERENCES e005_formal_batches(batch_id, authorization_id),
  FOREIGN KEY(authorization_id, task_id, task_payload_sha256) REFERENCES e005_provider_authorized_tasks(authorization_id, task_id, task_payload_sha256),
  CHECK (
    (state = 'pending' AND receipt_json IS NULL AND receipt_sha256 IS NULL AND started_at_unix_ms IS NULL AND sealed_at_unix_ms IS NULL)
    OR (state IN ('running', 'reconciliation_required') AND receipt_json IS NULL AND receipt_sha256 IS NULL AND started_at_unix_ms IS NOT NULL AND sealed_at_unix_ms IS NULL)
    OR (state = 'receipt_sealed' AND receipt_json IS NOT NULL AND receipt_sha256 IS NOT NULL AND started_at_unix_ms IS NOT NULL AND sealed_at_unix_ms IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS idx_e005_formal_batch_next_task
  ON e005_formal_batch_tasks(batch_id, state, task_ordinal);

CREATE INDEX IF NOT EXISTS idx_e005_formal_batch_recovery
  ON e005_formal_batch_tasks(state, batch_id, task_ordinal);

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0046', 'e005_formal_batch_checkpoint', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0046', 'e005_formal_batch_checkpoint');

COMMIT;
