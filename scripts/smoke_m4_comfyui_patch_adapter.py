#!/usr/bin/env python3
"""M4 smoke checks for the ComfyUI HTTP patch/inpaint adapter boundary."""

from __future__ import annotations

import json
import re
import sqlite3
import struct
import sys
import tempfile
import threading
import zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Dict

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest, PatchWeaponRequest, utc_now  # noqa: E402
from wushen_agent.providers.image import ComfyUIConfig, ComfyUIHTTPProvider  # noqa: E402
from wushen_agent.providers.llm import MockLLMProvider  # noqa: E402


PNG_BYTES = (
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01"
    b"\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\nIDATx\x9cc\x00\x01"
    b"\x00\x00\x05\x00\x01\r\n-\xb4\x00\x00\x00\x00IEND\xaeB`\x82"
)


class FakeComfyState:
    prompt_payloads: list[Dict[str, Any]] = []
    uploads: list[str] = []
    view_requests = 0

    @classmethod
    def reset(cls) -> None:
        cls.prompt_payloads = []
        cls.uploads = []
        cls.view_requests = 0

    @classmethod
    def prompt_id(cls) -> str:
        return f"prompt_fake_{len(cls.prompt_payloads):03d}"


class FakeComfyHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        if self.path == "/upload/image":
            length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(length)
            match = re.search(br'filename="([^"]+)"', body)
            filename = match.group(1).decode("utf-8") if match else f"upload_{len(FakeComfyState.uploads)}.png"
            FakeComfyState.uploads.append(filename)
            self._send_json({"name": filename, "subfolder": "", "type": "input"})
            return
        if self.path == "/prompt":
            length = int(self.headers.get("Content-Length", "0"))
            FakeComfyState.prompt_payloads.append(json.loads(self.rfile.read(length).decode("utf-8")))
            self._send_json({"prompt_id": FakeComfyState.prompt_id(), "number": len(FakeComfyState.prompt_payloads)})
            return
        self.send_error(404)

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        if self.path.startswith("/history/prompt_fake_"):
            prompt_id = self.path.rsplit("/", 1)[-1]
            filename = "patch.png" if prompt_id.endswith("002") else "concept.png"
            self._send_json(
                {
                    prompt_id: {
                        "outputs": {
                            "9": {
                                "images": [
                                    {"filename": filename, "subfolder": "", "type": "output"}
                                ]
                            }
                        }
                    }
                }
            )
            return
        if self.path.startswith("/view?"):
            FakeComfyState.view_requests += 1
            self.send_response(200)
            self.send_header("Content-Type", "image/png")
            self.send_header("Content-Length", str(len(PNG_BYTES)))
            self.end_headers()
            self.wfile.write(PNG_BYTES)
            return
        self.send_error(404)

    def log_message(self, _format: str, *_args: Any) -> None:
        return

    def _send_json(self, payload: Dict[str, Any]) -> None:
        data = json.dumps(payload).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def main() -> int:
    FakeComfyState.reset()
    server = ThreadingHTTPServer(("127.0.0.1", 0), FakeComfyHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base_url = f"http://127.0.0.1:{server.server_port}"

    try:
        _run_patch_adapter_case(base_url)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)
    return 0


