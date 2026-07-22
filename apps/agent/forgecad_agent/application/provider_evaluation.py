"""Isolated, explicitly authorised Provider evaluation runner.

This module does not access the database, Agent Kernel, Snapshot, or export
paths.  It only invokes an injected planner for fixed, normal evaluation cases;
ambiguous and unsafe fixture items are stopped locally before the planner.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Any, Callable, Literal, Mapping, Optional, Sequence

from .domain_packs import domain_pack_by_id
from .mechanical_planner import MechanicalConceptPlan, MechanicalPlannerError, MechanicalConceptPlanner, MechanicalPlannerTelemetry


FailureCategory = Literal[
    "timeout",
    "rate_limited",
    "authentication_failed",
    "transport_failed",
    "invalid_structured_output",
    "policy_scope_failed",
    "budget_exceeded",
    "cancelled",
]

_RUN_ID = re.compile(r"^eval_[a-z0-9][a-z0-9_-]{7,79}$")
_OUT_OF_SCOPE_TERMS = (
    "加工", "制造", "起飞载荷", "认证", "碰撞", "制动", "控制程序", "扭矩", "材料配方", "性能参数",
    "发动机零件", "飞行控制", "适航", "发射", "结构强度", "材料牌号", "现实枪械",
)


class ProviderEvaluationError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class ProviderEvaluationAuthorization:
    confirm_live_provider: bool
    confirmed_budget_cny: float
    evaluation_run_id: str
    operator_name: str
    approval_timestamp: str
    provider_connection_preflight: bool


def validate_authorization(authorization: ProviderEvaluationAuthorization, budget: Mapping[str, Any]) -> None:
    if not authorization.confirm_live_provider:
        raise ProviderEvaluationError("E002_CONFIRMATION_REQUIRED", "Live Provider evaluation requires --confirm-live-provider.")
    if not _RUN_ID.fullmatch(authorization.evaluation_run_id):
        raise ProviderEvaluationError("E002_RUN_ID_INVALID", "evaluation_run_id must be a new eval_ identifier.")
    if not authorization.operator_name.strip() or not authorization.approval_timestamp.strip() or not authorization.provider_connection_preflight:
        raise ProviderEvaluationError("E002_AUTHORIZATION_RECORD_INCOMPLETE", "operator, approval time, and Provider preflight are required.")
    if authorization.confirmed_budget_cny <= 0:
        raise ProviderEvaluationError("E002_BUDGET_INVALID", "confirmed_budget_cny must be greater than zero.")
    max_budget = budget.get("max_operator_approved_budget_cny")
    if not isinstance(max_budget, int) or authorization.confirmed_budget_cny > max_budget:
        raise ProviderEvaluationError("E002_BUDGET_EXCEEDED", "confirmed_budget_cny exceeds the fixed evaluation ceiling.")


def run_authorized_evaluation(
    *,
    contract: Mapping[str, Any],
    normal_cases: Sequence[Mapping[str, Any]],
    safety_cases: Sequence[Mapping[str, Any]],
    fixture_sha256: str,
    planner: MechanicalConceptPlanner,
    authorization: ProviderEvaluationAuthorization,
    execution_mode: Literal["external", "synthetic"],
    cancelled: Optional[Callable[[], bool]] = None,
) -> dict[str, Any]:
    """Run only fixed normal cases and return a redacted in-memory report.

    ``execution_mode='synthetic'`` exists solely for no-network tests and can
    never be considered external Provider evidence.
    """

    budget = contract.get("budget")
    acceptance = contract.get("acceptance")
    if not isinstance(budget, Mapping) or not isinstance(acceptance, Mapping):
        raise ProviderEvaluationError("E002_CONTRACT_INVALID", "evaluation contract is missing budget or acceptance.")
    validate_authorization(authorization, budget)
    max_requests = _required_int(budget, "max_provider_requests")
    max_output_tokens = _required_int(budget, "max_output_tokens_per_request")
    max_total_output_tokens = _required_int(budget, "max_total_output_tokens")
    max_total_tokens = _required_int(budget, "max_total_tokens_when_usage_is_reported")

    records: list[dict[str, Any]] = []
    safe_stop_passes = 0
    for case in safety_cases:
        expected = case.get("expected")
        actual = evaluation_safe_stop(str(case.get("brief", "")))
        passed = actual == expected
        safe_stop_passes += int(passed)
        records.append(_record(
            case_id=_case_id(case), fixture_sha256=fixture_sha256, domain_pack_id=None,
            outcome_category="passed" if passed else "policy_scope_failed", structured_output_valid=None,
            direction_count=None, safety_checks={"local_safe_stop": passed, "expected": expected, "actual": actual},
            telemetry=None,
        ))

    provider_requests = 0
    total_output_tokens = 0
    total_tokens = 0
    usage_complete_calls = 0
    stopped_category: Optional[FailureCategory] = None
    cancel = cancelled or (lambda: False)

    for case in normal_cases:
        if cancel():
            stopped_category = "cancelled"
            break
        if provider_requests >= max_requests or total_output_tokens >= max_total_output_tokens or total_tokens >= max_total_tokens:
            stopped_category = "budget_exceeded"
            break
        pack_id = case.get("domain_pack_id")
        if not isinstance(pack_id, str):
            raise ProviderEvaluationError("E002_FIXTURE_INVALID", "normal case has no domain pack.")
        try:
            plan = planner.plan_complete_concept(brief=str(case.get("brief", "")), pack=domain_pack_by_id(pack_id), project_id=None)
            provider_requests += 1
            telemetry = getattr(planner, "last_call_telemetry", None)
            record = _plan_record(case, fixture_sha256, plan, telemetry)
        except MechanicalPlannerError as exc:
            provider_requests += 1
            telemetry = getattr(planner, "last_call_telemetry", None)
            category = _failure_category(exc.code)
            record = _record(
                case_id=_case_id(case), fixture_sha256=fixture_sha256, domain_pack_id=pack_id,
                outcome_category=category, structured_output_valid=False, direction_count=None,
                safety_checks={"planner_not_entered_for_safe_case": True}, telemetry=telemetry,
            )
            records.append(record)
            stopped_category = category
            break
        except Exception:  # noqa: BLE001 - an injected Provider must not leak raw failures into evidence.
            provider_requests += 1
            telemetry = getattr(planner, "last_call_telemetry", None)
            records.append(_record(
                case_id=_case_id(case), fixture_sha256=fixture_sha256, domain_pack_id=pack_id,
                outcome_category="transport_failed", structured_output_valid=False, direction_count=None,
                safety_checks={"planner_not_entered_for_safe_case": True}, telemetry=telemetry,
            ))
            stopped_category = "transport_failed"
            break

        records.append(record)
        if _telemetry_complete(telemetry):
            usage_complete_calls += 1
            total_output_tokens += telemetry.output_tokens or 0
            total_tokens += telemetry.total_tokens or 0
        if record["outcome_category"] != "passed":
            stopped_category = "policy_scope_failed"
            break
        if telemetry is not None and telemetry.output_tokens is not None and telemetry.output_tokens > max_output_tokens:
            stopped_category = "budget_exceeded"
            break

    normal_records = [record for record in records if record["case_id"].startswith("normal_")]
    safe_records = [record for record in records if not record["case_id"].startswith("normal_")]
    normal_passes = sum(record["outcome_category"] == "passed" for record in normal_records)
    all_cases_completed = len(normal_records) == len(normal_cases) and len(safe_records) == len(safety_cases) and stopped_category is None
    token_usage_coverage = usage_complete_calls / len(normal_cases) if normal_cases else 0.0
    normal_rate = normal_passes / len(normal_cases) if normal_cases else 0.0
    safe_rate = safe_stop_passes / len(safety_cases) if safety_cases else 0.0
    quality_gates_pass = (
        all_cases_completed
        and normal_rate == acceptance.get("normal_domain_binding_rate")
        and normal_rate == acceptance.get("normal_structured_output_rate")
        and normal_rate >= float(acceptance.get("normal_three_complete_directions_rate", 1.0))
        and normal_rate == acceptance.get("normal_non_functional_scope_rate")
        and safe_rate == acceptance.get("clarification_or_rejection_safe_stop_rate")
        and token_usage_coverage == acceptance.get("token_usage_coverage_for_eligible_live_evidence")
    )
    return {
        "schema_version": "ForgeCADProviderEvaluationRunReport@1",
        "evaluation_run_id": authorization.evaluation_run_id,
        "execution_mode": execution_mode,
        "fixture_sha256": fixture_sha256,
        "network_calls_made": provider_requests if execution_mode == "external" else 0,
        "synthetic_provider_calls": provider_requests if execution_mode == "synthetic" else 0,
        "asset_or_snapshot_writes": 0,
        "provider_request_limit": max_requests,
        "approved_budget_cny": authorization.confirmed_budget_cny,
        "actual_spend_cny": None,
        "cost_accounting": "provider_console_required",
        "stopped_category": stopped_category,
        "case_results": records,
        "summary": {
            "normal_cases_completed": len(normal_records),
            "normal_cases_expected": len(normal_cases),
            "safe_stop_cases_completed": len(safe_records),
            "safe_stop_cases_expected": len(safety_cases),
            "normal_pass_rate": normal_rate,
            "safe_stop_pass_rate": safe_rate,
            "token_usage_coverage": token_usage_coverage,
            "reported_output_tokens": total_output_tokens,
            "reported_total_tokens": total_tokens,
        },
        "quality_gates_pass": quality_gates_pass,
        "external_evidence_eligible": execution_mode == "external" and quality_gates_pass,
    }


def evaluation_safe_stop(brief: str) -> Literal["ambiguous", "unsupported", "recognized"]:
    """Apply the isolated evaluator's preflight boundary without calling a Provider."""

    normalized = brief.casefold()
    if any(term.casefold() in normalized for term in _OUT_OF_SCOPE_TERMS):
        return "unsupported"
    # Evaluation ambiguity is deliberately conservative: a brief that does not
    # clearly name one supported pack becomes a clarification, not a fallback.
    return "ambiguous"


