//! Candidate-bound game-asset delivery preparation.
//!
//! P0 deliberately accepts three independently authored and compiled GLBs.
//! It does not claim automatic decimation. Runtime replays strict readback,
//! verifies stable semantic Part/material coverage and progressive triangle
//! budgets, derives one conservative local-space AABB box per Part from the
//! actual LOD2 POSITION bytes, and stores immutable JSON sidecars in CAS.
//! No candidate/version is changed, no file is exported and no external game
//! engine is invoked.

use super::{
    camera_identity_hash, canonical_json_bytes, canonical_json_hash,
    compile_geometry_with_runtime_worker, default_camera_calibration,
    hash_geometry_program_with_runtime_worker, material_pack_manifest_by_id, now_string,
    render_worker, sha256_hex, strict_glb_inspection, validate_geometry_candidate_evidence_output,
    validate_worker_metadata, CasObject, Runtime, RuntimeError,
};
use forgecad_contracts::{
    FictionalEnergyVfxBloomFrameLinkRecord, FictionalEnergyVfxFrameLinkRecord,
    FictionalEnergyVfxLinkRecord, FictionalEnergyVfxParticlesFrameLinkRecord,
    FictionalEnergyVfxSequenceLinkRecord, FictionalEnergyVfxTrailsBloomFrameLinkRecord,
    FictionalEnergyVfxTrailsFrameLinkRecord, GameAssetDeliveryLinkRecord,
    GameWeaponAnchorLinkRecord, GameWeaponGlbSocketMaterializationLinkRecord,
    GameWeaponGlbSocketMaterializationLodRecord,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const ERROR: &str = "GAME_ASSET_DELIVERY_INVALID";
const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PARTS: usize = 64;
const MAX_JSON_BYTES: u64 = 1024 * 1024;
const LOD_DERIVE_POLICY: &str = "runtime-owned-typed-segment-lowering-lod1-half-lod2-quarter@1";
const GAME_WEAPON_GLB_SOCKET_MATERIALIZED_GLB_KIND: &str =
    "game-weapon-glb-socket-materialized-glb";
const GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_RECEIPT_KIND: &str =
    "game-weapon-glb-socket-materialization-receipt";
const GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_STATUS: &str =
    "runtime-owned-durable-game-weapon-glb-socket-materialization";
const GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_POLICY: &str =
    "gltf-anchor-node-materialization-preserve-renderable-content@1";
const GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_LOD_SCOPE: &str = "lod0-lod1-lod2@1";
const GAME_WEAPON_GLB_SOCKET_PREPARE_SCHEMA: &str =
    "GameWeaponGlbSocketMaterializationPrepareRequest@1";
const GAME_WEAPON_GLB_SOCKET_PREPARE_RESULT_SCHEMA: &str =
    "GameWeaponGlbSocketMaterializationPrepareResult@1";
const GAME_WEAPON_GLB_SOCKET_GET_SCHEMA: &str = "GameWeaponGlbSocketMaterializationGetRequest@1";
const GAME_WEAPON_GLB_SOCKET_GET_RESULT_SCHEMA: &str =
    "GameWeaponGlbSocketMaterializationGetResult@1";
const GAME_WEAPON_GLB_SOCKET_RECEIPT_SCHEMA: &str = "GameWeaponGlbSocketMaterializationReceipt@1";
const GAME_WEAPON_GLB_SOCKET_READBACK_SCHEMA: &str =
    "GameWeaponGlbSocketMaterializationLodReadback@1";
const GAME_WEAPON_GLB_SOCKET_NODE_SCHEMA: &str = "GameWeaponGlbSocketNode@1";
const GAME_WEAPON_GLB_SOCKET_NODE_INVENTORY_SCHEMA: &str = "GameWeaponGlbSocketNodeInventory@1";
const GAME_WEAPON_GLB_RENDERABLE_INVENTORY_SCHEMA: &str = "GameWeaponGlbRenderableInventory@1";
const GAME_WEAPON_GLB_SOCKET_ROOT_EXTRA: &str = "game_weapon_glb_socket_materialization";
const GAME_WEAPON_GLB_SOCKET_NODE_PREFIX: &str = "forgecad-anchor-";
const GAME_WEAPON_GLB_SOCKET_NODE_ID_ENCODING: &str =
    "stable-name-prefix-forgecad-anchor-anchor-id@1";

#[derive(Clone, Copy, Debug, PartialEq)]
struct Bounds {
    min: [f64; 3],
    max: [f64; 3],
}

impl Bounds {
    fn empty() -> Self {
        Self {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }

    fn include(&mut self, point: [f64; 3]) -> Result<(), RuntimeError> {
        if point
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 10.0)
        {
            return invalid("POSITION is non-finite or outside the bounded asset space");
        }
        for axis in 0..3 {
            self.min[axis] = self.min[axis].min(point[axis]);
            self.max[axis] = self.max[axis].max(point[axis]);
        }
        Ok(())
    }

    fn validate(self) -> Result<Self, RuntimeError> {
        if (0..3).any(|axis| {
            !self.min[axis].is_finite()
                || !self.max[axis].is_finite()
                || self.max[axis] <= self.min[axis]
        }) {
            return invalid("a Part bound is missing or degenerate");
        }
        Ok(self)
    }

    fn center(self) -> [f64; 3] {
        std::array::from_fn(|axis| (self.min[axis] + self.max[axis]) * 0.5)
    }

    fn half_extents(self) -> [f64; 3] {
        std::array::from_fn(|axis| (self.max[axis] - self.min[axis]) * 0.5)
    }

    fn value(self, part_id: &str) -> Value {
        json!({"part_id":part_id,"min_m":self.min,"max_m":self.max})
    }
}

#[derive(Debug)]
struct Level {
    level: u64,
    candidate_id: String,
    candidate_state_sha256: String,
    artifact_sha256: String,
    artifact_readback_sha256: String,
    triangle_count: u64,
    part_ids: Vec<String>,
    source_node_ids: Vec<String>,
    material_zone_ids: Vec<String>,
    part_material_bindings: BTreeMap<String, String>,
    bounds: BTreeMap<String, Bounds>,
    appearance_binding: Option<AppearanceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppearanceBinding {
    schema_version: String,
    program_sha256: String,
    material_pack_id: String,
    material_pack_manifest_sha256: String,
    zone_material_ids: BTreeMap<String, String>,
}

pub(super) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "lods",
            "animation",
            "lod_policy",
            "collision_policy",
            "readiness_policy",
            "canonical_sha256",
        ],
        "GameAssetDeliveryPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "GameAssetDeliveryPrepareRequest@1"
        || text(object, "lod_policy")? != "authored-three-level-part-stable-progressive-triangles@1"
        || text(object, "collision_policy")? != "per-part-aabb-box-from-lod2-visual-geometry@1"
        || text(object, "readiness_policy")?
            != "engine-neutral-gltf2-embedded-assets-stable-names@1"
    {
        return invalid("delivery policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let lods = object
        .get("lods")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error("exactly three LOD declarations are required"))?;

    let mut levels = Vec::with_capacity(3);
    for (expected_level, declaration) in lods.iter().enumerate() {
        levels.push(load_level(
            runtime,
            &project_id,
            expected_level as u64,
            declaration,
        )?);
    }
    validate_level_set(&levels)?;

    let lod0_triangles = levels[0].triangle_count as f64;
    let mut level_values = Vec::with_capacity(3);
    for level in &levels {
        let part_bounds = level
            .bounds
            .iter()
            .map(|(part_id, bounds)| bounds.value(part_id))
            .collect::<Vec<_>>();
        level_values.push(json!({
            "level":level.level,
            "candidate_id":level.candidate_id,
            "candidate_state_sha256":level.candidate_state_sha256,
            "artifact_sha256":level.artifact_sha256,
            "artifact_readback_sha256":level.artifact_readback_sha256,
            "triangle_count":level.triangle_count,
            "triangle_ratio_to_lod0":level.triangle_count as f64 / lod0_triangles,
            "part_bounds_sha256":canonical_json_hash(&Value::Array(part_bounds.clone())),
            "part_bounds":part_bounds
        }));
    }
    let mut lod_receipt = json!({
        "schema_version":"GameLodSetReceipt@1",
        "project_id":project_id,
        "levels":level_values,
        "part_ids":levels[0].part_ids,
        "material_zone_ids":levels[0].material_zone_ids,
        "triangle_policy":"lod1-at-most-75pct-lod0-and-lod2-at-most-50pct-lod0@1",
        "envelope_policy":"per-part-aabb-center-and-extent-within-10pct-of-lod0@1",
        "validator_status":"strict-three-level-lod-readback-pass",
        "hard_gate_passed":true,
        "quality_status":"structural_only",
        "canonical_sha256":""
    });
    lod_receipt = seal_sidecar(lod_receipt)?;
    let lod_object = put_json(runtime, &lod_receipt, "game-lod-set-receipt")?;

    let lod2 = &levels[2];
    let proxies = lod2
        .bounds
        .iter()
        .map(|(part_id, bounds)| {
            let source_bound = bounds.value(part_id);
            json!({
                "proxy_id":format!("collision.{part_id}"),
                "part_id":part_id,
                "shape":"box",
                "center_m":bounds.center(),
                "half_extents_m":bounds.half_extents(),
                "rotation_quat_xyzw":[0,0,0,1],
                "source_bounds_sha256":canonical_json_hash(&source_bound)
            })
        })
        .collect::<Vec<_>>();
    let mut collision = json!({
        "schema_version":"CollisionProxySet@1",
        "project_id":project_id,
        "source_lod_level":2,
        "source_candidate_id":lod2.candidate_id,
        "source_candidate_state_sha256":lod2.candidate_state_sha256,
        "source_artifact_sha256":lod2.artifact_sha256,
        "source_artifact_readback_sha256":lod2.artifact_readback_sha256,
        "lod_receipt_object_sha256":lod_object.record.sha256,
        "part_ids":lod2.part_ids,
        "policy":"per-part-aabb-box-from-lod2-visual-geometry@1",
        "proxies":proxies,
        "coverage":1,
        "gameplay_only":true,
        "physical_properties_included":false,
        "validator_status":"exact-part-aabb-coverage-pass",
        "hard_gate_passed":true,
        "quality_status":"structural_only",
        "canonical_sha256":""
    });
    collision = seal_sidecar(collision)?;
    let collision_object = put_json(runtime, &collision, "collision-proxy-set")?;

    let animation = validate_animation(runtime, &levels[0], object.get("animation"))?;
    let mut readiness = json!({
        "schema_version":"GameEngineImportReadiness@1",
        "project_id":project_id,
        "lod_receipt_object_sha256":lod_object.record.sha256,
        "collision_proxy_object_sha256":collision_object.record.sha256,
        "target_profile":"engine-neutral-gltf2-embedded-assets-stable-names@1",
        "gltf_version":"2.0",
        "container":"GLB",
        "coordinate_system":"right-handed-y-up",
        "unit":"meter",
        "lod_count":3,
        "collision_proxy_count":collision["proxies"].as_array().map_or(0, Vec::len),
        "stable_part_names":true,
        "stable_material_zone_names":true,
        "embedded_resources_only":true,
        "external_uri_count":0,
        "animation_status":animation.as_ref().map_or("absent", |_| "lod0-only-strict-rigid-gltf-animation-readback-pass"),
        "animation_artifact_sha256":animation.as_ref().map(|value| value["animated_artifact_sha256"].clone()).unwrap_or(Value::Null),
        "product_loader_validation":"product-owned-strict-gltf-readiness-pass",
        "actual_engine_roundtrip":false,
        "engine_results":{"threejs":"NOT_RUN","godot":"NOT_RUN","unity":"NOT_RUN","unreal":"NOT_RUN"},
        "validator_status":"engine-neutral-readiness-pass-real-engine-roundtrip-not-run",
        "hard_gate_passed":true,
        "quality_status":"structural_only",
        "canonical_sha256":""
    });
    readiness = seal_sidecar(readiness)?;
    let readiness_object = put_json(runtime, &readiness, "game-engine-import-readiness")?;

    let request_sha256 = canonical_json_hash(request);
    let mut manifest = json!({
        "schema_version":"GameAssetDeliveryManifest@1",
        "project_id":project_id,
        "request_sha256":request_sha256,
        "lod_receipt_object_sha256":lod_object.record.sha256,
        "collision_proxy_object_sha256":collision_object.record.sha256,
        "readiness_object_sha256":readiness_object.record.sha256,
        "lod_artifact_sha256s":levels.iter().map(|level| level.artifact_sha256.clone()).collect::<Vec<_>>(),
        "animation":animation,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only",
        "canonical_sha256":""
    });
    manifest = seal_sidecar(manifest)?;
    let manifest_object = put_json(runtime, &manifest, "game-asset-delivery-manifest")?;

    let mut durable_link = GameAssetDeliveryLinkRecord {
        schema_version: "GameAssetDeliveryLink@1".to_owned(),
        project_id: project_id.clone(),
        lod_candidate_ids: levels
            .iter()
            .map(|level| level.candidate_id.clone())
            .collect(),
        lod_artifact_sha256s: levels
            .iter()
            .map(|level| level.artifact_sha256.clone())
            .collect(),
        request_sha256,
        lod_receipt_object_sha256: lod_object.record.sha256.clone(),
        collision_proxy_object_sha256: collision_object.record.sha256.clone(),
        readiness_object_sha256: readiness_object.record.sha256.clone(),
        delivery_manifest_object_sha256: manifest_object.record.sha256.clone(),
        animation_artifact_sha256: manifest
            .get("animation")
            .and_then(|value| value.get("animated_artifact_sha256"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        materialization_status: "runtime-owned-durable-game-delivery-link".to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    durable_link.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&durable_link).map_err(|source| error(source.to_string()))?,
    );
    let durable_link = runtime
        .store
        .record_game_asset_delivery_link(&durable_link)?;

    Ok(json!({
        "schema_version":"GameAssetDeliveryPrepareResult@1",
        "lod_receipt_object_sha256":lod_object.record.sha256,
        "lod_receipt":lod_receipt,
        "collision_proxy_object_sha256":collision_object.record.sha256,
        "collision_proxy_set":collision,
        "readiness_object_sha256":readiness_object.record.sha256,
        "readiness":readiness,
        "delivery_manifest_object_sha256":manifest_object.record.sha256,
        "durable_link":durable_link,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
        ],
        "GameAssetDeliveryGetRequest@1",
    )?;
    if text(object, "schema_version")? != "GameAssetDeliveryGetRequest@1" {
        return invalid("game delivery get schema version differs");
    }
    let project_id = identifier(object, "project_id")?;
    let manifest_sha256 = sha(object, "delivery_manifest_object_sha256")?;
    let link = runtime
        .store
        .get_game_asset_delivery_link(manifest_sha256)?
        .ok_or_else(|| error("durable game delivery link is unavailable"))?;
    if link.project_id != project_id {
        return invalid("durable game delivery belongs to a different project");
    }
    let manifest = read_json(
        runtime,
        &link.delivery_manifest_object_sha256,
        "GameAssetDeliveryManifest@1",
    )?;
    let lod_receipt = read_json(
        runtime,
        &link.lod_receipt_object_sha256,
        "GameLodSetReceipt@1",
    )?;
    let collision_proxy_set = read_json(
        runtime,
        &link.collision_proxy_object_sha256,
        "CollisionProxySet@1",
    )?;
    let readiness = read_json(
        runtime,
        &link.readiness_object_sha256,
        "GameEngineImportReadiness@1",
    )?;
    if manifest.get("project_id").and_then(Value::as_str) != Some(project_id)
        || manifest.get("request_sha256").and_then(Value::as_str)
            != Some(link.request_sha256.as_str())
        || manifest
            .get("lod_receipt_object_sha256")
            .and_then(Value::as_str)
            != Some(link.lod_receipt_object_sha256.as_str())
        || manifest
            .get("collision_proxy_object_sha256")
            .and_then(Value::as_str)
            != Some(link.collision_proxy_object_sha256.as_str())
        || manifest
            .get("readiness_object_sha256")
            .and_then(Value::as_str)
            != Some(link.readiness_object_sha256.as_str())
        || manifest
            .get("lod_artifact_sha256s")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            != Some(
                link.lod_artifact_sha256s
                    .iter()
                    .map(String::as_str)
                    .collect(),
            )
        || lod_receipt.get("project_id").and_then(Value::as_str) != Some(project_id)
        || collision_proxy_set
            .get("lod_receipt_object_sha256")
            .and_then(Value::as_str)
            != Some(link.lod_receipt_object_sha256.as_str())
        || readiness
            .get("lod_receipt_object_sha256")
            .and_then(Value::as_str)
            != Some(link.lod_receipt_object_sha256.as_str())
        || readiness
            .get("collision_proxy_object_sha256")
            .and_then(Value::as_str)
            != Some(link.collision_proxy_object_sha256.as_str())
    {
        return invalid("durable game delivery CAS binding differs");
    }
    Ok(json!({
        "schema_version":"GameAssetDeliveryGetResult@1",
        "link":link,
        "manifest":manifest,
        "lod_receipt":lod_receipt,
        "collision_proxy_set":collision_proxy_set,
        "readiness":readiness,
        "restart_hash_verified":true,
        "quality_status":"structural_only"
    }))
}

/// Persist a strictly typed, engine-neutral fictional-weapon attachment
/// sidecar. V1 creates metadata helpers only: it does not rewrite GLB nodes,
/// prove runtime pivots, define physics/hitboxes or invoke a commercial engine.
pub(super) fn weapon_anchor_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "anchor_policy",
            "anchors",
            "canonical_sha256",
        ],
        "GameWeaponAnchorPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "GameWeaponAnchorPrepareRequest@1"
        || text(object, "anchor_policy")? != "weapon-rh-x-forward-y-up-model-space-six-role@1"
    {
        return invalid("weapon anchor policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?;
    let delivery_manifest_sha256 = sha(object, "delivery_manifest_object_sha256")?;
    let delivery = get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_manifest_sha256
        }),
    )?;
    let link = delivery
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| error("weapon anchor durable delivery link is unavailable"))?;
    let lod_receipt = delivery
        .get("lod_receipt")
        .and_then(Value::as_object)
        .ok_or_else(|| error("weapon anchor LOD receipt is unavailable"))?;
    let part_ids = lod_receipt
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| error("weapon anchor Part inventory is unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| forgecad_contracts::is_opaque_id(value))
                .map(str::to_owned)
                .ok_or_else(|| error("weapon anchor Part ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if part_ids.len() < 5 || part_ids.len() > MAX_PARTS {
        return invalid("weapon anchor delivery requires at least five stable Parts");
    }
    let levels = lod_receipt
        .get("levels")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error("weapon anchor delivery requires exact LOD0/1/2 bindings"))?;
    let mut lod_bindings = Vec::with_capacity(3);
    for (expected_level, level) in levels.iter().enumerate() {
        if level.get("level").and_then(Value::as_u64) != Some(expected_level as u64) {
            return invalid("weapon anchor LOD order differs");
        }
        lod_bindings.push(json!({
            "level":expected_level,
            "candidate_id":level.get("candidate_id"),
            "candidate_state_sha256":level.get("candidate_state_sha256"),
            "artifact_sha256":level.get("artifact_sha256"),
            "artifact_readback_sha256":level.get("artifact_readback_sha256")
        }));
    }
    let lod0_bounds = part_bounds_from_lod(levels.first().expect("three LOD levels"))?;
    let anchors = validate_weapon_anchors(object.get("anchors"), &part_ids, &lod0_bounds)?;
    let animation_follow_status = if link
        .get("animation_artifact_sha256")
        .is_some_and(|value| !value.is_null())
    {
        "lod0-rigid-part-trs-follow-structural"
    } else {
        "static-only-no-animation"
    };
    let mut anchor_set = json!({
        "schema_version":"GameWeaponAnchorSet@1",
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_manifest_sha256,
        "lod_bindings":lod_bindings,
        "anchor_policy":"weapon-rh-x-forward-y-up-model-space-six-role@1",
        "semantic_scope":"fictional-nonfunctional-game-visual-authoring-only@1",
        "functional_semantics":false,
        "limitations":["no-ballistics","no-damage-or-hitbox","no-physics-binding","no-manufacturing-or-operation","no-commercial-engine-roundtrip"],
        "coordinate_frame":{
            "frame_id":"weapon-right-handed-x-muzzle-y-up-z-right",
            "handedness":"right-handed",
            "units":"meter",
            "forward_axis":"+X",
            "up_axis":"+Y",
            "side_axis":"+Z"
        },
        "transform_convention":"column-vector-parent-world-times-trs-quaternion-xyzw@1",
        "anchors":anchors,
        "part_ids":part_ids,
        "topology_status":"synthetic-root-five-part-bound-metadata-helpers-pass",
        "trs_status":"finite-unit-quaternion-identity-scale-pass",
        "part_bounds_status":"source-part-model-space-anchor-containment-pass",
        "animation_follow_status":animation_follow_status,
        "pivot_status":"not-proven-runtime-pivot",
        "node_materialization":"sidecar-only-not-glb-nodes",
        "runtime_write_performed":true,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only",
        "canonical_sha256":""
    });
    anchor_set = seal_sidecar(anchor_set)?;
    let anchor_bytes =
        canonical_json_bytes(&anchor_set).map_err(|source| error(source.to_string()))?;
    let anchor_sha256 = sha256_hex(&anchor_bytes);
    let request_sha256 = canonical_json_hash(request);
    if let Some(existing) = runtime
        .store
        .get_game_weapon_anchor_link(delivery_manifest_sha256)?
    {
        if existing.project_id != project_id
            || existing.request_sha256 != request_sha256
            || existing.anchor_set_object_sha256 != anchor_sha256
        {
            return invalid("weapon anchor delivery already has a different durable sidecar");
        }
        let existing_anchor_set = read_json(
            runtime,
            &existing.anchor_set_object_sha256,
            "GameWeaponAnchorSet@1",
        )?;
        return Ok(json!({
            "schema_version":"GameWeaponAnchorPrepareResult@1",
            "anchor_set_object_sha256":existing.anchor_set_object_sha256,
            "anchor_set":existing_anchor_set,
            "durable_link":existing,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }));
    }
    let anchor_object = put_json(runtime, &anchor_set, "game-weapon-anchor-set")?;
    let lod0_artifact_sha256 = link
        .get("lod_artifact_sha256s")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .ok_or_else(|| error("weapon anchor delivery LOD0 artifact is unavailable"))?;
    let mut durable_link = GameWeaponAnchorLinkRecord {
        schema_version: "GameWeaponAnchorLink@1".to_owned(),
        project_id: project_id.to_owned(),
        delivery_manifest_object_sha256: delivery_manifest_sha256.to_owned(),
        lod0_artifact_sha256: lod0_artifact_sha256.to_owned(),
        request_sha256,
        anchor_set_object_sha256: anchor_object.record.sha256.clone(),
        materialization_status: "runtime-owned-durable-weapon-anchor-sidecar".to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    durable_link.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&durable_link).map_err(|source| error(source.to_string()))?,
    );
    let durable_link = runtime
        .store
        .record_game_weapon_anchor_link(&durable_link)?;
    Ok(json!({
        "schema_version":"GameWeaponAnchorPrepareResult@1",
        "anchor_set_object_sha256":anchor_object.record.sha256,
        "anchor_set":anchor_set,
        "durable_link":durable_link,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn weapon_anchor_get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
        ],
        "GameWeaponAnchorGetRequest@1",
    )?;
    if text(object, "schema_version")? != "GameWeaponAnchorGetRequest@1" {
        return invalid("weapon anchor get schema version differs");
    }
    let project_id = identifier(object, "project_id")?;
    let delivery_manifest_sha256 = sha(object, "delivery_manifest_object_sha256")?;
    let link = runtime
        .store
        .get_game_weapon_anchor_link(delivery_manifest_sha256)?
        .ok_or_else(|| error("durable weapon anchor sidecar is unavailable"))?;
    if link.project_id != project_id {
        return invalid("durable weapon anchor sidecar belongs to a different project");
    }
    let anchor_set = read_json(
        runtime,
        &link.anchor_set_object_sha256,
        "GameWeaponAnchorSet@1",
    )?;
    exact_object(
        &anchor_set,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "lod_bindings",
            "anchor_policy",
            "semantic_scope",
            "functional_semantics",
            "limitations",
            "coordinate_frame",
            "transform_convention",
            "anchors",
            "part_ids",
            "topology_status",
            "trs_status",
            "part_bounds_status",
            "animation_follow_status",
            "pivot_status",
            "node_materialization",
            "runtime_write_performed",
            "candidate_confirmed",
            "export_performed",
            "actual_engine_roundtrip",
            "quality_status",
            "canonical_sha256",
        ],
        "durable GameWeaponAnchorSet@1",
    )?;
    let delivery = get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_manifest_sha256
        }),
    )?;
    let lod_receipt = delivery
        .get("lod_receipt")
        .and_then(Value::as_object)
        .ok_or_else(|| error("weapon anchor durable LOD receipt is unavailable"))?;
    let levels = lod_receipt
        .get("levels")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error("weapon anchor durable LOD set differs"))?;
    let expected_bindings = levels
        .iter()
        .enumerate()
        .map(|(level, value)| {
            json!({
                "level":level,
                "candidate_id":value.get("candidate_id"),
                "candidate_state_sha256":value.get("candidate_state_sha256"),
                "artifact_sha256":value.get("artifact_sha256"),
                "artifact_readback_sha256":value.get("artifact_readback_sha256")
            })
        })
        .collect::<Vec<_>>();
    let part_ids = lod_receipt
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| error("weapon anchor durable Part inventory is unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| error("weapon anchor durable Part ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lod0_bounds = part_bounds_from_lod(&levels[0])?;
    let validated_anchors =
        validate_weapon_anchors(anchor_set.get("anchors"), &part_ids, &lod0_bounds)?;
    if anchor_set.get("project_id").and_then(Value::as_str) != Some(project_id)
        || anchor_set
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(delivery_manifest_sha256)
        || anchor_set.get("lod_bindings") != Some(&Value::Array(expected_bindings))
        || anchor_set.get("part_ids")
            != Some(&Value::Array(
                part_ids.iter().cloned().map(Value::String).collect(),
            ))
        || anchor_set.get("anchors") != Some(&Value::Array(validated_anchors))
        || anchor_set.get("semantic_scope").and_then(Value::as_str)
            != Some("fictional-nonfunctional-game-visual-authoring-only@1")
        || anchor_set
            .get("functional_semantics")
            .and_then(Value::as_bool)
            != Some(false)
        || levels[0].get("artifact_sha256").and_then(Value::as_str)
            != Some(link.lod0_artifact_sha256.as_str())
    {
        return invalid("durable weapon anchor CAS binding differs");
    }
    Ok(json!({
        "schema_version":"GameWeaponAnchorGetResult@1",
        "link":link,
        "anchor_set":anchor_set,
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "quality_status":"structural_only"
    }))
}

/// Materialize the six closed GameWeaponAnchorSet helpers as empty glTF
/// transform nodes.  This is a derivative CAS operation: the three source
/// delivery GLBs, candidates, versions and confirmations are never changed.
/// The source renderable arrays and BIN are projected byte/semantic exact and
/// only the declared Part children, synthetic root and node/root extras may
/// differ.
pub(super) fn weapon_glb_socket_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "anchor_set_object_sha256",
            "materialization_policy",
            "lod_scope",
            "canonical_sha256",
        ],
        GAME_WEAPON_GLB_SOCKET_PREPARE_SCHEMA,
    )?;
    if text(object, "schema_version")? != GAME_WEAPON_GLB_SOCKET_PREPARE_SCHEMA
        || text(object, "materialization_policy")? != GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_POLICY
        || text(object, "lod_scope")? != GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_LOD_SCOPE
    {
        return invalid("GLB socket materialization policy or LOD scope differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?.to_owned();
    let anchor_sha256 = sha(object, "anchor_set_object_sha256")?.to_owned();
    let socket_key_sha256 = sha(object, "canonical_sha256")?.to_owned();

    if let Some((existing, _existing_lods)) = runtime
        .store
        .get_game_weapon_glb_socket_materialization_link(&socket_key_sha256)?
    {
        if existing.project_id != project_id
            || existing.delivery_manifest_object_sha256 != delivery_sha256
            || existing.anchor_set_object_sha256 != anchor_sha256
            || existing.request_sha256 != socket_key_sha256
        {
            return invalid("GLB socket materialization key is bound to a different request");
        }
        let value = weapon_glb_socket_get_by_key(runtime, &project_id, &socket_key_sha256)?;
        return Ok(json!({
            "schema_version":GAME_WEAPON_GLB_SOCKET_PREPARE_RESULT_SCHEMA,
            "socket_materialization_key_sha256":socket_key_sha256,
            "receipt_object_sha256":value["receipt_object_sha256"],
            "receipt":value["receipt"],
            "durable_link":value["link"],
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }));
    }

    let delivery = get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    let delivery_link: GameAssetDeliveryLinkRecord = serde_json::from_value(
        delivery
            .get("link")
            .cloned()
            .ok_or_else(|| error("GLB socket delivery link is unavailable"))?,
    )
    .map_err(|source| error(format!("GLB socket delivery link is invalid: {source}")))?;
    if delivery_link.project_id != project_id
        || delivery_link.delivery_manifest_object_sha256 != delivery_sha256
        || delivery_link.lod_artifact_sha256s.len() != 3
        || delivery_link.lod_candidate_ids.len() != 3
    {
        return invalid("GLB socket delivery binding is not an exact three-LOD cohort");
    }

    let anchor_result = weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    let anchor_link = anchor_result
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| error("GLB socket AnchorSet link is unavailable"))?;
    if anchor_link
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        != Some(anchor_sha256.as_str())
    {
        return invalid("GLB socket request AnchorSet differs from the durable AnchorSet");
    }
    let anchor_set = anchor_result
        .get("anchor_set")
        .cloned()
        .ok_or_else(|| error("GLB socket AnchorSet is unavailable"))?;
    let anchor_set_canonical_sha256 = anchor_set
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("GLB socket AnchorSet canonical hash is unavailable"))?
        .to_owned();
    let anchor_ids = socket_anchor_ids(&anchor_set)?;
    let socket_node_id_encoding = socket_node_id_encoding_value()?;
    let socket_node_id_encoding_sha256 = socket_node_id_encoding
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("GLB socket node ID encoding hash is unavailable"))?
        .to_owned();
    let levels = delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error("GLB socket delivery LOD receipt is incomplete"))?;

    // Complete every source readback and materialization before opening a CAS
    // reservation.  A malformed source/AnchorSet therefore has zero durable
    // writes; the reservation below is only for the exact three derived GLBs
    // and one receipt.
    let mut prepared_levels = Vec::with_capacity(3);
    for (lod_level, level) in levels.iter().enumerate() {
        let level_declaration = json!({
            "level":level.get("level"),
            "candidate_id":level.get("candidate_id"),
            "candidate_state_sha256":level.get("candidate_state_sha256"),
            "artifact_sha256":level.get("artifact_sha256"),
            "artifact_readback_sha256":level.get("artifact_readback_sha256")
        });
        let loaded = load_level(runtime, &project_id, lod_level as u64, &level_declaration)?;
        if loaded.level != lod_level as u64
            || loaded.artifact_sha256 != delivery_link.lod_artifact_sha256s[lod_level]
            || loaded.candidate_id != delivery_link.lod_candidate_ids[lod_level]
        {
            return invalid("GLB socket source LOD differs from the durable delivery link");
        }
        let source_bytes = runtime.cas_read_bounded(&loaded.artifact_sha256, MAX_GLB_BYTES)?;
        let materialized = materialize_socket_glb(
            &source_bytes,
            &loaded.artifact_sha256,
            &anchor_sha256,
            &anchor_set,
            &loaded.part_ids,
            &anchor_ids,
        )?;
        prepared_levels.push((loaded, materialized));
    }
    if prepared_levels.len() != 3 {
        return invalid("GLB socket materialization did not preflight three LODs");
    }

    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::<CasObject>::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let mut levels = Vec::with_capacity(3);
        for (lod_level, (loaded, materialized)) in prepared_levels.into_iter().enumerate() {
            let derived_object = runtime.store.put_object_reserved(
                &reservation,
                &materialized.glb,
                None,
                "model/gltf-binary",
                GAME_WEAPON_GLB_SOCKET_MATERIALIZED_GLB_KIND,
                &now_string(),
            )?;
            reserved_objects.push(derived_object.clone());
            let readback = socket_readback_value(
                &socket_key_sha256,
                lod_level as u64,
                &loaded,
                &derived_object.record.sha256,
                &materialized,
            )?;
            levels.push(readback);
        }

        if levels.len() != 3 {
            return invalid("GLB socket materialization did not produce three LODs");
        }
        let mut receipt = json!({
            "schema_version":GAME_WEAPON_GLB_SOCKET_RECEIPT_SCHEMA,
            "socket_materialization_key_sha256":socket_key_sha256,
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256,
            "anchor_set_object_sha256":anchor_sha256,
            "anchor_set_canonical_sha256":anchor_set_canonical_sha256,
            "request_sha256":socket_key_sha256,
            "socket_materialization_policy":GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_POLICY,
            "lod_scope":GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_LOD_SCOPE,
            "socket_node_id_encoding_sha256":socket_node_id_encoding_sha256,
            "levels":levels,
            "semantic_scope":"fictional-nonfunctional-game-visual-authoring-only@1",
            "functional_semantics":false,
            "materialization_status":GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_STATUS,
            "runtime_write_performed":true,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only",
            "limitations":[
                "no-ballistics",
                "no-damage-or-hitbox",
                "no-physics-binding",
                "no-manufacturing-or-operation",
                "no-commercial-engine-roundtrip",
                "no-runtime-pivot-proof",
                "no-visual-quality-pass"
            ],
            "created_at":now_string(),
            "canonical_sha256":""
        });
        receipt = seal_sidecar(receipt)?;
        let receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&receipt).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_RECEIPT_KIND,
            &now_string(),
        )?;
        reserved_objects.push(receipt_object.clone());

        let mut parent = GameWeaponGlbSocketMaterializationLinkRecord {
            schema_version: "GameWeaponGlbSocketMaterializationLink@1".to_owned(),
            socket_materialization_key_sha256: socket_key_sha256.clone(),
            project_id: project_id.clone(),
            delivery_manifest_object_sha256: delivery_sha256.clone(),
            anchor_set_object_sha256: anchor_sha256.clone(),
            anchor_set_canonical_sha256: anchor_set_canonical_sha256.clone(),
            request_sha256: socket_key_sha256.clone(),
            socket_materialization_policy: GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_POLICY.to_owned(),
            lod_scope: GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_LOD_SCOPE.to_owned(),
            socket_node_id_encoding_sha256: socket_node_id_encoding_sha256.clone(),
            receipt_object_sha256: receipt_object.record.sha256.clone(),
            materialization_status: GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        parent.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&parent).map_err(|source| error(source.to_string()))?,
        );
        let mut children = Vec::with_capacity(3);
        for lod in receipt
            .get("levels")
            .and_then(Value::as_array)
            .ok_or_else(|| error("GLB socket receipt levels are unavailable"))?
        {
            let mut child = GameWeaponGlbSocketMaterializationLodRecord {
                schema_version: "GameWeaponGlbSocketMaterializationLod@1".to_owned(),
                socket_materialization_key_sha256: socket_key_sha256.clone(),
                lod_level: lod["lod_level"]
                    .as_u64()
                    .ok_or_else(|| error("GLB socket child LOD level is invalid"))?,
                source_candidate_id: lod["source_candidate_id"]
                    .as_str()
                    .ok_or_else(|| error("GLB socket child source candidate is invalid"))?
                    .to_owned(),
                source_candidate_state_sha256: lod["source_candidate_state_sha256"]
                    .as_str()
                    .ok_or_else(|| error("GLB socket child source candidate state is invalid"))?
                    .to_owned(),
                source_artifact_sha256: lod["source_artifact_sha256"]
                    .as_str()
                    .ok_or_else(|| error("GLB socket child source artifact is invalid"))?
                    .to_owned(),
                source_artifact_readback_sha256: lod["source_artifact_readback_sha256"]
                    .as_str()
                    .ok_or_else(|| error("GLB socket child source readback is invalid"))?
                    .to_owned(),
                derived_artifact_sha256: lod["derived_artifact_sha256"]
                    .as_str()
                    .ok_or_else(|| error("GLB socket child derived artifact is invalid"))?
                    .to_owned(),
                derived_artifact_readback_sha256: lod["derived_artifact_readback_sha256"]
                    .as_str()
                    .ok_or_else(|| error("GLB socket child derived readback is invalid"))?
                    .to_owned(),
                source_renderable_inventory_sha256: lod["source_renderable_inventory_sha256"]
                    .as_str()
                    .ok_or_else(|| error("GLB socket child source inventory is invalid"))?
                    .to_owned(),
                derived_renderable_inventory_sha256: lod["derived_renderable_inventory_sha256"]
                    .as_str()
                    .ok_or_else(|| error("GLB socket child derived inventory is invalid"))?
                    .to_owned(),
                socket_node_inventory_sha256: lod["socket_node_inventory_sha256"]
                    .as_str()
                    .ok_or_else(|| error("GLB socket child node inventory is invalid"))?
                    .to_owned(),
                canonical_sha256: String::new(),
                created_at: now_string(),
            };
            child.canonical_sha256 = canonical_json_hash(
                &serde_json::to_value(&child).map_err(|source| error(source.to_string()))?,
            );
            children.push(child);
        }
        let (durable_parent, _durable_children) = runtime
            .store
            .record_game_weapon_glb_socket_materialization_link(&parent, &children)?;
        Ok(json!({
            "schema_version":GAME_WEAPON_GLB_SOCKET_PREPARE_RESULT_SCHEMA,
            "socket_materialization_key_sha256":socket_key_sha256,
            "receipt_object_sha256":receipt_object.record.sha256,
            "receipt":receipt,
            "durable_link":durable_parent,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }))
    })();
    match operation {
        Ok(value) => {
            for object in &reserved_objects {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(operation_error) => {
            let mut rollback_error = None;
            for object in reserved_objects.iter().rev() {
                if let Err(error) =
                    runtime
                        .store
                        .release_cas_reservation_object(&reservation, object, true)
                {
                    rollback_error = Some(error.to_string());
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(error(format!(
                    "GLB socket materialization failed ({operation_error}); reservation rollback also failed ({rollback_error})"
                )));
            }
            Err(operation_error)
        }
    }
}

pub(super) fn weapon_glb_socket_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "socket_materialization_key_sha256",
        ],
        GAME_WEAPON_GLB_SOCKET_GET_SCHEMA,
    )?;
    if text(object, "schema_version")? != GAME_WEAPON_GLB_SOCKET_GET_SCHEMA {
        return invalid("GLB socket materialization get schema differs");
    }
    let project_id = identifier(object, "project_id")?;
    let key = sha(object, "socket_materialization_key_sha256")?;
    weapon_glb_socket_get_by_key(runtime, project_id, key)
}

fn weapon_glb_socket_get_by_key(
    runtime: &Runtime,
    project_id: &str,
    socket_key_sha256: &str,
) -> Result<Value, RuntimeError> {
    let (parent, children) = runtime
        .store
        .get_game_weapon_glb_socket_materialization_link(socket_key_sha256)?
        .ok_or_else(|| error("durable GLB socket materialization is unavailable"))?;
    if parent.project_id != project_id
        || parent.socket_materialization_key_sha256 != socket_key_sha256
        || parent.request_sha256 != socket_key_sha256
        || parent.socket_materialization_policy != GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_POLICY
        || parent.lod_scope != GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_LOD_SCOPE
        || parent.materialization_status != GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_STATUS
    {
        return invalid("durable GLB socket parent binding differs");
    }
    let receipt = read_json(
        runtime,
        &parent.receipt_object_sha256,
        GAME_WEAPON_GLB_SOCKET_RECEIPT_SCHEMA,
    )?;
    let delivery = get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":parent.delivery_manifest_object_sha256
        }),
    )?;
    let anchor_result = weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":parent.delivery_manifest_object_sha256
        }),
    )?;
    let anchor_set = anchor_result
        .get("anchor_set")
        .cloned()
        .ok_or_else(|| error("durable GLB socket AnchorSet is unavailable"))?;
    if anchor_result
        .get("link")
        .and_then(|value| value.get("anchor_set_object_sha256"))
        .and_then(Value::as_str)
        != Some(parent.anchor_set_object_sha256.as_str())
    {
        return invalid("durable GLB socket AnchorSet link differs");
    }
    let anchor_ids = socket_anchor_ids(&anchor_set)?;
    let socket_node_id_encoding = socket_node_id_encoding_value()?;
    let socket_node_id_encoding_sha256 = socket_node_id_encoding
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("durable GLB socket node ID encoding hash is unavailable"))?;
    if parent.socket_node_id_encoding_sha256 != socket_node_id_encoding_sha256
        || receipt
            .get("socket_node_id_encoding_sha256")
            .and_then(Value::as_str)
            != Some(socket_node_id_encoding_sha256)
        || receipt
            .get("anchor_set_canonical_sha256")
            .and_then(Value::as_str)
            != anchor_set.get("canonical_sha256").and_then(Value::as_str)
    {
        return invalid("durable GLB socket node encoding or AnchorSet canonical binding differs");
    }
    let levels = receipt
        .get("levels")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error("durable GLB socket receipt does not contain three LODs"))?;
    if children.len() != 3 {
        return invalid("durable GLB socket child inventory is not exactly three LODs");
    }
    let delivery_link: GameAssetDeliveryLinkRecord = serde_json::from_value(
        delivery
            .get("link")
            .cloned()
            .ok_or_else(|| error("durable GLB socket delivery link is unavailable"))?,
    )
    .map_err(|source| {
        error(format!(
            "durable GLB socket delivery link is invalid: {source}"
        ))
    })?;
    if delivery_link.project_id != project_id
        || delivery_link.delivery_manifest_object_sha256 != parent.delivery_manifest_object_sha256
        || delivery_link.lod_artifact_sha256s.len() != 3
        || delivery_link.lod_candidate_ids.len() != 3
    {
        return invalid("durable GLB socket delivery cohort differs");
    }
    let mut verified_lods = Vec::with_capacity(3);
    for lod_level in 0..3usize {
        let child = children
            .iter()
            .find(|child| child.lod_level == lod_level as u64)
            .ok_or_else(|| error("durable GLB socket child LOD is missing"))?;
        let receipt_lod = levels
            .iter()
            .find(|value| value.get("lod_level").and_then(Value::as_u64) == Some(lod_level as u64))
            .ok_or_else(|| error("durable GLB socket receipt LOD is missing"))?;
        let level = delivery
            .get("lod_receipt")
            .and_then(|value| value.get("levels"))
            .and_then(Value::as_array)
            .and_then(|values| values.get(lod_level))
            .ok_or_else(|| error("durable GLB socket source LOD receipt is missing"))?;
        let loaded = load_level(
            runtime,
            project_id,
            lod_level as u64,
            &json!({
                "level":level.get("level"),
                "candidate_id":level.get("candidate_id"),
                "candidate_state_sha256":level.get("candidate_state_sha256"),
                "artifact_sha256":level.get("artifact_sha256"),
                "artifact_readback_sha256":level.get("artifact_readback_sha256")
            }),
        )?;
        if loaded.artifact_sha256 != delivery_link.lod_artifact_sha256s[lod_level]
            || child.source_artifact_sha256 != loaded.artifact_sha256
            || child.source_candidate_id != loaded.candidate_id
            || child.source_candidate_state_sha256 != loaded.candidate_state_sha256
            || child.source_artifact_readback_sha256 != loaded.artifact_readback_sha256
        {
            return invalid("durable GLB socket source child binding differs");
        }
        let source_bytes = runtime.cas_read_bounded(&loaded.artifact_sha256, MAX_GLB_BYTES)?;
        let materialized = materialize_socket_glb(
            &source_bytes,
            &loaded.artifact_sha256,
            &parent.anchor_set_object_sha256,
            &anchor_set,
            &loaded.part_ids,
            &anchor_ids,
        )?;
        let derived_record = runtime
            .store
            .get_object(&child.derived_artifact_sha256)?
            .ok_or_else(|| error("durable GLB socket derived object metadata is unavailable"))?;
        if derived_record.kind != GAME_WEAPON_GLB_SOCKET_MATERIALIZED_GLB_KIND
            || derived_record.mime != "model/gltf-binary"
            || derived_record.size_bytes == 0
            || derived_record.size_bytes > MAX_GLB_BYTES
        {
            return invalid("durable GLB socket derived object metadata differs");
        }
        let derived_bytes =
            runtime.cas_read_bounded(&child.derived_artifact_sha256, MAX_GLB_BYTES)?;
        if sha256_hex(&derived_bytes) != child.derived_artifact_sha256
            || derived_bytes != materialized.glb
            || child.derived_renderable_inventory_sha256
                != materialized.derived_renderable_inventory_sha256
            || child.source_renderable_inventory_sha256
                != materialized.source_renderable_inventory_sha256
            || child.socket_node_inventory_sha256 != materialized.socket_node_inventory_sha256
        {
            return invalid("durable GLB socket derived GLB or inventory differs");
        }
        let expected_readback = socket_readback_value(
            socket_key_sha256,
            lod_level as u64,
            &loaded,
            &child.derived_artifact_sha256,
            &materialized,
        )?;
        if child.derived_artifact_readback_sha256
            != expected_readback["derived_artifact_readback_sha256"]
                .as_str()
                .ok_or_else(|| error("durable GLB socket readback hash is invalid"))?
            || receipt_lod != &expected_readback
        {
            return invalid("durable GLB socket inline derived readback differs");
        }
        verified_lods.push(expected_readback);
    }
    validate_socket_receipt(
        &receipt,
        socket_key_sha256,
        project_id,
        &parent,
        &anchor_ids,
        &verified_lods,
    )?;
    Ok(json!({
        "schema_version":GAME_WEAPON_GLB_SOCKET_GET_RESULT_SCHEMA,
        "socket_materialization_key_sha256":socket_key_sha256,
        "receipt_object_sha256":parent.receipt_object_sha256,
        "receipt":receipt,
        "link":parent,
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn energy_vfx_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "anchor_set_object_sha256",
            "material_pack_id",
            "material_pack_manifest_sha256",
            "vfx_policy",
            "effects",
            "canonical_sha256",
        ],
        "FictionalEnergyVfxPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxPrepareRequest@1"
        || text(object, "vfx_policy")?
            != "fictional-energy-two-effect-time-sampled-emissive-intent@1"
        || text(object, "material_pack_id")? != "forgecad-fictional-energy-weapon-2k"
    {
        return invalid("fictional energy VFX policy or MaterialPack differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?;
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?;
    let anchor_sha256 = sha(object, "anchor_set_object_sha256")?;
    let material_sha256 = sha(object, "material_pack_manifest_sha256")?;
    let anchor = weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    if anchor
        .get("link")
        .and_then(|value| value.get("anchor_set_object_sha256"))
        .and_then(Value::as_str)
        != Some(anchor_sha256)
    {
        return invalid("fictional energy VFX AnchorSet binding differs");
    }
    let material_pack = material_pack_manifest_by_id("forgecad-fictional-energy-weapon-2k")
        .ok_or_else(|| error("fictional energy weapon 2K MaterialPack is unavailable"))?;
    if material_pack
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(material_sha256)
        || !material_pack
            .get("material_definitions")
            .and_then(Value::as_array)
            .is_some_and(|values| {
                ["energy-cyan-muzzle-emissive", "energy-cyan-emissive"]
                    .iter()
                    .all(|material_id| {
                        values.iter().any(|value| {
                            value.get("material_id").and_then(Value::as_str) == Some(*material_id)
                                && value.get("emissive_strength").and_then(Value::as_f64)
                                    == Some(6.0)
                                && value.get("texture_set_id") == Some(&Value::Null)
                        })
                    })
            })
    {
        return invalid("fictional energy VFX MaterialPack binding differs");
    }
    let effects = validate_energy_vfx_effects(object.get("effects"))?;
    let mut profile = json!({
        "schema_version":"FictionalEnergyVfxProfile@1",
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_sha256,
        "anchor_set_object_sha256":anchor_sha256,
        "material_pack_id":"forgecad-fictional-energy-weapon-2k",
        "material_pack_manifest_sha256":material_sha256,
        "vfx_policy":"fictional-energy-two-effect-time-sampled-emissive-intent@1",
        "semantic_scope":"fictional-nonfunctional-game-visual-authoring-only@1",
        "functional_semantics":false,
        "timebase_hz":1000,
        "effects":effects,
        "static_emissive_material_definition_verified":true,
        "execution_mode":"typed-time-sampled-emissive-intent-no-material-animation-render@1",
        "emissive_animation_rendered":false,
        "bloom_rendered":false,
        "particles_rendered":false,
        "trails_rendered":false,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only",
        "canonical_sha256":""
    });
    profile = seal_sidecar(profile)?;
    let profile_bytes =
        canonical_json_bytes(&profile).map_err(|source| error(source.to_string()))?;
    let profile_sha256 = sha256_hex(&profile_bytes);
    let request_sha256 = canonical_json_hash(request);
    if let Some(existing) = runtime
        .store
        .get_fictional_energy_vfx_link(delivery_sha256)?
    {
        if existing.project_id != project_id
            || existing.anchor_set_object_sha256 != anchor_sha256
            || existing.material_pack_manifest_sha256 != material_sha256
            || existing.request_sha256 != request_sha256
            || existing.vfx_profile_object_sha256 != profile_sha256
        {
            return invalid("fictional energy VFX delivery already has a different profile");
        }
        let existing_profile = read_json(
            runtime,
            &existing.vfx_profile_object_sha256,
            "FictionalEnergyVfxProfile@1",
        )?;
        return Ok(json!({
            "schema_version":"FictionalEnergyVfxPrepareResult@1",
            "vfx_profile_object_sha256":existing.vfx_profile_object_sha256,
            "vfx_profile":existing_profile,
            "durable_link":existing,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }));
    }
    let profile_object = put_json(runtime, &profile, "fictional-energy-vfx-profile")?;
    let mut link = FictionalEnergyVfxLinkRecord {
        schema_version: "FictionalEnergyVfxLink@1".to_owned(),
        project_id: project_id.to_owned(),
        delivery_manifest_object_sha256: delivery_sha256.to_owned(),
        anchor_set_object_sha256: anchor_sha256.to_owned(),
        material_pack_manifest_sha256: material_sha256.to_owned(),
        request_sha256,
        vfx_profile_object_sha256: profile_object.record.sha256.clone(),
        materialization_status: "runtime-owned-durable-fictional-energy-vfx-profile".to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    link.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&link).map_err(|source| error(source.to_string()))?,
    );
    let link = match runtime.store.record_fictional_energy_vfx_link(&link) {
        Ok(link) => link,
        Err(commit_error) => {
            if let Err(rollback_error) = runtime
                .store
                .discard_new_temporary_fictional_energy_vfx_profile(&profile_object)
            {
                return Err(error(format!(
                    "fictional energy VFX link commit failed ({commit_error}); temporary CAS rollback also failed ({rollback_error})"
                )));
            }
            return Err(commit_error.into());
        }
    };
    Ok(json!({
        "schema_version":"FictionalEnergyVfxPrepareResult@1",
        "vfx_profile_object_sha256":profile_object.record.sha256,
        "vfx_profile":profile,
        "durable_link":link,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn energy_vfx_get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
        ],
        "FictionalEnergyVfxGetRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxGetRequest@1" {
        return invalid("fictional energy VFX get schema version differs");
    }
    let project_id = identifier(object, "project_id")?;
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?;
    let link = runtime
        .store
        .get_fictional_energy_vfx_link(delivery_sha256)?
        .ok_or_else(|| error("durable fictional energy VFX profile is unavailable"))?;
    if link.project_id != project_id {
        return invalid("durable fictional energy VFX profile belongs to a different project");
    }
    let anchor = weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    let material_pack = material_pack_manifest_by_id("forgecad-fictional-energy-weapon-2k")
        .ok_or_else(|| error("fictional energy weapon 2K MaterialPack is unavailable"))?;
    let profile = read_json(
        runtime,
        &link.vfx_profile_object_sha256,
        "FictionalEnergyVfxProfile@1",
    )?;
    let effects = validate_energy_vfx_effects(profile.get("effects"))?;
    if profile.get("project_id").and_then(Value::as_str) != Some(project_id)
        || profile
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(delivery_sha256)
        || profile
            .get("anchor_set_object_sha256")
            .and_then(Value::as_str)
            != Some(link.anchor_set_object_sha256.as_str())
        || anchor
            .get("link")
            .and_then(|value| value.get("anchor_set_object_sha256"))
            .and_then(Value::as_str)
            != Some(link.anchor_set_object_sha256.as_str())
        || profile
            .get("material_pack_manifest_sha256")
            .and_then(Value::as_str)
            != Some(link.material_pack_manifest_sha256.as_str())
        || material_pack
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(link.material_pack_manifest_sha256.as_str())
        || profile.get("effects") != Some(&Value::Array(effects))
        || profile.get("semantic_scope").and_then(Value::as_str)
            != Some("fictional-nonfunctional-game-visual-authoring-only@1")
        || profile.get("functional_semantics").and_then(Value::as_bool) != Some(false)
        || profile.get("timebase_hz").and_then(Value::as_u64) != Some(1000)
        || profile
            .get("static_emissive_material_definition_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || profile.get("execution_mode").and_then(Value::as_str)
            != Some("typed-time-sampled-emissive-intent-no-material-animation-render@1")
        || profile
            .get("emissive_animation_rendered")
            .and_then(Value::as_bool)
            != Some(false)
        || profile
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
        || profile.get("bloom_rendered").and_then(Value::as_bool) != Some(false)
        || profile.get("particles_rendered").and_then(Value::as_bool) != Some(false)
        || profile.get("trails_rendered").and_then(Value::as_bool) != Some(false)
        || profile.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || profile.get("export_performed").and_then(Value::as_bool) != Some(false)
        || profile.get("quality_status").and_then(Value::as_str) != Some("structural_only")
    {
        return invalid("durable fictional energy VFX binding differs");
    }
    Ok(json!({
        "schema_version":"FictionalEnergyVfxGetResult@1",
        "link":link,
        "vfx_profile":profile,
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn energy_vfx_frame_sample(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "vfx_profile_object_sha256",
            "sample_time_ticks",
            "sampling_policy",
            "canonical_sha256",
        ],
        "FictionalEnergyVfxFrameSampleRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxFrameSampleRequest@1"
        || text(object, "sampling_policy")?
            != "integer-tick-linear-once-clamp-loop-modulo-duration@1"
    {
        return invalid("fictional energy VFX frame sampling policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?;
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?;
    let profile_sha256 = sha(object, "vfx_profile_object_sha256")?;
    let requested_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| error("fictional energy VFX frame sample time is invalid"))?;
    let durable = energy_vfx_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    if durable
        .get("link")
        .and_then(|value| value.get("vfx_profile_object_sha256"))
        .and_then(Value::as_str)
        != Some(profile_sha256)
    {
        return invalid("fictional energy VFX frame profile binding differs");
    }
    let profile = durable
        .get("vfx_profile")
        .ok_or_else(|| error("durable fictional energy VFX profile is unavailable"))?;
    let effects = validate_energy_vfx_effects(profile.get("effects"))?
        .iter()
        .map(|effect| sample_energy_vfx_effect(effect, requested_time_ticks))
        .collect::<Result<Vec<_>, _>>()?;
    seal_sidecar(json!({
        "schema_version":"FictionalEnergyVfxFrameSample@1",
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_sha256,
        "vfx_profile_object_sha256":profile_sha256,
        "timebase_hz":1000,
        "requested_time_ticks":requested_time_ticks,
        "sampling_policy":"integer-tick-linear-once-clamp-loop-modulo-duration@1",
        "interpolation":"LINEAR",
        "effects":effects,
        "glb_material_zone_binding_verified":false,
        "emissive_animation_rendered":false,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "quality_status":"structural_only",
        "limitations":[
            "sampled-emissive-intent-only",
            "no-appearance-or-glb-material-zone-binding",
            "no-render-or-cas-frame-receipt",
            "anchor-remains-sidecar-not-glb-socket",
            "no-bloom-particles-trails-or-engine-roundtrip",
            "structural-sampling-does-not-prove-visual-quality"
        ],
        "canonical_sha256":""
    }))
}

pub(super) fn energy_vfx_appearance_frame_sample(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "vfx_profile_object_sha256",
            "sample_time_ticks",
            "sampling_policy",
            "appearance_binding_policy",
            "canonical_sha256",
        ],
        "FictionalEnergyVfxAppearanceFrameSampleRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxAppearanceFrameSampleRequest@1"
        || text(object, "sampling_policy")?
            != "integer-tick-linear-once-clamp-loop-modulo-duration@1"
        || text(object, "appearance_binding_policy")?
            != "three-lod-appearance-program-glb-material-zone-stable-id@1"
    {
        return invalid("fictional energy VFX Appearance frame policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?;
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?;
    let profile_sha256 = sha(object, "vfx_profile_object_sha256")?;
    let sample_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| error("fictional energy VFX Appearance frame time is invalid"))?;
    let sampled_request = seal_request(json!({
        "schema_version":"FictionalEnergyVfxFrameSampleRequest@1",
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_sha256,
        "vfx_profile_object_sha256":profile_sha256,
        "sample_time_ticks":sample_time_ticks,
        "sampling_policy":"integer-tick-linear-once-clamp-loop-modulo-duration@1",
        "canonical_sha256":""
    }))?;
    let sampled = energy_vfx_frame_sample(runtime, &sampled_request)?;
    let durable = energy_vfx_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    let profile = durable
        .get("vfx_profile")
        .ok_or_else(|| error("durable fictional energy VFX profile is unavailable"))?;
    let effects = sampled
        .get("effects")
        .and_then(Value::as_array)
        .ok_or_else(|| error("sampled fictional energy VFX effects are unavailable"))?;
    let lod_appearance_bindings = verify_energy_vfx_appearance_binding(
        runtime,
        project_id,
        delivery_sha256,
        profile,
        effects,
    )?
    .ok_or_else(|| error("APPEARANCE_PROVENANCE_UNAVAILABLE"))?;
    seal_sidecar(json!({
        "schema_version":"FictionalEnergyVfxAppearanceFrameSample@1",
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_sha256,
        "vfx_profile_object_sha256":profile_sha256,
        "timebase_hz":1000,
        "requested_time_ticks":sample_time_ticks,
        "sampling_policy":"integer-tick-linear-once-clamp-loop-modulo-duration@1",
        "appearance_binding_policy":"three-lod-appearance-program-glb-material-zone-stable-id@1",
        "interpolation":"LINEAR",
        "effects":effects,
        "lod_appearance_bindings":lod_appearance_bindings,
        "glb_material_zone_binding_verified":true,
        "emissive_animation_rendered":false,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "quality_status":"structural_only",
        "limitations":[
            "appearance-and-glb-material-binding-verified-no-render",
            "no-runtime-cas-frame-receipt",
            "anchor-remains-sidecar-not-glb-socket",
            "no-bloom-particles-trails-or-engine-roundtrip",
            "structural-binding-does-not-prove-visual-quality"
        ],
        "canonical_sha256":""
    }))
}

pub(super) fn energy_vfx_rendered_frame_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "vfx_profile_object_sha256",
            "sample_time_ticks",
            "sampling_policy",
            "appearance_binding_policy",
            "effect_materialization_policy",
            "lod_level",
            "camera_policy",
            "render_policy",
            "canonical_sha256",
        ],
        "FictionalEnergyVfxFrameRenderPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxFrameRenderPrepareRequest@1"
        || text(object, "sampling_policy")?
            != "integer-tick-linear-once-clamp-loop-modulo-duration@1"
        || text(object, "appearance_binding_policy")?
            != "three-lod-appearance-program-glb-material-zone-stable-id@1"
        || text(object, "effect_materialization_policy")? != "independent-effect-material-zone@1"
        || object.get("lod_level").and_then(Value::as_u64) != Some(0)
        || text(object, "camera_policy")? != "runtime-fixed-default-camera-calibration@1"
        || text(object, "render_policy")?
            != "lod0-nine-aov-double-worker-byte-exact-reservation-safe@1"
    {
        return invalid("fictional energy VFX rendered frame policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?.to_owned();
    let profile_sha256 = sha(object, "vfx_profile_object_sha256")?.to_owned();
    let sample_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| error("fictional energy VFX rendered frame time is invalid"))?;
    let frame_key_sha256 = sha(object, "canonical_sha256")?.to_owned();
    if runtime
        .store
        .get_fictional_energy_vfx_frame_link(&frame_key_sha256)?
        .is_some()
    {
        return energy_vfx_rendered_frame_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
                "project_id":project_id,
                "frame_key_sha256":frame_key_sha256
            }),
        )
        .map(|value| {
            json!({
                "schema_version":"FictionalEnergyVfxRenderedFramePrepareResult@1",
                "frame_key_sha256":value["frame_key_sha256"],
                "receipt_object_sha256":value["receipt_object_sha256"],
                "receipt":value["receipt"],
                "durable_link":value["link"],
                "candidate_confirmed":false,
                "export_performed":false,
                "actual_engine_roundtrip":false,
                "quality_status":"structural_only"
            })
        });
    }
    let sample_request = seal_request(json!({
        "schema_version":"FictionalEnergyVfxAppearanceFrameSampleRequest@1",
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_sha256,
        "vfx_profile_object_sha256":profile_sha256,
        "sample_time_ticks":sample_time_ticks,
        "sampling_policy":"integer-tick-linear-once-clamp-loop-modulo-duration@1",
        "appearance_binding_policy":"three-lod-appearance-program-glb-material-zone-stable-id@1",
        "canonical_sha256":""
    }))?;
    let sample_request_sha256 = sample_request["canonical_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX sample request hash is unavailable"))?
        .to_owned();
    let sample = energy_vfx_appearance_frame_sample(runtime, &sample_request)?;
    let sample_result_sha256 = sample["canonical_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX sample result hash is unavailable"))?
        .to_owned();
    let lod0 = sample
        .get("lod_appearance_bindings")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .ok_or_else(|| error("fictional energy VFX LOD0 Appearance binding is unavailable"))?;
    let source_candidate_id = identifier(
        lod0.as_object()
            .ok_or_else(|| error("fictional energy VFX LOD0 binding is invalid"))?,
        "candidate_id",
    )?
    .to_owned();
    let source_artifact_sha256 = lod0["artifact_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX LOD0 artifact hash is unavailable"))?
        .to_owned();
    let source_artifact_readback_sha256 = lod0["artifact_readback_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX LOD0 readback hash is unavailable"))?
        .to_owned();
    let source_readback = runtime.artifact_readback_bounded(
        &source_artifact_sha256,
        &source_candidate_id,
        MAX_GLB_BYTES,
    )?;
    if source_readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(source_artifact_readback_sha256.as_str())
    {
        return invalid("fictional energy VFX LOD0 readback changed before rendering");
    }
    let source_program_sha256 = source_readback
        .get("program_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("fictional energy VFX LOD0 program hash is unavailable"))?
        .to_owned();
    let glb = runtime.cas_read(&source_artifact_sha256)?;
    let zone_bindings = lod0
        .get("zone_material_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| error("fictional energy VFX LOD0 material bindings are unavailable"))?;
    let effects = sample
        .get("effects")
        .and_then(Value::as_array)
        .ok_or_else(|| error("fictional energy VFX sampled effects are unavailable"))?;
    let known_zone_materials = zone_bindings
        .iter()
        .filter_map(|binding| {
            Some((
                binding.get("material_zone_id")?.as_str()?.to_owned(),
                binding.get("material_id")?.as_str()?.to_owned(),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let effect_bindings = lod0
        .get("effect_material_zone_bindings")
        .and_then(Value::as_array)
        .filter(|values| values.len() == effects.len())
        .ok_or_else(|| error("fictional energy VFX LOD0 effect bindings are unavailable"))?;
    let by_effect = effect_bindings
        .iter()
        .filter_map(|binding| Some((binding.get("effect_id")?.as_str()?.to_owned(), binding)))
        .collect::<BTreeMap<_, _>>();
    if by_effect.len() != effects.len() {
        return invalid("fictional energy VFX effect bindings contain duplicate identities");
    }
    let mut used_zones = BTreeSet::new();
    let overrides = effects
        .iter()
        .map(
            |effect| -> Result<render_worker::EmissiveMaterialOverride, RuntimeError> {
                let effect_id =
                    effect
                        .get("effect_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            error("fictional energy VFX sampled effect ID is unavailable")
                        })?;
                let material_id = effect
                    .get("material_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| error("fictional energy VFX sampled material is unavailable"))?;
                let binding = by_effect.get(effect_id).ok_or_else(|| {
                    error("fictional energy VFX sampled effect has no LOD0 binding")
                })?;
                let material_zone_id = binding
                    .get("material_zone_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        error("fictional energy VFX effect MaterialZone is unavailable")
                    })?;
                if binding.get("material_id").and_then(Value::as_str) != Some(material_id)
                    || known_zone_materials
                        .get(material_zone_id)
                        .map(String::as_str)
                        != Some(material_id)
                    || !used_zones.insert(material_zone_id.to_owned())
                {
                    return invalid(
                        "fictional energy VFX effects must resolve to distinct exact MaterialZones",
                    );
                }
                let color = effect
                    .get("color_linear_rgb")
                    .and_then(Value::as_array)
                    .filter(|values| values.len() == 3)
                    .ok_or_else(|| error("fictional energy VFX sampled color is unavailable"))?;
                let color_linear_rgb = [
                    color[0]
                        .as_f64()
                        .ok_or_else(|| error("VFX color is invalid"))? as f32,
                    color[1]
                        .as_f64()
                        .ok_or_else(|| error("VFX color is invalid"))? as f32,
                    color[2]
                        .as_f64()
                        .ok_or_else(|| error("VFX color is invalid"))? as f32,
                ];
                let emissive_strength = effect
                    .get("emissive_strength")
                    .and_then(Value::as_f64)
                    .ok_or_else(|| error("fictional energy VFX sampled strength is unavailable"))?;
                Ok(render_worker::EmissiveMaterialOverride {
                    material_zone_id: material_zone_id.to_owned(),
                    material_id: material_id.to_owned(),
                    color_linear_rgb,
                    emissive_strength: emissive_strength as f32,
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let camera = default_camera_calibration();
    let camera_identity_sha256 = camera_identity_hash(&camera)?;
    let first = render_worker::render_glb_vfx_frame_with_worker_identity(&glb, &camera, &overrides)
        .map_err(|source| {
            error(format!(
                "fictional energy VFX frame render failed: {source}"
            ))
        })?;
    let second =
        render_worker::render_glb_vfx_frame_with_worker_identity(&glb, &camera, &overrides)
            .map_err(|source| {
                error(format!(
                    "fictional energy VFX frame replay failed: {source}"
                ))
            })?;
    if first.build_cohort_sha256.is_none()
        || first.build_cohort_sha256 != second.build_cohort_sha256
        || first.render_profile != second.render_profile
        || first.applied_emissive_overrides.len() != second.applied_emissive_overrides.len()
        || first.applied_emissive_overrides.len() != overrides.len()
        || first
            .applied_emissive_overrides
            .iter()
            .zip(&overrides)
            .any(|(actual, requested)| {
                actual.material_zone_id != requested.material_zone_id
                    || actual.material_id != requested.material_id
            })
        || first.passes.len() != 9
        || first
            .passes
            .iter()
            .zip(&second.passes)
            .any(|(left, right)| {
                left.pass != right.pass
                    || left.png != right.png
                    || left.width != 512
                    || left.height != 512
            })
    {
        return invalid("fictional energy VFX frame replay or Worker cohort differs");
    }
    let worker_cohort = first
        .build_cohort_sha256
        .clone()
        .ok_or_else(|| error("fictional energy VFX Render Worker cohort is unavailable"))?;
    let render_profile_sha256 = first.render_profile["canonical_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX render profile hash is unavailable"))?
        .to_owned();
    let render_replay_sha256 = canonical_json_hash(&json!(first
        .passes
        .iter()
        .map(|pass| json!({"pass":pass.pass,"sha256":sha256_hex(&pass.png)}))
        .collect::<Vec<_>>()));
    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::<CasObject>::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let camera_bytes =
            canonical_json_bytes(&camera).map_err(|source| error(source.to_string()))?;
        let camera_object = runtime.store.put_object_reserved(
            &reservation,
            &camera_bytes,
            None,
            "application/json",
            "camera-calibration",
            &now_string(),
        )?;
        reserved_objects.push(camera_object.clone());
        let mut pass_artifacts = Map::new();
        let mut receipt_passes = Vec::new();
        let mut pass_hashes = Vec::new();
        for (ordinal, pass) in first.passes.iter().enumerate() {
            let pass_object = runtime.store.put_object_reserved(
                &reservation,
                &pass.png,
                None,
                "image/png",
                &format!("appearance-v2-render-{}", pass.pass),
                &now_string(),
            )?;
            let color_space = if pass.pass == "beauty" {
                "srgb"
            } else {
                "data"
            };
            let metadata = json!({
                "sha256":pass_object.record.sha256,
                "mime":"image/png",
                "size_bytes":pass_object.record.size_bytes,
                "width":512,
                "height":512,
                "channels":"rgba8",
                "color_space":color_space
            });
            pass_artifacts.insert(pass.pass.clone(), metadata.clone());
            receipt_passes.push(json!({
                "pass":pass.pass,
                "ordinal":ordinal,
                "sha256":metadata["sha256"],
                "mime":"image/png",
                "size_bytes":metadata["size_bytes"],
                "width":512,
                "height":512,
                "channels":"rgba8",
                "color_space":color_space
            }));
            pass_hashes.push(pass_object.record.sha256.clone());
            reserved_objects.push(pass_object);
        }
        let mut render_set = json!({
            "schema_version":"FictionalEnergyVfxRenderSet@1",
            "render_set_id":format!("vfx-frame-{}", &frame_key_sha256[..32]),
            "frame_key_sha256":frame_key_sha256,
            "candidate_id":source_candidate_id,
            "artifact_sha256":source_artifact_sha256,
            "program_sha256":source_program_sha256,
            "effect_materialization_policy":"independent-effect-material-zone@1",
            "camera_hash":camera_identity_sha256,
            "camera_object_sha256":camera_object.record.sha256,
            "renderer_hash":sha256_hex(b"forgecad-renderer-2"),
            "render_profile":first.render_profile,
            "render_profile_sha256":render_profile_sha256,
            "aov_definition_sha256":first.render_profile["aov_definition_sha256"],
            "color_pipeline_sha256":first.render_profile["color_pipeline_sha256"],
            "id_palette_definition_sha256":first.render_profile["id_palette_definition_sha256"],
            "render_worker_build_cohort_sha256":worker_cohort,
            "render_worker_binding_status":"same_cohort_verified",
            "width":512,
            "height":512,
            "passes":first.passes.iter().map(|pass| pass.pass.clone()).collect::<Vec<_>>(),
            "pass_artifacts":pass_artifacts,
            "canonical_sha256":""
        });
        render_set["canonical_sha256"] = Value::String(canonical_json_hash(&render_set));
        let render_set_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&render_set).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-render-set",
            &now_string(),
        )?;
        reserved_objects.push(render_set_object.clone());
        let applied = first
            .applied_emissive_overrides
            .iter()
            .zip(&overrides)
            .map(|(actual, requested)| {
                json!({
                    "material_zone_id":actual.material_zone_id,
                    "material_id":actual.material_id,
                    "glb_material_index":actual.glb_material_index,
                    "color_linear_rgb":requested.color_linear_rgb,
                    "emissive_strength":requested.emissive_strength
                })
            })
            .collect::<Vec<_>>();
        let mut receipt = json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameReceipt@1",
            "frame_key_sha256":frame_key_sha256,
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256,
            "vfx_profile_object_sha256":profile_sha256,
            "source_candidate_id":source_candidate_id,
            "source_artifact_sha256":source_artifact_sha256,
            "source_artifact_readback_sha256":source_artifact_readback_sha256,
            "sample_request_sha256":sample_request_sha256,
            "sample_result_sha256":sample_result_sha256,
            "sample_time_ticks":sample_time_ticks,
            "effect_materialization_policy":"independent-effect-material-zone@1",
            "camera_identity_sha256":camera_identity_sha256,
            "camera_object_sha256":camera_object.record.sha256,
            "render_profile_sha256":render_profile_sha256,
            "render_worker_build_cohort_sha256":worker_cohort,
            "render_set_object_sha256":render_set_object.record.sha256,
            "applied_emissive_overrides":applied,
            "pass_artifacts":receipt_passes,
            "render_replay_sha256":render_replay_sha256,
            "glb_material_zone_binding_verified":true,
            "sampled_emissive_frame_rendered":true,
            "emissive_animation_sequence_rendered":false,
            "runtime_write_performed":true,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only",
            "limitations":[
                "single-sampled-frame-not-animation-sequence",
                "no-glb-socket-transform-execution",
                "no-bloom-particles-or-trails",
                "no-commercial-engine-roundtrip",
                "no-visual-quality-or-likeness-pass",
                "appearance-source-program-not-yet-independent-durable-sidecar"
            ],
            "canonical_sha256":""
        });
        receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
        let receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&receipt).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-rendered-frame-receipt",
            &now_string(),
        )?;
        reserved_objects.push(receipt_object.clone());
        let mut link = FictionalEnergyVfxFrameLinkRecord {
            schema_version: "FictionalEnergyVfxFrameLink@1".to_owned(),
            frame_key_sha256: frame_key_sha256.clone(),
            project_id: project_id.clone(),
            delivery_manifest_object_sha256: delivery_sha256.clone(),
            vfx_profile_object_sha256: profile_sha256.clone(),
            source_candidate_id: source_candidate_id.clone(),
            source_artifact_sha256: source_artifact_sha256.clone(),
            sample_request_sha256: sample_request_sha256.clone(),
            camera_object_sha256: camera_object.record.sha256.clone(),
            camera_identity_sha256: camera_identity_sha256.clone(),
            render_profile_sha256: render_profile_sha256.clone(),
            render_worker_build_cohort_sha256: worker_cohort.clone(),
            render_set_object_sha256: render_set_object.record.sha256.clone(),
            receipt_object_sha256: receipt_object.record.sha256.clone(),
            pass_object_sha256s: pass_hashes,
            materialization_status: "runtime-owned-durable-fictional-energy-vfx-nine-aov-frame"
                .to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        link.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&link).map_err(|source| error(source.to_string()))?,
        );
        let link = runtime
            .store
            .record_fictional_energy_vfx_frame_link(&link)?;
        Ok(json!({
            "schema_version":"FictionalEnergyVfxRenderedFramePrepareResult@1",
            "frame_key_sha256":frame_key_sha256,
            "receipt_object_sha256":receipt_object.record.sha256,
            "receipt":receipt,
            "durable_link":link,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }))
    })();
    match operation {
        Ok(value) => {
            for object in &reserved_objects {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(operation_error) => {
            let mut rollback_error = None;
            for object in reserved_objects.iter().rev() {
                if let Err(error) =
                    runtime
                        .store
                        .release_cas_reservation_object(&reservation, object, true)
                {
                    rollback_error = Some(error.to_string());
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(error(format!(
                    "fictional energy VFX frame failed ({operation_error}); reservation rollback also failed ({rollback_error})"
                )));
            }
            Err(operation_error)
        }
    }
}

pub(super) fn energy_vfx_rendered_frame_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &["schema_version", "project_id", "frame_key_sha256"],
        "FictionalEnergyVfxRenderedFrameGetRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxRenderedFrameGetRequest@1" {
        return invalid("fictional energy VFX rendered frame get schema differs");
    }
    let project_id = identifier(object, "project_id")?;
    let frame_key_sha256 = sha(object, "frame_key_sha256")?;
    let link = runtime
        .store
        .get_fictional_energy_vfx_frame_link(frame_key_sha256)?
        .ok_or_else(|| error("durable fictional energy VFX rendered frame is unavailable"))?;
    if link.project_id != project_id {
        return invalid("durable fictional energy VFX rendered frame belongs to another project");
    }
    let receipt = read_json(
        runtime,
        &link.receipt_object_sha256,
        "FictionalEnergyVfxRenderedFrameReceipt@1",
    )?;
    if receipt.get("frame_key_sha256").and_then(Value::as_str) != Some(frame_key_sha256)
        || receipt.get("project_id").and_then(Value::as_str) != Some(project_id)
        || receipt
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(link.delivery_manifest_object_sha256.as_str())
        || receipt
            .get("vfx_profile_object_sha256")
            .and_then(Value::as_str)
            != Some(link.vfx_profile_object_sha256.as_str())
        || receipt
            .get("render_set_object_sha256")
            .and_then(Value::as_str)
            != Some(link.render_set_object_sha256.as_str())
        || receipt.get("source_candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || receipt
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || receipt.get("sample_request_sha256").and_then(Value::as_str)
            != Some(link.sample_request_sha256.as_str())
        || receipt.get("camera_object_sha256").and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || receipt
            .get("camera_identity_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || receipt.get("render_profile_sha256").and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || receipt
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || receipt
            .get("effect_materialization_policy")
            .and_then(Value::as_str)
            != Some("independent-effect-material-zone@1")
        || receipt
            .get("pass_artifacts")
            .and_then(Value::as_array)
            .map(|passes| {
                passes
                    .iter()
                    .filter_map(|pass| pass.get("sha256").and_then(Value::as_str))
                    .collect::<Vec<_>>()
            })
            != Some(
                link.pass_object_sha256s
                    .iter()
                    .map(String::as_str)
                    .collect(),
            )
    {
        return invalid("durable fictional energy VFX rendered frame receipt binding differs");
    }
    let render_set = read_json(
        runtime,
        &link.render_set_object_sha256,
        "FictionalEnergyVfxRenderSet@1",
    )?;
    if render_set.get("frame_key_sha256").and_then(Value::as_str) != Some(frame_key_sha256)
        || render_set.get("candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || render_set.get("artifact_sha256").and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || render_set.get("camera_hash").and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || render_set
            .get("camera_object_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || render_set
            .get("render_profile_sha256")
            .and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || render_set
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || render_set
            .get("effect_materialization_policy")
            .and_then(Value::as_str)
            != Some("independent-effect-material-zone@1")
    {
        return invalid("durable fictional energy VFX RenderSet binding differs");
    }
    let camera = read_json(runtime, &link.camera_object_sha256, "CameraCalibration@1")?;
    if camera_identity_hash(&camera)? != link.camera_identity_sha256 {
        return invalid("durable fictional energy VFX camera identity differs");
    }
    let expected_passes = [
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
    ];
    if render_set.get("passes") != Some(&json!(expected_passes))
        || render_set
            .get("render_profile")
            .and_then(|value| value.get("canonical_sha256"))
            .and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
    {
        return invalid("durable fictional energy VFX render profile or AOV order differs");
    }
    let render_passes = render_set
        .get("pass_artifacts")
        .and_then(Value::as_object)
        .ok_or_else(|| error("durable fictional energy VFX RenderSet passes are unavailable"))?;
    let receipt_passes = receipt
        .get("pass_artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| error("durable fictional energy VFX receipt passes are unavailable"))?;
    if receipt_passes.len() != expected_passes.len()
        || expected_passes
            .iter()
            .enumerate()
            .any(|(ordinal, pass_name)| {
                let Some(render_pass) = render_passes.get(*pass_name) else {
                    return true;
                };
                let Some(receipt_pass) = receipt_passes.get(ordinal) else {
                    return true;
                };
                receipt_pass.get("pass").and_then(Value::as_str) != Some(*pass_name)
                    || receipt_pass.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
                    || render_pass.get("sha256").and_then(Value::as_str)
                        != Some(link.pass_object_sha256s[ordinal].as_str())
                    || receipt_pass.get("sha256") != render_pass.get("sha256")
                    || receipt_pass.get("mime") != render_pass.get("mime")
                    || receipt_pass.get("size_bytes") != render_pass.get("size_bytes")
                    || receipt_pass.get("width") != render_pass.get("width")
                    || receipt_pass.get("height") != render_pass.get("height")
                    || receipt_pass.get("channels") != render_pass.get("channels")
                    || receipt_pass.get("color_space") != render_pass.get("color_space")
            })
    {
        return invalid("durable fictional energy VFX AOV bindings differ");
    }
    Ok(json!({
        "schema_version":"FictionalEnergyVfxRenderedFrameGetResult@1",
        "frame_key_sha256":frame_key_sha256,
        "receipt_object_sha256":link.receipt_object_sha256,
        "receipt":receipt,
        "link":link,
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn energy_vfx_rendered_sequence_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "vfx_profile_object_sha256",
            "sample_time_ticks",
            "sampling_policy",
            "appearance_binding_policy",
            "effect_materialization_policy",
            "lod_level",
            "camera_policy",
            "render_policy",
            "canonical_sha256",
        ],
        "FictionalEnergyVfxSequenceRenderPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxSequenceRenderPrepareRequest@1"
        || text(object, "sampling_policy")?
            != "integer-tick-linear-once-clamp-loop-modulo-duration@1"
        || text(object, "appearance_binding_policy")?
            != "three-lod-appearance-program-glb-material-zone-stable-id@1"
        || text(object, "effect_materialization_policy")? != "independent-effect-material-zone@1"
        || object.get("lod_level").and_then(Value::as_u64) != Some(0)
        || text(object, "camera_policy")? != "runtime-fixed-default-camera-calibration@1"
        || text(object, "render_policy")?
            != "lod0-nine-aov-sequence-same-cohort-byte-exact-reservation-safe@1"
    {
        return invalid("fictional energy VFX rendered sequence policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?.to_owned();
    let profile_sha256 = sha(object, "vfx_profile_object_sha256")?.to_owned();
    let sample_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .filter(|values| (2..=16).contains(&values.len()))
        .ok_or_else(|| error("fictional energy VFX rendered sequence tick count is invalid"))?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|tick| *tick <= 1_000_000)
                .ok_or_else(|| error("fictional energy VFX rendered sequence tick is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if sample_time_ticks.windows(2).any(|pair| pair[0] >= pair[1]) {
        return invalid("fictional energy VFX rendered sequence ticks must strictly increase");
    }
    let sequence_key_sha256 = sha(object, "canonical_sha256")?.to_owned();
    if runtime
        .store
        .get_fictional_energy_vfx_sequence_link(&sequence_key_sha256)?
        .is_some()
    {
        return energy_vfx_rendered_sequence_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxRenderedSequenceGetRequest@1",
                "project_id":project_id,
                "sequence_key_sha256":sequence_key_sha256
            }),
        )
        .map(|value| {
            json!({
                "schema_version":"FictionalEnergyVfxRenderedSequencePrepareResult@1",
                "sequence_key_sha256":value["sequence_key_sha256"],
                "receipt_object_sha256":value["receipt_object_sha256"],
                "receipt":value["receipt"],
                "durable_link":value["link"],
                "frames":value["frames"],
                "frame_count":value["frame_count"],
                "candidate_confirmed":false,
                "export_performed":false,
                "actual_engine_roundtrip":false,
                "quality_status":"structural_only"
            })
        });
    }

    // Validate every requested sample against the Appearance source lineage
    // before any durable frame is attempted. This keeps malformed schedules
    // write-free and ensures every frame uses the same three-LOD binding
    // contract with independent muzzle/core MaterialZones.
    for sample_time_tick in &sample_time_ticks {
        let sample_request = seal_request(json!({
            "schema_version":"FictionalEnergyVfxAppearanceFrameSampleRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256,
            "vfx_profile_object_sha256":profile_sha256,
            "sample_time_ticks":sample_time_tick,
            "sampling_policy":"integer-tick-linear-once-clamp-loop-modulo-duration@1",
            "appearance_binding_policy":"three-lod-appearance-program-glb-material-zone-stable-id@1",
            "canonical_sha256":""
        }))?;
        energy_vfx_appearance_frame_sample(runtime, &sample_request)?;
    }

    let mut frame_results = Vec::with_capacity(sample_time_ticks.len());
    for sample_time_tick in &sample_time_ticks {
        let frame_request = seal_request(json!({
            "schema_version":"FictionalEnergyVfxFrameRenderPrepareRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256,
            "vfx_profile_object_sha256":profile_sha256,
            "sample_time_ticks":sample_time_tick,
            "sampling_policy":"integer-tick-linear-once-clamp-loop-modulo-duration@1",
            "appearance_binding_policy":"three-lod-appearance-program-glb-material-zone-stable-id@1",
            "effect_materialization_policy":"independent-effect-material-zone@1",
            "lod_level":0,
            "camera_policy":"runtime-fixed-default-camera-calibration@1",
            "render_policy":"lod0-nine-aov-double-worker-byte-exact-reservation-safe@1",
            "canonical_sha256":""
        }))?;
        let frame_prepare = energy_vfx_rendered_frame_prepare(runtime, &frame_request)?;
        let frame_key_sha256 = frame_prepare
            .get("frame_key_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| error("fictional energy VFX frame key is unavailable"))?;
        let frame_get = energy_vfx_rendered_frame_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
                "project_id":project_id,
                "frame_key_sha256":frame_key_sha256
            }),
        )?;
        let receipt = frame_get
            .get("receipt")
            .ok_or_else(|| error("fictional energy VFX frame receipt is unavailable"))?;
        if receipt.get("sample_time_ticks").and_then(Value::as_u64) != Some(*sample_time_tick)
            || receipt
                .get("effect_materialization_policy")
                .and_then(Value::as_str)
                != Some("independent-effect-material-zone@1")
            || receipt
                .get("glb_material_zone_binding_verified")
                .and_then(Value::as_bool)
                != Some(true)
            || receipt
                .get("sampled_emissive_frame_rendered")
                .and_then(Value::as_bool)
                != Some(true)
            || receipt
                .get("emissive_animation_sequence_rendered")
                .and_then(Value::as_bool)
                != Some(false)
        {
            return invalid("fictional energy VFX frame receipt is not a bounded sampled frame");
        }
        let overrides = receipt
            .get("applied_emissive_overrides")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 2)
            .ok_or_else(|| {
                error("fictional energy VFX frame MaterialZone overrides are incomplete")
            })?;
        let muzzle_zone = overrides
            .iter()
            .find(|value| {
                value.get("material_id").and_then(Value::as_str)
                    == Some("energy-cyan-muzzle-emissive")
            })
            .and_then(|value| value.get("material_zone_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| error("fictional energy VFX muzzle MaterialZone is unavailable"))?
            .to_owned();
        let core_zone = overrides
            .iter()
            .find(|value| {
                value.get("material_id").and_then(Value::as_str) == Some("energy-cyan-emissive")
            })
            .and_then(|value| value.get("material_zone_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| error("fictional energy VFX core MaterialZone is unavailable"))?
            .to_owned();
        if muzzle_zone == core_zone {
            return invalid("fictional energy VFX muzzle/core MaterialZones are not independent");
        }
        frame_results.push((frame_get, muzzle_zone, core_zone));
    }

    let first_receipt = frame_results
        .first()
        .and_then(|value| value.0.get("receipt"))
        .ok_or_else(|| error("fictional energy VFX first frame receipt is unavailable"))?;
    let common_fields = [
        "source_candidate_id",
        "source_artifact_sha256",
        "camera_object_sha256",
        "camera_identity_sha256",
        "render_profile_sha256",
        "render_worker_build_cohort_sha256",
    ];
    for (frame, _, _) in &frame_results {
        let receipt = frame
            .get("receipt")
            .ok_or_else(|| error("fictional energy VFX frame receipt is unavailable"))?;
        for field in common_fields {
            if receipt.get(field) != first_receipt.get(field) {
                return invalid(format!(
                    "fictional energy VFX sequence {field} differs across frames"
                ));
            }
        }
        if receipt
            .get("pass_artifacts")
            .and_then(Value::as_array)
            .map_or(true, |passes| passes.len() != 9)
        {
            return invalid("fictional energy VFX sequence frame does not contain nine AOVs");
        }
    }
    let source_candidate_id = first_receipt["source_candidate_id"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX source candidate is unavailable"))?
        .to_owned();
    let source_artifact_sha256 = first_receipt["source_artifact_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX source artifact is unavailable"))?
        .to_owned();
    let camera_object_sha256 = first_receipt["camera_object_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX camera object is unavailable"))?
        .to_owned();
    let camera_identity_sha256 = first_receipt["camera_identity_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX camera identity is unavailable"))?
        .to_owned();
    let render_profile_sha256 = first_receipt["render_profile_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX render profile is unavailable"))?
        .to_owned();
    let render_worker_build_cohort_sha256 = first_receipt["render_worker_build_cohort_sha256"]
        .as_str()
        .ok_or_else(|| error("fictional energy VFX worker cohort is unavailable"))?
        .to_owned();
    let frame_key_sha256s = frame_results
        .iter()
        .map(|(frame, _, _)| {
            frame
                .get("frame_key_sha256")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| error("fictional energy VFX frame key is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let frame_receipt_entries = frame_results
        .iter()
        .enumerate()
        .map(|(ordinal, (frame, _, _))| {
            let receipt = frame
                .get("receipt")
                .ok_or_else(|| error("fictional energy VFX frame receipt is unavailable"))?;
            Ok(json!({
                "ordinal":ordinal,
                "sample_time_ticks":receipt["sample_time_ticks"],
                "frame_key_sha256":frame["frame_key_sha256"],
                "receipt_object_sha256":frame["receipt_object_sha256"],
                "render_set_object_sha256":receipt["render_set_object_sha256"],
                "pass_artifacts":receipt["pass_artifacts"]
            }))
        })
        .collect::<Result<Vec<Value>, RuntimeError>>()?;
    let muzzle_zone = frame_results[0].1.clone();
    let core_zone = frame_results[0].2.clone();
    let receipt = seal_sidecar(json!({
        "schema_version":"FictionalEnergyVfxRenderedSequenceReceipt@1",
        "sequence_key_sha256":sequence_key_sha256,
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_sha256,
        "vfx_profile_object_sha256":profile_sha256,
        "source_candidate_id":source_candidate_id,
        "source_artifact_sha256":source_artifact_sha256,
        "request_sha256":sequence_key_sha256,
        "sample_time_ticks":sample_time_ticks,
        "sampling_policy":"integer-tick-linear-once-clamp-loop-modulo-duration@1",
        "appearance_binding_policy":"three-lod-appearance-program-glb-material-zone-stable-id@1",
        "effect_materialization_policy":"independent-effect-material-zone@1",
        "lod_level":0,
        "camera_policy":"runtime-fixed-default-camera-calibration@1",
        "camera_object_sha256":camera_object_sha256,
        "camera_identity_sha256":camera_identity_sha256,
        "render_policy":"lod0-nine-aov-sequence-same-cohort-byte-exact-reservation-safe@1",
        "render_profile_sha256":render_profile_sha256,
        "render_worker_build_cohort_sha256":render_worker_build_cohort_sha256,
        "frame_count":frame_key_sha256s.len(),
        "frame_key_sha256s":frame_key_sha256s,
        "frames":frame_receipt_entries,
        "muzzle_material_zone_id":muzzle_zone,
        "core_material_zone_id":core_zone,
        "fixed_camera_verified":true,
        "lod0_verified":true,
        "nine_aov_per_frame_verified":true,
        "same_worker_cohort_verified":true,
        "independent_effect_material_zones_verified":true,
        "sampled_emissive_sequence_rendered":true,
        "emissive_animation_sequence_rendered":false,
        "runtime_write_performed":true,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only",
        "limitations":[
            "sampled-emissive-sequence-not-engine-material-animation",
            "no-glb-socket-transform-execution",
            "no-bloom-particles-or-trails",
            "no-commercial-engine-roundtrip",
            "no-visual-quality-or-likeness-pass",
            "frame-receipts-remain-independent-durable-links"
        ],
        "canonical_sha256":""
    }))?;
    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::<CasObject>::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&receipt).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-rendered-sequence-receipt",
            &now_string(),
        )?;
        reserved_objects.push(receipt_object.clone());
        let mut link = FictionalEnergyVfxSequenceLinkRecord {
            schema_version: "FictionalEnergyVfxSequenceLink@1".to_owned(),
            sequence_key_sha256: sequence_key_sha256.clone(),
            project_id: project_id.clone(),
            delivery_manifest_object_sha256: delivery_sha256.clone(),
            vfx_profile_object_sha256: profile_sha256.clone(),
            source_candidate_id: source_candidate_id.clone(),
            source_artifact_sha256: source_artifact_sha256.clone(),
            request_sha256: sequence_key_sha256.clone(),
            camera_object_sha256: camera_object_sha256.clone(),
            camera_identity_sha256: camera_identity_sha256.clone(),
            render_profile_sha256: render_profile_sha256.clone(),
            render_worker_build_cohort_sha256: render_worker_build_cohort_sha256.clone(),
            receipt_object_sha256: receipt_object.record.sha256.clone(),
            frame_key_sha256s: frame_key_sha256s.clone(),
            materialization_status: "runtime-owned-durable-fictional-energy-vfx-rendered-sequence"
                .to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        link.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&link).map_err(|source| error(source.to_string()))?,
        );
        let link = runtime
            .store
            .record_fictional_energy_vfx_sequence_link(&link)?;
        Ok(json!({
            "schema_version":"FictionalEnergyVfxRenderedSequencePrepareResult@1",
            "sequence_key_sha256":sequence_key_sha256,
            "receipt_object_sha256":receipt_object.record.sha256,
            "receipt":receipt,
            "durable_link":link,
            "frames":frame_results.iter().map(|value| value.0.clone()).collect::<Vec<_>>(),
            "frame_count":frame_key_sha256s.len(),
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }))
    })();
    match operation {
        Ok(value) => {
            for object in &reserved_objects {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(operation_error) => {
            let mut rollback_error = None;
            for object in reserved_objects.iter().rev() {
                if let Err(error) =
                    runtime
                        .store
                        .release_cas_reservation_object(&reservation, object, true)
                {
                    rollback_error = Some(error.to_string());
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(error(format!(
                    "fictional energy VFX sequence failed ({operation_error}); reservation rollback also failed ({rollback_error})"
                )));
            }
            Err(operation_error)
        }
    }
}

pub(super) fn energy_vfx_rendered_sequence_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &["schema_version", "project_id", "sequence_key_sha256"],
        "FictionalEnergyVfxRenderedSequenceGetRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxRenderedSequenceGetRequest@1" {
        return invalid("fictional energy VFX rendered sequence get schema differs");
    }
    let project_id = identifier(object, "project_id")?;
    let sequence_key_sha256 = sha(object, "sequence_key_sha256")?;
    let link = runtime
        .store
        .get_fictional_energy_vfx_sequence_link(sequence_key_sha256)?
        .ok_or_else(|| error("durable fictional energy VFX rendered sequence is unavailable"))?;
    if link.project_id != project_id {
        return invalid(
            "durable fictional energy VFX rendered sequence belongs to another project",
        );
    }
    let receipt = read_json(
        runtime,
        &link.receipt_object_sha256,
        "FictionalEnergyVfxRenderedSequenceReceipt@1",
    )?;
    if receipt.get("sequence_key_sha256").and_then(Value::as_str) != Some(sequence_key_sha256)
        || receipt.get("project_id").and_then(Value::as_str) != Some(project_id)
        || receipt
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(link.delivery_manifest_object_sha256.as_str())
        || receipt
            .get("vfx_profile_object_sha256")
            .and_then(Value::as_str)
            != Some(link.vfx_profile_object_sha256.as_str())
        || receipt.get("source_candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || receipt
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || receipt.get("camera_object_sha256").and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || receipt
            .get("camera_identity_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || receipt.get("render_profile_sha256").and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || receipt
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || receipt
            .get("frame_key_sha256s")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            != Some(link.frame_key_sha256s.iter().map(String::as_str).collect())
        || receipt.get("frame_count").and_then(Value::as_u64)
            != Some(link.frame_key_sha256s.len() as u64)
        || receipt
            .get("fixed_camera_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("lod0_verified").and_then(Value::as_bool) != Some(true)
        || receipt
            .get("nine_aov_per_frame_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("same_worker_cohort_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("independent_effect_material_zones_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("sampled_emissive_sequence_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("emissive_animation_sequence_rendered")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return invalid("durable fictional energy VFX sequence receipt binding differs");
    }
    let frames = receipt
        .get("frames")
        .and_then(Value::as_array)
        .filter(|values| values.len() == link.frame_key_sha256s.len())
        .ok_or_else(|| error("durable fictional energy VFX sequence frame index is unavailable"))?;
    let mut frame_results = Vec::with_capacity(link.frame_key_sha256s.len());
    for (ordinal, frame_key_sha256) in link.frame_key_sha256s.iter().enumerate() {
        let frame = energy_vfx_rendered_frame_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
                "project_id":project_id,
                "frame_key_sha256":frame_key_sha256
            }),
        )?;
        let frame_receipt = frame
            .get("receipt")
            .ok_or_else(|| error("durable fictional energy VFX frame receipt is unavailable"))?;
        let frame_entry = frames.get(ordinal).ok_or_else(|| {
            error("durable fictional energy VFX sequence frame entry is unavailable")
        })?;
        if frame_entry.get("frame_key_sha256").and_then(Value::as_str)
            != Some(frame_key_sha256.as_str())
            || frame_entry
                .get("receipt_object_sha256")
                .and_then(Value::as_str)
                != frame.get("receipt_object_sha256").and_then(Value::as_str)
            || frame_entry.get("sample_time_ticks") != frame_receipt.get("sample_time_ticks")
            || frame_entry
                .get("render_set_object_sha256")
                .and_then(Value::as_str)
                != frame_receipt
                    .get("render_set_object_sha256")
                    .and_then(Value::as_str)
            || frame_entry.get("pass_artifacts") != frame_receipt.get("pass_artifacts")
            || frame_receipt
                .get("camera_object_sha256")
                .and_then(Value::as_str)
                != Some(link.camera_object_sha256.as_str())
            || frame_receipt
                .get("camera_identity_sha256")
                .and_then(Value::as_str)
                != Some(link.camera_identity_sha256.as_str())
            || frame_receipt
                .get("render_profile_sha256")
                .and_then(Value::as_str)
                != Some(link.render_profile_sha256.as_str())
            || frame_receipt
                .get("render_worker_build_cohort_sha256")
                .and_then(Value::as_str)
                != Some(link.render_worker_build_cohort_sha256.as_str())
        {
            return invalid("durable fictional energy VFX sequence frame binding differs");
        }
        frame_results.push(frame);
    }
    Ok(json!({
        "schema_version":"FictionalEnergyVfxRenderedSequenceGetResult@1",
        "sequence_key_sha256":sequence_key_sha256,
        "receipt_object_sha256":link.receipt_object_sha256,
        "receipt":receipt,
        "link":link,
        "frames":frame_results,
        "frame_count":link.frame_key_sha256s.len(),
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

const HDR_BLOOM_RENDER_POLICY: &str =
    "lod0-hdr-emissive-source-two-pass-fixed-kernel-base-aov-byte-exact@1";
const HDR_BLOOM_RECEIPT_SCHEMA: &str = "FictionalEnergyVfxHdrBloomFrameReceipt@1";
const HDR_BLOOM_RENDER_SET_SCHEMA: &str = "FictionalEnergyVfxHdrBloomRenderSet@1";
const PARTICLES_RENDER_POLICY: &str =
    "lod0-three-typed-particle-aov-depth-tested-base-bloom-byte-exact@1";
const PARTICLES_RECEIPT_SCHEMA: &str = "FictionalEnergyVfxParticlesFrameReceipt@1";
const PARTICLES_RENDER_SET_SCHEMA: &str = "FictionalEnergyVfxParticlesRenderSet@1";
const PARTICLE_POLICY: &str = "two-closed-emitters-hash-seeded-typed-attributes@1";
const PARTICLE_EMITTER_POLICY: &str = "muzzle-burst-24-energy-core-sparks-32@1";
const PARTICLE_SEED_POLICY: &str = "durable-hash-concatenation-sha256-no-caller-rng@1";
const TRAILS_RENDER_POLICY: &str =
    "lod0-three-typed-trail-aov-depth-tested-base-bloom-particles-byte-exact-no-bloom-input@1";
const TRAILS_RECEIPT_SCHEMA: &str = "FictionalEnergyVfxTrailsFrameReceipt@1";
const TRAILS_RENDER_SET_SCHEMA: &str = "FictionalEnergyVfxTrailsRenderSet@1";
const TRAILS_BLOOM_POLICY: &str = "lod0-typed-trails-hdr-source-two-pass-fixed-kernel@1";
const TRAILS_BLOOM_INPUT_POLICY: &str =
    "existing-trail-color-depth-plus-current-base-opaque-depth-byte-exact@1";
const TRAILS_BLOOM_OCCLUSION_POLICY: &str =
    "current-base-opaque-depth-before-trail-depth-reversed-normalized-u8-epsilon-1e-4@1";
const TRAILS_BLOOM_RENDER_POLICY: &str =
    "lod0-trail-bloom-two-new-passes-base-bloom-particles-trails-byte-exact-reused@1";
const TRAILS_BLOOM_RECEIPT_SCHEMA: &str = "FictionalEnergyVfxTrailsBloomFrameReceipt@1";
const TRAILS_BLOOM_RENDER_SET_SCHEMA: &str = "FictionalEnergyVfxTrailsBloomRenderSet@1";

fn fixed_particle_simulation_quantization() -> Value {
    json!({
        "hash_stream":"sha256-q24@1",
        "position_m":"signed-micrometer-round-nearest@1",
        "radius_px":"q8-unsigned@1",
        "alpha":"q16-unsigned@1",
        "lifetime_ticks":"integer@1",
        "sort_order":"emitter-definition-then-spawn-ordinal@1"
    })
}

fn fixed_trail_id_encoding() -> Value {
    json!({
        "schema_version":"FictionalEnergyVfxTrailIdEncoding@1",
        "encoding":"lossless-little-endian-rgb24-parent-trail-id-plus-one-alpha-visible@1",
        "background_rgba8":[0,0,0,0],
        "id_range":[1,65535],
        "segment_identity":"segments-share-stable-parent-trail-id@1"
    })
}

fn fixed_trails_limitations() -> Value {
    json!([
        "parent-trail-id-shared-by-segments",
        "no-trail-bloom-input",
        "anchor-sidecar-not-glb-socket",
        "no-commercial-engine-roundtrip",
        "no-visual-quality-or-likeness-pass"
    ])
}

fn fixed_trails_bloom_profile_value() -> Value {
    json!({
        "threshold":1.0,
        "source_gain":8.0,
        "radius_px":8,
        "intensity":4.0,
        "hdr_clamp":16.0,
        "blur_passes":2,
        "kernel":"separable-box-two-pass-fixed-radius@1"
    })
}

fn parse_fixed_trails_bloom_profile(
    value: &Value,
) -> Result<(render_worker::TypedTrailBloomProfile, String), RuntimeError> {
    exact_object(
        value,
        &[
            "threshold",
            "source_gain",
            "radius_px",
            "intensity",
            "hdr_clamp",
            "blur_passes",
            "kernel",
        ],
        "trail Bloom profile",
    )?;
    if value != &fixed_trails_bloom_profile_value() {
        return invalid("trail Bloom profile must use the fixed Runtime profile");
    }
    Ok((
        render_worker::TypedTrailBloomProfile::FIXED,
        canonical_json_hash(value),
    ))
}

fn fixed_trails_bloom_limitations() -> Value {
    json!([
        "base-bloom-particles-trails-byte-exact-reused",
        "trail-bloom-passes-independent-from-base-bloom",
        "no-commercial-engine-roundtrip",
        "no-visual-quality-or-likeness-pass"
    ])
}

fn validate_trails_bloom_semantics(
    receipt: &Value,
    render_set: &Value,
) -> Result<(), RuntimeError> {
    let (_profile, profile_sha256) = parse_fixed_trails_bloom_profile(
        receipt
            .get("trail_bloom_profile")
            .ok_or_else(|| error("trail Bloom receipt profile is unavailable"))?,
    )?;
    let render_profile = render_set
        .get("trail_bloom_profile")
        .ok_or_else(|| error("trail Bloom RenderSet profile is unavailable"))?;
    if parse_fixed_trails_bloom_profile(render_profile)?.1 != profile_sha256
        || receipt
            .get("trail_bloom_profile_sha256")
            .and_then(Value::as_str)
            != Some(profile_sha256.as_str())
        || render_set
            .get("trail_bloom_profile_sha256")
            .and_then(Value::as_str)
            != Some(profile_sha256.as_str())
        || receipt
            .get("render_worker_binding_status")
            .and_then(Value::as_str)
            != Some("same_cohort_verified")
        || render_set
            .get("render_worker_binding_status")
            .and_then(Value::as_str)
            != Some("same_cohort_verified")
        || receipt
            .get("base_opaque_depth_pass")
            .and_then(Value::as_str)
            != Some("depth")
        || render_set
            .get("base_opaque_depth_pass")
            .and_then(Value::as_str)
            != Some("depth")
        || receipt.get("trail_bloom_policy").and_then(Value::as_str) != Some(TRAILS_BLOOM_POLICY)
        || render_set.get("trail_bloom_policy").and_then(Value::as_str) != Some(TRAILS_BLOOM_POLICY)
        || receipt.get("input_policy").and_then(Value::as_str) != Some(TRAILS_BLOOM_INPUT_POLICY)
        || render_set.get("input_policy").and_then(Value::as_str) != Some(TRAILS_BLOOM_INPUT_POLICY)
        || receipt.get("occlusion_policy").and_then(Value::as_str)
            != Some(TRAILS_BLOOM_OCCLUSION_POLICY)
        || render_set.get("occlusion_policy").and_then(Value::as_str)
            != Some(TRAILS_BLOOM_OCCLUSION_POLICY)
        || receipt.get("render_policy").and_then(Value::as_str) != Some(TRAILS_BLOOM_RENDER_POLICY)
        || render_set.get("render_policy").and_then(Value::as_str)
            != Some(TRAILS_BLOOM_RENDER_POLICY)
        || receipt
            .get("base_aov_byte_exact_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("base_opaque_depth_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("bloom_pass_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("particle_passes_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("source_trail_passes_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || render_set
            .get("base_aov_byte_exact_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || render_set
            .get("base_opaque_depth_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || render_set
            .get("bloom_pass_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || render_set
            .get("particle_passes_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || render_set
            .get("source_trail_passes_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("base_bloom_mutated").and_then(Value::as_bool) != Some(false)
        || receipt
            .get("particle_passes_mutated")
            .and_then(Value::as_bool)
            != Some(false)
        || receipt.get("trail_passes_mutated").and_then(Value::as_bool) != Some(false)
        || render_set
            .get("base_bloom_mutated")
            .and_then(Value::as_bool)
            != Some(false)
        || render_set
            .get("particle_passes_mutated")
            .and_then(Value::as_bool)
            != Some(false)
        || render_set
            .get("trail_passes_mutated")
            .and_then(Value::as_bool)
            != Some(false)
        || receipt
            .get("opaque_geometry_depth_tested")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("trail_bloom_source_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("trail_bloom_contribution_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("trail_bloom_rendered").and_then(Value::as_bool) != Some(true)
        || receipt.get("trail_bloom_input").and_then(Value::as_bool) != Some(true)
        || render_set
            .get("trail_bloom_source_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || render_set
            .get("trail_bloom_contribution_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || render_set
            .get("trail_bloom_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || render_set.get("trail_bloom_input").and_then(Value::as_bool) != Some(true)
        || receipt
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || receipt.get("export_performed").and_then(Value::as_bool) != Some(false)
        || receipt
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
        || receipt.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || receipt.get("limitations") != Some(&fixed_trails_bloom_limitations())
        || render_set.get("passes")
            != Some(&json!([
                "trail-emissive-source",
                "trail-bloom-contribution"
            ]))
    {
        return invalid("durable trail Bloom semantic policy differs");
    }
    Ok(())
}

fn validate_trails_bloom_parent_binding(
    link: &FictionalEnergyVfxTrailsBloomFrameLinkRecord,
    source_trail_link: &FictionalEnergyVfxTrailsFrameLinkRecord,
    base_link: &FictionalEnergyVfxFrameLinkRecord,
    bloom_link: &FictionalEnergyVfxBloomFrameLinkRecord,
) -> Result<(), RuntimeError> {
    if link.schema_version != "FictionalEnergyVfxTrailsBloomFrameLink@1"
        || link.project_id != source_trail_link.project_id
        || link.delivery_manifest_object_sha256 != source_trail_link.delivery_manifest_object_sha256
        || link.vfx_profile_object_sha256 != source_trail_link.vfx_profile_object_sha256
        || link.anchor_set_object_sha256 != source_trail_link.anchor_set_object_sha256
        || link.source_candidate_id != source_trail_link.source_candidate_id
        || link.source_artifact_sha256 != source_trail_link.source_artifact_sha256
        || link.sample_request_sha256 != source_trail_link.sample_request_sha256
        || link.base_frame_key_sha256 != source_trail_link.base_frame_key_sha256
        || link.bloom_key_sha256 != source_trail_link.bloom_key_sha256
        || link.source_trail_key_sha256 != source_trail_link.trail_key_sha256
        || link.camera_object_sha256 != source_trail_link.camera_object_sha256
        || link.camera_identity_sha256 != source_trail_link.camera_identity_sha256
        || link.render_profile_sha256 != source_trail_link.render_profile_sha256
        || link.render_worker_build_cohort_sha256
            != source_trail_link.render_worker_build_cohort_sha256
        || link.trail_seed_sha256 != source_trail_link.trail_seed_sha256
        || link.node_inventory_sha256 != source_trail_link.node_inventory_sha256
        || link.owner_world_transform_sha256 != source_trail_link.owner_world_transform_sha256
        || link.trail_inventory_sha256 != source_trail_link.trail_inventory_sha256
        || link.trail_id_encoding_sha256 != source_trail_link.trail_id_encoding_sha256
        || link.source_trail_color_object_sha256 != source_trail_link.trail_color_object_sha256
        || link.source_trail_id_object_sha256 != source_trail_link.trail_id_object_sha256
        || link.source_trail_depth_object_sha256 != source_trail_link.trail_depth_object_sha256
        || link.base_frame_key_sha256 != base_link.frame_key_sha256
        || link.project_id != base_link.project_id
        || link.delivery_manifest_object_sha256 != base_link.delivery_manifest_object_sha256
        || link.vfx_profile_object_sha256 != base_link.vfx_profile_object_sha256
        || link.source_candidate_id != base_link.source_candidate_id
        || link.source_artifact_sha256 != base_link.source_artifact_sha256
        || link.camera_object_sha256 != base_link.camera_object_sha256
        || link.camera_identity_sha256 != base_link.camera_identity_sha256
        || link.render_profile_sha256 != base_link.render_profile_sha256
        || link.render_worker_build_cohort_sha256 != base_link.render_worker_build_cohort_sha256
        || link.bloom_key_sha256 != bloom_link.bloom_key_sha256
        || link.base_frame_key_sha256 != bloom_link.base_frame_key_sha256
        || link.project_id != bloom_link.project_id
        || link.delivery_manifest_object_sha256 != bloom_link.delivery_manifest_object_sha256
        || link.vfx_profile_object_sha256 != bloom_link.vfx_profile_object_sha256
        || link.source_candidate_id != bloom_link.source_candidate_id
        || link.source_artifact_sha256 != bloom_link.source_artifact_sha256
        || link.camera_object_sha256 != bloom_link.camera_object_sha256
        || link.camera_identity_sha256 != bloom_link.camera_identity_sha256
        || link.render_profile_sha256 != bloom_link.render_profile_sha256
        || link.render_worker_build_cohort_sha256 != bloom_link.render_worker_build_cohort_sha256
        || base_link.pass_object_sha256s.len() != 9
        || link.base_opaque_depth_object_sha256 != base_link.pass_object_sha256s[2]
    {
        return invalid("durable trail Bloom parent link binding differs");
    }
    Ok(())
}

fn verify_trails_bloom_cas_hash(
    runtime: &Runtime,
    hash: &str,
    context: &str,
) -> Result<Vec<u8>, RuntimeError> {
    let bytes = runtime.cas_read(hash)?;
    if bytes.is_empty() || sha256_hex(&bytes) != hash {
        return invalid(format!("{context} CAS hash differs"));
    }
    Ok(bytes)
}

fn validate_trails_bloom_pass_metadata(
    runtime: &Runtime,
    pass: &Value,
    pass_name: &str,
    hash: &str,
) -> Result<(), RuntimeError> {
    let bytes = verify_trails_bloom_cas_hash(runtime, hash, pass_name)?;
    if pass.get("pass").and_then(Value::as_str) != Some(pass_name)
        || pass.get("sha256").and_then(Value::as_str) != Some(hash)
        || pass.get("mime").and_then(Value::as_str) != Some("image/png")
        || pass.get("size_bytes").and_then(Value::as_u64) != Some(bytes.len() as u64)
        || pass.get("width").and_then(Value::as_u64) != Some(512)
        || pass.get("height").and_then(Value::as_u64) != Some(512)
        || pass.get("channels").and_then(Value::as_str) != Some("rgba8")
        || pass.get("color_space").and_then(Value::as_str) != Some("data")
    {
        return invalid(format!("{pass_name} pass metadata differs"));
    }
    Ok(())
}

fn validate_trails_bloom_parent_passes(
    runtime: &Runtime,
    receipt: &Value,
    render_set: &Value,
    link: &FictionalEnergyVfxTrailsBloomFrameLinkRecord,
    source_trail_link: &FictionalEnergyVfxTrailsFrameLinkRecord,
    base_link: &FictionalEnergyVfxFrameLinkRecord,
    bloom_link: &FictionalEnergyVfxBloomFrameLinkRecord,
    particle_pass_sha256s: &[Value],
) -> Result<(), RuntimeError> {
    let base_aov_passes = json!(base_link.pass_object_sha256s);
    let bloom_passes = json!([
        bloom_link.source_object_sha256,
        bloom_link.contribution_object_sha256
    ]);
    let source_trail_passes = json!([
        source_trail_link.trail_color_object_sha256,
        source_trail_link.trail_id_object_sha256,
        source_trail_link.trail_depth_object_sha256
    ]);
    if receipt.get("base_aov_passes") != Some(&base_aov_passes)
        || render_set.get("base_aov_passes") != Some(&base_aov_passes)
        || receipt.get("bloom_passes") != Some(&bloom_passes)
        || render_set.get("bloom_passes") != Some(&bloom_passes)
        || receipt.get("particle_pass_sha256s")
            != Some(&Value::Array(particle_pass_sha256s.to_vec()))
        || render_set.get("particle_pass_sha256s")
            != Some(&Value::Array(particle_pass_sha256s.to_vec()))
        || receipt.get("source_trail_passes") != Some(&source_trail_passes)
        || render_set.get("source_trail_passes") != Some(&source_trail_passes)
        || receipt
            .get("base_opaque_depth_object_sha256")
            .and_then(Value::as_str)
            != Some(link.base_opaque_depth_object_sha256.as_str())
    {
        return invalid("durable trail Bloom parent pass inventory differs");
    }
    for hash in base_link
        .pass_object_sha256s
        .iter()
        .chain(std::iter::once(&bloom_link.source_object_sha256))
        .chain(std::iter::once(&bloom_link.contribution_object_sha256))
        .chain(std::iter::once(
            &source_trail_link.trail_color_object_sha256,
        ))
        .chain(std::iter::once(&source_trail_link.trail_id_object_sha256))
        .chain(std::iter::once(
            &source_trail_link.trail_depth_object_sha256,
        ))
    {
        verify_trails_bloom_cas_hash(runtime, hash, "durable trail Bloom parent pass")?;
    }
    for passes in particle_pass_sha256s {
        let values = passes
            .as_array()
            .filter(|values| values.len() == 3)
            .ok_or_else(|| error("durable trail Bloom particle pass inventory is invalid"))?;
        for hash in values {
            let hash = hash
                .as_str()
                .filter(|value| forgecad_contracts::is_sha256(value))
                .ok_or_else(|| error("durable trail Bloom particle pass hash is invalid"))?;
            verify_trails_bloom_cas_hash(runtime, hash, "durable trail Bloom particle pass")?;
        }
    }
    Ok(())
}

fn validate_particle_semantics(receipt: &Value, render_set: &Value) -> Result<(), RuntimeError> {
    if receipt.get("seed_policy").and_then(Value::as_str) != Some(PARTICLE_SEED_POLICY)
        || receipt.get("simulation_quantization") != Some(&fixed_particle_simulation_quantization())
        || receipt.get("particle_policy").and_then(Value::as_str) != Some(PARTICLE_POLICY)
        || receipt.get("emitter_policy").and_then(Value::as_str) != Some(PARTICLE_EMITTER_POLICY)
        || receipt.get("emitter_counts")
            != Some(&json!({"muzzle-burst":24,"energy-core-sparks":32}))
        || render_set.get("particle_policy").and_then(Value::as_str) != Some(PARTICLE_POLICY)
        || render_set.get("emitter_policy").and_then(Value::as_str) != Some(PARTICLE_EMITTER_POLICY)
    {
        return invalid("durable typed particle semantic policy differs");
    }
    Ok(())
}

fn validate_trails_receipt_semantics(receipt: &Value) -> Result<(), RuntimeError> {
    if receipt
        .get("opaque_geometry_depth_tested")
        .and_then(Value::as_bool)
        != Some(true)
        || receipt
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || receipt.get("export_performed").and_then(Value::as_bool) != Some(false)
        || receipt
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
        || receipt.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || receipt.get("limitations") != Some(&fixed_trails_limitations())
        || receipt.get("trail_id_encoding") != Some(&fixed_trail_id_encoding())
    {
        return invalid("durable typed trail semantic receipt differs");
    }
    Ok(())
}

fn particle_receipt_matches_render_binding(
    receipt: &Value,
    camera_object_sha256: &str,
    camera_identity_sha256: &str,
    render_profile_sha256: &str,
    render_worker_build_cohort_sha256: &str,
) -> bool {
    receipt.get("camera_object_sha256").and_then(Value::as_str) == Some(camera_object_sha256)
        && receipt
            .get("camera_identity_sha256")
            .and_then(Value::as_str)
            == Some(camera_identity_sha256)
        && receipt.get("render_profile_sha256").and_then(Value::as_str)
            == Some(render_profile_sha256)
        && receipt
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            == Some(render_worker_build_cohort_sha256)
}

fn fixed_hdr_bloom_profile_value() -> Value {
    json!({
        "threshold":1.0,
        "radius_px":8,
        "intensity":4.0,
        "hdr_clamp":16.0
    })
}

fn parse_fixed_hdr_bloom_profile(
    value: &Value,
) -> Result<(render_worker::HdrBloomProfile, String), RuntimeError> {
    let object = exact_object(
        value,
        &["threshold", "radius_px", "intensity", "hdr_clamp"],
        "HDR bloom profile",
    )?;
    let expected = fixed_hdr_bloom_profile_value();
    if value != &expected {
        return invalid("HDR bloom profile must use the fixed Runtime profile");
    }
    let profile = render_worker::HdrBloomProfile {
        threshold: object
            .get("threshold")
            .and_then(Value::as_f64)
            .ok_or_else(|| error("HDR bloom threshold is invalid"))? as f32,
        radius_px: object
            .get("radius_px")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| error("HDR bloom radius is invalid"))?,
        intensity: object
            .get("intensity")
            .and_then(Value::as_f64)
            .ok_or_else(|| error("HDR bloom intensity is invalid"))? as f32,
        hdr_clamp: object
            .get("hdr_clamp")
            .and_then(Value::as_f64)
            .ok_or_else(|| error("HDR bloom HDR clamp is invalid"))? as f32,
    };
    let profile_hash = canonical_json_hash(value);
    Ok((profile, profile_hash))
}

fn parse_bloom_overrides(
    value: Option<&Value>,
) -> Result<Vec<render_worker::EmissiveMaterialOverride>, RuntimeError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or_else(|| error("HDR bloom base frame must contain exactly two overrides"))?;
    let mut overrides = Vec::with_capacity(2);
    let mut zones = BTreeSet::new();
    let mut materials = BTreeSet::new();
    for value in values {
        let object = exact_object(
            value,
            &[
                "material_zone_id",
                "material_id",
                "glb_material_index",
                "color_linear_rgb",
                "emissive_strength",
            ],
            "HDR bloom emissive override",
        )?;
        let material_zone_id = text(object, "material_zone_id")?;
        let material_id = text(object, "material_id")?;
        if !forgecad_contracts::is_opaque_id(material_zone_id)
            || !forgecad_contracts::is_opaque_id(material_id)
            || !zones.insert(material_zone_id.to_owned())
            || !materials.insert(material_id.to_owned())
        {
            return invalid("HDR bloom emissive overrides must be two independent stable zones");
        }
        let _glb_material_index = object
            .get("glb_material_index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value < 256)
            .ok_or_else(|| error("HDR bloom GLB material index is invalid"))?;
        let color = object
            .get("color_linear_rgb")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| error("HDR bloom emissive color is invalid"))?;
        let mut color_linear_rgb = [0.0_f32; 3];
        for (index, channel) in color.iter().enumerate() {
            color_linear_rgb[index] = channel
                .as_f64()
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .ok_or_else(|| error("HDR bloom emissive color is outside 0 to 1"))?
                as f32;
        }
        let emissive_strength = object
            .get("emissive_strength")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (0.0..=16.0).contains(value))
            .ok_or_else(|| error("HDR bloom emissive strength is outside 0 to 16"))?
            as f32;
        overrides.push(render_worker::EmissiveMaterialOverride {
            material_zone_id: material_zone_id.to_owned(),
            material_id: material_id.to_owned(),
            color_linear_rgb,
            emissive_strength,
        });
    }
    Ok(overrides)
}

fn bloom_pass_metadata(object: &CasObject, pass: &str) -> Value {
    json!({
        "pass":pass,
        "sha256":object.record.sha256,
        "mime":"image/png",
        "size_bytes":object.record.size_bytes,
        "width":512,
        "height":512,
        "channels":"rgba8",
        "color_space":"data"
    })
}

fn particle_camera_depth(camera: &Value, position: [f64; 3]) -> Result<f32, RuntimeError> {
    let transform = camera
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| error("particle camera transform is unavailable"))?;
    let vector = |field: &str| -> Result<[f64; 3], RuntimeError> {
        let values = transform
            .get(field)
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| error(format!("particle camera {field} is invalid")))?;
        Ok([
            finite_f64(values.first(), field, 100.0)?,
            finite_f64(values.get(1), field, 100.0)?,
            finite_f64(values.get(2), field, 100.0)?,
        ])
    };
    let camera_position = vector("position_m")?;
    let target = vector("target_m")?;
    let up_input = vector("up")?;
    let subtract = |left: [f64; 3], right: [f64; 3]| {
        [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
    };
    let dot = |left: [f64; 3], right: [f64; 3]| {
        left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
    };
    let cross = |left: [f64; 3], right: [f64; 3]| {
        [
            left[1] * right[2] - left[2] * right[1],
            left[2] * right[0] - left[0] * right[2],
            left[0] * right[1] - left[1] * right[0],
        ]
    };
    let normalize = |value: [f64; 3]| -> Result<[f64; 3], RuntimeError> {
        let length = dot(value, value).sqrt();
        if !length.is_finite() || length <= f64::EPSILON {
            return invalid("particle camera basis is degenerate");
        }
        Ok([value[0] / length, value[1] / length, value[2] / length])
    };
    let forward = normalize(subtract(target, camera_position))?;
    let relative = subtract(position, camera_position);
    let z = dot(relative, forward);
    let near = camera
        .get("near_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| error("particle camera near plane is invalid"))?;
    let far = camera
        .get("far_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > near)
        .ok_or_else(|| error("particle camera far plane is invalid"))?;
    // Consume up_input and construct the full basis here so malformed camera
    // up vectors fail before any durable particle write.
    let right = normalize(cross(forward, up_input))?;
    let _up = normalize(cross(right, forward))?;
    if !z.is_finite() || z <= near || z >= far {
        return invalid("typed particle falls outside the fixed camera clip range");
    }
    Ok(((z - near) / (far - near)).clamp(0.0, 1.0) as f32)
}

fn particle_stream_q24(seed_sha256: &str, emitter_id: &str, index: usize, channel: &str) -> u32 {
    let material = format!("{seed_sha256}|{emitter_id}|{index}|{channel}");
    let digest = sha256_hex(material.as_bytes());
    u32::from_str_radix(&digest[..8], 16).unwrap_or(0) & 0x00ff_ffff
}

fn particle_stream_unit(seed_sha256: &str, emitter_id: &str, index: usize, channel: &str) -> f64 {
    f64::from(particle_stream_q24(seed_sha256, emitter_id, index, channel)) / 16_777_215.0
}

fn quantize_particle_meter(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn particle_world_anchor_position(
    anchor: &Value,
    world_transform: F64Mat4,
) -> Result<[f64; 3], RuntimeError> {
    let translation = anchor
        .get("local_translation_m")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error("typed particle Anchor local translation is unavailable"))?;
    let local = [
        finite_f64(translation.first(), "particle Anchor translation", 10.0)?,
        finite_f64(translation.get(1), "particle Anchor translation", 10.0)?,
        finite_f64(translation.get(2), "particle Anchor translation", 10.0)?,
    ];
    let rotation = anchor
        .get("local_rotation_quat_xyzw")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 4)
        .ok_or_else(|| error("typed particle Anchor local rotation is unavailable"))?;
    let quaternion = [
        finite_f64(rotation.first(), "particle Anchor rotation", 1.0)?,
        finite_f64(rotation.get(1), "particle Anchor rotation", 1.0)?,
        finite_f64(rotation.get(2), "particle Anchor rotation", 1.0)?,
        finite_f64(rotation.get(3), "particle Anchor rotation", 1.0)?,
    ];
    let norm = quaternion
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || (norm - 1.0).abs() > 1.0e-6 {
        return invalid("typed particle Anchor quaternion is not unit length");
    }
    // The anchor TRS translation locates the anchor origin in owner-Part
    // space. Its rotation orients emitted vectors; it must not rotate the
    // translation itself.
    let world = transform_f64_point(world_transform, local);
    if world
        .iter()
        .any(|value| !value.is_finite() || value.abs() > 10.0)
    {
        return invalid("typed particle world Anchor position is outside the bounded domain");
    }
    Ok(world)
}

fn derive_typed_particles(
    request: &Value,
    camera: &Value,
    anchor_set: &Value,
    appearance_sample: &Value,
    source_artifact_sha256: &str,
    node_inventory: &Value,
    world_transforms: &BTreeMap<String, F64Mat4>,
) -> Result<(String, Vec<Value>, Vec<render_worker::TypedParticle>, Value), RuntimeError> {
    let project_id = request
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed particle project is unavailable"))?;
    let delivery_sha256 = request
        .get("delivery_manifest_object_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed particle delivery is unavailable"))?;
    let profile_sha256 = request
        .get("vfx_profile_object_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed particle VFX profile is unavailable"))?;
    let anchor_sha256 = request
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed particle AnchorSet is unavailable"))?;
    let base_frame_key_sha256 = request
        .get("base_frame_key_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed particle base frame is unavailable"))?;
    let bloom_key_sha256 = request
        .get("bloom_key_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed particle HDR bloom frame is unavailable"))?;
    let sample_time_ticks = request
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("typed particle sample tick is unavailable"))?;
    let seed_material = json!({
        "schema_version":"FictionalEnergyVfxParticleSeed@1",
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_sha256,
        "vfx_profile_object_sha256":profile_sha256,
        "anchor_set_object_sha256":anchor_sha256,
        "source_artifact_sha256":source_artifact_sha256,
        "base_frame_key_sha256":base_frame_key_sha256,
        "bloom_key_sha256":bloom_key_sha256,
        "sample_time_ticks":sample_time_ticks,
        "node_inventory_sha256":node_inventory.get("canonical_sha256"),
        "particle_policy":PARTICLES_RENDER_POLICY
    });
    let seed_sha256 = canonical_json_hash(&seed_material);
    let anchors = anchor_set
        .get("anchors")
        .and_then(Value::as_array)
        .ok_or_else(|| error("typed particle AnchorSet anchors are unavailable"))?;
    let anchor_by_id = anchors
        .iter()
        .filter_map(|anchor| Some((anchor.get("anchor_id")?.as_str()?.to_owned(), anchor)))
        .collect::<BTreeMap<_, _>>();
    let effects = appearance_sample
        .get("effects")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or_else(|| error("typed particle sampled effects are unavailable"))?;
    let effect_by_id = effects
        .iter()
        .filter_map(|effect| Some((effect.get("effect_id")?.as_str()?.to_owned(), effect)))
        .collect::<BTreeMap<_, _>>();
    let emitter_definitions = [
        (
            "muzzle-burst",
            "muzzle-pulse",
            "socket-muzzle-vfx",
            "barrel-assembly",
            24_usize,
            10_000_u32,
        ),
        (
            "energy-core-sparks",
            "energy-core-breathe",
            "socket-energy-core-vfx",
            "energy-core",
            32_usize,
            20_000_u32,
        ),
    ];
    let mut particles = Vec::with_capacity(56);
    let mut typed = Vec::with_capacity(56);
    let mut owner_values = Vec::new();
    for (emitter_id, effect_id, anchor_id, owner_part_id, count, id_base) in emitter_definitions {
        let anchor = anchor_by_id
            .get(anchor_id)
            .ok_or_else(|| error(format!("typed particle Anchor {anchor_id} is unavailable")))?;
        if anchor.get("owner_part_id").and_then(Value::as_str) != Some(owner_part_id) {
            return invalid("typed particle emitter Anchor owner Part differs");
        }
        let world_transform = world_transforms
            .get(owner_part_id)
            .copied()
            .ok_or_else(|| error("typed particle owner Part world transform is unavailable"))?;
        let center = particle_world_anchor_position(anchor, world_transform)?;
        owner_values.push(json!({
            "emitter_id":emitter_id,
            "anchor_id":anchor_id,
            "owner_part_id":owner_part_id,
            "world_transform":world_transform,
            "world_anchor_position_m":center
        }));
        let effect = effect_by_id
            .get(effect_id)
            .ok_or_else(|| error(format!("typed particle effect {effect_id} is unavailable")))?;
        let color = effect
            .get("color_linear_rgb")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| error("typed particle effect color is unavailable"))?;
        let color = [
            color[0]
                .as_f64()
                .ok_or_else(|| error("particle effect color is invalid"))? as f32,
            color[1]
                .as_f64()
                .ok_or_else(|| error("particle effect color is invalid"))? as f32,
            color[2]
                .as_f64()
                .ok_or_else(|| error("particle effect color is invalid"))? as f32,
        ];
        let strength = effect
            .get("emissive_strength")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (0.0..=16.0).contains(value))
            .ok_or_else(|| error("typed particle effect strength is invalid"))?;
        for index in 0..count {
            let u = particle_stream_unit(&seed_sha256, emitter_id, index, "u");
            let v = particle_stream_unit(&seed_sha256, emitter_id, index, "v");
            let w = particle_stream_unit(&seed_sha256, emitter_id, index, "w");
            let position = if emitter_id == "muzzle-burst" {
                [
                    center[0] + 0.012 + u * 0.052,
                    center[1] + (v - 0.5) * 0.07,
                    center[2] + (w - 0.5) * 0.07,
                ]
            } else {
                [
                    center[0] + (u - 0.5) * 0.055,
                    center[1] + (v - 0.5) * 0.055,
                    center[2] + (w - 0.5) * 0.055,
                ]
            }
            .map(quantize_particle_meter);
            let depth = particle_camera_depth(camera, position)?;
            let radius_q8 = 256
                + (u64::from(particle_stream_q24(&seed_sha256, emitter_id, index, "r")) * 640
                    / 16_777_215) as u32;
            let radius_px = radius_q8 as f32 / 256.0;
            let alpha_q16 = 22_938
                + (u64::from(particle_stream_q24(&seed_sha256, emitter_id, index, "a")) * 36_044
                    / 16_777_215) as u32;
            let alpha = alpha_q16.min(65_535) as f32 / 65_535.0;
            let lifetime_ticks = 80
                + u64::from(particle_stream_q24(&seed_sha256, emitter_id, index, "l")) * 220
                    / 16_777_215;
            let id = id_base + index as u32;
            let value = json!({
                "emitter_id":emitter_id,
                "id":id,
                "position":position,
                "radius_px":radius_px,
                "color_linear_rgb":color,
                "alpha":alpha,
                "lifetime_ticks":lifetime_ticks,
                "depth":depth
            });
            particles.push(value);
            typed.push(render_worker::TypedParticle {
                emitter_id: emitter_id.to_owned(),
                id,
                position: position.map(|value| value as f32),
                radius_px,
                color_linear_rgb: color,
                alpha,
                lifetime_ticks,
                depth,
            });
        }
        let _ = strength;
    }
    if particles.len() != 56 || typed.len() != particles.len() {
        return invalid("typed particle emitter count budget differs");
    }
    let mut owner_transforms = json!({
        "schema_version":"FictionalEnergyVfxParticleOwnerWorldTransforms@1",
        "owners":owner_values,
        "canonical_sha256":""
    });
    owner_transforms["canonical_sha256"] = Value::String(canonical_json_hash(&owner_transforms));
    Ok((seed_sha256, particles, typed, owner_transforms))
}

fn derive_typed_trails(
    request: &Value,
    camera: &Value,
    _anchor_set: &Value,
    _appearance_sample: &Value,
    source_artifact_sha256: &str,
    node_inventory: &Value,
    _world_transforms: &BTreeMap<String, F64Mat4>,
    current_particle_key_sha256: &str,
    particle_history_key_sha256s: &[String],
    particle_receipts: &[Value],
) -> Result<(String, Vec<Value>, Vec<render_worker::TypedTrail>, Value), RuntimeError> {
    let project_id = request
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed trail project is unavailable"))?;
    let delivery_sha256 = request
        .get("delivery_manifest_object_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed trail delivery is unavailable"))?;
    let profile_sha256 = request
        .get("vfx_profile_object_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed trail VFX profile is unavailable"))?;
    let anchor_sha256 = request
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed trail AnchorSet is unavailable"))?;
    let base_frame_key_sha256 = request
        .get("base_frame_key_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed trail base frame is unavailable"))?;
    let bloom_key_sha256 = request
        .get("bloom_key_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("typed trail HDR bloom frame is unavailable"))?;
    let sample_time_ticks = request
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("typed trail sample tick is unavailable"))?;
    let seed_material = json!({
        "schema_version":"FictionalEnergyVfxTrailSeed@1",
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_sha256,
        "vfx_profile_object_sha256":profile_sha256,
        "anchor_set_object_sha256":anchor_sha256,
        "source_artifact_sha256":source_artifact_sha256,
        "base_frame_key_sha256":base_frame_key_sha256,
        "bloom_key_sha256":bloom_key_sha256,
        "current_particle_key_sha256":current_particle_key_sha256,
        "particle_history_key_sha256s":particle_history_key_sha256s,
        "sample_time_ticks":sample_time_ticks,
        "node_inventory_sha256":node_inventory.get("canonical_sha256"),
        "trail_policy":TRAILS_RENDER_POLICY
    });
    let seed_sha256 = canonical_json_hash(&seed_material);
    let mut particle_frames = particle_receipts
        .iter()
        .map(|receipt| {
            let particle_key = receipt
                .get("particle_key_sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| error("typed trail particle receipt key is unavailable"))?;
            let sample_time_ticks = receipt
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                .ok_or_else(|| error("typed trail particle receipt tick is unavailable"))?;
            let particles = receipt
                .get("particles")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 56)
                .ok_or_else(|| error("typed trail particle receipt inventory is invalid"))?;
            if particle_key != current_particle_key_sha256
                && !particle_history_key_sha256s
                    .iter()
                    .any(|value| value == particle_key)
            {
                return invalid("typed trail particle receipt is not bound to request history");
            }
            Ok((
                sample_time_ticks,
                particle_key == current_particle_key_sha256,
                particles,
            ))
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    particle_frames.sort_by_key(|(tick, current, _)| (*tick, !*current));
    if particle_frames.len() != particle_history_key_sha256s.len() + 1
        || !particle_frames.iter().any(|(_, current, _)| *current)
        || particle_frames
            .windows(2)
            .any(|frames| frames[0].0 == frames[1].0)
    {
        return invalid("typed trail particle history is not a unique ordered set");
    }
    let current_tick = particle_frames
        .iter()
        .find(|(_, current, _)| *current)
        .map(|(tick, _, _)| *tick)
        .ok_or_else(|| error("typed trail current particle tick is unavailable"))?;
    if particle_history_key_sha256s.len() > 0
        && particle_frames
            .iter()
            .any(|(tick, current, _)| !*current && *tick >= current_tick)
    {
        return invalid("typed trail particle history must precede the current tick");
    }
    let emitter_definitions = [
        ("muzzle-trail", "muzzle-burst", 30_000_u32),
        ("energy-core-trail", "energy-core-sparks", 31_000_u32),
    ];
    let mut trail_values = Vec::with_capacity(2);
    let mut typed = Vec::with_capacity(2);
    for (emitter_id, source_emitter_id, id) in emitter_definitions {
        let current_particles = particle_frames
            .iter()
            .find(|(_, current, _)| *current)
            .map(|(_, _, particles)| *particles)
            .ok_or_else(|| error("typed trail current particle inventory is unavailable"))?;
        let source_particle_id = current_particles
            .iter()
            .filter(|particle| {
                particle.get("emitter_id").and_then(Value::as_str) == Some(source_emitter_id)
            })
            .filter_map(|particle| particle.get("id").and_then(Value::as_u64))
            .min()
            .ok_or_else(|| error("typed trail source particle ID is unavailable"))?;
        let current_particle = current_particles
            .iter()
            .find(|particle| {
                particle.get("emitter_id").and_then(Value::as_str) == Some(source_emitter_id)
                    && particle.get("id").and_then(Value::as_u64) == Some(source_particle_id)
            })
            .ok_or_else(|| error("typed trail current source particle is unavailable"))?;
        let mut points = Vec::with_capacity(particle_frames.len());
        let mut source_ticks = Vec::with_capacity(particle_frames.len());
        for (sample_tick, _, particles) in &particle_frames {
            let particle = particles
                .iter()
                .find(|particle| {
                    particle.get("emitter_id").and_then(Value::as_str) == Some(source_emitter_id)
                        && particle.get("id").and_then(Value::as_u64) == Some(source_particle_id)
                })
                .ok_or_else(|| error("typed trail history is missing the matching particle ID"))?;
            let position = particle
                .get("position")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 3)
                .ok_or_else(|| error("typed trail matching particle position is unavailable"))?;
            let position = [
                finite_f64(position.first(), "typed trail particle position", 10.0)?,
                finite_f64(position.get(1), "typed trail particle position", 10.0)?,
                finite_f64(position.get(2), "typed trail particle position", 10.0)?,
            ];
            let _ = particle_camera_depth(camera, position)?;
            points.push(position.map(quantize_particle_meter));
            source_ticks.push(*sample_tick);
        }
        if points.len() < 2 || points.len() > 5 {
            return invalid("typed trail history point count is outside the fixed range");
        }
        let color_values = current_particle
            .get("color_linear_rgb")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 3)
            .ok_or_else(|| error("typed trail source particle color is unavailable"))?;
        let color = [
            finite_f64(color_values.first(), "typed trail source color", 1.0)? as f32,
            finite_f64(color_values.get(1), "typed trail source color", 1.0)? as f32,
            finite_f64(color_values.get(2), "typed trail source color", 1.0)? as f32,
        ];
        let radius_px = current_particle
            .get("radius_px")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (1.0..=8.0).contains(value))
            .ok_or_else(|| error("typed trail source particle radius is invalid"))?
            as f32;
        let alpha = current_particle
            .get("alpha")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .ok_or_else(|| error("typed trail source particle alpha is invalid"))?
            as f32;
        let lifetime_ticks = current_particle
            .get("lifetime_ticks")
            .and_then(Value::as_u64)
            .filter(|value| (1..=1_000_000).contains(value))
            .ok_or_else(|| error("typed trail source particle lifetime is invalid"))?;
        let typed_points = points
            .iter()
            .map(|point| point.map(|value| value as f32))
            .collect::<Vec<_>>();
        let trail_value = json!({
            "emitter_id":emitter_id,
            "id":id,
            "points":points,
            "source_particle_id":source_particle_id,
            "source_sample_time_ticks":source_ticks,
            "radius_px":radius_px,
            "color_linear_rgb":color,
            "alpha":alpha,
            "lifetime_ticks":lifetime_ticks
        });
        trail_values.push(trail_value);
        typed.push(render_worker::TypedTrail {
            emitter_id: emitter_id.to_owned(),
            id,
            points: typed_points,
            radius_px,
            color_linear_rgb: color,
            alpha,
            lifetime_ticks,
        });
    }
    if trail_values.len() != 2 || typed.len() != trail_values.len() {
        return invalid("typed trail emitter count budget differs");
    }
    let owner_transforms = particle_receipts
        .iter()
        .find(|receipt| {
            receipt.get("particle_key_sha256").and_then(Value::as_str)
                == Some(current_particle_key_sha256)
        })
        .and_then(|receipt| receipt.get("owner_world_transforms"))
        .cloned()
        .ok_or_else(|| error("typed trail current particle owner transforms are unavailable"))?;
    Ok((seed_sha256, trail_values, typed, owner_transforms))
}

pub(super) fn energy_vfx_hdr_bloom_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "vfx_profile_object_sha256",
            "base_frame_key_sha256",
            "bloom_profile",
            "bloom_policy",
            "canonical_sha256",
        ],
        "FictionalEnergyVfxHdrBloomFrameRenderPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxHdrBloomFrameRenderPrepareRequest@1"
        || text(object, "bloom_policy")? != HDR_BLOOM_RENDER_POLICY
    {
        return invalid("HDR bloom render policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?.to_owned();
    let profile_sha256 = sha(object, "vfx_profile_object_sha256")?.to_owned();
    let base_frame_key_sha256 = sha(object, "base_frame_key_sha256")?.to_owned();
    let bloom_profile_value = object
        .get("bloom_profile")
        .ok_or_else(|| error("HDR bloom profile is unavailable"))?;
    let (bloom_profile, bloom_profile_sha256) = parse_fixed_hdr_bloom_profile(bloom_profile_value)?;
    let bloom_key_sha256 = sha(object, "canonical_sha256")?.to_owned();
    if runtime
        .store
        .get_fictional_energy_vfx_bloom_frame_link(&bloom_key_sha256)?
        .is_some()
    {
        return energy_vfx_hdr_bloom_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
                "project_id":project_id,
                "bloom_key_sha256":bloom_key_sha256
            }),
        )
        .map(|value| {
            json!({
                "schema_version":"FictionalEnergyVfxHdrBloomFramePrepareResult@1",
                "bloom_key_sha256":value["bloom_key_sha256"],
                "receipt_object_sha256":value["receipt_object_sha256"],
                "receipt":value["receipt"],
                "durable_link":value["link"],
                "candidate_confirmed":false,
                "export_performed":false,
                "actual_engine_roundtrip":false,
                "quality_status":"structural_only"
            })
        });
    }

    let base = energy_vfx_rendered_frame_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":project_id,
            "frame_key_sha256":base_frame_key_sha256
        }),
    )?;
    let base_link: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(
        base.get("link")
            .cloned()
            .ok_or_else(|| error("HDR bloom base frame link is unavailable"))?,
    )
    .map_err(|source| error(format!("HDR bloom base frame link is invalid: {source}")))?;
    if base_link.project_id != project_id
        || base_link.delivery_manifest_object_sha256 != delivery_sha256
        || base_link.vfx_profile_object_sha256 != profile_sha256
        || base_link.frame_key_sha256 != base_frame_key_sha256
    {
        return invalid("HDR bloom request is not bound to the requested durable base frame");
    }
    let base_receipt = base
        .get("receipt")
        .ok_or_else(|| error("HDR bloom base frame receipt is unavailable"))?;
    if base_receipt
        .get("sampled_emissive_frame_rendered")
        .and_then(Value::as_bool)
        != Some(true)
        || base_receipt
            .get("glb_material_zone_binding_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || base_receipt
            .get("pass_artifacts")
            .and_then(Value::as_array)
            .is_none_or(|passes| passes.len() != 9)
    {
        return invalid("HDR bloom base frame is not a complete nine-AOV frame");
    }
    let base_pass_hashes = base_link.pass_object_sha256s.clone();
    if base_pass_hashes.len() != 9 {
        return invalid("HDR bloom base frame AOV inventory is not fixed");
    }
    let overrides = parse_bloom_overrides(base_receipt.get("applied_emissive_overrides"))?;
    let camera = read_json(
        runtime,
        &base_link.camera_object_sha256,
        "CameraCalibration@1",
    )?;
    if camera_identity_hash(&camera)? != base_link.camera_identity_sha256 {
        return invalid("HDR bloom base frame camera identity differs");
    }
    let glb = runtime.cas_read(&base_link.source_artifact_sha256)?;
    let first = render_worker::render_glb_vfx_bloom_frame_with_worker_identity(
        &glb,
        &camera,
        &overrides,
        bloom_profile,
    )
    .map_err(|source| error(format!("HDR bloom frame render failed: {source}")))?;
    let second = render_worker::render_glb_vfx_bloom_frame_with_worker_identity(
        &glb,
        &camera,
        &overrides,
        bloom_profile,
    )
    .map_err(|source| error(format!("HDR bloom frame replay failed: {source}")))?;
    let worker_cohort = first
        .build_cohort_sha256
        .clone()
        .ok_or_else(|| error("HDR bloom Render Worker cohort is unavailable"))?;
    if first.build_cohort_sha256 != second.build_cohort_sha256
        || first.build_cohort_sha256.as_deref()
            != Some(base_link.render_worker_build_cohort_sha256.as_str())
        || first.render_profile != second.render_profile
        || first.bloom_profile != second.bloom_profile
        || first.bloom_passes.len() != 2
        || first
            .bloom_passes
            .iter()
            .zip(&second.bloom_passes)
            .any(|(left, right)| left.pass != right.pass || left.png != right.png)
    {
        return invalid("HDR bloom Worker replay or cohort differs");
    }
    let render_profile_sha256 = first.render_profile["canonical_sha256"]
        .as_str()
        .ok_or_else(|| error("HDR bloom render profile hash is unavailable"))?;
    if render_profile_sha256 != base_link.render_profile_sha256 {
        return invalid("HDR bloom render profile differs from the durable base frame");
    }
    let base_passes = base_receipt
        .get("pass_artifacts")
        .and_then(Value::as_array)
        .ok_or_else(|| error("HDR bloom base AOV receipt is unavailable"))?;
    if base_passes.len() != 9
        || base_passes.iter().enumerate().any(|(ordinal, pass)| {
            pass.get("sha256").and_then(Value::as_str) != Some(base_pass_hashes[ordinal].as_str())
                || runtime
                    .cas_read(&base_pass_hashes[ordinal])
                    .map(|bytes| sha256_hex(&bytes) != base_pass_hashes[ordinal])
                    .unwrap_or(true)
        })
    {
        return invalid("HDR bloom base nine-AOV hashes differ from the durable frame");
    }
    if first.applied_emissive_overrides.len() != overrides.len()
        || first
            .applied_emissive_overrides
            .iter()
            .zip(&overrides)
            .any(|(actual, expected)| {
                actual.material_zone_id != expected.material_zone_id
                    || actual.material_id != expected.material_id
            })
    {
        return invalid("HDR bloom Worker MaterialZone binding differs");
    }
    if first.bloom_passes[0].pass != "emissive-source"
        || first.bloom_passes[1].pass != "bloom-contribution"
        || first
            .bloom_passes
            .iter()
            .any(|pass| pass.png.is_empty() || pass.width != 512 || pass.height != 512)
    {
        return invalid("HDR bloom pass inventory is not fixed");
    }

    let source_candidate_id = base_link.source_candidate_id.clone();
    let source_artifact_sha256 = base_link.source_artifact_sha256.clone();
    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::<CasObject>::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let source_object = runtime.store.put_object_reserved(
            &reservation,
            &first.bloom_passes[0].png,
            None,
            "image/png",
            "fictional-energy-vfx-bloom-source",
            &now_string(),
        )?;
        reserved_objects.push(source_object.clone());
        let contribution_object = runtime.store.put_object_reserved(
            &reservation,
            &first.bloom_passes[1].png,
            None,
            "image/png",
            "fictional-energy-vfx-bloom-contribution",
            &now_string(),
        )?;
        reserved_objects.push(contribution_object.clone());
        let render_set = seal_sidecar(json!({
            "schema_version":HDR_BLOOM_RENDER_SET_SCHEMA,
            "render_set_id":format!("vfx-bloom-{}", &bloom_key_sha256[..32]),
            "bloom_key_sha256":bloom_key_sha256,
            "base_frame_key_sha256":base_frame_key_sha256,
            "candidate_id":source_candidate_id,
            "artifact_sha256":source_artifact_sha256,
            "effect_materialization_policy":"independent-effect-material-zone@1",
            "camera_hash":base_link.camera_identity_sha256,
            "camera_object_sha256":base_link.camera_object_sha256,
            "renderer_hash":sha256_hex(b"forgecad-renderer-2"),
            "render_profile":first.render_profile,
            "render_profile_sha256":render_profile_sha256,
            "bloom_profile":fixed_hdr_bloom_profile_value(),
            "bloom_profile_sha256":bloom_profile_sha256,
            "render_worker_build_cohort_sha256":worker_cohort,
            "render_worker_binding_status":"same_cohort_verified",
            "base_aov_passes":base_pass_hashes,
            "width":512,
            "height":512,
            "passes":["emissive-source","bloom-contribution"],
            "pass_artifacts":{
                "emissive-source":bloom_pass_metadata(&source_object, "emissive-source"),
                "bloom-contribution":bloom_pass_metadata(&contribution_object, "bloom-contribution")
            },
            "canonical_sha256":""
        }))?;
        let render_set_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&render_set).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-bloom-render-set",
            &now_string(),
        )?;
        reserved_objects.push(render_set_object.clone());
        let receipt = seal_sidecar(json!({
            "schema_version":HDR_BLOOM_RECEIPT_SCHEMA,
            "bloom_key_sha256":bloom_key_sha256,
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256,
            "vfx_profile_object_sha256":profile_sha256,
            "source_candidate_id":source_candidate_id,
            "source_artifact_sha256":source_artifact_sha256,
            "sample_request_sha256":base_link.sample_request_sha256,
            "base_frame_key_sha256":base_frame_key_sha256,
            "camera_identity_sha256":base_link.camera_identity_sha256,
            "camera_object_sha256":base_link.camera_object_sha256,
            "render_profile_sha256":render_profile_sha256,
            "render_worker_build_cohort_sha256":worker_cohort,
            "bloom_profile":fixed_hdr_bloom_profile_value(),
            "bloom_profile_sha256":bloom_profile_sha256,
            "render_set_object_sha256":render_set_object.record.sha256,
            "base_aov_passes":base_pass_hashes,
            "source_object_sha256":source_object.record.sha256,
            "contribution_object_sha256":contribution_object.record.sha256,
            "source_pass":bloom_pass_metadata(&source_object, "emissive-source"),
            "contribution_pass":bloom_pass_metadata(&contribution_object, "bloom-contribution"),
            "base_aov_byte_exact_verified":true,
            "hdr_emissive_source_rendered":true,
            "hdr_bloom_contribution_rendered":true,
            "bloom_rendered":true,
            "runtime_write_performed":true,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only",
            "limitations":[
                "base-nine-aov-byte-exact-reused",
                "no-particles-or-trails",
                "no-commercial-engine-roundtrip",
                "no-visual-quality-or-likeness-pass"
            ],
            "canonical_sha256":""
        }))?;
        let receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&receipt).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-bloom-frame-receipt",
            &now_string(),
        )?;
        reserved_objects.push(receipt_object.clone());
        let mut link = FictionalEnergyVfxBloomFrameLinkRecord {
            schema_version: "FictionalEnergyVfxBloomFrameLink@1".to_owned(),
            bloom_key_sha256: bloom_key_sha256.clone(),
            project_id: project_id.clone(),
            delivery_manifest_object_sha256: delivery_sha256.clone(),
            vfx_profile_object_sha256: profile_sha256.clone(),
            source_candidate_id: source_candidate_id.clone(),
            source_artifact_sha256: source_artifact_sha256.clone(),
            sample_request_sha256: base_link.sample_request_sha256.clone(),
            base_frame_key_sha256: base_frame_key_sha256.clone(),
            camera_object_sha256: base_link.camera_object_sha256.clone(),
            camera_identity_sha256: base_link.camera_identity_sha256.clone(),
            render_profile_sha256: render_profile_sha256.to_owned(),
            render_worker_build_cohort_sha256: worker_cohort.clone(),
            bloom_profile_sha256: bloom_profile_sha256.clone(),
            render_set_object_sha256: render_set_object.record.sha256.clone(),
            receipt_object_sha256: receipt_object.record.sha256.clone(),
            source_object_sha256: source_object.record.sha256.clone(),
            contribution_object_sha256: contribution_object.record.sha256.clone(),
            materialization_status: "runtime-owned-durable-fictional-energy-vfx-hdr-bloom-frame"
                .to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        link.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&link).map_err(|source| error(source.to_string()))?,
        );
        let durable_link = runtime
            .store
            .record_fictional_energy_vfx_bloom_frame_link(&link)?;
        Ok(json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFramePrepareResult@1",
            "bloom_key_sha256":bloom_key_sha256,
            "receipt_object_sha256":receipt_object.record.sha256,
            "receipt":receipt,
            "durable_link":durable_link,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }))
    })();
    match operation {
        Ok(value) => {
            for object in &reserved_objects {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(operation_error) => {
            let mut rollback_error = None;
            for object in reserved_objects.iter().rev() {
                if let Err(error) =
                    runtime
                        .store
                        .release_cas_reservation_object(&reservation, object, true)
                {
                    rollback_error = Some(error.to_string());
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(error(format!(
                    "HDR bloom frame failed ({operation_error}); reservation rollback also failed ({rollback_error})"
                )));
            }
            Err(operation_error)
        }
    }
}

pub(super) fn energy_vfx_hdr_bloom_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &["schema_version", "project_id", "bloom_key_sha256"],
        "FictionalEnergyVfxHdrBloomFrameGetRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxHdrBloomFrameGetRequest@1" {
        return invalid("HDR bloom frame get schema differs");
    }
    let project_id = identifier(object, "project_id")?;
    let bloom_key_sha256 = sha(object, "bloom_key_sha256")?;
    let link = runtime
        .store
        .get_fictional_energy_vfx_bloom_frame_link(bloom_key_sha256)?
        .ok_or_else(|| error("durable HDR bloom frame is unavailable"))?;
    if link.project_id != project_id {
        return invalid("durable HDR bloom frame belongs to another project");
    }
    let base = energy_vfx_rendered_frame_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":project_id,
            "frame_key_sha256":link.base_frame_key_sha256
        }),
    )?;
    let base_link: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(
        base.get("link")
            .cloned()
            .ok_or_else(|| error("durable HDR bloom base frame link is unavailable"))?,
    )
    .map_err(|source| error(format!("durable HDR bloom base link is invalid: {source}")))?;
    let receipt = read_json(
        runtime,
        &link.receipt_object_sha256,
        HDR_BLOOM_RECEIPT_SCHEMA,
    )?;
    let render_set = read_json(
        runtime,
        &link.render_set_object_sha256,
        HDR_BLOOM_RENDER_SET_SCHEMA,
    )?;
    let (_profile, expected_profile_sha256) =
        parse_fixed_hdr_bloom_profile(&receipt["bloom_profile"])?;
    if receipt.get("bloom_key_sha256").and_then(Value::as_str) != Some(bloom_key_sha256)
        || receipt.get("project_id").and_then(Value::as_str) != Some(project_id)
        || receipt
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(link.delivery_manifest_object_sha256.as_str())
        || receipt
            .get("vfx_profile_object_sha256")
            .and_then(Value::as_str)
            != Some(link.vfx_profile_object_sha256.as_str())
        || receipt.get("source_candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || receipt
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || receipt.get("sample_request_sha256").and_then(Value::as_str)
            != Some(link.sample_request_sha256.as_str())
        || receipt.get("base_frame_key_sha256").and_then(Value::as_str)
            != Some(link.base_frame_key_sha256.as_str())
        || receipt.get("camera_object_sha256").and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || receipt
            .get("camera_identity_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || receipt.get("render_profile_sha256").and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || receipt
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || receipt.get("bloom_profile_sha256").and_then(Value::as_str)
            != Some(link.bloom_profile_sha256.as_str())
        || expected_profile_sha256 != link.bloom_profile_sha256
        || receipt
            .get("render_set_object_sha256")
            .and_then(Value::as_str)
            != Some(link.render_set_object_sha256.as_str())
        || receipt.get("source_object_sha256").and_then(Value::as_str)
            != Some(link.source_object_sha256.as_str())
        || receipt
            .get("contribution_object_sha256")
            .and_then(Value::as_str)
            != Some(link.contribution_object_sha256.as_str())
        || receipt
            .get("base_aov_byte_exact_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("hdr_emissive_source_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("hdr_bloom_contribution_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("bloom_rendered").and_then(Value::as_bool) != Some(true)
        || receipt
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || receipt.get("export_performed").and_then(Value::as_bool) != Some(false)
        || receipt
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
        || receipt.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || receipt.get("base_aov_passes") != Some(&json!(base_link.pass_object_sha256s))
    {
        return invalid("durable HDR bloom receipt binding differs");
    }
    if render_set.get("schema_version").and_then(Value::as_str) != Some(HDR_BLOOM_RENDER_SET_SCHEMA)
        || render_set.get("bloom_key_sha256").and_then(Value::as_str) != Some(bloom_key_sha256)
        || render_set
            .get("base_frame_key_sha256")
            .and_then(Value::as_str)
            != Some(link.base_frame_key_sha256.as_str())
        || render_set.get("candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || render_set.get("artifact_sha256").and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || render_set.get("camera_hash").and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || render_set
            .get("camera_object_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || render_set
            .get("render_profile_sha256")
            .and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || render_set
            .get("bloom_profile_sha256")
            .and_then(Value::as_str)
            != Some(link.bloom_profile_sha256.as_str())
        || render_set
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || render_set.get("base_aov_passes") != Some(&json!(base_link.pass_object_sha256s))
        || render_set.get("passes") != Some(&json!(["emissive-source", "bloom-contribution"]))
    {
        return invalid("durable HDR bloom RenderSet binding differs");
    }
    if link.base_frame_key_sha256 != base_link.frame_key_sha256
        || link.delivery_manifest_object_sha256 != base_link.delivery_manifest_object_sha256
        || link.vfx_profile_object_sha256 != base_link.vfx_profile_object_sha256
        || link.source_candidate_id != base_link.source_candidate_id
        || link.source_artifact_sha256 != base_link.source_artifact_sha256
        || link.camera_object_sha256 != base_link.camera_object_sha256
        || link.camera_identity_sha256 != base_link.camera_identity_sha256
        || link.render_profile_sha256 != base_link.render_profile_sha256
        || link.render_worker_build_cohort_sha256 != base_link.render_worker_build_cohort_sha256
    {
        return invalid("durable HDR bloom/base frame link binding differs");
    }
    let source_pass = receipt
        .get("source_pass")
        .ok_or_else(|| error("durable HDR bloom source pass is unavailable"))?;
    let contribution_pass = receipt
        .get("contribution_pass")
        .ok_or_else(|| error("durable HDR bloom contribution pass is unavailable"))?;
    if source_pass.get("sha256").and_then(Value::as_str) != Some(link.source_object_sha256.as_str())
        || contribution_pass.get("sha256").and_then(Value::as_str)
            != Some(link.contribution_object_sha256.as_str())
        || render_set
            .get("pass_artifacts")
            .and_then(Value::as_object)
            .and_then(|passes| passes.get("emissive-source"))
            != Some(source_pass)
        || render_set
            .get("pass_artifacts")
            .and_then(Value::as_object)
            .and_then(|passes| passes.get("bloom-contribution"))
            != Some(contribution_pass)
    {
        return invalid("durable HDR bloom pass bindings differ");
    }
    Ok(json!({
        "schema_version":"FictionalEnergyVfxHdrBloomFrameGetResult@1",
        "bloom_key_sha256":bloom_key_sha256,
        "receipt_object_sha256":link.receipt_object_sha256,
        "receipt":receipt,
        "render_set":render_set,
        "link":link,
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn energy_vfx_particles_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "vfx_profile_object_sha256",
            "anchor_set_object_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "sample_time_ticks",
            "particle_policy",
            "emitter_policy",
            "render_policy",
            "canonical_sha256",
        ],
        "FictionalEnergyVfxParticlesFrameRenderPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxParticlesFrameRenderPrepareRequest@1"
        || text(object, "particle_policy")? != "two-closed-emitters-hash-seeded-typed-attributes@1"
        || text(object, "emitter_policy")? != "muzzle-burst-24-energy-core-sparks-32@1"
        || text(object, "render_policy")? != PARTICLES_RENDER_POLICY
    {
        return invalid("typed particle policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?.to_owned();
    let profile_sha256 = sha(object, "vfx_profile_object_sha256")?.to_owned();
    let anchor_sha256 = sha(object, "anchor_set_object_sha256")?.to_owned();
    let base_frame_key_sha256 = sha(object, "base_frame_key_sha256")?.to_owned();
    let bloom_key_sha256 = sha(object, "bloom_key_sha256")?.to_owned();
    let sample_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| error("typed particle sample tick is invalid"))?;
    let particle_key_sha256 = sha(object, "canonical_sha256")?.to_owned();
    if runtime
        .store
        .get_fictional_energy_vfx_particles_frame_link(&particle_key_sha256)?
        .is_some()
    {
        return energy_vfx_particles_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxParticlesFrameGetRequest@1",
                "project_id":project_id,
                "particle_key_sha256":particle_key_sha256
            }),
        )
        .map(|value| {
            json!({
                "schema_version":"FictionalEnergyVfxParticlesFramePrepareResult@1",
                "particle_key_sha256":value["particle_key_sha256"],
                "receipt_object_sha256":value["receipt_object_sha256"],
                "receipt":value["receipt"],
                "durable_link":value["link"],
                "candidate_confirmed":false,
                "export_performed":false,
                "actual_engine_roundtrip":false,
                "quality_status":"structural_only"
            })
        });
    }

    let durable_vfx = energy_vfx_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    if durable_vfx
        .get("link")
        .and_then(|value| value.get("vfx_profile_object_sha256"))
        .and_then(Value::as_str)
        != Some(profile_sha256.as_str())
        || durable_vfx
            .get("link")
            .and_then(|value| value.get("anchor_set_object_sha256"))
            .and_then(Value::as_str)
            != Some(anchor_sha256.as_str())
    {
        return invalid("typed particle VFX profile or AnchorSet binding differs");
    }
    let anchor = weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256
        }),
    )?;
    if anchor
        .get("link")
        .and_then(|value| value.get("anchor_set_object_sha256"))
        .and_then(Value::as_str)
        != Some(anchor_sha256.as_str())
    {
        return invalid("typed particle AnchorSet binding differs");
    }
    let base = energy_vfx_rendered_frame_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":project_id,
            "frame_key_sha256":base_frame_key_sha256
        }),
    )?;
    let base_link: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(
        base.get("link")
            .cloned()
            .ok_or_else(|| error("typed particle base frame link is unavailable"))?,
    )
    .map_err(|source| {
        error(format!(
            "typed particle base frame link is invalid: {source}"
        ))
    })?;
    let base_receipt = base
        .get("receipt")
        .ok_or_else(|| error("typed particle base frame receipt is unavailable"))?;
    if base_receipt
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        != Some(sample_time_ticks)
        || base_link.vfx_profile_object_sha256 != profile_sha256
        || base_link.delivery_manifest_object_sha256 != delivery_sha256
    {
        return invalid("typed particle base frame tick or binding differs");
    }
    let bloom = energy_vfx_hdr_bloom_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
            "project_id":project_id,
            "bloom_key_sha256":bloom_key_sha256
        }),
    )?;
    let bloom_link: FictionalEnergyVfxBloomFrameLinkRecord = serde_json::from_value(
        bloom
            .get("link")
            .cloned()
            .ok_or_else(|| error("typed particle HDR bloom link is unavailable"))?,
    )
    .map_err(|source| {
        error(format!(
            "typed particle HDR bloom link is invalid: {source}"
        ))
    })?;
    if bloom_link.base_frame_key_sha256 != base_frame_key_sha256
        || bloom_link.delivery_manifest_object_sha256 != delivery_sha256
        || bloom_link.vfx_profile_object_sha256 != profile_sha256
        || bloom_link.camera_identity_sha256 != base_link.camera_identity_sha256
        || bloom_link.render_worker_build_cohort_sha256
            != base_link.render_worker_build_cohort_sha256
    {
        return invalid("typed particle HDR bloom/base binding differs");
    }
    let base_camera = read_json(
        runtime,
        &base_link.camera_object_sha256,
        "CameraCalibration@1",
    )?;
    if camera_identity_hash(&base_camera)? != base_link.camera_identity_sha256 {
        return invalid("typed particle camera identity differs");
    }
    let appearance_sample = energy_vfx_appearance_frame_sample(
        runtime,
        &seal_request(json!({
            "schema_version":"FictionalEnergyVfxAppearanceFrameSampleRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256,
            "vfx_profile_object_sha256":profile_sha256,
            "sample_time_ticks":sample_time_ticks,
            "sampling_policy":"integer-tick-linear-once-clamp-loop-modulo-duration@1",
            "appearance_binding_policy":"three-lod-appearance-program-glb-material-zone-stable-id@1",
            "canonical_sha256":""
        }))?,
    )?;
    let source_artifact_sha256 = base_link.source_artifact_sha256.clone();
    let source_glb = runtime.cas_read(&source_artifact_sha256)?;
    let part_ids = anchor
        .get("anchor_set")
        .and_then(|value| value.get("part_ids"))
        .and_then(Value::as_array)
        .ok_or_else(|| error("typed particle AnchorSet Part inventory is unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| error("typed particle Part ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (node_inventory, world_transforms) = extract_part_world_nodes(&source_glb, &part_ids)?;
    let (particle_seed_sha256, particle_values, typed_particles, owner_transforms) =
        derive_typed_particles(
            request,
            &base_camera,
            anchor
                .get("anchor_set")
                .ok_or_else(|| error("typed particle AnchorSet is unavailable"))?,
            &appearance_sample,
            &source_artifact_sha256,
            &node_inventory,
            &world_transforms,
        )?;
    let particle_inventory: Value = serde_json::from_slice(
        &canonical_json_bytes(&Value::Array(particle_values.clone()))
            .map_err(|source| error(source.to_string()))?,
    )
    .map_err(|_| error("typed particle inventory normalization failed"))?;
    let particle_inventory_sha256 = canonical_json_hash(&particle_inventory);
    let particle_id_encoding = json!({
        "schema_version":"FictionalEnergyVfxParticleIdEncoding@1",
        "encoding":"lossless-little-endian-rgb24-id-plus-one-alpha-visible@1",
        "background_rgba8":[0,0,0,0],
        "id_range":[1,65535]
    });
    let particle_id_encoding_sha256 = canonical_json_hash(&particle_id_encoding);
    let first = render_worker::render_typed_particles_with_worker_identity(
        &source_glb,
        &base_camera,
        &typed_particles,
        &particle_seed_sha256,
    )
    .map_err(|source| error(format!("typed particle render failed: {source}")))?;
    let second = render_worker::render_typed_particles_with_worker_identity(
        &source_glb,
        &base_camera,
        &typed_particles,
        &particle_seed_sha256,
    )
    .map_err(|source| error(format!("typed particle replay failed: {source}")))?;
    if first.build_cohort_sha256.is_none()
        || first.build_cohort_sha256 != second.build_cohort_sha256
        || first.render_profile != second.render_profile
        || first.seed_sha256 != particle_seed_sha256
        || second.seed_sha256 != particle_seed_sha256
        || first.particle_count != 56
        || second.particle_count != 56
        || first.emitter_counts != [24, 32]
        || second.emitter_counts != [24, 32]
        || first.particle_passes.len() != 3
        || first
            .particle_passes
            .iter()
            .zip(&second.particle_passes)
            .any(|(left, right)| {
                left.pass != right.pass
                    || left.png != right.png
                    || left.width != 512
                    || left.height != 512
            })
    {
        return invalid("typed particle replay or Worker cohort differs");
    }
    let worker_cohort = first
        .build_cohort_sha256
        .clone()
        .ok_or_else(|| error("typed particle Render Worker cohort is unavailable"))?;
    let render_profile_sha256 = first.render_profile["canonical_sha256"]
        .as_str()
        .ok_or_else(|| error("typed particle render profile hash is unavailable"))?
        .to_owned();
    if render_profile_sha256 != base_link.render_profile_sha256 {
        return invalid("typed particle render profile differs from base frame");
    }
    let node_inventory_sha256 = node_inventory["canonical_sha256"]
        .as_str()
        .ok_or_else(|| error("typed particle node inventory hash is unavailable"))?
        .to_owned();
    let owner_world_transform_sha256 = owner_transforms["canonical_sha256"]
        .as_str()
        .ok_or_else(|| error("typed particle owner transform hash is unavailable"))?
        .to_owned();
    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::<CasObject>::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let mut pass_artifacts = Map::new();
        let mut receipt_passes = Vec::new();
        let mut pass_hashes = Vec::new();
        for (ordinal, pass) in first.particle_passes.iter().enumerate() {
            let object_kind = match pass.pass.as_str() {
                "particle-color" => "fictional-energy-vfx-particles-color",
                "particle-id" => "fictional-energy-vfx-particles-id",
                "particle-depth" => "fictional-energy-vfx-particles-depth",
                _ => return invalid("typed particle pass inventory differs"),
            };
            let pass_object = runtime.store.put_object_reserved(
                &reservation,
                &pass.png,
                None,
                "image/png",
                object_kind,
                &now_string(),
            )?;
            let metadata = json!({
                "sha256":pass_object.record.sha256,
                "mime":"image/png",
                "size_bytes":pass_object.record.size_bytes,
                "width":512,
                "height":512,
                "channels":"rgba8",
                "color_space":"data"
            });
            pass_artifacts.insert(pass.pass.clone(), metadata.clone());
            receipt_passes.push(json!({
                "pass":pass.pass,
                "ordinal":ordinal,
                "sha256":metadata["sha256"],
                "mime":metadata["mime"],
                "size_bytes":metadata["size_bytes"],
                "width":512,
                "height":512,
                "channels":"rgba8",
                "color_space":"data"
            }));
            pass_hashes.push(pass_object.record.sha256.clone());
            reserved_objects.push(pass_object);
        }
        let mut render_set = json!({
            "schema_version":PARTICLES_RENDER_SET_SCHEMA,
            "render_set_id":format!("vfx-particles-{}", &particle_key_sha256[..32]),
            "particle_key_sha256":particle_key_sha256,
            "base_frame_key_sha256":base_frame_key_sha256,
            "bloom_key_sha256":bloom_key_sha256,
            "candidate_id":base_link.source_candidate_id,
            "artifact_sha256":source_artifact_sha256,
            "camera_hash":base_link.camera_identity_sha256,
            "camera_object_sha256":base_link.camera_object_sha256,
            "render_profile_sha256":render_profile_sha256,
            "render_worker_build_cohort_sha256":worker_cohort,
            "particle_seed_sha256":particle_seed_sha256,
            "particle_inventory_sha256":particle_inventory_sha256,
            "particle_id_encoding_sha256":particle_id_encoding_sha256,
            "node_inventory_sha256":node_inventory_sha256,
            "owner_world_transform_sha256":owner_world_transform_sha256,
            "particle_policy":"two-closed-emitters-hash-seeded-typed-attributes@1",
            "emitter_policy":"muzzle-burst-24-energy-core-sparks-32@1",
            "passes":["particle-color","particle-id","particle-depth"],
            "pass_artifacts":pass_artifacts,
            "base_aov_passes":base_link.pass_object_sha256s,
            "bloom_passes":[bloom_link.source_object_sha256,bloom_link.contribution_object_sha256],
            "canonical_sha256":""
        });
        render_set["canonical_sha256"] = Value::String(canonical_json_hash(&render_set));
        let render_set_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&render_set).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-particles-render-set",
            &now_string(),
        )?;
        reserved_objects.push(render_set_object.clone());
        let mut receipt_fields = Map::new();
        for (key, value) in [
            ("schema_version", json!(PARTICLES_RECEIPT_SCHEMA)),
            ("particle_key_sha256", json!(particle_key_sha256)),
            ("project_id", json!(project_id)),
            ("delivery_manifest_object_sha256", json!(delivery_sha256)),
            ("vfx_profile_object_sha256", json!(profile_sha256)),
            ("anchor_set_object_sha256", json!(anchor_sha256)),
            ("source_candidate_id", json!(base_link.source_candidate_id)),
            ("source_artifact_sha256", json!(source_artifact_sha256)),
            ("sample_request_sha256", json!(particle_key_sha256)),
            ("sample_time_ticks", json!(sample_time_ticks)),
            ("base_frame_key_sha256", json!(base_frame_key_sha256)),
            ("bloom_key_sha256", json!(bloom_key_sha256)),
            (
                "camera_object_sha256",
                json!(base_link.camera_object_sha256),
            ),
            (
                "camera_identity_sha256",
                json!(base_link.camera_identity_sha256),
            ),
            ("render_profile_sha256", json!(render_profile_sha256)),
            ("render_worker_build_cohort_sha256", json!(worker_cohort)),
            ("particle_seed_sha256", json!(particle_seed_sha256)),
            (
                "particle_inventory_sha256",
                json!(particle_inventory_sha256),
            ),
            (
                "particle_id_encoding_sha256",
                json!(particle_id_encoding_sha256),
            ),
            ("particle_id_encoding", particle_id_encoding.clone()),
            (
                "seed_policy",
                json!("durable-hash-concatenation-sha256-no-caller-rng@1"),
            ),
            (
                "simulation_quantization",
                json!({
                    "hash_stream":"sha256-q24@1",
                    "position_m":"signed-micrometer-round-nearest@1",
                    "radius_px":"q8-unsigned@1",
                    "alpha":"q16-unsigned@1",
                    "lifetime_ticks":"integer@1",
                    "sort_order":"emitter-definition-then-spawn-ordinal@1"
                }),
            ),
            ("node_inventory_sha256", json!(node_inventory_sha256)),
            ("node_inventory", node_inventory.clone()),
            (
                "owner_world_transform_sha256",
                json!(owner_world_transform_sha256),
            ),
            ("owner_world_transforms", owner_transforms.clone()),
            (
                "particle_policy",
                json!("two-closed-emitters-hash-seeded-typed-attributes@1"),
            ),
            (
                "emitter_policy",
                json!("muzzle-burst-24-energy-core-sparks-32@1"),
            ),
            ("particle_count", json!(typed_particles.len())),
            (
                "emitter_counts",
                json!({"muzzle-burst":24,"energy-core-sparks":32}),
            ),
            ("particles", particle_inventory.clone()),
            ("pass_artifacts", Value::Array(receipt_passes.clone())),
            ("base_aov_passes", json!(base_link.pass_object_sha256s)),
            (
                "bloom_passes",
                json!([
                    bloom_link.source_object_sha256,
                    bloom_link.contribution_object_sha256
                ]),
            ),
            ("base_aov_byte_exact_verified", json!(true)),
            ("bloom_pass_byte_exact_reused", json!(true)),
            ("opaque_geometry_depth_tested", json!(true)),
            ("typed_particle_attributes_verified", json!(true)),
            ("typed_particles_rendered", json!(true)),
            ("runtime_write_performed", json!(true)),
            ("candidate_confirmed", json!(false)),
            ("export_performed", json!(false)),
            ("actual_engine_roundtrip", json!(false)),
            ("quality_status", json!("structural_only")),
            (
                "limitations",
                json!([
                    "single-deterministic-particle-frame-not-animation-sequence",
                    "no-trails",
                    "no-commercial-engine-roundtrip",
                    "no-visual-quality-or-likeness-pass"
                ]),
            ),
            ("canonical_sha256", json!("")),
        ] {
            receipt_fields.insert(key.to_owned(), value);
        }
        let receipt = seal_sidecar(Value::Object(receipt_fields))?;
        let receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&receipt).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-particles-frame-receipt",
            &now_string(),
        )?;
        reserved_objects.push(receipt_object.clone());
        let mut link = FictionalEnergyVfxParticlesFrameLinkRecord {
            schema_version: "FictionalEnergyVfxParticlesFrameLink@1".to_owned(),
            particle_key_sha256: particle_key_sha256.clone(),
            project_id: project_id.clone(),
            delivery_manifest_object_sha256: delivery_sha256.clone(),
            vfx_profile_object_sha256: profile_sha256.clone(),
            anchor_set_object_sha256: anchor_sha256.clone(),
            source_candidate_id: base_link.source_candidate_id.clone(),
            source_artifact_sha256: source_artifact_sha256.clone(),
            sample_request_sha256: particle_key_sha256.clone(),
            base_frame_key_sha256: base_frame_key_sha256.clone(),
            bloom_key_sha256: bloom_key_sha256.clone(),
            camera_object_sha256: base_link.camera_object_sha256.clone(),
            camera_identity_sha256: base_link.camera_identity_sha256.clone(),
            render_profile_sha256: render_profile_sha256.clone(),
            render_worker_build_cohort_sha256: worker_cohort.clone(),
            particle_seed_sha256: particle_seed_sha256.clone(),
            node_inventory_sha256: node_inventory_sha256.clone(),
            owner_world_transform_sha256: owner_world_transform_sha256.clone(),
            render_set_object_sha256: render_set_object.record.sha256.clone(),
            receipt_object_sha256: receipt_object.record.sha256.clone(),
            particle_color_object_sha256: pass_hashes[0].clone(),
            particle_id_object_sha256: pass_hashes[1].clone(),
            particle_depth_object_sha256: pass_hashes[2].clone(),
            materialization_status:
                "runtime-owned-durable-fictional-energy-vfx-typed-particles-frame".to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        link.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&link).map_err(|source| error(source.to_string()))?,
        );
        let durable_link = runtime
            .store
            .record_fictional_energy_vfx_particles_frame_link(&link)?;
        Ok(json!({
            "schema_version":"FictionalEnergyVfxParticlesFramePrepareResult@1",
            "particle_key_sha256":particle_key_sha256,
            "receipt_object_sha256":receipt_object.record.sha256,
            "receipt":receipt,
            "durable_link":durable_link,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }))
    })();
    match operation {
        Ok(value) => {
            for object in &reserved_objects {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(operation_error) => {
            let mut rollback_error = None;
            for object in reserved_objects.iter().rev() {
                if let Err(error) =
                    runtime
                        .store
                        .release_cas_reservation_object(&reservation, object, true)
                {
                    rollback_error = Some(error.to_string());
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(error(format!("typed particle frame failed ({operation_error}); reservation rollback also failed ({rollback_error})")));
            }
            Err(operation_error)
        }
    }
}

pub(super) fn energy_vfx_particles_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &["schema_version", "project_id", "particle_key_sha256"],
        "FictionalEnergyVfxParticlesFrameGetRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxParticlesFrameGetRequest@1" {
        return invalid("typed particle get schema differs");
    }
    let project_id = identifier(object, "project_id")?;
    let particle_key_sha256 = sha(object, "particle_key_sha256")?;
    let link = runtime
        .store
        .get_fictional_energy_vfx_particles_frame_link(particle_key_sha256)?
        .ok_or_else(|| error("durable typed particle frame is unavailable"))?;
    if link.project_id != project_id {
        return invalid("durable typed particle frame belongs to another project");
    }
    let base = energy_vfx_rendered_frame_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":project_id,
            "frame_key_sha256":link.base_frame_key_sha256
        }),
    )?;
    let bloom = energy_vfx_hdr_bloom_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
            "project_id":project_id,
            "bloom_key_sha256":link.bloom_key_sha256
        }),
    )?;
    let base_link: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(
        base.get("link")
            .cloned()
            .ok_or_else(|| error("durable typed particle base link is unavailable"))?,
    )
    .map_err(|source| {
        error(format!(
            "durable typed particle base link is invalid: {source}"
        ))
    })?;
    let bloom_link: FictionalEnergyVfxBloomFrameLinkRecord = serde_json::from_value(
        bloom
            .get("link")
            .cloned()
            .ok_or_else(|| error("durable typed particle Bloom link is unavailable"))?,
    )
    .map_err(|source| {
        error(format!(
            "durable typed particle Bloom link is invalid: {source}"
        ))
    })?;
    let receipt = read_json(
        runtime,
        &link.receipt_object_sha256,
        PARTICLES_RECEIPT_SCHEMA,
    )?;
    let render_set = read_json(
        runtime,
        &link.render_set_object_sha256,
        PARTICLES_RENDER_SET_SCHEMA,
    )?;
    let expected_pass_hashes = [
        link.particle_color_object_sha256.as_str(),
        link.particle_id_object_sha256.as_str(),
        link.particle_depth_object_sha256.as_str(),
    ];
    let receipt_passes = receipt
        .get("pass_artifacts")
        .and_then(Value::as_array)
        .filter(|passes| passes.len() == 3)
        .ok_or_else(|| error("durable typed particle pass inventory is invalid"))?;
    let expected_names = ["particle-color", "particle-id", "particle-depth"];
    for (ordinal, ((pass, expected_name), expected_hash)) in receipt_passes
        .iter()
        .zip(expected_names)
        .zip(expected_pass_hashes)
        .enumerate()
    {
        if pass.get("pass").and_then(Value::as_str) != Some(expected_name)
            || pass.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
            || pass.get("sha256").and_then(Value::as_str) != Some(expected_hash)
            || pass.get("mime").and_then(Value::as_str) != Some("image/png")
            || pass.get("width").and_then(Value::as_u64) != Some(512)
            || pass.get("height").and_then(Value::as_u64) != Some(512)
            || sha256_hex(&runtime.cas_read(expected_hash)?) != expected_hash
            || render_set
                .get("pass_artifacts")
                .and_then(Value::as_object)
                .and_then(|passes| passes.get(expected_name))
                .and_then(|metadata| metadata.get("sha256"))
                .and_then(Value::as_str)
                != Some(expected_hash)
        {
            return invalid("durable typed particle pass binding differs");
        }
    }
    let node_inventory = receipt
        .get("node_inventory")
        .ok_or_else(|| error("durable typed particle node inventory is unavailable"))?;
    let owner_transforms = receipt
        .get("owner_world_transforms")
        .ok_or_else(|| error("durable typed particle owner transforms are unavailable"))?;
    verify_value_canonical(node_inventory)?;
    verify_value_canonical(owner_transforms)?;
    let particle_inventory_sha256 = canonical_json_hash(
        receipt
            .get("particles")
            .ok_or_else(|| error("durable typed particle inventory is unavailable"))?,
    );
    let particle_id_encoding_sha256 = canonical_json_hash(
        receipt
            .get("particle_id_encoding")
            .ok_or_else(|| error("durable typed particle ID encoding is unavailable"))?,
    );
    validate_particle_semantics(&receipt, &render_set)?;
    if receipt.get("particle_key_sha256").and_then(Value::as_str)
        != Some(link.particle_key_sha256.as_str())
        || receipt.get("project_id").and_then(Value::as_str) != Some(link.project_id.as_str())
        || receipt
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(link.delivery_manifest_object_sha256.as_str())
        || receipt
            .get("vfx_profile_object_sha256")
            .and_then(Value::as_str)
            != Some(link.vfx_profile_object_sha256.as_str())
        || receipt
            .get("anchor_set_object_sha256")
            .and_then(Value::as_str)
            != Some(link.anchor_set_object_sha256.as_str())
        || receipt.get("source_candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || receipt
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || receipt.get("base_frame_key_sha256").and_then(Value::as_str)
            != Some(link.base_frame_key_sha256.as_str())
        || receipt.get("bloom_key_sha256").and_then(Value::as_str)
            != Some(link.bloom_key_sha256.as_str())
        || receipt.get("camera_object_sha256").and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || receipt
            .get("camera_identity_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || receipt.get("render_profile_sha256").and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || receipt
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || receipt.get("particle_seed_sha256").and_then(Value::as_str)
            != Some(link.particle_seed_sha256.as_str())
        || receipt
            .get("particle_inventory_sha256")
            .and_then(Value::as_str)
            != Some(particle_inventory_sha256.as_str())
        || receipt
            .get("particle_id_encoding_sha256")
            .and_then(Value::as_str)
            != Some(particle_id_encoding_sha256.as_str())
        || receipt.get("node_inventory_sha256").and_then(Value::as_str)
            != Some(link.node_inventory_sha256.as_str())
        || node_inventory
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(link.node_inventory_sha256.as_str())
        || receipt
            .get("owner_world_transform_sha256")
            .and_then(Value::as_str)
            != Some(link.owner_world_transform_sha256.as_str())
        || owner_transforms
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(link.owner_world_transform_sha256.as_str())
        || receipt.get("particle_count").and_then(Value::as_u64) != Some(56)
        || receipt
            .get("base_aov_byte_exact_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("bloom_pass_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("opaque_geometry_depth_tested")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("typed_particles_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || receipt.get("export_performed").and_then(Value::as_bool) != Some(false)
        || receipt
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
        || receipt.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || render_set
            .get("particle_key_sha256")
            .and_then(Value::as_str)
            != Some(link.particle_key_sha256.as_str())
        || render_set
            .get("base_frame_key_sha256")
            .and_then(Value::as_str)
            != Some(link.base_frame_key_sha256.as_str())
        || render_set.get("bloom_key_sha256").and_then(Value::as_str)
            != Some(link.bloom_key_sha256.as_str())
        || render_set.get("passes")
            != Some(&json!(["particle-color", "particle-id", "particle-depth"]))
        || render_set
            .get("particle_inventory_sha256")
            .and_then(Value::as_str)
            != Some(particle_inventory_sha256.as_str())
        || render_set
            .get("particle_id_encoding_sha256")
            .and_then(Value::as_str)
            != Some(particle_id_encoding_sha256.as_str())
        || render_set.get("base_aov_passes") != Some(&json!(base_link.pass_object_sha256s))
        || render_set.get("bloom_passes")
            != Some(&json!([
                bloom_link.source_object_sha256,
                bloom_link.contribution_object_sha256
            ]))
        || base_link.frame_key_sha256 != link.base_frame_key_sha256
        || bloom_link.bloom_key_sha256 != link.bloom_key_sha256
        || bloom_link.base_frame_key_sha256 != link.base_frame_key_sha256
    {
        return invalid("durable typed particle receipt or dependency binding differs");
    }
    Ok(json!({
        "schema_version":"FictionalEnergyVfxParticlesFrameGetResult@1",
        "particle_key_sha256":particle_key_sha256,
        "receipt_object_sha256":link.receipt_object_sha256,
        "receipt":receipt,
        "render_set":render_set,
        "link":link,
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn energy_vfx_trails_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "vfx_profile_object_sha256",
            "anchor_set_object_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "current_particle_key_sha256",
            "particle_history_key_sha256s",
            "sample_time_ticks",
            "trail_policy",
            "history_policy",
            "render_policy",
            "bloom_input",
            "canonical_sha256",
        ],
        "FictionalEnergyVfxTrailsFrameRenderPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxTrailsFrameRenderPrepareRequest@1"
        || text(object, "trail_policy")? != "two-closed-history-bound-polyline-trails@1"
        || text(object, "history_policy")? != "one-to-four-strictly-earlier-particle-frames@1"
        || text(object, "render_policy")? != TRAILS_RENDER_POLICY
        || object.get("bloom_input").and_then(Value::as_bool) != Some(false)
    {
        return invalid("typed trail policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?.to_owned();
    let profile_sha256 = sha(object, "vfx_profile_object_sha256")?.to_owned();
    let anchor_sha256 = sha(object, "anchor_set_object_sha256")?.to_owned();
    let base_frame_key_sha256 = sha(object, "base_frame_key_sha256")?.to_owned();
    let bloom_key_sha256 = sha(object, "bloom_key_sha256")?.to_owned();
    let current_particle_key_sha256 = sha(object, "current_particle_key_sha256")?.to_owned();
    let history_values = object
        .get("particle_history_key_sha256s")
        .and_then(Value::as_array)
        .filter(|values| (1..=4).contains(&values.len()))
        .ok_or_else(|| error("typed trail requires one to four history particle keys"))?;
    let particle_history_key_sha256s = history_values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| forgecad_contracts::is_sha256(value))
                .map(str::to_owned)
                .ok_or_else(|| error("typed trail history particle key is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if particle_history_key_sha256s
        .iter()
        .collect::<BTreeSet<_>>()
        .len()
        != particle_history_key_sha256s.len()
        || particle_history_key_sha256s.contains(&current_particle_key_sha256)
    {
        return invalid("typed trail particle keys are duplicated");
    }
    let sample_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| error("typed trail sample tick is invalid"))?;
    let trail_key_sha256 = sha(object, "canonical_sha256")?.to_owned();
    if runtime
        .store
        .get_fictional_energy_vfx_trails_frame_link(&trail_key_sha256)?
        .is_some()
    {
        let value = energy_vfx_trails_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxTrailsFrameGetRequest@1",
                "project_id":project_id,
                "trail_key_sha256":trail_key_sha256
            }),
        )?;
        return Ok(json!({
            "schema_version":"FictionalEnergyVfxTrailsFramePrepareResult@1",
            "trail_key_sha256":value["trail_key_sha256"],
            "receipt_object_sha256":value["receipt_object_sha256"],
            "receipt":value["receipt"],
            "durable_link":value["link"],
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }));
    }

    let mut particle_frames = Vec::with_capacity(particle_history_key_sha256s.len() + 1);
    for key in particle_history_key_sha256s
        .iter()
        .chain(std::iter::once(&current_particle_key_sha256))
    {
        particle_frames.push(energy_vfx_particles_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxParticlesFrameGetRequest@1",
                "project_id":project_id,
                "particle_key_sha256":key
            }),
        )?);
    }
    let receipts = particle_frames
        .iter()
        .map(|frame| {
            frame
                .get("receipt")
                .ok_or_else(|| error("typed trail particle receipt is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let current_receipt = *receipts
        .last()
        .ok_or_else(|| error("typed trail current particle receipt is unavailable"))?;
    if current_receipt
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        != Some(sample_time_ticks)
        || current_receipt
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(delivery_sha256.as_str())
        || current_receipt
            .get("vfx_profile_object_sha256")
            .and_then(Value::as_str)
            != Some(profile_sha256.as_str())
        || current_receipt
            .get("anchor_set_object_sha256")
            .and_then(Value::as_str)
            != Some(anchor_sha256.as_str())
        || current_receipt
            .get("base_frame_key_sha256")
            .and_then(Value::as_str)
            != Some(base_frame_key_sha256.as_str())
        || current_receipt
            .get("bloom_key_sha256")
            .and_then(Value::as_str)
            != Some(bloom_key_sha256.as_str())
    {
        return invalid("typed trail current particle/request binding differs");
    }
    let expected_particle_keys = particle_history_key_sha256s
        .iter()
        .chain(std::iter::once(&current_particle_key_sha256));
    for (receipt, expected_key) in receipts.iter().zip(expected_particle_keys) {
        if receipt.get("particle_key_sha256").and_then(Value::as_str) != Some(expected_key.as_str())
            || receipt.get("project_id").and_then(Value::as_str) != Some(project_id.as_str())
            || receipt
                .get("delivery_manifest_object_sha256")
                .and_then(Value::as_str)
                != current_receipt
                    .get("delivery_manifest_object_sha256")
                    .and_then(Value::as_str)
            || receipt
                .get("vfx_profile_object_sha256")
                .and_then(Value::as_str)
                != current_receipt
                    .get("vfx_profile_object_sha256")
                    .and_then(Value::as_str)
            || receipt
                .get("anchor_set_object_sha256")
                .and_then(Value::as_str)
                != current_receipt
                    .get("anchor_set_object_sha256")
                    .and_then(Value::as_str)
            || receipt.get("source_candidate_id").and_then(Value::as_str)
                != current_receipt
                    .get("source_candidate_id")
                    .and_then(Value::as_str)
            || receipt
                .get("source_artifact_sha256")
                .and_then(Value::as_str)
                != current_receipt
                    .get("source_artifact_sha256")
                    .and_then(Value::as_str)
            || receipt.get("camera_object_sha256").and_then(Value::as_str)
                != current_receipt
                    .get("camera_object_sha256")
                    .and_then(Value::as_str)
            || receipt
                .get("camera_identity_sha256")
                .and_then(Value::as_str)
                != current_receipt
                    .get("camera_identity_sha256")
                    .and_then(Value::as_str)
            || receipt.get("render_profile_sha256").and_then(Value::as_str)
                != current_receipt
                    .get("render_profile_sha256")
                    .and_then(Value::as_str)
            || receipt
                .get("render_worker_build_cohort_sha256")
                .and_then(Value::as_str)
                != current_receipt
                    .get("render_worker_build_cohort_sha256")
                    .and_then(Value::as_str)
            || receipt.get("node_inventory_sha256").and_then(Value::as_str)
                != current_receipt
                    .get("node_inventory_sha256")
                    .and_then(Value::as_str)
            || receipt
                .get("owner_world_transform_sha256")
                .and_then(Value::as_str)
                != current_receipt
                    .get("owner_world_transform_sha256")
                    .and_then(Value::as_str)
            || receipt.get("particle_policy").and_then(Value::as_str)
                != Some("two-closed-emitters-hash-seeded-typed-attributes@1")
            || receipt.get("emitter_policy").and_then(Value::as_str)
                != Some("muzzle-burst-24-energy-core-sparks-32@1")
            || receipt
                .get("particle_id_encoding_sha256")
                .and_then(Value::as_str)
                != current_receipt
                    .get("particle_id_encoding_sha256")
                    .and_then(Value::as_str)
        {
            return invalid("typed trail particle history binding differs");
        }
    }
    let history_ticks = receipts[..receipts.len() - 1]
        .iter()
        .map(|receipt| {
            receipt
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                .ok_or_else(|| error("typed trail history tick is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if history_ticks.windows(2).any(|ticks| ticks[0] >= ticks[1])
        || history_ticks
            .last()
            .is_some_and(|tick| *tick >= sample_time_ticks)
    {
        return invalid("typed trail history ticks must be strictly increasing and earlier");
    }
    let base = energy_vfx_rendered_frame_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":project_id,
            "frame_key_sha256":base_frame_key_sha256
        }),
    )?;
    let bloom = energy_vfx_hdr_bloom_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
            "project_id":project_id,
            "bloom_key_sha256":bloom_key_sha256
        }),
    )?;
    let base_link: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(
        base.get("link")
            .cloned()
            .ok_or_else(|| error("typed trail base link is unavailable"))?,
    )
    .map_err(|source| error(format!("typed trail base link is invalid: {source}")))?;
    let bloom_link: FictionalEnergyVfxBloomFrameLinkRecord = serde_json::from_value(
        bloom
            .get("link")
            .cloned()
            .ok_or_else(|| error("typed trail Bloom link is unavailable"))?,
    )
    .map_err(|source| error(format!("typed trail Bloom link is invalid: {source}")))?;
    if base_link.frame_key_sha256 != base_frame_key_sha256
        || base_link.project_id != project_id
        || base_link.delivery_manifest_object_sha256 != delivery_sha256
        || base_link.vfx_profile_object_sha256 != profile_sha256
        || current_receipt
            .get("source_candidate_id")
            .and_then(Value::as_str)
            != Some(base_link.source_candidate_id.as_str())
        || current_receipt
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(base_link.source_artifact_sha256.as_str())
        || !particle_receipt_matches_render_binding(
            current_receipt,
            &base_link.camera_object_sha256,
            &base_link.camera_identity_sha256,
            &base_link.render_profile_sha256,
            &base_link.render_worker_build_cohort_sha256,
        )
        || bloom_link.bloom_key_sha256 != bloom_key_sha256
        || bloom_link.base_frame_key_sha256 != base_frame_key_sha256
        || bloom_link.project_id != base_link.project_id
        || bloom_link.delivery_manifest_object_sha256 != base_link.delivery_manifest_object_sha256
        || bloom_link.vfx_profile_object_sha256 != base_link.vfx_profile_object_sha256
        || bloom_link.source_candidate_id != base_link.source_candidate_id
        || bloom_link.source_artifact_sha256 != base_link.source_artifact_sha256
        || bloom_link.camera_object_sha256 != base_link.camera_object_sha256
        || bloom_link.camera_identity_sha256 != base_link.camera_identity_sha256
        || bloom_link.render_profile_sha256 != base_link.render_profile_sha256
        || bloom_link.render_worker_build_cohort_sha256
            != base_link.render_worker_build_cohort_sha256
    {
        return invalid("typed trail current base/Bloom binding differs");
    }
    let camera = read_json(
        runtime,
        &base_link.camera_object_sha256,
        "CameraCalibration@1",
    )?;
    let source_glb = runtime.cas_read(&base_link.source_artifact_sha256)?;
    let node_inventory = current_receipt
        .get("node_inventory")
        .cloned()
        .ok_or_else(|| error("typed trail node inventory is unavailable"))?;
    let (trail_seed_sha256, trail_values, typed_trails, owner_transforms) = derive_typed_trails(
        request,
        &camera,
        &json!({"anchors":[]}),
        &json!({"effects":[]}),
        &base_link.source_artifact_sha256,
        &node_inventory,
        &BTreeMap::new(),
        &current_particle_key_sha256,
        &particle_history_key_sha256s,
        &receipts
            .iter()
            .map(|value| (*value).clone())
            .collect::<Vec<_>>(),
    )?;
    let trail_inventory: Value = serde_json::from_slice(
        &canonical_json_bytes(&Value::Array(trail_values.clone()))
            .map_err(|source| error(source.to_string()))?,
    )
    .map_err(|_| error("typed trail inventory normalization failed"))?;
    let trail_inventory_sha256 = canonical_json_hash(&trail_inventory);
    let trail_id_encoding = json!({
        "schema_version":"FictionalEnergyVfxTrailIdEncoding@1",
        "encoding":"lossless-little-endian-rgb24-parent-trail-id-plus-one-alpha-visible@1",
        "background_rgba8":[0,0,0,0],
        "id_range":[1,65535],
        "segment_identity":"segments-share-stable-parent-trail-id@1"
    });
    let trail_id_encoding_sha256 = canonical_json_hash(&trail_id_encoding);
    let first = render_worker::render_typed_trails_with_worker_identity(
        &source_glb,
        &camera,
        &typed_trails,
        &trail_seed_sha256,
    )
    .map_err(|source| error(format!("typed trail render failed: {source}")))?;
    let second = render_worker::render_typed_trails_with_worker_identity(
        &source_glb,
        &camera,
        &typed_trails,
        &trail_seed_sha256,
    )
    .map_err(|source| error(format!("typed trail replay failed: {source}")))?;
    if first.build_cohort_sha256.is_none()
        || first.build_cohort_sha256 != second.build_cohort_sha256
        || first.render_profile != second.render_profile
        || first.seed_sha256 != trail_seed_sha256
        || second.seed_sha256 != trail_seed_sha256
        || first.trail_count != 2
        || second.trail_count != 2
        || first.segment_count != particle_history_key_sha256s.len() * 2
        || second.segment_count != first.segment_count
        || first.emitter_counts != [1, 1]
        || second.emitter_counts != [1, 1]
        || first.trail_passes.len() != 3
        || first
            .trail_passes
            .iter()
            .zip(&second.trail_passes)
            .any(|(left, right)| left.pass != right.pass || left.png != right.png)
    {
        return invalid("typed trail replay or Worker cohort differs");
    }
    let worker_cohort = first
        .build_cohort_sha256
        .clone()
        .ok_or_else(|| error("typed trail Worker cohort is unavailable"))?;
    if worker_cohort != base_link.render_worker_build_cohort_sha256
        || first.render_profile["canonical_sha256"].as_str()
            != Some(base_link.render_profile_sha256.as_str())
    {
        return invalid("typed trail Worker/base cohort or RenderProfile differs");
    }
    let history_ticks = history_ticks
        .into_iter()
        .map(Value::from)
        .collect::<Vec<_>>();
    let particle_pass_sha256s = receipts
        .iter()
        .map(|receipt| {
            receipt
                .get("pass_artifacts")
                .and_then(Value::as_array)
                .map(|passes| {
                    passes
                        .iter()
                        .map(|pass| pass["sha256"].clone())
                        .collect::<Vec<_>>()
                })
                .ok_or_else(|| error("typed trail particle pass inventory is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let node_inventory_sha256 = current_receipt["node_inventory_sha256"]
        .as_str()
        .ok_or_else(|| error("typed trail node inventory hash is unavailable"))?
        .to_owned();
    let owner_world_transform_sha256 = current_receipt["owner_world_transform_sha256"]
        .as_str()
        .ok_or_else(|| error("typed trail owner transform hash is unavailable"))?
        .to_owned();
    if owner_transforms
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(owner_world_transform_sha256.as_str())
    {
        return invalid("typed trail owner transform binding differs");
    }

    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::<CasObject>::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let mut pass_artifacts = Map::new();
        let mut receipt_passes = Vec::new();
        let mut pass_hashes = Vec::new();
        for (ordinal, pass) in first.trail_passes.iter().enumerate() {
            let kind = match pass.pass.as_str() {
                "trail-color" => "fictional-energy-vfx-trails-color",
                "trail-id" => "fictional-energy-vfx-trails-id",
                "trail-depth" => "fictional-energy-vfx-trails-depth",
                _ => return invalid("typed trail pass inventory differs"),
            };
            let object = runtime.store.put_object_reserved(
                &reservation,
                &pass.png,
                None,
                "image/png",
                kind,
                &now_string(),
            )?;
            let metadata = json!({
                "sha256":object.record.sha256,
                "mime":"image/png",
                "size_bytes":object.record.size_bytes,
                "width":512,
                "height":512,
                "channels":"rgba8",
                "color_space":"data"
            });
            pass_artifacts.insert(pass.pass.clone(), metadata.clone());
            receipt_passes.push(json!({
                "pass":pass.pass,
                "ordinal":ordinal,
                "sha256":metadata["sha256"],
                "mime":"image/png",
                "size_bytes":metadata["size_bytes"],
                "width":512,
                "height":512,
                "channels":"rgba8",
                "color_space":"data"
            }));
            pass_hashes.push(object.record.sha256.clone());
            reserved_objects.push(object);
        }
        let mut render_set = json!({
            "schema_version":TRAILS_RENDER_SET_SCHEMA,
            "render_set_id":format!("vfx-trails-{}", &trail_key_sha256[..32]),
            "trail_key_sha256":trail_key_sha256,
            "base_frame_key_sha256":base_frame_key_sha256,
            "bloom_key_sha256":bloom_key_sha256,
            "current_particle_key_sha256":current_particle_key_sha256,
            "particle_history_key_sha256s":particle_history_key_sha256s,
            "candidate_id":base_link.source_candidate_id,
            "artifact_sha256":base_link.source_artifact_sha256,
            "camera_hash":base_link.camera_identity_sha256,
            "camera_object_sha256":base_link.camera_object_sha256,
            "render_profile_sha256":base_link.render_profile_sha256,
            "render_worker_build_cohort_sha256":worker_cohort,
            "trail_seed_sha256":trail_seed_sha256,
            "trail_inventory_sha256":trail_inventory_sha256,
            "trail_id_encoding_sha256":trail_id_encoding_sha256,
            "node_inventory_sha256":node_inventory_sha256,
            "owner_world_transform_sha256":owner_world_transform_sha256,
            "passes":["trail-color","trail-id","trail-depth"],
            "pass_artifacts":pass_artifacts,
            "base_aov_passes":base_link.pass_object_sha256s,
            "bloom_passes":[bloom_link.source_object_sha256,bloom_link.contribution_object_sha256],
            "particle_pass_sha256s":particle_pass_sha256s,
            "bloom_input":false,
            "canonical_sha256":""
        });
        render_set["canonical_sha256"] = Value::String(canonical_json_hash(&render_set));
        let render_set_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&render_set).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-trails-render-set",
            &now_string(),
        )?;
        reserved_objects.push(render_set_object.clone());
        let mut receipt_fields = Map::new();
        for (key, value) in [
            ("schema_version", json!(TRAILS_RECEIPT_SCHEMA)),
            ("trail_key_sha256", json!(trail_key_sha256)),
            ("project_id", json!(project_id)),
            ("delivery_manifest_object_sha256", json!(delivery_sha256)),
            ("vfx_profile_object_sha256", json!(profile_sha256)),
            ("anchor_set_object_sha256", json!(anchor_sha256)),
            ("source_candidate_id", json!(base_link.source_candidate_id)),
            (
                "source_artifact_sha256",
                json!(base_link.source_artifact_sha256),
            ),
            ("sample_request_sha256", json!(trail_key_sha256)),
            ("sample_time_ticks", json!(sample_time_ticks)),
            ("history_time_ticks", json!(history_ticks)),
            ("base_frame_key_sha256", json!(base_frame_key_sha256)),
            ("bloom_key_sha256", json!(bloom_key_sha256)),
            (
                "current_particle_key_sha256",
                json!(current_particle_key_sha256),
            ),
            (
                "particle_history_key_sha256s",
                json!(particle_history_key_sha256s),
            ),
            (
                "camera_object_sha256",
                json!(base_link.camera_object_sha256),
            ),
            (
                "camera_identity_sha256",
                json!(base_link.camera_identity_sha256),
            ),
            (
                "render_profile_sha256",
                json!(base_link.render_profile_sha256),
            ),
            ("render_worker_build_cohort_sha256", json!(worker_cohort)),
            ("trail_seed_sha256", json!(trail_seed_sha256)),
            ("trail_inventory_sha256", json!(trail_inventory_sha256)),
            ("trail_id_encoding_sha256", json!(trail_id_encoding_sha256)),
            ("trail_id_encoding", trail_id_encoding.clone()),
            ("node_inventory_sha256", json!(node_inventory_sha256)),
            ("node_inventory", node_inventory.clone()),
            (
                "owner_world_transform_sha256",
                json!(owner_world_transform_sha256),
            ),
            ("owner_world_transforms", owner_transforms.clone()),
            ("trail_count", json!(typed_trails.len())),
            ("segment_count", json!(first.segment_count)),
            (
                "emitter_counts",
                json!({"muzzle-trail":1,"energy-core-trail":1}),
            ),
            ("trails", trail_inventory.clone()),
            ("pass_artifacts", Value::Array(receipt_passes.clone())),
            ("base_aov_passes", json!(base_link.pass_object_sha256s)),
            (
                "bloom_passes",
                json!([
                    bloom_link.source_object_sha256,
                    bloom_link.contribution_object_sha256
                ]),
            ),
            ("particle_pass_sha256s", json!(particle_pass_sha256s)),
            (
                "history_policy",
                json!("one-to-four-strictly-earlier-particle-frames@1"),
            ),
            (
                "trail_policy",
                json!("two-closed-history-bound-polyline-trails@1"),
            ),
            ("render_policy", json!(TRAILS_RENDER_POLICY)),
            (
                "simulation_quantization",
                json!({"position_m":"signed-micrometer-round-nearest@1","time":"integer-tick@1","sort_order":"emitter-definition-then-source-particle-id-then-time@1"}),
            ),
            ("base_aov_byte_exact_verified", json!(true)),
            ("bloom_pass_byte_exact_reused", json!(true)),
            ("particle_passes_byte_exact_reused", json!(true)),
            ("opaque_geometry_depth_tested", json!(true)),
            ("typed_trails_rendered", json!(true)),
            ("bloom_input", json!(false)),
            ("anchor_is_runtime_sidecar_not_glb_socket", json!(true)),
            ("runtime_write_performed", json!(true)),
            ("candidate_confirmed", json!(false)),
            ("export_performed", json!(false)),
            ("actual_engine_roundtrip", json!(false)),
            ("quality_status", json!("structural_only")),
            (
                "limitations",
                json!([
                    "parent-trail-id-shared-by-segments",
                    "no-trail-bloom-input",
                    "anchor-sidecar-not-glb-socket",
                    "no-commercial-engine-roundtrip",
                    "no-visual-quality-or-likeness-pass"
                ]),
            ),
            ("canonical_sha256", json!("")),
        ] {
            receipt_fields.insert(key.to_owned(), value);
        }
        let receipt = seal_sidecar(Value::Object(receipt_fields))?;
        let receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&receipt).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-trails-frame-receipt",
            &now_string(),
        )?;
        reserved_objects.push(receipt_object.clone());
        let mut link = FictionalEnergyVfxTrailsFrameLinkRecord {
            schema_version: "FictionalEnergyVfxTrailsFrameLink@1".to_owned(),
            trail_key_sha256: trail_key_sha256.clone(),
            project_id: project_id.clone(),
            delivery_manifest_object_sha256: delivery_sha256.clone(),
            vfx_profile_object_sha256: profile_sha256.clone(),
            anchor_set_object_sha256: anchor_sha256.clone(),
            source_candidate_id: base_link.source_candidate_id.clone(),
            source_artifact_sha256: base_link.source_artifact_sha256.clone(),
            sample_request_sha256: trail_key_sha256.clone(),
            base_frame_key_sha256: base_frame_key_sha256.clone(),
            bloom_key_sha256: bloom_key_sha256.clone(),
            current_particle_key_sha256: current_particle_key_sha256.clone(),
            particle_history_key_sha256s: particle_history_key_sha256s.clone(),
            camera_object_sha256: base_link.camera_object_sha256.clone(),
            camera_identity_sha256: base_link.camera_identity_sha256.clone(),
            render_profile_sha256: base_link.render_profile_sha256.clone(),
            render_worker_build_cohort_sha256: worker_cohort.clone(),
            trail_seed_sha256: trail_seed_sha256.clone(),
            node_inventory_sha256: node_inventory_sha256.clone(),
            owner_world_transform_sha256: owner_world_transform_sha256.clone(),
            trail_inventory_sha256: trail_inventory_sha256.clone(),
            trail_id_encoding_sha256: trail_id_encoding_sha256.clone(),
            render_set_object_sha256: render_set_object.record.sha256.clone(),
            receipt_object_sha256: receipt_object.record.sha256.clone(),
            trail_color_object_sha256: pass_hashes[0].clone(),
            trail_id_object_sha256: pass_hashes[1].clone(),
            trail_depth_object_sha256: pass_hashes[2].clone(),
            materialization_status: "runtime-owned-durable-fictional-energy-vfx-typed-trails-frame"
                .to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        link.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&link).map_err(|source| error(source.to_string()))?,
        );
        let durable_link = runtime
            .store
            .record_fictional_energy_vfx_trails_frame_link(&link)?;
        Ok(json!({
            "schema_version":"FictionalEnergyVfxTrailsFramePrepareResult@1",
            "trail_key_sha256":trail_key_sha256,
            "receipt_object_sha256":receipt_object.record.sha256,
            "receipt":receipt,
            "durable_link":durable_link,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }))
    })();
    match operation {
        Ok(value) => {
            for object in &reserved_objects {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(operation_error) => {
            let mut rollback_error = None;
            for object in reserved_objects.iter().rev() {
                if let Err(error) =
                    runtime
                        .store
                        .release_cas_reservation_object(&reservation, object, true)
                {
                    rollback_error = Some(error.to_string());
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(error(format!(
                    "typed trail frame failed ({operation_error}); reservation rollback also failed ({rollback_error})"
                )));
            }
            Err(operation_error)
        }
    }
}

pub(super) fn energy_vfx_trails_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &["schema_version", "project_id", "trail_key_sha256"],
        "FictionalEnergyVfxTrailsFrameGetRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxTrailsFrameGetRequest@1" {
        return invalid("typed trail get schema differs");
    }
    let project_id = identifier(object, "project_id")?;
    let trail_key_sha256 = sha(object, "trail_key_sha256")?;
    let link = runtime
        .store
        .get_fictional_energy_vfx_trails_frame_link(trail_key_sha256)?
        .ok_or_else(|| error("durable typed trail frame is unavailable"))?;
    if link.project_id != project_id {
        return invalid("durable typed trail frame belongs to another project");
    }
    let receipt = read_json(runtime, &link.receipt_object_sha256, TRAILS_RECEIPT_SCHEMA)?;
    let render_set = read_json(
        runtime,
        &link.render_set_object_sha256,
        TRAILS_RENDER_SET_SCHEMA,
    )?;
    let expected = [
        ("trail-color", &link.trail_color_object_sha256),
        ("trail-id", &link.trail_id_object_sha256),
        ("trail-depth", &link.trail_depth_object_sha256),
    ];
    let receipt_passes = receipt
        .get("pass_artifacts")
        .and_then(Value::as_array)
        .filter(|passes| passes.len() == 3)
        .ok_or_else(|| error("durable typed trail pass inventory is invalid"))?;
    for (ordinal, ((name, hash), pass)) in expected.iter().zip(receipt_passes).enumerate() {
        if pass.get("pass").and_then(Value::as_str) != Some(*name)
            || pass.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
            || pass.get("sha256").and_then(Value::as_str) != Some(hash.as_str())
            || sha256_hex(&runtime.cas_read(hash)?) != **hash
            || render_set
                .get("pass_artifacts")
                .and_then(Value::as_object)
                .and_then(|passes| passes.get(*name))
                .and_then(|value| value.get("sha256"))
                .and_then(Value::as_str)
                != Some(hash.as_str())
        {
            return invalid("durable typed trail pass binding differs");
        }
    }
    let mut particle_frames = Vec::new();
    for key in link
        .particle_history_key_sha256s
        .iter()
        .chain(std::iter::once(&link.current_particle_key_sha256))
    {
        particle_frames.push(energy_vfx_particles_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxParticlesFrameGetRequest@1",
                "project_id":project_id,
                "particle_key_sha256":key
            }),
        )?);
    }
    let particle_receipts = particle_frames
        .iter()
        .map(|frame| {
            frame
                .get("receipt")
                .ok_or_else(|| error("durable typed trail particle receipt is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let history_tick_numbers = particle_receipts[..particle_receipts.len() - 1]
        .iter()
        .map(|receipt| {
            receipt
                .get("sample_time_ticks")
                .and_then(Value::as_u64)
                .ok_or_else(|| error("durable typed trail history tick is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let current_tick_number = particle_receipts
        .last()
        .and_then(|receipt| receipt.get("sample_time_ticks"))
        .and_then(Value::as_u64)
        .ok_or_else(|| error("durable typed trail current tick is unavailable"))?;
    if history_tick_numbers
        .windows(2)
        .any(|ticks| ticks[0] >= ticks[1])
        || history_tick_numbers
            .last()
            .is_some_and(|tick| *tick >= current_tick_number)
    {
        return invalid("durable typed trail history order differs");
    }
    let history_ticks = history_tick_numbers
        .into_iter()
        .map(Value::from)
        .collect::<Vec<_>>();
    let current_tick = Value::from(current_tick_number);
    let particle_pass_sha256s = particle_frames
        .iter()
        .map(|frame| {
            frame["receipt"]["pass_artifacts"]
                .as_array()
                .map(|passes| {
                    passes
                        .iter()
                        .map(|pass| pass["sha256"].clone())
                        .collect::<Vec<_>>()
                })
                .ok_or_else(|| error("durable typed trail particle pass inventory is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let base = energy_vfx_rendered_frame_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":project_id,
            "frame_key_sha256":link.base_frame_key_sha256
        }),
    )?;
    let bloom = energy_vfx_hdr_bloom_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
            "project_id":project_id,
            "bloom_key_sha256":link.bloom_key_sha256
        }),
    )?;
    let base_link: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(base["link"].clone())
        .map_err(|source| {
            error(format!(
                "durable typed trail base link is invalid: {source}"
            ))
        })?;
    let bloom_link: FictionalEnergyVfxBloomFrameLinkRecord =
        serde_json::from_value(bloom["link"].clone()).map_err(|source| {
            error(format!(
                "durable typed trail Bloom link is invalid: {source}"
            ))
        })?;
    let trail_inventory_sha256 = canonical_json_hash(
        receipt
            .get("trails")
            .ok_or_else(|| error("durable typed trail inventory is unavailable"))?,
    );
    let trail_id_encoding_sha256 = canonical_json_hash(
        receipt
            .get("trail_id_encoding")
            .ok_or_else(|| error("durable typed trail ID encoding is unavailable"))?,
    );
    validate_trails_receipt_semantics(&receipt)?;
    if receipt.get("trail_key_sha256").and_then(Value::as_str)
        != Some(link.trail_key_sha256.as_str())
        || receipt.get("project_id").and_then(Value::as_str) != Some(project_id)
        || receipt
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(link.delivery_manifest_object_sha256.as_str())
        || receipt
            .get("vfx_profile_object_sha256")
            .and_then(Value::as_str)
            != Some(link.vfx_profile_object_sha256.as_str())
        || receipt
            .get("anchor_set_object_sha256")
            .and_then(Value::as_str)
            != Some(link.anchor_set_object_sha256.as_str())
        || receipt.get("source_candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || receipt
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || receipt.get("base_frame_key_sha256").and_then(Value::as_str)
            != Some(link.base_frame_key_sha256.as_str())
        || receipt.get("bloom_key_sha256").and_then(Value::as_str)
            != Some(link.bloom_key_sha256.as_str())
        || receipt.get("sample_time_ticks") != Some(&current_tick)
        || receipt.get("history_time_ticks") != Some(&Value::Array(history_ticks.clone()))
        || receipt.get("camera_object_sha256").and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || receipt
            .get("camera_identity_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || receipt.get("render_profile_sha256").and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || receipt
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || receipt.get("node_inventory_sha256").and_then(Value::as_str)
            != Some(link.node_inventory_sha256.as_str())
        || receipt
            .get("owner_world_transform_sha256")
            .and_then(Value::as_str)
            != Some(link.owner_world_transform_sha256.as_str())
        || receipt.get("trail_seed_sha256").and_then(Value::as_str)
            != Some(link.trail_seed_sha256.as_str())
        || receipt
            .get("trail_inventory_sha256")
            .and_then(Value::as_str)
            != Some(trail_inventory_sha256.as_str())
        || trail_inventory_sha256 != link.trail_inventory_sha256
        || receipt
            .get("trail_id_encoding_sha256")
            .and_then(Value::as_str)
            != Some(trail_id_encoding_sha256.as_str())
        || trail_id_encoding_sha256 != link.trail_id_encoding_sha256
        || receipt
            .get("current_particle_key_sha256")
            .and_then(Value::as_str)
            != Some(link.current_particle_key_sha256.as_str())
        || receipt.get("particle_history_key_sha256s")
            != Some(&json!(link.particle_history_key_sha256s))
        || receipt.get("history_policy").and_then(Value::as_str)
            != Some("one-to-four-strictly-earlier-particle-frames@1")
        || receipt.get("trail_policy").and_then(Value::as_str)
            != Some("two-closed-history-bound-polyline-trails@1")
        || receipt.get("render_policy").and_then(Value::as_str) != Some(TRAILS_RENDER_POLICY)
        || receipt.get("simulation_quantization")
            != Some(
                &json!({"position_m":"signed-micrometer-round-nearest@1","time":"integer-tick@1","sort_order":"emitter-definition-then-source-particle-id-then-time@1"}),
            )
        || receipt.get("trail_count").and_then(Value::as_u64) != Some(2)
        || receipt.get("segment_count").and_then(Value::as_u64)
            != Some((link.particle_history_key_sha256s.len() * 2) as u64)
        || receipt.get("emitter_counts") != Some(&json!({"muzzle-trail":1,"energy-core-trail":1}))
        || receipt.get("base_aov_passes") != Some(&json!(base_link.pass_object_sha256s))
        || receipt.get("bloom_passes")
            != Some(&json!([
                bloom_link.source_object_sha256,
                bloom_link.contribution_object_sha256
            ]))
        || receipt.get("particle_pass_sha256s") != Some(&json!(particle_pass_sha256s))
        || receipt
            .get("base_aov_byte_exact_verified")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("bloom_pass_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("particle_passes_byte_exact_reused")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("typed_trails_rendered")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("bloom_input").and_then(Value::as_bool) != Some(false)
        || receipt
            .get("anchor_is_runtime_sidecar_not_glb_socket")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
        || render_set.get("passes") != Some(&json!(["trail-color", "trail-id", "trail-depth"]))
        || render_set.get("trail_key_sha256").and_then(Value::as_str)
            != Some(link.trail_key_sha256.as_str())
        || render_set
            .get("base_frame_key_sha256")
            .and_then(Value::as_str)
            != Some(link.base_frame_key_sha256.as_str())
        || render_set.get("bloom_key_sha256").and_then(Value::as_str)
            != Some(link.bloom_key_sha256.as_str())
        || render_set
            .get("current_particle_key_sha256")
            .and_then(Value::as_str)
            != Some(link.current_particle_key_sha256.as_str())
        || render_set.get("particle_history_key_sha256s")
            != Some(&json!(link.particle_history_key_sha256s))
        || render_set.get("candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || render_set.get("artifact_sha256").and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || render_set
            .get("camera_object_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || render_set.get("camera_hash").and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || render_set
            .get("render_profile_sha256")
            .and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || render_set
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || render_set
            .get("node_inventory_sha256")
            .and_then(Value::as_str)
            != Some(link.node_inventory_sha256.as_str())
        || render_set
            .get("owner_world_transform_sha256")
            .and_then(Value::as_str)
            != Some(link.owner_world_transform_sha256.as_str())
        || render_set.get("trail_seed_sha256").and_then(Value::as_str)
            != Some(link.trail_seed_sha256.as_str())
        || render_set
            .get("trail_inventory_sha256")
            .and_then(Value::as_str)
            != Some(link.trail_inventory_sha256.as_str())
        || render_set
            .get("trail_id_encoding_sha256")
            .and_then(Value::as_str)
            != Some(link.trail_id_encoding_sha256.as_str())
        || render_set.get("base_aov_passes") != Some(&json!(base_link.pass_object_sha256s))
        || render_set.get("bloom_passes")
            != Some(&json!([
                bloom_link.source_object_sha256,
                bloom_link.contribution_object_sha256
            ]))
        || render_set.get("particle_pass_sha256s") != Some(&json!(particle_pass_sha256s))
        || render_set.get("bloom_input").and_then(Value::as_bool) != Some(false)
    {
        return invalid("durable typed trail receipt or dependency binding differs");
    }
    Ok(json!({
        "schema_version":"FictionalEnergyVfxTrailsFrameGetResult@1",
        "trail_key_sha256":trail_key_sha256,
        "receipt_object_sha256":link.receipt_object_sha256,
        "receipt":receipt,
        "render_set":render_set,
        "link":link,
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

pub(super) fn energy_vfx_trails_bloom_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "delivery_manifest_object_sha256",
            "vfx_profile_object_sha256",
            "anchor_set_object_sha256",
            "base_frame_key_sha256",
            "bloom_key_sha256",
            "source_trail_key_sha256",
            "trail_bloom_profile",
            "trail_bloom_policy",
            "input_policy",
            "occlusion_policy",
            "render_policy",
            "canonical_sha256",
        ],
        "FictionalEnergyVfxTrailsBloomFrameRenderPrepareRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxTrailsBloomFrameRenderPrepareRequest@1"
        || text(object, "trail_bloom_policy")? != TRAILS_BLOOM_POLICY
        || text(object, "input_policy")? != TRAILS_BLOOM_INPUT_POLICY
        || text(object, "occlusion_policy")? != TRAILS_BLOOM_OCCLUSION_POLICY
        || text(object, "render_policy")? != TRAILS_BLOOM_RENDER_POLICY
    {
        return invalid("typed trail Bloom policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let delivery_sha256 = sha(object, "delivery_manifest_object_sha256")?.to_owned();
    let profile_sha256 = sha(object, "vfx_profile_object_sha256")?.to_owned();
    let anchor_sha256 = sha(object, "anchor_set_object_sha256")?.to_owned();
    let base_frame_key_sha256 = sha(object, "base_frame_key_sha256")?.to_owned();
    let bloom_key_sha256 = sha(object, "bloom_key_sha256")?.to_owned();
    let source_trail_key_sha256 = sha(object, "source_trail_key_sha256")?.to_owned();
    let trail_bloom_profile_value = object
        .get("trail_bloom_profile")
        .ok_or_else(|| error("typed trail Bloom profile is unavailable"))?;
    let (trail_bloom_profile, trail_bloom_profile_sha256) =
        parse_fixed_trails_bloom_profile(trail_bloom_profile_value)?;
    let trail_bloom_key_sha256 = sha(object, "canonical_sha256")?.to_owned();
    if runtime
        .store
        .get_fictional_energy_vfx_trails_bloom_frame_link(&trail_bloom_key_sha256)?
        .is_some()
    {
        let value = energy_vfx_trails_bloom_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxTrailsBloomFrameGetRequest@1",
                "project_id":project_id,
                "trail_bloom_key_sha256":trail_bloom_key_sha256
            }),
        )?;
        return Ok(json!({
            "schema_version":"FictionalEnergyVfxTrailsBloomFramePrepareResult@1",
            "trail_bloom_key_sha256":value["trail_bloom_key_sha256"],
            "receipt_object_sha256":value["receipt_object_sha256"],
            "receipt":value["receipt"],
            "durable_link":value["link"],
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }));
    }

    let source_trails = energy_vfx_trails_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxTrailsFrameGetRequest@1",
            "project_id":project_id,
            "trail_key_sha256":source_trail_key_sha256
        }),
    )?;
    let source_trail_link: FictionalEnergyVfxTrailsFrameLinkRecord =
        serde_json::from_value(source_trails["link"].clone()).map_err(|source| {
            error(format!(
                "typed trail Bloom source trail link is invalid: {source}"
            ))
        })?;
    let source_trail_receipt = source_trails
        .get("receipt")
        .ok_or_else(|| error("typed trail Bloom source trail receipt is unavailable"))?;
    if source_trail_link.trail_key_sha256 != source_trail_key_sha256
        || source_trail_link.project_id != project_id
        || source_trail_link.delivery_manifest_object_sha256 != delivery_sha256
        || source_trail_link.vfx_profile_object_sha256 != profile_sha256
        || source_trail_link.anchor_set_object_sha256 != anchor_sha256
        || source_trail_link.base_frame_key_sha256 != base_frame_key_sha256
        || source_trail_link.bloom_key_sha256 != bloom_key_sha256
    {
        return invalid("typed trail Bloom request is not bound to the source trail");
    }

    let base = energy_vfx_rendered_frame_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":project_id,
            "frame_key_sha256":base_frame_key_sha256
        }),
    )?;
    let bloom = energy_vfx_hdr_bloom_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
            "project_id":project_id,
            "bloom_key_sha256":bloom_key_sha256
        }),
    )?;
    let base_link: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(base["link"].clone())
        .map_err(|source| {
            error(format!(
                "typed trail Bloom base frame link is invalid: {source}"
            ))
        })?;
    let bloom_link: FictionalEnergyVfxBloomFrameLinkRecord =
        serde_json::from_value(bloom["link"].clone()).map_err(|source| {
            error(format!(
                "typed trail Bloom HDR bloom link is invalid: {source}"
            ))
        })?;
    let provisional_link = FictionalEnergyVfxTrailsBloomFrameLinkRecord {
        schema_version: "FictionalEnergyVfxTrailsBloomFrameLink@1".to_owned(),
        trail_bloom_key_sha256: trail_bloom_key_sha256.clone(),
        project_id: project_id.clone(),
        delivery_manifest_object_sha256: delivery_sha256.clone(),
        vfx_profile_object_sha256: profile_sha256.clone(),
        anchor_set_object_sha256: anchor_sha256.clone(),
        source_candidate_id: source_trail_link.source_candidate_id.clone(),
        source_artifact_sha256: source_trail_link.source_artifact_sha256.clone(),
        sample_request_sha256: source_trail_link.sample_request_sha256.clone(),
        base_frame_key_sha256: base_frame_key_sha256.clone(),
        bloom_key_sha256: bloom_key_sha256.clone(),
        source_trail_key_sha256: source_trail_key_sha256.clone(),
        camera_object_sha256: source_trail_link.camera_object_sha256.clone(),
        camera_identity_sha256: source_trail_link.camera_identity_sha256.clone(),
        render_profile_sha256: source_trail_link.render_profile_sha256.clone(),
        render_worker_build_cohort_sha256: source_trail_link
            .render_worker_build_cohort_sha256
            .clone(),
        trail_bloom_profile_sha256: trail_bloom_profile_sha256.clone(),
        base_opaque_depth_object_sha256: base_link
            .pass_object_sha256s
            .get(2)
            .cloned()
            .ok_or_else(|| error("typed trail Bloom base depth pass is unavailable"))?,
        trail_seed_sha256: source_trail_link.trail_seed_sha256.clone(),
        node_inventory_sha256: source_trail_link.node_inventory_sha256.clone(),
        owner_world_transform_sha256: source_trail_link.owner_world_transform_sha256.clone(),
        trail_inventory_sha256: source_trail_link.trail_inventory_sha256.clone(),
        trail_id_encoding_sha256: source_trail_link.trail_id_encoding_sha256.clone(),
        source_trail_color_object_sha256: source_trail_link.trail_color_object_sha256.clone(),
        source_trail_id_object_sha256: source_trail_link.trail_id_object_sha256.clone(),
        source_trail_depth_object_sha256: source_trail_link.trail_depth_object_sha256.clone(),
        render_set_object_sha256: "0".repeat(64),
        receipt_object_sha256: "0".repeat(64),
        source_object_sha256: "0".repeat(64),
        contribution_object_sha256: "0".repeat(64),
        materialization_status:
            "runtime-owned-durable-fictional-energy-vfx-typed-trails-bloom-frame".to_owned(),
        canonical_sha256: "0".repeat(64),
        created_at: now_string(),
    };
    validate_trails_bloom_parent_binding(
        &provisional_link,
        &source_trail_link,
        &base_link,
        &bloom_link,
    )?;

    let particle_keys = source_trail_link
        .particle_history_key_sha256s
        .iter()
        .chain(std::iter::once(
            &source_trail_link.current_particle_key_sha256,
        ))
        .cloned()
        .collect::<Vec<_>>();
    let mut particle_frames = Vec::with_capacity(particle_keys.len());
    for key in &particle_keys {
        particle_frames.push(energy_vfx_particles_get(
            runtime,
            &json!({
                "schema_version":"FictionalEnergyVfxParticlesFrameGetRequest@1",
                "project_id":project_id,
                "particle_key_sha256":key
            }),
        )?);
    }
    let particle_receipts = particle_frames
        .iter()
        .map(|frame| {
            frame
                .get("receipt")
                .cloned()
                .ok_or_else(|| error("typed trail Bloom particle receipt is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let current_particle_receipt = particle_receipts
        .last()
        .ok_or_else(|| error("typed trail Bloom current particle receipt is unavailable"))?;
    let sample_time_ticks = current_particle_receipt
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("typed trail Bloom current sample tick is unavailable"))?;
    if source_trail_receipt
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        != Some(sample_time_ticks)
    {
        return invalid("typed trail Bloom sample tick differs from source trail");
    }
    let node_inventory = source_trail_receipt
        .get("node_inventory")
        .cloned()
        .ok_or_else(|| error("typed trail Bloom node inventory is unavailable"))?;
    let derive_request = json!({
        "project_id":project_id,
        "delivery_manifest_object_sha256":delivery_sha256,
        "vfx_profile_object_sha256":profile_sha256,
        "anchor_set_object_sha256":anchor_sha256,
        "base_frame_key_sha256":base_frame_key_sha256,
        "bloom_key_sha256":bloom_key_sha256,
        "sample_time_ticks":sample_time_ticks
    });
    let camera = read_json(
        runtime,
        &source_trail_link.camera_object_sha256,
        "CameraCalibration@1",
    )?;
    if camera_identity_hash(&camera)? != source_trail_link.camera_identity_sha256 {
        return invalid("typed trail Bloom camera identity differs");
    }
    let (trail_seed_sha256, trail_values, typed_trails, owner_transforms) = derive_typed_trails(
        &derive_request,
        &camera,
        &json!({"anchors":[]}),
        &json!({"effects":[]}),
        &source_trail_link.source_artifact_sha256,
        &node_inventory,
        &BTreeMap::new(),
        &source_trail_link.current_particle_key_sha256,
        &source_trail_link.particle_history_key_sha256s,
        &particle_receipts,
    )?;
    let trail_inventory: Value = serde_json::from_slice(
        &canonical_json_bytes(&Value::Array(trail_values))
            .map_err(|source| error(source.to_string()))?,
    )
    .map_err(|_| error("typed trail Bloom inventory normalization failed"))?;
    let trail_inventory_sha256 = canonical_json_hash(&trail_inventory);
    let trail_id_encoding = fixed_trail_id_encoding();
    let trail_id_encoding_sha256 = canonical_json_hash(&trail_id_encoding);
    let source_owner_transforms = source_trail_receipt
        .get("owner_world_transforms")
        .ok_or_else(|| error("typed trail Bloom source owner transforms are unavailable"))?;
    if trail_seed_sha256 != source_trail_link.trail_seed_sha256
        || trail_inventory
            != *source_trail_receipt
                .get("trails")
                .ok_or_else(|| error("typed trail Bloom source trail inventory is unavailable"))?
        || trail_inventory_sha256 != source_trail_link.trail_inventory_sha256
        || trail_id_encoding_sha256 != source_trail_link.trail_id_encoding_sha256
        || owner_transforms != *source_owner_transforms
        || owner_transforms
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(source_trail_link.owner_world_transform_sha256.as_str())
        || node_inventory
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(source_trail_link.node_inventory_sha256.as_str())
    {
        return invalid("typed trail Bloom derived inventory differs from source trail");
    }

    let source_trail_pass_bytes = [
        verify_trails_bloom_cas_hash(
            runtime,
            &source_trail_link.trail_color_object_sha256,
            "source trail color",
        )?,
        verify_trails_bloom_cas_hash(
            runtime,
            &source_trail_link.trail_id_object_sha256,
            "source trail ID",
        )?,
        verify_trails_bloom_cas_hash(
            runtime,
            &source_trail_link.trail_depth_object_sha256,
            "source trail depth",
        )?,
    ];
    let source_glb = runtime.cas_read(&source_trail_link.source_artifact_sha256)?;
    let first = render_worker::render_typed_trails_bloom_with_worker_identity(
        &source_glb,
        &camera,
        &typed_trails,
        &trail_seed_sha256,
        trail_bloom_profile,
    )
    .map_err(|source| error(format!("typed trail Bloom render failed: {source}")))?;
    let second = render_worker::render_typed_trails_bloom_with_worker_identity(
        &source_glb,
        &camera,
        &typed_trails,
        &trail_seed_sha256,
        trail_bloom_profile,
    )
    .map_err(|source| error(format!("typed trail Bloom replay failed: {source}")))?;
    let expected_passes = [
        "trail-color",
        "trail-id",
        "trail-depth",
        "trail-emissive-source",
        "trail-bloom-contribution",
    ];
    if first.build_cohort_sha256.is_none()
        || first.build_cohort_sha256 != second.build_cohort_sha256
        || first.build_cohort_sha256.as_deref()
            != Some(source_trail_link.render_worker_build_cohort_sha256.as_str())
        || first.render_profile != second.render_profile
        || first.render_profile["canonical_sha256"].as_str()
            != Some(source_trail_link.render_profile_sha256.as_str())
        || first.trail_bloom_profile != trail_bloom_profile
        || second.trail_bloom_profile != trail_bloom_profile
        || first.seed_sha256 != trail_seed_sha256
        || second.seed_sha256 != trail_seed_sha256
        || first.trail_count != 2
        || second.trail_count != 2
        || first.segment_count != source_trail_link.particle_history_key_sha256s.len() * 2
        || second.segment_count != first.segment_count
        || first.emitter_counts != [1, 1]
        || second.emitter_counts != [1, 1]
        || first.trail_bloom_passes.len() != expected_passes.len()
        || second.trail_bloom_passes.len() != expected_passes.len()
        || first
            .trail_bloom_passes
            .iter()
            .zip(&second.trail_bloom_passes)
            .enumerate()
            .any(|(ordinal, (left, right))| {
                left.pass != expected_passes[ordinal]
                    || right.pass != expected_passes[ordinal]
                    || left.width != 512
                    || left.height != 512
                    || right.width != 512
                    || right.height != 512
                    || left.png != right.png
            })
        || first
            .trail_bloom_passes
            .iter()
            .take(3)
            .zip(source_trail_pass_bytes.iter())
            .any(|(pass, expected)| pass.png != *expected)
    {
        return invalid("typed trail Bloom Worker replay or source trail bytes differ");
    }
    let worker_cohort = first
        .build_cohort_sha256
        .clone()
        .ok_or_else(|| error("typed trail Bloom Worker cohort is unavailable"))?;
    let particle_pass_sha256s = particle_receipts
        .iter()
        .map(|receipt| {
            receipt
                .get("pass_artifacts")
                .and_then(Value::as_array)
                .filter(|passes| passes.len() == 3)
                .map(|passes| {
                    passes
                        .iter()
                        .map(|pass| pass["sha256"].clone())
                        .collect::<Vec<_>>()
                })
                .ok_or_else(|| error("typed trail Bloom particle pass inventory is unavailable"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let base_aov_passes = base_link.pass_object_sha256s.clone();
    if base_aov_passes.len() != 9 {
        return invalid("typed trail Bloom base AOV inventory is not fixed");
    }
    let base_receipt_passes = base["receipt"]
        .get("pass_artifacts")
        .and_then(Value::as_array)
        .filter(|passes| passes.len() == 9)
        .ok_or_else(|| error("typed trail Bloom base AOV receipt is unavailable"))?;
    let base_depth_hash = base_aov_passes[2].clone();
    if base_receipt_passes[2].get("pass").and_then(Value::as_str) != Some("depth")
        || base_receipt_passes[2].get("sha256").and_then(Value::as_str)
            != Some(base_depth_hash.as_str())
    {
        return invalid("typed trail Bloom base opaque depth pass differs");
    }
    for hash in &base_aov_passes {
        verify_trails_bloom_cas_hash(runtime, hash, "base AOV")?;
    }
    let base_bloom_passes = vec![
        bloom_link.source_object_sha256.clone(),
        bloom_link.contribution_object_sha256.clone(),
    ];
    for hash in &base_bloom_passes {
        verify_trails_bloom_cas_hash(runtime, hash, "base Bloom pass")?;
    }
    for passes in &particle_pass_sha256s {
        for hash in passes {
            let hash = hash
                .as_str()
                .filter(|value| forgecad_contracts::is_sha256(value))
                .ok_or_else(|| error("typed trail Bloom particle pass hash is invalid"))?;
            verify_trails_bloom_cas_hash(runtime, hash, "particle pass")?;
        }
    }

    let reservation = runtime.store.begin_cas_reservation();
    let mut reserved_objects = Vec::<CasObject>::new();
    let operation = (|| -> Result<Value, RuntimeError> {
        let source_object = runtime.store.put_object_reserved(
            &reservation,
            &first.trail_bloom_passes[3].png,
            None,
            "image/png",
            "fictional-energy-vfx-trails-emissive-source",
            &now_string(),
        )?;
        reserved_objects.push(source_object.clone());
        let contribution_object = runtime.store.put_object_reserved(
            &reservation,
            &first.trail_bloom_passes[4].png,
            None,
            "image/png",
            "fictional-energy-vfx-trails-bloom-contribution",
            &now_string(),
        )?;
        reserved_objects.push(contribution_object.clone());
        let source_pass = bloom_pass_metadata(&source_object, "trail-emissive-source");
        let contribution_pass =
            bloom_pass_metadata(&contribution_object, "trail-bloom-contribution");
        let mut render_set_fields = Map::new();
        for (key, value) in [
            ("schema_version", json!(TRAILS_BLOOM_RENDER_SET_SCHEMA)),
            (
                "render_set_id",
                json!(format!(
                    "vfx-trails-bloom-{}",
                    &trail_bloom_key_sha256[..32]
                )),
            ),
            ("trail_bloom_key_sha256", json!(trail_bloom_key_sha256)),
            ("base_frame_key_sha256", json!(base_frame_key_sha256)),
            ("bloom_key_sha256", json!(bloom_key_sha256)),
            ("source_trail_key_sha256", json!(source_trail_key_sha256)),
            ("candidate_id", json!(source_trail_link.source_candidate_id)),
            (
                "artifact_sha256",
                json!(source_trail_link.source_artifact_sha256),
            ),
            (
                "camera_hash",
                json!(source_trail_link.camera_identity_sha256),
            ),
            (
                "camera_object_sha256",
                json!(source_trail_link.camera_object_sha256),
            ),
            (
                "render_profile_sha256",
                json!(source_trail_link.render_profile_sha256),
            ),
            ("render_worker_build_cohort_sha256", json!(worker_cohort)),
            (
                "render_worker_binding_status",
                json!("same_cohort_verified"),
            ),
            ("trail_bloom_profile", fixed_trails_bloom_profile_value()),
            (
                "trail_bloom_profile_sha256",
                json!(trail_bloom_profile_sha256),
            ),
            ("base_opaque_depth_object_sha256", json!(base_depth_hash)),
            ("base_opaque_depth_pass", json!("depth")),
            ("trail_seed_sha256", json!(trail_seed_sha256)),
            ("trail_inventory_sha256", json!(trail_inventory_sha256)),
            ("trail_id_encoding_sha256", json!(trail_id_encoding_sha256)),
            (
                "node_inventory_sha256",
                json!(source_trail_link.node_inventory_sha256),
            ),
            (
                "owner_world_transform_sha256",
                json!(source_trail_link.owner_world_transform_sha256),
            ),
            (
                "source_trail_color_object_sha256",
                json!(source_trail_link.trail_color_object_sha256),
            ),
            (
                "source_trail_id_object_sha256",
                json!(source_trail_link.trail_id_object_sha256),
            ),
            (
                "source_trail_depth_object_sha256",
                json!(source_trail_link.trail_depth_object_sha256),
            ),
            (
                "passes",
                json!(["trail-emissive-source", "trail-bloom-contribution"]),
            ),
            (
                "pass_artifacts",
                json!({
                    "trail-emissive-source":source_pass,
                    "trail-bloom-contribution":contribution_pass
                }),
            ),
            ("base_aov_passes", json!(base_aov_passes)),
            ("bloom_passes", json!(base_bloom_passes)),
            ("particle_pass_sha256s", json!(particle_pass_sha256s)),
            (
                "source_trail_passes",
                json!([
                    source_trail_link.trail_color_object_sha256,
                    source_trail_link.trail_id_object_sha256,
                    source_trail_link.trail_depth_object_sha256
                ]),
            ),
            ("trail_bloom_policy", json!(TRAILS_BLOOM_POLICY)),
            ("input_policy", json!(TRAILS_BLOOM_INPUT_POLICY)),
            ("occlusion_policy", json!(TRAILS_BLOOM_OCCLUSION_POLICY)),
            ("render_policy", json!(TRAILS_BLOOM_RENDER_POLICY)),
            ("base_aov_byte_exact_verified", json!(true)),
            ("base_opaque_depth_byte_exact_reused", json!(true)),
            ("bloom_pass_byte_exact_reused", json!(true)),
            ("particle_passes_byte_exact_reused", json!(true)),
            ("source_trail_passes_byte_exact_reused", json!(true)),
            ("base_bloom_mutated", json!(false)),
            ("particle_passes_mutated", json!(false)),
            ("trail_passes_mutated", json!(false)),
            ("trail_bloom_source_rendered", json!(true)),
            ("trail_bloom_contribution_rendered", json!(true)),
            ("trail_bloom_rendered", json!(true)),
            ("trail_bloom_input", json!(true)),
            ("canonical_sha256", json!("")),
        ] {
            render_set_fields.insert(key.to_owned(), value);
        }
        let render_set = seal_sidecar(Value::Object(render_set_fields))?;
        let render_set_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&render_set).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-trails-bloom-render-set",
            &now_string(),
        )?;
        reserved_objects.push(render_set_object.clone());
        let mut receipt_fields = Map::new();
        for (key, value) in [
            ("schema_version", json!(TRAILS_BLOOM_RECEIPT_SCHEMA)),
            ("trail_bloom_key_sha256", json!(trail_bloom_key_sha256)),
            ("project_id", json!(project_id)),
            ("delivery_manifest_object_sha256", json!(delivery_sha256)),
            ("vfx_profile_object_sha256", json!(profile_sha256)),
            ("anchor_set_object_sha256", json!(anchor_sha256)),
            (
                "source_candidate_id",
                json!(source_trail_link.source_candidate_id),
            ),
            (
                "source_artifact_sha256",
                json!(source_trail_link.source_artifact_sha256),
            ),
            (
                "sample_request_sha256",
                json!(source_trail_link.sample_request_sha256),
            ),
            ("base_frame_key_sha256", json!(base_frame_key_sha256)),
            ("bloom_key_sha256", json!(bloom_key_sha256)),
            ("source_trail_key_sha256", json!(source_trail_key_sha256)),
            (
                "camera_object_sha256",
                json!(source_trail_link.camera_object_sha256),
            ),
            (
                "camera_identity_sha256",
                json!(source_trail_link.camera_identity_sha256),
            ),
            (
                "render_profile_sha256",
                json!(source_trail_link.render_profile_sha256),
            ),
            ("render_worker_build_cohort_sha256", json!(worker_cohort)),
            (
                "render_worker_binding_status",
                json!("same_cohort_verified"),
            ),
            ("trail_bloom_profile", fixed_trails_bloom_profile_value()),
            (
                "trail_bloom_profile_sha256",
                json!(trail_bloom_profile_sha256),
            ),
            ("base_opaque_depth_object_sha256", json!(base_depth_hash)),
            ("base_opaque_depth_pass", json!("depth")),
            ("trail_seed_sha256", json!(trail_seed_sha256)),
            (
                "node_inventory_sha256",
                json!(source_trail_link.node_inventory_sha256),
            ),
            ("node_inventory", node_inventory),
            (
                "owner_world_transform_sha256",
                json!(source_trail_link.owner_world_transform_sha256),
            ),
            ("owner_world_transforms", owner_transforms),
            ("trail_inventory_sha256", json!(trail_inventory_sha256)),
            ("trail_inventory", trail_inventory),
            ("trail_id_encoding_sha256", json!(trail_id_encoding_sha256)),
            ("trail_id_encoding", trail_id_encoding),
            (
                "source_trail_color_object_sha256",
                json!(source_trail_link.trail_color_object_sha256),
            ),
            (
                "source_trail_id_object_sha256",
                json!(source_trail_link.trail_id_object_sha256),
            ),
            (
                "source_trail_depth_object_sha256",
                json!(source_trail_link.trail_depth_object_sha256),
            ),
            ("base_aov_passes", json!(base_aov_passes)),
            ("bloom_passes", json!(base_bloom_passes)),
            ("particle_pass_sha256s", json!(particle_pass_sha256s)),
            (
                "source_trail_passes",
                json!([
                    source_trail_link.trail_color_object_sha256,
                    source_trail_link.trail_id_object_sha256,
                    source_trail_link.trail_depth_object_sha256
                ]),
            ),
            ("source_object_sha256", json!(source_object.record.sha256)),
            (
                "contribution_object_sha256",
                json!(contribution_object.record.sha256),
            ),
            ("source_pass", source_pass),
            ("contribution_pass", contribution_pass),
            ("trail_bloom_policy", json!(TRAILS_BLOOM_POLICY)),
            ("input_policy", json!(TRAILS_BLOOM_INPUT_POLICY)),
            ("occlusion_policy", json!(TRAILS_BLOOM_OCCLUSION_POLICY)),
            ("render_policy", json!(TRAILS_BLOOM_RENDER_POLICY)),
            ("base_aov_byte_exact_verified", json!(true)),
            ("base_opaque_depth_byte_exact_reused", json!(true)),
            ("bloom_pass_byte_exact_reused", json!(true)),
            ("particle_passes_byte_exact_reused", json!(true)),
            ("source_trail_passes_byte_exact_reused", json!(true)),
            ("base_bloom_mutated", json!(false)),
            ("particle_passes_mutated", json!(false)),
            ("trail_passes_mutated", json!(false)),
            ("opaque_geometry_depth_tested", json!(true)),
            ("trail_bloom_source_rendered", json!(true)),
            ("trail_bloom_contribution_rendered", json!(true)),
            ("trail_bloom_rendered", json!(true)),
            ("trail_bloom_input", json!(true)),
            ("runtime_write_performed", json!(true)),
            ("candidate_confirmed", json!(false)),
            ("export_performed", json!(false)),
            ("actual_engine_roundtrip", json!(false)),
            ("quality_status", json!("structural_only")),
            ("limitations", fixed_trails_bloom_limitations()),
            ("canonical_sha256", json!("")),
        ] {
            receipt_fields.insert(key.to_owned(), value);
        }
        let receipt = seal_sidecar(Value::Object(receipt_fields))?;
        let receipt_object = runtime.store.put_object_reserved(
            &reservation,
            &canonical_json_bytes(&receipt).map_err(|source| error(source.to_string()))?,
            None,
            "application/json",
            "fictional-energy-vfx-trails-bloom-frame-receipt",
            &now_string(),
        )?;
        reserved_objects.push(receipt_object.clone());
        let mut link = FictionalEnergyVfxTrailsBloomFrameLinkRecord {
            schema_version: "FictionalEnergyVfxTrailsBloomFrameLink@1".to_owned(),
            trail_bloom_key_sha256: trail_bloom_key_sha256.clone(),
            project_id: project_id.clone(),
            delivery_manifest_object_sha256: delivery_sha256.clone(),
            vfx_profile_object_sha256: profile_sha256.clone(),
            anchor_set_object_sha256: anchor_sha256.clone(),
            source_candidate_id: source_trail_link.source_candidate_id.clone(),
            source_artifact_sha256: source_trail_link.source_artifact_sha256.clone(),
            sample_request_sha256: source_trail_link.sample_request_sha256.clone(),
            base_frame_key_sha256: base_frame_key_sha256.clone(),
            bloom_key_sha256: bloom_key_sha256.clone(),
            source_trail_key_sha256: source_trail_key_sha256.clone(),
            camera_object_sha256: source_trail_link.camera_object_sha256.clone(),
            camera_identity_sha256: source_trail_link.camera_identity_sha256.clone(),
            render_profile_sha256: source_trail_link.render_profile_sha256.clone(),
            render_worker_build_cohort_sha256: worker_cohort.clone(),
            trail_bloom_profile_sha256: trail_bloom_profile_sha256.clone(),
            base_opaque_depth_object_sha256: base_depth_hash,
            trail_seed_sha256: trail_seed_sha256.clone(),
            node_inventory_sha256: source_trail_link.node_inventory_sha256.clone(),
            owner_world_transform_sha256: source_trail_link.owner_world_transform_sha256.clone(),
            trail_inventory_sha256: trail_inventory_sha256.clone(),
            trail_id_encoding_sha256: trail_id_encoding_sha256.clone(),
            source_trail_color_object_sha256: source_trail_link.trail_color_object_sha256.clone(),
            source_trail_id_object_sha256: source_trail_link.trail_id_object_sha256.clone(),
            source_trail_depth_object_sha256: source_trail_link.trail_depth_object_sha256.clone(),
            render_set_object_sha256: render_set_object.record.sha256.clone(),
            receipt_object_sha256: receipt_object.record.sha256.clone(),
            source_object_sha256: source_object.record.sha256.clone(),
            contribution_object_sha256: contribution_object.record.sha256.clone(),
            materialization_status:
                "runtime-owned-durable-fictional-energy-vfx-typed-trails-bloom-frame".to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        link.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&link).map_err(|source| error(source.to_string()))?,
        );
        let durable_link = runtime
            .store
            .record_fictional_energy_vfx_trails_bloom_frame_link(&link)?;
        Ok(json!({
            "schema_version":"FictionalEnergyVfxTrailsBloomFramePrepareResult@1",
            "trail_bloom_key_sha256":trail_bloom_key_sha256,
            "receipt_object_sha256":receipt_object.record.sha256,
            "receipt":receipt,
            "durable_link":durable_link,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only"
        }))
    })();
    match operation {
        Ok(value) => {
            for object in &reserved_objects {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(operation_error) => {
            let mut rollback_error = None;
            for object in reserved_objects.iter().rev() {
                if let Err(error) =
                    runtime
                        .store
                        .release_cas_reservation_object(&reservation, object, true)
                {
                    rollback_error = Some(error.to_string());
                }
            }
            if let Some(rollback_error) = rollback_error {
                return Err(error(format!(
                    "typed trail Bloom frame failed ({operation_error}); reservation rollback also failed ({rollback_error})"
                )));
            }
            Err(operation_error)
        }
    }
}

pub(super) fn energy_vfx_trails_bloom_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &["schema_version", "project_id", "trail_bloom_key_sha256"],
        "FictionalEnergyVfxTrailsBloomFrameGetRequest@1",
    )?;
    if text(object, "schema_version")? != "FictionalEnergyVfxTrailsBloomFrameGetRequest@1" {
        return invalid("typed trail Bloom get schema differs");
    }
    let project_id = identifier(object, "project_id")?;
    let trail_bloom_key_sha256 = sha(object, "trail_bloom_key_sha256")?;
    let link = runtime
        .store
        .get_fictional_energy_vfx_trails_bloom_frame_link(trail_bloom_key_sha256)?
        .ok_or_else(|| error("durable typed trail Bloom frame is unavailable"))?;
    if link.project_id != project_id || link.trail_bloom_key_sha256 != trail_bloom_key_sha256 {
        return invalid("durable typed trail Bloom frame project/key differs");
    }
    let receipt = read_json(
        runtime,
        &link.receipt_object_sha256,
        TRAILS_BLOOM_RECEIPT_SCHEMA,
    )?;
    let render_set = read_json(
        runtime,
        &link.render_set_object_sha256,
        TRAILS_BLOOM_RENDER_SET_SCHEMA,
    )?;
    validate_trails_bloom_semantics(&receipt, &render_set)?;
    let source_trails = energy_vfx_trails_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxTrailsFrameGetRequest@1",
            "project_id":project_id,
            "trail_key_sha256":link.source_trail_key_sha256
        }),
    )?;
    let base = energy_vfx_rendered_frame_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxRenderedFrameGetRequest@1",
            "project_id":project_id,
            "frame_key_sha256":link.base_frame_key_sha256
        }),
    )?;
    let bloom = energy_vfx_hdr_bloom_get(
        runtime,
        &json!({
            "schema_version":"FictionalEnergyVfxHdrBloomFrameGetRequest@1",
            "project_id":project_id,
            "bloom_key_sha256":link.bloom_key_sha256
        }),
    )?;
    let source_trail_link: FictionalEnergyVfxTrailsFrameLinkRecord =
        serde_json::from_value(source_trails["link"].clone()).map_err(|source| {
            error(format!(
                "durable typed trail Bloom source trail link is invalid: {source}"
            ))
        })?;
    let base_link: FictionalEnergyVfxFrameLinkRecord = serde_json::from_value(base["link"].clone())
        .map_err(|source| {
            error(format!(
                "durable typed trail Bloom base frame link is invalid: {source}"
            ))
        })?;
    let bloom_link: FictionalEnergyVfxBloomFrameLinkRecord =
        serde_json::from_value(bloom["link"].clone()).map_err(|source| {
            error(format!(
                "durable typed trail Bloom HDR bloom link is invalid: {source}"
            ))
        })?;
    validate_trails_bloom_parent_binding(&link, &source_trail_link, &base_link, &bloom_link)?;

    let source_trail_receipt = source_trails
        .get("receipt")
        .ok_or_else(|| error("durable typed trail Bloom source receipt is unavailable"))?;
    let particle_pass_sha256s = source_trail_receipt
        .get("particle_pass_sha256s")
        .and_then(Value::as_array)
        .filter(|passes| (2..=5).contains(&passes.len()))
        .cloned()
        .ok_or_else(|| error("durable typed trail Bloom particle inventory is unavailable"))?;
    validate_trails_bloom_parent_passes(
        runtime,
        &receipt,
        &render_set,
        &link,
        &source_trail_link,
        &base_link,
        &bloom_link,
        &particle_pass_sha256s,
    )?;
    let trail_inventory = receipt
        .get("trail_inventory")
        .ok_or_else(|| error("durable typed trail Bloom inventory is unavailable"))?;
    let node_inventory = receipt
        .get("node_inventory")
        .ok_or_else(|| error("durable typed trail Bloom node inventory is unavailable"))?;
    let owner_transforms = receipt
        .get("owner_world_transforms")
        .ok_or_else(|| error("durable typed trail Bloom owner transforms are unavailable"))?;
    let trail_id_encoding = receipt
        .get("trail_id_encoding")
        .ok_or_else(|| error("durable typed trail Bloom ID encoding is unavailable"))?;
    let (_profile, profile_sha256) = parse_fixed_trails_bloom_profile(
        receipt
            .get("trail_bloom_profile")
            .ok_or_else(|| error("durable typed trail Bloom profile is unavailable"))?,
    )?;
    let source_trail_passes = json!([
        source_trail_link.trail_color_object_sha256,
        source_trail_link.trail_id_object_sha256,
        source_trail_link.trail_depth_object_sha256
    ]);
    let own_source_hash = link.source_object_sha256.clone();
    let own_contribution_hash = link.contribution_object_sha256.clone();
    let source_pass = receipt
        .get("source_pass")
        .ok_or_else(|| error("durable typed trail Bloom source pass is unavailable"))?;
    let contribution_pass = receipt
        .get("contribution_pass")
        .ok_or_else(|| error("durable typed trail Bloom contribution pass is unavailable"))?;
    validate_trails_bloom_pass_metadata(
        runtime,
        source_pass,
        "trail-emissive-source",
        &own_source_hash,
    )?;
    validate_trails_bloom_pass_metadata(
        runtime,
        contribution_pass,
        "trail-bloom-contribution",
        &own_contribution_hash,
    )?;
    if render_set
        .get("pass_artifacts")
        .and_then(Value::as_object)
        .and_then(|passes| passes.get("trail-emissive-source"))
        != Some(source_pass)
        || render_set
            .get("pass_artifacts")
            .and_then(Value::as_object)
            .and_then(|passes| passes.get("trail-bloom-contribution"))
            != Some(contribution_pass)
    {
        return invalid("durable typed trail Bloom own pass metadata differs");
    }
    if canonical_json_hash(trail_inventory) != link.trail_inventory_sha256
        || node_inventory
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(link.node_inventory_sha256.as_str())
        || owner_transforms
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(link.owner_world_transform_sha256.as_str())
        || canonical_json_hash(trail_id_encoding) != link.trail_id_encoding_sha256
        || receipt
            .get("trail_bloom_key_sha256")
            .and_then(Value::as_str)
            != Some(trail_bloom_key_sha256)
        || receipt.get("project_id").and_then(Value::as_str) != Some(project_id)
        || receipt
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(link.delivery_manifest_object_sha256.as_str())
        || receipt
            .get("vfx_profile_object_sha256")
            .and_then(Value::as_str)
            != Some(link.vfx_profile_object_sha256.as_str())
        || receipt
            .get("anchor_set_object_sha256")
            .and_then(Value::as_str)
            != Some(link.anchor_set_object_sha256.as_str())
        || receipt.get("source_candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || receipt
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || receipt.get("sample_request_sha256").and_then(Value::as_str)
            != Some(link.sample_request_sha256.as_str())
        || receipt.get("base_frame_key_sha256").and_then(Value::as_str)
            != Some(link.base_frame_key_sha256.as_str())
        || receipt.get("bloom_key_sha256").and_then(Value::as_str)
            != Some(link.bloom_key_sha256.as_str())
        || receipt
            .get("source_trail_key_sha256")
            .and_then(Value::as_str)
            != Some(link.source_trail_key_sha256.as_str())
        || receipt.get("camera_object_sha256").and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || receipt
            .get("camera_identity_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || receipt.get("render_profile_sha256").and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || receipt
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || receipt
            .get("trail_bloom_profile_sha256")
            .and_then(Value::as_str)
            != Some(profile_sha256.as_str())
        || receipt
            .get("base_opaque_depth_object_sha256")
            .and_then(Value::as_str)
            != Some(link.base_opaque_depth_object_sha256.as_str())
        || receipt.get("trail_seed_sha256").and_then(Value::as_str)
            != Some(link.trail_seed_sha256.as_str())
        || receipt.get("node_inventory_sha256").and_then(Value::as_str)
            != Some(link.node_inventory_sha256.as_str())
        || receipt
            .get("owner_world_transform_sha256")
            .and_then(Value::as_str)
            != Some(link.owner_world_transform_sha256.as_str())
        || receipt
            .get("trail_inventory_sha256")
            .and_then(Value::as_str)
            != Some(link.trail_inventory_sha256.as_str())
        || receipt
            .get("trail_id_encoding_sha256")
            .and_then(Value::as_str)
            != Some(link.trail_id_encoding_sha256.as_str())
        || receipt
            .get("source_trail_color_object_sha256")
            .and_then(Value::as_str)
            != Some(link.source_trail_color_object_sha256.as_str())
        || receipt
            .get("source_trail_id_object_sha256")
            .and_then(Value::as_str)
            != Some(link.source_trail_id_object_sha256.as_str())
        || receipt
            .get("source_trail_depth_object_sha256")
            .and_then(Value::as_str)
            != Some(link.source_trail_depth_object_sha256.as_str())
        || receipt.get("source_object_sha256").and_then(Value::as_str)
            != Some(own_source_hash.as_str())
        || receipt
            .get("contribution_object_sha256")
            .and_then(Value::as_str)
            != Some(own_contribution_hash.as_str())
        || render_set
            .get("trail_bloom_key_sha256")
            .and_then(Value::as_str)
            != Some(trail_bloom_key_sha256)
        || render_set
            .get("base_frame_key_sha256")
            .and_then(Value::as_str)
            != Some(link.base_frame_key_sha256.as_str())
        || render_set.get("bloom_key_sha256").and_then(Value::as_str)
            != Some(link.bloom_key_sha256.as_str())
        || render_set
            .get("source_trail_key_sha256")
            .and_then(Value::as_str)
            != Some(link.source_trail_key_sha256.as_str())
        || render_set.get("candidate_id").and_then(Value::as_str)
            != Some(link.source_candidate_id.as_str())
        || render_set.get("artifact_sha256").and_then(Value::as_str)
            != Some(link.source_artifact_sha256.as_str())
        || render_set.get("camera_hash").and_then(Value::as_str)
            != Some(link.camera_identity_sha256.as_str())
        || render_set
            .get("camera_object_sha256")
            .and_then(Value::as_str)
            != Some(link.camera_object_sha256.as_str())
        || render_set
            .get("render_profile_sha256")
            .and_then(Value::as_str)
            != Some(link.render_profile_sha256.as_str())
        || render_set
            .get("render_worker_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(link.render_worker_build_cohort_sha256.as_str())
        || render_set
            .get("trail_bloom_profile_sha256")
            .and_then(Value::as_str)
            != Some(profile_sha256.as_str())
        || render_set
            .get("base_opaque_depth_object_sha256")
            .and_then(Value::as_str)
            != Some(link.base_opaque_depth_object_sha256.as_str())
        || render_set.get("trail_seed_sha256").and_then(Value::as_str)
            != Some(link.trail_seed_sha256.as_str())
        || render_set
            .get("trail_inventory_sha256")
            .and_then(Value::as_str)
            != Some(link.trail_inventory_sha256.as_str())
        || render_set
            .get("trail_id_encoding_sha256")
            .and_then(Value::as_str)
            != Some(link.trail_id_encoding_sha256.as_str())
        || render_set
            .get("node_inventory_sha256")
            .and_then(Value::as_str)
            != Some(link.node_inventory_sha256.as_str())
        || render_set
            .get("owner_world_transform_sha256")
            .and_then(Value::as_str)
            != Some(link.owner_world_transform_sha256.as_str())
        || render_set
            .get("source_trail_color_object_sha256")
            .and_then(Value::as_str)
            != Some(link.source_trail_color_object_sha256.as_str())
        || render_set
            .get("source_trail_id_object_sha256")
            .and_then(Value::as_str)
            != Some(link.source_trail_id_object_sha256.as_str())
        || render_set
            .get("source_trail_depth_object_sha256")
            .and_then(Value::as_str)
            != Some(link.source_trail_depth_object_sha256.as_str())
    {
        return invalid("durable typed trail Bloom receipt or RenderSet binding differs");
    }
    if receipt.get("source_trail_passes") != Some(&source_trail_passes) {
        return invalid("durable typed trail Bloom source pass binding differs");
    }
    Ok(json!({
        "schema_version":"FictionalEnergyVfxTrailsBloomFrameGetResult@1",
        "trail_bloom_key_sha256":trail_bloom_key_sha256,
        "receipt_object_sha256":link.receipt_object_sha256,
        "receipt":receipt,
        "render_set":render_set,
        "link":link,
        "restart_hash_verified":true,
        "runtime_write_performed":false,
        "candidate_confirmed":false,
        "export_performed":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only"
    }))
}

fn sample_energy_vfx_effect(
    effect: &Value,
    requested_time_ticks: u64,
) -> Result<Value, RuntimeError> {
    let object = effect
        .as_object()
        .ok_or_else(|| error("fictional energy VFX effect is invalid"))?;
    let duration_ticks = object
        .get("duration_ticks")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("fictional energy VFX duration is invalid"))?;
    let loop_mode = text(object, "loop_mode")?;
    let resolved_time_ticks = match loop_mode {
        "once" => requested_time_ticks.min(duration_ticks),
        "loop" => requested_time_ticks % duration_ticks,
        _ => return invalid("fictional energy VFX loop mode differs"),
    };
    let times = object
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .ok_or_else(|| error("fictional energy VFX sample times are unavailable"))?;
    let strengths = object
        .get("emissive_strength_samples")
        .and_then(Value::as_array)
        .ok_or_else(|| error("fictional energy VFX sample strengths are unavailable"))?;
    let mut emissive_strength = strengths
        .last()
        .and_then(Value::as_f64)
        .ok_or_else(|| error("fictional energy VFX sample strength is invalid"))?;
    for index in 0..times.len().saturating_sub(1) {
        let start = times[index]
            .as_u64()
            .ok_or_else(|| error("fictional energy VFX sample time is invalid"))?;
        let end = times[index + 1]
            .as_u64()
            .ok_or_else(|| error("fictional energy VFX sample time is invalid"))?;
        if resolved_time_ticks <= end {
            let start_strength = strengths[index]
                .as_f64()
                .ok_or_else(|| error("fictional energy VFX sample strength is invalid"))?;
            let end_strength = strengths[index + 1]
                .as_f64()
                .ok_or_else(|| error("fictional energy VFX sample strength is invalid"))?;
            let alpha = (resolved_time_ticks.saturating_sub(start)) as f64 / (end - start) as f64;
            emissive_strength = start_strength + (end_strength - start_strength) * alpha;
            break;
        }
    }
    if !emissive_strength.is_finite() || !(0.0..=16.0).contains(&emissive_strength) {
        return invalid("fictional energy VFX interpolated strength is invalid");
    }
    Ok(json!({
        "effect_id":object["effect_id"],
        "anchor_id":object["anchor_id"],
        "effect_kind":object["effect_kind"],
        "material_id":object["material_id"],
        "color_linear_rgb":object["color_linear_rgb"],
        "loop_mode":loop_mode,
        "duration_ticks":duration_ticks,
        "requested_time_ticks":requested_time_ticks,
        "resolved_time_ticks":resolved_time_ticks,
        "emissive_strength":emissive_strength,
        "lod_visibility":object["lod_visibility"]
    }))
}

fn validate_energy_vfx_effects(value: Option<&Value>) -> Result<Vec<Value>, RuntimeError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or_else(|| error("fictional energy VFX requires exactly two effects"))?;
    let expected = [
        (
            "muzzle-pulse",
            "socket-muzzle-vfx",
            "muzzle-emissive-pulse",
            "energy-cyan-muzzle-emissive",
            "once",
            json!([true, true, false]),
        ),
        (
            "energy-core-breathe",
            "socket-energy-core-vfx",
            "energy-core-emissive-breathe",
            "energy-cyan-emissive",
            "loop",
            json!([true, true, true]),
        ),
    ];
    let mut by_id = BTreeMap::new();
    for value in values {
        let object = exact_object(
            value,
            &[
                "effect_id",
                "anchor_id",
                "effect_kind",
                "material_id",
                "color_linear_rgb",
                "duration_ticks",
                "sample_time_ticks",
                "emissive_strength_samples",
                "loop_mode",
                "lod_visibility",
            ],
            "fictional energy VFX effect",
        )?;
        let effect_id = identifier(object, "effect_id")?;
        if by_id.insert(effect_id.to_owned(), value.clone()).is_some() {
            return invalid("fictional energy VFX effect ID is duplicated");
        }
    }
    for (effect_id, anchor_id, effect_kind, material_id, loop_mode, lod_visibility) in &expected {
        let effect = by_id.get(*effect_id).ok_or_else(|| {
            error(format!(
                "fictional energy VFX effect is missing: {effect_id}"
            ))
        })?;
        let object = effect.as_object().expect("validated VFX effect");
        if text(object, "anchor_id")? != *anchor_id
            || text(object, "effect_kind")? != *effect_kind
            || text(object, "material_id")? != *material_id
            || text(object, "loop_mode")? != *loop_mode
            || object.get("lod_visibility") != Some(lod_visibility)
        {
            return invalid("fictional energy VFX effect binding differs");
        }
        let color = finite_vector(object.get("color_linear_rgb"), 3, 1.0)?;
        if color.iter().any(|value| *value < 0.0) || color.iter().all(|value| *value == 0.0) {
            return invalid("fictional energy VFX color must be visible");
        }
        let duration = object
            .get("duration_ticks")
            .and_then(Value::as_u64)
            .filter(|value| (1..=10_000).contains(value))
            .ok_or_else(|| error("fictional energy VFX duration is invalid"))?;
        let times = object
            .get("sample_time_ticks")
            .and_then(Value::as_array)
            .filter(|items| (2..=16).contains(&items.len()))
            .ok_or_else(|| error("fictional energy VFX sample times are invalid"))?;
        let strengths = finite_vector(object.get("emissive_strength_samples"), times.len(), 16.0)?;
        let parsed_times = times
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| error("fictional energy VFX sample time is invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if parsed_times.first() != Some(&0)
            || parsed_times.last() != Some(&duration)
            || parsed_times.windows(2).any(|pair| pair[0] >= pair[1])
            || strengths.iter().any(|value| *value < 0.0)
            || strengths
                .iter()
                .all(|value| (*value - strengths[0]).abs() <= 1.0e-9)
        {
            return invalid("fictional energy VFX time/strength samples differ");
        }
    }
    Ok(expected
        .iter()
        .map(|(effect_id, _, _, _, _, _)| {
            by_id.get(*effect_id).expect("complete VFX effects").clone()
        })
        .collect())
}

fn part_bounds_from_lod(level: &Value) -> Result<BTreeMap<String, Bounds>, RuntimeError> {
    let mut result = BTreeMap::new();
    let values = level
        .get("part_bounds")
        .and_then(Value::as_array)
        .ok_or_else(|| error("weapon anchor LOD0 Part bounds are unavailable"))?;
    for value in values {
        let part_id = value
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| forgecad_contracts::is_opaque_id(value))
            .ok_or_else(|| error("weapon anchor bound Part ID is invalid"))?;
        let vector = |field: &str| -> Result<[f64; 3], RuntimeError> {
            let values = value
                .get(field)
                .and_then(Value::as_array)
                .filter(|values| values.len() == 3)
                .ok_or_else(|| error("weapon anchor Part bound vector is invalid"))?;
            let mut output = [0.0; 3];
            for axis in 0..3 {
                output[axis] = values[axis]
                    .as_f64()
                    .filter(|number| number.is_finite())
                    .ok_or_else(|| error("weapon anchor Part bound is non-finite"))?;
            }
            Ok(output)
        };
        let bounds = Bounds {
            min: vector("min_m")?,
            max: vector("max_m")?,
        }
        .validate()?;
        if result.insert(part_id.to_owned(), bounds).is_some() {
            return invalid("weapon anchor Part bounds contain a duplicate");
        }
    }
    Ok(result)
}

#[derive(Debug)]
pub(super) struct MaterializedSocketGlb {
    pub(super) glb: Vec<u8>,
    pub(super) source_renderable_inventory_sha256: String,
    pub(super) derived_renderable_inventory_sha256: String,
    pub(super) socket_node_inventory_sha256: String,
    pub(super) socket_nodes: Vec<Value>,
    pub(super) source_bin_sha256: String,
    pub(super) derived_bin_sha256: String,
    pub(super) source_node_count: usize,
    pub(super) derived_node_count: usize,
}

pub(super) fn socket_anchor_ids(anchor_set: &Value) -> Result<Vec<String>, RuntimeError> {
    const EXPECTED: [(&str, &str); 6] = [
        ("weapon-root", "weapon-root"),
        ("grip-primary", "grip-primary"),
        ("socket-muzzle-vfx", "muzzle-vfx"),
        ("socket-magazine-well", "magazine-well"),
        ("socket-sight-primary", "sight-primary"),
        ("socket-energy-core-vfx", "energy-core-vfx"),
    ];
    if anchor_set.get("schema_version").and_then(Value::as_str) != Some("GameWeaponAnchorSet@1")
        || anchor_set
            .get("node_materialization")
            .and_then(Value::as_str)
            != Some("sidecar-only-not-glb-nodes")
    {
        return invalid("GLB socket source AnchorSet schema or materialization boundary differs");
    }
    let anchors = anchor_set
        .get("anchors")
        .and_then(Value::as_array)
        .filter(|values| values.len() == EXPECTED.len())
        .ok_or_else(|| error("GLB socket AnchorSet must contain exactly six anchors"))?;
    let mut by_role = BTreeMap::<String, &Value>::new();
    for anchor in anchors {
        let object = exact_object(
            anchor,
            &[
                "anchor_id",
                "role",
                "parent_kind",
                "owner_part_id",
                "local_translation_m",
                "local_rotation_quat_xyzw",
                "local_scale_xyz",
            ],
            "GLB socket AnchorSet anchor",
        )?;
        let role = identifier(object, "role")?.to_owned();
        if by_role.insert(role, anchor).is_some() {
            return invalid("GLB socket AnchorSet roles are duplicated");
        }
    }
    let mut result = Vec::with_capacity(EXPECTED.len());
    for (expected_id, expected_role) in EXPECTED {
        let anchor = by_role
            .get(expected_role)
            .copied()
            .ok_or_else(|| error("GLB socket AnchorSet role is missing"))?;
        if anchor.get("anchor_id").and_then(Value::as_str) != Some(expected_id) {
            return invalid("GLB socket AnchorSet role/ID mapping differs");
        }
        result.push(expected_id.to_owned());
    }
    Ok(result)
}

pub(super) fn socket_node_id_encoding_value() -> Result<Value, RuntimeError> {
    let mut value = json!({
        "schema_version":"GameWeaponGlbSocketNodeIdEncoding@1",
        "encoding":GAME_WEAPON_GLB_SOCKET_NODE_ID_ENCODING,
        "prefix":GAME_WEAPON_GLB_SOCKET_NODE_PREFIX,
        "name_pattern":"forgecad-anchor-{anchor_id}",
        "canonical_sha256":""
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    Ok(value)
}

fn socket_readback_value(
    socket_materialization_key_sha256: &str,
    lod_level: u64,
    loaded: &Level,
    derived_artifact_sha256: &str,
    materialized: &MaterializedSocketGlb,
) -> Result<Value, RuntimeError> {
    let mut readback = json!({
        "schema_version":GAME_WEAPON_GLB_SOCKET_READBACK_SCHEMA,
        "socket_materialization_key_sha256":socket_materialization_key_sha256,
        "lod_level":lod_level,
        "source_candidate_id":loaded.candidate_id,
        "source_candidate_state_sha256":loaded.candidate_state_sha256,
        "source_artifact_sha256":loaded.artifact_sha256,
        "source_artifact_readback_sha256":loaded.artifact_readback_sha256,
        "derived_artifact_sha256":derived_artifact_sha256,
        "derived_artifact_readback_sha256":"",
        "source_renderable_inventory_sha256":materialized.source_renderable_inventory_sha256,
        "derived_renderable_inventory_sha256":materialized.derived_renderable_inventory_sha256,
        "socket_node_inventory_sha256":materialized.socket_node_inventory_sha256,
        "source_bin_sha256":materialized.source_bin_sha256,
        "derived_bin_sha256":materialized.derived_bin_sha256,
        "source_renderable_projection_exact":true,
        "source_bin_byte_exact":true,
        "socket_nodes_materialized":true,
        "source_node_count":materialized.source_node_count,
        "derived_node_count":materialized.derived_node_count,
        "socket_node_count":materialized.socket_nodes.len(),
        "socket_nodes":materialized.socket_nodes,
        "canonical_sha256":""
    });
    // The child durable index points at this inline readback digest.  The
    // digest preimage intentionally leaves the binding field and canonical
    // field empty, avoiding a self-referential hash while remaining replayable.
    readback["derived_artifact_readback_sha256"] = Value::String(canonical_json_hash(&readback));
    seal_sidecar(readback)
}

pub(super) fn materialize_socket_glb(
    source_bytes: &[u8],
    source_artifact_sha256: &str,
    anchor_set_object_sha256: &str,
    anchor_set: &Value,
    part_ids: &[String],
    anchor_ids: &[String],
) -> Result<MaterializedSocketGlb, RuntimeError> {
    if source_bytes.is_empty()
        || source_bytes.len() as u64 > MAX_GLB_BYTES
        || sha256_hex(source_bytes) != source_artifact_sha256
    {
        return invalid("GLB socket source bytes or hash differs");
    }
    let (source_root, source_binary) = parse_glb(source_bytes)?;
    if source_root
        .get("buffers")
        .and_then(Value::as_array)
        .and_then(|buffers| buffers.first())
        .and_then(|buffer| buffer.get("byteLength"))
        .and_then(Value::as_u64)
        != Some(source_binary.len() as u64)
    {
        return invalid("GLB socket source BIN length differs from glTF buffer declaration");
    }
    let part_nodes = validate_socket_source_graph(&source_root, part_ids)?;
    let source_node_count = source_root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| error("GLB socket source nodes are unavailable"))?
        .len();
    let source_renderable_inventory =
        socket_renderable_inventory(&source_root, &source_binary, part_ids, &part_nodes)?;

    let anchors = anchor_set
        .get("anchors")
        .and_then(Value::as_array)
        .filter(|values| values.len() == anchor_ids.len())
        .ok_or_else(|| error("GLB socket AnchorSet anchor inventory is incomplete"))?;
    let anchors_by_id = anchors
        .iter()
        .filter_map(|anchor| Some((anchor.get("anchor_id")?.as_str()?.to_owned(), anchor)))
        .collect::<BTreeMap<_, _>>();
    if anchors_by_id.len() != anchor_ids.len()
        || anchor_ids
            .iter()
            .any(|anchor_id| !anchors_by_id.contains_key(anchor_id))
    {
        return invalid("GLB socket AnchorSet anchor IDs differ");
    }
    let mut root = source_root.clone();
    let source_scene = root
        .get("scene")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("GLB socket source scene index is unavailable"))?;
    if source_scene != 0 {
        return invalid("GLB socket source scene index differs");
    }
    if !forgecad_contracts::is_sha256(anchor_set_object_sha256) {
        return invalid("GLB socket AnchorSet object hash is invalid");
    }
    let anchor_set_canonical_sha256 = anchor_set
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("GLB socket AnchorSet canonical hash is invalid"))?;
    let mut node_indices = BTreeMap::<String, usize>::new();
    let mut owner_children = Vec::<(usize, usize)>::new();
    for anchor_id in anchor_ids {
        let anchor = anchors_by_id
            .get(anchor_id)
            .copied()
            .ok_or_else(|| error("GLB socket AnchorSet anchor is unavailable"))?;
        let object = anchor
            .as_object()
            .ok_or_else(|| error("GLB socket AnchorSet anchor is invalid"))?;
        let role = object
            .get("role")
            .and_then(Value::as_str)
            .ok_or_else(|| error("GLB socket AnchorSet anchor role is invalid"))?;
        let parent_kind = object
            .get("parent_kind")
            .and_then(Value::as_str)
            .ok_or_else(|| error("GLB socket AnchorSet parent kind is invalid"))?;
        let owner_part_id = object
            .get("owner_part_id")
            .ok_or_else(|| error("GLB socket AnchorSet owner Part is unavailable"))?;
        let node_name = format!("{GAME_WEAPON_GLB_SOCKET_NODE_PREFIX}{anchor_id}");
        if root["nodes"].as_array().is_some_and(|nodes| {
            nodes
                .iter()
                .any(|node| node.get("name").and_then(Value::as_str) == Some(&node_name))
        }) {
            return invalid("GLB socket anchor node name collides with source node");
        }
        socket_anchor_transform(object)?;
        let node_index = source_node_count + node_indices.len();
        if node_indices
            .insert(anchor_id.to_owned(), node_index)
            .is_some()
        {
            return invalid("GLB socket anchor node IDs are duplicated");
        }
        let node = json!({
            "name":node_name,
            "translation":object["local_translation_m"],
            "rotation":object["local_rotation_quat_xyzw"],
            "scale":object["local_scale_xyz"],
            "extras":{
                "forgecad":{
                    "schema_version":GAME_WEAPON_GLB_SOCKET_NODE_SCHEMA,
                    "anchor_id":anchor_id,
                    "role":role,
                    "parent_kind":parent_kind,
                    "owner_part_id":owner_part_id,
                    "anchor_set_object_sha256":anchor_set_object_sha256,
                    "anchor_set_canonical_sha256":anchor_set_canonical_sha256
                }
            }
        });
        root["nodes"]
            .as_array_mut()
            .ok_or_else(|| error("GLB socket output nodes are unavailable"))?
            .push(node);
        match parent_kind {
            "synthetic-scene-root" => {
                if !owner_part_id.is_null() || anchor_id != "weapon-root" {
                    return invalid("GLB socket synthetic root anchor binding differs");
                }
                root["scenes"][0]["nodes"]
                    .as_array_mut()
                    .ok_or_else(|| error("GLB socket output scene roots are unavailable"))?
                    .push(Value::from(node_index as u64));
            }
            "part-node" => {
                let owner_part_id = owner_part_id
                    .as_str()
                    .filter(|value| part_ids.iter().any(|part_id| part_id == value))
                    .ok_or_else(|| error("GLB socket owner Part is invalid"))?;
                let owner_node_index = *part_nodes
                    .get(owner_part_id)
                    .ok_or_else(|| error("GLB socket owner Part node is unavailable"))?;
                owner_children.push((owner_node_index, node_index));
            }
            _ => return invalid("GLB socket parent kind is outside the closed set"),
        }
    }
    for (owner_node_index, child_node_index) in &owner_children {
        let node = root["nodes"]
            .as_array_mut()
            .and_then(|nodes| nodes.get_mut(*owner_node_index))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| error("GLB socket owner node is unavailable"))?;
        let children = node
            .entry("children")
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| error("GLB socket owner children field is invalid"))?;
        if children
            .iter()
            .any(|value| value.as_u64() == Some(*child_node_index as u64))
        {
            return invalid("GLB socket owner child node is duplicated");
        }
        children.push(Value::from(*child_node_index as u64));
    }
    let socket_node_inventory = socket_node_inventory_value(
        anchor_set_object_sha256,
        anchor_set,
        anchor_ids,
        source_node_count,
    )?;
    let forgecad = root
        .get_mut("extras")
        .and_then(Value::as_object_mut)
        .and_then(|extras| extras.get_mut("forgecad"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| error("GLB socket source forgecad extras are unavailable"))?;
    if forgecad.contains_key(GAME_WEAPON_GLB_SOCKET_ROOT_EXTRA) {
        return invalid("GLB socket source already contains materialization metadata");
    }
    let mut materialization = json!({
        "schema_version":"GameWeaponGlbSocketMaterialization@1",
        "materialization_policy":GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_POLICY,
        "anchor_set_object_sha256":anchor_set_object_sha256,
        "anchor_set_canonical_sha256":anchor_set_canonical_sha256,
        "source_artifact_sha256":source_artifact_sha256,
        "anchor_ids":anchor_ids,
        "source_node_count":source_node_count,
        "derived_node_count":source_node_count + anchor_ids.len(),
        "socket_node_inventory_sha256":socket_node_inventory["canonical_sha256"],
        "attachment_mode":"part-node-children-and-scene-root@1",
        "canonical_sha256":""
    });
    materialization["canonical_sha256"] = Value::String(canonical_json_hash(&materialization));
    forgecad.insert(
        GAME_WEAPON_GLB_SOCKET_ROOT_EXTRA.to_owned(),
        materialization,
    );
    let derived_binary = source_binary.clone();
    let derived_glb = encode_glb(&root, &derived_binary)?;
    validate_socket_projection(
        &source_root,
        &source_binary,
        &root,
        &derived_binary,
        source_node_count,
        &owner_children,
        node_indices
            .get("weapon-root")
            .copied()
            .ok_or_else(|| error("GLB socket weapon-root node is unavailable"))?,
    )?;
    let derived_renderable_inventory =
        socket_renderable_inventory(&root, &derived_binary, part_ids, &part_nodes)?;
    let source_hash = source_renderable_inventory
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("GLB socket source inventory hash is unavailable"))?;
    let derived_hash = derived_renderable_inventory
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("GLB socket derived inventory hash is unavailable"))?;
    if source_hash != derived_hash {
        return invalid("GLB socket renderable inventory changed during materialization");
    }
    Ok(MaterializedSocketGlb {
        glb: derived_glb,
        source_renderable_inventory_sha256: source_hash.to_owned(),
        derived_renderable_inventory_sha256: derived_hash.to_owned(),
        socket_node_inventory_sha256: socket_node_inventory
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| error("GLB socket node inventory hash is unavailable"))?
            .to_owned(),
        socket_nodes: socket_node_inventory
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| error("GLB socket node inventory nodes are unavailable"))?
            .clone(),
        source_bin_sha256: sha256_hex(&source_binary),
        derived_bin_sha256: sha256_hex(&derived_binary),
        source_node_count,
        derived_node_count: source_node_count + anchor_ids.len(),
    })
}

fn socket_anchor_transform(object: &Map<String, Value>) -> Result<(), RuntimeError> {
    let translation = finite_vector(object.get("local_translation_m"), 3, 10.0)?;
    let quaternion = finite_vector(object.get("local_rotation_quat_xyzw"), 4, 1.0)?;
    if (quaternion.iter().map(|value| value * value).sum::<f64>() - 1.0).abs() > 1.0e-6
        || object.get("local_scale_xyz") != Some(&json!([1.0, 1.0, 1.0]))
        || translation
            .iter()
            .any(|value| ((*value as f32) as f64 - *value).abs() > 1.0e-6)
        || quaternion
            .iter()
            .any(|value| ((*value as f32) as f64 - *value).abs() > 1.0e-6)
    {
        return invalid("GLB socket anchor TRS is non-unit, scaled or not f32-stable");
    }
    Ok(())
}

fn validate_socket_source_graph(
    root: &Value,
    part_ids: &[String],
) -> Result<BTreeMap<String, usize>, RuntimeError> {
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| !nodes.is_empty() && nodes.len() <= MAX_PARTS)
        .ok_or_else(|| error("GLB socket source nodes are outside the bounded domain"))?;
    let scenes = root
        .get("scenes")
        .and_then(Value::as_array)
        .filter(|scenes| scenes.len() == 1)
        .ok_or_else(|| error("GLB socket source scenes are not exactly one"))?;
    if root.get("scene").and_then(Value::as_u64) != Some(0) {
        return invalid("GLB socket source scene is not scene zero");
    }
    let scene_roots = scenes[0]
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| error("GLB socket source scene roots are unavailable"))?;
    let mut names = BTreeSet::new();
    for node in nodes {
        if let Some(name) = node.get("name").and_then(Value::as_str) {
            if name.is_empty() || !names.insert(name.to_owned()) {
                return invalid("GLB socket source node names are not unique");
            }
        }
    }
    let mut visited = BTreeSet::new();
    let mut part_nodes = BTreeMap::<String, usize>::new();
    fn visit(
        index: usize,
        nodes: &[Value],
        part_ids: &[String],
        visited: &mut BTreeSet<usize>,
        part_nodes: &mut BTreeMap<String, usize>,
    ) -> Result<(), RuntimeError> {
        if !visited.insert(index) {
            return invalid("GLB socket source node graph has a cycle or duplicate instance");
        }
        let node = nodes
            .get(index)
            .ok_or_else(|| error("GLB socket source node index is invalid"))?;
        if let Some(name) = node.get("name").and_then(Value::as_str) {
            if part_ids.iter().any(|part_id| part_id == name) {
                if node.get("mesh").and_then(Value::as_u64).is_none()
                    || part_nodes.insert(name.to_owned(), index).is_some()
                {
                    return invalid("GLB socket source Part node is not exact-one renderable");
                }
            }
        }
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for child in children {
                let child = child
                    .as_u64()
                    .ok_or_else(|| error("GLB socket source child node index is invalid"))?
                    as usize;
                visit(child, nodes, part_ids, visited, part_nodes)?;
            }
        }
        Ok(())
    }
    for root_node in scene_roots {
        let root_node = root_node
            .as_u64()
            .ok_or_else(|| error("GLB socket source scene root index is invalid"))?
            as usize;
        visit(root_node, nodes, part_ids, &mut visited, &mut part_nodes)?;
    }
    if visited.len() != nodes.len() || part_nodes.keys().cloned().collect::<Vec<_>>() != part_ids {
        return invalid("GLB socket source node graph or Part coverage differs");
    }
    validate_socket_animation_targets(root, nodes.len())?;
    if root
        .get("buffers")
        .and_then(Value::as_array)
        .is_none_or(|buffers| {
            buffers.len() != 1
                || buffers[0]
                    .get("byteLength")
                    .and_then(Value::as_u64)
                    .is_none()
        })
        || root.get("meshes").and_then(Value::as_array).is_none()
        || root.get("materials").and_then(Value::as_array).is_none()
        || root.get("bufferViews").and_then(Value::as_array).is_none()
        || root.get("accessors").and_then(Value::as_array).is_none()
    {
        return invalid("GLB socket source renderable arrays are incomplete");
    }
    Ok(part_nodes)
}

fn validate_socket_animation_targets(root: &Value, node_count: usize) -> Result<(), RuntimeError> {
    let Some(animations) = root.get("animations") else {
        return Ok(());
    };
    let animations = animations
        .as_array()
        .ok_or_else(|| error("GLB socket source animations are invalid"))?;
    for animation in animations {
        let channels = animation
            .get("channels")
            .and_then(Value::as_array)
            .ok_or_else(|| error("GLB socket source animation channels are invalid"))?;
        for channel in channels {
            let node_index = channel
                .get("target")
                .and_then(|target| target.get("node"))
                .and_then(Value::as_u64)
                .ok_or_else(|| error("GLB socket source animation target is invalid"))?
                as usize;
            if node_index >= node_count {
                return invalid("GLB socket source animation targets a future node");
            }
        }
    }
    Ok(())
}

fn socket_renderable_inventory(
    root: &Value,
    binary: &[u8],
    part_ids: &[String],
    part_nodes: &BTreeMap<String, usize>,
) -> Result<Value, RuntimeError> {
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| error("GLB socket renderable inventory nodes are unavailable"))?;
    let mut part_values = Vec::with_capacity(part_ids.len());
    for part_id in part_ids {
        let node_index = *part_nodes
            .get(part_id)
            .ok_or_else(|| error("GLB socket renderable Part node is unavailable"))?;
        let node = nodes
            .get(node_index)
            .and_then(Value::as_object)
            .ok_or_else(|| error("GLB socket renderable Part node is invalid"))?;
        let mut node_projection = Value::Object(node.clone());
        node_projection
            .as_object_mut()
            .expect("node projection is an object")
            .remove("children");
        part_values.push(json!({
            "part_id":part_id,
            "node_index":node_index,
            "node":node_projection
        }));
    }
    let hash_field =
        |field: &str| -> String { canonical_json_hash(root.get(field).unwrap_or(&Value::Null)) };
    let mut inventory = json!({
        "schema_version":GAME_WEAPON_GLB_RENDERABLE_INVENTORY_SCHEMA,
        "part_nodes":part_values,
        "meshes_sha256":hash_field("meshes"),
        "materials_sha256":hash_field("materials"),
        "buffers_sha256":hash_field("buffers"),
        "buffer_views_sha256":hash_field("bufferViews"),
        "accessors_sha256":hash_field("accessors"),
        "images_sha256":hash_field("images"),
        "textures_sha256":hash_field("textures"),
        "animations_sha256":hash_field("animations"),
        "binary_sha256":sha256_hex(binary),
        "canonical_sha256":""
    });
    inventory["canonical_sha256"] = Value::String(canonical_json_hash(&inventory));
    Ok(inventory)
}

fn socket_node_inventory_value(
    anchor_set_object_sha256: &str,
    anchor_set: &Value,
    anchor_ids: &[String],
    source_node_count: usize,
) -> Result<Value, RuntimeError> {
    let anchors = anchor_set
        .get("anchors")
        .and_then(Value::as_array)
        .ok_or_else(|| error("GLB socket node inventory anchors are unavailable"))?;
    let by_id = anchors
        .iter()
        .filter_map(|anchor| Some((anchor.get("anchor_id")?.as_str()?, anchor)))
        .collect::<BTreeMap<_, _>>();
    let mut nodes = Vec::with_capacity(anchor_ids.len());
    for anchor_id in anchor_ids {
        let anchor = by_id
            .get(anchor_id.as_str())
            .copied()
            .ok_or_else(|| error("GLB socket node inventory anchor is unavailable"))?;
        let parent_kind = anchor["parent_kind"]
            .as_str()
            .ok_or_else(|| error("GLB socket node inventory parent kind is invalid"))?;
        let owner_part_id = anchor
            .get("owner_part_id")
            .ok_or_else(|| error("GLB socket node inventory owner Part is unavailable"))?;
        let parent_node_name = if parent_kind == "synthetic-scene-root" {
            Value::Null
        } else {
            Value::String(
                owner_part_id
                    .as_str()
                    .ok_or_else(|| error("GLB socket node inventory owner Part is invalid"))?
                    .to_owned(),
            )
        };
        nodes.push(json!({
            "socket_node_id":anchor_id,
            "anchor_id":anchor_id,
            "role":anchor["role"],
            "node_name":format!("{GAME_WEAPON_GLB_SOCKET_NODE_PREFIX}{anchor_id}"),
            "node_kind":"empty",
            "parent_kind":parent_kind,
            "parent_node_name":parent_node_name,
            "owner_part_id":owner_part_id,
            "local_translation_m":anchor["local_translation_m"],
            "local_rotation_quat_xyzw":anchor["local_rotation_quat_xyzw"],
            "local_scale_xyz":anchor["local_scale_xyz"]
        }));
    }
    let mut inventory = json!({
        "schema_version":GAME_WEAPON_GLB_SOCKET_NODE_INVENTORY_SCHEMA,
        "anchor_set_object_sha256":anchor_set_object_sha256,
        "anchor_set_canonical_sha256":anchor_set["canonical_sha256"],
        "source_node_count":source_node_count,
        "nodes":nodes,
        "canonical_sha256":""
    });
    inventory["canonical_sha256"] = Value::String(canonical_json_hash(&inventory));
    Ok(inventory)
}

fn validate_socket_projection(
    source_root: &Value,
    source_binary: &[u8],
    derived_root: &Value,
    derived_binary: &[u8],
    source_node_count: usize,
    owner_children: &[(usize, usize)],
    weapon_root_index: usize,
) -> Result<(), RuntimeError> {
    if source_binary != derived_binary
        || derived_root.get("meshes") != source_root.get("meshes")
        || derived_root.get("materials") != source_root.get("materials")
        || derived_root.get("buffers") != source_root.get("buffers")
        || derived_root.get("animations") != source_root.get("animations")
    {
        return invalid("GLB socket source renderable arrays or BIN are not exact");
    }
    let mut stripped = derived_root.clone();
    let derived_nodes = stripped
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| error("GLB socket derived nodes are unavailable"))?;
    if derived_nodes.len() != source_node_count + 6 {
        return invalid("GLB socket derived node count differs from six anchors");
    }
    derived_nodes.truncate(source_node_count);
    let appended_children = owner_children
        .iter()
        .map(|(_, child)| *child as u64)
        .collect::<BTreeSet<_>>();
    let owners = owner_children
        .iter()
        .map(|(owner, _)| *owner)
        .collect::<BTreeSet<_>>();
    for owner in owners {
        let node = derived_nodes
            .get_mut(owner)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| error("GLB socket stripped owner node is unavailable"))?;
        if let Some(children) = node.get_mut("children") {
            let values = children
                .as_array_mut()
                .ok_or_else(|| error("GLB socket stripped owner children are invalid"))?;
            values.retain(|value| !appended_children.contains(&value.as_u64().unwrap_or(u64::MAX)));
            if values.is_empty() {
                node.remove("children");
            }
        }
    }
    let scene_nodes = stripped
        .get_mut("scenes")
        .and_then(Value::as_array_mut)
        .and_then(|scenes| scenes.get_mut(0))
        .and_then(Value::as_object_mut)
        .and_then(|scene| scene.get_mut("nodes"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| error("GLB socket stripped scene roots are unavailable"))?;
    scene_nodes.retain(|value| value.as_u64() != Some(weapon_root_index as u64));
    let forgecad = stripped
        .get_mut("extras")
        .and_then(Value::as_object_mut)
        .and_then(|extras| extras.get_mut("forgecad"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| error("GLB socket stripped forgecad extras are unavailable"))?;
    forgecad.remove(GAME_WEAPON_GLB_SOCKET_ROOT_EXTRA);
    if stripped != *source_root {
        return invalid("GLB socket strip-and-compare did not reconstruct source GLB");
    }
    Ok(())
}

fn validate_socket_receipt(
    receipt: &Value,
    socket_key_sha256: &str,
    project_id: &str,
    parent: &GameWeaponGlbSocketMaterializationLinkRecord,
    anchor_ids: &[String],
    lods: &[Value],
) -> Result<(), RuntimeError> {
    let _ = anchor_ids;
    exact_object(
        receipt,
        &[
            "schema_version",
            "socket_materialization_key_sha256",
            "project_id",
            "delivery_manifest_object_sha256",
            "anchor_set_object_sha256",
            "anchor_set_canonical_sha256",
            "request_sha256",
            "socket_materialization_policy",
            "lod_scope",
            "socket_node_id_encoding_sha256",
            "levels",
            "semantic_scope",
            "functional_semantics",
            "materialization_status",
            "runtime_write_performed",
            "candidate_confirmed",
            "export_performed",
            "actual_engine_roundtrip",
            "quality_status",
            "limitations",
            "canonical_sha256",
            "created_at",
        ],
        GAME_WEAPON_GLB_SOCKET_RECEIPT_SCHEMA,
    )?;
    if receipt.get("schema_version").and_then(Value::as_str)
        != Some(GAME_WEAPON_GLB_SOCKET_RECEIPT_SCHEMA)
        || receipt
            .get("socket_materialization_key_sha256")
            .and_then(Value::as_str)
            != Some(socket_key_sha256)
        || receipt.get("project_id").and_then(Value::as_str) != Some(project_id)
        || receipt
            .get("delivery_manifest_object_sha256")
            .and_then(Value::as_str)
            != Some(parent.delivery_manifest_object_sha256.as_str())
        || receipt
            .get("anchor_set_object_sha256")
            .and_then(Value::as_str)
            != Some(parent.anchor_set_object_sha256.as_str())
        || receipt
            .get("anchor_set_canonical_sha256")
            .and_then(Value::as_str)
            != Some(parent.anchor_set_canonical_sha256.as_str())
        || receipt.get("request_sha256").and_then(Value::as_str) != Some(socket_key_sha256)
        || receipt
            .get("socket_materialization_policy")
            .and_then(Value::as_str)
            != Some(GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_POLICY)
        || receipt.get("lod_scope").and_then(Value::as_str)
            != Some(GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_LOD_SCOPE)
        || receipt.get("levels") != Some(&Value::Array(lods.to_vec()))
        || receipt.get("semantic_scope").and_then(Value::as_str)
            != Some("fictional-nonfunctional-game-visual-authoring-only@1")
        || receipt.get("functional_semantics").and_then(Value::as_bool) != Some(false)
        || receipt
            .get("materialization_status")
            .and_then(Value::as_str)
            != Some(GAME_WEAPON_GLB_SOCKET_MATERIALIZATION_STATUS)
        || receipt
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(true)
        || receipt.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || receipt.get("export_performed").and_then(Value::as_bool) != Some(false)
        || receipt
            .get("actual_engine_roundtrip")
            .and_then(Value::as_bool)
            != Some(false)
        || receipt.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || receipt.get("limitations")
            != Some(&json!([
                "no-ballistics",
                "no-damage-or-hitbox",
                "no-physics-binding",
                "no-manufacturing-or-operation",
                "no-commercial-engine-roundtrip",
                "no-runtime-pivot-proof",
                "no-visual-quality-pass"
            ]))
    {
        return invalid("GLB socket receipt semantics differ");
    }
    Ok(())
}

fn validate_weapon_anchors(
    value: Option<&Value>,
    part_ids: &[String],
    bounds: &BTreeMap<String, Bounds>,
) -> Result<Vec<Value>, RuntimeError> {
    const EXPECTED: [(&str, &str); 6] = [
        ("weapon-root", "weapon-root"),
        ("grip-primary", "grip-primary"),
        ("socket-muzzle-vfx", "muzzle-vfx"),
        ("socket-magazine-well", "magazine-well"),
        ("socket-sight-primary", "sight-primary"),
        ("socket-energy-core-vfx", "energy-core-vfx"),
    ];
    let anchors = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == EXPECTED.len())
        .ok_or_else(|| error("weapon anchor set requires exactly six roles"))?;
    let mut by_role = BTreeMap::new();
    let mut owner_parts = BTreeSet::new();
    for anchor in anchors {
        let object = exact_object(
            anchor,
            &[
                "anchor_id",
                "role",
                "parent_kind",
                "owner_part_id",
                "local_translation_m",
                "local_rotation_quat_xyzw",
                "local_scale_xyz",
            ],
            "weapon anchor",
        )?;
        let role = identifier(object, "role")?;
        if by_role.insert(role.to_owned(), anchor.clone()).is_some() {
            return invalid("weapon anchor role is duplicated");
        }
    }
    for (expected_id, role) in EXPECTED {
        let anchor = by_role
            .get(role)
            .ok_or_else(|| error(format!("weapon anchor role is missing: {role}")))?;
        let object = anchor.as_object().expect("validated weapon anchor");
        if identifier(object, "anchor_id")? != expected_id {
            return invalid("weapon anchor role/ID mapping differs");
        }
        let translation = finite_vector(object.get("local_translation_m"), 3, 10.0)?;
        let quaternion = finite_vector(object.get("local_rotation_quat_xyzw"), 4, 1.0)?;
        let norm = quaternion.iter().map(|value| value * value).sum::<f64>();
        if (norm - 1.0).abs() > 1.0e-6
            || object.get("local_scale_xyz") != Some(&json!([1.0, 1.0, 1.0]))
        {
            return invalid("weapon anchor quaternion or scale differs");
        }
        if role == "weapon-root" {
            if text(object, "parent_kind")? != "synthetic-scene-root"
                || !object.get("owner_part_id").is_some_and(Value::is_null)
                || translation != [0.0, 0.0, 0.0]
                || quaternion != [0.0, 0.0, 0.0, 1.0]
            {
                return invalid("weapon root must be an identity synthetic root");
            }
            continue;
        }
        if text(object, "parent_kind")? != "part-node" {
            return invalid("weapon attachment helper must be bound to a Part node");
        }
        let owner_part_id = object
            .get("owner_part_id")
            .and_then(Value::as_str)
            .filter(|value| forgecad_contracts::is_opaque_id(value))
            .ok_or_else(|| error("weapon attachment owner Part is invalid"))?;
        if !part_ids.iter().any(|part| part == owner_part_id)
            || !owner_parts.insert(owner_part_id.to_owned())
        {
            return invalid("weapon attachment owner Parts must exist and be unique");
        }
        let semantic_match = match role {
            "grip-primary" => owner_part_id.contains("grip"),
            "muzzle-vfx" => owner_part_id.contains("muzzle") || owner_part_id.contains("barrel"),
            "magazine-well" => owner_part_id.contains("mag"),
            "sight-primary" => owner_part_id.contains("sight") || owner_part_id.contains("optic"),
            "energy-core-vfx" => owner_part_id.contains("energy") || owner_part_id.contains("core"),
            _ => false,
        };
        if !semantic_match {
            return invalid("weapon attachment role/Part semantic binding differs");
        }
        let part_bounds = bounds
            .get(owner_part_id)
            .ok_or_else(|| error("weapon attachment owner Part bounds are unavailable"))?;
        for axis in 0..3 {
            let epsilon = (part_bounds.max[axis] - part_bounds.min[axis]) * 0.001 + 1.0e-6;
            if translation[axis] < part_bounds.min[axis] - epsilon
                || translation[axis] > part_bounds.max[axis] + epsilon
            {
                return invalid("weapon attachment translation is outside its owner Part");
            }
        }
        if role == "muzzle-vfx" {
            let width = part_bounds.max[0] - part_bounds.min[0];
            if translation[0] < part_bounds.max[0] - width * 0.1 - 1.0e-6 {
                return invalid("muzzle visual anchor is not on the forward +X end");
            }
        }
    }
    Ok(EXPECTED
        .iter()
        .map(|(_, role)| by_role.get(*role).expect("complete anchor roles").clone())
        .collect())
}

fn finite_vector(value: Option<&Value>, len: usize, limit: f64) -> Result<Vec<f64>, RuntimeError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == len)
        .ok_or_else(|| error("weapon anchor vector length differs"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|number| number.is_finite() && number.abs() <= limit)
                .ok_or_else(|| error("weapon anchor vector is non-finite or out of range"))
        })
        .collect()
}

/// Derive two lower-detail GeometryProgram variants from one exact durable
/// geometry candidate. This is deliberately a zero-write preview: Runtime
/// reloads and replays the persisted source program, lowers only a closed set
/// of tessellation/detail integers, compiles every derived program twice with
/// the fixed same-cohort Worker, and returns materializable typed programs.
/// It never performs arbitrary triangle decimation and fails closed when the
/// program cannot satisfy the 75/50 percent delivery triangle gates.
pub(super) fn derive_lods(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "source_candidate_id",
            "source_candidate_state_sha256",
            "source_artifact_sha256",
            "source_artifact_readback_sha256",
            "source_geometry_program_sha256",
            "source_operator_catalog_sha256",
            "source_readback_config_sha256",
            "derive_policy",
            "canonical_sha256",
        ],
        "GameAssetLodDeriveRequest@1",
    )?;
    if text(object, "schema_version")? != "GameAssetLodDeriveRequest@1"
        || text(object, "derive_policy")? != LOD_DERIVE_POLICY
    {
        return invalid("automatic LOD derive policy differs");
    }
    verify_request_canonical(request)?;
    let project_id = identifier(object, "project_id")?;
    let candidate_id = identifier(object, "source_candidate_id")?;
    let candidate_state_sha256 = sha(object, "source_candidate_state_sha256")?;
    let artifact_sha256 = sha(object, "source_artifact_sha256")?;
    let artifact_readback_sha256 = sha(object, "source_artifact_readback_sha256")?;
    let program_sha256 = sha(object, "source_geometry_program_sha256")?;
    let operator_catalog_sha256 = sha(object, "source_operator_catalog_sha256")?;
    let readback_config_sha256 = sha(object, "source_readback_config_sha256")?;

    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| error("automatic LOD source candidate is unavailable"))?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != candidate_state_sha256
        || candidate.prepared_object_sha256.as_deref() != Some(artifact_sha256)
        || candidate.manifest_hash.as_deref() != Some(artifact_sha256)
    {
        return invalid("automatic LOD source candidate binding differs");
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| error("automatic LOD source geometry evidence is unavailable"))?;
    let evidence_value = serde_json::to_value(&evidence)
        .map_err(|source| error(format!("source geometry evidence is invalid: {source}")))?;
    validate_geometry_candidate_evidence_output(&evidence_value)
        .map_err(|_| error("source geometry evidence canonical binding differs"))?;
    if evidence.project_id != project_id
        || evidence.candidate_id != candidate_id
        || evidence.artifact_object_sha256 != artifact_sha256
        || evidence.geometry_program_sha256 != program_sha256
        || evidence.operator_catalog_sha256 != operator_catalog_sha256
        || evidence.readback_config_sha256 != readback_config_sha256
    {
        return invalid("automatic LOD source evidence binding differs");
    }
    let artifact_record = runtime
        .store
        .get_object(artifact_sha256)?
        .ok_or_else(|| error("automatic LOD source artifact metadata is unavailable"))?;
    if artifact_record.kind != "geometry-glb"
        || artifact_record.mime != "model/gltf-binary"
        || artifact_record.size_bytes == 0
        || artifact_record.size_bytes > MAX_GLB_BYTES
    {
        return invalid("automatic LOD requires an exact geometry-only GLB source");
    }
    let source_bytes = runtime.cas_read_bounded(artifact_sha256, MAX_GLB_BYTES)?;
    if sha256_hex(&source_bytes) != artifact_sha256 {
        return invalid("automatic LOD source artifact bytes differ");
    }
    let source_readback =
        runtime.artifact_readback_bounded(artifact_sha256, candidate_id, MAX_GLB_BYTES)?;
    if source_readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(artifact_readback_sha256)
        || source_readback
            .get("program_sha256")
            .and_then(Value::as_str)
            != Some(program_sha256)
        || source_readback
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(operator_catalog_sha256)
        || source_readback
            .get("readback_config_sha256")
            .and_then(Value::as_str)
            != Some(readback_config_sha256)
    {
        return invalid("automatic LOD source ArtifactReadback binding differs");
    }
    let program_record = runtime
        .store
        .get_object(&evidence.geometry_program_object_sha256)?
        .ok_or_else(|| error("automatic LOD source GeometryProgram is unavailable"))?;
    if program_record.kind != "geometry-program-v2"
        || program_record.mime != "application/json"
        || program_record.size_bytes == 0
        || program_record.size_bytes > MAX_JSON_BYTES
    {
        return invalid("automatic LOD source GeometryProgram metadata differs");
    }
    let program_bytes =
        runtime.cas_read_bounded(&evidence.geometry_program_object_sha256, MAX_JSON_BYTES)?;
    let mut source_program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|_| error("automatic LOD source GeometryProgram is invalid JSON"))?;
    if source_program.get("canonical_sha256").is_some()
        || source_program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || source_program.get("project_id").and_then(Value::as_str) != Some(project_id)
    {
        return invalid("automatic LOD persisted GeometryProgram draft shape differs");
    }
    let source_hash =
        hash_geometry_program_with_runtime_worker(&source_program).map_err(|source| {
            error(format!(
                "automatic LOD source program hash failed: {source}"
            ))
        })?;
    if source_hash.get("canonical_sha256").and_then(Value::as_str) != Some(program_sha256)
        || source_hash
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(operator_catalog_sha256)
    {
        return invalid("automatic LOD source GeometryProgram hash differs");
    }
    source_program["canonical_sha256"] = Value::String(program_sha256.to_owned());
    let source_replay = compile_geometry_with_runtime_worker(&source_program, None)
        .map_err(|source| error(format!("automatic LOD source replay failed: {source}")))?;
    let source_repeat = compile_geometry_with_runtime_worker(&source_program, None)
        .map_err(|source| error(format!("automatic LOD source repeat failed: {source}")))?;
    let source_inspection = validate_replayed_artifact(&source_replay, &source_repeat)?;
    if source_replay.glb != source_bytes || source_replay.program_sha256 != program_sha256 {
        return invalid("automatic LOD source replay is not byte-exact");
    }
    let source_triangles = source_replay.triangle_count;
    let source_parts = sorted_strings(&source_replay.part_ids);
    let source_zones = sorted_strings(&source_replay.material_zone_ids);
    let source_binding_identity = binding_identity(&source_inspection);
    let (source_bindings, source_bounds, source_transient_readback, source_cohort) =
        transient_level_evidence(&source_replay, &source_inspection, &source_parts)?;

    let mut levels = vec![json!({
        "level":0,
        "source_kind":"durable-source-artifact",
        "geometry_program":source_program,
        "geometry_program_sha256":program_sha256,
        "transient_artifact_sha256":artifact_sha256,
        "triangle_count":source_triangles,
        "triangle_ratio_to_lod0":1.0,
        "parameter_changes":[],
        "part_bindings":source_bindings,
        "part_bounds":source_bounds,
        "transient_readback":source_transient_readback,
        "worker_build_cohort_sha256":source_cohort,
        "worker_replay_count":2,
        "replay_byte_exact":true
    })];
    let base_program = levels[0]["geometry_program"].clone();
    for (level, divisor) in [(1u64, 2u64), (2u64, 4u64)] {
        let (derived_program, changes) = lower_lod_program(&base_program, divisor)?;
        if changes.is_empty() {
            return Err(RuntimeError::InvalidInput(format!(
                "LOD_TARGET_UNACHIEVABLE: LOD{level} has no allowlisted reducible detail parameter"
            )));
        }
        let first = compile_geometry_with_runtime_worker(&derived_program, None)
            .map_err(|source| error(format!("automatic LOD{level} compile failed: {source}")))?;
        let repeat = compile_geometry_with_runtime_worker(&derived_program, None)
            .map_err(|source| error(format!("automatic LOD{level} repeat failed: {source}")))?;
        let inspection = validate_replayed_artifact(&first, &repeat)?;
        if sorted_strings(&first.part_ids) != source_parts
            || sorted_strings(&first.material_zone_ids) != source_zones
            || binding_identity(&inspection) != source_binding_identity
        {
            return invalid("automatic LOD Part/source/material/solid binding differs");
        }
        let (part_bindings, part_bounds, transient_readback, worker_cohort) =
            transient_level_evidence(&first, &inspection, &source_parts)?;
        validate_derived_envelopes(
            &levels[0]["part_bounds"],
            &Value::Array(part_bounds.clone()),
        )?;
        let target_ok = if level == 1 {
            first.triangle_count < source_triangles
                && first.triangle_count.saturating_mul(4) <= source_triangles.saturating_mul(3)
        } else {
            first.triangle_count < levels[1]["triangle_count"].as_u64().unwrap_or(u64::MAX)
                && first.triangle_count.saturating_mul(2) <= source_triangles
        };
        if !target_ok {
            return Err(RuntimeError::InvalidInput(format!(
                "LOD_TARGET_UNACHIEVABLE: LOD{level} produced {} triangles from {source_triangles}",
                first.triangle_count
            )));
        }
        let derived_sha = sha256_hex(&first.glb);
        levels.push(json!({
            "level":level,
            "source_kind":"runtime-derived-transient-program",
            "geometry_program":derived_program,
            "geometry_program_sha256":first.program_sha256,
            "transient_artifact_sha256":derived_sha,
            "triangle_count":first.triangle_count,
            "triangle_ratio_to_lod0":first.triangle_count as f64 / source_triangles as f64,
            "parameter_changes":changes,
            "part_bindings":part_bindings,
            "part_bounds":part_bounds,
            "transient_readback":transient_readback,
            "worker_build_cohort_sha256":worker_cohort,
            "worker_replay_count":2,
            "replay_byte_exact":true
        }));
    }
    let mut result = json!({
        "schema_version":"GameAssetLodDeriveResult@1",
        "project_id":project_id,
        "source_candidate_id":candidate_id,
        "source_candidate_state_sha256":candidate_state_sha256,
        "source_artifact_sha256":artifact_sha256,
        "source_artifact_readback_sha256":artifact_readback_sha256,
        "source_geometry_program_sha256":program_sha256,
        "derive_policy":LOD_DERIVE_POLICY,
        "levels":levels,
        "stable_part_ids":source_parts,
        "stable_material_zone_ids":source_zones,
        "worker_replay_verified":true,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "materialization_required":true,
        "quality_status":"structural_only",
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    if canonical_json_bytes(&result)
        .map_err(|source| {
            error(format!(
                "automatic LOD result canonicalization failed: {source}"
            ))
        })?
        .len()
        > MAX_JSON_BYTES as usize
    {
        return Err(RuntimeError::InvalidInput(
            "GAME_ASSET_LOD_DERIVE_RESPONSE_TOO_LARGE: result exceeds 1 MiB".to_owned(),
        ));
    }
    Ok(result)
}

fn validate_replayed_artifact(
    first: &super::geometry_worker::GeometryArtifact,
    repeat: &super::geometry_worker::GeometryArtifact,
) -> Result<super::integrity::GlbIntegrity, RuntimeError> {
    if first.glb != repeat.glb
        || first.program_sha256 != repeat.program_sha256
        || first.triangle_count != repeat.triangle_count
        || first.part_ids != repeat.part_ids
        || first.material_zone_ids != repeat.material_zone_ids
        || first.build_cohort_sha256 != repeat.build_cohort_sha256
    {
        return invalid("automatic LOD Geometry Worker replay differs");
    }
    let inspection = strict_glb_inspection(&first.glb)?;
    validate_worker_metadata(first, &inspection)?;
    if !inspection.hard_gate_passed {
        return invalid("automatic LOD strict GLB readback failed");
    }
    let cohort = first
        .build_cohort_sha256
        .as_deref()
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| error("LOD_WORKER_COHORT_MISMATCH: Worker cohort is unavailable"))?;
    if repeat.build_cohort_sha256.as_deref() != Some(cohort) {
        return invalid("LOD_WORKER_COHORT_MISMATCH: replay cohort differs");
    }
    Ok(inspection)
}

fn binding_identity(
    inspection: &super::integrity::GlbIntegrity,
) -> Vec<(String, String, String, bool)> {
    let mut values = inspection
        .part_bindings
        .iter()
        .map(|binding| {
            (
                binding.part_id.clone(),
                binding.source_node_id.clone(),
                binding.material_zone_id.clone(),
                binding.solid,
            )
        })
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn transient_level_evidence(
    artifact: &super::geometry_worker::GeometryArtifact,
    inspection: &super::integrity::GlbIntegrity,
    part_ids: &[String],
) -> Result<(Vec<Value>, Vec<Value>, Value, String), RuntimeError> {
    let mut bindings = inspection
        .part_bindings
        .iter()
        .map(|binding| {
            json!({
                "part_id":binding.part_id,
                "source_node_id":binding.source_node_id,
                "material_zone_id":binding.material_zone_id,
                "solid":binding.solid,
                "triangle_count":binding.triangle_count
            })
        })
        .collect::<Vec<_>>();
    bindings.sort_by(|left, right| canonical_json_hash(left).cmp(&canonical_json_hash(right)));
    let bounds = extract_part_bounds(&artifact.glb, part_ids)?
        .into_iter()
        .map(|(part_id, bounds)| bounds.value(&part_id))
        .collect::<Vec<_>>();
    let operator_catalog_sha256 = inspection
        .operator_catalog_sha256
        .as_deref()
        .ok_or_else(|| error("automatic LOD GLB omitted operator catalog binding"))?;
    if artifact.uv_status != "passed"
        || artifact.tangent_status != "passed"
        || inspection.external_uri_count != 0
    {
        return invalid("LOD_UV_TANGENT_READBACK_FAILED");
    }
    let mut readback = json!({
        "schema_version":"GameLodTransientReadback@1",
        "storage":"memory-only-no-CAS",
        "program_sha256":inspection.program_sha256,
        "operator_catalog_sha256":operator_catalog_sha256,
        "readback_config_sha256":inspection.readback_config_sha256,
        "uv_status":artifact.uv_status,
        "tangent_status":artifact.tangent_status,
        "external_uri_count":inspection.external_uri_count,
        "hard_gate_passed":inspection.hard_gate_passed,
        "canonical_sha256":""
    });
    readback["canonical_sha256"] = Value::String(canonical_json_hash(&readback));
    let cohort = artifact
        .build_cohort_sha256
        .as_deref()
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| error("LOD_WORKER_COHORT_MISMATCH: Worker cohort is unavailable"))?
        .to_owned();
    Ok((bindings, bounds, readback, cohort))
}

fn validate_derived_envelopes(source: &Value, derived: &Value) -> Result<(), RuntimeError> {
    let source = source
        .as_array()
        .ok_or_else(|| error("automatic LOD source bounds are unavailable"))?;
    let derived = derived
        .as_array()
        .ok_or_else(|| error("automatic LOD derived bounds are unavailable"))?;
    if source.len() != derived.len() {
        return invalid("LOD_ENVELOPE_DRIFT: Part bound coverage differs");
    }
    for (source, derived) in source.iter().zip(derived) {
        if source.get("part_id") != derived.get("part_id") {
            return invalid("LOD_ENVELOPE_DRIFT: Part bound order differs");
        }
        for key in ["min_m", "max_m"] {
            let source_values = source
                .get(key)
                .and_then(Value::as_array)
                .ok_or_else(|| error("automatic LOD source bound is invalid"))?;
            let derived_values = derived
                .get(key)
                .and_then(Value::as_array)
                .ok_or_else(|| error("automatic LOD derived bound is invalid"))?;
            for axis in 0..3 {
                let source_value = source_values[axis]
                    .as_f64()
                    .ok_or_else(|| error("automatic LOD source bound is non-numeric"))?;
                let derived_value = derived_values[axis]
                    .as_f64()
                    .ok_or_else(|| error("automatic LOD derived bound is non-numeric"))?;
                let tolerance = source_value.abs().mul_add(0.1, 1.0e-5);
                if (derived_value - source_value).abs() > tolerance {
                    return invalid(
                        "LOD_ENVELOPE_DRIFT: Part bound differs by more than ten percent",
                    );
                }
            }
        }
    }
    Ok(())
}

fn sorted_strings(values: &[String]) -> Vec<String> {
    let mut result = values.to_vec();
    result.sort();
    result
}

fn lower_lod_program(source: &Value, divisor: u64) -> Result<(Value, Vec<Value>), RuntimeError> {
    let mut program = source.clone();
    let object = program
        .as_object_mut()
        .ok_or_else(|| error("automatic LOD source program is not an object"))?;
    object.remove("canonical_sha256");
    let nodes = object
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| error("automatic LOD source nodes are unavailable"))?;
    let mut changes = Vec::new();
    for node in nodes {
        let node_object = node
            .as_object_mut()
            .ok_or_else(|| error("automatic LOD source node is invalid"))?;
        let node_id = node_object
            .get("node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| error("automatic LOD node ID is unavailable"))?
            .to_owned();
        let operator_id = node_object
            .get("operator_id")
            .and_then(Value::as_str)
            .ok_or_else(|| error("automatic LOD operator ID is unavailable"))?
            .to_owned();
        let parameters = node_object
            .get_mut("parameters")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| error("automatic LOD parameters are unavailable"))?;
        let fields: &[(&str, u64)] = match operator_id.as_str() {
            "forgecad.geometry.primitive@2" => &[
                ("radial_segments", 8),
                ("longitude_segments", 8),
                ("latitude_segments", 4),
            ],
            "forgecad.geometry.subd-cage@1" => &[("subdivision_levels", 0)],
            "forgecad.geometry.subd-cage@2" => &[("subdivision_levels", 1)],
            "forgecad.geometry.surface-patch@1" | "forgecad.geometry.surface-shell@1" => {
                &[("u_segments", 4), ("v_segments", 4)]
            }
            "forgecad.geometry.revolve@1"
            | "forgecad.geometry.tube-sweep@1"
            | "forgecad.geometry.joint-stack@1" => &[("radial_segments", 8)],
            "forgecad.geometry.energy-core@1" => &[("radial_segments", 12)],
            "forgecad.geometry.bevel@1" | "forgecad.geometry.bevel@2" => &[("segments", 1)],
            "forgecad.geometry.panel@2" => &[("bevel_segments", 1)],
            "forgecad.geometry.vent-array@2" => &[("bevel_segments", 1)],
            "forgecad.geometry.recessed-channel@1" => &[("transition_segments", 1)],
            _ => &[],
        };
        for (field, minimum) in fields {
            let Some(source_value) = parameters.get(*field).and_then(Value::as_u64) else {
                continue;
            };
            let derived_value = (*minimum).max(source_value.div_ceil(divisor));
            if derived_value < source_value {
                parameters.insert((*field).to_owned(), Value::from(derived_value));
                changes.push(json!({
                    "node_id":node_id,
                    "operator_id":operator_id,
                    "parameter":field,
                    "source_value":source_value,
                    "derived_value":derived_value
                }));
            }
        }
    }
    let hash = hash_geometry_program_with_runtime_worker(&program).map_err(|source| {
        error(format!(
            "automatic LOD derived program hash failed: {source}"
        ))
    })?;
    let program_sha256 = hash
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("automatic LOD Worker omitted derived program hash"))?;
    program["canonical_sha256"] = Value::String(program_sha256.to_owned());
    Ok((program, changes))
}

fn load_level(
    runtime: &Runtime,
    project_id: &str,
    expected_level: u64,
    declaration: &Value,
) -> Result<Level, RuntimeError> {
    let object = exact_object(
        declaration,
        &[
            "level",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_sha256",
            "artifact_readback_sha256",
        ],
        "LOD declaration",
    )?;
    if object.get("level").and_then(Value::as_u64) != Some(expected_level) {
        return invalid("LOD declarations must be ordered LOD0, LOD1, LOD2");
    }
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let candidate_state_sha256 = sha(object, "candidate_state_sha256")?.to_owned();
    let artifact_sha256 = sha(object, "artifact_sha256")?.to_owned();
    let artifact_readback_sha256 = sha(object, "artifact_readback_sha256")?.to_owned();
    let candidate = runtime
        .candidate(&candidate_id)?
        .ok_or_else(|| error("LOD candidate is unavailable"))?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != candidate_state_sha256
        || candidate.manifest_hash.as_deref() != Some(artifact_sha256.as_str())
        || candidate.prepared_object_sha256.as_deref() != Some(artifact_sha256.as_str())
    {
        return invalid("LOD candidate/project/state/artifact binding differs");
    }
    let readback =
        runtime.artifact_readback_bounded(&artifact_sha256, &candidate_id, MAX_GLB_BYTES)?;
    if readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
        || readback.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        || readback.get("canonical_sha256").and_then(Value::as_str)
            != Some(artifact_readback_sha256.as_str())
    {
        return invalid("LOD strict ArtifactReadback@2 binding differs");
    }
    let triangle_count = readback
        .get("triangle_count")
        .and_then(Value::as_u64)
        .filter(|value| (1..=250_000).contains(value))
        .ok_or_else(|| error("LOD triangle count is invalid"))?;
    let part_ids = string_set(&readback, "part_ids")?;
    let source_node_ids = string_set(&readback, "source_node_ids")?;
    let material_zone_ids = string_set(&readback, "material_zone_ids")?;
    let part_material_bindings = strict_solid_part_material_bindings(&readback, &part_ids)?;
    if part_ids.len() > MAX_PARTS || material_zone_ids.len() > MAX_PARTS {
        return invalid("LOD semantic coverage exceeds 64 items");
    }
    let bytes = runtime.cas_read(&artifact_sha256)?;
    if bytes.len() as u64 > MAX_GLB_BYTES || sha256_hex(&bytes) != artifact_sha256 {
        return invalid("LOD GLB bytes differ from their CAS binding");
    }
    let record = runtime
        .store
        .get_object(&artifact_sha256)?
        .ok_or_else(|| error("LOD GLB object metadata is unavailable"))?;
    let appearance_binding = match record.kind.as_str() {
        "appearance-v2-glb" => Some(parse_appearance_binding(&bytes, &material_zone_ids)?),
        "geometry-glb" | "appearance-glb" => None,
        _ => return invalid("LOD GLB kind is unsupported"),
    };
    let bounds = extract_part_bounds(&bytes, &part_ids)?;
    Ok(Level {
        level: expected_level,
        candidate_id,
        candidate_state_sha256,
        artifact_sha256,
        artifact_readback_sha256,
        triangle_count,
        part_ids,
        source_node_ids,
        material_zone_ids,
        part_material_bindings,
        bounds,
        appearance_binding,
    })
}

fn verify_energy_vfx_appearance_binding(
    runtime: &Runtime,
    project_id: &str,
    delivery_manifest_object_sha256: &str,
    profile: &Value,
    effects: &[Value],
) -> Result<Option<Vec<Value>>, RuntimeError> {
    let delivery = get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_manifest_object_sha256
        }),
    )?;
    let declarations = delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error("VFX frame delivery LOD declarations are unavailable"))?;
    let levels = declarations
        .iter()
        .enumerate()
        .map(|(level, declaration)| {
            load_level(
                runtime,
                project_id,
                level as u64,
                &json!({
                    "level":declaration.get("level"),
                    "candidate_id":declaration.get("candidate_id"),
                    "candidate_state_sha256":declaration.get("candidate_state_sha256"),
                    "artifact_sha256":declaration.get("artifact_sha256"),
                    "artifact_readback_sha256":declaration.get("artifact_readback_sha256")
                }),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_level_set(&levels)?;
    let appearance_count = levels
        .iter()
        .filter(|level| level.appearance_binding.is_some())
        .count();
    if appearance_count == 0 {
        return Ok(None);
    }
    if appearance_count != levels.len() {
        return invalid("VFX frame delivery mixes geometry and Appearance GLBs");
    }
    let expected_pack_id = profile
        .get("material_pack_id")
        .and_then(Value::as_str)
        .ok_or_else(|| error("VFX frame profile MaterialPack ID is unavailable"))?;
    let expected_manifest_sha256 = profile
        .get("material_pack_manifest_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| error("VFX frame profile MaterialPack hash is unavailable"))?;
    let lod0_binding = levels[0]
        .appearance_binding
        .as_ref()
        .expect("appearance count proved complete");
    for level in &levels {
        let binding = level
            .appearance_binding
            .as_ref()
            .expect("appearance count proved complete");
        if !matches!(
            binding.schema_version.as_str(),
            "AppearanceProgram@2" | "AppearanceProgram@3"
        ) || !forgecad_contracts::is_sha256(&binding.program_sha256)
            || binding.material_pack_id != expected_pack_id
            || binding.material_pack_manifest_sha256 != expected_manifest_sha256
            || binding.zone_material_ids != lod0_binding.zone_material_ids
        {
            return invalid("VFX frame Appearance provenance or material map differs");
        }
    }
    let anchor = weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_manifest_object_sha256
        }),
    )?;
    let anchors = anchor
        .get("anchor_set")
        .and_then(|value| value.get("anchors"))
        .and_then(Value::as_array)
        .ok_or_else(|| error("VFX frame weapon anchors are unavailable"))?;
    let owners = anchors
        .iter()
        .filter_map(|anchor| {
            Some((
                anchor.get("anchor_id")?.as_str()?.to_owned(),
                anchor.get("owner_part_id")?.as_str()?.to_owned(),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    for effect in effects {
        let anchor_id = effect
            .get("anchor_id")
            .and_then(Value::as_str)
            .ok_or_else(|| error("VFX frame effect anchor is unavailable"))?;
        let material_id = effect
            .get("material_id")
            .and_then(Value::as_str)
            .ok_or_else(|| error("VFX frame effect material is unavailable"))?;
        let owner_part_id = owners
            .get(anchor_id)
            .ok_or_else(|| error("VFX frame effect anchor has no Part owner"))?;
        for level in &levels {
            let zone_id = level
                .part_material_bindings
                .get(owner_part_id)
                .ok_or_else(|| error("VFX frame anchor Part has no MaterialZone"))?;
            let actual_material_id = level
                .appearance_binding
                .as_ref()
                .and_then(|binding| binding.zone_material_ids.get(zone_id))
                .ok_or_else(|| error("VFX frame MaterialZone has no GLB material"))?;
            if actual_material_id != material_id {
                return invalid("VFX frame anchor Part/MaterialZone/material binding differs");
            }
        }
    }
    Ok(Some(
        levels
            .iter()
            .map(|level| -> Result<Value, RuntimeError> {
                let binding = level
                    .appearance_binding
                    .as_ref()
                    .expect("appearance count proved complete");
                let effect_material_zone_bindings = effects
                    .iter()
                    .map(|effect| -> Result<Value, RuntimeError> {
                        let effect_id = effect
                            .get("effect_id")
                            .and_then(Value::as_str)
                            .ok_or_else(|| error("VFX frame effect ID is unavailable"))?;
                        let anchor_id = effect
                            .get("anchor_id")
                            .and_then(Value::as_str)
                            .ok_or_else(|| error("VFX frame effect anchor is unavailable"))?;
                        let material_id = effect
                            .get("material_id")
                            .and_then(Value::as_str)
                            .ok_or_else(|| error("VFX frame effect material is unavailable"))?;
                        let owner_part_id = owners
                            .get(anchor_id)
                            .ok_or_else(|| error("VFX frame effect anchor has no Part owner"))?;
                        let material_zone_id = level
                            .part_material_bindings
                            .get(owner_part_id)
                            .ok_or_else(|| error("VFX frame anchor Part has no MaterialZone"))?;
                        Ok(json!({
                            "effect_id":effect_id,
                            "anchor_id":anchor_id,
                            "owner_part_id":owner_part_id,
                            "material_zone_id":material_zone_id,
                            "material_id":material_id
                        }))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(json!({
                    "level":level.level,
                    "candidate_id":level.candidate_id,
                    "candidate_state_sha256":level.candidate_state_sha256,
                    "artifact_sha256":level.artifact_sha256,
                    "artifact_readback_sha256":level.artifact_readback_sha256,
                    "appearance_program_schema_version":binding.schema_version,
                    "appearance_program_sha256":binding.program_sha256,
                    "material_pack_id":binding.material_pack_id,
                    "material_pack_manifest_sha256":binding.material_pack_manifest_sha256,
                    "zone_material_bindings":binding.zone_material_ids.iter().map(|(zone, material)| json!({
                        "material_zone_id":zone,
                        "material_id":material
                    })).collect::<Vec<_>>(),
                    "effect_material_zone_bindings":effect_material_zone_bindings
                }))
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn parse_appearance_binding(
    glb: &[u8],
    expected_material_zone_ids: &[String],
) -> Result<AppearanceBinding, RuntimeError> {
    if glb.len() < 20 || &glb[..4] != b"glTF" || &glb[16..20] != b"JSON" {
        return invalid("Appearance GLB header is invalid");
    }
    let json_length = u32::from_le_bytes(
        glb[12..16]
            .try_into()
            .map_err(|_| error("Appearance GLB JSON length is invalid"))?,
    ) as usize;
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| error("Appearance GLB JSON length overflowed"))?;
    if json_end > glb.len() {
        return invalid("Appearance GLB JSON chunk is truncated");
    }
    let root: Value = serde_json::from_slice(&glb[20..json_end])
        .map_err(|source| error(format!("Appearance GLB JSON is invalid: {source}")))?;
    let forgecad = root
        .get("extras")
        .and_then(|value| value.get("forgecad"))
        .and_then(Value::as_object)
        .ok_or_else(|| error("Appearance GLB ForgeCAD metadata is missing"))?;
    let schema_version = forgecad
        .get("appearance_program_schema_version")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "AppearanceProgram@2" | "AppearanceProgram@3"))
        .ok_or_else(|| error("Appearance GLB program schema provenance is missing"))?
        .to_owned();
    let program_sha256 = forgecad
        .get("appearance_program_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("Appearance GLB program hash provenance is missing"))?
        .to_owned();
    let material_pack_id = forgecad
        .get("material_pack_id")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_opaque_id(value))
        .ok_or_else(|| error("Appearance GLB MaterialPack ID is missing"))?
        .to_owned();
    let material_pack_manifest_sha256 = forgecad
        .get("material_pack_manifest_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("Appearance GLB MaterialPack hash is missing"))?
        .to_owned();
    let materials = root
        .get("materials")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= MAX_PARTS)
        .ok_or_else(|| error("Appearance GLB material inventory is invalid"))?;
    let mut zone_material_ids = BTreeMap::new();
    for material in materials {
        let zone_id = material
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| forgecad_contracts::is_opaque_id(value))
            .ok_or_else(|| error("Appearance GLB MaterialZone name is invalid"))?;
        let metadata = material
            .get("extras")
            .and_then(|value| value.get("forgecad"))
            .and_then(Value::as_object)
            .ok_or_else(|| error("Appearance GLB material metadata is missing"))?;
        let material_id = metadata
            .get("material_id")
            .and_then(Value::as_str)
            .filter(|value| forgecad_contracts::is_opaque_id(value))
            .ok_or_else(|| error("Appearance GLB stable material ID is invalid"))?;
        if metadata.get("material_pack_id").and_then(Value::as_str)
            != Some(material_pack_id.as_str())
            || metadata
                .get("material_pack_manifest_sha256")
                .and_then(Value::as_str)
                != Some(material_pack_manifest_sha256.as_str())
            || zone_material_ids
                .insert(zone_id.to_owned(), material_id.to_owned())
                .is_some()
        {
            return invalid("Appearance GLB material binding is duplicated or drifts");
        }
    }
    let actual_zones = zone_material_ids.keys().cloned().collect::<Vec<_>>();
    if actual_zones != expected_material_zone_ids {
        return invalid("Appearance GLB material inventory differs from strict readback");
    }
    Ok(AppearanceBinding {
        schema_version,
        program_sha256,
        material_pack_id,
        material_pack_manifest_sha256,
        zone_material_ids,
    })
}

fn validate_level_set(levels: &[Level]) -> Result<(), RuntimeError> {
    let lod0 = &levels[0];
    if levels.iter().skip(1).any(|level| {
        level.part_ids != lod0.part_ids
            || level.source_node_ids != lod0.source_node_ids
            || level.material_zone_ids != lod0.material_zone_ids
            || level.part_material_bindings != lod0.part_material_bindings
    }) {
        return invalid("LOD semantic Part-to-MaterialZone binding differs");
    }
    if levels[1].triangle_count >= lod0.triangle_count
        || levels[2].triangle_count >= levels[1].triangle_count
        || levels[1].triangle_count * 4 > lod0.triangle_count * 3
        || levels[2].triangle_count * 2 > lod0.triangle_count
    {
        return invalid("LOD triangle counts do not satisfy the progressive 75/50 percent policy");
    }
    for level in levels.iter().skip(1) {
        for part_id in &lod0.part_ids {
            let reference = lod0
                .bounds
                .get(part_id)
                .ok_or_else(|| error("LOD0 Part bound is missing"))?;
            let candidate = level
                .bounds
                .get(part_id)
                .ok_or_else(|| error("derived LOD Part bound is missing"))?;
            let reference_center = reference.center();
            let candidate_center = candidate.center();
            let reference_extent = reference.half_extents();
            let candidate_extent = candidate.half_extents();
            for axis in 0..3 {
                let center_tolerance = (reference_extent[axis] * 0.1).max(1.0e-5);
                let extent_tolerance = (reference_extent[axis] * 0.1).max(1.0e-5);
                if (candidate_center[axis] - reference_center[axis]).abs() > center_tolerance
                    || (candidate_extent[axis] - reference_extent[axis]).abs() > extent_tolerance
                {
                    return invalid("LOD Part envelope differs by more than ten percent");
                }
            }
        }
    }
    Ok(())
}

fn extract_part_bounds(
    bytes: &[u8],
    expected_parts: &[String],
) -> Result<BTreeMap<String, Bounds>, RuntimeError> {
    let (root, binary) = parse_glb(bytes)?;
    let nodes = array_field(&root, "nodes")?;
    let meshes = array_field(&root, "meshes")?;
    let accessors = array_field(&root, "accessors")?;
    let views = array_field(&root, "bufferViews")?;
    let expected = expected_parts.iter().cloned().collect::<BTreeSet<_>>();
    let mut result = BTreeMap::new();
    for node in nodes {
        let Some(part_id) = node.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !expected.contains(part_id) {
            continue;
        }
        if result.contains_key(part_id) {
            return invalid("GLB has duplicate Part node names");
        }
        let mesh_index = node
            .get("mesh")
            .and_then(Value::as_u64)
            .ok_or_else(|| error("Part node has no mesh"))? as usize;
        let primitives = meshes
            .get(mesh_index)
            .and_then(|mesh| mesh.get("primitives"))
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty())
            .ok_or_else(|| error("Part mesh has no primitives"))?;
        let mut bound = Bounds::empty();
        for primitive in primitives {
            let accessor_index = primitive
                .get("attributes")
                .and_then(|value| value.get("POSITION"))
                .and_then(Value::as_u64)
                .ok_or_else(|| error("Part primitive has no POSITION accessor"))?
                as usize;
            include_positions(&mut bound, accessors, views, &binary, accessor_index)?;
        }
        result.insert(part_id.to_owned(), bound.validate()?);
    }
    if result.keys().cloned().collect::<Vec<_>>() != expected_parts {
        return invalid("GLB does not contain exact-one mesh node for every Part");
    }
    Ok(result)
}

type F64Mat4 = [[f64; 4]; 4];

fn identity_f64_mat4() -> F64Mat4 {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn multiply_f64_mat4(left: F64Mat4, right: F64Mat4) -> F64Mat4 {
    let mut output = [[0.0; 4]; 4];
    for row in 0..4 {
        for column in 0..4 {
            output[column][row] = (0..4)
                .map(|index| left[index][row] * right[column][index])
                .sum();
        }
    }
    output
}

fn finite_f64(value: Option<&Value>, label: &str, limit: f64) -> Result<f64, RuntimeError> {
    let value = value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && value.abs() <= limit)
        .ok_or_else(|| error(format!("{label} is outside the bounded finite domain")))?;
    Ok(value)
}

fn node_local_f64_mat4(node: &Value) -> Result<F64Mat4, RuntimeError> {
    if let Some(values) = node.get("matrix").and_then(Value::as_array) {
        if node.get("translation").is_some()
            || node.get("rotation").is_some()
            || node.get("scale").is_some()
        {
            return invalid("particle owner node cannot mix matrix and TRS");
        }
        if values.len() != 16 {
            return invalid("particle owner node matrix must contain sixteen values");
        }
        let mut matrix = [[0.0; 4]; 4];
        for (index, value) in values.iter().enumerate() {
            matrix[index % 4][index / 4] = finite_f64(Some(value), "particle node matrix", 100.0)?;
        }
        return Ok(matrix);
    }
    let vector = |field: &str, fallback: [f64; 3]| -> Result<[f64; 3], RuntimeError> {
        let Some(value) = node.get(field) else {
            return Ok(fallback);
        };
        let values = value
            .as_array()
            .filter(|values| values.len() == 3)
            .ok_or_else(|| error(format!("particle node {field} must contain three values")))?;
        Ok([
            finite_f64(values.first(), field, 100.0)?,
            finite_f64(values.get(1), field, 100.0)?,
            finite_f64(values.get(2), field, 100.0)?,
        ])
    };
    let translation = vector("translation", [0.0, 0.0, 0.0])?;
    let scale = vector("scale", [1.0, 1.0, 1.0])?;
    let rotation = if let Some(value) = node.get("rotation") {
        let values = value
            .as_array()
            .filter(|values| values.len() == 4)
            .ok_or_else(|| error("particle node rotation must contain four values"))?;
        [
            finite_f64(values.first(), "particle node rotation", 1.0)?,
            finite_f64(values.get(1), "particle node rotation", 1.0)?,
            finite_f64(values.get(2), "particle node rotation", 1.0)?,
            finite_f64(values.get(3), "particle node rotation", 1.0)?,
        ]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    };
    let [x, y, z, w] = rotation;
    let norm = (x * x + y * y + z * z + w * w).sqrt();
    if !norm.is_finite() || (norm - 1.0).abs() > 1.0e-6 {
        return invalid("particle owner node rotation is not unit length");
    }
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    Ok([
        [
            (1.0 - 2.0 * (yy + zz)) * scale[0],
            (2.0 * (x * y + w * z)) * scale[0],
            (2.0 * (x * z - w * y)) * scale[0],
            0.0,
        ],
        [
            (2.0 * (x * y - w * z)) * scale[1],
            (1.0 - 2.0 * (xx + zz)) * scale[1],
            (2.0 * (y * z + w * x)) * scale[1],
            0.0,
        ],
        [
            (2.0 * (x * z + w * y)) * scale[2],
            (2.0 * (y * z - w * x)) * scale[2],
            (1.0 - 2.0 * (xx + yy)) * scale[2],
            0.0,
        ],
        [translation[0], translation[1], translation[2], 1.0],
    ])
}

fn transform_f64_point(matrix: F64Mat4, point: [f64; 3]) -> [f64; 3] {
    [
        matrix[0][0] * point[0] + matrix[1][0] * point[1] + matrix[2][0] * point[2] + matrix[3][0],
        matrix[0][1] * point[0] + matrix[1][1] * point[1] + matrix[2][1] * point[2] + matrix[3][1],
        matrix[0][2] * point[0] + matrix[1][2] * point[1] + matrix[2][2] * point[2] + matrix[3][2],
    ]
}

fn extract_part_world_nodes(
    bytes: &[u8],
    expected_parts: &[String],
) -> Result<(Value, BTreeMap<String, F64Mat4>), RuntimeError> {
    let (root, _) = parse_glb(bytes)?;
    let nodes = array_field(&root, "nodes")?;
    let expected = expected_parts.iter().cloned().collect::<BTreeSet<_>>();
    let mut by_part = BTreeMap::<String, (usize, usize, F64Mat4)>::new();
    let mut visited = BTreeSet::new();
    let scene_index = root.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
    let roots = root
        .get("scenes")
        .and_then(Value::as_array)
        .and_then(|scenes| scenes.get(scene_index))
        .and_then(|scene| scene.get("nodes"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| {
            (0..nodes.len())
                .map(|index| Value::from(index as u64))
                .collect()
        });
    fn visit(
        index: usize,
        parent: F64Mat4,
        nodes: &[Value],
        expected: &BTreeSet<String>,
        visited: &mut BTreeSet<usize>,
        by_part: &mut BTreeMap<String, (usize, usize, F64Mat4)>,
    ) -> Result<(), RuntimeError> {
        if !visited.insert(index) {
            return invalid("particle owner node graph contains a cycle or duplicate instance");
        }
        let node = nodes
            .get(index)
            .ok_or_else(|| error("particle owner node index is invalid"))?;
        let local = node_local_f64_mat4(node)?;
        let world = multiply_f64_mat4(parent, local);
        if let Some(part_id) = node.get("name").and_then(Value::as_str) {
            if expected.contains(part_id) {
                let mesh_index = node
                    .get("mesh")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| error("particle owner Part node has no mesh"))?
                    as usize;
                if by_part
                    .insert(part_id.to_owned(), (index, mesh_index, world))
                    .is_some()
                {
                    return invalid("particle owner Part node is not exact-one");
                }
            }
        }
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for child in children {
                let child = child
                    .as_u64()
                    .ok_or_else(|| error("particle owner node child index is invalid"))?
                    as usize;
                visit(child, world, nodes, expected, visited, by_part)?;
            }
        }
        Ok(())
    }
    for root_node in roots {
        let index = root_node
            .as_u64()
            .ok_or_else(|| error("particle owner scene root index is invalid"))?
            as usize;
        visit(
            index,
            identity_f64_mat4(),
            nodes,
            &expected,
            &mut visited,
            &mut by_part,
        )?;
    }
    if by_part.keys().cloned().collect::<Vec<_>>() != expected_parts {
        return invalid("particle owner node inventory does not cover exact LOD0 Parts");
    }
    let inventory_nodes = by_part
        .iter()
        .map(|(part_id, (node_index, mesh_index, world))| {
            json!({
                "part_id":part_id,
                "node_index":node_index,
                "mesh_index":mesh_index,
                "world_transform":world
            })
        })
        .collect::<Vec<_>>();
    let mut inventory = json!({
        "schema_version":"FictionalEnergyVfxParticleNodeInventory@1",
        "nodes":inventory_nodes,
        "canonical_sha256":""
    });
    inventory["canonical_sha256"] = Value::String(canonical_json_hash(&inventory));
    let transforms = by_part
        .iter()
        .map(|(part_id, (_, _, world))| (part_id.clone(), *world))
        .collect();
    Ok((inventory, transforms))
}

fn include_positions(
    bound: &mut Bounds,
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
) -> Result<(), RuntimeError> {
    let accessor = accessors
        .get(accessor_index)
        .and_then(Value::as_object)
        .ok_or_else(|| error("POSITION accessor is invalid"))?;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str) != Some("VEC3")
        || accessor.get("sparse").is_some()
        || accessor.get("normalized").and_then(Value::as_bool) == Some(true)
    {
        return invalid("POSITION accessor layout is unsupported");
    }
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| (3..=750_000).contains(count))
        .ok_or_else(|| error("POSITION accessor count is invalid"))? as usize;
    let view = views
        .get(
            accessor
                .get("bufferView")
                .and_then(Value::as_u64)
                .ok_or_else(|| error("POSITION bufferView is missing"))? as usize,
        )
        .and_then(Value::as_object)
        .ok_or_else(|| error("POSITION bufferView is invalid"))?;
    if view.get("buffer").and_then(Value::as_u64) != Some(0) {
        return invalid("POSITION must use the embedded GLB buffer");
    }
    let stride = view.get("byteStride").and_then(Value::as_u64).unwrap_or(12) as usize;
    if stride < 12 || stride % 4 != 0 || stride > 252 {
        return invalid("POSITION byte stride is invalid");
    }
    let start = (view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize)
        .checked_add(
            accessor
                .get("byteOffset")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        )
        .ok_or_else(|| error("POSITION byte offset overflowed"))?;
    let view_end = (view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize)
        .checked_add(
            view.get("byteLength")
                .and_then(Value::as_u64)
                .ok_or_else(|| error("POSITION bufferView length is missing"))?
                as usize,
        )
        .ok_or_else(|| error("POSITION bufferView overflowed"))?;
    let final_end = start
        .checked_add((count - 1) * stride)
        .and_then(|value| value.checked_add(12))
        .filter(|end| *end <= view_end && *end <= binary.len())
        .ok_or_else(|| error("POSITION data exceeds BIN or its bufferView"))?;
    let _ = final_end;
    for index in 0..count {
        let offset = start + index * stride;
        let mut point = [0.0; 3];
        for axis in 0..3 {
            point[axis] = f32::from_le_bytes(
                binary[offset + axis * 4..offset + axis * 4 + 4]
                    .try_into()
                    .expect("bounded POSITION component"),
            ) as f64;
        }
        bound.include(point)?;
    }
    Ok(())
}

fn validate_animation(
    runtime: &Runtime,
    lod0: &Level,
    value: Option<&Value>,
) -> Result<Option<Value>, RuntimeError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let object = exact_object(
        value,
        &[
            "clip_id",
            "animated_artifact_sha256",
            "receipt_object_sha256",
        ],
        "animation declaration",
    )?;
    let clip_id = identifier(object, "clip_id")?;
    let animated_sha = sha(object, "animated_artifact_sha256")?;
    let receipt_sha = sha(object, "receipt_object_sha256")?;
    let receipt_record = runtime
        .store
        .get_object(receipt_sha)?
        .ok_or_else(|| error("animation receipt object is unavailable"))?;
    if receipt_record.mime != "application/json"
        || receipt_record.kind != "mechanical-animation-glb-receipt"
    {
        return invalid("animation receipt object kind or MIME differs");
    }
    let receipt_bytes = runtime.cas_read(receipt_sha)?;
    if sha256_hex(&receipt_bytes) != receipt_sha {
        return invalid("animation receipt CAS bytes differ");
    }
    let receipt: Value = serde_json::from_slice(&receipt_bytes)
        .map_err(|_| error("animation receipt JSON is invalid"))?;
    verify_value_canonical(&receipt)?;
    if receipt.get("schema_version").and_then(Value::as_str)
        != Some("MechanicalAnimationGlbReceipt@1")
        || receipt.get("project_id").and_then(Value::as_str)
            != Some(lod0_project_id(runtime, lod0)?.as_str())
        || receipt.get("candidate_id").and_then(Value::as_str) != Some(lod0.candidate_id.as_str())
        || receipt
            .get("candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(lod0.candidate_state_sha256.as_str())
        || receipt
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(lod0.artifact_sha256.as_str())
        || receipt
            .get("artifact_readback_sha256")
            .and_then(Value::as_str)
            != Some(lod0.artifact_readback_sha256.as_str())
        || receipt.get("clip_id").and_then(Value::as_str) != Some(clip_id)
        || receipt
            .get("animated_artifact_sha256")
            .and_then(Value::as_str)
            != Some(animated_sha)
        || receipt.get("validator_status").and_then(Value::as_str)
            != Some("strict-rigid-gltf-animation-readback-pass")
        || receipt.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
    {
        return invalid("animation receipt does not bind exact LOD0 state");
    }
    let animated_record = runtime
        .store
        .get_object(animated_sha)?
        .ok_or_else(|| error("animated GLB object is unavailable"))?;
    if animated_record.mime != "model/gltf-binary"
        || animated_record.kind != "mechanical-animation-glb"
        || sha256_hex(&runtime.cas_read(animated_sha)?) != animated_sha
    {
        return invalid("animated GLB object kind, MIME or bytes differ");
    }
    Ok(Some(json!({
        "clip_id":clip_id,
        "animated_artifact_sha256":animated_sha,
        "receipt_object_sha256":receipt_sha
    })))
}

fn lod0_project_id(runtime: &Runtime, lod0: &Level) -> Result<String, RuntimeError> {
    runtime
        .candidate(&lod0.candidate_id)?
        .map(|candidate| candidate.project_id)
        .ok_or_else(|| error("LOD0 candidate became unavailable"))
}

fn string_set(value: &Value, field: &str) -> Result<Vec<String>, RuntimeError> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= MAX_PARTS)
        .ok_or_else(|| error(format!("{field} is invalid")))?;
    let mut result = values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| valid_identifier(value))
                .map(str::to_owned)
                .ok_or_else(|| error(format!("{field} contains an invalid identifier")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    result.sort();
    if result.windows(2).any(|pair| pair[0] == pair[1]) {
        return invalid(format!("{field} must be unique"));
    }
    Ok(result)
}

fn strict_solid_part_material_bindings(
    readback: &Value,
    expected_parts: &[String],
) -> Result<BTreeMap<String, String>, RuntimeError> {
    let bindings = readback
        .get("part_bindings")
        .and_then(Value::as_array)
        .filter(|bindings| bindings.len() == expected_parts.len())
        .ok_or_else(|| error("ArtifactReadback Part bindings are incomplete"))?;
    let mut result = BTreeMap::new();
    for binding in bindings {
        let object = binding
            .as_object()
            .ok_or_else(|| error("ArtifactReadback Part binding is invalid"))?;
        let part_id = object
            .get("part_id")
            .and_then(Value::as_str)
            .filter(|value| valid_identifier(value))
            .ok_or_else(|| error("ArtifactReadback Part ID is invalid"))?;
        let material_zone_id = object
            .get("material_zone_id")
            .and_then(Value::as_str)
            .filter(|value| valid_identifier(value))
            .ok_or_else(|| error("ArtifactReadback MaterialZone ID is invalid"))?;
        if object.get("solid").and_then(Value::as_bool) != Some(true) {
            return invalid("P0 collision delivery requires every semantic Part to be solid");
        }
        if result
            .insert(part_id.to_owned(), material_zone_id.to_owned())
            .is_some()
        {
            return invalid("ArtifactReadback duplicates a Part binding");
        }
    }
    if result.keys().cloned().collect::<Vec<_>>() != expected_parts {
        return invalid("ArtifactReadback Part binding coverage differs");
    }
    Ok(result)
}

fn parse_glb(bytes: &[u8]) -> Result<(Value, Vec<u8>), RuntimeError> {
    if bytes.len() < 28
        || &bytes[..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2
        || u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize != bytes.len()
    {
        return invalid("GLB header is invalid");
    }
    let json_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| error("GLB JSON length overflowed"))?;
    if json_end + 8 > bytes.len()
        || &bytes[16..20] != b"JSON"
        || &bytes[json_end + 4..json_end + 8] != b"BIN\0"
    {
        return invalid("GLB chunks are invalid");
    }
    let binary_length =
        u32::from_le_bytes(bytes[json_end..json_end + 4].try_into().unwrap()) as usize;
    let binary_start = json_end + 8;
    let binary_end = binary_start
        .checked_add(binary_length)
        .filter(|end| *end == bytes.len())
        .ok_or_else(|| error("GLB BIN length differs"))?;
    let root =
        serde_json::from_slice(&bytes[20..json_end]).map_err(|_| error("GLB JSON is invalid"))?;
    Ok((root, bytes[binary_start..binary_end].to_vec()))
}

fn encode_glb(root: &Value, binary: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let mut json_bytes = serde_json::to_vec(root).map_err(|source| error(source.to_string()))?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12usize
        .checked_add(8)
        .and_then(|value| value.checked_add(json_bytes.len()))
        .and_then(|value| value.checked_add(8))
        .and_then(|value| value.checked_add(binary.len()))
        .ok_or_else(|| error("GLB socket output length overflowed"))?;
    if total > MAX_GLB_BYTES as usize || total > u32::MAX as usize {
        return invalid("GLB socket output exceeds its size budget");
    }
    let mut result = Vec::with_capacity(total);
    result.extend_from_slice(b"glTF");
    result.extend_from_slice(&2u32.to_le_bytes());
    result.extend_from_slice(&(total as u32).to_le_bytes());
    result.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    result.extend_from_slice(b"JSON");
    result.extend_from_slice(&json_bytes);
    result.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    result.extend_from_slice(b"BIN\0");
    result.extend_from_slice(binary);
    Ok(result)
}

fn array_field<'a>(root: &'a Value, field: &str) -> Result<&'a [Value], RuntimeError> {
    root.get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| error(format!("GLB {field} is missing")))
}

fn put_json(runtime: &Runtime, value: &Value, kind: &str) -> Result<CasObject, RuntimeError> {
    let bytes = canonical_json_bytes(value).map_err(|source| error(source.to_string()))?;
    if bytes.len() > 1024 * 1024 {
        return invalid("delivery sidecar exceeds one MiB");
    }
    runtime.put_object(&bytes, None, "application/json", kind)
}

/// Normalize serde_json number spellings through the exact bytes persisted in
/// CAS before sealing a durable sidecar. This keeps the declared canonical
/// digest stable when generated geometry contains values such as negative
/// zero that may otherwise change representation after a JSON round trip.
fn seal_sidecar(mut value: Value) -> Result<Value, RuntimeError> {
    value["canonical_sha256"] = Value::String(String::new());
    let bytes = canonical_json_bytes(&value).map_err(|source| error(source.to_string()))?;
    let mut normalized: Value =
        serde_json::from_slice(&bytes).map_err(|_| error("sidecar normalization failed"))?;
    normalized["canonical_sha256"] = Value::String(canonical_json_hash(&normalized));
    Ok(normalized)
}

fn seal_request(mut value: Value) -> Result<Value, RuntimeError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| error("request must be an object"))?;
    object.remove("canonical_sha256");
    let canonical_sha256 = canonical_json_hash(&value);
    value["canonical_sha256"] = Value::String(canonical_sha256);
    Ok(value)
}

fn read_json(runtime: &Runtime, sha256: &str, schema_version: &str) -> Result<Value, RuntimeError> {
    let bytes = runtime.cas_read(sha256)?;
    if bytes.is_empty() || bytes.len() > 1024 * 1024 || sha256_hex(&bytes) != sha256 {
        return invalid("durable game delivery CAS object is invalid");
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| error("durable game delivery CAS JSON is invalid"))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(schema_version) {
        return invalid("durable game delivery CAS schema differs");
    }
    verify_value_canonical(&value).map_err(|_| {
        RuntimeError::InvalidInput(format!(
            "{ERROR}: {schema_version} sidecar canonical_sha256 differs"
        ))
    })?;
    Ok(value)
}

fn verify_request_canonical(value: &Value) -> Result<(), RuntimeError> {
    let declared = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("canonical_sha256 is invalid"))?;
    let mut preimage = value.clone();
    preimage
        .as_object_mut()
        .ok_or_else(|| error("request must be an object"))?
        .remove("canonical_sha256");
    if canonical_json_hash(&preimage) != declared {
        return invalid("canonical_sha256 differs");
    }
    Ok(())
}

fn verify_value_canonical(value: &Value) -> Result<(), RuntimeError> {
    let declared = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| forgecad_contracts::is_sha256(value))
        .ok_or_else(|| error("sidecar canonical_sha256 is invalid"))?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != declared {
        return invalid("sidecar canonical_sha256 differs");
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| error(format!("{context} must be an object")))?;
    if object.len() != fields.len()
        || fields.iter().any(|field| !object.contains_key(*field))
        || object.keys().any(|field| !fields.contains(&field.as_str()))
    {
        return invalid(format!("{context} field set differs"));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error(format!("{field} is invalid")))
}

fn identifier<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    text(object, field).and_then(|value| {
        if valid_identifier(value) {
            Ok(value)
        } else {
            Err(error(format!("{field} is invalid")))
        }
    })
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    text(object, field).and_then(|value| {
        if forgecad_contracts::is_sha256(value) {
            Ok(value)
        } else {
            Err(error(format!("{field} is invalid")))
        }
    })
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        && value.as_bytes()[0].is_ascii_alphanumeric()
}

fn invalid<T>(detail: impl Into<String>) -> Result<T, RuntimeError> {
    Err(error(detail))
}

fn error(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("{ERROR}: {}", detail.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn particle_semantics_reject_each_untrusted_policy_field() {
        let receipt = json!({
            "seed_policy":PARTICLE_SEED_POLICY,
            "simulation_quantization":fixed_particle_simulation_quantization(),
            "particle_policy":PARTICLE_POLICY,
            "emitter_policy":PARTICLE_EMITTER_POLICY,
            "emitter_counts":{"muzzle-burst":24,"energy-core-sparks":32}
        });
        let render_set = json!({
            "particle_policy":PARTICLE_POLICY,
            "emitter_policy":PARTICLE_EMITTER_POLICY
        });
        assert!(validate_particle_semantics(&receipt, &render_set).is_ok());

        for (field, value) in [
            ("seed_policy", json!("caller-rng@1")),
            ("particle_policy", json!("open-particle-policy@1")),
            ("emitter_policy", json!("open-emitter-policy@1")),
            (
                "emitter_counts",
                json!({"muzzle-burst":23,"energy-core-sparks":32}),
            ),
            (
                "simulation_quantization",
                json!({"hash_stream":"float-rng@1"}),
            ),
        ] {
            let mut bad = receipt.clone();
            bad[field] = value;
            assert!(validate_particle_semantics(&bad, &render_set).is_err());
        }

        let bad_render_set = json!({
            "particle_policy":"open-particle-policy@1",
            "emitter_policy":PARTICLE_EMITTER_POLICY
        });
        assert!(validate_particle_semantics(&receipt, &bad_render_set).is_err());
    }

    #[test]
    fn trail_semantics_rejects_depth_and_truth_boundary_mutations() {
        let receipt = json!({
            "opaque_geometry_depth_tested":true,
            "runtime_write_performed":true,
            "candidate_confirmed":false,
            "export_performed":false,
            "actual_engine_roundtrip":false,
            "quality_status":"structural_only",
            "limitations":fixed_trails_limitations(),
            "trail_id_encoding":fixed_trail_id_encoding()
        });
        assert!(validate_trails_receipt_semantics(&receipt).is_ok());

        for (field, value) in [
            ("opaque_geometry_depth_tested", json!(false)),
            ("runtime_write_performed", json!(false)),
            ("candidate_confirmed", json!(true)),
            ("export_performed", json!(true)),
            ("actual_engine_roundtrip", json!(true)),
            ("quality_status", json!("quality_pass")),
            ("limitations", json!(["no-trail-bloom-input"])),
            ("trail_id_encoding", json!({"encoding":"caller-defined"})),
        ] {
            let mut bad = receipt.clone();
            bad[field] = value;
            assert!(validate_trails_receipt_semantics(&bad).is_err());
        }
    }

    #[test]
    fn particle_render_binding_rejects_each_current_base_mismatch() {
        let receipt = json!({
            "camera_object_sha256":"camera",
            "camera_identity_sha256":"identity",
            "render_profile_sha256":"profile",
            "render_worker_build_cohort_sha256":"cohort"
        });
        assert!(particle_receipt_matches_render_binding(
            &receipt, "camera", "identity", "profile", "cohort"
        ));
        for (field, value) in [
            ("camera_object_sha256", "other-camera"),
            ("camera_identity_sha256", "other-identity"),
            ("render_profile_sha256", "other-profile"),
            ("render_worker_build_cohort_sha256", "other-cohort"),
        ] {
            let mut bad = receipt.clone();
            bad[field] = Value::String(value.to_owned());
            assert!(!particle_receipt_matches_render_binding(
                &bad, "camera", "identity", "profile", "cohort"
            ));
        }
    }

    #[test]
    fn trail_bloom_profile_rejects_each_untrusted_value() {
        let profile = fixed_trails_bloom_profile_value();
        assert!(parse_fixed_trails_bloom_profile(&profile).is_ok());
        for (field, value) in [
            ("threshold", json!(0.5)),
            ("source_gain", json!(4.0)),
            ("radius_px", json!(4)),
            ("intensity", json!(2.0)),
            ("hdr_clamp", json!(8.0)),
            ("blur_passes", json!(1)),
            ("kernel", json!("caller-defined")),
        ] {
            let mut bad = profile.clone();
            bad[field] = value;
            assert!(parse_fixed_trails_bloom_profile(&bad).is_err());
        }
    }

    #[test]
    fn trail_bloom_semantics_rejects_truth_and_policy_mutations() {
        let profile = fixed_trails_bloom_profile_value();
        let profile_hash = canonical_json_hash(&profile);
        let mut receipt = Map::new();
        for (field, value) in [
            ("trail_bloom_profile", profile.clone()),
            ("trail_bloom_profile_sha256", json!(profile_hash)),
            (
                "render_worker_binding_status",
                json!("same_cohort_verified"),
            ),
            ("base_opaque_depth_pass", json!("depth")),
            ("trail_bloom_policy", json!(TRAILS_BLOOM_POLICY)),
            ("input_policy", json!(TRAILS_BLOOM_INPUT_POLICY)),
            ("occlusion_policy", json!(TRAILS_BLOOM_OCCLUSION_POLICY)),
            ("render_policy", json!(TRAILS_BLOOM_RENDER_POLICY)),
            ("base_aov_byte_exact_verified", json!(true)),
            ("base_opaque_depth_byte_exact_reused", json!(true)),
            ("bloom_pass_byte_exact_reused", json!(true)),
            ("particle_passes_byte_exact_reused", json!(true)),
            ("source_trail_passes_byte_exact_reused", json!(true)),
            ("base_bloom_mutated", json!(false)),
            ("particle_passes_mutated", json!(false)),
            ("trail_passes_mutated", json!(false)),
            ("opaque_geometry_depth_tested", json!(true)),
            ("trail_bloom_source_rendered", json!(true)),
            ("trail_bloom_contribution_rendered", json!(true)),
            ("trail_bloom_rendered", json!(true)),
            ("trail_bloom_input", json!(true)),
            ("runtime_write_performed", json!(true)),
            ("candidate_confirmed", json!(false)),
            ("export_performed", json!(false)),
            ("actual_engine_roundtrip", json!(false)),
            ("quality_status", json!("structural_only")),
            ("limitations", fixed_trails_bloom_limitations()),
        ] {
            receipt.insert(field.to_owned(), value);
        }
        let mut render_set = Map::new();
        for (field, value) in [
            ("trail_bloom_profile", profile),
            ("trail_bloom_profile_sha256", json!(profile_hash)),
            (
                "render_worker_binding_status",
                json!("same_cohort_verified"),
            ),
            ("base_opaque_depth_pass", json!("depth")),
            ("trail_bloom_policy", json!(TRAILS_BLOOM_POLICY)),
            ("input_policy", json!(TRAILS_BLOOM_INPUT_POLICY)),
            ("occlusion_policy", json!(TRAILS_BLOOM_OCCLUSION_POLICY)),
            ("render_policy", json!(TRAILS_BLOOM_RENDER_POLICY)),
            ("base_aov_byte_exact_verified", json!(true)),
            ("base_opaque_depth_byte_exact_reused", json!(true)),
            ("bloom_pass_byte_exact_reused", json!(true)),
            ("particle_passes_byte_exact_reused", json!(true)),
            ("source_trail_passes_byte_exact_reused", json!(true)),
            ("base_bloom_mutated", json!(false)),
            ("particle_passes_mutated", json!(false)),
            ("trail_passes_mutated", json!(false)),
            ("trail_bloom_source_rendered", json!(true)),
            ("trail_bloom_contribution_rendered", json!(true)),
            ("trail_bloom_rendered", json!(true)),
            ("trail_bloom_input", json!(true)),
            (
                "passes",
                json!(["trail-emissive-source", "trail-bloom-contribution"]),
            ),
        ] {
            render_set.insert(field.to_owned(), value);
        }
        let receipt = Value::Object(receipt);
        let render_set = Value::Object(render_set);
        assert!(validate_trails_bloom_semantics(&receipt, &render_set).is_ok());
        for (field, value) in [
            ("trail_bloom_policy", json!("caller-policy@1")),
            ("base_opaque_depth_pass", json!("beauty")),
            ("base_bloom_mutated", json!(true)),
            ("trail_bloom_rendered", json!(false)),
            ("trail_bloom_input", json!(false)),
            ("runtime_write_performed", json!(false)),
            ("quality_status", json!("quality_pass")),
            ("limitations", json!([])),
        ] {
            let mut bad = receipt.clone();
            bad[field] = value;
            assert!(validate_trails_bloom_semantics(&bad, &render_set).is_err());
        }
        let mut bad_render_set = render_set.clone();
        bad_render_set["passes"] = json!(["trail-color"]);
        assert!(validate_trails_bloom_semantics(&receipt, &bad_render_set).is_err());
    }

    fn socket_test_anchor_set() -> Value {
        json!({
            "schema_version":"GameWeaponAnchorSet@1",
            "node_materialization":"sidecar-only-not-glb-nodes",
            "canonical_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "anchors":[
                {"anchor_id":"weapon-root","role":"weapon-root","parent_kind":"synthetic-scene-root","owner_part_id":null,"local_translation_m":[0.0,0.0,0.0],"local_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"local_scale_xyz":[1.0,1.0,1.0]},
                {"anchor_id":"grip-primary","role":"grip-primary","parent_kind":"part-node","owner_part_id":"part-a","local_translation_m":[0.1,0.0,0.0],"local_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"local_scale_xyz":[1.0,1.0,1.0]},
                {"anchor_id":"socket-muzzle-vfx","role":"muzzle-vfx","parent_kind":"part-node","owner_part_id":"part-b","local_translation_m":[0.2,0.0,0.0],"local_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"local_scale_xyz":[1.0,1.0,1.0]},
                {"anchor_id":"socket-magazine-well","role":"magazine-well","parent_kind":"part-node","owner_part_id":"part-c","local_translation_m":[0.3,0.0,0.0],"local_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"local_scale_xyz":[1.0,1.0,1.0]},
                {"anchor_id":"socket-sight-primary","role":"sight-primary","parent_kind":"part-node","owner_part_id":"part-d","local_translation_m":[0.4,0.0,0.0],"local_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"local_scale_xyz":[1.0,1.0,1.0]},
                {"anchor_id":"socket-energy-core-vfx","role":"energy-core-vfx","parent_kind":"part-node","owner_part_id":"part-e","local_translation_m":[0.5,0.0,0.0],"local_rotation_quat_xyzw":[0.0,0.0,0.0,1.0],"local_scale_xyz":[1.0,1.0,1.0]}
            ]
        })
    }

    #[test]
    fn socket_anchor_set_is_closed_and_role_ordered() {
        let anchor_set = socket_test_anchor_set();
        assert_eq!(
            socket_anchor_ids(&anchor_set).unwrap(),
            vec![
                "weapon-root",
                "grip-primary",
                "socket-muzzle-vfx",
                "socket-magazine-well",
                "socket-sight-primary",
                "socket-energy-core-vfx"
            ]
        );

        let mut bad = anchor_set.clone();
        bad["anchors"][1]["role"] = json!("caller-defined");
        assert!(socket_anchor_ids(&bad).is_err());
        bad = anchor_set;
        bad["anchors"][0]["local_scale_xyz"] = json!([1.0, 2.0, 1.0]);
        assert!(socket_anchor_transform(bad["anchors"][0].as_object().unwrap()).is_err());
    }

    #[test]
    fn socket_materializer_replays_with_bin_and_renderable_arrays_exact() {
        let part_ids = vec![
            "part-a".to_owned(),
            "part-b".to_owned(),
            "part-c".to_owned(),
            "part-d".to_owned(),
            "part-e".to_owned(),
        ];
        let nodes = part_ids
            .iter()
            .enumerate()
            .map(|(index, part_id)| {
                if index == 0 {
                    json!({"name":part_id,"mesh":0,"children":[1,2,3,4]})
                } else {
                    json!({"name":part_id,"mesh":0})
                }
            })
            .collect::<Vec<_>>();
        let source_root = json!({
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":nodes,
            "meshes":[{"primitives":[]}],
            "materials":[],
            "buffers":[{"byteLength":4}],
            "bufferViews":[],
            "accessors":[],
            "animations":[],
            "extras":{"forgecad":{}}
        });
        let source_binary = vec![0_u8, 1, 2, 3];
        let source_glb = encode_glb(&source_root, &source_binary).unwrap();
        let anchor_set = socket_test_anchor_set();
        let anchor_ids = socket_anchor_ids(&anchor_set).unwrap();
        let source_hash = sha256_hex(&source_glb);
        let first = materialize_socket_glb(
            &source_glb,
            &source_hash,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            &anchor_set,
            &part_ids,
            &anchor_ids,
        )
        .unwrap();
        let replay = materialize_socket_glb(
            &source_glb,
            &source_hash,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            &anchor_set,
            &part_ids,
            &anchor_ids,
        )
        .unwrap();
        assert_eq!(first.glb, replay.glb);
        assert_eq!(first.source_bin_sha256, sha256_hex(&source_binary));
        assert_eq!(first.derived_bin_sha256, sha256_hex(&source_binary));
        assert_eq!(first.source_node_count + 6, first.derived_node_count);
        assert_eq!(first.socket_nodes.len(), 6);

        let (derived_root, derived_binary) = parse_glb(&first.glb).unwrap();
        assert_eq!(derived_binary, source_binary);
        assert_eq!(derived_root["meshes"], source_root["meshes"]);
        assert_eq!(derived_root["materials"], source_root["materials"]);
        assert_eq!(derived_root["buffers"], source_root["buffers"]);
        assert_eq!(derived_root["animations"], source_root["animations"]);
        assert_eq!(
            derived_root["nodes"].as_array().unwrap().len(),
            part_ids.len() + 6
        );
        assert_eq!(
            derived_root["nodes"][part_ids.len()]["extras"]["forgecad"]["anchor_set_object_sha256"],
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
        assert_eq!(
            derived_root["nodes"][part_ids.len()]["extras"]["forgecad"]
                ["anchor_set_canonical_sha256"],
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert!(derived_root["scenes"][0]["nodes"]
            .as_array()
            .unwrap()
            .contains(&json!(part_ids.len())));
    }

    #[test]
    fn socket_source_graph_rejects_cycles_and_future_animation_targets() {
        let cyclic = json!({
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"name":"part-a","mesh":0,"children":[0]}],
            "meshes":[{}],"materials":[],"buffers":[{"byteLength":0}],"bufferViews":[],"accessors":[]
        });
        assert!(validate_socket_source_graph(&cyclic, &["part-a".to_owned()]).is_err());

        let future_target = json!({
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":[{"name":"part-a","mesh":0}],
            "meshes":[{}],"materials":[],"buffers":[{"byteLength":0}],"bufferViews":[],"accessors":[],
            "animations":[{"channels":[{"target":{"node":1}}]}]
        });
        assert!(validate_socket_source_graph(&future_target, &["part-a".to_owned()]).is_err());
    }
}
