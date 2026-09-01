import * as THREE from 'three'

import type { KnifeMaterialZone } from './knife-scene-program.ts'

const STABLE_ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const MIN_SCALE_REPEAT = 0.25
const MAX_SCALE_REPEAT = 16
const TWO_PI = Math.PI * 2

export const KNIFE_LAYERED_MATERIAL_SCHEMA_VERSION = 'KnifeLayeredMaterialSpec@1' as const

export const KNIFE_MATERIAL_VOCABULARY = [
  'red-lacquer-metal',
  'antique-gold',
  'black-wrapped-grip',
  'ruby-emissive',
] as const

export type KnifeMaterialVocabulary = typeof KNIFE_MATERIAL_VOCABULARY[number]

export type KnifeMaterialLayer =
  | 'substrate'
  | 'lacquer'
  | 'metal'
  | 'patina'
  | 'wrap'
  | 'stone'
  | 'emission'
  | 'edge-wear'
  | 'engraving'

export interface KnifeMaterialControls {
  /** Bounded strength of the derived curvature channel. */
  readonly curvature: number
  /** Bounded strength of the derived edge-wear channel. */
  readonly edge_wear: number
  /** Bounded strength of the deterministic engraving-mask channel. */
  readonly engraving_mask: number
  /** Deterministic U/V repeat factors for the procedural channels. */
  readonly scale_repeat: readonly [number, number]
}

/**
 * Closed, texture-free material input for the lightweight Three.js route.
 *
 * The vocabulary chooses a first-party palette and built-in Three.js PBR
 * properties.  Procedural controls become named geometry attributes; callers
 * cannot provide a URL, texture, shader, or executable code through this type.
 */
export interface KnifeLayeredMaterialSpec {
  readonly schema_version: typeof KNIFE_LAYERED_MATERIAL_SCHEMA_VERSION
  readonly material_zone_id: string
  readonly vocabulary: KnifeMaterialVocabulary
  readonly controls: KnifeMaterialControls
}

export interface KnifeMaterialPalette {
  readonly layers: readonly KnifeMaterialLayer[]
  readonly clearcoat: number
  readonly clearcoat_roughness: number
  readonly emissive: string
  readonly emissive_intensity: number
  readonly wear_color: string
  readonly engraving_color: string
}

export const KNIFE_MATERIAL_ATTRIBUTE_NAMES = Object.freeze({
  curvature: 'materialCurvature',
  edge_wear: 'materialEdgeWear',
  engraving_mask: 'materialEngravingMask',
  scale_repeat: 'materialScaleRepeat',
}) as {
  readonly curvature: 'materialCurvature'
  readonly edge_wear: 'materialEdgeWear'
  readonly engraving_mask: 'materialEngravingMask'
  readonly scale_repeat: 'materialScaleRepeat'
}

/** Built-in Three.js vertex-color attribute used for visible bounded layering. */
export const KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE = 'color' as const

export class KnifeMaterialSpecError extends Error {
  constructor(message: string) {
    super(`INVALID_KNIFE_MATERIAL_SPEC: ${message}`)
    this.name = 'KnifeMaterialSpecError'
  }
}

const PALETTES: Readonly<Record<KnifeMaterialVocabulary, KnifeMaterialPalette>> = {
  'red-lacquer-metal': {
    layers: ['substrate', 'lacquer', 'edge-wear', 'engraving'],
    clearcoat: 0.46,
    clearcoat_roughness: 0.2,
    emissive: '#000000',
    emissive_intensity: 0,
    wear_color: '#B99A63',
    engraving_color: '#2A0707',
  },
  'antique-gold': {
    layers: ['metal', 'patina', 'edge-wear', 'engraving'],
    clearcoat: 0.12,
    clearcoat_roughness: 0.3,
    emissive: '#000000',
    emissive_intensity: 0,
    wear_color: '#E8C875',
    engraving_color: '#3A2108',
  },
  'black-wrapped-grip': {
    layers: ['wrap', 'edge-wear', 'engraving'],
    clearcoat: 0.05,
    clearcoat_roughness: 0.48,
    emissive: '#000000',
    emissive_intensity: 0,
    wear_color: '#6A6258',
    engraving_color: '#090909',
  },
  'ruby-emissive': {
    layers: ['stone', 'emission', 'edge-wear', 'engraving'],
    clearcoat: 0.38,
    clearcoat_roughness: 0.14,
    emissive: '#C51A43',
    emissive_intensity: 0.78,
    wear_color: '#F19A86',
    engraving_color: '#28020B',
  },
}

