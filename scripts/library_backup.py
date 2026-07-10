#!/usr/bin/env python3
"""Create, verify, and restore a ForgeCAD Library backup.

The backup is a directory containing one SQLite Backup API snapshot, every
object referenced by that snapshot, and a hash manifest. Provider secret files,
WAL/SHM files, trash, caches, and unreferenced object candidates are excluded.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sqlite3
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

MANIFEST_SCHEMA = "ForgeCADLibraryBackupManifest@1"
MANIFEST_NAME = "backup-manifest.json"
DATABASE_NAME = "library.db"
OBJECT_TABLES = (
    ("asset_files", "file_id"),
    ("concept_assets", "asset_id"),
)
COUNTED_TABLES = (
    "weapons",
    "weapon_versions",
    "generation_jobs",
    "asset_files",
    "projects",
    "project_versions",
    "module_assets",
    "module_connectors",
    "module_graphs",
    "design_change_sets",
    "change_set_audit_exports",
    "concept_assets",
    "concept_jobs",
)


class LibraryBackupError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def create_backup(library_root: Path, output: Path) -> dict[str, Any]:
    source_root = library_root.expanduser().resolve()
    source_database = source_root / DATABASE_NAME
    if not source_root.is_dir() or not source_database.is_file():
        raise LibraryBackupError(
            "SOURCE_LIBRARY_NOT_FOUND",
            f"ForgeCAD Library database was not found under {source_root}.",
        )
    output = output.expanduser().resolve()
    if _is_within(output, source_root):
        raise LibraryBackupError(
            "BACKUP_DESTINATION_INSIDE_LIBRARY",
            "Backup destination must be outside the source Library.",
        )
    _require_new_destination(output, code="BACKUP_DESTINATION_EXISTS")
    output.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f".{output.name}.tmp-", dir=output.parent))
    try:
        snapshot_database = staging / DATABASE_NAME
        _snapshot_database(source_database, snapshot_database)
        database_report = _inspect_database(snapshot_database)
        references = _object_references(snapshot_database)
        objects, logical_object_bytes = _collapse_references(references)

        object_entries: list[dict[str, Any]] = []
        for relative_path, expected in sorted(objects.items()):
            source = _safe_object_file(source_root, relative_path)
            _verify_file(
                source,
                expected_sha256=expected["sha256"],
                expected_byte_size=expected["byte_size"],
                label=relative_path,
            )
            target = staging / relative_path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
            _verify_file(
                target,
                expected_sha256=expected["sha256"],
                expected_byte_size=expected["byte_size"],
                label=relative_path,
            )
            object_entries.append(
                {
                    "path": relative_path,
                    "sha256": expected["sha256"],
                    "byte_size": expected["byte_size"],
                    "reference_count": expected["reference_count"],
                    "source_tables": sorted(expected["source_tables"]),
                }
            )

        source_store = _scan_object_store(source_root)
        referenced_paths = set(objects)
        unreferenced_paths = source_store["paths"] - referenced_paths
        unreferenced_bytes = sum(
            source_store["sizes"][path] for path in unreferenced_paths
        )
        database_sha256 = _sha256_file(snapshot_database)
        database_bytes = snapshot_database.stat().st_size
        unique_object_bytes = sum(item["byte_size"] for item in object_entries)
        created_at = _utc_now()
        backup_id = (
            f"backup_{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}_"
            f"{database_sha256[:12]}"
        )
        manifest = {
            "schema_version": MANIFEST_SCHEMA,
            "backup_id": backup_id,
            "created_at": created_at,
            "database": {
                "path": DATABASE_NAME,
                "sha256": database_sha256,
                "byte_size": database_bytes,
                "page_count": database_report["page_count"],
                "journal_mode": database_report["journal_mode"],
                "schema_versions": database_report["schema_versions"],
                "table_counts": database_report["table_counts"],
            },
            "objects": object_entries,
            "capacity": {
                "database_bytes": database_bytes,
                "reference_rows": len(references),
                "unique_object_count": len(object_entries),
                "unique_object_bytes": unique_object_bytes,
                "logical_object_bytes": logical_object_bytes,
                "deduplicated_bytes": logical_object_bytes - unique_object_bytes,
                "backup_payload_bytes": database_bytes + unique_object_bytes,
                "source_object_store_file_count": len(source_store["paths"]),
                "source_object_store_bytes": source_store["total_bytes"],
                "unreferenced_candidate_count": len(unreferenced_paths),
                "unreferenced_candidate_bytes": unreferenced_bytes,
            },
            "exclusions": [
                "library.db-wal",
                "library.db-shm",
                "config and provider secret files",
                "trash and caches",
                "unreferenced object candidates",
            ],
        }
        _write_json(staging / MANIFEST_NAME, manifest)
        verification = verify_backup(staging)
        staging.rename(output)
        return {
            "ok": True,
            "operation": "backup",
            "backup_id": backup_id,
            "backup_path": str(output),
            "verification": verification,
            "capacity": manifest["capacity"],
        }
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def verify_backup(backup_root: Path) -> dict[str, Any]:
    root = backup_root.expanduser().resolve()
    manifest = _read_manifest(root)
    database = _safe_child_file(root, manifest["database"]["path"])
    _verify_file(
        database,
        expected_sha256=manifest["database"]["sha256"],
        expected_byte_size=manifest["database"]["byte_size"],
        label=DATABASE_NAME,
    )
    database_report = _inspect_database(database)
    if (
        database_report["journal_mode"] != "delete"
        or manifest["database"].get("journal_mode") != "delete"
    ):
        raise LibraryBackupError(
            "BACKUP_JOURNAL_MODE_INVALID",
            "Backup database must be a standalone journal_mode=DELETE snapshot.",
        )
    if database_report["schema_versions"] != manifest["database"]["schema_versions"]:
        raise LibraryBackupError(
            "BACKUP_SCHEMA_MISMATCH",
            "Backup database schema versions do not match the manifest.",
        )
    if database_report["table_counts"] != manifest["database"]["table_counts"]:
        raise LibraryBackupError(
            "BACKUP_TABLE_COUNT_MISMATCH",
            "Backup database table counts do not match the manifest.",
        )

    references = _object_references(database)
    expected_objects, logical_object_bytes = _collapse_references(references)
    manifest_objects = _manifest_objects(manifest)
    if set(expected_objects) != set(manifest_objects):
        missing = sorted(set(expected_objects) - set(manifest_objects))
        extra = sorted(set(manifest_objects) - set(expected_objects))
        raise LibraryBackupError(
            "BACKUP_OBJECT_SET_MISMATCH",
            f"Backup object set mismatch; missing={missing[:5]}, extra={extra[:5]}.",
        )
    stored_objects = _scan_object_store(root)
    if stored_objects["paths"] != set(manifest_objects):
        missing = sorted(set(manifest_objects) - stored_objects["paths"])
        extra = sorted(stored_objects["paths"] - set(manifest_objects))
        raise LibraryBackupError(
            "BACKUP_OBJECT_FILE_SET_MISMATCH",
            f"Backup object files mismatch; missing={missing[:5]}, extra={extra[:5]}.",
        )
    for relative_path, entry in manifest_objects.items():
        expected = expected_objects[relative_path]
        if (
            entry["sha256"] != expected["sha256"]
            or entry["byte_size"] != expected["byte_size"]
            or entry["reference_count"] != expected["reference_count"]
            or sorted(entry["source_tables"]) != sorted(expected["source_tables"])
        ):
            raise LibraryBackupError(
                "BACKUP_OBJECT_METADATA_MISMATCH",
                f"Object metadata does not match the database: {relative_path}.",
            )
        _verify_file(
            _safe_object_file(root, relative_path),
            expected_sha256=entry["sha256"],
            expected_byte_size=entry["byte_size"],
            label=relative_path,
        )

    database_bytes = database.stat().st_size
    unique_object_bytes = sum(item["byte_size"] for item in manifest_objects.values())
    recomputed_capacity = {
        "database_bytes": database_bytes,
        "reference_rows": len(references),
        "unique_object_count": len(manifest_objects),
        "unique_object_bytes": unique_object_bytes,
        "logical_object_bytes": logical_object_bytes,
        "deduplicated_bytes": logical_object_bytes - unique_object_bytes,
        "backup_payload_bytes": database_bytes + unique_object_bytes,
    }
    for key, value in recomputed_capacity.items():
        if manifest["capacity"].get(key) != value:
            raise LibraryBackupError(
                "BACKUP_CAPACITY_MISMATCH",
                f"Backup capacity field {key} does not match payload contents.",
            )
    return {
        "ok": True,
        "operation": "verify",
        "backup_id": manifest["backup_id"],
        "schema_versions": database_report["schema_versions"],
        "object_count": len(manifest_objects),
        "foreign_key_violations": 0,
        "integrity_check": "ok",
        "journal_mode": database_report["journal_mode"],
        "capacity": manifest["capacity"],
    }


def restore_backup(backup_root: Path, destination: Path) -> dict[str, Any]:
    root = backup_root.expanduser().resolve()
    verification = verify_backup(root)
    manifest = _read_manifest(root)
    destination = destination.expanduser().resolve()
    if _is_within(destination, root):
        raise LibraryBackupError(
            "RESTORE_DESTINATION_INSIDE_BACKUP",
            "Restore destination must be outside the backup directory.",
        )
    _require_new_destination(destination, code="RESTORE_DESTINATION_EXISTS")
    destination.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(
        tempfile.mkdtemp(prefix=f".{destination.name}.tmp-", dir=destination.parent)
    )
    try:
        shutil.copy2(root / DATABASE_NAME, staging / DATABASE_NAME)
        for entry in manifest["objects"]:
            source = _safe_object_file(root, entry["path"])
            target = staging / entry["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        provenance = staging / "backups" / "manifests" / f"{manifest['backup_id']}.json"
        provenance.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / MANIFEST_NAME, provenance)
        restored_report = _verify_library_against_manifest(staging, manifest)
        staging.rename(destination)
        return {
            "ok": True,
            "operation": "restore",
            "backup_id": manifest["backup_id"],
            "destination": str(destination),
            "backup_verification": verification,
            "restored_verification": restored_report,
        }
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def _snapshot_database(source: Path, target: Path) -> None:
    source_uri = f"file:{source.as_posix()}?mode=ro"
    try:
        with sqlite3.connect(source_uri, uri=True, timeout=30) as source_connection:
            source_connection.execute("PRAGMA query_only=ON")
            integrity = [
                row[0] for row in source_connection.execute("PRAGMA integrity_check")
            ]
            if integrity != ["ok"]:
                raise LibraryBackupError(
                    "SOURCE_DATABASE_INTEGRITY_FAILED",
                    f"Source SQLite integrity_check failed: {integrity[:5]}.",
                )
            with sqlite3.connect(target) as target_connection:
                source_connection.backup(target_connection)
                target_connection.commit()
                journal_mode = target_connection.execute(
                    "PRAGMA journal_mode=DELETE"
                ).fetchone()[0]
                if str(journal_mode).lower() != "delete":
                    raise LibraryBackupError(
                        "DATABASE_SNAPSHOT_FAILED",
                        "Backup snapshot could not be normalized to a single SQLite file.",
                    )
        for suffix in ("-wal", "-shm"):
            transient = Path(f"{target}{suffix}")
            if transient.exists():
                transient.unlink()
    except sqlite3.Error as exc:
        raise LibraryBackupError(
            "DATABASE_SNAPSHOT_FAILED", f"SQLite Backup API failed: {exc}."
        ) from exc


def _inspect_database(database: Path) -> dict[str, Any]:
    try:
        with sqlite3.connect(
            f"file:{database.as_posix()}?mode=ro", uri=True
        ) as connection:
            connection.row_factory = sqlite3.Row
            connection.execute("PRAGMA foreign_keys=ON")
            integrity = [row[0] for row in connection.execute("PRAGMA integrity_check")]
            if integrity != ["ok"]:
                raise LibraryBackupError(
                    "DATABASE_INTEGRITY_FAILED",
                    f"SQLite integrity_check failed: {integrity[:5]}.",
                )
            violations = connection.execute("PRAGMA foreign_key_check").fetchall()
            if violations:
                raise LibraryBackupError(
                    "DATABASE_FOREIGN_KEY_FAILED",
                    f"SQLite foreign_key_check found {len(violations)} violation(s).",
                )
            tables = _table_names(connection)
            schema_versions = (
                [
                    str(row["version"])
                    for row in connection.execute(
                        "SELECT version FROM schema_migrations ORDER BY version"
                    ).fetchall()
                ]
                if "schema_migrations" in tables
                else []
            )
            table_counts = {
                table: int(
                    connection.execute(f'SELECT COUNT(*) FROM "{table}"').fetchone()[0]
                )
                for table in COUNTED_TABLES
                if table in tables
            }
            return {
                "integrity_check": "ok",
                "foreign_key_violations": 0,
                "page_count": int(
                    connection.execute("PRAGMA page_count").fetchone()[0]
                ),
                "journal_mode": str(
                    connection.execute("PRAGMA journal_mode").fetchone()[0]
                ).lower(),
                "schema_versions": schema_versions,
                "table_counts": table_counts,
            }
    except sqlite3.Error as exc:
        raise LibraryBackupError(
            "DATABASE_INSPECTION_FAILED", f"Could not inspect backup database: {exc}."
        ) from exc


def _object_references(database: Path) -> list[dict[str, Any]]:
    references: list[dict[str, Any]] = []
    try:
        with sqlite3.connect(
            f"file:{database.as_posix()}?mode=ro", uri=True
        ) as connection:
            connection.row_factory = sqlite3.Row
            tables = _table_names(connection)
            supported_tables = {table for table, _ in OBJECT_TABLES}
            for table in tables:
                columns = {
                    str(row["name"])
                    for row in connection.execute(
                        "SELECT name FROM pragma_table_info(?)", (table,)
                    ).fetchall()
                }
                if "object_path" in columns and table not in supported_tables:
                    raise LibraryBackupError(
                        "UNSUPPORTED_OBJECT_REFERENCE_TABLE",
                        f"Backup support is missing for object table {table}.",
                    )
            for table, id_column in OBJECT_TABLES:
                if table not in tables:
                    continue
                rows = connection.execute(
                    f"""
                    SELECT "{id_column}" AS reference_id, object_path, sha256, byte_size
                    FROM "{table}"
                    ORDER BY object_path, "{id_column}"
                    """
                ).fetchall()
                for row in rows:
                    relative_path = _safe_object_relative_path(str(row["object_path"]))
                    sha256 = str(row["sha256"])
                    byte_size = int(row["byte_size"])
                    _validate_digest(sha256, relative_path)
                    _validate_content_addressed_path(relative_path, sha256)
                    references.append(
                        {
                            "table": table,
                            "reference_id": str(row["reference_id"]),
                            "path": relative_path,
                            "sha256": sha256,
                            "byte_size": byte_size,
                        }
                    )
    except sqlite3.Error as exc:
        raise LibraryBackupError(
            "OBJECT_REFERENCE_QUERY_FAILED",
            f"Could not read object references from SQLite: {exc}.",
        ) from exc
    return references


def _collapse_references(
    references: Iterable[dict[str, Any]],
) -> tuple[dict[str, dict[str, Any]], int]:
    objects: dict[str, dict[str, Any]] = {}
    logical_bytes = 0
    for reference in references:
        logical_bytes += reference["byte_size"]
        current = objects.get(reference["path"])
        if current is None:
            objects[reference["path"]] = {
                "sha256": reference["sha256"],
                "byte_size": reference["byte_size"],
                "reference_count": 1,
                "source_tables": {reference["table"]},
            }
            continue
        if (
            current["sha256"] != reference["sha256"]
            or current["byte_size"] != reference["byte_size"]
        ):
            raise LibraryBackupError(
                "OBJECT_REFERENCE_CONFLICT",
                f"Conflicting metadata references object {reference['path']}.",
            )
        current["reference_count"] += 1
        current["source_tables"].add(reference["table"])
    return objects, logical_bytes


def _manifest_objects(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    objects = manifest.get("objects")
    if not isinstance(objects, list):
        raise LibraryBackupError(
            "BACKUP_MANIFEST_INVALID", "Backup manifest objects must be an array."
        )
    for item in objects:
        if not isinstance(item, dict):
            raise LibraryBackupError(
                "BACKUP_MANIFEST_INVALID", "Backup manifest object entry is invalid."
            )
        relative_path = _safe_object_relative_path(str(item.get("path", "")))
        if relative_path in result:
            raise LibraryBackupError(
                "BACKUP_MANIFEST_INVALID",
                f"Backup manifest repeats object path {relative_path}.",
            )
        _validate_digest(str(item.get("sha256", "")), relative_path)
        _validate_content_addressed_path(relative_path, str(item["sha256"]))
        if not isinstance(item.get("byte_size"), int) or item["byte_size"] < 0:
            raise LibraryBackupError(
                "BACKUP_MANIFEST_INVALID",
                f"Backup manifest byte_size is invalid for {relative_path}.",
            )
        if (
            not isinstance(item.get("reference_count"), int)
            or item["reference_count"] < 1
        ):
            raise LibraryBackupError(
                "BACKUP_MANIFEST_INVALID",
                f"Backup manifest reference_count is invalid for {relative_path}.",
            )
        if not isinstance(item.get("source_tables"), list):
            raise LibraryBackupError(
                "BACKUP_MANIFEST_INVALID",
                f"Backup manifest source_tables is invalid for {relative_path}.",
            )
        result[relative_path] = item
    return result


def _verify_library_against_manifest(
    library_root: Path, manifest: dict[str, Any]
) -> dict[str, Any]:
    database = library_root / DATABASE_NAME
    _verify_file(
        database,
        expected_sha256=manifest["database"]["sha256"],
        expected_byte_size=manifest["database"]["byte_size"],
        label=DATABASE_NAME,
    )
    report = _inspect_database(database)
    expected, _ = _collapse_references(_object_references(database))
    manifest_objects = _manifest_objects(manifest)
    if set(expected) != set(manifest_objects):
        raise LibraryBackupError(
            "RESTORED_OBJECT_SET_MISMATCH",
            "Restored database object references do not match the backup manifest.",
        )
    stored_objects = _scan_object_store(library_root)
    if stored_objects["paths"] != set(manifest_objects):
        raise LibraryBackupError(
            "RESTORED_OBJECT_FILE_SET_MISMATCH",
            "Restored object files do not exactly match the backup manifest.",
        )
    for relative_path, entry in manifest_objects.items():
        _verify_file(
            _safe_object_file(library_root, relative_path),
            expected_sha256=entry["sha256"],
            expected_byte_size=entry["byte_size"],
            label=relative_path,
        )
    return {
        "integrity_check": report["integrity_check"],
        "foreign_key_violations": report["foreign_key_violations"],
        "journal_mode": report["journal_mode"],
        "schema_versions": report["schema_versions"],
        "table_counts": report["table_counts"],
        "object_count": len(manifest_objects),
    }


def _read_manifest(root: Path) -> dict[str, Any]:
    manifest_path = _safe_child_file(root, MANIFEST_NAME)
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise LibraryBackupError(
            "BACKUP_MANIFEST_INVALID", f"Could not read backup manifest: {exc}."
        ) from exc
    if (
        not isinstance(manifest, dict)
        or manifest.get("schema_version") != MANIFEST_SCHEMA
    ):
        raise LibraryBackupError(
            "BACKUP_MANIFEST_INVALID",
            f"Backup manifest must use {MANIFEST_SCHEMA}.",
        )
    for key in ("backup_id", "created_at", "database", "objects", "capacity"):
        if key not in manifest:
            raise LibraryBackupError(
                "BACKUP_MANIFEST_INVALID", f"Backup manifest is missing {key}."
            )
    database = manifest["database"]
    if not isinstance(database, dict) or database.get("path") != DATABASE_NAME:
        raise LibraryBackupError(
            "BACKUP_MANIFEST_INVALID", "Backup manifest database path is invalid."
        )
    _validate_digest(str(database.get("sha256", "")), DATABASE_NAME)
    if not isinstance(database.get("byte_size"), int) or database["byte_size"] < 1:
        raise LibraryBackupError(
            "BACKUP_MANIFEST_INVALID", "Backup manifest database byte_size is invalid."
        )
    return manifest


def _scan_object_store(library_root: Path) -> dict[str, Any]:
    object_root = library_root / "objects" / "sha256"
    paths: set[str] = set()
    sizes: dict[str, int] = {}
    if not object_root.exists():
        return {"paths": paths, "sizes": sizes, "total_bytes": 0}
    if object_root.is_symlink() or not object_root.is_dir():
        raise LibraryBackupError(
            "OBJECT_STORE_INVALID", "objects/sha256 must be a real directory."
        )
    for path in object_root.rglob("*"):
        if path.is_symlink():
            raise LibraryBackupError(
                "OBJECT_STORE_SYMLINK", f"Object store contains a symlink: {path}."
            )
        if not path.is_file():
            continue
        relative_path = path.relative_to(library_root).as_posix()
        paths.add(relative_path)
        sizes[relative_path] = path.stat().st_size
    return {
        "paths": paths,
        "sizes": sizes,
        "total_bytes": sum(sizes.values()),
    }


def _safe_object_relative_path(value: str) -> str:
    path = Path(value)
    if (
        not value
        or path.is_absolute()
        or ".." in path.parts
        or len(path.parts) < 4
        or path.parts[:2] != ("objects", "sha256")
    ):
        raise LibraryBackupError(
            "OBJECT_PATH_INVALID",
            f"Object path is not safely content-addressed: {value}.",
        )
    return path.as_posix()


def _safe_object_file(root: Path, relative_path: str) -> Path:
    safe_path = _safe_object_relative_path(relative_path)
    return _safe_child_file(root, safe_path)


def _safe_child_file(root: Path, relative_path: str) -> Path:
    path = Path(relative_path)
    if path.is_absolute() or ".." in path.parts:
        raise LibraryBackupError(
            "BACKUP_PATH_INVALID", f"Backup path is unsafe: {relative_path}."
        )
    target = root / path
    try:
        target.resolve().relative_to(root.resolve())
    except ValueError as exc:
        raise LibraryBackupError(
            "BACKUP_PATH_INVALID", f"Backup path escapes its root: {relative_path}."
        ) from exc
    if target.is_symlink():
        raise LibraryBackupError(
            "BACKUP_SYMLINK_DENIED",
            f"Backup file cannot be a symlink: {relative_path}.",
        )
    if not target.is_file():
        raise LibraryBackupError(
            "BACKUP_FILE_MISSING", f"Backup file is missing: {relative_path}."
        )
    return target


def _verify_file(
    path: Path,
    *,
    expected_sha256: str,
    expected_byte_size: int,
    label: str,
) -> None:
    if path.is_symlink() or not path.is_file():
        raise LibraryBackupError("BACKUP_FILE_MISSING", f"File is missing: {label}.")
    actual_size = path.stat().st_size
    if actual_size != expected_byte_size:
        raise LibraryBackupError(
            "BACKUP_SIZE_MISMATCH",
            f"File size mismatch for {label}: expected {expected_byte_size}, got {actual_size}.",
        )
    actual_sha256 = _sha256_file(path)
    if actual_sha256 != expected_sha256:
        raise LibraryBackupError(
            "BACKUP_HASH_MISMATCH", f"File SHA-256 mismatch for {label}."
        )


def _validate_digest(value: str, label: str) -> None:
    if len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise LibraryBackupError(
            "BACKUP_DIGEST_INVALID", f"SHA-256 is invalid for {label}."
        )


def _validate_content_addressed_path(relative_path: str, sha256: str) -> None:
    parts = Path(relative_path).parts
    filename = parts[-1]
    if (
        len(parts) != 5
        or parts[2] != sha256[:2]
        or parts[3] != sha256[2:4]
        or not (
            filename == sha256
            or (filename.startswith(sha256) and filename[len(sha256)] == ".")
        )
    ):
        raise LibraryBackupError(
            "OBJECT_PATH_HASH_MISMATCH",
            f"Object path does not match its SHA-256: {relative_path}.",
        )


def _table_names(connection: sqlite3.Connection) -> set[str]:
    return {
        str(row[0])
        for row in connection.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table'"
        ).fetchall()
    }


def _require_new_destination(destination: Path, *, code: str) -> None:
    if destination.exists() or destination.is_symlink():
        raise LibraryBackupError(
            code,
            f"Destination already exists; refusing to overwrite: {destination}.",
        )


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Create, verify, or restore a ForgeCAD Library backup."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    backup = subparsers.add_parser("backup", help="Create a new backup directory.")
    backup.add_argument("--library-root", type=Path, required=True)
    backup.add_argument("--output", type=Path, required=True)
    verify = subparsers.add_parser("verify", help="Verify a backup directory.")
    verify.add_argument("--backup", type=Path, required=True)
    restore = subparsers.add_parser(
        "restore", help="Restore into a new Library directory."
    )
    restore.add_argument("--backup", type=Path, required=True)
    restore.add_argument("--destination", type=Path, required=True)
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        if args.command == "backup":
            result = create_backup(args.library_root, args.output)
        elif args.command == "verify":
            result = verify_backup(args.backup)
        else:
            result = restore_backup(args.backup, args.destination)
    except LibraryBackupError as exc:
        print(
            json.dumps(
                {"ok": False, "error": {"code": exc.code, "message": str(exc)}},
                ensure_ascii=False,
                indent=2,
            ),
            file=sys.stderr,
        )
        return 2
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
