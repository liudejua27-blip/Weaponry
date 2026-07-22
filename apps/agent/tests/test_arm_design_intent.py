from __future__ import annotations

import pytest
from pydantic import ValidationError

from forgecad_agent.application.arm_design_intent import infer_arm_design_intent
from forgecad_agent.application.agent_models import ArmDesignIntent


def test_brief_projects_to_composable_visual_axes() -> None:
    intent = infer_arm_design_intent(
        "一台蓝黑色六边形底座、开放桁架、装甲轴承盒、编织线束、分层腕部和自适应夹爪的长臂机械臂，加入流线和人字雕刻，抬升展示"
    )
    assert intent.schema_version == "ArmDesignIntent@1"
    assert intent.architecture == "serial_chain"
    assert intent.joint_language == "armored_bearing"
    assert intent.link_language == "open_truss"
    assert intent.base_language == "hex_platform"
    assert intent.end_effector_language == "adaptive_claw"
    assert intent.cable_language == "braided_external"
    assert intent.surface_language == ["flowline", "chevron_relief", "engraved_ribs"]
    assert intent.material_palette == "graphite_blue"
    assert intent.pose == "elevated"
    assert intent.proportion_profile == "long_reach"


def test_unmentioned_axes_have_explicit_safe_defaults() -> None:
    intent = infer_arm_design_intent("非功能性桌面机械臂概念")
    assert intent.architecture == "serial_chain"
    assert intent.link_language == "closed_shell"
    assert intent.surface_language == ["panel_seams", "flowline"]
    assert intent.visual_only is True


def test_intent_rejects_duplicate_surface_tokens_and_extra_fields() -> None:
    payload = infer_arm_design_intent("机械臂").model_dump(mode="json")
    payload["surface_language"] = ["flowline", "flowline"]
    with pytest.raises(ValidationError, match="surface_language must be unique"):
        ArmDesignIntent.model_validate(payload)
    payload = infer_arm_design_intent("机械臂").model_dump(mode="json")
    payload["arbitrary_geometry_code"] = "box()"
    with pytest.raises(ValidationError):
        ArmDesignIntent.model_validate(payload)
