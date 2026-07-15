#!/usr/bin/env python3
"""P008 coverage for no-secret packaged-sidecar input preflight."""

from __future__ import annotations

import json
import os
import struct
import tempfile
from pathlib import Path

from packaged_sidecar_preflight import DEFAULT_CONTRACT, preflight


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_p008_") as temporary:
        workspace = Path(temporary)
        contract = workspace / "sidecar-inputs.json"
        contract.write_text(DEFAULT_CONTRACT.read_text(encoding="utf-8"), encoding="utf-8")

        blocked = preflight(contract, workspace)
        _assert(blocked["status"] == "blocked_missing_sidecar", "empty packaged sidecar fixture must be reported as missing")
        _assert(blocked["provider_secret_accessed"] is False, "preflight must not read provider secrets")
        _assert(blocked["network_calls_made"] == 0 and blocked["binary_executed"] is False, "preflight must stay offline")
        _assert(blocked["launch"]["health_check_command"].startswith("curl --fail"), "health command missing")

        binary = workspace / "apps/desktop/src-tauri/binaries/wushen-agent-aarch64-apple-darwin"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b"\xcf\xfa\xed\xfe" + struct.pack("<I", 0x0100000C) + b"frozen-sidecar-input")
        os.chmod(binary, 0o755)
        ready = preflight(contract, workspace)
        _assert(ready["status"] == "ready_for_local_alpha", "valid target input was not ready")
        _assert(ready["binary"]["detected_architecture"] == "aarch64", "architecture was not inspected")

        binary.write_bytes(b"\xcf\xfa\xed\xfe" + struct.pack("<I", 0x01000007) + b"wrong-architecture")
        mismatch = preflight(contract, workspace)
        _assert(mismatch["status"] == "blocked_invalid_sidecar", "wrong architecture was accepted")

        payload = json.loads(DEFAULT_CONTRACT.read_text(encoding="utf-8"))
        payload["launch"]["unexpected"] = "synthetic-credential-marker"
        contract.write_text(json.dumps(payload), encoding="utf-8")
        invalid_contract = preflight(contract, workspace)
        _assert(invalid_contract["status"] == "blocked_invalid_contract", "secret-like contract value was accepted")

    print(json.dumps({"ok": True, "blocked_report": True, "ready_fixture": True, "secret_free": True}))
    return 0


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
