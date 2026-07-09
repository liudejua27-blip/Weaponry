#!/usr/bin/env python3
"""Local HTTP image-to-3D runtime wrapper for Wushen Forge.

This service implements the Wushen local HTTP 3D provider protocol and keeps
heavy model dependencies outside the desktop Agent process. The `mock` backend
is deterministic for local verification. The `sf3d-cli` and `triposr-cli`
backends call local checkouts through their documented `run.py` CLIs.
"""

from __future__ import annotations

import argparse
import base64
import json
import mimetypes
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Dict, Optional


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.providers.three_d import mock_glb_payload, mock_unity_material  # noqa: E402


TERMINAL_STATES = {"succeeded", "failed", "cancelled"}


@dataclass
class RuntimeConfig:
    backend: str
    work_dir: Path
    sf3d_repo: Optional[Path]
    sf3d_python: str
    triposr_repo: Optional[Path]
    triposr_python: str
    triposr_device: Optional[str]
    triposr_pretrained_model: Optional[str]
    triposr_chunk_size: Optional[int]
    triposr_mc_resolution: Optional[int]
    triposr_bake_texture: bool
    triposr_no_remove_bg: bool
    texture_resolution: Optional[int]
    remesh_option: Optional[str]
    task_timeout_seconds: float
    mock_delay_seconds: float
    keep_work_dir: bool


@dataclass
class RuntimeTask:
    task_id: str
    request: Dict[str, Any]
    task_dir: Path
    status: str = "submitted"
    progress: float = 0.0
    metadata: Dict[str, Any] = field(default_factory=dict)
    error: Optional[Dict[str, str]] = None
    raw_glb_path: Optional[Path] = None
    normalized_glb_path: Optional[Path] = None
    optimized_glb_path: Optional[Path] = None
    metrics: Dict[str, Any] = field(default_factory=dict)
    process: Optional[subprocess.Popen] = None
    created_at: float = field(default_factory=time.time)
    updated_at: float = field(default_factory=time.time)


