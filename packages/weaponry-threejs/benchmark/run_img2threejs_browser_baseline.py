#!/usr/bin/env python3
"""Build an offline browser baseline from the pinned img2threejs fixture.

The generated upstream factory and Vite output are both confined to a
temporary directory.  The repository keeps only this runner, its closed
normalization contract, and a structural receipt.  No WebGL context is
created here, so this script never reports PNG/AOV, visual, human, engine, or
commercial acceptance.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
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


PACKAGE_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = PACKAGE_ROOT.parents[1]
BENCHMARK_ROOT = PACKAGE_ROOT / "benchmark"
DEFAULT_SPEC = BENCHMARK_ROOT / "dragonfang-like-objects-sculpt-spec.json"
DEFAULT_CONTRACT = BENCHMARK_ROOT / "upstream-render-normalization.contract.json"
DEFAULT_TEMPLATE = BENCHMARK_ROOT / "upstream-browser-entry.template.ts"
DEFAULT_BASELINE_RECEIPT = BENCHMARK_ROOT / "img2threejs-baseline.receipt.json"
DEFAULT_RECEIPT = BENCHMARK_ROOT / "img2threejs-browser-baseline.receipt.json"
NODE_PROBE = r"""
import * as THREE from 'three';

const entry = await import(process.env.WPN_BROWSER_ENTRY_URL);
if (entry.UPSTREAM_RENDER_NORMALIZATION_CONTRACT?.schema_version !== 'WeaponryThreeJsUpstreamRenderNormalizationContract@1') {
  throw new Error('browser entry did not export the closed normalization contract');
}
if (entry.UPSTREAM_REQUIRED_AOV_IDS.join(',') !== 'beauty,silhouette,depth,normal,part-id,material-id,wireframe') {
  throw new Error('browser entry required AOV order does not match Weaponry');
}
if (entry.UPSTREAM_FIXED_VIEW_IDS.join(',') !== 'FRONT,BACK,TOP,BOTTOM,LEFT,RIGHT,REAR_THREE_QUARTER,FPS_HOLD') {
  throw new Error('browser entry fixed-view order does not match Weaponry');
}

const first = entry.createUpstreamBaselineScene();
const second = entry.createUpstreamBaselineScene();
const rig = entry.createUpstreamFixedViewRig();
const cameras = entry.createUpstreamFixedCameras(rig);
const firstObjectIds = entry.stableObjectPathIds(first.root);
const secondObjectIds = entry.stableObjectPathIds(second.root);
const firstSceneIdentity = entry.stableSceneIdentity(first.root);
const secondSceneIdentity = entry.stableSceneIdentity(second.root);
if (JSON.stringify(firstObjectIds) !== JSON.stringify(secondObjectIds)
  || JSON.stringify(firstSceneIdentity) !== JSON.stringify(secondSceneIdentity)
  || first.scene.uuid !== second.scene.uuid) {
  throw new Error('stable scene identity is not repeatable across factory invocations');
}

function vectorValues(vector) {
  return [vector.x, vector.y, vector.z];
}

function boundsCorners(bounds) {
  const min = bounds.min;
  const max = bounds.max;
  return [
    new THREE.Vector3(min.x, min.y, min.z),
    new THREE.Vector3(min.x, min.y, max.z),
    new THREE.Vector3(min.x, max.y, min.z),
    new THREE.Vector3(min.x, max.y, max.z),
    new THREE.Vector3(max.x, min.y, min.z),
    new THREE.Vector3(max.x, min.y, max.z),
    new THREE.Vector3(max.x, max.y, min.z),
    new THREE.Vector3(max.x, max.y, max.z),
  ];
}

function cameraFingerprint(view, camera) {
  return sha256(JSON.stringify(canonical({
    view_id: view.view_id,
    projection: view.projection,
    matrix_world: camera.matrixWorld.toArray(),
    matrix_world_inverse: camera.matrixWorldInverse.toArray(),
    projection_matrix: camera.projectionMatrix.toArray(),
  })));
}

