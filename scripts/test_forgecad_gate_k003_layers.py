from __future__ import annotations

import json
from pathlib import Path
import sys

import pytest

import forgecad_gate_k003_layers as gate


def _report(layer: str, *, status: str = "passed", exit_code: int = 0, schema: str | None = None) -> dict:
    value = {
        "schema_version": schema or gate.LAYER_REPORT_SCHEMAS[layer],
        "status": status,
        "exit_code": exit_code,
        "stable_error_code": None if status in {"pass", "passed"} else "TEST_FAILURE",
    }
    value.update(gate.LAYER_CONTRACT_FIELDS[layer])
    if layer == "host":
        value["ok"] = status in {"passed", "pass"}
    if layer == "packaged":
        value["ok"] = status in {"passed", "pass"}
    return value


def _command(payload: dict, exit_code: int = 0) -> list[str]:
    source = "import json,sys; print(json.dumps(%r)); sys.exit(%d)" % (payload, exit_code)
    return [sys.executable, "-c", source]


def test_layer_order_and_report_schemas_are_explicit_and_unique() -> None:
    assert gate.LAYER_ORDER == ("host", "rust_core", "rust_python_contract", "packaged", "workbench")
    assert tuple(gate.LAYER_ORDER) == tuple(gate.LAYER_REPORT_SCHEMAS)
    assert len(set(gate.LAYER_REPORT_SCHEMAS.values())) == len(gate.LAYER_REPORT_SCHEMAS)


def test_run_layer_rejects_wrong_schema(tmp_path: Path) -> None:
    result = gate._run_layer(
        "host", _command(_report("host", schema="Wrong@1")), env=dict(), timeout_seconds=2,
        report_path=tmp_path / "host.json",
    )
    assert result["stable_error_code"] == "LAYER_SCHEMA_MISMATCH"
    assert result["child_returncode"] == 0


def test_run_layer_rejects_nonzero_with_pass_report(tmp_path: Path) -> None:
    result = gate._run_layer(
        "host", _command(_report("host"), exit_code=7), env=dict(), timeout_seconds=2,
        report_path=tmp_path / "host.json",
    )
    assert result["stable_error_code"] == "LAYER_NONZERO_WITHOUT_FAILURE_REPORT"
    assert result["exit_code"] == 7


def test_run_layer_rejects_zero_with_failed_report(tmp_path: Path) -> None:
    result = gate._run_layer(
        "host", _command(_report("host", status="failed", exit_code=0)), env=dict(), timeout_seconds=2,
        report_path=tmp_path / "host.json",
    )
    assert result["stable_error_code"] == "LAYER_STATUS_EXIT_MISMATCH"


def test_run_layer_timeout_and_missing_report_are_stable(tmp_path: Path) -> None:
    timeout = gate._run_layer(
        "host", [sys.executable, "-c", "import time; time.sleep(1)"], env=dict(), timeout_seconds=0.01,
        report_path=tmp_path / "timeout.json",
    )
    missing = gate._run_layer(
        "host", [sys.executable, "-c", "print('not-json')"], env=dict(), timeout_seconds=2,
        report_path=tmp_path / "missing.json",
    )
    assert timeout["stable_error_code"] == "LAYER_TIMEOUT"
    assert missing["stable_error_code"] == "LAYER_REPORT_INVALID"


def test_external_artifact_directory_uses_stable_redacted_identifier(tmp_path: Path) -> None:
    assert gate._path_for_report(gate.ROOT / "output" / "inside") == "output/inside"
    external = tmp_path / "outside" / "layer"
    first = gate._path_for_report(external)
    assert first == gate._path_for_report(external)
    assert first.startswith("external-artifact/layer-")
    assert str(tmp_path) not in first
    assert "/Users/" not in first
    assert "/private/" not in first