class RuntimeState:
    def __init__(self, config: RuntimeConfig) -> None:
        self.config = config
        self.lock = threading.RLock()
        self.tasks: Dict[str, RuntimeTask] = {}
        self.config.work_dir.mkdir(parents=True, exist_ok=True)

    def create_task(self, payload: Dict[str, Any]) -> RuntimeTask:
        _validate_request(payload)
        task_id = f"w3d_{uuid.uuid4().hex[:12]}"
        task_dir = self.config.work_dir / task_id
        task_dir.mkdir(parents=True, exist_ok=True)
        task = RuntimeTask(
            task_id=task_id,
            request=payload,
            task_dir=task_dir,
            status="submitted",
            metadata={
                "backend": self.config.backend,
                "runtime": "wushen_local_3d_runtime",
                "non_manufacturing_asset": True,
            },
        )
        with self.lock:
            self.tasks[task_id] = task
        thread = threading.Thread(target=self._run_task, args=(task,), daemon=True)
        thread.start()
        return task

    def task(self, task_id: str) -> Optional[RuntimeTask]:
        with self.lock:
            return self.tasks.get(task_id)

    def cancel(self, task_id: str) -> Optional[RuntimeTask]:
        task = self.task(task_id)
        if task is None:
            return None
        with self.lock:
            if task.status in TERMINAL_STATES:
                return task
            task.status = "cancelled"
            task.progress = 0.0
            task.updated_at = time.time()
            process = task.process
        if process and process.poll() is None:
            _terminate_process(process)
        return task

    def _run_task(self, task: RuntimeTask) -> None:
        try:
            with self.lock:
                if task.status == "cancelled":
                    return
                task.status = "polling"
                task.progress = 0.1
                task.updated_at = time.time()
            if self.config.backend == "mock":
                self._run_mock_task(task)
            elif self.config.backend == "sf3d-cli":
                self._run_sf3d_cli_task(task)
            elif self.config.backend == "triposr-cli":
                self._run_triposr_cli_task(task)
            else:
                raise RuntimeError(f"Unsupported backend: {self.config.backend}")
        except Exception as exc:  # noqa: BLE001 - runtime must report backend failures as task state.
            with self.lock:
                if task.status != "cancelled":
                    task.status = "failed"
                    task.error = {"code": "RUNTIME_BACKEND_FAILED", "message": str(exc)}
                    task.updated_at = time.time()
        finally:
            if not self.config.keep_work_dir and task.status in {"failed", "cancelled"}:
                shutil.rmtree(task.task_dir, ignore_errors=True)

    def _run_mock_task(self, task: RuntimeTask) -> None:
        if self.config.mock_delay_seconds > 0:
            deadline = time.time() + self.config.mock_delay_seconds
            while time.time() < deadline:
                with self.lock:
                    if task.status == "cancelled":
                        return
                    task.progress = min(0.85, task.progress + 0.1)
                    task.updated_at = time.time()
                time.sleep(min(0.1, max(0.0, deadline - time.time())))
        weapon_id = str(task.request["weapon_id"])
        model_id = str(task.request["model_id"])
        for stage in ("raw", "normalized", "optimized"):
            path = task.task_dir / f"{stage}.glb"
            path.write_bytes(mock_glb_payload(weapon_id, model_id=model_id, stage=stage))
        with self.lock:
            if task.status == "cancelled":
                return
            task.raw_glb_path = task.task_dir / "raw.glb"
            task.normalized_glb_path = task.task_dir / "normalized.glb"
            task.optimized_glb_path = task.task_dir / "optimized.glb"
            task.metrics = {
                "triangle_count": 36,
                "mesh_count": 1,
                "material_count": 1,
                "backend": "mock",
            }
            task.status = "succeeded"
            task.progress = 1.0
            task.updated_at = time.time()

    def _run_sf3d_cli_task(self, task: RuntimeTask) -> None:
        if self.config.sf3d_repo is None:
            raise RuntimeError("sf3d-cli backend requires --sf3d-repo or WUSHEN_SF3D_REPO.")
        repo = self.config.sf3d_repo.expanduser().resolve()
        run_py = repo / "run.py"
        if not run_py.exists():
            raise RuntimeError(f"Stable Fast 3D run.py was not found: {run_py}")
        input_path = _write_source_image(task.request, task.task_dir)
        output_dir = task.task_dir / "sf3d_output"
        output_dir.mkdir(parents=True, exist_ok=True)
        command = [self.config.sf3d_python, str(run_py), str(input_path), "--output-dir", str(output_dir)]
        if self.config.texture_resolution:
            command.extend(["--texture-resolution", str(self.config.texture_resolution)])
        if self.config.remesh_option:
            command.extend(["--remesh_option", self.config.remesh_option])
        started_at = time.time()
        with self.lock:
            if task.status == "cancelled":
                return
            task.progress = 0.25
            task.updated_at = time.time()
            task.process = subprocess.Popen(
                command,
                cwd=str(repo),
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                start_new_session=True,
            )
            process = task.process
        assert process is not None
        try:
            output, _stderr = process.communicate(timeout=self.config.task_timeout_seconds)
        except subprocess.TimeoutExpired as exc:
            _terminate_process(process)
            raise RuntimeError(f"Stable Fast 3D task exceeded timeout: {self.config.task_timeout_seconds}s") from exc
        output_lines = output.splitlines() if output else []
        with self.lock:
            if task.status == "cancelled":
                return
            task.progress = 0.9
            task.updated_at = time.time()
        if process.returncode != 0:
            tail = "\n".join(output_lines[-20:])
            raise RuntimeError(f"Stable Fast 3D failed with exit code {process.returncode}: {tail}")
        glb_path = _find_first_glb(output_dir)
        if glb_path is None:
            raise RuntimeError(f"Stable Fast 3D did not produce a GLB under {output_dir}")
        raw_path, normalized_path, optimized_path = _copy_glb_variants(glb_path, task.task_dir)
        with self.lock:
            if task.status == "cancelled":
                return
            task.raw_glb_path = raw_path
            task.normalized_glb_path = normalized_path
            task.optimized_glb_path = optimized_path
            task.metrics = {
                "backend": "sf3d-cli",
                "optimized_glb_bytes": optimized_path.stat().st_size,
                "runtime_seconds": round(time.time() - started_at, 3),
                "provider_reported_metrics": False,
            }
            task.metadata.update({"sf3d_repo": str(repo), "sf3d_command": command})
            task.status = "succeeded"
            task.progress = 1.0
            task.updated_at = time.time()

    def _run_triposr_cli_task(self, task: RuntimeTask) -> None:
        if self.config.triposr_repo is None:
            raise RuntimeError("triposr-cli backend requires --triposr-repo or WUSHEN_TRIPOSR_REPO.")
        repo = self.config.triposr_repo.expanduser().resolve()
        run_py = repo / "run.py"
        if not run_py.exists():
            raise RuntimeError(f"TripoSR run.py was not found: {run_py}")
        input_path = _write_source_image(task.request, task.task_dir)
        output_dir = task.task_dir / "triposr_output"
        output_dir.mkdir(parents=True, exist_ok=True)
        command = [
            self.config.triposr_python,
            str(run_py),
            str(input_path),
            "--output-dir",
            str(output_dir),
            "--model-save-format",
            "glb",
        ]
        if self.config.triposr_device:
            command.extend(["--device", self.config.triposr_device])
        if self.config.triposr_pretrained_model:
            command.extend(["--pretrained-model-name-or-path", self.config.triposr_pretrained_model])
        if self.config.triposr_chunk_size is not None:
            command.extend(["--chunk-size", str(self.config.triposr_chunk_size)])
        if self.config.triposr_mc_resolution is not None:
            command.extend(["--mc-resolution", str(self.config.triposr_mc_resolution)])
        if self.config.triposr_no_remove_bg:
            command.append("--no-remove-bg")
        if self.config.triposr_bake_texture:
            command.append("--bake-texture")
            if self.config.texture_resolution:
                command.extend(["--texture-resolution", str(self.config.texture_resolution)])
        started_at = time.time()
        with self.lock:
            if task.status == "cancelled":
                return
            task.progress = 0.25
            task.updated_at = time.time()
            task.process = subprocess.Popen(
                command,
                cwd=str(repo),
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                start_new_session=True,
            )
            process = task.process
        assert process is not None
        try:
            output, _stderr = process.communicate(timeout=self.config.task_timeout_seconds)
        except subprocess.TimeoutExpired as exc:
            _terminate_process(process)
            raise RuntimeError(f"TripoSR task exceeded timeout: {self.config.task_timeout_seconds}s") from exc
        output_lines = output.splitlines() if output else []
        with self.lock:
            if task.status == "cancelled":
                return
            task.progress = 0.9
            task.updated_at = time.time()
        if process.returncode != 0:
            tail = "\n".join(output_lines[-20:])
            raise RuntimeError(f"TripoSR failed with exit code {process.returncode}: {tail}")
        glb_path = _find_first_glb(output_dir)
        if glb_path is None:
            raise RuntimeError(f"TripoSR did not produce a GLB under {output_dir}")
        raw_path, normalized_path, optimized_path = _copy_glb_variants(glb_path, task.task_dir)
        with self.lock:
            if task.status == "cancelled":
                return
            task.raw_glb_path = raw_path
            task.normalized_glb_path = normalized_path
            task.optimized_glb_path = optimized_path
            task.metrics = {
                "backend": "triposr-cli",
                "optimized_glb_bytes": optimized_path.stat().st_size,
                "runtime_seconds": round(time.time() - started_at, 3),
                "bake_texture": self.config.triposr_bake_texture,
                "provider_reported_metrics": False,
            }
            task.metadata.update({"triposr_repo": str(repo), "triposr_command": command})
            task.status = "succeeded"
            task.progress = 1.0
            task.updated_at = time.time()


