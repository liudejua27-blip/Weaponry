#!/usr/bin/env python3
"""Create and validate human review records for formal Blender Module Packs."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker

from concept_module_pack import (
    ModulePackValidationError,
    ValidatedModulePack,
    _read_glb_json,
    validate_module_pack,
)


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = (
    ROOT / "packages" / "concept-spec" / "schemas" / "formal-module-review.schema.json"
)
DEFAULT_BASELINE_PACK = ROOT / "assets" / "module-packs" / "weapon-concept-v1-reference"
COMMITTED_PACK_ROOT = ROOT / "assets" / "module-packs"
REVIEW_SCHEMA = "FormalModuleReview@1"
REPORT_SCHEMA = "ForgeCADFormalModulePromotionReport@1"
FIRST_THREE_IDS = {
    "module_core_shell_01",
    "module_front_shell_01",
    "module_front_shell_02",
}
LIMITATIONS = {
    "non_functional_concept_asset",
    "not_manufacturing_documentation",
    "not_structural_or_safety_validation",
}
ATTESTATION = (
    "I reviewed the listed source and exported artifacts and approve them for "
    "the declared non-functional visual asset scope."
)
MATERIALS = {"MAT_primary", "MAT_secondary", "MAT_accent"}
TRIANGLE_FLOORS = {
    "core_shell": 1000,
    "front_shell": 500,
    "rear_shell": 500,
    "grip_shell": 500,
    "top_accessory": 250,
    "side_accessory": 250,
    "lower_structure": 500,
    "storage_visual": 500,
    "armor_panel": 250,
}
FORBIDDEN_LICENSE_MARKERS = (
    "authoring-starter",
    "reference-assets",
    "not final art",
)


class FormalModuleReviewError(RuntimeError):
    def __init__(self, findings: list[dict[str, str]]) -> None:
        self.code = "FORMAL_MODULE_REVIEW_FAILED"
        self.findings = findings
        super().__init__(
            "; ".join(f"{item['code']}: {item['message']}" for item in findings[:8])
        )


def generate_review_draft(
    pack_root: Path,
    source_root: Path,
    output: Path,
    *,
    scope: str,
    baseline_pack_root: Path = DEFAULT_BASELINE_PACK,
) -> dict[str, Any]:
    pack, sources, _ = _prepare_inputs(
        pack_root,
        source_root,
        scope=scope,
        baseline_pack_root=baseline_pack_root,
    )
    destination = output.expanduser().resolve()
    if destination.exists() or destination.is_symlink():
        raise FormalModuleReviewError(
            [_finding("REVIEW_DRAFT_EXISTS", "Review draft output already exists.")]
        )
    if destination.is_relative_to(COMMITTED_PACK_ROOT):
        raise FormalModuleReviewError(
            [
                _finding(
                    "REVIEW_DRAFT_COMMITTED_PACK_DENIED",
                    "Review drafts must not be written into committed Module Packs.",
                )
            ]
        )
    raw_pack = _read_json(pack.root / "pack.json")
    draft = {
        "schema_version": REVIEW_SCHEMA,
        "review_id": (
            "fmr_draft_" + datetime.now(timezone.utc).strftime("%Y%m%d_%H%M%S").lower()
        ),
        "scope": scope,
        "pack_id": pack.manifest.pack_id,
        "pack_version": pack.manifest.version,
        "pack_manifest_sha256": _sha256(pack.root / "pack.json"),
        "pack_license_sha256": _sha256(pack.root / raw_pack["license"]["license_path"]),
        "author_id": "TODO_AUTHOR",
        "reviewer": {
            "reviewer_id": "TODO_REVIEWER",
            "display_name": "TODO Reviewer",
            "role": "asset_reviewer",
        },
        "reviewed_at": _utc_now(),
        "approval_status": "changes_requested",
        "reviewer_attestation": "Pending independent human art review and approval.",
        "limitations_acknowledged": sorted(LIMITATIONS),
        "pack_review": {
            "stable_ids_preserved": False,
            "connector_semantics_preserved": False,
            "license_reviewed": False,
            "source_export_reproducible": False,
            "intended_uses_verified": bool(raw_pack.get("non_functional_only")),
            "human_final_art_approved": False,
            "notes": "TODO: record pack-level review findings and approval rationale.",
        },
        "modules": [
            {
                "module_id": module.manifest.module_id,
                "source_blend_file": sources[module.manifest.module_id].name,
                "source_blend_sha256": _sha256(sources[module.manifest.module_id]),
                "module_manifest_sha256": _sha256(
                    pack.root
                    / next(
                        entry.manifest_path
                        for entry in pack.manifest.modules
                        if entry.module_id == module.manifest.module_id
                    )
                ),
                "glb_sha256": _sha256(module.glb_path),
                "thumbnail_sha256": _sha256(
                    pack.root
                    / next(
                        entry.thumbnail_path
                        for entry in pack.manifest.modules
                        if entry.module_id == module.manifest.module_id
                    )
                ),
                "license_sha256": _sha256(
                    pack.root
                    / next(
                        entry.license_path
                        for entry in pack.manifest.modules
                        if entry.module_id == module.manifest.module_id
                    )
                ),
                "review": {
                    "silhouette_distinct": False,
                    "surface_hierarchy_reviewed": False,
                    "material_partition_reviewed": False,
                    "uv0_reviewed": False,
                    "thumbnail_reviewed": False,
                    "connector_contract_reviewed": False,
                    "transforms_and_modifiers_reviewed": False,
                    "non_functional_visual_only": False,
                    "scores": {
                        "silhouette": 1,
                        "surface_hierarchy": 1,
                        "material_readability": 1,
                        "modular_readability": 1,
                        "thumbnail_quality": 1,
                    },
                    "notes": "TODO: record module-specific visual and technical review.",
                },
            }
            for module in pack.modules
        ],
    }
    _validate_review_schema(draft)
    destination.parent.mkdir(parents=True, exist_ok=True)
    _write_json(destination, draft)
    return {
        "ok": True,
        "status": "review_draft_created",
        "schema_version": REVIEW_SCHEMA,
        "scope": scope,
        "module_count": len(pack.modules),
        "output": str(destination),
    }


def validate_formal_review(
    pack_root: Path,
    source_root: Path,
    review_path: Path,
    *,
    baseline_pack_root: Path = DEFAULT_BASELINE_PACK,
) -> dict[str, Any]:
    review_file = review_path.expanduser().resolve()
    review = _read_json(review_file)
    _validate_review_schema(review)
    scope = str(review["scope"])
    pack, sources, baseline = _prepare_inputs(
        pack_root,
        source_root,
        scope=scope,
        baseline_pack_root=baseline_pack_root,
    )
    findings: list[dict[str, str]] = []
    raw_pack = _read_json(pack.root / "pack.json")
    if review["pack_id"] != pack.manifest.pack_id:
        findings.append(_finding("REVIEW_PACK_ID_MISMATCH", "Review pack_id changed."))
    if review["pack_version"] != pack.manifest.version:
        findings.append(
            _finding("REVIEW_PACK_VERSION_MISMATCH", "Review pack version changed.")
        )
    pack_hash = _sha256(pack.root / "pack.json")
    if review["pack_manifest_sha256"] != pack_hash:
        findings.append(
            _finding(
                "REVIEW_PACK_HASH_MISMATCH",
                "pack.json changed after the review draft was created.",
            )
        )
    pack_license = pack.root / raw_pack["license"]["license_path"]
    pack_license_hash = _sha256(pack_license)
    if review["pack_license_sha256"] != pack_license_hash:
        findings.append(
            _finding(
                "REVIEW_ARTIFACT_HASH_MISMATCH",
                "Pack license no longer matches the reviewed artifact.",
            )
        )

    _validate_manual_approval(review, findings)
    _validate_pack_license(pack, raw_pack, findings)
    _validate_baseline_compatibility(pack, baseline, scope, findings)

    reviewed_modules = review["modules"]
    review_by_id: dict[str, dict[str, Any]] = {}
    for item in reviewed_modules:
        module_id = str(item["module_id"])
        if module_id in review_by_id:
            findings.append(
                _finding(
                    "REVIEW_DUPLICATE_MODULE",
                    "Review contains the same module more than once.",
                    module_id,
                )
            )
        review_by_id[module_id] = item
    actual_ids = {module.manifest.module_id for module in pack.modules}
    if set(review_by_id) != actual_ids:
        findings.append(
            _finding(
                "REVIEW_MODULE_SET_MISMATCH",
                f"Review modules {sorted(review_by_id)} do not match pack {sorted(actual_ids)}.",
            )
        )

    module_reports: list[dict[str, Any]] = []
    for module in pack.modules:
        module_id = module.manifest.module_id
        item = review_by_id.get(module_id)
        if item is None:
            continue
        source = sources[module_id]
        entry = next(
            value for value in pack.manifest.modules if value.module_id == module_id
        )
        thumbnail = pack.root / entry.thumbnail_path
        expected_hashes = {
            "source_blend_sha256": _sha256(source),
            "module_manifest_sha256": _sha256(pack.root / entry.manifest_path),
            "glb_sha256": _sha256(module.glb_path),
            "thumbnail_sha256": _sha256(thumbnail),
            "license_sha256": _sha256(pack.root / entry.license_path),
        }
        if item["source_blend_file"] != source.name:
            findings.append(
                _finding(
                    "REVIEW_SOURCE_FILE_MISMATCH",
                    f"Expected source file {source.name}.",
                    module_id,
                )
            )
        for field, expected in expected_hashes.items():
            if item[field] != expected:
                findings.append(
                    _finding(
                        "REVIEW_ARTIFACT_HASH_MISMATCH",
                        f"{field} no longer matches the reviewed artifact.",
                        module_id,
                    )
                )

        generator = _glb_generator(module.glb_path)
        if "blender" not in generator.casefold():
            findings.append(
                _finding(
                    "FORMAL_DCC_GENERATOR_REQUIRED",
                    f"GLB generator is {generator!r}; a Blender export is required.",
                    module_id,
                )
            )
        category = _category_value(module.manifest.category)
        floor = TRIANGLE_FLOORS[category]
        if module.manifest.triangle_count < floor:
            findings.append(
                _finding(
                    "FORMAL_TRIANGLE_FLOOR_NOT_MET",
                    (
                        f"triangle_count={module.manifest.triangle_count}; "
                        f"anti-placeholder floor={floor}."
                    ),
                    module_id,
                )
            )
        if set(module.manifest.material_slots) != MATERIALS:
            findings.append(
                _finding(
                    "FORMAL_MATERIAL_SET_MISMATCH",
                    f"Formal modules must use {sorted(MATERIALS)}.",
                    module_id,
                )
            )
        _validate_module_review(item, findings)
        scores = item["review"]["scores"]
        module_reports.append(
            {
                "module_id": module_id,
                "category": category,
                "triangle_count": module.manifest.triangle_count,
                "triangle_floor": floor,
                "glb_generator": generator,
                **expected_hashes,
                "average_visual_score": round(
                    sum(int(value) for value in scores.values()) / len(scores), 2
                ),
            }
        )

    for label, hashes in (
        ("source", [item["source_blend_sha256"] for item in reviewed_modules]),
        ("GLB", [item["glb_sha256"] for item in reviewed_modules]),
        ("thumbnail", [item["thumbnail_sha256"] for item in reviewed_modules]),
    ):
        if len(set(hashes)) != len(hashes):
            findings.append(
                _finding(
                    "FORMAL_ARTIFACTS_NOT_DISTINCT",
                    f"All reviewed {label} artifacts must be distinct.",
                )
            )

    if findings:
        raise FormalModuleReviewError(findings)
    return {
        "schema_version": REPORT_SCHEMA,
        "ok": True,
        "status": "formal_module_review_validated",
        "evidence_class": (
            "formal_first_three" if scope == "first_three" else "formal_release_10_12"
        ),
        "formal_asset_evidence_eligible": True,
        "scope": scope,
        "pack_id": pack.manifest.pack_id,
        "pack_version": pack.manifest.version,
        "pack_manifest_sha256": pack_hash,
        "pack_license_sha256": pack_license_hash,
        "baseline_pack_manifest_sha256": _sha256(baseline.root / "pack.json"),
        "review_id": review["review_id"],
        "review_sha256": _sha256(review_file),
        "reviewer": {
            "reviewer_id": review["reviewer"]["reviewer_id"],
            "role": review["reviewer"]["role"],
        },
        "reviewed_at": review["reviewed_at"],
        "module_count": len(module_reports),
        "module_artifacts": module_reports,
        "source_set_sha256": _canonical_sha256(
            sorted(
                (
                    {
                        "module_id": item["module_id"],
                        "sha256": item["source_blend_sha256"],
                    }
                    for item in reviewed_modules
                ),
                key=lambda item: item["module_id"],
            )
        ),
        "checks": [
            "technical Module Pack contract",
            "exact source and reviewed artifact hashes",
            "Blender GLB generator",
            "anti-placeholder triangle floor",
            "reference ID and Connector compatibility",
            "independent author/reviewer",
            "all manual checklist items and scores >= 4",
            "non-functional use limitations",
        ],
        "cryptographic_signature": False,
        "limitations": [
            "human attestation is an audit record, not a cryptographic signature",
            "triangle floors reject trivial placeholders but do not prove visual quality",
            "this review does not prove manufacturing, structural, or safety readiness",
        ],
    }


def _prepare_inputs(
    pack_root: Path,
    source_root: Path,
    *,
    scope: str,
    baseline_pack_root: Path,
) -> tuple[ValidatedModulePack, dict[str, Path], ValidatedModulePack]:
    if scope not in {"first_three", "release_10_12"}:
        raise FormalModuleReviewError(
            [_finding("FORMAL_SCOPE_INVALID", f"Unsupported review scope: {scope}.")]
        )
    try:
        pack = validate_module_pack(
            pack_root,
            release=scope == "release_10_12",
        )
        baseline = validate_module_pack(baseline_pack_root, release=True)
    except ModulePackValidationError as exc:
        raise FormalModuleReviewError(
            [_finding("MODULE_PACK_INVALID", message) for message in exc.errors]
        ) from exc
    actual_ids = {module.manifest.module_id for module in pack.modules}
    baseline_ids = {module.manifest.module_id for module in baseline.modules}
    findings: list[dict[str, str]] = []
    if scope == "first_three" and actual_ids != FIRST_THREE_IDS:
        findings.append(
            _finding(
                "FIRST_THREE_MODULE_SET_INVALID",
                f"Expected {sorted(FIRST_THREE_IDS)}, found {sorted(actual_ids)}.",
            )
        )
    if scope == "release_10_12":
        if not 10 <= len(actual_ids) <= 12:
            findings.append(
                _finding(
                    "FORMAL_RELEASE_MODULE_COUNT_INVALID",
                    f"Formal release requires 10-12 modules, found {len(actual_ids)}.",
                )
            )
        if not baseline_ids.issubset(actual_ids):
            findings.append(
                _finding(
                    "FORMAL_RELEASE_BASELINE_MODULES_MISSING",
                    f"Missing stable baseline IDs: {sorted(baseline_ids - actual_ids)}.",
                )
            )
    source_directory = source_root.expanduser().resolve()
    expected_files = {f"{module_id}.blend" for module_id in actual_ids}
    actual_files = (
        {path.name for path in source_directory.glob("*.blend")}
        if source_directory.is_dir()
        else set()
    )
    if actual_files != expected_files:
        findings.append(
            _finding(
                "FORMAL_SOURCE_SET_MISMATCH",
                f"Expected {sorted(expected_files)}, found {sorted(actual_files)}.",
            )
        )
    sources: dict[str, Path] = {}
    for module_id in actual_ids:
        path = source_directory / f"{module_id}.blend"
        sources[module_id] = path
        if path.is_file() and not _has_blender_header(path):
            findings.append(
                _finding(
                    "FORMAL_SOURCE_HEADER_INVALID",
                    "Source does not have a Blender file header.",
                    module_id,
                )
            )
    if findings:
        raise FormalModuleReviewError(findings)
    return pack, sources, baseline


def _validate_review_schema(review: dict[str, Any]) -> None:
    schema = _read_json(SCHEMA_PATH)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = sorted(validator.iter_errors(review), key=lambda item: list(item.path))
    if errors:
        findings = []
        for error in errors:
            location = ".".join(str(part) for part in error.path) or "$"
            findings.append(
                _finding(
                    "FORMAL_REVIEW_SCHEMA_INVALID",
                    f"{location}: {error.message}",
                )
            )
        raise FormalModuleReviewError(findings)


def _validate_manual_approval(
    review: dict[str, Any], findings: list[dict[str, str]]
) -> None:
    author_id = str(review["author_id"])
    reviewer_id = str(review["reviewer"]["reviewer_id"])
    display_name = str(review["reviewer"]["display_name"])
    review_id = str(review["review_id"])
    if (
        _placeholder(author_id)
        or _placeholder(reviewer_id)
        or _placeholder(display_name)
        or "draft" in review_id.casefold()
    ):
        findings.append(
            _finding(
                "FORMAL_REVIEW_IDENTITY_INCOMPLETE",
                "Review ID, author, reviewer ID, and reviewer name must replace draft placeholders.",
            )
        )
    if author_id.casefold() == reviewer_id.casefold():
        findings.append(
            _finding(
                "FORMAL_REVIEW_NOT_INDEPENDENT",
                "The final reviewer must be different from the asset author.",
            )
        )
    if review["approval_status"] != "approved":
        findings.append(
            _finding("FORMAL_REVIEW_NOT_APPROVED", "approval_status must be approved.")
        )
    if review["reviewer_attestation"] != ATTESTATION:
        findings.append(
            _finding(
                "FORMAL_REVIEW_ATTESTATION_INVALID",
                "Reviewer attestation must use the published approval statement.",
            )
        )
    if set(review["limitations_acknowledged"]) != LIMITATIONS:
        findings.append(
            _finding(
                "FORMAL_REVIEW_LIMITATIONS_INCOMPLETE",
                "All non-functional and non-manufacturing limitations are required.",
            )
        )
    reviewed_at = datetime.fromisoformat(
        str(review["reviewed_at"]).replace("Z", "+00:00")
    )
    if reviewed_at.tzinfo is None:
        findings.append(
            _finding(
                "FORMAL_REVIEW_TIMEZONE_REQUIRED",
                "reviewed_at must include a timezone.",
            )
        )
    pack_review = review["pack_review"]
    for key, value in pack_review.items():
        if key == "notes":
            if _placeholder(str(value)):
                findings.append(
                    _finding(
                        "FORMAL_REVIEW_NOTES_INCOMPLETE",
                        "Pack review notes must replace the draft placeholder.",
                    )
                )
        elif value is not True:
            findings.append(
                _finding(
                    "FORMAL_PACK_CHECK_NOT_APPROVED",
                    f"pack_review.{key} must be true.",
                )
            )


def _validate_module_review(
    item: dict[str, Any], findings: list[dict[str, str]]
) -> None:
    module_id = str(item["module_id"])
    review = item["review"]
    for key, value in review.items():
        if key == "scores":
            for score_name, score in value.items():
                if int(score) < 4:
                    findings.append(
                        _finding(
                            "FORMAL_VISUAL_SCORE_BELOW_THRESHOLD",
                            f"{score_name} score={score}; required >=4.",
                            module_id,
                        )
                    )
        elif key == "notes":
            if _placeholder(str(value)):
                findings.append(
                    _finding(
                        "FORMAL_MODULE_NOTES_INCOMPLETE",
                        "Module notes must replace the draft placeholder.",
                        module_id,
                    )
                )
        elif value is not True:
            findings.append(
                _finding(
                    "FORMAL_MODULE_CHECK_NOT_APPROVED",
                    f"review.{key} must be true.",
                    module_id,
                )
            )


def _validate_pack_license(
    pack: ValidatedModulePack,
    raw_pack: dict[str, Any],
    findings: list[dict[str, str]],
) -> None:
    license_value = raw_pack.get("license")
    if not isinstance(license_value, dict):
        return
    spdx = str(license_value.get("spdx_expression", ""))
    path = pack.root / str(license_value.get("license_path", ""))
    texts: list[str] = []
    try:
        texts.append(path.read_text(encoding="utf-8"))
    except OSError:
        pass
    for entry in pack.manifest.modules:
        try:
            texts.append((pack.root / entry.license_path).read_text(encoding="utf-8"))
        except OSError:
            pass
    combined = f"{spdx}\n" + "\n".join(texts)
    combined = combined.casefold()
    markers = [marker for marker in FORBIDDEN_LICENSE_MARKERS if marker in combined]
    if markers:
        findings.append(
            _finding(
                "FORMAL_LICENSE_NOT_PROMOTABLE",
                f"Replace starter/reference license markers before approval: {markers}.",
            )
        )


def _validate_baseline_compatibility(
    pack: ValidatedModulePack,
    baseline: ValidatedModulePack,
    scope: str,
    findings: list[dict[str, str]],
) -> None:
    baseline_by_id = {
        module.manifest.module_id: module.manifest for module in baseline.modules
    }
    for module in pack.modules:
        module_id = module.manifest.module_id
        reference = baseline_by_id.get(module_id)
        if reference is None:
            continue
        if module.manifest.asset_id != reference.asset_id:
            findings.append(
                _finding(
                    "FORMAL_STABLE_ASSET_ID_CHANGED",
                    f"Expected asset_id={reference.asset_id}.",
                    module_id,
                )
            )
        actual_connectors = _canonical_connectors(module.manifest.connectors)
        reference_connectors = _canonical_connectors(reference.connectors)
        if actual_connectors != reference_connectors:
            findings.append(
                _finding(
                    "FORMAL_CONNECTOR_CONTRACT_CHANGED",
                    "Connector IDs, semantics, or transforms differ from the baseline.",
                    module_id,
                )
            )
    if scope == "first_three" and not FIRST_THREE_IDS.issubset(baseline_by_id):
        findings.append(
            _finding(
                "FORMAL_BASELINE_INCOMPLETE",
                "Baseline pack does not contain the three stable starter IDs.",
            )
        )


def _canonical_connectors(connectors: Any) -> str:
    values = sorted(
        (connector.model_dump(mode="json") for connector in connectors),
        key=lambda item: item["connector_id"],
    )
    return json.dumps(values, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _category_value(value: Any) -> str:
    return str(getattr(value, "value", value))


def _glb_generator(path: Path) -> str:
    try:
        document = _read_glb_json(path.read_bytes())
    except (OSError, ValueError):
        return "invalid_or_unknown"
    asset = document.get("asset")
    if not isinstance(asset, dict):
        return "unspecified"
    return str(asset.get("generator") or "unspecified")


def _has_blender_header(path: Path) -> bool:
    with path.open("rb") as handle:
        return handle.read(7) == b"BLENDER"


def _placeholder(value: str) -> bool:
    normalized = value.strip().casefold()
    return not normalized or "todo" in normalized or "pending" in normalized


def _finding(code: str, message: str, module_id: str | None = None) -> dict[str, str]:
    result = {"code": code, "message": message}
    if module_id is not None:
        result["module_id"] = module_id
    return result


def _read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise FormalModuleReviewError(
            [
                _finding(
                    "FORMAL_REVIEW_JSON_INVALID", f"Could not read {path.name}: {exc}."
                )
            ]
        ) from exc
    if not isinstance(value, dict):
        raise FormalModuleReviewError(
            [_finding("FORMAL_REVIEW_JSON_INVALID", f"{path.name} must be an object.")]
        )
    return value


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _canonical_sha256(value: Any) -> str:
    payload = json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Create or validate a FormalModuleReview@1 record."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    draft = subparsers.add_parser("draft")
    draft.add_argument("--pack-root", type=Path, required=True)
    draft.add_argument("--source-root", type=Path, required=True)
    draft.add_argument("--output", type=Path, required=True)
    draft.add_argument(
        "--scope", choices=("first_three", "release_10_12"), required=True
    )
    draft.add_argument("--baseline-pack-root", type=Path, default=DEFAULT_BASELINE_PACK)
    validate = subparsers.add_parser("validate")
    validate.add_argument("--pack-root", type=Path, required=True)
    validate.add_argument("--source-root", type=Path, required=True)
    validate.add_argument("--review", type=Path, required=True)
    validate.add_argument(
        "--baseline-pack-root", type=Path, default=DEFAULT_BASELINE_PACK
    )
    validate.add_argument("--report", type=Path)
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        if args.command == "draft":
            result = generate_review_draft(
                args.pack_root,
                args.source_root,
                args.output,
                scope=args.scope,
                baseline_pack_root=args.baseline_pack_root,
            )
        else:
            result = validate_formal_review(
                args.pack_root,
                args.source_root,
                args.review,
                baseline_pack_root=args.baseline_pack_root,
            )
            if args.report is not None:
                report = args.report.expanduser().resolve()
                if report.is_relative_to(COMMITTED_PACK_ROOT):
                    raise FormalModuleReviewError(
                        [
                            _finding(
                                "FORMAL_REPORT_COMMITTED_PACK_DENIED",
                                "Promotion reports must not be written into committed Module Packs.",
                            )
                        ]
                    )
                if report.exists() or report.is_symlink():
                    raise FormalModuleReviewError(
                        [
                            _finding(
                                "FORMAL_REPORT_EXISTS",
                                "Formal promotion report output already exists.",
                            )
                        ]
                    )
                report.parent.mkdir(parents=True, exist_ok=True)
                _write_json(report, result)
                result = {**result, "report": str(report)}
    except FormalModuleReviewError as exc:
        print(
            json.dumps(
                {
                    "ok": False,
                    "error": {
                        "code": exc.code,
                        "message": str(exc),
                        "findings": exc.findings,
                    },
                },
                ensure_ascii=False,
                indent=2,
            ),
            file=sys.stderr,
        )
        return 2
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
