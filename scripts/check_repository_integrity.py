#!/usr/bin/env python3
"""Fail closed if legacy product code or model/provider calls return."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOTS = [
    ROOT / "apps" / "desktop" / "src",
    ROOT / "apps" / "desktop" / "src-tauri" / "src",
    ROOT / "apps" / "desktop" / "src-tauri" / "crates",
    ROOT / "apps" / "geometry-worker",
    ROOT / "apps" / "render-worker",
    ROOT / "packages" / "forgecad-contracts",
]
BANNED = ("deepseek", "qwen", "dashscope", "providerregistry", "provider registry", "api_first", "api-first", "forgecad-app-server", "fastapi", ":8000")


def main() -> int:
    forbidden_paths = [
        ROOT / "apps" / "agent",
        ROOT / "apps" / "desktop" / "src" / "features" / "cad-workbench",
        ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-app-server",
        ROOT / "apps" / "desktop" / "src-tauri" / "crates" / "forgecad-app-server-protocol",
        ROOT / "packages" / "concept-spec",
        ROOT / "packages" / "agent-skills",
    ]
    present = [str(path.relative_to(ROOT)) for path in forbidden_paths if path.exists()]
    if present:
        raise SystemExit(f"legacy paths present: {present}")

    violations: list[str] = []
    for root in SOURCE_ROOTS:
        if not root.exists():
            continue
        for path in root.rglob("*"):
            if not path.is_file() or path.suffix in {".png", ".jpg", ".webp", ".ico", ".icns"}:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore").lower()
            for token in BANNED:
                if token in text:
                    violations.append(f"{path.relative_to(ROOT)} contains {token}")
    if violations:
        raise SystemExit("production source integrity violations:\n" + "\n".join(violations))
    print("repository integrity OK: legacy source absent and model calls absent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
