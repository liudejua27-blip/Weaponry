#!/usr/bin/env python3
"""Fail-closed structural gate for WPN-ARCH-EVALUATION-001.

This check records the physical boundary reached by the Evaluation extraction
atom.  It intentionally does not claim that every Evaluation record family
has moved: the ReadModel and QualityEvidence repositories remain explicit
follow-up gaps.
"""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PROFILE = ROOT / "packages/forgecad-contracts/profiles/weaponry-knife-p0.json"
RUNTIME = ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src"
RUNTIME_LIB = RUNTIME / "lib.rs"
RUNTIME_SERVICES = RUNTIME / "runtime_services.rs"
RUNTIME_ROUTER = RUNTIME / "runtime_operation_router.rs"
EVALUATION_SERVICE = RUNTIME / "evaluation_service.rs"
STORE = ROOT / "apps/desktop/src-tauri/crates/forgecad-store/src"
STORE_ROOT = STORE / "lib.rs"
EVALUATION_REPOSITORY = STORE / "evaluation_repository.rs"
REPOSITORY_BOUNDARIES = STORE / "repository_boundaries.rs"
CONTRACT_MAP = (
    ROOT / "apps/desktop/src-tauri/crates/forgecad-contracts/src/weaponry_domain_map.rs"
)

EXPECTED_FACADES = ("observe", "quality_review", "job")
EXPECTED_READ = (
    # observe (10)
    "artifact_readback_get",
    "authoring_mesh_durable_get",
    "authoring_mesh_get",
    "authoring_mesh_transaction_get",
    "authoring_mesh_v2_durable_get",
    "candidate_get",
    "production_stage_transition_get",
    "scene_observe_get",
    "selection_get",
    "snapshot_get",
    # quality_review (17, including one façade-native operation)
    "candidate_material_surface_quality_get",
    "candidate_topology_quality_get",
    "critic_report_get",
    "production_weapon_form_quality_get",
    "production_weapon_form_quality_v2_get",
    "quality_get",
    "render_evidence_integrity_get",
    "render_evidence_replay_get",
    "render_pass_get",
    "silhouette_candidate_compare",
    "silhouette_evaluation_objective_prepare",
    "silhouette_fit_prepare",
    "silhouette_part_error_get",
    "silhouette_rig_hash",
    "silhouette_target_get",
    "visual_evidence_bundle_get",
    "knife_pass_state_get",
    # job (4)
    "job_events_read",
    "job_get",
    "job_result_get",
    "optimization_job_get",
)
EXPECTED_WRITE = (
    # quality_review (8, including one façade-native operation)
    "candidate_material_surface_quality_prepare",
    "candidate_topology_quality_prepare",
    "human_visual_review_submit",
    "production_weapon_form_quality_prepare",
    "production_weapon_form_quality_v2_prepare",
    "reference_compare_prepare",
    "visual_review_submit",
    "knife_pass_state_prepare",
    # job (4)
    "job_cancel",
    "optimization_job_prepare",
    "optimization_job_resume",
    "primary_form_repair_job_prepare",
)

EXPECTED_OBSERVE = (
    "artifact_readback_get",
    "authoring_mesh_durable_get",
    "authoring_mesh_get",
    "authoring_mesh_transaction_get",
    "authoring_mesh_v2_durable_get",
    "candidate_get",
    "production_stage_transition_get",
    "scene_observe_get",
    "selection_get",
    "snapshot_get",
)
EXPECTED_QUALITY_TOOL_READ = (
    "candidate_material_surface_quality_get",
    "candidate_topology_quality_get",
    "critic_report_get",
    "production_weapon_form_quality_get",
    "production_weapon_form_quality_v2_get",
    "quality_get",
    "render_evidence_integrity_get",
    "render_evidence_replay_get",
    "render_pass_get",
    "silhouette_candidate_compare",
    "silhouette_evaluation_objective_prepare",
    "silhouette_fit_prepare",
    "silhouette_part_error_get",
    "silhouette_rig_hash",
    "silhouette_target_get",
    "visual_evidence_bundle_get",
)
EXPECTED_QUALITY_NATIVE_READ = ("knife_pass_state_get",)
EXPECTED_QUALITY_READ = EXPECTED_QUALITY_TOOL_READ + EXPECTED_QUALITY_NATIVE_READ
EXPECTED_QUALITY_TOOL_WRITE = (
    "candidate_material_surface_quality_prepare",
    "candidate_topology_quality_prepare",
    "human_visual_review_submit",
    "production_weapon_form_quality_prepare",
    "production_weapon_form_quality_v2_prepare",
    "reference_compare_prepare",
    "visual_review_submit",
)
EXPECTED_QUALITY_NATIVE_WRITE = ("knife_pass_state_prepare",)
EXPECTED_QUALITY_WRITE = EXPECTED_QUALITY_TOOL_WRITE + EXPECTED_QUALITY_NATIVE_WRITE
EXPECTED_JOB_READ = (
    "job_events_read",
    "job_get",
    "job_result_get",
    "optimization_job_get",
)
EXPECTED_JOB_WRITE = (
    "job_cancel",
    "optimization_job_prepare",
    "optimization_job_resume",
    "primary_form_repair_job_prepare",
)

