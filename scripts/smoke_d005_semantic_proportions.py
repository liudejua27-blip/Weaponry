#!/usr/bin/env python3
"""D005 gate: four-domain visual proportion recipes stay evidence-bound."""

from __future__ import annotations

import json
import tempfile
from pathlib import Path

from jsonschema import Draft202012Validator

from forgecad_agent.application.active_design import ActiveDesignService
from forgecad_agent.application.agent_asset_editing import AgentAssetEditingService, AgentAssetError
from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import (
    CommitAgentBlockoutRequest,
    NavigateActiveDesignRequest,
    ProposeAgentAssetChangeSetRequest,
    SegmentAgentBlockoutRequest,
)
from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.geometry_worker import build_blockout, segment_blockout
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner
from forgecad_agent.application.semantic_proportions import (
    SEMANTIC_PROPORTION_RECIPES,
    STYLE_TOKENS,
    part_id_for_role_selector,
    recipes_for_domain,
)
from forgecad_agent.application.shape_program_runtime import runtime_operation_names
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork


ROOT = Path(__file__).resolve().parents[1]
NOW = "2026-07-15T00:00:00+00:00"
BRIEFS = (
    "设计一个未来概念道具",
    "设计一辆冰原探索车",
    "设计一架垂直起降飞行器",
    "设计一台三关节机械臂",
)


def _seed_project(factory: SQLiteConnectionFactory, project_id: str) -> None:
    connection = factory.connect()
    try:
        connection.execute(
            """INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name,
               profile_json, profile_sha256, status, created_at, updated_at)
               VALUES ('profile_d005', 'weapon_concept', 'DesignDomainProfile@1', 'pack_vehicle_concept',
               'D005 fixture', '{}', ?, 'active', ?, ?)""",
            ("0" * 64, NOW, NOW),
        )
        connection.execute(
            """INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at)
               VALUES (?, 'profile_d005', 'weapon_concept', 'D005 proportions', 'active', NULL, ?, ?)""",
            (project_id, NOW, NOW),
        )
        connection.commit()
    finally:
        connection.close()


def _proposal(client_id: str, part_id: str, path: str, value: float) -> ProposeAgentAssetChangeSetRequest:
    return ProposeAgentAssetChangeSetRequest.model_validate({
        "client_request_id": client_id,
        "summary": "D005 语义比例配方",
        "operations": [{
            "operation_id": f"op_{client_id.replace('-', '_')}",
            "op": "set_part_parameter",
            "part_id": part_id,
            "path": path,
            "value": value,
        }],
    })


def _expect_error(action: object, code: str) -> None:
    try:
        assert callable(action)
        action()
    except AgentAssetError as exc:
        assert exc.code == code, (exc.code, code)
        return
    raise AssertionError(f"expected {code}")


def _contract_and_four_domain_gate() -> None:
    token_schema = json.loads((ROOT / "packages/concept-spec/schemas/mechanical-style-token.schema.json").read_text())
    recipe_schema = json.loads((ROOT / "packages/concept-spec/schemas/domain-semantic-proportion-recipe.schema.json").read_text())
    token_validator = Draft202012Validator(token_schema)
    recipe_validator = Draft202012Validator(recipe_schema)
    assert len(STYLE_TOKENS) == 4
    assert len(SEMANTIC_PROPORTION_RECIPES) == 16
    for token in STYLE_TOKENS:
        assert not list(token_validator.iter_errors(token.model_dump(mode="json")))
        assert token.visual_only and len(token.allowed_domains) == 4
    for recipe in SEMANTIC_PROPORTION_RECIPES:
        assert not list(recipe_validator.iter_errors(recipe.model_dump(mode="json")))
        assert recipe.non_functional_only
        assert all(item.path.startswith("transform.scale.") and item.step_delta in {-1, 1} for item in recipe.adjustments)

    planner = DeterministicMechanicalPlanner()
    seen_domains: set[str] = set()
    for brief in BRIEFS:
        pack = domain_pack_for_message(brief)
        plan = planner.plan_complete_concept(brief=brief, pack=pack, project_id="prj_d005_four_domain")
        direction_id = plan.directions[0].direction_id
        parts = segment_blockout(plan, direction_id)
        built = build_blockout(plan, direction_id)
        assert set(built.compile_readback.operation_names).issubset(set(runtime_operation_names()))
        raw_facts = [item.model_dump(mode="json") for item in built.compile_readback.surface_provenance]
        facts_by_role = {
            str(item["part_role"]): item
            for item in raw_facts
            if item.get("texture_ready") is True
        }
        recipes = list(recipes_for_domain(pack.pack_id))
        assert len(recipes) == 4
        seen_domains.add(pack.pack_id)
        for recipe in recipes:
            adjustment = recipe.adjustments[0]
            part_id = part_id_for_role_selector(parts, built.assembly_graph, adjustment.role_selector)
            part = next(item for item in parts if item["part_id"] == part_id)
            binding = next(item for item in part["editable_parameter_bindings"] if item["path"] == adjustment.path)
            assert binding["unit"] == "ratio" and binding["min"] == 0.6 and binding["max"] == 1.4 and binding["step"] == 0.1
            fact = facts_by_role[part["role"]]
            assert fact["source_operation_ids"] and fact["material_zone_id"] in part["material_zone_ids"]
    assert len(seen_domains) == 4


