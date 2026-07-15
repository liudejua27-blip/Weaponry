PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE agent_asset_quality_reports (
  quality_report_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  asset_version_id TEXT NOT NULL REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  report_json TEXT NOT NULL CHECK (json_valid(report_json)),
  status TEXT NOT NULL CHECK (status IN ('passed', 'warning', 'failed', 'unavailable')),
  created_at TEXT NOT NULL
);

CREATE INDEX idx_agent_asset_quality_reports_asset_created
  ON agent_asset_quality_reports(asset_version_id, created_at DESC, quality_report_id DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0025', 'agent_asset_quality_reports');

COMMIT;
