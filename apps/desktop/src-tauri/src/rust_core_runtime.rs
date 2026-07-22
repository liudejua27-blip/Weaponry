//! Desktop wiring for the Rust-owned K003 product-state core.
//!
//! This module deliberately contains only adapters. Durable lifecycle and
//! product-state decisions stay in `forgecad-core`; the app-server sees a
//! sealed persistence port and the temporary HTTP bridge sees explicit,
//! code-owned routes.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use forgecad_app_server::{
    compatibility::{AllowedHttpMethod, PreparedCompatHttpRequest},
    ActiveDesignSnapshotReader, CancellationToken, LifecyclePersistencePort, LifecyclePortError,
    LifecyclePortErrorKind, LifecyclePortFuture, ProductToolPortError,
};
use forgecad_app_server_protocol::{
    CompatHttpResponse, LifecyclePersistenceCommand, LifecyclePersistenceResult, ProtocolHttpBody,
    RpcError, HTTP_COMPAT_RESPONSE_SCHEMA_VERSION,
};
use forgecad_core::{
    builtin_surface_adornment_manifest, builtin_surface_adornment_manifest_v2,
    inspect_external_glb, is_external_glb_reference, reference_rebuild_plan_id_for_change_set,
    resolve_semantic_proportions, semantic_sha256, validate_reference_surface_analysis_for_plan,
    verify_forgecad_glb, ActiveDesignSnapshot, AgentAssetChangeSet, AgentAssetVersion,
    AgentSkillActivation, AgentSkillEvalReport, ChangeSetStatus, ComponentRecipeRef, CoreError,
    CoreRepository, CoreResult, CreateReferenceEvidenceRequest, ImportExternalGlbRequest,
    ImportedGlbInspection, LifecycleStore, MaterialTextureQuery, MaterialTextureRole,
    MaterialTextureSource, MaterialTextureSummary, NavigationAction, ObjectRecord, ObjectReference,
    Project, ProjectStatus, QualityReport, QualityStatus, RecipeInstantiationRequest,
    RecipeMaterialZoneOverride, RecipeParameterValue, RecipeRegistry, RecipeSlotBinding,
    RecipeValidator, ReferenceEvidenceKind, ReferenceGuidedRebuildPlan,
    ReferenceGuidedRebuildPlanStatus, ReferenceSurfaceAnalysis, ReferenceSurfaceBinding,
    ReferenceSurfaceFidelityCeiling, ReferenceSurfaceGlbReadbackFacts,
    ReferenceSurfaceIntentionalChange, ReferenceSurfaceObservationKind, ReferenceSurfaceUnresolved,
    RegisterMaterialTextureRequest, SkillEvalStatus, SnapshotEtag, SurfaceAdornmentProgram,
    EXTERNAL_GLB_ARTIFACT_PROFILE_ID, EXTERNAL_GLB_REFERENCE_ROLE,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

/// Read-only request for a reviewed C105 Recipe replacement candidate.  The
/// active Project, base Version and Snapshot revision are deliberately derived
/// by Rust rather than being accepted from the client.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveRecipeCandidateRequest {
    schema_version: String,
    recipe_request_id: String,
    component_recipe_ref: ComponentRecipeRef,
    #[serde(default)]
    slot_bindings: Vec<RecipeSlotBinding>,
    #[serde(default)]
    parameter_values: Vec<RecipeParameterValue>,
    #[serde(default)]
    material_zone_overrides: Vec<RecipeMaterialZoneOverride>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnableSurfaceAdornmentSkillRequest {
    schema_version: String,
    client_request_id: String,
    confirm_enable: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfaceAdornmentPreviewRequest {
    schema_version: String,
    client_request_id: String,
    part_id: String,
    material_zone_id: String,
    kind: String,
    motif: String,
    intensity: String,
    coverage: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceGuidedRebuildPreviewRequest {
    schema_version: String,
    client_request_id: String,
    evidence_id: String,
    domain_pack_id: String,
    #[serde(default)]
    base_asset_version_id: Option<String>,
}

/// One desktop-process owner for `library.db`, the CAS and lifecycle state.
///
/// `open` intentionally does not publish the writer cutover. The caller must
/// finish constructing every remaining desktop handler and then call
/// [`publish`](Self::publish). Dropping/rolling back before that point restores
/// the pre-K003 ownership marker.
#[derive(Debug, Clone)]
pub struct RustCoreRuntime {
    library_root: PathBuf,
    repository: CoreRepository,
    lifecycle: LifecycleStore,
}

/// Adapter used by the native Agent runtime to build a read-only context from
/// the same CoreRepository that owns ActiveDesignSnapshot, CAS and version
/// heads.  It carries no write capability and never exposes the database path
/// or object bytes to the Provider.
#[derive(Clone)]
pub struct RustCoreActiveDesignSnapshotReader {
    core: Arc<RustCoreRuntime>,
}

impl RustCoreActiveDesignSnapshotReader {
    pub fn new(core: Arc<RustCoreRuntime>) -> Self {
        Self { core }
    }
}

impl ActiveDesignSnapshotReader for RustCoreActiveDesignSnapshotReader {
    fn read_active_design_snapshot(
        &self,
        project_id: &str,
    ) -> Result<Option<Value>, ProductToolPortError> {
        let snapshot = self
            .core
            .repository()
            .snapshot(project_id)
            .map_err(|error| {
                ProductToolPortError::invalid_response(format!(
                    "Rust ActiveDesignSnapshot read failed ({}): {}",
                    error.code(),
                    error
                ))
            })?;
        snapshot
            .map(|snapshot| {
                serde_json::to_value(snapshot).map_err(|error| {
                    ProductToolPortError::invalid_response(format!(
                        "Rust ActiveDesignSnapshot serialization failed: {error}"
                    ))
                })
            })
            .transpose()
    }
}

impl RustCoreRuntime {
    pub fn open(
        library_root: impl AsRef<Path>,
        instance_id: impl Into<String>,
    ) -> CoreResult<Self> {
        let library_root = library_root.as_ref().to_path_buf();
        std::fs::create_dir_all(&library_root)?;
        let repository =
            CoreRepository::open(library_root.join("library.db"), &library_root, instance_id)?;
        if let Err(error) = repository.ensure_default_domain_profile(&now_timestamp()) {
            let _ = repository.rollback_cutover_before_publish();
            return Err(error);
        }
        let lifecycle = LifecycleStore::new(repository.clone());
        Ok(Self {
            library_root,
            repository,
            lifecycle,
        })
    }

    pub fn library_root(&self) -> &Path {
        &self.library_root
    }

    pub fn repository(&self) -> &CoreRepository {
        &self.repository
    }

    pub fn lifecycle_port(&self) -> CoreLifecyclePort {
        CoreLifecyclePort {
            store: self.lifecycle.clone(),
        }
    }

    pub fn publish(&self) -> CoreResult<()> {
        self.repository.publish()
    }

    pub fn rollback_cutover_before_publish(&self) -> CoreResult<bool> {
        self.repository.rollback_cutover_before_publish()
    }

    pub fn recover_orphaned_turns(&self, updated_at: &str) -> CoreResult<Vec<String>> {
        self.lifecycle.recover_orphaned_turns(updated_at)
    }

    /// Handles only code-owned product routes. The production bridge returns
    /// a stable Rust 410 for every unhandled product route; Python is not a
    /// product read or write fallback after K003.
    pub fn handle_compat_http(
        &self,
        request: &PreparedCompatHttpRequest,
    ) -> Option<Result<CompatHttpResponse, RpcError>> {
        let route = request.path.split('?').next().unwrap_or(&request.path);
        let segments = route.strip_prefix('/')?.split('/').collect::<Vec<_>>();
        let result = match (request.method, segments.as_slice()) {
            (AllowedHttpMethod::Get, ["api", "v1", "projects"]) => self.list_projects_response(),
            (AllowedHttpMethod::Post, ["api", "v1", "projects"]) => {
                self.create_project_response(request)
            }
            (AllowedHttpMethod::Get, ["api", "v1", "projects", project_id]) => {
                self.project_response(project_id)
            }
            (AllowedHttpMethod::Get, ["api", "v1", "versions", version_id]) => {
                self.legacy_version_response(version_id)
            }
            (AllowedHttpMethod::Get, ["api", "v1", "module-graphs", graph_id]) => {
                self.legacy_module_graph_response(graph_id)
            }
            (AllowedHttpMethod::Get, ["api", "v1", "module-assets"]) => {
                self.legacy_module_catalog_response(request)
            }
            (AllowedHttpMethod::Get, ["api", "v1", "module-assets", module_id, "file"]) => {
                self.legacy_module_glb_response(module_id)
            }
            (AllowedHttpMethod::Get, ["api", "v1", "projects", project_id, "active-design"]) => {
                self.active_design_response(project_id)
            }
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "projects", project_id, "active-design:convert-legacy"],
            ) => self.convert_legacy_response(project_id, request),
            (
                AllowedHttpMethod::Get,
                ["api", "v1", "projects", project_id, "active-design:navigation"],
            ) => self.navigation_response(project_id),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "projects", project_id, "active-design:select"],
            ) => self.select_response(project_id, request),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "projects", project_id, "active-design:undo"],
            ) => self.navigate_response(project_id, NavigationAction::Undo, request),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "projects", project_id, "active-design:redo"],
            ) => self.navigate_response(project_id, NavigationAction::Redo, request),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "projects", project_id, "active-design:render-preset"],
            ) => self.render_preset_response(project_id, request),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "projects", project_id, "active-design:part-display"],
            ) => self.part_display_response(project_id, request),
            (
                AllowedHttpMethod::Get,
                ["api", "v1", "agent", "quality-reports", quality_report_id],
            ) => self.quality_response(quality_report_id),
            (AllowedHttpMethod::Get, ["api", "v1", "agent", "materials"]) => {
                self.material_catalog_response()
            }
            (AllowedHttpMethod::Get, ["api", "v1", "agent", "material-textures"]) => {
                self.list_material_textures_response(request)
            }
            (
                AllowedHttpMethod::Get,
                ["api", "v1", "agent", "material-textures", texture_asset_id],
            ) => self.material_texture_response(texture_asset_id),
            (AllowedHttpMethod::Post, ["api", "v1", "agent", "material-textures"]) => {
                self.register_material_texture_response(request)
            }
            (AllowedHttpMethod::Post, ["api", "v1", "agent", "imports:glb"]) => {
                self.import_external_glb_response(request)
            }
            (AllowedHttpMethod::Post, ["api", "v1", "agent", "reference-evidence:create"]) => {
                self.create_reference_evidence_response(request)
            }
            (
                AllowedHttpMethod::Get,
                ["api", "v1", "agent", "projects", project_id, "reference-evidence"],
            ) => self.reference_evidence_list_response(project_id),
            (
                AllowedHttpMethod::Get,
                ["api", "v1", "agent", "projects", project_id, "reference-evidence", evidence_route],
            ) => evidence_route
                .strip_suffix(":content")
                .ok_or_else(|| CoreError::not_found("Agent route"))
                .and_then(|evidence_id| {
                    self.reference_evidence_content_response(project_id, evidence_id)
                }),
            (
                AllowedHttpMethod::Get,
                ["api", "v1", "agent", "projects", project_id, "reference-guided-rebuild-plans", rebuild_plan_id],
            ) => self.reference_guided_rebuild_plan_response(project_id, rebuild_plan_id),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "agent", "projects", project_id, "reference-guided-rebuild:preview"],
            ) => self.reference_guided_rebuild_preview_response(project_id, request),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "agent", "skills", "surface-adornment:enable"],
            ) => self.enable_surface_adornment_skill_response(request),
            (AllowedHttpMethod::Get, ["api", "v1", "agent", "components"]) => {
                self.list_components_response(request)
            }
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "agent", "asset-versions", asset_version_id, "components"],
            ) => self.save_component_response(asset_version_id, request),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "agent", "asset-versions", asset_version_id, "surface-adornments:preview"],
            ) => self.surface_adornment_preview_response(asset_version_id, request),
            (
                AllowedHttpMethod::Get,
                ["api", "v1", "agent", "asset-versions", asset_version_id, "components:compatible"],
            ) => self.compatible_components_response(asset_version_id, request),
            (
                AllowedHttpMethod::Get,
                ["api", "v1", "agent", "asset-versions", asset_version_id, "structure-suggestions"],
            ) => self.structure_suggestions_response(asset_version_id),
            (
                AllowedHttpMethod::Get,
                ["api", "v1", "agent", "asset-versions", asset_version_id, "parts", part_id, "semantic-proportions"],
            ) => self.semantic_proportions_response(asset_version_id, part_id),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "agent", "asset-versions", asset_version_id, "parts", part_id, "component-recipes:expand"],
            ) => self.active_recipe_candidate_response(asset_version_id, part_id, request),
            (
                AllowedHttpMethod::Post,
                ["api", "v1", "agent", "asset-versions", asset_version_id, "change-sets"],
            ) => self.propose_change_set_response(asset_version_id, request),
            (method, ["api", "v1", "agent", "change-sets", change_set_route]) => {
                if method == AllowedHttpMethod::Post {
                    if let Some(change_set_id) = change_set_route.strip_suffix(":preview") {
                        self.geometry_change_set_required_response(
                            change_set_id,
                            "preview",
                            request,
                        )
                    } else if let Some(change_set_id) = change_set_route.strip_suffix(":confirm") {
                        self.geometry_change_set_required_response(
                            change_set_id,
                            "confirm",
                            request,
                        )
                    } else if let Some(change_set_id) = change_set_route.strip_suffix(":reject") {
                        self.reject_change_set_response(change_set_id, request)
                    } else {
                        return None;
                    }
                } else if method == AllowedHttpMethod::Get {
                    if let Some(change_set_id) = change_set_route.strip_suffix(":preview.glb") {
                        self.geometry_change_set_required_response(
                            change_set_id,
                            "preview_glb",
                            request,
                        )
                    } else if !change_set_route.contains(':') {
                        self.change_set_response(change_set_route)
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            (method, ["api", "v1", "agent", "asset-versions", asset_route]) => {
                if method == AllowedHttpMethod::Get {
                    if let Some(asset_version_id) = asset_route.strip_suffix(":model.glb") {
                        self.production_glb_response(asset_version_id)
                    } else if let Some(asset_version_id) = asset_route.strip_suffix(":preview.glb")
                    {
                        self.preview_glb_response(asset_version_id)
                    } else if !asset_route.contains(':') {
                        self.asset_version_response(asset_route)
                    } else {
                        return None;
                    }
                } else if method == AllowedHttpMethod::Post {
                    if let Some(asset_version_id) = asset_route.strip_suffix(":export") {
                        self.export_response(asset_version_id)
                    } else if let Some(asset_version_id) = asset_route.strip_suffix(":quality") {
                        self.create_quality_response(asset_version_id, request)
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            (AllowedHttpMethod::Get, retired) if is_retired_legacy_get(retired) => {
                retired_legacy_get_response(route)
            }
            _ => return None,
        };
        Some(Ok(match result {
            Ok(response) => response,
            Err(error) => core_error_response(error),
        }))
    }

    fn material_catalog_response(&self) -> CoreResult<CompatHttpResponse> {
        let mut presets = crate::rust_product_catalog::material_presets();
        self.enrich_material_catalog(&mut presets)?;
        json_response(200, presets, Vec::new())
    }

    fn create_reference_evidence_response(
        &self,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?.to_string();
        let input: CreateReferenceEvidenceRequest = serde_json::from_value(body).map_err(|_| {
            CoreError::invalid_data(
                "REFERENCE_EVIDENCE_CREATE_REQUEST_INVALID",
                "Reference evidence create request is outside the R007 contract.",
            )
        })?;
        if input.client_request_id != client_request_id {
            return Err(CoreError::invalid_data(
                "IDEMPOTENCY_IDENTITY_MISMATCH",
                "Idempotency-Key must exactly equal client_request_id.",
            ));
        }
        let evidence = self
            .repository
            .create_reference_evidence(&input, &now_timestamp())?;
        json_response(
            201,
            json!({
                "schema_version": "ReferenceEvidenceCreateResponse@1",
                "reference_evidence": evidence,
            }),
            Vec::new(),
        )
    }

    /// Project-scoped R007 restore index. It returns sealed metadata and plan
    /// identities only; raw source bytes are available exclusively through the
    /// evidence-ID content route below.
    fn reference_evidence_list_response(&self, project_id: &str) -> CoreResult<CompatHttpResponse> {
        let reference_evidence = self.repository.reference_evidence_for_project(project_id)?;
        let reference_guided_rebuild_plans = self
            .repository
            .reference_guided_rebuild_plans_for_project(project_id)?;
        json_response(
            200,
            json!({
                "schema_version": "ReferenceEvidenceProjectRead@1",
                "reference_evidence": reference_evidence,
                "reference_guided_rebuild_plans": reference_guided_rebuild_plans,
            }),
            vec![("Cache-Control".into(), "no-store".into())],
        )
    }

    /// Returns a single sealed evidence object after Core has checked both the
    /// Project and its `reference_evidence_source` relation. This route does
    /// not accept an object hash, a CAS path, or a local filesystem path.
    fn reference_evidence_content_response(
        &self,
        project_id: &str,
        evidence_id: &str,
    ) -> CoreResult<CompatHttpResponse> {
        let (evidence, bytes) = self
            .repository
            .read_reference_evidence_content(project_id, evidence_id)?;
        let media_type = match evidence.source_media_type.as_str() {
            "image/png" | "image/jpeg" | "image/webp" | "model/gltf-binary" => {
                evidence.source_media_type.as_str()
            }
            _ => {
                return Err(CoreError::conflict(
                    "REFERENCE_EVIDENCE_MEDIA_TYPE_INVALID",
                    "Reference evidence has an unsupported sealed media type.",
                ));
            }
        };
        let extension = match media_type {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            "model/gltf-binary" => "glb",
            _ => unreachable!("validated media type"),
        };
        Ok(CompatHttpResponse {
            schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
            status: 200,
            headers: vec![
                ("Cache-Control".into(), "no-store".into()),
                ("Content-Type".into(), media_type.into()),
                (
                    "Content-Disposition".into(),
                    format!("inline; filename=\"reference-{evidence_id}.{extension}\""),
                ),
                (
                    "X-ForgeCAD-Reference-Evidence-ID".into(),
                    evidence.evidence_id,
                ),
            ],
            body: ProtocolHttpBody::Base64 {
                data: BASE64_STANDARD.encode(bytes),
            },
        })
    }

    /// Reads the exact persisted plan/analysis/pair. The response deliberately
    /// projects hashes and immutable version IDs rather than serializing an
    /// ObjectRecord, which would leak CAS storage paths.
    fn reference_guided_rebuild_plan_response(
        &self,
        project_id: &str,
        rebuild_plan_id: &str,
    ) -> CoreResult<CompatHttpResponse> {
        let pair = self
            .repository
            .reference_guided_rebuild_frozen_pair(rebuild_plan_id)?
            .ok_or_else(|| CoreError::not_found("Reference guided rebuild plan"))?;
        if pair.plan.project_id != project_id || pair.evidence.project_id != project_id {
            return Err(CoreError::not_found("Reference guided rebuild plan"));
        }
        let reference_result_pair = json!({
            "source_object_sha256": pair.evidence.source_object_sha256,
            "result_asset_version_id": pair.plan.confirmed_asset_version_id,
            "result_glb_sha256": pair.confirmed_production_glb.map(|object| object.sha256),
        });
        json_response(
            200,
            json!({
                "schema_version": "ReferenceGuidedRebuildPlanRead@1",
                "reference_guided_rebuild_plan": pair.plan,
                "reference_surface_analysis": pair.analysis,
                "reference_result_pair": reference_result_pair,
            }),
            vec![("Cache-Control".into(), "no-store".into())],
        )
    }

    fn reference_guided_rebuild_preview_response(
        &self,
        project_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?.to_string();
        let input: ReferenceGuidedRebuildPreviewRequest = serde_json::from_value(body.clone())
            .map_err(|_| {
                CoreError::invalid_data(
                    "REFERENCE_REBUILD_PREVIEW_REQUEST_INVALID",
                    "Reference-guided rebuild preview request is outside the R007 contract.",
                )
            })?;
        if input.schema_version != "ReferenceGuidedRebuildPreviewRequest@1"
            || input.client_request_id != client_request_id
        {
            return Err(CoreError::invalid_data(
                "REFERENCE_REBUILD_PREVIEW_REQUEST_INVALID",
                "Reference-guided rebuild preview requires its exact v1 schema and idempotency identity.",
            ));
        }
        let base_asset_version_id = input.base_asset_version_id.as_deref().ok_or_else(|| {
            CoreError::conflict(
                "REFERENCE_REBUILD_BASE_REQUIRED",
                "Reference-guided ChangeSet preview currently requires an active editable base; initial synthesis is owned by V003.",
            )
        })?;
        let evidence = self
            .repository
            .reference_evidence(&input.evidence_id)?
            .ok_or_else(|| CoreError::not_found("ReferenceEvidence"))?;
        if evidence.project_id != project_id || evidence.domain_pack_id != input.domain_pack_id {
            return Err(CoreError::conflict(
                "REFERENCE_REBUILD_PROJECT_MISMATCH",
                "Reference evidence, Project and Domain Pack must remain identical.",
            ));
        }
        let base = self
            .repository
            .version(base_asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        if base.project_id != project_id
            || base.domain_pack_id != input.domain_pack_id
            || is_external_glb_reference(&base)
        {
            return Err(CoreError::conflict(
                "REFERENCE_REBUILD_BASE_INVALID",
                "Reference rebuild base must be the same Project's editable ForgeCAD asset.",
            ));
        }
        let snapshot = self
            .repository
            .snapshot(project_id)?
            .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
        if snapshot.active_design.asset_version_id() != Some(base_asset_version_id)
            || self.repository.head(project_id)?.as_deref() != Some(base_asset_version_id)
            || snapshot.preview.is_some()
        {
            return Err(CoreError::conflict(
                "REFERENCE_REBUILD_BASE_STALE",
                "Reference rebuild requires the current active head with no pending preview.",
            ));
        }

        // R007B has one frozen, production-arm lineage.  The domain label is
        // never sufficient: an active AssemblyGraph must itself pin one exact
        // reviewed C106 root before this endpoint may create *any* plan or
        // ChangeSet.  Otherwise the old C105 fallback could return 201 without
        // `reference_surface_analysis`, which makes the client unable to prove
        // the result's exact reference lineage.
        let c106_registry = RecipeRegistry::from_embedded_c106_robotic_arm()?;
        let active_c106_root = if input.domain_pack_id == "pack_robotic_arm_concept" {
            active_c106_root_recipe_id(&base, &c106_registry)?
        } else {
            None
        }
        .ok_or_else(|| {
            CoreError::conflict(
                "REFERENCE_REBUILD_C106_BASE_REQUIRED",
                "Reference-guided rebuild preview requires the active project's exact reviewed C106 robotic-arm base so it can seal the required surface-analysis lineage.",
            )
        })?;
        let registry = c106_registry.clone();
        let root_part_id = base
            .assembly_graph
            .get("root_part_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let parts = base
            .assembly_graph
            .get("parts")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "REFERENCE_REBUILD_ASSEMBLY_INVALID",
                    "Reference rebuild base has no readable AssemblyGraph parts.",
                )
            })?;
        let change_set_id = stable_request_id(
            "changeset",
            &format!("{base_asset_version_id}:{client_request_id}"),
        )?;
        let rebuild_plan_id = reference_rebuild_plan_id_for_change_set(&change_set_id)?;
        // R007B must make the sealed evidence affect the actual preview, not
        // merely attach a ReferenceSurfaceAnalysis to a no-op replacement.
        // The mapping below is deliberately small and fixed: it only invokes
        // existing ChangeSet operations against the current C106 graph.  It
        // does not select a second arm root, copy source geometry, add an
        // operation type, or infer hidden/mechanical facts.
        let effect = r007b_reference_surface_effect(
            &self.repository,
            &evidence,
            &base,
            parts,
            root_part_id,
            &rebuild_plan_id,
        )?;
        let timestamp = now_timestamp();
        let plan = ReferenceGuidedRebuildPlan {
            schema_version: "ReferenceGuidedRebuildPlan@1".into(),
            rebuild_plan_id: rebuild_plan_id.clone(),
            project_id: project_id.to_string(),
            evidence_id: evidence.evidence_id.clone(),
            base_asset_version_id: Some(base_asset_version_id.to_string()),
            domain_pack_id: input.domain_pack_id.clone(),
            // This is a replacement of the active C106 asset, not a new
            // evidence-class-selected arm.  Pin the active root so the
            // immutable plan/analysis, slot bindings and eventual ChangeSet
            // retain one exact C106 lineage across all reviewed arm roots.
            recipe_id: active_c106_root,
            recipe_registry_sha256: registry.registry_sha256().to_string(),
            rebuild_summary:
                "依据授权参考的可见轮廓、比例与材质区证据，应用受限的可编辑设计表面调整。".into(),
            intended_differences: vec![
                effect.intended_difference.to_string(),
                "只保留可见设计语言；隐藏结构、精确尺寸、材料和功能保持未知。".into(),
            ],
            retained_evidence: vec![
                evidence.observations.silhouette_summary.clone(),
                evidence.observations.proportion_ranges.join("；"),
            ],
            unresolved_uncertainties: evidence.observations.uncertainties.clone(),
            status: ReferenceGuidedRebuildPlanStatus::Draft,
            preview_change_set_id: None,
            confirmed_asset_version_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        // An idempotent replay must read the analysis sealed with the original
        // plan, not rebuild it against a potentially newer registry/Skill.
        let reference_surface_analysis = match self
            .repository
            .reference_guided_rebuild_plan(&rebuild_plan_id)?
        {
            Some(existing)
                if existing.project_id != plan.project_id
                    || existing.evidence_id != plan.evidence_id
                    || existing.base_asset_version_id != plan.base_asset_version_id
                    || existing.domain_pack_id != plan.domain_pack_id
                    || existing.recipe_id != plan.recipe_id
                    || existing.recipe_registry_sha256 != plan.recipe_registry_sha256 =>
            {
                return Err(CoreError::conflict(
                    "IDEMPOTENCY_CONFLICT",
                    "Reference rebuild identity was already used for different evidence or base state.",
                ));
            }
            Some(_) => self
                .repository
                .reference_surface_analysis(&rebuild_plan_id)?
                .ok_or_else(|| {
                    CoreError::conflict(
                        "REFERENCE_REBUILD_FROZEN_ANALYSIS_REQUIRED",
                        "A persisted R007B rebuild plan is missing its required frozen surface analysis and cannot be replayed.",
                    )
                })?,
            None => {
                let analysis = build_reference_surface_analysis(
                    &c106_registry,
                    &evidence,
                    &plan,
                    &timestamp,
                )?;
                validate_reference_surface_analysis_for_plan(&analysis, &evidence, &plan)?;
                self.repository
                    .create_reference_guided_rebuild_plan_with_surface_analysis(
                        &plan,
                        Some(&analysis),
                    )?;
                analysis
            }
        };
        let change_set = AgentAssetChangeSet {
            change_set_id: change_set_id.clone(),
            project_id: project_id.to_string(),
            base_asset_version_id: base_asset_version_id.to_string(),
            summary: effect.summary.to_string(),
            operations: effect.operations,
            protected_part_ids: Vec::new(),
            preview: None,
            status: ChangeSetStatus::Proposed,
            resulting_asset_version_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        let scope =
            format!("POST /api/v1/agent/projects/{project_id}/reference-guided-rebuild:preview");
        let request_hash = semantic_sha256(&json!({
            "project_id": project_id,
            "request": body,
            "reference_surface_effect": change_set.operations,
        }))?;
        let stored = self.repository.create_change_set_idempotent(
            &change_set,
            &scope,
            &client_request_id,
            &request_hash,
        )?;
        let mut payload = change_set_payload(&stored)?;
        if let Some(object) = payload.as_object_mut() {
            object.insert(
                "reference_guided_rebuild_plan".into(),
                serde_json::to_value(&plan).map_err(json_encode_error)?,
            );
            object.insert(
                "reference_surface_analysis".into(),
                serde_json::to_value(reference_surface_analysis).map_err(json_encode_error)?,
            );
        } else {
            return Err(CoreError::invalid_data(
                "REFERENCE_REBUILD_CHANGESET_PAYLOAD_INVALID",
                "Reference-guided rebuild ChangeSet did not serialize as an object.",
            ));
        }
        json_response(201, payload, Vec::new())
    }

    fn enable_surface_adornment_skill_response(
        &self,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?.to_string();
        let input: EnableSurfaceAdornmentSkillRequest =
            serde_json::from_value(body).map_err(|_| {
                CoreError::invalid_data(
                    "SURFACE_ADORNMENT_ENABLE_REQUEST_INVALID",
                    "Surface appearance enable request is outside the reviewed contract.",
                )
            })?;
        if input.schema_version != "EnableSurfaceAdornmentSkillRequest@1"
            || !input.confirm_enable
            || input.client_request_id != client_request_id
        {
            return Err(CoreError::invalid_data(
                "SURFACE_ADORNMENT_ENABLE_CONFIRMATION_REQUIRED",
                "The first-party surface appearance Skill requires explicit confirmation.",
            ));
        }
        let timestamp = now_timestamp();
        let manifest = self
            .repository
            .ensure_builtin_surface_adornment_skill(&timestamp)?;
        let dry_run = self.repository.dry_run_skill(&manifest)?;
        if dry_run.product_state_write_performed {
            return Err(CoreError::conflict(
                "SKILL_DRY_RUN_SIDE_EFFECT",
                "Skill dry-run unexpectedly reported a product-state write.",
            ));
        }
        let manifest_sha256 = manifest.canonical_sha256()?;
        if let Some(activation) = self.repository.active_skill(&manifest.skill_id)? {
            if activation.skill_version != manifest.version
                || activation.skill_sha256 != manifest_sha256
            {
                // An older explicit activation is not silently granted C106
                // rights. The same explicit enable request performs the
                // immutable v2 activation below.
            } else {
                return json_response(
                    200,
                    json!({
                        "schema_version": "SurfaceAdornmentSkillStatus@1",
                        "status": "enabled",
                        "activation": activation,
                        "client_request_id": client_request_id,
                    }),
                    Vec::new(),
                );
            }
        }
        let skill_sha256 = manifest_sha256;
        let reports = self
            .repository
            .skill_eval_reports(&manifest.skill_id, manifest.version)?;
        if !reports.iter().any(|report| {
            report.status == SkillEvalStatus::Passed && report.skill_sha256 == skill_sha256
        }) {
            self.repository.record_skill_eval(&AgentSkillEvalReport {
                schema_version: "AgentSkillEvalReport@1".into(),
                report_id: "skilleval_first_party_surface_adornment_v2".into(),
                skill_id: manifest.skill_id.clone(),
                skill_version: manifest.version,
                skill_sha256: skill_sha256.clone(),
                status: SkillEvalStatus::Passed,
                findings: vec![
                    "code_owned_namespaces_passed".into(),
                    "visual_only_texture_bake_passed".into(),
                    "no_dynamic_execution_or_location_passed".into(),
                ],
                evaluated_at: timestamp.clone(),
            })?;
        }
        let activation = AgentSkillActivation {
            schema_version: "AgentSkillActivation@1".into(),
            activation_id: "skillact_first_party_surface_adornment_v2".into(),
            skill_id: manifest.skill_id,
            skill_version: manifest.version,
            skill_sha256,
            enabled: true,
            updated_at: timestamp,
        };
        let activation = self.repository.set_skill_activation(&activation)?;
        json_response(
            200,
            json!({
                "schema_version": "SurfaceAdornmentSkillStatus@1",
                "status": "enabled",
                "activation": activation,
                "client_request_id": client_request_id,
            }),
            Vec::new(),
        )
    }

    fn surface_adornment_preview_response(
        &self,
        asset_version_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?.to_string();
        let input: SurfaceAdornmentPreviewRequest =
            serde_json::from_value(body.clone()).map_err(|_| {
                CoreError::invalid_data(
                    "SURFACE_ADORNMENT_PREVIEW_REQUEST_INVALID",
                    "Surface appearance preview request is outside the reviewed contract.",
                )
            })?;
        if input.schema_version != "SurfaceAdornmentPreviewRequest@1"
            || input.client_request_id != client_request_id
        {
            return Err(CoreError::invalid_data(
                "SURFACE_ADORNMENT_PREVIEW_REQUEST_INVALID",
                "Surface appearance preview request identity is invalid.",
            ));
        }
        let version = self
            .repository
            .version(asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        let manifest = builtin_surface_adornment_manifest();
        let activation = self
            .repository
            .active_skill(&manifest.skill_id)?
            .ok_or_else(|| {
                CoreError::conflict(
                    "SURFACE_ADORNMENT_SKILL_DISABLED",
                    "请先明确启用内置外观细节能力，再生成预览。",
                )
            })?;
        let base_material = canonical_surface_adornment_material(&surface_adornment_base_material(
            &version,
            &input.part_id,
            &input.material_zone_id,
        )?);
        let identity_hash = semantic_sha256(&json!({
            "asset_version_id": asset_version_id,
            "client_request_id": client_request_id,
            "part_id": input.part_id,
            "material_zone_id": input.material_zone_id,
            "kind": input.kind,
            "motif": input.motif,
            "intensity": input.intensity,
            "coverage": input.coverage,
        }))?;
        let program = SurfaceAdornmentProgram {
            schema_version: "SurfaceAdornmentProgram@1".into(),
            program_id: format!("adorn_{}", &identity_hash[..24]),
            target_part_id: input.part_id.clone(),
            target_zone_id: input.material_zone_id.clone(),
            kind: input.kind,
            motif: input.motif,
            intensity: input.intensity,
            coverage: input.coverage,
            seed: u32::from_str_radix(&identity_hash[..8], 16).unwrap_or(0) & 0x7fff_ffff,
            base_material,
            execution: "texture_bake".into(),
            skill_id: activation.skill_id,
            skill_version: activation.skill_version,
            skill_sha256: activation.skill_sha256,
            generator: "a005_v1".into(),
            non_functional_only: true,
        };
        self.repository
            .validate_surface_adornment_program(asset_version_id, &program)?;
        let change_set_id = stable_request_id(
            "changeset",
            &format!("{asset_version_id}:surface-adornment:{client_request_id}"),
        )?;
        let created_at = now_timestamp();
        let change_set = AgentAssetChangeSet {
            change_set_id,
            project_id: version.project_id,
            base_asset_version_id: asset_version_id.to_string(),
            summary: "预览选中材质区的外观细节".into(),
            operations: vec![json!({
                "operation_id": format!("changeop_{}", &identity_hash[..24]),
                "op": "apply_surface_adornment",
                "part_id": input.part_id,
                "material_zone_id": input.material_zone_id,
                "surface_adornment_program": program,
            })],
            protected_part_ids: Vec::new(),
            preview: None,
            status: ChangeSetStatus::Proposed,
            resulting_asset_version_id: None,
            created_at: created_at.clone(),
            updated_at: created_at,
        };
        let scope = format!(
            "POST /api/v1/agent/asset-versions/{asset_version_id}/surface-adornments:preview"
        );
        let request_hash = semantic_sha256(&json!({
            "asset_version_id": asset_version_id,
            "request": body,
        }))?;
        let stored = self.repository.create_change_set_idempotent(
            &change_set,
            &scope,
            &client_request_id,
            &request_hash,
        )?;
        json_response(201, change_set_payload(&stored)?, Vec::new())
    }

    fn enrich_material_catalog(&self, presets: &mut Value) -> CoreResult<()> {
        let items = presets.as_array_mut().ok_or_else(|| {
            CoreError::invalid_data(
                "MATERIAL_CATALOG_INVALID",
                "Code-owned material catalog must be a JSON array.",
            )
        })?;
        for preset in items {
            let texture_slots = [
                (
                    "base_color_texture_asset_id",
                    MaterialTextureRole::BaseColor,
                ),
                (
                    "metallic_roughness_texture_asset_id",
                    MaterialTextureRole::MetallicRoughness,
                ),
                ("normal_texture_asset_id", MaterialTextureRole::Normal),
                ("occlusion_texture_asset_id", MaterialTextureRole::Occlusion),
                ("emissive_texture_asset_id", MaterialTextureRole::Emissive),
            ];
            let mut references = Vec::new();
            if let Some(pbr) = preset.get("pbr").and_then(Value::as_object) {
                for (field, role) in texture_slots {
                    if let Some(texture_asset_id) = pbr.get(field).and_then(Value::as_str) {
                        references.push((texture_asset_id.to_string(), role));
                    }
                }
            }
            if let Some(texture_asset_id) = preset.get("thumbnail_asset_id").and_then(Value::as_str)
            {
                references.push((texture_asset_id.to_string(), MaterialTextureRole::Thumbnail));
            }

            let mut summaries = Vec::new();
            let mut thumbnail_fallback = "parameter";
            for (texture_asset_id, role) in references {
                let texture = self.repository.material_texture(&texture_asset_id)?;
                let summary = match texture {
                    Some(texture) => {
                        if role == MaterialTextureRole::Thumbnail {
                            thumbnail_fallback = if texture.object_exists {
                                "texture"
                            } else {
                                "unavailable"
                            };
                        }
                        MaterialTextureSummary {
                            texture_asset_id,
                            texture_role: role,
                            exists: texture.object_exists,
                            source: Some(texture.source),
                            license: Some(texture.license),
                            license_ref: texture.license_ref,
                        }
                    }
                    None => {
                        if role == MaterialTextureRole::Thumbnail {
                            thumbnail_fallback = "unavailable";
                        }
                        MaterialTextureSummary {
                            texture_asset_id,
                            texture_role: role,
                            exists: false,
                            source: None,
                            license: None,
                            license_ref: None,
                        }
                    }
                };
                summaries.push(serde_json::to_value(summary).map_err(json_encode_error)?);
            }
            let object = preset.as_object_mut().ok_or_else(|| {
                CoreError::invalid_data(
                    "MATERIAL_CATALOG_INVALID",
                    "Code-owned material preset must be a JSON object.",
                )
            })?;
            object.insert("texture_summary".into(), Value::Array(summaries));
            object.insert(
                "thumbnail_fallback".into(),
                Value::String(thumbnail_fallback.into()),
            );
        }
        Ok(())
    }

    fn list_material_textures_response(
        &self,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let parameters = query_parameters(&request.path)?;
        let texture_role = parameters
            .get("texture_role")
            .map(|value| MaterialTextureRole::from_str(value))
            .transpose()?;
        let source = parameters
            .get("source")
            .map(|value| MaterialTextureSource::from_str(value))
            .transpose()?;
        let query = parameters.get("q").cloned();
        let limit = parameters
            .get("limit")
            .map(|value| {
                value.parse::<usize>().map_err(|_| {
                    CoreError::invalid_data(
                        "TEXTURE_QUERY_LIMIT_INVALID",
                        "Texture query limit must be an integer between 1 and 100.",
                    )
                })
            })
            .transpose()?
            .unwrap_or(100);
        let items = self
            .repository
            .list_material_textures(&MaterialTextureQuery {
                texture_role,
                source,
                query,
                limit,
            })?;
        json_response(200, json!({"items": items}), Vec::new())
    }

    fn material_texture_response(&self, texture_asset_id: &str) -> CoreResult<CompatHttpResponse> {
        let texture = self
            .repository
            .material_texture(texture_asset_id)?
            .ok_or_else(|| CoreError::not_found("MaterialTextureObject"))?;
        json_response(
            200,
            serde_json::to_value(texture).map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    fn register_material_texture_response(
        &self,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let idempotency_key = header(request, "idempotency-key").ok_or_else(|| {
            CoreError::invalid_data(
                "IDEMPOTENCY_KEY_REQUIRED",
                "Idempotency-Key is required for texture registration.",
            )
        })?;
        let body = request_json(request)?;
        let registration: RegisterMaterialTextureRequest =
            serde_json::from_value(body).map_err(|_| {
                CoreError::invalid_data(
                    "TEXTURE_REQUEST_INVALID",
                    "Texture registration body does not match RegisterAgentMaterialTextureRequest.",
                )
            })?;
        let texture = self.repository.register_material_texture(
            &registration,
            idempotency_key,
            &utc_now_timestamp(),
        )?;
        json_response(
            201,
            serde_json::to_value(texture).map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    fn import_external_glb_response(
        &self,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let idempotency_key = required_header(request, "idempotency-key")?;
        let body = request_json(request)?;
        let import: ImportExternalGlbRequest = serde_json::from_value(body).map_err(|_| {
            CoreError::invalid_data(
                "GLB_IMPORT_REQUEST_INVALID",
                "GLB import body does not match ImportAgentGlbRequest.",
            )
        })?;
        let bundle =
            self.repository
                .import_external_glb(&import, idempotency_key, &utc_now_timestamp())?;
        json_response(
            201,
            serde_json::to_value(bundle.response).map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    fn list_projects_response(&self) -> CoreResult<CompatHttpResponse> {
        let projects = self.repository.list_projects(false, 200)?;
        json_response(
            200,
            json!({"items": projects, "next_cursor": Value::Null}),
            Vec::new(),
        )
    }

    fn create_project_response(
        &self,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = required_bounded_string(&body, "client_request_id", 120)?;
        if let Some(header_key) = header(request, "idempotency-key") {
            if header_key != client_request_id {
                return Err(CoreError::conflict(
                    "IDEMPOTENCY_CONFLICT",
                    "Idempotency-Key does not match client_request_id.",
                ));
            }
        }
        let name = required_bounded_string(&body, "name", 120)?;
        let profile_id = body
            .get("profile_id")
            .and_then(Value::as_str)
            .unwrap_or("profile_weapon_concept_v1");
        let profile = self
            .repository
            .domain_profile(profile_id)?
            .ok_or_else(|| CoreError::not_found("DesignDomainProfile"))?;
        let project_id = stable_request_id("prj", client_request_id)?;
        if let Some(existing) = self.repository.project(&project_id)? {
            if existing.name != name || existing.profile_id != profile_id {
                return Err(CoreError::conflict(
                    "IDEMPOTENCY_CONFLICT",
                    "client_request_id was already used for another Project request.",
                ));
            }
            return json_response(200, project_detail(&existing, profile), Vec::new());
        }
        let timestamp = now_timestamp();
        let project = self.repository.create_project(&Project {
            project_id,
            profile_id: profile_id.to_string(),
            domain_type: profile
                .get("domain_type")
                .and_then(Value::as_str)
                .unwrap_or("weapon_concept")
                .to_string(),
            name: name.to_string(),
            status: ProjectStatus::Active,
            current_version_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        })?;
        json_response(201, project_detail(&project, profile), Vec::new())
    }

    fn project_response(&self, project_id: &str) -> CoreResult<CompatHttpResponse> {
        let project = self
            .repository
            .project(project_id)?
            .ok_or_else(|| CoreError::not_found("Project"))?;
        if project.current_version_id.is_some() {
            let detail = self
                .repository
                .legacy_project_detail(project_id)?
                .ok_or_else(|| CoreError::not_found("legacy Concept Project detail"))?;
            return json_response(200, detail, Vec::new());
        }
        let profile = self
            .repository
            .domain_profile(&project.profile_id)?
            .ok_or_else(|| CoreError::not_found("DesignDomainProfile"))?;
        json_response(200, project_detail(&project, profile), Vec::new())
    }

    fn asset_version_response(&self, asset_version_id: &str) -> CoreResult<CompatHttpResponse> {
        let version = self
            .repository
            .version(asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        json_response(200, asset_version_payload(&version)?, Vec::new())
    }

    fn active_design_response(&self, project_id: &str) -> CoreResult<CompatHttpResponse> {
        let Some(snapshot) = self.repository.snapshot_or_legacy_read_only(project_id)? else {
            // Q002 freezes this exact zero-state distinction. A real project
            // without a Snapshot is not a missing generic resource; desktop
            // state machines and live acceptance must be able to distinguish
            // it from an unknown project without guessing from prose.
            return json_response(
                404,
                json!({
                    "error": {
                        "code": "ACTIVE_DESIGN_NOT_FOUND",
                        "message": "ActiveDesignSnapshot does not exist for this project.",
                        "recoverable": false,
                        "details": {}
                    }
                }),
                Vec::new(),
            );
        };
        snapshot_response(200, &snapshot)
    }

    fn legacy_version_response(&self, version_id: &str) -> CoreResult<CompatHttpResponse> {
        let version = self
            .repository
            .legacy_version_detail(version_id)?
            .ok_or_else(|| CoreError::not_found("legacy Concept Version"))?;
        json_response(200, version, Vec::new())
    }

    fn legacy_module_graph_response(&self, graph_id: &str) -> CoreResult<CompatHttpResponse> {
        let graph = self
            .repository
            .legacy_module_graph_detail(graph_id)?
            .ok_or_else(|| CoreError::not_found("legacy ModuleGraph"))?;
        json_response(200, graph, Vec::new())
    }

    fn legacy_module_catalog_response(
        &self,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let parameters = query_parameters(&request.path)?;
        if parameters.keys().any(|key| {
            !matches!(
                key.as_str(),
                "pack_id"
                    | "category"
                    | "query"
                    | "review_status"
                    | "tag"
                    | "catalog_path"
                    | "cursor"
                    | "limit"
            )
        }) {
            return Err(CoreError::invalid_data(
                "LEGACY_MODULE_CATALOG_QUERY_INVALID",
                "Historical Module catalog query contains an unknown parameter.",
            ));
        }
        let pack_id = parameters.get("pack_id").ok_or_else(|| {
            CoreError::invalid_data(
                "LEGACY_MODULE_PACK_REQUIRED",
                "Historical Module catalog reads require an explicit Domain Pack ID.",
            )
        })?;
        let limit = parameters
            .get("limit")
            .map(|value| {
                value.parse::<usize>().map_err(|_| {
                    CoreError::invalid_data(
                        "LEGACY_MODULE_CATALOG_QUERY_INVALID",
                        "Historical Module catalog limit must be an integer between 1 and 100.",
                    )
                })
            })
            .transpose()?
            .unwrap_or(100);
        let catalog = self.repository.legacy_module_catalog(
            pack_id,
            parameters.get("category").map(String::as_str),
            parameters.get("query").map(String::as_str),
            parameters.get("review_status").map(String::as_str),
            parameters.get("tag").map(String::as_str),
            parameters.get("catalog_path").map(String::as_str),
            parameters.get("cursor").map(String::as_str),
            limit,
        )?;
        json_response(
            200,
            catalog,
            vec![("Cache-Control".into(), "no-store".into())],
        )
    }

    fn legacy_module_glb_response(&self, module_id: &str) -> CoreResult<CompatHttpResponse> {
        let module = self
            .repository
            .legacy_module_glb(module_id)?
            .ok_or_else(|| CoreError::not_found("legacy Module GLB"))?;
        Ok(CompatHttpResponse {
            schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
            status: 200,
            headers: vec![
                ("Content-Type".into(), module.mime_type),
                ("Cache-Control".into(), "no-store".into()),
                (
                    "Content-Disposition".into(),
                    format!("inline; filename=\"{}\"", module.file_name),
                ),
                ("X-ForgeCAD-Object-SHA256".into(), module.sha256),
                ("Content-Length".into(), module.byte_size.to_string()),
            ],
            body: ProtocolHttpBody::Base64 {
                data: BASE64_STANDARD.encode(module.bytes),
            },
        })
    }

    fn convert_legacy_response(
        &self,
        project_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?;
        let expected = expected_etag(request, &body)?;
        if body.as_object().is_some_and(|object| {
            object
                .keys()
                .any(|key| !matches!(key.as_str(), "client_request_id" | "snapshot_revision"))
        }) {
            return Err(CoreError::invalid_data(
                "HTTP_FIELD_INVALID",
                "Legacy conversion request contains an unknown field.",
            ));
        }
        let scope = format!("POST /api/v1/projects/{project_id}/active-design:convert-legacy");
        let normalized_request = json!({
            "client_request_id": client_request_id,
            "snapshot_revision": body
                .get("snapshot_revision")
                .cloned()
                .unwrap_or(Value::Null),
        });
        let request_hash = semantic_sha256(&json!({
            "project_id": project_id,
            "expected_revision": expected.0,
            "request": normalized_request,
        }))?;
        let result = self.repository.authorize_legacy_conversion_idempotent(
            project_id,
            expected,
            &now_timestamp(),
            &scope,
            client_request_id,
            &request_hash,
        )?;
        let etag = SnapshotEtag(result.snapshot_revision).to_string();
        json_response(
            200,
            serde_json::to_value(result).map_err(json_encode_error)?,
            vec![("ETag".into(), etag)],
        )
    }

    fn navigation_response(&self, project_id: &str) -> CoreResult<CompatHttpResponse> {
        json_response(
            200,
            serde_json::to_value(self.repository.navigation_availability(project_id)?)
                .map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    fn select_response(
        &self,
        project_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?;
        let expected = expected_etag(request, &body)?;
        let part_id = optional_string(&body, "selected_part_id")?;
        let zone_id = optional_string(&body, "selected_material_zone_id")?;
        let scope = format!("POST /api/v1/projects/{project_id}/active-design:select");
        let request_hash = semantic_sha256(&serde_json::json!({
            "project_id": project_id,
            "expected_revision": expected.0,
            "selected_part_id": part_id,
            "selected_material_zone_id": zone_id,
        }))?;
        let snapshot = self.repository.select_idempotent(
            project_id,
            expected,
            part_id,
            zone_id,
            &now_timestamp(),
            &scope,
            client_request_id,
            &request_hash,
        )?;
        snapshot_response(200, &snapshot)
    }

    fn navigate_response(
        &self,
        project_id: &str,
        action: NavigationAction,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?;
        let expected = expected_etag(request, &body)?;
        let action_text = match action {
            NavigationAction::Undo => "undo",
            NavigationAction::Redo => "redo",
        };
        let resulting_asset_version_id = stable_request_id(
            "assetver_nav",
            &format!("{project_id}:{action_text}:{client_request_id}"),
        )?;
        if self
            .repository
            .version(&resulting_asset_version_id)?
            .is_some()
        {
            let snapshot = self
                .repository
                .snapshot(project_id)?
                .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
            if snapshot.active_design.asset_version_id()
                == Some(resulting_asset_version_id.as_str())
            {
                return snapshot_response(200, &snapshot);
            }
            return Err(CoreError::conflict(
                "IDEMPOTENCY_CONFLICT",
                "Navigation client_request_id already resolved to a different active state.",
            ));
        }
        let result = self.repository.navigate(
            project_id,
            action,
            &resulting_asset_version_id,
            expected,
            &now_timestamp(),
        )?;
        snapshot_response(200, &result.snapshot)
    }

    fn render_preset_response(
        &self,
        project_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?;
        let expected = expected_etag(request, &body)?;
        let camera_view = body
            .get("camera_view")
            .and_then(Value::as_str)
            .unwrap_or("iso");
        let light_preset = body
            .get("light_preset")
            .and_then(Value::as_str)
            .unwrap_or("cad_neutral");
        let scope = format!("POST /api/v1/projects/{project_id}/active-design:render-preset");
        let request_hash = semantic_sha256(&json!({
            "project_id": project_id,
            "snapshot_revision": expected.0,
            "request": body,
        }))?;
        let snapshot = self.repository.set_render_preset_idempotent(
            project_id,
            expected,
            camera_view,
            light_preset,
            &now_timestamp(),
            &scope,
            client_request_id,
            &request_hash,
        )?;
        snapshot_response(200, &snapshot)
    }

    fn part_display_response(
        &self,
        project_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?;
        let expected = expected_etag(request, &body)?;
        let action = required_bounded_string(&body, "action", 64)?;
        let part_id = optional_string(&body, "part_id")?;
        let scope = format!("POST /api/v1/projects/{project_id}/active-design:part-display");
        let request_hash = semantic_sha256(&json!({
            "project_id": project_id,
            "snapshot_revision": expected.0,
            "request": body,
        }))?;
        let snapshot = self.repository.set_part_display_idempotent(
            project_id,
            expected,
            action,
            part_id,
            &now_timestamp(),
            &scope,
            client_request_id,
            &request_hash,
        )?;
        snapshot_response(200, &snapshot)
    }

    fn create_quality_response(
        &self,
        asset_version_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let idempotency_key = required_header(request, "idempotency-key")?;
        let expected = expected_etag_header(request)?;
        let version = self
            .repository
            .version(asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        let snapshot = self
            .repository
            .snapshot(&version.project_id)?
            .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
        if snapshot.active_design.asset_version_id() != Some(asset_version_id) {
            return Err(CoreError::conflict(
                "ACTIVE_DESIGN_STALE",
                "Quality requires the current active Agent asset Snapshot.",
            ));
        }
        if is_external_glb_reference(&version) {
            let artifact = self.external_reference_artifact(&version, &snapshot)?;
            let scope = format!("POST /api/v1/agent/asset-versions/{asset_version_id}:quality");
            let request_hash = semantic_sha256(&json!({
                "asset_version_id": asset_version_id,
                "snapshot_revision": expected.0,
                "quality_contract": "ExternalGlbInspection@1",
                "glb_sha256": artifact.object.sha256,
            }))?;
            let quality_report_id =
                stable_request_id("quality", &format!("{asset_version_id}:{idempotency_key}"))?;
            let checked_at = utc_now_timestamp();
            let part_ids = version
                .parts
                .first()
                .and_then(|part| part.get("part_id"))
                .and_then(Value::as_str)
                .map(|part_id| vec![part_id.to_string()])
                .unwrap_or_default();
            let report = QualityReport {
                quality_report_id: quality_report_id.clone(),
                project_id: version.project_id.clone(),
                asset_version_id: asset_version_id.to_string(),
                report: json!({
                    "schema_version":"AgentAssetQualityReport@1",
                    "quality_report_id":quality_report_id,
                    "asset_version_id":asset_version_id,
                    "status":"warning",
                    "triangle_count":artifact.inspection.triangle_count,
                    "bounds_mm":artifact.inspection.bounds_mm,
                    "evidence_source":"external_glb_inspection",
                    "findings":[{
                        "check_id":"external_glb_reference",
                        "severity":"warning",
                        "message":"Verified self-contained GLB reference. Rebuild it as a ForgeCAD ShapeProgram before part editing or production-quality review.",
                        "part_ids":part_ids
                    }],
                    "checked_at":checked_at,
                }),
                status: QualityStatus::Warning,
                created_at: checked_at,
            };
            let stored = self.repository.attach_quality_idempotent(
                &report,
                expected,
                &scope,
                idempotency_key,
                &request_hash,
            )?;
            return json_response(200, quality_payload(&stored)?, Vec::new());
        }
        let object = self
            .repository
            .object_for_reference(&ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: asset_version_id.into(),
                role: "production_glb".into(),
            })?
            .ok_or_else(|| {
                CoreError::conflict_with_details(
                    "RUST_GEOMETRY_PREVIEW_REQUIRED",
                    "A Rust-owned production geometry artifact is required before quality can run.",
                    json!({
                        "asset_version_id": asset_version_id,
                        "snapshot_revision": snapshot.revision,
                        "required_artifact_role": "production_glb"
                    }),
                )
            })?;
        let bytes = self.repository.read_object(&object.sha256)?;
        let readback = read_glb_readback(&bytes)?;
        if readback.artifact_profile_id != "production_concept" {
            return Err(CoreError::conflict(
                "PRODUCTION_PROFILE_REQUIRED",
                "Quality accepts only a production_concept GLB readback.",
            ));
        }
        let shape_program_sha256 = semantic_sha256(&version.shape_program)?;
        let scope = format!("POST /api/v1/agent/asset-versions/{asset_version_id}:quality");
        let request_hash = semantic_sha256(&json!({
            "asset_version_id": asset_version_id,
            "snapshot_revision": expected.0,
            "quality_contract": "GeometryCompileReadback@2",
        }))?;
        let quality_report_id =
            stable_request_id("quality", &format!("{asset_version_id}:{idempotency_key}"))?;
        let checked_at = now_timestamp();
        let mut findings = Vec::new();
        if !readback.closed_manifold {
            findings.push(json!({
                "check_id":"closed_manifold",
                "severity":"error",
                "message":"Production GLB readback found open, non-manifold or degenerate triangle edges.",
                "part_ids":[]
            }));
        }
        if !readback.surface_provenance_present {
            findings.push(json!({
                "check_id":"surface_provenance",
                "severity":"error",
                "message":"Production GLB is missing stable source-face or Material Zone provenance.",
                "part_ids":[]
            }));
        }
        let quality_status = if findings.is_empty() {
            QualityStatus::Passed
        } else {
            QualityStatus::Failed
        };
        let report_payload = json!({
            "schema_version": "AgentAssetQualityReport@1",
            "quality_report_id": quality_report_id,
            "asset_version_id": asset_version_id,
            "status": quality_status,
            "triangle_count": readback.triangle_count,
            "bounds_mm": readback.bounds_mm,
            "evidence_source": "geometry_compile_readback",
            "compile_readback": {
                "schema_version": "GeometryCompileReadback@2",
                "runtime_manifest_version": readback.runtime_manifest_version,
                "artifact_profile": readback.artifact_profile,
                "shape_program_sha256": shape_program_sha256,
                "glb_sha256": object.sha256,
                "glb_byte_size": object.byte_size,
                "triangle_count": readback.triangle_count,
                "bounds_mm": readback.bounds_mm,
                "mesh_count": readback.mesh_count,
                "primitive_count": readback.primitive_count,
                "material_count": readback.material_count,
                "closed_manifold": readback.closed_manifold,
                "surface_provenance_present": readback.surface_provenance_present,
                "uv0_primitive_count": readback.uv0_primitive_count,
                "normal_primitive_count": readback.normal_primitive_count,
                "tangent_primitive_count": readback.tangent_primitive_count,
                "visual_texture_set_count": readback.visual_texture_set_count,
                "visual_texture_map_count": readback.visual_texture_map_count,
            },
            "findings": findings,
            "checked_at": checked_at,
        });
        let report = QualityReport {
            quality_report_id,
            project_id: version.project_id,
            asset_version_id: asset_version_id.to_string(),
            report: report_payload,
            status: quality_status,
            created_at: checked_at,
        };
        let stored = self.repository.attach_quality_idempotent(
            &report,
            expected,
            &scope,
            idempotency_key,
            &request_hash,
        )?;
        json_response(200, quality_payload(&stored)?, Vec::new())
    }

    fn list_components_response(
        &self,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let project_id = query_parameter(&request.path, "project_id").ok_or_else(|| {
            CoreError::invalid_data(
                "PROJECT_ID_REQUIRED",
                "Rust-owned component listing requires project_id.",
            )
        })?;
        let components = self.repository.list_components(
            project_id,
            query_parameter(&request.path, "domain_pack_id"),
            query_parameter(&request.path, "role"),
            query_parameter(&request.path, "q"),
            false,
        )?;
        json_response(
            200,
            serde_json::to_value(components).map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    fn save_component_response(
        &self,
        asset_version_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?;
        let part_id = required_bounded_string(&body, "part_id", 160)?;
        let display_name = required_bounded_string(&body, "display_name", 120)?;
        let description = body
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        if description.chars().count() > 500 || description.contains('\0') {
            return Err(CoreError::invalid_data(
                "AGENT_COMPONENT_DESCRIPTION_INVALID",
                "Component description exceeds the bounded contract.",
            ));
        }
        let component_id = stable_request_id(
            "agentcomp",
            &format!("{asset_version_id}:{client_request_id}"),
        )?;
        let scope = format!("POST /api/v1/agent/asset-versions/{asset_version_id}/components");
        let request_hash = semantic_sha256(&json!({
            "asset_version_id": asset_version_id,
            "request": body,
        }))?;
        let component = self.repository.save_component_idempotent(
            asset_version_id,
            &component_id,
            part_id,
            display_name,
            description,
            &now_timestamp(),
            &scope,
            client_request_id,
            &request_hash,
        )?;
        json_response(
            201,
            serde_json::to_value(component).map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    fn compatible_components_response(
        &self,
        asset_version_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let part_id = query_parameter(&request.path, "part_id").ok_or_else(|| {
            CoreError::invalid_data(
                "PART_ID_REQUIRED",
                "Component compatibility requires one target part_id.",
            )
        })?;
        let candidates = self
            .repository
            .component_candidates(asset_version_id, part_id)?;
        json_response(
            200,
            serde_json::to_value(candidates).map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    fn structure_suggestions_response(
        &self,
        asset_version_id: &str,
    ) -> CoreResult<CompatHttpResponse> {
        let suggestions = self.repository.structure_suggestions(asset_version_id)?;
        json_response(
            200,
            serde_json::to_value(suggestions).map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    fn semantic_proportions_response(
        &self,
        asset_version_id: &str,
        part_id: &str,
    ) -> CoreResult<CompatHttpResponse> {
        let resolved = resolve_semantic_proportions(&self.repository, asset_version_id, part_id)?;
        json_response(
            200,
            serde_json::to_value(resolved).map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    /// Produces no database, CAS, Version, Snapshot, or temporary-GLB write.
    /// The response is only a deterministic C105 candidate reference that a
    /// later `replace_part` ChangeSet may seal and replay.
    fn active_recipe_candidate_response(
        &self,
        asset_version_id: &str,
        target_part_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let input: ActiveRecipeCandidateRequest = serde_json::from_value(body).map_err(|_| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_REQUEST_INVALID",
                "Recipe candidate request does not match the bounded C105 contract.",
            )
        })?;
        if input.schema_version != "ComponentRecipeActiveCandidateRequest@1" {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_REQUEST_INVALID",
                "Recipe candidate request must use ComponentRecipeActiveCandidateRequest@1.",
            ));
        }
        // C105 only permits an explicit binding for a fixed, reviewed
        // optional child declared by the selected root Recipe.  Core owns the
        // exact ref/duplicate/domain validation and the candidate hash; this
        // boundary still rejects every free parameter or material variation.
        if !input.parameter_values.is_empty() || !input.material_zone_overrides.is_empty() {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_REQUEST_INVALID",
                "Active Recipe replacement accepts no parameter or material override in C105 v1.",
            ));
        }
        let version = self
            .repository
            .version(asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        let snapshot = self
            .repository
            .snapshot(&version.project_id)?
            .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
        let c105_registry = RecipeRegistry::from_embedded()?;
        let c106_registry = RecipeRegistry::from_embedded_c106_robotic_arm()?;
        let recipe_registry_sha256 = if c106_registry
            .recipe(&input.component_recipe_ref.recipe_id)
            .is_some()
        {
            c106_registry.registry_sha256().to_string()
        } else {
            c105_registry.registry_sha256().to_string()
        };
        let candidate = self.repository.instantiate_component_recipe_candidate(
            &RecipeInstantiationRequest {
                schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                context_mode: "active_asset_edit".into(),
                request_id: input.recipe_request_id,
                project_id: Some(version.project_id),
                base_asset_version_id: Some(version.asset_version_id),
                snapshot_revision: Some(snapshot.revision),
                domain_pack_id: version.domain_pack_id,
                recipe_registry_sha256,
                recipe: input.component_recipe_ref,
                target_part_id: Some(target_part_id.to_string()),
                slot_bindings: input.slot_bindings,
                parameter_values: Vec::new(),
                material_zone_overrides: Vec::new(),
            },
        )?;
        json_response(
            200,
            serde_json::to_value(candidate).map_err(json_encode_error)?,
            Vec::new(),
        )
    }

    fn propose_change_set_response(
        &self,
        asset_version_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let body = request_json(request)?;
        let client_request_id = require_idempotency_identity(request, &body)?;
        let version = self
            .repository
            .version(asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        let operations = body
            .get("operations")
            .and_then(Value::as_array)
            .filter(|operations| !operations.is_empty())
            .cloned()
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "CHANGE_SET_OPERATIONS_EMPTY",
                    "Agent ChangeSet must contain at least one operation.",
                )
            })?;
        let protected_part_ids = optional_string_array(&body, "protected_part_ids")?;
        let summary = required_bounded_string(&body, "summary", 2_000)?;
        let created_at = now_timestamp();
        let change_set_id = stable_request_id(
            "changeset",
            &format!("{asset_version_id}:{client_request_id}"),
        )?;
        let change_set = AgentAssetChangeSet {
            change_set_id: change_set_id.clone(),
            project_id: version.project_id,
            base_asset_version_id: asset_version_id.to_string(),
            summary: summary.to_string(),
            operations,
            protected_part_ids,
            preview: None,
            status: ChangeSetStatus::Proposed,
            resulting_asset_version_id: None,
            created_at: created_at.clone(),
            updated_at: created_at,
        };
        let scope = format!("POST /api/v1/agent/asset-versions/{asset_version_id}/change-sets");
        let request_hash = semantic_sha256(&json!({
            "asset_version_id": asset_version_id,
            "request": body,
        }))?;
        let stored = self.repository.create_change_set_idempotent(
            &change_set,
            &scope,
            client_request_id,
            &request_hash,
        )?;
        json_response(201, change_set_payload(&stored)?, Vec::new())
    }

    fn change_set_response(&self, change_set_id: &str) -> CoreResult<CompatHttpResponse> {
        let change_set = self
            .repository
            .change_set(change_set_id)?
            .ok_or_else(|| CoreError::not_found("Agent ChangeSet"))?;
        json_response(200, change_set_payload(&change_set)?, Vec::new())
    }

    fn reject_change_set_response(
        &self,
        change_set_id: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        let idempotency_key = required_header(request, "idempotency-key")?;
        let change_set = self
            .repository
            .change_set(change_set_id)?
            .ok_or_else(|| CoreError::not_found("Agent ChangeSet"))?;
        let snapshot = self
            .repository
            .snapshot(&change_set.project_id)?
            .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
        let scope = format!("POST /api/v1/agent/change-sets/{change_set_id}:reject");
        let request_hash = semantic_sha256(&json!({"change_set_id": change_set_id}))?;
        let stored = self.repository.reject_change_set_idempotent(
            change_set_id,
            snapshot.etag(),
            &now_timestamp(),
            &scope,
            idempotency_key,
            &request_hash,
        )?;
        json_response(200, change_set_payload(&stored)?, Vec::new())
    }

    fn geometry_change_set_required_response(
        &self,
        change_set_id: &str,
        action: &str,
        request: &PreparedCompatHttpRequest,
    ) -> CoreResult<CompatHttpResponse> {
        if request.method == AllowedHttpMethod::Post {
            required_header(request, "idempotency-key")?;
        }
        let change_set = self
            .repository
            .change_set(change_set_id)?
            .ok_or_else(|| CoreError::not_found("Agent ChangeSet"))?;
        let snapshot = self
            .repository
            .snapshot(&change_set.project_id)?
            .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
        Err(CoreError::conflict_with_details(
            "RUST_GEOMETRY_PREVIEW_REQUIRED",
            "This ChangeSet requires a Rust-orchestrated restricted geometry preview before it can continue.",
            json!({
                "action": action,
                "change_set_id": change_set.change_set_id,
                "change_set_status": change_set.status,
                "base_asset_version_id": change_set.base_asset_version_id,
                "active_asset_version_id": snapshot.active_design.asset_version_id(),
                "snapshot_revision": snapshot.revision,
                "snapshot_etag": snapshot.etag().to_string(),
            }),
        ))
    }

    fn preview_glb_response(&self, asset_version_id: &str) -> CoreResult<CompatHttpResponse> {
        let version = self
            .repository
            .version(asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        let snapshot = self
            .repository
            .snapshot(&version.project_id)?
            .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
        if snapshot.active_design.asset_version_id() != Some(asset_version_id) {
            return Err(CoreError::conflict(
                "ACTIVE_DESIGN_STALE",
                "Interactive preview is only available for the active Agent asset.",
            ));
        }
        if is_external_glb_reference(&version) {
            let artifact = self.external_reference_artifact(&version, &snapshot)?;
            return external_glb_binary_response(&artifact, None);
        }
        let object = self
            .repository
            .object_for_reference(&ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: asset_version_id.into(),
                role: "interactive_preview_glb".into(),
            })?
            .ok_or_else(|| CoreError::conflict_with_details(
                "RUST_GEOMETRY_PREVIEW_REQUIRED",
                "The interactive preview GLB has not been compiled by the restricted geometry executor.",
                json!({
                    "asset_version_id": asset_version_id,
                    "snapshot_revision": snapshot.revision,
                    "required_artifact_role": "interactive_preview_glb"
                }),
            ))?;
        let bytes = self.repository.read_object(&object.sha256)?;
        let readback = read_glb_readback(&bytes)?;
        if readback.artifact_profile_id != "interactive_preview" {
            return Err(CoreError::conflict(
                "INTERACTIVE_PROFILE_REQUIRED",
                "Preview object is not an interactive_preview artifact.",
            ));
        }
        let headers = vec![
            ("Content-Type".into(), "model/gltf-binary".into()),
            ("Cache-Control".into(), "no-store".into()),
            ("ETag".into(), format!("\"sha256:{}\"", object.sha256)),
            (
                "X-ForgeCAD-Artifact-Profile".into(),
                readback.artifact_profile_id,
            ),
            (
                "X-ForgeCAD-Artifact-Profile-SHA256".into(),
                readback.artifact_profile_sha256,
            ),
            (
                "X-ForgeCAD-Shape-Program-SHA256".into(),
                semantic_sha256(&version.shape_program)?,
            ),
            ("X-ForgeCAD-GLB-SHA256".into(), object.sha256),
            (
                "X-ForgeCAD-GLB-Byte-Size".into(),
                object.byte_size.to_string(),
            ),
            (
                "X-ForgeCAD-Triangle-Count".into(),
                readback.triangle_count.to_string(),
            ),
        ];
        Ok(CompatHttpResponse {
            schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
            status: 200,
            headers,
            body: ProtocolHttpBody::Base64 {
                data: BASE64_STANDARD.encode(bytes),
            },
        })
    }

    fn quality_response(&self, quality_report_id: &str) -> CoreResult<CompatHttpResponse> {
        let report = self
            .repository
            .quality_report(quality_report_id)?
            .ok_or_else(|| CoreError::not_found("QualityReport"))?;
        json_response(200, quality_payload(&report)?, Vec::new())
    }

    fn production_glb_response(&self, asset_version_id: &str) -> CoreResult<CompatHttpResponse> {
        if let Some(artifact) = self.external_reference_artifact_by_id(asset_version_id)? {
            return external_glb_binary_response(
                &artifact,
                Some(format!("{asset_version_id}.glb")),
            );
        }
        let artifact = self.production_artifact(asset_version_id)?;
        let mut headers = artifact_headers(&artifact);
        headers.push((
            "Content-Disposition".into(),
            format!("attachment; filename=\"{asset_version_id}.glb\""),
        ));
        Ok(CompatHttpResponse {
            schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
            status: 200,
            headers,
            body: ProtocolHttpBody::Base64 {
                data: BASE64_STANDARD.encode(&artifact.bytes),
            },
        })
    }

    fn export_response(&self, asset_version_id: &str) -> CoreResult<CompatHttpResponse> {
        if let Some(artifact) = self.external_reference_artifact_by_id(asset_version_id)? {
            return json_response(
                200,
                json!({
                    "schema_version":"AgentAssetExport@2",
                    "asset_version_id":asset_version_id,
                    "format":"glb",
                    "glb_base64":BASE64_STANDARD.encode(&artifact.bytes),
                    "artifact_profile_id":EXTERNAL_GLB_ARTIFACT_PROFILE_ID,
                    "artifact_profile_sha256":Value::Null,
                    "shape_program_sha256":Value::Null,
                    "glb_sha256":artifact.object.sha256,
                    "glb_byte_size":artifact.object.byte_size,
                    "triangle_count":artifact.inspection.triangle_count,
                    "bounds_mm":artifact.inspection.bounds_mm,
                    "readback_status":"passed",
                    "readback_triangle_count":artifact.inspection.triangle_count,
                    "exported_at":utc_now_timestamp(),
                }),
                Vec::new(),
            );
        }
        let artifact = self.production_artifact(asset_version_id)?;
        json_response(
            200,
            json!({
                "schema_version": "AgentAssetExport@2",
                "asset_version_id": asset_version_id,
                "format": "glb",
                "glb_base64": BASE64_STANDARD.encode(&artifact.bytes),
                "artifact_profile_id": "production_concept",
                "artifact_profile_sha256": artifact.artifact_profile_sha256,
                "shape_program_sha256": artifact.shape_program_sha256,
                "glb_sha256": artifact.object.sha256,
                "glb_byte_size": artifact.object.byte_size,
                "triangle_count": artifact.triangle_count,
                "bounds_mm": artifact.bounds_mm,
                "readback_status": "passed",
                "readback_triangle_count": artifact.triangle_count,
                "exported_at": now_timestamp(),
            }),
            Vec::new(),
        )
    }

    fn external_reference_artifact_by_id(
        &self,
        asset_version_id: &str,
    ) -> CoreResult<Option<ExternalReferenceArtifact>> {
        let version = self
            .repository
            .version(asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        if !is_external_glb_reference(&version) {
            return Ok(None);
        }
        let snapshot = self
            .repository
            .snapshot(&version.project_id)?
            .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
        self.external_reference_artifact(&version, &snapshot)
            .map(Some)
    }

    fn external_reference_artifact(
        &self,
        version: &AgentAssetVersion,
        snapshot: &ActiveDesignSnapshot,
    ) -> CoreResult<ExternalReferenceArtifact> {
        if !is_external_glb_reference(version)
            || snapshot.active_design.asset_version_id() != Some(version.asset_version_id.as_str())
        {
            return Err(CoreError::conflict(
                "ACTIVE_DESIGN_STALE",
                "External GLB reference is only available for the active immutable Version.",
            ));
        }
        let object = self
            .repository
            .object_for_reference(&ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: version.asset_version_id.clone(),
                role: EXTERNAL_GLB_REFERENCE_ROLE.into(),
            })?
            .ok_or_else(|| {
                CoreError::conflict(
                    "EXTERNAL_GLB_UNAVAILABLE",
                    "External GLB reference has no Rust-owned CAS object binding.",
                )
            })?;
        if object.extension != "glb" {
            return Err(CoreError::conflict(
                "EXTERNAL_GLB_UNAVAILABLE",
                "External reference object is not an immutable GLB.",
            ));
        }
        let bytes = self.repository.read_object(&object.sha256)?;
        let inspection = inspect_external_glb(&bytes).map_err(|_| {
            CoreError::conflict(
                "EXTERNAL_GLB_UNAVAILABLE",
                "External GLB CAS bytes no longer satisfy strict self-contained readback.",
            )
        })?;
        let marker_sha = version
            .shape_program
            .get("source_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CoreError::conflict(
                    "EXTERNAL_GLB_UNAVAILABLE",
                    "External reference marker is missing its immutable source SHA-256.",
                )
            })?;
        if marker_sha != object.sha256
            || inspection.sha256 != object.sha256
            || inspection.byte_size != object.byte_size
        {
            return Err(CoreError::conflict(
                "EXTERNAL_GLB_UNAVAILABLE",
                "External GLB marker, CAS metadata and strict readback disagree.",
            ));
        }
        if let Some(imported) = self.repository.imported_glb(&version.asset_version_id)? {
            if imported.sha256 != inspection.sha256
                || imported.byte_size != inspection.byte_size
                || imported.inspection() != inspection
            {
                return Err(CoreError::conflict(
                    "EXTERNAL_GLB_UNAVAILABLE",
                    "External GLB legacy import metadata differs from strict Rust readback.",
                ));
            }
        }
        Ok(ExternalReferenceArtifact {
            object,
            bytes,
            inspection,
        })
    }

    fn production_artifact(&self, asset_version_id: &str) -> CoreResult<ProductionArtifact> {
        let version = self
            .repository
            .version(asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        let snapshot = self
            .repository
            .snapshot(&version.project_id)?
            .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
        if snapshot.active_design.asset_version_id() != Some(asset_version_id) {
            return Err(CoreError::conflict(
                "ACTIVE_DESIGN_STALE",
                "The requested asset is not the active design.",
            ));
        }
        let quality_reference = snapshot.quality.as_ref().ok_or_else(|| {
            CoreError::conflict(
                "EXPORT_QUALITY_REQUIRED",
                "Production GLB requires Snapshot-bound compile readback quality.",
            )
        })?;
        let quality = self
            .repository
            .quality_report(&quality_reference.quality_report_id)?
            .ok_or_else(|| CoreError::not_found("QualityReport"))?;
        if quality.asset_version_id != asset_version_id {
            return Err(CoreError::conflict(
                "QUALITY_ASSET_STALE",
                "Quality report does not belong to the requested asset version.",
            ));
        }
        let object = self
            .repository
            .object_for_reference(&ObjectReference {
                reference_kind: "asset_version".into(),
                owner_id: asset_version_id.into(),
                role: "production_glb".into(),
            })?
            .ok_or_else(|| CoreError::not_found("production GLB"))?;
        let bytes = self.repository.read_object(&object.sha256)?;
        if bytes.len() < 20 || bytes.get(0..4) != Some(b"glTF") {
            return Err(CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "CAS object is not a valid binary glTF artifact.",
            ));
        }
        production_artifact_from_readback(version, quality, object, bytes)
    }
}

#[derive(Debug)]
struct ProductionArtifact {
    object: ObjectRecord,
    bytes: Vec<u8>,
    artifact_profile_sha256: String,
    shape_program_sha256: String,
    triangle_count: u64,
    bounds_mm: Vec<f64>,
}

#[derive(Debug)]
struct ExternalReferenceArtifact {
    object: ObjectRecord,
    bytes: Vec<u8>,
    inspection: ImportedGlbInspection,
}

#[derive(Debug)]
struct GlbReadback {
    artifact_profile_id: String,
    artifact_profile_sha256: String,
    artifact_profile: Value,
    runtime_manifest_version: String,
    triangle_count: u64,
    bounds_mm: Vec<f64>,
    mesh_count: u64,
    primitive_count: u64,
    material_count: u64,
    closed_manifold: bool,
    surface_provenance_present: bool,
    uv0_primitive_count: u64,
    normal_primitive_count: u64,
    tangent_primitive_count: u64,
    visual_texture_set_count: u64,
    visual_texture_map_count: u64,
}

fn read_glb_readback(bytes: &[u8]) -> CoreResult<GlbReadback> {
    let readback = verify_forgecad_glb(bytes, None)?;
    Ok(GlbReadback {
        artifact_profile_id: readback.artifact_profile_id,
        artifact_profile_sha256: readback.artifact_profile_sha256,
        artifact_profile: readback.artifact_profile,
        runtime_manifest_version: readback.runtime_manifest_version,
        triangle_count: readback.triangle_count,
        bounds_mm: readback.bounds_mm,
        mesh_count: readback.mesh_count,
        primitive_count: readback.primitive_count,
        material_count: readback.material_count,
        closed_manifold: readback.closed_manifold,
        surface_provenance_present: readback.surface_provenance_present,
        uv0_primitive_count: readback.uv0_primitive_count,
        normal_primitive_count: readback.normal_primitive_count,
        tangent_primitive_count: readback.tangent_primitive_count,
        visual_texture_set_count: readback.visual_texture_set_count,
        visual_texture_map_count: readback.visual_texture_map_count,
    })
}

#[allow(dead_code)]
fn legacy_read_glb_readback(bytes: &[u8]) -> CoreResult<GlbReadback> {
    if bytes.len() < 20 || bytes.get(..4) != Some(b"glTF") {
        return Err(CoreError::invalid_data(
            "PRODUCTION_GLB_INVALID",
            "CAS object is not a binary glTF artifact.",
        ));
    }
    let version = read_u32_le(bytes, 4)?;
    let declared_length = read_u32_le(bytes, 8)? as usize;
    if version != 2 || declared_length != bytes.len() {
        return Err(CoreError::invalid_data(
            "PRODUCTION_GLB_INVALID",
            "Binary glTF header version or byte length is invalid.",
        ));
    }
    let mut cursor = 12usize;
    let mut json_chunk = None;
    let mut binary_chunk = None;
    while cursor < bytes.len() {
        if cursor.checked_add(8).map_or(true, |end| end > bytes.len()) {
            return Err(CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "Binary glTF chunk header is truncated.",
            ));
        }
        let length = read_u32_le(bytes, cursor)? as usize;
        let kind = read_u32_le(bytes, cursor + 4)?;
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "PRODUCTION_GLB_INVALID",
                    "Binary glTF chunk extends beyond the declared payload.",
                )
            })?;
        match kind {
            0x4e4f534a if json_chunk.is_none() => json_chunk = Some(&bytes[start..end]),
            0x004e4942 if binary_chunk.is_none() => binary_chunk = Some(&bytes[start..end]),
            _ => {}
        }
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(CoreError::invalid_data(
            "PRODUCTION_GLB_INVALID",
            "Binary glTF chunks do not consume the declared payload.",
        ));
    }
    let json_bytes = json_chunk.ok_or_else(|| {
        CoreError::invalid_data(
            "PRODUCTION_GLB_INVALID",
            "Binary glTF is missing its JSON chunk.",
        )
    })?;
    let document: Value = serde_json::from_slice(json_bytes).map_err(|_| {
        CoreError::invalid_data(
            "PRODUCTION_GLB_INVALID",
            "Binary glTF JSON chunk cannot be decoded.",
        )
    })?;
    let profile = document
        .get("extras")
        .and_then(|extras| extras.get("forgecad_geometry_artifact_profile"))
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| {
            CoreError::invalid_data(
                "GEOMETRY_PROFILE_MISSING",
                "GLB is missing the code-owned geometry artifact profile.",
            )
        })?;
    let artifact_profile_id = profile
        .get("artifact_profile_id")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "interactive_preview" | "production_concept"))
        .ok_or_else(|| {
            CoreError::invalid_data(
                "GEOMETRY_PROFILE_INVALID",
                "GLB artifact profile identity is invalid.",
            )
        })?
        .to_string();
    let artifact_profile_sha256 =
        required_sha(profile.get("profile_sha256"), "artifact profile SHA-256")?;
    let mut unsigned_profile = profile.clone();
    unsigned_profile.remove("profile_sha256");
    if Value::Object(unsigned_profile.clone()) != geometry_profile_contract(&artifact_profile_id)
        || semantic_sha256(&unsigned_profile)? != artifact_profile_sha256
    {
        return Err(CoreError::invalid_data(
            "GEOMETRY_PROFILE_INVALID",
            "GLB artifact profile does not match the code-owned manifest and semantic hash.",
        ));
    }
    let accessors = document
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB has no accessors.")
        })?;
    let views = document
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB has no buffer views.")
        })?;
    let meshes = document
        .get("meshes")
        .and_then(Value::as_array)
        .filter(|meshes| !meshes.is_empty())
        .ok_or_else(|| CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB has no meshes."))?;
    let binary = binary_chunk.ok_or_else(|| {
        CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB is missing its binary chunk.")
    })?;
    let mut triangle_count = 0u64;
    let mut primitive_count = 0u64;
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    let mut closed_manifold = true;
    let mut surface_provenance_present = true;
    for mesh in meshes {
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "PRODUCTION_GLB_INVALID",
                    "GLB mesh has no triangle primitives.",
                )
            })?;
        for primitive in primitives {
            if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
                return Err(CoreError::invalid_data(
                    "PRODUCTION_GLB_INVALID",
                    "GLB primitive is not a triangle list.",
                ));
            }
            primitive_count += 1;
            let index_accessor = primitive
                .get("indices")
                .and_then(Value::as_u64)
                .and_then(|index| accessors.get(index as usize))
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "PRODUCTION_GLB_INVALID",
                        "GLB primitive index accessor is invalid.",
                    )
                })?;
            let indices = read_index_accessor(index_accessor, views, binary)?;
            if indices.is_empty() || indices.len() % 3 != 0 {
                return Err(CoreError::invalid_data(
                    "PRODUCTION_GLB_INVALID",
                    "GLB index accessor is not a non-empty triangle list.",
                ));
            }
            let position_accessor = primitive
                .get("attributes")
                .and_then(|attributes| attributes.get("POSITION"))
                .and_then(Value::as_u64)
                .and_then(|index| accessors.get(index as usize))
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "PRODUCTION_GLB_INVALID",
                        "GLB primitive POSITION accessor is invalid.",
                    )
                })?;
            let positions = read_position_accessor(position_accessor, views, binary)?;
            triangle_count += (indices.len() / 3) as u64;
            closed_manifold &= triangle_edges_are_closed(&indices, &positions);
            update_accessor_bounds(position_accessor, &mut minimum, &mut maximum)?;
            let extras = primitive.get("extras").and_then(Value::as_object);
            let source_face_count = extras
                .and_then(|extras| extras.get("forgecad_source_face_ids"))
                .and_then(Value::as_array)
                .map(Vec::len);
            surface_provenance_present &= extras.is_some_and(|extras| {
                extras
                    .get("forgecad_feature_node_id")
                    .and_then(Value::as_str)
                    .is_some()
                    && extras
                        .get("forgecad_material_zone_id")
                        .and_then(Value::as_str)
                        .is_some()
                    && extras
                        .get("forgecad_surface_ranges")
                        .and_then(Value::as_array)
                        .is_some()
            }) && source_face_count == Some(indices.len() / 3);
        }
    }
    if triangle_count == 0
        || minimum
            .iter()
            .chain(maximum.iter())
            .any(|value| !value.is_finite())
    {
        return Err(CoreError::invalid_data(
            "PRODUCTION_GLB_INVALID",
            "GLB readback has no finite triangle bounds.",
        ));
    }
    let bounds_mm = (0..3)
        .map(|axis| ((maximum[axis] - minimum[axis]) * 1_000.0 * 10_000.0).round() / 10_000.0)
        .collect::<Vec<_>>();
    Ok(GlbReadback {
        artifact_profile_id,
        artifact_profile_sha256,
        artifact_profile: Value::Object(profile),
        runtime_manifest_version: "ShapeProgramRuntimeManifest@1".into(),
        triangle_count,
        bounds_mm,
        mesh_count: meshes.len() as u64,
        primitive_count,
        material_count: document
            .get("materials")
            .and_then(Value::as_array)
            .map_or(0, |items| items.len() as u64),
        closed_manifold,
        surface_provenance_present,
        uv0_primitive_count: primitive_count,
        normal_primitive_count: primitive_count,
        tangent_primitive_count: primitive_count,
        visual_texture_set_count: 0,
        visual_texture_map_count: 0,
    })
}

