from __future__ import annotations

import math
import re
import struct
from dataclasses import dataclass
from statistics import median
from typing import Any, Iterable, Sequence

from forgecad_agent.application.combined_glb import CombinedGlbError, read_glb
from forgecad_agent.application.triangle_intersections import (
    Triangle3,
    inspect_triangle_mesh_intersection,
    mesh_bounds,
)
from forgecad_agent.domain.concepts.connector_snapping import connector_alignment_error
from forgecad_agent.domain.concepts.models import (
    ModuleAssetManifest,
    ModuleGraph,
    QualityFinding,
    QualityGeometryReference,
    Transform,
    WeaponConceptSpec,
)


POSITION_WELD_TOLERANCE_MM = 0.001
DEGENERATE_AREA_EPSILON_MM2 = 1e-8
NORMAL_LENGTH_TOLERANCE = 0.05
CONNECTOR_DISTANCE_TOLERANCE_MM = 0.1
CONNECTOR_ROTATION_TOLERANCE_DEG = 0.1
CONNECTED_SURFACE_GAP_TOLERANCE_MM = 2.0
MAX_HIGHLIGHT_TRIANGLES_PER_NODE = 16
MESH_DENSITY_OUTLIER_FACTOR = 8.0
SYMMETRY_THRESHOLDS_PERCENT = {"symmetric": 5.0, "mostly_symmetric": 35.0}

_COMPONENT_FORMATS = {
    5120: ("b", 1),
    5121: ("B", 1),
    5122: ("h", 2),
    5123: ("H", 2),
    5125: ("I", 4),
    5126: ("f", 4),
}
_TYPE_WIDTHS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}


@dataclass(frozen=True)
class ModuleInspectionSource:
    node_id: str
    manifest: ModuleAssetManifest
    payload: bytes


@dataclass(frozen=True)
class MeshInspection:
    findings: list[QualityFinding]
    minimum_mm: tuple[float, float, float]
    maximum_mm: tuple[float, float, float]
    triangle_count: int
    triangles_mm: tuple[Triangle3, ...]
    surface_area_mm2: float
    boundary_edge_count: int
    non_manifold_edge_count: int


def inspect_concept_geometry(
    *,
    graph: ModuleGraph,
    sources: Sequence[ModuleInspectionSource],
    spec: WeaponConceptSpec | None = None,
) -> list[QualityFinding]:
    findings: list[QualityFinding] = []
    inspections: dict[str, MeshInspection] = {}
    for source in sources:
        inspection = inspect_module_mesh(source)
        inspections[source.node_id] = inspection
        findings.extend(inspection.findings)
    findings.extend(_inspect_mesh_density(graph, inspections, spec))
    findings.extend(_inspect_assembly_symmetry(graph, sources, inspections, spec))
    findings.extend(_inspect_connector_alignment(graph, sources))
    findings.extend(_inspect_connected_surface_gaps(graph, inspections))
    findings.extend(_inspect_unconnected_intersections(graph, inspections))
    if not findings:
        findings.append(
            _finding(
                1,
                "geometry.ruleset",
                "mesh",
                "info",
                "passed",
                [node.node_id for node in graph.nodes],
                "服务端几何与装配规则集未发现问题。",
                measured_value=len(graph.nodes),
                threshold="all checks passed",
            )
        )
    return [
        item.model_copy(update={"finding_id": f"finding_{index:04d}_{item.finding_id[8:]}"})
        for index, item in enumerate(findings, start=1)
    ]


