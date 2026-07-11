from __future__ import annotations

import math
from pathlib import PurePosixPath
from typing import Annotated, Any, Dict, List, Literal, Optional, Set, Union

from pydantic import BaseModel, ConfigDict, Field, StringConstraints, model_validator


ModuleCategory = Literal[
    "core_shell",
    "front_shell",
    "rear_shell",
    "grip_shell",
    "top_accessory",
    "side_accessory",
    "lower_structure",
    "storage_visual",
    "armor_panel",
]
IntendedUse = Literal[
    "visual_asset",
    "game_asset",
    "film_prop",
    "non_functional_display",
]
QualityStatus = Literal["passed", "warning", "failed", "not_run"]
MirrorAxis = Literal["none", "x", "y", "z"]
ContractId = Annotated[
    str,
    StringConstraints(
        pattern=r"^(prj|profile|pack|module|connector|mg|node|edge|change|quality|finding|job|evt|ver|asset|export)_[A-Za-z0-9_\-]+$"
    ),
]
ConnectorSlot = Annotated[
    str,
    StringConstraints(pattern=r"^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$"),
]


class StrictContractModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class Transform(StrictContractModel):
    position: List[float] = Field(
        min_length=3,
        max_length=3,
        description="Millimeters in graph-world or module-local space.",
    )
    rotation: List[float] = Field(
        min_length=3,
        max_length=3,
        description="Radians using Euler XYZ order.",
    )
    scale: List[float] = Field(
        min_length=3,
        max_length=3,
        description="Dimensionless positive scale.",
    )

    @model_validator(mode="after")
    def validate_finite_positive_scale(self) -> "Transform":
        values = [*self.position, *self.rotation, *self.scale]
        if not all(math.isfinite(value) for value in values):
            raise ValueError("transform values must be finite")
        if any(value <= 0 for value in self.scale):
            raise ValueError("transform scale must be greater than zero")
        return self


class DesignDomainProfile(StrictContractModel):
    schema_version: Literal["DesignDomainProfile@1"] = "DesignDomainProfile@1"
    profile_id: ContractId
    domain_type: Literal["weapon_concept"] = "weapon_concept"
    display_name: str = Field(min_length=1, max_length=120)
    pack_id: ContractId
    intended_uses: List[IntendedUse] = Field(min_length=1)
    module_categories: List[ModuleCategory] = Field(min_length=1)
    required_connectors: List[ConnectorSlot] = Field(min_length=1)
    optional_connectors: List[ConnectorSlot] = Field(default_factory=list)
    export_profiles: List[IntendedUse] = Field(min_length=1)
    non_functional_only: Literal[True] = True

    @model_validator(mode="after")
    def validate_unique_profile_values(self) -> "DesignDomainProfile":
        _require_unique("intended_uses", self.intended_uses)
        _require_unique("module_categories", self.module_categories)
        _require_unique("required_connectors", self.required_connectors)
        _require_unique("optional_connectors", self.optional_connectors)
        _require_unique("export_profiles", self.export_profiles)
        overlap = set(self.required_connectors) & set(self.optional_connectors)
        if overlap:
            raise ValueError(f"connector slots cannot be both required and optional: {sorted(overlap)}")
        return self


class ConceptStyle(StrictContractModel):
    keywords: List[str] = Field(min_length=1, max_length=12)
    palette: List[str] = Field(min_length=1, max_length=8)
    detail_density: float = Field(ge=0, le=1)


class ConceptProportions(StrictContractModel):
    overall_length_mm: float = Field(gt=0, le=1000)
    body_height_mm: float = Field(gt=0, le=1000)
    grip_angle_deg: float = Field(ge=-45, le=45)


class ConceptConstraints(StrictContractModel):
    symmetry: Literal["symmetric", "mostly_symmetric", "asymmetric"]
    max_triangle_count: int = Field(ge=1000, le=2_000_000)