EXPECTED_SILHOUETTE_OPERATIONS = (
    "silhouette_evaluation_objective_prepare",
    "silhouette_fit_prepare",
)
EXPECTED_JOB_OPERATIONS = (
    "job_cancel",
    "job_events_read",
    "job_get",
    "job_result_get",
    "optimization_job_get",
    "optimization_job_prepare",
    "optimization_job_resume",
    "primary_form_repair_job_prepare",
)
EXPECTED_OBSERVE_MAPPING_OPERATIONS = (
    "candidate_get",
    "snapshot_get",
    "scene_observe_get",
)
EXPECTED_KNIFE_PASS_STATE_OPERATIONS = (
    "knife_pass_state_get",
    "knife_pass_state_prepare",
)

JOB_REPOSITORY_METHODS = (
    "get_job",
    "get_job_record",
    "insert_job_with_event",
    "finish_job_with_event",
    "insert_job_with_event_if_absent",
    "update_job_with_event",
    "claim_job_running",
    "requeue_job",
    "insert_job",
    "cancel_job",
    "list_job_events",
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"WPN-ARCH-EVALUATION-001 FAIL: {message}")


def read(path: Path) -> str:
    require(path.is_file(), f"missing {path.relative_to(ROOT)}")
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        raise SystemExit(
            f"WPN-ARCH-EVALUATION-001 FAIL: cannot read {path.relative_to(ROOT)}: {exc}"
        ) from exc


def rust_array(text: str, constant: str) -> tuple[str, ...]:
    pattern = re.compile(
        rf"(?:pub\s+)?const\s+{re.escape(constant)}\s*:\s*&\[&str\]\s*=\s*&\[(?P<body>.*?)\];",
        re.DOTALL,
    )
    match = pattern.search(text)
    require(match is not None, f"missing Rust operation array {constant}")
    return tuple(re.findall(r'"([^"\\]*)"', match.group("body")))


def rust_function(text: str, marker: str) -> str:
    """Return one Rust function body while ignoring nested strings/comments."""

    start = text.find(marker)
    require(start >= 0, f"missing Rust function marker {marker!r}")
    opening = text.find("{", start)
    require(opening >= 0, f"function {marker!r} has no body")

    depth = 0
    state = "code"
    block_depth = 0
    index = opening
    while index < len(text):
        char = text[index]
        next_char = text[index + 1] if index + 1 < len(text) else ""
        if state == "line_comment":
            if char == "\n":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if char == "/" and next_char == "*":
                block_depth += 1
                index += 2
            elif char == "*" and next_char == "/":
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                index += 1
            continue
        if state == "string":
            if char == "\\":
                index += 2
            elif char == '"':
                state = "code"
                index += 1
            else:
                index += 1
            continue
        if state == "char":
            if char == "\\":
                index += 2
            elif char == "'":
                state = "code"
                index += 1
            else:
                index += 1
            continue
        if char == "/" and next_char == "/":
            state = "line_comment"
            index += 2
            continue
        if char == "/" and next_char == "*":
            state = "block_comment"
            block_depth = 1
            index += 2
            continue
        if char == '"':
            state = "string"
            index += 1
            continue
        if char == "'":
            # Rust lifetimes/labels do not start a character literal.
            if not (next_char.isalnum() or next_char == "_"):
                state = "char"
            index += 1
            continue
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text[opening : index + 1]
            require(depth > 0, f"unbalanced braces in function {marker!r}")
        index += 1
    require(False, f"unterminated Rust function {marker!r}")
    return ""


