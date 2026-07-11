#!/usr/bin/env python3
from __future__ import annotations

import copy
import hashlib
import json
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

from formal_module_review import (
    ATTESTATION,
    FIRST_THREE_IDS,
    FormalModuleReviewError,
    generate_review_draft,
    validate_formal_review,
)


ROOT = Path(__file__).resolve().parents[1]
REFERENCE_PACK = ROOT / "assets" / "module-packs" / "weapon-concept-v1-reference"
MODULE_ORDER = (
    "module_core_shell_01",
    "module_front_shell_01",
    "module_front_shell_02",
)
TRIANGLES = {
    "module_core_shell_01": 1000,
    "module_front_shell_01": 500,
    "module_front_shell_02": 500,
}


def main() -> int:
    with tempfile.TemporaryDirectory(
        prefix="forgecad_formal_module_review_"
    ) as temporary_directory:
        temporary_root = Path(temporary_directory)
        candidate_pack = temporary_root / "candidate-pack"
        candidate_sources = temporary_root / "candidate-sources"
        _create_candidate_pack(candidate_pack, candidate_sources, formal_like=True)

        draft_path = temporary_root / "formal-review.json"
        draft_result = generate_review_draft(
            candidate_pack,
            candidate_sources,
            draft_path,
            scope="first_three",
        )
        _assert(draft_result["module_count"] == 3, "review draft module count mismatch")
        review = json.loads(draft_path.read_text(encoding="utf-8"))
        _approve_review(review)
        _write_json(draft_path, review)

        validated = validate_formal_review(
            candidate_pack,
            candidate_sources,
            draft_path,
        )
        _assert(
            validated["formal_asset_evidence_eligible"] is True,
            "approved candidate did not validate",
        )
        _assert(
            {item["module_id"] for item in validated["module_artifacts"]}
            == FIRST_THREE_IDS,
            "promotion report module set mismatch",
        )

        report_path = temporary_root / "formal-promotion-report.json"
        completed = _run_cli(
            "validate",
            "--pack-root",
            str(candidate_pack),
            "--source-root",
            str(candidate_sources),
            "--review",
            str(draft_path),
            "--report",
            str(report_path),
        )
        _assert(
            completed.returncode == 0, f"formal review CLI failed: {completed.stderr}"
        )
        report_text = report_path.read_text(encoding="utf-8")
        report = json.loads(report_text)
        _assert(
            report["schema_version"] == "ForgeCADFormalModulePromotionReport@1",
            "promotion report schema mismatch",
        )
        _assert(
            str(temporary_root) not in report_text,
            "promotion report exposed absolute workspace paths",
        )
        overwrite = _run_cli(
            "validate",
            "--pack-root",
            str(candidate_pack),
            "--source-root",
            str(candidate_sources),
            "--review",
            str(draft_path),
            "--report",
            str(report_path),
        )
        _assert(overwrite.returncode == 2, "existing promotion report was overwritten")
        _assert(
            "FORMAL_REPORT_EXISTS" in overwrite.stderr,
            "report overwrite failure code mismatch",
        )
        committed_report = _run_cli(
            "validate",
            "--pack-root",
            str(candidate_pack),
            "--source-root",
            str(candidate_sources),
            "--review",
            str(draft_path),
            "--report",
            str(REFERENCE_PACK / "formal-promotion.json"),
        )
        _assert(
            committed_report.returncode == 2
            and "FORMAL_REPORT_COMMITTED_PACK_DENIED" in committed_report.stderr,
            "promotion report write into committed Pack was accepted",
        )

        reference_pack = temporary_root / "reference-subset"
        reference_sources = temporary_root / "reference-sources"
        _create_candidate_pack(reference_pack, reference_sources, formal_like=False)
        reference_review = temporary_root / "reference-review.json"
        generate_review_draft(
            reference_pack,
            reference_sources,
            reference_review,
            scope="first_three",
        )
        reference_value = json.loads(reference_review.read_text(encoding="utf-8"))
        _approve_review(reference_value)
        _write_json(reference_review, reference_value)
        reference_codes = _error_codes(
            lambda: validate_formal_review(
                reference_pack,
                reference_sources,
                reference_review,
            )
        )
        _assert(
            {
                "FORMAL_DCC_GENERATOR_REQUIRED",
                "FORMAL_TRIANGLE_FLOOR_NOT_MET",
                "FORMAL_LICENSE_NOT_PROMOTABLE",
            }.issubset(reference_codes),
            f"reference pack formal guards missing: {reference_codes}",
        )

        release_sources = temporary_root / "release-reference-sources"
        release_ids = {
            path.name
            for path in (REFERENCE_PACK / "modules").iterdir()
            if path.is_dir()
        }
        _write_sources(release_sources, sorted(release_ids))
        release_review = temporary_root / "release-reference-review.json"
        release_draft = generate_review_draft(
            REFERENCE_PACK,
            release_sources,
            release_review,
            scope="release_10_12",
        )
        _assert(release_draft["module_count"] == 10, "release scope count mismatch")
        release_value = json.loads(release_review.read_text(encoding="utf-8"))
        _approve_review(release_value)
        _write_json(release_review, release_value)
        release_codes = _error_codes(
            lambda: validate_formal_review(
                REFERENCE_PACK,
                release_sources,
                release_review,
            )
        )
        _assert(
            {
                "FORMAL_DCC_GENERATOR_REQUIRED",
                "FORMAL_TRIANGLE_FLOOR_NOT_MET",
                "FORMAL_LICENSE_NOT_PROMOTABLE",
            }.issubset(release_codes),
            "release_10_12 did not apply formal asset guards",
        )

        short_release_pack = temporary_root / "short-release-pack"
        shutil.copytree(REFERENCE_PACK, short_release_pack)
        short_pack_path = short_release_pack / "pack.json"
        short_pack = json.loads(short_pack_path.read_text(encoding="utf-8"))
        removed_id = "module_front_shell_02"
        short_pack["modules"] = [
            item for item in short_pack["modules"] if item["module_id"] != removed_id
        ]
        _write_json(short_pack_path, short_pack)
        shutil.rmtree(short_release_pack / "modules" / removed_id)
        short_sources = temporary_root / "short-release-sources"
        _write_sources(short_sources, sorted(release_ids - {removed_id}))
        short_release_codes = _error_codes(
            lambda: generate_review_draft(
                short_release_pack,
                short_sources,
                temporary_root / "short-release-review.json",
                scope="release_10_12",
            )
        )
        _assert(
            {
                "FORMAL_RELEASE_MODULE_COUNT_INVALID",
                "FORMAL_RELEASE_BASELINE_MODULES_MISSING",
            }.issubset(short_release_codes),
            "formal release accepted fewer than 10 stable modules",
        )

        identity_review = temporary_root / "identity-review.json"
        identity_value = copy.deepcopy(review)
        identity_value["reviewer"]["reviewer_id"] = identity_value["author_id"]
        _write_json(identity_review, identity_value)
        identity_codes = _error_codes(
            lambda: validate_formal_review(
                candidate_pack,
                candidate_sources,
                identity_review,
            )
        )
        _assert(
            "FORMAL_REVIEW_NOT_INDEPENDENT" in identity_codes,
            "author self-review was accepted",
        )

        quality_review = temporary_root / "quality-review.json"
        quality_value = copy.deepcopy(review)
        quality_value["modules"][0]["review"]["silhouette_distinct"] = False
        quality_value["modules"][1]["review"]["scores"]["silhouette"] = 3
        _write_json(quality_review, quality_value)
        quality_codes = _error_codes(
            lambda: validate_formal_review(
                candidate_pack,
                candidate_sources,
                quality_review,
            )
        )
        _assert(
            {
                "FORMAL_MODULE_CHECK_NOT_APPROVED",
                "FORMAL_VISUAL_SCORE_BELOW_THRESHOLD",
            }.issubset(quality_codes),
            "manual quality failures were accepted",
        )

        schema_review = temporary_root / "schema-review.json"
        schema_value = copy.deepcopy(review)
        schema_value["unexpected"] = True
        _write_json(schema_review, schema_value)
        schema_codes = _error_codes(
            lambda: validate_formal_review(
                candidate_pack,
                candidate_sources,
                schema_review,
            )
        )
        _assert(
            "FORMAL_REVIEW_SCHEMA_INVALID" in schema_codes,
            "unknown review field was accepted",
        )

        tampered_sources = temporary_root / "tampered-sources"
        shutil.copytree(candidate_sources, tampered_sources)
        with (tampered_sources / "module_front_shell_01.blend").open("ab") as handle:
            handle.write(b"tamper")
        source_codes = _error_codes(
            lambda: validate_formal_review(
                candidate_pack,
                tampered_sources,
                draft_path,
            )
        )
        _assert(
            "REVIEW_ARTIFACT_HASH_MISMATCH" in source_codes,
            "source tamper was accepted",
        )

        tampered_pack = temporary_root / "tampered-pack"
        shutil.copytree(candidate_pack, tampered_pack)
        with (tampered_pack / "modules" / "module_front_shell_01" / "model.glb").open(
            "ab"
        ) as handle:
            handle.write(b"tamper")
        glb_codes = _error_codes(
            lambda: validate_formal_review(
                tampered_pack,
                candidate_sources,
                draft_path,
            )
        )
        _assert("MODULE_PACK_INVALID" in glb_codes, "GLB tamper was accepted")

        module_license_pack = temporary_root / "module-license-pack"
        shutil.copytree(candidate_pack, module_license_pack)
        (
            module_license_pack / "modules" / "module_front_shell_01" / "LICENSE.txt"
        ).write_text(
            "SPDX-License-Identifier: LicenseRef-ForgeCAD-Authoring-Starter\n",
            encoding="utf-8",
        )
        license_codes = _error_codes(
            lambda: validate_formal_review(
                module_license_pack,
                candidate_sources,
                draft_path,
            )
        )
        _assert(
            {
                "FORMAL_LICENSE_NOT_PROMOTABLE",
                "REVIEW_ARTIFACT_HASH_MISMATCH",
            }.issubset(license_codes),
            "module-level starter license was accepted",
        )

        pack_license_pack = temporary_root / "pack-license-tamper"
        shutil.copytree(candidate_pack, pack_license_pack)
        (pack_license_pack / "LICENSES" / "PACK.txt").write_text(
            "SPDX-License-Identifier: LicenseRef-ForgeCAD-Changed-Final-Art\n",
            encoding="utf-8",
        )
        pack_license_codes = _error_codes(
            lambda: validate_formal_review(
                pack_license_pack,
                candidate_sources,
                draft_path,
            )
        )
        _assert(
            "REVIEW_ARTIFACT_HASH_MISMATCH" in pack_license_codes,
            "approved Pack license content could be changed",
        )

        thumbnail_pack = temporary_root / "thumbnail-tamper-pack"
        shutil.copytree(candidate_pack, thumbnail_pack)
        with (
            thumbnail_pack / "modules" / "module_front_shell_02" / "thumbnail.png"
        ).open("ab") as handle:
            handle.write(b"tamper")
        thumbnail_codes = _error_codes(
            lambda: validate_formal_review(
                thumbnail_pack,
                candidate_sources,
                draft_path,
            )
        )
        _assert(
            "REVIEW_ARTIFACT_HASH_MISMATCH" in thumbnail_codes,
            "thumbnail tamper was accepted",
        )

        connector_pack = temporary_root / "connector-drift-pack"
        shutil.copytree(candidate_pack, connector_pack)
        connector_manifest = (
            connector_pack / "modules" / "module_front_shell_01" / "module.json"
        )
        connector_value = json.loads(connector_manifest.read_text(encoding="utf-8"))
        connector_value["connectors"][0]["transform"]["position"][0] = 4
        _write_json(connector_manifest, connector_value)
        connector_codes = _error_codes(
            lambda: validate_formal_review(
                connector_pack,
                candidate_sources,
                draft_path,
            )
        )
        _assert(
            {
                "FORMAL_CONNECTOR_CONTRACT_CHANGED",
                "REVIEW_ARTIFACT_HASH_MISMATCH",
            }.issubset(connector_codes),
            "Connector drift was accepted",
        )

        committed_output_codes = _error_codes(
            lambda: generate_review_draft(
                candidate_pack,
                candidate_sources,
                REFERENCE_PACK / "formal-review.json",
                scope="first_three",
            )
        )
        _assert(
            "REVIEW_DRAFT_COMMITTED_PACK_DENIED" in committed_output_codes,
            "draft write into committed Pack was accepted",
        )

        print(
            json.dumps(
                {
                    "ok": True,
                    "schema_version": "FormalModuleReview@1",
                    "synthetic_positive_fixture_only": True,
                    "module_count": validated["module_count"],
                    "triangle_counts": {
                        item["module_id"]: item["triangle_count"]
                        for item in validated["module_artifacts"]
                    },
                    "reference_guard_codes": sorted(reference_codes),
                    "release_scope_guard_codes": sorted(release_codes),
                    "release_minimum_count_guard": True,
                    "identity_guard": True,
                    "quality_guard": True,
                    "schema_guard": True,
                    "source_tamper_guard": True,
                    "glb_tamper_guard": True,
                    "module_license_guard": True,
                    "pack_license_hash_guard": True,
                    "module_manifest_hash_guard": True,
                    "thumbnail_hash_guard": True,
                    "connector_drift_guard": True,
                    "report_overwrite_guard": True,
                    "committed_report_write_guard": True,
                    "absolute_paths_excluded": True,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _create_candidate_pack(
    pack_root: Path, source_root: Path, *, formal_like: bool
) -> None:
    shutil.copytree(REFERENCE_PACK, pack_root)
    pack_path = pack_root / "pack.json"
    pack = json.loads(pack_path.read_text(encoding="utf-8"))
    pack["modules"] = [
        item for item in pack["modules"] if item["module_id"] in FIRST_THREE_IDS
    ]
    if formal_like:
        pack["name"] = "Reviewed Blender asset candidate"
        pack["version"] = "1.0.0"
        pack["description"] = (
            "Non-functional visual asset candidate used only to exercise the formal review gate."
        )
        pack["license"] = {
            "spdx_expression": "LicenseRef-ForgeCAD-Internal-Final-Art",
            "license_path": "LICENSES/PACK.txt",
        }
        license_text = (
            "SPDX-License-Identifier: LicenseRef-ForgeCAD-Internal-Final-Art\n"
            "Non-functional visual asset review fixture.\n"
        )
        (pack_root / "LICENSES" / "PACK.txt").write_text(license_text, encoding="utf-8")
    else:
        license_text = (pack_root / "LICENSES" / "PACK.txt").read_text(encoding="utf-8")
    _write_json(pack_path, pack)
    selected = set(FIRST_THREE_IDS)
    for module_root in (pack_root / "modules").iterdir():
        if module_root.name not in selected:
            shutil.rmtree(module_root)
            continue
        if formal_like:
            glb_path = module_root / "model.glb"
            _rewrite_glb(glb_path, TRIANGLES[module_root.name])
            manifest_path = module_root / "module.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["sha256"] = hashlib.sha256(glb_path.read_bytes()).hexdigest()
            manifest["triangle_count"] = TRIANGLES[module_root.name]
            # A DCC export may reorder Connectors and serialize integral values as
            # floats (including negative zero). Those are representation changes,
            # not Connector contract drift.
            manifest["connectors"].reverse()
            for connector in manifest["connectors"]:
                position = connector["transform"]["position"]
                connector["transform"]["position"] = [float(value) for value in position]
                rotation = connector["transform"]["rotation"]
                connector["transform"]["rotation"] = [
                    -0.0 if index == 1 and value == 0 else float(value)
                    for index, value in enumerate(rotation)
                ]
            _write_json(manifest_path, manifest)
            (module_root / "LICENSE.txt").write_text(license_text, encoding="utf-8")
    _write_sources(source_root, MODULE_ORDER)


def _write_sources(source_root: Path, module_ids) -> None:
    source_root.mkdir(parents=True)
    for index, module_id in enumerate(module_ids, start=1):
        (source_root / f"{module_id}.blend").write_bytes(
            b"BLENDER" + bytes([index % 251 + 1]) * (128 + index)
        )


def _rewrite_glb(path: Path, target_triangles: int) -> None:
    payload = path.read_bytes()
    magic, version, _ = struct.unpack_from("<4sII", payload, 0)
    _assert(magic == b"glTF" and version == 2, "fixture GLB header mismatch")
    offset = 12
    chunks: list[tuple[int, bytes]] = []
    document = None
    binary_chunks: list[tuple[int, bytes]] = []
    while offset < len(payload):
        length, kind = struct.unpack_from("<II", payload, offset)
        offset += 8
        value = payload[offset : offset + length]
        offset += length
        if kind == 0x4E4F534A:
            document = json.loads(value.rstrip(b" \x00"))
        else:
            binary_chunks.append((kind, value))
    _assert(isinstance(document, dict), "fixture GLB JSON missing")
    document["asset"]["generator"] = "Khronos glTF Blender I/O v4.2"
    index_accessors = [
        primitive["indices"]
        for mesh in document["meshes"]
        for primitive in mesh["primitives"]
    ]
    _assert(target_triangles >= len(index_accessors), "triangle target too small")
    for accessor_index in index_accessors:
        document["accessors"][accessor_index]["count"] = 3
    document["accessors"][index_accessors[0]]["count"] = 3 * (
        target_triangles - len(index_accessors) + 1
    )
    json_chunk = json.dumps(document, separators=(",", ":")).encode("utf-8")
    json_chunk += b" " * ((4 - len(json_chunk) % 4) % 4)
    chunks.append((0x4E4F534A, json_chunk))
    chunks.extend(binary_chunks)
    total = 12 + sum(8 + len(value) for _, value in chunks)
    output = bytearray(struct.pack("<4sII", b"glTF", 2, total))
    for kind, value in chunks:
        output.extend(struct.pack("<II", len(value), kind))
        output.extend(value)
    path.write_bytes(bytes(output))


def _approve_review(review: dict) -> None:
    review["review_id"] = "fmr_approved_candidate_20260711"
    review["author_id"] = "artist_alpha"
    review["reviewer"] = {
        "reviewer_id": "reviewer_beta",
        "display_name": "Reviewer Beta",
        "role": "art_director",
    }
    review["reviewed_at"] = "2026-07-11T01:00:00+08:00"
    review["approval_status"] = "approved"
    review["reviewer_attestation"] = ATTESTATION
    for key in review["pack_review"]:
        review["pack_review"][key] = (
            "Independent synthetic gate review; not a real asset approval."
            if key == "notes"
            else True
        )
    for item in review["modules"]:
        module_review = item["review"]
        for key in module_review:
            if key == "scores":
                module_review[key] = {name: 4 for name in module_review[key]}
            elif key == "notes":
                module_review[key] = (
                    "Synthetic gate review only; no real asset quality claim."
                )
            else:
                module_review[key] = True


def _error_codes(action) -> set[str]:
    try:
        action()
    except FormalModuleReviewError as exc:
        return {item["code"] for item in exc.findings}
    raise AssertionError("expected formal review failure")


def _run_cli(*arguments: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "formal_module_review.py"), *arguments],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def _write_json(path: Path, value) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
