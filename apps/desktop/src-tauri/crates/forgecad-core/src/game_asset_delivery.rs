use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    semantic_sha256, simplify_game_asset_lod_with_global_error, verify_forgecad_glb, CoreError,
    CoreResult, GameAssetLodVertex, GameAssetProfile, UniversalAssetSourceV2,
};

pub const GAME_ASSET_LOD_RECEIPT_SCHEMA_VERSION: &str = "GameAssetLodReceipt@1";
pub const GAME_ASSET_DELIVERY_BINDINGS_SCHEMA_VERSION: &str = "GameAssetDeliveryBindings@1";
pub const GAME_ASSET_DELIVERY_RECEIPT_SCHEMA_VERSION: &str = "GameAssetDeliveryReceipt@1";
const MAX_GAME_ASSET_DELIVERY_BYTES: usize = 160 * 1024 * 1024;
const MSFT_LOD_EXTENSION: &str = "MSFT_lod";

/// A local, reproducible LOD tier.  The index data is held in the GLB itself;
/// this receipt is only the evidence needed to verify it was derived from the
/// sealed LOD0 geometry and the Rust-owned delivery profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetLodLevelReadback {
    pub level: u8,
    pub triangle_count: u32,
    pub simplification_error: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetLodDeliveryReadback {
    pub schema_version: String,
    pub source_glb_sha256: String,
    pub delivery_glb_sha256: String,
    pub game_asset_profile_id: String,
    pub game_asset_profile_sha256: String,
    pub lods: [GameAssetLodLevelReadback; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameAssetLodDelivery {
    pub glb_bytes: Vec<u8>,
    pub readback: GameAssetLodDeliveryReadback,
}

/// Rust-derived bridge between the universal semantic Part IDs and the
/// feature-node IDs embedded in the immutable GLB.  Providers and the desktop
/// cannot submit this mapping: it is the only allowed source for collision and
/// socket ownership in a delivery artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetDeliveryPartBinding {
    pub subject_part_id: String,
    pub terminal_operation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetDeliveryBindings {
    pub schema_version: String,
    pub source_id: String,
    pub source_request_sha256: String,
    pub game_asset_profile_sha256: String,
    pub parts: Vec<GameAssetDeliveryPartBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetCollisionProxyReadback {
    pub subject_part_id: String,
    pub terminal_operation_id: String,
    pub mesh_index: u32,
    pub bounds_meters: [[f32; 3]; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetSocketReadback {
    pub socket_id: String,
    pub subject_part_id: String,
    pub terminal_operation_id: String,
    pub node_index: u32,
    pub pivot_meters: [f32; 3],
    pub forward: [f32; 3],
}

/// Measured from the final delivery GLB's LOD0 positions, UV0 coordinates and
/// the embedded base-color PNG dimensions. The profile target is retained only
/// as an acceptance threshold; it is never substituted for this measurement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetMaterialTexelDensityReadback {
    pub material_index: u32,
    pub base_color_dimensions_pixels: [u32; 2],
    pub surface_area_square_meters: f32,
    pub uv_area_square_units: f32,
    pub effective_texel_density_pixels_per_meter: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetTexelDensityReadback {
    pub material_zones: Vec<GameAssetMaterialTexelDensityReadback>,
    pub surface_area_square_meters: f32,
    pub effective_texel_density_pixels_per_meter: f32,
    pub target_texel_density_pixels_per_meter: u16,
    pub target_met: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetDeliveryReadback {
    pub schema_version: String,
    pub source_glb_sha256: String,
    pub delivery_glb_sha256: String,
    pub game_asset_profile_sha256: String,
    pub bindings_sha256: String,
    pub lod: GameAssetLodDeliveryReadback,
    pub collision_proxies: Vec<GameAssetCollisionProxyReadback>,
    pub sockets: Vec<GameAssetSocketReadback>,
    pub texel_density: GameAssetTexelDensityReadback,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameAssetDeliveryArtifact {
    pub glb_bytes: Vec<u8>,
    pub readback: GameAssetDeliveryReadback,
}

/// Derives the exact delivery part bindings after independently validating the
/// UAS@2 and its sealed game profile. Procedural, local lattice-deform and
/// their reviewed per-part hybrid share the same Rust-owned terminal part
/// bindings. An unavailable local mesh patch remains excluded: it cannot
/// invent an opaque collider or socket mapping.
pub fn derive_game_asset_delivery_bindings(
    source: &UniversalAssetSourceV2,
) -> CoreResult<GameAssetDeliveryBindings> {
    source.validate()?;
    let profile = source.game_asset_profile.as_ref().ok_or_else(|| {
        invalid(
            "GAME_ASSET_PROFILE_REQUIRED",
            "Game asset delivery bindings require a sealed game asset profile.",
        )
    })?;
    let procedural = source.runtime_procedural().map_err(|_| {
        invalid(
            "GAME_ASSET_DELIVERY_REPRESENTATION_UNAVAILABLE",
            "Game asset collision and socket bindings require an executable local representation.",
        )
    })?;
    let requested_parts = profile
        .collision_proxy_part_ids
        .iter()
        .chain(profile.sockets.iter().map(|socket| &socket.part_id))
        .collect::<BTreeSet<_>>();
    let bindings = procedural
        .part_bindings
        .iter()
        .map(|binding| {
            (
                binding.subject_part_id.as_str(),
                binding.terminal_operation_id.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let parts = requested_parts
        .into_iter()
        .map(|part_id| {
            let terminal_operation_id = bindings.get(part_id.as_str()).ok_or_else(|| {
                invalid(
                    "GAME_ASSET_DELIVERY_PART_BINDING_INVALID",
                    "A delivery part is absent from the sealed executable source bindings.",
                )
            })?;
            Ok(GameAssetDeliveryPartBinding {
                subject_part_id: part_id.clone(),
                terminal_operation_id: (*terminal_operation_id).to_string(),
            })
        })
        .collect::<CoreResult<Vec<_>>>()?;
    Ok(GameAssetDeliveryBindings {
        schema_version: GAME_ASSET_DELIVERY_BINDINGS_SCHEMA_VERSION.into(),
        source_id: source.source_id.clone(),
        source_request_sha256: source.request_sha256.clone(),
        game_asset_profile_sha256: semantic_sha256(profile)?,
        parts,
    })
}

/// Adds collision and socket artifacts after LOD compilation. Collision meshes
/// deliberately do not enter the default glTF scene: a game loader must opt in
/// through the signed root receipt, so a generic preview never renders proxy
/// boxes as the authored appearance asset.
pub fn compile_game_asset_delivery(
    source_glb: &[u8],
    profile: &GameAssetProfile,
    bindings: &GameAssetDeliveryBindings,
) -> CoreResult<GameAssetDeliveryArtifact> {
    validate_delivery_bindings(profile, bindings)?;
    let lod_delivery = compile_game_asset_lod_delivery(source_glb, profile)?;
    let (mut document, mut binary) = parse_glb(&lod_delivery.glb_bytes)?;
    let source_primitives = document["meshes"][0]
        .get("primitives")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_SOURCE_INVALID",
                "LOD0 primitives are missing.",
            )
        })?
        .clone();
    let texel_density = measure_texel_density(&document, &binary, profile)?;
    let binding_by_part = bindings
        .parts
        .iter()
        .map(|binding| (binding.subject_part_id.as_str(), binding))
        .collect::<BTreeMap<_, _>>();
    let mut collision_proxies = Vec::new();
    for part_id in &profile.collision_proxy_part_ids {
        let binding = binding_by_part.get(part_id.as_str()).ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_PART_BINDING_INVALID",
                "Collision part has no sealed binding.",
            )
        })?;
        let bounds = operation_bounds(
            &source_primitives,
            &document,
            &binary,
            &binding.terminal_operation_id,
        )?;
        let mesh_index = append_collision_mesh(
            &mut document,
            &mut binary,
            part_id,
            &binding.terminal_operation_id,
            bounds,
        )?;
        collision_proxies.push(GameAssetCollisionProxyReadback {
            subject_part_id: part_id.clone(),
            terminal_operation_id: binding.terminal_operation_id.clone(),
            mesh_index,
            bounds_meters: bounds,
        });
    }
    let mut sockets = Vec::new();
    for socket in &profile.sockets {
        let binding = binding_by_part
            .get(socket.part_id.as_str())
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_DELIVERY_PART_BINDING_INVALID",
                    "Socket part has no sealed binding.",
                )
            })?;
        let node_index = append_socket_node(&mut document, socket, binding)?;
        sockets.push(GameAssetSocketReadback {
            socket_id: socket.socket_id.clone(),
            subject_part_id: socket.part_id.clone(),
            terminal_operation_id: binding.terminal_operation_id.clone(),
            node_index,
            pivot_meters: socket.pivot_meters,
            forward: normalize_forward(socket.forward)?,
        });
    }
    let receipt_unsigned = json!({
        "schema_version": GAME_ASSET_DELIVERY_RECEIPT_SCHEMA_VERSION,
        "source_glb_sha256": lod_delivery.readback.source_glb_sha256,
        "game_asset_profile_sha256": semantic_sha256(profile)?,
        "bindings_sha256": semantic_sha256(bindings)?,
        "collision_proxies": collision_proxies,
        "sockets": sockets,
        "texel_density": texel_density,
    });
    let receipt_sha256 = semantic_sha256(&receipt_unsigned)?;
    document
        .get_mut("extras")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_SOURCE_INVALID",
                "GLB root extras are invalid.",
            )
        })?
        .insert(
            "forgecad_game_asset_delivery_receipt".into(),
            json!({"receipt": receipt_unsigned, "receipt_sha256": receipt_sha256}),
        );
    document["buffers"][0]["byteLength"] = json!(binary.len());
    let glb_bytes = encode_glb(&document, binary)?;
    let readback = verify_game_asset_delivery_glb(source_glb, &glb_bytes, profile, bindings)?;
    Ok(GameAssetDeliveryArtifact {
        glb_bytes,
        readback,
    })
}

