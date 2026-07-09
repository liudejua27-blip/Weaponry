#!/usr/bin/env python3
"""M5 Unity export preflight and optional Unity batchmode import smoke."""

from __future__ import annotations

import hashlib
import argparse
import json
import os
import platform
import shutil
import sqlite3
import struct
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Any, Iterable

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest, ExportUnityRequest  # noqa: E402


GLTFAST_PACKAGE_VERSION = os.environ.get("WUSHEN_UNITY_GLTF_PACKAGE_VERSION", "6.1.0")
UNITY_TIMEOUT_SECONDS = int(os.environ.get("WUSHEN_UNITY_TIMEOUT_SECONDS", "240"))


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    with tempfile.TemporaryDirectory(prefix="wushen_m5_unity_import_") as tmp:
        tmp_root = Path(tmp)
        library_root = tmp_root / "WushenForgeLibrary"
        store = SQLiteAssetStore(library_root=library_root, migrations_dir=ROOT / "migrations")
        source_job = store.create_weapon(
            CreateWeaponRequest(
                client_request_id="m5-unity-import-source",
                text="青玉雷纹长枪，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产",
            ),
            idempotency_key="m5-unity-import-source-key",
        )
        detail = store.get_weapon_detail(source_job.weapon_id)
        _assert(detail.current_model_id is not None, "created weapon did not expose current_model_id")
        export_job = store.export_unity(
            source_job.weapon_id,
            ExportUnityRequest(
                client_request_id="m5-unity-import-export",
                model_id=detail.current_model_id,
                export_type="unity_glb",
                include_source_spec=True,
                include_quality_reports=True,
            ),
            idempotency_key="m5-unity-import-export-key",
        )
        package_asset_id = _asset_id_by_role(export_job, "unity_export_package")
        package_info = store.resolve_asset_file(package_asset_id)
        preflight = _preflight_package(Path(package_info["path"]))
        findings, stats = validate(library_root, store.db_path)
        _assert(stats["blockers"] == 0, f"asset library blockers: {findings}")

        unity_path = _find_unity_executable()
        if unity_path is None:
            print(
                json.dumps(
                    {
                        "ok": not args.require_unity,
                        "weapon_id": source_job.weapon_id,
                        "model_id": detail.current_model_id,
                        "export_job_id": export_job.job_id,
                        "package_asset_id": package_asset_id,
                        "package_preflight": preflight,
                        "unity_import_status": "blocked_unity_not_configured",
                        "release_gate": "blocked",
                        "require_unity": args.require_unity,
                        "blocking_failure": {
                            "code": "UNITY_EXECUTABLE_NOT_CONFIGURED",
                            "message": "Set WUSHEN_UNITY_EXECUTABLE or UNITY_EXECUTABLE to run the batchmode Unity import smoke.",
                        },
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 1 if args.require_unity else 0

        unity_result = _run_unity_import(
            unity_path=unity_path,
            package_path=Path(package_info["path"]),
            work_root=tmp_root,
        )
        print(
            json.dumps(
                {
                    "ok": unity_result["ok"],
                    "weapon_id": source_job.weapon_id,
                    "model_id": detail.current_model_id,
                    "export_job_id": export_job.job_id,
                    "package_asset_id": package_asset_id,
                    "package_preflight": preflight,
                    "unity_import_status": "imported" if unity_result["ok"] else "failed",
                    "release_gate": "passed" if unity_result["ok"] else "blocked",
                    "require_unity": args.require_unity,
                    "unity": unity_result,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0 if unity_result["ok"] else 1


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate Wushen Forge Unity export package preflight and optional Unity batchmode import.")
    parser.add_argument(
        "--require-unity",
        action="store_true",
        help="Fail with a non-zero exit code when no Unity executable is configured or auto-detected.",
    )
    return parser.parse_args(argv)


def _preflight_package(package_path: Path) -> dict[str, Any]:
    with zipfile.ZipFile(package_path) as archive:
        names = archive.namelist()
        _assert(names, "Unity package ZIP is empty")
        _assert(all(_safe_zip_name(name) for name in names), "Unity package ZIP contains unsafe paths")
        manifest_name = _single_suffix(names, "/manifest.json")
        manifest = json.loads(archive.read(manifest_name).decode("utf-8"))
        package_root = manifest.get("package_root")
        _assert(isinstance(package_root, str) and package_root.startswith("Assets/WushenForge/Weapons/"), "manifest package_root is invalid")
        _assert(manifest.get("schema_version") == "UnityExportManifest@1", "manifest schema_version mismatch")
        _assert(manifest.get("engine") == "unity", "manifest engine mismatch")
        safety = manifest.get("safety_boundary") or {}
        _assert(safety.get("asset_type") == "fictional_game_art", "manifest safety_boundary asset_type mismatch")
        _assert(safety.get("non_manufacturing_asset") is True, "manifest missing non-manufacturing boundary")
        required = [
            f"{package_root}/Models/rough_optimized.glb",
            f"{package_root}/Materials/unity_material.json",
            f"{package_root}/Specs/weapon_spec.json",
            f"{package_root}/Reports/model_quality_report.json",
            f"{package_root}/README_WUSHEN.txt",
        ]
        for expected in required:
            _assert(expected in names, f"Unity package missing {expected}")
        _assert(_parse_glb_bytes(archive.read(f"{package_root}/Models/rough_optimized.glb"))["asset"]["version"] == "2.0", "optimized GLB is not GLB 2.0")
        _assert(json.loads(archive.read(f"{package_root}/Materials/unity_material.json").decode("utf-8"))["schema_version"] == "UnityMaterial@1", "Unity material schema mismatch")
        _assert(json.loads(archive.read(f"{package_root}/Specs/weapon_spec.json").decode("utf-8"))["schema_version"] == "WeaponDesignSpec@1", "weapon spec schema mismatch")
        _assert(json.loads(archive.read(f"{package_root}/Reports/model_quality_report.json").decode("utf-8"))["target_type"] == "model_3d", "model quality report target mismatch")
        _validate_manifest_file_entries(archive, manifest)
        return {
            "ok": True,
            "zip_entries": len(names),
            "package_root": package_root,
            "manifest": manifest_name,
            "optimized_glb_bytes": len(archive.read(f"{package_root}/Models/rough_optimized.glb")),
        }


def _run_unity_import(*, unity_path: Path, package_path: Path, work_root: Path) -> dict[str, Any]:
    project_root = work_root / "UnityImportProject"
    log_path = work_root / "unity-import.log"
    result_path = work_root / "unity-import-result.json"
    _write_unity_project(project_root, package_path)
    command = [
        str(unity_path),
        "-quit",
        "-batchmode",
        "-nographics",
        "-accept-apiupdate",
        "-projectPath",
        str(project_root),
        "-executeMethod",
        "WushenForge.Editor.WushenImportSmoke.Run",
        "-logFile",
        str(log_path),
        "-wushenResult",
        str(result_path),
    ]
    try:
        completed = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=UNITY_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired as exc:
        return {
            "ok": False,
            "code": "UNITY_IMPORT_TIMEOUT",
            "unity_executable": str(unity_path),
            "timeout_seconds": UNITY_TIMEOUT_SECONDS,
            "stdout_tail": (exc.stdout or "")[-4000:] if isinstance(exc.stdout, str) else "",
            "stderr_tail": (exc.stderr or "")[-4000:] if isinstance(exc.stderr, str) else "",
            "log_path": str(log_path),
        }
    result = _read_json(result_path) if result_path.exists() else {}
    ok = completed.returncode == 0 and result.get("ok") is True
    return {
        "ok": ok,
        "code": result.get("code", "OK" if ok else "UNITY_IMPORT_FAILED"),
        "unity_executable": str(unity_path),
        "returncode": completed.returncode,
        "project_path": str(project_root),
        "log_path": str(log_path),
        "result": result,
        "stdout_tail": completed.stdout[-4000:],
        "stderr_tail": completed.stderr[-4000:],
    }


def _write_unity_project(project_root: Path, package_path: Path) -> None:
    (project_root / "Assets").mkdir(parents=True, exist_ok=True)
    (project_root / "Packages").mkdir(parents=True, exist_ok=True)
    (project_root / "ProjectSettings").mkdir(parents=True, exist_ok=True)
    (project_root / "Packages" / "manifest.json").write_text(
        json.dumps(
            {
                "dependencies": {
                    "com.unity.cloud.gltfast": GLTFAST_PACKAGE_VERSION,
                }
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    (project_root / "ProjectSettings" / "ProjectVersion.txt").write_text("m_EditorVersion: 6000.1.0f1\n", encoding="utf-8")
    with zipfile.ZipFile(package_path) as archive:
        for member in archive.infolist():
            _assert(_safe_zip_name(member.filename), f"unsafe package path: {member.filename}")
            target = (project_root / member.filename).resolve()
            target.relative_to(project_root.resolve())
            if member.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.read(member))
    editor_dir = project_root / "Assets" / "WushenForge" / "Editor"
    editor_dir.mkdir(parents=True, exist_ok=True)
    (editor_dir / "WushenImportSmoke.cs").write_text(_unity_import_smoke_cs(), encoding="utf-8")


def _unity_import_smoke_cs() -> str:
    return r'''#if UNITY_EDITOR
using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

namespace WushenForge.Editor
{
    public static class WushenImportSmoke
    {
        public static void Run()
        {
            string resultPath = GetArg("-wushenResult");
            try
            {
                AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport | ImportAssetOptions.ForceUpdate);
                string root = Directory.GetDirectories("Assets/WushenForge/Weapons").FirstOrDefault();
                Require(!string.IsNullOrEmpty(root), "NO_WEAPON_ROOT", "Assets/WushenForge/Weapons is empty.");
                root = Normalize(root);
                string manifest = Normalize(Path.Combine(root, "manifest.json"));
                string glb = Normalize(Path.Combine(root, "Models/rough_optimized.glb"));
                string unityMaterial = Normalize(Path.Combine(root, "Materials/unity_material.json"));
                string weaponSpec = Normalize(Path.Combine(root, "Specs/weapon_spec.json"));
                string qualityReport = Normalize(Path.Combine(root, "Reports/model_quality_report.json"));

                Require(File.Exists(manifest), "MISSING_MANIFEST", manifest);
                Require(File.Exists(glb), "MISSING_GLB", glb);
                Require(File.Exists(unityMaterial), "MISSING_UNITY_MATERIAL", unityMaterial);
                Require(File.Exists(weaponSpec), "MISSING_WEAPON_SPEC", weaponSpec);
                Require(File.Exists(qualityReport), "MISSING_QUALITY_REPORT", qualityReport);

                AssetImporter importer = AssetImporter.GetAtPath(glb);
                Require(importer != null, "NO_GLB_IMPORTER", "No Unity importer registered for rough_optimized.glb. Install com.unity.cloud.gltfast.");
                UnityEngine.Object[] glbAssets = AssetDatabase.LoadAllAssetsAtPath(glb);
                Require(glbAssets != null && glbAssets.Any(asset => asset != null), "GLB_IMPORT_EMPTY", "GLB imported but exposed no Unity assets.");
                Require(AssetDatabase.LoadAssetAtPath<TextAsset>(manifest) != null, "MANIFEST_NOT_TEXT_ASSET", manifest);
                Require(AssetDatabase.LoadAssetAtPath<TextAsset>(unityMaterial) != null, "UNITY_MATERIAL_NOT_TEXT_ASSET", unityMaterial);
                Require(AssetDatabase.LoadAssetAtPath<TextAsset>(weaponSpec) != null, "WEAPON_SPEC_NOT_TEXT_ASSET", weaponSpec);
                Require(AssetDatabase.LoadAssetAtPath<TextAsset>(qualityReport) != null, "QUALITY_REPORT_NOT_TEXT_ASSET", qualityReport);

                WriteResult(resultPath, true, "OK", "Unity imported Wushen Forge export package.", glb, importer.GetType().FullName, glbAssets.Length);
                EditorApplication.Exit(0);
            }
            catch (Exception error)
            {
                Debug.LogError(error.ToString());
                WriteResult(resultPath, false, error.GetType().Name, error.Message, "", "", 0);
                EditorApplication.Exit(1);
            }
        }

        static string GetArg(string name)
        {
            string[] args = Environment.GetCommandLineArgs();
            for (int i = 0; i < args.Length - 1; i++)
            {
                if (args[i] == name)
                {
                    return args[i + 1];
                }
            }
            return "";
        }

        static string Normalize(string path)
        {
            return path.Replace("\\", "/");
        }

        static void Require(bool condition, string code, string message)
        {
            if (!condition)
            {
                throw new InvalidOperationException(code + ": " + message);
            }
        }

        static void WriteResult(string path, bool ok, string code, string message, string glbPath, string importerType, int loadedAssetCount)
        {
            if (string.IsNullOrEmpty(path))
            {
                return;
            }
            string json = "{"
                + "\"ok\":" + (ok ? "true" : "false") + ","
                + "\"code\":\"" + Escape(code) + "\","
                + "\"message\":\"" + Escape(message) + "\","
                + "\"glb_path\":\"" + Escape(glbPath) + "\","
                + "\"importer_type\":\"" + Escape(importerType) + "\","
                + "\"loaded_asset_count\":" + loadedAssetCount
                + "}";
            File.WriteAllText(path, json);
        }

        static string Escape(string value)
        {
            return (value ?? "").Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\n", "\\n").Replace("\r", "\\r");
        }
    }
}
#endif
'''


def _find_unity_executable() -> Path | None:
    for key in ("WUSHEN_UNITY_EXECUTABLE", "UNITY_EXECUTABLE"):
        value = os.environ.get(key)
        if value:
            candidate = Path(value).expanduser()
            if candidate.exists() and candidate.is_file():
                return candidate
    candidates: list[Path] = []
    if platform.system() == "Darwin":
        candidates.extend(Path("/Applications/Unity/Hub/Editor").glob("*/Unity.app/Contents/MacOS/Unity"))
        candidates.append(Path("/Applications/Unity/Unity.app/Contents/MacOS/Unity"))
    elif platform.system() == "Linux":
        home = Path.home()
        candidates.extend((home / "Unity" / "Hub" / "Editor").glob("*/Editor/Unity"))
        candidates.append(Path("/opt/unity/Editor/Unity"))
    elif platform.system() == "Windows":
        program_files = [os.environ.get("ProgramFiles"), os.environ.get("ProgramFiles(x86)")]
        for root in [Path(item) for item in program_files if item]:
            candidates.extend((root / "Unity" / "Hub" / "Editor").glob("*\\Editor\\Unity.exe"))
    existing = [candidate for candidate in candidates if candidate.exists() and candidate.is_file()]
    if not existing:
        return None
    return sorted(existing, key=lambda item: item.parent.parent.parent.name if platform.system() == "Darwin" else item.name)[-1]


def _validate_manifest_file_entries(archive: zipfile.ZipFile, manifest: dict[str, Any]) -> None:
    names = set(archive.namelist())
    files = manifest.get("files")
    _assert(isinstance(files, list) and files, "manifest files must be a non-empty array")
    for entry in files:
        if entry.get("role") == "readme":
            continue
        path = entry.get("path")
        _assert(isinstance(path, str) and path in names, f"manifest entry missing from ZIP: {path}")
        payload = archive.read(path)
        _assert(entry.get("byte_size") == len(payload), f"manifest byte_size mismatch: {path}")
        _assert(entry.get("sha256") == hashlib.sha256(payload).hexdigest(), f"manifest sha256 mismatch: {path}")


def _parse_glb_bytes(payload: bytes) -> dict[str, Any]:
    _assert(payload[:4] == b"glTF", "GLB magic missing")
    version, total_length = struct.unpack("<II", payload[4:12])
    _assert(version == 2, "GLB version must be 2")
    _assert(total_length == len(payload), "GLB length header mismatch")
    json_length, json_type = struct.unpack("<I4s", payload[12:20])
    _assert(json_type == b"JSON", "GLB first chunk must be JSON")
    bin_header = 20 + json_length
    bin_length, bin_type = struct.unpack("<I4s", payload[bin_header:bin_header + 8])
    _assert(bin_type == b"BIN\x00", "GLB second chunk must be BIN")
    _assert(bin_length > 0, "GLB BIN chunk must not be empty")
    return json.loads(payload[20:20 + json_length].decode("utf-8"))


def _safe_zip_name(name: str) -> bool:
    path = Path(name)
    return bool(name) and not path.is_absolute() and ".." not in path.parts and "\\" not in name


def _single_suffix(names: Iterable[str], suffix: str) -> str:
    matches = [name for name in names if name.endswith(suffix)]
    _assert(len(matches) == 1, f"expected exactly one {suffix}, found {len(matches)}")
    return matches[0]


def _asset_id_by_role(job: Any, role: str) -> str:
    for asset_id, asset_role in job.outputs["asset_roles"].items():
        if asset_role == role:
            return asset_id
    raise AssertionError(f"Missing asset role {role}")


def _read_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        return {"ok": False, "code": "BAD_RESULT_JSON", "message": str(exc)}


def _assert(condition: Any, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
