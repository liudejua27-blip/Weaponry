PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

-- Imported GLB files stay immutable in the content-addressed object store.
-- The asset version only stores a safe reference contract, so a user upload
-- can never be mistaken for executable ShapeProgram source.
CREATE TABLE agent_imported_glbs (
  import_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  asset_version_id TEXT NOT NULL UNIQUE REFERENCES agent_asset_versions(asset_version_id) ON DELETE RESTRICT,
  domain_pack_id TEXT NOT NULL,
  file_name TEXT NOT NULL,
  object_path TEXT NOT NULL,
  sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
  byte_size INTEGER NOT NULL CHECK (byte_size >= 20),
  triangle_count INTEGER NOT NULL CHECK (triangle_count >= 1),
  bounds_mm_json TEXT NOT NULL CHECK (json_valid(bounds_mm_json)),
  mesh_count INTEGER NOT NULL CHECK (mesh_count >= 1),
  primitive_count INTEGER NOT NULL CHECK (primitive_count >= 1),
  material_count INTEGER NOT NULL CHECK (material_count >= 0),
  node_count INTEGER NOT NULL CHECK (node_count >= 0),
  created_at TEXT NOT NULL
);

CREATE INDEX idx_agent_imported_glbs_project_created
  ON agent_imported_glbs(project_id, created_at DESC, import_id DESC);

INSERT INTO schema_migrations(version, name)
VALUES ('0022', 'agent_external_glb_import');

COMMIT;
