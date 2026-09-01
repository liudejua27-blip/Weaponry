#!/usr/bin/env python3
"""Build one bounded, no-render blade successor from a contour envelope.

The generator reads the current closed ``KnifeSceneProgram@1`` and
``KnifeObjectiveLedger@1`` together with the frozen browser baseline/successor
metric receipts and an authorized ``KnifeContourReference@1``.  It maps the
reference's upper/lower contour envelope into the *existing program frame*,
materializes eight deterministic ``nurbs-like`` control points per rail, and
adds explicit intermediate loft stations.  The parent program is never
overwritten.

This is a mathematical successor proposal, not a render or approval tool.
The browser rig/camera and the previously frozen calibration are evidence
bindings only; this script does not refit them and does not predict a browser
metric pass for the new program.  Guard, grip, pommel, material, presentation,
and all non-blade program fields are required to remain byte-equivalent.
"""

from __future__ import annotations

import argparse
import bisect
import copy
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any, Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import calibrate_browser_reference as browser
import evaluate_metrics as contour_metrics
import search_candidates as search


ROOT = SCRIPT_DIR.parent
DEFAULT_PROGRAM = ROOT / "references" / "dragonfang-first-slice.json"
DEFAULT_LEDGER = ROOT / "references" / "dragonfang-objective-ledger.json"
CONTROL_COUNT_MIN = 6
CONTROL_COUNT_MAX = 12
DEFAULT_CONTROL_COUNT = 8
SECTION_COUNT_MIN = 6
SECTION_COUNT_MAX = 12
DEFAULT_SECTION_COUNT = 8
RAIL_SAMPLE_COUNT = 129
FIT_ALGORITHM = "reference-upper-lower-envelope-to-parent-blade-frame@1"
SECTION_ALGORITHM = "semantic-section-preserve-with-bounded-intermediate-stations@1"
ROLES = ("root", "shoulder", "belly", "tip")
SECTION_FIELDS = ("half_width", "thickness", "edge_offset", "spine_offset", "asymmetry", "twist")


