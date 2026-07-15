PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS agent_material_texture_objects (
  texture_asset_id TEXT PRIMARY KEY
    CHECK (texture_asset_id GLOB 'asset_tex_[0-9a-f]*'),
  texture_role TEXT NOT NULL
    CHECK (texture_role IN ('base_color', 'normal', 'thumbnail')),
  display_name TEXT NOT NULL,
  mime_type TEXT NOT NULL
    CHECK (mime_type IN ('image/png', 'image/jpeg', 'image/webp')),
  byte_size INTEGER NOT NULL CHECK (byte_size > 0 AND byte_size <= 4000000),
  sha256 TEXT NOT NULL CHECK (sha256 GLOB '[0-9a-f]*' AND length(sha256) = 64),
  object_path TEXT NOT NULL UNIQUE,
  width INTEGER NOT NULL CHECK (width > 0 AND width <= 4096),
  height INTEGER NOT NULL CHECK (height > 0 AND height <= 4096),
  source TEXT NOT NULL
    CHECK (source IN ('forgecad_builtin', 'user_created', 'imported_reference')),
  license TEXT NOT NULL
    CHECK (license IN ('not_applicable', 'self_declared_original', 'third_party', 'unknown')),
  license_ref TEXT,
  thumbnail_asset_id TEXT,
  visual_only INTEGER NOT NULL DEFAULT 1 CHECK (visual_only = 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (
    (source = 'forgecad_builtin' AND license = 'not_applicable')
    OR (source = 'user_created' AND license IN ('self_declared_original', 'unknown'))
    OR (source = 'imported_reference' AND license IN ('third_party', 'unknown'))
  ),
  CHECK (license != 'third_party' OR license_ref IS NOT NULL)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_material_texture_sha_role
  ON agent_material_texture_objects(sha256, texture_role);

CREATE INDEX IF NOT EXISTS idx_agent_material_texture_catalog
  ON agent_material_texture_objects(texture_role, source, updated_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0029', 'agent_material_texture_objects');

COMMIT;