/// Full delivery verification deliberately takes the original sealed GLB and
/// bindings as inputs. A detached GLB cannot prove which UAS/Part graph its
/// collision proxies belonged to, so accepting it without those two lineage
/// inputs would create a second asset truth.
pub fn verify_game_asset_delivery_glb(
    source_glb: &[u8],
    delivery_glb: &[u8],
    profile: &GameAssetProfile,
    bindings: &GameAssetDeliveryBindings,
) -> CoreResult<GameAssetDeliveryReadback> {
    validate_delivery_bindings(profile, bindings)?;
    let (document, binary) = parse_glb(delivery_glb)?;
    let lod_glb = lod_only_glb(&document, &binary)?;
    let lod = verify_game_asset_lod_delivery_glb(source_glb, &lod_glb, profile)?;
    let receipt = document
        .pointer("/extras/forgecad_game_asset_delivery_receipt")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_RECEIPT_MISSING",
                "Game delivery receipt is missing.",
            )
        })?;
    let unsigned = receipt.get("receipt").cloned().ok_or_else(|| {
        invalid(
            "GAME_ASSET_DELIVERY_RECEIPT_INVALID",
            "Game delivery receipt is invalid.",
        )
    })?;
    let receipt_sha256 = receipt
        .get("receipt_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_RECEIPT_INVALID",
                "Game delivery receipt hash is invalid.",
            )
        })?;
    if semantic_sha256(&unsigned)? != receipt_sha256 {
        return Err(invalid(
            "GAME_ASSET_DELIVERY_RECEIPT_INVALID",
            "Game delivery receipt semantic hash is invalid.",
        ));
    }
    let parsed = parse_delivery_receipt(&unsigned)?;
    if parsed.schema_version != GAME_ASSET_DELIVERY_RECEIPT_SCHEMA_VERSION
        || parsed.source_glb_sha256 != lod.source_glb_sha256
        || parsed.game_asset_profile_sha256 != semantic_sha256(profile)?
        || parsed.bindings_sha256 != semantic_sha256(bindings)?
    {
        return Err(invalid(
            "GAME_ASSET_DELIVERY_RECEIPT_INVALID",
            "Game delivery receipt lineage drifted.",
        ));
    }
    verify_collision_proxies(
        &document,
        &binary,
        profile,
        bindings,
        &parsed.collision_proxies,
    )?;
    verify_socket_nodes(&document, profile, bindings, &parsed.sockets)?;
    let expected_texel_density = measure_texel_density(&document, &binary, profile)?;
    if !texel_density_matches(&parsed.texel_density, &expected_texel_density) {
        return Err(invalid(
            "GAME_ASSET_TEXEL_DENSITY_INVALID",
            "Game delivery texel-density readback does not match the final LOD0 geometry and embedded PBR textures.",
        ));
    }
    Ok(GameAssetDeliveryReadback {
        schema_version: GAME_ASSET_DELIVERY_RECEIPT_SCHEMA_VERSION.into(),
        source_glb_sha256: lod.source_glb_sha256.clone(),
        delivery_glb_sha256: hex_sha256(delivery_glb),
        game_asset_profile_sha256: semantic_sha256(profile)?,
        bindings_sha256: semantic_sha256(bindings)?,
        lod,
        collision_proxies: parsed.collision_proxies,
        sockets: parsed.sockets,
        texel_density: expected_texel_density,
    })
}

/// Compiles `production_concept` LOD0 into a separate game-delivery GLB with
/// genuine, locally simplified LOD1/LOD2 index buffers.  This function does
/// not claim collision, socket, UV-density or export readiness; those are
/// separately sealed P4 delivery obligations.
pub fn compile_game_asset_lod_delivery(
    source_glb: &[u8],
    profile: &GameAssetProfile,
) -> CoreResult<GameAssetLodDelivery> {
    profile.validate()?;
    let source_readback = verify_forgecad_glb(source_glb, Some("production_concept"))?;
    let (mut document, mut binary) = parse_glb(source_glb)?;
    validate_lod0_container(&document)?;
    let source_triangle_count = source_readback.triangle_count as u32;
    if source_triangle_count > profile.lod_triangle_budgets[0] {
        return Err(invalid(
            "GAME_ASSET_LOD0_BUDGET_EXCEEDED",
            "Production GLB exceeds the Rust-owned LOD0 triangle budget.",
        ));
    }
    if source_triangle_count <= profile.lod_triangle_budgets[1]
        || profile.lod_triangle_budgets[1] <= profile.lod_triangle_budgets[2]
    {
        return Err(invalid(
            "GAME_ASSET_LOD_BUDGET_NOT_REDUCING",
            "Game asset LOD1 and LOD2 budgets must require real reduction from LOD0.",
        ));
    }

    let source_mesh = document
        .get("meshes")
        .and_then(Value::as_array)
        .and_then(|meshes| meshes.first())
        .cloned()
        .ok_or_else(|| invalid("GAME_ASSET_LOD_SOURCE_INVALID", "GLB LOD0 mesh is missing."))?;
    let source_primitives = source_mesh
        .get("primitives")
        .and_then(Value::as_array)
        .filter(|primitives| !primitives.is_empty())
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD0 has no triangle primitives.",
            )
        })?;
    let primitive_inputs = source_primitives
        .iter()
        .map(|primitive| primitive_input(primitive, &document, &binary))
        .collect::<CoreResult<Vec<_>>>()?;
    let source_counts = primitive_inputs
        .iter()
        .map(|input| input.indices.len() as u32 / 3)
        .collect::<Vec<_>>();
    if source_counts.iter().copied().sum::<u32>() != source_triangle_count {
        return Err(invalid(
            "GAME_ASSET_LOD_SOURCE_INVALID",
            "GLB LOD0 triangles disagree with strict ForgeCAD readback.",
        ));
    }
    let global_extent = global_extent(&primitive_inputs)?;

    let lod1_targets = distribute_triangle_budget(&source_counts, profile.lod_triangle_budgets[1])?;
    let lod2_targets = distribute_triangle_budget(&source_counts, profile.lod_triangle_budgets[2])?;
    let lod1 = simplify_tier(&primitive_inputs, &lod1_targets, global_extent)?;
    let lod2 = simplify_tier(&primitive_inputs, &lod2_targets, global_extent)?;
    if tier_triangle_count(&lod1) > profile.lod_triangle_budgets[1]
        || tier_triangle_count(&lod2) > profile.lod_triangle_budgets[2]
    {
        return Err(invalid(
            "GAME_ASSET_LOD_QUALITY_BUDGET_CONFLICT",
            "The requested LOD budget cannot be reached without exceeding the geometry quality error bound.",
        ));
    }

    let declared_binary_length = document
        .pointer("/buffers/0/byteLength")
        .and_then(Value::as_u64)
        .filter(|length| *length as usize <= binary.len())
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB binary buffer is invalid.",
            )
        })? as usize;
    binary.truncate(declared_binary_length);
    let lod1_mesh = build_lod_mesh(&source_mesh, &lod1, &mut document, &mut binary, "LOD1")?;
    let lod2_mesh = build_lod_mesh(&source_mesh, &lod2, &mut document, &mut binary, "LOD2")?;
    document["buffers"][0]["byteLength"] = json!(binary.len());
    let meshes = document
        .get_mut("meshes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB mesh table is invalid.",
            )
        })?;
    meshes.push(lod1_mesh);
    meshes.push(lod2_mesh);

    let profile_sha256 = semantic_sha256(profile)?;
    let receipt_unsigned = json!({
        "schema_version": GAME_ASSET_LOD_RECEIPT_SCHEMA_VERSION,
        "source_glb_sha256": source_readback.glb_sha256,
        "game_asset_profile_id": profile.profile_id,
        "game_asset_profile_sha256": profile_sha256,
        "lods": [
            tier_receipt(0, source_triangle_count, 0.0),
            tier_receipt(1, tier_triangle_count(&lod1), tier_max_error(&lod1)),
            tier_receipt(2, tier_triangle_count(&lod2), tier_max_error(&lod2)),
        ],
    });
    let receipt_sha256 = semantic_sha256(&receipt_unsigned)?;
    let root = document
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .and_then(|nodes| nodes.first_mut())
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("GAME_ASSET_LOD_SOURCE_INVALID", "GLB root node is invalid."))?;
    root.insert(
        "extensions".into(),
        json!({MSFT_LOD_EXTENSION: {"ids": [1, 2]}}),
    );
    root.entry("extras")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB root extras are invalid.",
            )
        })?
        .insert(
            "forgecad_game_asset_lod_receipt".into(),
            json!({"receipt": receipt_unsigned, "receipt_sha256": receipt_sha256}),
        );
    let nodes = document
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB node table is invalid.",
            )
        })?;
    nodes.push(json!({"name": "FORGECAD_BLOCKOUT_LOD1", "mesh": 1}));
    nodes.push(json!({"name": "FORGECAD_BLOCKOUT_LOD2", "mesh": 2}));
    let extensions_used = document
        .as_object_mut()
        .expect("JSON document is an object")
        .entry("extensionsUsed")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB extensionsUsed is invalid.",
            )
        })?;
    if !extensions_used
        .iter()
        .any(|value| value.as_str() == Some(MSFT_LOD_EXTENSION))
    {
        extensions_used.push(json!(MSFT_LOD_EXTENSION));
    }

    let glb_bytes = encode_glb(&document, binary)?;
    let readback = verify_game_asset_lod_delivery_glb(source_glb, &glb_bytes, profile)?;
    Ok(GameAssetLodDelivery {
        glb_bytes,
        readback,
    })
}

