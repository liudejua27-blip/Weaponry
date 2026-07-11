from __future__ import annotations

from dataclasses import dataclass
from typing import Sequence


Point3 = tuple[float, float, float]
Triangle3 = tuple[Point3, Point3, Point3]
Bounds3 = tuple[Point3, Point3]

_AXIS_EPSILON_SQUARED = 1e-18
_PROJECTION_EPSILON_MM = 1e-7
_RAY_EPSILON_MM = 1e-7
_BVH_LEAF_SIZE = 8
_DEFAULT_MAX_HITS = 128
_RAY_DIRECTIONS: tuple[Point3, ...] = (
    (1.0, 0.3713906763541037, 0.1732050807568877),
    (0.21997067253202995, 1.0, 0.4142135623730951),
    (0.31783724519578227, 0.13165249758739583, 1.0),
)


@dataclass(frozen=True)
class TriangleIntersectionResult:
    intersection_count: int
    tested_triangle_pairs: int
    capped: bool
    containment: bool
    hit_pairs: tuple[tuple[int, int], ...] = ()


@dataclass(frozen=True)
class _BvhNode:
    bounds: Bounds3
    triangle_indices: tuple[int, ...] = ()
    left: _BvhNode | None = None
    right: _BvhNode | None = None


def triangle_bounds(triangle: Triangle3) -> Bounds3:
    return (
        tuple(min(point[axis] for point in triangle) for axis in range(3)),
        tuple(max(point[axis] for point in triangle) for axis in range(3)),
    )


def mesh_bounds(triangles: Sequence[Triangle3]) -> Bounds3:
    if not triangles:
        return ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0))
    triangle_aabbs = [triangle_bounds(triangle) for triangle in triangles]
    return _union_bounds(triangle_aabbs)


def triangles_intersect(first: Triangle3, second: Triangle3) -> bool:
    """Return whether two non-degenerate triangles intersect or touch in 3D."""
    if not _bounds_overlap(triangle_bounds(first), triangle_bounds(second)):
        return False
    first_edges = _triangle_edges(first)
    second_edges = _triangle_edges(second)
    first_normal = _cross(first_edges[0], first_edges[1])
    second_normal = _cross(second_edges[0], second_edges[1])
    axes = [
        first_normal,
        second_normal,
        *(
            _cross(first_edge, second_edge)
            for first_edge in first_edges
            for second_edge in second_edges
        ),
        *(_cross(first_normal, edge) for edge in first_edges),
        *(_cross(second_normal, edge) for edge in second_edges),
    ]
    for axis in axes:
        if _dot(axis, axis) <= _AXIS_EPSILON_SQUARED:
            continue
        first_projection = [_dot(point, axis) for point in first]
        second_projection = [_dot(point, axis) for point in second]
        if (
            max(first_projection) < min(second_projection) - _PROJECTION_EPSILON_MM
            or max(second_projection) < min(first_projection) - _PROJECTION_EPSILON_MM
        ):
            return False
    return True


def inspect_triangle_mesh_intersection(
    first: Sequence[Triangle3],
    second: Sequence[Triangle3],
    *,
    first_is_closed: bool = False,
    second_is_closed: bool = False,
    max_hits: int = _DEFAULT_MAX_HITS,
) -> TriangleIntersectionResult:
    """Run BVH broad phase, triangle SAT narrow phase, then closed-mesh containment."""
    if not first or not second:
        return TriangleIntersectionResult(0, 0, False, False, ())
    first_bounds = [triangle_bounds(triangle) for triangle in first]
    second_bounds = [triangle_bounds(triangle) for triangle in second]
    first_root = _build_bvh(first_bounds, tuple(range(len(first))))
    second_root = _build_bvh(second_bounds, tuple(range(len(second))))
    stack = [(first_root, second_root)]
    tested = 0
    hits = 0
    hit_pairs: list[tuple[int, int]] = []
    while stack:
        first_node, second_node = stack.pop()
        if not _bounds_overlap(first_node.bounds, second_node.bounds):
            continue
        if first_node.triangle_indices and second_node.triangle_indices:
            for first_index in first_node.triangle_indices:
                for second_index in second_node.triangle_indices:
                    if not _bounds_overlap(first_bounds[first_index], second_bounds[second_index]):
                        continue
                    tested += 1
                    if triangles_intersect(first[first_index], second[second_index]):
                        hits += 1
                        hit_pairs.append((first_index, second_index))
                        if hits >= max_hits:
                            return TriangleIntersectionResult(
                                hits, tested, True, False, tuple(hit_pairs)
                            )
            continue
        if first_node.triangle_indices:
            if second_node.left is not None:
                stack.append((first_node, second_node.left))
            if second_node.right is not None:
                stack.append((first_node, second_node.right))
            continue
        if second_node.triangle_indices:
            if first_node.left is not None:
                stack.append((first_node.left, second_node))
            if first_node.right is not None:
                stack.append((first_node.right, second_node))
            continue
        if first_node.left is not None and second_node.left is not None:
            stack.append((first_node.left, second_node.left))
        if first_node.left is not None and second_node.right is not None:
            stack.append((first_node.left, second_node.right))
        if first_node.right is not None and second_node.left is not None:
            stack.append((first_node.right, second_node.left))
        if first_node.right is not None and second_node.right is not None:
            stack.append((first_node.right, second_node.right))
    containment = False
    if hits == 0 and first_is_closed and second_is_closed:
        first_mesh_bounds = first_root.bounds
        second_mesh_bounds = second_root.bounds
        if _bounds_overlap(first_mesh_bounds, second_mesh_bounds):
            containment = _mesh_has_point_inside(
                first, second, second_mesh_bounds
            ) or _mesh_has_point_inside(second, first, first_mesh_bounds)
    return TriangleIntersectionResult(hits, tested, False, containment, tuple(hit_pairs))


