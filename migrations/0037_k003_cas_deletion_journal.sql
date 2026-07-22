-- Durable K003 CAS deletion journal. SQLite records the exact verified object
-- identity before its index row is removed; startup can then finish an
-- interrupted unlink without scanning or deleting unrelated library files.
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS forgecad_core_object_deletion_journal (
  sha256 TEXT PRIMARY KEY CHECK (length(sha256) = 64),
  object_path TEXT NOT NULL,
  extension TEXT NOT NULL,
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_forgecad_core_object_deletion_created
  ON forgecad_core_object_deletion_journal(created_at, sha256);

INSERT OR IGNORE INTO forgecad_core_schema_migrations(version, name, applied_at)
VALUES ('0037', 'k003_cas_deletion_journal', datetime('now'));

-- Prevent the historical Python runner from attempting to execute this
-- Rust-owned schema migration. No legacy product row is rewritten.
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('0037', 'k003_cas_deletion_journal');

COMMIT;
