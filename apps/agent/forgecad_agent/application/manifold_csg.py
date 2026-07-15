from __future__ import annotations

import math
import multiprocessing
import time
from collections.abc import Callable, Mapping, Sequence
from typing import Any


MANIFOLD_KERNEL_ID = "manifold3d"
MANIFOLD_KERNEL_VERSION = "3.5.2"
DEFAULT_CSG_TIMEOUT_SECONDS = 5.0
MAX_CSG_INPUT_SOLIDS = 32
MAX_CSG_INPUT_TRIANGLES = 200_000
MIN_CSG_NORMAL_LENGTH_MM2 = 1e-6


class ManifoldCsgError(ValueError):
    """Stable, node-addressable production CSG failure."""

    def __init__(self, code: str, node_id: str, message: str) -> None:
        self.code = code
        self.node_id = node_id
        super().__init__(f"{code} at {node_id}: {message}")


def execute_manifold_boolean(
    *,
    node_id: str,
    operation: str,
    left_solids: Sequence[Mapping[str, Any]],
    right_solids: Sequence[Mapping[str, Any]],
    triangle_budget: int,
    cancel_check: Callable[[], bool] | None = None,
    timeout_seconds: float = DEFAULT_CSG_TIMEOUT_SECONDS,
) -> list[dict[str, Any]]:
    """Run one bounded Manifold operation outside the API process.

    Only canonical triangle/property payloads cross the process boundary.  The
    child never receives a database, object-store, Snapshot or file path.
    Killing the child therefore cannot leave a partial authoritative object.
    """

    if operation not in {"union", "subtract"}:
        raise ManifoldCsgError("CSG_OPERATION_UNSUPPORTED", node_id, operation)
    if not left_solids or not right_solids:
        raise ManifoldCsgError("CSG_INPUT_MISSING", node_id, "two non-empty operands are required")
    solid_count = len(left_solids) + len(right_solids)
    if solid_count > MAX_CSG_INPUT_SOLIDS:
        raise ManifoldCsgError(
            "CSG_INPUT_BUDGET_EXCEEDED",
            node_id,
            f"{solid_count} solids exceed the limit {MAX_CSG_INPUT_SOLIDS}",
        )
    input_triangles = sum(
        len(solid.get("triangles", []))
        for solid in [*left_solids, *right_solids]
        if isinstance(solid, Mapping)
    )
    if input_triangles > min(MAX_CSG_INPUT_TRIANGLES, triangle_budget * 2):
        raise ManifoldCsgError(
            "CSG_INPUT_BUDGET_EXCEEDED",
            node_id,
            f"{input_triangles} input triangles exceed the bounded compile budget",
        )
    if _has_near_coincident_planes(left_solids, right_solids):
        raise ManifoldCsgError(
            "CSG_DEGENERATE_OUTPUT",
            node_id,
            "near-coincident input planes were rejected before kernel/GLB",
        )
    if cancel_check is not None and cancel_check():
        raise ManifoldCsgError("CSG_CANCELLED", node_id, "cancelled before kernel start")
    if timeout_seconds <= 0:
        raise ManifoldCsgError("CSG_TIMEOUT", node_id, "timeout elapsed before kernel start")

    context = multiprocessing.get_context("spawn")
    parent, child = context.Pipe(duplex=False)
    process = context.Process(
        target=_kernel_child,
        args=(
            child,
            {
                "node_id": node_id,
                "operation": operation,
                "left_solids": list(left_solids),
                "right_solids": list(right_solids),
                "triangle_budget": triangle_budget,
            },
        ),
        name=f"forgecad-csg-{node_id}",
        daemon=True,
    )
    started = time.monotonic()
    process.start()
    child.close()
    try:
        while True:
            if cancel_check is not None and cancel_check():
                _terminate(process)
                raise ManifoldCsgError("CSG_CANCELLED", node_id, "kernel process was cancelled")
            if time.monotonic() - started > timeout_seconds:
                _terminate(process)
                raise ManifoldCsgError("CSG_TIMEOUT", node_id, "kernel process exceeded its time budget")
            if parent.poll(0.01):
                response = parent.recv()
                process.join(timeout=1)
                if process.is_alive():
                    _terminate(process)
                if not isinstance(response, dict):
                    raise ManifoldCsgError("CSG_KERNEL_FAILURE", node_id, "kernel returned an invalid response")
                if response.get("status") != "passed":
                    raise ManifoldCsgError(
                        str(response.get("error_code") or "CSG_KERNEL_FAILURE"),
                        node_id,
                        str(response.get("message") or "kernel rejected the operation"),
                    )
                triangles = response.get("triangles")
                if not isinstance(triangles, list) or not triangles:
                    raise ManifoldCsgError("CSG_EMPTY_RESULT", node_id, "kernel produced no closed result")
                return triangles
            if not process.is_alive():
                process.join(timeout=1)
                raise ManifoldCsgError(
                    "CSG_KERNEL_FAILURE",
                    node_id,
                    f"kernel process exited without a result (exit={process.exitcode})",
                )
    finally:
        parent.close()
        if process.is_alive():
            _terminate(process)


