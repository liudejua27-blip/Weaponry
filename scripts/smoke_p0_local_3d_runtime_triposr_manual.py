#!/usr/bin/env python3
"""Manual smoke for the TripoSR backend of the local 3D runtime wrapper.

This script is intentionally not part of m5:gate. It requires a local TripoSR
checkout, model dependencies, and a Python environment that can run inference.
"""

from __future__ import annotations

import json
import mimetypes
import os
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, Optional


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))
sys.path.insert(0, str(ROOT / "scripts"))

from smoke_p0_local_3d_runtime_sf3d_manual import _reference_png  # noqa: E402
from wushen_agent.providers.three_d import LocalHTTPThreeDConfig, LocalHTTPThreeDProvider  # noqa: E402


def main() -> int:
    triposr_repo = _required_path("WUSHEN_TRIPOSR_REPO")
    run_py = triposr_repo / "run.py"
    if not run_py.exists():
        raise RuntimeError(f"WUSHEN_TRIPOSR_REPO does not contain run.py: {run_py}")
    triposr_python = os.environ.get("WUSHEN_TRIPOSR_PYTHON", sys.executable)
    max_wait_seconds = float(os.environ.get("WUSHEN_TRIPOSR_SMOKE_MAX_WAIT_SECONDS", "900"))
    runtime: Optional[subprocess.Popen[str]] = None
    with tempfile.TemporaryDirectory(prefix="wushen_triposr_manual_") as tmp:
        tmp_path = Path(tmp)
        input_path = _input_image(tmp_path)
        output_dir = Path(os.environ.get("WUSHEN_TRIPOSR_SMOKE_OUTPUT_DIR", str(tmp_path / "output"))).expanduser().resolve()
        output_dir.mkdir(parents=True, exist_ok=True)
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        try:
            runtime = _start_runtime(
                port=port,
                tmp=tmp_path,
                triposr_repo=triposr_repo,
                triposr_python=triposr_python,
                max_wait_seconds=max_wait_seconds,
            )
            _wait_for_health(base_url, runtime)
            source_bytes = input_path.read_bytes()
            mime_type = mimetypes.guess_type(str(input_path))[0] or "image/png"
            provider = LocalHTTPThreeDProvider(
                LocalHTTPThreeDConfig(
                    base_url=base_url,
                    provider_id="local_http_3d_triposr_manual",
                    timeout_seconds=30,
                    poll_interval_seconds=float(os.environ.get("WUSHEN_TRIPOSR_SMOKE_POLL_INTERVAL_SECONDS", "2")),
                    max_wait_seconds=max_wait_seconds,
                    retry_attempts=1,
                )
            )
            started_at = time.time()
            result = provider.generate_rough_model(
                weapon_id="manual_triposr_weapon",
                model_id=f"manual_triposr_{int(started_at)}",
                source_image_asset_id="manual_source_png",
                source_image_bytes=source_bytes,
                source_image_mime_type=mime_type,
                source_image_logical_path=str(input_path),
                target_format="glb",
                style="stylized_toon_weapon",
                orientation_policy={"forward_axis": "+Z", "long_axis": "+Y", "pivot": "grip_center"},
                scale_policy="normalized_game_asset_scale",
            )
            raw_path = output_dir / "manual_triposr_raw.glb"
            normalized_path = output_dir / "manual_triposr_normalized.glb"
            optimized_path = output_dir / "manual_triposr_optimized.glb"
            material_path = output_dir / "manual_triposr_unity_material.json"
            raw_path.write_bytes(result.raw_glb_bytes)
            normalized_path.write_bytes(result.normalized_glb_bytes)
            optimized_path.write_bytes(result.optimized_glb_bytes)
            material_path.write_text(json.dumps(result.unity_material_json, ensure_ascii=False, indent=2), encoding="utf-8")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "backend": "triposr-cli",
                        "triposr_repo": str(triposr_repo),
                        "triposr_python": triposr_python,
                        "input_image": str(input_path),
                        "output_dir": str(output_dir),
                        "provider_task_id": result.provider_task_id,
                        "optimized_glb_bytes": len(result.optimized_glb_bytes),
                        "runtime_seconds": round(time.time() - started_at, 3),
                        "metrics": result.metrics,
                        "metadata": _redacted_metadata(result.metadata),
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 0
        finally:
            if runtime is not None:
                _terminate_process(runtime)


