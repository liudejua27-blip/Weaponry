//! Closed importer kernel for the ForgeCAD FPS production foundation assets.
//!
//! This is deliberately an input adapter, not a general-purpose glTF loader.
//! The only accepted sources are the three manifest-bound, offline foundation
//! assets represented by [`BuiltinWeaponFoundationAsset`].  The caller passes
//! bytes that it has already selected; this module does not read a path, URL,
//! environment variable, script, or external resource.
//!
//! The importer emits a compact ForgeCAD-owned projection suitable for the
//! next `AuthoringMesh@2` genesis step.  It retains normalized positions and
//! triangle faces, deterministic topology/semantic identities, accumulated
//! node transforms, source sockets, a rigid rest-pose skeleton, bounded
//! animation/PBR inventories, and `FpsPresentationPackage@1` availability
//! information.  It intentionally does not materialize a durable
//! `AuthoringMeshRevision`; doing that here would duplicate Store ownership
//! and can exceed the durable response budget for the larger source mesh.

use forgecad_core::{canonical_json_hash, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const GLB_MAGIC: &[u8; 4] = b"glTF";
const JSON_CHUNK: &[u8; 4] = b"JSON";
const BIN_CHUNK: &[u8; 4] = b"BIN\0";
const GLB_VERSION: u32 = 2;
const MAX_GLB_BYTES: usize = 64 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
const MAX_NODE_COUNT: usize = 4096;
const MAX_MESH_COUNT: usize = 512;
const MAX_PRIMITIVE_COUNT: usize = 4096;
const MAX_ACCESSOR_COUNT: usize = 16384;
const MAX_BUFFER_VIEW_COUNT: usize = 16384;
const MAX_VERTEX_COUNT: usize = 2_000_000;
const MAX_TRIANGLE_COUNT: usize = 4_000_000;
const MAX_MATERIAL_COUNT: usize = 1024;
const MAX_TEXTURE_COUNT: usize = 1024;
const MAX_IMAGE_COUNT: usize = 1024;
const MAX_SKIN_COUNT: usize = 64;
const MAX_JOINT_COUNT: usize = 4096;
const MAX_ANIMATION_COUNT: usize = 512;
const MAX_ANIMATION_CHANNEL_COUNT: usize = 16_384;
const MAX_ANIMATION_KEY_COUNT: usize = 1_000_000;
const MAX_ANIMATION_DURATION_SECONDS: f64 = 600.0;
const DEGENERATE_AREA_EPSILON: f64 = 1.0e-12;
const TRANSFORM_ABS_LIMIT: f64 = 1.0e9;
const MATRIX_EPSILON: f64 = 1.0e-12;

pub(crate) const WEAPON_FOUNDATION_IMPORT_SCHEMA_VERSION: &str = "WeaponFoundationImport@1";
pub(crate) const FPS_PRESENTATION_PACKAGE_SCHEMA_VERSION: &str = "FpsPresentationPackage@1";
pub(crate) const FOUNDATION_COORDINATE_FRAME_ID: &str =
    "forgecad-right-handed-x-muzzle-y-up-z-right@1";
pub(crate) const FOUNDATION_TOPOLOGY_POLICY: &str =
    "forgecad-foundation-triangle-topology@1:stable-part-vertex-face-ids:drop-degenerate-area-below-1e-12m2";

/// The closed source set.  These names intentionally describe provenance,
/// not an open file-system import API.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BuiltinWeaponFoundationAsset {
    PichuliruWeaponWest,
    WradArms,
    LightningBenchmark,
}

impl BuiltinWeaponFoundationAsset {
    pub(crate) const ALL: [Self; 3] = [
        Self::PichuliruWeaponWest,
        Self::WradArms,
        Self::LightningBenchmark,
    ];

    pub(crate) const fn asset_id(self) -> &'static str {
        match self {
            Self::PichuliruWeaponWest => "pichuliru-weapon-west",
            Self::WradArms => "wrad-arms",
            Self::LightningBenchmark => "lightning-low-pbr",
        }
    }

    pub(crate) const fn expected_sha256(self) -> &'static str {
        match self {
            Self::PichuliruWeaponWest => {
                "0d80dd2118c884172a856455968be14eadc97f041d27d52bfa75fedb708fa486"
            }
            Self::WradArms => "580efbb0852bf0b41f82dd3e17eafec86b3d2a48f4a7acaa7e64d60e850f565d",
            Self::LightningBenchmark => {
                "3f84f2b0d011ebfb142de7f7d9cfa7d57a59451a815b834b4f33603256c8f911"
            }
        }
    }

    /// Source-space axes are intentionally explicit and asset-specific.  The
    /// Pichuliru west source has its muzzle toward `-Z`; Lightning and WRAD
    /// use `+Z` for the forward direction.  With `s = forward_sign`, the
    /// target vector is `[s*source_z, source_y, -s*source_x]`, preserving a
    /// right-handed frame while keeping source Y up.
    pub(crate) const fn coordinate_policy(self) -> CoordinatePolicy {
        match self {
            Self::PichuliruWeaponWest => CoordinatePolicy { forward_sign: -1 },
            Self::WradArms | Self::LightningBenchmark => CoordinatePolicy { forward_sign: 1 },
        }
    }

    pub(crate) const fn slug(self) -> &'static str {
        match self {
            Self::PichuliruWeaponWest => "pichuliru_weapon_west",
            Self::WradArms => "wrad_arms",
            Self::LightningBenchmark => "lightning_low_pbr",
        }
    }
}

