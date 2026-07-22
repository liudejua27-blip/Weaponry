-- K003 authoritative schema source. The marker deliberately starts with the
-- Python compatibility owner. Only Rust WriterLease::acquire performs the
-- explicit owner/epoch CAS cutover; running this migration never grants Rust
-- or Python an active writer lease by itself.
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS forgecad_core_schema_migrations (
  version TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forgecad_core_ownership (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  schema_version TEXT NOT NULL CHECK (schema_version = 'ForgeCADCoreOwnership@1'),
  state_owner TEXT NOT NULL CHECK (state_owner IN ('python_compatibility_adapter', 'rust_app_server')),
  active_writer_instance_id TEXT,
  writer_epoch INTEGER NOT NULL DEFAULT 0 CHECK (writer_epoch >= 0),
  acquired_at TEXT,
  released_at TEXT,
  updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO forgecad_core_ownership (
  singleton, schema_version, state_owner, active_writer_instance_id,
  writer_epoch, acquired_at, released_at, updated_at
) VALUES (
  1, 'ForgeCADCoreOwnership@1', 'python_compatibility_adapter', NULL,
  0, NULL, NULL, datetime('now')
);

CREATE TABLE IF NOT EXISTS forgecad_core_objects (
  sha256 TEXT PRIMARY KEY CHECK (length(sha256) = 64),
  object_path TEXT NOT NULL UNIQUE,
  extension TEXT NOT NULL,
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  ref_count INTEGER NOT NULL DEFAULT 0 CHECK (ref_count >= 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forgecad_core_object_references (
  reference_kind TEXT NOT NULL CHECK (reference_kind IN (
    'candidate', 'asset_version', 'quality', 'export', 'preview', 'texture', 'reference'
  )),
  owner_id TEXT NOT NULL,
  role TEXT NOT NULL,
  sha256 TEXT NOT NULL REFERENCES forgecad_core_objects(sha256) ON DELETE RESTRICT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (reference_kind, owner_id, role)
);

-- The legacy candidate table keeps its glb_base64 column for historical row
-- readability. New Rust writes leave it empty and store the authoritative GLB
-- identity here; bytes live only in the content-addressed object library.
CREATE TABLE IF NOT EXISTS forgecad_core_candidate_objects (
  artifact_id TEXT PRIMARY KEY REFERENCES agent_blockout_candidates(artifact_id) ON DELETE CASCADE,
  glb_sha256 TEXT NOT NULL REFERENCES forgecad_core_objects(sha256) ON DELETE RESTRICT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_forgecad_core_object_refs_sha
  ON forgecad_core_object_references(sha256, reference_kind, owner_id);

CREATE TRIGGER IF NOT EXISTS forgecad_core_object_ref_insert
AFTER INSERT ON forgecad_core_object_references
BEGIN
  UPDATE forgecad_core_objects
  SET ref_count = ref_count + 1, updated_at = NEW.created_at
  WHERE sha256 = NEW.sha256;
END;

CREATE TRIGGER IF NOT EXISTS forgecad_core_object_ref_delete
AFTER DELETE ON forgecad_core_object_references
BEGIN
  UPDATE forgecad_core_objects
  SET ref_count = ref_count - 1, updated_at = OLD.created_at
  WHERE sha256 = OLD.sha256;
END;

CREATE TRIGGER IF NOT EXISTS forgecad_core_object_ref_sha_immutable
BEFORE UPDATE OF sha256 ON forgecad_core_object_references
BEGIN
  SELECT RAISE(ABORT, 'content object references are immutable; replace by delete then insert');
END;

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0035', 'k003_rust_core_ownership', datetime('now'));

-- Keeping this ledger entry makes the historical Python runner skip this
-- idempotent schema file on later boots. It does not change state_owner.
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0035', 'k003_rust_core_ownership');

COMMIT;
