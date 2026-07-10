#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import sqlite3
import tempfile
from pathlib import Path

from forgecad_agent.api import LocalApiSettings, create_local_api
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner
from forgecad_agent.infrastructure.storage import ContentAddressedStore, ObjectStoreError


ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad-r1-") as temporary_directory:
        root = Path(temporary_directory)
        library_root = root / "library"
        library_root.mkdir(parents=True)
        connection_factory = SQLiteConnectionFactory(library_root / "library.db")
        migration_runner = SQLiteMigrationRunner(connection_factory, ROOT / "migrations")

        first_apply = migration_runner.run()
        second_apply = migration_runner.run()
        assert first_apply, "fresh database should apply migrations"
        assert second_apply == [], "migration runner must be idempotent"

        with connection_factory.connect() as connection:
            foreign_keys = connection.execute("PRAGMA foreign_keys").fetchone()[0]
            busy_timeout = connection.execute("PRAGMA busy_timeout").fetchone()[0]
            migration_count = connection.execute("SELECT COUNT(*) FROM schema_migrations").fetchone()[0]
        assert foreign_keys == 1
        assert busy_timeout == 5000
        assert migration_count == len(list((ROOT / "migrations").glob("*.sql")))

        object_store = ContentAddressedStore(library_root)
        payload = b"forgecad-r1-content-addressed-object"
        stored = object_store.put(payload, extension=".bin")
        duplicate = object_store.put(payload, extension="bin")
        expected_hash = hashlib.sha256(payload).hexdigest()

        assert stored.sha256 == expected_hash
        assert duplicate.relative_path == stored.relative_path
        assert object_store.read(stored.relative_path, expected_sha256=expected_hash) == payload

        try:
            object_store.resolve("../outside")
        except ObjectStoreError as exc:
            assert exc.code == "OBJECT_PATH_DENIED"
        else:
            raise AssertionError("object store must reject paths outside the library")

        object_path = object_store.resolve(stored.relative_path)
        object_path.write_bytes(b"tampered")
        try:
            object_store.read(stored.relative_path, expected_sha256=expected_hash)
        except ObjectStoreError as exc:
            assert exc.code == "OBJECT_HASH_MISMATCH"
        else:
            raise AssertionError("object store must reject content hash drift")

        api_settings = LocalApiSettings.from_env(
            environ={"FORGECAD_CORS_ORIGINS": "http://127.0.0.1:5199/"}
        )
        assert "http://127.0.0.1:5199" in api_settings.cors_origins
        assert "http://127.0.0.1:5173" in api_settings.cors_origins
        assert create_local_api(api_settings).title == "ForgeCAD Local Agent"

    print(
        {
            "ok": True,
            "migration_count": migration_count,
            "fresh_applied": first_apply,
            "object_sha256": expected_hash,
        }
    )


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, sqlite3.Error) as exc:
        raise SystemExit(f"R1 foundation smoke failed: {exc}") from exc