/// Recomputes all LOD topology from LOD0 and rejects a GLB if its `MSFT_lod`
/// declaration, receipt, material bindings, primitive ordering, or bounded
/// simplification result drifts from the Core-owned compiler.
pub fn verify_game_asset_lod_delivery_glb(
    source_glb: &[u8],
    glb_bytes: &[u8],
    profile: &GameAssetProfile,
) -> CoreResult<GameAssetLodDeliveryReadback> {
    profile.validate()?;
    let source_readback = verify_forgecad_glb(source_glb, Some("production_concept"))?;
    let (source_document, source_binary) = parse_glb(source_glb)?;
    validate_lod0_container(&source_document)?;
    let (document, binary) = parse_glb(glb_bytes)?;
    validate_delivery_container(&document)?;
    let receipt = document
        .pointer("/nodes/0/extras/forgecad_game_asset_lod_receipt")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_RECEIPT_MISSING",
                "Game asset LOD receipt is missing.",
            )
        })?;
    let receipt_unsigned = receipt.get("receipt").cloned().ok_or_else(|| {
        invalid(
            "GAME_ASSET_LOD_RECEIPT_INVALID",
            "Game asset LOD receipt is invalid.",
        )
    })?;
    let receipt_sha256 = receipt
        .get("receipt_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_RECEIPT_INVALID",
                "Game asset LOD receipt hash is invalid.",
            )
        })?;
    if semantic_sha256(&receipt_unsigned)? != receipt_sha256 {
        return Err(invalid(
            "GAME_ASSET_LOD_RECEIPT_INVALID",
            "Game asset LOD receipt semantic hash does not match its payload.",
        ));
    }
    if document
        .get("meshes")
        .and_then(Value::as_array)
        .and_then(|meshes| meshes.first())
        != source_document
            .get("meshes")
            .and_then(Value::as_array)
            .and_then(|meshes| meshes.first())
        || binary.get(..source_binary.len()) != Some(source_binary.as_slice())
    {
        return Err(invalid(
            "GAME_ASSET_LOD_SOURCE_LINEAGE_INVALID",
            "Game asset LOD0 mesh or binary prefix drifted from the sealed source GLB.",
        ));
    }
    let profile_sha256 = semantic_sha256(profile)?;
    let parsed_receipt = parse_receipt(&receipt_unsigned)?;
    if parsed_receipt.schema_version != GAME_ASSET_LOD_RECEIPT_SCHEMA_VERSION
        || parsed_receipt.source_glb_sha256 != source_readback.glb_sha256
        || parsed_receipt.game_asset_profile_id != profile.profile_id
        || parsed_receipt.game_asset_profile_sha256 != profile_sha256
        || parsed_receipt.lods[0].level != 0
        || parsed_receipt.lods[0].triangle_count != source_readback.triangle_count as u32
        || parsed_receipt.lods[0].simplification_error != 0.0
    {
        return Err(invalid(
            "GAME_ASSET_LOD_RECEIPT_INVALID",
            "Game asset LOD receipt does not bind the current source and profile.",
        ));
    }
    if source_readback.triangle_count as u32 > profile.lod_triangle_budgets[0] {
        return Err(invalid(
            "GAME_ASSET_LOD0_BUDGET_EXCEEDED",
            "Game asset LOD0 exceeds its delivery profile budget.",
        ));
    }
    let base_primitives = document["meshes"][0]
        .get("primitives")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD0 primitives are invalid.",
            )
        })?;
    let primitive_inputs = base_primitives
        .iter()
        .map(|primitive| primitive_input(primitive, &document, &binary))
        .collect::<CoreResult<Vec<_>>>()?;
    let source_counts = primitive_inputs
        .iter()
        .map(|input| input.indices.len() as u32 / 3)
        .collect::<Vec<_>>();
    let global_extent = global_extent(&primitive_inputs)?;
    for (tier_index, (level, budget)) in [
        (1_u8, profile.lod_triangle_budgets[1]),
        (2, profile.lod_triangle_budgets[2]),
    ]
    .into_iter()
    .enumerate()
    {
        let mesh = &document["meshes"][tier_index + 1];
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .filter(|items| items.len() == base_primitives.len())
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_LOD_TOPOLOGY_INVALID",
                    "LOD primitive layout drifted.",
                )
            })?;
        let targets = distribute_triangle_budget(&source_counts, budget)?;
        let expected = simplify_tier(&primitive_inputs, &targets, global_extent)?;
        let mut triangle_count = 0u32;
        let mut maximum_error = 0.0_f32;
        for ((base, lod), expected_lod) in base_primitives.iter().zip(primitives).zip(&expected) {
            if base.get("attributes") != lod.get("attributes")
                || base.get("material") != lod.get("material")
                || base.get("mode") != lod.get("mode")
                || base.get("extras") != lod.get("extras")
            {
                return Err(invalid(
                    "GAME_ASSET_LOD_TOPOLOGY_INVALID",
                    "LOD changed a material or source binding.",
                ));
            }
            let actual_indices = primitive_indices(lod, &document, &binary)?;
            if actual_indices != expected_lod.indices {
                return Err(invalid(
                    "GAME_ASSET_LOD_TOPOLOGY_INVALID",
                    "LOD index topology is not the deterministic Core result.",
                ));
            }
            triangle_count += (actual_indices.len() / 3) as u32;
            maximum_error = maximum_error.max(expected_lod.simplification_error);
        }
        let reported = &parsed_receipt.lods[level as usize];
        if reported.level != level
            || reported.triangle_count != triangle_count
            || reported.triangle_count > budget
            || reported.triangle_count >= source_readback.triangle_count as u32
            || (reported.simplification_error - maximum_error).abs() > f32::EPSILON
        {
            return Err(invalid(
                "GAME_ASSET_LOD_RECEIPT_INVALID",
                "LOD receipt does not match actual delivery geometry.",
            ));
        }
    }
    Ok(GameAssetLodDeliveryReadback {
        schema_version: GAME_ASSET_LOD_RECEIPT_SCHEMA_VERSION.into(),
        source_glb_sha256: source_readback.glb_sha256,
        delivery_glb_sha256: hex_sha256(glb_bytes),
        game_asset_profile_id: profile.profile_id.clone(),
        game_asset_profile_sha256: profile_sha256,
        lods: parsed_receipt.lods,
    })
}

fn validate_delivery_bindings(
    profile: &GameAssetProfile,
    bindings: &GameAssetDeliveryBindings,
) -> CoreResult<()> {
    profile.validate()?;
    if bindings.schema_version != GAME_ASSET_DELIVERY_BINDINGS_SCHEMA_VERSION
        || bindings.source_id.is_empty()
        || !is_sha256(&bindings.source_request_sha256)
        || bindings.game_asset_profile_sha256 != semantic_sha256(profile)?
    {
        return Err(invalid(
            "GAME_ASSET_DELIVERY_BINDINGS_INVALID",
            "Game delivery bindings do not match the sealed profile lineage.",
        ));
    }
    let expected = profile
        .collision_proxy_part_ids
        .iter()
        .chain(profile.sockets.iter().map(|socket| &socket.part_id))
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let actual = bindings
        .parts
        .iter()
        .map(|binding| binding.subject_part_id.as_str())
        .collect::<BTreeSet<_>>();
    if expected != actual
        || bindings.parts.iter().any(|binding| {
            binding.subject_part_id.is_empty() || binding.terminal_operation_id.is_empty()
        })
    {
        return Err(invalid(
            "GAME_ASSET_DELIVERY_BINDINGS_INVALID",
            "Game delivery bindings must cover each and only each selected profile part.",
        ));
    }
    Ok(())
}

fn operation_bounds(
    primitives: &[Value],
    document: &Value,
    binary: &[u8],
    operation_id: &str,
) -> CoreResult<[[f32; 3]; 2]> {
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    let mut found = false;
    for primitive in primitives {
        if primitive
            .pointer("/extras/forgecad_feature_node_id")
            .and_then(Value::as_str)
            != Some(operation_id)
        {
            continue;
        }
        let attributes = primitive
            .get("attributes")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_DELIVERY_SOURCE_INVALID",
                    "GLB primitive attributes are invalid.",
                )
            })?;
        let positions = read_float_vec_accessor(
            attribute_accessor(attributes, "POSITION", document)?,
            document,
            binary,
            3,
        )?;
        for position in positions {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(position[axis]);
                maximum[axis] = maximum[axis].max(position[axis]);
            }
        }
        found = true;
    }
    if !found
        || minimum
            .iter()
            .chain(maximum.iter())
            .any(|value| !value.is_finite())
        || (0..3).any(|axis| minimum[axis] > maximum[axis])
    {
        return Err(invalid(
            "GAME_ASSET_DELIVERY_OPERATION_UNRESOLVED",
            "A delivery binding has no finite LOD0 GLB surface provenance.",
        ));
    }
    Ok([minimum, maximum])
}

fn append_collision_mesh(
    document: &mut Value,
    binary: &mut Vec<u8>,
    part_id: &str,
    operation_id: &str,
    bounds: [[f32; 3]; 2],
) -> CoreResult<u32> {
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let position_offset = binary.len();
    for point in aabb_vertices(bounds) {
        for value in point {
            binary.extend_from_slice(&value.to_le_bytes());
        }
    }
    let position_length = binary.len() - position_offset;
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let index_offset = binary.len();
    for index in aabb_indices() {
        binary.extend_from_slice(&index.to_le_bytes());
    }
    let index_length = binary.len() - index_offset;
    let views = document
        .get_mut("bufferViews")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_SOURCE_INVALID",
                "GLB buffer views are invalid.",
            )
        })?;
    let position_view = views.len() as u32;
    views.push(json!({"buffer": 0, "byteOffset": position_offset, "byteLength": position_length, "target": 34962}));
    let index_view = views.len() as u32;
    views.push(json!({"buffer": 0, "byteOffset": index_offset, "byteLength": index_length, "target": 34963}));
    let accessors = document
        .get_mut("accessors")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_SOURCE_INVALID",
                "GLB accessors are invalid.",
            )
        })?;
    let position_accessor = accessors.len() as u32;
    accessors.push(json!({
        "bufferView": position_view, "componentType": 5126, "count": 8, "type": "VEC3",
        "min": bounds[0], "max": bounds[1]
    }));
    let index_accessor = accessors.len() as u32;
    accessors.push(
        json!({"bufferView": index_view, "componentType": 5123, "count": 36, "type": "SCALAR"}),
    );
    let meshes = document
        .get_mut("meshes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_SOURCE_INVALID",
                "GLB meshes are invalid.",
            )
        })?;
    let mesh_index = meshes.len() as u32;
    meshes.push(json!({
        "name": format!("FORGECAD_COLLISION_{part_id}"),
        "primitives": [{
            "attributes": {"POSITION": position_accessor}, "indices": index_accessor, "mode": 4,
            "extras": {"forgecad_game_asset_collision_proxy": {
                "schema_version": "ForgeCadCollisionProxy@1", "subject_part_id": part_id,
                "terminal_operation_id": operation_id, "bounds_meters": bounds
            }}
        }],
        "extras": {"forgecad_game_asset_node_kind": "collision_proxy"}
    }));
    Ok(mesh_index)
}

