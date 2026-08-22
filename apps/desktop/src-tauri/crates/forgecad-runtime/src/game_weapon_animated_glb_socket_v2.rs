//! Pure, additive socket materialization checks for `GameWeaponAnimatedGlbSocket@2`.
//!
//! The durable Prepare/Get surface is deliberately kept separate from the V1
//! implementation in `rigid_animation_glb`.  This module owns only the
//! source/derived GLB proof used by that surface: six empty AnchorSet nodes
//! may be appended, while the animated/renderable projection and embedded BIN
//! remain byte-exact.  Store/Contract bindings are added at the call site once
//! the V2 durable records are available.

use super::{
    artifact_readback_v2_value, canonical_json_bytes, canonical_json_hash, exact_object,
    is_opaque_id, is_sha256, sha256_hex, strict_glb_inspection,
    validate_glb_material_pack_identity, Runtime, RuntimeError,
};
use crate::game_asset_delivery::{materialize_socket_glb, MaterializedSocketGlb};
use forgecad_contracts::{
    GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord,
    GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord,
    GameWeaponAnimatedGlbSocketMaterializationV2SocketNodeRecord,
    MechanicalAnimationGlbV2LinkRecord, MechanicalAnimationGlbV2ReceiptRecord,
};
use forgecad_store::CasObject;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const MAX_GLB_BYTES: usize = 64 * 1024 * 1024;
const SOCKET_METADATA_KEY: &str = "game_weapon_glb_socket_materialization";
const SOCKET_NODE_PREFIX: &str = "forgecad-anchor-";
const PREPARE_SCHEMA: &str = "GameWeaponAnimatedGlbSocketMaterializationPrepareRequest@2";
const GET_SCHEMA: &str = "GameWeaponAnimatedGlbSocketMaterializationGetRequest@2";
const PREPARE_RESULT_SCHEMA: &str = "GameWeaponAnimatedGlbSocketMaterializationPrepareResult@2";
const GET_RESULT_SCHEMA: &str = "GameWeaponAnimatedGlbSocketMaterializationGetResult@2";
const LINK_SCHEMA: &str = "GameWeaponAnimatedGlbSocketMaterializationLink@2";
const RECEIPT_SCHEMA: &str = "GameWeaponAnimatedGlbSocketMaterializationReceipt@2";
const SOCKET_METADATA_SCHEMA: &str = "GameWeaponGlbSocketMaterialization@2";
const POLICY: &str =
    "appearance-aware-animation-v2-socket-node-materialization-preserve-renderable-content@2";
const LOD_SCOPE: &str = "lod0-appearance-animated-source-only@2";
const GLB_KIND: &str = "game-weapon-animated-glb-v2-socket-materialized-glb";
const RECEIPT_KIND: &str = "game-weapon-animated-glb-v2-socket-materialization-receipt";
const GLB_MIME: &str = "model/gltf-binary";
const RECEIPT_MIME: &str = "application/json";
const STATUS: &str = "runtime-owned-durable-game-weapon-animated-glb-v2-socket-materialization";
const VALIDATOR_STATUS: &str =
    "strict-appearance-aware-animated-glb-socket-materialization-readback-pass";
const SEMANTIC_SCOPE: &str = "fictional-nonfunctional-game-visual-authoring-only@1";
const LIMITATIONS: &[&str] = &[
    "appearance-candidate-bound-rigid-Part-TRS-only",
    "scheduled-integer-ticks-and-LINEAR-interpolation-only",
    "no-skinning-morph-targets-armature-IK-constraints-NLA-or-drivers",
    "source-BIN-and-appearance-material-projection-must-remain-exact",
    "structural-readback-does-not-prove-visual-quality-or-engine-roundtrip",
];
const OWNED_CAS_KINDS: &[&str] = &[GLB_KIND, RECEIPT_KIND];
const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "appearance_candidate_id",
    "appearance_candidate_state_sha256",
    "clip_id",
    "clip_object_sha256",
    "clip_sha256",
    "appearance_delivery_manifest_object_sha256",
    "anchor_set_object_sha256",
    "anchor_set_canonical_sha256",
    "materialization_policy",
    "input_sha256",
    "idempotency_key",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "appearance_candidate_id",
    "clip_id",
    "animated_socket_materialization_key_sha256",
];

fn invalid(detail: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "GAME_WEAPON_ANIMATED_GLB_SOCKET_V2_INVALID: {}",
        detail.into()
    ))
}

#[derive(Debug, Clone)]
pub(super) struct SocketGlbV2Materialization {
    pub(super) glb: Vec<u8>,
    pub(super) source_renderable_inventory_sha256: String,
    pub(super) derived_renderable_inventory_sha256: String,
    pub(super) socket_node_inventory_sha256: String,
    pub(super) socket_nodes: Vec<Value>,
    pub(super) source_bin_sha256: String,
    pub(super) derived_bin_sha256: String,
    pub(super) source_node_count: usize,
    pub(super) derived_node_count: usize,
    pub(super) static_json_projection_sha256: String,
    pub(super) renderable_projection_sha256: String,
    pub(super) animation_projection_sha256: String,
    pub(super) source_bin_prefix_exact: bool,
    pub(super) static_json_projection_exact: bool,
    pub(super) renderable_projection_exact: bool,
    pub(super) animations_preserved: bool,
    pub(super) channels_preserved: bool,
    pub(super) samplers_preserved: bool,
    pub(super) skins_absent: bool,
    pub(super) morph_targets_absent: bool,
}

/// Materialize the existing pure six-node socket operation and then apply the
/// stricter animated V2 projection proof.  No CAS or Runtime state is touched
/// here, making the function safe to call for both first write and Get replay.
pub(super) fn materialize_socket_glb_v2(
    source_bytes: &[u8],
    source_artifact_sha256: &str,
    anchor_set_object_sha256: &str,
    anchor_set: &Value,
    part_ids: &[String],
    anchor_ids: &[String],
    expected_material_pack: Option<(&str, &str)>,
) -> Result<SocketGlbV2Materialization, RuntimeError> {
    let mut materialized = materialize_socket_glb(
        source_bytes,
        source_artifact_sha256,
        anchor_set_object_sha256,
        anchor_set,
        part_ids,
        anchor_ids,
    )?;
    materialized.glb = override_socket_metadata_v2(&materialized.glb)?;
    if let Some((pack_id, manifest_sha256)) = expected_material_pack {
        validate_glb_material_pack_identity(source_bytes, pack_id, manifest_sha256)?;
        validate_glb_material_pack_identity(&materialized.glb, pack_id, manifest_sha256)?;
    }
    let proof = validate_animated_socket_projection(
        source_bytes,
        &materialized.glb,
        &materialized,
        anchor_ids,
    )?;
    Ok(SocketGlbV2Materialization {
        glb: materialized.glb,
        source_renderable_inventory_sha256: materialized.source_renderable_inventory_sha256,
        derived_renderable_inventory_sha256: materialized.derived_renderable_inventory_sha256,
        socket_node_inventory_sha256: materialized.socket_node_inventory_sha256,
        socket_nodes: materialized.socket_nodes,
        source_bin_sha256: materialized.source_bin_sha256,
        derived_bin_sha256: materialized.derived_bin_sha256,
        source_node_count: materialized.source_node_count,
        derived_node_count: materialized.derived_node_count,
        static_json_projection_sha256: proof.static_json_projection_sha256,
        renderable_projection_sha256: proof.renderable_projection_sha256,
        animation_projection_sha256: proof.animation_projection_sha256,
        source_bin_prefix_exact: true,
        static_json_projection_exact: true,
        renderable_projection_exact: true,
        animations_preserved: true,
        channels_preserved: true,
        samplers_preserved: true,
        skins_absent: true,
        morph_targets_absent: true,
    })
}

