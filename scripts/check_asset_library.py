#!/usr/bin/env python3
"""Validate a Wushen Forge asset library.

Exit codes:
0 = no blocker
1 = warnings only
2 = blocker found
3 = database or schema could not be opened
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sqlite3
import struct
import sys
import zipfile
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Optional

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.spec_validation import validate_quality_report  # noqa: E402


@dataclass
class Finding:
    level: str
    code: str
    message: str
    file_id: Optional[str] = None
    path: Optional[str] = None


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def is_relative_safe(value: str) -> bool:
    p = Path(value)
    return not p.is_absolute() and ".." not in p.parts and "://" not in value


def validate(library_root: Path, db_path: Path) -> tuple[list[Finding], dict[str, int]]:
    findings: list[Finding] = []
    stats = {"asset_files": 0, "blockers": 0, "warnings": 0}

    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys=ON")

    for row in conn.execute("PRAGMA foreign_key_check").fetchall():
      findings.append(Finding("blocker", "FOREIGN_KEY", f"Foreign key violation: {tuple(row)}"))

    rows = conn.execute(
        """
        SELECT file_id, object_path, logical_path, sha256, byte_size, role, mime_type, soft_deleted_at
        FROM asset_files
        WHERE soft_deleted_at IS NULL
        """
    ).fetchall()

    stats["asset_files"] = len(rows)

    for row in rows:
        file_id = row["file_id"]
        object_path = row["object_path"]
        if not is_relative_safe(object_path):
            findings.append(Finding("blocker", "PATH_ESCAPE", "object_path is not library-relative", file_id, object_path))
            continue

        full_path = library_root / object_path
        try:
            full_path.resolve().relative_to(library_root.resolve())
        except ValueError:
            findings.append(Finding("blocker", "PATH_ESCAPE", "object_path escapes library root", file_id, object_path))
            continue

        if not full_path.exists():
            findings.append(Finding("blocker", "MISSING_FILE", "asset file is missing", file_id, object_path))
            continue

        actual_size = full_path.stat().st_size
        if actual_size != row["byte_size"]:
            findings.append(Finding("blocker", "SIZE_MISMATCH", f"expected {row['byte_size']}, got {actual_size}", file_id, object_path))

        actual_hash = sha256_file(full_path)
        if actual_hash != row["sha256"]:
            findings.append(Finding("blocker", "HASH_MISMATCH", "sha256 does not match file contents", file_id, object_path))

        logical_path = row["logical_path"]
        if not is_relative_safe(logical_path):
            findings.append(Finding("warning", "UNSAFE_LOGICAL_PATH", "logical_path is not safely relative", file_id, logical_path))

    for row in conn.execute(
        """
        SELECT job_id, job_type
        FROM generation_jobs
        WHERE status IN ('succeeded', 'partial_succeeded')
        """
    ).fetchall():
        asset_count = conn.execute(
            "SELECT COUNT(*) AS count FROM asset_files WHERE job_id = ? AND soft_deleted_at IS NULL",
            (row["job_id"],),
        ).fetchone()["count"]
        if asset_count == 0:
            findings.append(Finding("blocker", "JOB_WITHOUT_ASSETS", f"successful {row['job_type']} job has no assets", row["job_id"]))

    for row in conn.execute(
        """
        SELECT w.weapon_id, w.current_version_id
        FROM weapons w
        WHERE w.status = 'active' AND w.current_version_id IS NOT NULL
        """
    ).fetchall():
        version = conn.execute(
            """
            SELECT 1
            FROM weapon_versions
            WHERE version_id = ? AND weapon_id = ?
            """,
            (row["current_version_id"], row["weapon_id"]),
        ).fetchone()
        if version is None:
            findings.append(
                Finding(
                    "blocker",
                    "INVALID_CURRENT_VERSION",
                    "weapons.current_version_id does not point to a version for the same weapon",
                    row["weapon_id"],
                    row["current_version_id"],
                )
            )

    for row in conn.execute(
        """
        SELECT event_id, artifact_asset_id
        FROM agent_events
        WHERE artifact_asset_id IS NOT NULL
        """
    ).fetchall():
        asset = conn.execute(
            "SELECT 1 FROM asset_files WHERE file_id = ? AND soft_deleted_at IS NULL",
            (row["artifact_asset_id"],),
        ).fetchone()
        if asset is None:
            findings.append(
                Finding(
                    "blocker",
                    "EVENT_ARTIFACT_MISSING",
                    "agent event references a missing asset",
                    row["event_id"],
                    row["artifact_asset_id"],
                )
            )

    _validate_create_weapon_jobs(conn, library_root, findings)
    _validate_patch_jobs(conn, library_root, findings)
    _validate_generate_3d_jobs(conn, library_root, findings)
    _validate_export_unity_jobs(conn, library_root, findings)

    secret_markers = ["sk-", "api_key", "apikey", "authorization", "bearer "]
    for table, column in [("provider_configs", "config_json"), ("agent_events", "metadata_json"), ("asset_files", "metadata_json")]:
        for row in conn.execute(f"SELECT {column} AS payload FROM {table}").fetchall():
            payload = (row["payload"] or "").lower()
            if any(marker in payload for marker in secret_markers):
                findings.append(Finding("blocker", "PLAINTEXT_SECRET", f"possible secret marker in {table}.{column}"))

    for finding in findings:
        if finding.level == "blocker":
            stats["blockers"] += 1
        elif finding.level == "warning":
            stats["warnings"] += 1

    return findings, stats


def _validate_create_weapon_jobs(conn: sqlite3.Connection, library_root: Path, findings: list[Finding]) -> None:
    required_roles = {
        "weapon_spec",
        "prompt",
        "negative_prompt",
        "comfyui_workflow",
        "concept_image",
        "quality_report",
        "rough_raw_glb",
        "unity_material_json",
    }
    expected_event_steps = [
        "request_guard",
        "weapon_spec_planner",
        "prompt_builder",
        "image_submit",
        "image_quality_check",
        "rough3d_submit",
        "finalize_job",
    ]

    jobs = conn.execute(
        """
        SELECT job_id, weapon_id
        FROM generation_jobs
        WHERE job_type = 'create_weapon' AND status = 'succeeded'
        """
    ).fetchall()
    for job in jobs:
        job_id = job["job_id"]
        assets = conn.execute(
            """
            SELECT file_id, role, weapon_id, version_id, object_path, sha256, width, height, metadata_json
            FROM asset_files
            WHERE job_id = ? AND soft_deleted_at IS NULL
            """,
            (job_id,),
        ).fetchall()
        by_role: dict[str, list[sqlite3.Row]] = {}
        for asset in assets:
            by_role.setdefault(asset["role"], []).append(asset)

        missing = sorted(required_roles - set(by_role))
        if missing:
            findings.append(Finding("blocker", "CREATE_WEAPON_MISSING_ASSETS", f"create_weapon job missing roles: {missing}", job_id))
            continue

        concept = by_role["concept_image"][0]
        workflow = by_role["comfyui_workflow"][0]
        prompt = by_role["prompt"][0]
        report_asset = by_role["quality_report"][0]
        if not concept["width"] or not concept["height"]:
            findings.append(Finding("blocker", "CONCEPT_DIMENSIONS_MISSING", "concept image is missing width/height", concept["file_id"]))

        try:
            concept_meta = json.loads(concept["metadata_json"] or "{}")
            workflow_meta = json.loads(workflow["metadata_json"] or "{}")
        except json.JSONDecodeError as exc:
            findings.append(Finding("blocker", "BAD_ASSET_METADATA", str(exc), job_id))
            continue

        if concept_meta.get("workflow_asset_id") != workflow["file_id"]:
            findings.append(Finding("blocker", "CONCEPT_WORKFLOW_LINK_MISSING", "concept image does not reference workflow asset", concept["file_id"]))
        if concept_meta.get("prompt_asset_id") != prompt["file_id"]:
            findings.append(Finding("blocker", "CONCEPT_PROMPT_LINK_MISSING", "concept image does not reference prompt asset", concept["file_id"]))
        if not workflow_meta.get("provider_task_id"):
            findings.append(Finding("blocker", "WORKFLOW_TASK_ID_MISSING", "workflow metadata is missing provider_task_id", workflow["file_id"]))
        if workflow_meta.get("workflow_sha256") != workflow["sha256"]:
            findings.append(Finding("blocker", "WORKFLOW_SHA_MISMATCH", "workflow metadata hash does not match asset sha256", workflow["file_id"]))
        provenance = workflow_meta.get("generation_provenance") or {}
        sampler = provenance.get("sampler") if isinstance(provenance, dict) else None
        if not workflow_meta.get("checkpoint_name") or not isinstance(sampler, dict):
            findings.append(Finding("blocker", "WORKFLOW_PROVENANCE_MISSING", "workflow metadata is missing checkpoint or sampler provenance", workflow["file_id"]))
        elif not all(sampler.get(key) is not None for key in ["seed", "steps", "cfg", "sampler_name", "scheduler", "denoise"]):
            findings.append(Finding("blocker", "WORKFLOW_SAMPLER_PROVENANCE_INCOMPLETE", "sampler provenance is incomplete", workflow["file_id"]))
        _validate_glb_asset(library_root, by_role["rough_raw_glb"][0], findings)

        try:
            quality_report = json.loads((library_root / report_asset["object_path"]).read_text(encoding="utf-8"))
            validate_quality_report(quality_report, provider_id="asset_library_check")
        except Exception as exc:  # noqa: BLE001 - report validation should become a library finding.
            findings.append(Finding("blocker", "QUALITY_REPORT_INVALID", str(exc), report_asset["file_id"]))
            continue

        if quality_report.get("target_type") != "concept_image" or quality_report.get("target_id") != concept["file_id"]:
            findings.append(Finding("blocker", "QUALITY_REPORT_TARGET_MISMATCH", "quality report does not target the concept image", report_asset["file_id"]))
        if quality_report.get("status") not in {"passed", "warning"}:
            findings.append(Finding("blocker", "QUALITY_GATE_FAILED", "rough 3D cannot proceed without a passing concept quality report", report_asset["file_id"]))

        events = conn.execute(
            """
            SELECT seq, step, artifact_asset_id, metadata_json
            FROM agent_events
            WHERE job_id = ?
            ORDER BY seq ASC
            """,
            (job_id,),
        ).fetchall()
        steps = [event["step"] for event in events]
        if steps != expected_event_steps:
            findings.append(Finding("blocker", "CREATE_WEAPON_EVENT_ORDER", f"unexpected event order: {steps}", job_id))
            continue
        event_by_step = {event["step"]: event for event in events}
        if event_by_step["image_quality_check"]["artifact_asset_id"] != report_asset["file_id"]:
            findings.append(Finding("blocker", "QUALITY_EVENT_ARTIFACT_MISMATCH", "quality event does not point to quality report", job_id))
        try:
            rough_meta = json.loads(event_by_step["rough3d_submit"]["metadata_json"] or "{}")
        except json.JSONDecodeError:
            rough_meta = {}
        if rough_meta.get("gated_by") != report_asset["file_id"]:
            findings.append(Finding("blocker", "ROUGH3D_NOT_GATED", "rough3d_submit is not gated by concept quality report", job_id))


def _validate_glb_asset(library_root: Path, asset: sqlite3.Row, findings: list[Finding]) -> None:
    path = library_root / asset["object_path"]
    try:
        payload = path.read_bytes()
        if len(payload) < 28 or payload[:4] != b"glTF":
            raise ValueError("missing GLB magic")
        version, total_length = struct.unpack("<II", payload[4:12])
        if version != 2:
            raise ValueError(f"unsupported GLB version {version}")
        if total_length != len(payload):
            raise ValueError("GLB length header does not match file size")
        json_length, json_type = struct.unpack("<I4s", payload[12:20])
        if json_type != b"JSON":
            raise ValueError("first GLB chunk is not JSON")
        json.loads(payload[20:20 + json_length].decode("utf-8"))
        bin_header = 20 + json_length
        bin_length, bin_type = struct.unpack("<I4s", payload[bin_header:bin_header + 8])
        if bin_type != b"BIN\x00" or bin_length <= 0:
            raise ValueError("second GLB chunk is not a non-empty BIN chunk")
    except Exception as exc:  # noqa: BLE001 - library validation should report bad assets.
        findings.append(Finding("blocker", "GLB_INVALID", f"rough_raw_glb is not a valid binary glTF: {exc}", asset["file_id"]))


def _validate_patch_jobs(conn: sqlite3.Connection, library_root: Path, findings: list[Finding]) -> None:
    expected_event_steps = ["patch_interpreter", "image_inpaint", "image_quality_check", "finalize_job"]
    jobs = conn.execute(
        """
        SELECT job_id, weapon_id
        FROM generation_jobs
        WHERE job_type = 'patch_image' AND status = 'succeeded'
        """
    ).fetchall()
    for job in jobs:
        job_id = job["job_id"]
        version = conn.execute(
            """
            SELECT version_id, parent_version_id, version_type
            FROM weapon_versions
            WHERE job_id = ?
            """,
            (job_id,),
        ).fetchone()
        if version is None or version["version_type"] != "patch" or not version["parent_version_id"]:
            findings.append(Finding("blocker", "PATCH_VERSION_INVALID", "patch job did not create a patch version with parent", job_id))
            continue
        assets = conn.execute(
            """
            SELECT file_id, role, object_path, sha256, width, height, metadata_json
            FROM asset_files
            WHERE job_id = ? AND soft_deleted_at IS NULL
            """,
            (job_id,),
        ).fetchall()
        by_role: dict[str, list[sqlite3.Row]] = {}
        for asset in assets:
            by_role.setdefault(asset["role"], []).append(asset)
        missing = sorted({"patch_prompt", "concept_patch", "quality_report"} - set(by_role))
        if missing:
            findings.append(Finding("blocker", "PATCH_MISSING_ASSETS", f"patch job missing roles: {missing}", job_id))
            continue
        patch_asset = by_role["concept_patch"][0]
        if not patch_asset["width"] or not patch_asset["height"]:
            findings.append(Finding("blocker", "PATCH_DIMENSIONS_MISSING", "concept patch is missing width/height", patch_asset["file_id"]))
        try:
            report_asset = by_role["quality_report"][0]
            quality_report = json.loads((library_root / report_asset["object_path"]).read_text(encoding="utf-8"))
            validate_quality_report(quality_report, provider_id="asset_library_patch_check")
        except Exception as exc:  # noqa: BLE001 - report validation should become a library finding.
            findings.append(Finding("blocker", "PATCH_QUALITY_REPORT_INVALID", str(exc), job_id))
            continue
        if quality_report.get("target_type") != "patch_image" or quality_report.get("target_id") != patch_asset["file_id"]:
            findings.append(Finding("blocker", "PATCH_QUALITY_TARGET_MISMATCH", "patch quality report does not target concept_patch", report_asset["file_id"]))
        events = conn.execute(
            "SELECT step FROM agent_events WHERE job_id = ? ORDER BY seq",
            (job_id,),
        ).fetchall()
        steps = [event["step"] for event in events]
        if steps != expected_event_steps:
            findings.append(Finding("blocker", "PATCH_EVENT_ORDER", f"unexpected patch event order: {steps}", job_id))


def _validate_generate_3d_jobs(conn: sqlite3.Connection, library_root: Path, findings: list[Finding]) -> None:
    expected_event_steps = ["rough3d_plan", "rough3d_submit", "model_qc_optimize", "asset_commit_model", "finalize_job"]
    required_roles = {"other", "rough_raw_glb", "rough_normalized_glb", "rough_optimized_glb", "unity_material_json", "quality_report"}
    jobs = conn.execute(
        """
        SELECT job_id, weapon_id
        FROM generation_jobs
        WHERE job_type = 'generate_3d' AND status = 'succeeded'
        """
    ).fetchall()
    for job in jobs:
        job_id = job["job_id"]
        version = conn.execute(
            """
            SELECT version_id, parent_version_id, version_type
            FROM weapon_versions
            WHERE job_id = ?
            """,
            (job_id,),
        ).fetchone()
        if version is None or version["version_type"] != "rough_3d" or not version["parent_version_id"]:
            findings.append(Finding("blocker", "GENERATE3D_VERSION_INVALID", "generate_3d must create a rough_3d child version", job_id))
            continue
        assets = conn.execute(
            """
            SELECT file_id, role, object_path, sha256, width, height, metadata_json
            FROM asset_files
            WHERE job_id = ? AND soft_deleted_at IS NULL
            """,
            (job_id,),
        ).fetchall()
        by_role: dict[str, list[sqlite3.Row]] = {}
        for asset in assets:
            by_role.setdefault(asset["role"], []).append(asset)
        missing = sorted(required_roles - set(by_role))
        if missing:
            findings.append(Finding("blocker", "GENERATE3D_MISSING_ASSETS", f"generate_3d job missing roles: {missing}", job_id))
            continue
        for role in ["rough_raw_glb", "rough_normalized_glb", "rough_optimized_glb"]:
            _validate_glb_asset(library_root, by_role[role][0], findings)
        report_asset = by_role["quality_report"][0]
        try:
            report = json.loads((library_root / report_asset["object_path"]).read_text(encoding="utf-8"))
            validate_quality_report(report, provider_id="asset_library_model_check")
        except Exception as exc:  # noqa: BLE001 - report validation should become a library finding.
            findings.append(Finding("blocker", "MODEL_QUALITY_REPORT_INVALID", str(exc), report_asset["file_id"]))
            continue
        if report.get("target_type") != "model_3d" or report.get("status") not in {"passed", "warning"}:
            findings.append(Finding("blocker", "MODEL_QUALITY_TARGET_MISMATCH", "model quality report does not target model_3d", report_asset["file_id"]))
        _validate_model_quality_metrics(report, report_asset["file_id"], findings)
        events = conn.execute(
            """
            SELECT step, status, artifact_asset_id
            FROM agent_events
            WHERE job_id = ?
            ORDER BY seq
            """,
            (job_id,),
        ).fetchall()
        succeeded_steps = [event["step"] for event in events if event["status"] == "succeeded"]
        if succeeded_steps != expected_event_steps:
            findings.append(Finding("blocker", "GENERATE3D_EVENT_ORDER", f"unexpected generate_3d succeeded event order: {succeeded_steps}", job_id))
            continue
        event_by_step = {event["step"]: event for event in events if event["status"] == "succeeded"}
        if event_by_step["model_qc_optimize"]["artifact_asset_id"] != report_asset["file_id"]:
            findings.append(Finding("blocker", "MODEL_QC_EVENT_ARTIFACT_MISMATCH", "model_qc_optimize event does not point to model quality report", job_id))


def _validate_model_quality_metrics(report: dict, report_file_id: str, findings: list[Finding]) -> None:
    checks = report.get("checks")
    if not isinstance(checks, list):
        findings.append(Finding("blocker", "MODEL_QUALITY_METRICS_MISSING", "model quality report checks are missing", report_file_id))
        return
    by_code = {check.get("code"): check for check in checks if isinstance(check, dict)}
    mesh = by_code.get("MESH_NON_EMPTY")
    bounds = by_code.get("BOUNDING_BOX_VALID")
    material = by_code.get("MATERIAL_READABLE")
    if not isinstance(mesh, dict):
        findings.append(Finding("blocker", "MODEL_QUALITY_MESH_CHECK_MISSING", "model quality report is missing MESH_NON_EMPTY", report_file_id))
    else:
        evidence = mesh.get("evidence") if isinstance(mesh.get("evidence"), dict) else {}
        if int(evidence.get("triangle_count") or 0) <= 0 or int(evidence.get("mesh_count") or 0) <= 0:
            findings.append(Finding("blocker", "MODEL_QUALITY_MESH_EMPTY", "model quality report did not prove non-empty mesh", report_file_id))
    if not isinstance(bounds, dict):
        findings.append(Finding("blocker", "MODEL_QUALITY_BOUNDS_CHECK_MISSING", "model quality report is missing BOUNDING_BOX_VALID", report_file_id))
    else:
        evidence = bounds.get("evidence") if isinstance(bounds.get("evidence"), dict) else {}
        longest_axis = evidence.get("longest_axis")
        if not _positive_number(longest_axis) or not isinstance(evidence.get("bounds"), dict):
            findings.append(Finding("blocker", "MODEL_QUALITY_BOUNDS_INVALID", "model quality report did not prove finite model bounds", report_file_id))
    if not isinstance(material, dict):
        findings.append(Finding("warning", "MODEL_QUALITY_MATERIAL_CHECK_MISSING", "model quality report is missing MATERIAL_READABLE", report_file_id))
    else:
        evidence = material.get("evidence") if isinstance(material.get("evidence"), dict) else {}
        if int(evidence.get("material_count") or 0) <= 0:
            findings.append(Finding("warning", "MODEL_QUALITY_NO_MATERIALS", "model quality report has no material slots", report_file_id))


def _positive_number(value: object) -> bool:
    try:
        return float(value) > 0
    except (TypeError, ValueError):
        return False


def _validate_export_unity_jobs(conn: sqlite3.Connection, library_root: Path, findings: list[Finding]) -> None:
    expected_event_steps = ["export_plan", "export_manifest", "export_package", "finalize_job"]
    required_entries = {
        "manifest.json",
        "README_WUSHEN.txt",
        "Models/rough_optimized.glb",
        "Materials/unity_material.json",
        "Reports/model_quality_report.json",
        "Specs/weapon_spec.json",
    }
    jobs = conn.execute(
        """
        SELECT job_id, weapon_id
        FROM generation_jobs
        WHERE job_type = 'export_unity' AND status = 'succeeded'
        """
    ).fetchall()
    for job in jobs:
        job_id = job["job_id"]
        version = conn.execute(
            """
            SELECT version_id, parent_version_id, version_type
            FROM weapon_versions
            WHERE job_id = ?
            """,
            (job_id,),
        ).fetchone()
        if version is None or version["version_type"] != "export" or not version["parent_version_id"]:
            findings.append(Finding("blocker", "EXPORT_VERSION_INVALID", "export_unity must create an export child version", job_id))
            continue
        package = conn.execute(
            """
            SELECT file_id, role, object_path, sha256, mime_type, metadata_json
            FROM asset_files
            WHERE job_id = ? AND role = 'unity_export_package' AND soft_deleted_at IS NULL
            """,
            (job_id,),
        ).fetchone()
        if package is None:
            findings.append(Finding("blocker", "EXPORT_PACKAGE_MISSING", "export_unity job missing unity_export_package asset", job_id))
            continue
        if package["mime_type"] != "application/zip":
            findings.append(Finding("blocker", "EXPORT_PACKAGE_MIME", "unity export package must be application/zip", package["file_id"]))
        export_row = conn.execute(
            """
            SELECT export_id, status, package_path, manifest_json
            FROM export_packages
            WHERE job_id = ?
            """,
            (job_id,),
        ).fetchone()
        if export_row is None or export_row["status"] != "validated":
            findings.append(Finding("blocker", "EXPORT_ROW_MISSING", "export_packages row is missing or not validated", job_id))
            continue
        if not is_relative_safe(export_row["package_path"]):
            findings.append(Finding("blocker", "EXPORT_PACKAGE_PATH_UNSAFE", "export package_path must be relative", job_id, export_row["package_path"]))
        try:
            manifest = json.loads(export_row["manifest_json"] or "{}")
        except json.JSONDecodeError as exc:
            findings.append(Finding("blocker", "EXPORT_MANIFEST_INVALID", str(exc), job_id))
            continue
        if manifest.get("schema_version") != "UnityExportManifest@1":
            findings.append(Finding("blocker", "EXPORT_MANIFEST_SCHEMA", "Unity export manifest schema_version mismatch", job_id))
        if not manifest.get("safety_boundary", {}).get("non_manufacturing_asset"):
            findings.append(Finding("blocker", "EXPORT_SAFETY_BOUNDARY", "Unity export manifest must declare non_manufacturing_asset", job_id))
        for item in manifest.get("files", []):
            path = item.get("path", "")
            if not is_relative_safe(path):
                findings.append(Finding("blocker", "EXPORT_ENTRY_PATH_UNSAFE", "Unity export manifest contains unsafe package path", job_id, path))
        archive_path = library_root / package["object_path"]
        try:
            with zipfile.ZipFile(archive_path) as archive:
                names = set(archive.namelist())
                stripped = {"/".join(Path(name).parts[-2:]) if "/Models/" in name or "/Materials/" in name or "/Reports/" in name or "/Specs/" in name else Path(name).name for name in names}
                missing = sorted(required_entries - stripped)
                if missing:
                    findings.append(Finding("blocker", "EXPORT_ZIP_MISSING_ENTRIES", f"Unity export package missing entries: {missing}", package["file_id"]))
                for name in names:
                    if not is_relative_safe(name):
                        findings.append(Finding("blocker", "EXPORT_ZIP_PATH_UNSAFE", "Unity export zip entry path is unsafe", package["file_id"], name))
        except zipfile.BadZipFile as exc:
            findings.append(Finding("blocker", "EXPORT_ZIP_INVALID", str(exc), package["file_id"]))
        events = conn.execute(
            "SELECT step, status, artifact_asset_id FROM agent_events WHERE job_id = ? ORDER BY seq",
            (job_id,),
        ).fetchall()
        succeeded_events = [event for event in events if event["status"] == "succeeded"]
        steps = [event["step"] for event in succeeded_events]
        if steps != expected_event_steps:
            findings.append(Finding("blocker", "EXPORT_EVENT_ORDER", f"unexpected export_unity succeeded event order: {steps}", job_id))
        elif any(event["artifact_asset_id"] != package["file_id"] for event in succeeded_events[1:]):
            findings.append(Finding("blocker", "EXPORT_EVENT_ARTIFACT_MISMATCH", "export events must point to package asset after planning", job_id))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--library-root", required=True)
    parser.add_argument("--db", required=True)
    parser.add_argument("--json-report")
    args = parser.parse_args()

    library_root = Path(args.library_root).expanduser()
    db_path = Path(args.db).expanduser()

    try:
        findings, stats = validate(library_root, db_path)
    except Exception as exc:  # noqa: BLE001 - CLI should report any DB/schema failure.
        report = {
            "ok": False,
            "stats": {"asset_files": 0, "blockers": 1, "warnings": 0},
            "findings": [asdict(Finding("blocker", "DB_OPEN_FAILED", str(exc)))],
        }
        if args.json_report:
            Path(args.json_report).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 3

    report = {
        "ok": stats["blockers"] == 0,
        "stats": stats,
        "findings": [asdict(f) for f in findings],
    }

    if args.json_report:
        Path(args.json_report).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))

    if stats["blockers"]:
        return 2
    if stats["warnings"]:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
