#!/usr/bin/env python3
"""Release safety-scope gate for Wushen Forge.

This gate verifies that the product boundary is executable, not only documented:
fictional Unity game-art assets are allowed; real-world manufacturing drawings,
dimensions, material recipes, fabrication, and assembly instructions are not.
"""

from __future__ import annotations

import json
import os
import sqlite3
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest, ExportUnityRequest  # noqa: E402
from wushen_agent.providers.llm import build_fallback_weapon_spec  # noqa: E402
from wushen_agent.spec_validation import validate_weapon_design_spec  # noqa: E402


DISALLOWED_TERMS = [
    "real weapon blueprint",
    "manufacturing drawing",
    "manufacturing dimensions",
    "material recipe",
    "material formula",
    "fabrication process",
    "assembly instruction",
    "machining steps",
]

def main() -> int:
    checks: list[str] = []
    with tempfile.TemporaryDirectory(prefix="wushen_release_safety_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        migrations_dir = ROOT / "migrations"
        _force_mock_runtime()

        request = CreateWeaponRequest(
            client_request_id="release-safety-scope",
            text="赤金龙纹长剑，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产，不包含制造说明",
        )
        spec = build_fallback_weapon_spec(
            request,
            weapon_id="weapon_release_safety",
            display_name="赤金龙纹长剑",
            weapon_family="sword",
            planner_provider="mock",
        )
        validated_spec = validate_weapon_design_spec(spec, provider_id="mock")
        _assert(
            validated_spec["safety_boundary"]["real_world_manufacturing_details"] is False,
            "WeaponDesignSpec must force real_world_manufacturing_details=false",
        )
        _assert(
            validated_spec["unity_target"]["scale_contract"]["forbid_real_world_dimensions"] is True,
            "WeaponDesignSpec scale contract must forbid real-world dimensions",
        )
        negative_prompt = validated_spec["generation"]["negative_prompt"].lower()
        for term in ["manufacturing drawing", "dimensions", "material formula", "machining steps"]:
            _assert(term in negative_prompt, f"negative prompt missing safety exclusion: {term}")
        checks.append("weapon_spec_schema_and_prompt")

        schema = _read_json(ROOT / "packages" / "weapon-spec" / "schemas" / "weapon-design-spec.schema.json")
        _assert(
            schema["properties"]["safety_boundary"]["properties"]["real_world_manufacturing_details"]["const"] is False,
            "weapon-design-spec schema must lock manufacturing details to false",
        )
        _assert(
            schema["properties"]["unity_target"]["properties"]["scale_contract"]["properties"]["forbid_real_world_dimensions"]["const"] is True,
            "weapon-design-spec schema must lock real-world dimensions off",
        )
        checks.append("schema_contract")

        store = SQLiteAssetStore(library_root=library_root, migrations_dir=migrations_dir)
        created = store.create_weapon(request, idempotency_key="release-safety-create")
        weapon = store.get_weapon_detail(created.weapon_id)
        _assert(weapon.current_model_id, "mock create must produce a current model for export safety inspection")
        exported = store.export_unity(
            created.weapon_id,
            ExportUnityRequest(
                client_request_id="release-safety-export",
                model_id=weapon.current_model_id,
                include_source_spec=True,
                include_quality_reports=True,
            ),
            idempotency_key="release-safety-export",
        )
        checks.append("mock_asset_pipeline")

        conn = sqlite3.connect(library_root / "library.db")
        conn.row_factory = sqlite3.Row
        try:
            package = _package_row(conn, exported.job_id)
            package_path = library_root / package["object_path"]
            with zipfile.ZipFile(package_path) as archive:
                manifest_name = _single_zip_entry(archive, "manifest.json")
                readme_name = _single_zip_entry(archive, "README_WUSHEN.txt")
                spec_name = _single_zip_entry(archive, "weapon_spec.json")
                report_name = _single_zip_entry(archive, "model_quality_report.json")
                manifest = json.loads(archive.read(manifest_name).decode("utf-8"))
                readme = archive.read(readme_name).decode("utf-8")
                exported_spec = json.loads(archive.read(spec_name).decode("utf-8"))
                report = json.loads(archive.read(report_name).decode("utf-8"))
                _assert(all(not Path(name).is_absolute() and ".." not in Path(name).parts for name in archive.namelist()), "Unity export ZIP contains unsafe paths")
        finally:
            conn.close()

        safety = manifest.get("safety_boundary") or {}
        _assert(safety.get("asset_type") == "fictional_game_art", "Unity manifest must declare fictional_game_art")
        _assert(safety.get("non_manufacturing_asset") is True, "Unity manifest must declare non_manufacturing_asset=true")
        disallowed = {str(item).lower() for item in safety.get("disallowed") or []}
        for term in ["real weapon blueprint", "manufacturing dimensions", "material recipe", "fabrication process", "assembly instruction"]:
            _assert(term in disallowed, f"Unity manifest disallowed list missing: {term}")
        readme_lower = readme.lower()
        for term in ["no real-world weapon blueprints", "no manufacturing dimensions", "no material recipes", "no fabrication, assembly, or process instructions"]:
            _assert(term in readme_lower, f"Unity README missing safety statement: {term}")
        _assert(exported_spec["safety_boundary"]["real_world_manufacturing_details"] is False, "exported weapon_spec safety boundary drifted")
        _assert(_quality_report_has_safety_evidence(report), "model quality report missing non-manufacturing safety evidence")
        checks.append("unity_export_safety_manifest")

    _assert_docs_have_boundary()
    checks.append("docs_boundary")

    print(json.dumps({"ok": True, "checks": checks}, ensure_ascii=False, indent=2))
    return 0


def _force_mock_runtime() -> None:
    os.environ["WUSHEN_LLM_PROVIDER"] = "mock"
    os.environ["WUSHEN_IMAGE_PROVIDER"] = "mock"
    os.environ["WUSHEN_3D_PROVIDER"] = "mock"
    for name in ["WUSHEN_RUNTIME_WORKER", "WUSHEN_EXPORT_UNITY_WORKER", "WUSHEN_EXPORT_UNITY_ASYNC", "WUSHEN_GENERATE3D_WORKER"]:
        os.environ.pop(name, None)


def _package_row(conn: sqlite3.Connection, job_id: str) -> sqlite3.Row:
    row = conn.execute(
        """
        SELECT object_path
        FROM asset_files
        WHERE job_id = ? AND role = 'unity_export_package'
        ORDER BY created_at DESC
        LIMIT 1
        """,
        (job_id,),
    ).fetchone()
    _assert(row is not None, "export job did not write unity_export_package")
    return row


def _single_zip_entry(archive: zipfile.ZipFile, suffix: str) -> str:
    matches = [name for name in archive.namelist() if name.endswith("/" + suffix)]
    _assert(len(matches) == 1, f"expected one ZIP entry ending with {suffix}, got {matches}")
    return matches[0]


def _quality_report_has_safety_evidence(report: dict[str, Any]) -> bool:
    checks = report.get("checks")
    if not isinstance(checks, list):
        return False
    for item in checks:
        if not isinstance(item, dict):
            continue
        evidence = item.get("evidence")
        if isinstance(evidence, dict) and evidence.get("non_manufacturing_asset") is True:
            return True
    return False


def _assert_docs_have_boundary() -> None:
    requirements = {
        "README.md": ["虚构游戏美术资产", "非制造说明", "不输出可用于现实制造武器的精确图纸"],
        "docs/DESIGN.md": ["虚构游戏美术资产", "项目不生成现实可制造武器", "制造尺寸", "材料配方", "加工流程"],
        "docs/API.md": ["fictional Unity game-art boundary", "manufacturing drawings"],
        "docs/UNITY_IMPORT_SMOKE.md": ["fictional game-art assets", "manufacturing dimensions"],
    }
    for rel_path, phrases in requirements.items():
        text = (ROOT / rel_path).read_text(encoding="utf-8")
        missing = [phrase for phrase in phrases if phrase not in text]
        _assert(not missing, f"{rel_path} missing safety boundary phrase(s): {missing}")


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _assert(condition: Any, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AssertionError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, ensure_ascii=False, indent=2), file=sys.stderr)
        raise SystemExit(1)
