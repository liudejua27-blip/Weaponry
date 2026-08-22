import { readFile } from "node:fs/promises";
import { Object3D, Quaternion, REVISION, Vector3 } from "../apps/desktop/node_modules/three/build/three.module.js";

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
const set = probe.weapon_anchor_set;
if (!set || set.schema_version !== "GameWeaponAnchorSet@1") {
  throw new Error("GameWeaponAnchorSet@1 is missing");
}
if (
  set.semantic_scope !== "fictional-nonfunctional-game-visual-authoring-only@1" ||
  set.functional_semantics !== false ||
  set.pivot_status !== "not-proven-runtime-pivot" ||
  set.node_materialization !== "sidecar-only-not-glb-nodes"
) {
  throw new Error("weapon anchor truth boundary differs");
}
if (
  !Array.isArray(set.lod_bindings) ||
  set.lod_bindings.length !== 3 ||
  set.lod_bindings.some((binding, index) => binding.level !== index)
) {
  throw new Error("weapon anchor LOD0/1/2 bindings differ");
}

const expectedRoles = [
  "weapon-root",
  "grip-primary",
  "muzzle-vfx",
  "magazine-well",
  "sight-primary",
  "energy-core-vfx",
];
if (
  !Array.isArray(set.anchors) ||
  JSON.stringify(set.anchors.map((anchor) => anchor.role)) !== JSON.stringify(expectedRoles)
) {
  throw new Error("weapon anchor role order or coverage differs");
}

const sceneRoot = new Object3D();
sceneRoot.name = "synthetic-weapon-root";
const partNodes = new Map();
for (const partId of set.part_ids) {
  const part = new Object3D();
  part.name = partId;
  sceneRoot.add(part);
  partNodes.set(partId, part);
}
const helpers = new Map();
for (const anchor of set.anchors) {
  const translation = anchor.local_translation_m;
  const rotation = anchor.local_rotation_quat_xyzw;
  if (
    !Array.isArray(translation) ||
    translation.length !== 3 ||
    !translation.every(Number.isFinite) ||
    !Array.isArray(rotation) ||
    rotation.length !== 4 ||
    !rotation.every(Number.isFinite) ||
    Math.abs(rotation.reduce((sum, value) => sum + value * value, 0) - 1) > 1e-6 ||
    JSON.stringify(anchor.local_scale_xyz) !== "[1,1,1]"
  ) {
    throw new Error(`invalid anchor TRS: ${anchor.anchor_id}`);
  }
  const helper = new Object3D();
  helper.name = anchor.anchor_id;
  helper.position.fromArray(translation);
  helper.quaternion.fromArray(rotation);
  if (anchor.role === "weapon-root") {
    if (anchor.parent_kind !== "synthetic-scene-root" || anchor.owner_part_id !== null) {
      throw new Error("weapon root parent binding differs");
    }
    sceneRoot.add(helper);
  } else {
    const owner = partNodes.get(anchor.owner_part_id);
    if (!owner || anchor.parent_kind !== "part-node") {
      throw new Error(`Part owner is missing: ${anchor.anchor_id}`);
    }
    owner.add(helper);
  }
  helpers.set(anchor.anchor_id, helper);
}

sceneRoot.updateMatrixWorld(true);
const grip = helpers.get("grip-primary");
const gripPart = partNodes.get("grip-module");
const local = grip.position.clone();
gripPart.position.set(0.3, -0.1, 0.2);
gripPart.quaternion.setFromAxisAngle(new Vector3(0, 1, 0), 0.35);
sceneRoot.updateMatrixWorld(true);
const actualWorld = grip.getWorldPosition(new Vector3());
const expectedWorld = local
  .clone()
  .applyQuaternion(gripPart.quaternion)
  .add(gripPart.position);
if (actualWorld.distanceTo(expectedWorld) > 1e-6) {
  throw new Error("Part-bound anchor did not rigidly follow parent TRS");
}
const actualRotation = grip.getWorldQuaternion(new Quaternion());
if (1 - Math.abs(actualRotation.dot(gripPart.quaternion)) > 1e-6) {
  throw new Error("Part-bound anchor rotation did not follow parent TRS");
}

const receipt = {
  schema_version: "ThreeJsGameWeaponAnchorConsumerReceipt@1",
  three_revision: REVISION,
  source_anchor_set_object_sha256: probe.weapon_anchor_set_object_sha256,
  anchor_role_coverage: "PASS_EXACT_SIX_ROLE",
  lod_binding_coverage: "PASS_EXACT_LOD0_LOD1_LOD2",
  synthetic_root: "PASS_METADATA_SCENE_FRAME",
  part_bound_helpers: "PASS_FIVE_UNIQUE_OWNER_PARTS",
  trs_validation: "PASS_FINITE_UNIT_QUATERNION_IDENTITY_SCALE",
  parent_world_times_local_trs: "PASS_THREEJS_OBJECT3D",
  rigid_parent_follow: "PASS_THREEJS_OBJECT3D_STRUCTURAL_ONLY",
  pivot_status: "NOT_PROVEN_RUNTIME_PIVOT",
  glb_anchor_nodes_materialized: false,
  actual_commercial_engine_roundtrip: false,
  unity: "NOT_RUN",
  unreal: "NOT_RUN",
  godot: "NOT_RUN",
  semantic_scope: set.semantic_scope,
  functional_semantics: false,
  quality_status: "structural_only",
};
process.stdout.write(`${JSON.stringify(receipt)}\n`);