/// Compatibility alias for callers that use the shorter product vocabulary.
pub(crate) type WeaponFoundationAsset = BuiltinWeaponFoundationAsset;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CoordinatePolicy {
    pub forward_sign: i8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationCoordinateFrame {
    pub schema_version: String,
    pub frame_id: String,
    pub handedness: String,
    pub units: String,
    pub up_axis: String,
    pub forward_axis: String,
    pub side_axis: String,
    pub source_forward_sign: i8,
    /// Column-major 4x4 source-to-ForgeCAD basis change.
    pub source_to_forgecad: [f64; 16],
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationSourceInventory {
    pub asset_id: String,
    pub source_sha256: String,
    pub expected_sha256: String,
    pub byte_length: usize,
    pub node_count: usize,
    pub mesh_count: usize,
    pub primitive_count: usize,
    pub material_count: usize,
    pub texture_count: usize,
    pub image_count: usize,
    pub skin_count: usize,
    pub animation_count: usize,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationSanitation {
    pub policy: String,
    pub source_triangle_count: usize,
    pub degenerate_faces_removed: usize,
    pub removed_face_index_sha256: String,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationAttributeInventory {
    pub source_vertex_count: usize,
    pub has_normals: bool,
    pub has_uv0: bool,
    pub has_tangents: bool,
    pub has_skin_weights: bool,
    pub has_vertex_colors: bool,
    pub source_attribute_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationTopologyInventory {
    pub policy: String,
    pub vertex_id_scheme: String,
    pub face_id_scheme: String,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub boundary_edge_count: usize,
    pub non_manifold_edge_count: usize,
    pub topology_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationMesh {
    pub mesh_id: String,
    pub part_id: String,
    pub source_mesh_index: usize,
    pub source_node_index: usize,
    pub source_skin_index: Option<usize>,
    pub positions_m: Vec<[f64; 3]>,
    pub faces: Vec<[u32; 3]>,
    pub face_material_indices: Vec<u32>,
    pub world_transform_m: [f64; 16],
    pub attributes: FoundationAttributeInventory,
    pub topology: FoundationTopologyInventory,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationSemanticNode {
    pub stable_id: String,
    pub source_node_index: usize,
    pub source_name: String,
    pub kind: String,
    pub semantic_role: Option<String>,
    pub parent_stable_id: Option<String>,
    pub mesh_id: Option<String>,
    pub skin_index: Option<usize>,
    pub local_transform_m: [f64; 16],
    pub world_transform_m: [f64; 16],
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationSocket {
    pub socket_id: String,
    pub role: String,
    pub source_name: String,
    pub source_node_index: usize,
    pub stable_node_id: String,
    pub parent_stable_id: Option<String>,
    pub local_transform_m: [f64; 16],
    pub world_transform_m: [f64; 16],
    pub inferred: bool,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationRigBone {
    pub bone_id: String,
    pub source_node_index: usize,
    pub source_name: String,
    pub parent_bone_id: Option<String>,
    pub local_transform_m: [f64; 16],
    pub world_transform_m: [f64; 16],
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationRig {
    pub schema_version: String,
    pub rig_id: String,
    pub source_skin_index: Option<usize>,
    pub source_skin_name: Option<String>,
    pub root_bone_id: Option<String>,
    pub bones: Vec<FoundationRigBone>,
    pub skeleton_sha256: String,
    pub inverse_bind_matrices_sha256: Option<String>,
    pub rest_pose_sha256: String,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationAnimationChannel {
    pub stable_node_id: String,
    pub source_node_index: usize,
    pub path: String,
    pub interpolation: String,
    pub key_count: usize,
    pub time_range_seconds: [f64; 2],
    pub input_accessor: usize,
    pub output_accessor: usize,
    pub normalized_value_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationAnimation {
    pub clip_id: String,
    pub semantic_clip_id: String,
    pub source_animation_index: usize,
    pub source_name: String,
    pub duration_seconds: f64,
    pub channels: Vec<FoundationAnimationChannel>,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationImageInventory {
    pub image_index: usize,
    pub name: String,
    pub mime_type: String,
    pub byte_length: usize,
    pub embedded_bytes_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationTextureInventory {
    pub texture_index: usize,
    pub source_image_index: usize,
    pub sampler_index: Option<usize>,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationMaterialInventory {
    pub material_index: usize,
    pub name: String,
    pub base_color_factor: [f64; 4],
    pub metallic_factor: f64,
    pub roughness_factor: f64,
    pub base_color_texture: Option<usize>,
    pub metallic_roughness_texture: Option<usize>,
    pub normal_texture: Option<usize>,
    pub occlusion_texture: Option<usize>,
    pub emissive_texture: Option<usize>,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationPbrInventory {
    pub schema_version: String,
    pub materials: Vec<FoundationMaterialInventory>,
    pub textures: Vec<FoundationTextureInventory>,
    pub images: Vec<FoundationImageInventory>,
    pub material_inventory_sha256: String,
    pub texture_inventory_sha256: String,
    pub image_inventory_sha256: String,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FoundationRequiredClip {
    pub clip_id: String,
    pub source_backed: bool,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FpsPresentationPackage {
    pub schema_version: String,
    pub package_id: String,
    pub asset_id: String,
    pub coordinate_frame_id: String,
    pub rig_id: String,
    pub socket_ids: Vec<String>,
    pub missing_required_socket_roles: Vec<String>,
    pub animations: Vec<FoundationRequiredClip>,
    pub required_clips: Vec<FoundationRequiredClip>,
    pub camera_profiles: Vec<String>,
    pub gameplay_beats: Vec<String>,
    pub vfx_cues: Vec<String>,
    pub audio_cues: Vec<String>,
    pub status: String,
    pub promotion_eligible: bool,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WeaponFoundationImport {
    pub schema_version: String,
    pub source: FoundationSourceInventory,
    pub coordinate_frame: FoundationCoordinateFrame,
    pub meshes: Vec<FoundationMesh>,
    pub semantic_nodes: Vec<FoundationSemanticNode>,
    pub sockets: Vec<FoundationSocket>,
    pub rig: FoundationRig,
    pub animations: Vec<FoundationAnimation>,
    pub pbr: FoundationPbrInventory,
    pub sanitation: FoundationSanitation,
    pub fps_presentation: FpsPresentationPackage,
    pub canonical_sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum WeaponFoundationImportError {
    #[error("weapon foundation source is invalid: {0}")]
    Invalid(String),
    #[error("weapon foundation source exceeds budget: {0}")]
    Budget(String),
    #[error("weapon foundation source hash mismatch: expected {expected}, got {actual}")]
    SourceHashMismatch { expected: String, actual: String },
}

type ImportResult<T> = Result<T, WeaponFoundationImportError>;
type Mat4 = [f64; 16];

/// Import one manifest-bound foundation GLB.  `bytes` is intentionally the
/// sole payload; no path, URI, script or network lookup is possible here.
pub(crate) fn import_weapon_foundation(
    asset: BuiltinWeaponFoundationAsset,
    bytes: &[u8],
) -> ImportResult<WeaponFoundationImport> {
    let source_sha256 = sha256_hex(bytes);
    if source_sha256 != asset.expected_sha256() {
        return Err(WeaponFoundationImportError::SourceHashMismatch {
            expected: asset.expected_sha256().to_owned(),
            actual: source_sha256,
        });
    }

    let parsed = ParsedGlb::parse(bytes)?;
    let coordinate_frame = coordinate_frame(asset);
    let nodes = NodeGraph::build(asset, &parsed.root, &coordinate_frame)?;
    let skin = parse_skin(
        &parsed.root,
        &parsed.accessors,
        &parsed.views,
        parsed.binary,
    )?;
    let DecodedMeshes {
        meshes,
        source_triangle_count,
        removed_face_indices,
    } = decode_meshes(
        asset,
        &parsed.root,
        &parsed.accessors,
        &parsed.views,
        parsed.binary,
        &nodes,
        skin.as_ref(),
    )?;
    let semantic_nodes = nodes.semantic_nodes(&meshes);
    let sockets = nodes.sockets(asset)?;
    let rig = build_rig(asset, &nodes, skin.as_ref())?;
    let animations = decode_animations(
        asset,
        &parsed.root,
        &parsed.accessors,
        &parsed.views,
        parsed.binary,
        &nodes,
    )?;
    let pbr = decode_pbr_inventory(&parsed.root, &parsed.views, parsed.binary)?;

    let sanitation = sanitation(source_triangle_count, &removed_face_indices);
    let source = source_inventory(
        asset,
        bytes.len(),
        &source_sha256,
        &parsed,
        meshes.len(),
        skin.as_ref(),
        animations.len(),
        &pbr,
    );
    let fps_presentation = build_fps_presentation(asset, &rig, &sockets, &animations);

    let mut result = WeaponFoundationImport {
        schema_version: WEAPON_FOUNDATION_IMPORT_SCHEMA_VERSION.to_owned(),
        source,
        coordinate_frame,
        meshes,
        semantic_nodes,
        sockets,
        rig,
        animations,
        pbr,
        sanitation,
        fps_presentation,
        canonical_sha256: String::new(),
    };
    result.canonical_sha256 = canonical_hash_without_field(&result, "canonical_sha256");
    Ok(result)
}

/// Compatibility spelling used by callers that emphasize the closed input
/// boundary.
pub(crate) fn import_builtin_weapon_foundation(
    asset: BuiltinWeaponFoundationAsset,
    bytes: &[u8],
) -> ImportResult<WeaponFoundationImport> {
    import_weapon_foundation(asset, bytes)
}

#[derive(Debug)]
struct ParsedGlb<'a> {
    root: Value,
    binary: &'a [u8],
    accessors: Vec<Value>,
    views: Vec<Value>,
}

impl<'a> ParsedGlb<'a> {
    fn parse(bytes: &'a [u8]) -> ImportResult<Self> {
        if bytes.len() < 28 {
            return Err(invalid("GLB is shorter than one JSON and one BIN chunk"));
        }
        if bytes.len() > MAX_GLB_BYTES {
            return Err(budget("GLB byte length"));
        }
        if bytes.get(..4) != Some(GLB_MAGIC) {
            return Err(invalid("GLB magic is not glTF"));
        }
        if read_u32(bytes, 4)? != GLB_VERSION {
            return Err(invalid("GLB version must be 2"));
        }
        let declared_length = usize_from_u32(read_u32(bytes, 8)?, "GLB total length")?;
        if declared_length != bytes.len() {
            return Err(invalid("GLB total length differs from input length"));
        }
        let json_length = usize_from_u32(read_u32(bytes, 12)?, "GLB JSON length")?;
        if bytes.get(16..20) != Some(JSON_CHUNK) {
            return Err(invalid("first GLB chunk is not JSON"));
        }
        if json_length == 0 || json_length > MAX_JSON_BYTES {
            return Err(budget("GLB JSON chunk length"));
        }
        let json_end = 20usize
            .checked_add(json_length)
            .ok_or_else(|| invalid("GLB JSON end overflows"))?;
        if json_end.checked_add(8).is_none_or(|end| end > bytes.len()) {
            return Err(invalid("GLB JSON chunk is out of bounds"));
        }
        if bytes[20..json_end]
            .iter()
            .any(|byte| *byte == 0 || (!byte.is_ascii_whitespace() && *byte < 0x20))
        {
            return Err(invalid("GLB JSON padding contains a control byte"));
        }
        let root: Value = serde_json::from_slice(&bytes[20..json_end])
            .map_err(|error| invalid(format!("GLB JSON decode failed: {error}")))?;
        reject_external_fields(&root)?;
        validate_root_shape(&root)?;

        let binary_length = usize_from_u32(read_u32(bytes, json_end)?, "GLB BIN length")?;
        if bytes.get(json_end + 4..json_end + 8) != Some(BIN_CHUNK) {
            return Err(invalid("second GLB chunk is not BIN"));
        }
        let binary_start = json_end + 8;
        let binary_end = binary_start
            .checked_add(binary_length)
            .ok_or_else(|| invalid("GLB BIN end overflows"))?;
        if binary_end != bytes.len() {
            return Err(invalid("GLB BIN length differs from input length"));
        }
        let binary = &bytes[binary_start..binary_end];
        let accessors = optional_array(&root, "accessors")?.to_vec();
        let views = optional_array(&root, "bufferViews")?.to_vec();
        if accessors.len() > MAX_ACCESSOR_COUNT {
            return Err(budget("accessor count"));
        }
        if views.len() > MAX_BUFFER_VIEW_COUNT {
            return Err(budget("bufferView count"));
        }
        validate_buffer(&root, binary)?;
        Ok(Self {
            root,
            binary,
            accessors: accessors.to_vec(),
            views: views.to_vec(),
        })
    }
}

fn coordinate_frame(asset: BuiltinWeaponFoundationAsset) -> FoundationCoordinateFrame {
    let policy = asset.coordinate_policy();
    let basis = source_to_forgecad_matrix(policy);
    let mut frame = FoundationCoordinateFrame {
        schema_version: "SubjectCoordinateFrame@1".to_owned(),
        frame_id: FOUNDATION_COORDINATE_FRAME_ID.to_owned(),
        handedness: "right-handed".to_owned(),
        units: "meter".to_owned(),
        up_axis: "+Y".to_owned(),
        forward_axis: "+X".to_owned(),
        side_axis: "+Z".to_owned(),
        source_forward_sign: policy.forward_sign,
        source_to_forgecad: basis,
        canonical_sha256: String::new(),
    };
    frame.canonical_sha256 = canonical_hash_without_field(&frame, "canonical_sha256");
    frame
}

fn source_to_forgecad_matrix(policy: CoordinatePolicy) -> Mat4 {
    let sign = policy.forward_sign as f64;
    // Column-major matrix: target = B * source.
    [
        0.0, 0.0, sign, 0.0, // target X = sign * source Z
        0.0, 1.0, 0.0, 0.0, // target Y = source Y
        -sign, 0.0, 0.0, 0.0, // target Z = -sign * source X
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn inverse_basis(policy: CoordinatePolicy) -> Mat4 {
    // B is orthogonal, so inverse(B) = transpose(B).
    let sign = policy.forward_sign as f64;
    [
        0.0, 0.0, -sign, 0.0, // source X = -sign * target Z
        0.0, 1.0, 0.0, 0.0, sign, 0.0, 0.0, 0.0, // source Z = sign * target X
        0.0, 0.0, 0.0, 1.0,
    ]
}

fn convert_matrix(source: Mat4, policy: CoordinatePolicy) -> Mat4 {
    mat4_mul(
        mat4_mul(source_to_forgecad_matrix(policy), source),
        inverse_basis(policy),
    )
}

#[derive(Debug)]
struct NodeGraph {
    asset: BuiltinWeaponFoundationAsset,
    names: Vec<String>,
    stable_ids: Vec<String>,
    parents: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    locals_source: Vec<Mat4>,
    worlds_source: Vec<Mat4>,
    locals_target: Vec<Mat4>,
    worlds_target: Vec<Mat4>,
    meshes: Vec<Option<usize>>,
    skins: Vec<Option<usize>>,
}

impl NodeGraph {
    fn build(
        asset: BuiltinWeaponFoundationAsset,
        root: &Value,
        frame: &FoundationCoordinateFrame,
    ) -> ImportResult<Self> {
        let nodes_value = required_array(root, "nodes")?;
        if nodes_value.is_empty() || nodes_value.len() > MAX_NODE_COUNT {
            return Err(budget("node count"));
        }
        let node_count = nodes_value.len();
        let mut names = Vec::with_capacity(node_count);
        let mut locals_source = Vec::with_capacity(node_count);
        let mut parents = vec![None; node_count];
        let mut children = vec![Vec::new(); node_count];
        let mut meshes = Vec::with_capacity(node_count);
        let mut skins = Vec::with_capacity(node_count);

        for (index, value) in nodes_value.iter().enumerate() {
            let object = object(value, "node")?;
            reject_unknown_fields(
                object,
                &[
                    "camera",
                    "children",
                    "extensions",
                    "extras",
                    "matrix",
                    "mesh",
                    "name",
                    "rotation",
                    "scale",
                    "skin",
                    "translation",
                    "weights",
                ],
                "node",
            )?;
            let name = object
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| if index == 0 { "root" } else { "node" })
                .to_owned();
            if name.len() > 256 {
                return Err(budget("node name length"));
            }
            let local = node_local_matrix(object)?;
            locals_source.push(local);
            meshes.push(optional_index(object, "mesh")?);
            skins.push(optional_index(object, "skin")?);
            if object.contains_key("weights") {
                return Err(invalid("morph target weights are not supported"));
            }
            names.push(name);

            if let Some(children_value) = object.get("children") {
                let children_array = children_value
                    .as_array()
                    .ok_or_else(|| invalid("node children must be an array"))?;
                if children_array.len() > node_count {
                    return Err(budget("node child count"));
                }
                for child_value in children_array {
                    let child = index_from_value(child_value, "node child index")?;
                    if child >= node_count || child == index {
                        return Err(invalid("node child index is invalid or self-referential"));
                    }
                    if parents[child].replace(index).is_some() {
                        return Err(invalid("node has more than one parent"));
                    }
                    children[index].push(child);
                }
            }
        }

        let stable_ids = stable_node_ids(asset, &names)?;
        let scene_roots = active_scene_roots(root, node_count)?;
        if scene_roots.is_empty() {
            return Err(invalid("active scene has no roots"));
        }
        let mut state = vec![0u8; node_count];
        let mut worlds_source = vec![mat4_identity(); node_count];
        for root_index in scene_roots {
            visit_node(
                root_index,
                &children,
                &locals_source,
                mat4_identity(),
                &mut state,
                &mut worlds_source,
            )?;
        }
        if state.iter().any(|value| *value != 2) {
            return Err(invalid("active scene does not cover every node"));
        }
        let policy = CoordinatePolicy {
            forward_sign: frame.source_forward_sign,
        };
        let locals_target = locals_source
            .iter()
            .copied()
            .map(|matrix| convert_matrix(matrix, policy))
            .collect::<Vec<_>>();
        let worlds_target = worlds_source
            .iter()
            .copied()
            .map(|matrix| convert_matrix(matrix, policy))
            .collect::<Vec<_>>();
        Ok(Self {
            asset,
            names,
            stable_ids,
            parents,
            children,
            locals_source,
            worlds_source,
            locals_target,
            worlds_target,
            meshes,
            skins,
        })
    }

    fn semantic_nodes(&self, imported_meshes: &[FoundationMesh]) -> Vec<FoundationSemanticNode> {
        let mesh_by_node = imported_meshes
            .iter()
            .map(|mesh| (mesh.source_node_index, mesh.mesh_id.clone()))
            .collect::<HashMap<_, _>>();
        self.names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let role = semantic_role(name, self.meshes[index].is_some());
                let kind = semantic_kind(name, self.meshes[index].is_some(), role.as_deref());
                let mut node = FoundationSemanticNode {
                    stable_id: self.stable_ids[index].clone(),
                    source_node_index: index,
                    source_name: name.clone(),
                    kind,
                    semantic_role: role,
                    parent_stable_id: self.parents[index]
                        .map(|parent| self.stable_ids[parent].clone()),
                    mesh_id: mesh_by_node.get(&index).cloned(),
                    skin_index: self.skins[index],
                    local_transform_m: self.locals_target[index],
                    world_transform_m: self.worlds_target[index],
                    canonical_sha256: String::new(),
                };
                node.canonical_sha256 = canonical_hash_without_field(&node, "canonical_sha256");
                node
            })
            .collect()
    }

    fn sockets(&self, asset: BuiltinWeaponFoundationAsset) -> ImportResult<Vec<FoundationSocket>> {
        let mut sockets = Vec::new();
        let mut roles = BTreeSet::new();
        for (index, name) in self.names.iter().enumerate() {
            let Some(role) = socket_role(name) else {
                continue;
            };
            if !roles.insert(role.to_owned()) {
                return Err(invalid(format!("duplicate source socket role {role}")));
            }
            let mut socket = FoundationSocket {
                socket_id: format!("forgecad.foundation.socket.{}.{}", asset.slug(), role),
                role: role.to_owned(),
                source_name: name.clone(),
                source_node_index: index,
                stable_node_id: self.stable_ids[index].clone(),
                parent_stable_id: self.parents[index].map(|parent| self.stable_ids[parent].clone()),
                local_transform_m: self.locals_target[index],
                world_transform_m: self.worlds_target[index],
                inferred: false,
                canonical_sha256: String::new(),
            };
            socket.canonical_sha256 = canonical_hash_without_field(&socket, "canonical_sha256");
            sockets.push(socket);
        }
        Ok(sockets)
    }
}

fn visit_node(
    index: usize,
    children: &[Vec<usize>],
    locals_source: &[Mat4],
    parent_world: Mat4,
    state: &mut [u8],
    worlds_source: &mut [Mat4],
) -> ImportResult<()> {
    match state[index] {
        1 => return Err(invalid("node graph contains a cycle")),
        2 => return Err(invalid("node is reachable through multiple scene paths")),
        _ => {}
    }
    state[index] = 1;
    let world = mat4_mul(parent_world, locals_source[index]);
    ensure_matrix_bounded(world, "node world transform")?;
    worlds_source[index] = world;
    for child in &children[index] {
        visit_node(*child, children, locals_source, world, state, worlds_source)?;
    }
    state[index] = 2;
    Ok(())
}

fn active_scene_roots(root: &Value, node_count: usize) -> ImportResult<Vec<usize>> {
    let scenes = optional_array(root, "scenes")?;
    if scenes.is_empty() {
        return Err(invalid("scenes array is required"));
    }
    let scene_index = root
        .get("scene")
        .map(|value| index_from_value(value, "default scene index"))
        .transpose()?
        .unwrap_or(0);
    let scene = scenes
        .get(scene_index)
        .ok_or_else(|| invalid("default scene index is out of bounds"))?;
    let object = object(scene, "scene")?;
    reject_unknown_fields(object, &["extensions", "extras", "name", "nodes"], "scene")?;
    let roots = object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("active scene nodes are missing"))?;
    let mut result = Vec::with_capacity(roots.len());
    let mut seen = HashSet::new();
    for value in roots {
        let node = index_from_value(value, "scene root index")?;
        if node >= node_count || !seen.insert(node) {
            return Err(invalid("scene root index is invalid or duplicated"));
        }
        result.push(node);
    }
    Ok(result)
}

fn stable_node_ids(
    asset: BuiltinWeaponFoundationAsset,
    names: &[String],
) -> ImportResult<Vec<String>> {
    let mut counts = HashMap::<String, usize>::new();
    let mut ids = Vec::with_capacity(names.len());
    let prefix = format!("forgecad.foundation.node.{}.", asset.slug());
    let mut seen = HashSet::new();
    for (index, name) in names.iter().enumerate() {
        let slug = slug(name);
        let ordinal = counts.entry(slug.clone()).or_insert(0);
        let mut id = if *ordinal == 0 {
            format!("{prefix}{slug}")
        } else {
            format!("{prefix}{slug}.n{:04}", *ordinal)
        };
        *ordinal += 1;
        if !seen.insert(id.clone()) {
            id = format!("{prefix}{slug}.source{:04}", index);
            if !seen.insert(id.clone()) {
                return Err(invalid("stable node ID collision"));
            }
        }
        ids.push(id);
    }
    Ok(ids)
}

fn semantic_role(name: &str, has_mesh: bool) -> Option<String> {
    let lowered = name.to_ascii_lowercase();
    if lowered.contains("rig") || lowered.contains("armature") || lowered == "root" {
        return Some("root".to_owned());
    }
    if lowered == "body" {
        return Some("body".to_owned());
    }
    if let Some(socket) = socket_role(name) {
        return Some(format!("socket:{socket}"));
    }
    for part in [
        "magazine",
        "trigger",
        "selector",
        "bolt",
        "bolt release",
        "magazine release",
        "forward assist",
        "charging handle",
        "stock",
        "rear sights",
        "front sights",
        "dust cover",
        "hammer",
        "loadinggate",
        "lifter",
        "pump",
        "pin",
    ] {
        if lowered == part {
            return Some(format!("part:{}", slug(part)));
        }
    }
    if lowered.contains("wrist_ik") || lowered.contains("arm_target") {
        return Some(format!("control:{}", slug(name)));
    }
    if lowered.contains("finger_")
        || lowered.contains("forearm")
        || lowered.contains("bicep")
        || lowered.contains("shoulder")
        || lowered == "head"
    {
        return Some(format!("bone:{}", slug(name)));
    }
    if has_mesh {
        Some(format!("mesh:{}", slug(name)))
    } else {
        None
    }
}

fn semantic_kind(name: &str, has_mesh: bool, role: Option<&str>) -> String {
    if role.is_some_and(|value| value.starts_with("socket:")) {
        "socket".to_owned()
    } else if role.is_some_and(|value| value == "root") {
        "root".to_owned()
    } else if role.is_some_and(|value| value.starts_with("bone:")) {
        "bone".to_owned()
    } else if role.is_some_and(|value| value.starts_with("control:")) {
        "control".to_owned()
    } else if role.is_some_and(|value| value.starts_with("part:")) {
        "part".to_owned()
    } else if has_mesh {
        "mesh".to_owned()
    } else if name.eq_ignore_ascii_case("body") {
        "group".to_owned()
    } else {
        "group".to_owned()
    }
}

fn socket_role(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "attach_scope" => Some("optic"),
        "attach_muzzle" => Some("muzzle"),
        "attach_rail.top" => Some("rail_top"),
        "attach_rail.bottom" => Some("rail_bottom"),
        "attach_rail.sideleft" => Some("rail_left"),
        "attach_rail.sideright" => Some("rail_right"),
        "socket.l" => Some("left_grip"),
        "socket.r" => Some("right_grip"),
        _ => None,
    }
}

const REQUIRED_SOCKET_ROLES: &[&str] = &[
    "muzzle",
    "optic",
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

const REQUIRED_PRESENTATION_CLIPS: &[&str] = &[
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

fn build_fps_presentation(
    asset: BuiltinWeaponFoundationAsset,
    rig: &FoundationRig,
    sockets: &[FoundationSocket],
    animations: &[FoundationAnimation],
) -> FpsPresentationPackage {
    let available_sockets = sockets
        .iter()
        .map(|socket| socket.role.as_str())
        .collect::<BTreeSet<_>>();
    let missing_required_socket_roles = REQUIRED_SOCKET_ROLES
        .iter()
        .filter(|role| !available_sockets.contains(**role))
        .map(|role| (*role).to_owned())
        .collect::<Vec<_>>();
    let available_clips = animations
        .iter()
        .map(|animation| animation.semantic_clip_id.as_str())
        .collect::<BTreeSet<_>>();
    let required_clips = REQUIRED_PRESENTATION_CLIPS
        .iter()
        .map(|clip_id| FoundationRequiredClip {
            clip_id: (*clip_id).to_owned(),
            source_backed: available_clips.contains(*clip_id),
            status: if available_clips.contains(*clip_id) {
                "source-backed".to_owned()
            } else {
                "missing-source-clip".to_owned()
            },
        })
        .collect::<Vec<_>>();
    let source_animations = animations
        .iter()
        .map(|animation| FoundationRequiredClip {
            clip_id: animation.semantic_clip_id.clone(),
            source_backed: true,
            status: "source-backed".to_owned(),
        })
        .collect::<Vec<_>>();
    let mut package = FpsPresentationPackage {
        schema_version: FPS_PRESENTATION_PACKAGE_SCHEMA_VERSION.to_owned(),
        package_id: format!("forgecad.foundation.presentation.{}", asset.slug()),
        asset_id: asset.asset_id().to_owned(),
        coordinate_frame_id: FOUNDATION_COORDINATE_FRAME_ID.to_owned(),
        rig_id: rig.rig_id.clone(),
        socket_ids: sockets
            .iter()
            .map(|socket| socket.socket_id.clone())
            .collect(),
        missing_required_socket_roles,
        animations: source_animations,
        required_clips,
        camera_profiles: Vec::new(),
        gameplay_beats: Vec::new(),
        vfx_cues: Vec::new(),
        audio_cues: Vec::new(),
        status: "source-foundation-inventory-only".to_owned(),
        promotion_eligible: false,
        canonical_sha256: String::new(),
    };
    package.canonical_sha256 = canonical_hash_without_field(&package, "canonical_sha256");
    package
}

fn parse_skin(
    root: &Value,
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
) -> ImportResult<Option<ParsedSkin>> {
    let skins = optional_array(root, "skins")?;
    if skins.len() > MAX_SKIN_COUNT {
        return Err(budget("skin count"));
    }
    if skins.is_empty() {
        return Ok(None);
    }
    if skins.len() != 1 {
        return Err(invalid("foundation source must contain exactly one skin"));
    }
    let skin = object(&skins[0], "skin")?;
    reject_unknown_fields(
        skin,
        &[
            "extensions",
            "extras",
            "inverseBindMatrices",
            "joints",
            "name",
            "skeleton",
        ],
        "skin",
    )?;
    let joints = skin
        .get("joints")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("skin joints are missing"))?;
    if joints.is_empty() || joints.len() > MAX_JOINT_COUNT {
        return Err(budget("skin joint count"));
    }
    let mut joint_indices = Vec::with_capacity(joints.len());
    let mut seen = HashSet::new();
    for value in joints {
        let index = index_from_value(value, "skin joint index")?;
        if !seen.insert(index) {
            return Err(invalid("skin joints contain a duplicate"));
        }
        joint_indices.push(index);
    }
    let inverse_bind_matrices = skin
        .get("inverseBindMatrices")
        .map(|value| index_from_value(value, "inverse bind accessor"))
        .transpose()?;
    let inverse_bind_matrices_values = if let Some(accessor_index) = inverse_bind_matrices {
        let values = read_mat4_accessor(accessors, views, binary, accessor_index)?;
        if values.len() != joint_indices.len() {
            return Err(invalid(
                "inverse bind matrix count differs from joint count",
            ));
        }
        Some(values)
    } else {
        None
    };
    Ok(Some(ParsedSkin {
        index: 0,
        name: skin.get("name").and_then(Value::as_str).map(str::to_owned),
        joints: joint_indices,
        inverse_bind_matrices,
        inverse_bind_matrices_values,
    }))
}

#[derive(Debug)]
struct ParsedSkin {
    index: usize,
    name: Option<String>,
    joints: Vec<usize>,
    inverse_bind_matrices: Option<usize>,
    inverse_bind_matrices_values: Option<Vec<Mat4>>,
}

fn build_rig(
    asset: BuiltinWeaponFoundationAsset,
    nodes: &NodeGraph,
    skin: Option<&ParsedSkin>,
) -> ImportResult<FoundationRig> {
    let Some(skin) = skin else {
        let mut rig = FoundationRig {
            schema_version: "WeaponPresentationRig@1".to_owned(),
            rig_id: format!("forgecad.foundation.rig.{}.unskinned", asset.slug()),
            source_skin_index: None,
            source_skin_name: None,
            root_bone_id: None,
            bones: Vec::new(),
            skeleton_sha256: canonical_json_hash(&Value::Array(Vec::new())),
            inverse_bind_matrices_sha256: None,
            rest_pose_sha256: canonical_json_hash(&Value::Array(Vec::new())),
            canonical_sha256: String::new(),
        };
        rig.canonical_sha256 = canonical_hash_without_field(&rig, "canonical_sha256");
        return Ok(rig);
    };
    let joint_set = skin.joints.iter().copied().collect::<HashSet<_>>();
    if skin.joints.iter().any(|index| *index >= nodes.names.len()) {
        return Err(invalid("skin joint index is outside nodes"));
    }
    let roots = skin
        .joints
        .iter()
        .copied()
        .filter(|index| nodes.parents[*index].is_none_or(|parent| !joint_set.contains(&parent)))
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(invalid("skin must have exactly one skeleton root"));
    }
    let mut bones = Vec::with_capacity(skin.joints.len());
    for index in &skin.joints {
        let parent_bone_id = nodes.parents[*index]
            .filter(|parent| joint_set.contains(parent))
            .map(|parent| nodes.stable_ids[parent].clone());
        let mut bone = FoundationRigBone {
            bone_id: nodes.stable_ids[*index].clone(),
            source_node_index: *index,
            source_name: nodes.names[*index].clone(),
            parent_bone_id,
            local_transform_m: nodes.locals_target[*index],
            world_transform_m: nodes.worlds_target[*index],
            canonical_sha256: String::new(),
        };
        bone.canonical_sha256 = canonical_hash_without_field(&bone, "canonical_sha256");
        bones.push(bone);
    }
    let skeleton_value = bones
        .iter()
        .map(|bone| {
            serde_json::json!({
                "bone_id":bone.bone_id,
                "source_node_index":bone.source_node_index,
                "parent_bone_id":bone.parent_bone_id,
            })
        })
        .collect::<Vec<_>>();
    let skeleton_sha256 = canonical_json_hash(&Value::Array(skeleton_value));
    let inverse_bind_matrices_sha256 = skin.inverse_bind_matrices_values.as_ref().map(|values| {
        let value = values
            .iter()
            .map(|matrix| serde_json::json!(matrix))
            .collect::<Vec<_>>();
        canonical_json_hash(&Value::Array(value))
    });
    let rest_pose_value = serde_json::json!({
        "skeleton_sha256":skeleton_sha256,
        "bones":&bones,
        "inverse_bind_matrices_sha256":inverse_bind_matrices_sha256,
    });
    let rest_pose_sha256 = canonical_json_hash(&rest_pose_value);
    let mut rig = FoundationRig {
        schema_version: "WeaponPresentationRig@1".to_owned(),
        rig_id: format!("forgecad.foundation.rig.{}", asset.slug()),
        source_skin_index: Some(skin.index),
        source_skin_name: skin.name.clone(),
        root_bone_id: Some(nodes.stable_ids[roots[0]].clone()),
        bones,
        skeleton_sha256,
        inverse_bind_matrices_sha256,
        rest_pose_sha256,
        canonical_sha256: String::new(),
    };
    rig.canonical_sha256 = canonical_hash_without_field(&rig, "canonical_sha256");
    Ok(rig)
}

struct DecodedMeshes {
    meshes: Vec<FoundationMesh>,
    source_triangle_count: usize,
    removed_face_indices: Vec<usize>,
}

fn decode_meshes(
    asset: BuiltinWeaponFoundationAsset,
    root: &Value,
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    nodes: &NodeGraph,
    skin: Option<&ParsedSkin>,
) -> ImportResult<DecodedMeshes> {
    let meshes_value = optional_array(root, "meshes")?;
    if meshes_value.is_empty() || meshes_value.len() > MAX_MESH_COUNT {
        return Err(budget("mesh count"));
    }
    let primitive_count = meshes_value
        .iter()
        .map(|mesh| {
            mesh.get("primitives")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        })
        .sum::<usize>();
    if primitive_count > MAX_PRIMITIVE_COUNT {
        return Err(budget("primitive count"));
    }
    let mesh_nodes = nodes
        .meshes
        .iter()
        .enumerate()
        .filter_map(|(index, mesh)| mesh.map(|mesh_index| (index, mesh_index)))
        .collect::<Vec<_>>();
    if mesh_nodes.is_empty() {
        return Err(invalid("source contains no mesh-bearing scene node"));
    }
    let mut output = Vec::with_capacity(mesh_nodes.len());
    let mut total_source_triangle_count = 0usize;
    let mut all_removed_face_indices = Vec::new();
    for (node_index, mesh_index) in mesh_nodes {
        let mesh_object = object(
            meshes_value
                .get(mesh_index)
                .ok_or_else(|| invalid("node mesh index is out of bounds"))?,
            "mesh",
        )?;
        reject_unknown_fields(
            mesh_object,
            &["extensions", "extras", "name", "primitives", "weights"],
            "mesh",
        )?;
        if mesh_object.contains_key("weights") {
            return Err(invalid("mesh morph target weights are not supported"));
        }
        let primitives = mesh_object
            .get("primitives")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("mesh primitives are missing"))?;
        if primitives.is_empty() {
            return Err(invalid("mesh has no primitives"));
        }
        let part_id = nodes.stable_ids[node_index].clone();
        let mesh_id = format!(
            "forgecad.foundation.mesh.{}.{}",
            asset.slug(),
            slug(&nodes.names[node_index])
        );
        let mut positions = Vec::new();
        let mut faces = Vec::new();
        let mut face_material_indices = Vec::new();
        let mut source_triangle_count = 0usize;
        let mut removed_face_indices = Vec::new();
        let mut attribute_descriptors = Vec::new();
        for (primitive_index, primitive_value) in primitives.iter().enumerate() {
            let primitive = object(primitive_value, "primitive")?;
            reject_unknown_fields(
                primitive,
                &[
                    "attributes",
                    "extensions",
                    "extras",
                    "indices",
                    "material",
                    "mode",
                    "targets",
                ],
                "primitive",
            )?;
            if primitive.contains_key("targets") {
                return Err(invalid("morph target primitive is not supported"));
            }
            let mode = primitive
                .get("mode")
                .map(|value| integer_value(value, "primitive mode"))
                .transpose()?
                .unwrap_or(4);
            if mode != 4 {
                return Err(invalid("only TRIANGLES primitives are supported"));
            }
            let attributes = object(
                primitive
                    .get("attributes")
                    .ok_or_else(|| invalid("primitive attributes are missing"))?,
                "primitive attributes",
            )?;
            validate_attribute_keys(attributes)?;
            let position_accessor = required_index(attributes, "POSITION")?;
            let primitive_positions =
                read_vec3_accessor(accessors, views, binary, position_accessor)?;
            if primitive_positions.is_empty() {
                return Err(invalid("primitive POSITION accessor is empty"));
            }
            let source_position_count = primitive_positions.len();
            let normal_accessor = optional_attribute_index(attributes, "NORMAL")?;
            let uv_accessor = optional_attribute_index(attributes, "TEXCOORD_0")?;
            let tangent_accessor = optional_attribute_index(attributes, "TANGENT")?;
            let weights_accessor = optional_attribute_index(attributes, "WEIGHTS_0")?;
            let joints_accessor = optional_attribute_index(attributes, "JOINTS_0")?;
            let color0_accessor = optional_attribute_index(attributes, "COLOR_0")?;
            let color1_accessor = optional_attribute_index(attributes, "COLOR_1")?;
            if weights_accessor.is_some() != joints_accessor.is_some() {
                return Err(invalid("JOINTS_0 and WEIGHTS_0 must appear together"));
            }
            if let Some(accessor) = normal_accessor {
                validate_finite_vec3_accessor(accessors, views, binary, accessor, "NORMAL")?;
            }
            if let Some(accessor) = uv_accessor {
                validate_finite_vec2_accessor(accessors, views, binary, accessor, "TEXCOORD_0")?;
            }
            if let Some(accessor) = tangent_accessor {
                validate_finite_vec4_accessor(accessors, views, binary, accessor, "TANGENT")?;
            }
            if let Some(accessor) = color0_accessor {
                validate_color_accessor(accessors, views, binary, accessor, "COLOR_0")?;
            }
            if let Some(accessor) = color1_accessor {
                validate_color_accessor(accessors, views, binary, accessor, "COLOR_1")?;
            }
            if let (Some(joints_accessor), Some(weights_accessor)) =
                (joints_accessor, weights_accessor)
            {
                validate_skin_attributes(
                    accessors,
                    views,
                    binary,
                    joints_accessor,
                    weights_accessor,
                    source_position_count,
                    skin,
                )?;
            }
            attribute_descriptors.push(serde_json::json!({
                "primitive_index":primitive_index,
                "position_accessor":position_accessor,
                "normal_accessor":normal_accessor,
                "uv_accessor":uv_accessor,
                "tangent_accessor":tangent_accessor,
                "joints_accessor":joints_accessor,
                "weights_accessor":weights_accessor,
                "color0_accessor":color0_accessor,
                "color1_accessor":color1_accessor,
            }));
            let indices_accessor = primitive
                .get("indices")
                .ok_or_else(|| invalid("primitive index accessor is missing"))
                .and_then(|value| index_from_value(value, "primitive index accessor"))?;
            let indices = read_indices_accessor(accessors, views, binary, indices_accessor)?;
            if indices.is_empty() || indices.len() % 3 != 0 {
                return Err(invalid(
                    "primitive indices are not a non-empty triangle list",
                ));
            }
            let material_index = primitive
                .get("material")
                .map(|value| integer_value(value, "primitive material index"))
                .transpose()?
                .unwrap_or(0);
            let base = u32::try_from(positions.len()).map_err(|_| budget("vertex index"))?;
            let primitive_triangle_base = source_triangle_count;
            source_triangle_count = source_triangle_count
                .checked_add(indices.len() / 3)
                .ok_or_else(|| budget("triangle count"))?;
            let policy = asset.coordinate_policy();
            let transformed_primitive_positions = primitive_positions
                .iter()
                .copied()
                .map(|position| transform_point(nodes.worlds_source[node_index], position, policy))
                .collect::<Vec<_>>();
            for (triangle_index, triangle) in indices.chunks_exact(3).enumerate() {
                for index in triangle {
                    if (*index as usize) >= primitive_positions.len() {
                        return Err(invalid("primitive index is outside POSITION accessor"));
                    }
                }
                let a = transformed_primitive_positions[triangle[0] as usize];
                let b = transformed_primitive_positions[triangle[1] as usize];
                let c = transformed_primitive_positions[triangle[2] as usize];
                let area = triangle_area(a, b, c);
                let local_face_index = primitive_triangle_base + triangle_index;
                if !area.is_finite() {
                    return Err(invalid("triangle area is non-finite"));
                }
                if area < DEGENERATE_AREA_EPSILON {
                    removed_face_indices.push(local_face_index);
                    continue;
                }
                let shifted = [
                    base.checked_add(triangle[0])
                        .ok_or_else(|| budget("vertex index"))?,
                    base.checked_add(triangle[1])
                        .ok_or_else(|| budget("vertex index"))?,
                    base.checked_add(triangle[2])
                        .ok_or_else(|| budget("vertex index"))?,
                ];
                faces.push(shifted);
                face_material_indices.push(material_index as u32);
            }
            positions.extend(transformed_primitive_positions);
        }
        if positions.len() > MAX_VERTEX_COUNT {
            return Err(budget("mesh vertex count"));
        }
        if faces.is_empty() {
            return Err(invalid("mesh has no non-degenerate triangles"));
        }
        if faces.len() > MAX_TRIANGLE_COUNT {
            return Err(budget("mesh triangle count"));
        }
        let (boundary_edge_count, non_manifold_edge_count) = validate_topology(&faces)?;
        if non_manifold_edge_count != 0 {
            return Err(invalid("mesh contains a non-manifold or same-winding edge"));
        }
        let has_normals = attribute_descriptors
            .iter()
            .any(|value| !value["normal_accessor"].is_null());
        let has_uv0 = attribute_descriptors
            .iter()
            .any(|value| !value["uv_accessor"].is_null());
        let has_tangents = attribute_descriptors
            .iter()
            .any(|value| !value["tangent_accessor"].is_null());
        let has_skin_weights = attribute_descriptors
            .iter()
            .any(|value| !value["weights_accessor"].is_null());
        let has_vertex_colors = attribute_descriptors.iter().any(|value| {
            !value["color0_accessor"].is_null() || !value["color1_accessor"].is_null()
        });
        let attributes = FoundationAttributeInventory {
            source_vertex_count: positions.len(),
            has_normals,
            has_uv0,
            has_tangents,
            has_skin_weights,
            has_vertex_colors,
            source_attribute_sha256: canonical_json_hash(&Value::Array(attribute_descriptors)),
        };
        let topology_value = serde_json::json!({
            "policy":FOUNDATION_TOPOLOGY_POLICY,
            "part_id":&part_id,
            "positions_m":&positions,
            "faces":&faces,
            "face_material_indices":&face_material_indices,
            "removed_face_indices":&removed_face_indices,
        });
        let topology_sha256 = canonical_json_hash(&topology_value);
        let topology = FoundationTopologyInventory {
            policy: FOUNDATION_TOPOLOGY_POLICY.to_owned(),
            vertex_id_scheme: format!("forgecad.vertex:{}:ordinal", part_id),
            face_id_scheme: format!("forgecad.face:{}:ordinal", part_id),
            vertex_count: positions.len(),
            triangle_count: faces.len(),
            boundary_edge_count,
            non_manifold_edge_count,
            topology_sha256,
        };
        let mut mesh = FoundationMesh {
            mesh_id,
            part_id,
            source_mesh_index: mesh_index,
            source_node_index: node_index,
            source_skin_index: nodes.skins[node_index].or_else(|| skin.map(|value| value.index)),
            positions_m: positions,
            faces,
            face_material_indices,
            world_transform_m: nodes.worlds_target[node_index],
            attributes,
            topology,
            canonical_sha256: String::new(),
        };
        mesh.canonical_sha256 = canonical_hash_without_field(&mesh, "canonical_sha256");
        let mesh_triangle_base = total_source_triangle_count;
        total_source_triangle_count = total_source_triangle_count
            .checked_add(source_triangle_count)
            .ok_or_else(|| budget("triangle count"))?;
        all_removed_face_indices.extend(
            removed_face_indices
                .into_iter()
                .map(|index| mesh_triangle_base + index),
        );
        output.push(mesh);
    }
    Ok(DecodedMeshes {
        meshes: output,
        source_triangle_count: total_source_triangle_count,
        removed_face_indices: all_removed_face_indices,
    })
}

fn sanitation(source_triangle_count: usize, removed_indices: &[usize]) -> FoundationSanitation {
    let mut sanitation = FoundationSanitation {
        policy: FOUNDATION_TOPOLOGY_POLICY.to_owned(),
        source_triangle_count,
        degenerate_faces_removed: removed_indices.len(),
        removed_face_index_sha256: canonical_json_hash(&serde_json::json!(removed_indices)),
        canonical_sha256: String::new(),
    };
    sanitation.canonical_sha256 = canonical_hash_without_field(&sanitation, "canonical_sha256");
    sanitation
}

fn source_inventory(
    asset: BuiltinWeaponFoundationAsset,
    byte_length: usize,
    source_sha256: &str,
    parsed: &ParsedGlb<'_>,
    imported_mesh_count: usize,
    skin: Option<&ParsedSkin>,
    animation_count: usize,
    pbr: &FoundationPbrInventory,
) -> FoundationSourceInventory {
    let meshes = optional_array(&parsed.root, "meshes").map_or(0, Vec::len);
    let primitives = optional_array(&parsed.root, "meshes")
        .ok()
        .map(|values| {
            values
                .iter()
                .map(|mesh| {
                    mesh.get("primitives")
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len)
                })
                .sum()
        })
        .unwrap_or(imported_mesh_count);
    let mut source = FoundationSourceInventory {
        asset_id: asset.asset_id().to_owned(),
        source_sha256: source_sha256.to_owned(),
        expected_sha256: asset.expected_sha256().to_owned(),
        byte_length,
        node_count: optional_array(&parsed.root, "nodes").map_or(0, Vec::len),
        mesh_count: meshes,
        primitive_count: primitives,
        material_count: pbr.materials.len(),
        texture_count: pbr.textures.len(),
        image_count: pbr.images.len(),
        skin_count: usize::from(skin.is_some()),
        animation_count,
        canonical_sha256: String::new(),
    };
    source.canonical_sha256 = canonical_hash_without_field(&source, "canonical_sha256");
    source
}

fn decode_animations(
    asset: BuiltinWeaponFoundationAsset,
    root: &Value,
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    nodes: &NodeGraph,
) -> ImportResult<Vec<FoundationAnimation>> {
    let animations_value = optional_array(root, "animations")?;
    if animations_value.len() > MAX_ANIMATION_COUNT {
        return Err(budget("animation count"));
    }
    let mut output = Vec::with_capacity(animations_value.len());
    for (animation_index, animation_value) in animations_value.iter().enumerate() {
        let animation = object(animation_value, "animation")?;
        reject_unknown_fields(
            animation,
            &["channels", "extensions", "extras", "name", "samplers"],
            "animation",
        )?;
        let samplers = animation
            .get("samplers")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("animation samplers are missing"))?;
        let channels = animation
            .get("channels")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("animation channels are missing"))?;
        if channels.is_empty() || channels.len() > MAX_ANIMATION_CHANNEL_COUNT {
            return Err(budget("animation channel count"));
        }
        let source_name = animation
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                if animation_index == 0 {
                    "animation"
                } else {
                    "clip"
                }
            })
            .to_owned();
        let mut used_samplers = HashSet::new();
        let mut target_pairs = HashSet::new();
        let mut decoded_channels = Vec::with_capacity(channels.len());
        let mut duration: f64 = 0.0;
        for channel_value in channels {
            let channel = object(channel_value, "animation channel")?;
            reject_unknown_fields(
                channel,
                &["extensions", "extras", "sampler", "target"],
                "animation channel",
            )?;
            let sampler_index = index_from_value(
                channel
                    .get("sampler")
                    .ok_or_else(|| invalid("animation channel sampler is missing"))?,
                "animation sampler index",
            )?;
            let sampler = object(
                samplers
                    .get(sampler_index)
                    .ok_or_else(|| invalid("animation sampler index is out of bounds"))?,
                "animation sampler",
            )?;
            reject_unknown_fields(
                sampler,
                &["extensions", "extras", "input", "interpolation", "output"],
                "animation sampler",
            )?;
            let interpolation = sampler
                .get("interpolation")
                .and_then(Value::as_str)
                .unwrap_or("LINEAR");
            if !matches!(interpolation, "LINEAR" | "STEP") {
                return Err(invalid(format!(
                    "animation interpolation {interpolation} is unsupported"
                )));
            }
            let input_accessor = index_from_value(
                sampler
                    .get("input")
                    .ok_or_else(|| invalid("animation sampler input is missing"))?,
                "animation input accessor",
            )?;
            let output_accessor = index_from_value(
                sampler
                    .get("output")
                    .ok_or_else(|| invalid("animation sampler output is missing"))?,
                "animation output accessor",
            )?;
            let times = read_scalar_accessor(accessors, views, binary, input_accessor)?;
            if times.is_empty() || times.len() > MAX_ANIMATION_KEY_COUNT {
                return Err(budget("animation key count"));
            }
            if times
                .windows(2)
                .any(|window| window[1] < window[0] || !window[0].is_finite())
                || !times.last().is_some_and(|time| time.is_finite())
                || times[0] < 0.0
            {
                return Err(invalid("animation input times are not finite and monotone"));
            }
            duration = duration.max(*times.last().unwrap_or(&0.0));
            let target = object(
                channel
                    .get("target")
                    .ok_or_else(|| invalid("animation channel target is missing"))?,
                "animation target",
            )?;
            reject_unknown_fields(
                target,
                &["extensions", "extras", "node", "path"],
                "animation target",
            )?;
            let node_index = index_from_value(
                target
                    .get("node")
                    .ok_or_else(|| invalid("animation target node is missing"))?,
                "animation target node",
            )?;
            if node_index >= nodes.names.len() {
                return Err(invalid("animation target node is out of bounds"));
            }
            let path = target
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("animation target path is missing"))?;
            if !matches!(path, "translation" | "rotation" | "scale") {
                return Err(invalid(format!(
                    "animation target path {path} is unsupported"
                )));
            }
            if !target_pairs.insert((node_index, path.to_owned())) {
                return Err(invalid("animation has duplicate node/path channels"));
            }
            let normalized_value_sha256 = match path {
                "translation" => {
                    let source = read_vec3_accessor(accessors, views, binary, output_accessor)?;
                    if source.len() != times.len() {
                        return Err(invalid("translation key count differs from input"));
                    }
                    let values = source
                        .into_iter()
                        .map(|value| transform_vector(value, asset.coordinate_policy()))
                        .collect::<Vec<_>>();
                    Ok(canonical_json_hash(&serde_json::json!({
                        "times_seconds": &times,
                        "values": &values,
                    })))
                }
                "scale" => {
                    let source = read_vec3_accessor(accessors, views, binary, output_accessor)?;
                    if source.len() != times.len() {
                        return Err(invalid("scale key count differs from input"));
                    }
                    let values = source
                        .into_iter()
                        .map(|value| [value[2], value[1], value[0]])
                        .collect::<Vec<_>>();
                    Ok(canonical_json_hash(&serde_json::json!({
                        "times_seconds": &times,
                        "values": &values,
                    })))
                }
                "rotation" => {
                    let source = read_vec4_accessor(accessors, views, binary, output_accessor)?;
                    if source.len() != times.len() {
                        return Err(invalid("rotation key count differs from input"));
                    }
                    let values = source
                        .into_iter()
                        .map(|value| {
                            let rotation = quaternion_to_matrix(value)?;
                            Ok(convert_matrix(rotation, asset.coordinate_policy()))
                        })
                        .collect::<ImportResult<Vec<_>>>()?;
                    Ok(canonical_json_hash(&serde_json::json!({
                        "times_seconds": &times,
                        "values": &values,
                    })))
                }
                _ => unreachable!(),
            }?;
            used_samplers.insert(sampler_index);
            decoded_channels.push(FoundationAnimationChannel {
                stable_node_id: nodes.stable_ids[node_index].clone(),
                source_node_index: node_index,
                path: path.to_owned(),
                interpolation: interpolation.to_owned(),
                key_count: times.len(),
                time_range_seconds: [times[0], *times.last().unwrap_or(&times[0])],
                input_accessor,
                output_accessor,
                normalized_value_sha256,
            });
        }
        if duration > MAX_ANIMATION_DURATION_SECONDS {
            return Err(budget("animation duration"));
        }
        if used_samplers.len() != samplers.len() {
            return Err(invalid("animation contains an unreferenced sampler"));
        }
        let clip_id = format!(
            "forgecad.foundation.clip.{}.{}.{}",
            asset.slug(),
            slug(&source_name),
            animation_index
        );
        let semantic_clip_id = semantic_clip_id(&source_name);
        let mut animation = FoundationAnimation {
            clip_id,
            semantic_clip_id,
            source_animation_index: animation_index,
            source_name,
            duration_seconds: duration,
            channels: decoded_channels,
            canonical_sha256: String::new(),
        };
        animation.canonical_sha256 = canonical_hash_without_field(&animation, "canonical_sha256");
        output.push(animation);
    }
    Ok(output)
}

fn semantic_clip_id(source_name: &str) -> String {
    match source_name.to_ascii_lowercase().as_str() {
        "fire" => "fire_recoil".to_owned(),
        "pump" => "reload".to_owned(),
        _ => slug(source_name),
    }
}

fn decode_pbr_inventory(
    root: &Value,
    views: &[Value],
    binary: &[u8],
) -> ImportResult<FoundationPbrInventory> {
    let images_value = optional_array(root, "images")?;
    let textures_value = optional_array(root, "textures")?;
    let materials_value = optional_array(root, "materials")?;
    if images_value.len() > MAX_IMAGE_COUNT {
        return Err(budget("image count"));
    }
    if textures_value.len() > MAX_TEXTURE_COUNT {
        return Err(budget("texture count"));
    }
    if materials_value.len() > MAX_MATERIAL_COUNT {
        return Err(budget("material count"));
    }
    let mut images = Vec::with_capacity(images_value.len());
    for (image_index, image_value) in images_value.iter().enumerate() {
        let image = object(image_value, "image")?;
        reject_unknown_fields(
            image,
            &["bufferView", "extensions", "extras", "mimeType", "name"],
            "image",
        )?;
        if image.contains_key("uri") {
            return Err(invalid("image URI is forbidden"));
        }
        let view_index = index_from_value(
            image
                .get("bufferView")
                .ok_or_else(|| invalid("GLB image must use an embedded bufferView"))?,
            "image bufferView index",
        )?;
        let payload = read_buffer_view_bytes(views, binary, view_index)?;
        let mime_type = image
            .get("mimeType")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("embedded image mimeType is missing"))?;
        if !matches!(
            mime_type,
            "image/png" | "image/jpeg" | "image/webp" | "image/ktx2"
        ) {
            return Err(invalid(format!(
                "embedded image mimeType {mime_type} is unsupported"
            )));
        }
        images.push(FoundationImageInventory {
            image_index,
            name: image
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| if image_index == 0 { "image" } else { "texture" })
                .to_owned(),
            mime_type: mime_type.to_owned(),
            byte_length: payload.len(),
            embedded_bytes_sha256: sha256_hex(payload),
        });
    }
    let mut textures = Vec::with_capacity(textures_value.len());
    for (texture_index, texture_value) in textures_value.iter().enumerate() {
        let texture = object(texture_value, "texture")?;
        reject_unknown_fields(
            texture,
            &["extensions", "extras", "name", "sampler", "source"],
            "texture",
        )?;
        let source_image_index = index_from_value(
            texture
                .get("source")
                .ok_or_else(|| invalid("texture source image is missing"))?,
            "texture source image index",
        )?;
        if source_image_index >= images.len() {
            return Err(invalid("texture source image index is out of bounds"));
        }
        let sampler_index = texture
            .get("sampler")
            .map(|value| index_from_value(value, "texture sampler index"))
            .transpose()?;
        let mut inventory = FoundationTextureInventory {
            texture_index,
            source_image_index,
            sampler_index,
            canonical_sha256: String::new(),
        };
        inventory.canonical_sha256 = canonical_hash_without_field(&inventory, "canonical_sha256");
        textures.push(inventory);
    }
    let mut materials = Vec::with_capacity(materials_value.len());
    for (material_index, material_value) in materials_value.iter().enumerate() {
        let material = object(material_value, "material")?;
        reject_unknown_fields(
            material,
            &[
                "alphaCutoff",
                "alphaMode",
                "doubleSided",
                "emissiveFactor",
                "emissiveTexture",
                "extensions",
                "extras",
                "name",
                "normalTexture",
                "occlusionTexture",
                "pbrMetallicRoughness",
            ],
            "material",
        )?;
        let pbr = material
            .get("pbrMetallicRoughness")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("material pbrMetallicRoughness is missing"))?;
        reject_unknown_fields(
            pbr,
            &[
                "baseColorFactor",
                "baseColorTexture",
                "extensions",
                "extras",
                "metallicFactor",
                "metallicRoughnessTexture",
                "roughnessFactor",
            ],
            "pbrMetallicRoughness",
        )?;
        let base_color_factor = optional_vec4(pbr, "baseColorFactor")?.unwrap_or([1.0; 4]);
        let metallic_factor = optional_finite_number(pbr, "metallicFactor")?.unwrap_or(1.0);
        let roughness_factor = optional_finite_number(pbr, "roughnessFactor")?.unwrap_or(1.0);
        if !(0.0..=1.0).contains(&metallic_factor) || !(0.0..=1.0).contains(&roughness_factor) {
            return Err(invalid(
                "material metallic/roughness factors are outside [0,1]",
            ));
        }
        let base_color_texture = texture_ref(pbr, "baseColorTexture", &textures)?;
        let metallic_roughness_texture = texture_ref(pbr, "metallicRoughnessTexture", &textures)?;
        let normal_texture = texture_ref(material, "normalTexture", &textures)?;
        let occlusion_texture = texture_ref(material, "occlusionTexture", &textures)?;
        let emissive_texture = texture_ref(material, "emissiveTexture", &textures)?;
        let mut inventory = FoundationMaterialInventory {
            material_index,
            name: material
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| {
                    if material_index == 0 {
                        "material"
                    } else {
                        "surface"
                    }
                })
                .to_owned(),
            base_color_factor,
            metallic_factor,
            roughness_factor,
            base_color_texture,
            metallic_roughness_texture,
            normal_texture,
            occlusion_texture,
            emissive_texture,
            canonical_sha256: String::new(),
        };
        inventory.canonical_sha256 = canonical_hash_without_field(&inventory, "canonical_sha256");
        materials.push(inventory);
    }
    let material_inventory_sha256 = canonical_json_hash(&serde_json::json!(materials));
    let texture_inventory_sha256 = canonical_json_hash(&serde_json::json!(textures));
    let image_inventory_sha256 = canonical_json_hash(&serde_json::json!(images));
    let mut inventory = FoundationPbrInventory {
        schema_version: "PbrInventory@1".to_owned(),
        materials,
        textures,
        images,
        material_inventory_sha256,
        texture_inventory_sha256,
        image_inventory_sha256,
        canonical_sha256: String::new(),
    };
    inventory.canonical_sha256 = canonical_hash_without_field(&inventory, "canonical_sha256");
    Ok(inventory)
}

fn texture_ref(
    parent: &Map<String, Value>,
    key: &str,
    textures: &[FoundationTextureInventory],
) -> ImportResult<Option<usize>> {
    let Some(value) = parent.get(key) else {
        return Ok(None);
    };
    let texture_object = object(value, "texture reference")?;
    reject_unknown_fields(
        texture_object,
        &[
            "extensions",
            "extras",
            "index",
            "texCoord",
            "scale",
            "strength",
        ],
        "texture reference",
    )?;
    let index = index_from_value(
        texture_object
            .get("index")
            .ok_or_else(|| invalid("texture reference index is missing"))?,
        "texture reference index",
    )?;
    if index >= textures.len() {
        return Err(invalid("texture reference index is out of bounds"));
    }
    Ok(Some(index))
}

fn validate_root_shape(root: &Value) -> ImportResult<()> {
    let object = object(root, "GLB root")?;
    reject_unknown_fields(
        object,
        &[
            "accessors",
            "animations",
            "asset",
            "buffers",
            "bufferViews",
            "cameras",
            "extensions",
            "extensionsRequired",
            "extensionsUsed",
            "extras",
            "images",
            "materials",
            "meshes",
            "nodes",
            "scene",
            "scenes",
            "samplers",
            "skins",
            "textures",
        ],
        "GLB root",
    )?;
    let asset = object
        .get("asset")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("GLB asset object is missing"))?;
    if asset.get("version").and_then(Value::as_str) != Some("2.0") {
        return Err(invalid("GLB asset.version must be 2.0"));
    }
    for key in ["extensionsRequired", "extensionsUsed"] {
        if object
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
        {
            return Err(invalid(format!(
                "GLB {key} is unsupported by the closed importer"
            )));
        }
    }
    Ok(())
}

fn validate_buffer(root: &Value, binary: &[u8]) -> ImportResult<()> {
    let buffers = required_array(root, "buffers")?;
    if buffers.len() != 1 {
        return Err(invalid("GLB must contain exactly one buffer"));
    }
    let buffer = object(&buffers[0], "buffer")?;
    reject_unknown_fields(
        buffer,
        &["byteLength", "extensions", "extras", "name"],
        "buffer",
    )?;
    if buffer.contains_key("uri") {
        return Err(invalid("buffer URI is forbidden"));
    }
    let byte_length = usize_field(buffer, "byteLength")?;
    if byte_length != binary.len() {
        return Err(invalid(
            "buffer.byteLength differs from embedded BIN length",
        ));
    }
    Ok(())
}

fn decode_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
    expected_type: &str,
    allowed_component_types: &[u64],
) -> ImportResult<AccessorDescriptor> {
    let accessor = object(
        accessors
            .get(accessor_index)
            .ok_or_else(|| invalid("accessor index is out of bounds"))?,
        "accessor",
    )?;
    reject_unknown_fields(
        accessor,
        &[
            "bufferView",
            "byteOffset",
            "componentType",
            "count",
            "extensions",
            "extras",
            "max",
            "min",
            "name",
            "normalized",
            "sparse",
            "type",
        ],
        "accessor",
    )?;
    if accessor.contains_key("sparse") {
        return Err(invalid("sparse accessors are unsupported"));
    }
    let component_type = integer_field(accessor, "componentType")?;
    if !allowed_component_types.contains(&component_type) {
        return Err(invalid(format!(
            "accessor componentType {component_type} is unsupported"
        )));
    }
    if accessor.get("type").and_then(Value::as_str) != Some(expected_type) {
        return Err(invalid(format!("accessor type is not {expected_type}")));
    }
    let count = usize_field(accessor, "count")?;
    if count == 0 || count > MAX_ANIMATION_KEY_COUNT.max(MAX_VERTEX_COUNT) {
        return Err(budget("accessor count"));
    }
    let normalized = accessor
        .get("normalized")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if normalized && component_type == 5126 {
        return Err(invalid("float accessor cannot be normalized"));
    }
    let view_index = index_from_value(
        accessor
            .get("bufferView")
            .ok_or_else(|| invalid("accessor bufferView is missing"))?,
        "accessor bufferView index",
    )?;
    let view = object(
        views
            .get(view_index)
            .ok_or_else(|| invalid("accessor bufferView index is out of bounds"))?,
        "bufferView",
    )?;
    reject_unknown_fields(
        view,
        &[
            "buffer",
            "byteLength",
            "byteOffset",
            "byteStride",
            "extensions",
            "extras",
            "name",
            "target",
        ],
        "bufferView",
    )?;
    if integer_field(view, "buffer")? != 0 {
        return Err(invalid("bufferView must reference embedded buffer 0"));
    }
    let element_size = component_size(component_type)?
        .checked_mul(component_count(expected_type)?)
        .ok_or_else(|| budget("accessor element size"))?;
    let view_offset = optional_usize_field(view, "byteOffset")?.unwrap_or(0);
    let view_length = usize_field(view, "byteLength")?;
    let accessor_offset = optional_usize_field(accessor, "byteOffset")?.unwrap_or(0);
    let explicit_stride = optional_usize_field(view, "byteStride")?;
    let stride = explicit_stride.unwrap_or(element_size);
    if let Some(stride) = explicit_stride {
        if stride < element_size || stride > 252 || stride % 4 != 0 {
            return Err(invalid("bufferView byteStride is invalid"));
        }
    }
    if accessor_offset % component_size(component_type)? != 0 {
        return Err(invalid("accessor byteOffset is not component-aligned"));
    }
    if accessor_offset >= stride && count > 1 {
        return Err(invalid("accessor byteOffset exceeds byteStride"));
    }
    let start = view_offset
        .checked_add(accessor_offset)
        .ok_or_else(|| invalid("accessor byte offset overflows"))?;
    let span = stride
        .checked_mul(count.saturating_sub(1))
        .and_then(|value| value.checked_add(element_size))
        .ok_or_else(|| budget("accessor byte span"))?;
    if span > view_length {
        return Err(invalid("accessor bytes exceed bufferView"));
    }
    checked_range(start, span, binary.len(), "accessor bytes")?;
    Ok(AccessorDescriptor {
        component_type,
        normalized,
        count,
        type_name: expected_type.to_owned(),
        start,
        stride,
        element_size,
    })
}

#[derive(Debug, Clone)]
struct AccessorDescriptor {
    component_type: u64,
    normalized: bool,
    count: usize,
    type_name: String,
    start: usize,
    stride: usize,
    element_size: usize,
}

fn read_vec2_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
) -> ImportResult<Vec<[f64; 2]>> {
    let descriptor = decode_accessor(accessors, views, binary, accessor_index, "VEC2", &[5126])?;
    let mut output = Vec::with_capacity(descriptor.count);
    for index in 0..descriptor.count {
        let offset = descriptor.start + index * descriptor.stride;
        let value = [read_f32(binary, offset)?, read_f32(binary, offset + 4)?];
        if value.iter().any(|value| !value.is_finite()) {
            return Err(invalid("VEC2 contains a non-finite value"));
        }
        output.push(value);
    }
    Ok(output)
}

fn read_vec3_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
) -> ImportResult<Vec<[f64; 3]>> {
    let descriptor = decode_accessor(accessors, views, binary, accessor_index, "VEC3", &[5126])?;
    let mut output = Vec::with_capacity(descriptor.count);
    for index in 0..descriptor.count {
        let offset = descriptor.start + index * descriptor.stride;
        let value = [
            read_f32(binary, offset)?,
            read_f32(binary, offset + 4)?,
            read_f32(binary, offset + 8)?,
        ];
        if value.iter().any(|value| !value.is_finite()) {
            return Err(invalid("VEC3 contains a non-finite value"));
        }
        output.push(value);
    }
    Ok(output)
}

fn read_vec4_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
) -> ImportResult<Vec<[f64; 4]>> {
    let descriptor = decode_accessor(accessors, views, binary, accessor_index, "VEC4", &[5126])?;
    let mut output = Vec::with_capacity(descriptor.count);
    for index in 0..descriptor.count {
        let offset = descriptor.start + index * descriptor.stride;
        let value = [
            read_f32(binary, offset)?,
            read_f32(binary, offset + 4)?,
            read_f32(binary, offset + 8)?,
            read_f32(binary, offset + 12)?,
        ];
        if value.iter().any(|value| !value.is_finite()) {
            return Err(invalid("VEC4 contains a non-finite value"));
        }
        output.push(value);
    }
    Ok(output)
}

