#!/usr/bin/env python3
"""MCP002 contract smoke: every checked-in JSON contract must be valid and versioned."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_ROOT = ROOT / "packages" / "forgecad-contracts"
SCHEMA_ROOT = CONTRACT_ROOT / "schemas"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"ForgeCAD contract violation: {message}")


def load_schema(name: str) -> dict:
    return json.loads((SCHEMA_ROOT / name).read_text(encoding="utf-8"))


def require_required(schema: dict, expected: set[str], label: str) -> None:
    actual = set(schema.get("required", []))
    missing = sorted(expected - actual)
    require(not missing, f"{label} missing required fields: {missing}")


def check_mcp010b_contracts() -> None:
    """Keep V2 geometry truth contracts closed while V1 remains addressable."""
    expected_schemas = {
        "geometry-program-v2.schema.json",
        "geometry-program-hash-request.schema.json",
        "geometry-program-hash-result.schema.json",
        "operator-catalog.schema.json",
        "artifact-readback-v2.schema.json",
        "geometry-prepare-result-v2.schema.json",
        "geometry-quality-report-v2.schema.json",
        "geometry-candidate-evidence.schema.json",
    }
    actual_schemas = {path.name for path in SCHEMA_ROOT.glob("*.json")}
    require(expected_schemas <= actual_schemas, "MCP010B V2 schema files are missing")

    geometry = load_schema("geometry-program-v2.schema.json")
    require(
        geometry.get("properties", {}).get("schema_version", {}).get("const") == "GeometryProgram@2",
        "GeometryProgram@2 schema_version is not closed",
    )
    require_required(
        geometry,
        {
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "operator_catalog_sha256",
            "units",
            "budgets",
            "nodes",
            "part_outputs",
            "canonical_sha256",
        },
        "GeometryProgram@2",
    )
    units = geometry["properties"]["units"]
    require(
        units.get("properties", {}).get("length", {}).get("const") == "meter"
        and units.get("properties", {}).get("angle", {}).get("const") == "radian"
        and units.get("properties", {}).get("coordinate_system", {}).get("const") == "right-handed-y-up",
        "GeometryProgram@2 units must be meter/radian/right-handed-y-up",
    )
    budget_properties = geometry["properties"]["budgets"].get("properties", {})
    require(
        budget_properties.get("max_nodes", {}).get("maximum") == 512
        and budget_properties.get("max_triangles", {}).get("maximum") == 250000
        and budget_properties.get("max_glb_bytes", {}).get("maximum") == 67108864
        and budget_properties.get("max_worker_memory_bytes", {}).get("maximum") == 536870912
        and budget_properties.get("max_runtime_ms", {}).get("maximum") == 10000,
        "GeometryProgram@2 limits do not match MCP010B budgets",
    )
    node_properties = geometry["$defs"]["geometry_node"].get("properties", {})
    require(
        set(node_properties.get("operator_id", {}).get("enum", []))
        == {
            "forgecad.geometry.primitive@2",
            "forgecad.geometry.profile-extrude@1",
            "forgecad.geometry.profile-loft@1",
            "forgecad.geometry.revolve@1",
            "forgecad.geometry.tube-sweep@1",
            "forgecad.geometry.transform@2",
            "forgecad.geometry.mirror@1",
            "forgecad.geometry.array@1",
            "forgecad.geometry.panel@1",
            "forgecad.geometry.vent-array@1",
            "forgecad.geometry.joint-stack@1",
            "forgecad.geometry.part-output@1",
        }
        and node_properties.get("inputs", {}).get("maxItems") == 64
        and node_properties.get("inputs", {}).get("uniqueItems") is True,
        "GeometryProgram@2 must expose the closed MCP010D operator set and explicit DAG inputs",
    )
    parameter_refs = {
        item.get("$ref")
        for item in node_properties.get("parameters", {}).get("oneOf", [])
    }
    require(
        parameter_refs
        == {
            "#/$defs/box_parameters",
            "#/$defs/cylinder_parameters",
            "#/$defs/ellipsoid_parameters",
            "#/$defs/sphere_parameters",
            "#/$defs/profile_extrude_parameters",
            "#/$defs/profile_loft_parameters",
            "#/$defs/revolve_parameters",
            "#/$defs/tube_sweep_parameters",
            "#/$defs/transform_parameters",
            "#/$defs/mirror_parameters",
            "#/$defs/array_parameters",
            "#/$defs/panel_parameters",
            "#/$defs/vent_array_parameters",
            "#/$defs/joint_stack_parameters",
            "#/$defs/part_output_parameters",
        },
        "GeometryProgram@2 operator parameter variants drifted",
    )
    definitions = geometry["$defs"]
    require(
        definitions.get("identifier", {}).get("pattern") == "^[A-Za-z0-9_.-]{1,128}$"
        and definitions.get("dimension_scalar", {}).get("maximum") == 10
        and definitions.get("radius_scalar", {}).get("maximum") == 5
        and definitions.get("coordinate_scalar", {}).get("minimum") == -10
        and definitions.get("coordinate_scalar", {}).get("maximum") == 10,
        "GeometryProgram@2 identifier and physical primitive bounds drifted from the bounded worker",
    )
    part_output = geometry["$defs"]["part_output"]
    part_output_fields = {
        "part_id",
        "input_node_ids",
        "material_zone_id",
        "solid",
    }
    require(
        part_output.get("type") == "object"
        and part_output.get("additionalProperties") is False
        and set(part_output.get("required", [])) == part_output_fields
        and set(part_output.get("properties", {})) == part_output_fields
        and "source_node_id" not in part_output.get("properties", {}),
        "GeometryProgram@2 part_output must be a closed semantic Part sink",
    )
    part_output_inputs = part_output["properties"]["input_node_ids"]
    require(
        part_output_inputs.get("type") == "array"
        and part_output_inputs.get("minItems") == 1
        and part_output_inputs.get("maxItems") == 512
        and part_output_inputs.get("uniqueItems") is True
        and part_output_inputs.get("items", {}).get("$ref") == "#/$defs/identifier",
        "GeometryProgram@2 Part sink inputs must be ordered, non-empty, unique node IDs",
    )

    hash_request = load_schema("geometry-program-hash-request.schema.json")
    require(
        hash_request.get("type") == "object"
        and hash_request.get("additionalProperties") is False
        and hash_request.get("properties", {}).get("schema_version", {}).get("const")
        == "GeometryProgramHashRequest@1",
        "GeometryProgramHashRequest@1 must be a closed request envelope",
    )
    require_required(
        hash_request,
        {"schema_version", "geometry_program_draft"},
        "GeometryProgramHashRequest@1",
    )
    draft = hash_request.get("properties", {}).get("geometry_program_draft", {})
    expected_draft_fields = {
        "schema_version",
        "project_id",
        "representation_plan_sha256",
        "operator_catalog_sha256",
        "units",
        "budgets",
        "nodes",
        "part_outputs",
    }
    require(
        draft.get("type") == "object"
        and draft.get("additionalProperties") is False
        and set(draft.get("required", [])) == expected_draft_fields
        and set(draft.get("properties", {})) == expected_draft_fields
        and "canonical_sha256" not in draft.get("properties", {}),
        "GeometryProgramHashRequest@1 must accept exactly a hash-free GeometryProgram@2 draft",
    )
    require(
        draft.get("properties", {}).get("schema_version", {}).get("const") == "GeometryProgram@2"
        and draft.get("properties", {}).get("nodes", {}).get("items", {}).get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/geometry_node"
        and draft.get("properties", {}).get("part_outputs", {}).get("items", {}).get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json#/$defs/part_output",
        "GeometryProgramHashRequest@1 draft must reuse the GeometryProgram@2 node and Part output definitions",
    )

    hash_result = load_schema("geometry-program-hash-result.schema.json")
    expected_hash_result_fields = {
        "schema_version",
        "geometry_program_schema_version",
        "canonical_sha256",
        "operator_catalog_sha256",
        "validation_status",
    }
    require(
        hash_result.get("type") == "object"
        and hash_result.get("additionalProperties") is False
        and set(hash_result.get("required", [])) == expected_hash_result_fields
        and set(hash_result.get("properties", {})) == expected_hash_result_fields
        and hash_result.get("properties", {}).get("schema_version", {}).get("const")
        == "GeometryProgramHashResult@1"
        and hash_result.get("properties", {}).get("geometry_program_schema_version", {}).get("const")
        == "GeometryProgram@2"
        and hash_result.get("properties", {}).get("validation_status", {}).get("const") == "passed",
        "GeometryProgramHashResult@1 must be a closed, passing V2 hash receipt",
    )
    require(
        hash_result.get("properties", {}).get("canonical_sha256", {}).get("$ref") == "#/$defs/sha256"
        and hash_result.get("properties", {}).get("operator_catalog_sha256", {}).get("$ref")
        == "#/$defs/sha256"
        and hash_result.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$",
        "GeometryProgramHashResult@1 hashes must be lowercase SHA-256 values",
    )

    catalog = load_schema("operator-catalog.schema.json")
    require(
        catalog.get("properties", {}).get("schema_version", {}).get("const") == "OperatorCatalog@1",
        "OperatorCatalog@1 schema_version is not closed",
    )
    operator = catalog["$defs"]["operator"]
    catalog_entries = catalog.get("properties", {}).get("operators", {})
    require(
        catalog_entries.get("maxItems") == 32
        and operator.get("properties", {}).get("status", {}).get("enum") == ["active", "unavailable"],
        "OperatorCatalog@1 must expose a bounded active/unavailable operator catalog",
    )
    require(
        operator.get("properties", {}).get("operator_id", {}).get("$ref")
        == "#/$defs/identifier"
        and operator.get("properties", {}).get("supported_shapes", {}).get("maxItems") == 8,
        "OperatorCatalog@1 generic operator definition drifted",
    )

    readback = load_schema("artifact-readback-v2.schema.json")
    require(
        readback.get("properties", {}).get("schema_version", {}).get("const") == "ArtifactReadback@2",
        "ArtifactReadback@2 schema_version is not closed",
    )
    require_required(
        readback,
        {
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "triangle_count",
            "part_ids",
            "source_node_ids",
            "material_zone_ids",
            "part_bindings",
            "validator_status",
            "hard_gate_passed",
            "integrity",
            "canonical_sha256",
        },
        "ArtifactReadback@2",
    )
    require_required(
        readback["$defs"]["part_binding"],
        {"part_id", "source_node_id", "material_zone_id", "solid", "triangle_count"},
        "ArtifactReadback@2 part_binding",
    )
    require(
        "input_node_ids" not in readback["$defs"]["part_binding"].get("properties", {}),
        "ArtifactReadback@2 must preserve one source binding per Part input",
    )
    integrity_required = {
        "glb_parse_status",
        "invalid_index_count",
        "non_finite_count",
        "degenerate_triangle_count",
        "boundary_edge_count",
        "non_manifold_edge_count",
        "winding_error_count",
        "uv_non_finite_count",
        "zero_area_uv_triangle_count",
        "tangent_non_finite_count",
        "tangent_orthogonality_error_count",
        "tangent_handedness_error_count",
        "metadata_mismatch_count",
        "external_uri_count",
        "part_coverage",
        "source_coverage",
        "material_zone_coverage",
    }
    require_required(readback["$defs"]["integrity"], integrity_required, "ArtifactReadback@2 integrity")
    passing_integrity = readback["$defs"]["passing_integrity"]["allOf"][1].get("properties", {})
    require(
        all(passing_integrity.get(name, {}).get("const") == 0 for name in integrity_required if name.endswith("_count"))
        and all(
            passing_integrity.get(name, {}).get("const") == 1
            for name in {"part_coverage", "source_coverage", "material_zone_coverage"}
        )
        and passing_integrity.get("glb_parse_status", {}).get("const") == "passed",
        "ArtifactReadback@2 passing integrity must represent actual zero-error readback",
    )

    result = load_schema("geometry-prepare-result-v2.schema.json")
    require(
        result.get("properties", {}).get("schema_version", {}).get("const") == "GeometryPrepareResult@2"
        and result.get("properties", {}).get("operator_catalog", {}).get("$ref")
        == "https://forgecad.local/contracts/operator-catalog.schema.json"
        and result.get("properties", {}).get("artifact", {}).get("$ref")
        == "https://forgecad.local/contracts/artifact-readback-v2.schema.json",
        "GeometryPrepareResult@2 must bind the V2 catalog and readback contracts",
    )

    quality = load_schema("geometry-quality-report-v2.schema.json")
    expected_quality_fields = {
        "schema_version",
        "scope",
        "quality_report_id",
        "candidate_id",
        "artifact_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "artifact_readback_object_sha256",
        "integrity",
        "hard_gate_passed",
        "canonical_sha256",
    }
    quality_properties = quality.get("properties", {})
    require(
        quality.get("type") == "object"
        and quality.get("additionalProperties") is False
        and set(quality.get("required", [])) == expected_quality_fields
        and set(quality_properties) == expected_quality_fields
        and quality_properties.get("schema_version", {}).get("const") == "GeometryQualityReport@2"
        and quality_properties.get("scope", {}).get("const")
        == "mcp010b-strict-glb-bin-accessor-hard-gates"
        and quality_properties.get("hard_gate_passed", {}).get("const") is True,
        "GeometryQualityReport@2 must be a closed, strict V2 hard-gate receipt",
    )
    require(
        all(
            quality_properties.get(name, {}).get("$ref") == "#/$defs/identifier"
            for name in {"quality_report_id", "candidate_id"}
        )
        and all(
            quality_properties.get(name, {}).get("$ref") == "#/$defs/sha256"
            for name in {
                "artifact_sha256",
                "program_sha256",
                "operator_catalog_sha256",
                "readback_config_sha256",
                "artifact_readback_object_sha256",
                "canonical_sha256",
            }
        )
        and quality.get("$defs", {}).get("identifier", {}).get("pattern")
        == "^[A-Za-z0-9_.-]{1,128}$"
        and quality.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$",
        "GeometryQualityReport@2 identity and provenance hashes must be bounded",
    )
    require(
        quality_properties.get("integrity", {}).get("$ref")
        == "geometry-quality-report-v2.schema.json#/$defs/integrity"
        and quality.get("$defs", {}).get("integrity", {}).get("$ref")
        == "artifact-readback-v2.schema.json#/$defs/passing_integrity",
        "GeometryQualityReport@2 must require ArtifactReadback@2 passing integrity",
    )

    evidence = load_schema("geometry-candidate-evidence.schema.json")
    expected_evidence_fields = {
        "schema_version",
        "candidate_id",
        "project_id",
        "reference_id",
        "reference_sha256",
        "geometry_program_sha256",
        "geometry_program_object_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "artifact_object_sha256",
        "artifact_readback_object_sha256",
        "quality_report_object_sha256",
        "quality_report_id",
        "canonical_sha256",
        "created_at",
    }
    evidence_properties = evidence.get("properties", {})
    require(
        evidence.get("type") == "object"
        and evidence.get("additionalProperties") is False
        and set(evidence.get("required", [])) == expected_evidence_fields
        and set(evidence_properties) == expected_evidence_fields
        and evidence_properties.get("schema_version", {}).get("const") == "GeometryCandidateEvidence@1",
        "GeometryCandidateEvidence@1 must be a closed candidate provenance record",
    )
    require(
        all(
            evidence_properties.get(name, {}).get("$ref") == "#/$defs/identifier"
            for name in {"candidate_id", "project_id", "quality_report_id"}
        )
        and all(
            evidence_properties.get(name, {}).get("$ref") == "#/$defs/sha256"
            for name in {
                "geometry_program_sha256",
                "geometry_program_object_sha256",
                "operator_catalog_sha256",
                "readback_config_sha256",
                "artifact_object_sha256",
                "artifact_readback_object_sha256",
                "quality_report_object_sha256",
                "canonical_sha256",
            }
        )
        and evidence.get("$defs", {}).get("identifier", {}).get("pattern")
        == "^[A-Za-z0-9_.-]{1,128}$"
        and evidence.get("$defs", {}).get("sha256", {}).get("pattern") == "^[0-9a-f]{64}$"
        and evidence_properties.get("created_at", {}).get("type") == "string"
        and evidence_properties.get("created_at", {}).get("minLength") == 1
        and evidence_properties.get("created_at", {}).get("maxLength") == 64,
        "GeometryCandidateEvidence@1 must bind bounded identities, hashes, and creation evidence",
    )
    require(
        set(evidence_properties.get("reference_id", {}).get("type", [])) == {"string", "null"}
        and evidence_properties.get("reference_id", {}).get("pattern")
        == "^[A-Za-z0-9_.-]{1,128}$"
        and set(evidence_properties.get("reference_sha256", {}).get("type", [])) == {"string", "null"}
        and evidence_properties.get("reference_sha256", {}).get("pattern") == "^[0-9a-f]{64}$",
        "GeometryCandidateEvidence@1 reference fields must be bounded nullable values",
    )
    reference_pairing = evidence.get("allOf", [])
    require(
        len(reference_pairing) == 1
        and reference_pairing[0].get("if", {}).get("properties", {}).get("reference_id", {}).get("const")
        is None
        and reference_pairing[0].get("if", {}).get("required") == ["reference_id"]
        and reference_pairing[0].get("then", {}).get("properties", {}).get("reference_sha256", {}).get("const")
        is None
        and reference_pairing[0].get("else", {}).get("properties", {}).get("reference_sha256", {}).get("$ref")
        == "#/$defs/sha256",
        "GeometryCandidateEvidence@1 must pair a missing reference ID with null hash and a bound ID with SHA-256",
    )

    appearance = load_schema("appearance-prepare-result.schema.json")
    require_required(
        appearance,
        {"render_set_object_sha256", "quality_report_object_sha256"},
        "AppearancePrepareResult@1 CAS receipts",
    )
    render_set = load_schema("render-set.schema.json")
    require_required(render_set, {"pass_artifacts"}, "RenderSet@1 pass artifacts")
    pass_artifacts = render_set["properties"]["pass_artifacts"]
    require(
        pass_artifacts.get("additionalProperties") is False
        and set(pass_artifacts.get("properties", {}))
        == {"beauty", "silhouette", "normal", "part-id", "material-id", "depth", "ao", "wireframe"},
        "RenderSet@1 pass artifacts must use a closed pass mapping",
    )

    skill_manifest = load_schema("skill-bundle-manifest.schema.json")
    execution_availability = skill_manifest.get("properties", {}).get("execution_availability", {})
    missing_operator_ids = skill_manifest.get("properties", {}).get("missing_operator_ids", {})
    require(
        execution_availability.get("enum") == ["active", "partial", "unavailable"],
        "SkillBundleManifest execution availability is not closed",
    )
    require(
        missing_operator_ids.get("uniqueItems") is True
        and missing_operator_ids.get("items", {}).get("pattern")
        == "^forgecad\\.[a-z0-9_.-]+@[0-9]+$",
        "SkillBundleManifest missing operator IDs are not bounded",
    )
    conditions = skill_manifest.get("allOf", [])
    require(
        len(conditions) == 3
        and conditions[0].get("then", {}).get("required") == ["missing_operator_ids"]
        and conditions[1].get("then", {}).get("properties", {}).get("missing_operator_ids", {}).get("maxItems") == 0
        and conditions[2].get("then", {}).get("properties", {}).get("missing_operator_ids", {}).get("minItems") == 1,
        "SkillBundleManifest execution availability must fail closed when an operator lock is incomplete",
    )


def check_mcp010c_contracts() -> None:
    """Keep the fixed-render/reference-review contracts closed and hash-bound."""
    expected = {
        "reference-view-spec.schema.json": "ReferenceViewSpec@1",
        "camera-calibration.schema.json": "CameraCalibration@1",
        "render-set-v2.schema.json": "RenderSet@2",
        "reference-comparison-report.schema.json": "ReferenceComparisonReport@1",
        "visual-review-report.schema.json": "VisualReviewReport@1",
        "human-visual-review-receipt.schema.json": "HumanVisualReviewReceipt@1",
        "quality-report-v2.schema.json": "QualityReport@2",
    }
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version,
            f"{version} must be a closed object contract",
        )
        require_required(schema, {"schema_version", "canonical_sha256"}, version)

    camera = load_schema("camera-calibration.schema.json")
    require(
        camera["properties"]["projection"].get("const") == "perspective"
        and camera["properties"]["resolution"]["properties"]["width"].get("const") == 512
        and camera["properties"]["resolution"]["properties"]["height"].get("const") == 512
        and camera["properties"]["coordinate_system"].get("const") == "right-handed-y-up-meter",
        "CameraCalibration@1 must be a deterministic 512x512 perspective camera",
    )

    render = load_schema("render-set-v2.schema.json")
    render_passes = [
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
    ]
    passes = render["properties"]["passes"]
    require(
        passes.get("minItems") == 9
        and passes.get("maxItems") == 9
        and set(passes.get("items", {}).get("enum", [])) == set(render_passes)
        and set(render["properties"]["pass_artifacts"].get("required", [])) == set(render_passes),
        "RenderSet@2 must require exactly the nine fixed AOV passes",
    )
    pass_artifact = render["$defs"]["pass_artifact"]["properties"]
    require(
        pass_artifact["mime"].get("const") == "image/png"
        and pass_artifact["width"].get("const") == 512
        and pass_artifact["height"].get("const") == 512,
        "RenderSet@2 pass artifacts must be 512x512 PNGs",
    )

    comparison = load_schema("reference-comparison-report.schema.json")
    require(
        set(comparison["properties"]["status"].get("enum", []))
        == {"PARTIAL_VISIBLE_VIEW_PASS", "QUALITY_TARGET_NOT_MET", "BLOCKED_REFERENCE_COVERAGE"}
        and set(comparison["properties"]["metrics"]["required"])
        == {
            "silhouette_iou",
            "boundary_f1_4px",
            "bbox_edge_error",
            "centroid_error",
            "landmark_coverage",
            "landmark_nme",
            "region_median_iou",
            "critical_region_min_iou",
        },
        "ReferenceComparisonReport@1 must expose the fixed metric set and explicit partial/blocked status",
    )
    require(
        comparison["properties"]["mask"]["properties"]["method"].get("const")
        == "local-border-flood-fill-morphology",
        "ReferenceComparisonReport@1 must use the local deterministic mask method",
    )

    review = load_schema("visual-review-report.schema.json")
    require(
        review["properties"]["round"].get("maximum") == 5
        and set(review["properties"]["stage"].get("enum", []))
        == {"silhouette", "structure", "form", "material-surface", "final"},
        "VisualReviewReport@1 must bound review rounds to the five fixed stages",
    )
    human = load_schema("human-visual-review-receipt.schema.json")
    require(
        set(human["properties"]["scores"]["required"])
        == {"likeness", "geometry_detail", "material_fidelity", "editability"}
        and human["properties"]["scores"]["properties"]["likeness"].get("minimum") == 1
        and human["properties"]["scores"]["properties"]["likeness"].get("maximum") == 5,
        "HumanVisualReviewReceipt@1 must bind all four 1-5 user scores",
    )
    quality = load_schema("quality-report-v2.schema.json")
    require(
        quality["properties"]["visual_status"].get("enum")
        == [
            "PARTIAL_VISIBLE_VIEW_PASS",
            "QUALITY_TARGET_NOT_MET",
            "BLOCKED_REFERENCE_COVERAGE",
            "not-run",
        ]
        and quality["properties"]["hard_gate_passed"].get("type") == "boolean",
        "QualityReport@2 must distinguish visual status from structural hard gates",
    )


def check_mcp010e_contracts() -> None:
    """Keep the offline material/texture authoring path closed and bounded."""
    expected = {
        "material-pack-manifest.schema.json": "MaterialPackManifest@1",
        "material-definition.schema.json": "MaterialDefinition@1",
        "texture-set.schema.json": "TextureSet@1",
        "texture-build-receipt.schema.json": "TextureBuildReceipt@1",
        "appearance-program-v2.schema.json": "AppearanceProgram@2",
        "appearance-prepare-result-v2.schema.json": "AppearancePrepareResult@2",
    }
    versioned = {
        filename: version
        for filename, version in expected.items()
        if filename
        not in {"material-definition.schema.json", "texture-set.schema.json"}
    }
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and (
                filename not in {"material-definition.schema.json", "texture-set.schema.json"}
                or schema.get("title") == version.removesuffix("@1")
            ),
            f"{version} must be a closed object contract",
        )
        if filename in versioned:
            require_required(schema, {"schema_version"}, version)

    manifest = load_schema("material-pack-manifest.schema.json")
    require_required(
        manifest,
        {
            "schema_version",
            "pack_id",
            "version",
            "source_assets",
            "textures",
            "material_definitions",
            "texture_sets",
            "limits",
            "canonical_sha256",
        },
        "MaterialPackManifest@1",
    )
    limits = manifest["properties"]["limits"]["properties"]
    require(
        limits["max_texture_bytes"].get("const") == 67108864
        and limits["max_export_bytes"].get("const") == 134217728
        and limits["embedded_only"].get("const") is True
        and limits["external_uri"].get("const") is False,
        "MaterialPackManifest@1 must keep embedded texture and size limits closed",
    )
    texture = load_schema("texture-set.schema.json")
    texture_fields = texture["$defs"]["texture"]["properties"]
    require(
        texture_fields["file"].get("pattern") == r"^textures/[A-Za-z0-9_.-]+\.png$"
        and texture_fields["normal_convention"].get("enum") == ["OpenGL+Y", None]
        and texture_fields["width"].get("maximum") == 2048,
        "TextureSet@1 must use embedded PNGs and explicit OpenGL normal convention",
    )
    appearance = load_schema("appearance-program-v2.schema.json")
    require_required(
        appearance,
        {
            "schema_version",
            "project_id",
            "geometry_program_sha256",
            "material_pack_id",
            "material_pack_manifest_sha256",
            "material_zones",
            "canonical_sha256",
        },
        "AppearanceProgram@2",
    )
    require(
        appearance["properties"]["material_pack_id"].get("const")
        == "forgecad-hard-surface-robot"
        and appearance["properties"]["material_zones"]["items"].get("additionalProperties")
        is False,
        "AppearanceProgram@2 must bind the first-party pack and closed material zones",
    )
    result = load_schema("appearance-prepare-result-v2.schema.json")
    require(
        result["properties"]["artifact"].get("$ref")
        == "https://forgecad.local/contracts/artifact-readback-v2.schema.json"
        and result["properties"]["render_set"].get("$ref")
        == "https://forgecad.local/contracts/render-set-v2.schema.json",
        "AppearancePrepareResult@2 must expose strict artifact and nine-pass render receipts",
    )


def check_mcp010f_silhouette_contracts() -> None:
    """Keep the contour-first target and diagnostic receipts closed."""
    expected = {
        "silhouette-target.schema.json": "SilhouetteTarget@1",
        "reference-mask-prepare-result.schema.json": "ReferenceMaskPrepareResult@1",
        "camera-fit-result.schema.json": "CameraFitResult@1",
        "camera-calibration-ref.schema.json": "CameraCalibrationRef@1",
        "boundary-error-result.schema.json": "BoundaryErrorResult@1",
        "silhouette-rig.schema.json": "SilhouetteRig@1",
        "silhouette-rig-hash-request.schema.json": "SilhouetteRigHashRequest@1",
        "silhouette-rig-hash-result.schema.json": "SilhouetteRigHashResult@1",
        "silhouette-fit-intent.schema.json": "SilhouetteFitIntent@1",
        "silhouette-fit-result.schema.json": "SilhouetteFitResult@1",
        "part-contour-fit-result.schema.json": "PartContourFitResult@1",
        "silhouette-candidate-compare-result.schema.json": "SilhouetteCandidateCompareResult@1",
    }
    for filename, version in expected.items():
        schema = load_schema(filename)
        require(
            schema.get("type") == "object"
            and schema.get("additionalProperties") is False
            and schema.get("properties", {}).get("schema_version", {}).get("const") == version,
            f"{version} must be a closed object contract",
        )
        required = {"schema_version"}
        if filename not in {"silhouette-rig-hash-request.schema.json"}:
            required.add("canonical_sha256")
        require_required(schema, required, version)

    target = load_schema("silhouette-target.schema.json")
    require(
        target["properties"]["coordinate_space"].get("const")
        == "normalized_reference_image"
        and target["properties"]["width"].get("const") == 512
        and target["properties"]["height"].get("const") == 512
        and target["properties"]["contour_points"].get("maxItems") == 512,
        "SilhouetteTarget@1 must use normalized 512x512 contour truth",
    )
    camera_fit = load_schema("camera-fit-result.schema.json")
    require(
        camera_fit["properties"]["candidates"].get("maxItems") == 128
        and camera_fit["properties"]["status"].get("enum")
        == ["ready", "no_improvement", "unavailable"],
        "CameraFitResult@1 must remain bounded and typed",
    )
    boundary = load_schema("boundary-error-result.schema.json")
    require(
        boundary["properties"]["segments"].get("maxItems") == 64
        and len(boundary["$defs"]["point_px"].get("prefixItems", [])) == 2
        and boundary["properties"]["segments"]["items"]["properties"]["direction"].get("enum")
        == ["inward", "outward", "aligned"],
        "BoundaryErrorResult@1 must expose bounded signed pixel segments",
    )
    require(
        "sdf_chamfer_px" in boundary["properties"]["metrics"]["required"],
        "BoundaryErrorResult@1 must expose SDF/Chamfer loss",
    )
    rig = load_schema("silhouette-rig.schema.json")
    require(
        rig["properties"]["parameters"].get("maxItems") == 64
        and rig["additionalProperties"] is False,
        "SilhouetteRig@1 must be bounded and closed",
    )
    fit = load_schema("silhouette-fit-result.schema.json")
    require(
        fit["properties"]["evaluations"].get("maximum") == 64
        and fit["$defs"]["thresholds"]["properties"]["silhouette_iou"].get("const") == 0.9,
        "SilhouetteFitResult@1 must expose bounded optimizer evidence and strict thresholds",
    )
    require(
        set(
            [
                "baseline_camera",
                "baseline_metrics",
                "camera_evaluations",
                "baseline_loss",
                "selected_loss",
                "strict_improvement",
            ]
        ).issubset(set(fit.get("required", []))),
        "SilhouetteFitResult@1 must retain Runtime-owned baseline and strict-improvement evidence",
    )
    selected_geometry_program = fit["properties"].get("selected_geometry_program", {})
    require(
        "selected_geometry_program" in fit.get("required", [])
        and selected_geometry_program.get("oneOf", [{}])[0].get("type") == "null"
        and selected_geometry_program.get("oneOf", [{}, {}])[1].get("$ref")
        == "https://forgecad.local/contracts/geometry-program-v2.schema.json",
        "SilhouetteFitResult@1 must expose an optional Runtime-validated GeometryProgram proposal",
    )
    camera_ref = load_schema("camera-calibration-ref.schema.json")
    require(
        camera_ref.get("additionalProperties") is False
        and set(camera_ref.get("required", []))
        == {"schema_version", "camera_hash", "canonical_sha256"}
        and camera_ref["properties"]["schema_version"].get("const")
        == "CameraCalibrationRef@1",
        "CameraCalibrationRef@1 must contain only Runtime-owned camera hashes",
    )
    fit_intent = load_schema("silhouette-fit-intent.schema.json")
    fit_camera = fit_intent["properties"]["base_camera"]
    require(
        {item.get("$ref") for item in fit_camera.get("oneOf", [])}
        == {
            "https://forgecad.local/contracts/camera-calibration.schema.json",
            "https://forgecad.local/contracts/camera-calibration-ref.schema.json",
        },
        "SilhouetteFitIntent@1 must accept a full camera or a compact Runtime camera reference",
    )
    compare = load_schema("silhouette-candidate-compare-result.schema.json")
    require(
        compare["properties"]["candidates"].get("minItems") == 2
        and compare["properties"]["candidates"].get("maxItems") == 8,
        "SilhouetteCandidateCompareResult@1 must compare a bounded candidate set",
    )


def main() -> int:
    required = [
        CONTRACT_ROOT / "manifest.json",
        SCHEMA_ROOT / "audit-event.schema.json",
        SCHEMA_ROOT / "candidate.schema.json",
        SCHEMA_ROOT / "cas-object.schema.json",
        SCHEMA_ROOT / "design-asset-version.schema.json",
        SCHEMA_ROOT / "job-event.schema.json",
        SCHEMA_ROOT / "project.schema.json",
        SCHEMA_ROOT / "runtime-capabilities.schema.json",
        SCHEMA_ROOT / "runtime-tool.schema.json",
        SCHEMA_ROOT / "runtime-project.schema.json",
        SCHEMA_ROOT / "runtime-snapshot.schema.json",
        SCHEMA_ROOT / "runtime-job.schema.json",
        SCHEMA_ROOT / "runtime-error.schema.json",
        SCHEMA_ROOT / "runtime-resource.schema.json",
        SCHEMA_ROOT / "runtime-selection.schema.json",
        SCHEMA_ROOT / "snapshot.schema.json",
        ROOT / "migrations-runtime-v1" / "0001_runtime.sql",
    ]
    missing = [str(path.relative_to(ROOT)) for path in required if not path.exists()]
    if missing:
        raise SystemExit(f"missing MCP002 contract files: {missing}")

    for path in sorted(SCHEMA_ROOT.glob("*.json")):
        document = json.loads(path.read_text(encoding="utf-8"))
        if document.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
            raise SystemExit(f"schema draft missing: {path}")
        if not str(document.get("$id", "")).startswith("https://forgecad.local/contracts/"):
            raise SystemExit(f"schema id missing: {path}")

    manifest = json.loads((CONTRACT_ROOT / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("contract_set") != "forgecad-runtime-contracts@1":
        raise SystemExit("unexpected contract set")
    if manifest.get("model_calls") is not False:
        raise SystemExit("MCP002 contracts must declare model_calls=false")
    actual_schemas = sorted(path.name for path in SCHEMA_ROOT.glob("*.json"))
    declared_schemas = sorted(manifest.get("schemas", []))
    if actual_schemas != declared_schemas:
        raise SystemExit("contract manifest schema list does not match checked-in schemas")
    check_mcp010b_contracts()
    check_mcp010c_contracts()
    check_mcp010f_silhouette_contracts()
    check_mcp010e_contracts()
    print(f"ForgeCAD contracts OK: {len(actual_schemas)} schemas")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
