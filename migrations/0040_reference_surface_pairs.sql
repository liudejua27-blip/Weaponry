PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- R007B freezes the visible-language interpretation once, alongside the
-- existing R007A rebuild plan.  Source bytes remain authoritative through
-- reference_evidence; this table deliberately does not duplicate them.
CREATE TABLE IF NOT EXISTS reference_surface_analyses (
  rebuild_plan_id TEXT PRIMARY KEY REFERENCES reference_guided_rebuild_plans(rebuild_plan_id) ON DELETE RESTRICT,
  analysis_sha256 TEXT NOT NULL UNIQUE CHECK (length(analysis_sha256) = 64),
  analysis_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

-- A confirmed plan already owns its original result via
-- reference_guided_rebuild_plans.confirmed_asset_version_id.  This narrow
-- lineage index lets immutable undo/redo descendants retain that same frozen
-- pair without copying evidence, analysis, or GLB hashes into version JSON.
CREATE TABLE IF NOT EXISTS reference_rebuild_result_lineage (
  asset_version_id TEXT PRIMARY KEY REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  rebuild_plan_id TEXT NOT NULL REFERENCES reference_guided_rebuild_plans(rebuild_plan_id) ON DELETE RESTRICT,
  source_result_asset_version_id TEXT NOT NULL REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reference_rebuild_result_lineage_plan
  ON reference_rebuild_result_lineage(rebuild_plan_id, asset_version_id);

-- Frozen analysis content is immutable; lifecycle state continues to be owned
-- by reference_guided_rebuild_plans and normal ChangeSet rows.
CREATE TRIGGER IF NOT EXISTS reference_surface_analysis_immutable
BEFORE UPDATE ON reference_surface_analyses
BEGIN
  SELECT RAISE(ABORT, 'Reference surface analysis is immutable');
END;

CREATE TRIGGER IF NOT EXISTS reference_rebuild_result_lineage_immutable
BEFORE UPDATE ON reference_rebuild_result_lineage
BEGIN
  SELECT RAISE(ABORT, 'Reference rebuild result lineage is immutable');
END;

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0040', 'reference_surface_pairs', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0040', 'reference_surface_pairs');

COMMIT;