fn read_scalar_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
) -> ImportResult<Vec<f64>> {
    let descriptor = decode_accessor(accessors, views, binary, accessor_index, "SCALAR", &[5126])?;
    let mut output = Vec::with_capacity(descriptor.count);
    for index in 0..descriptor.count {
        let value = read_f32(binary, descriptor.start + index * descriptor.stride)?;
        if !value.is_finite() {
            return Err(invalid("SCALAR contains a non-finite value"));
        }
        output.push(value);
    }
    Ok(output)
}

fn read_indices_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
) -> ImportResult<Vec<u32>> {
    let accessor = object(
        accessors
            .get(accessor_index)
            .ok_or_else(|| invalid("index accessor is out of bounds"))?,
        "index accessor",
    )?;
    let type_name = accessor
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("index accessor type is missing"))?;
    if type_name != "SCALAR" {
        return Err(invalid("index accessor must be SCALAR"));
    }
    let component_type = integer_field(accessor, "componentType")?;
    if !matches!(component_type, 5121 | 5123 | 5125) {
        return Err(invalid("index accessor componentType must be U8/U16/U32"));
    }
    let descriptor = decode_accessor(
        accessors,
        views,
        binary,
        accessor_index,
        "SCALAR",
        &[5121, 5123, 5125],
    )?;
    let mut output = Vec::with_capacity(descriptor.count);
    for index in 0..descriptor.count {
        let offset = descriptor.start + index * descriptor.stride;
        let value = match descriptor.component_type {
            5121 => binary
                .get(offset)
                .copied()
                .ok_or_else(|| invalid("U8 index is out of bounds"))? as u32,
            5123 => read_u16(binary, offset)? as u32,
            5125 => read_u32(binary, offset)?,
            _ => unreachable!(),
        };
        output.push(value);
    }
    Ok(output)
}

