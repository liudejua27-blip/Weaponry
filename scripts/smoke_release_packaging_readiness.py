#!/usr/bin/env python3
"""Exercise sidecar artifact integrity guards without modifying release assets."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

from check_release_packaging_readiness import _icon_integrity_issues, _sidecar_integrity_issues


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_packaging_sidecar_") as temporary:
        root = Path(temporary)
        empty = root / "wushen-agent-aarch64-apple-darwin"
        empty.touch()
        _assert(
            {"empty", "owner_execute_bit_missing", "invalid_header_expected_Mach-O"}
            <= set(_sidecar_integrity_issues(empty)),
            "empty macOS sidecar was accepted",
        )
        macos = root / "wushen-agent-aarch64-apple-darwin"
        macos.write_bytes(b"\xcf\xfa\xed\xfeplaceholder")
        os.chmod(macos, 0o755)
        _assert(not _sidecar_integrity_issues(macos), "Mach-O fixture was rejected")
        linux = root / "wushen-agent-x86_64-unknown-linux-gnu"
        linux.write_bytes(b"\x7fELFplaceholder")
        os.chmod(linux, 0o755)
        _assert(not _sidecar_integrity_issues(linux), "ELF fixture was rejected")
        windows = root / "wushen-agent-x86_64-pc-windows-msvc.exe"
        windows.write_bytes(b"MZplaceholder")
        _assert(not _sidecar_integrity_issues(windows), "PE fixture was rejected")
        empty_icon = root / "empty.png"
        empty_icon.touch()
        _assert(_icon_integrity_issues(empty_icon) == ["empty"], "empty icon was accepted")
        png_icon = root / "valid.png"
        png_icon.write_bytes(b"\x89PNG\r\n\x1a\n" + b"x" * 16)
        _assert(not _icon_integrity_issues(png_icon), "PNG header fixture was rejected")
    print('{"ok": true, "empty_sidecar_rejected": true, "target_headers_validated": true}')
    return 0


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