fn geometry_profile_contract(profile_id: &str) -> Value {
    let production = profile_id == "production_concept";
    json!({
        "schema_version": "GeometryArtifactProfile@1",
        "artifact_profile_id": profile_id,
        "radial_segments": if production { 64 } else { 24 },
        "capsule_hemisphere_segments": if production { 14 } else { 5 },
        "smooth_loft_normals": production,
        "texture_width": if production { 1024 } else { 128 },
        "texture_height": if production { 1024 } else { 128 },
        "texture_mime_type": "image/png",
        "texture_compression": "png_deflate",
        "delivery": if production { "on_demand" } else { "interactive" },
        "triangle_budget_multiplier": if production { 6 } else { 1 },
        "max_triangle_count": if production { 250_000 } else { 100_000 },
    })
}

fn read_u32_le(bytes: &[u8], offset: usize) -> CoreResult<u32> {
    let raw: [u8; 4] = bytes
        .get(offset..offset + 4)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "Binary glTF integer is truncated.",
            )
        })?;
    Ok(u32::from_le_bytes(raw))
}

fn read_index_accessor(accessor: &Value, views: &[Value], binary: &[u8]) -> CoreResult<Vec<u32>> {
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0 && *count <= 3_000_000)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "GLB index accessor count is invalid.",
            )
        })? as usize;
    let component_type = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "GLB index component type is missing.",
            )
        })?;
    let component_size = match component_type {
        5123 => 2usize,
        5125 => 4usize,
        _ => {
            return Err(CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "GLB indices must use unsigned 16-bit or 32-bit components.",
            ))
        }
    };
    let view = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|index| views.get(index as usize))
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "GLB index buffer view is invalid.",
            )
        })?;
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .unwrap_or(component_size as u64) as usize;
    if stride < component_size {
        return Err(CoreError::invalid_data(
            "PRODUCTION_GLB_INVALID",
            "GLB index accessor stride is invalid.",
        ));
    }
    let start = view_offset.checked_add(accessor_offset).ok_or_else(|| {
        CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB index offset overflowed.")
    })?;
    let mut result = Vec::with_capacity(count);
    for ordinal in 0..count {
        let offset = start
            .checked_add(ordinal.checked_mul(stride).ok_or_else(|| {
                CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB index stride overflowed.")
            })?)
            .ok_or_else(|| {
                CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB index offset overflowed.")
            })?;
        let value = match component_type {
            5123 => {
                let raw: [u8; 2] = binary
                    .get(offset..offset + 2)
                    .and_then(|slice| slice.try_into().ok())
                    .ok_or_else(|| {
                        CoreError::invalid_data(
                            "PRODUCTION_GLB_INVALID",
                            "GLB index data is truncated.",
                        )
                    })?;
                u16::from_le_bytes(raw) as u32
            }
            5125 => read_u32_le(binary, offset)?,
            _ => unreachable!(),
        };
        result.push(value);
    }
    Ok(result)
}