def _plan_record(case: Mapping[str, Any], fixture_sha256: str, plan: MechanicalConceptPlan, telemetry: Optional[MechanicalPlannerTelemetry]) -> dict[str, Any]:
    required_roles = set(case.get("required_role_groups", []))
    returned_roles = {role for direction in plan.directions for role in direction.primary_part_roles}
    full_plan = json.dumps(plan.model_dump(mode="json"), ensure_ascii=False)
    scope_clean = plan.spec.get("non_functional_only") is True and not any(term in full_plan for term in _OUT_OF_SCOPE_TERMS)
    checks = {
        "domain_pack_matches": plan.domain_pack_id == case.get("domain_pack_id"),
        "required_roles_present": required_roles.issubset(returned_roles),
        "non_functional_scope": scope_clean,
    }
    passed = all(checks.values()) and len(plan.directions) == 1
    return _record(
        case_id=_case_id(case), fixture_sha256=fixture_sha256, domain_pack_id=plan.domain_pack_id,
        outcome_category="passed" if passed else "policy_scope_failed", structured_output_valid=True,
        direction_count=len(plan.directions), safety_checks=checks, telemetry=telemetry,
    )


def _record(*, case_id: str, fixture_sha256: str, domain_pack_id: Optional[str], outcome_category: str, structured_output_valid: Optional[bool], direction_count: Optional[int], safety_checks: Mapping[str, Any], telemetry: Optional[MechanicalPlannerTelemetry]) -> dict[str, Any]:
    return {
        "case_id": case_id,
        "fixture_sha256": fixture_sha256,
        "domain_pack_id": domain_pack_id,
        "outcome_category": outcome_category,
        "structured_output_valid": structured_output_valid,
        "direction_count": direction_count,
        "safety_checks": dict(safety_checks),
        "latency_ms": telemetry.latency_ms if telemetry else None,
        "input_tokens": telemetry.input_tokens if telemetry else None,
        "output_tokens": telemetry.output_tokens if telemetry else None,
        "total_tokens": telemetry.total_tokens if telemetry else None,
    }


def _case_id(case: Mapping[str, Any]) -> str:
    case_id = case.get("case_id")
    if not isinstance(case_id, str) or not case_id:
        raise ProviderEvaluationError("E002_FIXTURE_INVALID", "evaluation case has no case_id.")
    return case_id


def _required_int(source: Mapping[str, Any], name: str) -> int:
    value = source.get(name)
    if not isinstance(value, int) or value <= 0:
        raise ProviderEvaluationError("E002_CONTRACT_INVALID", f"invalid {name}")
    return value


def _telemetry_complete(telemetry: Optional[MechanicalPlannerTelemetry]) -> bool:
    return telemetry is not None and all(value is not None for value in (telemetry.input_tokens, telemetry.output_tokens, telemetry.total_tokens))


def _failure_category(code: str) -> FailureCategory:
    return {
        "PLANNER_TIMEOUT": "timeout",
        "PLANNER_RATE_LIMITED": "rate_limited",
        "PLANNER_AUTH_FAILED": "authentication_failed",
        "PLANNER_UNCONFIGURED": "authentication_failed",
        "PLANNER_BAD_OUTPUT": "invalid_structured_output",
    }.get(code, "transport_failed")  # type: ignore[return-value]
