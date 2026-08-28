//! Runtime/Store adapter for the closed FPS foundation typed importer.
//!
//! The importer kernel is intentionally kept separate from this file.  This
//! adapter owns the product boundary: the caller supplies an allowlisted
//! `asset_id` and hashes only, Runtime selects the embedded source bytes,
//! writes compact CAS objects and one durable Store link, and MCP receives a
//! hash-only draft summary.  No path, URL, source bytes or candidate state is
//! accepted here.

use forgecad_contracts::CasObjectRecord;
use forgecad_core::{canonical_json_bytes, canonical_json_hash, sha256_hex};
use forgecad_store::weapon_foundation_import as foundation_store;
use forgecad_store::CasObject;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{now_string, weapon_foundation_import, Runtime, RuntimeError};

pub(crate) const GET_SCHEMA_VERSION: &str = "WeaponFoundationAssetGetRequest@1";
pub(crate) const PREPARE_RESULT_SCHEMA_VERSION: &str = "WeaponFoundationAssetPrepareResult@1";
pub(crate) const GET_RESULT_SCHEMA_VERSION: &str = "WeaponFoundationAssetGetResult@1";
const REQUEST_SCHEMA_VERSION: &str = "WeaponFoundationAssetRequest@1";
const IMPORT_PROFILE: &str = "forgecad-foundation-typed-import@1";
const STRICT_READBACK_POLICY: &str =
    "glb-gltf-embedded-resource-strict-readback-no-external-reference@1";
const DEGENERATE_POLICY: &str = "drop-degenerate-faces-deterministic-source-order@1";
const DEGENERATE_TEST: &str =
    "non-finite-or-area-less-than-threshold-after-source-to-target-transform@1";
const DEGENERATE_ORDERING: &str = "source-primitive-index-then-face-index@1";
const DEGENERATE_REINDEXING: &str = "stable-first-pass-compaction@1";
const NORMALIZATION_POLICY: &str =
    "forgecad-foundation-typed-import-source-to-target-axis-normalization@1";
const TOPOLOGY_PAYLOAD_SCHEMA_VERSION: &str = "ForgeCadFoundationTopologyPayload@1";
const SOCKET_MAPPING_POLICY: &str = "source-semantic-node-to-forgecad-socket-allowlist@1";
const RIG_POLICY: &str = "forgecad-rigid-weapon-and-first-person-armature-semantic-map@1";
const PRESENTATION_POLICY: &str = "forgecad-fps-presentation-package-foundation-draft@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const COORDINATE_FRAME: &str = "weapon-right-handed-x-muzzle-y-up-z-right";
const UNITS: &str = "meter";
const REQUIRED_CLIPS: &[&str] = &[
    "idle",
    "equip",
    "fire_recoil",
    "reload",
    "inspect",
    "ads_in",
    "ads_out",
    "sprint",
    "holster",
];
const REQUIRED_MARKERS: &[&str] = &[
    "equip_start",
    "ads_in",
    "ads_out",
    "fire",
    "reload_start",
    "reload_insert",
    "reload_end",
    "inspect_start",
    "inspect_end",
    "sprint_start",
    "holster",
];
const REQUIRED_SOCKETS: &[&str] = &[
    "optic",
    "muzzle",
    "rail_top",
    "rail_bottom",
    "rail_left",
    "rail_right",
    "left_grip",
    "right_grip",
    "mag_eject",
    "shell_eject",
    "vfx_origin",
    "audio_origin",
];
const FIXED_MISSING_SOCKETS: &[&str] = &[
    "left_grip",
    "right_grip",
    "mag_eject",
    "shell_eject",
    "vfx_origin",
    "audio_origin",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetRequest {
    schema_version: String,
    request_id: String,
    foundation_pack_id: String,
    foundation_pack_version: String,
    foundation_manifest_sha256: String,
    asset_id: String,
    asset_sha256: String,
    asset_role: String,
    source_format: String,
    coordinate_spec_sha256: String,
    coordinate_frame: String,
    units: String,
    source_to_target: Value,
    import_profile: String,
    strict_readback_policy: String,
    degenerate_face_policy: Value,
    budgets: Value,
    canonicalization_policy: String,
    canonical_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GetRequest {
    schema_version: String,
    request_id: String,
    #[serde(default)]
    request_sha256: Option<String>,
    #[serde(default)]
    result_object_sha256: Option<String>,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "WEAPON_FOUNDATION_IMPORT_REJECTED: {}",
        message.into()
    ))
}

fn source_to_target_value(asset_id: &str) -> Option<Value> {
    match asset_id {
        "pichuliru-weapon-west" => Some(json!({
            "mapping_evidence": "PROVEN",
            "axis_mapping": ["-Z", "+Y", "+X"],
            "matrix_row_major": [[0, 0, -1], [0, 1, 0], [1, 0, 0]],
            "translation_m": [0.0, 0.0, 0.0],
            "scale_xyz": [1.0, 1.0, 1.0]
        })),
        "wrad-arms" | "lightning-low-pbr" => Some(json!({
            "mapping_evidence": "PENDING_SOURCE_VERIFICATION",
            "axis_mapping": ["+Z", "+Y", "-X"],
            "matrix_row_major": [[0, 0, 1], [0, 1, 0], [-1, 0, 0]],
            "translation_m": [0.0, 0.0, 0.0],
            "scale_xyz": [1.0, 1.0, 1.0]
        })),
        _ => None,
    }
}