def _run_patch_adapter_case(base_url: str) -> None:
    with tempfile.TemporaryDirectory(prefix="wushen_m4_comfyui_patch_") as tmp:
        store = SQLiteAssetStore(
            library_root=Path(tmp) / "WushenForgeLibrary",
            migrations_dir=ROOT / "migrations",
            llm_provider=MockLLMProvider(),
            image_provider=ComfyUIHTTPProvider(
                ComfyUIConfig(
                    base_url=base_url,
                    timeout_seconds=5,
                    poll_interval_seconds=0.05,
                    max_wait_seconds=3,
                    retry_attempts=3,
                    retry_backoff_seconds=0.01,
                )
            ),
        )
        create_request = CreateWeaponRequest(
            client_request_id="m4-comfyui-patch-source",
            text="赤金龙纹长剑，3渲2国风神兵，逼真外观，仅作为虚构 Unity 游戏资产",
        )
        source_job = store.create_weapon(create_request, idempotency_key="m4-comfyui-patch-source-key")
        source_version_id = source_job.outputs["current_version_id"]
        source_image_id = _asset_id_by_role(source_job, "concept_image")
        source_image = _asset_row(store.db_path, source_image_id)
        with store._connect() as conn:  # noqa: SLF001 - smoke tests persistent asset invariants.
            mask_id = store._write_asset(
                conn,
                weapon_id=source_job.weapon_id,
                version_id=source_version_id,
                job_id=None,
                role="patch_mask",
                logical_path=f"weapons/{source_job.weapon_id}/versions/{source_version_id}/uploads/patch-mask.png",
                payload=png_mask(1, 1, ink=True),
                ext=".png",
                mime_type="image/png",
                metadata={"purpose": "m4_comfyui_patch_smoke"},
                width=1,
                height=1,
            )
            manifest_id = store._write_asset(
                conn,
                weapon_id=source_job.weapon_id,
                version_id=source_version_id,
                job_id=None,
                role="patch_manifest",
                logical_path=f"weapons/{source_job.weapon_id}/versions/{source_version_id}/uploads/patch-manifest.json",
                payload=json.dumps(
                    {
                        "schema_version": "PatchManifest@1",
                        "weapon_id": source_job.weapon_id,
                        "source_asset_id": source_image_id,
                        "source_image": source_image["logical_path"],
                        "mask_asset_id": mask_id,
                        "mask_image": f"weapons/{source_job.weapon_id}/versions/{source_version_id}/uploads/patch-mask.png",
                        "selection": {
                            "tool": "rectangle",
                            "polygon": [{"x": 0, "y": 0}, {"x": 1, "y": 0}, {"x": 1, "y": 1}, {"x": 0, "y": 1}],
                        },
                        "instruction": {"target": "blade", "text": "把剑身局部改成赤金龙纹发光刻印"},
                        "preserve": ["overall_silhouette", "chinese_motifs", "toon_outline"],
                        "strength": "strong",
                        "regenerate_3d": False,
                        "created_at": utc_now(),
                    },
                    ensure_ascii=False,
                ).encode("utf-8"),
                ext=".json",
                mime_type="application/json",
                metadata={"schema_version": "PatchManifest@1"},
            )
            conn.commit()
        patch_request = PatchWeaponRequest(
            client_request_id="m4-comfyui-patch",
            source_version_id=source_version_id,
            source_image_asset_id=source_image_id,
            mask_asset_id=mask_id,
            patch_manifest_asset_id=manifest_id,
            target_area="blade",
            instruction="把剑身局部改成赤金龙纹发光刻印，保持整体剪影",
            preserve=["overall_silhouette", "chinese_motifs", "toon_outline"],
            strength="strong",
            provider_id="comfyui",
        )
        patch_job = store.patch_weapon(source_job.weapon_id, patch_request, idempotency_key="m4-comfyui-patch-key")

        assert len(FakeComfyState.prompt_payloads) == 2
        assert len(FakeComfyState.uploads) == 2
        patch_prompt = FakeComfyState.prompt_payloads[1]["prompt"]
        assert patch_prompt["10"]["inputs"]["image"] == FakeComfyState.uploads[0]
        assert patch_prompt["11"]["inputs"]["image"] == FakeComfyState.uploads[1]
        assert patch_prompt["3"]["inputs"]["denoise"] == 0.75
        assert patch_prompt["12"]["inputs"]["width"] == 1
        assert patch_prompt["12"]["inputs"]["height"] == 1

        _assert_patch_assets(store.db_path, patch_job.job_id)
        findings, stats = validate(store.library_root, store.db_path)
        assert stats["blockers"] == 0, findings
        print(
            json.dumps(
                {
                    "ok": True,
                    "provider": "comfyui",
                    "weapon_id": source_job.weapon_id,
                    "patch_job_id": patch_job.job_id,
                    "uploads": FakeComfyState.uploads,
                    "view_requests": FakeComfyState.view_requests,
                    "asset_count": stats["asset_files"],
                },
                ensure_ascii=False,
                indent=2,
            )
        )


def _assert_patch_assets(db_path: Path, job_id: str) -> None:
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    rows = conn.execute(
        "SELECT role, mime_type, width, height, metadata_json FROM asset_files WHERE job_id = ?",
        (job_id,),
    ).fetchall()
    by_role = {row["role"]: row for row in rows}
    patch = by_role["concept_patch"]
    workflow = by_role["comfyui_workflow"]
    patch_meta = json.loads(patch["metadata_json"])
    workflow_meta = json.loads(workflow["metadata_json"])
    assert patch["mime_type"] == "image/png"
    assert patch["width"] == 1
    assert patch["height"] == 1
    assert patch_meta["provider"] == "comfyui"
    assert patch_meta["workflow_template_id"] == "wushen_patch_inpaint_sd_basic"
    assert patch_meta["source_upload"]["name"] == FakeComfyState.uploads[0]
    assert patch_meta["mask_upload"]["name"] == FakeComfyState.uploads[1]
    assert workflow_meta["workflow_template_id"] == "wushen_patch_inpaint_sd_basic"


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


def png_mask(width: int, height: int, *, ink: bool) -> bytes:
    pixel = b"\xff\xff\xff\xff" if ink else b"\x00\x00\x00\x00"
    raw = b"".join(b"\x00" + pixel * width for _ in range(height))
    compressed = zlib.compress(raw)
    return (
        b"\x89PNG\r\n\x1a\n"
        + _png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + _png_chunk(b"IDAT", compressed)
        + _png_chunk(b"IEND", b"")
    )


def _png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    checksum = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", checksum)


if __name__ == "__main__":
    sys.exit(main())