#[derive(Debug, Clone)]
struct ProjectionProof {
    static_json_projection_sha256: String,
    renderable_projection_sha256: String,
    animation_projection_sha256: String,
}

fn encode_glb(root: &Value, binary: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let mut json_bytes = serde_json::to_vec(root)
        .map_err(|error| invalid(format!("GLB JSON cannot be encoded: {error}")))?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total_length = 12usize
        .checked_add(8)
        .and_then(|length| length.checked_add(json_bytes.len()))
        .and_then(|length| length.checked_add(8))
        .and_then(|length| length.checked_add(binary.len()))
        .ok_or_else(|| invalid("GLB output length overflowed"))?;
    if total_length > MAX_GLB_BYTES
        || total_length > u32::MAX as usize
        || binary.len() > u32::MAX as usize
    {
        return Err(invalid("GLB output exceeds its bounded size"));
    }
    let mut bytes = Vec::with_capacity(total_length);
    bytes.extend_from_slice(b"glTF");
    bytes.extend_from_slice(&2_u32.to_le_bytes());
    bytes.extend_from_slice(&(total_length as u32).to_le_bytes());
    bytes.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"JSON");
    bytes.extend_from_slice(&json_bytes);
    bytes.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"BIN\0");
    bytes.extend_from_slice(binary);
    Ok(bytes)
}

fn override_socket_metadata_v2(bytes: &[u8]) -> Result<Vec<u8>, RuntimeError> {
    let (mut root, binary) = parse_glb(bytes)?;
    let metadata = root
        .get_mut("extras")
        .and_then(Value::as_object_mut)
        .and_then(|extras| extras.get_mut("forgecad"))
        .and_then(Value::as_object_mut)
        .and_then(|forgecad| forgecad.get_mut(SOCKET_METADATA_KEY))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("socket GLB materialization metadata is unavailable"))?;
    metadata.insert(
        "schema_version".to_owned(),
        Value::String(SOCKET_METADATA_SCHEMA.to_owned()),
    );
    metadata.insert(
        "materialization_policy".to_owned(),
        Value::String(POLICY.to_owned()),
    );
    metadata.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    let canonical_sha256 = canonical_json_hash(&Value::Object(metadata.clone()));
    metadata.insert(
        "canonical_sha256".to_owned(),
        Value::String(canonical_sha256),
    );
    encode_glb(&root, &binary)
}

fn parse_glb(bytes: &[u8]) -> Result<(Value, Vec<u8>), RuntimeError> {
    if bytes.len() < 20
        || bytes.len() > MAX_GLB_BYTES
        || &bytes[0..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| invalid("GLB version"))?) != 2
        || &bytes[16..20] != b"JSON"
    {
        return Err(invalid("GLB header is invalid"));
    }
    let total_length = u32::from_le_bytes(
        bytes[8..12]
            .try_into()
            .map_err(|_| invalid("GLB total length"))?,
    ) as usize;
    if total_length != bytes.len() {
        return Err(invalid("GLB total length differs"));
    }
    let json_length = u32::from_le_bytes(
        bytes[12..16]
            .try_into()
            .map_err(|_| invalid("GLB JSON length"))?,
    ) as usize;
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or_else(|| invalid("GLB JSON length overflowed"))?;
    if json_end + 8 > bytes.len() || &bytes[json_end + 4..json_end + 8] != b"BIN\0" {
        return Err(invalid("GLB chunks are invalid"));
    }
    let binary_length = u32::from_le_bytes(
        bytes[json_end..json_end + 4]
            .try_into()
            .map_err(|_| invalid("GLB BIN length"))?,
    ) as usize;
    let binary_start = json_end + 8;
    let binary_end = binary_start
        .checked_add(binary_length)
        .filter(|end| *end == bytes.len())
        .ok_or_else(|| invalid("GLB BIN length differs"))?;
    let root: Value = serde_json::from_slice(&bytes[20..json_end])
        .map_err(|error| invalid(format!("GLB JSON is invalid: {error}")))?;
    Ok((root, bytes[binary_start..binary_end].to_vec()))
}

fn array<'a>(root: &'a Value, field: &str) -> Result<&'a Vec<Value>, RuntimeError> {
    root.get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("GLB {field} array is unavailable")))
}

fn reject_skin_or_morph(root: &Value) -> Result<(), RuntimeError> {
    if root.get("skins").is_some() {
        return Err(invalid("GLB socket materialization rejects skins"));
    }
    let has_morph = root
        .get("meshes")
        .and_then(Value::as_array)
        .is_some_and(|meshes| {
            meshes.iter().any(|mesh| {
                mesh.get("weights").is_some()
                    || mesh
                        .get("primitives")
                        .and_then(Value::as_array)
                        .is_some_and(|primitives| {
                            primitives
                                .iter()
                                .any(|primitive| primitive.get("targets").is_some())
                        })
            })
        })
        || root
            .get("nodes")
            .and_then(Value::as_array)
            .is_some_and(|nodes| nodes.iter().any(|node| node.get("weights").is_some()));
    if has_morph {
        return Err(invalid("GLB socket materialization rejects morph targets"));
    }
    Ok(())
}

fn animation_projection(root: &Value, source_node_count: usize) -> Result<String, RuntimeError> {
    let animations = root
        .get("animations")
        .and_then(Value::as_array)
        .filter(|animations| !animations.is_empty())
        .ok_or_else(|| invalid("animated GLB socket source has no animations"))?;
    let mut used_samplers = BTreeSet::new();
    for animation in animations {
        let samplers = animation
            .get("samplers")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("animation samplers are unavailable"))?;
        let channels = animation
            .get("channels")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("animation channels are unavailable"))?;
        for channel in channels {
            let sampler_index = channel
                .get("sampler")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid("animation channel sampler is invalid"))?
                as usize;
            let sampler = samplers
                .get(sampler_index)
                .ok_or_else(|| invalid("animation channel sampler overflows"))?;
            if sampler.get("interpolation").and_then(Value::as_str) != Some("LINEAR") {
                return Err(invalid("animation sampler interpolation is not LINEAR"));
            }
            used_samplers.insert(sampler_index);
            let target = channel
                .get("target")
                .ok_or_else(|| invalid("animation channel target is unavailable"))?;
            let node = target
                .get("node")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid("animation target node is invalid"))?;
            if node as usize >= source_node_count
                || !matches!(
                    target.get("path").and_then(Value::as_str),
                    Some("translation") | Some("rotation")
                )
            {
                return Err(invalid("animation target leaves the rigid Part domain"));
            }
        }
        if used_samplers.len() != samplers.len() {
            return Err(invalid("animation contains an unreferenced sampler"));
        }
    }
    Ok(canonical_json_hash(root.get("animations").ok_or_else(
        || invalid("animation projection is unavailable"),
    )?))
}

