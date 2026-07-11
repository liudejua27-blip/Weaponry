PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE change_set_audit_exports (
  audit_export_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  package_asset_id TEXT REFERENCES concept_assets(asset_id) ON DELETE SET NULL,
  filters_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(filters_json)),
  manifest_json TEXT NOT NULL CHECK (json_valid(manifest_json)),
  record_count INTEGER NOT NULL CHECK (record_count >= 0),
  retention_class TEXT NOT NULL DEFAULT 'project_lifetime'
    CHECK (retention_class = 'project_lifetime'),
  status TEXT NOT NULL DEFAULT 'validated'
    CHECK (status IN ('validated', 'soft_deleted')),
  created_at TEXT NOT NULL
);

CREATE INDEX idx_change_set_audit_exports_project_created
  ON change_set_audit_exports(project_id, created_at DESC, audit_export_id DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0016', 'change_set_audit_exports');

COMMIT;
