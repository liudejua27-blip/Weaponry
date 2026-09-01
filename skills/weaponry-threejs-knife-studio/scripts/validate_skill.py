#!/usr/bin/env python3
"""Focused static validation for the Three.js knife studio Skill.

This validator does not build an asset, render a scene, or claim visual quality.
"""

from __future__ import annotations

import json
import hashlib
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


skill = (ROOT / "SKILL.md").read_text(encoding="utf-8")
require(skill.startswith("---\n"), "SKILL.md frontmatter missing")
require("name: weaponry-threejs-knife-studio" in skill, "Skill name drifted")
for marker in (
    "KnifeSceneProgram@1",
    "KnifeObjectiveLedger@1",
    "Runtime-owned program",
    "Closed method: local knowledge, discrete grammar, objective, search",
    "NOT_COMPUTABLE",
    "quantitative comparison inputs",
    "two plateau revisions",
    "METRICALLY_SUPERIOR_TO_PINNED_BASELINE",
    "COMMERCIAL_ACCEPTED",
):
    require(marker in skill, f"missing Skill invariant: {marker}")

ui = (ROOT / "agents" / "openai.yaml").read_text(encoding="utf-8")
require("$weaponry-threejs-knife-studio" in ui, "default prompt does not name the Skill")
require("allow_implicit_invocation: false" in ui, "route must remain explicit")

schemas = {}
for name in ("knife-scene-program.schema.json", "knife-objective-ledger.schema.json", "knife-knowledge.schema.json"):
    schema = json.loads((ROOT / "references" / name).read_text(encoding="utf-8"))
    schemas[name] = schema
    require(schema.get("additionalProperties") is False, f"{name} root must be closed")
    require(isinstance(schema.get("required"), list), f"{name} required fields missing")

program_schema = schemas["knife-scene-program.schema.json"]
require(program_schema["properties"]["assembly"]["$ref"] == "#/$defs/assembly", "assembly schema missing")
require(
    set(program_schema["$defs"]["assembly"]["properties"])
    == {"guard", "grip", "pommel", "fasteners", "gems", "reliefs"},
    "assembly vocabulary drifted",
)

knowledge_schema = schemas["knife-knowledge.schema.json"]
require("silhouette_grammar" in knowledge_schema["required"], "KnifeKnowledge schema must bind silhouette grammar")
require("formula" in knowledge_schema["$defs"] and knowledge_schema["$defs"]["formula"]["additionalProperties"] is False, "knowledge formulas must remain closed")
require("silhouetteGrammar" in knowledge_schema["$defs"], "knowledge schema must expose discrete grammar priors")
claim_schema = knowledge_schema["$defs"]["claim"]
require(len(claim_schema.get("allOf", [])) == 3, "claim evidence conditionals must remain explicit")
require(claim_schema["allOf"][0]["then"]["properties"]["source_refs"]["minItems"] == 1, "observed/inferred claims need source references")
require(claim_schema["allOf"][1]["then"]["properties"]["supporting_claims"]["minItems"] == 1, "inferred claims need supporting observations")
require(claim_schema["allOf"][2]["then"]["properties"]["permitted_use"]["const"] == "preserve-gap", "unknown claims must preserve the gap")
ledger_schema = schemas["knife-objective-ledger.schema.json"]
require("objective_metrics" in ledger_schema["required"] and "regression_limits" in ledger_schema["required"], "objective ledger must bind metric terms")
require(ledger_schema["properties"]["candidate_budget"]["maximum"] == 32, "objective candidate budget must remain bounded")
catalog_source = (ROOT.parents[1] / "packages" / "weaponry-threejs" / "src" / "knife-objective-metric-catalog.ts").read_text(encoding="utf-8")
catalog_metric_ids = re.findall(r"^\s*metric\('([^']+)'", catalog_source, flags=re.MULTILINE)
require(
    ledger_schema["$defs"]["metric"]["enum"] == catalog_metric_ids,
    "objective metric schema drifted from the append-only TypeScript catalog",
)
require(len(catalog_metric_ids) == len(set(catalog_metric_ids)), "objective metric catalog contains duplicate IDs")
require(
    catalog_metric_ids[:12] == [
        "silhouette-iou", "boundary-f1", "symmetric-chamfer", "p95-contour-distance",
        "tip-landmark-error", "belly-depth-error", "thickness-continuity", "normal-continuity",
        "part-id-coverage", "material-id-coverage", "negative-space-error", "fps-occupancy",
    ],
    "legacy objective metric order or meaning drifted",
)


