#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"
F_GATE_TARGET="$(mktemp -d "${TMPDIR:-/tmp}/forgecad-mcp010f-tauri.XXXXXX")"
trap 'rm -rf "$F_GATE_TARGET"' EXIT

python3 scripts/check_mcp010f_stage0_truth.py
python3 scripts/check_forgecad_contracts.py
python3 scripts/check_mcp010f_viewer.py
python3 - <<'PY'
from pathlib import Path

source = Path("scripts/probe_mcp010f_part_correction.py").read_text(encoding="utf-8")
preflight = source.index("preflight = read_ponytail_preflight(client)")
project_create = source.index('project = client.tool("project_create"')
assert preflight < project_create
assert '"skill_id": "ponytail-preflight"' in source
assert '"version": "0.1.0"' in source
assert '"shoulder-armor-right": "shoulder-armor-right"' in source
assert '"shin-pair": "shin-left"' in source
assert '--target-mode' in source
assert 'def part_parameter_prefix' in source
assert 'primary_form_repair_prepare' in source
assert 'scene_observe_get' in source
assert 'silhouette_rig_hash' in source
assert 'runtime_search_owner": "forgecad-runtime"' in source
assert 'for fraction in (0.4, 0.7, 1.0)' not in source
assert 'apply_part_adjustment' not in source
assert 'silhouette_candidate_compare' not in source
assert 'part_contour_fit_prepare' not in source
print("MCP010F part-correction probe uses one Runtime-owned Primary Form repair and consolidated observation")
PY
python3 - <<'PY'
from pathlib import Path

source = Path("scripts/probe_mcp010c_codex_cli.py").read_text(encoding="utf-8")
for parameter_id in ("upper-arm-height", "forearm-height", "thigh-height", "shin-height", "elbow-offset-y", "knee-offset-y"):
    assert f'"parameter_id": "{parameter_id}"' in source
assert '"max_evaluations": 64' in source
assert '"--part-contour-sequence"' in source
assert 'def parse_bound_silhouette_turn' in source
assert 'def run_primary_form_repair_step' in source
assert 'primary_form_repair_steps' in source
assert 'def build_primary_form_composition_lineage' in source
assert 'primary_form_composition_lineage' in source
assert 'lineage did not authorize candidate advance' in source
assert 'PRIMARY_FORM_COMPOSITION_INVALID: lineage was not consumed' in source
assert 'silhouette observation before composition step' in source
print("MCP010F source route includes bounded Primary Form controls and candidate-bound composition sequence")
PY
python3 - <<'PY'
import sys

sys.path.insert(0, "scripts")
from probe_mcp010c_codex_cli import build_primary_form_composition_lineage, canonical_hash

def hash_value(fill):
    return fill * 64

steps = [
    {
        "step": 1,
        "part_id": "chest-shell",
        "source_candidate_id": "candidate-0",
        "observation_candidate_id": "candidate-0",
        "observation_sha256": hash_value("a"),
        "target_sha256": hash_value("t"),
        "camera_hash": hash_value("b"),
        "camera_canonical_sha256": hash_value("c"),
        "rig_sha256": hash_value("d"),
        "intent_sha256": hash_value("e"),
        "fit_camera_hash": hash_value("f"),
        "status": "prepared",
        "acceptance": {"status": "accepted", "strict_improvement": True},
        "prepared_candidate_id": "candidate-1",
    },
    {
        "step": 2,
        "part_id": "hip-pair",
        "source_candidate_id": "candidate-1",
        "observation_candidate_id": "candidate-1",
        "observation_sha256": hash_value("g"),
        "target_sha256": hash_value("t"),
        "camera_hash": hash_value("h"),
        "camera_canonical_sha256": hash_value("i"),
        "rig_sha256": hash_value("j"),
        "intent_sha256": hash_value("k"),
        "fit_camera_hash": hash_value("l"),
        "status": "no_improvement",
        "acceptance": {"status": "retained_source", "strict_improvement": False},
        "prepared_candidate_id": None,
    },
]
lineage = build_primary_form_composition_lineage(
    "project-0", "candidate-0", "candidate-1", hash_value("t"), ("chest-shell", "hip-pair"), steps
)
assert lineage["schema_version"] == "ForgeCADPrimaryFormCompositionLineage@1"
assert lineage["accepted_step_count"] == 1
assert lineage["final_candidate_id"] == "candidate-1"
canonical_input = dict(lineage)
canonical_input["canonical_sha256"] = ""
assert lineage["canonical_sha256"] == canonical_hash(canonical_input)
broken = [dict(step) for step in steps]
broken[1] = dict(broken[1], source_candidate_id="candidate-0")
try:
    build_primary_form_composition_lineage(
        "project-0", "candidate-0", "candidate-1", hash_value("t"), ("chest-shell", "hip-pair"), broken
    )