function cameraSummary(view, camera, bounds, index) {
  camera.updateMatrixWorld(true);
  const projected = boundsCorners(bounds).map((corner) => corner.project(camera));
  const clipVisible = projected.filter((point) => Number.isFinite(point.x) && Number.isFinite(point.y) && Number.isFinite(point.z)
    && point.x >= -1 && point.x <= 1 && point.y >= -1 && point.y <= 1 && point.z >= -1 && point.z <= 1);
  return {
    index,
    view_id: view.view_id,
    projection: view.projection,
    camera_uuid: camera.uuid,
    camera_name: camera.name,
    position: vectorValues(camera.position),
    matrix_world: camera.matrixWorld.toArray(),
    matrix_world_inverse: camera.matrixWorldInverse.toArray(),
    projection_matrix: camera.projectionMatrix.toArray(),
    camera_fingerprint: cameraFingerprint(view, camera),
    viewport: { x: 0, y: 0, width: rig.frame_width, height: rig.frame_height },
    projected_corner_count: projected.length,
    clip_visible_corner_count: clipVisible.length,
    all_bounds_corners_clip_visible: clipVisible.length === projected.length,
  };
}

const meshes = [];
first.root.traverse((object) => {
  if (!object.isMesh) return;
  const component = object.userData?.sculptComponent;
  if (typeof component?.id !== 'string' || typeof component?.primitive !== 'string') {
    throw new Error(`mesh ${object.name || '(unnamed)'} is missing sculptComponent metadata`);
  }
  const position = object.geometry.getAttribute('position');
  const index = object.geometry.getIndex();
  const triangles = index ? index.count / 3 : position.count / 3;
  if (!Number.isInteger(triangles) || triangles <= 0) throw new Error(`invalid triangle count for ${component.id}`);
  meshes.push({
    id: component.id,
    primitive: component.primitive,
    triangles,
    material_count: Array.isArray(object.material) ? object.material.length : 1,
  });
});
const ids = new Set();
for (const part of meshes) {
  if (ids.has(part.id)) throw new Error(`duplicate generated part id: ${part.id}`);
  ids.add(part.id);
}

const boundsBefore = entry.boundsSummary(first.bounds_before);
const boundsAfter = entry.boundsSummary(first.bounds_after);
const normalizedCenterError = Math.max(...boundsAfter.center.map((value) => Math.abs(value)));
const targetExtent = entry.UPSTREAM_RENDER_NORMALIZATION_CONTRACT.scene_normalization.target_max_extent;
const normalizedExtentError = Math.abs(boundsAfter.max_extent - targetExtent);
const camerasReceipt = rig.views.map((view, index) => cameraSummary(view, cameras[index], first.bounds_after, index));
console.log(JSON.stringify({
  entry_schema_version: 'WeaponryThreeJsUpstreamBrowserBaseline@1',
  scene_name: first.scene.name,
  root_name: first.root.name,
  source_root_name: first.source_root.name,
  source_center: vectorValues(first.source_center),
  source_size: vectorValues(first.source_size),
  uniform_scale: first.uniform_scale,
  bounds_before: boundsBefore,
  bounds_after: boundsAfter,
  normalized_center_error: normalizedCenterError,
  normalized_extent_error: normalizedExtentError,
  target_max_extent: targetExtent,
  mesh_count: meshes.length,
  triangles: meshes.reduce((sum, part) => sum + part.triangles, 0),
  parts: meshes,
  stable_object_id_count: firstObjectIds.length,
  stable_object_ids_repeatable: true,
  stable_scene_identity_count: firstSceneIdentity.length,
  stable_scene_identity_repeatable: true,
  rig: {
    schema_version: rig.schema_version,
    rig_id: rig.rig_id,
    coordinate_convention: rig.coordinate_convention,
    frame_width: rig.frame_width,
    frame_height: rig.frame_height,
    margin: rig.margin,
    deterministic_fingerprint: rig.deterministic_fingerprint,
    view_ids: rig.views.map((view) => view.view_id),
  },
  cameras: camerasReceipt,
}));

function canonical(value) {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number') return Object.is(value, -0) ? 0 : value;
  if (Array.isArray(value)) return value.map(canonical);
  const result = {};
  for (const key of Object.keys(value).sort()) result[key] = canonical(value[key]);
  return result;
}

