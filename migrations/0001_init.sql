PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE library_meta (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE idempotency_records (
  scope TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  response_json TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  PRIMARY KEY (scope, idempotency_key)
);

CREATE TABLE weapons (
  weapon_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  weapon_family TEXT NOT NULL,
  fantasy_category TEXT,
  style TEXT NOT NULL DEFAULT '3渲2国风神兵',
  status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'archived', 'soft_deleted')),
  current_version_id TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE generation_jobs (
  job_id TEXT PRIMARY KEY,
  weapon_id TEXT REFERENCES weapons(weapon_id) ON DELETE SET NULL,
  job_type TEXT NOT NULL
    CHECK (job_type IN ('create_weapon', 'patch_image', 'generate_3d', 'export_unity')),
  status TEXT NOT NULL
    CHECK (status IN (
      'created', 'queued', 'running', 'waiting_provider', 'waiting_user',
      'retrying', 'succeeded', 'failed', 'cancelled', 'partial_succeeded'
    )),
  current_step TEXT,
  idempotency_scope TEXT,
  idempotency_key TEXT,
  request_hash TEXT,
  request_json TEXT NOT NULL,
  provider_task_id TEXT,
  retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  finished_at TEXT,
  UNIQUE (idempotency_scope, idempotency_key)
);

CREATE TABLE job_steps (
  step_id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES generation_jobs(job_id) ON DELETE CASCADE,
  step_name TEXT NOT NULL,
  attempt INTEGER NOT NULL DEFAULT 1 CHECK (attempt >= 1),
  status TEXT NOT NULL
    CHECK (status IN ('queued', 'running', 'waiting_provider', 'succeeded', 'failed', 'skipped', 'cancelled')),
  input_hash TEXT,
  output_ref TEXT,
  provider TEXT,
  error_code TEXT,
  error_message TEXT,
  started_at TEXT,
  finished_at TEXT,
  UNIQUE (job_id, step_name, attempt)
);

CREATE TABLE weapon_versions (
  version_id TEXT PRIMARY KEY,
  weapon_id TEXT NOT NULL REFERENCES weapons(weapon_id) ON DELETE CASCADE,
  parent_version_id TEXT REFERENCES weapon_versions(version_id) ON DELETE SET NULL,
  job_id TEXT REFERENCES generation_jobs(job_id) ON DELETE SET NULL,
  version_no INTEGER NOT NULL CHECK (version_no >= 1),
  version_type TEXT NOT NULL
    CHECK (version_type IN ('initial_concept', 'patch', 'model_sheet', 'rough_3d', 'export')),
  status TEXT NOT NULL DEFAULT 'committed'
    CHECK (status IN ('draft', 'committed', 'superseded', 'soft_deleted')),
  summary TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (weapon_id, version_no)
);

CREATE TABLE weapon_specs (
  spec_id TEXT PRIMARY KEY,
  weapon_id TEXT NOT NULL REFERENCES weapons(weapon_id) ON DELETE CASCADE,
  version_id TEXT NOT NULL REFERENCES weapon_versions(version_id) ON DELETE CASCADE,
  schema_version TEXT NOT NULL DEFAULT 'WeaponDesignSpec@1',
  spec_json TEXT NOT NULL,
  spec_sha256 TEXT,
  safety_policy_version TEXT NOT NULL DEFAULT 'safety_boundary@1',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (version_id)
);

CREATE TABLE provider_configs (
  provider_config_id TEXT PRIMARY KEY,
  provider_type TEXT NOT NULL
    CHECK (provider_type IN ('llm', 'comfyui', '3d_provider', 'gltf_optimizer', 'unity')),
  provider_name TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  base_url TEXT,
  secret_ref TEXT,
  config_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (provider_type, provider_name)
);

