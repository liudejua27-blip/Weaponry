#!/usr/bin/env python3
"""Render a safe Markdown checklist for an independent FormalModuleReview reviewer."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

from concept_module_pack import validate_module_pack
from formal_module_review import _validate_review_schema


ROOT = Path(__file__).resolve().parents[1]
COMMITTED_PACK_ROOT = ROOT / "assets" / "module-packs"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Create a Markdown handoff from an existing FormalModuleReview draft."
    )
    parser.add_argument("--pack-root", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--review", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    pack_root = args.pack_root.expanduser().resolve()
    source_root = args.source_root.expanduser().resolve()
    review_path = args.review.expanduser().resolve()
    output = args.output.expanduser().resolve()
    _check_output(output)

    pack = validate_module_pack(pack_root)
    review = _read_json(review_path)
    _validate_review_schema(review)
    module_records = _verify_integrity(pack, source_root, review)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(_render_handoff(pack, review, module_records), encoding="utf-8")
    print(
        json.dumps(
            {
                "ok": True,
                "status": "review_handoff_created",
                "scope": review["scope"],
                "module_count": len(module_records),
                "output": str(output),
                "absolute_paths_excluded_from_handoff": True,
                "approval_granted": False,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _check_output(output: Path) -> None:
    if output.suffix != ".md":
        raise ValueError("--output must be a Markdown (.md) path")
    if output.exists() or output.is_symlink():
        raise ValueError("--output must not overwrite an existing file")
    if output.is_relative_to(COMMITTED_PACK_ROOT):
        raise ValueError("--output cannot be written into a committed Module Pack")


def _read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError("--review must be readable JSON") from exc
    if not isinstance(value, dict):
        raise ValueError("--review must contain a JSON object")
    return value


def _verify_integrity(
    pack: Any, source_root: Path, review: dict[str, Any]
) -> list[dict[str, str]]:
    if review["pack_id"] != pack.manifest.pack_id:
        raise ValueError("review pack_id does not match the Pack")
    if review["pack_version"] != pack.manifest.version:
        raise ValueError("review pack_version does not match the Pack")
    if review["pack_manifest_sha256"] != _sha256(pack.root / "pack.json"):
        raise ValueError("pack.json changed after the review draft was generated")

    entries = {entry.module_id: entry for entry in pack.manifest.modules}
    records_by_id = {str(record["module_id"]): record for record in review["modules"]}
    if set(records_by_id) != set(entries):
        raise ValueError("review module set does not match the Pack")

    result = []
    for module in pack.modules:
        module_id = module.manifest.module_id
        record = records_by_id[module_id]
        entry = entries[module_id]
        source = source_root / f"{module_id}.blend"
        expected = {
            "source_blend_sha256": _sha256(source),
            "module_manifest_sha256": _sha256(pack.root / entry.manifest_path),
            "glb_sha256": _sha256(module.glb_path),
            "thumbnail_sha256": _sha256(pack.root / entry.thumbnail_path),
            "license_sha256": _sha256(pack.root / entry.license_path),
        }
        if record["source_blend_file"] != source.name:
            raise ValueError(f"{module_id}: review source file does not match")
        stale = [field for field, value in expected.items() if record[field] != value]
        if stale:
            raise ValueError(
                f"{module_id}: reviewed artifacts changed: {', '.join(stale)}"
            )
        result.append(
            {
                "module_id": module_id,
                "category": module.manifest.category,
                "triangle_count": str(module.manifest.triangle_count),
                "source_blend_file": source.name,
                "thumbnail_path": entry.thumbnail_path,
                "glb_sha256": expected["glb_sha256"],
            }
        )
    return result


def _render_handoff(
    pack: Any, review: dict[str, Any], modules: list[dict[str, str]]
) -> str:
    lines = [
        "# ForgeCAD Formal Module Review Handoff",
        "",
        "> Status: **DRAFT ONLY — this file cannot grant approval or promotion.**",
        "",
        "This checklist is a reviewer aid generated from an integrity-checked `FormalModuleReview@1` draft. "
        "The independent reviewer must still update the original review JSON, replace all draft placeholders, "
        "and run `assets:formal-review-validate` to produce a promotion report.",
        "",
        "## Reviewed artifact set",
        "",
        f"- Scope: `{review['scope']}`",
        f"- Pack: `{pack.manifest.pack_id}` version `{pack.manifest.version}`",
        f"- Modules: {len(modules)}",
        f"- Current draft approval status: `{review['approval_status']}`",
        "- Integrity: source `.blend`, manifests, GLB, thumbnail, and license hashes match the draft.",
        "- Asset boundary: non-functional visual concept/game/film-prop asset only; not manufacturing documentation.",
        "",
        "## Independent reviewer actions",
        "",
        "1. Confirm you are not the asset author and inspect the editable source plus thumbnail for every module.",
        "2. Confirm the final asset license and ownership before replacing any starter license marker.",
        "3. In the original JSON draft, replace identity placeholders; set each required checklist item after inspection; add notes; and score all five visual criteria from 1 to 5.",
        "4. Set `approval_status=approved` only after all items pass and use the published reviewer attestation exactly.",
        "5. Run `npm run assets:formal-review-validate -- ... --scope release_10_12` equivalent validation. A successful Markdown handoff is not a successful formal review.",
        "",
        "## Module checklist",
        "",
    ]
    for module in modules:
        lines.extend(
            [
                f"### `{module['module_id']}`",
                "",
                f"- Category / LOD0 triangles: `{module['category']}` / {module['triangle_count']}",
                f"- Source file: `{module['source_blend_file']}`",
                f"- Thumbnail within Pack: `{module['thumbnail_path']}`",
                f"- GLB SHA-256: `{module['glb_sha256']}`",
                "- [ ] silhouette distinct",
                "- [ ] surface hierarchy reviewed",
                "- [ ] material partition reviewed",
                "- [ ] UV0 reviewed",
                "- [ ] thumbnail reviewed",
                "- [ ] Connector contract reviewed",
                "- [ ] transforms and modifiers reviewed",
                "- [ ] non-functional visual-only boundary confirmed",
                "- Scores (1–5): silhouette ___; surface hierarchy ___; material readability ___; modular readability ___; thumbnail quality ___",
                "- Reviewer notes: ________________________________________________",
                "",
            ]
        )
    lines.extend(
        [
            "## Required promotion blockers",
            "",
            "- Final Pack and Module licenses must not contain starter/reference/not-final markers.",
            "- Reviewer identity must differ from author identity; draft placeholder text is invalid.",
            "- All checkboxes in the JSON review must be true and all five scores must be at least 4.",
            "- Source/export hashes and stable Module/Asset/Connector contracts must remain unchanged after review.",
            "",
        ]
    )
    return "\n".join(lines)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError) as exc:
        print(
            json.dumps(
                {"ok": False, "status": "review_handoff_failed", "message": str(exc)}
            )
        )
        raise SystemExit(1) from exc