class WeaponConceptSpec(StrictContractModel):
    schema_version: Literal["WeaponConceptSpec@1"] = "WeaponConceptSpec@1"
    project_id: ContractId
    profile_id: ContractId
    name: str = Field(min_length=1, max_length=120)
    archetype: Literal["future_modular_sidearm"] = "future_modular_sidearm"
    intended_uses: List[IntendedUse] = Field(min_length=1)
    style: ConceptStyle
    proportions: ConceptProportions
    required_slots: List[str] = Field(min_length=1)
    optional_slots: List[str] = Field(default_factory=list)
    constraints: ConceptConstraints
    assumptions: List[str] = Field(min_length=1)

    @model_validator(mode="after")
    def validate_slots(self) -> "WeaponConceptSpec":
        _require_unique("intended_uses", self.intended_uses)
        _require_unique("required_slots", self.required_slots)
        _require_unique("optional_slots", self.optional_slots)
        overlap = set(self.required_slots) & set(self.optional_slots)
        if overlap:
            raise ValueError(f"slots cannot be both required and optional: {sorted(overlap)}")
        return self


class ModuleConnector(StrictContractModel):
    connector_id: ContractId
    slot: ConnectorSlot
    connector_type: Annotated[str, StringConstraints(pattern=r"^[a-z][a-z0-9_]*$")]
    transform: Transform
    scale_range: List[float] = Field(min_length=2, max_length=2)
    exclusive: bool = True

    @model_validator(mode="after")
    def validate_scale_range(self) -> "ModuleConnector":
        minimum, maximum = self.scale_range
        if not math.isfinite(minimum) or not math.isfinite(maximum):
            raise ValueError("connector scale range must be finite")
        if minimum <= 0 or maximum <= 0 or minimum > maximum:
            raise ValueError("connector scale range must be positive and ordered")
        return self


class ModuleAssetManifest(StrictContractModel):
    schema_version: Literal["ModuleAssetManifest@1"] = "ModuleAssetManifest@1"
    module_id: ContractId
    pack_id: ContractId
    category: ModuleCategory
    asset_id: ContractId
    sha256: str
    bounds_mm: List[float] = Field(min_length=3, max_length=3)
    triangle_count: int = Field(ge=1)
    material_slots: List[str] = Field(min_length=1)
    connectors: List[ModuleConnector] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_manifest(self) -> "ModuleAssetManifest":
        if len(self.sha256) != 64 or any(char not in "0123456789abcdef" for char in self.sha256):
            raise ValueError("sha256 must be a lowercase hexadecimal digest")
        if any(not math.isfinite(value) or value <= 0 for value in self.bounds_mm):
            raise ValueError("module bounds must contain finite positive values")
        _require_unique("material_slots", self.material_slots)
        _require_unique("connector ids", [item.connector_id for item in self.connectors])
        _require_unique("connector slots", [item.slot for item in self.connectors])
        return self


class ModulePackLicense(StrictContractModel):
    spdx_expression: str = Field(min_length=1, max_length=120)
    license_path: str = Field(min_length=1)

    @model_validator(mode="after")
    def validate_license(self) -> "ModulePackLicense":
        _validate_relative_path(self.license_path)
        return self


class ModulePackEntry(StrictContractModel):
    module_id: ContractId
    manifest_path: str = Field(min_length=1)
    glb_path: str = Field(min_length=1)
    thumbnail_path: str = Field(min_length=1)
    license_path: str = Field(min_length=1)
    lod: Literal["LOD0", "LOD1", "LOD2"] = "LOD0"

    @model_validator(mode="after")
    def validate_paths(self) -> "ModulePackEntry":
        for value in (
            self.manifest_path,
            self.glb_path,
            self.thumbnail_path,
            self.license_path,
        ):
            _validate_relative_path(value)
        return self


