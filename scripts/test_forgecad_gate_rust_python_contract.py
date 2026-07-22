from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "forgecad_gate_rust_python_contract.py"
sys.path.insert(0, str(ROOT / "scripts"))

import forgecad_gate_rust_python_contract as gate  # noqa: E402


def test_self_test_fault_injection_is_stable_and_json_safe() -> None:
    report = gate.run_fault_injection_self_tests()
    assert report["schema_version"] == "ForgeCADRustPythonContractSelfTest@1"
    assert report["phase"] == gate.PHASE
    assert report["subsystem"] == gate.SUBSYSTEM
    assert report["result"] == "passed"
    assert report["exit_code"] == 0
    encoded = json.dumps(report, ensure_ascii=False)
    assert "glb_base64" not in encoded
    assert "prompt" not in encoded
    assert "secret" not in encoded


def test_material_drift_fault_is_not_silently_accepted() -> None:
    result = gate.validate_material_catalog_contract(inject_drift=True)
    assert result["result"] == "failed"
    assert result["stable_error_code"] == "MATERIAL_CATALOG_DRIFT"


def test_shape_program_hash_drift_fault_is_not_silently_accepted() -> None:
    result = gate.validate_shape_program_contract(inject_hash_drift=True)
    assert result["result"] == "failed"
    assert result["stable_error_code"] == "SHAPE_PROGRAM_SEMANTIC_HASH_DRIFT"


def test_missing_source_face_accessor_fault_is_fail_closed() -> None:
    try:
        gate.verify_source_face_provenance(
            {"meshes": [{"primitives": [{"attributes": {"POSITION": 0}}]}], "accessors": [], "bufferViews": []},
            b"",
        )
    except gate.GateContractFailure as error:
        assert error.code == "GLB_SOURCE_FACE_ACCESSOR_MISSING"
    else:
        raise AssertionError("missing source-face accessor was accepted")


def test_cli_self_test_emits_only_the_versioned_json_report() -> None:
    completed = subprocess.run(
        [sys.executable, str(SCRIPT), "--self-test"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert completed.returncode == 0
    report = json.loads(completed.stdout)
    assert report["schema_version"] == "ForgeCADRustPythonContractSelfTest@1"
    assert report["result"] == "passed"
    assert completed.stderr == ""
