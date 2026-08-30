#!/usr/bin/env python3
"""Fail-closed source check for WPN-ARCH-RUNTIME-STORE-SPLIT-001.

This gate proves that selected implementation families left the giant roots;
it does not infer completion from the presence of new facade files.
"""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNTIME_ROOT = ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/lib.rs"
RUNTIME_SERVICE = ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/evaluation_service.rs"
RUNTIME_FAMILY = ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/evaluation_reference_comparison.rs"
STORE_ROOT = ROOT / "apps/desktop/src-tauri/crates/forgecad-store/src/lib.rs"
STORE_FAMILY = ROOT / "apps/desktop/src-tauri/crates/forgecad-store/src/approval_repository.rs"
MCP_DEFAULT = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/main.rs"
MCP_COMPAT = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/agentic_write_tools.rs"
MCP_SESSION = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/agentic_write_tools_session.rs"


def fail(message: str) -> None:
    raise SystemExit(f"WPN_ARCH_PHYSICAL_SPLIT_INVALID: {message}")


def text(path: Path) -> str:
    if not path.is_file():
        fail(f"missing source {path.relative_to(ROOT)}")
    return path.read_text(encoding="utf-8")


runtime_root = text(RUNTIME_ROOT)
runtime_service = text(RUNTIME_SERVICE)
runtime_family = text(RUNTIME_FAMILY)
store_root = text(STORE_ROOT)
store_family = text(STORE_FAMILY)
mcp_default = text(MCP_DEFAULT)
mcp_compat = text(MCP_COMPAT)
mcp_session = text(MCP_SESSION)

if len(runtime_root.splitlines()) > 51_603:
    fail("Runtime root exceeded the locked 51,603-line ceiling")
if len(store_root.splitlines()) > 78_865:
    fail("Store root exceeded the locked 78,865-line ceiling")
if len(mcp_compat.splitlines()) > 16_532:
    fail("compatibility Agentic root exceeded the locked 16,532-line ceiling")

runtime_methods = (
    "pub fn prepare_reference_comparison(",
    "pub fn render_pass_get(",
    "pub fn visual_evidence(",
    "pub fn submit_visual_review(",
    "pub fn submit_human_visual_review(",
)
for signature in runtime_methods:
    if signature in runtime_root:
        fail(f"Runtime implementation returned to lib.rs: {signature}")
    if signature not in runtime_family:
        fail(f"Runtime family lost extracted implementation: {signature}")
if '#[path = "evaluation_reference_comparison.rs"]' not in runtime_service:
    fail("Evaluation family is not a child of the existing service boundary")
if "mod evaluation_reference_comparison;" in runtime_root:
    fail("Evaluation extraction grew the Runtime root-module surface")

store_methods = (
    "pub fn insert_version(",
    "pub fn confirm_candidate(",
    "pub fn prepare_export(",
    "pub fn confirm_export(",
    "pub fn reject_candidate(",
)
for signature in store_methods:
    if signature in store_root:
        fail(f"Store implementation returned to lib.rs: {signature}")
    if signature not in store_family:
        fail(f"ApprovalLifecycle lost extracted implementation: {signature}")
for forbidden in ("fn migrate(", "CREATE TABLE", "CREATE INDEX"):
    if forbidden in store_family:
        fail(f"approval repository introduced migration ownership: {forbidden}")
if "mod approval_repository;" not in store_root:
    fail("Store root no longer wires the ApprovalLifecycle implementation")

if "mod agentic_write_tools" in mcp_default or "mod agentic_write_tools_session" in mcp_default:
    fail("default MCP source graph declared compatibility Agentic modules")
if "mod agentic_write_tools_session;" not in mcp_compat:
    fail("compatibility Agentic root no longer owns the session child")
for marker in ("session_create_schema", "checkpoint_prepare_schema", "validate_scope"):
    if marker not in mcp_session:
        fail(f"session/checkpoint child lost required implementation: {marker}")

print(
    "WPN_ARCH_PHYSICAL_SPLIT=PASS "
    "runtime_root=51603 store_root=78865 compat_agentic_root=16532 "
    "runtime_family=evaluation_reference_comparison "
    "store_family=approval_lifecycle compatibility_family=session_checkpoint_recovery"
)
