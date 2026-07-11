#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional, Sequence

from forgecad_agent.application.concept_briefs import ConceptBriefService
from forgecad_agent.application.concept_change_sets import (
    ConceptChangeSetError,
    ConceptChangeSetService,
)
from forgecad_agent.application.concept_models import (
    GenerateDesignVariantsRequest,
    InterpretDesignBriefRequest,
    PlanDesignChangeSetRequest,
)
from forgecad_agent.application.concept_planner import (
    ConceptPlannerProvider,
    DeterministicConceptPlanner,
    OpenAICompatibleConceptPlanner,
    concept_planner_from_env,
)
from forgecad_agent.domain.concepts.models import ModuleGraph, WeaponConceptSpec


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TRUTH_SET = ROOT / "evaluations" / "r4" / "planner_truth_set.json"
DEFAULT_OUTPUT = ROOT / "output" / "evaluations" / "r4_planner_metrics.json"
EXPECTED_CASES_PER_SECTION = 20


def main() -> int:
    args = _parse_args()
    try:
        truth_set = _load_truth_set(args.truth_set)
        provider, generator, live_provider_run = _provider(args)
        report = evaluate(
            truth_set,
            provider=provider,
            generator=generator,
            live_provider_run=live_provider_run,
            max_cases=args.max_cases,
            truth_set_path=args.truth_set,
        )
    except EvaluationSetupError as exc:
        print(
            json.dumps(
                {"ok": False, "error": {"code": exc.code, "message": str(exc)}},
                ensure_ascii=False,
                indent=2,
            ),
            file=sys.stderr,
        )
        return 2

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
    display_report = (
        {key: value for key, value in report.items() if key != "results"}
        if args.summary_only
        else report
    )
    print(json.dumps(display_report, ensure_ascii=False, indent=2))
    if args.require_baseline_thresholds and not all(
        report["threshold_results"].values()
    ):
        return 4
    if args.require_thresholds and not report["real_provider_evidence_eligible"]:
        return 3
    return 0


class EvaluationSetupError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