except RuntimeError as error:
    assert "source candidate chain drifted" in str(error)
else:
    raise AssertionError("stale composition source unexpectedly passed")
print("MCP010F composition lineage binds observation, target, Runtime search and candidate transitions")
PY

INVENTORY_ROOT="$F_GATE_TARGET/reference-inventory"
mkdir -p "$INVENTORY_ROOT"
python3 scripts/validate_mcp010f_reference_inventory.py \
  --inventory docs/evidence/mcp010f/reference-detail-inventory-real-reference.json \
  --operator-catalog docs/evidence/mcp010d/raw-stdio.json \
  --assetpack-manifest packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/manifest.json \
  --output "$INVENTORY_ROOT/validation.json" >/dev/null
python3 - "$INVENTORY_ROOT/invalid.json" <<'PY'
import json
import pathlib
import sys

source = json.loads(pathlib.Path("docs/evidence/mcp010f/reference-detail-inventory-real-reference.json").read_text())
source["detail_inventory"][0]["operator_ids"].append("boolean@1")
pathlib.Path(sys.argv[1]).write_text(json.dumps(source) + "\n", encoding="utf-8")
PY
if python3 scripts/validate_mcp010f_reference_inventory.py \
  --inventory "$INVENTORY_ROOT/invalid.json" \
  --operator-catalog docs/evidence/mcp010d/raw-stdio.json \
  --assetpack-manifest packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/manifest.json >/dev/null 2>&1; then
  echo "reference inventory inactive-operator negative test unexpectedly passed" >&2
  exit 1
fi

LINEFLOW_ROOT="$F_GATE_TARGET/lineflow"
mkdir -p "$LINEFLOW_ROOT"
python3 scripts/build_mcp010f_surface_lineflow_plan.py \
  --inventory docs/evidence/mcp010f/reference-detail-inventory-real-reference.json \
  --operator-catalog docs/evidence/mcp010f/lineflow-fit-input-20260812/operator-catalog.json \
  --assetpack-manifest packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/manifest.json \
  --validation "$INVENTORY_ROOT/validation.json" \
  --output "$LINEFLOW_ROOT/plan.json" >/dev/null
python3 - "$LINEFLOW_ROOT/plan.json" <<'PY'
import json
import sys

plan = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert plan["schema_version"] == "ForgeCADSurfaceLineFlowPlan@1"
assert plan["status"] == "READY_FOR_SINGLE_PART_FLOW_REVIEW"
assert plan["current_gate"]["stage"] == "silhouette-blockout"
assert plan["current_gate"]["surface_material_unlocked"] is False
assert len(plan["actions"]) == 5
assert all(action["stage"] == "silhouette-blockout" for action in plan["actions"])
assert all(action["change_policy"].startswith("one_semantic_part") for action in plan["actions"])
assert any(item["detail_id"] == "lower-leg-and-feet" for item in plan["deferred_unknown_details"])
assert plan["runtime_write"] is False
assert plan["persistent_user_data_touched"] is False
assert len(plan["plan_sha256"]) == 64
PY
python3 - "$LINEFLOW_ROOT/inactive-catalog.json" <<'PY'
import json
import pathlib
import sys

catalog = json.loads(pathlib.Path("docs/evidence/mcp010f/lineflow-fit-input-20260812/operator-catalog.json").read_text())
for entry in catalog["operators"]:
    if entry.get("operator_id") == "forgecad.geometry.panel@1":
        entry["status"] = "unavailable"
        break
pathlib.Path(sys.argv[1]).write_text(json.dumps(catalog) + "\n", encoding="utf-8")
PY
if python3 scripts/build_mcp010f_surface_lineflow_plan.py \
  --inventory docs/evidence/mcp010f/reference-detail-inventory-real-reference.json \
  --operator-catalog "$LINEFLOW_ROOT/inactive-catalog.json" \
  --assetpack-manifest packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/manifest.json \
  --validation "$INVENTORY_ROOT/validation.json" \
  --output "$LINEFLOW_ROOT/invalid-plan.json" >/dev/null 2>&1; then
  echo "surface line-flow inactive-operator negative test unexpectedly passed" >&2
  exit 1