def inspect_module_mesh(source: ModuleInspectionSource) -> MeshInspection:
    node_ids = [source.node_id]
    findings: list[QualityFinding] = []
    try:
        document, binary = read_glb(source.payload)
        if document.get("extensionsRequired"):
            raise ValueError("不支持需要扩展解码的 GLB")
        _assert_baked_node_transforms(document)
        meshes = document.get("meshes", [])
        if not meshes:
            raise ValueError("GLB 不包含 mesh")
        lod_contract_issue_count = _lod0_contract_issue_count(
            document, source.manifest.module_id
        )
        all_positions: list[tuple[float, float, float]] = []
        topology_triangles: list[tuple[int, int, int]] = []
        triangle_count = 0
        degenerate_count = 0
        invalid_index_count = 0
        boundary_count = 0
        non_manifold_count = 0
        missing_normal_primitives = 0
        invalid_normal_count = 0
        missing_uv_primitives = 0
        invalid_uv_count = 0
        for mesh in meshes:
            for primitive in mesh.get("primitives", []):
                if int(primitive.get("mode", 4)) != 4:
                    raise ValueError("首版检查器仅支持 TRIANGLES primitive")
                attributes = primitive.get("attributes", {})
                if "POSITION" not in attributes:
                    raise ValueError("primitive 缺少 POSITION")
                positions = _vectors(document, binary, int(attributes["POSITION"]), 3)
                if any(not all(math.isfinite(value) for value in point) for point in positions):
                    raise ValueError("POSITION 包含非法数值")
                vertex_offset = len(all_positions)
                all_positions.extend(positions)
                if "indices" in primitive:
                    index_accessor = document["accessors"][int(primitive["indices"])]
                    if int(index_accessor["componentType"]) not in {5121, 5123, 5125}:
                        raise ValueError("indices 必须使用无符号整数 componentType")
                    indices = [
                        int(value[0])
                        for value in _accessor(document, binary, int(primitive["indices"]))
                    ]
                else:
                    indices = list(range(len(positions)))
                if len(indices) % 3:
                    invalid_index_count += len(indices) % 3
                triangle_count += len(indices) // 3
                for offset in range(0, len(indices) - 2, 3):
                    triangle = (indices[offset], indices[offset + 1], indices[offset + 2])
                    if any(index < 0 or index >= len(positions) for index in triangle):
                        invalid_index_count += 1
                        continue
                    topology_triangles.append(tuple(vertex_offset + index for index in triangle))
                    if (
                        _triangle_area_squared(*(positions[index] for index in triangle))
                        <= DEGENERATE_AREA_EPSILON_MM2**2
                    ):
                        degenerate_count += 1
                if "NORMAL" not in attributes:
                    missing_normal_primitives += 1
                else:
                    normals = _vectors(document, binary, int(attributes["NORMAL"]), 3)
                    invalid_normal_count += sum(
                        1
                        for normal in normals
                        if not all(math.isfinite(value) for value in normal)
                        or abs(math.sqrt(sum(value * value for value in normal)) - 1.0)
                        > NORMAL_LENGTH_TOLERANCE
                    )
                if "TEXCOORD_0" not in attributes:
                    missing_uv_primitives += 1
                else:
                    uvs = _vectors(document, binary, int(attributes["TEXCOORD_0"]), 2)
                    invalid_uv_count += sum(
                        1 for uv in uvs if not all(math.isfinite(value) for value in uv)
                    )
        if not all_positions:
            raise ValueError("GLB 不包含顶点")
        boundary_count, non_manifold_count = _edge_counts(
            all_positions,
            topology_triangles,
        )
        minimum = tuple(min(point[axis] for point in all_positions) * 1000 for axis in range(3))
        maximum = tuple(max(point[axis] for point in all_positions) * 1000 for axis in range(3))
        triangles_mm = tuple(
            tuple(
                tuple(all_positions[index][axis] * 1000 for axis in range(3)) for index in triangle
            )
            for triangle in topology_triangles
        )
        surface_area_mm2 = sum(_triangle_area_mm2(triangle) for triangle in triangles_mm)
        duplicate_triangle_indices = _duplicate_triangle_indices(
            all_positions, topology_triangles
        )
        hidden_component_indices = _hidden_closed_component_indices(triangles_mm)
        expected_triangles = source.manifest.triangle_count
        if triangle_count != expected_triangles:
            findings.append(
                _finding(
                    1,
                    "mesh.triangle_manifest",
                    "mesh",
                    "error",
                    "failed",
                    node_ids,
                    "GLB 三角形数量与模块清单不一致。",
                    triangle_count,
                    expected_triangles,
                    "重新生成清单并以内容哈希固定资产。",
                )
            )
        if invalid_index_count:
            findings.append(
                _finding(
                    2,
                    "mesh.invalid_indices",
                    "mesh",
                    "error",
                    "failed",
                    node_ids,
                    "检测到越界或不完整的三角形索引。",
                    invalid_index_count,
                    0,
                    "修复索引缓冲后重新导入模块。",
                )
            )
        if degenerate_count:
            findings.append(
                _finding(
                    3,
                    "mesh.degenerate_triangles",
                    "mesh",
                    "error",
                    "failed",
                    node_ids,
                    "检测到退化三角形。",
                    degenerate_count,
                    0,
                    "删除零面积面并重新三角化。",
                )
            )
        if non_manifold_count:
            findings.append(
                _finding(
                    4,
                    "mesh.non_manifold_edges",
                    "mesh",
                    "error",
                    "failed",
                    node_ids,
                    "检测到被三个以上三角形共享的非流形边。",
                    non_manifold_count,
                    0,
                    "修复重叠面或内部面。",
                )
            )
        if boundary_count:
            findings.append(
                _finding(
                    5,
                    "mesh.boundary_edges",
                    "mesh",
                    "warning",
                    "warning",
                    node_ids,
                    "网格包含开放边界；展示资产可以继续，但不是封闭实体。",
                    boundary_count,
                    0,
                    "如需封闭模型，请补面并焊接边界顶点。",
                )
            )
        if missing_normal_primitives or invalid_normal_count:
            findings.append(
                _finding(
                    6,
                    "mesh.normals",
                    "mesh",
                    "warning",
                    "warning",
                    node_ids,
                    "法线缺失或未归一化，可能导致查看器明暗异常。",
                    missing_normal_primitives + invalid_normal_count,
                    0,
                    "重新计算并导出顶点法线。",
                )
            )
        if missing_uv_primitives or invalid_uv_count:
            findings.append(
                _finding(
                    7,
                    "mesh.uv0",
                    "mesh",
                    "warning",
                    "warning",
                    node_ids,
                    "UV0 缺失或包含非法数值。",
                    missing_uv_primitives + invalid_uv_count,
                    0,
                    "为所有可见 primitive 提供有效 UV0。",
                )
            )
        declared_bounds = source.manifest.bounds_mm
        actual_bounds = [maximum[index] - minimum[index] for index in range(3)]
        max_bound_error = max(
            abs(actual_bounds[index] - declared_bounds[index]) for index in range(3)
        )
        if max_bound_error > 0.1:
            findings.append(
                _finding(
                    8,
                    "mesh.bounds_manifest",
                    "mesh",
                    "warning",
                    "warning",
                    node_ids,
                    "GLB 包围盒与模块清单不一致。",
                    round(max_bound_error, 6),
                    0.1,
                    "重新生成模块 bounds_mm。",
                )
            )
        if duplicate_triangle_indices:
            findings.append(
                _finding(
                    14,
                    "mesh.duplicate_triangles",
                    "mesh",
                    "warning",
                    "warning",
                    node_ids,
                    "检测到完全重合的重复三角形，可能形成隐藏面或闪烁。",
                    len(duplicate_triangle_indices),
                    0,
                    "删除重复面并重新计算拓扑。",
                )
            )
        if hidden_component_indices:
            findings.append(
                _finding(
                    15,
                    "mesh.enclosed_components",
                    "mesh",
                    "warning",
                    "warning",
                    node_ids,
                    "检测到被另一封闭子组件完全包裹的隐藏几何。",
                    len(hidden_component_indices),
                    0,
                    "删除不可见的内部封闭组件，或将其明确拆分为需要保留的资产。",
                )
            )
        if lod_contract_issue_count:
            findings.append(
                _finding(
                    16,
                    "mesh.lod0_contract",
                    "mesh",
                    "error",
                    "failed",
                    node_ids,
                    "GLB Mesh/Node 名称不符合当前只支持 LOD0 的发布合同。",
                    lod_contract_issue_count,
                    0,
                    "按 MESH_/GEO_<module_id>_LOD0[_NN] 重命名；不要发布尚未支持的 LOD1/LOD2。",
                )
            )
        return MeshInspection(
            findings,
            minimum,
            maximum,
            triangle_count,
            triangles_mm,
            surface_area_mm2,
            boundary_count,
            non_manifold_count,
        )
    except (CombinedGlbError, KeyError, IndexError, TypeError, ValueError, struct.error) as exc:
        findings.append(
            _finding(
                9,
                "mesh.asset_readable",
                "mesh",
                "error",
                "failed",
                node_ids,
                f"无法检查模块 GLB：{exc}",
                suggestion="重新导出静态、内嵌 Buffer 的 glTF 2.0 GLB。",
            )
        )
        return MeshInspection(
            findings,
            (0.0, 0.0, 0.0),
            (0.0, 0.0, 0.0),
            0,
            (),
            0.0,
            0,
            0,
        )


