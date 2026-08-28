import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { dirname, resolve } from "node:path";

const scriptPath = resolve(dirname(fileURLToPath(import.meta.url)), "check_threejs_game_asset_probe.mjs");
const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const candidateId = "candidate-threejs-static-fixture";
const candidateStateSha256 = "a".repeat(64);
const partIds = ["part-a", "part-b"];
const materialNames = ["zone-a", "zone-b"];

const sha256Hex = (value) => createHash("sha256").update(value).digest("hex");
const canonicalJson = (value) => {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
};

const buildStaticGlb = ({ externalUri = false } = {}) => {
  const positions = Buffer.alloc(6 * 3 * 4);
  const vertices = [
    [0, 0, 0], [1, 0, 0], [0, 1, 0],
    [1, 0, 0], [2, 0, 0], [1, 1, 0],
  ];
  vertices.flat().forEach((value, index) => positions.writeFloatLE(value, index * 4));
  const indices = Buffer.alloc(6 * 2);
  [0, 1, 2, 0, 1, 2].forEach((value, index) => indices.writeUInt16LE(value, index * 2));
  const binary = Buffer.concat([positions, indices]);
  const gltf = {
    asset: { version: "2.0", generator: "forgecad-threejs-static-fixture" },
    scene: 0,
    scenes: [{ nodes: [0, 1] }],
    nodes: [
      { name: "part-a", mesh: 0 },
      { name: "part-b", mesh: 1 },
    ],
    meshes: [
      { name: "mesh-a", primitives: [{ attributes: { POSITION: 0 }, indices: 2, material: 0 }] },
      { name: "mesh-b", primitives: [{ attributes: { POSITION: 1 }, indices: 3, material: 1 }] },
    ],
    materials: [
      { name: "zone-a", pbrMetallicRoughness: { baseColorFactor: [1, 0, 0, 1] } },
      { name: "zone-b", pbrMetallicRoughness: { baseColorFactor: [0, 1, 0, 1] } },
    ],
    accessors: [
      { bufferView: 0, componentType: 5126, count: 3, type: "VEC3", min: [0, 0, 0], max: [1, 1, 0] },
      { bufferView: 0, byteOffset: 36, componentType: 5126, count: 3, type: "VEC3", min: [1, 0, 0], max: [2, 1, 0] },
      { bufferView: 1, componentType: 5123, count: 3, type: "SCALAR", min: [0], max: [2] },
      { bufferView: 1, byteOffset: 6, componentType: 5123, count: 3, type: "SCALAR", min: [0], max: [2] },
    ],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: positions.length, target: 34962 },
      { buffer: 0, byteOffset: positions.length, byteLength: indices.length, target: 34963 },
    ],
    buffers: [{ byteLength: binary.length, ...(externalUri ? { uri: "https://example.invalid/external.bin" } : {}) }],
  };
  const json = Buffer.from(JSON.stringify(gltf));
  const jsonPadding = Buffer.alloc((4 - (json.length % 4)) % 4, 0x20);
  const jsonChunk = Buffer.concat([json, jsonPadding]);
  const binaryPadding = Buffer.alloc((4 - (binary.length % 4)) % 4);
  const binaryChunk = Buffer.concat([binary, binaryPadding]);
  const totalLength = 12 + 8 + jsonChunk.length + 8 + binaryChunk.length;
  const header = Buffer.alloc(12);
  header.write("glTF", 0, "ascii");
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(totalLength, 8);
  const jsonHeader = Buffer.alloc(8);
  jsonHeader.writeUInt32LE(jsonChunk.length, 0);
  jsonHeader.writeUInt32LE(0x4e4f534a, 4);
  const binaryHeader = Buffer.alloc(8);
  binaryHeader.writeUInt32LE(binaryChunk.length, 0);
  binaryHeader.writeUInt32LE(0x004e4942, 4);
  return Buffer.concat([header, jsonHeader, jsonChunk, binaryHeader, binaryChunk]);
};

