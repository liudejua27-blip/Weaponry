#!/usr/bin/env python3
"""Build, ad-hoc sign and atomically install the MCP010A development App.

The installer is intentionally local-only. It never reads a signing identity,
contacts a network service, changes the Codex configuration, or touches the
user's persistent ForgeCAD Runtime database.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TAURI_ROOT = ROOT / "apps" / "desktop" / "src-tauri"
DEV_BUNDLE = TAURI_ROOT / "target" / "release" / "bundle" / "macos" / "ForgeCAD Runtime Dev.app"
INSTALL_ROOT = Path.home() / "Applications"
INSTALL_TARGET = INSTALL_ROOT / "ForgeCAD Runtime Dev.app"
EVIDENCE = ROOT / "docs" / "evidence" / "mcp010a" / "dev-app-install.json"
EXCLUDED_PARTS = {".git", "target", "node_modules", "dist", "__pycache__"}
SOURCE_INPUTS = (
    ROOT / "apps" / "desktop" / "src",
    ROOT / "apps" / "desktop" / "package.json",
    ROOT / "apps" / "desktop" / "src-tauri",
    ROOT / "apps" / "geometry-worker" / "Cargo.toml",
    ROOT / "apps" / "geometry-worker" / "Cargo.lock",
    ROOT / "apps" / "geometry-worker" / "src",
    ROOT / "apps" / "render-worker" / "Cargo.toml",
    ROOT / "apps" / "render-worker" / "Cargo.lock",
    ROOT / "apps" / "render-worker" / "src",
    ROOT / "packages" / "forgecad-contracts",
    ROOT / "packages" / "forgecad-skills",
    ROOT / "package.json",
    ROOT / "package-lock.json",
    ROOT / "script" / "with_rust_toolchain.sh",
)
LOCKFILES = (
    ROOT / "package-lock.json",
    TAURI_ROOT / "Cargo.lock",
    ROOT / "apps" / "geometry-worker" / "Cargo.lock",
    ROOT / "apps" / "render-worker" / "Cargo.lock",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--keep-previous",
        action="store_true",
        help="Keep a timestamped previous Dev.app after a successful replacement.",
    )
    return parser.parse_args()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def source_files() -> list[Path]:
    files: set[Path] = set()
    for source in SOURCE_INPUTS:
        if not source.exists():
            continue
        candidates = [source] if source.is_file() else source.rglob("*")
        for candidate in candidates:
            if any(part in EXCLUDED_PARTS for part in candidate.parts):
                continue
            if candidate.is_symlink():
                raise SystemExit(f"source cohort refuses symlink: {candidate.relative_to(ROOT)}")
            if candidate.is_file():
                files.add(candidate)
    return sorted(files, key=lambda path: path.relative_to(ROOT).as_posix())


def source_cohort() -> tuple[str, int]:
    digest = hashlib.sha256()
    files = source_files()
    for path in files:
        relative = path.relative_to(ROOT).as_posix().encode("utf-8")
        digest.update(len(relative).to_bytes(4, "big"))
        digest.update(relative)
        digest.update(bytes.fromhex(sha256_file(path)))
    return digest.hexdigest(), len(files)


def lock_hashes() -> dict[str, str]:
    return {
        path.relative_to(ROOT).as_posix(): sha256_file(path)
        for path in LOCKFILES
        if path.is_file()
    }


def run(command: list[str], *, environment: dict[str, str]) -> None:
    subprocess.run(command, cwd=ROOT, env=environment, check=True)


def git_value(*arguments: str) -> str:
    completed = subprocess.run(
        ["git", *arguments], cwd=ROOT, text=True, capture_output=True, check=True
    )
    return completed.stdout.strip()


def assert_safe_generated_bundle(path: Path) -> None:
    expected_parent = (TAURI_ROOT / "target" / "release" / "bundle" / "macos").resolve()
    if path.name != "ForgeCAD Runtime Dev.app" or path.parent.resolve() != expected_parent:
        raise SystemExit("refusing to replace an unexpected build artifact")


def component_paths(app: Path) -> dict[str, Path]:
    return {
        "forgecad-mcp": app / "Contents" / "Resources" / "forgecad-mcp",
        "forgecad-runtime": app / "Contents" / "Resources" / "forgecad-runtime",
        "forgecad-geometry-worker": app
        / "Contents"
        / "Resources"
        / "forgecad-geometry-worker",
        "forgecad-viewer": app / "Contents" / "MacOS" / "forgecad-desktop",
    }


def require_layout(app: Path) -> dict[str, Path]:
    if not app.is_dir():
        raise SystemExit("Tauri did not produce ForgeCAD Runtime Dev.app")
    paths = component_paths(app)
    for name, path in paths.items():
        if not path.is_file() or not os.access(path, os.X_OK):
            raise SystemExit(f"development bundle is missing executable {name}")
    resources = app / "Contents" / "Resources"
    if (resources / "forgecad-mcp-host").exists():
        raise SystemExit("obsolete forgecad-mcp-host leaked into the development bundle")
    return paths


def build_identity(executable: Path) -> dict[str, object]:
    completed = subprocess.run(
        [str(executable), "--build-identity"],
        text=True,
        capture_output=True,
        timeout=20,
        check=True,
    )
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise SystemExit("component build identity was not an object")
    return value


def sign_staging_app(app: Path, cohort: str, source_revision: str, dirty: bool, file_count: int, locks: dict[str, str]) -> dict[str, str]:
    paths = require_layout(app)
    sign_environment = os.environ.copy()
    for name in ("forgecad-mcp", "forgecad-runtime", "forgecad-geometry-worker"):
        run(
            [
                "codesign",
                "--force",
                "--sign",
                "-",
                "--timestamp=none",
                "--options",
                "runtime",
                str(paths[name]),
            ],
            environment=sign_environment,
        )
    resource_hashes = {
        name: sha256_file(paths[name])
        for name in ("forgecad-mcp", "forgecad-runtime", "forgecad-geometry-worker")
    }
    manifest = {
        "schema_version": "ForgeCADDevBuildManifest@1",
        "build_cohort_sha256": cohort,
        "source_revision": source_revision,
        "source_worktree_dirty": dirty,
        "source_file_count": file_count,
        "lockfile_sha256": locks,
        "resource_sha256": resource_hashes,
        "components": {
            "forgecad-mcp": "packaged",
            "forgecad-runtime": "packaged",
            "forgecad-viewer": "packaged",
            "geometry-worker": "packaged same-cohort executable; Runtime still uses the linked product-owned crate until MCP010D",
            "render-worker": "unavailable until MCP010C",
        },
        "distribution_profile": "local-ad-hoc-development-only",
    }
    manifest_path = app / "Contents" / "Resources" / "forgecad-dev-build-manifest.json"
    manifest_path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    run(
        [
            "codesign",
            "--force",
            "--sign",
            "-",
            "--timestamp=none",
            "--options",
            "runtime",
            str(app),
        ],
        environment=sign_environment,
    )
    run(
        ["codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app)],
        environment=sign_environment,
    )
    # Outer bundle signing must not have rewritten the already-signed resource binaries.
    for name, expected in resource_hashes.items():
        if sha256_file(paths[name]) != expected:
            raise SystemExit(f"outer signing changed sealed resource {name}")
    identities = {name: build_identity(path) for name, path in paths.items()}
    for name, identity in identities.items():
        if identity.get("build_cohort_sha256") != cohort:
            raise SystemExit(f"{name} does not belong to the current build cohort")
    return resource_hashes


def install_atomically(staging_app: Path, keep_previous: bool) -> Path | None:
    INSTALL_ROOT.mkdir(parents=True, exist_ok=True)
    if INSTALL_TARGET.parent.resolve() != INSTALL_ROOT.resolve() or INSTALL_TARGET.name != "ForgeCAD Runtime Dev.app":
        raise SystemExit("refusing to install outside the fixed user Applications target")
    backup: Path | None = None
    if INSTALL_TARGET.exists():
        backup = INSTALL_ROOT / f"ForgeCAD Runtime Dev.app.backup-{int(time.time())}"
        if backup.exists():
            raise SystemExit("development App backup target already exists")
        INSTALL_TARGET.rename(backup)
    try:
        staging_app.rename(INSTALL_TARGET)
    except Exception:
        if backup is not None and not INSTALL_TARGET.exists():
            backup.rename(INSTALL_TARGET)
        raise
    if backup is not None and not keep_previous:
        shutil.rmtree(backup)
        backup = None
    return backup


def main() -> int:
    args = parse_args()
    cohort, file_count = source_cohort()
    source_revision = git_value("rev-parse", "HEAD")
    dirty = bool(git_value("status", "--porcelain", "--", *[str(path.relative_to(ROOT)) for path in SOURCE_INPUTS]))
    locks_before = lock_hashes()
    environment = os.environ.copy()
    environment["FORGECAD_BUILD_COHORT_SHA256"] = cohort
    environment["CARGO_NET_OFFLINE"] = "true"
    environment["CI"] = "true"

    assert_safe_generated_bundle(DEV_BUNDLE)
    if DEV_BUNDLE.exists():
        shutil.rmtree(DEV_BUNDLE)

    run(
        [
            str(ROOT / "script" / "with_rust_toolchain.sh"),
            "cargo",
            "build",
            "--manifest-path",
            str(TAURI_ROOT / "Cargo.toml"),
            "--release",
            "--locked",
            "--offline",
            "-p",
            "forgecad-runtime",
            "--bin",
            "forgecad-runtime",
            "-p",
            "forgecad-mcp",
            "--bin",
            "forgecad-mcp",
        ],
        environment=environment,
    )
    run(
        [
            str(ROOT / "script" / "with_rust_toolchain.sh"),
            "cargo",
            "build",
            "--manifest-path",
            str(ROOT / "apps" / "geometry-worker" / "Cargo.toml"),
            "--release",
            "--locked",
            "--offline",
            "--target-dir",
            str(TAURI_ROOT / "target"),
            "--bin",
            "forgecad-geometry-worker",
        ],
        environment=environment,
    )
    run(
        [
            str(ROOT / "script" / "with_rust_toolchain.sh"),
            "npm",
            "--workspace",
            "apps/desktop",
            "run",
            "tauri",
            "--",
            "build",
            "--bundles",
            "app",
            "--no-sign",
            "--ci",
            "--config",
            "src-tauri/tauri.dev.conf.json",
            "--",
            "--locked",
            "--offline",
        ],
        environment=environment,
    )
    if lock_hashes() != locks_before:
        raise SystemExit("a lockfile changed during the offline development build")
    require_layout(DEV_BUNDLE)

    INSTALL_ROOT.mkdir(parents=True, exist_ok=True)
    staging_root = Path(
        tempfile.mkdtemp(prefix=".forgecad-runtime-dev-staging-", dir=INSTALL_ROOT)
    )
    staging_app = staging_root / "ForgeCAD Runtime Dev.app"
    backup: Path | None = None
    try:
        shutil.copytree(DEV_BUNDLE, staging_app, symlinks=True)
        resource_hashes = sign_staging_app(
            staging_app, cohort, source_revision, dirty, file_count, locks_before
        )
        backup = install_atomically(staging_app, args.keep_previous)
    finally:
        if staging_root.exists():
            shutil.rmtree(staging_root)

    receipt = {
        "schema_version": "ForgeCADMCP010ADevInstallReceipt@1",
        "task_id": "FGC-MCP010A",
        "status": "PASS",
        "installed_app": "$USER_APPLICATIONS/ForgeCAD Runtime Dev.app",
        "build_cohort_sha256": cohort,
        "source_revision": source_revision,
        "source_worktree_dirty": dirty,
        "source_file_count": file_count,
        "resource_sha256": resource_hashes,
        "viewer_sha256": sha256_file(component_paths(INSTALL_TARGET)["forgecad-viewer"]),
        "codesign": "ad-hoc deep strict PASS",
        "developer_id": "NOT_RUN",
        "notarization": "NOT_RUN",
        "previous_app_backup": "retained" if backup is not None else "none",
    }
    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
