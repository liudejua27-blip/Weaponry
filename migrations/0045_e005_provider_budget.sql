PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- Rust-owned E005 formal Provider authorization and conservative usage ledger.
-- Prompts, credentials, reference bytes and Provider responses are excluded;
-- only immutable hashes, ceilings and state transitions are persisted.
CREATE TABLE IF NOT EXISTS e005_provider_run_authorizations (
  authorization_id TEXT PRIMARY KEY,
  task_set_sha256 TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  model_id TEXT NOT NULL,
  source_policy_sha256 TEXT NOT NULL,
  pricing_snapshot_sha256 TEXT NOT NULL,
  disclosure_sha256 TEXT NOT NULL,
  authorization_binding_sha256 TEXT NOT NULL UNIQUE,
  authorization_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('authorized', 'consumed', 'cancelled', 'expired')),
  maximum_author_calls INTEGER NOT NULL CHECK (maximum_author_calls = 30),
  maximum_patch_calls INTEGER NOT NULL CHECK (maximum_patch_calls = 30),
  maximum_total_calls INTEGER NOT NULL CHECK (maximum_total_calls = 60),
  maximum_input_tokens INTEGER NOT NULL CHECK (maximum_input_tokens > 0),
  maximum_output_tokens INTEGER NOT NULL CHECK (maximum_output_tokens > 0),
  maximum_variable_cost_microusd INTEGER NOT NULL CHECK (maximum_variable_cost_microusd > 0),
  maximum_batch_wall_time_ms INTEGER NOT NULL CHECK (maximum_batch_wall_time_ms BETWEEN 1 AND 10800000),
  maximum_single_call_wall_time_ms INTEGER NOT NULL CHECK (maximum_single_call_wall_time_ms BETWEEN 1 AND 105000),
  reservations_created INTEGER NOT NULL DEFAULT 0 CHECK (reservations_created >= 0),
  author_calls_accounted INTEGER NOT NULL DEFAULT 0 CHECK (author_calls_accounted BETWEEN 0 AND maximum_author_calls),
  patch_calls_accounted INTEGER NOT NULL DEFAULT 0 CHECK (patch_calls_accounted BETWEEN 0 AND maximum_patch_calls),
  calls_accounted INTEGER NOT NULL DEFAULT 0 CHECK (calls_accounted BETWEEN 0 AND maximum_total_calls),
  reserved_input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (reserved_input_tokens BETWEEN 0 AND maximum_input_tokens),
  reserved_output_tokens INTEGER NOT NULL DEFAULT 0 CHECK (reserved_output_tokens BETWEEN 0 AND maximum_output_tokens),
  reserved_cost_ceiling_microusd INTEGER NOT NULL DEFAULT 0 CHECK (reserved_cost_ceiling_microusd BETWEEN 0 AND maximum_variable_cost_microusd),
  accounted_input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (accounted_input_tokens BETWEEN 0 AND maximum_input_tokens),
  accounted_output_tokens INTEGER NOT NULL DEFAULT 0 CHECK (accounted_output_tokens BETWEEN 0 AND maximum_output_tokens),
  accounted_cost_ceiling_microusd INTEGER NOT NULL DEFAULT 0 CHECK (accounted_cost_ceiling_microusd BETWEEN 0 AND maximum_variable_cost_microusd),
  authorized_at_unix_ms INTEGER NOT NULL,
  expires_at_unix_ms INTEGER NOT NULL,
  batch_deadline_unix_ms INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL
  ,CHECK (calls_accounted = author_calls_accounted + patch_calls_accounted)
  ,CHECK (reserved_input_tokens + accounted_input_tokens <= maximum_input_tokens)
  ,CHECK (reserved_output_tokens + accounted_output_tokens <= maximum_output_tokens)
  ,CHECK (reserved_cost_ceiling_microusd + accounted_cost_ceiling_microusd <= maximum_variable_cost_microusd)
  ,CHECK (maximum_single_call_wall_time_ms <= maximum_batch_wall_time_ms)
  ,CHECK (authorized_at_unix_ms < batch_deadline_unix_ms AND batch_deadline_unix_ms <= expires_at_unix_ms)
  ,CHECK (authorized_at_unix_ms <= updated_at_unix_ms)
  ,UNIQUE(authorization_id, authorization_binding_sha256)
);

CREATE TABLE IF NOT EXISTS e005_provider_authorized_tasks (
  authorization_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  task_payload_sha256 TEXT NOT NULL,
  task_ordinal INTEGER NOT NULL CHECK (task_ordinal BETWEEN 1 AND 30),
  PRIMARY KEY(authorization_id, task_id),
  UNIQUE(authorization_id, task_ordinal),
  UNIQUE(authorization_id, task_id, task_payload_sha256),
  FOREIGN KEY(authorization_id) REFERENCES e005_provider_run_authorizations(authorization_id)
);

