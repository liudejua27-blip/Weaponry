from __future__ import annotations

import hashlib
import json
import sqlite3
from pathlib import Path

import pytest

import library_backup
from library_backup import (
    LibraryBackupError,
    create_backup,
    restore_backup,
    verify_backup,
)


CORE_OBJECTS = """
CREATE TABLE forgecad_core_objects (
  sha256 TEXT PRIMARY KEY,
  object_path TEXT NOT NULL UNIQUE,
  extension TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  ref_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE forgecad_core_object_references (
  reference_kind TEXT NOT NULL,
  owner_id TEXT NOT NULL,
  role TEXT NOT NULL,
  sha256 TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (reference_kind, owner_id, role)
);
CREATE TABLE forgecad_core_object_deletion_journal (
  sha256 TEXT PRIMARY KEY,
  object_path TEXT NOT NULL,
  extension TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  created_at TEXT NOT NULL
);
"""


def _object_path(sha256: str, extension: str = "bin") -> str:
    return f"objects/sha256/{sha256[:2]}/{sha256[2:4]}/{sha256}.{extension}"


def _write_object(root: Path, payload: bytes, extension: str = "bin") -> tuple[str, str]:
    sha256 = hashlib.sha256(payload).hexdigest()
    relative = _object_path(sha256, extension)
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    return sha256, relative


def _seed_core_library(root: Path) -> dict[str, str]:
    root.mkdir(parents=True)
    database = root / "library.db"
    shared_sha, shared_path = _write_object(root, b"shared-preview-production", "glb")
    texture_sha, texture_path = _write_object(root, b"texture-bytes", "png")
    with sqlite3.connect(database) as connection:
        connection.execute("PRAGMA journal_mode=WAL")
        connection.execute("PRAGMA wal_autocheckpoint=0")
        connection.executescript(CORE_OBJECTS)
        connection.execute(
            "CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, name TEXT)"
        )
        connection.execute("INSERT INTO schema_migrations VALUES ('0035', 'k003')")
        connection.execute(
            "INSERT INTO forgecad_core_objects VALUES (?, ?, 'glb', ?, 7, 't', 't')",
            (shared_sha, shared_path, len(b"shared-preview-production")),
        )
        connection.execute(
            "INSERT INTO forgecad_core_objects VALUES (?, ?, 'png', ?, 1, 't', 't')",
            (texture_sha, texture_path, len(b"texture-bytes")),
        )
        for kind, owner, role in (
            ("candidate", "candidate-1", "production_glb"),
            ("asset_version", "asset-1", "production_glb"),
            ("quality", "quality-1", "report"),
            ("export", "export-1", "production_glb"),
            ("preview", "preview-1", "interactive_preview_glb"),
            ("reference", "reference-1", "external_reference_glb"),
            ("asset_version", "asset-1", "interactive_preview_glb"),
        ):
            connection.execute(
                "INSERT INTO forgecad_core_object_references VALUES (?, ?, ?, ?, 't')",
                (kind, owner, role, shared_sha),
            )
        connection.execute(
            "INSERT INTO forgecad_core_object_references VALUES (?, ?, ?, ?, 't')",
            ("texture", "texture-1", "base_color", texture_sha),
        )
        connection.commit()
    return {
        "shared_sha": shared_sha,
        "shared_path": shared_path,
        "texture_sha": texture_sha,
        "texture_path": texture_path,
    }


def _expect_code(code: str, callback) -> None:
    with pytest.raises(LibraryBackupError) as error:
        callback()
    assert error.value.code == code
    assert "/Users/" not in str(error.value)