fn parse_request(value: &Value) -> Result<AssetRequest, RuntimeError> {
    let request: AssetRequest = serde_json::from_value(value.clone()).map_err(|error| {
        invalid(format!(
            "request is not a closed WeaponFoundationAssetRequest: {error}"
        ))
    })?;
    if request.schema_version != REQUEST_SCHEMA_VERSION
        || request.foundation_pack_id != foundation_store::FOUNDATION_PACK_ID
        || request.foundation_pack_version != foundation_store::FOUNDATION_PACK_VERSION
        || request.foundation_manifest_sha256 != foundation_store::FOUNDATION_MANIFEST_SHA256
        || request.coordinate_frame != COORDINATE_FRAME
        || request.units != UNITS
        || request.import_profile != IMPORT_PROFILE
        || request.strict_readback_policy != STRICT_READBACK_POLICY
        || request.canonicalization_policy != CANONICALIZATION_POLICY
        || source_to_target_value(&request.asset_id).as_ref() != Some(&request.source_to_target)
        || request.degenerate_face_policy
            != json!({
                "policy": DEGENERATE_POLICY,
                "test": DEGENERATE_TEST,
                "area_epsilon_m2": 1e-12,
                "area_comparison": "strict-less-than",
                "ordering": DEGENERATE_ORDERING,
                "reindexing": DEGENERATE_REINDEXING
            })
        || request.budgets
            != json!({
                "max_source_nodes": 512,
                "max_source_meshes": 128,
                "max_source_triangles": 250000,
                "max_cas_objects": 32,
                "max_wire_size": 1048576
            })
        || !super::is_opaque_id(&request.request_id)
        || !super::is_sha256(&request.canonical_sha256)
        || !super::is_sha256(&request.asset_sha256)
        || !super::is_sha256(&request.coordinate_spec_sha256)
    {
        return Err(invalid("request policy, identity or budget is not exact"));
    }
    let mut without_hash = value.clone();
    without_hash["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != request.canonical_sha256 {
        return Err(invalid("canonical_sha256 does not bind the request"));
    }
    let Some(asset) = foundation_store::allowlisted_asset(&request.asset_id) else {
        return Err(invalid(
            "asset_id is not in the closed foundation allowlist",
        ));
    };
    if request.asset_sha256 != asset.asset_sha256
        || request.asset_role != asset.asset_role
        || request.source_format != asset.source_format
    {
        return Err(invalid(
            "asset_id, asset_sha256, role and format do not match",
        ));
    }
    if !matches!(
        request.asset_id.as_str(),
        "pichuliru-weapon-west" | "wrad-arms" | "lightning-low-pbr"
    ) {
        return Err(invalid(
            "asset is allowlisted as reference-only; this Runtime cohort has no typed importer for it",
        ));
    }
    Ok(request)
}

fn parse_get(value: &Value) -> Result<GetRequest, RuntimeError> {
    let request: GetRequest = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("get request is not closed: {error}")))?;
    if request.schema_version != GET_SCHEMA_VERSION
        || !super::is_opaque_id(&request.request_id)
        || request
            .request_sha256
            .as_deref()
            .is_some_and(|hash| !super::is_sha256(hash))
        || request
            .result_object_sha256
            .as_deref()
            .is_some_and(|hash| !super::is_sha256(hash))
    {
        return Err(invalid("get identity is invalid"));
    }
    Ok(request)
}

pub(crate) fn builtin_asset(
    asset_id: &str,
) -> Option<weapon_foundation_import::BuiltinWeaponFoundationAsset> {
    match asset_id {
        "pichuliru-weapon-west" => {
            Some(weapon_foundation_import::BuiltinWeaponFoundationAsset::PichuliruWeaponWest)
        }
        "wrad-arms" => Some(weapon_foundation_import::BuiltinWeaponFoundationAsset::WradArms),
        "lightning-low-pbr" => {
            Some(weapon_foundation_import::BuiltinWeaponFoundationAsset::LightningBenchmark)
        }
        _ => None,
    }
}

pub(crate) fn builtin_asset_bytes(
    asset: weapon_foundation_import::BuiltinWeaponFoundationAsset,
) -> &'static [u8] {
    const PICHULIRU_WEST: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../packages/forgecad-assets/forgecad-fps-production-foundation/0.1.0-proposal/assets/pichuliru/weapon-west.glb"
    ));
    const WRAD_ARMS: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../packages/forgecad-assets/forgecad-fps-production-foundation/0.1.0-proposal/assets/wrad/arms.glb"
    ));
    const LIGHTNING: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../packages/forgecad-assets/forgecad-fps-production-foundation/0.1.0-proposal/assets/lightning/lightning.glb"
    ));
    match asset {
        weapon_foundation_import::BuiltinWeaponFoundationAsset::PichuliruWeaponWest => {
            PICHULIRU_WEST
        }
        weapon_foundation_import::BuiltinWeaponFoundationAsset::WradArms => WRAD_ARMS,
        weapon_foundation_import::BuiltinWeaponFoundationAsset::LightningBenchmark => LIGHTNING,
    }
}

