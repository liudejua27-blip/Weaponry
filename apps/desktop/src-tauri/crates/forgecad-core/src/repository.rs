use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use crate::skills::MATERIAL_PRESET_IDS;
use crate::{
    builtin_surface_adornment_manifest, builtin_surface_adornment_manifest_v2,
    external_glb::{build_external_asset_version, safe_import_file_name},
    migration::open_connection,
    read_ownership_marker, ActiveDesign, ActiveDesignSnapshot, AgentAssetChangeSet,
    AgentAssetVersion, AgentComponentCandidate, AgentComponentCompatibility, AgentComponentRecord,
    AgentSkillActivation, AgentSkillDryRun, AgentSkillEvalReport, AgentSkillManifest,
    AgentStructureSuggestion, AgentStructureSuggestionList, ArtifactMigrationRunner,
    AssetVersionStatus, BlockoutCandidate, BootstrapLease, CandidateBundleReadback,
    CandidateStatus, ChangeSetConfirmBundleReadback, ChangeSetPreviewBundleReadback,
    ChangeSetStatus, ComponentRecipeRef, ContentAddressedObjectStore, CoreError, CoreResult,
    CreateReferenceEvidenceRequest, ExpandedComponentCandidate, ExportReference,
    ExternalGlbImportBundleReadback, ForgeCadGlbReadback, ImportExternalGlbRequest,
    ImportExternalGlbResponse, ImportedGlbRecord, LegacyActiveDesignConversionResponse,
    LegacyActiveDesignSource, LegacyAgentConversionIntent, MaterialTextureLicense,
    MaterialTextureObject, MaterialTextureQuery, MaterialTextureRole, MaterialTextureSource,
    NavigationAction, NavigationAvailability, NavigationResult, ObjectRecord, ObjectReference,
    PartDisplay, PreviewReference, Project, QualityReference, QualityReport, QualityStatus,
    RecipeExpander, RecipeExpansionPolicy, RecipeInstantiationRequest, RecipeRegistry,
    RecipeSlotBinding, RecipeSurfaceAdornmentSlot, ReferenceEvidence, ReferenceEvidenceKind,
    ReferenceGuidedRebuildPlan, ReferenceGuidedRebuildPlanStatus, ReferenceSurfaceAnalysis,
    RegisterMaterialTextureRequest, RenderPreset, SkillEvalStatus, SnapshotEtag, StoredObject,
    SurfaceAdornmentProgram, WriterLease, EXTERNAL_GLB_REFERENCE_ROLE,
    REFERENCE_EVIDENCE_SCHEMA_VERSION, REFERENCE_EVIDENCE_SOURCE_ROLE,
    REFERENCE_GUIDED_REBUILD_PLAN_SCHEMA_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

mod legacy_read;

pub use legacy_read::LegacyModuleGlb;

const CHANGE_SET_PREVIEW_SEAL_SCHEMA: &str = "ChangeSetPreviewSeal@1";
const CHANGE_SET_CONFIRM_SEAL_SCHEMA: &str = "ChangeSetConfirmSeal@1";

const C106_ROBOTIC_ARM_DOMAIN: &str = "pack_robotic_arm_concept";
const C106_ROOT_RECIPE_IDS: &[&str] = &[
    "recipe_c106_arm_desktop_assistant",
    "recipe_c106_arm_gallery_industrial",
    "recipe_c106_arm_service_display",
];
const C110C_ARM_RECIPES: &[&str] = &[
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
    "recipe_c110g_parallel_rail",
    "recipe_c110g_parallel_carriage",
    "recipe_c110g_parallel_link",
    "recipe_c110g_parallel_end_effector",
];
const C110C_ARM_ATTACHMENT_SLOTS: &[&str] = &[
    "slot_arm_sensor_pod",
    "slot_arm_guard_rail",
    "slot_arm_tool_changer",
    "slot_arm_camera_boom",
    "slot_c110g_parallel_rail",
    "slot_c110g_parallel_carriage",
    "slot_c110g_parallel_link",
    "slot_c110g_parallel_tool",
];

/// Read-only projection of a frozen R007/R007B reference-result pair. Hashes
/// remain authoritative in `ReferenceEvidence` and the asset-version object
/// reference; this value intentionally stores no copied hash fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceGuidedRebuildFrozenPair {
    pub plan: ReferenceGuidedRebuildPlan,
    pub evidence: ReferenceEvidence,
    pub analysis: Option<ReferenceSurfaceAnalysis>,
    pub confirmed_production_glb: Option<ObjectRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ChangeSetPreviewSeal {
    schema_version: String,
    sealed_preview: AgentAssetVersion,
    interactive_readback: serde_json::Value,
    interactive_glb_sha256: String,
    interactive_glb_byte_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ChangeSetConfirmSeal {
    schema_version: String,
    sealed_preview: AgentAssetVersion,
    interactive_readback: serde_json::Value,
    interactive_glb_sha256: String,
    interactive_glb_byte_size: u64,
    resulting_asset_version_id: String,
    production_glb_sha256: String,
    production_glb_byte_size: u64,
    quality_report_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CandidateBundleActivation {
    FreshProject,
    AuthorizedLegacy(LegacyAgentConversionIntent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeletionRecoveryPhase {
    Bootstrap,
    Published,
}

#[derive(Debug, Clone)]
pub struct CoreRepository {
    lease: Arc<WriterLease>,
    object_store: ContentAddressedObjectStore,
}

impl CoreRepository {
    /// Full desktop bootstrap: migrate, discover first-cutover versus restart,
    /// acquire the single-writer lease and recover pending CAS journals.
    /// Call `publish()` only after all remaining handler initialization passes.
    pub fn open(
        db_path: impl AsRef<Path>,
        library_root: impl AsRef<Path>,
        instance_id: impl Into<String>,
    ) -> CoreResult<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let library_root: PathBuf = library_root.as_ref().to_path_buf();
        let instance_id = instance_id.into();
        let bootstrap = BootstrapLease::acquire(&library_root)?;
        crate::MigrationRunner::new(&db_path).run()?;
        let marker = read_ownership_marker(&db_path)?;
        let lease = bootstrap.cutover(&db_path, instance_id, marker.state_owner)?;
        if let Err(error) = ArtifactMigrationRunner::new(&db_path, &library_root).run(&lease) {
            let _ = lease.rollback_before_publish();
            return Err(error);
        }
        let object_store = ContentAddressedObjectStore::new(&library_root)?;
        match Self::new(lease.clone(), object_store) {
            Ok(repository) => Ok(repository),
            Err(error) => {
                let _ = lease.rollback_before_publish();
                Err(error)
            }
        }
    }

    pub fn new(
        lease: Arc<WriterLease>,
        object_store: ContentAddressedObjectStore,
    ) -> CoreResult<Self> {
        // Opening once here makes configuration errors fail before any API is
        // exposed. Every mutation still opens a fresh transaction and checks
        // the durable lease inside that transaction.
        let connection = open_connection(lease.db_path())?;
        lease.assert_current(&connection)?;
        let repository = Self {
            lease,
            object_store,
        };
        repository.recover_object_store()?;
        repository.recover_object_deletions_during_bootstrap()?;
        repository.adopt_legacy_material_texture_objects()?;
        Ok(repository)
    }

    pub fn db_path(&self) -> &Path {
        self.lease.db_path()
    }

    pub fn publish(&self) -> CoreResult<()> {
        self.lease.publish()
    }

    pub fn rollback_cutover_before_publish(&self) -> CoreResult<bool> {
        self.lease.rollback_before_publish()
    }

    /// Installs the code-owned compatibility profile during unpublished
    /// bootstrap. It creates no Project, legacy Version or ModuleGraph.
    pub fn ensure_default_domain_profile(&self, timestamp: &str) -> CoreResult<serde_json::Value> {
        let profile = serde_json::json!({
            "schema_version": "DesignDomainProfile@1",
            "profile_id": "profile_weapon_concept_v1",
            "domain_type": "weapon_concept",
            "display_name": "Weapon Concept Pack",
            "pack_id": "pack_weapon_concept_v1",
            "intended_uses": ["visual_asset", "game_asset", "film_prop", "non_functional_display"],
            "module_categories": ["core_shell", "front_shell", "rear_shell", "grip_shell", "top_accessory", "side_accessory", "lower_structure", "storage_visual", "armor_panel"],
            "required_connectors": ["core.front", "core.rear", "core.grip"],
            "optional_connectors": ["core.top", "core.bottom", "core.left", "core.right", "core.side_panel_left", "core.side_panel_right"],
            "export_profiles": ["visual_asset", "game_asset", "film_prop", "non_functional_display"],
            "non_functional_only": true
        });
        let profile_json = json_text(&profile)?;
        let sha = crate::semantic_sha256(&profile)?;
        self.bootstrap_write(|transaction| {
            transaction.execute(
                "INSERT OR IGNORE INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at) VALUES ('profile_weapon_concept_v1', 'weapon_concept', 'DesignDomainProfile@1', 'pack_weapon_concept_v1', 'Weapon Concept Pack', ?, ?, 'active', ?, ?)",
                params![profile_json, sha, timestamp, timestamp],
            )?;
            Ok(())
        })?;
        let stored = self
            .domain_profile("profile_weapon_concept_v1")?
            .ok_or_else(|| CoreError::not_found("default domain profile"))?;
        if stored != profile {
            return Err(CoreError::conflict(
                "DEFAULT_DOMAIN_PROFILE_DRIFT",
                "The persisted code-owned default Domain Profile does not match this runtime.",
            ));
        }
        Ok(stored)
    }

    pub fn domain_profile(&self, profile_id: &str) -> CoreResult<Option<serde_json::Value>> {
        let connection = open_connection(self.db_path())?;
        let value: Option<String> = connection
            .query_row(
                "SELECT profile_json FROM domain_profiles WHERE profile_id=? AND status!='disabled'",
                [profile_id],
                |row| row.get(0),
            )
            .optional()?;
        value.map(parse_json).transpose()
    }

    /// Registers one bounded visual-only texture. Bytes are promoted to the
    /// Rust CAS before a single SQLite transaction seals the legacy M103 row,
    /// the Rust object index/reference and the exact idempotency response.
    /// No Project, Version, ChangeSet or ActiveDesignSnapshot row is touched.
    pub fn register_material_texture(
        &self,
        request: &RegisterMaterialTextureRequest,
        idempotency_key: &str,
        timestamp: &str,
    ) -> CoreResult<MaterialTextureObject> {
        let validated = request.validate_and_decode()?;
        let request_hash = request.request_hash()?;
        validate_idempotency_identity(
            "POST /api/v1/agent/material-textures",
            idempotency_key,
            &request_hash,
        )?;
        let mut promoted = self
            .object_store
            .stage(&validated.bytes, validated.extension)?
            .promote()?;
        let stored = promoted.metadata().clone();
        let texture_asset_id = format!("asset_tex_{}", &stored.sha256[..24]);
        let legacy_object_path = format!("objects/sha256/{}", stored.relative_path);
        let result = self.idempotent_write(
            "POST /api/v1/agent/material-textures",
            idempotency_key,
            &request_hash,
            timestamp,
            |transaction| {
                let by_sha_role = material_texture_row_optional(
                    transaction,
                    "sha256=? AND texture_role=?",
                    params![stored.sha256, request.texture_role.as_str()],
                )?;
                let by_id = material_texture_row_optional(
                    transaction,
                    "texture_asset_id=?",
                    params![texture_asset_id],
                )?;
                let existing = match (by_sha_role, by_id) {
                    (Some(left), Some(right)) if left.texture_asset_id == right.texture_asset_id => {
                        Some(left)
                    }
                    (None, None) => None,
                    (Some(existing), None) | (None, Some(existing)) => Some(existing),
                    (Some(_), Some(_)) => {
                        return Err(CoreError::conflict(
                            "TEXTURE_METADATA_CONFLICT",
                            "Texture SHA-derived identity conflicts with existing M103 metadata.",
                        ))
                    }
                };

                let row = if let Some(existing) = existing {
                    if !material_texture_metadata_matches(
                        &existing,
                        request,
                        &stored,
                        validated.width,
                        validated.height,
                        &legacy_object_path,
                    ) {
                        return Err(CoreError::conflict(
                            "TEXTURE_METADATA_CONFLICT",
                            "The same texture content is already registered with different visual provenance or use metadata.",
                        ));
                    }
                    existing
                } else {
                    transaction.execute(
                        "INSERT INTO agent_material_texture_objects(texture_asset_id, texture_role, display_name, mime_type, byte_size, sha256, object_path, width, height, source, license, license_ref, thumbnail_asset_id, visual_only, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
                        params![
                            texture_asset_id,
                            request.texture_role.as_str(),
                            request.display_name,
                            request.mime_type,
                            stored.byte_size,
                            stored.sha256,
                            legacy_object_path,
                            validated.width,
                            validated.height,
                            request.source.as_str(),
                            request.license.as_str(),
                            request.license_ref,
                            request.thumbnail_asset_id,
                            timestamp,
                            timestamp,
                        ],
                    )?;
                    material_texture_row_optional(
                        transaction,
                        "texture_asset_id=?",
                        params![texture_asset_id],
                    )?
                    .ok_or_else(|| {
                        CoreError::conflict(
                            "TEXTURE_NOT_PERSISTED",
                            "Texture registration did not read back from SQLite.",
                        )
                    })?
                };

                insert_object_metadata(transaction, &stored, timestamp)?;
                transaction.execute(
                    "INSERT OR IGNORE INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('texture', ?, ?, ?, ?)",
                    params![row.texture_asset_id, row.texture_role.as_str(), stored.sha256, timestamp],
                )?;
                let reference_matches: bool = transaction.query_row(
                    "SELECT sha256=? FROM forgecad_core_object_references WHERE reference_kind='texture' AND owner_id=? AND role=?",
                    params![stored.sha256, row.texture_asset_id, row.texture_role.as_str()],
                    |sql_row| sql_row.get(0),
                )?;
                if !reference_matches {
                    return Err(CoreError::conflict(
                        "TEXTURE_OBJECT_REFERENCE_CONFLICT",
                        "Texture object reference conflicts with the sealed content identity.",
                    ));
                }
                let texture = row.into_model(true);
                texture.validate()?;
                Ok(texture)
            },
        );
        match result {
            Ok(texture) => {
                if let Err(error) = texture.validate() {
                    promoted.cleanup_after_rollback();
                    return Err(error);
                }
                if promoted.finalize_commit().is_err() {
                    self.recover_object_store()?;
                }
                Ok(texture)
            }
            Err(error) => {
                promoted.cleanup_after_rollback();
                Err(error)
            }
        }
    }

    pub fn material_texture(
        &self,
        texture_asset_id: &str,
    ) -> CoreResult<Option<MaterialTextureObject>> {
        crate::models::validate_material_texture_asset_id(texture_asset_id)?;
        let connection = open_connection(self.db_path())?;
        let row = material_texture_row_optional(
            &connection,
            "texture_asset_id=?",
            params![texture_asset_id],
        )?;
        row.map(|row| {
            let exists = self.material_texture_object_exists(&connection, &row)?;
            let texture = row.into_model(exists);
            texture.validate()?;
            Ok(texture)
        })
        .transpose()
    }

    pub fn list_material_textures(
        &self,
        query: &MaterialTextureQuery,
    ) -> CoreResult<Vec<MaterialTextureObject>> {
        query.validate()?;
        let connection = open_connection(self.db_path())?;
        let texture_role = query.texture_role.map(|value| value.as_str().to_string());
        let source = query.source.map(|value| value.as_str().to_string());
        let search = query
            .query
            .as_ref()
            .map(|value| format!("%{}%", value.to_lowercase()));
        let mut statement = connection.prepare(
            "SELECT texture_asset_id, texture_role, display_name, mime_type, byte_size, sha256, object_path, width, height, source, license, license_ref, thumbnail_asset_id, created_at, updated_at FROM agent_material_texture_objects WHERE (?1 IS NULL OR texture_role=?1) AND (?2 IS NULL OR source=?2) AND (?3 IS NULL OR lower(display_name) LIKE ?3 OR lower(texture_asset_id) LIKE ?3) ORDER BY updated_at DESC, texture_asset_id LIMIT ?4",
        )?;
        let rows = statement.query_map(
            params![texture_role, source, search, query.limit as u64],
            material_texture_row_from_sql,
        )?;
        let mut items = Vec::new();
        for row in rows {
            let row = row?;
            let exists = self.material_texture_object_exists(&connection, &row)?;
            let texture = row.into_model(exists);
            texture.validate()?;
            items.push(texture);
        }
        Ok(items)
    }

    fn material_texture_object_exists(
        &self,
        connection: &Connection,
        row: &MaterialTextureRow,
    ) -> CoreResult<bool> {
        let stored: Option<StoredObject> = connection
            .query_row(
                "SELECT o.sha256, o.object_path, o.extension, o.byte_size FROM forgecad_core_object_references r JOIN forgecad_core_objects o ON o.sha256=r.sha256 WHERE r.reference_kind='texture' AND r.owner_id=? AND r.role=? AND r.sha256=?",
                params![row.texture_asset_id, row.texture_role.as_str(), row.sha256],
                |sql_row| {
                    Ok(StoredObject {
                        sha256: sql_row.get(0)?,
                        relative_path: sql_row.get(1)?,
                        extension: sql_row.get(2)?,
                        byte_size: sql_row.get(3)?,
                    })
                },
            )
            .optional()?;
        Ok(stored
            .as_ref()
            .is_some_and(|stored| self.object_store.read(stored).is_ok()))
    }

    fn adopt_legacy_material_texture_objects(&self) -> CoreResult<()> {
        let connection = open_connection(self.db_path())?;
        let mut statement = connection.prepare(
            "SELECT texture_asset_id, texture_role, mime_type, byte_size, sha256, object_path, updated_at FROM agent_material_texture_objects ORDER BY texture_asset_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?;
        let legacy = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);

        for (
            texture_asset_id,
            texture_role,
            mime_type,
            byte_size,
            sha256,
            object_path,
            updated_at,
        ) in legacy
        {
            if crate::models::validate_material_texture_asset_id(&texture_asset_id).is_err()
                || texture_asset_id != format!("asset_tex_{}", &sha256[..sha256.len().min(24)])
                || MaterialTextureRole::from_str(&texture_role).is_err()
            {
                continue;
            }
            let extension = match mime_type.as_str() {
                "image/png" => "png",
                "image/jpeg" => "jpg",
                "image/webp" => "webp",
                _ => continue,
            };
            let stored = match self.object_store.adopt_existing_legacy_object(
                &object_path,
                &sha256,
                byte_size,
                extension,
            ) {
                Ok(stored) => stored,
                Err(error)
                    if matches!(
                        error.code(),
                        "LEGACY_OBJECT_PATH_INVALID"
                            | "LEGACY_OBJECT_MISSING"
                            | "LEGACY_OBJECT_CORRUPT"
                    ) =>
                {
                    continue
                }
                Err(error) => return Err(error),
            };
            self.bootstrap_write(|transaction| {
                insert_object_metadata(transaction, &stored, &updated_at)?;
                transaction.execute(
                    "INSERT OR IGNORE INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('texture', ?, ?, ?, ?)",
                    params![texture_asset_id, texture_role, sha256, updated_at],
                )?;
                let matches: bool = transaction.query_row(
                    "SELECT sha256=? FROM forgecad_core_object_references WHERE reference_kind='texture' AND owner_id=? AND role=?",
                    params![sha256, texture_asset_id, texture_role],
                    |row| row.get(0),
                )?;
                if !matches {
                    return Err(CoreError::conflict(
                        "LEGACY_TEXTURE_REFERENCE_CONFLICT",
                        "Historical M103 texture conflicts with an existing Rust object reference.",
                    ));
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Imports one user GLB as an immutable, read-only reference asset.
    ///
    /// Strict inspection and CAS promotion happen before one SQLite IMMEDIATE
    /// transaction seals the Version, legacy import metadata, Rust object
    /// reference, Head, Snapshot and historical-compatible idempotency
    /// response.  The external bytes never become a ShapeProgram or a
    /// production-quality artifact.
    pub fn import_external_glb(
        &self,
        request: &ImportExternalGlbRequest,
        idempotency_key: &str,
        timestamp: &str,
    ) -> CoreResult<ExternalGlbImportBundleReadback> {
        const SCOPE: &str = "POST /api/v1/agent/imports:glb";
        let validated = request.validate_and_decode()?;
        let request_hash = request.request_hash()?;
        validate_idempotency_identity(SCOPE, idempotency_key, &request_hash)?;
        if !is_registered_import_domain_pack(&request.domain_pack_id) {
            return Err(CoreError::invalid_data(
                "DOMAIN_PACK_UNKNOWN",
                "Imported GLB must use one of the four registered visual Domain Packs.",
            ));
        }
        let identity = crate::semantic_sha256(&serde_json::json!({
            "scope":SCOPE,
            "idempotency_key":idempotency_key,
            "request_hash":request_hash,
        }))?;
        let asset_version_id = format!("assetver_import_{}", &identity[..20]);
        let import_id = format!("glbimport_{}", &identity[20..40]);

        let mut promoted = self
            .object_store
            .stage(&validated.bytes, "glb")?
            .promote()?;
        let stored = promoted.metadata().clone();
        if stored.sha256 != validated.inspection.sha256
            || stored.byte_size != validated.inspection.byte_size
        {
            promoted.cleanup_after_rollback();
            return Err(CoreError::conflict(
                "GLB_HASH_MISMATCH",
                "Imported GLB CAS identity does not match strict inspection.",
            ));
        }
        let legacy_object_path = format!("objects/sha256/{}", stored.relative_path);
        let response_result = self.idempotent_write(
            SCOPE,
            idempotency_key,
            &request_hash,
            timestamp,
            |transaction| {
                let project_active: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id=? AND status='active')",
                    [&request.project_id],
                    |row| row.get(0),
                )?;
                if !project_active {
                    return Err(CoreError::not_found("Project"));
                }
                let version_no: u64 = transaction.query_row(
                    "SELECT COALESCE(MAX(version_no), 0) + 1 FROM agent_asset_versions WHERE project_id=?",
                    [&request.project_id],
                    |row| row.get(0),
                )?;
                let version = build_external_asset_version(
                    request,
                    &validated.inspection,
                    asset_version_id.clone(),
                    version_no,
                    timestamp,
                )?;

                insert_object_metadata(transaction, &stored, timestamp)?;
                insert_version(transaction, &version)?;
                transaction.execute(
                    "INSERT INTO agent_imported_glbs(import_id, project_id, asset_version_id, domain_pack_id, file_name, object_path, sha256, byte_size, triangle_count, bounds_mm_json, mesh_count, primitive_count, material_count, node_count, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        import_id,
                        request.project_id,
                        version.asset_version_id,
                        request.domain_pack_id,
                        safe_import_file_name(&request.file_name),
                        legacy_object_path,
                        stored.sha256,
                        stored.byte_size,
                        validated.inspection.triangle_count,
                        json_text(&validated.inspection.bounds_mm)?,
                        validated.inspection.mesh_count,
                        validated.inspection.primitive_count,
                        validated.inspection.material_count,
                        validated.inspection.node_count,
                        timestamp,
                    ],
                )?;
                transaction.execute(
                    "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('asset_version', ?, ?, ?, ?)",
                    params![
                        version.asset_version_id,
                        EXTERNAL_GLB_REFERENCE_ROLE,
                        stored.sha256,
                        timestamp,
                    ],
                )?;
                set_head(
                    transaction,
                    &request.project_id,
                    &version.asset_version_id,
                    timestamp,
                )?;
                match snapshot_from_connection(transaction, &request.project_id)? {
                    None => insert_snapshot(transaction, &initial_snapshot(&version)?)?,
                    Some(snapshot)
                        if matches!(snapshot.active_design, ActiveDesign::AgentAsset { .. }) =>
                    {
                        advance_snapshot(transaction, &snapshot, &version, timestamp)?
                    }
                    Some(snapshot) => promote_legacy_snapshot_for_import(
                        transaction,
                        &snapshot,
                        &version,
                        timestamp,
                    )?,
                }
                let response = ImportExternalGlbResponse {
                    asset_version: require_version(transaction, &version.asset_version_id)?,
                    inspection: validated.inspection.clone(),
                };
                let imported = imported_glb_from_connection(
                    transaction,
                    &response.asset_version.asset_version_id,
                )?
                .ok_or_else(|| {
                    CoreError::conflict(
                        "GLB_IMPORT_BUNDLE_INCOMPLETE",
                        "Imported GLB metadata did not read back inside the atomic transaction.",
                    )
                })?;
                if imported.inspection() != response.inspection
                    || require_snapshot(transaction, &request.project_id)?
                        .active_design
                        .asset_version_id()
                        != Some(response.asset_version.asset_version_id.as_str())
                {
                    return Err(CoreError::conflict(
                        "GLB_IMPORT_BUNDLE_INCOMPLETE",
                        "Imported GLB Version, inspection and Snapshot did not seal atomically.",
                    ));
                }
                Ok(response)
            },
        );

        match response_result {
            Ok(response) => {
                if promoted.finalize_commit().is_err() {
                    self.recover_object_store()?;
                }
                let bundle = self
                    .external_glb_import_bundle(&response.asset_version.asset_version_id)?
                    .ok_or_else(|| {
                        CoreError::conflict(
                            "GLB_IMPORT_BUNDLE_INCOMPLETE",
                            "Imported GLB bundle is missing after the atomic commit.",
                        )
                    })?;
                if bundle.response != response {
                    return Err(CoreError::conflict(
                        "GLB_IMPORT_BUNDLE_DRIFT",
                        "Imported GLB idempotency response differs from durable readback.",
                    ));
                }
                self.validate_external_glb_import_bundle(&bundle)?;
                Ok(bundle)
            }
            Err(error) => {
                promoted.cleanup_after_rollback();
                Err(error)
            }
        }
    }

    pub fn imported_glb(&self, asset_version_id: &str) -> CoreResult<Option<ImportedGlbRecord>> {
        let connection = open_connection(self.db_path())?;
        imported_glb_from_connection(&connection, asset_version_id)
    }

    pub fn external_glb_import_bundle(
        &self,
        asset_version_id: &str,
    ) -> CoreResult<Option<ExternalGlbImportBundleReadback>> {
        let connection = open_connection(self.db_path())?;
        let Some(imported_glb) = imported_glb_from_connection(&connection, asset_version_id)?
        else {
            return Ok(None);
        };
        let version = require_version(&connection, asset_version_id)?;
        let object = connection
            .query_row(
                "SELECT o.sha256, o.object_path, o.extension, o.byte_size, o.ref_count, o.created_at, o.updated_at FROM forgecad_core_object_references r JOIN forgecad_core_objects o ON o.sha256=r.sha256 WHERE r.reference_kind='asset_version' AND r.owner_id=? AND r.role=?",
                params![asset_version_id, EXTERNAL_GLB_REFERENCE_ROLE],
                object_record_from_row,
            )
            .optional()?
            .ok_or_else(|| CoreError::not_found("external GLB content object"))?;
        let snapshot =
            snapshot_from_connection(&connection, &version.project_id)?.filter(|snapshot| {
                snapshot.active_design.asset_version_id() == Some(version.asset_version_id.as_str())
            });
        Ok(Some(ExternalGlbImportBundleReadback {
            response: ImportExternalGlbResponse {
                asset_version: version,
                inspection: imported_glb.inspection(),
            },
            imported_glb,
            object,
            snapshot,
        }))
    }

    pub fn validate_external_glb_import_bundle(
        &self,
        bundle: &ExternalGlbImportBundleReadback,
    ) -> CoreResult<()> {
        bundle.imported_glb.validate()?;
        bundle.response.asset_version.validate()?;
        bundle.response.inspection.validate()?;
        let version = &bundle.response.asset_version;
        if !crate::is_external_glb_reference(version)
            || version.project_id != bundle.imported_glb.project_id
            || version.asset_version_id != bundle.imported_glb.asset_version_id
            || version.domain_pack_id != bundle.imported_glb.domain_pack_id
            || bundle.response.inspection != bundle.imported_glb.inspection()
            || bundle.object.sha256 != bundle.imported_glb.sha256
            || bundle.object.byte_size != bundle.imported_glb.byte_size
            || bundle.object.extension != "glb"
            || format!("objects/sha256/{}", bundle.object.object_path)
                != bundle.imported_glb.object_path
            || bundle.object.ref_count < 1
        {
            return Err(CoreError::conflict(
                "GLB_IMPORT_BUNDLE_DRIFT",
                "Imported GLB bundle identities, Snapshot or read-only object role are inconsistent.",
            ));
        }
        if let Some(snapshot) = &bundle.snapshot {
            if snapshot.project_id != version.project_id
                || snapshot.active_design.asset_version_id()
                    != Some(version.asset_version_id.as_str())
                || snapshot.export.source_version_id() != version.asset_version_id
                || snapshot.preview.is_some()
                || snapshot.quality.as_ref().is_some_and(|quality| {
                    quality.project_id != version.project_id
                        || quality.asset_version_id != version.asset_version_id
                })
                || self.head(&version.project_id)?.as_deref()
                    != Some(version.asset_version_id.as_str())
            {
                return Err(CoreError::conflict(
                    "GLB_IMPORT_BUNDLE_DRIFT",
                    "Active imported GLB Snapshot is inconsistent with its Version and Head.",
                ));
            }
        }
        let bytes = self.read_object(&bundle.object.sha256)?;
        let inspection = crate::inspect_external_glb(&bytes)?;
        if inspection != bundle.response.inspection {
            return Err(CoreError::conflict(
                "GLB_IMPORT_READBACK_MISMATCH",
                "Imported GLB bytes no longer match the sealed inspection.",
            ));
        }
        Ok(())
    }

    /// Records user-authorized reference evidence without turning it into
    /// editable geometry. Image/direct-GLB bytes are sealed into CAS; the
    /// compatibility GLB path may alias a pre-existing same-project read-only
    /// import. Neither path creates or advances an asset Version.
    pub fn create_reference_evidence(
        &self,
        request: &CreateReferenceEvidenceRequest,
        timestamp: &str,
    ) -> CoreResult<ReferenceEvidence> {
        request.validate_shape()?;
        let request_sha256 = crate::semantic_sha256(request)?;
        {
            let connection = open_connection(self.db_path())?;
            let existing: Option<(String, String)> = connection.query_row(
                "SELECT evidence_id, request_sha256 FROM reference_evidence WHERE project_id=? AND client_request_id=?",
                params![request.project_id, request.client_request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
            if let Some((evidence_id, stored_request_sha256)) = existing {
                if stored_request_sha256 != request_sha256 {
                    return Err(CoreError::conflict(
                        "IDEMPOTENCY_CONFLICT",
                        "Reference evidence idempotency key was already used for different input.",
                    ));
                }
                return reference_evidence_from_connection(&connection, &evidence_id)?.ok_or_else(
                    || {
                        CoreError::conflict(
                            "REFERENCE_EVIDENCE_READBACK_INCOMPLETE",
                            "Idempotent reference evidence row could not be read back.",
                        )
                    },
                );
            }
        }
        let (resolved, mut promoted) = match request.kind {
            ReferenceEvidenceKind::Image => {
                let bytes = request.decode_image()?;
                // The local analyser runs before CAS promotion, over the exact
                // magic-checked authorized bytes. Only its bounded facts flow
                // into observations; decoded pixels are dropped immediately.
                let image_surface_facts = request.analyze_image(&bytes)?;
                let extension = crate::reference_evidence::image_extension_for_media_type(
                    request
                        .media_type
                        .as_deref()
                        .expect("validated image media type"),
                )?;
                let promoted = self.object_store.stage(&bytes, extension)?.promote()?;
                let object = ObjectRecord {
                    sha256: promoted.metadata().sha256.clone(),
                    object_path: promoted.metadata().relative_path.clone(),
                    extension: promoted.metadata().extension.clone(),
                    byte_size: promoted.metadata().byte_size,
                    ref_count: 1,
                    created_at: timestamp.into(),
                    updated_at: timestamp.into(),
                };
                (
                    crate::reference_evidence::ResolvedReferenceSource {
                        file_name: request
                            .file_name
                            .clone()
                            .expect("validated image file name"),
                        media_type: request
                            .media_type
                            .clone()
                            .expect("validated image media type"),
                        object,
                        imported_asset_version_id: None,
                        glb_inspection: None,
                        image_surface_facts: Some(image_surface_facts),
                    },
                    Some(promoted),
                )
            }
            ReferenceEvidenceKind::Glb => {
                if let Some(source_asset_id) = request.imported_asset_version_id.as_deref() {
                    let bundle = self
                        .external_glb_import_bundle(source_asset_id)?
                        .ok_or_else(|| CoreError::not_found("Imported GLB reference"))?;
                    if bundle.response.asset_version.project_id != request.project_id {
                        return Err(CoreError::conflict(
                            "REFERENCE_EVIDENCE_PROJECT_MISMATCH",
                            "A GLB reference may only be used by its owning Project.",
                        ));
                    }
                    (
                        crate::reference_evidence::ResolvedReferenceSource {
                            file_name: bundle.imported_glb.file_name,
                            media_type: "model/gltf-binary".into(),
                            object: bundle.object,
                            imported_asset_version_id: Some(source_asset_id.into()),
                            glb_inspection: Some(bundle.response.inspection),
                            image_surface_facts: None,
                        },
                        None,
                    )
                } else {
                    let (bytes, inspection) = request.decode_direct_glb()?;
                    let promoted = self.object_store.stage(&bytes, "glb")?.promote()?;
                    let object = ObjectRecord {
                        sha256: promoted.metadata().sha256.clone(),
                        object_path: promoted.metadata().relative_path.clone(),
                        extension: promoted.metadata().extension.clone(),
                        byte_size: promoted.metadata().byte_size,
                        ref_count: 1,
                        created_at: timestamp.into(),
                        updated_at: timestamp.into(),
                    };
                    if object.sha256 != inspection.sha256
                        || object.byte_size != inspection.byte_size
                    {
                        let mut promoted = promoted;
                        promoted.cleanup_after_rollback();
                        return Err(CoreError::conflict(
                            "REFERENCE_GLB_HASH_MISMATCH",
                            "Direct GLB CAS identity does not match its strict inspection.",
                        ));
                    }
                    (
                        crate::reference_evidence::ResolvedReferenceSource {
                            file_name: request
                                .file_name
                                .clone()
                                .expect("validated direct GLB file name"),
                            media_type: "model/gltf-binary".into(),
                            object,
                            imported_asset_version_id: None,
                            glb_inspection: Some(inspection),
                            image_surface_facts: None,
                        },
                        Some(promoted),
                    )
                }
            }
        };
        let evidence_identity = crate::semantic_sha256(&serde_json::json!({
            "project_id": request.project_id,
            "client_request_id": request.client_request_id,
            "request_sha256": request_sha256,
        }))?;
        let evidence_id = format!("refevid_{}", &evidence_identity[..24]);
        let domain_pack_id = request.domain_pack_id.clone().unwrap_or_else(|| {
            // An imported GLB retains its selected visual pack.  Images must
            // declare one rather than silently defaulting to a weapon pack.
            if request.kind == ReferenceEvidenceKind::Glb {
                self.version(
                    request
                        .imported_asset_version_id
                        .as_deref()
                        .expect("validated import"),
                )
                .ok()
                .flatten()
                .map(|value| value.domain_pack_id)
                .unwrap_or_default()
            } else {
                String::new()
            }
        });
        crate::reference_evidence::valid_domain_pack(&domain_pack_id)?;
        let reference_class = match request.kind {
            ReferenceEvidenceKind::Image => request
                .reference_class
                .unwrap_or(crate::ReferenceClass::SingleImage),
            ReferenceEvidenceKind::Glb => crate::ReferenceClass::GlbReadback,
        };
        if reference_class == crate::ReferenceClass::MultiViewContactSheet
            && !resolved
                .image_surface_facts
                .as_ref()
                .map(|facts| facts.contact_sheet_layout_evidence)
                .unwrap_or(false)
        {
            return Err(CoreError::invalid_data(
                "REFERENCE_CLASS_LAYOUT_EVIDENCE_REQUIRED",
                "Multi-view contact-sheet evidence requires a locally observed divider with visible regions on both sides.",
            ));
        }
        let observations = derive_reference_observations(request, &resolved);
        let evidence = ReferenceEvidence {
            schema_version: REFERENCE_EVIDENCE_SCHEMA_VERSION.into(),
            evidence_id: evidence_id.clone(),
            project_id: request.project_id.clone(),
            kind: request.kind,
            reference_class,
            domain_pack_id,
            source_file_name: resolved.file_name.clone(),
            source_media_type: resolved.media_type.clone(),
            source_object_sha256: resolved.object.sha256.clone(),
            source_imported_asset_version_id: resolved.imported_asset_version_id.clone(),
            source_statement: request.source_statement.clone(),
            license_statement: request.license_statement.clone(),
            missing_views: request.missing_views.clone(),
            user_notes: request.user_notes.clone(),
            observations,
            created_at: timestamp.into(),
            glb_inspection: resolved.glb_inspection.clone(),
        };
        evidence.validate()?;
        let result = self.write(|transaction| {
            let existing: Option<(String, String)> = transaction.query_row(
                "SELECT evidence_id, request_sha256 FROM reference_evidence WHERE project_id=? AND client_request_id=?",
                params![request.project_id, request.client_request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).optional()?;
            if let Some((existing_evidence_id, stored_request_sha256)) = existing {
                if stored_request_sha256 != request_sha256 {
                    return Err(CoreError::conflict("IDEMPOTENCY_CONFLICT", "Reference evidence idempotency key was already used for different input."));
                }
                return reference_evidence_from_connection(transaction, &existing_evidence_id)?.ok_or_else(|| CoreError::conflict("REFERENCE_EVIDENCE_READBACK_INCOMPLETE", "Idempotent reference evidence row could not be read back."));
            }
            let project_active: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id=? AND status='active')",
                [&request.project_id],
                |row| row.get(0),
            )?;
            if !project_active { return Err(CoreError::not_found("Project")); }
            if let Some(source_asset_version_id) = &evidence.source_imported_asset_version_id {
                let owner: Option<(String, String)> = transaction.query_row(
                    "SELECT project_id, domain_pack_id FROM agent_asset_versions WHERE asset_version_id=?",
                    [source_asset_version_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                ).optional()?;
                let Some((owner_project_id, owner_domain_pack_id)) = owner else { return Err(CoreError::not_found("Imported GLB reference")); };
                if owner_project_id != evidence.project_id || owner_domain_pack_id != evidence.domain_pack_id {
                    return Err(CoreError::conflict("REFERENCE_EVIDENCE_PROJECT_MISMATCH", "GLB evidence cannot cross Project or Domain Pack boundaries."));
                }
                let is_imported: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM agent_imported_glbs WHERE asset_version_id=?)",
                    [source_asset_version_id], |row| row.get(0),
                )?;
                if !is_imported { return Err(CoreError::conflict("REFERENCE_GLB_NOT_READ_ONLY_IMPORT", "GLB evidence must use a read-only imported GLB.")); }
            }
            if promoted.is_some() {
                insert_object_metadata(transaction, promoted.as_ref().expect("reference promotion").metadata(), timestamp)?;
            }
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('reference', ?, ?, ?, ?)",
                params![evidence.evidence_id, REFERENCE_EVIDENCE_SOURCE_ROLE, evidence.source_object_sha256, timestamp],
            )?;
            transaction.execute(
                "INSERT INTO reference_evidence(evidence_id, project_id, client_request_id, request_sha256, kind, reference_class, domain_pack_id, source_file_name, source_media_type, source_object_sha256, source_imported_asset_version_id, source_statement, license_statement, missing_views_json, user_notes, observations_json, glb_inspection_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    evidence.evidence_id, evidence.project_id, request.client_request_id, request_sha256, evidence.kind.as_str(), evidence.reference_class.as_str(), evidence.domain_pack_id,
                    evidence.source_file_name, evidence.source_media_type, evidence.source_object_sha256,
                    evidence.source_imported_asset_version_id, evidence.source_statement, evidence.license_statement,
                    json_text(&evidence.missing_views)?, evidence.user_notes, json_text(&evidence.observations)?,
                    json_option(&evidence.glb_inspection)?, evidence.created_at,
                ],
            )?;
            reference_evidence_from_connection(transaction, &evidence_id)?.ok_or_else(|| CoreError::conflict(
                "REFERENCE_EVIDENCE_READBACK_INCOMPLETE", "Reference evidence did not read back in its write transaction."
            ))
        });
        match result {
            Ok(record) => {
                if let Some(promoted) = promoted.as_mut() {
                    promoted.finalize_commit()?;
                }
                Ok(record)
            }
            Err(error) => {
                if let Some(promoted) = promoted.as_mut() {
                    promoted.cleanup_after_rollback();
                }
                Err(error)
            }
        }
    }

    pub fn reference_evidence(&self, evidence_id: &str) -> CoreResult<Option<ReferenceEvidence>> {
        let connection = open_connection(self.db_path())?;
        reference_evidence_from_connection(&connection, evidence_id)
    }

    /// Lists immutable, project-local evidence metadata for a restored
    /// workbench. The caller never receives a CAS path or arbitrary object
    /// lookup capability.
    pub fn reference_evidence_for_project(
        &self,
        project_id: &str,
    ) -> CoreResult<Vec<ReferenceEvidence>> {
        let connection = open_connection(self.db_path())?;
        let mut statement = connection.prepare(
            "SELECT evidence_id FROM reference_evidence WHERE project_id=? ORDER BY created_at DESC, evidence_id DESC",
        )?;
        let ids = statement
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|evidence_id| {
                reference_evidence_from_connection(&connection, &evidence_id)?.ok_or_else(|| {
                    CoreError::conflict(
                        "REFERENCE_EVIDENCE_LIST_READBACK_INCOMPLETE",
                        "Reference evidence list contains an unreadable record.",
                    )
                })
            })
            .collect()
    }

    /// Returns a sealed source only after checking both the project ownership
    /// and the immutable `reference_evidence_source` object relation. This is
    /// intentionally evidence-ID based; UI/runtime code cannot provide a hash
    /// or a filesystem path.
    pub fn read_reference_evidence_content(
        &self,
        project_id: &str,
        evidence_id: &str,
    ) -> CoreResult<(ReferenceEvidence, Vec<u8>)> {
        let connection = open_connection(self.db_path())?;
        let evidence = reference_evidence_from_connection(&connection, evidence_id)?
            .ok_or_else(|| CoreError::not_found("Reference evidence"))?;
        if evidence.project_id != project_id {
            return Err(CoreError::conflict(
                "REFERENCE_EVIDENCE_PROJECT_MISMATCH",
                "Reference evidence belongs to another Project.",
            ));
        }
        let source_is_sealed: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM forgecad_core_object_references WHERE reference_kind='reference' AND owner_id=? AND role=? AND sha256=?)",
            params![evidence_id, REFERENCE_EVIDENCE_SOURCE_ROLE, evidence.source_object_sha256],
            |row| row.get(0),
        )?;
        if !source_is_sealed {
            return Err(CoreError::conflict(
                "REFERENCE_EVIDENCE_SOURCE_MISSING",
                "Reference evidence no longer has its sealed source object relation.",
            ));
        }
        let bytes = self.read_object(&evidence.source_object_sha256)?;
        Ok((evidence, bytes))
    }

    /// Persists a constrained R007 plan.  This is not a candidate and does
    /// not write Snapshot, Version, geometry or an external GLB.  A base is
    /// optional so a user-authorized image can start an initial C105 candidate.
    pub fn create_reference_guided_rebuild_plan(
        &self,
        plan: &ReferenceGuidedRebuildPlan,
    ) -> CoreResult<ReferenceGuidedRebuildPlan> {
        self.create_reference_guided_rebuild_plan_with_surface_analysis(plan, None)
    }

    /// Atomically persists one R007 plan and, for the C106/R007B path, its
    /// already-validated frozen design-surface interpretation.  The analysis
    /// is deliberately supplied by Rust orchestration rather than rebuilt by
    /// this repository; replays read these exact canonical bytes.
    pub fn create_reference_guided_rebuild_plan_with_surface_analysis(
        &self,
        plan: &ReferenceGuidedRebuildPlan,
        analysis: Option<&ReferenceSurfaceAnalysis>,
    ) -> CoreResult<ReferenceGuidedRebuildPlan> {
        plan.validate()?;
        if plan.status != ReferenceGuidedRebuildPlanStatus::Draft {
            return Err(CoreError::invalid_data(
                "REFERENCE_REBUILD_PLAN_CREATE_INVALID",
                "New reference rebuild plan must start as draft.",
            ));
        }
        let registry = recipe_registry_for_reference_rebuild_plan(plan)?;
        let recipe = registry.recipe(&plan.recipe_id);
        if registry.registry_sha256() != plan.recipe_registry_sha256
            || recipe.is_none()
            || !recipe
                .expect("checked recipe")
                .allowed_domains
                .iter()
                .any(|domain| domain == &plan.domain_pack_id)
        {
            return Err(CoreError::conflict(
                "REFERENCE_REBUILD_RECIPE_STALE",
                "Reference rebuild plan must name a reviewed current C105 Recipe identity.",
            ));
        }
        self.write(|transaction| {
            let evidence = reference_evidence_from_connection(transaction, &plan.evidence_id)?.ok_or_else(|| CoreError::not_found("Reference evidence"))?;
            if evidence.project_id != plan.project_id || evidence.domain_pack_id != plan.domain_pack_id {
                return Err(CoreError::conflict("REFERENCE_REBUILD_PROJECT_MISMATCH", "Reference rebuild plan and evidence must remain in one Project and Domain Pack."));
            }
            if let Some(base_id) = &plan.base_asset_version_id {
                let base = require_version(transaction, base_id)?;
                if base.project_id != plan.project_id || base.domain_pack_id != plan.domain_pack_id || crate::is_external_glb_reference(&base) {
                    return Err(CoreError::conflict("REFERENCE_REBUILD_BASE_INVALID", "Reference rebuild base must be a same-project editable ForgeCAD asset, never an imported GLB."));
                }
            }
            if let Some(analysis) = analysis {
                crate::validate_reference_surface_analysis_for_plan(analysis, &evidence, plan)?;
            }
            transaction.execute(
                "INSERT INTO reference_guided_rebuild_plans(rebuild_plan_id, project_id, evidence_id, base_asset_version_id, domain_pack_id, recipe_id, recipe_registry_sha256, rebuild_summary, intended_differences_json, retained_evidence_json, unresolved_uncertainties_json, status, preview_change_set_id, confirmed_asset_version_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'draft', NULL, NULL, ?, ?)",
                params![plan.rebuild_plan_id, plan.project_id, plan.evidence_id, plan.base_asset_version_id, plan.domain_pack_id, plan.recipe_id, plan.recipe_registry_sha256, plan.rebuild_summary, json_text(&plan.intended_differences)?, json_text(&plan.retained_evidence)?, json_text(&plan.unresolved_uncertainties)?, plan.created_at, plan.updated_at],
            )?;
            if let Some(analysis) = analysis {
                let analysis_sha256 = crate::semantic_sha256(analysis)?;
                transaction.execute(
                    "INSERT INTO reference_surface_analyses(rebuild_plan_id, analysis_sha256, analysis_json, created_at) VALUES (?, ?, ?, ?)",
                    params![plan.rebuild_plan_id, analysis_sha256, json_text(analysis)?, analysis.created_at],
                )?;
            }
            reference_rebuild_plan_from_connection(transaction, &plan.rebuild_plan_id)?.ok_or_else(|| CoreError::conflict("REFERENCE_REBUILD_PLAN_READBACK_INCOMPLETE", "Reference rebuild plan did not read back in its write transaction."))
        })
    }

    pub fn reference_guided_rebuild_plan(
        &self,
        rebuild_plan_id: &str,
    ) -> CoreResult<Option<ReferenceGuidedRebuildPlan>> {
        let connection = open_connection(self.db_path())?;
        reference_rebuild_plan_from_connection(&connection, rebuild_plan_id)
    }

    /// Lists persisted R007 plans for a project so reopening the workbench
    /// does not depend on a dismissed drawer's in-memory state.
    pub fn reference_guided_rebuild_plans_for_project(
        &self,
        project_id: &str,
    ) -> CoreResult<Vec<ReferenceGuidedRebuildPlan>> {
        let connection = open_connection(self.db_path())?;
        let mut statement = connection.prepare(
            "SELECT rebuild_plan_id FROM reference_guided_rebuild_plans WHERE project_id=? ORDER BY created_at DESC, rebuild_plan_id DESC",
        )?;
        let ids = statement
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|rebuild_plan_id| {
                reference_rebuild_plan_from_connection(&connection, &rebuild_plan_id)?.ok_or_else(
                    || {
                        CoreError::conflict(
                            "REFERENCE_REBUILD_PLAN_LIST_READBACK_INCOMPLETE",
                            "Reference rebuild plan list contains an unreadable record.",
                        )
                    },
                )
            })
            .collect()
    }

    /// Reads the exact C106/R007B analysis sealed with a rebuild plan.  This
    /// is intentionally a readback API: callers must never regenerate an
    /// analysis after a plan has been created.
    pub fn reference_surface_analysis(
        &self,
        rebuild_plan_id: &str,
    ) -> CoreResult<Option<ReferenceSurfaceAnalysis>> {
        let connection = open_connection(self.db_path())?;
        reference_surface_analysis_from_connection(&connection, rebuild_plan_id)
    }

    /// Reads a complete frozen pair from existing authoritative relations. No
    /// source or result hash is duplicated here: evidence owns the source and
    /// the immutable asset-version object reference owns the result GLB.
    pub fn reference_guided_rebuild_frozen_pair(
        &self,
        rebuild_plan_id: &str,
    ) -> CoreResult<Option<ReferenceGuidedRebuildFrozenPair>> {
        let connection = open_connection(self.db_path())?;
        let Some(plan) = reference_rebuild_plan_from_connection(&connection, rebuild_plan_id)?
        else {
            return Ok(None);
        };
        let evidence = reference_evidence_from_connection(&connection, &plan.evidence_id)?
            .ok_or_else(|| {
                CoreError::conflict(
                    "REFERENCE_REBUILD_PAIR_EVIDENCE_MISSING",
                    "Reference rebuild plan has no readable immutable evidence.",
                )
            })?;
        let analysis = reference_surface_analysis_from_connection(&connection, rebuild_plan_id)?;
        let confirmed_production_glb = match plan.confirmed_asset_version_id.as_deref() {
            Some(asset_version_id) => connection.query_row(
                "SELECT o.sha256, o.object_path, o.extension, o.byte_size, o.ref_count, o.created_at, o.updated_at FROM forgecad_core_object_references r JOIN forgecad_core_objects o ON o.sha256=r.sha256 WHERE r.reference_kind='asset_version' AND r.owner_id=? AND r.role='production_glb'",
                [asset_version_id],
                object_record_from_row,
            ).optional()?,
            None => None,
        };
        Ok(Some(ReferenceGuidedRebuildFrozenPair {
            plan,
            evidence,
            analysis,
            confirmed_production_glb,
        }))
    }

    /// Returns the frozen-plan and original-result identity associated with an
    /// immutable asset version.  Undo/redo descendants copy this lightweight
    /// lineage relation; source bytes, analysis and GLB hashes remain owned by
    /// their existing tables/object references.
    pub fn reference_rebuild_result_lineage(
        &self,
        asset_version_id: &str,
    ) -> CoreResult<Option<(String, String)>> {
        let connection = open_connection(self.db_path())?;
        connection
            .query_row(
                "SELECT rebuild_plan_id, source_result_asset_version_id FROM reference_rebuild_result_lineage WHERE asset_version_id=?",
                [asset_version_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(Into::into)
    }

    /// Links a normal ChangeSet preview to an R007 plan.  Initial-candidate
    /// plans (base=None) deliberately cannot use this method until the caller
    /// has created a normal editable candidate/base through C105.
    pub fn attach_reference_rebuild_preview(
        &self,
        rebuild_plan_id: &str,
        change_set_id: &str,
        updated_at: &str,
    ) -> CoreResult<ReferenceGuidedRebuildPlan> {
        self.write(|transaction| {
            let plan = require_reference_rebuild_plan(transaction, rebuild_plan_id)?;
            if plan.status != ReferenceGuidedRebuildPlanStatus::Draft { return Err(CoreError::conflict("REFERENCE_REBUILD_PLAN_NOT_DRAFT", "Only a draft rebuild plan can receive its first ChangeSet preview.")); }
            let base_id = plan.base_asset_version_id.as_deref().ok_or_else(|| CoreError::conflict("REFERENCE_REBUILD_BASE_REQUIRED", "An initial-candidate plan must first establish an editable base before ChangeSet preview."))?;
            let change_set = require_change_set(transaction, change_set_id)?;
            if change_set.project_id != plan.project_id || change_set.base_asset_version_id != base_id || change_set.status != ChangeSetStatus::Previewed { return Err(CoreError::conflict("REFERENCE_REBUILD_PREVIEW_INVALID", "Reference rebuild preview must be a same-project previewed ChangeSet on the sealed base asset.")); }
            transaction.execute("UPDATE reference_guided_rebuild_plans SET status='previewed', preview_change_set_id=?, updated_at=? WHERE rebuild_plan_id=?", params![change_set_id, updated_at, rebuild_plan_id])?;
            require_reference_rebuild_plan(transaction, rebuild_plan_id)
        })
    }

    pub fn finalize_reference_rebuild_plan(
        &self,
        rebuild_plan_id: &str,
        confirmed: bool,
        updated_at: &str,
    ) -> CoreResult<ReferenceGuidedRebuildPlan> {
        self.write(|transaction| {
            let plan = require_reference_rebuild_plan(transaction, rebuild_plan_id)?;
            if plan.status != ReferenceGuidedRebuildPlanStatus::Previewed { return Err(CoreError::conflict("REFERENCE_REBUILD_PLAN_NOT_PREVIEWED", "Only a previewed rebuild plan can be confirmed or rejected.")); }
            let change_set = require_change_set(transaction, plan.preview_change_set_id.as_deref().expect("validated preview link"))?;
            if change_set.project_id != plan.project_id { return Err(CoreError::conflict("REFERENCE_REBUILD_PROJECT_MISMATCH", "Reference rebuild ChangeSet crossed Project boundaries.")); }
            if confirmed {
                let resulting_asset_version_id = change_set.resulting_asset_version_id.as_deref().ok_or_else(|| CoreError::conflict("REFERENCE_REBUILD_CONFIRM_REQUIRED", "Reference rebuild cannot be marked confirmed before normal ChangeSet confirm creates a new asset."))?;
                if change_set.status != ChangeSetStatus::Confirmed { return Err(CoreError::conflict("REFERENCE_REBUILD_CONFIRM_REQUIRED", "Reference rebuild confirmation must follow normal ChangeSet confirm.")); }
                let result = require_version(transaction, resulting_asset_version_id)?;
                if result.project_id != plan.project_id || crate::is_external_glb_reference(&result) { return Err(CoreError::conflict("REFERENCE_REBUILD_RESULT_INVALID", "Reference rebuild must point to a new same-project ForgeCAD asset, never the imported source.")); }
                transaction.execute("UPDATE reference_guided_rebuild_plans SET status='confirmed', confirmed_asset_version_id=?, updated_at=? WHERE rebuild_plan_id=?", params![resulting_asset_version_id, updated_at, rebuild_plan_id])?;
            } else {
                if change_set.status != ChangeSetStatus::Rejected { return Err(CoreError::conflict("REFERENCE_REBUILD_REJECT_REQUIRED", "Reference rebuild rejection must follow normal ChangeSet rejection.")); }
                transaction.execute("UPDATE reference_guided_rebuild_plans SET status='rejected', updated_at=? WHERE rebuild_plan_id=?", params![updated_at, rebuild_plan_id])?;
            }
            require_reference_rebuild_plan(transaction, rebuild_plan_id)
        })
    }

    pub fn insert_project(&self, project: &Project) -> CoreResult<()> {
        project.validate()?;
        self.write(|transaction| {
            transaction.execute(
                "INSERT INTO projects(project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    project.project_id,
                    project.profile_id,
                    project.domain_type,
                    project.name,
                    project.status.as_str(),
                    project.current_version_id,
                    project.created_at,
                    project.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn create_project(&self, project: &Project) -> CoreResult<Project> {
        self.insert_project(project)?;
        self.project(&project.project_id)?
            .ok_or_else(|| CoreError::not_found("Project"))
    }

    pub fn list_projects(&self, include_archived: bool, limit: u16) -> CoreResult<Vec<Project>> {
        if limit == 0 || limit > 200 {
            return Err(CoreError::invalid_data(
                "PROJECT_LIST_LIMIT_INVALID",
                "Project list limit must be between 1 and 200.",
            ));
        }
        let connection = open_connection(self.db_path())?;
        let sql = if include_archived {
            "SELECT project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at FROM projects WHERE status!='soft_deleted' ORDER BY updated_at DESC, project_id DESC LIMIT ?"
        } else {
            "SELECT project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at FROM projects WHERE status='active' ORDER BY updated_at DESC, project_id DESC LIMIT ?"
        };
        let mut statement = connection.prepare(sql)?;
        let projects = statement
            .query_map([limit], project_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
    }

    /// Persists an internal V003 candidate while keeping its GLB bytes out of
    /// SQLite. The legacy `glb_base64` column is written as an empty
    /// compatibility sentinel; SHA-identified bytes live only in CAS.
    pub fn create_candidate(
        &self,
        mut candidate: BlockoutCandidate,
        glb_bytes: &[u8],
    ) -> CoreResult<BlockoutCandidate> {
        if candidate.status != CandidateStatus::Candidate {
            return Err(CoreError::invalid_data(
                "BLOCKOUT_CANDIDATE_STATE_INVALID",
                "A new blockout candidate must start in candidate state.",
            ));
        }
        let mut promoted = self.object_store.stage(glb_bytes, "glb")?.promote()?;
        candidate.glb_sha256 = promoted.metadata().sha256.clone();
        if let Err(error) = candidate.validate() {
            promoted.cleanup_after_rollback();
            return Err(error);
        }
        let stored = promoted.metadata().clone();
        let result = self.write(|transaction| {
            insert_object_metadata(transaction, &stored, &candidate.created_at)?;
            transaction.execute(
                "INSERT INTO agent_blockout_candidates(artifact_id, project_id, plan_id, direction_id, domain_pack_id, status, candidate_json, shape_program_json, assembly_graph_json, material_bindings_json, glb_base64, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'candidate', ?, ?, ?, ?, '', ?, ?)",
                params![
                    candidate.artifact_id,
                    candidate.project_id,
                    candidate.plan_id,
                    candidate.direction_id,
                    candidate.domain_pack_id,
                    json_text(&candidate.candidate)?,
                    json_text(&candidate.shape_program)?,
                    json_text(&candidate.assembly_graph)?,
                    json_text(&candidate.material_bindings)?,
                    candidate.created_at,
                    candidate.updated_at,
                ],
            )?;
            transaction.execute(
                "INSERT INTO forgecad_core_candidate_objects(artifact_id, glb_sha256, created_at) VALUES (?, ?, ?)",
                params![candidate.artifact_id, candidate.glb_sha256, candidate.created_at],
            )?;
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('candidate', ?, 'production_glb', ?, ?)",
                params![candidate.artifact_id, candidate.glb_sha256, candidate.created_at],
            )?;
            require_candidate(transaction, &candidate.artifact_id)
        });
        match result {
            Ok(candidate) => {
                promoted.finalize_commit()?;
                Ok(candidate)
            }
            Err(error) => {
                promoted.cleanup_after_rollback();
                Err(error)
            }
        }
    }

    pub fn candidate(&self, artifact_id: &str) -> CoreResult<Option<BlockoutCandidate>> {
        let connection = open_connection(self.db_path())?;
        candidate_from_connection(&connection, artifact_id)
    }

    /// Reads the initial candidate promotion only when every authoritative
    /// component is present and mutually consistent. A partially persisted
    /// legacy/multi-transaction attempt is a conflict, never a successful
    /// replay.
    pub fn read_candidate_bundle(
        &self,
        artifact_id: &str,
        asset_version_id: &str,
        quality_report_id: &str,
    ) -> CoreResult<Option<CandidateBundleReadback>> {
        let connection = open_connection(self.db_path())?;
        let Some(bundle) = candidate_bundle_from_connection(
            &connection,
            artifact_id,
            asset_version_id,
            quality_report_id,
        )?
        else {
            return Ok(None);
        };
        self.validate_candidate_bundle_objects(&bundle)?;
        Ok(Some(bundle))
    }

    /// Revalidates a previously returned bundle against current SQLite and
    /// content-addressed bytes. This is suitable for packaged probes and
    /// idempotent route replay checks.
    pub fn validate_candidate_bundle(&self, bundle: &CandidateBundleReadback) -> CoreResult<()> {
        let current = self
            .read_candidate_bundle(
                &bundle.candidate.artifact_id,
                &bundle.version.asset_version_id,
                &bundle.quality.quality_report_id,
            )?
            .ok_or_else(|| candidate_bundle_incomplete(bundle_ids(bundle), vec!["bundle"]))?;
        if current != *bundle {
            return Err(CoreError::conflict_with_details(
                "CANDIDATE_BUNDLE_READBACK_DRIFT",
                "Candidate bundle no longer matches its authoritative readback.",
                bundle_ids(bundle),
            ));
        }
        Ok(())
    }

    /// Atomically promotes a selected candidate into its complete initial
    /// product state. Both GLBs are staged/promoted before one SQLite IMMEDIATE
    /// transaction writes candidate status, v1/head/Snapshot, both object
    /// roles and passed quality. Any pre-commit failure removes newly promoted
    /// files and leaves no partial database state.
    pub fn commit_candidate_bundle(
        &self,
        mut candidate: BlockoutCandidate,
        production_glb: &[u8],
        interactive_preview_glb: &[u8],
        version: &AgentAssetVersion,
        quality: &QualityReport,
    ) -> CoreResult<CandidateBundleReadback> {
        ensure_canonical_shape_program(
            &version.shape_program,
            "CANDIDATE_NON_CANONICAL_SHAPE_PROGRAM",
        )?;
        validate_glb_container(production_glb)?;
        validate_glb_container(interactive_preview_glb)?;

        // A complete pre-existing bundle must also have readable CAS bytes.
        // This check runs before staging so missing/corrupt bytes cannot be
        // silently repaired and mistaken for an idempotent success. A bundle
        // that wins concurrently after this point is a valid replay.
        let _ = self.read_candidate_bundle(
            &candidate.artifact_id,
            &version.asset_version_id,
            &quality.quality_report_id,
        )?;

        let requested_production_sha = candidate.glb_sha256.clone();
        let mut production = self.object_store.stage(production_glb, "glb")?.promote()?;
        let mut interactive = match self
            .object_store
            .stage(interactive_preview_glb, "glb")
            .and_then(|staged| staged.promote())
        {
            Ok(promoted) => promoted,
            Err(error) => {
                production.cleanup_after_rollback();
                return Err(error);
            }
        };
        let production_stored = production.metadata().clone();
        let interactive_stored = interactive.metadata().clone();
        candidate.glb_sha256 = production_stored.sha256.clone();

        if !requested_production_sha.is_empty()
            && requested_production_sha != production_stored.sha256
        {
            interactive.cleanup_after_rollback();
            production.cleanup_after_rollback();
            return Err(CoreError::conflict(
                "CANDIDATE_BUNDLE_IDEMPOTENCY_CONFLICT",
                "Candidate production GLB hash differs from the promoted bytes.",
            ));
        }
        if let Err(error) = validate_candidate_bundle_input(
            &candidate,
            version,
            quality,
            &production_stored,
            &interactive_stored,
        ) {
            interactive.cleanup_after_rollback();
            production.cleanup_after_rollback();
            return Err(error);
        }

        let result = self.write(|transaction| {
            if let Some(existing) = candidate_bundle_from_connection(
                transaction,
                &candidate.artifact_id,
                &version.asset_version_id,
                &quality.quality_report_id,
            )? {
                validate_candidate_bundle_replay(
                    &existing,
                    &candidate,
                    version,
                    quality,
                    &production_stored,
                    &interactive_stored,
                )?;
                return Ok(existing);
            }

            let project_id = candidate.project_id.as_deref().ok_or_else(|| {
                CoreError::invalid_data(
                    "CANDIDATE_BUNDLE_PROJECT_REQUIRED",
                    "Candidate bundle must bind a Project before commit.",
                )
            })?;
            let project_exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id=?)",
                [project_id],
                |row| row.get(0),
            )?;
            if !project_exists {
                return Err(CoreError::not_found("Project"));
            }
            let activation = candidate_bundle_activation(transaction, project_id)?;

            insert_object_metadata(transaction, &production_stored, &candidate.created_at)?;
            insert_object_metadata(transaction, &interactive_stored, &candidate.created_at)?;
            transaction.execute(
                "INSERT INTO agent_blockout_candidates(artifact_id, project_id, plan_id, direction_id, domain_pack_id, status, candidate_json, shape_program_json, assembly_graph_json, material_bindings_json, glb_base64, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 'candidate', ?, ?, ?, ?, '', ?, ?)",
                params![
                    candidate.artifact_id,
                    candidate.project_id,
                    candidate.plan_id,
                    candidate.direction_id,
                    candidate.domain_pack_id,
                    json_text(&candidate.candidate)?,
                    json_text(&candidate.shape_program)?,
                    json_text(&candidate.assembly_graph)?,
                    json_text(&candidate.material_bindings)?,
                    candidate.created_at,
                    candidate.updated_at,
                ],
            )?;
            transaction.execute(
                "INSERT INTO forgecad_core_candidate_objects(artifact_id, glb_sha256, created_at) VALUES (?, ?, ?)",
                params![candidate.artifact_id, production_stored.sha256, candidate.created_at],
            )?;
            insert_version(transaction, version)?;
            transaction.execute(
                "INSERT INTO agent_asset_heads(project_id, asset_version_id, updated_at) VALUES (?, ?, ?)",
                params![version.project_id, version.asset_version_id, version.created_at],
            )?;
            if matches!(&activation, CandidateBundleActivation::FreshProject) {
                let snapshot = initial_snapshot(version)?;
                insert_snapshot(transaction, &snapshot)?;
            }
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('asset_version', ?, 'production_glb', ?, ?)",
                params![version.asset_version_id, production_stored.sha256, version.created_at],
            )?;
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('asset_version', ?, 'interactive_preview_glb', ?, ?)",
                params![version.asset_version_id, interactive_stored.sha256, version.created_at],
            )?;
            let changed = transaction.execute(
                "UPDATE agent_blockout_candidates SET status='committed', updated_at=? WHERE artifact_id=? AND status='candidate'",
                params![version.created_at, candidate.artifact_id],
            )?;
            if changed != 1 {
                return Err(candidate_bundle_incomplete(
                    bundle_input_ids(&candidate, version, quality),
                    vec!["candidate_status"],
                ));
            }
            transaction.execute(
                "INSERT INTO agent_asset_quality_reports(quality_report_id, project_id, asset_version_id, report_json, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    quality.quality_report_id,
                    quality.project_id,
                    quality.asset_version_id,
                    json_text(&quality.report)?,
                    quality.status.as_str(),
                    quality.created_at,
                ],
            )?;
            let changed = match &activation {
                CandidateBundleActivation::FreshProject => transaction.execute(
                    "UPDATE active_design_snapshots SET quality_report_id=?, quality_asset_version_id=?, revision=revision+1, updated_at=? WHERE project_id=? AND source='agent_asset' AND revision=1",
                    params![
                        quality.quality_report_id,
                        quality.asset_version_id,
                        quality.created_at,
                        quality.project_id,
                    ],
                )?,
                CandidateBundleActivation::AuthorizedLegacy(intent) => {
                    promote_authorized_legacy_snapshot(
                        transaction,
                        intent,
                        version,
                        quality,
                    )?;
                    1
                }
            };
            if changed != 1 {
                return Err(candidate_bundle_incomplete(
                    bundle_input_ids(&candidate, version, quality),
                    vec!["snapshot_quality"],
                ));
            }
            candidate_bundle_from_connection(
                transaction,
                &candidate.artifact_id,
                &version.asset_version_id,
                &quality.quality_report_id,
            )?
            .ok_or_else(|| {
                candidate_bundle_incomplete(
                    bundle_input_ids(&candidate, version, quality),
                    vec!["transaction_readback"],
                )
            })
        });

        match result {
            Ok(bundle) => {
                let production_finalize = production.finalize_commit();
                let interactive_finalize = interactive.finalize_commit();
                if production_finalize.is_err() || interactive_finalize.is_err() {
                    // SQLite already committed. Recovery verifies the indexed
                    // SHA before removing only the pending journal, so a
                    // caller never receives a false failure after commit.
                    self.recover_object_store()?;
                }
                self.validate_candidate_bundle_objects(&bundle)?;
                Ok(bundle)
            }
            Err(error) => {
                interactive.cleanup_after_rollback();
                production.cleanup_after_rollback();
                Err(error)
            }
        }
    }

    fn validate_candidate_bundle_objects(
        &self,
        bundle: &CandidateBundleReadback,
    ) -> CoreResult<()> {
        let production = self
            .object_store
            .read(&stored_from_record(&bundle.production_glb))
            .and_then(|bytes| {
                validate_glb_container(&bytes)?;
                Ok(bytes)
            });
        let interactive = self
            .object_store
            .read(&stored_from_record(&bundle.interactive_preview_glb))
            .and_then(|bytes| {
                validate_glb_container(&bytes)?;
                Ok(bytes)
            });
        if production.is_err() || interactive.is_err() {
            return Err(candidate_bundle_incomplete(
                bundle_ids(bundle),
                vec!["content_addressed_bytes"],
            ));
        }
        Ok(())
    }

    /// Selects the single winning candidate and atomically commits the first
    /// immutable version, head, Snapshot and production-GLB object binding.
    pub fn commit_candidate(
        &self,
        artifact_id: &str,
        version: &AgentAssetVersion,
    ) -> CoreResult<(BlockoutCandidate, AgentAssetVersion, ActiveDesignSnapshot)> {
        ensure_canonical_shape_program(
            &version.shape_program,
            "CANDIDATE_NON_CANONICAL_SHAPE_PROGRAM",
        )?;
        version.validate()?;
        if version.status != AssetVersionStatus::Committed
            || version.version_no != 1
            || version.parent_asset_version_id.is_some()
            || version.artifact_id != artifact_id
        {
            return Err(CoreError::invalid_data(
                "BLOCKOUT_CANDIDATE_COMMIT_INVALID",
                "Candidate commit requires immutable version 1 with matching artifact identity.",
            ));
        }
        self.write(|transaction| {
            let candidate = require_candidate(transaction, artifact_id)?;
            let project_id = candidate.project_id.as_deref().ok_or_else(|| {
                CoreError::conflict(
                    "BLOCKOUT_CANDIDATE_PROJECT_REQUIRED",
                    "A candidate must bind a Project before commit.",
                )
            })?;
            if candidate.status != CandidateStatus::Candidate
                || project_id != version.project_id
                || candidate.plan_id != version.plan_id
                || candidate.direction_id != version.direction_id
                || candidate.domain_pack_id != version.domain_pack_id
                || candidate.shape_program != version.shape_program
                || candidate.assembly_graph != version.assembly_graph
                || candidate.material_bindings != version.material_bindings
            {
                return Err(CoreError::conflict(
                    "BLOCKOUT_CANDIDATE_IDENTITY_DRIFT",
                    "Committed version must preserve the selected candidate's sealed geometry identity.",
                ));
            }
            let has_state: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM agent_asset_heads WHERE project_id=? UNION ALL SELECT 1 FROM active_design_snapshots WHERE project_id=?)",
                params![project_id, project_id],
                |row| row.get(0),
            )?;
            if has_state {
                return Err(CoreError::conflict(
                    "BLOCKOUT_CANDIDATE_PROJECT_ALREADY_INITIALIZED",
                    "Candidate commit cannot replace an existing authoritative design head.",
                ));
            }
            let glb_exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM forgecad_core_objects WHERE sha256=?)",
                [&candidate.glb_sha256],
                |row| row.get(0),
            )?;
            if !glb_exists {
                return Err(CoreError::conflict(
                    "BLOCKOUT_CANDIDATE_GLB_MISSING",
                    "Candidate production GLB is missing from CAS.",
                ));
            }
            insert_version(transaction, version)?;
            transaction.execute(
                "INSERT INTO agent_asset_heads(project_id, asset_version_id, updated_at) VALUES (?, ?, ?)",
                params![version.project_id, version.asset_version_id, version.created_at],
            )?;
            let snapshot = initial_snapshot(version)?;
            insert_snapshot(transaction, &snapshot)?;
            transaction.execute(
                "DELETE FROM forgecad_core_object_references WHERE reference_kind='candidate' AND owner_id=? AND role='production_glb'",
                [artifact_id],
            )?;
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('asset_version', ?, 'production_glb', ?, ?)",
                params![version.asset_version_id, candidate.glb_sha256, version.created_at],
            )?;
            transaction.execute(
                "UPDATE agent_blockout_candidates SET status='committed', updated_at=? WHERE artifact_id=? AND status='candidate'",
                params![version.created_at, artifact_id],
            )?;
            Ok((
                require_candidate(transaction, artifact_id)?,
                require_version(transaction, &version.asset_version_id)?,
                require_snapshot(transaction, &version.project_id)?,
            ))
        })
    }

    /// Atomically commits the first immutable Agent asset, head and Snapshot.
    pub fn commit_initial_asset(
        &self,
        version: &AgentAssetVersion,
    ) -> CoreResult<ActiveDesignSnapshot> {
        ensure_canonical_shape_program(
            &version.shape_program,
            "INITIAL_ASSET_NON_CANONICAL_SHAPE_PROGRAM",
        )?;
        version.validate()?;
        if version.parent_asset_version_id.is_some()
            || version.version_no != 1
            || version.status != AssetVersionStatus::Committed
        {
            return Err(CoreError::invalid_data(
                "INITIAL_ASSET_VERSION_INVALID",
                "Initial Agent asset must be committed version 1 without a parent.",
            ));
        }
        self.write(|transaction| {
            insert_version(transaction, version)?;
            transaction.execute(
                "INSERT INTO agent_asset_heads(project_id, asset_version_id, updated_at) VALUES (?, ?, ?)",
                params![version.project_id, version.asset_version_id, version.created_at],
            )?;
            let snapshot = initial_snapshot(version)?;
            insert_snapshot(transaction, &snapshot)?;
            Ok(snapshot)
        })
    }

    pub fn project(&self, project_id: &str) -> CoreResult<Option<Project>> {
        let connection = open_connection(self.db_path())?;
        connection
            .query_row(
                "SELECT project_id, profile_id, domain_type, name, status, current_version_id, created_at, updated_at FROM projects WHERE project_id=?",
                [project_id],
                project_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn snapshot(&self, project_id: &str) -> CoreResult<Option<ActiveDesignSnapshot>> {
        let connection = open_connection(self.db_path())?;
        snapshot_from_connection(&connection, project_id)
    }

    /// Records the explicit S007 hand-off from one read-only legacy Snapshot
    /// revision to a future Agent rebuild. The write creates no Agent version,
    /// head, geometry object or quality record. If an older Project has not
    /// materialized its Snapshot row yet, the validated current legacy source
    /// is sealed as revision 1 in the same idempotent transaction.
    pub fn authorize_legacy_conversion_idempotent(
        &self,
        project_id: &str,
        expected: SnapshotEtag,
        requested_at: &str,
        idempotency_scope: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> CoreResult<LegacyActiveDesignConversionResponse> {
        self.idempotent_write(
            idempotency_scope,
            idempotency_key,
            request_hash,
            requested_at,
            |transaction| {
                let snapshot = match snapshot_from_connection(transaction, project_id)? {
                    Some(snapshot) => snapshot,
                    None => bootstrap_legacy_snapshot(
                        transaction,
                        project_id,
                        expected,
                        requested_at,
                    )?,
                };
                if snapshot.revision != expected.0 {
                    return Err(stale("ACTIVE_DESIGN_STALE"));
                }
                let (legacy_version_id, module_graph_id) = match &snapshot.active_design {
                    ActiveDesign::LegacyConceptReadOnly {
                        legacy_version_id,
                        module_graph_id,
                        ..
                    } => (legacy_version_id, module_graph_id),
                    ActiveDesign::AgentAsset { .. } => {
                        return Err(CoreError::conflict(
                            "ACTIVE_DESIGN_NOT_LEGACY",
                            "The active design is already an Agent asset and does not require legacy conversion.",
                        ))
                    }
                };
                require_legacy_source_binding(
                    transaction,
                    project_id,
                    legacy_version_id,
                    module_graph_id,
                )?;
                transaction.execute(
                    "INSERT INTO legacy_agent_conversion_intents(project_id, legacy_version_id, legacy_module_graph_id, snapshot_revision, requested_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(project_id) DO UPDATE SET legacy_version_id=excluded.legacy_version_id, legacy_module_graph_id=excluded.legacy_module_graph_id, snapshot_revision=excluded.snapshot_revision, requested_at=excluded.requested_at",
                    params![
                        project_id,
                        legacy_version_id,
                        module_graph_id,
                        snapshot.revision,
                        requested_at,
                    ],
                )?;
                let response = LegacyActiveDesignConversionResponse::ready(
                    LegacyActiveDesignSource::new(
                        project_id,
                        legacy_version_id,
                        module_graph_id,
                    ),
                    snapshot.revision,
                )?;
                response.validate()?;
                Ok(response)
            },
        )
    }

    pub fn legacy_conversion_intent(
        &self,
        project_id: &str,
    ) -> CoreResult<Option<LegacyAgentConversionIntent>> {
        let connection = open_connection(self.db_path())?;
        legacy_conversion_intent_from_connection(&connection, project_id)
    }

    pub fn navigation_availability(&self, project_id: &str) -> CoreResult<NavigationAvailability> {
        let connection = open_connection(self.db_path())?;
        let snapshot = snapshot_from_connection(&connection, project_id)?
            .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?;
        let Some(version_id) = snapshot.active_design.asset_version_id() else {
            return Ok(NavigationAvailability {
                project_id: project_id.to_string(),
                active_asset_version_id: None,
                can_undo: false,
                can_redo: false,
                preview_pending: false,
            });
        };
        let version = require_version(&connection, version_id)?;
        let (undo, redo) = navigation_targets(&connection, &version)?;
        let preview_pending = snapshot.preview.is_some();
        Ok(NavigationAvailability {
            project_id: project_id.to_string(),
            active_asset_version_id: Some(version_id.to_string()),
            can_undo: undo.is_some() && !preview_pending,
            can_redo: redo.is_some() && !preview_pending,
            preview_pending,
        })
    }

    pub fn version(&self, asset_version_id: &str) -> CoreResult<Option<AgentAssetVersion>> {
        let connection = open_connection(self.db_path())?;
        version_from_connection(&connection, asset_version_id)
    }

    pub fn head(&self, project_id: &str) -> CoreResult<Option<String>> {
        let connection = open_connection(self.db_path())?;
        connection
            .query_row(
                "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
                [project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    /// Expands one reviewed Component Recipe without creating product state.
    ///
    /// Initial candidates deliberately have no Project/Snapshot authority.
    /// Active-asset candidates are checked before and after the deterministic
    /// expansion so a concurrent head or Snapshot change cannot be smuggled
    /// into a later preview. Persisting a ChangeSet remains a separate,
    /// explicit preview -> confirm workflow.
    pub fn instantiate_component_recipe_candidate(
        &self,
        request: &RecipeInstantiationRequest,
    ) -> CoreResult<ExpandedComponentCandidate> {
        if request.context_mode == "initial_candidate" {
            let registry = recipe_registry_for_repository_request(request, None)?;
            return RecipeExpander::expand(&registry, request, &RecipeExpansionPolicy::default());
        }

        let before = self.require_component_recipe_context(request)?;
        let base_asset_version_id = request
            .base_asset_version_id
            .as_deref()
            .expect("validated context");
        let version = self
            .version(base_asset_version_id)?
            .ok_or_else(|| CoreError::not_found("AgentAssetVersion"))?;
        let registry = recipe_registry_for_repository_request(request, Some(&version))?;
        let mut candidate =
            RecipeExpander::expand(&registry, request, &RecipeExpansionPolicy::default())?;
        self.place_active_component_recipe_candidate(&mut candidate, request, &[])?;
        let after = self.require_component_recipe_context(request)?;
        if before != after {
            return Err(stale("COMPONENT_RECIPE_CONTEXT_STALE"));
        }
        Ok(candidate)
    }

    /// Active Recipe candidates are not merely templates at the origin.  The
    /// immutable base Version provides the target Part's translation and
    /// parent anchor, which Rust bakes into every generated operation before
    /// assigning the candidate identity.  The bridge is consequently never
    /// allowed to translate a candidate after its SHA has been sealed.
    fn place_active_component_recipe_candidate(
        &self,
        candidate: &mut ExpandedComponentCandidate,
        request: &RecipeInstantiationRequest,
        protected_part_ids: &[String],
    ) -> CoreResult<()> {
        let project_id = request.project_id.as_deref().ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_CONTEXT_INVALID",
                "Active Recipe placement requires a Project identity.",
            )
        })?;
        let base_asset_version_id = request.base_asset_version_id.as_deref().ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_CONTEXT_INVALID",
                "Active Recipe placement requires a base asset version.",
            )
        })?;
        let expected_revision = request.snapshot_revision.ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_CONTEXT_INVALID",
                "Active Recipe placement requires a Snapshot revision.",
            )
        })?;
        let connection = open_connection(self.db_path())?;
        let snapshot = require_snapshot(&connection, project_id)?;
        let version = require_version(&connection, base_asset_version_id)?;
        if snapshot.revision != expected_revision
            || snapshot.active_design.asset_version_id() != Some(base_asset_version_id)
            || version.project_id != project_id
        {
            return Err(stale("COMPONENT_RECIPE_CONTEXT_STALE"));
        }
        place_recipe_candidate_in_active_context(
            candidate,
            request,
            &version,
            &snapshot,
            protected_part_ids,
        )
    }

    /// Recreates the transient reviewed Recipe candidate sealed into a
    /// `replace_part` ChangeSet operation.  The operation stores only a small
    /// immutable reference, never an in-memory GLB or an expanded mesh.  This
    /// makes a restart deterministic: the current active Project/Version and
    /// Snapshot are checked again before the Recipe engine is allowed to
    /// expand it.
    pub fn recipe_replacement_candidate(
        &self,
        change_set: &AgentAssetChangeSet,
        operation: &serde_json::Value,
    ) -> CoreResult<ExpandedComponentCandidate> {
        let connection = open_connection(self.db_path())?;
        let snapshot = require_snapshot(&connection, &change_set.project_id)?;
        if snapshot.preview.is_some() {
            return Err(CoreError::conflict(
                "COMPONENT_RECIPE_PREVIEW_PENDING",
                "A Recipe replacement cannot be expanded while another preview is pending.",
            ));
        }
        let head: String = connection
            .query_row(
                "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
                [&change_set.project_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| CoreError::not_found("Agent asset head"))?;
        if snapshot.active_design.asset_version_id()
            != Some(change_set.base_asset_version_id.as_str())
            || head != change_set.base_asset_version_id
        {
            return Err(stale("CHANGE_SET_BASE_STALE"));
        }
        let version = require_version(&connection, &change_set.base_asset_version_id)?;
        require_internal_editable_asset(&connection, &version)?;
        recipe_replacement_candidate_from_context(
            &connection,
            &version,
            &snapshot,
            change_set,
            operation,
        )
    }

    fn require_component_recipe_context(
        &self,
        request: &RecipeInstantiationRequest,
    ) -> CoreResult<(u64, String, String)> {
        let project_id = request.project_id.as_deref().ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_CONTEXT_INVALID",
                "Active Recipe expansion requires a Project identity.",
            )
        })?;
        let base_asset_version_id = request.base_asset_version_id.as_deref().ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_CONTEXT_INVALID",
                "Active Recipe expansion requires a base asset version.",
            )
        })?;
        let expected_revision = request.snapshot_revision.ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_CONTEXT_INVALID",
                "Active Recipe expansion requires a Snapshot revision.",
            )
        })?;
        let connection = open_connection(self.db_path())?;
        let snapshot = require_snapshot(&connection, project_id)?;
        let head: String = connection
            .query_row(
                "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
                [project_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| CoreError::not_found("Agent asset head"))?;
        if snapshot.revision != expected_revision
            || snapshot.active_design.asset_version_id() != Some(base_asset_version_id)
            || head != base_asset_version_id
        {
            return Err(stale("COMPONENT_RECIPE_CONTEXT_STALE"));
        }
        if snapshot.preview.is_some() {
            return Err(CoreError::conflict(
                "COMPONENT_RECIPE_PREVIEW_PENDING",
                "A Recipe candidate cannot be based on a Snapshot with a pending preview.",
            ));
        }
        let version = require_version(&connection, base_asset_version_id)?;
        require_internal_editable_asset(&connection, &version)?;
        if version.project_id != project_id {
            return Err(CoreError::conflict(
                "COMPONENT_RECIPE_PROJECT_MISMATCH",
                "Recipe base version does not belong to the requested Project.",
            ));
        }
        if version.domain_pack_id != request.domain_pack_id {
            return Err(CoreError::conflict(
                "COMPONENT_RECIPE_DOMAIN_INCOMPATIBLE",
                "Recipe domain does not match the active Agent asset.",
            ));
        }
        if let Some(target_part_id) = request.target_part_id.as_deref() {
            let part = version
                .parts
                .iter()
                .find(|part| {
                    part.get("part_id").and_then(serde_json::Value::as_str) == Some(target_part_id)
                })
                .ok_or_else(|| CoreError::not_found("Agent asset part"))?;
            let snapshot_locked = snapshot.part_display.as_ref().is_some_and(|display| {
                display
                    .locked_part_ids
                    .iter()
                    .any(|part_id| part_id == target_part_id)
            });
            if snapshot_locked
                || part.get("locked").and_then(serde_json::Value::as_bool) == Some(true)
            {
                return Err(CoreError::conflict(
                    "PART_PROTECTED",
                    "Locked parts cannot be replaced by a Component Recipe candidate.",
                ));
            }
        }
        Ok((snapshot.revision, head, version.asset_version_id))
    }

    pub fn change_set(&self, change_set_id: &str) -> CoreResult<Option<AgentAssetChangeSet>> {
        let connection = open_connection(self.db_path())?;
        change_set_from_connection(&connection, change_set_id)
    }

    pub fn quality_report(&self, quality_report_id: &str) -> CoreResult<Option<QualityReport>> {
        let connection = open_connection(self.db_path())?;
        quality_from_connection(&connection, quality_report_id)
    }

    pub fn select(
        &self,
        project_id: &str,
        expected: SnapshotEtag,
        part_id: Option<&str>,
        material_zone_id: Option<&str>,
        updated_at: &str,
    ) -> CoreResult<ActiveDesignSnapshot> {
        self.write(|transaction| {
            let snapshot = require_agent_snapshot(transaction, project_id, expected)?;
            let version = require_active_version(transaction, &snapshot)?;
            validate_selection(&version, part_id, material_zone_id)?;
            let changed = transaction.execute(
                "UPDATE active_design_snapshots SET selected_part_id=?, selected_material_zone_id=?, revision=revision+1, updated_at=? WHERE project_id=? AND source='agent_asset' AND revision=?",
                params![part_id, material_zone_id, updated_at, project_id, expected.0],
            )?;
            require_changed(changed)?;
            require_snapshot(transaction, project_id)
        })
    }

    pub fn select_idempotent(
        &self,
        project_id: &str,
        expected: SnapshotEtag,
        part_id: Option<&str>,
        material_zone_id: Option<&str>,
        updated_at: &str,
        idempotency_scope: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> CoreResult<ActiveDesignSnapshot> {
        self.idempotent_write(
            idempotency_scope,
            idempotency_key,
            request_hash,
            updated_at,
            |transaction| {
                let snapshot = require_agent_snapshot(transaction, project_id, expected)?;
                let version = require_active_version(transaction, &snapshot)?;
                validate_selection(&version, part_id, material_zone_id)?;
                let changed = transaction.execute(
                    "UPDATE active_design_snapshots SET selected_part_id=?, selected_material_zone_id=?, revision=revision+1, updated_at=? WHERE project_id=? AND source='agent_asset' AND revision=?",
                    params![part_id, material_zone_id, updated_at, project_id, expected.0],
                )?;
                require_changed(changed)?;
                require_snapshot(transaction, project_id)
            },
        )
    }

    pub fn set_render_preset_idempotent(
        &self,
        project_id: &str,
        expected: SnapshotEtag,
        camera_view: &str,
        light_preset: &str,
        updated_at: &str,
        idempotency_scope: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> CoreResult<ActiveDesignSnapshot> {
        if !matches!(camera_view, "iso" | "front" | "top" | "right") {
            return Err(CoreError::invalid_data(
                "RENDER_CAMERA_VIEW_INVALID",
                "Render camera view is outside the code-owned visual preset list.",
            ));
        }
        if !matches!(
            light_preset,
            "cad_neutral" | "soft_studio" | "concept_contrast"
        ) {
            return Err(CoreError::invalid_data(
                "RENDER_LIGHT_PRESET_INVALID",
                "Render light preset is outside the code-owned visual preset list.",
            ));
        }
        self.idempotent_write(
            idempotency_scope,
            idempotency_key,
            request_hash,
            updated_at,
            |transaction| {
                let snapshot = require_agent_snapshot(transaction, project_id, expected)?;
                let version = require_active_version(transaction, &snapshot)?;
                let preset = RenderPreset {
                    schema_version: "ActiveDesignRenderPreset@1".into(),
                    preset_id: format!(
                        "render_{}_{}_{}",
                        version.asset_version_id, camera_view, light_preset
                    ),
                    project_id: project_id.to_string(),
                    asset_version_id: version.asset_version_id,
                    camera_view: camera_view.to_string(),
                    light_preset: light_preset.to_string(),
                    updated_at: updated_at.to_string(),
                };
                let changed = transaction.execute(
                    "UPDATE active_design_snapshots SET render_preset_json=?, revision=revision+1, updated_at=? WHERE project_id=? AND source='agent_asset' AND revision=?",
                    params![json_text(&preset)?, updated_at, project_id, expected.0],
                )?;
                require_changed(changed)?;
                require_snapshot(transaction, project_id)
            },
        )
    }

    pub fn set_part_display_idempotent(
        &self,
        project_id: &str,
        expected: SnapshotEtag,
        action: &str,
        part_id: Option<&str>,
        updated_at: &str,
        idempotency_scope: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> CoreResult<ActiveDesignSnapshot> {
        let requires_part = matches!(action, "lock" | "unlock" | "hide" | "show" | "isolate");
        if !matches!(
            action,
            "lock" | "unlock" | "hide" | "show" | "isolate" | "clear_isolation" | "show_all"
        ) || requires_part != part_id.is_some()
        {
            return Err(CoreError::invalid_data(
                "PART_DISPLAY_ACTION_INVALID",
                "Part display action and part identity do not match the bounded contract.",
            ));
        }
        self.idempotent_write(
            idempotency_scope,
            idempotency_key,
            request_hash,
            updated_at,
            |transaction| {
                let snapshot = require_agent_snapshot(transaction, project_id, expected)?;
                if snapshot.preview.is_some() {
                    return Err(CoreError::conflict(
                        "ACTIVE_DESIGN_PREVIEW_PENDING",
                        "Resolve the active preview before changing part display state.",
                    ));
                }
                let version = require_active_version(transaction, &snapshot)?;
                let part_index = version.part_zone_index()?;
                if part_id.is_some_and(|part| !part_index.contains_key(part)) {
                    return Err(CoreError::not_found("Agent asset part"));
                }
                let mut display = snapshot
                    .part_display
                    .clone()
                    .unwrap_or_else(|| PartDisplay::empty(project_id, &version.asset_version_id));
                display.project_id = project_id.to_string();
                display.asset_version_id = version.asset_version_id;
                match action {
                    "lock" => insert_sorted_unique(&mut display.locked_part_ids, part_id.unwrap()),
                    "unlock" => remove_value(&mut display.locked_part_ids, part_id.unwrap()),
                    "hide" => insert_sorted_unique(&mut display.hidden_part_ids, part_id.unwrap()),
                    "show" => remove_value(&mut display.hidden_part_ids, part_id.unwrap()),
                    "isolate" => display.isolated_part_id = part_id.map(str::to_string),
                    "clear_isolation" => display.isolated_part_id = None,
                    "show_all" => {
                        display.hidden_part_ids.clear();
                        display.isolated_part_id = None;
                    }
                    _ => unreachable!("validated part display action"),
                }
                let selection_visible = snapshot.selected_part_id.as_ref().map_or(true, |selected| {
                    !display.hidden_part_ids.iter().any(|hidden| hidden == selected)
                        && display
                            .isolated_part_id
                            .as_ref()
                            .map_or(true, |isolated| isolated == selected)
                });
                let (selected_part_id, selected_material_zone_id) = if selection_visible {
                    (
                        snapshot.selected_part_id.as_deref(),
                        snapshot.selected_material_zone_id.as_deref(),
                    )
                } else {
                    (None, None)
                };
                let changed = transaction.execute(
                    "UPDATE active_design_snapshots SET part_display_json=?, selected_part_id=?, selected_material_zone_id=?, revision=revision+1, updated_at=? WHERE project_id=? AND source='agent_asset' AND revision=?",
                    params![
                        json_text(&display)?,
                        selected_part_id,
                        selected_material_zone_id,
                        updated_at,
                        project_id,
                        expected.0,
                    ],
                )?;
                require_changed(changed)?;
                require_snapshot(transaction, project_id)
            },
        )
    }

    /// Saves one immutable, project-local component snapshot from the current
    /// active Rust-owned asset. Geometry, role and visual bindings are derived
    /// from the authoritative version; clients cannot submit their own
    /// reusable geometry payload.
    #[allow(clippy::too_many_arguments)]
    pub fn save_component_idempotent(
        &self,
        asset_version_id: &str,
        component_id: &str,
        part_id: &str,
        display_name: &str,
        description: &str,
        created_at: &str,
        idempotency_scope: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> CoreResult<AgentComponentRecord> {
        if display_name.is_empty()
            || display_name.chars().count() > 120
            || display_name.contains('\0')
            || description.chars().count() > 500
            || description.contains('\0')
        {
            return Err(CoreError::invalid_data(
                "AGENT_COMPONENT_INPUT_INVALID",
                "Component name or description exceeds the bounded contract.",
            ));
        }
        self.idempotent_write(
            idempotency_scope,
            idempotency_key,
            request_hash,
            created_at,
            |transaction| {
                let version = require_version(transaction, asset_version_id)?;
                let snapshot = require_snapshot(transaction, &version.project_id)?;
                if version.status != AssetVersionStatus::Committed
                    || snapshot.active_design.asset_version_id() != Some(asset_version_id)
                    || require_head(transaction, &version.project_id)? != asset_version_id
                {
                    return Err(CoreError::conflict(
                        "ASSET_VERSION_STALE",
                        "Components may only be saved from the current active editable asset.",
                    ));
                }
                require_internal_editable_asset(transaction, &version)?;
                let (part_template, shape_operation, material_bindings) =
                    derive_component_snapshot(&version, part_id)?;
                let role = part_template
                    .get("role")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        CoreError::invalid_data(
                            "PART_ROLE_INVALID",
                            "Component source part is missing its stable role.",
                        )
                    })?;
                let source_quality_status =
                    source_quality_status_from_connection(transaction, asset_version_id)?;
                let component = AgentComponentRecord {
                    schema_version: "AgentComponent@1".into(),
                    component_id: component_id.to_string(),
                    project_id: version.project_id.clone(),
                    domain_pack_id: version.domain_pack_id.clone(),
                    role: role.to_string(),
                    display_name: display_name.to_string(),
                    description: description.to_string(),
                    source_asset_version_id: version.asset_version_id.clone(),
                    source_part_id: part_id.to_string(),
                    part_template,
                    shape_operation,
                    material_bindings,
                    status: "active".into(),
                    source_quality_status,
                    created_at: created_at.to_string(),
                    updated_at: created_at.to_string(),
                };
                component.validate()?;
                transaction.execute(
                    "INSERT INTO agent_components(component_id, project_id, domain_pack_id, role, display_name, description, source_asset_version_id, source_part_id, part_template_json, shape_operation_json, material_bindings_json, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)",
                    params![
                        component.component_id,
                        component.project_id,
                        component.domain_pack_id,
                        component.role,
                        component.display_name,
                        component.description,
                        component.source_asset_version_id,
                        component.source_part_id,
                        json_text(&component.part_template)?,
                        json_text(&component.shape_operation)?,
                        json_text(&component.material_bindings)?,
                        component.created_at,
                        component.updated_at,
                    ],
                )?;
                component_from_connection(transaction, component_id)?.ok_or_else(|| {
                    CoreError::not_found("newly saved AgentComponent")
                })
            },
        )
    }

    pub fn component(&self, component_id: &str) -> CoreResult<Option<AgentComponentRecord>> {
        let connection = open_connection(self.db_path())?;
        component_from_connection(&connection, component_id)
    }

    pub fn list_components(
        &self,
        project_id: &str,
        domain_pack_id: Option<&str>,
        role: Option<&str>,
        query: Option<&str>,
        include_disabled: bool,
    ) -> CoreResult<Vec<AgentComponentRecord>> {
        let connection = open_connection(self.db_path())?;
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id=? AND status!='soft_deleted')",
                [project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(CoreError::from)?
            .then_some(())
            .ok_or_else(|| CoreError::not_found("Project"))?;
        let needle = query.map(|value| format!("%{value}%"));
        let mut statement = connection.prepare(
            "SELECT component_id FROM agent_components WHERE project_id=? AND (? OR status='active') AND (? IS NULL OR domain_pack_id=?) AND (? IS NULL OR role=?) AND (? IS NULL OR display_name LIKE ? OR description LIKE ? OR role LIKE ?) ORDER BY updated_at DESC, component_id DESC LIMIT 100",
        )?;
        let ids = statement
            .query_map(
                params![
                    project_id,
                    include_disabled,
                    domain_pack_id,
                    domain_pack_id,
                    role,
                    role,
                    needle,
                    needle,
                    needle,
                    needle,
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|component_id| {
                component_from_connection(&connection, &component_id)?
                    .ok_or_else(|| CoreError::not_found("AgentComponent"))
            })
            .collect()
    }

    pub fn component_candidates(
        &self,
        asset_version_id: &str,
        part_id: &str,
    ) -> CoreResult<Vec<AgentComponentCandidate>> {
        let connection = open_connection(self.db_path())?;
        let version = require_version(&connection, asset_version_id)?;
        require_current_editable_asset(&connection, &version)?;
        let target = version
            .parts
            .iter()
            .find(|part| part.get("part_id").and_then(serde_json::Value::as_str) == Some(part_id))
            .ok_or_else(|| CoreError::not_found("Agent asset part"))?;
        let target_role = target
            .get("role")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "PART_ROLE_INVALID",
                    "Target part is missing a stable role.",
                )
            })?;
        let components = self.list_components(&version.project_id, None, None, None, true)?;
        Ok(components
            .into_iter()
            .map(|component| component_candidate(&version, part_id, target_role, component))
            .collect())
    }

    /// Recomputes replacement eligibility for the current asset. This is used
    /// by the geometry preview bridge and never trusts a client-carried
    /// compatibility result.
    pub fn replacement_component(
        &self,
        asset_version_id: &str,
        part_id: &str,
        component_id: &str,
    ) -> CoreResult<AgentComponentRecord> {
        let candidate = self
            .component_candidates(asset_version_id, part_id)?
            .into_iter()
            .find(|candidate| candidate.component.component_id == component_id)
            .ok_or_else(|| CoreError::not_found("AgentComponent"))?;
        require_component_candidate_eligible(&candidate)?;
        Ok(candidate.component)
    }

    pub fn structure_suggestions(
        &self,
        asset_version_id: &str,
    ) -> CoreResult<AgentStructureSuggestionList> {
        let connection = open_connection(self.db_path())?;
        let version = require_version(&connection, asset_version_id)?;
        let snapshot = require_current_editable_asset(&connection, &version)?;
        let suggestions = derive_structure_suggestions(&version, &snapshot)?;
        Ok(AgentStructureSuggestionList {
            schema_version: "AgentStructureSuggestionList@1".into(),
            asset_version_id: version.asset_version_id,
            unavailable_message: suggestions.is_empty().then(|| {
                "当前模型没有足够的装配和几何事实，或相关部件已锁定，暂不能建议拆分或合并部件。"
                    .into()
            }),
            suggestions,
        })
    }

    pub fn verified_structure_suggestion(
        &self,
        asset_version_id: &str,
        suggestion_id: &str,
        kind: &str,
        part_id: &str,
        target_part_id: Option<&str>,
    ) -> CoreResult<AgentStructureSuggestion> {
        self.structure_suggestions(asset_version_id)?
            .suggestions
            .into_iter()
            .find(|suggestion| {
                suggestion.suggestion_id == suggestion_id
                    && suggestion.kind == kind
                    && suggestion.part_id == part_id
                    && suggestion.target_part_id.as_deref() == target_part_id
            })
            .ok_or_else(|| {
                CoreError::conflict(
                    "STRUCTURE_SUGGESTION_NOT_AVAILABLE",
                    "The structure suggestion is stale, forged, locked, or no longer derivable from current facts.",
                )
            })
    }

    pub fn create_change_set(&self, change_set: &AgentAssetChangeSet) -> CoreResult<()> {
        change_set.validate()?;
        if change_set.status != ChangeSetStatus::Proposed
            || change_set.preview.is_some()
            || change_set.resulting_asset_version_id.is_some()
        {
            return Err(CoreError::invalid_data(
                "CHANGE_SET_INITIAL_STATE_INVALID",
                "A new ChangeSet must be proposed without preview or result.",
            ));
        }
        self.write(|transaction| {
            validate_change_set_context(transaction, change_set)?;
            insert_change_set(transaction, change_set)?;
            Ok(())
        })
    }

    pub fn create_change_set_idempotent(
        &self,
        change_set: &AgentAssetChangeSet,
        idempotency_scope: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> CoreResult<AgentAssetChangeSet> {
        change_set.validate()?;
        if change_set.status != ChangeSetStatus::Proposed
            || change_set.preview.is_some()
            || change_set.resulting_asset_version_id.is_some()
        {
            return Err(CoreError::invalid_data(
                "CHANGE_SET_INITIAL_STATE_INVALID",
                "A new ChangeSet must be proposed without preview or result.",
            ));
        }
        self.idempotent_write(
            idempotency_scope,
            idempotency_key,
            request_hash,
            &change_set.created_at,
            |transaction| {
                validate_change_set_context(transaction, change_set)?;
                insert_change_set(transaction, change_set)?;
                require_change_set(transaction, &change_set.change_set_id)
            },
        )
    }

    pub fn read_change_set_preview_bundle(
        &self,
        change_set_id: &str,
    ) -> CoreResult<Option<ChangeSetPreviewBundleReadback>> {
        let connection = open_connection(self.db_path())?;
        let Some(bundle) = change_set_preview_bundle_from_connection(&connection, change_set_id)?
        else {
            return Ok(None);
        };
        self.validate_change_set_preview_bundle_objects(&bundle)?;
        Ok(Some(bundle))
    }

    pub fn validate_change_set_preview_bundle(
        &self,
        bundle: &ChangeSetPreviewBundleReadback,
    ) -> CoreResult<()> {
        let current = self
            .read_change_set_preview_bundle(&bundle.change_set.change_set_id)?
            .ok_or_else(|| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    preview_bundle_ids(&bundle.change_set.change_set_id),
                    vec!["preview_bundle"],
                )
            })?;
        if current != *bundle {
            return Err(CoreError::conflict_with_details(
                "CHANGE_SET_PREVIEW_BUNDLE_READBACK_DRIFT",
                "ChangeSet preview bundle no longer matches its authoritative readback.",
                preview_bundle_ids(&bundle.change_set.change_set_id),
            ));
        }
        Ok(())
    }

    /// Stages a verified interactive GLB and atomically seals preview JSON,
    /// the temporary CAS reference and Snapshot preview revision.
    pub fn preview_change_set_bundle(
        &self,
        change_set_id: &str,
        sealed_preview: &AgentAssetVersion,
        interactive_glb: &[u8],
        interactive_readback: &serde_json::Value,
        expected: SnapshotEtag,
        updated_at: &str,
    ) -> CoreResult<ChangeSetPreviewBundleReadback> {
        let normalized_shape_program =
            crate::normalize_persisted_shape_program(&sealed_preview.shape_program)?;
        if normalized_shape_program != sealed_preview.shape_program {
            return Err(CoreError::conflict(
                "CHANGE_SET_PREVIEW_NON_CANONICAL_SHAPE_PROGRAM",
                "ChangeSet preview must be normalized before geometry compilation and persistence.",
            ));
        }
        let sealed_preview = sealed_preview.clone();
        sealed_preview.validate()?;
        let verified = crate::verify_forgecad_glb(interactive_glb, Some("interactive_preview"))?;
        validate_interactive_readback(interactive_readback, &sealed_preview, &verified)?;

        // Detect incomplete persisted state before staging so missing CAS
        // bytes cannot be silently repaired into a successful replay.
        let _ = self.read_change_set_preview_bundle(change_set_id)?;
        let mut promoted = self.object_store.stage(interactive_glb, "glb")?.promote()?;
        let stored = promoted.metadata().clone();
        let seal = ChangeSetPreviewSeal {
            schema_version: CHANGE_SET_PREVIEW_SEAL_SCHEMA.into(),
            sealed_preview: sealed_preview.clone(),
            interactive_readback: interactive_readback.clone(),
            interactive_glb_sha256: stored.sha256.clone(),
            interactive_glb_byte_size: stored.byte_size,
        };
        let result = self.write(|transaction| {
            if let Some(existing) =
                change_set_preview_bundle_from_connection(transaction, change_set_id)?
            {
                validate_change_set_preview_replay(
                    &existing,
                    &seal,
                    &stored,
                    change_set_id,
                )?;
                return Ok(existing);
            }

            let change_set = require_change_set(transaction, change_set_id)?;
            if change_set.status != ChangeSetStatus::Proposed
                || change_set.preview.is_some()
                || change_set.resulting_asset_version_id.is_some()
            {
                return Err(CoreError::conflict(
                    "CHANGE_SET_PREVIEW_STATE_CONFLICT",
                    "Only a proposed ChangeSet without prior preview state can create a preview bundle.",
                ));
            }
            validate_change_set_preview_identity(transaction, &change_set, &sealed_preview)?;
            let snapshot =
                require_agent_snapshot(transaction, &change_set.project_id, expected)?;
            if snapshot
                .preview
                .as_ref()
                .is_some_and(|active| active.change_set_id != change_set_id)
            {
                return Err(CoreError::conflict(
                    "ACTIVE_DESIGN_PREVIEW_PENDING",
                    "Resolve the active ChangeSet preview before previewing another edit.",
                ));
            }
            if snapshot.active_design.asset_version_id()
                != Some(change_set.base_asset_version_id.as_str())
                || require_head(transaction, &change_set.project_id)?
                    != change_set.base_asset_version_id
            {
                return Err(stale("CHANGE_SET_BASE_STALE"));
            }
            let active_base = require_version(transaction, &change_set.base_asset_version_id)?;
            validate_change_set_operations(transaction, &active_base, &snapshot, &change_set)?;

            insert_object_metadata(transaction, &stored, updated_at)?;
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('preview', ?, 'interactive_preview_glb', ?, ?)",
                params![change_set_id, stored.sha256, updated_at],
            )?;
            transaction.execute(
                "UPDATE agent_asset_change_sets SET preview_json=?, status='previewed', resulting_asset_version_id=NULL, updated_at=? WHERE change_set_id=? AND status='proposed'",
                params![json_text(&seal)?, updated_at, change_set_id],
            )?;
            // Only R007-generated ChangeSets occupy the `changeset_`
            // namespace and have a deterministic rebuild-plan companion.
            // Ordinary historical ChangeSets remain valid bundle inputs and
            // must not be forced through the reference-only ID contract.
            if change_set_id.starts_with("changeset_") {
                let rebuild_plan_id =
                    crate::reference_rebuild_plan_id_for_change_set(change_set_id)?;
                let rebuild_plan: Option<(String, Option<String>, String)> = transaction
                    .query_row(
                        "SELECT status, base_asset_version_id, project_id FROM reference_guided_rebuild_plans WHERE rebuild_plan_id=?",
                        [&rebuild_plan_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .optional()?;
                if let Some((status, base_asset_version_id, project_id)) = rebuild_plan {
                    if status != "draft"
                        || base_asset_version_id.as_deref()
                            != Some(change_set.base_asset_version_id.as_str())
                        || project_id != change_set.project_id
                    {
                        return Err(CoreError::conflict(
                            "REFERENCE_REBUILD_PREVIEW_INVALID",
                            "Reference rebuild plan no longer matches its proposed ChangeSet base and Project.",
                        ));
                    }
                    let linked = transaction.execute(
                        "UPDATE reference_guided_rebuild_plans SET status='previewed', preview_change_set_id=?, updated_at=? WHERE rebuild_plan_id=? AND status='draft'",
                        params![change_set_id, updated_at, rebuild_plan_id],
                    )?;
                    require_changed(linked)?;
                }
            }
            let changed = transaction.execute(
                "UPDATE active_design_snapshots SET preview_change_set_id=?, preview_base_asset_version_id=active_asset_version_id, revision=revision+1, updated_at=? WHERE project_id=? AND revision=?",
                params![change_set_id, updated_at, change_set.project_id, expected.0],
            )?;
            require_changed(changed)?;
            change_set_preview_bundle_from_connection(transaction, change_set_id)?.ok_or_else(|| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    preview_bundle_ids(change_set_id),
                    vec!["transaction_readback"],
                )
            })
        });
        match result {
            Ok(bundle) => {
                if promoted.finalize_commit().is_err() {
                    self.recover_object_store()?;
                }
                self.validate_change_set_preview_bundle_objects(&bundle)?;
                Ok(bundle)
            }
            Err(error) => {
                promoted.cleanup_after_rollback();
                Err(error)
            }
        }
    }

    fn validate_change_set_preview_bundle_objects(
        &self,
        bundle: &ChangeSetPreviewBundleReadback,
    ) -> CoreResult<()> {
        let bytes = self
            .object_store
            .read(&stored_from_record(&bundle.interactive_preview_glb))
            .map_err(|_| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    preview_bundle_ids(&bundle.change_set.change_set_id),
                    vec!["interactive_preview_bytes"],
                )
            })?;
        let verified =
            crate::verify_forgecad_glb(&bytes, Some("interactive_preview")).map_err(|_| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    preview_bundle_ids(&bundle.change_set.change_set_id),
                    vec!["interactive_preview_readback"],
                )
            })?;
        validate_interactive_readback(
            &bundle.interactive_readback,
            &bundle.sealed_preview,
            &verified,
        )
        .map_err(|_| {
            change_set_bundle_incomplete(
                "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                preview_bundle_ids(&bundle.change_set.change_set_id),
                vec!["interactive_readback_metadata"],
            )
        })
    }

    /// Stores a compiled preview and binds it to the same Snapshot revision in
    /// one transaction. It never creates a permanent asset version.
    pub fn preview_change_set(
        &self,
        change_set_id: &str,
        preview: &AgentAssetVersion,
        expected: SnapshotEtag,
        updated_at: &str,
    ) -> CoreResult<(AgentAssetChangeSet, ActiveDesignSnapshot)> {
        preview.validate()?;
        self.write(|transaction| {
            let change_set = require_change_set(transaction, change_set_id)?;
            if !matches!(change_set.status, ChangeSetStatus::Proposed | ChangeSetStatus::Previewed)
                || change_set.project_id != preview.project_id
                || change_set.base_asset_version_id != preview.parent_asset_version_id.as_deref().unwrap_or("")
            {
                return Err(CoreError::conflict(
                    "CHANGE_SET_PREVIEW_INVALID",
                    "Preview identity or ChangeSet state is invalid.",
                ));
            }
            let snapshot = require_agent_snapshot(transaction, &change_set.project_id, expected)?;
            if snapshot
                .preview
                .as_ref()
                .is_some_and(|active| active.change_set_id != change_set_id)
            {
                return Err(CoreError::conflict(
                    "ACTIVE_DESIGN_PREVIEW_PENDING",
                    "Resolve the active ChangeSet preview before previewing another edit.",
                ));
            }
            let active = snapshot.active_design.asset_version_id().unwrap_or_default();
            let head = require_head(transaction, &change_set.project_id)?;
            if active != change_set.base_asset_version_id || head != change_set.base_asset_version_id {
                return Err(stale("CHANGE_SET_BASE_STALE"));
            }
            transaction.execute(
                "UPDATE agent_asset_change_sets SET preview_json=?, status='previewed', resulting_asset_version_id=NULL, updated_at=? WHERE change_set_id=?",
                params![json_text(preview)?, updated_at, change_set_id],
            )?;
            let changed = transaction.execute(
                "UPDATE active_design_snapshots SET preview_change_set_id=?, preview_base_asset_version_id=active_asset_version_id, revision=revision+1, updated_at=? WHERE project_id=? AND revision=?",
                params![change_set_id, updated_at, change_set.project_id, expected.0],
            )?;
            require_changed(changed)?;
            Ok((
                require_change_set(transaction, change_set_id)?,
                require_snapshot(transaction, &change_set.project_id)?,
            ))
        })
    }

    /// Confirms an already compiled preview as a new immutable child version.
    pub fn confirm_change_set(
        &self,
        change_set_id: &str,
        resulting: &AgentAssetVersion,
        expected: SnapshotEtag,
    ) -> CoreResult<(AgentAssetChangeSet, AgentAssetVersion, ActiveDesignSnapshot)> {
        resulting.validate()?;
        if resulting.status != AssetVersionStatus::Committed {
            return Err(CoreError::invalid_data(
                "CHANGE_SET_RESULT_INVALID",
                "Confirmed ChangeSet result must be committed.",
            ));
        }
        self.write(|transaction| {
            let change_set = require_change_set(transaction, change_set_id)?;
            if change_set.status != ChangeSetStatus::Previewed
                || change_set.preview.is_none()
                || change_set.project_id != resulting.project_id
                || resulting.parent_asset_version_id.as_deref()
                    != Some(change_set.base_asset_version_id.as_str())
            {
                return Err(CoreError::conflict(
                    "CHANGE_SET_NOT_PREVIEWED",
                    "ChangeSet has no matching confirmed preview.",
                ));
            }
            let snapshot = require_agent_snapshot(transaction, &change_set.project_id, expected)?;
            if snapshot.preview.as_ref().map(|item| item.change_set_id.as_str()) != Some(change_set_id)
                || snapshot.active_design.asset_version_id()
                    != Some(change_set.base_asset_version_id.as_str())
                || require_head(transaction, &change_set.project_id)? != change_set.base_asset_version_id
            {
                mark_change_set_stale(transaction, change_set_id, &resulting.created_at)?;
                return Err(stale("CHANGE_SET_BASE_STALE"));
            }
            let sealed_preview = change_set
                .preview
                .as_ref()
                .ok_or_else(|| {
                    CoreError::conflict(
                        "CHANGE_SET_NOT_PREVIEWED",
                        "ChangeSet has no sealed preview to confirm.",
                    )
                })
                .and_then(|value| {
                    serde_json::from_value::<AgentAssetVersion>(value.clone()).map_err(|_| {
                        CoreError::conflict(
                            "CHANGE_SET_PREVIEW_INVALID",
                            "Stored ChangeSet preview cannot be decoded by the current contract.",
                        )
                    })
                })?;
            if !same_preview_semantics(&sealed_preview, resulting)? {
                return Err(CoreError::conflict(
                    "CHANGE_SET_PREVIEW_DRIFT",
                    "Confirmed asset content does not match the exact preview accepted by the user.",
                ));
            }
            let next_version: u64 = transaction.query_row(
                "SELECT COALESCE(MAX(version_no), 0) + 1 FROM agent_asset_versions WHERE project_id=?",
                [&change_set.project_id],
                |row| row.get(0),
            )?;
            if resulting.version_no != next_version {
                return Err(CoreError::conflict(
                    "ASSET_VERSION_NUMBER_STALE",
                    "Confirmed asset version number is not the next immutable version.",
                ));
            }
            insert_version(transaction, resulting)?;
            transaction.execute(
                "UPDATE agent_asset_versions SET status='superseded' WHERE asset_version_id=? AND status='committed'",
                [&change_set.base_asset_version_id],
            )?;
            set_head(transaction, &change_set.project_id, &resulting.asset_version_id, &resulting.created_at)?;
            advance_snapshot(transaction, &snapshot, resulting, &resulting.created_at)?;
            transaction.execute(
                "UPDATE agent_asset_change_sets SET preview_json=?, status='confirmed', resulting_asset_version_id=?, updated_at=? WHERE change_set_id=?",
                params![json_text(resulting)?, resulting.asset_version_id, resulting.created_at, change_set_id],
            )?;
            // R007 plans deliberately piggyback the ordinary ChangeSet
            // lifecycle.  R007B additionally requires a real production GLB
            // object, so it cannot be confirmed through this legacy semantic-
            // only method.
            finalize_reference_rebuild_result(
                transaction,
                change_set_id,
                &resulting.asset_version_id,
                None,
                &resulting.created_at,
            )?;
            Ok((
                require_change_set(transaction, change_set_id)?,
                require_version(transaction, &resulting.asset_version_id)?,
                require_snapshot(transaction, &change_set.project_id)?,
            ))
        })
    }

    pub fn read_change_set_confirm_bundle(
        &self,
        change_set_id: &str,
        resulting_asset_version_id: &str,
        quality_report_id: &str,
    ) -> CoreResult<Option<ChangeSetConfirmBundleReadback>> {
        let connection = open_connection(self.db_path())?;
        let Some(bundle) = change_set_confirm_bundle_from_connection(
            &connection,
            change_set_id,
            resulting_asset_version_id,
            quality_report_id,
        )?
        else {
            return Ok(None);
        };
        self.validate_change_set_confirm_bundle_objects(&bundle)?;
        Ok(Some(bundle))
    }

    pub fn validate_change_set_confirm_bundle(
        &self,
        bundle: &ChangeSetConfirmBundleReadback,
    ) -> CoreResult<()> {
        let current = self
            .read_change_set_confirm_bundle(
                &bundle.change_set.change_set_id,
                &bundle.version.asset_version_id,
                &bundle.quality.quality_report_id,
            )?
            .ok_or_else(|| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                    confirm_bundle_ids(
                        &bundle.change_set.change_set_id,
                        &bundle.version.asset_version_id,
                        &bundle.quality.quality_report_id,
                    ),
                    vec!["confirm_bundle"],
                )
            })?;
        if current != *bundle {
            return Err(CoreError::conflict_with_details(
                "CHANGE_SET_CONFIRM_BUNDLE_READBACK_DRIFT",
                "Confirmed ChangeSet bundle no longer matches its authoritative readback.",
                confirm_bundle_ids(
                    &bundle.change_set.change_set_id,
                    &bundle.version.asset_version_id,
                    &bundle.quality.quality_report_id,
                ),
            ));
        }
        Ok(())
    }

    /// Confirms the exact sealed preview and atomically transfers its
    /// interactive CAS object to the new immutable version while adding the
    /// production object, quality, head and Snapshot state.
    pub fn confirm_change_set_bundle(
        &self,
        change_set_id: &str,
        sealed_preview: &AgentAssetVersion,
        resulting: &AgentAssetVersion,
        interactive_glb: &[u8],
        production_glb: &[u8],
        quality: &QualityReport,
        expected: SnapshotEtag,
    ) -> CoreResult<ChangeSetConfirmBundleReadback> {
        ensure_canonical_shape_program(
            &sealed_preview.shape_program,
            "CHANGE_SET_CONFIRM_NON_CANONICAL_SHAPE_PROGRAM",
        )?;
        ensure_canonical_shape_program(
            &resulting.shape_program,
            "CHANGE_SET_CONFIRM_NON_CANONICAL_SHAPE_PROGRAM",
        )?;
        sealed_preview.validate()?;
        resulting.validate()?;
        quality.validate()?;
        if resulting.status != AssetVersionStatus::Committed
            || !same_preview_semantics(sealed_preview, resulting)?
        {
            return Err(CoreError::invalid_data(
                "CHANGE_SET_CONFIRM_RESULT_INVALID",
                "Confirmed version must be committed and preserve the exact sealed preview semantics.",
            ));
        }
        let interactive_verified =
            crate::verify_forgecad_glb(interactive_glb, Some("interactive_preview"))?;
        let production_verified =
            crate::verify_forgecad_glb(production_glb, Some("production_concept"))?;
        validate_production_quality_readback(quality, resulting, &production_verified)?;

        let existing_confirm = self.read_change_set_confirm_bundle(
            change_set_id,
            &resulting.asset_version_id,
            &quality.quality_report_id,
        )?;
        let active_preview = if existing_confirm.is_none() {
            Some(
                self.read_change_set_preview_bundle(change_set_id)?
                    .ok_or_else(|| {
                        change_set_bundle_incomplete(
                            "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                            preview_bundle_ids(change_set_id),
                            vec!["active_preview_bundle"],
                        )
                    })?,
            )
        } else {
            None
        };
        if let Some(preview) = active_preview.as_ref() {
            if preview.sealed_preview != *sealed_preview
                || preview.interactive_preview_glb.sha256 != interactive_verified.glb_sha256
                || preview.interactive_preview_glb.byte_size != interactive_verified.glb_byte_size
            {
                return Err(CoreError::conflict_with_details(
                    "CHANGE_SET_CONFIRM_BUNDLE_IDEMPOTENCY_CONFLICT",
                    "Confirmation input does not match the exact active preview bundle.",
                    confirm_bundle_ids(
                        change_set_id,
                        &resulting.asset_version_id,
                        &quality.quality_report_id,
                    ),
                ));
            }
            validate_interactive_readback(
                &preview.interactive_readback,
                sealed_preview,
                &interactive_verified,
            )?;
        }

        let mut production = self.object_store.stage(production_glb, "glb")?.promote()?;
        let mut interactive = match self
            .object_store
            .stage(interactive_glb, "glb")
            .and_then(|staged| staged.promote())
        {
            Ok(promoted) => promoted,
            Err(error) => {
                production.cleanup_after_rollback();
                return Err(error);
            }
        };
        let production_stored = production.metadata().clone();
        let interactive_stored = interactive.metadata().clone();
        let result = self.write(|transaction| {
            if let Some(existing) = change_set_confirm_bundle_from_connection(
                transaction,
                change_set_id,
                &resulting.asset_version_id,
                &quality.quality_report_id,
            )? {
                validate_change_set_confirm_replay(
                    &existing,
                    sealed_preview,
                    resulting,
                    quality,
                    &production_stored,
                    &interactive_stored,
                )?;
                return Ok(existing);
            }

            let preview_bundle = change_set_preview_bundle_from_connection(
                transaction,
                change_set_id,
            )?
            .ok_or_else(|| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    preview_bundle_ids(change_set_id),
                    vec!["active_preview_bundle"],
                )
            })?;
            if preview_bundle.sealed_preview != *sealed_preview
                || preview_bundle.interactive_preview_glb.sha256 != interactive_stored.sha256
                || preview_bundle.interactive_preview_glb.byte_size
                    != interactive_stored.byte_size
            {
                return Err(CoreError::conflict(
                    "CHANGE_SET_PREVIEW_DRIFT",
                    "Confirmed asset or interactive artifact differs from the sealed preview.",
                ));
            }
            let change_set = &preview_bundle.change_set;
            let snapshot =
                require_agent_snapshot(transaction, &change_set.project_id, expected)?;
            if snapshot.preview.as_ref().map(|value| value.change_set_id.as_str())
                != Some(change_set_id)
                || snapshot.active_design.asset_version_id()
                    != Some(change_set.base_asset_version_id.as_str())
                || require_head(transaction, &change_set.project_id)?
                    != change_set.base_asset_version_id
                || resulting.project_id != change_set.project_id
                || resulting.parent_asset_version_id.as_deref()
                    != Some(change_set.base_asset_version_id.as_str())
            {
                return Err(stale("CHANGE_SET_BASE_STALE"));
            }
            let active_base = require_version(transaction, &change_set.base_asset_version_id)?;
            validate_change_set_operations(transaction, &active_base, &snapshot, change_set)?;
            let next_version: u64 = transaction.query_row(
                "SELECT COALESCE(MAX(version_no), 0) + 1 FROM agent_asset_versions WHERE project_id=?",
                [&change_set.project_id],
                |row| row.get(0),
            )?;
            if resulting.version_no != next_version {
                return Err(CoreError::conflict(
                    "ASSET_VERSION_NUMBER_STALE",
                    "Confirmed asset version number is not the next immutable version.",
                ));
            }

            insert_object_metadata(transaction, &interactive_stored, &resulting.created_at)?;
            insert_object_metadata(transaction, &production_stored, &resulting.created_at)?;
            insert_version(transaction, resulting)?;
            transaction.execute(
                "UPDATE agent_asset_versions SET status='superseded' WHERE asset_version_id=? AND status='committed'",
                [&change_set.base_asset_version_id],
            )?;
            set_head(
                transaction,
                &change_set.project_id,
                &resulting.asset_version_id,
                &resulting.created_at,
            )?;
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('asset_version', ?, 'interactive_preview_glb', ?, ?)",
                params![resulting.asset_version_id, interactive_stored.sha256, resulting.created_at],
            )?;
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('asset_version', ?, 'production_glb', ?, ?)",
                params![resulting.asset_version_id, production_stored.sha256, resulting.created_at],
            )?;
            transaction.execute(
                "INSERT INTO agent_asset_quality_reports(quality_report_id, project_id, asset_version_id, report_json, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    quality.quality_report_id,
                    quality.project_id,
                    quality.asset_version_id,
                    json_text(&quality.report)?,
                    quality.status.as_str(),
                    quality.created_at,
                ],
            )?;
            advance_snapshot(transaction, &snapshot, resulting, &resulting.created_at)?;
            let changed = transaction.execute(
                "UPDATE active_design_snapshots SET quality_report_id=?, quality_asset_version_id=?, updated_at=? WHERE project_id=? AND active_asset_version_id=? AND revision=?",
                params![
                    quality.quality_report_id,
                    resulting.asset_version_id,
                    quality.created_at,
                    change_set.project_id,
                    resulting.asset_version_id,
                    snapshot.revision + 1,
                ],
            )?;
            if changed != 1 {
                return Err(stale("CHANGE_SET_CONFIRM_SNAPSHOT_STALE"));
            }
            let preview_seal = parse_change_set_preview_seal(
                change_set.preview.as_ref().ok_or_else(|| {
                    CoreError::conflict(
                        "CHANGE_SET_PREVIEW_INVALID",
                        "ChangeSet preview seal is missing.",
                    )
                })?,
            )?;
            let confirm_seal = ChangeSetConfirmSeal {
                schema_version: CHANGE_SET_CONFIRM_SEAL_SCHEMA.into(),
                sealed_preview: preview_seal.sealed_preview,
                interactive_readback: preview_seal.interactive_readback,
                interactive_glb_sha256: interactive_stored.sha256.clone(),
                interactive_glb_byte_size: interactive_stored.byte_size,
                resulting_asset_version_id: resulting.asset_version_id.clone(),
                production_glb_sha256: production_stored.sha256.clone(),
                production_glb_byte_size: production_stored.byte_size,
                quality_report_id: quality.quality_report_id.clone(),
            };
            transaction.execute(
                "UPDATE agent_asset_change_sets SET preview_json=?, status='confirmed', resulting_asset_version_id=?, updated_at=? WHERE change_set_id=? AND status='previewed'",
                params![
                    json_text(&confirm_seal)?,
                    resulting.asset_version_id,
                    quality.created_at,
                    change_set_id,
                ],
            )?;
            finalize_reference_rebuild_result(
                transaction,
                change_set_id,
                &resulting.asset_version_id,
                Some(&production_stored.sha256),
                &quality.created_at,
            )?;
            transaction.execute(
                "DELETE FROM forgecad_core_object_references WHERE reference_kind='preview' AND owner_id=? AND role='interactive_preview_glb'",
                [change_set_id],
            )?;
            change_set_confirm_bundle_from_connection(
                transaction,
                change_set_id,
                &resulting.asset_version_id,
                &quality.quality_report_id,
            )?
            .ok_or_else(|| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                    confirm_bundle_ids(
                        change_set_id,
                        &resulting.asset_version_id,
                        &quality.quality_report_id,
                    ),
                    vec!["transaction_readback"],
                )
            })
        });
        match result {
            Ok(bundle) => {
                let production_finalize = production.finalize_commit();
                let interactive_finalize = interactive.finalize_commit();
                if production_finalize.is_err() || interactive_finalize.is_err() {
                    self.recover_object_store()?;
                }
                self.validate_change_set_confirm_bundle_objects(&bundle)?;
                Ok(bundle)
            }
            Err(error) => {
                interactive.cleanup_after_rollback();
                production.cleanup_after_rollback();
                Err(error)
            }
        }
    }

    fn validate_change_set_confirm_bundle_objects(
        &self,
        bundle: &ChangeSetConfirmBundleReadback,
    ) -> CoreResult<()> {
        let details = confirm_bundle_ids(
            &bundle.change_set.change_set_id,
            &bundle.version.asset_version_id,
            &bundle.quality.quality_report_id,
        );
        let production_bytes = self
            .object_store
            .read(&stored_from_record(&bundle.production_glb))
            .map_err(|_| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                    details.clone(),
                    vec!["production_glb_bytes"],
                )
            })?;
        let interactive_bytes = self
            .object_store
            .read(&stored_from_record(&bundle.interactive_preview_glb))
            .map_err(|_| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                    details.clone(),
                    vec!["interactive_preview_bytes"],
                )
            })?;
        let production = crate::verify_forgecad_glb(&production_bytes, Some("production_concept"))
            .map_err(|_| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                    details.clone(),
                    vec!["production_glb_readback"],
                )
            })?;
        let interactive =
            crate::verify_forgecad_glb(&interactive_bytes, Some("interactive_preview")).map_err(
                |_| {
                    change_set_bundle_incomplete(
                        "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                        details.clone(),
                        vec!["interactive_preview_readback"],
                    )
                },
            )?;
        validate_production_quality_readback(&bundle.quality, &bundle.version, &production)
            .map_err(|_| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                    details.clone(),
                    vec!["quality_readback"],
                )
            })?;
        let seal = parse_change_set_confirm_seal(bundle.change_set.preview.as_ref().ok_or_else(
            || {
                change_set_bundle_incomplete(
                    "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                    details.clone(),
                    vec!["confirm_seal"],
                )
            },
        )?)?;
        validate_interactive_readback(
            &seal.interactive_readback,
            &seal.sealed_preview,
            &interactive,
        )
        .map_err(|_| {
            change_set_bundle_incomplete(
                "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                details,
                vec!["interactive_readback_metadata"],
            )
        })
    }

    pub fn reject_change_set(
        &self,
        change_set_id: &str,
        expected: SnapshotEtag,
        updated_at: &str,
    ) -> CoreResult<(AgentAssetChangeSet, ActiveDesignSnapshot)> {
        self.write(|transaction| {
            let change_set = require_change_set(transaction, change_set_id)?;
            if !matches!(change_set.status, ChangeSetStatus::Proposed | ChangeSetStatus::Previewed) {
                return Err(CoreError::conflict(
                    "CHANGE_SET_STATE_CONFLICT",
                    "ChangeSet was already resolved.",
                ));
            }
            let snapshot = require_agent_snapshot(transaction, &change_set.project_id, expected)?;
            transaction.execute(
                "UPDATE agent_asset_change_sets SET preview_json=NULL, status='rejected', resulting_asset_version_id=NULL, updated_at=? WHERE change_set_id=?",
                params![updated_at, change_set_id],
            )?;
            transaction.execute(
                "UPDATE reference_guided_rebuild_plans SET status='rejected', updated_at=? WHERE preview_change_set_id=? AND status='previewed'",
                params![updated_at, change_set_id],
            )?;
            transaction.execute(
                "DELETE FROM forgecad_core_object_references WHERE reference_kind='preview' AND owner_id=? AND role='interactive_preview_glb'",
                [change_set_id],
            )?;
            if snapshot
                .preview
                .as_ref()
                .is_some_and(|preview| preview.change_set_id == change_set_id)
            {
                let changed = transaction.execute(
                    "UPDATE active_design_snapshots SET preview_change_set_id=NULL, preview_base_asset_version_id=NULL, revision=revision+1, updated_at=? WHERE project_id=? AND revision=?",
                    params![updated_at, change_set.project_id, snapshot.revision],
                )?;
                require_changed(changed)?;
            }
            Ok((
                require_change_set(transaction, change_set_id)?,
                require_snapshot(transaction, &change_set.project_id)?,
            ))
        })
    }

    pub fn reject_change_set_idempotent(
        &self,
        change_set_id: &str,
        expected: SnapshotEtag,
        updated_at: &str,
        idempotency_scope: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> CoreResult<AgentAssetChangeSet> {
        self.idempotent_write(
            idempotency_scope,
            idempotency_key,
            request_hash,
            updated_at,
            |transaction| {
                let change_set = require_change_set(transaction, change_set_id)?;
                if !matches!(
                    change_set.status,
                    ChangeSetStatus::Proposed | ChangeSetStatus::Previewed
                ) {
                    return Err(CoreError::conflict(
                        "CHANGE_SET_STATE_CONFLICT",
                        "ChangeSet was already resolved.",
                    ));
                }
                let snapshot = require_agent_snapshot(transaction, &change_set.project_id, expected)?;
                transaction.execute(
                    "UPDATE agent_asset_change_sets SET preview_json=NULL, status='rejected', resulting_asset_version_id=NULL, updated_at=? WHERE change_set_id=?",
                    params![updated_at, change_set_id],
                )?;
                transaction.execute(
                    "UPDATE reference_guided_rebuild_plans SET status='rejected', updated_at=? WHERE preview_change_set_id=? AND status='previewed'",
                    params![updated_at, change_set_id],
                )?;
                transaction.execute(
                    "DELETE FROM forgecad_core_object_references WHERE reference_kind='preview' AND owner_id=? AND role='interactive_preview_glb'",
                    [change_set_id],
                )?;
                if snapshot
                    .preview
                    .as_ref()
                    .is_some_and(|preview| preview.change_set_id == change_set_id)
                {
                    let changed = transaction.execute(
                        "UPDATE active_design_snapshots SET preview_change_set_id=NULL, preview_base_asset_version_id=NULL, revision=revision+1, updated_at=? WHERE project_id=? AND revision=?",
                        params![updated_at, change_set.project_id, snapshot.revision],
                    )?;
                    require_changed(changed)?;
                }
                require_change_set(transaction, change_set_id)
            },
        )
    }

    pub fn attach_quality(
        &self,
        report: &QualityReport,
        expected: SnapshotEtag,
    ) -> CoreResult<ActiveDesignSnapshot> {
        report.validate()?;
        self.write(|transaction| {
            let snapshot = require_agent_snapshot(transaction, &report.project_id, expected)?;
            if snapshot.active_design.asset_version_id() != Some(report.asset_version_id.as_str())
                || require_head(transaction, &report.project_id)? != report.asset_version_id
            {
                return Err(stale("QUALITY_ASSET_STALE"));
            }
            transaction.execute(
                "INSERT INTO agent_asset_quality_reports(quality_report_id, project_id, asset_version_id, report_json, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                params![report.quality_report_id, report.project_id, report.asset_version_id, json_text(&report.report)?, report.status.as_str(), report.created_at],
            )?;
            let changed = transaction.execute(
                "UPDATE active_design_snapshots SET quality_report_id=?, quality_asset_version_id=?, revision=revision+1, updated_at=? WHERE project_id=? AND revision=?",
                params![report.quality_report_id, report.asset_version_id, report.created_at, report.project_id, expected.0],
            )?;
            require_changed(changed)?;
            require_snapshot(transaction, &report.project_id)
        })
    }

    pub fn attach_quality_idempotent(
        &self,
        report: &QualityReport,
        expected: SnapshotEtag,
        idempotency_scope: &str,
        idempotency_key: &str,
        request_hash: &str,
    ) -> CoreResult<QualityReport> {
        report.validate()?;
        self.idempotent_write(
            idempotency_scope,
            idempotency_key,
            request_hash,
            &report.created_at,
            |transaction| {
                let snapshot = require_agent_snapshot(transaction, &report.project_id, expected)?;
                if snapshot.active_design.asset_version_id()
                    != Some(report.asset_version_id.as_str())
                    || require_head(transaction, &report.project_id)? != report.asset_version_id
                {
                    return Err(stale("QUALITY_ASSET_STALE"));
                }
                transaction.execute(
                    "INSERT INTO agent_asset_quality_reports(quality_report_id, project_id, asset_version_id, report_json, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                    params![
                        report.quality_report_id,
                        report.project_id,
                        report.asset_version_id,
                        json_text(&report.report)?,
                        report.status.as_str(),
                        report.created_at,
                    ],
                )?;
                let changed = transaction.execute(
                    "UPDATE active_design_snapshots SET quality_report_id=?, quality_asset_version_id=?, revision=revision+1, updated_at=? WHERE project_id=? AND revision=?",
                    params![
                        report.quality_report_id,
                        report.asset_version_id,
                        report.created_at,
                        report.project_id,
                        expected.0,
                    ],
                )?;
                require_changed(changed)?;
                require_quality(transaction, &report.quality_report_id)
            },
        )
    }

    /// Undo/redo always copies historical semantic content into a new immutable
    /// committed version. Historical rows are never reactivated.
    pub fn navigate(
        &self,
        project_id: &str,
        action: NavigationAction,
        resulting_asset_version_id: &str,
        expected: SnapshotEtag,
        created_at: &str,
    ) -> CoreResult<NavigationResult> {
        self.write(|transaction| {
            let snapshot = require_agent_snapshot(transaction, project_id, expected)?;
            if snapshot.preview.is_some() {
                return Err(CoreError::conflict(
                    "ACTIVE_DESIGN_PREVIEW_PENDING",
                    "Resolve the active preview before version navigation.",
                ));
            }
            let current_id = snapshot.active_design.asset_version_id().unwrap_or_default();
            if require_head(transaction, project_id)? != current_id {
                return Err(stale("ACTIVE_DESIGN_HEAD_INVALID"));
            }
            let current = require_version(transaction, current_id)?;
            let (current_undo, current_redo) = navigation_targets(transaction, &current)?;
            let target_id = match action {
                NavigationAction::Undo => current_undo,
                NavigationAction::Redo => current_redo,
            }
            .ok_or_else(|| {
                CoreError::conflict(
                    match action {
                        NavigationAction::Undo => "ACTIVE_DESIGN_UNDO_UNAVAILABLE",
                        NavigationAction::Redo => "ACTIVE_DESIGN_REDO_UNAVAILABLE",
                    },
                    "No immutable navigation target is available.",
                )
            })?;
            let target = require_version(transaction, &target_id)?;
            if target.project_id != project_id {
                return Err(CoreError::conflict(
                    "ACTIVE_DESIGN_NAVIGATION_INVALID",
                    "Navigation target belongs to another project.",
                ));
            }
            let next_version: u64 = transaction.query_row(
                "SELECT COALESCE(MAX(version_no), 0) + 1 FROM agent_asset_versions WHERE project_id=?",
                [project_id],
                |row| row.get(0),
            )?;
            let mut result = target.clone();
            result.asset_version_id = resulting_asset_version_id.to_string();
            result.parent_asset_version_id = Some(target.asset_version_id.clone());
            result.version_no = next_version;
            result.status = AssetVersionStatus::Committed;
            result.summary = format!(
                "{} v{}: {}",
                match action { NavigationAction::Undo => "Undo to", NavigationAction::Redo => "Redo to" },
                target.version_no,
                target.summary
            );
            result.created_at = created_at.to_string();
            result.validate()?;
            insert_version(transaction, &result)?;
            copy_asset_version_object_references(
                transaction,
                &target.asset_version_id,
                resulting_asset_version_id,
                created_at,
            )?;
            copy_reference_rebuild_result_lineage(
                transaction,
                &target.asset_version_id,
                resulting_asset_version_id,
                created_at,
            )?;
            let cloned_quality = clone_navigation_quality(
                transaction,
                &target.asset_version_id,
                resulting_asset_version_id,
                project_id,
                created_at,
            )?;
            transaction.execute(
                "UPDATE agent_asset_versions SET status='superseded' WHERE asset_version_id=? AND status='committed'",
                [current_id],
            )?;
            set_head(transaction, project_id, resulting_asset_version_id, created_at)?;
            advance_snapshot(transaction, &snapshot, &result, created_at)?;
            if let Some(quality_report_id) = cloned_quality {
                transaction.execute(
                    "UPDATE active_design_snapshots SET quality_report_id=?, quality_asset_version_id=? WHERE project_id=? AND source='agent_asset'",
                    params![quality_report_id, resulting_asset_version_id, project_id],
                )?;
            }
            let (target_undo, target_redo) = navigation_targets(transaction, &target)?;
            transaction.execute(
                "INSERT INTO agent_asset_navigation_frames(resulting_asset_version_id, project_id, undo_target_asset_version_id, redo_target_asset_version_id, action, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    resulting_asset_version_id,
                    project_id,
                    target_undo,
                    if action == NavigationAction::Undo { Some(current_id.to_string()) } else { target_redo },
                    action.as_str(),
                    created_at,
                ],
            )?;
            Ok(NavigationResult {
                version: require_version(transaction, resulting_asset_version_id)?,
                snapshot: require_snapshot(transaction, project_id)?,
            })
        })
    }

    /// Promotes bytes first, then commits object metadata and its reference in
    /// the same SQLite transaction. A failed transaction removes a new file.
    pub fn attach_object_bytes(
        &self,
        reference: &ObjectReference,
        bytes: &[u8],
        extension: &str,
        timestamp: &str,
    ) -> CoreResult<ObjectRecord> {
        reference.validate()?;
        let mut promoted = self.object_store.stage(bytes, extension)?.promote()?;
        let stored = promoted.metadata().clone();
        let result = self.write(|transaction| {
            insert_object_metadata(transaction, &stored, timestamp)?;
            let existing: Option<String> = transaction.query_row(
                "SELECT sha256 FROM forgecad_core_object_references WHERE reference_kind=? AND owner_id=? AND role=?",
                params![reference.reference_kind, reference.owner_id, reference.role],
                |row| row.get(0),
            ).optional()?;
            if existing.as_deref() != Some(stored.sha256.as_str()) {
                transaction.execute(
                    "DELETE FROM forgecad_core_object_references WHERE reference_kind=? AND owner_id=? AND role=?",
                    params![reference.reference_kind, reference.owner_id, reference.role],
                )?;
                transaction.execute(
                    "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES (?, ?, ?, ?, ?)",
                    params![reference.reference_kind, reference.owner_id, reference.role, stored.sha256, timestamp],
                )?;
            }
            object_record(transaction, &stored.sha256)
        });
        match result {
            Ok(record) => {
                promoted.finalize_commit()?;
                Ok(record)
            }
            Err(error) => {
                promoted.cleanup_after_rollback();
                Err(error)
            }
        }
    }

    pub fn object(&self, sha256: &str) -> CoreResult<Option<ObjectRecord>> {
        let connection = open_connection(self.db_path())?;
        object_record_optional(&connection, sha256)
    }

    pub fn object_for_reference(
        &self,
        reference: &ObjectReference,
    ) -> CoreResult<Option<ObjectRecord>> {
        reference.validate()?;
        let connection = open_connection(self.db_path())?;
        connection
            .query_row(
                "SELECT o.sha256, o.object_path, o.extension, o.byte_size, o.ref_count, o.created_at, o.updated_at FROM forgecad_core_object_references r JOIN forgecad_core_objects o ON o.sha256=r.sha256 WHERE r.reference_kind=? AND r.owner_id=? AND r.role=?",
                params![reference.reference_kind, reference.owner_id, reference.role],
                object_record_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn recover_object_store(&self) -> CoreResult<Vec<String>> {
        let connection = open_connection(self.db_path())?;
        let mut statement =
            connection.prepare("SELECT sha256 FROM forgecad_core_objects ORDER BY sha256")?;
        let indexed = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<BTreeSet<_>, _>>()?;
        self.object_store.recover_pending(&indexed)
    }

    /// Completes only deletions named by the durable SQLite journal. This is
    /// intentionally not a filesystem scan: historical or user-owned files
    /// can never be collected merely because they are absent from the index.
    pub fn recover_object_deletions(&self) -> CoreResult<Vec<String>> {
        self.recover_object_deletions_with(DeletionRecoveryPhase::Published)
    }

    /// Startup recovery runs while the OS writer lock and durable writer epoch
    /// are already authoritative, but before the desktop handlers are exposed.
    /// It must not publish the first Python→Rust cutover: later handler setup
    /// can still fail and restore the previous owner. Each completed unlink is
    /// paired with deletion of its exact journal row in a bootstrap-fenced
    /// transaction, so partial recovery remains consistent and retryable.
    fn recover_object_deletions_during_bootstrap(&self) -> CoreResult<Vec<String>> {
        self.recover_object_deletions_with(DeletionRecoveryPhase::Bootstrap)
    }

    fn recover_object_deletions_with(
        &self,
        phase: DeletionRecoveryPhase,
    ) -> CoreResult<Vec<String>> {
        let connection = open_connection(self.db_path())?;
        let records = {
            let mut statement = connection.prepare(
                "SELECT sha256, object_path, extension, byte_size, created_at FROM forgecad_core_object_deletion_journal ORDER BY sha256",
            )?;
            let records = statement
                .query_map([], |row| {
                    Ok((
                        StoredObject {
                            sha256: row.get(0)?,
                            relative_path: row.get(1)?,
                            extension: row.get(2)?,
                            byte_size: row.get(3)?,
                        },
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            records
        };
        drop(connection);

        let mut recovered = Vec::with_capacity(records.len());
        for (stored, created_at) in records {
            // Fail before touching CAS if the durable epoch no longer matches.
            // The process-scoped OS lock fences legitimate competing writers;
            // the transaction below rechecks the same epoch after unlink.
            let connection = open_connection(self.db_path())?;
            self.lease.assert_current(&connection)?;
            drop(connection);
            self.object_store.remove(&stored)?;
            let clear_journal = |transaction: &Transaction<'_>| {
                let changed = transaction.execute(
                    "DELETE FROM forgecad_core_object_deletion_journal WHERE sha256=? AND object_path=? AND extension=? AND byte_size=? AND created_at=?",
                    params![
                        stored.sha256,
                        stored.relative_path,
                        stored.extension,
                        stored.byte_size,
                        created_at,
                    ],
                )?;
                if changed != 1 {
                    return Err(CoreError::conflict(
                        "CONTENT_OBJECT_DELETION_JOURNAL_STALE",
                        "CAS deletion journal changed while recovery held the Rust writer lease.",
                    ));
                }
                Ok(())
            };
            match phase {
                DeletionRecoveryPhase::Bootstrap => self.bootstrap_write(clear_journal)?,
                DeletionRecoveryPhase::Published => self.write(clear_journal)?,
            }
            recovered.push(stored.sha256);
        }
        Ok(recovered)
    }

    /// Writes an export artifact only when the caller's Snapshot ETag still
    /// names the active/head version. The immutable export source never
    /// switches implicitly by file format.
    pub fn attach_export_bytes(
        &self,
        project_id: &str,
        expected: SnapshotEtag,
        role: &str,
        bytes: &[u8],
        extension: &str,
        timestamp: &str,
    ) -> CoreResult<(ObjectRecord, ActiveDesignSnapshot)> {
        let reference_probe = ObjectReference {
            reference_kind: "export".to_string(),
            owner_id: "placeholder".to_string(),
            role: role.to_string(),
        };
        reference_probe.validate()?;
        let mut promoted = self.object_store.stage(bytes, extension)?.promote()?;
        let stored = promoted.metadata().clone();
        let result = self.write(|transaction| {
            let snapshot = require_agent_snapshot(transaction, project_id, expected)?;
            let version_id = snapshot.active_design.asset_version_id().unwrap_or_default();
            if require_head(transaction, project_id)? != version_id
                || snapshot.export.source_version_id() != version_id
            {
                return Err(stale("EXPORT_SOURCE_STALE"));
            }
            insert_object_metadata(transaction, &stored, timestamp)?;
            transaction.execute(
                "DELETE FROM forgecad_core_object_references WHERE reference_kind='export' AND owner_id=? AND role=?",
                params![version_id, role],
            )?;
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('export', ?, ?, ?, ?)",
                params![version_id, role, stored.sha256, timestamp],
            )?;
            Ok((object_record(transaction, &stored.sha256)?, snapshot))
        });
        match result {
            Ok(output) => {
                promoted.finalize_commit()?;
                Ok(output)
            }
            Err(error) => {
                promoted.cleanup_after_rollback();
                Err(error)
            }
        }
    }

    /// Canonical read-only fingerprint for historical Concept/Profile/Module
    /// rows. Migration verification can compare this before/after K003 without
    /// ever treating legacy data as a writable Agent asset.
    pub fn legacy_read_only_hash(&self, project_id: &str) -> CoreResult<Option<String>> {
        let connection = open_connection(self.db_path())?;
        let row: Option<(String, Option<String>, Option<String>)> = connection
            .query_row(
                "SELECT d.profile_json, v.spec_json, g.graph_json FROM projects p JOIN domain_profiles d ON d.profile_id=p.profile_id LEFT JOIN project_versions v ON v.version_id=p.current_version_id LEFT JOIN module_graphs g ON g.graph_id=v.module_graph_id WHERE p.project_id=?",
                [project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        row.map(|(profile, spec, graph)| {
            crate::semantic_sha256(&serde_json::json!({
                "project_id": project_id,
                "profile": parse_json::<serde_json::Value>(profile)?,
                "spec": spec.map(parse_json::<serde_json::Value>).transpose()?,
                "graph": graph.map(parse_json::<serde_json::Value>).transpose()?,
            }))
        })
        .transpose()
    }

    /// Validates a prospective A005 Skill without opening a transaction,
    /// promoting an object, changing a Snapshot, or invoking any Product Tool.
    /// This is deliberately the only dry-run surface: callers cannot use it
    /// to probe filesystem paths, URLs, dynamic operations, or credentials.
    pub fn dry_run_skill(&self, manifest: &AgentSkillManifest) -> CoreResult<AgentSkillDryRun> {
        self.validate_skill_manifest_policy(manifest)?;
        let skill_sha256 = manifest.canonical_sha256()?;
        Ok(AgentSkillDryRun {
            schema_version: "AgentSkillDryRun@1".to_string(),
            skill_id: manifest.skill_id.clone(),
            skill_version: manifest.version,
            skill_sha256,
            allowed_product_tool_ids: manifest.product_tool_ids.clone(),
            allowed_g819_operations: manifest.g819_operations.clone(),
            allowed_recipe_ids: manifest.recipe_ids.clone(),
            allowed_material_preset_ids: manifest.material_preset_ids.clone(),
            product_state_write_performed: false,
        })
    }

    /// Persists one immutable Skill version. Canonical manifest bytes are put
    /// into the Rust CAS and referenced from SQLite, so later source edits or
    /// restarts cannot silently change an enabled Skill.
    pub fn create_skill_draft(
        &self,
        manifest: &AgentSkillManifest,
        timestamp: &str,
    ) -> CoreResult<AgentSkillManifest> {
        self.validate_skill_manifest_policy(manifest)?;
        let canonical = crate::canonical_json(manifest)?;
        let manifest_sha256 = manifest.canonical_sha256()?;
        let mut promoted = self
            .object_store
            .stage(canonical.as_bytes(), "json")?
            .promote()?;
        let stored = promoted.metadata().clone();
        let owner_id = skill_manifest_owner_id(&manifest.skill_id, manifest.version);
        let result = self.write(|transaction| {
            insert_object_metadata(transaction, &stored, timestamp)?;
            let existing = skill_manifest_from_connection(transaction, &manifest.skill_id, manifest.version)?;
            if let Some((existing, existing_sha, existing_object_sha, _)) = existing {
                if existing != *manifest || existing_sha != manifest_sha256 || existing_object_sha != stored.sha256 {
                    return Err(CoreError::conflict(
                        "SKILL_VERSION_IMMUTABLE",
                        "A Skill version already exists and cannot be overwritten.",
                    ));
                }
                return Ok(existing);
            }
            let prior_max: Option<u32> = transaction.query_row(
                "SELECT MAX(version) FROM agent_skill_versions WHERE skill_id=?",
                [&manifest.skill_id],
                |row| row.get(0),
            )?;
            if let Some(prior) = prior_max {
                if manifest.version != prior.saturating_add(1) {
                    return Err(CoreError::conflict(
                        "SKILL_VERSION_SEQUENCE_INVALID",
                        "New Skill versions must advance exactly one immutable version.",
                    ));
                }
            } else if manifest.version != 1 {
                return Err(CoreError::conflict(
                    "SKILL_VERSION_SEQUENCE_INVALID",
                    "The first immutable Skill version must be version 1.",
                ));
            }
            transaction.execute(
                "INSERT INTO agent_skill_versions(skill_id, version, manifest_json, manifest_sha256, manifest_object_sha256, status, created_at, evaluated_at) VALUES (?, ?, ?, ?, ?, 'draft', ?, NULL)",
                params![manifest.skill_id, manifest.version, canonical, manifest_sha256, stored.sha256, timestamp],
            )?;
            transaction.execute(
                "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('reference', ?, 'skill_manifest', ?, ?)",
                params![owner_id, stored.sha256, timestamp],
            )?;
            Ok(manifest.clone())
        });
        match result {
            Ok(value) => {
                promoted.finalize_commit()?;
                Ok(value)
            }
            Err(error) => {
                promoted.cleanup_after_rollback();
                Err(error)
            }
        }
    }

    pub fn skill_manifest(
        &self,
        skill_id: &str,
        version: u32,
    ) -> CoreResult<Option<AgentSkillManifest>> {
        let connection = open_connection(self.db_path())?;
        skill_manifest_from_connection(&connection, skill_id, version)?
            .map(|(manifest, _, _, _)| {
                self.validate_skill_manifest_policy(&manifest)?;
                Ok(manifest)
            })
            .transpose()
    }

    /// Ordered immutable history for a Skill ID.  Activation is intentionally
    /// not inferred from this list; callers must read the separate pointer.
    pub fn skill_manifests(&self, skill_id: &str) -> CoreResult<Vec<AgentSkillManifest>> {
        let connection = open_connection(self.db_path())?;
        let mut statement = connection.prepare(
            "SELECT manifest_json, manifest_sha256, manifest_object_sha256, status FROM agent_skill_versions WHERE skill_id=? ORDER BY version ASC",
        )?;
        let manifests = statement
            .query_map([skill_id], |row| {
                let manifest_json: String = row.get(0)?;
                let manifest: AgentSkillManifest =
                    serde_json::from_str(&manifest_json).map_err(|_| {
                        to_sql_error(CoreError::invalid_data(
                            "SKILL_MANIFEST_INVALID",
                            "Stored Skill manifest cannot be decoded.",
                        ))
                    })?;
                manifest.validate().map_err(to_sql_error)?;
                let stored_hash: String = row.get(1)?;
                let object_hash: String = row.get(2)?;
                if manifest.canonical_sha256().map_err(to_sql_error)? != stored_hash
                    || object_hash != stored_hash
                {
                    return Err(to_sql_error(CoreError::invalid_data(
                        "SKILL_MANIFEST_HASH_INVALID",
                        "Stored Skill manifest bytes no longer match their sealed hash.",
                    )));
                }
                Ok(manifest)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for manifest in &manifests {
            self.validate_skill_manifest_policy(manifest)?;
        }
        Ok(manifests)
    }

    /// Creates the first-party A005 starter if absent.  It remains a draft so
    /// callers must still dry-run, record an evaluation and explicitly enable
    /// it; this helper can never silently activate a Skill after restart.
    pub fn ensure_builtin_surface_adornment_skill(
        &self,
        timestamp: &str,
    ) -> CoreResult<AgentSkillManifest> {
        // Preserve the immutable C105-only v1 before publishing the explicit
        // C105+C106 v2. Fresh and upgraded repositories therefore seal the
        // same version/hash chain without granting old activations new rights.
        self.create_skill_draft(&builtin_surface_adornment_manifest(), timestamp)?;
        self.create_skill_draft(&builtin_surface_adornment_manifest_v2(), timestamp)
    }

    /// Validates a visual-only adornment against the sealed, currently enabled
    /// Skill and the immutable asset it names.  This deliberately returns no
    /// geometry and writes no Product state: the app-server must still create
    /// a preview ChangeSet and obtain explicit confirmation for a persistent
    /// asset edit.
    pub fn validate_surface_adornment_program(
        &self,
        asset_version_id: &str,
        program: &SurfaceAdornmentProgram,
    ) -> CoreResult<()> {
        program.validate()?;
        let connection = open_connection(self.db_path())?;
        let version = require_version(&connection, asset_version_id)?;
        let activation = connection
            .query_row(
                "SELECT activation_id, skill_id, skill_version, skill_sha256, enabled, updated_at FROM agent_skill_activations WHERE skill_id=? AND enabled=1",
                [&program.skill_id],
                skill_activation_from_row,
            )
            .optional()?
            .ok_or_else(|| CoreError::conflict("SURFACE_ADORNMENT_SKILL_DISABLED", "Surface adornment requires an explicitly enabled Skill."))?;
        if activation.skill_version != program.skill_version
            || activation.skill_sha256 != program.skill_sha256
        {
            return Err(CoreError::conflict(
                "SURFACE_ADORNMENT_SKILL_STALE",
                "Surface adornment must name the active immutable Skill version and hash.",
            ));
        }
        let (manifest, manifest_sha, _, _) =
            require_skill_manifest(&connection, &program.skill_id, program.skill_version)?;
        self.validate_skill_manifest_policy(&manifest)?;
        if manifest_sha != program.skill_sha256
            || !manifest
                .allowed_domains
                .iter()
                .any(|domain| domain == &version.domain_pack_id)
            || !manifest
                .material_preset_ids
                .iter()
                .any(|material| material == &program.base_material)
        {
            return Err(CoreError::conflict(
                "SURFACE_ADORNMENT_POLICY_DENIED",
                "Surface adornment is outside the enabled Skill domain or material policy.",
            ));
        }
        let target_part = version
            .parts
            .iter()
            .find(|part| {
                part.get("part_id").and_then(serde_json::Value::as_str)
                    == Some(program.target_part_id.as_str())
            })
            .ok_or_else(|| CoreError::not_found("Surface adornment target part"))?;
        let has_zone = ["material_zone_ids", "material_zones"].iter().any(|field| {
            target_part
                .get(*field)
                .and_then(serde_json::Value::as_array)
                .is_some_and(|zones| {
                    zones
                        .iter()
                        .any(|zone| zone.as_str() == Some(program.target_zone_id.as_str()))
                })
        });
        if !has_zone {
            return Err(CoreError::conflict(
                "SURFACE_ADORNMENT_ZONE_INVALID",
                "Surface adornment target zone is not owned by the target part.",
            ));
        }
        validate_surface_adornment_recipe_slot(
            &version,
            &program.target_part_id,
            program,
            &manifest,
        )?;
        Ok(())
    }

    /// Records a sealed, non-networked evaluation. Passing an evaluation only
    /// makes a version eligible for later explicit activation; it never turns
    /// it on or writes product state.
    pub fn record_skill_eval(
        &self,
        report: &AgentSkillEvalReport,
    ) -> CoreResult<AgentSkillEvalReport> {
        report.validate()?;
        self.write(|transaction| {
            let (manifest, manifest_sha, _, _) = require_skill_manifest(transaction, &report.skill_id, report.skill_version)?;
            self.validate_skill_manifest_policy(&manifest)?;
            if manifest_sha != report.skill_sha256 {
                return Err(CoreError::conflict("SKILL_EVAL_REFERENCE_STALE", "Skill evaluation does not name the immutable manifest hash."));
            }
            let existing: Option<AgentSkillEvalReport> = transaction.query_row(
                "SELECT report_id, skill_id, skill_version, skill_sha256, status, findings_json, evaluated_at FROM agent_skill_eval_reports WHERE report_id=?",
                [&report.report_id], skill_eval_report_from_row,
            ).optional()?;
            if let Some(existing) = existing {
                if existing != *report { return Err(CoreError::conflict("SKILL_EVAL_IMMUTABLE", "Skill evaluation reports are immutable.")); }
                return Ok(existing);
            }
            transaction.execute(
                "INSERT INTO agent_skill_eval_reports(report_id, skill_id, skill_version, skill_sha256, status, findings_json, evaluated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![report.report_id, report.skill_id, report.skill_version, report.skill_sha256, report.status.as_str(), json_text(&report.findings)?, report.evaluated_at],
            )?;
            if report.status == SkillEvalStatus::Passed {
                transaction.execute(
                    "UPDATE agent_skill_versions SET status='evaluated', evaluated_at=? WHERE skill_id=? AND version=? AND status='draft'",
                    params![report.evaluated_at, report.skill_id, report.skill_version],
                )?;
            }
            Ok(report.clone())
        })
    }

    pub fn skill_eval_reports(
        &self,
        skill_id: &str,
        skill_version: u32,
    ) -> CoreResult<Vec<AgentSkillEvalReport>> {
        let connection = open_connection(self.db_path())?;
        let mut statement = connection.prepare(
            "SELECT report_id, skill_id, skill_version, skill_sha256, status, findings_json, evaluated_at FROM agent_skill_eval_reports WHERE skill_id=? AND skill_version=? ORDER BY evaluated_at ASC, report_id ASC",
        )?;
        let reports = statement
            .query_map(params![skill_id, skill_version], skill_eval_report_from_row)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CoreError::from)?;
        Ok(reports)
    }

    /// Sets the independent mutable activation pointer. Enabling requires a
    /// sealed passing evaluation. Disabling keeps every old manifest, eval and
    /// asset provenance row intact.
    pub fn set_skill_activation(
        &self,
        activation: &AgentSkillActivation,
    ) -> CoreResult<AgentSkillActivation> {
        activation.validate()?;
        self.write(|transaction| {
            let (manifest, manifest_sha, _, status) = require_skill_manifest(transaction, &activation.skill_id, activation.skill_version)?;
            self.validate_skill_manifest_policy(&manifest)?;
            if manifest_sha != activation.skill_sha256 { return Err(CoreError::conflict("SKILL_ACTIVATION_REFERENCE_STALE", "Activation does not name the immutable Skill hash.")); }
            if activation.enabled {
                let passed: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM agent_skill_eval_reports WHERE skill_id=? AND skill_version=? AND skill_sha256=? AND status='passed')",
                    params![activation.skill_id, activation.skill_version, activation.skill_sha256], |row| row.get(0),
                )?;
                if status != "evaluated" || !passed { return Err(CoreError::conflict("SKILL_ENABLE_REQUIRES_EVAL", "Only a passing evaluated Skill can be enabled.")); }
            }
            transaction.execute(
                "INSERT INTO agent_skill_activations(skill_id, activation_id, skill_version, skill_sha256, enabled, updated_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(skill_id) DO UPDATE SET activation_id=excluded.activation_id, skill_version=excluded.skill_version, skill_sha256=excluded.skill_sha256, enabled=excluded.enabled, updated_at=excluded.updated_at",
                params![activation.skill_id, activation.activation_id, activation.skill_version, activation.skill_sha256, activation.enabled, activation.updated_at],
            )?;
            Ok(activation.clone())
        })
    }

    pub fn active_skill(&self, skill_id: &str) -> CoreResult<Option<AgentSkillActivation>> {
        let connection = open_connection(self.db_path())?;
        connection.query_row(
            "SELECT activation_id, skill_id, skill_version, skill_sha256, enabled, updated_at FROM agent_skill_activations WHERE skill_id=? AND enabled=1",
            [skill_id], skill_activation_from_row,
        ).optional().map_err(Into::into)
    }

    /// Associates an already-confirmed immutable asset with an immutable Skill
    /// version. This is provenance only; it cannot edit the version, Snapshot,
    /// ShapeProgram, material or GLB.
    pub fn record_asset_skill_reference(
        &self,
        asset_version_id: &str,
        skill_id: &str,
        skill_version: u32,
        skill_sha256: &str,
        timestamp: &str,
    ) -> CoreResult<()> {
        self.write(|transaction| {
            let exists: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM agent_asset_versions WHERE asset_version_id=?)", [asset_version_id], |row| row.get(0))?;
            if !exists { return Err(CoreError::not_found("Agent asset version")); }
            let (manifest, manifest_sha, _, _) = require_skill_manifest(transaction, skill_id, skill_version)?;
            self.validate_skill_manifest_policy(&manifest)?;
            if manifest_sha != skill_sha256 { return Err(CoreError::conflict("ASSET_SKILL_REFERENCE_STALE", "Asset Skill provenance must name the sealed manifest hash.")); }
            transaction.execute(
                "INSERT OR IGNORE INTO agent_asset_skill_references(asset_version_id, skill_id, skill_version, skill_sha256, recorded_at) VALUES (?, ?, ?, ?, ?)",
                params![asset_version_id, skill_id, skill_version, skill_sha256, timestamp],
            )?;
            Ok(())
        })
    }

    /// Physical deletion is deliberately narrow: an active pointer or any old
    /// immutable asset provenance is a hard blocker. The linked CAS manifest
    /// is detached only after the relational protection has succeeded.
    pub fn delete_skill_version(
        &self,
        skill_id: &str,
        version: u32,
        timestamp: &str,
    ) -> CoreResult<()> {
        let manifest = self
            .skill_manifest(skill_id, version)?
            .ok_or_else(|| CoreError::not_found("Skill version"))?;
        let owner_id = skill_manifest_owner_id(skill_id, version);
        self.write(|transaction| {
            let active: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM agent_skill_activations WHERE skill_id=? AND skill_version=? AND enabled=1)", params![skill_id, version], |row| row.get(0))?;
            let referenced: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM agent_asset_skill_references WHERE skill_id=? AND skill_version=?)", params![skill_id, version], |row| row.get(0))?;
            // A005 asset truth is also embedded in the immutable
            // AssemblyGraph so a crash after an asset bundle commit cannot
            // leave deletion protection dependent on a later side-table
            // write. Legacy versions simply omit `surface_adornments`.
            let graph_referenced = {
                let mut statement = transaction.prepare(
                    "SELECT assembly_graph_json FROM agent_asset_versions",
                )?;
                let graphs = statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                graphs.into_iter().any(|graph| {
                    serde_json::from_str::<serde_json::Value>(&graph)
                        .ok()
                        .and_then(|value| {
                            value
                                .get("surface_adornments")
                                .and_then(serde_json::Value::as_array)
                                .cloned()
                        })
                        .is_some_and(|programs| {
                            programs.iter().any(|program| {
                                program.get("skill_id").and_then(serde_json::Value::as_str)
                                    == Some(skill_id)
                                    && program
                                        .get("skill_version")
                                        .and_then(serde_json::Value::as_u64)
                                        == Some(u64::from(version))
                            })
                        })
                })
            };
            if active || referenced || graph_referenced { return Err(CoreError::conflict("SKILL_VERSION_REFERENCED", "Active or asset-referenced Skills cannot be physically deleted.")); }
            transaction.execute("DELETE FROM agent_skill_eval_reports WHERE skill_id=? AND skill_version=?", params![skill_id, version])?;
            transaction.execute("DELETE FROM agent_skill_activations WHERE skill_id=? AND skill_version=?", params![skill_id, version])?;
            let changed = transaction.execute("DELETE FROM agent_skill_versions WHERE skill_id=? AND version=?", params![skill_id, version])?;
            if changed != 1 { return Err(CoreError::not_found("Skill version")); }
            transaction.execute("DELETE FROM forgecad_core_object_references WHERE reference_kind='reference' AND owner_id=? AND role='skill_manifest'", params![owner_id])?;
            let _ = timestamp; // API symmetry; deletion has no invented wall-clock mutation.
            Ok(())
        })?;
        // Keep the local binding explicit so future refactors cannot claim a
        // delete succeeded without proving the exact manifest identity first.
        manifest.validate()
    }

    fn validate_skill_manifest_policy(&self, manifest: &AgentSkillManifest) -> CoreResult<()> {
        manifest.validate()?;
        // Product tools and G819 operations are independently checked by the
        // manifest's code-owned allow-lists.  Recipes are a separate C105
        // namespace and must exist in its reviewed registry; material IDs are
        // separately constrained to the M101/M102/M108A catalog above.
        let recipes = RecipeRegistry::from_embedded()?;
        // Keep historical C105 manifests independent from the newer pack.
        // The C106 registry is loaded only when a manifest explicitly names
        // at least one Recipe absent from the frozen C105 registry.
        let c106_recipes = manifest
            .recipe_ids
            .iter()
            .any(|recipe_id| recipes.recipe(recipe_id).is_none())
            .then(RecipeRegistry::from_embedded_c106_robotic_arm)
            .transpose()?;
        for recipe_id in &manifest.recipe_ids {
            let recipe = recipes
                .recipe(recipe_id)
                .or_else(|| {
                    c106_recipes
                        .as_ref()
                        .and_then(|registry| registry.recipe(recipe_id))
                })
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "SKILL_RECIPE_POLICY_INVALID",
                        "Skill names a Recipe outside the reviewed C105/C106 registries.",
                    )
                })?;
            if !recipe.allowed_domains.iter().any(|domain| {
                manifest
                    .allowed_domains
                    .iter()
                    .any(|allowed| allowed == domain)
            }) {
                return Err(CoreError::invalid_data(
                    "SKILL_RECIPE_DOMAIN_INVALID",
                    "Skill Recipe and Skill domain policy do not overlap.",
                ));
            }
        }
        if !manifest
            .material_preset_ids
            .iter()
            .all(|id| MATERIAL_PRESET_IDS.contains(&id.as_str()))
        {
            return Err(CoreError::invalid_data(
                "SKILL_MATERIAL_POLICY_INVALID",
                "Skill names a material outside the code-owned visual material catalog.",
            ));
        }
        Ok(())
    }

    pub fn read_object(&self, sha256: &str) -> CoreResult<Vec<u8>> {
        let record = self
            .object(sha256)?
            .ok_or_else(|| CoreError::not_found("content object"))?;
        self.object_store.read(&StoredObject {
            sha256: record.sha256,
            relative_path: record.object_path,
            extension: record.extension,
            byte_size: record.byte_size,
        })
    }

    pub fn detach_object(&self, reference: &ObjectReference) -> CoreResult<()> {
        reference.validate()?;
        self.write(|transaction| {
            transaction.execute(
                "DELETE FROM forgecad_core_object_references WHERE reference_kind=? AND owner_id=? AND role=?",
                params![reference.reference_kind, reference.owner_id, reference.role],
            )?;
            Ok(())
        })
    }

    pub fn collect_unreferenced_objects(&self) -> CoreResult<Vec<String>> {
        // A previous interrupted collection must be resolved before another
        // batch can reuse any of its SHA identities.
        self.recover_object_deletions()?;
        let records = self.write(|transaction| {
            let mut statement = transaction.prepare(
                "SELECT sha256, object_path, extension, byte_size, ref_count, created_at, updated_at FROM forgecad_core_objects WHERE ref_count=0 ORDER BY sha256",
            )?;
            let records = statement
                .query_map([], object_record_from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            for record in &records {
                transaction.execute(
                    "INSERT INTO forgecad_core_object_deletion_journal(sha256, object_path, extension, byte_size, created_at) VALUES (?, ?, ?, ?, ?)",
                    params![
                        record.sha256,
                        record.object_path,
                        record.extension,
                        record.byte_size,
                        record.updated_at,
                    ],
                )?;
            }
            transaction.execute("DELETE FROM forgecad_core_objects WHERE ref_count=0", [])?;
            Ok(records)
        })?;
        let expected = records
            .iter()
            .map(|record| record.sha256.clone())
            .collect::<Vec<_>>();
        let collected = self.recover_object_deletions()?;
        if collected != expected {
            return Err(CoreError::conflict(
                "CONTENT_OBJECT_DELETION_READBACK_MISMATCH",
                "CAS deletion recovery did not match the exact journaled collection batch.",
            ));
        }
        Ok(collected)
    }

    fn idempotent_write<T>(
        &self,
        scope: &str,
        key: &str,
        request_hash: &str,
        created_at: &str,
        operation: impl FnOnce(&Transaction<'_>) -> CoreResult<T>,
    ) -> CoreResult<T>
    where
        T: serde::Serialize + DeserializeOwned,
    {
        validate_idempotency_identity(scope, key, request_hash)?;
        self.write(|transaction| {
            let replay: Option<(String, String)> = transaction
                .query_row(
                    "SELECT request_hash, response_json FROM idempotency_records WHERE scope=? AND idempotency_key=?",
                    params![scope, key],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((stored_hash, response_json)) = replay {
                if stored_hash != request_hash {
                    return Err(CoreError::conflict(
                        "IDEMPOTENCY_CONFLICT",
                        "Idempotency-Key was already used for a different sealed request.",
                    ));
                }
                return serde_json::from_str(&response_json).map_err(|_| {
                    CoreError::invalid_data(
                        "IDEMPOTENCY_REPLAY_INVALID",
                        "Persisted idempotency response cannot be decoded by the Rust core.",
                    )
                });
            }
            let result = operation(transaction)?;
            transaction.execute(
                "INSERT INTO idempotency_records(scope, idempotency_key, request_hash, response_json, created_at) VALUES (?, ?, ?, ?, ?)",
                params![scope, key, request_hash, json_text(&result)?, created_at],
            )?;
            Ok(result)
        })
    }

    pub(crate) fn write<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> CoreResult<T>,
    ) -> CoreResult<T> {
        // Any repository mutation is an externally meaningful Rust write and
        // makes the first ownership cutover irreversible.
        self.lease.publish()?;
        let mut connection = open_connection(self.db_path())?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        self.lease.assert_current(&transaction)?;
        let result = operation(&transaction)?;
        transaction.commit()?;
        Ok(result)
    }

    fn bootstrap_write<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> CoreResult<T>,
    ) -> CoreResult<T> {
        let mut connection = open_connection(self.db_path())?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        self.lease.assert_current(&transaction)?;
        let result = operation(&transaction)?;
        transaction.commit()?;
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MaterialTextureRow {
    texture_asset_id: String,
    texture_role: MaterialTextureRole,
    display_name: String,
    mime_type: String,
    byte_size: u64,
    sha256: String,
    object_path: String,
    width: u32,
    height: u32,
    source: MaterialTextureSource,
    license: MaterialTextureLicense,
    license_ref: Option<String>,
    thumbnail_asset_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl MaterialTextureRow {
    fn into_model(self, object_exists: bool) -> MaterialTextureObject {
        MaterialTextureObject {
            schema_version: "MaterialTextureObject@1".into(),
            texture_asset_id: self.texture_asset_id,
            texture_role: self.texture_role,
            display_name: self.display_name,
            mime_type: self.mime_type,
            byte_size: self.byte_size,
            sha256: self.sha256,
            object_path: self.object_path,
            width: self.width,
            height: self.height,
            source: self.source,
            license: self.license,
            license_ref: self.license_ref,
            thumbnail_asset_id: self.thumbnail_asset_id,
            visual_only: true,
            object_exists,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

fn material_texture_row_optional<P: rusqlite::Params>(
    connection: &Connection,
    predicate: &str,
    parameters: P,
) -> CoreResult<Option<MaterialTextureRow>> {
    let sql = format!(
        "SELECT texture_asset_id, texture_role, display_name, mime_type, byte_size, sha256, object_path, width, height, source, license, license_ref, thumbnail_asset_id, created_at, updated_at FROM agent_material_texture_objects WHERE {predicate}"
    );
    connection
        .query_row(&sql, parameters, material_texture_row_from_sql)
        .optional()
        .map_err(Into::into)
}

fn material_texture_row_from_sql(row: &Row<'_>) -> rusqlite::Result<MaterialTextureRow> {
    let texture_role = row.get::<_, String>(1)?;
    let source = row.get::<_, String>(9)?;
    let license = row.get::<_, String>(10)?;
    Ok(MaterialTextureRow {
        texture_asset_id: row.get(0)?,
        texture_role: MaterialTextureRole::from_str(&texture_role).map_err(to_sql_error)?,
        display_name: row.get(2)?,
        mime_type: row.get(3)?,
        byte_size: row.get(4)?,
        sha256: row.get(5)?,
        object_path: row.get(6)?,
        width: row.get(7)?,
        height: row.get(8)?,
        source: MaterialTextureSource::from_str(&source).map_err(to_sql_error)?,
        license: MaterialTextureLicense::from_str(&license).map_err(to_sql_error)?,
        license_ref: row.get(11)?,
        thumbnail_asset_id: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn material_texture_metadata_matches(
    row: &MaterialTextureRow,
    request: &RegisterMaterialTextureRequest,
    stored: &StoredObject,
    width: u32,
    height: u32,
    legacy_object_path: &str,
) -> bool {
    row.texture_asset_id == format!("asset_tex_{}", &stored.sha256[..24])
        && row.texture_role == request.texture_role
        && row.mime_type == request.mime_type
        && row.byte_size == stored.byte_size
        && row.sha256 == stored.sha256
        && row.object_path == legacy_object_path
        && row.width == width
        && row.height == height
        && row.source == request.source
        && row.license == request.license
        && row.license_ref == request.license_ref
        && row.thumbnail_asset_id == request.thumbnail_asset_id
}

fn insert_version(transaction: &Transaction<'_>, version: &AgentAssetVersion) -> CoreResult<()> {
    transaction.execute(
        "INSERT INTO agent_asset_versions(asset_version_id, project_id, parent_asset_version_id, version_no, status, summary, stage, plan_id, direction_id, domain_pack_id, artifact_id, parts_json, shape_program_json, assembly_graph_json, material_bindings_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            version.asset_version_id,
            version.project_id,
            version.parent_asset_version_id,
            version.version_no,
            version.status.as_str(),
            version.summary,
            version.stage.as_str(),
            version.plan_id,
            version.direction_id,
            version.domain_pack_id,
            version.artifact_id,
            json_text(&version.parts)?,
            json_text(&version.shape_program)?,
            json_text(&version.assembly_graph)?,
            json_text(&version.material_bindings)?,
            version.created_at,
        ],
    )?;
    Ok(())
}

fn imported_glb_from_connection(
    connection: &Connection,
    asset_version_id: &str,
) -> CoreResult<Option<ImportedGlbRecord>> {
    connection
        .query_row(
            "SELECT import_id, project_id, asset_version_id, domain_pack_id, file_name, object_path, sha256, byte_size, triangle_count, bounds_mm_json, mesh_count, primitive_count, material_count, node_count, created_at FROM agent_imported_glbs WHERE asset_version_id=?",
            [asset_version_id],
            |row| {
                let bounds_json: String = row.get(9)?;
                let bounds_mm = serde_json::from_str::<[f64; 3]>(&bounds_json)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(ImportedGlbRecord {
                    import_id: row.get(0)?,
                    project_id: row.get(1)?,
                    asset_version_id: row.get(2)?,
                    domain_pack_id: row.get(3)?,
                    file_name: row.get(4)?,
                    object_path: row.get(5)?,
                    sha256: row.get(6)?,
                    byte_size: row.get(7)?,
                    triangle_count: row.get(8)?,
                    bounds_mm,
                    mesh_count: row.get(10)?,
                    primitive_count: row.get(11)?,
                    material_count: row.get(12)?,
                    node_count: row.get(13)?,
                    created_at: row.get(14)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
        .and_then(|record| {
            record
                .map(|record| {
                    record.validate()?;
                    Ok(record)
                })
                .transpose()
        })
}

fn promote_legacy_snapshot_for_import(
    transaction: &Transaction<'_>,
    snapshot: &ActiveDesignSnapshot,
    version: &AgentAssetVersion,
    updated_at: &str,
) -> CoreResult<()> {
    let ActiveDesign::LegacyConceptReadOnly {
        legacy_version_id,
        module_graph_id,
        ..
    } = &snapshot.active_design
    else {
        return Err(CoreError::conflict(
            "ACTIVE_DESIGN_INVALID",
            "Only a read-only legacy Snapshot can enter the explicit import promotion path.",
        ));
    };
    let intent = legacy_conversion_intent_from_connection(transaction, &version.project_id)?
        .ok_or_else(|| {
            CoreError::conflict(
                "LEGACY_CONVERSION_NOT_AUTHORIZED",
                "Authorize the exact read-only legacy Snapshot before importing an Agent reference asset.",
            )
        })?;
    if intent.snapshot_revision != snapshot.revision
        || intent.legacy_version_id != *legacy_version_id
        || intent.legacy_module_graph_id != *module_graph_id
    {
        return Err(CoreError::conflict(
            "LEGACY_CONVERSION_AUTHORIZATION_STALE",
            "Legacy conversion authorization no longer matches the active Snapshot.",
        ));
    }
    require_legacy_source_binding(
        transaction,
        &version.project_id,
        legacy_version_id,
        module_graph_id,
    )?;
    let render =
        RenderPreset::default_for(&version.project_id, &version.asset_version_id, updated_at);
    let part_display = PartDisplay::empty(&version.project_id, &version.asset_version_id);
    let changed = transaction.execute(
        "UPDATE active_design_snapshots SET source='agent_asset', active_asset_version_id=?, active_assembly_graph_id=?, legacy_version_id=NULL, legacy_module_graph_id=NULL, selected_part_id=NULL, selected_material_zone_id=NULL, preview_change_set_id=NULL, preview_base_asset_version_id=NULL, quality_report_id=NULL, quality_asset_version_id=NULL, export_source='agent_asset', export_source_version_id=?, render_preset_json=?, part_display_json=?, revision=revision+1, updated_at=? WHERE project_id=? AND source='legacy_concept_read_only' AND legacy_version_id=? AND legacy_module_graph_id=? AND revision=?",
        params![
            version.asset_version_id,
            version.assembly_graph_id()?,
            version.asset_version_id,
            json_text(&render)?,
            json_text(&part_display)?,
            updated_at,
            version.project_id,
            legacy_version_id,
            module_graph_id,
            snapshot.revision,
        ],
    )?;
    require_changed(changed)?;
    transaction.execute(
        "DELETE FROM legacy_agent_conversion_intents WHERE project_id=? AND legacy_version_id=? AND legacy_module_graph_id=? AND snapshot_revision=?",
        params![
            version.project_id,
            legacy_version_id,
            module_graph_id,
            snapshot.revision,
        ],
    )?;
    Ok(())
}

fn is_registered_import_domain_pack(domain_pack_id: &str) -> bool {
    matches!(
        domain_pack_id,
        "pack_future_weapon_prop"
            | "pack_vehicle_concept"
            | "pack_aircraft_concept"
            | "pack_robotic_arm_concept"
    )
}

fn insert_change_set(
    transaction: &Transaction<'_>,
    change_set: &AgentAssetChangeSet,
) -> CoreResult<()> {
    transaction.execute(
        "INSERT INTO agent_asset_change_sets(change_set_id, project_id, base_asset_version_id, summary, operations_json, protected_part_ids_json, preview_json, status, resulting_asset_version_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, NULL, 'proposed', NULL, ?, ?)",
        params![
            change_set.change_set_id,
            change_set.project_id,
            change_set.base_asset_version_id,
            change_set.summary,
            json_text(&change_set.operations)?,
            json_text(&change_set.protected_part_ids)?,
            change_set.created_at,
            change_set.updated_at,
        ],
    )?;
    Ok(())
}

fn require_internal_editable_asset(
    connection: &Connection,
    version: &AgentAssetVersion,
) -> CoreResult<()> {
    let external: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM forgecad_core_object_references WHERE reference_kind='asset_version' AND owner_id=? AND role='external_reference_glb') OR EXISTS(SELECT 1 FROM agent_imported_glbs WHERE asset_version_id=?)",
        params![version.asset_version_id, version.asset_version_id],
        |row| row.get(0),
    )?;
    if external {
        return Err(CoreError::conflict(
            "EXTERNAL_REFERENCE_NOT_EDITABLE",
            "Imported GLB assets remain read-only references and cannot enter component or structure edits.",
        ));
    }
    Ok(())
}

fn require_current_editable_asset(
    connection: &Connection,
    version: &AgentAssetVersion,
) -> CoreResult<ActiveDesignSnapshot> {
    require_internal_editable_asset(connection, version)?;
    let snapshot = require_snapshot(connection, &version.project_id)?;
    let head: Option<String> = connection
        .query_row(
            "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
            [&version.project_id],
            |row| row.get(0),
        )
        .optional()?;
    if version.status != AssetVersionStatus::Committed
        || snapshot.active_design.asset_version_id() != Some(version.asset_version_id.as_str())
        || head.as_deref() != Some(version.asset_version_id.as_str())
    {
        return Err(CoreError::conflict(
            "ACTIVE_DESIGN_STALE",
            "Component and structure operations require the current active Rust-owned asset.",
        ));
    }
    Ok(snapshot)
}

fn derive_component_snapshot(
    version: &AgentAssetVersion,
    part_id: &str,
) -> CoreResult<(
    serde_json::Value,
    serde_json::Value,
    std::collections::BTreeMap<String, serde_json::Value>,
)> {
    let part = version
        .parts
        .iter()
        .find(|part| part.get("part_id").and_then(serde_json::Value::as_str) == Some(part_id))
        .cloned()
        .ok_or_else(|| CoreError::not_found("Agent asset part"))?;
    let role = part
        .get("role")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PART_ROLE_INVALID",
                "Component source part has no stable role.",
            )
        })?;
    let graph_part = version
        .assembly_graph
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("part_id").and_then(serde_json::Value::as_str) == Some(part_id))
        .ok_or_else(|| CoreError::not_found("AssemblyGraph part"))?;
    if graph_part
        .get("geometry_source")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|source| source != "shape_program")
    {
        return Err(CoreError::conflict(
            "PART_GEOMETRY_NOT_FOUND",
            "Only a bounded ShapeProgram part can be saved as a reusable component.",
        ));
    }
    let operations = version
        .shape_program
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "SHAPE_PROGRAM_OPERATIONS_INVALID",
                "ShapeProgram operations are unavailable.",
            )
        })?;
    let operation = if let Some(operation_id) = graph_part
        .get("operation_id")
        .and_then(serde_json::Value::as_str)
    {
        operations
            .iter()
            .find(|operation| {
                operation
                    .get("operation_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(operation_id)
            })
            .cloned()
    } else {
        let matches = operations
            .iter()
            .filter(|operation| {
                operation
                    .get("args")
                    .and_then(|args| args.get("part_role"))
                    .and_then(serde_json::Value::as_str)
                    == Some(role)
            })
            .cloned()
            .collect::<Vec<_>>();
        (matches.len() == 1).then(|| matches[0].clone())
    }
    .ok_or_else(|| {
        CoreError::conflict(
            "PART_GEOMETRY_NOT_FOUND",
            "The source part does not have one stable reusable ShapeProgram operation.",
        )
    })?;
    if operation
        .get("args")
        .and_then(|args| args.get("part_role"))
        .and_then(serde_json::Value::as_str)
        != Some(role)
    {
        return Err(CoreError::conflict(
            "PART_GEOMETRY_ROLE_MISMATCH",
            "The source part role differs from its ShapeProgram operation.",
        ));
    }
    if operation
        .get("inputs")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|inputs| !inputs.is_empty())
    {
        return Err(CoreError::conflict(
            "PART_GEOMETRY_NOT_REUSABLE",
            "A project-local component must be one self-contained bounded ShapeProgram operation.",
        ));
    }
    let prefix = format!("{part_id}:");
    let material_bindings = version
        .material_bindings
        .iter()
        .filter(|(key, _)| key.starts_with(&prefix))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    Ok((part, operation, material_bindings))
}

fn component_from_connection(
    connection: &Connection,
    component_id: &str,
) -> CoreResult<Option<AgentComponentRecord>> {
    let row = connection
        .query_row(
            "SELECT component_id, project_id, domain_pack_id, role, display_name, description, source_asset_version_id, source_part_id, part_template_json, shape_operation_json, material_bindings_json, status, created_at, updated_at FROM agent_components WHERE component_id=?",
            [component_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                ))
            },
        )
        .optional()?;
    let Some((
        component_id,
        project_id,
        domain_pack_id,
        role,
        display_name,
        description,
        source_asset_version_id,
        source_part_id,
        part_template_json,
        shape_operation_json,
        material_bindings_json,
        status,
        created_at,
        updated_at,
    )) = row
    else {
        return Ok(None);
    };
    let component = AgentComponentRecord {
        schema_version: "AgentComponent@1".into(),
        component_id,
        project_id,
        domain_pack_id,
        role,
        display_name,
        description,
        source_asset_version_id: source_asset_version_id.clone(),
        source_part_id,
        part_template: parse_json(part_template_json)?,
        shape_operation: parse_json(shape_operation_json)?,
        material_bindings: parse_json(material_bindings_json)?,
        status,
        source_quality_status: source_quality_status_from_connection(
            connection,
            &source_asset_version_id,
        )?,
        created_at,
        updated_at,
    };
    component.validate()?;
    Ok(Some(component))
}

fn source_quality_status_from_connection(
    connection: &Connection,
    asset_version_id: &str,
) -> CoreResult<QualityStatus> {
    let latest = connection
        .query_row(
            "SELECT quality_report_id, project_id, report_json, status, created_at FROM agent_asset_quality_reports WHERE asset_version_id=? ORDER BY created_at DESC, quality_report_id DESC LIMIT 1",
            [asset_version_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((quality_report_id, project_id, report_json, status, created_at)) = latest else {
        return Ok(QualityStatus::Unavailable);
    };
    let status = QualityStatus::from_str(&status)?;
    if status == QualityStatus::Failed {
        return Ok(QualityStatus::Failed);
    }
    if status == QualityStatus::Unavailable {
        return Ok(QualityStatus::Unavailable);
    }
    let report: serde_json::Value = match parse_json(report_json) {
        Ok(report) => report,
        Err(_) => return Ok(QualityStatus::Unavailable),
    };
    let version = match version_from_connection(connection, asset_version_id)? {
        Some(version) => version,
        None => return Ok(QualityStatus::Unavailable),
    };
    let production: Option<(String, u64)> = connection
        .query_row(
            "SELECT o.sha256, o.byte_size FROM forgecad_core_object_references r JOIN forgecad_core_objects o ON o.sha256=r.sha256 WHERE r.reference_kind='asset_version' AND r.owner_id=? AND r.role='production_glb'",
            [asset_version_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((production_sha, production_size)) = production else {
        return Ok(QualityStatus::Unavailable);
    };
    let compile = report.get("compile_readback");
    let status_text = status.as_str();
    let exact = report
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        == Some("AgentAssetQualityReport@1")
        && report
            .get("quality_report_id")
            .and_then(serde_json::Value::as_str)
            == Some(quality_report_id.as_str())
        && report
            .get("asset_version_id")
            .and_then(serde_json::Value::as_str)
            == Some(asset_version_id)
        && report.get("status").and_then(serde_json::Value::as_str) == Some(status_text)
        && report
            .get("evidence_source")
            .and_then(serde_json::Value::as_str)
            == Some("geometry_compile_readback")
        && compile
            .and_then(|value| value.get("schema_version"))
            .and_then(serde_json::Value::as_str)
            == Some("GeometryCompileReadback@2")
        && compile
            .and_then(|value| value.get("artifact_profile"))
            .and_then(|value| value.get("artifact_profile_id"))
            .and_then(serde_json::Value::as_str)
            == Some("production_concept")
        && compile
            .and_then(|value| value.get("shape_program_sha256"))
            .and_then(serde_json::Value::as_str)
            == Some(crate::semantic_sha256(&version.shape_program)?.as_str())
        && compile
            .and_then(|value| value.get("glb_sha256"))
            .and_then(serde_json::Value::as_str)
            == Some(production_sha.as_str())
        && compile
            .and_then(|value| value.get("glb_byte_size"))
            .and_then(serde_json::Value::as_u64)
            == Some(production_size)
        && compile
            .and_then(|value| value.get("closed_manifold"))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        && compile
            .and_then(|value| value.get("surface_provenance_present"))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        && project_id == version.project_id
        && !created_at.is_empty();
    Ok(if exact {
        status
    } else {
        QualityStatus::Unavailable
    })
}

fn component_candidate(
    version: &AgentAssetVersion,
    target_part_id: &str,
    target_role: &str,
    component: AgentComponentRecord,
) -> AgentComponentCandidate {
    let mut eligible = true;
    let mut reasons = Vec::new();
    if component.status == "active" {
        reasons.push("component_active".into());
    } else {
        reasons.push("component_disabled".into());
        eligible = false;
    }
    if component.domain_pack_id == version.domain_pack_id {
        reasons.push("same_domain_pack".into());
    } else {
        reasons.push("domain_pack_mismatch".into());
        eligible = false;
    }
    if component.role == target_role {
        reasons.push("same_role".into());
    } else {
        reasons.push("role_mismatch".into());
        eligible = false;
    }
    reasons.push(format!(
        "source_quality_{}",
        component.source_quality_status.as_str()
    ));
    if !matches!(
        component.source_quality_status,
        QualityStatus::Passed | QualityStatus::Warning
    ) {
        eligible = false;
    }
    // Replacement keeps the target identity and AssemblyGraph location, so
    // known target connectors remain unchanged; no connector is invented from
    // the project-local component snapshot.
    reasons.push("target_connectors_preserved".into());
    AgentComponentCandidate {
        schema_version: "AgentComponentCandidate@1".into(),
        compatibility: AgentComponentCompatibility {
            schema_version: "AgentComponentCompatibility@1".into(),
            component_id: component.component_id.clone(),
            target_asset_version_id: version.asset_version_id.clone(),
            target_part_id: target_part_id.to_string(),
            eligible,
            source_quality_status: component.source_quality_status,
            reason_codes: reasons,
        },
        component,
    }
}

fn require_component_candidate_eligible(candidate: &AgentComponentCandidate) -> CoreResult<()> {
    if candidate.compatibility.eligible {
        return Ok(());
    }
    if !matches!(
        candidate.compatibility.source_quality_status,
        QualityStatus::Passed | QualityStatus::Warning
    ) {
        return Err(CoreError::conflict(
            "COMPONENT_QUALITY_NOT_READY",
            "The component source asset does not have current passed or warning production readback.",
        ));
    }
    Err(CoreError::conflict(
        "COMPONENT_INCOMPATIBLE",
        "The component is disabled or does not match the current project domain and stable role.",
    ))
}

const SPLITTABLE_PRIMITIVES: [&str; 4] = ["box", "cylinder", "capsule", "wedge"];

fn derive_structure_suggestions(
    version: &AgentAssetVersion,
    snapshot: &ActiveDesignSnapshot,
) -> CoreResult<Vec<AgentStructureSuggestion>> {
    let graph_parts = version
        .assembly_graph
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "ASSEMBLY_GRAPH_PARTS_INVALID",
                "AssemblyGraph must contain stable parts before structure suggestions.",
            )
        })?;
    let graph_by_id = graph_parts
        .iter()
        .filter_map(|part| {
            part.get("part_id")
                .and_then(serde_json::Value::as_str)
                .map(|part_id| (part_id, part))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let parts_by_id = version
        .parts
        .iter()
        .filter_map(|part| {
            part.get("part_id")
                .and_then(serde_json::Value::as_str)
                .map(|part_id| (part_id, part))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if graph_by_id.len() != graph_parts.len() || parts_by_id.len() != version.parts.len() {
        return Err(CoreError::invalid_data(
            "ASSEMBLY_GRAPH_PART_ID_INVALID",
            "Structure suggestions require one stable identity per part.",
        ));
    }
    let mut role_counts = std::collections::BTreeMap::<String, usize>::new();
    for part in &version.parts {
        if let Some(role) = part.get("role").and_then(serde_json::Value::as_str) {
            *role_counts.entry(role.to_string()).or_default() += 1;
        }
    }
    let operations = version
        .shape_program
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let operations_by_id = operations
        .iter()
        .filter_map(|operation| {
            operation
                .get("operation_id")
                .and_then(serde_json::Value::as_str)
                .map(|operation_id| (operation_id, operation))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let outputs = version
        .shape_program
        .get("outputs")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut outputs_by_role = std::collections::BTreeMap::<String, Vec<String>>::new();
    for output in &outputs {
        if let (Some(role), Some(operation_id)) = (
            output.get("part_role").and_then(serde_json::Value::as_str),
            output
                .get("operation_id")
                .and_then(serde_json::Value::as_str),
        ) {
            outputs_by_role
                .entry(role.to_string())
                .or_default()
                .push(operation_id.to_string());
        }
    }
    let connections = version
        .assembly_graph
        .get("connections")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|connection| {
            connection
                .get("from_part_id")
                .and_then(serde_json::Value::as_str)
                .is_some()
                && connection
                    .get("to_part_id")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
        })
        .collect::<Vec<_>>();
    let mut child_count = std::collections::BTreeMap::<String, usize>::new();
    for graph_part in graph_parts {
        if let Some(parent_id) = graph_part
            .get("parent_part_id")
            .and_then(serde_json::Value::as_str)
        {
            *child_count.entry(parent_id.to_string()).or_default() += 1;
        }
    }
    let locked = snapshot
        .part_display
        .as_ref()
        .map(|display| display.locked_part_ids.iter().map(String::as_str).collect())
        .unwrap_or_else(BTreeSet::new);
    let mut suggestions = Vec::new();
    for part in &version.parts {
        let Some(part_id) = part.get("part_id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(role) = part.get("role").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(graph_part) = graph_by_id.get(part_id).copied() else {
            continue;
        };
        if locked.contains(part_id)
            || part.get("locked").and_then(serde_json::Value::as_bool) == Some(true)
            || graph_part
                .get("locked")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        {
            continue;
        }
        let primitive_outputs = outputs_by_role
            .get(role)
            .into_iter()
            .flatten()
            .filter(|operation_id| {
                operations_by_id
                    .get(operation_id.as_str())
                    .is_some_and(|operation| {
                        operation
                            .get("op")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|kind| SPLITTABLE_PRIMITIVES.contains(&kind))
                            && operation
                                .get("inputs")
                                .and_then(serde_json::Value::as_array)
                                .is_none_or(Vec::is_empty)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        let connected = connections.iter().any(|connection| {
            connection
                .get("from_part_id")
                .and_then(serde_json::Value::as_str)
                == Some(part_id)
                || connection
                    .get("to_part_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(part_id)
        });
        let has_joints = graph_part
            .get("joints")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|joints| !joints.is_empty());
        if role_counts.get(role) == Some(&1)
            && primitive_outputs.len() >= 2
            && !connected
            && !has_joints
            && child_count.get(part_id).copied().unwrap_or(0) == 0
        {
            let target = primitive_outputs.last().unwrap();
            let suggestion = AgentStructureSuggestion {
                schema_version: "AgentStructureSuggestion@1".into(),
                suggestion_id: structure_suggestion_id(
                    &version.asset_version_id,
                    "split_part",
                    part_id,
                    target,
                ),
                kind: "split_part".into(),
                asset_version_id: version.asset_version_id.clone(),
                part_id: part_id.to_string(),
                target_part_id: None,
                affected_part_ids: vec![part_id.to_string()],
                source_facts: vec![
                    "independent_shape_outputs".into(),
                    "no_connection_or_joint".into(),
                    "no_child_parts".into(),
                ],
                summary: "将这个部件拆成两个可单独调整的外观部件".into(),
            };
            suggestion.validate()?;
            suggestions.push(suggestion);
        }
    }
    for connection in &connections {
        let parent_id = connection
            .get("from_part_id")
            .and_then(serde_json::Value::as_str)
            .unwrap();
        let child_id = connection
            .get("to_part_id")
            .and_then(serde_json::Value::as_str)
            .unwrap();
        let (Some(parent), Some(child), Some(parent_graph), Some(child_graph)) = (
            parts_by_id.get(parent_id).copied(),
            parts_by_id.get(child_id).copied(),
            graph_by_id.get(parent_id).copied(),
            graph_by_id.get(child_id).copied(),
        ) else {
            continue;
        };
        let parent_role = parent.get("role").and_then(serde_json::Value::as_str);
        let child_role = child.get("role").and_then(serde_json::Value::as_str);
        let Some((parent_role, child_role)) = parent_role.zip(child_role) else {
            continue;
        };
        let locked_part = |part_id: &str, part: &serde_json::Value, graph: &serde_json::Value| {
            locked.contains(part_id)
                || part.get("locked").and_then(serde_json::Value::as_bool) == Some(true)
                || graph.get("locked").and_then(serde_json::Value::as_bool) == Some(true)
        };
        if locked_part(parent_id, parent, parent_graph) || locked_part(child_id, child, child_graph)
        {
            continue;
        }
        let child_has_joints = child_graph
            .get("joints")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|joints| !joints.is_empty());
        let parent_joint_targets_child = parent_graph
            .get("joints")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|joints| {
                joints.iter().any(|joint| {
                    joint
                        .get("target_part_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(child_id)
                })
            });
        let child_connections = connections
            .iter()
            .filter(|item| {
                item.get("from_part_id").and_then(serde_json::Value::as_str) == Some(child_id)
                    || item.get("to_part_id").and_then(serde_json::Value::as_str) == Some(child_id)
            })
            .count();
        let child_outputs = outputs_by_role.get(child_role).cloned().unwrap_or_default();
        let child_outputs_supported = !child_outputs.is_empty()
            && child_outputs.iter().all(|operation_id| {
                operations_by_id
                    .get(operation_id.as_str())
                    .and_then(|operation| operation.get("op"))
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|kind| SPLITTABLE_PRIMITIVES.contains(&kind))
            });
        if !child_has_joints
            && !parent_joint_targets_child
            && child_count.get(child_id).copied().unwrap_or(0) == 0
            && child_connections == 1
            && role_counts.get(parent_role) == Some(&1)
            && role_counts.get(child_role) == Some(&1)
            && child_outputs_supported
        {
            let suggestion = AgentStructureSuggestion {
                schema_version: "AgentStructureSuggestion@1".into(),
                suggestion_id: structure_suggestion_id(
                    &version.asset_version_id,
                    "merge_parts",
                    parent_id,
                    child_id,
                ),
                kind: "merge_parts".into(),
                asset_version_id: version.asset_version_id.clone(),
                part_id: parent_id.to_string(),
                target_part_id: Some(child_id.to_string()),
                affected_part_ids: vec![parent_id.to_string(), child_id.to_string()],
                source_facts: vec![
                    "direct_leaf_connection".into(),
                    "leaf_has_no_joint".into(),
                    "independent_shape_output".into(),
                ],
                summary: "将这两个已连接的外观部件合并为一个可编辑部件".into(),
            };
            suggestion.validate()?;
            suggestions.push(suggestion);
        }
    }
    Ok(suggestions)
}

fn structure_suggestion_id(
    asset_version_id: &str,
    kind: &str,
    part_id: &str,
    target: &str,
) -> String {
    let digest = crate::canonical::sha256_bytes(
        format!("{asset_version_id}|{kind}|{part_id}|{target}").as_bytes(),
    );
    format!("structure_{kind}_{}", &digest[..18])
}

fn validate_change_set_context(
    transaction: &Transaction<'_>,
    change_set: &AgentAssetChangeSet,
) -> CoreResult<()> {
    let snapshot = require_snapshot(transaction, &change_set.project_id)?;
    let active = snapshot.active_design.asset_version_id().ok_or_else(|| {
        CoreError::conflict(
            "ACTIVE_DESIGN_LEGACY_READ_ONLY",
            "Legacy designs cannot accept Agent ChangeSets.",
        )
    })?;
    let head = require_head(transaction, &change_set.project_id)?;
    if active != change_set.base_asset_version_id || head != change_set.base_asset_version_id {
        return Err(stale("CHANGE_SET_BASE_STALE"));
    }
    let version = require_version(transaction, &change_set.base_asset_version_id)?;
    require_internal_editable_asset(transaction, &version)?;
    validate_change_set_operations(transaction, &version, &snapshot, change_set)
}

/// Decode and deterministically replay the sealed C105 Recipe replacement
/// reference.  The expanded candidate is deliberately not persisted: it is a
/// pure function of the reviewed registry plus the exact active Version and
/// Snapshot revision.  Calling this from both proposal validation and preview
/// sealing prevents a stale, forged, cross-domain, or expired in-memory
/// candidate from becoming a version.
fn place_recipe_candidate_in_active_context(
    candidate: &mut ExpandedComponentCandidate,
    request: &RecipeInstantiationRequest,
    version: &AgentAssetVersion,
    snapshot: &ActiveDesignSnapshot,
    protected_part_ids: &[String],
) -> CoreResult<()> {
    let target_part_id = request.target_part_id.as_deref().ok_or_else(|| {
        CoreError::invalid_data(
            "COMPONENT_RECIPE_CONTEXT_INVALID",
            "Active Recipe placement requires a stable target Part.",
        )
    })?;
    let target = version
        .parts
        .iter()
        .find(|part| {
            part.get("part_id").and_then(serde_json::Value::as_str) == Some(target_part_id)
        })
        .ok_or_else(|| CoreError::not_found("Agent asset part"))?;
    let graph_parts = version
        .assembly_graph
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "ASSEMBLY_GRAPH_PARTS_INVALID",
                "Recipe replacement requires stable AssemblyGraph parts.",
            )
        })?;
    let target_graph_part = graph_parts
        .iter()
        .find(|part| {
            part.get("part_id").and_then(serde_json::Value::as_str) == Some(target_part_id)
        })
        .ok_or_else(|| {
            CoreError::invalid_data(
                "ASSEMBLY_GRAPH_PARTS_INVALID",
                "Recipe replacement target is absent from the stable AssemblyGraph.",
            )
        })?;
    let subtree = recipe_target_subtree(graph_parts, target_part_id);
    recipe_target_must_be_unprotected(version, snapshot, &subtree, protected_part_ids)?;

    let root_part_id = candidate
        .expanded_assembly_graph
        .get("root_part_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate AssemblyGraph has no root Part.",
            )
        })?
        .to_owned();
    let candidate_parts = candidate
        .expanded_assembly_graph
        .get_mut("parts")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate AssemblyGraph has no parts.",
            )
        })?;
    let root_part = candidate_parts
        .iter_mut()
        .find(|part| {
            part.get("part_id").and_then(serde_json::Value::as_str) == Some(root_part_id.as_str())
        })
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate AssemblyGraph root Part is absent.",
            )
        })?;
    let candidate_root_instance_id = root_part
        .get("recipe_instance_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate root Part has no durable instance identity.",
            )
        })?
        .to_owned();
    let candidate_root_operation_id = root_part
        .get("operation_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate root Part has no ShapeProgram operation identity.",
            )
        })?
        .to_owned();
    let root_role = root_part.get("role").and_then(serde_json::Value::as_str);
    if root_role != target.get("role").and_then(serde_json::Value::as_str) {
        return Err(CoreError::conflict(
            "COMPONENT_INCOMPATIBLE",
            "Reviewed Recipe root role does not match the replacement target role.",
        ));
    }
    let target_transform = target_graph_part.get("transform").ok_or_else(|| {
        CoreError::invalid_data(
            "RECIPE_REPLACEMENT_TRANSFORM_UNSUPPORTED",
            "Stable target Part has no translation-only transform.",
        )
    })?;
    let candidate_transform = root_part.get("transform").ok_or_else(|| {
        CoreError::invalid_data(
            "RECIPE_PREVIEW_GRAPH_INVALID",
            "Recipe candidate root Part has no transform.",
        )
    })?;
    let target_position = recipe_translation_only(target_transform)?;
    let candidate_position = recipe_translation_only(candidate_transform)?;
    let delta = [
        target_position[0] - candidate_position[0],
        target_position[1] - candidate_position[1],
        target_position[2] - candidate_position[2],
    ];

    // The parent is an immutable base-version anchor.  It is intentionally
    // included in the candidate graph and its hash so bridge-only remapping
    // cannot turn a valid candidate into a differently attached asset.
    let parent_anchor = target_graph_part
        .get("parent_part_id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    // A replacement owns the reviewed root Part subtree plus its geometry
    // dependency closure, not unrelated visual outputs which happen to live
    // in the same Recipe file. Required and explicitly selected child slots
    // are real Parts in that reviewed subtree, so their operation roots must
    // survive alongside the root operation. A disconnected template output
    // without a Part remains excluded. This does not relax the bridge's
    // external-reference guard for the active asset; that guard still rejects
    // an old closure consumed by any operation outside the replaced subtree.
    retain_recipe_root_operation_closure(candidate, &candidate_root_operation_id)?;
    translate_recipe_candidate(candidate, delta)?;
    anchor_recipe_candidate_provenance(
        candidate,
        version,
        target_graph_part,
        &candidate_root_instance_id,
    )?;
    let placed_root = candidate
        .expanded_assembly_graph
        .get_mut("parts")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|parts| {
            parts.iter_mut().find(|part| {
                part.get("part_id").and_then(serde_json::Value::as_str)
                    == Some(root_part_id.as_str())
            })
        })
        .expect("validated candidate root remains present after placement");
    placed_root
        .as_object_mut()
        .expect("validated candidate root object")
        .insert("parent_part_id".into(), parent_anchor);
    RecipeExpander::reidentify(candidate, request)
}

fn retain_recipe_root_operation_closure(
    candidate: &mut ExpandedComponentCandidate,
    root_operation_id: &str,
) -> CoreResult<()> {
    let operations = candidate
        .expanded_shape_program
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe candidate ShapeProgram has no operations.",
            )
        })?;
    let operation_inputs = operations
        .iter()
        .map(|operation| {
            let operation_id = operation
                .get("operation_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "RECIPE_PREVIEW_GEOMETRY_INVALID",
                        "Recipe candidate operation has no stable identity.",
                    )
                })?;
            let inputs = operation
                .get("inputs")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "RECIPE_PREVIEW_GEOMETRY_INVALID",
                        "Recipe candidate operation has no bounded inputs.",
                    )
                })?
                .iter()
                .map(|input| {
                    input.as_str().map(str::to_owned).ok_or_else(|| {
                        CoreError::invalid_data(
                            "RECIPE_PREVIEW_GEOMETRY_INVALID",
                            "Recipe candidate operation input has no stable identity.",
                        )
                    })
                })
                .collect::<CoreResult<Vec<_>>>()?;
            Ok((operation_id.to_owned(), inputs))
        })
        .collect::<CoreResult<std::collections::BTreeMap<_, _>>>()?;
    if !operation_inputs.contains_key(root_operation_id) {
        return Err(CoreError::invalid_data(
            "RECIPE_PREVIEW_GRAPH_INVALID",
            "Recipe candidate root Part references an unknown ShapeProgram operation.",
        ));
    }
    let graph_part_operation_ids = candidate
        .expanded_assembly_graph
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate AssemblyGraph has no Parts.",
            )
        })?
        .iter()
        .map(|part| {
            part.get("operation_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "RECIPE_PREVIEW_GRAPH_INVALID",
                        "Recipe candidate Part has no stable ShapeProgram operation.",
                    )
                })
        })
        .collect::<CoreResult<std::collections::BTreeSet<_>>>()?;
    if !graph_part_operation_ids.contains(root_operation_id) {
        return Err(CoreError::invalid_data(
            "RECIPE_PREVIEW_GRAPH_INVALID",
            "Recipe candidate root Part is not represented by its AssemblyGraph operation set.",
        ));
    }
    let graph_instance_operation_prefixes = candidate
        .expanded_assembly_graph
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .expect("validated Recipe candidate AssemblyGraph parts")
        .iter()
        .filter_map(|part| {
            part.get("recipe_instance_id")
                .and_then(serde_json::Value::as_str)
                .map(|instance_id| format!("op_{}_", instance_id.trim_start_matches("recipeinst_")))
        })
        .collect::<std::collections::BTreeSet<_>>();
    // A stable graph Part identifies its primary output, but one reviewed
    // component can emit several independent visual outputs (for example a
    // joint shell, core and outer ring). All of those operations share its
    // instance prefix and must stay with the selected Part subtree.
    let graph_operations = operation_inputs
        .keys()
        .filter(|operation_id| {
            graph_part_operation_ids.contains(*operation_id)
                || graph_instance_operation_prefixes
                    .iter()
                    .any(|prefix| operation_id.starts_with(prefix))
        })
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if graph_operations
        .iter()
        .any(|operation_id| !operation_inputs.contains_key(operation_id))
    {
        return Err(CoreError::invalid_data(
            "RECIPE_PREVIEW_GRAPH_INVALID",
            "Recipe candidate AssemblyGraph references an operation outside its ShapeProgram.",
        ));
    }

    let mut retained = graph_operations;
    let mut pending = retained.iter().cloned().collect::<Vec<_>>();
    while let Some(operation_id) = pending.pop() {
        for input in operation_inputs
            .get(&operation_id)
            .expect("root and dependency operations were checked")
        {
            if !operation_inputs.contains_key(input) {
                return Err(CoreError::invalid_data(
                    "RECIPE_PREVIEW_GEOMETRY_INVALID",
                    "Recipe candidate root closure references an unknown ShapeProgram operation.",
                ));
            }
            if retained.insert(input.clone()) {
                pending.push(input.clone());
            }
        }
    }
    let program = candidate
        .expanded_shape_program
        .as_object_mut()
        .expect("validated ShapeProgram is an object");
    let operations = program
        .get_mut("operations")
        .and_then(serde_json::Value::as_array_mut)
        .expect("validated ShapeProgram retains operations");
    operations.retain(|operation| {
        operation
            .get("operation_id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|operation_id| retained.contains(operation_id))
    });
    if let Some(outputs) = program
        .get_mut("outputs")
        .and_then(serde_json::Value::as_array_mut)
    {
        outputs.retain(|output| {
            output
                .get("operation_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|operation_id| retained.contains(operation_id))
        });
    }
    Ok(())
}

/// The candidate's local `root` path belongs to the Recipe template.  When a
/// reviewed Recipe replaces a non-root active Part, its provenance must retain
/// the immutable target's path/parent/slot rather than introduce a second
/// `root` record.  Child paths remain deterministic below that anchored path.
fn anchor_recipe_candidate_provenance(
    candidate: &mut ExpandedComponentCandidate,
    version: &AgentAssetVersion,
    target_graph_part: &serde_json::Value,
    candidate_root_instance_id: &str,
) -> CoreResult<()> {
    let target_instance_id = target_graph_part
        .get("recipe_instance_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            CoreError::conflict(
                "RECIPE_REPLACEMENT_PROVENANCE_MISSING",
                "A C105 Recipe target must retain a durable Recipe instance identity.",
            )
        })?;
    let target_provenance = version
        .assembly_graph
        .get("component_recipe_instances")
        .and_then(serde_json::Value::as_array)
        .and_then(|instances| {
            instances.iter().find(|instance| {
                instance
                    .get("instance_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(target_instance_id)
            })
        })
        .ok_or_else(|| {
            CoreError::conflict(
                "RECIPE_REPLACEMENT_PROVENANCE_MISSING",
                "A C105 Recipe target has no immutable instance provenance.",
            )
        })?;
    let anchor_path = target_provenance
        .get("instance_path")
        .and_then(serde_json::Value::as_str)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_REPLACEMENT_PROVENANCE_MISSING",
                "A C105 Recipe target has no stable instance path.",
            )
        })?
        .to_owned();
    let anchor_parent_instance_id = target_provenance
        .get("parent_instance_id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let anchor_parent_slot_id = target_provenance
        .get("parent_slot_id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let anchor_parent_instance_id: Option<String> =
        serde_json::from_value(anchor_parent_instance_id).map_err(|_| {
            CoreError::invalid_data(
                "RECIPE_REPLACEMENT_PROVENANCE_MISSING",
                "A C105 Recipe target has an invalid parent instance provenance.",
            )
        })?;
    let anchor_parent_slot_id: Option<String> = serde_json::from_value(anchor_parent_slot_id)
        .map_err(|_| {
            CoreError::invalid_data(
                "RECIPE_REPLACEMENT_PROVENANCE_MISSING",
                "A C105 Recipe target has an invalid parent slot provenance.",
            )
        })?;

    let candidate_paths = candidate
        .component_recipe_instances
        .iter()
        .map(|instance| (instance.instance_id.clone(), instance.instance_path.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    if candidate_paths
        .get(candidate_root_instance_id)
        .map(String::as_str)
        != Some("root")
    {
        return Err(CoreError::invalid_data(
            "RECIPE_PREVIEW_PROVENANCE_MISMATCH",
            "Recipe candidate root provenance must begin at the reviewed local root.",
        ));
    }
    for provenance in &mut candidate.component_recipe_instances {
        let original_path = candidate_paths
            .get(&provenance.instance_id)
            .expect("candidate provenance source is complete");
        provenance.instance_path = if provenance.instance_id == candidate_root_instance_id {
            anchor_path.clone()
        } else {
            let suffix = original_path.strip_prefix("root/").ok_or_else(|| {
                CoreError::invalid_data(
                    "RECIPE_PREVIEW_PROVENANCE_MISMATCH",
                    "Recipe candidate child provenance must descend from the reviewed local root.",
                )
            })?;
            format!("{anchor_path}/{suffix}")
        };
        if provenance.instance_id == candidate_root_instance_id {
            provenance.parent_instance_id = anchor_parent_instance_id.clone();
            provenance.parent_slot_id = anchor_parent_slot_id.clone();
        }
    }
    for instance in &mut candidate.instances {
        let provenance = candidate
            .component_recipe_instances
            .iter()
            .find(|provenance| provenance.instance_id == instance.instance_id)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "RECIPE_PREVIEW_PROVENANCE_MISMATCH",
                    "Recipe candidate execution instances must match persisted provenance.",
                )
            })?;
        instance.instance_path = provenance.instance_path.clone();
        instance.provenance = provenance.clone();
    }
    candidate.instance_path = anchor_path;
    Ok(())
}

fn recipe_target_subtree(
    graph_parts: &[serde_json::Value],
    target_part_id: &str,
) -> BTreeSet<String> {
    let mut subtree = BTreeSet::from([target_part_id.to_owned()]);
    loop {
        let before = subtree.len();
        for part in graph_parts {
            if let (Some(part_id), Some(parent_id)) = (
                part.get("part_id").and_then(serde_json::Value::as_str),
                part.get("parent_part_id")
                    .and_then(serde_json::Value::as_str),
            ) {
                if subtree.contains(parent_id) {
                    subtree.insert(part_id.to_owned());
                }
            }
        }
        if subtree.len() == before {
            return subtree;
        }
    }
}

fn recipe_target_must_be_unprotected(
    version: &AgentAssetVersion,
    snapshot: &ActiveDesignSnapshot,
    subtree: &BTreeSet<String>,
    protected_part_ids: &[String],
) -> CoreResult<()> {
    let snapshot_locks = snapshot
        .part_display
        .as_ref()
        .map(|display| {
            display
                .locked_part_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let protected = subtree.iter().any(|part_id| {
        protected_part_ids
            .iter()
            .any(|protected| protected == part_id)
            || snapshot_locks.contains(part_id.as_str())
            || version
                .parts
                .iter()
                .find(|part| {
                    part.get("part_id").and_then(serde_json::Value::as_str)
                        == Some(part_id.as_str())
                })
                .and_then(|part| part.get("locked"))
                .and_then(serde_json::Value::as_bool)
                == Some(true)
    });
    if protected {
        return Err(CoreError::conflict(
            "PART_PROTECTED",
            "Locked or protected Recipe target descendants cannot be replaced.",
        ));
    }
    Ok(())
}

fn recipe_translation_only(value: &serde_json::Value) -> CoreResult<[f64; 3]> {
    let object = value.as_object().ok_or_else(|| {
        CoreError::invalid_data(
            "RECIPE_REPLACEMENT_TRANSFORM_UNSUPPORTED",
            "Recipe placement transform must be an object.",
        )
    })?;
    let position = recipe_vec3(
        object.get("position"),
        "RECIPE_REPLACEMENT_TRANSFORM_UNSUPPORTED",
    )?;
    let rotation = recipe_vec3(
        object.get("rotation"),
        "RECIPE_REPLACEMENT_TRANSFORM_UNSUPPORTED",
    )?;
    let scale = recipe_vec3(
        object.get("scale"),
        "RECIPE_REPLACEMENT_TRANSFORM_UNSUPPORTED",
    )?;
    if rotation.iter().any(|value| value.abs() > 1e-9)
        || scale.iter().any(|value| (*value - 1.0).abs() > 1e-9)
    {
        return Err(CoreError::invalid_data(
            "RECIPE_REPLACEMENT_TRANSFORM_UNSUPPORTED",
            "C105 Recipe placement supports only a finite translation frame.",
        ));
    }
    Ok(position)
}

fn recipe_vec3(value: Option<&serde_json::Value>, code: &'static str) -> CoreResult<[f64; 3]> {
    let values = value.and_then(serde_json::Value::as_array).ok_or_else(|| {
        CoreError::invalid_data(
            code,
            "Recipe placement requires a finite three-axis vector.",
        )
    })?;
    if values.len() != 3 {
        return Err(CoreError::invalid_data(
            code,
            "Recipe placement requires a finite three-axis vector.",
        ));
    }
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        let numeric = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                CoreError::invalid_data(
                    code,
                    "Recipe placement vectors must contain finite numbers.",
                )
            })?;
        result[index] = numeric;
    }
    Ok(result)
}

fn translate_recipe_candidate(
    candidate: &mut ExpandedComponentCandidate,
    delta: [f64; 3],
) -> CoreResult<()> {
    let operations = candidate
        .expanded_shape_program
        .get_mut("operations")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe candidate ShapeProgram has no operations.",
            )
        })?;
    for operation in operations {
        // A ProfileSketch is a two-dimensional construction input.  It has no
        // independent world-space mesh to translate; the positioned extrude,
        // revolve or loft consuming it owns its placement.  Keep accepting
        // that bounded helper node while continuing to reject every other
        // unpositioned root operation.
        let is_profile_sketch =
            operation.get("op").and_then(serde_json::Value::as_str) == Some("profile");
        let inherits_input_placement = operation
            .get("inputs")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|inputs| !inputs.is_empty());
        let args = operation
            .get_mut("args")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "RECIPE_PREVIEW_GEOMETRY_INVALID",
                    "Recipe candidate operation has no bounded arguments.",
                )
            })?;
        let mut positioned = false;
        if let Some(position) = args.get_mut("position") {
            let translated =
                recipe_vec3(Some(position), "RECIPE_PREVIEW_GEOMETRY_INVALID").map(|position| {
                    serde_json::json!([
                        position[0] + delta[0],
                        position[1] + delta[1],
                        position[2] + delta[2],
                    ])
                })?;
            *position = translated;
            positioned = true;
        }
        if let Some(path_points) = args
            .get_mut("path_points")
            .and_then(serde_json::Value::as_array_mut)
        {
            if path_points.is_empty() {
                return Err(CoreError::invalid_data(
                    "RECIPE_PREVIEW_GEOMETRY_INVALID",
                    "Recipe sweep path cannot be empty during placement.",
                ));
            }
            for point in path_points {
                let translated = recipe_vec3(Some(point), "RECIPE_PREVIEW_GEOMETRY_INVALID")?;
                *point = serde_json::json!([
                    translated[0] + delta[0],
                    translated[1] + delta[1],
                    translated[2] + delta[2],
                ]);
            }
            positioned = true;
        }
        if !positioned && !inherits_input_placement && !is_profile_sketch {
            return Err(CoreError::invalid_data(
                "RECIPE_PREVIEW_GEOMETRY_INVALID",
                "Recipe operation cannot be translated by the bounded C105 worker contract.",
            ));
        }
    }
    let parts = candidate
        .expanded_assembly_graph
        .get_mut("parts")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate AssemblyGraph has no parts.",
            )
        })?;
    for part in parts {
        let transform = part.get_mut("transform").ok_or_else(|| {
            CoreError::invalid_data(
                "RECIPE_PREVIEW_GRAPH_INVALID",
                "Recipe candidate Part has no transform.",
            )
        })?;
        let position = recipe_translation_only(transform)?;
        transform
            .as_object_mut()
            .expect("translation-only transform is an object")
            .insert(
                "position".into(),
                serde_json::json!([
                    position[0] + delta[0],
                    position[1] + delta[1],
                    position[2] + delta[2],
                ]),
            );
    }
    for instance in &mut candidate.instances {
        for axis in 0..3 {
            instance.world_transform[axis][3] += delta[axis];
        }
    }
    Ok(())
}

/// Selects exactly one embedded review catalog for repository-owned Recipe
/// expansion. C106 is never inferred from the robotic-arm domain: callers
/// must pin its immutable registry hash and one reviewed root Recipe. A
/// partial or mixed marker is rejected rather than silently falling back to
/// C105 and changing candidate provenance.
fn recipe_registry_for_repository_request(
    request: &RecipeInstantiationRequest,
    active_base: Option<&AgentAssetVersion>,
) -> CoreResult<RecipeRegistry> {
    let c105 = RecipeRegistry::from_embedded()?;
    let c106 = RecipeRegistry::from_embedded_c106_robotic_arm()?;
    let requested_registry_sha256 = request.recipe_registry_sha256.as_str();
    let recipe_id = request.recipe.recipe_id.as_str();
    let c106_recipe_known = c106.recipe(recipe_id).is_some();
    let c106_marker = c106_recipe_known || requested_registry_sha256 == c106.registry_sha256();

    if c106_marker {
        let _recipe = c106.recipe(recipe_id).ok_or_else(|| {
            CoreError::invalid_data(
                "COMPONENT_RECIPE_C106_PROVENANCE_INVALID",
                "C106 registry provenance cannot be paired with a Recipe outside the reviewed C106 catalog.",
            )
        })?;
        let exact_registry_and_domain = request.domain_pack_id == C106_ROBOTIC_ARM_DOMAIN
            && requested_registry_sha256 == c106.registry_sha256();
        let exact_root = c106_root_has_reviewed_provenance(recipe_id, &c106);
        let active_child_is_proven = active_base
            .is_some_and(|base| c106_active_target_has_exact_provenance(base, request, &c106));
        if !exact_registry_and_domain
            || (!active_child_is_proven && !(active_base.is_none() && exact_root))
        {
            return Err(CoreError::invalid_data(
                "COMPONENT_RECIPE_C106_PROVENANCE_INVALID",
                "C106 expansion requires its exact registry hash and domain; initial candidates require a reviewed root, while active edits require exact base/target provenance.",
            ));
        }
        return Ok(c106);
    }

    if requested_registry_sha256 != c105.registry_sha256() {
        return Err(CoreError::conflict(
            "COMPONENT_RECIPE_REGISTRY_STALE",
            "Recipe request names an unknown or stale reviewed registry identity.",
        ));
    }
    if c105.recipe(recipe_id).is_none() {
        return Err(CoreError::not_found("reviewed Component Recipe"));
    }
    Ok(c105)
}

fn c106_root_has_reviewed_provenance(recipe_id: &str, registry: &RecipeRegistry) -> bool {
    let Some(root) = registry.recipe(recipe_id) else {
        return false;
    };
    C106_ROOT_RECIPE_IDS.contains(&recipe_id)
        && root.component_role == "base_form"
        && root.child_slots.len() == 9
        && root.child_slots.iter().all(|slot| slot.required)
        && root.source["source_kind"] == "forgecad_first_party"
        && root.source["source_id"] == "source_c106_arm"
        && root.license["license_id"] == "ForgeCAD-Internal-Visual-Only"
        && root.license["redistributable"] == false
        && root.review_state["reviewer_kind"] == "forgecad_internal"
        && root.quality_status == "passed"
        && root.non_functional_only
}

fn c106_active_target_has_exact_provenance(
    version: &AgentAssetVersion,
    request: &RecipeInstantiationRequest,
    registry: &RecipeRegistry,
) -> bool {
    let Some(target_part_id) = request.target_part_id.as_deref() else {
        return false;
    };
    let Some(parts) = version
        .assembly_graph
        .get("parts")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    let Some(target_part) = parts.iter().find(|part| {
        part.get("part_id").and_then(serde_json::Value::as_str) == Some(target_part_id)
    }) else {
        return false;
    };
    let Some(target_instance_id) = target_part
        .get("recipe_instance_id")
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    let Some(instances) = version
        .assembly_graph
        .get("component_recipe_instances")
        .and_then(serde_json::Value::as_array)
    else {
        return false;
    };
    let root_is_exact = instances.iter().any(|instance| {
        instance
            .get("instance_path")
            .and_then(serde_json::Value::as_str)
            == Some("root")
            && instance.get("parent_instance_id") == Some(&serde_json::Value::Null)
            && instance.get("parent_slot_id") == Some(&serde_json::Value::Null)
            && instance
                .get("registry_sha256")
                .and_then(serde_json::Value::as_str)
                == Some(registry.registry_sha256())
            && instance
                .get("recipe")
                .and_then(|recipe| recipe.get("recipe_id"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|recipe_id| c106_root_has_reviewed_provenance(recipe_id, registry))
    });
    let target_is_exact = instances.iter().any(|instance| {
        instance
            .get("instance_id")
            .and_then(serde_json::Value::as_str)
            == Some(target_instance_id)
            && instance
                .get("registry_sha256")
                .and_then(serde_json::Value::as_str)
                == Some(registry.registry_sha256())
            && instance
                .get("recipe")
                .and_then(|recipe| {
                    serde_json::from_value::<ComponentRecipeRef>(recipe.clone()).ok()
                })
                .is_some_and(|reference| reference == request.recipe)
    });
    let target_role_matches = target_part.get("role").and_then(serde_json::Value::as_str)
        == registry
            .recipe(&request.recipe.recipe_id)
            .map(|recipe| recipe.component_role.as_str());
    root_is_exact && target_is_exact && target_role_matches
}

/// R007B plans bind the same two reviewed catalogs as candidate expansion.
/// This prevents a C106 design-surface plan from being rejected by a stale
/// C105-only lookup or, worse, from being silently reinterpreted as a C105
/// robotic-arm request.
fn recipe_registry_for_reference_rebuild_plan(
    plan: &ReferenceGuidedRebuildPlan,
) -> CoreResult<RecipeRegistry> {
    let c105 = RecipeRegistry::from_embedded()?;
    let c106 = RecipeRegistry::from_embedded_c106_robotic_arm()?;
    let c106_recipe_known = c106.recipe(&plan.recipe_id).is_some();
    let c106_marker = c106_recipe_known || plan.recipe_registry_sha256 == c106.registry_sha256();
    if c106_marker {
        let root = c106.recipe(&plan.recipe_id).ok_or_else(|| {
            CoreError::invalid_data(
                "REFERENCE_REBUILD_C106_PROVENANCE_INVALID",
                "C106 reference rebuild provenance cannot name a Recipe outside its reviewed catalog.",
            )
        })?;
        let exact_root = C106_ROOT_RECIPE_IDS.contains(&plan.recipe_id.as_str())
            && plan.domain_pack_id == C106_ROBOTIC_ARM_DOMAIN
            && plan.recipe_registry_sha256 == c106.registry_sha256()
            && root.component_role == "base_form"
            && root.child_slots.len() == 9
            && root.child_slots.iter().all(|slot| slot.required)
            && root.source["source_kind"] == "forgecad_first_party"
            && root.source["source_id"] == "source_c106_arm"
            && root.license["license_id"] == "ForgeCAD-Internal-Visual-Only"
            && root.license["redistributable"] == false
            && root.review_state["reviewer_kind"] == "forgecad_internal"
            && root.quality_status == "passed"
            && root.non_functional_only;
        if !exact_root {
            return Err(CoreError::invalid_data(
                "REFERENCE_REBUILD_C106_PROVENANCE_INVALID",
                "C106 reference rebuild requires the exact reviewed root, registry hash and visual-only provenance.",
            ));
        }
        return Ok(c106);
    }
    if plan.recipe_registry_sha256 != c105.registry_sha256() {
        return Err(CoreError::conflict(
            "REFERENCE_REBUILD_RECIPE_STALE",
            "Reference rebuild plan names an unknown or stale reviewed registry identity.",
        ));
    }
    if c105.recipe(&plan.recipe_id).is_none() {
        return Err(CoreError::not_found("reviewed Component Recipe"));
    }
    Ok(c105)
}

fn recipe_replacement_candidate_from_context(
    connection: &Connection,
    version: &AgentAssetVersion,
    snapshot: &ActiveDesignSnapshot,
    change_set: &AgentAssetChangeSet,
    operation: &serde_json::Value,
) -> CoreResult<ExpandedComponentCandidate> {
    let object = operation.as_object().ok_or_else(|| {
        CoreError::invalid_data(
            "CHANGE_SET_OPERATION_INVALID",
            "Every Agent ChangeSet operation must be a JSON object.",
        )
    })?;
    let allowed = [
        "operation_id",
        "op",
        "part_id",
        "recipe_request_id",
        "recipe_registry_sha256",
        "component_recipe_ref",
        "recipe_slot_bindings",
        "recipe_candidate_id",
        "recipe_candidate_sha256",
        "recipe_snapshot_revision",
    ];
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(CoreError::invalid_data(
            "REPLACE_PART_VARIANT_INVALID",
            "A Recipe-backed replace_part operation contains fields outside its sealed contract.",
        ));
    }
    if required_operation_string(object, "op")? != "replace_part" {
        return Err(CoreError::invalid_data(
            "REPLACE_PART_VARIANT_INVALID",
            "The sealed Component Recipe reference is only valid for replace_part.",
        ));
    }
    if change_set.operations.len() != 1 {
        return Err(CoreError::invalid_data(
            "RECIPE_REPLACEMENT_MIXED_OPERATIONS_UNSUPPORTED",
            "C105 Recipe replacement must be the only ChangeSet operation so placement cannot depend on operation order.",
        ));
    }
    let target_part_id = required_operation_string(object, "part_id")?;
    let request_id = required_operation_string(object, "recipe_request_id")?;
    let candidate_id = required_operation_string(object, "recipe_candidate_id")?;
    let candidate_sha256 = required_operation_string(object, "recipe_candidate_sha256")?;
    let recipe_registry_sha256 = required_operation_string(object, "recipe_registry_sha256")?;
    let sealed_revision = object
        .get("recipe_snapshot_revision")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "REPLACE_PART_VARIANT_INVALID",
                "Recipe replacement requires recipe_snapshot_revision.",
            )
        })?;
    if !valid_recipe_stable_id(request_id, "recipereq")
        || !valid_recipe_stable_id(candidate_id, "recipecandidate")
        || candidate_sha256.len() != 64
        || !candidate_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || recipe_registry_sha256.len() != 64
        || !recipe_registry_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(CoreError::invalid_data(
            "REPLACE_PART_VARIANT_INVALID",
            "Recipe replacement identities must use the bounded stable-ID and lowercase SHA-256 contracts.",
        ));
    }
    let recipe_ref: ComponentRecipeRef =
        serde_json::from_value(object.get("component_recipe_ref").cloned().ok_or_else(|| {
            CoreError::invalid_data(
                "REPLACE_PART_VARIANT_INVALID",
                "Recipe-backed replace_part requires component_recipe_ref.",
            )
        })?)
        .map_err(|_| {
            CoreError::invalid_data(
                "REPLACE_PART_VARIANT_INVALID",
                "component_recipe_ref does not match ComponentRecipeRef@1.",
            )
        })?;
    // The array is mandatory for the Recipe variant even when no optional
    // reviewed child is enabled.  It is part of the sealed identity rather
    // than a client-side presentation preference.
    let slot_bindings: Vec<RecipeSlotBinding> = serde_json::from_value(
        object
            .get("recipe_slot_bindings")
            .cloned()
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "REPLACE_PART_VARIANT_INVALID",
                    "Recipe-backed replace_part requires recipe_slot_bindings (an empty array is valid).",
                )
            })?,
    )
    .map_err(|_| {
        CoreError::invalid_data(
            "REPLACE_PART_VARIANT_INVALID",
            "recipe_slot_bindings must be a bounded array of RecipeSlotBinding values.",
        )
    })?;
    if slot_bindings.len() > 12 {
        return Err(CoreError::invalid_data(
            "REPLACE_PART_VARIANT_INVALID",
            "Recipe-backed replace_part exceeds the bounded optional child-slot limit.",
        ));
    }

    let preview_state_matches = match snapshot.preview.as_ref() {
        None => {
            change_set.status == ChangeSetStatus::Proposed && snapshot.revision == sealed_revision
        }
        Some(preview) => {
            change_set.status == ChangeSetStatus::Previewed
                && preview.project_id == change_set.project_id
                && preview.change_set_id == change_set.change_set_id
                && preview.base_asset_version_id == change_set.base_asset_version_id
                && snapshot.revision == sealed_revision.saturating_add(1)
        }
    };
    if !preview_state_matches
        || snapshot.active_design.asset_version_id() != Some(version.asset_version_id.as_str())
        || version.project_id != change_set.project_id
        || version.asset_version_id != change_set.base_asset_version_id
    {
        return Err(stale("CHANGE_SET_BASE_STALE"));
    }
    let head: String = connection
        .query_row(
            "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
            [&change_set.project_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| CoreError::not_found("Agent asset head"))?;
    if head != version.asset_version_id {
        return Err(stale("CHANGE_SET_BASE_STALE"));
    }
    let request = RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "active_asset_edit".into(),
        request_id: request_id.to_string(),
        project_id: Some(change_set.project_id.clone()),
        base_asset_version_id: Some(version.asset_version_id.clone()),
        snapshot_revision: Some(sealed_revision),
        domain_pack_id: version.domain_pack_id.clone(),
        recipe_registry_sha256: recipe_registry_sha256.to_string(),
        recipe: recipe_ref.clone(),
        target_part_id: Some(target_part_id.to_string()),
        // Fixed reviewed optional child choices are sealed verbatim and are
        // revalidated by the same Recipe engine that made the transient
        // candidate. Parameter and material overrides remain unavailable.
        slot_bindings,
        parameter_values: Vec::new(),
        material_zone_overrides: Vec::new(),
    };
    let registry = recipe_registry_for_repository_request(&request, Some(version))?;
    let mut candidate =
        RecipeExpander::expand(&registry, &request, &RecipeExpansionPolicy::default())?;
    place_recipe_candidate_in_active_context(
        &mut candidate,
        &request,
        version,
        snapshot,
        &change_set.protected_part_ids,
    )?;
    if candidate.recipe != recipe_ref
        || candidate.target_part_id.as_deref() != Some(target_part_id)
        || candidate.candidate_id != candidate_id
        || candidate.candidate_sha256 != candidate_sha256
    {
        return Err(CoreError::conflict(
            "COMPONENT_RECIPE_CANDIDATE_STALE",
            "The sealed Recipe candidate no longer matches the active reviewed registry, Version, or Snapshot.",
        ));
    }
    Ok(candidate)
}

fn validate_change_set_operations(
    transaction: &Connection,
    version: &AgentAssetVersion,
    snapshot: &ActiveDesignSnapshot,
    change_set: &AgentAssetChangeSet,
) -> CoreResult<()> {
    if change_set.operations.len() > 32 {
        return Err(CoreError::invalid_data(
            "CHANGE_SET_OPERATION_LIMIT",
            "Agent ChangeSet may contain at most 32 bounded operations.",
        ));
    }
    let part_zones = version.part_zone_index()?;
    let mut protected = BTreeSet::new();
    for part_id in &change_set.protected_part_ids {
        if !part_zones.contains_key(part_id) {
            return Err(CoreError::not_found("protected Agent asset part"));
        }
        if !protected.insert(part_id.as_str()) {
            return Err(CoreError::invalid_data(
                "CHANGE_SET_PROTECTED_PART_DUPLICATE",
                "Protected part identities must be unique.",
            ));
        }
    }
    let mut locked = snapshot
        .part_display
        .as_ref()
        .map(|display| display.locked_part_ids.iter().map(String::as_str).collect())
        .unwrap_or_else(BTreeSet::new);
    for part in &version.parts {
        if part.get("locked").and_then(serde_json::Value::as_bool) == Some(true) {
            if let Some(part_id) = part.get("part_id").and_then(serde_json::Value::as_str) {
                locked.insert(part_id);
            }
        }
    }
    let mut operation_ids = BTreeSet::new();
    for operation in &change_set.operations {
        let object = operation.as_object().ok_or_else(|| {
            CoreError::invalid_data(
                "CHANGE_SET_OPERATION_INVALID",
                "Every Agent ChangeSet operation must be a JSON object.",
            )
        })?;
        let operation_id = required_operation_string(object, "operation_id")?;
        if !operation_ids.insert(operation_id) {
            return Err(CoreError::invalid_data(
                "CHANGE_SET_OPERATION_DUPLICATE",
                "Agent ChangeSet operation identities must be unique.",
            ));
        }
        let operation_kind = required_operation_string(object, "op")?;
        if !matches!(
            operation_kind,
            "set_part_transform"
                | "set_part_parameter"
                | "set_joint_pose"
                | "apply_material_preset"
                | "apply_surface_adornment"
                | "replace_part"
                | "add_reviewed_recipe"
                | "replace_reviewed_recipe"
                | "snap_part_to_connector"
                | "split_part"
                | "merge_parts"
        ) {
            return Err(CoreError::invalid_data(
                "CHANGE_SET_OPERATION_UNSUPPORTED",
                "Agent ChangeSet operation is outside the code-owned allowlist.",
            ));
        }
        let part_id = required_operation_string(object, "part_id")?;
        let target_part_id = object
            .get("target_part_id")
            .and_then(serde_json::Value::as_str);
        if !part_zones.contains_key(part_id)
            || target_part_id.is_some_and(|target| !part_zones.contains_key(target))
        {
            return Err(CoreError::not_found("Agent asset part"));
        }
        if protected.contains(part_id)
            || locked.contains(part_id)
            || target_part_id
                .is_some_and(|target| protected.contains(target) || locked.contains(target))
        {
            return Err(CoreError::conflict(
                "PART_PROTECTED",
                "Locked or request-protected parts cannot be modified.",
            ));
        }
        if let Some(transform) = object.get("transform") {
            validate_transform(transform)?;
        }
        match operation_kind {
            "set_part_transform" if object.get("transform").is_none() => {
                return Err(CoreError::invalid_data(
                    "TRANSFORM_REQUIRED",
                    "set_part_transform requires a bounded transform.",
                ));
            }
            "set_part_parameter" => validate_parameter_operation(version, part_id, object)?,
            "apply_material_preset" => {
                validate_material_operation(version, &part_zones, part_id, object)?
            }
            "apply_surface_adornment" => {
                let value = object.get("surface_adornment_program").ok_or_else(|| {
                    CoreError::invalid_data(
                        "SURFACE_ADORNMENT_PROGRAM_REQUIRED",
                        "Surface appearance ChangeSet requires one bounded program.",
                    )
                })?;
                let program: SurfaceAdornmentProgram = serde_json::from_value(value.clone())
                    .map_err(|_| {
                        CoreError::invalid_data(
                            "SURFACE_ADORNMENT_PROGRAM_INVALID",
                            "Surface appearance program does not match SurfaceAdornmentProgram@1.",
                        )
                    })?;
                program.validate()?;
                let zone_id = required_operation_string(object, "material_zone_id")?;
                if program.target_part_id != part_id
                    || program.target_zone_id != zone_id
                    || !part_zones
                        .get(part_id)
                        .is_some_and(|zones| zones.iter().any(|zone| zone == zone_id))
                {
                    return Err(CoreError::conflict(
                        "SURFACE_ADORNMENT_TARGET_MISMATCH",
                        "Surface appearance program must target the selected Part and Material Zone.",
                    ));
                }
                let activation = transaction
                    .query_row(
                        "SELECT activation_id, skill_id, skill_version, skill_sha256, enabled, updated_at FROM agent_skill_activations WHERE skill_id=? AND enabled=1",
                        [&program.skill_id],
                        skill_activation_from_row,
                    )
                    .optional()?
                    .ok_or_else(|| {
                        CoreError::conflict(
                            "SURFACE_ADORNMENT_SKILL_DISABLED",
                            "Surface appearance requires an explicitly enabled Skill.",
                        )
                    })?;
                if activation.skill_version != program.skill_version
                    || activation.skill_sha256 != program.skill_sha256
                {
                    return Err(CoreError::conflict(
                        "SURFACE_ADORNMENT_SKILL_STALE",
                        "Surface appearance program does not name the active immutable Skill.",
                    ));
                }
                let (manifest, manifest_sha, _, _) =
                    require_skill_manifest(transaction, &program.skill_id, program.skill_version)?;
                if manifest_sha != program.skill_sha256
                    || !manifest
                        .allowed_domains
                        .iter()
                        .any(|domain| domain == &version.domain_pack_id)
                    || !manifest
                        .material_preset_ids
                        .iter()
                        .any(|material| material == &program.base_material)
                {
                    return Err(CoreError::conflict(
                        "SURFACE_ADORNMENT_POLICY_DENIED",
                        "Surface appearance is outside the enabled Skill policy.",
                    ));
                }
                validate_surface_adornment_recipe_slot(version, part_id, &program, &manifest)?;
            }
            "replace_part" => {
                let has_legacy_component = object.contains_key("replacement_component_id");
                let has_recipe_fields = [
                    "recipe_request_id",
                    "recipe_registry_sha256",
                    "component_recipe_ref",
                    "recipe_slot_bindings",
                    "recipe_candidate_id",
                    "recipe_candidate_sha256",
                    "recipe_snapshot_revision",
                ]
                .iter()
                .any(|field| object.contains_key(*field));
                match (has_legacy_component, has_recipe_fields) {
                    (true, false) => {
                        let component_id =
                            required_operation_string(object, "replacement_component_id")?;
                        let component = component_from_connection(transaction, component_id)?
                            .ok_or_else(|| CoreError::not_found("AgentComponent"))?;
                        let target_role = version
                            .parts
                            .iter()
                            .find(|part| {
                                part.get("part_id").and_then(serde_json::Value::as_str)
                                    == Some(part_id)
                            })
                            .and_then(|part| part.get("role"))
                            .and_then(serde_json::Value::as_str)
                            .ok_or_else(|| {
                                CoreError::invalid_data(
                                    "PART_ROLE_INVALID",
                                    "Replacement target is missing a stable part role.",
                                )
                            })?;
                        let candidate =
                            component_candidate(version, part_id, target_role, component);
                        require_component_candidate_eligible(&candidate)?;
                    }
                    (false, true) => {
                        recipe_replacement_candidate_from_context(
                            transaction,
                            version,
                            snapshot,
                            change_set,
                            operation,
                        )?;
                    }
                    _ => {
                        return Err(CoreError::invalid_data(
                            "REPLACE_PART_VARIANT_INVALID",
                            "replace_part must contain exactly one of replacement_component_id or the sealed Component Recipe reference.",
                        ));
                    }
                }
            }
            "add_reviewed_recipe" => {
                if version.domain_pack_id != C106_ROBOTIC_ARM_DOMAIN {
                    return Err(CoreError::conflict(
                        "ASSEMBLY_DELTA_DOMAIN_INVALID",
                        "Reviewed arm attachment Recipes are only available in the robotic-arm Domain Pack.",
                    ));
                }
                let new_part_id = required_operation_string(object, "new_part_id")?;
                if part_zones.contains_key(new_part_id) {
                    return Err(CoreError::conflict(
                        "ASSEMBLY_DELTA_PART_EXISTS",
                        "A reviewed attachment cannot reuse an existing Part identity.",
                    ));
                }
                let recipe_id = required_operation_string(object, "recipe_id")?;
                if !C110C_ARM_RECIPES.contains(&recipe_id) {
                    return Err(CoreError::conflict(
                        "ASSEMBLY_DELTA_RECIPE_UNREVIEWED",
                        "Assembly delta references a Recipe outside the reviewed robotic-arm catalog.",
                    ));
                }
                let slot_id = required_operation_string(object, "slot_id")?;
                if !C110C_ARM_ATTACHMENT_SLOTS.contains(&slot_id) {
                    return Err(CoreError::conflict(
                        "ASSEMBLY_DELTA_SLOT_UNREVIEWED",
                        "Assembly delta references an attachment slot outside the reviewed contract.",
                    ));
                }
                validate_transform(object.get("transform").ok_or_else(|| {
                    CoreError::invalid_data(
                        "ASSEMBLY_DELTA_TRANSFORM_REQUIRED",
                        "add_reviewed_recipe requires a bounded transform.",
                    )
                })?)?;
                for field in ["parent_connector_id", "child_connector_id"] {
                    required_operation_string(object, field)?;
                }
            }
            "replace_reviewed_recipe" => {
                if version.domain_pack_id != C106_ROBOTIC_ARM_DOMAIN {
                    return Err(CoreError::conflict(
                        "ASSEMBLY_DELTA_DOMAIN_INVALID",
                        "Reviewed arm replacement Recipes are only available in the robotic-arm Domain Pack.",
                    ));
                }
                let recipe_id = required_operation_string(object, "recipe_id")?;
                if !C110C_ARM_RECIPES.contains(&recipe_id) {
                    return Err(CoreError::conflict(
                        "ASSEMBLY_DELTA_RECIPE_UNREVIEWED",
                        "Assembly delta references a Recipe outside the reviewed robotic-arm catalog.",
                    ));
                }
            }
            "snap_part_to_connector" => {
                for field in ["target_part_id", "target_connector_id", "connector_id"] {
                    required_operation_string(object, field)?;
                }
            }
            "set_joint_pose" => {
                required_operation_string(object, "joint_id")?;
                let pose = object
                    .get("pose")
                    .and_then(serde_json::Value::as_object)
                    .ok_or_else(|| {
                        CoreError::invalid_data(
                            "ASSEMBLY_DELTA_POSE_REQUIRED",
                            "set_joint_pose requires a bounded pose object.",
                        )
                    })?;
                for field in ["rotation", "translation"] {
                    let values = pose
                        .get(field)
                        .and_then(serde_json::Value::as_array)
                        .filter(|values| values.len() == 3)
                        .ok_or_else(|| {
                            CoreError::invalid_data(
                                "ASSEMBLY_DELTA_POSE_INVALID",
                                "Joint pose vectors must contain exactly three values.",
                            )
                        })?;
                    if values.iter().any(|value| {
                        value
                            .as_f64()
                            .is_none_or(|number| !number.is_finite() || number.abs() > 100_000.0)
                    }) {
                        return Err(CoreError::invalid_data(
                            "ASSEMBLY_DELTA_POSE_INVALID",
                            "Joint pose values must be finite and bounded.",
                        ));
                    }
                }
            }
            "split_part" => {
                let suggestion_id = required_operation_string(object, "structure_suggestion_id")?;
                let suggestions = derive_structure_suggestions(version, snapshot)?;
                if !suggestions.iter().any(|suggestion| {
                    suggestion.suggestion_id == suggestion_id
                        && suggestion.kind == "split_part"
                        && suggestion.part_id == part_id
                        && suggestion.target_part_id.is_none()
                }) {
                    return Err(CoreError::conflict(
                        "STRUCTURE_SUGGESTION_NOT_AVAILABLE",
                        "The split suggestion is stale, forged, locked, or no longer derivable from current facts.",
                    ));
                }
            }
            "merge_parts" => {
                let target = required_operation_string(object, "target_part_id")?;
                let suggestion_id = required_operation_string(object, "structure_suggestion_id")?;
                let suggestions = derive_structure_suggestions(version, snapshot)?;
                if !suggestions.iter().any(|suggestion| {
                    suggestion.suggestion_id == suggestion_id
                        && suggestion.kind == "merge_parts"
                        && suggestion.part_id == part_id
                        && suggestion.target_part_id.as_deref() == Some(target)
                }) {
                    return Err(CoreError::conflict(
                        "STRUCTURE_SUGGESTION_NOT_AVAILABLE",
                        "The merge suggestion is stale, forged, locked, or no longer derivable from current facts.",
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// C106 keeps its A005 affordances on the immutable Recipe-expanded graph.
/// Looking up both the part and its Recipe provenance here prevents a mutable
/// UI selection or an arbitrary skill program from granting an undeclared
/// design-surface slot.
fn validate_surface_adornment_recipe_slot(
    version: &AgentAssetVersion,
    target_part_id: &str,
    program: &SurfaceAdornmentProgram,
    manifest: &AgentSkillManifest,
) -> CoreResult<()> {
    let graph_parts = version
        .assembly_graph
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "ASSEMBLY_GRAPH_PARTS_INVALID",
                "Surface adornment requires stable AssemblyGraph parts.",
            )
        })?;
    let graph_part = graph_parts
        .iter()
        .find(|part| {
            part.get("part_id").and_then(serde_json::Value::as_str) == Some(target_part_id)
        })
        .ok_or_else(|| CoreError::not_found("Surface adornment target AssemblyGraph part"))?;
    let Some(instance_id) = graph_part
        .get("recipe_instance_id")
        .and_then(serde_json::Value::as_str)
    else {
        // Legacy/non-Recipe assets retain the existing A005 lifecycle.  They
        // cannot impersonate a C106 Recipe because C106 requires provenance.
        return Ok(());
    };
    let recipe_id = version
        .assembly_graph
        .get("component_recipe_instances")
        .and_then(serde_json::Value::as_array)
        .and_then(|instances| {
            instances.iter().find(|instance| {
                instance
                    .get("instance_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(instance_id)
            })
        })
        .and_then(|instance| instance.get("recipe"))
        .and_then(|recipe| recipe.get("recipe_id"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            CoreError::conflict(
                "SURFACE_ADORNMENT_RECIPE_PROVENANCE_MISSING",
                "Recipe-backed surface adornment requires immutable instance provenance.",
            )
        })?;
    if !manifest
        .recipe_ids
        .iter()
        .any(|allowed| allowed == recipe_id)
    {
        return Err(CoreError::conflict(
            "SURFACE_ADORNMENT_RECIPE_POLICY_DENIED",
            "The active immutable Skill manifest does not grant the target Recipe.",
        ));
    }
    let slots: Vec<RecipeSurfaceAdornmentSlot> = serde_json::from_value(
        graph_part
            .get("surface_adornment_slots")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new())),
    )
    .map_err(|_| {
        CoreError::invalid_data(
            "SURFACE_ADORNMENT_RECIPE_SLOT_INVALID",
            "Recipe surface adornment slots are not a closed reviewed contract.",
        )
    })?;
    program.validate_recipe_surface_slot(recipe_id, &slots)
}

fn required_operation_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> CoreResult<&'a str> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 256)
        .ok_or_else(|| {
            CoreError::invalid_data(
                "CHANGE_SET_OPERATION_INVALID",
                format!("ChangeSet operation requires bounded field {field}."),
            )
        })
}

fn valid_recipe_stable_id(value: &str, prefix: &str) -> bool {
    let Some(suffix) = value.strip_prefix(&format!("{prefix}_")) else {
        return false;
    };
    !suffix.is_empty()
        && suffix.len() <= 240
        && suffix.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

fn validate_transform(value: &serde_json::Value) -> CoreResult<()> {
    let object = value.as_object().ok_or_else(|| {
        CoreError::invalid_data("TRANSFORM_INVALID", "Part transform must be a JSON object.")
    })?;
    for (name, vector) in object {
        if !matches!(name.as_str(), "position" | "rotation" | "scale") {
            return Err(CoreError::invalid_data(
                "TRANSFORM_INVALID",
                "Part transform contains an unsupported vector.",
            ));
        }
        let values = vector
            .as_array()
            .filter(|values| values.len() == 3)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "TRANSFORM_INVALID",
                    "Part transform vectors must contain exactly three values.",
                )
            })?;
        for number in values {
            let number = number
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    CoreError::invalid_data(
                        "TRANSFORM_INVALID",
                        "Part transform values must be finite numbers.",
                    )
                })?;
            let valid = if name == "scale" {
                (0.1..=10.0).contains(&number)
            } else {
                number.abs() <= 100_000.0
            };
            if !valid {
                return Err(CoreError::invalid_data(
                    "PARAMETER_OUT_OF_RANGE",
                    "Part transform is outside the lightweight concept range.",
                ));
            }
        }
    }
    Ok(())
}

fn validate_parameter_operation(
    version: &AgentAssetVersion,
    part_id: &str,
    object: &serde_json::Map<String, serde_json::Value>,
) -> CoreResult<()> {
    let path = required_operation_string(object, "path")?;
    if !matches!(
        path,
        "transform.position.x"
            | "transform.position.y"
            | "transform.position.z"
            | "transform.scale.x"
            | "transform.scale.y"
            | "transform.scale.z"
    ) {
        return Err(CoreError::invalid_data(
            "PARAMETER_NOT_ALLOWED",
            "Only the frozen bounded position and scale paths are accepted.",
        ));
    }
    let number = object
        .get("value")
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            CoreError::invalid_data(
                "PARAMETER_INVALID",
                "Part parameter value must be a finite number.",
            )
        })?;
    let part = version
        .parts
        .iter()
        .find(|part| part.get("part_id").and_then(serde_json::Value::as_str) == Some(part_id))
        .ok_or_else(|| CoreError::not_found("Agent asset part"))?;
    let bindings = part
        .get("editable_parameter_bindings")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(binding) = bindings
        .iter()
        .find(|binding| binding.get("path").and_then(serde_json::Value::as_str) == Some(path))
    {
        let minimum = binding
            .get("min")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "EDITABLE_PARAMETER_BINDING_INVALID",
                    "Declared editable parameter minimum is invalid.",
                )
            })?;
        let maximum = binding
            .get("max")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite() && *value > minimum)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "EDITABLE_PARAMETER_BINDING_INVALID",
                    "Declared editable parameter maximum is invalid.",
                )
            })?;
        let step = binding
            .get("step")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite() && *value > 0.0 && *value <= maximum - minimum)
            .ok_or_else(|| {
                CoreError::invalid_data(
                    "EDITABLE_PARAMETER_BINDING_INVALID",
                    "Declared editable parameter step is invalid.",
                )
            })?;
        if number < minimum || number > maximum {
            return Err(CoreError::invalid_data(
                "PARAMETER_OUT_OF_RANGE",
                "Part parameter is outside its declared editable range.",
            ));
        }
        let steps = (number - minimum) / step;
        if (steps - steps.round()).abs() > 1e-8 {
            return Err(CoreError::invalid_data(
                "PARAMETER_STEP_MISMATCH",
                "Part parameter does not match its declared editing step.",
            ));
        }
        return Ok(());
    }
    if !bindings.is_empty() {
        return Err(CoreError::invalid_data(
            "PARAMETER_NOT_DECLARED",
            "Part does not declare this editable parameter path.",
        ));
    }
    let legacy_valid = if path.starts_with("transform.scale") {
        (0.1..=10.0).contains(&number)
    } else {
        number.abs() <= 100_000.0
    };
    if !legacy_valid {
        return Err(CoreError::invalid_data(
            "PARAMETER_OUT_OF_RANGE",
            "Legacy part parameter is outside the frozen lightweight concept range.",
        ));
    }
    Ok(())
}

fn validate_material_operation(
    version: &AgentAssetVersion,
    part_zones: &std::collections::BTreeMap<String, Vec<String>>,
    part_id: &str,
    object: &serde_json::Map<String, serde_json::Value>,
) -> CoreResult<()> {
    let material_id = required_operation_string(object, "material_id")?;
    let domain = domain_for_pack(&version.domain_pack_id).ok_or_else(|| {
        CoreError::conflict(
            "DOMAIN_PACK_UNKNOWN",
            "Asset Domain Pack is not registered by the Rust core.",
        )
    })?;
    let allowed = material_allowed_domains(material_id)
        .ok_or_else(|| CoreError::not_found("visual material preset"))?;
    if !allowed.contains(&domain) {
        return Err(CoreError::invalid_data(
            "MATERIAL_DOMAIN_INCOMPATIBLE",
            "Visual material is not allowed for this concept domain.",
        ));
    }
    if let Some(zone_id) = object
        .get("material_zone_id")
        .and_then(serde_json::Value::as_str)
    {
        if !part_zones
            .get(part_id)
            .is_some_and(|zones| zones.iter().any(|zone| zone == zone_id))
        {
            return Err(CoreError::not_found("Agent asset Material Zone"));
        }
    }
    Ok(())
}

fn domain_for_pack(pack_id: &str) -> Option<&'static str> {
    match pack_id {
        "pack_future_weapon_prop" | "pack_future_prop" => Some("future_weapon_prop"),
        "pack_vehicle_concept" => Some("vehicle_concept"),
        "pack_aircraft_concept" => Some("aircraft_concept"),
        "pack_robotic_arm_concept" => Some("robotic_arm_concept"),
        _ => None,
    }
}

pub(crate) fn material_allowed_domains(material_id: &str) -> Option<&'static [&'static str]> {
    const ALL: &[&str] = &[
        "future_weapon_prop",
        "vehicle_concept",
        "aircraft_concept",
        "robotic_arm_concept",
    ];
    const VEHICLE_WEAPON: &[&str] = &["vehicle_concept", "future_weapon_prop"];
    const RUBBER: &[&str] = &[
        "vehicle_concept",
        "robotic_arm_concept",
        "future_weapon_prop",
    ];
    const COMPOSITE: &[&str] = &[
        "aircraft_concept",
        "robotic_arm_concept",
        "future_weapon_prop",
    ];
    const GLASS: &[&str] = &["aircraft_concept", "vehicle_concept", "future_weapon_prop"];
    const TIRE: &[&str] = &["vehicle_concept", "robotic_arm_concept"];
    match material_id {
        "mat_graphite"
        | "mat_aluminum"
        | "mat_signal_red"
        | "mat_painted_steel"
        | "mat_abs_matte"
        | "mat_carbon_composite"
        | "mat_powder_coat" => Some(ALL),
        "mat_automotive_paint" => Some(VEHICLE_WEAPON),
        "mat_rubber" => Some(RUBBER),
        "mat_composite" => Some(COMPOSITE),
        "mat_dark_glass" | "mat_clear_glass" => Some(GLASS),
        "mat_rubber_tire" => Some(TIRE),
        _ => None,
    }
}

fn insert_sorted_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
        values.sort();
    }
}

fn remove_value(values: &mut Vec<String>, value: &str) {
    values.retain(|existing| existing != value);
}

fn validate_idempotency_identity(scope: &str, key: &str, request_hash: &str) -> CoreResult<()> {
    if scope.is_empty()
        || scope.len() > 512
        || key.is_empty()
        || key.len() > 256
        || request_hash.len() != 64
        || !request_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(CoreError::invalid_data(
            "IDEMPOTENCY_IDENTITY_INVALID",
            "Idempotency scope, key or request hash is invalid.",
        ));
    }
    Ok(())
}

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    let status: String = row.get(4)?;
    Ok(Project {
        project_id: row.get(0)?,
        profile_id: row.get(1)?,
        domain_type: row.get(2)?,
        name: row.get(3)?,
        status: match status.as_str() {
            "active" => crate::ProjectStatus::Active,
            "archived" => crate::ProjectStatus::Archived,
            "soft_deleted" => crate::ProjectStatus::SoftDeleted,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        current_version_id: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn initial_snapshot(version: &AgentAssetVersion) -> CoreResult<ActiveDesignSnapshot> {
    let snapshot = ActiveDesignSnapshot {
        schema_version: "ActiveDesignSnapshot@1".to_string(),
        project_id: version.project_id.clone(),
        active_design: ActiveDesign::AgentAsset {
            project_id: version.project_id.clone(),
            asset_version_id: version.asset_version_id.clone(),
            assembly_graph_id: version.assembly_graph_id()?.to_string(),
        },
        selected_part_id: None,
        selected_material_zone_id: None,
        preview: None,
        quality: None,
        export: ExportReference::AgentAsset {
            project_id: version.project_id.clone(),
            source_version_id: version.asset_version_id.clone(),
        },
        render_preset: Some(RenderPreset::default_for(
            &version.project_id,
            &version.asset_version_id,
            &version.created_at,
        )),
        part_display: Some(PartDisplay::empty(
            &version.project_id,
            &version.asset_version_id,
        )),
        revision: 1,
        updated_at: version.created_at.clone(),
    };
    snapshot.validate()?;
    Ok(snapshot)
}

fn insert_snapshot(
    transaction: &Transaction<'_>,
    snapshot: &ActiveDesignSnapshot,
) -> CoreResult<()> {
    let ActiveDesign::AgentAsset {
        asset_version_id,
        assembly_graph_id,
        ..
    } = &snapshot.active_design
    else {
        return Err(CoreError::invalid_data(
            "ACTIVE_DESIGN_SOURCE_INVALID",
            "Initial Snapshot must be an Agent asset.",
        ));
    };
    transaction.execute(
        "INSERT INTO active_design_snapshots(project_id, source, active_asset_version_id, active_assembly_graph_id, legacy_version_id, legacy_module_graph_id, selected_part_id, preview_change_set_id, preview_base_asset_version_id, quality_report_id, quality_asset_version_id, export_source, export_source_version_id, revision, updated_at, render_preset_json, selected_material_zone_id, part_display_json) VALUES (?, 'agent_asset', ?, ?, NULL, NULL, ?, NULL, NULL, NULL, NULL, 'agent_asset', ?, ?, ?, ?, ?, ?)",
        params![
            snapshot.project_id,
            asset_version_id,
            assembly_graph_id,
            snapshot.selected_part_id,
            asset_version_id,
            snapshot.revision,
            snapshot.updated_at,
            json_option(&snapshot.render_preset)?,
            snapshot.selected_material_zone_id,
            json_option(&snapshot.part_display)?,
        ],
    )?;
    Ok(())
}

fn advance_snapshot(
    transaction: &Transaction<'_>,
    previous: &ActiveDesignSnapshot,
    version: &AgentAssetVersion,
    updated_at: &str,
) -> CoreResult<()> {
    let zones = version.part_zone_index()?;
    let (selected_part, selected_zone) = match previous.selected_part_id.as_deref() {
        Some(part) if zones.contains_key(part) => {
            let zone = previous
                .selected_material_zone_id
                .as_deref()
                .filter(|zone| {
                    zones
                        .get(part)
                        .is_some_and(|items| items.iter().any(|item| item == *zone))
                });
            (Some(part.to_string()), zone.map(str::to_string))
        }
        _ => (None, None),
    };
    let mut display = previous
        .part_display
        .clone()
        .unwrap_or_else(|| PartDisplay::empty(&version.project_id, &version.asset_version_id));
    display.asset_version_id = version.asset_version_id.clone();
    display
        .locked_part_ids
        .retain(|part| zones.contains_key(part));
    display
        .hidden_part_ids
        .retain(|part| zones.contains_key(part));
    if display
        .isolated_part_id
        .as_ref()
        .is_some_and(|part| !zones.contains_key(part))
    {
        display.isolated_part_id = None;
    }
    let render =
        RenderPreset::default_for(&version.project_id, &version.asset_version_id, updated_at);
    let changed = transaction.execute(
        "UPDATE active_design_snapshots SET active_asset_version_id=?, active_assembly_graph_id=?, selected_part_id=?, selected_material_zone_id=?, preview_change_set_id=NULL, preview_base_asset_version_id=NULL, quality_report_id=NULL, quality_asset_version_id=NULL, export_source='agent_asset', export_source_version_id=?, render_preset_json=?, part_display_json=?, revision=revision+1, updated_at=? WHERE project_id=? AND source='agent_asset' AND revision=?",
        params![
            version.asset_version_id,
            version.assembly_graph_id()?,
            selected_part,
            selected_zone,
            version.asset_version_id,
            json_text(&render)?,
            json_text(&display)?,
            updated_at,
            version.project_id,
            previous.revision,
        ],
    )?;
    require_changed(changed)
}

fn set_head(
    transaction: &Transaction<'_>,
    project_id: &str,
    version_id: &str,
    updated_at: &str,
) -> CoreResult<()> {
    transaction.execute(
        "INSERT INTO agent_asset_heads(project_id, asset_version_id, updated_at) VALUES (?, ?, ?) ON CONFLICT(project_id) DO UPDATE SET asset_version_id=excluded.asset_version_id, updated_at=excluded.updated_at",
        params![project_id, version_id, updated_at],
    )?;
    Ok(())
}

fn require_head(transaction: &Transaction<'_>, project_id: &str) -> CoreResult<String> {
    transaction
        .query_row(
            "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
            [project_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| CoreError::not_found("Agent asset head"))
}

fn version_from_connection(
    connection: &Connection,
    id: &str,
) -> CoreResult<Option<AgentAssetVersion>> {
    connection.query_row(
        "SELECT asset_version_id, project_id, parent_asset_version_id, version_no, status, summary, stage, plan_id, direction_id, domain_pack_id, artifact_id, parts_json, shape_program_json, assembly_graph_json, material_bindings_json, created_at FROM agent_asset_versions WHERE asset_version_id=?",
        [id], version_from_row,
    ).optional().map_err(Into::into)
}

fn require_version(connection: &Connection, id: &str) -> CoreResult<AgentAssetVersion> {
    version_from_connection(connection, id)?
        .ok_or_else(|| CoreError::not_found("Agent asset version"))
}

fn version_from_row(row: &Row<'_>) -> rusqlite::Result<AgentAssetVersion> {
    let status: String = row.get(4)?;
    let stage: String = row.get(6)?;
    Ok(AgentAssetVersion {
        asset_version_id: row.get(0)?,
        project_id: row.get(1)?,
        parent_asset_version_id: row.get(2)?,
        version_no: row.get(3)?,
        status: AssetVersionStatus::from_str(&status).map_err(to_sql_error)?,
        summary: row.get(5)?,
        stage: crate::AssetStage::from_str(&stage).map_err(to_sql_error)?,
        plan_id: row.get(7)?,
        direction_id: row.get(8)?,
        domain_pack_id: row.get(9)?,
        artifact_id: row.get(10)?,
        parts: parse_json(row.get::<_, String>(11)?).map_err(to_sql_error)?,
        shape_program: parse_json(row.get::<_, String>(12)?).map_err(to_sql_error)?,
        assembly_graph: parse_json(row.get::<_, String>(13)?).map_err(to_sql_error)?,
        material_bindings: parse_json(row.get::<_, String>(14)?).map_err(to_sql_error)?,
        created_at: row.get(15)?,
    })
}

fn bootstrap_legacy_snapshot(
    transaction: &Transaction<'_>,
    project_id: &str,
    expected: SnapshotEtag,
    updated_at: &str,
) -> CoreResult<ActiveDesignSnapshot> {
    let has_agent_head: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM agent_asset_heads WHERE project_id=?)",
        [project_id],
        |row| row.get(0),
    )?;
    if has_agent_head {
        return Err(CoreError::conflict(
            "ACTIVE_DESIGN_NOT_LEGACY",
            "The Project already has an Agent asset head and cannot authorize a legacy rebuild.",
        ));
    }
    let legacy_source: Option<(String, String)> = transaction
        .query_row(
            "SELECT pv.version_id, mg.graph_id FROM projects p JOIN project_versions pv ON pv.version_id=p.current_version_id AND pv.project_id=p.project_id JOIN module_graphs mg ON mg.graph_id=pv.module_graph_id AND mg.project_id=p.project_id WHERE p.project_id=? AND p.status='active' AND pv.status!='soft_deleted'",
            [project_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((legacy_version_id, module_graph_id)) = legacy_source else {
        let project_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id=?)",
            [project_id],
            |row| row.get(0),
        )?;
        return Err(if project_exists {
            CoreError::not_found("Legacy ActiveDesign")
        } else {
            CoreError::not_found("Project")
        });
    };
    if expected.0 != 1 {
        return Err(stale("ACTIVE_DESIGN_STALE"));
    }
    transaction.execute(
        "INSERT INTO active_design_snapshots(project_id, source, active_asset_version_id, active_assembly_graph_id, legacy_version_id, legacy_module_graph_id, selected_part_id, preview_change_set_id, preview_base_asset_version_id, quality_report_id, quality_asset_version_id, export_source, export_source_version_id, revision, updated_at, render_preset_json, selected_material_zone_id, part_display_json) VALUES (?, 'legacy_concept_read_only', NULL, NULL, ?, ?, NULL, NULL, NULL, NULL, NULL, 'legacy_concept_read_only', ?, 1, ?, NULL, NULL, NULL)",
        params![
            project_id,
            legacy_version_id,
            module_graph_id,
            legacy_version_id,
            updated_at,
        ],
    )?;
    require_snapshot(transaction, project_id)
}

fn require_legacy_source_binding(
    connection: &Connection,
    project_id: &str,
    legacy_version_id: &str,
    module_graph_id: &str,
) -> CoreResult<()> {
    legacy_read::validate_legacy_source_binding(
        connection,
        project_id,
        legacy_version_id,
        module_graph_id,
    )
}

fn legacy_conversion_intent_from_connection(
    connection: &Connection,
    project_id: &str,
) -> CoreResult<Option<LegacyAgentConversionIntent>> {
    let intent = connection
        .query_row(
            "SELECT project_id, legacy_version_id, legacy_module_graph_id, snapshot_revision, requested_at FROM legacy_agent_conversion_intents WHERE project_id=?",
            [project_id],
            |row| {
                Ok(LegacyAgentConversionIntent {
                    project_id: row.get(0)?,
                    legacy_version_id: row.get(1)?,
                    legacy_module_graph_id: row.get(2)?,
                    snapshot_revision: row.get(3)?,
                    requested_at: row.get(4)?,
                })
            },
        )
        .optional()?;
    intent
        .map(|intent| {
            intent.validate()?;
            Ok(intent)
        })
        .transpose()
}

fn candidate_bundle_activation(
    transaction: &Transaction<'_>,
    project_id: &str,
) -> CoreResult<CandidateBundleActivation> {
    let has_agent_state: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM agent_asset_heads WHERE project_id=? UNION ALL SELECT 1 FROM agent_asset_versions WHERE project_id=?)",
        params![project_id, project_id],
        |row| row.get(0),
    )?;
    if has_agent_state {
        return Err(CoreError::conflict(
            "CANDIDATE_BUNDLE_PROJECT_ALREADY_INITIALIZED",
            "Candidate bundle cannot replace an existing authoritative Agent design chain.",
        ));
    }
    let snapshot = snapshot_from_connection(transaction, project_id)?;
    let intent = legacy_conversion_intent_from_connection(transaction, project_id)?;
    let Some(snapshot) = snapshot else {
        if intent.is_some() {
            return Err(CoreError::conflict(
                "LEGACY_CONVERSION_AUTHORIZATION_STALE",
                "Legacy conversion authorization has no matching durable Snapshot.",
            ));
        }
        let has_legacy_current: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects p JOIN project_versions pv ON pv.version_id=p.current_version_id AND pv.project_id=p.project_id JOIN module_graphs mg ON mg.graph_id=pv.module_graph_id AND mg.project_id=p.project_id WHERE p.project_id=? AND pv.status!='soft_deleted')",
            [project_id],
            |row| row.get(0),
        )?;
        if has_legacy_current {
            return Err(CoreError::conflict(
                "LEGACY_CONVERSION_NOT_AUTHORIZED",
                "A Project with a legacy current design requires explicit conversion authorization before the first Agent commit.",
            ));
        }
        return Ok(CandidateBundleActivation::FreshProject);
    };
    let ActiveDesign::LegacyConceptReadOnly {
        legacy_version_id,
        module_graph_id,
        ..
    } = &snapshot.active_design
    else {
        return Err(CoreError::conflict(
            "CANDIDATE_BUNDLE_PROJECT_ALREADY_INITIALIZED",
            "Candidate bundle cannot replace an existing authoritative Agent design chain.",
        ));
    };
    let intent = intent.ok_or_else(|| {
        CoreError::conflict(
            "LEGACY_CONVERSION_NOT_AUTHORIZED",
            "An explicit legacy conversion authorization is required before the first Agent commit.",
        )
    })?;
    if intent.project_id != project_id
        || intent.legacy_version_id != *legacy_version_id
        || intent.legacy_module_graph_id != *module_graph_id
        || intent.snapshot_revision != snapshot.revision
    {
        return Err(CoreError::conflict(
            "LEGACY_CONVERSION_AUTHORIZATION_STALE",
            "Legacy conversion authorization no longer matches the exact read-only Snapshot source and revision.",
        ));
    }
    require_legacy_source_binding(transaction, project_id, legacy_version_id, module_graph_id)?;
    Ok(CandidateBundleActivation::AuthorizedLegacy(intent))
}

fn promote_authorized_legacy_snapshot(
    transaction: &Transaction<'_>,
    intent: &LegacyAgentConversionIntent,
    version: &AgentAssetVersion,
    quality: &QualityReport,
) -> CoreResult<()> {
    intent.validate()?;
    let render = RenderPreset::default_for(
        &version.project_id,
        &version.asset_version_id,
        &quality.created_at,
    );
    let part_display = PartDisplay::empty(&version.project_id, &version.asset_version_id);
    let changed = transaction.execute(
        "UPDATE active_design_snapshots SET source='agent_asset', active_asset_version_id=?, active_assembly_graph_id=?, legacy_version_id=NULL, legacy_module_graph_id=NULL, selected_part_id=NULL, selected_material_zone_id=NULL, preview_change_set_id=NULL, preview_base_asset_version_id=NULL, quality_report_id=?, quality_asset_version_id=?, export_source='agent_asset', export_source_version_id=?, render_preset_json=?, part_display_json=?, revision=revision+1, updated_at=? WHERE project_id=? AND source='legacy_concept_read_only' AND legacy_version_id=? AND legacy_module_graph_id=? AND revision=?",
        params![
            version.asset_version_id,
            version.assembly_graph_id()?,
            quality.quality_report_id,
            quality.asset_version_id,
            version.asset_version_id,
            json_text(&render)?,
            json_text(&part_display)?,
            quality.created_at,
            intent.project_id,
            intent.legacy_version_id,
            intent.legacy_module_graph_id,
            intent.snapshot_revision,
        ],
    )?;
    if changed != 1 {
        return Err(CoreError::conflict(
            "LEGACY_CONVERSION_AUTHORIZATION_STALE",
            "Legacy Snapshot changed before the authorized Agent bundle could be activated.",
        ));
    }
    let consumed = transaction.execute(
        "DELETE FROM legacy_agent_conversion_intents WHERE project_id=? AND legacy_version_id=? AND legacy_module_graph_id=? AND snapshot_revision=?",
        params![
            intent.project_id,
            intent.legacy_version_id,
            intent.legacy_module_graph_id,
            intent.snapshot_revision,
        ],
    )?;
    if consumed != 1 {
        return Err(CoreError::conflict(
            "LEGACY_CONVERSION_AUTHORIZATION_STALE",
            "Legacy conversion authorization could not be consumed atomically.",
        ));
    }
    Ok(())
}

fn snapshot_from_connection(
    connection: &Connection,
    project_id: &str,
) -> CoreResult<Option<ActiveDesignSnapshot>> {
    connection.query_row(
        "SELECT project_id, source, active_asset_version_id, active_assembly_graph_id, legacy_version_id, legacy_module_graph_id, selected_part_id, preview_change_set_id, preview_base_asset_version_id, quality_report_id, quality_asset_version_id, export_source, export_source_version_id, revision, updated_at, render_preset_json, selected_material_zone_id, part_display_json FROM active_design_snapshots WHERE project_id=?",
        [project_id], snapshot_from_row,
    ).optional().map_err(Into::into)
}

fn require_snapshot(connection: &Connection, project_id: &str) -> CoreResult<ActiveDesignSnapshot> {
    snapshot_from_connection(connection, project_id)?
        .ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))
}

fn require_agent_snapshot(
    connection: &Connection,
    project_id: &str,
    expected: SnapshotEtag,
) -> CoreResult<ActiveDesignSnapshot> {
    let snapshot = require_snapshot(connection, project_id)?;
    if snapshot.revision != expected.0 {
        return Err(stale("ACTIVE_DESIGN_STALE"));
    }
    if !matches!(snapshot.active_design, ActiveDesign::AgentAsset { .. }) {
        return Err(CoreError::conflict(
            "ACTIVE_DESIGN_LEGACY_READ_ONLY",
            "Legacy design is read-only.",
        ));
    }
    Ok(snapshot)
}

fn require_active_version(
    connection: &Connection,
    snapshot: &ActiveDesignSnapshot,
) -> CoreResult<AgentAssetVersion> {
    require_version(
        connection,
        snapshot.active_design.asset_version_id().ok_or_else(|| {
            CoreError::conflict(
                "ACTIVE_DESIGN_LEGACY_READ_ONLY",
                "Legacy design is read-only.",
            )
        })?,
    )
}

fn snapshot_from_row(row: &Row<'_>) -> rusqlite::Result<ActiveDesignSnapshot> {
    let project_id: String = row.get(0)?;
    let source: String = row.get(1)?;
    let asset_id: Option<String> = row.get(2)?;
    let graph_id: Option<String> = row.get(3)?;
    let legacy_version: Option<String> = row.get(4)?;
    let legacy_graph: Option<String> = row.get(5)?;
    let active_design = match source.as_str() {
        "agent_asset" => ActiveDesign::AgentAsset {
            project_id: project_id.clone(),
            asset_version_id: asset_id.ok_or_else(|| rusqlite::Error::InvalidQuery)?,
            assembly_graph_id: graph_id.ok_or_else(|| rusqlite::Error::InvalidQuery)?,
        },
        "legacy_concept_read_only" => ActiveDesign::LegacyConceptReadOnly {
            project_id: project_id.clone(),
            legacy_version_id: legacy_version.ok_or_else(|| rusqlite::Error::InvalidQuery)?,
            module_graph_id: legacy_graph.ok_or_else(|| rusqlite::Error::InvalidQuery)?,
        },
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let preview_id: Option<String> = row.get(7)?;
    let preview_base: Option<String> = row.get(8)?;
    let quality_id: Option<String> = row.get(9)?;
    let quality_asset: Option<String> = row.get(10)?;
    let export_source: String = row.get(11)?;
    let export_id: String = row.get(12)?;
    let snapshot = ActiveDesignSnapshot {
        schema_version: "ActiveDesignSnapshot@1".to_string(),
        project_id: project_id.clone(),
        active_design,
        selected_part_id: row.get(6)?,
        selected_material_zone_id: row.get(16)?,
        preview: match (preview_id, preview_base) {
            (Some(change_set_id), Some(base_asset_version_id)) => Some(PreviewReference {
                project_id: project_id.clone(),
                change_set_id,
                base_asset_version_id,
            }),
            (None, None) => None,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        quality: match (quality_id, quality_asset) {
            (Some(quality_report_id), Some(asset_version_id)) => Some(QualityReference {
                project_id: project_id.clone(),
                quality_report_id,
                asset_version_id,
            }),
            (None, None) => None,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        export: match export_source.as_str() {
            "agent_asset" => ExportReference::AgentAsset {
                project_id: project_id.clone(),
                source_version_id: export_id,
            },
            "legacy_concept_read_only" => ExportReference::LegacyConceptReadOnly {
                project_id: project_id.clone(),
                source_version_id: export_id,
            },
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        render_preset: parse_optional_json(row.get(15)?).map_err(to_sql_error)?,
        part_display: parse_optional_json(row.get(17)?).map_err(to_sql_error)?,
        revision: row.get(13)?,
        updated_at: row.get(14)?,
    };
    snapshot.validate().map_err(to_sql_error)?;
    Ok(snapshot)
}

fn change_set_from_connection(
    connection: &Connection,
    id: &str,
) -> CoreResult<Option<AgentAssetChangeSet>> {
    connection.query_row(
        "SELECT change_set_id, project_id, base_asset_version_id, summary, operations_json, protected_part_ids_json, preview_json, status, resulting_asset_version_id, created_at, updated_at FROM agent_asset_change_sets WHERE change_set_id=?",
        [id], change_set_from_row,
    ).optional().map_err(Into::into)
}

fn require_change_set(connection: &Connection, id: &str) -> CoreResult<AgentAssetChangeSet> {
    change_set_from_connection(connection, id)?
        .ok_or_else(|| CoreError::not_found("Agent ChangeSet"))
}

fn reference_evidence_from_connection(
    connection: &Connection,
    evidence_id: &str,
) -> CoreResult<Option<ReferenceEvidence>> {
    connection.query_row(
        "SELECT evidence_id, project_id, kind, reference_class, domain_pack_id, source_file_name, source_media_type, source_object_sha256, source_imported_asset_version_id, source_statement, license_statement, missing_views_json, user_notes, observations_json, glb_inspection_json, created_at FROM reference_evidence WHERE evidence_id=?",
        [evidence_id],
        |row| {
            let kind: String = row.get(2)?;
            let reference_class: String = row.get(3)?;
            let value = ReferenceEvidence {
                schema_version: REFERENCE_EVIDENCE_SCHEMA_VERSION.into(),
                evidence_id: row.get(0)?, project_id: row.get(1)?,
                kind: ReferenceEvidenceKind::from_str(&kind).map_err(to_sql_error)?,
                reference_class: crate::ReferenceClass::from_str(&reference_class).map_err(to_sql_error)?,
                domain_pack_id: row.get(4)?, source_file_name: row.get(5)?, source_media_type: row.get(6)?,
                source_object_sha256: row.get(7)?, source_imported_asset_version_id: row.get(8)?,
                source_statement: row.get(9)?, license_statement: row.get(10)?,
                missing_views: parse_json(row.get::<_, String>(11)?).map_err(to_sql_error)?,
                user_notes: row.get(12)?, observations: parse_json(row.get::<_, String>(13)?).map_err(to_sql_error)?,
                glb_inspection: parse_optional_json(row.get(14)?).map_err(to_sql_error)?, created_at: row.get(15)?,
            };
            value.validate().map_err(to_sql_error)?;
            Ok(value)
        },
    ).optional().map_err(Into::into)
}

fn reference_rebuild_plan_from_connection(
    connection: &Connection,
    rebuild_plan_id: &str,
) -> CoreResult<Option<ReferenceGuidedRebuildPlan>> {
    connection.query_row(
        "SELECT rebuild_plan_id, project_id, evidence_id, base_asset_version_id, domain_pack_id, recipe_id, recipe_registry_sha256, rebuild_summary, intended_differences_json, retained_evidence_json, unresolved_uncertainties_json, status, preview_change_set_id, confirmed_asset_version_id, created_at, updated_at FROM reference_guided_rebuild_plans WHERE rebuild_plan_id=?",
        [rebuild_plan_id],
        |row| {
            let status: String = row.get(11)?;
            let value = ReferenceGuidedRebuildPlan {
                schema_version: REFERENCE_GUIDED_REBUILD_PLAN_SCHEMA_VERSION.into(),
                rebuild_plan_id: row.get(0)?, project_id: row.get(1)?, evidence_id: row.get(2)?,
                base_asset_version_id: row.get(3)?, domain_pack_id: row.get(4)?, recipe_id: row.get(5)?,
                recipe_registry_sha256: row.get(6)?, rebuild_summary: row.get(7)?,
                intended_differences: parse_json(row.get::<_, String>(8)?).map_err(to_sql_error)?,
                retained_evidence: parse_json(row.get::<_, String>(9)?).map_err(to_sql_error)?,
                unresolved_uncertainties: parse_json(row.get::<_, String>(10)?).map_err(to_sql_error)?,
                status: ReferenceGuidedRebuildPlanStatus::from_str(&status).map_err(to_sql_error)?,
                preview_change_set_id: row.get(12)?, confirmed_asset_version_id: row.get(13)?,
                created_at: row.get(14)?, updated_at: row.get(15)?,
            };
            value.validate().map_err(to_sql_error)?;
            Ok(value)
        },
    ).optional().map_err(Into::into)
}

fn require_reference_rebuild_plan(
    connection: &Connection,
    rebuild_plan_id: &str,
) -> CoreResult<ReferenceGuidedRebuildPlan> {
    reference_rebuild_plan_from_connection(connection, rebuild_plan_id)?
        .ok_or_else(|| CoreError::not_found("Reference guided rebuild plan"))
}

fn reference_surface_analysis_from_connection(
    connection: &Connection,
    rebuild_plan_id: &str,
) -> CoreResult<Option<ReferenceSurfaceAnalysis>> {
    connection
        .query_row(
            "SELECT analysis_sha256, analysis_json FROM reference_surface_analyses WHERE rebuild_plan_id=?",
            [rebuild_plan_id],
            |row| {
                let expected_sha256: String = row.get(0)?;
                let analysis: ReferenceSurfaceAnalysis =
                    parse_json(row.get::<_, String>(1)?).map_err(to_sql_error)?;
                let actual_sha256 = crate::semantic_sha256(&analysis).map_err(to_sql_error)?;
                if actual_sha256 != expected_sha256 {
                    return Err(to_sql_error(CoreError::conflict(
                        "REFERENCE_SURFACE_ANALYSIS_HASH_MISMATCH",
                        "Frozen reference surface analysis no longer matches its canonical hash.",
                    )));
                }
                Ok(analysis)
            },
        )
        .optional()
        .map_err(Into::into)
}

/// Finishes the ordinary ChangeSet lifecycle for an R007 plan.  R007A keeps
/// its existing plan-only behaviour; an R007B plan is not confirmed unless the
/// exact frozen analysis, original evidence and real production CAS object all
/// agree within this same SQLite transaction.
fn finalize_reference_rebuild_result(
    transaction: &Transaction<'_>,
    change_set_id: &str,
    resulting_asset_version_id: &str,
    production_glb_sha256: Option<&str>,
    updated_at: &str,
) -> CoreResult<()> {
    let row: Option<(String, String, String, Option<String>, Option<String>)> = transaction
        .query_row(
            "SELECT plan.rebuild_plan_id, plan.evidence_id, evidence.source_object_sha256, analysis.analysis_sha256, analysis.analysis_json \
             FROM reference_guided_rebuild_plans AS plan \
             JOIN reference_evidence AS evidence ON evidence.evidence_id=plan.evidence_id \
             LEFT JOIN reference_surface_analyses AS analysis ON analysis.rebuild_plan_id=plan.rebuild_plan_id \
             WHERE plan.preview_change_set_id=? AND plan.status='previewed'",
            [change_set_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((rebuild_plan_id, evidence_id, source_object_sha256, analysis_sha256, analysis_json)) =
        row
    else {
        return Ok(());
    };

    match (analysis_sha256, analysis_json) {
        (None, None) => {}
        (Some(expected_sha256), Some(analysis_json)) => {
            let production_glb_sha256 = production_glb_sha256.ok_or_else(|| {
                CoreError::conflict(
                    "REFERENCE_SURFACE_RESULT_BUNDLE_REQUIRED",
                    "Frozen reference surface plans require bundled confirmation with a production GLB.",
                )
            })?;
            let analysis: ReferenceSurfaceAnalysis = parse_json(analysis_json)?;
            let actual_sha256 = crate::semantic_sha256(&analysis)?;
            if actual_sha256 != expected_sha256
                || analysis.rebuild_plan_id != rebuild_plan_id
                || analysis.evidence_id != evidence_id
                || analysis.source_object_sha256 != source_object_sha256
            {
                return Err(CoreError::conflict(
                    "REFERENCE_SURFACE_ANALYSIS_BINDING_INVALID",
                    "Frozen reference surface analysis is not bound to this plan and immutable evidence source.",
                ));
            }
            if source_object_sha256 == production_glb_sha256 {
                return Err(CoreError::conflict(
                    "REFERENCE_SURFACE_RESULT_EQUALS_SOURCE",
                    "Reference-guided rebuild result must be a new ForgeCAD GLB, never the source object.",
                ));
            }
            let production_binding_exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM forgecad_core_object_references WHERE reference_kind='asset_version' AND owner_id=? AND role='production_glb' AND sha256=?)",
                params![resulting_asset_version_id, production_glb_sha256],
                |row| row.get(0),
            )?;
            if !production_binding_exists {
                return Err(CoreError::conflict(
                    "REFERENCE_SURFACE_RESULT_GLB_MISSING",
                    "Frozen reference result has no matching production GLB object binding.",
                ));
            }
            transaction.execute(
                "INSERT INTO reference_rebuild_result_lineage(asset_version_id, rebuild_plan_id, source_result_asset_version_id, created_at) VALUES (?, ?, ?, ?)",
                params![resulting_asset_version_id, rebuild_plan_id, resulting_asset_version_id, updated_at],
            )?;
        }
        _ => {
            return Err(CoreError::conflict(
                "REFERENCE_SURFACE_ANALYSIS_ROW_INCOMPLETE",
                "Frozen reference surface analysis row is incomplete.",
            ));
        }
    }
    transaction.execute(
        "UPDATE reference_guided_rebuild_plans SET status='confirmed', confirmed_asset_version_id=?, updated_at=? WHERE rebuild_plan_id=? AND status='previewed'",
        params![resulting_asset_version_id, updated_at, rebuild_plan_id],
    )?;
    Ok(())
}

fn copy_reference_rebuild_result_lineage(
    transaction: &Transaction<'_>,
    source_asset_version_id: &str,
    resulting_asset_version_id: &str,
    created_at: &str,
) -> CoreResult<()> {
    let lineage: Option<(String, String)> = transaction
        .query_row(
            "SELECT rebuild_plan_id, source_result_asset_version_id FROM reference_rebuild_result_lineage WHERE asset_version_id=?",
            [source_asset_version_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((rebuild_plan_id, source_result_asset_version_id)) = lineage {
        transaction.execute(
            "INSERT INTO reference_rebuild_result_lineage(asset_version_id, rebuild_plan_id, source_result_asset_version_id, created_at) VALUES (?, ?, ?, ?)",
            params![resulting_asset_version_id, rebuild_plan_id, source_result_asset_version_id, created_at],
        )?;
    }
    Ok(())
}

fn derive_reference_observations(
    request: &CreateReferenceEvidenceRequest,
    source: &crate::reference_evidence::ResolvedReferenceSource,
) -> crate::ReferenceEvidenceObservations {
    match (&request.kind, &source.glb_inspection) {
        (ReferenceEvidenceKind::Glb, Some(inspection)) => {
            let [x, y, z] = inspection.bounds_mm;
            crate::ReferenceEvidenceObservations {
                silhouette_summary: format!(
                    "基于用户授权的只读 GLB 外观证据：{} 个网格、{} 个 primitive；仅用于可见轮廓与比例，不恢复隐藏结构。",
                    inspection.mesh_count, inspection.primitive_count
                ),
                proportion_ranges: vec![format!(
                    "已读取包围范围 {:.1} × {:.1} × {:.1} mm；仅作为相对比例区间，不是制造尺寸。", x, y, z
                )],
                material_zone_observations: vec![format!(
                    "GLB 读取到 {} 个可见材质槽；材质名称和物理属性不作为事实恢复。", inspection.material_count
                )],
                visible_part_hypotheses: derive_visible_part_hypotheses(&request.user_notes),
                uncertainties: vec![
                    "导入 GLB 保持只读；其内部层级、连接关系、材料配方和功能均未被推断。".into(),
                    "重建将使用受限 Recipe 和 ShapeProgram 生成新资产，而不是复制源网格。".into(),
                ],
                image_surface_facts: None,
            }
        }
        (ReferenceEvidenceKind::Image, _) => {
            let facts = source
                .image_surface_facts
                .as_ref()
                .expect("image source is locally analysed");
            let [left, top, right, bottom] = facts.foreground_bbox_normalized;
            let mut visible_part_hypotheses = derive_visible_part_hypotheses(&request.user_notes);
            // This is deliberately a *visible-form* suggestion, not a claim
            // about mechanics.  It gives the sealed R007B plan a real local
            // pixel-derived cue without inferring hidden components.
            let bbox_width = right.saturating_sub(left);
            let bbox_height = bottom.saturating_sub(top);
            let pixel_role = if bbox_width > bbox_height.saturating_mul(5) / 4 {
                "upper_link_form"
            } else {
                "base_form"
            };
            if !visible_part_hypotheses
                .iter()
                .any(|item| item.role == pixel_role)
            {
                visible_part_hypotheses.insert(0, crate::VisiblePartHypothesis {
                    role: pixel_role.into(),
                    confidence: match facts.foreground_confidence {
                        crate::ReferenceImageForegroundConfidence::Low => "low".into(),
                        crate::ReferenceImageForegroundConfidence::Medium => "medium".into(),
                    },
                    visible_basis: format!(
                        "local_pixel_heuristic: 归一化前景包围框 [{left},{top},{right},{bottom}]、{} 边缘密度；仅表示可见外形提示，不推断内部结构。",
                        image_edge_density_label(facts.edge_density),
                    ),
                });
            }
            crate::ReferenceEvidenceObservations {
                silhouette_summary: format!(
                    "本地受限像素分析：{}×{} 画面、前景包围框 [{left},{top},{right},{bottom}]（{} 置信度）；仅保留可见二维轮廓范围。",
                    facts.width,
                    facts.height,
                    image_foreground_confidence_label(facts.foreground_confidence),
                ),
                proportion_ranges: vec![format!(
                    "本地画面宽高比约 {:.3}；仅作为相对可见比例提示，不声明真实尺寸或制造数据。",
                    f64::from(facts.aspect_ratio_milli) / 1_000.0
                )],
                material_zone_observations: vec![format!(
                    "本地像素色块提示：{}；亮度{}、边缘密度{}。仅用于视觉材质区和表面语言，不证明真实材料。",
                    image_color_bucket_labels(&facts.dominant_color_buckets),
                    image_brightness_label(facts.brightness),
                    image_edge_density_label(facts.edge_density),
                )],
                visible_part_hypotheses,
                uncertainties: vec![
                    "单图或有限视角无法恢复背面、内部结构、精确尺寸、材料或功能。".into(),
                    "前景包围框、色块、亮度和边缘密度均为本地启发式低维事实，不是精确轮廓或对象分割。".into(),
                    "用户备注被保留为声明证据，不会被当作已验证几何事实。".into(),
                ],
                image_surface_facts: Some(facts.clone()),
            }
        }
        (ReferenceEvidenceKind::Glb, None) => {
            // Construction resolves every GLB path through strict inspection.
            // Keep this explicit so an accidental future source variant cannot
            // receive the image fallback or invent unverified observations.
            unreachable!("validated GLB evidence must retain strict inspection")
        }
    }
}

fn image_color_bucket_labels(buckets: &[crate::ReferenceImageColorBucket]) -> String {
    buckets
        .iter()
        .map(|bucket| match bucket {
            crate::ReferenceImageColorBucket::Black => "黑",
            crate::ReferenceImageColorBucket::Gray => "灰",
            crate::ReferenceImageColorBucket::White => "白",
            crate::ReferenceImageColorBucket::Blue => "蓝",
            crate::ReferenceImageColorBucket::Cyan => "青",
            crate::ReferenceImageColorBucket::Red => "红",
            crate::ReferenceImageColorBucket::Yellow => "黄",
            crate::ReferenceImageColorBucket::Green => "绿",
            crate::ReferenceImageColorBucket::Violet => "紫",
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn image_brightness_label(bucket: crate::ReferenceImageBrightnessBucket) -> &'static str {
    match bucket {
        crate::ReferenceImageBrightnessBucket::Dark => "偏暗",
        crate::ReferenceImageBrightnessBucket::Balanced => "平衡",
        crate::ReferenceImageBrightnessBucket::Bright => "偏亮",
    }
}

fn image_edge_density_label(bucket: crate::ReferenceImageEdgeDensityBucket) -> &'static str {
    match bucket {
        crate::ReferenceImageEdgeDensityBucket::Low => "低",
        crate::ReferenceImageEdgeDensityBucket::Medium => "中",
        crate::ReferenceImageEdgeDensityBucket::High => "高",
    }
}

fn image_foreground_confidence_label(
    confidence: crate::ReferenceImageForegroundConfidence,
) -> &'static str {
    match confidence {
        crate::ReferenceImageForegroundConfidence::Low => "低",
        crate::ReferenceImageForegroundConfidence::Medium => "中",
    }
}

fn derive_visible_part_hypotheses(user_notes: &str) -> Vec<crate::VisiblePartHypothesis> {
    let normalized = user_notes.to_lowercase();
    let candidates = [
        (("base", "底座"), "base_form", "用户声明的可见底座外形"),
        (("joint", "关节"), "joint_housing", "用户声明的可见关节外壳"),
        (
            ("link", "臂段"),
            "upper_link_form",
            "用户声明的可见连杆/臂段外形",
        ),
        (
            ("cable", "线缆"),
            "visual_cable",
            "用户声明的可见线缆视觉元素",
        ),
        (
            ("end effector", "夹爪"),
            "end_effector_form",
            "用户声明的可见末端执行器外形",
        ),
    ];
    let mut hypotheses = candidates
        .into_iter()
        .filter(|((english, chinese), _, _)| {
            normalized.contains(english) || user_notes.contains(chinese)
        })
        .map(|(_, role, detail)| crate::VisiblePartHypothesis {
            role: role.into(),
            confidence: "low".into(),
            visible_basis: format!(
                "user_declared_visual_note: {detail}；未验证，也不推断隐藏结构。"
            ),
        })
        .collect::<Vec<_>>();
    if hypotheses.is_empty() {
        hypotheses.push(crate::VisiblePartHypothesis {
            role: "primary_form".into(), confidence: "low".into(),
            visible_basis: "user_declared_visual_note: 未提供可验证部件标签；仅保留主可见形体假设，不推断隐藏结构。".into(),
        });
    }
    hypotheses
}

fn change_set_from_row(row: &Row<'_>) -> rusqlite::Result<AgentAssetChangeSet> {
    let status: String = row.get(7)?;
    Ok(AgentAssetChangeSet {
        change_set_id: row.get(0)?,
        project_id: row.get(1)?,
        base_asset_version_id: row.get(2)?,
        summary: row.get(3)?,
        operations: parse_json(row.get::<_, String>(4)?).map_err(to_sql_error)?,
        protected_part_ids: parse_json(row.get::<_, String>(5)?).map_err(to_sql_error)?,
        preview: parse_optional_json(row.get(6)?).map_err(to_sql_error)?,
        status: ChangeSetStatus::from_str(&status).map_err(to_sql_error)?,
        resulting_asset_version_id: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn quality_from_connection(connection: &Connection, id: &str) -> CoreResult<Option<QualityReport>> {
    connection.query_row(
        "SELECT quality_report_id, project_id, asset_version_id, report_json, status, created_at FROM agent_asset_quality_reports WHERE quality_report_id=?",
        [id], |row| {
            let status: String = row.get(4)?;
            Ok(QualityReport { quality_report_id: row.get(0)?, project_id: row.get(1)?, asset_version_id: row.get(2)?, report: parse_json(row.get::<_, String>(3)?).map_err(to_sql_error)?, status: crate::QualityStatus::from_str(&status).map_err(to_sql_error)?, created_at: row.get(5)? })
        },
    ).optional().map_err(Into::into)
}

fn require_quality(connection: &Connection, id: &str) -> CoreResult<QualityReport> {
    quality_from_connection(connection, id)?
        .ok_or_else(|| CoreError::not_found("Agent asset quality report"))
}

fn navigation_targets(
    connection: &Connection,
    version: &AgentAssetVersion,
) -> CoreResult<(Option<String>, Option<String>)> {
    let frame: Option<(Option<String>, Option<String>)> = connection.query_row(
        "SELECT undo_target_asset_version_id, redo_target_asset_version_id FROM agent_asset_navigation_frames WHERE resulting_asset_version_id=?",
        [&version.asset_version_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional()?;
    Ok(frame.unwrap_or_else(|| (version.parent_asset_version_id.clone(), None)))
}

fn validate_selection(
    version: &AgentAssetVersion,
    part: Option<&str>,
    zone: Option<&str>,
) -> CoreResult<()> {
    if zone.is_some() && part.is_none() {
        return Err(CoreError::invalid_data(
            "ACTIVE_DESIGN_SELECTION_INVALID",
            "Material Zone requires a part selection.",
        ));
    }
    let index = version.part_zone_index()?;
    if let Some(part) = part {
        let zones = index.get(part).ok_or_else(|| {
            CoreError::invalid_data(
                "ACTIVE_DESIGN_PART_INVALID",
                "Selected part is not in the active AssemblyGraph.",
            )
        })?;
        if let Some(zone) = zone {
            if !zones.iter().any(|value| value == zone) {
                return Err(CoreError::invalid_data(
                    "ACTIVE_DESIGN_ZONE_INVALID",
                    "Selected Material Zone is not on the selected part.",
                ));
            }
        }
    }
    Ok(())
}

fn change_set_preview_bundle_from_connection(
    connection: &Connection,
    change_set_id: &str,
) -> CoreResult<Option<ChangeSetPreviewBundleReadback>> {
    let preview_object = object_for_reference_from_connection(
        connection,
        "preview",
        change_set_id,
        "interactive_preview_glb",
    )?;
    let Some(change_set) = change_set_from_connection(connection, change_set_id)? else {
        return if preview_object.is_some() {
            Err(change_set_bundle_incomplete(
                "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                preview_bundle_ids(change_set_id),
                vec!["change_set"],
            ))
        } else {
            Ok(None)
        };
    };
    let snapshot = snapshot_from_connection(connection, &change_set.project_id)?;
    let snapshot_names_preview = snapshot.as_ref().is_some_and(|value| {
        value
            .preview
            .as_ref()
            .is_some_and(|preview| preview.change_set_id == change_set_id)
    });
    match change_set.status {
        ChangeSetStatus::Proposed => {
            if change_set.preview.is_some() || preview_object.is_some() || snapshot_names_preview {
                return Err(change_set_bundle_incomplete(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    preview_bundle_ids(change_set_id),
                    vec!["proposed_state_cleanup"],
                ));
            }
            return Ok(None);
        }
        ChangeSetStatus::Confirmed => {
            if preview_object.is_some() || snapshot_names_preview {
                return Err(change_set_bundle_incomplete(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    preview_bundle_ids(change_set_id),
                    vec!["resolved_preview_cleanup"],
                ));
            }
            return Ok(None);
        }
        ChangeSetStatus::Rejected | ChangeSetStatus::Stale => {
            if change_set.preview.is_some() || preview_object.is_some() || snapshot_names_preview {
                return Err(change_set_bundle_incomplete(
                    "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                    preview_bundle_ids(change_set_id),
                    vec!["resolved_preview_cleanup"],
                ));
            }
            return Ok(None);
        }
        ChangeSetStatus::Previewed => {}
    }

    let mut missing = Vec::new();
    if change_set.preview.is_none() {
        missing.push("preview_seal");
    }
    if preview_object.is_none() {
        missing.push("interactive_preview_glb");
    }
    if snapshot.is_none() || !snapshot_names_preview {
        missing.push("snapshot_preview");
    }
    if !missing.is_empty() {
        return Err(change_set_bundle_incomplete(
            "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
            preview_bundle_ids(change_set_id),
            missing,
        ));
    }
    let seal = parse_change_set_preview_seal(change_set.preview.as_ref().expect("checked"))
        .map_err(|_| {
            change_set_bundle_incomplete(
                "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
                preview_bundle_ids(change_set_id),
                vec!["preview_seal"],
            )
        })?;
    let bundle = ChangeSetPreviewBundleReadback {
        change_set,
        sealed_preview: seal.sealed_preview,
        snapshot: snapshot.expect("checked"),
        interactive_preview_glb: preview_object.expect("checked"),
        interactive_readback: seal.interactive_readback,
    };
    let head: String = connection
        .query_row(
            "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
            [&bundle.change_set.project_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| CoreError::not_found("Agent asset head"))?;
    if let Err(error) = validate_change_set_preview_readback(connection, &bundle, &head) {
        let mut details = preview_bundle_ids(change_set_id);
        if let Some(object) = details.as_object_mut() {
            object.insert("cause_code".into(), error.code().into());
        }
        return Err(change_set_bundle_incomplete(
            "CHANGE_SET_PREVIEW_BUNDLE_INCOMPLETE",
            details,
            vec!["semantic_consistency"],
        ));
    }
    Ok(Some(bundle))
}

fn change_set_confirm_bundle_from_connection(
    connection: &Connection,
    change_set_id: &str,
    resulting_asset_version_id: &str,
    quality_report_id: &str,
) -> CoreResult<Option<ChangeSetConfirmBundleReadback>> {
    let change_set = change_set_from_connection(connection, change_set_id)?;
    let version = version_from_connection(connection, resulting_asset_version_id)?;
    let quality = quality_from_connection(connection, quality_report_id)?;
    let production = object_for_reference_from_connection(
        connection,
        "asset_version",
        resulting_asset_version_id,
        "production_glb",
    )?;
    let interactive = object_for_reference_from_connection(
        connection,
        "asset_version",
        resulting_asset_version_id,
        "interactive_preview_glb",
    )?;
    let preview_reference = object_for_reference_from_connection(
        connection,
        "preview",
        change_set_id,
        "interactive_preview_glb",
    )?;
    let confirmation_specific = version.is_some()
        || quality.is_some()
        || production.is_some()
        || interactive.is_some()
        || change_set.as_ref().is_some_and(|value| {
            value.status == ChangeSetStatus::Confirmed || value.resulting_asset_version_id.is_some()
        });
    if !confirmation_specific {
        return Ok(None);
    }
    let Some(change_set) = change_set else {
        return Err(change_set_bundle_incomplete(
            "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
            confirm_bundle_ids(change_set_id, resulting_asset_version_id, quality_report_id),
            vec!["change_set"],
        ));
    };
    if change_set.status == ChangeSetStatus::Confirmed {
        let seal = change_set
            .preview
            .as_ref()
            .ok_or_else(|| {
                change_set_bundle_incomplete(
                    "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                    confirm_bundle_ids(
                        change_set_id,
                        resulting_asset_version_id,
                        quality_report_id,
                    ),
                    vec!["confirm_seal"],
                )
            })
            .and_then(|value| {
                parse_change_set_confirm_seal(value).map_err(|_| {
                    change_set_bundle_incomplete(
                        "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
                        confirm_bundle_ids(
                            change_set_id,
                            resulting_asset_version_id,
                            quality_report_id,
                        ),
                        vec!["confirm_seal"],
                    )
                })
            })?;
        if change_set.resulting_asset_version_id.as_deref() != Some(resulting_asset_version_id)
            || seal.resulting_asset_version_id != resulting_asset_version_id
            || seal.quality_report_id != quality_report_id
        {
            return Err(CoreError::conflict_with_details(
                "CHANGE_SET_CONFIRM_BUNDLE_IDEMPOTENCY_CONFLICT",
                "ChangeSet was confirmed with a different sealed result identity.",
                confirm_bundle_ids(change_set_id, resulting_asset_version_id, quality_report_id),
            ));
        }
    }

    let project_id = version
        .as_ref()
        .map(|value| value.project_id.as_str())
        .unwrap_or(change_set.project_id.as_str());
    let snapshot = snapshot_from_connection(connection, project_id)?;
    let head = connection
        .query_row(
            "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
            [project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let base_superseded = connection
        .query_row(
            "SELECT status='superseded' FROM agent_asset_versions WHERE asset_version_id=?",
            [&change_set.base_asset_version_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false);
    let mut missing = Vec::new();
    if change_set.status != ChangeSetStatus::Confirmed {
        missing.push("confirmed_change_set");
    }
    if version.is_none() {
        missing.push("resulting_version");
    }
    if quality.is_none() {
        missing.push("quality");
    }
    if production.is_none() {
        missing.push("production_glb");
    }
    if interactive.is_none() {
        missing.push("interactive_preview_glb");
    }
    if snapshot.is_none() {
        missing.push("snapshot");
    }
    if head.as_deref() != Some(resulting_asset_version_id) {
        missing.push("head");
    }
    if preview_reference.is_some() {
        missing.push("preview_reference_cleanup");
    }
    if !base_superseded {
        missing.push("superseded_base");
    }
    if !missing.is_empty() {
        return Err(change_set_bundle_incomplete(
            "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
            confirm_bundle_ids(change_set_id, resulting_asset_version_id, quality_report_id),
            missing,
        ));
    }
    let bundle = ChangeSetConfirmBundleReadback {
        change_set,
        version: version.expect("checked"),
        snapshot: snapshot.expect("checked"),
        quality: quality.expect("checked"),
        production_glb: production.expect("checked"),
        interactive_preview_glb: interactive.expect("checked"),
    };
    if let Err(error) = validate_change_set_confirm_readback(&bundle, head.as_deref()) {
        let mut details =
            confirm_bundle_ids(change_set_id, resulting_asset_version_id, quality_report_id);
        if let Some(object) = details.as_object_mut() {
            object.insert("cause_code".into(), error.code().into());
        }
        return Err(change_set_bundle_incomplete(
            "CHANGE_SET_CONFIRM_BUNDLE_INCOMPLETE",
            details,
            vec!["semantic_consistency"],
        ));
    }
    Ok(Some(bundle))
}

fn validate_change_set_preview_identity(
    connection: &Connection,
    change_set: &AgentAssetChangeSet,
    preview: &AgentAssetVersion,
) -> CoreResult<()> {
    let base = require_version(connection, &change_set.base_asset_version_id)?;
    if preview.status != AssetVersionStatus::Committed
        || preview.project_id != change_set.project_id
        || preview.parent_asset_version_id.as_deref()
            != Some(change_set.base_asset_version_id.as_str())
        || preview.version_no != base.version_no + 1
        || preview.plan_id != base.plan_id
        || preview.direction_id != base.direction_id
        || preview.domain_pack_id != base.domain_pack_id
    {
        return Err(CoreError::conflict(
            "CHANGE_SET_PREVIEW_INVALID",
            "Sealed preview must be the exact next child of the active base version.",
        ));
    }
    if version_from_connection(connection, &preview.asset_version_id)?.is_some() {
        return Err(CoreError::conflict(
            "CHANGE_SET_PREVIEW_VERSION_PERSISTED",
            "An ephemeral preview identity cannot already be a persisted asset version.",
        ));
    }
    Ok(())
}

fn validate_change_set_preview_readback(
    connection: &Connection,
    bundle: &ChangeSetPreviewBundleReadback,
    head: &str,
) -> CoreResult<()> {
    bundle.change_set.validate()?;
    bundle.sealed_preview.validate()?;
    validate_change_set_preview_identity(connection, &bundle.change_set, &bundle.sealed_preview)?;
    if bundle.change_set.status != ChangeSetStatus::Previewed
        || bundle.change_set.resulting_asset_version_id.is_some()
        || bundle.snapshot.active_design.asset_version_id()
            != Some(bundle.change_set.base_asset_version_id.as_str())
        || head != bundle.change_set.base_asset_version_id
        || bundle
            .snapshot
            .preview
            .as_ref()
            .map(|value| value.change_set_id.as_str())
            != Some(bundle.change_set.change_set_id.as_str())
        || bundle.interactive_preview_glb.extension != "glb"
        || bundle.interactive_preview_glb.ref_count < 1
    {
        return Err(CoreError::conflict(
            "CHANGE_SET_PREVIEW_BUNDLE_INVALID",
            "ChangeSet, Snapshot and interactive preview reference are inconsistent.",
        ));
    }
    let seal =
        parse_change_set_preview_seal(bundle.change_set.preview.as_ref().ok_or_else(|| {
            CoreError::conflict("CHANGE_SET_PREVIEW_INVALID", "Preview seal is missing.")
        })?)?;
    if seal.sealed_preview != bundle.sealed_preview
        || seal.interactive_readback != bundle.interactive_readback
        || seal.interactive_glb_sha256 != bundle.interactive_preview_glb.sha256
        || seal.interactive_glb_byte_size != bundle.interactive_preview_glb.byte_size
    {
        return Err(CoreError::conflict(
            "CHANGE_SET_PREVIEW_BUNDLE_INVALID",
            "Stored preview seal differs from its CAS object or readback.",
        ));
    }
    validate_interactive_readback_record(
        &bundle.interactive_readback,
        &bundle.sealed_preview,
        &bundle.interactive_preview_glb.sha256,
        bundle.interactive_preview_glb.byte_size,
    )
}

fn validate_change_set_confirm_readback(
    bundle: &ChangeSetConfirmBundleReadback,
    head: Option<&str>,
) -> CoreResult<()> {
    bundle.change_set.validate()?;
    bundle.version.validate()?;
    bundle.quality.validate()?;
    bundle.snapshot.validate()?;
    let seal =
        parse_change_set_confirm_seal(bundle.change_set.preview.as_ref().ok_or_else(|| {
            CoreError::conflict(
                "CHANGE_SET_CONFIRM_SEAL_INVALID",
                "Confirm seal is missing.",
            )
        })?)?;
    if bundle.change_set.status != ChangeSetStatus::Confirmed
        || bundle.change_set.resulting_asset_version_id.as_deref()
            != Some(bundle.version.asset_version_id.as_str())
        || bundle.version.status != AssetVersionStatus::Committed
        || bundle.version.parent_asset_version_id.as_deref()
            != Some(bundle.change_set.base_asset_version_id.as_str())
        || !same_preview_semantics(&seal.sealed_preview, &bundle.version)?
        || head != Some(bundle.version.asset_version_id.as_str())
        || bundle.snapshot.active_design.asset_version_id()
            != Some(bundle.version.asset_version_id.as_str())
        || bundle.snapshot.preview.is_some()
        || bundle
            .snapshot
            .quality
            .as_ref()
            .map(|value| value.quality_report_id.as_str())
            != Some(bundle.quality.quality_report_id.as_str())
        || bundle.production_glb.extension != "glb"
        || bundle.interactive_preview_glb.extension != "glb"
        || bundle.production_glb.ref_count < 1
        || bundle.interactive_preview_glb.ref_count < 1
        || seal.resulting_asset_version_id != bundle.version.asset_version_id
        || seal.quality_report_id != bundle.quality.quality_report_id
        || seal.production_glb_sha256 != bundle.production_glb.sha256
        || seal.production_glb_byte_size != bundle.production_glb.byte_size
        || seal.interactive_glb_sha256 != bundle.interactive_preview_glb.sha256
        || seal.interactive_glb_byte_size != bundle.interactive_preview_glb.byte_size
    {
        return Err(CoreError::conflict(
            "CHANGE_SET_CONFIRM_BUNDLE_INVALID",
            "Confirmed ChangeSet bundle identities are inconsistent.",
        ));
    }
    validate_bundle_quality(
        &bundle.quality,
        &bundle.version,
        &bundle.production_glb.sha256,
        bundle.production_glb.byte_size,
    )?;
    validate_interactive_readback_record(
        &seal.interactive_readback,
        &seal.sealed_preview,
        &bundle.interactive_preview_glb.sha256,
        bundle.interactive_preview_glb.byte_size,
    )
}

fn parse_change_set_preview_seal(value: &serde_json::Value) -> CoreResult<ChangeSetPreviewSeal> {
    let seal: ChangeSetPreviewSeal = serde_json::from_value(value.clone()).map_err(|_| {
        CoreError::conflict(
            "CHANGE_SET_PREVIEW_SEAL_INVALID",
            "Stored ChangeSet preview seal cannot be decoded.",
        )
    })?;
    if seal.schema_version != CHANGE_SET_PREVIEW_SEAL_SCHEMA {
        return Err(CoreError::conflict(
            "CHANGE_SET_PREVIEW_SEAL_INVALID",
            "Stored ChangeSet preview seal schema is unsupported.",
        ));
    }
    Ok(seal)
}

fn parse_change_set_confirm_seal(value: &serde_json::Value) -> CoreResult<ChangeSetConfirmSeal> {
    let seal: ChangeSetConfirmSeal = serde_json::from_value(value.clone()).map_err(|_| {
        CoreError::conflict(
            "CHANGE_SET_CONFIRM_SEAL_INVALID",
            "Stored ChangeSet confirm seal cannot be decoded.",
        )
    })?;
    if seal.schema_version != CHANGE_SET_CONFIRM_SEAL_SCHEMA {
        return Err(CoreError::conflict(
            "CHANGE_SET_CONFIRM_SEAL_INVALID",
            "Stored ChangeSet confirm seal schema is unsupported.",
        ));
    }
    Ok(seal)
}

fn validate_interactive_readback_record(
    readback: &serde_json::Value,
    preview: &AgentAssetVersion,
    glb_sha256: &str,
    glb_byte_size: u64,
) -> CoreResult<()> {
    let shape_sha = crate::semantic_sha256(&preview.shape_program)?;
    let bounds_valid = readback
        .get("bounds_mm")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| {
            values.len() == 3
                && values
                    .iter()
                    .all(|value| value.as_f64().is_some_and(f64::is_finite))
        });
    let failure_code = if !readback.is_object() {
        Some("CHANGE_SET_PREVIEW_READBACK_NOT_OBJECT")
    } else if readback
        .get("artifact_profile_id")
        .and_then(serde_json::Value::as_str)
        != Some("interactive_preview")
    {
        Some("CHANGE_SET_PREVIEW_READBACK_PROFILE_INVALID")
    } else if readback
        .get("shape_program_sha256")
        .and_then(serde_json::Value::as_str)
        != Some(shape_sha.as_str())
    {
        Some("CHANGE_SET_PREVIEW_READBACK_SHAPE_HASH_MISMATCH")
    } else if readback
        .get("glb_sha256")
        .and_then(serde_json::Value::as_str)
        != Some(glb_sha256)
    {
        Some("CHANGE_SET_PREVIEW_READBACK_GLB_HASH_MISMATCH")
    } else if readback
        .get("glb_byte_size")
        .and_then(serde_json::Value::as_u64)
        != Some(glb_byte_size)
    {
        Some("CHANGE_SET_PREVIEW_READBACK_GLB_SIZE_MISMATCH")
    } else if !readback
        .get("triangle_count")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|value| value > 0)
        || !readback
            .get("mesh_count")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|value| value > 0)
        || !readback
            .get("primitive_count")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|value| value > 0)
    {
        Some("CHANGE_SET_PREVIEW_READBACK_METRICS_INVALID")
    } else if readback
        .get("closed_manifold")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
        || readback
            .get("surface_provenance_present")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
    {
        Some("CHANGE_SET_PREVIEW_READBACK_TOPOLOGY_INVALID")
    } else if !bounds_valid {
        Some("CHANGE_SET_PREVIEW_READBACK_BOUNDS_INVALID")
    } else {
        None
    };
    if let Some(failure_code) = failure_code {
        return Err(CoreError::invalid_data(
            failure_code,
            "Interactive readback must bind the exact sealed ShapeProgram and GLB bytes.",
        ));
    }
    Ok(())
}

fn validate_interactive_readback(
    readback: &serde_json::Value,
    preview: &AgentAssetVersion,
    verified: &ForgeCadGlbReadback,
) -> CoreResult<()> {
    validate_interactive_readback_record(
        readback,
        preview,
        &verified.glb_sha256,
        verified.glb_byte_size,
    )?;
    let matches = readback
        .get("runtime_manifest_version")
        .and_then(serde_json::Value::as_str)
        == Some(verified.runtime_manifest_version.as_str())
        && readback
            .get("triangle_count")
            .and_then(serde_json::Value::as_u64)
            == Some(verified.triangle_count)
        && readback_bounds_match(readback.get("bounds_mm"), &verified.bounds_mm)
        && readback
            .get("mesh_count")
            .and_then(serde_json::Value::as_u64)
            == Some(verified.mesh_count)
        && readback
            .get("primitive_count")
            .and_then(serde_json::Value::as_u64)
            == Some(verified.primitive_count)
        && readback
            .get("material_count")
            .and_then(serde_json::Value::as_u64)
            == Some(verified.material_count)
        && readback
            .get("closed_manifold")
            .and_then(serde_json::Value::as_bool)
            == Some(verified.closed_manifold)
        && readback
            .get("surface_provenance_present")
            .and_then(serde_json::Value::as_bool)
            == Some(verified.surface_provenance_present);
    if !matches {
        return Err(CoreError::invalid_data(
            "CHANGE_SET_INTERACTIVE_READBACK_INVALID",
            "Interactive readback differs from Rust's canonical GLB readback.",
        ));
    }
    Ok(())
}

fn validate_production_quality_readback(
    quality: &QualityReport,
    version: &AgentAssetVersion,
    verified: &ForgeCadGlbReadback,
) -> CoreResult<()> {
    validate_bundle_quality(
        quality,
        version,
        &verified.glb_sha256,
        verified.glb_byte_size,
    )?;
    let compile = quality.report.get("compile_readback");
    let matches = compile
        .and_then(|value| value.get("runtime_manifest_version"))
        .and_then(serde_json::Value::as_str)
        == Some(verified.runtime_manifest_version.as_str())
        && compile
            .and_then(|value| value.get("triangle_count"))
            .and_then(serde_json::Value::as_u64)
            == Some(verified.triangle_count)
        && readback_bounds_match(
            compile.and_then(|value| value.get("bounds_mm")),
            &verified.bounds_mm,
        )
        && compile
            .and_then(|value| value.get("mesh_count"))
            .and_then(serde_json::Value::as_u64)
            == Some(verified.mesh_count)
        && compile
            .and_then(|value| value.get("primitive_count"))
            .and_then(serde_json::Value::as_u64)
            == Some(verified.primitive_count)
        && compile
            .and_then(|value| value.get("material_count"))
            .and_then(serde_json::Value::as_u64)
            == Some(verified.material_count)
        && compile
            .and_then(|value| value.get("closed_manifold"))
            .and_then(serde_json::Value::as_bool)
            == Some(verified.closed_manifold)
        && compile
            .and_then(|value| value.get("surface_provenance_present"))
            .and_then(serde_json::Value::as_bool)
            == Some(verified.surface_provenance_present);
    if !matches {
        return Err(CoreError::invalid_data(
            "CHANGE_SET_PRODUCTION_QUALITY_INVALID",
            "Production quality differs from Rust's canonical GLB readback.",
        ));
    }
    Ok(())
}

fn readback_bounds_match(value: Option<&serde_json::Value>, verified: &[f64]) -> bool {
    value
        .and_then(serde_json::Value::as_array)
        .is_some_and(|bounds| {
            bounds.len() == verified.len()
                && bounds.iter().zip(verified).all(|(left, right)| {
                    left.as_f64()
                        .is_some_and(|left| (left - right).abs() <= 0.01)
                })
        })
}

fn validate_change_set_preview_replay(
    existing: &ChangeSetPreviewBundleReadback,
    requested: &ChangeSetPreviewSeal,
    stored: &StoredObject,
    change_set_id: &str,
) -> CoreResult<()> {
    if existing.sealed_preview != requested.sealed_preview
        || existing.interactive_readback != requested.interactive_readback
        || existing.interactive_preview_glb.sha256 != stored.sha256
        || existing.interactive_preview_glb.byte_size != stored.byte_size
    {
        return Err(CoreError::conflict_with_details(
            "CHANGE_SET_PREVIEW_BUNDLE_IDEMPOTENCY_CONFLICT",
            "ChangeSet preview was already sealed with different content.",
            preview_bundle_ids(change_set_id),
        ));
    }
    Ok(())
}

fn validate_change_set_confirm_replay(
    existing: &ChangeSetConfirmBundleReadback,
    sealed_preview: &AgentAssetVersion,
    resulting: &AgentAssetVersion,
    quality: &QualityReport,
    production: &StoredObject,
    interactive: &StoredObject,
) -> CoreResult<()> {
    let seal = existing
        .change_set
        .preview
        .as_ref()
        .ok_or_else(|| {
            CoreError::conflict(
                "CHANGE_SET_CONFIRM_SEAL_INVALID",
                "Confirm seal is missing.",
            )
        })
        .and_then(parse_change_set_confirm_seal)?;
    if seal.sealed_preview != *sealed_preview
        || existing.version != *resulting
        || existing.quality != *quality
        || existing.production_glb.sha256 != production.sha256
        || existing.production_glb.byte_size != production.byte_size
        || existing.interactive_preview_glb.sha256 != interactive.sha256
        || existing.interactive_preview_glb.byte_size != interactive.byte_size
    {
        return Err(CoreError::conflict_with_details(
            "CHANGE_SET_CONFIRM_BUNDLE_IDEMPOTENCY_CONFLICT",
            "ChangeSet confirmation was already committed with different sealed content.",
            confirm_bundle_ids(
                &existing.change_set.change_set_id,
                &resulting.asset_version_id,
                &quality.quality_report_id,
            ),
        ));
    }
    Ok(())
}

fn preview_bundle_ids(change_set_id: &str) -> serde_json::Value {
    serde_json::json!({"change_set_id": change_set_id})
}

fn confirm_bundle_ids(
    change_set_id: &str,
    resulting_asset_version_id: &str,
    quality_report_id: &str,
) -> serde_json::Value {
    serde_json::json!({
        "change_set_id": change_set_id,
        "resulting_asset_version_id": resulting_asset_version_id,
        "quality_report_id": quality_report_id,
    })
}

fn change_set_bundle_incomplete(
    code: &'static str,
    mut details: serde_json::Value,
    missing: Vec<&str>,
) -> CoreError {
    let cause_code = details
        .get("cause_code")
        .and_then(serde_json::Value::as_str);
    let code = match (missing.as_slice(), cause_code) {
        (["interactive_preview_readback"], _) => "CHANGE_SET_PREVIEW_GLB_READBACK_INVALID",
        (["interactive_readback_metadata"], _) => "CHANGE_SET_PREVIEW_READBACK_METADATA_INVALID",
        (["transaction_readback"], _) => "CHANGE_SET_PREVIEW_TRANSACTION_READBACK_INVALID",
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_BUNDLE_INVALID")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_BUNDLE_INVALID"
        }
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_INVALID")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_IDENTITY_INVALID"
        }
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_VERSION_PERSISTED")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_VERSION_PERSISTED"
        }
        (["semantic_consistency"], Some("CHANGE_SET_INTERACTIVE_READBACK_INVALID")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_READBACK_INVALID"
        }
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_READBACK_RECORD_INVALID")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_READBACK_RECORD_INVALID"
        }
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_READBACK_SHAPE_HASH_MISMATCH")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_SHAPE_HASH_MISMATCH"
        }
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_READBACK_GLB_HASH_MISMATCH")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_GLB_HASH_MISMATCH"
        }
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_READBACK_GLB_SIZE_MISMATCH")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_GLB_SIZE_MISMATCH"
        }
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_READBACK_METRICS_INVALID")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_METRICS_INVALID"
        }
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_READBACK_TOPOLOGY_INVALID")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_TOPOLOGY_INVALID"
        }
        (["semantic_consistency"], Some("CHANGE_SET_PREVIEW_READBACK_BOUNDS_INVALID")) => {
            "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_BOUNDS_INVALID"
        }
        (["semantic_consistency"], _) => "CHANGE_SET_PREVIEW_SEMANTIC_CONSISTENCY_INVALID",
        (_, _) => code,
    };
    if let Some(object) = details.as_object_mut() {
        object.insert("missing".into(), serde_json::json!(missing));
    }
    CoreError::conflict_with_details(
        code,
        "Existing ChangeSet state is incomplete and cannot be treated as a successful bundle.",
        details,
    )
}

fn same_preview_semantics(
    preview: &AgentAssetVersion,
    resulting: &AgentAssetVersion,
) -> CoreResult<bool> {
    Ok(preview.project_id == resulting.project_id
        && preview.summary == resulting.summary
        && preview.stage == resulting.stage
        && preview.plan_id == resulting.plan_id
        && preview.direction_id == resulting.direction_id
        && preview.domain_pack_id == resulting.domain_pack_id
        && preview.artifact_id == resulting.artifact_id
        && preview.parts == resulting.parts
        && preview.shape_program == resulting.shape_program
        && preview.assembly_graph == resulting.assembly_graph
        && preview.material_bindings == resulting.material_bindings
        && crate::semantic_sha256(&preview.shape_program)?
            == crate::semantic_sha256(&resulting.shape_program)?)
}

fn copy_asset_version_object_references(
    transaction: &Transaction<'_>,
    source_asset_version_id: &str,
    resulting_asset_version_id: &str,
    created_at: &str,
) -> CoreResult<()> {
    let references = {
        let mut statement = transaction.prepare(
            "SELECT role, sha256 FROM forgecad_core_object_references WHERE reference_kind='asset_version' AND owner_id=? ORDER BY role",
        )?;
        let rows = statement
            .query_map([source_asset_version_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    if references.is_empty() {
        return Err(CoreError::conflict(
            "NAVIGATION_ARTIFACT_MISSING",
            "Navigation target has no Rust-owned content artifact to bind to the new immutable version.",
        ));
    }
    let has_production = references.iter().any(|(role, _)| role == "production_glb");
    let has_interactive = references
        .iter()
        .any(|(role, _)| role == "interactive_preview_glb");
    let has_external = references
        .iter()
        .any(|(role, _)| role == "external_reference_glb");
    if !(has_external || (has_production && has_interactive)) {
        return Err(CoreError::conflict(
            "NAVIGATION_ARTIFACT_INCOMPLETE",
            "Navigation target must have both interactive and production GLBs, or one explicit external-reference GLB.",
        ));
    }
    for (role, sha256) in references {
        transaction.execute(
            "INSERT INTO forgecad_core_object_references(reference_kind, owner_id, role, sha256, created_at) VALUES ('asset_version', ?, ?, ?, ?)",
            params![resulting_asset_version_id, role, sha256, created_at],
        )?;
    }
    Ok(())
}

fn clone_navigation_quality(
    transaction: &Transaction<'_>,
    source_asset_version_id: &str,
    resulting_asset_version_id: &str,
    project_id: &str,
    created_at: &str,
) -> CoreResult<Option<String>> {
    let source: Option<(String, String)> = transaction
        .query_row(
            "SELECT report_json, status FROM agent_asset_quality_reports WHERE asset_version_id=? AND status='passed' ORDER BY created_at DESC, quality_report_id DESC LIMIT 1",
            [source_asset_version_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((report_json, status)) = source else {
        return Ok(None);
    };
    let mut report = parse_json::<serde_json::Value>(report_json)?;
    let report_object = report.as_object_mut().ok_or_else(|| {
        CoreError::invalid_data(
            "QUALITY_REPORT_INVALID",
            "Historical quality report is not a JSON object.",
        )
    })?;
    let identity = crate::semantic_sha256(&serde_json::json!({
        "source_asset_version_id": source_asset_version_id,
        "resulting_asset_version_id": resulting_asset_version_id,
        "purpose": "immutable_navigation_quality"
    }))?;
    let quality_report_id = format!("quality_nav_{}", &identity[..24]);
    report_object.insert(
        "quality_report_id".into(),
        serde_json::Value::String(quality_report_id.clone()),
    );
    report_object.insert(
        "asset_version_id".into(),
        serde_json::Value::String(resulting_asset_version_id.to_string()),
    );
    report_object.insert(
        "checked_at".into(),
        serde_json::Value::String(created_at.to_string()),
    );
    let quality_status = QualityStatus::from_str(&status)?;
    let cloned = QualityReport {
        quality_report_id: quality_report_id.clone(),
        project_id: project_id.to_string(),
        asset_version_id: resulting_asset_version_id.to_string(),
        report,
        status: quality_status,
        created_at: created_at.to_string(),
    };
    cloned.validate()?;
    transaction.execute(
        "INSERT INTO agent_asset_quality_reports(quality_report_id, project_id, asset_version_id, report_json, status, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        params![
            cloned.quality_report_id,
            cloned.project_id,
            cloned.asset_version_id,
            json_text(&cloned.report)?,
            cloned.status.as_str(),
            cloned.created_at,
        ],
    )?;
    Ok(Some(quality_report_id))
}

fn mark_change_set_stale(transaction: &Transaction<'_>, id: &str, at: &str) -> CoreResult<()> {
    transaction.execute("UPDATE agent_asset_change_sets SET status='stale', preview_json=NULL, resulting_asset_version_id=NULL, updated_at=? WHERE change_set_id=?", params![at, id])?;
    transaction.execute(
        "DELETE FROM forgecad_core_object_references WHERE reference_kind='preview' AND owner_id=? AND role='interactive_preview_glb'",
        [id],
    )?;
    Ok(())
}

fn candidate_bundle_from_connection(
    connection: &Connection,
    artifact_id: &str,
    asset_version_id: &str,
    quality_report_id: &str,
) -> CoreResult<Option<CandidateBundleReadback>> {
    let candidate_project: Option<Option<String>> = connection
        .query_row(
            "SELECT project_id FROM agent_blockout_candidates WHERE artifact_id=?",
            [artifact_id],
            |row| row.get(0),
        )
        .optional()?;
    let candidate_object_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM forgecad_core_candidate_objects WHERE artifact_id=?)",
        [artifact_id],
        |row| row.get(0),
    )?;
    let candidate = candidate_from_connection(connection, artifact_id)?;
    let version = version_from_connection(connection, asset_version_id)?;
    let quality = quality_from_connection(connection, quality_report_id)?;
    let production_glb = object_for_reference_from_connection(
        connection,
        "asset_version",
        asset_version_id,
        "production_glb",
    )?;
    let interactive_preview_glb = object_for_reference_from_connection(
        connection,
        "asset_version",
        asset_version_id,
        "interactive_preview_glb",
    )?;
    let any = candidate_project.is_some()
        || candidate_object_exists
        || version.is_some()
        || quality.is_some()
        || production_glb.is_some()
        || interactive_preview_glb.is_some();
    if !any {
        return Ok(None);
    }

    let project_id = version
        .as_ref()
        .map(|value| value.project_id.as_str())
        .or_else(|| quality.as_ref().map(|value| value.project_id.as_str()))
        .or_else(|| candidate_project.as_ref().and_then(Option::as_deref));
    let snapshot = project_id
        .map(|value| snapshot_from_connection(connection, value))
        .transpose()?
        .flatten();
    let head = project_id
        .map(|value| {
            connection
                .query_row(
                    "SELECT asset_version_id FROM agent_asset_heads WHERE project_id=?",
                    [value],
                    |row| row.get::<_, String>(0),
                )
                .optional()
        })
        .transpose()?
        .flatten();

    let mut missing = Vec::new();
    if candidate_project.is_none() {
        missing.push("candidate");
    }
    if !candidate_object_exists {
        missing.push("candidate_object");
    }
    if version.is_none() {
        missing.push("version");
    }
    if quality.is_none() {
        missing.push("quality");
    }
    if production_glb.is_none() {
        missing.push("production_glb");
    }
    if interactive_preview_glb.is_none() {
        missing.push("interactive_preview_glb");
    }
    if snapshot.is_none() {
        missing.push("snapshot");
    }
    if head.is_none() {
        missing.push("head");
    }
    if !missing.is_empty() {
        return Err(candidate_bundle_incomplete(
            serde_json::json!({
                "artifact_id": artifact_id,
                "asset_version_id": asset_version_id,
                "quality_report_id": quality_report_id,
            }),
            missing,
        ));
    }

    let bundle = CandidateBundleReadback {
        candidate: candidate.ok_or_else(|| {
            candidate_bundle_incomplete(
                serde_json::json!({"artifact_id": artifact_id}),
                vec!["candidate_readback"],
            )
        })?,
        version: version.ok_or_else(|| CoreError::not_found("Agent asset version"))?,
        snapshot: snapshot.ok_or_else(|| CoreError::not_found("ActiveDesignSnapshot"))?,
        quality: quality.ok_or_else(|| CoreError::not_found("Agent asset quality report"))?,
        production_glb: production_glb.ok_or_else(|| CoreError::not_found("production GLB"))?,
        interactive_preview_glb: interactive_preview_glb
            .ok_or_else(|| CoreError::not_found("interactive preview GLB"))?,
    };
    if let Err(error) = validate_candidate_bundle_readback(&bundle, head.as_deref()) {
        let mut details = bundle_ids(&bundle);
        if let Some(object) = details.as_object_mut() {
            object.insert(
                "cause_code".to_string(),
                serde_json::Value::String(error.code().to_string()),
            );
        }
        return Err(candidate_bundle_incomplete(
            details,
            vec!["semantic_consistency"],
        ));
    }
    Ok(Some(bundle))
}

fn object_for_reference_from_connection(
    connection: &Connection,
    reference_kind: &str,
    owner_id: &str,
    role: &str,
) -> CoreResult<Option<ObjectRecord>> {
    connection
        .query_row(
            "SELECT o.sha256, o.object_path, o.extension, o.byte_size, o.ref_count, o.created_at, o.updated_at FROM forgecad_core_object_references r JOIN forgecad_core_objects o ON o.sha256=r.sha256 WHERE r.reference_kind=? AND r.owner_id=? AND r.role=?",
            params![reference_kind, owner_id, role],
            object_record_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn validate_candidate_bundle_input(
    candidate: &BlockoutCandidate,
    version: &AgentAssetVersion,
    quality: &QualityReport,
    production_glb: &StoredObject,
    interactive_preview_glb: &StoredObject,
) -> CoreResult<()> {
    candidate.validate()?;
    version.validate()?;
    quality.validate()?;
    if candidate.status != CandidateStatus::Candidate
        || version.status != AssetVersionStatus::Committed
        || version.version_no != 1
        || version.parent_asset_version_id.is_some()
    {
        return Err(CoreError::invalid_data(
            "CANDIDATE_BUNDLE_STATE_INVALID",
            "Bundle commit requires a candidate and immutable committed version 1.",
        ));
    }
    validate_candidate_version_identity(candidate, version)?;
    if production_glb.extension != "glb"
        || interactive_preview_glb.extension != "glb"
        || production_glb.byte_size == 0
        || interactive_preview_glb.byte_size == 0
        || candidate.glb_sha256 != production_glb.sha256
    {
        return Err(CoreError::invalid_data(
            "CANDIDATE_BUNDLE_OBJECT_INVALID",
            "Candidate bundle requires non-empty production and interactive GLB objects.",
        ));
    }
    validate_bundle_quality(
        quality,
        version,
        &production_glb.sha256,
        production_glb.byte_size,
    )
}

fn validate_candidate_bundle_readback(
    bundle: &CandidateBundleReadback,
    head: Option<&str>,
) -> CoreResult<()> {
    bundle.candidate.validate()?;
    bundle.version.validate()?;
    bundle.quality.validate()?;
    bundle.snapshot.validate()?;
    if bundle.candidate.status != CandidateStatus::Committed
        || bundle.version.status != AssetVersionStatus::Committed
        || bundle.version.version_no != 1
        || bundle.version.parent_asset_version_id.is_some()
        || head != Some(bundle.version.asset_version_id.as_str())
    {
        return Err(CoreError::conflict(
            "CANDIDATE_BUNDLE_STATE_INVALID",
            "Candidate bundle is not the committed authoritative initial chain.",
        ));
    }
    validate_candidate_version_identity(&bundle.candidate, &bundle.version)?;
    if bundle.candidate.glb_sha256 != bundle.production_glb.sha256
        || bundle.production_glb.extension != "glb"
        || bundle.interactive_preview_glb.extension != "glb"
        || bundle.production_glb.byte_size == 0
        || bundle.interactive_preview_glb.byte_size == 0
    {
        return Err(CoreError::conflict(
            "CANDIDATE_BUNDLE_OBJECT_INVALID",
            "Candidate bundle object roles do not match their sealed identities.",
        ));
    }
    let expected_ref_count =
        if bundle.production_glb.sha256 == bundle.interactive_preview_glb.sha256 {
            2
        } else {
            1
        };
    if bundle.production_glb.ref_count < expected_ref_count
        || bundle.interactive_preview_glb.ref_count < expected_ref_count
    {
        return Err(CoreError::conflict(
            "CANDIDATE_BUNDLE_OBJECT_REFERENCE_INVALID",
            "Candidate bundle object reference counts do not include both required roles.",
        ));
    }
    let snapshot_quality = bundle.snapshot.quality.as_ref();
    if bundle.snapshot.active_design.asset_version_id()
        != Some(bundle.version.asset_version_id.as_str())
        || bundle.snapshot.project_id != bundle.version.project_id
        || bundle.snapshot.revision < 2
        || snapshot_quality.map(|value| value.quality_report_id.as_str())
            != Some(bundle.quality.quality_report_id.as_str())
        || snapshot_quality.map(|value| value.asset_version_id.as_str())
            != Some(bundle.version.asset_version_id.as_str())
    {
        return Err(CoreError::conflict(
            "CANDIDATE_BUNDLE_SNAPSHOT_INVALID",
            "Candidate bundle Snapshot, version and quality identities are inconsistent.",
        ));
    }
    validate_bundle_quality(
        &bundle.quality,
        &bundle.version,
        &bundle.production_glb.sha256,
        bundle.production_glb.byte_size,
    )
}

fn validate_candidate_version_identity(
    candidate: &BlockoutCandidate,
    version: &AgentAssetVersion,
) -> CoreResult<()> {
    if candidate.project_id.as_deref() != Some(version.project_id.as_str())
        || candidate.artifact_id != version.artifact_id
        || candidate.plan_id != version.plan_id
        || candidate.direction_id != version.direction_id
        || candidate.domain_pack_id != version.domain_pack_id
        || candidate.shape_program != version.shape_program
        || candidate.assembly_graph != version.assembly_graph
        || candidate.material_bindings != version.material_bindings
    {
        return Err(CoreError::conflict(
            "CANDIDATE_BUNDLE_IDENTITY_DRIFT",
            "Version 1 must preserve the selected candidate's sealed geometry identity.",
        ));
    }
    Ok(())
}

fn validate_bundle_quality(
    quality: &QualityReport,
    version: &AgentAssetVersion,
    production_sha256: &str,
    production_byte_size: u64,
) -> CoreResult<()> {
    let compile = quality.report.get("compile_readback");
    let triangle_count = compile
        .and_then(|value| value.get("triangle_count"))
        .and_then(serde_json::Value::as_u64);
    let valid = quality.status == QualityStatus::Passed
        && quality.project_id == version.project_id
        && quality.asset_version_id == version.asset_version_id
        && quality
            .report
            .get("schema_version")
            .and_then(serde_json::Value::as_str)
            == Some("AgentAssetQualityReport@1")
        && quality
            .report
            .get("quality_report_id")
            .and_then(serde_json::Value::as_str)
            == Some(quality.quality_report_id.as_str())
        && quality
            .report
            .get("asset_version_id")
            .and_then(serde_json::Value::as_str)
            == Some(version.asset_version_id.as_str())
        && quality
            .report
            .get("status")
            .and_then(serde_json::Value::as_str)
            == Some("passed")
        && quality
            .report
            .get("evidence_source")
            .and_then(serde_json::Value::as_str)
            == Some("geometry_compile_readback")
        && compile
            .and_then(|value| value.get("schema_version"))
            .and_then(serde_json::Value::as_str)
            == Some("GeometryCompileReadback@2")
        && compile
            .and_then(|value| value.get("artifact_profile"))
            .and_then(|value| value.get("artifact_profile_id"))
            .and_then(serde_json::Value::as_str)
            == Some("production_concept")
        && compile
            .and_then(|value| value.get("shape_program_sha256"))
            .and_then(serde_json::Value::as_str)
            == Some(crate::semantic_sha256(&version.shape_program)?.as_str())
        && compile
            .and_then(|value| value.get("glb_sha256"))
            .and_then(serde_json::Value::as_str)
            == Some(production_sha256)
        && compile
            .and_then(|value| value.get("glb_byte_size"))
            .and_then(serde_json::Value::as_u64)
            == Some(production_byte_size)
        && triangle_count.is_some_and(|value| value > 0)
        && quality
            .report
            .get("triangle_count")
            .and_then(serde_json::Value::as_u64)
            == triangle_count
        && compile
            .and_then(|value| value.get("closed_manifold"))
            .and_then(serde_json::Value::as_bool)
            == Some(true)
        && compile
            .and_then(|value| value.get("surface_provenance_present"))
            .and_then(serde_json::Value::as_bool)
            == Some(true);
    if !valid {
        return Err(CoreError::invalid_data(
            "CANDIDATE_BUNDLE_QUALITY_INVALID",
            "Candidate bundle quality must be passed GeometryCompileReadback@2 for the exact production GLB and ShapeProgram.",
        ));
    }
    Ok(())
}

fn validate_candidate_bundle_replay(
    existing: &CandidateBundleReadback,
    candidate: &BlockoutCandidate,
    version: &AgentAssetVersion,
    quality: &QualityReport,
    production_glb: &StoredObject,
    interactive_preview_glb: &StoredObject,
) -> CoreResult<()> {
    let stored_candidate = &existing.candidate;
    let candidate_matches = stored_candidate.artifact_id == candidate.artifact_id
        && stored_candidate.project_id == candidate.project_id
        && stored_candidate.plan_id == candidate.plan_id
        && stored_candidate.direction_id == candidate.direction_id
        && stored_candidate.domain_pack_id == candidate.domain_pack_id
        && stored_candidate.candidate == candidate.candidate
        && stored_candidate.shape_program == candidate.shape_program
        && stored_candidate.assembly_graph == candidate.assembly_graph
        && stored_candidate.material_bindings == candidate.material_bindings
        && stored_candidate.glb_sha256 == candidate.glb_sha256
        && stored_candidate.created_at == candidate.created_at;
    let objects_match = existing.production_glb.sha256 == production_glb.sha256
        && existing.production_glb.byte_size == production_glb.byte_size
        && existing.production_glb.object_path == production_glb.relative_path
        && existing.interactive_preview_glb.sha256 == interactive_preview_glb.sha256
        && existing.interactive_preview_glb.byte_size == interactive_preview_glb.byte_size
        && existing.interactive_preview_glb.object_path == interactive_preview_glb.relative_path;
    if !candidate_matches
        || existing.version != *version
        || existing.quality != *quality
        || !objects_match
    {
        return Err(CoreError::conflict_with_details(
            "CANDIDATE_BUNDLE_IDEMPOTENCY_CONFLICT",
            "Candidate bundle identity was already used for a different sealed request.",
            bundle_input_ids(candidate, version, quality),
        ));
    }
    Ok(())
}

fn validate_glb_container(bytes: &[u8]) -> CoreResult<()> {
    if bytes.len() < 12
        || &bytes[..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap_or_default()) != 2
        || u32::from_le_bytes(bytes[8..12].try_into().unwrap_or_default()) as usize != bytes.len()
    {
        return Err(CoreError::invalid_data(
            "CANDIDATE_BUNDLE_GLB_INVALID",
            "Candidate bundle artifact is not a complete GLB 2.0 container.",
        ));
    }
    Ok(())
}

fn stored_from_record(record: &ObjectRecord) -> StoredObject {
    StoredObject {
        sha256: record.sha256.clone(),
        relative_path: record.object_path.clone(),
        extension: record.extension.clone(),
        byte_size: record.byte_size,
    }
}

fn bundle_input_ids(
    candidate: &BlockoutCandidate,
    version: &AgentAssetVersion,
    quality: &QualityReport,
) -> serde_json::Value {
    serde_json::json!({
        "artifact_id": candidate.artifact_id,
        "asset_version_id": version.asset_version_id,
        "quality_report_id": quality.quality_report_id,
    })
}

fn bundle_ids(bundle: &CandidateBundleReadback) -> serde_json::Value {
    bundle_input_ids(&bundle.candidate, &bundle.version, &bundle.quality)
}

fn candidate_bundle_incomplete(mut details: serde_json::Value, missing: Vec<&str>) -> CoreError {
    if let Some(object) = details.as_object_mut() {
        object.insert("missing".to_string(), serde_json::json!(missing));
    }
    CoreError::conflict_with_details(
        "CANDIDATE_BUNDLE_INCOMPLETE",
        "Existing candidate state is incomplete and cannot be treated as a successful bundle commit.",
        details,
    )
}

fn candidate_from_connection(
    connection: &Connection,
    artifact_id: &str,
) -> CoreResult<Option<BlockoutCandidate>> {
    connection.query_row(
        "SELECT c.artifact_id, c.project_id, c.plan_id, c.direction_id, c.domain_pack_id, c.status, c.candidate_json, c.shape_program_json, c.assembly_graph_json, c.material_bindings_json, o.glb_sha256, c.created_at, c.updated_at FROM agent_blockout_candidates c JOIN forgecad_core_candidate_objects o ON o.artifact_id=c.artifact_id WHERE c.artifact_id=?",
        [artifact_id],
        |row| {
            let status: String = row.get(5)?;
            Ok(BlockoutCandidate {
                artifact_id: row.get(0)?,
                project_id: row.get(1)?,
                plan_id: row.get(2)?,
                direction_id: row.get(3)?,
                domain_pack_id: row.get(4)?,
                status: CandidateStatus::from_str(&status).map_err(to_sql_error)?,
                candidate: parse_json(row.get::<_, String>(6)?).map_err(to_sql_error)?,
                shape_program: parse_json(row.get::<_, String>(7)?).map_err(to_sql_error)?,
                assembly_graph: parse_json(row.get::<_, String>(8)?).map_err(to_sql_error)?,
                material_bindings: parse_json(row.get::<_, String>(9)?).map_err(to_sql_error)?,
                glb_sha256: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        },
    ).optional().map_err(Into::into)
}

fn require_candidate(connection: &Connection, artifact_id: &str) -> CoreResult<BlockoutCandidate> {
    candidate_from_connection(connection, artifact_id)?
        .ok_or_else(|| CoreError::not_found("blockout candidate"))
}

fn insert_object_metadata(
    transaction: &Transaction<'_>,
    stored: &StoredObject,
    timestamp: &str,
) -> CoreResult<()> {
    transaction.execute(
        "INSERT OR IGNORE INTO forgecad_core_objects(sha256, object_path, extension, byte_size, ref_count, created_at, updated_at) VALUES (?, ?, ?, ?, 0, ?, ?)",
        params![stored.sha256, stored.relative_path, stored.extension, stored.byte_size, timestamp, timestamp],
    )?;
    let valid: bool = transaction.query_row(
        "SELECT object_path=? AND extension=? AND byte_size=? FROM forgecad_core_objects WHERE sha256=?",
        params![stored.relative_path, stored.extension, stored.byte_size, stored.sha256],
        |row| row.get(0),
    )?;
    if !valid {
        return Err(CoreError::conflict(
            "CONTENT_OBJECT_IDENTITY_CONFLICT",
            "Persisted object metadata conflicts with its SHA-256 identity.",
        ));
    }
    Ok(())
}

fn object_record(connection: &Connection, sha: &str) -> CoreResult<ObjectRecord> {
    object_record_optional(connection, sha)?.ok_or_else(|| CoreError::not_found("content object"))
}

fn object_record_optional(connection: &Connection, sha: &str) -> CoreResult<Option<ObjectRecord>> {
    connection.query_row(
        "SELECT sha256, object_path, extension, byte_size, ref_count, created_at, updated_at FROM forgecad_core_objects WHERE sha256=?",
        [sha], object_record_from_row,
    ).optional().map_err(Into::into)
}

fn object_record_from_row(row: &Row<'_>) -> rusqlite::Result<ObjectRecord> {
    Ok(ObjectRecord {
        sha256: row.get(0)?,
        object_path: row.get(1)?,
        extension: row.get(2)?,
        byte_size: row.get(3)?,
        ref_count: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn skill_manifest_owner_id(skill_id: &str, version: u32) -> String {
    format!("skillref_{skill_id}_v{version}")
}

fn skill_manifest_from_connection(
    connection: &Connection,
    skill_id: &str,
    version: u32,
) -> CoreResult<Option<(AgentSkillManifest, String, String, String)>> {
    connection
        .query_row(
            "SELECT manifest_json, manifest_sha256, manifest_object_sha256, status FROM agent_skill_versions WHERE skill_id=? AND version=?",
            params![skill_id, version],
            |row| {
                let manifest_json: String = row.get(0)?;
                let manifest: AgentSkillManifest = serde_json::from_str(&manifest_json).map_err(|_| {
                    to_sql_error(CoreError::invalid_data(
                        "SKILL_MANIFEST_INVALID",
                        "Stored Skill manifest cannot be decoded.",
                    ))
                })?;
                manifest.validate().map_err(to_sql_error)?;
                let manifest_sha256: String = row.get(1)?;
                let manifest_object_sha256: String = row.get(2)?;
                if manifest.canonical_sha256().map_err(to_sql_error)? != manifest_sha256
                    || manifest_object_sha256 != manifest_sha256
                {
                    return Err(to_sql_error(CoreError::invalid_data(
                        "SKILL_MANIFEST_HASH_INVALID",
                        "Stored Skill manifest bytes no longer match their sealed hash.",
                    )));
                }
                Ok((manifest, manifest_sha256, manifest_object_sha256, row.get(3)?))
            },
        )
        .optional()
        .map_err(Into::into)
}

fn require_skill_manifest(
    connection: &Connection,
    skill_id: &str,
    version: u32,
) -> CoreResult<(AgentSkillManifest, String, String, String)> {
    skill_manifest_from_connection(connection, skill_id, version)?
        .ok_or_else(|| CoreError::not_found("Skill version"))
}

fn skill_eval_report_from_row(row: &Row<'_>) -> rusqlite::Result<AgentSkillEvalReport> {
    let status: String = row.get(4)?;
    let status = match status.as_str() {
        "passed" => SkillEvalStatus::Passed,
        "failed" => SkillEvalStatus::Failed,
        _ => {
            return Err(to_sql_error(CoreError::invalid_data(
                "SKILL_EVAL_REPORT_INVALID",
                "Stored Skill evaluation status is invalid.",
            )))
        }
    };
    let findings_json: String = row.get(5)?;
    let findings = serde_json::from_str(&findings_json).map_err(|_| {
        to_sql_error(CoreError::invalid_data(
            "SKILL_EVAL_REPORT_INVALID",
            "Stored Skill findings cannot be decoded.",
        ))
    })?;
    Ok(AgentSkillEvalReport {
        schema_version: "AgentSkillEvalReport@1".to_string(),
        report_id: row.get(0)?,
        skill_id: row.get(1)?,
        skill_version: row.get(2)?,
        skill_sha256: row.get(3)?,
        status,
        findings,
        evaluated_at: row.get(6)?,
    })
}

fn skill_activation_from_row(row: &Row<'_>) -> rusqlite::Result<AgentSkillActivation> {
    Ok(AgentSkillActivation {
        schema_version: "AgentSkillActivation@1".to_string(),
        activation_id: row.get(0)?,
        skill_id: row.get(1)?,
        skill_version: row.get(2)?,
        skill_sha256: row.get(3)?,
        enabled: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn require_changed(changed: usize) -> CoreResult<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(stale("ACTIVE_DESIGN_STALE"))
    }
}

fn stale(code: &'static str) -> CoreError {
    CoreError::conflict(
        code,
        "Expected revision or authoritative asset head is stale.",
    )
}

fn json_text<T: serde::Serialize + ?Sized>(value: &T) -> CoreResult<String> {
    serde_json::to_string(value).map_err(|_| {
        CoreError::invalid_data(
            "JSON_SERIALIZATION_FAILED",
            "Core state could not be serialized.",
        )
    })
}

fn ensure_canonical_shape_program(
    shape_program: &serde_json::Value,
    code: &'static str,
) -> CoreResult<()> {
    if crate::normalize_persisted_shape_program(shape_program)? != *shape_program {
        return Err(CoreError::conflict(
            code,
            "ShapeProgram must be normalized before Rust Core persistence.",
        ));
    }
    Ok(())
}

fn json_option<T: serde::Serialize>(value: &Option<T>) -> CoreResult<Option<String>> {
    value.as_ref().map(json_text).transpose()
}

fn parse_json<T: DeserializeOwned>(value: String) -> CoreResult<T> {
    serde_json::from_str(&value).map_err(|_| {
        CoreError::invalid_data("PERSISTED_JSON_INVALID", "Persisted core JSON is invalid.")
    })
}

fn parse_optional_json<T: DeserializeOwned>(value: Option<String>) -> CoreResult<Option<T>> {
    value.map(parse_json).transpose()
}

fn to_sql_error(_error: CoreError) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        process::Command,
        thread,
        time::{Duration, Instant},
    };

    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::{read_ownership_marker, MigrationRunner, ProjectStatus, QualityStatus, StateOwner};

    struct Fixture {
        _root: tempfile::TempDir,
        repository: CoreRepository,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempdir().unwrap();
            let db = root.path().join("forgecad.db");
            MigrationRunner::new(&db).run().unwrap();
            let connection = open_connection(&db).unwrap();
            connection.execute(
                "INSERT INTO domain_profiles(profile_id, domain_type, schema_version, pack_id, display_name, profile_json, profile_sha256, status, created_at, updated_at) VALUES ('profile_weapon', 'weapon_concept', 'DesignDomainProfile@1', 'pack_weapon', 'Weapon', '{}', ?, 'active', 't0', 't0')",
                ["0".repeat(64)],
            ).unwrap();
            let lease = WriterLease::acquire(
                &db,
                root.path(),
                "test-writer",
                StateOwner::PythonCompatibilityAdapter,
            )
            .unwrap();
            let store = ContentAddressedObjectStore::new(root.path()).unwrap();
            let repository = CoreRepository::new(lease, store).unwrap();
            Self {
                _root: root,
                repository,
            }
        }

        fn seed(&self) -> AgentAssetVersion {
            self.repository
                .insert_project(&Project {
                    project_id: "project_a".into(),
                    profile_id: "profile_weapon".into(),
                    domain_type: "weapon_concept".into(),
                    name: "Concept".into(),
                    status: ProjectStatus::Active,
                    current_version_id: None,
                    created_at: "t1".into(),
                    updated_at: "t1".into(),
                })
                .unwrap();
            let version = asset("asset_v1", None, 1, "shell-a", "t1");
            self.repository.commit_initial_asset(&version).unwrap();
            version
        }
    }

    fn seed_pending_deletion(
        db: &Path,
        library_root: &Path,
        bytes: &[u8],
        created_at: &str,
    ) -> (StoredObject, PathBuf) {
        let store = ContentAddressedObjectStore::new(library_root).unwrap();
        let mut promoted = store.stage(bytes, "glb").unwrap().promote().unwrap();
        let stored = promoted.metadata().clone();
        promoted.finalize_commit().unwrap();
        let object_path = library_root
            .join("objects/sha256")
            .join(&stored.relative_path);
        assert!(object_path.is_file());
        open_connection(db)
            .unwrap()
            .execute(
                "INSERT INTO forgecad_core_object_deletion_journal(sha256, object_path, extension, byte_size, created_at) VALUES (?, ?, ?, ?, ?)",
                params![
                    stored.sha256,
                    stored.relative_path,
                    stored.extension,
                    stored.byte_size,
                    created_at,
                ],
            )
            .unwrap();
        (stored, object_path)
    }

    fn pending_deletion_count(db: &Path) -> u64 {
        open_connection(db)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM forgecad_core_object_deletion_journal",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn pending_promotion_count(library_root: &Path) -> usize {
        std::fs::read_dir(library_root.join("objects/.pending"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .count()
    }

    fn asset(
        id: &str,
        parent: Option<&str>,
        no: u64,
        shell: &str,
        created_at: &str,
    ) -> AgentAssetVersion {
        AgentAssetVersion {
            asset_version_id: id.into(),
            project_id: "project_a".into(),
            parent_asset_version_id: parent.map(str::to_string),
            version_no: no,
            status: AssetVersionStatus::Committed,
            summary: shell.into(),
            stage: crate::AssetStage::EditableAsset,
            plan_id: "plan_a".into(),
            direction_id: "direction_best".into(),
            domain_pack_id: "pack_weapon".into(),
            artifact_id: format!("artifact_{id}"),
            parts: vec![json!({"part_id":"part_shell"})],
            shape_program: json!({"schema_version":"ShapeProgram@1","shell":shell}),
            assembly_graph: json!({"graph_id":format!("graph_{id}"),"parts":[{"part_id":"part_shell","material_zone_ids":["zone_shell"]}]}),
            material_bindings: BTreeMap::new(),
            created_at: created_at.into(),
        }
    }

    fn change_set() -> AgentAssetChangeSet {
        AgentAssetChangeSet {
            change_set_id: "change_a".into(),
            project_id: "project_a".into(),
            base_asset_version_id: "asset_v1".into(),
            summary: "refine shell".into(),
            operations: vec![json!({
                "operation_id":"op_transform_shell",
                "op":"set_part_transform",
                "part_id":"part_shell",
                "transform":{
                    "position":[0,0,0],
                    "rotation":[0,0,0],
                    "scale":[1,1,1]
                }
            })],
            protected_part_ids: vec![],
            preview: None,
            status: ChangeSetStatus::Proposed,
            resulting_asset_version_id: None,
            created_at: "t2".into(),
            updated_at: "t2".into(),
        }
    }

    #[test]
    fn c106_surface_adornment_joins_part_recipe_provenance_to_its_reviewed_slot() {
        let mut version = asset("asset_c106_surface", None, 1, "c106 shell", "t1");
        version.assembly_graph = json!({
            "graph_id": "graph_c106_surface",
            "parts": [{
                "part_id": "part_shell",
                "recipe_instance_id": "recipeinst_c106_link",
                "material_zone_ids": ["zone_c106_link_shell"],
                "surface_adornment_slots": [{
                    "slot_id": "adornslot_c106_link_shell",
                    "zone_id": "zone_c106_link_shell",
                    "allowed_kinds": ["flowline"],
                    "allowed_motifs": ["double_flowline"],
                    "allowed_coverages": ["center_band"]
                }]
            }],
            "component_recipe_instances": [{
                "instance_id": "recipeinst_c106_link",
                "recipe": {"recipe_id": "recipe_c106_arm_link_armor"}
            }]
        });
        let program = SurfaceAdornmentProgram {
            schema_version: "SurfaceAdornmentProgram@1".into(),
            program_id: "adorn_c106_link_flowline".into(),
            target_part_id: "part_shell".into(),
            target_zone_id: "zone_c106_link_shell".into(),
            kind: "flowline".into(),
            motif: "double_flowline".into(),
            intensity: "balanced".into(),
            coverage: "center_band".into(),
            seed: 106,
            base_material: "mat_aluminum".into(),
            execution: "texture_bake".into(),
            skill_id: "skill_surface_groove".into(),
            skill_version: 1,
            skill_sha256: "a".repeat(64),
            generator: "a005_v1".into(),
            non_functional_only: true,
        };
        let legacy_manifest = builtin_surface_adornment_manifest();
        assert_eq!(
            validate_surface_adornment_recipe_slot(
                &version,
                "part_shell",
                &program,
                &legacy_manifest,
            )
            .unwrap_err()
            .code(),
            "SURFACE_ADORNMENT_RECIPE_POLICY_DENIED"
        );

        // The historical activation keeps its original C105 permission; the
        // new double gate must not revoke what its sealed v1 explicitly named.
        version.assembly_graph["component_recipe_instances"][0]["recipe"]["recipe_id"] =
            json!("recipe_robotic_arm_link");
        validate_surface_adornment_recipe_slot(&version, "part_shell", &program, &legacy_manifest)
            .unwrap();
        version.assembly_graph["component_recipe_instances"][0]["recipe"]["recipe_id"] =
            json!("recipe_c106_arm_link_armor");
        let c106_manifest = builtin_surface_adornment_manifest_v2();
        validate_surface_adornment_recipe_slot(&version, "part_shell", &program, &c106_manifest)
            .unwrap();

        version.assembly_graph["parts"][0]["surface_adornment_slots"][0]["allowed_coverages"] =
            json!(["edge_band"]);
        assert_eq!(
            validate_surface_adornment_recipe_slot(
                &version,
                "part_shell",
                &program,
                &c106_manifest,
            )
            .unwrap_err()
            .code(),
            "SURFACE_ADORNMENT_RECIPE_SLOT_DENIED"
        );

        version.assembly_graph["component_recipe_instances"][0]["recipe"]["recipe_id"] =
            json!("recipe_unknown_mixed_surface");
        assert_eq!(
            validate_surface_adornment_recipe_slot(
                &version,
                "part_shell",
                &program,
                &c106_manifest,
            )
            .unwrap_err()
            .code(),
            "SURFACE_ADORNMENT_RECIPE_POLICY_DENIED"
        );
    }

    fn png_1x1() -> Vec<u8> {
        [
            "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c489",
            "0000000d49444154789c6360f8cfc000000301010018dd8db40000000049454e44ae426082",
        ]
        .concat()
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
    }

    fn material_texture_request() -> RegisterMaterialTextureRequest {
        RegisterMaterialTextureRequest {
            display_name: "M103 预览纹理".into(),
            texture_role: MaterialTextureRole::BaseColor,
            mime_type: "image/png".into(),
            payload_base64: BASE64_STANDARD.encode(png_1x1()),
            source: MaterialTextureSource::UserCreated,
            license: MaterialTextureLicense::SelfDeclaredOriginal,
            license_ref: None,
            thumbnail_asset_id: None,
        }
    }

    #[test]
    fn material_texture_registration_is_idempotent_cas_backed_and_snapshot_free() {
        let fixture = Fixture::new();
        fixture.seed();
        let snapshot_before = fixture.repository.snapshot("project_a").unwrap().unwrap();
        let request = material_texture_request();
        assert_eq!(
            request.request_hash().unwrap(),
            "33d3f6dc58f762ae42846e74e6cfb95ee136b5dcafc6c31d169af322458947a1"
        );
        let created = fixture
            .repository
            .register_material_texture(&request, "m103-register", "2026-07-17T00:00:00Z")
            .unwrap();
        assert_eq!(created.schema_version, "MaterialTextureObject@1");
        assert_eq!(created.width, 1);
        assert_eq!(created.height, 1);
        assert!(created.visual_only);
        assert!(created.object_exists);
        assert!(created.object_path.starts_with("objects/sha256/"));

        let replay = fixture
            .repository
            .register_material_texture(&request, "m103-register", "2026-07-17T00:00:01Z")
            .unwrap();
        assert_eq!(replay, created);

        let mut changed = request.clone();
        changed.display_name = "different request".into();
        assert_eq!(
            fixture
                .repository
                .register_material_texture(&changed, "m103-register", "2026-07-17T00:00:02Z",)
                .unwrap_err()
                .code(),
            "IDEMPOTENCY_CONFLICT"
        );
        assert_eq!(
            fixture
                .repository
                .register_material_texture(&changed, "m103-display-alias", "2026-07-17T00:00:03Z",)
                .unwrap(),
            created
        );
        changed.source = MaterialTextureSource::ImportedReference;
        changed.license = MaterialTextureLicense::Unknown;
        assert_eq!(
            fixture
                .repository
                .register_material_texture(
                    &changed,
                    "m103-metadata-conflict",
                    "2026-07-17T00:00:04Z",
                )
                .unwrap_err()
                .code(),
            "TEXTURE_METADATA_CONFLICT"
        );

        let listed = fixture
            .repository
            .list_material_textures(&MaterialTextureQuery {
                texture_role: Some(MaterialTextureRole::BaseColor),
                source: Some(MaterialTextureSource::UserCreated),
                query: Some("M103".into()),
                limit: 10,
            })
            .unwrap();
        assert_eq!(listed, vec![created.clone()]);
        assert_eq!(
            fixture.repository.snapshot("project_a").unwrap().unwrap(),
            snapshot_before
        );

        std::fs::remove_file(fixture._root.path().join(&created.object_path)).unwrap();
        assert!(
            !fixture
                .repository
                .material_texture(&created.texture_asset_id)
                .unwrap()
                .unwrap()
                .object_exists
        );
    }

    #[test]
    fn material_texture_rejects_unlicensed_and_path_like_payloads_before_writing() {
        let fixture = Fixture::new();
        let mut request = material_texture_request();
        request.source = MaterialTextureSource::ImportedReference;
        request.license = MaterialTextureLicense::ThirdParty;
        assert_eq!(
            fixture
                .repository
                .register_material_texture(&request, "m103-license", "2026-07-17T00:00:00Z",)
                .unwrap_err()
                .code(),
            "TEXTURE_PROVENANCE_INVALID"
        );
        request.source = MaterialTextureSource::UserCreated;
        request.license = MaterialTextureLicense::SelfDeclaredOriginal;
        request.payload_base64 = "not-a-path:/tmp/texture.png".into();
        assert_eq!(
            fixture
                .repository
                .register_material_texture(&request, "m103-path", "2026-07-17T00:00:00Z",)
                .unwrap_err()
                .code(),
            "TEXTURE_BASE64_INVALID"
        );
        let connection = open_connection(fixture.repository.db_path()).unwrap();
        let count: u64 = connection
            .query_row(
                "SELECT COUNT(*) FROM agent_material_texture_objects",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn material_texture_bootstrap_adopts_valid_historical_python_cas_object() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        MigrationRunner::new(&db).run().unwrap();
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let mut promoted = store.stage(&png_1x1(), "png").unwrap().promote().unwrap();
        let stored = promoted.metadata().clone();
        promoted.finalize_commit().unwrap();
        let texture_asset_id = format!("asset_tex_{}", &stored.sha256[..24]);
        let connection = open_connection(&db).unwrap();
        connection
            .execute(
                "INSERT INTO agent_material_texture_objects(texture_asset_id, texture_role, display_name, mime_type, byte_size, sha256, object_path, width, height, source, license, license_ref, thumbnail_asset_id, visual_only, created_at, updated_at) VALUES (?, 'base_color', 'Historical M103', 'image/png', ?, ?, ?, 1, 1, 'user_created', 'self_declared_original', NULL, NULL, 1, '2026-07-16T00:00:00Z', '2026-07-16T00:00:00Z')",
                params![
                    texture_asset_id,
                    stored.byte_size,
                    stored.sha256,
                    format!("objects/sha256/{}", stored.relative_path),
                ],
            )
            .unwrap();
        drop(connection);

        let repository = CoreRepository::open(&db, root.path(), "m103-adoption").unwrap();
        let adopted = repository
            .material_texture(&texture_asset_id)
            .unwrap()
            .unwrap();
        assert!(adopted.object_exists);
        let connection = open_connection(&db).unwrap();
        let references: u64 = connection
            .query_row(
                "SELECT COUNT(*) FROM forgecad_core_object_references WHERE reference_kind='texture' AND owner_id=? AND role='base_color' AND sha256=?",
                params![texture_asset_id, stored.sha256],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(references, 1);
    }

    #[test]
    fn stale_etag_rolls_back_without_partial_state() {
        let fixture = Fixture::new();
        fixture.seed();
        let before = fixture.repository.snapshot("project_a").unwrap().unwrap();
        let error = fixture
            .repository
            .select(
                "project_a",
                SnapshotEtag(99),
                Some("part_shell"),
                None,
                "t2",
            )
            .unwrap_err();
        assert_eq!(error.code(), "ACTIVE_DESIGN_STALE");
        assert_eq!(
            fixture.repository.snapshot("project_a").unwrap().unwrap(),
            before
        );
    }

    #[test]
    fn declared_editable_parameter_range_and_step_are_authoritative() {
        let mut version = asset("asset_binding", None, 1, "shell-a", "t1");
        version.parts = vec![json!({
            "part_id":"part_shell",
            "editable_parameter_bindings":[{
                "schema_version":"EditableParameterBinding@1",
                "parameter_id":"editparam_part_shell_scale_x",
                "path":"transform.scale.x",
                "display_name":"横向比例",
                "unit":"ratio",
                "default":1.0,
                "min":0.6,
                "max":1.4,
                "step":0.1
            }]
        })];
        let operation = |path: &str, value: f64| {
            json!({"path":path,"value":value})
                .as_object()
                .unwrap()
                .clone()
        };
        assert!(validate_parameter_operation(
            &version,
            "part_shell",
            &operation("transform.scale.x", 1.4)
        )
        .is_ok());
        assert_eq!(
            validate_parameter_operation(
                &version,
                "part_shell",
                &operation("transform.scale.x", 1.45)
            )
            .unwrap_err()
            .code(),
            "PARAMETER_OUT_OF_RANGE"
        );
        assert_eq!(
            validate_parameter_operation(
                &version,
                "part_shell",
                &operation("transform.scale.x", 0.65)
            )
            .unwrap_err()
            .code(),
            "PARAMETER_STEP_MISMATCH"
        );
        assert_eq!(
            validate_parameter_operation(
                &version,
                "part_shell",
                &operation("transform.position.x", 1.0)
            )
            .unwrap_err()
            .code(),
            "PARAMETER_NOT_DECLARED"
        );
    }

    #[test]
    fn rust_components_and_structure_suggestions_recompute_authoritative_facts() {
        let fixture = Fixture::new();
        fixture
            .repository
            .insert_project(&Project {
                project_id: "project_a".into(),
                profile_id: "profile_weapon".into(),
                domain_type: "weapon_concept".into(),
                name: "Component facts".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "t1".into(),
                updated_at: "t1".into(),
            })
            .unwrap();
        let version = AgentAssetVersion {
            asset_version_id: "asset_component_v1".into(),
            project_id: "project_a".into(),
            parent_asset_version_id: None,
            version_no: 1,
            status: AssetVersionStatus::Committed,
            summary: "bounded exterior component".into(),
            stage: crate::AssetStage::EditableAsset,
            plan_id: "plan_component".into(),
            direction_id: "direction_component".into(),
            domain_pack_id: "pack_weapon".into(),
            artifact_id: "artifact_component".into(),
            parts: vec![json!({
                "part_id":"part_shell",
                "role":"body_panel",
                "parent_part_id":null,
                "position_mm":[0.0,0.0,0.0],
                "size_mm":[100.0,40.0,50.0],
                "material_zone_ids":["zone_body_panel"],
                "editable_parameters":["transform.scale"],
                "editable_parameter_bindings":[],
                "locked":false,
                "provenance":"agent_generated"
            })],
            shape_program: json!({
                "schema_version":"ShapeProgram@1",
                "program_id":"shape_component",
                "units":"millimeter",
                "seed":7,
                "triangle_budget":100000,
                "parameters":[],
                "non_functional_only":true,
                "operations":[
                    {"operation_id":"op_shell_a","op":"box","inputs":[],"args":{"position":[-30.0,0.0,0.0],"size":[30.0,40.0,50.0],"part_role":"body_panel","zone_id":"zone_body_panel","material_id":"mat_primary"}},
                    {"operation_id":"op_shell_b","op":"box","inputs":[],"args":{"position":[30.0,0.0,0.0],"size":[30.0,40.0,50.0],"part_role":"body_panel","zone_id":"zone_body_panel","material_id":"mat_primary"}}
                ],
                "outputs":[
                    {"output_id":"output_shell_a","operation_id":"op_shell_a","kind":"mesh","part_role":"body_panel"},
                    {"output_id":"output_shell_b","operation_id":"op_shell_b","kind":"mesh","part_role":"body_panel"}
                ]
            }),
            assembly_graph: json!({
                "schema_version":"AssemblyGraph@1",
                "graph_id":"graph_component",
                "root_part_id":"part_shell",
                "parts":[{
                    "part_id":"part_shell","role":"body_panel","parent_part_id":null,
                    "geometry_source":"shape_program","operation_id":"op_shell_a","output_id":"output_shell_a",
                    "transform":{"position":[0.0,0.0,0.0],"rotation":[0.0,0.0,0.0],"scale":[1.0,1.0,1.0]},
                    "connectors":[],"joints":[],"material_zones":["zone_body_panel"],
                    "material_zone_ids":["zone_body_panel"],"locked":false
                }],
                "connections":[]
            }),
            material_bindings: BTreeMap::from([(
                "part_shell:zone_body_panel".into(),
                json!("mat_primary"),
            )]),
            created_at: "t1".into(),
        };
        let snapshot = fixture.repository.commit_initial_asset(&version).unwrap();
        let object = fixture
            .repository
            .attach_object_bytes(
                &ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: version.asset_version_id.clone(),
                    role: "production_glb".into(),
                },
                b"component-production-glb",
                "glb",
                "t2",
            )
            .unwrap();
        let quality = QualityReport {
            quality_report_id: "quality_component".into(),
            project_id: version.project_id.clone(),
            asset_version_id: version.asset_version_id.clone(),
            report: json!({
                "schema_version":"AgentAssetQualityReport@1",
                "quality_report_id":"quality_component",
                "asset_version_id":version.asset_version_id,
                "status":"passed",
                "evidence_source":"geometry_compile_readback",
                "triangle_count":12,
                "compile_readback":{
                    "schema_version":"GeometryCompileReadback@2",
                    "artifact_profile":{"artifact_profile_id":"production_concept"},
                    "shape_program_sha256":crate::semantic_sha256(&version.shape_program).unwrap(),
                    "glb_sha256":object.sha256,
                    "glb_byte_size":object.byte_size,
                    "triangle_count":12,
                    "closed_manifold":true,
                    "surface_provenance_present":true
                }
            }),
            status: QualityStatus::Passed,
            created_at: "t2".into(),
        };
        let quality_snapshot = fixture
            .repository
            .attach_quality(&quality, snapshot.etag())
            .unwrap();
        let component = fixture
            .repository
            .save_component_idempotent(
                &version.asset_version_id,
                "agentcomp_shell",
                "part_shell",
                "外壳替换件",
                "只读项目内快照",
                "t3",
                "POST component",
                "save_component",
                &"a".repeat(64),
            )
            .unwrap();
        assert_eq!(component.source_quality_status, QualityStatus::Passed);
        assert_eq!(component.shape_operation["operation_id"], "op_shell_a");
        assert_eq!(
            component.material_bindings["part_shell:zone_body_panel"],
            "mat_primary"
        );
        assert_eq!(
            fixture
                .repository
                .save_component_idempotent(
                    &version.asset_version_id,
                    "agentcomp_shell",
                    "part_shell",
                    "外壳替换件",
                    "只读项目内快照",
                    "t3",
                    "POST component",
                    "save_component",
                    &"a".repeat(64),
                )
                .unwrap(),
            component
        );
        let candidates = fixture
            .repository
            .component_candidates(&version.asset_version_id, "part_shell")
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].compatibility.eligible);
        assert!(candidates[0]
            .compatibility
            .reason_codes
            .contains(&"target_connectors_preserved".into()));

        let split = fixture
            .repository
            .structure_suggestions(&version.asset_version_id)
            .unwrap();
        assert_eq!(split.suggestions.len(), 1);
        assert_eq!(split.suggestions[0].kind, "split_part");
        let forged = AgentAssetChangeSet {
            change_set_id: "change_forged_structure".into(),
            project_id: version.project_id.clone(),
            base_asset_version_id: version.asset_version_id.clone(),
            summary: "forged split".into(),
            operations: vec![json!({
                "operation_id":"op_forged",
                "op":"split_part",
                "part_id":"part_shell",
                "structure_suggestion_id":"structure_split_part_forged"
            })],
            protected_part_ids: vec![],
            preview: None,
            status: ChangeSetStatus::Proposed,
            resulting_asset_version_id: None,
            created_at: "t4".into(),
            updated_at: "t4".into(),
        };
        assert_eq!(
            fixture
                .repository
                .create_change_set(&forged)
                .unwrap_err()
                .code(),
            "STRUCTURE_SUGGESTION_NOT_AVAILABLE"
        );
        let valid_split = AgentAssetChangeSet {
            change_set_id: "change_valid_structure".into(),
            operations: vec![json!({
                "operation_id":"op_valid",
                "op":"split_part",
                "part_id":"part_shell",
                "structure_suggestion_id":split.suggestions[0].suggestion_id
            })],
            summary: "valid split".into(),
            ..forged.clone()
        };
        fixture.repository.create_change_set(&valid_split).unwrap();

        let locked = fixture
            .repository
            .set_part_display_idempotent(
                "project_a",
                quality_snapshot.etag(),
                "lock",
                Some("part_shell"),
                "t5",
                "POST part-display lock",
                "lock_part",
                &"b".repeat(64),
            )
            .unwrap();
        assert!(fixture
            .repository
            .structure_suggestions(&version.asset_version_id)
            .unwrap()
            .suggestions
            .is_empty());
        let replace_while_locked = AgentAssetChangeSet {
            change_set_id: "change_locked_replace".into(),
            operations: vec![json!({
                "operation_id":"op_locked_replace",
                "op":"replace_part",
                "part_id":"part_shell",
                "replacement_component_id":"agentcomp_shell"
            })],
            summary: "locked replacement".into(),
            ..forged.clone()
        };
        assert_eq!(
            fixture
                .repository
                .create_change_set(&replace_while_locked)
                .unwrap_err()
                .code(),
            "PART_PROTECTED"
        );
        fixture
            .repository
            .set_part_display_idempotent(
                "project_a",
                locked.etag(),
                "unlock",
                Some("part_shell"),
                "t6",
                "POST part-display unlock",
                "unlock_part",
                &"c".repeat(64),
            )
            .unwrap();

        fixture
            .repository
            .write(|transaction| {
                transaction.execute(
                    "UPDATE agent_asset_quality_reports SET status='failed', report_json=json_set(report_json, '$.status', 'failed') WHERE quality_report_id='quality_component'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .component_candidates(&version.asset_version_id, "part_shell")
                .unwrap()[0]
                .compatibility
                .source_quality_status,
            QualityStatus::Failed
        );
        let replace_after_failure = AgentAssetChangeSet {
            change_set_id: "change_failed_quality_replace".into(),
            operations: vec![json!({
                "operation_id":"op_failed_quality_replace",
                "op":"replace_part",
                "part_id":"part_shell",
                "replacement_component_id":"agentcomp_shell"
            })],
            summary: "failed source".into(),
            ..forged.clone()
        };
        assert_eq!(
            fixture
                .repository
                .create_change_set(&replace_after_failure)
                .unwrap_err()
                .code(),
            "COMPONENT_QUALITY_NOT_READY"
        );

        fixture
            .repository
            .attach_object_bytes(
                &ObjectReference {
                    reference_kind: "asset_version".into(),
                    owner_id: version.asset_version_id.clone(),
                    role: "external_reference_glb".into(),
                },
                b"external-reference",
                "glb",
                "t7",
            )
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .structure_suggestions(&version.asset_version_id)
                .unwrap_err()
                .code(),
            "EXTERNAL_REFERENCE_NOT_EDITABLE"
        );
    }

    #[test]
    fn preview_confirm_and_immutable_undo_redo_are_atomic() {
        let fixture = Fixture::new();
        let original = fixture.seed();
        fixture.repository.create_change_set(&change_set()).unwrap();
        let preview = asset("preview_a", Some("asset_v1"), 2, "shell-b", "t2");
        let (_, snapshot) = fixture
            .repository
            .preview_change_set("change_a", &preview, SnapshotEtag(1), "t2")
            .unwrap();
        let mut competing_change = change_set();
        competing_change.change_set_id = "change_b".into();
        fixture
            .repository
            .create_change_set(&competing_change)
            .unwrap();
        let competing_preview = asset("preview_b", Some("asset_v1"), 2, "shell-c", "t2");
        let competing_error = fixture
            .repository
            .preview_change_set("change_b", &competing_preview, snapshot.etag(), "t2")
            .unwrap_err();
        assert_eq!(competing_error.code(), "ACTIVE_DESIGN_PREVIEW_PENDING");
        assert_eq!(
            fixture
                .repository
                .change_set("change_b")
                .unwrap()
                .unwrap()
                .status,
            ChangeSetStatus::Proposed
        );

        let mut confirmed = preview.clone();
        confirmed.asset_version_id = "asset_v2".into();
        confirmed.created_at = "t3".into();
        let mut drifted = confirmed.clone();
        drifted.shape_program = json!({"schema_version":"ShapeProgram@1","shell":"shell-drift"});
        let drift_error = fixture
            .repository
            .confirm_change_set("change_a", &drifted, snapshot.etag())
            .unwrap_err();
        assert_eq!(drift_error.code(), "CHANGE_SET_PREVIEW_DRIFT");
        assert!(fixture.repository.version("asset_v2").unwrap().is_none());
        let (change, version, snapshot) = fixture
            .repository
            .confirm_change_set("change_a", &confirmed, snapshot.etag())
            .unwrap();
        assert_eq!(change.status, ChangeSetStatus::Confirmed);
        assert_eq!(snapshot.active_design.asset_version_id(), Some("asset_v2"));
        assert_eq!(
            fixture.repository.head("project_a").unwrap().as_deref(),
            Some("asset_v2")
        );

        for asset_version_id in ["asset_v1", "asset_v2"] {
            for role in ["interactive_preview_glb", "production_glb"] {
                fixture
                    .repository
                    .attach_object_bytes(
                        &ObjectReference {
                            reference_kind: "asset_version".into(),
                            owner_id: asset_version_id.into(),
                            role: role.into(),
                        },
                        format!("glTF-{asset_version_id}-{role}").as_bytes(),
                        "glb",
                        "t3",
                    )
                    .unwrap();
            }
        }

        let quality_snapshot = fixture
            .repository
            .attach_quality(
                &QualityReport {
                    quality_report_id: "quality_asset_v2".into(),
                    project_id: "project_a".into(),
                    asset_version_id: "asset_v2".into(),
                    report: json!({
                        "schema_version":"AgentAssetQualityReport@1",
                        "quality_report_id":"quality_asset_v2",
                        "asset_version_id":"asset_v2",
                        "status":"passed"
                    }),
                    status: QualityStatus::Passed,
                    created_at: "t3".into(),
                },
                snapshot.etag(),
            )
            .unwrap();

        let undo = fixture
            .repository
            .navigate(
                "project_a",
                NavigationAction::Undo,
                "asset_v3",
                quality_snapshot.etag(),
                "t4",
            )
            .unwrap();
        assert_eq!(undo.version.shape_program, original.shape_program);
        assert_ne!(undo.version.asset_version_id, original.asset_version_id);
        let redo = fixture
            .repository
            .navigate(
                "project_a",
                NavigationAction::Redo,
                "asset_v4",
                undo.snapshot.etag(),
                "t5",
            )
            .unwrap();
        assert_eq!(redo.version.shape_program, version.shape_program);
        let cloned_quality = redo.snapshot.quality.as_ref().unwrap();
        assert_eq!(cloned_quality.asset_version_id, "asset_v4");
        let cloned_report = fixture
            .repository
            .quality_report(&cloned_quality.quality_report_id)
            .unwrap()
            .unwrap();
        assert_eq!(cloned_report.asset_version_id, "asset_v4");
        assert_eq!(cloned_report.report["asset_version_id"], "asset_v4");
        assert_eq!(
            fixture.repository.head("project_a").unwrap().as_deref(),
            Some("asset_v4")
        );
    }

    #[test]
    fn quality_is_bound_to_current_asset_and_stale_insert_rolls_back() {
        let fixture = Fixture::new();
        fixture.seed();
        let report = QualityReport {
            quality_report_id: "quality_a".into(),
            project_id: "project_a".into(),
            asset_version_id: "asset_v1".into(),
            report: json!({"glb_readback":true}),
            status: QualityStatus::Passed,
            created_at: "t2".into(),
        };
        let snapshot = fixture
            .repository
            .attach_quality(&report, SnapshotEtag(1))
            .unwrap();
        assert_eq!(snapshot.quality.unwrap().quality_report_id, "quality_a");
        let stale = QualityReport {
            quality_report_id: "quality_stale".into(),
            created_at: "t3".into(),
            ..report
        };
        assert_eq!(
            fixture
                .repository
                .attach_quality(&stale, SnapshotEtag(1))
                .unwrap_err()
                .code(),
            "ACTIVE_DESIGN_STALE"
        );
        assert!(fixture
            .repository
            .quality_report("quality_stale")
            .unwrap()
            .is_none());
    }

    #[test]
    fn object_reference_counts_deduplicate_and_corruption_is_detected() {
        let fixture = Fixture::new();
        let reference_a = ObjectReference {
            reference_kind: "asset_version".into(),
            owner_id: "asset_v1".into(),
            role: "production_glb".into(),
        };
        let reference_b = ObjectReference {
            owner_id: "asset_v2".into(),
            ..reference_a.clone()
        };
        let first = fixture
            .repository
            .attach_object_bytes(&reference_a, b"glb", "glb", "t1")
            .unwrap();
        fixture
            .repository
            .attach_object_bytes(&reference_b, b"glb", "glb", "t2")
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .object(&first.sha256)
                .unwrap()
                .unwrap()
                .ref_count,
            2
        );
        fixture.repository.detach_object(&reference_a).unwrap();
        assert_eq!(
            fixture
                .repository
                .object(&first.sha256)
                .unwrap()
                .unwrap()
                .ref_count,
            1
        );
        fixture.repository.detach_object(&reference_b).unwrap();
        assert_eq!(
            fixture.repository.collect_unreferenced_objects().unwrap(),
            vec![first.sha256.clone()]
        );
        assert!(fixture.repository.object(&first.sha256).unwrap().is_none());
    }

    #[test]
    fn reference_evidence_source_is_readable_by_id_but_cannot_be_detached_or_collected() {
        let fixture = Fixture::new();
        fixture.seed();
        let request = CreateReferenceEvidenceRequest {
            schema_version: "ReferenceEvidenceCreateRequest@1".into(),
            client_request_id: "reference-source-protection".into(),
            project_id: "project_a".into(),
            kind: ReferenceEvidenceKind::Image,
            file_name: Some("reference.png".into()),
            media_type: Some("image/png".into()),
            reference_class: None,
            content_base64: Some("iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGPQjVnwHx9mGBkKANXiigEwD3bkAAAAAElFTkSuQmCC".into()),
            imported_asset_version_id: None,
            source_statement: "User-authorized test reference.".into(),
            license_statement: "Test-only local rights.".into(),
            missing_views: vec!["rear".into()],
            user_notes: "visible shell".into(),
            domain_pack_id: Some("pack_future_weapon_prop".into()),
        };
        let evidence = fixture
            .repository
            .create_reference_evidence(&request, "t2")
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .reference_evidence_for_project("project_a")
                .unwrap(),
            vec![evidence.clone()]
        );
        let (readback, bytes) = fixture
            .repository
            .read_reference_evidence_content("project_a", &evidence.evidence_id)
            .unwrap();
        assert_eq!(readback, evidence);
        assert!(!bytes.is_empty());
        assert_eq!(
            fixture
                .repository
                .read_reference_evidence_content("another_project", &evidence.evidence_id)
                .unwrap_err()
                .code(),
            "REFERENCE_EVIDENCE_PROJECT_MISMATCH"
        );

        let connection = open_connection(fixture.repository.db_path()).unwrap();
        let detached = connection.execute(
            "DELETE FROM forgecad_core_object_references WHERE reference_kind='reference' AND owner_id=? AND role=?",
            params![evidence.evidence_id, REFERENCE_EVIDENCE_SOURCE_ROLE],
        );
        assert!(
            detached.is_err(),
            "immutable reference source must not detach"
        );
        drop(connection);
        assert!(fixture
            .repository
            .collect_unreferenced_objects()
            .unwrap()
            .is_empty());
        assert!(fixture
            .repository
            .object(&evidence.source_object_sha256)
            .unwrap()
            .is_some());
    }

    #[test]
    fn deletion_journal_recovers_crash_after_index_commit_without_scanning() {
        let fixture = Fixture::new();
        let reference = ObjectReference {
            reference_kind: "asset_version".into(),
            owner_id: "asset_gc".into(),
            role: "production_glb".into(),
        };
        let record = fixture
            .repository
            .attach_object_bytes(&reference, b"journaled-glb", "glb", "t1")
            .unwrap();
        fixture.repository.detach_object(&reference).unwrap();
        let object_path = fixture
            ._root
            .path()
            .join("objects/sha256")
            .join(&record.object_path);
        assert!(object_path.is_file());

        // Simulate a process dying after SQLite committed the GC intent and
        // object-row deletion, but before the final unlink.
        fixture
            .repository
            .write(|transaction| {
                transaction.execute(
                    "INSERT INTO forgecad_core_object_deletion_journal(sha256, object_path, extension, byte_size, created_at) VALUES (?, ?, ?, ?, 't2')",
                    params![record.sha256, record.object_path, record.extension, record.byte_size],
                )?;
                transaction.execute(
                    "DELETE FROM forgecad_core_objects WHERE sha256=? AND ref_count=0",
                    [&record.sha256],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(fixture.repository.object(&record.sha256).unwrap().is_none());
        assert!(object_path.is_file());

        assert_eq!(
            fixture.repository.recover_object_deletions().unwrap(),
            vec![record.sha256.clone()]
        );
        assert!(!object_path.exists());
        assert!(fixture
            .repository
            .recover_object_deletions()
            .unwrap()
            .is_empty());
        let connection = open_connection(fixture.repository.db_path()).unwrap();
        let pending: u64 = connection
            .query_row(
                "SELECT COUNT(*) FROM forgecad_core_object_deletion_journal",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pending, 0);
    }

    #[test]
    fn bootstrap_deletion_recovery_does_not_publish_and_allows_cutover_rollback() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        MigrationRunner::new(&db).run().unwrap();
        let (_stored, object_path) =
            seed_pending_deletion(&db, root.path(), b"bootstrap-pending-glb", "t1");

        let initializing =
            CoreRepository::open(&db, root.path(), "bootstrap_recovery_initializing").unwrap();
        assert!(!initializing.lease.is_published());
        assert_eq!(pending_deletion_count(&db), 0);
        assert!(!object_path.exists());

        // Simulate a later desktop-handler initialization failure. Recovery
        // completed consistently, but did not make the first cutover public.
        assert!(initializing.rollback_cutover_before_publish().unwrap());
        drop(initializing);
        let marker = read_ownership_marker(&db).unwrap();
        assert_eq!(marker.state_owner, StateOwner::PythonCompatibilityAdapter);
        assert!(marker.active_writer_instance_id.is_none());
        assert_eq!(pending_deletion_count(&db), 0);
        assert!(!object_path.exists());
    }

    #[test]
    fn bootstrap_deletion_recovery_retries_then_publishes_and_restarts_normally() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        MigrationRunner::new(&db).run().unwrap();
        let (_stored, object_path) =
            seed_pending_deletion(&db, root.path(), b"retryable-pending-glb", "t1");

        let failed_initialization =
            CoreRepository::open(&db, root.path(), "bootstrap_recovery_failed_once").unwrap();
        assert!(!failed_initialization.lease.is_published());
        assert!(failed_initialization
            .rollback_cutover_before_publish()
            .unwrap());
        drop(failed_initialization);

        let retry = CoreRepository::open(&db, root.path(), "bootstrap_recovery_retry").unwrap();
        assert!(!retry.lease.is_published());
        assert_eq!(pending_deletion_count(&db), 0);
        assert!(!object_path.exists());
        retry.publish().unwrap();
        drop(retry);
        assert_eq!(
            read_ownership_marker(&db).unwrap().state_owner,
            StateOwner::RustAppServer
        );

        let restarted =
            CoreRepository::open(&db, root.path(), "bootstrap_recovery_restart").unwrap();
        restarted.publish().unwrap();
    }

    #[test]
    fn candidate_glb_lives_in_cas_and_commit_creates_one_authoritative_chain() {
        let fixture = Fixture::new();
        fixture
            .repository
            .insert_project(&Project {
                project_id: "project_a".into(),
                profile_id: "profile_weapon".into(),
                domain_type: "weapon_concept".into(),
                name: "Candidate Project".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "t1".into(),
                updated_at: "t1".into(),
            })
            .unwrap();
        let candidate = BlockoutCandidate {
            artifact_id: "artifact_best".into(),
            project_id: Some("project_a".into()),
            plan_id: "plan_a".into(),
            direction_id: "direction_best".into(),
            domain_pack_id: "pack_weapon".into(),
            status: CandidateStatus::Candidate,
            candidate: json!({"score": 0.97}),
            shape_program: json!({"schema_version":"ShapeProgram@1","shell":"best"}),
            assembly_graph: json!({"graph_id":"graph_best","parts":[{"part_id":"part_shell","material_zone_ids":["zone_shell"]}]}),
            material_bindings: BTreeMap::new(),
            glb_sha256: String::new(),
            created_at: "t2".into(),
            updated_at: "t2".into(),
        };
        let candidate = fixture
            .repository
            .create_candidate(candidate, b"production-glb")
            .unwrap();
        assert_eq!(
            fixture
                .repository
                .read_object(&candidate.glb_sha256)
                .unwrap(),
            b"production-glb"
        );
        let connection = open_connection(fixture.repository.db_path()).unwrap();
        let legacy_base64: String = connection
            .query_row(
                "SELECT glb_base64 FROM agent_blockout_candidates WHERE artifact_id='artifact_best'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(legacy_base64.is_empty());
        drop(connection);

        let mut version = asset("asset_best", None, 1, "best", "t3");
        version.artifact_id = candidate.artifact_id.clone();
        version.shape_program = candidate.shape_program.clone();
        version.assembly_graph = candidate.assembly_graph.clone();
        let (candidate, version, snapshot) = fixture
            .repository
            .commit_candidate("artifact_best", &version)
            .unwrap();
        assert_eq!(candidate.status, CandidateStatus::Committed);
        assert_eq!(
            snapshot.active_design.asset_version_id(),
            Some("asset_best")
        );
        assert_eq!(
            fixture
                .repository
                .object(&candidate.glb_sha256)
                .unwrap()
                .unwrap()
                .ref_count,
            1
        );
        assert_eq!(version.artifact_id, "artifact_best");
    }

    #[test]
    fn repository_rechecks_durable_writer_epoch_inside_every_transaction() {
        let fixture = Fixture::new();
        let connection = open_connection(fixture.repository.db_path()).unwrap();
        connection
            .execute(
                "UPDATE forgecad_core_ownership SET writer_epoch=writer_epoch+1 WHERE singleton=1",
                [],
            )
            .unwrap();
        let error = fixture
            .repository
            .insert_project(&Project {
                project_id: "project_x".into(),
                profile_id: "profile_weapon".into(),
                domain_type: "weapon_concept".into(),
                name: "X".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "t".into(),
                updated_at: "t".into(),
            })
            .unwrap_err();
        assert_eq!(error.code(), "RUST_CORE_WRITER_LEASE_STALE");
    }

    #[test]
    fn open_handles_first_cutover_rollback_duplicate_and_rust_restart() {
        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        let first = CoreRepository::open(&db, root.path(), "bootstrap_first").unwrap();
        assert_eq!(
            read_ownership_marker(&db).unwrap().state_owner,
            StateOwner::RustAppServer
        );
        assert_eq!(
            CoreRepository::open(&db, root.path(), "bootstrap_duplicate")
                .unwrap_err()
                .code(),
            "RUST_CORE_WRITER_ALREADY_ACTIVE"
        );
        assert!(first.rollback_cutover_before_publish().unwrap());
        drop(first);
        assert_eq!(
            read_ownership_marker(&db).unwrap().state_owner,
            StateOwner::PythonCompatibilityAdapter
        );

        let published = CoreRepository::open(&db, root.path(), "bootstrap_published").unwrap();
        published.publish().unwrap();
        drop(published);
        assert_eq!(
            read_ownership_marker(&db).unwrap().state_owner,
            StateOwner::RustAppServer
        );
        let restarted = CoreRepository::open(&db, root.path(), "bootstrap_restart").unwrap();
        restarted.publish().unwrap();
    }

    #[test]
    fn unpublished_first_cutover_process_crash_fail_forwards_and_recovers_pending_cas() {
        const CHILD_DB: &str = "FORGECAD_K003_CRASH_TEST_DB";
        const CHILD_ROOT: &str = "FORGECAD_K003_CRASH_TEST_ROOT";
        const CHILD_READY: &str = "FORGECAD_K003_CRASH_TEST_READY";
        const TEST_NAME: &str = "repository::tests::unpublished_first_cutover_process_crash_fail_forwards_and_recovers_pending_cas";

        // The child is killed while the Arc<WriterLease> is live. Its Drop and
        // rollback paths therefore cannot run, matching SIGKILL/power-loss
        // behavior after durable acquire but before publish.
        if let (Ok(db), Ok(root), Ok(ready)) = (
            std::env::var(CHILD_DB),
            std::env::var(CHILD_ROOT),
            std::env::var(CHILD_READY),
        ) {
            let lease = WriterLease::acquire(
                &db,
                &root,
                "writer_crashed_before_publish",
                StateOwner::PythonCompatibilityAdapter,
            )
            .unwrap();
            assert_eq!(lease.epoch(), 1);
            assert!(!lease.is_published());
            std::fs::write(ready, b"durable-acquire-complete").unwrap();
            loop {
                thread::park();
            }
        }

        let root = tempdir().unwrap();
        let db = root.path().join("library.db");
        MigrationRunner::new(&db).run().unwrap();

        // One pending promotion models rename-before-SQLite-commit. A separate
        // deletion journal models SQLite-commit-before-unlink. Both must be
        // safe and idempotent when the replacement Rust epoch initializes.
        let store = ContentAddressedObjectStore::new(root.path()).unwrap();
        let promoted = store
            .stage(b"crash-orphan-promotion", "glb")
            .unwrap()
            .promote()
            .unwrap();
        let orphan = promoted.metadata().clone();
        let orphan_path = root
            .path()
            .join("objects/sha256")
            .join(&orphan.relative_path);
        drop(promoted);
        let (_deleting, deleting_path) =
            seed_pending_deletion(&db, root.path(), b"crash-pending-deletion", "t1");
        assert_eq!(pending_promotion_count(root.path()), 1);
        assert_eq!(pending_deletion_count(&db), 1);
        assert!(orphan_path.is_file());
        assert!(deleting_path.is_file());

        let ready = root.path().join("crash-ready");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(TEST_NAME)
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env(CHILD_DB, &db)
            .env(CHILD_ROOT, root.path())
            .env(CHILD_READY, &ready)
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.is_file() {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("crash-test child exited before acquisition: {status}");
            }
            assert!(
                Instant::now() < deadline,
                "crash-test child did not acquire the writer lease"
            );
            thread::sleep(Duration::from_millis(10));
        }
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());

        let crashed = read_ownership_marker(&db).unwrap();
        assert_eq!(crashed.state_owner, StateOwner::RustAppServer);
        assert_eq!(
            crashed.active_writer_instance_id.as_deref(),
            Some("writer_crashed_before_publish")
        );
        assert_eq!(crashed.writer_epoch, 1);

        // Acquiring the now-free OS lock proves the old process is gone. The
        // replacement keeps Rust as durable owner, advances the epoch, records
        // explicit recovery evidence, completes journals, then publishes.
        let recovered = CoreRepository::open(&db, root.path(), "writer_recovered").unwrap();
        let recovery = recovered.lease.recovered_writer().unwrap();
        assert_eq!(
            recovery.previous_instance_id,
            "writer_crashed_before_publish"
        );
        assert_eq!(recovery.previous_epoch, 1);
        assert_eq!(recovered.lease.epoch(), 2);
        assert_eq!(pending_promotion_count(root.path()), 0);
        assert_eq!(pending_deletion_count(&db), 0);
        assert!(!orphan_path.exists());
        assert!(!deleting_path.exists());
        recovered.publish().unwrap();
        drop(recovered);

        let published = read_ownership_marker(&db).unwrap();
        assert_eq!(published.state_owner, StateOwner::RustAppServer);
        assert!(published.active_writer_instance_id.is_none());
        assert_eq!(published.writer_epoch, 2);

        // Even after the replacement shuts down, a compatibility writer can
        // never reclaim the durable Rust phase. A clean Rust restart advances
        // the epoch again without reporting another crash recovery.
        let python_reclaim = WriterLease::acquire(
            &db,
            root.path(),
            "python_legacy_writer",
            StateOwner::PythonCompatibilityAdapter,
        )
        .unwrap_err();
        assert_eq!(python_reclaim.code(), "RUST_CORE_OWNERSHIP_STALE");
        let clean_restart = CoreRepository::open(&db, root.path(), "writer_clean_restart").unwrap();
        assert_eq!(clean_restart.lease.epoch(), 3);
        assert!(clean_restart.lease.recovered_writer().is_none());
        clean_restart.publish().unwrap();
    }

    #[test]
    fn r007_image_evidence_is_cas_sealed_does_not_mutate_design_and_can_start_initial_plan() {
        let fixture = Fixture::new();
        fixture
            .repository
            .insert_project(&Project {
                project_id: "prj_robot".into(),
                profile_id: "profile_weapon".into(),
                // The legacy fixture's profile table is intentionally limited
                // to its historical domain; R007's Domain Pack is validated
                // independently by the new contract.
                domain_type: "weapon_concept".into(),
                name: "Robot reference".into(),
                status: ProjectStatus::Active,
                current_version_id: None,
                created_at: "t1".into(),
                updated_at: "t1".into(),
            })
            .unwrap();
        let request = CreateReferenceEvidenceRequest {
            schema_version: "ReferenceEvidenceCreateRequest@1".into(),
            client_request_id: "ui-r007-image-1".into(),
            project_id: "prj_robot".into(),
            kind: ReferenceEvidenceKind::Image,
            // Omitted UI classification is intentionally resolved by the
            // repository from the locally observed image facts.
            reference_class: None,
            file_name: Some("arm.png".into()),
            media_type: Some("image/png".into()),
            // Complete bounded 8x8 PNG fixture. A signature-only byte string
            // is not a valid reference image and must remain rejected by the
            // same container/decode path used in production.
            content_base64: Some(
                "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGPQjVnwHx9mGBkKANXiigEwD3bkAAAAAElFTkSuQmCC"
                    .into(),
            ),
            imported_asset_version_id: None,
            source_statement: "User supplied this image for visual reference.".into(),
            license_statement: "User declares permission for local reference use only.".into(),
            missing_views: vec!["rear".into(), "left".into()],
            user_notes: "Blue articulated desk arm: base, joint, link, cable and end effector."
                .into(),
            domain_pack_id: Some("pack_robotic_arm_concept".into()),
        };
        let evidence = fixture
            .repository
            .create_reference_evidence(&request, "t2")
            .unwrap();
        let replay = fixture
            .repository
            .create_reference_evidence(&request, "t3")
            .unwrap();
        assert_eq!(replay, evidence);
        let mut conflicting_request = request.clone();
        conflicting_request.user_notes = "different source input".into();
        assert_eq!(
            fixture
                .repository
                .create_reference_evidence(&conflicting_request, "t4")
                .unwrap_err()
                .code(),
            "IDEMPOTENCY_CONFLICT"
        );
        let mut invalid_image_request = request.clone();
        invalid_image_request.client_request_id = "ui-r007-image-bad".into();
        invalid_image_request.content_base64 = Some(BASE64_STANDARD.encode(b"not-a-png"));
        assert_eq!(
            fixture
                .repository
                .create_reference_evidence(&invalid_image_request, "t4")
                .unwrap_err()
                .code(),
            "REFERENCE_IMAGE_MAGIC_INVALID"
        );
        assert_eq!(evidence.kind, ReferenceEvidenceKind::Image);
        assert!(evidence
            .observations
            .uncertainties
            .iter()
            .any(|item| item.contains("无法恢复")));
        assert!(evidence
            .observations
            .visible_part_hypotheses
            .iter()
            .any(|item| item.role == "joint_housing"));
        assert_eq!(fixture.repository.head("prj_robot").unwrap(), None);
        assert_eq!(fixture.repository.snapshot("prj_robot").unwrap(), None);
        assert!(fixture
            .repository
            .object(&evidence.source_object_sha256)
            .unwrap()
            .is_some());
        assert_eq!(
            fixture
                .repository
                .detach_object(&ObjectReference {
                    reference_kind: "reference".into(),
                    owner_id: evidence.evidence_id.clone(),
                    role: REFERENCE_EVIDENCE_SOURCE_ROLE.into(),
                })
                .unwrap_err()
                .code(),
            "SQLITE_OPERATION_FAILED"
        );

        let registry = RecipeRegistry::from_embedded().unwrap();
        let plan = fixture.repository.create_reference_guided_rebuild_plan(&ReferenceGuidedRebuildPlan {
            schema_version: REFERENCE_GUIDED_REBUILD_PLAN_SCHEMA_VERSION.into(),
            rebuild_plan_id: "rebuildplan_robot_image_1".into(), project_id: "prj_robot".into(),
            evidence_id: evidence.evidence_id, base_asset_version_id: None,
            domain_pack_id: "pack_robotic_arm_concept".into(), recipe_id: "recipe_robotic_arm_link".into(),
            recipe_registry_sha256: registry.registry_sha256().into(), rebuild_summary: "Rebuild a new visual-only articulated arm from visible evidence.".into(),
            intended_differences: vec!["Use the reviewed robotic-arm link Recipe rather than source pixels.".into()],
            retained_evidence: vec!["Keep visible articulated silhouette and blue/dark zone language as non-authoritative evidence.".into()],
            unresolved_uncertainties: vec!["Rear and internal structure remain unknown.".into()],
            status: ReferenceGuidedRebuildPlanStatus::Draft, preview_change_set_id: None,
            confirmed_asset_version_id: None, created_at: "t2".into(), updated_at: "t2".into(),
        }).unwrap();
        assert_eq!(plan.status, ReferenceGuidedRebuildPlanStatus::Draft);
        assert!(plan.base_asset_version_id.is_none());
    }
}
