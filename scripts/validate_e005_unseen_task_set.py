#!/usr/bin/env python3
"""Validate the frozen E005 contract without generating or scoring assets."""

from __future__ import annotations

import copy
import hashlib
import itertools
import json
import math
import statistics
from collections import Counter
from datetime import datetime
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "concept-spec" / "schemas"
FIXTURE = ROOT / "packages" / "concept-spec" / "fixtures" / "e005-unseen-mechanical-hard-surface-task-set.json"
PROVIDER_AUTHORIZATION_FIXTURE = (
    ROOT
    / "packages"
    / "concept-spec"
    / "fixtures"
    / "e005-provider-run-authorization-not-authorized.json"
)
EXPECTED_TASK_SET_SHA256 = "471c592b5f328f6e899b430b49eb042d3c6955f498b14fd1d2558a0934e18dde"
EXPECTED_FAMILIES = {
    "enclosure_chassis",
    "articulated_tool",
    "mobility_module",
    "aerial_mechanical",
    "industrial_machine",
    "fictional_prop",
}
BANNED_LEAK_TOKENS = {
    "c111",
    "bracket",
    "rotor",
    "duct",
    "forge-visual-geometry-v2",
    "robotic_arm_iteration",
}
E005_FAILURE_CODES = {
    "E005_SOURCE_UNAVAILABLE",
    "E005_PROVIDER_UNAUTHORIZED",
    "E005_AUTHORING_FAILED",
    "E005_SCHEMA_INVALID",
    "E005_EXPANSION_FAILED",
    "E005_LOWERING_FAILED",
    "E005_COMPILE_FAILED",
    "E005_READBACK_FAILED",
    "E005_RENDER_FAILED",
    "E005_HARD_GATE_FAILED",
    "E005_PATCH_FAILED",
    "E005_CANCELLED",
    "E005_TIMEOUT",
    "E005_INTERNAL_ERROR",
}
NOT_RUN_FAILURE_CODES = {"E005_SOURCE_UNAVAILABLE", "E005_PROVIDER_UNAUTHORIZED"}
HUMAN_SCORE_DIMENSIONS = {
    "macro_shape",
    "meso_structure",
    "micro_detail",
    "identity_alignment",
    "pbr_material",
    "hidden_surface_reasonableness",
    "artifact_free",
}
STRUCTURAL_DIFFERENCE_AXES = (
    "topology",
    "operation_sequence",
    "profile",
    "part_zone",
    "bounds",
    "glb",
)


def canonical_sha256(value: object) -> str:
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def schema_registry() -> tuple[dict[str, object], Registry]:
    schemas = {
        path.name: json.loads(path.read_text(encoding="utf-8"))
        for path in sorted(SCHEMA_ROOT.glob("*.json"))
    }
    registry = Registry().with_resources(
        (schema["$id"], Resource.from_contents(schema)) for schema in schemas.values()
    )
    return schemas, registry


def validate_schema(
    schemas: dict[str, object], registry: Registry, schema_name: str, payload: dict[str, object]
) -> None:
    validator = Draft202012Validator(
        schemas[schema_name], registry=registry, format_checker=FormatChecker()
    )
    errors = sorted(validator.iter_errors(payload), key=lambda item: list(item.path))
    if errors:
        raise ValueError(f"E005_SCHEMA_INVALID:{schema_name}:{errors[0].message}")


def validate_task_set(payload: dict[str, object]) -> dict[str, object]:
    schemas, registry = schema_registry()
    validate_schema(schemas, registry, "e005-unseen-task-set-v1.schema.json", payload)
    tasks = payload["tasks"]
    if not isinstance(tasks, list):
        raise ValueError("E005_TASKS_INVALID")
    ids = [task["task_id"] for task in tasks]
    if len(ids) != len(set(ids)):
        raise ValueError("E005_TASK_ID_DUPLICATE")
    families = Counter(task["morphology_family"] for task in tasks)
    if set(families) != EXPECTED_FAMILIES or any(count != 5 for count in families.values()):
        raise ValueError("E005_FAMILY_DISTRIBUTION_INVALID")
    normalized_prompts = [" ".join(task["prompt"].split()).casefold() for task in tasks]
    if len(normalized_prompts) != len(set(normalized_prompts)):
        raise ValueError("E005_PROMPT_DUPLICATE")
    image_description_count = 0
    operation_coverage: set[str] = set()
    for task in tasks:
        combined = json.dumps(task, ensure_ascii=False).casefold()
        if any(token in combined for token in BANNED_LEAK_TOKENS):
            raise ValueError(f"E005_FIXTURE_LEAK:{task['task_id']}")
        has_description = task["image_description"] is not None
        if has_description and not task["image_description"].strip():
            raise ValueError(f"E005_IMAGE_DESCRIPTION_EMPTY:{task['task_id']}")
        if (task["input_mode"] == "text_plus_image_description") != has_description:
            raise ValueError(f"E005_INPUT_MODE_MISMATCH:{task['task_id']}")
        image_description_count += int(has_description)
        operation_coverage.update(task["allowed_operation_families"])
    if image_description_count < 6:
        raise ValueError("E005_IMAGE_DESCRIPTION_COVERAGE_INVALID")
    if operation_coverage != {"box", "extrude", "revolve", "loft", "sweep", "boolean", "mirror", "array"}:
        raise ValueError("E005_OPERATION_COVERAGE_INVALID")
    task_set_sha256 = canonical_sha256(payload)
    if task_set_sha256 != EXPECTED_TASK_SET_SHA256:
        raise ValueError("E005_FROZEN_TASK_SET_HASH_MISMATCH")
    return {
        "status": "contract_pass",
        "task_count": len(tasks),
        "family_counts": dict(sorted(families.items())),
        "image_description_count": image_description_count,
        "operation_coverage": sorted(operation_coverage),
        "task_set_sha256": task_set_sha256,
        "generation_runs": 0,
        "human_reviews": 0,
        "formal_eligible": False,
    }


