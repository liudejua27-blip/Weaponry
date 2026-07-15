#!/usr/bin/env python3
"""Release gate for secrets and local file access boundaries."""

from __future__ import annotations

import json
import re
import sqlite3
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import AssetStoreError, SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest, ExportUnityRequest  # noqa: E402


SCAN_ROOTS = [
    "README.md",
    "package.json",
    "apps/agent",
    "apps/desktop/src",
    "apps/desktop/src-tauri",
    "docs",
    "migrations",
    "packages",
    "scripts",
    "workflows",
]
SKIP_PARTS = {
    "__pycache__",
    ".pytest_cache",
    ".ruff_cache",
    "target",
    "dist",
    "generated",
    "node_modules",
}
SKIP_SUFFIXES = {
    ".png",
    ".jpg",
    ".jpeg",
    ".webp",
    ".glb",
    ".zip",
    ".db",
    ".db-shm",
    ".db-wal",
    ".tsbuildinfo",
}
SECRET_PATTERNS = [
    ("OPENAI_KEY", re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b")),
    ("GENERIC_API_KEY_ASSIGNMENT", re.compile(r"(?i)\b(api[_-]?key|secret|token)\b\s*[:=]\s*['\"][^'\"\s]{16,}['\"]")),
    ("AUTHORIZATION_BEARER_LITERAL", re.compile(r"(?i)authorization['\"]?\s*[:=]\s*['\"]bearer\s+[A-Za-z0-9._~+/=-]{16,}['\"]")),
]
ALLOWED_SECRET_CONTEXTS = [
    "WUSHEN_LLM_API_KEY",
    "WUSHEN_LLM_API_KEY_FILE",
    "OPENAI_API_KEY",
    "Authorization",
    "Bearer {",
    "sk-proj keys",
    "secret {provider.has_secret",
]


def main() -> int:
    findings: list[dict[str, Any]] = []
    secret_summary = scan_for_secrets(findings)
    tauri_summary = check_tauri_hardening(findings)
    asset_summary = check_asset_file_boundaries(findings)
    docs_summary = check_docs(findings)

    blockers = [item for item in findings if item["severity"] == "blocker"]
    warnings = [item for item in findings if item["severity"] == "warning"]
    print(
        json.dumps(
            {
                "ok": not blockers,
                "summaries": {
                    "secrets": secret_summary,
                    "tauri": tauri_summary,
                    "asset_boundaries": asset_summary,
                    "docs": docs_summary,
                },
                "blockers": blockers,
                "warnings": warnings,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0 if not blockers else 1


def scan_for_secrets(findings: list[dict[str, Any]]) -> dict[str, Any]:
    scanned_files = 0
    matches = 0
    for path in iter_scan_files():
        scanned_files += 1
        text = path.read_text(encoding="utf-8", errors="ignore")
        for line_no, line in enumerate(text.splitlines(), start=1):
            if is_allowed_secret_context(line):
                continue
            for code, pattern in SECRET_PATTERNS:
                if pattern.search(line):
                    matches += 1
                    findings.append(blocker("SECRET_LITERAL", f"{code} matched {path.relative_to(ROOT)}:{line_no}."))
    return {"files": scanned_files, "matches": matches}


def iter_scan_files() -> list[Path]:
    files: list[Path] = []
    for entry in SCAN_ROOTS:
        path = ROOT / entry
        if path.is_file():
            files.append(path)
            continue
        if not path.exists():
            continue
        for candidate in path.rglob("*"):
            if not candidate.is_file():
                continue
            rel_parts = set(candidate.relative_to(ROOT).parts)
            if rel_parts & SKIP_PARTS:
                continue
            if candidate.suffix.lower() in SKIP_SUFFIXES:
                continue
            files.append(candidate)
    return sorted(set(files))


def is_allowed_secret_context(line: str) -> bool:
    return any(context in line for context in ALLOWED_SECRET_CONTEXTS)


def check_tauri_hardening(findings: list[dict[str, Any]]) -> dict[str, Any]:
    config_path = ROOT / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
    if not config_path.exists():
        findings.append(blocker("TAURI_CONFIG_MISSING", "Tauri config is required for desktop release."))
        return {"config": False, "csp": None, "capabilities": 0}
    config = json.loads(config_path.read_text(encoding="utf-8"))
    csp = ((config.get("app") or {}).get("security") or {}).get("csp")
    if not isinstance(csp, str) or not csp.strip():
        findings.append(blocker("TAURI_CSP_DISABLED", "Tauri production config must define a restrictive CSP instead of null/empty."))

    capabilities_dir = ROOT / "apps" / "desktop" / "src-tauri" / "capabilities"
    capability_files = sorted(capabilities_dir.glob("*.json")) if capabilities_dir.exists() else []
    if not capability_files:
        findings.append(blocker("TAURI_CAPABILITIES_MISSING", "Tauri production shell must define explicit capabilities/permissions for invoke commands."))
    return {"config": True, "csp": csp, "capabilities": len(capability_files)}


def check_asset_file_boundaries(findings: list[dict[str, Any]]) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="wushen_release_file_scope_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        store = SQLiteAssetStore(library_root=library_root, migrations_dir=ROOT / "migrations")
        created = store.create_weapon(
            CreateWeaponRequest(
                client_request_id="release-file-scope-create",
                text="玄铁龙纹剑，3渲2国风神兵，仅作为虚构 Unity 游戏资产",
            ),
            idempotency_key="release-file-scope-create",
        )
        detail = store.get_weapon_detail(created.weapon_id)
        exported = store.export_unity(
            created.weapon_id,
            ExportUnityRequest(client_request_id="release-file-scope-export", model_id=detail.current_model_id),
            idempotency_key="release-file-scope-export",
        )
        package_asset_id = unity_package_asset_id(store, exported.job_id)
        reveal = store.reveal_asset_file(package_asset_id, dry_run=True)
        leaked = reveal.model_dump()
        if any(key in leaked for key in ["path", "absolute_path", "object_path"]):
            findings.append(blocker("ASSET_REVEAL_PATH_LEAK", "Asset reveal response must not expose local paths."))
        if Path(reveal.filename).name != reveal.filename:
            findings.append(blocker("ASSET_REVEAL_FILENAME_LEAK", "Asset reveal filename must not contain a path."))

        asset = store.resolve_asset_file(package_asset_id)
        if "path" not in asset:
            findings.append(blocker("ASSET_RESOLVE_INTERNAL_PATH_MISSING", "Internal asset resolution must retain path for FileResponse."))
        if "path" in leaked:
            findings.append(blocker("ASSET_REVEAL_INTERNAL_PATH_EXPOSED", "Internal path leaked into reveal response."))

        with sqlite3.connect(library_root / "library.db") as conn:
            conn.execute("UPDATE asset_files SET object_path = ? WHERE file_id = ?", ("/tmp/wushen_escape.zip", package_asset_id))
            conn.commit()
        try:
            store.resolve_asset_file(package_asset_id)
            findings.append(blocker("ASSET_OBJECT_PATH_ESCAPE_ALLOWED", "Absolute asset object_path was not rejected."))
        except AssetStoreError as exc:
            if exc.code != "ASSET_PERMISSION_DENIED":
                findings.append(blocker("ASSET_OBJECT_PATH_ESCAPE_WRONG_ERROR", f"Expected ASSET_PERMISSION_DENIED, got {exc.code}."))
    return {"dynamic_checks": ["reveal_no_path", "resolve_internal_only", "absolute_object_path_rejected"]}


def unity_package_asset_id(store: SQLiteAssetStore, job_id: str) -> str:
    job = store.get_job(job_id)
    for asset_id, role in job.outputs.get("asset_roles", {}).items():
        if role == "unity_export_package":
            return asset_id
    raise AssertionError("export job did not expose unity_export_package")


def check_docs(findings: list[dict[str, Any]]) -> dict[str, Any]:
    requirements = {
        "docs/DEVELOPMENT.md": ["CSP", "capability", "Provider Key"],
        "docs/DATABASE.md": ["no export package contains absolute local paths", "object_path"],
        "docs/IMPLEMENTATION_PLAN.md": ["secret/file-overreach"],
    }
    missing: dict[str, list[str]] = {}
    for rel_path, phrases in requirements.items():
        text = (ROOT / rel_path).read_text(encoding="utf-8") if (ROOT / rel_path).exists() else ""
        absent = [phrase for phrase in phrases if phrase not in text]
        if absent:
            missing[rel_path] = absent
    if missing:
        findings.append(blocker("SECURITY_DOCS_INCOMPLETE", "Security/file boundary docs are incomplete.", missing))
    return {"checked": sorted(requirements), "missing": missing}


def blocker(code: str, message: str, details: Any | None = None) -> dict[str, Any]:
    item: dict[str, Any] = {"severity": "blocker", "code": code, "message": message}
    if details:
        item["details"] = details
    return item


if __name__ == "__main__":
    raise SystemExit(main())
