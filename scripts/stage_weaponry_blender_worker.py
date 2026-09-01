#!/usr/bin/env python3
"""Stage and verify Weaponry's fixed Blender knife worker.

This is a development packaging tool, not a Runtime API.  The only external
input it accepts is a complete, explicitly selected ``Blender.app`` bundle;
the modeling job itself still receives a closed typed request and never gets
to choose a path, Python file, add-on, executable, URL, or environment value.

The checked-in worker manifest pins the sidecar identity and policy.  The
entrypoint digest is deliberately derived from the bytes on every staging
run, so changing the worker cannot leave a stale package manifest behind.
Generated resources stay below Tauri's ignored ``target`` directory and are
always marked development-only until GPL source-offer, NOTICE, SPDX SBOM,
legal review, and product distribution signing are complete.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import re
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Iterable, NoReturn


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "apps" / "blender-worker"
WORKER_SCRIPT = SOURCE / "weaponry_knife_worker.py"
WORKER_SOURCE_MANIFEST = SOURCE / "manifest.json"
DEFAULT_BLENDER_APP = (
    ROOT
    / "apps"
    / "desktop"
    / "src-tauri"
    / "target"
    / "weaponry-blender-runtime"
    / "Blender.app"
)
DEFAULT_OUTPUT = (
    ROOT
    / "apps"
    / "desktop"
    / "src-tauri"
    / "target"
    / "weaponry-blender-worker"
)

SOURCE_MANIFEST_SCHEMA = "WeaponryBlenderFixedWorkerManifest@1"
PACKAGED_MANIFEST_SCHEMA = "WeaponryBlenderPackagedWorkerManifest@1"
PACKAGED_MANIFEST_NAME = "weaponry-blender-worker-manifest.json"
COMPLIANCE_DIRECTORY = "compliance"
COMPLIANCE_ARTIFACT_NAMES = {
    "notice": "NOTICE",
    "spdx_sbom": "sbom.spdx.json",
    "gpl_source_offer": "GPL-SOURCE-OFFER.md",
}
COMPLIANCE_MANIFEST_NAME = "release-eligibility.json"
COMPLIANCE_SCHEMA = "WeaponryBlenderDistributionEligibility@1"
TAURI_RESOURCE_DESTINATION = "Resources/weaponry-blender-worker"
TAURI_RESOURCE_SOURCE = "target/weaponry-blender-worker"
TAURI_CONFIGS = (
    ROOT / "apps" / "desktop" / "src-tauri" / "tauri.conf.json",
    ROOT / "apps" / "desktop" / "src-tauri" / "tauri.dev.conf.json",
)
EXPECTED_WORKER_ID = "weaponry-blender-knife-worker@1"
EXPECTED_WORKER_VERSION = "0.1.0"
EXPECTED_PROTOCOL = "weaponry-fixed-worker-stdio-json@1"
EXPECTED_REQUEST_SCHEMA = "WeaponryBlenderKnifeWorkerRequest@1"
EXPECTED_RESPONSE_SCHEMA = "WeaponryBlenderKnifeWorkerResponse@1"
EXPECTED_RESULT_SCHEMA = "WeaponryBlenderKnifeWorkerResult@1"
EXPECTED_OPERATION = "knife_high_low_uv_bake@1"
EXPECTED_RECIPE_ID = "weaponry.knife.blender.high-low-uv-bake@1"
EXPECTED_INPUT_PATH = "input/source.glb"
EXPECTED_ENTRYPOINT_PATH = "apps/blender-worker/weaponry_knife_worker.py"
EXPECTED_BUNDLE_ID = "org.blenderfoundation.blender"
EXPECTED_LICENSE = "GPL-3.0-or-later"
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

REQUIRED_LICENSE_RESOURCES = {
    "copyright": "Contents/Resources/text/copyright.txt",
    "license": "Contents/Resources/text/license/license.md",
    "third_party_index": "Contents/Resources/text/license/licenses.json",
    "gpl_text": "Contents/Resources/text/license/spdx/GPL-3.0-or-later.txt",
}

REQUIRED_COMPLIANCE_MARKERS = {
    "notice": "runtime/Blender.app/Contents/Resources/text/license/license.md",
    "gpl_source_offer": "GPL-SOURCE-OFFER.md",
}

FIXED_LAUNCH_POLICY = {
    "network": "disabled",
    "filesystem": "runtime_scratch_only",
    "script": "frozen_bundle_only",
    "blender_autoexec": "disabled",
    "blender_factory_startup": "required",
    "user_python_environment": "disabled",
    "runtime_environment": "cleared_by_runtime_launcher",
    "runtime_write_performed": False,
}


def fail(message: str) -> NoReturn:
    raise SystemExit(f"WEAPONRY_BLENDER_PACKAGE_UNAVAILABLE: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def read_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"{label} is not valid UTF-8 JSON: {error}")
    if not isinstance(value, dict):
        fail(f"{label} must contain a JSON object")
    return value


def require_sha(value: object, label: str) -> str:
    require(
        isinstance(value, str) and SHA256_RE.fullmatch(value) is not None,
        f"{label} must be a lowercase SHA-256",
    )
    return value


def safe_relative(value: object, label: str) -> str:
    require(isinstance(value, str) and value != "", f"{label} must be a relative path")
    path = Path(value)
    require(
        not path.is_absolute() and ".." not in path.parts and path.as_posix() == value,
        f"{label} escapes its package",
    )
    return value


def tree_inventory(root: Path, *, excluded: Iterable[str] = ()) -> tuple[str, int, int]:
    """Hash a deterministic path/kind/size/content inventory.

    Symlinks are represented by their link target instead of being followed.
    The Blender bundle validator rejects symlinks; retaining this representation
    here also makes package verification fail closed if one appears later.
    """

    require(root.is_dir(), f"resource directory is missing: {root}")
    excluded_set = set(excluded)
    digest = hashlib.sha256()
    file_count = 0
    total_bytes = 0
    for path in sorted(root.rglob("*"), key=lambda item: item.as_posix()):
        relative = path.relative_to(root).as_posix()
        if relative in excluded_set or path.is_dir():
            continue
        if path.is_symlink():
            payload = os.readlink(path).encode("utf-8")
            kind = b"symlink"
            byte_size = len(payload)
        elif path.is_file():
            payload = None
            kind = b"file"
            byte_size = path.stat().st_size
            total_bytes += byte_size
        else:
            fail(f"resource tree contains an unsupported entry: {relative}")
        file_count += 1
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(kind)
        digest.update(b"\0")
        digest.update(str(byte_size).encode("ascii"))
        digest.update(b"\0")
        if payload is not None:
            digest.update(payload)
        else:
            with path.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(chunk)
        digest.update(b"\0")
    return digest.hexdigest(), file_count, total_bytes


def reject_symlinks(root: Path, label: str) -> None:
    links = sorted(
        path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_symlink()
    )
    require(not links, f"{label} contains unsupported symlink(s): {', '.join(links[:4])}")


def tree_sha256(root: Path, *, excluded: Iterable[str] = ()) -> str:
    return tree_inventory(root, excluded=excluded)[0]


def command_output(command: list[str]) -> str:
    try:
        completed = subprocess.run(
            command,
            check=True,
            text=True,
            capture_output=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", None) or getattr(error, "stdout", None) or str(error)
        fail(f"required command failed: {detail.strip()}")
    return completed.stdout.strip()


def host_from_source_manifest(source_manifest: dict[str, Any]) -> dict[str, Any]:
    require(
        source_manifest.get("schema_version") == SOURCE_MANIFEST_SCHEMA,
        "fixed Worker source manifest schema drifted",
    )
    require(
        source_manifest.get("status") == "isolated-prototype",
        "fixed Worker source manifest is not an isolated prototype",
    )
    require(
        source_manifest.get("entrypoint") == EXPECTED_ENTRYPOINT_PATH,
        "fixed Worker entrypoint path drifted",
    )
    require(
        source_manifest.get("entrypoint_sha256") is None,
        "source manifest entrypoint hash must be derived at staging, not pinned",
    )
    fixed_markers = {
        "worker_id": EXPECTED_WORKER_ID,
        "worker_version": EXPECTED_WORKER_VERSION,
        "protocol": EXPECTED_PROTOCOL,
        "request_schema": EXPECTED_REQUEST_SCHEMA,
        "response_schema": EXPECTED_RESPONSE_SCHEMA,
        "result_schema": EXPECTED_RESULT_SCHEMA,
        "operation": EXPECTED_OPERATION,
        "recipe_id": EXPECTED_RECIPE_ID,
    }
    for key, expected in fixed_markers.items():
        require(source_manifest.get(key) == expected, f"fixed Worker {key} drifted")
    transport = source_manifest.get("transport")
    require(isinstance(transport, dict), "fixed Worker transport metadata is missing")
    require(transport.get("network") is False, "fixed Worker transport must be offline")
    require(
        transport.get("input_relative_path") == EXPECTED_INPUT_PATH,
        "fixed Worker input path drifted",
    )
    require(
        transport.get("output_relative_directory") == "output",
        "fixed Worker output directory drifted",
    )
    output_policy = source_manifest.get("output_policy")
    require(isinstance(output_policy, dict), "fixed Worker output policy is missing")
    require(
        output_policy.get("runtime_write_performed") is False,
        "source Worker output policy permits Runtime writes",
    )
    require(
        source_manifest.get("runtime_integration") == "not_connected",
        "source Worker must remain disconnected from Runtime",
    )
    require(
        source_manifest.get("package_status") == "not_packaged",
        "source Worker manifest must remain source-only",
    )

    host = source_manifest.get("host")
    require(isinstance(host, dict), "source Worker host metadata is missing")
    for key in (
        "blender_version",
        "blender_display_version",
        "source_revision",
        "build_hash",
        "build_branch",
        "build_platform",
        "build_type",
        "bundle_id",
        "team_id",
        "signing_authority",
        "source_license",
        "binary_sha256",
        "bundle_tree_sha256",
        "python_bundle_sha256",
        "bundle_file_count",
        "bundle_total_bytes",
        "license_resources",
        "download_artifact",
    ):
        require(key in host, f"source Worker host metadata is missing {key}")
    require(host["bundle_id"] == EXPECTED_BUNDLE_ID, "Blender bundle identifier is not allowlisted")
    require(
        host["source_license"] == EXPECTED_LICENSE,
        "Blender source license must be GPL-3.0-or-later",
    )
    for key in ("binary_sha256", "bundle_tree_sha256", "python_bundle_sha256"):
        require_sha(host[key], f"host.{key}")
    require(
        isinstance(host["bundle_file_count"], int) and host["bundle_file_count"] > 0,
        "host.bundle_file_count is invalid",
    )
    require(
        isinstance(host["bundle_total_bytes"], int) and host["bundle_total_bytes"] > 0,
        "host.bundle_total_bytes is invalid",
    )
    license_resources = host["license_resources"]
    require(isinstance(license_resources, dict), "host.license_resources is invalid")
    for key, expected_path in REQUIRED_LICENSE_RESOURCES.items():
        entry = license_resources.get(key)
        require(isinstance(entry, dict), f"host.license_resources.{key} is missing")
        require(
            entry.get("path") == expected_path,
            f"host.license_resources.{key}.path drifted",
        )
        require_sha(entry.get("sha256"), f"host.license_resources.{key}.sha256")

    download = host["download_artifact"]
    require(isinstance(download, dict), "host.download_artifact is invalid")
    require(
        download.get("status") == "NOT_AVAILABLE_IN_WORKSPACE",
        "download artifact status is not truthful",
    )
    require(
        download.get("sha256") is None,
        "a missing download artifact cannot have a claimed hash",
    )

    distribution = source_manifest.get("distribution_gates")
    require(isinstance(distribution, dict), "source distribution gate metadata is missing")
    for key, expected in {
        "gpl_source_offer": "NOT_INCLUDED_DEVELOPMENT_STAGING",
        "notice": "NOT_INCLUDED_DEVELOPMENT_STAGING",
        "spdx_sbom": "NOT_INCLUDED_DEVELOPMENT_STAGING",
        "legal_review": "NOT_RUN",
        "release_eligible": False,
    }.items():
        require(
            distribution.get(key) == expected,
            f"source distribution gate {key} is not truthful",
        )

    fixed_recipe = source_manifest.get("fixed_recipe")
    require(isinstance(fixed_recipe, dict), "fixed Worker recipe metadata is missing")
    recipe_preimage = {
        "recipe_id": source_manifest["recipe_id"],
        "policy": source_manifest["policy"],
        "source": "staged_glb_only",
        **fixed_recipe,
    }
    require_sha(source_manifest.get("recipe_sha256"), "recipe_sha256")
    require(
        hashlib.sha256(canonical_bytes(recipe_preimage)).hexdigest()
        == source_manifest["recipe_sha256"],
        "fixed Worker recipe hash does not match its declared recipe",
    )
    dependency_preimage = {
        "blender_version": host["blender_version"],
        "blender_revision": host["source_revision"],
        "python_dependencies": [],
        "addons": [],
        "network": False,
    }
    require_sha(source_manifest.get("dependency_lock_sha256"), "dependency_lock_sha256")
    require(
        hashlib.sha256(canonical_bytes(dependency_preimage)).hexdigest()
        == source_manifest["dependency_lock_sha256"],
        "fixed Worker dependency lock hash does not match its declared lock",
    )
    return host


def blender_version_info(output: str, host: dict[str, Any]) -> None:
    lines = [line.strip() for line in output.splitlines() if line.strip()]
    require(
        lines and lines[0] == f"Blender {host['blender_display_version']}",
        "Blender display version drifted",
    )
    fields = {
        "build hash": host["build_hash"],
        "build branch": host.get("build_branch"),
        "build platform": host.get("build_platform"),
        "build type": host.get("build_type"),
    }
    for label, expected in fields.items():
        if expected is None:
            continue
        prefix = f"{label}:"
        values = [line.removeprefix(prefix).strip() for line in lines if line.startswith(prefix)]
        require(values == [expected], f"Blender {label} drifted")


def validate_blender_bundle(
    bundle: Path,
    host: dict[str, Any],
    *,
    verify_command: bool = True,
) -> dict[str, Any]:
    try:
        bundle = bundle.resolve(strict=True)
    except FileNotFoundError:
        fail(f"Blender bundle is missing: {bundle}")
    require(
        bundle.name == "Blender.app" and bundle.is_dir(),
        "Blender input must be the complete Blender.app bundle",
    )
    reject_symlinks(bundle, "Blender bundle")
    executable = bundle / "Contents" / "MacOS" / "Blender"
    info_path = bundle / "Contents" / "Info.plist"
    require(
        executable.is_file() and os.access(executable, os.X_OK),
        "Blender bundle executable is missing or not executable",
    )
    require(info_path.is_file(), "Blender bundle Info.plist is missing")
    with info_path.open("rb") as handle:
        try:
            info = plistlib.load(handle)
        except (OSError, plistlib.InvalidFileException, ValueError) as error:
            fail(f"Blender Info.plist is invalid: {error}")
    require(info.get("CFBundleIdentifier") == host["bundle_id"], "Blender bundle identifier drifted")
    require(info.get("CFBundleExecutable") == "Blender", "Blender executable metadata drifted")
    require(
        info.get("CFBundleShortVersionString") == host["blender_version"],
        "Blender Info.plist version drifted",
    )
    require(
        info.get("CFBundleVersion") == host["blender_version"],
        "Blender Info.plist build version drifted",
    )

    if verify_command:
        blender_version_info(command_output([str(executable), "--version"]), host)
    executable_hash = sha256_file(executable)
    require(executable_hash == host["binary_sha256"], "Blender executable hash drifted")

    signature_verification = subprocess.run(
        ["codesign", "--verify", "--deep", "--strict", str(bundle)],
        text=True,
        capture_output=True,
        check=False,
    )
    require(
        signature_verification.returncode == 0,
        "Blender Developer ID strict signature verification failed",
    )
    signature = subprocess.run(
        ["codesign", "-dv", "--verbose=4", str(bundle)],
        text=True,
        capture_output=True,
        check=False,
    )
    signature_text = f"{signature.stdout}\n{signature.stderr}"
    require(signature.returncode == 0, "Blender Developer ID signature is invalid")
    require(
        f"TeamIdentifier={host['team_id']}" in signature_text,
        "Blender signing team identifier drifted",
    )
    require(
        host["signing_authority"] in signature_text,
        "Blender signing authority drifted",
    )

    actual_license_resources: dict[str, dict[str, Any]] = {}
    for key, expected_path in REQUIRED_LICENSE_RESOURCES.items():
        entry = host["license_resources"][key]
        path = bundle / expected_path
        require(path.is_file(), f"Blender license resource is missing: {expected_path}")
        actual_hash = sha256_file(path)
        require(
            actual_hash == entry["sha256"],
            f"Blender license resource hash drifted: {expected_path}",
        )
        require(
            entry.get("byte_size") == path.stat().st_size,
            f"Blender license resource size drifted: {expected_path}",
        )
        actual_license_resources[key] = {
            "path": expected_path,
            "sha256": actual_hash,
            "byte_size": path.stat().st_size,
        }

    license_text = (
        bundle / REQUIRED_LICENSE_RESOURCES["license"]
    ).read_text(encoding="utf-8", errors="replace")
    gpl_text = (
        bundle / REQUIRED_LICENSE_RESOURCES["gpl_text"]
    ).read_text(encoding="utf-8", errors="replace")
    copyright_text = (
        bundle / REQUIRED_LICENSE_RESOURCES["copyright"]
    ).read_text(encoding="utf-8", errors="replace")
    license_index = read_json_object(
        bundle / REQUIRED_LICENSE_RESOURCES["third_party_index"],
        "Blender third-party license index",
    )
    require(
        "GPU-GPL 3.0 or later" in license_text
        and "GNU General Public License v3.0 or later" in license_text,
        "Blender GPL-3.0-or-later license text is missing",
    )
    require(
        "GNU GENERAL PUBLIC LICENSE" in gpl_text and "Version 3" in gpl_text,
        "Blender SPDX GPL-3.0-or-later text is invalid",
    )
    require("GNU GPL" in copyright_text, "Blender copyright resource does not state GPL")
    require(
        any("GPL-3.0-or-later" in str(key) for key in license_index),
        "Blender license index lacks GPL-3.0-or-later",
    )
    require(
        (bundle / "Contents" / "Resources" / "5.2" / "python").is_dir(),
        "Blender bundled Python runtime is missing",
    )

    bundle_hash, file_count, total_bytes = tree_inventory(bundle)
    require(bundle_hash == host["bundle_tree_sha256"], "Blender bundle tree hash drifted")
    require(file_count == host["bundle_file_count"], "Blender bundle file count drifted")
    require(total_bytes == host["bundle_total_bytes"], "Blender bundle byte inventory drifted")
    python_hash = tree_sha256(bundle / "Contents" / "Resources" / "5.2" / "python")
    require(python_hash == host["python_bundle_sha256"], "Blender Python bundle hash drifted")
    return {
        "executable": executable,
        "executable_sha256": executable_hash,
        "bundle_tree_sha256": bundle_hash,
        "bundle_file_count": file_count,
        "bundle_total_bytes": total_bytes,
        "python_bundle_sha256": python_hash,
        "license_resources": actual_license_resources,
        "signature": "PASS_VERIFIED",
    }


def normalized_source_manifest(
    source_manifest: dict[str, Any], entrypoint_hash: str
) -> dict[str, Any]:
    normalized = dict(source_manifest)
    normalized["entrypoint_sha256"] = entrypoint_hash
    normalized["entrypoint_hash_policy"] = "DERIVED_FROM_STAGED_ENTRYPOINT_BYTES"
    return normalized


def output_path(value: Path) -> Path:
    path = value.expanduser()
    path = (ROOT / path).resolve() if not path.is_absolute() else path.resolve()
    target_root = (ROOT / "apps" / "desktop" / "src-tauri" / "target").resolve()
    try:
        path.relative_to(target_root)
    except ValueError:
        fail(f"output must remain below the ignored Tauri target directory: {path}")
    require(path != target_root, "output cannot be the entire Tauri target directory")
    return path


def validate_tauri_resource_mapping() -> None:
    """Keep the generated resource reachable from both Tauri configurations."""

    for config_path in TAURI_CONFIGS:
        config = read_json_object(config_path, f"Tauri configuration {config_path.name}")
        bundle = config.get("bundle")
        require(isinstance(bundle, dict), f"Tauri configuration {config_path.name} has no bundle")
        macos = bundle.get("macOS")
        require(isinstance(macos, dict), f"Tauri configuration {config_path.name} has no macOS bundle")
        files = macos.get("files")
        require(isinstance(files, dict), f"Tauri configuration {config_path.name} has no macOS resource map")
        require(
            files.get(TAURI_RESOURCE_DESTINATION) == TAURI_RESOURCE_SOURCE,
            f"Tauri configuration {config_path.name} does not map {TAURI_RESOURCE_DESTINATION}",
        )


def package_host_metadata(manifest: dict[str, Any]) -> dict[str, Any]:
    blender = manifest.get("blender")
    require(isinstance(blender, dict), "packaged manifest Blender metadata is missing")
    return {
        "blender_version": blender.get("version"),
        "blender_display_version": blender.get("display_version"),
        "source_revision": blender.get("source_revision"),
        "build_hash": blender.get("build_hash"),
        "build_branch": blender.get("build_branch"),
        "build_platform": blender.get("build_platform"),
        "build_type": blender.get("build_type"),
        "bundle_id": blender.get("bundle_id"),
        "team_id": blender.get("team_id"),
        "signing_authority": blender.get("signing_authority"),
        "binary_sha256": blender.get("executable_sha256"),
        "bundle_tree_sha256": blender.get("bundle_tree_sha256"),
        "python_bundle_sha256": blender.get("python_bundle_sha256"),
        "bundle_file_count": blender.get("bundle_file_count"),
        "bundle_total_bytes": blender.get("bundle_total_bytes"),
        "license_resources": blender.get("license_resources"),
    }


def require_regular_file(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as error:
        fail(f"{label} is unavailable: {error}")
    require(path.is_file() and not path.is_symlink(), f"{label} must be a regular file")
    require(metadata.st_size > 0, f"{label} must not be empty")


def validate_spdx_document(
    value: dict[str, Any],
    *,
    host: dict[str, Any],
    entrypoint_hash: str,
) -> None:
    """Validate the small SPDX supplement without changing the Runtime wire.

    The Rust Worker manifest is an intentionally closed compatibility contract.
    This supplement therefore carries package compliance facts separately while
    still binding the two executable components to the same staged bytes.
    """

    require(value.get("SPDXID") == "SPDXRef-DOCUMENT", "Blender SPDX document identity drifted")
    require(value.get("spdxVersion") == "SPDX-2.3", "Blender SPDX document must use SPDX-2.3")
    require(value.get("dataLicense") == "CC0-1.0", "Blender SPDX data license drifted")
    creation = value.get("creationInfo")
    require(isinstance(creation, dict), "Blender SPDX creation info is missing")
    require(
        isinstance(creation.get("created"), str) and creation.get("created", "").endswith("Z"),
        "Blender SPDX creation timestamp is invalid",
    )
    creators = creation.get("creators")
    require(isinstance(creators, list) and creators, "Blender SPDX creators are missing")
    packages = value.get("packages")
    require(isinstance(packages, list), "Blender SPDX packages are missing")
    by_id = {
        package.get("SPDXID"): package
        for package in packages
        if isinstance(package, dict) and isinstance(package.get("SPDXID"), str)
    }
    worker = by_id.get("SPDXRef-Package-WeaponryBlenderWorker")
    require(isinstance(worker, dict), "Blender SPDX Worker package is missing")
    require(
        worker.get("name") == "weaponry-blender-knife-worker"
        and worker.get("versionInfo") == EXPECTED_WORKER_VERSION
        and worker.get("licenseDeclared") == "NOASSERTION"
        and worker.get("licenseConcluded") == "NOASSERTION",
        "Blender SPDX Worker package identity is unresolved or drifted",
    )
    worker_checksums = worker.get("checksums")
    require(isinstance(worker_checksums, list), "Blender SPDX Worker checksum is missing")
    require(
        any(
            isinstance(checksum, dict)
            and checksum.get("algorithm") == "SHA256"
            and checksum.get("checksumValue") == entrypoint_hash
            for checksum in worker_checksums
        ),
        "Blender SPDX Worker checksum does not bind the staged entrypoint",
    )

    blender = by_id.get("SPDXRef-Package-Blender-5.2.1")
    require(isinstance(blender, dict), "Blender SPDX sidecar package is missing")
    require(
        blender.get("name") == "Blender"
        and blender.get("versionInfo") == host["blender_display_version"]
        and blender.get("licenseDeclared") == EXPECTED_LICENSE
        and blender.get("licenseConcluded") == "NOASSERTION",
        "Blender SPDX sidecar identity is unresolved or drifted",
    )
    blender_checksums = blender.get("checksums")
    require(isinstance(blender_checksums, list), "Blender SPDX sidecar checksum is missing")
    require(
        any(
            isinstance(checksum, dict)
            and checksum.get("algorithm") == "SHA256"
            and checksum.get("checksumValue") == host["binary_sha256"]
            for checksum in blender_checksums
        ),
        "Blender SPDX sidecar checksum does not bind the staged executable",
    )
    external_refs = blender.get("externalRefs")
    require(isinstance(external_refs, list), "Blender SPDX build reference is missing")
    require(
        any(
            isinstance(reference, dict)
            and reference.get("referenceType") == "other"
            and reference.get("referenceLocator") == f"blender-build:{host['source_revision']}"
            for reference in external_refs
        ),
        "Blender SPDX build reference does not bind the staged revision",
    )
    document_describes = value.get("documentDescribes")
    require(
        isinstance(document_describes, list)
        and set(document_describes) == set(by_id),
        "Blender SPDX document descriptions do not match its packages",
    )
    relationships = value.get("relationships")
    require(isinstance(relationships, list), "Blender SPDX relationships are missing")
    require(
        any(
            isinstance(relationship, dict)
            and relationship.get("spdxElementId") == "SPDXRef-Package-WeaponryBlenderWorker"
            and relationship.get("relationshipType") == "DEPENDS_ON"
            and relationship.get("relatedSpdxElement") == "SPDXRef-Package-Blender-5.2.1"
            for relationship in relationships
        ),
        "Blender SPDX Worker dependency relationship is missing",
    )


def source_compliance_artifacts() -> dict[str, Path]:
    return {key: SOURCE / name for key, name in COMPLIANCE_ARTIFACT_NAMES.items()}


def validate_source_compliance(host: dict[str, Any], entrypoint_hash: str) -> None:
    artifacts = source_compliance_artifacts()
    for key, path in artifacts.items():
        require_regular_file(path, f"source compliance artifact {key}")
    notice = artifacts["notice"].read_text(encoding="utf-8")
    require(
        all(marker in notice for marker in REQUIRED_COMPLIANCE_MARKERS.values()),
        "source NOTICE does not identify the offline Blender license resources",
    )
    source_offer = artifacts["gpl_source_offer"].read_text(encoding="utf-8")
    require(
        host["blender_version"] in source_offer
        and host["source_revision"] in source_offer
        and host["binary_sha256"] in source_offer
        and host["bundle_tree_sha256"] in source_offer
        and "not a completed written offer" in source_offer.lower(),
        "source GPL acquisition record is incomplete or overclaims its status",
    )
    sbom = read_json_object(artifacts["spdx_sbom"], "source Blender SPDX supplement")
    validate_spdx_document(sbom, host=host, entrypoint_hash=entrypoint_hash)


def copy_source_compliance_artifacts(staging: Path) -> None:
    for key, source in source_compliance_artifacts().items():
        destination = staging / COMPLIANCE_DIRECTORY / COMPLIANCE_ARTIFACT_NAMES[key]
        require_regular_file(source, f"source compliance artifact {key}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)


def compliance_artifact_records(root: Path) -> dict[str, dict[str, Any]]:
    records: dict[str, dict[str, Any]] = {}
    for key, name in COMPLIANCE_ARTIFACT_NAMES.items():
        relative = f"{COMPLIANCE_DIRECTORY}/{name}"
        path = root / relative
        require_regular_file(path, f"packaged compliance artifact {key}")
        records[key] = {
            "path": relative,
            "sha256": sha256_file(path),
            "byte_size": path.stat().st_size,
        }
    return records


def write_compliance_manifest(
    staging: Path,
    *,
    host: dict[str, Any],
    entrypoint_hash: str,
) -> Path:
    records = compliance_artifact_records(staging)
    manifest: dict[str, Any] = {
        "schema_version": COMPLIANCE_SCHEMA,
        "status": "DEVELOPMENT_STAGED_NOT_RELEASE_ELIGIBLE",
        "scope": "supplemental_tauri_resource_compliance",
        "tauri_resource_root": "Resources/weaponry-blender-worker",
        "worker": {
            "worker_id": EXPECTED_WORKER_ID,
            "worker_version": EXPECTED_WORKER_VERSION,
            "entrypoint_sha256": entrypoint_hash,
        },
        "blender": {
            "version": host["blender_version"],
            "display_version": host["blender_display_version"],
            "source_revision": host["source_revision"],
            "source_license": host["source_license"],
            "executable_sha256": host["binary_sha256"],
            "bundle_tree_sha256": host["bundle_tree_sha256"],
            "bundle_signature": "PASS_VERIFIED",
            "download_artifact_status": host["download_artifact"]["status"],
        },
        "artifacts": records,
        "gates": {
            "offline_blender_resource": "PASS_INCLUDED",
            "blender_bundle_signature": "PASS_VERIFIED",
            "notice": "PASS_INCLUDED",
            "spdx_sbom": "PASS_INCLUDED",
            "gpl_source_offer": "PRESENT_UNREVIEWED_INSTRUCTIONS_ONLY",
            "gpl_corresponding_source": "NOT_INCLUDED",
            "first_party_worker_license": "NOASSERTION",
            "legal_review": "NOT_RUN",
            "product_distribution_signature": "NOT_RUN",
            "distribution_artifact": "NOT_AVAILABLE_IN_WORKSPACE",
        },
        "release_eligible": False,
        "release_blockers": [
            "GPL_SOURCE_CORRESPONDING_SOURCE_NOT_INCLUDED",
            "GPL_SOURCE_OFFER_LEGAL_REVIEW_NOT_RUN",
            "FIRST_PARTY_WORKER_LICENSE_NOT_ASSERTED",
            "LEGAL_REVIEW_NOT_RUN",
            "PRODUCT_DISTRIBUTION_SIGNATURE_NOT_RUN",
            "DISTRIBUTION_ARTIFACT_NOT_AVAILABLE",
        ],
        "notes": [
            "The Blender binary is included so a matching macOS arm64 packaged user does not need a separate Blender installation.",
            "The package contains Blender license texts and this acquisition record, but no Blender corresponding-source archive.",
            "The upstream Blender Developer ID signature is verified; this is not a Weaponry product distribution signature.",
            "The supplemental record is not Runtime truth and cannot advance a candidate, stage, approval, version, or export gate.",
        ],
        "canonical_sha256": "",
    }
    canonical_preimage = dict(manifest)
    canonical_preimage["canonical_sha256"] = ""
    manifest["canonical_sha256"] = hashlib.sha256(canonical_bytes(canonical_preimage)).hexdigest()
    path = staging / COMPLIANCE_DIRECTORY / COMPLIANCE_MANIFEST_NAME
    path.write_bytes(canonical_bytes(manifest) + b"\n")
    return path


def verify_compliance_package(
    output: Path,
    *,
    host: dict[str, Any],
    entrypoint_hash: str,
) -> dict[str, Any]:
    root = output / COMPLIANCE_DIRECTORY
    require(root.is_dir(), f"packaged compliance directory is missing: {root}")
    manifest_path = root / COMPLIANCE_MANIFEST_NAME
    require_regular_file(manifest_path, "packaged compliance manifest")
    manifest = read_json_object(manifest_path, "packaged compliance manifest")
    require(manifest.get("schema_version") == COMPLIANCE_SCHEMA, "packaged compliance schema drifted")
    require(
        manifest.get("status") == "DEVELOPMENT_STAGED_NOT_RELEASE_ELIGIBLE"
        and manifest.get("release_eligible") is False,
        "packaged compliance manifest must remain development-only",
    )
    canonical = require_sha(manifest.get("canonical_sha256"), "compliance canonical_sha256")
    canonical_preimage = dict(manifest)
    canonical_preimage["canonical_sha256"] = ""
    require(
        hashlib.sha256(canonical_bytes(canonical_preimage)).hexdigest() == canonical,
        "packaged compliance manifest canonical hash drifted",
    )
    worker = manifest.get("worker")
    require(isinstance(worker, dict), "packaged compliance Worker identity is missing")
    require(
        worker.get("worker_id") == EXPECTED_WORKER_ID
        and worker.get("worker_version") == EXPECTED_WORKER_VERSION
        and worker.get("entrypoint_sha256") == entrypoint_hash,
        "packaged compliance Worker identity drifted",
    )
    blender = manifest.get("blender")
    require(isinstance(blender, dict), "packaged compliance Blender identity is missing")
    require(
        blender.get("version") == host["blender_version"]
        and blender.get("display_version") == host["blender_display_version"]
        and blender.get("source_revision") == host["source_revision"]
        and blender.get("source_license") == EXPECTED_LICENSE
        and blender.get("executable_sha256") == host["binary_sha256"]
        and blender.get("bundle_tree_sha256") == host["bundle_tree_sha256"]
        and blender.get("bundle_signature") == "PASS_VERIFIED"
        and blender.get("download_artifact_status") == "NOT_AVAILABLE_IN_WORKSPACE",
        "packaged compliance Blender identity drifted",
    )
    records = manifest.get("artifacts")
    require(isinstance(records, dict) and set(records) == set(COMPLIANCE_ARTIFACT_NAMES), "packaged compliance artifacts are incomplete")
    for key, name in COMPLIANCE_ARTIFACT_NAMES.items():
        record = records.get(key)
        require(isinstance(record, dict), f"packaged compliance artifact record {key} is missing")
        require(
            record.get("path") == f"{COMPLIANCE_DIRECTORY}/{name}"
            and isinstance(record.get("sha256"), str)
            and SHA256_RE.fullmatch(record["sha256"]) is not None
            and isinstance(record.get("byte_size"), int)
            and record["byte_size"] > 0,
            f"packaged compliance artifact record {key} is invalid",
        )
        path = output / record["path"]
        require_regular_file(path, f"packaged compliance artifact {key}")
        require(sha256_file(path) == record["sha256"], f"packaged compliance artifact {key} hash drifted")
        require(path.stat().st_size == record["byte_size"], f"packaged compliance artifact {key} size drifted")

    notice = (output / records["notice"]["path"]).read_text(encoding="utf-8")
    require(
        host["binary_sha256"] in notice
        and host["bundle_tree_sha256"] in notice
        and "GPL-3.0-or-later" in notice
        and "GPL-SOURCE-OFFER.md" in notice,
        "packaged NOTICE is incomplete",
    )
    source_offer = (output / records["gpl_source_offer"]["path"]).read_text(encoding="utf-8")
    require(
        host["blender_version"] in source_offer
        and host["source_revision"] in source_offer
        and host["binary_sha256"] in source_offer
        and host["bundle_tree_sha256"] in source_offer
        and "not a completed written offer" in source_offer.lower(),
        "packaged GPL acquisition record is incomplete or overclaims its status",
    )
    sbom = read_json_object(output / records["spdx_sbom"]["path"], "packaged Blender SPDX supplement")
    validate_spdx_document(sbom, host=host, entrypoint_hash=entrypoint_hash)
    gates = manifest.get("gates")
    require(isinstance(gates, dict), "packaged compliance gates are missing")
    require(
        gates.get("offline_blender_resource") == "PASS_INCLUDED"
        and gates.get("blender_bundle_signature") == "PASS_VERIFIED"
        and gates.get("notice") == "PASS_INCLUDED"
        and gates.get("spdx_sbom") == "PASS_INCLUDED"
        and gates.get("gpl_source_offer") == "PRESENT_UNREVIEWED_INSTRUCTIONS_ONLY"
        and gates.get("gpl_corresponding_source") == "NOT_INCLUDED"
        and gates.get("first_party_worker_license") == "NOASSERTION"
        and gates.get("legal_review") == "NOT_RUN"
        and gates.get("product_distribution_signature") == "NOT_RUN"
        and gates.get("distribution_artifact") == "NOT_AVAILABLE_IN_WORKSPACE",
        "packaged compliance gate state drifted",
    )
    blockers = manifest.get("release_blockers")
    require(
        blockers == [
            "GPL_SOURCE_CORRESPONDING_SOURCE_NOT_INCLUDED",
            "GPL_SOURCE_OFFER_LEGAL_REVIEW_NOT_RUN",
            "FIRST_PARTY_WORKER_LICENSE_NOT_ASSERTED",
            "LEGAL_REVIEW_NOT_RUN",
            "PRODUCT_DISTRIBUTION_SIGNATURE_NOT_RUN",
            "DISTRIBUTION_ARTIFACT_NOT_AVAILABLE",
        ],
        "packaged compliance blockers drifted",
    )
    return {
        "status": manifest["status"],
        "release_eligible": False,
        "release_blockers": blockers,
        "canonical_sha256": canonical,
    }


def verify_packaged(
    output: Path,
    *,
    compare_current_source: bool = True,
    verify_command: bool = False,
) -> dict[str, Any]:
    validate_tauri_resource_mapping()
    manifest_path = output / PACKAGED_MANIFEST_NAME
    require(manifest_path.is_file(), f"packaged manifest is missing: {manifest_path}")
    manifest = read_json_object(manifest_path, "packaged manifest")
    require(
        manifest.get("schema_version") == PACKAGED_MANIFEST_SCHEMA,
        "packaged manifest schema drifted",
    )
    require(
        manifest.get("status") == "DEVELOPMENT_STAGED_NOT_RELEASE_ELIGIBLE",
        "packaged worker status is not development-only",
    )
    distribution_gates = manifest.get("distribution_gates")
    require(isinstance(distribution_gates, dict), "packaged distribution gates are missing")
    require(
        distribution_gates.get("release_eligible") is False,
        "packaged worker falsely claims release eligibility",
    )
    require(manifest.get("policy") == FIXED_LAUNCH_POLICY, "packaged worker launch policy drifted")

    claimed_resource_hash = require_sha(manifest.get("resource_tree_sha256"), "resource_tree_sha256")
    actual_resource_hash, _, _ = tree_inventory(output, excluded={PACKAGED_MANIFEST_NAME})
    require(actual_resource_hash == claimed_resource_hash, "packaged resource tree hash drifted")
    claimed_canonical = require_sha(manifest.get("canonical_sha256"), "canonical_sha256")
    canonical_preimage = dict(manifest)
    canonical_preimage["canonical_sha256"] = ""
    require(
        hashlib.sha256(canonical_bytes(canonical_preimage)).hexdigest() == claimed_canonical,
        "packaged manifest canonical hash drifted",
    )

    worker = manifest.get("worker")
    require(isinstance(worker, dict), "packaged worker metadata is missing")
    entrypoint_relative = safe_relative(worker.get("entrypoint_path"), "worker.entrypoint_path")
    source_manifest_relative = safe_relative(
        worker.get("source_manifest_path"), "worker.source_manifest_path"
    )
    entrypoint = output / entrypoint_relative
    source_manifest_path = output / source_manifest_relative
    require(
        entrypoint.is_file() and source_manifest_path.is_file(),
        "packaged Worker sources are incomplete",
    )
    entrypoint_hash = sha256_file(entrypoint)
    require(
        entrypoint_hash == worker.get("entrypoint_sha256"),
        "packaged Worker entrypoint hash drifted",
    )
    source_manifest = read_json_object(source_manifest_path, "packaged source manifest")
    require(
        source_manifest.get("entrypoint_sha256") == entrypoint_hash,
        "packaged source manifest entrypoint hash is inconsistent",
    )
    require_sha(worker.get("source_manifest_sha256"), "worker.source_manifest_sha256")
    require(
        sha256_file(source_manifest_path) == worker["source_manifest_sha256"],
        "packaged source manifest hash drifted",
    )
    if compare_current_source:
        require(
            entrypoint_hash == sha256_file(WORKER_SCRIPT),
            "staged package is stale; re-run staging after the entrypoint changed",
        )
        current_source = read_json_object(WORKER_SOURCE_MANIFEST, "checked-in source manifest")
        host = host_from_source_manifest(current_source)
        expected_source = normalized_source_manifest(current_source, entrypoint_hash)
        expected_source_bytes = canonical_bytes(expected_source) + b"\n"
        require(
            source_manifest_path.read_bytes() == expected_source_bytes,
            "packaged source manifest is stale; re-run staging",
        )
    else:
        host = package_host_metadata(manifest)

    compliance = verify_compliance_package(
        output,
        host=host,
        entrypoint_hash=entrypoint_hash,
    )
    blender_metadata = manifest.get("blender")
    require(isinstance(blender_metadata, dict), "packaged Blender metadata is missing")
    runtime_relative = safe_relative(
        blender_metadata.get("executable_path"), "blender.executable_path"
    )
    executable = output / runtime_relative
    require(
        executable.name == "Blender" and executable.parent.name == "MacOS",
        "packaged Blender executable path is invalid",
    )
    # ``executable`` is .../Blender.app/Contents/MacOS/Blender; the bundle is
    # therefore three parents up (MacOS, Contents, Blender.app).
    bundle_root = executable.parents[2]
    require(bundle_root.name == "Blender.app", "packaged Blender bundle path is invalid")
    validate_blender_bundle(bundle_root, host, verify_command=verify_command)
    return {
        "status": manifest["status"],
        "resource_tree_sha256": claimed_resource_hash,
        "canonical_sha256": claimed_canonical,
        "entrypoint_sha256": entrypoint_hash,
        "bundle_tree_sha256": blender_metadata["bundle_tree_sha256"],
        "release_eligible": False,
        "compliance": compliance["status"],
        "release_blockers": compliance["release_blockers"],
    }


def stage(bundle: Path, output: Path) -> Path:
    source_manifest = read_json_object(WORKER_SOURCE_MANIFEST, "checked-in source manifest")
    host = host_from_source_manifest(source_manifest)
    entrypoint_hash = sha256_file(WORKER_SCRIPT)
    validate_source_compliance(host, entrypoint_hash)
    validate_blender_bundle(bundle, host)
    output = output_path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(
        tempfile.mkdtemp(prefix=".weaponry-blender-stage-", dir=output.parent)
    )
    try:
        staged_bundle = temporary / "runtime" / "Blender.app"
        staged_bundle.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            ["ditto", str(bundle.resolve(strict=True)), str(staged_bundle)],
            check=True,
        )
        worker_root = temporary / "worker"
        worker_root.mkdir(parents=True, exist_ok=True)
        shutil.copy2(WORKER_SCRIPT, worker_root / "weaponry_knife_worker.py")
        normalized = normalized_source_manifest(source_manifest, entrypoint_hash)
        source_manifest_bytes = canonical_bytes(normalized) + b"\n"
        (worker_root / "source-manifest.json").write_bytes(source_manifest_bytes)
        copy_source_compliance_artifacts(temporary)
        write_compliance_manifest(
            temporary,
            host=host,
            entrypoint_hash=entrypoint_hash,
        )

        staged_metadata = validate_blender_bundle(staged_bundle, host)
        require(
            staged_metadata["executable_sha256"] == host["binary_sha256"],
            "staged Blender executable hash drifted",
        )
        require(
            staged_metadata["bundle_tree_sha256"] == host["bundle_tree_sha256"],
            "staged Blender bundle tree hash drifted",
        )
        require(
            sha256_file(worker_root / "weaponry_knife_worker.py") == entrypoint_hash,
            "staged Worker entrypoint hash drifted",
        )
        require(
            sha256_file(worker_root / "source-manifest.json")
            == hashlib.sha256(source_manifest_bytes).hexdigest(),
            "staged source manifest hash drifted",
        )

        resource_hash, resource_file_count, resource_total_bytes = tree_inventory(
            temporary,
            excluded={PACKAGED_MANIFEST_NAME},
        )
        manifest: dict[str, Any] = {
            "schema_version": PACKAGED_MANIFEST_SCHEMA,
            "status": "DEVELOPMENT_STAGED_NOT_RELEASE_ELIGIBLE",
            "blender": {
                "version": host["blender_version"],
                "display_version": host["blender_display_version"],
                "source_revision": host["source_revision"],
                "build_hash": host["build_hash"],
                "build_branch": host.get("build_branch"),
                "build_platform": host.get("build_platform"),
                "build_type": host.get("build_type"),
                "bundle_id": host["bundle_id"],
                "team_id": host["team_id"],
                "signing_authority": host["signing_authority"],
                "executable_path": "runtime/Blender.app/Contents/MacOS/Blender",
                "executable_sha256": staged_metadata["executable_sha256"],
                "bundle_tree_sha256": staged_metadata["bundle_tree_sha256"],
                "bundle_file_count": staged_metadata["bundle_file_count"],
                "bundle_total_bytes": staged_metadata["bundle_total_bytes"],
                "python_bundle_sha256": staged_metadata["python_bundle_sha256"],
                "license_resources": staged_metadata["license_resources"],
                "download_artifact": host["download_artifact"],
            },
            "worker": {
                "entrypoint_path": "worker/weaponry_knife_worker.py",
                "entrypoint_sha256": entrypoint_hash,
                "entrypoint_hash_policy": "DERIVED_FROM_STAGED_ENTRYPOINT_BYTES",
                "source_manifest_path": "worker/source-manifest.json",
                "source_manifest_sha256": hashlib.sha256(source_manifest_bytes).hexdigest(),
                "worker_id": source_manifest["worker_id"],
                "worker_version": source_manifest["worker_version"],
                "protocol": source_manifest["protocol"],
                "operation": source_manifest["operation"],
                "recipe_sha256": source_manifest["recipe_sha256"],
                "dependency_lock_sha256": source_manifest["dependency_lock_sha256"],
            },
            "policy": dict(FIXED_LAUNCH_POLICY),
            "runtime_invocation": {
                "environment": "cleared_by_runtime_launcher",
                "arguments": [
                    "--background",
                    "--factory-startup",
                    "--disable-autoexec",
                    "--threads",
                    "1",
                    "--debug-depsgraph-no-threads",
                    "--python-exit-code",
                    "1",
                    "--python",
                    "<sealed-worker-entrypoint>",
                    "--",
                    "--scratch-root",
                    "<runtime-scratch>",
                ],
                "caller_controls": {
                    "python": False,
                    "addon": False,
                    "url": False,
                    "path": False,
                    "environment": False,
                    "network": False,
                },
            },
            "distribution_gates": {
                "blender_license": "PRESENT_BUNDLE_RESOURCE",
                "blender_bundle_signature": staged_metadata["signature"],
                "gpl_source_offer": "NOT_INCLUDED",
                "notice": "NOT_INCLUDED",
                "spdx_sbom": "NOT_INCLUDED",
                "legal_review": "NOT_RUN",
                "product_distribution_signature": "NOT_RUN",
                "release_eligible": False,
                "release_blockers": [
                    "GPL_SOURCE_OFFER_NOT_INCLUDED",
                    "NOTICE_NOT_INCLUDED",
                    "SPDX_SBOM_NOT_INCLUDED",
                    "LEGAL_REVIEW_NOT_RUN",
                    "PRODUCT_DISTRIBUTION_SIGNATURE_NOT_RUN",
                ],
            },
            "provenance": {
                "input": "locally_installed_blender_app",
                "download_artifact_status": host["download_artifact"]["status"],
                "download_artifact_sha256": host["download_artifact"]["sha256"],
                "bundle_tree_sha256": staged_metadata["bundle_tree_sha256"],
            },
            "resource_tree_sha256": resource_hash,
            "resource_tree_file_count": resource_file_count,
            "resource_tree_total_bytes": resource_total_bytes,
            "canonical_sha256": "",
        }
        canonical_preimage = dict(manifest)
        canonical_preimage["canonical_sha256"] = ""
        manifest["canonical_sha256"] = hashlib.sha256(
            canonical_bytes(canonical_preimage)
        ).hexdigest()
        manifest_path = temporary / PACKAGED_MANIFEST_NAME
        manifest_path.write_bytes(canonical_bytes(manifest) + b"\n")
        verify_packaged(temporary, compare_current_source=True, verify_command=False)

        if output.exists():
            marker = output / PACKAGED_MANIFEST_NAME
            require(
                marker.is_file(),
                f"refusing to replace an unrecognized output directory: {output}",
            )
            shutil.rmtree(output)
        temporary.replace(output)
        return output / PACKAGED_MANIFEST_NAME
    except Exception:
        shutil.rmtree(temporary, ignore_errors=True)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--blender-app",
        type=Path,
        default=DEFAULT_BLENDER_APP,
        help="complete pinned Blender.app input bundle",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help="generated resource directory below the ignored Tauri target",
    )
    parser.add_argument(
        "--verify",
        action="store_true",
        help="verify an existing staged package and detect stale entrypoint bytes",
    )
    args = parser.parse_args()
    output = output_path(args.output)
    if args.verify:
        result = verify_packaged(output, compare_current_source=True, verify_command=True)
        print(json.dumps(result, sort_keys=True))
        return 0
    manifest = stage(args.blender_app, output)
    print(manifest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