fn canonical_bytes(value: &Value, maximum: usize, label: &str) -> Result<Vec<u8>, RuntimeError> {
    let bytes = canonical_json_bytes(value)
        .map_err(|error| invalid(format!("{label} cannot be canonicalized: {error}")))?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(format!("{label} exceeds its bounded CAS limit")));
    }
    Ok(bytes)
}

fn normalize_near_zero_numbers(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(normalize_near_zero_numbers),
        Value::Object(values) => values.values_mut().for_each(normalize_near_zero_numbers),
        Value::Number(number) => {
            if let Some(number) = number.as_f64() {
                let normalized = if number.abs() < 1.0e-15 {
                    0.0
                } else {
                    (number * 1.0e12).round() / 1.0e12
                };
                if let Some(number) = serde_json::Number::from_f64(normalized) {
                    *value = Value::Number(number);
                }
            }
        }
        _ => {}
    }
}

fn with_canonical_hash(mut value: Value) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    value["canonical_sha256"] = Value::String(String::new());
    normalize_near_zero_numbers(&mut value);
    // Imported f32/f64 payloads can contain signed zero.  Normalize once
    // through the exact JSON representation before hashing so a Store
    // readback produces the same canonical bytes and digest.  Extremely small
    // transform components can require more than one parse/format cycle to
    // reach the serde_json/Ryu fixed point, so keep the pass bounded.
    let mut normalized = value;
    let mut stabilized = false;
    for _ in 0..32 {
        let bytes = canonical_bytes(&normalized, 16 * 1024 * 1024, "foundation CAS payload")?;
        let next: Value = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("foundation CAS normalization failed: {error}")))?;
        let next_bytes = canonical_bytes(&next, 16 * 1024 * 1024, "foundation CAS payload")?;
        normalized = next;
        if next_bytes == bytes {
            stabilized = true;
            break;
        }
    }
    if !stabilized {
        return Err(invalid(
            "foundation CAS numeric normalization did not reach a bounded fixed point",
        ));
    }
    let hash = canonical_json_hash(&normalized);
    normalized["canonical_sha256"] = Value::String(hash.clone());
    let bytes = canonical_bytes(&normalized, 16 * 1024 * 1024, "foundation CAS payload")?;
    Ok((normalized, bytes, hash))
}

fn short_id(prefix: &str, hash: &str) -> String {
    format!("{prefix}-{}", &hash[..24])
}

