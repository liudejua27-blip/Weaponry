#!/usr/bin/env python3
"""Fail-closed structural gate for WPN-ARCH-PRESENTATION-001.

This gate checks the physical Presentation seam without treating the complete
FPS/animation family as migrated.  The active public surface is the exact
``fps_presentation`` knife façade.  Runtime owns the typed dispatch and the
Store extraction is discovered from the repository's exported
``PresentationRepository`` symbol rather than from a guessed filename.

The checker is intentionally source-oriented: it does not build Runtime or
claim visual, engine, human-review, or commercial acceptance.
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
STORE = ROOT / "apps/desktop/src-tauri/crates/forgecad-store/src"
STORE_ROOT = STORE / "lib.rs"
CONTRACT_MAP = (
    ROOT / "apps/desktop/src-tauri/crates/forgecad-contracts/src/weaponry_domain_map.rs"
)

FACADE = "fps_presentation"
EXPECTED_READ = (
    "fps_presentation_package_v2_candidate_get",
    "fps_presentation_package_v2_get",
    "fps_presentation_package_v2_production_preflight_get",
    "game_weapon_anchor_get",
    "game_weapon_animated_glb_socket_get",
    "game_weapon_animated_glb_socket_transform_projection_get",
    "game_weapon_animated_glb_socket_transform_projection_v2_get",
    "mechanical_animation_clip_get",
    "mechanical_animation_clip_preview_get",
    "mechanical_animation_clip_v2_get",
    "mechanical_animation_clip_v2_preview",
    "mechanical_animation_glb_v2_get",
)
EXPECTED_WRITE = (
    "fps_presentation_package_v2_candidate_prepare",
    "fps_presentation_package_v2_prepare",
    "game_weapon_anchor_prepare",
    "game_weapon_animated_glb_socket_prepare",
    "game_weapon_animated_glb_socket_transform_projection_prepare",
    "game_weapon_animated_glb_socket_transform_projection_v2_prepare",
    "mechanical_animation_clip_prepare",
    "mechanical_animation_clip_v2_prepare",
    "mechanical_animation_glb_v2_prepare",
)
EXPECTED_OPERATIONS = EXPECTED_READ + EXPECTED_WRITE


def fail(message: str) -> None:
    raise SystemExit(f"WPN-ARCH-PRESENTATION-001 FAIL: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def relative(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def read(path: Path) -> str:
    require(path.is_file(), f"missing {relative(path)}")
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        fail(f"cannot read {relative(path)}: {exc}")


def rust_array(text: str, constant: str) -> tuple[str, ...]:
    """Read one ``&[&str]`` constant and reject duplicate declarations."""

    pattern = re.compile(
        rf"(?:pub(?:\([^)]*\))?\s+)?const\s+{re.escape(constant)}\s*"
        r":\s*&\[\s*&str\s*\]\s*=\s*&\[(?P<body>.*?)\];",
        re.DOTALL,
    )
    matches = list(pattern.finditer(text))
    require(matches, f"missing Rust operation array {constant}")
    require(len(matches) == 1, f"duplicate Rust operation array {constant}")
    return tuple(re.findall(r'"([^"\\]*)"', matches[0].group("body")))


def balanced_block(text: str, opening: int) -> str:
    """Return a Rust brace-delimited block, ignoring comments and literals."""

    require(opening >= 0 and text[opening] == "{", "invalid Rust block start")
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
            require(depth > 0, "unbalanced Rust braces")
        index += 1
    fail("unterminated Rust brace block")
    return ""


def rust_function(text: str, marker: str) -> str:
    start = text.find(marker)
    require(start >= 0, f"missing Rust function marker {marker!r}")
    opening = text.find("{", start)
    require(opening >= 0, f"function {marker!r} has no body")
    return balanced_block(text, opening)


def match_body(text: str, marker: str = "match method") -> str:
    start = text.find(marker)
    require(start >= 0, f"missing {marker!r}")
    opening = text.find("{", start)
    require(opening >= 0, f"{marker!r} has no body")
    return balanced_block(text, opening)


def struct_body(text: str, marker: str) -> str:
    start = text.find(marker)
    require(start >= 0, f"missing {marker!r}")
    opening = text.find("{", start)
    require(opening >= 0, f"{marker!r} has no body")
    return balanced_block(text, opening)


def check_profile() -> tuple[tuple[str, ...], tuple[str, ...]]:
    try:
        profile = json.loads(read(PROFILE))
    except json.JSONDecodeError as exc:
        fail(f"invalid profile JSON: {exc}")
    require(isinstance(profile, dict), "profile must contain an object")
    facades = profile.get("facades")
    require(isinstance(facades, dict), "profile.facades must be an object")
    facade = facades.get(FACADE)
    require(isinstance(facade, dict), f"profile omits {FACADE} façade")
    require(facade.get("facade_name") == FACADE, "Presentation façade name drifted")
    require(facade.get("classification") == "read-write", "Presentation façade classification drifted")
    require(facade.get("default_enabled") is True, "Presentation façade is not default-enabled")

    read_operations = facade.get("read_tools")
    write_operations = facade.get("write_tools")
    underlying = facade.get("underlying_operations")
    require(isinstance(read_operations, list), "Presentation read_tools must be an array")
    require(isinstance(write_operations, list), "Presentation write_tools must be an array")
    require(isinstance(underlying, list), "Presentation underlying_operations must be an array")
    actual_read = tuple(read_operations)
    actual_write = tuple(write_operations)
    actual_underlying = tuple(underlying)
    require(actual_read == EXPECTED_READ, "fps_presentation read operation set/order drifted")
    require(actual_write == EXPECTED_WRITE, "fps_presentation write operation set/order drifted")
    require(
        set(actual_underlying) == set(EXPECTED_OPERATIONS),
        "fps_presentation underlying operation set drifted",
    )
    require(len(actual_read) == 12 and len(actual_write) == 9, "Presentation operation counts must be 12 read / 9 write")
    require(len(actual_underlying) == 21, "Presentation operation count must be 21")
    require(len(set(actual_read)) == 12, "Presentation read operations contain duplicates")
    require(len(set(actual_write)) == 9, "Presentation write operations contain duplicates")
    require(len(set(actual_underlying)) == 21, "Presentation underlying operations contain duplicates")
    require(not set(actual_read).intersection(actual_write), "Presentation read/write sets overlap")

    # No other public façade may silently claim one of the Presentation routes.
    owners: dict[str, set[str]] = {}
    for name, value in facades.items():
        if not isinstance(value, dict):
            continue
        for key in ("read_tools", "write_tools", "underlying_operations"):
            routes = value.get(key)
            if isinstance(routes, list):
                for operation in routes:
                    if operation in EXPECTED_OPERATIONS:
                        owners.setdefault(operation, set()).add(name)
    for operation in EXPECTED_OPERATIONS:
        require(
            owners.get(operation) == {FACADE},
            f"profile Presentation operation {operation} has an unexpected façade owner: {sorted(owners.get(operation, set()))}",
        )
    return actual_read, actual_write


def check_presentation_service(
    read_operations: tuple[str, ...], write_operations: tuple[str, ...]
) -> None:
    service_path = RUNTIME / "presentation_service.rs"
    service = read(service_path)
    implementation = service.split("#[cfg(test)]", 1)[0]
    require(
        "pub(crate) const PRESENTATION_READ_OPERATIONS" in implementation
        and "pub(crate) const PRESENTATION_WRITE_OPERATIONS" in implementation,
        "Presentation service operation inventory is not Runtime-owned",
    )
    require(
        rust_array(implementation, "PRESENTATION_READ_OPERATIONS") == read_operations,
        "Presentation service read inventory differs from the locked profile",
    )
    require(
        rust_array(implementation, "PRESENTATION_WRITE_OPERATIONS") == write_operations,
        "Presentation service write inventory differs from the locked profile",
    )
    invoke = rust_function(implementation, "pub(crate) fn invoke(")
    require("match operation" in invoke, "Presentation service invoke is not exhaustive")
    route_arms = re.findall(r'"([a-z0-9_]+)"\s*=>', invoke)
    for operation in EXPECTED_OPERATIONS:
        require(
            re.search(rf'"{re.escape(operation)}"\s*=>', invoke) is not None,
            f"Presentation service invoke omits typed arm {operation}",
        )
    require(
        len(route_arms) == len(EXPECTED_OPERATIONS)
        and len(set(route_arms)) == len(EXPECTED_OPERATIONS),
        "Presentation service invoke must contain exactly one typed arm per active route",
    )
    require(
        len(re.findall(r"\bruntime\.[a-zA-Z_][a-zA-Z0-9_]*\s*\(", invoke))
        >= len(EXPECTED_OPERATIONS),
        "Presentation service does not directly call a typed Runtime method for every route",
    )
    require(
        re.search(r"\bdispatch_ipc\s*\(", implementation) is None,
        "Presentation service calls the generic Runtime dispatcher",
    )
    require(
        re.search(r"_\s*=>\s*Err\s*\(\s*RuntimeError::InvalidInput", invoke) is not None,
        "Presentation service does not fail closed for unknown operations",
    )


def check_runtime_router() -> None:
    services = read(RUNTIME_SERVICES)
    router = read(RUNTIME_ROUTER)
    runtime_lib = read(RUNTIME_LIB)
    require(
        '#[path = "presentation_service.rs"]\npub(crate) mod presentation_service;' in services,
        "Presentation service is not owned by Runtime services",
    )
    require("mod presentation_service;" not in runtime_lib, "Presentation service became a Runtime root module")

    router_invoke = rust_function(router, "pub fn invoke(")
    require(
        "WeaponryServiceDomain::Presentation =>" in router_invoke
        and "presentation_service::invoke(self.runtime, operation, payload)" in router_invoke,
        "typed Runtime router does not invoke Presentation service directly",
    )
    presentation_arm = re.search(
        r"WeaponryServiceDomain::Presentation\s*=>\s*\{(?P<body>.*?)\n\s*\}",
        router_invoke,
        re.DOTALL,
    )
    require(presentation_arm is not None, "Presentation Runtime router arm is not explicit")
    require(
        re.search(r"\bdispatch_ipc\s*\(", presentation_arm.group("body")) is None,
        "Presentation Runtime router arm falls back to dispatch_ipc",
    )

    dispatch = rust_function(runtime_lib, "pub(crate) fn dispatch_ipc")
    bridge = "if runtime_services::presentation_service::is_presentation_operation(method)"
    bridge_return = "return runtime_services::presentation_service::invoke(self, method, payload);"
    require(bridge in dispatch and bridge_return in dispatch, "compatibility IPC does not bridge to Presentation service")
    require(dispatch.find(bridge) < dispatch.find("match method"), "Presentation compatibility bridge occurs after generic match")

    generic_match = match_body(dispatch)
    arms: list[str] = []
    arm_pattern = re.compile(r'(?P<names>"[a-z0-9_]+"(?:\s*\|\s*"[a-z0-9_]+")*)\s*=>')
    for arm in arm_pattern.finditer(generic_match):
        arms.extend(re.findall(r'"([a-z0-9_]+)"', arm.group("names")))
    presentation_arms = sorted(set(arms).intersection(EXPECTED_OPERATIONS))
    require(
        not presentation_arms,
        "active Presentation operations remain in Runtime::dispatch_ipc generic match: "
        + ", ".join(presentation_arms),
    )


def extract_struct_blocks(text: str, marker: str) -> list[str]:
    blocks: list[str] = []
    start = 0
    while True:
        index = text.find(marker, start)
        if index < 0:
            return blocks
        opening = text.find("{", index)
        require(opening >= 0, f"{marker!r} has no body")
        blocks.append(balanced_block(text, opening))
        start = opening + 1


def find_presentation_repository() -> tuple[Path, str]:
    """Discover the extracted repository by its public symbol, never by name."""

    candidates: list[tuple[Path, str]] = []
    for path in sorted(STORE.glob("*.rs")):
        if path == STORE_ROOT:
            continue
        text = read(path)
        if re.search(r"\bpub\s+struct\s+PresentationRepository\s*<\s*'store\s*>", text):
            candidates.append((path, text))
    require(
        len(candidates) == 1,
        "expected exactly one extracted file exporting PresentationRepository<'store>; found "
        + ", ".join(relative(path) for path, _ in candidates),
    )
    return candidates[0]


def public_names(repository: str, kind: str) -> tuple[str, ...]:
    if kind == "const":
        pattern = r"^\s*pub\s+const\s+([A-Z][A-Z0-9_]*)\b"
    else:
        pattern = r"^\s*pub\s+fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\("
    return tuple(re.findall(pattern, repository, re.MULTILINE))


def check_store() -> dict[str, object]:
    store_root = read(STORE_ROOT)
    repository_path, repository = find_presentation_repository()
    module = repository_path.stem
    require(
        re.search(rf"\bpub\s+mod\s+{re.escape(module)}\s*;", store_root) is not None,
        f"Store root does not register discovered Presentation module {module}",
    )
    require(
        re.search(
            rf"\bpub\s+use\s+{re.escape(module)}\s*::[^;]*\bPresentationRepository\b",
            store_root,
        )
        is not None,
        "Store root does not re-export discovered PresentationRepository",
    )

    repository_struct = struct_body(repository, "pub struct PresentationRepository")
    fields = re.findall(r"^\s*(\w+)\s*:\s*([^,]+),", repository_struct, re.MULTILINE)
    require(
        any(
            name == "store"
            and "&'storeStore" in re.sub(r"\s+", "", type_text)
            for name, type_text in fields
        ),
        "PresentationRepository must borrow &'store Store",
    )
    forbidden_field = re.compile(
        r"\b(?:connection|migration|cas|cas_root|sqlite|transaction)\b|"
        r"(?:rusqlite::)?Connection|CasStore|CasRoot|Migration",
        re.IGNORECASE,
    )
    require(
        not any(forbidden_field.search(f"{name}: {type_text}") for name, type_text in fields),
        "PresentationRepository owns a Connection, migration, transaction, or CAS field",
    )

    constants = public_names(repository, "const")
    methods = public_names(repository, "fn")
    require(methods, "discovered PresentationRepository exposes no public aggregate methods")
    # A repository method must have a real aggregate-shaped name.  This keeps
    # a marker-only or empty façade from being counted as physical extraction.
    require(
        any(re.match(r"(?:record|commit|get|list|discard|read|write|ensure|insert)_", name) for name in methods),
        "PresentationRepository public methods do not identify a moved aggregate",
    )

    # A compatibility façade may retain a public Store method, but it must be
    # a one-line delegate.  Any second implementation in Store::impl would be
    # a second physical owner of the aggregate.
    for method in methods:
        root_method = re.search(
            rf"^\s*pub\s+fn\s+{re.escape(method)}\s*\(", store_root, re.MULTILINE
        )
        if root_method is None:
            continue
        root_body = rust_function(store_root, root_method.group(0).strip())
        require(
            "self.presentation_repository()" in root_body
            and re.search(rf"\.\s*{re.escape(method)}\s*\(", root_body) is not None,
            f"Store root method {method} is not a PresentationRepository delegate",
        )

    accessor_pattern = r"\bpub\s+fn\s+presentation_repository\s*\(\s*&self\s*\)"
    accessor_locations = []
    if re.search(accessor_pattern, repository):
        accessor_locations.append("repository")
    if re.search(accessor_pattern, store_root):
        accessor_locations.append("store_root")
    require(accessor_locations, "Store lacks a PresentationRepository accessor")
    accessor_text = repository if "repository" in accessor_locations else store_root
    accessor_match = re.search(accessor_pattern, accessor_text)
    require(accessor_match is not None, "PresentationRepository accessor could not be located")
    accessor_body = rust_function(accessor_text, accessor_match.group(0))
    require(
        "PresentationRepository::new(self)" in accessor_body or "Self::new(self)" in accessor_body,
        "Store::presentation_repository does not construct the borrowed repository",
    )

    # Confirm that the discovered module owns a real durable aggregate rather
    # than only an empty marker: it must carry its typed record, table and a
    # Store-backed read/write path.
    require(
        "MechanicalAnimationClipLinkRecord" in repository
        and "mechanical_animation_clip_links" in repository
        and "lock_connection" in repository,
        "discovered Presentation repository does not contain a real Store aggregate implementation",
    )

    # Public constants are reported when present, but are not required: a
    # legitimate aggregate can be represented entirely by typed methods.
    return {
        "file": relative(repository_path),
        "module": module,
        "public_constants": list(constants),
        "public_methods": list(methods),
        "accessor": accessor_locations[0],
    }


def rust_array_literal_or_named(
    text: str, field: str, declarations: str | None = None
) -> tuple[str, ...]:
    """Resolve a mapping operation field from a named or inline string array."""

    named = re.search(rf"\b{re.escape(field)}\s*:\s*([A-Z][A-Z0-9_]*)\s*,", text)
    if named:
        return rust_array(declarations if declarations is not None else text, named.group(1))
    inline = re.search(rf"\b{re.escape(field)}\s*:\s*&\[(?P<body>.*?)\],", text, re.DOTALL)
    require(inline is not None, f"mapping has no resolvable {field} array")
    return tuple(re.findall(r'"([^"\\]*)"', inline.group("body")))


def check_contract_map() -> dict[str, object]:
    contract_map = read(CONTRACT_MAP)
    start = contract_map.find("pub const KNIFE_CAPABILITY_MAPPINGS")
    require(start >= 0, "central Contract map omits KNIFE_CAPABILITY_MAPPINGS")
    end = contract_map.find("\n];", start)
    require(end >= 0, "central Contract mapping array is unterminated")
    mapping_source = contract_map[start : end + 3]
    blocks = extract_struct_blocks(mapping_source, "WeaponryCapabilityMapping {")
    require(blocks, "central Contract map has no capability mappings")

    operation_owners: dict[str, list[tuple[str, str, str]]] = {}
    presentation_mappings: list[str] = []
    for block in blocks:
        capability_match = re.search(r'\bcapability\s*:\s*"([^"]+)"', block)
        domain_match = re.search(r"\bdomain\s*:\s*WeaponryServiceDomain::(\w+)", block)
        facade_match = re.search(r'\bmcp_facade\s*:\s*Some\("([^"]+)"\)', block)
        require(capability_match and domain_match, "central capability mapping lacks capability/domain")
        capability = capability_match.group(1)
        domain = domain_match.group(1)
        facade = facade_match.group(1) if facade_match else ""
        operations = rust_array_literal_or_named(block, "mcp_operations", contract_map)
        require(operations, f"central capability mapping {capability} has no operations")
        if domain == "Presentation" or facade == FACADE:
            require(
                domain == "Presentation" and facade == FACADE,
                f"Presentation capability {capability} has an inconsistent domain/façade",
            )
            require(
                set(operations) <= set(EXPECTED_OPERATIONS),
                f"Presentation capability {capability} exposes an operation outside fps_presentation",
            )
        if domain == "Presentation" or facade == FACADE or set(operations).intersection(EXPECTED_OPERATIONS):
            presentation_mappings.append(capability)
        for operation in operations:
            operation_owners.setdefault(operation, []).append((capability, domain, facade))

    require(
        len(presentation_mappings) >= 1,
        "central Contract map has no Presentation capability mapping",
    )
    for operation in EXPECTED_OPERATIONS:
        owners = operation_owners.get(operation, [])
        require(
            len(owners) == 1,
            f"central Contract map must own {operation} exactly once; found {owners}",
        )
        capability, domain, facade = owners[0]
        require(domain == "Presentation", f"{operation} maps to domain {domain}, not Presentation")
        require(facade == FACADE, f"{operation} maps to façade {facade}, not {FACADE}")

    return {
        "presentation_capabilities": len(set(presentation_mappings)),
        "mapped_operations": len(EXPECTED_OPERATIONS),
    }


def main() -> int:
    read_operations, write_operations = check_profile()
    check_presentation_service(read_operations, write_operations)
    check_runtime_router()
    store = check_store()
    contract = check_contract_map()
    print(
        json.dumps(
            {
                "schema_version": "WeaponryPresentationArchitectureCheck@1",
                "status": "PASS",
                "presentation_facade": FACADE,
                "active_presentation_operations": len(read_operations) + len(write_operations),
                "active_read_operations": len(read_operations),
                "active_write_operations": len(write_operations),
                "runtime_router": "typed_presentation_service",
                "compatibility_bridge": "presentation_service_reused",
                "legacy_active_match_arms_remaining": 0,
                "store_repository": "borrowed_single_owner",
                "store_extraction": store,
                "central_contract_map": contract,
                "unextracted_gaps": [
                    "remaining Presentation package/camera/socket/clip families outside the discovered aggregate",
                    "compatibility/archive retirement and consumer-zero replay remain pending",
                ],
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
