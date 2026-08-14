#!/usr/bin/env python3
"""Validate the MCP006 first-party declarative Skill registry.

This gate deliberately uses only the Python standard library. It checks the
checked-in aggregate manifest and recipe metadata; it never executes a Skill,
loads a model, or runs a GitHub repository.
"""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKILLS = ROOT / "packages" / "forgecad-skills"
CONTRACTS = ROOT / "packages" / "forgecad-contracts" / "schemas"
BUNDLES = SKILLS / "bundles"
ARCHIVE = SKILLS / "archive" / "superseded"
EXPECTED = {
    "reference-intake",
    "subject-profile",
    "semantic-assembly",
    "silhouette-blockout",
    "hard-surface-detail",
    "mesh-integrity",
    "uv-pbr",
    "render-evidence",
    "reference-compare",
    "local-edit-and-export",
    "primitive-blockout",
    "ponytail-preflight",
}
ARCHIVED = {
    ("reference-to-typed-plan", "0.1.0"): {
        "tree_sha256": "d12e69cbceac04da9c1386645a083e32a39f370c60be9e41038f7d43a63e596f",
        "replacement": "materialized-mcp006-bundles",
    },
    ("hard-surface-detail", "0.1.0"): {
        "tree_sha256": "98a38f2b44a11962efe9a8bf201990825864f118ab2683f6a582a0512a2521aa",
        "replacement": "hard-surface-detail@0.2.0",
    },
    ("uv-pbr", "0.1.0"): {
        "tree_sha256": "1e851aca170e665bf0618a61d316ae88d063749b2528036083fd374e0872563f",
        "replacement": "uv-pbr@0.2.0",
    },
}


def fail(message: str) -> None:
    raise SystemExit(message)


def assert_acyclic(nodes: set[str], edges: list[dict[str, str]], skill_key: str) -> None:
    graph = {node: [] for node in nodes}
    for edge in edges:
        source, target = edge.get("from"), edge.get("to")
        if source not in nodes or target not in nodes:
            fail(f"{skill_key} recipe has an edge to an unknown node")
        graph[source].append(target)
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> None:
        if node in visiting:
            fail(f"{skill_key} recipe contains a DAG cycle")
        if node in visited:
            return
        visiting.add(node)
        for child in graph[node]:
            visit(child)
        visiting.remove(node)
        visited.add(node)

    for node in nodes:
        visit(node)


def assert_representation_plan(plan: dict[str, object], skill_key: str) -> None:
    if plan.get("units") != "meter" or plan.get("coordinate_system") != "right-handed-y-up":
        fail(f"{skill_key} plan uses an unsupported unit or coordinate system")
    budgets = plan.get("budgets")
    max_parts = budgets.get("max_parts") if isinstance(budgets, dict) else None
    if not isinstance(max_parts, int) or isinstance(max_parts, bool) or not (1 <= max_parts <= 64):
        fail(f"{skill_key} plan has an invalid part budget")

    def finite(value: object) -> bool:
        if isinstance(value, float):
            return math.isfinite(value)
        if isinstance(value, dict):
            return all(finite(child) for child in value.values())
        if isinstance(value, list):
            return all(finite(child) for child in value)
        return True

    if not finite(plan):
        fail(f"{skill_key} plan contains a non-finite value")


