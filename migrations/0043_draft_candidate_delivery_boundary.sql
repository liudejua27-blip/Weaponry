-- Rust-owned creative-loop staging. A row here is never an AgentAssetVersion
-- and never names ActiveDesignSnapshot. Confirmation is the only operation
-- that may create a permanent version and its production object references.
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS forgecad_core_draft_candidates (
  candidate_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  base_asset_version_id TEXT,
  draft_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('draft', 'confirmed', 'cancelled', 'failed')),
  idempotency_key TEXT NOT NULL,
  request_hash TEXT NOT NULL CHECK (length(request_hash) = 64),
  confirmed_asset_version_id TEXT,
  quality_report_id TEXT,
  failure_code TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_forgecad_core_draft_candidates_project
  ON forgecad_core_draft_candidates(project_id, updated_at DESC, candidate_id DESC);

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0043', 'draft_candidate_delivery_boundary', datetime('now'));

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0043', 'draft_candidate_delivery_boundary');

COMMIT;
