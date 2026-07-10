PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

ALTER TABLE design_change_sets ADD COLUMN diagnostic_json TEXT
  CHECK (diagnostic_json IS NULL OR json_valid(diagnostic_json));

CREATE INDEX IF NOT EXISTS idx_change_sets_project_updated
  ON design_change_sets(project_id, updated_at DESC, change_set_id DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0012', 'r3_change_set_audit');

COMMIT;