def validate_run_receipt(
    payload: dict[str, object], *, task_set_sha256: str, tasks_by_id: dict[str, dict[str, object]]
) -> None:
    schemas, registry = schema_registry()
    validate_schema(schemas, registry, "e005-run-receipt-v1.schema.json", payload)
    if payload["task_set_sha256"] != task_set_sha256:
        raise ValueError("E005_RECEIPT_TASK_SET_HASH_MISMATCH")
    if payload["task_id"] not in tasks_by_id:
        raise ValueError("E005_RECEIPT_TASK_UNKNOWN")
    if payload["task_payload_sha256"] != canonical_sha256(tasks_by_id[payload["task_id"]]):
        raise ValueError("E005_RECEIPT_TASK_PAYLOAD_HASH_MISMATCH")
    failure_codes = set(payload["failure_codes"])
    if not failure_codes <= E005_FAILURE_CODES:
        raise ValueError("E005_RECEIPT_FAILURE_CODE_UNKNOWN")
    if payload["status"] == "not_run" and not failure_codes <= NOT_RUN_FAILURE_CODES:
        raise ValueError("E005_NOT_RUN_FAILURE_CODE_INVALID")
    if payload["status"] == "cancelled" and not failure_codes & {"E005_CANCELLED", "E005_TIMEOUT"}:
        raise ValueError("E005_CANCELLED_FAILURE_CODE_MISSING")
    if payload["run_mode"] == "formal_provider":
        evidence = payload["provider_call_evidence"]
        if payload["provider_call_evidence_sha256"] != canonical_sha256(evidence):
            raise ValueError("E005_RECEIPT_PROVIDER_EVIDENCE_HASH_MISMATCH")
        if len(evidence) != payload["network_provider_calls"] or len(evidence) != (
            payload["authoring_count"] + payload["patch_count"]
        ):
            raise ValueError("E005_RECEIPT_PROVIDER_EVIDENCE_COUNT_MISMATCH")
        if [item["call_kind"] for item in evidence] != (
            ["author"] if payload["patch_count"] == 0 else ["author", "patch"]
        ):
            raise ValueError("E005_RECEIPT_PROVIDER_EVIDENCE_ORDER_MISMATCH")
        if len({item["reservation_id"] for item in evidence}) != len(evidence):
            raise ValueError("E005_RECEIPT_PROVIDER_RESERVATION_DUPLICATE")
        if any(
            item["authorization_id"] != payload["provider_authorization_id"]
            or item["task_id"] != payload["task_id"]
            or item["task_payload_sha256"] != payload["task_payload_sha256"]
            or item["settlement"] != "accounted"
            or item["network_call_made"] is not True
            for item in evidence
        ):
            raise ValueError("E005_RECEIPT_PROVIDER_EVIDENCE_LINEAGE_MISMATCH")
        if payload["status"] == "passed_without_patch" and (
            evidence[0]["outcome_code"] != "PROVIDER_COMPLETED_PASSED"
            or evidence[0]["output_source_sha256"] != payload["source_program_sha256"]
            or evidence[0]["output_gate_sha256"] != payload["gate_outcome_sha256"]
        ):
            raise ValueError("E005_RECEIPT_PROVIDER_FIRST_PASS_EVIDENCE_MISMATCH")
        if payload["status"] == "passed_after_patch" and (
            evidence[0]["outcome_code"] != "PROVIDER_COMPLETED_REPAIRABLE"
            or evidence[1]["outcome_code"] != "PROVIDER_COMPLETED_PASSED"
            or evidence[1]["output_source_sha256"] != payload["source_program_sha256"]
            or evidence[1]["output_gate_sha256"] != payload["gate_outcome_sha256"]
        ):
            raise ValueError("E005_RECEIPT_PROVIDER_PATCH_EVIDENCE_MISMATCH")
    if payload["status"] in {"passed_without_patch", "passed_after_patch"}:
        if payload["fixed_view_sha256"] != canonical_sha256(payload["fixed_views"]):
            raise ValueError("E005_RECEIPT_FIXED_VIEW_HASH_MISMATCH")
        expected_structural_descriptor = canonical_sha256(
            {
                "final_source_program_sha256": payload["source_program_sha256"],
                "shape_program_sha256": payload["shape_program_sha256"],
                "glb_sha256": payload["glb_sha256"],
                "semantic_structure_sha256": payload["semantic_structure_sha256"],
                "normalized_geometry_sha256": payload[
                    "normalized_geometry_sha256"
                ],
            }
        )
        if payload["structural_descriptor_sha256"] != expected_structural_descriptor:
            raise ValueError("E005_RECEIPT_STRUCTURAL_DESCRIPTOR_HASH_MISMATCH")
        usage = payload["usage"]
        if payload["network_provider_calls"] != usage["provider_requests"]:
            raise ValueError("E005_RECEIPT_PROVIDER_USAGE_MISMATCH")
        if payload["billable_cost_microusd"] != usage["estimated_cost_microusd"]:
            raise ValueError("E005_RECEIPT_COST_USAGE_MISMATCH")
        phases = payload["phase_receipts"]
        if [phase["sequence"] for phase in phases] != list(range(1, len(phases) + 1)):
            raise ValueError("E005_RECEIPT_PHASE_SEQUENCE_INVALID")
        if any(
            previous["output_sha256"] != current["input_sha256"]
            for previous, current in zip(phases, phases[1:])
        ):
            raise ValueError("E005_RECEIPT_PHASE_HASH_CHAIN_INVALID")
        names = [phase["phase"] for phase in phases]
        first_pass_names = [
            "author",
            "validate",
            "expand",
            "lower",
            "compile_readback",
            "render",
            "evaluate",
            "preview",
        ]
        patched_names = first_pass_names[:-1] + [
            "patch",
            "validate",
            "expand",
            "lower",
            "compile_readback",
            "render",
            "evaluate",
            "preview",
        ]
        expected_names = (
            first_pass_names
            if payload["status"] == "passed_without_patch"
            else patched_names
        )
        if names != expected_names:
            raise ValueError("E005_RECEIPT_PHASE_SET_INVALID")
        if phases[-1]["output_sha256"] != payload["source_program_sha256"]:
            raise ValueError("E005_RECEIPT_PREVIEW_SOURCE_HASH_MISMATCH")
        final = phases if payload["status"] == "passed_without_patch" else phases[-8:]
        expected_outputs = [
            payload["source_program_sha256"],
            payload["source_program_sha256"],
            payload["expanded_program_sha256"],
            payload["shape_program_sha256"],
            payload["glb_sha256"],
            payload["fixed_view_sha256"],
            payload["gate_outcome_sha256"],
            payload["source_program_sha256"],
        ]
        if [phase["output_sha256"] for phase in final] != expected_outputs:
            raise ValueError("E005_RECEIPT_FINAL_PHASE_LINEAGE_INVALID")


def validate_provider_run_authorization(
    payload: dict[str, object], *, task_set_sha256: str
) -> bool:
    schemas, registry = schema_registry()
    validate_schema(
        schemas,
        registry,
        "e005-provider-run-authorization-v1.schema.json",
        payload,
    )
    if payload["task_set_sha256"] != task_set_sha256:
        raise ValueError("E005_PROVIDER_AUTHORIZATION_TASK_SET_HASH_MISMATCH")
    binding_payload = copy.deepcopy(payload)
    binding_sha256 = binding_payload.pop("authorization_binding_sha256")
    if binding_sha256 != canonical_sha256(binding_payload):
        raise ValueError("E005_PROVIDER_AUTHORIZATION_BINDING_HASH_MISMATCH")
    if payload["status"] == "not_authorized":
        return False
    authorized_at = datetime.fromisoformat(
        str(payload["authorized_at"]).replace("Z", "+00:00")
    )
    expires_at = datetime.fromisoformat(str(payload["expires_at"]).replace("Z", "+00:00"))
    if authorized_at >= expires_at:
        raise ValueError("E005_PROVIDER_AUTHORIZATION_TIME_RANGE_INVALID")
    if payload["maximum_batch_wall_time_ms"] < payload["maximum_single_call_wall_time_ms"]:
        raise ValueError("E005_PROVIDER_AUTHORIZATION_TIME_BUDGET_INVALID")
    return True


