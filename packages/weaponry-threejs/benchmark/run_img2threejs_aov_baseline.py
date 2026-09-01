#!/usr/bin/env python3
"""Capture a real browser 8x7 AOV baseline for the pinned fixture.

The pinned generator output, adapter inputs, copied capture modules, Vite
application, static server, and PNG bytes all remain temporary. Only the
closed manifest/receipt is persisted. This benchmark does not run Cargo,
Runtime, Store, or CAS and it never interprets a rendered image as quality
evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
from typing import Any

from run_img2threejs_baseline import (
    BenchmarkBlocked,
    EXPECTED_TREE,
    REVISION,
    extract_pinned_source,
    parse_json_output,
    run_checked,
    sha256_bytes,
    sha256_file,
    verify_pinned_source,
)
from run_img2threejs_browser_baseline import load_json, validate_closed_inputs


PACKAGE_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = PACKAGE_ROOT.parents[1]
BENCHMARK_ROOT = PACKAGE_ROOT / "benchmark"
DEFAULT_SPEC = BENCHMARK_ROOT / "dragonfang-like-objects-sculpt-spec.json"
DEFAULT_CONTRACT = BENCHMARK_ROOT / "upstream-render-normalization.contract.json"
DEFAULT_BASELINE_RECEIPT = BENCHMARK_ROOT / "img2threejs-baseline.receipt.json"
DEFAULT_SCENE_TEMPLATE = BENCHMARK_ROOT / "upstream-browser-entry.template.ts"
DEFAULT_AOV_TEMPLATE = BENCHMARK_ROOT / "upstream-aov-entry.template.ts"
DEFAULT_ADAPTER = BENCHMARK_ROOT / "img2threejs-compiled-scene-adapter.ts"
DEFAULT_RECEIPT = BENCHMARK_ROOT / "img2threejs-browser-aov.receipt.json"
DEFAULT_NODE_MODULES = REPOSITORY_ROOT / "node_modules"
FIXED_RIG_FINGERPRINT = "3fa0202473e3352b"
REQUIRED_AOV_IDS = ["beauty", "silhouette", "depth", "normal", "part-id", "material-id", "wireframe"]
EXPECTED_VIEW_IDS = ["FRONT", "BACK", "TOP", "BOTTOM", "LEFT", "RIGHT", "REAR_THREE_QUARTER", "FPS_HOLD"]
CAPTURE_SOURCE_FILES = [
    "knife-browser-capture.ts",
    "knife-view-evaluation.ts",
    "knife-scene-compiler.ts",
    "knife-scene-program.ts",
    "knife-assembly-compiler.ts",
]
SHA256_PATTERN = "0123456789abcdef"

NODE_ADAPTER_PROBE = r"""
const entry = await import(process.env.WPN_AOV_ENTRY_URL);
const binding = entry.createUpstreamCaptureBinding();
if (!binding || !binding.scene || !binding.compiled || !binding.rig) throw new Error('capture binding was not created');
if (binding.rig.deterministic_fingerprint !== '3fa0202473e3352b') throw new Error('fixed rig fingerprint mismatch');
if (binding.compiled.group.parent !== binding.scene) throw new Error('compiled group is not attached to capture scene');
if (binding.compiled.parts.length !== 7 || binding.compiled.triangle_count !== 1049) throw new Error('adapter structural counts drifted');
const parts = binding.compiled.parts.map((part) => ({
  part_id: part.part_id,
  material_zone_id: part.material_zone_id,
  surface_role: part.surface_role,
  assembly_primitive: part.assembly_primitive ?? null,
  triangles: (part.geometry.getIndex()?.count ?? part.geometry.getAttribute('position').count) / 3,
  parent_bound: part.mesh.parent === binding.compiled.group,
}));
if (parts.some((part) => !part.parent_bound || !Number.isInteger(part.triangles) || part.triangles <= 0)) throw new Error('adapter part binding is not closed');
console.log(JSON.stringify({
  schema_version: 'WeaponryThreeJsUpstreamCompiledScene@1',
  scene_name: binding.scene.name,
  group_name: binding.compiled.group.name,
  deterministic_fingerprint: binding.compiled.deterministic_fingerprint,
  mesh_count: binding.compiled.parts.length,
  triangle_count: binding.compiled.triangle_count,
  assembly_part_count: binding.compiled.assembly_parts.length,
  assembly_status: binding.compiled.assembly_status,
  part_ids: binding.compiled.parts.map((part) => part.part_id),
  material_zone_ids: [...new Set(binding.compiled.parts.map((part) => part.material_zone_id))].sort(),
  parts,
}));
"""


class BrowserBlocked(BenchmarkBlocked):
    """A precise browser/WebGL blocker, distinct from a quality failure."""


def discover_playwright_cli() -> Path:
    """Resolve the required local wrapper without persisting a machine path."""
    configured = os.environ.get("WPN_PLAYWRIGHT_CLI") or os.environ.get("PLAYWRIGHT_CLI")
    if configured:
        return Path(configured).expanduser()
    candidates = (
        Path.home() / ".codex/skills/playwright/scripts/playwright_cli.sh",
        Path.home() / ".agents/skills/playwright/scripts/playwright_cli.sh",
    )
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    return Path("playwright_cli.sh")


def run_playwright(
    playwright_cli: Path,
    session: str,
    *arguments: str,
    cwd: Path,
    label: str,
) -> subprocess.CompletedProcess[str]:
    if not playwright_cli.is_file():
        raise BrowserBlocked(f"Playwright wrapper is unavailable: {playwright_cli.name}")
    environment = os.environ.copy()
    environment["PLAYWRIGHT_CLI_SESSION"] = session
    result = subprocess.run(
        [str(playwright_cli), "--session", session, "--json", *arguments],
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
        env=environment,
    )
    if result.returncode != 0:
        detail = (result.stderr.strip() or result.stdout.strip() or "no output").replace(str(cwd), "<temporary-app>")
        raise BrowserBlocked(f"{label} failed with exit {result.returncode}: {detail[-1200:]}")
    return result


def parse_playwright_value(output: str, label: str) -> Any:
    text = output.strip()
    candidates = [text]
    for line in text.splitlines():
        candidates.append(line.strip())
    decoder = json.JSONDecoder()
    for candidate in candidates:
        if not candidate:
            continue
        try:
            value = json.loads(candidate)
        except json.JSONDecodeError:
            value = None
        if value is not None:
            found = find_result_object(value)
            if found is not None:
                return found
        for offset, character in enumerate(candidate):
            if character not in "[{":
                continue
            try:
                value, _end = decoder.raw_decode(candidate[offset:])
            except json.JSONDecodeError:
                continue
            found = find_result_object(value)
            if found is not None:
                return found
    raise BrowserBlocked(f"{label} did not return a JSON value")


def find_result_object(value: Any) -> Any | None:
    if isinstance(value, dict):
        if "manifest" in value or "error" in value or "status" in value and "receipt" in value:
            return value
        for key in ("result", "value", "data"):
            if key in value:
                found = find_result_object(value[key])
                if found is not None:
                    return found
        return value if value else None
    if isinstance(value, str):
        try:
            decoded = json.loads(value)
        except json.JSONDecodeError:
            return None
        return find_result_object(decoded)
    return value


def validate_capture_payload(payload: dict[str, Any], adapter: dict[str, Any]) -> None:
    if payload.get("status") != "PASS_BROWSER_AOV_CAPTURE":
        raise BrowserBlocked(f"browser page reported {payload.get('status')}: {payload.get('error', 'unknown error')}")
    manifest = payload.get("manifest")
    capture_receipt = payload.get("receipt")
    sink_records = payload.get("sink_records")
    if not isinstance(manifest, dict) or not isinstance(capture_receipt, dict) or not isinstance(sink_records, list):
        raise BrowserBlocked("browser page did not return manifest, capture receipt, and sink records")
    if manifest.get("schema_version") != "WeaponryThreeJsCaptureManifest@1":
        raise BrowserBlocked("browser capture manifest schema is not the Weaponry schema")
    if manifest.get("rig_id") != "knife-fixed-eight-view@1" or manifest.get("rig_fingerprint") != FIXED_RIG_FINGERPRINT:
        raise BrowserBlocked("browser capture manifest does not use the fixed rig fingerprint")
    if manifest.get("view_ids") != EXPECTED_VIEW_IDS or len(manifest.get("views", [])) != 8:
        raise BrowserBlocked("browser capture manifest does not contain the closed eight-view order")
    if manifest.get("aov_ids") != REQUIRED_AOV_IDS:
        raise BrowserBlocked("browser capture manifest does not contain the closed seven-AOV order")
    if manifest.get("renderer") != "browser-webgl@1" or manifest.get("capture_mode") != "browser-canvas-to-png@1":
        raise BrowserBlocked("browser capture manifest does not prove canvas PNG capture")
    if manifest.get("renderer_invoked") is not True or manifest.get("render_status") != "RENDERED":
        raise BrowserBlocked("browser capture manifest did not prove renderer invocation")
    if manifest.get("quality_status") != "RENDERED_NOT_APPROVED":
        raise BrowserBlocked("browser capture manifest crossed the render approval boundary")
    if any(manifest.get(key) != "NOT_RUN" for key in ("visual_status", "human_status", "engine_status", "commercial_status")):
        raise BrowserBlocked("browser capture manifest crossed a non-render quality boundary")
    for view in manifest["views"]:
        if view.get("view_id") not in EXPECTED_VIEW_IDS or view.get("aovs") is None or len(view["aovs"]) != 7:
            raise BrowserBlocked(f"view {view.get('view_id')} does not contain exactly seven AOV records")
        if [aov.get("aov_id") for aov in view["aovs"]] != REQUIRED_AOV_IDS:
            raise BrowserBlocked(f"view {view.get('view_id')} AOV order is not closed")
        for aov in view["aovs"]:
            if aov.get("mime_type") != "image/png" or aov.get("width") != 256 or aov.get("height") != 256:
                raise BrowserBlocked(f"AOV {view.get('view_id')}/{aov.get('aov_id')} is not a 256x256 PNG record")
            if not isinstance(aov.get("png_sha256"), str) or len(aov["png_sha256"]) != 64 or any(character not in SHA256_PATTERN for character in aov["png_sha256"]):
                raise BrowserBlocked(f"AOV {view.get('view_id')}/{aov.get('aov_id')} has an invalid PNG hash")
            if not isinstance(aov.get("png_size_bytes"), int) or aov["png_size_bytes"] <= 0:
                raise BrowserBlocked(f"AOV {view.get('view_id')}/{aov.get('aov_id')} has an invalid PNG size")
    if capture_receipt.get("expected_view_count") != 8 or capture_receipt.get("captured_view_count") != 8:
        raise BrowserBlocked("browser capture receipt does not prove eight captured views")
    if capture_receipt.get("expected_aov_count_per_view") != 7 or capture_receipt.get("captured_aov_count") != 56:
        raise BrowserBlocked("browser capture receipt does not prove 8x7 AOV capture")
    if capture_receipt.get("missing_capture_count") != 0 or capture_receipt.get("renderer_invoked") is not True:
        raise BrowserBlocked("browser capture receipt reports missing captures or no renderer")
    if payload.get("sink_count") != 56 or len(sink_records) != 56:
        raise BrowserBlocked("capture sink did not receive all 56 PNG byte streams")
    if manifest.get("program_fingerprint") != adapter.get("deterministic_fingerprint"):
        raise BrowserBlocked("manifest program fingerprint does not match the bounded adapter")
    sink_keys = [(item.get("view_id"), item.get("aov_id"), item.get("png_sha256"), item.get("png_size_bytes")) for item in sink_records]
    manifest_keys = [
        (view["view_id"], aov["aov_id"], aov["png_sha256"], aov["png_size_bytes"])
        for view in manifest["views"]
        for aov in view["aovs"]
    ]
    if sink_keys != manifest_keys:
        raise BrowserBlocked("capture sink byte hashes do not match the immutable manifest")


def choose_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


def start_static_server(dist: Path) -> tuple[subprocess.Popen[bytes], int]:
    port = choose_port()
    server = subprocess.Popen(
        [sys.executable, "-m", "http.server", str(port), "--bind", "127.0.0.1"],
        cwd=dist,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if server.poll() is not None:
            raise BrowserBlocked("temporary static server exited before browser navigation")
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                return server, port
        except OSError:
            time.sleep(0.05)
    server.terminate()
    raise BrowserBlocked("temporary static server did not become ready")


def run_aov_benchmark(
    source_path: Path,
    spec_path: Path,
    contract_path: Path,
    baseline_receipt_path: Path,
    scene_template_path: Path,
    aov_template_path: Path,
    adapter_path: Path,
    node_modules: Path,
    playwright_cli: Path,
) -> dict[str, Any]:
    source_info = verify_pinned_source(source_path)
    spec, spec_bytes = load_json(spec_path, "closed benchmark spec")
    contract, contract_bytes = load_json(contract_path, "normalization contract")
    baseline_receipt, _baseline_bytes = load_json(baseline_receipt_path, "img2threejs baseline receipt")
    validate_closed_inputs(contract, baseline_receipt, spec, spec_path)
    if contract.get("source", {}).get("factory_sha256") != baseline_receipt.get("generation", {}).get("factory_sha256"):
        raise BrowserBlocked("normalization contract factory hash does not match the frozen baseline receipt")
    if not shutil.which("npx"):
        raise BrowserBlocked("npx is unavailable; Playwright CLI prerequisite is not satisfied")
    if not node_modules.resolve().joinpath("three", "package.json").is_file():
        raise BrowserBlocked(f"existing Three.js runtime is unavailable at {node_modules.name}; installation is forbidden")
    vite_binary = node_modules.resolve() / ".bin" / "vite"
    if not vite_binary.is_file():
        raise BrowserBlocked("existing Vite CLI is unavailable; installation is forbidden")
    for required in (scene_template_path, aov_template_path, adapter_path):
        if not required.is_file():
            raise BrowserBlocked(f"benchmark bridge file is missing: {required.name}")
    adapter_bytes = adapter_path.read_bytes()
    aov_template_bytes = aov_template_path.read_bytes()
    scene_template_bytes = scene_template_path.read_bytes()
    capture_source_hashes: dict[str, str] = {}
    for name in CAPTURE_SOURCE_FILES:
        source_file = PACKAGE_ROOT / "src" / name
        if not source_file.is_file():
            raise BrowserBlocked(f"current capture source file is missing: {name}")
        capture_source_hashes[f"src/{name}"] = sha256_file(source_file)

    with tempfile.TemporaryDirectory(prefix="weaponry-img2threejs-aov-") as temporary:
        isolated = Path(temporary)
        pinned = isolated / "source"
        pinned.mkdir()
        extract_pinned_source(source_path, pinned)
        isolated_spec = isolated / "input" / spec_path.name
        isolated_spec.parent.mkdir()
        isolated_spec.write_bytes(spec_bytes)
        validation = run_checked(
            [sys.executable, str(pinned / "forge/stage2_spec/validate_sculpt_spec.py"), str(isolated_spec), "--json"],
            cwd=isolated,
            label="pinned ObjectSculptSpec validator",
        )
        validation_payload = parse_json_output(validation.stdout, "validator")
        if validation_payload.get("ok") is not True:
            raise BrowserBlocked("pinned validator rejected the browser AOV fixture")
        generated = isolated / "output" / "DragonfangLikeBaseline.ts"
        generation = run_checked(
            [sys.executable, str(pinned / "forge/stage3_build/generate_threejs_factory.py"), str(isolated_spec), "--out", str(generated), "--allow-nonstrict"],
            cwd=isolated,
            label="pinned img2threejs generator",
        )
        if "non-production test-fixture" not in generation.stderr:
            raise BrowserBlocked("generator did not report its fixture-only non-production mode")
        factory_sha = sha256_file(generated)
        baseline_generation = baseline_receipt.get("generation", {})
        if factory_sha != baseline_generation.get("factory_sha256") or generated.stat().st_size != baseline_generation.get("factory_bytes"):
            raise BrowserBlocked("generated factory does not match the frozen baseline receipt")

        app = isolated / "app"
        (app / "generated").mkdir(parents=True)
        (app / "weaponry-source").mkdir(parents=True)
        shutil.copyfile(generated, app / "generated" / "DragonfangLikeBaseline.ts")
        shutil.copyfile(scene_template_path, app / "upstream-entry.ts")
        shutil.copyfile(aov_template_path, app / "upstream-aov-entry.ts")
        shutil.copyfile(adapter_path, app / "img2threejs-compiled-scene-adapter.ts")
        shutil.copyfile(contract_path, app / contract_path.name)
        for name in CAPTURE_SOURCE_FILES:
            shutil.copyfile(PACKAGE_ROOT / "src" / name, app / "weaponry-source" / name)
        (app / "index.html").write_text(
            "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>Weaponry upstream AOV baseline</title></head>\n"
            "<body><div id=\"app\"></div><script type=\"module\">\n"
            "import { THREE, captureUpstreamAovs } from '/upstream-aov-entry.ts';\n"
            "try {\n"
            "  const canvas = document.createElement('canvas');\n"
            "  document.getElementById('app').appendChild(canvas);\n"
            "  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, preserveDrawingBuffer: true });\n"
            "  renderer.setPixelRatio(1);\n"
            "  renderer.setSize(256, 256, false);\n"
            "  const capture = captureUpstreamAovs(renderer);\n"
            "  globalThis.__WPN_AOV_RESULT__ = { status: 'PASS_BROWSER_AOV_CAPTURE', manifest: capture.result.manifest, receipt: capture.result.receipt, sink_records: capture.sink_records, sink_count: capture.sink_records.length };\n"
            "  document.documentElement.dataset.weaponryAovStatus = 'PASS_BROWSER_AOV_CAPTURE';\n"
            "  renderer.dispose();\n"
            "} catch (error) {\n"
            "  globalThis.__WPN_AOV_ERROR__ = String(error instanceof Error ? error.message : error);\n"
            "  document.documentElement.dataset.weaponryAovStatus = 'BLOCKED_BROWSER_AOV';\n"
            "}\n"
            "</script></body></html>\n",
            encoding="utf-8",
        )
        (app / "node_modules").symlink_to(node_modules.resolve(), target_is_directory=True)
        node_environment = os.environ.copy()
        node_environment["WPN_AOV_ENTRY_URL"] = (app / "upstream-aov-entry.ts").as_uri()
        node_probe = subprocess.run(
            ["node", "--experimental-strip-types", "--input-type=module", "-e", NODE_ADAPTER_PROBE],
            cwd=app,
            check=False,
            capture_output=True,
            text=True,
            env=node_environment,
        )
        if node_probe.returncode != 0:
            detail = (node_probe.stderr.strip() or node_probe.stdout.strip() or "no output").strip()
            raise BrowserBlocked(f"bounded adapter Node probe failed: {detail[-1200:]}")
        adapter_payload = parse_json_output(node_probe.stdout, "bounded adapter Node probe")

        dist = app / "dist"
        vite_version = run_checked([str(vite_binary), "--version"], cwd=app, label="Vite availability probe").stdout.strip()
        run_checked([str(vite_binary), "build", "--outDir", str(dist)], cwd=app, label="offline AOV browser bundle build")
        bundle_files = sorted(path for path in dist.rglob("*") if path.is_file())
        if not bundle_files:
            raise BrowserBlocked("Vite produced an empty AOV bundle")
        bundle_digest = hashlib.sha256()
        bundle_inventory: list[dict[str, Any]] = []
        for path in bundle_files:
            relative = path.relative_to(dist).as_posix()
            data = path.read_bytes()
            bundle_digest.update(relative.encode("utf-8"))
            bundle_digest.update(b"\0")
            bundle_digest.update(data)
            bundle_digest.update(b"\0")
            bundle_inventory.append({"path": relative, "bytes": len(data), "sha256": sha256_bytes(data)})

        server: subprocess.Popen[bytes] | None = None
        session = f"wpn-three-aov-002-{os.getpid()}"
        try:
            server, port = start_static_server(dist)
            base_url = f"http://127.0.0.1:{port}/index.html"
            run_playwright(playwright_cli, session, "open", base_url, cwd=app, label="Playwright browser open")
            run_playwright(
                playwright_cli,
                session,
                "run-code",
                "async (page) => { await page.waitForFunction(() => globalThis.__WPN_AOV_RESULT__ || globalThis.__WPN_AOV_ERROR__, { timeout: 30000 }); }",
                cwd=app,
                label="Playwright browser AOV wait",
            )
            value = run_playwright(
                playwright_cli,
                session,
                "eval",
                "() => globalThis.__WPN_AOV_RESULT__ || { status: 'BLOCKED_BROWSER_AOV', error: globalThis.__WPN_AOV_ERROR__ || 'missing capture result' }",
                cwd=app,
                label="Playwright browser AOV result readback",
            )
            capture_payload = parse_playwright_value(value.stdout, "Playwright browser AOV result")
            if not isinstance(capture_payload, dict):
                raise BrowserBlocked("Playwright browser AOV result is not an object")
            validate_capture_payload(capture_payload, adapter_payload)
        finally:
            try:
                run_playwright(playwright_cli, session, "close", cwd=app, label="Playwright browser close")
            except BrowserBlocked:
                pass
            if server is not None:
                server.terminate()
                try:
                    server.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    server.kill()

        manifest = capture_payload["manifest"]
        capture_receipt = capture_payload["receipt"]
        return {
            "source": source_info,
            "input": {
                "spec_path": str(spec_path.relative_to(PACKAGE_ROOT)),
                "spec_sha256": sha256_bytes(spec_bytes),
                "schema_version": spec.get("schemaVersion"),
                "target_name": spec.get("targetName"),
                "component_count": len(spec.get("componentTree", [])),
                "material_count": len(spec.get("materials", [])),
            },
            "adapter": {
                "module": "benchmark/img2threejs-compiled-scene-adapter.ts",
                "schema_version": adapter_payload.get("schema_version"),
                "deterministic_fingerprint": adapter_payload.get("deterministic_fingerprint"),
                "mesh_count": adapter_payload.get("mesh_count"),
                "triangle_count": adapter_payload.get("triangle_count"),
                "assembly_part_count": adapter_payload.get("assembly_part_count"),
                "assembly_status": adapter_payload.get("assembly_status"),
                "part_ids": adapter_payload.get("part_ids"),
                "material_zone_ids": adapter_payload.get("material_zone_ids"),
                "parts": adapter_payload.get("parts"),
                "adapter_sha256": sha256_bytes(adapter_bytes),
            },
            "capture_source": {
                "files": capture_source_hashes,
                "route": "existing-captureKnifeAovs@1",
                "source_runtime_typecheck": "NOT_RUN_CURRENT_WORKTREE_HAS_UNFINISHED_CALIBRATION_SYMBOLS",
            },
            "entry": {
                "scene_template_sha256": sha256_bytes(scene_template_bytes),
                "aov_template_sha256": sha256_bytes(aov_template_bytes),
                "contract_sha256": sha256_bytes(contract_bytes),
                "factory_sha256": factory_sha,
                "factory_persisted": False,
            },
            "rig": {
                "schema_version": manifest.get("schema_version"),
                "rig_id": manifest.get("rig_id"),
                "rig_fingerprint": manifest.get("rig_fingerprint"),
                "frame_width": manifest.get("frame_width"),
                "frame_height": manifest.get("frame_height"),
                "margin": manifest.get("rig_margin"),
                "view_ids": manifest.get("view_ids"),
                "view_count": len(manifest.get("views", [])),
                "camera_bindings": [
                    {
                        "view_id": view["view_id"],
                        "projection": view["camera"]["projection"],
                        "camera_fingerprint": view["camera"]["camera_fingerprint"],
                        "matrix_world": view["camera"]["matrix_world"],
                        "matrix_world_inverse": view["camera"]["matrix_world_inverse"],
                        "projection_matrix": view["camera"]["projection_matrix"],
                    }
                    for view in manifest["views"]
                ],
            },
            "capture": {
                "renderer": manifest.get("renderer"),
                "capture_mode": manifest.get("capture_mode"),
                "renderer_invoked": manifest.get("renderer_invoked"),
                "render_status": manifest.get("render_status"),
                "quality_status": manifest.get("quality_status"),
                "manifest": manifest,
                "receipt": capture_receipt,
                "sink_png_count": capture_payload.get("sink_count"),
                "sink_png_total_bytes": sum(item.get("png_size_bytes", 0) for item in capture_payload.get("sink_records", [])),
                "png_artifacts_persisted": False,
            },
            "reference_calibration": {
                "status": "NOT_RUN",
                "reason": "The closed fixture has no authorized FRONT reference bytes or reference part-ID binding; the fixed rig was reused without refitting.",
                "minimum_gap": "Provide an authorized FRONT reference plus explicit focus_part_ids and a valid baseline calibration receipt before any reference comparison.",
                "superiority": "NOT_COMPUTED",
            },
            "browser": {
                "status": "PASS_BROWSER_AOV_CAPTURE",
                "engine": "Playwright Chromium",
                "vite_version": vite_version,
                "static_network_policy": "local-static-bundle-only@1",
                "external_network_used": False,
                "dependencies_installed": False,
                "bundle": {
                    "file_count": len(bundle_inventory),
                    "bytes": sum(item["bytes"] for item in bundle_inventory),
                    "sha256": bundle_digest.hexdigest(),
                    "files": bundle_inventory,
                    "persisted": False,
                },
            },
            "quality": {
                "quality_status": "NOT_RUN",
                "visual_status": "NOT_RUN",
                "human_status": "NOT_RUN",
                "engine_status": "NOT_RUN",
                "commercial_status": "NOT_RUN",
                "visual_superiority": "NOT_COMPUTED",
            },
        }


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-checkout", type=Path, required=True)
    parser.add_argument("--spec", type=Path, default=DEFAULT_SPEC)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--baseline-receipt", type=Path, default=DEFAULT_BASELINE_RECEIPT)
    parser.add_argument("--scene-template", type=Path, default=DEFAULT_SCENE_TEMPLATE)
    parser.add_argument("--aov-template", type=Path, default=DEFAULT_AOV_TEMPLATE)
    parser.add_argument("--adapter", type=Path, default=DEFAULT_ADAPTER)
    parser.add_argument("--node-modules", type=Path, default=DEFAULT_NODE_MODULES)
    parser.add_argument("--playwright-cli", type=Path, default=discover_playwright_cli())
    parser.add_argument("--receipt", type=Path, default=DEFAULT_RECEIPT)
    parser.add_argument("--force", action="store_true", help="overwrite an existing receipt")
    args = parser.parse_args(argv)
    receipt_path = args.receipt.expanduser().resolve()
    if receipt_path != BENCHMARK_ROOT and BENCHMARK_ROOT not in receipt_path.parents:
        print("BLOCKED: receipt must remain inside packages/weaponry-threejs/benchmark", file=sys.stderr)
        return 2
    if receipt_path.exists() and not args.force:
        print(f"BLOCKED: receipt already exists: {receipt_path}; use --force to refresh", file=sys.stderr)
        return 2
    try:
        result = run_aov_benchmark(
            args.source_checkout.expanduser().resolve(),
            args.spec.expanduser().resolve(),
            args.contract.expanduser().resolve(),
            args.baseline_receipt.expanduser().resolve(),
            args.scene_template.expanduser().resolve(),
            args.aov_template.expanduser().resolve(),
            args.adapter.expanduser().resolve(),
            args.node_modules.expanduser().resolve(),
            args.playwright_cli.expanduser().resolve(),
        )
    except (BenchmarkBlocked, BrowserBlocked, OSError, ValueError) as error:
        print(f"BLOCKED: {error}", file=sys.stderr)
        return 2

    receipt = {
        "schema_version": "WeaponryThreeJsImg2ThreeJsBrowserAovReceipt@1",
        "task_id": "WPN-THREE-UPSTREAM-AOV-002",
        "benchmark_only": True,
        "status": "PASS_BROWSER_AOV_MANIFEST",
        "quality_status": "NOT_RUN",
        "visual_superiority": "NOT_COMPUTED",
        "upstream_execution_scope": "isolated-temporary-benchmark-only",
        "upstream_generator_executed": True,
        "browser_renderer_invoked": True,
        "product_runtime_execution": False,
        "runtime_store_cas_write": False,
        "network_used": False,
        "dependencies_installed": False,
        **result,
    }
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(json.dumps(receipt, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(receipt, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
