#!/usr/bin/env python3
"""Fail-closed contract, registry, Rust expansion, and GLB gate for FGC-C105.

The gate never contacts a Provider.  It proves that the reviewed registry is a
closed Draft 2020-12 contract, consumes the one G819 operation manifest, and
rejects malformed graphs before Rust expansion.  Focused Rust tests regenerate
and compare the four-domain expanded golden; Python then compiles those exact
ShapePrograms through RestrictedGeometryExecutor and verifies GLB/readback
surface provenance.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import copy
import hashlib
import json
import math
import re
import subprocess
import sys
from collections import Counter
from collections.abc import Iterator, Mapping, Sequence
from pathlib import Path
from typing import Any, NoReturn

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry as SchemaRegistry, Resource


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "packages/concept-spec/schemas"
RECIPE_SCHEMA_PATH = SCHEMA_DIR / "editable-component-recipe.schema.json"
C105_SCHEMA_PATHS = (
    RECIPE_SCHEMA_PATH,
    SCHEMA_DIR / "component-recipe-ref.schema.json",
    SCHEMA_DIR / "component-recipe-instance-provenance.schema.json",
    SCHEMA_DIR / "component-recipe-instantiation-request.schema.json",
    SCHEMA_DIR / "component-recipe-candidate.schema.json",
)
REGISTRY_PATH = ROOT / "packages/concept-spec/fixtures/editable-component-recipe-registry.json"
EXPANDED_GOLDEN_PATH = ROOT / "packages/concept-spec/fixtures/component-recipe-expanded-golden.json"
RUNTIME_MANIFEST_PATH = ROOT / "packages/concept-spec/fixtures/shape-program-runtime-manifest.json"
RUST_MANIFEST_PATH = ROOT / "apps/desktop/src-tauri/Cargo.toml"
RUST_CORE_ROOT = ROOT / "apps/desktop/src-tauri/crates/forgecad-core"

REGISTRY_SCHEMA_VERSION = "EditableComponentRecipeRegistry@1"
RECIPE_SCHEMA_VERSION = "EditableComponentRecipe@1"
POLICY_VERSION = "ComponentRecipePolicy@1"
EXPANSION_GOLDEN_SCHEMA_VERSION = "ComponentRecipeExpansionGolden@1"
CANDIDATE_HASH_SCOPE = (
    "semantic_sha256(ComponentRecipeCandidate@1 with candidate_sha256 blank; "
    "transient in-memory instances omitted)"
)
MAX_EXPANDED_TRIANGLES = 100_000
MAX_EXPANDED_INSTANCES = 128
MAX_EXPANDED_FEATURES = 256
MAX_CHILD_DEPTH = 6
FRAME_EPSILON = 1e-6

FORBIDDEN_KEYS = {
    "code",
    "command",
    "executable",
    "file",
    "file_path",
    "filename",
    "javascript",
    "python",
    "script",
    "shell",
    "uri",
    "url",
}
URL_PATTERN = re.compile(r"(?:[a-z][a-z0-9+.-]*://|www\.)", re.IGNORECASE)
ABSOLUTE_PATH_PATTERN = re.compile(r"(?:^/|^\\\\|^[A-Za-z]:[\\/])")
RELATIVE_PATH_PATTERN = re.compile(r"(?:^|\s)(?:\.\.?[\\/])")
EXECUTABLE_TEXT_PATTERN = re.compile(
    r"(?:^#!|\b(?:eval|exec|subprocess|os\.system)\s*\(|\b(?:function|import)\s+[A-Za-z_])",
    re.IGNORECASE,
)


class C105GateFailure(AssertionError):
    """A stable, user-safe C105 gate failure."""

    def __init__(self, code: str, detail: str) -> None:
        super().__init__(f"{code}: {detail}")
        self.code = code
        self.detail = detail


def _fail(code: str, detail: str) -> NoReturn:
    raise C105GateFailure(code, detail)


def _load_json(path: Path) -> Any:
    if not path.is_file():
        _fail("C105_CONTRACT_MISSING", f"required repository contract is missing: {path.name}")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        _fail("C105_CONTRACT_INVALID_JSON", f"{path.name}: {exc}")


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


def _walk(value: Any, path: tuple[str, ...] = ()) -> Iterator[tuple[tuple[str, ...], Any]]:
    yield path, value
    if isinstance(value, Mapping):
        for key, child in value.items():
            yield from _walk(child, (*path, str(key)))
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        for index, child in enumerate(value):
            yield from _walk(child, (*path, str(index)))


def _schema_registry() -> SchemaRegistry:
    resources: list[tuple[str, Resource[Any]]] = []
    for path in SCHEMA_DIR.glob("*.schema.json"):
        schema = _load_json(path)
        schema_id = schema.get("$id")
        if isinstance(schema_id, str):
            resources.append((schema_id, Resource.from_contents(schema)))
    return SchemaRegistry().with_resources(resources)


def _assert_c105_schemas() -> tuple[dict[str, Any], Draft202012Validator]:
    schema_registry = _schema_registry()
    for path in C105_SCHEMA_PATHS:
        schema = _load_json(path)
        try:
            Draft202012Validator.check_schema(schema)
        except Exception as exc:  # jsonschema exposes several schema error subclasses.
            _fail("C105_SCHEMA_INVALID", f"{path.name}: {exc}")

    recipe_schema = _load_json(RECIPE_SCHEMA_PATH)
    return recipe_schema, Draft202012Validator(
        recipe_schema,
        registry=schema_registry,
        format_checker=FormatChecker(),
    )


def _schema_validator(path: Path) -> Draft202012Validator:
    schema = _load_json(path)
    try:
        Draft202012Validator.check_schema(schema)
    except Exception as exc:
        _fail("C105_SCHEMA_INVALID", f"{path.name}: {exc}")
    return Draft202012Validator(
        schema,
        registry=_schema_registry(),
        format_checker=FormatChecker(),
    )


def _assert_schema_value(
    validator: Draft202012Validator,
    value: Mapping[str, Any],
    *,
    code: str,
    identity: str,
) -> None:
    errors = sorted(validator.iter_errors(value), key=lambda error: tuple(str(item) for item in error.path))
    if errors:
        error = errors[0]
        location = "/".join(str(item) for item in error.absolute_path) or "<root>"
        _fail(code, f"{identity} at {location}: {error.message}")


def _assert_schema_recipe(validator: Draft202012Validator, recipe: Mapping[str, Any]) -> None:
    _assert_schema_value(
        validator,
        recipe,
        code="C105_RECIPE_SCHEMA_INVALID",
        identity=str(recipe.get("recipe_id", "<unknown>")),
    )


def _expected_registry_sha256(golden: Mapping[str, Any]) -> str:
    digest = golden.get("registry_sha256")
    if not isinstance(digest, str) or not re.fullmatch(r"[a-f0-9]{64}", digest):
        _fail("C105_GOLDEN_REGISTRY_HASH_MISSING", "expanded golden must pin the reviewed registry SHA-256")
    return digest


def _assert_registry_envelope(registry: Mapping[str, Any], expected_registry_sha256: str) -> str:
    expected_keys = {"schema_version", "registry_id", "policy_version", "recipes"}
    if set(registry) != expected_keys:
        _fail("C105_REGISTRY_NOT_CLOSED", f"unexpected registry keys: {sorted(set(registry) ^ expected_keys)}")
    if registry.get("schema_version") != REGISTRY_SCHEMA_VERSION:
        _fail("C105_REGISTRY_VERSION_UNSUPPORTED", str(registry.get("schema_version")))
    if registry.get("policy_version") != POLICY_VERSION:
        _fail("C105_POLICY_VERSION_UNSUPPORTED", str(registry.get("policy_version")))
    recipes = registry.get("recipes")
    if not isinstance(recipes, list) or len(recipes) < 8:
        _fail("C105_REGISTRY_INCOMPLETE", "reviewed registry must contain the four roots and four detail children")

    digest = canonical_sha256(registry)
    if digest != expected_registry_sha256:
        _fail("C105_REGISTRY_CANONICAL_HASH_MISMATCH", f"expected {expected_registry_sha256}, received {digest}")
    # A parse/canonicalize/parse cycle must be byte-stable and must reject NaN.
    if canonical_bytes(json.loads(canonical_bytes(registry).decode("utf-8"))) != canonical_bytes(registry):
        _fail("C105_REGISTRY_CANONICALIZATION_UNSTABLE", "canonical JSON did not round-trip")
    return digest


def _runtime_operations(recipe_schema: Mapping[str, Any]) -> frozenset[str]:
    manifest = _load_json(RUNTIME_MANIFEST_PATH)
    if manifest.get("schema_version") != "ShapeProgramRuntimeManifest@1":
        _fail("C105_G819_MANIFEST_INVALID", str(manifest.get("schema_version")))
    rows = manifest.get("operations")
    if not isinstance(rows, list) or not rows:
        _fail("C105_G819_MANIFEST_INVALID", "operations must be a non-empty array")
    operations = [row.get("op") for row in rows if isinstance(row, Mapping)]
    if len(operations) != len(rows) or any(not isinstance(op, str) for op in operations):
        _fail("C105_G819_MANIFEST_INVALID", "each operation must have one string op")
    if len(set(operations)) != len(operations):
        _fail("C105_G819_MANIFEST_INVALID", "duplicate operation")

    shape_program_schema = _load_json(SCHEMA_DIR / "shape-program.schema.json")
    try:
        schema_operations = shape_program_schema["properties"]["operations"]["items"]["properties"]["op"]["enum"]
    except (KeyError, TypeError):
        _fail("C105_SCHEMA_G819_BINDING_MISSING", "geometry feature op enum is missing")
    if list(schema_operations) != operations:
        _fail("C105_SCHEMA_G819_DRIFT", "recipe Schema op enum must exactly follow the ordered G819 manifest")
    return frozenset(operations)


def _assert_no_executable_or_location_payload(value: Any) -> None:
    for path, child in _walk(value):
        location = "/".join(path) or "<root>"
        if path and path[-1].casefold() in FORBIDDEN_KEYS:
            _fail("C105_FORBIDDEN_FIELD", location)
        if not isinstance(child, str):
            continue
        if URL_PATTERN.search(child):
            _fail("C105_FORBIDDEN_URL", location)
        if ABSOLUTE_PATH_PATTERN.search(child):
            _fail("C105_FORBIDDEN_ABSOLUTE_PATH", location)
        if RELATIVE_PATH_PATTERN.search(child):
            _fail("C105_FORBIDDEN_RELATIVE_PATH", location)
        if EXECUTABLE_TEXT_PATTERN.search(child):
            _fail("C105_FORBIDDEN_EXECUTABLE_TEXT", location)


def _assert_review_source_license_quality(recipe: Mapping[str, Any]) -> None:
    recipe_id = str(recipe.get("recipe_id", "<unknown>"))
    if recipe.get("quality_status") != "passed":
        _fail("C105_QUALITY_NOT_PASSED", recipe_id)
    source = recipe.get("source")
    if not isinstance(source, Mapping) or source.get("source_kind") != "forgecad_first_party":
        _fail("C105_SOURCE_UNTRUSTED", recipe_id)
    license_value = recipe.get("license")
    if (
        not isinstance(license_value, Mapping)
        or license_value.get("license_id") != "ForgeCAD-Internal-Visual-Only"
        or license_value.get("redistributable") is not False
    ):
        _fail("C105_LICENSE_INVALID", recipe_id)
    review = recipe.get("review_state")
    if not isinstance(review, Mapping) or review.get("reviewer_kind") != "forgecad_internal":
        _fail("C105_REVIEW_NOT_READY", recipe_id)
    if recipe.get("non_functional_only") is not True:
        _fail("C105_NON_FUNCTIONAL_BOUNDARY_MISSING", recipe_id)


def _canonical_profile_resource(value: Mapping[str, Any], location: str) -> tuple[dict[str, Any], str]:
    agent_root = ROOT / "apps/agent"
    if str(agent_root) not in sys.path:
        sys.path.insert(0, str(agent_root))
    try:
        from forgecad_agent.application.profile_contracts import canonical_profile_payload

        normalized, _canonical, digest = canonical_profile_payload(value)
    except Exception as exc:
        _fail("C105_PROFILE_RESOURCE_INVALID", f"{location}: {exc}")
    return normalized, digest


def _assert_embedded_profile_resources(recipe: Mapping[str, Any]) -> None:
    recipe_id = str(recipe["recipe_id"])
    program = recipe.get("shape_program_template")
    if not isinstance(program, Mapping):
        _fail("C105_SHAPE_PROGRAM_TEMPLATE_MISSING", recipe_id)
    actual_resources: dict[str, tuple[str, dict[str, Any], str]] = {}
    for item in program.get("profile_inputs", []):
        input_id = item.get("input_id")
        if not isinstance(input_id, str):
            _fail("C105_PROFILE_INPUT_ID_MISSING", recipe_id)
        payload = item.get("canonical_payload")
        if not isinstance(payload, Mapping):
            _fail("C105_PROFILE_INPUT_PAYLOAD_MISSING", f"{recipe_id}/{input_id}")
        normalized, digest = _canonical_profile_resource(payload, f"{recipe_id}/{input_id}")
        if digest != item.get("input_sha256"):
            _fail("C105_PROFILE_INPUT_HASH_MISMATCH", f"{recipe_id}/{input_id}")
        if input_id in actual_resources:
            _fail("C105_PROFILE_INPUT_DUPLICATE", f"{recipe_id}/{input_id}")
        actual_resources[input_id] = (str(item.get("contract_version")), normalized, digest)

    referenced_input_ids: set[str] = set()
    for profile_ref in recipe["profiles"]:
        input_id = profile_ref["profile_input_id"]
        resource = actual_resources.get(input_id)
        if resource is None:
            _fail("C105_PROFILE_RESOURCE_BINDING_MISSING", f"{recipe_id}/{input_id}")
        contract_version, normalized, _digest = resource
        if contract_version != "ProfileSketch@1" or normalized.get("sketch_id") != profile_ref["profile_sketch_id"]:
            _fail("C105_PROFILE_RESOURCE_ID_MISMATCH", f"{recipe_id}/{input_id}")
        referenced_input_ids.add(input_id)

    for section_ref in recipe["section_sets"]:
        section_id = section_ref["section_set_id"]
        payload = section_ref.get("canonical_payload")
        if not isinstance(payload, Mapping):
            _fail("C105_PROFILE_RESOURCE_PAYLOAD_MISSING", f"{recipe_id}/{section_id}")
        normalized, digest = _canonical_profile_resource(payload, f"{recipe_id}/{section_id}")
        if normalized.get("section_set_id") != section_id or digest != section_ref["sha256"]:
            _fail("C105_PROFILE_RESOURCE_HASH_MISMATCH", f"{recipe_id}/{section_id}")
        matching_ids = {
            input_id
            for input_id, (version, input_payload, input_digest) in actual_resources.items()
            if version == "ProfileSectionSet@1"
            and input_payload.get("section_set_id") == section_id
            and input_digest == digest
        }
        if len(matching_ids) != 1:
            _fail("C105_PROFILE_RESOURCE_BINDING_MISMATCH", f"{recipe_id}/{section_id}")
        referenced_input_ids.update(matching_ids)

    if referenced_input_ids != set(actual_resources):
        _fail("C105_PROFILE_RESOURCE_UNREFERENCED", recipe_id)


def _assert_shape_program_template_alignment(recipe: Mapping[str, Any]) -> None:
    recipe_id = str(recipe["recipe_id"])
    program = recipe.get("shape_program_template")
    if not isinstance(program, Mapping):
        _fail("C105_SHAPE_PROGRAM_TEMPLATE_MISSING", recipe_id)
    if program.get("non_functional_only") is not True:
        _fail("C105_SHAPE_PROGRAM_SCOPE_INVALID", recipe_id)
    if not isinstance(program.get("triangle_budget"), int) or program["triangle_budget"] < recipe["triangle_estimate"]:
        _fail("C105_SHAPE_PROGRAM_BUDGET_INVALID", recipe_id)
    operations = program.get("operations")
    outputs = program.get("outputs")
    if not isinstance(operations, list) or not operations or not isinstance(outputs, list) or not outputs:
        _fail("C105_SHAPE_PROGRAM_TEMPLATE_INCOMPLETE", recipe_id)

    operation_by_id = {
        operation.get("operation_id"): operation
        for operation in operations
        if isinstance(operation, Mapping)
    }
    feature_operation_ids = [feature["operation_id"] for feature in recipe["feature_template"]]
    if len(feature_operation_ids) != len(set(feature_operation_ids)):
        _fail("C105_FEATURE_OPERATION_DUPLICATE", recipe_id)
    if set(feature_operation_ids) != set(operation_by_id):
        _fail("C105_FEATURE_TEMPLATE_PROGRAM_DRIFT", recipe_id)
    for feature in recipe["feature_template"]:
        operation = operation_by_id[feature["operation_id"]]
        if operation.get("args", {}).get("part_role") != feature["role"]:
            _fail("C105_FEATURE_ROLE_PROGRAM_DRIFT", f"{recipe_id}/{feature['feature_id']}")

    zones = {zone["zone_id"]: zone["material_preset_id"] for zone in recipe["material_zones"]}
    used_zones: set[str] = set()
    for operation in operations:
        args = operation.get("args", {})
        if not isinstance(args, Mapping):
            _fail("C105_SHAPE_PROGRAM_TEMPLATE_INCOMPLETE", recipe_id)
        zone_id = args.get("zone_id")
        material_id = args.get("material_id")
        if zone_id is None:
            continue
        if zones.get(zone_id) != material_id:
            _fail("C105_MATERIAL_ZONE_PROGRAM_DRIFT", f"{recipe_id}/{zone_id}")
        used_zones.add(zone_id)
    if used_zones != set(zones):
        missing = sorted(set(zones) - used_zones)
        _fail("C105_MATERIAL_ZONE_PROGRAM_MISSING", f"{recipe_id}: {missing}")

    agent_root = ROOT / "apps/agent"
    if str(agent_root) not in sys.path:
        sys.path.insert(0, str(agent_root))
    try:
        from forgecad_agent.application.geometry_worker import assert_shape_program_runtime_compatible

        assert_shape_program_runtime_compatible(dict(program))
    except Exception as exc:
        _fail("C105_SHAPE_PROGRAM_RUNTIME_INVALID", f"{recipe_id}: {exc}")


def _finite_vector(value: Any, *, length: int, location: str) -> list[float]:
    if not isinstance(value, list) or len(value) != length:
        _fail("C105_TRANSFORM_INVALID", f"{location} must contain {length} values")
    result: list[float] = []
    for item in value:
        if isinstance(item, bool) or not isinstance(item, (int, float)) or not math.isfinite(float(item)):
            _fail("C105_TRANSFORM_INVALID", f"{location} must contain finite numbers")
        result.append(float(item))
    return result


def _assert_frame(frame: Mapping[str, Any], location: str) -> None:
    _finite_vector(frame.get("position"), length=3, location=f"{location}/position")
    normal = _finite_vector(frame.get("normal"), length=3, location=f"{location}/normal")
    up = _finite_vector(frame.get("up"), length=3, location=f"{location}/up")
    normal_length = math.sqrt(sum(value * value for value in normal))
    up_length = math.sqrt(sum(value * value for value in up))
    dot = sum(left * right for left, right in zip(normal, up, strict=True))
    if abs(normal_length - 1.0) > FRAME_EPSILON or abs(up_length - 1.0) > FRAME_EPSILON:
        _fail("C105_FRAME_NOT_UNIT", location)
    if abs(dot) > FRAME_EPSILON:
        _fail("C105_FRAME_NOT_ORTHOGONAL", location)
    tangent = (
        normal[1] * up[2] - normal[2] * up[1],
        normal[2] * up[0] - normal[0] * up[2],
        normal[0] * up[1] - normal[1] * up[0],
    )
    if math.sqrt(sum(value * value for value in tangent)) < 1.0 - FRAME_EPSILON:
        _fail("C105_FRAME_NOT_RIGHT_HANDED", location)


def _assert_local_transform(transform: Mapping[str, Any], location: str) -> None:
    _finite_vector(transform.get("position"), length=3, location=f"{location}/position")
    _finite_vector(transform.get("rotation"), length=3, location=f"{location}/rotation")
    scale = _finite_vector(transform.get("scale"), length=3, location=f"{location}/scale")
    if any(value <= 0.0 for value in scale):
        _fail("C105_TRANSFORM_INVALID", f"{location}/scale must remain positive")


def _assert_recipe_local_contract(recipe: Mapping[str, Any], runtime_operations: frozenset[str]) -> None:
    recipe_id = str(recipe["recipe_id"])
    if recipe.get("schema_version") != RECIPE_SCHEMA_VERSION:
        _fail("C105_RECIPE_VERSION_UNSUPPORTED", recipe_id)
    _assert_no_executable_or_location_payload(recipe)
    _assert_review_source_license_quality(recipe)

    feature_ids: set[str] = set()
    for feature in recipe["feature_template"]:
        feature_id = feature["feature_id"]
        if feature_id in feature_ids:
            _fail("C105_DUPLICATE_FEATURE", f"{recipe_id}/{feature_id}")
        feature_ids.add(feature_id)

    program = recipe.get("shape_program_template")
    if not isinstance(program, Mapping):
        _fail("C105_SHAPE_PROGRAM_TEMPLATE_MISSING", recipe_id)
    for operation in program.get("operations", []):
        if operation.get("op") not in runtime_operations:
            _fail("C105_UNSUPPORTED_RUNTIME_OPERATION", f"{recipe_id}/{operation.get('op')}")

    _assert_embedded_profile_resources(recipe)
    _assert_shape_program_template_alignment(recipe)

    connector_ids: set[str] = set()
    for connector in recipe["connectors"]:
        connector_id = connector["connector_id"]
        if connector_id in connector_ids:
            _fail("C105_DUPLICATE_CONNECTOR", f"{recipe_id}/{connector_id}")
        connector_ids.add(connector_id)
        _assert_frame(connector, f"{recipe_id}/connectors/{connector_id}")
    _assert_frame(recipe["pivot"], f"{recipe_id}/pivot")
    _assert_local_transform(recipe["root_local_transform"], f"{recipe_id}/root_local_transform")

    zone_ids: set[str] = set()
    for zone in recipe["material_zones"]:
        if zone["zone_id"] in zone_ids:
            _fail("C105_DUPLICATE_MATERIAL_ZONE", f"{recipe_id}/{zone['zone_id']}")
        zone_ids.add(zone["zone_id"])

    slot_ids: set[str] = set()
    for slot in recipe["child_slots"]:
        slot_id = slot["slot_id"]
        if slot_id in slot_ids:
            _fail("C105_DUPLICATE_CHILD_SLOT", f"{recipe_id}/{slot_id}")
        slot_ids.add(slot_id)
        if slot["parent_connector_id"] not in connector_ids:
            _fail("C105_PARENT_CONNECTOR_MISSING", f"{recipe_id}/{slot_id}")
        if "parent_local_transform" not in slot:
            _fail("C105_PARENT_LOCAL_TRANSFORM_MISSING", f"{recipe_id}/{slot_id}")
        _assert_local_transform(slot["parent_local_transform"], f"{recipe_id}/child_slots/{slot_id}/parent_local_transform")

    for path, _child in _walk(recipe):
        if path and "world" in path[-1].casefold():
            _fail("C105_WORLD_TRANSFORM_FORBIDDEN", f"{recipe_id}/{'/'.join(path)}")


def _recipe_index(registry: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    recipes = registry["recipes"]
    index: dict[str, Mapping[str, Any]] = {}
    version_keys: set[tuple[str, int]] = set()
    for recipe in recipes:
        recipe_id = recipe["recipe_id"]
        version_key = (recipe_id, recipe["version"])
        if version_key in version_keys:
            _fail("C105_DUPLICATE_RECIPE_VERSION", f"{recipe_id}@{recipe['version']}")
        version_keys.add(version_key)
        if recipe_id in index:
            _fail("C105_AMBIGUOUS_RECIPE_VERSION", f"child slots require one active version for {recipe_id}")
        index[recipe_id] = recipe
    return index


def _assert_recipe_graph(registry: Mapping[str, Any]) -> dict[str, list[str]]:
    index = _recipe_index(registry)
    adjacency: dict[str, list[str]] = {recipe_id: [] for recipe_id in index}
    for parent_id, parent in index.items():
        parent_domains = set(parent["allowed_domains"])
        for slot in parent["child_slots"]:
            child_id = slot["child_recipe_id"]
            child = index.get(child_id)
            if child is None:
                _fail("C105_CHILD_RECIPE_MISSING", f"{parent_id}/{slot['slot_id']} -> {child_id}")
            if child["component_role"] not in slot["accepted_roles"]:
                _fail("C105_CHILD_ROLE_INCOMPATIBLE", f"{parent_id}/{slot['slot_id']} -> {child_id}")
            child_domains = set(child["allowed_domains"])
            if not parent_domains.issubset(child_domains):
                _fail("C105_CHILD_DOMAIN_INCOMPATIBLE", f"{parent_id}/{slot['slot_id']} -> {child_id}")
            child_connectors = {item["connector_id"] for item in child["connectors"]}
            if slot["child_connector_id"] not in child_connectors:
                _fail("C105_CHILD_CONNECTOR_MISSING", f"{parent_id}/{slot['slot_id']} -> {child_id}")
            adjacency[parent_id].append(child_id)

    state: dict[str, int] = {}

    def visit(recipe_id: str, stack: tuple[str, ...]) -> None:
        if state.get(recipe_id) == 1:
            _fail("C105_RECIPE_CYCLE", " -> ".join((*stack, recipe_id)))
        if state.get(recipe_id) == 2:
            return
        state[recipe_id] = 1
        for child_id in adjacency[recipe_id]:
            visit(child_id, (*stack, recipe_id))
        state[recipe_id] = 2

    for recipe_id in sorted(index):
        visit(recipe_id, ())
    return adjacency


def _assert_budgets(registry: Mapping[str, Any], adjacency: Mapping[str, list[str]]) -> None:
    index = _recipe_index(registry)

    def aggregate(recipe_id: str, depth: int) -> tuple[int, int, int]:
        if depth > MAX_CHILD_DEPTH:
            _fail("C105_CHILD_DEPTH_BUDGET_EXCEEDED", recipe_id)
        recipe = index[recipe_id]
        instances = 1
        triangles = int(recipe["triangle_estimate"])
        features = len(recipe["feature_template"])
        for child_id in adjacency[recipe_id]:
            child_instances, child_triangles, child_features = aggregate(child_id, depth + 1)
            instances += child_instances
            triangles += child_triangles
            features += child_features
        if instances > MAX_EXPANDED_INSTANCES:
            _fail("C105_INSTANCE_BUDGET_EXCEEDED", f"{recipe_id}: {instances}")
        if triangles > MAX_EXPANDED_TRIANGLES:
            _fail("C105_TRIANGLE_BUDGET_EXCEEDED", f"{recipe_id}: {triangles}")
        if features > MAX_EXPANDED_FEATURES:
            _fail("C105_FEATURE_BUDGET_EXCEEDED", f"{recipe_id}: {features}")
        return instances, triangles, features

    child_ids = {child_id for children in adjacency.values() for child_id in children}
    roots = sorted(set(index) - child_ids)
    if not roots:
        _fail("C105_ROOT_RECIPE_MISSING", "registry graph has no roots")
    for root in roots:
        aggregate(root, 0)


def _assert_four_domain_coverage(registry: Mapping[str, Any], recipe_schema: Mapping[str, Any]) -> set[str]:
    expected_domains = set(recipe_schema["$defs"]["domain"]["enum"])
    actual_domains = {domain for recipe in registry["recipes"] for domain in recipe["allowed_domains"]}
    if len(expected_domains) != 4 or actual_domains != expected_domains:
        _fail("C105_DOMAIN_COVERAGE_INCOMPLETE", f"expected {sorted(expected_domains)}, received {sorted(actual_domains)}")
    for domain in sorted(expected_domains):
        recipes = [recipe for recipe in registry["recipes"] if domain in recipe["allowed_domains"]]
        if not any(recipe["component_role"] != "visual_detail" and recipe["child_slots"] for recipe in recipes):
            _fail("C105_DOMAIN_ROOT_RECIPE_MISSING", domain)
        if not any(recipe["component_role"] == "visual_detail" for recipe in recipes):
            _fail("C105_DOMAIN_DETAIL_RECIPE_MISSING", domain)
    return expected_domains


def validate_registry(
    registry: Mapping[str, Any],
    *,
    expected_registry_sha256: str,
    verify_canonical_hash: bool = True,
) -> dict[str, Any]:
    recipe_schema, validator = _assert_c105_schemas()
    digest = (
        _assert_registry_envelope(registry, expected_registry_sha256)
        if verify_canonical_hash
        else canonical_sha256(registry)
    )
    runtime_operations = _runtime_operations(recipe_schema)
    for recipe in registry["recipes"]:
        _assert_schema_recipe(validator, recipe)
        _assert_recipe_local_contract(recipe, runtime_operations)
    _assert_four_domain_coverage(registry, recipe_schema)
    adjacency = _assert_recipe_graph(registry)
    _assert_budgets(registry, adjacency)
    return {
        "registry_sha256": digest,
        "recipes": len(registry["recipes"]),
        "domains": len({domain for recipe in registry["recipes"] for domain in recipe["allowed_domains"]}),
        "operations": len(runtime_operations),
        "edges": sum(len(children) for children in adjacency.values()),
    }


def _assert_expanded_golden(
    golden: Mapping[str, Any],
    registry: Mapping[str, Any],
) -> list[Mapping[str, Any]]:
    expected_keys = {"schema_version", "registry_sha256", "candidate_hash_scope", "candidates"}
    if set(golden) != expected_keys:
        _fail("C105_EXPANDED_GOLDEN_NOT_CLOSED", f"unexpected keys: {sorted(set(golden) ^ expected_keys)}")
    if golden.get("schema_version") != EXPANSION_GOLDEN_SCHEMA_VERSION:
        _fail("C105_EXPANDED_GOLDEN_VERSION_UNSUPPORTED", str(golden.get("schema_version")))
    if golden.get("candidate_hash_scope") != CANDIDATE_HASH_SCOPE:
        _fail("C105_CANDIDATE_HASH_SCOPE_INVALID", str(golden.get("candidate_hash_scope")))
    registry_sha256 = canonical_sha256(registry)
    if golden.get("registry_sha256") != registry_sha256:
        _fail("C105_GOLDEN_REGISTRY_HASH_MISMATCH", f"expected {registry_sha256}")
    candidates = golden.get("candidates")
    if not isinstance(candidates, list) or len(candidates) != 4:
        _fail("C105_EXPANDED_GOLDEN_INCOMPLETE", "exactly four initial candidates are required")

    candidate_validator = _schema_validator(SCHEMA_DIR / "component-recipe-candidate.schema.json")
    recipe_index = _recipe_index(registry)
    expected_domains = set(_load_json(RECIPE_SCHEMA_PATH)["$defs"]["domain"]["enum"])
    observed_domains: set[str] = set()
    observed_root_recipes: set[str] = set()
    observed_candidate_ids: set[str] = set()
    observed_recipe_hashes: dict[str, str] = {}
    for raw_candidate in candidates:
        if not isinstance(raw_candidate, Mapping):
            _fail("C105_EXPANDED_GOLDEN_INVALID", "candidate must be an object")
        candidate = dict(raw_candidate)
        candidate_id = str(candidate.get("candidate_id", "<unknown>"))
        _assert_schema_value(
            candidate_validator,
            candidate,
            code="C105_EXPANDED_CANDIDATE_SCHEMA_INVALID",
            identity=candidate_id,
        )
        if candidate_id in observed_candidate_ids:
            _fail("C105_EXPANDED_CANDIDATE_DUPLICATE", candidate_id)
        observed_candidate_ids.add(candidate_id)
        if candidate.get("context_mode") != "initial_candidate" or any(
            candidate.get(field) is not None
            for field in ("project_id", "base_asset_version_id", "snapshot_revision", "target_part_id")
        ):
            _fail("C105_INITIAL_CONTEXT_INVALID", candidate_id)
        if candidate.get("changeset_id") is not None or candidate.get("status") != "expanded":
            _fail("C105_INITIAL_CANDIDATE_SIDE_EFFECT", candidate_id)
        if candidate.get("registry_sha256") != registry_sha256:
            _fail("C105_CANDIDATE_REGISTRY_HASH_MISMATCH", candidate_id)

        recipe_ref = candidate.get("recipe")
        if not isinstance(recipe_ref, Mapping):
            _fail("C105_CANDIDATE_RECIPE_REF_INVALID", candidate_id)
        root_recipe_id = str(recipe_ref.get("recipe_id"))
        root_recipe = recipe_index.get(root_recipe_id)
        if root_recipe is None:
            _fail("C105_CANDIDATE_RECIPE_REF_INVALID", root_recipe_id)
        if (
            recipe_ref.get("version") != root_recipe["version"]
            or not isinstance(recipe_ref.get("recipe_sha256"), str)
            or not re.fullmatch(r"[a-f0-9]{64}", recipe_ref["recipe_sha256"])
        ):
            _fail("C105_CANDIDATE_RECIPE_HASH_MISMATCH", root_recipe_id)
        observed_recipe_hashes[root_recipe_id] = recipe_ref["recipe_sha256"]
        if root_recipe_id in observed_root_recipes:
            _fail("C105_EXPANDED_ROOT_DUPLICATE", root_recipe_id)
        observed_root_recipes.add(root_recipe_id)

        instances = candidate.get("component_recipe_instances")
        if not isinstance(instances, list) or len(instances) < 2:
            _fail("C105_CANDIDATE_INSTANCE_PROVENANCE_INCOMPLETE", candidate_id)
        assembly = candidate.get("expanded_assembly_graph")
        if (
            not isinstance(assembly, Mapping)
            or assembly.get("component_recipe_instances") != instances
            or len(assembly.get("parts", [])) != len(instances)
            or len(assembly.get("connections", [])) != len(instances) - 1
        ):
            _fail("C105_CANDIDATE_ASSEMBLY_PROVENANCE_DRIFT", candidate_id)
        instance_paths: set[str] = set()
        for instance in instances:
            if instance.get("registry_sha256") != registry_sha256:
                _fail("C105_INSTANCE_REGISTRY_HASH_MISMATCH", candidate_id)
            instance_ref = instance.get("recipe")
            instance_recipe = recipe_index.get(instance_ref.get("recipe_id")) if isinstance(instance_ref, Mapping) else None
            if (
                instance_recipe is None
                or instance_ref.get("version") != instance_recipe["version"]
                or not isinstance(instance_ref.get("recipe_sha256"), str)
                or not re.fullmatch(r"[a-f0-9]{64}", instance_ref["recipe_sha256"])
            ):
                _fail("C105_INSTANCE_RECIPE_HASH_MISMATCH", candidate_id)
            prior_hash = observed_recipe_hashes.setdefault(
                instance_ref["recipe_id"], instance_ref["recipe_sha256"]
            )
            if prior_hash != instance_ref["recipe_sha256"]:
                _fail("C105_INSTANCE_RECIPE_HASH_MISMATCH", candidate_id)
            if instance.get("policy_version") != POLICY_VERSION or instance.get("quality_status") != "passed":
                _fail("C105_INSTANCE_POLICY_PROVENANCE_INVALID", candidate_id)
            path = str(instance.get("instance_path"))
            if path in instance_paths:
                _fail("C105_INSTANCE_PATH_DUPLICATE", f"{candidate_id}/{path}")
            instance_paths.add(path)
            observed_domains.add(str(instance.get("domain_pack_id")))
        if candidate.get("instance_path") != "root" or "root" not in instance_paths:
            _fail("C105_ROOT_INSTANCE_PATH_INVALID", candidate_id)

        candidate_hash_payload = copy.deepcopy(candidate)
        candidate_hash_payload["candidate_sha256"] = ""
        if canonical_sha256(candidate_hash_payload) != candidate.get("candidate_sha256"):
            _fail("C105_CANDIDATE_CANONICAL_HASH_MISMATCH", candidate_id)

    if observed_domains != expected_domains:
        _fail("C105_GOLDEN_DOMAIN_COVERAGE_INCOMPLETE", f"received {sorted(observed_domains)}")
    if set(observed_recipe_hashes) != set(recipe_index):
        _fail(
            "C105_GOLDEN_RECIPE_HASH_COVERAGE_INCOMPLETE",
            f"missing {sorted(set(recipe_index) - set(observed_recipe_hashes))}",
        )
    return candidates


def _compile_expanded_golden_candidates(
    candidates: Sequence[Mapping[str, Any]],
    registry: Mapping[str, Any],
) -> list[dict[str, Any]]:
    agent_root = ROOT / "apps/agent"
    scripts_root = ROOT / "scripts"
    for path in (agent_root, scripts_root):
        if str(path) not in sys.path:
            sys.path.insert(0, str(path))
    try:
        from forgecad_agent.application.restricted_geometry_executor import (
            RestrictedGeometryExecutionRequest,
            RestrictedGeometryExecutor,
        )
        from forgecad_gate_rust_python_contract import _parse_glb, verify_source_face_provenance
    except ImportError as exc:
        _fail("C105_RESTRICTED_GEOMETRY_UNAVAILABLE", str(exc))

    recipe_index = _recipe_index(registry)
    executor = RestrictedGeometryExecutor(environment={})
    reports: list[dict[str, Any]] = []
    for index, candidate in enumerate(candidates):
        candidate_id = str(candidate["candidate_id"])
        program = candidate["expanded_shape_program"]
        request = RestrictedGeometryExecutionRequest.model_validate(
            {
                "schema_version": "RestrictedGeometryExecutionRequest@1",
                "protocol_version": "forgecad.restricted-geometry/1",
                "execution_id": f"exec_c105_golden_{index}",
                "idempotency_key": f"idem_c105_golden_{index}",
                "cancellation_id": f"cancel_c105_golden_{index}",
                "cancellation_token": f"cancel_token_c105_golden_{index}",
                "action": "compile_readback",
                "timeout_ms": 120_000,
                "artifact_profile_id": "production_concept",
                "shape_program": program,
            }
        )
        boundary_payload = request.model_dump(mode="json", exclude_none=True)
        high_level_keys = {
            "assembly",
            "candidate_sha256",
            "component_recipe",
            "component_recipe_instances",
            "context_mode",
            "expanded_assembly_graph",
            "project_id",
            "recipe",
            "recipe_assembly_graph",
            "recipe_candidate_sha256",
            "recipe_component_instances",
            "registry_sha256",
            "snapshot_revision",
            "target_part_id",
        }
        leaked_keys = {
            path[-1]
            for path, _value in _walk(boundary_payload)
            if path and path[-1] in high_level_keys
        }
        if leaked_keys:
            _fail("C105_PYTHON_BOUNDARY_CONTEXT_LEAK", f"{candidate_id}: {sorted(leaked_keys)}")
        try:
            result = executor.execute(request)
        except Exception as exc:
            _fail("C105_EXPANDED_PROGRAM_COMPILE_FAILED", f"{candidate_id}: {exc}")
        if not result.glb_base64 or not isinstance(result.readback, Mapping):
            _fail("C105_EXPANDED_PROGRAM_READBACK_MISSING", candidate_id)
        try:
            glb = base64.b64decode(result.glb_base64, validate=True)
        except (ValueError, binascii.Error):
            _fail("C105_EXPANDED_GLB_INVALID_BASE64", candidate_id)
        if hashlib.sha256(glb).hexdigest() != result.glb_sha256 or len(glb) != result.glb_byte_size:
            _fail("C105_EXPANDED_GLB_IDENTITY_MISMATCH", candidate_id)
        if result.shape_program_sha256 != canonical_sha256(program):
            _fail("C105_EXPANDED_PROGRAM_HASH_MISMATCH", candidate_id)
        readback = result.readback
        if (
            readback.get("shape_program_sha256") != result.shape_program_sha256
            or readback.get("glb_sha256") != result.glb_sha256
            or readback.get("artifact_profile", {}).get("artifact_profile_id") != "production_concept"
            or readback.get("closed_manifold") is not True
            or readback.get("surface_provenance_present") is not True
        ):
            _fail("C105_EXPANDED_READBACK_IDENTITY_MISMATCH", candidate_id)

        expected_zones: Counter[tuple[str, str]] = Counter()
        triangle_budget = 0
        for instance in candidate["component_recipe_instances"]:
            recipe = recipe_index[instance["recipe"]["recipe_id"]]
            triangle_budget += int(recipe["triangle_estimate"])
            expected_zones.update(
                (zone["zone_id"], zone["material_preset_id"])
                for zone in recipe["material_zones"]
            )
        actual_zones = Counter(
            (item.get("material_zone_id"), item.get("material_id"))
            for item in readback.get("material_zone_faces", [])
        )
        surface_zones = Counter(
            item.get("material_zone_id")
            for item in readback.get("surface_provenance", [])
        )
        if actual_zones != expected_zones:
            _fail("C105_MATERIAL_ZONE_READBACK_DRIFT", f"{candidate_id}: {actual_zones} != {expected_zones}")
        if surface_zones != Counter(zone for zone, _material in expected_zones.elements()):
            _fail("C105_SURFACE_ZONE_READBACK_DRIFT", candidate_id)
        if (
            result.triangle_count > triangle_budget
            or result.triangle_count > int(program.get("triangle_budget", 0))
            or result.triangle_count < 1
        ):
            _fail("C105_COMPILED_TRIANGLE_BUDGET_EXCEEDED", candidate_id)

        try:
            document, binary = _parse_glb(glb)
            provenance = verify_source_face_provenance(document, binary)
        except Exception as exc:
            _fail("C105_GLB_PROVENANCE_INVALID", f"{candidate_id}: {exc}")
        if provenance["triangle_count"] != result.triangle_count:
            _fail("C105_GLB_TRIANGLE_READBACK_DRIFT", candidate_id)
        reports.append(
            {
                "candidate_id": candidate_id,
                "glb_sha256": result.glb_sha256,
                "triangles": result.triangle_count,
                "zones": sum(actual_zones.values()),
            }
        )
    return reports


def _expect_failure(action: Any, expected_code: str) -> None:
    try:
        action()
    except C105GateFailure as exc:
        if exc.code != expected_code:
            _fail("C105_SELF_TEST_WRONG_FAILURE", f"expected {expected_code}, received {exc.code}")
        return
    _fail("C105_SELF_TEST_FALSE_NEGATIVE", f"mutation passed: {expected_code}")


def run_self_tests(registry: Mapping[str, Any], *, expected_registry_sha256: str) -> dict[str, str]:
    recipe_schema, _validator = _assert_c105_schemas()
    runtime_operations = _runtime_operations(recipe_schema)
    results: dict[str, str] = {}

    cycle = copy.deepcopy(registry)
    cycle["recipes"][0]["child_slots"][0]["child_recipe_id"] = cycle["recipes"][0]["recipe_id"]
    cycle["recipes"][0]["child_slots"][0]["accepted_roles"] = [cycle["recipes"][0]["component_role"]]
    cycle["recipes"][0]["child_slots"][0]["child_connector_id"] = cycle["recipes"][0]["connectors"][0]["connector_id"]
    _expect_failure(lambda: _assert_recipe_graph(cycle), "C105_RECIPE_CYCLE")
    results["cycle"] = "rejected"

    cross_domain = copy.deepcopy(registry)
    child_id = cross_domain["recipes"][0]["child_slots"][0]["child_recipe_id"]
    child = next(recipe for recipe in cross_domain["recipes"] if recipe["recipe_id"] == child_id)
    parent_domain = cross_domain["recipes"][0]["allowed_domains"][0]
    child["allowed_domains"] = [
        domain for domain in recipe_schema["$defs"]["domain"]["enum"] if domain != parent_domain
    ][:1]
    _expect_failure(lambda: _assert_recipe_graph(cross_domain), "C105_CHILD_DOMAIN_INCOMPATIBLE")
    results["cross_domain"] = "rejected"

    unsupported = copy.deepcopy(registry)
    unsupported["recipes"][0]["shape_program_template"]["operations"][0]["op"] = "execute_arbitrary_code"
    _expect_failure(
        lambda: _assert_recipe_local_contract(unsupported["recipes"][0], runtime_operations),
        "C105_UNSUPPORTED_RUNTIME_OPERATION",
    )
    results["unsupported_op"] = "rejected"

    bad_frame = copy.deepcopy(registry)
    bad_frame["recipes"][0]["connectors"][0]["up"] = bad_frame["recipes"][0]["connectors"][0]["normal"]
    _expect_failure(
        lambda: _assert_recipe_local_contract(bad_frame["recipes"][0], runtime_operations),
        "C105_FRAME_NOT_ORTHOGONAL",
    )
    results["bad_frame"] = "rejected"

    unreviewed = copy.deepcopy(registry)
    unreviewed["recipes"][0]["review_state"]["reviewer_kind"] = "project_local_unreviewed"
    _expect_failure(
        lambda: _assert_review_source_license_quality(unreviewed["recipes"][0]),
        "C105_REVIEW_NOT_READY",
    )
    results["unreviewed"] = "rejected"

    bad_license = copy.deepcopy(registry)
    bad_license["recipes"][0]["license"]["license_id"] = "Unknown"
    _expect_failure(
        lambda: _assert_review_source_license_quality(bad_license["recipes"][0]),
        "C105_LICENSE_INVALID",
    )
    results["bad_license"] = "rejected"

    failed_quality = copy.deepcopy(registry)
    failed_quality["recipes"][0]["quality_status"] = "failed"
    _expect_failure(
        lambda: _assert_review_source_license_quality(failed_quality["recipes"][0]),
        "C105_QUALITY_NOT_PASSED",
    )
    results["failed_quality"] = "rejected"

    invalid_transform = copy.deepcopy(registry)
    invalid_transform["recipes"][0]["root_local_transform"]["scale"][0] = 0
    _expect_failure(
        lambda: _assert_local_transform(
            invalid_transform["recipes"][0]["root_local_transform"],
            "self_test/root_local_transform",
        ),
        "C105_TRANSFORM_INVALID",
    )
    results["invalid_transform"] = "rejected"

    resource_tamper = copy.deepcopy(registry)
    resource = resource_tamper["recipes"][0]["shape_program_template"]["profile_inputs"][0]
    resource["canonical_payload"]["provenance"]["source_ref"] += "_tampered"
    _expect_failure(
        lambda: _assert_embedded_profile_resources(resource_tamper["recipes"][0]),
        "C105_PROFILE_INPUT_HASH_MISMATCH",
    )
    results["resource_hash_tamper"] = "rejected"

    forbidden_path = copy.deepcopy(registry)
    forbidden_path["recipes"][0]["description"] = "load ../private/recipe.py"
    _expect_failure(
        lambda: _assert_no_executable_or_location_payload(forbidden_path["recipes"][0]),
        "C105_FORBIDDEN_RELATIVE_PATH",
    )
    results["forbidden_path"] = "rejected"

    forbidden_url = copy.deepcopy(registry)
    forbidden_url["recipes"][0]["description"] = "https://example.invalid/payload"
    _expect_failure(
        lambda: _assert_no_executable_or_location_payload(forbidden_url["recipes"][0]),
        "C105_FORBIDDEN_URL",
    )
    results["forbidden_url"] = "rejected"

    budget = copy.deepcopy(registry)
    budget["recipes"][0]["triangle_estimate"] = MAX_EXPANDED_TRIANGLES
    budget_adjacency = _assert_recipe_graph(budget)
    _expect_failure(
        lambda: _assert_budgets(budget, budget_adjacency),
        "C105_TRIANGLE_BUDGET_EXCEEDED",
    )
    results["aggregate_budget"] = "rejected"

    hash_drift = copy.deepcopy(registry)
    hash_drift["recipes"][0]["display_name"] += " drift"
    _expect_failure(
        lambda: _assert_registry_envelope(hash_drift, expected_registry_sha256),
        "C105_REGISTRY_CANONICAL_HASH_MISMATCH",
    )
    results["canonical_hash_drift"] = "rejected"
    return results


def run_golden_self_tests(golden: Mapping[str, Any], registry: Mapping[str, Any]) -> dict[str, str]:
    results: dict[str, str] = {}

    candidate_validator = _schema_validator(
        SCHEMA_DIR / "component-recipe-candidate.schema.json"
    )
    active_context = copy.deepcopy(golden["candidates"][0])
    active_context.update(
        {
            "context_mode": "active_asset_edit",
            "project_id": "prj_c105_active_context",
            "base_asset_version_id": "assetver_c105_active_base",
            "snapshot_revision": 7,
            "target_part_id": "part_c105_active_target",
        }
    )
    active_context["candidate_sha256"] = ""
    active_context["candidate_sha256"] = canonical_sha256(active_context)
    _assert_schema_value(
        candidate_validator,
        active_context,
        code="C105_ACTIVE_CONTEXT_SCHEMA_INVALID",
        identity="active_asset_edit",
    )

    active_missing_snapshot = copy.deepcopy(active_context)
    active_missing_snapshot["snapshot_revision"] = None
    active_missing_snapshot["candidate_sha256"] = ""
    active_missing_snapshot["candidate_sha256"] = canonical_sha256(
        active_missing_snapshot
    )
    _expect_failure(
        lambda: _assert_schema_value(
            candidate_validator,
            active_missing_snapshot,
            code="C105_ACTIVE_CONTEXT_SCHEMA_INVALID",
            identity="active_asset_edit_missing_snapshot",
        ),
        "C105_ACTIVE_CONTEXT_SCHEMA_INVALID",
    )
    results["active_missing_snapshot"] = "rejected"

    candidate_hash = copy.deepcopy(golden)
    candidate_hash["candidates"][0]["candidate_sha256"] = "0" * 64
    _expect_failure(
        lambda: _assert_expanded_golden(candidate_hash, registry),
        "C105_CANDIDATE_CANONICAL_HASH_MISMATCH",
    )
    results["candidate_hash_tamper"] = "rejected"

    mixed_context = copy.deepcopy(golden)
    mixed_context["candidates"][0]["project_id"] = "prj_forged_active_context"
    _expect_failure(
        lambda: _assert_expanded_golden(mixed_context, registry),
        "C105_EXPANDED_CANDIDATE_SCHEMA_INVALID",
    )
    results["mixed_context"] = "rejected"

    registry_pin = copy.deepcopy(golden)
    registry_pin["registry_sha256"] = "0" * 64
    _expect_failure(
        lambda: _assert_expanded_golden(registry_pin, registry),
        "C105_GOLDEN_REGISTRY_HASH_MISMATCH",
    )
    results["golden_registry_hash_tamper"] = "rejected"
    return results


def _assert_and_run_rust_engine() -> dict[str, Any]:
    source_candidates = sorted(
        path
        for path in (RUST_CORE_ROOT / "src").glob("**/*.rs")
        if path.stem in {"component_recipe", "component_recipes"}
        or "component_recipe" in path.parts
        or "component_recipes" in path.parts
    )
    test_candidates = sorted((RUST_CORE_ROOT / "tests").glob("*component_recipe*.rs"))
    lib_path = RUST_CORE_ROOT / "src/lib.rs"
    lib_text = lib_path.read_text(encoding="utf-8") if lib_path.is_file() else ""
    if not source_candidates or "component_recipe" not in lib_text:
        _fail("C105_RUST_ENGINE_MISSING", "forgecad-core does not expose a component recipe module")
    if not test_candidates and not any("#[cfg(test)]" in path.read_text(encoding="utf-8") for path in source_candidates):
        _fail("C105_RUST_ENGINE_TEST_MISSING", "focused component recipe tests are absent")

    required_targets = {
        "component_recipe_contract",
        "component_recipe_expansion_golden",
        "component_recipe_repository",
    }
    target_names = {path.stem for path in test_candidates}
    if not required_targets.issubset(target_names):
        _fail("C105_RUST_ENGINE_TEST_MISSING", f"missing targets: {sorted(required_targets - target_names)}")
    passed = 0
    for test_path in test_candidates:
        command = [
            str(ROOT / "script/with_rust_toolchain.sh"),
            "cargo",
            "test",
            "--manifest-path",
            str(RUST_MANIFEST_PATH),
            "-p",
            "forgecad-core",
            "--test",
            test_path.stem,
            "--offline",
        ]
        try:
            completed = subprocess.run(
                command,
                cwd=ROOT,
                capture_output=True,
                text=True,
                timeout=600,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            _fail("C105_RUST_ENGINE_UNAVAILABLE", str(exc))
        output = f"{completed.stdout}\n{completed.stderr}"
        if completed.returncode != 0:
            _fail("C105_RUST_ENGINE_TEST_FAILED", f"{test_path.stem}: {output[-4000:]}")
        target_passed = sum(int(value) for value in re.findall(r"test result: ok\. (\d+) passed", output))
        if target_passed < 1:
            _fail("C105_RUST_ENGINE_TEST_MISSING", f"{test_path.stem} executed zero tests")
        passed += target_passed

    carrier_command = [
        str(ROOT / "script/with_rust_toolchain.sh"),
        "cargo",
        "test",
        "--manifest-path",
        str(RUST_MANIFEST_PATH),
        "-p",
        "forgecad-app-server",
        "--lib",
        "--offline",
        "component_recipe_preview_carrier",
    ]
    try:
        carrier_completed = subprocess.run(
            carrier_command,
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=600,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        _fail("C105_APP_SERVER_CARRIER_UNAVAILABLE", str(exc))
    carrier_output = f"{carrier_completed.stdout}\n{carrier_completed.stderr}"
    if carrier_completed.returncode != 0:
        _fail("C105_APP_SERVER_CARRIER_TEST_FAILED", carrier_output[-4000:])
    carrier_passed = sum(
        int(value) for value in re.findall(r"test result: ok\. (\d+) passed", carrier_output)
    )
    if carrier_passed < 1 or "component_recipe_preview_carrier" not in carrier_output:
        _fail(
            "C105_APP_SERVER_CARRIER_TEST_MISSING",
            "app-server must prove Recipe assembly/provenance retention and Python boundary stripping",
        )
    passed += carrier_passed
    return {
        "focused_tests_passed": passed,
        "source_files": len(source_candidates),
        "test_files": len(test_candidates),
        "app_server_carrier_tests": carrier_passed,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run only the deterministic fault-injection suite (no Cargo invocation).",
    )
    parser.add_argument(
        "--compile-golden-only",
        action="store_true",
        help="Validate and production-compile the Rust golden without running Cargo (diagnostic only).",
    )
    args = parser.parse_args()

    registry = _load_json(REGISTRY_PATH)
    if not isinstance(registry, Mapping):
        _fail("C105_REGISTRY_INVALID", "registry root must be an object")
    golden = _load_json(EXPANDED_GOLDEN_PATH)
    if not isinstance(golden, Mapping):
        _fail("C105_EXPANDED_GOLDEN_INVALID", "expanded golden root must be an object")
    expected_registry_sha256 = _expected_registry_sha256(golden)

    if args.self_test:
        validate_registry(registry, expected_registry_sha256=expected_registry_sha256)
        mutations = run_self_tests(registry, expected_registry_sha256=expected_registry_sha256)
        _assert_expanded_golden(golden, registry)
        mutations.update(run_golden_self_tests(golden, registry))
        print(
            "C105 component recipe self-test passed: "
            + ", ".join(f"{name}={result}" for name, result in sorted(mutations.items()))
        )
        return 0

    if args.compile_golden_only:
        validate_registry(registry, expected_registry_sha256=expected_registry_sha256)
        candidates = _assert_expanded_golden(golden, registry)
        compiled = _compile_expanded_golden_candidates(candidates, registry)
        print(
            "C105 component recipe production golden passed: "
            f"production_glbs={len(compiled)}, "
            f"compiled_triangles={sum(item['triangles'] for item in compiled)}, provider_calls=0"
        )
        return 0

    contract = validate_registry(registry, expected_registry_sha256=expected_registry_sha256)
    mutations = run_self_tests(registry, expected_registry_sha256=expected_registry_sha256)
    candidates = _assert_expanded_golden(golden, registry)
    mutations.update(run_golden_self_tests(golden, registry))
    rust = _assert_and_run_rust_engine()
    compiled = _compile_expanded_golden_candidates(candidates, registry)
    print(
        "C105 component recipe gate passed: "
        f"recipes={contract['recipes']}, domains={contract['domains']}, "
        f"G819_operations={contract['operations']}, child_edges={contract['edges']}, "
        f"registry_sha256={contract['registry_sha256']}, "
        f"negative_cases={len(mutations)}, rust_tests={rust['focused_tests_passed']}, "
        f"production_glbs={len(compiled)}, compiled_triangles={sum(item['triangles'] for item in compiled)}, "
        "provider_calls=0"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except C105GateFailure as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(1) from None