fn normalize_source_nodes(
    source_nodes: &[Value],
    derived_nodes: &[Value],
    source_node_count: usize,
) -> Result<Vec<Value>, RuntimeError> {
    if source_nodes.len() != source_node_count || derived_nodes.len() != source_node_count + 6 {
        return Err(invalid("socket GLB node count is not source plus six"));
    }
    let mut normalized = Vec::with_capacity(source_node_count);
    for (index, source_node) in source_nodes.iter().enumerate() {
        let mut node = derived_nodes
            .get(index)
            .cloned()
            .ok_or_else(|| invalid("socket GLB source node is unavailable"))?;
        let source_children = source_node.get("children").and_then(Value::as_array);
        let derived_children = node.get("children").and_then(Value::as_array);
        if let Some(children) = derived_children {
            if children.iter().any(|child| {
                child
                    .as_u64()
                    .is_none_or(|child| child as usize >= derived_nodes.len())
            }) {
                return Err(invalid("socket GLB child index overflows"));
            }
            let kept = children
                .iter()
                .filter(|child| {
                    child
                        .as_u64()
                        .is_some_and(|child| (child as usize) < source_node_count)
                })
                .cloned()
                .collect::<Vec<_>>();
            if source_children.map(|value| value.as_slice()) != Some(kept.as_slice()) {
                if source_children.is_some() || !kept.is_empty() {
                    return Err(invalid("socket GLB source child projection changed"));
                }
            }
            if kept.is_empty() {
                node.as_object_mut()
                    .ok_or_else(|| invalid("socket GLB node is not an object"))?
                    .remove("children");
            } else {
                node["children"] = Value::Array(kept);
            }
        } else if source_children.is_some() {
            return Err(invalid("socket GLB source children disappeared"));
        }
        normalized.push(node);
    }
    Ok(normalized)
}

fn normalize_source_scenes(
    source: &Value,
    derived: &Value,
    source_node_count: usize,
) -> Result<Value, RuntimeError> {
    let mut scenes = derived
        .get("scenes")
        .cloned()
        .ok_or_else(|| invalid("socket GLB scenes are unavailable"))?;
    let source_scenes = source
        .get("scenes")
        .ok_or_else(|| invalid("source GLB scenes are unavailable"))?;
    let scenes_array = scenes
        .as_array_mut()
        .ok_or_else(|| invalid("socket GLB scenes are invalid"))?;
    for scene in scenes_array {
        let nodes = scene
            .get_mut("nodes")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("socket GLB scene roots are invalid"))?;
        nodes.retain(|node| {
            node.as_u64()
                .is_some_and(|node| (node as usize) < source_node_count)
        });
    }
    if scenes != *source_scenes {
        return Err(invalid("socket GLB scene projection changed"));
    }
    Ok(scenes)
}

fn validate_animated_socket_projection(
    source_bytes: &[u8],
    derived_bytes: &[u8],
    materialized: &MaterializedSocketGlb,
    anchor_ids: &[String],
) -> Result<ProjectionProof, RuntimeError> {
    if source_bytes.is_empty() || derived_bytes.is_empty() {
        return Err(invalid("socket GLB source or derived bytes are empty"));
    }
    let (source_root, source_binary) = parse_glb(source_bytes)?;
    let (derived_root, derived_binary) = parse_glb(derived_bytes)?;
    reject_skin_or_morph(&source_root)?;
    reject_skin_or_morph(&derived_root)?;
    if derived_binary.len() != source_binary.len()
        || derived_binary.get(..source_binary.len()) != Some(source_binary.as_slice())
    {
        return Err(invalid("socket GLB BIN is not the full source prefix"));
    }
    if materialized.source_bin_sha256 != sha256_hex(&source_binary)
        || materialized.derived_bin_sha256 != sha256_hex(&derived_binary)
    {
        return Err(invalid("socket GLB BIN inventory hash differs"));
    }
    let source_nodes = array(&source_root, "nodes")?;
    let derived_nodes = array(&derived_root, "nodes")?;
    if materialized.source_node_count != source_nodes.len()
        || materialized.derived_node_count != derived_nodes.len()
        || materialized.socket_nodes.len() != anchor_ids.len()
        || anchor_ids.len() != 6
    {
        return Err(invalid("socket GLB source/derived node inventory differs"));
    }
    let appended = &derived_nodes[materialized.source_node_count..];
    let mut appended_names = BTreeSet::new();
    for (index, node) in appended.iter().enumerate() {
        let object = node
            .as_object()
            .ok_or_else(|| invalid("socket GLB appended node is not an object"))?;
        let expected_name = format!("{SOCKET_NODE_PREFIX}{}", anchor_ids[index]);
        if object.get("name").and_then(Value::as_str) != Some(expected_name.as_str())
            || object.get("mesh").is_some()
            || object.get("skin").is_some()
            || object.get("camera").is_some()
            || object.get("weights").is_some()
            || !appended_names.insert(expected_name)
        {
            return Err(invalid(
                "socket GLB appended node is not an empty Anchor node",
            ));
        }
    }
    if source_root.get("animations") != derived_root.get("animations") {
        return Err(invalid("socket GLB animation JSON changed"));
    }
    let animation_projection_sha256 = animation_projection(&source_root, source_nodes.len())?;
    if animation_projection(&derived_root, source_nodes.len())? != animation_projection_sha256 {
        return Err(invalid("socket GLB animation projection changed"));
    }
    for field in [
        "meshes",
        "materials",
        "textures",
        "images",
        "samplers",
        "accessors",
        "bufferViews",
        "buffers",
    ] {
        if source_root.get(field) != derived_root.get(field) {
            return Err(invalid(format!("socket GLB {field} projection changed")));
        }
    }
    let normalized_nodes = normalize_source_nodes(source_nodes, derived_nodes, source_nodes.len())?;
    let normalized_scenes =
        normalize_source_scenes(&source_root, &derived_root, source_nodes.len())?;
    let mut projected = derived_root.clone();
    projected["nodes"] = Value::Array(normalized_nodes);
    projected["scenes"] = normalized_scenes;
    projected
        .get_mut("extras")
        .and_then(Value::as_object_mut)
        .and_then(|extras| extras.get_mut("forgecad"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("socket GLB forgecad metadata is unavailable"))?
        .remove(SOCKET_METADATA_KEY);
    if projected != source_root {
        return Err(invalid("socket GLB static JSON projection is not exact"));
    }
    let renderable_projection = json!({
        "meshes":source_root.get("meshes").cloned().unwrap_or(Value::Null),
        "materials":source_root.get("materials").cloned().unwrap_or(Value::Null),
        "textures":source_root.get("textures").cloned().unwrap_or(Value::Null),
        "images":source_root.get("images").cloned().unwrap_or(Value::Null),
        "samplers":source_root.get("samplers").cloned().unwrap_or(Value::Null),
    });
    Ok(ProjectionProof {
        static_json_projection_sha256: canonical_json_hash(&source_root),
        renderable_projection_sha256: canonical_json_hash(&renderable_projection),
        animation_projection_sha256,
    })
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("{field} is invalid")))
}

fn id<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an opaque identifier")));
    }
    Ok(value)
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(value)
}