fn append_socket_node(
    document: &mut Value,
    socket: &crate::GameAssetSocket,
    binding: &GameAssetDeliveryPartBinding,
) -> CoreResult<u32> {
    let forward = normalize_forward(socket.forward)?;
    let nodes = document
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_SOURCE_INVALID",
                "GLB nodes are invalid.",
            )
        })?;
    let node_index = nodes.len() as u32;
    nodes.push(json!({
        "name": format!("FORGECAD_SOCKET_{}", socket.socket_id),
        "translation": socket.pivot_meters,
        "rotation": z_axis_rotation(forward),
        "extras": {"forgecad_game_asset_socket": {
            "schema_version": "ForgeCadSocket@1", "socket_id": socket.socket_id,
            "subject_part_id": socket.part_id, "terminal_operation_id": binding.terminal_operation_id,
            "forward": forward
        }}
    }));
    Ok(node_index)
}

fn aabb_vertices(bounds: [[f32; 3]; 2]) -> [[f32; 3]; 8] {
    let [minimum, maximum] = bounds;
    [
        [minimum[0], minimum[1], minimum[2]],
        [maximum[0], minimum[1], minimum[2]],
        [maximum[0], maximum[1], minimum[2]],
        [minimum[0], maximum[1], minimum[2]],
        [minimum[0], minimum[1], maximum[2]],
        [maximum[0], minimum[1], maximum[2]],
        [maximum[0], maximum[1], maximum[2]],
        [minimum[0], maximum[1], maximum[2]],
    ]
}

fn aabb_indices() -> [u16; 36] {
    [
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6,
        3, 0, 4, 3, 4, 7,
    ]
}

fn normalize_forward(forward: [f32; 3]) -> CoreResult<[f32; 3]> {
    let length = forward
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if !length.is_finite() || length <= f32::EPSILON {
        return Err(invalid(
            "GAME_ASSET_SOCKET_INVALID",
            "Socket forward vector is invalid.",
        ));
    }
    Ok([
        forward[0] / length,
        forward[1] / length,
        forward[2] / length,
    ])
}

fn z_axis_rotation(forward: [f32; 3]) -> [f32; 4] {
    let [x, y, z] = forward;
    if z > 1.0 - 1e-6 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    if z < -1.0 + 1e-6 {
        return [0.0, 1.0, 0.0, 0.0];
    }
    let scale = ((1.0 + z) * 2.0).sqrt();
    [-y / scale, x / scale, 0.0, scale * 0.5]
}

fn lod_only_glb(document: &Value, binary: &[u8]) -> CoreResult<Vec<u8>> {
    let mut lod_document = document.clone();
    let meshes = lod_document
        .get_mut("meshes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_CONTAINER_INVALID",
                "Game delivery meshes are invalid.",
            )
        })?;
    if meshes.len() < 3 {
        return Err(invalid(
            "GAME_ASSET_DELIVERY_CONTAINER_INVALID",
            "Game delivery LOD meshes are missing.",
        ));
    }
    meshes.truncate(3);
    let nodes = lod_document
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_CONTAINER_INVALID",
                "Game delivery nodes are invalid.",
            )
        })?;
    if nodes.len() < 3 {
        return Err(invalid(
            "GAME_ASSET_DELIVERY_CONTAINER_INVALID",
            "Game delivery LOD nodes are missing.",
        ));
    }
    nodes.truncate(3);
    lod_document
        .get_mut("extras")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_DELIVERY_CONTAINER_INVALID",
                "Game delivery extras are invalid.",
            )
        })?
        .remove("forgecad_game_asset_delivery_receipt");
    encode_glb(&lod_document, binary.to_vec())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GameAssetDeliveryReceipt {
    schema_version: String,
    source_glb_sha256: String,
    game_asset_profile_sha256: String,
    bindings_sha256: String,
    collision_proxies: Vec<GameAssetCollisionProxyReadback>,
    sockets: Vec<GameAssetSocketReadback>,
    texel_density: GameAssetTexelDensityReadback,
}

fn parse_delivery_receipt(value: &Value) -> CoreResult<GameAssetDeliveryReceipt> {
    serde_json::from_value(value.clone()).map_err(|_| {
        invalid(
            "GAME_ASSET_DELIVERY_RECEIPT_INVALID",
            "Game delivery receipt shape is invalid.",
        )
    })
}

#[derive(Default)]
struct TexelDensityAccumulator {
    dimensions: [u32; 2],
    surface_area_square_meters: f64,
    uv_area_square_units: f64,
    weighted_texel_area_square_pixels: f64,
}

/// Measures UV coverage from actual LOD0 triangles. A source model can reuse
/// or tile UVs, so this is an *effective* density rather than an assertion
/// that every texel is unique. Overlap/waste analysis remains a separate UV
/// quality gate and must not be inferred from this scalar alone.
fn measure_texel_density(
    document: &Value,
    binary: &[u8],
    profile: &GameAssetProfile,
) -> CoreResult<GameAssetTexelDensityReadback> {
    let primitives = document
        .pointer("/meshes/0/primitives")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_TEXEL_DENSITY_INVALID",
                "Game delivery LOD0 primitives are missing.",
            )
        })?;
    let mut zones = BTreeMap::<u32, TexelDensityAccumulator>::new();
    for primitive in primitives {
        let material_index = primitive
            .get("material")
            .and_then(Value::as_u64)
            .filter(|index| *index <= u32::MAX as u64)
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_TEXEL_DENSITY_INVALID",
                    "Game delivery LOD0 primitive has no material binding.",
                )
            })? as u32;
        let dimensions = base_color_dimensions(document, binary, material_index)?;
        let input = primitive_input(primitive, document, binary)?;
        if input.indices.len() % 3 != 0 {
            return Err(invalid(
                "GAME_ASSET_TEXEL_DENSITY_INVALID",
                "Game delivery LOD0 indices are not a triangle list.",
            ));
        }
        let zone = zones
            .entry(material_index)
            .or_insert_with(|| TexelDensityAccumulator {
                dimensions,
                ..TexelDensityAccumulator::default()
            });
        if zone.dimensions != dimensions {
            return Err(invalid(
                "GAME_ASSET_TEXEL_DENSITY_INVALID",
                "A game delivery material resolved to inconsistent base-color dimensions.",
            ));
        }
        for triangle in input.indices.chunks_exact(3) {
            let vertices = triangle
                .iter()
                .map(|index| input.vertices.get(*index as usize))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    invalid(
                        "GAME_ASSET_TEXEL_DENSITY_INVALID",
                        "Game delivery triangle index is outside its LOD0 vertex stream.",
                    )
                })?;
            let surface_area = triangle_surface_area(
                vertices[0].position,
                vertices[1].position,
                vertices[2].position,
            );
            let uv_area = triangle_uv_area(vertices[0].uv0, vertices[1].uv0, vertices[2].uv0);
            if !surface_area.is_finite()
                || !uv_area.is_finite()
                || surface_area <= 1e-12
                || uv_area <= 1e-12
            {
                return Err(invalid(
                    "GAME_ASSET_TEXEL_DENSITY_INVALID",
                    "Game delivery LOD0 contains a degenerate world-space or UV triangle.",
                ));
            }
            zone.surface_area_square_meters += surface_area;
            zone.uv_area_square_units += uv_area;
            zone.weighted_texel_area_square_pixels +=
                uv_area * f64::from(dimensions[0]) * f64::from(dimensions[1]);
        }
    }
    if zones.is_empty() {
        return Err(invalid(
            "GAME_ASSET_TEXEL_DENSITY_INVALID",
            "Game delivery has no LOD0 material zones to measure.",
        ));
    }
    let mut total_surface = 0.0_f64;
    let mut total_texel_area = 0.0_f64;
    let material_zones = zones
        .into_iter()
        .map(|(material_index, zone)| {
            let density = effective_texel_density(
                zone.weighted_texel_area_square_pixels,
                zone.surface_area_square_meters,
            )?;
            total_surface += zone.surface_area_square_meters;
            total_texel_area += zone.weighted_texel_area_square_pixels;
            Ok(GameAssetMaterialTexelDensityReadback {
                material_index,
                base_color_dimensions_pixels: zone.dimensions,
                surface_area_square_meters: finite_f32(zone.surface_area_square_meters)?,
                uv_area_square_units: finite_f32(zone.uv_area_square_units)?,
                effective_texel_density_pixels_per_meter: finite_f32(density)?,
            })
        })
        .collect::<CoreResult<Vec<_>>>()?;
    let effective = effective_texel_density(total_texel_area, total_surface)?;
    let effective = finite_f32(effective)?;
    Ok(GameAssetTexelDensityReadback {
        material_zones,
        surface_area_square_meters: finite_f32(total_surface)?,
        effective_texel_density_pixels_per_meter: effective,
        target_texel_density_pixels_per_meter: profile.target_texel_density_pixels_per_meter,
        target_met: effective >= f32::from(profile.target_texel_density_pixels_per_meter),
    })
}

