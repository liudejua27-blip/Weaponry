#!/usr/bin/env python3
"""Focused closed-contract and source-evidence gate for AuthoringMesh transactions.

This checker intentionally validates only the public envelope and the pure
kernel's three-command journal.  It does not exercise Store, Runtime durability
or MCP transport.  Those layers consume these schemas in their own focused
gates.  Cross-field ordering and generated-reference rules are checked here
because JSON Schema cannot compare two indexes or parallel array lengths.  The
WPN-AUTH source receipt is also rebound to the exact Runtime/Store/MCP/schema
files so the receipt cannot silently outlive an implementation change.
"""

from __future__ import annotations

import copy
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "forgecad-contracts" / "schemas"
MANIFEST_PATH = ROOT / "packages" / "forgecad-contracts" / "manifest.json"
EVIDENCE_PATH = (
    ROOT
    / "docs"
    / "evidence"
    / "weaponry"
    / "wpn-auth-001-authoring-mesh-transaction-current-source-gate-20260829.json"
)
TRUTH_PATH = ROOT / "docs" / "evidence" / "mcp010f" / "current-benchmark-truth.json"
SUMMARY_PATH = ROOT / "docs" / "evidence" / "mcp010f" / "source-tool-manifest-summary.json"

IMPLEMENTATION_SOURCES = {
    "runtime_authoring_mesh_transaction_rs": ROOT
    / "apps/desktop/src-tauri/crates/forgecad-runtime/src/authoring_mesh_transaction.rs",
    "store_authoring_mesh_v2_transaction_rs": ROOT
    / "apps/desktop/src-tauri/crates/forgecad-store/src/authoring_mesh_v2_transaction.rs",
    "mcp_authoring_mesh_transaction_tools_rs": ROOT
    / "apps/desktop/src-tauri/crates/forgecad-mcp/src/authoring_mesh_transaction_tools.rs",
    "journal_schema_json": SCHEMA_ROOT / "authoring-mesh-transaction.schema.json",
    "result_schema_json": SCHEMA_ROOT / "authoring-mesh-transaction-result.schema.json",
}

EXPECTED = {
    "authoring-mesh-transaction.schema.json": "AuthoringMeshTransaction@1",
    "authoring-mesh-transaction-prepare-request.schema.json": "AuthoringMeshTransactionPrepareRequest@1",
    "authoring-mesh-transaction-get-request.schema.json": "AuthoringMeshTransactionGetRequest@1",
    "authoring-mesh-transaction-result.schema.json": "AuthoringMeshTransactionResult@1",
}
OPERATIONS = {"split_edge", "move_vertices", "face_extrude"}
ELEMENT_KINDS = {"vertex", "edge", "half_edge", "corner", "face", "loop", "ring"}
HASH_A = "a" * 64
HASH_B = "b" * 64
HASH_C = "c" * 64
HASH_D = "d" * 64
HASH_E = "e" * 64


def fail(message: str) -> None:
    raise SystemExit(f"AuthoringMesh transaction contract violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot load {path.relative_to(ROOT)}: {exc}")
    require(isinstance(value, dict), f"{path.name} must be a JSON object")
    return value


