#!/usr/bin/env python3
"""Build a deterministic packaged Viewer core-control smoke receipt for MCP010F.

This probe is intentionally conservative: it aggregates existing evidence that is
already available in-repo (`packaged-window` and `packaged-viewer-controls`).
It does not claim formal packaged UI E2E, VoiceOver accessibility, or human
review; those gates remain explicitly NOT_RUN.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--window-evidence",
        type=Path,
        default=ROOT / "docs/evidence/mcp010f/packaged-window-probe-20260812-contour.json",
        help="Packaged window smoke evidence JSON.",
    )
    parser.add_argument(
        "--controls-evidence",
        type=Path,
        default=ROOT / "docs/evidence/mcp010f/packaged-viewer-controls-20260812.json",
        help="Packaged AX core controls evidence JSON.",
    )
    parser.add_argument(
        "--dom-smoke-evidence",
        type=Path,
        default=ROOT / "docs/evidence/mcp010f/viewer-browser-dom-smoke-20260812.json",
        help="Vite browser DOM smoke evidence for non-packaged control parity.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional output path for the MCP010F packaged UI e2e receipt.",
    )
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    if not path.is_file():
        raise SystemExit(f"missing evidence file: {path}")
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise SystemExit(f"evidence must be JSON object: {path}")
    return value


def bool_value(value: Any) -> bool:
    if isinstance(value, bool):
        return value
    return bool(value)


def main() -> int:
    args = parse_args()
    window = load_json(args.window_evidence.expanduser())
    controls = load_json(args.controls_evidence.expanduser())
    dom = load_json(args.dom_smoke_evidence.expanduser())

    window_ok = bool_value(window.get("status", "").startswith("PASS"))
    control_ok = controls.get("status") == "PASS_PACKAGED_AX_CORE_CONTROLS"
    dom_ok = dom.get("status") == "PASS_ISOLATED_VITE_BROWSER_DOM_SMOKE"

    has_essential_controls = bool_value(
        controls.get("controls", {}).get("aov_material_id_selected")
        and controls.get("controls", {}).get("aov_home_to_beauty")
        and controls.get("controls", {}).get("compare_overlay")
        and controls.get("controls", {}).get("compare_flicker")
        and controls.get("controls", {}).get("contour_canvas")
        and controls.get("controls", {}).get("difference_heatmap")
        and controls.get("controls", {}).get("explosion_view")
    )

    status = "PASS_PACKAGED_UI_CORE_SMOKE"
    limitations = []
    if window.get("runtime_handoff_status") != "ready":
        status = "NOT_RUN"
        limitations.append("runtime handoff was not ready")
    if not window.get("window_count"):
        status = "NOT_RUN"
        limitations.append("runtime window was not observed")
    if not control_ok or not has_essential_controls:
        status = "NOT_RUN"
        limitations.append("core control evidence did not confirm required packaged controls")
    if not dom_ok:
        limitations.append("browser DOM smoke evidence missing expected control parity mark")

    receipt = {
        "schema_version": "ForgeCADMCP010FPackagedUIE2EProbe@1",
        "task_id": "FGC-MCP010F",
        "status": status,
        "packaged_viewer_ui_e2e": "NOT_RUN",
        "packaged_viewer_core_smoke": "PASS" if status.startswith("PASS") else "NOT_RUN",
        "packaged_window_status": window.get("status"),
        "packaged_window_count": window.get("window_count"),
        "packaged_window_first_size": window.get("windows", [{}])[0] if isinstance(window.get("windows"), list) else None,
        "runtime_handoff_status": window.get("runtime_handoff_status"),
        "control_surface_status": controls.get("status"),
        "aov_material_id_selected": controls.get("controls", {}).get("aov_material_id_selected"),
        "compare_modes": {
            "overlay": controls.get("controls", {}).get("compare_overlay"),
            "flicker": controls.get("controls", {}).get("compare_flicker"),
        },
        "reference_and_visual_quality": controls.get("reference_and_visual_quality"),
        "voiceover_formal_accessibility": controls.get("voiceover_formal_accessibility"),
        "browser_dom_smoke_status": dom.get("status"),
        "browser_dom_smoke_observed_aov_count": len((dom.get("observed") or {}).get("aov_names", [])),
        "browser_dom_smoke_compare_mode_count": len((dom.get("observed") or {}).get("compare_modes", [])),
        "packaged_viewer_core_controls": "PASS" if (control_ok and has_essential_controls and dom_ok) else "NOT_RUN",
        "viewer_accessibility_formal": "NOT_RUN",
        "limitations": limitations or [
            "Core-control/window/DOM smoke only; formal packaged UI E2E, VoiceOver accessibility and human review remain NOT_RUN."
        ],
        "persistent_user_data_touched": bool_value(window.get("persistent_user_data_touched", False))
        or bool_value(controls.get("persistent_user_data_touched", False))
        or bool_value(dom.get("persistent_user_data_touched", False)),
    }

    if args.output:
        destination = args.output.expanduser()
        if not str(destination).startswith(str((ROOT / "docs/evidence").resolve())):
            raise SystemExit("--output must be under docs/evidence")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(receipt, ensure_ascii=False, sort_keys=True))
    return 0 if status.startswith("PASS") else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (SystemExit, KeyboardInterrupt):
        raise
    except Exception as error:
        print(json.dumps({"schema_version": "ForgeCADMCP010FPackagedUIE2EProbe@1", "task_id": "FGC-MCP010F", "status": "FAIL", "reason": str(error)[:2000], "persistent_user_data_touched": False}, ensure_ascii=False, sort_keys=True))
        raise SystemExit(1)
