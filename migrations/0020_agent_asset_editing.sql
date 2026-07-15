PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- General mechanical Agent assets deliberately live beside, not inside, the
-- legacy WeaponConcept project_versions/module_graphs tables. This lets the
-- strangler migration keep immutable general assets without pretending they
-- are WeaponConceptSpec@1 records.
CREATE TABLE agent_blockout_candidates (
  artifact_id TEXT PRIMARY KEY,
  project_id TEXT,
  plan_id TEXT NOT NULL,
  direction_id TEXT NOT NULL,
  domain_pack_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'candidate'
    CHECK (status IN ('candidate', 'committed', 'discarded')),
  candidate_json TEXT NOT NULL CHECK (json_valid(candidate_json)),
  shape_program_json TEXT NOT NULL CHECK (json_valid(shape_program_json)),
  assembly_graph_json TEXT NOT NULL CHECK (json_valid(assembly_graph_json)),
  material_bindings_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(material_bindings_json)),
  glb_base64 TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_agent_blockout_candidates_project_created
  ON agent_blockout_candidates(project_id, created_at DESC, artifact_id DESC);

CREATE TABLE agent_asset_versions (
  asset_version_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  parent_asset_version_id TEXT REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  version_no INTEGER NOT NULL CHECK (version_no >= 1),
  status TEXT NOT NULL DEFAULT 'committed'
    CHECK (status IN ('committed', 'superseded', 'soft_deleted')),
  summary TEXT NOT NULL,
  stage TEXT NOT NULL CHECK (stage IN ('segmented_concept', 'editable_asset')),
  plan_id TEXT NOT NULL,
  direction_id TEXT NOT NULL,
  domain_pack_id TEXT NOT NULL,
  artifact_id TEXT NOT NULL,
  parts_json TEXT NOT NULL CHECK (json_valid(parts_json)),
  shape_program_json TEXT NOT NULL CHECK (json_valid(shape_program_json)),
  assembly_graph_json TEXT NOT NULL CHECK (json_valid(assembly_graph_json)),
  material_bindings_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(material_bindings_json)),
  created_at TEXT NOT NULL,
  UNIQUE(project_id, version_no)
);

CREATE INDEX idx_agent_asset_versions_project_created
  ON agent_asset_versions(project_id, created_at DESC, asset_version_id DESC);

CREATE TABLE agent_asset_heads (
  project_id TEXT PRIMARY KEY,
  asset_version_id TEXT NOT NULL REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  updated_at TEXT NOT NULL
);

CREATE TABLE agent_asset_change_sets (
  change_set_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  base_asset_version_id TEXT NOT NULL REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  summary TEXT NOT NULL,
  operations_json TEXT NOT NULL CHECK (json_valid(operations_json)),
  protected_part_ids_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(protected_part_ids_json)),
  preview_json TEXT CHECK (preview_json IS NULL OR json_valid(preview_json)),
  status TEXT NOT NULL DEFAULT 'proposed'
    CHECK (status IN ('proposed', 'previewed', 'confirmed', 'rejected', 'stale')),
  resulting_asset_version_id TEXT REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_agent_asset_change_sets_project_updated
  ON agent_asset_change_sets(project_id, updated_at DESC, change_set_id DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0020', 'agent_asset_editing');

COMMIT;
