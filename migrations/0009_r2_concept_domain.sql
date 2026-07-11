PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS domain_profiles (
  profile_id TEXT PRIMARY KEY,
  domain_type TEXT NOT NULL CHECK (domain_type IN ('weapon_concept')),
  schema_version TEXT NOT NULL CHECK (schema_version = 'DesignDomainProfile@1'),
  pack_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  profile_json TEXT NOT NULL CHECK (json_valid(profile_json)),
  profile_sha256 TEXT NOT NULL CHECK (length(profile_sha256) = 64),
  status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'deprecated', 'disabled')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
  project_id TEXT PRIMARY KEY,
  profile_id TEXT NOT NULL REFERENCES domain_profiles(profile_id) ON DELETE RESTRICT,
  domain_type TEXT NOT NULL CHECK (domain_type IN ('weapon_concept')),
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'archived', 'soft_deleted')),
  current_version_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS project_versions (
  version_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  parent_version_id TEXT REFERENCES project_versions(version_id) ON DELETE RESTRICT,
  version_no INTEGER NOT NULL CHECK (version_no >= 1),
  status TEXT NOT NULL DEFAULT 'committed'
    CHECK (status IN ('draft', 'committed', 'superseded', 'soft_deleted')),
  summary TEXT NOT NULL,
  spec_schema_version TEXT NOT NULL CHECK (spec_schema_version = 'WeaponConceptSpec@1'),
  spec_json TEXT NOT NULL CHECK (json_valid(spec_json)),
  spec_sha256 TEXT NOT NULL CHECK (length(spec_sha256) = 64),
  module_graph_id TEXT,
  change_set_id TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (project_id, version_no)
);

