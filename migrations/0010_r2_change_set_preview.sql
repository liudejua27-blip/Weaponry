PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

ALTER TABLE design_change_sets ADD COLUMN preview_spec_json TEXT
  CHECK (preview_spec_json IS NULL OR json_valid(preview_spec_json));
ALTER TABLE design_change_sets ADD COLUMN preview_graph_json TEXT
  CHECK (preview_graph_json IS NULL OR json_valid(preview_graph_json));
ALTER TABLE design_change_sets ADD COLUMN preview_sha256 TEXT
  CHECK (preview_sha256 IS NULL OR length(preview_sha256) = 64);
ALTER TABLE design_change_sets ADD COLUMN confirmed_at TEXT;

CREATE INDEX IF NOT EXISTS idx_change_sets_result_version
  ON design_change_sets(result_version_id, status);

INSERT INTO schema_migrations(version, name)
VALUES ('0010', 'r2_change_set_preview');

COMMIT;
