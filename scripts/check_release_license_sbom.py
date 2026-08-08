#!/usr/bin/env python3
"""MCP002 license/SBOM gate: dependencies must be inventoryable before adoption."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    ledger = ROOT / "docs" / "THIRD_PARTY_LICENSES.md"
    manifest = ROOT / "packages" / "forgecad-contracts" / "manifest.json"
    cargo = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
    for path in (ledger, manifest, cargo):
        if not path.exists():
            raise SystemExit(f"license/SBOM input missing: {path.relative_to(ROOT)}")
    data = json.loads(manifest.read_text(encoding="utf-8"))
    if data.get("sbom_status") != "tracked-before-adoption":
        raise SystemExit("contract manifest must declare SBOM status")
    if "MCP002" not in ledger.read_text(encoding="utf-8"):
        raise SystemExit("license ledger does not mention MCP002")
    print("release license-sbom OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
