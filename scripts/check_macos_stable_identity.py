#!/usr/bin/env python3
"""Report ForgeCAD's macOS code-signing identity without Keychain access."""

from __future__ import annotations

import argparse
from dataclasses import asdict
import json
from pathlib import Path

from macos_stable_app_identity import inspect_stable_app_identity
from smoke_packaged_tauri_alpha import APP_BUNDLE


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--require-ready", action="store_true")
    parser.add_argument("--app", type=Path, default=APP_BUNDLE)
    arguments = parser.parse_args()
    evidence = inspect_stable_app_identity(arguments.app.expanduser())
    report = {
        "schema_version": "ForgeCADMacOsStableIdentityCheck@1",
        **asdict(evidence),
        "keychain_reads": 0,
        "network_calls": 0,
    }
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 1 if arguments.require_ready and not evidence.ready else 0


if __name__ == "__main__":
    raise SystemExit(main())
