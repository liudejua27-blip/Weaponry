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
        worker_runtime_boundary = _assert_worker_runtime_boundary()
        unity_export_boundary = _assert_unity_export_boundary()
        patch_workflow_boundary = _assert_patch_workflow_boundary()
        asset_store_facade_boundary = _assert_asset_store_facade_boundary()

    print(
        {
            "ok": True,
            "migration_count": migration_count,
            "fresh_applied": first_apply,
            "object_sha256": expected_hash,
            "create_weapon_workflow": create_workflow_boundary,
            "generate_3d_workflow": generate_3d_workflow_boundary,
            "worker_runtime": worker_runtime_boundary,
            "unity_export_workflow": unity_export_boundary,
            "patch_workflow": patch_workflow_boundary,
            "asset_store_facade": asset_store_facade_boundary,
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


def _assert_worker_runtime_boundary() -> dict[str, int | bool]:
    facade_path = ROOT / "apps" / "agent" / "wushen_agent" / "asset_store.py"
    service_path = (
        ROOT / "apps" / "agent" / "wushen_agent" / "application" / "worker_runtime.py"
    )
    facade_source = facade_path.read_text(encoding="utf-8")
    service_source = service_path.read_text(encoding="utf-8")
    facade_worker = _class_method_source(
        facade_source,
        "SQLiteAssetStore",
        "run_worker_once",
    )
    service_worker = _class_method_source(
        service_source,
        "LegacyWorkerService",
        "run_worker_once",
    )
    assert "worker_runtime.run_worker_once" in facade_worker
    for forbidden in ("generation_jobs", "waiting_provider", "_complete_generate_3d"):
        assert forbidden not in facade_worker
    for required in (
        "generation_jobs",
        "waiting_provider",
        "_complete_generate_3d_worker_job",
        "_resume_generate_3d_provider_job",
        "_complete_export_unity_worker_job",
        "lease_expires_at",
    ):
        assert required in service_worker, (
            f"worker runtime lost dispatch responsibility: {required}"
        )
    for method_name in (
        "_complete_generate_3d_worker_job",
        "_resume_generate_3d_provider_job",
        "_handle_generate_3d_provider_poll",
    ):
        _class_method_source(service_source, "LegacyWorkerService", method_name)
    facade_lines = len(facade_source.splitlines())
    assert facade_lines <= 2450, f"AssetStore facade expanded to {facade_lines} lines"
    return {
        "facade_delegates": True,
        "claim_lease_dispatch_in_service": True,
        "generate_provider_commit_in_service": True,
        "asset_store_lines": facade_lines,
        "service_lines": len(service_source.splitlines()),
    }


def _assert_unity_export_boundary() -> dict[str, int | bool]:
    facade_path = ROOT / "apps" / "agent" / "wushen_agent" / "asset_store.py"
    service_path = (
        ROOT / "apps" / "agent" / "wushen_agent" / "application" / "unity_export.py"
    )
    facade_source = facade_path.read_text(encoding="utf-8")
    service_source = service_path.read_text(encoding="utf-8")
    facade_enqueue = _class_method_source(
        facade_source,
        "SQLiteAssetStore",
        "enqueue_export_unity",
    )
    facade_export = _class_method_source(
        facade_source,
        "SQLiteAssetStore",
        "export_unity",
    )
    facade_init = _class_method_source(
        facade_source,
        "SQLiteAssetStore",
        "__init__",
    )
    assert "unity_export_workflow.enqueue_export_unity" in facade_enqueue
    assert "unity_export_workflow.export_unity" in facade_export
    assert "unity_export_workflow.complete_worker_job" in facade_init
    assert "def _complete_export_unity_worker_job" not in facade_source
    for forbidden in ("export_packages", "_build_unity_export_zip", "zipfile"):
        assert forbidden not in facade_enqueue
        assert forbidden not in facade_export
    for method_name in (
        "enqueue_export_unity",
        "export_unity",
        "complete_worker_job",
        "_validate_unity_export_inputs",
        "_unity_export_manifest",
        "_build_unity_export_zip",
    ):
        _class_method_source(service_source, "LegacyUnityExportService", method_name)
    facade_lines = len(facade_source.splitlines())
    assert facade_lines <= 1850, f"AssetStore facade expanded to {facade_lines} lines"
    return {
        "facade_delegates_sync_and_queue": True,
        "worker_handler_in_service": True,
        "manifest_zip_in_service": True,
        "asset_store_lines": facade_lines,
        "service_lines": len(service_source.splitlines()),
    }


def _assert_patch_workflow_boundary() -> dict[str, int | bool]:
    facade_path = ROOT / "apps" / "agent" / "wushen_agent" / "asset_store.py"
    service_path = (
        ROOT / "apps" / "agent" / "wushen_agent" / "application" / "patch_workflow.py"
    )
    facade_source = facade_path.read_text(encoding="utf-8")
    service_source = service_path.read_text(encoding="utf-8")
    facade_patch = _class_method_source(
        facade_source,
        "SQLiteAssetStore",
        "patch_weapon",
    )
    service_patch = _class_method_source(
        service_source,
        "LegacyPatchService",
        "patch_weapon",
    )
    assert "patch_workflow.patch_weapon" in facade_patch
    for forbidden in ("generate_patch", "validate_patch_manifest", "mask_png_has_ink"):
        assert forbidden not in facade_patch
    for required in (
        "generate_patch",
        "validate_patch_manifest",
        "mask_png_has_ink",
        "_record_provider_task",
        "_upsert_checkpoint",
        "_patch_quality_report",
    ):
        assert required in service_patch or required in service_source, (
            f"Patch service lost workflow responsibility: {required}"
        )
    assert "def _mock_patch_svg" not in facade_source
    assert "def _mock_patch_svg" not in service_source
    facade_lines = len(facade_source.splitlines())
    assert facade_lines <= 1475, f"AssetStore facade expanded to {facade_lines} lines"
    return {
        "facade_delegates": True,
        "mask_manifest_provider_in_service": True,
        "dead_mock_svg_removed": True,
        "asset_store_lines": facade_lines,
        "service_lines": len(service_source.splitlines()),
    }


def _assert_asset_store_facade_boundary() -> dict[str, int | bool]:
    facade_path = ROOT / "apps" / "agent" / "wushen_agent" / "asset_store.py"
    facade_source = facade_path.read_text(encoding="utf-8")
    workflow_methods = (
        "create_weapon",
        "create_interpretation",
        "confirm_recast",
        "generate_3d",
        "enqueue_generate_3d",
        "run_worker_once",
        "export_unity",
        "enqueue_export_unity",
        "patch_weapon",
        "upload_asset",
    )
    method_lines: dict[str, int] = {}
    for method_name in workflow_methods:
        source = _class_method_source(
            facade_source,
            "SQLiteAssetStore",
            method_name,
        )
        method_lines[method_name] = len(source.splitlines())
        assert method_lines[method_name] <= 30, (
            f"AssetStore workflow facade grew: {method_name}={method_lines[method_name]}"
        )
    for forbidden in (
        "plan_weapon_spec(",
        "generate_concept(",
        "generate_patch(",
        "submit_rough_model(",
        "poll_rough_model(",
        "fetch_rough_model(",
        "zipfile.ZipFile(",
    ):
        assert forbidden not in facade_source, (
            f"Complete workflow responsibility returned to AssetStore: {forbidden}"
        )
    return {
        "complete_workflows_extracted": True,
        "workflow_facade_count": len(workflow_methods),
        "maximum_facade_method_lines": max(method_lines.values()),
        "asset_store_lines": len(facade_source.splitlines()),
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
