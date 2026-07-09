#!/usr/bin/env python3
"""M4 HTTP smoke for patch asset upload and patch job creation."""

from __future__ import annotations

import base64
import hashlib
import json
import os
import socket
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import zlib
from pathlib import Path
from typing import Any, Dict, Optional

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.models import utc_now  # noqa: E402


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_m4_http_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        env = os.environ.copy()
        env["WUSHEN_LIBRARY_ROOT"] = str(library_root)
        env["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")

        process = subprocess.Popen(
            [
                sys.executable,
                "-m",
                "uvicorn",
                "wushen_agent.main:create_app",
                "--factory",
                "--host",
                "127.0.0.1",
                "--port",
                str(port),
            ],
            cwd=ROOT,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        try:
            _wait_for_health(base_url, process)
            create_body = {
                "client_request_id": "m4-http-source",
                "text": "玄铁青玉长戟，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产",
                "sketch_asset_id": None,
                "reference_asset_ids": [],
                "auto_run": True,
                "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
            }
            created = _json_request(base_url, "/api/weapons", method="POST", body=create_body, idempotency_key="m4-http-source-key")
            source_job = _json_request(base_url, f"/api/jobs/{created['job_id']}", method="GET")
            initial_detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            _assert(initial_detail["current_version_id"] == source_job["outputs"]["current_version_id"], "weapon detail current version mismatch")
            source_version_id = source_job["outputs"]["current_version_id"]
            source_image_id = _asset_id_by_role(source_job, "concept_image")
            source_image = _asset_row(library_root / "library.db", source_image_id)
            asset_meta = _json_request(base_url, f"/api/assets/{source_image_id}", method="GET")
            _assert(asset_meta["asset_id"] == source_image_id, "asset metadata endpoint returned wrong asset")
            source_bytes = _read_bytes(base_url, f"/api/assets/{source_image_id}/file")
            _assert(hashlib.sha256(source_bytes).hexdigest() == source_image["sha256"], "asset file endpoint sha256 mismatch")

            mask_body = {
                "client_request_id": "m4-http-mask",
                "role": "patch_mask",
                "filename": "core-mask.png",
                "mime_type": "image/png",
                "data_base64": _b64(png_mask(int(source_image["width"]), int(source_image["height"]), ink=True)),
                "metadata": {"purpose": "m4_http_smoke"},
            }
            mask = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/versions/{source_version_id}/assets",
                method="POST",
                body=mask_body,
                idempotency_key="m4-http-mask-key",
            )
            _assert(mask["logical_path"].endswith("core-mask.png"), "mask upload did not return logical_path")
            mask_replay = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/versions/{source_version_id}/assets",
                method="POST",
                body=mask_body,
                idempotency_key="m4-http-mask-key",
            )
            _assert(mask["asset_id"] == mask_replay["asset_id"], "asset upload idempotency replay returned a different asset")
            status, conflict = _json_request_allow_error(
                base_url,
                f"/api/weapons/{created['weapon_id']}/versions/{source_version_id}/assets",
                method="POST",
                body={**mask_body, "filename": "different-mask.png"},
                idempotency_key="m4-http-mask-key",
            )
            _assert(status == 409 and conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT", "upload idempotency conflict was not reported")

            mask_row = _asset_row(library_root / "library.db", mask["asset_id"])
            manifest = {
                "schema_version": "PatchManifest@1",
                "weapon_id": created["weapon_id"],
                "source_asset_id": source_image_id,
                "source_image": source_image["logical_path"],
                "mask_asset_id": mask["asset_id"],
                "mask_image": mask_row["logical_path"],
                "selection": {
                    "tool": "rectangle",
                    "polygon": [{"x": 160, "y": 140}, {"x": 430, "y": 140}, {"x": 430, "y": 360}, {"x": 160, "y": 360}],
                },
                "instruction": {"target": "core", "text": "把核心改成青蓝玉石雷纹能量核"},
                "preserve": ["overall_silhouette", "chinese_motifs", "toon_outline"],
                "strength": "medium",
                "regenerate_3d": False,
                "created_at": utc_now(),
            }
            manifest_body = {
                "client_request_id": "m4-http-manifest",
                "role": "patch_manifest",
                "filename": "patch-manifest.json",
                "mime_type": "application/json",
                "data_base64": _b64(json.dumps(manifest, ensure_ascii=False).encode("utf-8")),
                "metadata": {"purpose": "m4_http_smoke"},
            }
            manifest_upload = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/versions/{source_version_id}/assets",
                method="POST",
                body=manifest_body,
                idempotency_key="m4-http-manifest-key",
            )
            _assert(manifest_upload["logical_path"].endswith("patch-manifest.json"), "manifest upload did not return logical_path")

            patch_body = {
                "client_request_id": "m4-http-patch",
                "source_version_id": source_version_id,
                "source_image_asset_id": source_image_id,
                "mask_asset_id": mask["asset_id"],
                "patch_manifest_asset_id": manifest_upload["asset_id"],
                "target_area": "core",
                "instruction": "把核心改成青蓝玉石雷纹能量核，保持整体剪影和国风纹样",
                "preserve": ["overall_silhouette", "chinese_motifs", "toon_outline"],
                "strength": "medium",
                "regenerate_3d": False,
                "provider_id": "mock_comfyui",
            }
            patch = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/patch",
                method="POST",
                body=patch_body,
                idempotency_key="m4-http-patch-key",
            )
            patch_job = _json_request(base_url, f"/api/jobs/{patch['job_id']}", method="GET")
            _assert(patch_job["outputs"]["current_version_id"] != source_version_id, "patch did not create a new version")
            _assert("concept_patch" in patch_job["outputs"]["asset_roles"].values(), "patch job did not produce concept_patch")
            patched_detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            _assert(patched_detail["current_version_id"] == patch_job["outputs"]["current_version_id"], "patched detail did not expose current patch version")
            _assert(len(patched_detail["versions"]) == 2, "patched detail did not expose both source and patch versions")
            parent_detail = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/versions/{source_version_id}/activate",
                method="POST",
            )
            _assert(parent_detail["current_version_id"] == source_version_id, "activate parent version did not update current_version_id")
            patch_detail = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/versions/{patch_job['outputs']['current_version_id']}/activate",
                method="POST",
            )
            _assert(
                patch_detail["current_version_id"] == patch_job["outputs"]["current_version_id"],
                "activate patch version did not restore current_version_id",
            )
            missing_status, missing_body = _json_request_allow_error(
                base_url,
                f"/api/weapons/{created['weapon_id']}/versions/ver_missing/activate",
                method="POST",
            )
            _assert(missing_status == 404 and missing_body["error"]["code"] == "VERSION_NOT_FOUND", "missing activate version did not 404")

            findings, stats = validate(library_root, library_root / "library.db")
            _assert(stats["blockers"] == 0, f"asset library blockers: {findings}")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "weapon_id": created["weapon_id"],
                        "source_version_id": source_version_id,
                        "patch_version_id": patch_job["outputs"]["current_version_id"],
                        "mask_asset_id": mask["asset_id"],
                        "manifest_asset_id": manifest_upload["asset_id"],
                        "asset_count": stats["asset_files"],
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 0
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _wait_for_health(base_url: str, process: subprocess.Popen) -> None:
    deadline = time.time() + 15
    while time.time() < deadline:
        if process.poll() is not None:
            output = process.stdout.read() if process.stdout else ""
            raise RuntimeError(f"Agent exited before health check:\n{output}")
        try:
            health = _json_request(base_url, "/api/health", method="GET")
            if health.get("status") == "ok":
                return
        except Exception:
            time.sleep(0.2)
    raise TimeoutError("Agent health check timed out")