def mapping_block(text: str, capability: str) -> str:
    marker = f'capability: "{capability}"'
    index = text.find(marker)
    require(index >= 0, f"Contract map omits capability {capability}")
    start = text.rfind("WeaponryCapabilityMapping {", 0, index)
    require(start >= 0, f"capability {capability} has no mapping block")
    match = re.search(r"\n    \},\n(?:    WeaponryCapabilityMapping \{|\];)", text[index:])
    require(match is not None, f"capability {capability} mapping block is incomplete")
    return text[start : index + match.start() + len("\n    },")]


def profile_facade(profile: dict[str, object], facade: str) -> dict[str, object]:
    facades = profile.get("facades")
    require(isinstance(facades, dict), "profile.facades must be an object")
    value = facades.get(facade)
    require(isinstance(value, dict), f"profile omits Evaluation façade {facade}")
    return value


def profile_operations(value: dict[str, object], field: str, facade: str) -> tuple[str, ...]:
    raw = value.get(field)
    require(isinstance(raw, list), f"profile facades.{facade}.{field} must be an array")
    require(all(isinstance(item, str) and item for item in raw), f"profile facades.{facade}.{field} has invalid operation")
    return tuple(raw)


def profile_native_operations(
    profile: dict[str, object], facade: str
) -> tuple[tuple[str, ...], tuple[str, ...]]:
    """Return the read/write native operations owned by one façade.

    Native operations are intentionally kept out of a façade's legacy
    ``read_tools``/``write_tools`` arrays.  They are nevertheless active
    Runtime routes and are classified by their closed profile metadata.
    """

    raw = profile.get("native_operations")
    require(isinstance(raw, dict), "profile.native_operations must be an object")
    native_read: list[str] = []
    native_write: list[str] = []
    for operation, metadata in raw.items():
        require(
            isinstance(operation, str) and operation,
            "profile.native_operations contains an invalid operation name",
        )
        require(
            isinstance(metadata, dict),
            f"profile.native_operations.{operation} must be an object",
        )
        if metadata.get("facade_name") != facade:
            continue
        classification = metadata.get("classification")
        require(
            classification in {"read", "write"},
            f"profile.native_operations.{operation} has invalid classification",
        )
        if classification == "read":
            native_read.append(operation)
        else:
            native_write.append(operation)
    return tuple(native_read), tuple(native_write)