def _terminate(process: multiprocessing.Process) -> None:
    process.terminate()
    process.join(timeout=1)
    if process.is_alive():
        process.kill()
        process.join(timeout=1)


def _kernel_child(connection: Any, payload: Mapping[str, Any]) -> None:
    try:
        response = _execute_in_child(payload)
    except ManifoldCsgError as exc:
        response = {"status": "rejected", "error_code": exc.code, "message": str(exc)}
    except Exception:
        # No traceback or library internals cross the runtime boundary.
        response = {
            "status": "rejected",
            "error_code": "CSG_KERNEL_FAILURE",
            "message": "Manifold rejected the bounded geometry operation",
        }
    try:
        connection.send(response)
    finally:
        connection.close()


def _execute_in_child(payload: Mapping[str, Any]) -> dict[str, Any]:
    import manifold3d as manifold
    import numpy as np

    node_id = str(payload["node_id"])
    operation = str(payload["operation"])
    left, left_sources = _compose_operand(manifold, np, payload["left_solids"], node_id)
    right, right_sources = _compose_operand(manifold, np, payload["right_solids"], node_id)
    sources = {**left_sources, **right_sources}
    result = left + right if operation == "union" else left - right
    status = result.status()
    if status != manifold.Error.NoError:
        code = "CSG_NON_MANIFOLD_INPUT" if status == manifold.Error.NotManifold else "CSG_KERNEL_FAILURE"
        raise ManifoldCsgError(code, node_id, f"Manifold status {status}")
    result = result.simplify(1e-7)
    mesh = result.to_mesh()
    if len(mesh.tri_verts) == 0:
        raise ManifoldCsgError("CSG_EMPTY_RESULT", node_id, "boolean removed all geometry")
    if len(mesh.tri_verts) > int(payload["triangle_budget"]):
        raise ManifoldCsgError(
            "CSG_RESULT_BUDGET_EXCEEDED",
            node_id,
            f"{len(mesh.tri_verts)} triangles exceed the program budget",
        )
    if mesh.vert_properties.shape[1] < 6:
        raise ManifoldCsgError("CSG_PROVENANCE_LOST", node_id, "property channels are missing")
    if len(mesh.run_index) != len(mesh.run_original_id) + 1:
        raise ManifoldCsgError("CSG_PROVENANCE_LOST", node_id, "run metadata is incomplete")

    triangles: list[dict[str, Any]] = []
    for run_index, original_id_value in enumerate(mesh.run_original_id):
        original_id = int(original_id_value)
        source = sources.get(original_id)
        if source is None:
            raise ManifoldCsgError("CSG_PROVENANCE_LOST", node_id, "unknown source run")
        first = int(mesh.run_index[run_index]) // 3
        end = int(mesh.run_index[run_index + 1]) // 3
        backside = bool(mesh.backside(run_index))
        for triangle_index in range(first, end):
            face_id = int(mesh.face_id[triangle_index])
            face = source["faces"].get(face_id)
            if face is None:
                raise ManifoldCsgError("CSG_PROVENANCE_LOST", node_id, "source face mapping is missing")
            vertex_indices = [int(value) for value in mesh.tri_verts[triangle_index]]
            properties = mesh.vert_properties[vertex_indices, 3:6]
            expected = np.asarray(face["property_codes"], dtype=properties.dtype)
            if not np.allclose(properties, expected, rtol=0, atol=1e-4):
                raise ManifoldCsgError("CSG_PROVENANCE_LOST", node_id, "source/material/zone channels changed")
            vertices = [
                [round(float(value), 7) for value in mesh.vert_properties[index, :3]]
                for index in vertex_indices
            ]
            if _normal_length(vertices) <= MIN_CSG_NORMAL_LENGTH_MM2:
                raise ManifoldCsgError("CSG_DEGENERATE_OUTPUT", node_id, "near-degenerate triangle rejected before GLB")
            triangles.append(
                {
                    "vertices_mm": vertices,
                    "source_operation_id": face["source_operation_id"],
                    "source_part_role": face["source_part_role"],
                    "material_id": face["material_id"],
                    "material_zone_id": face["material_zone_id"],
                    "source_face_id": int(face["source_face_id"]),
                    "boolean_backside": backside,
                    "surface_role": "boolean_cut" if backside else face["surface_role"],
                }
            )
    triangles.sort(
        key=lambda item: (
            item["source_operation_id"],
            item["material_id"],
            item["material_zone_id"],
            item["surface_role"],
            item["boolean_backside"],
            item["source_face_id"],
            item["vertices_mm"],
        )
    )
    return {"status": "passed", "triangles": triangles}


