-- K003 historical artifact adoption ledger. This migration deliberately does
-- not rewrite legacy rows or promote historical preview/reference bytes to a
-- production_concept artifact. Rust records every decision so interrupted
-- adoption can resume without a second product-state truth.
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS forgecad_core_artifact_migration_runs (
  run_id TEXT PRIMARY KEY,
  schema_version TEXT NOT NULL
    CHECK (schema_version = 'ForgeCADArtifactMigrationRun@1'),
  writer_epoch INTEGER NOT NULL CHECK (writer_epoch >= 1),
  state TEXT NOT NULL
    CHECK (state IN ('inventory', 'migrating', 'ready', 'blocked', 'failed')),
  inventory_sha256 TEXT NOT NULL CHECK (length(inventory_sha256) = 64),
  semantic_sha256_before TEXT NOT NULL CHECK (length(semantic_sha256_before) = 64),
  semantic_sha256_after TEXT CHECK (
    semantic_sha256_after IS NULL OR length(semantic_sha256_after) = 64
  ),
  total_items INTEGER NOT NULL CHECK (total_items >= 0),
  migrated_items INTEGER NOT NULL DEFAULT 0 CHECK (migrated_items >= 0),
  legacy_read_only_items INTEGER NOT NULL DEFAULT 0
    CHECK (legacy_read_only_items >= 0),
  retryable_error_items INTEGER NOT NULL DEFAULT 0
    CHECK (retryable_error_items >= 0),
  error_code TEXT,
  started_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  CHECK (migrated_items + legacy_read_only_items + retryable_error_items <= total_items),
  CHECK ((state = 'ready' AND semantic_sha256_after IS NOT NULL AND completed_at IS NOT NULL)
    OR state != 'ready')
);

CREATE TABLE IF NOT EXISTS forgecad_core_artifact_migration_items (
  migration_key TEXT PRIMARY KEY CHECK (length(migration_key) = 64),
  run_id TEXT NOT NULL
    REFERENCES forgecad_core_artifact_migration_runs(run_id) ON DELETE RESTRICT,
  source_kind TEXT NOT NULL CHECK (source_kind IN (
    'agent_candidate_inline_glb',
    'agent_production_cache_glb',
    'agent_imported_glb',
    'agent_quality_readback',
    'agent_material_texture',
    'legacy_asset_file',
    'legacy_concept_asset',
    'agent_version_compile'
  )),
  source_id TEXT NOT NULL,
  source_fingerprint_sha256 TEXT NOT NULL
    CHECK (length(source_fingerprint_sha256) = 64),
  target_reference_kind TEXT NOT NULL CHECK (target_reference_kind IN (
    'candidate', 'asset_version', 'quality', 'export', 'preview', 'texture', 'reference'
  )),
  target_owner_id TEXT NOT NULL,
  target_role TEXT NOT NULL,
  outcome TEXT NOT NULL
    CHECK (outcome IN ('pending', 'migrated', 'legacy_read_only', 'retryable_error')),
  object_sha256 TEXT REFERENCES forgecad_core_objects(sha256) ON DELETE RESTRICT,
  artifact_profile_id TEXT CHECK (
    artifact_profile_id IS NULL OR artifact_profile_id IN (
      'interactive_preview', 'production_concept', 'external_reference', 'legacy'
    )
  ),
  readback_sha256 TEXT CHECK (
    readback_sha256 IS NULL OR length(readback_sha256) = 64
  ),
  reason_code TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  UNIQUE (
    source_kind, source_id, target_reference_kind, target_owner_id, target_role
  ),
  CHECK (
    (outcome = 'migrated' AND object_sha256 IS NOT NULL AND reason_code IS NULL)
    OR (outcome = 'legacy_read_only' AND object_sha256 IS NULL AND reason_code IS NOT NULL)
    OR (outcome = 'retryable_error' AND reason_code IS NOT NULL)
    OR outcome = 'pending'
  )
);

CREATE INDEX IF NOT EXISTS idx_forgecad_core_artifact_migration_items_run
  ON forgecad_core_artifact_migration_items(run_id, outcome, migration_key);
CREATE INDEX IF NOT EXISTS idx_forgecad_core_artifact_migration_items_object
  ON forgecad_core_artifact_migration_items(object_sha256, target_reference_kind, target_owner_id);

CREATE TABLE IF NOT EXISTS forgecad_core_artifact_migration_state (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  cohort_captured_run_id TEXT
    REFERENCES forgecad_core_artifact_migration_runs(run_id) ON DELETE RESTRICT,
  cohort_captured_at TEXT,
  CHECK ((cohort_captured_run_id IS NULL AND cohort_captured_at IS NULL)
    OR (cohort_captured_run_id IS NOT NULL AND cohort_captured_at IS NOT NULL))
);

INSERT OR IGNORE INTO forgecad_core_artifact_migration_state(
  singleton, cohort_captured_run_id, cohort_captured_at
) VALUES (1, NULL, NULL);

-- Freeze the exact Version cohort that existed when Rust first adopted the
-- library. Later Rust-native versions are authoritative at creation time and
-- must never be reclassified as historical data on a subsequent boot.
CREATE TABLE IF NOT EXISTS forgecad_core_asset_migration_cohort (
  asset_version_id TEXT PRIMARY KEY
    REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  captured_run_id TEXT NOT NULL
    REFERENCES forgecad_core_artifact_migration_runs(run_id) ON DELETE RESTRICT,
  source_shape_program_sha256 TEXT NOT NULL
    CHECK (length(source_shape_program_sha256) = 64),
  captured_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forgecad_core_asset_migration_status (
  asset_version_id TEXT PRIMARY KEY
    REFERENCES agent_asset_versions(asset_version_id) ON DELETE CASCADE,
  schema_version TEXT NOT NULL
    CHECK (schema_version = 'ForgeCADAssetMigrationStatus@1'),
  shape_program_sha256 TEXT NOT NULL CHECK (length(shape_program_sha256) = 64),
  state TEXT NOT NULL CHECK (state IN (
    'ready',
    'external_reference_read_only',
    'legacy_read_only',
    'artifact_recompile_required'
  )),
  reason_code TEXT,
  updated_at TEXT NOT NULL,
  CHECK ((state = 'ready' AND reason_code IS NULL)
    OR (state != 'ready' AND reason_code IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_forgecad_core_asset_migration_status_state
  ON forgecad_core_asset_migration_status(state, updated_at, asset_version_id);

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0036', 'k003_legacy_artifact_adoption', datetime('now'));

-- Keep the historical Python runner from trying to interpret this Rust-owned
-- idempotent schema file. This marker does not migrate or mutate legacy data.
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0036', 'k003_legacy_artifact_adoption');

COMMIT;
