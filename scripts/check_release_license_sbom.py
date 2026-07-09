#!/usr/bin/env python3
"""Release license and SBOM gate for Wushen Forge.

The gate has two jobs:
- verify the dependency inventories that are currently machine-readable;
- keep production release blocked when a dependency family is not locked or an
  external model/runtime license review is still pending.
"""

from __future__ import annotations

import json
import re
import sys
from importlib import metadata
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
ALLOWED_LICENSE_ATOMS = {
    "0BSD",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC-BY-4.0",
    "ISC",
    "MIT",
    "MPL-2.0",
    "PSF-2.0",
    "Python-2.0",
    "Unicode-DFS-2016",
}
BLOCKED_LICENSE_MARKERS = ("AGPL", "GPL", "LGPL", "SSPL", "BUSL", "Commons-Clause", "UNLICENSED")
REQUIRED_EXTERNAL_LEDGER_ITEMS = [
    "ComfyUI",
    "Stable Fast 3D",
    "TripoSR",
    "Hunyuan3D",
    "TRELLIS",
    "Unity glTFast",
    "Tauri",
    "FastAPI",
]


def main() -> int:
    findings: list[dict[str, Any]] = []
    npm_summary = check_npm_lock(findings)
    python_summary = check_python_project(findings)
    rust_summary = check_rust_project(findings)
    docs_summary = check_license_ledger(findings)

    blockers = [item for item in findings if item["severity"] == "blocker"]
    warnings = [item for item in findings if item["severity"] == "warning"]
    result = {
        "ok": not blockers,
        "summaries": {
            "npm": npm_summary,
            "python": python_summary,
            "rust": rust_summary,
            "docs": docs_summary,
        },
        "blockers": blockers,
        "warnings": warnings,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if not blockers else 1


def check_npm_lock(findings: list[dict[str, Any]]) -> dict[str, Any]:
    lock_path = ROOT / "package-lock.json"
    if not lock_path.exists():
        findings.append(blocker("NPM_LOCK_MISSING", "package-lock.json is required for release SBOM."))
        return {"packages": 0, "licenses": {}}
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    packages = lock.get("packages")
    if not isinstance(packages, dict):
        findings.append(blocker("NPM_LOCK_INVALID", "package-lock.json packages section is missing."))
        return {"packages": 0, "licenses": {}}

    licenses: dict[str, int] = {}
    package_count = 0
    for path, meta in packages.items():
        if not isinstance(meta, dict) or not str(path).startswith("node_modules/"):
            continue
        if meta.get("link") is True:
            continue
        name = str(meta.get("name") or path.rsplit("node_modules/", 1)[-1])
        version = str(meta.get("version") or "unknown")
        license_expr = meta.get("license")
        package_count += 1
        if not isinstance(license_expr, str) or not license_expr.strip():
            findings.append(blocker("NPM_LICENSE_MISSING", f"{name}@{version} has no license in package-lock.json."))
            continue
        license_expr = license_expr.strip()
        licenses[license_expr] = licenses.get(license_expr, 0) + 1
        if not is_allowed_license_expression(license_expr):
            findings.append(blocker("NPM_LICENSE_NOT_ALLOWED", f"{name}@{version} uses {license_expr}."))
    return {"packages": package_count, "licenses": licenses}


def check_python_project(findings: list[dict[str, Any]]) -> dict[str, Any]:
    pyproject = ROOT / "apps" / "agent" / "pyproject.toml"
    lock_path = ROOT / "apps" / "agent" / "requirements-release.lock"
    if not pyproject.exists():
        findings.append(blocker("PYPROJECT_MISSING", "apps/agent/pyproject.toml is required."))
        return {"dependencies": []}
    text = pyproject.read_text(encoding="utf-8")
    dependencies = parse_toml_array(text, "dependencies")
    dev_dependencies = parse_toml_array(text, "dev")
    lock_entries = parse_python_release_lock(lock_path, findings)
    if dependencies and not lock_entries:
        findings.append(
            blocker(
                "PYTHON_LOCK_MISSING",
                "Python release dependencies are declared but no lock/requirements file is present for license/SBOM verification.",
                {"dependencies": dependencies},
            )
        )
    direct_names = {dependency_name(dep) for dep in dependencies}
    locked_names = {name.lower().replace("_", "-") for name in lock_entries}
    missing_direct = sorted(name for name in direct_names if name and name not in locked_names)
    if missing_direct:
        findings.append(blocker("PYTHON_LOCK_INCOMPLETE", "Python release lock is missing direct dependencies.", {"dependencies": missing_direct}))
    verify_python_lock_installed_versions(lock_entries, findings)
    return {
        "dependencies": dependencies,
        "dev_dependencies": dev_dependencies,
        "release_lock": str(lock_path.relative_to(ROOT)),
        "locked_packages": len(lock_entries),
    }


def check_rust_project(findings: list[dict[str, Any]]) -> dict[str, Any]:
    cargo_toml = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
    cargo_lock = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.lock"
    if not cargo_toml.exists():
        findings.append(blocker("CARGO_TOML_MISSING", "Tauri Cargo.toml is required."))
        return {"cargo_toml": False, "cargo_lock": False}
    if not cargo_lock.exists():
        findings.append(blocker("CARGO_LOCK_MISSING", "Tauri release builds must commit Cargo.lock for Rust license/SBOM verification."))
    return {"cargo_toml": True, "cargo_lock": cargo_lock.exists()}


def check_license_ledger(findings: list[dict[str, Any]]) -> dict[str, Any]:
    ledger_path = ROOT / "docs" / "THIRD_PARTY_LICENSES.md"
    if not ledger_path.exists():
        findings.append(blocker("LICENSE_LEDGER_MISSING", "docs/THIRD_PARTY_LICENSES.md is required before release."))
        return {"exists": False, "pending_items": []}
    text = ledger_path.read_text(encoding="utf-8")
    for item in REQUIRED_EXTERNAL_LEDGER_ITEMS:
        if item not in text:
            findings.append(blocker("LICENSE_LEDGER_INCOMPLETE", f"License ledger missing external item: {item}."))
    pending_items = []
    for line in text.splitlines():
        if not line.startswith("|") or "---" in line or line.lower().startswith("| item"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) >= 3 and cells[2].lower() in {"pending", "blocked"}:
            pending_items.append(cells[0])
    pending_items = sorted(set(pending_items))
    if pending_items:
        findings.append(
            blocker(
                "EXTERNAL_LICENSE_REVIEW_PENDING",
                "External runtime/model/package license reviews are still pending.",
                {"items": pending_items},
            )
        )
    return {"exists": True, "pending_items": pending_items}


def parse_toml_array(text: str, key: str) -> list[str]:
    pattern = re.compile(rf"^\s*{re.escape(key)}\s*=\s*\[(.*?)^\s*\]", re.MULTILINE | re.DOTALL)
    match = pattern.search(text)
    if not match:
        return []
    block = match.group(1)
    return [item for item in re.findall(r'"([^"]+)"', block)]


def parse_python_release_lock(path: Path, findings: list[dict[str, Any]]) -> dict[str, dict[str, str]]:
    if not path.exists():
        return {}
    entries: dict[str, dict[str, str]] = {}
    pattern = re.compile(r"^([A-Za-z0-9_.-]+)==([A-Za-z0-9_.!+:-]+)\s+#\s+license=([^;#]+);\s+required_by=(.+)$")
    for line_no, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        match = pattern.match(line)
        if not match:
            findings.append(blocker("PYTHON_LOCK_INVALID_LINE", f"{path}:{line_no} is not a pinned requirement with license metadata."))
            continue
        name, version, license_expr, required_by = (item.strip() for item in match.groups())
        key = name.lower().replace("_", "-")
        entries[key] = {"name": name, "version": version, "license": license_expr, "required_by": required_by}
        if not is_allowed_license_expression(license_expr):
            findings.append(blocker("PYTHON_LICENSE_NOT_ALLOWED", f"{name}=={version} uses {license_expr}."))
    return entries


def dependency_name(requirement: str) -> str:
    match = re.match(r"\s*([A-Za-z0-9_.-]+)", requirement)
    return match.group(1).lower().replace("_", "-") if match else ""


def verify_python_lock_installed_versions(lock_entries: dict[str, dict[str, str]], findings: list[dict[str, Any]]) -> None:
    for key, entry in sorted(lock_entries.items()):
        try:
            dist = metadata.distribution(entry["name"])
        except metadata.PackageNotFoundError:
            try:
                dist = metadata.distribution(key)
            except metadata.PackageNotFoundError:
                findings.append(blocker("PYTHON_LOCK_PACKAGE_NOT_INSTALLED", f"{entry['name']}=={entry['version']} is locked but not installed in the active environment."))
                continue
        if dist.version != entry["version"]:
            findings.append(
                blocker(
                    "PYTHON_LOCK_VERSION_DRIFT",
                    f"{entry['name']} lock version {entry['version']} does not match installed {dist.version}.",
                )
            )


def is_allowed_license_expression(expression: str) -> bool:
    if any(marker in expression for marker in BLOCKED_LICENSE_MARKERS):
        return False
    atoms = [part.strip(" ()") for part in re.split(r"\bOR\b|\bAND\b|/|\+", expression) if part.strip(" ()")]
    return bool(atoms) and all(atom in ALLOWED_LICENSE_ATOMS for atom in atoms)


def blocker(code: str, message: str, details: dict[str, Any] | None = None) -> dict[str, Any]:
    item: dict[str, Any] = {"severity": "blocker", "code": code, "message": message}
    if details:
        item["details"] = details
    return item


if __name__ == "__main__":
    raise SystemExit(main())