def _inspect_mesh_density(
    graph: ModuleGraph,
    inspections: dict[str, MeshInspection],
    spec: WeaponConceptSpec | None,
) -> list[QualityFinding]:
    if spec is None:
        return []
    findings: list[QualityFinding] = []
    node_ids = [node.node_id for node in graph.nodes if node.node_id in inspections]
    total_triangles = sum(inspections[node_id].triangle_count for node_id in node_ids)
    if total_triangles > spec.constraints.max_triangle_count:
        findings.append(
            _finding(
                17,
                "mesh.triangle_budget",
                "mesh",
                "error",
                "failed",
                node_ids,
                "装配实际三角形总数超过 WeaponConceptSpec 预算。",
                total_triangles,
                spec.constraints.max_triangle_count,
                "降低模块面数、替换高密度模块，或显式调整概念资产预算。",
            )
        )
    densities = {
        node_id: inspections[node_id].triangle_count
        / (inspections[node_id].surface_area_mm2 / 1000.0)
        for node_id in node_ids
        if inspections[node_id].surface_area_mm2 > 1e-9
    }
    if len(densities) < 3:
        return findings
    baseline = float(median(densities.values()))
    if baseline <= 0:
        return findings
    threshold = baseline * MESH_DENSITY_OUTLIER_FACTOR
    for node_id, density in sorted(densities.items()):
        if density <= threshold:
            continue
        findings.append(
            _finding(
                18,
                "mesh.density_outlier",
                "mesh",
                "warning",
                "warning",
                [node_id],
                "模块单位表面积三角形密度明显高于当前装配中位数。",
                f"{density:.6f} triangles/1000mm2; median={baseline:.6f}",
                f"<= {threshold:.6f} triangles/1000mm2",
                "检查不可见细分、重复面或与其他模块不一致的细节级别。",
            )
        )
    return findings