def canonical_hash_without(value: dict[str, object], field: str) -> str:
    payload = dict(value)
    payload.pop(field, None)
    return hashlib.sha256(
        json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


def canonical_hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


def assert_recipe_plan(recipe: dict[str, object], entry: dict[str, object], skill_key: str) -> None:
    if recipe.get("schema_version") != "RecipePlan@1" or recipe.get("skill_id") != entry["skill_id"]:
        fail(f"{skill_key} bundle recipe header is invalid")
    nodes = recipe.get("nodes")
    if not isinstance(nodes, list) or not (1 <= len(nodes) <= 64):
        fail(f"{skill_key} recipe node count is outside the contract")
    node_ids = [node.get("node_id") for node in nodes if isinstance(node, dict)]
    if len(node_ids) != len(nodes) or any(not isinstance(node_id, str) or not node_id for node_id in node_ids):
        fail(f"{skill_key} recipe has an invalid node id")
    if len(set(node_ids)) != len(node_ids):
        fail(f"{skill_key} recipe has duplicate node ids")
    if recipe.get("deterministic_order") != node_ids:
        fail(f"{skill_key} recipe deterministic order is not explicit")
    if recipe.get("units") != "meter" or recipe.get("coordinate_system") != "right-handed-y-up":
        fail(f"{skill_key} recipe uses an unsupported unit or coordinate system")
    edges = recipe.get("edges")
    if not isinstance(edges, list) or len(edges) > 128:
        fail(f"{skill_key} recipe edge count is outside the contract")
    assert_acyclic(set(node_ids), edges, skill_key)
    if not isinstance(recipe.get("max_edges"), int) or recipe["max_edges"] < len(edges) or recipe["max_edges"] > 128:
        fail(f"{skill_key} recipe max_edges budget is invalid")
    budgets = recipe.get("budgets")
    if not isinstance(budgets, dict) or not isinstance(budgets.get("max_nodes"), int):
        fail(f"{skill_key} recipe node budget is invalid")
    if budgets["max_nodes"] < len(nodes) or budgets["max_nodes"] > 512:
        fail(f"{skill_key} recipe exceeds its node budget")
    allowlist = set(entry["operator_ids"])
    for node in nodes:
        if not isinstance(node, dict) or node.get("operator_id") not in allowlist:
            fail(f"{skill_key} recipe invokes an operator outside its lock")
        for key in ("input_schema", "output_schema"):
            value = node.get(key)
            if not isinstance(value, str) or not value or ".." in value or "/" in value or "\\" in value:
                fail(f"{skill_key} recipe has an invalid {key}")

    def finite(value: object) -> bool:
        if isinstance(value, float):
            return math.isfinite(value)
        if isinstance(value, dict):
            return all(finite(child) for child in value.values())
        if isinstance(value, list):
            return all(finite(child) for child in value)
        return True

    if not finite(recipe):
        fail(f"{skill_key} recipe contains a non-finite value")
    if recipe.get("canonical_sha256") != canonical_hash_without(recipe, "canonical_sha256"):
        fail(f"{skill_key} recipe canonical hash does not match")


def assert_materialized_bundle(entry: dict[str, object]) -> None:
    skill_id = str(entry["skill_id"])
    version = str(entry["version"])
    key = f"{skill_id}@{version}"
    bundle = SKILLS / "bundles" / skill_id / version
    required = (
        "manifest.json",
        "skill.yaml",
        "operators.lock",
        "validators/validator-set.json",
        "assets/index.json",
        "materials/index.json",
        "benchmarks/suite.yaml",
        "benchmarks/fixtures/valid.json",
        "benchmarks/fixtures/invalid-cycle-unit-finite.json",
        "benchmark-receipt.json",
        "LICENSES/ForgeCAD-FIRST-PARTY.txt",
        "NOTICE",
        "sbom.spdx.json",
        "provenance.intoto.jsonl",
        "trust/manifest.sha256",
        "signature.bundle",
        "recipes/default.recipe.json",
    )
    missing = [relative for relative in required if not (bundle / relative).exists()]
    if missing:
        fail(f"{key} independent bundle is missing: {missing}")
    forbidden_suffixes = {".py", ".js", ".ts", ".sh", ".wasm", ".dylib", ".so", ".dll", ".exe"}
    if any(path.suffix.lower() in forbidden_suffixes for path in bundle.rglob("*")):
        fail(f"{key} bundle contains an executable or script payload")

    manifest = json.loads((bundle / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("schema_version") != "SkillBundleManifest@1" or manifest.get("skill_id") != skill_id:
        fail(f"{key} independent manifest header is invalid")
    if manifest.get("canonical_sha256") != canonical_hash_without(manifest, "canonical_sha256"):
        fail(f"{key} independent manifest canonical hash does not match")
    for field in ("version", "status", "publisher", "contract_range", "operator_ids", "validator_ids", "capabilities", "budgets", "benchmark_suite"):
        if manifest.get(field) != (entry.get(field) if field in entry else {"status": "development-only", "publisher": "forgecad-first-party", "contract_range": "forgecad-runtime-contracts@1"}.get(field)):
            fail(f"{key} independent manifest drifts from the registry: {field}")
    if manifest.get("trust_profile") != "development-root" or manifest.get("signature") != "development-only":
        fail(f"{key} independent manifest trust profile is invalid")

    recipe = json.loads((bundle / "recipes/default.recipe.json").read_text(encoding="utf-8"))
    assert_recipe_plan(recipe, entry, key)
    for schema_field in ("input_schema", "output_schema"):
        schema_path = bundle / str(manifest[schema_field])
        if not schema_path.exists() or not str(schema_path.resolve()).startswith(str(bundle.resolve())):
            fail(f"{key} schema path escapes the bundle")
        document = json.loads(schema_path.read_text(encoding="utf-8"))
        if document.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
            fail(f"{key} bundled schema is not draft 2020-12")

    validator_set = json.loads((bundle / "validators/validator-set.json").read_text(encoding="utf-8"))
    ids = {validator.get("id") for validator in validator_set.get("validators", [])}
    if not set(entry["validator_ids"]).issubset(ids) or validator_set.get("network") is not False or validator_set.get("dynamic_code") is not False:
        fail(f"{key} validator subset is not bounded")
    lock_lines = {
        line.split(" = ", 1)[0].strip()
        for line in (bundle / "operators.lock").read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    }
    if lock_lines != set(entry["operator_ids"]):
        fail(f"{key} operator lock drifts from the registry")
    fixtures = {
        path.name: json.loads(path.read_text(encoding="utf-8"))
        for path in (bundle / "benchmarks/fixtures").glob("*.json")
    }
    if set(fixtures) != {"valid.json", "invalid-cycle-unit-finite.json"} or fixtures["valid.json"].get("expected", {}).get("dag") != "acyclic":
        fail(f"{key} benchmark fixtures are incomplete")
    receipt = json.loads((bundle / "benchmark-receipt.json").read_text(encoding="utf-8"))
    expected_fixture_hash = canonical_hash({"valid": fixtures["valid.json"], "invalid": fixtures["invalid-cycle-unit-finite.json"]})
    if receipt.get("status") != "passed" or receipt.get("fixture_sha256") != expected_fixture_hash:
        fail(f"{key} benchmark receipt is not bound to its fixtures")
    trust = [line.strip() for line in (bundle / "trust/manifest.sha256").read_text(encoding="utf-8").splitlines() if line.strip()]
    expected_trust = [
        f"{hashlib.sha256((bundle / 'manifest.json').read_bytes()).hexdigest()}  manifest.json",
        f"{hashlib.sha256((bundle / 'recipes/default.recipe.json').read_bytes()).hexdigest()}  recipes/default.recipe.json",
    ]
    if trust != expected_trust:
        fail(f"{key} trust manifest does not bind manifest and recipe bytes")
    signature = json.loads((bundle / "signature.bundle").read_text(encoding="utf-8"))
    if signature.get("status") != "deferred-to-mcp012-013" or signature.get("cryptographic_signature") is not None:
        fail(f"{key} signature placeholder is not explicitly deferred")


def tree_hash(root: Path) -> str:
    files = sorted(path for path in root.rglob("*") if path.is_file())
    if not files:
        fail(f"archive bundle is empty: {root.relative_to(ROOT)}")
    lines = "".join(
        f"{hashlib.sha256(path.read_bytes()).hexdigest()}  ./{path.relative_to(root).as_posix()}\n"
        for path in files
    )
    return hashlib.sha256(lines.encode("utf-8")).hexdigest()


def assert_archived_bundles() -> None:
    manifest_path = ARCHIVE / "manifest.json"
    if not manifest_path.exists():
        fail("superseded Skill archive manifest is missing")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema_version") != "ForgeCADSkillArchiveManifest@1" or manifest.get("status") != "SUPERSEDED":
        fail("superseded Skill archive manifest is invalid")
    entries = manifest.get("entries")
    if not isinstance(entries, list):
        fail("superseded Skill archive entries are invalid")
    actual = {
        (entry.get("skill_id"), entry.get("version")): entry
        for entry in entries
        if isinstance(entry, dict)
    }
    if set(actual) != set(ARCHIVED):
        fail("superseded Skill archive entries drift from the isolation policy")
    for key, expected in ARCHIVED.items():
        skill_id, version = key
        entry = actual[key]
        bundle = ARCHIVE / skill_id / version
        if not (bundle / "skill.yaml").exists():
            fail(f"archived Skill is missing: {skill_id}@{version}")
        if entry.get("archive_path") != f"{skill_id}/{version}" or entry.get("status") != "SUPERSEDED":
            fail(f"archived Skill receipt is invalid: {skill_id}@{version}")
        if entry.get("replacement") != expected["replacement"]:
            fail(f"archived Skill replacement is invalid: {skill_id}@{version}")
        if entry.get("tree_sha256") != expected["tree_sha256"]:
            fail(f"archived Skill tree hash is invalid: {skill_id}@{version}")
        if tree_hash(bundle) != expected["tree_sha256"]:
            fail(f"archived Skill bytes drifted: {skill_id}@{version}")


def main() -> int:
    assert_archived_bundles()

    registry_path = SKILLS / "registry.json"
    registry_bytes = registry_path.read_bytes()
    registry = json.loads(registry_bytes)
    if registry.get("schema_version") != "ForgeCADSkillRegistry@1":
        fail("MCP006 registry schema is invalid")
    if registry.get("publisher") != "forgecad-first-party" or registry.get("status") != "development-only":
        fail("MCP006 registry must be first-party development-only")
    entries = registry.get("skills")
    if not isinstance(entries, list) or {entry.get("skill_id") for entry in entries} != EXPECTED:
        fail("MCP006 registry must contain exactly the twelve MVP Skill IDs")

    active_bundle_paths = {
        (str(entry.get("skill_id")), str(entry.get("version")))
        for entry in entries
    }
    actual_bundle_paths = {
        (skill_dir.name, version_dir.name)
        for skill_dir in BUNDLES.iterdir()
        if skill_dir.is_dir()
        for version_dir in skill_dir.iterdir()
        if version_dir.is_dir()
    }
    unexpected_bundle_paths = sorted(actual_bundle_paths - active_bundle_paths)
    if unexpected_bundle_paths:
        fail(f"unregistered Skill bundles must be archived: {unexpected_bundle_paths}")
    missing_bundle_paths = sorted(active_bundle_paths - actual_bundle_paths)
    if missing_bundle_paths:
        fail(f"active Skill bundles are missing: {missing_bundle_paths}")

    expected_registry_hash = hashlib.sha256(registry_bytes).hexdigest()
    for required in (
        "skill.yaml",
        "operators.lock",
        "validators/validator-set.json",
        "assets/index.json",
        "materials/index.json",
        "benchmarks/suite.yaml",
        "LICENSES/ForgeCAD-FIRST-PARTY.txt",
        "NOTICE",
        "sbom.spdx.json",
        "provenance.intoto.jsonl",
        "benchmark-receipt.json",
        "trust/manifest.sha256",
    ):
        if not (SKILLS / required).exists():
            fail(f"MCP006 bundle metadata missing: {required}")
    trust_lines = [line.strip() for line in (SKILLS / "trust" / "manifest.sha256").read_text().splitlines() if line.strip() and not line.startswith("#")]
    if trust_lines != [f"{expected_registry_hash}  registry.json"]:
        fail("MCP006 development trust manifest does not bind registry.json")

    seen = set()
    for entry in entries:
        skill_id = entry.get("skill_id")
        version = entry.get("version")
        key = f"{skill_id}@{version}"
        if key in seen:
            fail(f"duplicate Skill version: {key}")
        seen.add(key)
        expected_version = "0.2.0" if skill_id in {"primitive-blockout", "hard-surface-detail", "uv-pbr"} else "0.1.0"
        if version != expected_version:
            fail(f"unexpected Skill version: {key}")
        capabilities = entry.get("capabilities", {})
        for forbidden in ("network", "filesystem_read", "filesystem_write", "dynamic_code", "model_calls"):
            if capabilities.get(forbidden) is not False:
                fail(f"{key} enables forbidden capability {forbidden}")
        operators = entry.get("operator_ids", [])
        validators = entry.get("validator_ids", [])
        if not operators or not validators:
            fail(f"{key} has an empty operator/validator allowlist")
        if any(
            not operator.startswith("forgecad.")
            or operator.rsplit("@", 1)[-1] not in {"1", "2"}
            for operator in operators
        ):
            fail(f"{key} has an unregistered operator")
        if any(not validator.endswith("@1") for validator in validators):
            fail(f"{key} has an invalid validator id")

        for contract_ref in (entry.get("input_schema"), entry.get("output_schema")):
            if not contract_ref.startswith("contracts/"):
                fail(f"{key} contract reference is not path-bounded")
            schema_name = contract_ref.removeprefix("contracts/")
            if not (CONTRACTS / schema_name).exists():
                fail(f"{key} references missing contract schema {schema_name}")

        recipe_path = SKILLS / entry["recipe"]
        if not recipe_path.exists():
            fail(f"{key} recipe is missing: {recipe_path.relative_to(ROOT)}")
        recipe = json.loads(recipe_path.read_text())
        assert_recipe_plan(recipe, entry, key)
        serialized_recipe = json.dumps(recipe)
        if "http://" in serialized_recipe or "https://" in serialized_recipe or ".." in serialized_recipe:
            fail(f"{key} recipe contains an external URL or traversal path")
        assert_materialized_bundle(entry)

    validator_set = json.loads((SKILLS / "validators/validator-set.json").read_text())
    validator_ids = {validator.get("id") for validator in validator_set.get("validators", [])}
    referenced_validators = {validator for entry in entries for validator in entry["validator_ids"]}
    if not referenced_validators.issubset(validator_ids):
        fail("MCP006 Skill references a validator missing from the built-in validator set")
    if validator_set.get("network") is not False or validator_set.get("dynamic_code") is not False:
        fail("MCP006 validator set enables a forbidden capability")

    benchmark = (SKILLS / "benchmark-receipt.json").read_text()
    if '"status":"not-run"' in benchmark:
        fail("MCP006 registry safety benchmark receipt is still not-run")

    # Negative contract probes: these are kept in-process so the gate never
    # executes arbitrary fixture code or downloads a test package.
    negative_nodes = {"a", "b"}
    try:
        assert_acyclic(negative_nodes, [{"from": "a", "to": "b"}, {"from": "b", "to": "a"}], "negative-cycle")
    except SystemExit:
        pass
    else:
        fail("MCP006 negative cycle probe did not fail closed")

    valid_plan = {
        "units": "meter",
        "coordinate_system": "right-handed-y-up",
        "budgets": {"max_parts": 16},
        "scale": 1.0,
    }
    assert_representation_plan(valid_plan, "valid-plan")
    for invalid_plan in (
        {**valid_plan, "units": "millimeter"},
        {**valid_plan, "scale": float("nan")},
    ):
        try:
            assert_representation_plan(invalid_plan, "negative-plan")
        except SystemExit:
            continue
        fail("MCP006 negative unit/non-finite plan probe did not fail closed")

    print(f"MCP006 Skill registry OK: {len(entries)} first-party declarative Skills; registry_sha256={expected_registry_hash}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