fn read_position_accessor(
    accessor: &Value,
    views: &[Value],
    binary: &[u8],
) -> CoreResult<Vec<[i64; 3]>> {
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0 && *count <= 3_000_000)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "GLB POSITION accessor count is invalid.",
            )
        })? as usize;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("VEC3")
    {
        return Err(CoreError::invalid_data(
            "PRODUCTION_GLB_INVALID",
            "GLB POSITION accessor must use float32 VEC3 data.",
        ));
    }
    let view = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|index| views.get(index as usize))
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "GLB POSITION buffer view is invalid.",
            )
        })?;
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let stride = view.get("byteStride").and_then(Value::as_u64).unwrap_or(12) as usize;
    if stride < 12 {
        return Err(CoreError::invalid_data(
            "PRODUCTION_GLB_INVALID",
            "GLB POSITION accessor stride is invalid.",
        ));
    }
    let start = view_offset.checked_add(accessor_offset).ok_or_else(|| {
        CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB POSITION offset overflowed.")
    })?;
    let mut positions = Vec::with_capacity(count);
    for ordinal in 0..count {
        let offset = start
            .checked_add(ordinal.checked_mul(stride).ok_or_else(|| {
                CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB POSITION stride overflowed.")
            })?)
            .ok_or_else(|| {
                CoreError::invalid_data("PRODUCTION_GLB_INVALID", "GLB POSITION offset overflowed.")
            })?;
        let mut point = [0i64; 3];
        for (axis, target) in point.iter_mut().enumerate() {
            let component_offset = offset + axis * 4;
            let raw: [u8; 4] = binary
                .get(component_offset..component_offset + 4)
                .and_then(|slice| slice.try_into().ok())
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "PRODUCTION_GLB_INVALID",
                        "GLB POSITION data is truncated.",
                    )
                })?;
            let value = f32::from_le_bytes(raw) as f64;
            if !value.is_finite() || value.abs() > 10_000.0 {
                return Err(CoreError::invalid_data(
                    "PRODUCTION_GLB_INVALID",
                    "GLB POSITION contains a non-finite or unbounded value.",
                ));
            }
            // Match the Worker readback contract: topology is evaluated after
            // welding coincident split-normal/UV vertices at eight decimals.
            *target = (value * 100_000_000.0).round() as i64;
        }
        positions.push(point);
    }
    Ok(positions)
}

