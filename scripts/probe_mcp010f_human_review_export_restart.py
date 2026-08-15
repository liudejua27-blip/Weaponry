#!/usr/bin/env python3
"""Run dedicated MCP010C raw-stdio probes to gather MCP010F synthetic evidence.

This script reuses the existing MCP010C raw probe fixtures to prove:
  - fixed synthetic human-review transport fixture;
  - export/restart hash consistency.

It intentionally excludes user references and is scoped to synthetic-only evidence.
"""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True, help="Path to forgecad-mcp binary.")
    parser.add_argument("--runtime", type=Path, required=True, help="Path to forgecad-runtime binary.")
    parser.add_argument("--temp-root", type=Path, required=True, help="Root directory for temporary data roots.")
    parser.add_argument("--expected-build-cohort", help="Optional expected build cohort hash to assert.")
    parser.add_argument(
        "--run-human-review",
        action="store_true",
        help="Run synthetic human_visual_review_submit fixture path.",
    )
    parser.add_argument(
        "--run-export-restart",
        action="store_true",
        help="Run synthetic export_confirm + runtime restart + export replay hash check.",
    )
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--output", type=Path, required=True, help="Primary evidence output path.")
    return parser.parse_args()


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def run_raw_probe(args: argparse.Namespace, mode: str, data_root: Path, output_path: Path) -> dict:
    probe = Path(__file__).resolve().parents[1] / "scripts/probe_mcp010c_raw_stdio.py"
    output_path = output_path.expanduser().resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        "python3",
        str(probe),
        "--mcp",
        str(args.mcp),
        "--runtime",
        str(args.runtime),
        "--data-root",
        str(data_root),
        "--determinism-repeats",
        "2",
        "--evidence",
        str(output_path),
        "--timeout",
        str(args.timeout),
    ]
    if args.expected_build_cohort:
        cmd.extend(["--expected-build-cohort", args.expected_build_cohort])
    if mode == "human":
        cmd.append("--human-review")
    elif mode == "export":
        cmd.append("--export-restart")
    try:
        subprocess.run(
            cmd,
            check=True,
            cwd=str(Path(__file__).resolve().parents[1]),
            env=os.environ.copy(),
            text=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError as error:
        raise SystemExit(f"mcp010c raw probe for {mode} failed: {error.stderr or error.stdout}")
    try:
        return read_json(output_path)
    finally:
        output_path.unlink(missing_ok=True)


def main() -> int:
    args = parse_args()
    if not args.run_human_review and not args.run_export_restart:
        raise SystemExit("at least one of --run-human-review / --run-export-restart is required")
    if not args.mcp.is_file():
        raise SystemExit("mcp binary is missing")
    if not args.runtime.is_file():
        raise SystemExit("runtime binary is missing")
    args.temp_root.mkdir(parents=True, exist_ok=True)
    root = args.temp_root.resolve()

    human_receipt: dict | None = None
    export_receipt: dict | None = None
    raw_receipt_root = Path(__file__).resolve().parents[1] / "docs" / "evidence" / "mcp010f"
    raw_receipt_root.mkdir(parents=True, exist_ok=True)
    if args.run_human_review:
        human_root = root / "human-review"
        human_receipt = run_raw_probe(
            args,
            "human",
            human_root,
            raw_receipt_root / f".tmp-human-review-raw-{os.getpid()}.json",
        )
    if args.run_export_restart:
        export_root = root / "export-restart"
        export_receipt = run_raw_probe(
            args,
            "export",
            export_root,
            raw_receipt_root / f".tmp-export-restart-raw-{os.getpid()}.json",
        )

    status = "PASS_SYNTHETIC_TRANSPORT"
    limitations = []
    if human_receipt is not None:
        if human_receipt.get("human_review_receipt") != "PASS":
            status = "PARTIAL_SYNTHETIC_TRANSPORT"
            limitations.append("human_review_submit synthetic fixture did not return PASS")
    if export_receipt is not None:
        if (
            "export_restart_hash_evidence" not in export_receipt
            or not isinstance(export_receipt.get("export_restart_hash_evidence"), dict)
        ):
            status = "PARTIAL_SYNTHETIC_TRANSPORT"
            limitations.append("export/restart hash evidence was not captured")

    if args.run_export_restart and status == "PASS_SYNTHETIC_TRANSPORT" and export_receipt is not None:
        status = "PASS_SYNTHETIC_TRANSPORT_WITH_QUALITY_TARGET_NOT_MET"

    receipt = {
        "schema_version": "ForgeCADMCP010FHumanExportProbe@1",
        "task_id": "FGC-MCP010F",
        "status": status,
        "mode": {
            "human_review": bool(args.run_human_review),
            "export_restart": bool(args.run_export_restart),
        },
        "human_visual_review": "NOT_RUN",
        "formal_human_visual_review": "NOT_RUN",
        "synthetic_human_review_transport": "PASS" if human_receipt is not None else "NOT_RUN",
        "export_restart_hash": "PASS_SYNTHETIC_FIXTURE" if (export_receipt is not None and "export_restart_hash_evidence" in export_receipt) else "NOT_RUN",
        "human_review_receipt": human_receipt,
        "export_restart_receipt": (
            export_receipt.get("export_restart_hash_evidence") if export_receipt is not None else None
        ),
        "limitations": limitations or [
            "Synthetic-only MCP010C raw-stdio fixture proofs; user-reference and production formal human thresholds are still required."
        ],
        "persistent_user_data_touched": (
            bool(human_receipt.get("persistent_user_data_touched", False))
            or bool(export_receipt.get("persistent_user_data_touched", False)) if human_receipt or export_receipt else False
        ),
    }

    if args.output:
        output_path = args.output.expanduser().resolve()
        if not str(output_path).startswith(str((Path(__file__).resolve().parents[1] / "docs/evidence").resolve())):
            raise SystemExit("--output must be under docs/evidence")
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(receipt, ensure_ascii=False, sort_keys=True))
    return 0 if status.startswith("PASS") else 1


if __name__ == "__main__":
    raise SystemExit(main())
