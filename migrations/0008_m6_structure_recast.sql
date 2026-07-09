PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS structure_interpretations (
  interpretation_id TEXT PRIMARY KEY,
  weapon_id TEXT NOT NULL REFERENCES weapons(weapon_id) ON DELETE CASCADE,
  source_object TEXT NOT NULL,
  raw_description TEXT NOT NULL,
  status TEXT NOT NULL
    CHECK (status IN ('ready', 'resampled_ready', 'failed', 'confirmed')),
  candidate_count INTEGER NOT NULL CHECK (candidate_count BETWEEN 0 AND 3),
  candidates_json TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  stable_seed INTEGER NOT NULL,
  resample_attempted INTEGER NOT NULL DEFAULT 0 CHECK (resample_attempted IN (0, 1)),
  preserved_candidate_id TEXT,
  candidate_snapshot_hash TEXT NOT NULL,
  failure_code TEXT,
  failure_reason TEXT,
  confirmed_candidate_id TEXT,
  confirmed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS creative_weapon_graphs (
  creative_graph_id TEXT PRIMARY KEY,
  weapon_id TEXT NOT NULL REFERENCES weapons(weapon_id) ON DELETE CASCADE,
  origin_interpretation_id TEXT NOT NULL REFERENCES structure_interpretations(interpretation_id) ON DELETE CASCADE,
  selected_candidate_id TEXT NOT NULL,
  selected_candidate_rank INTEGER NOT NULL CHECK (selected_candidate_rank BETWEEN 1 AND 3),
  graph_json TEXT NOT NULL,
  graph_parent_id TEXT REFERENCES creative_weapon_graphs(creative_graph_id) ON DELETE SET NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS skill_graphs (
  skill_graph_id TEXT PRIMARY KEY,
  weapon_id TEXT NOT NULL REFERENCES weapons(weapon_id) ON DELETE CASCADE,
  origin_graph_id TEXT NOT NULL REFERENCES creative_weapon_graphs(creative_graph_id) ON DELETE CASCADE,
  origin_interpretation_id TEXT NOT NULL REFERENCES structure_interpretations(interpretation_id) ON DELETE CASCADE,
  skill_graph_json TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

ALTER TABLE weapon_versions ADD COLUMN structure_interpretation_id TEXT REFERENCES structure_interpretations(interpretation_id) ON DELETE SET NULL;
ALTER TABLE weapon_versions ADD COLUMN creative_graph_id TEXT REFERENCES creative_weapon_graphs(creative_graph_id) ON DELETE SET NULL;
ALTER TABLE weapon_versions ADD COLUMN skill_graph_id TEXT REFERENCES skill_graphs(skill_graph_id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_structure_interpretations_weapon_id ON structure_interpretations(weapon_id);
CREATE INDEX IF NOT EXISTS idx_structure_interpretations_status ON structure_interpretations(status, updated_at);
CREATE INDEX IF NOT EXISTS idx_creative_graphs_origin_interpretation ON creative_weapon_graphs(origin_interpretation_id);
CREATE INDEX IF NOT EXISTS idx_creative_graphs_weapon_rank ON creative_weapon_graphs(weapon_id, graph_parent_id);
CREATE INDEX IF NOT EXISTS idx_skill_graphs_origin_graph ON skill_graphs(origin_graph_id);
CREATE INDEX IF NOT EXISTS idx_weapon_versions_graph_trace ON weapon_versions(creative_graph_id, skill_graph_id);

INSERT INTO schema_migrations(version, name)
VALUES ('0008', 'm6_structure_recast');

COMMIT;