fn triangle_edges_are_closed(indices: &[u32], positions: &[[i64; 3]]) -> bool {
    let mut edges = std::collections::BTreeMap::<([i64; 3], [i64; 3]), u32>::new();
    for triangle in indices.chunks_exact(3) {
        let Some(points) = triangle
            .iter()
            .map(|index| positions.get(*index as usize).copied())
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        if points[0] == points[1] || points[1] == points[2] || points[2] == points[0] {
            return false;
        }
        for (left, right) in [
            (points[0], points[1]),
            (points[1], points[2]),
            (points[2], points[0]),
        ] {
            let edge = if left < right {
                (left, right)
            } else {
                (right, left)
            };
            *edges.entry(edge).or_default() += 1;
        }
    }
    !edges.is_empty() && edges.values().all(|count| *count == 2)
}

fn update_accessor_bounds(
    accessor: &Value,
    minimum: &mut [f64; 3],
    maximum: &mut [f64; 3],
) -> CoreResult<()> {
    let lower = accessor
        .get("min")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "GLB POSITION accessor is missing finite minimum bounds.",
            )
        })?;
    let upper = accessor
        .get("max")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PRODUCTION_GLB_INVALID",
                "GLB POSITION accessor is missing finite maximum bounds.",
            )
        })?;
    for axis in 0..3 {
        let lower = lower[axis]
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "PRODUCTION_GLB_INVALID",
                    "GLB POSITION minimum is not finite.",
                )
            })?;
        let upper = upper[axis]
            .as_f64()
            .filter(|value| value.is_finite() && *value >= lower)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "PRODUCTION_GLB_INVALID",
                    "GLB POSITION maximum is invalid.",
                )
            })?;
        minimum[axis] = minimum[axis].min(lower);
        maximum[axis] = maximum[axis].max(upper);
    }
    Ok(())
}

fn production_artifact_from_readback(
    version: AgentAssetVersion,
    quality: QualityReport,
    object: ObjectRecord,
    bytes: Vec<u8>,
) -> CoreResult<ProductionArtifact> {
    let report = quality_payload(&quality)?;
    if report.get("status").and_then(Value::as_str) != Some("passed")
        || report.get("evidence_source").and_then(Value::as_str)
            != Some("geometry_compile_readback")
    {
        return Err(CoreError::conflict(
            "EXPORT_QUALITY_REQUIRED",
            "Production GLB requires passed geometry compile readback.",
        ));
    }
    let readback = report
        .get("compile_readback")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CoreError::conflict(
                "EXPORT_QUALITY_REQUIRED",
                "Production GLB quality is missing compile readback.",
            )
        })?;
    let canonical = verify_forgecad_glb(&bytes, Some("production_concept"))?;
    if readback.get("schema_version").and_then(Value::as_str) != Some("GeometryCompileReadback@2") {
        return Err(CoreError::conflict(
            "COMPILE_READBACK_STALE",
            "Production export requires GeometryCompileReadback@2.",
        ));
    }
    let artifact_profile = readback
        .get("artifact_profile")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "COMPILE_READBACK_INVALID",
                "Compile readback is missing its artifact profile.",
            )
        })?;
    if artifact_profile
        .get("artifact_profile_id")
        .and_then(Value::as_str)
        != Some("production_concept")
    {
        return Err(CoreError::conflict(
            "PRODUCTION_PROFILE_REQUIRED",
            "Only the production_concept artifact may be exported.",
        ));
    }
    let artifact_profile_sha256 = required_sha(
        artifact_profile.get("profile_sha256"),
        "artifact profile SHA-256",
    )?;
    if artifact_profile_sha256 != canonical.artifact_profile_sha256 {
        return Err(CoreError::conflict(
            "COMPILE_READBACK_PROFILE_STALE",
            "Compile readback profile does not match the canonical GLB profile manifest.",
        ));
    }
    if readback
        .get("runtime_manifest_version")
        .and_then(Value::as_str)
        != Some(canonical.runtime_manifest_version.as_str())
    {
        return Err(CoreError::conflict(
            "COMPILE_READBACK_RUNTIME_STALE",
            "Compile readback runtime manifest does not match the GLB feature history.",
        ));
    }
    let shape_program_sha256 =
        required_sha(readback.get("shape_program_sha256"), "ShapeProgram SHA-256")?;
    if shape_program_sha256 != semantic_sha256(&version.shape_program)? {
        return Err(CoreError::conflict(
            "COMPILE_READBACK_SHAPE_STALE",
            "Compile readback does not match the immutable ShapeProgram.",
        ));
    }
    let readback_glb_sha256 = required_sha(readback.get("glb_sha256"), "GLB SHA-256")?;
    let readback_size = readback
        .get("glb_byte_size")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "COMPILE_READBACK_INVALID",
                "Compile readback is missing GLB byte size.",
            )
        })?;
    if readback_glb_sha256 != object.sha256
        || readback_glb_sha256 != canonical.glb_sha256
        || readback_size != object.byte_size
        || readback_size != canonical.glb_byte_size
        || readback_size != bytes.len() as u64
    {
        return Err(CoreError::conflict(
            "COMPILE_READBACK_GLB_STALE",
            "Compile readback does not match the CAS production GLB.",
        ));
    }
    let triangle_count = readback
        .get("triangle_count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "COMPILE_READBACK_INVALID",
                "Compile readback must contain a positive triangle count.",
            )
        })?;
    if triangle_count != canonical.triangle_count {
        return Err(CoreError::conflict(
            "COMPILE_READBACK_GEOMETRY_STALE",
            "Compile readback triangle count does not match canonical GLB readback.",
        ));
    }
    let bounds_mm = readback
        .get("bounds_mm")
        .and_then(Value::as_array)
        .filter(|bounds| bounds.len() == 3)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "COMPILE_READBACK_INVALID",
                "Compile readback must contain three positive bounds.",
            )
        })?
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|item| item.is_finite() && *item > 0.0)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            CoreError::invalid_data(
                "COMPILE_READBACK_INVALID",
                "Compile readback bounds must be finite and positive.",
            )
        })?;
    if bounds_mm != canonical.bounds_mm {
        return Err(CoreError::conflict(
            "COMPILE_READBACK_GEOMETRY_STALE",
            "Compile readback bounds do not match canonical GLB readback.",
        ));
    }
    let report_bounds = report
        .get("bounds_mm")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_f64).collect::<Vec<_>>());
    if report.get("triangle_count").and_then(Value::as_u64) != Some(triangle_count)
        || report_bounds.as_deref() != Some(bounds_mm.as_slice())
    {
        return Err(CoreError::conflict(
            "QUALITY_READBACK_MISMATCH",
            "Quality facts do not match their compile readback.",
        ));
    }
    Ok(ProductionArtifact {
        object,
        bytes,
        artifact_profile_sha256,
        shape_program_sha256,
        triangle_count,
        bounds_mm,
    })
}

fn artifact_headers(artifact: &ProductionArtifact) -> Vec<(String, String)> {
    vec![
        ("Cache-Control".into(), "no-store".into()),
        ("Content-Type".into(), "model/gltf-binary".into()),
        ("ETag".into(), format!("\"{}\"", artifact.object.sha256)),
        (
            "X-ForgeCAD-Artifact-Profile".into(),
            "production_concept".into(),
        ),
        (
            "X-ForgeCAD-Artifact-Profile-SHA256".into(),
            artifact.artifact_profile_sha256.clone(),
        ),
        (
            "X-ForgeCAD-Shape-Program-SHA256".into(),
            artifact.shape_program_sha256.clone(),
        ),
        (
            "X-ForgeCAD-GLB-SHA256".into(),
            artifact.object.sha256.clone(),
        ),
        (
            "X-ForgeCAD-GLB-Byte-Size".into(),
            artifact.object.byte_size.to_string(),
        ),
        (
            "X-ForgeCAD-Triangle-Count".into(),
            artifact.triangle_count.to_string(),
        ),
    ]
}

fn external_glb_binary_response(
    artifact: &ExternalReferenceArtifact,
    filename: Option<String>,
) -> CoreResult<CompatHttpResponse> {
    let mut headers = vec![
        ("Cache-Control".into(), "no-store".into()),
        ("Content-Type".into(), "model/gltf-binary".into()),
        ("ETag".into(), format!("\"{}\"", artifact.object.sha256)),
        (
            "X-ForgeCAD-Artifact-Profile".into(),
            EXTERNAL_GLB_ARTIFACT_PROFILE_ID.into(),
        ),
        (
            "X-ForgeCAD-GLB-SHA256".into(),
            artifact.object.sha256.clone(),
        ),
        (
            "X-ForgeCAD-GLB-Byte-Size".into(),
            artifact.object.byte_size.to_string(),
        ),
        (
            "X-ForgeCAD-Triangle-Count".into(),
            artifact.inspection.triangle_count.to_string(),
        ),
    ];
    if let Some(filename) = filename {
        headers.push((
            "Content-Disposition".into(),
            format!("attachment; filename=\"{filename}\""),
        ));
    }
    Ok(CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
        status: 200,
        headers,
        body: ProtocolHttpBody::Base64 {
            data: BASE64_STANDARD.encode(&artifact.bytes),
        },
    })
}

fn project_detail(project: &Project, profile: Value) -> Value {
    let intended_uses = profile
        .get("intended_uses")
        .cloned()
        .unwrap_or_else(|| json!(["non_functional_display"]));
    let mut detail = serde_json::to_value(project)
        .unwrap_or_else(|_| Value::Object(Map::new()))
        .as_object()
        .cloned()
        .unwrap_or_default();
    detail.insert("profile".into(), profile);
    detail.insert(
        "current_spec".into(),
        json!({
            "schema_version": "WeaponConceptSpec@1",
            "project_id": project.project_id,
            "profile_id": project.profile_id,
            "name": project.name,
            "archetype": "future_modular_sidearm",
            "intended_uses": intended_uses,
            "style": {
                "keywords": ["unconfigured_concept"],
                "palette": ["neutral"],
                "detail_density": 0.0
            },
            "proportions": {
                "overall_length_mm": 1.0,
                "body_height_mm": 1.0,
                "grip_angle_deg": 0.0
            },
            "required_slots": ["core", "front", "rear", "grip"],
            "optional_slots": [],
            "constraints": {
                "symmetry": "mostly_symmetric",
                "max_triangle_count": 200000
            },
            "assumptions": ["非功能性概念资产；尚未由 Agent 生成首个可编辑版本"]
        }),
    );
    // K003 Project creation must not silently create a legacy Version or
    // ModuleGraph. AgentAssetVersion history is read through its own API.
    detail.insert("versions".into(), Value::Array(Vec::new()));
    Value::Object(detail)
}

fn is_retired_legacy_get(segments: &[&str]) -> bool {
    if segments.len() < 3 || segments[..3] != ["api", "v1", "module-assets"] {
        // Continue with the other frozen Concept read surfaces below.
    } else {
        return true;
    }
    if segments.len() >= 5
        && segments[..3] == ["api", "v1", "projects"]
        && matches!(
            segments[4],
            "variants" | "change-sets" | "change-set-audit-exports"
        )
    {
        return true;
    }
    if segments.len() > 4 && segments[..3] == ["api", "v1", "versions"] {
        return true;
    }
    matches!(
        segments,
        ["api", "v1", "quality-runs", ..]
            | ["api", "v1", "jobs", ..]
            | ["api", "v1", "concept-jobs", ..]
            | ["api", "v1", "exports", ..]
            | ["api", "v1", "change-sets", ..]
            | ["api", "v1", "change-set-audit-exports", ..]
            | ["api", "v1", "module-graphs"]
            | ["api", "v1", "module-graphs", _, ..]
    )
}

fn retired_legacy_get_response(_route: &str) -> CoreResult<CompatHttpResponse> {
    json_response(
        410,
        json!({
            "error": {
                "code": "LEGACY_CONCEPT_ROUTE_RETIRED",
                "message": "This historical Concept read route is not part of the bounded Rust legacy-detail adapter.",
                "recoverable": false,
                "details": {}
            }
        }),
        Vec::new(),
    )
}

fn asset_version_payload(version: &AgentAssetVersion) -> CoreResult<Value> {
    let mut payload = serde_json::to_value(version)
        .map_err(json_encode_error)?
        .as_object()
        .cloned()
        .ok_or_else(|| {
            CoreError::invalid_data(
                "ASSET_VERSION_INVALID",
                "AgentAssetVersion did not serialize as an object.",
            )
        })?;
    payload.insert(
        "schema_version".into(),
        Value::String("AgentAssetVersion@1".into()),
    );
    Ok(Value::Object(payload))
}

fn quality_payload(report: &QualityReport) -> CoreResult<Value> {
    let mut payload = report.report.as_object().cloned().ok_or_else(|| {
        CoreError::invalid_data(
            "QUALITY_REPORT_INVALID",
            "Quality report payload must be a JSON object.",
        )
    })?;
    payload.insert(
        "schema_version".into(),
        Value::String("AgentAssetQualityReport@1".into()),
    );
    payload.insert(
        "quality_report_id".into(),
        Value::String(report.quality_report_id.clone()),
    );
    payload.insert(
        "asset_version_id".into(),
        Value::String(report.asset_version_id.clone()),
    );
    payload.insert(
        "status".into(),
        serde_json::to_value(report.status).map_err(json_encode_error)?,
    );
    payload.insert(
        "checked_at".into(),
        Value::String(report.created_at.clone()),
    );
    Ok(Value::Object(payload))
}

/// Detects the R007B path from immutable active-asset provenance, never from
/// a domain label. A C106 child without its exact reviewed root is a malformed
/// active graph rather than permission to fall back to C105.
fn active_c106_root_recipe_id(
    base: &AgentAssetVersion,
    registry: &RecipeRegistry,
) -> CoreResult<Option<String>> {
    const ROOT_IDS: [&str; 3] = [
        "recipe_c106_arm_desktop_assistant",
        "recipe_c106_arm_gallery_industrial",
        "recipe_c106_arm_service_display",
    ];
    let Some(instances) = base
        .assembly_graph
        .get("component_recipe_instances")
        .and_then(Value::as_array)
    else {
        return Ok(None);
    };
    let has_c106_marker = instances.iter().any(|instance| {
        instance.get("registry_sha256").and_then(Value::as_str) == Some(registry.registry_sha256())
            || instance
                .pointer("/recipe/recipe_id")
                .and_then(Value::as_str)
                .is_some_and(|id| ROOT_IDS.contains(&id) || registry.recipe(id).is_some())
    });
    let Some(root_instance) = instances.iter().find(|instance| {
        let recipe_id = instance
            .pointer("/recipe/recipe_id")
            .and_then(Value::as_str);
        ROOT_IDS.contains(&recipe_id.unwrap_or_default())
            && instance.get("registry_sha256").and_then(Value::as_str)
                == Some(registry.registry_sha256())
            && instance.get("instance_path").and_then(Value::as_str) == Some("root")
            && instance.get("parent_instance_id") == Some(&Value::Null)
            && instance.get("parent_slot_id") == Some(&Value::Null)
    }) else {
        return if has_c106_marker {
            Err(CoreError::invalid_data(
                "REFERENCE_REBUILD_C106_PROVENANCE_INVALID",
                "A C106 reference rebuild requires one exact reviewed C106 root provenance in the active AssemblyGraph.",
            ))
        } else {
            Ok(None)
        };
    };
    let root_part_id = base
        .assembly_graph
        .get("root_part_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "REFERENCE_REBUILD_ASSEMBLY_INVALID",
                "C106 AssemblyGraph has no root Part.",
            )
        })?;
    let root_instance_id = root_instance
        .get("instance_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "REFERENCE_REBUILD_C106_PROVENANCE_INVALID",
                "C106 root provenance has no instance identity.",
            )
        })?;
    let root_part_has_provenance = base
        .assembly_graph
        .get("parts")
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts
                .iter()
                .find(|part| part.get("part_id").and_then(Value::as_str) == Some(root_part_id))
        })
        .and_then(|part| part.get("recipe_instance_id").and_then(Value::as_str))
        == Some(root_instance_id);
    if !root_part_has_provenance {
        return Err(CoreError::invalid_data(
            "REFERENCE_REBUILD_C106_PROVENANCE_INVALID",
            "C106 root Part must retain its exact immutable recipe-instance provenance.",
        ));
    }
    Ok(root_instance
        .pointer("/recipe/recipe_id")
        .and_then(Value::as_str)
        .map(str::to_string))
}

/// The one sealed, user-visible R007B action selected from read-only evidence.
///
/// This is intentionally not a new Design Surface executor.  It only lowers
/// evidence to one pre-existing, validated ChangeSet operation.  Keeping the
/// operation itself here makes the result auditable through the normal
/// preview/confirm readback and prevents a plan from claiming an effect that
/// the resulting asset never receives.
#[derive(Debug)]
struct R007bReferenceSurfaceEffect {
    operations: Vec<Value>,
    summary: String,
    intended_difference: String,
}

fn r007b_non_root_part_for_role<'a>(
    parts: &'a [Value],
    root_part_id: &str,
    role: &str,
) -> CoreResult<&'a Value> {
    parts
        .iter()
        .find(|part| {
            part.get("part_id").and_then(Value::as_str) != Some(root_part_id)
                && part.get("role").and_then(Value::as_str) == Some(role)
        })
        .ok_or_else(|| {
            CoreError::conflict(
                "REFERENCE_REBUILD_TARGET_NOT_FOUND",
                "The active C106 asset has no reviewed non-root Design Surface target for this reference evidence.",
            )
        })
}

fn r007b_part_id(part: &Value) -> CoreResult<&str> {
    part.get("part_id").and_then(Value::as_str).ok_or_else(|| {
        CoreError::invalid_data(
            "REFERENCE_REBUILD_ASSEMBLY_INVALID",
            "The reviewed C106 Design Surface target has no stable Part identity.",
        )
    })
}

fn r007b_require_parameter_binding(part: &Value, path: &str) -> CoreResult<()> {
    let declared = part
        .get("editable_parameter_bindings")
        .and_then(Value::as_array)
        .is_some_and(|bindings| {
            bindings
                .iter()
                .any(|binding| binding.get("path").and_then(Value::as_str) == Some(path))
        });
    if !declared {
        return Err(CoreError::conflict(
            "REFERENCE_REBUILD_PARAMETER_TARGET_INVALID",
            "The reviewed C106 reference target does not declare the bounded visual-proportion parameter required by this evidence class.",
        ));
    }
    Ok(())
}

fn r007b_require_material_zone(part: &Value, zone_id: &str) -> CoreResult<()> {
    let declared = part
        .get("material_zone_ids")
        .and_then(Value::as_array)
        .is_some_and(|zones| zones.iter().any(|zone| zone.as_str() == Some(zone_id)));
    if !declared {
        return Err(CoreError::conflict(
            "REFERENCE_REBUILD_MATERIAL_TARGET_INVALID",
            "The reviewed C106 reference target does not own the required stable Material Zone.",
        ));
    }
    Ok(())
}

fn r007b_require_surface_adornment_slot(
    part: &Value,
    zone_id: &str,
    kind: &str,
    motif: &str,
    coverage: &str,
) -> CoreResult<()> {
    let declared = part
        .get("surface_adornment_slots")
        .and_then(Value::as_array)
        .is_some_and(|slots| {
            slots.iter().any(|slot| {
                slot.get("zone_id").and_then(Value::as_str) == Some(zone_id)
                    && slot
                        .get("allowed_kinds")
                        .and_then(Value::as_array)
                        .is_some_and(|values| {
                            values.iter().any(|value| value.as_str() == Some(kind))
                        })
                    && slot
                        .get("allowed_motifs")
                        .and_then(Value::as_array)
                        .is_some_and(|values| {
                            values.iter().any(|value| value.as_str() == Some(motif))
                        })
                    && slot
                        .get("allowed_coverages")
                        .and_then(Value::as_array)
                        .is_some_and(|values| {
                            values.iter().any(|value| value.as_str() == Some(coverage))
                        })
            })
        });
    if !declared {
        return Err(CoreError::conflict(
            "REFERENCE_REBUILD_SURFACE_SLOT_INVALID",
            "The reviewed C106 reference target does not expose the requested immutable A005 visual-only surface slot.",
        ));
    }
    Ok(())
}

