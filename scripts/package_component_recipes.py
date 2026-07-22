#!/usr/bin/env python3
"""Package the reviewed component registry without weakening C105 references.

M108B-01 introduces domain-owned pack manifests.  The C105 v1 recipes are a
frozen compatibility source: emitted aggregate ordering and every semantic hash
must remain exact because persisted ComponentRecipeRef/provenance record them.
Future packs can replace ``recipe_source`` with an inline ``recipes`` array,
but a change to this lock requires an explicit registry-version migration.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PACK_ROOT = ROOT / "packages/concept-spec/fixtures/component-recipes"
PRODUCTION_REGISTRY_PATH = ROOT / "packages/concept-spec/fixtures/production-component-recipe-registry.json"
PRODUCTION_DOMAINS = (
    "pack_future_weapon_prop",
    "pack_vehicle_concept",
    "pack_aircraft_concept",
    "pack_robotic_arm_concept",
)


class PackageError(ValueError):
    pass


def load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PackageError(f"COMPONENT_RECIPE_PACKAGE_INVALID_JSON:{path.name}:{error}") from error
    if not isinstance(value, dict):
        raise PackageError(f"COMPONENT_RECIPE_PACKAGE_INVALID_OBJECT:{path.name}")
    return value


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)


def canonical_sha256(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode()).hexdigest()


def policy_range(value: Any, field: str) -> tuple[int, int]:
    if (
        not isinstance(value, list)
        or len(value) != 2
        or any(not isinstance(item, int) or isinstance(item, bool) for item in value)
        or value[0] < 0
        or value[0] > value[1]
    ):
        raise PackageError(f"M108B_PRODUCTION_{field}_POLICY_INVALID")
    return value[0], value[1]


def expanded_recipe_facts(
    recipe_id: str,
    catalog: dict[str, dict[str, Any]],
    *,
    stack: tuple[str, ...] = (),
) -> tuple[int, set[str]]:
    """Return exact recursive instance and Material Zone facts for one root.

    The package lock records these source-derived facts so a varied domain
    recipe (for example, an aircraft with one additional visual fin) is not
    forced into a misleading global `parts_per_root` constant.  Rust expansion
    and the GLB smoke still independently verify the runtime result.
    """
    if recipe_id in stack:
        raise PackageError("M108B_PRODUCTION_CHILD_CYCLE_INVALID")
    recipe = catalog.get(recipe_id)
    if recipe is None:
        raise PackageError("M108B_PRODUCTION_CHILD_REFERENCE_UNKNOWN")
    zones: set[str] = set()
    for zone in recipe.get("material_zones", []):
        if not isinstance(zone, dict) or not isinstance(zone.get("zone_id"), str):
            raise PackageError("M108B_PRODUCTION_ZONE_CONTRACT_INVALID")
        zones.add(zone["zone_id"])
    instances = 1
    for slot in recipe.get("child_slots", []):
        if not isinstance(slot, dict) or not isinstance(slot.get("child_recipe_id"), str):
            raise PackageError("M108B_PRODUCTION_CHILD_REFERENCE_INVALID")
        child_instances, child_zones = expanded_recipe_facts(
            slot["child_recipe_id"], catalog, stack=(*stack, recipe_id)
        )
        instances += child_instances
        zones.update(child_zones)
    return instances, zones


def validate_embedded_profile_hashes(recipe: dict[str, Any]) -> None:
    """Match Rust's runtime canonical boundary for embedded Profile resources.

    M108B source packs are validated before the generated registry is written.
    A payload can be schema-valid but not runtime-canonical: for example, JSON
    `1.0` is accepted as an integer by the profile contract, while the runtime
    normalizes it to `1`.  Python's ordinary dict equality would miss that
    distinction, so compare canonical JSON bytes as well as both hashes.

    This is deliberately production-pack-only.  The frozen C105 aggregate has
    its own exact compatibility boundary and is not reinterpreted here.
    """
    agent_root = ROOT / "apps/agent"
    if str(agent_root) not in sys.path:
        sys.path.insert(0, str(agent_root))
    try:
        from forgecad_agent.application.profile_contracts import canonical_profile_payload
    except Exception as error:  # pragma: no cover - environment setup failure
        raise PackageError(f"M108B_PRODUCTION_PROFILE_RUNTIME_UNAVAILABLE:{error}") from error

    recipe_id = str(recipe.get("recipe_id", "<unknown>"))
    program = recipe.get("shape_program_template")
    if not isinstance(program, dict):
        raise PackageError("M108B_PRODUCTION_RECIPE_INVALID")

    def assert_runtime_canonical(
        item: Any,
        *,
        payload_field: str,
        digest_field: str,
        invalid_code: str,
        resource_kind: str,
        resource_id_field: str,
    ) -> None:
        if not isinstance(item, dict) or not isinstance(item.get(payload_field), dict):
            raise PackageError(invalid_code)
        payload = item[payload_field]
        resource_id = str(item.get(resource_id_field, "<unknown>"))
        try:
            normalized, runtime_json, runtime_digest = canonical_profile_payload(payload)
        except Exception as error:
            raise PackageError(
                f"M108B_PRODUCTION_PROFILE_CONTRACT_INVALID:{recipe_id}:{resource_kind}:{resource_id}"
            ) from error
        raw_json = canonical_json(payload)
        raw_digest = canonical_sha256(payload)
        # Comparing bytes rather than `payload != normalized` deliberately
        # distinguishes integer 1 from float 1.0.
        if raw_json != runtime_json or raw_digest != runtime_digest:
            raise PackageError(
                f"M108B_PRODUCTION_PROFILE_PAYLOAD_NOT_CANONICAL:{recipe_id}:{resource_kind}:{resource_id}"
            )
        expected = item.get(digest_field)
        if not isinstance(expected, str) or expected != runtime_digest or expected != raw_digest:
            raise PackageError(f"M108B_PRODUCTION_PROFILE_HASH_DRIFT:{recipe_id}")
        # Keep the normalized object live in the contract: if canonical JSON
        # ever changes representation, the byte comparison above remains the
        # fail-closed authority and this guards accidental dead-code drift.
        if canonical_json(normalized) != runtime_json:
            raise PackageError(
                f"M108B_PRODUCTION_PROFILE_RUNTIME_CANONICALIZATION_INVALID:{recipe_id}:{resource_kind}:{resource_id}"
            )

    for item in program.get("profile_inputs", []):
        assert_runtime_canonical(
            item,
            payload_field="canonical_payload",
            digest_field="input_sha256",
            invalid_code="M108B_PRODUCTION_PROFILE_INPUT_INVALID",
            resource_kind="profile_input",
            resource_id_field="input_id",
        )
    for item in recipe.get("section_sets", []):
        assert_runtime_canonical(
            item,
            payload_field="canonical_payload",
            digest_field="sha256",
            invalid_code="M108B_PRODUCTION_SECTION_SET_INVALID",
            resource_kind="section_set",
            resource_id_field="section_set_id",
        )


def relative(root: Path, path: str) -> Path:
    candidate = (root / path).resolve()
    if root.resolve() not in candidate.parents and candidate != root.resolve():
        raise PackageError("COMPONENT_RECIPE_PACKAGE_PATH_ESCAPE")
    return candidate


def fixture_relative(root: Path, path: str, fixture_root: Path) -> Path:
    candidate = (root / path).resolve()
    boundary = fixture_root.resolve()
    if boundary not in candidate.parents and candidate != boundary:
        raise PackageError("COMPONENT_RECIPE_PACKAGE_PATH_ESCAPE")
    return candidate


def build(pack_root: Path = PACK_ROOT) -> tuple[dict[str, Any], dict[str, Any]]:
    manifest = load(pack_root / "manifest.json")
    if manifest.get("schema_version") != "ComponentRecipePackManifest@1" or manifest.get("aggregate_strategy") != "legacy_c105_v1_exact":
        raise PackageError("COMPONENT_RECIPE_PACKAGE_MANIFEST_INVALID")
    locks = load(relative(pack_root, str(manifest.get("lock_path", ""))))
    if locks.get("schema_version") != "ComponentRecipeRegistryLock@1":
        raise PackageError("COMPONENT_RECIPE_PACKAGE_LOCK_INVALID")
    aggregate_path = fixture_relative(pack_root, str(manifest.get("aggregate_path", "")), pack_root.parent)
    aggregate = load(aggregate_path)
    if aggregate.get("schema_version") != "EditableComponentRecipeRegistry@1":
        raise PackageError("COMPONENT_RECIPE_PACKAGE_COMPAT_SOURCE_INVALID")
    source = {item.get("recipe_id"): item for item in aggregate.get("recipes", []) if isinstance(item, dict)}
    if len(source) != len(aggregate.get("recipes", [])):
        raise PackageError("COMPONENT_RECIPE_PACKAGE_DUPLICATE_ID")
    selected: list[dict[str, Any]] = []
    seen: set[str] = set()
    for entry in manifest.get("packs", []):
        if not isinstance(entry, dict):
            raise PackageError("COMPONENT_RECIPE_PACKAGE_MANIFEST_INVALID")
        pack = load(relative(pack_root, str(entry.get("path", ""))))
        if pack.get("schema_version") != "ComponentRecipePack@1" or pack.get("pack_id") != entry.get("pack_id"):
            raise PackageError("COMPONENT_RECIPE_PACKAGE_PACK_INVALID")
        compatibility = load(relative(pack_root, str(pack.get("recipe_source", ""))))
        source_path = fixture_relative(pack_root / "shared", str(compatibility.get("source_registry_path", "")), pack_root.parent)
        if source_path != aggregate_path or compatibility.get("strategy") != "C105_v1_aggregate_is_semantically_frozen":
            raise PackageError("COMPONENT_RECIPE_PACKAGE_COMPAT_SOURCE_INVALID")
        domain = pack.get("domain_pack_id")
        for recipe_id in pack.get("recipe_ids", []):
            if not isinstance(recipe_id, str) or recipe_id in seen:
                raise PackageError("COMPONENT_RECIPE_PACKAGE_DUPLICATE_ID")
            recipe = source.get(recipe_id)
            if recipe is None:
                raise PackageError("COMPONENT_RECIPE_PACKAGE_UNKNOWN_RECIPE")
            if recipe.get("allowed_domains") != [domain]:
                raise PackageError("COMPONENT_RECIPE_PACKAGE_CROSS_PACK")
            seen.add(recipe_id)
            selected.append(copy.deepcopy(recipe))
    order = locks.get("recipe_order")
    if [item["recipe_id"] for item in selected] != order:
        raise PackageError("COMPONENT_RECIPE_PACKAGE_ORDER_DRIFT")
    if set(source) != seen:
        raise PackageError("COMPONENT_RECIPE_PACKAGE_COVERAGE_DRIFT")
    recipe_locks = locks.get("recipe_sha256")
    if not isinstance(recipe_locks, dict):
        raise PackageError("COMPONENT_RECIPE_PACKAGE_LOCK_INVALID")
    for recipe in selected:
        if recipe_locks.get(recipe["recipe_id"]) != canonical_sha256(recipe):
            raise PackageError("COMPONENT_RECIPE_PACKAGE_RECIPE_HASH_DRIFT")
    emitted = {"schema_version": aggregate["schema_version"], "registry_id": aggregate["registry_id"], "policy_version": aggregate["policy_version"], "recipes": selected}
    expected = locks.get("aggregate_registry_sha256")
    if expected != canonical_sha256(emitted):
        raise PackageError("COMPONENT_RECIPE_PACKAGE_AGGREGATE_HASH_DRIFT")
    if emitted != aggregate:
        raise PackageError("COMPONENT_RECIPE_PACKAGE_LEGACY_REFERENCE_DRIFT")
    return emitted, locks


def self_test() -> None:
    build()
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp) / "component-recipes"
        # The fixture tree is deliberately copied as JSON so every mutation is isolated.
        import shutil
        shutil.copytree(PACK_ROOT, root)
        shutil.copy2(PACK_ROOT.parent / "editable-component-recipe-registry.json", root.parent / "editable-component-recipe-registry.json")
        cases = {
            "duplicate": (root / "packs/future-weapon-prop.json", lambda v: v["recipe_ids"].append("recipe_future_prop_shell"), "DUPLICATE_ID"),
            "order": (root / "locks/c105-v1.lock.json", lambda v: v.__setitem__("recipe_order", list(reversed(v["recipe_order"]))), "ORDER_DRIFT"),
            "cross": (root / "packs/vehicle-concept.json", lambda v: v.__setitem__("domain_pack_id", "pack_aircraft_concept"), "CROSS_PACK"),
            "hash": (root / "locks/c105-v1.lock.json", lambda v: v["recipe_sha256"].__setitem__("recipe_future_prop_shell", "0" * 64), "RECIPE_HASH_DRIFT"),
            "legacy": (root / "locks/c105-v1.lock.json", lambda v: v.__setitem__("aggregate_registry_sha256", "0" * 64), "AGGREGATE_HASH_DRIFT"),
        }
        for _, (path, mutate, expected) in cases.items():
            original = load(path); mutate(original); path.write_text(json.dumps(original), encoding="utf-8")
            try:
                build(root)
            except PackageError as error:
                if expected not in str(error):
                    raise AssertionError(f"expected {expected}, got {error}") from error
            else:
                raise AssertionError(f"negative package case did not fail: {expected}")
            shutil.rmtree(root); shutil.copytree(PACK_ROOT, root)


def build_production(pack_root: Path = PACK_ROOT) -> tuple[dict[str, Any], dict[str, Any]]:
    """Aggregate four domain-owned M108B packs without moving the C105 catalog.

    The checked-in production registry is a generated compatibility artifact for
    Rust's ``include_str!``.  Pack files are the editable source of truth.  A
    package run must reproduce the registry byte-for-byte *as parsed JSON* and
    preserve its semantic identity before it is accepted by the Rust catalog.
    """
    manifest = load(pack_root / "production-manifest.json")
    if (
        manifest.get("schema_version") != "M108BProductionRecipePackManifest@1"
        or manifest.get("registry_id") != "registry_m108b_production_concept_v1"
        or manifest.get("policy_version") != "ComponentRecipePolicy@1"
    ):
        raise PackageError("M108B_PRODUCTION_MANIFEST_INVALID")
    registry_path = fixture_relative(pack_root, str(manifest.get("aggregate_path", "")), pack_root.parent)
    registry = load(registry_path)
    if (
        registry.get("schema_version") != "EditableComponentRecipeRegistry@1"
        or registry.get("registry_id") != manifest.get("registry_id")
        or registry.get("policy_version") != manifest.get("policy_version")
    ):
        raise PackageError("M108B_PRODUCTION_REGISTRY_INVALID")
    lock = load(fixture_relative(pack_root, str(manifest.get("lock_path", "")), pack_root))
    if lock.get("schema_version") != "M108BProductionRecipeLock@1" or lock.get("registry_id") != registry.get("registry_id"):
        raise PackageError("M108B_PRODUCTION_LOCK_INVALID")
    root_slot_range = policy_range(lock.get("root_slot_range"), "ROOT_SLOT_RANGE")
    root_zone_range = policy_range(lock.get("root_zone_range"), "ROOT_ZONE_RANGE")

    entries = manifest.get("packs")
    if not isinstance(entries, list) or len(entries) != len(PRODUCTION_DOMAINS):
        raise PackageError("M108B_PRODUCTION_DOMAIN_COVERAGE_INVALID")
    expected_pack_order = lock.get("pack_order")
    if not isinstance(expected_pack_order, list) or len(expected_pack_order) != len(PRODUCTION_DOMAINS):
        raise PackageError("M108B_PRODUCTION_LOCK_INVALID")

    selected: list[dict[str, Any]] = []
    roots_by_domain: dict[str, list[str]] = {}
    seen_recipes: set[str] = set()
    seen_packs: set[str] = set()
    seen_domains: set[str] = set()
    observed_pack_order: list[str] = []
    for expected_order, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise PackageError("M108B_PRODUCTION_MANIFEST_INVALID")
        pack_id = entry.get("pack_id")
        domain = entry.get("domain_pack_id")
        if (
            not isinstance(pack_id, str)
            or not isinstance(domain, str)
            or not isinstance(entry.get("path"), str)
            or entry.get("order") != expected_order
            or pack_id in seen_packs
            or domain in seen_domains
        ):
            raise PackageError("M108B_PRODUCTION_PACK_ORDER_DRIFT")
        pack_path = fixture_relative(pack_root, entry["path"], pack_root)
        pack = load(pack_path)
        if (
            pack.get("schema_version") != "M108BProductionRecipeDomainPack@1"
            or pack.get("pack_id") != pack_id
            or pack.get("domain_pack_id") != domain
            or not isinstance(pack.get("recipes"), list)
            or not isinstance(pack.get("root_recipe_ids"), list)
        ):
            raise PackageError("M108B_PRODUCTION_PACK_INVALID")
        seen_packs.add(pack_id)
        seen_domains.add(domain)
        observed_pack_order.append(pack_id)
        roots_by_domain[domain] = list(pack["root_recipe_ids"])
        for recipe in pack["recipes"]:
            if not isinstance(recipe, dict) or not isinstance(recipe.get("recipe_id"), str):
                raise PackageError("M108B_PRODUCTION_RECIPE_INVALID")
            validate_embedded_profile_hashes(recipe)
            recipe_id = recipe["recipe_id"]
            if recipe_id in seen_recipes:
                raise PackageError("M108B_PRODUCTION_DUPLICATE_ID")
            if recipe.get("allowed_domains") != [domain]:
                raise PackageError("M108B_PRODUCTION_CROSS_DOMAIN")
            seen_recipes.add(recipe_id)
            selected.append(copy.deepcopy(recipe))
    if seen_domains != set(PRODUCTION_DOMAINS) or observed_pack_order != expected_pack_order:
        raise PackageError("M108B_PRODUCTION_PACK_ORDER_DRIFT")

    emitted = {
        "schema_version": registry["schema_version"],
        "registry_id": registry["registry_id"],
        "policy_version": registry["policy_version"],
        "recipes": selected,
    }
    aggregate_ids = [recipe.get("recipe_id") for recipe in registry.get("recipes", []) if isinstance(recipe, dict)]
    emitted_ids = [recipe["recipe_id"] for recipe in selected]
    if len(aggregate_ids) != len(registry.get("recipes", [])) or len(set(aggregate_ids)) != len(aggregate_ids):
        raise PackageError("M108B_PRODUCTION_REGISTRY_INVALID")
    if emitted_ids != aggregate_ids or set(emitted_ids) != set(aggregate_ids):
        raise PackageError("M108B_PRODUCTION_COVERAGE_DRIFT")
    if lock.get("recipe_order") != emitted_ids:
        raise PackageError("M108B_PRODUCTION_RECIPE_ORDER_DRIFT")
    recipe_locks = lock.get("recipe_sha256")
    if not isinstance(recipe_locks, dict) or set(recipe_locks) != set(emitted_ids):
        raise PackageError("M108B_PRODUCTION_LOCK_INVALID")
    for recipe in selected:
        if recipe_locks.get(recipe["recipe_id"]) != canonical_sha256(recipe):
            raise PackageError("M108B_PRODUCTION_RECIPE_HASH_DRIFT")
    if emitted != registry:
        raise PackageError("M108B_PRODUCTION_AGGREGATE_DRIFT")
    if lock.get("registry_sha256") != canonical_sha256(emitted):
        raise PackageError("M108B_PRODUCTION_REGISTRY_HASH_DRIFT")

    roots = [recipe_id for domain in PRODUCTION_DOMAINS for recipe_id in roots_by_domain.get(domain, [])]
    if len(roots) != lock.get("expected_root_count") or len(set(roots)) != len(roots):
        raise PackageError("M108B_PRODUCTION_ROOT_COUNT_INVALID")
    hashes = lock.get("root_recipe_sha256")
    recipes = {recipe["recipe_id"]: recipe for recipe in selected}
    if not isinstance(hashes, dict) or set(hashes) != set(roots):
        raise PackageError("M108B_PRODUCTION_ROOT_LOCK_INVALID")
    expected_parts = lock.get("expected_parts_by_root")
    expected_zones = lock.get("expected_unique_zones_by_root")
    if (
        not isinstance(expected_parts, dict)
        or set(expected_parts) != set(roots)
        or not isinstance(expected_zones, dict)
        or set(expected_zones) != set(roots)
    ):
        raise PackageError("M108B_PRODUCTION_EXPANDED_FACT_LOCK_INVALID")
    observed_parts: dict[str, int] = {}
    observed_zones: dict[str, int] = {}
    for domain in PRODUCTION_DOMAINS:
        root_ids = roots_by_domain.get(domain, [])
        if len(root_ids) != 3:
            raise PackageError("M108B_PRODUCTION_DOMAIN_ROOT_COUNT_INVALID")
        for recipe_id in root_ids:
            recipe = recipes.get(recipe_id)
            if not recipe or hashes.get(recipe_id) != canonical_sha256(recipe):
                raise PackageError("M108B_PRODUCTION_ROOT_HASH_DRIFT")
            if not root_slot_range[0] <= len(recipe.get("child_slots", [])) <= root_slot_range[1] or not all(slot.get("required") is True for slot in recipe["child_slots"]):
                raise PackageError("M108B_PRODUCTION_SLOT_CONTRACT_INVALID")
            if not root_zone_range[0] <= len(recipe.get("material_zones", [])) <= root_zone_range[1]:
                raise PackageError("M108B_PRODUCTION_ZONE_CONTRACT_INVALID")
            part_count, unique_zones = expanded_recipe_facts(recipe_id, recipes)
            observed_parts[recipe_id] = part_count
            observed_zones[recipe_id] = len(unique_zones)
            if expected_parts.get(recipe_id) != part_count or expected_zones.get(recipe_id) != len(unique_zones):
                raise PackageError("M108B_PRODUCTION_EXPANDED_FACT_DRIFT")
    if lock.get("expected_parts_per_root_range") != [min(observed_parts.values()), max(observed_parts.values())]:
        raise PackageError("M108B_PRODUCTION_EXPANDED_FACT_RANGE_DRIFT")
    if lock.get("expected_unique_zones_per_root_range") != [min(observed_zones.values()), max(observed_zones.values())]:
        raise PackageError("M108B_PRODUCTION_EXPANDED_FACT_RANGE_DRIFT")
    return emitted, lock


def production_self_test() -> None:
    build_production()
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp) / "component-recipes"
        import shutil
        shutil.copytree(PACK_ROOT, root)
        shutil.copy2(PRODUCTION_REGISTRY_PATH, root.parent / "production-component-recipe-registry.json")
        source_before = {
            path.relative_to(root): path.read_bytes()
            for path in root.glob("production-packs/*.json")
        }
        manifest_before = (root / "production-manifest.json").read_bytes()
        # The writer is the sole intentional drift-reconciliation path.  It
        # must touch the aggregate + lock only and leave all source packs
        # untouched, then the normal read-only verifier must accept output.
        write_production(root)
        assert manifest_before == (root / "production-manifest.json").read_bytes()
        assert source_before == {
            path.relative_to(root): path.read_bytes()
            for path in root.glob("production-packs/*.json")
        }
        build_production(root)
        policy_lock_path = root / "locks/m108b-production-v1.lock.json"
        policy_lock = load(policy_lock_path)
        policy_lock["root_slot_range"] = [6, 8]
        policy_lock["root_zone_range"] = [3, 5]
        policy_lock_path.write_text(json.dumps(policy_lock), encoding="utf-8")
        write_production(root)
        rewritten_policy_lock = load(policy_lock_path)
        assert rewritten_policy_lock["root_slot_range"] == [6, 8]
        assert rewritten_policy_lock["root_zone_range"] == [3, 5]
        build_production(root)
        expected_future_recipe_count = len(
            load(root / "production-packs/future-weapon-prop.json")["recipes"]
        )
        assert validate_production_pack("pack_future_weapon_prop", root)["recipes"] == expected_future_recipe_count
        invalid_pack_path = root / "production-packs/future-weapon-prop.json"
        invalid_pack = load(invalid_pack_path)
        recipe_with_child = next(
            recipe for recipe in invalid_pack["recipes"] if recipe.get("child_slots")
        )
        recipe_with_child["child_slots"][0]["child_recipe_id"] = "recipe_missing"
        invalid_pack_path.write_text(json.dumps(invalid_pack), encoding="utf-8")
        try:
            validate_production_pack("production-packs/future-weapon-prop.json", root)
        except PackageError as error:
            if "CHILD_REFERENCE_UNKNOWN" not in str(error):
                raise AssertionError(f"expected CHILD_REFERENCE_UNKNOWN, got {error}") from error
        else:
            raise AssertionError("source-only pack validation accepted unknown child recipe")
        shutil.rmtree(root)
        shutil.copytree(PACK_ROOT, root)
        invalid_profile_path = root / "production-packs/future-weapon-prop.json"
        invalid_profile_pack = load(invalid_profile_path)
        profile_recipe = next(
            recipe
            for recipe in invalid_profile_pack["recipes"]
            if recipe.get("shape_program_template", {}).get("profile_inputs")
        )
        profile_recipe["shape_program_template"]["profile_inputs"][0]["input_sha256"] = "0" * 64
        invalid_profile_path.write_text(json.dumps(invalid_profile_pack), encoding="utf-8")
        try:
            validate_production_pack("pack_future_weapon_prop", root)
        except PackageError as error:
            if "PROFILE_HASH_DRIFT" not in str(error):
                raise AssertionError(f"expected PROFILE_HASH_DRIFT, got {error}") from error
        else:
            raise AssertionError("source-only pack validation accepted profile hash drift")
        shutil.rmtree(root)
        shutil.copytree(PACK_ROOT, root)
        # Python dict equality considers `1 == 1.0`, but the frozen JSON
        # contract must not.  Exercise both resource carriers because a
        # SectionSet is compiled through the same runtime normalizer.
        for resource_kind, items in (
            (
                "profile_input",
                lambda recipe: recipe.get("shape_program_template", {}).get("profile_inputs", []),
            ),
            ("section_set", lambda recipe: recipe.get("section_sets", [])),
        ):
            invalid_numeric_path = root / "production-packs/future-weapon-prop.json"
            invalid_numeric_pack = load(invalid_numeric_path)
            numeric_recipe = next(recipe for recipe in invalid_numeric_pack["recipes"] if items(recipe))
            resource = items(numeric_recipe)[0]
            payload = resource["canonical_payload"]
            assert isinstance(payload.get("version"), int)
            payload["version"] = float(payload["version"])
            invalid_numeric_path.write_text(json.dumps(invalid_numeric_pack), encoding="utf-8")
            try:
                validate_production_pack("pack_future_weapon_prop", root)
            except PackageError as error:
                if "PROFILE_PAYLOAD_NOT_CANONICAL" not in str(error) or resource_kind not in str(error):
                    raise AssertionError(
                        f"expected non-canonical numeric {resource_kind} rejection, got {error}"
                    ) from error
            else:
                raise AssertionError(
                    f"source-only pack validation accepted non-canonical numeric {resource_kind}"
                )
            shutil.rmtree(root)
            shutil.copytree(PACK_ROOT, root)
        cases = {
            "duplicate": (root / "production-packs/future-weapon-prop.json", lambda v: v["recipes"].append(copy.deepcopy(v["recipes"][0])), "DUPLICATE_ID"),
            "cross-domain": (root / "production-packs/vehicle-concept.json", lambda v: v["recipes"][0].__setitem__("allowed_domains", ["pack_aircraft_concept"]), "CROSS_DOMAIN"),
            "pack-order": (root / "production-manifest.json", lambda v: v["packs"][0].__setitem__("order", 1), "PACK_ORDER_DRIFT"),
            "coverage": (root / "production-packs/aircraft-concept.json", lambda v: v["recipes"].pop(), "COVERAGE_DRIFT"),
            "hash": (root / "locks/m108b-production-v1.lock.json", lambda v: v["recipe_sha256"].__setitem__("recipe_m108b_prop_accent", "0" * 64), "RECIPE_HASH_DRIFT"),
            "path-escape": (root / "production-manifest.json", lambda v: v["packs"][0].__setitem__("path", "../production-component-recipe-registry.json"), "PATH_ESCAPE"),
        }
        for _, (path, mutate, expected) in cases.items():
            original = load(path)
            mutate(original)
            path.write_text(json.dumps(original), encoding="utf-8")
            try:
                build_production(root)
            except PackageError as error:
                if expected not in str(error):
                    raise AssertionError(f"expected {expected}, got {error}") from error
            else:
                raise AssertionError(f"negative production package case did not fail: {expected}")
            shutil.rmtree(root)
            shutil.copytree(PACK_ROOT, root)


def write_production(pack_root: Path = PACK_ROOT) -> None:
    """Regenerate only the M108B aggregate registry and lock from domain packs.

    This is deliberately opt-in.  Normal CI uses ``--production`` and fails if
    a domain edit has not been explicitly packaged by its owner.  The writer
    never touches C105 files, manifests, or any domain-owned source pack.
    """
    manifest = load(pack_root / "production-manifest.json")
    if (
        manifest.get("schema_version") != "M108BProductionRecipePackManifest@1"
        or manifest.get("registry_id") != "registry_m108b_production_concept_v1"
        or manifest.get("policy_version") != "ComponentRecipePolicy@1"
    ):
        raise PackageError("M108B_PRODUCTION_MANIFEST_INVALID")
    existing_lock_path = fixture_relative(pack_root, str(manifest.get("lock_path", "")), pack_root)
    existing_lock = load(existing_lock_path)
    if existing_lock.get("schema_version") != "M108BProductionRecipeLock@1" or existing_lock.get("registry_id") != manifest.get("registry_id"):
        raise PackageError("M108B_PRODUCTION_LOCK_INVALID")
    root_slot_range = policy_range(existing_lock.get("root_slot_range", [5, 5]), "ROOT_SLOT_RANGE")
    root_zone_range = policy_range(existing_lock.get("root_zone_range", [3, 3]), "ROOT_ZONE_RANGE")
    entries = manifest.get("packs")
    if not isinstance(entries, list) or len(entries) != len(PRODUCTION_DOMAINS):
        raise PackageError("M108B_PRODUCTION_DOMAIN_COVERAGE_INVALID")
    selected: list[dict[str, Any]] = []
    seen_recipes: set[str] = set()
    seen_packs: set[str] = set()
    seen_domains: set[str] = set()
    pack_order: list[str] = []
    roots_by_domain: dict[str, list[str]] = {}
    for expected_order, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise PackageError("M108B_PRODUCTION_MANIFEST_INVALID")
        pack_id = entry.get("pack_id")
        domain = entry.get("domain_pack_id")
        if (
            not isinstance(pack_id, str)
            or not isinstance(domain, str)
            or not isinstance(entry.get("path"), str)
            or entry.get("order") != expected_order
            or pack_id in seen_packs
            or domain in seen_domains
        ):
            raise PackageError("M108B_PRODUCTION_PACK_ORDER_DRIFT")
        pack = load(fixture_relative(pack_root, entry["path"], pack_root))
        if (
            pack.get("schema_version") != "M108BProductionRecipeDomainPack@1"
            or pack.get("pack_id") != pack_id
            or pack.get("domain_pack_id") != domain
            or not isinstance(pack.get("recipes"), list)
            or not isinstance(pack.get("root_recipe_ids"), list)
        ):
            raise PackageError("M108B_PRODUCTION_PACK_INVALID")
        seen_packs.add(pack_id)
        seen_domains.add(domain)
        pack_order.append(pack_id)
        roots_by_domain[domain] = list(pack["root_recipe_ids"])
        for recipe in pack["recipes"]:
            if not isinstance(recipe, dict) or not isinstance(recipe.get("recipe_id"), str):
                raise PackageError("M108B_PRODUCTION_RECIPE_INVALID")
            validate_embedded_profile_hashes(recipe)
            recipe_id = recipe["recipe_id"]
            if recipe_id in seen_recipes:
                raise PackageError("M108B_PRODUCTION_DUPLICATE_ID")
            if recipe.get("allowed_domains") != [domain]:
                raise PackageError("M108B_PRODUCTION_CROSS_DOMAIN")
            seen_recipes.add(recipe_id)
            selected.append(copy.deepcopy(recipe))
    if seen_domains != set(PRODUCTION_DOMAINS):
        raise PackageError("M108B_PRODUCTION_DOMAIN_COVERAGE_INVALID")
    roots = [recipe_id for domain in PRODUCTION_DOMAINS for recipe_id in roots_by_domain.get(domain, [])]
    if len(roots) != 12 or len(set(roots)) != len(roots) or any(recipe_id not in seen_recipes for recipe_id in roots):
        raise PackageError("M108B_PRODUCTION_ROOT_COUNT_INVALID")
    for recipe_id in roots:
        recipe = next(recipe for recipe in selected if recipe["recipe_id"] == recipe_id)
        if not root_slot_range[0] <= len(recipe.get("child_slots", [])) <= root_slot_range[1] or not all(slot.get("required") is True for slot in recipe["child_slots"]):
            raise PackageError("M108B_PRODUCTION_SLOT_CONTRACT_INVALID")
        if not root_zone_range[0] <= len(recipe.get("material_zones", [])) <= root_zone_range[1]:
            raise PackageError("M108B_PRODUCTION_ZONE_CONTRACT_INVALID")
    catalog = {recipe["recipe_id"]: recipe for recipe in selected}
    expected_parts_by_root: dict[str, int] = {}
    expected_unique_zones_by_root: dict[str, int] = {}
    for recipe_id in roots:
        part_count, unique_zones = expanded_recipe_facts(recipe_id, catalog)
        expected_parts_by_root[recipe_id] = part_count
        expected_unique_zones_by_root[recipe_id] = len(unique_zones)
    registry = {
        "schema_version": "EditableComponentRecipeRegistry@1",
        "registry_id": manifest["registry_id"],
        "policy_version": manifest["policy_version"],
        "recipes": selected,
    }
    lock = {
        "schema_version": "M108BProductionRecipeLock@1",
        "registry_id": manifest["registry_id"],
        "registry_sha256": canonical_sha256(registry),
        "pack_order": pack_order,
        "recipe_order": [recipe["recipe_id"] for recipe in selected],
        "recipe_sha256": {recipe["recipe_id"]: canonical_sha256(recipe) for recipe in selected},
        "root_recipe_sha256": {recipe_id: canonical_sha256(next(recipe for recipe in selected if recipe["recipe_id"] == recipe_id)) for recipe_id in roots},
        "root_slot_range": list(root_slot_range),
        "root_zone_range": list(root_zone_range),
        "expected_root_count": 12,
        "expected_parts_per_root_range": [
            min(expected_parts_by_root.values()),
            max(expected_parts_by_root.values()),
        ],
        "expected_unique_zones_per_root_range": [
            min(expected_unique_zones_by_root.values()),
            max(expected_unique_zones_by_root.values()),
        ],
        "expected_parts_by_root": expected_parts_by_root,
        "expected_unique_zones_by_root": expected_unique_zones_by_root,
    }
    aggregate_path = fixture_relative(pack_root, str(manifest.get("aggregate_path", "")), pack_root.parent)
    lock_path = fixture_relative(pack_root, str(manifest.get("lock_path", "")), pack_root)
    aggregate_path.write_text(json.dumps(registry, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    lock_path.write_text(json.dumps(lock, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def validate_production_pack(selector: str, pack_root: Path = PACK_ROOT) -> dict[str, Any]:
    """Validate one domain-owned source pack without reading aggregate or lock.

    Parallel asset authors use this fast check before the release owner invokes
    ``--write-production``.  Cross-pack child references are resolved from the
    four source packs, never from the generated Rust compatibility registry.
    """
    manifest = load(pack_root / "production-manifest.json")
    entries = manifest.get("packs")
    if manifest.get("schema_version") != "M108BProductionRecipePackManifest@1" or not isinstance(entries, list):
        raise PackageError("M108B_PRODUCTION_MANIFEST_INVALID")
    matched = [
        entry for entry in entries
        if isinstance(entry, dict) and selector in {entry.get("domain_pack_id"), entry.get("path")}
    ]
    if len(matched) != 1:
        raise PackageError("M108B_PRODUCTION_PACK_SELECTOR_INVALID")
    source_packs: list[tuple[dict[str, Any], dict[str, Any]]] = []
    catalog: dict[str, dict[str, Any]] = {}
    for entry in entries:
        if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
            raise PackageError("M108B_PRODUCTION_MANIFEST_INVALID")
        pack = load(fixture_relative(pack_root, entry["path"], pack_root))
        if (
            pack.get("schema_version") != "M108BProductionRecipeDomainPack@1"
            or pack.get("pack_id") != entry.get("pack_id")
            or pack.get("domain_pack_id") != entry.get("domain_pack_id")
            or not isinstance(pack.get("recipes"), list)
            or not isinstance(pack.get("root_recipe_ids"), list)
        ):
            raise PackageError("M108B_PRODUCTION_PACK_INVALID")
        source_packs.append((entry, pack))
        for recipe in pack["recipes"]:
            if not isinstance(recipe, dict) or not isinstance(recipe.get("recipe_id"), str):
                raise PackageError("M108B_PRODUCTION_RECIPE_INVALID")
            validate_embedded_profile_hashes(recipe)
            if recipe["recipe_id"] in catalog:
                raise PackageError("M108B_PRODUCTION_DUPLICATE_ID")
            catalog[recipe["recipe_id"]] = recipe

    entry = matched[0]
    pack = next(pack for candidate, pack in source_packs if candidate is entry)
    domain = pack["domain_pack_id"]
    local_ids = {recipe["recipe_id"] for recipe in pack["recipes"]}
    if len(local_ids) != len(pack["recipes"]):
        raise PackageError("M108B_PRODUCTION_DUPLICATE_ID")
    roots = pack["root_recipe_ids"]
    if len(roots) != 3 or len(set(roots)) != len(roots) or not set(roots).issubset(local_ids):
        raise PackageError("M108B_PRODUCTION_ROOT_COUNT_INVALID")
    for recipe in pack["recipes"]:
        if recipe.get("allowed_domains") != [domain]:
            raise PackageError("M108B_PRODUCTION_CROSS_DOMAIN")
        connectors = {
            connector.get("connector_id")
            for connector in recipe.get("connectors", [])
            if isinstance(connector, dict) and isinstance(connector.get("connector_id"), str)
        }
        for slot in recipe.get("child_slots", []):
            if not isinstance(slot, dict) or not isinstance(slot.get("child_recipe_id"), str):
                raise PackageError("M108B_PRODUCTION_CHILD_REFERENCE_INVALID")
            child = catalog.get(slot["child_recipe_id"])
            if child is None:
                raise PackageError("M108B_PRODUCTION_CHILD_REFERENCE_UNKNOWN")
            child_connectors = {
                connector.get("connector_id")
                for connector in child.get("connectors", [])
                if isinstance(connector, dict) and isinstance(connector.get("connector_id"), str)
            }
            if slot.get("parent_connector_id") not in connectors or slot.get("child_connector_id") not in child_connectors:
                raise PackageError("M108B_PRODUCTION_CHILD_CONNECTOR_INVALID")
            if child.get("allowed_domains") != [domain]:
                raise PackageError("M108B_PRODUCTION_CHILD_CROSS_DOMAIN")
            accepted_roles = slot.get("accepted_roles")
            if not isinstance(accepted_roles, list) or child.get("component_role") not in accepted_roles:
                raise PackageError("M108B_PRODUCTION_CHILD_ROLE_INVALID")
    return {"pack_id": pack["pack_id"], "domain_pack_id": domain, "recipes": len(pack["recipes"]), "roots": len(roots)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--production", action="store_true", help="verify the separate M108B production registry and lock")
    parser.add_argument("--write-production", action="store_true", help="explicitly regenerate only M108B aggregate registry + lock from domain-owned packs")
    parser.add_argument("--validate-production-pack", metavar="DOMAIN_OR_PATH", help="read-only source-pack validation; does not read aggregate registry or lock")
    args = parser.parse_args()
    if args.validate_production_pack:
        report = validate_production_pack(args.validate_production_pack)
        print(f"M108B production source pack validated: pack_id={report['pack_id']}, domain={report['domain_pack_id']}, recipes={report['recipes']}, roots={report['roots']}")
        if not (args.verify or args.self_test or args.production or args.write_production):
            return 0
    if args.write_production:
        write_production()
    emitted, locks = build()
    production = build_production() if args.production or args.write_production else None
    if args.self_test:
        self_test()
        if args.production:
            production_self_test()
    suffix = "" if production is None else f", production_recipes={len(production[0]['recipes'])}, production_registry_sha256={production[1]['registry_sha256']}"
    print(f"Component Recipe package verified: recipes={len(emitted['recipes'])}, registry_sha256={locks['aggregate_registry_sha256']}{suffix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
