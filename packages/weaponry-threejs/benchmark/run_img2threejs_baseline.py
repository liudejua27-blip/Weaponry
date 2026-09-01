#!/usr/bin/env python3
"""Run the pinned img2threejs generator on a closed structural fixture.

This is a benchmark-only bridge.  It extracts the exact pinned source from an
already available git object into a temporary directory, runs the stdlib-only
validator/generator there, and executes the generated factory with the
repository's already-installed Three.js package.  It never installs packages,
fetches the network, writes Runtime/Store/CAS, or copies generated upstream
source into the product tree.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
from typing import Any


REVISION = "9fbd0ca5bbcc3b13bebe712745d6784d33db0b85"
EXPECTED_TREE = "0ee3c2a6d781407808df98b33174539842f85fcc"
EXPECTED_TREE_MANIFEST = "bf4b35eb5b468a77ae6d8a24fc2e4b7a42fa27ad87c2afacd32bf635e069d91e"
EXPECTED_FILE_COUNT = 337
EXPECTED_BLOB_BYTES = 4126550

PACKAGE_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = PACKAGE_ROOT.parents[1]
DEFAULT_SPEC = PACKAGE_ROOT / "benchmark" / "dragonfang-like-objects-sculpt-spec.json"
DEFAULT_RECEIPT = PACKAGE_ROOT / "benchmark" / "img2threejs-baseline.receipt.json"

NODE_PROBE = r"""
const generated = await import(process.env.WPN_GENERATED_FACTORY_URL);
const factoryEntry = Object.entries(generated).find(([name, value]) =>
  name.startsWith("create") && name.endsWith("Model") && typeof value === "function"
);
if (!factoryEntry) throw new Error("generated factory export create*Model was not found");
const [, factory] = factoryEntry;
const root = factory();
const meshes = [];
root.traverse((object) => {
  if (!object.isMesh) return;
  const component = object.userData?.sculptComponent;
  const id = component?.id;
  const primitive = component?.primitive;
  if (typeof id !== "string" || typeof primitive !== "string") {
    throw new Error(`mesh ${object.name || "(unnamed)"} is missing sculptComponent id/primitive`);
  }
  const position = object.geometry.getAttribute("position");
  const index = object.geometry.getIndex();
  const triangles = index ? index.count / 3 : position.count / 3;
  if (!Number.isFinite(triangles) || !Number.isInteger(triangles) || triangles <= 0) {
    throw new Error(`component ${id} emitted an invalid triangle count: ${triangles}`);
  }
  meshes.push({ id, primitive, triangles });
});
const ids = new Set();
for (const part of meshes) {
  if (ids.has(part.id)) throw new Error(`duplicate generated part id: ${part.id}`);
  ids.add(part.id);
}
console.log(JSON.stringify({
  rootName: root.name,
  meshCount: meshes.length,
  parts: meshes,
  triangles: meshes.reduce((sum, part) => sum + part.triangles, 0),
}));
"""


class BenchmarkBlocked(RuntimeError):
    """A precise local blocker that should not be mistaken for a failed quality gate."""


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run_git(source: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(source), *args],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise BenchmarkBlocked(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout


def run_git_bytes(source: Path, *args: str) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(source), *args],
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise BenchmarkBlocked(detail or f"git {' '.join(args)} failed")
    return result.stdout


def source_blob(source: Path, relative_path: str) -> bytes:
    return run_git_bytes(source, "show", f"{REVISION}:{relative_path}")


def verify_pinned_source(source: Path) -> dict[str, Any]:
    source = source.expanduser().resolve()
    if not source.is_dir() or not (source / ".git").exists():
        raise BenchmarkBlocked(f"source checkout is not a git checkout: {source}")
    if source == PACKAGE_ROOT or PACKAGE_ROOT in source.parents:
        raise BenchmarkBlocked("source checkout must remain outside packages/weaponry-threejs")

    resolved_revision = run_git(source, "rev-parse", f"{REVISION}^{{commit}}").strip()
    if resolved_revision != REVISION:
        raise BenchmarkBlocked(f"pinned commit resolved to {resolved_revision}, expected {REVISION}")
    tree = run_git(source, "rev-parse", f"{REVISION}^{{tree}}").strip()
    if tree != EXPECTED_TREE:
        raise BenchmarkBlocked(f"pinned tree is {tree}, expected {EXPECTED_TREE}")

    listing = run_git(source, "ls-tree", "-r", "--long", REVISION)
    digest = hashlib.sha256()
    file_count = 0
    blob_bytes = 0
    for line in listing.splitlines():
        header, separator, path = line.partition("\t")
        if not separator:
            raise BenchmarkBlocked("git ls-tree returned a malformed entry")
        fields = header.split()
        if len(fields) != 4 or fields[1] != "blob":
            raise BenchmarkBlocked(f"unexpected pinned tree entry: {line}")
        size = int(fields[3])
        digest.update(f"{path}\t{fields[2]}\t{size}\n".encode("utf-8"))
        file_count += 1
        blob_bytes += size
    manifest_digest = digest.hexdigest()
    if (file_count, blob_bytes, manifest_digest) != (
        EXPECTED_FILE_COUNT,
        EXPECTED_BLOB_BYTES,
        EXPECTED_TREE_MANIFEST,
    ):
        raise BenchmarkBlocked(
            "pinned source inventory mismatch: "
            f"count={file_count} bytes={blob_bytes} digest={manifest_digest}"
        )

    requirements = source_blob(source, "forge/requirements.txt").decode("utf-8")
    if "standard library only" not in requirements or "NO third-party dependencies" not in requirements:
        raise BenchmarkBlocked("pinned forge/requirements.txt does not confirm the stdlib-only core")

    return {
        "project": "img2threejs",
        "revision": REVISION,
        "tree": tree,
        "tree_manifest_sha256": manifest_digest,
        "tracked_file_count": file_count,
        "tracked_blob_bytes": blob_bytes,
        "generator_sha256": sha256_bytes(source_blob(source, "forge/stage3_build/generate_threejs_factory.py")),
        "validator_sha256": sha256_bytes(source_blob(source, "forge/stage2_spec/validate_sculpt_spec.py")),
        "requirements_sha256": sha256_bytes(source_blob(source, "forge/requirements.txt")),
    }


def extract_pinned_source(source: Path, destination: Path) -> None:
    archive = run_git_bytes(source, "archive", "--format=tar", REVISION)
    destination_root = destination.resolve()
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as stream:
        members = stream.getmembers()
        for member in members:
            target = (destination / member.name).resolve()
            if target != destination_root and destination_root not in target.parents:
                raise BenchmarkBlocked(f"pinned archive contains an unsafe path: {member.name}")
        stream.extractall(destination)


def run_checked(command: list[str], *, cwd: Path, label: str) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, cwd=cwd, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        detail = (result.stderr.strip() or result.stdout.strip() or "no output").strip()
        raise BenchmarkBlocked(f"{label} failed with exit {result.returncode}: {detail}")
    return result


def parse_json_output(output: str, label: str) -> dict[str, Any]:
    try:
        value = json.loads(output.strip())
    except json.JSONDecodeError:
        value = None
    if isinstance(value, dict):
        return value

    decoder = json.JSONDecoder()
    text = output.strip()
    for offset, character in enumerate(text):
        if character != "{":
            continue
        try:
            value, _end = decoder.raw_decode(text[offset:])
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    for line in reversed(output.splitlines()):
        candidate = line.strip()
        if not candidate.startswith("{"):
            continue
        try:
            value = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    raise BenchmarkBlocked(f"{label} did not emit a machine-readable JSON object")


def run_benchmark(source: Path, spec_path: Path, node_modules: Path) -> dict[str, Any]:
    source_info = verify_pinned_source(source)
    spec_path = spec_path.expanduser().resolve()
    if not spec_path.is_file():
        raise BenchmarkBlocked(f"benchmark spec is missing: {spec_path}")
    try:
        spec_bytes = spec_path.read_bytes()
        spec = json.loads(spec_bytes.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BenchmarkBlocked(f"benchmark spec is unreadable JSON: {error}") from error
    if not isinstance(spec, dict):
        raise BenchmarkBlocked("benchmark spec must be a JSON object")

    node_modules = node_modules.expanduser().resolve()
    if not (node_modules / "three" / "package.json").is_file():
        raise BenchmarkBlocked(
            "existing Three.js runtime is unavailable at "
            f"{node_modules}; install is intentionally forbidden for this benchmark"
        )

    with tempfile.TemporaryDirectory(prefix="weaponry-img2threejs-baseline-") as temporary:
        isolated = Path(temporary)
        pinned = isolated / "source"
        pinned.mkdir()
        extract_pinned_source(source, pinned)
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
            raise BenchmarkBlocked(f"pinned validator rejected the benchmark spec: {validation.stdout.strip()}")
        validation_warnings = validation_payload.get("warnings", [])
        warning_count = len(validation_warnings) if isinstance(validation_warnings, list) else 0

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

        (isolated / "node_modules").symlink_to(node_modules, target_is_directory=True)
        node_environment = os.environ.copy()
        node_environment["WPN_GENERATED_FACTORY_URL"] = generated.as_uri()
        node_version = run_checked(
            ["node", "--version"], cwd=isolated, label="Node.js availability probe"
        ).stdout.strip()
        execution = subprocess.run(
            ["node", "--experimental-strip-types", "--input-type=module", "-e", NODE_PROBE],
            cwd=isolated,
            check=False,
            capture_output=True,
            text=True,
            env=node_environment,
        )
        if execution.returncode != 0:
            detail = (execution.stderr.strip() or execution.stdout.strip() or "no output").strip()
            raise BenchmarkBlocked(f"generated factory execution failed with exit {execution.returncode}: {detail}")
        execution_payload = parse_json_output(execution.stdout, "generated factory execution")

        parts = execution_payload.get("parts")
        if not isinstance(parts, list) or not all(isinstance(item, dict) for item in parts):
            raise BenchmarkBlocked("generated factory execution did not return a parts list")
        expected_ids = [
            item.get("id")
            for item in spec.get("componentTree", [])
            if isinstance(item, dict) and item.get("level", "macro") == "macro"
        ]
        actual_ids = [item.get("id") for item in parts]
        if actual_ids != expected_ids:
            raise BenchmarkBlocked(
                "generated factory part order/coverage differs from the macro fixture: "
                f"expected={expected_ids} actual={actual_ids}"
            )

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
            "generation": {
                "generator": "forge/stage3_build/generate_threejs_factory.py",
                "mode": "allow-nonstrict-test-fixture",
                "normal_validation": "PASS",
                "normal_validation_warning_count": warning_count,
                "strict_quality": "BYPASSED_FOR_FIXTURE",
                "factory_sha256": sha256_file(generated),
                "factory_bytes": generated.stat().st_size,
            },
            "execution": {
                "mode": "isolated-generated-factory-node-smoke",
                "node_version": node_version,
                "mesh_count": execution_payload.get("meshCount"),
                "triangles": execution_payload.get("triangles"),
                "parts": parts,
                "network_used": False,
                "dependencies_installed": False,
                "product_runtime_invoked": False,
                "runtime_store_cas_write": False,
            },
        }


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-checkout", type=Path, required=True)
    parser.add_argument("--spec", type=Path, default=DEFAULT_SPEC)
    parser.add_argument("--node-modules", type=Path, default=REPOSITORY_ROOT / "node_modules")
    parser.add_argument("--receipt", type=Path, default=DEFAULT_RECEIPT)
    parser.add_argument("--force", action="store_true", help="overwrite an existing receipt")
    args = parser.parse_args(argv)
    receipt_path = args.receipt.expanduser().resolve()
    if receipt_path != PACKAGE_ROOT and PACKAGE_ROOT not in receipt_path.parents:
        print("BLOCKED: receipt must remain inside packages/weaponry-threejs", file=sys.stderr)
        return 2
    if receipt_path.exists() and not args.force:
        print(f"BLOCKED: receipt already exists: {receipt_path}; use --force to refresh", file=sys.stderr)
        return 2
    try:
        result = run_benchmark(args.source_checkout, args.spec, args.node_modules)
    except (BenchmarkBlocked, OSError, ValueError) as error:
        print(f"BLOCKED: {error}", file=sys.stderr)
        return 2

    receipt = {
        "schema_version": "WeaponryThreeJsImg2ThreeJsBaselineReceipt@1",
        "task_id": "WPN-THREE-BENCH-001",
        "benchmark_only": True,
        "status": "PASS_STRUCTURAL_BASELINE",
        "quality_status": "NOT_RUN",
        "visual_superiority": "NOT_COMPUTED",
        "upstream_execution_scope": "isolated-temporary-benchmark-only",
        "upstream_generator_executed": True,
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