const DEFAULT_CONTROLS: Readonly<Record<KnifeMaterialVocabulary, KnifeMaterialControls>> = {
  'red-lacquer-metal': { curvature: 0.35, edge_wear: 0.28, engraving_mask: 0.18, scale_repeat: [1, 1] },
  'antique-gold': { curvature: 0.28, edge_wear: 0.22, engraving_mask: 0.24, scale_repeat: [1, 1] },
  'black-wrapped-grip': { curvature: 0.2, edge_wear: 0.16, engraving_mask: 0.06, scale_repeat: [4, 2] },
  'ruby-emissive': { curvature: 0.42, edge_wear: 0.12, engraving_mask: 0.04, scale_repeat: [1, 1] },
}

function exactKeys(value: unknown, expected: readonly string[], label: string): asserts value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new KnifeMaterialSpecError(`${label} must be an object`)
  const keys = Object.keys(value).sort()
  const wanted = [...expected].sort()
  if (keys.length !== wanted.length || keys.some((key, index) => key !== wanted[index])) {
    throw new KnifeMaterialSpecError(`${label} keys are not closed`)
  }
}

function finiteNumber(value: unknown, label: string, minimum: number, maximum: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < minimum || value > maximum) {
    throw new KnifeMaterialSpecError(`${label} must be finite and in [${minimum}, ${maximum}]`)
  }
  return value
}

function boundedId(value: unknown, label: string): string {
  if (typeof value !== 'string' || !STABLE_ID_PATTERN.test(value)) {
    throw new KnifeMaterialSpecError(`${label} must be a bounded stable ID`)
  }
  return value
}

function vocabulary(value: unknown): KnifeMaterialVocabulary {
  if (typeof value !== 'string' || !(KNIFE_MATERIAL_VOCABULARY as readonly string[]).includes(value)) {
    throw new KnifeMaterialSpecError(`vocabulary is outside the first-party bounded set`)
  }
  return value as KnifeMaterialVocabulary
}

function validateControls(value: unknown): asserts value is KnifeMaterialControls {
  exactKeys(value, ['curvature', 'edge_wear', 'engraving_mask', 'scale_repeat'], 'controls')
  finiteNumber(value.curvature, 'controls.curvature', 0, 1)
  finiteNumber(value.edge_wear, 'controls.edge_wear', 0, 1)
  finiteNumber(value.engraving_mask, 'controls.engraving_mask', 0, 1)
  if (!Array.isArray(value.scale_repeat) || value.scale_repeat.length !== 2) {
    throw new KnifeMaterialSpecError('controls.scale_repeat must contain exactly two values')
  }
  finiteNumber(value.scale_repeat[0], 'controls.scale_repeat[0]', MIN_SCALE_REPEAT, MAX_SCALE_REPEAT)
  finiteNumber(value.scale_repeat[1], 'controls.scale_repeat[1]', MIN_SCALE_REPEAT, MAX_SCALE_REPEAT)
}

export function validateKnifeLayeredMaterialSpec(value: unknown): asserts value is KnifeLayeredMaterialSpec {
  exactKeys(value, ['schema_version', 'material_zone_id', 'vocabulary', 'controls'], 'KnifeLayeredMaterialSpec')
  if (value.schema_version !== KNIFE_LAYERED_MATERIAL_SCHEMA_VERSION) {
    throw new KnifeMaterialSpecError('schema_version drifted')
  }
  boundedId(value.material_zone_id, 'material_zone_id')
  vocabulary(value.vocabulary)
  validateControls(value.controls)
}

export function validateKnifeLayeredMaterialSpecSet(
  specs: readonly KnifeLayeredMaterialSpec[] | undefined,
  materialZoneIds?: readonly string[],
): void {
  if (specs === undefined) return
  if (!Array.isArray(specs)) throw new KnifeMaterialSpecError('material_specs must be an array')
  const seen = new Set<string>()
  const known = materialZoneIds ? new Set(materialZoneIds) : undefined
  for (const spec of specs) {
    validateKnifeLayeredMaterialSpec(spec)
    if (seen.has(spec.material_zone_id)) throw new KnifeMaterialSpecError(`duplicate material_zone_id ${spec.material_zone_id}`)
    seen.add(spec.material_zone_id)
    if (known && !known.has(spec.material_zone_id)) {
      throw new KnifeMaterialSpecError(`material_zone_id ${spec.material_zone_id} is not bound by the program`)
    }
  }
}

export function knifeMaterialPalette(kind: KnifeMaterialVocabulary): KnifeMaterialPalette {
  return PALETTES[kind]
}

export function inferKnifeMaterialVocabulary(zone: KnifeMaterialZone, partId = ''): KnifeMaterialVocabulary {
  const token = `${zone.material_zone_id}:${partId}`.toLowerCase()
  if (token.includes('grip') || token.includes('wrap') || token.includes('handle')) return 'black-wrapped-grip'
  if (token.includes('ruby') || token.includes('gem') || token.includes('jewel')) return 'ruby-emissive'
  if (token.includes('gold') || token.includes('ornament') || token.includes('edge')) return 'antique-gold'
  if (token.includes('red') || token.includes('lacquer') || token.includes('blade')) return 'red-lacquer-metal'
  if (zone.metalness >= 0.82) return 'antique-gold'
  if (zone.metalness <= 0.18) return 'black-wrapped-grip'
  return 'red-lacquer-metal'
}

