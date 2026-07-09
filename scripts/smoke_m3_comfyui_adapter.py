#!/usr/bin/env python3
"""M3 smoke checks for the ComfyUI HTTP adapter boundary."""

from __future__ import annotations

import json
import sqlite3
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Dict

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest  # noqa: E402
from wushen_agent.providers.image import ComfyUIConfig, ComfyUIHTTPProvider  # noqa: E402
from wushen_agent.providers.llm import MockLLMProvider  # noqa: E402


PNG_BYTES = (
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01"
    b"\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\nIDATx\x9cc\x00\x01"
    b"\x00\x00\x05\x00\x01\r\n-\xb4\x00\x00\x00\x00IEND\xaeB`\x82"
)


class FakeComfyState:
    prompt_payload: Dict[str, Any] | None = None
    prompt_id = "prompt_fake_001"
    view_requested = False
    prompt_failures_remaining = 0
    view_failures_remaining = 0
    prompt_bad_request_remaining = 0
    prompt_attempts = 0
    view_attempts = 0

    @classmethod
    def reset(cls) -> None:
        cls.prompt_payload = None
        cls.prompt_id = "prompt_fake_001"
        cls.view_requested = False
        cls.prompt_failures_remaining = 0
        cls.view_failures_remaining = 0
        cls.prompt_bad_request_remaining = 0
        cls.prompt_attempts = 0
        cls.view_attempts = 0


class FakeComfyHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        if self.path != "/prompt":
            self.send_error(404)
            return
        FakeComfyState.prompt_attempts += 1
        if FakeComfyState.prompt_failures_remaining:
            FakeComfyState.prompt_failures_remaining -= 1
            self.send_error(503)
            return
        if FakeComfyState.prompt_bad_request_remaining:
            FakeComfyState.prompt_bad_request_remaining -= 1
            self.send_error(400)
            return
        length = int(self.headers.get("Content-Length", "0"))
        FakeComfyState.prompt_payload = json.loads(self.rfile.read(length).decode("utf-8"))
        self._send_json({"prompt_id": FakeComfyState.prompt_id, "number": 1})

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        if self.path == f"/history/{FakeComfyState.prompt_id}":
            self._send_json(
                {
                    FakeComfyState.prompt_id: {
                        "outputs": {
                            "9": {
                                "images": [
                                    {"filename": "concept.png", "subfolder": "", "type": "output"}
                                ]
                            }
                        }
                    }
                }
            )
            return
        if self.path.startswith("/view?"):
            FakeComfyState.view_attempts += 1
            if FakeComfyState.view_failures_remaining:
                FakeComfyState.view_failures_remaining -= 1
                self.send_error(502)
                return
            FakeComfyState.view_requested = True
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
    server = ThreadingHTTPServer(("127.0.0.1", 0), FakeComfyHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base_url = f"http://127.0.0.1:{server.server_port}"

    try:
        _run_adapter_case(base_url, expect_prompt_attempts=1, expect_view_attempts=1)
        FakeComfyState.reset()
        FakeComfyState.prompt_failures_remaining = 2
        FakeComfyState.view_failures_remaining = 1
        _run_adapter_case(base_url, expect_prompt_attempts=3, expect_view_attempts=2)
        FakeComfyState.reset()
        FakeComfyState.prompt_bad_request_remaining = 1
        _run_bad_workflow_case(base_url)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)
    return 0


def _run_adapter_case(base_url: str, *, expect_prompt_attempts: int, expect_view_attempts: int) -> None:
        with tempfile.TemporaryDirectory(prefix="wushen_m3_comfyui_") as tmp:
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
            request = CreateWeaponRequest(
                client_request_id="m3-comfyui-smoke",
                text="赤金龙纹长剑，3渲2国风神兵，逼真外观，仅作为虚构 Unity 游戏资产",
            )
            job = store.create_weapon(request, idempotency_key="m3-comfyui-smoke-key")
            assert job.status == "succeeded"
            assert FakeComfyState.prompt_payload is not None
            assert "prompt" in FakeComfyState.prompt_payload
            prompt_graph = FakeComfyState.prompt_payload["prompt"]
            assert prompt_graph["3"]["class_type"] == "KSampler"
            assert prompt_graph["5"]["inputs"]["width"] == 1280
            assert prompt_graph["5"]["inputs"]["height"] == 720
            assert prompt_graph["9"]["inputs"]["filename_prefix"].startswith(f"wushen/{job.weapon_id}/")
            assert "wushen_meta" not in prompt_graph
            assert FakeComfyState.view_requested
            assert FakeComfyState.prompt_attempts == expect_prompt_attempts
            assert FakeComfyState.view_attempts == expect_view_attempts

            db_path = Path(tmp) / "WushenForgeLibrary" / "library.db"
            _assert_http_provider_assets(db_path, job.job_id)
            findings, stats = validate(Path(tmp) / "WushenForgeLibrary", db_path)
            assert stats["blockers"] == 0, findings

            print(
                json.dumps(
                    {
                        "ok": True,
                        "provider": "comfyui",
                        "prompt_id": FakeComfyState.prompt_id,
                        "prompt_attempts": FakeComfyState.prompt_attempts,
                        "view_attempts": FakeComfyState.view_attempts,
                        "asset_count": stats["asset_files"],
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )


def _run_bad_workflow_case(base_url: str) -> None:
    with tempfile.TemporaryDirectory(prefix="wushen_m3_comfyui_bad_") as tmp:
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
        request = CreateWeaponRequest(
            client_request_id="m3-comfyui-bad-workflow-smoke",
            text="赤金龙纹长剑，3渲2国风神兵，逼真外观，仅作为虚构 Unity 游戏资产",
        )
        try:
            store.create_weapon(request, idempotency_key="m3-comfyui-bad-workflow-key")
        except Exception as exc:  # noqa: BLE001 - smoke checks provider boundary error mapping.
            assert "PROVIDER_BAD_OUTPUT" in getattr(exc, "code", "")
            assert FakeComfyState.prompt_attempts == 1
            return
        raise AssertionError("ComfyUI HTTP 400 did not fail as PROVIDER_BAD_OUTPUT")


def _assert_http_provider_assets(db_path: Path, job_id: str) -> None:
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    rows = conn.execute(
        "SELECT role, mime_type, width, height, metadata_json FROM asset_files WHERE job_id = ?",
        (job_id,),
    ).fetchall()
    by_role = {row["role"]: row for row in rows}
    concept = by_role["concept_image"]
    workflow = by_role["comfyui_workflow"]
    assert concept["mime_type"] == "image/png"
    assert concept["width"] == 1
    assert concept["height"] == 1
    assert json.loads(concept["metadata_json"])["provider"] == "comfyui"
    workflow_meta = json.loads(workflow["metadata_json"])
    assert workflow_meta["provider_task_id"] == FakeComfyState.prompt_id
    assert workflow_meta["workflow_template_id"] == "wushen_concept_sd_basic"
    assert workflow_meta["workflow_template_version"] == "0.1.0"
    assert workflow_meta["checkpoint_name"] == "v1-5-pruned-emaonly.safetensors"
    sampler = workflow_meta["generation_provenance"]["sampler"]
    assert sampler["sampler_name"] == "euler"
    assert sampler["scheduler"] == "normal"
    assert sampler["steps"] == 24
    assert sampler["cfg"] == 7
    assert sampler["denoise"] == 1


if __name__ == "__main__":
    sys.exit(main())
