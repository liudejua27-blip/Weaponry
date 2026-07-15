from __future__ import annotations

from math import isfinite
from typing import List, Literal, Optional

from pydantic import Field, model_validator

from .concept_models import StrictApiModel


GeometrySurfaceRole = Literal[
    "surface",
    "side",
    "loft_side",
    "sweep_side",
    "hole_wall",
    "start_cap",
    "end_cap",
    "seam",
    "boolean_cut",
    "trim",
]

GeometryNormalMode = Literal["split", "split_weighted"]


class GeometryEdgeFinishReadback(StrictApiModel):
    """Bounded visual edge completion; never an engineering fillet claim."""

    mode: Literal["none", "bevel_approximation"]
    edge_set: Literal["none", "xz_perimeter"]
    selected_edge_count: int = Field(ge=0, le=4)
    radius_ratio: float = Field(ge=0, le=0.25)
    subdivision_count: int = Field(ge=0, le=3)

    @model_validator(mode="after")
    def validate_edge_finish(self) -> "GeometryEdgeFinishReadback":
        empty = self.mode == "none"
        if empty != (self.edge_set == "none"):
            raise ValueError("edge finish mode and edge set must align")
        if empty and any((self.selected_edge_count, self.radius_ratio, self.subdivision_count)):
            raise ValueError("empty edge finish cannot report geometry work")
        if not empty and (
            self.selected_edge_count != 4
            or self.radius_ratio <= 0
            or self.subdivision_count <= 0
        ):
            raise ValueError("bevel approximation must report its bounded perimeter work")
        return self


class GeometrySurfaceRange(StrictApiModel):
    surface_role: GeometrySurfaceRole
    first_triangle: int = Field(ge=0)
    triangle_count: int = Field(ge=0)


class GeometrySurfaceProvenance(StrictApiModel):
    primitive_id: str = Field(pattern=r"^primitive_[a-z0-9_\-]+$")
    part_instance_id: str = Field(pattern=r"^partface_[a-z0-9_\-]+$")
    part_role: str = Field(min_length=1, max_length=64)
    profile_input_id: Optional[str] = Field(default=None, pattern=r"^profileinput_[a-z0-9_\-]+$")
    surface_roles: List[GeometrySurfaceRole] = Field(min_length=1, max_length=8)
    surface_ranges: List[GeometrySurfaceRange] = Field(min_length=1, max_length=8)
    uv0_min: List[float] = Field(min_length=2, max_length=2)
    uv0_max: List[float] = Field(min_length=2, max_length=2)
    closed: bool
    boundary_edge_count: int = Field(ge=0)
    non_manifold_edge_count: int = Field(ge=0)
    degenerate_triangle_count: int = Field(ge=0)
    feature_node_id: Optional[str] = Field(default=None, pattern=r"^op_[a-z0-9_\-]+$")
    source_operation_ids: List[str] = Field(default_factory=list, max_length=32)
    material_zone_id: str = Field(pattern=r"^zone_[a-z0-9_\-]+$")
    boolean_backside: Optional[bool] = None
    normal_mode: GeometryNormalMode
    tangent_min_length: float = Field(ge=0.999, le=1.001)
    tangent_max_length: float = Field(ge=0.999, le=1.001)
    tangent_handedness: List[Literal[-1, 1]] = Field(min_length=1, max_length=2)
    uv_degenerate_triangle_count: int = Field(ge=0)
    tangent_fallback_triangle_count: int = Field(ge=0)
    face_id_min: int = Field(ge=0)
    face_id_max: int = Field(ge=0)
    face_id_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    edge_finish: GeometryEdgeFinishReadback
    texture_ready: bool

    @model_validator(mode="after")
    def validate_surface(self) -> "GeometrySurfaceProvenance":
        if len(self.surface_roles) != len(set(self.surface_roles)):
            raise ValueError("surface roles must be unique")
        if any(item.surface_role not in self.surface_roles for item in self.surface_ranges):
            raise ValueError("surface ranges must reference declared roles")
        if any(not isfinite(value) for value in [*self.uv0_min, *self.uv0_max]):
            raise ValueError("UV0 bounds must be finite")
        if any(left > right for left, right in zip(self.uv0_min, self.uv0_max)):
            raise ValueError("UV0 min must not exceed max")
        if self.closed and (self.boundary_edge_count or self.non_manifold_edge_count or self.degenerate_triangle_count):
            raise ValueError("closed surface cannot report topology failures")
        if self.face_id_min != 0 or self.face_id_max < self.face_id_min:
            raise ValueError("surface face ids must start at zero and be bounded")
        if self.uv_degenerate_triangle_count or self.tangent_fallback_triangle_count or not self.texture_ready:
            raise ValueError("surface is not ready for a later texture pipeline")
        if self.tangent_min_length > self.tangent_max_length:
            raise ValueError("tangent length bounds are reversed")
        return self