CREATE TABLE IF NOT EXISTS e005_provider_call_reservations (
  reservation_id TEXT PRIMARY KEY,
  authorization_id TEXT NOT NULL,
  authorization_binding_sha256 TEXT NOT NULL,
  task_id TEXT NOT NULL,
  task_payload_sha256 TEXT NOT NULL,
  call_kind TEXT NOT NULL CHECK (call_kind IN ('author', 'patch')),
  call_number INTEGER NOT NULL CHECK (call_number BETWEEN 1 AND 60),
  kind_call_number INTEGER NOT NULL CHECK (kind_call_number BETWEEN 1 AND 30),
  reservation_ordinal INTEGER NOT NULL CHECK (reservation_ordinal >= 1),
  request_sha256 TEXT NOT NULL,
  patch_base_source_sha256 TEXT,
  failed_gate_sha256 TEXT,
  reserved_input_tokens INTEGER NOT NULL CHECK (reserved_input_tokens > 0),
  reserved_output_tokens INTEGER NOT NULL CHECK (reserved_output_tokens > 0),
  reserved_cost_ceiling_microusd INTEGER NOT NULL CHECK (reserved_cost_ceiling_microusd > 0),
  deadline_unix_ms INTEGER NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('reserved', 'dispatching', 'accounted', 'released')),
  network_call_made INTEGER CHECK (network_call_made IN (0, 1)),
  outcome_code TEXT,
  output_source_sha256 TEXT,
  output_gate_sha256 TEXT,
  settlement_evidence_json TEXT,
  settlement_evidence_sha256 TEXT,
  created_at_unix_ms INTEGER NOT NULL,
  dispatched_at_unix_ms INTEGER,
  settled_at_unix_ms INTEGER,
  FOREIGN KEY(authorization_id, authorization_binding_sha256) REFERENCES e005_provider_run_authorizations(authorization_id, authorization_binding_sha256),
  FOREIGN KEY(authorization_id, task_id, task_payload_sha256) REFERENCES e005_provider_authorized_tasks(authorization_id, task_id, task_payload_sha256),
  CHECK ((call_kind = 'author' AND patch_base_source_sha256 IS NULL AND failed_gate_sha256 IS NULL) OR (call_kind = 'patch' AND patch_base_source_sha256 IS NOT NULL AND failed_gate_sha256 IS NOT NULL)),
  CHECK (deadline_unix_ms > created_at_unix_ms),
  CHECK (dispatched_at_unix_ms IS NULL OR dispatched_at_unix_ms >= created_at_unix_ms),
  CHECK (settled_at_unix_ms IS NULL OR settled_at_unix_ms >= created_at_unix_ms),
  CHECK (
    (state = 'reserved' AND network_call_made IS NULL AND outcome_code IS NULL AND dispatched_at_unix_ms IS NULL AND settled_at_unix_ms IS NULL AND settlement_evidence_json IS NULL AND settlement_evidence_sha256 IS NULL)
    OR (state = 'dispatching' AND network_call_made = 1 AND outcome_code IS NULL AND dispatched_at_unix_ms IS NOT NULL AND settled_at_unix_ms IS NULL AND settlement_evidence_json IS NULL AND settlement_evidence_sha256 IS NULL)
    OR (state = 'accounted' AND network_call_made = 1 AND outcome_code IS NOT NULL AND dispatched_at_unix_ms IS NOT NULL AND settled_at_unix_ms IS NOT NULL AND settlement_evidence_json IS NOT NULL AND settlement_evidence_sha256 IS NOT NULL)
    OR (state = 'released' AND network_call_made = 0 AND outcome_code IS NOT NULL AND dispatched_at_unix_ms IS NULL AND settled_at_unix_ms IS NOT NULL AND settlement_evidence_json IS NOT NULL AND settlement_evidence_sha256 IS NOT NULL)
  ),
  UNIQUE(authorization_id, reservation_ordinal)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_e005_provider_one_counted_task_kind
  ON e005_provider_call_reservations(authorization_id, task_id, call_kind)
  WHERE state IN ('reserved', 'dispatching', 'accounted');

CREATE INDEX IF NOT EXISTS idx_e005_provider_reservation_recovery
  ON e005_provider_call_reservations(state, deadline_unix_ms, reservation_id);

CREATE INDEX IF NOT EXISTS idx_e005_provider_authorization_audit
  ON e005_provider_run_authorizations(status, updated_at_unix_ms, authorization_id);

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0045', 'e005_provider_budget', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0045', 'e005_provider_budget');

COMMIT;
