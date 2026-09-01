#!/usr/bin/env python3
"""Stage the fixed Weaponry Three.js worker as a relocatable app resource.

The source-tree worker is useful for development, but it is not an application
resource: it imports TypeScript from the repository and relies on the caller's
Node installation.  This script creates the exact resource tree consumed by a
Tauri bundle.  On macOS it also copies Node's non-system dylib closure and
rewrites those references to loader-relative names.  No binary is checked into
the repository; the generated tree is deliberately placed below the ignored
Tauri target directory.

The resulting manifest is fail-closed.  A null build cohort is allowed only for
source inspection.  Release/package callers must pass --require-cohort so a
source-only staging cannot be mistaken for a packaged build.

Preview is also fail-closed for release packaging.  A packaged build must
provide WEAPONRY_BROWSER_BUNDLE (a complete, redistributable browser bundle),
WEAPONRY_BROWSER_EXECUTABLE_RELATIVE (unless the bundle is a Chrome .app), and
WEAPONRY_BROWSER_LICENSE.  The host browser is never silently copied or
treated as packaged; the bundle is copied only when the caller explicitly
provides it.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Iterable


ROOT = Path(__file__).resolve().parents[1]
PACKAGE_ROOT = ROOT / "packages" / "weaponry-threejs"
SOURCE_ROOT = PACKAGE_ROOT / "src"
FIXED_WORKER = PACKAGE_ROOT / "scripts" / "fixed-worker.mjs"
BROWSER_PREVIEW_WORKER = PACKAGE_ROOT / "scripts" / "browser-preview-worker.mjs"
PREVIEW_ENTRY = PACKAGE_ROOT / "preview" / "worker-main.ts"
THREE_ROOT = ROOT / "node_modules" / "three"
DEFAULT_OUTPUT = ROOT / "apps" / "desktop" / "src-tauri" / "target" / "weaponry-threejs-worker"
MANIFEST_NAME = "weaponry-threejs-worker-manifest.json"
MANIFEST_SCHEMA = "WeaponryThreeJsPackagedWorkerManifest@1"
WORKER_ID = "weaponry-threejs-fixed-knife-worker@1"
WORKER_REQUEST_SCHEMA = "WeaponryThreeJsFixedWorkerRequest@1"
THREE_VERSION = "0.185.1"
DEFAULT_CHROME_EXECUTABLE_RELATIVE = "Contents/MacOS/Google Chrome"


def die(message: str) -> "NoReturn":
    raise SystemExit(f"WEAPONRY_THREEJS_PACKAGE_UNAVAILABLE: {message}")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode(
        "utf-8"
    )


def tree_entries(root: Path, *, exclude: Iterable[str] = ()) -> list[tuple[str, Path]]:
    excluded = set(exclude)
    if not root.is_dir():
        die(f"resource directory is missing: {root}")
    entries: list[tuple[str, Path]] = []
    for path in sorted((item for item in root.rglob("*") if item.is_file()), key=lambda item: item.as_posix()):
        relative = path.relative_to(root).as_posix()
        if relative not in excluded:
            entries.append((relative, path))
    return entries


def tree_hash(root: Path, *, exclude: Iterable[str] = ()) -> str:
    digest = hashlib.sha256()
    for relative, path in tree_entries(root, exclude=exclude):
        payload = path.read_bytes()
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(str(len(payload)).encode("ascii"))
        digest.update(b"\0")
        digest.update(payload)
        digest.update(b"\0")
    return digest.hexdigest()


def copy_file(source: Path, destination: Path) -> None:
    if not source.is_file():
        die(f"required resource is missing: {source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def copy_worker_sources(staging: Path) -> set[str]:
    worker = staging / "worker"
    (worker / "scripts").mkdir(parents=True, exist_ok=True)
    # Keep the package-relative layout: fixed-worker.mjs imports ../src/index.ts.
    copy_file(FIXED_WORKER, worker / "scripts" / "fixed-worker.mjs")
    copy_file(BROWSER_PREVIEW_WORKER, worker / "scripts" / "browser-preview-worker.mjs")
    shutil.copytree(SOURCE_ROOT, worker / "src", symlinks=False)
    copy_file(PREVIEW_ENTRY, worker / "preview" / "worker-main.ts")
    if not THREE_ROOT.is_dir():
        die("node_modules/three is unavailable; run the locked dependency install before staging")
    package = json.loads((THREE_ROOT / "package.json").read_text(encoding="utf-8"))
    if package.get("version") != THREE_VERSION:
        die(f"Three.js version drifted: expected {THREE_VERSION}, got {package.get('version')!r}")
    shutil.copytree(THREE_ROOT, worker / "node_modules" / "three", symlinks=False)
    npm_packages: set[str] = set()
    copy_node_package("vite", worker / "node_modules", npm_packages)
    return npm_packages


def package_source(name: str) -> Path:
    candidate = ROOT / "node_modules" / Path(*name.split("/"))
    if not candidate.is_dir():
        die(f"locked npm dependency is missing: {name}")
    return candidate


def copy_node_package(name: str, destination_root: Path, seen: set[str]) -> None:
    if name in seen or name == "three":
        return
    # Optional packages are selected for the current platform.  Copying every
    # esbuild/Rollup target would add unrelated architectures to the app.
    if name.startswith("@esbuild/") and name != "@esbuild/darwin-arm64":
        return
    if name.startswith("@rollup/rollup-") and name != "@rollup/rollup-darwin-arm64":
        return
    if name == "fsevents" and sys.platform != "darwin":
        return
    source = package_source(name)
    package = json.loads((source / "package.json").read_text(encoding="utf-8"))
    seen.add(name)
    destination = destination_root / Path(*name.split("/"))
    shutil.copytree(source, destination, symlinks=False)
    for dependency in sorted({*package.get("dependencies", {}), *package.get("optionalDependencies", {})}):
        copy_node_package(dependency, destination_root, seen)


def browser_bundle_input() -> tuple[Path, str, Path] | None:
    configured = os.environ.get("WEAPONRY_BROWSER_BUNDLE")
    if not configured:
        return None
    source = Path(configured).expanduser()
    try:
        source = source.resolve(strict=True)
    except FileNotFoundError:
        die(f"WEAPONRY_BROWSER_BUNDLE does not exist: {source}")
    if not source.is_dir():
        die("WEAPONRY_BROWSER_BUNDLE must be a complete directory bundle, not an executable")
    executable_relative = os.environ.get("WEAPONRY_BROWSER_EXECUTABLE_RELATIVE")
    if not executable_relative:
        if source.name.endswith(".app"):
            executable_relative = DEFAULT_CHROME_EXECUTABLE_RELATIVE
        else:
            die("WEAPONRY_BROWSER_EXECUTABLE_RELATIVE is required for a non-.app browser bundle")
    relative = Path(executable_relative)
    if relative.is_absolute() or ".." in relative.parts:
        die("WEAPONRY_BROWSER_EXECUTABLE_RELATIVE must stay inside the browser bundle")
    executable = source / relative
    if not executable.is_file() or not os.access(executable, os.X_OK):
        die(f"browser executable is missing or not executable: {executable}")
    license_value = os.environ.get("WEAPONRY_BROWSER_LICENSE")
    if not license_value:
        die("WEAPONRY_BROWSER_LICENSE is required when packaging a browser bundle")
    license_path = Path(license_value).expanduser()
    try:
        license_path = license_path.resolve(strict=True)
    except FileNotFoundError:
        die(f"WEAPONRY_BROWSER_LICENSE does not exist: {license_path}")
    if not license_path.is_file():
        die("WEAPONRY_BROWSER_LICENSE must be a license or notice file")
    return source, relative.as_posix(), license_path


def stage_browser_bundle(staging: Path) -> dict[str, object]:
    bundle = browser_bundle_input()
    if bundle is None:
        return {
            "status": "EXTERNAL_FIXED_BROWSER_REQUIRED",
            "browser_id": "google-chrome-headless@1",
            "packaged": False,
            "bundle_path": None,
            "executable_path": None,
            "bundle_sha256": None,
            "executable_sha256": None,
            "license_path": None,
            "license_sha256": None,
        }
    source, executable_relative, license_path = bundle
    destination = staging / "browser" / source.name
    # A bundle is an explicit distribution input.  Reject symlinks so the
    # resulting app resource cannot escape its sealed resource directory.
    for path in source.rglob("*"):
        if path.is_symlink():
            die(f"browser bundle contains unsupported symlink: {path}")
    shutil.copytree(source, destination, symlinks=False)
    executable = destination / executable_relative
    if not executable.is_file() or not os.access(executable, os.X_OK):
        die(f"staged browser executable is missing: {executable}")
    relative_bundle = f"browser/{source.name}"
    return {
        "status": "PACKAGED_FIXED_BROWSER",
        "browser_id": "google-chrome-headless@1",
        "packaged": True,
        "bundle_path": relative_bundle,
        "executable_path": f"{relative_bundle}/{executable_relative}",
        "bundle_sha256": tree_hash(destination),
        "executable_sha256": sha256_file(executable),
        "license_path": f"licenses/browser-{source.name}-{license_path.name}",
        "license_sha256": sha256_file(license_path),
        "license_source": license_path,
    }


def command_output(command: list[str]) -> str:
    try:
        completed = subprocess.run(command, check=True, text=True, capture_output=True)
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", None) or getattr(error, "stdout", None) or str(error)
        die(f"required tool failed: {' '.join(command)} ({detail.strip()})")
    return completed.stdout.strip()


def host_node() -> Path:
    configured = os.environ.get("WEAPONRY_NODE_RUNTIME")
    candidate_value = configured or shutil.which("node")
    if not candidate_value:
        die("Node 22+ executable is not available; set WEAPONRY_NODE_RUNTIME")
    candidate = Path(candidate_value).expanduser()
    try:
        candidate = candidate.resolve(strict=True)
    except FileNotFoundError:
        die(f"Node runtime does not exist: {candidate}")
    if not candidate.is_file() or not os.access(candidate, os.X_OK):
        die(f"Node runtime is not an executable file: {candidate}")
    version = command_output([str(candidate), "--version"])
    match = re.fullmatch(r"v(\d+)(?:\.\d+){1,2}", version)
    if not match or int(match.group(1)) < 22:
        die(f"Node 22+ is required for the fixed TypeScript worker, got {version!r}")
    return candidate


def otool_dependencies(path: Path) -> list[str]:
    output = command_output(["otool", "-L", str(path)])
    values: list[str] = []
    for line in output.splitlines()[1:]:
        match = re.match(r"\s+(.+?) \(compatibility version", line)
        if match:
            values.append(match.group(1))
    return values


def otool_rpaths(path: Path) -> list[str]:
    output = command_output(["otool", "-l", str(path)])
    values: list[str] = []
    lines = output.splitlines()
    for index, line in enumerate(lines):
        if "cmd LC_RPATH" in line:
            for candidate in lines[index + 1 : index + 4]:
                match = re.search(r"path (.+?) \(offset", candidate.strip())
                if match:
                    values.append(match.group(1))
                    break
    return values


def resolve_macos_dependency(raw: str, owner: Path, rpaths: list[str]) -> Path | None:
    if raw.startswith("/System/Library/") or raw.startswith("/usr/lib/"):
        return None
    candidates: list[Path] = []
    if raw.startswith("@rpath/"):
        suffix = raw.removeprefix("@rpath/")
        candidates.extend(Path(rpath.replace("@loader_path", str(owner.parent))) / suffix for rpath in rpaths)
    elif raw.startswith("@loader_path/"):
        candidates.append(owner.parent / raw.removeprefix("@loader_path/"))
    elif raw.startswith("/"):
        candidates.append(Path(raw))
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    die(f"cannot resolve non-system Node dependency {raw!r} from {owner}")


def stage_macos_node(staging: Path, node: Path) -> dict[str, object]:
    if sys.platform != "darwin":
        return {
            "status": "UNSUPPORTED_HOST_NOT_PACKAGED",
            "architecture": platform.machine(),
            "runtime_sha256": None,
            "dependencies": [],
        }
    if shutil.which("otool") is None or shutil.which("install_name_tool") is None:
        die("macOS otool and install_name_tool are required to make Node relocatable")
    architecture = command_output(["file", str(node)])
    expected_arch = "arm64" if platform.machine() in {"arm64", "aarch64"} else "x86_64"
    if expected_arch not in architecture:
        die(f"Node architecture does not match host ({expected_arch}): {architecture}")

    runtime = staging / "runtime"
    runtime.mkdir(parents=True, exist_ok=True)
    staged_node = runtime / "node"
    copy_file(node, staged_node)
    dependency_sources: dict[Path, str] = {}
    queue = [node]
    while queue:
        owner = queue.pop()
        rpaths = otool_rpaths(owner)
        for raw in otool_dependencies(owner):
            resolved = resolve_macos_dependency(raw, owner, rpaths)
            if resolved is None or resolved in dependency_sources:
                continue
            basename = resolved.name
            if basename in dependency_sources.values():
                die(f"Node dependency basename collision: {basename}")
            dependency_sources[resolved] = basename
            queue.append(resolved)

    lib_dir = runtime / "lib"
    lib_dir.mkdir(parents=True, exist_ok=True)
    for source, basename in dependency_sources.items():
        copy_file(source, lib_dir / basename)

    def target_name(owner: Path, resolved: Path) -> str:
        basename = resolved.name
        return f"@loader_path/lib/{basename}" if owner == node else f"@loader_path/{basename}"

    owners = [node, *dependency_sources.keys()]
    for owner in owners:
        owner_dependencies = otool_dependencies(owner)
        rpaths = otool_rpaths(owner)
        changes: list[str] = []
        for raw in owner_dependencies:
            resolved = resolve_macos_dependency(raw, owner, rpaths)
            if resolved is not None:
                changes.extend([raw, target_name(owner, resolved)])
        if changes:
            destination = staged_node if owner == node else lib_dir / dependency_sources[owner]
            command_output(["install_name_tool", *sum((["-change", changes[index], changes[index + 1]] for index in range(0, len(changes), 2)), []), str(destination)])
    for source, basename in dependency_sources.items():
        command_output(["install_name_tool", "-id", f"@loader_path/{basename}", str(lib_dir / basename)])

    # install_name_tool invalidates the original Mach-O signature.  Re-sign
    # every rewritten image before probing the copy; otherwise macOS may kill
    # the process with SIGKILL before it can report a useful loader error.
    if shutil.which("codesign") is None:
        die("codesign is required after making the bundled Node dylib closure relocatable")
    for source, basename in dependency_sources.items():
        command_output(
            [
                "codesign",
                "--force",
                "--sign",
                "-",
                "--timestamp=none",
                str(lib_dir / basename),
            ]
        )
    command_output(
        [
            "codesign",
            "--force",
            "--sign",
            "-",
            "--timestamp=none",
            str(staged_node),
        ]
    )

    # Resolve and execute the copied runtime once; this catches a missing
    # transitive dylib before the app is built.
    version = command_output([str(staged_node), "--version"])
    return {
        "status": "PACKAGED_RELOCATABLE",
        "architecture": expected_arch,
        "runtime_sha256": sha256_file(staged_node),
        "runtime_version": version,
        "dependency_sources": list(dependency_sources),
        "dependencies": [
            {"path": f"runtime/lib/{basename}", "sha256": sha256_file(lib_dir / basename)}
            for basename in sorted(dependency_sources.values())
        ],
    }


def license_files(source: Path, levels: int) -> list[Path]:
    parents = [source.parents[index] for index in range(min(levels, len(source.parents)))]
    for parent in parents:
        matches = sorted(path for path in parent.glob("LICENSE*") if path.is_file())
        if matches:
            return matches
    return []


def write_license_notices(
    staging: Path,
    node: Path,
    dependency_sources: Iterable[Path],
    npm_packages: Iterable[str],
    browser_info: dict[str, object],
) -> list[dict[str, str]]:
    notices: list[dict[str, str]] = []
    node_licenses = license_files(node, 4)
    for node_license in node_licenses:
        relative = f"licenses/node-{node_license.name}"
        copy_file(node_license, staging / relative)
        notices.append({"component": "Node.js", "path": relative, "sha256": sha256_file(staging / relative)})
    three_license = THREE_ROOT / "LICENSE"
    if three_license.is_file():
        copy_file(three_license, staging / "licenses" / "three.LICENSE")
        notices.append(
            {
                "component": f"three.js@{THREE_VERSION}",
                "path": "licenses/three.LICENSE",
                "sha256": sha256_file(staging / "licenses" / "three.LICENSE"),
            }
        )
    if not node_licenses or not three_license.is_file():
        die("Node.js and Three.js license notices are required for packaged resources")
    seen_dependency_licenses: set[str] = set()
    for source in dependency_sources:
        dependency_licenses = license_files(source, 5)
        if not dependency_licenses:
            die(f"license notice is missing for non-system Node dependency: {source.name}")
        for license_path in dependency_licenses:
            license_hash = sha256_file(license_path)
            if license_hash in seen_dependency_licenses:
                continue
            seen_dependency_licenses.add(license_hash)
            relative = f"licenses/dependency-{source.name}-{license_path.name}"
            copy_file(license_path, staging / relative)
            notices.append({"component": f"Node dependency {source.name}", "path": relative, "sha256": license_hash})
    seen_npm_licenses: set[str] = set()
    for name in sorted(npm_packages):
        source = package_source(name)
        package_licenses = sorted(path for path in source.glob("LICENSE*") if path.is_file())
        if not package_licenses:
            # Platform-only esbuild/Rollup packages carry the license in the
            # parent package rather than duplicating it in every binary shim.
            fallback_name = "esbuild" if name.startswith("@esbuild/") else "rollup" if name.startswith("@rollup/") else ""
            if fallback_name:
                package_licenses = sorted(path for path in package_source(fallback_name).glob("LICENSE*") if path.is_file())
            if not package_licenses:
                die(f"license notice is missing for npm package: {name}")
        safe_name = name.replace("/", "-").replace("@", "")
        for license_path in package_licenses:
            license_hash = sha256_file(license_path)
            if license_hash in seen_npm_licenses:
                continue
            seen_npm_licenses.add(license_hash)
            relative = f"licenses/npm-{safe_name}-{license_path.name}"
            copy_file(license_path, staging / relative)
            notices.append({"component": f"npm {name}", "path": relative, "sha256": license_hash})
    browser_license = browser_info.get("license_source")
    if browser_license is not None:
        relative = str(browser_info["license_path"])
        copy_file(Path(browser_license), staging / relative)
        notices.append(
            {
                "component": f"browser {browser_info['browser_id']}",
                "path": relative,
                "sha256": sha256_file(staging / relative),
            }
        )
    return notices


def git_metadata() -> tuple[str, bool]:
    try:
        revision = command_output(["git", "rev-parse", "HEAD"])
        dirty = bool(command_output(["git", "status", "--porcelain", "--", "packages/weaponry-threejs", "scripts/stage_weaponry_threejs_worker.py"]))
    except SystemExit:
        revision, dirty = "UNKNOWN", True
    return revision, dirty


def output_path(value: str) -> Path:
    path = (ROOT / value).resolve() if not Path(value).is_absolute() else Path(value).resolve()
    target_root = (ROOT / "apps" / "desktop" / "src-tauri" / "target").resolve()
    try:
        path.relative_to(target_root)
    except ValueError as error:
        die(f"output must remain below the ignored Tauri target directory: {path}")
    if path == target_root:
        die("output cannot be the entire Tauri target directory")
    return path


def stage(args: argparse.Namespace) -> int:
    output = output_path(args.output)
    cohort = os.environ.get("FORGECAD_BUILD_COHORT_SHA256")
    browser_configured = bool(os.environ.get("WEAPONRY_BROWSER_BUNDLE"))
    if args.require_cohort and (not cohort or not re.fullmatch(r"[a-f0-9]{64}", cohort)):
        die("--require-cohort needs FORGECAD_BUILD_COHORT_SHA256 as a lowercase SHA-256")
    if args.require_cohort and not browser_configured:
        die("--require-cohort needs WEAPONRY_BROWSER_BUNDLE for packaged eight-view preview")
    node = host_node()
    target_parent = output.parent
    target_parent.mkdir(parents=True, exist_ok=True)
    temp_root = Path(tempfile.mkdtemp(prefix=".weaponry-threejs-worker-", dir=target_parent))
    staging = temp_root / output.name
    try:
        npm_packages = copy_worker_sources(staging)
        node_info = stage_macos_node(staging, node)
        if node_info["status"] != "PACKAGED_RELOCATABLE":
            die("relocatable Node packaging is currently implemented only for macOS; refusing an unbundled system runtime")
        browser_info = stage_browser_bundle(staging)
        if args.require_cohort and browser_info["status"] != "PACKAGED_FIXED_BROWSER":
            die("packaged preview browser is unavailable")
        notices = write_license_notices(
            staging,
            node,
            node_info.get("dependency_sources", []),
            npm_packages,
            browser_info,
        )
        revision, dirty = git_metadata()
        worker_hash = tree_hash(staging / "worker")
        dependency_hash = tree_hash(staging / "worker" / "node_modules" / "three")
        runtime_hash = tree_hash(staging / "runtime") if (staging / "runtime").is_dir() else None
        package_status = (
            "SOURCE_ONLY_STAGED_UNCOHORTED"
            if not cohort
            else "PACKAGED_RELOCATABLE"
            if browser_info["packaged"]
            else "PACKAGED_WORKER_BROWSER_EXTERNAL"
        )
        manifest: dict[str, object] = {
            "schema_version": MANIFEST_SCHEMA,
            "status": package_status,
            "worker_id": WORKER_ID,
            "request_schema": WORKER_REQUEST_SCHEMA,
            "worker_entry": "worker/scripts/fixed-worker.mjs",
            "runtime_entry": "runtime/node" if node_info["status"] == "PACKAGED_RELOCATABLE" else None,
            "runtime_invocation": ["runtime/node", "--experimental-strip-types", "worker/scripts/fixed-worker.mjs"],
            "preview_entry": "worker/preview/worker-main.ts",
            "preview_runtime": {
                key: value
                for key, value in browser_info.items()
                if key != "license_source"
            },
            "worker_source_tree_sha256": worker_hash,
            "three_dependency_tree_sha256": dependency_hash,
            "runtime_tree_sha256": runtime_hash,
            "runtime_sha256": node_info["runtime_sha256"],
            "runtime_version": node_info.get("runtime_version"),
            "runtime_architecture": node_info.get("architecture"),
            "runtime_dependencies": node_info["dependencies"],
            "three_version": THREE_VERSION,
            "vite_version": json.loads((package_source("vite") / "package.json").read_text(encoding="utf-8"))["version"],
            "npm_packages": sorted(npm_packages),
            "npm_dependency_tree_sha256": tree_hash(staging / "worker" / "node_modules"),
            "package_lock_sha256": sha256_file(ROOT / "package-lock.json"),
            "license_notices": notices,
            "build_cohort_sha256": cohort,
            "source_revision": revision,
            "source_worktree_dirty": dirty,
            "resource_tree_sha256": None,
            "manifest_sha256": "",
        }
        manifest["resource_tree_sha256"] = tree_hash(staging)
        manifest["manifest_sha256"] = sha256_bytes(canonical_json(manifest))
        (staging / MANIFEST_NAME).write_text(json.dumps(manifest, indent=2, ensure_ascii=False, sort_keys=True) + "\n", encoding="utf-8")
        if output.exists():
            marker = output / MANIFEST_NAME
            if not marker.is_file():
                die(f"refusing to replace an unrecognized output directory: {output}")
            shutil.rmtree(output)
        staging.rename(output)
    finally:
        if temp_root.exists():
            shutil.rmtree(temp_root)
    print(json.dumps({"status": manifest["status"], "output": str(output), "manifest": str(output / MANIFEST_NAME), "manifest_sha256": manifest["manifest_sha256"]}, sort_keys=True))
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT), help="generated resource directory below apps/desktop/src-tauri/target")
    parser.add_argument("--require-cohort", action="store_true", help="fail unless FORGECAD_BUILD_COHORT_SHA256 is set")
    return parser.parse_args()


if __name__ == "__main__":
    raise SystemExit(stage(parse_args()))