class ModulePackManifest(StrictContractModel):
    schema_version: Literal["ModulePackManifest@1"] = "ModulePackManifest@1"
    pack_id: ContractId
    profile_id: ContractId
    name: str = Field(min_length=1, max_length=120)
    version: Annotated[str, StringConstraints(pattern=r"^[0-9]+\.[0-9]+\.[0-9]+$")]
    description: str = Field(min_length=1, max_length=500)
    intended_uses: List[IntendedUse] = Field(min_length=1)
    non_functional_only: Literal[True] = True
    units: Literal["millimeter"] = "millimeter"
    up_axis: Literal["Y"] = "Y"
    forward_axis: Literal["-Z"] = "-Z"
    handedness: Literal["right"] = "right"
    license: ModulePackLicense
    modules: List[ModulePackEntry] = Field(min_length=1)

    @model_validator(mode="after")
    def validate_pack(self) -> "ModulePackManifest":
        _require_unique("intended_uses", self.intended_uses)
        _require_unique("module ids", [item.module_id for item in self.modules])
        for field_name in ("manifest_path", "glb_path", "thumbnail_path"):
            _require_unique(field_name, [getattr(item, field_name) for item in self.modules])
        return self


class ModuleGraphNode(StrictContractModel):
    node_id: ContractId
    module_id: ContractId
    transform: Transform
    mirror_axis: MirrorAxis = "none"
    locked: bool = False
    visible: bool = True


class ModuleGraphEdge(StrictContractModel):
    edge_id: ContractId
    from_node_id: ContractId
    from_connector_id: ContractId
    to_node_id: ContractId
    to_connector_id: ContractId
    status: Literal["connected", "invalid"] = "connected"


class ModuleGraph(StrictContractModel):
    schema_version: Literal["ModuleGraph@1"] = "ModuleGraph@1"
    graph_id: ContractId
    project_id: ContractId
    root_node_id: ContractId
    nodes: List[ModuleGraphNode] = Field(min_length=1)
    edges: List[ModuleGraphEdge] = Field(default_factory=list)

    @model_validator(mode="after")
    def validate_graph(self) -> "ModuleGraph":
        node_ids = [node.node_id for node in self.nodes]
        edge_ids = [edge.edge_id for edge in self.edges]
        _require_unique("node ids", node_ids)
        _require_unique("edge ids", edge_ids)
        node_id_set = set(node_ids)
        if self.root_node_id not in node_id_set:
            raise ValueError("root_node_id must reference a graph node")

        adjacency: Dict[str, Set[str]] = {node_id: set() for node_id in node_ids}
        connector_uses: set[tuple[str, str]] = set()
        for edge in self.edges:
            if edge.from_node_id not in node_id_set or edge.to_node_id not in node_id_set:
                raise ValueError(f"edge {edge.edge_id} references an unknown node")
            if edge.from_node_id == edge.to_node_id:
                raise ValueError(f"edge {edge.edge_id} cannot connect a node to itself")
            for endpoint in (
                (edge.from_node_id, edge.from_connector_id),
                (edge.to_node_id, edge.to_connector_id),
            ):
                if endpoint in connector_uses:
                    raise ValueError(f"connector endpoint is occupied more than once: {endpoint}")
                connector_uses.add(endpoint)
            if edge.status == "connected":
                adjacency[edge.from_node_id].add(edge.to_node_id)
                adjacency[edge.to_node_id].add(edge.from_node_id)

        reachable = _reachable_nodes(adjacency, self.root_node_id)
        missing = node_id_set - reachable
        if missing:
            raise ValueError(f"all graph nodes must connect to the root: {sorted(missing)}")
        return self


ChangeOperationType = Literal[
    "add_module",
    "remove_module",
    "replace_module",
    "connect",
    "disconnect",
    "set_transform",
    "set_mirror",
    "set_style",
    "set_parameter",
]


