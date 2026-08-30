#!/usr/bin/env python3
"""Audit the active Weaponry request-schema closure.

The active operation inventory is deliberately derived from the checked-in
knife profile.  The Contract domain map contributes façade/domain ownership
where a capability mapping exists, but it is not treated as a second active
operation registry: it is intentionally partial while the five domains are
being extracted.  Request schemas are discovered from the real Contract
manifest and the files it names.  The MCP source is read separately so this
gate can distinguish a direct package binding, an explicit compatibility-safe
alias, an active_schema.rs embedded/builtin closed schema, and an unresolved
operation.  It never copies the active operation table into this checker.

This is an inventory and drift gate, not proof that the Runtime
implementation, visual quality, human review, engine import, or commercial
acceptance gates pass.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import shutil
import tempfile
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CONFIG = ROOT / "config/weaponry/active-operation-request-schema-closure.json"
OPERATION_PATTERN = re.compile(r"^[a-z][a-z0-9_]{2,63}$")
RUST_OPERATION_PATTERN = re.compile(r'"([a-z][a-z0-9_]{2,63})"')
SCHEMA_FILENAME_PATTERN = re.compile(r"^[a-z0-9][a-z0-9_.-]{0,191}\.schema\.json$")


class ClosureViolation(RuntimeError):
    """A fail-closed source or contract violation."""


def fail(message: str) -> None:
    raise ClosureViolation(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def rel(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def load_json(path: Path, label: str) -> Any:
    require(path.is_file(), f"missing {label}: {rel(path)}")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        fail(f"cannot read {label} {rel(path)}: {exc}")


def load_text(path: Path, label: str) -> str:
    require(path.is_file(), f"missing {label}: {rel(path)}")
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        fail(f"cannot read {label} {rel(path)}: {exc}")


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def canonical_hash(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def file_hash(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError as exc:
        fail(f"cannot hash {rel(path)}: {exc}")
    return ""


def require_object(value: Any, label: str) -> dict[str, Any]:
    require(isinstance(value, dict), f"{label} must be an object")
    return value


def unique_strings(value: Any, label: str) -> list[str]:
    require(isinstance(value, list), f"{label} must be an array")
    require(all(isinstance(item, str) and item for item in value), f"{label} contains a non-string")
    require(len(value) == len(set(value)), f"{label} contains duplicate entries")
    return list(value)


def profile_operations(profile: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, str]]:
    """Return the 125-style active inventory without a numeric assertion."""

    facades = require_object(profile.get("facades"), "profile.facades")
    native = require_object(profile.get("native_operations"), "profile.native_operations")
    native_names = set(native)
    owners: dict[str, str] = {}
    operations: list[dict[str, Any]] = []

    for facade_name, facade_value in facades.items():
        facade = require_object(facade_value, f"profile.facades.{facade_name}")
        require(
            facade.get("facade_name") == facade_name,
            f"profile façade name drifted: {facade_name}",
        )
        read = unique_strings(facade.get("read_tools"), f"{facade_name}.read_tools")
        write = unique_strings(facade.get("write_tools"), f"{facade_name}.write_tools")
        underlying = unique_strings(
            facade.get("underlying_operations"),
            f"{facade_name}.underlying_operations",
        )
        require(not set(read) & set(write), f"{facade_name} classifies an operation as both read and write")
        classified = set(read) | set(write)
        if facade_name == "authoring_transaction":
            classified |= native_names
        require(
            classified == set(underlying),
            f"{facade_name}.underlying_operations does not match its profile classification",
        )
        for operation in underlying:
            require(OPERATION_PATTERN.fullmatch(operation) is not None, f"invalid active operation name: {operation}")
            require(operation not in owners, f"active operation has duplicate façade owners: {operation}")
            owners[operation] = facade_name
            if operation in native_names:
                metadata = require_object(native[operation], f"profile.native_operations.{operation}")
                classification = metadata.get("classification")
                require(classification in {"read", "write"}, f"native operation classification is invalid: {operation}")
                require(metadata.get("facade_name") == facade_name, f"native operation façade drifted: {operation}")
            elif operation in read:
                classification = "read"
            else:
                require(operation in write, f"active operation has no read/write classification: {operation}")
                classification = "write"
            operations.append(
                {
                    "operation": operation,
                    "facade": facade_name,
                    "classification": classification,
                }
            )

    require(len(operations) == len(owners), "active operation inventory contains duplicate entries")
    return operations, owners


def check_compatibility_binding(
    profile: dict[str, Any], source_summary: dict[str, Any]
) -> dict[str, Any]:
    """Validate the old 226 route binding independently of active closure."""

    legacy = require_object(profile.get("legacy_operations"), "profile.legacy_operations")
    compatibility = require_object(profile.get("compatibility_profile"), "profile.compatibility_profile")
    compatibility_manifest = require_object(
        compatibility.get("legacy_manifest"),
        "profile.compatibility_profile.legacy_manifest",
    )
    read = unique_strings(legacy.get("read_tools"), "profile.legacy_operations.read_tools")
    write = unique_strings(legacy.get("write_tools"), "profile.legacy_operations.write_tools")
    require(not set(read) & set(write), "compatibility read/write manifests overlap")
    require(legacy.get("read_count") == len(read), "compatibility read count is not source-derived")
    require(legacy.get("write_count") == len(write), "compatibility write count is not source-derived")
    require(legacy.get("total_count") == len(set(read) | set(write)), "compatibility total count is not source-derived")

    for key, expected in (
        ("schema_version", source_summary.get("schema_version")),
        ("read_tools", source_summary.get("read_names")),
        ("write_tools", source_summary.get("write_names")),
        ("read_count", source_summary.get("read_count")),
        ("write_count", source_summary.get("write_count")),
        ("total_count", source_summary.get("total_count")),
        ("read_manifest_sha256", source_summary.get("read_manifest_sha256")),
        ("write_enabled_manifest_sha256", source_summary.get("write_enabled_manifest_sha256")),
        ("canonical_sha256", source_summary.get("canonical_sha256")),
    ):
        require(legacy.get(key) == expected, f"legacy operation binding drifted at {key}")
        if key in {"schema_version", "read_count", "write_count", "total_count", "read_manifest_sha256", "write_enabled_manifest_sha256", "canonical_sha256"}:
            require(compatibility_manifest.get(key) == expected, f"compatibility profile binding drifted at {key}")

    require(
        compatibility.get("facade_names") == [],
        "compatibility profile must remain separate from the active façade surface",
    )
    return {
        "read_count": len(read),
        "write_count": len(write),
        "total_count": len(set(read) | set(write)),
        "read_manifest_sha256": source_summary.get("read_manifest_sha256"),
        "write_enabled_manifest_sha256": source_summary.get("write_enabled_manifest_sha256"),
        "source_summary_sha256": file_hash(Path(source_summary["__path__"])),
    }


def rust_array(rust: str, name: str) -> tuple[str, ...]:
    pattern = re.compile(
        rf"(?:pub\([^)]*\)\s+)?(?:pub\s+)?const\s+{re.escape(name)}\s*"
        r":\s*&\[\s*&str\s*\]\s*=\s*&\[(?P<body>.*?)\];",
        re.DOTALL,
    )
    matches = list(pattern.finditer(rust))
    require(matches, f"domain map is missing operation array {name}")
    require(len(matches) == 1, f"domain map has duplicate operation array {name}")
    return tuple(RUST_OPERATION_PATTERN.findall(matches[0].group("body")))


def domain_map_bindings(
    domain_map: str,
    active: list[dict[str, Any]],
    legacy_operations: set[str],
) -> dict[str, dict[str, Any]]:
    """Resolve central façade/domain and capability operation ownership."""

    facade_domains: dict[str, str] = {}
    for match in re.finditer(
        r"KnifeFacadeBinding\s*\{\s*facade_name:\s*\"([^\"]+)\"\s*,\s*"
        r"domain:\s*WeaponryServiceDomain::([A-Za-z]+)",
        domain_map,
    ):
        facade, domain = match.groups()
        require(facade not in facade_domains, f"domain map has duplicate façade binding: {facade}")
        facade_domains[facade] = domain.lower()
    require(facade_domains, "domain map façade bindings are missing")
    profile_facades = {item["facade"] for item in active}
    require(
        set(facade_domains) == profile_facades,
        "domain map façade set drifted: "
        + json.dumps(
            {
                "map_only": sorted(set(facade_domains) - profile_facades),
                "profile_only": sorted(profile_facades - set(facade_domains)),
            },
            separators=(",", ":"),
        ),
    )

    active_by_name = {item["operation"]: item for item in active}
    map_operation_owners: dict[str, list[dict[str, str]]] = {}
    mapping_start = domain_map.find("pub const KNIFE_CAPABILITY_MAPPINGS")
    require(mapping_start >= 0, "domain map capability mappings are missing")
    mapping_source = domain_map[mapping_start:]
    blocks = re.findall(r"WeaponryCapabilityMapping\s*\{(.*?)\n\s*\},", mapping_source, re.DOTALL)
    require(blocks, "domain map has no capability mapping rows")
    for block in blocks:
        capability_match = re.search(r'capability:\s*"([^"]+)"', block)
        domain_match = re.search(r"domain:\s*WeaponryServiceDomain::([A-Za-z]+)", block)
        facade_match = re.search(r'mcp_facade:\s*Some\("([^"]+)"\)', block)
        operations_match = re.search(r"mcp_operations:\s*([A-Z][A-Z0-9_]*)", block)
        require(capability_match and domain_match and facade_match and operations_match, "unreadable capability mapping row")
        capability = capability_match.group(1)
        domain = domain_match.group(1).lower()
        facade = facade_match.group(1)
        require(facade in facade_domains, f"capability {capability} names unknown façade {facade}")
        require(facade_domains[facade] == domain, f"capability {capability} façade/domain disagree")
        operation_array = operations_match.group(1)
        for operation in rust_array(domain_map, operation_array):
            require(
                operation in active_by_name or operation in legacy_operations,
                f"domain map names unknown operation {operation}",
            )
            map_operation_owners.setdefault(operation, []).append(
                {"capability": capability, "facade": facade, "domain": domain}
            )

    duplicates = {name: rows for name, rows in map_operation_owners.items() if len(rows) != 1}
    require(not duplicates, "domain map assigns an operation to multiple capabilities: " + json.dumps(duplicates, sort_keys=True))
    unknown_active = set(active_by_name) - set(map_operation_owners) - legacy_operations
    require(
        not unknown_active,
        "active profile names operations absent from both the domain map and compatibility manifest: "
        + json.dumps(sorted(unknown_active)),
    )

    result: dict[str, dict[str, Any]] = {}
    for item in active:
        operation = item["operation"]
        facade = item["facade"]
        require(facade in facade_domains, f"active façade is absent from domain map: {facade}")
        owners = map_operation_owners.get(operation, [])
        if owners:
            owner = owners[0]
            require(owner["facade"] == facade, f"domain map façade drifted for {operation}")
            require(owner["domain"] == facade_domains[facade], f"domain map domain drifted for {operation}")
            result[operation] = {
                "domain": facade_domains[facade],
                "central_mapping": owner["capability"],
                "central_mapping_status": "mapped",
            }
        else:
            # The current map is capability-level and intentionally partial;
            # profile ownership remains the source for unmapped active rows.
            result[operation] = {
                "domain": facade_domains[facade],
                "central_mapping": None,
                "central_mapping_status": "unmapped",
            }
    return result


def validate_schema_manifest(
    manifest: dict[str, Any], schema_root: Path
) -> tuple[list[str], dict[str, dict[str, Any]]]:
    entries = unique_strings(manifest.get("schemas"), "contract manifest.schemas")
    require(all("/" not in path and "\\" not in path and path.endswith(".json") for path in entries), "schema manifest contains an unsafe path")
    actual = sorted(path.name for path in schema_root.glob("*.json"))
    require(
        set(entries) == set(actual),
        "schema manifest drifted: "
        + json.dumps(
            {"manifest_only": sorted(set(entries) - set(actual)), "filesystem_only": sorted(set(actual) - set(entries))},
            separators=(",", ":"),
        ),
    )
    parsed: dict[str, dict[str, Any]] = {}
    for name in entries:
        parsed[name] = require_object(load_json(schema_root / name, f"schema {name}"), f"schema {name}")
    return entries, parsed


def _rust_match_arm_key_pattern() -> str:
    operation = r'"[a-z][a-z0-9_]{2,63}"'
    return rf"{operation}(?:\s*\|\s*{operation})*"


def parse_active_schema_source(
    source: str, active_operations: set[str]
) -> dict[str, Any]:
    """Read the active MCP schema resolver's source-level bindings.

    The Rust resolver intentionally owns the consumed schema surface.  This
    parser only reads its bounded source tables and builtin match arms; it
    does not reproduce the profile's operation inventory.  Any source shape
    that cannot be read unambiguously fails closed instead of silently
    reducing the reported coverage.
    """

    table_pattern = re.compile(
        r"const\s+EMBEDDED_SCHEMA_DOCUMENTS\s*:\s*&\[\s*\(&str,\s*&str\)\s*\]"
        r"\s*=\s*&\[(?P<body>.*?)\n\];",
        re.DOTALL,
    )
    table_matches = list(table_pattern.finditer(source))
    require(
        len(table_matches) == 1,
        "active_schema.rs must contain exactly one embedded schema document table",
    )
    table_body = table_matches[0].group("body")
    pair_pattern = re.compile(
        r"\(\s*\"(?P<name>[^\"]+)\"\s*,\s*"
        r"embedded_schema!\s*\(\s*\"(?P<embedded>[^\"]+)\"\s*\)\s*,?\s*\)",
        re.DOTALL,
    )
    pairs = list(pair_pattern.finditer(table_body))
    macro_files = re.findall(
        r"embedded_schema!\s*\(\s*\"([^\"]+)\"\s*\)", table_body, re.DOTALL
    )
    tuple_names = re.findall(r"\(\s*\"([^\"]+)\"\s*,", table_body)
    require(
        len(pairs) == len(macro_files) == len(tuple_names),
        "active_schema.rs embedded schema table contains an unreadable entry",
    )
    embedded_files: list[str] = []
    for pair in pairs:
        name = pair.group("name")
        embedded = pair.group("embedded")
        require(
            name == embedded,
            f"active_schema.rs embedded schema tuple disagrees with macro: {name} -> {embedded}",
        )
        require(
            SCHEMA_FILENAME_PATTERN.fullmatch(name) is not None,
            f"active_schema.rs contains an unsafe embedded schema filename: {name}",
        )
        embedded_files.append(name)
    require(
        len(embedded_files) == len(set(embedded_files)),
        "active_schema.rs embedded schema table contains duplicate filenames",
    )

    schema_start = source.find("fn schema_file_for_operation")
    builtin_start = source.find("fn builtin_schema")
    document_table_start = source.find("/// Schema documents present", schema_start)
    schema_end = builtin_start if builtin_start > schema_start else document_table_start
    require(
        schema_start >= 0 and schema_end > schema_start,
        "active_schema.rs schema resolver functions are missing or out of order",
    )
    schema_source = source[schema_start:schema_end]
    alias_matches = list(
        re.finditer(
            r'"(?P<operation>[a-z][a-z0-9_]{2,63})"\s*=>\s*'
            r'"(?P<stem>[a-z0-9][a-z0-9_-]{1,127})"',
            schema_source,
        )
    )
    source_aliases: dict[str, str] = {}
    for match in alias_matches:
        operation = match.group("operation")
        stem = match.group("stem")
        require(operation not in source_aliases, f"active_schema.rs has duplicate alias: {operation}")
        source_aliases[operation] = f"{stem}-request.schema.json"
    require(source_aliases, "active_schema.rs schema_file_for_operation has no explicit alias arms")
    require(
        "operation.replace('_', \"-\")" in schema_source,
        "active_schema.rs schema resolver lost its bounded snake-to-kebab fallback",
    )
    require(
        "request.schema.json" in schema_source,
        "active_schema.rs schema resolver lost its request-schema suffix",
    )
    require(
        set(source_aliases).issubset(active_operations),
        "active_schema.rs alias names an operation outside the active profile: "
        + json.dumps(sorted(set(source_aliases) - active_operations)),
    )

    # Package-owned request schemas are the final architecture.  A legacy MCP
    # builtin resolver is accepted only while migration is partial; once it is
    # removed, the resolver itself must still return None for an unregistered
    # operation so unknown fields cannot fall through to Runtime.
    if builtin_start < 0:
        require(
            "return Ok(None);" in source[:document_table_start],
            "active_schema.rs package-only resolver must fail closed when a schema is absent",
        )
        return {
            "embedded_files": embedded_files,
            "source_aliases": source_aliases,
            "builtin_schemas": {},
        }

    builtin_end = source.find("    Ok(Some(schema))", builtin_start)
    require(builtin_end > builtin_start, "active_schema.rs builtin resolver is unterminated")
    builtin_source = source[builtin_start:builtin_end]
    require(
        re.search(r"\n\s*_\s*=>\s*return\s+Ok\(None\)", builtin_source) is not None,
        "active_schema.rs builtin resolver must fail closed for unknown operations",
    )
    match_start = builtin_source.find("match operation {")
    require(match_start >= 0, "active_schema.rs builtin operation match is missing")
    match_source = builtin_source[match_start:]
    arm_key = _rust_match_arm_key_pattern()
    arm_pattern = re.compile(
        rf"(?ms)^[ \t]*(?P<ops>{arm_key})\s*=>\s*(?P<body>.*?)(?=^[ \t]*(?:{arm_key}|_)\s*=>)"
    )
    arms = list(arm_pattern.finditer(match_source))
    require(arms, "active_schema.rs builtin resolver has no operation arms")

    helper_start = source.find("fn required_string_object")
    require(helper_start >= 0, "active_schema.rs required_string_object helper is missing")
    helper_end = source.find("/// Schema documents present", helper_start)
    require(helper_end > helper_start, "active_schema.rs required_string_object helper is unterminated")
    helper_source = source[helper_start:helper_end]
    helper_closed_match = re.search(
        r'"additionalProperties"\s*:\s*(true|false)', helper_source
    )
    require(
        helper_closed_match is not None,
        "active_schema.rs required_string_object helper has no root closure marker",
    )
    helper_closed = helper_closed_match.group(1) == "false"

    builtin_schemas: dict[str, dict[str, Any]] = {}
    for arm in arms:
        operations = re.findall(RUST_OPERATION_PATTERN, arm.group("ops"))
        require(operations, "active_schema.rs builtin arm has no operation name")
        body = arm.group("body").strip()
        if "required_string_object(" in body:
            require(
                body.startswith("required_string_object("),
                "active_schema.rs builtin arm has unsupported helper siblings",
            )
            closed = helper_closed
            schema_kind = "required_string_object"
        elif "json!(" in body:
            require(
                body.startswith("json!("),
                "active_schema.rs builtin arm has unsupported JSON siblings",
            )
            closed = re.search(
                r'"additionalProperties"\s*:\s*false\b', body
            ) is not None
            schema_kind = "json"
        else:
            fail("active_schema.rs builtin arm has an unsupported schema expression")
        for operation in operations:
            require(
                operation not in builtin_schemas,
                f"active_schema.rs builtin resolver has duplicate operation: {operation}",
            )
            builtin_schemas[operation] = {
                "closed": closed,
                "schema_kind": schema_kind,
            }
    require(
        set(builtin_schemas).issubset(active_operations),
        "active_schema.rs builtin resolver names an operation outside the active profile: "
        + json.dumps(sorted(set(builtin_schemas) - active_operations)),
    )
    return {
        "embedded_files": embedded_files,
        "source_aliases": source_aliases,
        "builtin_schemas": builtin_schemas,
    }


def config_schema_aliases(config: dict[str, Any]) -> dict[str, str]:
    value = require_object(
        config.get("request_schema_aliases"), "closure config request_schema_aliases"
    )
    aliases: dict[str, str] = {}
    targets: set[str] = set()
    for operation, filename in value.items():
        require(
            OPERATION_PATTERN.fullmatch(operation) is not None,
            f"closure config request schema alias has an invalid operation: {operation}",
        )
        require(
            isinstance(filename, str) and SCHEMA_FILENAME_PATTERN.fullmatch(filename) is not None,
            f"closure config request schema alias has an invalid filename for {operation}",
        )
        require(filename not in targets, f"closure config aliases reuse schema filename: {filename}")
        targets.add(filename)
        aliases[operation] = filename
    return aliases


def schema_binding(
    operation: str,
    manifest_entries: set[str],
    schemas: dict[str, dict[str, Any]],
    schema_root: Path,
    templates: Iterable[str],
    aliases: dict[str, str] | None = None,
) -> dict[str, Any]:
    slug = operation.replace("_", "-")
    aliases = aliases or {}
    package_classification = "direct"
    if operation in aliases:
        candidates = [aliases[operation]]
        package_classification = "explicit-alias"
        direct_candidates = []
        for template in templates:
            require(isinstance(template, str) and template, "request schema filename template is invalid")
            try:
                filename = template.format(operation=operation, operation_kebab=slug)
            except (KeyError, ValueError) as exc:
                fail(f"request schema filename template is invalid: {exc}")
            require(
                SCHEMA_FILENAME_PATTERN.fullmatch(filename) is not None,
                f"request schema filename template produced an unsafe filename: {filename}",
            )
            direct_candidates.append(filename)
        require(
            not set(direct_candidates) & manifest_entries,
            f"explicit alias shadows a direct request-schema filename: {operation}",
        )
    else:
        candidates = []
        for template in templates:
            require(isinstance(template, str) and template, "request schema filename template is invalid")
            try:
                filename = template.format(operation=operation, operation_kebab=slug)
            except (KeyError, ValueError) as exc:
                fail(f"request schema filename template is invalid: {exc}")
            require(
                SCHEMA_FILENAME_PATTERN.fullmatch(filename) is not None,
                f"request schema filename template produced an unsafe filename: {filename}",
            )
            if filename in manifest_entries:
                candidates.append(filename)
    require(len(candidates) <= 1, f"operation has duplicate request-schema bindings: {operation} -> {candidates}")
    if not candidates:
        return {
            "classification": "unresolved",
            "package_schema_classification": None,
            "schema_path": None,
            "schema_sha256": None,
            "schema_version": None,
            "package_schema_filename": None,
        }

    filename = candidates[0]
    require(
        filename in manifest_entries,
        f"explicit alias target is absent from the Contract manifest: {operation} -> {filename}",
    )
    schema = schemas[filename]
    require(schema.get("type") == "object", f"request schema root is not an object: {filename}")
    require(
        schema.get("additionalProperties") is False,
        f"request schema root must set additionalProperties=false: {filename}",
    )
    schema_id = schema.get("$id")
    require(
        isinstance(schema_id, str) and schema_id.endswith("/" + filename),
        f"request schema $id/path drifted: {filename}",
    )
    schema_version = None
    version_node = schema.get("properties", {}).get("schema_version") if isinstance(schema.get("properties"), dict) else None
    if isinstance(version_node, dict):
        schema_version = version_node.get("const")
    try:
        schema_path = str(schema_root.relative_to(ROOT) / filename)
    except ValueError:
        # Negative fixtures run against a temporary schema tree.  Keep the
        # report path stable there without making the fixture depend on the
        # repository's absolute path.
        schema_path = str(Path(schema_root.name) / filename)
    return {
        "classification": "closed-schema",
        "package_schema_classification": package_classification,
        "schema_path": schema_path,
        "schema_sha256": file_hash(schema_root / filename),
        "schema_version": schema_version,
        "package_schema_filename": filename,
    }


def audit_paths(
    config: dict[str, Any],
    profile_path: Path,
    domain_map_path: Path,
    manifest_path: Path,
    schema_root: Path,
    compatibility_summary_path: Path,
    active_schema_path: Path | None = None,
) -> dict[str, Any]:
    profile = require_object(load_json(profile_path, "knife profile"), "knife profile")
    manifest = require_object(load_json(manifest_path, "contract manifest"), "contract manifest")
    domain_map = load_text(domain_map_path, "Contract domain map")
    if active_schema_path is None:
        configured = config.get("active_schema_source_path")
        require(
            isinstance(configured, str) and configured and not Path(configured).is_absolute(),
            "closure config active_schema_source_path must be a relative path",
        )
        active_schema_path = ROOT / configured
    active_schema_source = load_text(active_schema_path, "active MCP schema source")
    source_summary = require_object(
        load_json(compatibility_summary_path, "compatibility source summary"),
        "compatibility source summary",
    )
    source_summary = copy.deepcopy(source_summary)
    source_summary["__path__"] = str(compatibility_summary_path)

    active, _ = profile_operations(profile)
    active_names = {item["operation"] for item in active}
    aliases = config_schema_aliases(config)
    active_schema = parse_active_schema_source(active_schema_source, active_names)
    require(
        aliases == active_schema["source_aliases"],
        "closure config aliases drifted from active_schema.rs: "
        + json.dumps(
            {
                "config_only": sorted(set(aliases) - set(active_schema["source_aliases"])),
                "source_only": sorted(set(active_schema["source_aliases"]) - set(aliases)),
                "different_targets": sorted(
                    operation
                    for operation in set(aliases) & set(active_schema["source_aliases"])
                    if aliases[operation] != active_schema["source_aliases"][operation]
                ),
            },
            separators=(",", ":"),
        ),
    )
    compatibility = check_compatibility_binding(profile, source_summary)
    domain_rows = domain_map_bindings(
        domain_map,
        active,
        set(profile.get("legacy_operations", {}).get("read_tools", []))
        | set(profile.get("legacy_operations", {}).get("write_tools", [])),
    )
    manifest_entries, schemas = validate_schema_manifest(manifest, schema_root)
    templates = config.get("request_schema_filename_templates")
    require(isinstance(templates, list) and templates, "request schema filename templates are missing")
    for filename in active_schema["embedded_files"]:
        require(
            filename in manifest_entries,
            f"active_schema.rs embeds a schema absent from the Contract manifest: {filename}",
        )

    rows: list[dict[str, Any]] = []
    package_direct = 0
    package_alias = 0
    mcp_embedded = 0
    mcp_builtin = 0
    for item in active:
        binding = schema_binding(
            item["operation"],
            set(manifest_entries),
            schemas,
            schema_root,
            templates,
            aliases,
        )
        package_classification = binding.get("package_schema_classification")
        if package_classification == "direct":
            package_direct += 1
        elif package_classification == "explicit-alias":
            package_alias += 1

        operation = item["operation"]
        schema_filename = aliases.get(
            operation, f"{operation.replace('_', '-')}-request.schema.json"
        )
        embedded = schema_filename in set(active_schema["embedded_files"])
        builtin = active_schema["builtin_schemas"].get(operation)
        require(
            not (embedded and builtin is not None),
            f"active_schema.rs resolves an operation through both embedded and builtin metadata: {operation}",
        )
        if embedded:
            require(
                binding.get("classification") == "closed-schema",
                f"MCP embedded schema is not a closed package root: {operation} -> {schema_filename}",
            )
            mcp_classification = "embedded"
            mcp_schema_path = str(Path(config["schema_root"]) / schema_filename)
            mcp_embedded += 1
        elif builtin is not None and builtin["closed"]:
            mcp_classification = "builtin"
            mcp_schema_path = "active_schema.rs::builtin_schema"
            mcp_builtin += 1
        else:
            mcp_classification = None
            mcp_schema_path = None
        mcp_consumed = mcp_classification is not None
        rows.append(
            {
                **item,
                **domain_rows[operation],
                **binding,
                "mcp_schema_classification": mcp_classification,
                "mcp_schema_consumed": mcp_consumed,
                "mcp_schema_path": mcp_schema_path,
                "request_schema_classification": (
                    f"mcp-{mcp_classification}-closed"
                    if mcp_classification is not None
                    else "unresolved"
                ),
            }
        )

    active_count = len(rows)
    mcp_consumed_closed = mcp_embedded + mcp_builtin
    expected_mcp_consumed = config.get("expected_mcp_consumed_closed_count")
    require(
        isinstance(expected_mcp_consumed, int) and not isinstance(expected_mcp_consumed, bool)
        and expected_mcp_consumed >= 0,
        "closure config expected_mcp_consumed_closed_count must be a non-negative integer",
    )
    require(
        mcp_consumed_closed == expected_mcp_consumed,
        "MCP consumed closed-schema coverage drifted: "
        + json.dumps(
            {
                "actual": mcp_consumed_closed,
                "expected": expected_mcp_consumed,
                "active": active_count,
                "embedded": mcp_embedded,
                "builtin": mcp_builtin,
            },
            separators=(",", ":"),
        ),
    )
    blocked_unresolved = active_count - mcp_consumed_closed
    unresolved = [row["operation"] for row in rows if not row["mcp_schema_consumed"]]
    require(
        blocked_unresolved == len(unresolved),
        "MCP unresolved operation accounting is inconsistent",
    )
    report = {
        "schema_version": "WeaponryActiveOperationRequestSchemaClosure@1",
        "profile_id": profile.get("profile_id"),
        "active_operation_count": active_count,
        "mcp_closed_request_schema_count": mcp_consumed_closed,
        "closed_request_schema_count": mcp_consumed_closed,
        "request_schema_closure_status": "COMPLETE" if blocked_unresolved == 0 else "PARTIAL",
        "package_direct_schema_count": package_direct,
        "package_alias_schema_count": package_alias,
        "explicit_alias_schema_count": package_alias,
        "package_closed_schema_count": package_direct + package_alias,
        "package_unresolved_schema_count": active_count - package_direct - package_alias,
        "mcp_embedded_closed_schema_count": mcp_embedded,
        "mcp_embedded_consumed_closed_schema_count": mcp_embedded,
        "mcp_builtin_closed_schema_count": mcp_builtin,
        "mcp_consumed_closed_schema_count": mcp_consumed_closed,
        "mcp_consumed_coverage": f"{mcp_consumed_closed}/{active_count}",
        "mcp_consumed_coverage_status": "PASS",
        "mcp_blocked_unresolved_schema_count": blocked_unresolved,
        "mcp_blocked_operation_count": blocked_unresolved,
        "mcp_schema_blocked_count": blocked_unresolved,
        "unresolved_operation_count": blocked_unresolved,
        "unresolved_request_schema_count": blocked_unresolved,
        "runtime_validation_fallback": False,
        "runtime_fallback_used": False,
        "runtime_fallback_count": 0,
        "package_unresolved_operations": [
            row["operation"] for row in rows if row.get("package_schema_classification") is None
        ],
        "mcp_blocked_operations": unresolved,
        "package_schema_coverage": {
            "direct": package_direct,
            "explicit_alias": package_alias,
            "closed": package_direct + package_alias,
            "unresolved": active_count - package_direct - package_alias,
        },
        "mcp_schema_coverage": {
            "embedded_closed": mcp_embedded,
            "builtin_closed": mcp_builtin,
            "consumed_closed": mcp_consumed_closed,
            "blocked_unresolved": blocked_unresolved,
            "active_operations": active_count,
        },
        "active_schema_source": {
            "path": rel(active_schema_path),
            "sha256": file_hash(active_schema_path),
            "embedded_document_count": len(active_schema["embedded_files"]),
            "embedded_active_closed_count": mcp_embedded,
            "builtin_operation_count": len(active_schema["builtin_schemas"]),
            "builtin_active_closed_count": mcp_builtin,
        },
        "compatibility_manifest": compatibility,
        "contract_manifest": {
            "schema_count": len(manifest_entries),
            "schema_entries_sha256": canonical_hash(manifest_entries),
        },
        "operations": rows,
    }
    return report


def resolve_config(config_path: Path) -> tuple[dict[str, Any], dict[str, Path]]:
    config = require_object(load_json(config_path, "closure config"), "closure config")
    require(config.get("schema_version") == "WeaponryActiveOperationRequestSchemaClosureConfig@1", "closure config version drifted")

    def configured_path(key: str) -> Path:
        value = config.get(key)
        require(isinstance(value, str) and value and not Path(value).is_absolute(), f"closure config {key} must be a relative path")
        return ROOT / value

    paths = {
        "profile": configured_path("profile_path"),
        "domain_map": configured_path("domain_map_path"),
        "manifest": configured_path("contract_manifest_path"),
        "schema_root": configured_path("schema_root"),
        "compatibility_summary": configured_path("compatibility_summary_path"),
        "active_schema": configured_path("active_schema_source_path"),
    }
    require(paths["schema_root"].is_dir(), f"missing schema root: {rel(paths['schema_root'])}")
    require(paths["active_schema"].is_file(), f"missing active MCP schema source: {rel(paths['active_schema'])}")
    expected = config.get("expected_mcp_consumed_closed_count")
    require(
        isinstance(expected, int) and not isinstance(expected, bool) and expected >= 0,
        "closure config expected_mcp_consumed_closed_count must be a non-negative integer",
    )
    return config, paths


def run_negative_fixtures(config: dict[str, Any], paths: dict[str, Path]) -> None:
    """Run small in-memory/temp-tree negative fixtures for the drift gates."""

    baseline = audit_paths(
        config,
        paths["profile"],
        paths["domain_map"],
        paths["manifest"],
        paths["schema_root"],
        paths["compatibility_summary"],
        paths["active_schema"],
    )
    require(baseline["active_operation_count"] > 0, "negative fixtures require a non-empty active inventory")

    with tempfile.TemporaryDirectory(prefix="weaponry-schema-closure-") as temp:
        root = Path(temp)
        schema_root = root / "schemas"
        shutil.copytree(paths["schema_root"], schema_root)
        profile_path = root / "profile.json"
        manifest_path = root / "manifest.json"
        domain_map_path = root / "domain-map.rs"
        summary_path = root / "summary.json"
        active_schema_path = root / "active_schema.rs"
        shutil.copy2(paths["profile"], profile_path)
        shutil.copy2(paths["manifest"], manifest_path)
        shutil.copy2(paths["domain_map"], domain_map_path)
        shutil.copy2(paths["compatibility_summary"], summary_path)
        shutil.copy2(paths["active_schema"], active_schema_path)

        def add_delivery_request_schema() -> None:
            profile = json.loads(profile_path.read_text(encoding="utf-8"))
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            active_names = {
                operation
                for facade in profile["facades"].values()
                for operation in facade["underlying_operations"]
            }
            manifest_names = set(manifest["schemas"])
            aliases = config_schema_aliases(config)
            operation = next(
                operation
                for operation in active_names
                if operation not in aliases
                and operation.replace("_", "-") + "-request.schema.json" not in manifest_names
            )
            filename = operation.replace("_", "-") + "-request.schema.json"
            (schema_root / filename).write_text(
                json.dumps(
                    {
                        "$id": "https://schemas.forgecad.dev/" + filename,
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "title": filename,
                        "type": "object",
                        "additionalProperties": False,
                        "properties": {},
                        "required": [],
                    }
                ),
                encoding="utf-8",
            )
            manifest["schemas"].append(filename)
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            report = audit_paths(
                config,
                profile_path,
                domain_map_path,
                manifest_path,
                schema_root,
                summary_path,
                active_schema_path,
            )
            require(
                report["package_direct_schema_count"] == baseline["package_direct_schema_count"] + 1,
                "new Delivery request schema did not increase package-direct count",
            )
            require(
                report["mcp_consumed_closed_schema_count"] == baseline["mcp_consumed_closed_schema_count"],
                "a package-only Delivery schema incorrectly changed MCP consumed coverage",
            )

        package_growth_fixture_ran = baseline["unresolved_operation_count"] > 0
        if package_growth_fixture_ran:
            add_delivery_request_schema()
        else:
            require(
                baseline["request_schema_closure_status"] == "COMPLETE",
                "zero unresolved operations require COMPLETE closure",
            )
            require(
                baseline["mcp_consumed_closed_schema_count"]
                == baseline["active_operation_count"],
                "complete closure must consume every active operation schema",
            )
            require(
                baseline["runtime_fallback_count"] == 0,
                "complete closure must not retain Runtime schema fallback",
            )
        shutil.copy2(paths["manifest"], manifest_path)
        shutil.rmtree(schema_root)
        shutil.copytree(paths["schema_root"], schema_root)

        def expect_failure(label: str, mutate: Any) -> None:
            mutate()
            try:
                audit_paths(
                    config,
                    profile_path,
                    domain_map_path,
                    manifest_path,
                    schema_root,
                    summary_path,
                    active_schema_path,
                )
            except ClosureViolation:
                return
            fail(f"negative fixture unexpectedly passed: {label}")

        def expect_config_failure(label: str, fixture_config: dict[str, Any]) -> None:
            try:
                audit_paths(
                    fixture_config,
                    profile_path,
                    domain_map_path,
                    manifest_path,
                    schema_root,
                    summary_path,
                    active_schema_path,
                )
            except ClosureViolation:
                return
            fail(f"negative fixture unexpectedly passed: {label}")

        def duplicate_manifest() -> None:
            value = json.loads(manifest_path.read_text(encoding="utf-8"))
            value["schemas"].append(value["schemas"][0])
            manifest_path.write_text(json.dumps(value), encoding="utf-8")

        expect_failure("duplicate manifest schema", duplicate_manifest)

        shutil.copy2(paths["manifest"], manifest_path)

        def missing_manifest_schema() -> None:
            value = json.loads(manifest_path.read_text(encoding="utf-8"))
            schema_name = value["schemas"][0]
            (schema_root / schema_name).unlink()

        expect_failure("missing manifest schema file", missing_manifest_schema)

        shutil.rmtree(schema_root)
        shutil.copytree(paths["schema_root"], schema_root)

        def open_request_schema() -> None:
            profile = json.loads(paths["profile"].read_text(encoding="utf-8"))
            aliases = config_schema_aliases(config)
            operations = [
                operation
                for facade in profile["facades"].values()
                for operation in facade["underlying_operations"]
                if operation not in aliases
                and (operation.replace("_", "-") + "-request.schema.json")
                in json.loads(paths["manifest"].read_text(encoding="utf-8"))["schemas"]
            ]
            require(operations, "negative fixture could not find a direct request schema")
            filename = operations[0].replace("_", "-") + "-request.schema.json"
            schema_path = schema_root / filename
            schema = json.loads(schema_path.read_text(encoding="utf-8"))
            schema["additionalProperties"] = True
            schema_path.write_text(json.dumps(schema), encoding="utf-8")

        expect_failure("open request schema", open_request_schema)

        shutil.copy2(paths["manifest"], manifest_path)
        shutil.rmtree(schema_root)
        shutil.copytree(paths["schema_root"], schema_root)

        def alias_drift() -> dict[str, Any]:
            fixture_config = copy.deepcopy(config)
            aliases = config_schema_aliases(fixture_config)
            operation = next(iter(aliases))
            aliases[operation] = "alias-drift-request.schema.json"
            fixture_config["request_schema_aliases"] = aliases
            return fixture_config

        expect_config_failure("explicit alias drift", alias_drift())

        def mcp_embed_drift() -> None:
            report = baseline
            operation = next(
                row["operation"]
                for row in report["operations"]
                if row.get("mcp_schema_classification") == "embedded"
                and row.get("package_schema_filename")
            )
            filename = next(
                row["package_schema_filename"]
                for row in report["operations"]
                if row["operation"] == operation
            )
            source = active_schema_path.read_text(encoding="utf-8")
            entry_pattern = re.compile(
                r"\n\s*\(\s*\""
                + re.escape(filename)
                + r"\"\s*,\s*embedded_schema!\s*\(\s*\""
                + re.escape(filename)
                + r"\"\s*\)\s*,?\s*\)",
                re.DOTALL,
            )
            mutated, replacements = entry_pattern.subn("", source, count=1)
            require(replacements == 1, "negative fixture could not find an active embedded schema row")
            active_schema_path.write_text(mutated, encoding="utf-8")

        expect_failure("MCP embedded schema drift", mcp_embed_drift)

        shutil.copy2(paths["active_schema"], active_schema_path)

        def duplicate_profile_operation() -> None:
            value = json.loads(profile_path.read_text(encoding="utf-8"))
            value["facades"]["observe"]["underlying_operations"].append("candidate_get")
            profile_path.write_text(json.dumps(value), encoding="utf-8")

        expect_failure("duplicate active operation owner", duplicate_profile_operation)

        shutil.copy2(paths["profile"], profile_path)

        def unknown_profile_operation() -> None:
            value = json.loads(profile_path.read_text(encoding="utf-8"))
            value["facades"]["observe"]["read_tools"].append("unknown_active_operation")
            value["facades"]["observe"]["underlying_operations"].append("unknown_active_operation")
            profile_path.write_text(json.dumps(value), encoding="utf-8")

        expect_failure("unknown active operation", unknown_profile_operation)

    completion_fixture = (
        "package request-schema growth PASS"
        if package_growth_fixture_ran
        else "complete 125-operation package closure PASS"
    )
    print(
        "Weaponry MCP schema closure negative fixtures PASS: duplicate, missing, "
        "manifest drift, open schema, alias drift, MCP embed drift, duplicate owner, "
        f"and unknown operation; {completion_fixture}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--self-test", action="store_true", help="run focused negative fixtures after the source audit")
    args = parser.parse_args()

    config, paths = resolve_config(args.config.resolve())
    report = audit_paths(
        config,
        paths["profile"],
        paths["domain_map"],
        paths["manifest"],
        paths["schema_root"],
        paths["compatibility_summary"],
        paths["active_schema"],
    )
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=False))
    if args.self_test:
        run_negative_fixtures(config, paths)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ClosureViolation as exc:
        raise SystemExit(f"WPN-ARCH-MCP-SCHEMA-002 FAIL: {exc}")
