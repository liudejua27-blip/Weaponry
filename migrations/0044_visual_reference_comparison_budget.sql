PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- Rust-owned, prompt-free authorization and conservative spend ledger for
-- reference-versus-candidate visual comparison. Credentials, prompts, image
-- bytes, URLs and Provider responses are intentionally excluded.
CREATE TABLE IF NOT EXISTS visual_reference_comparison_authorizations (
  authorization_id TEXT PRIMARY KEY,
  client_request_id TEXT NOT NULL UNIQUE,
  project_id TEXT NOT NULL,
  request_sha256 TEXT NOT NULL,
  evidence_graph_sha256 TEXT NOT NULL,
  acceptance_policy_sha256 TEXT NOT NULL,
  authorization_binding_sha256 TEXT NOT NULL,
  bound_turn_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('authorized', 'consumed', 'cancelled', 'expired')),
  maximum_calls INTEGER NOT NULL CHECK (maximum_calls = 3),
  maximum_variable_cost_microusd INTEGER NOT NULL CHECK (maximum_variable_cost_microusd = 100000),
  reservations_created INTEGER NOT NULL DEFAULT 0 CHECK (reservations_created >= 0),
  calls_accounted INTEGER NOT NULL DEFAULT 0 CHECK (calls_accounted BETWEEN 0 AND maximum_calls),
  accounted_cost_ceiling_microusd INTEGER NOT NULL DEFAULT 0 CHECK (accounted_cost_ceiling_microusd BETWEEN 0 AND maximum_variable_cost_microusd),
  reserved_cost_ceiling_microusd INTEGER NOT NULL DEFAULT 0 CHECK (reserved_cost_ceiling_microusd BETWEEN 0 AND maximum_variable_cost_microusd),
  authorized_at_unix_ms INTEGER NOT NULL,
  expires_at_unix_ms INTEGER NOT NULL,
  updated_at_unix_ms INTEGER NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(project_id)
);

CREATE TABLE IF NOT EXISTS visual_reference_comparison_reservations (
  reservation_id TEXT PRIMARY KEY,
  authorization_id TEXT NOT NULL,
  turn_id TEXT NOT NULL,
  comparison_input_sha256 TEXT NOT NULL,
  call_number INTEGER NOT NULL CHECK (call_number BETWEEN 1 AND 3),
  reservation_ordinal INTEGER NOT NULL CHECK (reservation_ordinal >= 1),
  reserved_cost_ceiling_microusd INTEGER NOT NULL CHECK (reserved_cost_ceiling_microusd BETWEEN 1 AND 100000),
  state TEXT NOT NULL CHECK (state IN ('reserved', 'accounted', 'released')),
  network_call_made INTEGER CHECK (network_call_made IN (0, 1)),
  outcome_code TEXT,
  created_at_unix_ms INTEGER NOT NULL,
  settled_at_unix_ms INTEGER,
  FOREIGN KEY(authorization_id) REFERENCES visual_reference_comparison_authorizations(authorization_id),
  UNIQUE(authorization_id, reservation_ordinal)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_visual_reference_comparison_one_active_reservation
  ON visual_reference_comparison_reservations(authorization_id)
  WHERE state = 'reserved';

CREATE INDEX IF NOT EXISTS idx_visual_reference_comparison_authorization_audit
  ON visual_reference_comparison_authorizations(project_id, status, updated_at_unix_ms, authorization_id);

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0044', 'visual_reference_comparison_budget', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0044', 'visual_reference_comparison_budget');

COMMIT;