def require_closed_branch(name: str, properties: set[str], required: set[str]) -> None:
    branch = program_schema["$defs"].get(name)
    require(isinstance(branch, dict), f"missing assembly branch: {name}")
    require(branch.get("type") == "object", f"{name} must be an object branch")
    require(branch.get("additionalProperties") is False, f"{name} must remain closed")
    require(set(branch.get("properties", {})) == properties, f"{name} properties drifted")
    require(set(branch.get("required", [])) == required, f"{name} required fields drifted")


assembly_defs = program_schema["$defs"]
for primitive, classic_name, semantic_name in (
    ("guard", "classicGuard", "dragonGuard"),
    ("grip", "classicGrip", "segmentedGrip"),
    ("pommel", "classicPommel", "hookedPommel"),
):
    union = assembly_defs[primitive]
    require(
        union.get("oneOf") == [
            {"$ref": f"#/$defs/{classic_name}"},
            {"$ref": f"#/$defs/{semantic_name}"},
        ],
        f"{primitive} must expose the exact classic|semantic union",
    )

require_closed_branch(
    "classicGuard",
    {"primitive", "part_id", "center", "span", "thickness", "depth", "style"},
    {"primitive", "part_id", "center", "span", "thickness", "depth"},
)
require_closed_branch(
    "dragonGuard",
    {"primitive", "part_id", "center", "span", "thickness", "depth", "style", "jaw_gap", "upper_jaw", "lower_jaw", "horns", "eye_sockets"},
    {"primitive", "part_id", "center", "span", "thickness", "depth", "style", "jaw_gap", "upper_jaw", "lower_jaw", "horns", "eye_sockets"},
)
require_closed_branch(
    "classicGrip",
    {"primitive", "part_id", "center", "length", "radius", "taper", "facets", "style"},
    {"primitive", "part_id", "center", "length", "radius", "taper", "facets"},
)
require_closed_branch(
    "segmentedGrip",
    {"primitive", "part_id", "center", "length", "radius", "taper", "facets", "style", "centerline", "segments", "metal_frames", "fasteners"},
    {"primitive", "part_id", "center", "length", "radius", "taper", "facets", "style", "centerline", "segments", "metal_frames", "fasteners"},
)
require_closed_branch(
    "classicPommel",
    {"primitive", "part_id", "center", "length", "radius", "depth", "style"},
    {"primitive", "part_id", "center", "length", "radius", "depth"},
)
require_closed_branch(
    "hookedPommel",
    {"primitive", "part_id", "center", "length", "radius", "depth", "style", "hook", "gem_seat"},
    {"primitive", "part_id", "center", "length", "radius", "depth", "style", "hook", "gem_seat"},
)
for branch_name, style in (
    ("classicGuard", "classic"),
    ("dragonGuard", "dragon-guard"),
    ("classicGrip", "classic"),
    ("segmentedGrip", "segmented-grip"),
    ("classicPommel", "classic"),
    ("hookedPommel", "hooked-pommel"),
):
    require(assembly_defs[branch_name]["properties"]["style"].get("const") == style, f"{branch_name} style discriminator drifted")

