#!/usr/bin/env python3
"""Weaponry's fixed Blender knife prototype worker.

This is an isolated, one-shot prototype provider.  It is deliberately not a
general Blender bridge: the only accepted operation is the closed knife
High/Low/UV/Bake recipe below.  The Runtime launcher owns the scratch
directory and stages ``input/source.glb`` there; the JSON job cannot choose a
different file, script, addon, URL, executable, or output location.

The worker produces temporary observations only.  It has no SQLite/CAS
access, does not create candidates or versions, and never advances a stage.
The Runtime must independently inspect and adopt the returned bytes.

Blender is imported lazily so that this file can be syntax-checked by the
repository's normal Python tooling without Blender installed.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import re
import struct
import sys
import zipfile
from pathlib import Path
from typing import Any


WORKER_VERSION = "0.1.0"
# These markers are the closed wire defined by apps/blender-worker/src/knife.rs.
# Keep the entrypoint aligned with the Rust launcher; a visually similar but
# differently named envelope would fail before Blender output is read.
PROTOCOL = "weaponry-fixed-worker-stdio-json@1"
REQUEST_SCHEMA = "WeaponryBlenderKnifeWorkerRequest@1"
RESPONSE_SCHEMA = "WeaponryBlenderKnifeWorkerResponse@1"
RESULT_SCHEMA = "WeaponryBlenderKnifeWorkerResult@1"
WORKER_ID = "weaponry-blender-knife-worker@1"
OPERATION = "knife_high_low_uv_bake@1"
BLENDER_VERSION = "5.2.1"
BLENDER_REVISION = "9e2066aef7ef"
RECIPE_ID = "weaponry.knife.blender.high-low-uv-bake@1"
POLICY = "fixed-built-in-bevel-weighted-normal-decimate-smart-uv-cycles-bake@1"
INPUT_RELATIVE_PATH = "input/source.glb"
OUTPUT_DIRECTORY = "output"
MAX_REQUEST_BYTES = 2 * 1024 * 1024
MAX_STDOUT_BYTES = 64 * 1024
MAX_INPUT_BYTES = 96 * 1024 * 1024
MAX_OUTPUT_BYTES = 200 * 1024 * 1024
MAX_TRIANGLES = 250_000
MAX_TEXTURE_SIZE = 512
MAX_OBJECTS = 128
UV_QUANTIZATION_GRID_DENOMINATOR = 65_536
GEOMETRY_QUANTIZATION_DECIMALS = 6
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$")

FIXED_RECIPE = {
    "recipe_id": RECIPE_ID,
    "policy": POLICY,
    "source": "staged_glb_only",
    "high": ["bevel", "weighted_normal"],
    "low": ["decimate", "weighted_normal", "smart_project_uv"],
    "bake": ["tangent_normal", "ambient_occlusion"],
    "renderer": "cycles-cpu",
    "texture_size": MAX_TEXTURE_SIZE,
    "margin_texels": 8,
    "cage_extrusion_m": 0.02,
    "randomness": "disabled",
}

# The wire recipe above is intentionally unchanged.  Its hash is embedded in
# the closed Rust request/response contract and in already-issued sample jobs.
# This profile describes the stronger implementation behind that same wire so
# a replay of an old request receives the improved bounded recipe without a
# schema migration.  It is emitted in the temporary worker manifest only; it
# is not caller-controlled input and never becomes Runtime truth by itself.
ENHANCED_RECIPE_PROFILE = {
    "profile_id": "weaponry.knife.blender.high-low-uv-bake-enhanced@1",
    "wire_compatibility": RECIPE_ID,
    "part_resolution": "stable-object-name-role@1",
    "high": [
        "part-local-bevel",
        "bounded-subdivision",
        "edge-crease",
        "bounded-smooth",
        "weighted-normal",
    ],
    "low": [
        "part-local-decimate",
        "part-local-bevel",
        "edge-crease",
        "bounded-smooth",
        "weighted-normal",
        "smart-project-uv",
    ],
    "cage": "independent-low-derived-normal-offset@1",
    "surface_signals": [
        "curvature-adjacent-normal-proxy",
        "thickness-bounded-min-extent-proxy",
        "material-id-glb-material-index-and-color-attribute",
    ],
    "determinism": "single-thread-and-geometry-normal-surface-quantization-1e-6-uv-grid-1over65536@3",
}

# Role policy is selected only from the imported object's stable name.  The
# values are deliberately small and fixed; a request cannot tune a modifier,
# supply a vertex group, or inject a Blender expression.  Keep the fallback
# conservative because source meshes may be authored with arbitrary names.
ROLE_SURFACE_POLICY: dict[str, dict[str, Any]] = {
    "blade": {
        "high_bevel_ratio": 0.012,
        "low_bevel_ratio": 0.006,
        "bevel_cap_m": 0.004,
        "bevel_segments": 3,
        "angle_limit_rad": 0.48,
        "subdivision": True,
        "smooth_factor": 0.035,
        "crease": 0.72,
        "decimate_ratio": 0.72,
    },
    "guard": {
        "high_bevel_ratio": 0.018,
        "low_bevel_ratio": 0.009,
        "bevel_cap_m": 0.006,
        "bevel_segments": 3,
        "angle_limit_rad": 0.52,
        "subdivision": True,
        "smooth_factor": 0.045,
        "crease": 0.64,
        "decimate_ratio": 0.70,
    },
    "handle": {
        "high_bevel_ratio": 0.022,
        "low_bevel_ratio": 0.011,
        "bevel_cap_m": 0.008,
        "bevel_segments": 4,
        "angle_limit_rad": 0.60,
        "subdivision": True,
        "smooth_factor": 0.075,
        "crease": 0.42,
        "decimate_ratio": 0.68,
    },
    "hardware": {
        "high_bevel_ratio": 0.010,
        "low_bevel_ratio": 0.005,
        "bevel_cap_m": 0.003,
        "bevel_segments": 2,
        "angle_limit_rad": 0.62,
        "subdivision": False,
        "smooth_factor": 0.02,
        "crease": 0.86,
        "decimate_ratio": 0.76,
    },
    "generic": {
        "high_bevel_ratio": 0.015,
        "low_bevel_ratio": 0.007,
        "bevel_cap_m": 0.005,
        "bevel_segments": 3,
        "angle_limit_rad": 0.56,
        "subdivision": False,
        "smooth_factor": 0.03,
        "crease": 0.60,
        "decimate_ratio": 0.72,
    },
}

MAX_SUBDIVISION_TRIANGLES_PER_PART = 2_000
MAX_SUBDIVISION_SOURCE_TRIANGLES = 20_000
MAX_MATERIAL_IDS_PER_PART = 64
SURFACE_SIGNAL_NAMES = (
    "WPN_Curvature",
    "WPN_Thickness",
    "WPN_MaterialID",
)
SURFACE_COLOR_ATTRIBUTE = "WPN_SurfaceSignals"

# This lock covers the fixed host's Python-side dependencies.  There are no
# dynamic Python packages or add-ons: the worker uses Blender built-ins only.
DEPENDENCY_LOCK = {
    "blender_version": BLENDER_VERSION,
    "blender_revision": BLENDER_REVISION,
    "python_dependencies": [],
    "addons": [],
    "network": False,
}


class WorkerFailure(Exception):
    """A safe, stable failure that may be returned to the Runtime."""

    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code
        self.message = message


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def canonical_sha256(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def dependency_lock_sha256() -> str:
    return canonical_sha256(DEPENDENCY_LOCK)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def worker_entrypoint_sha256() -> str:
    """Return the digest of this fixed entrypoint, never a caller path."""
    return sha256_file(Path(__file__).resolve())


def normalize_glb_attributes(source_path: Path, normalized_path: Path) -> None:
    """Strip non-standard vertex attributes before invoking Blender's importer.

    Weaponry's Three.js source GLB carries many product-authored scalar/vector
    attributes (curvature, material masks, section indices, and IDs). The
    fixed Blender glTF importer can reject this particular mixed
    custom-attribute layout while importing an otherwise valid glTF 2.0 asset.
    The prototype worker does not discard the source bytes: it creates a
    scratch-only import view containing the standard POSITION/NORMAL/
    TEXCOORD_0 accessors. The original hash remains the request identity and
    the Runtime must perform its own semantic readback. This is a compatibility
    normalization, not a silent source mutation or a second product truth.
    """
    data = source_path.read_bytes()
    require(len(data) >= 20 and data[:4] == b"glTF", "WORKER_INPUT_INVALID", "staged file is not a glTF binary")
    version, declared_length = struct.unpack_from("<II", data, 4)
    require(version == 2 and declared_length == len(data), "WORKER_INPUT_INVALID", "staged glTF header is invalid")
    cursor = 12
    json_value: dict[str, Any] | None = None
    bin_chunk: bytes | None = None
    other_chunks: list[tuple[int, bytes]] = []
    while cursor + 8 <= len(data):
        chunk_length, chunk_type = struct.unpack_from("<II", data, cursor)
        cursor += 8
        require(cursor + chunk_length <= len(data), "WORKER_INPUT_INVALID", "staged glTF chunk exceeds the file")
        chunk = data[cursor : cursor + chunk_length]
        cursor += chunk_length
        if chunk_type == 0x4E4F534A:
            require(json_value is None, "WORKER_INPUT_INVALID", "staged glTF has duplicate JSON chunks")
            try:
                json_value = json.loads(chunk.rstrip(b" \t\r\n\x00").decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise WorkerFailure("WORKER_INPUT_INVALID", "staged glTF JSON is invalid") from error
        elif chunk_type == 0x004E4942:
            require(bin_chunk is None, "WORKER_INPUT_INVALID", "staged glTF has duplicate binary chunks")
            bin_chunk = chunk
        else:
            other_chunks.append((chunk_type, chunk))
    require(isinstance(json_value, dict) and bin_chunk is not None, "WORKER_INPUT_INVALID", "staged glTF is missing required chunks")
    meshes = json_value.get("meshes")
    require(isinstance(meshes, list), "WORKER_INPUT_INVALID", "staged glTF meshes are invalid")
    allowed_attributes = {"POSITION", "NORMAL", "TEXCOORD_0"}
    for mesh in meshes:
        require(isinstance(mesh, dict) and isinstance(mesh.get("primitives"), list), "WORKER_INPUT_INVALID", "staged glTF mesh primitive is invalid")
        for primitive in mesh["primitives"]:
            require(isinstance(primitive, dict) and isinstance(primitive.get("attributes"), dict), "WORKER_INPUT_INVALID", "staged glTF primitive attributes are invalid")
            primitive["attributes"] = {
                name: accessor
                for name, accessor in primitive["attributes"].items()
                if name in allowed_attributes
            }
            require("POSITION" in primitive["attributes"], "WORKER_INPUT_INVALID", "staged glTF primitive has no POSITION")
    json_bytes = canonical_bytes(json_value)
    json_bytes += b" " * ((4 - (len(json_bytes) % 4)) % 4)
    chunks: list[bytes] = [struct.pack("<II", len(json_bytes), 0x4E4F534A) + json_bytes]
    chunks.extend(struct.pack("<II", len(chunk), chunk_type) + chunk for chunk_type, chunk in other_chunks)
    chunks.append(struct.pack("<II", len(bin_chunk), 0x004E4942) + bin_chunk)
    rebuilt = b"glTF" + struct.pack("<II", 2, 12 + sum(len(chunk) for chunk in chunks)) + b"".join(chunks)
    normalized_path.write_bytes(rebuilt)


def json_file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition: bool, code: str, message: str) -> None:
    if not condition:
        raise WorkerFailure(code, message)


def require_exact_keys(value: Any, expected: set[str], label: str) -> None:
    require(isinstance(value, dict), "WORKER_SCHEMA_INVALID", f"{label} must be an object")
    actual = set(value)
    require(
        actual == expected,
        "WORKER_SCHEMA_INVALID",
        f"{label} fields are not closed",
    )


def require_string(value: Any, label: str, *, pattern: re.Pattern[str] | None = None) -> str:
    require(isinstance(value, str) and value != "", "WORKER_SCHEMA_INVALID", f"{label} must be a string")
    if pattern is not None:
        require(pattern.fullmatch(value) is not None, "WORKER_SCHEMA_INVALID", f"{label} is invalid")
    return value


def require_sha(value: Any, label: str) -> str:
    text = require_string(value, label)
    require(SHA256_RE.fullmatch(text) is not None, "WORKER_SCHEMA_INVALID", f"{label} is not sha256")
    return text


def require_uint(value: Any, label: str, maximum: int) -> int:
    require(isinstance(value, int) and not isinstance(value, bool), "WORKER_SCHEMA_INVALID", f"{label} must be an integer")
    require(0 <= value <= maximum, "WORKER_BUDGET_EXCEEDED", f"{label} exceeds the fixed ceiling")
    return value


def fixed_recipe_sha256() -> str:
    return canonical_sha256(FIXED_RECIPE)


def parse_job(raw: Any, scratch_root: Path) -> dict[str, Any]:
    request_fields = {
        "schema_version",
        "operation",
        "request_id",
        "project_id",
        "candidate_id",
        "input_glb",
        "recipe_id",
        "recipe_sha256",
        "budgets",
        "policies",
        "canonical_sha256",
    }
    require_exact_keys(raw, request_fields, "request")
    require(raw["schema_version"] == REQUEST_SCHEMA, "WORKER_SCHEMA_INVALID", "request schema version is unsupported")
    require(raw["operation"] == OPERATION, "WORKER_OPERATION_NOT_ALLOWED", "operation is not allowlisted")
    require_string(raw["request_id"], "request_id", pattern=ID_RE)
    require_string(raw["project_id"], "project_id", pattern=ID_RE)
    require_string(raw["candidate_id"], "candidate_id", pattern=ID_RE)
    require(raw["recipe_id"] == RECIPE_ID, "WORKER_RECIPE_NOT_ALLOWED", "recipe is not allowlisted")
    require(raw["recipe_sha256"] == fixed_recipe_sha256(), "WORKER_RECIPE_DRIFT", "recipe hash is not the fixed recipe hash")

    input_fields = {"kind", "relative_path", "sha256", "byte_size", "mime"}
    require_exact_keys(raw["input_glb"], input_fields, "input_glb")
    require(raw["input_glb"]["kind"] == "authoring_mesh_glb", "WORKER_INPUT_KIND_UNSUPPORTED", "input kind is unsupported")
    require(raw["input_glb"]["relative_path"] == INPUT_RELATIVE_PATH, "WORKER_INPUT_PATH_NOT_ALLOWED", "input path is not the fixed staged path")
    input_sha256 = require_sha(raw["input_glb"]["sha256"], "input_glb.sha256")
    input_size = require_uint(raw["input_glb"]["byte_size"], "input_glb.byte_size", MAX_INPUT_BYTES)
    require(raw["input_glb"]["mime"] == "model/gltf-binary", "WORKER_INPUT_KIND_UNSUPPORTED", "input mime is unsupported")

    budget_fields = {"max_runtime_ms", "max_memory_bytes", "max_input_bytes", "max_output_bytes", "max_triangles", "texture_size"}
    require_exact_keys(raw["budgets"], budget_fields, "budgets")
    require_uint(raw["budgets"]["max_runtime_ms"], "budgets.max_runtime_ms", 120_000)
    require_uint(raw["budgets"]["max_memory_bytes"], "budgets.max_memory_bytes", 512 * 1024 * 1024)
    require_uint(raw["budgets"]["max_input_bytes"], "budgets.max_input_bytes", MAX_INPUT_BYTES)
    require_uint(raw["budgets"]["max_output_bytes"], "budgets.max_output_bytes", MAX_OUTPUT_BYTES)
    require_uint(raw["budgets"]["max_triangles"], "budgets.max_triangles", MAX_TRIANGLES)
    require(raw["budgets"]["texture_size"] == MAX_TEXTURE_SIZE, "WORKER_BUDGET_UNSUPPORTED", "only the fixed 512px bake recipe is available")
    require(raw["budgets"]["max_input_bytes"] >= input_size, "WORKER_BUDGET_EXCEEDED", "input exceeds the request input budget")

    policy_fields = {"network_policy", "filesystem_policy", "script_policy", "output_policy"}
    require_exact_keys(raw["policies"], policy_fields, "policies")
    require(raw["policies"] == {
        "network_policy": "disabled",
        "filesystem_policy": "runtime_scratch_only",
        "script_policy": "frozen_bundle_only",
        "output_policy": "temporary_observation_runtime_adoption",
    }, "WORKER_POLICY_NOT_ALLOWED", "request policies do not match the fixed worker policy")

    supplied_canonical = require_sha(raw["canonical_sha256"], "canonical_sha256")
    preimage = copy.deepcopy(raw)
    preimage["canonical_sha256"] = ""
    require(canonical_sha256(preimage) == supplied_canonical, "WORKER_CANONICAL_HASH_MISMATCH", "request canonical hash does not match")

    staged = (scratch_root / INPUT_RELATIVE_PATH).resolve()
    require(scratch_root == staged or scratch_root in staged.parents, "WORKER_INPUT_PATH_NOT_ALLOWED", "staged input escapes scratch")
    require(staged.is_file(), "WORKER_INPUT_MISSING", "staged input is missing")
    actual_size = staged.stat().st_size
    require(actual_size == input_size, "WORKER_INPUT_HASH_MISMATCH", "staged input byte size drifted")
    require(actual_size <= MAX_INPUT_BYTES, "WORKER_INPUT_TOO_LARGE", "staged input exceeds fixed input ceiling")
    require(sha256_file(staged) == input_sha256, "WORKER_INPUT_HASH_MISMATCH", "staged input hash drifted")
    return raw


def safe_name(value: str, fallback: str) -> str:
    text = re.sub(r"[^A-Za-z0-9_.-]+", "_", value).strip("._-")
    return (text or fallback)[:80]


def normalized_object_name(value: str) -> str:
    """Return a bounded comparison form for stable semantic role matching."""
    return re.sub(r"[^a-z0-9]+", "_", value.lower()).strip("_")[:128]


def semantic_role_for_name(value: str) -> str:
    """Resolve a source object's role from its stable, imported name.

    This is intentionally a tiny allowlisted vocabulary.  It is not a
    natural-language or script surface and does not inspect request text.
    Matching the more specific guard/handle/hardware terms first prevents
    names such as ``blade_guard`` from receiving the wrong policy.
    """
    name = normalized_object_name(value)
    role_tokens = (
        ("guard", ("guard", "crossguard", "quillon")),
        ("handle", ("handle", "grip", "pommel", "hilt", "wrap")),
        ("hardware", ("screw", "bolt", "pin", "fastener", "washer", "rivet")),
        (
            "blade",
            (
                "blade",
                "edge",
                "tip",
                "spine",
                "fuller",
                "choil",
                "ricasso",
                "point",
            ),
        ),
    )
    for role, tokens in role_tokens:
        if any(token in name for token in tokens):
            return role
    return "generic"


def stable_part_id(source_name: str, index: int, used: set[str]) -> str:
    """Create a deterministic, collision-safe semantic Part id."""
    base = safe_name(source_name, f"part_{index:03d}").lower()
    base = base[:72] or f"part_{index:03d}"
    candidate = base
    suffix = 1
    while candidate in used:
        candidate = f"{base[: max(1, 72 - len(str(suffix)) - 1)]}-{suffix}"
        suffix += 1
    used.add(candidate)
    return candidate


def role_policy(role: str) -> dict[str, Any]:
    return ROLE_SURFACE_POLICY.get(role, ROLE_SURFACE_POLICY["generic"])


def object_triangles(obj: Any) -> int:
    return sum(max(0, len(poly.vertices) - 2) for poly in obj.data.polygons)


def mesh_topology_sha256(obj: Any) -> str:
    """Hash only topology, excluding coordinates and Blender datablock names."""
    mesh = obj.data
    topology = {
        "vertex_count": len(mesh.vertices),
        "edge_count": len(mesh.edges),
        "edges": [list(edge.vertices) for edge in mesh.edges],
        "polygon_count": len(mesh.polygons),
        "polygons": [list(poly.vertices) for poly in mesh.polygons],
    }
    return canonical_sha256(topology)


def select_only(bpy: Any, objects: list[Any], active: Any | None = None) -> None:
    for obj in bpy.context.view_layer.objects:
        obj.select_set(False)
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = active if active is not None else (objects[0] if objects else None)


def apply_modifier(bpy: Any, obj: Any, modifier_name: str) -> None:
    select_only(bpy, [obj], obj)
    try:
        bpy.ops.object.modifier_apply(modifier=modifier_name)
    except Exception as error:  # Blender's operator errors are not stable enough to expose.
        raise WorkerFailure("BLENDER_MODIFIER_FAILED", f"fixed modifier {modifier_name} failed") from error


def apply_optional_modifier(bpy: Any, obj: Any, modifier: Any) -> bool:
    """Apply an optional bounded modifier without weakening mandatory gates."""
    modifier_name = modifier.name
    try:
        apply_modifier(bpy, obj, modifier_name)
        return True
    except WorkerFailure:
        # Optional smoothing/subdivision is a quality refinement.  A source
        # mesh that is not suitable for it should still receive the fixed
        # bevel/decimate/UV/bake path.  Remove the unapplied modifier so it can
        # never leak into an exported observation.
        try:
            leftover = obj.modifiers.get(modifier_name)
            if leftover is not None:
                obj.modifiers.remove(leftover)
        except Exception:
            pass
        return False


def set_edge_crease(mesh: Any, amount: float) -> bool:
    """Set a fixed edge crease attribute when Blender exposes one.

    Blender 4+ stores crease weights as an EDGE-domain attribute.  The
    fallback simply records that the pass was requested; the subsequent
    smooth/weighted-normal passes remain valid on older hosts where the
    attribute API differs.  No caller can select an attribute name or value.
    """
    bounded = max(0.0, min(1.0, float(amount)))
    try:
        attributes = mesh.attributes
        attribute = attributes.get("crease_edge")
        if attribute is None:
            attribute = attributes.get(".edge_crease")
        if attribute is None:
            attribute = attributes.new(name="crease_edge", type="FLOAT", domain="EDGE")
        if getattr(attribute, "domain", None) != "EDGE":
            return False
        for datum in attribute.data:
            datum.value = bounded
        return True
    except Exception:
        return False


def apply_bounded_subdivision(
    bpy: Any,
    obj: Any,
    role: str,
    source_triangles: int,
    total_source_triangles: int,
    high_triangles_so_far: int,
) -> bool:
    """Apply at most one level of local subdivision to eligible curved Parts."""
    policy = role_policy(role)
    if not policy["subdivision"]:
        return False
    if source_triangles <= 0 or source_triangles > MAX_SUBDIVISION_TRIANGLES_PER_PART:
        return False
    if total_source_triangles > MAX_SUBDIVISION_SOURCE_TRIANGLES:
        return False
    # A single level can produce up to four times as many triangles.  Keep the
    # decision closed and deterministic before asking Blender to evaluate it.
    if high_triangles_so_far + source_triangles * 4 > MAX_TRIANGLES:
        return False
    polygons = obj.data.polygons
    if not polygons or any(len(poly.vertices) not in (3, 4) for poly in polygons):
        return False
    modifier = obj.modifiers.new(name="WPN_Subdivision", type="SUBSURF")
    modifier.levels = 1
    modifier.render_levels = 1
    # Imported GLBs are commonly triangulated.  SIMPLE keeps the silhouette
    # stable in that case; quad Parts get the real Catmull-Clark surface pass.
    modifier.subdivision_type = "CATMULL_CLARK" if all(len(poly.vertices) == 4 for poly in polygons) else "SIMPLE"
    if hasattr(modifier, "show_only_control_edges"):
        modifier.show_only_control_edges = True
    return apply_optional_modifier(bpy, obj, modifier)


def apply_bounded_surface_pass(bpy: Any, obj: Any, role: str) -> dict[str, Any]:
    """Apply fixed crease + smooth semantics to one resolved Part."""
    policy = role_policy(role)
    crease_applied = set_edge_crease(obj.data, policy["crease"])
    smooth_polygons = 0
    for polygon in obj.data.polygons:
        polygon.use_smooth = True
        smooth_polygons += 1
    smooth_modifier = obj.modifiers.new(name="WPN_SurfaceSmooth", type="SMOOTH")
    smooth_modifier.factor = float(policy["smooth_factor"])
    smooth_modifier.iterations = 1
    smooth_applied = apply_optional_modifier(bpy, obj, smooth_modifier)
    return {
        "crease": crease_applied,
        "smooth": smooth_applied,
        "smooth_polygon_count": smooth_polygons,
        "policy": {
            "crease": policy["crease"],
            "smooth_factor": policy["smooth_factor"],
            "iterations": 1,
        },
    }


def apply_scale(bpy: Any, obj: Any) -> None:
    select_only(bpy, [obj], obj)
    try:
        bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    except Exception as error:
        raise WorkerFailure("BLENDER_TRANSFORM_FAILED", "source transform application failed") from error


def smart_project_uv(bpy: Any, obj: Any) -> None:
    select_only(bpy, [obj], obj)
    try:
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.uv.smart_project(
            angle_limit=1.151917,
            island_margin=0.03,
            area_weight=0.0,
            correct_aspect=True,
            scale_to_bounds=False,
        )
        bpy.ops.object.mode_set(mode="OBJECT")
    except Exception as error:
        try:
            bpy.ops.object.mode_set(mode="OBJECT")
        except Exception:
            pass
        raise WorkerFailure("BLENDER_UV_FAILED", "fixed Smart Project UV operation failed") from error


def quantize_uv_coordinates(obj: Any) -> int:
    """Remove sub-ULP Blender UV evaluation drift before GLB export.

    Subdivision can interpolate an inherited UV to adjacent f32 values across
    otherwise identical Blender processes. A 1/65536 grid is about 0.008 texel
    at the fixed 512px bake resolution and is wide enough to absorb the measured
    1.0133e-6 cross-process drift without changing the visible UV layout.
    """
    quantized = 0
    denominator = float(UV_QUANTIZATION_GRID_DENOMINATOR)
    for layer in obj.data.uv_layers:
        for loop in layer.data:
            u = round(float(loop.uv.x) * denominator) / denominator
            v = round(float(loop.uv.y) * denominator) / denominator
            loop.uv.x = u
            loop.uv.y = v
            quantized += 1
    obj.data.update()
    return quantized


def quantize_mesh_geometry(obj: Any) -> tuple[int, int]:
    """Freeze modifier-evaluated positions and split normals before export.

    Blender's bevel/subdivision/weighted-normal evaluation can differ by a
    few low f32 bits across otherwise identical background processes. Those
    differences are invisible at the fixed knife scale but change GLB bytes.
    Quantizing the evaluated mesh to 1e-6 before export keeps the product
    geometry deterministic without accepting caller-controlled tolerances.
    """
    mesh = obj.data
    position_count = 0
    for vertex in mesh.vertices:
        for axis in range(3):
            vertex.co[axis] = round(
                float(vertex.co[axis]), GEOMETRY_QUANTIZATION_DECIMALS
            )
        position_count += 1
    mesh.update()

    normal_count = 0
    try:
        split_normals = []
        for loop in mesh.loops:
            split_normals.append(
                tuple(
                    round(float(component), GEOMETRY_QUANTIZATION_DECIMALS)
                    for component in loop.normal
                )
            )
            normal_count += 1
        mesh.normals_split_custom_set(split_normals)
        mesh.update()
    except Exception:
        # The fixed Blender 5.2.1 sidecar supports custom split normals. Keep
        # this defensive fallback closed so an older development host fails
        # the byte-determinism gate instead of executing caller-selected code.
        normal_count = 0
    return position_count, normal_count


def copy_materials(source: Any, target: Any) -> None:
    target.data.materials.clear()
    for material in source.data.materials:
        if material is not None:
            target.data.materials.append(material.copy())


def add_weighted_normal(bpy: Any, obj: Any) -> None:
    modifier = obj.modifiers.new(name="WPN_WeightedNormal", type="WEIGHTED_NORMAL")
    modifier.keep_sharp = True
    modifier.weight = 50
    apply_modifier(bpy, obj, modifier.name)


def create_collection(bpy: Any, name: str) -> Any:
    collection = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(collection)
    return collection


def duplicate_mesh(source: Any, collection: Any, name: str) -> Any:
    duplicate = source.copy()
    duplicate.data = source.data.copy()
    duplicate.name = name
    collection.objects.link(duplicate)
    duplicate.matrix_world = source.matrix_world.copy()
    copy_materials(source, duplicate)
    return duplicate


def material_ids_for_object(obj: Any) -> list[str]:
    """Return bounded, deterministic material ids for an imported Part."""
    material_ids: list[str] = []
    for index, material in enumerate(obj.data.materials):
        material_name = getattr(material, "name", "") if material is not None else ""
        material_id = safe_name(material_name, f"material_{index:03d}").lower()
        if material_id in material_ids:
            material_id = f"{material_id}-{index}"
        material_ids.append(material_id[:96])
    if not material_ids:
        material_ids.append("material_000")
    return material_ids[:MAX_MATERIAL_IDS_PER_PART]


def material_id_code(material_id: str) -> tuple[int, int, int]:
    """Encode a stable material id into a compact, verifiable RGB token."""
    digest = hashlib.sha256(material_id.encode("utf-8")).digest()
    return digest[0], digest[1], digest[2]


def vector_length(value: Any) -> float:
    try:
        return float(value.length)
    except Exception:
        return math.sqrt(sum(float(component) ** 2 for component in value))


def vector_dot(left: Any, right: Any) -> float:
    try:
        return float(left.dot(right))
    except Exception:
        return sum(float(a) * float(b) for a, b in zip(left, right))


def vector_distance(left: Any, right: Any) -> float:
    try:
        return float((left - right).length)
    except Exception:
        return math.sqrt(sum((float(a) - float(b)) ** 2 for a, b in zip(left, right)))


def clamp01(value: float) -> float:
    return max(0.0, min(1.0, float(value)))


def surface_signal_values(
    obj: Any,
    material_ids_override: list[str] | None = None,
) -> tuple[list[float], list[float], list[int], float, float]:
    """Derive bounded surface signals without textures, files, or external code.

    Curvature is an adjacent-normal discontinuity proxy.  Thickness is
    explicitly a bounded minimum-extent proxy, because measuring true local
    wall thickness requires a separate closed-mesh ray/correspondence gate.
    Material ids remain canonical in GLB material slots and are additionally
    encoded in a color attribute for readback tooling.
    """
    mesh = obj.data
    try:
        mesh.update(calc_edges=True)
    except Exception:
        mesh.update()
    vertices = list(mesh.vertices)
    adjacency: list[set[int]] = [set() for _ in vertices]
    for edge in mesh.edges:
        endpoints = list(edge.vertices)
        if len(endpoints) == 2 and all(0 <= int(index) < len(vertices) for index in endpoints):
            first, second = int(endpoints[0]), int(endpoints[1])
            adjacency[first].add(second)
            adjacency[second].add(first)
    dimensions = [float(value) for value in obj.dimensions]
    max_dimension = max(0.0005, max(dimensions) if dimensions else 0.01)
    min_dimension = max(0.00005, min(dimensions) if dimensions else 0.01)
    curvature: list[float] = []
    for vertex in vertices:
        normal = vertex.normal
        normal_length = vector_length(normal)
        if normal_length <= 1.0e-8 or not adjacency[vertex.index]:
            curvature.append(0.0)
            continue
        signal = 0.0
        count = 0
        for neighbor_index in sorted(adjacency[vertex.index]):
            neighbor = vertices[neighbor_index]
            neighbor_normal = neighbor.normal
            neighbor_length = vector_length(neighbor_normal)
            edge_length = vector_distance(vertex.co, neighbor.co)
            if neighbor_length <= 1.0e-8 or edge_length <= 1.0e-8:
                continue
            normal_change = 0.5 * (1.0 - clamp01(vector_dot(normal, neighbor_normal) / (normal_length * neighbor_length) * 0.5 + 0.5))
            signal += normal_change * max_dimension / edge_length
            count += 1
        curvature.append(clamp01(signal / count) if count else 0.0)
    # No false precision: this is a stable, conservative proxy until the
    # Runtime's exact closed-mesh thickness gate is available.
    thickness_m = min(0.02, max(0.00005, min_dimension * 0.5))
    thickness_normalized = clamp01(thickness_m / max_dimension)
    thickness = [thickness_normalized for _ in vertices]
    material_ids = material_ids_override or material_ids_for_object(obj)
    vertex_material_codes = [0 for _ in vertices]
    for polygon in mesh.polygons:
        slot = int(polygon.material_index)
        material_id = material_ids[slot] if 0 <= slot < len(material_ids) else material_ids[0]
        r, g, b = material_id_code(material_id)
        code = (r << 16) | (g << 8) | b
        for vertex_index in polygon.vertices:
            if 0 <= int(vertex_index) < len(vertex_material_codes):
                vertex_material_codes[int(vertex_index)] = code
    return curvature, thickness, vertex_material_codes, thickness_m, max_dimension


def remove_attribute(mesh: Any, name: str) -> None:
    for collection_name in ("attributes", "color_attributes"):
        collection = getattr(mesh, collection_name, None)
        if collection is None:
            continue
        try:
            attribute = collection.get(name)
            if attribute is not None:
                collection.remove(attribute)
        except Exception:
            pass


def install_surface_signal_attributes(
    obj: Any,
    part_id: str,
    role: str,
    material_ids_override: list[str] | None = None,
) -> dict[str, Any]:
    """Attach fixed signal attributes and an object-extra readback fallback."""
    mesh = obj.data
    curvature, thickness, material_codes, thickness_m, max_dimension = surface_signal_values(obj, material_ids_override)
    material_ids = material_ids_override or material_ids_for_object(obj)
    attributes_written: list[str] = []
    for name in SURFACE_SIGNAL_NAMES:
        remove_attribute(mesh, name)
    try:
        curvature_attribute = mesh.attributes.new(name="WPN_Curvature", type="FLOAT", domain="POINT")
        thickness_attribute = mesh.attributes.new(name="WPN_Thickness", type="FLOAT", domain="POINT")
        material_attribute = mesh.attributes.new(name="WPN_MaterialID", type="FLOAT", domain="POINT")
        for index, datum in enumerate(curvature_attribute.data):
            datum.value = (
                round(curvature[index], GEOMETRY_QUANTIZATION_DECIMALS)
                if index < len(curvature)
                else 0.0
            )
        for index, datum in enumerate(thickness_attribute.data):
            datum.value = (
                round(thickness[index], GEOMETRY_QUANTIZATION_DECIMALS)
                if index < len(thickness)
                else 0.0
            )
        for index, datum in enumerate(material_attribute.data):
            code = material_codes[index] if index < len(material_codes) else 0
            datum.value = code / 16_777_215.0
        attributes_written.extend(SURFACE_SIGNAL_NAMES)
    except Exception:
        # GLB material slots and object extras below remain a verifiable
        # material-id fallback even if a host lacks the generic attribute API.
        pass

    color_attribute_written = False
    remove_attribute(mesh, SURFACE_COLOR_ATTRIBUTE)
    try:
        color_attributes = mesh.color_attributes
        color_attribute = color_attributes.new(
            name=SURFACE_COLOR_ATTRIBUTE,
            type="BYTE_COLOR",
            domain="CORNER",
        )
        for polygon in mesh.polygons:
            slot = int(polygon.material_index)
            material_id = material_ids[slot] if 0 <= slot < len(material_ids) else material_ids[0]
            r, g, b = material_id_code(material_id)
            color = (r / 255.0, g / 255.0, b / 255.0, 1.0)
            for loop_index in polygon.loop_indices:
                color_attribute.data[loop_index].color = color
        color_attribute_written = True
    except Exception:
        pass

    signal_payload = {
        "schema": "weaponry.surface-signals@1",
        "part_id": part_id,
        "role": role,
        "object": obj.name,
        "curvature": [round(value, 6) for value in curvature],
        "thickness_proxy_m": round(thickness_m, 6),
        "thickness_normalized": [round(value, 6) for value in thickness],
        "material_ids": material_ids,
        "material_codes": [f"{code:06x}" for code in sorted(set(material_codes))],
        "storage": {
            "attributes": attributes_written,
            "color_attribute": SURFACE_COLOR_ATTRIBUTE if color_attribute_written else None,
            "material_slots": True,
            "object_extras": True,
        },
    }
    signal_hash = canonical_sha256(signal_payload)
    obj["weaponry_surface_signal_schema"] = "weaponry.surface-signals@1"
    obj["weaponry_surface_signal_hash"] = signal_hash
    obj["weaponry_surface_signal_storage"] = "glb-material-slots-object-extras-and-bounded-attributes@1"
    obj["weaponry_material_ids_json"] = json.dumps(material_ids, ensure_ascii=False, separators=(",", ":"))
    obj["weaponry_curvature_definition"] = "adjacent-normal-discontinuity-bounded-proxy@1"
    obj["weaponry_thickness_definition"] = "minimum-extent-bounded-proxy-not-exact-wall-thickness@1"
    return {
        "schema": "weaponry.surface-signals@1",
        "part_id": part_id,
        "role": role,
        "object": obj.name,
        "signal_sha256": signal_hash,
        "attributes": attributes_written,
        "color_attribute": SURFACE_COLOR_ATTRIBUTE if color_attribute_written else None,
        "material_ids": material_ids,
        "material_id_encoding": "sha256-first-24-bits-rgb@1",
        "curvature": {
            "kind": "adjacent_normal_discontinuity_proxy",
            "range": [0.0, 1.0],
            "sample_count": len(curvature),
        },
        "thickness": {
            "kind": "bounded_min_extent_proxy",
            "measured_m": round(thickness_m, 6),
            "normalization_extent_m": round(max_dimension, 6),
            "exact": False,
        },
    }


def create_independent_cage(
    low: Any,
    collection: Any,
    index: int,
    part_id: str,
    role: str,
) -> tuple[Any, dict[str, Any]]:
    """Create a topology-preserving low-derived cage for selected-to-active bake."""
    cage = duplicate_mesh(low, collection, f"WPN_CAGE_{index:03d}_{safe_name(part_id, 'part')}")
    cage.hide_render = False
    cage.hide_viewport = False
    try:
        low.data.update(calc_edges=True)
    except Exception:
        low.data.update()
    dimensions = [float(value) for value in low.dimensions]
    min_dimension = max(0.00005, min(dimensions) if dimensions else 0.01)
    requested_offset = float(FIXED_RECIPE["cage_extrusion_m"])
    applied_offset = min(requested_offset, max(0.00005, min_dimension * 0.08))
    moved_vertices = 0
    for vertex in cage.data.vertices:
        normal = vertex.normal
        length = vector_length(normal)
        if length <= 1.0e-8:
            continue
        vertex.co += normal.normalized() * applied_offset
        moved_vertices += 1
    cage.data.update()
    topology_hash = mesh_topology_sha256(low)
    cage_topology_hash = mesh_topology_sha256(cage)
    cage["weaponry_part_id"] = part_id
    cage["weaponry_semantic_role"] = role
    cage["weaponry_cage_for"] = low.name
    cage["weaponry_cage_policy"] = "independent-low-derived-normal-offset@1"
    cage["weaponry_cage_offset_m"] = applied_offset
    cage["weaponry_cage_topology_sha256"] = topology_hash
    record = {
        "cage_object": cage.name,
        "low_object": low.name,
        "part_id": part_id,
        "role": role,
        "independent": True,
        "policy": "independent-low-derived-normal-offset@1",
        "requested_offset_m": requested_offset,
        "applied_offset_m": round(applied_offset, 6),
        "moved_vertex_count": moved_vertices,
        "vertex_count": len(cage.data.vertices),
        "polygon_count": len(cage.data.polygons),
        "low_topology_sha256": topology_hash,
        "cage_topology_sha256": cage_topology_hash,
        "topology_preserved": topology_hash == cage_topology_hash,
        "storage": "temporary-bake-participant-not-product-truth",
    }
    return cage, record


def make_image(bpy: Any, name: str, size: int, colorspace: str) -> Any:
    image = bpy.data.images.new(name=name, width=size, height=size, alpha=False, float_buffer=False)
    image.colorspace_settings.name = colorspace
    return image


def bake_one(
    bpy: Any,
    low: Any,
    high: Any,
    cage: Any | None,
    image: Any,
    output_path: Path,
    bake_type: str,
    selected_to_active: bool,
) -> None:
    materials = [material for material in low.data.materials if material is not None]
    if not materials:
        material = bpy.data.materials.new(name=f"WPN_BakeMaterial_{safe_name(low.name, 'part')}")
        material.use_nodes = True
        low.data.materials.append(material)
        materials = [material]
    nodes_to_remove: list[Any] = []
    for material in materials:
        material.use_nodes = True
        nodes = material.node_tree.nodes
        image_node = nodes.new("ShaderNodeTexImage")
        image_node.image = image
        nodes.active = image_node
        nodes_to_remove.append(image_node)
    try:
        if selected_to_active:
            # The cage is an explicit, independent bake participant but is
            # named through Blender's closed operator argument rather than
            # relying on selection order.  Keep only High + Low selected so
            # the source and cage can never become accidental bake sources.
            select_only(bpy, [high, low], low)
        else:
            select_only(bpy, [low], low)
        bpy.context.scene.render.engine = "CYCLES"
        bpy.context.scene.cycles.samples = 8
        bpy.context.scene.cycles.use_denoising = False
        kwargs = {
            "type": bake_type,
            "target": "IMAGE_TEXTURES",
            "use_clear": True,
            "margin": 8,
        }
        if selected_to_active:
            kwargs.update({"use_selected_to_active": True, "normal_space": "TANGENT"})
            if cage is not None:
                cage.hide_render = False
                kwargs.update({"use_cage": True, "cage_object": cage.name, "cage_extrusion": 0.0})
            else:
                # This fallback is retained for defensive compatibility with
                # an older caller inside this fixed file; normal production
                # pairings always provide an explicit cage.
                kwargs.update({"use_cage": False, "cage_extrusion": 0.02})
        bpy.ops.object.bake(**kwargs)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        image.filepath_raw = str(output_path)
        image.file_format = "PNG"
        image.save()
    except Exception as error:
        raise WorkerFailure("BLENDER_BAKE_FAILED", f"fixed {bake_type.lower()} bake failed") from error
    finally:
        if cage is not None:
            cage.hide_render = True
        for material in materials:
            for node in nodes_to_remove:
                if node.name in material.node_tree.nodes:
                    material.node_tree.nodes.remove(node)


def export_glb(bpy: Any, objects: list[Any], path: Path) -> None:
    select_only(bpy, objects, objects[0] if objects else None)
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        bpy.ops.export_scene.gltf(
            filepath=str(path),
            export_format="GLB",
            use_selection=True,
            export_apply=True,
            export_materials="EXPORT",
            export_attributes=True,
            export_extras=True,
        )
    except Exception as error:
        raise WorkerFailure("BLENDER_EXPORT_FAILED", "fixed GLB export failed") from error
    require(path.is_file(), "BLENDER_EXPORT_FAILED", "fixed GLB export produced no file")


def output_record(path: Path, root: Path, kind: str, mime: str) -> dict[str, Any]:
    relative = path.relative_to(root).as_posix()
    byte_size = path.stat().st_size
    require(byte_size <= MAX_OUTPUT_BYTES, "WORKER_OUTPUT_TOO_LARGE", "one output exceeds the fixed output ceiling")
    return {
        "kind": kind,
        "relative_path": relative,
        "mime": mime,
        "byte_size": byte_size,
        "sha256": sha256_file(path),
        "cas_owner": "runtime",
        "durability": "pending_runtime_adoption",
    }


def run_job(job: dict[str, Any], scratch_root: Path, bpy: Any) -> dict[str, Any]:
    # The scene is always reset.  A persistent Blender session or caller
    # supplied .blend is not part of this provider boundary.
    bpy.ops.wm.read_factory_settings(use_empty=True)
    source_path = scratch_root / INPUT_RELATIVE_PATH
    output_root = scratch_root / OUTPUT_DIRECTORY
    output_root.mkdir(parents=True, exist_ok=True)
    high_collection = create_collection(bpy, "Weaponry_High")
    low_collection = create_collection(bpy, "Weaponry_Low")
    cage_collection = create_collection(bpy, "Weaponry_Cage")

    # Keep the caller-staged bytes immutable.  Blender 5.1 currently rejects
    # the custom attribute layout emitted by the Three.js authoring exporter;
    # normalize only the scratch import copy and retain the original hash in
    # every result record.
    normalized_path = scratch_root / "input" / "normalized-source.glb"
    normalize_glb_attributes(source_path, normalized_path)
    try:
        bpy.ops.import_scene.gltf(filepath=str(normalized_path))
    except Exception as error:
        raise WorkerFailure("BLENDER_IMPORT_FAILED", "fixed staged GLB import failed") from error
    sources = sorted((obj for obj in bpy.context.scene.objects if obj.type == "MESH"), key=lambda obj: obj.name)
    require(0 < len(sources) <= MAX_OBJECTS, "WORKER_OBJECT_BUDGET_EXCEEDED", "mesh object count is outside the fixed budget")

    high_objects: list[Any] = []
    low_objects: list[Any] = []
    cage_objects: list[Any] = []
    pairings: list[tuple[Any, Any, Any, str, str, dict[str, Any], dict[str, Any]]] = []
    source_triangles = sum(object_triangles(source) for source in sources)
    high_triangles = 0
    low_triangles = 0
    used_part_ids: set[str] = set()
    part_records: list[dict[str, Any]] = []
    cage_records: list[dict[str, Any]] = []
    surface_signal_records: list[dict[str, Any]] = []

    for index, source in enumerate(sources):
        source_part_triangles = object_triangles(source)
        source.hide_render = True
        base_name = safe_name(source.name, f"part_{index:03d}")
        part_id = stable_part_id(source.name, index, used_part_ids)
        role = semantic_role_for_name(source.name)
        policy = role_policy(role)
        source_material_ids = material_ids_for_object(source)
        high = duplicate_mesh(source, high_collection, f"WPN_HI_{index:03d}_{base_name}")
        low = duplicate_mesh(source, low_collection, f"WPN_LO_{index:03d}_{base_name}")
        # ``Object.copy`` inherits hide_render from the imported source.  The
        # source is intentionally hidden, but both derived bake participants
        # must be render-enabled or Cycles' selected-to-active poll rejects
        # the pair before tracing any rays.
        high.hide_render = False
        low.hide_render = False
        apply_scale(bpy, high)
        apply_scale(bpy, low)

        dimensions = [float(value) for value in high.dimensions]
        smallest_dimension = max(0.0005, min(dimensions) if dimensions else 0.01)
        high_subdivision = apply_bounded_subdivision(
            bpy,
            high,
            role,
            source_part_triangles,
            source_triangles,
            high_triangles,
        )
        high_bevel_width = min(
            float(policy["bevel_cap_m"]),
            max(0.00005, smallest_dimension * float(policy["high_bevel_ratio"])),
            smallest_dimension * 0.20,
        )
        bevel = high.modifiers.new(name="WPN_Bevel", type="BEVEL")
        bevel.width = high_bevel_width
        bevel.segments = int(policy["bevel_segments"])
        bevel.limit_method = "ANGLE"
        bevel.angle_limit = float(policy["angle_limit_rad"])
        bevel.harden_normals = True
        apply_modifier(bpy, high, bevel.name)
        high_surface_pass = apply_bounded_surface_pass(bpy, high, role)
        add_weighted_normal(bpy, high)
        quantize_mesh_geometry(high)

        decimate = low.modifiers.new(name="WPN_Decimate", type="DECIMATE")
        decimate.decimate_type = "COLLAPSE"
        decimate.ratio = float(policy["decimate_ratio"])
        apply_modifier(bpy, low, decimate.name)
        low_dimensions = [float(value) for value in low.dimensions]
        low_smallest_dimension = max(0.0005, min(low_dimensions) if low_dimensions else smallest_dimension)
        low_bevel_width = min(
            float(policy["bevel_cap_m"]) * 0.75,
            max(0.00005, low_smallest_dimension * float(policy["low_bevel_ratio"])),
            low_smallest_dimension * 0.16,
        )
        low_bevel = low.modifiers.new(name="WPN_Bevel", type="BEVEL")
        low_bevel.width = low_bevel_width
        low_bevel.segments = max(2, min(3, int(policy["bevel_segments"])))
        low_bevel.limit_method = "ANGLE"
        low_bevel.angle_limit = float(policy["angle_limit_rad"])
        low_bevel.harden_normals = True
        low_bevel_applied = apply_optional_modifier(bpy, low, low_bevel)
        low_surface_pass = apply_bounded_surface_pass(bpy, low, role)
        add_weighted_normal(bpy, low)
        quantize_mesh_geometry(low)
        smart_project_uv(bpy, low)

        high_uv_loop_count = quantize_uv_coordinates(high)
        low_uv_loop_count = quantize_uv_coordinates(low)

        for derived in (high, low):
            derived["weaponry_part_id"] = part_id
            derived["weaponry_semantic_role"] = role
            derived["weaponry_source_object"] = source.name
            derived["weaponry_operation_scope"] = "part-local-fixed-role-policy@1"
            derived["weaponry_material_ids_json"] = json.dumps(
                source_material_ids,
                ensure_ascii=False,
                separators=(",", ":"),
            )
        high["weaponry_source_object"] = source.name
        high["weaponry_part_role"] = "high"
        low["weaponry_source_object"] = source.name
        low["weaponry_part_role"] = "low"
        high_objects.append(high)
        low_objects.append(low)
        cage, cage_record = create_independent_cage(low, cage_collection, index, part_id, role)
        cage_objects.append(cage)
        cage_records.append(cage_record)
        high_surface_signal = install_surface_signal_attributes(high, part_id, role, source_material_ids)
        low_surface_signal = install_surface_signal_attributes(low, part_id, role, source_material_ids)
        surface_signal_records.extend([high_surface_signal, low_surface_signal])
        part_records.append({
            "part_id": part_id,
            "source_object": source.name,
            "role": role,
            "high_object": high.name,
            "low_object": low.name,
            "source_triangle_count": source_part_triangles,
            "high_subdivision": high_subdivision,
            "high_bevel_width_m": round(high_bevel_width, 6),
            "high_surface_pass": high_surface_pass,
            "low_decimate_ratio": policy["decimate_ratio"],
            "low_bevel_applied": low_bevel_applied,
            "low_bevel_width_m": round(low_bevel_width, 6),
            "low_surface_pass": low_surface_pass,
            "high_uv_loop_count": high_uv_loop_count,
            "low_uv_loop_count": low_uv_loop_count,
            "uv_quantization_grid_denominator": UV_QUANTIZATION_GRID_DENOMINATOR,
            "material_ids": source_material_ids,
        })
        pairings.append((low, high, cage, role, part_id, high_surface_signal, low_surface_signal))
        high_triangles += object_triangles(high)
        low_triangles += object_triangles(low)

    require(source_triangles <= MAX_TRIANGLES, "WORKER_TRIANGLE_BUDGET_EXCEEDED", "source triangle count exceeds fixed budget")
    require(high_triangles <= MAX_TRIANGLES, "WORKER_TRIANGLE_BUDGET_EXCEEDED", "High triangle count exceeds fixed budget")
    require(low_triangles <= MAX_TRIANGLES, "WORKER_TRIANGLE_BUDGET_EXCEEDED", "Low triangle count exceeds fixed budget")

    high_path = output_root / "dragonfang-high.blend.glb"
    low_path = output_root / "dragonfang-low.blend.glb"
    export_glb(bpy, high_objects, high_path)
    export_glb(bpy, low_objects, low_path)

    outputs = [
        output_record(high_path, scratch_root, "high_glb", "model/gltf-binary"),
        output_record(low_path, scratch_root, "low_glb", "model/gltf-binary"),
    ]
    map_records: list[dict[str, Any]] = []
    for index, (low, high, cage, role, part_id, high_surface_signal, low_surface_signal) in enumerate(pairings):
        map_base = safe_name(low.name, f"part_{index:03d}")
        normal_path = output_root / "maps" / f"{index:03d}-{map_base}-normal.png"
        normal_image = make_image(bpy, f"WPN_Normal_{index:03d}", MAX_TEXTURE_SIZE, "Non-Color")
        bake_one(bpy, low, high, cage, normal_image, normal_path, "NORMAL", True)
        normal_record = output_record(scratch_root / OUTPUT_DIRECTORY / "maps" / normal_path.name, scratch_root, "normal_map", "image/png")
        outputs.append(normal_record)

        ao_path = output_root / "maps" / f"{index:03d}-{map_base}-ao.png"
        ao_image = make_image(bpy, f"WPN_AO_{index:03d}", MAX_TEXTURE_SIZE, "Non-Color")
        bake_one(bpy, low, high, cage, ao_image, ao_path, "AO", False)
        ao_record = output_record(scratch_root / OUTPUT_DIRECTORY / "maps" / ao_path.name, scratch_root, "ao_map", "image/png")
        outputs.append(ao_record)
        map_records.extend([
            {"part_index": index, "part_id": part_id, "role": role, "kind": "normal_map", "sha256": normal_record["sha256"], "relative_path": normal_record["relative_path"]},
            {"part_index": index, "part_id": part_id, "role": role, "kind": "ao_map", "sha256": ao_record["sha256"], "relative_path": ao_record["relative_path"]},
        ])

    total_output_bytes = sum(record["byte_size"] for record in outputs)
    require(total_output_bytes <= job["budgets"]["max_output_bytes"], "WORKER_OUTPUT_TOO_LARGE", "outputs exceed the request output budget")
    stats = {
        "source_object_count": len(sources),
        "high_object_count": len(high_objects),
        "low_object_count": len(low_objects),
        "source_triangle_count": source_triangles,
        "high_triangle_count": high_triangles,
        "low_triangle_count": low_triangles,
        "bake_map_count": len(map_records),
        "texture_size": MAX_TEXTURE_SIZE,
    }
    checks = {
        "validator_status": "prototype_pending_runtime_readback",
        "readback_status": "prototype_pending_runtime_readback",
        "deterministic_replay_status": "not_run",
        "stage_eligibility": "non_promoting_prototype",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
    }
    manifest = {
        "schema_version": "WeaponryBlenderKnifeWorkerManifest@1",
        "worker_id": WORKER_ID,
        "worker_version": WORKER_VERSION,
        "blender_version": BLENDER_VERSION,
        "blender_revision": BLENDER_REVISION,
        "blender_build_hash": BLENDER_REVISION,
        "worker_entrypoint_sha256": worker_entrypoint_sha256(),
        "dependency_lock_sha256": dependency_lock_sha256(),
        "operation": OPERATION,
        "policy": POLICY,
        "request_id": job["request_id"],
        "project_id": job["project_id"],
        "candidate_id": job["candidate_id"],
        "source_authoring_mesh_sha256": job["input_glb"]["sha256"],
        "blender_import_normalization": "standard-position-normal-uv-only-scratch-copy@1",
        "recipe_sha256": job["recipe_sha256"],
        "implementation_profile": ENHANCED_RECIPE_PROFILE,
        "part_operations": part_records,
        "cages": cage_records,
        "surface_signals": surface_signal_records,
        "outputs": outputs,
        "maps": map_records,
        "stats": stats,
        "checks": checks,
        "runtime_write_performed": False,
        "stage_advanced": False,
        "candidate_confirmed": False,
        "version_created": False,
        "export_performed": False,
        "canonical_sha256": "",
    }
    manifest["canonical_sha256"] = canonical_sha256(manifest)
    manifest_path = output_root / "manifest.json"
    manifest_path.write_bytes(canonical_bytes(manifest))
    manifest_record = output_record(manifest_path, scratch_root, "worker_manifest", "application/json")
    outputs.append(manifest_record)
    result = {
        "schema_version": RESULT_SCHEMA,
        "operation": OPERATION,
        "request_id": job["request_id"],
        "project_id": job["project_id"],
        "candidate_id": job["candidate_id"],
        "source_authoring_mesh_sha256": job["input_glb"]["sha256"],
        "recipe_sha256": job["recipe_sha256"],
        "policy": POLICY,
        "worker_id": WORKER_ID,
        "worker_version": WORKER_VERSION,
        "blender_version": BLENDER_VERSION,
        "blender_revision": BLENDER_REVISION,
        "blender_build_hash": BLENDER_REVISION,
        "worker_entrypoint_sha256": worker_entrypoint_sha256(),
        "dependency_lock_sha256": dependency_lock_sha256(),
        "input_canonical_sha256": job["canonical_sha256"],
        "outputs": outputs,
        "stats": stats,
        "checks": checks,
        "runtime_write_performed": False,
        "stage_advanced": False,
        "candidate_confirmed": False,
        "version_created": False,
        "export_performed": False,
        "canonical_sha256": "",
    }
    result["canonical_sha256"] = canonical_sha256(result)
    (output_root / "worker-result.json").write_bytes(canonical_bytes(result))
    return result


def success_response(result: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": RESPONSE_SCHEMA,
        "protocol": PROTOCOL,
        "request_id": result["request_id"],
        "operation": result["operation"],
        "ok": True,
        "result": result,
        "error": None,
    }


def error_response(request_id: str, code: str, message: str) -> dict[str, Any]:
    return {
        "schema_version": RESPONSE_SCHEMA,
        "protocol": PROTOCOL,
        "request_id": request_id if ID_RE.fullmatch(request_id) else "invalid-request",
        "operation": OPERATION,
        "ok": False,
        "result": None,
        "error": {"code": code, "message": message},
    }


def emit(response: dict[str, Any]) -> int:
    payload = canonical_bytes(response)
    if len(payload) > MAX_STDOUT_BYTES:
        response = error_response("invalid-request", "WORKER_RESPONSE_TOO_LARGE", "worker response exceeds the fixed stdout ceiling")
        payload = canonical_bytes(response)
    sys.stdout.buffer.write(payload + b"\n")
    sys.stdout.buffer.flush()
    return 0 if response["ok"] else 1


def read_request() -> Any:
    data = sys.stdin.buffer.read(MAX_REQUEST_BYTES + 1)
    require(len(data) <= MAX_REQUEST_BYTES, "WORKER_REQUEST_TOO_LARGE", "request exceeds the fixed input ceiling")
    try:
        return json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise WorkerFailure("WORKER_PROTOCOL_INVALID", "request is not valid UTF-8 JSON") from error


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(add_help=True)
    parser.add_argument("--scratch-root", required=True)
    # Blender owns the arguments before its ``--`` sentinel.  Only the
    # launcher-owned suffix is part of this worker's tiny command contract;
    # accepting Blender's global argv here would make the entrypoint fragile
    # and would accidentally allow a second executable policy surface.
    argv = sys.argv[1:]
    require("--" in argv, "WORKER_ARGUMENTS_NOT_ALLOWED", "worker arguments require the Blender -- sentinel")
    worker_argv = argv[argv.index("--") + 1 :]
    try:
        return parser.parse_args(worker_argv)
    except SystemExit as error:
        raise WorkerFailure("WORKER_ARGUMENTS_NOT_ALLOWED", "worker arguments are not the fixed scratch-root contract") from error


def main() -> int:
    request_id = "invalid-request"
    try:
        arguments = parse_args()
        scratch_root = Path(arguments.scratch_root).resolve()
        require(scratch_root.is_dir(), "WORKER_SCRATCH_INVALID", "scratch root is not a directory")
        raw = read_request()
        if isinstance(raw, dict) and isinstance(raw.get("request_id"), str):
            request_id = raw["request_id"]
        job = parse_job(raw, scratch_root)
        try:
            import bpy  # type: ignore
        except ImportError as error:
            raise WorkerFailure("BLENDER_UNAVAILABLE", "the fixed Blender Python host is unavailable") from error
        result = run_job(job, scratch_root, bpy)
        return emit(success_response(result))
    except WorkerFailure as error:
        return emit(error_response(request_id, error.code, error.message))
    except Exception:
        # Never expose host paths, Python tracebacks, environment values or
        # user-supplied payloads through the worker protocol.
        return emit(error_response(request_id, "BLENDER_WORKER_FAILED", "fixed Blender worker failed closed"))


if __name__ == "__main__":
    raise SystemExit(main())
