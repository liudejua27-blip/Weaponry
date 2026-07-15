#!/usr/bin/env python3
"""Focused, no-network regression tests for FGC-E001's evaluation contract."""

from __future__ import annotations

from copy import deepcopy

from check_agent_provider_evaluation_contract import ContractError, load_json, validate


def _assert(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def _reject(contract: dict, truth_set: dict, expected: str) -> None:
    try:
        validate(contract, truth_set)
    except ContractError as exc:
        _assert(expected in str(exc), f"expected {expected}, got {exc}")
        return
    raise AssertionError(f"expected contract rejection containing {expected}")


def main() -> int:
    from check_agent_provider_evaluation_contract import DEFAULT_CONTRACT, DEFAULT_TRUTH_SET

    contract = load_json(DEFAULT_CONTRACT)
    truth_set = load_json(DEFAULT_TRUTH_SET)
    report = validate(contract, truth_set)
    _assert(report["network_calls_made"] == 0, "dry run must never call a Provider")
    _assert(report["normal_case_count"] == 80, "must expand 80 normal four-domain cases")
    _assert(report["clarification_or_rejection_case_count"] == 20, "must include 20 safe-stop cases")
    _assert(report["budget"]["default_spend_cap_cny"] == 0, "default cost must be zero")
    _assert(report["budget"]["max_provider_requests"] == 80, "safe-stop cases must not call a Provider")

    unsafe_budget = deepcopy(contract)
    unsafe_budget["budget"]["default_spend_cap_cny"] = 1
    _reject(unsafe_budget, truth_set, "default budget")

    unsafe_live = deepcopy(contract)
    unsafe_live["live_execution"]["automatic_invocation_forbidden"].remove("ci")
    _reject(unsafe_live, truth_set, "automatic live invocation")

    incomplete_truth_set = deepcopy(truth_set)
    incomplete_truth_set["normal_case_matrix"][0]["brief_stems"].pop()
    _reject(contract, incomplete_truth_set, "80 unique normal cases")

    print("FGC-E001 Provider evaluation contract smoke passed (no network calls)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