for name, properties, required in (
    ("dragonJaw", {"span", "thickness", "depth", "offset_y", "offset_z", "curvature"}, {"span", "thickness", "depth", "offset_y", "offset_z", "curvature"}),
    ("dragonHorn", {"feature_id", "side", "length", "radius", "sweep", "offset_z"}, {"feature_id", "side", "length", "radius", "sweep", "offset_z"}),
    ("dragonEyeSocket", {"feature_id", "side", "radius", "depth", "offset_y", "offset_z"}, {"feature_id", "side", "radius", "depth", "offset_y", "offset_z"}),
    ("gripSegment", {"feature_id", "start_u", "end_u", "radius_scale"}, {"feature_id", "start_u", "end_u", "radius_scale"}),
    ("gripFrame", {"feature_id", "at", "width", "thickness"}, {"feature_id", "at", "width", "thickness"}),
    ("gripFastenerFeature", {"feature_id", "at", "side", "radius", "depth"}, {"feature_id", "at", "side", "radius", "depth"}),
    ("pommelHook", {"length", "radius", "bend", "direction"}, {"length", "radius", "bend", "direction"}),
    ("pommelGemSeat", {"feature_id", "radius", "depth", "offset_x", "offset_y", "offset_z", "axis"}, {"feature_id", "radius", "depth", "offset_x", "offset_y", "offset_z", "axis"}),
):
    require_closed_branch(name, properties, required)

require(assembly_defs["dragonGuard"]["properties"]["horns"]["minItems"] == 2, "dragon guard needs at least two horns")
require(assembly_defs["dragonGuard"]["properties"]["horns"]["maxItems"] == 4, "dragon guard horn bound drifted")
require(assembly_defs["dragonGuard"]["properties"]["eye_sockets"]["minItems"] == 1, "dragon guard needs an eye socket")
require(assembly_defs["dragonGuard"]["properties"]["eye_sockets"]["maxItems"] == 2, "dragon guard eye socket bound drifted")
require(assembly_defs["segmentedGrip"]["properties"]["fasteners"]["minItems"] == 3, "segmented grip fastener lower bound drifted")
require(assembly_defs["segmentedGrip"]["properties"]["fasteners"]["maxItems"] == 5, "segmented grip fastener upper bound drifted")
for owner, field in (
    ("dragonGuard", "horns"),
    ("dragonGuard", "eye_sockets"),
    ("segmentedGrip", "segments"),
    ("segmentedGrip", "metal_frames"),
    ("segmentedGrip", "fasteners"),
):
    require(assembly_defs[owner]["properties"][field].get("uniqueItems") is True, f"{owner}.{field} must keep unique feature entries")

program = json.loads((ROOT / "references" / "dragonfang-first-slice.json").read_text(encoding="utf-8"))
require(program["schema_version"] == "KnifeSceneProgram@1", "template schema version drifted")
sections = program["blade_surface"]["sections"]
roles = [section["role"] for section in sections]
core_roles = [role for role in roles if role != "intermediate"]
require(core_roles == ["root", "shoulder", "belly", "tip"], "required core section roles drifted")
require(all(role in {"root", "shoulder", "belly", "tip", "intermediate"} for role in roles), "unsupported section role present")
require([section["u"] for section in sections] == sorted(section["u"] for section in sections), "sections are not monotonic")
require(program["blade_surface"]["spine_curve"]["curve_id"] != program["blade_surface"]["cutting_edge_curve"]["curve_id"], "spine and edge identities collapsed")
parts = {part["part_id"]: part for part in program["parts"]}
require(parts["blade-body"]["frozen"] is True and parts["cutting-edge"]["frozen"] is True, "accepted blade scope must remain frozen during assembly work")
for part_id in ("guard", "grip", "pommel"):
    require(parts[part_id]["frozen"] is False, f"{part_id} must be available to the assembly objective")
    require(program["assembly"][part_id]["part_id"] == part_id, f"{part_id} assembly binding drifted")
    require(program["assembly"][part_id]["primitive"] == part_id, f"{part_id} primitive role drifted")