fn read_mat4_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
) -> ImportResult<Vec<Mat4>> {
    let descriptor = decode_accessor(accessors, views, binary, accessor_index, "MAT4", &[5126])?;
    let mut output = Vec::with_capacity(descriptor.count);
    for index in 0..descriptor.count {
        let offset = descriptor.start + index * descriptor.stride;
        let mut matrix = [0.0; 16];
        for (component, value) in matrix.iter_mut().enumerate() {
            *value = read_f32(binary, offset + component * 4)?;
        }
        ensure_matrix_bounded(matrix, "inverse bind matrix")?;
        output.push(matrix);
    }
    Ok(output)
}

fn read_buffer_view_bytes<'a>(
    views: &[Value],
    binary: &'a [u8],
    view_index: usize,
) -> ImportResult<&'a [u8]> {
    let view = object(
        views
            .get(view_index)
            .ok_or_else(|| invalid("bufferView index is out of bounds"))?,
        "bufferView",
    )?;
    let offset = optional_usize_field(view, "byteOffset")?.unwrap_or(0);
    let length = usize_field(view, "byteLength")?;
    checked_range(offset, length, binary.len(), "bufferView bytes")?;
    Ok(&binary[offset..offset + length])
}

fn validate_finite_vec2_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
    label: &str,
) -> ImportResult<()> {
    read_vec2_accessor(accessors, views, binary, accessor_index)?;
    let _ = label;
    Ok(())
}