CREATE TABLE asset_files (
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

CREATE TABLE agent_events (
  event_id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES generation_jobs(job_id) ON DELETE CASCADE,
  seq INTEGER NOT NULL CHECK (seq >= 1),
  weapon_id TEXT REFERENCES weapons(weapon_id) ON DELETE SET NULL,
  step TEXT NOT NULL,
  level TEXT NOT NULL CHECK (level IN ('info', 'warning', 'error')),
  status TEXT NOT NULL CHECK (status IN ('started', 'progress', 'succeeded', 'failed')),
  message TEXT NOT NULL,
  artifact_asset_id TEXT REFERENCES asset_files(file_id) ON DELETE SET NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE (job_id, seq)
);

CREATE TABLE models_3d (
  model_id TEXT PRIMARY KEY,
  weapon_id TEXT NOT NULL REFERENCES weapons(weapon_id) ON DELETE CASCADE,
  version_id TEXT REFERENCES weapon_versions(version_id) ON DELETE SET NULL,
  job_id TEXT REFERENCES generation_jobs(job_id) ON DELETE SET NULL,
  provider TEXT NOT NULL,
  status TEXT NOT NULL
    CHECK (status IN ('submitted', 'raw_archived', 'normalized', 'optimized', 'rough_preview', 'failed')),
  source_image_file_id TEXT REFERENCES asset_files(file_id) ON DELETE SET NULL,
  raw_model_file_id TEXT REFERENCES asset_files(file_id) ON DELETE SET NULL,
  normalized_model_file_id TEXT REFERENCES asset_files(file_id) ON DELETE SET NULL,
  optimized_model_file_id TEXT REFERENCES asset_files(file_id) ON DELETE SET NULL,
  preview_file_id TEXT REFERENCES asset_files(file_id) ON DELETE SET NULL,
  unity_material_file_id TEXT REFERENCES asset_files(file_id) ON DELETE SET NULL,
  orientation_policy_json TEXT NOT NULL DEFAULT '{}',
  quality_report_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE export_packages (
  export_id TEXT PRIMARY KEY,
  weapon_id TEXT NOT NULL REFERENCES weapons(weapon_id) ON DELETE CASCADE,
  version_id TEXT REFERENCES weapon_versions(version_id) ON DELETE SET NULL,
  model_id TEXT REFERENCES models_3d(model_id) ON DELETE SET NULL,
  job_id TEXT REFERENCES generation_jobs(job_id) ON DELETE SET NULL,
  export_type TEXT NOT NULL CHECK (export_type IN ('unity_glb')),
  status TEXT NOT NULL CHECK (status IN ('created', 'validated', 'failed', 'soft_deleted')),
  package_path TEXT NOT NULL,
  manifest_json TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_weapon_versions_weapon ON weapon_versions(weapon_id, version_no);
CREATE INDEX idx_idempotency_created ON idempotency_records(created_at);
CREATE INDEX idx_weapon_versions_parent ON weapon_versions(parent_version_id);
CREATE INDEX idx_weapon_specs_weapon ON weapon_specs(weapon_id);
CREATE INDEX idx_jobs_weapon_status ON generation_jobs(weapon_id, status);
CREATE INDEX idx_jobs_idempotency ON generation_jobs(idempotency_scope, idempotency_key);
CREATE INDEX idx_jobs_status_updated ON generation_jobs(status, updated_at);
CREATE INDEX idx_steps_job ON job_steps(job_id, step_name, attempt);
CREATE INDEX idx_events_job_created ON agent_events(job_id, created_at);
CREATE INDEX idx_events_job_seq ON agent_events(job_id, seq);
CREATE INDEX idx_events_weapon ON agent_events(weapon_id);
CREATE INDEX idx_asset_files_weapon_role ON asset_files(weapon_id, role);
CREATE INDEX idx_asset_files_version ON asset_files(version_id);
CREATE INDEX idx_asset_files_job ON asset_files(job_id);
CREATE INDEX idx_asset_files_sha ON asset_files(sha256);
CREATE INDEX idx_asset_files_soft_deleted ON asset_files(soft_deleted_at);
CREATE INDEX idx_asset_files_sha_object ON asset_files(sha256, object_path);
CREATE INDEX idx_models_weapon_status ON models_3d(weapon_id, status);
CREATE INDEX idx_exports_weapon ON export_packages(weapon_id, export_type);

INSERT INTO schema_migrations(version, name)
VALUES ('0001', 'init');

COMMIT;
