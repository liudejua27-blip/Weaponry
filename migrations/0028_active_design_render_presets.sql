PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- R001 keeps visual camera/light choices server-owned and tied to the active
-- Agent asset. Legacy snapshots intentionally remain NULL and read-only.
ALTER TABLE active_design_snapshots ADD COLUMN render_preset_json TEXT;

UPDATE active_design_snapshots
SET render_preset_json = json_object(
  'schema_version', 'ActiveDesignRenderPreset@1',
  'preset_id', 'render_' || active_asset_version_id || '_iso_cad_neutral',
  'project_id', project_id,
  'asset_version_id', active_asset_version_id,
  'camera_view', 'iso',
  'light_preset', 'cad_neutral',
  'updated_at', updated_at
)
WHERE source = 'agent_asset' AND active_asset_version_id IS NOT NULL;

INSERT INTO schema_migrations(version, name)
VALUES ('0028', 'active_design_render_presets');

COMMIT;
