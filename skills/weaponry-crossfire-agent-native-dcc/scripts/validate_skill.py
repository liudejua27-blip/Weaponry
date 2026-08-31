#!/usr/bin/env python3
"""Validate the repository's knife orchestration Skill surface.

This is a static safety/structure check. It never loads attachments, starts a
worker, calls MCP, reads SQLite/CAS, or treats a benchmark profile as product
truth.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / "SKILL.md"
UI = ROOT / "agents" / "openai.yaml"
REFERENCES = ROOT / "references"
PROFILE = ROOT.parents[1] / "packages" / "forgecad-contracts" / "profiles" / "weaponry-knife-p0.json"

FACADES = (
    "weapon_preflight",
    "reference_intake",
    "observe",
    "authoring_transaction",
    "surface_pipeline",
    "fps_presentation",
    "quality_review",
    "delivery",
    "approval",
    "recovery",
    "job",
)

REQUIRED_SKILL_MARKERS = (
    "Runtime is the sole writer",
    "closed\ntyped operations",
    "ponytail-preflight@0.1.0",
    "weaponry-knife-p0-default@1",
    "weaponry_knife_production_brief_prepare",
    "weaponry_knife_production_brief_get",
    "immutable-successor-preserve-source-claims@1",
    "KnifeReferenceIntentBundle@1",
    "KnifePassState@1",
    "KnifeCorrectionLedger@1",
    "Curve/AuthoringTransaction",
    "High → editable Low → UV/Cage/Bake → Material → FPS",
    "Quality(pre-delivery) → Engine → Quality(final human)",
    "Only explicit user approval",
    "PASS`, `FAIL`, `BLOCKED`, and `NOT_RUN",
    "commercial=NOT_PROVEN",
)

# These patterns catch copied identity/source data while allowing the reference
# to explain that such data is excluded.
FORBIDDEN_REFERENCE_PATTERNS = (
    re.compile(r"[\w.+-]+@[\w.-]+\.[a-z]{2,}", re.IGNORECASE),
    re.compile(r"(?:\+?\d[\d ()-]{7,}\d)"),
    re.compile(r"(?:data:image/|data:application/|base64,)", re.IGNORECASE),
)


def fail(message: str) -> None:
    raise AssertionError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def read(path: Path) -> str:
    require(path.is_file(), f"missing file: {path.relative_to(ROOT)}")
    return path.read_text(encoding="utf-8")


def check_frontmatter(content: str) -> None:
    match = re.match(r"^---\n(.*?)\n---\n", content, re.DOTALL)
    require(match is not None, "SKILL.md frontmatter is missing")
    frontmatter = match.group(1)
    require("name: weaponry-crossfire-agent-native-dcc" in frontmatter, "Skill name drifted")
    require("description:" in frontmatter, "Skill description is missing")
    require("TODO" not in frontmatter, "frontmatter contains a scaffold placeholder")


def check_facade_route(content: str) -> None:
    positions: list[int] = []
    for index, facade in enumerate(FACADES, start=1):
        marker = f"{index}. `{facade}`"
        position = content.find(marker)
        require(position >= 0, f"missing ordered façade: {facade}")
        positions.append(position)
    require(positions == sorted(positions), "11 façade route is not in profile order")
    require(content.count("`weapon_preflight`") >= 2, "preflight façade is not operationally referenced")
    require(content.count("`authoring_transaction`") >= 2, "authoring façade is not operationally referenced")


def check_ui() -> None:
    content = read(UI)
    # This Skill remains explicit-only by product policy; it is still discoverable
    # by its name and default prompt, and must not silently activate on unrelated work.
    require("allow_implicit_invocation: false" in content, "Skill invocation policy must remain explicit-only")
    require("$weaponry-crossfire-agent-native-dcc" in content, "default prompt must name the Skill")


def check_profile_binding(content: str, profile: dict[str, object]) -> None:
    default_profile = profile.get("default_profile")
    require(isinstance(default_profile, dict), "knife profile default_profile is missing")
    facade_names = default_profile.get("facade_names")
    require(facade_names == list(FACADES), "Skill façade order drifted from the contracts profile")
    require(profile.get("profile_status") == "development-only", "expected development-only profile status drifted")
    require("development-only" in content and "commercial=NOT_PROVEN" in content, "Skill hides the live development status")


def check_live_profile_binding(content: str) -> None:
    require(PROFILE.is_file(), "weaponry knife profile is missing")
    try:
        profile = json.loads(PROFILE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot read weaponry knife profile: {exc}")
    require(isinstance(profile, dict), "weaponry knife profile must be an object")
    check_profile_binding(content, profile)


def check_references(content: str) -> None:
    expected = {
        "action-space-and-gates.md",
        "dragonfang-benchmark-profile.md",
        "knife-reference-convergence-loop.md",
    }
    actual = {path.name for path in REFERENCES.glob("*.md")}
    require(actual == expected, f"unexpected or missing references: {sorted(actual ^ expected)}")
    for name in expected:
        reference = read(REFERENCES / name)
        require(f"references/{name}" in content, f"SKILL.md does not route to {name}")
        require("TODO" not in reference, f"{name} contains an unfinished placeholder")
        if name == "dragonfang-benchmark-profile.md":
            check_sanitized_benchmark(reference)
            require("embedded_source: false" in reference, "benchmark profile embeds source content")
            require("authorization: required-at-runtime" in reference, "benchmark authorization is not runtime-bound")
            require("replacement_policy:" in reference, "benchmark replacement policy is missing")
            require("CONFLICT_PENDING" in reference, "brief conflict handling is missing")
            require("UNRESOLVED" in reference, "unresolved value handling is missing")


def check_sanitized_benchmark(reference: str) -> None:
    for pattern in FORBIDDEN_REFERENCE_PATTERNS:
        require(
            pattern.search(reference) is None,
            f"sanitized benchmark contains copied identity/source data: {pattern.pattern}",
        )


def check_static_markers(content: str) -> None:
    for marker in REQUIRED_SKILL_MARKERS:
        require(marker in content, f"missing orchestration invariant: {marker}")
    for forbidden in ("execute caller-supplied", "automatically approve", "direct CAS write"):
        require(forbidden not in content.lower(), f"unsafe instruction leaked into Skill: {forbidden}")


def check_no_source_payloads() -> None:
    forbidden_suffixes = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".blend", ".glb", ".fbx", ".zip"}
    payloads = [path.relative_to(ROOT) for path in ROOT.rglob("*") if path.is_file() and path.suffix.lower() in forbidden_suffixes]
    require(not payloads, f"Skill contains source/asset payloads: {payloads}")


def validate() -> None:
    content = read(SKILL)
    check_frontmatter(content)
    check_static_markers(content)
    check_facade_route(content)
    check_live_profile_binding(content)
    check_ui()
    check_references(content)
    check_no_source_payloads()


if __name__ == "__main__":
    try:
        validate()
    except (AssertionError, OSError) as exc:
        print(f"Weaponry knife Skill validation FAILED: {exc}", file=sys.stderr)
        raise SystemExit(1)
    print("Weaponry knife orchestration Skill validation PASS: explicit discovery, 11 façades, conflict freeze, sanitized benchmark, and no source payloads")