def check_profile() -> tuple[tuple[str, ...], tuple[str, ...]]:
    try:
        profile = json.loads(read(PROFILE))
    except json.JSONDecodeError as exc:
        raise SystemExit(f"WPN-ARCH-EVALUATION-001 FAIL: invalid profile JSON: {exc}") from exc
    require(isinstance(profile, dict), "profile must contain an object")

    observe = profile_facade(profile, "observe")
    quality = profile_facade(profile, "quality_review")
    job = profile_facade(profile, "job")
    expected_by_facade = {
        "observe": (EXPECTED_OBSERVE, (), (), ()),
        "quality_review": (
            EXPECTED_QUALITY_TOOL_READ,
            EXPECTED_QUALITY_TOOL_WRITE,
            EXPECTED_QUALITY_NATIVE_READ,
            EXPECTED_QUALITY_NATIVE_WRITE,
        ),
        "job": (EXPECTED_JOB_READ, EXPECTED_JOB_WRITE, (), ()),
    }
    for facade, value in (("observe", observe), ("quality_review", quality), ("job", job)):
        expected_read, expected_write, expected_native_read, expected_native_write = expected_by_facade[facade]
        actual_read = profile_operations(value, "read_tools", facade)
        actual_write = profile_operations(value, "write_tools", facade)
        require(actual_read == expected_read, f"profile {facade}.read_tools drifted")
        require(actual_write == expected_write, f"profile {facade}.write_tools drifted")
        actual_native_read, actual_native_write = profile_native_operations(profile, facade)
        require(
            actual_native_read == expected_native_read,
            f"profile {facade} native read operations drifted",
        )
        require(
            actual_native_write == expected_native_write,
            f"profile {facade} native write operations drifted",
        )
        actual_underlying = profile_operations(value, "underlying_operations", facade)
        expected_underlying = actual_read + actual_write + actual_native_read + actual_native_write
        require(
            len(actual_underlying) == len(set(actual_underlying))
            and set(actual_underlying) == set(expected_underlying),
            f"profile {facade}.underlying_operations does not match read/write ownership",
        )

    facades = profile.get("facades")
    require(isinstance(facades, dict), "profile.facades must be an object")
    actual_facades = tuple(name for name in EXPECTED_FACADES if name in facades)
    require(actual_facades == EXPECTED_FACADES, "Evaluation façade set drifted")
    observe_read = profile_operations(observe, "read_tools", "observe")
    observe_native_read, observe_native_write = profile_native_operations(profile, "observe")
    quality_read = profile_operations(quality, "read_tools", "quality_review")
    quality_write = profile_operations(quality, "write_tools", "quality_review")
    quality_native_read, quality_native_write = profile_native_operations(profile, "quality_review")
    job_read = profile_operations(job, "read_tools", "job")
    job_write = profile_operations(job, "write_tools", "job")
    job_native_read, job_native_write = profile_native_operations(profile, "job")
    combined_read = observe_read + observe_native_read + quality_read + quality_native_read + job_read + job_native_read
    combined_write = quality_write + quality_native_write + job_write + job_native_write
    require(len(combined_read) == 31, "Evaluation read operation count must remain 31")
    require(len(combined_write) == 12, "Evaluation write operation count must remain 12")
    require(len(set(combined_read)) == 31, "Evaluation read operations contain duplicates")
    require(len(set(combined_write)) == 12, "Evaluation write operations contain duplicates")
    require(not set(combined_read).intersection(combined_write), "Evaluation read/write sets overlap")
    require(combined_read == EXPECTED_READ, "Evaluation read operation order/set drifted")
    require(combined_write == EXPECTED_WRITE, "Evaluation write operation order/set drifted")
    return combined_read, combined_write


def check_runtime_service(read_operations: tuple[str, ...], write_operations: tuple[str, ...]) -> None:
    service = read(EVALUATION_SERVICE)
    implementation = service.split("#[cfg(test)]", 1)[0]
    require(
        "pub(crate) const EVALUATION_READ_OPERATIONS" in implementation
        and "pub(crate) const EVALUATION_WRITE_OPERATIONS" in implementation,
        "Evaluation service operation inventory is not Runtime-owned",
    )
    require(
        rust_array(implementation, "EVALUATION_READ_OPERATIONS") == read_operations,
        "Evaluation service read inventory drifted from the locked profile",
    )
    require(
        rust_array(implementation, "EVALUATION_WRITE_OPERATIONS") == write_operations,
        "Evaluation service write inventory drifted from the locked profile",
    )
    require("pub(crate) fn is_evaluation_operation" in implementation, "Evaluation service lacks operation predicate")
    require("pub(crate) fn invoke(" in implementation, "Evaluation service lacks typed invoke entry point")
    for operation in read_operations + write_operations:
        require(f'"{operation}"' in implementation, f"Evaluation service omits {operation}")
    require(
        re.search(r"(?:\.|::)dispatch_ipc\s*\(", implementation) is None,
        "Evaluation service re-enters Runtime::dispatch_ipc",
    )


