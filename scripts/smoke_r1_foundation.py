#!/usr/bin/env python3
from __future__ import annotations

import ast
import hashlib
import sqlite3
import tempfile
from pathlib import Path

from forgecad_agent.api import LocalApiSettings, create_local_api
from forgecad_agent.infrastructure.db import (
    SQLiteConnectionFactory,
    SQLiteMigrationRunner,
)
from forgecad_agent.infrastructure.storage import (
    ContentAddressedStore,
    ObjectStoreError,
)


ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad-r1-") as temporary_directory:
        root = Path(temporary_directory)
        library_root = root / "library"
        library_root.mkdir(parents=True)
        connection_factory = SQLiteConnectionFactory(library_root / "library.db")
        migration_runner = SQLiteMigrationRunner(
            connection_factory, ROOT / "migrations"
        )

        first_apply = migration_runner.run()
        second_apply = migration_runner.run()
        assert first_apply, "fresh database should apply migrations"
        assert second_apply == [], "migration runner must be idempotent"

        with connection_factory.connect() as connection:
            foreign_keys = connection.execute("PRAGMA foreign_keys").fetchone()[0]
            busy_timeout = connection.execute("PRAGMA busy_timeout").fetchone()[0]
            migration_count = connection.execute(
                "SELECT COUNT(*) FROM schema_migrations"
            ).fetchone()[0]
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
        assert (
            object_store.read(stored.relative_path, expected_sha256=expected_hash)
            == payload
        )

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

        create_workflow_boundary = _assert_create_weapon_workflow_boundary()
        generate_3d_workflow_boundary = _assert_generate_3d_workflow_boundary()

    print(
        {
            "ok": True,
            "migration_count": migration_count,
            "fresh_applied": first_apply,
            "object_sha256": expected_hash,
            "create_weapon_workflow": create_workflow_boundary,
            "generate_3d_workflow": generate_3d_workflow_boundary,
        }
    )


def _assert_create_weapon_workflow_boundary() -> dict[str, int | bool]:
    facade_path = ROOT / "apps" / "agent" / "wushen_agent" / "asset_store.py"
    service_path = (
        ROOT / "apps" / "agent" / "wushen_agent" / "application" / "create_weapon.py"
    )
    facade_source = facade_path.read_text(encoding="utf-8")
    service_source = service_path.read_text(encoding="utf-8")
    facade_method = _class_method_source(
        facade_source,
        "SQLiteAssetStore",
        "create_weapon",
    )
    service_method = _class_method_source(
        service_source,
        "LegacyCreateWeaponService",
        "create_weapon",
    )
    assert "create_weapon_workflow.create_weapon" in facade_method
    assert "plan_weapon_spec" not in facade_method
    assert "generate_concept" not in facade_method
    assert "_write_asset" not in facade_method
    for required in (
        "plan_weapon_spec",
        "generate_concept",
        "_write_rough_model_assets",
        "_record_provider_task",
        "_insert_event",
    ):
        assert required in service_method, (
            f"create workflow lost orchestration step: {required}"
        )
    facade_lines = len(facade_source.splitlines())
    assert facade_lines <= 3300, f"AssetStore facade expanded to {facade_lines} lines"
    return {
        "facade_delegates": True,
        "provider_orchestration_in_service": True,
        "asset_store_lines": facade_lines,
        "service_lines": len(service_source.splitlines()),
    }


def _assert_generate_3d_workflow_boundary() -> dict[str, int | bool]:
    facade_path = ROOT / "apps" / "agent" / "wushen_agent" / "asset_store.py"
    service_path = (
        ROOT / "apps" / "agent" / "wushen_agent" / "application" / "generate_3d.py"
    )
    facade_source = facade_path.read_text(encoding="utf-8")
    service_source = service_path.read_text(encoding="utf-8")
    facade_generate = _class_method_source(
        facade_source,
        "SQLiteAssetStore",
        "generate_3d",
    )
    facade_enqueue = _class_method_source(
        facade_source,
        "SQLiteAssetStore",
        "enqueue_generate_3d",
    )
    service_generate = _class_method_source(
        service_source,
        "LegacyGenerate3DService",
        "generate_3d",
    )
    service_enqueue = _class_method_source(
        service_source,
        "LegacyGenerate3DService",
        "enqueue_generate_3d",
    )
    assert "generate_3d_workflow.generate_3d" in facade_generate
    assert "generate_3d_workflow.enqueue_generate_3d" in facade_enqueue
    for forbidden in (
        "WUSHEN_GENERATE3D",
        "_write_rough_model_assets",
        "_insert_event",
    ):
        assert forbidden not in facade_generate
        assert forbidden not in facade_enqueue
    for required in (
        "WUSHEN_GENERATE3D_RUNTIME",
        "enqueue_generate_3d",
        "_write_rough_model_assets",
        "_record_provider_task",
        "_insert_event",
    ):
        assert required in service_generate, (
            f"generate-3d workflow lost orchestration step: {required}"
        )
    for required in ("_validate_source", "queued", "_insert_step", "_insert_event"):
        assert required in service_enqueue, (
            f"generate-3d queue lost orchestration step: {required}"
        )
    facade_lines = len(facade_source.splitlines())
    assert facade_lines <= 3075, f"AssetStore facade expanded to {facade_lines} lines"
    return {
        "facade_delegates_sync_and_queue": True,
        "runtime_selection_in_service": True,
        "asset_store_lines": facade_lines,
        "service_lines": len(service_source.splitlines()),
    }


def _class_method_source(source: str, class_name: str, method_name: str) -> str:
    tree = ast.parse(source)
    for node in tree.body:
        if isinstance(node, ast.ClassDef) and node.name == class_name:
            for child in node.body:
                if (
                    isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef))
                    and child.name == method_name
                ):
                    segment = ast.get_source_segment(source, child)
                    assert segment is not None
                    return segment
    raise AssertionError(f"missing {class_name}.{method_name}")


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, sqlite3.Error) as exc:
        raise SystemExit(f"R1 foundation smoke failed: {exc}") from exc
