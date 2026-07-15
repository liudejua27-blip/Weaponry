#!/usr/bin/env python3
"""No-secret structural preflight for a ForgeCAD packaged Agent sidecar.

This deliberately does not execute a supplied binary.  P008 establishes that
the declared input is structurally suitable for P002's real packaged Alpha
launch check; P002 owns process startup, first initialization and recovery.
"""

from __future__ import annotations

import argparse
import json
import stat
import struct
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
TAURI_DIR = ROOT / "apps" / "desktop" / "src-tauri"
DEFAULT_CONTRACT = TAURI_DIR / "binaries" / "sidecar-inputs.json"
SCHEMA_VERSION = "ForgeCADPackagedSidecarInput@1"
TARGETS = {
    "aarch64-apple-darwin": ("macos", "aarch64", "mach_o"),
    "x86_64-apple-darwin": ("macos", "x86_64", "mach_o"),
    "x86_64-pc-windows-msvc": ("windows", "x86_64", "pe"),
    "x86_64-unknown-linux-gnu": ("linux", "x86_64", "elf"),
}
SECRET_MARKERS = ("sk-", "api_key=", "authorization: bearer", "bearer ", "synthetic-credential-marker")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--workspace-root", type=Path, default=ROOT)
    parser.add_argument(
        "--require-ready",
        action="store_true",
        help="return non-zero unless the structural input is ready for P002",
    )
    args = parser.parse_args()
    report = preflight(args.contract, args.workspace_root)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if not args.require_ready or report["status"] == "ready_for_local_alpha" else 1


