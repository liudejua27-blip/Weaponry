#!/usr/bin/env python3
"""Deterministic, no-render candidate search for the Three.js knife route.

This is deliberately a small Codex-side evaluator.  It reads closed
``KnifeSceneProgram@1`` and ``KnifeObjectiveLedger@1`` documents, explores a
bounded blade or explicitly-ledgered assembly parameter space, and emits
reviewable successor proposals.
It never writes the input documents, calls a renderer, or promotes a
candidate to visual or commercial acceptance.

The search is useful for the first Dragonfang slice because the fixture has
two independent blade curves, four semantic sections, and closed classic or
semantic assembly branches.  Only numeric paths belonging to the ledger's
allowed scope are mutated; IDs, categorical fields, frozen parts, and all
unknown fields remain unchanged.  This is not a replacement for fixed-view
comparison or the Rust Runtime writer.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import random
import re
import sys
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PROGRAM = ROOT / "references" / "dragonfang-first-slice.json"
PROGRAM_SCHEMA = ROOT / "references" / "knife-scene-program.schema.json"
LEDGER_SCHEMA = ROOT / "references" / "knife-objective-ledger.schema.json"

ROLES = ("root", "shoulder", "belly", "tip")
CURVE_NAMES = ("spine_curve", "cutting_edge_curve")
SECTION_FIELDS = ("half_width", "thickness", "edge_offset", "spine_offset", "asymmetry", "twist")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
ID_RE = re.compile(r"^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$")

PROGRAM_KEYS = {
    "schema_version",
    "asset_id",
    "family",
    "design_basis",
    "coordinate_convention",
    "blade_surface",
    "assembly",
    "parts",
    "material_zones",
    "presentation",
    "budgets",
    "unknowns",
    "canonical_sha256",
}
LEDGER_KEYS = {
    "schema_version",
    "ledger_id",
    "revision",
    "parent_ledger_sha256",
    "program_sha256",
    "baseline_candidate_sha256",
    "stage",
    "allowed_scope",
    "frozen_parts",
    "hypothesis",
    "objective_metrics",
    "regression_limits",
    "candidate_budget",
    "minimum_improvement",
    "plateau_limit",
    "evidence_sha256",
    "status",
    "canonical_sha256",
}

PROGRAM_FAMILIES = {"kukri", "tanto", "karambit", "bayonet", "machete", "original-knife"}
PROGRAM_BASES = {"authorized-reference-inspired", "original-design", "img2threejs-compatible-import"}
PART_ROLES = {"blade-body", "cutting-edge", "guard", "grip", "pommel", "fastener", "gem", "relief", "helper"}
SOURCE_CLASSES = {"observed", "inferred", "design-prior", "original-choice"}
SURFACE_ROLES = {"blade-body", "cutting-edge", "spine", "root-transition", "ricasso", "fuller"}
SECTION_ROLES = {"root", "shoulder", "belly", "tip", "intermediate"}
OBJECTIVE_METRICS = {
    "silhouette-iou",
    "boundary-f1",
    "symmetric-chamfer",
    "p95-contour-distance",
    "tip-landmark-error",
    "belly-depth-error",
    "thickness-continuity",
    "normal-continuity",
    "part-id-coverage",
    "material-id-coverage",
    "negative-space-error",
    "negative-space-proxy",
    "hook-continuity",
    "hook-continuity-error",
    "relief-coverage",
    "relief-coverage-error",
    "relief-spacing",
    "relief-spacing-error",
    "fps-occupancy",
}
STAGES = {"blockout", "structural", "form", "material", "surface", "lighting", "interaction", "optimization"}
LEDGER_STATUSES = {"active", "accepted", "rejected", "plateau", "blocked", "budget-exhausted"}

# The fixture's source coordinate convention is intentionally left untouched.
# Search only edits the two axes that are not the dominant longitudinal axis;
# this preserves section order and makes the mutation scope unambiguous.
DEFAULT_SEED = 20260831
DEFAULT_CANDIDATE_COUNT = 8
MAX_CANDIDATES = 32
EPSILON = 1e-6
MIN_DIMENSION = 1e-4


class InputError(ValueError):
    """A malformed or incompatible closed input document."""


def canonical_bytes(value: Any) -> bytes:
    """Return the repository's canonical JSON byte representation."""

    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def canonical_sha256(value: Any) -> str:
    """Hash a value, blanking its own canonical field when present."""

    draft = copy.deepcopy(value)
    if isinstance(draft, dict) and "canonical_sha256" in draft:
        draft["canonical_sha256"] = ""
    return hashlib.sha256(canonical_bytes(draft)).hexdigest()


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise InputError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicate_keys)
    except OSError as exc:
        raise InputError(f"cannot read {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise InputError(f"invalid JSON in {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise InputError(f"{path} must contain a JSON object")
    return value


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise InputError(message)


def _exact_keys(value: Any, expected: set[str], label: str) -> None:
    _require(isinstance(value, dict), f"{label} must be an object")
    _require(set(value) == expected, f"{label} keys are not closed")


def _number(value: Any, label: str, minimum: float | None = None, maximum: float | None = None, exclusive_minimum: bool = False) -> None:
    _require(isinstance(value, (int, float)) and not isinstance(value, bool), f"{label} must be numeric")
    _require(math.isfinite(float(value)), f"{label} must be finite")
    if minimum is not None:
        if exclusive_minimum:
            _require(float(value) > minimum, f"{label} must be greater than {minimum}")
        else:
            _require(float(value) >= minimum, f"{label} must be at least {minimum}")
    if maximum is not None:
        _require(float(value) <= maximum, f"{label} must be at most {maximum}")


def _string(value: Any, label: str, minimum: int = 1, maximum: int | None = None) -> None:
    _require(isinstance(value, str), f"{label} must be a string")
    _require(len(value) >= minimum, f"{label} is too short")
    if maximum is not None:
        _require(len(value) <= maximum, f"{label} is too long")


def _id(value: Any, label: str) -> None:
    _string(value, label)
    _require(ID_RE.fullmatch(value) is not None, f"{label} is not a closed ID")


def _sha(value: Any, label: str, allow_empty: bool = False) -> None:
    _require(isinstance(value, str), f"{label} must be a SHA-256 string")
    if allow_empty and value == "":
        return
    _require(SHA256_RE.fullmatch(value) is not None, f"{label} is not a lowercase SHA-256")


def _unique(values: Iterable[Any], label: str) -> None:
    values_list = list(values)
    _require(len(values_list) == len(set(values_list)), f"{label} contains duplicates")


def _validate_point(point: Any, label: str) -> None:
    _require(isinstance(point, list) and len(point) == 3, f"{label} must be a 3D point")
    for index, value in enumerate(point):
        _number(value, f"{label}[{index}]", -4.0, 4.0)


def _validate_curve(curve: Any, label: str) -> None:
    _exact_keys(curve, {"curve_id", "basis", "control_points"}, label)
    _id(curve["curve_id"], f"{label}.curve_id")
    _require(curve["basis"] in {"bezier", "nurbs-like"}, f"{label}.basis is unsupported")
    points = curve["control_points"]
    _require(isinstance(points, list) and 4 <= len(points) <= 64, f"{label}.control_points count is outside [4,64]")
    for index, point in enumerate(points):
        _validate_point(point, f"{label}.control_points[{index}]")


def _validate_section(section: Any, label: str) -> None:
    expected = {"section_id", "role", "u", "half_width", "thickness", "edge_offset", "spine_offset", "asymmetry", "twist"}
    _exact_keys(section, expected, label)
    _id(section["section_id"], f"{label}.section_id")
    _require(section["role"] in SECTION_ROLES, f"{label}.role is unsupported")
    _number(section["u"], f"{label}.u", 0.0, 1.0)
    _number(section["half_width"], f"{label}.half_width", 0.0, 2.0, exclusive_minimum=True)
    _number(section["thickness"], f"{label}.thickness", 0.0, 1.0, exclusive_minimum=True)
    _number(section["edge_offset"], f"{label}.edge_offset", -1.0, 1.0)
    _number(section["spine_offset"], f"{label}.spine_offset", -1.0, 1.0)
    _number(section["asymmetry"], f"{label}.asymmetry", -1.0, 1.0)
    _number(section["twist"], f"{label}.twist", -1.5708, 1.5708)


def _positive_dimension(value: Any, label: str, maximum: float) -> None:
    _number(value, label, MIN_DIMENSION, maximum, exclusive_minimum=True)


def _validate_assembly_base(spec: Any, primitive: str, label: str) -> None:
    _require(isinstance(spec, dict), f"{label} must be an object")
    _require(spec.get("primitive") == primitive, f"{label}.primitive drifted")
    _id(spec.get("part_id"), f"{label}.part_id")
    _validate_point(spec.get("center"), f"{label}.center")


def _validate_classic_assembly_spec(spec: Any, primitive: str, label: str, fields: set[str]) -> None:
    common = {"primitive", "part_id", "center"}
    expected = common | fields
    if isinstance(spec, dict) and "style" in spec:
        expected.add("style")
    _exact_keys(spec, expected, label)
    _validate_assembly_base(spec, primitive, label)
    if "style" in spec:
        _require(spec["style"] == "classic", f"{label}.style must be classic")
    for field, maximum in {
        "span": 2.0,
        "thickness": 1.0,
        "depth": 1.0,
        "length": 2.0 if primitive == "grip" else 1.0,
        "radius": 1.0,
    }.items():
        if field in fields:
            _positive_dimension(spec[field], f"{label}.{field}", maximum)
    if primitive == "grip":
        _number(spec["taper"], f"{label}.taper", -0.9, 0.9)
        _require(isinstance(spec["facets"], int) and not isinstance(spec["facets"], bool) and 6 <= spec["facets"] <= 32, f"{label}.facets is invalid")


def _validate_dragon_jaw(jaw: Any, label: str, guard_span: float) -> None:
    expected = {"span", "thickness", "depth", "offset_y", "offset_z", "curvature"}
    _exact_keys(jaw, expected, label)
    _positive_dimension(jaw["span"], f"{label}.span", guard_span)
    _positive_dimension(jaw["thickness"], f"{label}.thickness", 0.4)
    _positive_dimension(jaw["depth"], f"{label}.depth", 0.4)
    _number(jaw["offset_y"], f"{label}.offset_y", -guard_span * 0.5, guard_span * 0.5)
    _number(jaw["offset_z"], f"{label}.offset_z", -0.8, 0.8)
    _number(jaw["curvature"], f"{label}.curvature", -0.25, 0.25)


def _validate_feature_id(value: Any, label: str, seen: list[str]) -> None:
    _id(value, label)
    _require(value not in seen, f"{label} must be unique")
    seen.append(value)


def _validate_dragon_guard(spec: Any, label: str) -> None:
    common = {"primitive", "part_id", "center"}
    expected = common | {"span", "thickness", "depth", "style", "jaw_gap", "upper_jaw", "lower_jaw", "horns", "eye_sockets"}
    _exact_keys(spec, expected, label)
    _validate_assembly_base(spec, "guard", label)
    _require(spec["style"] == "dragon-guard", f"{label}.style must be dragon-guard")
    _positive_dimension(spec["span"], f"{label}.span", 2.0)
    _positive_dimension(spec["thickness"], f"{label}.thickness", 1.0)
    _positive_dimension(spec["depth"], f"{label}.depth", 1.0)
    _number(spec["jaw_gap"], f"{label}.jaw_gap", 0.01, 0.6)
    _validate_dragon_jaw(spec["upper_jaw"], f"{label}.upper_jaw", float(spec["span"]))
    _validate_dragon_jaw(spec["lower_jaw"], f"{label}.lower_jaw", float(spec["span"]))
    upper_radius = min(float(spec["upper_jaw"]["thickness"]), float(spec["upper_jaw"]["depth"])) * 0.5
    lower_radius = min(float(spec["lower_jaw"]["thickness"]), float(spec["lower_jaw"]["depth"])) * 0.5
    negative_curvature = max(0.0, -float(spec["upper_jaw"]["curvature"]), -float(spec["lower_jaw"]["curvature"]))
    _require(float(spec["jaw_gap"]) - negative_curvature * 2.0 > upper_radius + lower_radius, f"{label}.jaw_gap must leave positive space between jaw rails")

    horns = spec["horns"]
    _require(isinstance(horns, list) and 2 <= len(horns) <= 4, f"{label}.horns must contain 2 to 4 entries")
    eyes = spec["eye_sockets"]
    _require(isinstance(eyes, list) and 1 <= len(eyes) <= 2, f"{label}.eye_sockets must contain 1 to 2 entries")
    feature_ids: list[str] = []
    horn_sides: set[int] = set()
    for index, horn in enumerate(horns):
        horn_label = f"{label}.horns[{index}]"
        _exact_keys(horn, {"feature_id", "side", "length", "radius", "sweep", "offset_z"}, horn_label)
        _validate_feature_id(horn["feature_id"], f"{horn_label}.feature_id", feature_ids)
        _require(horn["side"] in (-1, 1), f"{horn_label}.side is unsupported")
        horn_sides.add(horn["side"])
        _positive_dimension(horn["length"], f"{horn_label}.length", 0.8)
        _positive_dimension(horn["radius"], f"{horn_label}.radius", 0.2)
        _number(horn["sweep"], f"{horn_label}.sweep", -0.75, 0.75)
        _number(horn["offset_z"], f"{horn_label}.offset_z", -0.8, 0.8)
    _require(horn_sides == {-1, 1}, f"{label}.horns must include both sides")
    for index, eye in enumerate(eyes):
        eye_label = f"{label}.eye_sockets[{index}]"
        _exact_keys(eye, {"feature_id", "side", "radius", "depth", "offset_y", "offset_z"}, eye_label)
        _validate_feature_id(eye["feature_id"], f"{eye_label}.feature_id", feature_ids)
        _require(eye["side"] in (-1, 1), f"{eye_label}.side is unsupported")
        _positive_dimension(eye["radius"], f"{eye_label}.radius", 0.25)
        _positive_dimension(eye["depth"], f"{eye_label}.depth", 0.2)
        _number(eye["offset_y"], f"{eye_label}.offset_y", -float(spec["span"]) * 0.5, float(spec["span"]) * 0.5)
        _number(eye["offset_z"], f"{eye_label}.offset_z", -0.8, 0.8)


def _validate_segmented_grip(spec: Any, label: str) -> None:
    common = {"primitive", "part_id", "center"}
    expected = common | {"length", "radius", "taper", "facets", "style", "centerline", "segments", "metal_frames", "fasteners"}
    _exact_keys(spec, expected, label)
    _validate_assembly_base(spec, "grip", label)
    _require(spec["style"] == "segmented-grip", f"{label}.style must be segmented-grip")
    _positive_dimension(spec["length"], f"{label}.length", 2.0)
    _positive_dimension(spec["radius"], f"{label}.radius", 1.0)
    _number(spec["taper"], f"{label}.taper", -0.9, 0.9)
    _require(isinstance(spec["facets"], int) and not isinstance(spec["facets"], bool) and 6 <= spec["facets"] <= 32, f"{label}.facets is invalid")

    centerline = spec["centerline"]
    _require(isinstance(centerline, list) and 3 <= len(centerline) <= 8, f"{label}.centerline must contain 3 to 8 points")
    previous_x = float("-inf")
    for index, point in enumerate(centerline):
        _validate_point(point, f"{label}.centerline[{index}]")
        _require(point[0] > previous_x, f"{label}.centerline x values must be strictly increasing")
        previous_x = float(point[0])

    segments = spec["segments"]
    _require(isinstance(segments, list) and 2 <= len(segments) <= 8, f"{label}.segments must contain 2 to 8 entries")
    feature_ids: list[str] = []
    previous_end = 0.0
    for index, segment in enumerate(segments):
        segment_label = f"{label}.segments[{index}]"
        _exact_keys(segment, {"feature_id", "start_u", "end_u", "radius_scale"}, segment_label)
        _validate_feature_id(segment["feature_id"], f"{segment_label}.feature_id", feature_ids)
        _number(segment["start_u"], f"{segment_label}.start_u", 0.0, 1.0)
        _number(segment["end_u"], f"{segment_label}.end_u", 0.0, 1.0)
        _require(abs(float(segment["start_u"]) - previous_end) <= EPSILON and float(segment["end_u"]) > float(segment["start_u"]), f"{label}.segments must be contiguous and strictly increasing")
        _number(segment["radius_scale"], f"{segment_label}.radius_scale", 0.5, 1.5)
        previous_end = float(segment["end_u"])
    _require(abs(previous_end - 1.0) <= EPSILON, f"{label}.segments must cover [0, 1]")

    frames = spec["metal_frames"]
    _require(isinstance(frames, list) and 1 <= len(frames) <= 8, f"{label}.metal_frames must contain 1 to 8 entries")
    for index, frame in enumerate(frames):
        frame_label = f"{label}.metal_frames[{index}]"
        _exact_keys(frame, {"feature_id", "at", "width", "thickness"}, frame_label)
        _validate_feature_id(frame["feature_id"], f"{frame_label}.feature_id", feature_ids)
        _number(frame["at"], f"{frame_label}.at", 0.0, 1.0)
        _positive_dimension(frame["width"], f"{frame_label}.width", 0.5)
        _positive_dimension(frame["thickness"], f"{frame_label}.thickness", 0.15)
        _require(float(frame["width"]) >= float(frame["thickness"]) * 2.0, f"{frame_label}.width must be at least twice its thickness")

    fasteners = spec["fasteners"]
    _require(isinstance(fasteners, list) and 3 <= len(fasteners) <= 5, f"{label}.fasteners must contain 3 to 5 entries")
    for index, fastener in enumerate(fasteners):
        fastener_label = f"{label}.fasteners[{index}]"
        _exact_keys(fastener, {"feature_id", "at", "side", "radius", "depth"}, fastener_label)
        _validate_feature_id(fastener["feature_id"], f"{fastener_label}.feature_id", feature_ids)
        _number(fastener["at"], f"{fastener_label}.at", 0.0, 1.0)
        _require(fastener["side"] in (-1, 1), f"{fastener_label}.side is unsupported")
        _positive_dimension(fastener["radius"], f"{fastener_label}.radius", 0.1)
        _positive_dimension(fastener["depth"], f"{fastener_label}.depth", 0.25)


def _validate_hooked_pommel(spec: Any, label: str) -> None:
    common = {"primitive", "part_id", "center"}
    expected = common | {"length", "radius", "depth", "style", "hook", "gem_seat"}
    _exact_keys(spec, expected, label)
    _validate_assembly_base(spec, "pommel", label)
    _require(spec["style"] == "hooked-pommel", f"{label}.style must be hooked-pommel")
    _positive_dimension(spec["length"], f"{label}.length", 1.0)
    _positive_dimension(spec["radius"], f"{label}.radius", 1.0)
    _positive_dimension(spec["depth"], f"{label}.depth", 1.0)

    hook = spec["hook"]
    _exact_keys(hook, {"length", "radius", "bend", "direction"}, f"{label}.hook")
    _positive_dimension(hook["length"], f"{label}.hook.length", 0.8)
    _positive_dimension(hook["radius"], f"{label}.hook.radius", 0.2)
    _number(hook["bend"], f"{label}.hook.bend", 0.2, 1.0)
    _require(hook["direction"] in (-1, 1), f"{label}.hook.direction is unsupported")

    seat = spec["gem_seat"]
    _exact_keys(seat, {"feature_id", "radius", "depth", "offset_x", "offset_y", "offset_z", "axis"}, f"{label}.gem_seat")
    feature_ids: list[str] = []
    _validate_feature_id(seat["feature_id"], f"{label}.gem_seat.feature_id", feature_ids)
    _positive_dimension(seat["radius"], f"{label}.gem_seat.radius", 0.25)
    _positive_dimension(seat["depth"], f"{label}.gem_seat.depth", 0.2)
    for field in ("offset_x", "offset_y", "offset_z"):
        _number(seat[field], f"{label}.gem_seat.{field}", -0.8, 0.8)
    _require(seat["axis"] in ("x", "y", "z"), f"{label}.gem_seat.axis is unsupported")


def _validate_assembly_spec(spec: Any, primitive: str, label: str) -> None:
    """Validate one closed classic or semantic assembly union branch."""

    if primitive == "guard":
        style = spec.get("style") if isinstance(spec, dict) else None
        if style == "dragon-guard":
            _validate_dragon_guard(spec, label)
        else:
            _require(style in (None, "classic"), f"{label}.style must be classic or dragon-guard")
            _validate_classic_assembly_spec(spec, primitive, label, {"span", "thickness", "depth"})
        return
    if primitive == "grip":
        style = spec.get("style") if isinstance(spec, dict) else None
        if style == "segmented-grip":
            _validate_segmented_grip(spec, label)
        else:
            _require(style in (None, "classic"), f"{label}.style must be classic or segmented-grip")
            _validate_classic_assembly_spec(spec, primitive, label, {"length", "radius", "taper", "facets"})
        return
    if primitive == "pommel":
        style = spec.get("style") if isinstance(spec, dict) else None
        if style == "hooked-pommel":
            _validate_hooked_pommel(spec, label)
        else:
            _require(style in (None, "classic"), f"{label}.style must be classic or hooked-pommel")
            _validate_classic_assembly_spec(spec, primitive, label, {"length", "radius", "depth"})
        return

    common = {"primitive", "part_id", "center"}
    fields = {
        "fastener": {"radius", "depth", "axis"},
        "gem": {"radius", "depth", "axis"},
        "relief": {"width", "height", "depth", "shape", "axis"},
    }[primitive]
    _exact_keys(spec, common | fields, label)
    _validate_assembly_base(spec, primitive, label)
    if primitive in {"fastener", "gem"}:
        _positive_dimension(spec["radius"], f"{label}.radius", 0.5)
        _positive_dimension(spec["depth"], f"{label}.depth", 1.0)
        _require(spec["axis"] in ("x", "y", "z"), f"{label}.axis is unsupported")
    else:
        _positive_dimension(spec["width"], f"{label}.width", 1.0)
        _positive_dimension(spec["height"], f"{label}.height", 1.0)
        _positive_dimension(spec["depth"], f"{label}.depth", 0.5)
        _require(spec["shape"] in ("panel", "diamond"), f"{label}.shape is unsupported")
        _require(spec["axis"] in ("x", "y", "z"), f"{label}.axis is unsupported")


def _validate_assembly(assembly: Any, parts: list[dict[str, Any]]) -> None:
    allowed = {"guard", "grip", "pommel", "fasteners", "gems", "reliefs"}
    _require(isinstance(assembly, dict) and 1 <= len(assembly) <= len(allowed), "program.assembly must be a non-empty object")
    _require(set(assembly).issubset(allowed), "program.assembly keys are not closed")
    specs: list[dict[str, Any]] = []
    for primitive in ("guard", "grip", "pommel"):
        if primitive in assembly:
            _validate_assembly_spec(assembly[primitive], primitive, f"program.assembly.{primitive}")
            specs.append(assembly[primitive])
    for label, primitive in (("fasteners", "fastener"), ("gems", "gem"), ("reliefs", "relief")):
        if label not in assembly:
            continue
        values = assembly[label]
        _require(isinstance(values, list) and len(values) <= 32, f"program.assembly.{label} is invalid")
        for index, spec in enumerate(values):
            _validate_assembly_spec(spec, primitive, f"program.assembly.{label}[{index}]")
            specs.append(spec)
    _unique((spec["part_id"] for spec in specs), "program.assembly.part_id")
    parts_by_id = {part["part_id"]: part for part in parts}
    for spec in specs:
        _require(spec["part_id"] in parts_by_id, f"assembly references missing part {spec['part_id']}")
        _require(parts_by_id[spec["part_id"]]["role"] == spec["primitive"], f"assembly role mismatch for {spec['part_id']}")


def _validate_program(program: dict[str, Any]) -> str:
    _require(isinstance(program, dict), "KnifeSceneProgram must be an object")
    _require(set(program) in (PROGRAM_KEYS, PROGRAM_KEYS - {"assembly"}), "KnifeSceneProgram keys are not closed")
    _require(program["schema_version"] == "KnifeSceneProgram@1", "program schema version drifted")
    _id(program["asset_id"], "program.asset_id")
    _require(program["family"] in PROGRAM_FAMILIES, "program.family is unsupported")
    _require(program["design_basis"] in PROGRAM_BASES, "program.design_basis is unsupported")
    _require(program["coordinate_convention"] == "weapon-front-z-up-right-handed@1", "program coordinate convention drifted")
    _sha(program["canonical_sha256"], "program.canonical_sha256", allow_empty=True)

    blade = program["blade_surface"]
    _exact_keys(blade, {"spine_curve", "cutting_edge_curve", "sections", "surface_roles"}, "program.blade_surface")
    _validate_curve(blade["spine_curve"], "program.blade_surface.spine_curve")
    _validate_curve(blade["cutting_edge_curve"], "program.blade_surface.cutting_edge_curve")
    _require(blade["spine_curve"]["curve_id"] != blade["cutting_edge_curve"]["curve_id"], "spine and edge curve IDs must remain independent")
    sections = blade["sections"]
    _require(isinstance(sections, list) and 4 <= len(sections) <= 32, "program.blade_surface.sections count is outside [4,32]")
    section_ids: list[str] = []
    for index, section in enumerate(sections):
        _validate_section(section, f"program.blade_surface.sections[{index}]")
        section_ids.append(section["section_id"])
    _unique(section_ids, "program.blade_surface.sections.section_id")
    # Semantic calibration roles remain unique; a loft may contain more than
    # one explicit intermediate station.  This keeps the closed program
    # contract compatible with the bounded six-plus-section blade fit while
    # preserving the required root/shoulder/belly/tip roles.
    semantic_roles = [section["role"] for section in sections if section["role"] != "intermediate"]
    _unique(semantic_roles, "program.blade_surface.sections.semantic_role")
    section_roles = {section["role"] for section in sections}
    _require(set(ROLES).issubset(section_roles), "program.blade_surface must expose root/shoulder/belly/tip sections")
    surface_roles = blade["surface_roles"]
    _require(isinstance(surface_roles, list) and 4 <= len(surface_roles), "program.blade_surface.surface_roles is too short")
    _unique(surface_roles, "program.blade_surface.surface_roles")
    _require(all(role in SURFACE_ROLES for role in surface_roles), "program.blade_surface.surface_roles contains an unsupported role")

    parts = program["parts"]
    _require(isinstance(parts, list) and 2 <= len(parts) <= 64, "program.parts count is outside [2,64]")
    part_ids: list[str] = []
    for index, part in enumerate(parts):
        _exact_keys(part, {"part_id", "role", "source_class", "material_zone_id", "frozen"}, f"program.parts[{index}]")
        _id(part["part_id"], f"program.parts[{index}].part_id")
        _require(part["role"] in PART_ROLES, f"program.parts[{index}].role is unsupported")
        _require(part["source_class"] in SOURCE_CLASSES, f"program.parts[{index}].source_class is unsupported")
        _id(part["material_zone_id"], f"program.parts[{index}].material_zone_id")
        _require(isinstance(part["frozen"], bool), f"program.parts[{index}].frozen must be boolean")
        part_ids.append(part["part_id"])
    _unique(part_ids, "program.parts.part_id")
    if "assembly" in program:
        _validate_assembly(program["assembly"], parts)

    materials = program["material_zones"]
    _require(isinstance(materials, list) and 1 <= len(materials) <= 32, "program.material_zones count is outside [1,32]")
    material_ids: list[str] = []
    for index, material in enumerate(materials):
        _exact_keys(material, {"material_zone_id", "model", "base_color", "metalness", "roughness"}, f"program.material_zones[{index}]")
        _id(material["material_zone_id"], f"program.material_zones[{index}].material_zone_id")
        _require(material["model"] == "mesh-standard-layered@1", f"program.material_zones[{index}].model drifted")
        _require(isinstance(material["base_color"], str) and re.fullmatch(r"#[A-Fa-f0-9]{6}", material["base_color"]) is not None, f"program.material_zones[{index}].base_color is invalid")
        _number(material["metalness"], f"program.material_zones[{index}].metalness", 0.0, 1.0)
        _number(material["roughness"], f"program.material_zones[{index}].roughness", 0.0, 1.0)
        material_ids.append(material["material_zone_id"])
    _unique(material_ids, "program.material_zones.material_zone_id")
    _require(set(part["material_zone_id"] for part in parts).issubset(set(material_ids)), "parts reference an unknown material zone")

    presentation = program["presentation"]
    _exact_keys(presentation, {"camera_set", "renderer", "aovs"}, "program.presentation")
    _require(presentation["camera_set"] == "knife-fixed-eight-view@1", "program camera set drifted")
    _require(presentation["renderer"] == "threejs-browser-authority@1", "program renderer drifted")
    aovs = presentation["aovs"]
    _require(isinstance(aovs, list) and len(aovs) >= 6, "program.presentation.aovs is too short")
    _unique(aovs, "program.presentation.aovs")
    _require(all(aov in {"beauty", "silhouette", "depth", "normal", "part-id", "material-id", "wireframe", "curvature", "uv-stretch"} for aov in aovs), "program.presentation.aovs contains an unsupported AOV")

    budgets = program["budgets"]
    _exact_keys(budgets, {"max_triangles", "max_draw_calls", "max_texture_bytes"}, "program.budgets")
    _require(isinstance(budgets["max_triangles"], int) and not isinstance(budgets["max_triangles"], bool) and 64 <= budgets["max_triangles"] <= 200000, "program.budgets.max_triangles is invalid")
    _require(isinstance(budgets["max_draw_calls"], int) and not isinstance(budgets["max_draw_calls"], bool) and 1 <= budgets["max_draw_calls"] <= 128, "program.budgets.max_draw_calls is invalid")
    _require(isinstance(budgets["max_texture_bytes"], int) and not isinstance(budgets["max_texture_bytes"], bool) and 0 <= budgets["max_texture_bytes"] <= 268435456, "program.budgets.max_texture_bytes is invalid")

    unknowns = program["unknowns"]
    _require(isinstance(unknowns, list) and len(unknowns) <= 32, "program.unknowns is invalid")
    _unique(unknowns, "program.unknowns")
    for index, unknown in enumerate(unknowns):
        _string(unknown, f"program.unknowns[{index}]", maximum=120)

    digest = canonical_sha256(program)
    if program["canonical_sha256"]:
        _require(program["canonical_sha256"] == digest, "program canonical hash does not match canonical JSON")
    return digest


def validate_program(program: dict[str, Any]) -> str:
    """Public wrapper used by focused callers and smoke checks."""

    return _validate_program(program)


def _validate_ledger(ledger: dict[str, Any]) -> str:
    _exact_keys(ledger, LEDGER_KEYS, "KnifeObjectiveLedger")
    _require(ledger["schema_version"] == "KnifeObjectiveLedger@1", "ledger schema version drifted")
    _id(ledger["ledger_id"], "ledger.ledger_id")
    _require(isinstance(ledger["revision"], int) and not isinstance(ledger["revision"], bool) and ledger["revision"] >= 0, "ledger.revision is invalid")
    parent = ledger["parent_ledger_sha256"]
    _require(parent is None or (isinstance(parent, str) and SHA256_RE.fullmatch(parent) is not None), "ledger.parent_ledger_sha256 is invalid")
    _sha(ledger["program_sha256"], "ledger.program_sha256")
    _sha(ledger["baseline_candidate_sha256"], "ledger.baseline_candidate_sha256")
    _require(ledger["stage"] in STAGES, "ledger.stage is unsupported")
    _require(isinstance(ledger["allowed_scope"], list) and 1 <= len(ledger["allowed_scope"] ) <= 4, "ledger.allowed_scope is invalid")
    for index, scope in enumerate(ledger["allowed_scope"]):
        _id(scope, f"ledger.allowed_scope[{index}]")
    _unique(ledger["allowed_scope"], "ledger.allowed_scope")
    _require(isinstance(ledger["frozen_parts"], list), "ledger.frozen_parts must be a list")
    for index, part_id in enumerate(ledger["frozen_parts"]):
        _id(part_id, f"ledger.frozen_parts[{index}]")
    _unique(ledger["frozen_parts"], "ledger.frozen_parts")
    _require(set(ledger["allowed_scope"]).isdisjoint(set(ledger["frozen_parts"])), "ledger allowed_scope overlaps frozen_parts")
    _string(ledger["hypothesis"], "ledger.hypothesis", minimum=8, maximum=300)
    for field in ("objective_metrics", "regression_limits"):
        values = ledger[field]
        _require(isinstance(values, list), f"ledger.{field} must be a list")
        _unique(values, f"ledger.{field}")
        _require(len(values) <= 12, f"ledger.{field} is too long")
        for index, metric in enumerate(values):
            _require(metric in OBJECTIVE_METRICS, f"ledger.{field}[{index}] is unsupported")
    _require(isinstance(ledger["candidate_budget"], int) and not isinstance(ledger["candidate_budget"], bool) and 1 <= ledger["candidate_budget"] <= MAX_CANDIDATES, "ledger.candidate_budget is invalid")
    _number(ledger["minimum_improvement"], "ledger.minimum_improvement", 0.0, 1.0, exclusive_minimum=True)
    _require(ledger["plateau_limit"] == 2, "ledger.plateau_limit must remain 2")
    evidence = ledger["evidence_sha256"]
    _require(isinstance(evidence, list) and 1 <= len(evidence) <= 32, "ledger.evidence_sha256 is invalid")
    _unique(evidence, "ledger.evidence_sha256")
    for index, item in enumerate(evidence):
        _sha(item, f"ledger.evidence_sha256[{index}]")
    _require(ledger["status"] in LEDGER_STATUSES, "ledger.status is unsupported")
    _sha(ledger["canonical_sha256"], "ledger.canonical_sha256", allow_empty=True)
    digest = canonical_sha256(ledger)
    if ledger["canonical_sha256"]:
        _require(ledger["canonical_sha256"] == digest, "ledger canonical hash does not match canonical JSON")
    return digest


def validate_ledger(ledger: dict[str, Any]) -> str:
    """Public wrapper used by focused callers and smoke checks."""

    return _validate_ledger(ledger)


def _dominant_longitudinal_axis(program: dict[str, Any]) -> int:
    points = []
    for name in CURVE_NAMES:
        points.extend(program["blade_surface"][name]["control_points"])
    spans = [max(point[axis] for point in points) - min(point[axis] for point in points) for axis in range(3)]
    return max(range(3), key=lambda axis: (spans[axis], -axis))


def _required_section_indices(program: dict[str, Any]) -> list[int]:
    sections = program["blade_surface"]["sections"]
    by_role = {section["role"]: index for index, section in enumerate(sections)}
    return [by_role[role] for role in ROLES]


def _bezier_point(points: list[list[float]], t: float) -> list[float]:
    work = [[float(value) for value in point] for point in points]
    while len(work) > 1:
        work = [[(1.0 - t) * left[axis] + t * right[axis] for axis in range(3)] for left, right in zip(work, work[1:])]
    return work[0]


def _sample_curve(curve: dict[str, Any], count: int = 33) -> list[list[float]]:
    points = curve["control_points"]
    samples: list[list[float]] = []
    for index in range(count):
        t = index / float(count - 1)
        if curve["basis"] == "bezier":
            samples.append(_bezier_point(points, t))
            continue
        position = t * (len(points) - 1)
        left = min(int(math.floor(position)), len(points) - 2)
        local = position - left
        samples.append([
            (1.0 - local) * float(points[left][axis]) + local * float(points[left + 1][axis])
            for axis in range(3)
        ])
    return samples


def _distance(left: list[float], right: list[float]) -> float:
    return math.sqrt(sum((left[axis] - right[axis]) ** 2 for axis in range(3)))


def _arc_length(points: list[list[float]]) -> float:
    return sum(_distance(left, right) for left, right in zip(points, points[1:]))


def _clamp(value: float, minimum: float, maximum: float) -> float:
    return max(minimum, min(maximum, value))


def _continuity(values: list[float]) -> float:
    if len(values) < 3:
        return 1.0
    scale = max(max(values) - min(values), max(abs(value) for value in values) * 0.25, 1e-6)
    second = sum(abs(values[index + 1] - 2.0 * values[index] + values[index - 1]) for index in range(1, len(values) - 1))
    return _clamp(1.0 - second / (2.0 * scale * (len(values) - 2)), 0.0, 1.0)


def _curve_smoothness(points: list[list[float]]) -> float:
    angles: list[float] = []
    for index in range(1, len(points) - 1):
        before = [points[index][axis] - points[index - 1][axis] for axis in range(3)]
        after = [points[index + 1][axis] - points[index][axis] for axis in range(3)]
        before_length = math.sqrt(sum(value * value for value in before))
        after_length = math.sqrt(sum(value * value for value in after))
        if before_length <= 1e-12 or after_length <= 1e-12:
            continue
        dot = sum(before[axis] * after[axis] for axis in range(3)) / (before_length * after_length)
        angles.append(math.acos(_clamp(dot, -1.0, 1.0)))
    if not angles:
        return 0.0
    return _clamp(1.0 - sum(angles) / (len(angles) * math.pi), 0.0, 1.0)


def _orientation(a: tuple[float, float], b: tuple[float, float], c: tuple[float, float]) -> float:
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def _segments_intersect(a: tuple[float, float], b: tuple[float, float], c: tuple[float, float], d: tuple[float, float]) -> bool:
    epsilon = 1e-10
    ab_c = _orientation(a, b, c)
    ab_d = _orientation(a, b, d)
    cd_a = _orientation(c, d, a)
    cd_b = _orientation(c, d, b)
    if max(abs(ab_c), abs(ab_d), abs(cd_a), abs(cd_b)) <= epsilon:
        # Collinear, disjoint runs are not self-intersections.  This matters
        # for envelope-fitted uniform stations whose local reference segment
        # can be exactly straight.
        return not (
            max(a[0], b[0]) < min(c[0], d[0]) - epsilon
            or max(c[0], d[0]) < min(a[0], b[0]) - epsilon
            or max(a[1], b[1]) < min(c[1], d[1]) - epsilon
            or max(c[1], d[1]) < min(a[1], b[1]) - epsilon
        )
    if (ab_c > epsilon and ab_d > epsilon) or (ab_c < -epsilon and ab_d < -epsilon):
        return False
    if (cd_a > epsilon and cd_b > epsilon) or (cd_a < -epsilon and cd_b < -epsilon):
        return False
    return True


def _polyline_self_intersects(points: list[list[float]], axis_a: int, axis_b: int) -> bool:
    projected = [(float(point[axis_a]), float(point[axis_b])) for point in points]
    for first in range(len(projected) - 1):
        for second in range(first + 2, len(projected) - 1):
            if second == first + 1:
                continue
            if _segments_intersect(projected[first], projected[first + 1], projected[second], projected[second + 1]):
                return True
    return False


def _assembly_specs_by_part_id(program: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Return validated assembly records without introducing a new schema."""

    assembly = program.get("assembly")
    if not isinstance(assembly, dict):
        return {}
    specs: list[dict[str, Any]] = []
    for primitive in ("guard", "grip", "pommel"):
        spec = assembly.get(primitive)
        if isinstance(spec, dict):
            specs.append(spec)
    for label in ("fasteners", "gems", "reliefs"):
        values = assembly.get(label, [])
        if isinstance(values, list):
            specs.extend(spec for spec in values if isinstance(spec, dict))
    return {str(spec["part_id"]): spec for spec in specs}


def _assembly_box_extents(spec: dict[str, Any]) -> list[float]:
    """Approximate a primitive's bounded world extents for numeric proxies."""

    primitive = spec["primitive"]
    if primitive == "guard":
        upper = spec.get("upper_jaw")
        lower = spec.get("lower_jaw")
        if isinstance(upper, dict) and isinstance(lower, dict):
            jaw_lateral = max(
                float(upper["span"]) * 0.5 + abs(float(upper["offset_y"])),
                float(lower["span"]) * 0.5 + abs(float(lower["offset_y"])),
            ) + float(spec["jaw_gap"]) * 0.5
            horn_length = max((float(horn["length"]) for horn in spec["horns"]), default=0.0)
            return [
                max(float(spec["thickness"]) * 0.5, horn_length * 0.75),
                max(float(spec["span"]) * 0.5, jaw_lateral),
                max(float(spec["depth"]) * 0.5, horn_length * 0.2),
            ]
        return [float(spec["thickness"]) * 0.5, float(spec["span"]) * 0.5, float(spec["depth"]) * 0.5]
    if primitive == "grip":
        radial = float(spec["radius"]) * (1.0 + abs(float(spec["taper"])))
        return [float(spec["length"]) * 0.5, radial, radial]
    if primitive == "pommel":
        hook = spec.get("hook")
        hook_length = float(hook["length"]) if isinstance(hook, dict) else 0.0
        hook_bend = abs(float(hook["bend"])) if isinstance(hook, dict) else 0.0
        return [
            float(spec["length"]) * 0.5 + hook_length,
            float(spec["radius"]) + hook_length * hook_bend,
            float(spec["depth"]) * 0.5,
        ]
    if primitive == "relief":
        return [float(spec["width"]) * 0.5, float(spec["height"]) * 0.5, float(spec["depth"]) * 0.5]
    radius = float(spec["radius"])
    depth = float(spec["depth"]) * 0.5
    return [max(radius, depth), max(radius, depth), max(radius, depth)]


def _asset_bbox_points(program: dict[str, Any], blade_points: list[list[float]]) -> list[list[float]]:
    points = [list(point) for point in blade_points]
    for spec in _assembly_specs_by_part_id(program).values():
        center = [float(value) for value in spec["center"]]
        extents = _assembly_box_extents(spec)
        for axis in range(3):
            plus = list(center)
            minus = list(center)
            plus[axis] += extents[axis]
            minus[axis] -= extents[axis]
            points.extend((plus, minus))
    return points


def _hook_path_points(spec: dict[str, Any]) -> list[list[float]]:
    hook = spec["hook"]
    length = float(hook["length"])
    bend = float(hook["bend"])
    direction = float(hook["direction"])
    # This is the same closed six-station path consumed by the package
    # attachment compiler, represented as plain numbers for no-render search.
    return [
        [length * 0.12, 0.0, 0.0],
        [length * 0.42, 0.0, 0.0],
        [length * 0.70, direction * length * 0.20 * bend, 0.0],
        [length * 0.92, direction * length * 0.58 * bend, 0.0],
        [length * 0.70, direction * length * 0.88 * bend, 0.0],
        [length * 0.46, direction * length * 0.96 * bend, 0.0],
    ]


def _set_intrinsic_metric(
    values: dict[str, float | None],
    details: dict[str, dict[str, Any]],
    name: str,
    value: float,
    direction: str,
    basis: str,
) -> None:
    bounded = float(value)
    _require(math.isfinite(bounded), f"intrinsic metric {name} is not finite")
    values[name] = round(bounded, 9)
    details[name] = {"direction": direction, "computable": True, "basis": basis}


def _assembly_intrinsic_metrics(
    program: dict[str, Any],
    blade_points: list[list[float]],
    longitudinal_axis: int,
    lateral_axis: int,
) -> tuple[dict[str, float | None], dict[str, dict[str, Any]]]:
    """Compute bounded attachment/relief proxies without rendering or image facts."""

    values: dict[str, float | None] = {
        "negative-space-proxy": None,
        "negative-space-error": None,
        "hook-continuity": None,
        "hook-continuity-error": None,
        "relief-coverage": None,
        "relief-coverage-error": None,
        "relief-spacing": None,
        "relief-spacing-error": None,
    }
    details: dict[str, dict[str, Any]] = {
        "negative-space-proxy": {"direction": "maximize", "computable": False, "basis": "dragon-guard-required"},
        "negative-space-error": {"direction": "minimize", "computable": False, "basis": "dragon-guard-required"},
        "hook-continuity": {"direction": "maximize", "computable": False, "basis": "hooked-pommel-required"},
        "hook-continuity-error": {"direction": "minimize", "computable": False, "basis": "hooked-pommel-required"},
        "relief-coverage": {"direction": "maximize", "computable": False, "basis": "relief-required"},
        "relief-coverage-error": {"direction": "minimize", "computable": False, "basis": "relief-required"},
        "relief-spacing": {"direction": "maximize", "computable": False, "basis": "relief-required"},
        "relief-spacing-error": {"direction": "minimize", "computable": False, "basis": "relief-required"},
    }
    specs = _assembly_specs_by_part_id(program)

    guard = specs.get("guard")
    if isinstance(guard, dict) and guard.get("style") == "dragon-guard":
        upper = guard["upper_jaw"]
        lower = guard["lower_jaw"]
        negative_curvature = max(0.0, -float(upper["curvature"]), -float(lower["curvature"]))
        rail_clearance = max(
            0.0,
            float(guard["jaw_gap"])
            - abs(float(upper["offset_y"]) - float(lower["offset_y"]))
            - 2.0 * negative_curvature
            - 0.5 * (float(upper["thickness"]) + float(lower["thickness"])),
        )
        opening_span = min(float(upper["span"]), float(lower["span"]))
        guard_span = max(float(guard["span"]), opening_span, MIN_DIMENSION)
        guard_height = max(
            float(guard["jaw_gap"])
            + float(upper["thickness"])
            + float(lower["thickness"])
            + abs(float(upper["offset_y"]) - float(lower["offset_y"])),
            MIN_DIMENSION,
        )
        negative_space_proxy = _clamp(rail_clearance * opening_span / (guard_span * guard_height), 0.0, 1.0)
        _set_intrinsic_metric(
            values,
            details,
            "negative-space-proxy",
            negative_space_proxy,
            "maximize",
            "dragon-jaw-gap-rail-area-proxy-no-render",
        )
        _set_intrinsic_metric(
            values,
            details,
            "negative-space-error",
            1.0 - negative_space_proxy,
            "minimize",
            "one-minus-dragon-jaw-gap-proxy-no-render",
        )

    pommel = specs.get("pommel")
    if isinstance(pommel, dict) and pommel.get("style") == "hooked-pommel":
        hook_continuity = _curve_smoothness(_hook_path_points(pommel))
        _set_intrinsic_metric(
            values,
            details,
            "hook-continuity",
            hook_continuity,
            "maximize",
            "hook-path-turning-continuity-proxy-no-render",
        )
        _set_intrinsic_metric(
            values,
            details,
            "hook-continuity-error",
            1.0 - hook_continuity,
            "minimize",
            "one-minus-hook-path-continuity-proxy-no-render",
        )

    reliefs = [spec for spec in specs.values() if spec.get("primitive") == "relief"]
    if reliefs:
        longitudinal_values = [float(point[longitudinal_axis]) for point in blade_points]
        lateral_values = [float(point[lateral_axis]) for point in blade_points]
        blade_longitudinal_span = max(max(longitudinal_values) - min(longitudinal_values), MIN_DIMENSION)
        blade_lateral_span = max(max(lateral_values) - min(lateral_values), MIN_DIMENSION)
        blade_area = max(blade_longitudinal_span * blade_lateral_span, MIN_DIMENSION)
        relief_area = sum(float(spec["width"]) * float(spec["height"]) for spec in reliefs)
        relief_coverage = _clamp(relief_area / blade_area, 0.0, 1.0)
        _set_intrinsic_metric(
            values,
            details,
            "relief-coverage",
            relief_coverage,
            "maximize",
            "relief-projected-area-over-blade-envelope-proxy-no-render",
        )
        _set_intrinsic_metric(
            values,
            details,
            "relief-coverage-error",
            1.0 - relief_coverage,
            "minimize",
            "one-minus-relief-coverage-proxy-no-render",
        )
        if len(reliefs) < 2:
            relief_spacing = 1.0
        else:
            clearances: list[float] = []
            for first_index, first in enumerate(reliefs):
                first_center = first["center"]
                first_radius = 0.5 * math.hypot(float(first["width"]), float(first["height"]))
                for second in reliefs[first_index + 1 :]:
                    second_center = second["center"]
                    second_radius = 0.5 * math.hypot(float(second["width"]), float(second["height"]))
                    center_distance = math.hypot(
                        float(first_center[longitudinal_axis]) - float(second_center[longitudinal_axis]),
                        float(first_center[lateral_axis]) - float(second_center[lateral_axis]),
                    )
                    clearances.append(max(0.0, center_distance - first_radius - second_radius))
            relief_spacing = _clamp(min(clearances) / max(blade_longitudinal_span, blade_lateral_span, MIN_DIMENSION), 0.0, 1.0)
        _set_intrinsic_metric(
            values,
            details,
            "relief-spacing",
            relief_spacing,
            "maximize",
            "pairwise-relief-clearance-over-blade-span-proxy-no-render",
        )
        _set_intrinsic_metric(
            values,
            details,
            "relief-spacing-error",
            1.0 - relief_spacing,
            "minimize",
            "one-minus-relief-spacing-proxy-no-render",
        )

    asset_points = _asset_bbox_points(program, blade_points)
    asset_longitudinal_span = max(
        max(float(point[longitudinal_axis]) for point in asset_points)
        - min(float(point[longitudinal_axis]) for point in asset_points),
        MIN_DIMENSION,
    )
    asset_lateral_span = max(
        max(float(point[lateral_axis]) for point in asset_points)
        - min(float(point[lateral_axis]) for point in asset_points),
        MIN_DIMENSION,
    )
    blade_longitudinal_span = max(
        max(float(point[longitudinal_axis]) for point in blade_points)
        - min(float(point[longitudinal_axis]) for point in blade_points),
        MIN_DIMENSION,
    )
    blade_lateral_span = max(
        max(float(point[lateral_axis]) for point in blade_points)
        - min(float(point[lateral_axis]) for point in blade_points),
        MIN_DIMENSION,
    )
    frame_longitudinal_span = max(blade_longitudinal_span * 3.0, 1.0)
    frame_lateral_span = max(blade_lateral_span * 3.0, 0.5)
    fps_occupancy = _clamp(
        (asset_longitudinal_span / frame_longitudinal_span)
        * (asset_lateral_span / frame_lateral_span),
        0.0,
        1.0,
    )
    _set_intrinsic_metric(
        values,
        details,
        "fps-occupancy",
        fps_occupancy,
        "maximize",
        "fixed-normalized-frame-bbox-proxy-no-render",
    )
    return values, details


def evaluate_geometry(program: dict[str, Any]) -> dict[str, Any]:
    """Evaluate intrinsic geometry only; no render or image metric is used."""

    _validate_program(program)
    blade = program["blade_surface"]
    sections = blade["sections"]
    required_indices = _required_section_indices(program)
    required_sections = [sections[index] for index in required_indices]
    spine = _sample_curve(blade["spine_curve"])
    edge = _sample_curve(blade["cutting_edge_curve"])
    longitudinal_axis = _dominant_longitudinal_axis(program)
    non_longitudinal = [axis for axis in range(3) if axis != longitudinal_axis]
    lateral_axis = max(non_longitudinal, key=lambda axis: max(point[axis] for point in spine + edge) - min(point[axis] for point in spine + edge))
    thickness_axis = next(axis for axis in non_longitudinal if axis != lateral_axis)

    widths = [float(section["half_width"]) for section in required_sections]
    thicknesses = [float(section["thickness"]) for section in required_sections]
    asymmetries = [float(section["asymmetry"]) for section in required_sections]
    edge_offsets = [float(section["edge_offset"]) for section in required_sections]
    spine_offsets = [float(section["spine_offset"]) for section in required_sections]
    twists = [float(section["twist"]) for section in required_sections]
    pair_separations = [_distance(left, right) for left, right in zip(spine, edge)]
    max_width = max(widths[:-1]) if len(widths) > 1 else widths[0]
    tip_convergence = _clamp(1.0 - 0.5 * (widths[-1] / max(max_width, 1e-9) + thicknesses[-1] / max(max(thicknesses[:-1]), 1e-9)), 0.0, 1.0)
    taper_steps = [1.0 if right <= left + 1e-12 else 0.0 for left, right in zip(thicknesses, thicknesses[1:])]
    taper_monotonicity = sum(taper_steps) / max(len(taper_steps), 1)
    curve_smoothness = (_curve_smoothness(spine) + _curve_smoothness(edge)) / 2.0
    section_continuity = (_continuity(widths) + _continuity(thicknesses) + _continuity(asymmetries) + _continuity(edge_offsets) + _continuity(spine_offsets) + _continuity(twists)) / 6.0
    thickness_continuity = (_continuity(thicknesses) + _continuity(widths)) / 2.0
    normal_continuity = _clamp(0.55 * curve_smoothness + 0.45 * section_continuity, 0.0, 1.0)
    separation_scale = max(_arc_length(spine), _arc_length(edge), 1e-9)
    min_separation = min(pair_separations)
    min_separation_normalized = _clamp(min_separation / separation_scale * len(spine), 0.0, 1.0)
    estimated_triangles = (len(sections) - 1) * (len(spine) - 1) * 4
    assembly_metric_values, assembly_metric_details = _assembly_intrinsic_metrics(
        program,
        spine + edge,
        longitudinal_axis,
        lateral_axis,
    )

    gates = {
        "finite_values": True,
        "independent_spine_edge_ids": blade["spine_curve"]["curve_id"] != blade["cutting_edge_curve"]["curve_id"],
        "sections_strictly_monotonic": all(left["u"] < right["u"] for left, right in zip(sections, sections[1:])),
        "required_four_sections_present": [section["role"] for section in sections if section["role"] in ROLES] == list(ROLES),
        "positive_section_width_and_thickness": all(section["half_width"] > 0.0 and section["thickness"] > 0.0 for section in sections),
        "tip_converges_without_zero_section": widths[-1] < max_width and thicknesses[-1] < max(thicknesses[:-1]),
        "spine_edge_separation": min_separation > 1e-6,
        "nondegenerate_curve_samples": all(_distance(left, right) > 1e-9 for samples in (spine, edge) for left, right in zip(samples, samples[1:])),
        "spine_longitudinal_order": all((right[longitudinal_axis] - left[longitudinal_axis]) >= -1e-8 for left, right in zip(spine, spine[1:])),
        "edge_longitudinal_order": all((right[longitudinal_axis] - left[longitudinal_axis]) >= -1e-8 for left, right in zip(edge, edge[1:])),
        "spine_curve_no_planar_self_intersection": not _polyline_self_intersects(spine, longitudinal_axis, lateral_axis),
        "edge_curve_no_planar_self_intersection": not _polyline_self_intersects(edge, longitudinal_axis, lateral_axis),
        "estimated_triangle_budget": estimated_triangles <= program["budgets"]["max_triangles"],
    }
    metric_values = {
        "section-profile-continuity": round(section_continuity, 9),
        "thickness-continuity": round(thickness_continuity, 9),
        "normal-continuity": round(normal_continuity, 9),
        "curve-smoothness": round(curve_smoothness, 9),
        "tip-convergence": round(tip_convergence, 9),
        "taper-monotonicity": round(taper_monotonicity, 9),
        "min-curve-separation": round(min_separation_normalized, 9),
        "estimated-triangles": estimated_triangles,
        "part-id-coverage": 1.0 if program["parts"] else 0.0,
        "material-id-coverage": 1.0 if all(part["material_zone_id"] for part in program["parts"]) else 0.0,
        **assembly_metric_values,
    }
    metric_details = {
        "section-profile-continuity": {"direction": "maximize", "computable": True, "basis": "intrinsic-section-profile-no-render"},
        "thickness-continuity": {"direction": "maximize", "computable": True, "basis": "intrinsic-section-profile-no-render"},
        "normal-continuity": {"direction": "maximize", "computable": True, "basis": "surface-profile-smoothness-proxy-no-render"},
        "curve-smoothness": {"direction": "maximize", "computable": True, "basis": "sampled-curve-turning-no-render"},
        "tip-convergence": {"direction": "maximize", "computable": True, "basis": "four-section-proportion-no-render"},
        "taper-monotonicity": {"direction": "maximize", "computable": True, "basis": "section-thickness-order-no-render"},
        "min-curve-separation": {"direction": "maximize", "computable": True, "basis": "spine-edge-sampled-distance-no-render"},
        "estimated-triangles": {"direction": "minimize", "computable": True, "basis": "bounded-loft-cost-estimate-no-render"},
        "part-id-coverage": {"direction": "maximize", "computable": True, "basis": "semantic-program-binding-no-render"},
        "material-id-coverage": {"direction": "maximize", "computable": True, "basis": "semantic-program-binding-no-render"},
    }
    metric_details.update(assembly_metric_details)
    return {
        "hard_gates": gates,
        "hard_gate_pass": all(gates.values()),
        "metrics": metric_values,
        "metric_details": metric_details,
        "coordinate_axes": {"longitudinal": longitudinal_axis, "lateral": lateral_axis, "thickness": thickness_axis},
        "required_section_roles": list(ROLES),
        "sample_count": len(spine),
    }


def _diff_paths(left: Any, right: Any, prefix: str = "") -> list[str]:
    if isinstance(left, dict) and isinstance(right, dict):
        paths: list[str] = []
        for key in sorted(set(left) | set(right)):
            if key == "canonical_sha256":
                continue
            child = f"{prefix}.{key}" if prefix else key
            if key not in left or key not in right:
                paths.append(child)
            else:
                paths.extend(_diff_paths(left[key], right[key], child))
        return paths
    if isinstance(left, list) and isinstance(right, list):
        paths = []
        for index in range(max(len(left), len(right))):
            child = f"{prefix}[{index}]"
            if index >= len(left) or index >= len(right):
                paths.append(child)
            else:
                paths.extend(_diff_paths(left[index], right[index], child))
        return paths
    return [] if left == right else [prefix]


def _path_scope_id(path: str, program: dict[str, Any] | None = None) -> str | None:
    """Resolve one exact mutable path to its semantic ledger owner."""

    curve_match = re.fullmatch(
        r"blade_surface\.(spine_curve|cutting_edge_curve)\.control_points\[(\d+)\]\[([012])\]",
        path,
    )
    if curve_match:
        if program is not None:
            curve_name, point_index, _ = curve_match.groups()
            if int(point_index) >= len(program["blade_surface"][curve_name]["control_points"]):
                return None
        return "blade-body" if curve_match.group(1) == "spine_curve" else "cutting-edge"

    section_match = re.fullmatch(r"blade_surface\.sections\[(\d+)\]\.([a-z_]+)", path)
    if section_match and section_match.group(2) in SECTION_FIELDS:
        if program is not None and int(section_match.group(1)) >= len(program["blade_surface"]["sections"]):
            return None
        return "blade-body"

    if re.fullmatch(
        r"assembly\.guard\.(?:span|thickness|depth|jaw_gap|"
        r"(?:upper_jaw|lower_jaw)\.(?:span|thickness|depth|offset_y|offset_z|curvature)|"
        r"horns\[\d+\]\.(?:length|radius|sweep|offset_z))",
        path,
    ):
        if program is not None:
            horn_match = re.fullmatch(r"assembly\.guard\.horns\[(\d+)\]\..+", path)
            if horn_match and int(horn_match.group(1)) >= len(program.get("assembly", {}).get("guard", {}).get("horns", [])):
                return None
        return "guard"

    if re.fullmatch(r"assembly\.pommel\.(?:length|radius|depth|hook\.(?:length|radius|bend))", path):
        return "pommel"

    relief_match = re.fullmatch(
        r"assembly\.reliefs\[(\d+)\]\.(?:width|height|depth|center\[[012]\])",
        path,
    )
    if relief_match:
        if program is None:
            return "relief"
        index = int(relief_match.group(1))
        reliefs = program.get("assembly", {}).get("reliefs", [])
        if index >= len(reliefs):
            return None
        return str(reliefs[index].get("part_id"))
    return None


def _allowed_change_path(
    path: str,
    program: dict[str, Any] | None = None,
    allowed_scope: Iterable[str] | None = None,
) -> bool:
    owner = _path_scope_id(path, program)
    return owner is not None and (allowed_scope is None or owner in set(allowed_scope))


def _frozen_snapshot(program: dict[str, Any], frozen_parts: list[str]) -> dict[str, Any]:
    by_id = {part["part_id"]: part for part in program["parts"]}
    missing = [part_id for part_id in frozen_parts if part_id not in by_id]
    _require(not missing, f"ledger frozen_parts reference unknown parts: {missing}")
    return {part_id: copy.deepcopy(by_id[part_id]) for part_id in frozen_parts}


def _frozen_assembly_snapshot(program: dict[str, Any], frozen_parts: list[str]) -> dict[str, Any]:
    assembly = _assembly_specs_by_part_id(program)
    return {part_id: copy.deepcopy(assembly[part_id]) for part_id in frozen_parts if part_id in assembly}


def _validate_scope_compatibility(program: dict[str, Any], ledger: dict[str, Any]) -> None:
    part_ids = {part["part_id"] for part in program["parts"]}
    _require(set(ledger["allowed_scope"]).issubset(part_ids), "ledger.allowed_scope must name program parts")
    _require(set(ledger["frozen_parts"]).issubset(part_ids), "ledger.frozen_parts must name program parts")
    # The ledger is the authority for this search's mutable/frozen boundary.
    # A program part can remain marked ``frozen`` because it is the accepted
    # baseline lineage while its blade surface is the explicitly authorized
    # successor scope.  Conversely, ledger.frozen_parts names the assembly
    # records that must remain byte-equivalent even when their program-level
    # ``frozen`` flag is false.  The path gate and frozen snapshot below keep
    # both records immutable; do not conflate the two vocabularies here.
    _require(set(ledger["allowed_scope"]).isdisjoint(set(ledger["frozen_parts"])), "ledger scope overlaps frozen_parts")


def _parameter_pool(
    program: dict[str, Any],
    axes: list[int],
    allowed_scope: Iterable[str] | None = None,
) -> tuple[list[tuple[Any, ...]], list[tuple[Any, ...]]]:
    scope = set(allowed_scope) if allowed_scope is not None else {"blade-body", "cutting-edge"}
    blade = program["blade_surface"]
    curve_pool: list[tuple[Any, ...]] = []
    for curve_name in CURVE_NAMES:
        curve_owner = "blade-body" if curve_name == "spine_curve" else "cutting-edge"
        if curve_owner not in scope:
            continue
        curve = blade[curve_name]
        for point_index in range(len(curve["control_points"])):
            for axis in axes:
                curve_pool.append(("curve", curve_name, point_index, axis))
    parameter_pool: list[tuple[Any, ...]] = []
    if {"blade-body", "cutting-edge"} & scope:
        by_role = {section["role"]: index for index, section in enumerate(blade["sections"])}
        for role in ROLES:
            index = by_role[role]
            for field in SECTION_FIELDS:
                parameter_pool.append(("section", index, role, field))

    assembly = program.get("assembly")
    if isinstance(assembly, dict):
        guard = assembly.get("guard")
        if isinstance(guard, dict) and guard.get("part_id") in scope:
            if guard.get("style") == "dragon-guard":
                for field in ("span", "thickness", "depth", "jaw_gap"):
                    parameter_pool.append(("assembly", "guard", field))
                for jaw_name in ("upper_jaw", "lower_jaw"):
                    for field in ("span", "curvature", "offset_y"):
                        parameter_pool.append(("assembly", "guard", jaw_name, field))
                for horn_index in range(len(guard["horns"])):
                    parameter_pool.append(("assembly", "guard", "horns", horn_index, "sweep"))
            else:
                for field in ("span", "thickness", "depth"):
                    parameter_pool.append(("assembly", "guard", field))

        pommel = assembly.get("pommel")
        if isinstance(pommel, dict) and pommel.get("part_id") in scope:
            for field in ("length", "radius", "depth"):
                parameter_pool.append(("assembly", "pommel", field))
            if pommel.get("style") == "hooked-pommel":
                for field in ("length", "radius", "bend"):
                    parameter_pool.append(("assembly", "pommel", "hook", field))

        reliefs = assembly.get("reliefs", [])
        if isinstance(reliefs, list):
            for index, relief in enumerate(reliefs):
                if isinstance(relief, dict) and relief.get("part_id") in scope:
                    for field in ("width", "height", "depth"):
                        parameter_pool.append(("assembly", "relief", index, field))
                    for axis in range(3):
                        parameter_pool.append(("assembly", "relief", index, "center", axis))
    return curve_pool, parameter_pool


def _random_delta(rng: random.Random, maximum: float) -> float:
    magnitude = maximum * (0.15 + 0.65 * rng.random())
    return round(magnitude if rng.randrange(2) else -magnitude, 6)


def _read_parameter(program: dict[str, Any], parameter: tuple[Any, ...]) -> float:
    if parameter[0] == "curve":
        _, curve_name, point_index, axis = parameter
        return float(program["blade_surface"][curve_name]["control_points"][point_index][axis])
    if parameter[0] == "section":
        _, section_index, _, field = parameter
        return float(program["blade_surface"]["sections"][section_index][field])
    _require(parameter[0] == "assembly", f"unsupported parameter kind: {parameter[0]}")
    _, primitive, *tokens = parameter
    if primitive == "relief":
        spec = program["assembly"]["reliefs"][tokens.pop(0)]
    else:
        spec = program["assembly"][primitive]
    value: Any = spec
    for token in tokens:
        value = value[token]
    return float(value)


def _write_parameter(program: dict[str, Any], parameter: tuple[Any, ...], value: float) -> float:
    if parameter[0] == "curve":
        _, curve_name, point_index, axis = parameter
        bounded = _clamp(value, -4.0, 4.0)
        program["blade_surface"][curve_name]["control_points"][point_index][axis] = round(bounded, 6)
        return round(bounded, 6)
    if parameter[0] == "section":
        _, section_index, _, field = parameter
        limits = {
            "half_width": (0.0005, 2.0),
            "thickness": (0.0005, 1.0),
            "edge_offset": (-1.0, 1.0),
            "spine_offset": (-1.0, 1.0),
            "asymmetry": (-1.0, 1.0),
            "twist": (-1.5708, 1.5708),
        }
        minimum, maximum = limits[field]
        bounded = _clamp(value, minimum, maximum)
        program["blade_surface"]["sections"][section_index][field] = round(bounded, 6)
        return round(bounded, 6)

    _require(parameter[0] == "assembly", f"unsupported parameter kind: {parameter[0]}")
    _, primitive, *tokens = parameter
    if primitive == "relief":
        spec = program["assembly"]["reliefs"][tokens.pop(0)]
    else:
        spec = program["assembly"][primitive]
    field = tokens[-1]
    field_name = "center" if isinstance(field, int) else str(field)
    limits = {
        "span": (0.0005, 2.0),
        "thickness": (0.0005, 1.0),
        "depth": (0.0005, 1.0),
        "jaw_gap": (0.0101, 0.6),
        "length": (0.0005, 2.0),
        "radius": (0.0005, 1.0),
        "bend": (0.2, 1.0),
        "curvature": (-0.25, 0.25),
        "offset_y": (-0.8, 0.8),
        "sweep": (-0.75, 0.75),
        "width": (0.0005, 2.0),
        "height": (0.0005, 2.0),
        "center": (-4.0, 4.0),
    }
    minimum, maximum = limits[field_name]
    bounded = _clamp(value, minimum, maximum)
    cursor: Any = spec
    for token in tokens[:-1]:
        cursor = cursor[token]
    cursor[field] = round(bounded, 6)
    return round(bounded, 6)


def _parameter_path(parameter: tuple[Any, ...]) -> str:
    if parameter[0] == "curve":
        _, curve_name, point_index, axis = parameter
        return f"blade_surface.{curve_name}.control_points[{point_index}][{axis}]"
    if parameter[0] == "section":
        _, section_index, _, field = parameter
        return f"blade_surface.sections[{section_index}].{field}"
    _, primitive, *tokens = parameter
    path = "assembly.reliefs" if primitive == "relief" else f"assembly.{primitive}"
    if primitive == "relief":
        path += f"[{tokens.pop(0)}]"
    for token in tokens:
        if isinstance(token, int):
            path += f"[{token}]"
        else:
            path += f".{token}"
    return path


def _parameter_delta_limit(parameter: tuple[Any, ...]) -> float:
    field = parameter[-1] if isinstance(parameter[-1], str) else "center"
    return {
        "half_width": 0.08,
        "thickness": 0.025,
        "edge_offset": 0.06,
        "spine_offset": 0.06,
        "asymmetry": 0.06,
        "twist": 0.10,
        "span": 0.06,
        "depth": 0.025,
        "jaw_gap": 0.025,
        "length": 0.06,
        "radius": 0.025,
        "bend": 0.08,
        "curvature": 0.025,
        "offset_y": 0.035,
        "sweep": 0.08,
        "width": 0.08,
        "height": 0.025,
        "center": 0.045,
    }.get(str(field), 0.025)


def _metric_direction(name: str, details: dict[str, Any]) -> str:
    if name in details:
        return str(details[name]["direction"])
    return "minimize" if name.endswith("error") or name in {"symmetric-chamfer", "p95-contour-distance", "estimated-triangles"} else "maximize"


def _objective_projection(evaluation: dict[str, Any], ledger: dict[str, Any]) -> tuple[dict[str, float | None], dict[str, Any]]:
    values = evaluation["metrics"]
    objective_values: dict[str, float | None] = {}
    details: dict[str, Any] = {}
    for name in ledger["objective_metrics"]:
        value = values.get(name)
        computable = value is not None
        objective_values[name] = value
        details[name] = {
            "value": value,
            "direction": _metric_direction(name, evaluation["metric_details"]),
            "computable": computable,
            "basis": evaluation["metric_details"].get(name, {}).get("basis", "reference-or-render-required"),
        }
    return objective_values, details


def _compare_objectives(baseline: dict[str, Any], candidate: dict[str, Any], ledger: dict[str, Any]) -> dict[str, Any]:
    baseline_values, baseline_details = _objective_projection(baseline, ledger)
    candidate_values, candidate_details = _objective_projection(candidate, ledger)
    improvements: dict[str, float | None] = {}
    for name in ledger["objective_metrics"]:
        left = baseline_values[name]
        right = candidate_values[name]
        if left is None or right is None:
            improvements[name] = None
            continue
        direction = baseline_details[name]["direction"]
        improvements[name] = round(right - left if direction == "maximize" else left - right, 9)
    computable_regressions: dict[str, bool | None] = {}
    for name in ledger["regression_limits"]:
        left = baseline_values.get(name)
        right = candidate_values.get(name)
        if left is None or right is None:
            computable_regressions[name] = None
            continue
        direction = baseline_details[name]["direction"]
        computable_regressions[name] = right >= left - 1e-12 if direction == "maximize" else right <= left + 1e-12
    computable_improvements = [value for value in improvements.values() if value is not None]
    target_improved = any(value >= ledger["minimum_improvement"] for value in computable_improvements)
    regression_values = [value for value in computable_regressions.values() if value is not None]
    regression_ok = all(regression_values) if regression_values else None
    return {
        "baseline": baseline_values,
        "candidate": candidate_values,
        "improvements": improvements,
        "regression_limits": computable_regressions,
        "target_improved": target_improved,
        "regression_ok": regression_ok,
        "all_objectives_computable": all(value is not None for value in candidate_values.values()),
        "baseline_details": baseline_details,
        "candidate_details": candidate_details,
    }


def generate_candidates(program: dict[str, Any], ledger: dict[str, Any], seed: int, candidate_count: int) -> dict[str, Any]:
    """Generate and evaluate a bounded immutable candidate set."""

    program_hash = _validate_program(program)
    ledger_hash = _validate_ledger(ledger)
    _require(ledger["program_sha256"] == program_hash, "ledger.program_sha256 does not bind the supplied program")
    _validate_scope_compatibility(program, ledger)
    _require(1 <= candidate_count <= ledger["candidate_budget"], "candidate_count exceeds ledger candidate_budget")
    _require(candidate_count <= MAX_CANDIDATES, "candidate_count exceeds hard candidate limit")

    baseline_evaluation = evaluate_geometry(program)
    frozen = _frozen_snapshot(program, ledger["frozen_parts"])
    frozen_hashes = {part_id: canonical_sha256(part) for part_id, part in frozen.items()}
    frozen_assembly = _frozen_assembly_snapshot(program, ledger["frozen_parts"])
    frozen_assembly_hashes = {part_id: canonical_sha256(spec) for part_id, spec in frozen_assembly.items()}
    axes = [axis for axis in range(3) if axis != baseline_evaluation["coordinate_axes"]["longitudinal"]]
    curve_pool, parameter_pool = _parameter_pool(program, axes, ledger["allowed_scope"])
    combined_pool = curve_pool + parameter_pool
    _require(combined_pool, "ledger allowed_scope has no supported numeric search paths")
    rng = random.Random(seed)
    candidates: list[dict[str, Any]] = []
    seen_hashes = {program_hash}

    for ordinal in range(1, candidate_count + 1):
        candidate_program = copy.deepcopy(program)
        chosen: list[tuple[Any, ...]] = []
        if curve_pool:
            chosen.append(rng.choice(curve_pool))
        if parameter_pool:
            chosen.append(rng.choice(parameter_pool))
        if not chosen:
            chosen.append(rng.choice(combined_pool))
        extra_count = min(len(combined_pool), max(len(chosen), 2) + rng.randrange(5))
        shuffled = list(combined_pool)
        rng.shuffle(shuffled)
        for parameter in shuffled:
            if parameter not in chosen and len(chosen) < extra_count:
                chosen.append(parameter)
        deltas: list[dict[str, Any]] = []
        for parameter in chosen:
            old_value = _read_parameter(candidate_program, parameter)
            new_value = _write_parameter(candidate_program, parameter, old_value + _random_delta(rng, _parameter_delta_limit(parameter)))
            deltas.append({"path": _parameter_path(parameter), "before": round(old_value, 6), "after": new_value, "delta": round(new_value - old_value, 6)})

        candidate_hash = canonical_sha256(candidate_program)
        if candidate_hash in seen_hashes:
            # Quantized parameters make duplicates unlikely, but a deterministic
            # extra nudge keeps the output one-candidate-per-ordinal.
            parameter = combined_pool[(ordinal + len(candidates)) % len(combined_pool)]
            old_value = _read_parameter(candidate_program, parameter)
            nudge = min(0.0001, _parameter_delta_limit(parameter))
            new_value = _write_parameter(candidate_program, parameter, old_value + (nudge if ordinal % 2 else -nudge))
            deltas.append({"path": _parameter_path(parameter), "before": round(old_value, 6), "after": new_value, "delta": round(new_value - old_value, 6)})
            candidate_hash = canonical_sha256(candidate_program)
        candidate_program["canonical_sha256"] = candidate_hash
        _validate_program(candidate_program)
        changed_paths = _diff_paths(program, candidate_program)
        scope_gate = bool(changed_paths) and all(_allowed_change_path(path, program, ledger["allowed_scope"]) for path in changed_paths)
        candidate_parts = {part["part_id"]: part for part in candidate_program["parts"]}
        frozen_gate = all(candidate_parts[part_id] == snapshot for part_id, snapshot in frozen.items())
        candidate_assembly = _assembly_specs_by_part_id(candidate_program)
        frozen_assembly_gate = all(candidate_assembly.get(part_id) == snapshot for part_id, snapshot in frozen_assembly.items())
        evaluation = evaluate_geometry(candidate_program)
        gates = dict(evaluation["hard_gates"])
        gates["allowed_scope_only"] = scope_gate
        gates["frozen_parts_unchanged"] = frozen_gate
        gates["frozen_assembly_parts_unchanged"] = frozen_assembly_gate
        gates["program_outside_scope_unchanged"] = scope_gate
        # Preserve the historical receipt key while making it describe the
        # ledger scope rather than assuming blade-only search.
        gates["program_outside_blade_unchanged"] = scope_gate
        evaluation["hard_gates"] = gates
        evaluation["hard_gate_pass"] = all(gates.values())
        seen_hashes.add(candidate_hash)
        objective = _compare_objectives(baseline_evaluation, evaluation, ledger)
        candidates.append({
            "candidate_id": f"candidate-{ordinal:02d}-{candidate_hash[:12]}",
            "ordinal": ordinal,
            "program_sha256": candidate_hash,
            "program": candidate_program,
            "parameter_delta": deltas,
            "changed_paths": changed_paths,
            "hard_gates": gates,
            "hard_gate_pass": evaluation["hard_gate_pass"],
            "metrics": evaluation["metrics"],
            "metric_details": evaluation["metric_details"],
            "objective": objective,
            "objective_metrics": copy.deepcopy(objective["candidate"]),
            "search_scope": list(ledger["allowed_scope"]),
            "frozen_parts": list(ledger["frozen_parts"]),
            "frozen_part_hashes": copy.deepcopy(frozen_hashes),
            "frozen_assembly_part_hashes": copy.deepcopy(frozen_assembly_hashes),
        })

    return {
        "program_sha256": program_hash,
        "ledger_sha256": ledger_hash,
        "baseline": {
            "candidate_id": "baseline",
            "program_sha256": program_hash,
            "metrics": baseline_evaluation["metrics"],
            "metric_details": baseline_evaluation["metric_details"],
            "objective_metrics": {name: baseline_evaluation["metrics"].get(name) for name in ledger["objective_metrics"]},
            "hard_gates": baseline_evaluation["hard_gates"],
            "hard_gate_pass": baseline_evaluation["hard_gate_pass"],
        },
        "candidates": candidates,
        "search_axes": {
            "longitudinal": baseline_evaluation["coordinate_axes"]["longitudinal"],
            "mutable_curve_axes": axes,
            "mutable_parameter_count": len(combined_pool),
            "mutable_parameter_paths": [_parameter_path(parameter) for parameter in combined_pool],
        },
        "required_section_roles": list(ROLES),
        "frozen_part_hashes": frozen_hashes,
        "frozen_assembly_part_hashes": frozen_assembly_hashes,
    }


def _dominates(left: dict[str, Any], right: dict[str, Any], objective_names: list[str], details: dict[str, Any]) -> bool:
    if not left["hard_gate_pass"] or not right["hard_gate_pass"]:
        return False
    left_values = left["metrics"]
    right_values = right["metrics"]
    no_worse = True
    strictly_better = False
    comparable = False
    for name in objective_names:
        left_value = left_values.get(name)
        right_value = right_values.get(name)
        if left_value is None or right_value is None:
            continue
        comparable = True
        direction = _metric_direction(name, details)
        if direction == "maximize":
            if left_value < right_value - 1e-12:
                no_worse = False
            if left_value > right_value + 1e-12:
                strictly_better = True
        else:
            if left_value > right_value + 1e-12:
                no_worse = False
            if left_value < right_value - 1e-12:
                strictly_better = True
    return comparable and no_worse and strictly_better


def pareto_front(candidates: list[dict[str, Any]], ledger: dict[str, Any], metric_details: dict[str, Any]) -> tuple[list[str], list[str]]:
    objective_names = list(ledger["objective_metrics"])
    supported = [name for name in objective_names if any(candidate["metrics"].get(name) is not None for candidate in candidates)]
    fallback_used = False
    if not supported:
        supported = ["thickness-continuity", "normal-continuity", "tip-convergence", "curve-smoothness"]
        fallback_used = True
    front: list[str] = []
    eligible = [candidate for candidate in candidates if candidate["hard_gate_pass"]]
    for candidate in eligible:
        if not any(_dominates(other, candidate, supported, metric_details) for other in eligible if other is not candidate):
            front.append(candidate["candidate_id"])
    return sorted(front, key=lambda candidate_id: next(item["ordinal"] for item in candidates if item["candidate_id"] == candidate_id)), supported if not fallback_used else [f"fallback:{name}" for name in supported]


def _evaluation_evidence(candidate: dict[str, Any], parent_ledger_sha256: str) -> str:
    payload = {
        "schema_version": "KnifeGeometryEvaluationEvidence@1",
        "parent_ledger_sha256": parent_ledger_sha256,
        "candidate_id": candidate["candidate_id"],
        "program_sha256": candidate["program_sha256"],
        "changed_paths": candidate["changed_paths"],
        "hard_gates": candidate["hard_gates"],
        "metrics": candidate["metrics"],
        "render_status": "NOT_RUN",
    }
    return canonical_sha256(payload)


def _build_successor_ledger(parent: dict[str, Any], parent_hash: str, candidate: dict[str, Any], evidence_hash: str) -> dict[str, Any]:
    successor = copy.deepcopy(parent)
    successor["revision"] = parent["revision"] + 1
    successor["parent_ledger_sha256"] = parent_hash
    successor["program_sha256"] = candidate["program_sha256"]
    successor["evidence_sha256"] = list(dict.fromkeys(list(parent["evidence_sha256"]) + [evidence_hash]))
    successor["status"] = "active"
    successor["canonical_sha256"] = ""
    successor["canonical_sha256"] = canonical_sha256(successor)
    _validate_ledger(successor)
    _require(successor["allowed_scope"] == parent["allowed_scope"], "successor changed allowed_scope")
    _require(successor["frozen_parts"] == parent["frozen_parts"], "successor changed frozen_parts")
    return successor


def build_search_receipt(program: dict[str, Any], ledger: dict[str, Any], seed: int, candidate_count: int) -> dict[str, Any]:
    result = generate_candidates(program, ledger, seed, candidate_count)
    parent_hash = result["ledger_sha256"]
    all_details = result["baseline"]["metric_details"]
    front, pareto_objectives = pareto_front(result["candidates"], ledger, all_details)
    front_set = set(front)
    candidate_by_id = {candidate["candidate_id"]: candidate for candidate in result["candidates"]}
    for candidate in result["candidates"]:
        candidate["pareto"] = candidate["candidate_id"] in front_set
        candidate["decision"] = "PARETO_ALTERNATIVE" if candidate["pareto"] else ("REJECTED_HARD_GATE" if not candidate["hard_gate_pass"] else "REJECTED_DOMINATED")

    proposals: list[dict[str, Any]] = []
    for candidate_id in front:
        candidate = candidate_by_id[candidate_id]
        evidence_hash = _evaluation_evidence(candidate, parent_hash)
        successor = _build_successor_ledger(ledger, parent_hash, candidate, evidence_hash)
        # A proposal is never accepted here.  Even the narrower geometry-only
        # follow-up flag is fail-closed: all named regression limits must be
        # computable and true before it can be considered by a later reviewer.
        promotable = bool(candidate["hard_gate_pass"] and candidate["objective"]["target_improved"] and candidate["objective"]["regression_ok"] is True)
        proposals.append({
            "proposal_id": f"successor-{candidate_id}",
            "proposal_kind": "immutable-candidate-successor",
            "proposal_status": "REVIEW_ONLY",
            "promotable_by_geometry_objectives": promotable,
            "parent_ledger_sha256": parent_hash,
            "parent_program_sha256": result["program_sha256"],
            "successor_ledger": successor,
            "candidate_program": copy.deepcopy(candidate["program"]),
            "candidate_metrics": copy.deepcopy(candidate["metrics"]),
            "candidate_hard_gates": copy.deepcopy(candidate["hard_gates"]),
            "evaluation_evidence_sha256": evidence_hash,
            "allowed_scope": list(ledger["allowed_scope"]),
            "frozen_parts": list(ledger["frozen_parts"]),
            "frozen_part_hashes": copy.deepcopy(candidate["frozen_part_hashes"]),
            "render_status": "NOT_RUN",
            "visual_status": "NOT_RUN",
            "human_status": "NOT_RUN",
            "engine_status": "NOT_RUN",
            "commercial_status": "NOT_RUN",
            "note": "Geometry-only successor proposal; no render, visual review, engine validation, approval, or commercial acceptance was performed.",
        })

    selected: str | None = None
    if front:
        def selection_key(candidate: dict[str, Any]) -> tuple[Any, ...]:
            improvements = [value for value in candidate["objective"]["improvements"].values() if value is not None]
            threshold_hits = sum(value >= ledger["minimum_improvement"] for value in improvements)
            positive_total = sum(max(0.0, value) for value in improvements)
            drift = sum(abs(float(delta["delta"])) for delta in candidate["parameter_delta"])
            return (-threshold_hits, -round(positive_total, 9), round(drift, 9), candidate["ordinal"])
        selected = min((candidate_by_id[candidate_id] for candidate_id in front), key=selection_key)["candidate_id"]
        for candidate in result["candidates"]:
            if candidate["candidate_id"] == selected:
                candidate["decision"] = "SELECTED_PARETO_SUCCESSOR_PROPOSAL"

    receipt: dict[str, Any] = {
        "schema_version": "KnifeCandidateSearchReceipt@1",
        "route": "weaponry-threejs-knife-studio@0.1.0",
        "search_status": "PROPOSALS_READY" if proposals else "NO_ELIGIBLE_CANDIDATE",
        "search": {
            "seed": seed,
            "candidate_budget": ledger["candidate_budget"],
            "candidate_count": candidate_count,
            "parameter_space": "ledger-scoped-blade-or-assembly-numeric@2",
            "mutable_curve_axes": result["search_axes"]["mutable_curve_axes"],
            "mutable_parameter_count": result["search_axes"]["mutable_parameter_count"],
            "mutable_parameter_paths": result["search_axes"]["mutable_parameter_paths"],
            "required_section_roles": result["required_section_roles"],
            "render_used": False,
            "render_status": "NOT_RUN",
        },
        "source": {
            "program_sha256": result["program_sha256"],
            "ledger_sha256": parent_hash,
            "baseline_candidate_sha256": ledger["baseline_candidate_sha256"],
            "allowed_scope": list(ledger["allowed_scope"]),
            "frozen_parts": list(ledger["frozen_parts"]),
            "frozen_assembly_part_hashes": result["frozen_assembly_part_hashes"],
        },
        "baseline": result["baseline"],
        "candidates": result["candidates"],
        "pareto_front": front,
        "pareto_objectives": pareto_objectives,
        "selected_candidate_id": selected,
        "successor_proposals": proposals,
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
    receipt["canonical_sha256"] = canonical_sha256(receipt)
    return receipt


def build_smoke_ledger(program: dict[str, Any]) -> dict[str, Any]:
    program_hash = _validate_program(program)
    ledger: dict[str, Any] = {
        "schema_version": "KnifeObjectiveLedger@1",
        "ledger_id": "dragonfang-blade-search-ledger",
        "revision": 0,
        "parent_ledger_sha256": None,
        "program_sha256": program_hash,
        "baseline_candidate_sha256": program_hash,
        "stage": "form",
        "allowed_scope": ["blade-body", "cutting-edge"],
        "frozen_parts": ["guard", "grip", "pommel"],
        "hypothesis": "Small bounded changes to blade curves and four semantic sections can improve intrinsic continuity without touching frozen assembly parts.",
        "objective_metrics": ["thickness-continuity", "normal-continuity"],
        "regression_limits": ["thickness-continuity", "normal-continuity"],
        "candidate_budget": DEFAULT_CANDIDATE_COUNT,
        "minimum_improvement": 0.001,
        "plateau_limit": 2,
        "evidence_sha256": [program_hash],
        "status": "active",
        "canonical_sha256": "",
    }
    ledger["canonical_sha256"] = canonical_sha256(ledger)
    _validate_ledger(ledger)
    return ledger


def run_smoke() -> dict[str, Any]:
    program = load_json(DEFAULT_PROGRAM)
    before = canonical_bytes(program)
    ledger = build_smoke_ledger(program)
    first = build_search_receipt(program, ledger, DEFAULT_SEED, DEFAULT_CANDIDATE_COUNT)
    second = build_search_receipt(program, ledger, DEFAULT_SEED, DEFAULT_CANDIDATE_COUNT)
    _require(canonical_sha256(first) == canonical_sha256(second), "smoke search is not deterministic")
    _require(canonical_bytes(program) == before, "smoke mutated the source program")
    _require(first["baseline"]["hard_gate_pass"], "Dragonfang fixture baseline geometry gates did not pass")
    _require(first["source"]["allowed_scope"] == ledger["allowed_scope"], "smoke allowed_scope drifted")
    _require(first["source"]["frozen_parts"] == ledger["frozen_parts"], "smoke frozen_parts drifted")
    _require(first["candidates"], "smoke did not generate candidates")
    for proposal in first["successor_proposals"]:
        _require(proposal["parent_ledger_sha256"] == first["source"]["ledger_sha256"], "successor parent ledger binding drifted")
        _require(proposal["allowed_scope"] == ledger["allowed_scope"], "successor allowed_scope drifted")
        _require(proposal["frozen_parts"] == ledger["frozen_parts"], "successor frozen_parts drifted")
        _require(proposal["visual_status"] == "NOT_RUN" and proposal["commercial_status"] == "NOT_RUN", "smoke emitted an acceptance status")
    first["smoke_status"] = "PASS"
    first["canonical_sha256"] = canonical_sha256(first)
    return first


def _write_output(value: dict[str, Any], output: Path | None, pretty: bool) -> None:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2 if pretty else None, separators=None if pretty else (",", ":")) + "\n"
    if output is None:
        print(encoded, end="")
        return
    try:
        output.write_text(encoded, encoding="utf-8")
    except OSError as exc:
        raise InputError(f"cannot write {output}: {exc}") from exc
    print(encoded, end="")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Deterministic no-render ledger-scoped KnifeSceneProgram candidate search")
    parser.add_argument("--smoke", action="store_true", help="run the self-contained Dragonfang fixture smoke")
    parser.add_argument("--program", type=Path, default=DEFAULT_PROGRAM, help="closed KnifeSceneProgram@1 JSON (default: Dragonfang fixture)")
    parser.add_argument("--ledger", "--objective-ledger", dest="ledger", type=Path, help="closed KnifeObjectiveLedger@1 JSON; omitted for an in-memory smoke ledger")
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED, help="fixed deterministic seed")
    parser.add_argument("--candidate-count", "--count", dest="candidate_count", type=int, help="bounded candidate count (must be <= ledger budget)")
    parser.add_argument("--output", type=Path, help="optional output JSON path; inputs are never overwritten")
    parser.add_argument("--pretty", action="store_true", help="pretty-print the receipt")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.smoke:
            receipt = run_smoke()
        else:
            program = load_json(args.program)
            program_hash = _validate_program(program)
            ledger = load_json(args.ledger) if args.ledger else build_smoke_ledger(program)
            _validate_ledger(ledger)
            _require(ledger["program_sha256"] == program_hash, "ledger.program_sha256 does not bind the supplied program")
            count = args.candidate_count if args.candidate_count is not None else min(ledger["candidate_budget"], DEFAULT_CANDIDATE_COUNT)
            receipt = build_search_receipt(program, ledger, args.seed, count)
        if args.output is not None:
            input_paths = {args.program.resolve()}
            if args.ledger is not None:
                input_paths.add(args.ledger.resolve())
            _require(args.output.resolve() not in input_paths, "output path must not overwrite a program or ledger input")
        _write_output(receipt, args.output, args.pretty)
        return 0
    except (InputError, OSError, ValueError) as exc:
        print(f"search_candidates: ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