class GeometryMaterialZoneFaceSet(StrictApiModel):
    primitive_id: str = Field(pattern=r"^primitive_[a-z0-9_\-]+$")
    part_instance_id: str = Field(pattern=r"^partface_[a-z0-9_\-]+$")
    material_zone_id: str = Field(pattern=r"^zone_[a-z0-9_\-]+$")
    face_count: int = Field(ge=1)
    face_id_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    surface_roles: List[GeometrySurfaceRole] = Field(min_length=1, max_length=16)
    source_operation_ids: List[str] = Field(min_length=1, max_length=32)
    texture_ready: Literal[True] = True

    @model_validator(mode="after")
    def validate_zone_faces(self) -> "GeometryMaterialZoneFaceSet":
        if len(self.surface_roles) != len(set(self.surface_roles)):
            raise ValueError("zone surface roles must be unique")
        if len(self.source_operation_ids) != len(set(self.source_operation_ids)):
            raise ValueError("zone source operation ids must be unique")
        if any(not value.startswith("op_") for value in self.source_operation_ids):
            raise ValueError("zone face provenance must reference operations")
        return self


class GeometryFeatureNodeReadback(StrictApiModel):
    """Canonical feature input and real compiled result identity for one ordered node."""

    schema_version: Literal["GeometryFeatureNodeReadback@1"] = "GeometryFeatureNodeReadback@1"
    node_id: str = Field(pattern=r"^op_[a-z0-9_\-]+$")
    operation: str = Field(min_length=1, max_length=64)
    input_node_ids: List[str] = Field(default_factory=list, max_length=8)
    input_hashes: List[str] = Field(default_factory=list, max_length=8)
    parameters_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    node_input_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    result_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    surface_provenance_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    runtime_manifest_version: Literal["ShapeProgramRuntimeManifest@1"] = "ShapeProgramRuntimeManifest@1"
    kernel_id: Literal["forgecad_builtin", "manifold3d"]
    kernel_version: str = Field(min_length=1, max_length=64)
    csg_depth: int = Field(ge=0, le=8)
    result_triangle_count: int = Field(ge=0)
    result_closed: bool
    material_ids: List[str] = Field(default_factory=list, max_length=64)
    material_zone_ids: List[str] = Field(default_factory=list, max_length=64)
    surface_roles: List[GeometrySurfaceRole] = Field(default_factory=list, max_length=16)

    @model_validator(mode="after")
    def validate_feature_node(self) -> "GeometryFeatureNodeReadback":
        if len(self.input_node_ids) != len(self.input_hashes):
            raise ValueError("feature node input ids and hashes must align")
        if len(self.input_node_ids) != len(set(self.input_node_ids)):
            raise ValueError("feature node input ids must be unique")
        if any(not value.startswith("op_") for value in self.input_node_ids):
            raise ValueError("feature node inputs must reference ordered operation nodes")
        if any(len(value) != 64 or any(char not in "0123456789abcdef" for char in value) for value in self.input_hashes):
            raise ValueError("feature node input hashes must be sha256 values")
        if self.kernel_id == "manifold3d" and self.operation not in {"union", "subtract"}:
            raise ValueError("Manifold may only own the selected CSG operations")
        if self.operation in {"union", "subtract"} and self.kernel_id != "manifold3d":
            raise ValueError("CSG operations must use the selected Manifold kernel")
        return self