fn parse_prepare(value: &Value) -> Result<(Map<String, Value>, String, String), RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?.clone();
    if text(&object, "schema_version")? != PREPARE_SCHEMA
        || text(&object, "materialization_policy")? != POLICY
    {
        return Err(invalid("prepare schema or materialization policy differs"));
    }
    id(&object, "project_id")?;
    id(&object, "appearance_candidate_id")?;
    id(&object, "clip_id")?;
    id(&object, "idempotency_key")?;
    for field in [
        "appearance_candidate_state_sha256",
        "clip_object_sha256",
        "clip_sha256",
        "appearance_delivery_manifest_object_sha256",
        "anchor_set_object_sha256",
        "anchor_set_canonical_sha256",
        "input_sha256",
    ] {
        sha(&object, field)?;
    }
    let mut input_preimage = object.clone();
    input_preimage.remove("input_sha256");
    input_preimage.remove("idempotency_key");
    let input_sha256 = canonical_json_hash(&Value::Object(input_preimage));
    if sha(&object, "input_sha256")? != input_sha256 {
        return Err(invalid("input_sha256 does not bind the closed request"));
    }
    let key = canonical_json_hash(&json!({
        "project_id":id(&object,"project_id")?,
        "appearance_candidate_id":id(&object,"appearance_candidate_id")?,
        "appearance_candidate_state_sha256":sha(&object,"appearance_candidate_state_sha256")?,
        "clip_id":id(&object,"clip_id")?,
        "clip_object_sha256":sha(&object,"clip_object_sha256")?,
        "clip_sha256":sha(&object,"clip_sha256")?,
        "appearance_delivery_manifest_object_sha256":sha(&object,"appearance_delivery_manifest_object_sha256")?,
        "anchor_set_object_sha256":sha(&object,"anchor_set_object_sha256")?,
        "anchor_set_canonical_sha256":sha(&object,"anchor_set_canonical_sha256")?,
        "materialization_policy":POLICY,
    }));
    Ok((object, key, input_sha256))
}

fn parse_get(value: &Value) -> Result<(String, String, String, String), RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if text(object, "schema_version")? != GET_SCHEMA {
        return Err(invalid("get schema differs"));
    }
    Ok((
        id(object, "project_id")?.to_owned(),
        id(object, "appearance_candidate_id")?.to_owned(),
        id(object, "clip_id")?.to_owned(),
        sha(object, "animated_socket_materialization_key_sha256")?.to_owned(),
    ))
}

fn derive_key_from_link(link: &GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord) -> String {
    canonical_json_hash(&json!({
        "project_id":link.project_id,
        "appearance_candidate_id":link.appearance_candidate_id,
        "appearance_candidate_state_sha256":link.appearance_candidate_state_sha256,
        "clip_id":link.clip_id,
        "clip_object_sha256":link.clip_object_sha256,
        "clip_sha256":link.clip_sha256,
        "appearance_delivery_manifest_object_sha256":link.appearance_delivery_manifest_object_sha256,
        "anchor_set_object_sha256":link.anchor_set_object_sha256,
        "anchor_set_canonical_sha256":link.anchor_set_canonical_sha256,
        "materialization_policy":POLICY,
    }))
}

fn derive_request_sha256_from_link(
    link: &GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord,
) -> String {
    canonical_json_hash(&json!({
        "schema_version":PREPARE_SCHEMA,
        "project_id":link.project_id,
        "appearance_candidate_id":link.appearance_candidate_id,
        "appearance_candidate_state_sha256":link.appearance_candidate_state_sha256,
        "clip_id":link.clip_id,
        "clip_object_sha256":link.clip_object_sha256,
        "clip_sha256":link.clip_sha256,
        "appearance_delivery_manifest_object_sha256":link.appearance_delivery_manifest_object_sha256,
        "anchor_set_object_sha256":link.anchor_set_object_sha256,
        "anchor_set_canonical_sha256":link.anchor_set_canonical_sha256,
        "materialization_policy":POLICY,
    }))
}

#[derive(Debug)]
struct SourceContext {
    parent_link: MechanicalAnimationGlbV2LinkRecord,
    parent_receipt: MechanicalAnimationGlbV2ReceiptRecord,
    anchor_set: Value,
    part_ids: Vec<String>,
    anchor_ids: Vec<String>,
    source_glb: Vec<u8>,
    socket_node_id_encoding_sha256: String,
}

fn load_source(
    runtime: &Runtime,
    project_id: &str,
    appearance_candidate_id: &str,
    appearance_candidate_state_sha256: &str,
    clip_id: &str,
    clip_object_sha256: &str,
    clip_sha256: &str,
    delivery_sha256: &str,
    anchor_object_sha256: &str,
    anchor_canonical_sha256: &str,
) -> Result<SourceContext, RuntimeError> {
    let parent = super::mechanical_animation_glb_v2::get(
        runtime,
        &json!({
            "schema_version":"MechanicalAnimationGlbGetRequest@2",
            "project_id":project_id,
            "appearance_candidate_id":appearance_candidate_id,
            "clip_id":clip_id,
        }),
    )?;
    let parent_link: MechanicalAnimationGlbV2LinkRecord = serde_json::from_value(
        parent
            .get("durable_link")
            .cloned()
            .ok_or_else(|| invalid("MechanicalAnimationGlb@2 durable link is unavailable"))?,
    )
    .map_err(|error| {
        invalid(format!(
            "MechanicalAnimationGlb@2 link is malformed: {error}"
        ))
    })?;
    let parent_receipt: MechanicalAnimationGlbV2ReceiptRecord = serde_json::from_value(
        parent
            .get("receipt")
            .cloned()
            .ok_or_else(|| invalid("MechanicalAnimationGlb@2 receipt is unavailable"))?,
    )
    .map_err(|error| {
        invalid(format!(
            "MechanicalAnimationGlb@2 receipt is malformed: {error}"
        ))
    })?;
    if parent_link.project_id != project_id
        || parent_link.appearance_candidate_id != appearance_candidate_id
        || parent_link.appearance_candidate_state_sha256 != appearance_candidate_state_sha256
        || parent_link.clip_id != clip_id
        || parent_link.clip_object_sha256 != clip_object_sha256
        || parent_link.clip_sha256 != clip_sha256
        || parent_receipt.hard_gate_passed != true
        || parent_receipt.quality_status != "structural_only"
    {
        return Err(invalid("MechanicalAnimationGlb@2 binding differs"));
    }

    let delivery = super::game_asset_delivery::get(
        runtime,
        &json!({
            "schema_version":"GameAssetDeliveryGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256,
        }),
    )?;
    let levels = delivery
        .get("lod_receipt")
        .and_then(|value| value.get("levels"))
        .and_then(Value::as_array)
        .filter(|levels| levels.len() == 3)
        .ok_or_else(|| invalid("appearance delivery LOD receipt is incomplete"))?;
    let lod0 = levels
        .first()
        .ok_or_else(|| invalid("appearance delivery LOD0 is unavailable"))?;
    if lod0.get("level").and_then(Value::as_u64) != Some(0)
        || lod0.get("candidate_id").and_then(Value::as_str) != Some(appearance_candidate_id)
        || lod0.get("candidate_state_sha256").and_then(Value::as_str)
            != Some(appearance_candidate_state_sha256)
        || lod0.get("artifact_sha256").and_then(Value::as_str)
            != Some(parent_link.appearance_artifact_sha256.as_str())
    {
        return Err(invalid("appearance delivery LOD0 binding differs"));
    }

    let anchor = super::game_asset_delivery::weapon_anchor_get(
        runtime,
        &json!({
            "schema_version":"GameWeaponAnchorGetRequest@1",
            "project_id":project_id,
            "delivery_manifest_object_sha256":delivery_sha256,
        }),
    )?;
    let anchor_link = anchor
        .get("link")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("AnchorSet durable link is unavailable"))?;
    if anchor_link
        .get("anchor_set_object_sha256")
        .and_then(Value::as_str)
        != Some(anchor_object_sha256)
    {
        return Err(invalid("AnchorSet object differs"));
    }
    let anchor_set = anchor
        .get("anchor_set")
        .cloned()
        .ok_or_else(|| invalid("AnchorSet is unavailable"))?;
    if anchor_set.get("canonical_sha256").and_then(Value::as_str) != Some(anchor_canonical_sha256) {
        return Err(invalid("AnchorSet canonical hash differs"));
    }
    let part_ids = anchor_set
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("AnchorSet Part inventory is unavailable"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| is_opaque_id(value))
                .map(str::to_owned)
                .ok_or_else(|| invalid("AnchorSet Part ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let anchor_ids = super::game_asset_delivery::socket_anchor_ids(&anchor_set)?;
    if part_ids.is_empty() || anchor_ids.len() != 6 {
        return Err(invalid("AnchorSet inventory is outside the V2 bound"));
    }
    let node_encoding = super::game_asset_delivery::socket_node_id_encoding_value()?;
    let socket_node_id_encoding_sha256 = node_encoding
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("socket node encoding hash is unavailable"))?
        .to_owned();
    let source_glb =
        runtime.cas_read_bounded(&parent_link.animated_artifact_sha256, MAX_GLB_BYTES as u64)?;
    if sha256_hex(&source_glb) != parent_link.animated_artifact_sha256 {
        return Err(invalid("MechanicalAnimationGlb@2 CAS bytes differ"));
    }
    Ok(SourceContext {
        parent_link,
        parent_receipt,
        anchor_set,
        part_ids,
        anchor_ids,
        source_glb,
        socket_node_id_encoding_sha256,
    })
}