def _service_gate() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad-d005-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        project_id = "prj_d005_semantic_proportions"
        _seed_project(factory, project_id)
        planner = DeterministicMechanicalPlanner()
        plan = planner.plan_complete_concept(
            brief="设计一辆冰原探索车",
            pack=domain_pack_for_message("设计一辆冰原探索车"),
            project_id=project_id,
        )
        direction_id = plan.directions[0].direction_id
        kernel = AgentKernelService(factory)
        segmented = kernel.segment_blockout(
            SegmentAgentBlockoutRequest(
                client_request_id="d005-segment",
                plan=plan,
                direction_id=direction_id,
                variant_id="exploration_vehicle_a",
            ),
            "d005-segment",
        )
        assets = AgentAssetEditingService(factory)
        version = assets.commit_blockout(
            CommitAgentBlockoutRequest(client_request_id="d005-commit", artifact_id=segmented.artifact_id),
            "d005-commit",
        )
        target_id = part_id_for_role_selector(version.parts, version.assembly_graph, "primary_form")
        target = next(part for part in version.parts if part.part_id == target_id)
        resolved = assets.list_semantic_proportions(version.asset_version_id, part_id=target.part_id)
        assert resolved.shape_program_sha256 and resolved.glb_sha256
        assert {item.recipe_id for item in resolved.options} == {
            "proportion_vehicle_compact", "proportion_vehicle_sleek", "proportion_vehicle_substantial"
        }
        assert all(item.unit == "ratio" and item.source_operation_ids for item in resolved.options)

        no_binding = next(part for part in version.parts if not part.editable_parameter_bindings)
        unavailable = assets.list_semantic_proportions(version.asset_version_id, part_id=no_binding.part_id)
        assert not unavailable.options and unavailable.unavailable_message

        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot(project_id)
            assert snapshot is not None
            locked = unit.active_designs.set_part_display(
                project_id=project_id, expected_revision=snapshot.revision,
                action="lock", part_id=target.part_id, updated_at=NOW,
            )
        locked_result = assets.list_semantic_proportions(version.asset_version_id, part_id=target.part_id)
        assert locked_result.locked and not locked_result.options
        option_for_rejection = resolved.options[0]
        _expect_error(
            lambda: assets.propose_change_set(version.asset_version_id, _proposal("d005-locked", target.part_id, option_for_rejection.path, option_for_rejection.target_value), "d005-locked"),
            "PART_PROTECTED",
        )
        with SQLiteUnitOfWork(factory) as unit:
            unit.active_designs.set_part_display(
                project_id=project_id, expected_revision=locked.revision,
                action="unlock", part_id=target.part_id, updated_at=NOW,
            )

        _expect_error(
            lambda: assets.propose_change_set(version.asset_version_id, _proposal("d005-range", target.part_id, "transform.scale.x", 1.5), "d005-range"),
            "PARAMETER_OUT_OF_RANGE",
        )
        _expect_error(
            lambda: assets.propose_change_set(version.asset_version_id, _proposal("d005-step", target.part_id, "transform.scale.x", 1.05), "d005-step"),
            "PARAMETER_STEP_MISMATCH",
        )

        cancel = assets.propose_change_set(
            version.asset_version_id,
            _proposal("d005-cancel", target.part_id, option_for_rejection.path, option_for_rejection.target_value),
            "d005-cancel",
        )
        assert assets.preview_change_set(cancel.change_set_id, "d005-cancel-preview").preview is not None
        assert assets.reject_change_set(cancel.change_set_id, "d005-cancel-reject").status == "rejected"
        with SQLiteUnitOfWork(factory) as unit:
            assert len(unit.agent_assets.list_versions(project_id)) == 1

        resolved = assets.list_semantic_proportions(version.asset_version_id, part_id=target.part_id)
        option = next(item for item in resolved.options if item.recipe_id == "proportion_vehicle_sleek")
        change = assets.propose_change_set(
            version.asset_version_id,
            _proposal("d005-confirm", target.part_id, option.path, option.target_value),
            "d005-confirm",
        )
        assert assets.preview_change_set(change.change_set_id, "d005-confirm-preview").preview is not None
        confirmed = assets.confirm_change_set(change.change_set_id, "d005-confirm-save")
        assert confirmed.asset_version.parent_asset_version_id == version.asset_version_id
        report = assets.quality(confirmed.asset_version.asset_version_id)
        assert report.evidence_source == "geometry_compile_readback" and report.triangle_count > 0

        restarted = AgentAssetEditingService(SQLiteConnectionFactory(Path(raw) / "library.db"))
        after_restart = restarted.list_semantic_proportions(confirmed.asset_version.asset_version_id, part_id=target.part_id)
        assert any(item.current_value == option.target_value for item in after_restart.options)

        navigation = ActiveDesignService(factory)
        with SQLiteUnitOfWork(factory) as unit:
            snapshot = unit.active_designs.get_snapshot(project_id)
            assert snapshot is not None
        undone = navigation.navigate_asset(
            project_id,
            NavigateActiveDesignRequest(client_request_id="d005-undo", snapshot_revision=snapshot.revision),
            expected_revision=snapshot.revision,
            idempotency_key="d005-undo",
            action="undo",
        )
        undo_id = undone.active_design.asset_version_id
        undo_result = restarted.list_semantic_proportions(undo_id, part_id=target.part_id)
        assert any(item.current_value == 1.0 for item in undo_result.options)
        redone = navigation.navigate_asset(
            project_id,
            NavigateActiveDesignRequest(client_request_id="d005-redo", snapshot_revision=undone.revision),
            expected_revision=undone.revision,
            idempotency_key="d005-redo",
            action="redo",
        )
        redo_result = restarted.list_semantic_proportions(redone.active_design.asset_version_id, part_id=target.part_id)
        assert any(item.current_value == option.target_value for item in redo_result.options)


def main() -> int:
    _contract_and_four_domain_gate()
    _service_gate()
    print("D005 semantic proportions smoke passed: four domains, contracts, bindings, GLB readback, lock/range/step, preview cancel/confirm, restart and undo/redo")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