def sha256_file(path: Path) -> str:
    require(path.is_file(), f"missing source evidence input: {path.relative_to(ROOT)}")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_source_evidence_binding() -> None:
    evidence = load_json(EVIDENCE_PATH)
    truth = load_json(TRUTH_PATH)
    summary = load_json(SUMMARY_PATH)
    require(evidence.get("task_id") == "WPN-AUTH-001", "source receipt task id drifted")
    require(
        evidence.get("status") == "PASS_SOURCE_STRUCTURAL_DURABLE_TRANSACTION",
        "source receipt status drifted",
    )
    require(evidence.get("historical_receipts_mutated") is False, "source receipt rewrites history")
    implementation_hashes = evidence.get("implementation_source_sha256")
    require(isinstance(implementation_hashes, dict), "source receipt implementation hashes missing")
    require(
        set(implementation_hashes) == set(IMPLEMENTATION_SOURCES),
        "source receipt implementation hash key set drifted",
    )
    for key, path in IMPLEMENTATION_SOURCES.items():
        require(
            implementation_hashes[key] == sha256_file(path),
            f"source receipt implementation hash drifted: {key}",
        )

    current = evidence.get("current_source")
    require(isinstance(current, dict), "source receipt current_source missing")
    truth_current = truth.get("current_source", {})
    truth_contracts = truth_current.get("contracts", {})
    truth_tools = truth_current.get("mcp_tools", {})
    expected = {
        "schema_count": truth_contracts.get("schema_count"),
        "read_tool_count": truth_tools.get("read_count"),
        "write_tool_count": truth_tools.get("write_count"),
        "total_tool_count": truth_tools.get("total_count"),
        "contract_manifest_sha256": truth_contracts.get("manifest_sha256"),
        "schema_content_set_sha256": truth_contracts.get("schema_content_set_sha256"),
        "runtime_source_sha256": truth_current.get("visible_view_policy", {}).get(
            "runtime_source_sha256"
        ),
        "compiled_summary_sha256": truth_tools.get("summary_receipt_sha256"),
        "build_cohort_sha256": None,
    }
    require(current == expected, "source receipt current cohort projection drifted")
    require(summary.get("build_cohort_sha256") is None, "source summary is not source-only")
    require(summary.get("read_count") == current["read_tool_count"], "receipt read count drifted")
    require(summary.get("write_count") == current["write_tool_count"], "receipt write count drifted")
    require(summary.get("total_count") == current["total_tool_count"], "receipt total count drifted")


def walk_property_names(node: Any) -> list[str]:
    if not isinstance(node, dict):
        return []
    names: list[str] = []
    properties = node.get("properties")
    if isinstance(properties, dict):
        names.extend(properties)
        for child in properties.values():
            names.extend(walk_property_names(child))
    for key in ("$defs", "items", "allOf", "anyOf", "oneOf", "not", "if", "then", "else"):
        child = node.get(key)
        if isinstance(child, list):
            for value in child:
                names.extend(walk_property_names(value))
        elif isinstance(child, dict):
            names.extend(walk_property_names(child))
    return names


def load_contracts() -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]]]:
    manifest = load_json(MANIFEST_PATH)
    declared = set(manifest.get("schemas", []))
    for filename in EXPECTED:
        require(filename in declared, f"manifest omits {filename}")
    schemas = {filename: load_json(SCHEMA_ROOT / filename) for filename in EXPECTED}
    for filename, version in EXPECTED.items():
        schema = schemas[filename]
        require(
            schema.get("$id") == f"https://forgecad.local/contracts/{filename}",
            f"{filename} has an unexpected $id",
        )
        require(schema.get("title") == version, f"{filename} has title {schema.get('title')!r}")
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False,
            f"{filename} root is not closed",
        )
        properties = schema.get("properties", {})
        require(
            schema.get("required") == list(properties),
            f"{filename} must require every public property exactly once",
        )
        for name in walk_property_names(schema):
            require(
                name.lower()
                not in {
                    "path",
                    "url",
                    "uri",
                    "raw",
                    "raw_bytes",
                    "bytes",
                    "secret",
                    "token",
                    "password",
                    "api_key",
                    "prompt",
                    "script",
                    "shell",
                    "environment",
                },
                f"{filename} exposes forbidden property {name}",
            )

    # Reuse the repository's standard draft-2020-12 subset validator for
    # positive/negative fixtures, including the journal's external $ref.
    sys.path.insert(0, str(ROOT / "scripts"))
    from check_agentic_contracts import is_valid, load_schema_registry  # type: ignore

    registry = load_schema_registry(manifest)
    return schemas, {"registry": registry, "is_valid": is_valid}


def stable_ref(kind: str, identifier: str) -> dict[str, str]:
    return {"kind": kind, "id": identifier}


def generated_ref(kind: str, command_index: int, output_index: int) -> dict[str, Any]:
    return {
        "kind": kind,
        "command_index": command_index,
        "output_index": output_index,
    }


