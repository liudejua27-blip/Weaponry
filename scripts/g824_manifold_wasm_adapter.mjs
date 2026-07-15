#!/usr/bin/env node
/* Isolated Manifold WASM benchmark adapter; never imported by production. */
import crypto from "node:crypto"
const {default: Module} = await import(process.argv[2])

const started = performance.now()
const module = await Module()
module.setup()

function runOnce(size, toolSize, offset, operation) {
  const base = module.Manifold.cube(size, true)
  const tool = module.Manifold.cube(toolSize, true).translate(offset)
  const result = operation === "union" ? base.add(tool) : base.subtract(tool)
  const mesh = result.getMesh()
  const hash = crypto.createHash("sha256")
    .update(Buffer.from(mesh.vertProperties.buffer, mesh.vertProperties.byteOffset, mesh.vertProperties.byteLength))
    .update(Buffer.from(mesh.triVerts.buffer, mesh.triVerts.byteOffset, mesh.triVerts.byteLength))
    .digest("hex")
  const triangles = result.numTri()
  base.delete(); tool.delete(); result.delete()
  return {hash, triangles}
}

const fixtures = {
  vehicle_window_subtract: [[1800, 800, 600], [520, 500, 700], [250, 0, 0], "subtract"],
  aircraft_canopy_subtract: [[1600, 650, 500], [700, 420, 520], [120, 80, 0], "subtract"],
  appliance_vent_subtract: [[900, 700, 1100], [500, 800, 260], [0, 0, 350], "subtract"],
  robot_arm_housing_union: [[700, 700, 800], [480, 480, 900], [260, 0, 0], "union"],
}
const first = runOnce(...fixtures.vehicle_window_subtract)
const coldMs = performance.now() - started
const durations = []
const fixtureResults = {}
for (const [fixtureId, fixture] of Object.entries(fixtures)) {
  const hashes = []
  let triangles = 0
  for (let index = 0; index < 5; index += 1) {
    const begin = performance.now()
    const result = runOnce(...fixture)
    durations.push(performance.now() - begin)
    hashes.push(result.hash); triangles = result.triangles
  }
  fixtureResults[fixtureId] = {triangle_count: triangles, deterministic: new Set(hashes).size === 1, sha256: hashes[0]}
}
durations.sort((a, b) => a - b)
const repeated = runOnce(...fixtures.vehicle_window_subtract)
const coplanar = runOnce([1000,700,500], [420,520,620], [290,0,0], "subtract")
const near = runOnce([1000,700,500], [420,520,620], [289.999999,0,0], "subtract")
console.log(JSON.stringify({
  adapter: "manifold_wasm",
  cold_ms: Number(coldMs.toFixed(4)),
  warm_median_ms: Number(durations[Math.floor(durations.length / 2)].toFixed(4)),
  peak_rss_kib: Math.round(process.memoryUsage().rss / 1024),
  triangle_count: first.triangles,
  deterministic_identical_fixture: first.hash === repeated.hash,
  fixture_results: fixtureResults,
  near_degenerate_completed: Boolean(near.hash),
  coplanar_completed: Boolean(coplanar.hash),
  material_surface_provenance_verified: false,
  cancellation_verified: false,
}))
