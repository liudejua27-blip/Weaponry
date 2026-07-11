from __future__ import annotations

import math
import re
import struct
from dataclasses import dataclass
from typing import Any, Iterable, Sequence

from forgecad_agent.application.combined_glb import CombinedGlbError, read_glb


class CombinedObjError(ValueError):
    pass


@dataclass(frozen=True)
class CombinedObjResult:
    obj: bytes
    mtl: bytes
    vertex_count: int
    triangle_count: int


_COMPONENT_FORMATS = {
    5120: ("b", 1),
    5121: ("B", 1),
    5122: ("h", 2),
    5123: ("H", 2),
    5125: ("I", 4),
    5126: ("f", 4),
}
_TYPE_WIDTHS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}
_IDENTITY = (
    (1.0, 0.0, 0.0, 0.0),
    (0.0, 1.0, 0.0, 0.0),
    (0.0, 0.0, 1.0, 0.0),
    (0.0, 0.0, 0.0, 1.0),
)


def build_combined_obj(combined_glb: bytes) -> CombinedObjResult:
    try:
        document, binary = read_glb(combined_glb)
    except CombinedGlbError as exc:
        raise CombinedObjError(str(exc)) from exc
    _assert_supported(document)
    scene_index = int(document.get("scene", 0))
    scenes = document.get("scenes", [])
    if not 0 <= scene_index < len(scenes):
        raise CombinedObjError("combined GLB active scene is missing")

    materials = document.get("materials", [])
    material_names = [
        f"mat_{index:03d}_{_safe_name(material.get('name', 'material'))}"
        for index, material in enumerate(materials)
    ]
    default_material = "mat_default"
    obj_lines = [
        "# ForgeCAD combined OBJ",
        "# units: meter",
        "# source: immutable Concept Version + ModuleGraph + GLB modules",
        "mtllib combined.mtl",
    ]
    mtl_lines = ["# ForgeCAD deterministic material projection"]
    for name, material in zip(material_names, materials):
        mtl_lines.extend(_material_lines(name, material))
    mtl_lines.extend(_material_lines(default_material, {}))

    nodes = document.get("nodes", [])
    meshes = document.get("meshes", [])
    vertex_offset = 0
    uv_offset = 0
    normal_offset = 0
    triangle_count = 0

    def visit(
        node_index: int,
        parent_matrix: tuple[tuple[float, ...], ...],
        path: tuple[str, ...],
        ancestry: frozenset[int],
    ) -> None:
        nonlocal vertex_offset, uv_offset, normal_offset, triangle_count
        if node_index in ancestry:
            raise CombinedObjError("combined GLB node hierarchy contains a cycle")
        if not 0 <= node_index < len(nodes):
            raise CombinedObjError("combined GLB scene references an invalid node")
        node = nodes[node_index]
        node_name = _safe_name(node.get("name", f"node_{node_index}"))
        current_path = (*path, node_name)
        world = _multiply_matrix(parent_matrix, _node_matrix(node))
        if "mesh" in node:
            if abs(_determinant3(world)) <= 1e-12:
                raise CombinedObjError("mesh node transform is singular")
            mesh_index = int(node["mesh"])
            if not 0 <= mesh_index < len(meshes):
                raise CombinedObjError("combined GLB node references an invalid mesh")
            mesh = meshes[mesh_index]
            object_name = "__".join(current_path)
            obj_lines.append(f"o {object_name}")
            for primitive_index, primitive in enumerate(mesh.get("primitives", [])):
                if int(primitive.get("mode", 4)) != 4:
                    raise CombinedObjError("combined OBJ supports TRIANGLES primitives only")
                if primitive.get("extensions") or primitive.get("targets"):
                    raise CombinedObjError("extended or morphed primitives are not supported")
                attributes = primitive.get("attributes", {})
                if "POSITION" not in attributes:
                    raise CombinedObjError("mesh primitive is missing POSITION")
                positions = _vectors(document, binary, int(attributes["POSITION"]), 3)
                normals = (
                    _vectors(document, binary, int(attributes["NORMAL"]), 3)
                    if "NORMAL" in attributes
                    else []
                )
                uvs = (
                    _vectors(document, binary, int(attributes["TEXCOORD_0"]), 2)
                    if "TEXCOORD_0" in attributes
                    else []
                )
                if normals and len(normals) != len(positions):
                    raise CombinedObjError("NORMAL count must match POSITION count")
                if uvs and len(uvs) != len(positions):
                    raise CombinedObjError("TEXCOORD_0 count must match POSITION count")
                indices = (
                    _indices(document, binary, int(primitive["indices"]))
                    if "indices" in primitive
                    else list(range(len(positions)))
                )
                if len(indices) % 3:
                    raise CombinedObjError("triangle index count must be divisible by three")
                if any(index < 0 or index >= len(positions) for index in indices):
                    raise CombinedObjError("triangle index is outside POSITION range")
                transformed_positions = [_transform_point(world, point) for point in positions]
                transformed_normals = [_transform_normal(world, normal) for normal in normals]
                obj_lines.append(f"g {object_name}__primitive_{primitive_index:03d}")
                material_index = primitive.get("material")
                material_name = (
                    material_names[int(material_index)]
                    if material_index is not None and 0 <= int(material_index) < len(material_names)
                    else default_material
                )
                obj_lines.append(f"usemtl {material_name}")
                obj_lines.extend(
                    f"v {_number(x)} {_number(y)} {_number(z)}" for x, y, z in transformed_positions
                )
                obj_lines.extend(f"vt {_number(u)} {_number(v)}" for u, v in uvs)
                obj_lines.extend(
                    f"vn {_number(x)} {_number(y)} {_number(z)}" for x, y, z in transformed_normals
                )
                mirrored = _determinant3(world) < 0
                for start in range(0, len(indices), 3):
                    triangle = indices[start : start + 3]
                    if mirrored:
                        triangle = [triangle[0], triangle[2], triangle[1]]
                    obj_lines.append(
                        "f "
                        + " ".join(
                            _face_vertex(
                                index,
                                vertex_offset,
                                uv_offset if uvs else None,
                                normal_offset if normals else None,
                            )
                            for index in triangle
                        )
                    )
                vertex_offset += len(positions)
                uv_offset += len(uvs)
                normal_offset += len(normals)
                triangle_count += len(indices) // 3
        next_ancestry = ancestry | {node_index}
        for child in node.get("children", []):
            visit(int(child), world, current_path, next_ancestry)

    for root in scenes[scene_index].get("nodes", []):
        visit(int(root), _IDENTITY, (), frozenset())
    if vertex_offset == 0 or triangle_count == 0:
        raise CombinedObjError("combined GLB does not contain triangle geometry")
    obj_lines.append(f"# summary: {vertex_offset} vertices, {triangle_count} triangles")
    return CombinedObjResult(
        obj=("\n".join(obj_lines) + "\n").encode("utf-8"),
        mtl=("\n".join(mtl_lines) + "\n").encode("utf-8"),
        vertex_count=vertex_offset,
        triangle_count=triangle_count,
    )