fi

CONTOUR_ROOT="$F_GATE_TARGET/contour-draft"
mkdir -p "$CONTOUR_ROOT"
python3 - "$CONTOUR_ROOT" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
receipt = {
    "project_id": "project-contour-fixture",
    "candidate_id": "candidate-contour-fixture",
    "reference_id": "reference-contour-fixture",
    "reference_sha256": "a" * 64,
    "artifact_sha256": "b" * 64,
    "render_set_hash": "c" * 64,
    "comparison_report_hash": "d" * 64,
    "part_ids": ["chest-shell", "visor"],
    "material_zone_ids": ["zone-white-shell", "zone-black-anodized"],
}
draft = {
    "schema_version": "ForgeCADViewerContourDraft@2",
    "coordinate_space": "normalized_reference_image",
    "points": [{"x": 0.25, "y": 0.18}, {"x": 0.75, "y": 0.18}, {"x": 0.82, "y": 0.82}, {"x": 0.18, "y": 0.82}],
    "closed": True,
    "transient_only": True,
    "runtime_write": False,
    "project_id": "project-contour-fixture",
    "candidate_id": receipt["candidate_id"],
    "reference_id": receipt["reference_id"],
    "artifact_sha256": receipt["artifact_sha256"],
    "render_set_hash": receipt["render_set_hash"],
    "comparison_report_hash": receipt["comparison_report_hash"],
    "source_pass": "silhouette",
    "selected_part_id": "chest-shell",
    "selected_material_zone_id": "zone-white-shell",
}
(root / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
(root / "draft.json").write_text(json.dumps(draft, indent=2) + "\n", encoding="utf-8")
bad_hash = dict(draft)
bad_hash["comparison_report_hash"] = "e" * 64
(root / "bad-hash.json").write_text(json.dumps(bad_hash, indent=2) + "\n", encoding="utf-8")
self_crossing = dict(draft)
self_crossing["points"] = [{"x": 0.2, "y": 0.2}, {"x": 0.8, "y": 0.8}, {"x": 0.2, "y": 0.8}, {"x": 0.8, "y": 0.2}]
(root / "self-crossing.json").write_text(json.dumps(self_crossing, indent=2) + "\n", encoding="utf-8")
all_parts = dict(draft)
all_parts["selected_part_id"] = None
all_parts["selected_material_zone_id"] = None
(root / "all-parts.json").write_text(json.dumps(all_parts, indent=2) + "\n", encoding="utf-8")
PY
python3 scripts/validate_mcp010f_contour_draft.py \
  --draft "$CONTOUR_ROOT/draft.json" \
  --receipt "$CONTOUR_ROOT/receipt.json" \
  --output "$CONTOUR_ROOT/intent.json" >/dev/null
python3 - "$CONTOUR_ROOT/intent.json" <<'PY'
import json
import sys

intent = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert intent["schema_version"] == "ForgeCADContourCorrectionIntent@1"
assert intent["status"] == "READY_FOR_SINGLE_PART_CONTOUR_EDIT"
assert intent["draft"]["point_count"] == 4
assert intent["draft"]["selected_part_id"] == "chest-shell"
assert intent["edit_policy"]["locked_until_pass"][-1] == "export_confirm"
assert intent["runtime_write"] is False
assert intent["persistent_user_data_touched"] is False
assert len(intent["intent_sha256"]) == 64
PY
if python3 scripts/validate_mcp010f_contour_draft.py \
  --draft "$CONTOUR_ROOT/bad-hash.json" \
  --receipt "$CONTOUR_ROOT/receipt.json" \
  --output "$CONTOUR_ROOT/bad-hash-intent.json" >/dev/null 2>&1; then
  echo "contour draft hash-binding negative test unexpectedly passed" >&2
  exit 1
fi
if python3 scripts/validate_mcp010f_contour_draft.py \
  --draft "$CONTOUR_ROOT/self-crossing.json" \
  --receipt "$CONTOUR_ROOT/receipt.json" \
  --output "$CONTOUR_ROOT/self-crossing-intent.json" >/dev/null 2>&1; then
  echo "contour draft self-intersection negative test unexpectedly passed" >&2
  exit 1
fi
python3 scripts/validate_mcp010f_contour_draft.py \
  --draft "$CONTOUR_ROOT/all-parts.json" \
  --receipt "$CONTOUR_ROOT/receipt.json" \
  --output "$CONTOUR_ROOT/all-parts-intent.json" >/dev/null
python3 - "$CONTOUR_ROOT/all-parts-intent.json" <<'PY'
import json
import sys

intent = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert intent["status"] == "CONTOUR_DRAFT_BOUND_PART_SELECTION_REQUIRED"
assert intent["runtime_write"] is False
PY

SHEET_ROOT="$F_GATE_TARGET/review-sheet"
mkdir -p "$SHEET_ROOT/render"
python3 - "$SHEET_ROOT" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
png = bytes.fromhex(
    "89504e470d0a1a0a0000000d4948445200000001000000010804000000b51c0c02"
    "0000000b4944415478da6364f80f00010501012718e3660000000049454e44ae426082"
)
(root / "reference.png").write_bytes(png)
for name in ("beauty", "silhouette", "material-id"):
    (root / "render" / f"{name}.png").write_bytes(png)
PY
python3 scripts/make_mcp010f_comparison_sheet.py \
  --reference "$SHEET_ROOT/reference.png" \
  --render-dir "$SHEET_ROOT/render" \
  --output "$SHEET_ROOT/review-sheet.png" \
  --manifest "$SHEET_ROOT/review-sheet.json" >/dev/null
python3 - "$SHEET_ROOT/review-sheet.json" <<'PY'
import json
import sys

receipt = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert receipt["schema_version"] == "ForgeCADMCP010FComparisonSheet@1"
assert receipt["status"] == "PASS"
assert receipt["panel_order"] == ["reference", "beauty", "silhouette", "material-id"]
assert receipt["sheet_dimensions"] == {"width": 1024, "height": 1080}
assert receipt["persistent_user_data_touched"] is False
PY

FIT_ROOT="$F_GATE_TARGET/fit-plan"
mkdir -p "$FIT_ROOT"
python3 - "$FIT_ROOT" <<'PY'
import hashlib
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])