class SuccessorInputError(ValueError):
    """A malformed or incompatible successor input."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise SuccessorInputError(message)


def _number(value: Any, label: str, minimum: float | None = None, maximum: float | None = None) -> None:
    _require(isinstance(value, (int, float)) and not isinstance(value, bool), f"{label} must be numeric")
    number = float(value)
    _require(math.isfinite(number), f"{label} must be finite")
    if minimum is not None:
        _require(number >= minimum, f"{label} must be at least {minimum}")
    if maximum is not None:
        _require(number <= maximum, f"{label} must be at most {maximum}")


def _round(value: float) -> float:
    result = round(float(value), 6)
    return 0.0 if result == 0.0 else result


def _sha(value: Any, label: str) -> None:
    _require(isinstance(value, str) and len(value) == 64 and all(char in "0123456789abcdef" for char in value), f"{label} must be a lowercase SHA-256")


def _load(path: Path) -> dict[str, Any]:
    try:
        return search.load_json(path)
    except (search.InputError, OSError, ValueError) as exc:
        raise SuccessorInputError(str(exc)) from exc


def _validate_browser_chain(baseline: dict[str, Any], successor: dict[str, Any]) -> dict[str, Any]:
    baseline_sha = browser.validate_receipt(baseline)
    successor_sha = browser.validate_receipt(successor)
    _require(baseline["schema_version"] == "KnifeBrowserMetricReceipt@1", "baseline browser receipt schema drifted")
    _require(successor["schema_version"] == "KnifeBrowserMetricReceipt@1", "successor browser receipt schema drifted")
    _require(baseline["evaluation_mode"] == "baseline_calibration", "baseline receipt is not a one-time baseline calibration measurement")
    _require(successor["evaluation_mode"] == "frozen_replay", "successor receipt is not a frozen calibration replay")
    _require(baseline["fit_reused"] is False and baseline["refit_performed"] is True, "baseline fit flags are not closed")
    _require(successor["fit_reused"] is True and successor["refit_performed"] is False, "successor receipt proves no frozen-fit replay")
    _require(baseline["calibration_sha256"] == successor["calibration_sha256"], "baseline and successor use different browser calibrations")
    for key in (
        "reference_sha256",
        "camera_fingerprint",
        "rig_id",
        "rig_fingerprint",
        "frame_width",
        "frame_height",
        "allowed_part_ids",
        "baseline_mask_sha256",
    ):
        _require(baseline[key] == successor[key], f"browser evidence binding drifted for {key}")
    _require(successor["baseline_capture_manifest_sha256"] == baseline["capture_manifest_sha256"], "successor does not point to the frozen baseline capture manifest")
    _require(baseline["statuses"]["quality_status"] == "MEASURED_NOT_APPROVED", "baseline quality status crossed the measurement boundary")
    _require(successor["statuses"]["quality_status"] == "MEASURED_NOT_APPROVED", "successor quality status crossed the measurement boundary")
    _require(baseline["statuses"]["visual_status"] == "NOT_RUN" and successor["statuses"]["visual_status"] == "NOT_RUN", "browser evidence contains visual approval")
    return {
        "baseline_receipt_sha256": baseline_sha,
        "successor_receipt_sha256": successor_sha,
        "calibration_sha256": baseline["calibration_sha256"],
        "reference_sha256": baseline["reference_sha256"],
        "camera_fingerprint": baseline["camera_fingerprint"],
        "rig_id": baseline["rig_id"],
        "rig_fingerprint": baseline["rig_fingerprint"],
        "frame_width": baseline["frame_width"],
        "frame_height": baseline["frame_height"],
        "allowed_part_ids": list(baseline["allowed_part_ids"]),
        "baseline_capture_manifest_sha256": baseline["capture_manifest_sha256"],
        "successor_capture_manifest_sha256": successor["capture_manifest_sha256"],
        "baseline_program_fingerprint": baseline["program_fingerprint"],
        "successor_program_fingerprint": successor["program_fingerprint"],
        "baseline_scene_fingerprint": baseline["scene_fingerprint"],
        "successor_scene_fingerprint": successor["scene_fingerprint"],
        "baseline_metrics": copy.deepcopy(baseline["metrics"]),
        "successor_metrics": copy.deepcopy(successor["metrics"]),
    }


def _validate_sources(program: dict[str, Any], ledger: dict[str, Any], reference: dict[str, Any], baseline: dict[str, Any], successor: dict[str, Any]) -> tuple[str, str, str, dict[str, Any]]:
    program_sha = search.validate_program(program)
    ledger_sha = search.validate_ledger(ledger)
    reference_sha = contour_metrics.validate_reference(reference)
    _require(ledger["program_sha256"] == program_sha, "objective ledger does not bind the supplied current program")
    _require(reference_sha == baseline["reference_sha256"], "authorized contour reference does not match baseline browser receipt")
    _require(reference_sha == successor["reference_sha256"], "authorized contour reference does not match successor browser receipt")
    _require(ledger["allowed_scope"] == ["blade-body", "cutting-edge"], "allowed_scope is not the closed blade-only scope")
    _require(ledger["frozen_parts"] == ["guard", "grip", "pommel"], "frozen_parts drifted from the closed assembly scope")
    browser_chain = _validate_browser_chain(baseline, successor)
    for evidence in (browser_chain["calibration_sha256"], browser_chain["baseline_receipt_sha256"], browser_chain["successor_receipt_sha256"]):
        _require(evidence in ledger["evidence_sha256"], f"objective ledger is missing frozen browser evidence {evidence}")
    return program_sha, ledger_sha, reference_sha, browser_chain


def _path_between(points: list[list[float]], start: int, end: int) -> list[list[float]]:
    result: list[list[float]] = []
    index = start
    while True:
        result.append([float(points[index][0]), float(points[index][1])])
        if index == end:
            return result
        index = (index + 1) % len(points)
        _require(len(result) <= len(points) + 1, "reference contour path did not close")


def _collapse_branch(points: list[list[float]], take_max: bool) -> list[tuple[float, float]]:
    ordered = sorted(points, key=lambda point: (float(point[0]), float(point[1])))
    result: list[tuple[float, float]] = []
    index = 0
    while index < len(ordered):
        x = float(ordered[index][0])
        ys: list[float] = []
        while index < len(ordered) and abs(float(ordered[index][0]) - x) <= 1e-12:
            ys.append(float(ordered[index][1]))
            index += 1
        result.append((x, max(ys) if take_max else min(ys)))
    _require(len(result) >= 3, "reference envelope branch is too short")
    _require(all(left[0] < right[0] for left, right in zip(result, result[1:])), "reference envelope branch has a non-increasing x run")
    return result


def _reference_envelopes(reference: dict[str, Any]) -> dict[str, Any]:
    contour = [[float(point[0]), float(point[1])] for point in reference["outer_contour"]]
    min_index = min(range(len(contour)), key=lambda index: (contour[index][0], index))
    max_index = max(range(len(contour)), key=lambda index: (contour[index][0], -index))
    branch_a = _path_between(contour, min_index, max_index)
    branch_b = list(reversed(_path_between(contour, max_index, min_index)))
    average_a = sum(point[1] for point in branch_a) / len(branch_a)
    average_b = sum(point[1] for point in branch_b) / len(branch_b)
    upper_points = branch_a if average_a >= average_b else branch_b
    lower_points = branch_b if upper_points is branch_a else branch_a
    upper = _collapse_branch(upper_points, take_max=True)
    lower = _collapse_branch(lower_points, take_max=False)
    x_min = min(point[0] for point in contour)
    x_max = max(point[0] for point in contour)
    y_min = min(point[1] for point in contour)
    y_max = max(point[1] for point in contour)
    _require(x_max > x_min and y_max > y_min, "reference contour envelope has zero span")
    for index in range(33):
        u = index / 32.0
        x = x_min + u * (x_max - x_min)
        upper_y = _interpolate_branch(upper, x)
        lower_y = _interpolate_branch(lower, x)
        _require(upper_y > lower_y + 1e-9, f"reference upper/lower envelope crosses at u={u:.6f}")
    return {
        "upper": upper,
        "lower": lower,
        "x_min": x_min,
        "x_max": x_max,
        "y_min": y_min,
        "y_max": y_max,
        "x_span": x_max - x_min,
        "y_span": y_max - y_min,
    }


def _interpolate_branch(branch: Sequence[tuple[float, float]], x: float) -> float:
    _require(branch, "reference envelope branch is empty")
    if x <= branch[0][0]:
        return branch[0][1]
    if x >= branch[-1][0]:
        return branch[-1][1]
    position = bisect.bisect_right([point[0] for point in branch], x)
    left = branch[position - 1]
    right = branch[position]
    fraction = (x - left[0]) / (right[0] - left[0])
    return left[1] + (right[1] - left[1]) * fraction


def _program_frame(program: dict[str, Any]) -> dict[str, Any]:
    samples: list[list[float]] = []
    for curve_name in search.CURVE_NAMES:
        samples.extend(search._sample_curve(program["blade_surface"][curve_name], RAIL_SAMPLE_COUNT))
    longitudinal = search._dominant_longitudinal_axis(program)
    other_axes = [axis for axis in range(3) if axis != longitudinal]
    spans = [max(point[axis] for point in samples) - min(point[axis] for point in samples) for axis in range(3)]
    lateral = max(other_axes, key=lambda axis: (spans[axis], -axis))
    depth = next(axis for axis in other_axes if axis != lateral)
    longitudinal_min = min(point[longitudinal] for point in samples)
    longitudinal_max = max(point[longitudinal] for point in samples)
    lateral_min = min(point[lateral] for point in samples)
    lateral_max = max(point[lateral] for point in samples)
    _require(longitudinal_max > longitudinal_min and lateral_max > lateral_min, "parent blade frame is degenerate")
    return {
        "longitudinal_axis": longitudinal,
        "lateral_axis": lateral,
        "depth_axis": depth,
        "longitudinal_min": longitudinal_min,
        "longitudinal_max": longitudinal_max,
        "longitudinal_span": longitudinal_max - longitudinal_min,
        "lateral_min": lateral_min,
        "lateral_max": lateral_max,
        "lateral_center": (lateral_min + lateral_max) * 0.5,
        "lateral_span": lateral_max - lateral_min,
    }


def _map_reference_point(point: Sequence[float], envelope: dict[str, Any], frame: dict[str, Any]) -> list[float]:
    longitudinal_u = (float(point[0]) - envelope["x_min"]) / envelope["x_span"]
    scale = frame["longitudinal_span"] / envelope["x_span"]
    lateral_center_reference = (envelope["y_min"] + envelope["y_max"]) * 0.5
    return [
        frame["longitudinal_min"] + longitudinal_u * frame["longitudinal_span"],
        frame["lateral_center"] + (float(point[1]) - lateral_center_reference) * scale,
    ]


def _parent_curve_at(curve: dict[str, Any], u: float) -> list[float]:
    if curve["basis"] == "bezier":
        return search._bezier_point(curve["control_points"], u)
    samples = search._sample_curve(curve, RAIL_SAMPLE_COUNT)
    return samples[min(RAIL_SAMPLE_COUNT - 1, max(0, round(u * (RAIL_SAMPLE_COUNT - 1))))]


def _make_curve(parent_curve: dict[str, Any], envelope_branch: Sequence[tuple[float, float]], envelope: dict[str, Any], frame: dict[str, Any], control_count: int) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    points: list[list[float]] = []
    deltas: list[dict[str, Any]] = []
    longitudinal_axis = frame["longitudinal_axis"]
    lateral_axis = frame["lateral_axis"]
    depth_axis = frame["depth_axis"]
    for index in range(control_count):
        u = index / float(control_count - 1)
        reference_x = envelope["x_min"] + u * envelope["x_span"]
        reference_y = _interpolate_branch(envelope_branch, reference_x)
        parent_point = _parent_curve_at(parent_curve, u)
        mapped = _map_reference_point([reference_x, reference_y], envelope, frame)
        candidate_point = [float(value) for value in parent_point]
        candidate_point[longitudinal_axis] = mapped[0]
        candidate_point[lateral_axis] = mapped[1]
        candidate_point[depth_axis] = float(parent_point[depth_axis])
        candidate_point = [_round(value) for value in candidate_point]
        parent_at_u = [_round(value) for value in parent_point]
        deltas.append({
            "index": index,
            "u": _round(u),
            "is_new_control_point": index >= len(parent_curve["control_points"]),
            "reference_x": _round(reference_x),
            "reference_y": _round(reference_y),
            "before": parent_at_u,
            "after": candidate_point,
            "delta": [_round(after - before) for after, before in zip(candidate_point, parent_at_u)],
        })
        points.append(candidate_point)
    candidate_curve = {
        "curve_id": parent_curve["curve_id"],
        "basis": "nurbs-like",
        "control_points": points,
    }
    return candidate_curve, deltas


def _allocate_intermediates(semantic_us: Sequence[float], intermediate_count: int) -> list[int]:
    if intermediate_count <= 0:
        return [0] * (len(semantic_us) - 1)
    lengths = [float(right) - float(left) for left, right in zip(semantic_us, semantic_us[1:])]
    total = sum(lengths)
    raw = [intermediate_count * value / total for value in lengths]
    counts = [int(math.floor(value)) for value in raw]
    remaining = intermediate_count - sum(counts)
    order = sorted(range(len(raw)), key=lambda index: (-(raw[index] - counts[index]), index))
    for index in order[:remaining]:
        counts[index] += 1
    return counts


def _interpolate_section(left: dict[str, Any], right: dict[str, Any], u: float, ordinal: int) -> tuple[dict[str, Any], dict[str, Any]]:
    fraction = (u - float(left["u"])) / (float(right["u"]) - float(left["u"]))
    section = {
        "section_id": f"section-fit-{ordinal:02d}",
        "role": "intermediate",
        "u": _round(u),
    }
    for field in SECTION_FIELDS:
        section[field] = _round(float(left[field]) + (float(right[field]) - float(left[field])) * fraction)
    return section, {
        "section_id": section["section_id"],
        "role": "intermediate",
        "u": section["u"],
        "between": [left["section_id"], right["section_id"]],
        "interpolation_fraction": _round(fraction),
        "fields": {field: section[field] for field in SECTION_FIELDS},
    }


def _make_sections(parent: dict[str, Any], section_count: int) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    sections = parent["blade_surface"]["sections"]
    by_role = {section["role"]: section for section in sections}
    semantic = [copy.deepcopy(by_role[role]) for role in ROLES]
    intermediate_count = section_count - len(semantic)
    allocation = _allocate_intermediates([float(section["u"]) for section in semantic], intermediate_count)
    output: list[dict[str, Any]] = []
    deltas: list[dict[str, Any]] = []
    ordinal = 1
    for interval_index, (left, right) in enumerate(zip(semantic, semantic[1:])):
        output.append(copy.deepcopy(left))
        count = allocation[interval_index]
        for offset in range(1, count + 1):
            u = float(left["u"]) + (float(right["u"]) - float(left["u"])) * offset / float(count + 1)
            section, delta = _interpolate_section(left, right, u, ordinal)
            output.append(section)
            deltas.append(delta)
            ordinal += 1
    output.append(copy.deepcopy(semantic[-1]))
    _require(len(output) == section_count, "generated section count does not match the bounded request")
    return output, deltas


def _frozen_hashes(program: dict[str, Any]) -> dict[str, str]:
    by_id = {part["part_id"]: part for part in program["parts"]}
    return {part_id: search.canonical_sha256(by_id[part_id]) for part_id in ("guard", "grip", "pommel")}


def _non_blade_snapshot(program: dict[str, Any]) -> dict[str, Any]:
    return {
        "asset_id": copy.deepcopy(program["asset_id"]),
        "family": copy.deepcopy(program["family"]),
        "design_basis": copy.deepcopy(program["design_basis"]),
        "coordinate_convention": copy.deepcopy(program["coordinate_convention"]),
        "assembly": copy.deepcopy(program.get("assembly")),
        "parts": copy.deepcopy(program["parts"]),
        "material_zones": copy.deepcopy(program["material_zones"]),
        "presentation": copy.deepcopy(program["presentation"]),
        "budgets": copy.deepcopy(program["budgets"]),
        "unknowns": copy.deepcopy(program["unknowns"]),
    }


def _direct_fit_metrics(candidate: dict[str, Any], envelope: dict[str, Any], frame: dict[str, Any]) -> dict[str, Any]:
    scale = frame["longitudinal_span"] / envelope["x_span"]
    sample_errors: dict[str, list[float]] = {"spine": [], "cutting_edge": []}
    for index in range(RAIL_SAMPLE_COUNT):
        u = index / float(RAIL_SAMPLE_COUNT - 1)
        reference_x = envelope["x_min"] + u * envelope["x_span"]
        for label, branch, curve_name in (
            ("spine", envelope["upper"], "spine_curve"),
            ("cutting_edge", envelope["lower"], "cutting_edge_curve"),
        ):
            target_y = frame["lateral_center"] + (_interpolate_branch(branch, reference_x) - (envelope["y_min"] + envelope["y_max"]) * 0.5) * scale
            actual = _parent_curve_at(candidate["blade_surface"][curve_name], u)
            sample_errors[label].append(abs(float(actual[frame["lateral_axis"]]) - target_y) / frame["longitudinal_span"])
    rails = {}
    for label, errors in sample_errors.items():
        ordered = sorted(errors)
        position = 0.95 * (len(ordered) - 1)
        lower = int(math.floor(position))
        upper = min(lower + 1, len(ordered) - 1)
        fraction = position - lower
        p95 = ordered[lower] * (1.0 - fraction) + ordered[upper] * fraction
        rails[label] = {
            "mean_abs_error_normalized": _round(sum(errors) / len(errors)),
            "p95_abs_error_normalized": _round(p95),
            "max_abs_error_normalized": _round(max(errors)),
        }
    return {
        "algorithm": FIT_ALGORITHM,
        "rails": rails,
        "basis": "mapped reference upper/lower envelope versus generated control-rail interpolation; no browser render",
    }


def _metric_delta(baseline: dict[str, Any], successor: dict[str, Any]) -> dict[str, Any]:
    names = ("silhouette_iou", "boundary_f1", "symmetric_chamfer", "p95_contour_distance", "landmark_error")
    output: dict[str, Any] = {}
    for name in names:
        left = float(baseline[name])
        right = float(successor[name])
        output[name] = {
            "baseline": _round(left),
            "successor": _round(right),
            "successor_minus_baseline": _round(right - left),
        }
    return output


def _make_candidate(program: dict[str, Any], reference: dict[str, Any], control_count: int, section_count: int) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    envelope = _reference_envelopes(reference)
    frame = _program_frame(program)
    spine_curve, spine_deltas = _make_curve(program["blade_surface"]["spine_curve"], envelope["upper"], envelope, frame, control_count)
    edge_curve, edge_deltas = _make_curve(program["blade_surface"]["cutting_edge_curve"], envelope["lower"], envelope, frame, control_count)
    sections, section_deltas = _make_sections(program, section_count)
    candidate = copy.deepcopy(program)
    candidate["blade_surface"]["spine_curve"] = spine_curve
    candidate["blade_surface"]["cutting_edge_curve"] = edge_curve
    candidate["blade_surface"]["sections"] = sections
    candidate["canonical_sha256"] = ""
    candidate["canonical_sha256"] = search.canonical_sha256(candidate)
    candidate_hash = search.validate_program(candidate)
    _require(candidate_hash == candidate["canonical_sha256"], "generated candidate program hash drifted")
    geometry = search.evaluate_geometry(candidate)
    _require(geometry["hard_gate_pass"], f"generated candidate failed geometry hard gates: {geometry['hard_gates']}")
    changes = {
        "spine": {"basis_before": program["blade_surface"]["spine_curve"]["basis"], "basis_after": spine_curve["basis"], "control_points": spine_deltas},
        "cutting_edge": {"basis_before": program["blade_surface"]["cutting_edge_curve"]["basis"], "basis_after": edge_curve["basis"], "control_points": edge_deltas},
        "intermediate_sections": section_deltas,
    }
    derived = {
        "envelope": {
            "reference_x_min": _round(envelope["x_min"]),
            "reference_x_max": _round(envelope["x_max"]),
            "reference_y_min": _round(envelope["y_min"]),
            "reference_y_max": _round(envelope["y_max"]),
            "reference_x_span": _round(envelope["x_span"]),
            "reference_y_span": _round(envelope["y_span"]),
            "sample_count": len(envelope["upper"]) + len(envelope["lower"]),
        },
        "program_frame": {key: (_round(value) if isinstance(value, float) else value) for key, value in frame.items()},
        "fit": {
            "algorithm": FIT_ALGORITHM,
            "control_count_per_curve": control_count,
            "section_count": section_count,
            "basis": "nurbs-like",
            "camera_refit": False,
            "browser_calibration_refit": False,
            "hidden_depth_policy": "copy-parent-depth-axis-at-control-u@1",
        },
        "changes": changes,
        "geometry": {
            "hard_gate_pass": geometry["hard_gate_pass"],
            "hard_gates": geometry["hard_gates"],
            "metrics": geometry["metrics"],
            "coordinate_axes": geometry["coordinate_axes"],
        },
        "direct_fit": _direct_fit_metrics(candidate, envelope, frame),
    }
    return candidate, derived, geometry


def _changed_paths(candidate: dict[str, Any], changes: dict[str, Any]) -> list[str]:
    paths = [
        "blade_surface.spine_curve.basis",
        "blade_surface.cutting_edge_curve.basis",
    ]
    for curve_name, key in (("spine_curve", "spine"), ("cutting_edge_curve", "cutting_edge")):
        for item in changes[key]["control_points"]:
            for axis in range(3):
                # Report only coordinates whose value actually changed.  New
                # stations still report all three coordinates because their
                # indices did not exist in the parent program.
                if item["is_new_control_point"] or abs(float(item["delta"][axis])) > 1e-12:
                    paths.append(f"blade_surface.{curve_name}.control_points[{item['index']}][{axis}]")
    for index, section in enumerate(candidate["blade_surface"]["sections"]):
        if section["role"] != "intermediate":
            continue
        paths.append(f"blade_surface.sections[{index}].section_id")
        paths.append(f"blade_surface.sections[{index}].role")
        paths.append(f"blade_surface.sections[{index}].u")
        paths.extend(f"blade_surface.sections[{index}].{field}" for field in SECTION_FIELDS)
    return paths


def _make_receipt(program: dict[str, Any], ledger: dict[str, Any], reference: dict[str, Any], source: dict[str, Any], candidate: dict[str, Any], derived: dict[str, Any], program_sha: str, ledger_sha: str, reference_sha: str, control_count: int, section_count: int) -> dict[str, Any]:
    candidate_sha = candidate["canonical_sha256"]
    frozen = _frozen_hashes(program)
    candidate_frozen = _frozen_hashes(candidate)
    _require(frozen == candidate_frozen, "generated candidate changed frozen assembly part hashes")
    _require(_non_blade_snapshot(program) == _non_blade_snapshot(candidate), "generated candidate changed a non-blade program field")
    candidate_id = f"candidate-blade-math-{candidate_sha[:16]}"
    receipt = {
        "schema_version": "KnifeBladeMathSuccessorReceipt@1",
        "route": "weaponry-threejs-knife-studio@0.1.0",
        "generator": {
            "algorithm": FIT_ALGORITHM,
            "section_algorithm": SECTION_ALGORITHM,
            "deterministic": True,
            "control_count_per_curve": control_count,
            "section_count": section_count,
            "per_candidate_refit": False,
            "render_used": False,
        },
        "source": {
            "program_sha256": program_sha,
            "ledger_sha256": ledger_sha,
            "reference_sha256": reference_sha,
            "allowed_scope": copy.deepcopy(ledger["allowed_scope"]),
            "frozen_parts": copy.deepcopy(ledger["frozen_parts"]),
            "frozen_part_hashes": frozen,
        },
        "browser_evidence": {
            "baseline_receipt_sha256": source["baseline_receipt_sha256"],
            "successor_receipt_sha256": source["successor_receipt_sha256"],
            "calibration_sha256": source["calibration_sha256"],
            "reference_sha256": source["reference_sha256"],
            "camera_fingerprint": source["camera_fingerprint"],
            "rig_id": source["rig_id"],
            "rig_fingerprint": source["rig_fingerprint"],
            "frame_width": source["frame_width"],
            "frame_height": source["frame_height"],
            "allowed_part_ids": source["allowed_part_ids"],
            "baseline_capture_manifest_sha256": source["baseline_capture_manifest_sha256"],
            "successor_capture_manifest_sha256": source["successor_capture_manifest_sha256"],
            "baseline_program_fingerprint": source["baseline_program_fingerprint"],
            "successor_program_fingerprint": source["successor_program_fingerprint"],
            "baseline_scene_fingerprint": source["baseline_scene_fingerprint"],
            "successor_scene_fingerprint": source["successor_scene_fingerprint"],
            "baseline_metrics": source["baseline_metrics"],
            "successor_metrics": source["successor_metrics"],
            "successor_minus_baseline": _metric_delta(source["baseline_metrics"], source["successor_metrics"]),
            "refit_forbidden": True,
        },
        "parent": {
            "program_sha256": program_sha,
            "ledger_sha256": ledger_sha,
            "frozen_part_hashes": frozen,
        },
        "candidate": {
            "candidate_id": candidate_id,
            "program_sha256": candidate_sha,
            "program": copy.deepcopy(candidate),
            "changed_paths": _changed_paths(candidate, derived["changes"]),
            "parameter_changes": derived["changes"],
            "frozen_part_hashes": candidate_frozen,
            "geometry": derived["geometry"],
            "direct_fit": derived["direct_fit"],
            "expected_outcomes": {
                "browser_metrics": "NOT_MEASURED_FOR_THIS_PROPOSAL",
                "next_evidence": "capture FRONT candidate with the existing frozen browser calibration, then replay KnifeBrowserMetricReceipt@1",
                "hypothesis": "direct upper/lower reference-envelope rails and denser loft stations can improve the fixed FRONT blade silhouette while preserving the frozen assembly",
            },
        },
        "proposal_status": "SUCCESSOR_PROPOSED_NOT_APPROVED",
        "status_boundary": {
            "geometry_status": "EVALUATED_NO_RENDER",
            "render_status": "NOT_RUN",
            "visual_status": "NOT_RUN",
            "human_status": "NOT_RUN",
            "engine_status": "NOT_RUN",
            "commercial_status": "NOT_RUN",
        },
        "canonical_sha256": "",
    }
    receipt["canonical_sha256"] = search.canonical_sha256(receipt)
    _validate_receipt(receipt)
    return receipt


def _validate_receipt(receipt: dict[str, Any]) -> str:
    expected = {
        "schema_version", "route", "generator", "source", "browser_evidence", "parent", "candidate", "proposal_status", "status_boundary", "canonical_sha256",
    }
    _require(set(receipt) == expected, "KnifeBladeMathSuccessorReceipt keys are not closed")
    _require(receipt["schema_version"] == "KnifeBladeMathSuccessorReceipt@1", "successor receipt schema version drifted")
    _require(receipt["route"] == "weaponry-threejs-knife-studio@0.1.0", "successor receipt route drifted")
    _require(receipt["proposal_status"] == "SUCCESSOR_PROPOSED_NOT_APPROVED", "successor receipt crossed the proposal boundary")
    _require(receipt["generator"]["algorithm"] == FIT_ALGORITHM and receipt["generator"]["section_algorithm"] == SECTION_ALGORITHM, "successor generator algorithm drifted")
    _require(receipt["generator"]["deterministic"] is True and receipt["generator"]["per_candidate_refit"] is False and receipt["generator"]["render_used"] is False, "successor generator boundary drifted")
    control_count = receipt["generator"]["control_count_per_curve"]
    section_count = receipt["generator"]["section_count"]
    _require(isinstance(control_count, int) and not isinstance(control_count, bool), "successor control count is not an integer")
    _require(isinstance(section_count, int) and not isinstance(section_count, bool), "successor section count is not an integer")
    _require(CONTROL_COUNT_MIN <= control_count <= CONTROL_COUNT_MAX, "successor control count is outside the bounded range")
    _require(SECTION_COUNT_MIN <= section_count <= SECTION_COUNT_MAX, "successor section count is outside the bounded range")
    for field in ("source", "parent"):
        _require(isinstance(receipt[field], dict), f"successor receipt.{field} is invalid")
        _sha(receipt[field]["program_sha256"], f"successor receipt.{field}.program_sha256")
        _sha(receipt[field]["ledger_sha256"], f"successor receipt.{field}.ledger_sha256")
    _require(receipt["source"] == receipt["parent"] or receipt["source"]["program_sha256"] == receipt["parent"]["program_sha256"], "successor parent/source binding drifted")
    browser_evidence = receipt["browser_evidence"]
    for field in ("baseline_receipt_sha256", "successor_receipt_sha256", "calibration_sha256", "reference_sha256", "camera_fingerprint", "baseline_capture_manifest_sha256", "successor_capture_manifest_sha256"):
        _sha(browser_evidence[field], f"successor receipt.browser_evidence.{field}")
    _require(browser_evidence["refit_forbidden"] is True, "successor browser refit policy is not closed")
    _require(isinstance(receipt["candidate"], dict), "successor receipt.candidate is invalid")
    _require(receipt["candidate"]["candidate_id"].startswith("candidate-blade-math-"), "candidate id is not bounded")
    _sha(receipt["candidate"]["program_sha256"], "successor receipt.candidate.program_sha256")
    candidate_program = receipt["candidate"]["program"]
    _require(isinstance(candidate_program, dict), "successor receipt.candidate.program is invalid")
    _require(search.validate_program(candidate_program) == receipt["candidate"]["program_sha256"], "embedded candidate program hash does not match")
    _require(len(candidate_program["blade_surface"]["spine_curve"]["control_points"]) == control_count, "candidate spine control count does not match generator")
    _require(len(candidate_program["blade_surface"]["cutting_edge_curve"]["control_points"]) == control_count, "candidate edge control count does not match generator")
    _require(candidate_program["blade_surface"]["spine_curve"]["basis"] == "nurbs-like" and candidate_program["blade_surface"]["cutting_edge_curve"]["basis"] == "nurbs-like", "candidate rails are not envelope-fit curves")
    _require(len(candidate_program["blade_surface"]["sections"]) == section_count, "candidate section count does not match generator")
    _require(receipt["candidate"]["geometry"]["hard_gate_pass"] is True, "embedded candidate geometry is not hard-gate valid")
    _require(receipt["status_boundary"] == {
        "geometry_status": "EVALUATED_NO_RENDER",
        "render_status": "NOT_RUN",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "commercial_status": "NOT_RUN",
    }, "successor status boundary drifted")
    _sha(receipt["canonical_sha256"], "successor receipt.canonical_sha256")
    _require(receipt["canonical_sha256"] == search.canonical_sha256(receipt), "successor receipt canonical hash does not match")
    return receipt["canonical_sha256"]


def validate_receipt(receipt: dict[str, Any]) -> str:
    """Validate a generated immutable successor receipt."""

    return _validate_receipt(receipt)


def generate(program: dict[str, Any], ledger: dict[str, Any], reference: dict[str, Any], baseline: dict[str, Any], successor: dict[str, Any], control_count: int = DEFAULT_CONTROL_COUNT, section_count: int = DEFAULT_SECTION_COUNT) -> tuple[dict[str, Any], dict[str, Any]]:
    _require(CONTROL_COUNT_MIN <= control_count <= CONTROL_COUNT_MAX, f"control_count must be within [{CONTROL_COUNT_MIN},{CONTROL_COUNT_MAX}]")
    _require(SECTION_COUNT_MIN <= section_count <= SECTION_COUNT_MAX, f"section_count must be within [{SECTION_COUNT_MIN},{SECTION_COUNT_MAX}]")
    program_sha, ledger_sha, reference_sha, browser_chain = _validate_sources(program, ledger, reference, baseline, successor)
    candidate, derived, _ = _make_candidate(program, reference, control_count, section_count)
    receipt = _make_receipt(program, ledger, reference, browser_chain, candidate, derived, program_sha, ledger_sha, reference_sha, control_count, section_count)
    return candidate, receipt


def _write(path: Path, value: dict[str, Any]) -> None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    except OSError as exc:
        raise SuccessorInputError(f"cannot write {path}: {exc}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--program", type=Path, default=DEFAULT_PROGRAM)
    parser.add_argument("--ledger", type=Path, default=DEFAULT_LEDGER)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--baseline-receipt", type=Path, required=True)
    parser.add_argument("--successor-receipt", type=Path, required=True)
    parser.add_argument("--control-count", type=int, choices=range(CONTROL_COUNT_MIN, CONTROL_COUNT_MAX + 1), default=DEFAULT_CONTROL_COUNT)
    parser.add_argument("--section-count", type=int, choices=range(SECTION_COUNT_MIN, SECTION_COUNT_MAX + 1), default=DEFAULT_SECTION_COUNT)
    parser.add_argument("--program-output", type=Path, required=True)
    parser.add_argument("--receipt-output", type=Path, required=True)
    parser.add_argument("--pretty", action="store_true")
    args = parser.parse_args(argv)
    try:
        input_paths = {path.resolve() for path in (args.program, args.ledger, args.reference, args.baseline_receipt, args.successor_receipt)}
        _require(args.program_output.resolve() not in input_paths, "program output must not overwrite an input")
        _require(args.receipt_output.resolve() not in input_paths, "receipt output must not overwrite an input")
        program = _load(args.program)
        ledger = _load(args.ledger)
        reference = _load(args.reference)
        baseline = _load(args.baseline_receipt)
        successor = _load(args.successor_receipt)
        candidate, receipt = generate(program, ledger, reference, baseline, successor, args.control_count, args.section_count)
        _write(args.program_output, candidate)
        _write(args.receipt_output, receipt)
        summary = {
            "schema_version": receipt["schema_version"],
            "proposal_status": receipt["proposal_status"],
            "parent_program_sha256": receipt["parent"]["program_sha256"],
            "candidate_program_sha256": receipt["candidate"]["program_sha256"],
            "control_count_per_curve": receipt["generator"]["control_count_per_curve"],
            "section_count": receipt["generator"]["section_count"],
            "per_candidate_refit": receipt["generator"]["per_candidate_refit"],
            "geometry_hard_gate_pass": receipt["candidate"]["geometry"]["hard_gate_pass"],
            "direct_fit": receipt["candidate"]["direct_fit"],
            "canonical_sha256": receipt["canonical_sha256"],
        }
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True, indent=2 if args.pretty else None))
        return 0
    except (SuccessorInputError, search.InputError, browser.CalibrationInputError, OSError, ValueError) as exc:
        print(f"BLADE_MATH_SUCCESSOR_NOT_RUN: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
