#!/usr/bin/env python3
"""Rebuild the mutable MCP010F current-source truth from a real MCP build.

The Stage 0 truth file intentionally contains both immutable historical
observation facts and a small mutable ``current_source`` projection.  This
command only refreshes that projection (plus the canonical hashes that cover
the two mutable files); it never edits a historical receipt.  The caller must
provide a freshly generated ``forgecad-mcp --tool-manifest-summary`` output.

The source gate invokes the explicit compatibility MCP build without
``FORGECAD_BUILD_COHORT_SHA256``
so this command requires a null build cohort in that source-only summary.  A
packaged/development cohort must not be silently frozen as source truth.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
TRUTH_PATH = ROOT / "docs/evidence/mcp010f/current-benchmark-truth.json"
SUMMARY_PATH = ROOT / "docs/evidence/mcp010f/source-tool-manifest-summary.json"
MANIFEST_PATH = ROOT / "packages/forgecad-contracts/manifest.json"
SCHEMA_ROOT = ROOT / "packages/forgecad-contracts/schemas"
MCP_SOURCE = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/compat_main.rs"
RUNTIME_SOURCE = ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/lib.rs"
VIEWER_SOURCE = ROOT / "apps/desktop/src/features/runtime-viewer/RuntimeViewer.tsx"
FIT_PLAN_SOURCE = ROOT / "scripts/build_mcp010f_fit_plan.py"
EVIDENCE_MANIFEST_PATH = ROOT / "docs/evidence/mcp010f/manifest.json"


def fail(message: str) -> None:
    raise SystemExit(f"MCP010F current-source rebuild refused: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    require(path.is_file(), f"missing input file: {path}")
    return sha256_bytes(path.read_bytes())


def canonical_hash(value: dict[str, Any]) -> str:
    payload = dict(value)
    payload.pop("canonical_sha256", None)
    encoded = json.dumps(
        payload,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return sha256_bytes(encoded)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read JSON {path}: {error}")
    require(isinstance(value, dict), f"expected JSON object: {path}")
    return value


def atomic_write(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temp_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temp_name, path)
    except Exception:
        # The temporary file is private to this command.  Leave it for the
        # normal OS cleanup path if an exceptional write occurs; importantly,
        # do not alter the previous truth file in that case.
        raise


def source_tool_names() -> tuple[list[str], list[str]]:
    checker_path = ROOT / "scripts/check_mcp010f_stage0_truth.py"
    spec = importlib.util.spec_from_file_location("mcp010f_stage0_checker", checker_path)
    require(spec is not None and spec.loader is not None, "cannot load Stage 0 checker")
    checker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checker)
    return checker.source_tool_names()


def validate_summary(summary: dict[str, Any]) -> None:
    expected_keys = {
        "build_cohort_sha256",
        "canonical_sha256",
        "read_count",
        "read_manifest_sha256",
        "read_names",
        "schema_version",
        "total_count",
        "write_count",
        "write_enabled_manifest_sha256",
        "write_names",
    }
    require(set(summary) == expected_keys, "compiled summary key set drifted")
    require(summary.get("schema_version") == "ForgeCADMcpToolManifestSummary@1", "compiled summary schema drifted")
    require(summary.get("build_cohort_sha256") is None, "source summary contains a development build cohort")
    read_names = summary.get("read_names")
    write_names = summary.get("write_names")
    require(isinstance(read_names, list) and all(isinstance(item, str) for item in read_names), "compiled read names invalid")
    require(isinstance(write_names, list) and all(isinstance(item, str) for item in write_names), "compiled write names invalid")
    require(read_names == sorted(set(read_names)), "compiled read names are duplicate or unsorted")
    require(write_names == sorted(set(write_names)), "compiled write names are duplicate or unsorted")
    require(set(read_names).isdisjoint(write_names), "compiled read/write names overlap")
    source_read, source_write = source_tool_names()
    require(read_names == source_read, "compiled read names differ from the independent source parser")
    require(write_names == source_write, "compiled write names differ from the independent source parser")
    require(summary.get("read_count") == len(read_names), "compiled read count is stale")
    require(summary.get("write_count") == len(write_names), "compiled write count is stale")
    require(summary.get("total_count") == len(read_names) + len(write_names), "compiled total count is stale")
    require(summary.get("canonical_sha256") == canonical_hash(summary), "compiled summary canonical hash mismatch")


def schema_content_set_sha256(schema_paths: list[Path]) -> str:
    rows = [
        {"path": path.name, "sha256": sha256_file(path)}
        for path in sorted(schema_paths, key=lambda item: item.name)
    ]
    encoded = json.dumps(rows, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return sha256_bytes(encoded)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--compiled-summary",
        required=True,
        type=Path,
        help="path to the summary emitted by the freshly built forgecad-mcp binary",
    )
    args = parser.parse_args()

    compiled_path = args.compiled_summary
    require(compiled_path.resolve() != SUMMARY_PATH.resolve(), "compiled summary must be a separate generated file")
    summary = load_json(compiled_path)
    validate_summary(summary)

    manifest = load_json(MANIFEST_PATH)
    declared = sorted(manifest.get("schemas", []))
    schema_paths = sorted(SCHEMA_ROOT.glob("*.json"), key=lambda item: item.name)
    actual = [path.name for path in schema_paths]
    require(declared == actual, "contract manifest and schema directory differ")

    truth = load_json(TRUTH_PATH)
    current = truth.get("current_source")
    require(isinstance(current, dict), "truth current_source is missing")
    contracts = current.get("contracts")
    mcp_tools = current.get("mcp_tools")
    policy = current.get("visible_view_policy")
    require(isinstance(contracts, dict), "truth current_source.contracts is missing")
    require(isinstance(mcp_tools, dict), "truth current_source.mcp_tools is missing")
    require(isinstance(policy, dict), "truth current_source.visible_view_policy is missing")

    summary_bytes = compiled_path.read_bytes()
    # Preserve the compiler's exact JSON serialization as the generated
    # current summary.  This makes the summary byte hash evidence of the
    # actual binary output rather than of a hand-normalized reconstruction.
    summary_payload = summary_bytes if summary_bytes.endswith(b"\n") else summary_bytes + b"\n"

    contracts.update(
        {
            "manifest_sha256": sha256_file(MANIFEST_PATH),
            "schema_content_set_sha256": schema_content_set_sha256(schema_paths),
            "schema_count": len(actual),
        }
    )
    mcp_tools.update(
        {
            "read_count": summary["read_count"],
            "read_manifest_sha256": summary["read_manifest_sha256"],
            "read_names": summary["read_names"],
            "source_path": "apps/desktop/src-tauri/crates/forgecad-mcp/src/compat_main.rs",
            "source_sha256": sha256_file(MCP_SOURCE),
            "summary_receipt_sha256": sha256_bytes(summary_payload),
            "total_count": summary["total_count"],
            "write_count": summary["write_count"],
            "write_enabled_manifest_sha256": summary["write_enabled_manifest_sha256"],
            "write_names": summary["write_names"],
        }
    )
    policy.update(
        {
            "runtime_source_sha256": sha256_file(RUNTIME_SOURCE),
            "viewer_projection_sha256": sha256_file(VIEWER_SOURCE),
            "fit_plan_projection_sha256": sha256_file(FIT_PLAN_SOURCE),
        }
    )
    truth["evidence_manifest"]["sha256"] = sha256_file(EVIDENCE_MANIFEST_PATH)
    truth["canonical_sha256"] = canonical_hash(truth)
    truth_payload = (json.dumps(truth, ensure_ascii=False, indent=2) + "\n").encode("utf-8")

    # All source, manifest, schema, and truth inputs have now been validated
    # and hashed.  Only after that do we replace either mutable projection;
    # a late input failure therefore cannot leave the pair half refreshed.
    atomic_write(SUMMARY_PATH, summary_payload)
    atomic_write(TRUTH_PATH, truth_payload)

    print(
        json.dumps(
            {
                "schema_version": "ForgeCADMCP010FCurrentSourceTruthRebuild@1",
                "status": "PASS_CURRENT_SOURCE_GENERATED",
                "compiled_summary_path": str(compiled_path),
                "summary_sha256": sha256_bytes(summary_payload),
                "runtime_source_sha256": sha256_file(RUNTIME_SOURCE),
                "schema_count": len(actual),
                "read_count": summary["read_count"],
                "write_count": summary["write_count"],
                "total_count": summary["total_count"],
                "historical_receipts_mutated": False,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
