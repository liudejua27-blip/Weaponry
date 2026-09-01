import * as THREE from 'three'

import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import {
  KNIFE_MATERIAL_ATTRIBUTE_NAMES,
  bindKnifeLayeredMaterialGeometry,
  compileKnifeScene,
  createKnifeLayeredMaterial,
  KnifeMaterialSpecError,
  type KnifeLayeredMaterialSpec,
} from '../src/index.ts'
import { KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE } from '../src/knife-material.ts'

const vocabularies = [
  'red-lacquer-metal',
  'antique-gold',
  'black-wrapped-grip',
  'ruby-emissive',
] as const

const vocabularyByZone: Record<string, KnifeLayeredMaterialSpec['vocabulary']> = {
  'blade-steel': 'red-lacquer-metal',
  'edge-steel': 'antique-gold',
  'assembly-metal': 'antique-gold',
  'assembly-grip': 'black-wrapped-grip',
}

const specs: KnifeLayeredMaterialSpec[] = knifeSceneProgramFixture.material_zones.map((zone, index) => ({
  schema_version: 'KnifeLayeredMaterialSpec@1',
  material_zone_id: zone.material_zone_id,
  vocabulary: vocabularyByZone[zone.material_zone_id],
  controls: {
    curvature: 0.2 + index * 0.1,
    edge_wear: 0.15 + index * 0.05,
    engraving_mask: index === 0 ? 0 : 0.1,
    scale_repeat: index === 2 ? [4, 2] : [1, 1],
  },
}))

const first = compileKnifeScene(knifeSceneProgramFixture, {
  longitudinal_segments: 16,
  material_specs: specs,
})
const second = compileKnifeScene(knifeSceneProgramFixture, {
  longitudinal_segments: 16,
  material_specs: specs,
})

if (first.deterministic_fingerprint !== second.deterministic_fingerprint) {
  throw new Error('material compile is not deterministic')
}
if (first.parts.length !== 5) throw new Error('material smoke fixture part count drifted')

for (const part of first.parts) {
  const spec = specs.find((candidate) => candidate.material_zone_id === part.material_zone_id)
  if (spec && part.material_spec.vocabulary !== spec.vocabulary) throw new Error(`material spec did not bind ${part.part_id}`)
  if (!(part.material instanceof THREE.MeshPhysicalMaterial)) throw new Error(`material ${part.part_id} is not the bounded physical material`)
  if (part.material.vertexColors !== true) throw new Error(`material ${part.part_id} did not enable vertexColors`)
  const attributes = part.geometry.attributes
  for (const name of Object.values(KNIFE_MATERIAL_ATTRIBUTE_NAMES)) {
    const attribute = attributes[name]
    if (!attribute || attribute.count !== attributes.position.count || [...attribute.array].some((value) => !Number.isFinite(value))) {
      throw new Error(`material attribute ${name} is missing or non-finite on ${part.part_id}`)
    }
  }
  const vertexColors = attributes[KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE]
  if (!vertexColors || vertexColors.itemSize !== 3 || vertexColors.count !== attributes.position.count
    || [...vertexColors.array].some((value) => !Number.isFinite(value) || value < 0 || value > 1)) {
    throw new Error(`visible vertex colors are missing or out of bounds on ${part.part_id}`)
  }
  if (part.geometry.userData.procedural_texture_source !== 'none' || part.material.userData.procedural.arbitrary_shader !== false) {
    throw new Error(`material ${part.part_id} crossed the closed procedural boundary`)
  }
}

function sampledVertexColors(spec: KnifeLayeredMaterialSpec): number[] {
  const geometry = new THREE.BoxGeometry(1, 1, 1, 2, 2, 2)
  geometry.computeVertexNormals()
  bindKnifeLayeredMaterialGeometry(geometry, spec)
  return [...geometry.getAttribute(KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE).array]
}

function differs(left: readonly number[], right: readonly number[]): boolean {
  return left.length === right.length && left.some((value, index) => Math.abs(value - right[index]) > 1e-6)
}

