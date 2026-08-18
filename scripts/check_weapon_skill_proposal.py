#!/usr/bin/env python3
"""Focused safety gate for the sci-fi FPS weapon Skill proposal.

This checker is intentionally independent from the active MCP006 registry gate.
It validates the Bundle-shaped proposal without registering, materializing, or
executing it.  It uses only the Python standard library and never loads a model,
starts a worker, accesses SQLite/CAS, or performs network I/O.
"""

from __future__ import annotations

import hashlib
import json
import math
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SKILLS = ROOT / "packages" / "forgecad-skills"
PROPOSAL = SKILLS / "proposals" / "sci-fi-fps-weapon" / "1.0.0"
ACTIVE_BUNDLES = SKILLS / "bundles"
REGISTRY = SKILLS / "registry.json"
COMMON_VALIDATORS = SKILLS / "validators" / "validator-set.json"
EXPECTED_SKILL_ID = "sci-fi-fps-weapon"
EXPECTED_VERSION = "1.0.0"

FORBIDDEN_SUFFIXES = {
    ".py",
    ".js",
    ".ts",
    ".sh",
    ".wasm",
    ".dylib",
    ".so",
    ".dll",
    ".exe",
}
ALLOWED_OPERATOR_PATTERN = re.compile(r"^forgecad\.[a-z0-9_.-]+@[0-9]+$")
ALLOWED_VALIDATOR_PATTERN = re.compile(r"^[a-z0-9_.-]+@[0-9]+$")


class CheckFailure(Exception):
    """A bounded, user-readable proposal validation failure."""


def fail(message: str) -> None:
    raise CheckFailure(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def read_json(path: Path) -> dict[str, Any]:
    require(path.exists(), f"missing JSON file: {path.relative_to(ROOT)}")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"invalid JSON {path.relative_to(ROOT)}: {exc}")
    require(isinstance(value, dict), f"JSON root is not an object: {path.relative_to(ROOT)}")
    return value


