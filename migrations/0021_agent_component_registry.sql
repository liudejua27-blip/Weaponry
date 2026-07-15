PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- Reusable Agent components are immutable snapshots separate from the legacy
-- ModuleAssetManifest@1 catalog. They are intentionally scoped to a local
-- project workspace and can only be inserted from a confirmed Agent asset.
CREATE TABLE agent_components (
  component_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  domain_pack_id TEXT NOT NULL,
  role TEXT NOT NULL,
  display_name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  source_asset_version_id TEXT NOT NULL REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  source_part_id TEXT NOT NULL,
  part_template_json TEXT NOT NULL CHECK (json_valid(part_template_json)),
  shape_operation_json TEXT NOT NULL CHECK (json_valid(shape_operation_json)),
  material_bindings_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(material_bindings_json)),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_agent_components_scope
  ON agent_components(project_id, domain_pack_id, role, status, updated_at DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0021', 'agent_component_registry');

COMMIT;
