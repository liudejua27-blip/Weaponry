from __future__ import annotations

from math import isfinite
from typing import List, Literal, Optional

from pydantic import Field, model_validator

from .concept_models import StrictApiModel


GeometrySurfaceRole = Literal["surface", "side", "loft_side", "sweep_side", "hole_wall", "start_cap", "end_cap", "seam"]


class GeometrySurfaceRange(StrictApiModel):
    surface_role: GeometrySurfaceRole
    first_triangle: int = Field(ge=0)
    triangle_count: int = Field(ge=0)


class GeometrySurfaceProvenance(StrictApiModel):
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
    surface_provenance: List[GeometrySurfaceProvenance] = Field(min_length=1, max_length=512)
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
        if self.uv0_primitive_count != self.primitive_count or self.normal_primitive_count != self.primitive_count:
            raise ValueError("compile readback requires UV0 and normals on every primitive")
        if len(self.surface_provenance) != self.primitive_count:
            raise ValueError("compile readback surface provenance must align with primitives")
        return self