def validate_human_review_bundle(
    payload: dict[str, object],
    *,
    task_set_sha256: str,
    receipts: list[dict[str, object]],
) -> dict[str, int]:
    schemas, registry = schema_registry()
    validate_schema(schemas, registry, "e005-human-review-bundle-v1.schema.json", payload)
    if payload["task_set_sha256"] != task_set_sha256:
        raise ValueError("E005_REVIEW_TASK_SET_HASH_MISMATCH")
    if payload["run_receipts_sha256"] != canonical_sha256(receipts):
        raise ValueError("E005_REVIEW_RUN_RECEIPTS_HASH_MISMATCH")
    reviews = payload["reviews"]
    if payload["review_count"] != len(reviews):
        raise ValueError("E005_REVIEW_COUNT_MISMATCH")
    if payload["reviews_sha256"] != canonical_sha256(reviews):
        raise ValueError("E005_REVIEW_SET_HASH_MISMATCH")
    if payload["status"] == "not_run":
        return {
            "human_review_complete_count": 0,
            "independent_reviewers_per_task_minimum": 0,
            "first_pass_human_quality_count": 0,
            "within_one_patch_human_quality_count": 0,
            "human_review_receipt_count": 0,
        }

    receipts_by_task = {receipt["task_id"]: receipt for receipt in receipts}
    if len(receipts) != 30 or len(receipts_by_task) != 30:
        raise ValueError("E005_REVIEW_FORMAL_RUN_COVERAGE_INVALID")
    commitments = payload["reviewer_commitments"]
    commitments_by_reviewer = {
        commitment["reviewer_id"]: commitment["identity_commitment_sha256"]
        for commitment in commitments
    }
    if len(commitments_by_reviewer) != 3 or len(set(commitments_by_reviewer.values())) != 3:
        raise ValueError("E005_REVIEWER_COMMITMENTS_NOT_INDEPENDENT")

    pairs: set[tuple[str, str]] = set()
    review_ids: set[str] = set()
    blind_packet_hashes: set[str] = set()
    task_scores: dict[str, list[int]] = {task_id: [] for task_id in receipts_by_task}
    reviewers_by_task: dict[str, set[str]] = {task_id: set() for task_id in receipts_by_task}
    tasks_by_reviewer: dict[str, set[str]] = {
        reviewer_id: set() for reviewer_id in commitments_by_reviewer
    }
    for review in reviews:
        reviewer_id = review["reviewer_id"]
        task_id = review["task_id"]
        if reviewer_id not in commitments_by_reviewer:
            raise ValueError("E005_REVIEWER_UNKNOWN")
        if review["reviewer_commitment_sha256"] != commitments_by_reviewer[reviewer_id]:
            raise ValueError("E005_REVIEWER_COMMITMENT_MISMATCH")
        if task_id not in receipts_by_task:
            raise ValueError("E005_REVIEW_TASK_UNKNOWN")
        pair = (reviewer_id, task_id)
        if pair in pairs or review["review_id"] in review_ids:
            raise ValueError("E005_REVIEW_DUPLICATE")
        if review["blind_packet_sha256"] in blind_packet_hashes:
            raise ValueError("E005_REVIEW_BLIND_PACKET_REUSED")
        pairs.add(pair)
        review_ids.add(review["review_id"])
        blind_packet_hashes.add(review["blind_packet_sha256"])

        receipt = receipts_by_task[task_id]
        if receipt["status"] not in {"passed_without_patch", "passed_after_patch"}:
            raise ValueError("E005_REVIEW_NON_SUCCESSFUL_RUN")
        if receipt["run_mode"] != "formal_provider" or receipt["distribution_eligible"] is not True:
            raise ValueError("E005_REVIEW_NON_FORMAL_RUN")
        if receipt["human_review_status"] != "pending":
            raise ValueError("E005_REVIEW_RUN_MUST_REMAIN_PENDING")
        if review["run_id"] != receipt["run_id"]:
            raise ValueError("E005_REVIEW_RUN_ID_MISMATCH")
        if review["run_receipt_sha256"] != canonical_sha256(receipt):
            raise ValueError("E005_REVIEW_RUN_RECEIPT_HASH_MISMATCH")
        if review["fixed_view_sha256"] != receipt["fixed_view_sha256"]:
            raise ValueError("E005_REVIEW_FIXED_VIEW_SET_HASH_MISMATCH")
        if review["fixed_views"] != receipt["fixed_views"]:
            raise ValueError("E005_REVIEW_FIXED_VIEW_HASH_MISMATCH")
        if set(review["view_order"]) != {"front", "iso", "side", "top"}:
            raise ValueError("E005_REVIEW_VIEW_COVERAGE_INVALID")
        expected_stage = (
            "first_pass" if receipt["status"] == "passed_without_patch" else "after_patch"
        )
        if review["result_stage"] != expected_stage:
            raise ValueError("E005_REVIEW_RESULT_STAGE_MISMATCH")
        scores = review["scores"]
        if set(scores) != HUMAN_SCORE_DIMENSIONS:
            raise ValueError("E005_REVIEW_SCORE_DIMENSIONS_INVALID")
        derived_overall = int(statistics.median(scores.values()))
        if review["overall_score"] != derived_overall:
            raise ValueError("E005_REVIEW_OVERALL_SCORE_MISMATCH")
        task_scores[task_id].append(derived_overall)
        reviewers_by_task[task_id].add(reviewer_id)
        tasks_by_reviewer[reviewer_id].add(task_id)

    expected_task_ids = set(receipts_by_task)
    if any(task_ids != expected_task_ids for task_ids in tasks_by_reviewer.values()):
        raise ValueError("E005_REVIEWER_TASK_COVERAGE_INVALID")
    reviewer_counts = [len(reviewers) for reviewers in reviewers_by_task.values()]
    if any(count != 3 for count in reviewer_counts):
        raise ValueError("E005_REVIEW_TASK_REVIEWER_COUNT_INVALID")
    quality_tasks = {
        task_id
        for task_id, values in task_scores.items()
        if len(values) == 3 and statistics.median(values) >= 4
    }
    first_pass_quality = sum(
        task_id in quality_tasks
        and receipts_by_task[task_id]["status"] == "passed_without_patch"
        for task_id in receipts_by_task
    )
    return {
        "human_review_complete_count": sum(
            len(values) == 3 for values in task_scores.values()
        ),
        "independent_reviewers_per_task_minimum": min(reviewer_counts, default=0),
        "first_pass_human_quality_count": first_pass_quality,
        "within_one_patch_human_quality_count": len(quality_tasks),
        "human_review_receipt_count": len(reviews),
    }