def _inspect_assembly_symmetry(
    graph: ModuleGraph,
    sources: Sequence[ModuleInspectionSource],
    inspections: dict[str, MeshInspection],
    spec: WeaponConceptSpec | None,
) -> list[QualityFinding]:
    if spec is None or spec.constraints.symmetry == "asymmetric":
        return []
    nodes = {node.node_id: node for node in graph.nodes}
    root = nodes.get(graph.root_node_id)
    if root is None:
        return []
    world_triangles = _world_triangles(nodes, inspections)
    if not world_triangles:
        return []
    origin = _world_point(root.transform, root.mirror_axis, (0.0, 0.0, 0.0))
    normal_point = _world_point(root.transform, root.mirror_axis, (0.0, 0.0, 1.0))
    normal_vector = tuple(normal_point[index] - origin[index] for index in range(3))
    normal_length = math.sqrt(sum(value * value for value in normal_vector))
    if normal_length <= 1e-9:
        return []
    normal = tuple(value / normal_length for value in normal_vector)
    bounds_by_node = {
        node_id: mesh_bounds(triangles) for node_id, triangles in world_triangles.items()
    }
    assembly_minimum = tuple(
        min(bounds[0][axis] for bounds in bounds_by_node.values()) for axis in range(3)
    )
    assembly_maximum = tuple(
        max(bounds[1][axis] for bounds in bounds_by_node.values()) for axis in range(3)
    )
    assembly_diagonal = math.dist(assembly_minimum, assembly_maximum)
    center_tolerance = max(2.0, assembly_diagonal * 0.02)
    categories = {source.node_id: source.manifest.category for source in sources}
    records: dict[str, tuple[tuple[float, float, float], tuple[float, float, float], float]] = {}
    for node_id, (minimum, maximum) in bounds_by_node.items():
        center = tuple((minimum[axis] + maximum[axis]) * 0.5 for axis in range(3))
        size = tuple(maximum[axis] - minimum[axis] for axis in range(3))
        distance = sum((center[axis] - origin[axis]) * normal[axis] for axis in range(3))
        records[node_id] = (center, size, distance)

    unmatched = set(records)
    for node_id, (_center, size, distance) in records.items():
        projected_radius = 0.5 * sum(abs(normal[axis]) * size[axis] for axis in range(3))
        if abs(distance) <= center_tolerance and projected_radius + center_tolerance >= abs(
            distance
        ):
            unmatched.discard(node_id)

    for node_id in sorted(tuple(unmatched)):
        if node_id not in unmatched:
            continue
        center, size, distance = records[node_id]
        mirrored_center = tuple(
            center[axis] - 2.0 * distance * normal[axis] for axis in range(3)
        )
        best: tuple[float, str] | None = None
        for candidate_id in sorted(unmatched - {node_id}):
            if categories.get(candidate_id) != categories.get(node_id):
                continue
            candidate_center, candidate_size, _candidate_distance = records[candidate_id]
            size_tolerance = max(2.0, max(size + candidate_size) * 0.08)
            if max(
                abs(size[axis] - candidate_size[axis]) for axis in range(3)
            ) > size_tolerance:
                continue
            center_error = math.dist(mirrored_center, candidate_center)
            if center_error > center_tolerance:
                continue
            if best is None or center_error < best[0]:
                best = (center_error, candidate_id)
        if best is not None:
            unmatched.discard(node_id)
            unmatched.discard(best[1])

    deviation = len(unmatched) * 100.0 / len(records)
    threshold = SYMMETRY_THRESHOLDS_PERCENT[spec.constraints.symmetry]
    if deviation <= threshold:
        return []
    return [
        _finding(
            19,
            "assembly.symmetry_deviation",
            "assembly",
            "warning",
            "warning",
            sorted(unmatched),
            "模块占位相对根节点局部中面超出目标对称偏差。",
            f"{deviation:.6f}% unmatched_modules={len(unmatched)}",
            f"<= {threshold:.6f}%",
            "检查未配对的侧面模块，或把设计约束显式改为 mostly_symmetric/asymmetric。",
        )
    ]


