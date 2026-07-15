PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- C104 keeps the user-facing part controls in the server-owned Snapshot. They
-- are view/edit protections, not geometry data and therefore do not mutate an
-- immutable AgentAssetVersion.
ALTER TABLE active_design_snapshots ADD COLUMN part_display_json TEXT;

UPDATE active_design_snapshots
SET part_display_json = json_object(
  'schema_version', 'ActiveDesignPartDisplay@1',
  'project_id', project_id,
  'asset_version_id', active_asset_version_id,
  'locked_part_ids', json('[]'),
  'hidden_part_ids', json('[]'),
  'isolated_part_id', NULL
)
WHERE source = 'agent_asset' AND active_asset_version_id IS NOT NULL;

INSERT INTO schema_migrations(version, name)
VALUES ('0031', 'active_design_part_display');

COMMIT;
