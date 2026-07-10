from __future__ import annotations

import math
from typing import Iterable

from forgecad_agent.domain.concepts.models import Transform


Quaternion = tuple[float, float, float, float]
Vector3 = tuple[float, float, float]


def snap_child_transform(
    *,
    parent_transform: Transform,
    parent_connector: Transform,
    child_scale: Iterable[float],
    child_connector: Transform,
) -> Transform:
    """Place a child node so both connector frames coincide in millimeter world space."""

    parent_rotation = _quaternion_from_euler(parent_transform.rotation)
    parent_connector_rotation = _quaternion_from_euler(parent_connector.rotation)
    child_connector_rotation = _quaternion_from_euler(child_connector.rotation)
    connector_world_rotation = _quaternion_multiply(
        parent_rotation,
        parent_connector_rotation,
    )
    child_rotation = _quaternion_normalize(
        _quaternion_multiply(
            connector_world_rotation,
            _quaternion_inverse(child_connector_rotation),
        )
    )
    parent_anchor = _add(
        _vector(parent_transform.position),
        _rotate(
            parent_rotation,
            _multiply_components(
                _vector(parent_transform.scale),
                _vector(parent_connector.position),
            ),
        ),
    )
    child_scale_vector = _vector(child_scale)
    child_offset = _rotate(
        child_rotation,
        _multiply_components(child_scale_vector, _vector(child_connector.position)),
    )
    child_position = _subtract(parent_anchor, child_offset)
    return Transform(
        position=list(child_position),
        rotation=list(_euler_from_quaternion(child_rotation)),
        scale=list(child_scale_vector),
    )


def connector_alignment_error(
    *,
    first_transform: Transform,
    first_connector: Transform,
    second_transform: Transform,
    second_connector: Transform,
) -> tuple[float, float]:
    """Return connector origin distance in mm and frame rotation error in degrees."""

    first_position, first_rotation = _connector_world_frame(
        first_transform,
        first_connector,
    )
    second_position, second_rotation = _connector_world_frame(
        second_transform,
        second_connector,
    )
    distance = math.sqrt(sum((a - b) ** 2 for a, b in zip(first_position, second_position)))
    dot = abs(sum(a * b for a, b in zip(first_rotation, second_rotation)))
    dot = max(-1.0, min(1.0, dot))
    rotation_degrees = math.degrees(2 * math.acos(dot))
    return distance, rotation_degrees


def _connector_world_frame(
    node_transform: Transform,
    connector_transform: Transform,
) -> tuple[Vector3, Quaternion]:
    node_rotation = _quaternion_from_euler(node_transform.rotation)
    position = _add(
        _vector(node_transform.position),
        _rotate(
            node_rotation,
            _multiply_components(
                _vector(node_transform.scale),
                _vector(connector_transform.position),
            ),
        ),
    )
    rotation = _quaternion_normalize(
        _quaternion_multiply(
            node_rotation,
            _quaternion_from_euler(connector_transform.rotation),
        )
    )
    return position, rotation


def _quaternion_from_euler(rotation: Iterable[float]) -> Quaternion:
    x, y, z = _vector(rotation)
    c1, c2, c3 = math.cos(x / 2), math.cos(y / 2), math.cos(z / 2)
    s1, s2, s3 = math.sin(x / 2), math.sin(y / 2), math.sin(z / 2)
    return _quaternion_normalize(
        (
            s1 * c2 * c3 + c1 * s2 * s3,
            c1 * s2 * c3 - s1 * c2 * s3,
            c1 * c2 * s3 + s1 * s2 * c3,
            c1 * c2 * c3 - s1 * s2 * s3,
        )
    )


def _euler_from_quaternion(value: Quaternion) -> Vector3:
    x, y, z, w = _quaternion_normalize(value)
    m11 = 1 - 2 * (y * y + z * z)
    m12 = 2 * (x * y - z * w)
    m13 = 2 * (x * z + y * w)
    m22 = 1 - 2 * (x * x + z * z)
    m23 = 2 * (y * z - x * w)
    m32 = 2 * (y * z + x * w)
    m33 = 1 - 2 * (x * x + y * y)
    rotation_y = math.asin(max(-1.0, min(1.0, m13)))
    if abs(m13) < 0.9999999:
        rotation_x = math.atan2(-m23, m33)
        rotation_z = math.atan2(-m12, m11)
    else:
        rotation_x = math.atan2(m32, m22)
        rotation_z = 0.0
    return rotation_x, rotation_y, rotation_z


def _quaternion_multiply(first: Quaternion, second: Quaternion) -> Quaternion:
    ax, ay, az, aw = first
    bx, by, bz, bw = second
    return (
        ax * bw + aw * bx + ay * bz - az * by,
        ay * bw + aw * by + az * bx - ax * bz,
        az * bw + aw * bz + ax * by - ay * bx,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def _quaternion_inverse(value: Quaternion) -> Quaternion:
    x, y, z, w = _quaternion_normalize(value)
    return -x, -y, -z, w


def _quaternion_normalize(value: Quaternion) -> Quaternion:
    length = math.sqrt(sum(component * component for component in value))
    if length <= 1e-12:
        raise ValueError("connector rotation produced a zero-length quaternion")
    return tuple(component / length for component in value)  # type: ignore[return-value]


def _rotate(rotation: Quaternion, vector: Vector3) -> Vector3:
    x, y, z, w = rotation
    vx, vy, vz = vector
    tx = 2 * (y * vz - z * vy)
    ty = 2 * (z * vx - x * vz)
    tz = 2 * (x * vy - y * vx)
    return (
        vx + w * tx + (y * tz - z * ty),
        vy + w * ty + (z * tx - x * tz),
        vz + w * tz + (x * ty - y * tx),
    )


def _vector(value: Iterable[float]) -> Vector3:
    rendered = tuple(float(component) for component in value)
    if len(rendered) != 3:
        raise ValueError("expected three vector components")
    return rendered  # type: ignore[return-value]


def _add(first: Vector3, second: Vector3) -> Vector3:
    return tuple(a + b for a, b in zip(first, second))  # type: ignore[return-value]


def _subtract(first: Vector3, second: Vector3) -> Vector3:
    return tuple(a - b for a, b in zip(first, second))  # type: ignore[return-value]


def _multiply_components(first: Vector3, second: Vector3) -> Vector3:
    return tuple(a * b for a, b in zip(first, second))  # type: ignore[return-value]