def preflight(contract_path: Path, workspace_root: Path) -> dict[str, Any]:
    """Read only the contract and sidecar bytes; never read provider configuration."""
    report: dict[str, Any] = {
        "schema_version": "ForgeCADPackagedSidecarReadiness@1",
        "status": "blocked_invalid_contract",
        "contract_path": _display_path(contract_path, workspace_root),
        "provider_secret_accessed": False,
        "network_calls_made": 0,
        "binary_executed": False,
        "missing_inputs": [],
        "invalid_inputs": [],
        "next_gate": "FGC-P002",
    }
    try:
        contract = json.loads(contract_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        report["invalid_inputs"].append(f"contract_unreadable:{exc}")
        return report

    errors = validate_contract(contract)
    if errors:
        report["invalid_inputs"].extend(errors)
        return report

    target = contract["target"]
    binary_path = _resolve_binary_path(workspace_root, target["binary_path"])
    report["target"] = {
        "triple": target["triple"],
        "os": target["os"],
        "architecture": target["architecture"],
        "file_format": target["file_format"],
        "binary_path": _display_path(binary_path, workspace_root),
    }
    report["launch"] = {
        "arguments": contract["launch"]["arguments"],
        "health_check_command": _health_check_command(contract["launch"], contract["health_check"]),
        "provider_secret_boundary": contract["launch"]["provider_secret_boundary"],
    }
    report["local_alpha_checks"] = contract["local_alpha_checks"]

    if not binary_path.exists() or not binary_path.is_file() or binary_path.stat().st_size == 0:
        report["status"] = "blocked_missing_sidecar"
        report["missing_inputs"].append(
            {
                "code": "TARGET_SIDECAR_REQUIRED",
                "path": _display_path(binary_path, workspace_root),
                "target": target["triple"],
                "reason": "Provide a non-empty frozen sidecar for the declared target; do not add a credential or a placeholder header.",
            }
        )
        return report

    binary_issues, binary_details = inspect_binary(binary_path, target)
    report["binary"] = binary_details
    if binary_issues:
        report["status"] = "blocked_invalid_sidecar"
        report["invalid_inputs"].extend(binary_issues)
        return report

    report["status"] = "ready_for_local_alpha"
    report["ready_boundary"] = (
        "Structural P008 input is complete. P002 must still launch this exact sidecar, "
        "run the declared health check, and verify first initialization and restart recovery."
    )
    return report


def validate_contract(contract: Any) -> list[str]:
    if not isinstance(contract, dict):
        return ["contract_must_be_object"]
    allowed = {"schema_version", "sidecar_id", "target", "launch", "health_check", "local_alpha_checks"}
    errors = _exact_keys(contract, allowed, "contract")
    if contract.get("schema_version") != SCHEMA_VERSION:
        errors.append("schema_version_mismatch")
    if contract.get("sidecar_id") != "wushen-agent":
        errors.append("sidecar_id_must_be_wushen-agent")
    target = contract.get("target")
    launch = contract.get("launch")
    health = contract.get("health_check")
    checks = contract.get("local_alpha_checks")
    if not isinstance(target, dict):
        errors.append("target_must_be_object")
    else:
        allowed_target = {"triple", "os", "architecture", "binary_path", "file_format", "requires_owner_execute"}
        errors.extend(_exact_keys(target, allowed_target, "target"))
        triple = target.get("triple")
        expected = TARGETS.get(triple)
        actual = (target.get("os"), target.get("architecture"), target.get("file_format"))
        if expected is None or actual != expected:
            errors.append("target_triple_os_architecture_format_mismatch")
        expected_path = f"binaries/wushen-agent-{triple}" if isinstance(triple, str) else None
        if target.get("binary_path") != expected_path:
            errors.append("target_binary_path_mismatch")
        if target.get("requires_owner_execute") is not (target.get("os") != "windows"):
            errors.append("target_execute_requirement_mismatch")
    if not isinstance(launch, dict):
        errors.append("launch_must_be_object")
    else:
        allowed_launch = {"arguments", "managed_host", "managed_port", "required_environment_names", "provider_secret_boundary"}
        errors.extend(_exact_keys(launch, allowed_launch, "launch"))
        if launch.get("arguments") != ["agent", "serve"]:
            errors.append("launch_arguments_must_be_agent_serve")
        if launch.get("managed_host") != "127.0.0.1" or launch.get("managed_port") != 8000:
            errors.append("launch_endpoint_mismatch")
        if launch.get("required_environment_names") != ["WUSHEN_LIBRARY_ROOT", "WUSHEN_MIGRATIONS_DIR"]:
            errors.append("launch_environment_contract_mismatch")
        if launch.get("provider_secret_boundary") != "runtime-keychain-or-permission-restricted-secret-file":
            errors.append("provider_secret_boundary_mismatch")
    if not isinstance(health, dict):
        errors.append("health_check_must_be_object")
    else:
        allowed_health = {"method", "path", "expected_status", "expected_json"}
        errors.extend(_exact_keys(health, allowed_health, "health_check"))
        if health.get("method") != "GET" or health.get("path") != "/api/health" or health.get("expected_status") != 200:
            errors.append("health_check_request_mismatch")
        if health.get("expected_json") != {"service": "wushen-agent", "status": "ok"}:
            errors.append("health_check_payload_mismatch")
    if checks != ["first_initialization", "workbench_startup", "glb_export", "restart_recovery"]:
        errors.append("local_alpha_checks_mismatch")
    if _contains_secret_like_value(contract):
        errors.append("secret_like_value_forbidden")
    return sorted(set(errors))


def inspect_binary(path: Path, target: dict[str, Any]) -> tuple[list[str], dict[str, Any]]:
    payload = path.read_bytes()[:4096]
    details: dict[str, Any] = {
        "byte_size": path.stat().st_size,
        "owner_executable": bool(path.stat().st_mode & stat.S_IXUSR),
        "detected_format": "unknown",
        "detected_architecture": None,
    }
    detected_format, detected_architecture = _detect_binary(payload)
    details["detected_format"] = detected_format
    details["detected_architecture"] = detected_architecture
    errors: list[str] = []
    if target["requires_owner_execute"] and not details["owner_executable"]:
        errors.append("owner_execute_bit_missing")
    if detected_format != target["file_format"]:
        errors.append(f"binary_format_mismatch_expected_{target['file_format']}")
    if detected_architecture != target["architecture"]:
        errors.append(f"binary_architecture_mismatch_expected_{target['architecture']}")
    return errors, details


def _detect_binary(payload: bytes) -> tuple[str, str | None]:
    if len(payload) >= 8 and payload[:4] in {b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf"}:
        endian = "<" if payload[:4] == b"\xcf\xfa\xed\xfe" else ">"
        cpu_type = struct.unpack(f"{endian}I", payload[4:8])[0]
        return "mach_o", {0x0100000C: "aarch64", 0x01000007: "x86_64"}.get(cpu_type)
    if len(payload) >= 20 and payload[:4] == b"\x7fELF":
        endian = "<" if payload[5:6] == b"\x01" else ">" if payload[5:6] == b"\x02" else None
        machine = struct.unpack(f"{endian}H", payload[18:20])[0] if endian else None
        return "elf", {0x3E: "x86_64", 0xB7: "aarch64"}.get(machine)
    if len(payload) >= 0x40 and payload[:2] == b"MZ":
        offset = struct.unpack("<I", payload[0x3C:0x40])[0]
        if len(payload) >= offset + 6 and payload[offset : offset + 4] == b"PE\0\0":
            machine = struct.unpack("<H", payload[offset + 4 : offset + 6])[0]
            return "pe", {0x8664: "x86_64", 0xAA64: "aarch64"}.get(machine)
        return "pe", None
    return "unknown", None


def _resolve_binary_path(workspace_root: Path, binary_path: str) -> Path:
    root = (workspace_root / "apps" / "desktop" / "src-tauri").resolve()
    candidate = (root / binary_path).resolve()
    if root not in (candidate, *candidate.parents):
        raise ValueError("binary_path escapes src-tauri")
    return candidate


def _health_check_command(launch: dict[str, Any], health: dict[str, Any]) -> str:
    return (
        f"curl --fail --silent --show-error http://{launch['managed_host']}:{launch['managed_port']}{health['path']} "
        "# expect HTTP 200 JSON: service=wushen-agent, status=ok"
    )


def _display_path(path: Path, workspace_root: Path) -> str:
    try:
        return str(path.resolve().relative_to(workspace_root.resolve()))
    except ValueError:
        return str(path)


def _exact_keys(value: dict[str, Any], allowed: set[str], label: str) -> list[str]:
    return [f"{label}_unknown_key_{key}" for key in value if key not in allowed]


def _contains_secret_like_value(value: Any) -> bool:
    if isinstance(value, dict):
        return any(_contains_secret_like_value(item) for item in value.values())
    if isinstance(value, list):
        return any(_contains_secret_like_value(item) for item in value)
    return isinstance(value, str) and any(marker in value.lower() for marker in SECRET_MARKERS)


if __name__ == "__main__":
    raise SystemExit(main())