def _inspect_connector_alignment(
    graph: ModuleGraph,
    sources: Sequence[ModuleInspectionSource],
) -> list[QualityFinding]:
    manifests = {source.manifest.module_id: source.manifest for source in sources}
    nodes = {node.node_id: node for node in graph.nodes}
    findings: list[QualityFinding] = []
    for edge in graph.edges:
        first_node, second_node = nodes[edge.from_node_id], nodes[edge.to_node_id]
        first = next(
            (
                item
                for item in manifests[first_node.module_id].connectors
                if item.connector_id == edge.from_connector_id
            ),
            None,
        )
        second = next(
            (
                item
                for item in manifests[second_node.module_id].connectors
                if item.connector_id == edge.to_connector_id
            ),
            None,
        )
        if first is None or second is None:
            findings.append(
                _finding(
                    10,
                    "assembly.connector_resolved",
                    "assembly",
                    "error",
                    "failed",
                    [first_node.node_id, second_node.node_id],
                    "装配边引用了不存在的 Connector。",
                    suggestion="重新验证 ModuleGraph。",
                )
            )
            continue
        distance, rotation = connector_alignment_error(
            first_transform=first_node.transform,
            first_connector=first.transform,
            second_transform=second_node.transform,
            second_connector=second.transform,
            first_mirror_axis=first_node.mirror_axis,
            second_mirror_axis=second_node.mirror_axis,
        )
        if (
            distance > CONNECTOR_DISTANCE_TOLERANCE_MM
            or rotation > CONNECTOR_ROTATION_TOLERANCE_DEG
        ):
            findings.append(
                _finding(
                    11,
                    "assembly.connector_alignment",
                    "assembly",
                    "error",
                    "failed",
                    [first_node.node_id, second_node.node_id],
                    "Connector 世界坐标未对齐，组件处于错位或悬空状态。",
                    f"{distance:.6f} mm / {rotation:.6f} deg",
                    f"<= {CONNECTOR_DISTANCE_TOLERANCE_MM} mm / {CONNECTOR_ROTATION_TOLERANCE_DEG} deg",
                    "重新执行 Connector snap。",
                )
            )
    return findings


def _inspect_unconnected_intersections(
    graph: ModuleGraph,
    inspections: dict[str, MeshInspection],
) -> list[QualityFinding]:
    nodes = {node.node_id: node for node in graph.nodes}
    connected = {frozenset((edge.from_node_id, edge.to_node_id)) for edge in graph.edges}
    world_triangles = _world_triangles(nodes, inspections)
    findings: list[QualityFinding] = []
    node_ids = sorted(world_triangles)
    for first_index, first_id in enumerate(node_ids):
        for second_id in node_ids[first_index + 1 :]:
            if frozenset((first_id, second_id)) in connected:
                continue
            first_inspection = inspections[first_id]
            second_inspection = inspections[second_id]
            result = inspect_triangle_mesh_intersection(
                world_triangles[first_id],
                world_triangles[second_id],
                first_is_closed=(
                    first_inspection.boundary_edge_count == 0
                    and first_inspection.non_manifold_edge_count == 0
                ),
                second_is_closed=(
                    second_inspection.boundary_edge_count == 0
                    and second_inspection.non_manifold_edge_count == 0
                ),
            )
            if result.intersection_count or result.containment:
                measured = (
                    f"surface_pairs={result.intersection_count}"
                    f"; containment={str(result.containment).lower()}"
                    f"; tested_pairs={result.tested_triangle_pairs}"
                    f"; capped={str(result.capped).lower()}"
                )
                findings.append(
                    _finding(
                        12,
                        "assembly.unconnected_triangle_intersection",
                        "assembly",
                        "warning",
                        "warning",
                        [first_id, second_id],
                        "未直接连接的组件发生三角形表面相交或封闭网格包含。",
                        measured,
                        "surface_pairs=0; containment=false",
                        "点击本条结果聚焦组件，并调整位置或建立正确的 Connector 连接。",
                        geometry_refs=_intersection_geometry_refs(
                            first_id,
                            second_id,
                            world_triangles[first_id],
                            world_triangles[second_id],
                            result.hit_pairs,
                        ),
                    )
                )
    return findings