fn r007b_adornment_program(
    base: &AgentAssetVersion,
    part: &Value,
    zone_id: &str,
    kind: &str,
    motif: &str,
    intensity: &str,
    coverage: &str,
    fact_hash: &str,
) -> CoreResult<SurfaceAdornmentProgram> {
    let target_part_id = r007b_part_id(part)?;
    r007b_require_material_zone(part, zone_id)?;
    r007b_require_surface_adornment_slot(part, zone_id, kind, motif, coverage)?;
    let skill = builtin_surface_adornment_manifest_v2();
    let skill_sha256 = skill.canonical_sha256()?;
    let seed = u32::from_str_radix(&fact_hash[..8], 16).map_err(|_| {
        CoreError::invalid_data(
            "REFERENCE_REBUILD_FACT_HASH_INVALID",
            "Sealed reference visual facts could not be converted into a bounded texture variation seed.",
        )
    })? & 0x7fff_ffff;
    Ok(SurfaceAdornmentProgram {
        schema_version: "SurfaceAdornmentProgram@1".into(),
        program_id: format!("adorn_{}", &fact_hash[..24]),
        target_part_id: target_part_id.to_string(),
        target_zone_id: zone_id.to_string(),
        kind: kind.to_string(),
        motif: motif.to_string(),
        intensity: intensity.to_string(),
        coverage: coverage.to_string(),
        seed,
        base_material: canonical_surface_adornment_material(&surface_adornment_base_material(
            base,
            target_part_id,
            zone_id,
        )?),
        execution: "texture_bake".into(),
        skill_id: skill.skill_id,
        skill_version: skill.version,
        skill_sha256,
        generator: "a005_v1".into(),
        non_functional_only: true,
    })
}

fn r007b_image_visual_style(
    facts: &forgecad_core::ReferenceImageSurfaceFacts,
) -> (&'static str, &'static str, &'static str, &'static str, f64) {
    let [left, top, right, bottom] = facts.foreground_bbox_normalized;
    let foreground_width = u32::from(right.saturating_sub(left));
    let foreground_height = u32::from(bottom.saturating_sub(top)).max(1);
    let foreground_aspect_milli = foreground_width.saturating_mul(1_000) / foreground_height;
    let has_blue_signal = facts.dominant_color_buckets.iter().any(|bucket| {
        matches!(
            bucket,
            forgecad_core::ReferenceImageColorBucket::Blue
                | forgecad_core::ReferenceImageColorBucket::Cyan
        )
    });
    let (kind, motif) = if has_blue_signal {
        ("flowline", "double_flowline")
    } else if matches!(
        facts.edge_density,
        forgecad_core::ReferenceImageEdgeDensityBucket::High
    ) {
        ("pattern", "hex_microgrid")
    } else {
        ("normal_relief", "parallel_groove")
    };
    let coverage = if facts.contact_sheet_layout_evidence {
        "symmetric_pair"
    } else if foreground_aspect_milli >= 1_250 {
        "center_band"
    } else if foreground_aspect_milli <= 750 {
        "edge_band"
    } else {
        "full_zone"
    };
    let intensity = match (facts.brightness, facts.edge_density) {
        (_, forgecad_core::ReferenceImageEdgeDensityBucket::High)
        | (forgecad_core::ReferenceImageBrightnessBucket::Dark, _) => "pronounced",
        (forgecad_core::ReferenceImageBrightnessBucket::Bright, _) => "subtle",
        _ => "balanced",
    };
    // This is deliberately a visual proportion selector, not a source size.
    // Every output is one declared 0.05 C106 step in [0.80, 1.20].
    let scale = match foreground_aspect_milli {
        0..=649 => 0.80,
        650..=824 => 0.90,
        825..=999 => 0.95,
        1_000..=1_174 => 1.00,
        1_175..=1_349 => 1.05,
        1_350..=1_524 => 1.10,
        _ => 1.20,
    };
    (kind, motif, intensity, coverage, scale)
}

fn r007b_glb_visual_style(
    facts: &forgecad_core::ImportedGlbInspection,
) -> (&'static str, &'static str, &'static str, &'static str, f64) {
    let [x, y, z] = facts.bounds_mm;
    let shortest = x.min(y).min(z).max(0.001);
    let longest = x.max(y).max(z);
    let visible_aspect = longest / shortest;
    let primitive_density = facts.triangle_count / facts.primitive_count.max(1);
    let (kind, motif) = if facts.material_count >= 3 {
        ("flowline", "double_flowline")
    } else if facts.primitive_count >= 4 {
        ("pattern", "hex_microgrid")
    } else {
        ("normal_relief", "parallel_groove")
    };
    let coverage = if facts.node_count > facts.mesh_count.saturating_mul(2) {
        "symmetric_pair"
    } else if visible_aspect >= 1.75 {
        "center_band"
    } else {
        "edge_band"
    };
    let intensity = if primitive_density >= 1_000 {
        "pronounced"
    } else if primitive_density <= 150 {
        "subtle"
    } else {
        "balanced"
    };
    let scale = if visible_aspect >= 2.5 {
        1.20
    } else if visible_aspect >= 1.9 {
        1.15
    } else if visible_aspect >= 1.45 {
        1.10
    } else if visible_aspect >= 1.1 {
        1.05
    } else {
        1.00
    };
    (kind, motif, intensity, coverage, scale)
}

/// Lowers sealed bounded evidence facts to two already-allowed operations:
/// one quantized visual-proportion binding and one A005 texture bake. It never
/// copies pixels, source mesh, hierarchy, material names, or hidden structure.
fn r007b_reference_surface_effect(
    repository: &CoreRepository,
    evidence: &forgecad_core::ReferenceEvidence,
    base: &AgentAssetVersion,
    parts: &[Value],
    root_part_id: &str,
    rebuild_plan_id: &str,
) -> CoreResult<R007bReferenceSurfaceEffect> {
    let link = r007b_non_root_part_for_role(parts, root_part_id, "link_armor")?;
    let trim = r007b_non_root_part_for_role(parts, root_part_id, "surface_trim")?;
    r007b_require_parameter_binding(link, "transform.scale.y")?;
    const TRIM_ZONE_ID: &str = "zone_arm_surface_trim";
    let (kind, motif, intensity, coverage, scale, fact_hash, summary, intended_difference) =
        match evidence.kind {
            ReferenceEvidenceKind::Image => {
                let facts = evidence.observations.image_surface_facts.as_ref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_REBUILD_IMAGE_SURFACE_FACTS_REQUIRED",
                        "R007B image evidence requires sealed bounded local surface facts before it can affect a Design Surface.",
                    )
                })?;
                let (kind, motif, intensity, coverage, scale) = r007b_image_visual_style(facts);
                let fact_hash = semantic_sha256(&json!({"image_surface_facts": facts}))?;
                (
                    kind,
                    motif,
                    intensity,
                    coverage,
                    scale,
                    fact_hash,
                    "依据已密封图像表面事实预览可见比例与表面语言".to_string(),
                    format!(
                        "依据已密封的图像前景比例、色块、亮度和边缘密度，受限地将已审核连杆视觉比例调整为 {scale:.2}，并在已审核饰条槽应用 {motif} 纹理烘焙；不复制像素、不推断背面、尺寸或内部结构。"
                    ),
                )
            }
            ReferenceEvidenceKind::Glb => {
                let facts = evidence.glb_inspection.as_ref().ok_or_else(|| {
                    CoreError::invalid_data(
                        "REFERENCE_SURFACE_GLB_READBACK_REQUIRED",
                        "R007B GLB evidence requires sealed readback facts before it can affect a Design Surface.",
                    )
                })?;
                let (kind, motif, intensity, coverage, scale) = r007b_glb_visual_style(facts);
                let fact_hash = semantic_sha256(&json!({"glb_readback_facts": facts}))?;
                (
                    kind,
                    motif,
                    intensity,
                    coverage,
                    scale,
                    fact_hash,
                    "依据已密封 GLB 回读事实预览可见比例与表面语言".to_string(),
                    format!(
                        "依据已密封的 GLB 可见包围范围、网格、primitive、材质和节点计数，受限地将已审核连杆视觉比例调整为 {scale:.2}，并在已审核饰条槽应用 {motif} 纹理烘焙；不复制源网格、节点或材质名称。"
                    ),
                )
            }
        };
    let adornment = r007b_adornment_program(
        base,
        trim,
        TRIM_ZONE_ID,
        kind,
        motif,
        intensity,
        coverage,
        &fact_hash,
    )?;
    // This is deliberately a read-only fail-closed authorization check before
    // any rebuild plan or ChangeSet is persisted. It neither enables nor
    // upgrades A005: users must explicitly do that through the Skill route.
    repository.validate_surface_adornment_program(&base.asset_version_id, &adornment)?;
    let link_part_id = r007b_part_id(link)?;
    let trim_part_id = r007b_part_id(trim)?;
    Ok(R007bReferenceSurfaceEffect {
        operations: vec![
            json!({
                "operation_id": stable_request_id("changeop", &format!("{rebuild_plan_id}:scale"))?,
                "op": "set_part_parameter",
                "part_id": link_part_id,
                "path": "transform.scale.y",
                "value": scale,
            }),
            json!({
                "operation_id": stable_request_id("changeop", &format!("{rebuild_plan_id}:adornment"))?,
                "op": "apply_surface_adornment",
                "part_id": trim_part_id,
                "material_zone_id": TRIM_ZONE_ID,
                "surface_adornment_program": adornment,
            }),
        ],
        summary,
        intended_difference,
    })
}

fn c106_recipe_ref(registry: &RecipeRegistry, recipe_id: &str) -> CoreResult<ComponentRecipeRef> {
    let recipe = registry
        .recipe(recipe_id)
        .ok_or_else(|| CoreError::not_found("C106 EditableComponentRecipe"))?;
    Ok(ComponentRecipeRef {
        schema_version: "ComponentRecipeRef@1".into(),
        recipe_id: recipe.recipe_id.clone(),
        version: recipe.version,
        recipe_sha256: RecipeValidator::recipe_sha256(recipe)?,
    })
}

fn c106_surface_binding(
    registry: &RecipeRegistry,
    root_recipe_id: &str,
    binding_id: String,
    observation_kind: ReferenceSurfaceObservationKind,
    observation_index: u32,
    target_role: &str,
) -> CoreResult<ReferenceSurfaceBinding> {
    let root = registry
        .recipe(root_recipe_id)
        .ok_or_else(|| CoreError::not_found("C106 root Recipe"))?;
    let (target_slot_id, target) = if target_role == root.component_role {
        (None, root)
    } else {
        let slot = root
            .child_slots
            .iter()
            .find(|slot| {
                slot.required && slot.accepted_roles.iter().any(|role| role == target_role)
            })
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "REFERENCE_SURFACE_TARGET_INVALID",
                    "C106 root has no reviewed target slot for this visible role.",
                )
            })?;
        let child = registry
            .recipe(&slot.child_recipe_id)
            .ok_or_else(|| CoreError::not_found("C106 child Recipe"))?;
        (Some(slot.slot_id.clone()), child)
    };
    let zone = target
        .material_zones
        .first()
        .and_then(|zone| zone.get("zone_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "REFERENCE_SURFACE_TARGET_INVALID",
                "C106 target has no material zone.",
            )
        })?;
    let surface_slot = target
        .surface_adornment_slots
        .iter()
        .find(|slot| slot.zone_id == zone)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "REFERENCE_SURFACE_TARGET_INVALID",
                "C106 target has no A005 surface slot for its material zone.",
            )
        })?;
    Ok(ReferenceSurfaceBinding {
        binding_id,
        observation_kind,
        observation_index,
        target_part_slot_id: target_slot_id,
        target_recipe: c106_recipe_ref(registry, &target.recipe_id)?,
        target_part_role: target.component_role.clone(),
        target_material_zone_id: zone.to_string(),
        target_surface_slot_id: surface_slot.slot_id.clone(),
    })
}

fn build_reference_surface_analysis(
    registry: &RecipeRegistry,
    evidence: &forgecad_core::ReferenceEvidence,
    plan: &ReferenceGuidedRebuildPlan,
    timestamp: &str,
) -> CoreResult<ReferenceSurfaceAnalysis> {
    let root_recipe_id = plan.recipe_id.as_str();
    let mut bindings = vec![
        c106_surface_binding(
            registry,
            root_recipe_id,
            "refsrfbind_silhouette".into(),
            ReferenceSurfaceObservationKind::Silhouette,
            0,
            "base_form",
        )?,
        c106_surface_binding(
            registry,
            root_recipe_id,
            "refsrfbind_proportion".into(),
            ReferenceSurfaceObservationKind::Proportion,
            0,
            "link_armor",
        )?,
        c106_surface_binding(
            registry,
            root_recipe_id,
            "refsrfbind_material".into(),
            ReferenceSurfaceObservationKind::MaterialZone,
            0,
            "surface_trim",
        )?,
    ];
    if evidence.kind == ReferenceEvidenceKind::Image {
        let visible = evidence
            .observations
            .visible_part_hypotheses
            .iter()
            .enumerate()
            .find_map(|(index, hypothesis)| {
                let role = match hypothesis.role.as_str() {
                    "base_form" => "base_form",
                    "joint_housing" => "joint_housing",
                    "upper_link_form" => "link_armor",
                    "visual_cable" => "cable_harness",
                    "end_effector_form" => "end_effector_form",
                    _ => return None,
                };
                Some((index as u32, role))
            })
            .ok_or_else(|| CoreError::invalid_data(
                "REFERENCE_SURFACE_VISIBLE_PART_UNMAPPABLE",
                "Image evidence has no reviewed C106-compatible visible part; provide a visible base, joint, link, cable or end-effector label rather than inferring one.",
            ))?;
        bindings.push(c106_surface_binding(
            registry,
            root_recipe_id,
            format!("refsrfbind_visible_{}", visible.0),
            ReferenceSurfaceObservationKind::VisiblePart,
            visible.0,
            visible.1,
        )?);
    }
    let mut unresolved = vec![
        ReferenceSurfaceUnresolved::HiddenStructure,
        ReferenceSurfaceUnresolved::ExactDimensions,
        ReferenceSurfaceUnresolved::MaterialPhysics,
        ReferenceSurfaceUnresolved::FunctionalBehavior,
    ];
    let (fidelity_ceiling, glb_readback_facts) = match evidence.kind {
        ReferenceEvidenceKind::Image if evidence.missing_views.is_empty() => (
            ReferenceSurfaceFidelityCeiling::MultiViewImageVisibleSurfaceOnly,
            None,
        ),
        ReferenceEvidenceKind::Image => {
            unresolved.insert(0, ReferenceSurfaceUnresolved::MissingViews);
            (
                ReferenceSurfaceFidelityCeiling::SingleImageVisibleSurfaceOnly,
                None,
            )
        }
        ReferenceEvidenceKind::Glb => {
            let inspection = evidence.glb_inspection.as_ref().ok_or_else(|| {
                CoreError::invalid_data(
                    "REFERENCE_SURFACE_GLB_READBACK_REQUIRED",
                    "GLB R007B analysis requires sealed readback facts.",
                )
            })?;
            (
                ReferenceSurfaceFidelityCeiling::StrictGlbReadbackVisibleBoundsOnly,
                Some(ReferenceSurfaceGlbReadbackFacts {
                    sha256: inspection.sha256.clone(),
                    triangle_count: inspection.triangle_count,
                    bounds_mm: inspection.bounds_mm,
                    mesh_count: inspection.mesh_count,
                    primitive_count: inspection.primitive_count,
                    material_count: inspection.material_count,
                    node_count: inspection.node_count,
                }),
            )
        }
    };
    let skill = builtin_surface_adornment_manifest_v2();
    let surface_skill_sha256 = skill.canonical_sha256()?;
    Ok(ReferenceSurfaceAnalysis {
        schema_version: "ReferenceSurfaceAnalysis@1".into(),
        analysis_id: stable_request_id("refsrfanalysis", &plan.rebuild_plan_id)?,
        rebuild_plan_id: plan.rebuild_plan_id.clone(),
        evidence_id: evidence.evidence_id.clone(),
        source_object_sha256: evidence.source_object_sha256.clone(),
        domain_pack_id: plan.domain_pack_id.clone(),
        target_root_recipe: c106_recipe_ref(registry, root_recipe_id)?,
        c106_registry_sha256: registry.registry_sha256().to_string(),
        surface_skill_id: skill.skill_id,
        surface_skill_version: skill.version,
        surface_skill_sha256,
        fidelity_ceiling,
        retained_observation_kinds: bindings
            .iter()
            .map(|binding| binding.observation_kind)
            .collect(),
        bindings,
        intentionally_changed: vec![
            ReferenceSurfaceIntentionalChange::NonFunctionalRecipeInterpretation,
            ReferenceSurfaceIntentionalChange::ReviewedRecipeComponentSubstitution,
            ReferenceSurfaceIntentionalChange::MaterialPresetNormalization,
            ReferenceSurfaceIntentionalChange::SurfaceAdornmentNormalization,
        ],
        unresolved,
        glb_readback_facts,
        created_at: timestamp.to_string(),
    })
}

fn change_set_payload(change_set: &AgentAssetChangeSet) -> CoreResult<Value> {
    let mut payload = serde_json::to_value(change_set)
        .map_err(json_encode_error)?
        .as_object()
        .cloned()
        .ok_or_else(|| {
            CoreError::invalid_data(
                "CHANGE_SET_INVALID",
                "Agent ChangeSet did not serialize as an object.",
            )
        })?;
    payload.insert(
        "schema_version".into(),
        Value::String("AgentAssetChangeSet@1".into()),
    );
    Ok(Value::Object(payload))
}

fn snapshot_response(
    status: u16,
    snapshot: &ActiveDesignSnapshot,
) -> CoreResult<CompatHttpResponse> {
    json_response(
        status,
        serde_json::to_value(snapshot).map_err(json_encode_error)?,
        vec![("ETag".into(), snapshot.etag().to_string())],
    )
}

fn json_response(
    status: u16,
    body: Value,
    mut headers: Vec<(String, String)>,
) -> CoreResult<CompatHttpResponse> {
    headers.insert(0, ("Content-Type".into(), "application/json".into()));
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("cache-control"))
    {
        headers.push(("Cache-Control".into(), "no-store".into()));
    }
    Ok(CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
        status,
        headers,
        body: ProtocolHttpBody::Utf8 {
            data: serde_json::to_string(&body).map_err(json_encode_error)?,
        },
    })
}

fn core_error_response(error: CoreError) -> CompatHttpResponse {
    let (status, recoverable) = match &error {
        CoreError::InvalidData { .. } => (400, false),
        CoreError::Conflict { .. } | CoreError::ConflictWithDetails { .. } => (409, true),
        CoreError::NotFound { .. } => (404, false),
        CoreError::Sqlite(_) | CoreError::Io(_) | CoreError::Migration { .. } => (503, true),
    };
    let details = match &error {
        CoreError::ConflictWithDetails { details, .. } => details.clone(),
        _ => json!({}),
    };
    json_response(
        status,
        json!({
            "error": {
                "code": error.code(),
                "message": error.to_string(),
                "recoverable": recoverable,
                "details": details
            }
        }),
        Vec::new(),
    )
    .unwrap_or_else(|_| CompatHttpResponse {
        schema_version: HTTP_COMPAT_RESPONSE_SCHEMA_VERSION.into(),
        status: 500,
        headers: vec![
            ("Content-Type".into(), "application/json".into()),
            ("Cache-Control".into(), "no-store".into()),
        ],
        body: ProtocolHttpBody::Utf8 {
            data: "{\"error\":{\"code\":\"INTERNAL_ERROR\",\"message\":\"Response encoding failed.\",\"recoverable\":false,\"details\":{}}}".into(),
        },
    })
}

fn request_json(request: &PreparedCompatHttpRequest) -> CoreResult<Value> {
    let text = match &request.body {
        ProtocolHttpBody::Utf8 { data } => data.clone(),
        ProtocolHttpBody::Base64 { data } => {
            String::from_utf8(BASE64_STANDARD.decode(data).map_err(|_| {
                CoreError::invalid_data("HTTP_BODY_INVALID", "Request body is not valid base64.")
            })?)
            .map_err(|_| {
                CoreError::invalid_data("HTTP_BODY_INVALID", "Request body is not UTF-8 JSON.")
            })?
        }
        ProtocolHttpBody::Empty => {
            return Err(CoreError::invalid_data(
                "HTTP_BODY_REQUIRED",
                "This route requires a JSON request body.",
            ))
        }
    };
    let value: Value = serde_json::from_str(&text).map_err(|_| {
        CoreError::invalid_data("HTTP_BODY_INVALID", "Request body is invalid JSON.")
    })?;
    if !value.is_object() {
        return Err(CoreError::invalid_data(
            "HTTP_BODY_INVALID",
            "Request body must be a JSON object.",
        ));
    }
    Ok(value)
}

fn query_parameters(path: &str) -> CoreResult<BTreeMap<String, String>> {
    let mut parameters = BTreeMap::new();
    let Some((_, query)) = path.split_once('?') else {
        return Ok(parameters);
    };
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_decode_query(raw_key)?;
        let value = percent_decode_query(raw_value)?;
        if parameters.insert(key, value).is_some() {
            return Err(CoreError::invalid_data(
                "HTTP_QUERY_INVALID",
                "Duplicate query parameters are not accepted by Rust product routes.",
            ));
        }
    }
    Ok(parameters)
}

fn percent_decode_query(value: &str) -> CoreResult<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(CoreError::invalid_data(
                        "HTTP_QUERY_INVALID",
                        "Query parameter contains invalid percent encoding.",
                    ));
                }
                let high = hex_nibble(bytes[index + 1])?;
                let low = hex_nibble(bytes[index + 2])?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).map_err(|_| {
        CoreError::invalid_data("HTTP_QUERY_INVALID", "Query parameter is not valid UTF-8.")
    })
}

fn hex_nibble(value: u8) -> CoreResult<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(CoreError::invalid_data(
            "HTTP_QUERY_INVALID",
            "Query parameter contains invalid percent encoding.",
        )),
    }
}

fn header<'a>(request: &'a PreparedCompatHttpRequest, wanted: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
}

fn required_header<'a>(
    request: &'a PreparedCompatHttpRequest,
    wanted: &str,
) -> CoreResult<&'a str> {
    header(request, wanted)
        .filter(|value| value.len() <= 256)
        .ok_or_else(|| {
            CoreError::invalid_data(
                if wanted.eq_ignore_ascii_case("idempotency-key") {
                    "IDEMPOTENCY_KEY_REQUIRED"
                } else {
                    "HTTP_HEADER_REQUIRED"
                },
                format!("Required HTTP header {wanted} is missing or invalid."),
            )
        })
}

fn expected_etag_header(request: &PreparedCompatHttpRequest) -> CoreResult<SnapshotEtag> {
    let raw = required_header(request, "if-match").map_err(|_| {
        CoreError::invalid_data(
            "ACTIVE_DESIGN_REVISION_REQUIRED",
            "This operation requires the current ActiveDesignSnapshot If-Match.",
        )
    })?;
    SnapshotEtag::from_str(raw)
}

fn expected_etag(request: &PreparedCompatHttpRequest, body: &Value) -> CoreResult<SnapshotEtag> {
    let body_revision = match body.get("snapshot_revision") {
        Some(Value::Null) | None => None,
        Some(value) => Some(value.as_u64().filter(|item| *item > 0).ok_or_else(|| {
            CoreError::invalid_data(
                "ACTIVE_DESIGN_REVISION_INVALID",
                "snapshot_revision must be a positive integer.",
            )
        })?),
    };
    let header_etag = header(request, "if-match")
        .map(SnapshotEtag::from_str)
        .transpose()?;
    match (body_revision, header_etag) {
        (None, None) => Err(CoreError::invalid_data(
            "ACTIVE_DESIGN_REVISION_REQUIRED",
            "snapshot_revision or If-Match is required.",
        )),
        (Some(revision), Some(etag)) if revision != etag.0 => Err(CoreError::conflict(
            "ACTIVE_DESIGN_STALE",
            "snapshot_revision and If-Match do not match.",
        )),
        (Some(revision), _) => Ok(SnapshotEtag(revision)),
        (None, Some(etag)) => Ok(etag),
    }
}

fn require_idempotency_identity<'a>(
    request: &PreparedCompatHttpRequest,
    body: &'a Value,
) -> CoreResult<&'a str> {
    let client_request_id = required_bounded_string(body, "client_request_id", 120)?;
    let header_key = header(request, "idempotency-key").ok_or_else(|| {
        CoreError::invalid_data(
            "IDEMPOTENCY_KEY_REQUIRED",
            "Idempotency-Key is required for active-design writes.",
        )
    })?;
    if header_key != client_request_id {
        return Err(CoreError::conflict(
            "IDEMPOTENCY_CONFLICT",
            "Idempotency-Key does not match client_request_id.",
        ));
    }
    Ok(client_request_id)
}

fn required_bounded_string<'a>(body: &'a Value, field: &str, max: usize) -> CoreResult<&'a str> {
    body.get(field)
        .and_then(Value::as_str)
        .filter(|value| {
            !value.trim().is_empty()
                && value.chars().count() <= max
                && !value.chars().any(char::is_control)
        })
        .ok_or_else(|| {
            CoreError::invalid_data(
                "HTTP_FIELD_INVALID",
                format!("{field} must be non-empty bounded text."),
            )
        })
}

fn optional_string<'a>(body: &'a Value, field: &str) -> CoreResult<Option<&'a str>> {
    match body.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_str().map(Some).ok_or_else(|| {
            CoreError::invalid_data(
                "HTTP_FIELD_INVALID",
                format!("{field} must be a string or null."),
            )
        }),
    }
}

fn optional_string_array(body: &Value, field: &str) -> CoreResult<Vec<String>> {
    let Some(value) = body.get(field) else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let values = value
        .as_array()
        .filter(|values| values.len() <= 256)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "HTTP_FIELD_INVALID",
                format!("{field} must be a bounded string array."),
            )
        })?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty() && value.len() <= 256)
                .map(str::to_string)
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "HTTP_FIELD_INVALID",
                        format!("{field} must contain bounded stable identities."),
                    )
                })
        })
        .collect()
}

fn query_parameter<'a>(path: &'a str, wanted: &str) -> Option<&'a str> {
    let query = path.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == wanted && !value.is_empty() && value.len() <= 256).then_some(value)
    })
}

fn stable_request_id(prefix: &str, identity: &str) -> CoreResult<String> {
    let hash = semantic_sha256(&json!({"identity": identity}))?;
    Ok(format!("{prefix}_{}", &hash[..24]))
}

