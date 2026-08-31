#!/usr/bin/env python3
"""Build and validate the closed Dragonfang blade multi-view constraint set.

This is a Codex-side, metadata-only fixture.  It does not read an image, write
Runtime/CAS state, invoke a DCC, or claim that fixture numbers are a measured
mesh.  The output is intentionally shaped for a future dual-curve and section-
loft adapter while retaining unknown values whenever the supplied panel does
not expose a measurement.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import re
from pathlib import Path
from typing import Any


SCHEMA_VERSION = "BladeMultiViewConstraintSet@1"
VIEW_IDS = ("FRONT", "TOP", "BOTTOM", "LEFT", "RIGHT")
CURVE_IDS = ("spine", "cutting-edge")
LANDMARK_IDS = ("root", "mid", "belly", "tip")
STATION_U = {"root": 0.0, "mid": 0.36, "belly": 0.72, "tip": 1.0}
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def canonical_sha256(value: dict[str, Any]) -> str:
    draft = copy.deepcopy(value)
    draft["canonical_sha256"] = ""
    return hashlib.sha256(canonical_bytes(draft)).hexdigest()


def observation(
    status: str,
    value: Any,
    confidence: float,
    basis: str,
    source_panel: str | None,
) -> dict[str, Any]:
    if status == "unknown":
        return {
            "basis": "unknown",
            "confidence": 0.0,
            "source_panel": None,
            "status": "unknown",
            "value": None,
        }
    return {
        "basis": basis,
        "confidence": confidence,
        "source_panel": source_panel,
        "status": status,
        "value": value,
    }


def landmark_observations(
    view_id: str,
    points: dict[str, dict[str, list[float]]],
) -> dict[str, list[dict[str, Any]]]:
    panel = view_id.lower()
    curves: dict[str, list[dict[str, Any]]] = {}
    for curve_id in CURVE_IDS:
        curves[curve_id] = []
        for landmark_id in LANDMARK_IDS:
            point = points.get(curve_id, {}).get(landmark_id)
            if point is None:
                item = observation("unknown", None, 0.0, "unknown", None)
            else:
                item = observation(
                    "observed",
                    point,
                    0.9 if landmark_id in {"root", "tip"} else 0.84,
                    "panel-observation-fixture",
                    panel,
                )
            item["landmark_id"] = landmark_id
            curves[curve_id].append(item)
    return curves


def view(
    view_id: str,
    projection: str,
    role: str,
    roi: list[float],
    points: dict[str, dict[str, list[float]]],
) -> dict[str, Any]:
    panel = view_id.lower()
    x_min, y_min, x_max, y_max = roi
    unknown_fields = []
    for curve_id in CURVE_IDS:
        for landmark_id in LANDMARK_IDS:
            if landmark_id not in points.get(curve_id, {}):
                unknown_fields.append(f"curve_landmarks.{curve_id}.{landmark_id}")
    return {
        "curve_landmarks": landmark_observations(view_id, points),
        "projection": projection,
        "role": role,
        "roi": {
            "basis": "panel-bounds-fixture",
            "confidence": 0.8,
            "coordinate_space": "normalized-image",
            "origin": "top-left",
            "status": "observed",
            "x_max": x_max,
            "x_min": x_min,
            "y_max": y_max,
            "y_min": y_min,
        },
        "source_panel": panel,
        "unknown_fields": unknown_fields,
        "view_id": view_id,
    }


def build_constraint_set() -> dict[str, Any]:
    # The two in-plane sets are deliberately modest metadata-only fixtures.  A
    # real Runtime admission must replace their source binding and re-observe.
    front_points = {
        "spine": {
            "root": [0.16, 0.23],
            "mid": [0.44, 0.17],
            "belly": [0.73, 0.25],
            "tip": [0.96, 0.49],
        },
        "cutting-edge": {
            "root": [0.16, 0.68],
            "mid": [0.44, 0.82],
            "belly": [0.73, 0.88],
            "tip": [0.96, 0.49],
        },
    }
    left_points = {
        "spine": {
            "root": [0.17, 0.27],
            "mid": [0.45, 0.21],
            "belly": [0.73, 0.28],
            "tip": [0.95, 0.49],
        },
        "cutting-edge": {
            "root": [0.17, 0.66],
            "mid": [0.45, 0.79],
            "belly": [0.73, 0.85],
            "tip": [0.95, 0.49],
        },
    }
    right_points = {
        "spine": {
            "root": [0.17, 0.26],
            "mid": [0.45, 0.20],
            "belly": [0.74, 0.27],
            "tip": [0.95, 0.49],
        },
        "cutting-edge": {
            "root": [0.17, 0.67],
            "mid": [0.45, 0.80],
            "belly": [0.74, 0.86],
            "tip": [0.95, 0.49],
        },
    }

    views = [
        view("FRONT", "x-z", "primary-silhouette", [0.08, 0.08, 0.94, 0.92], front_points),
        view("TOP", "x-y", "thickness-and-tip", [0.12, 0.24, 0.88, 0.76], {}),
        view("BOTTOM", "x-y", "thickness-and-edge-continuity", [0.12, 0.24, 0.88, 0.76], {}),
        view("LEFT", "y-z", "side-thickness", [0.16, 0.12, 0.91, 0.88], left_points),
        view("RIGHT", "y-z", "side-thickness", [0.16, 0.12, 0.91, 0.88], right_points),
    ]

    stations = []
    for station_id in LANDMARK_IDS:
        stations.append(
            {
                "cross_section": {
                    "basis": "unknown",
                    "confidence": 0.0,
                    "profile": "unknown",
                    "status": "unknown",
                    "thickness_m": None,
                    "width_m": None,
                },
                "curve_landmark_ids": {
                    "cutting_edge": f"cutting-edge:{station_id}",
                    "spine": f"spine:{station_id}",
                },
                "station_id": station_id,
                "u": STATION_U[station_id],
            }
        )

    document: dict[str, Any] = {
        "canonical_sha256": "",
        "consumers": {
            "dual_curve_api": {
                "input": "curves",
                "schema_version": "BladeDualCurveConstraint@1",
            },
            "section_loft_api": {
                "input": "section_loft",
                "schema_version": "BladeSectionLoftConstraint@1",
            },
        },
        "constraint_set_id": "dragonfang-blade-multiview-fixture@1",
        "correction_scope": {
            "allowed_features": ["overall-curve", "tip", "belly"],
            "allowed_parts": ["blade-body", "cutting-edge"],
            "locked_material_zones": [
                "dark-red-blade",
                "silver-edge",
                "antique-gold-ornament",
                "black-grip",
                "ruby-accent",
            ],
            "locked_parts": ["dragon-relief", "guard-dragon-head", "grip", "pommel"],
            "policy": "single-range-correction-blade-body-and-cutting-edge-only@1",
        },
        "curves": {
            "coordinate_space": "normalized-blade-plane",
            "landmark_order": list(LANDMARK_IDS),
            "roles": {
                "cutting-edge": "lower-boundary",
                "spine": "upper-boundary",
            },
        },
        "freeze_contract": {
            "confidence": "0..1-observation-only; unknown-requires-zero",
            "curve_landmarks": list(LANDMARK_IDS),
            "handedness": "right-handed",
            "roi": "normalized-image-top-left-unit-interval",
            "section_stations": list(LANDMARK_IDS),
            "thickness_stations": "root-mid-belly-tip;absolute-values-unknown",
            "unknown": "null-value-zero-confidence-no-inference",
            "units": "meter-length-radian-angle",
        },
        "handedness": {
            "axis_convention": {
                "blade_length": "+Z",
                "blade_lateral": "+X",
                "blade_thickness": "+Y",
            },
            "reflection_policy": "do-not-mirror-or-infer-a-missing-view",
            "status": "frozen",
            "value": "right-handed",
        },
        "reference": {
            "authorization": "runtime-required",
            "excluded_panels": [],
            "panel_coverage": list(VIEW_IDS),
            "reference_id": "dragonfang-kukri-multiview-fixture@1",
            "source_kind": "metadata-only-deterministic-fixture",
            "source_sha256": None,
        },
        "schema_version": SCHEMA_VERSION,
        "section_loft": {
            "relative_thickness_order": {
                "basis": "panel-observation-summary",
                "confidence": 0.82,
                "order": ["root", "tip"],
                "relation": "root-thicker-than-tip;mid-and-belly-order-unknown",
                "status": "observed",
            },
            "station_order": list(LANDMARK_IDS),
            "stations": stations,
            "units": "meter",
        },
        "units": {
            "angle": "radian",
            "length": "meter",
            "normalized_image": "unit_interval",
            "status": "frozen",
        },
        "unknowns": [
            {
                "confidence": 0.0,
                "reason": "TOP and BOTTOM panels do not expose a reliable in-plane dual-curve measurement in this fixture.",
                "scope": "views.TOP|views.BOTTOM.curve_landmarks",
                "status": "unknown",
                "unknown_id": "top-bottom-in-plane-curve",
            },
            {
                "confidence": 0.0,
                "reason": "The source summary establishes relative taper but not physical section dimensions or profile topology.",
                "scope": "section_loft.stations[*].cross_section",
                "status": "unknown",
                "unknown_id": "physical-section-width-thickness-profile",
            },
            {
                "confidence": 0.0,
                "reason": "Hidden blade interior, winding, and back-face continuity are not admitted by this five-view set.",
                "scope": "hidden-geometry-and-topology",
                "status": "unknown",
                "unknown_id": "hidden-interior-continuity",
            },
        ],
        "views": views,
    }
    document["canonical_sha256"] = canonical_sha256(document)
    return document


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    _require(set(value) == expected, f"{label} keys are not closed")


def _finite_unit(value: Any, label: str) -> None:
    _require(isinstance(value, (int, float)) and not isinstance(value, bool), f"{label} is not numeric")
    _require(math.isfinite(float(value)) and 0.0 <= float(value) <= 1.0, f"{label} is outside [0,1]")


def validate_constraint_set(document: dict[str, Any]) -> None:
    expected_top = {
        "canonical_sha256",
        "consumers",
        "constraint_set_id",
        "correction_scope",
        "curves",
        "freeze_contract",
        "handedness",
        "reference",
        "schema_version",
        "section_loft",
        "units",
        "unknowns",
        "views",
    }
    _require(isinstance(document, dict), "constraint set must be an object")
    _exact_keys(document, expected_top, "constraint set")
    _require(document["schema_version"] == SCHEMA_VERSION, "schema version drifted")
    digest = document["canonical_sha256"]
    _require(isinstance(digest, str) and SHA256_RE.fullmatch(digest) is not None, "canonical hash is invalid")
    _require(digest == canonical_sha256(document), "canonical hash does not match canonical JSON")

    _exact_keys(document["handedness"], {"axis_convention", "reflection_policy", "status", "value"}, "handedness")
    _require(document["handedness"]["status"] == "frozen", "handedness must be frozen")
    _require(document["handedness"]["value"] == "right-handed", "handedness is not closed")
    _exact_keys(document["handedness"]["axis_convention"], {"blade_length", "blade_lateral", "blade_thickness"}, "axis convention")
    _require(document["handedness"]["axis_convention"] == {"blade_length": "+Z", "blade_lateral": "+X", "blade_thickness": "+Y"}, "axis convention drifted")
    _require(document["handedness"]["reflection_policy"] == "do-not-mirror-or-infer-a-missing-view", "reflection policy drifted")

    _exact_keys(document["freeze_contract"], {"confidence", "curve_landmarks", "handedness", "roi", "section_stations", "thickness_stations", "unknown", "units"}, "freeze contract")
    _require(document["freeze_contract"] == {
        "confidence": "0..1-observation-only; unknown-requires-zero",
        "curve_landmarks": list(LANDMARK_IDS),
        "handedness": "right-handed",
        "roi": "normalized-image-top-left-unit-interval",
        "section_stations": list(LANDMARK_IDS),
        "thickness_stations": "root-mid-belly-tip;absolute-values-unknown",
        "unknown": "null-value-zero-confidence-no-inference",
        "units": "meter-length-radian-angle",
    }, "freeze contract drifted")

    _exact_keys(document["units"], {"angle", "length", "normalized_image", "status"}, "units")
    _require(document["units"] == {"angle": "radian", "length": "meter", "normalized_image": "unit_interval", "status": "frozen"}, "units are not frozen")

    _exact_keys(document["correction_scope"], {"allowed_features", "allowed_parts", "locked_material_zones", "locked_parts", "policy"}, "correction scope")
    _require(document["correction_scope"]["allowed_parts"] == ["blade-body", "cutting-edge"], "allowed correction parts drifted")
    _require(document["correction_scope"]["allowed_features"] == ["overall-curve", "tip", "belly"], "allowed correction features drifted")
    _require(document["correction_scope"]["locked_parts"] == ["dragon-relief", "guard-dragon-head", "grip", "pommel"], "locked parts drifted")
    _require(document["correction_scope"]["locked_material_zones"] == ["dark-red-blade", "silver-edge", "antique-gold-ornament", "black-grip", "ruby-accent"], "locked materials drifted")

    _exact_keys(document["reference"], {"authorization", "excluded_panels", "panel_coverage", "reference_id", "source_kind", "source_sha256"}, "reference")
    _require(document["reference"]["panel_coverage"] == list(VIEW_IDS), "reference coverage is not the closed five-view set")
    _require(document["reference"]["source_sha256"] is None, "fixture must not invent a source hash")

    _exact_keys(document["curves"], {"coordinate_space", "landmark_order", "roles"}, "curves")
    _require(document["curves"]["landmark_order"] == list(LANDMARK_IDS), "curve landmark order drifted")
    _require(document["curves"]["roles"] == {"cutting-edge": "lower-boundary", "spine": "upper-boundary"}, "curve roles drifted")

    _require(isinstance(document["views"], list) and [v.get("view_id") for v in document["views"]] == list(VIEW_IDS), "views are not closed or ordered")
    for item in document["views"]:
        _exact_keys(item, {"curve_landmarks", "projection", "role", "roi", "source_panel", "unknown_fields", "view_id"}, f"view {item.get('view_id')}")
        view_id = item["view_id"]
        _require(item["source_panel"] == view_id.lower(), f"view {view_id} source panel drifted")
        _exact_keys(item["roi"], {"basis", "confidence", "coordinate_space", "origin", "status", "x_max", "x_min", "y_max", "y_min"}, f"view {view_id} ROI")
        _require(item["roi"]["coordinate_space"] == "normalized-image" and item["roi"]["origin"] == "top-left", f"view {view_id} ROI frame drifted")
        for bound in ("x_min", "y_min", "x_max", "y_max", "confidence"):
            _finite_unit(item["roi"][bound], f"view {view_id} ROI {bound}")
        _require(item["roi"]["x_min"] < item["roi"]["x_max"] and item["roi"]["y_min"] < item["roi"]["y_max"], f"view {view_id} ROI is empty")
        _exact_keys(item["curve_landmarks"], set(CURVE_IDS), f"view {view_id} curves")
        for curve_id in CURVE_IDS:
            landmarks = item["curve_landmarks"][curve_id]
            _require(isinstance(landmarks, list) and [p.get("landmark_id") for p in landmarks] == list(LANDMARK_IDS), f"view {view_id} {curve_id} landmarks are not closed")
            for point in landmarks:
                _exact_keys(point, {"basis", "confidence", "landmark_id", "source_panel", "status", "value"}, f"view {view_id} {curve_id} point")
                _finite_unit(point["confidence"], f"view {view_id} {curve_id} confidence")
                if point["status"] == "unknown":
                    _require(point["value"] is None and point["confidence"] == 0.0 and point["basis"] == "unknown" and point["source_panel"] is None, "unknown point contains an invented observation")
                else:
                    _require(point["status"] == "observed", "unknown point status vocabulary drifted")
                    _require(point["basis"] == "panel-observation-fixture", "fixture observation basis is not closed")
                    _require(isinstance(point["value"], list) and len(point["value"]) == 2, "observed point must be a normalized pair")
                    _finite_unit(point["value"][0], "observed landmark x")
                    _finite_unit(point["value"][1], "observed landmark y")
                    _require(point["confidence"] > 0.0 and point["source_panel"] == view_id.lower(), "observed point binding is invalid")
        expected_unknown_fields = [
            f"curve_landmarks.{curve_id}.{landmark_id}"
            for curve_id in CURVE_IDS
            for landmark_id in LANDMARK_IDS
            if next(point for point in item["curve_landmarks"][curve_id] if point["landmark_id"] == landmark_id)["status"] == "unknown"
        ]
        _require(item["unknown_fields"] == expected_unknown_fields, f"view {view_id} unknown fields are not bound to unknown points")

    _exact_keys(document["section_loft"], {"relative_thickness_order", "station_order", "stations", "units"}, "section loft")
    _require(document["section_loft"]["station_order"] == list(LANDMARK_IDS), "section station order drifted")
    _require(document["section_loft"]["units"] == "meter", "section loft units drifted")
    relation = document["section_loft"]["relative_thickness_order"]
    _exact_keys(relation, {"basis", "confidence", "order", "relation", "status"}, "thickness relation")
    _require(relation["status"] == "observed" and relation["order"] == ["root", "tip"], "thickness station relation is not frozen")
    _require(relation["relation"] == "root-thicker-than-tip;mid-and-belly-order-unknown", "thickness relation overclaims an unobserved order")
    _require(isinstance(document["section_loft"]["stations"], list) and len(document["section_loft"]["stations"]) == 4, "section station count drifted")
    for station in document["section_loft"]["stations"]:
        _exact_keys(station, {"cross_section", "curve_landmark_ids", "station_id", "u"}, "section station")
        station_id = station["station_id"]
        _require(station_id in LANDMARK_IDS and station["u"] == STATION_U[station_id], f"station {station_id} u is not frozen")
        _exact_keys(station["curve_landmark_ids"], {"cutting_edge", "spine"}, f"station {station_id} landmark binding")
        _exact_keys(station["cross_section"], {"basis", "confidence", "profile", "status", "thickness_m", "width_m"}, f"station {station_id} section")
        section = station["cross_section"]
        _require(section == {"basis": "unknown", "confidence": 0.0, "profile": "unknown", "status": "unknown", "thickness_m": None, "width_m": None}, f"station {station_id} invents section dimensions")

    _exact_keys(document["consumers"], {"dual_curve_api", "section_loft_api"}, "consumer bindings")
    _require(document["consumers"]["dual_curve_api"] == {"input": "curves", "schema_version": "BladeDualCurveConstraint@1"}, "dual-curve consumer binding drifted")
    _require(document["consumers"]["section_loft_api"] == {"input": "section_loft", "schema_version": "BladeSectionLoftConstraint@1"}, "section-loft consumer binding drifted")

    _require(isinstance(document["unknowns"], list), "unknown ledger is invalid")
    for unknown in document["unknowns"]:
        _exact_keys(unknown, {"confidence", "reason", "scope", "status", "unknown_id"}, "unknown ledger entry")
        _require(unknown["status"] == "unknown" and unknown["confidence"] == 0.0, "unknown ledger entry is not explicit unknown")


def write_fixture(path: Path) -> None:
    document = build_constraint_set()
    validate_constraint_set(document)
    path.write_bytes(canonical_bytes(document) + b"\n")


def load_and_validate(path: Path) -> dict[str, Any]:
    document = json.loads(path.read_text(encoding="utf-8"))
    validate_constraint_set(document)
    return document


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", type=Path, help="validate an existing canonical JSON fixture")
    parser.add_argument("--output", type=Path, help="write the deterministic fixture to this path")
    args = parser.parse_args()
    if args.output is not None:
        write_fixture(args.output)
        print(f"Blade multi-view constraint fixture PASS: {args.output}")
    if args.check is not None:
        load_and_validate(args.check)
        print(f"Blade multi-view constraint self-check PASS: {args.check}")
    if args.output is None and args.check is None:
        validate_constraint_set(build_constraint_set())
        print("Blade multi-view constraint self-check PASS: deterministic fixture")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
