import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

if (typeof globalThis.ProgressEvent === "undefined") {
  globalThis.ProgressEvent = class ProgressEvent {
    constructor(type, init = {}) {
      this.type = type;
      Object.assign(this, init);
    }
  };
}

// Three's GLTFLoader uses the browser ImageLoader even when a Node probe only
// needs material/texture binding readback. Keep the probe offline and
// deterministic with a minimal image-event surface; physical embedded PNG
// bytes are independently parsed and SHA-256 checked below.
if (typeof globalThis.document === "undefined") {
  globalThis.document = {
    createElementNS(_namespace, name) {
      if (name !== "img") throw new Error(`unexpected DOM element request: ${name}`);
      const listeners = new Map();
      let source = "";
      return {
        width: 1,
        height: 1,
        complete: true,
        crossOrigin: "anonymous",
        addEventListener(type, callback) { listeners.set(type, callback); },
        removeEventListener(type) { listeners.delete(type); },
        set src(value) {
          source = value;
          queueMicrotask(() => listeners.get("load")?.({ type: "load" }));
        },
        get src() { return source; },
      };
    },
  };
}
if (typeof globalThis.self === "undefined") globalThis.self = globalThis;

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const threePackageRoots = [
  { label: "root-node_modules/three", path: resolve(repositoryRoot, "node_modules/three") },
  { label: "desktop-node_modules/three", path: resolve(repositoryRoot, "apps/desktop/node_modules/three") },
];

const loadThree = async () => {
  const failures = [];
  for (const candidate of threePackageRoots) {
    try {
      const three = await import(
        pathToFileURL(resolve(candidate.path, "build/three.module.js")).href
      );
      const loader = await import(
        pathToFileURL(resolve(candidate.path, "examples/jsm/loaders/GLTFLoader.js")).href
      );
      if (three.REVISION !== "185") {
        throw new Error(`loaded revision r${three.REVISION}, expected r185`);
      }
      return {
        ...three,
        GLTFLoader: loader.GLTFLoader,
        packageLabel: candidate.label,
      };
    } catch (error) {
      failures.push(`${candidate.label}: ${error instanceof Error ? error.message : String(error)}`);
    }
  }
  throw new Error(`Three.js r185 loader unavailable; attempted roots: ${failures.join(" | ")}`);
};