fn base_color_dimensions(
    document: &Value,
    binary: &[u8],
    material_index: u32,
) -> CoreResult<[u32; 2]> {
    let texture_index = document
        .get("materials")
        .and_then(Value::as_array)
        .and_then(|materials| materials.get(material_index as usize))
        .and_then(|material| material.pointer("/pbrMetallicRoughness/baseColorTexture/index"))
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_TEXEL_DENSITY_INVALID",
                "Game delivery material has no embedded base-color texture.",
            )
        })? as usize;
    let image_index = document
        .get("textures")
        .and_then(Value::as_array)
        .and_then(|textures| textures.get(texture_index))
        .and_then(|texture| texture.get("source"))
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_TEXEL_DENSITY_INVALID",
                "Game delivery base-color texture has no embedded image source.",
            )
        })? as usize;
    let image = document
        .get("images")
        .and_then(Value::as_array)
        .and_then(|images| images.get(image_index))
        .filter(|image| image.get("mimeType").and_then(Value::as_str) == Some("image/png"))
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_TEXEL_DENSITY_INVALID",
                "Game delivery base-color image is not an embedded PNG.",
            )
        })?;
    let view_index = image
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_TEXEL_DENSITY_INVALID",
                "Game delivery base-color image buffer view is missing.",
            )
        })? as usize;
    let view = document
        .get("bufferViews")
        .and_then(Value::as_array)
        .and_then(|views| views.get(view_index))
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_TEXEL_DENSITY_INVALID",
                "Game delivery base-color image buffer view is invalid.",
            )
        })?;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .filter(|length| *length >= 24)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_TEXEL_DENSITY_INVALID",
                "Game delivery base-color image byte range is invalid.",
            )
        })? as usize;
    let end = offset.checked_add(length).ok_or_else(|| {
        invalid(
            "GAME_ASSET_TEXEL_DENSITY_INVALID",
            "Game delivery base-color image range overflowed.",
        )
    })?;
    let payload = binary.get(offset..end).ok_or_else(|| {
        invalid(
            "GAME_ASSET_TEXEL_DENSITY_INVALID",
            "Game delivery base-color image data is truncated.",
        )
    })?;
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if payload.get(..8) != Some(PNG_SIGNATURE) || payload.get(12..16) != Some(b"IHDR") {
        return Err(invalid(
            "GAME_ASSET_TEXEL_DENSITY_INVALID",
            "Game delivery base-color image has no valid PNG IHDR header.",
        ));
    }
    let width = u32::from_be_bytes(payload[16..20].try_into().expect("validated PNG width"));
    let height = u32::from_be_bytes(payload[20..24].try_into().expect("validated PNG height"));
    if width == 0 || height == 0 {
        return Err(invalid(
            "GAME_ASSET_TEXEL_DENSITY_INVALID",
            "Game delivery base-color image dimensions are invalid.",
        ));
    }
    Ok([width, height])
}

fn triangle_surface_area(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f64 {
    let ab = [
        f64::from(b[0] - a[0]),
        f64::from(b[1] - a[1]),
        f64::from(b[2] - a[2]),
    ];
    let ac = [
        f64::from(c[0] - a[0]),
        f64::from(c[1] - a[1]),
        f64::from(c[2] - a[2]),
    ];
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    0.5 * (cross
        .iter()
        .map(|component| component * component)
        .sum::<f64>())
    .sqrt()
}

fn triangle_uv_area(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f64 {
    let ab = [f64::from(b[0] - a[0]), f64::from(b[1] - a[1])];
    let ac = [f64::from(c[0] - a[0]), f64::from(c[1] - a[1])];
    0.5 * (ab[0] * ac[1] - ab[1] * ac[0]).abs()
}

fn effective_texel_density(texel_area: f64, surface_area: f64) -> CoreResult<f64> {
    if !texel_area.is_finite()
        || !surface_area.is_finite()
        || texel_area <= 0.0
        || surface_area <= 0.0
    {
        return Err(invalid(
            "GAME_ASSET_TEXEL_DENSITY_INVALID",
            "Game delivery texel-density inputs are not finite positive areas.",
        ));
    }
    Ok((texel_area / surface_area).sqrt())
}

fn finite_f32(value: f64) -> CoreResult<f32> {
    let value = value as f32;
    if !value.is_finite() || value <= 0.0 {
        return Err(invalid(
            "GAME_ASSET_TEXEL_DENSITY_INVALID",
            "Game delivery texel-density output is not finite.",
        ));
    }
    Ok(value)
}

fn texel_density_matches(
    actual: &GameAssetTexelDensityReadback,
    expected: &GameAssetTexelDensityReadback,
) -> bool {
    actual.target_texel_density_pixels_per_meter == expected.target_texel_density_pixels_per_meter
        && actual.target_met == expected.target_met
        && approximately_equal(
            actual.surface_area_square_meters,
            expected.surface_area_square_meters,
        )
        && approximately_equal(
            actual.effective_texel_density_pixels_per_meter,
            expected.effective_texel_density_pixels_per_meter,
        )
        && actual.material_zones.len() == expected.material_zones.len()
        && actual
            .material_zones
            .iter()
            .zip(&expected.material_zones)
            .all(|(actual, expected)| {
                actual.material_index == expected.material_index
                    && actual.base_color_dimensions_pixels == expected.base_color_dimensions_pixels
                    && approximately_equal(
                        actual.surface_area_square_meters,
                        expected.surface_area_square_meters,
                    )
                    && approximately_equal(
                        actual.uv_area_square_units,
                        expected.uv_area_square_units,
                    )
                    && approximately_equal(
                        actual.effective_texel_density_pixels_per_meter,
                        expected.effective_texel_density_pixels_per_meter,
                    )
            })
}

fn approximately_equal(actual: f32, expected: f32) -> bool {
    let scale = actual.abs().max(expected.abs()).max(1.0);
    (actual - expected).abs() <= scale * 1e-5
}

fn verify_collision_proxies(
    document: &Value,
    binary: &[u8],
    profile: &GameAssetProfile,
    bindings: &GameAssetDeliveryBindings,
    proxies: &[GameAssetCollisionProxyReadback],
) -> CoreResult<()> {
    if proxies.len() != profile.collision_proxy_part_ids.len()
        || document
            .get("meshes")
            .and_then(Value::as_array)
            .is_none_or(|meshes| meshes.len() != 3 + proxies.len())
    {
        return Err(invalid(
            "GAME_ASSET_COLLISION_INVALID",
            "Game delivery collision proxy count is invalid.",
        ));
    }
    let source_primitives = document["meshes"][0]
        .get("primitives")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_COLLISION_INVALID",
                "Game delivery LOD0 primitives are invalid.",
            )
        })?;
    let binding_map = bindings
        .parts
        .iter()
        .map(|binding| (binding.subject_part_id.as_str(), binding))
        .collect::<BTreeMap<_, _>>();
    let mut seen_parts = BTreeSet::new();
    let scene_meshes = document
        .pointer("/scenes/0/nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_COLLISION_INVALID",
                "Game delivery scene is invalid.",
            )
        })?;
    for proxy in proxies {
        let binding = binding_map
            .get(proxy.subject_part_id.as_str())
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_COLLISION_INVALID",
                    "Collision proxy part has no sealed binding.",
                )
            })?;
        let mesh = document
            .get("meshes")
            .and_then(Value::as_array)
            .and_then(|meshes| meshes.get(proxy.mesh_index as usize))
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_COLLISION_INVALID",
                    "Collision proxy mesh index is invalid.",
                )
            })?;
        let primitive = mesh.pointer("/primitives/0").ok_or_else(|| {
            invalid(
                "GAME_ASSET_COLLISION_INVALID",
                "Collision proxy primitive is missing.",
            )
        })?;
        let extras = primitive
            .pointer("/extras/forgecad_game_asset_collision_proxy")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_COLLISION_INVALID",
                    "Collision proxy metadata is missing.",
                )
            })?;
        let expected_bounds = operation_bounds(
            source_primitives,
            document,
            binary,
            &binding.terminal_operation_id,
        )?;
        let actual_positions = primitive
            .get("attributes")
            .and_then(Value::as_object)
            .and_then(|attrs| attribute_accessor(attrs, "POSITION", document).ok())
            .map(|accessor| read_float_vec_accessor(accessor, document, binary, 3))
            .transpose()?
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_COLLISION_INVALID",
                    "Collision proxy positions are missing.",
                )
            })?;
        let actual_indices = primitive_indices(primitive, document, binary)?;
        if !seen_parts.insert(proxy.subject_part_id.as_str())
            || !profile
                .collision_proxy_part_ids
                .contains(&proxy.subject_part_id)
            || proxy.terminal_operation_id != binding.terminal_operation_id
            || proxy.mesh_index < 3
            || proxy.bounds_meters != expected_bounds
            || extras.get("subject_part_id").and_then(Value::as_str)
                != Some(proxy.subject_part_id.as_str())
            || extras.get("terminal_operation_id").and_then(Value::as_str)
                != Some(binding.terminal_operation_id.as_str())
            || actual_positions
                != aabb_vertices(expected_bounds)
                    .into_iter()
                    .map(|item| item.to_vec())
                    .collect::<Vec<_>>()
            || actual_indices
                != aabb_indices()
                    .iter()
                    .map(|value| *value as u32)
                    .collect::<Vec<_>>()
            || scene_meshes.iter().any(|node_index| {
                document
                    .get("nodes")
                    .and_then(Value::as_array)
                    .and_then(|nodes| {
                        node_index
                            .as_u64()
                            .and_then(|index| nodes.get(index as usize))
                    })
                    .and_then(|node| node.get("mesh"))
                    .and_then(Value::as_u64)
                    == Some(proxy.mesh_index as u64)
            })
        {
            return Err(invalid(
                "GAME_ASSET_COLLISION_INVALID",
                "Collision proxy geometry or scene isolation drifted.",
            ));
        }
    }
    Ok(())
}