def test_core_round_trip_shared_hash_and_idempotent_rejection(tmp_path: Path) -> None:
    source = tmp_path / "source"
    seeded = _seed_core_library(source)
    backup = tmp_path / "backup"

    result = create_backup(source, backup)
    assert result["verification"]["ok"] is True
    manifest = json.loads((backup / "backup-manifest.json").read_text())
    assert len(manifest["objects"]) == 2
    shared = next(item for item in manifest["objects"] if item["sha256"] == seeded["shared_sha"])
    assert shared["reference_count"] == 7
    assert shared["ref_count"] == 7
    assert set(shared["source_kinds"]) == {
        "candidate",
        "asset_version",
        "quality",
        "export",
        "preview",
        "reference",
    }
    assert manifest["capacity"]["unique_object_count"] == 2
    assert manifest["capacity"]["logical_object_bytes"] > manifest["capacity"]["unique_object_bytes"]
    assert verify_backup(backup)["ok"] is True

    restored = tmp_path / "restored"
    restored_result = restore_backup(backup, restored)
    assert restored_result["restored_verification"]["integrity_check"] == "ok"
    assert (restored / seeded["shared_path"]).read_bytes() == b"shared-preview-production"
    assert (restored / seeded["texture_path"]).read_bytes() == b"texture-bytes"
    with sqlite3.connect(restored / "library.db") as connection:
        assert connection.execute(
            "SELECT COUNT(*) FROM forgecad_core_object_references"
        ).fetchone()[0] == 8
    _expect_code("BACKUP_DESTINATION_EXISTS", lambda: create_backup(source, backup))


def test_core_missing_tampered_size_and_ref_count_fail(tmp_path: Path) -> None:
    source = tmp_path / "source"
    seeded = _seed_core_library(source)
    with sqlite3.connect(source / "library.db") as connection:
        connection.execute(
            "UPDATE forgecad_core_objects SET ref_count = 99 WHERE sha256 = ?",
            (seeded["shared_sha"],),
        )
        connection.commit()
    _expect_code("CORE_REF_COUNT_MISMATCH", lambda: create_backup(source, tmp_path / "backup"))

    source = tmp_path / "missing"
    seeded = _seed_core_library(source)
    (source / seeded["shared_path"]).unlink()
    _expect_code("BACKUP_FILE_MISSING", lambda: create_backup(source, tmp_path / "missing-backup"))

    source = tmp_path / "tampered"
    seeded = _seed_core_library(source)
    original_size = len(b"shared-preview-production")
    (source / seeded["shared_path"]).write_bytes(b"x" * original_size)
    _expect_code("BACKUP_HASH_MISMATCH", lambda: create_backup(source, tmp_path / "tampered-backup"))

    source = tmp_path / "wrong-size"
    seeded = _seed_core_library(source)
    (source / seeded["shared_path"]).write_bytes(b"short")
    _expect_code("BACKUP_SIZE_MISMATCH", lambda: create_backup(source, tmp_path / "wrong-size-backup"))


def test_core_orphan_missing_reference_and_path_safety_fail(tmp_path: Path) -> None:
    source = tmp_path / "orphan"
    seeded = _seed_core_library(source)
    orphan_sha, orphan_path = _write_object(source, b"orphan", "bin")
    with sqlite3.connect(source / "library.db") as connection:
        connection.execute(
            "INSERT INTO forgecad_core_objects VALUES (?, ?, 'bin', 6, 0, 't', 't')",
            (orphan_sha, orphan_path),
        )
        connection.commit()
    _expect_code("CORE_ORPHAN_OBJECT", lambda: create_backup(source, tmp_path / "orphan-backup"))

    source = tmp_path / "missing-ref"
    seeded = _seed_core_library(source)
    missing_sha = "f" * 64
    with sqlite3.connect(source / "library.db") as connection:
        connection.execute("PRAGMA foreign_keys=OFF")
        connection.execute(
            "INSERT INTO forgecad_core_object_references VALUES ('export', 'missing', 'glb', ?, 't')",
            (missing_sha,),
        )
        connection.commit()
    _expect_code("CORE_REFERENCE_OBJECT_MISSING", lambda: create_backup(source, tmp_path / "missing-ref-backup"))

    source = tmp_path / "unsafe"
    seeded = _seed_core_library(source)
    with sqlite3.connect(source / "library.db") as connection:
        connection.execute(
            "UPDATE forgecad_core_objects SET object_path = ? WHERE sha256 = ?",
            ("objects/sha256/../outside.bin", seeded["shared_sha"]),
        )
        connection.commit()
    _expect_code("OBJECT_PATH_INVALID", lambda: create_backup(source, tmp_path / "unsafe-backup"))