def journal_fixture() -> dict[str, Any]:
    return {
        "schema_version": "AuthoringMeshTransaction@1",
        "transaction_id": "tx-1",
        "mesh_id": "mesh-1",
        "lineage_id": "lineage-1",
        "base_revision_id": "rev-0",
        "base_revision_index": 0,
        "base_revision_sha256": HASH_A,
        "commands": [
            {
                "command_index": 0,
                "operation": "split_edge",
                "operation_id": "split-1",
                "edge": stable_ref("edge", "edge-1"),
                "split_ratio_milli": 500,
                "operation_lineage_sha256": HASH_B,
            },
            {
                "command_index": 1,
                "operation": "move_vertices",
                "operation_id": "move-1",
                "vertices": [generated_ref("vertex", 0, 0)],
                "delta_m": [[0.0, 0.0, 0.125]],
                "operation_lineage_sha256": HASH_C,
            },
            {
                "command_index": 2,
                "operation": "face_extrude",
                "operation_id": "extrude-1",
                "face": stable_ref("face", "face-1"),
                "distance_m": 0.25,
                "operation_lineage_sha256": HASH_D,
            },
        ],
        "budgets": {
            "max_commands": 32,
            "max_move_vertices_per_command": 32,
            "max_face_degree": 32,
            "max_vertex_delta_m": 1,
            "max_face_extrude_distance_m": 10,
            "overflow_policy": "reject-entire-transaction@1",
        },
        "execution_policy": {
            "writer_policy": "forgecad-runtime-only-state-writer@1",
            "source_of_truth": "original-authoring-mesh@2",
            "reference_policy": "stable-or-earlier-generated-element-by-kind@1",
            "atomicity_policy": "clone-before-first-command-no-partial-result@1",
            "replay_policy": "same-input-same-base-deterministic-revision-chain@1",
            "evaluation_policy": "authored-edit-invalidates-evaluated-sidecar@2",
            "identity_policy": "runtime-derived-lineage-operation-parent-stable-no-reuse@2",
        },
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "canonical_sha256": HASH_E,
    }


def prepare_fixture(journal: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": "AuthoringMeshTransactionPrepareRequest@1",
        "project_id": "project-1",
        "transaction": journal,
        "idempotency_key": "idem-1",
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "input_sha256": HASH_A,
    }


def get_fixture() -> dict[str, Any]:
    return {
        "schema_version": "AuthoringMeshTransactionGetRequest@1",
        "project_id": "project-1",
        "transaction_id": "tx-1",
        "transaction_sha256": HASH_E,
        "transaction_object_sha256": HASH_D,
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "writer_policy": "forgecad-runtime-only-state-writer@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "input_sha256": HASH_D,
    }


def check_journal_semantics(transaction: dict[str, Any]) -> None:
    commands = transaction["commands"]
    require([command["command_index"] for command in commands] == list(range(len(commands))), "command indexes must be contiguous and ordered")
    operation_ids = [command["operation_id"] for command in commands]
    require(len(operation_ids) == len(set(operation_ids)), "operation_id values must be unique")

    def check_ref(ref: dict[str, Any], command_index: int, expected_kind: str) -> None:
        require(ref.get("kind") == expected_kind, f"command {command_index} reference kind must be {expected_kind}")
        if "command_index" in ref:
            require(ref["command_index"] < command_index, "generated reference must point to an earlier command")
            require(ref["command_index"] >= 0, "generated reference command index must be non-negative")
            require(ref["output_index"] >= 0, "generated reference output index must be non-negative")

    for command_index, command in enumerate(commands):
        operation = command["operation"]
        require(operation in OPERATIONS, f"command {command_index} exposes an unavailable operation")
        if operation == "split_edge":
            check_ref(command["edge"], command_index, "edge")
        elif operation == "move_vertices":
            vertices = command["vertices"]
            deltas = command["delta_m"]
            require(len(vertices) == len(deltas), f"command {command_index} vertices/delta_m must be parallel")
            require(any(any(value != 0 for value in delta) for delta in deltas), f"command {command_index} must move at least one coordinate")
            encoded = [json.dumps(ref, sort_keys=True) for ref in vertices]
            require(len(encoded) == len(set(encoded)), f"command {command_index} repeats a vertex reference")
            for ref in vertices:
                check_ref(ref, command_index, "vertex")
        else:
            check_ref(command["face"], command_index, "face")