def _assert_supported(document: dict[str, Any]) -> None:
    unsupported = [name for name in ("animations", "skins", "cameras") if document.get(name)]
    if unsupported:
        raise CombinedObjError(f"combined OBJ does not support: {', '.join(unsupported)}")
    if document.get("extensionsRequired"):
        raise CombinedObjError("required glTF extensions are not supported")


def _accessor(
    document: dict[str, Any],
    binary: bytes,
    accessor_index: int,
) -> list[tuple[float | int, ...]]:
    try:
        accessor = document["accessors"][accessor_index]
        if accessor.get("sparse"):
            raise CombinedObjError("sparse accessors are not supported")
        view = document["bufferViews"][int(accessor["bufferView"])]
        if int(view.get("buffer", 0)) != 0:
            raise CombinedObjError("only the embedded GLB buffer is supported")
        component_type = int(accessor["componentType"])
        component_format, component_size = _COMPONENT_FORMATS[component_type]
        width = _TYPE_WIDTHS[str(accessor["type"])]
        element_size = component_size * width
        stride = int(view.get("byteStride", element_size))
        if stride < element_size:
            raise CombinedObjError("accessor byteStride is smaller than its element")
        offset = int(view.get("byteOffset", 0)) + int(accessor.get("byteOffset", 0))
        count = int(accessor["count"])
        normalized = bool(accessor.get("normalized", False))
        result: list[tuple[float | int, ...]] = []
        for index in range(count):
            start = offset + index * stride
            if start + element_size > len(binary):
                raise CombinedObjError("accessor exceeds embedded GLB buffer")
            values = struct.unpack_from("<" + component_format * width, binary, start)
            result.append(
                tuple(
                    _normalized(value, component_type) if normalized else value for value in values
                )
            )
        return result
    except (IndexError, KeyError, TypeError, ValueError, struct.error) as exc:
        if isinstance(exc, CombinedObjError):
            raise
        raise CombinedObjError(f"invalid glTF accessor {accessor_index}: {exc}") from exc


