#!/usr/bin/env python3
"""Static safety boundary for the model-free MCP001 release."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    for path in [ROOT / "apps" / "desktop" / "src-tauri" / "tauri.conf.json", ROOT / "package.json"]:
        text = path.read_text(encoding="utf-8").lower()
        for token in ("externalbin", "deepseek", "qwen", "dashscope", "providerregistry"):
            if token in text:
                raise SystemExit(f"release safety violation: {path.relative_to(ROOT)} contains {token}")
    print("release safety scope OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
