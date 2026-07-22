"""Bounded, visual-only edits that continue an existing Agent asset.

This is deliberately an intent contract.  It does not contain ShapeProgram
operations and it cannot be executed by Python.  Rust validates the same
payload again before lowering it into the existing ChangeSet preview flow.
"""

from __future__ import annotations

from typing import Annotated, List, Literal, Union

from pydantic import Field, model_validator

from .concept_models import StrictApiModel


DeltaId = Annotated[str, Field(min_length=1, max_length=120, pattern=r"^[A-Za-z0-9_:-]+$")]
Vector3 = Annotated[List[float], Field(min_length=3, max_length=3)]

ReviewedRecipeId = Literal[
    "recipe_c106_arm_turntable",
    "recipe_c106_arm_joint_housing",
    "recipe_c106_arm_link_armor",
    "recipe_c106_arm_cable_harness",
    "recipe_c106_arm_gripper",
    "recipe_c106_arm_surface_trim",
    "recipe_c110c_arm_sensor_pod",
    "recipe_c110d_arm_actuator_cover",
    "recipe_c110d_arm_cable_guide",
    "recipe_c110d_arm_wrist_tool_mount",
]

ReviewedAttachmentSlot = Literal[
    "slot_arm_sensor_pod",
    "slot_arm_guard_rail",
    "slot_arm_tool_changer",
    "slot_arm_camera_boom",
]


class AssemblyDeltaTransform(StrictApiModel):
    position: Vector3
    rotation: Vector3
    scale: Vector3 = (1.0, 1.0, 1.0)


class AssemblyDeltaPose(StrictApiModel):
    rotation: Vector3
    translation: Vector3


class AddReviewedRecipeOperation(StrictApiModel):
    op: Literal["add_reviewed_recipe"]
    operation_id: DeltaId
    new_part_id: Annotated[str, Field(min_length=1, max_length=120, pattern=r"^part_[A-Za-z0-9_:-]+$")]
    parent_part_id: DeltaId
    parent_connector_id: DeltaId
    child_connector_id: DeltaId
    recipe_id: ReviewedRecipeId
    slot_id: ReviewedAttachmentSlot
    transform: AssemblyDeltaTransform


class ReplaceReviewedRecipeOperation(StrictApiModel):
    op: Literal["replace_reviewed_recipe"]
    operation_id: DeltaId
    part_id: DeltaId
    recipe_id: ReviewedRecipeId


class SetPartTransformOperation(StrictApiModel):
    op: Literal["set_part_transform"]
    operation_id: DeltaId
    part_id: DeltaId
    transform: AssemblyDeltaTransform


class SetJointPoseOperation(StrictApiModel):
    op: Literal["set_joint_pose"]
    operation_id: DeltaId
    part_id: DeltaId
    joint_id: DeltaId
    pose: AssemblyDeltaPose


class SnapPartToConnectorOperation(StrictApiModel):
    op: Literal["snap_part_to_connector"]
    operation_id: DeltaId
    part_id: DeltaId
    target_part_id: DeltaId
    target_connector_id: DeltaId
    connector_id: DeltaId


AssemblyDeltaOperation = Union[
    AddReviewedRecipeOperation,
    ReplaceReviewedRecipeOperation,
    SetPartTransformOperation,
    SetJointPoseOperation,
    SnapPartToConnectorOperation,
]


class AssemblyDeltaProgram(StrictApiModel):
    schema_version: Literal["AssemblyDeltaProgram@1"] = "AssemblyDeltaProgram@1"
    domain_pack_id: Literal["pack_robotic_arm_concept"] = "pack_robotic_arm_concept"
    base_asset_version_id: DeltaId
    summary: str = Field(min_length=1, max_length=2000)
    operations: List[AssemblyDeltaOperation] = Field(min_length=1, max_length=8)
    visual_only: Literal[True] = True

    @model_validator(mode="after")
    def validate_operation_ids(self) -> "AssemblyDeltaProgram":
        operation_ids = [operation.operation_id for operation in self.operations]
        if len(set(operation_ids)) != len(operation_ids):
            raise ValueError("operation_id values must be unique")
        return self