def _inspect_connected_surface_gaps(
    graph: ModuleGraph,
    inspections: dict[str, MeshInspection],
) -> list[QualityFinding]:
    nodes = {node.node_id: node for node in graph.nodes}
    world_triangles = _world_triangles(nodes, inspections)
    findings: list[QualityFinding] = []
    for edge in graph.edges:
        if edge.from_node_id not in world_triangles or edge.to_node_id not in world_triangles:
            continue
        distance = _bounds_distance(
            mesh_bounds(world_triangles[edge.from_node_id]),
            mesh_bounds(world_triangles[edge.to_node_id]),
        )
        if distance <= CONNECTED_SURFACE_GAP_TOLERANCE_MM:
            continue
        findings.append(
            _finding(
                13,
                "assembly.connected_surface_gap",
                "assembly",
                "warning",
                "warning",
                [edge.from_node_id, edge.to_node_id],
                "Connector 已建立，但两个组件的世界包围盒仍存在明显间隙。",
                round(distance, 6),
                CONNECTED_SURFACE_GAP_TOLERANCE_MM,
                "检查 Connector 是否位于可见表面，或调整模块几何和连接点。",
            )
        )
    return findings


def _world_triangles(
    nodes: dict[str, Any],
    inspections: dict[str, MeshInspection],
) -> dict[str, tuple[Triangle3, ...]]:
    return {
        node_id: tuple(
            tuple(
                _world_point(nodes[node_id].transform, nodes[node_id].mirror_axis, point)
                for point in triangle
            )
            for triangle in inspection.triangles_mm
        )
        for node_id, inspection in inspections.items()
        if inspection.triangles_mm
    }


def _bounds_distance(
    first: tuple[tuple[float, float, float], tuple[float, float, float]],
    second: tuple[tuple[float, float, float], tuple[float, float, float]],
) -> float:
    separations = [
        max(0.0, first[0][axis] - second[1][axis], second[0][axis] - first[1][axis])
        for axis in range(3)
    ]
    return math.sqrt(sum(value * value for value in separations))


def _intersection_geometry_refs(
    first_id: str,
    second_id: str,
    first_triangles: Sequence[Triangle3],
    second_triangles: Sequence[Triangle3],
    hit_pairs: Sequence[tuple[int, int]],
) -> list[QualityGeometryReference]:
    first_indices = list(dict.fromkeys(pair[0] for pair in hit_pairs))[
        :MAX_HIGHLIGHT_TRIANGLES_PER_NODE
    ]
    second_indices = list(dict.fromkeys(pair[1] for pair in hit_pairs))[
        :MAX_HIGHLIGHT_TRIANGLES_PER_NODE
    ]
    if not first_indices and not second_indices:
        return []
    return [
        QualityGeometryReference(
            node_id=first_id,
            triangle_indices=first_indices,
            world_triangles_mm=[first_triangles[index] for index in first_indices],
        ),
        QualityGeometryReference(
            node_id=second_id,
            triangle_indices=second_indices,
            world_triangles_mm=[second_triangles[index] for index in second_indices],
        ),
    ]


def _accessor(
    document: dict[str, Any], binary: bytes, accessor_index: int
) -> list[tuple[float | int, ...]]:
    accessor = document["accessors"][accessor_index]
    if accessor.get("sparse"):
        raise ValueError("首版检查器不支持 sparse accessor")
    view = document["bufferViews"][int(accessor["bufferView"])]
    if int(view.get("buffer", 0)) != 0:
        raise ValueError("仅支持 GLB 内嵌 Buffer")
    component_type = int(accessor["componentType"])
    component_format, component_size = _COMPONENT_FORMATS[component_type]
    width = _TYPE_WIDTHS[str(accessor["type"])]
    element_size = component_size * width
    stride = int(view.get("byteStride", element_size))
    if stride < element_size:
        raise ValueError("accessor byteStride 小于元素宽度")
    offset = int(view.get("byteOffset", 0)) + int(accessor.get("byteOffset", 0))
    count = int(accessor["count"])
    result: list[tuple[float | int, ...]] = []
    for index in range(count):
        start = offset + index * stride
        if start + element_size > len(binary):
            raise ValueError("accessor 超出 GLB Buffer")
        result.append(struct.unpack_from("<" + component_format * width, binary, start))
    return result


