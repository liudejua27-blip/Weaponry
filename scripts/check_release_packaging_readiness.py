#!/usr/bin/env python3
"""Desktop packaging readiness gate for Wushen Forge release candidates."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Optional


ROOT = Path(__file__).resolve().parents[1]
TAURI_DIR = ROOT / "apps" / "desktop" / "src-tauri"


REQUIRED_DOC_PHRASES = {
    "docs/PACKAGING.md": [
        "bundle.externalBin",
        "binaries/wushen-agent",
        "packaged-sidecar",
        "local-dev-python",
        "Cargo.lock",
        "release:packaging-readiness",
        "fictional Unity game-art",
        "non-manufacturing",
    ],
    "docs/M3_DESKTOP_SUPERVISOR.md": [
        "local-dev-python",
        "packaged-sidecar",
        "bundle.externalBin",
    ],
    "README.md": [
        "npm run release:packaging-readiness",
        "docs/PACKAGING.md",
    ],
}


def main() -> int:
    blockers: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    summaries: dict[str, Any] = {}

    config_path = TAURI_DIR / "tauri.conf.json"
    cargo_toml_path = TAURI_DIR / "Cargo.toml"
    cargo_lock_path = TAURI_DIR / "Cargo.lock"
    main_rs_path = TAURI_DIR / "src" / "main.rs"
    capabilities_dir = TAURI_DIR / "capabilities"

    config = _read_json(config_path)
    bundle = config.get("bundle") if isinstance(config.get("bundle"), dict) else {}
    build = config.get("build") if isinstance(config.get("build"), dict) else {}
    app = config.get("app") if isinstance(config.get("app"), dict) else {}
    security = app.get("security") if isinstance(app.get("security"), dict) else {}

    _require(blockers, bool(config.get("productName")), "TAURI_PRODUCT_NAME_MISSING", "tauri.conf.json must define productName.")
    _require(blockers, bool(config.get("version")), "TAURI_VERSION_MISSING", "tauri.conf.json must define version.")
    _require(blockers, bool(config.get("identifier")), "TAURI_IDENTIFIER_MISSING", "tauri.conf.json must define identifier.")
    _require(blockers, build.get("beforeBuildCommand") == "npm run build", "TAURI_BUILD_COMMAND_MISMATCH", "Tauri beforeBuildCommand must run the desktop production build.")
    _require(blockers, build.get("frontendDist") == "../dist", "TAURI_FRONTEND_DIST_MISMATCH", "Tauri frontendDist must point to the desktop dist folder.")
    _require(blockers, bundle.get("active") is True, "TAURI_BUNDLE_DISABLED", "Tauri bundle.active must be true.")
    _require(blockers, bool(bundle.get("targets")), "TAURI_BUNDLE_TARGETS_MISSING", "Tauri bundle.targets must be configured.")
    _require(blockers, bool(security.get("csp")), "TAURI_CSP_MISSING", "Tauri production CSP must not be disabled.")
    _require(blockers, capabilities_dir.exists() and any(capabilities_dir.glob("*.json")), "TAURI_CAPABILITIES_MISSING", "Tauri capabilities JSON must exist.")
    _require(blockers, cargo_toml_path.exists(), "CARGO_TOML_MISSING", "Tauri Cargo.toml must exist.")
    _require(blockers, cargo_lock_path.exists(), "CARGO_LOCK_MISSING", "Tauri release packaging must commit Cargo.lock.")

    icons = bundle.get("icon") if isinstance(bundle.get("icon"), list) else []
    icon_missing = [icon for icon in icons if not (TAURI_DIR / icon).exists()]
    _require(blockers, bool(icons), "TAURI_ICONS_MISSING", "Tauri production bundle must define application icons.")
    _require(blockers, not icon_missing, "TAURI_ICON_FILE_MISSING", "Configured Tauri icon files must exist.", {"icons": icon_missing})

    external_bins = bundle.get("externalBin") if isinstance(bundle.get("externalBin"), list) else []
    _require(
        blockers,
        "binaries/wushen-agent" in external_bins,
        "SIDECAR_EXTERNAL_BIN_MISSING",
        "Tauri bundle.externalBin must include binaries/wushen-agent for the packaged Agent sidecar.",
    )
    sidecar_candidates = _sidecar_candidates(external_bins)
    _require(
        blockers,
        bool(sidecar_candidates),
        "SIDECAR_BINARY_MISSING",
        "No target-suffixed packaged Agent sidecar binary was found under src-tauri/binaries.",
        {"expected_base": "binaries/wushen-agent"},
    )

    main_rs = main_rs_path.read_text(encoding="utf-8")
    _require(
        blockers,
        "packaged-sidecar" in main_rs,
        "SIDECAR_MODE_NOT_IMPLEMENTED",
        "Rust supervisor must expose a packaged-sidecar runtime mode before production release.",
    )
    _require(
        blockers,
        "sidecar" in main_rs.lower(),
        "SIDECAR_SPAWN_NOT_IMPLEMENTED",
        "Rust supervisor must start the packaged sidecar rather than only a repo-local Python process.",
    )
    if "local-dev-python" in main_rs:
        warnings.append(
            {
                "severity": "warning",
                "code": "LOCAL_DEV_PYTHON_PRESENT",
                "message": "local-dev-python fallback remains present; release builds must prefer packaged-sidecar.",
            }
        )
    if "WUSHEN_AGENT_PYTHON" in main_rs or "WUSHEN_REPO_ROOT" in main_rs:
        warnings.append(
            {
                "severity": "warning",
                "code": "DEV_OVERRIDE_PRESENT",
                "message": "Development override hooks remain present and must not be the production startup path.",
            }
        )

    docs_summary = _check_docs(blockers)

    summaries["tauri"] = {
        "productName": config.get("productName"),
        "identifier": config.get("identifier"),
        "bundle_active": bundle.get("active"),
        "targets": bundle.get("targets"),
        "icons": icons,
        "externalBin": external_bins,
        "sidecar_candidates": [str(path.relative_to(TAURI_DIR)) for path in sidecar_candidates],
        "cargo_lock": cargo_lock_path.exists(),
        "csp": bool(security.get("csp")),
        "capabilities": len(list(capabilities_dir.glob("*.json"))) if capabilities_dir.exists() else 0,
    }
    summaries["docs"] = docs_summary

    report = {"ok": not blockers, "summaries": summaries, "blockers": blockers, "warnings": warnings}
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if not blockers else 1


def _sidecar_candidates(external_bins: list[Any]) -> list[Path]:
    candidates: list[Path] = []
    for entry in external_bins:
        if not isinstance(entry, str):
            continue
        base = TAURI_DIR / entry
        candidates.extend(path for path in base.parent.glob(base.name + "-*") if path.is_file())
    return sorted(candidates)


def _check_docs(blockers: list[dict[str, Any]]) -> dict[str, Any]:
    summary: dict[str, Any] = {}
    for rel_path, phrases in REQUIRED_DOC_PHRASES.items():
        path = ROOT / rel_path
        if not path.exists():
            blockers.append(
                {
                    "severity": "blocker",
                    "code": "PACKAGING_DOC_MISSING",
                    "message": f"{rel_path} is required for packaging release readiness.",
                }
            )
            summary[rel_path] = {"exists": False, "missing_phrases": phrases}
            continue
        text = path.read_text(encoding="utf-8")
        missing = [phrase for phrase in phrases if phrase not in text]
        if missing:
            blockers.append(
                {
                    "severity": "blocker",
                    "code": "PACKAGING_DOC_GAP",
                    "message": f"{rel_path} is missing required packaging phrase(s).",
                    "details": {"phrases": missing},
                }
            )
        summary[rel_path] = {"exists": True, "missing_phrases": missing}
    return summary


def _require(
    blockers: list[dict[str, Any]],
    condition: bool,
    code: str,
    message: str,
    details: Optional[dict[str, Any]] = None,
) -> None:
    if condition:
        return
    finding: dict[str, Any] = {"severity": "blocker", "code": code, "message": message}
    if details:
        finding["details"] = details
    blockers.append(finding)


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    raise SystemExit(main())