def _compose_operand(manifold: Any, np: Any, solids: Sequence[Mapping[str, Any]], node_id: str) -> tuple[Any, dict[int, dict[str, Any]]]:
    values = []
    source_by_original: dict[int, dict[str, Any]] = {}
    property_catalog: dict[tuple[str, str, str], tuple[float, float, float]] = {}
    for solid_index, solid in enumerate(solids):
        triangles = solid.get("triangles") if isinstance(solid, Mapping) else None
        if not isinstance(triangles, list) or not triangles:
            raise ManifoldCsgError("CSG_INPUT_INVALID", node_id, "input solid has no triangles")
        vertices: list[list[float]] = []
        tri_verts: list[list[int]] = []
        face_ids: list[int] = []
        faces: dict[int, dict[str, Any]] = {}
        first_by_position: dict[tuple[float, float, float], int] = {}
        merge_from: list[int] = []
        merge_to: list[int] = []
        for face_id, triangle in enumerate(triangles):
            points = triangle.get("vertices_mm")
            if not isinstance(points, list) or len(points) != 3:
                raise ManifoldCsgError("CSG_INPUT_INVALID", node_id, "triangle vertices are invalid")
            source_id = str(triangle.get("source_operation_id") or f"source_{solid_index}")
            material_id = str(triangle.get("material_id") or "mat_primary")
            zone_id = str(triangle.get("material_zone_id") or "zone_primary")
            catalog_key = (source_id, material_id, zone_id)
            if catalog_key not in property_catalog:
                index = len(property_catalog) + 1
                property_catalog[catalog_key] = (float(index), float(index + 10_000), float(index + 20_000))
            codes = property_catalog[catalog_key]
            start = len(vertices)
            for point in points:
                if not isinstance(point, list) or len(point) != 3 or any(not math.isfinite(float(value)) for value in point):
                    raise ManifoldCsgError("CSG_INPUT_INVALID", node_id, "triangle contains a non-finite vertex")
                xyz = tuple(round(float(value), 7) for value in point)
                vertex_index = len(vertices)
                vertices.append([*xyz, *codes])
                existing = first_by_position.get(xyz)
                if existing is None:
                    first_by_position[xyz] = vertex_index
                else:
                    merge_from.append(vertex_index)
                    merge_to.append(existing)
            tri_verts.append([start, start + 1, start + 2])
            face_ids.append(face_id)
            faces[face_id] = {
                "source_operation_id": source_id,
                "source_part_role": str(triangle.get("source_part_role") or "part"),
                "material_id": material_id,
                "material_zone_id": zone_id,
                "source_face_id": int(triangle.get("source_face_id", face_id)),
                "surface_role": str(triangle.get("surface_role") or "surface"),
                "property_codes": codes,
            }
        mesh = manifold.Mesh64(
            np.asarray(vertices, dtype=np.float64),
            np.asarray(tri_verts, dtype=np.uint64),
            merge_from_vert=np.asarray(merge_from, dtype=np.uint64),
            merge_to_vert=np.asarray(merge_to, dtype=np.uint64),
            face_id=np.asarray(face_ids, dtype=np.uint64),
        )
        value = manifold.Manifold(mesh)
        if value.status() != manifold.Error.NoError:
            raise ManifoldCsgError("CSG_NON_MANIFOLD_INPUT", node_id, f"input solid {solid_index} is not closed manifold geometry")
        source_mesh = value.to_mesh()
        if len(source_mesh.run_original_id) != 1:
            raise ManifoldCsgError("CSG_PROVENANCE_LOST", node_id, "input source run is ambiguous")
        source_by_original[int(source_mesh.run_original_id[0])] = {"faces": faces}
        values.append(value)
    operand = values[0] if len(values) == 1 else manifold.Manifold.batch_boolean(values, manifold.OpType.Add)
    if operand.status() != manifold.Error.NoError:
        raise ManifoldCsgError("CSG_NON_MANIFOLD_INPUT", node_id, "operand composition failed")
    return operand, source_by_original


def _normal_length(vertices: Sequence[Sequence[float]]) -> float:
    a, b, c = vertices
    ab = [float(b[index]) - float(a[index]) for index in range(3)]
    ac = [float(c[index]) - float(a[index]) for index in range(3)]
    cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ]
    return math.sqrt(sum(value * value for value in cross))


def _has_near_coincident_planes(
    left_solids: Sequence[Mapping[str, Any]],
    right_solids: Sequence[Mapping[str, Any]],
) -> bool:
    """Reject epsilon slivers while allowing exactly coplanar Manifold inputs."""

    def coordinates(solids: Sequence[Mapping[str, Any]], axis: int) -> set[float]:
        return {
            round(float(point[axis]), 7)
            for solid in solids
            for triangle in solid.get("triangles", [])
            for point in triangle.get("vertices_mm", [])
        }

    for axis in range(3):
        left = coordinates(left_solids, axis)
        right = coordinates(right_solids, axis)
        if any(0 < abs(a - b) < 1e-5 for a in left for b in right):
            return True
    return False
