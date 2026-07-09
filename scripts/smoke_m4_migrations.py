#!/usr/bin/env python3
"""Smoke check for upgrading older M4 SQLite libraries."""

from __future__ import annotations

import json
import sqlite3
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_m4_migration_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        library_root.mkdir(parents=True)
        db_path = library_root / "library.db"

        conn = sqlite3.connect(db_path)
        conn.executescript((ROOT / "migrations" / "0001_init.sql").read_text(encoding="utf-8"))
        conn.execute("DROP INDEX IF EXISTS idx_idempotency_created")
        conn.execute("DROP TABLE IF EXISTS idempotency_records")
        conn.commit()
        conn.close()

        SQLiteAssetStore(library_root=library_root, migrations_dir=ROOT / "migrations")

        conn = sqlite3.connect(db_path)
        tables = {
            row[0]
            for row in conn.execute("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        }
        migrations = {
            row[0]
            for row in conn.execute("SELECT version FROM schema_migrations")
        }
        index = conn.execute(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_idempotency_created'"
        ).fetchone()
        conn.execute(
            """
            INSERT INTO asset_files (
              file_id, role, logical_path, object_path, sha256, byte_size, mime_type, ext, metadata_json
            )
            VALUES (
              'file_concept_patch_check', 'concept_patch', 'check/concept_patch.svg',
              'objects/sha256/check.svg', ?, 0, 'image/svg+xml', '.svg', '{}'
            )
            """,
            ("0" * 64,),
        )
        conn.execute(
            """
            INSERT INTO asset_files (
              file_id, role, logical_path, object_path, sha256, byte_size, mime_type, ext, metadata_json
            )
            VALUES (
              'file_content_reuse_check', 'concept_patch', 'check/concept_patch_reuse.svg',
              'objects/sha256/check.svg', ?, 0, 'image/svg+xml', '.svg', '{}'
            )
            """,
            ("0" * 64,),
        )
        conn.commit()
        conn.close()

        assert "idempotency_records" in tables
        assert "0002" in migrations
        assert "0003" in migrations
        assert "0004" in migrations
        assert "0005" in migrations
        assert "0006" in migrations
        assert "provider_tasks" in tables
        assert "job_checkpoints" in tables
        assert index is not None
        print(json.dumps({"ok": True, "migrations": sorted(migrations)}, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