const {
  AnimationMixer,
  Bone,
  Box3,
  GLTFLoader,
  REVISION,
  SkinnedMesh,
  Vector3,
  packageLabel: threePackageLabel,
} = await loadThree();
if (REVISION !== "185") {
  throw new Error(`expected Three.js r185, loaded r${REVISION} from ${threePackageLabel}`);
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

const sha256Hex = (value) =>
  createHash("sha256").update(Buffer.from(value)).digest("hex");
const isSha256 = (value) => typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
const sorted = (values) => [...values].sort();
const sameJson = (left, right) => JSON.stringify(left) === JSON.stringify(right);

const requireStringArray = (value, field) => {
  if (
    !Array.isArray(value) ||
    value.length === 0 ||
    value.some((item) => typeof item !== "string" || item.length === 0)
  ) {
    throw new Error(`${field} must be a non-empty string array`);
  }
  return value;
};

const requireFiniteVector = (value, field) => {
  if (!Array.isArray(value) || value.length !== 3 || value.some((item) => !Number.isFinite(item))) {
    throw new Error(`${field} must be a finite 3-vector`);
  }
  return value;
};

const countExternalUris = (value) =>
  ["buffers", "images"].reduce(
    (count, key) =>
      count +
      (Array.isArray(value?.[key])
        ? value[key].filter((item) => typeof item?.uri === "string" && item.uri.length > 0).length
        : 0),
    0,
  );

const parseGlbJson = (bytes) => {
  const buffer = Buffer.from(bytes);
  if (buffer.length < 20 || buffer.toString("ascii", 0, 4) !== "glTF") {
    throw new Error("static GLB header is invalid");
  }
  if (buffer.readUInt32LE(4) !== 2 || buffer.readUInt32LE(8) !== buffer.length) {
    throw new Error("static GLB version or declared length is invalid");
  }
  let offset = 12;
  let json = null;
  let binary = null;
  while (offset + 8 <= buffer.length) {
    const chunkLength = buffer.readUInt32LE(offset);
    const chunkType = buffer.readUInt32LE(offset + 4);
    offset += 8;
    const end = offset + chunkLength;
    if (end > buffer.length) throw new Error("static GLB chunk exceeds declared length");
    if (chunkType === 0x4e4f534a) {
      if (json !== null) throw new Error("static GLB contains multiple JSON chunks");
      const text = buffer.toString("utf8", offset, end).replace(/\u0000/g, "").trim();
      try {
        json = JSON.parse(text);
      } catch (error) {
        throw new Error(`static GLB JSON is invalid: ${error instanceof Error ? error.message : String(error)}`);
      }
    } else if (chunkType === 0x004e4942) {
      if (binary !== null) throw new Error("static GLB contains multiple BIN chunks");
      binary = buffer.subarray(offset, end);
    }
    offset = end;
  }
  if (offset !== buffer.length || json === null) throw new Error("static GLB JSON chunk is missing");
  if (binary === null) throw new Error("static GLB BIN chunk is missing");
  return { json, binary, externalUriCount: countExternalUris(json) };
};

const pngPayload = (bytes) => {
  if (bytes.length < 8 || !bytes.subarray(0, 8).equals(Buffer.from("89504e470d0a1a0a", "hex"))) {
    throw new Error("embedded texture is not a PNG payload");
  }
  let offset = 8;
  while (offset + 12 <= bytes.length) {
    const length = bytes.readUInt32BE(offset);
    const end = offset + 12 + length;
    if (end > bytes.length) throw new Error("embedded PNG chunk is truncated");
    if (bytes.toString("ascii", offset + 4, offset + 8) === "IEND") return bytes.subarray(0, end);
    offset = end;
  }
  throw new Error("embedded PNG IEND is missing");
};

const embeddedPngInventory = (json, binary) => {
  if (!Array.isArray(json.images) || !Array.isArray(json.bufferViews)) {
    throw new Error("embedded image inventory is missing");
  }
  const inventory = new Map();
  for (const image of json.images) {
    if (typeof image?.name !== "string" || image.mimeType !== "image/png" || !Number.isInteger(image.bufferView)) {
      throw new Error("embedded image descriptor is invalid");
    }
    const view = json.bufferViews[image.bufferView];
    const start = view?.byteOffset ?? 0;
    const end = start + (view?.byteLength ?? -1);
    if (!Number.isInteger(start) || !Number.isInteger(end) || start < 0 || end > binary.length) {
      throw new Error(`embedded image range is invalid: ${image.name}`);
    }
    if (inventory.has(image.name)) throw new Error(`embedded image name is duplicated: ${image.name}`);
    const png = pngPayload(binary.subarray(start, end));
    inventory.set(image.name, { sha256: sha256Hex(png), size_bytes: png.length });
  }
  return inventory;
};

const inspectStaticAsset = (asset, expectedParts, expectedMaterialNames, expectedTriangleCount, expectedAabb, tolerance) => {
  const parts = [];
  const materialNames = [];
  let triangles = 0;
  let meshCount = 0;
  let bones = 0;
  let skinnedMeshes = 0;
  asset.scene.traverse((object) => {
    if (expectedParts.includes(object.name)) parts.push(object.name);
    if (object instanceof Bone) bones += 1;
    if (object instanceof SkinnedMesh) skinnedMeshes += 1;
    if (!object.isMesh) return;
    meshCount += 1;
    const geometry = object.geometry;
    const count = geometry.index
      ? geometry.index.count
      : geometry.getAttribute("position")?.count ?? 0;
    if (!Number.isInteger(count) || count <= 0 || count % 3 !== 0) {
      throw new Error(`static mesh ${object.name || "<unnamed>"} has invalid triangle index count`);
    }
    triangles += count / 3;
    const materials = Array.isArray(object.material) ? object.material : [object.material];
    for (const material of materials) {
      if (!material || typeof material.name !== "string" || material.name.length === 0) {
        throw new Error(`static mesh ${object.name || "<unnamed>"} has an unnamed material`);
      }
      materialNames.push(material.name);
    }
  });
  parts.sort();
  const uniqueMaterialNames = sorted(new Set(materialNames));
  if (!sameJson(parts, sorted(expectedParts))) {
    throw new Error(`stable Part node coverage differs: ${JSON.stringify(parts)}`);
  }
  if (!sameJson(uniqueMaterialNames, sorted(expectedMaterialNames))) {
    throw new Error(`static material names differ: ${JSON.stringify(uniqueMaterialNames)}`);
  }
  if (triangles !== expectedTriangleCount) {
    throw new Error(`static triangle count differs: ${triangles} !== ${expectedTriangleCount}`);
  }
  if (meshCount === 0) throw new Error("static GLB contains no renderable meshes");
  if (bones !== 0 || skinnedMeshes !== 0) {
    throw new Error("static asset unexpectedly contains bones or skinned meshes");
  }
  if (asset.animations.length !== 0) throw new Error("static-only GLB unexpectedly contains animation clips");

  asset.scene.updateMatrixWorld(true);
  const box = new Box3().setFromObject(asset.scene);
  const actualMin = box.min.toArray();
  const actualMax = box.max.toArray();
  const near = (left, right) => Math.abs(left - right) <= tolerance;
  if (
    !actualMin.every((value, axis) => near(value, expectedAabb.min_m[axis])) ||
    !actualMax.every((value, axis) => near(value, expectedAabb.max_m[axis]))
  ) {
    throw new Error(`static AABB differs: ${JSON.stringify({ min_m: actualMin, max_m: actualMax })}`);
  }
  return {
    parts,
    material_names: uniqueMaterialNames,
    triangles,
    aabb: { min_m: actualMin, max_m: actualMax },
    mesh_count: meshCount,
    bones,
    skinned_meshes: skinnedMeshes,
  };
};

const runStaticOnly = async (probe) => {
  if (probe.schema_version !== "ForgeCadThreeJsStaticGameAssetProbe@1") {
    throw new Error("static-only probe schema_version differs");
  }
  const candidateId = probe.candidate_id;
  const candidateStateSha256 = probe.candidate_state_sha256;
  if (typeof candidateId !== "string" || candidateId.length === 0 || !isSha256(candidateStateSha256)) {
    throw new Error("static-only candidate binding is invalid");
  }
  const expectedParts = requireStringArray(probe.part_ids, "part_ids");
  if (new Set(expectedParts).size !== expectedParts.length) throw new Error("part_ids must be unique");
  const expectedMaterialNames = requireStringArray(probe.material_names, "material_names");
  if (!Number.isInteger(probe.triangle_count) || probe.triangle_count <= 0) {
    throw new Error("triangle_count must be a positive integer");
  }
  const expectedAabb = probe.aabb;
  requireFiniteVector(expectedAabb?.min_m, "aabb.min_m");
  requireFiniteVector(expectedAabb?.max_m, "aabb.max_m");
  for (let axis = 0; axis < 3; axis += 1) {
    if (expectedAabb.min_m[axis] > expectedAabb.max_m[axis]) throw new Error("aabb min exceeds max");
  }
  const tolerance = probe.aabb_tolerance ?? 1e-5;
  if (!Number.isFinite(tolerance) || tolerance < 0 || tolerance > 1e-3) {
    throw new Error("aabb_tolerance is outside the bounded static probe range");
  }
  if (typeof probe.artifact_base64 !== "string") throw new Error("artifact_base64 is required");
  if (!isSha256(probe.artifact_sha256) || !isSha256(probe.artifact_readback_sha256)) {
    throw new Error("artifact and readback SHA-256 bindings are invalid");
  }
  if (probe.restart_hash_passed !== true || probe.durable_get_passed !== true) {
    throw new Error("Runtime restart/get evidence must already be true");
  }
  if (probe.candidate_confirmed !== false || probe.export_performed !== false) {
    throw new Error("static-only source probe cannot consume confirmed or exported state");
  }
  if (probe.actual_engine_roundtrip === true || probe.actual_commercial_engine_roundtrip === true) {
    throw new Error("static-only source probe cannot claim an engine round-trip");
  }
  const bytes = decode(probe.artifact_base64);
  const artifactSha256 = sha256Hex(bytes);
  if (artifactSha256 !== probe.artifact_sha256) {
    throw new Error(`static artifact hash differs: ${artifactSha256} !== ${probe.artifact_sha256}`);
  }
  const readback = probe.artifact_readback;
  if (!readback || typeof readback !== "object") throw new Error("artifact_readback is required");
  if (readback.artifact_id !== probe.artifact_sha256) throw new Error("artifact readback object hash differs");
  if (readback.object_sha256 !== undefined && readback.object_sha256 !== probe.artifact_sha256) {
    throw new Error("artifact readback object SHA-256 differs");
  }
  if (readback.canonical_sha256 !== probe.artifact_readback_sha256) {
    throw new Error("artifact readback canonical hash differs");
  }
  if (readback.candidate_id !== candidateId) throw new Error("artifact readback candidate binding differs");
  if (
    readback.candidate_state_sha256 !== undefined &&
    readback.candidate_state_sha256 !== candidateStateSha256
  ) {
    throw new Error("artifact readback candidate state binding differs");
  }
  if (readback.hard_gate_passed !== true) throw new Error("artifact readback hard gate is not passed");
  if (readback.integrity?.glb_parse_status !== undefined && readback.integrity.glb_parse_status !== "passed") {
    throw new Error("artifact readback GLB parse status is not passed");
  }
  if (readback.integrity?.external_uri_count !== undefined && readback.integrity.external_uri_count !== 0) {
    throw new Error("artifact readback contains external URI evidence");
  }
  if (Array.isArray(readback.part_ids) && !sameJson(sorted(readback.part_ids), sorted(expectedParts))) {
    throw new Error("artifact readback Part IDs differ");
  }

  const { json, externalUriCount } = parseGlbJson(bytes);
  if (externalUriCount !== 0) throw new Error(`static GLB contains ${externalUriCount} external URI field(s)`);
  const asset = await parse(bytes);
  const inspection = inspectStaticAsset(
    asset,
    expectedParts,
    expectedMaterialNames,
    probe.triangle_count,
    expectedAabb,
    tolerance,
  );
  return {
    schema_version: "ThreeJsStaticGameAssetSourceReceipt@1",
    mode: "static-only",
    three_revision: REVISION,
    loader: "three/examples/jsm/loaders/GLTFLoader.js",
    three_package_source: threePackageLabel,
    static_glb_parse: "PASS",
    stable_part_nodes: "PASS",
    static_material_readback: "PASS",
    triangle_readback: "PASS",
    aabb_readback: "PASS",
    no_external_uris: "PASS",
    external_uri_count: externalUriCount,
    animation_status: "ABSENT_EXPECTED",
    artifact_sha256: probe.artifact_sha256,
    artifact_readback_sha256: probe.artifact_readback_sha256,
    candidate_id: candidateId,
    candidate_state_sha256: candidateStateSha256,
    part_ids: inspection.parts,
    material_names: inspection.material_names,
    triangle_count: inspection.triangles,
    aabb: inspection.aabb,
    mesh_count: inspection.mesh_count,
    runtime_restart_hash: "PASS",
    durable_get_after_restart: "PASS",
    candidate_confirmed: false,
    export_performed: false,
    threejs_source_readback: true,
    actual_engine_roundtrip: false,
    actual_commercial_engine_roundtrip: false,
    commercial_engine_roundtrip: "NOT_RUN",
    godot: "NOT_RUN",
    unity: "NOT_RUN",
    unreal: "NOT_RUN",
    quality_status: "structural_only",
    gltf_json_keys: Object.keys(json).sort(),
  };
};

const runHeroMaterialEngine = async (probe) => {
  if (probe.schema_version !== "ForgeCadThreeJsHeroMaterialEngineProbe@1") {
    throw new Error("Hero material engine probe schema_version differs");
  }
  if (typeof probe.artifact_base64 !== "string" || !isSha256(probe.artifact_sha256)) {
    throw new Error("Hero material artifact binding is invalid");
  }
  if (!isSha256(probe.hero_material_result_canonical_sha256) || !isSha256(probe.geometric_bake_canonical_sha256)) {
    throw new Error("Hero material result/Bake binding is invalid");
  }
  if (probe.candidate_confirmed !== false || probe.export_performed !== false) {
    throw new Error("Hero material engine probe cannot consume confirmed or exported state");
  }
  const expectedParts = requireStringArray(probe.part_ids, "part_ids");
  const expectedMaterials = requireStringArray(probe.material_names, "material_names");
  if (new Set(expectedParts).size !== expectedParts.length || new Set(expectedMaterials).size !== expectedMaterials.length) {
    throw new Error("Hero material Part/material names must be unique");
  }
  if (!Number.isInteger(probe.triangle_count) || probe.triangle_count <= 0) {
    throw new Error("Hero material triangle_count is invalid");
  }
  requireFiniteVector(probe.aabb?.min_m, "aabb.min_m");
  requireFiniteVector(probe.aabb?.max_m, "aabb.max_m");
  const bytes = decode(probe.artifact_base64);
  if (sha256Hex(bytes) !== probe.artifact_sha256) throw new Error("Hero material artifact hash differs");
  const { json, binary, externalUriCount } = parseGlbJson(bytes);
  if (externalUriCount !== 0) throw new Error("Hero material GLB contains external URI fields");
  if (!Array.isArray(probe.texture_outputs) || probe.texture_outputs.length !== 6) {
    throw new Error("Hero material probe requires exactly six texture outputs");
  }
  const pngs = embeddedPngInventory(json, binary);
  if (pngs.size !== 6 || json.textures?.length !== 6) throw new Error("Hero material embedded texture count differs");
  for (const output of probe.texture_outputs) {
    const embedded = pngs.get(output.texture_id);
    if (!embedded || embedded.sha256 !== output.sha256 || embedded.size_bytes !== output.size_bytes) {
      throw new Error(`Hero material embedded PNG differs: ${output.texture_id}`);
    }
  }
  const asset = await parse(bytes);
  const inspection = inspectStaticAsset(
    asset,
    expectedParts,
    expectedMaterials,
    probe.triangle_count,
    probe.aabb,
    probe.aabb_tolerance ?? 1e-5,
  );
  const materialBindings = new Map();
  asset.scene.traverse((object) => {
    if (!object.isMesh) return;
    for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
      if (!expectedMaterials.includes(material.name)) continue;
      materialBindings.set(material.name, {
        base_color: Boolean(material.map),
        normal: Boolean(material.normalMap),
        metallic: Boolean(material.metalnessMap),
        roughness: Boolean(material.roughnessMap),
        ao: Boolean(material.aoMap),
        emissive: Boolean(material.emissiveMap),
      });
    }
  });
  if (materialBindings.size !== expectedMaterials.length) throw new Error("Hero material engine zone coverage differs");
  for (const [name, binding] of materialBindings) {
    if (Object.values(binding).some((value) => value !== true)) {
      throw new Error(`Hero material engine texture binding is incomplete: ${name}`);
    }
  }
  const heroBuild = json.extras?.forgecad?.hero_material_build;
  if (
    heroBuild?.geometric_bake_canonical_sha256 !== probe.geometric_bake_canonical_sha256 ||
    heroBuild?.low_artifact_sha256 !== probe.low_artifact_sha256 ||
    heroBuild?.resolution !== 2048 || heroBuild?.normal_convention !== "OpenGL+Y"
  ) {
    throw new Error("Hero material embedded build receipt differs");
  }
  return {
    schema_version: "ThreeJsHeroMaterialEngineReceipt@1",
    mode: "hero-material-engine",
    engine: `Three.js r${REVISION}`,
    engine_class: "web-3d-runtime-consumer",
    loader: "three/examples/jsm/loaders/GLTFLoader.js",
    three_package_source: threePackageLabel,
    artifact_sha256: probe.artifact_sha256,
    hero_material_result_canonical_sha256: probe.hero_material_result_canonical_sha256,
    geometric_bake_canonical_sha256: probe.geometric_bake_canonical_sha256,
    low_artifact_sha256: probe.low_artifact_sha256,
    glb_parse: "PASS",
    stable_part_nodes: "PASS",
    pbr_material_bindings: "PASS",
    embedded_texture_hashes: "PASS",
    triangle_readback: "PASS",
    aabb_readback: "PASS",
    no_external_uris: "PASS",
    part_ids: inspection.parts,
    material_names: inspection.material_names,
    triangle_count: inspection.triangles,
    aabb: inspection.aabb,
    mesh_count: inspection.mesh_count,
    texture_count: pngs.size,
    material_texture_bindings: Object.fromEntries([...materialBindings.entries()].sort()),
    actual_engine_roundtrip: true,
    actual_commercial_engine_roundtrip: false,
    commercial_engine_roundtrip: "NOT_RUN",
    candidate_confirmed: false,
    export_performed: false,
    quality_status: "source_structural_only",
    visual_quality_status: "QUALITY_TARGET_NOT_MET",
  };
};

if (probe.mode === "static-only") {
  process.stdout.write(`${JSON.stringify(await runStaticOnly(probe))}\n`);
} else if (probe.mode === "hero-material-engine") {
  process.stdout.write(`${JSON.stringify(await runHeroMaterialEngine(probe))}\n`);
} else {
  if (probe.mode !== undefined) throw new Error(`unsupported probe mode: ${probe.mode}`);
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
}