def test_external_artifact_run_redacts_report_and_manifest_paths(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    artifact_dir = tmp_path / "outside" / "layer"
    monkeypatch.setattr(gate, "_source_fingerprint", lambda: {"sha256": "stable"})
    monkeypatch.setattr(gate, "_require_packaged_artifacts", lambda: {"sidecar": {"path": "sidecar", "sha256": "a"}, "app_executable": {"path": "app", "sha256": "b"}})
    monkeypatch.setattr(gate, "_bind_packaged_artifacts", lambda report: {**report, "artifacts": {"sidecar": {"path": "sidecar", "sha256": "a"}, "app_executable": {"path": "app", "sha256": "b"}}})
    monkeypatch.setattr(gate, "_run_layer", lambda layer, *args, **kwargs: _report(layer))
    monkeypatch.setattr(gate, "_run_workbench_layer", lambda **kwargs: {**_report("workbench"), "ok": True, "status": "passed"})

    assert gate.run_gate(artifact_dir, 1) == 0

    serialized = (artifact_dir / "report.json").read_text(encoding="utf-8") + (artifact_dir / "manifest.json").read_text(encoding="utf-8")
    assert str(tmp_path) not in serialized
    assert "/Users/" not in serialized
    assert "/private/" not in serialized
    report = json.loads((artifact_dir / "report.json").read_text(encoding="utf-8"))
    manifest = json.loads((artifact_dir / "manifest.json").read_text(encoding="utf-8"))
    assert report["manifest"].startswith("external-artifact/manifest.json-")
    assert manifest["report_directory"].startswith("external-artifact/layer-")


def test_missing_packaged_artifact_is_fail_closed(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    paths = {"sidecar": tmp_path / "sidecar", "app_bundle": tmp_path / "app.app", "app_executable": tmp_path / "app.app" / "bin"}
    monkeypatch.setattr(gate, "_packaged_artifact_paths", lambda: paths)
    with pytest.raises(gate.GateFailure) as error:
        gate._require_packaged_artifacts()
    assert error.value.stable_error_code == "PACKAGED_SIDECAR_MISSING"
    paths["sidecar"].write_bytes(b"sidecar")
    with pytest.raises(gate.GateFailure) as error:
        gate._require_packaged_artifacts()
    assert error.value.stable_error_code == "PACKAGED_APP_MISSING"


def test_source_fingerprint_covers_staged_and_untracked_content_and_deletion(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    source_root = tmp_path / "workspace"
    source_root.mkdir()
    untracked = source_root / "fixture.py"
    untracked.write_text("one", encoding="utf-8")
    monkeypatch.setattr(gate, "ROOT", source_root)
    state = {"untracked": b"fixture.py\0", "staged": b"staged-one"}

    def fake_git(*args: str) -> bytes:
        if args[:2] == ("rev-parse", "HEAD"):
            return b"head\n"
        if args[:2] == ("status", "--porcelain=v1"):
            return b"?? fixture.py\0"
        if args[:2] == ("ls-files", "--others"):
            return state["untracked"]
        if "--cached" in args:
            return state["staged"]
        return b"unstaged"

    monkeypatch.setattr(gate, "_git_bytes", fake_git)
    first = gate._source_fingerprint()
    untracked.write_text("two", encoding="utf-8")
    second = gate._source_fingerprint()
    assert first["staged_diff_sha256"] == second["staged_diff_sha256"]
    assert first["untracked"] != second["untracked"]
    state["untracked"] = b""
    deleted = gate._source_fingerprint()
    assert deleted["untracked"] == []
    assert deleted["sha256"] != second["sha256"]


def test_workbench_coverage_requires_t002_and_m108() -> None:
    t002 = {
        "schema_version": gate.LAYER_REPORT_SCHEMAS["workbench"], "ok": True,
        "scenario_count": 14, "expected_scenario_count": 14,
        "facets": {name: {"status": "passed"} for name in ("browser", "renderer", "quality", "export")},
    }
    m108 = {
        "schema_version": "M108WorkbenchRendererSelfTest@1", "phase": "self_test",
        "subsystem": "workbench_renderer_route_lifecycle", "ok": True, "repeat_count": 3,
    }
    assert gate.validate_workbench_coverage(t002, m108)["ok"] is True
    assert gate.validate_workbench_coverage(t002, {**m108, "repeat_count": 2})["stable_error_code"] == "WORKBENCH_COVERAGE_INCOMPLETE"


def test_run_gate_fails_fast_at_first_layer(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    calls: list[str] = []
    monkeypatch.setattr(gate, "_source_fingerprint", lambda: {"sha256": "stable"})

    def fake_run(layer: str, *args, **kwargs):
        calls.append(layer)
        return _report(layer, status="failed", exit_code=gate.EXIT_CODES[layer])

    monkeypatch.setattr(gate, "_run_layer", fake_run)
    monkeypatch.setattr(gate, "_write_aggregate", lambda *args, **kwargs: gate.EXIT_CODES["host"])
    assert gate.run_gate(tmp_path, 1) == gate.EXIT_CODES["host"]
    assert calls == ["host"]


@pytest.mark.parametrize("failed_layer", gate.LAYER_ORDER)
def test_run_gate_fails_fast_at_each_layer(monkeypatch: pytest.MonkeyPatch, tmp_path: Path, failed_layer: str) -> None:
    calls: list[str] = []
    monkeypatch.setattr(gate, "_source_fingerprint", lambda: {"sha256": "stable"})
    monkeypatch.setattr(gate, "_require_packaged_artifacts", lambda: {"sidecar": {"sha256": "a"}, "app_executable": {"sha256": "b"}})

    def fake_run(layer: str, *args, **kwargs):
        calls.append(layer)
        return _report(layer, status="failed" if layer == failed_layer else "passed", exit_code=gate.EXIT_CODES[layer] if layer == failed_layer else 0)

    monkeypatch.setattr(gate, "_run_layer", fake_run)
    monkeypatch.setattr(gate, "_write_aggregate", lambda *args, **kwargs: gate.EXIT_CODES[failed_layer])
    assert gate.run_gate(tmp_path, 1) == gate.EXIT_CODES[failed_layer]
    assert calls == list(gate.LAYER_ORDER[: gate.LAYER_ORDER.index(failed_layer) + 1])


def test_host_layer_receives_real_toolchain_and_artifact_checks(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setattr(gate, "_source_fingerprint", lambda: {"sha256": "stable"})
    seen: list[list[str]] = []

    def fake_run(layer: str, command, *args, **kwargs):
        seen.append(list(command))
        return _report(layer, status="failed", exit_code=gate.EXIT_CODES[layer])

    monkeypatch.setattr(gate, "_run_layer", fake_run)
    monkeypatch.setattr(gate, "_write_aggregate", lambda *args, **kwargs: gate.EXIT_CODES["host"])
    gate.run_gate(tmp_path, 1)
    command = seen[0]
    assert "--no-default-commands" not in command
    assert "--no-venv" not in command
    assert "--sidecar" in command and "--app-artifact" in command
    library_index = command.index("--library")
    host_library = Path(command[library_index + 1])
    assert host_library == tmp_path / "host-library"
    assert host_library.is_dir()


def test_run_gate_complete_path_includes_workbench(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setattr(gate, "_source_fingerprint", lambda: {"sha256": "stable"})
    monkeypatch.setattr(gate, "_require_packaged_artifacts", lambda: {"sidecar": {"path": "sidecar", "sha256": "a"}, "app_executable": {"path": "app", "sha256": "b"}})
    monkeypatch.setattr(gate, "_bind_packaged_artifacts", lambda report: {**report, "artifacts": {"sidecar": {"sha256": "a"}, "app_executable": {"sha256": "b"}}})
    monkeypatch.setattr(gate, "_run_layer", lambda layer, *args, **kwargs: _report(layer))
    monkeypatch.setattr(gate, "_run_workbench_layer", lambda **kwargs: {**_report("workbench"), "ok": True, "status": "passed"})
    result: dict = {}

    def fake_write(*args, **kwargs):
        result["reports"] = args[1]
        return 0

    monkeypatch.setattr(gate, "_write_aggregate", fake_write)
    assert gate.run_gate(tmp_path, 1) == 0
    assert set(result["reports"]) == set(gate.LAYER_ORDER)


def test_main_exception_writes_structured_report(monkeypatch: pytest.MonkeyPatch, tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    monkeypatch.setattr(gate, "run_gate", lambda *args, **kwargs: (_ for _ in ()).throw(RuntimeError("hidden")))
    assert gate.main(["--artifact-dir", str(tmp_path)]) == gate.EXIT_CODES["internal"]
    report = json.loads((tmp_path / "report.json").read_text(encoding="utf-8"))
    assert report["stable_error_code"] == "RUNNER_INTERNAL_ERROR"
    assert "hidden" not in capsys.readouterr().out