def point_inside_closed_mesh(point: Point3, triangles: Sequence[Triangle3]) -> bool:
    """Classify a point by majority parity from three deterministic ray directions."""
    inside_votes = 0
    for direction in _RAY_DIRECTIONS:
        distances = sorted(
            distance
            for triangle in triangles
            if (distance := _ray_triangle_distance(point, direction, triangle)) is not None
        )
        unique_distances: list[float] = []
        for distance in distances:
            if not unique_distances or abs(distance - unique_distances[-1]) > _RAY_EPSILON_MM:
                unique_distances.append(distance)
        inside_votes += len(unique_distances) % 2
    return inside_votes >= 2


def _mesh_has_point_inside(
    candidates: Sequence[Triangle3],
    container: Sequence[Triangle3],
    container_bounds: Bounds3,
) -> bool:
    sampled: list[Point3] = []
    for triangle in candidates:
        for point in (*triangle, _triangle_centroid(triangle)):
            if _point_in_bounds(point, container_bounds):
                sampled.append(point)
                if len(sampled) >= 16:
                    break
        if len(sampled) >= 16:
            break
    return any(point_inside_closed_mesh(point, container) for point in sampled)


def _build_bvh(bounds: Sequence[Bounds3], indices: tuple[int, ...]) -> _BvhNode:
    node_bounds = _union_bounds([bounds[index] for index in indices])
    if len(indices) <= _BVH_LEAF_SIZE:
        return _BvhNode(node_bounds, indices)
    centroids = {
        index: tuple((bounds[index][0][axis] + bounds[index][1][axis]) * 0.5 for axis in range(3))
        for index in indices
    }
    spans = [
        max(centroids[index][axis] for index in indices)
        - min(centroids[index][axis] for index in indices)
        for axis in range(3)
    ]
    split_axis = max(range(3), key=lambda axis: (spans[axis], -axis))
    ordered = tuple(sorted(indices, key=lambda index: (centroids[index][split_axis], index)))
    midpoint = len(ordered) // 2
    return _BvhNode(
        node_bounds,
        left=_build_bvh(bounds, ordered[:midpoint]),
        right=_build_bvh(bounds, ordered[midpoint:]),
    )


def _triangle_edges(triangle: Triangle3) -> tuple[Point3, Point3, Point3]:
    return (
        _subtract(triangle[1], triangle[0]),
        _subtract(triangle[2], triangle[1]),
        _subtract(triangle[0], triangle[2]),
    )


def _triangle_centroid(triangle: Triangle3) -> Point3:
    return tuple(sum(point[axis] for point in triangle) / 3.0 for axis in range(3))


def _ray_triangle_distance(origin: Point3, direction: Point3, triangle: Triangle3) -> float | None:
    first_edge = _subtract(triangle[1], triangle[0])
    second_edge = _subtract(triangle[2], triangle[0])
    p_vector = _cross(direction, second_edge)
    determinant = _dot(first_edge, p_vector)
    if abs(determinant) <= _RAY_EPSILON_MM:
        return None
    inverse = 1.0 / determinant
    t_vector = _subtract(origin, triangle[0])
    u = _dot(t_vector, p_vector) * inverse
    if u < -_RAY_EPSILON_MM or u > 1.0 + _RAY_EPSILON_MM:
        return None
    q_vector = _cross(t_vector, first_edge)
    v = _dot(direction, q_vector) * inverse
    if v < -_RAY_EPSILON_MM or u + v > 1.0 + _RAY_EPSILON_MM:
        return None
    distance = _dot(second_edge, q_vector) * inverse
    return distance if distance > _RAY_EPSILON_MM else None


def _union_bounds(bounds: Sequence[Bounds3]) -> Bounds3:
    return (
        tuple(min(item[0][axis] for item in bounds) for axis in range(3)),
        tuple(max(item[1][axis] for item in bounds) for axis in range(3)),
    )


def _bounds_overlap(first: Bounds3, second: Bounds3) -> bool:
    return all(
        first[1][axis] >= second[0][axis] - _PROJECTION_EPSILON_MM
        and second[1][axis] >= first[0][axis] - _PROJECTION_EPSILON_MM
        for axis in range(3)
    )


def _point_in_bounds(point: Point3, bounds: Bounds3) -> bool:
    return all(
        bounds[0][axis] - _PROJECTION_EPSILON_MM
        <= point[axis]
        <= bounds[1][axis] + _PROJECTION_EPSILON_MM
        for axis in range(3)
    )


def _subtract(first: Sequence[float], second: Sequence[float]) -> Point3:
    return tuple(first[axis] - second[axis] for axis in range(3))


def _cross(first: Sequence[float], second: Sequence[float]) -> Point3:
    return (
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    )


def _dot(first: Sequence[float], second: Sequence[float]) -> float:
    return sum(first[axis] * second[axis] for axis in range(3))