fn validate_finite_vec3_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
    label: &str,
) -> ImportResult<()> {
    read_vec3_accessor(accessors, views, binary, accessor_index)?;
    let _ = label;
    Ok(())
}

fn validate_finite_vec4_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
    label: &str,
) -> ImportResult<()> {
    read_vec4_accessor(accessors, views, binary, accessor_index)?;
    let _ = label;
    Ok(())
}

fn validate_color_accessor(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    accessor_index: usize,
    _label: &str,
) -> ImportResult<()> {
    let accessor = object(
        accessors
            .get(accessor_index)
            .ok_or_else(|| invalid("color accessor is out of bounds"))?,
        "color accessor",
    )?;
    let type_name = accessor
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("color accessor type is missing"))?;
    if !matches!(type_name, "VEC3" | "VEC4") {
        return Err(invalid("COLOR accessor must be VEC3 or VEC4"));
    }
    let component_type = integer_field(accessor, "componentType")?;
    if !matches!(component_type, 5121 | 5123 | 5126) {
        return Err(invalid("COLOR accessor component type is unsupported"));
    }
    let descriptor = decode_accessor(
        accessors,
        views,
        binary,
        accessor_index,
        type_name,
        &[5121, 5123, 5126],
    )?;
    for index in 0..descriptor.count {
        let offset = descriptor.start + index * descriptor.stride;
        for component in 0..component_count(type_name)? {
            let value = read_component_as_f64(
                binary,
                offset + component * component_size(component_type)?,
                component_type,
                descriptor.normalized,
            )?;
            if !value.is_finite() {
                return Err(invalid("COLOR accessor contains a non-finite value"));
            }
        }
    }
    Ok(())
}