def _assert_baked_node_transforms(document: dict[str, Any]) -> None:
    for node in document.get("nodes", []):
        if "matrix" in node:
            raise ValueError("模块 GLB 必须烘焙 node matrix")
        translation = [float(value) for value in node.get("translation", [0, 0, 0])]
        rotation = [float(value) for value in node.get("rotation", [0, 0, 0, 1])]
        scale = [float(value) for value in node.get("scale", [1, 1, 1])]
        if any(abs(value) > 1e-9 for value in translation):
            raise ValueError("模块 GLB 必须烘焙 node translation")
        if any(abs(value) > 1e-9 for value in rotation[:3]) or abs(rotation[3] - 1) > 1e-9:
            raise ValueError("模块 GLB 必须烘焙 node rotation")
        if any(abs(value - 1) > 1e-9 for value in scale):
            raise ValueError("模块 GLB 必须烘焙 node scale")


def _vectors(
    document: dict[str, Any], binary: bytes, accessor_index: int, width: int
) -> list[tuple[Any, ...]]:
    values = _accessor(document, binary, accessor_index)
    if any(len(value) != width for value in values):
        raise ValueError(f"accessor 必须是 VEC{width}")
    return values


def _triangle_area_squared(
    first: Sequence[float], second: Sequence[float], third: Sequence[float]
) -> float:
    ab = [second[index] - first[index] for index in range(3)]
    ac = [third[index] - first[index] for index in range(3)]
    cross = (
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    )
    return sum(value * value for value in cross) * 0.25 * 1_000_000_000_000


def _triangle_area_mm2(triangle: Triangle3) -> float:
    first, second, third = triangle
    ab = tuple(second[index] - first[index] for index in range(3))
    ac = tuple(third[index] - first[index] for index in range(3))
    cross = (
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    )
    return 0.5 * math.sqrt(sum(value * value for value in cross))


def _lod0_contract_issue_count(document: dict[str, Any], module_id: str) -> int:
    mesh_pattern = re.compile(rf"^MESH_{re.escape(module_id)}_LOD0(?:_[0-9]{{2}})?$")
    node_pattern = re.compile(rf"^GEO_{re.escape(module_id)}_LOD0(?:_[0-9]{{2}})?$")
    mesh_names = [str(mesh.get("name", "")) for mesh in document.get("meshes", [])]
    mesh_node_names = [
        str(node.get("name", ""))
        for node in document.get("nodes", [])
        if "mesh" in node
    ]
    return sum(not mesh_pattern.fullmatch(name) for name in mesh_names) + sum(
        not node_pattern.fullmatch(name) for name in mesh_node_names
    )


def _welded_vertex_ids(positions: Sequence[Sequence[float]]) -> list[int]:
    welded: dict[tuple[int, int, int], int] = {}
    vertex_ids: list[int] = []
    for point in positions:
        key = tuple(round(value * 1000 / POSITION_WELD_TOLERANCE_MM) for value in point)
        if key not in welded:
            welded[key] = len(welded)
        vertex_ids.append(welded[key])
    return vertex_ids


def _duplicate_triangle_indices(
    positions: Sequence[Sequence[float]], triangles: Sequence[tuple[int, int, int]]
) -> list[int]:
    vertex_ids = _welded_vertex_ids(positions)
    seen: set[tuple[int, int, int]] = set()
    duplicates: list[int] = []
    for triangle_index, triangle in enumerate(triangles):
        rendered = tuple(sorted(vertex_ids[index] for index in triangle))
        if rendered in seen:
            duplicates.append(triangle_index)
        else:
            seen.add(rendered)
    return duplicates


def _hidden_closed_component_indices(triangles: Sequence[Triangle3]) -> list[int]:
    components = _triangle_components(triangles)
    closed: list[
        tuple[
            list[int],
            tuple[Triangle3, ...],
            tuple[tuple[float, float, float], tuple[float, float, float]],
        ]
    ] = []
    for component in components:
        component_triangles = tuple(triangles[index] for index in component)
        if len(component_triangles) < 4 or not _triangles_form_closed_component(
            component_triangles
        ):
            continue
        closed.append((component, component_triangles, mesh_bounds(component_triangles)))
    hidden: set[int] = set()
    for first_index, first in enumerate(closed):
        for second in closed[first_index + 1 :]:
            if _bounds_strictly_contains(first[2], second[2]):
                outer, inner = first, second
            elif _bounds_strictly_contains(second[2], first[2]):
                outer, inner = second, first
            else:
                continue
            result = inspect_triangle_mesh_intersection(
                outer[1], inner[1], first_is_closed=True, second_is_closed=True
            )
            if result.intersection_count == 0 and result.containment:
                hidden.update(inner[0])
    return sorted(hidden)