def _vectors(
    document: dict[str, Any],
    binary: bytes,
    accessor_index: int,
    width: int,
) -> list[tuple[float, ...]]:
    values = _accessor(document, binary, accessor_index)
    if any(len(value) != width for value in values):
        raise CombinedObjError(f"accessor must be VEC{width}")
    rendered = [tuple(float(component) for component in value) for value in values]
    if any(not all(math.isfinite(component) for component in value) for value in rendered):
        raise CombinedObjError("accessor contains a non-finite value")
    return rendered


def _indices(document: dict[str, Any], binary: bytes, accessor_index: int) -> list[int]:
    accessor = document["accessors"][accessor_index]
    if int(accessor["componentType"]) not in {5121, 5123, 5125}:
        raise CombinedObjError("indices must use an unsigned integer component type")
    if accessor.get("normalized"):
        raise CombinedObjError("indices accessor cannot be normalized")
    values = _accessor(document, binary, accessor_index)
    if any(len(value) != 1 for value in values):
        raise CombinedObjError("indices accessor must be SCALAR")
    return [int(value[0]) for value in values]


def _node_matrix(node: dict[str, Any]) -> tuple[tuple[float, ...], ...]:
    if "matrix" in node:
        values = [float(value) for value in node["matrix"]]
        if len(values) != 16 or not all(math.isfinite(value) for value in values):
            raise CombinedObjError("node matrix must contain 16 finite values")
        matrix = tuple(tuple(values[column * 4 + row] for column in range(4)) for row in range(4))
        if any(
            abs(matrix[3][index] - expected) > 1e-9 for index, expected in enumerate((0, 0, 0, 1))
        ):
            raise CombinedObjError("node matrix must be affine")
        return matrix
    translation = _finite_vector(node.get("translation", [0, 0, 0]), 3, "translation")
    rotation = _finite_vector(node.get("rotation", [0, 0, 0, 1]), 4, "rotation")
    scale = _finite_vector(node.get("scale", [1, 1, 1]), 3, "scale")
    x, y, z, w = rotation
    length = math.sqrt(x * x + y * y + z * z + w * w)
    if length <= 1e-12:
        raise CombinedObjError("node quaternion has zero length")
    x, y, z, w = (value / length for value in rotation)
    sx, sy, sz = scale
    tx, ty, tz = translation
    return (
        ((1 - 2 * (y * y + z * z)) * sx, 2 * (x * y - z * w) * sy, 2 * (x * z + y * w) * sz, tx),
        (2 * (x * y + z * w) * sx, (1 - 2 * (x * x + z * z)) * sy, 2 * (y * z - x * w) * sz, ty),
        (2 * (x * z - y * w) * sx, 2 * (y * z + x * w) * sy, (1 - 2 * (x * x + y * y)) * sz, tz),
        (0.0, 0.0, 0.0, 1.0),
    )


def _multiply_matrix(
    first: tuple[tuple[float, ...], ...],
    second: tuple[tuple[float, ...], ...],
) -> tuple[tuple[float, ...], ...]:
    return tuple(
        tuple(
            sum(first[row][index] * second[index][column] for index in range(4))
            for column in range(4)
        )
        for row in range(4)
    )


def _transform_point(
    matrix: tuple[tuple[float, ...], ...],
    point: Sequence[float],
) -> tuple[float, float, float]:
    rendered = tuple(
        sum(matrix[row][index] * point[index] for index in range(3)) + matrix[row][3]
        for row in range(3)
    )
    if not all(math.isfinite(value) for value in rendered):
        raise CombinedObjError("transformed vertex is not finite")
    return rendered  # type: ignore[return-value]