export function createDefaultKnifeLayeredMaterialSpec(zone: KnifeMaterialZone, partId = ''): KnifeLayeredMaterialSpec {
  const kind = inferKnifeMaterialVocabulary(zone, partId)
  const controls = DEFAULT_CONTROLS[kind]
  return Object.freeze({
    schema_version: KNIFE_LAYERED_MATERIAL_SCHEMA_VERSION,
    material_zone_id: zone.material_zone_id,
    vocabulary: kind,
    controls: Object.freeze({
      curvature: controls.curvature,
      edge_wear: controls.edge_wear,
      engraving_mask: controls.engraving_mask,
      scale_repeat: Object.freeze([...controls.scale_repeat] as [number, number]),
    }),
  })
}

export function resolveKnifeLayeredMaterialSpec(
  zone: KnifeMaterialZone,
  partId: string,
  specs?: readonly KnifeLayeredMaterialSpec[],
): KnifeLayeredMaterialSpec {
  validateKnifeLayeredMaterialSpecSet(specs)
  const explicit = specs?.find((candidate) => candidate.material_zone_id === zone.material_zone_id)
  return explicit ?? createDefaultKnifeLayeredMaterialSpec(zone, partId)
}

export function createKnifeLayeredMaterial(
  spec: KnifeLayeredMaterialSpec,
  zone: KnifeMaterialZone,
  partId: string,
): THREE.MeshPhysicalMaterial {
  validateKnifeLayeredMaterialSpec(spec)
  if (spec.material_zone_id !== zone.material_zone_id) {
    throw new KnifeMaterialSpecError(`spec zone ${spec.material_zone_id} does not match ${zone.material_zone_id}`)
  }
  const palette = knifeMaterialPalette(spec.vocabulary)
  const material = new THREE.MeshPhysicalMaterial({
    color: new THREE.Color(zone.base_color),
    metalness: zone.metalness,
    roughness: zone.roughness,
    vertexColors: true,
    clearcoat: palette.clearcoat,
    clearcoatRoughness: palette.clearcoat_roughness,
    emissive: palette.emissive,
    emissiveIntensity: palette.emissive_intensity,
  })
  material.userData = {
    material_schema_version: KNIFE_LAYERED_MATERIAL_SCHEMA_VERSION,
    material_zone_id: zone.material_zone_id,
    part_id: partId,
    model: zone.model,
    vocabulary: spec.vocabulary,
    controls: {
      curvature: spec.controls.curvature,
      edge_wear: spec.controls.edge_wear,
      engraving_mask: spec.controls.engraving_mask,
      scale_repeat: [...spec.controls.scale_repeat],
    },
    layers: [...palette.layers],
    palette: {
      clearcoat: palette.clearcoat,
      clearcoat_roughness: palette.clearcoat_roughness,
      emissive: palette.emissive,
      emissive_intensity: palette.emissive_intensity,
      wear_color: palette.wear_color,
      engraving_color: palette.engraving_color,
    },
    procedural: {
      texture_source: 'none',
      shader_source: 'built-in-threejs-meshphysical@1',
      visible_layering: 'built-in-vertex-color@1',
      network_used: false,
      arbitrary_shader: false,
      arbitrary_code: false,
    },
  }
  return material
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function normalDistance(attribute: THREE.BufferAttribute | THREE.InterleavedBufferAttribute, left: number, right: number): number {
  const dx = attribute.getX(left) - attribute.getX(right)
  const dy = attribute.getY(left) - attribute.getY(right)
  const dz = attribute.getZ(left) - attribute.getZ(right)
  return Math.sqrt(dx * dx + dy * dy + dz * dz)
}

/**
 * Attach the four bounded procedural channels to a derived geometry.
 *
 * These are data attributes for the first-party material path.  The compiler
 * does not install `onBeforeCompile`, `ShaderMaterial`, texture URLs, or any
 * caller-provided executable source.
 */
export function bindKnifeLayeredMaterialGeometry(
  geometry: THREE.BufferGeometry,
  spec: KnifeLayeredMaterialSpec,
): void {
  validateKnifeLayeredMaterialSpec(spec)
  const positions = geometry.getAttribute('position')
  if (!positions || positions.itemSize !== 3 || positions.count === 0) {
    throw new KnifeMaterialSpecError('geometry requires a non-empty position attribute')
  }
  const normals = geometry.getAttribute('normal')
  const uvs = geometry.getAttribute('uv')
  const sectionUs = geometry.getAttribute('sectionU')
  const count = positions.count
  const curvature = new Float32Array(count)
  const edgeWear = new Float32Array(count)
  const engravingMask = new Float32Array(count)
  const scaleRepeat = new Float32Array(count * 2)
  const visibleColors = new Float32Array(count * 3)
  const [repeatU, repeatV] = spec.controls.scale_repeat
  const palette = knifeMaterialPalette(spec.vocabulary)
  const wearColor = new THREE.Color(palette.wear_color)
  const engravingColor = new THREE.Color(palette.engraving_color)

  for (let index = 0; index < count; index += 1) {
    let curvatureProxy = 0
    if (normals && normals.itemSize === 3) {
      const neighborCount = (index > 0 ? 1 : 0) + (index + 1 < count ? 1 : 0)
      if (neighborCount > 0) {
        const variation = (index > 0 ? normalDistance(normals, index, index - 1) : 0)
          + (index + 1 < count ? normalDistance(normals, index, index + 1) : 0)
        curvatureProxy = clamp01(variation / (2 * neighborCount))
      }
    }
    const v = uvs && uvs.itemSize >= 2 ? clamp01(uvs.getY(index)) : 0.5
    const u = uvs && uvs.itemSize >= 1
      ? uvs.getX(index)
      : sectionUs && sectionUs.itemSize >= 1
        ? sectionUs.getX(index)
        : index / Math.max(count - 1, 1)
    const edgeProximity = 1 - clamp01(Math.min(v, 1 - v) * 2)
    const engravingPhase = (u * repeatU + v * repeatV) % 1
    const positivePhase = engravingPhase < 0 ? engravingPhase + 1 : engravingPhase
    const engravingWave = 0.5 + 0.5 * Math.sin(TWO_PI * positivePhase)
    const curvatureValue = curvatureProxy * spec.controls.curvature
    const edgeWearValue = edgeProximity * spec.controls.edge_wear
    const engravingValue = engravingWave * spec.controls.engraving_mask
    curvature[index] = curvatureValue
    edgeWear[index] = edgeWearValue
    engravingMask[index] = engravingValue
    scaleRepeat[index * 2] = repeatU
    scaleRepeat[index * 2 + 1] = repeatV

    // MeshPhysicalMaterial multiplies its base color by `color`.  Start from a
    // bounded curvature brightness, then blend fixed first-party wear and
    // engraving colors into that built-in vertex-color channel.  This makes
    // all three channels visible without a custom shader or texture lookup.
    const visibleColor = new THREE.Color(1, 1, 1)
    visibleColor.multiplyScalar(0.68 + 0.32 * curvatureValue)
    visibleColor.lerp(wearColor, clamp01(edgeWearValue * 0.92))
    visibleColor.lerp(engravingColor, clamp01(engravingValue * 0.86))
    visibleColors[index * 3] = clamp01(visibleColor.r)
    visibleColors[index * 3 + 1] = clamp01(visibleColor.g)
    visibleColors[index * 3 + 2] = clamp01(visibleColor.b)
  }

  geometry.setAttribute(KNIFE_MATERIAL_ATTRIBUTE_NAMES.curvature, new THREE.Float32BufferAttribute(curvature, 1))
  geometry.setAttribute(KNIFE_MATERIAL_ATTRIBUTE_NAMES.edge_wear, new THREE.Float32BufferAttribute(edgeWear, 1))
  geometry.setAttribute(KNIFE_MATERIAL_ATTRIBUTE_NAMES.engraving_mask, new THREE.Float32BufferAttribute(engravingMask, 1))
  geometry.setAttribute(KNIFE_MATERIAL_ATTRIBUTE_NAMES.scale_repeat, new THREE.Float32BufferAttribute(scaleRepeat, 2))
  geometry.setAttribute(KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE, new THREE.Float32BufferAttribute(visibleColors, 3))
  geometry.userData = {
    ...geometry.userData,
    material_schema_version: KNIFE_LAYERED_MATERIAL_SCHEMA_VERSION,
    material_zone_id: spec.material_zone_id,
    material_vocabulary: spec.vocabulary,
    material_attribute_names: { ...KNIFE_MATERIAL_ATTRIBUTE_NAMES },
    material_vertex_color_attribute: KNIFE_MATERIAL_VERTEX_COLOR_ATTRIBUTE,
    material_channels: {
      curvature: spec.controls.curvature,
      edge_wear: spec.controls.edge_wear,
      engraving_mask: spec.controls.engraving_mask,
      scale_repeat: [...spec.controls.scale_repeat],
    },
    procedural_texture_source: 'none',
    arbitrary_shader: false,
    arbitrary_code: false,
  }
}