fn verify_socket_nodes(
    document: &Value,
    profile: &GameAssetProfile,
    bindings: &GameAssetDeliveryBindings,
    sockets: &[GameAssetSocketReadback],
) -> CoreResult<()> {
    let nodes = document
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_SOCKET_INVALID",
                "Game delivery nodes are invalid.",
            )
        })?;
    if sockets.len() != profile.sockets.len() || nodes.len() != 3 + sockets.len() {
        return Err(invalid(
            "GAME_ASSET_SOCKET_INVALID",
            "Game delivery socket count is invalid.",
        ));
    }
    let binding_map = bindings
        .parts
        .iter()
        .map(|binding| (binding.subject_part_id.as_str(), binding))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for socket in sockets {
        let profile_socket = profile
            .sockets
            .iter()
            .find(|candidate| candidate.socket_id == socket.socket_id)
            .ok_or_else(|| invalid("GAME_ASSET_SOCKET_INVALID", "Socket receipt ID is unknown."))?;
        let binding = binding_map
            .get(socket.subject_part_id.as_str())
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_SOCKET_INVALID",
                    "Socket receipt part has no sealed binding.",
                )
            })?;
        let node = nodes
            .get(socket.node_index as usize)
            .ok_or_else(|| invalid("GAME_ASSET_SOCKET_INVALID", "Socket node index is invalid."))?;
        let translation = read_vec3(node.get("translation"))?;
        let rotation = read_vec4(node.get("rotation"))?;
        let expected_forward = normalize_forward(profile_socket.forward)?;
        if !seen.insert(socket.socket_id.as_str())
            || socket.node_index < 3
            || socket.subject_part_id != profile_socket.part_id
            || socket.terminal_operation_id != binding.terminal_operation_id
            || socket.pivot_meters != profile_socket.pivot_meters
            || socket.forward != expected_forward
            || translation != profile_socket.pivot_meters
            || rotation != z_axis_rotation(expected_forward)
            || node.get("mesh").is_some()
            || node
                .pointer("/extras/forgecad_game_asset_socket/socket_id")
                .and_then(Value::as_str)
                != Some(socket.socket_id.as_str())
            || node
                .pointer("/extras/forgecad_game_asset_socket/terminal_operation_id")
                .and_then(Value::as_str)
                != Some(binding.terminal_operation_id.as_str())
        {
            return Err(invalid(
                "GAME_ASSET_SOCKET_INVALID",
                "Socket node geometry or provenance drifted.",
            ));
        }
    }
    Ok(())
}

fn read_vec3(value: Option<&Value>) -> CoreResult<[f32; 3]> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid("GAME_ASSET_SOCKET_INVALID", "Socket vector is invalid."))?;
    let mut output = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        output[index] = value
            .as_f64()
            .map(|value| value as f32)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_SOCKET_INVALID",
                    "Socket vector component is invalid.",
                )
            })?;
    }
    Ok(output)
}

fn read_vec4(value: Option<&Value>) -> CoreResult<[f32; 4]> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 4)
        .ok_or_else(|| invalid("GAME_ASSET_SOCKET_INVALID", "Socket rotation is invalid."))?;
    let mut output = [0.0; 4];
    for (index, value) in values.iter().enumerate() {
        output[index] = value
            .as_f64()
            .map(|value| value as f32)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_SOCKET_INVALID",
                    "Socket rotation component is invalid.",
                )
            })?;
    }
    Ok(output)
}

#[derive(Clone)]
struct PrimitiveInput {
    vertices: Vec<GameAssetLodVertex>,
    indices: Vec<u32>,
}

fn primitive_input(
    primitive: &Value,
    document: &Value,
    binary: &[u8],
) -> CoreResult<PrimitiveInput> {
    let attributes = primitive
        .get("attributes")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB primitive attributes are invalid.",
            )
        })?;
    let positions = read_float_vec_accessor(
        attribute_accessor(attributes, "POSITION", document)?,
        document,
        binary,
        3,
    )?;
    let normals = read_float_vec_accessor(
        attribute_accessor(attributes, "NORMAL", document)?,
        document,
        binary,
        3,
    )?;
    let uv0 = read_float_vec_accessor(
        attribute_accessor(attributes, "TEXCOORD_0", document)?,
        document,
        binary,
        2,
    )?;
    if positions.len() != normals.len() || positions.len() != uv0.len() || positions.len() < 3 {
        return Err(invalid(
            "GAME_ASSET_LOD_SOURCE_INVALID",
            "GLB LOD surface attribute counts disagree.",
        ));
    }
    let vertices = positions
        .iter()
        .zip(&normals)
        .zip(&uv0)
        .map(|((position, normal), uv)| GameAssetLodVertex {
            position: [position[0], position[1], position[2]],
            normal: [normal[0], normal[1], normal[2]],
            uv0: [uv[0], uv[1]],
        })
        .collect::<Vec<_>>();
    Ok(PrimitiveInput {
        vertices,
        indices: primitive_indices(primitive, document, binary)?,
    })
}

fn attribute_accessor<'a>(
    attributes: &serde_json::Map<String, Value>,
    name: &str,
    document: &'a Value,
) -> CoreResult<&'a Value> {
    attributes
        .get(name)
        .and_then(Value::as_u64)
        .and_then(|index| document.get("accessors")?.get(index as usize))
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD attribute accessor is missing.",
            )
        })
}

fn primitive_indices(primitive: &Value, document: &Value, binary: &[u8]) -> CoreResult<Vec<u32>> {
    let accessor = primitive
        .get("indices")
        .and_then(Value::as_u64)
        .and_then(|index| document.get("accessors")?.get(index as usize))
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD index accessor is missing.",
            )
        })?;
    read_index_accessor(accessor, document, binary)
}

fn simplify_tier(
    inputs: &[PrimitiveInput],
    targets: &[u32],
    global_extent: f32,
) -> CoreResult<Vec<crate::GameAssetLodMesh>> {
    inputs
        .iter()
        .zip(targets)
        .map(|(input, target)| simplify_primitive_with_quality_floor(input, *target, global_extent))
        .collect()
}

/// A target is an allocation goal, not permission to violate visible shape.
/// If the strict error bound rejects it, deterministically raise that one
/// primitive's target until a valid result exists. The enclosing tier then
/// verifies the global budget and fails closed if the profile cannot afford
/// the quality-preserving result.
fn simplify_primitive_with_quality_floor(
    input: &PrimitiveInput,
    target: u32,
    global_extent: f32,
) -> CoreResult<crate::GameAssetLodMesh> {
    let source = (input.indices.len() / 3) as u32;
    let mut lower = target;
    let mut upper = source;
    let mut best = simplify_game_asset_lod_with_global_error(
        &input.vertices,
        &input.indices,
        source,
        global_extent,
    )?;
    while lower < upper {
        let candidate = lower + (upper - lower) / 2;
        match simplify_game_asset_lod_with_global_error(
            &input.vertices,
            &input.indices,
            candidate,
            global_extent,
        ) {
            Ok(mesh) => {
                best = mesh;
                upper = candidate;
            }
            Err(error) if error.code() == "GAME_ASSET_LOD_SIMPLIFICATION_FAILED" => {
                lower = candidate + 1;
            }
            Err(error) => return Err(error),
        }
    }
    if let Ok(mesh) = simplify_game_asset_lod_with_global_error(
        &input.vertices,
        &input.indices,
        lower,
        global_extent,
    ) {
        best = mesh;
    }
    Ok(best)
}

fn global_extent(inputs: &[PrimitiveInput]) -> CoreResult<f32> {
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    for vertex in inputs.iter().flat_map(|input| &input.vertices) {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(vertex.position[axis]);
            maximum[axis] = maximum[axis].max(vertex.position[axis]);
        }
    }
    let extent = (0..3)
        .map(|axis| maximum[axis] - minimum[axis])
        .fold(0.0_f32, f32::max);
    if !extent.is_finite() || extent <= f32::EPSILON {
        return Err(invalid(
            "GAME_ASSET_LOD_SOURCE_INVALID",
            "GLB LOD0 has no finite asset extent.",
        ));
    }
    Ok(extent)
}

fn distribute_triangle_budget(source_counts: &[u32], requested: u32) -> CoreResult<Vec<u32>> {
    let source_total = source_counts.iter().copied().sum::<u32>();
    if source_counts.is_empty() || requested == 0 || requested >= source_total {
        return Err(invalid(
            "GAME_ASSET_LOD_BUDGET_NOT_REDUCING",
            "LOD budget must be smaller than non-empty LOD0.",
        ));
    }
    // Small closed components often encode silhouette-critical fasteners or
    // material boundaries. Keep them at LOD0 rather than forcing a one-face
    // reduction that the geometric error gate would reject. Larger primitive
    // budgets are then reduced proportionally, while preserving all source
    // material-zone rows in every LOD tier.
    let protected_counts = source_counts
        .iter()
        .map(|count| if *count <= 64 { *count } else { 4 })
        .collect::<Vec<_>>();
    let minimum_total = protected_counts.iter().copied().sum::<u32>();
    if requested < minimum_total || source_counts.iter().any(|count| *count < 4) {
        return Err(invalid(
            "GAME_ASSET_LOD_BUDGET_UNREPRESENTABLE",
            "LOD budget cannot preserve every material-bound primitive.",
        ));
    }
    let mut targets = protected_counts;
    let mut remaining = requested - minimum_total;
    let capacities = source_counts
        .iter()
        .zip(&targets)
        .map(|(count, target)| count - target)
        .collect::<Vec<_>>();
    let capacity_total = capacities.iter().copied().sum::<u32>();
    for (target, capacity) in targets.iter_mut().zip(&capacities) {
        let allocation = ((remaining as u64 * *capacity as u64) / capacity_total as u64) as u32;
        *target += allocation;
    }
    remaining = requested - targets.iter().copied().sum::<u32>();
    for (target, source) in targets.iter_mut().zip(source_counts) {
        if remaining == 0 {
            break;
        }
        let capacity = source - *target;
        let increment = capacity.min(remaining);
        *target += increment;
        remaining -= increment;
    }
    if remaining != 0
        || targets
            .iter()
            .zip(source_counts)
            .any(|(target, source)| *target < 4 || *target > *source)
    {
        return Err(invalid(
            "GAME_ASSET_LOD_BUDGET_UNREPRESENTABLE",
            "LOD budget allocation was not bounded.",
        ));
    }
    Ok(targets)
}

