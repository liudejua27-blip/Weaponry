from pydantic import ValidationError
import pytest

from forgecad_agent.application.assembly_delta import AssemblyDeltaProgram


def _delta() -> dict:
    return {
        "schema_version": "AssemblyDeltaProgram@1",
        "domain_pack_id": "pack_robotic_arm_concept",
        "base_asset_version_id": "assetver_arm_v3",
        "summary": "在当前腕部增加一个已审核的传感器舱，并保持概念展示用途。",
        "operations": [
            {
                "op": "add_reviewed_recipe",
                "operation_id": "delta_sensor_pod",
                "new_part_id": "part_sensor_pod_1",
                "parent_part_id": "part_wrist_1",
                "parent_connector_id": "conn_wrist_top",
                "child_connector_id": "conn_sensor_mount",
                "recipe_id": "recipe_c110c_arm_sensor_pod",
                "slot_id": "slot_arm_sensor_pod",
                "transform": {
                    "position": [0, 20, 0],
                    "rotation": [0, 0, 0],
                    "scale": [1, 1, 1],
                },
            }
        ],
        "visual_only": True,
    }


def test_assembly_delta_is_strict_and_bounded() -> None:
    delta = AssemblyDeltaProgram.model_validate(_delta())
    assert delta.operations[0].op == "add_reviewed_recipe"
    assert delta.model_dump(mode="json")["visual_only"] is True


def test_assembly_delta_rejects_duplicate_operation_ids() -> None:
    payload = _delta()
    payload["operations"].append(dict(payload["operations"][0]))
    payload["operations"][1]["new_part_id"] = "part_sensor_pod_2"
    with pytest.raises(ValidationError, match="operation_id values must be unique"):
        AssemblyDeltaProgram.model_validate(payload)


def test_assembly_delta_rejects_unreviewed_recipe() -> None:
    payload = _delta()
    payload["operations"][0]["recipe_id"] = "recipe_unknown"
    with pytest.raises(ValidationError):
        AssemblyDeltaProgram.model_validate(payload)