fn set_canonical<T: serde::Serialize>(value: &mut T) -> Result<(), RuntimeError>
where
    T: CanonicalField,
{
    value.set_canonical(String::new());
    let hash = canonical_json_hash(
        &serde_json::to_value(&*value)
            .map_err(|error| invalid(format!("canonical value cannot be serialized: {error}")))?,
    );
    value.set_canonical(hash);
    Ok(())
}

trait CanonicalField {
    fn set_canonical(&mut self, value: String);
}

impl CanonicalField for GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord {
    fn set_canonical(&mut self, value: String) {
        self.canonical_sha256 = value;
    }
}

impl CanonicalField for GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord {
    fn set_canonical(&mut self, value: String) {
        self.canonical_sha256 = value;
    }
}

fn build_receipt(
    context: &SourceContext,
    key: &str,
    request_sha256: &str,
    delivery_sha256: &str,
    anchor_object_sha256: &str,
    anchor_canonical_sha256: &str,
    derived_sha256: &str,
    derived_readback_sha256: &str,
    materialized: &SocketGlbV2Materialization,
) -> Result<GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord, RuntimeError> {
    let socket_nodes = materialized
        .socket_nodes
        .iter()
        .cloned()
        .map(serde_json::from_value)
        .collect::<Result<Vec<GameWeaponAnimatedGlbSocketMaterializationV2SocketNodeRecord>, _>>()
        .map_err(|error| invalid(format!("socket node readback is malformed: {error}")))?;
    let derived_validation_sha256 = canonical_json_hash(&json!({
        "animation_glb_key_sha256":context.parent_link.animation_glb_key_sha256,
        "derived_artifact_sha256":derived_sha256,
        "derived_artifact_readback_sha256":derived_readback_sha256,
        "animation_projection_sha256":materialized.animation_projection_sha256,
        "renderable_projection_sha256":materialized.renderable_projection_sha256,
        "static_projection_sha256":materialized.static_json_projection_sha256,
        "socket_node_inventory_sha256":materialized.socket_node_inventory_sha256,
    }));
    let mut receipt = GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord {
        schema_version: RECEIPT_SCHEMA.to_owned(),
        animated_socket_materialization_key_sha256: key.to_owned(),
        project_id: context.parent_link.project_id.clone(),
        appearance_candidate_id: context.parent_link.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: context
            .parent_link
            .appearance_candidate_state_sha256
            .clone(),
        appearance_delivery_manifest_object_sha256: delivery_sha256.to_owned(),
        appearance_artifact_sha256: context.parent_link.appearance_artifact_sha256.clone(),
        appearance_artifact_readback_sha256: context
            .parent_link
            .appearance_artifact_readback_sha256
            .clone(),
        animation_glb_key_sha256: context.parent_link.animation_glb_key_sha256.clone(),
        animated_artifact_sha256: context.parent_link.animated_artifact_sha256.clone(),
        animated_artifact_readback_sha256: context
            .parent_link
            .animated_artifact_readback_sha256
            .clone(),
        animation_receipt_object_sha256: context.parent_link.receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: context.parent_link.receipt_canonical_sha256.clone(),
        clip_id: context.parent_link.clip_id.clone(),
        clip_object_sha256: context.parent_link.clip_object_sha256.clone(),
        clip_sha256: context.parent_link.clip_sha256.clone(),
        anchor_set_object_sha256: anchor_object_sha256.to_owned(),
        anchor_set_canonical_sha256: anchor_canonical_sha256.to_owned(),
        request_sha256: request_sha256.to_owned(),
        socket_materialization_policy: POLICY.to_owned(),
        lod_scope: LOD_SCOPE.to_owned(),
        socket_node_id_encoding_sha256: context.socket_node_id_encoding_sha256.clone(),
        derived_animated_socket_artifact_sha256: derived_sha256.to_owned(),
        derived_animated_socket_artifact_readback_sha256: derived_readback_sha256.to_owned(),
        source_animation_projection_sha256: materialized.animation_projection_sha256.clone(),
        derived_animation_projection_sha256: materialized.animation_projection_sha256.clone(),
        source_animation_validation_sha256: context
            .parent_receipt
            .animation_validation_sha256
            .clone(),
        derived_animation_validation_sha256: derived_validation_sha256,
        source_renderable_inventory_sha256: materialized.source_renderable_inventory_sha256.clone(),
        derived_renderable_inventory_sha256: materialized
            .derived_renderable_inventory_sha256
            .clone(),
        source_bin_sha256: materialized.source_bin_sha256.clone(),
        derived_bin_sha256: materialized.derived_bin_sha256.clone(),
        source_appearance_material_projection_sha256: context
            .parent_receipt
            .appearance_material_projection_sha256
            .clone(),
        derived_appearance_material_projection_sha256: context
            .parent_receipt
            .appearance_material_projection_sha256
            .clone(),
        sampling_policy_sha256: context.parent_link.sampling_policy_sha256.clone(),
        sample_time_ticks: context.parent_receipt.sample_time_ticks.clone(),
        part_ids: context.parent_receipt.part_ids.clone(),
        sampler_count: context.parent_receipt.sampler_count,
        channel_count: context.parent_receipt.channel_count,
        node_count: context.parent_receipt.node_count,
        source_node_count: materialized.source_node_count as u64,
        derived_node_count: materialized.derived_node_count as u64,
        accessor_count_added: context.parent_receipt.accessor_count_added,
        buffer_view_count_added: context.parent_receipt.buffer_view_count_added,
        socket_node_inventory_sha256: materialized.socket_node_inventory_sha256.clone(),
        socket_node_count: socket_nodes.len() as u64,
        socket_nodes,
        owned_cas_kinds: OWNED_CAS_KINDS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        animations_preserved: materialized.animations_preserved,
        channels_preserved: materialized.channels_preserved,
        samplers_preserved: materialized.samplers_preserved,
        renderable_projection_exact: materialized.renderable_projection_exact,
        bin_byte_exact: materialized.source_bin_sha256 == materialized.derived_bin_sha256,
        source_static_projection_exact: materialized.static_json_projection_exact,
        appearance_material_projection_exact: true,
        material_pack_identity_exact: true,
        no_skinning: materialized.skins_absent,
        no_morph_targets: materialized.morph_targets_absent,
        socket_nodes_materialized: true,
        runtime_write_performed: true,
        restart_hash_verified: true,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        production_stage_advanced: false,
        actual_engine_roundtrip: false,
        semantic_scope: SEMANTIC_SCOPE.to_owned(),
        functional_semantics: false,
        materialization_status: STATUS.to_owned(),
        validator_status: VALIDATOR_STATUS.to_owned(),
        hard_gate_passed: true,
        quality_status: "structural_only".to_owned(),
        visual_quality_status: "NOT_PROVEN".to_owned(),
        commercial_fps_quality_status: "NOT_PROVEN".to_owned(),
        human_review_status: "NOT_RUN".to_owned(),
        commercial_engine_status: "NOT_RUN".to_owned(),
        limitations: LIMITATIONS
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        canonical_sha256: String::new(),
        created_at: context.parent_receipt.created_at.clone(),
    };
    set_canonical(&mut receipt)?;
    Ok(receipt)
}