def check_runtime_router(read_operations: tuple[str, ...], write_operations: tuple[str, ...]) -> None:
    services = read(RUNTIME_SERVICES)
    router = read(RUNTIME_ROUTER)
    runtime_lib = read(RUNTIME_LIB)
    require(
        '#[path = "evaluation_service.rs"]\npub(crate) mod evaluation_service;' in services,
        "Evaluation service is not owned by the Runtime services domain module",
    )
    require(
        "mod evaluation_service;" not in runtime_lib,
        "Evaluation service became a new Runtime root module",
    )
    require(
        "const EVALUATION_READ_OPERATIONS: &[&str] = evaluation_service::EVALUATION_READ_OPERATIONS;"
        in services
        and "const EVALUATION_WRITE_OPERATIONS: &[&str] = evaluation_service::EVALUATION_WRITE_OPERATIONS;"
        in services,
        "Runtime service boundary does not borrow the Evaluation service inventory",
    )
    require(
        "facade_names: EVALUATION_FACADES" in services
        and "read_operations: EVALUATION_READ_OPERATIONS" in services
        and "write_operations: EVALUATION_WRITE_OPERATIONS" in services,
        "Runtime Evaluation boundary is not wired to the three façades",
    )
    require(
        re.search(
            r"runtime_services\s*::\s*\{[^}]*\bevaluation_service\b[^}]*\}",
            router,
            re.DOTALL,
        )
        is not None,
        "typed Runtime router does not import Evaluation service",
    )
    require(
        "WeaponryServiceDomain::Evaluation => {\n                evaluation_service::invoke(self.runtime, operation, payload)\n            }"
        in router,
        "typed Runtime router does not invoke Evaluation service directly",
    )
    require(
        "WeaponryServiceDomain::Evaluation => {\n                evaluation_service::invoke(self.runtime, operation, payload)\n            }"
        in router,
        "Evaluation router branch drifted",
    )

    dispatch = rust_function(runtime_lib, "pub(crate) fn dispatch_ipc")
    require(
        "if runtime_services::evaluation_service::is_evaluation_operation(method) {" in dispatch
        and "return runtime_services::evaluation_service::invoke(self, method, payload);" in dispatch,
        "compatibility IPC does not bridge to the typed Evaluation service",
    )
    old_arm = re.compile(r'"[a-z0-9_]+"(?:\s*\|\s*"[a-z0-9_]+")*\s*=>')
    runtime_arms: set[str] = set()
    for match in old_arm.finditer(runtime_lib):
        runtime_arms.update(re.findall(r'"([a-z0-9_]+)"', match.group(0)))
    for operation in read_operations + write_operations:
        require(
            f'"{operation}"' not in dispatch,
            f"old Evaluation operation arm {operation} remains in Runtime::dispatch_ipc",
        )
        require(
            operation not in runtime_arms,
            f"old Evaluation operation arm {operation} remains in Runtime lib",
        )


def check_store(read_operations: tuple[str, ...], write_operations: tuple[str, ...]) -> None:
    repository = read(EVALUATION_REPOSITORY)
    store_root = read(STORE_ROOT)
    boundaries = read(REPOSITORY_BOUNDARIES)
    require(
        "pub struct EvaluationRepository<'store>" in repository
        and "store: &'store Store" in repository,
        "EvaluationRepository is not a borrowed Store repository",
    )
    require(
        "pub type JobRepository<'store> = EvaluationRepository<'store>;" in repository,
        "JobRepository is not the borrowed Evaluation repository alias",
    )
    require(
        "pub(crate) fn new(store: &'store Store) -> Self" in repository,
        "EvaluationRepository constructor does not borrow Store",
    )
    require(
        "pub mod evaluation_repository;" in store_root
        and "EvaluationRepository, JobEventRecord, JobRecord, JobRepository, JobSummary" in store_root,
        "Store root does not expose the Evaluation/Job repository",
    )
    require(
        "pub fn evaluation_repository(&self) -> EvaluationRepository<'_>" in repository,
        "EvaluationRepository Store accessor is missing",
    )
    require(
        "pub fn job_repository(&self) -> JobRepository<'_>" in repository,
        "JobRepository Store accessor is missing",
    )
    for method in JOB_REPOSITORY_METHODS:
        require(
            re.search(rf"\bpub(?:\(crate\))?\s+fn\s+{re.escape(method)}\s*\(", repository)
            is not None,
            f"EvaluationRepository omits {method}",
        )
        require(
            re.search(rf"self\.evaluation_repository\(\)\s*\.\s*{re.escape(method)}\s*\(", repository)
            is not None,
            f"Store compatibility accessor does not delegate {method} to EvaluationRepository",
        )

    # Job INSERT/UPDATE SQL is allowed in evaluation_repository.rs only.  A
    # Store root may query Job tables for reachability/audit tests, but may not
    # retain a second write implementation.
    sql_strings = re.findall(r'"((?:\\.|[^"\\])*)"', store_root, flags=re.DOTALL)
    job_write_strings = []
    for sql in sql_strings:
        normalised = " ".join(sql.upper().split())
        if re.search(r"\b(?:INSERT|UPDATE)\b", normalised) and re.search(
            r"\bRUNTIME_JOB(?:S|_EVENTS|_CHECKPOINTS)\b", normalised
        ):
            job_write_strings.append(normalised)
    require(not job_write_strings, "Job INSERT/UPDATE SQL remains in Store root")
    require(
        "evaluation_repository::insert_job_and_event_in_transaction(" in store_root,
        "cross-domain Store transactions do not borrow the Evaluation Job SQL helper",
    )

    require(
        "const EVALUATION_REPOSITORY_EXTRACTED_RECORD_FAMILIES" in boundaries,
        "Store boundary does not record the extracted Job aggregate",
    )
    for family in (
        "JobRecord / JobSummary (runtime_jobs)",
        "JobEventRecord (runtime_job_events)",
        "Job checkpoint bindings (runtime_job_checkpoints)",
    ):
        require(f'"{family}"' in boundaries, f"Store boundary omits extracted family {family}")
    require(
        "src/evaluation_repository.rs (borrowed Job/Event/Checkpoint aggregate façade)" in boundaries,
        "Store Evaluation implementation boundary is missing",
    )
    require(
        "src/lib.rs (subdivision, quality, observe, visual and remaining evaluation compatibility)" in boundaries,
        "Store Evaluation root concentration is not explicit",
    )


