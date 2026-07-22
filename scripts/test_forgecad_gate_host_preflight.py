from __future__ import annotations

import json
import errno
from pathlib import Path
import subprocess
import sys
import time
from types import SimpleNamespace

import pytest

import forgecad_gate_host_preflight as gate
import forgecad_gate_k003_layers as layers


ROOT = Path(__file__).resolve().parents[1]


def _config(tmp_path: Path, **overrides) -> gate.PreflightConfig:
    workspace = tmp_path / "workspace"
    library = tmp_path / "library"
    workspace.mkdir()
    library.mkdir()
    values = {
        "workspace": workspace,
        "library": library,
        "tmp_dir": tmp_path,
        "required_commands": ("python3",),
        "venv": Path(sys.executable),
        "dynamic_port": True,
    }
    values.update(overrides)
    return gate.PreflightConfig(**values)


def _healthy_macos(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(gate.platform, "system", lambda: "Darwin")
    values = {
        "kern.num_files": 100,
        "kern.maxfiles": 1000,
        "kern.num_vnodes": 100,
        "kern.maxvnodes": 1000,
    }
    monkeypatch.setattr(gate, "_read_sysctl", lambda name: values[name])
    monkeypatch.setattr(gate, "_fd_snapshot", lambda: {"open_count": 10, "soft_limit": 100, "hard_limit": 200})
    monkeypatch.setattr(
        gate,
        "_collect_no_interference_sampling",
        lambda: {
            "schema_version": "ForgeCADHostNoInterferenceSampling@1",
            "phase": "host_preflight",
            "subsystem": "sampling",
            "stable_error_code": "HOST_NO_INTERFERENCE_SAMPLING_CLEAR",
            "no_interference": True,
            "status": "clear",
            "forgecad_process_attribution": {
                "status": "none_observed",
                "processes": [],
                "basis": "test_fixture_injected_clear",
            },
        },
    )


def test_success_path_is_mac_priority_and_does_not_terminate_processes(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    _healthy_macos(monkeypatch)
    report = gate.run_preflight(_config(tmp_path))
    assert report["ok"] is True
    assert report["exit_code"] == 0
    assert report["schema_version"] == "ForgeCADHostPreflightReport@1"
    assert report["checks"]["tmp"]["transaction"]["operations"] == ["create", "write", "fsync", "rename", "delete"]
    assert report["process_termination"] == "none"
    sampling = report["no_interference_sampling"]
    assert sampling["schema_version"] == "ForgeCADHostNoInterferenceSampling@1"
    assert sampling["no_interference"] is True
    assert set(sampling["resource_snapshot"]) >= {"num_vnodes", "max_vnodes", "num_files", "max_files", "ulimit_nofile_soft"}
    assert not (tmp_path / "forgecad-host-preflight").exists()


def test_fixed_port_occupied_reports_owner_and_never_kills(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    monkeypatch.setattr(gate, "_port_is_occupied", lambda port: True)
    monkeypatch.setattr(gate, "_lookup_port_owners", lambda port: [{"pid": 321, "command": "other-process"}])
    report = gate.run_preflight(_config(tmp_path, port=8000, dynamic_port=False))
    assert report["exit_code"] == gate.EXIT_HARD_FAIL
    assert report["stable_error_code"] == "HOST_PORT_OCCUPIED"
    assert report["checks"]["ports"]["owners"] == [{"pid": 321, "command": "other-process"}]
    assert report["checks"]["ports"]["action"] == "observe_only_no_kill"


def test_expected_port_owner_is_reported_without_failure(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    monkeypatch.setattr(gate, "_port_is_occupied", lambda port: True)
    monkeypatch.setattr(gate, "_lookup_port_owners", lambda port: [{"pid": 321, "command": "forgecad"}])
    report = gate.run_preflight(_config(tmp_path, port=8000, dynamic_port=False, expected_owner="321"))
    assert report["exit_code"] == 0
    assert report["checks"]["ports"]["owner_status"] == "expected_owner"


def test_tmp_unwritable_injection_is_hard_fail(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    def fail_tmp(path: Path) -> dict:
        raise gate.ProbeFailure("HOST_TMP_UNWRITABLE", "tmp")
    monkeypatch.setattr(gate, "probe_tmp_transaction", fail_tmp)
    report = gate.run_preflight(_config(tmp_path))
    assert report["exit_code"] == gate.EXIT_HARD_FAIL
    assert any(item["stable_error_code"] == "HOST_TMP_UNWRITABLE" for item in report["findings"])


def test_tmp_primary_failure_survives_cleanup_residue(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    real_rmtree = gate.shutil.rmtree
    def fail_replace(*args, **kwargs) -> None:
        raise OSError(errno.EIO, "injected primary failure")

    def fail_cleanup(*args, **kwargs) -> None:
        raise OSError(errno.EIO, "injected cleanup failure")

    monkeypatch.setattr(gate.os, "replace", fail_replace)
    monkeypatch.setattr(gate.shutil, "rmtree", fail_cleanup)
    with pytest.raises(gate.ProbeFailure) as captured:
        gate.probe_tmp_transaction(tmp_path)
    assert captured.value.code == "HOST_TMP_UNWRITABLE"
    assert captured.value.details == {"cleanup_status": "residue", "cleanup_residue": True}
    for residue in tmp_path.glob("forgecad-host-preflight-*"):
        real_rmtree(residue)

    _healthy_macos(monkeypatch)
    monkeypatch.setattr(gate, "probe_tmp_transaction", lambda root: (_ for _ in ()).throw(captured.value))
    report = gate.run_preflight(_config(tmp_path))
    finding = next(item for item in report["findings"] if item["check"] == "tmp_transaction")
    assert finding["stable_error_code"] == "HOST_TMP_UNWRITABLE"
    assert finding["details"] == {"cleanup_status": "residue", "cleanup_residue": True}


def test_low_fd_threshold_is_hard_fail_and_attributed_to_host(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    monkeypatch.setattr(gate, "_fd_snapshot", lambda: {"open_count": 99, "soft_limit": 100, "hard_limit": 200})
    report = gate.run_preflight(_config(tmp_path))
    finding = next(item for item in report["findings"] if item["stable_error_code"] == "HOST_FD_PRESSURE_HARD")
    assert report["exit_code"] == gate.EXIT_HARD_FAIL
    assert finding["phase"] == "host_preflight"
    assert report["checks"]["resources"]["attribution"] == "host_resource_only"


def test_vnode_pressure_adds_bounded_holder_summary_and_safe_recovery(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    values = {
        "kern.num_files": 100,
        "kern.maxfiles": 1000,
        "kern.num_vnodes": 1000,
        "kern.maxvnodes": 1000,
    }
    monkeypatch.setattr(gate, "_read_sysctl", lambda name: values[name])
    monkeypatch.setattr(
        gate,
        "_collect_holder_summary",
        lambda limit: {
            "status": "available",
            "source": "lsof",
            "visibility": "visible_user_scope_only",
            "holders": [{"pid": 123, "process_class": "forgecad_or_test_tool", "open_file_count": 77}],
        },
    )
    observed_roots = []

    def capacity_probe(root: Path, *, root_label: str, **kwargs) -> dict:
        observed_roots.append((root_label, kwargs))
        return {"root_label": root_label, "status": "passed", "operations": ["create", "rmdir"]}

    monkeypatch.setattr(gate, "probe_filesystem_capacity", capacity_probe)
    report = gate.run_preflight(_config(tmp_path))
    resources = report["checks"]["resources"]
    vnode_warning = next(item for item in report["findings"] if item["stable_error_code"] == "HOST_VNODES_PRESSURE_WARNING")
    assert report["exit_code"] == gate.EXIT_OK
    assert vnode_warning["details"] == {
        "current": 1000,
        "maximum": 1000,
        "attribution": "host_resource_only",
        "metric_kind": "kernel_cache_proxy",
        "threshold_level": "hard",
        "requires_capability_probe": True,
    }
    assert [root for root, _ in observed_roots] == ["tmp", "library"]
    assert all(kwargs["file_count"] == 64 for _, kwargs in observed_roots)
    assert resources["holder_summary"]["holders"][0]["process_class"] == "forgecad_or_test_tool"
    assert resources["recovery_guidance"]["status"] == "task_residue_possible"
    assert resources["recovery_guidance"]["destructive_action"] == "none"


def test_below_vnode_hard_threshold_never_runs_capacity_probe(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    values = {
        "kern.num_files": 100,
        "kern.maxfiles": 1000,
        "kern.num_vnodes": 850,
        "kern.maxvnodes": 1000,
    }
    monkeypatch.setattr(gate, "_read_sysctl", lambda name: values[name])
    monkeypatch.setattr(gate, "probe_filesystem_capacity", lambda *args, **kwargs: pytest.fail("capacity probe must not run below hard vnode threshold"))
    report = gate.run_preflight(_config(tmp_path))
    assert report["exit_code"] == gate.EXIT_OK
    assert report["checks"]["resources"]["filesystem_capacity_probe"] == {"status": "not_needed", "roots": []}
    finding = next(item for item in report["findings"] if item["stable_error_code"] == "HOST_VNODES_PRESSURE_WARNING")
    assert finding["details"]["threshold_level"] == "warning"
    assert finding["details"]["requires_capability_probe"] is False


def test_capacity_probe_enfile_corrobates_vnode_hard_failure(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    values = {
        "kern.num_files": 100,
        "kern.maxfiles": 1000,
        "kern.num_vnodes": 1000,
        "kern.maxvnodes": 1000,
    }
    monkeypatch.setattr(gate, "_read_sysctl", lambda name: values[name])

    def fail_capacity(*args, **kwargs):
        raise gate._capacity_probe_failure("tmp", "create_probe_root", OSError(errno.ENFILE, "file table full"))

    monkeypatch.setattr(gate, "probe_filesystem_capacity", fail_capacity)
    report = gate.run_preflight(_config(tmp_path))
    finding = next(item for item in report["findings"] if item["stable_error_code"] == "HOST_VNODES_PRESSURE_HARD")
    assert report["exit_code"] == gate.EXIT_HARD_FAIL
    assert finding["details"] == {
        "corroborated": True,
        "operation": "create_probe_root",
        "errno_class": "file_table_exhausted",
        "root_label": "tmp",
        "cleanup_status": "not_needed",
        "cleanup_residue": False,
        "worker_cleanup_status": "not_needed",
        "worker_process_residue": False,
    }


@pytest.mark.parametrize("errno_value, expected", [(errno.EMFILE, "file_table_exhausted"), (errno.ENOSPC, "disk_full"), (errno.EIO, "io_error"), (errno.EROFS, "read_only")])
def test_capacity_probe_errno_classification_is_stable(errno_value: int, expected: str) -> None:
    assert gate._filesystem_errno_class(OSError(errno_value, "injected")) == expected


def test_capacity_probe_is_bounded_cleans_up_and_hides_disposable_path(tmp_path: Path) -> None:
    result = gate.probe_filesystem_capacity(tmp_path, root_label="tmp", file_count=64, payload_bytes=4096, deadline_seconds=2.0)
    serialized = json.dumps(result, sort_keys=True)
    assert result["completed_files"] == 64
    assert result["peak_files"] == 64
    assert result["total_bytes"] == 64 * 4096 <= 1024 * 1024
    assert "forgecad-capacity-" not in serialized
    assert not list(tmp_path.glob("forgecad-capacity-*"))


def test_capacity_payload_has_exact_requested_length_at_boundary() -> None:
    assert len(gate._capacity_payload(1)) == 1
    assert len(gate._capacity_payload(64)) == 64


def _sleeping_capacity_worker(*args) -> None:
    time.sleep(5)


def test_capacity_probe_timeout_is_parent_terminable_and_cleans_up(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setattr(gate, "_capacity_worker", _sleeping_capacity_worker)
    started = time.monotonic()
    with pytest.raises(gate.FilesystemCapacityFailure) as captured:
        gate.probe_filesystem_capacity(tmp_path, root_label="tmp", file_count=64, payload_bytes=1, deadline_seconds=0.05)
    assert time.monotonic() - started < 1.0
    assert captured.value.operation == "deadline"
    assert captured.value.errno_class == "timeout"
    assert not list(tmp_path.glob("forgecad-capacity-*"))


def test_never_alive_worker_preserves_deadline_and_reports_residue(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    class NeverAliveProcess:
        terminate_calls = 0
        kill_calls = 0

        def start(self) -> None:
            pass

        def is_alive(self) -> bool:
            return True

        def join(self, timeout=None) -> None:
            pass

        def terminate(self) -> None:
            self.terminate_calls += 1

        def kill(self) -> None:
            self.kill_calls += 1

    class FakeQueue:
        def cancel_join_thread(self) -> None:
            pass

        def close(self) -> None:
            pass

    process = NeverAliveProcess()

    class FakeContext:
        def Process(self, *args, **kwargs):
            return process

        def Queue(self, **kwargs):
            return FakeQueue()

    monkeypatch.setattr(gate, "_capacity_context", lambda: FakeContext())
    with pytest.raises(gate.FilesystemCapacityFailure) as captured:
        gate.probe_filesystem_capacity(tmp_path, root_label="tmp", file_count=64, payload_bytes=1, deadline_seconds=0.01)
    failure = captured.value
    assert failure.operation == "deadline"
    assert failure.errno_class == "timeout"
    assert failure.worker_cleanup_status == "residue"
    assert failure.worker_process_residue is True
    assert failure.cleanup_status == "not_attempted_worker_residue"
    assert failure.cleanup_residue is True
    assert process.terminate_calls >= 2
    assert process.kill_calls >= 2


@pytest.mark.parametrize("operation, errno_class", [("deadline", "timeout"), ("cleanup", "cleanup_residue")])
def test_capacity_probe_timeout_and_cleanup_residue_remain_hard_fail(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, operation: str, errno_class: str
) -> None:
    _healthy_macos(monkeypatch)
    values = {
        "kern.num_files": 100,
        "kern.maxfiles": 1000,
        "kern.num_vnodes": 1000,
        "kern.maxvnodes": 1000,
    }
    monkeypatch.setattr(gate, "_read_sysctl", lambda name: values[name])
    monkeypatch.setattr(
        gate,
        "probe_filesystem_capacity",
        lambda *args, **kwargs: (_ for _ in ()).throw(gate.FilesystemCapacityFailure(root_label="tmp", operation=operation, errno_class=errno_class)),
    )
    report = gate.run_preflight(_config(tmp_path))
    finding = next(item for item in report["findings"] if item["stable_error_code"] == "HOST_VNODES_PRESSURE_HARD")
    assert report["exit_code"] == gate.EXIT_HARD_FAIL
    assert finding["details"]["operation"] == operation
    assert finding["details"]["errno_class"] == errno_class


def test_primary_capacity_failure_is_not_hidden_by_cleanup_residue(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    values = {"kern.num_files": 100, "kern.maxfiles": 1000, "kern.num_vnodes": 1000, "kern.maxvnodes": 1000}
    monkeypatch.setattr(gate, "_read_sysctl", lambda name: values[name])
    monkeypatch.setattr(
        gate,
        "probe_filesystem_capacity",
        lambda *args, **kwargs: (_ for _ in ()).throw(
            gate.FilesystemCapacityFailure(
                root_label="tmp",
                operation="phase_a_create_write_fsync",
                errno_class="file_table_exhausted",
                cleanup_status="residue",
                cleanup_residue=True,
            )
        ),
    )
    report = gate.run_preflight(_config(tmp_path))
    finding = next(item for item in report["findings"] if item["stable_error_code"] == "HOST_VNODES_PRESSURE_HARD")
    assert finding["details"]["operation"] == "phase_a_create_write_fsync"
    assert finding["details"]["errno_class"] == "file_table_exhausted"
    assert finding["details"]["cleanup_status"] == "residue"
    assert finding["details"]["cleanup_residue"] is True


def test_vnode_warning_respects_warnings_fail_after_capacity_succeeds(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    values = {
        "kern.num_files": 100,
        "kern.maxfiles": 1000,
        "kern.num_vnodes": 1000,
        "kern.maxvnodes": 1000,
    }
    monkeypatch.setattr(gate, "_read_sysctl", lambda name: values[name])
    monkeypatch.setattr(gate, "probe_filesystem_capacity", lambda root, *, root_label, **kwargs: {"root_label": root_label, "status": "passed"})
    report = gate.run_preflight(_config(tmp_path, warnings_fail=True))
    assert report["exit_code"] == gate.EXIT_WARNING
    assert report["stable_error_code"] == "HOST_PREFLIGHT_WARNINGS"
    assert report["ok"] is False
    assert layers._validate_layer_report("host", report, gate.EXIT_WARNING) is None


def test_holder_category_is_path_free() -> None:
    assert gate._holder_category("Google Chrome", "Chrome Helper") == "chrome"
    assert gate._holder_category("syspolicyd", "syspolicyd") == "syspolicyd"
    assert gate._holder_category("cargo", "cargo") == "forgecad_or_test_tool"


def test_active_heavy_process_sampling_is_warning_and_preserves_hard_gate(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    monkeypatch.setattr(
        gate,
        "_collect_no_interference_sampling",
        lambda: {
            "schema_version": "ForgeCADHostNoInterferenceSampling@1",
            "phase": "host_preflight",
            "subsystem": "sampling",
            "stable_error_code": "HOST_SAMPLING_INTERFERENCE_ACTIVE",
            "no_interference": False,
            "status": "active_heavy_processes",
            "forgecad_process_attribution": {"status": "active_processes_observed", "processes": []},
        },
    )
    report = gate.run_preflight(_config(tmp_path))
    assert report["exit_code"] == 0
    assert any(item["stable_error_code"] == "HOST_SAMPLING_INTERFERENCE_ACTIVE" for item in report["findings"])


def test_missing_tool_is_stable_and_does_not_capture_command_output(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    _healthy_macos(monkeypatch)
    monkeypatch.setattr(gate, "_resolve_command", lambda command: None if command == "missing-tool" else "/usr/bin/python3")
    report = gate.run_preflight(_config(tmp_path, required_commands=("missing-tool",)))
    serialized = json.dumps(report, sort_keys=True)
    assert report["stable_error_code"] == "HOST_TOOL_MISSING"
    assert report["exit_code"] == gate.EXIT_HARD_FAIL
    assert "secret-value" not in serialized
    assert "prompt" not in serialized
    assert "body" not in serialized


def test_rust_commands_resolve_via_workspace_wrapper_when_path_is_missing(monkeypatch: pytest.MonkeyPatch) -> None:
    original_resolve = gate._resolve_command

    def resolve(command: str):
        if command in {"rustc", "cargo", "rustup"}:
            return None
        return original_resolve(command)

    monkeypatch.setattr(gate, "_resolve_command", resolve)
    report, findings = gate._check_tools(gate.PreflightConfig(workspace=ROOT, required_commands=("rustc", "cargo")))
    assert findings == []
    assert report["rust_toolchain"]["wrapper"]["path"] == "script/with_rust_toolchain.sh"
    assert report["rust_toolchain"]["wrapper"]["executable"] is True
    assert report["rust_toolchain"]["rustup"]["status"] == "unresolved"
    assert report["rust_toolchain"]["commands"]["cargo"]["wrapper_probe"] == "passed"
    assert report["required"]["rustc"]["status"] == "resolved"
    assert report["required"]["rustc"]["resolution"] == "workspace_wrapper"
    assert report["required"]["rustc"]["location"] == "workspace_wrapper"
    assert report["required"]["cargo"]["status"] == "resolved"


def test_rust_wrapper_failure_remains_a_missing_rust_tool(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    wrapper = tmp_path / "script" / "with_rust_toolchain.sh"
    wrapper.parent.mkdir()
    wrapper.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
    wrapper.chmod(0o755)
    monkeypatch.setattr(gate, "_resolve_command", lambda command: None)
    report, findings = gate._check_tools(gate.PreflightConfig(workspace=tmp_path, required_commands=("rustc", "cargo")))
    assert report["rust_toolchain"]["wrapper"]["executable"] is True
    assert report["rust_toolchain"]["commands"]["rustc"]["status"] == "unresolved"
    assert report["rust_toolchain"]["commands"]["rustc"]["wrapper_probe"] == "failed"
    assert [item["stable_error_code"] for item in findings] == ["HOST_TOOL_MISSING", "HOST_TOOL_MISSING"]


def test_sampling_excludes_probe_ancestry_but_keeps_other_active_processes(monkeypatch: pytest.MonkeyPatch) -> None:
    process_table = "\n".join(
        (
            "100 90 python /tmp/test_forgecad_gate_host_preflight.py",
            "90 80 node npm run aggregate-self-tests",
            "80 70 zsh zsh -lc aggregate",
            "70 1 bash bash aggregate",
            "200 1 cargo cargo test --manifest-path ForgeCAD/Cargo.toml",
            "201 1 node playwright chromium",
        )
    )
    monkeypatch.setattr(gate.os, "getpid", lambda: 100)
    monkeypatch.setattr(
        gate.subprocess,
        "run",
        lambda *args, **kwargs: SimpleNamespace(returncode=0, stdout=process_table),
    )
    report = gate._collect_no_interference_sampling()
    observed = report["forgecad_process_attribution"]["processes"]
    assert report["no_interference"] is False
    assert {item["pid"] for item in observed} == {200, 201}
    assert report["forgecad_process_attribution"]["excluded_probe_ancestry_count"] == 4


def test_sampling_never_misclassifies_git_command_text(monkeypatch: pytest.MonkeyPatch) -> None:
    process_table = "\n".join(
        (
            "100 1 python forgecad_gate_host_preflight.py",
            "200 1 git git diff -- scripts/smoke_workbench_e2e_scenarios.mjs playwright",
            "201 1 node node playwright chromium",
            "202 1 cargo cargo test",
        )
    )
    monkeypatch.setattr(gate.os, "getpid", lambda: 100)
    monkeypatch.setattr(gate.subprocess, "run", lambda *args, **kwargs: SimpleNamespace(returncode=0, stdout=process_table))
    report = gate._collect_no_interference_sampling()
    observed = report["forgecad_process_attribution"]["processes"]
    assert {item["pid"] for item in observed} == {201, 202}
    assert report["forgecad_process_attribution"]["basis"] == "short_executable_allowlist_and_redacted_classification_only"


def test_artifact_fingerprint_mismatch_is_fail_closed_without_permission_changes(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    _healthy_macos(monkeypatch)
    sidecar = tmp_path / "sidecar"
    sidecar.write_bytes(b"artifact")
    sidecar.chmod(0o755)
    before = sidecar.stat().st_mode
    report = gate.run_preflight(_config(tmp_path, sidecar=sidecar, expected_sha256={str(sidecar.resolve()): "0" * 64}))
    assert report["exit_code"] == gate.EXIT_HARD_FAIL
    assert any(item["stable_error_code"] == "HOST_PATH_FINGERPRINT_MISMATCH" for item in report["findings"])
    assert sidecar.stat().st_mode == before


def test_cli_emits_one_json_report_and_dynamic_port(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    library = tmp_path / "library"
    workspace.mkdir()
    library.mkdir()
    command = [
        sys.executable,
        str(Path(__file__).with_name("forgecad_gate_host_preflight.py")),
        "--workspace",
        str(workspace),
        "--library",
        str(library),
        "--tmp-dir",
        str(tmp_path),
        "--no-default-commands",
        "--no-venv",
        "--required-command",
        "python3",
        "--dynamic-port",
    ]
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    report = json.loads(completed.stdout)
    assert report["schema_version"] == "ForgeCADHostPreflightReport@1"
    assert report["checks"]["ports"]["policy"] == "dynamic"
    assert completed.returncode == 0
    assert report["stable_error_code"] != "HOST_VNODES_PRESSURE_HARD"
    serialized = json.dumps(report, sort_keys=True)
    assert str(tmp_path) not in serialized
    assert "/Users/" not in serialized
    assert completed.stderr == ""