fn build_link(
    context: &SourceContext,
    receipt: &GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord,
    receipt_object_sha256: &str,
) -> Result<GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord, RuntimeError> {
    let mut link = GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord {
        schema_version: LINK_SCHEMA.to_owned(),
        animated_socket_materialization_key_sha256: receipt
            .animated_socket_materialization_key_sha256
            .clone(),
        project_id: receipt.project_id.clone(),
        appearance_candidate_id: receipt.appearance_candidate_id.clone(),
        appearance_candidate_state_sha256: receipt.appearance_candidate_state_sha256.clone(),
        appearance_delivery_manifest_object_sha256: receipt
            .appearance_delivery_manifest_object_sha256
            .clone(),
        appearance_artifact_sha256: receipt.appearance_artifact_sha256.clone(),
        appearance_artifact_readback_sha256: receipt.appearance_artifact_readback_sha256.clone(),
        animation_glb_key_sha256: context.parent_link.animation_glb_key_sha256.clone(),
        animated_artifact_sha256: context.parent_link.animated_artifact_sha256.clone(),
        animated_artifact_readback_sha256: context
            .parent_link
            .animated_artifact_readback_sha256
            .clone(),
        animation_receipt_object_sha256: context.parent_link.receipt_object_sha256.clone(),
        animation_receipt_canonical_sha256: context.parent_link.receipt_canonical_sha256.clone(),
        clip_id: context.parent_link.clip_id.clone(),
        clip_object_sha256: context.parent_link.clip_object_sha256.clone(),
        clip_sha256: context.parent_link.clip_sha256.clone(),
        anchor_set_object_sha256: receipt.anchor_set_object_sha256.clone(),
        anchor_set_canonical_sha256: receipt.anchor_set_canonical_sha256.clone(),
        request_sha256: receipt.request_sha256.clone(),
        socket_materialization_policy: POLICY.to_owned(),
        lod_scope: LOD_SCOPE.to_owned(),
        socket_node_id_encoding_sha256: context.socket_node_id_encoding_sha256.clone(),
        derived_animated_socket_artifact_sha256: receipt
            .derived_animated_socket_artifact_sha256
            .clone(),
        derived_animated_socket_artifact_readback_sha256: receipt
            .derived_animated_socket_artifact_readback_sha256
            .clone(),
        receipt_object_sha256: receipt_object_sha256.to_owned(),
        validator_status: VALIDATOR_STATUS.to_owned(),
        hard_gate_passed: true,
        materialization_status: STATUS.to_owned(),
        quality_status: "structural_only".to_owned(),
        canonical_sha256: String::new(),
        created_at: receipt.created_at.clone(),
    };
    set_canonical(&mut link)?;
    Ok(link)
}

fn result_value(
    schema: &str,
    link: &GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord,
    receipt: &GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord,
    replayed: bool,
    runtime_write_performed: bool,
) -> Result<Value, RuntimeError> {
    Ok(json!({
        "schema_version":schema,
        "animated_socket_materialization_key_sha256":link.animated_socket_materialization_key_sha256,
        "derived_animated_socket_artifact_sha256":link.derived_animated_socket_artifact_sha256,
        "receipt_object_sha256":link.receipt_object_sha256,
        "receipt":serde_json::to_value(receipt).map_err(|error| invalid(error.to_string()))?,
        "durable_link":serde_json::to_value(link).map_err(|error| invalid(error.to_string()))?,
        "replayed":replayed,
        "restart_hash_verified":true,
        "runtime_write_performed":runtime_write_performed,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "production_stage_advanced":false,
        "actual_engine_roundtrip":false,
        "quality_status":"structural_only",
    }))
}