fn build_lod_mesh(
    source_mesh: &Value,
    simplified: &[crate::GameAssetLodMesh],
    document: &mut Value,
    binary: &mut Vec<u8>,
    suffix: &str,
) -> CoreResult<Value> {
    let mut mesh = source_mesh.clone();
    let primitives = mesh
        .get_mut("primitives")
        .and_then(Value::as_array_mut)
        .filter(|items| items.len() == simplified.len())
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "Source primitive table drifted.",
            )
        })?;
    for (primitive, topology) in primitives.iter_mut().zip(simplified) {
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let byte_offset = binary.len();
        for index in &topology.indices {
            binary.extend_from_slice(&index.to_le_bytes());
        }
        let byte_length = binary.len() - byte_offset;
        let view_index = document["bufferViews"]
            .as_array_mut()
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_LOD_SOURCE_INVALID",
                    "GLB buffer view table is invalid.",
                )
            })?
            .len();
        document["bufferViews"]
            .as_array_mut()
            .expect("checked")
            .push(json!({
                "buffer": 0, "byteOffset": byte_offset, "byteLength": byte_length, "target": 34963
            }));
        let accessor_index = document["accessors"]
            .as_array_mut()
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_LOD_SOURCE_INVALID",
                    "GLB accessor table is invalid.",
                )
            })?
            .len();
        document["accessors"].as_array_mut().expect("checked").push(json!({
            "bufferView": view_index, "componentType": 5125, "count": topology.indices.len(), "type": "SCALAR"
        }));
        primitive["indices"] = json!(accessor_index);
    }
    let base_name = mesh
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("FORGECAD_SHAPE_PROGRAM_MESH");
    mesh["name"] = json!(format!("{base_name}_{suffix}"));
    Ok(mesh)
}

fn tier_triangle_count(tier: &[crate::GameAssetLodMesh]) -> u32 {
    tier.iter().map(|mesh| mesh.triangle_count).sum()
}

fn tier_max_error(tier: &[crate::GameAssetLodMesh]) -> f32 {
    tier.iter().fold(0.0_f32, |maximum, mesh| {
        maximum.max(mesh.simplification_error)
    })
}

fn tier_receipt(level: u8, triangle_count: u32, simplification_error: f32) -> Value {
    json!({"level": level, "triangle_count": triangle_count, "simplification_error": simplification_error})
}

fn parse_receipt(value: &Value) -> CoreResult<GameAssetLodDeliveryReadback> {
    let mut parsed: GameAssetLodDeliveryReadback = serde_json::from_value(json!({
        "schema_version": value.get("schema_version"),
        "source_glb_sha256": value.get("source_glb_sha256"),
        "delivery_glb_sha256": "",
        "game_asset_profile_id": value.get("game_asset_profile_id"),
        "game_asset_profile_sha256": value.get("game_asset_profile_sha256"),
        "lods": value.get("lods"),
    }))
    .map_err(|_| {
        invalid(
            "GAME_ASSET_LOD_RECEIPT_INVALID",
            "Game asset LOD receipt shape is invalid.",
        )
    })?;
    parsed.delivery_glb_sha256.clear();
    if !is_sha256(&parsed.source_glb_sha256)
        || !is_sha256(&parsed.game_asset_profile_sha256)
        || parsed.lods.len() != 3
    {
        return Err(invalid(
            "GAME_ASSET_LOD_RECEIPT_INVALID",
            "Game asset LOD receipt values are invalid.",
        ));
    }
    Ok(parsed)
}

fn validate_lod0_container(document: &Value) -> CoreResult<()> {
    let scene_is_root = document.get("scene").and_then(Value::as_u64) == Some(0);
    let scene_nodes = document
        .pointer("/scenes/0/nodes")
        .and_then(Value::as_array);
    let nodes = document.get("nodes").and_then(Value::as_array);
    let meshes = document.get("meshes").and_then(Value::as_array);
    if !scene_is_root
        || scene_nodes != Some(&vec![json!(0)])
        || nodes.is_none_or(|values| {
            values.len() != 1 || values[0].get("mesh").and_then(Value::as_u64) != Some(0)
        })
        || meshes.is_none_or(|values| values.len() != 1)
    {
        return Err(invalid(
            "GAME_ASSET_LOD_SOURCE_INVALID",
            "Game LOD compiler requires the strict ForgeCAD single-root GLB.",
        ));
    }
    Ok(())
}

fn validate_delivery_container(document: &Value) -> CoreResult<()> {
    validate_lod_root(document)?;
    let ids = document
        .pointer("/nodes/0/extensions/MSFT_lod/ids")
        .and_then(Value::as_array);
    let meshes = document.get("meshes").and_then(Value::as_array);
    let nodes = document.get("nodes").and_then(Value::as_array);
    let has_extension = document
        .get("extensionsUsed")
        .and_then(Value::as_array)
        .is_some_and(|values| {
            values
                .iter()
                .any(|value| value.as_str() == Some(MSFT_LOD_EXTENSION))
        });
    if ids != Some(&vec![json!(1), json!(2)])
        || !has_extension
        || meshes.is_none_or(|values| values.len() != 3)
        || nodes.is_none_or(|values| {
            values.len() != 3
                || values[1].get("mesh").and_then(Value::as_u64) != Some(1)
                || values[2].get("mesh").and_then(Value::as_u64) != Some(2)
                || values[1].get("name").and_then(Value::as_str) != Some("FORGECAD_BLOCKOUT_LOD1")
                || values[2].get("name").and_then(Value::as_str) != Some("FORGECAD_BLOCKOUT_LOD2")
        })
    {
        return Err(invalid(
            "GAME_ASSET_LOD_CONTAINER_INVALID",
            "Game asset GLB does not contain the exact LOD hierarchy.",
        ));
    }
    Ok(())
}

fn validate_lod_root(document: &Value) -> CoreResult<()> {
    let scene_is_root = document.get("scene").and_then(Value::as_u64) == Some(0);
    let scene_nodes = document
        .pointer("/scenes/0/nodes")
        .and_then(Value::as_array);
    if !scene_is_root
        || scene_nodes != Some(&vec![json!(0)])
        || document.pointer("/nodes/0/mesh").and_then(Value::as_u64) != Some(0)
    {
        return Err(invalid(
            "GAME_ASSET_LOD_CONTAINER_INVALID",
            "Game asset GLB root hierarchy is invalid.",
        ));
    }
    Ok(())
}

fn parse_glb(bytes: &[u8]) -> CoreResult<(Value, Vec<u8>)> {
    if bytes.len() < 20
        || bytes.len() > MAX_GAME_ASSET_DELIVERY_BYTES
        || bytes.get(..4) != Some(b"glTF")
    {
        return Err(invalid(
            "GAME_ASSET_LOD_CONTAINER_INVALID",
            "Game asset payload is not a bounded GLB.",
        ));
    }
    if read_u32(bytes, 4)? != 2 || read_u32(bytes, 8)? as usize != bytes.len() {
        return Err(invalid(
            "GAME_ASSET_LOD_CONTAINER_INVALID",
            "Game asset GLB header is invalid.",
        ));
    }
    let mut cursor = 12usize;
    let mut document = None;
    let mut binary = None;
    while cursor < bytes.len() {
        let length = read_u32(bytes, cursor)? as usize;
        let kind = read_u32(bytes, cursor + 4)?;
        let start = cursor.checked_add(8).ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_CONTAINER_INVALID",
                "Game asset GLB offset overflowed.",
            )
        })?;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| {
                invalid(
                    "GAME_ASSET_LOD_CONTAINER_INVALID",
                    "Game asset GLB chunk is truncated.",
                )
            })?;
        match kind {
            0x4e4f_534a if document.is_none() => {
                document = Some(serde_json::from_slice(&bytes[start..end]).map_err(|_| {
                    invalid(
                        "GAME_ASSET_LOD_CONTAINER_INVALID",
                        "Game asset GLB JSON is invalid.",
                    )
                })?)
            }
            0x004e_4942 if binary.is_none() => binary = Some(bytes[start..end].to_vec()),
            _ => {
                return Err(invalid(
                    "GAME_ASSET_LOD_CONTAINER_INVALID",
                    "Game asset GLB has duplicate or unknown chunks.",
                ))
            }
        }
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err(invalid(
            "GAME_ASSET_LOD_CONTAINER_INVALID",
            "Game asset GLB chunk alignment is invalid.",
        ));
    }
    Ok((
        document.ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_CONTAINER_INVALID",
                "Game asset GLB JSON is missing.",
            )
        })?,
        binary.ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_CONTAINER_INVALID",
                "Game asset GLB binary is missing.",
            )
        })?,
    ))
}

fn encode_glb(document: &Value, mut binary: Vec<u8>) -> CoreResult<Vec<u8>> {
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let mut json_chunk = serde_json::to_vec(document).map_err(|_| {
        invalid(
            "GAME_ASSET_LOD_CONTAINER_INVALID",
            "Game asset GLB JSON cannot be encoded.",
        )
    })?;
    while json_chunk.len() % 4 != 0 {
        json_chunk.push(b' ');
    }
    let total = 12usize
        .checked_add(8 + json_chunk.len())
        .and_then(|value| value.checked_add(8 + binary.len()))
        .filter(|value| *value <= MAX_GAME_ASSET_DELIVERY_BYTES && *value <= u32::MAX as usize)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_CONTAINER_INVALID",
                "Game asset GLB output is too large.",
            )
        })?;
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2_u32.to_le_bytes());
    output.extend_from_slice(&(total as u32).to_le_bytes());
    output.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    output.extend_from_slice(b"JSON");
    output.extend_from_slice(&json_chunk);
    output.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    output.extend_from_slice(b"BIN\0");
    output.extend_from_slice(&binary);
    Ok(output)
}