def _start_runtime(
    *,
    port: int,
    tmp: Path,
    triposr_repo: Path,
    triposr_python: str,
    max_wait_seconds: float,
) -> subprocess.Popen[str]:
    command = [
        sys.executable,
        str(ROOT / "scripts" / "wushen_local_3d_runtime.py"),
        "--host",
        "127.0.0.1",
        "--port",
        str(port),
        "--backend",
        "triposr-cli",
        "--work-dir",
        str(tmp / "runtime-work"),
        "--triposr-repo",
        str(triposr_repo),
        "--triposr-python",
        triposr_python,
        "--task-timeout-seconds",
        str(max_wait_seconds),
    ]
    for env_name, flag in (
        ("WUSHEN_TRIPOSR_DEVICE", "--triposr-device"),
        ("WUSHEN_TRIPOSR_PRETRAINED_MODEL", "--triposr-pretrained-model"),
        ("WUSHEN_TRIPOSR_CHUNK_SIZE", "--triposr-chunk-size"),
        ("WUSHEN_TRIPOSR_MC_RESOLUTION", "--triposr-mc-resolution"),
        ("WUSHEN_TRIPOSR_TEXTURE_RESOLUTION", "--texture-resolution"),
    ):
        value = os.environ.get(env_name)
        if value:
            command.extend([flag, value])
    if os.environ.get("WUSHEN_TRIPOSR_BAKE_TEXTURE", "0") == "1":
        command.append("--triposr-bake-texture")
    if os.environ.get("WUSHEN_TRIPOSR_NO_REMOVE_BG", "0") == "1":
        command.append("--triposr-no-remove-bg")
    if os.environ.get("WUSHEN_TRIPOSR_SMOKE_KEEP_WORK_DIR", "0") == "1":
        command.append("--keep-work-dir")
    return subprocess.Popen(
        command,
        cwd=str(ROOT),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )


def _wait_for_health(base_url: str, process: subprocess.Popen[str]) -> None:
    deadline = time.time() + 10
    last_error: Optional[Exception] = None
    while time.time() < deadline:
        if process.poll() is not None:
            stderr = process.stderr.read() if process.stderr else ""
            raise RuntimeError(f"local TripoSR runtime exited before health check at {base_url}: {stderr}")
        try:
            health = _http_json(f"{base_url}/health")
            if health.get("status") == "ok" and health.get("backend") == "triposr-cli":
                return
            raise RuntimeError(f"runtime health mismatch: {health}")
        except (urllib.error.URLError, TimeoutError, RuntimeError) as exc:
            last_error = exc
            time.sleep(0.1)
    raise RuntimeError(f"local TripoSR runtime did not become healthy at {base_url}: {last_error}")


def _input_image(tmp: Path) -> Path:
    configured = os.environ.get("WUSHEN_TRIPOSR_INPUT_IMAGE")
    if configured:
        path = Path(configured).expanduser().resolve()
        if not path.exists():
            raise RuntimeError(f"WUSHEN_TRIPOSR_INPUT_IMAGE does not exist: {path}")
        return path
    path = tmp / "wushen_triposr_reference.png"
    path.write_bytes(_reference_png())
    return path


def _required_path(name: str) -> Path:
    value = os.environ.get(name)
    if not value:
        raise RuntimeError(f"{name} is required for the manual TripoSR smoke.")
    return Path(value).expanduser().resolve()


def _http_json(url: str) -> Dict[str, Any]:
    with urllib.request.urlopen(url, timeout=2) as response:
        payload = json.loads(response.read().decode("utf-8"))
    if not isinstance(payload, dict):
        raise RuntimeError(f"HTTP response was not a JSON object: {url}")
    return payload


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _terminate_process(process: subprocess.Popen[str]) -> None:
    try:
        if process.poll() is not None:
            return
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGTERM)
        else:
            process.terminate()
        process.wait(timeout=5)
    except Exception:
        try:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGKILL)
            else:
                process.kill()
        except Exception:
            return


def _redacted_metadata(metadata: Dict[str, Any]) -> Dict[str, Any]:
    redacted = dict(metadata)
    command = redacted.get("triposr_command")
    if isinstance(command, list):
        redacted["triposr_command"] = [str(item) for item in command[:2]] + ["..."]
    return redacted


if __name__ == "__main__":
    sys.exit(main())