class RuntimeHandler(BaseHTTPRequestHandler):
    state: RuntimeState

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        if self.path == "/health":
            self._send_json({"status": "ok", "backend": self.state.config.backend})
            return
        task_id, suffix = _parse_task_path(self.path)
        if not task_id:
            self.send_error(404)
            return
        task = self.state.task(task_id)
        if task is None:
            self.send_error(404)
            return
        if suffix == "":
            self._send_json(_task_status_payload(task))
            return
        if suffix == "result":
            if task.status != "succeeded":
                self.send_error(409, f"Task is not ready: {task.status}")
                return
            self._send_json(_task_result_payload(task))
            return
        self.send_error(404)

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        if self.path == "/v1/rough-models":
            try:
                payload = self._read_json()
                task = self.state.create_task(payload)
            except Exception as exc:  # noqa: BLE001 - protocol boundary should return structured 400 errors.
                self._send_error_json(400, "INVALID_REQUEST", str(exc))
                return
            self._send_json(
                {
                    "provider_task_id": task.task_id,
                    "status": task.status,
                    "metadata": task.metadata,
                }
            )
            return
        task_id, suffix = _parse_task_path(self.path)
        if task_id and suffix == "cancel":
            task = self.state.cancel(task_id)
            if task is None:
                self.send_error(404)
                return
            self._send_json({"provider_task_id": task.task_id, "status": task.status, "metadata": task.metadata})
            return
        self.send_error(404)

    def log_message(self, _format: str, *_args: Any) -> None:
        return

    def _read_json(self) -> Dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        try:
            payload = json.loads(self.rfile.read(length).decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise RuntimeError("Request body must be JSON.") from exc
        if not isinstance(payload, dict):
            raise RuntimeError("Request body must be a JSON object.")
        return payload

    def _send_json(self, payload: Dict[str, Any]) -> None:
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _send_error_json(self, status_code: int, code: str, message: str) -> None:
        data = json.dumps({"error": {"code": code, "message": message}}, ensure_ascii=False).encode("utf-8")
        self.send_response(status_code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Run the Wushen local HTTP 3D runtime wrapper.")
    parser.add_argument("--host", default=os.environ.get("WUSHEN_LOCAL_3D_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.environ.get("WUSHEN_LOCAL_3D_PORT", "8787")))
    parser.add_argument("--backend", choices=["mock", "sf3d-cli", "triposr-cli"], default=os.environ.get("WUSHEN_LOCAL_3D_BACKEND", "mock"))
    parser.add_argument("--work-dir", default=os.environ.get("WUSHEN_LOCAL_3D_WORK_DIR"))
    parser.add_argument("--sf3d-repo", default=os.environ.get("WUSHEN_SF3D_REPO"))
    parser.add_argument("--sf3d-python", default=os.environ.get("WUSHEN_SF3D_PYTHON", sys.executable))
    parser.add_argument("--triposr-repo", default=os.environ.get("WUSHEN_TRIPOSR_REPO"))
    parser.add_argument("--triposr-python", default=os.environ.get("WUSHEN_TRIPOSR_PYTHON", sys.executable))
    parser.add_argument("--triposr-device", default=os.environ.get("WUSHEN_TRIPOSR_DEVICE"))
    parser.add_argument("--triposr-pretrained-model", default=os.environ.get("WUSHEN_TRIPOSR_PRETRAINED_MODEL"))
    parser.add_argument("--triposr-chunk-size", type=int, default=_optional_int(os.environ.get("WUSHEN_TRIPOSR_CHUNK_SIZE")))
    parser.add_argument("--triposr-mc-resolution", type=int, default=_optional_int(os.environ.get("WUSHEN_TRIPOSR_MC_RESOLUTION")))
    parser.add_argument("--triposr-bake-texture", action="store_true", default=_env_bool("WUSHEN_TRIPOSR_BAKE_TEXTURE"))
    parser.add_argument("--triposr-no-remove-bg", action="store_true", default=_env_bool("WUSHEN_TRIPOSR_NO_REMOVE_BG"))
    parser.add_argument(
        "--texture-resolution",
        type=int,
        default=_optional_int(os.environ.get("WUSHEN_SF3D_TEXTURE_RESOLUTION") or os.environ.get("WUSHEN_TRIPOSR_TEXTURE_RESOLUTION")),
    )
    parser.add_argument("--remesh-option", default=os.environ.get("WUSHEN_SF3D_REMESH_OPTION"))
    parser.add_argument("--task-timeout-seconds", type=float, default=float(os.environ.get("WUSHEN_LOCAL_3D_TASK_TIMEOUT_SECONDS", "900")))
    parser.add_argument("--mock-delay-seconds", type=float, default=float(os.environ.get("WUSHEN_LOCAL_3D_MOCK_DELAY_SECONDS", "0.05")))
    parser.add_argument("--keep-work-dir", action="store_true", default=os.environ.get("WUSHEN_LOCAL_3D_KEEP_WORK_DIR", "0") == "1")
    args = parser.parse_args(argv)

    work_dir = Path(args.work_dir) if args.work_dir else Path(tempfile.gettempdir()) / "wushen-local-3d-runtime"
    config = RuntimeConfig(
        backend=args.backend,
        work_dir=work_dir,
        sf3d_repo=Path(args.sf3d_repo) if args.sf3d_repo else None,
        sf3d_python=args.sf3d_python,
        triposr_repo=Path(args.triposr_repo) if args.triposr_repo else None,
        triposr_python=args.triposr_python,
        triposr_device=args.triposr_device,
        triposr_pretrained_model=args.triposr_pretrained_model,
        triposr_chunk_size=args.triposr_chunk_size,
        triposr_mc_resolution=args.triposr_mc_resolution,
        triposr_bake_texture=args.triposr_bake_texture,
        triposr_no_remove_bg=args.triposr_no_remove_bg,
        texture_resolution=args.texture_resolution,
        remesh_option=args.remesh_option,
        task_timeout_seconds=args.task_timeout_seconds,
        mock_delay_seconds=args.mock_delay_seconds,
        keep_work_dir=args.keep_work_dir,
    )
    RuntimeHandler.state = RuntimeState(config)
    server = ThreadingHTTPServer((args.host, args.port), RuntimeHandler)
    print(
        json.dumps(
            {
                "status": "listening",
                "base_url": f"http://{args.host}:{server.server_port}",
                "backend": args.backend,
                "work_dir": str(work_dir),
            },
            ensure_ascii=False,
        ),
        flush=True,
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        return 130
    finally:
        server.server_close()
    return 0


def _validate_request(payload: Dict[str, Any]) -> None:
    if payload.get("schema_version") != "WushenThreeDProviderRequest@1":
        raise RuntimeError("schema_version must be WushenThreeDProviderRequest@1.")
    if payload.get("target_format") != "glb":
        raise RuntimeError("target_format must be glb.")
    output_contract = payload.get("output_contract")
    if not isinstance(output_contract, dict) or output_contract.get("non_manufacturing_asset") is not True:
        raise RuntimeError("output_contract.non_manufacturing_asset must be true.")
    source_image = payload.get("source_image")
    if not isinstance(source_image, dict):
        raise RuntimeError("source_image is required.")
    data_base64 = source_image.get("data_base64")
    if not isinstance(data_base64, str) or not data_base64:
        raise RuntimeError("source_image.data_base64 is required.")
    base64.b64decode(data_base64, validate=True)
    for key in ("weapon_id", "model_id", "source_image_asset_id"):
        if not payload.get(key):
            raise RuntimeError(f"{key} is required.")


def _write_source_image(payload: Dict[str, Any], task_dir: Path) -> Path:
    source_image = payload["source_image"]
    mime_type = str(source_image.get("mime_type") or "image/png")
    ext = mimetypes.guess_extension(mime_type) or ".png"
    path = task_dir / f"source{ext}"
    path.write_bytes(base64.b64decode(str(source_image["data_base64"]), validate=True))
    return path


def _find_first_glb(output_dir: Path) -> Optional[Path]:
    candidates = sorted(output_dir.rglob("*.glb"), key=lambda path: (path.stat().st_mtime, str(path)), reverse=True)
    return candidates[0] if candidates else None


def _copy_glb_variants(glb_path: Path, task_dir: Path) -> tuple[Path, Path, Path]:
    payload = glb_path.read_bytes()
    _validate_glb(payload)
    raw_path = task_dir / "raw.glb"
    normalized_path = task_dir / "normalized.glb"
    optimized_path = task_dir / "optimized.glb"
    raw_path.write_bytes(payload)
    normalized_path.write_bytes(payload)
    optimized_path.write_bytes(payload)
    return raw_path, normalized_path, optimized_path


def _validate_glb(payload: bytes) -> None:
    if len(payload) < 12 or payload[:4] != b"glTF":
        raise RuntimeError("Output is not a GLB payload.")
    version = int.from_bytes(payload[4:8], "little")
    declared_length = int.from_bytes(payload[8:12], "little")
    if version != 2 or declared_length != len(payload):
        raise RuntimeError("Output GLB header is invalid.")


def _parse_task_path(path: str) -> tuple[Optional[str], str]:
    parts = path.strip("/").split("/")
    if len(parts) < 3 or parts[0] != "v1" or parts[1] != "rough-models":
        return None, ""
    task_id = parts[2]
    suffix = "/".join(parts[3:])
    return task_id, suffix


def _task_status_payload(task: RuntimeTask) -> Dict[str, Any]:
    return {
        "provider_task_id": task.task_id,
        "status": task.status,
        "progress": task.progress,
        "metadata": task.metadata,
        "error": task.error,
        "created_at": task.created_at,
        "updated_at": task.updated_at,
    }


def _task_result_payload(task: RuntimeTask) -> Dict[str, Any]:
    assert task.raw_glb_path and task.normalized_glb_path and task.optimized_glb_path
    weapon_id = str(task.request["weapon_id"])
    return {
        "provider_task_id": task.task_id,
        "raw_glb_base64": base64.b64encode(task.raw_glb_path.read_bytes()).decode("ascii"),
        "normalized_glb_base64": base64.b64encode(task.normalized_glb_path.read_bytes()).decode("ascii"),
        "optimized_glb_base64": base64.b64encode(task.optimized_glb_path.read_bytes()).decode("ascii"),
        "unity_material_json": mock_unity_material(weapon_id),
        "metrics": task.metrics,
        "metadata": task.metadata,
    }


def _terminate_process(process: subprocess.Popen) -> None:
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


def _optional_int(value: Optional[str]) -> Optional[int]:
    if value in {None, ""}:
        return None
    return int(value)


def _env_bool(name: str) -> bool:
    return os.environ.get(name, "0").strip().lower() in {"1", "true", "yes", "on"}


if __name__ == "__main__":
    raise SystemExit(main())
