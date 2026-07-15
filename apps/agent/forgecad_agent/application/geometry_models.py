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
    material_id: str = Field(pattern=r"^mat_[a-z0-9_\-]+$")
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


class GeometryVisualTextureMapReadback(StrictApiModel):
    texture_id: str = Field(pattern=r"^vtex_[a-z0-9_\-]+$")
    texture_role: Literal["base_color", "metallic_roughness", "normal", "occlusion", "emissive"]
    mime_type: Literal["image/png", "image/jpeg", "image/webp"]
    byte_size: int = Field(gt=0, le=4_000_000)
    sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    color_space: Literal["srgb", "linear"]
    width: int = Field(gt=0, le=4096)
    height: int = Field(gt=0, le=4096)
    source: Literal["forgecad_builtin", "user_created", "imported_reference"]
    license: Literal["not_applicable", "self_declared_original", "third_party", "unknown"]
    fallback: Literal["none", "parameter", "unavailable"]
    glb_image_index: int = Field(ge=0)
    glb_texture_index: int = Field(ge=0)


class GeometryVisualTextureSetReadback(StrictApiModel):
    schema_version: Literal["VisualTextureSet@1"] = "VisualTextureSet@1"
    visual_texture_set_id: str = Field(pattern=r"^vtexset_[a-z0-9_\-]+$")
    material_id: str = Field(pattern=r"^mat_[a-z0-9_\-]+$")
    material_index: int = Field(ge=0)
    material_zone_ids: List[str] = Field(min_length=1, max_length=512)
    maps: List[GeometryVisualTextureMapReadback] = Field(min_length=5, max_length=5)
    extensions: List[str] = Field(default_factory=list, max_length=8)
    texture_byte_size: int = Field(gt=0, le=20_000_000)

    @model_validator(mode="after")
    def validate_texture_set(self) -> "GeometryVisualTextureSetReadback":
        if len(self.material_zone_ids) != len(set(self.material_zone_ids)):
            raise ValueError("visual texture zones must be unique")
        roles = [item.texture_role for item in self.maps]
        if len(roles) != len(set(roles)) or set(roles) != {
            "base_color", "metallic_roughness", "normal", "occlusion", "emissive"
        }:
            raise ValueError("visual texture readback must contain every PBR map once")
        if self.texture_byte_size != sum(item.byte_size for item in self.maps):
            raise ValueError("visual texture byte budget does not match maps")
        return self


class GeometryVisualEnvironmentReadback(StrictApiModel):
    schema_version: Literal["ForgeCADVisualEnvironment@1"] = "ForgeCADVisualEnvironment@1"
    environment_id: str = Field(pattern=r"^env_[a-z0-9_\-]+$")
    environment_kind: Literal["procedural_studio"]
    environment_sha256: str = Field(pattern=r"^[a-f0-9]{64}$")
    source: Literal["forgecad_builtin"]
    license: Literal["not_applicable"]
    color_workflow: Literal["linear_srgb"]
    output_color_space: Literal["srgb"]
    tone_mapping: Literal["aces_filmic"]
    tone_mapping_exposure: float = Field(ge=0.1, le=3)
    contact_shadows: Literal[True]
    pmrem: "GeometryVisualPmremReadback"


class GeometryVisualPmremReadback(StrictApiModel):
    near: float = Field(gt=0, le=1)
    cube_size: Literal[128]


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
    visual_texture_sets: List[GeometryVisualTextureSetReadback] = Field(min_length=1, max_length=64)
    visual_environment: GeometryVisualEnvironmentReadback
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
        zones_by_material: dict[str, set[str]] = {}
        for zone in self.material_zone_faces:
            zones_by_material.setdefault(zone.material_id, set()).add(zone.material_zone_id)
        if {item.material_id for item in self.visual_texture_sets} != set(zones_by_material):
            raise ValueError("visual texture sets must cover exactly the GLB material-zone materials")
        for texture_set in self.visual_texture_sets:
            if set(texture_set.material_zone_ids) != zones_by_material[texture_set.material_id]:
                raise ValueError("visual texture set zones diverge from GLB material-zone readback")
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