def result_fixture(request_kind: str) -> dict[str, Any]:
    chain = [
        {
            "command_index": index,
            "operation_id": operation_id,
            "operation": operation,
            "parent_revision_id": f"rev-{index}",
            "revision_id": f"rev-{index + 1}",
            "revision_index": index + 1,
            "revision_sha256": [HASH_B, HASH_C, HASH_D][index],
            "revision_object_sha256": [HASH_C, HASH_D, HASH_E][index],
            "readback_sha256": [HASH_D, HASH_E, HASH_A][index],
        }
        for index, (operation_id, operation) in enumerate(
            [("split-1", "split_edge"), ("move-1", "move_vertices"), ("extrude-1", "face_extrude")]
        )
    ]
    steps = [
        {
            "command_index": item["command_index"],
            "operation_id": item["operation_id"],
            "operation": item["operation"],
            "parent_revision_id": item["parent_revision_id"],
            "child_revision_id": item["revision_id"],
            "child_revision_sha256": item["revision_sha256"],
            "child_revision_object_sha256": item["revision_object_sha256"],
            "changed_elements": [stable_ref("vertex", f"v-{item['command_index']}")],
            "generated_elements": [stable_ref("vertex", f"generated-{item['command_index']}")],
            "retired_elements": [],
            "readback_sha256": item["readback_sha256"],
        }
        for item in chain
    ]
    return {
        "schema_version": "AuthoringMeshTransactionResult@1",
        "request_kind": request_kind,
        "status": "prepared" if request_kind == "prepare" else "found",
        "project_id": "project-1",
        "transaction_id": "tx-1",
        "transaction_sha256": HASH_E,
        "transaction_object_sha256": HASH_D,
        "mesh_id": "mesh-1",
        "lineage_id": "lineage-1",
        "base_revision_id": "rev-0",
        "base_revision_index": 0,
        "base_revision_sha256": HASH_A,
        "final_revision_id": "rev-3",
        "final_revision_index": 3,
        "final_revision_sha256": HASH_D,
        "final_revision_object_sha256": HASH_E,
        "revision_chain": chain,
        "steps": steps,
        "readback": {
            "status": "passed",
            "revision_sha256": HASH_D,
            "revision_object_sha256": HASH_E,
            "readback_sha256": HASH_A,
            "topology_validation_status": "passed",
            "deterministic_replay": True,
            "byte_exact_revision_replay": True,
            "restart_hash_verified": True,
            "partial_result_exposed": False,
        },
        "replayed": False,
        "idempotency_key": "idem-1" if request_kind == "prepare" else None,
        "atomicity_status": "committed",
        "source_revision_unchanged": True,
        "revision_chain_persisted": True,
        "partial_result_exposed": False,
        "store_commit_status": "committed" if request_kind == "prepare" else "not-touched",
        "cas_commit_status": "committed" if request_kind == "prepare" else "not-touched",
        "runtime_write_performed": request_kind == "prepare",
        "persistent_user_data_touched": request_kind == "prepare",
        "stage_advanced": False,
        "candidate_confirmed": False,
        "version_created": False,
        "export_performed": False,
        "quality_status": "structural_only",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
        "canonical_sha256": HASH_B,
    }


def check_result_semantics(result: dict[str, Any]) -> None:
    chain = result["revision_chain"]
    steps = result["steps"]
    require([item["command_index"] for item in chain] == list(range(len(chain))), "result revision chain indexes must be ordered")
    require([item["command_index"] for item in steps] == list(range(len(steps))), "result step indexes must be ordered")
    require(chain[-1]["revision_id"] == result["final_revision_id"], "final revision must be the last chain revision")
    require(chain[-1]["revision_sha256"] == result["final_revision_sha256"], "final revision hash must match the last chain revision")
    require(result["readback"]["revision_sha256"] == result["final_revision_sha256"], "readback revision hash must match final revision")
    require(result["readback"]["revision_object_sha256"] == result["final_revision_object_sha256"], "readback object hash must match final revision object")
    require(result["readback"]["partial_result_exposed"] is False, "result must not expose a partial chain")
    for index, step in enumerate(steps):
        require(step["child_revision_id"] == chain[index]["revision_id"], f"step {index} child must match chain")
        expected_parent = result["base_revision_id"] if index == 0 else chain[index - 1]["revision_id"]
        require(step["parent_revision_id"] == expected_parent, f"step {index} parent must match ordered chain")