def _json_request(
    base_url: str, path: str, *, method: str, body: Optional[Dict[str, Any]] = None, idempotency_key: Optional[str] = None
) -> Dict[str, Any]:
    status, data = _json_request_allow_error(base_url, path, method=method, body=body, idempotency_key=idempotency_key)
    _assert(200 <= status < 300, f"{method} {path} failed with {status}: {data}")
    return data


def _json_request_allow_error(
    base_url: str, path: str, *, method: str, body: Optional[Dict[str, Any]] = None, idempotency_key: Optional[str] = None
) -> tuple[int, Dict[str, Any]]:
    payload = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(f"{base_url}{path}", data=payload, method=method)
    request.add_header("Content-Type", "application/json")
    if idempotency_key:
        request.add_header("Idempotency-Key", idempotency_key)
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        return exc.code, json.loads(exc.read().decode("utf-8"))

def _read_bytes(base_url: str, path: str) -> bytes:
    with urllib.request.urlopen(f"{base_url}{path}", timeout=10) as response:
        return response.read()


def _asset_id_by_role(job: Dict[str, Any], role: str) -> str:
    for file_id, item_role in job["outputs"]["asset_roles"].items():
        if item_role == role:
            return file_id
    raise AssertionError(f"asset role not found: {role}")


def _asset_row(db_path: Path, file_id: str) -> sqlite3.Row:
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    row = conn.execute("SELECT * FROM asset_files WHERE file_id = ?", (file_id,)).fetchone()
    if row is None:
        raise AssertionError(f"asset not found: {file_id}")
    return row


def png_mask(width: int, height: int, *, ink: bool) -> bytes:
    rows = []
    for y in range(height):
        row = bytearray([0])
        for x in range(width):
            value = 255 if ink and width // 4 <= x < width // 2 and height // 4 <= y < height // 2 else 0
            row.extend([value, value, value, value])
        rows.append(bytes(row))
    raw = b"".join(rows)
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + png_chunk(b"IDAT", zlib.compress(raw))
        + png_chunk(b"IEND", b"")
    )


def png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def _b64(payload: bytes) -> str:
    return base64.b64encode(payload).decode("ascii")


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
