#!/usr/bin/env python3
"""Ensure the hard-cut source tree contains no credentials or model keys."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKIP = {".git", "node_modules", "target", "dist", ".playwright-cli", ".venv", ".pytest_cache", ".ruff_cache"}
PATTERNS = [
    re.compile(r"(?i)(api[_-]?key|secret|token)\s*[:=]\s*[\"'][^\"']{12,}[\"']"),
    re.compile(r"(?i)sk-[a-z0-9]{16,}"),
]


def main() -> int:
    violations: list[str] = []
    for directory, directories, filenames in __import__("os").walk(ROOT):
        directories[:] = [name for name in directories if name not in SKIP]
        for filename in filenames:
            path = Path(directory) / filename
            if not path.is_file():
                continue
            if path.suffix not in {".rs", ".ts", ".tsx", ".js", ".mjs", ".json", ".toml", ".py", ".md"}:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            if any(pattern.search(text) for pattern in PATTERNS):
                violations.append(str(path.relative_to(ROOT)))
    if violations:
        raise SystemExit("possible secret literals: " + ", ".join(violations))
    print("release secrets-files OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
