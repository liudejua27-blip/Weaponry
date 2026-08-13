#!/usr/bin/env python3
"""Probe that the packaged ForgeCAD Viewer opens a real native window.

This is intentionally narrower than a UI/accessibility E2E test.  It starts
the supplied bundle executable against an already-ready isolated Runtime,
queries macOS CoreGraphics for the process-owned window, and then terminates
only the Viewer child.  It never opens SQLite/CAS and never sends a Runtime
write request.  Accessibility/DOM interaction remains a separate gate.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
EVIDENCE_ROOT = ROOT / "docs" / "evidence"


def fail(message: str) -> int:
    print(
        json.dumps(
            {
                "schema_version": "ForgeCADMCP010FPackagedWindowProbe@1",
                "task_id": "FGC-MCP010F",
                "status": "FAIL",
                "failure": message,
                "packaged_viewer_ui_e2e": "NOT_RUN",
                "accessibility_e2e": "NOT_RUN",
                "persistent_user_data_touched": False,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 1


def coregraphics_windows(pid: int) -> list[dict[str, Any]]:
    """Return sanitized on-screen windows owned by pid using macOS CoreGraphics."""

    if sys.platform != "darwin":
        raise RuntimeError("packaged window probe requires macOS")
    source = f'''
import CoreGraphics
import Foundation
let targetPid: Int32 = {pid}
let items = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for item in items {{
  let ownerPid = (item[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value ?? -1
  if ownerPid != targetPid {{ continue }}
  let title = item[kCGWindowName as String] as? String ?? ""
  let bounds = item[kCGWindowBounds as String] as? NSDictionary
  func value(_ key: String) -> Double {{ (bounds?[key] as? NSNumber)?.doubleValue ?? 0 }}
  print("WINDOW\\t" + title.replacingOccurrences(of: "\\t", with: " ") + "\\t" + String(value("X")) + "\\t" + String(value("Y")) + "\\t" + String(value("Width")) + "\\t" + String(value("Height")))
}}
'''
    result = subprocess.run(
        ["swift", "-e", source],
        check=False,
        capture_output=True,
        text=True,
        timeout=15,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "CoreGraphics query failed")
    windows: list[dict[str, Any]] = []
    for line in result.stdout.splitlines():
        fields = line.split("\t")
        if len(fields) != 6 or fields[0] != "WINDOW":
            continue
        windows.append(
            {
                "title": fields[1],
                "x": float(fields[2]),
                "y": float(fields[3]),
                "width": float(fields[4]),
                "height": float(fields[5]),
            }
        )
    return windows


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--viewer-executable", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--timeout", type=float, default=15.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    executable = args.viewer_executable.expanduser().resolve()
    data_root = args.data_root.expanduser().resolve()
    if not executable.is_file() or not os.access(executable, os.X_OK):
        return fail("Viewer executable is missing or not executable")
    ready = data_root / "ipc" / "ready.json"
    if not ready.is_file():
        return fail("isolated Runtime ready handoff is missing")
    try:
        handoff = json.loads(ready.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return fail(f"Runtime ready handoff is unreadable: {error}")
    if handoff.get("status") != "ready":
        return fail("isolated Runtime is not ready")

    environment = os.environ.copy()
    environment["FORGECAD_RUNTIME_DATA_DIR"] = str(data_root)
    process = subprocess.Popen(
        [str(executable)],
        env=environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    windows: list[dict[str, Any]] = []
    query_error: str | None = None
    deadline = time.monotonic() + args.timeout
    try:
        while time.monotonic() < deadline:
            try:
                windows = coregraphics_windows(process.pid)
            except (OSError, RuntimeError, subprocess.SubprocessError) as error:
                query_error = str(error)
                break
            if windows:
                break
            time.sleep(0.25)
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)

    matching = [window for window in windows if window["title"] == "ForgeCAD Runtime Viewer"]
    if not matching:
        return fail(query_error or "ForgeCAD Runtime Viewer window was not observed")
    if any(window["width"] < 1180 or window["height"] < 760 for window in matching):
        return fail("Viewer window is below the declared minimum size")

    result: dict[str, Any] = {
        "schema_version": "ForgeCADMCP010FPackagedWindowProbe@1",
        "task_id": "FGC-MCP010F",
        "status": "PASS_STRUCTURAL_WINDOW",
        "packaged_viewer_ui_e2e": "NOT_RUN",
        "accessibility_e2e": "NOT_RUN",
        "window_count": len(matching),
        "windows": matching,
        "runtime_handoff_status": "ready",
        "persistent_user_data_touched": False,
    }
    if args.evidence:
        destination = args.evidence.expanduser().resolve()
        try:
            destination.relative_to(EVIDENCE_ROOT.resolve())
        except ValueError as error:
            raise SystemExit("--evidence must be under docs/evidence") from error
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