fn surface_adornment_base_material(
    version: &AgentAssetVersion,
    part_id: &str,
    zone_id: &str,
) -> CoreResult<String> {
    let zones = version.part_zone_index()?;
    if !zones
        .get(part_id)
        .is_some_and(|zones| zones.iter().any(|zone| zone == zone_id))
    {
        return Err(CoreError::not_found(
            "Surface adornment Part or Material Zone",
        ));
    }
    if let Some(existing) = version
        .assembly_graph
        .get("surface_adornments")
        .and_then(Value::as_array)
        .and_then(|programs| {
            programs.iter().find(|program| {
                program.get("target_part_id").and_then(Value::as_str) == Some(part_id)
                    && program.get("target_zone_id").and_then(Value::as_str) == Some(zone_id)
            })
        })
    {
        let program: SurfaceAdornmentProgram =
            serde_json::from_value(existing.clone()).map_err(|_| {
                CoreError::invalid_data(
                    "SURFACE_ADORNMENT_PROVENANCE_INVALID",
                    "Stored surface appearance provenance is malformed.",
                )
            })?;
        program.validate()?;
        return Ok(program.base_material);
    }
    if let Some(material_id) = version
        .material_bindings
        .get(&format!("{part_id}:{zone_id}"))
        .and_then(Value::as_str)
    {
        return Ok(material_id.to_string());
    }
    let operation_id = version
        .assembly_graph
        .get("parts")
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts
                .iter()
                .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        })
        .and_then(|part| part.get("operation_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CoreError::conflict(
                "SURFACE_ADORNMENT_BASE_MATERIAL_MISSING",
                "Selected Material Zone has no editable ShapeProgram material source.",
            )
        })?;
    version
        .shape_program
        .get("operations")
        .and_then(Value::as_array)
        .and_then(|operations| {
            operations.iter().find(|operation| {
                operation.get("operation_id").and_then(Value::as_str) == Some(operation_id)
            })
        })
        .and_then(|operation| operation.get("args"))
        .and_then(|args| args.get("material_id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            CoreError::conflict(
                "SURFACE_ADORNMENT_BASE_MATERIAL_MISSING",
                "Selected Material Zone has no committed visual base material.",
            )
        })
}

/// A005's immutable texture-bake manifest intentionally uses a compact set of
/// canonical PBR bases. The broader workbench catalog may expose visual
/// aliases (for example painted steel); normalize those aliases before the
/// sealed Skill contract is checked. This changes neither the selected
/// material binding nor geometry, and unknown IDs still fail closed later.
pub(crate) fn canonical_surface_adornment_material(material_id: &str) -> String {
    match material_id {
        "mat_painted_steel" | "mat_powder_coat" => "mat_aluminum",
        "mat_abs_matte" => "mat_graphite",
        "mat_carbon_composite" => "mat_composite",
        "mat_clear_glass" => "mat_dark_glass",
        "mat_rubber_tire" => "mat_rubber",
        _ => material_id,
    }
    .to_string()
}

fn required_sha(value: Option<&Value>, label: &str) -> CoreResult<String> {
    value
        .and_then(Value::as_str)
        .filter(|sha| {
            sha.len() == 64
                && sha
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .map(str::to_string)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "COMPILE_READBACK_INVALID",
                format!("Compile readback has an invalid {label}."),
            )
        })
}

fn json_encode_error(_error: serde_json::Error) -> CoreError {
    CoreError::invalid_data(
        "JSON_ENCODING_FAILED",
        "Rust Core response could not be encoded.",
    )
}

/// Thin async adapter from the K002 app-server port to K003 `LifecycleStore`.
#[derive(Debug, Clone)]
pub struct CoreLifecyclePort {
    store: LifecycleStore,
}

impl LifecyclePersistencePort for CoreLifecyclePort {
    fn execute(
        &self,
        command: LifecyclePersistenceCommand,
        cancellation: CancellationToken,
    ) -> LifecyclePortFuture<LifecyclePersistenceResult> {
        let store = self.store.clone();
        Box::pin(async move {
            // The synchronous SQLite transaction begins only after this check.
            // Once it begins, it is intentionally atomic and is never reported
            // as cancelled after a successful commit.
            if cancellation.is_cancelled() {
                return Err(LifecyclePortError::cancelled());
            }
            store
                .execute_lifecycle(command)
                .map_err(map_lifecycle_error)
        })
    }
}

fn map_lifecycle_error(error: CoreError) -> LifecyclePortError {
    let (kind, recoverable) = match &error {
        CoreError::NotFound { .. } => (LifecyclePortErrorKind::NotFound, false),
        CoreError::Conflict { .. } | CoreError::ConflictWithDetails { .. } => {
            (LifecyclePortErrorKind::Conflict, true)
        }
        CoreError::InvalidData { .. } => (LifecyclePortErrorKind::InvalidData, false),
        CoreError::Sqlite(_) | CoreError::Io(_) | CoreError::Migration { .. } => {
            (LifecyclePortErrorKind::Unavailable, true)
        }
    };
    LifecyclePortError {
        code: error.code().to_string(),
        kind,
        message: error.to_string(),
        recoverable,
    }
}

fn now_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("unix_ms_{}", duration.as_millis())
}

fn utc_now_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_seconds = duration.as_secs() as i64;
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_date_from_unix_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z",
        duration.subsec_millis()
    )
}

