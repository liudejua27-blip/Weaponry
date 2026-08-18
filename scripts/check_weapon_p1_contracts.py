#!/usr/bin/env python3
"""Focused checker for the P1 visual cross-section contract slice.

This checker complements the repository-wide manifest gate with semantic,
cross-contract, canonical-hash, and positive/negative fixture checks for the
three registered P1 schemas.
"""

from __future__ import annotations

import copy
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "forgecad-contracts" / "schemas"
FIXTURE_ROOT = ROOT / "packages" / "forgecad-contracts" / "fixtures" / "p1"

# Reuse the repository's dependency-free Draft-2020-12 subset validator.  It
# is also used by the existing Agentic contract checker and keeps this focused
# gate runnable in a clean Python installation.
sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid  # noqa: E402


EXPECTED = {
    "cross-section-plan.schema.json": "CrossSectionPlan@1",
    "sketch-program.schema.json": "SketchProgram@1",
    "weapon-design-graph.schema.json": "WeaponDesignGraph@1",
}

POSITIVE = {
    "cross-section-plan.schema.json": "cross-section-plan.json",
    "sketch-program.schema.json": "sketch-program.json",
    "weapon-design-graph.schema.json": "weapon-design-graph.json",
}

NEGATIVE = {
    "cross-section-plan.schema.json": "cross-section-plan-unsafe-field.json",
    "sketch-program.schema.json": "sketch-program-script-field.json",
    "weapon-design-graph.schema.json": "weapon-design-graph-runtime-write.json",
}

FORBIDDEN_PROPERTY_NAMES = {
    "path",
    "url",
    "uri",
    "raw",
    "raw_bytes",
    "bytes",
    "secret",
    "token",
    "password",
    "api_key",
    "prompt",
    "script",
    "shell",
    "environment",
    "manufacturing",
    "manufacturing_tolerance_m",
    "tolerance",
    "propulsion",
}


def fail(message: str) -> None:
    raise SystemExit(f"Weapon P1 contract violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path.relative_to(ROOT)}: {exc}")


def load_object(path: Path) -> dict[str, Any]:
    value = load_json(path)
    require(isinstance(value, dict), f"{path.relative_to(ROOT)} must be a JSON object")
    return value