def check_contract_map(read_operations: tuple[str, ...], write_operations: tuple[str, ...]) -> None:
    contract_map = read(CONTRACT_MAP)
    require(
        rust_array(contract_map, "SILHOUETTE_OPERATIONS") == EXPECTED_SILHOUETTE_OPERATIONS,
        "central silhouette operation mapping drifted",
    )
    require(
        rust_array(contract_map, "JOB_OPERATIONS") == EXPECTED_JOB_OPERATIONS,
        "central Job operation mapping drifted",
    )
    require(
        rust_array(contract_map, "OBSERVE_OPERATIONS") == EXPECTED_OBSERVE_MAPPING_OPERATIONS,
        "central observe mapping drifted",
    )
    require(
        rust_array(contract_map, "KNIFE_PASS_STATE_OPERATIONS")
        == EXPECTED_KNIFE_PASS_STATE_OPERATIONS,
        "central KnifePassState operation mapping drifted",
    )
    profile_operations_set = set(read_operations + write_operations)
    require(
        set(EXPECTED_SILHOUETTE_OPERATIONS).issubset(profile_operations_set)
        and set(EXPECTED_JOB_OPERATIONS).issubset(profile_operations_set),
        "central Evaluation mappings contain operations outside the profile",
    )

    silhouette = mapping_block(contract_map, "silhouette_evaluation")
    require("domain: WeaponryServiceDomain::Evaluation" in silhouette, "silhouette mapping has wrong domain")
    require('contract: Some("SilhouetteEvaluationObjective@1")' in silhouette, "silhouette contract drifted")
    require('runtime_service: Some("silhouette::{objective,fit}")' in silhouette, "silhouette Runtime owner drifted")
    require("store_record: None" in silhouette and "persistence: PersistenceKind::None" in silhouette, "silhouette must remain non-durable")
    require('mcp_facade: Some("quality_review")' in silhouette, "silhouette is not owned by quality_review")
    require("mcp_operations: SILHOUETTE_OPERATIONS" in silhouette, "silhouette mapping does not use central operation array")
    require("status: MappingStatus::Complete" in silhouette, "silhouette mapping is not complete")

    pass_state = mapping_block(contract_map, "knife_pass_state")
    require("domain: WeaponryServiceDomain::Evaluation" in pass_state, "KnifePassState mapping has wrong domain")
    require('contract: Some("KnifePassState@1")' in pass_state, "KnifePassState contract drifted")
    require(
        'runtime_service: Some("evaluation_service::knife_pass_state::{prepare,get}")'
        in pass_state,
        "KnifePassState Runtime owner drifted",
    )
    require(
        'store_record: Some("KnifePassStateStoreRecord")' in pass_state,
        "KnifePassState mapping does not name its Store record",
    )
    require(
        "persistence: PersistenceKind::DurableTransaction" in pass_state,
        "KnifePassState mapping is not durable",
    )
    require(
        'mcp_facade: Some("quality_review")' in pass_state,
        "KnifePassState is not owned by quality_review",
    )
    require(
        "mcp_operations: KNIFE_PASS_STATE_OPERATIONS" in pass_state,
        "KnifePassState mapping does not use central operation array",
    )
    require("status: MappingStatus::Complete" in pass_state, "KnifePassState mapping is not complete")

    job = mapping_block(contract_map, "runtime_job_lifecycle")
    require("domain: WeaponryServiceDomain::Evaluation" in job, "Job mapping has wrong domain")
    require('contract: Some("RuntimeJob@1")' in job, "Job contract drifted")
    require('runtime_service: Some("evaluation_service::runtime_job_lifecycle")' in job, "Job Runtime owner drifted")
    require('store_record: Some("JobRecord")' in job, "Job mapping does not name JobRecord")
    require("persistence: PersistenceKind::DurableTransaction" in job, "Job mapping is not durable")
    require('mcp_facade: Some("job")' in job, "Job mapping is not owned by job façade")
    require("mcp_operations: JOB_OPERATIONS" in job, "Job mapping does not use central operation array")
    require("status: MappingStatus::Partial" in job, "Job mapping must retain partial status for remaining Evaluation gaps")

    observe = mapping_block(contract_map, "observe_read_model")
    require("domain: WeaponryServiceDomain::Evaluation" in observe, "observe mapping has wrong domain")
    require('runtime_service: Some("observe::{read_model}")' in observe, "observe Runtime owner drifted")
    require('store_record: Some("EvaluationReadModelProjection")' in observe, "observe mapping lost ReadModel marker")
    require("persistence: PersistenceKind::Projection" in observe, "observe mapping lost projection semantics")
    require('mcp_facade: Some("observe")' in observe, "observe mapping has wrong façade")
    require("status: MappingStatus::Partial" in observe, "observe ReadModel gap was incorrectly closed")