class GeometryCompileReadback(StrictApiModel):
    """Immutable facts read from the exact GLB produced by one compilation."""

    schema_version: Literal["GeometryCompileReadback@1"] = "GeometryCompileReadback@1"
    runtime_manifest_version: Literal["ShapeProgramRuntimeManifest@1"] = "ShapeProgramRuntimeManifest@1"
    program_id: str = Field(min_length=1, max_length=160)
    shape_program_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    glb_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    glb_byte_size: int = Field(ge=20, le=64 * 1024 * 1024)
    triangle_count: int = Field(ge=1)
    bounds_mm: List[float] = Field(min_length=3, max_length=3)
    mesh_count: int = Field(ge=1)
    primitive_count: int = Field(ge=1)
    material_count: int = Field(ge=0)
    uv0_primitive_count: int = Field(ge=1)
    normal_primitive_count: int = Field(ge=1)
    tangent_primitive_count: int = Field(ge=1)
    surface_provenance: List[GeometrySurfaceProvenance] = Field(min_length=1, max_length=512)
    material_zone_faces: List[GeometryMaterialZoneFaceSet] = Field(min_length=1, max_length=512)
    feature_history: List[GeometryFeatureNodeReadback] = Field(default_factory=list, max_length=256)
    operation_ids: List[str] = Field(min_length=1, max_length=512)
    operation_names: List[str] = Field(min_length=1, max_length=512)
    output_roles: List[str] = Field(min_length=1, max_length=512)
    material_ids: List[str] = Field(default_factory=list, max_length=64)
    readback_status: Literal["passed"] = "passed"

    @model_validator(mode="after")
    def validate_readback(self) -> "GeometryCompileReadback":
        if any(not isfinite(value) or value <= 0 for value in self.bounds_mm):
            raise ValueError("compile readback bounds must be finite positive values")
        if len(self.operation_ids) != len(self.operation_names):
            raise ValueError("compile readback operation ids and names must align")
        if len(self.operation_ids) != len(set(self.operation_ids)):
            raise ValueError("compile readback operation ids must be unique")
        if any(not item for item in [*self.operation_ids, *self.operation_names, *self.output_roles]):
            raise ValueError("compile readback operation and output facts must be non-empty")
        if (
            self.uv0_primitive_count != self.primitive_count
            or self.normal_primitive_count != self.primitive_count
            or self.tangent_primitive_count != self.primitive_count
        ):
            raise ValueError("compile readback requires UV0, normals and tangents on every primitive")
        if len(self.surface_provenance) != self.primitive_count:
            raise ValueError("compile readback surface provenance must align with primitives")
        if len(self.material_zone_faces) != self.primitive_count:
            raise ValueError("compile readback material zones must align with primitives")
        surface_ids = [item.primitive_id for item in self.surface_provenance]
        zone_ids = [item.primitive_id for item in self.material_zone_faces]
        if len(surface_ids) != len(set(surface_ids)) or surface_ids != zone_ids:
            raise ValueError("surface and zone primitive identities must align uniquely")
        surface_by_id = {item.primitive_id: item for item in self.surface_provenance}
        for zone in self.material_zone_faces:
            surface = surface_by_id[zone.primitive_id]
            if (
                zone.part_instance_id != surface.part_instance_id
                or zone.material_zone_id != surface.material_zone_id
                or zone.face_id_sha256 != surface.face_id_sha256
                or zone.face_count != surface.face_id_max + 1
            ):
                raise ValueError("material zone face mapping diverges from surface readback")
        if self.feature_history:
            if [item.node_id for item in self.feature_history] != self.operation_ids:
                raise ValueError("compile readback feature history must align with ordered operations")
            result_hash_by_id: dict[str, str] = {}
            for item in self.feature_history:
                if any(input_id not in result_hash_by_id for input_id in item.input_node_ids):
                    raise ValueError("compile readback feature history has a forward input")
                if item.input_hashes != [result_hash_by_id[input_id] for input_id in item.input_node_ids]:
                    raise ValueError("compile readback feature input hashes do not match prior results")
                result_hash_by_id[item.node_id] = item.result_sha256
        return self
