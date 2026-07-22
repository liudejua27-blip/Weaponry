PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS agent_skill_versions (
  skill_id TEXT NOT NULL,
  version INTEGER NOT NULL CHECK (version >= 1),
  manifest_json TEXT NOT NULL,
  manifest_sha256 TEXT NOT NULL CHECK (length(manifest_sha256) = 64),
  manifest_object_sha256 TEXT NOT NULL CHECK (manifest_object_sha256 = manifest_sha256) REFERENCES forgecad_core_objects(sha256) ON DELETE RESTRICT,
  status TEXT NOT NULL CHECK (status IN ('draft', 'evaluated', 'disabled')),
  created_at TEXT NOT NULL,
  evaluated_at TEXT,
  PRIMARY KEY (skill_id, version),
  UNIQUE (skill_id, version, manifest_sha256)
);

CREATE TABLE IF NOT EXISTS agent_skill_activations (
  skill_id TEXT PRIMARY KEY,
  activation_id TEXT NOT NULL UNIQUE,
  skill_version INTEGER NOT NULL,
  skill_sha256 TEXT NOT NULL CHECK (length(skill_sha256) = 64),
  enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
  updated_at TEXT NOT NULL,
  FOREIGN KEY (skill_id, skill_version) REFERENCES agent_skill_versions(skill_id, version) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS agent_skill_eval_reports (
  report_id TEXT PRIMARY KEY,
  skill_id TEXT NOT NULL,
  skill_version INTEGER NOT NULL,
  skill_sha256 TEXT NOT NULL CHECK (length(skill_sha256) = 64),
  status TEXT NOT NULL CHECK (status IN ('passed', 'failed')),
  findings_json TEXT NOT NULL,
  evaluated_at TEXT NOT NULL,
  FOREIGN KEY (skill_id, skill_version) REFERENCES agent_skill_versions(skill_id, version) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS agent_asset_skill_references (
  asset_version_id TEXT NOT NULL REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  skill_id TEXT NOT NULL,
  skill_version INTEGER NOT NULL,
  skill_sha256 TEXT NOT NULL CHECK (length(skill_sha256) = 64),
  recorded_at TEXT NOT NULL,
  PRIMARY KEY (asset_version_id, skill_id, skill_version),
  FOREIGN KEY (skill_id, skill_version) REFERENCES agent_skill_versions(skill_id, version) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_agent_skill_eval_lookup ON agent_skill_eval_reports(skill_id, skill_version, status);
CREATE INDEX IF NOT EXISTS idx_agent_asset_skill_lookup ON agent_asset_skill_references(skill_id, skill_version);

-- Skill manifest identity is append-only.  Evaluation status is the sole
-- mutable field on this row, and activation remains in its own pointer table.
CREATE TRIGGER IF NOT EXISTS agent_skill_manifest_identity_immutable
BEFORE UPDATE OF skill_id, version, manifest_json, manifest_sha256, manifest_object_sha256
ON agent_skill_versions
BEGIN
  SELECT RAISE(ABORT, 'Skill manifest identity is immutable');
END;

CREATE TRIGGER IF NOT EXISTS agent_skill_eval_immutable
BEFORE UPDATE ON agent_skill_eval_reports
BEGIN
  SELECT RAISE(ABORT, 'Skill evaluation reports are immutable');
END;

CREATE TRIGGER IF NOT EXISTS agent_asset_skill_reference_immutable
BEFORE UPDATE ON agent_asset_skill_references
BEGIN
  SELECT RAISE(ABORT, 'Asset Skill provenance is immutable');
END;

CREATE TRIGGER IF NOT EXISTS agent_skill_activation_sealed_insert
BEFORE INSERT ON agent_skill_activations
WHEN NOT EXISTS (
  SELECT 1 FROM agent_skill_versions v
  WHERE v.skill_id = NEW.skill_id
    AND v.version = NEW.skill_version
    AND v.manifest_sha256 = NEW.skill_sha256
)
BEGIN
  SELECT RAISE(ABORT, 'Skill activation must reference a sealed manifest hash');
END;

CREATE TRIGGER IF NOT EXISTS agent_skill_activation_sealed_update
BEFORE UPDATE OF skill_version, skill_sha256 ON agent_skill_activations
WHEN NOT EXISTS (
  SELECT 1 FROM agent_skill_versions v
  WHERE v.skill_id = NEW.skill_id
    AND v.version = NEW.skill_version
    AND v.manifest_sha256 = NEW.skill_sha256
)
BEGIN
  SELECT RAISE(ABORT, 'Skill activation must reference a sealed manifest hash');
END;

CREATE TRIGGER IF NOT EXISTS agent_asset_skill_reference_sealed_insert
BEFORE INSERT ON agent_asset_skill_references
WHEN NOT EXISTS (
  SELECT 1 FROM agent_skill_versions v
  WHERE v.skill_id = NEW.skill_id
    AND v.version = NEW.skill_version
    AND v.manifest_sha256 = NEW.skill_sha256
)
BEGIN
  SELECT RAISE(ABORT, 'Asset Skill provenance must reference a sealed manifest hash');
END;

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0038', 'agent_skills', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0038', 'agent_skills');

COMMIT;