require(program["assembly"]["guard"]["style"] == "dragon-guard", "Dragonfang guard language is not active")
require(program["assembly"]["grip"]["style"] == "segmented-grip", "Dragonfang grip language is not active")
require(program["assembly"]["pommel"]["style"] == "hooked-pommel", "Dragonfang pommel language is not active")
require(re.fullmatch(r"[a-zA-Z][a-zA-Z0-9_.-]{0,63}", program["asset_id"]) is not None, "asset ID invalid")
program_preimage = dict(program)
program_preimage["canonical_sha256"] = ""
program_sha256 = hashlib.sha256(json.dumps(program_preimage, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")).hexdigest()
require(program["canonical_sha256"] == program_sha256, "Dragonfang program canonical SHA drifted")

r5_ledger = json.loads((ROOT / "references" / "dragonfang-objective-ledger-r5.json").read_text(encoding="utf-8"))
r6_ledger = json.loads((ROOT / "references" / "dragonfang-objective-ledger-r6-intrinsic.json").read_text(encoding="utf-8"))
require(r6_ledger["revision"] == r5_ledger["revision"] + 1, "intrinsic ledger revision is not a direct successor")
require(r6_ledger["parent_ledger_sha256"] == r5_ledger["canonical_sha256"], "intrinsic ledger parent binding drifted")
require(r6_ledger["program_sha256"] == program["canonical_sha256"], "intrinsic ledger program binding drifted")
require(set(r6_ledger["objective_metrics"]) == {
    "assembly-ratio-prior-score", "assembly-attachment-continuity",
    "assembly-material-readability", "assembly-complexity-efficiency",
}, "intrinsic ledger metric scope drifted")
r6_preimage = dict(r6_ledger)
r6_preimage["canonical_sha256"] = ""
r6_sha256 = hashlib.sha256(json.dumps(r6_preimage, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")).hexdigest()
require(r6_ledger["canonical_sha256"] == r6_sha256, "intrinsic ledger canonical SHA drifted")

repo_root = ROOT.parents[1]
successor_path = ROOT / "references" / "dragonfang-procedural-successor-r7.json"
successor_bytes = successor_path.read_bytes()
successor = json.loads(successor_bytes)
successor_preimage = dict(successor)
successor_preimage["canonical_sha256"] = ""
successor_sha256 = hashlib.sha256(json.dumps(successor_preimage, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")).hexdigest()
require(successor["canonical_sha256"] == successor_sha256, "Dragonfang r7 successor canonical SHA drifted")
require([part["part_id"] for part in successor["parts"]] == [part["part_id"] for part in program["parts"]], "Dragonfang r7 Part cohort drifted")
require([zone["material_zone_id"] for zone in successor["material_zones"]] == [zone["material_zone_id"] for zone in program["material_zones"]], "Dragonfang r7 MaterialZone cohort drifted")

artifact_root = repo_root / "packages" / "weaponry-threejs" / "artifacts"
successor_glb = artifact_root / "dragonfang-kukri-procedural-r7.glb"
successor_receipt = json.loads((artifact_root / "dragonfang-kukri-procedural-r7.receipt.json").read_text(encoding="utf-8"))
successor_readiness = json.loads((artifact_root / "dragonfang-kukri-procedural-r7.readiness.json").read_text(encoding="utf-8"))
require(successor_receipt["schema_version"] == "WeaponryThreeJsProceduralSuccessorLineage@1", "Dragonfang r7 lineage schema drifted")
require(successor_receipt["baseline"]["program_bytes_sha256"] == hashlib.sha256((ROOT / "references" / "dragonfang-first-slice.json").read_bytes()).hexdigest(), "Dragonfang r7 baseline bytes drifted")
require(successor_receipt["baseline"]["program_sha256"] == program["canonical_sha256"], "Dragonfang r7 baseline semantic identity drifted")
require(successor_receipt["generation"]["objective_ledger_sha256"] == r6_ledger["canonical_sha256"], "Dragonfang r7 ledger binding drifted")
require(successor_receipt["generation"]["selected_mutation_scope"] == "grip-taper", "Dragonfang r7 deterministic selection drifted")
require(successor_receipt["successor"]["program_bytes_sha256"] == hashlib.sha256(successor_bytes).hexdigest(), "Dragonfang r7 program bytes drifted")
require(successor_receipt["successor"]["program_sha256"] == successor["canonical_sha256"], "Dragonfang r7 program semantic binding drifted")
require(successor_receipt["successor"]["glb_sha256"] == hashlib.sha256(successor_glb.read_bytes()).hexdigest(), "Dragonfang r7 GLB bytes drifted")
require(successor_receipt["successor"]["glb_bytes"] == successor_glb.stat().st_size, "Dragonfang r7 GLB size drifted")
require(successor_receipt["structural_delta"]["status"] == "MEASURED_NONZERO", "Dragonfang r7 structural delta disappeared")
require(successor_receipt["status"] == "SUCCESSOR_MATERIALIZED_REVIEW_ONLY", "Dragonfang r7 crossed the review-only boundary")
require(successor_readiness["status"] == "THREEJS_DESIGN_READY", "Dragonfang r7 Procedural Draft readiness is not closed")
require(all(gate["status"] == "PASS" for gate in successor_readiness["gates"].values()), "Dragonfang r7 readiness contains a non-passing gate")
require(successor_readiness["visual_status"] == "NOT_REQUESTED" and successor_readiness["commercial_status"] == "NOT_RUN", "Dragonfang r7 readiness crossed a quality boundary")
receipt_preimage = dict(successor_receipt)
receipt_preimage["canonical_sha256"] = ""
receipt_sha256 = hashlib.sha256(json.dumps(receipt_preimage, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")).hexdigest()
require(successor_receipt["canonical_sha256"] == receipt_sha256, "Dragonfang r7 lineage canonical SHA drifted")

knowledge = json.loads((ROOT / "references" / "crossfire-knife-knowledge.json").read_text(encoding="utf-8"))
require(knowledge["schema_version"] == "KnifeKnowledge@1", "knowledge schema version drifted")
require(knowledge["route"] == "weaponry-threejs-knife-studio", "knowledge route drifted")
require(knowledge["real_world_dimensions_permitted"] is False, "knowledge must forbid real-world dimensions")
require(len(knowledge["claims"]) >= 8, "knowledge claim space is too small")
require(len({claim["claim_id"] for claim in knowledge["claims"]}) == len(knowledge["claims"]), "knowledge claim IDs duplicated")
require("scripts/search_knife_knowledge.py" in skill, "Skill does not require local knowledge search")
knowledge_search = (ROOT / "scripts" / "search_knife_knowledge.py").read_text(encoding="utf-8")
for marker in ("without network access", "ranked-design-priors-only-not-observed-truth-or-quality-approval", "knowledge_sha256"):
    require(marker in knowledge_search, f"local knowledge search invariant missing: {marker}")

objective_doc = (ROOT / "references" / "objective-and-quality.md").read_text(encoding="utf-8")
require("Δ_i(c,b)" in objective_doc and "J(c|b)" in objective_doc, "objective document lacks the mathematical objective")
require("NOT_COMPUTABLE`, not zero" in objective_doc, "objective document must fail closed on missing evidence")
require("direction-aware Pareto" in objective_doc, "objective document lacks candidate search ordering")
search = (ROOT / "scripts" / "search_candidates.py").read_text(encoding="utf-8")
for marker in (
    "random.Random(seed)",
    "MAX_CANDIDATES = 32",
    "def pareto_front",
    '"visual_status": "NOT_RUN"',
    "Geometry-only successor proposal",
    "def _validate_classic_assembly_spec",
    "def _validate_dragon_guard",
    "def _validate_segmented_grip",
    "def _validate_hooked_pommel",
):
    require(marker in search, f"candidate search invariant missing: {marker}")

print("Weaponry Three.js knife studio Skill validation PASS: local knowledge, discrete grammar, append-only intrinsic metric catalog, quantitative objective, bounded candidate search, closed schemas, frozen accepted blade")
