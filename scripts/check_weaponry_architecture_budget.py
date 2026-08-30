#!/usr/bin/env python3
"""Check the physical architecture budget for the Weaponry root modules.

The default check is deliberately fail-closed: current source measurements may
be below a locked ceiling, but never above it.  ``compare`` reports the delta
against a locked baseline (or an explicitly supplied snapshot), while
``require-decrease`` is the migration proof mode and requires at least one
tracked measurement to go down.  The checker does not infer a migration from a
new facade or repository file; it only accepts measurable root reduction.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CONFIG = ROOT / "config" / "weaponry" / "architecture-budget.json"
SCHEMA_VERSION = "WeaponryArchitectureBudget@1"
REPORT_SCHEMA_VERSION = "WeaponryArchitectureBudgetCheck@1"

EXPECTED_BASELINE: dict[str, int] = {
    "runtime_lib_lines": 52927,
    "store_lib_lines": 81050,
    "mcp_main_lines": 20508,
    "mcp_agentic_write_tools_lines": 22800,
    "runtime_root_module_declarations": 92,
}
REQUIRED_TARGETS = tuple(EXPECTED_BASELINE)
TARGET_FIELDS = {"id", "path", "metric", "baseline", "ceiling"}
SUPPORTED_METRICS = {"line_count", "lines", "root_module_declarations"}
MODULE_DECLARATION = re.compile(
    r"^\s*(?:(?:pub)(?:\([^)]*\))?\s+)?mod\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*(?:;|\{)"
)


def fail(message: str) -> None:
    raise SystemExit(f"WPN_ARCHITECTURE_BUDGET_INVALID: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def display_path(path: Path) -> str:
    try:
        return path.relative_to(ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def resolve_path(value: str | Path) -> Path:
    path = Path(value)
    return path if path.is_absolute() else ROOT / path


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        fail(f"cannot load {display_path(path)}: {exc}")
    require(isinstance(value, dict), f"{display_path(path)} must contain a JSON object")
    return value


def integer(value: Any, label: str) -> int:
    require(isinstance(value, int) and not isinstance(value, bool), f"{label} must be an integer")
    require(value >= 0, f"{label} must be non-negative")
    return value


def metric_map(value: Any, label: str) -> dict[str, int]:
    require(isinstance(value, dict), f"{label} must be an object")
    require(set(value) == set(REQUIRED_TARGETS), f"{label} target set drifted")
    return {key: integer(value[key], f"{label}.{key}") for key in REQUIRED_TARGETS}


def target_rows(config: dict[str, Any]) -> dict[str, dict[str, Any]]:
    raw = config.get("targets")
    require(isinstance(raw, list), "targets must be an array")
    require(len(raw) == len(REQUIRED_TARGETS), "targets count drifted")
    rows: dict[str, dict[str, Any]] = {}
    for index, value in enumerate(raw):
        require(isinstance(value, dict), f"target {index} must be an object")
        require(set(value) == TARGET_FIELDS, f"target {index} fields drifted")
        target_id = value.get("id")
        require(isinstance(target_id, str) and target_id, f"target {index} id is invalid")
        require(target_id in EXPECTED_BASELINE, f"unsupported architecture budget target {target_id!r}")
        require(target_id not in rows, f"architecture budget target repeats {target_id}")
        metric = value.get("metric")
        require(metric in SUPPORTED_METRICS, f"target {target_id} has unsupported metric {metric!r}")
        path = value.get("path")
        require(isinstance(path, str) and path and not Path(path).is_absolute(), f"target {target_id} path must be repository-relative")
        baseline = integer(value.get("baseline"), f"target {target_id}.baseline")
        ceiling = integer(value.get("ceiling"), f"target {target_id}.ceiling")
        rows[target_id] = value
        rows[target_id]["baseline"] = baseline
        rows[target_id]["ceiling"] = ceiling
    require(set(rows) == set(REQUIRED_TARGETS), "architecture budget target set drifted")
    return rows


def validate_config(config: dict[str, Any]) -> tuple[dict[str, dict[str, Any]], dict[str, int], dict[str, int]]:
    require(config.get("schema_version") == SCHEMA_VERSION, "schema_version drifted")
    policy = config.get("policy")
    require(isinstance(policy, dict), "policy must be an object")
    require(policy.get("default_mode") == "reject-growth", "default policy must reject growth")
    require(integer(policy.get("max_ceiling_growth"), "policy.max_ceiling_growth") == 0, "ceiling growth must remain forbidden")
    require(integer(policy.get("minimum_target_decreases"), "policy.minimum_target_decreases") >= 1, "migration must require a target decrease")
    migration = policy.get("physical_migration")
    require(isinstance(migration, dict), "policy.physical_migration must be an object")
    require(migration.get("ceiling_rule") == "never-increase", "physical migration ceiling rule drifted")
    require(migration.get("require_target_decrease") is True, "physical migration must require a target decrease")

    declared_baseline = metric_map(config.get("baseline"), "baseline")
    require(declared_baseline == EXPECTED_BASELINE, f"locked baseline drifted: expected={EXPECTED_BASELINE} observed={declared_baseline}")
    declared_ceilings = metric_map(config.get("ceilings"), "ceilings")
    rows = target_rows(config)
    for target_id, row in rows.items():
        expected = EXPECTED_BASELINE[target_id]
        require(row["baseline"] == expected, f"target {target_id} baseline drifted")
        require(row["ceiling"] == declared_ceilings[target_id], f"target {target_id} ceiling disagrees with ceilings map")
        require(row["ceiling"] <= expected, f"target {target_id} raises its locked ceiling")
    return rows, declared_baseline, declared_ceilings


def strip_rust_strings_and_comments(text: str) -> str:
    """Remove braces in comments/literals while preserving line boundaries."""

    output: list[str] = []
    index = 0
    state = "code"
    block_depth = 0
    while index < len(text):
        char = text[index]
        next_char = text[index + 1] if index + 1 < len(text) else ""
        if state == "line_comment":
            if char == "\n":
                output.append(char)
                state = "code"
            else:
                output.append(" ")
            index += 1
            continue
        if state == "block_comment":
            if char == "/" and next_char == "*":
                block_depth += 1
                output.extend((" ", " "))
                index += 2
            elif char == "*" and next_char == "/":
                block_depth -= 1
                output.extend((" ", " "))
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                output.append("\n" if char == "\n" else " ")
                index += 1
            continue
        if state in {"string", "char"}:
            terminator = '"' if state == "string" else "'"
            if char == "\\":
                output.append(" ")
                if next_char:
                    output.append("\n" if next_char == "\n" else " ")
                    index += 2
                else:
                    index += 1
            elif char == terminator:
                output.append(" ")
                state = "code"
                index += 1
            else:
                output.append("\n" if char == "\n" else " ")
                index += 1
            continue
        if char == "/" and next_char == "/":
            output.extend((" ", " "))
            state = "line_comment"
            index += 2
        elif char == "/" and next_char == "*":
            output.extend((" ", " "))
            state = "block_comment"
            block_depth = 1
            index += 2
        elif char == '"':
            output.append(" ")
            state = "string"
            index += 1
        elif char == "'" and not (
            next_char.isalnum()
            and index + 2 < len(text)
            and text[index + 2] != "'"
        ):
            # A Rust lifetime (`'a`) or label (`'outer:`) is code, not a
            # character literal.  Only enter the char state when the opening
            # quote has a matching terminator after one character.
            output.append(" ")
            state = "char"
            index += 1
        else:
            output.append(char)
            index += 1
    return "".join(output)


def root_module_declarations(text: str) -> int:
    cleaned = strip_rust_strings_and_comments(text)
    depth = 0
    count = 0
    for line in cleaned.splitlines():
        match = MODULE_DECLARATION.match(line)
        if depth == 0 and match and match.group("name") != "tests":
            # The checked-in test module is a harness, not a product root
            # module.  Keeping it out makes this metric useful for detecting
            # façade/module growth while retaining the 92-module product
            # baseline.
            count += 1
        depth += line.count("{") - line.count("}")
        require(depth >= 0, "Rust root brace depth became negative while counting modules")
    require(depth == 0, "Rust root braces are unbalanced while counting modules")
    return count


def measure_target(row: dict[str, Any], source_root: Path) -> int:
    path = source_root / row["path"]
    require(path.is_file(), f"missing architecture budget source: {display_path(path)}")
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        fail(f"cannot read {display_path(path)}: {exc}")
    metric = row["metric"]
    if metric in {"line_count", "lines"}:
        return len(text.splitlines())
    if metric == "root_module_declarations":
        return root_module_declarations(text)
    fail(f"unsupported metric {metric!r} for {row['id']}")


def extract_reference_metrics(value: dict[str, Any], label: str) -> dict[str, int]:
    for field in ("metrics", "observed", "current", "baseline", "ceilings"):
        candidate = value.get(field)
        if isinstance(candidate, dict) and set(candidate) == set(REQUIRED_TARGETS):
            return metric_map(candidate, f"{label}.{field}")
    raw_targets = value.get("targets")
    if isinstance(raw_targets, list):
        extracted: dict[str, int] = {}
        for row in raw_targets:
            if not isinstance(row, dict) or row.get("id") not in EXPECTED_BASELINE:
                continue
            target_id = row["id"]
            for field in ("observed", "current", "value", "baseline", "ceiling"):
                if field in row:
                    extracted[target_id] = integer(row[field], f"{label}.targets.{target_id}.{field}")
                    break
        if set(extracted) == set(REQUIRED_TARGETS):
            return extracted
    fail(f"{label} does not contain a complete architecture metric snapshot")


def normalise_mode(value: str) -> str:
    aliases = {
        "check": "ceiling",
        "ceiling": "ceiling",
        "reject-growth": "ceiling",
        "compare": "compare",
        "require-decrease": "require-decrease",
        "migration": "require-decrease",
    }
    require(value in aliases, f"unsupported mode {value!r}")
    return aliases[value]


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", default=str(DEFAULT_CONFIG), help="locked architecture budget JSON")
    parser.add_argument("--source-root", default=str(ROOT), help="repository root containing the measured source")
    parser.add_argument("--mode", default="ceiling", help="ceiling (default), compare, or require-decrease")
    parser.add_argument(
        "--compare",
        nargs="?",
        const="",
        metavar="SNAPSHOT",
        help="explicitly compare with the locked baseline or a JSON metric snapshot",
    )
    parser.add_argument("--baseline", help="JSON metric snapshot used as the comparison reference")
    parser.add_argument("--require-decrease", action="store_true", help="require at least one measured target to decrease")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    mode = normalise_mode(args.mode)
    if args.require_decrease:
        mode = "require-decrease"
    if args.compare is not None:
        mode = "require-decrease" if args.require_decrease else "compare"
    require(not (args.compare is not None and args.baseline), "use only one of --compare and --baseline")

    config_path = resolve_path(args.config)
    config = load_json(config_path)
    rows, baseline, ceilings = validate_config(config)
    source_root = resolve_path(args.source_root)
    observed = {target_id: measure_target(row, source_root) for target_id, row in rows.items()}

    reference = baseline
    reference_source = "locked-baseline"
    comparison_path: Path | None = None
    if args.baseline:
        comparison_path = resolve_path(args.baseline)
    elif args.compare:
        if args.compare:
            comparison_path = resolve_path(args.compare)
    if comparison_path is not None:
        reference = extract_reference_metrics(load_json(comparison_path), display_path(comparison_path))
        reference_source = display_path(comparison_path)

    ceiling_violations = [
        target_id for target_id in REQUIRED_TARGETS if observed[target_id] > ceilings[target_id]
    ]
    require(not ceiling_violations, f"current source exceeds ceiling: {ceiling_violations}")
    if mode != "ceiling":
        growth = [target_id for target_id in REQUIRED_TARGETS if observed[target_id] > reference[target_id]]
        require(not growth, f"comparison rejects growth: {growth}")
        if mode == "require-decrease":
            decreased = [target_id for target_id in REQUIRED_TARGETS if observed[target_id] < reference[target_id]]
            require(decreased, "physical migration requires at least one measured target decrease")

    report_targets = []
    for target_id in REQUIRED_TARGETS:
        report_targets.append(
            {
                "id": target_id,
                "path": rows[target_id]["path"],
                "metric": rows[target_id]["metric"],
                "baseline": baseline[target_id],
                "ceiling": ceilings[target_id],
                "observed": observed[target_id],
                "delta_from_baseline": observed[target_id] - baseline[target_id],
                "delta_from_ceiling": observed[target_id] - ceilings[target_id],
                "delta_from_reference": observed[target_id] - reference[target_id],
                "status": "PASS",
            }
        )
    print(
        json.dumps(
            {
                "schema_version": REPORT_SCHEMA_VERSION,
                "status": "PASS",
                "mode": mode,
                "reference": reference_source,
                "policy": {
                    "default": "reject-growth",
                    "ceiling_growth": "forbidden",
                    "require_decrease": mode == "require-decrease",
                },
                "targets": report_targets,
            },
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
