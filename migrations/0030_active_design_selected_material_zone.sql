PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

ALTER TABLE active_design_snapshots
  ADD COLUMN selected_material_zone_id TEXT
  CHECK (selected_material_zone_id IS NULL OR selected_material_zone_id GLOB 'zone_[a-z0-9_-]*');

INSERT INTO schema_migrations(version, name)
VALUES ('0030', 'active_design_selected_material_zone');

COMMIT;