fn release_reservation_objects(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    objects: &[&CasObject],
    cleanup: bool,
) -> Result<(), RuntimeError> {
    let mut errors = Vec::new();
    for object in objects.iter().rev() {
        if let Err(error) = runtime.store.release_cas_reservation_object(
            reservation,
            object,
            cleanup && object.created_new,
        ) {
            errors.push(error.to_string());
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(invalid(format!(
            "V2 socket CAS reservation release failed: {}",
            errors.join(" | ")
        )))
    }
}

fn verify_durable(
    runtime: &Runtime,
    link: &GameWeaponAnimatedGlbSocketMaterializationV2LinkRecord,
) -> Result<GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord, RuntimeError> {
    if derive_key_from_link(link) != link.animated_socket_materialization_key_sha256 {
        return Err(invalid(
            "durable V2 socket key does not bind the frozen business inputs",
        ));
    }
    if derive_request_sha256_from_link(link) != link.request_sha256 {
        return Err(invalid(
            "durable V2 socket request hash does not bind the frozen request",
        ));
    }
    let context = load_source(
        runtime,
        &link.project_id,
        &link.appearance_candidate_id,
        &link.appearance_candidate_state_sha256,
        &link.clip_id,
        &link.clip_object_sha256,
        &link.clip_sha256,
        &link.appearance_delivery_manifest_object_sha256,
        &link.anchor_set_object_sha256,
        &link.anchor_set_canonical_sha256,
    )?;
    if context.parent_link.animation_glb_key_sha256 != link.animation_glb_key_sha256
        || context.parent_link.animated_artifact_sha256 != link.animated_artifact_sha256
        || context.parent_link.animated_artifact_readback_sha256
            != link.animated_artifact_readback_sha256
        || context.parent_link.receipt_object_sha256 != link.animation_receipt_object_sha256
        || context.parent_link.receipt_canonical_sha256 != link.animation_receipt_canonical_sha256
        || context.parent_link.appearance_artifact_sha256 != link.appearance_artifact_sha256
        || context.parent_link.appearance_artifact_readback_sha256
            != link.appearance_artifact_readback_sha256
    {
        return Err(invalid("durable V2 socket parent binding differs"));
    }
    let materialized = materialize_socket_glb_v2(
        &context.source_glb,
        &context.parent_link.animated_artifact_sha256,
        &link.anchor_set_object_sha256,
        &context.anchor_set,
        &context.part_ids,
        &context.anchor_ids,
        Some((
            &context.parent_receipt.material_pack_id,
            &context.parent_receipt.material_pack_manifest_sha256,
        )),
    )?;
    let derived_sha256 = sha256_hex(&materialized.glb);
    if derived_sha256 != link.derived_animated_socket_artifact_sha256 {
        return Err(invalid("durable V2 socket derived artifact hash differs"));
    }
    let stored = runtime.cas_read_bounded(&derived_sha256, MAX_GLB_BYTES as u64)?;
    if stored != materialized.glb {
        return Err(invalid("durable V2 socket GLB is not byte-replayable"));
    }
    let inspection = strict_glb_inspection(&stored)?;
    let readback = artifact_readback_v2_value(
        &derived_sha256,
        &link.appearance_candidate_id,
        &inspection,
        stored.len() as u64,
    );
    let derived_readback_sha256 = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("durable V2 socket ArtifactReadback hash is unavailable"))?;
    if derived_readback_sha256 != link.derived_animated_socket_artifact_readback_sha256 {
        return Err(invalid("durable V2 socket ArtifactReadback hash differs"));
    }
    let expected = build_receipt(
        &context,
        &link.animated_socket_materialization_key_sha256,
        &link.request_sha256,
        &link.appearance_delivery_manifest_object_sha256,
        &link.anchor_set_object_sha256,
        &link.anchor_set_canonical_sha256,
        &derived_sha256,
        derived_readback_sha256,
        &materialized,
    )?;
    let receipt_bytes = runtime.cas_read_bounded(&link.receipt_object_sha256, 1024 * 1024)?;
    if sha256_hex(&receipt_bytes) != link.receipt_object_sha256 {
        return Err(invalid("durable V2 socket receipt CAS hash differs"));
    }
    let receipt: GameWeaponAnimatedGlbSocketMaterializationV2ReceiptRecord =
        serde_json::from_slice(&receipt_bytes)
            .map_err(|error| invalid(format!("durable V2 socket receipt is malformed: {error}")))?;
    let expected_bytes = canonical_json_bytes(
        &serde_json::to_value(&expected).map_err(|error| invalid(error.to_string()))?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    if receipt_bytes != expected_bytes || receipt != expected {
        return Err(invalid("durable V2 socket receipt is not byte-replayable"));
    }
    Ok(receipt)
}

pub(super) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let (object, key, request_sha256) = parse_prepare(value)?;
    let project_id = id(&object, "project_id")?.to_owned();
    let appearance_candidate_id = id(&object, "appearance_candidate_id")?.to_owned();
    let clip_id = id(&object, "clip_id")?.to_owned();
    if let Some(existing) = runtime
        .store
        .get_game_weapon_animated_glb_socket_materialization_v2_link(&key)?
    {
        if existing.project_id != project_id
            || existing.appearance_candidate_id != appearance_candidate_id
            || existing.appearance_candidate_state_sha256
                != sha(&object, "appearance_candidate_state_sha256")?
            || existing.clip_id != clip_id
            || existing.clip_object_sha256 != sha(&object, "clip_object_sha256")?
            || existing.clip_sha256 != sha(&object, "clip_sha256")?
            || existing.appearance_delivery_manifest_object_sha256
                != sha(&object, "appearance_delivery_manifest_object_sha256")?
            || existing.anchor_set_object_sha256 != sha(&object, "anchor_set_object_sha256")?
            || existing.anchor_set_canonical_sha256 != sha(&object, "anchor_set_canonical_sha256")?
            || existing.request_sha256 != request_sha256
        {
            return Err(invalid("V2 socket key is bound to different frozen inputs"));
        }
        let mut replay = get(
            runtime,
            &json!({
                "schema_version":GET_SCHEMA,
                "project_id":project_id,
                "appearance_candidate_id":appearance_candidate_id,
                "clip_id":clip_id,
                "animated_socket_materialization_key_sha256":key,
            }),
        )?;
        replay["schema_version"] = Value::String(PREPARE_RESULT_SCHEMA.to_owned());
        replay["replayed"] = Value::Bool(true);
        replay["runtime_write_performed"] = Value::Bool(true);
        return Ok(replay);
    }

    let context = load_source(
        runtime,
        &project_id,
        &appearance_candidate_id,
        sha(&object, "appearance_candidate_state_sha256")?,
        &clip_id,
        sha(&object, "clip_object_sha256")?,
        sha(&object, "clip_sha256")?,
        sha(&object, "appearance_delivery_manifest_object_sha256")?,
        sha(&object, "anchor_set_object_sha256")?,
        sha(&object, "anchor_set_canonical_sha256")?,
    )?;
    let materialized = materialize_socket_glb_v2(
        &context.source_glb,
        &context.parent_link.animated_artifact_sha256,
        sha(&object, "anchor_set_object_sha256")?,
        &context.anchor_set,
        &context.part_ids,
        &context.anchor_ids,
        Some((
            &context.parent_receipt.material_pack_id,
            &context.parent_receipt.material_pack_manifest_sha256,
        )),
    )?;
    let derived_sha256 = sha256_hex(&materialized.glb);
    let inspection = strict_glb_inspection(&materialized.glb)?;
    let readback = artifact_readback_v2_value(
        &derived_sha256,
        &appearance_candidate_id,
        &inspection,
        materialized.glb.len() as u64,
    );
    let derived_readback_sha256 = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("derived V2 socket ArtifactReadback hash is unavailable"))?
        .to_owned();
    let receipt = build_receipt(
        &context,
        &key,
        &request_sha256,
        sha(&object, "appearance_delivery_manifest_object_sha256")?,
        sha(&object, "anchor_set_object_sha256")?,
        sha(&object, "anchor_set_canonical_sha256")?,
        &derived_sha256,
        &derived_readback_sha256,
        &materialized,
    )?;
    let receipt_value =
        serde_json::to_value(&receipt).map_err(|error| invalid(error.to_string()))?;
    let receipt_bytes =
        canonical_json_bytes(&receipt_value).map_err(|error| invalid(error.to_string()))?;
    if receipt_bytes.len() > 1024 * 1024 {
        return Err(invalid("V2 socket receipt exceeds its JSON budget"));
    }
    let receipt_sha256 = sha256_hex(&receipt_bytes);
    let link = build_link(&context, &receipt, &receipt_sha256)?;
    let reservation = runtime.store.begin_cas_reservation();
    let derived_object = runtime.store.put_object_reserved(
        &reservation,
        &materialized.glb,
        Some(&derived_sha256),
        GLB_MIME,
        GLB_KIND,
        &receipt.created_at,
    )?;
    let receipt_object = match runtime.store.put_object_reserved(
        &reservation,
        &receipt_bytes,
        Some(&receipt_sha256),
        RECEIPT_MIME,
        RECEIPT_KIND,
        &receipt.created_at,
    ) {
        Ok(object) => object,
        Err(error) => {
            if let Err(rollback_error) =
                release_reservation_objects(runtime, &reservation, &[&derived_object], true)
            {
                return Err(invalid(format!(
                    "{error}; rollback also failed: {rollback_error}"
                )));
            }
            return Err(error.into());
        }
    };
    let stored_link = match runtime
        .store
        .record_game_weapon_animated_glb_socket_materialization_v2_link(&link)
    {
        Ok(link) => link,
        Err(error) => {
            if let Err(rollback_error) = release_reservation_objects(
                runtime,
                &reservation,
                &[&derived_object, &receipt_object],
                true,
            ) {
                return Err(invalid(format!(
                    "{error}; rollback also failed: {rollback_error}"
                )));
            }
            return Err(error.into());
        }
    };
    release_reservation_objects(
        runtime,
        &reservation,
        &[&derived_object, &receipt_object],
        false,
    )?;
    result_value(PREPARE_RESULT_SCHEMA, &stored_link, &receipt, false, true)
}