class DesignChangeOperation(StrictContractModel):
    operation_id: Annotated[str, StringConstraints(pattern=r"^op_[A-Za-z0-9_\-]+$")]
    op: ChangeOperationType
    node_id: Optional[ContractId] = None
    module_id: Optional[ContractId] = None
    edge_id: Optional[ContractId] = None
    from_node_id: Optional[ContractId] = None
    from_connector_id: Optional[ContractId] = None
    to_node_id: Optional[ContractId] = None
    to_connector_id: Optional[ContractId] = None
    path: Optional[str] = None
    value: Any = None
    mirror_axis: Optional[MirrorAxis] = None
    transform: Optional[Transform] = None

    @model_validator(mode="after")
    def validate_operation_payload(self) -> "DesignChangeOperation":
        node_ops = {"remove_module", "replace_module", "set_transform", "set_mirror"}
        if self.op in node_ops and not self.node_id:
            raise ValueError(f"{self.op} requires node_id")
        if self.op in {"add_module", "replace_module"} and not self.module_id:
            raise ValueError(f"{self.op} requires module_id")
        if self.op == "add_module" and (not self.node_id or self.transform is None):
            raise ValueError("add_module requires node_id and transform")
        if self.op == "set_transform" and self.transform is None:
            raise ValueError("set_transform requires transform")
        if self.op == "set_mirror" and self.mirror_axis is None:
            raise ValueError("set_mirror requires mirror_axis")
        if self.op in {"set_style", "set_parameter"} and not self.path:
            raise ValueError(f"{self.op} requires path")
        if self.op in {"connect", "disconnect"} and not self.edge_id:
            raise ValueError(f"{self.op} requires edge_id")
        if self.op == "connect" and not all(
            (
                self.from_node_id,
                self.from_connector_id,
                self.to_node_id,
                self.to_connector_id,
            )
        ):
            raise ValueError("connect requires both node and connector endpoints")
        return self


class DesignChangeSet(StrictContractModel):
    schema_version: Literal["DesignChangeSet@1"] = "DesignChangeSet@1"
    change_set_id: ContractId
    project_id: ContractId
    base_version_id: ContractId
    summary: str = Field(min_length=1, max_length=500)
    operations: List[DesignChangeOperation] = Field(min_length=1)
    protected_node_ids: List[ContractId] = Field(default_factory=list)
    status: Literal["proposed", "previewed", "confirmed", "rejected", "stale"] = "proposed"

    @model_validator(mode="after")
    def validate_protected_nodes(self) -> "DesignChangeSet":
        _require_unique("operation ids", [operation.operation_id for operation in self.operations])
        _require_unique("protected node ids", self.protected_node_ids)
        protected = set(self.protected_node_ids)
        destructive_ops = {"remove_module", "replace_module", "set_transform", "set_mirror"}
        conflicts = [
            operation.operation_id
            for operation in self.operations
            if operation.op in destructive_ops and operation.node_id in protected
        ]
        if conflicts:
            raise ValueError(f"operations cannot modify protected nodes: {conflicts}")
        return self


class QualityGeometryReference(StrictContractModel):
    node_id: ContractId
    triangle_indices: List[Annotated[int, Field(ge=0)]] = Field(
        default_factory=list, max_length=16
    )
    world_triangles_mm: List[List[List[float]]] = Field(default_factory=list, max_length=16)

    @model_validator(mode="after")
    def validate_triangles(self) -> "QualityGeometryReference":
        if len(self.triangle_indices) != len(self.world_triangles_mm):
            raise ValueError("triangle_indices and world_triangles_mm must have equal length")
        for triangle in self.world_triangles_mm:
            if len(triangle) != 3 or any(len(point) != 3 for point in triangle):
                raise ValueError("world triangles must contain exactly three VEC3 points")
            if any(not math.isfinite(value) for point in triangle for value in point):
                raise ValueError("world triangle coordinates must be finite")
        return self


class QualityFinding(StrictContractModel):
    finding_id: ContractId
    check_id: str = Field(min_length=1)
    category: Literal["graph", "mesh", "assembly"]
    severity: Literal["info", "warning", "error"]
    status: QualityStatus
    node_ids: List[ContractId] = Field(default_factory=list)
    geometry_refs: List[QualityGeometryReference] = Field(default_factory=list)
    measured_value: Optional[Union[float, str]] = None
    threshold: Optional[Union[float, str]] = None
    message: str = Field(min_length=1)
    suggestion: str = ""

    @model_validator(mode="after")
    def validate_geometry_references(self) -> "QualityFinding":
        _require_unique("quality geometry node ids", [item.node_id for item in self.geometry_refs])
        unknown = [item.node_id for item in self.geometry_refs if item.node_id not in self.node_ids]
        if unknown:
            raise ValueError(f"quality geometry references unknown finding nodes: {unknown}")
        return self


