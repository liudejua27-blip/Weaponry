#!/usr/bin/env node
/* Isolated Manifold WASM provenance/cancellation evidence adapter. */
import crypto from "node:crypto"
import fs from "node:fs"

const moduleUrl = process.argv[2]
const busyIndex = process.argv.indexOf("--busy-probe")
const {default: Module} = await import(moduleUrl)
const module = await Module()
module.setup()

const TAGS = {
  base: {source_id: "source_base", material_id: "material_shell", zone_id: "zone_shell", codes: [101, 1001, 10001]},
  tool: {source_id: "source_tool", material_id: "material_cut", zone_id: "zone_cut", codes: [202, 2002, 20002]},
}

const FIXTURES = {
  vehicle_window_subtract: [[1800, 800, 600], [520, 500, 700], [250, 0, 0], "subtract"],
  aircraft_canopy_subtract: [[1600, 650, 500], [700, 420, 520], [120, 80, 0], "subtract"],
  appliance_vent_subtract: [[900, 700, 1100], [500, 800, 260], [0, 0, 350], "subtract"],
  robot_arm_housing_union: [[700, 700, 800], [480, 480, 900], [260, 0, 0], "union"],
  coplanar_subtract: [[1000, 700, 500], [420, 520, 620], [290, 0, 0], "subtract"],
  near_degenerate_subtract: [[1000, 700, 500], [420, 520, 620], [289.999999, 0, 0], "subtract"],
}

function taggedCube(size, offset, tag) {
  const raw = module.Manifold.cube(size, true)
  const moved = offset.some(value => value !== 0) ? raw.translate(offset) : raw
  if (moved !== raw) raw.delete()
  const tagged = moved.setProperties(3, (next) => { next.splice(0, 3, ...TAGS[tag].codes) })
  moved.delete()
  return tagged
}

function originalId(value) {
  const mesh = value.getMesh()
  if (mesh.runOriginalID.length !== 1) throw new Error("tagged input must have exactly one original run")
  return mesh.runOriginalID[0]
}

function fixturePayload(fixture) {
  const [size, toolSize, offset, operation] = fixture
  const base = taggedCube(size, [0, 0, 0], "base")
  const tool = taggedCube(toolSize, offset, "tool")
  const sourceByOriginal = new Map([[originalId(base), TAGS.base], [originalId(tool), TAGS.tool]])
  const raw = operation === "union" ? base.add(tool) : base.subtract(tool)
  const optimized = raw.simplify(1e-7)
  const mesh = optimized.getMesh()
  if (mesh.numProp !== 6) throw new Error("candidate lost provenance property channels")
  if (mesh.runIndex.length !== mesh.runOriginalID.length + 1) throw new Error("candidate run metadata is incomplete")
  const triangles = []
  for (let run = 0; run < mesh.runOriginalID.length; run += 1) {
    const source = sourceByOriginal.get(mesh.runOriginalID[run])
    if (!source) throw new Error(`unknown source original id: ${mesh.runOriginalID[run]}`)
    const first = mesh.runIndex[run] / 3
    const end = mesh.runIndex[run + 1] / 3
    const backside = mesh.backside(run)
    for (let triangleIndex = first; triangleIndex < end; triangleIndex += 1) {
      const indices = Array.from(mesh.triVerts.subarray(triangleIndex * 3, triangleIndex * 3 + 3))
      const vertices = indices.map(index => {
        const start = index * mesh.numProp
        const props = Array.from(mesh.vertProperties.subarray(start + 3, start + 6))
        if (props.some((value, propertyIndex) => Math.abs(value - source.codes[propertyIndex]) > 1e-5)) {
          throw new Error("candidate mixed source/material/zone property channels")
        }
        return Array.from(mesh.vertProperties.subarray(start, start + 3))
      })
      const faceId = mesh.faceID[triangleIndex]
      triangles.push({
        vertices_mm: vertices,
        source_id: source.source_id,
        material_id: source.material_id,
        zone_id: source.zone_id,
        source_face_id: faceId,
        backside,
        surface_role: backside ? "boolean_cut" : `source_face_${faceId}`,
      })
    }
  }
  base.delete(); tool.delete(); raw.delete(); optimized.delete()
  if (!triangles.length) throw new Error("candidate produced no triangles")
  const provenance = triangles.map(({vertices_mm: _vertices, ...item}) => item)
  return {
    operation,
    triangle_count: triangles.length,
    provenance_sha256: crypto.createHash("sha256").update(JSON.stringify(provenance)).digest("hex"),
    source_ids: [...new Set(triangles.map(item => item.source_id))].sort(),
    material_ids: [...new Set(triangles.map(item => item.material_id))].sort(),
    zone_ids: [...new Set(triangles.map(item => item.zone_id))].sort(),
    has_backside_cut_surface: triangles.some(item => item.backside),
    optimized: true,
    triangles,
  }
}

if (busyIndex >= 0) {
  const marker = process.argv[busyIndex + 1]
  const output = process.argv[busyIndex + 2]
  fs.mkdirSync(new URL(".", `file://${marker}`).pathname, {recursive: true})
  fs.writeFileSync(marker, JSON.stringify({pid: process.pid, state: "kernel_loop_started"}))
  let counter = 0
  while (true) {
    const base = module.Manifold.sphere(120, 96)
    const tool = module.Manifold.cube([180, 180, 180], true).translate([counter % 17, 0, 0])
    const result = base.subtract(tool)
    result.numTri()
    base.delete(); tool.delete(); result.delete()
    counter += 1
  }
  fs.writeFileSync(output, "unreachable")
} else {
  const first = Object.fromEntries(Object.entries(FIXTURES).map(([id, fixture]) => [id, fixturePayload(fixture)]))
  const second = Object.fromEntries(Object.entries(FIXTURES).map(([id, fixture]) => [id, fixturePayload(fixture)]))
  const deterministic = Object.keys(first).every(id => first[id].provenance_sha256 === second[id].provenance_sha256)
  console.log(JSON.stringify({
    adapter: "manifold_wasm",
    fixtures: first,
    deterministic_provenance: deterministic,
    property_channels_verified: true,
    simplify_provenance_verified: true,
  }))
}
