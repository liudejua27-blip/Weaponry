#!/usr/bin/env python3
"""Create an isolated, non-promoted workspace for formal Blender asset production."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from concept_module_pack import ValidatedModulePack, validate_module_pack


ROOT = Path(__file__).resolve().parents[1]
COMMITTED_PACK_ROOT = ROOT / "assets" / "module-packs"
WORKSPACE_SCHEMA = "ForgeCADFormalAssetWorkspace@1"


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Copy a 10-12 module Blender candidate into an isolated workspace "
            "without granting final-license or reviewer approval."
        )
    )
    parser.add_argument("--pack-root", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    args = parser.parse_args()

    pack = validate_module_pack(args.pack_root, release=True)
    source_root = _validate_sources(args.source_root, pack)
    output_root = _validate_output(args.output_root, pack.root, source_root)
    _copy_workspace(pack, source_root, output_root)
    print(
        json.dumps(
            {
                "ok": True,
                "schema_version": WORKSPACE_SCHEMA,
                "status": "formal_asset_workspace_created",
                "module_count": len(pack.modules),
                "output": str(output_root),
                "promotion_granted": False,
                "final_license_declared": False,
                "independent_review_completed": False,
                "next_steps": [
                    "edit sources without changing stable Module/Asset/Connector contracts",
                    "re-export to a separate final Pack",
                    "record actual ownership and final-license terms",
                    "obtain an independent human review and validate FormalModuleReview",
                ],
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _validate_sources(source_root: Path, pack: ValidatedModulePack) -> Path:
    source = source_root.expanduser().resolve()
    expected = {f"{module.manifest.module_id}.blend" for module in pack.modules}
    actual = {path.name for path in source.glob("*.blend")} if source.is_dir() else set()
    if actual != expected:
        raise ValueError(
            f"source set mismatch: expected {sorted(expected)}, found {sorted(actual)}"
        )
    for path in source.glob("*.blend"):
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"source must be a regular non-symlink file: {path.name}")
        if path.read_bytes()[:7] != b"BLENDER":
            raise ValueError(f"source is not a Blender file: {path.name}")
    return source


def _validate_output(output_root: Path, pack_root: Path, source_root: Path) -> Path:
    output = output_root.expanduser()
    if not output.is_absolute():
        raise ValueError("--output-root must be an absolute path")
    resolved = output.resolve()
    if resolved.exists() or resolved.is_symlink():
        raise ValueError("--output-root must not already exist")
    if resolved.is_relative_to(COMMITTED_PACK_ROOT):
        raise ValueError("--output-root cannot be inside a committed Module Pack")
    if resolved.is_relative_to(pack_root) or resolved.is_relative_to(source_root):
        raise ValueError("--output-root must be outside the input Pack and source roots")
    return resolved


def _copy_workspace(
    pack: ValidatedModulePack, source_root: Path, output_root: Path
) -> None:
    if any(path.is_symlink() for path in pack.root.rglob("*")):
        raise ValueError("--pack-root cannot contain symlinks")
    output_root.parent.mkdir(parents=True, exist_ok=True)
    try:
        shutil.copytree(source_root, output_root / "sources")
        shutil.copytree(pack.root, output_root / "staging-pack")
        manifest = {
            "schema_version": WORKSPACE_SCHEMA,
            "status": "staged_not_promoted",
            "module_ids": [module.manifest.module_id for module in pack.modules],
            "pack_manifest_sha256": _sha256(pack.root / "pack.json"),
            "source_blend_sha256": {
                module.manifest.module_id: _sha256(
                    source_root / f"{module.manifest.module_id}.blend"
                )
                for module in pack.modules
            },
            "promotion_granted": False,
            "final_license_declared": False,
            "independent_review_completed": False,
        }
        (output_root / "INPUT_MANIFEST.json").write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        (output_root / "LICENSE_DECISION_REQUIRED.md").write_text(
            _license_template(), encoding="utf-8"
        )
        (output_root / "REVIEWER_BRIEF.md").write_text(
            _reviewer_brief(), encoding="utf-8"
        )
        (output_root / "NEXT_STEPS.md").write_text(
            _next_steps(), encoding="utf-8"
        )
    except Exception:
        shutil.rmtree(output_root, ignore_errors=True)
        raise


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _license_template() -> str:
    return """# Final license decision required

This workspace is **not** a final-art declaration. Do not replace the Pack or
Module licenses until the following statements are true and documented by the
rights holder.

- [ ] The right holder for every `.blend`, texture, reference and purchased asset is identified.
- [ ] Commissioned or contractor work has an assignment or product-distribution grant.
- [ ] Any third-party asset license permits modification, GLB export and product distribution.
- [ ] A real final SPDX expression or controlled `LicenseRef-*` identifier is selected.
- [ ] The Pack license and all ten Module licenses use the same actual rights decision.
- [ ] The final text does not contain authoring-starter, reference-assets or not-final markers.

Record the actual license decision, right holder, source records and approval
date here before editing `final-pack/LICENSES/PACK.txt` or any Module license.
This is an operational checklist, not legal advice.
"""


def _reviewer_brief() -> str:
    return """# Independent reviewer brief

The reviewer must not be the recorded asset author. Review every source,
thumbnail and exported Module after the final Pack has been re-exported.

The FormalModuleReview validator requires:

- distinct author and reviewer identities;
- all Pack and Module checklist entries set to true after actual inspection;
- silhouette, surface hierarchy, material readability, modular readability and
  thumbnail quality each scored at least 4/5;
- the exact published non-functional asset attestation;
- a final license decision and stable Module/Asset/Connector contracts.

Do not approve a module merely because the technical Pack validator passes.
The review remains limited to non-functional concept, game, film-prop and
display assets; it is not a manufacturing, structural or safety approval.
"""


def _next_steps() -> str:
    return """# Next steps

1. Edit `sources/` in Blender. Preserve all stable Module/Asset/Connector IDs,
   connector semantics and required material/UV/LOD contracts.
2. Re-export to a **new** `final-pack/` outside this workspace's `staging-pack/`.
3. After actual rights are confirmed, replace the final Pack and Module licenses.
4. Run `assets:formal-review-draft --scope release_10_12` against the new final
   Pack and these edited sources.
5. Give the generated review handoff to an independent reviewer, complete the
   original review JSON, then run `assets:formal-review-validate --report ...`.
6. Only a successful `formal_release_10_12` promotion report permits the
   formal-asset recovery drill and formal evidence claims.
"""


if __name__ == "__main__":
    raise SystemExit(main())
