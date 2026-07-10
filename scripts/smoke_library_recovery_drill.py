#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import sqlite3
import tempfile
from pathlib import Path

from concept_module_pack import import_module_pack, validate_module_pack
from library_recovery_drill import (
    REPORT_NAME,
    REPORT_SCHEMA,
    RecoveryDrillError,
    _classify_evidence,
    run_recovery_drill,
)
from smoke_r2_concept_projects import (
    _assert,
    _create_body,
    _free_port,
    _json_request,
    _start_agent,
    _stop_agent,
    _wait_for_health,
)
from smoke_r3_module_pack_tooling import _reference_graph


ROOT = Path(__file__).resolve().parents[1]
REFERENCE_PACK = ROOT / "assets" / "module-packs" / "weapon-concept-v1-reference"


def main() -> int:
    with tempfile.TemporaryDirectory(
        prefix="forgecad_library_recovery_drill_"
    ) as temporary_directory:
        temporary_root = Path(temporary_directory)
        library_root = temporary_root / "ForgeCADLibrary"
        _create_reference_library(library_root)
        orphan_payload = b"reference-library-unreferenced-candidate"
        _write_orphan(library_root, orphan_payload)

        nested_guard = _expect_error(
            "DRILL_DESTINATION_INSIDE_LIBRARY",
            lambda: run_recovery_drill(
                library_root,
                library_root / "invalid-drill",
            ),
        )
        existing_output = temporary_root / "existing-drill"
        existing_output.mkdir()
        overwrite_guard = _expect_error(
            "DRILL_DESTINATION_EXISTS",
            lambda: run_recovery_drill(library_root, existing_output),
        )

        output = temporary_root / "reference-drill"
        result = run_recovery_drill(
            library_root,
            output,
            repeats=2,
            evidence_class="reference_fixture",
        )
        report_text = (output / REPORT_NAME).read_text(encoding="utf-8")
        report = json.loads(report_text)
        _assert(report["schema_version"] == REPORT_SCHEMA, "drill schema mismatch")
        _assert(result["drill_id"] == report["drill_id"], "drill id mismatch")
        _assert(len(report["runs"]) == 2, "drill repeat count mismatch")
        _assert(not (output / "runs").exists(), "ephemeral drill artifacts remained")
        _assert(
            sorted(path.name for path in output.iterdir()) == [REPORT_NAME],
            "drill output contained an unmanifested artifact",
        )
        _assert(
            str(library_root) not in report_text,
            "drill report exposed the source Library absolute path",
        )

        capacity = report["summary"]["capacity"]
        _assert(capacity["reference_rows"] == 10, "reference row count mismatch")
        _assert(capacity["unique_object_count"] == 10, "object count mismatch")
        _assert(
            capacity["source_object_store_file_count"] == 11,
            "source object count mismatch",
        )
        _assert(
            capacity["unreferenced_candidate_count"] == 1
            and capacity["unreferenced_candidate_bytes"] == len(orphan_payload),
            "unreferenced capacity mismatch",
        )
        _assert(
            report["summary"]["stable_source_snapshot"] is True,
            "source stability was not proven",
        )
        for run in report["runs"]:
            durations = run["duration_ms"]
            _assert(
                all(float(value) > 0 for value in durations.values()),
                "drill duration was not measured",
            )
            _assert(
                all(
                    int(value) > 0
                    for value in run["throughput_bytes_per_second"].values()
                ),
                "drill throughput was not measured",
            )
            _assert(
                run["source_snapshot"]["table_counts"]["module_assets"] == 10,
                "module table count mismatch",
            )
            _assert(
                run["source_snapshot"]["table_counts"]["module_connectors"] == 17,
                "connector table count mismatch",
            )
            _assert(
                run["source_snapshot"]["table_counts"]["module_graphs"] == 1,
                "graph table count mismatch",
            )
            readback = run["agent_readback"]
            _assert(readback["project_count"] == 1, "project readback mismatch")
            _assert(
                readback["project_version_count"] == 1,
                "version readback mismatch",
            )
            _assert(readback["module_count"] == 10, "module readback mismatch")
            _assert(
                readback["module_download_count"] == 10
                and readback["module_hashes_verified"] is True,
                "module payload readback mismatch",
            )
            _assert(
                readback["known_fixture_module_count"] == 10,
                "known reference generator was not detected",
            )

        _assert(
            report["evidence"]["formal_asset_evidence_eligible"] is False,
            "reference fixture was treated as formal evidence",
        )
        formal_guard = _expect_error(
            "FORMAL_ASSET_EVIDENCE_REJECTED",
            lambda: _classify_evidence(
                "formal_blender_10_12", report["runs"][0]["agent_readback"]
            ),
        )

        comparison_output = temporary_root / "reference-drill-comparison"
        comparison = run_recovery_drill(
            library_root,
            comparison_output,
            repeats=1,
            evidence_class="reference_fixture",
            baseline_report=output / REPORT_NAME,
            retain_artifacts=True,
        )
        comparison_report = json.loads(
            (comparison_output / REPORT_NAME).read_text(encoding="utf-8")
        )
        _assert(
            comparison_report["configuration"]["artifacts_retained"] is True,
            "retain-artifacts configuration was not recorded",
        )
        _assert(
            (comparison_output / "runs" / "run-001" / "backup").is_dir()
            and (comparison_output / "runs" / "run-001" / "restored-library").is_dir(),
            "explicitly retained drill artifacts are missing",
        )
        growth = comparison["summary"]["baseline_growth"]
        _assert(growth is not None, "baseline growth was not recorded")
        _assert(
            all(value == 0 for value in growth["capacity_delta"].values()),
            "unchanged reference library reported capacity growth",
        )
        _assert(
            len(growth["baseline_report_sha256"]) == 64,
            "baseline report hash missing",
        )

        print(
            json.dumps(
                {
                    "ok": True,
                    "schema_version": REPORT_SCHEMA,
                    "repeats": 2,
                    "capacity": capacity,
                    "duration_ms": report["summary"]["duration_ms"],
                    "agent_readback": report["runs"][0]["agent_readback"],
                    "baseline_capacity_delta": growth["capacity_delta"],
                    "nested_output_guard": nested_guard,
                    "overwrite_guard": overwrite_guard,
                    "formal_fixture_guard": formal_guard,
                    "ephemeral_artifact_cleanup": True,
                    "explicit_artifact_retention": True,
                    "source_path_excluded_from_report": True,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _create_reference_library(library_root: Path) -> None:
    reference_pack = validate_module_pack(REFERENCE_PACK, release=True)
    _assert(len(reference_pack.modules) == 10, "reference pack count mismatch")
    port = _free_port()
    base_url = f"http://127.0.0.1:{port}"
    process = _start_agent(library_root, port)
    try:
        _wait_for_health(base_url, process)
        imported = import_module_pack(reference_pack, base_url)
        _assert(len(imported) == 10, "reference pack import mismatch")
        project = _json_request(
            base_url,
            "/api/v1/projects",
            method="POST",
            body=_create_body(),
            idempotency_key="library-drill-reference-project",
        )
        graph = _reference_graph(project["project_id"])
        validated = _json_request(
            base_url,
            f"/api/v1/module-graphs/{graph['graph_id']}/validate",
            method="POST",
            body={
                "client_request_id": "library-drill-reference-graph",
                "graph": graph,
                "persist": True,
            },
            idempotency_key="library-drill-reference-graph",
        )
        _assert(validated["valid"] is True, "reference graph did not validate")
    finally:
        _stop_agent(process)

    with sqlite3.connect(library_root / "library.db") as connection:
        counts = {
            table: int(
                connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
            )
            for table in (
                "projects",
                "project_versions",
                "module_assets",
                "module_graphs",
            )
        }
    _assert(
        counts
        == {
            "projects": 1,
            "project_versions": 1,
            "module_assets": 10,
            "module_graphs": 1,
        },
        "reference Library table counts mismatch",
    )


def _write_orphan(library_root: Path, payload: bytes) -> None:
    digest = hashlib.sha256(payload).hexdigest()
    path = (
        library_root / "objects" / "sha256" / digest[:2] / digest[2:4] / f"{digest}.bin"
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)


def _expect_error(code: str, action) -> bool:
    try:
        action()
    except RecoveryDrillError as exc:
        _assert(exc.code == code, f"expected {code}, found {exc.code}")
        return True
    raise AssertionError(f"expected {code}")


if __name__ == "__main__":
    raise SystemExit(main())
