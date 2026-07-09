PRAGMA foreign_keys = OFF;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS asset_files_new (
  file_id TEXT PRIMARY KEY,
  weapon_id TEXT REFERENCES weapons(weapon_id) ON DELETE CASCADE,
  version_id TEXT REFERENCES weapon_versions(version_id) ON DELETE SET NULL,
  job_id TEXT REFERENCES generation_jobs(job_id) ON DELETE SET NULL,
  role TEXT NOT NULL
    CHECK (role IN (
      'sketch', 'reference_image', 'weapon_spec', 'concept_image', 'concept_patch',
      'prompt', 'negative_prompt', 'comfyui_workflow', 'patch_mask',
      'patch_manifest', 'patch_prompt', 'quality_report',
      'model_sheet_image', 'rough_raw_glb', 'rough_normalized_glb',
      'rough_optimized_glb', 'rough_preview_png', 'unity_material_json',
      'unity_import_report', 'unity_export_package', 'texture', 'other'
    )),
  logical_path TEXT NOT NULL,
  object_path TEXT NOT NULL,
  sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  mime_type TEXT NOT NULL,
  ext TEXT NOT NULL,
  width INTEGER CHECK (width IS NULL OR width > 0),
  height INTEGER CHECK (height IS NULL OR height > 0),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  soft_deleted_at TEXT
);

INSERT INTO asset_files_new (
  file_id, weapon_id, version_id, job_id, role, logical_path, object_path,
  sha256, byte_size, mime_type, ext, width, height, metadata_json, created_at, soft_deleted_at
)
SELECT
  file_id, weapon_id, version_id, job_id, role, logical_path, object_path,
  sha256, byte_size, mime_type, ext, width, height, metadata_json, created_at, soft_deleted_at
FROM asset_files;

DROP TABLE asset_files;
ALTER TABLE asset_files_new RENAME TO asset_files;

CREATE INDEX IF NOT EXISTS idx_asset_files_weapon_role ON asset_files(weapon_id, role);
CREATE INDEX IF NOT EXISTS idx_asset_files_version ON asset_files(version_id);
CREATE INDEX IF NOT EXISTS idx_asset_files_job ON asset_files(job_id);
CREATE INDEX IF NOT EXISTS idx_asset_files_sha ON asset_files(sha256);
CREATE INDEX IF NOT EXISTS idx_asset_files_soft_deleted ON asset_files(soft_deleted_at);
CREATE INDEX IF NOT EXISTS idx_asset_files_sha_object ON asset_files(sha256, object_path);

INSERT INTO schema_migrations(version, name)
VALUES ('0003', 'm4_concept_patch_role');

COMMIT;

PRAGMA foreign_keys = ON;