def evaluate(
    truth_set: dict[str, Any],
    *,
    provider: ConceptPlannerProvider,
    generator: str,
    live_provider_run: bool,
    max_cases: Optional[int],
    truth_set_path: Path,
) -> dict[str, Any]:
    spec, graph, module_catalog = _base_context()
    brief_service = ConceptBriefService(None, provider)  # type: ignore[arg-type]
    change_service = ConceptChangeSetService(None, provider)  # type: ignore[arg-type]
    call_records: list[dict[str, Any]] = []
    brief_results: list[dict[str, Any]] = []
    variant_results: list[dict[str, Any]] = []
    change_results: list[dict[str, Any]] = []
    lock_results: list[dict[str, Any]] = []
    explainable_passes = 0
    explainable_total = 0

    brief_cases = _limit(truth_set["brief_cases"], max_cases)
    for case in brief_cases:
        interpreted: Optional[WeaponConceptSpec] = None
        started_at = time.perf_counter()
        try:
            used_provider, interpreted, _, _, _ = brief_service._interpret_with_provider(  # noqa: SLF001
                request=InterpretDesignBriefRequest(
                    client_request_id=f"eval-{case['id']}",
                    source_text=case["text"],
                    generator=generator,
                ),
                current_spec=spec,
                module_catalog=module_catalog,
            )
            passed, failures = _check_expected_values(
                interpreted.model_dump(mode="json"), case["expect"]
            )
            brief_results.append(
                {"id": case["id"], "passed": passed, "failures": failures}
            )
            _record_call(
                call_records,
                used_provider,
                stage="brief",
                case_id=case["id"],
                started_at=started_at,
            )
        except Exception as exc:  # noqa: BLE001 - eval records failures.
            brief_results.append(
                {
                    "id": case["id"],
                    "passed": False,
                    "failures": [_error_label(exc)],
                }
            )
            _record_call(
                call_records,
                provider,
                stage="brief",
                case_id=case["id"],
                started_at=started_at,
            )

        if interpreted is None:
            variant_results.append(
                {
                    "id": case["id"],
                    "passed": False,
                    "failures": ["brief_failed_before_variant_evaluation"],
                }
            )
            continue
        started_at = time.perf_counter()
        try:
            used_provider, plans, _, _, _ = brief_service._variants_with_provider(  # noqa: SLF001
                request=GenerateDesignVariantsRequest(
                    client_request_id=f"eval-variants-{case['id']}",
                    brief_id=f"brief_{case['id']}",
                    generator=generator,
                ),
                source_text=case["text"],
                interpreted_spec=interpreted,
                base_graph=graph,
                module_catalog=module_catalog,
            )
            signatures = {
                (plan.target_node_id, tuple(plan.scale)) for plan in plans
            }
            passed = len(plans) == 3 and len(signatures) == 3
            explainable_total += 1
            if all(plan.rationale for plan in plans):
                explainable_passes += 1
            variant_results.append(
                {
                    "id": case["id"],
                    "passed": passed,
                    "signatures": len(signatures),
                    "failures": [] if passed else ["variants_not_structurally_distinct"],
                }
            )
            _record_call(
                call_records,
                used_provider,
                stage="variants",
                case_id=case["id"],
                started_at=started_at,
            )
        except Exception as exc:  # noqa: BLE001
            variant_results.append(
                {"id": case["id"], "passed": False, "failures": [_error_label(exc)]}
            )
            _record_call(
                call_records,
                provider,
                stage="variants",
                case_id=case["id"],
                started_at=started_at,
            )

    change_cases = _limit(truth_set["change_cases"], max_cases)
    for case in change_cases:
        started_at = time.perf_counter()
        try:
            used_provider, plan, _, _, _ = change_service._change_plan_with_provider(  # noqa: SLF001
                request=PlanDesignChangeSetRequest(
                    client_request_id=f"eval-{case['id']}",
                    instruction=case["instruction"],
                    generator=generator,
                    selected_node_id=case.get("selected_node_id"),
                    selected_module_id=case.get("selected_module_id"),
                ),
                current_spec=spec,
                base_graph=graph,
                module_catalog=module_catalog,
            )
            passed, failures = _check_change_plan(plan, case)
            explainable_total += 1
            if plan.rationale and all(item.rationale for item in plan.operations):
                explainable_passes += 1
            change_results.append(
                {
                    "id": case["id"],
                    "passed": passed,
                    "operation_count": len(plan.operations),
                    "failures": failures,
                }
            )
            _record_call(
                call_records,
                used_provider,
                stage="change",
                case_id=case["id"],
                started_at=started_at,
            )
        except Exception as exc:  # noqa: BLE001
            change_results.append(
                {"id": case["id"], "passed": False, "failures": [_error_label(exc)]}
            )
            _record_call(
                call_records,
                provider,
                stage="change",
                case_id=case["id"],
                started_at=started_at,
            )

    lock_cases = _limit(truth_set["lock_probes"], max_cases)
    for case in lock_cases:
        started_at = time.perf_counter()
        try:
            used_provider, plan, _, _, _ = change_service._change_plan_with_provider(  # noqa: SLF001
                request=PlanDesignChangeSetRequest(
                    client_request_id=f"eval-{case['id']}",
                    instruction=case["instruction"],
                    generator=generator,
                    selected_node_id=case.get("selected_node_id"),
                    selected_module_id=case.get("selected_module_id"),
                ),
                current_spec=spec,
                base_graph=graph,
                module_catalog=module_catalog,
            )
            unsafe = [
                item
                for item in plan.operations
                if item.op in {"replace_module", "set_mirror"}
                and item.node_id == graph.root_node_id
            ]
            lock_results.append(
                {
                    "id": case["id"],
                    "passed": not unsafe,
                    "outcome": "safe_plan" if not unsafe else "unsafe_plan",
                }
            )
            _record_call(
                call_records,
                used_provider,
                stage="lock_probe",
                case_id=case["id"],
                started_at=started_at,
            )
        except ConceptChangeSetError as exc:
            policy_rejection = exc.code in {"PLANNER_BAD_OUTPUT", "PLANNER_NO_ACTION"}
            lock_results.append(
                {
                    "id": case["id"],
                    "passed": policy_rejection,
                    "outcome": "policy_rejected" if policy_rejection else _error_label(exc),
                }
            )
            _record_call(
                call_records,
                provider,
                stage="lock_probe",
                case_id=case["id"],
                started_at=started_at,
            )
        except Exception as exc:  # noqa: BLE001
            lock_results.append(
                {"id": case["id"], "passed": False, "outcome": _error_label(exc)}
            )
            _record_call(
                call_records,
                provider,
                stage="lock_probe",
                case_id=case["id"],
                started_at=started_at,
            )

    metrics = {
        "brief_success_rate": _pass_rate(brief_results),
        "variant_distinct_rate": _pass_rate(variant_results),
        "change_success_rate": _pass_rate(change_results),
        "lock_preservation_rate": _pass_rate(lock_results),
        "explainability_rate": (
            round(explainable_passes / explainable_total, 4)
            if explainable_total
            else 0.0
        ),
    }
    thresholds = truth_set["thresholds"]
    threshold_results = {
        name: metrics[name] >= float(value) for name, value in thresholds.items()
    }
    full_truth_set = (
        len(brief_cases) == EXPECTED_CASES_PER_SECTION
        and len(change_cases) == EXPECTED_CASES_PER_SECTION
        and len(lock_cases) == EXPECTED_CASES_PER_SECTION
    )
    expected_provider_calls = (
        len(brief_cases) * 2 + len(change_cases) + len(lock_cases)
    )
    telemetry = _telemetry_summary(call_records)
    token_coverage_complete = (
        telemetry["calls_with_token_usage"] == expected_provider_calls
    )
    real_provider_evidence_eligible = (
        live_provider_run
        and full_truth_set
        and all(threshold_results.values())
        and len(call_records) == expected_provider_calls
        and token_coverage_complete
    )
    return {
        "ok": True,
        "schema_version": "ForgeCADPlannerEvaluationReport@1",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "truth_set": {
            "name": truth_set["name"],
            "sha256": hashlib.sha256(truth_set_path.read_bytes()).hexdigest(),
            "brief_cases": len(brief_cases),
            "variant_cases": len(variant_results),
            "change_cases": len(change_cases),
            "lock_probes": len(lock_cases),
            "full_truth_set": full_truth_set,
        },
        "provider": {
            "mode": generator,
            "provider_id": provider.provider_id,
            "provider_type": provider.provider_type,
            "model": provider.model_name,
            "live_provider_run": live_provider_run,
        },
        "metrics": metrics,
        "thresholds": thresholds,
        "threshold_results": threshold_results,
        "telemetry": telemetry,
        "real_provider_evidence_eligible": real_provider_evidence_eligible,
        "limitations": (
            []
            if real_provider_evidence_eligible
            else [
                "This report is not eligible as real-provider release evidence.",
                "A complete configured-provider run with token coverage and all thresholds is required.",
            ]
        ),
        "results": {
            "briefs": brief_results,
            "variants": variant_results,
            "changes": change_results,
            "lock_probes": lock_results,
        },
    }


