#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import io
import json
import sqlite3
import struct
import tempfile
import urllib.request
import zipfile
import zlib
from pathlib import Path

from forgecad_agent.application.combined_glb import (
    CombinedGlbError,
    CombinedGlbSource,
    build_combined_glb,
    read_glb,
    write_glb,
)
from forgecad_agent.application.combined_obj import CombinedObjError, build_combined_obj
from forgecad_agent.application.concept_renderer import render_concept_pngs
from forgecad_agent.domain.concepts.models import ModuleGraph, Transform

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
from smoke_r2_module_registry import (
    _connector,
    _graph,
    _manifest,
    _register,
)
from smoke_r3_module_pack_tooling import _triangle_glb


def main() -> int:
    with tempfile.TemporaryDirectory(
        prefix="forgecad_r2_exports_"
    ) as temporary_directory:
        library_root = Path(temporary_directory) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            project = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=_create_body(),
                idempotency_key="r2-export-project",
            )
            project_id = project["project_id"]
            core_payload = _triangle_glb("export_core")
            front_payload = _triangle_glb("export_front")
            _register(
                base_url,
                _manifest(
                    module_id="module_core_shell_01",
                    asset_id="asset_core_shell_01",
                    category="core_shell",
                    payload=core_payload,
                    connectors=[
                        _connector("connector_core_front", "core.front", "shell_mount")
                    ],
                ),
                core_payload,
                "packs/weapon-concept/core-shell-01.glb",
                "r2-export-core",
            )
            _register(
                base_url,
                _manifest(
                    module_id="module_front_shell_01",
                    asset_id="asset_front_shell_01",
                    category="front_shell",
                    payload=front_payload,
                    connectors=[
                        _connector("connector_front_core", "front.core", "shell_mount")
                    ],
                ),
                front_payload,
                "packs/weapon-concept/front-shell-01.glb",
                "r2-export-front",
            )
            graph = _graph(project_id)
            _json_request(
                base_url,
                f"/api/v1/module-graphs/{graph['graph_id']}/validate",
                method="POST",
                body={
                    "client_request_id": "r2-export-graph",
                    "graph": graph,
                    "persist": True,
                },
                idempotency_key="r2-export-graph",
            )
            bound = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body={
                    "client_request_id": "r2-export-bind",
                    "parent_version_id": project["current_version_id"],
                    "summary": "绑定用于概念导出的模块图。",
                    "spec": project["current_spec"],
                    "module_graph_id": graph["graph_id"],
                },
                idempotency_key="r2-export-bind",
            )
            version_id = bound["current_version_id"]
            quality_report = _quality_report(project_id, version_id)
            _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/quality-runs",
                method="POST",
                body={
                    "client_request_id": "r2-export-quality",
                    "report": quality_report,
                },
                idempotency_key="r2-export-quality",
            )

            export_body = {
                "client_request_id": "r2-export-create",
                "profile": "game_asset",
                "include_modules": True,
                "include_combined_glb": True,
                "include_combined_obj": True,
                "include_render_png": True,
                "include_quality_report": True,
            }
            created = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/exports",
                method="POST",
                body=export_body,
                idempotency_key="r2-export-create",
            )
            _assert(created["status"] == "validated", "export status mismatch")
            _assert(
                created["manifest"]["non_functional_only"] is True,
                "export boundary missing",
            )
            _assert(
                len(created["manifest"]["modules"]) == 2,
                "module manifest count mismatch",
            )
            _assert(
                created["manifest"]["quality_report_id"] == quality_report["report_id"],
                "quality report was not linked",
            )
            job = _json_request(
                base_url, f"/api/v1/jobs/{created['job_id']}", method="GET"
            )
            _assert(job["status"] == "succeeded", "export job status mismatch")
            render_event = next(
                event for event in job["events"] if event["step"] == "render"
            )
            _assert(
                render_event["metadata"]["exploded_distance_m"] > 0,
                "export render event lost exploded distance",
            )
            _assert(
                job["events"][-1]["artifact_asset_id"] == created["package_asset_id"],
                "export artifact was not linked from JobEvent@2",
            )

            package_payload = _download(base_url, created["export_id"])
            _assert(
                hashlib.sha256(package_payload).hexdigest()
                == created["package_sha256"],
                "downloaded export hash mismatch",
            )
            _assert(
                len(package_payload) == created["package_byte_size"],
                "export size mismatch",
            )
            with zipfile.ZipFile(io.BytesIO(package_payload)) as archive:
                names = set(archive.namelist())
                required = {
                    "Manifest/concept-export-manifest.json",
                    "Specs/weapon-concept-spec.json",
                    "Graphs/module-graph.json",
                    "Modules/node_core.glb",
                    "Modules/node_front.glb",
                    "Model/combined.glb",
                    "Model/combined.obj",
                    "Model/combined.mtl",
                    "Renders/preview.png",
                    "Renders/exploded.png",
                    "Quality/model-quality-report.json",
                    "README.txt",
                }
                _assert(
                    required <= names,
                    f"export package files missing: {sorted(required - names)}",
                )
                manifest = json.loads(
                    archive.read("Manifest/concept-export-manifest.json")
                )
                _assert(manifest == created["manifest"], "ZIP manifest mismatch")
                _assert(
                    archive.read("Modules/node_core.glb") == core_payload,
                    "core GLB changed",
                )
                _assert(
                    archive.read("Modules/node_front.glb") == front_payload,
                    "front GLB changed",
                )
                combined_payload = archive.read("Model/combined.glb")
                combined_document, _ = read_glb(combined_payload)
                wrapper_names = {
                    node.get("name")
                    for node in combined_document["nodes"]
                    if str(node.get("name", "")).startswith("NODE_")
                }
                _assert(
                    wrapper_names
                    == {
                        "NODE_node_core__module_core_shell_01",
                        "NODE_node_front__module_front_shell_01",
                    },
                    "combined GLB wrapper nodes mismatch",
                )
                _assert(
                    hashlib.sha256(combined_payload).hexdigest()
                    == created["combined_glb_sha256"],
                    "combined GLB hash mismatch",
                )
                _assert(
                    len(combined_payload) == created["combined_glb_byte_size"],
                    "combined GLB size mismatch",
                )
                combined_obj = archive.read("Model/combined.obj")
                combined_mtl = archive.read("Model/combined.mtl")
                obj_text = combined_obj.decode("utf-8")
                _assert(
                    "# units: meter" in obj_text,
                    "combined OBJ unit declaration missing",
                )
                _assert(
                    "o NODE_node_core__module_core_shell_01__module_root" in obj_text,
                    "combined OBJ lost stable core node name",
                )
                _assert(
                    obj_text.count("\nv ") == 6, "combined OBJ vertex count mismatch"
                )
                _assert(obj_text.count("\nf ") == 2, "combined OBJ face count mismatch")
                _assert(
                    b"newmtl mat_000_export_core" in combined_mtl,
                    "combined MTL missing",
                )
                _assert(
                    hashlib.sha256(combined_obj).hexdigest()
                    == created["combined_obj_sha256"],
                    "combined OBJ hash mismatch",
                )
                _assert(
                    len(combined_obj) == created["combined_obj_byte_size"],
                    "combined OBJ size mismatch",
                )
                preview_png = archive.read("Renders/preview.png")
                exploded_png = archive.read("Renders/exploded.png")
                preview_stats = _png_stats(preview_png)
                exploded_stats = _png_stats(exploded_png)
                _assert(
                    preview_stats[0:2] == (640, 640), "preview PNG dimensions mismatch"
                )
                _assert(
                    exploded_stats[0:2] == (640, 640),
                    "exploded PNG dimensions mismatch",
                )
                _assert(
                    preview_stats[2] > 0 and preview_stats[3] > 0,
                    "preview transparency mismatch",
                )
                _assert(
                    exploded_stats[2] > 0 and exploded_stats[3] > 0,
                    "exploded transparency mismatch",
                )
                _assert(
                    preview_png != exploded_png,
                    "preview and exploded PNG are identical",
                )
                _assert(
                    hashlib.sha256(preview_png).hexdigest()
                    == created["preview_png_sha256"],
                    "preview PNG hash mismatch",
                )
                _assert(
                    hashlib.sha256(exploded_png).hexdigest()
                    == created["exploded_png_sha256"],
                    "exploded PNG hash mismatch",
                )
                _assert(
                    len(preview_png) == created["preview_png_byte_size"],
                    "preview PNG size mismatch",
                )
                _assert(
                    len(exploded_png) == created["exploded_png_byte_size"],
                    "exploded PNG size mismatch",
                )
                rerendered = render_concept_pngs(combined_payload)
                _assert(
                    rerendered.preview_png == preview_png,
                    "preview render is not deterministic",
                )
                _assert(
                    rerendered.exploded_png == exploded_png,
                    "exploded render is not deterministic",
                )
                for entry in manifest["files"]:
                    payload = archive.read(entry["path"])
                    _assert(
                        hashlib.sha256(payload).hexdigest() == entry["sha256"],
                        "file hash mismatch",
                    )
            _assert(
                _download_combined(base_url, created["export_id"]) == combined_payload,
                "direct combined GLB download mismatch",
            )
            _assert(
                _download_combined_obj(base_url, created["export_id"]) == combined_obj,
                "direct combined OBJ download mismatch",
            )
            _assert(
                _download_combined_mtl(base_url, created["export_id"]) == combined_mtl,
                "direct combined MTL download mismatch",
            )
            _assert(
                _download_png(base_url, created["export_id"], "preview") == preview_png,
                "direct preview PNG download mismatch",
            )
            _assert(
                _download_png(base_url, created["export_id"], "exploded")
                == exploded_png,
                "direct exploded PNG download mismatch",
            )
            _assert_unsupported_static_feature_rejected(
                ModuleGraph.model_validate(graph)
            )
            _assert_obj_transform_and_mirror(
                ModuleGraph.model_validate(graph), core_payload
            )

            replay = _json_request(
                base_url,
                f"/api/v1/versions/{version_id}/exports",
                method="POST",
                body=export_body,
                idempotency_key="r2-export-create",
            )
            _assert(
                replay["export_id"] == created["export_id"], "export replay mismatch"
            )
            conflict_status, conflict = _json_request_allow_error(
                base_url,
                f"/api/v1/versions/{version_id}/exports",
                method="POST",
                body={**export_body, "profile": "film_prop"},
                idempotency_key="r2-export-create",
            )
            _assert(
                conflict_status == 409
                and conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT",
                "export idempotency conflict was not rejected",
            )
        finally:
            _stop_agent(process)

        database_path = library_root / "library.db"
        with sqlite3.connect(database_path) as connection:
            export_count = connection.execute(
                "SELECT COUNT(*) FROM export_packages_v2"
            ).fetchone()[0]
            asset_count = connection.execute(
                "SELECT COUNT(*) FROM concept_assets WHERE role = 'export_package'"
            ).fetchone()[0]
            link_count = connection.execute(
                "SELECT COUNT(*) FROM artifact_links WHERE relation = 'concept_export_package'"
            ).fetchone()[0]
        _assert(
            (export_count, asset_count, link_count) == (1, 1, 1),
            "export persistence mismatch",
        )

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted_process = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted_process)
            restored = _json_request(
                restart_url,
                f"/api/v1/exports/{created['export_id']}",
                method="GET",
            )
            _assert(
                restored["package_sha256"] == created["package_sha256"],
                "restart export mismatch",
            )
            _assert(
                _download(restart_url, created["export_id"]) == package_payload,
                "restart download mismatch",
            )
            _assert(
                _download_combined(restart_url, created["export_id"])
                == combined_payload,
                "restart combined GLB mismatch",
            )
            _assert(
                _download_combined_obj(restart_url, created["export_id"])
                == combined_obj,
                "restart combined OBJ mismatch",
            )
            _assert(
                _download_combined_mtl(restart_url, created["export_id"])
                == combined_mtl,
                "restart combined MTL mismatch",
            )
            _assert(
                _download_png(restart_url, created["export_id"], "preview")
                == preview_png,
                "restart preview PNG mismatch",
            )
            _assert(
                _download_png(restart_url, created["export_id"], "exploded")
                == exploded_png,
                "restart exploded PNG mismatch",
            )
        finally:
            _stop_agent(restarted_process)

        print(
            json.dumps(
                {
                    "ok": True,
                    "export_id": created["export_id"],
                    "job_id": created["job_id"],
                    "package_sha256": created["package_sha256"],
                    "module_count": len(created["manifest"]["modules"]),
                    "quality_report_included": True,
                    "combined_glb_sha256": created["combined_glb_sha256"],
                    "combined_obj_sha256": created["combined_obj_sha256"],
                    "combined_obj_transform_mirror_verified": True,
                    "combined_obj_unsupported_mode_rejected": True,
                    "preview_png_sha256": created["preview_png_sha256"],
                    "exploded_png_sha256": created["exploded_png_sha256"],
                    "transparent_png_verified": True,
                    "unsupported_feature_rejected": True,
                    "restart_restored": True,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _quality_report(project_id: str, version_id: str) -> dict:
    return {
        "schema_version": "ModelQualityReport@1",
        "report_id": "quality_export_v2",
        "project_id": project_id,
        "version_id": version_id,
        "ruleset_version": "weapon-concept-quality/1.0",
        "status": "passed",
        "findings": [],
        "created_at": "2026-07-10T12:00:00+00:00",
    }


def _download(base_url: str, export_id: str) -> bytes:
    with urllib.request.urlopen(
        f"{base_url}/api/v1/exports/{export_id}/file", timeout=10
    ) as response:
        _assert(response.status == 200, "export download failed")
        _assert(
            response.headers.get_content_type() == "application/zip",
            "export MIME mismatch",
        )
        return response.read()


def _download_combined(base_url: str, export_id: str) -> bytes:
    with urllib.request.urlopen(
        f"{base_url}/api/v1/exports/{export_id}/combined.glb",
        timeout=10,
    ) as response:
        _assert(response.status == 200, "combined GLB download failed")
        _assert(
            response.headers.get_content_type() == "model/gltf-binary",
            "GLB MIME mismatch",
        )
        return response.read()


def _download_combined_obj(base_url: str, export_id: str) -> bytes:
    with urllib.request.urlopen(
        f"{base_url}/api/v1/exports/{export_id}/combined.obj",
        timeout=10,
    ) as response:
        _assert(response.status == 200, "combined OBJ download failed")
        _assert(response.headers.get_content_type() == "model/obj", "OBJ MIME mismatch")
        return response.read()


def _download_combined_mtl(base_url: str, export_id: str) -> bytes:
    with urllib.request.urlopen(
        f"{base_url}/api/v1/exports/{export_id}/combined.mtl",
        timeout=10,
    ) as response:
        _assert(response.status == 200, "combined MTL download failed")
        _assert(response.headers.get_content_type() == "model/mtl", "MTL MIME mismatch")
        return response.read()


def _download_png(base_url: str, export_id: str, kind: str) -> bytes:
    with urllib.request.urlopen(
        f"{base_url}/api/v1/exports/{export_id}/{kind}.png",
        timeout=10,
    ) as response:
        _assert(response.status == 200, f"{kind} PNG download failed")
        _assert(response.headers.get_content_type() == "image/png", "PNG MIME mismatch")
        return response.read()


def _png_stats(payload: bytes) -> tuple[int, int, int, int]:
    _assert(payload.startswith(b"\x89PNG\r\n\x1a\n"), "PNG signature mismatch")
    offset = 8
    width = height = 0
    compressed = bytearray()
    while offset < len(payload):
        length = struct.unpack_from(">I", payload, offset)[0]
        kind = payload[offset + 4 : offset + 8]
        data = payload[offset + 8 : offset + 8 + length]
        stored_crc = struct.unpack_from(">I", payload, offset + 8 + length)[0]
        _assert(
            zlib.crc32(kind + data) & 0xFFFFFFFF == stored_crc,
            f"PNG chunk CRC mismatch: {kind!r}",
        )
        offset += 12 + length
        if kind == b"IHDR":
            width, height, bit_depth, color_type, _, _, _ = struct.unpack(
                ">IIBBBBB", data
            )
            _assert((bit_depth, color_type) == (8, 6), "PNG must be RGBA8")
        elif kind == b"IDAT":
            compressed.extend(data)
        elif kind == b"IEND":
            break
    raw = zlib.decompress(bytes(compressed))
    stride = width * 4
    opaque = transparent = 0
    for row in range(height):
        start = row * (stride + 1)
        _assert(raw[start] == 0, "PNG smoke only supports deterministic filter 0")
        alpha = raw[start + 4 : start + stride + 1 : 4]
        opaque += sum(value > 0 for value in alpha)
        transparent += sum(value == 0 for value in alpha)
    return width, height, opaque, transparent


def _assert_obj_transform_and_mirror(graph: ModuleGraph, payload: bytes) -> None:
    transformed = graph.nodes[0].model_copy(
        update={
            "transform": Transform(
                position=[86, 33, 0],
                rotation=[0, 0, 0],
                scale=[1, 1, 1],
            ),
            "mirror_axis": "x",
        }
    )
    combined = build_combined_glb(
        [CombinedGlbSource(node=transformed, payload=payload)]
    )
    first_result = build_combined_obj(combined)
    second_result = build_combined_obj(combined)
    _assert(
        first_result == second_result, "combined OBJ conversion is not deterministic"
    )
    obj = first_result.obj.decode("utf-8")
    first_vertex = next(line for line in obj.splitlines() if line.startswith("v "))
    values = [float(value) for value in first_vertex.split()[1:]]
    _assert(
        all(
            abs(actual - expected) <= 1e-9
            for actual, expected in zip(values, [0.136, 0.008, 0])
        ),
        f"OBJ transform/mirror mismatch: {values}",
    )
    face = next(line for line in obj.splitlines() if line.startswith("f "))
    _assert(face.split()[1:] == ["1/1", "3/3", "2/2"], "mirrored OBJ winding mismatch")
    document, binary = read_glb(combined)
    document["meshes"][0]["primitives"][0]["mode"] = 1
    try:
        build_combined_obj(write_glb(document, binary))
    except CombinedObjError as exc:
        _assert("TRIANGLES" in str(exc), "OBJ unsupported-mode error was not specific")
    else:
        raise AssertionError("combined OBJ accepted a non-triangle primitive")


def _assert_unsupported_static_feature_rejected(graph: ModuleGraph) -> None:
    unsupported = write_glb(
        {
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"name": "animated", "mesh": 0}],
            "meshes": [{"primitives": []}],
            "animations": [{"channels": [], "samplers": []}],
            "buffers": [],
        },
        b"",
    )
    try:
        build_combined_glb(
            [CombinedGlbSource(node=graph.nodes[0], payload=unsupported)]
        )
    except CombinedGlbError as exc:
        _assert("animations" in str(exc), "unsupported feature error was not specific")
        return
    raise AssertionError("combined GLB accepted an unsupported animation")


if __name__ == "__main__":
    raise SystemExit(main())