def _transform_normal(
    matrix: tuple[tuple[float, ...], ...],
    normal: Sequence[float],
) -> tuple[float, float, float]:
    a = matrix
    cofactors = (
        (
            a[1][1] * a[2][2] - a[1][2] * a[2][1],
            a[1][2] * a[2][0] - a[1][0] * a[2][2],
            a[1][0] * a[2][1] - a[1][1] * a[2][0],
        ),
        (
            a[0][2] * a[2][1] - a[0][1] * a[2][2],
            a[0][0] * a[2][2] - a[0][2] * a[2][0],
            a[0][1] * a[2][0] - a[0][0] * a[2][1],
        ),
        (
            a[0][1] * a[1][2] - a[0][2] * a[1][1],
            a[0][2] * a[1][0] - a[0][0] * a[1][2],
            a[0][0] * a[1][1] - a[0][1] * a[1][0],
        ),
    )
    rendered = tuple(
        sum(cofactors[row][column] * normal[column] for column in range(3)) for row in range(3)
    )
    if not all(math.isfinite(value) for value in rendered):
        raise CombinedObjError("transformed normal is not finite")
    length = math.sqrt(sum(value * value for value in rendered))
    if length <= 1e-12:
        raise CombinedObjError("node transform makes a normal singular")
    return tuple(value / length for value in rendered)  # type: ignore[return-value]


def _determinant3(matrix: tuple[tuple[float, ...], ...]) -> float:
    return (
        matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0])
    )


def _face_vertex(
    index: int,
    vertex_offset: int,
    uv_offset: int | None,
    normal_offset: int | None,
) -> str:
    vertex = vertex_offset + index + 1
    uv = uv_offset + index + 1 if uv_offset is not None else None
    normal = normal_offset + index + 1 if normal_offset is not None else None
    if uv is not None and normal is not None:
        return f"{vertex}/{uv}/{normal}"
    if uv is not None:
        return f"{vertex}/{uv}"
    if normal is not None:
        return f"{vertex}//{normal}"
    return str(vertex)


def _material_lines(name: str, material: dict[str, Any]) -> list[str]:
    pbr = material.get("pbrMetallicRoughness", {})
    base = _float_values(pbr.get("baseColorFactor", [0.8, 0.8, 0.8, 1.0]), 4)
    emissive = _float_values(material.get("emissiveFactor", [0, 0, 0]), 3)
    roughness = float(pbr.get("roughnessFactor", 1.0))
    if not math.isfinite(roughness):
        raise CombinedObjError("material roughnessFactor must be finite")
    roughness = min(1.0, max(0.0, roughness))
    return [
        "",
        f"newmtl {name}",
        f"Kd {_number(base[0])} {_number(base[1])} {_number(base[2])}",
        f"d {_number(base[3])}",
        f"Ns {_number(max(1.0, (1.0 - roughness) * 1000.0))}",
        f"Ke {_number(emissive[0])} {_number(emissive[1])} {_number(emissive[2])}",
        "illum 2",
    ]


def _normalized(value: float | int, component_type: int) -> float:
    if component_type == 5120:
        return max(float(value) / 127.0, -1.0)
    if component_type == 5121:
        return float(value) / 255.0
    if component_type == 5122:
        return max(float(value) / 32767.0, -1.0)
    if component_type == 5123:
        return float(value) / 65535.0
    if component_type == 5125:
        return float(value) / 4294967295.0
    return float(value)


def _finite_vector(value: Iterable[Any], width: int, label: str) -> tuple[float, ...]:
    rendered = tuple(float(component) for component in value)
    if len(rendered) != width or not all(math.isfinite(component) for component in rendered):
        raise CombinedObjError(f"node {label} must contain {width} finite values")
    return rendered


def _float_values(value: Iterable[Any], width: int) -> tuple[float, ...]:
    rendered = tuple(float(component) for component in value)
    if len(rendered) != width or not all(math.isfinite(component) for component in rendered):
        raise CombinedObjError("material factor contains invalid values")
    return rendered


def _safe_name(value: Any) -> str:
    rendered = re.sub(r"[^A-Za-z0-9_.-]+", "_", str(value)).strip("_")
    return rendered or "unnamed"


def _number(value: float) -> str:
    rendered = f"{value:.9f}".rstrip("0").rstrip(".")
    return "0" if rendered in {"", "-0"} else rendered