CREATE TABLE IF NOT EXISTS concept_assets (
  asset_id TEXT PRIMARY KEY,
  project_id TEXT REFERENCES projects(project_id) ON DELETE CASCADE,
  version_id TEXT REFERENCES project_versions(version_id) ON DELETE SET NULL,
  role TEXT NOT NULL CHECK (role IN (
    'reference_image', 'module_glb', 'combined_glb', 'obj', 'preview_png',
    'exploded_png', 'turntable', 'module_manifest', 'quality_report',
    'project_report', 'export_package', 'other'
  )),
  logical_path TEXT NOT NULL,
  object_path TEXT NOT NULL,
  sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  mime_type TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
  created_at TEXT NOT NULL,
  soft_deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS module_assets (
  module_id TEXT PRIMARY KEY,
  pack_id TEXT NOT NULL,
  category TEXT NOT NULL CHECK (category IN (
    'core_shell', 'front_shell', 'rear_shell', 'grip_shell',
    'top_accessory', 'side_accessory', 'lower_structure',
    'storage_visual', 'armor_panel'
  )),
  asset_id TEXT NOT NULL REFERENCES concept_assets(asset_id) ON DELETE RESTRICT,
  schema_version TEXT NOT NULL CHECK (schema_version = 'ModuleAssetManifest@1'),
  manifest_json TEXT NOT NULL CHECK (json_valid(manifest_json)),
  manifest_sha256 TEXT NOT NULL CHECK (length(manifest_sha256) = 64),
  status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'disabled', 'soft_deleted')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS module_connectors (
  connector_id TEXT PRIMARY KEY,
  module_id TEXT NOT NULL REFERENCES module_assets(module_id) ON DELETE CASCADE,
  slot TEXT NOT NULL,
  connector_type TEXT NOT NULL,
  transform_json TEXT NOT NULL CHECK (json_valid(transform_json)),
  scale_min REAL NOT NULL CHECK (scale_min > 0),
  scale_max REAL NOT NULL CHECK (scale_max >= scale_min),
  exclusive INTEGER NOT NULL DEFAULT 1 CHECK (exclusive IN (0, 1)),
  created_at TEXT NOT NULL,
  UNIQUE (module_id, slot)
);

CREATE TABLE IF NOT EXISTS module_graphs (
  graph_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  version_id TEXT REFERENCES project_versions(version_id) ON DELETE SET NULL,
  root_node_id TEXT NOT NULL,
  schema_version TEXT NOT NULL CHECK (schema_version = 'ModuleGraph@1'),
  graph_json TEXT NOT NULL CHECK (json_valid(graph_json)),
  graph_sha256 TEXT NOT NULL CHECK (length(graph_sha256) = 64),
  validation_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (validation_status IN ('pending', 'valid', 'invalid')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS module_graph_nodes (
  graph_id TEXT NOT NULL REFERENCES module_graphs(graph_id) ON DELETE CASCADE,
  node_id TEXT NOT NULL,
  module_id TEXT NOT NULL REFERENCES module_assets(module_id) ON DELETE RESTRICT,
  transform_json TEXT NOT NULL CHECK (json_valid(transform_json)),
  locked INTEGER NOT NULL DEFAULT 0 CHECK (locked IN (0, 1)),
  visible INTEGER NOT NULL DEFAULT 1 CHECK (visible IN (0, 1)),
  PRIMARY KEY (graph_id, node_id)
);

CREATE TABLE IF NOT EXISTS module_graph_edges (
  graph_id TEXT NOT NULL REFERENCES module_graphs(graph_id) ON DELETE CASCADE,
  edge_id TEXT NOT NULL,
  from_node_id TEXT NOT NULL,
  from_connector_id TEXT NOT NULL REFERENCES module_connectors(connector_id) ON DELETE RESTRICT,
  to_node_id TEXT NOT NULL,
  to_connector_id TEXT NOT NULL REFERENCES module_connectors(connector_id) ON DELETE RESTRICT,
  status TEXT NOT NULL CHECK (status IN ('connected', 'invalid')),
  PRIMARY KEY (graph_id, edge_id),
  FOREIGN KEY (graph_id, from_node_id) REFERENCES module_graph_nodes(graph_id, node_id) ON DELETE CASCADE,
  FOREIGN KEY (graph_id, to_node_id) REFERENCES module_graph_nodes(graph_id, node_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS design_briefs (
  brief_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  source_text TEXT NOT NULL,
  reference_asset_ids_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(reference_asset_ids_json)),
  interpreted_spec_json TEXT CHECK (interpreted_spec_json IS NULL OR json_valid(interpreted_spec_json)),
  status TEXT NOT NULL CHECK (status IN ('draft', 'interpreted', 'confirmed', 'failed')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS design_variants (
  variant_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  brief_id TEXT REFERENCES design_briefs(brief_id) ON DELETE SET NULL,
  rank INTEGER NOT NULL CHECK (rank BETWEEN 1 AND 3),
  name TEXT NOT NULL,
  summary TEXT NOT NULL,
  module_graph_json TEXT NOT NULL CHECK (json_valid(module_graph_json)),
  status TEXT NOT NULL CHECK (status IN ('proposed', 'selected', 'rejected')),
  created_at TEXT NOT NULL,
  UNIQUE (project_id, brief_id, rank)
);

CREATE TABLE IF NOT EXISTS design_change_sets (
  change_set_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  base_version_id TEXT NOT NULL REFERENCES project_versions(version_id) ON DELETE RESTRICT,
  result_version_id TEXT REFERENCES project_versions(version_id) ON DELETE SET NULL,
  schema_version TEXT NOT NULL CHECK (schema_version = 'DesignChangeSet@1'),
  change_set_json TEXT NOT NULL CHECK (json_valid(change_set_json)),
  change_set_sha256 TEXT NOT NULL CHECK (length(change_set_sha256) = 64),
  status TEXT NOT NULL CHECK (status IN ('proposed', 'previewed', 'confirmed', 'rejected', 'stale')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS quality_runs (
  quality_run_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  version_id TEXT NOT NULL REFERENCES project_versions(version_id) ON DELETE CASCADE,
  report_asset_id TEXT REFERENCES concept_assets(asset_id) ON DELETE SET NULL,
  ruleset_version TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('passed', 'warning', 'failed', 'not_run')),
  report_json TEXT NOT NULL CHECK (json_valid(report_json)),
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS quality_findings (
  finding_id TEXT PRIMARY KEY,
  quality_run_id TEXT NOT NULL REFERENCES quality_runs(quality_run_id) ON DELETE CASCADE,
  check_id TEXT NOT NULL,
  category TEXT NOT NULL CHECK (category IN ('graph', 'mesh', 'assembly')),
  severity TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'error')),
  status TEXT NOT NULL CHECK (status IN ('passed', 'warning', 'failed', 'not_run')),
  node_ids_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(node_ids_json)),
  measured_value_json TEXT,
  threshold_json TEXT,
  message TEXT NOT NULL,
  suggestion TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS export_packages_v2 (
  export_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  version_id TEXT NOT NULL REFERENCES project_versions(version_id) ON DELETE RESTRICT,
  profile TEXT NOT NULL CHECK (profile IN (
    'visual_asset', 'game_asset', 'film_prop', 'non_functional_display'
  )),
  package_asset_id TEXT REFERENCES concept_assets(asset_id) ON DELETE SET NULL,
  manifest_json TEXT NOT NULL CHECK (json_valid(manifest_json)),
  status TEXT NOT NULL CHECK (status IN ('created', 'validated', 'failed', 'soft_deleted')),
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS artifact_links (
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
  version_id TEXT REFERENCES project_versions(version_id) ON DELETE CASCADE,
  asset_id TEXT NOT NULL REFERENCES concept_assets(asset_id) ON DELETE CASCADE,
  relation TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (project_id, asset_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_projects_profile_status ON projects(profile_id, status, updated_at);
CREATE INDEX IF NOT EXISTS idx_project_versions_project_no ON project_versions(project_id, version_no);
CREATE INDEX IF NOT EXISTS idx_project_versions_parent ON project_versions(parent_version_id);
CREATE INDEX IF NOT EXISTS idx_concept_assets_project_role ON concept_assets(project_id, role, created_at);
CREATE INDEX IF NOT EXISTS idx_concept_assets_sha ON concept_assets(sha256);
CREATE INDEX IF NOT EXISTS idx_module_assets_pack_category ON module_assets(pack_id, category, status);
CREATE INDEX IF NOT EXISTS idx_module_connectors_type ON module_connectors(connector_type, slot);
CREATE INDEX IF NOT EXISTS idx_module_graphs_project_version ON module_graphs(project_id, version_id);
CREATE INDEX IF NOT EXISTS idx_design_briefs_project ON design_briefs(project_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_design_variants_project ON design_variants(project_id, status, rank);
CREATE INDEX IF NOT EXISTS idx_change_sets_base_version ON design_change_sets(base_version_id, status);
CREATE INDEX IF NOT EXISTS idx_quality_runs_version ON quality_runs(version_id, created_at);
CREATE INDEX IF NOT EXISTS idx_quality_findings_run ON quality_findings(quality_run_id, severity);
CREATE INDEX IF NOT EXISTS idx_exports_v2_version ON export_packages_v2(version_id, profile, status);

INSERT INTO schema_migrations(version, name)
VALUES ('0009', 'r2_concept_domain');

COMMIT;