fn validate_skin_attributes(
    accessors: &[Value],
    views: &[Value],
    binary: &[u8],
    joints_accessor: usize,
    weights_accessor: usize,
    position_count: usize,
    skin: Option<&ParsedSkin>,
) -> ImportResult<()> {
    let joints_descriptor = decode_accessor(
        accessors,
        views,
        binary,
        joints_accessor,
        "VEC4",
        &[5121, 5123],
    )?;
    if joints_descriptor.count != position_count {
        return Err(invalid("JOINTS_0 count differs from POSITION count"));
    }
    let weights = read_vec4_accessor(accessors, views, binary, weights_accessor)?;
    if weights.len() != position_count {
        return Err(invalid("WEIGHTS_0 count differs from POSITION count"));
    }
    let Some(skin) = skin else {
        return Err(invalid("skinned attributes require a source skin"));
    };
    for index in 0..position_count {
        let offset = joints_descriptor.start + index * joints_descriptor.stride;
        let mut joints = [0u32; 4];
        for (component, joint) in joints.iter_mut().enumerate() {
            *joint = match joints_descriptor.component_type {
                5121 => binary
                    .get(offset + component)
                    .copied()
                    .ok_or_else(|| invalid("JOINTS_0 U8 is out of bounds"))?
                    as u32,
                5123 => read_u16(binary, offset + component * 2)? as u32,
                _ => unreachable!(),
            };
            if (*joint as usize) >= skin.joints.len() {
                return Err(invalid("JOINTS_0 points outside skin joint palette"));
            }
        }
        let weight_sum: f64 = weights[index].iter().sum();
        if weights[index]
            .iter()
            .any(|weight| !weight.is_finite() || *weight < 0.0 || *weight > 1.0)
            || (weight_sum - 1.0).abs() > 1.0e-4
        {
            return Err(invalid(
                "WEIGHTS_0 is not a finite normalized influence set",
            ));
        }
    }
    Ok(())
}

