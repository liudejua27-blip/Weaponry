PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

DROP INDEX IF EXISTS idx_asset_files_sha_object;
CREATE INDEX IF NOT EXISTS idx_asset_files_sha_object ON asset_files(sha256, object_path);

INSERT INTO schema_migrations(version, name)
VALUES ('0004', 'm4_asset_file_content_reuse');

COMMIT;