def derive_structural_entries_and_comparisons(
    *,
    tasks_by_id: dict[str, dict[str, object]],
    receipts: list[dict[str, object]],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    task_ids = list(tasks_by_id)
    receipts_by_task = {receipt["task_id"]: receipt for receipt in receipts}
    expected_entries = []
    for task_id in task_ids:
        receipt = receipts_by_task[task_id]
        expected_entries.append(
            {
                "task_id": task_id,
                "run_id": receipt["run_id"],
                "run_receipt_sha256": canonical_sha256(receipt),
                "structural_descriptor_sha256": receipt[
                    "structural_descriptor_sha256"
                ],
                "semantic_structure_sha256": receipt[
                    "semantic_structure_sha256"
                ],
                "normalized_geometry_sha256": receipt[
                    "normalized_geometry_sha256"
                ],
                "topology_signature_sha256": receipt["topology_signature_sha256"],
                "operation_sequence_sha256": receipt[
                    "operation_sequence_sha256"
                ],
                "profile_signature_sha256": receipt["profile_signature_sha256"],
                "part_zone_signature_sha256": receipt[
                    "part_zone_signature_sha256"
                ],
                "bounds_mm": receipt["bounds_mm"],
                "glb_sha256": receipt["glb_sha256"],
            }
        )
    entries_by_task = {entry["task_id"]: entry for entry in expected_entries}
    expected_comparisons = []
    for task_a, task_b in itertools.combinations(task_ids, 2):
        entry_a = entries_by_task[task_a]
        entry_b = entries_by_task[task_b]
        field_by_axis = {
            "topology": "topology_signature_sha256",
            "operation_sequence": "operation_sequence_sha256",
            "profile": "profile_signature_sha256",
            "part_zone": "part_zone_signature_sha256",
            "bounds": "bounds_mm",
            "glb": "glb_sha256",
        }
        difference_axes = [
            axis
            for axis in STRUCTURAL_DIFFERENCE_AXES
            if entry_a[field_by_axis[axis]] != entry_b[field_by_axis[axis]]
        ]
        same_semantic_structure = (
            entry_a["semantic_structure_sha256"]
            == entry_b["semantic_structure_sha256"]
        )
        same_normalized_geometry = (
            entry_a["normalized_geometry_sha256"]
            == entry_b["normalized_geometry_sha256"]
        )
        failure_codes = []
        if same_semantic_structure:
            failure_codes.append("E005_STRUCTURAL_PARAMETRIC_CLONE")
        if same_normalized_geometry:
            failure_codes.append("E005_STRUCTURAL_MATERIAL_OR_SCALE_CLONE")
        if same_semantic_structure and same_normalized_geometry:
            clone_class = "same_structure_and_geometry"
        elif same_semantic_structure:
            clone_class = "same_topology_parametric_clone"
        elif same_normalized_geometry:
            clone_class = "material_or_scale_clone"
        else:
            clone_class = "none"
        structurally_distinct = not same_semantic_structure and not same_normalized_geometry
        expected_comparisons.append(
            {
                "task_a": task_a,
                "task_b": task_b,
                "same_morphology_family": tasks_by_id[task_a]["morphology_family"]
                == tasks_by_id[task_b]["morphology_family"],
                "difference_axes": difference_axes,
                "same_semantic_structure": same_semantic_structure,
                "same_normalized_geometry": same_normalized_geometry,
                "clone_class": clone_class,
                "failure_codes": failure_codes,
                "structurally_distinct": structurally_distinct,
            }
        )
    return expected_entries, expected_comparisons


def build_structural_difference_matrix(
    *,
    matrix_id: str,
    task_set_sha256: str,
    tasks_by_id: dict[str, dict[str, object]],
    receipts: list[dict[str, object]],
    status: str,
) -> dict[str, object]:
    if status == "not_run":
        entries: list[dict[str, object]] = []
        comparisons: list[dict[str, object]] = []
    elif status == "complete":
        entries, comparisons = derive_structural_entries_and_comparisons(
            tasks_by_id=tasks_by_id,
            receipts=receipts,
        )
    else:
        raise ValueError("E005_STRUCTURAL_MATRIX_STATUS_INVALID")
    distinct_count = sum(
        comparison["structurally_distinct"] for comparison in comparisons
    )
    matrix_pass = len(comparisons) == 435 and distinct_count == 435
    return {
        "schema_version": "E005StructuralDifferenceMatrix@1",
        "matrix_id": matrix_id,
        "task_set_sha256": task_set_sha256,
        "run_receipts_sha256": canonical_sha256(receipts),
        "status": status,
        "entry_count": len(entries),
        "entries": entries,
        "entries_sha256": canonical_sha256(entries),
        "pair_count": len(comparisons),
        "comparisons": comparisons,
        "comparisons_sha256": canonical_sha256(comparisons),
        "structurally_distinct_pair_count": distinct_count,
        "matrix_pass": matrix_pass,
    }


def validate_structural_difference_matrix(
    payload: dict[str, object],
    *,
    task_set_sha256: str,
    tasks_by_id: dict[str, dict[str, object]],
    receipts: list[dict[str, object]],
) -> dict[str, int | bool]:
    schemas, registry = schema_registry()
    validate_schema(
        schemas,
        registry,
        "e005-structural-difference-matrix-v1.schema.json",
        payload,
    )
    if payload["task_set_sha256"] != task_set_sha256:
        raise ValueError("E005_STRUCTURAL_MATRIX_TASK_SET_HASH_MISMATCH")
    if payload["run_receipts_sha256"] != canonical_sha256(receipts):
        raise ValueError("E005_STRUCTURAL_MATRIX_RECEIPTS_HASH_MISMATCH")
    if payload["entry_count"] != len(payload["entries"]):
        raise ValueError("E005_STRUCTURAL_MATRIX_ENTRY_COUNT_MISMATCH")
    if payload["entries_sha256"] != canonical_sha256(payload["entries"]):
        raise ValueError("E005_STRUCTURAL_MATRIX_ENTRIES_HASH_MISMATCH")
    if payload["pair_count"] != len(payload["comparisons"]):
        raise ValueError("E005_STRUCTURAL_MATRIX_PAIR_COUNT_MISMATCH")
    if payload["comparisons_sha256"] != canonical_sha256(payload["comparisons"]):
        raise ValueError("E005_STRUCTURAL_MATRIX_COMPARISONS_HASH_MISMATCH")
    if payload["status"] == "not_run":
        return {
            "structural_pair_count": 0,
            "structurally_distinct_pair_count": 0,
            "structural_difference_matrix_pass": False,
        }

    task_ids = list(tasks_by_id)
    receipts_by_task = {receipt["task_id"]: receipt for receipt in receipts}
    if len(receipts) != 30 or set(receipts_by_task) != set(task_ids):
        raise ValueError("E005_STRUCTURAL_MATRIX_RUN_COVERAGE_INVALID")
    for receipt in receipts:
        if (
            receipt["status"] not in {"passed_without_patch", "passed_after_patch"}
            or receipt["run_mode"] != "formal_provider"
            or receipt["distribution_eligible"] is not True
        ):
            raise ValueError("E005_STRUCTURAL_MATRIX_NON_FORMAL_RUN")
    expected_entries, expected_comparisons = derive_structural_entries_and_comparisons(
        tasks_by_id=tasks_by_id,
        receipts=receipts,
    )
    if payload["entries"] != expected_entries:
        raise ValueError("E005_STRUCTURAL_MATRIX_ENTRIES_DERIVATION_MISMATCH")
    if payload["comparisons"] != expected_comparisons:
        raise ValueError("E005_STRUCTURAL_MATRIX_COMPARISON_DERIVATION_MISMATCH")
    distinct_count = sum(
        comparison["structurally_distinct"] for comparison in expected_comparisons
    )
    matrix_pass = len(expected_comparisons) == 435 and distinct_count == 435
    if payload["structurally_distinct_pair_count"] != distinct_count:
        raise ValueError("E005_STRUCTURAL_MATRIX_DISTINCT_COUNT_MISMATCH")
    if payload["matrix_pass"] is not matrix_pass:
        raise ValueError("E005_STRUCTURAL_MATRIX_PASS_MISMATCH")
    return {
        "structural_pair_count": len(expected_comparisons),
        "structurally_distinct_pair_count": distinct_count,
        "structural_difference_matrix_pass": matrix_pass,
    }


def validate_distribution_report(
    payload: dict[str, object],
    *,
    task_set_sha256: str,
    receipts: list[dict[str, object]] | None = None,
    human_review_bundle: dict[str, object] | None = None,
    structural_matrix: dict[str, object] | None = None,
    tasks_by_id: dict[str, dict[str, object]] | None = None,
    provider_authorization: dict[str, object] | None = None,
) -> None:
    schemas, registry = schema_registry()
    validate_schema(schemas, registry, "e005-distribution-report-v1.schema.json", payload)
    if payload["task_set_sha256"] != task_set_sha256:
        raise ValueError("E005_REPORT_TASK_SET_HASH_MISMATCH")
    if provider_authorization is None:
        raise ValueError("E005_REPORT_PROVIDER_AUTHORIZATION_EVIDENCE_REQUIRED")
    if payload["provider_authorization_sha256"] != canonical_sha256(
        provider_authorization
    ):
        raise ValueError("E005_REPORT_PROVIDER_AUTHORIZATION_HASH_MISMATCH")
    provider_authorized = validate_provider_run_authorization(
        provider_authorization,
        task_set_sha256=task_set_sha256,
    )
    if receipts is None:
        raise ValueError("E005_REPORT_RECEIPT_EVIDENCE_REQUIRED")
    if human_review_bundle is None:
        raise ValueError("E005_REPORT_HUMAN_REVIEW_EVIDENCE_REQUIRED")
    if payload["human_review_bundle_sha256"] != canonical_sha256(human_review_bundle):
        raise ValueError("E005_REPORT_HUMAN_REVIEW_BUNDLE_HASH_MISMATCH")
    review_summary = validate_human_review_bundle(
        human_review_bundle,
        task_set_sha256=task_set_sha256,
        receipts=receipts,
    )
    if structural_matrix is None or tasks_by_id is None:
        raise ValueError("E005_REPORT_STRUCTURAL_MATRIX_EVIDENCE_REQUIRED")
    if payload["structural_matrix_sha256"] != canonical_sha256(structural_matrix):
        raise ValueError("E005_REPORT_STRUCTURAL_MATRIX_HASH_MISMATCH")
    structural_summary = validate_structural_difference_matrix(
        structural_matrix,
        task_set_sha256=task_set_sha256,
        tasks_by_id=tasks_by_id,
        receipts=receipts,
    )
    if len(receipts) != payload["total_receipt_count"]:
        raise ValueError("E005_REPORT_RECEIPT_COUNT_MISMATCH")
    receipt_task_ids = [receipt["task_id"] for receipt in receipts]
    if len(receipt_task_ids) != len(set(receipt_task_ids)):
        raise ValueError("E005_REPORT_RECEIPT_TASK_DUPLICATE")
    if payload["receipts_sha256"] != canonical_sha256(receipts):
        raise ValueError("E005_REPORT_RECEIPT_HASH_MISMATCH")
    statuses = Counter(receipt["status"] for receipt in receipts)
    formal_receipts = [
        receipt for receipt in receipts if receipt["run_mode"] == "formal_provider"
    ]
    if formal_receipts and not provider_authorized:
        raise ValueError("E005_REPORT_FORMAL_RUN_NOT_AUTHORIZED")
    for receipt in formal_receipts:
        if (
            receipt["provider_authorization_id"]
            != provider_authorization["authorization_id"]
            or receipt["provider_authorization_sha256"]
            != canonical_sha256(provider_authorization)
        ):
            raise ValueError("E005_REPORT_FORMAL_RUN_AUTHORIZATION_MISMATCH")
        if any(
            evidence["authorization_binding_sha256"]
            != provider_authorization["authorization_binding_sha256"]
            or evidence["provider_id"] != provider_authorization["provider_id"]
            or evidence["model_id"] != provider_authorization["model_id"]
            for evidence in receipt["provider_call_evidence"]
        ):
            raise ValueError("E005_REPORT_PROVIDER_LEDGER_SCOPE_MISMATCH")
    derived_counts = {
        "not_run_count": statuses["not_run"],
        "first_pass_success_count": statuses["passed_without_patch"],
        "patched_success_count": statuses["passed_after_patch"],
        "failed_count": statuses["failed"],
        "cancelled_count": statuses["cancelled"],
        "run_count": len(receipts) - statuses["not_run"],
        "lineage_complete_count": sum(
            receipt["status"] in {"passed_without_patch", "passed_after_patch"}
            for receipt in receipts
        ),
        **review_summary,
        **structural_summary,
    }
    for field, derived in derived_counts.items():
        if payload[field] != derived:
            raise ValueError(f"E005_REPORT_DERIVED_COUNT_MISMATCH:{field}")
    failure_histogram = Counter(
        code for receipt in receipts for code in receipt["failure_codes"]
    )
    if payload["failure_histogram"] != dict(sorted(failure_histogram.items())):
        raise ValueError("E005_REPORT_FAILURE_HISTOGRAM_MISMATCH")
    elapsed = sorted(
        receipt["elapsed_ms"] for receipt in receipts if receipt["status"] != "not_run"
    )
    if elapsed:
        derived_timings = {
            "p50_ms": elapsed[max(0, math.ceil(len(elapsed) * 0.50) - 1)],
            "p90_ms": elapsed[max(0, math.ceil(len(elapsed) * 0.90) - 1)],
            "max_ms": elapsed[-1],
        }
        for field, derived in derived_timings.items():
            if payload.get(field) != derived:
                raise ValueError(f"E005_REPORT_DERIVED_TIMING_MISMATCH:{field}")
    if payload["total_receipt_count"] != payload["run_count"] + payload["not_run_count"]:
        raise ValueError("E005_REPORT_TOTAL_COUNT_INVALID")
    classified_run_count = sum(
        payload[key]
        for key in (
            "first_pass_success_count",
            "patched_success_count",
            "failed_count",
            "cancelled_count",
        )
    )
    if payload["run_count"] != classified_run_count:
        raise ValueError("E005_REPORT_RUN_CLASSIFICATION_INVALID")
    if payload["first_pass_human_quality_count"] > payload["first_pass_success_count"]:
        raise ValueError("E005_REPORT_FIRST_PASS_HUMAN_COUNT_INVALID")
    if payload["within_one_patch_human_quality_count"] < payload["first_pass_human_quality_count"]:
        raise ValueError("E005_REPORT_HUMAN_QUALITY_ORDER_INVALID")
    if payload["within_one_patch_human_quality_count"] > (
        payload["first_pass_success_count"] + payload["patched_success_count"]
    ):
        raise ValueError("E005_REPORT_HUMAN_QUALITY_SUCCESS_INVALID")
    if payload["human_review_complete_count"] > payload["run_count"]:
        raise ValueError("E005_REPORT_HUMAN_REVIEW_COUNT_INVALID")
    if payload["lineage_complete_count"] > payload["run_count"]:
        raise ValueError("E005_REPORT_LINEAGE_COUNT_INVALID")
    timing_fields = ("p50_ms", "p90_ms", "max_ms")
    has_timings = [field in payload for field in timing_fields]
    if any(has_timings) != all(has_timings):
        raise ValueError("E005_REPORT_TIMING_PARTIAL")
    if payload["run_count"] == 0 and any(has_timings):
        raise ValueError("E005_REPORT_EMPTY_RUN_HAS_TIMING")
    if all(has_timings) and not (payload["p50_ms"] <= payload["p90_ms"] <= payload["max_ms"]):
        raise ValueError("E005_REPORT_TIMING_ORDER_INVALID")

    formal_eligible = (
        provider_authorized
        and payload["total_receipt_count"] == 30
        and payload["run_count"] == 30
        and payload["not_run_count"] == 0
        and payload["lineage_complete_count"] == 30
        and payload["structural_difference_matrix_pass"] is True
        and payload["human_review_complete_count"] == 30
        and payload["independent_reviewers_per_task_minimum"] == 3
        and payload["first_pass_human_quality_count"] >= 21
        and payload["within_one_patch_human_quality_count"] >= 26
        and all(has_timings)
        and payload["p50_ms"] <= 32000
        and payload["p90_ms"] <= 70000
        and payload["max_ms"] <= 105000
    )
    if payload["formal_eligible"] is not formal_eligible:
        raise ValueError("E005_REPORT_FORMAL_ELIGIBILITY_MISMATCH")


def self_test(payload: dict[str, object]) -> None:
    duplicate = copy.deepcopy(payload)
    duplicate["tasks"][1]["task_id"] = duplicate["tasks"][0]["task_id"]
    try:
        validate_task_set(duplicate)
    except ValueError as error:
        if "E005_TASK_ID_DUPLICATE" not in str(error):
            raise
    else:
        raise AssertionError("E005 duplicate task self-test did not fail")
    leaked = copy.deepcopy(payload)
    leaked["tasks"][0]["prompt"] += " C111 rotor"
    try:
        validate_task_set(leaked)
    except ValueError as error:
        if "E005_FIXTURE_LEAK" not in str(error):
            raise
    else:
        raise AssertionError("E005 fixture leak self-test did not fail")

    task_set_sha256 = canonical_sha256(payload)
    tasks_by_id = {task["task_id"]: task for task in payload["tasks"]}
    not_authorized_provider = json.loads(
        PROVIDER_AUTHORIZATION_FIXTURE.read_text(encoding="utf-8")
    )
    if validate_provider_run_authorization(
        not_authorized_provider,
        task_set_sha256=task_set_sha256,
    ):
        raise AssertionError("E005 not-authorized Provider self-test became authorized")
    empty_hash = "0" * 64
    not_run_receipt = {
        "schema_version": "E005RunReceipt@1",
        "run_id": "e005_not_run_test",
        "task_set_sha256": task_set_sha256,
        "task_id": payload["tasks"][0]["task_id"],
        "status": "not_run",
        "run_mode": "offline_deterministic",
        "distribution_eligible": False,
        "author_source_mode": "missing",
        "task_payload_sha256": canonical_sha256(payload["tasks"][0]),
        "request_sha256": canonical_sha256({"task_id": payload["tasks"][0]["task_id"], "source": "missing"}),
        "authoring_count": 0,
        "patch_count": 0,
        "network_provider_calls": 0,
        "billable_cost_microusd": 0,
        "failure_codes": ["E005_SOURCE_UNAVAILABLE"],
        "human_review_status": "not_run",
    }
    validate_run_receipt(not_run_receipt, task_set_sha256=task_set_sha256, tasks_by_id=tasks_by_id)
    forged_not_run = copy.deepcopy(not_run_receipt)
    forged_not_run["glb_sha256"] = empty_hash
    try:
        validate_run_receipt(forged_not_run, task_set_sha256=task_set_sha256, tasks_by_id=tasks_by_id)
    except ValueError as error:
        if "E005_SCHEMA_INVALID" not in str(error):
            raise
    else:
        raise AssertionError("E005 not-run artifact self-test did not fail")

    not_run_review_bundle = {
        "schema_version": "E005HumanReviewBundle@1",
        "bundle_id": "e005_review_self_test",
        "task_set_sha256": task_set_sha256,
        "run_receipts_sha256": canonical_sha256([]),
        "status": "not_run",
        "reviewer_commitments": [],
        "review_count": 0,
        "reviews": [],
        "reviews_sha256": canonical_sha256([]),
    }
    validate_human_review_bundle(
        not_run_review_bundle,
        task_set_sha256=task_set_sha256,
        receipts=[],
    )
    not_run_structural_matrix = build_structural_difference_matrix(
        matrix_id="e005_structural_matrix_self_test",
        task_set_sha256=task_set_sha256,
        tasks_by_id=tasks_by_id,
        receipts=[],
        status="not_run",
    )
    validate_structural_difference_matrix(
        not_run_structural_matrix,
        task_set_sha256=task_set_sha256,
        tasks_by_id=tasks_by_id,
        receipts=[],
    )
    empty_report = {
        "schema_version": "E005DistributionReport@1",
        "report_id": "e005_empty_report_test",
        "task_set_sha256": task_set_sha256,
        "provider_authorization_sha256": canonical_sha256(
            not_authorized_provider
        ),
        "total_receipt_count": 0,
        "run_count": 0,
        "not_run_count": 0,
        "first_pass_success_count": 0,
        "patched_success_count": 0,
        "failed_count": 0,
        "cancelled_count": 0,
        "human_review_complete_count": 0,
        "human_review_receipt_count": 0,
        "human_review_bundle_sha256": canonical_sha256(not_run_review_bundle),
        "independent_reviewers_per_task_minimum": 0,
        "first_pass_human_quality_count": 0,
        "within_one_patch_human_quality_count": 0,
        "lineage_complete_count": 0,
        "structural_matrix_sha256": canonical_sha256(not_run_structural_matrix),
        "structural_pair_count": 0,
        "structurally_distinct_pair_count": 0,
        "structural_difference_matrix_pass": False,
        "formal_eligible": False,
        "failure_histogram": {},
        "receipts_sha256": canonical_sha256([]),
    }
    validate_distribution_report(
        empty_report,
        task_set_sha256=task_set_sha256,
        receipts=[],
        human_review_bundle=not_run_review_bundle,
        structural_matrix=not_run_structural_matrix,
        tasks_by_id=tasks_by_id,
        provider_authorization=not_authorized_provider,
    )
    forged_report = copy.deepcopy(empty_report)
    forged_report["formal_eligible"] = True
    try:
        validate_distribution_report(
            forged_report,
            task_set_sha256=task_set_sha256,
            receipts=[],
            human_review_bundle=not_run_review_bundle,
            structural_matrix=not_run_structural_matrix,
            tasks_by_id=tasks_by_id,
            provider_authorization=not_authorized_provider,
        )
    except ValueError as error:
        if "E005_SCHEMA_INVALID" not in str(error):
            raise
    else:
        raise AssertionError("E005 formal eligibility self-test did not fail")

    def self_test_hash(label: str) -> str:
        return hashlib.sha256(label.encode("utf-8")).hexdigest()

    formal_provider_authorization = {
        "schema_version": "E005ProviderRunAuthorization@1",
        "authorization_id": "e005_provider_contract_self_test",
        "task_set_sha256": task_set_sha256,
        "status": "authorized",
        "grant_mode": "explicit_user_confirmation",
        "provider_id": "provider_self_test",
        "model_id": "model_self_test_v1",
        "source_policy_sha256": self_test_hash("source-policy"),
        "pricing_snapshot_sha256": self_test_hash("pricing"),
        "disclosure_sha256": self_test_hash("disclosure"),
        "authorized_at": "2026-07-29T00:00:00Z",
        "expires_at": "2026-07-30T00:00:00Z",
        "maximum_author_calls": 30,
        "maximum_patch_calls": 30,
        "maximum_total_calls": 60,
        "maximum_input_tokens": 300000,
        "maximum_output_tokens": 150000,
        "maximum_variable_cost_microusd": 30000000,
        "maximum_batch_wall_time_ms": 3150000,
        "maximum_single_call_wall_time_ms": 105000,
        "whole_object_template_policy": "forbidden",
    }
    formal_provider_authorization["authorization_binding_sha256"] = canonical_sha256(
        formal_provider_authorization
    )
    if not validate_provider_run_authorization(
        formal_provider_authorization,
        task_set_sha256=task_set_sha256,
    ):
        raise AssertionError("E005 authorized Provider self-test was rejected")

    formal_receipts: list[dict[str, object]] = []
    for index, task in enumerate(payload["tasks"]):
        task_id = task["task_id"]
        source_hash = self_test_hash(f"{task_id}:source")
        expanded_hash = self_test_hash(f"{task_id}:expanded")
        shape_hash = self_test_hash(f"{task_id}:shape")
        topology_hash = self_test_hash(f"{task_id}:topology")
        operation_sequence_hash = self_test_hash(f"{task_id}:operation_sequence")
        profile_hash = self_test_hash(f"{task_id}:profile")
        part_zone_hash = self_test_hash(f"{task_id}:part_zone")
        semantic_structure_hash = self_test_hash(f"{task_id}:semantic_structure")
        normalized_geometry_hash = self_test_hash(f"{task_id}:normalized_geometry")
        glb_hash = self_test_hash(f"{task_id}:glb")
        structural_descriptor_hash = canonical_sha256(
            {
                "final_source_program_sha256": source_hash,
                "shape_program_sha256": shape_hash,
                "glb_sha256": glb_hash,
                "semantic_structure_sha256": semantic_structure_hash,
                "normalized_geometry_sha256": normalized_geometry_hash,
            }
        )
        gate_hash = self_test_hash(f"{task_id}:gate")
        request_hash = self_test_hash(f"{task_id}:request")
        fixed_views = {
            view_id: self_test_hash(f"{task_id}:view:{view_id}")
            for view_id in ("front", "iso", "side", "top")
        }
        fixed_view_hash = canonical_sha256(fixed_views)
        phase_outputs = [
            source_hash,
            source_hash,
            expanded_hash,
            shape_hash,
            glb_hash,
            fixed_view_hash,
            gate_hash,
            source_hash,
        ]
        phase_names = [
            "author",
            "validate",
            "expand",
            "lower",
            "compile_readback",
            "render",
            "evaluate",
            "preview",
        ]
        phase_inputs = [request_hash, *phase_outputs[:-1]]
        phases = [
            {
                "sequence": sequence,
                "phase": phase_name,
                "duration_ms": 0,
                "input_sha256": phase_input,
                "output_sha256": phase_output,
                "cache": "not_applicable",
            }
            for sequence, (phase_name, phase_input, phase_output) in enumerate(
                zip(phase_names, phase_inputs, phase_outputs), start=1
            )
        ]
        provider_call_evidence = [
            {
                "schema_version": "E005ProviderBudgetEvidence@1",
                "authorization_id": formal_provider_authorization[
                    "authorization_id"
                ],
                "authorization_binding_sha256": formal_provider_authorization[
                    "authorization_binding_sha256"
                ],
                "reservation_id": f"e005_reservation_self_test_{index + 1:02d}",
                "task_id": task_id,
                "task_payload_sha256": canonical_sha256(task),
                "request_sha256": request_hash,
                "provider_id": formal_provider_authorization["provider_id"],
                "model_id": formal_provider_authorization["model_id"],
                "call_kind": "author",
                "call_number": index + 1,
                "kind_call_number": index + 1,
                "settlement": "accounted",
                "network_call_made": True,
                "outcome_code": "PROVIDER_COMPLETED_PASSED",
                "output_source_sha256": source_hash,
                "output_gate_sha256": gate_hash,
                "reserved_input_tokens": 4000,
                "reserved_output_tokens": 8192,
                "reserved_cost_ceiling_microusd": 5000,
                "author_calls_accounted_after": index + 1,
                "patch_calls_accounted_after": 0,
                "calls_accounted_after": index + 1,
                "accounted_input_tokens_after": 4000 * (index + 1),
                "accounted_output_tokens_after": 8192 * (index + 1),
                "accounted_cost_ceiling_microusd_after": 5000 * (index + 1),
                "settled_at_unix_ms": 1785283200000 + index,
            }
        ]
        receipt = {
            "schema_version": "E005RunReceipt@1",
            "run_id": f"run_{task_id}_formal",
            "task_set_sha256": task_set_sha256,
            "task_id": task_id,
            "status": "passed_without_patch",
            "run_mode": "formal_provider",
            "distribution_eligible": True,
            "author_source_mode": "provider_authored_v2",
            "task_payload_sha256": canonical_sha256(task),
            "request_sha256": request_hash,
            "authoring_count": 1,
            "patch_count": 0,
            "provider_authorization_id": formal_provider_authorization[
                "authorization_id"
            ],
            "provider_authorization_sha256": canonical_sha256(
                formal_provider_authorization
            ),
            "provider_call_evidence": provider_call_evidence,
            "provider_call_evidence_sha256": canonical_sha256(
                provider_call_evidence
            ),
            "source_program_sha256": source_hash,
            "expanded_program_sha256": expanded_hash,
            "shape_program_sha256": shape_hash,
            "structural_descriptor_sha256": structural_descriptor_hash,
            "semantic_structure_sha256": semantic_structure_hash,
            "normalized_geometry_sha256": normalized_geometry_hash,
            "topology_signature_sha256": topology_hash,
            "operation_sequence_sha256": operation_sequence_hash,
            "profile_signature_sha256": profile_hash,
            "part_zone_signature_sha256": part_zone_hash,
            "glb_sha256": glb_hash,
            "fixed_view_sha256": fixed_view_hash,
            "fixed_views": fixed_views,
            "vp204_session_sha256": self_test_hash(f"{task_id}:session"),
            "vp204_receipt_sha256": self_test_hash(f"{task_id}:receipt"),
            "gate_outcome_sha256": gate_hash,
            "compile_readback_sha256": self_test_hash(f"{task_id}:readback"),
            "restricted_geometry_evidence_sha256": self_test_hash(f"{task_id}:evidence"),
            "artifact_profile_id": "interactive_preview",
            "runtime_manifest_version": "ShapeProgramRuntimeManifest@1",
            "triangle_count": 100 + index,
            "bounds_mm": [400.0 + index, 300.0, 200.0],
            "mesh_count": 1,
            "primitive_count": 4,
            "material_count": 3,
            "usage": {
                "provider_requests": 1,
                "product_tool_calls": 4,
                "input_tokens": 2000,
                "output_tokens": 1000,
                "prompt_cache_hit_tokens": 0,
                "prompt_cache_miss_tokens": 2000,
                "estimated_cost_microusd": 1000,
            },
            "phase_receipts": phases,
            "elapsed_ms": 1000 + index,
            "network_provider_calls": 1,
            "billable_cost_microusd": 1000,
            "failure_codes": [],
            "human_review_status": "pending",
        }
        validate_run_receipt(
            receipt,
            task_set_sha256=task_set_sha256,
            tasks_by_id=tasks_by_id,
        )
        formal_receipts.append(receipt)

    tampered_provider_evidence = copy.deepcopy(formal_receipts[0])
    tampered_provider_evidence["provider_call_evidence"][0][
        "request_sha256"
    ] = self_test_hash("tampered-provider-request")
    try:
        validate_run_receipt(
            tampered_provider_evidence,
            task_set_sha256=task_set_sha256,
            tasks_by_id=tasks_by_id,
        )
    except ValueError as error:
        if "E005_RECEIPT_PROVIDER_EVIDENCE_HASH_MISMATCH" not in str(error):
            raise
    else:
        raise AssertionError("E005 tampered Provider evidence hash did not fail")

    swapped_provider_evidence = copy.deepcopy(formal_receipts[0])
    swapped_provider_evidence["provider_call_evidence"] = copy.deepcopy(
        formal_receipts[1]["provider_call_evidence"]
    )
    swapped_provider_evidence["provider_call_evidence_sha256"] = canonical_sha256(
        swapped_provider_evidence["provider_call_evidence"]
    )
    try:
        validate_run_receipt(
            swapped_provider_evidence,
            task_set_sha256=task_set_sha256,
            tasks_by_id=tasks_by_id,
        )
    except ValueError as error:
        if "E005_RECEIPT_PROVIDER_EVIDENCE_LINEAGE_MISMATCH" not in str(error):
            raise
    else:
        raise AssertionError("E005 swapped Provider reservation evidence did not fail")

    commitments = [
        {
            "reviewer_id": f"reviewer_e005_{index}",
            "identity_commitment_sha256": self_test_hash(f"reviewer:{index}"),
        }
        for index in range(1, 4)
    ]
    reviews: list[dict[str, object]] = []
    base_order = ["front", "iso", "side", "top"]
    for reviewer_index, commitment in enumerate(commitments):
        view_order = base_order[reviewer_index:] + base_order[:reviewer_index]
        for receipt in formal_receipts:
            task_id = receipt["task_id"]
            reviews.append(
                {
                    "review_id": f"e005_reviewitem_{reviewer_index + 1}_{task_id}",
                    "reviewer_id": commitment["reviewer_id"],
                    "reviewer_commitment_sha256": commitment[
                        "identity_commitment_sha256"
                    ],
                    "task_id": task_id,
                    "run_id": receipt["run_id"],
                    "run_receipt_sha256": canonical_sha256(receipt),
                    "fixed_view_sha256": receipt["fixed_view_sha256"],
                    "fixed_views": receipt["fixed_views"],
                    "blind_packet_sha256": self_test_hash(
                        f"packet:{reviewer_index}:{task_id}"
                    ),
                    "view_order": view_order,
                    "human_reviewer": True,
                    "independent_of_implementation": True,
                    "implementation_participant": False,
                    "agent_or_vlm_used": False,
                    "submitted_at": f"2026-07-{10 + reviewer_index:02d}T12:00:00Z",
                    "result_stage": "first_pass",
                    "scores": {dimension: 4 for dimension in HUMAN_SCORE_DIMENSIONS},
                    "overall_score": 4,
                }
            )
    complete_review_bundle = {
        "schema_version": "E005HumanReviewBundle@1",
        "bundle_id": "e005_review_contract_self_test",
        "task_set_sha256": task_set_sha256,
        "run_receipts_sha256": canonical_sha256(formal_receipts),
        "status": "complete",
        "reviewer_commitments": commitments,
        "review_count": len(reviews),
        "reviews": reviews,
        "reviews_sha256": canonical_sha256(reviews),
    }
    review_summary = validate_human_review_bundle(
        complete_review_bundle,
        task_set_sha256=task_set_sha256,
        receipts=formal_receipts,
    )
    if review_summary != {
        "human_review_complete_count": 30,
        "independent_reviewers_per_task_minimum": 3,
        "first_pass_human_quality_count": 30,
        "within_one_patch_human_quality_count": 30,
        "human_review_receipt_count": 90,
    }:
        raise AssertionError("E005 complete review derivation self-test failed")
    complete_structural_matrix = build_structural_difference_matrix(
        matrix_id="e005_structural_matrix_contract_self_test",
        task_set_sha256=task_set_sha256,
        tasks_by_id=tasks_by_id,
        receipts=formal_receipts,
        status="complete",
    )
    structural_summary = validate_structural_difference_matrix(
        complete_structural_matrix,
        task_set_sha256=task_set_sha256,
        tasks_by_id=tasks_by_id,
        receipts=formal_receipts,
    )
    if structural_summary != {
        "structural_pair_count": 435,
        "structurally_distinct_pair_count": 435,
        "structural_difference_matrix_pass": True,
    }:
        raise AssertionError("E005 structural matrix derivation self-test failed")
    formal_report = {
        "schema_version": "E005DistributionReport@1",
        "report_id": "e005_formal_contract_self_test",
        "task_set_sha256": task_set_sha256,
        "provider_authorization_sha256": canonical_sha256(
            formal_provider_authorization
        ),
        "total_receipt_count": 30,
        "run_count": 30,
        "not_run_count": 0,
        "first_pass_success_count": 30,
        "patched_success_count": 0,
        "failed_count": 0,
        "cancelled_count": 0,
        **review_summary,
        "human_review_bundle_sha256": canonical_sha256(complete_review_bundle),
        "lineage_complete_count": 30,
        **structural_summary,
        "structural_matrix_sha256": canonical_sha256(complete_structural_matrix),
        "p50_ms": 1014,
        "p90_ms": 1026,
        "max_ms": 1029,
        "formal_eligible": True,
        "failure_histogram": {},
        "receipts_sha256": canonical_sha256(formal_receipts),
    }
    validate_distribution_report(
        formal_report,
        task_set_sha256=task_set_sha256,
        receipts=formal_receipts,
        human_review_bundle=complete_review_bundle,
        structural_matrix=complete_structural_matrix,
        tasks_by_id=tasks_by_id,
        provider_authorization=formal_provider_authorization,
    )
    tampered_matrix = copy.deepcopy(complete_structural_matrix)
    tampered_matrix["comparisons"][0]["difference_axes"] = ["glb"]
    tampered_matrix["comparisons"][0]["structurally_distinct"] = True
    tampered_matrix["comparisons_sha256"] = canonical_sha256(
        tampered_matrix["comparisons"]
    )
    try:
        validate_structural_difference_matrix(
            tampered_matrix,
            task_set_sha256=task_set_sha256,
            tasks_by_id=tasks_by_id,
            receipts=formal_receipts,
        )
    except ValueError as error:
        if "E005_STRUCTURAL_MATRIX_COMPARISON_DERIVATION_MISMATCH" not in str(error):
            raise
    else:
        raise AssertionError("E005 tampered structural matrix self-test did not fail")
    tampered_review_bundle = copy.deepcopy(complete_review_bundle)
    tampered_review_bundle["reviews"][0]["overall_score"] = 5
    tampered_review_bundle["reviews_sha256"] = canonical_sha256(
        tampered_review_bundle["reviews"]
    )
    try:
        validate_human_review_bundle(
            tampered_review_bundle,
            task_set_sha256=task_set_sha256,
            receipts=formal_receipts,
        )
    except ValueError as error:
        if "E005_REVIEW_OVERALL_SCORE_MISMATCH" not in str(error):
            raise
    else:
        raise AssertionError("E005 tampered review score self-test did not fail")


def main() -> int:
    payload = json.loads(FIXTURE.read_text(encoding="utf-8"))
    evidence = validate_task_set(payload)
    self_test(payload)
    print(json.dumps(evidence, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
