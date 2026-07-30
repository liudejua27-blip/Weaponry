#!/usr/bin/env python3
"""Load and validate the frozen C111B visual acceptance contract.

The contract is a gate input, not a quality result.  It freezes the authorized
reference, must-show/must-not-show boundary, score thresholds, renderer views,
resource budgets and the independent-human protocol.  This module deliberately
uses only stdlib JSON and hashes so the offline C111B gate cannot silently call
an external service.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Mapping


SCHEMA_VERSION = "C111BVisualAcceptanceContract@2"
CONTRACT_RELATIVE_PATH = Path(
    "packages/concept-spec/fixtures/c111b-visual-acceptance-contract.json"
)
REQUIRED_DETAIL_CLASSES = {
    "service_panel",
    "joint_stack",
    "auxiliary_linkage",
    "cable_clamps",
    "gripper_hinges",
    "decal",
    "wear",
}
REQUIRED_TEXTURE_ROLES = {
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
}
REQUIRED_VIEW_IDS = {
    "iso",
    "front",
    "back",
    "left",
    "right",
    "top",
    "gripper_iso",
    "gripper_front",
}


def _mapping(value: Any, field: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise AssertionError(f"C111B_ACCEPTANCE_{field.upper()}_INVALID")
    return value


def _string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise AssertionError(f"C111B_ACCEPTANCE_{field.upper()}_INVALID")
    return value


def _sha256(value: Any, field: str) -> str:
    text = _string(value, field)
    if len(text) != 64 or any(char not in "0123456789abcdef" for char in text):
        raise AssertionError(f"C111B_ACCEPTANCE_{field.upper()}_HASH_INVALID")
    return text


def load_c111b_visual_acceptance_contract(
    root: Path, inventory: Mapping[str, Any]
) -> tuple[Mapping[str, Any], str]:
    """Return the frozen contract and its raw-file SHA-256 after validation."""

    path = root / CONTRACT_RELATIVE_PATH
    raw = path.read_bytes()
    try:
        contract = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise AssertionError("C111B_ACCEPTANCE_JSON_INVALID") from exc
    contract = _mapping(contract, "contract")
    if contract.get("schema_version") != SCHEMA_VERSION:
        raise AssertionError("C111B_ACCEPTANCE_SCHEMA_INVALID")
    if contract.get("contract_id") != "c111b_robotic_arm_visual_acceptance_v2":
        raise AssertionError("C111B_ACCEPTANCE_ID_INVALID")
    if contract.get("status") != "frozen" or contract.get("formal_eligible") is not False:
        raise AssertionError("C111B_ACCEPTANCE_STATUS_INVALID")
    if contract.get("root_recipe_id") != "recipe_c111_arm_golden_surface":
        raise AssertionError("C111B_ACCEPTANCE_ROOT_RECIPE_INVALID")
    if contract.get("registry_id") != "registry_c111_golden_surface_robotic_arm_v1":
        raise AssertionError("C111B_ACCEPTANCE_REGISTRY_INVALID")

    reference = _mapping(contract.get("authorized_reference"), "authorized_reference")
    inventory_reference = _mapping(inventory.get("reference_evidence"), "inventory_reference")
    if (
        reference.get("reference_set_id") != inventory_reference.get("reference_set_id")
        or reference.get("source_kind") != "user_authorized_reference"
        or reference.get("repository_storage") != "external_user_authorized"
        or reference.get("status") != "digest_verified"
        or reference.get("source_filename") != inventory_reference.get("source_filename")
        or _sha256(reference.get("sha256"), "reference.sha256")
        != _sha256(inventory_reference.get("sha256"), "inventory_reference.sha256")
    ):
        raise AssertionError("C111B_ACCEPTANCE_REFERENCE_LINEAGE_INVALID")

    must_show = contract.get("must_show")
    if not isinstance(must_show, list) or len(must_show) != len(REQUIRED_DETAIL_CLASSES):
        raise AssertionError("C111B_ACCEPTANCE_MUST_SHOW_INVALID")
    shown_classes = set()
    for item in must_show:
        item = _mapping(item, "must_show_item")
        detail_class = _string(item.get("detail_class"), "must_show.detail_class")
        if detail_class in shown_classes:
            raise AssertionError("C111B_ACCEPTANCE_MUST_SHOW_DUPLICATE")
        shown_classes.add(detail_class)
        if detail_class not in REQUIRED_DETAIL_CLASSES:
            raise AssertionError("C111B_ACCEPTANCE_MUST_SHOW_CLASS_INVALID")
        if not isinstance(item.get("required_part_roles"), list) or not item["required_part_roles"]:
            raise AssertionError("C111B_ACCEPTANCE_MUST_SHOW_PART_ROLES_INVALID")
        if not isinstance(item.get("required_claim_ids"), list) or not item["required_claim_ids"]:
            raise AssertionError("C111B_ACCEPTANCE_MUST_SHOW_CLAIMS_INVALID")
    if shown_classes != REQUIRED_DETAIL_CLASSES:
        raise AssertionError("C111B_ACCEPTANCE_MUST_SHOW_COVERAGE_INVALID")

    must_not_show = contract.get("must_not_show")
    if not isinstance(must_not_show, list) or len(must_not_show) < 4:
        raise AssertionError("C111B_ACCEPTANCE_MUST_NOT_SHOW_INVALID")
    if any(not isinstance(item, str) or not item for item in must_not_show):
        raise AssertionError("C111B_ACCEPTANCE_MUST_NOT_SHOW_ITEM_INVALID")

    claims = contract.get("claims")
    if not isinstance(claims, list) or len(claims) != 4:
        raise AssertionError("C111B_ACCEPTANCE_CLAIMS_INVALID")
    claim_by_id = {}
    for claim in claims:
        claim = _mapping(claim, "claim")
        claim_id = _string(claim.get("claim_id"), "claim.claim_id")
        if claim_id in claim_by_id:
            raise AssertionError("C111B_ACCEPTANCE_CLAIM_DUPLICATE")
        claim_by_id[claim_id] = claim
        if claim.get("critical") is not True:
            raise AssertionError("C111B_ACCEPTANCE_CLAIM_NOT_CRITICAL")
        if not isinstance(claim.get("required_evidence"), list) or not claim["required_evidence"]:
            raise AssertionError("C111B_ACCEPTANCE_CLAIM_EVIDENCE_INVALID")
    expected_claims = {
        "macro_structure": ("macro", 7600),
        "meso_structure": ("meso", 6500),
        "micro_surface": ("micro", 5000),
    }
    for claim_id, (level, threshold) in expected_claims.items():
        claim = claim_by_id.get(claim_id)
        if claim is None or claim.get("level") != level or claim.get("minimum_similarity_bps") != threshold:
            raise AssertionError("C111B_ACCEPTANCE_SCORE_THRESHOLD_INVALID")
        if claim.get("not_visible_allowed") is not False:
            raise AssertionError("C111B_ACCEPTANCE_NOT_VISIBLE_POLICY_INVALID")
    pbr_claim = claim_by_id.get("pbr_lineage")
    if (
        pbr_claim is None
        or pbr_claim.get("level") != "pbr"
        or pbr_claim.get("not_visible_allowed") is not False
        or set(pbr_claim.get("required_texture_roles", [])) != REQUIRED_TEXTURE_ROLES
    ):
        raise AssertionError("C111B_ACCEPTANCE_PBR_POLICY_INVALID")

    views = contract.get("fixed_views")
    if not isinstance(views, list) or set(views) != REQUIRED_VIEW_IDS or len(views) != len(REQUIRED_VIEW_IDS):
        raise AssertionError("C111B_ACCEPTANCE_VIEW_SET_INVALID")

    budgets = _mapping(contract.get("budgets"), "budgets")
    triangles = _mapping(budgets.get("production_triangle_count"), "triangle_budget")
    if triangles.get("minimum") != 80_000 or triangles.get("maximum") != 150_000:
        raise AssertionError("C111B_ACCEPTANCE_TRIANGLE_BUDGET_INVALID")
    texture = _mapping(budgets.get("texture"), "texture_budget")
    if (
        texture.get("minimum_resolution") != 1024
        or set(texture.get("required_roles", [])) != REQUIRED_TEXTURE_ROLES
        or texture.get("maximum_map_count") != 320
    ):
        raise AssertionError("C111B_ACCEPTANCE_TEXTURE_BUDGET_INVALID")
    timing = _mapping(budgets.get("timing"), "timing_budget")
    if timing.get("record_stage_durations") is not True or timing.get("target_total_seconds") != 120:
        raise AssertionError("C111B_ACCEPTANCE_TIMING_BUDGET_INVALID")
    provider = _mapping(budgets.get("provider"), "provider_budget")
    generation_provider = _mapping(
        provider.get("generation"), "generation_provider_budget"
    )
    if (
        generation_provider.get("network_allowed") is not False
        or generation_provider.get("maximum_calls") != 0
        or generation_provider.get("record_cache_hits") is not True
        or generation_provider.get("record_usage") is not True
    ):
        raise AssertionError("C111B_ACCEPTANCE_GENERATION_PROVIDER_BUDGET_INVALID")
    comparison_provider = _mapping(
        provider.get("visual_comparison"), "comparison_provider_budget"
    )
    if (
        comparison_provider.get("requires_explicit_user_authorization") is not True
        or comparison_provider.get("network_allowed_when_authorized") is not True
        or comparison_provider.get("maximum_calls_per_candidate") != 3
        or comparison_provider.get("maximum_same_intent_repairs") != 2
        or comparison_provider.get("record_cache_hits") is not True
        or comparison_provider.get("record_usage") is not True
    ):
        raise AssertionError("C111B_ACCEPTANCE_COMPARISON_PROVIDER_BUDGET_INVALID")
    cost = _mapping(budgets.get("cost"), "cost_budget")
    if (
        cost.get("currency") != "USD"
        or cost.get("generation_maximum_variable_cost_microusd") != 0
        or cost.get("visual_comparison_maximum_variable_cost_microusd") != 100_000
        or cost.get("hard_stop_before_call") is not True
        or cost.get("record_estimate") is not True
    ):
        raise AssertionError("C111B_ACCEPTANCE_COST_BUDGET_INVALID")

    review = _mapping(contract.get("independent_human_review"), "human_review")
    if (
        review.get("automatic_gate_must_pass_first") is not True
        or review.get("independent_reviewers") != 3
        or review.get("score_minimum") != 4
        or review.get("score_scale") != [1, 2, 3, 4, 5]
        or review.get("agent_or_vlm_substitution_allowed") is not False
    ):
        raise AssertionError("C111B_ACCEPTANCE_HUMAN_PROTOCOL_INVALID")

    lineage = _mapping(contract.get("evidence_lineage"), "evidence_lineage")
    if (
        lineage.get("inventory_id") != inventory.get("inventory_id")
        or lineage.get("compiled_glb_sha256")
        != _mapping(inventory.get("compiled_evidence"), "compiled_evidence").get("production_glb_sha256")
        or set(lineage.get("required_inputs", []))
        != {
            "c111b_visual_acceptance_contract",
            "c111_golden_surface_visual_detail_inventory",
            "c111_structural_detail_contract",
            "production_glb_readback",
            "same_renderer_fixed_views",
            "visual_convergence_report",
            "visual_reference_comparison_report",
        }
    ):
        raise AssertionError("C111B_ACCEPTANCE_EVIDENCE_LINEAGE_INVALID")

    return contract, hashlib.sha256(raw).hexdigest()


def summarize_contract(contract: Mapping[str, Any], contract_sha256: str) -> dict[str, Any]:
    budgets = _mapping(contract["budgets"], "budgets")
    triangles = _mapping(budgets["production_triangle_count"], "triangle_budget")
    texture = _mapping(budgets["texture"], "texture_budget")
    provider = _mapping(budgets["provider"], "provider_budget")
    generation_provider = _mapping(
        provider["generation"], "generation_provider_budget"
    )
    comparison_provider = _mapping(
        provider["visual_comparison"], "comparison_provider_budget"
    )
    cost = _mapping(budgets["cost"], "cost_budget")
    return {
        "schema_version": contract["schema_version"],
        "contract_id": contract["contract_id"],
        "contract_sha256": contract_sha256,
        "reference_sha256": _mapping(contract["authorized_reference"], "reference")["sha256"],
        "fixed_view_ids": list(contract["fixed_views"]),
        "required_detail_classes": sorted(REQUIRED_DETAIL_CLASSES),
        "production_triangle_minimum": triangles["minimum"],
        "production_triangle_maximum": triangles["maximum"],
        "texture_required_roles": sorted(texture["required_roles"]),
        "generation_provider_maximum_calls": generation_provider["maximum_calls"],
        "visual_comparison_requires_explicit_user_authorization": comparison_provider[
            "requires_explicit_user_authorization"
        ],
        "visual_comparison_maximum_calls_per_candidate": comparison_provider[
            "maximum_calls_per_candidate"
        ],
        "visual_comparison_maximum_variable_cost_microusd": cost[
            "visual_comparison_maximum_variable_cost_microusd"
        ],
        "formal_eligible": contract["formal_eligible"],
    }
