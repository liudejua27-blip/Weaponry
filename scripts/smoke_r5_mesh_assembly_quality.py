#!/usr/bin/env python3
from __future__ import annotations

import copy
import hashlib
import json
import sqlite3
import struct
import tempfile
from pathlib import Path

from concept_module_pack import import_module_pack, validate_module_pack
from forgecad_agent.application.combined_glb import read_glb, write_glb
from forgecad_agent.application.mesh_quality_inspector import (
    ModuleInspectionSource,
    inspect_concept_geometry,
    inspect_module_mesh,
)
from forgecad_agent.application.triangle_intersections import (
    inspect_triangle_mesh_intersection,
    triangles_intersect,
)
from forgecad_agent.domain.concepts.models import ModuleAssetManifest, ModuleGraph
from smoke_r2_concept_projects import (
    _assert,
    _create_body,
    _free_port,
    _json_request,
    _json_request_allow_error,
    _start_agent,
    _stop_agent,
    _wait_for_health,
)
from smoke_r3_module_pack_tooling import _reference_graph, _triangle_glb


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    mesh_negative_checks = _assert_mesh_negative_checks()
    exact_intersection_checks = _assert_exact_intersection_checks()
    connected_gap_checks = _assert_connected_gap_checks()
    with tempfile.TemporaryDirectory(
        prefix="forgecad_r5_quality_"
    ) as temporary_directory:
        library_root = Path(temporary_directory) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            pack = validate_module_pack(
                ROOT / "assets" / "module-packs" / "weapon-concept-v1-reference",
                release=True,
            )
            imported = import_module_pack(pack, base_url)
            _assert(len(imported) == 10, "reference module pack import failed")
            project = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=_create_body(),
                idempotency_key="r5-quality-project",
            )
            project_id = project["project_id"]
            graph = _reference_graph(project_id)
            _persist_graph(base_url, graph, "r5-quality-reference-graph")
            version_2 = _bind_graph(
                base_url,
                project,
                graph["graph_id"],
                "r5-quality-bind-reference",
            )

            request = {
                "client_request_id": "r5-quality-inspect-reference",
                "ruleset_version": "weapon-concept-geometry/1.2",
            }
            legacy_status, _legacy_error = _json_request_allow_error(
                base_url,
                f"/api/v1/versions/{version_2}/quality-runs:inspect",
                method="POST",
                body={
                    "client_request_id": "r5-quality-reject-legacy-ruleset",
                    "ruleset_version": "weapon-concept-geometry/1.1",
                },
                idempotency_key="r5-quality-reject-legacy-ruleset",
            )
            _assert(
                legacy_status == 422, "superseded 1.1 ruleset version was not rejected"
            )
            inspected = _json_request(
                base_url,
                f"/api/v1/versions/{version_2}/quality-runs:inspect",
                method="POST",
                body=request,
                idempotency_key="r5-quality-inspect-reference",
            )
            _assert(
                inspected["report"]["status"] == "warning",
                "reference report status mismatch",
            )
            checks = {item["check_id"] for item in inspected["report"]["findings"]}
            _assert(
                checks == {"assembly.unconnected_triangle_intersection"},
                f"reference meshes should pass mesh checks: {sorted(checks)}",
            )
            _assert(
                len(inspected["report"]["findings"]) == 2,
                "reference intersection count changed",
            )
            _assert(
                all(
                    "tested_pairs=" in item["measured_value"]
                    for item in inspected["report"]["findings"]
                ),
                "exact intersection evidence missing",
            )
            _assert(
                all(
                    len(item.get("geometry_refs", [])) == 2
                    and all(
                        reference.get("triangle_indices")
                        and len(reference["triangle_indices"])
                        == len(reference.get("world_triangles_mm", []))
                        for reference in item["geometry_refs"]
                    )
                    for item in inspected["report"]["findings"]
                ),
                "intersection triangle provenance missing",
            )
            quality_job = _json_request(
                base_url,
                f"/api/v1/jobs/{inspected['job_id']}",
                method="GET",
            )
            _assert(
                len(quality_job["events"]) == 4, "quality JobEvent timeline mismatch"
            )
            _assert(
                quality_job["events"][-1]["status"] == "succeeded", "quality job failed"
            )
            replay = _json_request(
                base_url,
                f"/api/v1/versions/{version_2}/quality-runs:inspect",
                method="POST",
                body=request,
                idempotency_key="r5-quality-inspect-reference",
            )
            _assert(
                replay["quality_run_id"] == inspected["quality_run_id"],
                "quality replay mismatch",
            )
            conflict_status, conflict = _json_request_allow_error(
                base_url,
                f"/api/v1/versions/{version_2}/quality-runs:inspect",
                method="POST",
                body={**request, "client_request_id": "different-request"},
                idempotency_key="r5-quality-inspect-reference",
            )
            _assert(
                conflict_status == 409
                and conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT",
                "quality idempotency conflict was not rejected",
            )

            misaligned_graph = copy.deepcopy(graph)
            misaligned_graph["graph_id"] = "mg_reference_pack_misaligned"
            front = next(
                node
                for node in misaligned_graph["nodes"]
                if node["node_id"] == "node_front"
            )
            front["transform"]["position"][0] += 5
            _persist_graph(base_url, misaligned_graph, "r5-quality-misaligned-graph")
            project_v2 = _json_request(
                base_url, f"/api/v1/projects/{project_id}", method="GET"
            )
            version_3 = _bind_graph(
                base_url,
                project_v2,
                misaligned_graph["graph_id"],
                "r5-quality-bind-misaligned",
            )
            failed = _json_request(
                base_url,
                f"/api/v1/versions/{version_3}/quality-runs:inspect",
                method="POST",
                body={
                    "client_request_id": "r5-quality-inspect-misaligned",
                    "ruleset_version": "weapon-concept-geometry/1.2",
                },
                idempotency_key="r5-quality-inspect-misaligned",
            )
            _assert(
                failed["report"]["status"] == "failed",
                "misaligned graph was not failed",
            )
            alignment = [
                item
                for item in failed["report"]["findings"]
                if item["check_id"] == "assembly.connector_alignment"
            ]
            _assert(len(alignment) == 1, "misaligned Connector finding missing")
            _assert(
                "5.000000 mm" in alignment[0]["measured_value"],
                "alignment distance mismatch",
            )
        except Exception as exc:
            _stop_agent(process)
            output = process.stdout.read() if process.stdout else ""
            raise RuntimeError(
                f"quality smoke failed: {exc}\nAgent output:\n{output}"
            ) from exc
        finally:
            if process.poll() is None:
                _stop_agent(process)

        with sqlite3.connect(library_root / "library.db") as connection:
            report_count = connection.execute(
                "SELECT COUNT(*) FROM quality_runs"
            ).fetchone()[0]
            finding_count = connection.execute(
                "SELECT COUNT(*) FROM quality_findings"
            ).fetchone()[0]
            geometry_ref_row_count = connection.execute(
                """
                SELECT COUNT(*)
                FROM quality_finding_geometry_refs refs
                JOIN quality_findings findings USING (finding_id)
                WHERE findings.quality_run_id = ?
                """,
                (inspected["quality_run_id"],),
            ).fetchone()[0]
        _assert(report_count == 2, "quality reports were not persisted exactly once")
        _assert(finding_count >= 5, "quality findings were not normalized")
        _assert(
            geometry_ref_row_count == 4,
            "intersection geometry references were not normalized exactly four times",
        )

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted)
            restored = _json_request(
                restart_url,
                f"/api/v1/quality-runs/{inspected['quality_run_id']}",
                method="GET",
            )
            restored_failed = _json_request(
                restart_url,
                f"/api/v1/quality-runs/{failed['quality_run_id']}",
                method="GET",
            )
            _assert(
                restored["report"]["status"] == "warning", "restart lost warning report"
            )
            _assert(
                restored["report"]["findings"][0]["geometry_refs"]
                == inspected["report"]["findings"][0]["geometry_refs"],
                "restart lost intersection triangle provenance",
            )
            _assert(
                restored_failed["report"]["status"] == "failed",
                "restart lost failed report",
            )
        finally:
            _stop_agent(restarted)

        print(
            json.dumps(
                {
                    "ok": True,
                    "ruleset_version": "weapon-concept-geometry/1.2",
                    "module_meshes_checked": 9,
                    "reference_mesh_checks_passed": True,
                    "mesh_negative_checks": mesh_negative_checks,
                    "exact_intersection_checks": exact_intersection_checks,
                    "connected_gap_checks": connected_gap_checks,
                    "intersection_geometry_refs_persisted": True,
                    "reference_intersection_warnings": 2,
                    "superseded_ruleset_rejected": True,
                    "connector_misalignment_failed": True,
                    "idempotent_replay": True,
                    "job_event_count": 4,
                    "report_count": report_count,
                    "finding_count": finding_count,
                    "geometry_ref_row_count": geometry_ref_row_count,
                    "restart_restored": True,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _assert_mesh_negative_checks() -> list[str]:
    payload = _triangle_glb("mat_quality_negative")
    document, binary = read_glb(payload)
    corrupted = bytearray(binary)
    corrupted[:36] = struct.pack("<9f", *([0.0] * 9))
    corrupted_payload = write_glb(document, bytes(corrupted))
    manifest_payload = {
        "schema_version": "ModuleAssetManifest@1",
        "module_id": "module_quality_negative",
        "pack_id": "pack_weapon_concept_v1",
        "category": "core_shell",
        "asset_id": "asset_quality_negative",
        "bounds_mm": [100.0, 50.0, 10.0],
        "triangle_count": 1,
        "material_slots": ["mat_quality_negative"],
        "connectors": [],
    }
    manifest = ModuleAssetManifest.model_validate(
        {
            **manifest_payload,
            "sha256": hashlib.sha256(corrupted_payload).hexdigest(),
        }
    )
    result = inspect_module_mesh(
        ModuleInspectionSource(
            node_id="node_quality_negative",
            manifest=manifest,
            payload=corrupted_payload,
        )
    )
    open_result = inspect_module_mesh(
        ModuleInspectionSource(
            node_id="node_quality_open",
            manifest=ModuleAssetManifest.model_validate(
                {
                    **manifest_payload,
                    "module_id": "module_quality_open",
                    "asset_id": "asset_quality_open",
                    "sha256": hashlib.sha256(payload).hexdigest(),
                }
            ),
            payload=payload,
        )
    )
    checks = sorted(
        {finding.check_id for finding in result.findings + open_result.findings}
    )
    required = {"mesh.degenerate_triangles", "mesh.normals", "mesh.boundary_edges"}
    _assert(
        required <= set(checks),
        f"mesh negative checks missing: {sorted(required - set(checks))}",
    )
    return checks


def _assert_exact_intersection_checks() -> dict[str, int | bool]:
    base = ((0.0, 0.0, 0.0), (2.0, 0.0, 0.0), (0.0, 2.0, 0.0))
    crossing = ((0.5, 0.5, -1.0), (0.5, 0.5, 1.0), (1.5, 0.5, 0.0))
    separated_coplanar = ((1.5, 1.5, 0.0), (3.0, 1.5, 0.0), (1.5, 3.0, 0.0))
    touching = ((2.0, 0.0, 0.0), (3.0, 0.0, 0.0), (2.0, 1.0, 0.0))
    skew_separated = ((0.5, 0.5, 1.0), (0.5, 1.5, 1.0), (1.5, 0.5, 1.0))
    _assert(triangles_intersect(base, crossing), "crossing triangles were missed")
    _assert(
        not triangles_intersect(base, separated_coplanar),
        "coplanar gap was a false positive",
    )
    _assert(triangles_intersect(base, touching), "touching triangles were missed")
    _assert(
        not triangles_intersect(base, skew_separated), "skew gap was a false positive"
    )

    outer = _cube_triangles((-2.0, -2.0, -2.0), (2.0, 2.0, 2.0))
    inner = _cube_triangles((-0.5, -0.5, -0.5), (0.5, 0.5, 0.5))
    containment = inspect_triangle_mesh_intersection(
        outer,
        inner,
        first_is_closed=True,
        second_is_closed=True,
    )
    _assert(
        containment.intersection_count == 0,
        "contained cubes should not have surface crossings",
    )
    _assert(containment.containment, "closed-mesh containment was missed")

    first_many = tuple(
        (
            ((index * 10.0), 0.0, 0.0),
            ((index * 10.0) + 1.0, 0.0, 0.0),
            ((index * 10.0), 1.0, 0.0),
        )
        for index in range(128)
    )
    second_many = tuple(
        (
            ((index * 10.0) + 5.0, 0.0, 0.0),
            ((index * 10.0) + 6.0, 0.0, 0.0),
            ((index * 10.0) + 5.0, 1.0, 0.0),
        )
        for index in range(128)
    )
    pruned = inspect_triangle_mesh_intersection(first_many, second_many)
    cartesian_pairs = len(first_many) * len(second_many)
    _assert(pruned.intersection_count == 0, "separated BVH corpus intersected")
    _assert(
        pruned.tested_triangle_pairs < cartesian_pairs // 100,
        "BVH did not prune the corpus",
    )
    return {
        "crossing": True,
        "coplanar_gap": True,
        "touching": True,
        "skew_gap": True,
        "containment": True,
        "bvh_cartesian_pairs": cartesian_pairs,
        "bvh_tested_pairs": pruned.tested_triangle_pairs,
    }


def _assert_connected_gap_checks() -> dict[str, float | bool]:
    first_payload = _triangle_glb("mat_gap_first")
    document, binary = read_glb(_triangle_glb("mat_gap_second"))
    shifted = bytearray(binary)
    values = list(struct.unpack_from("<9f", shifted, 0))
    for offset in range(0, 9, 3):
        values[offset] += 0.2
    shifted[:36] = struct.pack("<9f", *values)
    second_payload = write_glb(document, bytes(shifted))
    identity = {
        "position": [0.0, 0.0, 0.0],
        "rotation": [0.0, 0.0, 0.0],
        "scale": [1.0, 1.0, 1.0],
    }
    first_manifest = ModuleAssetManifest.model_validate(
        {
            "module_id": "module_gap_first",
            "pack_id": "pack_weapon_concept_v1",
            "category": "core_shell",
            "asset_id": "asset_gap_first",
            "sha256": hashlib.sha256(first_payload).hexdigest(),
            "bounds_mm": [100.0, 50.0, 10.0],
            "triangle_count": 1,
            "material_slots": ["mat_gap_first"],
            "connectors": [
                {
                    "connector_id": "connector_gap_first",
                    "slot": "core.front",
                    "connector_type": "surface_male",
                    "transform": identity,
                    "scale_range": [0.8, 1.2],
                }
            ],
        }
    )
    second_manifest = ModuleAssetManifest.model_validate(
        {
            "module_id": "module_gap_second",
            "pack_id": "pack_weapon_concept_v1",
            "category": "front_shell",
            "asset_id": "asset_gap_second",
            "sha256": hashlib.sha256(second_payload).hexdigest(),
            "bounds_mm": [100.0, 50.0, 10.0],
            "triangle_count": 1,
            "material_slots": ["mat_gap_second"],
            "connectors": [
                {
                    "connector_id": "connector_gap_second",
                    "slot": "front.core",
                    "connector_type": "surface_female",
                    "transform": identity,
                    "scale_range": [0.8, 1.2],
                }
            ],
        }
    )
    graph = ModuleGraph.model_validate(
        {
            "graph_id": "mg_connected_surface_gap_truth",
            "project_id": "prj_connected_surface_gap_truth",
            "root_node_id": "node_gap_first",
            "nodes": [
                {
                    "node_id": "node_gap_first",
                    "module_id": first_manifest.module_id,
                    "transform": identity,
                },
                {
                    "node_id": "node_gap_second",
                    "module_id": second_manifest.module_id,
                    "transform": identity,
                },
            ],
            "edges": [
                {
                    "edge_id": "edge_connected_surface_gap",
                    "from_node_id": "node_gap_first",
                    "from_connector_id": "connector_gap_first",
                    "to_node_id": "node_gap_second",
                    "to_connector_id": "connector_gap_second",
                    "status": "connected",
                }
            ],
        }
    )
    findings = inspect_concept_geometry(
        graph=graph,
        sources=[
            ModuleInspectionSource("node_gap_first", first_manifest, first_payload),
            ModuleInspectionSource("node_gap_second", second_manifest, second_payload),
        ],
    )
    alignment = [
        item for item in findings if item.check_id == "assembly.connector_alignment"
    ]
    gaps = [item for item in findings if item.check_id == "assembly.connected_surface_gap"]
    _assert(not alignment, "gap truth set unexpectedly failed Connector alignment")
    _assert(len(gaps) == 1, "connected surface gap was not detected exactly once")
    distance = float(gaps[0].measured_value)
    _assert(abs(distance - 100.0) <= 0.001, "connected gap distance mismatch")
    return {"connector_aligned": True, "gap_detected": True, "distance_mm": distance}


def _cube_triangles(
    minimum: tuple[float, float, float],
    maximum: tuple[float, float, float],
) -> tuple[tuple[tuple[float, float, float], ...], ...]:
    x0, y0, z0 = minimum
    x1, y1, z1 = maximum
    vertices = (
        (x0, y0, z0),
        (x1, y0, z0),
        (x1, y1, z0),
        (x0, y1, z0),
        (x0, y0, z1),
        (x1, y0, z1),
        (x1, y1, z1),
        (x0, y1, z1),
    )
    faces = (
        (0, 2, 1),
        (0, 3, 2),
        (4, 5, 6),
        (4, 6, 7),
        (0, 1, 5),
        (0, 5, 4),
        (3, 7, 6),
        (3, 6, 2),
        (0, 4, 7),
        (0, 7, 3),
        (1, 2, 6),
        (1, 6, 5),
    )
    return tuple(tuple(vertices[index] for index in face) for face in faces)


def _persist_graph(base_url: str, graph: dict, key: str) -> None:
    response = _json_request(
        base_url,
        f"/api/v1/module-graphs/{graph['graph_id']}/validate",
        method="POST",
        body={"client_request_id": key, "graph": graph, "persist": True},
        idempotency_key=key,
    )
    _assert(
        response["valid"] is True and response["persisted"] is True,
        "graph persistence failed",
    )


def _bind_graph(base_url: str, project: dict, graph_id: str, key: str) -> str:
    response = _json_request(
        base_url,
        f"/api/v1/projects/{project['project_id']}/versions",
        method="POST",
        body={
            "client_request_id": key,
            "parent_version_id": project["current_version_id"],
            "summary": f"绑定质量检查图 {graph_id}。",
            "spec": project["current_spec"],
            "module_graph_id": graph_id,
        },
        idempotency_key=key,
    )
    return response["current_version_id"]


if __name__ == "__main__":
    raise SystemExit(main())