pub(super) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let (project_id, appearance_candidate_id, clip_id, key) = parse_get(value)?;
    let link = runtime
        .store
        .get_game_weapon_animated_glb_socket_materialization_v2_link(&key)?
        .ok_or_else(|| invalid("durable V2 animated socket is unavailable"))?;
    if link.project_id != project_id
        || link.appearance_candidate_id != appearance_candidate_id
        || link.clip_id != clip_id
    {
        return Err(invalid("durable V2 animated socket identity differs"));
    }
    let receipt = verify_durable(runtime, &link)?;
    result_value(GET_RESULT_SCHEMA, &link, &receipt, false, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_glb(root: &Value, binary: &[u8]) -> Vec<u8> {
        let mut json_bytes = serde_json::to_vec(root).expect("JSON");
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }
        let total = 12 + 8 + json_bytes.len() + 8 + binary.len();
        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&(total as u32).to_le_bytes());
        bytes.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"JSON");
        bytes.extend_from_slice(&json_bytes);
        bytes.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"BIN\0");
        bytes.extend_from_slice(binary);
        bytes
    }

    fn anchor_set() -> Value {
        json!({
            "schema_version":"GameWeaponAnchorSet@1",
            "node_materialization":"sidecar-only-not-glb-nodes",
            "canonical_sha256":"a".repeat(64),
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

    fn source_glb() -> (Vec<u8>, Vec<String>, Value) {
        let part_ids = ["part-a", "part-b", "part-c", "part-d", "part-e"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
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
        let manifest = "m".repeat(64);
        let root = json!({
            "asset":{"version":"2.0"},
            "scene":0,
            "scenes":[{"nodes":[0]}],
            "nodes":nodes,
            "meshes":[{"primitives":[]}],
            "materials":[{"name":"paint"}],
            "textures":[{"sampler":0,"source":0}],
            "images":[{"bufferView":0,"mimeType":"image/png"}],
            "samplers":[{"magFilter":9729,"minFilter":9729}],
            "buffers":[{"byteLength":8}],
            "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":8}],
            "accessors":[{"bufferView":0,"componentType":5126,"count":2,"type":"SCALAR"},{"bufferView":0,"componentType":5126,"count":2,"type":"SCALAR"}],
            "animations":[{"name":"ForgeCAD rigid mechanical clip","samplers":[{"input":0,"output":1,"interpolation":"LINEAR"}],"channels":[{"sampler":0,"target":{"node":0,"path":"translation"}}]}],
            "extras":{"forgecad":{"material_pack_id":"pack-1","material_pack_manifest_sha256":manifest}}
        });
        (encode_glb(&root, &[1, 2, 3, 4, 5, 6, 7, 8]), part_ids, root)
    }

    #[test]
    fn animated_socket_v2_materializer_is_byte_replayable_projection_exact_and_v2_metadata_bound() {
        let (source, part_ids, source_root) = source_glb();
        let anchors = anchor_set();
        let anchor_ids = crate::game_asset_delivery::socket_anchor_ids(&anchors).unwrap();
        let source_hash = sha256_hex(&source);
        let first = materialize_socket_glb_v2(
            &source,
            &source_hash,
            &"b".repeat(64),
            &anchors,
            &part_ids,
            &anchor_ids,
            Some(("pack-1", &"m".repeat(64))),
        )
        .unwrap();
        let replay = materialize_socket_glb_v2(
            &source,
            &source_hash,
            &"b".repeat(64),
            &anchors,
            &part_ids,
            &anchor_ids,
            Some(("pack-1", &"m".repeat(64))),
        )
        .unwrap();
        assert_eq!(first.glb, replay.glb);
        assert_eq!(first.source_node_count + 6, first.derived_node_count);
        assert!(first.source_bin_prefix_exact);
        assert!(first.static_json_projection_exact);
        assert!(first.renderable_projection_exact);
        assert!(first.animations_preserved && first.channels_preserved && first.samplers_preserved);
        assert!(first.skins_absent && first.morph_targets_absent);
        let (derived_root, derived_bin) = parse_glb(&first.glb).unwrap();
        let (_, source_bin) = parse_glb(&source).unwrap();
        assert_eq!(source_bin, derived_bin);
        let metadata = derived_root["extras"]["forgecad"][SOCKET_METADATA_KEY]
            .as_object()
            .expect("V2 socket metadata");
        assert_eq!(metadata["schema_version"], SOCKET_METADATA_SCHEMA);
        assert_eq!(metadata["materialization_policy"], POLICY);
        assert_ne!(
            metadata["schema_version"],
            "GameWeaponGlbSocketMaterialization@1"
        );
        assert_ne!(
            metadata["materialization_policy"],
            "gltf-anchor-node-materialization-preserve-renderable-content@1"
        );
        let mut metadata_preimage = Value::Object(metadata.clone());
        metadata_preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            metadata["canonical_sha256"],
            canonical_json_hash(&metadata_preimage)
        );
        assert_eq!(source_root["meshes"], derived_root["meshes"]);
        assert_eq!(source_root["materials"], derived_root["materials"]);
        assert_eq!(source_root["textures"], derived_root["textures"]);
        assert_eq!(source_root["images"], derived_root["images"]);
        assert_eq!(source_root["samplers"], derived_root["samplers"]);
        assert_eq!(source_root["animations"], derived_root["animations"]);
    }

    #[test]
    fn animated_socket_v2_materializer_rejects_skin_morph_and_tick_target_drift() {
        let (source, part_ids, _) = source_glb();
        let anchors = anchor_set();
        let anchor_ids = crate::game_asset_delivery::socket_anchor_ids(&anchors).unwrap();
        for mutation in [
            |root: &mut Value| root["skins"] = json!([]),
            |root: &mut Value| root["meshes"][0]["primitives"] = json!([{"targets":[]}]),
            |root: &mut Value| root["animations"][0]["channels"][0]["target"]["node"] = json!(99),
        ] {
            let (mut root, binary) = parse_glb(&source).unwrap();
            mutation(&mut root);
            let mutated = encode_glb(&root, &binary);
            let result = materialize_socket_glb_v2(
                &mutated,
                &sha256_hex(&mutated),
                &"b".repeat(64),
                &anchors,
                &part_ids,
                &anchor_ids,
                None,
            );
            assert!(result.is_err());
        }
    }
}