fn node_local_matrix(node: &Map<String, Value>) -> ImportResult<Mat4> {
    if let Some(matrix) = node.get("matrix") {
        if node.contains_key("translation")
            || node.contains_key("rotation")
            || node.contains_key("scale")
        {
            return Err(invalid("node cannot combine matrix with TRS"));
        }
        let values = matrix
            .as_array()
            .ok_or_else(|| invalid("node matrix must be an array"))?;
        if values.len() != 16 {
            return Err(invalid("node matrix must contain 16 values"));
        }
        let mut result = [0.0; 16];
        for (index, value) in values.iter().enumerate() {
            result[index] = finite_number(value, "node matrix component")?;
        }
        ensure_matrix_bounded(result, "node local matrix")?;
        return Ok(result);
    }
    let translation = optional_vec3(node, "translation")?.unwrap_or([0.0; 3]);
    let rotation = optional_vec4(node, "rotation")?.unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let scale = optional_vec3(node, "scale")?.unwrap_or([1.0; 3]);
    if scale.iter().any(|value| value.abs() < MATRIX_EPSILON) {
        return Err(invalid("node scale contains zero"));
    }
    let rotation_matrix = quaternion_to_matrix(rotation)?;
    let mut result = rotation_matrix;
    // Scale the rotation columns in column-major storage.
    for column in 0..3 {
        for row in 0..3 {
            result[column * 4 + row] *= scale[column];
        }
    }
    result[12] = translation[0];
    result[13] = translation[1];
    result[14] = translation[2];
    ensure_matrix_bounded(result, "node TRS")?;
    Ok(result)
}

