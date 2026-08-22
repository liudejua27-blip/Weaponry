import { readFile } from "node:fs/promises";
import { Matrix4, Quaternion, REVISION, Vector3 } from "../apps/desktop/node_modules/three/build/three.module.js";
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
  probe.schema_version !== "GameWeaponGlbSocketThreeJsProbe@1" ||
  !Array.isArray(probe.lod_glb_base64s) ||
  probe.lod_glb_base64s.length !== 3 ||
  !Array.isArray(probe.levels) ||
  probe.levels.length !== 3 ||
  probe.restart_hash_verified !== true
) {
  throw new Error("Runtime GLB socket probe is incomplete");
}

const decode = (value) => {
  const bytes = Buffer.from(value, "base64");
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
};
const parse = (bytes) =>
  new Promise((resolve, reject) => new GLTFLoader().parse(bytes, "", resolve, reject));
const assets = await Promise.all(probe.lod_glb_base64s.map((value) => parse(decode(value))));

const tolerance = 1e-6;
const near = (left, right) => Math.abs(left - right) <= tolerance;
const expectedNames = probe.levels[0].socket_nodes.map((node) => node.node_name).sort();
const perLod = assets.map((asset, lodLevel) => {
  const level = probe.levels[lodLevel];
  if (
    level.lod_level !== lodLevel ||
    level.socket_nodes_materialized !== true ||
    level.source_renderable_projection_exact !== true ||
    level.source_bin_byte_exact !== true ||
    level.socket_node_count !== 6
  ) {
    throw new Error(`LOD${lodLevel} Runtime readback differs`);
  }
  asset.scene.updateMatrixWorld(true);
  const names = [];
  let meshCount = 0;
  asset.scene.traverse((object) => {
    if (object.isMesh) meshCount += 1;
    if (expectedNames.includes(object.name)) names.push(object.name);
  });
  names.sort();
  if (JSON.stringify(names) !== JSON.stringify(expectedNames)) {
    throw new Error(`LOD${lodLevel} socket node coverage differs`);
  }
  for (const expected of level.socket_nodes) {
    const node = asset.scene.getObjectByName(expected.node_name);
    if (!node || node.isMesh || node.children.length !== 0) {
      throw new Error(`LOD${lodLevel} socket is missing or renderable: ${expected.node_name}`);
    }
    const expectedParent = expected.parent_kind === "synthetic-scene-root"
      ? asset.scene
      : asset.scene.getObjectByName(expected.parent_node_name);
    if (!expectedParent || node.parent !== expectedParent) {
      throw new Error(`LOD${lodLevel} socket parent differs: ${expected.node_name}`);
    }
    const translation = node.position.toArray();
    const rotation = node.quaternion.toArray();
    const scale = node.scale.toArray();
    if (
      !translation.every((value, axis) => near(value, expected.local_translation_m[axis])) ||
      !rotation.every((value, axis) => near(value, expected.local_rotation_quat_xyzw[axis])) ||
      !scale.every((value, axis) => near(value, expected.local_scale_xyz[axis]))
    ) {
      throw new Error(`LOD${lodLevel} socket local TRS differs: ${expected.node_name}`);
    }
    const composed = new Matrix4().compose(
      new Vector3(...expected.local_translation_m),
      new Quaternion(...expected.local_rotation_quat_xyzw),
      new Vector3(...expected.local_scale_xyz),
    );
    if (!node.matrix.equals(composed)) {
      throw new Error(`LOD${lodLevel} socket matrix differs: ${expected.node_name}`);
    }
  }
  return { lod_level: lodLevel, socket_names: names, mesh_count: meshCount };
});

process.stdout.write(`${JSON.stringify({
  schema_version: "ThreeJsGameWeaponGlbSocketConsumerReceipt@1",
  three_revision: REVISION,
  loader: "three/examples/jsm/loaders/GLTFLoader.js",
  lod_glb_parse: ["PASS", "PASS", "PASS"],
  exact_six_named_empty_nodes: "PASS",
  owner_part_parenting: "PASS",
  scene_root_parenting: "PASS",
  local_trs_readback: "PASS",
  non_rendering_socket_nodes: "PASS",
  runtime_restart_hash: "PASS",
  per_lod: perLod,
  actual_commercial_engine_roundtrip: false,
  unity: "NOT_RUN",
  unreal: "NOT_RUN",
  godot: "NOT_RUN",
  candidate_confirmed: false,
  export_performed: false,
  quality_status: "structural_only",
})}\n`);
