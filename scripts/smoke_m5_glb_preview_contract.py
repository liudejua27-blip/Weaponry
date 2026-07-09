#!/usr/bin/env python3
"""M5 smoke checks for rough GLB validity and preview contract inputs."""

from __future__ import annotations

import json
import sqlite3
import struct
import sys
import tempfile
from pathlib import Path
from typing import Any

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest  # noqa: E402


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_m5_glb_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        store = SQLiteAssetStore(library_root=library_root, migrations_dir=ROOT / "migrations")
        job = store.create_weapon(
            CreateWeaponRequest(
                client_request_id="m5-glb-preview",
                text="赤金龙纹长剑，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产",
            ),
            idempotency_key="m5-glb-preview-key",
        )
        detail = store.get_weapon_detail(job.weapon_id)
        assert detail.current_model_id is not None
        glb_asset_id = _asset_id_by_role(job, "rough_raw_glb")
        unity_asset_id = _asset_id_by_role(job, "unity_material_json")
        report_asset_id = _model_quality_report_asset_id(library_root, store.db_path, job.job_id)
        glb = _asset_row(store.db_path, glb_asset_id)
        unity = _asset_row(store.db_path, unity_asset_id)
        report_asset = _asset_row(store.db_path, report_asset_id)
        _assert(glb["mime_type"] == "model/gltf-binary", "rough_raw_glb mime type mismatch")
        _assert(int(glb["byte_size"]) > 1024, "rough_raw_glb is too small to be a usable preview asset")
        _assert(unity["mime_type"] == "application/json", "unity_material_json mime type mismatch")
        gltf = _parse_glb(library_root / glb["object_path"])
        _assert(gltf["asset"]["version"] == "2.0", "GLB asset version mismatch")
        _assert(gltf["meshes"][0]["primitives"][0]["mode"] == 4, "GLB primitive is not triangles")
        _assert("POSITION" in gltf["meshes"][0]["primitives"][0]["attributes"], "GLB missing POSITION accessor")
        _assert("NORMAL" in gltf["meshes"][0]["primitives"][0]["attributes"], "GLB missing NORMAL accessor")
        report = json.loads((library_root / report_asset["object_path"]).read_text(encoding="utf-8"))
        mesh_evidence = _check_evidence(report, "MESH_NON_EMPTY")
        bounds_evidence = _check_evidence(report, "BOUNDING_BOX_VALID")
        material_evidence = _check_evidence(report, "MATERIAL_READABLE")
        _assert(int(mesh_evidence["triangle_count"]) > 0, "quality report missing triangle count")
        _assert(int(mesh_evidence["mesh_count"]) > 0, "quality report missing mesh count")
        _assert(float(bounds_evidence["longest_axis"]) > 0, "quality report missing model bounds")
        _assert(int(material_evidence["material_count"]) > 0, "quality report missing material count")
        findings, stats = validate(library_root, store.db_path)
        _assert(stats["blockers"] == 0, f"asset library blockers: {findings}")
        print(
            json.dumps(
                {
                    "ok": True,
                    "weapon_id": job.weapon_id,
                    "model_id": detail.current_model_id,
                    "glb_asset_id": glb_asset_id,
                    "unity_material_asset_id": unity_asset_id,
                    "quality_report_asset_id": report_asset_id,
                    "glb_bytes": int(glb["byte_size"]),
                    "triangle_count": int(mesh_evidence["triangle_count"]),
                    "material_count": int(material_evidence["material_count"]),
                    "asset_count": stats["asset_files"],
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _parse_glb(path: Path) -> dict[str, Any]:
    payload = path.read_bytes()
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


def _asset_id_by_role(job: Any, role: str) -> str:
    for asset_id, asset_role in job.outputs["asset_roles"].items():
        if asset_role == role:
            return asset_id
    raise AssertionError(f"Missing asset role {role}")


def _asset_row(db_path: Path, asset_id: str) -> sqlite3.Row:
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    row = conn.execute("SELECT * FROM asset_files WHERE file_id = ?", (asset_id,)).fetchone()
    if row is None:
        raise AssertionError(f"Missing asset {asset_id}")
    return row


def _model_quality_report_asset_id(library_root: Path, db_path: Path, job_id: str) -> str:
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    rows = conn.execute(
        "SELECT file_id, object_path FROM asset_files WHERE job_id = ? AND role = 'quality_report'",
        (job_id,),
    ).fetchall()
    for row in rows:
        report = json.loads((library_root / row["object_path"]).read_text(encoding="utf-8"))
        if report.get("target_type") == "model_3d":
            return str(row["file_id"])
    raise AssertionError("Missing model_3d quality report")


def _check_evidence(report: dict[str, Any], code: str) -> dict[str, Any]:
    for check in report.get("checks", []):
        if check.get("code") == code:
            evidence = check.get("evidence")
            if isinstance(evidence, dict):
                return evidence
    raise AssertionError(f"Missing quality report check {code}")


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
