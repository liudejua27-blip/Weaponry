#!/usr/bin/env python3
"""Validate the pinned img2threejs adoption metadata without network or Cargo."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys


HERE = Path(__file__).resolve().parent
REVISION = "9fbd0ca5bbcc3b13bebe712745d6784d33db0b85"


def fail(message: str) -> "NoReturn":
    raise SystemExit(f"weaponry-threejs adoption check failed: {message}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        fail(f"{path.name} is not valid JSON: {exc}")
    if not isinstance(value, dict):
        fail(f"{path.name} must contain an object")
    return value


def check_text(path: Path, needles: tuple[str, ...]) -> None:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        fail(f"cannot read {path.name}: {exc}")
    for needle in needles:
        if needle not in text:
            fail(f"{path.name} is missing required attribution/policy text: {needle}")


def reject_sensitive_metadata(value: object, label: str = "metadata") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            reject_sensitive_metadata(key, f"{label}.key")
            reject_sensitive_metadata(child, f"{label}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            reject_sensitive_metadata(child, f"{label}[{index}]")
    elif isinstance(value, str):
        if re.search(r"(?:^|[/\\])(?:Users|home|var|tmp)[/\\]", value):
            fail(f"{label} contains an absolute/local path: {value}")
        if re.search(r"(?i)(?:api[_-]?key|access[_-]?token|secret|password)\s*[:=]", value):
            fail(f"{label} contains a credential-like field")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache-checkout", type=Path, help="optional already-restored checkout to validate")
    args = parser.parse_args()

    manifest_path = HERE / "manifest.json"
    sbom_path = HERE / "sbom.spdx.json"
    provenance_path = HERE / "provenance.json"
    notice_path = HERE / "NOTICE"
    license_path = HERE / "LICENSES" / "Apache-2.0.txt"
    first_party_license_path = HERE / "LICENSES" / "ForgeCAD-FIRST-PARTY.txt"
    for path in (manifest_path, sbom_path, provenance_path, notice_path, license_path, first_party_license_path):
        if not path.is_file():
            fail(f"required metadata file missing: {path}")

    manifest = load_json(manifest_path)
    sbom = load_json(sbom_path)
    provenance = load_json(provenance_path)
    reject_sensitive_metadata(manifest)
    reject_sensitive_metadata(sbom)
    reject_sensitive_metadata(provenance)

    if manifest.get("schema_version") != "ForgeCadUpstreamAdoptionManifest@1":
        fail("unexpected manifest schema")
    if manifest.get("status") != "SOURCE_BASELINE_ACCEPTED_ADAPTER_PARTIAL":
        fail("unexpected source-baseline adoption status")
    if manifest.get("accepted_as_source_baseline") is not True:
        fail("accepted_as_source_baseline must be true")
    if manifest.get("accepted_as_runtime_dependency") is not False:
        fail("accepted_as_runtime_dependency must remain false")
    if manifest.get("accepted_as_dependency") is not False:
        fail("accepted_as_dependency must remain false")
    source = manifest.get("source")
    if not isinstance(source, dict) or source.get("revision") != REVISION:
        fail("source revision is not pinned")
    if source.get("tracked_file_count") != 337 or source.get("tracked_blob_bytes") != 4126550:
        fail("source tree size inventory drifted")
    if source.get("canonical_tree_manifest_sha256") != "bf4b35eb5b468a77ae6d8a24fc2e4b7a42fa27ad87c2afacd32bf635e069d91e":
        fail("source tree manifest digest drifted")

    license_info = manifest.get("license")
    if not isinstance(license_info, dict) or license_info.get("spdx") != "Apache-2.0":
        fail("Apache-2.0 license record missing")
    expected_license = license_info.get("upstream_license_sha256")
    actual_license = sha256_file(license_path)
    if actual_license != expected_license:
        fail(f"local Apache license hash mismatch: {actual_license}")
    if sha256_file(first_party_license_path) != "49f82378a300f6ba3ecfacfa5bf79fc2db51ed31730fc10eb74371333853cdbb":
        fail("first-party metadata license hash mismatch")
    check_text(notice_path, ("img2threejs", REVISION, "Apache-2.0", "absent-at-frozen-revision"))

    if sbom.get("spdxVersion") != "SPDX-2.3":
        fail("SBOM is not SPDX-2.3")
    packages = sbom.get("packages")
    if not isinstance(packages, list) or not any(
        isinstance(item, dict)
        and item.get("name") == "img2threejs"
        and item.get("versionInfo") == f"git-{REVISION}"
        and item.get("licenseDeclared") == "Apache-2.0"
        for item in packages
    ):
        fail("SBOM does not describe the pinned img2threejs source")
    files = sbom.get("files")
    if not isinstance(files, list) or not any(
        isinstance(item, dict)
        and item.get("fileName") == "LICENSES/Apache-2.0.txt"
        and item.get("checksums", [{}])[0].get("checksum") == actual_license
        for item in files
    ):
        fail("SBOM license file checksum is missing")

    if provenance.get("schema_version") != "ForgeCadUpstreamAdoptionProvenance@1":
        fail("provenance schema is missing")
    materials = provenance.get("materials")
    if not isinstance(materials, list) or not materials or materials[0].get("revision") != REVISION:
        fail("provenance does not bind the pinned revision")
    if provenance.get("repository_policy", {}).get("upstream_execution") != "forbidden":
        fail("provenance must forbid upstream execution")
    if provenance.get("subject", {}).get("manifest_sha256") != sha256_file(manifest_path):
        fail("provenance manifest digest does not match manifest.json")

    snapshot = manifest.get("snapshot", {})
    if snapshot.get("vendored_upstream_files") != [] or snapshot.get("lockfile_or_package_change") is not False:
        fail("metadata no longer describes an isolated, non-vendored snapshot")

    compatibility = manifest.get("compatibility_import", {})
    if compatibility.get("status") != "PASS_STATIC_BOUNDED_IMPORT":
        fail("bounded static compatibility import is not recorded")
    if compatibility.get("first_party_adapter") != "packages/weaponry-threejs/src/img2threejs-object-sculpt-adapter.ts":
        fail("first-party compatibility adapter path drifted")
    if manifest.get("gates", {}).get("one_bounded_import") != "PASS":
        fail("one_bounded_import gate is not PASS")

    if args.cache_checkout is not None:
        verifier = HERE / "restore_pinned_snapshot.py"
        import subprocess

        result = subprocess.run(
            [sys.executable, str(verifier), "--verify", str(args.cache_checkout)],
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            fail(result.stdout.strip() or result.stderr.strip() or "cache verification failed")

    print("weaponry-threejs adoption metadata OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
