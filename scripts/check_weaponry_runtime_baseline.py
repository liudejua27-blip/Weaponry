#!/usr/bin/env python3
"""Fail closed when the fast Runtime architecture baseline drifts."""

from __future__ import annotations

import json
import re
import sys
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = (
    ROOT
    / "apps"
    / "desktop"
    / "src-tauri"
    / "crates"
    / "forgecad-runtime"
    / "ignored-tests.json"
)
RUNTIME_SRC = POLICY_PATH.parent / "src"
CODEX_CONFIGS = (
    ROOT / "config" / "codex" / "cli.toml",
    ROOT / "config" / "codex" / "desktop.toml",
    ROOT / "config" / "codex" / "ide.toml",
)
IPC_SOURCE = RUNTIME_SRC / "ipc.rs"
EXPECTED_TIMEOUT_SECONDS = 180
EXPECTED_IGNORED_CATEGORIES = {
    "platform_limited",
    "historical_compatibility",
    "fixture_required",
    "real_coverage_gap",
}
IGNORE_PATTERN = re.compile(
    r'#\[ignore = "([^"]+)"\]\s*fn\s+([A-Za-z0-9_]+)', re.MULTILINE
)


def refuse(message: str) -> None:
    raise SystemExit(f"WPN_ARCH_BASELINE_FAST_INVALID: {message}")


def validate_timeout_contract() -> None:
    observed: dict[str, int] = {}
    for path in CODEX_CONFIGS:
        config_text = path.read_text(encoding="utf-8")
        matches = re.findall(r"(?m)^tool_timeout_sec\s*=\s*(\d+)\s*$", config_text)
        if len(matches) != 1:
            refuse(
                f"expected one ForgeCAD tool timeout in {path.relative_to(ROOT)}, "
                f"found {len(matches)}"
            )
        timeout = int(matches[0])
        observed[path.relative_to(ROOT).as_posix()] = timeout

    if set(observed.values()) != {EXPECTED_TIMEOUT_SECONDS}:
        refuse(
            "Codex host timeouts must all equal the Runtime IPC timeout; "
            f"expected={EXPECTED_TIMEOUT_SECONDS} observed={observed}"
        )

    ipc_source = IPC_SOURCE.read_text(encoding="utf-8")
    marker = (
        "const IPC_REQUEST_TIMEOUT: Duration = "
        f"Duration::from_secs({EXPECTED_TIMEOUT_SECONDS});"
    )
    if marker not in ipc_source:
        refuse("Runtime IPC timeout source marker drifted")