fn quaternion_to_matrix(quaternion: [f64; 4]) -> ImportResult<Mat4> {
    if quaternion.iter().any(|value| !value.is_finite()) {
        return Err(invalid("quaternion contains a non-finite value"));
    }
    let norm = quaternion
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm < MATRIX_EPSILON {
        return Err(invalid("quaternion has zero length"));
    }
    let [x, y, z, w] = quaternion.map(|value| value / norm);
    Ok([
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        0.0,
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        0.0,
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ])
}

fn transform_point(matrix: Mat4, point: [f64; 3], policy: CoordinatePolicy) -> [f64; 3] {
    let source = [point[0], point[1], point[2], 1.0];
    let world = mat4_vec4(matrix, source);
    let transformed = transform_vector([world[0], world[1], world[2]], policy);
    [transformed[0], transformed[1], transformed[2]]
}

fn transform_vector(vector: [f64; 3], policy: CoordinatePolicy) -> [f64; 3] {
    let sign = policy.forward_sign as f64;
    [sign * vector[2], vector[1], -sign * vector[0]]
}

fn validate_topology(faces: &[[u32; 3]]) -> ImportResult<(usize, usize)> {
    let mut edges = BTreeMap::<(u32, u32), Vec<(u32, u32)>>::new();
    for face in faces {
        if face[0] == face[1] || face[1] == face[2] || face[2] == face[0] {
            return Err(invalid("triangle has repeated vertex index"));
        }
        for (a, b) in [(face[0], face[1]), (face[1], face[2]), (face[2], face[0])] {
            edges.entry((a.min(b), a.max(b))).or_default().push((a, b));
        }
    }
    let mut boundary = 0;
    let mut non_manifold = 0;
    for incidences in edges.values() {
        match incidences.len() {
            1 => boundary += 1,
            2 => {
                let (left_a, left_b) = incidences[0];
                let (right_a, right_b) = incidences[1];
                if left_a == right_a && left_b == right_b {
                    non_manifold += 1;
                }
            }
            _ => non_manifold += 1,
        }
    }
    Ok((boundary, non_manifold))
}

fn triangle_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt()
}

fn mat4_identity() -> Mat4 {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn mat4_mul(left: Mat4, right: Mat4) -> Mat4 {
    let mut output = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            output[column * 4 + row] = (0..4)
                .map(|index| left[index * 4 + row] * right[column * 4 + index])
                .sum();
        }
    }
    output
}

fn mat4_vec4(matrix: Mat4, vector: [f64; 4]) -> [f64; 4] {
    [
        matrix[0] * vector[0]
            + matrix[4] * vector[1]
            + matrix[8] * vector[2]
            + matrix[12] * vector[3],
        matrix[1] * vector[0]
            + matrix[5] * vector[1]
            + matrix[9] * vector[2]
            + matrix[13] * vector[3],
        matrix[2] * vector[0]
            + matrix[6] * vector[1]
            + matrix[10] * vector[2]
            + matrix[14] * vector[3],
        matrix[3] * vector[0]
            + matrix[7] * vector[1]
            + matrix[11] * vector[2]
            + matrix[15] * vector[3],
    ]
}

fn ensure_matrix_bounded(matrix: Mat4, label: &str) -> ImportResult<()> {
    if matrix
        .iter()
        .any(|value| !value.is_finite() || value.abs() > TRANSFORM_ABS_LIMIT)
    {
        return Err(invalid(format!("{label} is non-finite or outside bounds")));
    }
    Ok(())
}

fn component_count(type_name: &str) -> ImportResult<usize> {
    match type_name {
        "SCALAR" => Ok(1),
        "VEC2" => Ok(2),
        "VEC3" => Ok(3),
        "VEC4" => Ok(4),
        "MAT4" => Ok(16),
        _ => Err(invalid(format!("accessor type {type_name} is unsupported"))),
    }
}

fn component_size(component_type: u64) -> ImportResult<usize> {
    match component_type {
        5121 => Ok(1),
        5123 => Ok(2),
        5125 | 5126 => Ok(4),
        _ => Err(invalid(format!(
            "component type {component_type} is unsupported"
        ))),
    }
}

fn read_component_as_f64(
    binary: &[u8],
    offset: usize,
    component_type: u64,
    normalized: bool,
) -> ImportResult<f64> {
    let raw = match component_type {
        5121 => binary
            .get(offset)
            .copied()
            .ok_or_else(|| invalid("U8 component is out of bounds"))? as f64,
        5123 => read_u16(binary, offset)? as f64,
        5126 => read_f32(binary, offset)?,
        _ => return Err(invalid("unsupported component type")),
    };
    Ok(if normalized {
        match component_type {
            5121 => raw / 255.0,
            5123 => raw / 65535.0,
            _ => raw,
        }
    } else {
        raw
    })
}

fn read_f32(bytes: &[u8], offset: usize) -> ImportResult<f64> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| invalid("f32 offset overflows"))?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| invalid("f32 value is out of bounds"))?;
    Ok(f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]) as f64)
}

fn read_u16(bytes: &[u8], offset: usize) -> ImportResult<u16> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| invalid("u16 offset overflows"))?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| invalid("u16 value is out of bounds"))?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> ImportResult<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| invalid("u32 offset overflows"))?;
    let slice = bytes
        .get(offset..end)
        .ok_or_else(|| invalid("u32 value is out of bounds"))?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn required_array<'a>(root: &'a Value, key: &str) -> ImportResult<&'a Vec<Value>> {
    root.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{key} array is missing")))
}

fn optional_array<'a>(root: &'a Value, key: &str) -> ImportResult<&'a Vec<Value>> {
    match root.get(key) {
        None => Ok(&EMPTY_VALUE_ARRAY),
        Some(value) => value
            .as_array()
            .ok_or_else(|| invalid(format!("{key} must be an array"))),
    }
}

static EMPTY_VALUE_ARRAY: Vec<Value> = Vec::new();

fn object<'a>(value: &'a Value, label: &str) -> ImportResult<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} must be an object")))
}

fn required_index(object: &Map<String, Value>, key: &str) -> ImportResult<usize> {
    index_from_value(
        object
            .get(key)
            .ok_or_else(|| invalid(format!("attribute {key} is missing")))?,
        key,
    )
}

fn optional_attribute_index(object: &Map<String, Value>, key: &str) -> ImportResult<Option<usize>> {
    object
        .get(key)
        .map(|value| index_from_value(value, key))
        .transpose()
}

fn optional_index(object: &Map<String, Value>, key: &str) -> ImportResult<Option<usize>> {
    object
        .get(key)
        .map(|value| index_from_value(value, key))
        .transpose()
}

fn index_from_value(value: &Value, label: &str) -> ImportResult<usize> {
    let number = value
        .as_u64()
        .ok_or_else(|| invalid(format!("{label} must be an unsigned integer")))?;
    usize::try_from(number).map_err(|_| invalid(format!("{label} overflows usize")))
}

fn usize_field(object: &Map<String, Value>, key: &str) -> ImportResult<usize> {
    index_from_value(
        object
            .get(key)
            .ok_or_else(|| invalid(format!("{key} is missing")))?,
        key,
    )
}

fn optional_usize_field(object: &Map<String, Value>, key: &str) -> ImportResult<Option<usize>> {
    object
        .get(key)
        .map(|value| index_from_value(value, key))
        .transpose()
}

fn integer_field(object: &Map<String, Value>, key: &str) -> ImportResult<u64> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("{key} must be an unsigned integer")))
}

fn integer_value(value: &Value, label: &str) -> ImportResult<u64> {
    value
        .as_u64()
        .ok_or_else(|| invalid(format!("{label} must be an unsigned integer")))
}

fn optional_finite_number(object: &Map<String, Value>, key: &str) -> ImportResult<Option<f64>> {
    object
        .get(key)
        .map(|value| finite_number(value, key))
        .transpose()
}

fn finite_number(value: &Value, label: &str) -> ImportResult<f64> {
    let number = value
        .as_f64()
        .ok_or_else(|| invalid(format!("{label} must be a number")))?;
    if !number.is_finite() {
        return Err(invalid(format!("{label} must be finite")));
    }
    Ok(number)
}

fn optional_vec3(object: &Map<String, Value>, key: &str) -> ImportResult<Option<[f64; 3]>> {
    object
        .get(key)
        .map(|value| finite_array::<3>(value, key))
        .transpose()
}

fn optional_vec4(object: &Map<String, Value>, key: &str) -> ImportResult<Option<[f64; 4]>> {
    object
        .get(key)
        .map(|value| finite_array::<4>(value, key))
        .transpose()
}

fn finite_array<const N: usize>(value: &Value, label: &str) -> ImportResult<[f64; N]> {
    let values = value
        .as_array()
        .ok_or_else(|| invalid(format!("{label} must be an array")))?;
    if values.len() != N {
        return Err(invalid(format!("{label} must contain {N} values")));
    }
    let mut result = [0.0; N];
    for (index, value) in values.iter().enumerate() {
        result[index] = finite_number(value, &format!("{label}[{index}]"))?;
    }
    Ok(result)
}

fn validate_attribute_keys(attributes: &Map<String, Value>) -> ImportResult<()> {
    const ALLOWED: &[&str] = &[
        "COLOR_0",
        "COLOR_1",
        "JOINTS_0",
        "NORMAL",
        "POSITION",
        "TANGENT",
        "TEXCOORD_0",
        "WEIGHTS_0",
    ];
    if attributes.is_empty()
        || attributes
            .keys()
            .any(|key| !ALLOWED.contains(&key.as_str()))
    {
        return Err(invalid(
            "primitive contains an unsupported vertex attribute",
        ));
    }
    Ok(())
}

fn reject_external_fields(value: &Value) -> ImportResult<()> {
    match value {
        Value::Array(values) => values.iter().try_for_each(reject_external_fields),
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(
                    key.as_str(),
                    "uri"
                        | "url"
                        | "script"
                        | "scripts"
                        | "command"
                        | "exec"
                        | "eval"
                        | "environment"
                        | "env"
                ) {
                    return Err(invalid(format!(
                        "external or executable field {key} is forbidden"
                    )));
                }
                reject_external_fields(child)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn reject_unknown_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
    label: &str,
) -> ImportResult<()> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        let unknown = object
            .keys()
            .find(|key| !allowed.contains(&key.as_str()))
            .cloned()
            .unwrap_or_else(|| "unknown".to_owned());
        return Err(invalid(format!(
            "{label} contains unsupported field {unknown}"
        )));
    }
    Ok(())
}

fn checked_range(offset: usize, length: usize, total: usize, label: &str) -> ImportResult<()> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid(format!("{label} range overflows")))?;
    if end > total {
        return Err(invalid(format!("{label} range is out of bounds")));
    }
    Ok(())
}

fn usize_from_u32(value: u32, label: &str) -> ImportResult<usize> {
    usize::try_from(value).map_err(|_| invalid(format!("{label} overflows usize")))
}

fn budget(label: &str) -> WeaponFoundationImportError {
    WeaponFoundationImportError::Budget(label.to_owned())
}

fn invalid(label: impl Into<String>) -> WeaponFoundationImportError {
    WeaponFoundationImportError::Invalid(label.into())
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    let mut previous_separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator {
            output.push('_');
            previous_separator = true;
        }
    }
    let trimmed = output.trim_matches('_');
    if trimmed.is_empty() {
        "node".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn canonical_hash_without_field<T: Serialize>(value: &T, field: &str) -> String {
    let mut json = serde_json::to_value(value).expect("ForgeCAD contracts are serializable");
    if let Some(object) = json.as_object_mut() {
        object.insert(field.to_owned(), Value::String(String::new()));
    }
    canonical_json_hash(&json)
}
