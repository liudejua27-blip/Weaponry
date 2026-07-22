PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS reference_evidence (
  evidence_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE RESTRICT,
  client_request_id TEXT NOT NULL,
  request_sha256 TEXT NOT NULL CHECK (length(request_sha256) = 64),
  kind TEXT NOT NULL CHECK (kind IN ('image', 'glb')),
  domain_pack_id TEXT NOT NULL,
  source_file_name TEXT NOT NULL,
  source_media_type TEXT NOT NULL,
  source_object_sha256 TEXT NOT NULL REFERENCES forgecad_core_objects(sha256) ON DELETE RESTRICT,
  source_imported_asset_version_id TEXT REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  source_statement TEXT NOT NULL,
  license_statement TEXT NOT NULL,
  missing_views_json TEXT NOT NULL,
  user_notes TEXT NOT NULL,
  observations_json TEXT NOT NULL,
  glb_inspection_json TEXT,
  created_at TEXT NOT NULL,
  CHECK ((kind = 'image' AND source_imported_asset_version_id IS NULL AND glb_inspection_json IS NULL) OR (kind = 'glb' AND glb_inspection_json IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS reference_guided_rebuild_plans (
  rebuild_plan_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE RESTRICT,
  evidence_id TEXT NOT NULL REFERENCES reference_evidence(evidence_id) ON DELETE RESTRICT,
  base_asset_version_id TEXT REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  domain_pack_id TEXT NOT NULL,
  recipe_id TEXT NOT NULL,
  recipe_registry_sha256 TEXT NOT NULL CHECK (length(recipe_registry_sha256) = 64),
  rebuild_summary TEXT NOT NULL,
  intended_differences_json TEXT NOT NULL,
  retained_evidence_json TEXT NOT NULL,
  unresolved_uncertainties_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('draft', 'previewed', 'confirmed', 'rejected')),
  preview_change_set_id TEXT REFERENCES agent_asset_change_sets(change_set_id) ON DELETE RESTRICT,
  confirmed_asset_version_id TEXT REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK ((status = 'draft' AND preview_change_set_id IS NULL AND confirmed_asset_version_id IS NULL) OR (status IN ('previewed', 'rejected') AND preview_change_set_id IS NOT NULL AND confirmed_asset_version_id IS NULL) OR (status = 'confirmed' AND preview_change_set_id IS NOT NULL AND confirmed_asset_version_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_reference_evidence_project_created ON reference_evidence(project_id, created_at, evidence_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_reference_evidence_project_idempotency ON reference_evidence(project_id, client_request_id);
CREATE INDEX IF NOT EXISTS idx_reference_rebuild_plan_project ON reference_guided_rebuild_plans(project_id, created_at, rebuild_plan_id);

-- Evidence describes an immutable user-authorized source.  Its observations
-- never overwrite original bytes, and a referenced source cannot be detached
-- through generic CAS cleanup.
CREATE TRIGGER IF NOT EXISTS reference_evidence_immutable
BEFORE UPDATE ON reference_evidence
BEGIN
  SELECT RAISE(ABORT, 'Reference evidence is immutable');
END;

CREATE TRIGGER IF NOT EXISTS reference_evidence_source_delete_protected
BEFORE DELETE ON forgecad_core_object_references
WHEN OLD.reference_kind = 'reference'
 AND OLD.role = 'reference_evidence_source'
 AND EXISTS (SELECT 1 FROM reference_evidence WHERE evidence_id = OLD.owner_id)
BEGIN
  SELECT RAISE(ABORT, 'Reference evidence source is immutable');
END;

CREATE TRIGGER IF NOT EXISTS reference_rebuild_plan_identity_immutable
BEFORE UPDATE OF rebuild_plan_id, project_id, evidence_id, base_asset_version_id, domain_pack_id,
                 recipe_id, recipe_registry_sha256, rebuild_summary, intended_differences_json,
                 retained_evidence_json, unresolved_uncertainties_json, created_at
ON reference_guided_rebuild_plans
BEGIN
  SELECT RAISE(ABORT, 'Reference rebuild plan identity is immutable');
END;

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0039', 'reference_evidence', datetime('now'));
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0039', 'reference_evidence');

COMMIT;
