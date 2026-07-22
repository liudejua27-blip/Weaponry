#!/usr/bin/env python3
"""K002 shared Product Tool Registry manifest smoke.

The committed fixture is the language-neutral contract consumed by the Rust
Agent runtime and the temporary Python product-tool executor.  Its source is
the existing A004 code-owned registry; this smoke prevents either side from
silently maintaining a second list or a second JSON Schema.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from collections.abc import Iterator, Mapping, Sequence
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

from forgecad_agent.application.product_tool_registry import forgecad_product_tool_registry


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_PATH = ROOT / "packages/concept-spec/fixtures/k002-product-tool-registry.json"
A004_FIXTURE_PATH = ROOT / "packages/concept-spec/fixtures/k001-a004-turn-compatibility.json"

FIXTURE_SCHEMA_VERSION = "K002ProductToolRegistryFixture@1"
FIXTURE_ID = "k002_shared_a004_product_tool_registry"
CANONICALIZATION = {
    "algorithm": "sha256",
    "encoding": "utf-8",
    "ensure_ascii": False,
    "json_separators": [",", ":"],
    "json_sort_keys": True,
    "manifest_hash_scope": "public_manifest_without_derived_hashes",
}
PUBLIC_TOOL_KEYS = {
    "tool_id",
    "name",
    "description",
    "input_schema",
    "output_schema",
    "approval_policy",
}
FIXTURE_TOOL_KEYS = PUBLIC_TOOL_KEYS | {
    "input_schema_sha256",
    "output_schema_sha256",
}
FIXTURE_KEYS = {
    "schema_version",
    "fixture_id",
    "registry_schema_version",
    "canonicalization",
    "tools",
    "manifest_sha256",
}
FORBIDDEN_CONTRACT_KEYS = {
    "api_key",
    "base_url",
    "deepseek_api_key",
    "endpoint_url",
    "hidden_reasoning",
    "provider_api_key",
    "provider_key",
    "provider_url",
    "reasoning",
    "reasoning_content",
    "secret",
}
TOOL_ID_PATTERN = re.compile(r"^forgecad\.[a-z0-9_.\-]+\.v1$")
TOOL_NAME_PATTERN = re.compile(r"^[a-z][a-z0-9_]{1,63}$")


def _canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def _sha256(value: Any) -> str:
    return hashlib.sha256(_canonical_bytes(value)).hexdigest()


def _public_manifest() -> dict[str, Any]:
    return forgecad_product_tool_registry().public_manifest().model_dump(mode="json")


def _build_fixture(public_manifest: Mapping[str, Any]) -> dict[str, Any]:
    tools: list[dict[str, Any]] = []
    for raw_tool in public_manifest["tools"]:
        tool = dict(raw_tool)
        tool["input_schema_sha256"] = _sha256(tool["input_schema"])
        tool["output_schema_sha256"] = _sha256(tool["output_schema"])
        tools.append(tool)
    return {
        "schema_version": FIXTURE_SCHEMA_VERSION,
        "fixture_id": FIXTURE_ID,
        "registry_schema_version": public_manifest["schema_version"],
        "canonicalization": CANONICALIZATION,
        "tools": tools,
        "manifest_sha256": _sha256(public_manifest),
    }


def _public_projection(fixture: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": fixture["registry_schema_version"],
        "tools": [
            {key: tool[key] for key in PUBLIC_TOOL_KEYS}
            for tool in fixture["tools"]
        ],
    }


def _walk(value: Any, path: tuple[str, ...] = ()) -> Iterator[tuple[tuple[str, ...], Any]]:
    yield path, value
    if isinstance(value, Mapping):
        for key, child in value.items():
            yield from _walk(child, (*path, str(key)))
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        for index, child in enumerate(value):
            yield from _walk(child, (*path, str(index)))


def _assert_schema_contracts(tools: Sequence[Mapping[str, Any]]) -> None:
    for index, tool in enumerate(tools):
        assert set(tool) == FIXTURE_TOOL_KEYS, (
            f"tool[{index}] record is not closed: {sorted(set(tool) ^ FIXTURE_TOOL_KEYS)}"
        )
        assert TOOL_ID_PATTERN.fullmatch(str(tool["tool_id"])), tool["tool_id"]
        assert TOOL_NAME_PATTERN.fullmatch(str(tool["name"])), tool["name"]
        assert tool["approval_policy"] in {"read_only", "candidate_only"}

        input_schema = tool["input_schema"]
        output_schema = tool["output_schema"]
        Draft202012Validator.check_schema(input_schema)
        Draft202012Validator.check_schema(output_schema)
        assert input_schema.get("type") == "object"
        assert input_schema.get("additionalProperties") is False, (
            f"{tool['name']} input schema must be top-level closed"
        )
        # A004 output contracts intentionally expose a required-facts floor and
        # allow the bounded executor to return additional readback evidence.
        # Preserve that source truth, but require the openness to be explicit.
        assert output_schema.get("type") == "object"
        assert output_schema.get("additionalProperties") is True, (
            f"{tool['name']} output openness must be explicit"
        )
        assert _sha256(input_schema) == tool["input_schema_sha256"]
        assert _sha256(output_schema) == tool["output_schema_sha256"]

        for path, value in _walk(input_schema):
            if path and path[-1] == "$ref":
                assert isinstance(value, str) and value.startswith("#/"), (
                    f"{tool['name']} contains an external JSON Schema reference: {value!r}"
                )
        for path, value in _walk(output_schema):
            if path and path[-1] == "$ref":
                assert isinstance(value, str) and value.startswith("#/"), (
                    f"{tool['name']} contains an external JSON Schema reference: {value!r}"
                )


def _assert_no_provider_or_reasoning_fields(fixture: Mapping[str, Any]) -> None:
    for path, value in _walk(fixture):
        if path:
            key = path[-1].casefold()
            assert key not in FORBIDDEN_CONTRACT_KEYS, (
                f"forbidden provider/reasoning field at {'/'.join(path)}"
            )
        if isinstance(value, str):
            assert not re.search(r"https?://", value, re.IGNORECASE), (
                f"fixture contains a provider or external URL at {'/'.join(path)}"
            )


def _assert_a004_compatibility(fixture: Mapping[str, Any]) -> None:
    a004 = json.loads(A004_FIXTURE_PATH.read_text(encoding="utf-8"))
    expected_names = a004["expected_product_tools"]
    ordered_markers = a004["expected_ordered_markers"]
    marker_names = [
        marker.removeprefix("tool_call:")
        for marker in ordered_markers
        if marker.startswith("tool_call:")
    ]
    assert marker_names == expected_names

    tools = fixture["tools"]
    names = [tool["name"] for tool in tools]
    ids = [tool["tool_id"] for tool in tools]
    assert len(names) == len(set(names)) == 13
    assert len(ids) == len(set(ids)) == 13
    assert names[-len(expected_names) :] == expected_names, (
        "A004 Product Tool execution order drifted from the shared registry"
    )
    a004_ids = [tool["tool_id"] for tool in tools if tool["name"] in set(expected_names)]
    assert len(a004_ids) == len(expected_names)
    assert all(TOOL_ID_PATTERN.fullmatch(tool_id) for tool_id in a004_ids)


def _verify() -> dict[str, Any]:
    public_manifest = _public_manifest()
    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    expected_fixture = _build_fixture(public_manifest)

    assert set(fixture) == FIXTURE_KEYS, (
        f"fixture record is not closed: {sorted(set(fixture) ^ FIXTURE_KEYS)}"
    )
    assert fixture == expected_fixture, (
        "K002 Product Tool fixture drifted from the A004 code-owned registry; "
        "review the contract change and regenerate explicitly"
    )
    assert fixture["canonicalization"] == CANONICALIZATION
    assert len(fixture["tools"]) == 13

    public_projection = _public_projection(fixture)
    assert public_projection == public_manifest
    assert _sha256(public_projection) == fixture["manifest_sha256"]
    assert all(
        tool["approval_policy"] != "user_confirmation_required"
        for tool in fixture["tools"]
    )

    _assert_schema_contracts(fixture["tools"])
    _assert_no_provider_or_reasoning_fields(fixture)
    _assert_a004_compatibility(fixture)
    return fixture


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write-fixture",
        action="store_true",
        help="Regenerate the reviewed fixture from the current A004 registry.",
    )
    args = parser.parse_args()

    if args.write_fixture:
        fixture = _build_fixture(_public_manifest())
        FIXTURE_PATH.write_text(
            json.dumps(fixture, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )

    fixture = _verify()
    print(
        "K002 Product Tool manifest smoke passed: "
        f"tools={len(fixture['tools'])}, "
        f"manifest_sha256={fixture['manifest_sha256']}, "
        "A004_order=compatible, provider_fields=0, reasoning_fields=0"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