def validate_ignored_test_policy() -> Counter[str]:
    policy = json.loads(POLICY_PATH.read_text(encoding="utf-8"))
    if policy.get("schema_version") != "WeaponryRuntimeIgnoredTestPolicy@1":
        refuse("ignored-test policy schema_version drifted")
    if policy.get("coverage_status") != "INVENTORY_CLOSED_EXECUTION_NOT_PROVEN":
        refuse("ignored-test policy must distinguish inventory from execution coverage")
    if policy.get("current_cohort_ignored_tests_executed") != 0:
        refuse("ignored-test policy must not claim unrecorded current-cohort execution")
    tests = policy.get("tests")
    if not isinstance(tests, list):
        refuse("ignored-test policy tests must be an array")
    expected_count = policy.get("expected_count")
    if expected_count != len(tests):
        refuse(
            f"ignored-test expected_count drifted: declared={expected_count} actual={len(tests)}"
        )

    declared: dict[tuple[str, str], tuple[str, str]] = {}
    declared_by_marker: dict[tuple[str, str], tuple[str, str]] = {}
    category_counts: Counter[str] = Counter()
    for index, row in enumerate(tests):
        if not isinstance(row, dict):
            refuse(f"ignored-test row {index} must be an object")
        required = {"source_path", "test_name", "category", "owner", "reason"}
        if set(row) != required:
            refuse(
                f"ignored-test row {index} fields drifted: "
                f"expected={sorted(required)} actual={sorted(row)}"
            )
        if not all(isinstance(row[field], str) and row[field] for field in required):
            refuse(f"ignored-test row {index} contains an empty or non-string field")
        key = (row["source_path"], row["test_name"])
        if key in declared:
            refuse(f"ignored-test policy repeats {row['test_name']}")
        declared[key] = (row["reason"], row["category"])
        marker_key = (row["source_path"], row["test_name"].rsplit("::", 1)[-1])
        if marker_key in declared_by_marker:
            refuse(f"ignored-test policy repeats source marker {marker_key}")
        declared_by_marker[marker_key] = key
        category_counts[row["category"]] += 1

    categories = policy.get("categories")
    if not isinstance(categories, dict) or not categories:
        refuse("ignored-test categories must be a non-empty object")
    if set(categories) != EXPECTED_IGNORED_CATEGORIES:
        refuse(
            "ignored-test audit categories must cover platform, historical "
            f"compatibility, fixture and real gaps: actual={sorted(categories)}"
        )
    if set(categories) != set(category_counts):
        refuse(
            "ignored-test category directory drifted: "
            f"declared={sorted(categories)} actual={sorted(category_counts)}"
        )
    for category, count in category_counts.items():
        metadata = categories[category]
        if not isinstance(metadata, dict):
            refuse(f"ignored-test category {category} metadata must be an object")
        if set(metadata) != {"expected_count", "lane", "owner", "meaning"}:
            refuse(f"ignored-test category {category} metadata fields drifted")
        if metadata.get("expected_count") != count:
            refuse(
                f"ignored-test category {category} count drifted: "
                f"declared={metadata.get('expected_count')} actual={count}"
            )
        if metadata.get("lane") not in {
            "qualification",
            "fixture-input",
            "historical-replay",
            "retirement",
        }:
            refuse(f"ignored-test category {category} has unsupported lane")

    actual: dict[tuple[str, str], str] = {}
    unlisted_markers: list[tuple[str, str]] = []
    for source in sorted(RUNTIME_SRC.rglob("*.rs")):
        source_path = source.relative_to(POLICY_PATH.parent).as_posix()
        text = source.read_text(encoding="utf-8")
        for reason, function_name in IGNORE_PATTERN.findall(text):
            marker_key = (source_path, function_name)
            key = declared_by_marker.get(marker_key)
            if key is None:
                unlisted_markers.append(marker_key)
                continue
            if key in actual:
                refuse(f"source repeats ignored test {key[1]}")
            actual[key] = reason

    if unlisted_markers:
        refuse(f"Runtime source contains unclassified ignored tests: {unlisted_markers}")

    if set(actual) != set(declared):
        missing = sorted(set(actual) - set(declared))
        stale = sorted(set(declared) - set(actual))
        refuse(f"ignored-test inventory drifted: missing={missing} stale={stale}")
    for key, actual_reason in actual.items():
        declared_reason, _ = declared[key]
        if actual_reason != declared_reason:
            refuse(
                f"ignored-test reason drifted for {key[1]}: "
                f"declared={declared_reason!r} actual={actual_reason!r}"
            )
    return category_counts


def main() -> int:
    validate_timeout_contract()
    category_counts = validate_ignored_test_policy()
    print(
        json.dumps(
            {
                "schema_version": "WeaponryRuntimeArchitectureBaselineCheck@1",
                "status": "PASS",
                "tool_timeout_sec": EXPECTED_TIMEOUT_SECONDS,
                "ignored_test_count": sum(category_counts.values()),
                "ignored_test_categories": dict(sorted(category_counts.items())),
                "ignored_test_inventory_status": "CLOSED",
                "ignored_test_execution_status": "NOT_PROVEN",
                "current_cohort_ignored_tests_executed": 0,
            },
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