def sha256(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError as exc:
        fail(f"cannot hash {path.relative_to(ROOT)}: {exc}")


def canonical_hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


def without(value: dict[str, Any], field: str) -> dict[str, Any]:
    result = dict(value)
    result.pop(field, None)
    return result


def parse_lock(path: Path) -> set[str]:
    require(path.exists(), f"missing operator lock: {path.relative_to(ROOT)}")
    entries: set[str] = set()
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        require(" = " in line, f"malformed operator lock line in {path.relative_to(ROOT)}")
        operator, implementation = line.split(" = ", 1)
        require(implementation == "forgecad-runtime-builtin", f"non-product operator implementation: {operator}")
        require(ALLOWED_OPERATOR_PATTERN.fullmatch(operator) is not None, f"invalid operator id: {operator}")
        entries.add(operator)
    return entries


def check_unregistered_boundary() -> None:
    require(PROPOSAL.exists() and PROPOSAL.is_dir(), "proposal directory is missing")
    require(not (ACTIVE_BUNDLES / EXPECTED_SKILL_ID).exists(), "proposal leaked into active bundles/")
    registry = read_json(REGISTRY)
    entries = registry.get("skills")
    require(isinstance(entries, list), "active registry skills is not a list")
    registered = {(entry.get("skill_id"), entry.get("version")) for entry in entries if isinstance(entry, dict)}
    require((EXPECTED_SKILL_ID, EXPECTED_VERSION) not in registered, "proposal is unexpectedly registered")
    require("not registered" in (PROPOSAL / "PROPOSAL.md").read_text(encoding="utf-8"), "proposal boundary note is missing")


def check_manifest(manifest: dict[str, Any]) -> tuple[set[str], set[str]]:
    allowed_keys = {
        "schema_version",
        "skill_id",
        "version",
        "status",
        "publisher",
        "contract_range",
        "input_schema",
        "output_schema",
        "recipe",
        "operator_ids",
        "validator_ids",
        "capabilities",
        "budgets",
        "benchmark_suite",
        "trust_profile",
        "signature",
        "canonical_sha256",
    }
    require(set(manifest) == allowed_keys, "proposal manifest has missing or non-standard fields")
    require(manifest.get("schema_version") == "SkillBundleManifest@1", "manifest schema version is invalid")
    require(manifest.get("skill_id") == EXPECTED_SKILL_ID, "manifest skill id is invalid")
    require(manifest.get("version") == EXPECTED_VERSION, "manifest version is invalid")
    require(manifest.get("status") == "development-only", "proposal manifest must remain development-only")
    require(manifest.get("publisher") == "forgecad-first-party", "manifest publisher is invalid")
    require(manifest.get("contract_range") == "forgecad-runtime-contracts@1", "manifest contract range is invalid")
    require(manifest.get("recipe") == "recipes/default.recipe.json", "manifest recipe reference is not the bundled recipe")
    require(manifest.get("trust_profile") == "development-root", "proposal trust profile is invalid")
    require(manifest.get("signature") == "development-only", "proposal signature status is invalid")
    require(manifest.get("canonical_sha256") == canonical_hash(without(manifest, "canonical_sha256")), "manifest canonical hash drifted")

    capabilities = manifest.get("capabilities")
    require(isinstance(capabilities, dict), "manifest capabilities are not an object")
    for key in ("network", "filesystem_read", "filesystem_write", "dynamic_code", "model_calls"):
        require(capabilities.get(key) is False, f"forbidden capability is enabled: {key}")
    require(capabilities.get("geometry_execution") is False, "proposal must not claim geometry execution")
    require(capabilities.get("render_execution") is False, "proposal must not claim render execution")

    operators = manifest.get("operator_ids")
    validators = manifest.get("validator_ids")
    require(isinstance(operators, list) and operators, "manifest operator_ids is empty")
    require(isinstance(validators, list) and validators, "manifest validator_ids is empty")
    require(len(operators) == len(set(operators)), "manifest operator_ids are duplicated")
    require(len(validators) == len(set(validators)), "manifest validator_ids are duplicated")
    require(all(isinstance(item, str) and ALLOWED_OPERATOR_PATTERN.fullmatch(item) for item in operators), "manifest has an invalid operator id")
    require(all(isinstance(item, str) and ALLOWED_VALIDATOR_PATTERN.fullmatch(item) for item in validators), "manifest has an invalid validator id")

    proposal_operator_ids = parse_lock(PROPOSAL / "operators.lock")
    require(proposal_operator_ids == set(operators), "proposal operators.lock does not close over manifest operator_ids")
    common_operator_ids = parse_lock(ACTIVE_BUNDLES / "hard-surface-detail" / "0.2.0" / "operators.lock")
    common_operator_ids |= parse_lock(ACTIVE_BUNDLES / "primitive-blockout" / "0.2.0" / "operators.lock")
    require(set(operators) <= common_operator_ids, "proposal references an operator outside current active locks")

    common_validators = read_json(COMMON_VALIDATORS)
    common_validator_ids = {item.get("id") for item in common_validators.get("validators", []) if isinstance(item, dict)}
    require(set(validators) <= common_validator_ids, "proposal references a validator outside the first-party validator set")
    return set(operators), set(validators)


def check_registration_aid(manifest: dict[str, Any], operators: set[str], validators: set[str]) -> None:
    """Check the proposed entry without treating it as an active registry entry."""
    entry = read_json(PROPOSAL / "registry-entry.proposed.json")
    require(entry.get("skill_id") == EXPECTED_SKILL_ID and entry.get("version") == EXPECTED_VERSION, "proposed registry aid identity drifted")
    require(entry.get("recipe") == "recipes/sci-fi-fps-weapon.recipe.json", "proposed registry aid lost its deferred top-level recipe target")
    require(set(entry.get("operator_ids", [])) == operators, "proposed registry aid operator_ids are not closed")
    require(set(entry.get("validator_ids", [])) == validators, "proposed registry aid validator_ids are not closed")
    require(entry.get("capabilities") == manifest.get("capabilities"), "proposed registry aid capabilities drifted")
    require(entry.get("budgets") == manifest.get("budgets"), "proposed registry aid budgets drifted")
    require(entry.get("benchmark_suite") == manifest.get("benchmark_suite"), "proposed registry aid benchmark reference drifted")


def check_schema_refs(manifest: dict[str, Any]) -> None:
    for field in ("input_schema", "output_schema"):
        reference = manifest.get(field)
        require(isinstance(reference, str), f"manifest {field} is not a string")
        require(".." not in reference and not reference.startswith(("/", "\\")), f"schema reference escapes proposal: {reference}")
        path = PROPOSAL / reference
        require(path.exists() and path.is_file(), f"missing bundled schema: {reference}")
        schema = read_json(path)
        require(schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema", f"schema is not draft 2020-12: {reference}")
        require(schema.get("type") == "object", f"schema is not an object contract: {reference}")


def check_recipe(manifest: dict[str, Any], operators: set[str]) -> set[str]:
    recipe_path = PROPOSAL / "recipes" / "default.recipe.json"
    recipe = read_json(recipe_path)
    require(recipe.get("schema_version") == "RecipePlan@1", "recipe schema version is invalid")
    require(recipe.get("skill_id") == EXPECTED_SKILL_ID, "recipe skill id is invalid")
    require(recipe.get("units") == "meter", "recipe units are not metres")
    require(recipe.get("coordinate_system") == "right-handed-y-up", "recipe coordinate system is invalid")
    require(recipe.get("canonical_sha256") == canonical_hash(without(recipe, "canonical_sha256")), "recipe canonical hash drifted")
    nodes = recipe.get("nodes")
    edges = recipe.get("edges")
    require(isinstance(nodes, list) and 1 <= len(nodes) <= 64, "recipe node count is outside bounds")
    require(isinstance(edges, list) and len(edges) <= 128, "recipe edge count is outside bounds")
    node_ids = [node.get("node_id") for node in nodes if isinstance(node, dict)]
    require(len(node_ids) == len(nodes) and all(isinstance(node_id, str) and node_id for node_id in node_ids), "recipe node ids are invalid")
    require(len(node_ids) == len(set(node_ids)), "recipe node ids are duplicated")
    require(recipe.get("deterministic_order") == node_ids, "recipe deterministic order is not explicit")
    require(isinstance(recipe.get("max_edges"), int) and recipe["max_edges"] >= len(edges) and recipe["max_edges"] <= 128, "recipe max_edges is invalid")
    budgets = recipe.get("budgets")
    require(isinstance(budgets, dict), "recipe budgets are not an object")
    require(isinstance(budgets.get("max_nodes"), int) and len(nodes) <= budgets["max_nodes"] <= 512, "recipe max_nodes is invalid")

    recipe_operators: set[str] = set()
    for node in nodes:
        operator = node.get("operator_id")
        recipe_operators.add(operator)
        require(operator in operators, f"recipe operator is not in the manifest lock: {operator}")
        for field in ("input_schema", "output_schema"):
            value = node.get(field)
            require(isinstance(value, str) and value and "/" not in value and "\\" not in value and ".." not in value, f"recipe {field} is not bounded")

    graph = {node_id: [] for node_id in node_ids}
    for edge in edges:
        require(isinstance(edge, dict), "recipe edge is not an object")
        source, target = edge.get("from"), edge.get("to")
        require(source in graph and target in graph, "recipe edge references an unknown node")
        graph[source].append(target)
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node_id: str) -> None:
        if node_id in visiting:
            fail("recipe contains a DAG cycle")
        if node_id in visited:
            return
        visiting.add(node_id)
        for child in graph[node_id]:
            visit(child)
        visiting.remove(node_id)
        visited.add(node_id)

    for node_id in graph:
        visit(node_id)
    return recipe_operators


def check_validators(manifest: dict[str, Any], validators: set[str]) -> None:
    validator_set = read_json(PROPOSAL / "validators" / "validator-set.json")
    require(validator_set.get("schema_version") == "SkillValidatorSet@1", "proposal validator set schema is invalid")
    require(validator_set.get("network") is False and validator_set.get("dynamic_code") is False, "validator set enables forbidden capability")
    declared = {item.get("id") for item in validator_set.get("validators", []) if isinstance(item, dict)}
    require(declared == validators, "proposal validator subset does not close over manifest validator_ids")


def check_assets_and_materials() -> None:
    assets = read_json(PROPOSAL / "assets" / "index.json")
    require(assets.get("schema_version") == "SkillAssetIndex@1", "asset index schema is invalid")
    require(assets.get("network") is False and assets.get("external_assets_allowed") is False, "asset index enables network or external assets")
    require(assets.get("assets") == [] and assets.get("required_asset_packs") == [], "proposal contains an undeclared asset payload")

    materials = read_json(PROPOSAL / "materials" / "index.json")
    require(materials.get("schema_version") == "SkillMaterialIndex@1", "material index schema is invalid")
    require(materials.get("network") is False and materials.get("asset_payload_included") is False, "material index enables network or payloads")
    for material in materials.get("materials", []):
        require(material.get("provenance_status") == "declaration_only", "material is not declaration-only")
        require(material.get("license_spdx") == "LicenseRef-ForgeCAD-FIRST-PARTY", "material license declaration is invalid")
    serialized = json.dumps(materials, ensure_ascii=False).lower()
    require("cyan" not in serialized and "gold" not in serialized, "proposal material index invents unavailable cyan/gold assets")


def _all_finite(value: Any) -> bool:
    if isinstance(value, bool) or value is None or isinstance(value, str):
        return not (isinstance(value, str) and value in {"NaN", "Infinity", "+Infinity", "-Infinity"})
    if isinstance(value, (int, float)):
        return math.isfinite(value)
    if isinstance(value, list):
        return all(_all_finite(item) for item in value)
    if isinstance(value, dict):
        return all(_all_finite(item) for item in value.values())
    return False


def _acyclic(edges: object) -> bool:
    if not isinstance(edges, list):
        return False
    graph: dict[str, list[str]] = {}
    for edge in edges:
        if not isinstance(edge, dict):
            return False
        source, target = edge.get("from"), edge.get("to")
        if not isinstance(source, str) or not isinstance(target, str):
            return False
        graph.setdefault(source, []).append(target)
        graph.setdefault(target, [])

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> bool:
        if node in visiting:
            return False
        if node in visited:
            return True
        visiting.add(node)
        if not all(visit(child) for child in graph[node]):
            return False
        visiting.remove(node)
        visited.add(node)
        return True

    return all(visit(node) for node in graph)


def _fixture_is_accepted(fixture: dict[str, Any]) -> bool:
    values = fixture.get("values")
    if not isinstance(values, dict):
        return False
    return (
        fixture.get("units") == "meter"
        and fixture.get("coordinate_system") == "right-handed-y-up"
        and values.get("scope") == "fictional-game-asset"
        and values.get("nonfunctional_asset") is True
        and _all_finite(values)
        and _acyclic(fixture.get("edges", []))
    )


def check_benchmark() -> None:
    valid = read_json(PROPOSAL / "benchmarks" / "fixtures" / "valid.json")
    invalid = read_json(PROPOSAL / "benchmarks" / "fixtures" / "invalid-cycle-unit-finite.json")
    require(valid.get("values", {}).get("scope") == "fictional-game-asset", "valid fixture scope is unsafe")
    require(valid.get("values", {}).get("nonfunctional_asset") is True, "valid fixture does not enforce nonfunctional_asset")
    require(valid.get("expected", {}).get("dag") == "acyclic", "valid fixture does not expect an acyclic DAG")
    require(invalid.get("units") == "millimeter", "negative fixture lost its invalid unit")
    require(invalid.get("coordinate_system") == "left-handed-z-up", "negative fixture lost its invalid coordinate system")
    require(invalid.get("values", {}).get("scale") == "NaN", "negative fixture lost its non-finite marker")
    require(invalid.get("values", {}).get("nonfunctional_asset") is False, "negative fixture lost safety rejection")
    require(invalid.get("expected", {}).get("dag") == "reject", "negative fixture does not expect DAG rejection")
    require(_fixture_is_accepted(valid), "positive benchmark fixture was rejected by the bounded smoke evaluator")
    require(not _fixture_is_accepted(invalid), "negative benchmark fixture was accepted by the bounded smoke evaluator")

    receipt = read_json(PROPOSAL / "benchmark-receipt.json")
    require(receipt.get("schema_version") == "SkillBenchmarkReceipt@1", "benchmark receipt schema is invalid")
    require(receipt.get("status") == "passed", "static proposal benchmark receipt is not passed")
    require(receipt.get("fixture_sha256") == canonical_hash({"valid.json": valid, "invalid-cycle-unit-finite.json": invalid}), "benchmark fixture hash drifted")
    suite = (PROPOSAL / "benchmarks" / "suite.yaml").read_text(encoding="utf-8")
    require("status: passed" in suite and "fixtures/valid.json" in suite and "fixtures/invalid-cycle-unit-finite.json" in suite, "benchmark suite is incomplete")
    require("geometry, render, PBR likeness, human, export, or 360" in suite, "benchmark suite lacks structural-only limitation")


def check_trust_and_provenance() -> None:
    manifest_hash = sha256(PROPOSAL / "manifest.json")
    recipe_hash = sha256(PROPOSAL / "recipes" / "default.recipe.json")
    trust_lines = [line.strip() for line in (PROPOSAL / "trust" / "manifest.sha256").read_text(encoding="utf-8").splitlines() if line.strip()]
    require(trust_lines == [f"{manifest_hash}  manifest.json", f"{recipe_hash}  recipes/default.recipe.json"], "trust manifest does not bind manifest/recipe bytes")

    signature = read_json(PROPOSAL / "signature.bundle")
    require(signature.get("status") == "deferred-to-mcp012-013", "signature is not explicitly deferred")
    require(signature.get("trust_profile") == "development-root", "signature trust profile is invalid")
    require(signature.get("manifest_sha256") == manifest_hash, "signature manifest hash drifted")
    require(signature.get("cryptographic_signature") is None, "proposal contains an unexpected cryptographic signature")

    lines = [line for line in (PROPOSAL / "provenance.intoto.jsonl").read_text(encoding="utf-8").splitlines() if line.strip()]
    require(len(lines) == 1, "provenance must contain exactly one statement")
    statement = json.loads(lines[0])
    require(statement.get("subject", [{}])[0].get("digest", {}).get("sha256") == manifest_hash, "provenance subject does not bind manifest bytes")
    metadata = statement.get("predicate", {}).get("metadata", {})
    require(metadata.get("status") == "target-design" and metadata.get("registered") is False, "provenance does not retain target-design status")
    require(metadata.get("runtime_consumer") == "unavailable" and metadata.get("material_pack") == "unavailable", "provenance overstates unavailable dependencies")


def check_no_payload_or_dynamic_code() -> None:
    for path in PROPOSAL.rglob("*"):
        if not path.is_file():
            continue
        require(path.suffix.lower() not in FORBIDDEN_SUFFIXES, f"proposal contains executable payload: {path.relative_to(ROOT)}")
        data = path.read_bytes()
        require(b"\x00" not in data, f"proposal contains binary payload: {path.relative_to(ROOT)}")
    skill_yaml = (PROPOSAL / "skill.yaml").read_text(encoding="utf-8")
    for marker in (
        "proposal_status: target-design",
        "registered: false",
        "recipe: recipes/default.recipe.json",
        "execution_availability: unavailable",
        "runtime_consumer: unavailable",
        "material_pack: unavailable",
    ):
        require(marker in skill_yaml, f"skill.yaml is missing proposal boundary marker: {marker}")
    require("network: true" not in skill_yaml and "dynamic_code: true" not in skill_yaml and "model_calls: true" not in skill_yaml, "skill.yaml enables a forbidden capability")


def main() -> int:
    try:
        check_unregistered_boundary()
        manifest = read_json(PROPOSAL / "manifest.json")
        operators, validators = check_manifest(manifest)
        check_registration_aid(manifest, operators, validators)
        check_schema_refs(manifest)
        recipe_operators = check_recipe(manifest, operators)
        check_validators(manifest, validators)
        check_assets_and_materials()
        check_benchmark()
        check_trust_and_provenance()
        check_no_payload_or_dynamic_code()
    except (CheckFailure, OSError, UnicodeError, json.JSONDecodeError) as exc:
        print(f"SCI-FI FPS WEAPON PROPOSAL CHECK FAILED: {exc}", file=sys.stderr)
        return 1

    print("SCI-FI FPS WEAPON PROPOSAL CHECK PASS")
    print(f"proposal={PROPOSAL.relative_to(ROOT)}")
    print(f"registered=false active_bundle=false runtime_consumer=unavailable material_pack=unavailable")
    print(f"operators={len(operators)} recipe_operators={len(recipe_operators)} validators={len(validators)}")
    print(f"manifest_sha256={sha256(PROPOSAL / 'manifest.json')}")
    print(f"recipe_sha256={sha256(PROPOSAL / 'recipes' / 'default.recipe.json')}")
    print("static_benchmark=PASS; geometry/render/quality/human/export/360=NOT_CLAIMED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