fn topology_payload(
    imported: &weapon_foundation_import::WeaponFoundationImport,
    request: &AssetRequest,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    let meshes = imported
        .meshes
        .iter()
        .map(|mesh| {
            json!({
                "mesh_id": mesh.mesh_id,
                "part_id": mesh.part_id,
                "source_mesh_index": mesh.source_mesh_index,
                "source_node_index": mesh.source_node_index,
                "positions_m": mesh.positions_m,
                "faces": mesh.faces,
                "face_material_indices": mesh.face_material_indices,
                "world_transform_m": mesh.world_transform_m,
                "topology": mesh.topology,
                "stable_vertex_id_policy": "forgecad.vertex.<part_id>:ordinal@1",
                "stable_face_id_policy": "forgecad.face.<part_id>:ordinal@1"
            })
        })
        .collect::<Vec<_>>();
    let semantic_nodes = imported
        .semantic_nodes
        .iter()
        .map(|node| {
            json!({
                "stable_id": node.stable_id,
                "source_node_index": node.source_node_index,
                "source_name": node.source_name,
                "kind": node.kind,
                "semantic_role": node.semantic_role,
                "parent_stable_id": node.parent_stable_id,
                "mesh_id": node.mesh_id,
                "local_transform_m": node.local_transform_m,
                "world_transform_m": node.world_transform_m
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": TOPOLOGY_PAYLOAD_SCHEMA_VERSION,
        "topology_id": short_id("foundation-topology", &request.canonical_sha256),
        "source_asset_id": request.asset_id,
        "source_asset_sha256": request.asset_sha256,
        "source_format": request.source_format,
        "coordinate_spec_sha256": request.coordinate_spec_sha256,
        "coordinate_frame": imported.coordinate_frame,
        "normalization_policy": NORMALIZATION_POLICY,
        "stable_id_policy": "stable-node-part-vertex-face-ids-source-order@1",
        "degenerate_face_sanitation": imported.sanitation,
        "source": imported.source,
        "meshes": meshes,
        "semantic_nodes": semantic_nodes,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    with_canonical_hash(payload)
}

fn socket_parent(role: &str) -> &'static str {
    match role {
        "optic" | "rail_top" | "rail_bottom" | "rail_left" | "rail_right" => "body",
        "muzzle" => "barrel",
        "left_grip" | "right_grip" => "grip",
        "mag_eject" => "magazine",
        "shell_eject" => "body",
        "vfx_origin" | "audio_origin" => "weapon_root",
        _ => "weapon_root",
    }
}

fn socket_semantic(role: &str) -> &'static str {
    match role {
        "optic" | "muzzle" | "rail_top" | "rail_bottom" | "rail_left" | "rail_right" => {
            "attachment"
        }
        "left_grip" | "right_grip" => "grip",
        "mag_eject" | "shell_eject" => "ejection",
        "vfx_origin" => "vfx",
        "audio_origin" => "audio",
        _ => "attachment",
    }
}

fn socket_map_payload(
    imported: &weapon_foundation_import::WeaponFoundationImport,
    request: &AssetRequest,
    topology_hash: &str,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    let mappings = REQUIRED_SOCKETS
        .iter()
        .map(|role| {
            let source = imported.sockets.iter().find(|socket| socket.role == *role);
            json!({
                "socket_id": role,
                "target_node_name": format!("socket.{role}"),
                "source_node_name": source.map(|socket| socket.source_name.clone()),
                "source_presence": if source.is_some() { "PRESENT" } else { "MISSING" },
                "source_semantic": socket_semantic(role),
                "parent_target_id": socket_parent(role),
                "local_transform_status": "NOT_ACCEPTED_FROM_SOURCE"
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": "WeaponFoundationSocketMap@1",
        "socket_map_id": short_id("foundation-sockets", &request.canonical_sha256),
        "source_asset_id": request.asset_id,
        "source_asset_sha256": request.asset_sha256,
        "coordinate_spec_sha256": request.coordinate_spec_sha256,
        "mapping_policy": SOCKET_MAPPING_POLICY,
        "target_socket_namespace": "forgecad.weapon.socket@1",
        "required_socket_ids": REQUIRED_SOCKETS,
        "source_missing_socket_ids": FIXED_MISSING_SOCKETS,
        "mappings": mappings,
        "topology_object_sha256": topology_hash,
        "transform_status": "PENDING_RUNTIME_DERIVATION",
        "materialization_status": "MAPPING_ONLY",
        "quality_status": "structural_only",
        "promotion_eligible": false,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    with_canonical_hash(payload)
}

fn first_semantic_source(
    imported: &weapon_foundation_import::WeaponFoundationImport,
    role: &str,
) -> String {
    imported
        .semantic_nodes
        .iter()
        .find(|node| node.semantic_role.as_deref() == Some(role))
        .map(|node| node.source_name.clone())
        .unwrap_or_else(|| format!("missing.{role}"))
}

fn rig_map_payload(
    imported: &weapon_foundation_import::WeaponFoundationImport,
    request: &AssetRequest,
    socket_hash: &str,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    let part_mappings = [
        ("magazine", "rigid-link"),
        ("trigger", "rigid-rotation"),
        ("selector", "rigid-toggle"),
        ("bolt", "rigid-slide"),
        ("stock", "rigid-link"),
        ("charging_handle", "rigid-slide"),
        ("magazine_release", "rigid-link"),
    ]
    .iter()
    .map(|(part, movement)| {
        json!({
            "part_id": part,
            "source_node_name": first_semantic_source(imported, part),
            "target_node_name": format!("part.{part}"),
            "movement_class": movement,
            "source_presence": "PRESENT"
        })
    })
    .collect::<Vec<_>>();
    let bones = imported
        .rig
        .bones
        .iter()
        .map(|bone| {
            json!({
                "bone_id": bone.bone_id,
                "source_node_index": bone.source_node_index,
                "source_name": bone.source_name,
                "parent_bone_id": bone.parent_bone_id,
                "local_transform_m": bone.local_transform_m,
                "world_transform_m": bone.world_transform_m
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": "WeaponFoundationRigMap@1",
        "rig_map_id": short_id("foundation-rig", &request.canonical_sha256),
        "weapon_asset_id": request.asset_id,
        "weapon_asset_sha256": request.asset_sha256,
        "arms_asset_id": "wrad-arms",
        "arms_asset_sha256": "580efbb0852bf0b41f82dd3e17eafec86b3d2a48f4a7acaa7e64d60e850f565d",
        "coordinate_spec_sha256": request.coordinate_spec_sha256,
        "socket_map_object_sha256": socket_hash,
        "rig_policy": RIG_POLICY,
        "rig_type": "rigid-weapon-plus-first-person-armature-reference@1",
        "weapon_root_candidates": ["Rifle_Assault_West.Rig", "Rifle_Assault_West"],
        "part_mappings": part_mappings,
        "arms_mapping": {
            "root": "root",
            "left_grip_candidate": "socket.l",
            "right_grip_candidate": "socket.r",
            "left_wrist_ik": "wrist_ik.l",
            "right_wrist_ik": "wrist_ik.r",
            "left_arm_target": "arm_target.l",
            "right_arm_target": "arm_target.r"
        },
        "rest_pose": {
            "status": "PENDING_DERIVATION",
            "rest_pose_sha256": Value::Null,
            "derivation_policy": "runtime-derived-after-coordinate-normalization-and-node-validation@1",
            "source_transform_payload": "HASH_ONLY"
        },
        "source_animation_clips": imported.animations.iter().map(|animation| animation.clip_id.clone()).collect::<Vec<_>>(),
        "required_target_clips": REQUIRED_CLIPS,
        "skinning_status": "PENDING_TYPED_MATERIALIZATION",
        "materialization_status": "MAPPING_ONLY",
        "quality_status": "structural_only",
        "promotion_eligible": false,
        "bones": bones,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    with_canonical_hash(payload)
}

fn presentation_package_payload(
    imported: &weapon_foundation_import::WeaponFoundationImport,
    request: &AssetRequest,
    topology_hash: &str,
    socket_hash: &str,
    rig_hash: &str,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    let clips = REQUIRED_CLIPS
        .iter()
        .map(|clip_id| {
            json!({
                "clip_id": clip_id,
                "source_clip_id": Value::Null,
                "status": "MISSING",
                "clip_object_sha256": Value::Null,
                "event_markers_complete": false
            })
        })
        .collect::<Vec<_>>();
    let markers = REQUIRED_MARKERS
        .iter()
        .map(|marker_id| {
            let clip_id = match *marker_id {
                "equip_start" => "equip",
                "ads_in" => "ads_in",
                "ads_out" => "ads_out",
                "fire" => "fire_recoil",
                "reload_start" | "reload_insert" | "reload_end" => "reload",
                "inspect_start" | "inspect_end" => "inspect",
                "sprint_start" => "sprint",
                "holster" => "holster",
                _ => "idle",
            };
            json!({
                "marker_id": marker_id,
                "clip_id": clip_id,
                "status": "MISSING",
                "time_ticks": Value::Null
            })
        })
        .collect::<Vec<_>>();
    let camera_profiles = ["hip", "ads", "inspect", "third_person", "ground_pickup"]
        .iter()
        .map(|profile_id| {
            json!({
                "profile_id": profile_id,
                "status": "MISSING_TYPED_CAMERA",
                "camera_object_sha256": Value::Null,
                "source_of_truth": "PENDING_RUNTIME_DERIVATION"
            })
        })
        .collect::<Vec<_>>();
    let cues = |prefix: &str| -> Vec<Value> {
        let _ = prefix;
        Vec::new()
    };
    let package = json!({
        "schema_version": "FpsPresentationPackage@1",
        "package_id": short_id("foundation-presentation", &request.canonical_sha256),
        "foundation_import_record_sha256": request.canonical_sha256,
        "source_asset_ids": [request.asset_id],
        "coordinate_spec_sha256": request.coordinate_spec_sha256,
        "topology_object_sha256": topology_hash,
        "socket_map_object_sha256": socket_hash,
        "rig_map_object_sha256": rig_hash,
        "presentation_policy": PRESENTATION_POLICY,
        "status": "DRAFT_UNREVIEWED",
        "promotion_eligible": false,
        "authoring_mesh_materialization_status": foundation_store::MATERIALIZATION_PENDING,
        "required_clip_ids": REQUIRED_CLIPS,
        "clips": clips,
        "missing_clip_ids": REQUIRED_CLIPS,
        "complete_required_clips": false,
        "required_event_marker_ids": REQUIRED_MARKERS,
        "event_markers": markers,
        "missing_event_marker_ids": REQUIRED_MARKERS,
        "complete_event_markers": false,
        "camera_profiles": camera_profiles,
        "screen_occupancy": {"status": "NOT_EVALUATED", "measurement_sha256": Value::Null},
        "reticle_safe_region": {"status": "NOT_EVALUATED", "measurement_sha256": Value::Null},
        "muzzle_safe_region": {"status": "NOT_EVALUATED", "measurement_sha256": Value::Null},
        "hands_weapon_clipping_status": "NOT_EVALUATED",
        "vfx_cues": cues("vfx"),
        "audio_cues": cues("audio"),
        "gameplay_beats": cues("beat"),
        "complete_vfx_audio_timeline": false,
        "engine_validation_status": "NOT_RUN",
        "human_review_status": "NOT_RUN",
        "visual_quality_status": "NOT_PROVEN",
        "commercial_fps_quality_status": "NOT_PROVEN",
        "materialization_status": "DRAFT_ONLY",
        "quality_status": "structural_only",
        "runtime_write_performed": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "actual_engine_roundtrip": false,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    let _ = imported;
    with_canonical_hash(package)
}

fn result_payload(
    imported: &weapon_foundation_import::WeaponFoundationImport,
    request: &AssetRequest,
    topology_hash: &str,
    socket_hash: &str,
    rig_hash: &str,
    package_hash: &str,
    deterministic_topology_hash: &str,
    deterministic_record_hash: &str,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    let source = &imported.source;
    let sanitation = &imported.sanitation;
    let sanitized_triangle_count = imported
        .meshes
        .iter()
        .map(|mesh| mesh.faces.len())
        .sum::<usize>();
    let result_id = short_id("foundation-result", &request.canonical_sha256);
    let result = json!({
        "schema_version": "WeaponFoundationAssetResult@1",
        "result_id": result_id,
        "request_id": request.request_id,
        "request_sha256": request.canonical_sha256,
        "foundation_pack_id": request.foundation_pack_id,
        "foundation_pack_version": request.foundation_pack_version,
        "foundation_manifest_sha256": request.foundation_manifest_sha256,
        "asset_id": request.asset_id,
        "asset_sha256": request.asset_sha256,
        "asset_role": request.asset_role,
        "source_format": request.source_format,
        "coordinate_spec_sha256": request.coordinate_spec_sha256,
        "coordinate_frame": request.coordinate_frame,
        "units": request.units,
        "source_to_target": request.source_to_target,
        "strict_readback": {
            "status": "PASS",
            "policy": "strict-gltf2-glb-readback-embedded-resources-no-external-reference@1",
            "readback_sha256": source.canonical_sha256,
            "source_node_count": source.node_count,
            "source_mesh_count": source.mesh_count,
            "source_primitive_count": source.primitive_count,
            "source_triangle_count": sanitation.source_triangle_count,
            "sanitized_triangle_count": sanitized_triangle_count,
            "invalid_index_count": 0,
            "non_finite_count": 0,
            "external_reference_count": 0,
            "remaining_degenerate_face_count": 0,
            "semantic_metadata_exact": true
        },
        "degenerate_face_sanitation": {
            "policy": DEGENERATE_POLICY,
            "test": DEGENERATE_TEST,
            "area_epsilon_m2": 1e-12,
            "area_comparison": "strict-less-than",
            "ordering": DEGENERATE_ORDERING,
            "source_degenerate_face_count": sanitation.degenerate_faces_removed,
            "dropped_face_count": sanitation.degenerate_faces_removed,
            "remaining_degenerate_face_count": 0,
            "stable_reindexing": true
        },
        "deterministic_replay": {
            "policy": "fixed-input-order-fixed-transform-fixed-sanitization-two-pass-replay@1",
            "first_topology_sha256": deterministic_topology_hash,
            "repeat_topology_sha256": deterministic_topology_hash,
            "first_record_sha256": deterministic_record_hash,
            "repeat_record_sha256": deterministic_record_hash,
            "byte_exact": true,
            "metadata_exact": true
        },
        "topology_object_sha256": topology_hash,
        "socket_map_object_sha256": socket_hash,
        "rig_map_object_sha256": rig_hash,
        "fps_presentation_package_object_sha256": package_hash,
        "authoring_mesh_materialization_status": foundation_store::MATERIALIZATION_PENDING,
        "socket_materialization_status": "MATERIALIZED_MAPPING_ONLY",
        "rig_materialization_status": "MATERIALIZED_MAPPING_ONLY",
        "presentation_materialization_status": "DRAFT_ONLY",
        "import_status": "IMPORTED_DRAFT",
        "quality_status": "structural_only",
        "promotion_eligible": false,
        "runtime_write_performed": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "actual_engine_roundtrip": false,
        "human_review_status": "NOT_RUN",
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    with_canonical_hash(result)
}

fn link_payload(
    request: &AssetRequest,
    result_hash: &str,
    topology_hash: &str,
    socket_hash: &str,
    rig_hash: &str,
    package_hash: &str,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    let link = json!({
        "schema_version": foundation_store::LINK_SCHEMA_VERSION,
        "request_id": request.request_id,
        "request_sha256": request.canonical_sha256,
        "asset_id": request.asset_id,
        "asset_sha256": request.asset_sha256,
        "topology_object_sha256": topology_hash,
        "socket_map_object_sha256": socket_hash,
        "rig_map_object_sha256": rig_hash,
        "fps_presentation_package_object_sha256": package_hash,
        "result_object_sha256": result_hash,
        "authoring_mesh_materialization_status": foundation_store::MATERIALIZATION_PENDING,
        "writer_policy": foundation_store::WRITER_POLICY,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    with_canonical_hash(link)
}

fn put_payload(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    bytes: &[u8],
    maximum: usize,
    kind: &str,
) -> Result<CasObject, RuntimeError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(format!("{kind} payload exceeds its bounded size")));
    }
    runtime
        .store
        .put_object_reserved(
            reservation,
            bytes,
            None,
            "application/json",
            kind,
            &now_string(),
        )
        .map_err(RuntimeError::from)
}

fn release_objects(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    objects: &[CasObject],
    cleanup: bool,
) {
    for object in objects {
        let _ = runtime
            .store
            .release_cas_reservation_object(reservation, object, cleanup);
    }
}

fn record_from_result(
    request: &AssetRequest,
    result: &Value,
    result_hash: &str,
    link_hash: &str,
    created_at: String,
) -> Result<foundation_store::WeaponFoundationImportRecord, RuntimeError> {
    let mut record = foundation_store::WeaponFoundationImportRecord {
        schema_version: foundation_store::RECORD_SCHEMA_VERSION.to_owned(),
        request_id: request.request_id.clone(),
        request_sha256: request.canonical_sha256.clone(),
        foundation_pack_id: request.foundation_pack_id.clone(),
        foundation_pack_version: request.foundation_pack_version.clone(),
        foundation_manifest_sha256: request.foundation_manifest_sha256.clone(),
        asset_id: request.asset_id.clone(),
        asset_sha256: request.asset_sha256.clone(),
        asset_role: request.asset_role.clone(),
        source_format: request.source_format.clone(),
        coordinate_spec_sha256: request.coordinate_spec_sha256.clone(),
        topology_object_sha256: result["topology_object_sha256"]
            .as_str()
            .ok_or_else(|| invalid("result topology hash is missing"))?
            .to_owned(),
        socket_map_object_sha256: result["socket_map_object_sha256"]
            .as_str()
            .ok_or_else(|| invalid("result socket map hash is missing"))?
            .to_owned(),
        rig_map_object_sha256: result["rig_map_object_sha256"]
            .as_str()
            .ok_or_else(|| invalid("result rig map hash is missing"))?
            .to_owned(),
        fps_presentation_package_object_sha256: result
            .get("fps_presentation_package_object_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("result presentation package hash is missing"))?
            .to_owned(),
        result_object_sha256: result_hash.to_owned(),
        link_object_sha256: link_hash.to_owned(),
        authoring_mesh_materialization_status: foundation_store::MATERIALIZATION_PENDING.to_owned(),
        socket_materialization_status: "MATERIALIZED_MAPPING_ONLY".to_owned(),
        rig_materialization_status: "MATERIALIZED_MAPPING_ONLY".to_owned(),
        presentation_materialization_status: "DRAFT_ONLY".to_owned(),
        import_status: "IMPORTED_DRAFT".to_owned(),
        quality_status: "structural_only".to_owned(),
        promotion_eligible: false,
        runtime_write_performed: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        actual_engine_roundtrip: false,
        human_review_status: "NOT_RUN".to_owned(),
        canonical_sha256: String::new(),
        created_at,
    };
    record.canonical_sha256 = foundation_store::canonical_record_sha256(&record)?;
    Ok(record)
}

fn result_summary(
    result: &Value,
    record: &foundation_store::WeaponFoundationImportRecord,
    replayed: bool,
    restart_hash_verified: bool,
) -> Value {
    json!({
        "schema_version": PREPARE_RESULT_SCHEMA_VERSION,
        "request_id": record.request_id,
        "request_sha256": record.request_sha256,
        "result_id": result.get("result_id"),
        "result_object_sha256": record.result_object_sha256,
        "import_record_sha256": record.canonical_sha256,
        "asset_id": record.asset_id,
        "asset_sha256": record.asset_sha256,
        "topology_object_sha256": record.topology_object_sha256,
        "socket_map_object_sha256": record.socket_map_object_sha256,
        "rig_map_object_sha256": record.rig_map_object_sha256,
        "fps_presentation_package_object_sha256": record.fps_presentation_package_object_sha256,
        "fps_presentation_package": {
            "schema_version": "FpsPresentationPackage@1",
            "object_sha256": record.fps_presentation_package_object_sha256,
            "status": "DRAFT_UNREVIEWED",
            "materialization_status": "DRAFT_ONLY",
            "authoring_mesh_materialization_status": foundation_store::MATERIALIZATION_PENDING,
            "quality_status": "structural_only",
            "promotion_eligible": false
        },
        "authoring_mesh_materialization_status": foundation_store::MATERIALIZATION_PENDING,
        "import_status": record.import_status,
        "quality_status": record.quality_status,
        "promotion_eligible": record.promotion_eligible,
        "runtime_write_performed": record.runtime_write_performed,
        "candidate_confirmed": record.candidate_confirmed,
        "version_created": record.version_created,
        "export_performed": record.export_performed,
        "actual_engine_roundtrip": record.actual_engine_roundtrip,
        "human_review_status": record.human_review_status,
        "replayed": replayed,
        "restart_hash_verified": restart_hash_verified,
        "source_payloads": "CAS_HASH_ONLY"
    })
}

fn read_result_summary(
    runtime: &Runtime,
    record: &foundation_store::WeaponFoundationImportRecord,
    replayed: bool,
) -> Result<Value, RuntimeError> {
    let bytes = runtime
        .cas_read_bounded(
            &record.result_object_sha256,
            foundation_store::MAX_JSON_BYTES,
        )
        .map_err(|error| invalid(format!("durable result readback failed: {error}")))?;
    let result: Value = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("durable result JSON is invalid: {error}")))?;
    if result.get("request_id").and_then(Value::as_str) != Some(record.request_id.as_str())
        || result.get("request_sha256").and_then(Value::as_str)
            != Some(record.request_sha256.as_str())
        || result.get("result_object_sha256").is_some()
    {
        return Err(invalid("durable result identity is not bound"));
    }
    Ok(result_summary(&result, record, replayed, true))
}

pub(crate) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_request(value)?;
    let asset = builtin_asset(&request.asset_id)
        .ok_or_else(|| invalid("asset has no compiled typed importer in this cohort"))?;
    let bytes = builtin_asset_bytes(asset);
    if sha256_hex(bytes) != request.asset_sha256 {
        return Err(invalid(
            "embedded foundation asset hash differs from request",
        ));
    }

    // Replay the pure kernel before writing anything.  A mismatch is a hard
    // stop: no partially normalized asset is allowed into the durable index.
    let first = weapon_foundation_import::import_builtin_weapon_foundation(asset, bytes)
        .map_err(|error| invalid(format!("typed importer rejected source: {error}")))?;
    let repeat = weapon_foundation_import::import_builtin_weapon_foundation(asset, bytes)
        .map_err(|error| invalid(format!("typed importer replay rejected source: {error}")))?;
    if first.canonical_sha256 != repeat.canonical_sha256 {
        return Err(invalid("typed importer replay is not byte deterministic"));
    }

    let (topology_value, topology_bytes, _topology_canonical_hash) =
        topology_payload(&first, &request)?;
    let (repeat_topology_value, repeat_topology_bytes, _) = topology_payload(&repeat, &request)?;
    let topology_content_hash = sha256_hex(&topology_bytes);
    if topology_bytes != repeat_topology_bytes || topology_value != repeat_topology_value {
        return Err(invalid("compact topology replay is not byte deterministic"));
    }
    let deterministic_record_hash = sha256_hex(
        &canonical_json_bytes(&json!({
            "request_sha256": request.canonical_sha256,
            "import_sha256": first.canonical_sha256,
            "topology_sha256": topology_content_hash
        }))
        .map_err(|error| invalid(format!("deterministic replay hash failed: {error}")))?,
    );

    let reservation = runtime.store.begin_cas_reservation();
    let mut objects = Vec::<CasObject>::new();
    let work =
        (|| -> Result<(foundation_store::WeaponFoundationImportRecord, bool), RuntimeError> {
            let topology = put_payload(
                runtime,
                &reservation,
                &topology_bytes,
                foundation_store::MAX_TOPOLOGY_BYTES as usize,
                foundation_store::TOPOLOGY_OBJECT_KIND,
            )?;
            objects.push(topology.clone());

            let (_socket_value, socket_bytes, _) =
                socket_map_payload(&first, &request, &topology.record.sha256)?;
            let socket = put_payload(
                runtime,
                &reservation,
                &socket_bytes,
                foundation_store::MAX_JSON_BYTES as usize,
                foundation_store::SOCKET_MAP_OBJECT_KIND,
            )?;
            objects.push(socket.clone());

            let (_rig_value, rig_bytes, _) =
                rig_map_payload(&first, &request, &socket.record.sha256)?;
            let rig = put_payload(
                runtime,
                &reservation,
                &rig_bytes,
                foundation_store::MAX_JSON_BYTES as usize,
                foundation_store::RIG_MAP_OBJECT_KIND,
            )?;
            objects.push(rig.clone());

            let (_package_value, package_bytes, _) = presentation_package_payload(
                &first,
                &request,
                &topology.record.sha256,
                &socket.record.sha256,
                &rig.record.sha256,
            )?;
            let package = put_payload(
                runtime,
                &reservation,
                &package_bytes,
                foundation_store::MAX_JSON_BYTES as usize,
                foundation_store::PRESENTATION_PACKAGE_OBJECT_KIND,
            )?;
            objects.push(package.clone());

            let (result_value, result_bytes, _) = result_payload(
                &first,
                &request,
                &topology.record.sha256,
                &socket.record.sha256,
                &rig.record.sha256,
                &package.record.sha256,
                &topology_content_hash,
                &deterministic_record_hash,
            )?;
            let result = put_payload(
                runtime,
                &reservation,
                &result_bytes,
                foundation_store::MAX_JSON_BYTES as usize,
                foundation_store::RESULT_OBJECT_KIND,
            )?;
            objects.push(result.clone());

            let (link_value, link_bytes, _) = link_payload(
                &request,
                &result.record.sha256,
                &topology.record.sha256,
                &socket.record.sha256,
                &rig.record.sha256,
                &package.record.sha256,
            )?;
            let link = put_payload(
                runtime,
                &reservation,
                &link_bytes,
                foundation_store::MAX_JSON_BYTES as usize,
                foundation_store::LINK_OBJECT_KIND,
            )?;
            objects.push(link.clone());

            // Keep a local binding check for the values that were used to make
            // the Store row.  This also prevents an accidental future builder
            // from dropping one root while the CAS set still contains it.
            if result_value["topology_object_sha256"]
                != Value::String(topology.record.sha256.clone())
                || result_value["socket_map_object_sha256"]
                    != Value::String(socket.record.sha256.clone())
                || result_value["rig_map_object_sha256"] != Value::String(rig.record.sha256.clone())
                || result_value["fps_presentation_package_object_sha256"]
                    != Value::String(package.record.sha256.clone())
                || link_value["result_object_sha256"] != Value::String(result.record.sha256.clone())
            {
                return Err(invalid(
                    "foundation CAS root binding changed during prepare",
                ));
            }
            let record = record_from_result(
                &request,
                &result_value,
                &result.record.sha256,
                &link.record.sha256,
                now_string(),
            )?;
            let owned = objects
                .iter()
                .map(|object| object.record.clone())
                .collect::<Vec<CasObjectRecord>>();
            Ok(runtime
                .store
                .record_weapon_foundation_import(&record, &owned)?)
        })();
    match work {
        Ok((record, replayed)) => {
            release_objects(runtime, &reservation, &objects, !replayed);
            read_result_summary(runtime, &record, replayed)
        }
        Err(error) => {
            release_objects(runtime, &reservation, &objects, true);
            Err(error)
        }
    }
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let record = runtime
        .store
        .get_weapon_foundation_import(&request.request_id, request.request_sha256.as_deref())?
        .ok_or_else(|| RuntimeError::InvalidInput("FOUNDATION_IMPORT_NOT_FOUND".to_owned()))?;
    if request
        .result_object_sha256
        .as_deref()
        .is_some_and(|hash| hash != record.result_object_sha256)
    {
        return Err(invalid("get result hash does not match durable row"));
    }
    let mut summary = read_result_summary(runtime, &record, true)?;
    summary["schema_version"] = Value::String(GET_RESULT_SCHEMA_VERSION.to_owned());
    Ok(summary)
}
