-- Persist the logical undo/redo cursor for immutable Agent asset versions.
-- A navigation action always creates a new AgentAssetVersion; this table only
-- records which historical content is next in either direction.

CREATE TABLE IF NOT EXISTS agent_asset_navigation_frames (
  resulting_asset_version_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  undo_target_asset_version_id TEXT,
  redo_target_asset_version_id TEXT,
  action TEXT NOT NULL CHECK (action IN ('undo', 'redo')),
  created_at TEXT NOT NULL,
  FOREIGN KEY (project_id) REFERENCES projects(project_id) ON DELETE CASCADE,
  FOREIGN KEY (resulting_asset_version_id) REFERENCES agent_asset_versions(asset_version_id) ON DELETE CASCADE,
  FOREIGN KEY (undo_target_asset_version_id) REFERENCES agent_asset_versions(asset_version_id) ON DELETE SET NULL,
  FOREIGN KEY (redo_target_asset_version_id) REFERENCES agent_asset_versions(asset_version_id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_asset_navigation_frames_project
  ON agent_asset_navigation_frames(project_id, created_at DESC);