const makeProbe = (artifact) => {
  const artifactSha256 = sha256Hex(artifact);
  const readback = {
    schema_version: "ArtifactReadback@2",
    artifact_id: artifactSha256,
    object_sha256: artifactSha256,
    candidate_id: candidateId,
    hard_gate_passed: true,
    integrity: { glb_parse_status: "passed", external_uri_count: 0 },
    part_ids: partIds,
    material_zone_ids: materialNames,
  };
  readback.canonical_sha256 = sha256Hex(canonicalJson(readback));
  return {
    mode: "static-only",
    schema_version: "ForgeCadThreeJsStaticGameAssetProbe@1",
    candidate_id: candidateId,
    candidate_state_sha256: candidateStateSha256,
    artifact_base64: artifact.toString("base64"),
    artifact_sha256: artifactSha256,
    artifact_readback: readback,
    artifact_readback_sha256: readback.canonical_sha256,
    part_ids: partIds,
    material_names: materialNames,
    triangle_count: 2,
    aabb: { min_m: [0, 0, 0], max_m: [2, 1, 0] },
    restart_hash_passed: true,
    durable_get_passed: true,
    candidate_confirmed: false,
    export_performed: false,
  };
};

const runProbe = (probe) =>
  new Promise((resolveRun, reject) => {
    const child = spawn(process.execPath, [scriptPath], {
      cwd: repositoryRoot,
      stdio: ["pipe", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("error", reject);
    child.on("close", (code) => resolveRun({ code, stdout, stderr }));
    child.stdin.end(JSON.stringify(probe));
  });

const positive = await runProbe(makeProbe(buildStaticGlb()));
assert.equal(positive.code, 0, positive.stderr);
const receipt = JSON.parse(positive.stdout);
assert.equal(receipt.schema_version, "ThreeJsStaticGameAssetSourceReceipt@1");
assert.equal(receipt.mode, "static-only");
assert.equal(receipt.three_revision, "185");
assert.equal(receipt.three_package_source, "root-node_modules/three");
assert.equal(receipt.static_glb_parse, "PASS");
assert.equal(receipt.stable_part_nodes, "PASS");
assert.equal(receipt.static_material_readback, "PASS");
assert.equal(receipt.triangle_readback, "PASS");
assert.equal(receipt.aabb_readback, "PASS");
assert.equal(receipt.no_external_uris, "PASS");
assert.equal(receipt.external_uri_count, 0);
assert.equal(receipt.animation_status, "ABSENT_EXPECTED");
assert.equal(receipt.triangle_count, 2);
assert.deepEqual(receipt.part_ids, partIds);
assert.deepEqual(receipt.material_names, materialNames);
assert.deepEqual(receipt.aabb, { min_m: [0, 0, 0], max_m: [2, 1, 0] });
assert.equal(receipt.runtime_restart_hash, "PASS");
assert.equal(receipt.durable_get_after_restart, "PASS");
assert.equal(receipt.threejs_source_readback, true);
assert.equal(receipt.actual_engine_roundtrip, false);
assert.equal(receipt.actual_commercial_engine_roundtrip, false);
assert.equal(receipt.commercial_engine_roundtrip, "NOT_RUN");
assert.equal(receipt.godot, "NOT_RUN");
assert.equal(receipt.unity, "NOT_RUN");
assert.equal(receipt.unreal, "NOT_RUN");
assert.equal(receipt.quality_status, "structural_only");
assert.equal(JSON.stringify(receipt).includes(repositoryRoot), false);

const wrongHash = makeProbe(buildStaticGlb());
wrongHash.artifact_sha256 = "f".repeat(64);
const rejectedHash = await runProbe(wrongHash);
assert.notEqual(rejectedHash.code, 0);
assert.match(rejectedHash.stderr, /static artifact hash differs/);

const externalUri = makeProbe(buildStaticGlb({ externalUri: true }));
const rejectedExternalUri = await runProbe(externalUri);
assert.notEqual(rejectedExternalUri.code, 0);
assert.match(rejectedExternalUri.stderr, /external URI field/);

process.stdout.write("Three.js static-only game asset probe fixture: PASS\n");