fn civil_date_from_unix_days(days: i64) -> (i64, i64, i64) {
    // Proleptic Gregorian conversion for days since 1970-01-01. This is the
    // integer civil-date algorithm used by the C++ standard calendar types.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod legacy_read_http_tests;

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use forgecad_app_server::compatibility::LocalAgentEndpoint;
    use forgecad_app_server_protocol::{
        AgentThreadStatus, AgentThreadSummary, LifecyclePersistenceOperation,
        LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION,
    };
    use forgecad_core::{
        AgentAssetChangeSet, AssetStage, AssetVersionStatus, ChangeSetStatus, ComponentRecipeRef,
        QualityStatus, RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest,
        RecipeRegistry, RecipeSlotBinding, RecipeValidator, ReferenceClass, ReferenceEvidence,
        ReferenceEvidenceObservations,
    };
    use sha2::{Digest, Sha256};

    use super::*;

    static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(1);

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(label: &str) -> Self {
            let serial = NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "forgecad-rust-core-runtime-{label}-{}-{serial}",
                std::process::id()
            ));
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn command(operation: LifecyclePersistenceOperation) -> LifecyclePersistenceCommand {
        LifecyclePersistenceCommand {
            schema_version: LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION.into(),
            command_id: "cmd_runtime_create".into(),
            idempotency_key: "a".repeat(64),
            expected_revision: None,
            command: operation,
        }
    }

    fn thread() -> AgentThreadSummary {
        AgentThreadSummary {
            thread_id: "thread_runtime".into(),
            project_id: None,
            title: "Runtime restart".into(),
            status: AgentThreadStatus::Idle,
            summary: String::new(),
            provider_id: "deepseek".into(),
            created_at: "2026-07-17T00:00:00Z".into(),
            updated_at: "2026-07-17T00:00:00Z".into(),
            last_turn_id: None,
        }
    }

    fn request(
        method: AllowedHttpMethod,
        path: impl Into<String>,
        headers: Vec<(String, String)>,
        body: Value,
    ) -> PreparedCompatHttpRequest {
        PreparedCompatHttpRequest {
            endpoint: LocalAgentEndpoint::parse("http://127.0.0.1:8000").unwrap(),
            method,
            path: path.into(),
            headers,
            body: if body.is_null() {
                ProtocolHttpBody::Empty
            } else {
                ProtocolHttpBody::Utf8 {
                    data: serde_json::to_string(&body).unwrap(),
                }
            },
        }
    }

    fn handled(
        runtime: &RustCoreRuntime,
        request: &PreparedCompatHttpRequest,
    ) -> CompatHttpResponse {
        runtime
            .handle_compat_http(request)
            .expect("route must be Rust-owned")
            .unwrap()
    }

    fn response_json(response: &CompatHttpResponse) -> Value {
        match &response.body {
            ProtocolHttpBody::Utf8 { data } => serde_json::from_str(data).unwrap(),
            other => panic!("expected UTF-8 JSON, got {other:?}"),
        }
    }

    fn response_header<'a>(response: &'a CompatHttpResponse, name: &str) -> Option<&'a str> {
        response
            .headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn create_project(runtime: &RustCoreRuntime) -> String {
        let body = json!({
            "client_request_id": "runtime_project_create",
            "profile_id": "profile_weapon_concept_v1",
            "name": "Production concept"
        });
        let response = handled(
            runtime,
            &request(
                AllowedHttpMethod::Post,
                "/api/v1/projects",
                vec![("Idempotency-Key".into(), "runtime_project_create".into())],
                body,
            ),
        );
        assert!(matches!(response.status, 200 | 201));
        response_json(&response)["project_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn asset(
        project_id: &str,
        id: &str,
        parent: Option<&str>,
        no: u64,
        shell: &str,
    ) -> AgentAssetVersion {
        AgentAssetVersion {
            asset_version_id: id.into(),
            project_id: project_id.into(),
            parent_asset_version_id: parent.map(str::to_string),
            version_no: no,
            status: AssetVersionStatus::Committed,
            summary: shell.into(),
            stage: AssetStage::EditableAsset,
            plan_id: "plan_runtime".into(),
            direction_id: "direction_best".into(),
            domain_pack_id: "pack_future_weapon_prop".into(),
            artifact_id: format!("artifact_{id}"),
            parts: vec![json!({"part_id":"part_shell"})],
            shape_program: json!({"schema_version":"ShapeProgram@1","shell":shell}),
            assembly_graph: json!({
                "graph_id": format!("mg_{id}"),
                "parts": [{"part_id":"part_shell","material_zone_ids":["zone_shell"]}]
            }),
            material_bindings: BTreeMap::new(),
            created_at: format!("2026-07-17T00:00:0{no}Z"),
        }
    }

    fn test_profile_glb(profile_id: &str) -> Vec<u8> {
        let mut profile = json!({
            "schema_version": "GeometryArtifactProfile@1",
            "artifact_profile_id": profile_id,
            "radial_segments": if profile_id == "production_concept" { 64 } else { 24 },
            "capsule_hemisphere_segments": if profile_id == "production_concept" { 14 } else { 5 },
            "smooth_loft_normals": profile_id == "production_concept",
            "texture_width": if profile_id == "production_concept" { 1024 } else { 128 },
            "texture_height": if profile_id == "production_concept" { 1024 } else { 128 },
            "texture_mime_type": "image/png",
            "texture_compression": "png_deflate",
            "delivery": if profile_id == "production_concept" { "on_demand" } else { "interactive" },
            "triangle_budget_multiplier": if profile_id == "production_concept" { 6 } else { 1 },
            "max_triangle_count": if profile_id == "production_concept" { 250_000 } else { 100_000 },
        });
        let profile_sha = semantic_sha256(&profile).unwrap();
        profile["profile_sha256"] = Value::String(profile_sha);
        let dimension = if profile_id == "production_concept" {
            1024_u32
        } else {
            128_u32
        };
        let texture_version = if profile_id == "production_concept" {
            "v4"
        } else {
            "v3"
        };
        let indices = [0_u16, 1, 2, 0, 3, 1, 0, 2, 3, 1, 3, 2];
        let positions = [0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let normals = [0_f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let tangents = [
            1_f32, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0,
        ];
        let uvs = [0_f32, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let mut binary = Vec::new();
        let mut views = Vec::<Value>::new();
        let mut append_view = |payload: &[u8], target: Option<u64>| {
            let offset = binary.len();
            binary.extend_from_slice(payload);
            let index = views.len();
            let mut view = json!({
                "buffer": 0,
                "byteOffset": offset,
                "byteLength": payload.len()
            });
            if let Some(target) = target {
                view["target"] = json!(target);
            }
            views.push(view);
            while binary.len() % 4 != 0 {
                binary.push(0);
            }
            index
        };
        let index_bytes = indices
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let position_bytes = positions
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let normal_bytes = normals
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let tangent_bytes = tangents
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let uv_bytes = uvs
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let index_view = append_view(&index_bytes, Some(34963));
        let position_view = append_view(&position_bytes, Some(34962));
        let normal_view = append_view(&normal_bytes, Some(34962));
        let tangent_view = append_view(&tangent_bytes, Some(34962));
        let uv_view = append_view(&uv_bytes, Some(34962));

        let roles = [
            "base_color",
            "metallic_roughness",
            "normal",
            "occlusion",
            "emissive",
        ];
        let mut images = Vec::new();
        let mut textures = Vec::new();
        for (index, role) in roles.into_iter().enumerate() {
            let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
            png.extend_from_slice(&dimension.to_be_bytes());
            png.extend_from_slice(&dimension.to_be_bytes());
            let view = append_view(&png, None);
            let sha = format!("{:x}", Sha256::digest(&png));
            let color_space = if matches!(role, "base_color" | "emissive") {
                "srgb"
            } else {
                "linear"
            };
            images.push(json!({
                "name": format!("vtex_test_{role}_{texture_version}"),
                "bufferView": view,
                "mimeType": "image/png",
                "extras": {"forgecad_visual_texture": {
                    "texture_id": format!("vtex_test_{role}_{texture_version}"),
                    "texture_role": role,
                    "mime_type": "image/png",
                    "byte_size": png.len(),
                    "sha256": sha,
                    "color_space": color_space,
                    "width": dimension,
                    "height": dimension,
                    "source": "forgecad_builtin",
                    "license": "not_applicable",
                    "fallback": "none",
                    "visual_only": true
                }}
            }));
            textures.push(json!({
                "name": format!("vtex_test_{role}_{texture_version}"),
                "source": index
            }));
        }
        drop(append_view);
        let document = json!({
            "asset": {"version": "2.0", "generator": "ForgeCAD test"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0}],
            "meshes": [{"primitives": [{
                "attributes": {
                    "POSITION": 1,
                    "NORMAL": 2,
                    "TANGENT": 3,
                    "TEXCOORD_0": 4
                },
                "indices": 0,
                "material": 0,
                "mode": 4,
                "extras": {
                    "forgecad_feature_node_id": "op_shell",
                    "forgecad_material_zone_id": "zone_shell",
                    "forgecad_surface_ranges": [{"surface_role":"surface","first_triangle":0,"triangle_count":4}],
                    "forgecad_source_face_ids": [0,1,2,3]
                }
            }]}],
            "materials": [{
                "pbrMetallicRoughness": {
                    "baseColorFactor": [1,1,1,1],
                    "metallicFactor": 1,
                    "roughnessFactor": 1,
                    "baseColorTexture": {"index":0},
                    "metallicRoughnessTexture": {"index":1}
                },
                "normalTexture": {"index":2},
                "occlusionTexture": {"index":3},
                "emissiveTexture": {"index":4},
                "emissiveFactor": [1,1,1],
                "extras": {
                    "forgecad_visual_texture_set_id": format!("vtexset_primary_builtin_{texture_version}"),
                    "forgecad_texture_material_id": "mat_primary",
                    "forgecad_visual_only": true
                }
            }],
            "images": images,
            "textures": textures,
            "buffers": [{"byteLength": binary.len()}],
            "bufferViews": views,
            "accessors": [
                {"bufferView":index_view,"componentType":5123,"count":12,"type":"SCALAR"},
                {"bufferView":position_view,"componentType":5126,"count":4,"type":"VEC3","min":[0,0,0],"max":[1,1,1]},
                {"bufferView":normal_view,"componentType":5126,"count":4,"type":"VEC3"},
                {"bufferView":tangent_view,"componentType":5126,"count":4,"type":"VEC4"},
                {"bufferView":uv_view,"componentType":5126,"count":4,"type":"VEC2"}
            ],
            "extras": {
                "forgecad_geometry_artifact_profile": profile,
                "forgecad_feature_history": [{
                    "node_id":"op_shell",
                    "runtime_manifest_version":"ShapeProgramRuntimeManifest@1",
                    "result_sha256":"a".repeat(64)
                }]
            }
        });
        let mut json_chunk = serde_json::to_vec(&document).unwrap();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(b' ');
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let total_length = 12 + 8 + json_chunk.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4e4f534a_u32.to_le_bytes());
        glb.extend_from_slice(&json_chunk);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004e4942_u32.to_le_bytes());
        glb.extend_from_slice(&binary);
        glb
    }

    fn rewrite_test_glb(glb: &[u8], mutate: impl FnOnce(&mut Value, &mut Vec<u8>)) -> Vec<u8> {
        let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json_start = 20;
        let binary_header = json_start + json_length;
        let binary_length =
            u32::from_le_bytes(glb[binary_header..binary_header + 4].try_into().unwrap()) as usize;
        let binary_start = binary_header + 8;
        let mut document: Value =
            serde_json::from_slice(&glb[json_start..json_start + json_length]).unwrap();
        let mut binary = glb[binary_start..binary_start + binary_length].to_vec();
        mutate(&mut document, &mut binary);
        let mut json_chunk = serde_json::to_vec(&document).unwrap();
        while json_chunk.len() % 4 != 0 {
            json_chunk.push(b' ');
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let total_length = 12 + 8 + json_chunk.len() + 8 + binary.len();
        let mut rewritten = Vec::with_capacity(total_length);
        rewritten.extend_from_slice(b"glTF");
        rewritten.extend_from_slice(&2_u32.to_le_bytes());
        rewritten.extend_from_slice(&(total_length as u32).to_le_bytes());
        rewritten.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
        rewritten.extend_from_slice(&0x4e4f534a_u32.to_le_bytes());
        rewritten.extend_from_slice(&json_chunk);
        rewritten.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        rewritten.extend_from_slice(&0x004e4942_u32.to_le_bytes());
        rewritten.extend_from_slice(&binary);
        rewritten
    }

    #[test]
    fn canonical_glb_readback_rejects_stale_pbr_attributes_and_texture_bytes() {
        let production = test_profile_glb("production_concept");
        let facts = verify_forgecad_glb(&production, Some("production_concept")).unwrap();
        assert_eq!(
            facts.runtime_manifest_version,
            "ShapeProgramRuntimeManifest@1"
        );
        assert_eq!(facts.primitive_count, 1);
        assert_eq!(facts.visual_texture_set_count, 1);
        assert_eq!(facts.visual_texture_map_count, 5);

        let stale_contract = rewrite_test_glb(&production, |document, _| {
            document["materials"][0]["extras"]["forgecad_visual_texture_set_id"] =
                json!("vtexset_primary_builtin_v3");
        });
        assert_eq!(
            verify_forgecad_glb(&stale_contract, Some("production_concept"))
                .unwrap_err()
                .code(),
            "FORGECAD_TEXTURE_CONTRACT_STALE"
        );

        let missing_tangent = rewrite_test_glb(&production, |document, _| {
            document["meshes"][0]["primitives"][0]["attributes"]
                .as_object_mut()
                .unwrap()
                .remove("TANGENT");
        });
        assert_eq!(
            verify_forgecad_glb(&missing_tangent, Some("production_concept"))
                .unwrap_err()
                .code(),
            "FORGECAD_PBR_ATTRIBUTES_MISSING"
        );

        let tampered_texture = rewrite_test_glb(&production, |document, binary| {
            let view_index = document["images"][0]["bufferView"].as_u64().unwrap() as usize;
            let offset = document["bufferViews"][view_index]["byteOffset"]
                .as_u64()
                .unwrap() as usize;
            binary[offset + 20] ^= 1;
        });
        assert_eq!(
            verify_forgecad_glb(&tampered_texture, Some("production_concept"))
                .unwrap_err()
                .code(),
            "FORGECAD_TEXTURE_HASH_MISMATCH"
        );
    }

    fn seed_production_asset(
        runtime: &RustCoreRuntime,
        project_id: &str,
    ) -> (AgentAssetVersion, Vec<u8>) {
        let version = asset(project_id, "assetver_runtime_v1", None, 1, "shell-a");
        let first_snapshot = runtime.repository.commit_initial_asset(&version).unwrap();
        let glb = test_profile_glb("production_concept");
        let object = runtime
            .repository
            .attach_object_bytes(
                &ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: version.asset_version_id.clone(),
                    role: "production_glb".into(),
                },
                &glb,
                "glb",
                "2026-07-17T00:00:02Z",
            )
            .unwrap();
        let canonical = verify_forgecad_glb(&glb, Some("production_concept")).unwrap();
        let shape_sha = semantic_sha256(&version.shape_program).unwrap();
        runtime
            .repository
            .attach_quality(
                &QualityReport {
                    quality_report_id: "quality_runtime_v1".into(),
                    project_id: project_id.into(),
                    asset_version_id: version.asset_version_id.clone(),
                    report: json!({
                        "schema_version": "AgentAssetQualityReport@1",
                        "quality_report_id": "quality_runtime_v1",
                        "asset_version_id": version.asset_version_id,
                        "status": "passed",
                        "triangle_count": canonical.triangle_count,
                        "bounds_mm": canonical.bounds_mm,
                        "evidence_source": "geometry_compile_readback",
                        "compile_readback": {
                            "schema_version": "GeometryCompileReadback@2",
                            "runtime_manifest_version": canonical.runtime_manifest_version,
                            "artifact_profile": {
                                "artifact_profile_id": "production_concept",
                                "profile_sha256": canonical.artifact_profile_sha256
                            },
                            "shape_program_sha256": shape_sha,
                            "glb_sha256": object.sha256,
                            "glb_byte_size": object.byte_size,
                            "triangle_count": canonical.triangle_count,
                            "bounds_mm": canonical.bounds_mm
                        },
                        "findings": [],
                        "checked_at": "2026-07-17T00:00:03Z"
                    }),
                    status: QualityStatus::Passed,
                    created_at: "2026-07-17T00:00:03Z".into(),
                },
                first_snapshot.etag(),
            )
            .unwrap();
        (version, glb)
    }

    #[test]
    fn lifecycle_port_persists_across_published_runtime_restart() {
        let root = TestRoot::new("restart");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-a").unwrap();
        let tokio = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        tokio
            .block_on(runtime.lifecycle_port().execute(
                command(LifecyclePersistenceOperation::CreateThread { thread: thread() }),
                CancellationToken::new(),
            ))
            .unwrap();
        runtime.publish().unwrap();
        drop(runtime);

        let restarted = RustCoreRuntime::open(root.path(), "runtime-b").unwrap();
        let result = tokio
            .block_on(restarted.lifecycle_port().execute(
                LifecyclePersistenceCommand {
                    schema_version: LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION.into(),
                    command_id: "cmd_runtime_load".into(),
                    idempotency_key: "b".repeat(64),
                    expected_revision: None,
                    command: LifecyclePersistenceOperation::LoadThread {
                        thread_id: "thread_runtime".into(),
                    },
                },
                CancellationToken::new(),
            ))
            .unwrap();
        let encoded = serde_json::to_value(result).unwrap();
        assert_eq!(
            encoded.pointer("/result/thread/thread_id"),
            Some(&serde_json::json!("thread_runtime"))
        );
    }

    #[test]
    fn cancelled_lifecycle_command_never_writes() {
        let root = TestRoot::new("cancel");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-cancel").unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let tokio = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let error = tokio
            .block_on(runtime.lifecycle_port().execute(
                command(LifecyclePersistenceOperation::CreateThread { thread: thread() }),
                cancellation,
            ))
            .unwrap_err();
        assert_eq!(error.kind, LifecyclePortErrorKind::Cancelled);

        let loaded = runtime
            .lifecycle
            .execute_lifecycle(LifecyclePersistenceCommand {
                schema_version: LIFECYCLE_PERSISTENCE_COMMAND_SCHEMA_VERSION.into(),
                command_id: "cmd_runtime_absent".into(),
                idempotency_key: "c".repeat(64),
                expected_revision: None,
                command: LifecyclePersistenceOperation::LoadThread {
                    thread_id: "thread_runtime".into(),
                },
            })
            .unwrap();
        assert_eq!(
            serde_json::to_value(loaded)
                .unwrap()
                .pointer("/result/thread"),
            None
        );
    }

    #[test]
    fn project_routes_bootstrap_profile_without_creating_legacy_versions() {
        let root = TestRoot::new("projects");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-projects").unwrap();
        let project_id = create_project(&runtime);

        let detail = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                format!("/api/v1/projects/{project_id}"),
                Vec::new(),
                Value::Null,
            ),
        );
        let detail_json = response_json(&detail);
        assert_eq!(detail.status, 200);
        assert_eq!(
            detail_json.pointer("/profile/non_functional_only"),
            Some(&json!(true))
        );
        assert_eq!(
            detail_json.pointer("/current_spec/schema_version"),
            Some(&json!("WeaponConceptSpec@1"))
        );
        assert_eq!(detail_json["current_version_id"], Value::Null);
        assert_eq!(detail_json["versions"], json!([]));

        let empty_active_design = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                format!("/api/v1/projects/{project_id}/active-design"),
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(empty_active_design.status, 404);
        assert_eq!(
            response_json(&empty_active_design).pointer("/error/code"),
            Some(&json!("ACTIVE_DESIGN_NOT_FOUND"))
        );

        let list = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                "/api/v1/projects",
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(response_json(&list)["items"].as_array().unwrap().len(), 1);
        assert!(runtime
            .handle_compat_http(&request(
                AllowedHttpMethod::Get,
                "/api/v1/projects/not-a-route/extra",
                Vec::new(),
                Value::Null,
            ))
            .is_none());
    }

    #[test]
    fn snapshot_quality_glb_export_and_etag_contracts_are_rust_authoritative() {
        let root = TestRoot::new("artifact");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-artifact").unwrap();
        let project_id = create_project(&runtime);
        let (version, glb) = seed_production_asset(&runtime, &project_id);

        let version_response = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                format!("/api/v1/agent/asset-versions/{}", version.asset_version_id),
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(
            response_json(&version_response)["schema_version"],
            "AgentAssetVersion@1"
        );

        let snapshot_response = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                format!("/api/v1/projects/{project_id}/active-design"),
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(
            response_header(&snapshot_response, "etag"),
            Some("W/\"active-design-2\"")
        );
        assert_eq!(
            response_header(&snapshot_response, "cache-control"),
            Some("no-store")
        );

        let quality = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                "/api/v1/agent/quality-reports/quality_runtime_v1",
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(response_json(&quality)["triangle_count"], 4);

        let model = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                format!(
                    "/api/v1/agent/asset-versions/{}:model.glb",
                    version.asset_version_id
                ),
                Vec::new(),
                Value::Null,
            ),
        );
        let encoded = match &model.body {
            ProtocolHttpBody::Base64 { data } => data,
            other => panic!("expected base64 GLB, got {other:?}"),
        };
        assert_eq!(BASE64_STANDARD.decode(encoded).unwrap(), glb);
        assert_eq!(
            response_header(&model, "content-type"),
            Some("model/gltf-binary")
        );
        assert_eq!(
            response_header(&model, "x-forgecad-artifact-profile"),
            Some("production_concept")
        );

        let export = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!(
                    "/api/v1/agent/asset-versions/{}:export",
                    version.asset_version_id
                ),
                Vec::new(),
                Value::Null,
            ),
        );
        let export_json = response_json(&export);
        assert_eq!(export_json["schema_version"], "AgentAssetExport@2");
        assert_eq!(export_json["readback_status"], "passed");
        assert_eq!(
            BASE64_STANDARD
                .decode(export_json["glb_base64"].as_str().unwrap())
                .unwrap(),
            glb
        );

        let selected = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/projects/{project_id}/active-design:select"),
                vec![
                    ("Idempotency-Key".into(), "select_runtime".into()),
                    ("If-Match".into(), "W/\"active-design-2\"".into()),
                ],
                json!({
                    "client_request_id": "select_runtime",
                    "snapshot_revision": 2,
                    "selected_part_id": "part_shell",
                    "selected_material_zone_id": "zone_shell"
                }),
            ),
        );
        assert_eq!(
            response_header(&selected, "etag"),
            Some("W/\"active-design-3\"")
        );
        assert_eq!(response_json(&selected)["selected_part_id"], "part_shell");
        let selected_replay = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/projects/{project_id}/active-design:select"),
                vec![
                    ("Idempotency-Key".into(), "select_runtime".into()),
                    ("If-Match".into(), "W/\"active-design-2\"".into()),
                ],
                json!({
                    "client_request_id": "select_runtime",
                    "snapshot_revision": 2,
                    "selected_part_id": "part_shell",
                    "selected_material_zone_id": "zone_shell"
                }),
            ),
        );
        assert_eq!(response_json(&selected_replay), response_json(&selected));
        assert_eq!(
            response_header(&selected_replay, "etag"),
            Some("W/\"active-design-3\"")
        );

        let stale = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/projects/{project_id}/active-design:select"),
                vec![
                    ("Idempotency-Key".into(), "select_stale".into()),
                    ("If-Match".into(), "W/\"active-design-3\"".into()),
                ],
                json!({"client_request_id":"select_stale","snapshot_revision":2}),
            ),
        );
        assert_eq!(stale.status, 409);
        assert_eq!(
            response_json(&stale).pointer("/error/code"),
            Some(&json!("ACTIVE_DESIGN_STALE"))
        );
    }

    #[test]
    fn m103_material_texture_routes_are_rust_owned_and_catalog_preserving() {
        let root = TestRoot::new("material-textures");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-material-textures").unwrap();
        let registration = request(
            AllowedHttpMethod::Post,
            "/api/v1/agent/material-textures",
            vec![("Idempotency-Key".into(), "m103-runtime-register".into())],
            json!({
                "display_name":"M103 预览纹理",
                "texture_role":"base_color",
                "mime_type":"image/png",
                "payload_base64":"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNg+M/AAAADAQEAGN2NtAAAAABJRU5ErkJggg==",
                "source":"user_created",
                "license":"self_declared_original"
            }),
        );
        let created = handled(&runtime, &registration);
        assert_eq!(created.status, 201);
        let created_json = response_json(&created);
        assert_eq!(created_json["schema_version"], "MaterialTextureObject@1");
        assert_eq!(created_json["visual_only"], true);
        assert_eq!(created_json["object_exists"], true);
        assert_eq!(
            response_json(&handled(&runtime, &registration)),
            created_json
        );

        let texture_asset_id = created_json["texture_asset_id"].as_str().unwrap();
        let detail_path = format!("/api/v1/agent/material-textures/{texture_asset_id}");
        let detail = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                &detail_path,
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(detail.status, 200);
        assert_eq!(response_json(&detail), created_json);

        let missing = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                "/api/v1/agent/material-textures/asset_tex_000000000000000000000000",
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(missing.status, 404);
        assert_eq!(
            response_json(&missing).pointer("/error/code"),
            Some(&json!("RESOURCE_NOT_FOUND"))
        );

        let listed = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                "/api/v1/agent/material-textures?texture_role=base_color&source=user_created&q=M103&limit=10",
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(listed.status, 200);
        assert_eq!(response_json(&listed)["items"], json!([created_json]));

        let mut synthetic_catalog = json!([{
            "pbr":{"base_color_texture_asset_id":texture_asset_id},
            "thumbnail_asset_id":"asset_tex_000000000000000000000000"
        }]);
        runtime
            .enrich_material_catalog(&mut synthetic_catalog)
            .unwrap();
        assert_eq!(
            synthetic_catalog[0]["texture_summary"][0]["texture_asset_id"],
            texture_asset_id
        );
        assert_eq!(synthetic_catalog[0]["texture_summary"][0]["exists"], true);
        assert_eq!(
            synthetic_catalog[0]["texture_summary"][0]["source"],
            "user_created"
        );
        assert_eq!(synthetic_catalog[0]["texture_summary"][1]["exists"], false);
        assert_eq!(synthetic_catalog[0]["thumbnail_fallback"], "unavailable");

        std::fs::remove_file(
            root.path()
                .join(created_json["object_path"].as_str().unwrap()),
        )
        .unwrap();
        let unavailable = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                &detail_path,
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(unavailable.status, 200);
        assert_eq!(response_json(&unavailable)["object_exists"], false);

        let catalog = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                "/api/v1/agent/materials?domain=future_weapon_prop",
                Vec::new(),
                Value::Null,
            ),
        );
        let catalog_json = response_json(&catalog);
        assert_eq!(catalog_json.as_array().unwrap().len(), 13);
        assert!(catalog_json.as_array().unwrap().iter().all(|preset| {
            preset["thumbnail_fallback"] == "parameter"
                && preset["texture_summary"].as_array().unwrap().is_empty()
        }));
    }

    #[test]
    fn external_glb_import_preview_quality_and_export_are_rust_owned_read_only_references() {
        let root = TestRoot::new("external-glb-runtime");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-external-glb").unwrap();
        let project_id = create_project(&runtime);
        // Even an input GLB carrying ForgeCAD production extras remains an
        // untrusted external_reference and cannot inherit production status.
        let glb = test_profile_glb("production_concept");
        let import_request = request(
            AllowedHttpMethod::Post,
            "/api/v1/agent/imports:glb",
            vec![("Idempotency-Key".into(), "external-runtime-import".into())],
            json!({
                "client_request_id":"external-runtime-import",
                "project_id":project_id,
                "domain_pack_id":"pack_vehicle_concept",
                "file_name":"../outside\\vehicle-reference.glb",
                "glb_base64":BASE64_STANDARD.encode(&glb),
                "summary":"Read-only vehicle reference"
            }),
        );
        let imported = handled(&runtime, &import_request);
        assert_eq!(imported.status, 201);
        let imported_json = response_json(&imported);
        let asset_version_id = imported_json["asset_version"]["asset_version_id"]
            .as_str()
            .unwrap();
        assert_eq!(
            imported_json["asset_version"]["shape_program"]["schema_version"],
            "ExternalGLBReference@1"
        );
        assert_eq!(
            imported_json["asset_version"]["shape_program"]["editable"],
            false
        );
        assert_eq!(imported_json["inspection"]["triangle_count"], 4);
        assert_eq!(
            response_json(&handled(&runtime, &import_request)),
            imported_json
        );

        for suffix in [":preview.glb", ":model.glb"] {
            let response = handled(
                &runtime,
                &request(
                    AllowedHttpMethod::Get,
                    format!("/api/v1/agent/asset-versions/{asset_version_id}{suffix}"),
                    Vec::new(),
                    Value::Null,
                ),
            );
            assert_eq!(response.status, 200);
            assert_eq!(
                response_header(&response, "x-forgecad-artifact-profile"),
                Some("external_reference")
            );
            assert_eq!(
                response_header(&response, "x-forgecad-artifact-profile-sha256"),
                None
            );
            assert_eq!(
                response_header(&response, "x-forgecad-shape-program-sha256"),
                None
            );
            let encoded = match response.body {
                ProtocolHttpBody::Base64 { data } => data,
                other => panic!("expected external GLB bytes, got {other:?}"),
            };
            assert_eq!(BASE64_STANDARD.decode(encoded).unwrap(), glb);
        }

        let quality = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/agent/asset-versions/{asset_version_id}:quality"),
                vec![
                    ("Idempotency-Key".into(), "external-runtime-quality".into()),
                    ("If-Match".into(), "W/\"active-design-1\"".into()),
                ],
                Value::Null,
            ),
        );
        assert_eq!(quality.status, 200);
        let quality_json = response_json(&quality);
        assert_eq!(quality_json["status"], "warning");
        assert_eq!(quality_json["evidence_source"], "external_glb_inspection");
        assert!(quality_json.get("compile_readback").is_none());
        assert_eq!(
            response_json(&handled(&runtime, &import_request)),
            imported_json
        );

        let exported = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/agent/asset-versions/{asset_version_id}:export"),
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(exported.status, 200);
        let exported_json = response_json(&exported);
        assert_eq!(exported_json["artifact_profile_id"], "external_reference");
        assert_eq!(exported_json["artifact_profile_sha256"], Value::Null);
        assert_eq!(exported_json["shape_program_sha256"], Value::Null);
        assert_eq!(
            BASE64_STANDARD
                .decode(exported_json["glb_base64"].as_str().unwrap())
                .unwrap(),
            glb
        );

        let edit = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/agent/asset-versions/{asset_version_id}/change-sets"),
                vec![("Idempotency-Key".into(), "external-runtime-edit".into())],
                json!({
                    "client_request_id":"external-runtime-edit",
                    "summary":"Must remain read only",
                    "operations":[{
                        "operation_id":"op_external_edit",
                        "op":"set_part_parameter",
                        "part_id":"part_1_imported_model",
                        "path":"transform.scale.x",
                        "value":1.2
                    }]
                }),
            ),
        );
        assert_eq!(edit.status, 409);
        assert_eq!(
            response_json(&edit).pointer("/error/code"),
            Some(&json!("EXTERNAL_REFERENCE_NOT_EDITABLE"))
        );
    }

    #[test]
    fn k003_product_routes_are_idempotent_validated_and_never_require_python_state() {
        let root = TestRoot::new("product-routes");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-product-routes").unwrap();
        let project_id = create_project(&runtime);
        let (version, _) = seed_production_asset(&runtime, &project_id);

        let quality_request = request(
            AllowedHttpMethod::Post,
            format!(
                "/api/v1/agent/asset-versions/{}:quality",
                version.asset_version_id
            ),
            vec![
                ("Idempotency-Key".into(), "quality_route_once".into()),
                ("If-Match".into(), "W/\"active-design-2\"".into()),
            ],
            Value::Null,
        );
        let quality = handled(&runtime, &quality_request);
        assert_eq!(quality.status, 200);
        let quality_json = response_json(&quality);
        assert_eq!(quality_json["evidence_source"], "geometry_compile_readback");
        assert_eq!(quality_json["triangle_count"], 4);
        assert_eq!(quality_json["status"], "passed");
        assert_eq!(
            response_json(&handled(&runtime, &quality_request)),
            quality_json
        );
        let quality_key_conflict = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!(
                    "/api/v1/agent/asset-versions/{}:quality",
                    version.asset_version_id
                ),
                vec![
                    ("Idempotency-Key".into(), "quality_route_once".into()),
                    ("If-Match".into(), "W/\"active-design-3\"".into()),
                ],
                Value::Null,
            ),
        );
        assert_eq!(
            response_json(&quality_key_conflict).pointer("/error/code"),
            Some(&json!("IDEMPOTENCY_CONFLICT"))
        );

        let render_request = request(
            AllowedHttpMethod::Post,
            format!("/api/v1/projects/{project_id}/active-design:render-preset"),
            vec![
                ("Idempotency-Key".into(), "render_route_once".into()),
                ("If-Match".into(), "W/\"active-design-3\"".into()),
            ],
            json!({
                "client_request_id":"render_route_once",
                "snapshot_revision":3,
                "camera_view":"right",
                "light_preset":"soft_studio"
            }),
        );
        let render = handled(&runtime, &render_request);
        assert_eq!(
            response_header(&render, "etag"),
            Some("W/\"active-design-4\"")
        );
        assert_eq!(
            response_json(&render).pointer("/render_preset/camera_view"),
            Some(&json!("right"))
        );
        assert_eq!(
            response_json(&handled(&runtime, &render_request)),
            response_json(&render)
        );
        let render_key_conflict = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/projects/{project_id}/active-design:render-preset"),
                vec![
                    ("Idempotency-Key".into(), "render_route_once".into()),
                    ("If-Match".into(), "W/\"active-design-3\"".into()),
                ],
                json!({
                    "client_request_id":"render_route_once",
                    "snapshot_revision":3,
                    "camera_view":"front",
                    "light_preset":"soft_studio"
                }),
            ),
        );
        assert_eq!(
            response_json(&render_key_conflict).pointer("/error/code"),
            Some(&json!("IDEMPOTENCY_CONFLICT"))
        );

        let lock = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/projects/{project_id}/active-design:part-display"),
                vec![
                    ("Idempotency-Key".into(), "lock_route_once".into()),
                    ("If-Match".into(), "W/\"active-design-4\"".into()),
                ],
                json!({
                    "client_request_id":"lock_route_once",
                    "snapshot_revision":4,
                    "action":"lock",
                    "part_id":"part_shell"
                }),
            ),
        );
        assert_eq!(
            response_json(&lock).pointer("/part_display/locked_part_ids/0"),
            Some(&json!("part_shell"))
        );

        let locked_change = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!(
                    "/api/v1/agent/asset-versions/{}/change-sets",
                    version.asset_version_id
                ),
                vec![("Idempotency-Key".into(), "locked_change".into())],
                json!({
                    "client_request_id":"locked_change",
                    "summary":"Change locked shell material",
                    "operations":[{
                        "operation_id":"op_locked_material",
                        "op":"apply_material_preset",
                        "part_id":"part_shell",
                        "material_id":"mat_graphite",
                        "material_zone_id":"zone_shell"
                    }]
                }),
            ),
        );
        assert_eq!(locked_change.status, 409);
        assert_eq!(
            response_json(&locked_change).pointer("/error/code"),
            Some(&json!("PART_PROTECTED"))
        );

        let unlock = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/projects/{project_id}/active-design:part-display"),
                vec![
                    ("Idempotency-Key".into(), "unlock_route_once".into()),
                    ("If-Match".into(), "W/\"active-design-5\"".into()),
                ],
                json!({
                    "client_request_id":"unlock_route_once",
                    "snapshot_revision":5,
                    "action":"unlock",
                    "part_id":"part_shell"
                }),
            ),
        );
        assert_eq!(response_json(&unlock)["revision"], 6);

        let missing_zone = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!(
                    "/api/v1/agent/asset-versions/{}/change-sets",
                    version.asset_version_id
                ),
                vec![("Idempotency-Key".into(), "missing_material_zone".into())],
                json!({
                    "client_request_id":"missing_material_zone",
                    "summary":"Invalid material zone",
                    "operations":[{
                        "operation_id":"op_missing_zone",
                        "op":"apply_material_preset",
                        "part_id":"part_shell",
                        "material_id":"mat_graphite",
                        "material_zone_id":"zone_missing"
                    }]
                }),
            ),
        );
        assert_eq!(missing_zone.status, 404);
        assert_eq!(
            response_json(&missing_zone).pointer("/error/code"),
            Some(&json!("RESOURCE_NOT_FOUND"))
        );

        let incompatible = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!(
                    "/api/v1/agent/asset-versions/{}/change-sets",
                    version.asset_version_id
                ),
                vec![("Idempotency-Key".into(), "incompatible_material".into())],
                json!({
                    "client_request_id":"incompatible_material",
                    "summary":"Invalid domain material",
                    "operations":[{
                        "operation_id":"op_incompatible_material",
                        "op":"apply_material_preset",
                        "part_id":"part_shell",
                        "material_id":"mat_rubber_tire",
                        "material_zone_id":"zone_shell"
                    }]
                }),
            ),
        );
        assert_eq!(incompatible.status, 400);
        assert_eq!(
            response_json(&incompatible).pointer("/error/code"),
            Some(&json!("MATERIAL_DOMAIN_INCOMPATIBLE"))
        );

        let change_request = request(
            AllowedHttpMethod::Post,
            format!(
                "/api/v1/agent/asset-versions/{}/change-sets",
                version.asset_version_id
            ),
            vec![("Idempotency-Key".into(), "valid_material_change".into())],
            json!({
                "client_request_id":"valid_material_change",
                "summary":"Refine shell material",
                "operations":[{
                    "operation_id":"op_valid_material",
                    "op":"apply_material_preset",
                    "part_id":"part_shell",
                    "material_id":"mat_graphite",
                    "material_zone_id":"zone_shell"
                }]
            }),
        );
        let proposed = handled(&runtime, &change_request);
        assert_eq!(proposed.status, 201);
        let proposed_json = response_json(&proposed);
        assert_eq!(proposed_json["status"], "proposed");
        assert_eq!(
            response_json(&handled(&runtime, &change_request)),
            proposed_json
        );
        let change_set_id = proposed_json["change_set_id"].as_str().unwrap();

        let preview_required = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/agent/change-sets/{change_set_id}:preview"),
                vec![("Idempotency-Key".into(), "preview_route_once".into())],
                Value::Null,
            ),
        );
        assert_eq!(preview_required.status, 409);
        let preview_error = response_json(&preview_required);
        assert_eq!(
            preview_error.pointer("/error/code"),
            Some(&json!("RUST_GEOMETRY_PREVIEW_REQUIRED"))
        );
        assert_eq!(
            preview_error.pointer("/error/details/base_asset_version_id"),
            Some(&json!(version.asset_version_id))
        );
        assert_eq!(
            preview_error.pointer("/error/details/snapshot_revision"),
            Some(&json!(6))
        );

        let rejected_request = request(
            AllowedHttpMethod::Post,
            format!("/api/v1/agent/change-sets/{change_set_id}:reject"),
            vec![("Idempotency-Key".into(), "reject_route_once".into())],
            Value::Null,
        );
        let rejected = handled(&runtime, &rejected_request);
        assert_eq!(response_json(&rejected)["status"], "rejected");
        assert_eq!(
            response_json(&handled(&runtime, &rejected_request)),
            response_json(&rejected)
        );

        let components = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                format!("/api/v1/agent/components?project_id={project_id}"),
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(response_json(&components), json!([]));

        let missing_preview = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                format!(
                    "/api/v1/agent/asset-versions/{}:preview.glb",
                    version.asset_version_id
                ),
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(missing_preview.status, 409);
        assert_eq!(
            response_json(&missing_preview).pointer("/error/code"),
            Some(&json!("RUST_GEOMETRY_PREVIEW_REQUIRED"))
        );
        let interactive_glb = test_profile_glb("interactive_preview");
        runtime
            .repository
            .attach_object_bytes(
                &ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: version.asset_version_id.clone(),
                    role: "interactive_preview_glb".into(),
                },
                &interactive_glb,
                "glb",
                "2026-07-17T00:00:09Z",
            )
            .unwrap();
        let preview_glb = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                format!(
                    "/api/v1/agent/asset-versions/{}:preview.glb",
                    version.asset_version_id
                ),
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(preview_glb.status, 200);
        assert_eq!(
            response_header(&preview_glb, "x-forgecad-artifact-profile"),
            Some("interactive_preview")
        );

        drop(runtime);
        let restarted =
            RustCoreRuntime::open(root.path(), "runtime-product-routes-restart").unwrap();
        let render_replay = handled(&restarted, &render_request);
        assert_eq!(
            response_header(&render_replay, "etag"),
            Some("W/\"active-design-4\"")
        );
    }

    #[test]
    fn undo_and_redo_create_new_immutable_versions_and_replay_by_request_id() {
        let root = TestRoot::new("navigation");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-navigation").unwrap();
        let project_id = create_project(&runtime);
        let original = asset(&project_id, "assetver_nav_v1", None, 1, "shell-a");
        let snapshot = runtime.repository.commit_initial_asset(&original).unwrap();
        runtime
            .repository
            .create_change_set(&AgentAssetChangeSet {
                change_set_id: "assetcs_runtime".into(),
                project_id: project_id.clone(),
                base_asset_version_id: original.asset_version_id.clone(),
                summary: "Refine shell".into(),
                operations: vec![json!({
                    "operation_id":"op_material_runtime",
                    "op":"apply_material_preset",
                    "part_id":"part_shell",
                    "material_id":"mat_graphite",
                    "material_zone_id":"zone_shell"
                })],
                protected_part_ids: Vec::new(),
                preview: None,
                status: ChangeSetStatus::Proposed,
                resulting_asset_version_id: None,
                created_at: "2026-07-17T00:00:02Z".into(),
                updated_at: "2026-07-17T00:00:02Z".into(),
            })
            .unwrap();
        let preview = asset(
            &project_id,
            "assetver_nav_preview",
            Some(&original.asset_version_id),
            2,
            "shell-b",
        );
        let (_, preview_snapshot) = runtime
            .repository
            .preview_change_set(
                "assetcs_runtime",
                &preview,
                snapshot.etag(),
                "2026-07-17T00:00:03Z",
            )
            .unwrap();
        let mut refined = preview.clone();
        refined.asset_version_id = "assetver_nav_v2".into();
        refined.created_at = "2026-07-17T00:00:04Z".into();
        let (_, _, current) = runtime
            .repository
            .confirm_change_set("assetcs_runtime", &refined, preview_snapshot.etag())
            .unwrap();
        for asset_version_id in [&original.asset_version_id, &refined.asset_version_id] {
            for (role, profile) in [
                ("interactive_preview_glb", "interactive_preview"),
                ("production_glb", "production_concept"),
            ] {
                runtime
                    .repository
                    .attach_object_bytes(
                        &ObjectReference {
                            reference_kind: "asset_version".into(),
                            owner_id: asset_version_id.clone(),
                            role: role.into(),
                        },
                        &test_profile_glb(profile),
                        "glb",
                        "2026-07-17T00:00:05Z",
                    )
                    .unwrap();
            }
        }

        let availability = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                format!("/api/v1/projects/{project_id}/active-design:navigation"),
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(response_json(&availability)["can_undo"], true);

        let undo_request = request(
            AllowedHttpMethod::Post,
            format!("/api/v1/projects/{project_id}/active-design:undo"),
            vec![
                ("Idempotency-Key".into(), "nav_undo_runtime".into()),
                ("If-Match".into(), current.etag().to_string()),
            ],
            json!({
                "client_request_id":"nav_undo_runtime",
                "snapshot_revision":current.revision
            }),
        );
        let undo = handled(&runtime, &undo_request);
        let undo_json = response_json(&undo);
        assert_eq!(undo_json["revision"], current.revision + 1);
        let undo_id = undo_json
            .pointer("/active_design/asset_version_id")
            .and_then(Value::as_str)
            .unwrap()
            .to_string();
        assert_ne!(undo_id, original.asset_version_id);
        assert_eq!(
            runtime
                .repository
                .version(&undo_id)
                .unwrap()
                .unwrap()
                .shape_program,
            original.shape_program
        );

        let replay = handled(&runtime, &undo_request);
        assert_eq!(response_json(&replay)["revision"], undo_json["revision"]);

        let redo = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/projects/{project_id}/active-design:redo"),
                vec![
                    ("Idempotency-Key".into(), "nav_redo_runtime".into()),
                    (
                        "If-Match".into(),
                        format!("W/\"active-design-{}\"", current.revision + 1),
                    ),
                ],
                json!({
                    "client_request_id":"nav_redo_runtime",
                    "snapshot_revision":current.revision + 1
                }),
            ),
        );
        let redo_id = response_json(&redo)
            .pointer("/active_design/asset_version_id")
            .and_then(Value::as_str)
            .unwrap()
            .to_string();
        assert_eq!(
            runtime
                .repository
                .version(&redo_id)
                .unwrap()
                .unwrap()
                .shape_program,
            refined.shape_program
        );
    }

    #[test]
    fn convert_legacy_post_is_rust_owned_and_rejects_an_agent_snapshot() {
        let root = TestRoot::new("convert-legacy-route");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-convert-legacy").unwrap();
        let project_id = create_project(&runtime);
        let snapshot = runtime
            .repository
            .commit_initial_asset(&asset(
                &project_id,
                "assetver_convert_legacy_route",
                None,
                1,
                "agent-shell",
            ))
            .unwrap();
        let response = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/projects/{project_id}/active-design:convert-legacy"),
                vec![
                    ("Idempotency-Key".into(), "convert_legacy_route".into()),
                    ("If-Match".into(), snapshot.etag().to_string()),
                ],
                json!({
                    "client_request_id":"convert_legacy_route",
                    "snapshot_revision":snapshot.revision,
                }),
            ),
        );
        assert_eq!(response.status, 409);
        assert_eq!(
            response_json(&response).pointer("/error/code"),
            Some(&json!("ACTIVE_DESIGN_NOT_LEGACY"))
        );
        assert_eq!(
            response_header(&response, "cache-control"),
            Some("no-store")
        );
        assert!(runtime
            .repository
            .legacy_conversion_intent(&project_id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn semantic_proportion_get_is_rust_owned_and_never_falls_back_to_python() {
        let root = TestRoot::new("semantic-proportion-route");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-semantic-route").unwrap();
        let response = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Get,
                "/api/v1/agent/asset-versions/assetver_missing/parts/part_missing/semantic-proportions",
                Vec::new(),
                Value::Null,
            ),
        );
        assert_eq!(response.status, 404);
        assert_eq!(
            response_json(&response).pointer("/error/code"),
            Some(&json!("RESOURCE_NOT_FOUND"))
        );
        assert_eq!(
            response_header(&response, "cache-control"),
            Some("no-store")
        );
    }

    #[test]
    fn active_recipe_http_accepts_only_fixed_optional_slots_without_writing_state() {
        let root = TestRoot::new("active-recipe-optional-slot");
        let runtime =
            RustCoreRuntime::open(root.path(), "runtime-active-recipe-optional-slot").unwrap();
        let project_id = create_project(&runtime);
        let registry = RecipeRegistry::from_embedded().unwrap();
        let root_recipe = registry.recipe("recipe_robotic_arm_link").unwrap();
        let root_ref = ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: root_recipe.recipe_id.clone(),
            version: root_recipe.version,
            recipe_sha256: RecipeValidator::recipe_sha256(root_recipe).unwrap(),
        };
        let initial = RecipeExpander::expand(
            &registry,
            &RecipeInstantiationRequest {
                schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                context_mode: "initial_candidate".into(),
                request_id: "recipereq_runtime_arm_initial".into(),
                project_id: None,
                base_asset_version_id: None,
                snapshot_revision: None,
                domain_pack_id: "pack_robotic_arm_concept".into(),
                recipe_registry_sha256: registry.registry_sha256().into(),
                recipe: root_ref.clone(),
                target_part_id: None,
                slot_bindings: Vec::new(),
                parameter_values: Vec::new(),
                material_zone_overrides: Vec::new(),
            },
            &RecipeExpansionPolicy::default(),
        )
        .unwrap();
        let root_part_id = initial.expanded_assembly_graph["root_part_id"]
            .as_str()
            .unwrap()
            .to_string();
        let initial_asset = AgentAssetVersion {
            asset_version_id: "assetver_runtime_arm_v1".into(),
            project_id: project_id.clone(),
            parent_asset_version_id: None,
            version_no: 1,
            status: AssetVersionStatus::Committed,
            summary: "Robotic arm recipe baseline".into(),
            stage: AssetStage::EditableAsset,
            plan_id: "plan_runtime_arm".into(),
            direction_id: "direction_runtime_arm".into(),
            domain_pack_id: "pack_robotic_arm_concept".into(),
            artifact_id: "artifact_runtime_arm".into(),
            parts: initial.expanded_assembly_graph["parts"]
                .as_array()
                .unwrap()
                .clone(),
            shape_program: initial.expanded_shape_program,
            assembly_graph: initial.expanded_assembly_graph,
            material_bindings: BTreeMap::new(),
            created_at: "2026-07-18T08:10:00Z".into(),
        };
        let snapshot = runtime
            .repository
            .commit_initial_asset(&initial_asset)
            .unwrap();
        let child = registry.recipe("recipe_robotic_arm_detail").unwrap();
        let binding = RecipeSlotBinding {
            slot_id: "slot_arm_detail".into(),
            child_recipe: ComponentRecipeRef {
                schema_version: "ComponentRecipeRef@1".into(),
                recipe_id: child.recipe_id.clone(),
                version: child.version,
                recipe_sha256: RecipeValidator::recipe_sha256(child).unwrap(),
            },
        };
        let body = json!({
            "schema_version":"ComponentRecipeActiveCandidateRequest@1",
            "recipe_request_id":"recipereq_runtime_arm_optional_detail",
            "component_recipe_ref":root_ref,
            "slot_bindings":[binding],
            "parameter_values":[],
            "material_zone_overrides":[]
        });
        let response = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!(
                    "/api/v1/agent/asset-versions/{}/parts/{root_part_id}/component-recipes:expand",
                    initial_asset.asset_version_id
                ),
                Vec::new(),
                body.clone(),
            ),
        );
        assert_eq!(response.status, 200);
        let candidate = response_json(&response);
        assert_eq!(
            candidate["component_recipe_instances"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(candidate["component_recipe_instances"]
            .as_array()
            .unwrap()
            .iter()
            .any(|instance| instance["parent_slot_id"] == "slot_arm_detail"));
        assert_eq!(
            runtime.repository.snapshot(&project_id).unwrap().unwrap(),
            snapshot
        );
        assert_eq!(
            runtime
                .repository
                .version(&initial_asset.asset_version_id)
                .unwrap()
                .unwrap(),
            initial_asset
        );

        let mut rejected_parameter = body.clone();
        rejected_parameter["parameter_values"] = json!([{
            "parameter_id":"editparam_arm_link_profile_height",
            "value":1.1
        }]);
        let rejected = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!(
                    "/api/v1/agent/asset-versions/{}/parts/{root_part_id}/component-recipes:expand",
                    initial_asset.asset_version_id
                ),
                Vec::new(),
                rejected_parameter,
            ),
        );
        assert_eq!(rejected.status, 400);
        assert_eq!(
            response_json(&rejected).pointer("/error/code"),
            Some(&json!("COMPONENT_RECIPE_REQUEST_INVALID"))
        );

        let mut duplicate = body;
        let bound = duplicate["slot_bindings"][0].clone();
        duplicate["slot_bindings"] = json!([bound.clone(), bound]);
        let duplicate = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!(
                    "/api/v1/agent/asset-versions/{}/parts/{root_part_id}/component-recipes:expand",
                    initial_asset.asset_version_id
                ),
                Vec::new(),
                duplicate,
            ),
        );
        assert_eq!(duplicate.status, 400);
        assert_eq!(
            response_json(&duplicate).pointer("/error/code"),
            Some(&json!("COMPONENT_RECIPE_SLOT_BINDING_DUPLICATE"))
        );
        assert_eq!(
            runtime.repository.snapshot(&project_id).unwrap().unwrap(),
            snapshot
        );
    }

    #[test]
    fn r007b_preview_rejects_non_c106_base_before_creating_a_plan_or_changeset() {
        let root = TestRoot::new("r007-http-rebuild");
        let runtime = RustCoreRuntime::open(root.path(), "runtime-r007-http-rebuild").unwrap();
        let project_id = create_project(&runtime);
        let registry = RecipeRegistry::from_embedded().unwrap();
        let root_recipe = registry.recipe("recipe_robotic_arm_link").unwrap();
        let child_recipe = registry.recipe("recipe_robotic_arm_detail").unwrap();
        let component_ref = |recipe: &forgecad_core::EditableComponentRecipe| ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: recipe.recipe_id.clone(),
            version: recipe.version,
            recipe_sha256: RecipeValidator::recipe_sha256(recipe).unwrap(),
        };
        let expanded = RecipeExpander::expand(
            &registry,
            &RecipeInstantiationRequest {
                schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                context_mode: "initial_candidate".into(),
                request_id: "recipereq_r007_http_base".into(),
                project_id: None,
                base_asset_version_id: None,
                snapshot_revision: None,
                domain_pack_id: "pack_robotic_arm_concept".into(),
                recipe_registry_sha256: registry.registry_sha256().into(),
                recipe: component_ref(root_recipe),
                target_part_id: None,
                slot_bindings: vec![RecipeSlotBinding {
                    slot_id: "slot_arm_detail".into(),
                    child_recipe: component_ref(child_recipe),
                }],
                parameter_values: Vec::new(),
                material_zone_overrides: Vec::new(),
            },
            &RecipeExpansionPolicy::default(),
        )
        .unwrap();
        let base = AgentAssetVersion {
            asset_version_id: "assetver_r007_http_v1".into(),
            project_id: project_id.clone(),
            parent_asset_version_id: None,
            version_no: 1,
            status: AssetVersionStatus::Committed,
            summary: "R007 robotic arm baseline".into(),
            stage: AssetStage::EditableAsset,
            plan_id: "plan_r007_http".into(),
            direction_id: "direction_r007_http".into(),
            domain_pack_id: "pack_robotic_arm_concept".into(),
            artifact_id: "artifact_r007_http".into(),
            parts: expanded.expanded_assembly_graph["parts"]
                .as_array()
                .unwrap()
                .clone(),
            shape_program: expanded.expanded_shape_program,
            assembly_graph: expanded.expanded_assembly_graph,
            material_bindings: BTreeMap::new(),
            created_at: "2026-07-18T12:00:00Z".into(),
        };
        let snapshot = runtime.repository.commit_initial_asset(&base).unwrap();
        let evidence_key = "reference_evidence_http_1";
        let evidence_response = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                "/api/v1/agent/reference-evidence:create",
                vec![("Idempotency-Key".into(), evidence_key.into())],
                json!({
                    "schema_version":"ReferenceEvidenceCreateRequest@1",
                    "client_request_id":evidence_key,
                    "project_id":project_id,
                    "kind":"image",
                    "reference_class":"single_image",
                    "file_name":"robot-arm.png",
                    "media_type":"image/png",
                    "content_base64":"iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGPQjVnwHx9mGBkKANXiigEwD3bkAAAAAElFTkSuQmCC",
                    "source_statement":"User supplied this image as visual reference.",
                    "license_statement":"User declares local reference rights.",
                    "missing_views":["rear","top"],
                    "user_notes":"Blue joint and cable are visible.",
                    "domain_pack_id":"pack_robotic_arm_concept"
                }),
            ),
        );
        assert_eq!(
            evidence_response.status,
            201,
            "unexpected R007 evidence response: {:?}",
            response_json(&evidence_response)
        );
        let evidence_id = response_json(&evidence_response)["reference_evidence"]["evidence_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(
            runtime.repository.head(&project_id).unwrap(),
            Some(base.asset_version_id.clone())
        );
        assert_eq!(
            runtime.repository.snapshot(&project_id).unwrap(),
            Some(snapshot.clone())
        );

        let rebuild_key = "reference_rebuild_http_1";
        let proposed = handled(
            &runtime,
            &request(
                AllowedHttpMethod::Post,
                format!("/api/v1/agent/projects/{project_id}/reference-guided-rebuild:preview"),
                vec![("Idempotency-Key".into(), rebuild_key.into())],
                json!({
                    "schema_version":"ReferenceGuidedRebuildPreviewRequest@1",
                    "client_request_id":rebuild_key,
                    "evidence_id":evidence_id,
                    "domain_pack_id":"pack_robotic_arm_concept",
                    "base_asset_version_id":base.asset_version_id,
                }),
            ),
        );
        assert_eq!(proposed.status, 409, "{:?}", response_json(&proposed));
        let proposed_json = response_json(&proposed);
        assert_eq!(
            proposed_json.pointer("/error/code"),
            Some(&json!("REFERENCE_REBUILD_C106_BASE_REQUIRED"))
        );
        let rejected_change_set_id = stable_request_id(
            "changeset",
            &format!("{}:{}", base.asset_version_id, rebuild_key),
        )
        .unwrap();
        let rejected_plan_id =
            reference_rebuild_plan_id_for_change_set(&rejected_change_set_id).unwrap();
        assert!(runtime
            .repository
            .reference_guided_rebuild_plan(&rejected_plan_id)
            .unwrap()
            .is_none());
        assert!(runtime
            .repository
            .change_set(&rejected_change_set_id)
            .unwrap()
            .is_none());
        assert_eq!(
            runtime.repository.snapshot(&project_id).unwrap(),
            Some(snapshot)
        );
    }

    #[test]
    fn r007b_c106_reference_preview_returns_one_analyzed_nonroot_changeset() {
        // R007B must not happen to work only for the desktop-arm fixture.
        // Each reviewed C106 root has the same target contract: a real,
        // non-root, translation-only turntable with no external consumer of
        // its ShapeProgram root operation.
        for root_recipe_id in [
            "recipe_c106_arm_desktop_assistant",
            "recipe_c106_arm_gallery_industrial",
            "recipe_c106_arm_service_display",
        ] {
            let root = TestRoot::new(&format!("r007b-c106-reference-preview-{root_recipe_id}"));
            let runtime = RustCoreRuntime::open(root.path(), "runtime-r007b-c106").unwrap();
            let project_id = create_project(&runtime);
            let registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
            let root_recipe = registry.recipe(root_recipe_id).unwrap();
            let root_ref = ComponentRecipeRef {
                schema_version: "ComponentRecipeRef@1".into(),
                recipe_id: root_recipe.recipe_id.clone(),
                version: root_recipe.version,
                recipe_sha256: RecipeValidator::recipe_sha256(root_recipe).unwrap(),
            };
            let expanded = RecipeExpander::expand(
                &registry,
                &RecipeInstantiationRequest {
                    schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                    context_mode: "initial_candidate".into(),
                    request_id: "recipereq_r007b_c106_base".into(),
                    project_id: None,
                    base_asset_version_id: None,
                    snapshot_revision: None,
                    domain_pack_id: "pack_robotic_arm_concept".into(),
                    recipe_registry_sha256: registry.registry_sha256().into(),
                    recipe: root_ref,
                    target_part_id: None,
                    slot_bindings: Vec::new(),
                    parameter_values: Vec::new(),
                    material_zone_overrides: Vec::new(),
                },
                &RecipeExpansionPolicy::default(),
            )
            .unwrap();
            let base = AgentAssetVersion {
                asset_version_id: "assetver_r007b_c106_v1".into(),
                project_id: project_id.clone(),
                parent_asset_version_id: None,
                version_no: 1,
                status: AssetVersionStatus::Committed,
                summary: "C106 production-arm baseline".into(),
                stage: AssetStage::EditableAsset,
                plan_id: "plan_r007b_c106".into(),
                direction_id: "direction_r007b_c106".into(),
                domain_pack_id: "pack_robotic_arm_concept".into(),
                artifact_id: "artifact_r007b_c106".into(),
                parts: expanded.expanded_assembly_graph["parts"]
                    .as_array()
                    .unwrap()
                    .clone(),
                shape_program: expanded.expanded_shape_program,
                assembly_graph: expanded.expanded_assembly_graph,
                material_bindings: BTreeMap::new(),
                created_at: "2026-07-18T12:30:00Z".into(),
            };
            let snapshot = runtime.repository.commit_initial_asset(&base).unwrap();
            let evidence_key = "reference_evidence_r007b_c106";
            let evidence_response = handled(
                &runtime,
                &request(
                    AllowedHttpMethod::Post,
                    "/api/v1/agent/reference-evidence:create",
                    vec![("Idempotency-Key".into(), evidence_key.into())],
                    json!({
                        "schema_version":"ReferenceEvidenceCreateRequest@1",
                        "client_request_id":evidence_key,
                        "project_id":project_id,
                        "kind":"image",
                        "file_name":"c106-arm.png",
                        "media_type":"image/png",
                        "content_base64":"iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGPQjVnwHx9mGBkKANXiigEwD3bkAAAAAElFTkSuQmCC",
                        "source_statement":"User supplied this visual reference.",
                        "license_statement":"User declares local reference rights.",
                        "missing_views":["rear"],
                        "user_notes":"visible link shell and blue trim",
                        "domain_pack_id":"pack_robotic_arm_concept"
                    }),
                ),
            );
            assert_eq!(
                evidence_response.status,
                201,
                "{:?}",
                response_json(&evidence_response)
            );
            let evidence_id = response_json(&evidence_response)["reference_evidence"]
                ["evidence_id"]
                .as_str()
                .unwrap()
                .to_string();
            let rebuild_key = "reference_rebuild_r007b_c106";
            let disabled = handled(
                &runtime,
                &request(
                    AllowedHttpMethod::Post,
                    format!("/api/v1/agent/projects/{project_id}/reference-guided-rebuild:preview"),
                    vec![("Idempotency-Key".into(), rebuild_key.into())],
                    json!({
                        "schema_version":"ReferenceGuidedRebuildPreviewRequest@1",
                        "client_request_id":rebuild_key,
                        "evidence_id":evidence_id,
                        "domain_pack_id":"pack_robotic_arm_concept",
                        "base_asset_version_id":base.asset_version_id,
                    }),
                ),
            );
            assert_eq!(disabled.status, 409, "{:?}", response_json(&disabled));
            assert_eq!(
                response_json(&disabled).pointer("/error/code"),
                Some(&json!("SURFACE_ADORNMENT_SKILL_DISABLED"))
            );
            assert!(runtime
                .repository
                .reference_guided_rebuild_plan(
                    &reference_rebuild_plan_id_for_change_set(
                        &stable_request_id(
                            "changeset",
                            &format!("{}:{}", base.asset_version_id, rebuild_key),
                        )
                        .unwrap(),
                    )
                    .unwrap()
                )
                .unwrap()
                .is_none());
            let skill_enable_key = "enable_surface_adornment_r007b_c106";
            let skill_enabled = handled(
                &runtime,
                &request(
                    AllowedHttpMethod::Post,
                    "/api/v1/agent/skills/surface-adornment:enable",
                    vec![("Idempotency-Key".into(), skill_enable_key.into())],
                    json!({
                        "schema_version":"EnableSurfaceAdornmentSkillRequest@1",
                        "client_request_id":skill_enable_key,
                        "confirm_enable":true
                    }),
                ),
            );
            assert_eq!(
                skill_enabled.status,
                200,
                "{:?}",
                response_json(&skill_enabled)
            );
            assert_eq!(
                response_json(&skill_enabled)["activation"]["skill_version"],
                2
            );
            let proposed = handled(
                &runtime,
                &request(
                    AllowedHttpMethod::Post,
                    format!("/api/v1/agent/projects/{project_id}/reference-guided-rebuild:preview"),
                    vec![("Idempotency-Key".into(), rebuild_key.into())],
                    json!({
                        "schema_version":"ReferenceGuidedRebuildPreviewRequest@1",
                        "client_request_id":rebuild_key,
                        "evidence_id":evidence_id,
                        "domain_pack_id":"pack_robotic_arm_concept",
                        "base_asset_version_id":base.asset_version_id,
                    }),
                ),
            );
            assert_eq!(proposed.status, 201, "{:?}", response_json(&proposed));
            let payload = response_json(&proposed);
            assert_eq!(payload["operations"].as_array().unwrap().len(), 2);
            assert_eq!(payload["operations"][0]["op"], "set_part_parameter");
            assert_eq!(payload["operations"][0]["path"], "transform.scale.y");
            assert!(payload["operations"][0]["value"].as_f64().unwrap() >= 0.8);
            assert!(payload["operations"][0]["value"].as_f64().unwrap() <= 1.2);
            let target_part_id = payload["operations"][0]["part_id"].as_str().unwrap();
            let target_part = base
                .parts
                .iter()
                .find(|part| part["part_id"].as_str() == Some(target_part_id))
                .unwrap();
            assert_eq!(target_part["role"], "link_armor");
            assert!(
                target_part["editable_parameter_bindings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|binding| binding["path"] == "transform.scale.y"),
                "{root_recipe_id} must retain the reviewed link-proportion binding"
            );
            assert_eq!(payload["operations"][1]["op"], "apply_surface_adornment");
            assert_eq!(
                payload["operations"][1]["surface_adornment_program"]["skill_version"],
                2
            );
            assert_eq!(
                payload["operations"][1]["surface_adornment_program"]["target_zone_id"],
                "zone_arm_surface_trim"
            );
            assert_eq!(
                payload["reference_guided_rebuild_plan"]["recipe_id"],
                root_recipe_id
            );
            assert_eq!(
                payload["reference_surface_analysis"]["fidelity_ceiling"],
                "single_image_visible_surface_only"
            );
            assert_eq!(
                payload["reference_surface_analysis"]["target_root_recipe"]["recipe_id"],
                root_recipe_id
            );
            assert_eq!(
                payload["reference_surface_analysis"]["bindings"]
                    .as_array()
                    .unwrap()
                    .len(),
                4
            );
            assert_eq!(
                runtime.repository.snapshot(&project_id).unwrap(),
                Some(snapshot)
            );
            assert_eq!(
                runtime.repository.head(&project_id).unwrap(),
                Some(base.asset_version_id.clone())
            );
            let frozen_plan = handled(
                &runtime,
                &request(
                    AllowedHttpMethod::Get,
                    format!(
                        "/api/v1/agent/projects/{project_id}/reference-guided-rebuild-plans/{}",
                        payload["reference_guided_rebuild_plan"]["rebuild_plan_id"]
                            .as_str()
                            .unwrap()
                    ),
                    Vec::new(),
                    Value::Null,
                ),
            );
            assert_eq!(frozen_plan.status, 200, "{:?}", response_json(&frozen_plan));
            let frozen_payload = response_json(&frozen_plan);
            assert_eq!(
                frozen_payload["reference_guided_rebuild_plan"]["rebuild_plan_id"],
                payload["reference_guided_rebuild_plan"]["rebuild_plan_id"]
            );
            assert_eq!(
                frozen_payload["reference_surface_analysis"]["analysis_id"],
                payload["reference_surface_analysis"]["analysis_id"]
            );
            let replayed = handled(
                &runtime,
                &request(
                    AllowedHttpMethod::Post,
                    format!("/api/v1/agent/projects/{project_id}/reference-guided-rebuild:preview"),
                    vec![("Idempotency-Key".into(), rebuild_key.into())],
                    json!({
                        "schema_version":"ReferenceGuidedRebuildPreviewRequest@1",
                        "client_request_id":rebuild_key,
                        "evidence_id":evidence_id,
                        "domain_pack_id":"pack_robotic_arm_concept",
                        "base_asset_version_id":base.asset_version_id,
                    }),
                ),
            );
            assert_eq!(replayed.status, 201, "{:?}", response_json(&replayed));
            let replayed_payload = response_json(&replayed);
            assert_eq!(
                replayed_payload["reference_guided_rebuild_plan"]["rebuild_plan_id"],
                payload["reference_guided_rebuild_plan"]["rebuild_plan_id"]
            );
            assert_eq!(
                replayed_payload["reference_surface_analysis"]["analysis_id"],
                payload["reference_surface_analysis"]["analysis_id"]
            );
        }
    }

    #[test]
    fn r007b_evidence_classes_lower_to_distinct_visible_c106_surface_effects() {
        let test_root = TestRoot::new("r007b-content-facts");
        let runtime =
            RustCoreRuntime::open(test_root.path(), "runtime-r007b-content-facts").unwrap();
        let project_id = create_project(&runtime);
        let registry = RecipeRegistry::from_embedded_c106_robotic_arm().unwrap();
        let root = registry
            .recipe("recipe_c106_arm_desktop_assistant")
            .unwrap();
        let expanded = RecipeExpander::expand(
            &registry,
            &RecipeInstantiationRequest {
                schema_version: "ComponentRecipeInstantiationRequest@1".into(),
                context_mode: "initial_candidate".into(),
                request_id: "recipereq_r007b_effects".into(),
                project_id: None,
                base_asset_version_id: None,
                snapshot_revision: None,
                domain_pack_id: "pack_robotic_arm_concept".into(),
                recipe_registry_sha256: registry.registry_sha256().into(),
                recipe: ComponentRecipeRef {
                    schema_version: "ComponentRecipeRef@1".into(),
                    recipe_id: root.recipe_id.clone(),
                    version: root.version,
                    recipe_sha256: RecipeValidator::recipe_sha256(root).unwrap(),
                },
                target_part_id: None,
                slot_bindings: Vec::new(),
                parameter_values: Vec::new(),
                material_zone_overrides: Vec::new(),
            },
            &RecipeExpansionPolicy::default(),
        )
        .unwrap();
        let base = AgentAssetVersion {
            asset_version_id: "assetver_r007b_content_v1".into(),
            project_id: project_id.clone(),
            parent_asset_version_id: None,
            version_no: 1,
            status: AssetVersionStatus::Committed,
            summary: "C106 content-fact baseline".into(),
            stage: AssetStage::EditableAsset,
            plan_id: "plan_r007b_content".into(),
            direction_id: "direction_r007b_content".into(),
            domain_pack_id: "pack_robotic_arm_concept".into(),
            artifact_id: "artifact_r007b_content".into(),
            parts: expanded.expanded_assembly_graph["parts"]
                .as_array()
                .unwrap()
                .clone(),
            shape_program: expanded.expanded_shape_program,
            assembly_graph: expanded.expanded_assembly_graph,
            material_bindings: BTreeMap::new(),
            created_at: "2026-07-19T00:00:00Z".into(),
        };
        runtime.repository.commit_initial_asset(&base).unwrap();
        let graph = &base.assembly_graph;
        let parts = graph["parts"].as_array().unwrap();
        let root_part_id = graph["root_part_id"].as_str().unwrap();
        let image_evidence =
            |evidence_id: &str, facts: forgecad_core::ReferenceImageSurfaceFacts| {
                ReferenceEvidence {
                    schema_version: "ReferenceEvidence@1".into(),
                    evidence_id: evidence_id.into(),
                    project_id: project_id.clone(),
                    kind: ReferenceEvidenceKind::Image,
                    reference_class: ReferenceClass::SingleImage,
                    domain_pack_id: "pack_robotic_arm_concept".into(),
                    source_file_name: "authorized-reference.png".into(),
                    source_media_type: "image/png".into(),
                    source_object_sha256: "a".repeat(64),
                    source_imported_asset_version_id: None,
                    source_statement: "authorized fixture".into(),
                    license_statement: "user declared rights".into(),
                    missing_views: vec!["rear".into()],
                    user_notes: "visible-only fixture".into(),
                    observations: ReferenceEvidenceObservations {
                        silhouette_summary: "visible arm silhouette".into(),
                        proportion_ranges: vec!["visible ratio".into()],
                        material_zone_observations: vec!["visible blue trim".into()],
                        visible_part_hypotheses: Vec::new(),
                        uncertainties: vec!["hidden structure".into()],
                        image_surface_facts: Some(facts),
                    },
                    created_at: "2026-07-19T00:00:00Z".into(),
                    glb_inspection: None,
                }
            };
        let wide_blue = image_evidence(
            "refevid_r007b_image_wide_blue",
            forgecad_core::ReferenceImageSurfaceFacts {
                width: 1_600,
                height: 900,
                aspect_ratio_milli: 1_777,
                dominant_color_buckets: vec![forgecad_core::ReferenceImageColorBucket::Blue],
                brightness: forgecad_core::ReferenceImageBrightnessBucket::Bright,
                edge_density: forgecad_core::ReferenceImageEdgeDensityBucket::Medium,
                foreground_bbox_normalized: [20, 180, 980, 700],
                contact_sheet_layout_evidence: false,
                foreground_confidence: forgecad_core::ReferenceImageForegroundConfidence::Medium,
            },
        );
        let tall_dense = image_evidence(
            "refevid_r007b_image_tall_dense",
            forgecad_core::ReferenceImageSurfaceFacts {
                width: 900,
                height: 1_600,
                aspect_ratio_milli: 562,
                dominant_color_buckets: vec![forgecad_core::ReferenceImageColorBucket::Gray],
                brightness: forgecad_core::ReferenceImageBrightnessBucket::Dark,
                edge_density: forgecad_core::ReferenceImageEdgeDensityBucket::High,
                foreground_bbox_normalized: [300, 20, 700, 980],
                contact_sheet_layout_evidence: false,
                foreground_confidence: forgecad_core::ReferenceImageForegroundConfidence::Medium,
            },
        );
        let disabled = r007b_reference_surface_effect(
            &runtime.repository,
            &wide_blue,
            &base,
            parts,
            root_part_id,
            "rebuildplan_r007b_image_disabled",
        )
        .unwrap_err();
        assert_eq!(disabled.code(), "SURFACE_ADORNMENT_SKILL_DISABLED");
        let mut missing_image_facts = wide_blue.clone();
        missing_image_facts.observations.image_surface_facts = None;
        assert_eq!(
            r007b_reference_surface_effect(
                &runtime.repository,
                &missing_image_facts,
                &base,
                parts,
                root_part_id,
                "rebuildplan_r007b_image_missing_facts",
            )
            .unwrap_err()
            .code(),
            "REFERENCE_REBUILD_IMAGE_SURFACE_FACTS_REQUIRED"
        );
        let skill_enable_key = "enable_surface_adornment_r007b_content";
        assert_eq!(
            handled(
                &runtime,
                &request(
                    AllowedHttpMethod::Post,
                    "/api/v1/agent/skills/surface-adornment:enable",
                    vec![("Idempotency-Key".into(), skill_enable_key.into())],
                    json!({
                        "schema_version":"EnableSurfaceAdornmentSkillRequest@1",
                        "client_request_id":skill_enable_key,
                        "confirm_enable":true
                    }),
                ),
            )
            .status,
            200
        );
        let wide = r007b_reference_surface_effect(
            &runtime.repository,
            &wide_blue,
            &base,
            parts,
            root_part_id,
            "rebuildplan_r007b_image_wide",
        )
        .unwrap();
        let tall = r007b_reference_surface_effect(
            &runtime.repository,
            &tall_dense,
            &base,
            parts,
            root_part_id,
            "rebuildplan_r007b_image_tall",
        )
        .unwrap();
        assert_eq!(wide.operations.len(), 2);
        assert_eq!(wide.operations[1]["op"], "apply_surface_adornment");
        assert_eq!(
            wide.operations[1]["surface_adornment_program"]["skill_version"],
            2
        );
        assert_eq!(
            wide.operations[1]["surface_adornment_program"]["motif"],
            "double_flowline"
        );
        assert_eq!(
            tall.operations[1]["surface_adornment_program"]["motif"],
            "hex_microgrid"
        );
        assert_ne!(wide.operations[0]["value"], tall.operations[0]["value"]);
        assert_ne!(
            wide.operations[1]["surface_adornment_program"]["seed"],
            tall.operations[1]["surface_adornment_program"]["seed"]
        );

        let glb = |evidence_id: &str, inspection: forgecad_core::ImportedGlbInspection| {
            ReferenceEvidence {
                schema_version: "ReferenceEvidence@1".into(),
                evidence_id: evidence_id.into(),
                project_id: project_id.clone(),
                kind: ReferenceEvidenceKind::Glb,
                reference_class: ReferenceClass::GlbReadback,
                domain_pack_id: "pack_robotic_arm_concept".into(),
                source_file_name: "authorized-reference.glb".into(),
                source_media_type: "model/gltf-binary".into(),
                source_object_sha256: inspection.sha256.clone(),
                source_imported_asset_version_id: None,
                source_statement: "authorized fixture".into(),
                license_statement: "user declared rights".into(),
                missing_views: Vec::new(),
                user_notes: "visible-only fixture".into(),
                observations: ReferenceEvidenceObservations {
                    silhouette_summary: "visible arm silhouette".into(),
                    proportion_ranges: vec!["visible ratio".into()],
                    material_zone_observations: vec!["visible material slots".into()],
                    visible_part_hypotheses: Vec::new(),
                    uncertainties: vec!["hidden structure".into()],
                    image_surface_facts: None,
                },
                created_at: "2026-07-19T00:00:00Z".into(),
                glb_inspection: Some(inspection),
            }
        };
        let glb_long = glb(
            "refevid_r007b_glb_long",
            forgecad_core::ImportedGlbInspection {
                sha256: "b".repeat(64),
                byte_size: 1_024,
                triangle_count: 6_000,
                bounds_mm: [260.0, 100.0, 70.0],
                mesh_count: 4,
                primitive_count: 6,
                material_count: 4,
                node_count: 12,
            },
        );
        let glb_compact = glb(
            "refevid_r007b_glb_compact",
            forgecad_core::ImportedGlbInspection {
                sha256: "c".repeat(64),
                byte_size: 2_048,
                triangle_count: 200,
                bounds_mm: [100.0, 95.0, 90.0],
                mesh_count: 3,
                primitive_count: 2,
                material_count: 1,
                node_count: 3,
            },
        );
        let mut missing_glb_facts = glb_compact.clone();
        missing_glb_facts.glb_inspection = None;
        assert_eq!(
            r007b_reference_surface_effect(
                &runtime.repository,
                &missing_glb_facts,
                &base,
                parts,
                root_part_id,
                "rebuildplan_r007b_glb_missing_facts",
            )
            .unwrap_err()
            .code(),
            "REFERENCE_SURFACE_GLB_READBACK_REQUIRED"
        );
        let long_effect = r007b_reference_surface_effect(
            &runtime.repository,
            &glb_long,
            &base,
            parts,
            root_part_id,
            "rebuildplan_r007b_glb_long",
        )
        .unwrap();
        let compact_effect = r007b_reference_surface_effect(
            &runtime.repository,
            &glb_compact,
            &base,
            parts,
            root_part_id,
            "rebuildplan_r007b_glb_compact",
        )
        .unwrap();
        assert_ne!(
            long_effect.operations[0]["value"],
            compact_effect.operations[0]["value"]
        );
        assert_ne!(
            long_effect.operations[1]["surface_adornment_program"]["seed"],
            compact_effect.operations[1]["surface_adornment_program"]["seed"]
        );
    }
}
