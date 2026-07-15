#!/usr/bin/env python3
"""Isolated Manifold Python benchmark adapter; never imported by production."""

from __future__ import annotations

import hashlib
import json
import resource
import statistics
import sys
import time

import manifold3d as manifold


def digest(value: object) -> str:
    mesh = value.to_mesh()
    payload = mesh.vert_properties.tobytes() + mesh.tri_verts.tobytes()
    return hashlib.sha256(payload).hexdigest()


def run_once(size: tuple[float, float, float], tool_size: tuple[float, float, float], offset: tuple[float, float, float], operation: str) -> tuple[str, int]:
    base = manifold.Manifold.cube(size, True)
    tool = manifold.Manifold.cube(tool_size, True).translate(offset)
    result = base + tool if operation == "union" else base - tool
    return digest(result), result.num_tri()


fixtures = {
    "vehicle_window_subtract": ((1800, 800, 600), (520, 500, 700), (250, 0, 0), "subtract"),
    "aircraft_canopy_subtract": ((1600, 650, 500), (700, 420, 520), (120, 80, 0), "subtract"),
    "appliance_vent_subtract": ((900, 700, 1100), (500, 800, 260), (0, 0, 350), "subtract"),
    "robot_arm_housing_union": ((700, 700, 800), (480, 480, 900), (260, 0, 0), "union"),
}
started = time.perf_counter()
first_hash, first_triangles = run_once(*fixtures["vehicle_window_subtract"])
cold_ms = (time.perf_counter() - started) * 1000
durations: list[float] = []
fixture_results: dict[str, dict[str, object]] = {}
for fixture_id, fixture in fixtures.items():
    hashes: list[str] = []
    triangles = 0
    for _index in range(5):
        begin = time.perf_counter()
        candidate_hash, triangles = run_once(*fixture)
        durations.append((time.perf_counter() - begin) * 1000)
        hashes.append(candidate_hash)
    fixture_results[fixture_id] = {"triangle_count": triangles, "deterministic": len(set(hashes)) == 1, "sha256": hashes[0]}
for index in range(4):
    begin = time.perf_counter()
    run_once((1000, 700, 500), (420, 520, 620), (160 + index * 1e-7, 0, 0), "subtract")
    durations.append((time.perf_counter() - begin) * 1000)
coplanar_hash, _ = run_once((1000, 700, 500), (420, 520, 620), (290, 0, 0), "subtract")
near_hash, _ = run_once((1000, 700, 500), (420, 520, 620), (289.999999, 0, 0), "subtract")
print(json.dumps({
    "adapter": "manifold_python",
    "cold_ms": round(cold_ms, 4),
    "warm_median_ms": round(statistics.median(durations), 4),
    "peak_rss_kib": round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024) if sys.platform == "darwin" else resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
    "triangle_count": first_triangles,
    "deterministic_identical_fixture": fixture_results["vehicle_window_subtract"]["sha256"] == first_hash,
    "fixture_results": fixture_results,
    "near_degenerate_completed": bool(near_hash),
    "coplanar_completed": bool(coplanar_hash),
    "material_surface_provenance_verified": False,
    "cancellation_verified": False,
}, sort_keys=True))
