#!/usr/bin/env python3
"""Validate the no-call ForgeCAD general-mechanical Provider evaluation contract."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CONTRACT = ROOT / "evaluations" / "agent-provider-v1" / "contract.json"
DEFAULT_TRUTH_SET = ROOT / "evaluations" / "agent-provider-v1" / "truth_set.json"
EXPECTED_PACKS = {
    "pack_future_weapon_prop",
    "pack_vehicle_concept",
    "pack_aircraft_concept",
    "pack_robotic_arm_concept",
}
PROHIBITED_NORMAL_TERMS = ("制造", "加工", "材料配方", "性能参数", "适航", "认证", "扭矩计算", "控制程序", "现实枪械")


class ContractError(ValueError):
    pass


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ContractError(f"E001_CONTRACT_UNREADABLE: {path.name}") from exc
    if not isinstance(value, dict):
        raise ContractError(f"E001_CONTRACT_INVALID: {path.name} must contain an object")
    return value


def expanded_normal_cases(truth_set: dict[str, Any]) -> list[dict[str, Any]]:
    modifiers = truth_set.get("visual_modifiers")
    matrix = truth_set.get("normal_case_matrix")
    if not isinstance(modifiers, list) or not isinstance(matrix, list):
        raise ContractError("E001_TRUTH_SET_INVALID: missing normal case matrix")
    cases: list[dict[str, Any]] = []
    for pack in matrix:
        if not isinstance(pack, dict):
            raise ContractError("E001_TRUTH_SET_INVALID: pack matrix item")
        pack_id = pack.get("domain_pack_id")
        stems = pack.get("brief_stems")
        roles = pack.get("required_role_groups")
        scope = pack.get("safety_scope")
        if not isinstance(pack_id, str) or not isinstance(stems, list) or not isinstance(roles, list) or not isinstance(scope, str):
            raise ContractError("E001_TRUTH_SET_INVALID: incomplete pack matrix item")
        for stem_index, stem in enumerate(stems, start=1):
            if not isinstance(stem, str) or not stem.strip():
                raise ContractError("E001_TRUTH_SET_INVALID: empty brief stem")
            for modifier_index, modifier in enumerate(modifiers, start=1):
                if not isinstance(modifier, str) or not modifier.strip():
                    raise ContractError("E001_TRUTH_SET_INVALID: empty visual modifier")
                cases.append(
                    {
                        "case_id": f"normal_{pack_id.removeprefix('pack_')}_{stem_index:02d}_{modifier_index:02d}",
                        "domain_pack_id": pack_id,
                        "brief": f"{stem}，{modifier}。",
                        "required_role_groups": roles,
                        "safety_scope": scope,
                    }
                )
    return cases


def validate(contract: dict[str, Any], truth_set: dict[str, Any]) -> dict[str, Any]:
    if contract.get("schema_version") != "ForgeCADProviderEvaluationContract@1":
        raise ContractError("E001_CONTRACT_INVALID: unsupported contract schema")
    if truth_set.get("schema_version") != "ForgeCADProviderTruthSet@1":
        raise ContractError("E001_TRUTH_SET_INVALID: unsupported truth-set schema")
    if contract.get("default_execution") != "dry_run":
        raise ContractError("E001_CONTRACT_UNSAFE: dry_run must be the default")

    live = contract.get("live_execution")
    budget = contract.get("budget")
    acceptance = contract.get("acceptance")
    evidence = contract.get("evidence")
    if not all(isinstance(section, dict) for section in (live, budget, acceptance, evidence)):
        raise ContractError("E001_CONTRACT_INVALID: missing contract sections")
    required_flags = {"--confirm-live-provider", "--confirmed-budget-cny", "--evaluation-run-id"}
    if set(live.get("required_operator_flags", [])) != required_flags:
        raise ContractError("E001_CONTRACT_UNSAFE: live authorization flags changed")
    forbidden_automatic = {"ci", "first_launch", "ordinary_agent_turn", "retry_after_failure"}
    if not forbidden_automatic.issubset(set(live.get("automatic_invocation_forbidden", []))):
        raise ContractError("E001_CONTRACT_UNSAFE: automatic live invocation is not fully forbidden")
    if budget.get("default_spend_cap_cny") != 0 or budget.get("max_retries_per_case") != 0:
        raise ContractError("E001_CONTRACT_UNSAFE: default budget or automatic retries changed")
    for field in ("max_operator_approved_budget_cny", "max_provider_requests", "max_request_timeout_seconds", "max_output_tokens_per_request", "max_total_output_tokens", "max_total_tokens_when_usage_is_reported"):
        value = budget.get(field)
        if not isinstance(value, int) or value <= 0:
            raise ContractError(f"E001_CONTRACT_INVALID: {field}")
    case_plan = contract.get("case_plan")
    if not isinstance(case_plan, dict):
        raise ContractError("E001_CONTRACT_INVALID: missing case plan")
    if case_plan.get("total_planned_cases") != 100 or case_plan.get("normal_provider_requests") != 80:
        raise ContractError("E001_CONTRACT_INVALID: fixed case plan must be 100 cases and 80 Provider requests")
    if budget["max_provider_requests"] != case_plan["normal_provider_requests"]:
        raise ContractError("E001_CONTRACT_INVALID: Provider request limit must exclude local safe-stop cases")
    if acceptance.get("asset_or_snapshot_writes_before_confirmation") != 0:
        raise ContractError("E001_CONTRACT_UNSAFE: evaluation may not write assets or snapshots")
    for field in ("store_raw_prompts", "store_raw_provider_responses", "store_api_key", "store_base_url", "store_model_identifier"):
        if evidence.get(field) is not False:
            raise ContractError(f"E001_CONTRACT_UNSAFE: {field} must be false")

    normal_cases = expanded_normal_cases(truth_set)
    normal_ids = [case["case_id"] for case in normal_cases]
    if len(normal_cases) != 80 or len(set(normal_ids)) != len(normal_ids):
        raise ContractError("E001_TRUTH_SET_INVALID: expected 80 unique normal cases")
    pack_counts = {pack_id: sum(case["domain_pack_id"] == pack_id for case in normal_cases) for pack_id in EXPECTED_PACKS}
    if set(pack_counts) != EXPECTED_PACKS or any(count != 20 for count in pack_counts.values()):
        raise ContractError("E001_TRUTH_SET_INVALID: expected 20 normal cases per domain")
    for case in normal_cases:
        if any(term in case["brief"] for term in PROHIBITED_NORMAL_TERMS):
            raise ContractError(f"E001_TRUTH_SET_UNSAFE: prohibited normal brief in {case['case_id']}")

    clarification_cases = truth_set.get("clarification_or_rejection_cases")
    if not isinstance(clarification_cases, list) or len(clarification_cases) != 20:
        raise ContractError("E001_TRUTH_SET_INVALID: expected 20 clarification or rejection cases")
    clarification_ids = [case.get("case_id") for case in clarification_cases if isinstance(case, dict)]
    if len(clarification_ids) != 20 or len(set(clarification_ids)) != 20:
        raise ContractError("E001_TRUTH_SET_INVALID: clarification case IDs must be unique")
    expected_outcomes = {case.get("expected") for case in clarification_cases if isinstance(case, dict)}
    if expected_outcomes != {"ambiguous", "unsupported"}:
        raise ContractError("E001_TRUTH_SET_INVALID: clarification and rejection paths are both required")
    if len(normal_cases) != case_plan["normal_provider_requests"] or len(normal_cases) + len(clarification_cases) != case_plan["total_planned_cases"]:
        raise ContractError("E001_CONTRACT_INVALID: case plan differs from fixed truth set")

    return {
        "schema_version": "ForgeCADProviderEvaluationDryRun@1",
        "ok": True,
        "mode": "dry_run",
        "network_calls_made": 0,
        "live_calls_authorized": False,
        "asset_or_snapshot_writes": 0,
        "normal_case_count": len(normal_cases),
        "clarification_or_rejection_case_count": len(clarification_cases),
        "pack_counts": pack_counts,
        "budget": {
            "default_spend_cap_cny": budget["default_spend_cap_cny"],
            "max_operator_approved_budget_cny": budget["max_operator_approved_budget_cny"],
            "max_provider_requests": budget["max_provider_requests"],
            "max_retries_per_case": budget["max_retries_per_case"],
            "max_request_timeout_seconds": budget["max_request_timeout_seconds"],
            "max_total_output_tokens": budget["max_total_output_tokens"],
        },
        "live_run_requires": live["required_operator_flags"],
        "fixture_sha256": hashlib.sha256(_canonical_json(truth_set).encode("utf-8")).hexdigest(),
    }


def _canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate ForgeCAD's no-call Provider evaluation contract.")
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--truth-set", type=Path, default=DEFAULT_TRUTH_SET)
    args = parser.parse_args()
    try:
        report = validate(load_json(args.contract), load_json(args.truth_set))
    except ContractError as exc:
        print(json.dumps({"ok": False, "error": str(exc), "network_calls_made": 0}, ensure_ascii=False, indent=2))
        return 2
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