const visibleBaseSpec: KnifeLayeredMaterialSpec = {
  schema_version: 'KnifeLayeredMaterialSpec@1',
  material_zone_id: 'visible-layer-smoke',
  vocabulary: 'red-lacquer-metal',
  controls: { curvature: 0, edge_wear: 0, engraving_mask: 0, scale_repeat: [1, 1] },
}
const neutralColors = sampledVertexColors(visibleBaseSpec)
const curvatureColors = sampledVertexColors({
  ...visibleBaseSpec,
  controls: { ...visibleBaseSpec.controls, curvature: 1 },
})
const wearColors = sampledVertexColors({
  ...visibleBaseSpec,
  controls: { ...visibleBaseSpec.controls, edge_wear: 1 },
})
const engravingColors = sampledVertexColors({
  ...visibleBaseSpec,
  controls: { ...visibleBaseSpec.controls, engraving_mask: 1 },
})
const repeatedEngravingColors = sampledVertexColors({
  ...visibleBaseSpec,
  controls: { ...visibleBaseSpec.controls, engraving_mask: 1, scale_repeat: [2.5, 3.5] },
})
if (!differs(neutralColors, curvatureColors)) throw new Error('curvature did not reach the visible vertex-color channel')
if (!differs(neutralColors, wearColors)) throw new Error('edge wear did not reach the visible vertex-color channel')
if (!differs(neutralColors, engravingColors)) throw new Error('engraving mask did not reach the visible vertex-color channel')
if (!differs(engravingColors, repeatedEngravingColors)) throw new Error('scale repeat did not change visible engraving color')

const rubySpec: KnifeLayeredMaterialSpec = {
  schema_version: 'KnifeLayeredMaterialSpec@1',
  material_zone_id: 'ruby-smoke',
  vocabulary: 'ruby-emissive',
  controls: { curvature: 0.42, edge_wear: 0.12, engraving_mask: 0.04, scale_repeat: [2, 2] },
}
const rubyZone = { ...knifeSceneProgramFixture.material_zones[0], material_zone_id: rubySpec.material_zone_id }
const rubyGeometry = new THREE.BoxGeometry(1, 1, 1)
rubyGeometry.computeVertexNormals()
bindKnifeLayeredMaterialGeometry(rubyGeometry, rubySpec)
const rubyMaterial = createKnifeLayeredMaterial(rubySpec, rubyZone, 'ruby-smoke')
if (!(rubyMaterial instanceof THREE.MeshPhysicalMaterial)
  || rubyMaterial.emissive.getHexString() !== 'c51a43'
  || rubyGeometry.getAttribute(KNIFE_MATERIAL_ATTRIBUTE_NAMES.engraving_mask).count !== rubyGeometry.getAttribute('position').count) {
  throw new Error('ruby-emissive mapping did not bind its built-in material and geometry channels')
}

let rejected = false
try {
  compileKnifeScene(knifeSceneProgramFixture, {
    material_specs: [{ ...specs[0], texture_url: 'https://example.invalid/texture.png' } as never],
  })
} catch (error) {
  rejected = error instanceof KnifeMaterialSpecError
}
if (!rejected) throw new Error('network texture input was not rejected')

console.log(JSON.stringify({
  schema_version: 'KnifeLayeredMaterialSpec@1',
  vocabularies,
  parts: first.parts.map((part) => ({
    part_id: part.part_id,
    material_zone_id: part.material_zone_id,
    vocabulary: part.material_spec.vocabulary,
    material_type: part.material.type,
    vertex_colors: part.material.vertexColors,
    visible_attribute: KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE,
    attributes: Object.values(KNIFE_MATERIAL_ATTRIBUTE_NAMES),
  })),
  visible_vertex_color_channels: {
    curvature: true,
    edge_wear: true,
    engraving_mask: true,
    scale_repeat: true,
  },
  deterministic_fingerprint: first.deterministic_fingerprint,
  ruby_emissive: {
    material_type: rubyMaterial.type,
    emissive: rubyMaterial.emissive.getHexString(),
    attributes: Object.values(KNIFE_MATERIAL_ATTRIBUTE_NAMES),
  },
  network_texture_rejected: rejected,
  renderer_invoked: first.renderer_invoked,
  quality_status: first.quality_status,
}))