function sha256(value) {
  return requireNodeCrypto().createHash('sha256').update(value, 'utf8').digest('hex');
}

function requireNodeCrypto() {
  // The probe is executed by Node and is never shipped in the browser bundle.
  return process.getBuiltinModule('node:crypto');
}
"""


def load_json(path: Path, label: str) -> tuple[dict[str, Any], bytes]:
    try:
        raw = path.read_bytes()
        value = json.loads(raw.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BenchmarkBlocked(f"{label} is unreadable JSON: {error}") from error
    if not isinstance(value, dict):
        raise BenchmarkBlocked(f"{label} must be a JSON object")
    return value, raw


def validate_closed_inputs(
    contract: dict[str, Any],
    baseline_receipt: dict[str, Any],
    spec: dict[str, Any],
    spec_path: Path,
) -> None:
    if contract.get("schema_version") != "WeaponryThreeJsUpstreamRenderNormalizationContract@1":
        raise BenchmarkBlocked("normalization contract schema is not supported")
    source = contract.get("source")
    baseline_source = baseline_receipt.get("source")
    if not isinstance(source, dict) or not isinstance(baseline_source, dict):
        raise BenchmarkBlocked("normalization contract or baseline receipt has no source binding")
    if source.get("revision") != REVISION or source.get("tree") != EXPECTED_TREE:
        raise BenchmarkBlocked("normalization contract is not bound to the pinned source")
    if baseline_source.get("revision") != REVISION or baseline_source.get("tree") != EXPECTED_TREE:
        raise BenchmarkBlocked("baseline receipt is not bound to the pinned source")
    if source.get("fixture_spec_sha256") != sha256_file(spec_path):
        raise BenchmarkBlocked("normalization contract fixture hash does not match the closed spec")
    if baseline_receipt.get("status") != "PASS_STRUCTURAL_BASELINE" or baseline_receipt.get("quality_status") != "NOT_RUN":
        raise BenchmarkBlocked("baseline receipt crossed the structural-only quality boundary")
    rig = contract.get("fixed_view_rig")
    if not isinstance(rig, dict) or rig.get("schema_version") != "KnifeFixedEightViewRig@1" or len(rig.get("views", [])) != 8:
        raise BenchmarkBlocked("normalization contract does not contain the closed eight-view rig")
    aovs = contract.get("aov_contract")
    if not isinstance(aovs, dict) or aovs.get("required") != [
        "beauty", "silhouette", "depth", "normal", "part-id", "material-id", "wireframe"
    ]:
        raise BenchmarkBlocked("normalization contract required AOV order is not the Weaponry set")
    if not isinstance(spec.get("componentTree"), list) or not spec["componentTree"]:
        raise BenchmarkBlocked("closed fixture does not contain componentTree entries")


def bundle_inventory(dist: Path) -> dict[str, Any]:
    digest = hashlib.sha256()
    files: list[dict[str, Any]] = []
    for path in sorted((candidate for candidate in dist.rglob("*") if candidate.is_file()), key=lambda item: item.relative_to(dist).as_posix()):
        relative = path.relative_to(dist).as_posix()
        data = path.read_bytes()
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(data)
        digest.update(b"\0")
        files.append({"path": relative, "bytes": len(data), "sha256": sha256_bytes(data)})
    if not files:
        raise BenchmarkBlocked("Vite produced an empty browser bundle")
    return {
        "file_count": len(files),
        "bytes": sum(item["bytes"] for item in files),
        "sha256": digest.hexdigest(),
        "files": files,
    }


def run_browser_benchmark(
    source_path: Path,
    spec_path: Path,
    contract_path: Path,
    template_path: Path,
    baseline_receipt_path: Path,
    node_modules: Path,
) -> dict[str, Any]:
    source_info = verify_pinned_source(source_path)
    spec, spec_bytes = load_json(spec_path, "closed benchmark spec")
    contract, contract_bytes = load_json(contract_path, "normalization contract")
    baseline_receipt, _baseline_bytes = load_json(baseline_receipt_path, "img2threejs baseline receipt")
    validate_closed_inputs(contract, baseline_receipt, spec, spec_path)

    template_bytes = template_path.read_bytes()
    if b"createDragonfangLikeBaselineModel" not in template_bytes or b"createUpstreamBaselineScene" not in template_bytes:
        raise BenchmarkBlocked("browser entry template does not expose the closed factory/scene entry")
    node_modules = node_modules.expanduser().resolve()
    if not (node_modules / "three" / "package.json").is_file():
        raise BenchmarkBlocked(f"existing Three.js runtime is unavailable at {node_modules}; installation is forbidden")
    vite_binary = node_modules / ".bin" / "vite"
    if not vite_binary.is_file():
        raise BenchmarkBlocked(f"existing Vite CLI is unavailable at {vite_binary}; installation is forbidden")

    baseline_source = baseline_receipt["source"]
    baseline_generation = baseline_receipt.get("generation", {})
    with tempfile.TemporaryDirectory(prefix="weaponry-img2threejs-browser-") as temporary:
        isolated = Path(temporary)
        pinned = isolated / "source"
        pinned.mkdir()
        extract_pinned_source(source_path, pinned)
        isolated_spec = isolated / "input" / spec_path.name
        isolated_spec.parent.mkdir()
        isolated_spec.write_bytes(spec_bytes)

        validation = run_checked(
            [
                sys.executable,
                str(pinned / "forge/stage2_spec/validate_sculpt_spec.py"),
                str(isolated_spec),
                "--json",
            ],
            cwd=isolated,
            label="pinned ObjectSculptSpec validator",
        )
        validation_payload = parse_json_output(validation.stdout, "validator")
        if validation_payload.get("ok") is not True:
            raise BenchmarkBlocked(f"pinned validator rejected the browser fixture: {validation.stdout.strip()}")

        generated = isolated / "output" / "DragonfangLikeBaseline.ts"
        generation = run_checked(
            [
                sys.executable,
                str(pinned / "forge/stage3_build/generate_threejs_factory.py"),
                str(isolated_spec),
                "--out",
                str(generated),
                "--allow-nonstrict",
            ],
            cwd=isolated,
            label="pinned img2threejs generator",
        )
        if "non-production test-fixture" not in generation.stderr:
            raise BenchmarkBlocked("generator did not report its fixture-only non-production mode")
        factory_sha = sha256_file(generated)
        factory_bytes = generated.stat().st_size
        if factory_sha != baseline_generation.get("factory_sha256") or factory_bytes != baseline_generation.get("factory_bytes"):
            raise BenchmarkBlocked("temporary generated factory does not match the frozen baseline receipt")

        app = isolated / "app"
        generated_dir = app / "generated"
        generated_dir.mkdir(parents=True)
        shutil.copyfile(generated, generated_dir / "DragonfangLikeBaseline.ts")
        shutil.copyfile(template_path, app / "upstream-entry.ts")
        shutil.copyfile(contract_path, app / contract_path.name)
        (app / "index.html").write_text(
            "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>Weaponry upstream baseline</title></head>\n"
            "<body><div id=\"app\"></div><script type=\"module\">\n"
            "import { createUpstreamBaselineScene, createUpstreamFixedViewRig, createUpstreamFixedCameras } from '/upstream-entry.ts';\n"
            "const baseline = createUpstreamBaselineScene();\n"
            "const rig = createUpstreamFixedViewRig();\n"
            "const cameras = createUpstreamFixedCameras(rig);\n"
            "document.documentElement.dataset.weaponryBaseline = JSON.stringify({ scene: baseline.scene.name, views: cameras.length, rig: rig.rig_id });\n"
            "</script></body></html>\n",
            encoding="utf-8",
        )
        (app / "node_modules").symlink_to(node_modules, target_is_directory=True)
        dist = app / "dist"
        vite_version = run_checked(
            [str(vite_binary), "--version"],
            cwd=app,
            label="Vite availability probe",
        ).stdout.strip()
        vite = run_checked(
            [str(vite_binary), "build", "--outDir", str(dist)],
            cwd=app,
            label="offline Vite browser baseline build",
        )
        dist_repeat = app / "dist-repeat"
        run_checked(
            [str(vite_binary), "build", "--outDir", str(dist_repeat)],
            cwd=app,
            label="offline Vite browser baseline repeatability build",
        )

        node_environment = os.environ.copy()
        node_environment["WPN_BROWSER_ENTRY_URL"] = (app / "upstream-entry.ts").as_uri()
        node_version = run_checked(["node", "--version"], cwd=app, label="Node.js availability probe").stdout.strip()
        execution = subprocess.run(
            ["node", "--experimental-strip-types", "--input-type=module", "-e", NODE_PROBE],
            cwd=app,
            check=False,
            capture_output=True,
            text=True,
            env=node_environment,
        )
        if execution.returncode != 0:
            detail = (execution.stderr.strip() or execution.stdout.strip() or "no output").strip()
            raise BenchmarkBlocked(f"browser baseline entry execution failed with exit {execution.returncode}: {detail}")
        execution_payload = parse_json_output(execution.stdout, "browser baseline entry execution")

        parts = execution_payload.get("parts")
        expected_ids = [
            item.get("id")
            for item in spec.get("componentTree", [])
            if isinstance(item, dict) and item.get("level", "macro") == "macro"
        ]
        actual_ids = [item.get("id") for item in parts] if isinstance(parts, list) else []
        if actual_ids != expected_ids:
            raise BenchmarkBlocked(f"browser baseline part order differs from the closed fixture: expected={expected_ids} actual={actual_ids}")

        bundle = bundle_inventory(dist)
        repeat_bundle = bundle_inventory(dist_repeat)
        if bundle["sha256"] != repeat_bundle["sha256"] or bundle["bytes"] != repeat_bundle["bytes"]:
            raise BenchmarkBlocked("repeated offline Vite build did not produce the same bundle hash")
        bundle["repeatability"] = "PASS_SAME_INPUT_HASHED_OUTPUT"
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
            "factory": {
                "generator": "forge/stage3_build/generate_threejs_factory.py",
                "factory_sha256": factory_sha,
                "factory_bytes": factory_bytes,
                "temporary_only": True,
                "generated_source_persisted": False,
            },
            "entry": {
                "template_path": str(template_path.relative_to(PACKAGE_ROOT)),
                "template_sha256": sha256_bytes(template_bytes),
                "contract_path": str(contract_path.relative_to(PACKAGE_ROOT)),
                "contract_sha256": sha256_bytes(contract_bytes),
                "temporary_import_path": "generated/DragonfangLikeBaseline.ts",
                "scene_entry": "createUpstreamBaselineScene",
                "camera_entry": "createUpstreamFixedCameras",
            },
            "normalization": {
                "contract_id": contract.get("contract_id"),
                "schema_version": contract.get("scene_normalization", {}).get("schema_version"),
                "bounds_before": execution_payload.get("bounds_before"),
                "bounds_after": execution_payload.get("bounds_after"),
                "source_center": execution_payload.get("source_center"),
                "source_size": execution_payload.get("source_size"),
                "uniform_scale": execution_payload.get("uniform_scale"),
                "target_max_extent": execution_payload.get("target_max_extent"),
                "normalized_center_error": execution_payload.get("normalized_center_error"),
                "normalized_extent_error": execution_payload.get("normalized_extent_error"),
                "bounds_centered": execution_payload.get("normalized_center_error", float("inf")) <= 1e-9,
                "target_extent_reached": execution_payload.get("normalized_extent_error", float("inf")) <= 1e-9,
                "stable_object_id_count": execution_payload.get("stable_object_id_count"),
                "stable_object_ids_repeatable": execution_payload.get("stable_object_ids_repeatable"),
                "stable_scene_identity_count": execution_payload.get("stable_scene_identity_count"),
                "stable_scene_identity_repeatable": execution_payload.get("stable_scene_identity_repeatable"),
            },
            "camera": {
                "rig_schema_version": execution_payload.get("rig", {}).get("schema_version"),
                "rig_id": execution_payload.get("rig", {}).get("rig_id"),
                "coordinate_convention": execution_payload.get("rig", {}).get("coordinate_convention"),
                "rig_fingerprint": execution_payload.get("rig", {}).get("deterministic_fingerprint"),
                "frame_width": execution_payload.get("rig", {}).get("frame_width"),
                "frame_height": execution_payload.get("rig", {}).get("frame_height"),
                "margin": execution_payload.get("rig", {}).get("margin"),
                "view_ids": execution_payload.get("rig", {}).get("view_ids"),
                "view_count": len(execution_payload.get("cameras", [])),
                "matrix_binding_status": "STRUCTURAL_ONLY",
                "views": execution_payload.get("cameras"),
                "all_bounds_corners_clip_visible": all(
                    item.get("all_bounds_corners_clip_visible") is True
                    for item in execution_payload.get("cameras", [])
                ),
            },
            "execution": {
                "mode": "isolated-generated-factory-node-and-vite-build",
                "node_version": node_version,
                "vite_version": vite_version,
                "mesh_count": execution_payload.get("mesh_count"),
                "triangles": execution_payload.get("triangles"),
                "parts": parts,
                "vite_build_status": "PASS",
                "renderer_invoked": False,
                "network_used": False,
                "dependencies_installed": False,
                "product_runtime_invoked": False,
                "runtime_store_cas_write": False,
            },
            "bundle": {
                "build_status": "BUILT",
                "renderer_bundle_role": "browser-loadable-scene-entry-only",
                "network_policy": "bundled-static-only@1",
                "artifact_persisted": False,
                "output_discarded_after_receipt": True,
                **bundle,
            },
            "aov": {
                "required": contract["aov_contract"]["required"],
                "optional": contract["aov_contract"]["optional"],
                "capture_mode": contract["aov_contract"]["capture_mode"],
                "capture_status": "NOT_RUN",
                "renderer_invoked": False,
                "png_count": 0,
                "missing_boundary": contract["aov_contract"]["missing_renderer_boundary"],
                "existing_weaponry_capture_reuse": contract["aov_contract"]["existing_weaponry_capture_reuse"],
            },
            "gap": {
                "minimum_next_step": "Add the bounded raw-THREE.Group-to-CompiledKnifeScene adapter, run the temporary entry in a real browser with THREE.WebGLRenderer(preserveDrawingBuffer=true), then produce the closed 8x7 PNG capture manifest.",
                "capture_adapter_required": True,
                "capture_adapter_reason": "Existing captureKnifeAovs validates CompiledKnifeScene ownership and cannot consume the raw upstream Group directly.",
                "visual_quality": "NOT_RUN",
                "visual_superiority": "NOT_COMPUTED",
                "human_review": "NOT_RUN",
                "engine_acceptance": "NOT_RUN",
                "commercial_acceptance": "NOT_RUN",
            },
        }


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-checkout", type=Path, required=True)
    parser.add_argument("--spec", type=Path, default=DEFAULT_SPEC)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--template", type=Path, default=DEFAULT_TEMPLATE)
    parser.add_argument("--baseline-receipt", type=Path, default=DEFAULT_BASELINE_RECEIPT)
    parser.add_argument("--node-modules", type=Path, default=REPOSITORY_ROOT / "node_modules")
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
        result = run_browser_benchmark(
            args.source_checkout,
            args.spec.expanduser().resolve(),
            args.contract.expanduser().resolve(),
            args.template.expanduser().resolve(),
            args.baseline_receipt.expanduser().resolve(),
            args.node_modules,
        )
    except (BenchmarkBlocked, OSError, ValueError) as error:
        print(f"BLOCKED: {error}", file=sys.stderr)
        return 2

    receipt = {
        "schema_version": "WeaponryThreeJsImg2ThreeJsBrowserBaselineReceipt@1",
        "task_id": "WPN-THREE-UPSTREAM-RENDER-001",
        "benchmark_only": True,
        "status": "PASS_STRUCTURAL_BROWSER_BUNDLE",
        "quality_status": "NOT_RUN",
        "visual_superiority": "NOT_COMPUTED",
        "upstream_execution_scope": "isolated-temporary-benchmark-only",
        "upstream_generator_executed": True,
        "browser_entry_executed": True,
        "browser_bundle_built": True,
        "renderer_invoked": False,
        "product_runtime_execution": False,
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
