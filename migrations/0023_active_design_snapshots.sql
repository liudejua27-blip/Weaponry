PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- One server-owned state record per project. The two design sources are
-- intentionally mutually exclusive: legacy data stays readable without ever
-- sharing an Agent asset version, preview, quality, selection or export head.
CREATE TABLE active_design_snapshots (
  project_id TEXT PRIMARY KEY REFERENCES projects(project_id) ON DELETE CASCADE,
  source TEXT NOT NULL CHECK (source IN ('agent_asset', 'legacy_concept_read_only')),
  active_asset_version_id TEXT REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  active_assembly_graph_id TEXT,
  legacy_version_id TEXT REFERENCES project_versions(version_id) ON DELETE RESTRICT,
  legacy_module_graph_id TEXT REFERENCES module_graphs(graph_id) ON DELETE RESTRICT,
  selected_part_id TEXT,
  preview_change_set_id TEXT REFERENCES agent_asset_change_sets(change_set_id) ON DELETE RESTRICT,
  preview_base_asset_version_id TEXT REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  quality_report_id TEXT,
  quality_asset_version_id TEXT REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  export_source TEXT NOT NULL CHECK (export_source IN ('agent_asset', 'legacy_concept_read_only')),
  export_source_version_id TEXT NOT NULL,
  revision INTEGER NOT NULL CHECK (revision >= 1),
  updated_at TEXT NOT NULL,
  CHECK (
    (source = 'agent_asset'
      AND active_asset_version_id IS NOT NULL
      AND active_assembly_graph_id IS NOT NULL
      AND legacy_version_id IS NULL
      AND legacy_module_graph_id IS NULL
      AND export_source = 'agent_asset'
      AND export_source_version_id = active_asset_version_id)
    OR
    (source = 'legacy_concept_read_only'
      AND active_asset_version_id IS NULL
      AND active_assembly_graph_id IS NULL
      AND legacy_version_id IS NOT NULL
      AND legacy_module_graph_id IS NOT NULL
      AND selected_part_id IS NULL
      AND preview_change_set_id IS NULL
      AND preview_base_asset_version_id IS NULL
      AND quality_report_id IS NULL
      AND quality_asset_version_id IS NULL
      AND export_source = 'legacy_concept_read_only'
      AND export_source_version_id = legacy_version_id)
  ),
  CHECK (
    (preview_change_set_id IS NULL AND preview_base_asset_version_id IS NULL)
    OR
    (source = 'agent_asset'
      AND preview_change_set_id IS NOT NULL
      AND preview_base_asset_version_id = active_asset_version_id)
  ),
  CHECK (
    (quality_report_id IS NULL AND quality_asset_version_id IS NULL)
    OR
    (source = 'agent_asset'
      AND quality_report_id IS NOT NULL
      AND quality_asset_version_id = active_asset_version_id)
  )
);

CREATE INDEX idx_active_design_snapshots_source_updated
  ON active_design_snapshots(source, updated_at DESC, project_id DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0023', 'active_design_snapshots');

COMMIT;