fn read_index_accessor(accessor: &Value, document: &Value, binary: &[u8]) -> CoreResult<Vec<u32>> {
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0 && *count <= 3_000_000)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD index count is invalid.",
            )
        })? as usize;
    if accessor.get("type").and_then(Value::as_str) != Some("SCALAR") {
        return Err(invalid(
            "GAME_ASSET_LOD_SOURCE_INVALID",
            "GLB LOD indices are not scalars.",
        ));
    }
    let component_type = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD index type is missing.",
            )
        })?;
    let component_size = match component_type {
        5123 => 2,
        5125 => 4,
        _ => {
            return Err(invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD index type is unsupported.",
            ))
        }
    };
    let (start, stride) = accessor_range(accessor, document, binary, component_size)?;
    (0..count)
        .map(|ordinal| {
            let offset = start
                .checked_add(ordinal.checked_mul(stride).ok_or_else(|| {
                    invalid(
                        "GAME_ASSET_LOD_SOURCE_INVALID",
                        "GLB LOD index offset overflowed.",
                    )
                })?)
                .ok_or_else(|| {
                    invalid(
                        "GAME_ASSET_LOD_SOURCE_INVALID",
                        "GLB LOD index offset overflowed.",
                    )
                })?;
            match component_type {
                5123 => binary
                    .get(offset..offset + 2)
                    .and_then(|raw| raw.try_into().ok())
                    .map(u16::from_le_bytes)
                    .map(u32::from)
                    .ok_or_else(|| {
                        invalid(
                            "GAME_ASSET_LOD_SOURCE_INVALID",
                            "GLB LOD index data is truncated.",
                        )
                    }),
                5125 => read_u32(binary, offset),
                _ => unreachable!(),
            }
        })
        .collect()
}

fn read_float_vec_accessor(
    accessor: &Value,
    document: &Value,
    binary: &[u8],
    width: usize,
) -> CoreResult<Vec<Vec<f32>>> {
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .filter(|count| *count > 0 && *count <= 3_000_000)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD attribute count is invalid.",
            )
        })? as usize;
    if accessor.get("componentType").and_then(Value::as_u64) != Some(5126)
        || accessor.get("type").and_then(Value::as_str)
            != Some(if width == 3 { "VEC3" } else { "VEC2" })
    {
        return Err(invalid(
            "GAME_ASSET_LOD_SOURCE_INVALID",
            "GLB LOD attribute type is invalid.",
        ));
    }
    let (start, stride) = accessor_range(accessor, document, binary, width * 4)?;
    (0..count)
        .map(|ordinal| {
            let offset = start
                .checked_add(ordinal.checked_mul(stride).ok_or_else(|| {
                    invalid(
                        "GAME_ASSET_LOD_SOURCE_INVALID",
                        "GLB LOD attribute offset overflowed.",
                    )
                })?)
                .ok_or_else(|| {
                    invalid(
                        "GAME_ASSET_LOD_SOURCE_INVALID",
                        "GLB LOD attribute offset overflowed.",
                    )
                })?;
            (0..width)
                .map(|axis| {
                    binary
                        .get(offset + axis * 4..offset + axis * 4 + 4)
                        .and_then(|raw| raw.try_into().ok())
                        .map(f32::from_le_bytes)
                        .filter(|value| value.is_finite())
                        .ok_or_else(|| {
                            invalid(
                                "GAME_ASSET_LOD_SOURCE_INVALID",
                                "GLB LOD attribute data is invalid.",
                            )
                        })
                })
                .collect()
        })
        .collect()
}

fn accessor_range(
    accessor: &Value,
    document: &Value,
    binary: &[u8],
    element_size: usize,
) -> CoreResult<(usize, usize)> {
    let view = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|index| document.get("bufferViews")?.get(index as usize))
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD buffer view is missing.",
            )
        })?;
    if view.get("buffer").and_then(Value::as_u64) != Some(0) {
        return Err(invalid(
            "GAME_ASSET_LOD_SOURCE_INVALID",
            "GLB LOD uses a non-primary buffer.",
        ));
    }
    let view_offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let view_length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_SOURCE_INVALID",
                "GLB LOD view length is invalid.",
            )
        })? as usize;
    let accessor_offset = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .unwrap_or(element_size as u64) as usize;
    let start = view_offset.checked_add(accessor_offset).ok_or_else(|| {
        invalid(
            "GAME_ASSET_LOD_SOURCE_INVALID",
            "GLB LOD view offset overflowed.",
        )
    })?;
    if stride < element_size
        || start
            .checked_add(element_size)
            .is_none_or(|end| end > view_offset + view_length || end > binary.len())
    {
        return Err(invalid(
            "GAME_ASSET_LOD_SOURCE_INVALID",
            "GLB LOD view range is invalid.",
        ));
    }
    Ok((start, stride))
}

fn read_u32(bytes: &[u8], offset: usize) -> CoreResult<u32> {
    bytes
        .get(offset..offset + 4)
        .and_then(|raw| raw.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| {
            invalid(
                "GAME_ASSET_LOD_CONTAINER_INVALID",
                "Game asset GLB integer is truncated.",
            )
        })
}

fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message)
}

#[cfg(test)]
mod tests {
    use super::{
        distribute_triangle_budget, effective_texel_density, normalize_forward,
        triangle_surface_area, triangle_uv_area, validate_delivery_bindings, z_axis_rotation,
        GameAssetDeliveryBindings, GameAssetDeliveryPartBinding,
    };
    use crate::{
        semantic_sha256, GameAssetProfile, GameAssetSocket,
        GAME_ASSET_DELIVERY_BINDINGS_SCHEMA_VERSION, GAME_ASSET_PROFILE_SCHEMA_VERSION,
    };
    use serde_json::json;

    fn profile() -> GameAssetProfile {
        GameAssetProfile {
            schema_version: GAME_ASSET_PROFILE_SCHEMA_VERSION.into(),
            profile_id: "delivery_test".into(),
            lod_triangle_budgets: [100, 64, 32],
            collision_proxy_part_ids: vec!["part_body".into()],
            sockets: vec![GameAssetSocket {
                socket_id: "socket_vfx".into(),
                part_id: "part_body".into(),
                pivot_meters: [0.0, 0.0, 0.0],
                forward: [0.0, 1.0, 0.0],
            }],
            target_texel_density_pixels_per_meter: 1024,
        }
    }

    #[test]
    fn distributes_a_real_reducing_budget_without_losing_material_slots() {
        let targets = distribute_triangle_budget(&[100, 120, 140], 100).unwrap();
        assert_eq!(targets.iter().sum::<u32>(), 100);
        assert!(targets
            .iter()
            .zip([100, 120, 140])
            .all(|(target, source)| *target >= 4 && *target <= source));
    }

    #[test]
    fn rejects_a_budget_that_would_drop_a_material_bound_primitive() {
        assert!(distribute_triangle_budget(&[20, 20], 7).is_err());
        assert!(distribute_triangle_budget(&[4, 20], 12).is_err());
    }

    #[test]
    fn bindings_reject_unsealed_or_extra_part_ownership() {
        let profile = profile();
        let bindings = GameAssetDeliveryBindings {
            schema_version: GAME_ASSET_DELIVERY_BINDINGS_SCHEMA_VERSION.into(),
            source_id: "source_delivery_test".into(),
            source_request_sha256: "a".repeat(64),
            game_asset_profile_sha256: semantic_sha256(&profile).unwrap(),
            parts: vec![
                GameAssetDeliveryPartBinding {
                    subject_part_id: "part_body".into(),
                    terminal_operation_id: "op_body".into(),
                },
                GameAssetDeliveryPartBinding {
                    subject_part_id: "part_extra".into(),
                    terminal_operation_id: "op_extra".into(),
                },
            ],
        };
        assert_eq!(
            validate_delivery_bindings(&profile, &bindings)
                .unwrap_err()
                .code(),
            "GAME_ASSET_DELIVERY_BINDINGS_INVALID"
        );
    }

    #[test]
    fn socket_rotation_has_a_stable_forward_contract() {
        assert_eq!(normalize_forward([0.0, 0.0, 5.0]).unwrap(), [0.0, 0.0, 1.0]);
        assert_eq!(z_axis_rotation([0.0, 0.0, 1.0]), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(z_axis_rotation([0.0, 0.0, -1.0]), [0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn measures_a_unit_square_uv_at_the_embedded_texture_density() {
        let first = triangle_surface_area([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]);
        let second = triangle_surface_area([0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]);
        let first_uv = triangle_uv_area([0.0, 0.0], [1.0, 0.0], [1.0, 1.0]);
        let second_uv = triangle_uv_area([0.0, 0.0], [1.0, 1.0], [0.0, 1.0]);
        let density =
            effective_texel_density((first_uv + second_uv) * 1024.0 * 1024.0, first + second)
                .unwrap();
        assert!((density - 1024.0).abs() < 1e-8);
    }

    #[test]
    fn rejects_zero_surface_or_uv_area_for_texel_density() {
        assert!(effective_texel_density(0.0, 1.0).is_err());
        assert!(effective_texel_density(1.0, 0.0).is_err());
    }

    #[test]
    fn texel_readback_uses_final_glb_uvs_and_embedded_png_dimensions() {
        let mut binary = Vec::new();
        for value in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        for value in [0.0_f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0] {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        for value in [0.0_f32, 0.0, 1.0, 0.0, 0.0, 1.0] {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        for value in [0_u16, 1, 2] {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        let png_offset = binary.len();
        binary.extend_from_slice(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x04\0\0\0\x04\0");
        let document = json!({
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 36},
                {"buffer": 0, "byteOffset": 36, "byteLength": 36},
                {"buffer": 0, "byteOffset": 72, "byteLength": 24},
                {"buffer": 0, "byteOffset": 96, "byteLength": 6},
                {"buffer": 0, "byteOffset": png_offset, "byteLength": 24}
            ],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"},
                {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"},
                {"bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2"},
                {"bufferView": 3, "componentType": 5123, "count": 3, "type": "SCALAR"}
            ],
            "images": [{"bufferView": 4, "mimeType": "image/png"}],
            "textures": [{"source": 0}],
            "materials": [{"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}}],
            "meshes": [{"primitives": [{
                "attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2},
                "indices": 3, "material": 0, "mode": 4
            }]}]
        });
        let readback = super::measure_texel_density(&document, &binary, &profile()).unwrap();
        assert_eq!(readback.material_zones.len(), 1);
        assert_eq!(
            readback.material_zones[0].base_color_dimensions_pixels,
            [1024, 1024]
        );
        assert!((readback.effective_texel_density_pixels_per_meter - 1024.0).abs() < 1e-4);
        assert!(readback.target_met);
    }
}
