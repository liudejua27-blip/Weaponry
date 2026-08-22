import { readFile } from "node:fs/promises";
import {
  AnimationMixer,
  Matrix4,
  Quaternion,
  REVISION,
  Vector3,
} from "../apps/desktop/node_modules/three/build/three.module.js";
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
if (
  probe.schema_version !== "GameWeaponAnimatedGlbSocketThreeJsProbe@1" ||
  typeof probe.source_animated_glb_base64 !== "string" ||
  typeof probe.derived_animated_socket_glb_base64 !== "string" ||
  probe.restart_hash_verified !== true ||
  probe.receipt?.socket_node_count !== 6 ||
  probe.receipt?.animations_preserved !== true ||
  probe.receipt?.channels_preserved !== true ||
  probe.receipt?.samplers_preserved !== true ||
  probe.receipt?.renderable_projection_exact !== true ||
  probe.receipt?.bin_byte_exact !== true
) {
  throw new Error("Runtime animated GLB socket probe is incomplete");
}

const decode = (value) => {
  const bytes = Buffer.from(value, "base64");
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
};
const parse = (bytes) =>
  new Promise((resolve, reject) => new GLTFLoader().parse(bytes, "", resolve, reject));
const [sourceAsset, derivedAsset] = await Promise.all([
  parse(decode(probe.source_animated_glb_base64)),
  parse(decode(probe.derived_animated_socket_glb_base64)),
]);

if (sourceAsset.animations.length !== 1 || derivedAsset.animations.length !== 1) {
  throw new Error("source or derived rigid animation count differs");
}
const sourceClip = sourceAsset.animations[0];
const derivedClip = derivedAsset.animations[0];
const trackProjection = (clip) => ({
  name: clip.name,
  duration: clip.duration,
  tracks: clip.tracks.map((track) => ({
    name: track.name,
    times: Array.from(track.times),
    values: Array.from(track.values),
  })),
});
if (JSON.stringify(trackProjection(sourceClip)) !== JSON.stringify(trackProjection(derivedClip))) {
  throw new Error("Three.js animation clip or keyframe projection differs");
}

let sourceMeshCount = 0;
let derivedMeshCount = 0;
sourceAsset.scene.traverse((object) => {
  if (object.isMesh) sourceMeshCount += 1;
});
derivedAsset.scene.traverse((object) => {
  if (object.isMesh) derivedMeshCount += 1;
});
if (sourceMeshCount !== derivedMeshCount) {
  throw new Error("Three.js renderable mesh count differs");
}

const tolerance = 1e-6;
const near = (left, right) => Math.abs(left - right) <= tolerance;
const socketNames = probe.receipt.socket_nodes.map((node) => node.node_name).sort();
sourceAsset.scene.updateMatrixWorld(true);
derivedAsset.scene.updateMatrixWorld(true);
for (const name of socketNames) {
  if (sourceAsset.scene.getObjectByName(name)) {
    throw new Error(`source animated GLB unexpectedly contains socket ${name}`);
  }
}
for (const expected of probe.receipt.socket_nodes) {
  const node = derivedAsset.scene.getObjectByName(expected.node_name);
  if (!node || node.isMesh || node.children.length !== 0) {
    throw new Error(`derived socket is missing or renderable: ${expected.node_name}`);
  }
  const expectedParent = expected.parent_kind === "synthetic-scene-root"
    ? derivedAsset.scene
    : derivedAsset.scene.getObjectByName(expected.parent_node_name);
  if (!expectedParent || node.parent !== expectedParent) {
    throw new Error(`derived socket parent differs: ${expected.node_name}`);
  }
  if (
    !node.position.toArray().every((value, axis) => near(value, expected.local_translation_m[axis])) ||
    !node.quaternion.toArray().every((value, axis) => near(value, expected.local_rotation_quat_xyzw[axis])) ||
    !node.scale.toArray().every((value, axis) => near(value, expected.local_scale_xyz[axis]))
  ) {
    throw new Error(`derived socket local TRS differs: ${expected.node_name}`);
  }
  const composed = new Matrix4().compose(
    new Vector3(...expected.local_translation_m),
    new Quaternion(...expected.local_rotation_quat_xyzw),
    new Vector3(...expected.local_scale_xyz),
  );
  if (!node.matrix.equals(composed)) {
    throw new Error(`derived socket local matrix differs: ${expected.node_name}`);
  }
}

const mixer = new AnimationMixer(derivedAsset.scene);
mixer.clipAction(derivedClip).play();
mixer.setTime(0);
derivedAsset.scene.updateMatrixWorld(true);
const startWorld = new Map(
  socketNames.map((name) => [name, derivedAsset.scene.getObjectByName(name).matrixWorld.clone()]),
);
mixer.setTime(derivedClip.duration * 0.5);
derivedAsset.scene.updateMatrixWorld(true);
const halfWorld = new Map(
  socketNames.map((name) => [name, derivedAsset.scene.getObjectByName(name).matrixWorld.clone()]),
);
const animatedSocketNames = socketNames.filter(
  (name) => !startWorld.get(name).equals(halfWorld.get(name)),
);
if (animatedSocketNames.length < 2) {
  throw new Error("fewer than two socket nodes followed animated Part parents");
}
const matrixProjection = (matrix) => {
  const position = new Vector3();
  const quaternion = new Quaternion();
  const scale = new Vector3();
  matrix.decompose(position, quaternion, scale);
  return {
    position: position.toArray(),
    quaternion_xyzw: quaternion.toArray(),
    scale: scale.toArray(),
  };
};
const socketAnimationSamples = socketNames.map((name) => ({
  name,
  start: matrixProjection(startWorld.get(name)),
  half: matrixProjection(halfWorld.get(name)),
}));

process.stdout.write(`${JSON.stringify({
  schema_version: "ThreeJsGameWeaponAnimatedGlbSocketConsumerReceipt@1",
  three_revision: REVISION,
  loader: "three/examples/jsm/loaders/GLTFLoader.js",
  source_animated_glb_parse: "PASS",
  derived_animated_socket_glb_parse: "PASS",
  animation_clip_projection_exact: "PASS",
  animation_track_count: derivedClip.tracks.length,
  mesh_count_preserved: "PASS",
  mesh_count: derivedMeshCount,
  exact_six_named_empty_nodes: "PASS",
  owner_part_parenting: "PASS",
  scene_root_parenting: "PASS",
  local_trs_readback: "PASS",
  animated_parent_follow: "PASS",
  animated_socket_names: animatedSocketNames,
  socket_animation_samples: socketAnimationSamples,
  runtime_restart_hash: "PASS",
  actual_commercial_engine_roundtrip: false,
  unity: "NOT_RUN",
  unreal: "NOT_RUN",
  godot: "NOT_RUN",
  candidate_confirmed: false,
  export_performed: false,
  quality_status: "structural_only",
})}\n`);
