//! Strict read-only adapters for the historical Concept data model.
//!
//! These adapters deliberately expose only the JSON contracts still consumed
//! by the desktop's explicit legacy-detail surface. They never reinterpret a
//! Concept Version as an `AgentAssetVersion`, never return storage paths and
//! never mutate a legacy row or materialize a Snapshot during a GET.

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, Row};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{open_connection, snapshot_from_connection, CoreRepository};
use crate::{
    semantic_sha256, ActiveDesign, ActiveDesignSnapshot, CoreError, CoreResult, ExportReference,
    ProjectStatus,
};

const MAX_LEGACY_PROFILE_BYTES: usize = 256 * 1024;
const MAX_LEGACY_SPEC_BYTES: usize = 512 * 1024;
const MAX_LEGACY_GRAPH_BYTES: usize = 2 * 1024 * 1024;
const MAX_LEGACY_VERSIONS: usize = 256;
const MAX_LEGACY_MODULE_MANIFEST_BYTES: usize = 256 * 1024;
const MAX_LEGACY_MODULE_CATALOG_PAGE: usize = 100;
const MAX_LEGACY_MODULE_GLB_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyModuleGlb {
    pub module_id: String,
    pub file_name: String,
    pub mime_type: String,
    pub sha256: String,
    pub byte_size: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyModuleConnector {
    connector_id: String,
    slot: String,
    connector_type: String,
    transform: LegacyTransform,
    scale_range: [f64; 2],
    #[serde(rename = "exclusive")]
    _exclusive: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyModuleManifest {
    schema_version: String,
    module_id: String,
    pack_id: String,
    category: String,
    asset_id: String,
    sha256: String,
    bounds_mm: [f64; 3],
    triangle_count: u64,
    material_slots: Vec<String>,
    connectors: Vec<LegacyModuleConnector>,
}

impl LegacyModuleManifest {
    fn is_valid(&self) -> bool {
        let connector_ids = self
            .connectors
            .iter()
            .map(|connector| connector.connector_id.as_str())
            .collect::<BTreeSet<_>>();
        let material_slots = self
            .material_slots
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        self.schema_version == "ModuleAssetManifest@1"
            && valid_id(&self.module_id)
            && valid_id(&self.pack_id)
            && valid_module_category(&self.category)
            && valid_id(&self.asset_id)
            && valid_sha256(&self.sha256)
            && self
                .bounds_mm
                .iter()
                .all(|value| value.is_finite() && *value > 0.0 && *value <= 100_000.0)
            && (1..=2_000_000).contains(&self.triangle_count)
            && self.material_slots.len() <= 64
            && material_slots.len() == self.material_slots.len()
            && self
                .material_slots
                .iter()
                .all(|slot| bounded_text(slot, 1, 120))
            && self.connectors.len() <= 128
            && connector_ids.len() == self.connectors.len()
            && self.connectors.iter().all(|connector| {
                valid_id(&connector.connector_id)
                    && bounded_text(&connector.slot, 1, 120)
                    && bounded_text(&connector.connector_type, 1, 120)
                    && connector.transform.is_valid()
                    && connector.scale_range[0].is_finite()
                    && connector.scale_range[1].is_finite()
                    && connector.scale_range[0] > 0.0
                    && connector.scale_range[1] >= connector.scale_range[0]
                    && connector.scale_range[1] <= 100.0
            })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyModuleCatalogMetadata {
    display_name: String,
    description: String,
    tags: Vec<String>,
    catalog_path: String,
    origin_claim: String,
    creator_name: String,
    review_status: String,
    reviewer_name: Option<String>,
    reviewed_at: Option<String>,
    review_note: Option<String>,
    updated_at: String,
}

impl LegacyModuleCatalogMetadata {
    fn is_valid(&self) -> bool {
        let normalized_tags = self
            .tags
            .iter()
            .map(|tag| tag.to_lowercase())
            .collect::<BTreeSet<_>>();
        let approved_review_is_valid = self.review_status != "approved"
            || self.reviewer_name.as_deref().is_some_and(|reviewer| {
                bounded_text(reviewer, 1, 120) && !reviewer.eq_ignore_ascii_case(&self.creator_name)
            }) && self
                .reviewed_at
                .as_deref()
                .is_some_and(|value| bounded_text(value, 1, 64));
        bounded_text(&self.display_name, 1, 120)
            && bounded_text(&self.description, 1, 500)
            && self.tags.len() <= 24
            && normalized_tags.len() == self.tags.len()
            && self.tags.iter().all(|tag| bounded_text(tag, 1, 80))
            && bounded_text(&self.catalog_path, 1, 180)
            && matches!(
                self.origin_claim.as_str(),
                "self_declared_original" | "third_party" | "unknown"
            )
            && bounded_text(&self.creator_name, 1, 120)
            && matches!(
                self.review_status.as_str(),
                "draft" | "pending_review" | "approved" | "restricted"
            )
            && self
                .reviewer_name
                .as_deref()
                .is_none_or(|value| bounded_text(value, 1, 120))
            && self
                .reviewed_at
                .as_deref()
                .is_none_or(|value| bounded_text(value, 1, 64))
            && self
                .review_note
                .as_deref()
                .is_none_or(|value| bounded_text(value, 1, 1_000))
            && bounded_text(&self.updated_at, 1, 128)
            && approved_review_is_valid
    }

    fn response_value(self) -> Value {
        json!({
            "display_name": self.display_name,
            "description": self.description,
            "tags": self.tags,
            "catalog_path": self.catalog_path,
            "origin_claim": self.origin_claim,
            "creator_name": self.creator_name,
            "review_status": self.review_status,
            "reviewer_name": self.reviewer_name,
            "reviewed_at": self.reviewed_at,
            "review_note": self.review_note,
            "updated_at": self.updated_at,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyProfile {
    schema_version: String,
    profile_id: String,
    domain_type: String,
    display_name: String,
    pack_id: String,
    intended_uses: Vec<String>,
    module_categories: Vec<String>,
    required_connectors: Vec<String>,
    optional_connectors: Vec<String>,
    export_profiles: Vec<String>,
    non_functional_only: bool,
}

impl LegacyProfile {
    fn is_valid(&self) -> bool {
        self.schema_version == "DesignDomainProfile@1"
            && valid_id(&self.profile_id)
            && self.domain_type == "weapon_concept"
            && bounded_text(&self.display_name, 1, 120)
            && valid_id(&self.pack_id)
            && valid_unique_values(&self.intended_uses, 1, 16)
            && valid_unique_values(&self.module_categories, 1, 32)
            && valid_unique_values(&self.required_connectors, 1, 32)
            && valid_unique_values(&self.optional_connectors, 0, 64)
            && valid_unique_values(&self.export_profiles, 1, 16)
            && self.non_functional_only
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyStyle {
    keywords: Vec<String>,
    palette: Vec<String>,
    detail_density: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyProportions {
    overall_length_mm: f64,
    body_height_mm: f64,
    grip_angle_deg: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyConstraints {
    symmetry: String,
    max_triangle_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacySpec {
    schema_version: String,
    project_id: String,
    profile_id: String,
    name: String,
    archetype: String,
    intended_uses: Vec<String>,
    style: LegacyStyle,
    proportions: LegacyProportions,
    required_slots: Vec<String>,
    optional_slots: Vec<String>,
    constraints: LegacyConstraints,
    assumptions: Vec<String>,
}

impl LegacySpec {
    fn is_valid(&self) -> bool {
        self.schema_version == "WeaponConceptSpec@1"
            && valid_id(&self.project_id)
            && valid_id(&self.profile_id)
            && bounded_text(&self.name, 1, 120)
            && self.archetype == "future_modular_sidearm"
            && valid_unique_values(&self.intended_uses, 1, 16)
            && valid_unique_values(&self.style.keywords, 1, 12)
            && valid_unique_values(&self.style.palette, 1, 8)
            && self.style.detail_density.is_finite()
            && (0.0..=1.0).contains(&self.style.detail_density)
            && self.proportions.overall_length_mm.is_finite()
            && self.proportions.overall_length_mm > 0.0
            && self.proportions.overall_length_mm <= 1_000.0
            && self.proportions.body_height_mm.is_finite()
            && self.proportions.body_height_mm > 0.0
            && self.proportions.body_height_mm <= 1_000.0
            && self.proportions.grip_angle_deg.is_finite()
            && (-45.0..=45.0).contains(&self.proportions.grip_angle_deg)
            && valid_unique_values(&self.required_slots, 1, 32)
            && valid_unique_values(&self.optional_slots, 0, 32)
            && matches!(
                self.constraints.symmetry.as_str(),
                "symmetric" | "mostly_symmetric" | "asymmetric"
            )
            && (1_000..=2_000_000).contains(&self.constraints.max_triangle_count)
            && !self.assumptions.is_empty()
            && self
                .assumptions
                .iter()
                .all(|value| bounded_text(value, 1, 2_000))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyTransform {
    position: [f64; 3],
    rotation: [f64; 3],
    scale: [f64; 3],
}

impl LegacyTransform {
    fn is_valid(&self) -> bool {
        self.position.iter().all(|value| value.is_finite())
            && self.rotation.iter().all(|value| value.is_finite())
            && self
                .scale
                .iter()
                .all(|value| value.is_finite() && *value > 0.0)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyGraphNode {
    node_id: String,
    module_id: String,
    transform: LegacyTransform,
    #[serde(default)]
    mirror_axis: Option<String>,
    locked: bool,
    visible: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyGraphEdge {
    edge_id: String,
    from_node_id: String,
    from_connector_id: String,
    to_node_id: String,
    to_connector_id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyGraph {
    schema_version: String,
    graph_id: String,
    project_id: String,
    root_node_id: String,
    nodes: Vec<LegacyGraphNode>,
    edges: Vec<LegacyGraphEdge>,
}

impl LegacyGraph {
    fn is_valid(&self) -> bool {
        if self.schema_version != "ModuleGraph@1"
            || !valid_id(&self.graph_id)
            || !valid_id(&self.project_id)
            || !valid_id(&self.root_node_id)
            || self.nodes.is_empty()
            || self.nodes.len() > 512
            || self.edges.len() > 1_024
        {
            return false;
        }
        let node_ids = self
            .nodes
            .iter()
            .map(|node| node.node_id.as_str())
            .collect::<BTreeSet<_>>();
        if node_ids.len() != self.nodes.len() || !node_ids.contains(self.root_node_id.as_str()) {
            return false;
        }
        if self.nodes.iter().any(|node| {
            !valid_id(&node.node_id)
                || !valid_id(&node.module_id)
                || !node.transform.is_valid()
                || node
                    .mirror_axis
                    .as_deref()
                    .is_some_and(|axis| !matches!(axis, "none" | "x" | "y" | "z"))
                || (!node.locked && !node.visible && node.node_id.is_empty())
        }) {
            return false;
        }
        let mut edge_ids = BTreeSet::new();
        self.edges.iter().all(|edge| {
            valid_id(&edge.edge_id)
                && edge_ids.insert(edge.edge_id.as_str())
                && node_ids.contains(edge.from_node_id.as_str())
                && node_ids.contains(edge.to_node_id.as_str())
                && valid_id(&edge.from_connector_id)
                && valid_id(&edge.to_connector_id)
                && matches!(edge.status.as_str(), "connected" | "invalid")
        })
    }
}

#[derive(Debug, Clone)]
struct LegacyVersion {
    version_id: String,
    project_id: String,
    parent_version_id: Option<String>,
    version_no: u64,
    status: String,
    summary: String,
    spec_schema_version: String,
    spec: Value,
    spec_sha256: String,
    module_graph_id: Option<String>,
    change_set_id: Option<String>,
    created_at: String,
}

impl LegacyVersion {
    fn summary_value(&self) -> Value {
        json!({
            "version_id": self.version_id,
            "parent_version_id": self.parent_version_id,
            "version_no": self.version_no,
            "status": self.status,
            "summary": self.summary,
            "spec_schema_version": self.spec_schema_version,
            "spec_sha256": self.spec_sha256,
            "module_graph_id": self.module_graph_id,
            "change_set_id": self.change_set_id,
            "created_at": self.created_at,
        })
    }

    fn detail_value(&self) -> Value {
        let mut value = self.summary_value();
        let object = value
            .as_object_mut()
            .expect("legacy version summary is an object");
        object.insert("project_id".into(), Value::String(self.project_id.clone()));
        object.insert("spec".into(), self.spec.clone());
        value
    }
}

#[derive(Debug, Clone)]
struct LegacyGraphRecord {
    graph_id: String,
    project_id: String,
    version_id: Option<String>,
    graph: Value,
    graph_sha256: String,
    validation_status: String,
    created_at: String,
    updated_at: String,
}

impl LegacyGraphRecord {
    fn response_value(&self) -> Value {
        json!({
            "graph": self.graph,
            "graph_sha256": self.graph_sha256,
            "validation_status": self.validation_status,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        })
    }
}

#[derive(Debug)]
struct RawLegacyModuleCatalogItem {
    module_id: String,
    pack_id: String,
    category: String,
    asset_id: String,
    schema_version: String,
    manifest_json: String,
    manifest_sha256: String,
    asset_sha256: String,
    byte_size: i64,
    mime_type: String,
    role: String,
    created_at: String,
    display_name: String,
    description: String,
    tags_json: String,
    catalog_path: String,
    origin_claim: String,
    creator_name: String,
    review_status: String,
    reviewer_name: Option<String>,
    reviewed_at: Option<String>,
    review_note: Option<String>,
    metadata_updated_at: String,
}

impl CoreRepository {
    pub fn legacy_project_detail(&self, project_id: &str) -> CoreResult<Option<Value>> {
        let Some(project) = self.project(project_id)? else {
            return Ok(None);
        };
        project.validate()?;
        if project.status == ProjectStatus::SoftDeleted || project.current_version_id.is_none() {
            return Ok(None);
        }
        let connection = open_connection(self.db_path())?;
        let profile_row: Option<(String, String, String, String, String)> = connection
            .query_row(
                "SELECT schema_version, pack_id, domain_type, profile_json, profile_sha256 FROM domain_profiles WHERE profile_id=? AND status!='disabled'",
                [&project.profile_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()?;
        let (profile_schema, pack_id, domain_type, profile_json, profile_sha256) =
            profile_row.ok_or_else(|| CoreError::not_found("legacy domain profile"))?;
        let (profile, profile_value) = decode_verified_document::<LegacyProfile>(
            &profile_json,
            &profile_sha256,
            MAX_LEGACY_PROFILE_BYTES,
            "LEGACY_PROFILE_INVALID",
            "Historical Domain Profile",
        )?;
        if !profile.is_valid()
            || profile_schema != "DesignDomainProfile@1"
            || profile.profile_id != project.profile_id
            || profile.pack_id != pack_id
            || profile.domain_type != domain_type
            || project.domain_type != domain_type
        {
            return Err(invalid_legacy_document(
                "LEGACY_PROFILE_INVALID",
                "Historical Domain Profile identity does not match its Project row.",
            ));
        }

        let mut statement = connection.prepare(
            "SELECT version_id, project_id, parent_version_id, version_no, status, summary, spec_schema_version, spec_json, spec_sha256, module_graph_id, change_set_id, created_at FROM project_versions WHERE project_id=? AND status!='soft_deleted' ORDER BY version_no, created_at, version_id LIMIT ?",
        )?;
        let rows = statement
            .query_map(
                rusqlite::params![project_id, (MAX_LEGACY_VERSIONS + 1) as i64],
                raw_legacy_version_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.len() > MAX_LEGACY_VERSIONS {
            return Err(invalid_legacy_document(
                "LEGACY_VERSION_LIST_TOO_LARGE",
                "Historical Version list exceeds the bounded read-only adapter.",
            ));
        }
        let versions = rows
            .into_iter()
            .map(verify_legacy_version)
            .collect::<CoreResult<Vec<_>>>()?;
        let current_version_id = project
            .current_version_id
            .as_deref()
            .expect("checked current legacy version");
        let current = versions
            .iter()
            .find(|version| version.version_id == current_version_id)
            .ok_or_else(|| CoreError::not_found("current legacy Concept version"))?;
        let current_spec: LegacySpec =
            serde_json::from_value(current.spec.clone()).map_err(|_| {
                invalid_legacy_document(
                    "LEGACY_SPEC_INVALID",
                    "Historical current Concept Spec failed strict decoding.",
                )
            })?;
        if current.project_id != project.project_id
            || current_spec.project_id != project.project_id
            || current_spec.profile_id != project.profile_id
        {
            return Err(invalid_legacy_document(
                "LEGACY_SPEC_INVALID",
                "Historical current Concept Spec identity does not match its Project.",
            ));
        }
        Ok(Some(json!({
            "project_id": project.project_id,
            "profile_id": project.profile_id,
            "domain_type": project.domain_type,
            "name": project.name,
            "status": project.status,
            "current_version_id": project.current_version_id,
            "created_at": project.created_at,
            "updated_at": project.updated_at,
            "profile": profile_value,
            "current_spec": current.spec,
            "versions": versions.iter().map(LegacyVersion::summary_value).collect::<Vec<_>>(),
        })))
    }

    pub fn legacy_version_detail(&self, version_id: &str) -> CoreResult<Option<Value>> {
        let connection = open_connection(self.db_path())?;
        legacy_version_from_connection(&connection, version_id)
            .map(|version| version.map(|version| version.detail_value()))
    }

    pub fn legacy_module_graph_detail(&self, graph_id: &str) -> CoreResult<Option<Value>> {
        let connection = open_connection(self.db_path())?;
        legacy_graph_from_connection(&connection, graph_id)
            .map(|record| record.map(|record| record.response_value()))
    }

    /// Lists only database-registered historical modules from one explicit
    /// Domain Pack. The response is cursor-paged and intentionally omits both
    /// logical and physical storage paths; callers reach bytes exclusively via
    /// [`Self::legacy_module_glb`], which repeats the CAS and digest checks.
    #[allow(clippy::too_many_arguments)]
    pub fn legacy_module_catalog(
        &self,
        pack_id: &str,
        category: Option<&str>,
        query: Option<&str>,
        review_status: Option<&str>,
        tag: Option<&str>,
        catalog_path: Option<&str>,
        cursor: Option<&str>,
        limit: usize,
    ) -> CoreResult<Value> {
        validate_legacy_module_catalog_query(
            pack_id,
            category,
            query,
            review_status,
            tag,
            catalog_path,
            cursor,
            limit,
        )?;
        let connection = open_connection(self.db_path())?;
        let scan_limit = (limit + 1) as i64;
        let mut statement = connection.prepare(
            "SELECT m.module_id, m.pack_id, m.category, m.asset_id, m.schema_version, m.manifest_json, m.manifest_sha256, ca.sha256, ca.byte_size, ca.mime_type, ca.role, ca.created_at, md.display_name, md.description, md.tags_json, md.catalog_path, md.origin_claim, md.creator_name, md.review_status, md.reviewer_name, md.reviewed_at, md.review_note, md.updated_at FROM module_assets m JOIN concept_assets ca ON ca.asset_id=m.asset_id JOIN module_asset_catalog_metadata md ON md.module_id=m.module_id WHERE m.pack_id=?1 AND m.status='active' AND ca.soft_deleted_at IS NULL AND (?2 IS NULL OR m.category=?2) AND (?3 IS NULL OR instr(lower(m.module_id || ' ' || md.display_name || ' ' || md.description), lower(?3)) > 0) AND (?4 IS NULL OR md.review_status=?4) AND (?5 IS NULL OR EXISTS (SELECT 1 FROM json_each(md.tags_json) AS catalog_tag WHERE lower(CAST(catalog_tag.value AS TEXT))=lower(?5))) AND (?6 IS NULL OR md.catalog_path=?6) AND (?7 IS NULL OR m.module_id>?7) ORDER BY m.module_id LIMIT ?8",
        )?;
        let rows = statement
            .query_map(
                rusqlite::params![
                    pack_id,
                    category,
                    query,
                    review_status,
                    tag,
                    catalog_path,
                    cursor,
                    scan_limit,
                ],
                raw_legacy_module_catalog_item_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let mut verified = rows
            .into_iter()
            .map(verify_legacy_module_catalog_item)
            .collect::<CoreResult<Vec<_>>>()?;
        let has_more = verified.len() > limit;
        if has_more {
            verified.pop();
        }
        let next_cursor = has_more
            .then(|| verified.last().map(|(module_id, _)| module_id.clone()))
            .flatten();
        Ok(json!({
            "items": verified.into_iter().map(|(_, item)| item).collect::<Vec<_>>(),
            "pack_id": pack_id,
            "category": category,
            "next_cursor": next_cursor,
        }))
    }

    /// Returns the persisted Snapshot or a stable, validated legacy read model.
    /// The derived legacy value is intentionally not inserted by this GET path;
    /// an explicit conversion POST may materialize it together with its intent.
    pub fn snapshot_or_legacy_read_only(
        &self,
        project_id: &str,
    ) -> CoreResult<Option<ActiveDesignSnapshot>> {
        let connection = open_connection(self.db_path())?;
        if let Some(snapshot) = snapshot_from_connection(&connection, project_id)? {
            return Ok(Some(snapshot));
        }
        let Some(project) = self.project(project_id)? else {
            return Ok(None);
        };
        project.validate()?;
        if project.status == ProjectStatus::SoftDeleted {
            return Ok(None);
        }
        let agent_head: Option<String> = connection
            .query_row(
                "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
                [project_id],
                |row| row.get(0),
            )
            .optional()?;
        if agent_head.is_some() {
            return Err(CoreError::conflict(
                "ACTIVE_DESIGN_HEAD_WITHOUT_SNAPSHOT",
                "An Agent head exists without its authoritative Snapshot.",
            ));
        }
        let Some(version_id) = project.current_version_id.as_deref() else {
            return Ok(None);
        };
        let Some(version) = legacy_version_from_connection(&connection, version_id)? else {
            return Err(CoreError::conflict(
                "ACTIVE_DESIGN_LEGACY_SOURCE_INVALID",
                "The Project current legacy Version is unavailable.",
            ));
        };
        if version.project_id != project_id || version.status != "committed" {
            return Err(CoreError::conflict(
                "ACTIVE_DESIGN_LEGACY_SOURCE_INVALID",
                "The Project current legacy Version is not a committed Project-owned source.",
            ));
        }
        let Some(graph_id) = version.module_graph_id.as_deref() else {
            return Ok(None);
        };
        let graph = legacy_graph_from_connection(&connection, graph_id)?.ok_or_else(|| {
            CoreError::conflict(
                "ACTIVE_DESIGN_LEGACY_SOURCE_INVALID",
                "The current legacy Version references an unavailable ModuleGraph.",
            )
        })?;
        if graph.project_id != project_id
            || graph.version_id.as_deref() != Some(version_id)
            || graph.validation_status != "valid"
        {
            return Err(CoreError::conflict(
                "ACTIVE_DESIGN_LEGACY_SOURCE_INVALID",
                "The current legacy ModuleGraph is not a valid Version-bound Project source.",
            ));
        }
        let snapshot = ActiveDesignSnapshot {
            schema_version: "ActiveDesignSnapshot@1".into(),
            project_id: project_id.into(),
            active_design: ActiveDesign::LegacyConceptReadOnly {
                project_id: project_id.into(),
                legacy_version_id: version_id.into(),
                module_graph_id: graph.graph_id,
            },
            selected_part_id: None,
            selected_material_zone_id: None,
            preview: None,
            quality: None,
            export: ExportReference::LegacyConceptReadOnly {
                project_id: project_id.into(),
                source_version_id: version_id.into(),
            },
            render_preset: None,
            part_display: None,
            revision: 1,
            updated_at: project.updated_at,
        };
        snapshot.validate()?;
        Ok(Some(snapshot))
    }

    /// Reads one historical module GLB only after verifying the exact legacy
    /// CAS path, bounded size, byte count and SHA-256. The path itself never
    /// leaves this method.
    pub fn legacy_module_glb(&self, module_id: &str) -> CoreResult<Option<LegacyModuleGlb>> {
        if !valid_id(module_id) {
            return Err(invalid_legacy_document(
                "LEGACY_MODULE_ID_INVALID",
                "Historical Module ID is outside the bounded identifier contract.",
            ));
        }
        let connection = open_connection(self.db_path())?;
        let row: Option<(String, String, String, String, i64, String, String)> = connection
            .query_row(
                "SELECT m.status, ca.role, ca.object_path, ca.sha256, ca.byte_size, ca.mime_type, ca.asset_id FROM module_assets m JOIN concept_assets ca ON ca.asset_id=m.asset_id WHERE m.module_id=? AND m.status!='soft_deleted' AND ca.soft_deleted_at IS NULL",
                [module_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
            )
            .optional()?;
        let Some((status, role, object_path, sha256, byte_size, mime_type, asset_id)) = row else {
            return Ok(None);
        };
        if !matches!(status.as_str(), "active" | "disabled")
            || role != "module_glb"
            || mime_type != "model/gltf-binary"
            || !valid_id(&asset_id)
            || byte_size <= 0
            || byte_size as u64 > MAX_LEGACY_MODULE_GLB_BYTES
            || object_path.len() > 500
        {
            return Err(invalid_legacy_document(
                "LEGACY_MODULE_ASSET_INVALID",
                "Historical Module asset metadata is outside the read-only GLB contract.",
            ));
        }
        let stored = self.object_store.adopt_existing_legacy_object(
            &object_path,
            &sha256,
            byte_size as u64,
            "glb",
        )?;
        let bytes = self.object_store.read(&stored)?;
        validate_legacy_glb(&bytes)?;
        Ok(Some(LegacyModuleGlb {
            module_id: module_id.into(),
            file_name: format!("{module_id}.glb"),
            mime_type,
            sha256,
            byte_size: byte_size as u64,
            bytes,
        }))
    }
}

fn raw_legacy_module_catalog_item_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<RawLegacyModuleCatalogItem> {
    Ok(RawLegacyModuleCatalogItem {
        module_id: row.get(0)?,
        pack_id: row.get(1)?,
        category: row.get(2)?,
        asset_id: row.get(3)?,
        schema_version: row.get(4)?,
        manifest_json: row.get(5)?,
        manifest_sha256: row.get(6)?,
        asset_sha256: row.get(7)?,
        byte_size: row.get(8)?,
        mime_type: row.get(9)?,
        role: row.get(10)?,
        created_at: row.get(11)?,
        display_name: row.get(12)?,
        description: row.get(13)?,
        tags_json: row.get(14)?,
        catalog_path: row.get(15)?,
        origin_claim: row.get(16)?,
        creator_name: row.get(17)?,
        review_status: row.get(18)?,
        reviewer_name: row.get(19)?,
        reviewed_at: row.get(20)?,
        review_note: row.get(21)?,
        metadata_updated_at: row.get(22)?,
    })
}

fn verify_legacy_module_catalog_item(
    row: RawLegacyModuleCatalogItem,
) -> CoreResult<(String, Value)> {
    let (manifest, manifest_value) = decode_verified_document::<LegacyModuleManifest>(
        &row.manifest_json,
        &row.manifest_sha256,
        MAX_LEGACY_MODULE_MANIFEST_BYTES,
        "LEGACY_MODULE_MANIFEST_INVALID",
        "Historical Module Manifest",
    )?;
    let tags = decode_bounded_tags(&row.tags_json)?;
    let metadata = LegacyModuleCatalogMetadata {
        display_name: row.display_name,
        description: row.description,
        tags,
        catalog_path: row.catalog_path,
        origin_claim: row.origin_claim,
        creator_name: row.creator_name,
        review_status: row.review_status,
        reviewer_name: row.reviewer_name,
        reviewed_at: row.reviewed_at,
        review_note: row.review_note,
        updated_at: row.metadata_updated_at,
    };
    if !manifest.is_valid()
        || !metadata.is_valid()
        || row.schema_version != "ModuleAssetManifest@1"
        || manifest.module_id != row.module_id
        || manifest.pack_id != row.pack_id
        || manifest.category != row.category
        || manifest.asset_id != row.asset_id
        || manifest.sha256 != row.asset_sha256
        || row.role != "module_glb"
        || row.mime_type != "model/gltf-binary"
        || row.byte_size <= 0
        || row.byte_size as u64 > MAX_LEGACY_MODULE_GLB_BYTES
        || !bounded_text(&row.created_at, 1, 128)
    {
        return Err(invalid_legacy_document(
            "LEGACY_MODULE_CATALOG_INVALID",
            "Historical Module catalog metadata failed the strict read-only contract.",
        ));
    }
    let module_id = row.module_id;
    Ok((
        module_id,
        json!({
            "manifest": manifest_value,
            "byte_size": row.byte_size,
            "mime_type": row.mime_type,
            "created_at": row.created_at,
            "catalog_metadata": metadata.response_value(),
        }),
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_legacy_module_catalog_query(
    pack_id: &str,
    category: Option<&str>,
    query: Option<&str>,
    review_status: Option<&str>,
    tag: Option<&str>,
    catalog_path: Option<&str>,
    cursor: Option<&str>,
    limit: usize,
) -> CoreResult<()> {
    if !valid_id(pack_id)
        || category.is_some_and(|value| !valid_module_category(value))
        || query.is_some_and(|value| !bounded_text(value, 1, 200))
        || review_status.is_some_and(|value| {
            !matches!(
                value,
                "draft" | "pending_review" | "approved" | "restricted"
            )
        })
        || tag.is_some_and(|value| !bounded_text(value, 1, 80))
        || catalog_path.is_some_and(|value| !bounded_text(value, 1, 180))
        || cursor.is_some_and(|value| !valid_id(value))
        || !(1..=MAX_LEGACY_MODULE_CATALOG_PAGE).contains(&limit)
    {
        return Err(invalid_legacy_document(
            "LEGACY_MODULE_CATALOG_QUERY_INVALID",
            "Historical Module catalog query is outside the bounded read-only contract.",
        ));
    }
    Ok(())
}

fn decode_bounded_tags(encoded: &str) -> CoreResult<Vec<String>> {
    if encoded.len() > 16 * 1024 {
        return Err(invalid_legacy_document(
            "LEGACY_MODULE_CATALOG_INVALID",
            "Historical Module catalog tags exceed the bounded read contract.",
        ));
    }
    serde_json::from_str(encoded).map_err(|_| {
        invalid_legacy_document(
            "LEGACY_MODULE_CATALOG_INVALID",
            "Historical Module catalog tags are not a strict string array.",
        )
    })
}

type RawLegacyVersion = (
    String,
    String,
    Option<String>,
    i64,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
);

fn raw_legacy_version_from_row(row: &Row<'_>) -> rusqlite::Result<RawLegacyVersion> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
    ))
}

fn legacy_version_from_connection(
    connection: &Connection,
    version_id: &str,
) -> CoreResult<Option<LegacyVersion>> {
    let raw = connection
        .query_row(
            "SELECT version_id, project_id, parent_version_id, version_no, status, summary, spec_schema_version, spec_json, spec_sha256, module_graph_id, change_set_id, created_at FROM project_versions WHERE version_id=? AND status!='soft_deleted'",
            [version_id],
            raw_legacy_version_from_row,
        )
        .optional()?;
    raw.map(verify_legacy_version).transpose()
}

fn verify_legacy_version(raw: RawLegacyVersion) -> CoreResult<LegacyVersion> {
    let (
        version_id,
        project_id,
        parent_version_id,
        version_no,
        status,
        summary,
        spec_schema_version,
        spec_json,
        spec_sha256,
        module_graph_id,
        change_set_id,
        created_at,
    ) = raw;
    let (spec, spec_value) = decode_verified_document::<LegacySpec>(
        &spec_json,
        &spec_sha256,
        MAX_LEGACY_SPEC_BYTES,
        "LEGACY_SPEC_INVALID",
        "Historical Concept Spec",
    )?;
    if !spec.is_valid()
        || spec_schema_version != "WeaponConceptSpec@1"
        || spec.project_id != project_id
        || !valid_id(&version_id)
        || !valid_id(&project_id)
        || parent_version_id.as_deref().is_some_and(|id| !valid_id(id))
        || version_no <= 0
        || !matches!(status.as_str(), "draft" | "committed" | "superseded")
        || !bounded_text(&summary, 1, 500)
        || module_graph_id.as_deref().is_some_and(|id| !valid_id(id))
        || change_set_id.as_deref().is_some_and(|id| !valid_id(id))
        || !bounded_text(&created_at, 1, 128)
    {
        return Err(invalid_legacy_document(
            "LEGACY_VERSION_INVALID",
            "Historical Concept Version failed the strict read-only contract.",
        ));
    }
    Ok(LegacyVersion {
        version_id,
        project_id,
        parent_version_id,
        version_no: version_no as u64,
        status,
        summary,
        spec_schema_version,
        spec: spec_value,
        spec_sha256,
        module_graph_id,
        change_set_id,
        created_at,
    })
}

fn legacy_graph_from_connection(
    connection: &Connection,
    graph_id: &str,
) -> CoreResult<Option<LegacyGraphRecord>> {
    let row: Option<(
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        String,
    )> = connection
        .query_row(
            "SELECT graph_id, project_id, version_id, schema_version, graph_json, graph_sha256, validation_status, created_at, updated_at FROM module_graphs WHERE graph_id=?",
            [graph_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        graph_id,
        project_id,
        version_id,
        schema_version,
        graph_json,
        graph_sha256,
        validation_status,
        created_at,
        updated_at,
    )) = row
    else {
        return Ok(None);
    };
    let (graph, graph_value) = decode_verified_document::<LegacyGraph>(
        &graph_json,
        &graph_sha256,
        MAX_LEGACY_GRAPH_BYTES,
        "LEGACY_MODULE_GRAPH_INVALID",
        "Historical ModuleGraph",
    )?;
    if !graph.is_valid()
        || schema_version != "ModuleGraph@1"
        || graph.graph_id != graph_id
        || graph.project_id != project_id
        || version_id.as_deref().is_some_and(|id| !valid_id(id))
        || !matches!(validation_status.as_str(), "pending" | "valid" | "invalid")
        || !bounded_text(&created_at, 1, 128)
        || !bounded_text(&updated_at, 1, 128)
    {
        return Err(invalid_legacy_document(
            "LEGACY_MODULE_GRAPH_INVALID",
            "Historical ModuleGraph identity failed the strict read-only contract.",
        ));
    }
    Ok(Some(LegacyGraphRecord {
        graph_id,
        project_id,
        version_id,
        graph: graph_value,
        graph_sha256,
        validation_status,
        created_at,
        updated_at,
    }))
}

/// Transaction-local source validation shared by explicit conversion and
/// first-Agent-asset promotion. This recomputes every persisted semantic hash
/// before a legacy source can authorize a write; relational IDs alone are not
/// sufficient.
pub(super) fn validate_legacy_source_binding(
    connection: &Connection,
    project_id: &str,
    legacy_version_id: &str,
    module_graph_id: &str,
) -> CoreResult<()> {
    let project: Option<(String, String, String, Option<String>)> = connection
        .query_row(
            "SELECT profile_id, domain_type, status, current_version_id FROM projects WHERE project_id=?",
            [project_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((profile_id, project_domain, project_status, current_version_id)) = project else {
        return Err(CoreError::conflict(
            "LEGACY_CONVERSION_SOURCE_INVALID",
            "Legacy conversion Project is unavailable.",
        ));
    };
    let version =
        legacy_version_from_connection(connection, legacy_version_id)?.ok_or_else(|| {
            CoreError::conflict(
                "LEGACY_CONVERSION_SOURCE_INVALID",
                "Legacy conversion Version is unavailable.",
            )
        })?;
    let graph = legacy_graph_from_connection(connection, module_graph_id)?.ok_or_else(|| {
        CoreError::conflict(
            "LEGACY_CONVERSION_SOURCE_INVALID",
            "Legacy conversion ModuleGraph is unavailable.",
        )
    })?;
    let profile_row: Option<(String, String, String, String, String)> = connection
        .query_row(
            "SELECT schema_version, pack_id, domain_type, profile_json, profile_sha256 FROM domain_profiles WHERE profile_id=? AND status!='disabled'",
            [&profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((profile_schema, pack_id, profile_domain, profile_json, profile_sha256)) = profile_row
    else {
        return Err(CoreError::conflict(
            "LEGACY_CONVERSION_SOURCE_INVALID",
            "Legacy conversion Domain Profile is unavailable.",
        ));
    };
    let (profile, _) = decode_verified_document::<LegacyProfile>(
        &profile_json,
        &profile_sha256,
        MAX_LEGACY_PROFILE_BYTES,
        "LEGACY_PROFILE_INVALID",
        "Historical Domain Profile",
    )?;
    let spec: LegacySpec = serde_json::from_value(version.spec.clone()).map_err(|_| {
        invalid_legacy_document(
            "LEGACY_SPEC_INVALID",
            "Historical Concept Spec failed strict conversion-source decoding.",
        )
    })?;
    if !profile.is_valid()
        || !spec.is_valid()
        || profile_schema != "DesignDomainProfile@1"
        || profile.profile_id != profile_id
        || profile.pack_id != pack_id
        || profile.domain_type != profile_domain
        || profile_domain != project_domain
        || project_status == "soft_deleted"
        || current_version_id.as_deref() != Some(legacy_version_id)
        || version.project_id != project_id
        || version.status != "committed"
        || version.module_graph_id.as_deref() != Some(module_graph_id)
        || spec.project_id != project_id
        || spec.profile_id != profile_id
        || graph.project_id != project_id
        || graph.version_id.as_deref() != Some(legacy_version_id)
        || graph.validation_status != "valid"
    {
        return Err(CoreError::conflict(
            "LEGACY_CONVERSION_SOURCE_INVALID",
            "Legacy conversion source identities or validated semantic documents no longer match.",
        ));
    }
    Ok(())
}

fn decode_verified_document<T: DeserializeOwned>(
    encoded: &str,
    expected_sha256: &str,
    max_bytes: usize,
    error_code: &'static str,
    label: &str,
) -> CoreResult<(T, Value)> {
    if encoded.is_empty() || encoded.len() > max_bytes {
        return Err(invalid_legacy_document(
            error_code,
            format!("{label} exceeds the bounded read-only contract."),
        ));
    }
    let value: Value = serde_json::from_str(encoded)
        .map_err(|_| invalid_legacy_document(error_code, format!("{label} is not valid JSON.")))?;
    if !value.is_object() || semantic_sha256(&value)? != expected_sha256 {
        return Err(invalid_legacy_document(
            "LEGACY_SEMANTIC_HASH_MISMATCH",
            format!("{label} does not match its persisted semantic SHA-256."),
        ));
    }
    let decoded = serde_json::from_value(value.clone()).map_err(|_| {
        invalid_legacy_document(
            error_code,
            format!("{label} contains fields outside its frozen schema."),
        )
    })?;
    Ok((decoded, value))
}

fn invalid_legacy_document(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message)
}

fn bounded_text(value: &str, min: usize, max: usize) -> bool {
    let count = value.chars().count();
    count >= min && count <= max && !value.contains('\0')
}

fn valid_id(value: &str) -> bool {
    if !bounded_text(value, 3, 160) {
        return false;
    }
    let Some((prefix, suffix)) = value.split_once('_') else {
        return false;
    };
    matches!(
        prefix,
        "prj"
            | "profile"
            | "pack"
            | "module"
            | "connector"
            | "mg"
            | "node"
            | "edge"
            | "change"
            | "quality"
            | "finding"
            | "job"
            | "evt"
            | "ver"
            | "asset"
            | "assetver"
            | "assetcs"
            | "export"
    ) && !suffix.is_empty()
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_module_category(value: &str) -> bool {
    matches!(
        value,
        "core_shell"
            | "front_shell"
            | "rear_shell"
            | "grip_shell"
            | "top_accessory"
            | "side_accessory"
            | "lower_structure"
            | "storage_visual"
            | "armor_panel"
    )
}

fn valid_unique_values(values: &[String], min: usize, max: usize) -> bool {
    values.len() >= min
        && values.len() <= max
        && values.iter().all(|value| bounded_text(value, 1, 160))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn validate_legacy_glb(bytes: &[u8]) -> CoreResult<()> {
    if bytes.len() < 20
        || bytes.len() as u64 > MAX_LEGACY_MODULE_GLB_BYTES
        || bytes.get(..4) != Some(b"glTF")
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap_or_default()) != 2
        || u32::from_le_bytes(bytes[8..12].try_into().unwrap_or_default()) as usize != bytes.len()
    {
        return Err(invalid_legacy_document(
            "LEGACY_MODULE_GLB_INVALID",
            "Historical Module asset is not a bounded complete GLB 2.0 container.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::params;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::*;
    use crate::{Project, ProjectStatus};

    #[test]
    fn legacy_read_models_preserve_semantic_hash_and_zero_write_across_restart() {
        let root = TempDir::new().unwrap();
        let db = root.path().join("library.db");
        let repository = CoreRepository::open(&db, root.path(), "legacy-read-first").unwrap();
        repository
            .ensure_default_domain_profile("2026-07-17T00:00:00Z")
            .unwrap();
        let fixture = seed_legacy_fixture(&repository, root.path());
        let semantic_before = repository
            .legacy_read_only_hash(&fixture.project_id)
            .unwrap()
            .unwrap();
        let sentinel = open_connection(&db).unwrap();
        let data_version_before: i64 = sentinel
            .query_row("PRAGMA data_version", [], |row| row.get(0))
            .unwrap();

        let detail = repository
            .legacy_project_detail(&fixture.project_id)
            .unwrap()
            .unwrap();
        assert_eq!(detail["current_spec"], fixture.spec);
        assert_eq!(detail["versions"][0]["spec_sha256"], fixture.spec_sha256);
        let version = repository
            .legacy_version_detail(&fixture.version_id)
            .unwrap()
            .unwrap();
        assert_eq!(version["spec"], fixture.spec);
        let graph = repository
            .legacy_module_graph_detail(&fixture.graph_id)
            .unwrap()
            .unwrap();
        assert_eq!(graph["graph"], fixture.graph);
        let snapshot = repository
            .snapshot_or_legacy_read_only(&fixture.project_id)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.active_design.asset_version_id(), None);
        assert_eq!(snapshot.export.source_version_id(), fixture.version_id);
        let snapshot_hash = snapshot.semantic_hash().unwrap();
        let module = repository
            .legacy_module_glb(&fixture.module_id)
            .unwrap()
            .unwrap();
        assert_eq!(module.bytes, fixture.glb);
        assert_eq!(module.sha256, fixture.glb_sha256);

        let serialized =
            serde_json::to_string(&(detail.clone(), version.clone(), graph.clone())).unwrap();
        assert!(!serialized.contains("object_path"));
        assert!(!serialized.contains(root.path().to_string_lossy().as_ref()));
        let snapshot_rows: i64 = sentinel
            .query_row(
                "SELECT COUNT(*) FROM active_design_snapshots WHERE project_id=?",
                [&fixture.project_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            snapshot_rows, 0,
            "GET recovery must not materialize a Snapshot"
        );
        let data_version_after: i64 = sentinel
            .query_row("PRAGMA data_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            data_version_after, data_version_before,
            "read-only adapters must not commit through another connection"
        );
        assert_eq!(
            repository
                .legacy_read_only_hash(&fixture.project_id)
                .unwrap()
                .unwrap(),
            semantic_before
        );

        drop(sentinel);
        drop(repository);
        let restarted = CoreRepository::open(&db, root.path(), "legacy-read-restart").unwrap();
        assert_eq!(
            restarted
                .legacy_project_detail(&fixture.project_id)
                .unwrap()
                .unwrap(),
            detail
        );
        assert_eq!(
            restarted
                .snapshot_or_legacy_read_only(&fixture.project_id)
                .unwrap()
                .unwrap()
                .semantic_hash()
                .unwrap(),
            snapshot_hash
        );
        assert_eq!(
            restarted
                .legacy_module_glb(&fixture.module_id)
                .unwrap()
                .unwrap()
                .bytes,
            fixture.glb
        );
        assert_eq!(
            restarted
                .legacy_read_only_hash(&fixture.project_id)
                .unwrap()
                .unwrap(),
            semantic_before
        );
        let connection = open_connection(&db).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM active_design_snapshots WHERE project_id=?",
                    [&fixture.project_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn legacy_adapter_rejects_semantic_hash_drift_and_non_cas_paths() {
        let root = TempDir::new().unwrap();
        let db = root.path().join("library.db");
        let repository = CoreRepository::open(&db, root.path(), "legacy-read-invalid").unwrap();
        repository
            .ensure_default_domain_profile("2026-07-17T00:00:00Z")
            .unwrap();
        let fixture = seed_legacy_fixture(&repository, root.path());
        let connection = open_connection(&db).unwrap();
        connection
            .execute(
                "UPDATE project_versions SET spec_sha256=? WHERE version_id=?",
                params!["0".repeat(64), fixture.version_id],
            )
            .unwrap();
        let error = repository
            .legacy_version_detail(&fixture.version_id)
            .unwrap_err();
        assert_eq!(error.code(), "LEGACY_SEMANTIC_HASH_MISMATCH");
        connection
            .execute(
                "UPDATE project_versions SET spec_sha256=? WHERE version_id=?",
                params![fixture.spec_sha256, fixture.version_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE concept_assets SET object_path='../../outside.glb' WHERE asset_id='asset_legacy_module_glb'",
                [],
            )
            .unwrap();
        let error = repository
            .legacy_module_glb(&fixture.module_id)
            .unwrap_err();
        assert_eq!(error.code(), "LEGACY_OBJECT_PATH_INVALID");
    }

    struct LegacyFixture {
        project_id: String,
        version_id: String,
        graph_id: String,
        module_id: String,
        spec: Value,
        spec_sha256: String,
        graph: Value,
        glb: Vec<u8>,
        glb_sha256: String,
    }

    fn seed_legacy_fixture(repository: &CoreRepository, root: &std::path::Path) -> LegacyFixture {
        let project_id = "prj_legacy_adapter".to_string();
        let version_id = "ver_legacy_adapter_v1".to_string();
        let graph_id = "mg_legacy_adapter_v1".to_string();
        let module_id = "module_legacy_shell".to_string();
        repository
            .create_project(&Project {
                project_id: project_id.clone(),
                profile_id: "profile_weapon_concept_v1".into(),
                domain_type: "weapon_concept".into(),
                name: "Legacy read-only concept".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "2026-07-17T00:00:01Z".into(),
                updated_at: "2026-07-17T00:00:02Z".into(),
            })
            .unwrap();
        let spec = json!({
            "schema_version": "WeaponConceptSpec@1",
            "project_id": project_id,
            "profile_id": "profile_weapon_concept_v1",
            "name": "Legacy read-only concept",
            "archetype": "future_modular_sidearm",
            "intended_uses": ["game_asset", "film_prop", "non_functional_display"],
            "style": {"keywords": ["future", "non-functional"], "palette": ["graphite"], "detail_density": 0.7},
            "proportions": {"overall_length_mm": 230.0, "body_height_mm": 54.0, "grip_angle_deg": 15.0},
            "required_slots": ["core"],
            "optional_slots": [],
            "constraints": {"symmetry": "mostly_symmetric", "max_triangle_count": 180000},
            "assumptions": ["Fictional non-functional visual concept only"]
        });
        let graph = json!({
            "schema_version": "ModuleGraph@1",
            "graph_id": graph_id,
            "project_id": project_id,
            "root_node_id": "node_legacy_shell",
            "nodes": [{
                "node_id": "node_legacy_shell",
                "module_id": module_id,
                "transform": {"position": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0]},
                "mirror_axis": "none",
                "locked": false,
                "visible": true
            }],
            "edges": []
        });
        let spec_sha256 = semantic_sha256(&spec).unwrap();
        let graph_sha256 = semantic_sha256(&graph).unwrap();
        let glb = minimal_glb();
        let glb_sha256 = hex_sha256(&glb);
        let relative = format!(
            "objects/sha256/{}/{}/{}.glb",
            &glb_sha256[..2],
            &glb_sha256[2..4],
            glb_sha256
        );
        let object_path = root.join(&relative);
        fs::create_dir_all(object_path.parent().unwrap()).unwrap();
        fs::write(&object_path, &glb).unwrap();
        let manifest = json!({"schema_version": "ModuleAssetManifest@1", "module_id": module_id});
        let manifest_sha256 = semantic_sha256(&manifest).unwrap();
        let connection = open_connection(repository.db_path()).unwrap();
        connection
            .execute(
                "INSERT INTO project_versions(version_id, project_id, parent_version_id, version_no, status, summary, spec_schema_version, spec_json, spec_sha256, module_graph_id, change_set_id, created_at) VALUES (?, ?, NULL, 1, 'committed', 'Legacy immutable concept', 'WeaponConceptSpec@1', ?, ?, ?, NULL, '2026-07-17T00:00:03Z')",
                params![version_id, project_id, spec.to_string(), spec_sha256, graph_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO concept_assets(asset_id, project_id, version_id, role, logical_path, object_path, sha256, byte_size, mime_type, metadata_json, created_at, soft_deleted_at) VALUES ('asset_legacy_module_glb', ?, ?, 'module_glb', 'modules/legacy_shell.glb', ?, ?, ?, 'model/gltf-binary', '{}', '2026-07-17T00:00:03Z', NULL)",
                params![project_id, version_id, relative, glb_sha256, glb.len() as i64],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO module_assets(module_id, pack_id, category, asset_id, schema_version, manifest_json, manifest_sha256, status, created_at, updated_at) VALUES (?, 'pack_weapon_concept_v1', 'core_shell', 'asset_legacy_module_glb', 'ModuleAssetManifest@1', ?, ?, 'active', '2026-07-17T00:00:03Z', '2026-07-17T00:00:03Z')",
                params![module_id, manifest.to_string(), manifest_sha256],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO module_graphs(graph_id, project_id, version_id, root_node_id, schema_version, graph_json, graph_sha256, validation_status, created_at, updated_at) VALUES (?, ?, ?, 'node_legacy_shell', 'ModuleGraph@1', ?, ?, 'valid', '2026-07-17T00:00:04Z', '2026-07-17T00:00:04Z')",
                params![graph_id, project_id, version_id, graph.to_string(), graph_sha256],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE projects SET current_version_id=? WHERE project_id=?",
                params![version_id, project_id],
            )
            .unwrap();
        LegacyFixture {
            project_id,
            version_id,
            graph_id,
            module_id,
            spec,
            spec_sha256,
            graph,
            glb,
            glb_sha256,
        }
    }

    fn minimal_glb() -> Vec<u8> {
        let mut document = serde_json::to_vec(&json!({"asset": {"version": "2.0"}})).unwrap();
        while document.len() % 4 != 0 {
            document.push(b' ');
        }
        let total_length = 12 + 8 + document.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(document.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&document);
        glb
    }

    fn hex_sha256(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}
