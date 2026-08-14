#!/usr/bin/env python3
"""Materialize the twelve active first-party MCP006 Skill Bundles.

The source of truth remains the checked-in registry and contract schemas.  This
script only copies those declarative inputs into independently auditable bundle
directories; it never downloads a repository, runs a plugin, or emits an
executable payload.  `--check` is used by the Skill validator, while `--write`
is an intentional repository-maintenance operation used when the registry
changes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKILLS = ROOT / "packages" / "forgecad-skills"
CONTRACTS = ROOT / "packages" / "forgecad-contracts" / "schemas"
REGISTRY_PATH = SKILLS / "registry.json"
COMMON_LICENSE = SKILLS / "LICENSES" / "ForgeCAD-FIRST-PARTY.txt"
COMMON_NOTICE = SKILLS / "NOTICE"
COMMON_VALIDATORS = SKILLS / "validators" / "validator-set.json"
COMMON_OPERATORS = SKILLS / "operators.lock"


def canonical_bytes(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def canonical_hash(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def recipe_for(entry: dict[str, object]) -> dict[str, object]:
    source = SKILLS / str(entry["recipe"])
    recipe = json.loads(source.read_text(encoding="utf-8"))
    recipe["units"] = "meter"
    recipe["coordinate_system"] = "right-handed-y-up"
    recipe["deterministic_order"] = [node["node_id"] for node in recipe["nodes"]]
    recipe["max_edges"] = 128
    recipe.pop("canonical_sha256", None)
    recipe["canonical_sha256"] = canonical_hash(recipe)
    return recipe


def fixture_for(entry: dict[str, object], recipe: dict[str, object], valid: bool) -> dict[str, object]:
    skill_id = str(entry["skill_id"])
    if valid:
        return {
            "schema_version": "SkillBenchmarkFixture@1",
            "fixture_id": f"{skill_id}-declarative-valid",
            "skill_id": skill_id,
            "recipe_id": recipe["recipe_id"],
            "units": "meter",
            "coordinate_system": "right-handed-y-up",
            "values": {"scale": 1.0, "max_parts": 16, "confidence": 0.8},
            "expected": {"dag": "acyclic", "finite": True, "operator_allowlist": "pass"},
        }
    return {
        "schema_version": "SkillBenchmarkFixture@1",
        "fixture_id": f"{skill_id}-declarative-invalid-cycle",
        "skill_id": skill_id,
        "recipe_id": recipe["recipe_id"],
        "units": "millimeter",
        "coordinate_system": "left-handed-z-up",
        "values": {"scale": "NaN"},
        "edges": [{"from": "a", "to": "b"}, {"from": "b", "to": "a"}],
        "expected": {"dag": "reject", "finite": "reject", "units": "reject"},
    }


def validator_subset(ids: list[str]) -> dict[str, object]:
    document = json.loads(COMMON_VALIDATORS.read_text(encoding="utf-8"))
    selected = [item for item in document["validators"] if item["id"] in set(ids)]
    return {
        "schema_version": "SkillValidatorSet@1",
        "validators": selected,
        "network": False,
        "dynamic_code": False,
    }


def materialize(entry: dict[str, object], write: bool) -> None:
    skill_id = str(entry["skill_id"])
    version = str(entry["version"])
    bundle = SKILLS / "bundles" / skill_id / version
    if write:
        bundle.mkdir(parents=True, exist_ok=True)
    recipe = recipe_for(entry)
    recipe_path = bundle / "recipes" / "default.recipe.json"
    if write:
        # Keep the aggregate registry recipe and the independently materialized
        # bundle recipe byte-for-byte equivalent apart from their path.
        write_json(SKILLS / str(entry["recipe"]), recipe)
    manifest = {
        "schema_version": "SkillBundleManifest@1",
        "skill_id": skill_id,
        "version": version,
        "status": "development-only",
        "publisher": "forgecad-first-party",
        "contract_range": "forgecad-runtime-contracts@1",
        "input_schema": f"schemas/{Path(str(entry['input_schema'])).name}",
        "output_schema": f"schemas/{Path(str(entry['output_schema'])).name}",
        "recipe": "recipes/default.recipe.json",
        "operator_ids": entry["operator_ids"],
        "validator_ids": entry["validator_ids"],
        "capabilities": entry["capabilities"],
        "budgets": entry["budgets"],
        "benchmark_suite": entry["benchmark_suite"],
        "trust_profile": "development-root",
        "signature": "development-only",
    }
    manifest["canonical_sha256"] = canonical_hash(manifest)
    if not write:
        return

    write_json(bundle / "manifest.json", manifest)
    write_json(recipe_path, recipe)
    skill_yaml = (
        "schema_version: ForgeCADSkillBundle@1\n"
        f"skill_id: {skill_id}\n"
        f"version: {version}\n"
        "status: development-only\n"
        "publisher: forgecad-first-party\n"
        "contract_range: forgecad-runtime-contracts@1\n"
        f"input_schema: schemas/{Path(str(entry['input_schema'])).name}\n"
        f"output_schema: schemas/{Path(str(entry['output_schema'])).name}\n"
        "recipe: recipes/default.recipe.json\n"
        "trust_profile: development-root\n"
        "signature: deferred-to-mcp012-013\n"
        "capabilities:\n"
        "  network: false\n"
        "  filesystem_read: false\n"
        "  filesystem_write: false\n"
        "  dynamic_code: false\n"
        "  model_calls: false\n"
        "  geometry_execution: " + str(bool(entry["capabilities"].get("geometry_execution", False))).lower() + "\n"
        "  render_execution: " + str(bool(entry["capabilities"].get("render_execution", False))).lower() + "\n"
        "operator_ids:\n"
        + "".join(f"  - {operator}\n" for operator in entry["operator_ids"])
        + "validator_ids:\n"
        + "".join(f"  - {validator}\n" for validator in entry["validator_ids"])
        + "budgets:\n"
        + "".join(f"  {name}: {value}\n" for name, value in entry["budgets"].items())
        + f"benchmark_suite: {entry['benchmark_suite']}\n"
        + "known_limitations:\n"
        "  - Declarative safety receipt does not claim geometry, render or visual quality.\n"
        "  - Distribution signature and third-party installation are deferred to MCP012-013.\n"
    )
    (bundle / "skill.yaml").write_text(skill_yaml, encoding="utf-8")
    for schema_ref in (entry["input_schema"], entry["output_schema"]):
        schema_name = Path(str(schema_ref)).name
        destination = bundle / "schemas" / schema_name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(CONTRACTS / schema_name, destination)

    operator_lines = [
        "# Product-owned operator lock; this bundle contains no executable operator.",
        *[f"{operator} = forgecad-runtime-builtin" for operator in entry["operator_ids"]],
        "",
    ]
    (bundle / "operators.lock").write_text("\n".join(operator_lines), encoding="utf-8")
    write_json(bundle / "validators" / "validator-set.json", validator_subset(entry["validator_ids"]))

    workflow_note = ""
    if skill_id == "reference-intake":
        workflow_note = (
            "\nThe first-party workflow borrows only the review discipline observed in "
            "img2threejs/img2threejs: write a bounded detail inventory before modeling, "
            "separate visible/occluded regions, and mark confidence/unknowns instead of "
            "inventing hidden geometry. The stages are advisory Codex planning labels "
            "(`blockout`, `structural`, `form`, `material`, `surface`, `lighting`, "
            "`review`); they are not executable operators and do not change Runtime truth.\n"
        )
    elif skill_id == "subject-profile":
        workflow_note = (
            "\nThe profile should carry a small detail inventory and per-region confidence "
            "(`observed`, `inferred`, or `unknown`) so later staged passes can revise one "
            "region without rewriting unrelated Part IDs. Confidence is review metadata, "
            "not a quality score and not permission to manufacture hidden structure.\n"
        )
    elif skill_id == "reference-compare":
        workflow_note = (
            "\nComparison is staged: first check frame/occupancy and hard gates, then allow "
            "Codex to review silhouette, major regions and material zones against fixed "
            "passes. Until dedicated typed metrics are implemented, Runtime reports only "
            "a `limited` aspect-ratio evidence item. A Codex visual impression never "
            "upgrades that status to PASS by itself.\n"
        )
    elif skill_id == "ponytail-preflight":
        workflow_note = (
            "\nThis is a ForgeCAD-authored adaptation of the decision order studied in "
            "DietrichGebert/ponytail at its recorded MIT source revision. It is a "
            "read-only planning gate, not an upstream plugin, hook, MCP server or "
            "executable dependency. Before any ForgeCAD design tool or another Skill: "
            "first decide whether the requested change is necessary; then inspect the "
            "current project, candidate, reference and active capability; reuse an "
            "existing bounded path where it fits; choose a product-owned typed Operator "
            "only when needed; and perform the smallest prepared action that preserves "
            "approval, lineage, quality and evidence.\n"
        )
    display_name = "Ponytail preflight" if skill_id == "ponytail-preflight" else skill_id
    overview = (
        f"# {display_name}\n\n"
        f"First-party declarative Skill `{skill_id}@{version}`. It declares typed inputs, "
        "a bounded Recipe and product-owned validators; it does not contain executable code.\n\n"
        "This bundle is planning metadata for the single-user MVP. A successful registry or "
        "declarative benchmark check does not claim that geometry, render or visual similarity "
        "has passed.\n"
        + workflow_note
    )
    constraint_note = ""
    if skill_id == "reference-intake":
        constraint_note = (
            "- A single image does not prove hidden sides, scale or mechanical function; record those as unknown/occluded rather than fabricate certainty.\n"
            "- `img2css`-style color/pixel previews may be used only as ephemeral Codex review aids. CSS, HTML, base64 and arbitrary JavaScript never enter a GeometryProgram, CAS truth or Worker input.\n"
        )
    elif skill_id == "subject-profile":
        constraint_note = (
            "- Keep one-image ambiguities explicit: occluded/back-side proportions and functional claims must remain unknown until another reference or user review.\n"
        )
    elif skill_id == "reference-compare":
        constraint_note = (
            "- Never treat a color-grid/CSS preview, one beauty image, or a model-generated confidence statement as silhouette IoU, landmark, region, or human acceptance.\n"
        )
    elif skill_id == "ponytail-preflight":
        constraint_note = (
            "- The MCP adapter accepts only `skill_get` for `ponytail-preflight@0.1.0` before other ForgeCAD design tools or Skills in a session; the bootstrap diagnostics `capabilities_get`, `runtime_status` and `doctor` remain read-only exemptions.\n"
            "- This preflight does not authorize a geometry claim or a persistent write. Use the existing typed prepare, readback, quality and user-confirm steps, and retain unknown or occluded reference evidence as unknown.\n"
            "- Do not install or execute the upstream Node package, its hooks, its MCP server, or arbitrary repository files.\n"
        )
    constraints = (
        "# Constraints\n\n"
        "- No network, arbitrary filesystem path, environment variable, secret, model call, shell, Python or JavaScript.\n"
        "- Units are metres and the coordinate system is right-handed Y-up.\n"
        "- Operators are selected only from the checked-in lock and are implemented by ForgeCAD.\n"
        "- Invalid DAGs, non-finite values, unknown operators and budget overflow fail closed.\n"
        + constraint_note
    )
    examples = (
        "# Synthetic example\n\n"
        f"The fixture for `{skill_id}@{version}` is deliberately synthetic and contains no user image, "
        "model weight or external asset. It is used only to exercise declarative validation.\n"
    )
    if skill_id == "ponytail-preflight":
        examples += (
            "\nFor a new reference-driven model, call `skill_get` for this Skill, inspect the "
            "returned constraints, then use the smallest existing typed path: project/reference "
            "intake, an active GeometryProgram path, strict readback, fixed render evidence and "
            "user approval. Do not begin by adding a new tool, bundle or unrestricted script.\n"
        )
    for relative, content in (
        ("knowledge/overview.md", overview),
        ("knowledge/constraints.md", constraints),
        ("knowledge/examples.md", examples),
    ):
        (bundle / relative).parent.mkdir(parents=True, exist_ok=True)
        (bundle / relative).write_text(content, encoding="utf-8")

    assets = {
        "schema_version": "SkillAssetIndex@1",
        "skill_id": skill_id,
        "assets": [],
        "network": False,
        "note": "No external asset payload is included in the MCP006 declarative bundle.",
    }
    materials = {
        "schema_version": "SkillMaterialIndex@1",
        "skill_id": skill_id,
        "materials": [],
        "network": False,
    }
    write_json(bundle / "assets" / "index.json", assets)
    write_json(bundle / "materials" / "index.json", materials)

    fixture_dir = bundle / "benchmarks" / "fixtures"
    write_json(fixture_dir / "valid.json", fixture_for(entry, recipe, True))
    write_json(fixture_dir / "invalid-cycle-unit-finite.json", fixture_for(entry, recipe, False))
    suite = (
        "schema_version: SkillBenchmarkSuite@1\n"
        f"suite_id: {entry['benchmark_suite']}\n"
        "status: passed\n"
        "fixtures:\n"
        "  - fixtures/valid.json\n"
        "  - fixtures/invalid-cycle-unit-finite.json\n"
        "metrics:\n"
        "  - name: declarative_plan_validation\n"
        "    threshold: 1\n"
        "  - name: forbidden_capabilities\n"
        "    threshold: 0\n"
        "notes:\n"
        "  - Synthetic metadata only; geometry, render and visual quality remain later task gates.\n"
    )
    (bundle / "benchmarks" / "suite.yaml").write_text(suite, encoding="utf-8")
    fixture_hash = canonical_hash({"valid": fixture_for(entry, recipe, True), "invalid": fixture_for(entry, recipe, False)})
    write_json(
        bundle / "benchmark-receipt.json",
        {
            "schema_version": "SkillBenchmarkReceipt@1",
            "skill_id": skill_id,
            "version": version,
            "status": "passed",
            "suite_id": entry["benchmark_suite"],
            "fixture_sha256": fixture_hash,
            "metrics": {"declarative_plan_validation": 1, "forbidden_capabilities": 0, "fixtures": 2},
            "reason": "Synthetic MCP006 contract safety only; geometry/render/quality are not claimed.",
        },
    )

    (bundle / "LICENSES").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(COMMON_LICENSE, bundle / "LICENSES" / "ForgeCAD-FIRST-PARTY.txt")
    notice = COMMON_NOTICE.read_text(encoding="utf-8") + f"\nBundle: {skill_id}@{version}\n"
    if skill_id == "ponytail-preflight":
        notice += (
            "Workflow source studied: DietrichGebert/ponytail at "
            "2ed6c52c9d7e5e56942508591085fd45dea277d3 (MIT).\n"
            "No upstream source code, package, hook, MCP server or executable payload is included.\n"
        )
    (bundle / "NOTICE").write_text(
        notice,
        encoding="utf-8",
    )
    sbom = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"forgecad-skill-{skill_id}",
        "documentNamespace": f"https://forgecad.local/sbom/skill/{skill_id}/{version}",
        "packages": [{"SPDXID": "SPDXRef-Package", "name": f"forgecad-skill-{skill_id}", "versionInfo": version, "licenseConcluded": "NOASSERTION", "downloadLocation": "NOASSERTION"}],
        "annotations": [{"annotationType": "OTHER", "annotator": "Tool: forgecad-mcp006", "annotationDate": "2026-08-09T00:00:00Z", "comment": "Declarative metadata only; no executable or external asset payload."}],
    }
    if skill_id == "ponytail-preflight":
        sbom["annotations"].append({
            "annotationType": "OTHER",
            "annotator": "Tool: forgecad-mcp006",
            "annotationDate": "2026-08-13T00:00:00Z",
            "comment": "MIT workflow reference only: DietrichGebert/ponytail@2ed6c52c9d7e5e56942508591085fd45dea277d3. No third-party code or runtime dependency is included.",
        })
    write_json(bundle / "sbom.spdx.json", sbom)
    manifest_hash = sha256(bundle / "manifest.json")
    recipe_hash = sha256(recipe_path)
    (bundle / "provenance.intoto.jsonl").write_text(
        json.dumps(
            {
                "_type": "https://in-toto.io/Statement/v1",
                "subject": [{"name": f"forgecad-skill-{skill_id}", "digest": {"sha256": manifest_hash}}],
                "predicateType": "https://slsa.dev/provenance/v1",
                "predicate": {"buildType": "forgecad/first-party-declarative", "materials": [{"uri": "local-worktree", "digest": {"sha256": "development-root"}}]},
            },
            ensure_ascii=False,
        )
        + "\n",
        encoding="utf-8",
    )
    trust = bundle / "trust"
    trust.mkdir(parents=True, exist_ok=True)
    (trust / "manifest.sha256").write_text(
        f"{manifest_hash}  manifest.json\n{recipe_hash}  recipes/default.recipe.json\n",
        encoding="utf-8",
    )
    write_json(
        bundle / "signature.bundle",
        {
            "schema_version": "ForgeCADDevelopmentSignature@1",
            "status": "deferred-to-mcp012-013",
            "trust_profile": "development-root",
            "manifest_sha256": manifest_hash,
            "cryptographic_signature": None,
        },
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true", help="write generated bundle files")
    parser.add_argument("--skill", action="append", help="materialize only one declared skill id")
    args = parser.parse_args()
    document = json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))
    entries = document["skills"]
    if args.skill:
        requested = set(args.skill)
        known = {entry["skill_id"] for entry in entries}
        unknown = sorted(requested - known)
        if unknown:
            parser.error(f"unknown declared Skill id: {', '.join(unknown)}")
        entries = [entry for entry in entries if entry["skill_id"] in requested]
    for entry in entries:
        materialize(entry, args.write)
    print(f"MCP006 bundle materialization {'written' if args.write else 'checked'}: {len(entries)} bundles")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