class ModelQualityReport(StrictContractModel):
    schema_version: Literal["ModelQualityReport@1"] = "ModelQualityReport@1"
    report_id: ContractId
    project_id: ContractId
    version_id: ContractId
    ruleset_version: str = Field(min_length=1)
    status: QualityStatus
    findings: List[QualityFinding] = Field(default_factory=list)
    created_at: str

    @model_validator(mode="after")
    def validate_status_summary(self) -> "ModelQualityReport":
        _require_unique("finding ids", [finding.finding_id for finding in self.findings])
        statuses = {finding.status for finding in self.findings}
        if "failed" in statuses and self.status != "failed":
            raise ValueError("report status must be failed when a finding failed")
        if "failed" not in statuses and "warning" in statuses and self.status not in {"warning", "failed"}:
            raise ValueError("report status must include warning severity")
        return self


class JobEventV2(StrictContractModel):
    schema_version: Literal["JobEvent@2"] = "JobEvent@2"
    event_id: ContractId
    job_id: ContractId
    seq: int = Field(ge=1)
    project_id: ContractId
    version_id: Optional[ContractId] = None
    step: str = Field(min_length=1)
    level: Literal["info", "warning", "error"] = "info"
    status: Literal[
        "created",
        "queued",
        "running",
        "waiting_provider",
        "waiting_user",
        "retrying",
        "succeeded",
        "failed",
        "cancelled",
        "partial_succeeded",
    ]
    message: str = Field(min_length=1)
    progress: float = Field(ge=0, le=1)
    artifact_asset_id: Optional[ContractId] = None
    metadata: Dict[str, Any] = Field(default_factory=dict)
    created_at: str


class ExportModuleEntry(StrictContractModel):
    node_id: ContractId
    module_id: ContractId
    asset_id: ContractId
    sha256: str
    logical_path: str
    mirror_axis: MirrorAxis = "none"
    transform: Transform

    @model_validator(mode="after")
    def validate_entry(self) -> "ExportModuleEntry":
        _validate_sha256(self.sha256)
        _validate_relative_path(self.logical_path)
        return self


class ExportFileEntry(StrictContractModel):
    path: str
    sha256: str
    byte_size: int = Field(ge=0)
    mime_type: str = Field(min_length=1)

    @model_validator(mode="after")
    def validate_entry(self) -> "ExportFileEntry":
        _validate_sha256(self.sha256)
        _validate_relative_path(self.path)
        return self


class ConceptExportManifest(StrictContractModel):
    schema_version: Literal["ConceptExportManifest@1"] = "ConceptExportManifest@1"
    export_id: ContractId
    project_id: ContractId
    version_id: ContractId
    profile: IntendedUse
    non_functional_only: Literal[True] = True
    spec_sha256: str
    graph_sha256: str
    modules: List[ExportModuleEntry] = Field(min_length=1)
    quality_report_id: Optional[ContractId] = None
    files: List[ExportFileEntry] = Field(min_length=1)
    created_at: str

    @model_validator(mode="after")
    def validate_manifest(self) -> "ConceptExportManifest":
        _validate_sha256(self.spec_sha256)
        _validate_sha256(self.graph_sha256)
        _require_unique("export node ids", [item.node_id for item in self.modules])
        _require_unique("export file paths", [item.path for item in self.files])
        return self


def _require_unique(label: str, values: List[Any]) -> None:
    rendered = [str(value) for value in values]
    if len(rendered) != len(set(rendered)):
        raise ValueError(f"{label} must be unique")


def _reachable_nodes(adjacency: Dict[str, Set[str]], root_node_id: str) -> Set[str]:
    visited: Set[str] = set()
    pending = [root_node_id]
    while pending:
        node_id = pending.pop()
        if node_id in visited:
            continue
        visited.add(node_id)
        pending.extend(adjacency[node_id] - visited)
    return visited


def _validate_sha256(value: str) -> None:
    if len(value) != 64 or any(char not in "0123456789abcdef" for char in value):
        raise ValueError("sha256 must be a lowercase hexadecimal digest")


def _validate_relative_path(value: str) -> None:
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or ".." in path.parts
        or "://" in value
        or "\\" in value
        or (len(value) >= 2 and value[0].isalpha() and value[1] == ":")
    ):
        raise ValueError("path must be a traversal-free relative path")
