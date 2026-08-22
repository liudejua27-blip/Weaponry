import { readFile } from "node:fs/promises";
import { AnimationMixer, Bone, Box3, REVISION, SkinnedMesh, Vector3 } from "../apps/desktop/node_modules/three/build/three.module.js";
import { GLTFLoader } from "../apps/desktop/node_modules/three/examples/jsm/loaders/GLTFLoader.js";

if (typeof globalThis.ProgressEvent === "undefined") {
  globalThis.ProgressEvent = class ProgressEvent {
    constructor(type, init = {}) {
      this.type = type;
      Object.assign(this, init);
    }
  };
}

const source = process.argv[2]
  ? await readFile(process.argv[2], "utf8")
  : await new Promise((resolve, reject) => {
      let value = "";
      process.stdin.setEncoding("utf8");
      process.stdin.on("data", (chunk) => (value += chunk));
      process.stdin.on("end", () => resolve(value));
      process.stdin.on("error", reject);
    });
const probe = JSON.parse(source.trim());
const decode = (value) => {
  const bytes = Buffer.from(value, "base64");
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
};
const parse = (bytes) =>
  new Promise((resolve, reject) => new GLTFLoader().parse(bytes, "", resolve, reject));

if (!Array.isArray(probe.lod_glb_base64s) || probe.lod_glb_base64s.length !== 3) {
  throw new Error("probe must provide exactly three LOD GLBs");
}
const [lodAssets, animatedAsset] = await Promise.all([
  Promise.all(probe.lod_glb_base64s.map((value) => parse(decode(value)))),
  parse(decode(probe.animated_glb_base64)),
]);
const expectedParts = [...probe.part_ids].sort();
const inspect = (asset) => {
  const parts = [];
  const materialNames = [];
  let triangles = 0;
  let bones = 0;
  let skinnedMeshes = 0;
  asset.scene.traverse((object) => {
    if (expectedParts.includes(object.name)) parts.push(object.name);
    if (object instanceof Bone) bones += 1;
    if (object instanceof SkinnedMesh) skinnedMeshes += 1;
    if (object.isMesh) {
      const geometry = object.geometry;
      triangles += geometry.index
        ? geometry.index.count / 3
        : geometry.getAttribute("position").count / 3;
      const materials = Array.isArray(object.material) ? object.material : [object.material];
      for (const material of materials) materialNames.push(material.name);
    }
  });
  parts.sort();
  materialNames.sort();
  if (JSON.stringify(parts) !== JSON.stringify(expectedParts)) {
    throw new Error(`stable Part node coverage differs: ${JSON.stringify(parts)}`);
  }
  if (bones !== 0 || skinnedMeshes !== 0) {
    throw new Error("rigid asset unexpectedly contains bones or skinned meshes");
  }
  return { parts, material_names: materialNames, triangles, bones, skinned_meshes: skinnedMeshes };
};
const lodInspections = lodAssets.map(inspect);
const animatedInspection = inspect(animatedAsset);
for (const [index, asset] of lodAssets.entries()) {
  if (asset.animations.length !== 0) {
    throw new Error(`static LOD${index} unexpectedly contains animation clips`);
  }
  if (lodInspections[index].triangles !== probe.lod_triangle_counts[index]) {
    throw new Error(`LOD${index} triangle count differs after Three.js import`);
  }
  if (
    JSON.stringify(lodInspections[index].material_names) !==
    JSON.stringify(lodInspections[0].material_names)
  ) {
    throw new Error(`LOD${index} material names differ from LOD0`);
  }
}
if (animatedAsset.animations.length !== 1) {
  throw new Error("animated GLB must expose exactly one Three.js AnimationClip");
}
const mixer = new AnimationMixer(animatedAsset.scene);
const action = mixer.clipAction(animatedAsset.animations[0]);
action.play();
mixer.update(0.5);
if (animatedAsset.animations[0].tracks.length !== probe.animation_channel_count) {
  throw new Error("Three.js animation track count differs from the Runtime receipt");
}
if (!probe.restart_hash_passed || !probe.durable_get_passed) {
  throw new Error("Runtime restart hash verification failed before Three.js import");
}

const tolerance = 1e-5;
const near = (left, right) => Math.abs(left - right) <= tolerance;
const lod2 = lodAssets[2];
lod2.scene.updateMatrixWorld(true);
for (const proxy of probe.collision_proxies) {
  const part = lod2.scene.getObjectByName(proxy.part_id);
  if (!part) throw new Error(`LOD2 collision Part is missing: ${proxy.part_id}`);
  const box = new Box3().setFromObject(part);
  const center = box.getCenter(new Vector3()).toArray();
  const halfExtents = box.getSize(new Vector3()).multiplyScalar(0.5).toArray();
  if (
    !center.every((value, axis) => near(value, proxy.center_m[axis])) ||
    !halfExtents.every((value, axis) => near(value, proxy.half_extents_m[axis]))
  ) {
    throw new Error(`LOD2 collision proxy differs after Three.js import: ${proxy.part_id}`);
  }
}

const receipt = {
  schema_version: "ThreeJsGameAssetConsumerReceipt@1",
  three_revision: REVISION,
  loader: "three/examples/jsm/loaders/GLTFLoader.js",
  static_glb_parse: "PASS",
  lod_glb_parse: ["PASS", "PASS", "PASS"],
  lod_triangle_readback: "PASS",
  lod_material_name_stability: "PASS",
  collision_proxy_aabb_match: "PASS",
  animated_glb_parse: "PASS",
  animation_mixer_sample: "PASS",
  animation_track_count: animatedAsset.animations[0].tracks.length,
  stable_part_nodes: lodInspections[0].parts,
  no_bones: animatedInspection.bones === 0,
  no_skinned_meshes: animatedInspection.skinned_meshes === 0,
  lod_triangle_counts: probe.lod_triangle_counts,
  collision_proxy_count: probe.collision_proxy_count,
  runtime_restart_hash: "PASS",
  durable_delivery_get_after_restart: "PASS",
  candidate_confirmed: false,
  export_performed: false,
  threejs_consumer_roundtrip: true,
  actual_commercial_engine_roundtrip: false,
  unity: "NOT_RUN",
  unreal: "NOT_RUN",
  godot: "NOT_RUN",
  quality_status: "structural_only",
};
process.stdout.write(`${JSON.stringify(receipt)}\n`);