def canon(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()

comparison = {
    "schema_version": "ReferenceComparisonReport@1",
    "report_id": "comparison-fixture",
    "candidate_id": "candidate-fixture",
    "artifact_sha256": "a" * 64,
    "reference_id": "reference-fixture",
    "reference_sha256": "b" * 64,
    "render_set_hash": "c" * 64,
    "camera_hash": "d" * 64,
    "mask": {"method": "local-border-flood-fill-morphology", "revision": "mask-2", "sha256": "e" * 64, "width": 512, "height": 512},
    "metrics": {
        "silhouette_iou": 0.60,
        "boundary_f1_4px": 0.30,
        "bbox_edge_error": 0.08,
        "centroid_error": 0.02,
        "landmark_coverage": 0.50,
        "landmark_nme": 0.20,
        "region_median_iou": 0.25,
        "critical_region_min_iou": 0.10,
    },
    "status": "QUALITY_TARGET_NOT_MET",
    "canonical_sha256": "",
}
comparison["canonical_sha256"] = hashlib.sha256(canon(comparison)).hexdigest()

view = {
    "schema_version": "ReferenceViewSpec@1",
    "reference_id": "reference-fixture",
    "reference_sha256": "b" * 64,
    "view_id": "three-quarter-fixture",
    "source_view": "three-quarter",
    "image": {"width": 1254, "height": 1254, "rotation_degrees": 0, "crop": {"x": 0, "y": 0, "width": 1, "height": 1}},
    "landmarks": [
        {"landmark_id": "crown", "x": 0.5, "y": 0.1, "visibility": "observed", "confidence": 0.95},
        {"landmark_id": "rear-unknown", "x": 0.5, "y": 0.9, "visibility": "unknown", "confidence": 0.1},
    ],
    "regions": [
        {"region_id": "head-visor", "x": 0.35, "y": 0.02, "width": 0.28, "height": 0.28, "visibility": "observed", "confidence": 0.92},
        {"region_id": "chest-armor", "x": 0.3, "y": 0.3, "width": 0.4, "height": 0.28, "visibility": "observed", "confidence": 0.90},
        {"region_id": "rear-shell", "x": 0.6, "y": 0.3, "width": 0.2, "height": 0.3, "visibility": "inferred", "confidence": 0.2},
    ],
    "canonical_sha256": "",
}
view["canonical_sha256"] = hashlib.sha256(canon(view)).hexdigest()

catalog = {
    "schema_version": "OperatorCatalog@1",
    "catalog_id": "catalog-fixture",
    "geometry_program_schema_version": "GeometryProgram@2",
    "operators": [
        {"operator_id": "forgecad.geometry.panel@1", "status": "active"},
        {"operator_id": "forgecad.geometry.profile-extrude@1", "status": "active"},
        {"operator_id": "forgecad.geometry.joint-stack@1", "status": "active"},
    ],
    "canonical_sha256": "f" * 64,
}
for name, value in (("comparison.json", comparison), ("view.json", view), ("catalog.json", catalog)):
    (root / name).write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
PY
python3 scripts/build_mcp010f_fit_plan.py \
  --comparison "$FIT_ROOT/comparison.json" \
  --view-spec "$FIT_ROOT/view.json" \
  --operator-catalog "$FIT_ROOT/catalog.json" \
  --output "$FIT_ROOT/plan.json"
python3 - "$FIT_ROOT/plan.json" <<'PY'
import json
import sys

plan = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert plan["schema_version"] == "ForgeCADReferenceFitPlan@1"
assert plan["decision"] == "revise"
assert [item["stage"] for item in plan["actions"]] == ["silhouette"]
assert plan["actions"][0]["operator_hints"]
assert plan["actions"][0]["primary_part_ids"] == ["chest-shell"]
assert "chest-vent" in plan["actions"][0]["supporting_part_ids"]
assert plan["actions"][0]["part_operator_hints"]["chest-shell"]
assert plan["actions"][0]["material_zone_hints"]
assert plan["workflow"]["current_stage"] == "silhouette"
assert plan["workflow"]["gates"]["silhouette"]["passed"] is False
assert plan["workflow"]["gates"]["structure"]["passed"] is False
assert plan["workflow"]["gates"]["surface_material_unlocked"] is False
assert plan["workflow"]["canvas"]["transient_only"] is True
assert plan["persistent_user_data_touched"] is False
assert len(plan["canonical_sha256"]) == 64
assert plan["blocked_reasons"]
PY
cat > "$FIT_ROOT/review.json" <<'JSON'
{
  "schema_version": "VisualReviewReport@1",
  "review_id": "review-fixture",
  "candidate_id": "candidate-fixture",
  "reference_id": "reference-fixture",
  "render_set_hash": "c000000000000000000000000000000000000000000000000000000000000000",
  "comparison_report_hash": "d000000000000000000000000000000000000000000000000000000000000000",
  "round": 1,
  "stage": "silhouette",
  "issues": [{
    "issue_id": "material-fixture",
    "pass": "material-id",
    "region_id": "chest-armor",
    "claim": "Material zone is only a later review issue.",
    "confidence": 0.9,
    "visibility": "observed",
    "action": "Defer until cumulative silhouette, structure and form gates pass."
  }],
  "status": "needs_revision",
  "canonical_sha256": "e000000000000000000000000000000000000000000000000000000000000000"
}
JSON
python3 scripts/build_mcp010f_fit_plan.py \
  --comparison "$FIT_ROOT/comparison.json" \
  --view-spec "$FIT_ROOT/view.json" \
  --review "$FIT_ROOT/review.json" \
  --operator-catalog "$FIT_ROOT/catalog.json" \
  --output "$FIT_ROOT/review-locked-plan.json"
python3 - "$FIT_ROOT/review-locked-plan.json" <<'PY'
import json
import sys

plan = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert [item["stage"] for item in plan["actions"]] == ["silhouette"]
assert plan["workflow"]["gates"]["surface_material_unlocked"] is False
assert not any(item["stage"] == "material-surface" for item in plan["actions"])
PY
python3 - "$FIT_ROOT/comparison.json" "$FIT_ROOT/passing-comparison.json" <<'PY'
import hashlib
import json
import sys

def canon(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()

source = json.loads(open(sys.argv[1], encoding="utf-8").read())
source["status"] = "PARTIAL_VISIBLE_VIEW_PASS"
source["metrics"] = {
    "silhouette_iou": 0.90,
    "boundary_f1_4px": 0.90,
    "bbox_edge_error": 0.01,
    "centroid_error": 0.01,
    "landmark_coverage": 0.90,
    "landmark_nme": 0.01,
    "region_median_iou": 0.90,
    "critical_region_min_iou": 0.90,
}
source["canonical_sha256"] = ""
source["canonical_sha256"] = hashlib.sha256(canon(source)).hexdigest()
open(sys.argv[2], "w", encoding="utf-8").write(json.dumps(source, indent=2) + "\n")
PY
python3 scripts/build_mcp010f_fit_plan.py \
  --comparison "$FIT_ROOT/passing-comparison.json" \
  --view-spec "$FIT_ROOT/view.json" \
  --review "$FIT_ROOT/review.json" \
  --operator-catalog "$FIT_ROOT/catalog.json" \
  --output "$FIT_ROOT/material-unlocked-plan.json"
python3 - "$FIT_ROOT/material-unlocked-plan.json" <<'PY'
import json
import sys

plan = json.loads(open(sys.argv[1], encoding="utf-8").read())
assert [item["stage"] for item in plan["actions"]] == ["material-surface"]
assert plan["workflow"]["current_stage"] == "material-surface"
assert plan["workflow"]["gates"]["silhouette"]["passed"] is True
assert plan["workflow"]["gates"]["structure"]["passed"] is True
assert plan["workflow"]["gates"]["form"]["passed"] is True
assert plan["workflow"]["gates"]["surface_material_unlocked"] is True
PY
python3 - "$FIT_ROOT/comparison.json" <<'PY'
import json
import sys

path = sys.argv[1]
value = json.loads(open(path, encoding="utf-8").read())
value["canonical_sha256"] = "0" * 64
open(path, "w", encoding="utf-8").write(json.dumps(value) + "\n")
PY
if python3 scripts/build_mcp010f_fit_plan.py \
  --comparison "$FIT_ROOT/comparison.json" \
  --view-spec "$FIT_ROOT/view.json" >/dev/null 2>&1; then
  echo "fit-plan canonical hash negative test unexpectedly passed" >&2
  exit 1
fi

npm --workspace apps/desktop run typecheck
npm --workspace apps/desktop run build
env -u FORGECAD_BUILD_COHORT_SHA256 CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-mcp --bin forgecad-mcp --offline
TOOL_MANIFEST_ACTUAL="$F_GATE_TARGET/source-tool-manifest-summary.actual.json"
"$F_GATE_TARGET/debug/forgecad-mcp" --tool-manifest-summary > "$TOOL_MANIFEST_ACTUAL"
python3 - "$TOOL_MANIFEST_ACTUAL" docs/evidence/mcp010f/source-tool-manifest-summary.json <<'PY'
import json
import pathlib
import sys


def require_equal(actual, expected, path="$"):
    if type(actual) is not type(expected):
        raise SystemExit(
            f"tool manifest summary type mismatch at {path}: "
            f"actual={type(actual).__name__} expected={type(expected).__name__}"
        )
    if isinstance(expected, dict):
        actual_keys = set(actual)
        expected_keys = set(expected)
        if actual_keys != expected_keys:
            raise SystemExit(
                f"tool manifest summary key mismatch at {path}: "
                f"missing={sorted(expected_keys - actual_keys)} extra={sorted(actual_keys - expected_keys)}"
            )
        for key in sorted(expected):
            require_equal(actual[key], expected[key], f"{path}.{key}")
        return
    if isinstance(expected, list):
        if len(actual) != len(expected):
            raise SystemExit(
                f"tool manifest summary list length mismatch at {path}: "
                f"actual={len(actual)} expected={len(expected)}"
            )
        for index, (actual_item, expected_item) in enumerate(zip(actual, expected)):
            require_equal(actual_item, expected_item, f"{path}[{index}]")
        return
    if actual != expected:
        raise SystemExit(
            f"tool manifest summary value mismatch at {path}: "
            f"actual={actual!r} expected={expected!r}"
        )


actual = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
expected = json.loads(pathlib.Path(sys.argv[2]).read_text(encoding="utf-8"))
require_equal(actual, expected)
PY
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --workspace --offline
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-mcp tool_manifest_summary_is_derived_from_the_actual_enabled_manifests --offline
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-runtime --features test-render-worker-fallback visible_view_gate_rejects_exploratory_thresholds_and_accepts_strict_metrics --offline
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-runtime --features test-render-worker-fallback silhouette_target_is_hash_bound_and_refinement_is_immutable --offline
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-runtime --features test-render-worker-fallback automatic_silhouette_target_round_trips_float_contour_hash --offline
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-runtime --features test-render-worker-fallback camera_fit_returns_bounded_hash_bound_candidates_without_mutating_candidate --offline
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-runtime --features test-render-worker-fallback camera_fit_search_covers_global_scale_with_deterministic_budget --offline
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-runtime --features test-render-worker-fallback contour_fit_part_proposal_and_candidate_compare_are_bounded_and_read_only --offline
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-runtime --features test-render-worker-fallback bounded_agentic_action_run_executes_primary_form_and_round_trips_immutably --offline
CARGO_TARGET_DIR="$F_GATE_TARGET" script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-runtime --features test-render-worker-fallback --offline silhouette_part_error
git diff --check

python3 - docs/evidence/mcp010f/current-benchmark-truth.json <<'PY'
import json
import pathlib
import sys

truth = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
observation = truth["provisional_retained_observation"]
assert observation["current_candidate_visible_view_gate"] == "FAIL_QUALITY_TARGET_NOT_MET"
assert observation["benchmark_eligibility"] == "BLOCKED_INCOMPLETE_BINDING"
assert observation["camera_binding"]["binding_status"] == "MISMATCH"
assert truth["evidence_status"] == "INCOMPLETE_TRUTH_BINDING"
assert truth["assertion_ledger"]["BT009_AOV_HASH_COMPLETENESS"] == "MISSING"
assert truth["packaged_viewer"]["provisional_observation_binding"] == "PASS_CURRENT_COHORT_BOUND_READ_MODEL"
assert truth["assertion_ledger"]["BT016_SURFACE_RAW_PAIR"] == "FAIL"

print(json.dumps({
    "schema_version": "ForgeCADMCP010FRawSourceGate@1",
    "task_id": "FGC-MCP010F",
    "status": "PASS_SOURCE_GATE_WITH_FAILED_VISUAL_QUALITY_AND_INCOMPLETE_BINDING",
    "source_tool_manifest_summary": "PASS_EXACT_FIELD_EQUALITY",
    "provisional_observation_visual_quality": "FAIL_QUALITY_TARGET_NOT_MET",
    "benchmark_eligibility": "BLOCKED_INCOMPLETE_BINDING",
    "camera_binding": "MISMATCH",
    "benchmark_truth": "INCOMPLETE_TRUTH_BINDING",
    "aov_hash_completeness": "MISSING",
    "packaged_viewer_provisional_observation_binding": "PASS_CURRENT_COHORT_BOUND_READ_MODEL",
    "surface_curated_raw_hash_binding": "FAIL",
    "viewer_source": "PASS",
    "contour_target_runtime": "PASS_HASH_BOUND_AUTOMATIC_AND_USER_REFINED",
    "camera_fit_runtime": "PASS_BOUNDED_TYPED_CAMERA_SEARCH",
    "silhouette_fit_runtime": "PASS_BOUNDED_RIG_CAMERA_AND_GEOMETRY_VARIANT_SEARCH",
    "part_contour_fit_runtime": "PASS_SINGLE_PART_READ_ONLY_PROPOSAL",
    "silhouette_part_error_runtime": "PASS_HASH_BOUND_MULTI_PART_ERROR_TABLE",
    "silhouette_candidate_compare_runtime": "PASS_HASH_BOUND_TWO_TO_EIGHT_COMPARE",
    "reference_inventory": "PASS_ACTIVE_OPERATOR_AND_ASSETPACK",
    "packaged_viewer_read_model": "PASS_STRUCTURAL_SEPARATE_PROBE",
    "packaged_viewer_window": "PASS_STRUCTURAL_SEPARATE_PROBE",
    "packaged_viewer_ui_e2e": "NOT_RUN",
    "human_visual_review": "NOT_RUN",
    "full_360_reference": "BLOCKED_REFERENCE_COVERAGE",
    "persistent_user_data_touched": False,
}, ensure_ascii=False, sort_keys=True))
PY