def canonical_contract_hash(value: dict[str, Any]) -> str:
    payload = copy.deepcopy(value)
    payload["canonical_sha256"] = ""
    encoded = json.dumps(
        payload,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def walk_objects(node: Any, path: str = "$") -> list[tuple[str, dict[str, Any]]]:
    found: list[tuple[str, dict[str, Any]]] = []
    if not isinstance(node, dict):
        return found
    if node.get("type") == "object":
        found.append((path, node))
    for key, child in node.items():
        if key in {"properties", "$defs", "definitions"} and isinstance(child, dict):
            for name, value in child.items():
                found.extend(walk_objects(value, f"{path}.{key}.{name}"))
        elif key in {"items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"}:
            if isinstance(child, list):
                for index, value in enumerate(child):
                    found.extend(walk_objects(value, f"{path}.{key}[{index}]"))
            else:
                found.extend(walk_objects(child, f"{path}.{key}"))
    return found


def walk_property_names(node: Any) -> list[str]:
    names: list[str] = []
    if not isinstance(node, dict):
        return names
    properties = node.get("properties")
    if isinstance(properties, dict):
        names.extend(properties)
        for value in properties.values():
            names.extend(walk_property_names(value))
    for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                names.extend(walk_property_names(value))
        elif isinstance(child, dict):
            names.extend(walk_property_names(child))
    return names


def check_schema_shape(filename: str, schema: dict[str, Any]) -> None:
    version = EXPECTED[filename]
    require(
        schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema",
        f"{filename} is not draft 2020-12",
    )
    require(
        schema.get("$id") == f"https://forgecad.local/contracts/{filename}",
        f"{filename} has the wrong $id",
    )
    require(
        schema.get("type") == "object" and schema.get("additionalProperties") is False,
        f"{filename} root is not closed",
    )
    require(
        schema.get("properties", {}).get("schema_version", {}).get("const") == version,
        f"{filename} has the wrong schema_version",
    )
    require(
        set(schema.get("required", [])) >= {"schema_version", "canonical_sha256"},
        f"{filename} is not version/hash bound",
    )
    require(
        schema.get("$defs", {}).get("identifier", {}).get("pattern")
        == "^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$",
        f"{filename} identifier is not strict",
    )
    require(
        schema.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$",
        f"{filename} SHA-256 is not strict",
    )

    for path, object_schema in walk_objects(schema):
        require(
            object_schema.get("additionalProperties") is False,
            f"{filename} {path} is an open object",
        )

    forbidden = {name.lower() for name in FORBIDDEN_PROPERTY_NAMES}
    for name in walk_property_names(schema):
        require(name.lower() not in forbidden, f"{filename} exposes forbidden property {name}")

    def inspect_properties(node: Any) -> None:
        if not isinstance(node, dict):
            return
        properties = node.get("properties")
        if isinstance(properties, dict):
            for name, child in properties.items():
                if name.endswith("_id"):
                    require(
                        child.get("$ref") == "#/$defs/identifier"
                        or "pattern" in child
                        or "const" in child,
                        f"{filename}.{name} is not identifier constrained",
                    )
                if name.endswith("_sha256"):
                    require(
                        child.get("$ref") == "#/$defs/sha256"
                        or child.get("pattern") == "^[0-9a-f]{64}$"
                        or "const" in child,
                        f"{filename}.{name} is not SHA-256 constrained",
                    )
                inspect_properties(child)
        for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
            child = node.get(key)
            if isinstance(child, list):
                for value in child:
                    inspect_properties(value)
            elif isinstance(child, dict):
                inspect_properties(child)

    inspect_properties(schema)

    if filename == "weapon-design-graph.schema.json":
        require(
            schema.get("properties", {}).get("hq_360_status", {}).get("enum")
            == ["BLOCKED_REFERENCE_COVERAGE", "ELIGIBLE_FOR_HQ_360_REVIEW"],
            "WeaponDesignGraph HQ-360 status must separate coverage blocking from review eligibility",
        )


def check_cross_contract_bindings(positives: dict[str, dict[str, Any]]) -> None:
    plan = positives["cross-section-plan.schema.json"]
    sketch = positives["sketch-program.schema.json"]
    graph = positives["weapon-design-graph.schema.json"]

    require(
        plan["project_id"] == sketch["project_id"] == graph["project_id"],
        "positive project binding drifted",
    )
    require(
        plan["scope"] == sketch["scope"] == graph["scope"]
        and plan["nonfunctional_asset"]
        and sketch["nonfunctional_asset"]
        and graph["nonfunctional_asset"],
        "positive fictional/nonfunctional scope binding drifted",
    )
    require(
        sketch["cross_section_plan_sha256"] == plan["canonical_sha256"],
        "SketchProgram does not bind the CrossSectionPlan canonical hash",
    )
    require(
        graph["cross_section_plan_sha256"] == plan["canonical_sha256"]
        and graph["sketch_program_sha256"] == sketch["canonical_sha256"],
        "WeaponDesignGraph does not bind both upstream canonical hashes",
    )
    require(
        graph["reference_view_set_sha256"] == plan["reference_view_set_sha256"],
        "WeaponDesignGraph and CrossSectionPlan bind different ReferenceViewSets",
    )
    require(
        len(plan["stations"]) <= plan["station_policy"]["max_station_count"],
        "CrossSectionPlan exceeds its declared station policy",
    )
    station_ids = [station["station_id"] for station in plan["stations"]]
    require(len(station_ids) == len(set(station_ids)), "CrossSectionPlan station IDs are not unique")
    positions = [station["position_m"] for station in plan["stations"]]
    require(
        all(left < right for left, right in zip(positions, positions[1:])),
        "CrossSectionPlan stations are not strictly increasing along +X",
    )
    point_counts = [len(station["points"]) for station in plan["stations"]]
    if plan["station_policy"]["equal_point_count"]:
        require(
            len(set(point_counts)) == 1,
            "CrossSectionPlan declares equal point counts but stations drift",
        )
    reference_view_ids = set(plan["reference_view_ids"])
    require(
        all(set(station["source_view_ids"]) <= reference_view_ids for station in plan["stations"]),
        "CrossSectionPlan station references an undeclared ReferenceViewSet view",
    )

    sketch_ids = [item["sketch_id"] for item in sketch["sketches"]]
    require(len(sketch_ids) == len(set(sketch_ids)), "SketchProgram sketch IDs are not unique")
    require(
        {item["station_id"] for item in sketch["sketches"]} <= set(station_ids),
        "SketchProgram references an unknown CrossSectionPlan station",
    )
    for item in sketch["sketches"]:
        entity_ids = [entity["entity_id"] for entity in item["entities"]]
        require(
            len(entity_ids) == len(set(entity_ids)),
            f"SketchProgram entity IDs are not unique in {item['sketch_id']}",
        )
        entity_set = set(entity_ids)
        for constraint in item["constraints"]:
            require(
                set(constraint["entity_ids"]) <= entity_set,
                f"SketchProgram constraint references an unknown entity in {item['sketch_id']}",
            )

    node_ids = [node["node_id"] for node in graph["nodes"]]
    require(len(node_ids) == len(set(node_ids)), "WeaponDesignGraph node IDs are not unique")
    edge_ids = [edge["edge_id"] for edge in graph["edges"]]
    require(len(edge_ids) == len(set(edge_ids)), "WeaponDesignGraph edge IDs are not unique")
    node_set = set(node_ids)
    continuity_group_ids = {station["continuity_group_id"] for station in plan["stations"]}
    sketch_source_ids = set(sketch_ids)
    sketch_source_ids.update(
        entity["entity_id"]
        for item in sketch["sketches"]
        for entity in item["entities"]
    )
    for node in graph["nodes"]:
        source_kind = node["source_kind"]
        source_id = node["source_id"]
        if source_kind == "reference":
            require(source_id in reference_view_ids, "WeaponDesignGraph references an unknown view")
        elif source_kind == "cross-section":
            require(
                source_id in set(station_ids) | continuity_group_ids,
                "WeaponDesignGraph references an unknown cross-section source",
            )
        elif source_kind == "sketch":
            require(source_id in sketch_source_ids, "WeaponDesignGraph references an unknown sketch source")
    for edge in graph["edges"]:
        require(
            edge["source_node_id"] in node_set and edge["target_node_id"] in node_set,
            f"WeaponDesignGraph edge {edge['edge_id']} references an unknown node",
        )
        require(
            edge["source_node_id"] != edge["target_node_id"],
            f"WeaponDesignGraph edge {edge['edge_id']} is self-referential",
        )


def expect_binding_rejection(
    positives: dict[str, dict[str, Any]],
    mutate: Any,
    label: str,
) -> None:
    candidate = copy.deepcopy(positives)
    mutate(candidate)
    try:
        check_cross_contract_bindings(candidate)
    except SystemExit:
        return
    fail(f"cross-contract negative case was accepted: {label}")


def main() -> int:
    schemas: dict[str, dict[str, Any]] = {}
    positives: dict[str, dict[str, Any]] = {}
    for filename, version in EXPECTED.items():
        schema_path = SCHEMA_ROOT / filename
        require(schema_path.exists(), f"missing {schema_path.relative_to(ROOT)}")
        schema = load_object(schema_path)
        schemas[filename] = schema
        check_schema_shape(filename, schema)
        require(
            schema["properties"]["schema_version"]["const"] == version,
            f"{filename} version drifted",
        )

        positive_path = FIXTURE_ROOT / "positive" / POSITIVE[filename]
        positive = load_object(positive_path)
        positives[filename] = positive
        require(is_valid(schema, positive), f"positive fixture rejected: {positive_path.name}")
        require(
            positive["canonical_sha256"] == canonical_contract_hash(positive),
            f"positive fixture canonical hash drifted: {positive_path.name}",
        )

        extra = copy.deepcopy(positive)
        extra["unexpected"] = True
        require(not is_valid(schema, extra), f"top-level unknown field accepted: {filename}")

        bad_hash = copy.deepcopy(positive)
        bad_hash["canonical_sha256"] = "not-a-sha256"
        require(not is_valid(schema, bad_hash), f"invalid canonical hash accepted: {filename}")

        negative_path = FIXTURE_ROOT / "negative" / NEGATIVE[filename]
        negative = load_object(negative_path)
        require(
            not is_valid(schema, negative),
            f"negative fixture unexpectedly accepted: {negative_path.name}",
        )

    check_cross_contract_bindings(positives)
    expect_binding_rejection(
        positives,
        lambda value: value["weapon-design-graph.schema.json"].__setitem__(
            "reference_view_set_sha256", "f" * 64
        ),
        "graph binds a different ReferenceViewSet",
    )
    expect_binding_rejection(
        positives,
        lambda value: value["cross-section-plan.schema.json"]["stations"][0][
            "source_view_ids"
        ].append("unknown-view"),
        "station references an undeclared view",
    )
    expect_binding_rejection(
        positives,
        lambda value: value["cross-section-plan.schema.json"]["stations"].__setitem__(
            0,
            {
                **value["cross-section-plan.schema.json"]["stations"][0],
                "position_m": 0.2,
            },
        ),
        "cross-section stations out of order",
    )
    expect_binding_rejection(
        positives,
        lambda value: value["cross-section-plan.schema.json"]["stations"][1]["points"].pop(),
        "cross-section station point counts differ",
    )
    expect_binding_rejection(
        positives,
        lambda value: value["sketch-program.schema.json"]["sketches"][0].__setitem__(
            "station_id", "unknown-station"
        ),
        "sketch references an unknown station",
    )
    expect_binding_rejection(
        positives,
        lambda value: value["weapon-design-graph.schema.json"]["edges"][0].__setitem__(
            "target_node_id", "unknown-node"
        ),
        "graph edge references an unknown node",
    )
    print(
        "Weapon P1 contracts OK: 3 schemas; positive/negative fixtures, closed shape, "
        "hash bindings, station invariants, and graph references passed"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