def _triangle_components(triangles: Sequence[Triangle3]) -> list[list[int]]:
    point_to_triangles: dict[tuple[int, int, int], list[int]] = {}
    for triangle_index, triangle in enumerate(triangles):
        for point in triangle:
            point_to_triangles.setdefault(_point_key_mm(point), []).append(triangle_index)
    adjacency = [set() for _ in triangles]
    for related in point_to_triangles.values():
        for triangle_index in related:
            adjacency[triangle_index].update(related)
    components: list[list[int]] = []
    remaining = set(range(len(triangles)))
    while remaining:
        seed = min(remaining)
        stack = [seed]
        component: list[int] = []
        remaining.remove(seed)
        while stack:
            current = stack.pop()
            component.append(current)
            for neighbor in sorted(adjacency[current] & remaining, reverse=True):
                remaining.remove(neighbor)
                stack.append(neighbor)
        components.append(sorted(component))
    return components


def _triangles_form_closed_component(triangles: Sequence[Triangle3]) -> bool:
    counts: dict[tuple[tuple[int, int, int], tuple[int, int, int]], int] = {}
    for triangle in triangles:
        points = [_point_key_mm(point) for point in triangle]
        for first, second in (
            (points[0], points[1]),
            (points[1], points[2]),
            (points[2], points[0]),
        ):
            edge = tuple(sorted((first, second)))
            counts[edge] = counts.get(edge, 0) + 1
    return bool(counts) and all(count == 2 for count in counts.values())


def _point_key_mm(point: Sequence[float]) -> tuple[int, int, int]:
    return tuple(round(value / POSITION_WELD_TOLERANCE_MM) for value in point)


def _bounds_strictly_contains(
    outer: tuple[tuple[float, float, float], tuple[float, float, float]],
    inner: tuple[tuple[float, float, float], tuple[float, float, float]],
) -> bool:
    return all(
        outer[0][axis] + POSITION_WELD_TOLERANCE_MM < inner[0][axis]
        and outer[1][axis] - POSITION_WELD_TOLERANCE_MM > inner[1][axis]
        for axis in range(3)
    )


def _edge_counts(
    positions: Sequence[Sequence[float]], triangles: Iterable[tuple[int, int, int]]
) -> tuple[int, int]:
    vertex_ids = _welded_vertex_ids(positions)
    counts: dict[tuple[int, int], int] = {}
    for triangle in triangles:
        rendered = [vertex_ids[index] for index in triangle]
        for first, second in (
            (rendered[0], rendered[1]),
            (rendered[1], rendered[2]),
            (rendered[2], rendered[0]),
        ):
            edge = tuple(sorted((first, second)))
            counts[edge] = counts.get(edge, 0) + 1
    return sum(value == 1 for value in counts.values()), sum(value > 2 for value in counts.values())


def _world_point(
    transform: Transform, mirror_axis: str, point: Sequence[float]
) -> tuple[float, float, float]:
    x, y, z = transform.rotation
    cx, cy, cz = math.cos(x), math.cos(y), math.cos(z)
    sx, sy, sz = math.sin(x), math.sin(y), math.sin(z)
    matrix = (
        (cy * cz, cz * sx * sy - cx * sz, sx * sz + cx * cz * sy),
        (cy * sz, cx * cz + sx * sy * sz, cx * sy * sz - cz * sx),
        (-sy, cy * sx, cx * cy),
    )
    scale = list(transform.scale)
    if mirror_axis != "none":
        scale[{"x": 0, "y": 1, "z": 2}[mirror_axis]] *= -1
    local = tuple(point[axis] * scale[axis] for axis in range(3))
    return tuple(
        transform.position[row] + sum(matrix[row][column] * local[column] for column in range(3))
        for row in range(3)
    )


def _finding(
    suffix: int,
    check_id: str,
    category: str,
    severity: str,
    status: str,
    node_ids: list[str],
    message: str,
    measured_value: float | str | None = None,
    threshold: float | str | None = None,
    suggestion: str = "",
    geometry_refs: list[QualityGeometryReference] | None = None,
) -> QualityFinding:
    return QualityFinding(
        finding_id=f"finding_pending_{suffix}",
        check_id=check_id,
        category=category,
        severity=severity,
        status=status,
        node_ids=node_ids,
        geometry_refs=geometry_refs or [],
        measured_value=measured_value,
        threshold=threshold,
        message=message,
        suggestion=suggestion,
    )