def _provider(
    args: argparse.Namespace,
) -> tuple[ConceptPlannerProvider, str, bool]:
    if args.provider == "deterministic_rules":
        return DeterministicConceptPlanner(), "deterministic_rules", False
    if not args.confirm_live_provider:
        raise EvaluationSetupError(
            "EVAL_LIVE_CONFIRMATION_REQUIRED",
            "Configured-provider evaluation requires --confirm-live-provider because it may consume paid API tokens.",
        )
    provider = concept_planner_from_env()
    if not isinstance(provider, OpenAICompatibleConceptPlanner):
        raise EvaluationSetupError(
            "EVAL_PROVIDER_NOT_CONFIGURED",
            "Set FORGECAD_CONCEPT_PLANNER_PROVIDER=openai_compatible before a live evaluation.",
        )
    if not provider.config.model or not provider.config.api_key:
        raise EvaluationSetupError(
            "EVAL_PROVIDER_NOT_CONFIGURED",
            "Live evaluation requires model and API key or API key file configuration.",
        )
    return provider, "configured_provider", True


def _load_truth_set(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise EvaluationSetupError("EVAL_TRUTH_SET_INVALID", str(exc)) from exc
    if payload.get("schema_version") != "ForgeCADPlannerTruthSet@1":
        raise EvaluationSetupError(
            "EVAL_TRUTH_SET_INVALID", "Unsupported planner truth-set schema version."
        )
    for section in ("brief_cases", "change_cases", "lock_probes"):
        values = payload.get(section)
        if not isinstance(values, list) or len(values) < EXPECTED_CASES_PER_SECTION:
            raise EvaluationSetupError(
                "EVAL_TRUTH_SET_INVALID",
                f"{section} must contain at least {EXPECTED_CASES_PER_SECTION} cases.",
            )
        ids = [item.get("id") for item in values if isinstance(item, dict)]
        if len(ids) != len(values) or len(set(ids)) != len(ids):
            raise EvaluationSetupError(
                "EVAL_TRUTH_SET_INVALID", f"{section} case ids must be unique."
            )
    return payload


def _base_context() -> tuple[
    WeaponConceptSpec, ModuleGraph, list[dict[str, str]]
]:
    spec = WeaponConceptSpec.model_validate(
        {
            "schema_version": "WeaponConceptSpec@1",
            "project_id": "prj_eval_arctic",
            "profile_id": "profile_weapon_concept_v1",
            "name": "R4 Planner Evaluation",
            "archetype": "future_modular_sidearm",
            "intended_uses": ["game_asset", "film_prop", "non_functional_display"],
            "style": {
                "keywords": ["寒地", "工业", "模块化"],
                "palette": ["graphite", "gunmetal", "signal_red"],
                "detail_density": 0.68,
            },
            "proportions": {
                "overall_length_mm": 230,
                "body_height_mm": 54,
                "grip_angle_deg": 15,
            },
            "required_slots": ["core", "front", "grip"],
            "optional_slots": ["top", "side_panels"],
            "constraints": {
                "symmetry": "mostly_symmetric",
                "max_triangle_count": 180000,
            },
            "assumptions": ["非功能性概念模型，不用于真实制造或使用"],
        }
    )
    transform = {"position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1]}
    graph = ModuleGraph.model_validate(
        {
            "schema_version": "ModuleGraph@1",
            "graph_id": "mg_eval_arctic",
            "project_id": spec.project_id,
            "root_node_id": "node_core",
            "nodes": [
                {"node_id": "node_core", "module_id": "module_core_shell_01", "transform": transform, "locked": True, "visible": True},
                {"node_id": "node_front", "module_id": "module_front_shell_01", "transform": transform, "locked": False, "visible": True},
                {"node_id": "node_grip", "module_id": "module_grip_shell_01", "transform": transform, "locked": False, "visible": True}
            ],
            "edges": [
                {"edge_id": "edge_eval_front", "from_node_id": "node_core", "from_connector_id": "connector_eval_core_front", "to_node_id": "node_front", "to_connector_id": "connector_eval_front_core", "status": "connected"},
                {"edge_id": "edge_eval_grip", "from_node_id": "node_core", "from_connector_id": "connector_eval_core_grip", "to_node_id": "node_grip", "to_connector_id": "connector_eval_grip_core", "status": "connected"}
            ]
        }
    )
    module_catalog = [
        {"module_id": "module_core_shell_01", "category": "core_shell"},
        {"module_id": "module_front_shell_01", "category": "front_shell"},
        {"module_id": "module_front_shell_02", "category": "front_shell"},
        {"module_id": "module_grip_shell_01", "category": "grip_shell"},
        {"module_id": "module_grip_shell_02", "category": "grip_shell"},
    ]
    return spec, graph, module_catalog


def _check_expected_values(
    payload: dict[str, Any], expectations: dict[str, dict[str, Any]]
) -> tuple[bool, list[str]]:
    failures: list[str] = []
    for path, rule in expectations.items():
        actual = _value_at_path(payload, path)
        operator = rule["op"]
        expected = rule["value"]
        if operator == "eq":
            passed = actual == expected
        elif operator == "near":
            passed = _numbers_near(actual, expected, rule.get("tolerance", 0.001))
        elif operator == "lte":
            passed = _is_number(actual) and float(actual) <= float(expected)
        elif operator == "gte":
            passed = _is_number(actual) and float(actual) >= float(expected)
        else:
            passed = False
        if not passed:
            failures.append(f"{path}:{actual!r} does not satisfy {operator} {expected!r}")
    return not failures, failures


def _check_change_plan(plan: Any, case: dict[str, Any]) -> tuple[bool, list[str]]:
    failures: list[str] = []
    operations = plan.operations
    path_operations = {item.path: item for item in operations if item.path}
    for path, expected in case.get("expect", {}).items():
        operation = path_operations.get(path)
        if operation is None or not _numbers_near(operation.value, expected, 0.01):
            actual = operation.value if operation is not None else None
            failures.append(f"{path}:{actual!r} != {expected!r}")
    for path in case.get("expect_paths", []):
        if path not in path_operations:
            failures.append(f"missing path operation: {path}")
    expected_replace = case.get("expect_replace")
    if expected_replace and not any(
        item.op == "replace_module"
        and item.node_id == expected_replace["node_id"]
        and item.module_id == expected_replace["module_id"]
        for item in operations
    ):
        failures.append("missing expected registry replacement")
    expected_mirror = case.get("expect_mirror")
    if expected_mirror and not any(
        item.op == "set_mirror"
        and item.node_id == expected_mirror["node_id"]
        and item.mirror_axis == expected_mirror["axis"]
        for item in operations
    ):
        failures.append("missing expected mirror operation")
    return not failures, failures


def _record_call(
    records: list[dict[str, Any]],
    provider: ConceptPlannerProvider,
    *,
    stage: str,
    case_id: str,
    started_at: float,
) -> None:
    metrics = getattr(provider, "last_call_metrics", None)
    records.append(
        {
            "stage": stage,
            "case_id": case_id,
            "wall_latency_ms": max(0, round((time.perf_counter() - started_at) * 1000)),
            "provider_latency_ms": metrics.latency_ms if metrics is not None else None,
            "input_tokens": metrics.input_tokens if metrics is not None else None,
            "output_tokens": metrics.output_tokens if metrics is not None else None,
            "total_tokens": metrics.total_tokens if metrics is not None else None,
        }
    )


def _telemetry_summary(records: list[dict[str, Any]]) -> dict[str, Any]:
    provider_latencies = [
        int(item["provider_latency_ms"])
        for item in records
        if item["provider_latency_ms"] is not None
    ]
    wall_latencies = [int(item["wall_latency_ms"]) for item in records]
    token_records = [item for item in records if item["total_tokens"] is not None]
    return {
        "provider_call_records": len(records),
        "calls_with_provider_latency": len(provider_latencies),
        "calls_with_token_usage": len(token_records),
        "provider_latency_ms": _distribution(provider_latencies),
        "wall_latency_ms": _distribution(wall_latencies),
        "input_tokens": sum(int(item["input_tokens"] or 0) for item in token_records),
        "output_tokens": sum(int(item["output_tokens"] or 0) for item in token_records),
        "total_tokens": sum(int(item["total_tokens"] or 0) for item in token_records),
    }


def _distribution(values: Sequence[int]) -> dict[str, Optional[int]]:
    if not values:
        return {"count": 0, "p50": None, "p95": None, "max": None}
    ordered = sorted(values)
    return {
        "count": len(ordered),
        "p50": _percentile(ordered, 0.5),
        "p95": _percentile(ordered, 0.95),
        "max": ordered[-1],
    }


def _percentile(values: Sequence[int], percentile: float) -> int:
    index = max(0, math.ceil(len(values) * percentile) - 1)
    return int(values[index])


def _value_at_path(payload: dict[str, Any], path: str) -> Any:
    value: Any = payload
    for segment in path.split("."):
        if not isinstance(value, dict) or segment not in value:
            return None
        value = value[segment]
    return value


def _numbers_near(actual: Any, expected: Any, tolerance: float) -> bool:
    return (
        _is_number(actual)
        and _is_number(expected)
        and abs(float(actual) - float(expected)) <= float(tolerance)
    )


def _is_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _pass_rate(results: Sequence[dict[str, Any]]) -> float:
    return (
        round(sum(bool(item.get("passed")) for item in results) / len(results), 4)
        if results
        else 0.0
    )


def _error_label(exc: Exception) -> str:
    code = getattr(exc, "code", type(exc).__name__)
    return f"{code}: {exc}"


def _limit(values: list[Any], max_cases: Optional[int]) -> list[Any]:
    return values if max_cases is None else values[:max_cases]


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Evaluate ForgeCAD R4 planners against the fixed visual truth set."
    )
    parser.add_argument(
        "--provider",
        choices=("deterministic_rules", "configured_provider"),
        default="deterministic_rules",
    )
    parser.add_argument(
        "--confirm-live-provider",
        action="store_true",
        help="Explicitly authorize calls that may consume paid provider tokens.",
    )
    parser.add_argument("--require-thresholds", action="store_true")
    parser.add_argument("--require-baseline-thresholds", action="store_true")
    parser.add_argument("--summary-only", action="store_true")
    parser.add_argument("--max-cases", type=int, default=None)
    parser.add_argument("--truth-set", type=Path, default=DEFAULT_TRUTH_SET)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


if __name__ == "__main__":
    raise SystemExit(main())