def test_core_symlink_unknown_schema_and_atomic_restore_failure(tmp_path: Path) -> None:
    source = tmp_path / "symlink"
    seeded = _seed_core_library(source)
    target = source / seeded["shared_path"]
    real = target.with_suffix(".real")
    target.rename(real)
    target.symlink_to(real.name)
    _expect_code("OBJECT_STORE_SYMLINK", lambda: create_backup(source, tmp_path / "symlink-backup"))

    source = tmp_path / "unknown-schema"
    _seed_core_library(source)
    with sqlite3.connect(source / "library.db") as connection:
        connection.execute("CREATE TABLE future_object_refs (object_path TEXT)")
        connection.commit()
    _expect_code("UNSUPPORTED_OBJECT_REFERENCE_TABLE", lambda: create_backup(source, tmp_path / "unknown-backup"))

    source = tmp_path / "restore-source"
    _seed_core_library(source)
    backup = tmp_path / "restore-backup"
    create_backup(source, backup)
    manifest_path = backup / "backup-manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["objects"][0]["byte_size"] += 1
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    destination = tmp_path / "should-not-exist"
    _expect_code("BACKUP_OBJECT_METADATA_MISMATCH", lambda: restore_backup(backup, destination))
    assert not destination.exists()


def test_core_wal_snapshot_and_interrupted_backup_leave_no_partial_output(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = tmp_path / "wal-source"
    _seed_core_library(source)
    assert (source / "library.db-wal").exists()
    backup = tmp_path / "wal-backup"
    create_backup(source, backup)
    assert not (backup / "library.db-wal").exists()
    assert not (backup / "library.db-shm").exists()
    assert verify_backup(backup)["journal_mode"] == "delete"

    interrupted_source = tmp_path / "interrupted-source"
    _seed_core_library(interrupted_source)
    output = tmp_path / "interrupted-backup"
    original_copy2 = library_backup.shutil.copy2
    calls = 0

    def fail_after_first_copy(*args, **kwargs):
        nonlocal calls
        calls += 1
        if calls == 2:
            raise OSError("injected backup interruption")
        return original_copy2(*args, **kwargs)

    monkeypatch.setattr(library_backup.shutil, "copy2", fail_after_first_copy)
    with pytest.raises(OSError, match="injected backup interruption"):
        create_backup(interrupted_source, output)
    assert not output.exists()
    assert not list(tmp_path.glob(".interrupted-backup.tmp-*"))


def test_legacy_object_tables_keep_compatibility_semantics(tmp_path: Path) -> None:
    source = tmp_path / "legacy-source"
    source.mkdir()
    payload = b"legacy-compatible-object"
    sha256, relative = _write_object(source, payload, "bin")
    orphan_sha, orphan_relative = _write_object(source, b"legacy-orphan", "bin")
    with sqlite3.connect(source / "library.db") as connection:
        connection.executescript(
            """
            CREATE TABLE asset_files(
              file_id TEXT PRIMARY KEY, object_path TEXT, sha256 TEXT, byte_size INTEGER
            );
            CREATE TABLE concept_assets(
              asset_id TEXT PRIMARY KEY, object_path TEXT, sha256 TEXT, byte_size INTEGER
            );
            CREATE TABLE agent_imported_glbs(
              import_id TEXT PRIMARY KEY, object_path TEXT, sha256 TEXT, byte_size INTEGER
            );
            """
        )
        connection.execute(
            "INSERT INTO asset_files VALUES ('legacy-file', ?, ?, ?)",
            (relative, sha256, len(payload)),
        )
        connection.execute(
            "INSERT INTO concept_assets VALUES ('legacy-asset', ?, ?, ?)",
            (relative, sha256, len(payload)),
        )
        connection.execute(
            "INSERT INTO agent_imported_glbs VALUES ('legacy-glb', ?, ?, ?)",
            (relative, sha256, len(payload)),
        )
        connection.commit()
    backup = tmp_path / "legacy-backup"
    result = create_backup(source, backup)
    assert result["capacity"]["reference_rows"] == 3
    assert result["capacity"]["unique_object_count"] == 1
    assert result["capacity"]["unreferenced_candidate_count"] == 1
    manifest = json.loads((backup / "backup-manifest.json").read_text())
    assert set(manifest["objects"][0]["source_tables"]) == {
        "asset_files",
        "concept_assets",
        "agent_imported_glbs",
    }
    assert manifest["objects"][0]["source_kinds"] == ["legacy"]
    restored = tmp_path / "legacy-restored"
    restore_backup(backup, restored)
    assert (restored / relative).read_bytes() == payload
    assert not (backup / orphan_relative).exists()