def main() -> int:
    check_source_evidence_binding()
    schemas, validator = load_contracts()
    is_valid = validator["is_valid"]
    registry = validator["registry"]
    journal = journal_fixture()
    require(is_valid(schemas["authoring-mesh-transaction.schema.json"], journal, registry), "positive journal rejected")
    check_journal_semantics(journal)

    prepare = prepare_fixture(journal)
    require(is_valid(schemas["authoring-mesh-transaction-prepare-request.schema.json"], prepare, registry), "positive prepare request rejected")
    unknown = copy.deepcopy(prepare)
    unknown["path"] = "forbidden"
    require(not is_valid(schemas["authoring-mesh-transaction-prepare-request.schema.json"], unknown, registry), "prepare accepted unknown/path field")

    unavailable = copy.deepcopy(journal)
    unavailable["commands"][0]["operation"] = "collapse_edge"
    require(not is_valid(schemas["authoring-mesh-transaction.schema.json"], unavailable, registry), "journal accepted unavailable operation")

    forward = copy.deepcopy(journal)
    forward["commands"][1]["vertices"][0]["command_index"] = 2
    require(is_valid(schemas["authoring-mesh-transaction.schema.json"], forward, registry), "schema fixture unexpectedly rejected structurally valid forward reference")
    try:
        check_journal_semantics(forward)
    except SystemExit:
        pass
    else:
        fail("forward generated reference was accepted by cross-field checker")

    mismatch = copy.deepcopy(journal)
    mismatch["commands"][1]["delta_m"] = [[0.0, 0.0, 0.1], [0.0, 0.0, 0.1]]
    require(is_valid(schemas["authoring-mesh-transaction.schema.json"], mismatch, registry), "schema fixture unexpectedly rejected structurally valid parallel-array mismatch")
    try:
        check_journal_semantics(mismatch)
    except SystemExit:
        pass
    else:
        fail("parallel vertices/delta_m mismatch was accepted by cross-field checker")

    get_request = get_fixture()
    require(is_valid(schemas["authoring-mesh-transaction-get-request.schema.json"], get_request, registry), "positive get request rejected")
    bad_get = copy.deepcopy(get_request)
    bad_get["runtime_write_performed"] = True
    require(not is_valid(schemas["authoring-mesh-transaction-get-request.schema.json"], bad_get, registry), "get request accepted runtime write")

    prepare_result = result_fixture("prepare")
    require(is_valid(schemas["authoring-mesh-transaction-result.schema.json"], prepare_result, registry), "positive prepare result rejected")
    check_result_semantics(prepare_result)
    replay_result = copy.deepcopy(prepare_result)
    replay_result["status"] = "replayed"
    replay_result["replayed"] = True
    replay_result["store_commit_status"] = "not-touched"
    replay_result["cas_commit_status"] = "not-touched"
    replay_result["runtime_write_performed"] = False
    replay_result["persistent_user_data_touched"] = False
    require(is_valid(schemas["authoring-mesh-transaction-result.schema.json"], replay_result, registry), "positive replay result rejected")
    check_result_semantics(replay_result)
    get_result = result_fixture("get")
    require(is_valid(schemas["authoring-mesh-transaction-result.schema.json"], get_result, registry), "positive get result rejected")
    check_result_semantics(get_result)

    invalid_result = copy.deepcopy(prepare_result)
    invalid_result["status"] = "rejected"
    require(not is_valid(schemas["authoring-mesh-transaction-result.schema.json"], invalid_result, registry), "result accepted an undeclared rejected status")

    runtime_error = load_json(SCHEMA_ROOT / "runtime-error.schema.json")
    require(runtime_error.get("additionalProperties") is False, "RuntimeError must remain closed")
    require(set(runtime_error.get("required", [])) >= {"code", "message", "retryable", "next_action", "evidence_ids"}, "RuntimeError lost typed error fields")
    print("AuthoringMesh transaction contracts/evidence OK: journal, envelopes, result, ordering, negative fixtures and exact source hashes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
