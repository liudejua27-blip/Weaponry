#!/usr/bin/env python3
"""Fail-closed source gate for WPN-ARCH-DELIVERY-001.

The Delivery domain is the first Runtime/Store extraction slice for the
Weaponry knife profile.  This checker intentionally audits source ownership
and routing only.  It does not build the Rust crates and does not claim that
the remaining approval, socket, schema, visual, or commercial gates pass.

The active Delivery domain has two MCP façades: ``delivery`` (six operations)
and ``approval`` (five operations).  Runtime owns the typed service dispatch;
the old IPC dispatcher is only a compatibility bridge.  Store owns one
borrowed ``DeliveryRepository<'store>`` for the extracted
``GameAssetDeliveryLinkRecord`` aggregate, while migration ownership remains
singular in ``Store::migrate``.
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
STORE_BOUNDARIES = STORE / "repository_boundaries.rs"
CONTRACT_MAP = (
    ROOT / "apps/desktop/src-tauri/crates/forgecad-contracts/src/weaponry_domain_map.rs"
)

DELIVERY_FACADE = "delivery"
APPROVAL_FACADE = "approval"

# These tuples are the locked active profile split.  The Runtime service has
# one domain inventory, so its arrays combine the two façade inventories.
DELIVERY_READ = (
    "game_asset_delivery_get",
    "game_asset_lod_derive",
    "game_weapon_glb_socket_get",
)
DELIVERY_WRITE = (
    "export_prepare",
    "game_asset_delivery_prepare",
    "game_weapon_glb_socket_prepare",
)
APPROVAL_READ = ("version_diff",)
APPROVAL_WRITE = (
    "candidate_confirm",
    "candidate_reject",
    "cross_view_promotion_confirm",
    "export_confirm",
)
EXPECTED_READ = DELIVERY_READ + APPROVAL_READ
EXPECTED_WRITE = DELIVERY_WRITE + APPROVAL_WRITE
EXPECTED_DELIVERY_OPERATIONS = DELIVERY_READ + DELIVERY_WRITE
EXPECTED_APPROVAL_OPERATIONS = APPROVAL_READ + APPROVAL_WRITE
EXPECTED_OPERATIONS = EXPECTED_READ + EXPECTED_WRITE
EXPECTED_CAPABILITIES = {
    "game_asset_delivery",
    "game_asset_lod",
    "game_weapon_glb_socket",
    "export_prepare",
    "version_diff",
    "candidate_confirm",
    "candidate_reject",
    "cross_view_promotion",
    "export_confirm",
}


def fail(message: str) -> None:
    raise SystemExit(f"WPN-ARCH-DELIVERY-001 FAIL: {message}")


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
    return ""


def balanced_block(text: str, opening: int) -> str:
    """Return a Rust brace block while ignoring comments and literals."""

    require(opening >= 0 and opening < len(text) and text[opening] == "{", "invalid Rust block start")
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
            # Lifetimes/labels start with an identifier.  A non-identifier
            # character starts a character literal instead.
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


def rust_function_at(text: str, start: int) -> str:
    opening = text.find("{", start)
    require(opening >= 0, "Rust function has no body")
    return balanced_block(text, opening)


def struct_body(text: str, marker: str) -> str:
    start = text.find(marker)
    require(start >= 0, f"missing Rust block marker {marker!r}")
    opening = text.find("{", start)
    require(opening >= 0, f"block {marker!r} has no body")
    return balanced_block(text, opening)


def match_body(text: str, marker: str = "match method") -> str:
    start = text.find(marker)
    require(start >= 0, f"missing {marker!r}")
    opening = text.find("{", start)
    require(opening >= 0, f"{marker!r} has no body")
    return balanced_block(text, opening)


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


def rust_array(text: str, constant: str) -> tuple[str, ...]:
    """Resolve exactly one Rust ``&[&str]`` declaration."""

    pattern = re.compile(
        rf"(?:pub(?:\([^)]*\))?\s+)?const\s+{re.escape(constant)}\s*"
        r":\s*&\[\s*&str\s*\]\s*=\s*&\[(?P<body>.*?)\];",
        re.DOTALL,
    )
    matches = list(pattern.finditer(text))
    require(matches, f"missing Rust operation array {constant}")
    require(len(matches) == 1, f"duplicate Rust operation array {constant}")
    return tuple(re.findall(r'"([^"\\]*)"', matches[0].group("body")))


def rust_array_literal_or_named(
    text: str, field: str, declarations: str | None = None
) -> tuple[str, ...]:
    """Resolve a mapping operation field from a named or inline string array."""

    named = re.search(
        rf"\b{re.escape(field)}\s*:\s*([A-Z][A-Z0-9_]*)\s*,", text
    )
    if named:
        return rust_array(declarations if declarations is not None else text, named.group(1))
    inline = re.search(
        rf"\b{re.escape(field)}\s*:\s*&\[(?P<body>.*?)\],", text, re.DOTALL
    )
    require(inline is not None, f"mapping has no resolvable {field} array")
    return tuple(re.findall(r'"([^"\\]*)"', inline.group("body")))


def check_profile() -> dict[str, object]:
    try:
        profile = json.loads(read(PROFILE))
    except json.JSONDecodeError as exc:
        fail(f"invalid profile JSON: {exc}")
    require(isinstance(profile, dict), "profile must contain an object")
    facades = profile.get("facades")
    require(isinstance(facades, dict), "profile.facades must be an object")

    def check_facade(name: str, expected_read: tuple[str, ...], expected_write: tuple[str, ...]) -> tuple[str, ...]:
        value = facades.get(name)
        require(isinstance(value, dict), f"profile omits {name} façade")
        require(value.get("facade_name") == name, f"{name} façade name drifted")
        require(value.get("classification") == "read-write", f"{name} façade classification drifted")
        require(value.get("default_enabled") is True, f"{name} façade is not default-enabled")
        read_tools = value.get("read_tools")
        write_tools = value.get("write_tools")
        underlying = value.get("underlying_operations")
        require(isinstance(read_tools, list), f"{name} read_tools must be an array")
        require(isinstance(write_tools, list), f"{name} write_tools must be an array")
        require(isinstance(underlying, list), f"{name} underlying_operations must be an array")
        actual_read = tuple(read_tools)
        actual_write = tuple(write_tools)
        actual_underlying = tuple(underlying)
        require(actual_read == expected_read, f"{name} read operation set/order drifted")
        require(actual_write == expected_write, f"{name} write operation set/order drifted")
        require(set(actual_underlying) == set(expected_read + expected_write), f"{name} underlying operation set drifted")
        require(len(actual_underlying) == len(expected_read) + len(expected_write), f"{name} underlying operations contain duplicates")
        require(len(set(actual_read)) == len(actual_read), f"{name} read operations contain duplicates")
        require(len(set(actual_write)) == len(actual_write), f"{name} write operations contain duplicates")
        require(not set(actual_read).intersection(actual_write), f"{name} read/write sets overlap")
        return actual_underlying

    delivery_underlying = check_facade(DELIVERY_FACADE, DELIVERY_READ, DELIVERY_WRITE)
    approval_underlying = check_facade(APPROVAL_FACADE, APPROVAL_READ, APPROVAL_WRITE)

    # No other public façade may silently claim an active Delivery route.
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
        expected_owner = DELIVERY_FACADE if operation in EXPECTED_DELIVERY_OPERATIONS else APPROVAL_FACADE
        require(
            owners.get(operation) == {expected_owner},
            f"profile Delivery operation {operation} has an unexpected façade owner: {sorted(owners.get(operation, set()))}",
        )

    require(len(DELIVERY_READ) + len(APPROVAL_READ) == 4, "Delivery domain must expose exactly four reads")
    require(len(DELIVERY_WRITE) + len(APPROVAL_WRITE) == 7, "Delivery domain must expose exactly seven writes")
    require(len(delivery_underlying) == 6, "delivery façade must expose exactly six operations")
    require(len(approval_underlying) == 5, "approval façade must expose exactly five operations")
    require(len(set(EXPECTED_OPERATIONS)) == 11, "Delivery domain must expose exactly eleven unique operations")
    return {
        "read": EXPECTED_READ,
        "write": EXPECTED_WRITE,
        "delivery_facade_operations": len(delivery_underlying),
        "approval_facade_operations": len(approval_underlying),
    }


def check_delivery_service(profile: dict[str, object]) -> dict[str, object]:
    service_path = RUNTIME / "delivery_service.rs"
    service = read(service_path)
    implementation = service.split("#[cfg(test)]", 1)[0]

    require(
        "pub(crate) const DELIVERY_READ_OPERATIONS" in implementation
        and "pub(crate) const DELIVERY_WRITE_OPERATIONS" in implementation,
        "Delivery service operation inventory is not Runtime-owned",
    )
    expected_read = tuple(profile["read"])
    expected_write = tuple(profile["write"])
    require(
        rust_array(implementation, "DELIVERY_READ_OPERATIONS") == expected_read,
        "Delivery service read inventory differs from the locked profile",
    )
    require(
        rust_array(implementation, "DELIVERY_WRITE_OPERATIONS") == expected_write,
        "Delivery service write inventory differs from the locked profile",
    )
    invoke = rust_function(implementation, "pub(crate) fn invoke(")
    require("match operation" in invoke, "Delivery service invoke is not exhaustive")
    route_arms = re.findall(r'"([a-z0-9_]+)"\s*=>', invoke)
    for operation in EXPECTED_OPERATIONS:
        require(
            re.search(rf'"{re.escape(operation)}"\s*=>', invoke) is not None,
            f"Delivery service invoke omits typed arm {operation}",
        )
    require(
        len(route_arms) == len(EXPECTED_OPERATIONS)
        and len(set(route_arms)) == len(EXPECTED_OPERATIONS)
        and set(route_arms) == set(EXPECTED_OPERATIONS),
        "Delivery service invoke must contain exactly one typed arm per active route",
    )
    direct_calls = re.findall(r"\bruntime\.[A-Za-z_][A-Za-z0-9_]*\s*\(", invoke)
    require(
        len(direct_calls) >= len(EXPECTED_OPERATIONS),
        "Delivery service does not directly call a typed Runtime method for every route",
    )
    require(
        re.search(r"\bdispatch_ipc\s*\(", implementation) is None,
        "Delivery service calls the generic Runtime dispatcher",
    )
    require(
        re.search(r"_\s*=>\s*Err\s*\(\s*RuntimeError::InvalidInput", invoke) is not None,
        "Delivery service does not fail closed for unknown operations",
    )
    return {
        "file": relative(service_path),
        "read_operations": len(expected_read),
        "write_operations": len(expected_write),
        "typed_route_arms": len(route_arms),
    }


def check_runtime_services() -> None:
    services = read(RUNTIME_SERVICES)
    runtime_lib = read(RUNTIME_LIB)
    require(
        '#[path = "delivery_service.rs"]\npub(crate) mod delivery_service;' in services,
        "Delivery service is not owned by Runtime services",
    )
    require(
        re.search(r"\bmod\s+delivery_service\s*;", runtime_lib) is None,
        "Delivery service became a Runtime root module",
    )
    require(
        "const DELIVERY_READ_OPERATIONS: &[&str] = delivery_service::DELIVERY_READ_OPERATIONS;" in services,
        "Runtime service boundary does not reuse Delivery read inventory",
    )
    require(
        "const DELIVERY_WRITE_OPERATIONS: &[&str] = delivery_service::DELIVERY_WRITE_OPERATIONS;" in services,
        "Runtime service boundary does not reuse Delivery write inventory",
    )
    delivery_boundaries = [
        block
        for block in extract_struct_blocks(services, "RuntimeServiceBoundary {")
        if "domain: RuntimeServiceDomain::Delivery" in block
    ]
    require(len(delivery_boundaries) == 1, "Runtime service boundaries must contain one Delivery entry")
    boundary = delivery_boundaries[0]
    require("facade_names: DELIVERY_FACADES" in boundary, "Delivery service boundary façade projection drifted")
    require("read_operations: DELIVERY_READ_OPERATIONS" in boundary, "Delivery service boundary read projection drifted")
    require("write_operations: DELIVERY_WRITE_OPERATIONS" in boundary, "Delivery service boundary write projection drifted")
    require(
        re.search(r"define_runtime_service!\s*\(\s*DeliveryService\s*,\s*Delivery\s*,\s*4\s*\)", services)
        is not None,
        "DeliveryService is not bound to the Delivery boundary slot",
    )


def check_runtime_router() -> dict[str, object]:
    router = read(RUNTIME_ROUTER)
    runtime_lib = read(RUNTIME_LIB)
    router_invoke = rust_function(router, "pub fn invoke(")
    require(
        "WeaponryServiceDomain::Delivery =>" in router_invoke
        and "delivery_service::invoke(self.runtime, operation, payload)" in router_invoke,
        "typed Runtime router does not invoke Delivery service directly",
    )
    delivery_arm = re.search(
        r"WeaponryServiceDomain::Delivery\s*=>\s*\{(?P<body>.*?)\n\s*\}",
        router_invoke,
        re.DOTALL,
    )
    require(delivery_arm is not None, "Delivery Runtime router arm is not explicit")
    require(
        re.search(r"\bdispatch_ipc\s*\(", delivery_arm.group("body")) is None,
        "Delivery Runtime router arm falls back to dispatch_ipc",
    )

    dispatch = rust_function(runtime_lib, "pub(crate) fn dispatch_ipc")
    bridge = re.search(
        r"(?m)^\s*if\s+runtime_services::delivery_service::is_delivery_operation\(method\)\s*\{",
        dispatch,
    )
    bridge_return = re.search(
        r"(?m)^\s*return\s+runtime_services::delivery_service::invoke\(self,\s*method,\s*payload\);",
        dispatch,
    )
    require(bridge is not None and bridge_return is not None, "compatibility IPC does not bridge to Delivery service")
    generic_start = dispatch.find("match method")
    require(generic_start >= 0, "Runtime::dispatch_ipc has no generic method match")
    require(bridge.start() < generic_start, "Delivery compatibility bridge occurs after generic match")

    generic_match = match_body(dispatch, "match method")
    arms: list[str] = []
    arm_pattern = re.compile(r'(?P<names>"[a-z0-9_]+"(?:\s*\|\s*"[a-z0-9_]+")*)\s*=>')
    for arm in arm_pattern.finditer(generic_match):
        arms.extend(re.findall(r'"([a-z0-9_]+)"', arm.group("names")))
    legacy_arms = sorted(set(arms).intersection(EXPECTED_OPERATIONS))
    require(
        not legacy_arms,
        "active Delivery operations remain in Runtime::dispatch_ipc generic match: "
        + ", ".join(legacy_arms),
    )
    return {
        "router": "typed_delivery_service",
        "compatibility_bridge": "delivery_service_reused",
        "legacy_active_match_arms_remaining": len(legacy_arms),
    }


def find_delivery_repository() -> tuple[Path, str]:
    """Discover the physical repository by symbol, not a guessed filename."""

    candidates: list[tuple[Path, str]] = []
    for path in sorted(STORE.glob("*.rs")):
        if path == STORE_ROOT:
            continue
        text = read(path)
        if re.search(r"\bpub\s+struct\s+DeliveryRepository\s*<\s*'store\s*>", text):
            candidates.append((path, text))
    require(
        len(candidates) == 1,
        "expected exactly one extracted file exporting DeliveryRepository<'store>; found "
        + ", ".join(relative(path) for path, _ in candidates),
    )
    return candidates[0]


def store_impl_blocks(text: str) -> list[str]:
    blocks: list[str] = []
    pattern = re.compile(r"\bimpl\s+Store\s*\{")
    for match in pattern.finditer(text):
        opening = text.find("{", match.start())
        require(opening >= 0, "impl Store has no body")
        blocks.append(balanced_block(text, opening))
    return blocks


def methods_in_store_impls(text: str, method: str) -> list[str]:
    bodies: list[str] = []
    for block in store_impl_blocks(text):
        for match in re.finditer(
            rf"\bpub\s+fn\s+{re.escape(method)}\s*\(", block
        ):
            bodies.append(rust_function_at(block, match.start()))
    return bodies


def check_store() -> dict[str, object]:
    store_root = read(STORE_ROOT)
    repository_path, repository = find_delivery_repository()
    module = repository_path.stem
    require(
        re.search(rf"\bpub\s+mod\s+{re.escape(module)}\s*;", store_root) is not None,
        f"Store root does not register discovered Delivery module {module}",
    )
    require(
        re.search(
            rf"\bpub\s+use\s+{re.escape(module)}\s*::[^;]*\bDeliveryRepository\b",
            store_root,
        )
        is not None,
        "Store root does not re-export discovered DeliveryRepository",
    )

    repository_struct = struct_body(repository, "pub struct DeliveryRepository")
    fields = re.findall(r"^\s*(\w+)\s*:\s*([^,]+),", repository_struct, re.MULTILINE)
    require(
        any(
            name == "store" and "&'storeStore" in re.sub(r"\s+", "", type_text)
            for name, type_text in fields
        ),
        "DeliveryRepository must borrow &'store Store",
    )
    forbidden_field = re.compile(
        r"\b(?:connection|conn|migration|migrations|cas(?:[_a-z0-9]*)|sqlite(?:[_a-z0-9]*)|transaction(?:[_a-z0-9]*)|tx)\b|"
        r"(?:rusqlite::)?Connection|CasStore|CasRoot|Migration",
        re.IGNORECASE,
    )
    require(
        not any(forbidden_field.search(f"{name}: {type_text}") for name, type_text in fields),
        "DeliveryRepository owns a Connection, migration, transaction, or CAS field",
    )

    repository_impl = struct_body(repository, "impl<'store> DeliveryRepository<'store>")
    aggregate_methods = (
        "record_game_asset_delivery_link",
        "get_game_asset_delivery_link",
        "list_game_asset_delivery_links",
    )
    for method in aggregate_methods:
        require(
            len(methods_in_store_impls(repository_impl, method)) == 0,
            f"DeliveryRepository impl parser unexpectedly found nested Store impl for {method}",
        )
        require(
            re.search(rf"\bpub\s+fn\s+{re.escape(method)}\s*\(", repository_impl) is not None,
            f"DeliveryRepository omits aggregate method {method}",
        )
    require(
        "GameAssetDeliveryLinkRecord" in repository
        and "game_asset_delivery_links" in repository
        and "lock_connection" in repository
        and "INSERT INTO game_asset_delivery_links" in repository
        and ("mark_reachable_in_transaction" in repository or "read_verified_bounded" in repository),
        "discovered Delivery repository does not contain a real Store aggregate implementation",
    )
    repository_production = repository.split("#[cfg(test)]", 1)[0]
    require(
        not re.search(
            r"\b(?:fn\s+(?:migrate|ensure_schema|ensure_table|bootstrap_schema)|Store::migrate\s*\(|CREATE\s+TABLE|ALTER\s+TABLE|DROP\s+TABLE)\b",
            repository_production,
            re.IGNORECASE,
        ),
        "DeliveryRepository owns an independent migration or DDL entry point",
    )

    # Every public Store-root aggregate API is retained only as a delegate.
    # Enumerating every impl Store block avoids mistaking a test helper or an
    # unrelated impl for the compatibility owner.
    shim_locations: dict[str, int] = {}
    for method in aggregate_methods:
        root_bodies = methods_in_store_impls(store_root, method)
        repo_bodies = methods_in_store_impls(repository, method)
        bodies = root_bodies + repo_bodies
        require(bodies, f"no Store compatibility API found for {method}")
        shim_locations[method] = len(bodies)
        for body in bodies:
            require(
                "self.delivery_repository()" in body
                and re.search(rf"\.\s*{re.escape(method)}\s*\(", body) is not None,
                f"Store root method {method} is not a DeliveryRepository delegate",
            )
            require(
                not re.search(r"lock_connection|transaction|INSERT\s+INTO|SELECT\s+", body, re.IGNORECASE),
                f"Store compatibility method {method} contains a second aggregate implementation",
            )

    # Helpers and the production insert path must not remain duplicated in
    # the root.  The root may still contain the migration DDL and unrelated
    # socket compatibility reads.
    root_production = store_root.split("#[cfg(test)]", 1)[0]
    duplicate_helpers = (
        "validate_game_asset_delivery_link",
        "game_asset_delivery_link_from_row",
        "same_game_asset_delivery_link",
        "game_asset_delivery_json_hashes",
        "game_asset_delivery_reachable_hashes",
        "validate_game_asset_delivery_bindings_in_transaction",
        "verify_delivery_objects",
    )
    for helper in duplicate_helpers:
        require(
            re.search(rf"^\s*(?:pub\s+)?fn\s+{re.escape(helper)}\s*\(", root_production, re.MULTILINE) is None,
            f"Store root still defines extracted Delivery helper {helper}",
        )
    require(
        "INSERT INTO game_asset_delivery_links" not in root_production,
        "Store root still owns the game_asset_delivery_links write path",
    )

    return {
        "file": relative(repository_path),
        "module": module,
        "aggregate": "GameAssetDeliveryLinkRecord",
        "repository_lifetime": "borrowed &'store Store",
        "store_api_shims": shim_locations,
    }


def check_migration_and_gaps(contract_map: str) -> dict[str, object]:
    boundaries = read(STORE_BOUNDARIES)
    require(
        re.search(
            r"pub\s+const\s+STORE_MIGRATION_SEQUENCE\s*:\s*&\[\s*&str\s*\]\s*=\s*&\[\s*STORE_MIGRATION_SOURCE\s*\];",
            boundaries,
        )
        is not None,
        "Store migration sequence is not the single runtime-v1 source",
    )
    require(
        'pub const STORE_MIGRATION_OWNER: &str = "forgecad-store::Store::migrate";' in boundaries,
        "Store migration owner constant drifted",
    )
    require(
        'pub const STORE_MIGRATION_SOURCE: &str = "migrations-runtime-v1/0001_runtime.sql";' in boundaries,
        "Store migration source constant drifted",
    )

    delivery_boundaries = [
        block
        for block in extract_struct_blocks(boundaries, "StoreRepositoryBoundary {")
        if "domain: StoreRepositoryDomain::Delivery" in block
    ]
    require(len(delivery_boundaries) == 1, "Store ownership directory must contain one Delivery boundary")
    delivery_boundary = delivery_boundaries[0]
    require(
        "migration_owner: STORE_MIGRATION_OWNER" in delivery_boundary
        and "migration_source: STORE_MIGRATION_SOURCE" in delivery_boundary,
        "Delivery boundary does not reuse the single Store migration owner/source",
    )
    require(
        "implementation_modules: DELIVERY_IMPLEMENTATION_MODULES" in delivery_boundary
        and "physical_first_slice_delivery_repository_game_asset_delivery_link_and_approval_lifecycle;socket_records_not_extracted"
        in delivery_boundary,
        "Delivery boundary does not record the bounded physical extraction",
    )
    require(
        "src/delivery_repository.rs" in boundaries
        and "GameAssetDeliveryLinkRecord" in boundaries,
        "Delivery implementation inventory does not name the extracted aggregate",
    )

    extracted = rust_array(boundaries, "DELIVERY_REPOSITORY_EXTRACTED_RECORD_FAMILIES")
    require(extracted == (
        "GameAssetDeliveryLinkRecord (record/get/list/commit; game_asset_delivery_links)",
        "ApprovalReceiptRecord / DesignAssetVersionRecord / ExportManifestRecord (ApprovalLifecycle; approval_repository.rs)",
    ), "Delivery extracted-record inventory drifted")
    unextracted = rust_array(boundaries, "DELIVERY_REPOSITORY_UNEXTRACTED_RECORD_FAMILIES")
    require(unextracted, "Delivery boundary hides all remaining record families")
    require(any("Socket" in item or "socket" in item for item in unextracted), "socket aggregate gap is not explicit")

    # There must be one physical migration function for this Store crate.  A
    # repository may use a borrowed Connection argument for a transaction,
    # but it cannot introduce a second migration entry point or DDL owner.
    migration_locations: list[str] = []
    for path in sorted(STORE.glob("*.rs")):
        source = read(path)
        matches = list(re.finditer(r"^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+migrate\s*\(", source, re.MULTILINE))
        migration_locations.extend(relative(path) for _ in matches)
        if path != STORE_ROOT:
            non_test = source.split("#[cfg(test)]", 1)[0]
            require(
                not re.search(r"\b(?:fn\s+migrate|Store::migrate\s*\()", non_test),
                f"non-root Store module {relative(path)} owns a migration entry point",
            )
    require(
        migration_locations == [relative(STORE_ROOT)],
        "Store migration must have exactly one owner in Store::migrate",
    )

    require(
        "all eleven Delivery operations" in contract_map
        and "not request-schema debt" in contract_map,
        "central Contract map does not record closed Delivery request schemas",
    )

    remaining_gaps = [
        "socket: GameWeaponAnchorLinkRecord and GLB socket materialization sidecars remain compatibility implementations",
        "mapping: Delivery capability rows remain Partial until the remaining Runtime/Store/CAS families are physically aligned",
    ]
    return {
        "migration_owner": "forgecad-store::Store::migrate",
        "migration_sources": ["migrations-runtime-v1/0001_runtime.sql"],
        "extracted_record_families": list(extracted),
        "unextracted_record_families": list(unextracted),
        "remaining_gaps": remaining_gaps,
    }


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
    delivery_mappings: list[tuple[str, str, tuple[str, ...], str]] = []
    for block in blocks:
        capability_match = re.search(r'\bcapability\s*:\s*"([^"]+)"', block)
        domain_match = re.search(r"\bdomain\s*:\s*WeaponryServiceDomain::(\w+)", block)
        facade_match = re.search(r'\bmcp_facade\s*:\s*Some\("([^"]+)"\)', block)
        status_match = re.search(r"\bstatus\s*:\s*MappingStatus::(\w+)", block)
        require(capability_match and domain_match and status_match, "central capability mapping lacks ownership fields")
        capability = capability_match.group(1)
        domain = domain_match.group(1)
        facade = facade_match.group(1) if facade_match else ""
        status = status_match.group(1)
        operations = rust_array_literal_or_named(block, "mcp_operations", contract_map)
        require(operations, f"central capability mapping {capability} has no operations")
        for operation in operations:
            operation_owners.setdefault(operation, []).append((capability, domain, facade))
        if domain == "Delivery":
            require(
                facade in (DELIVERY_FACADE, APPROVAL_FACADE),
                f"Delivery capability {capability} has an unexpected façade {facade}",
            )
            require(status == "Partial", f"Delivery capability {capability} must remain Partial")
            delivery_mappings.append((capability, facade, operations, status))

    require(len(delivery_mappings) == 9, "central Contract map must contain exactly nine Delivery capability mappings")
    require(
        {capability for capability, _, _, _ in delivery_mappings} == EXPECTED_CAPABILITIES,
        "central Delivery capability mapping set drifted",
    )
    mapped_delivery_operations: list[str] = []
    for capability, facade, operations, _ in delivery_mappings:
        expected_facade = DELIVERY_FACADE if set(operations).issubset(set(EXPECTED_DELIVERY_OPERATIONS)) else APPROVAL_FACADE
        require(facade == expected_facade, f"Delivery capability {capability} has an inconsistent façade")
        mapped_delivery_operations.extend(operations)
    require(
        set(mapped_delivery_operations) == set(EXPECTED_OPERATIONS)
        and len(mapped_delivery_operations) == len(EXPECTED_OPERATIONS),
        "Delivery capability mappings must cover the eleven operations exactly once",
    )

    # Every active operation has one and only one central capability owner,
    # and that owner must carry the Delivery domain and exact façade.
    for operation in EXPECTED_OPERATIONS:
        owners = operation_owners.get(operation, [])
        require(
            len(owners) == 1,
            f"central Contract map must own {operation} exactly once; found {owners}",
        )
        _, domain, facade = owners[0]
        expected_facade = DELIVERY_FACADE if operation in EXPECTED_DELIVERY_OPERATIONS else APPROVAL_FACADE
        require(domain == "Delivery", f"{operation} maps to domain {domain}, not Delivery")
        require(facade == expected_facade, f"{operation} maps to façade {facade}, not {expected_facade}")

    return {
        "delivery_capabilities": len(delivery_mappings),
        "partial_capabilities": sum(status == "Partial" for _, _, _, status in delivery_mappings),
        "mapped_operations": len(EXPECTED_OPERATIONS),
        "unique_operation_owners": len(EXPECTED_OPERATIONS),
    }


def main() -> int:
    profile = check_profile()
    service = check_delivery_service(profile)
    check_runtime_services()
    router = check_runtime_router()
    store = check_store()
    contract_map_text = read(CONTRACT_MAP)
    contract = check_contract_map()
    gaps = check_migration_and_gaps(contract_map_text)
    print(
        json.dumps(
            {
                "schema_version": "WeaponryDeliveryArchitectureCheck@1",
                "status": "PASS",
                "delivery_domain": "Delivery",
                "active_delivery_operations": len(EXPECTED_OPERATIONS),
                "active_read_operations": len(EXPECTED_READ),
                "active_write_operations": len(EXPECTED_WRITE),
                "delivery_facade_operations": profile["delivery_facade_operations"],
                "approval_facade_operations": profile["approval_facade_operations"],
                "delivery_service": service,
                **router,
                "store_repository": "borrowed_single_owner",
                "store_extraction": store,
                "central_contract_map": contract,
                **gaps,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