def check_remaining_gaps() -> None:
    boundaries = read(REPOSITORY_BOUNDARIES)
    require(
        "pub const EVALUATION_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES" in boundaries,
        "remaining Evaluation repository gaps are not declared",
    )
    for gap in (
        "ReadModel project/candidate/snapshot projections",
        "QualityEvidence form/topology/material/animation records",
    ):
        require(f'"{gap}"' in boundaries, f"remaining Evaluation gap is missing: {gap}")
    require(
        "ownership_seam_only:quality_evidence_evaluation_owner;strict_mapping_gap" in boundaries,
        "quality_review does not retain the QualityEvidence mapping gap",
    )
    require(
        "ownership_seam_only:read_only_evaluation_projection" in boundaries,
        "observe does not retain the ReadModel projection gap",
    )


def main() -> int:
    read_operations, write_operations = check_profile()
    check_runtime_service(read_operations, write_operations)
    check_runtime_router(read_operations, write_operations)
    check_store(read_operations, write_operations)
    check_contract_map(read_operations, write_operations)
    check_remaining_gaps()
    print(
        json.dumps(
            {
                "schema_version": "WeaponryEvaluationArchitectureCheck@1",
                "status": "PASS",
                "evaluation_facades": list(EXPECTED_FACADES),
                "active_evaluation_operations": len(read_operations) + len(write_operations),
                "active_read_operations": len(read_operations),
                "active_write_operations": len(write_operations),
                "runtime_router": "typed_evaluation_service",
                "compatibility_bridge": "evaluation_service_reused",
                "store_repository": "borrowed_evaluation_job_repository",
                "central_mappings": {
                    "observe_read_model": "partial_projection_gap",
                    "silhouette_evaluation": "complete_non_durable",
                    "runtime_job_lifecycle": "partial_durable_job_record",
                },
                "remaining_gaps": ["ReadModel", "QualityEvidence"],
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
