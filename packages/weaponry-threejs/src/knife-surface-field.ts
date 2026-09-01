import * as THREE from 'three'

import {
  knifeMaterialPalette,
  validateKnifeLayeredMaterialSpec,
  type KnifeLayeredMaterialSpec,
} from './knife-material.ts'
import { sha256Hex } from './knife-browser-capture.ts'

/** Versioned derived surface data; this is not a replacement for the v1 material spec. */
export const LAYERED_SURFACE_FIELD_SCHEMA_VERSION = 'LayeredSurfaceField@2' as const

/** Closed input alias: vocabulary and controls remain owned by KnifeLayeredMaterialSpec@1. */
export type LayeredSurfaceFieldSpec = KnifeLayeredMaterialSpec

export const LAYERED_SURFACE_FIELD_COLOR_ATTRIBUTE = 'color' as const

export const LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES = Object.freeze({
  base_lacquer_variation: 'surfaceLacquerVariation',
  edge_wear: 'surfaceEdgeWear',
  engraved_groove_mask: 'surfaceEngravedGroove',
  engraved_ridge_mask: 'surfaceEngravedRidge',
  scale_tile_field: 'surfaceScaleTile',
  roughness: 'surfaceRoughness',
  metalness: 'surfaceMetalness',
  emissive: 'surfaceEmissive',
})

export const LAYERED_SURFACE_FIELD_GLTF_CUSTOM_ATTRIBUTES = Object.freeze({
  base_lacquer_variation: '_SURFACELACQUERVARIATION',
  edge_wear: '_SURFACEEDGEWEAR',
  engraved_groove_mask: '_SURFACEENGRAVEDGROOVE',
  engraved_ridge_mask: '_SURFACEENGRAVEDRIDGE',
  scale_tile_field: '_SURFACESCALETILE',
  roughness: '_SURFACEROUGHNESS',
  metalness: '_SURFACEMETALNESS',
  emissive: '_SURFACEEMISSIVE',
})

/**
 * Three.js GLTFExporter maps `color` to COLOR_0 and prefixes other attributes
 * with `_` after upper-casing them.  Keep this mapping explicit so a receipt
 * can distinguish standard export from custom data that needs a consumer.
 */
export const LAYERED_SURFACE_FIELD_GLTF_ATTRIBUTES = Object.freeze({
  color: 'COLOR_0',
  ...LAYERED_SURFACE_FIELD_GLTF_CUSTOM_ATTRIBUTES,
})

/**
 * The preview-only item is the renderer's lit MeshPhysicalMaterial response.
 * `color` is both visible in the preview and a standard glTF COLOR_0 channel;
 * scalar/mask fields are exported custom vertex attributes but are not
 * interpreted by a stock glTF consumer without a separate bounded reader.
 */
export const LAYERED_SURFACE_FIELD_CHANNEL_CONTRACT = Object.freeze({
  preview_only: Object.freeze(['built-in-MeshPhysicalMaterial-lighting-response@1']),
  color_0: 'color -> COLOR_0 (composed visible surface signal)',
  custom_attributes: Object.freeze({
    base_lacquer_variation: '_SURFACELACQUERVARIATION',
    edge_wear: '_SURFACEEDGEWEAR',
    engraved_groove_mask: '_SURFACEENGRAVEDGROOVE',
    engraved_ridge_mask: '_SURFACEENGRAVEDRIDGE',
    scale_tile_field: '_SURFACESCALETILE',
    roughness: '_SURFACEROUGHNESS',
    metalness: '_SURFACEMETALNESS',
    emissive: '_SURFACEEMISSIVE',
  }),
  standard_material_properties: Object.freeze({
    roughness: 'material.roughness -> pbrMetallicRoughness.roughnessFactor',
    metalness: 'material.metalness -> pbrMetallicRoughness.metallicFactor',
    emissive: 'material.emissive/emissiveIntensity -> emissiveFactor/emissiveStrength',
  }),
})

export class LayeredSurfaceFieldError extends Error {
  constructor(message: string) {
    super(`INVALID_LAYERED_SURFACE_FIELD: ${message}`)
    this.name = 'LayeredSurfaceFieldError'
  }
}

export interface LayeredSurfaceFieldReceipt {
  readonly schema_version: typeof LAYERED_SURFACE_FIELD_SCHEMA_VERSION
  readonly material_zone_id: string
  readonly material_type: 'MeshPhysicalMaterial'
  readonly vertex_count: number
  readonly attribute_names: readonly string[]
  readonly visible_preview_attributes: readonly ['color']
  readonly preview_only_channels: readonly ['built-in-MeshPhysicalMaterial-lighting-response@1']
  readonly exported_color_attribute: 'COLOR_0'
  readonly exported_custom_attributes: typeof LAYERED_SURFACE_FIELD_GLTF_CUSTOM_ATTRIBUTES
  readonly standard_material_properties: typeof LAYERED_SURFACE_FIELD_CHANNEL_CONTRACT.standard_material_properties
  readonly texture_source: 'none'
  readonly shader_source: 'built-in-MeshPhysicalMaterial@1'
  readonly arbitrary_shader: false
  readonly arbitrary_code: false
  readonly renderer_invoked: false
  readonly quality_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

const MAX_FIELD_VERTICES = 2_000_000
const TWO_PI = Math.PI * 2

/** Validate the closed v1 material input used to derive a v2 field. */
export function validateLayeredSurfaceFieldSpec(value: unknown): asserts value is LayeredSurfaceFieldSpec {
  try {
    validateKnifeLayeredMaterialSpec(value)
  } catch (error) {
    throw new LayeredSurfaceFieldError(error instanceof Error ? error.message : String(error))
  }
}

/**
 * Derive bounded, deterministic surface channels and bind them to a
 * MeshPhysicalMaterial/BufferGeometry pair.  No texture, shader, callback,
 * URL, or executable caller input is accepted by this API.
 */
export function bindLayeredSurfaceField(
  geometry: THREE.BufferGeometry,
  material: THREE.MeshPhysicalMaterial,
  spec: LayeredSurfaceFieldSpec,
): LayeredSurfaceFieldReceipt {
  validateLayeredSurfaceFieldSpec(spec)
  validateGeometry(geometry)
  if (!(material instanceof THREE.MeshPhysicalMaterial)) {
    throw new LayeredSurfaceFieldError('material must be a Three.js MeshPhysicalMaterial')
  }
  if (![material.color.r, material.color.g, material.color.b, material.roughness, material.metalness, material.emissiveIntensity].every(Number.isFinite)) {
    throw new LayeredSurfaceFieldError('material PBR values must be finite')
  }

  const positions = geometry.getAttribute('position')
  const normals = geometry.getAttribute('normal')
  const uvs = geometry.getAttribute('uv')
  const sectionUs = geometry.getAttribute('sectionU')
  const vertexCount = positions.count
  const baseLacquerVariation = new Float32Array(vertexCount)
  const edgeWear = new Float32Array(vertexCount)
  const engravedGroove = new Float32Array(vertexCount)
  const engravedRidge = new Float32Array(vertexCount)
  const scaleTile = new Float32Array(vertexCount * 2)
  const roughness = new Float32Array(vertexCount)
  const metalness = new Float32Array(vertexCount)
  const emissive = new Float32Array(vertexCount)
  const colors = new Float32Array(vertexCount * 3)
  const [repeatU, repeatV] = spec.controls.scale_repeat
  const palette = knifeMaterialPalette(spec.vocabulary)
  const wearColor = new THREE.Color(palette.wear_color)
  const engravingColor = new THREE.Color(palette.engraving_color)
  const ridgeColor = wearColor.clone().lerp(new THREE.Color(1, 1, 1), 0.34)
  const baseColor = material.color.clone()
  const baseEmissive = clamp01(material.emissiveIntensity)

  for (let index = 0; index < vertexCount; index += 1) {
    const u = coordinateAt(sectionUs, uvs, index, 0, index / Math.max(vertexCount - 1, 1))
    const v = coordinateAt(uvs, undefined, index, 1, 0.5)
    const tileU = u * repeatU
    const tileV = v * repeatV
    const phase = positiveFraction(tileU + tileV)
    const lacquerWave = 0.5 + 0.5 * Math.sin(TWO_PI * phase + 0.37)
    const lacquer = clamp01(0.5 + (lacquerWave - 0.5) * 2 * spec.controls.curvature)
    const edgeProximity = 1 - clamp01(Math.min(v, 1 - v) * 2)
    const wear = clamp01(edgeProximity * spec.controls.edge_wear)
    const groove = clamp01(smoothBand(lacquerWave, 0.25, 0.22) * spec.controls.engraving_mask)
    const ridge = clamp01(smoothBand(lacquerWave, 0.75, 0.22) * spec.controls.engraving_mask)
    const vertexRoughness = clamp01(material.roughness + wear * 0.16 + groove * 0.08 - ridge * 0.03)
    const vertexMetalness = clamp01(material.metalness * (0.9 + lacquer * 0.1) + ridge * 0.05)
    const vertexEmissive = clamp01(baseEmissive * (0.35 + ridge * 0.65))

    baseLacquerVariation[index] = lacquer
    edgeWear[index] = wear
    engravedGroove[index] = groove
    engravedRidge[index] = ridge
    scaleTile[index * 2] = tileU
    scaleTile[index * 2 + 1] = tileV
    roughness[index] = vertexRoughness
    metalness[index] = vertexMetalness
    emissive[index] = vertexEmissive

    // MeshPhysicalMaterial consumes only `color` here.  The scalar/mask
    // channels are also folded into this bounded color proxy so every layer
    // has a visible preview signal without a custom shader.
    const visibleColor = baseColor.clone()
    visibleColor.multiplyScalar(0.76 + lacquer * 0.18 + (1 - vertexRoughness) * 0.04)
    visibleColor.lerp(wearColor, clamp01(wear * 0.72))
    visibleColor.lerp(engravingColor, clamp01(groove * 0.68))
    visibleColor.lerp(ridgeColor, clamp01(ridge * 0.3))
    visibleColor.lerp(new THREE.Color(0.86, 0.9, 1), clamp01(vertexMetalness * 0.04))
    visibleColor.multiplyScalar(1 + vertexEmissive * 0.04)
    colors[index * 3] = clamp01(visibleColor.r)
    colors[index * 3 + 1] = clamp01(visibleColor.g)
    colors[index * 3 + 2] = clamp01(visibleColor.b)
  }

  geometry.setAttribute(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES.base_lacquer_variation, new THREE.Float32BufferAttribute(baseLacquerVariation, 1))
  geometry.setAttribute(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES.edge_wear, new THREE.Float32BufferAttribute(edgeWear, 1))
  geometry.setAttribute(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES.engraved_groove_mask, new THREE.Float32BufferAttribute(engravedGroove, 1))
  geometry.setAttribute(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES.engraved_ridge_mask, new THREE.Float32BufferAttribute(engravedRidge, 1))
  geometry.setAttribute(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES.scale_tile_field, new THREE.Float32BufferAttribute(scaleTile, 2))
  geometry.setAttribute(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES.roughness, new THREE.Float32BufferAttribute(roughness, 1))
  geometry.setAttribute(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES.metalness, new THREE.Float32BufferAttribute(metalness, 1))
  geometry.setAttribute(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES.emissive, new THREE.Float32BufferAttribute(emissive, 1))
  geometry.setAttribute(LAYERED_SURFACE_FIELD_COLOR_ATTRIBUTE, new THREE.Float32BufferAttribute(colors, 3))
  // `color` already contains the bounded final base colour. Three.js and glTF
  // multiply COLOR_0 by the material baseColorFactor, so retaining the source
  // base colour here would square/tint it and make lacquer and gold too dark.
  // Normalize only the colour factor; metalness, roughness, clearcoat, and
  // emissive remain material-owned standard PBR properties.
  material.color.setRGB(1, 1, 1)
  material.vertexColors = true
  material.needsUpdate = true

  const attributeNames = Object.freeze([
    LAYERED_SURFACE_FIELD_COLOR_ATTRIBUTE,
    ...Object.values(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES),
  ])
  const fingerprint = fingerprintField(geometry, material, spec, attributeNames)
  const metadata = {
    schema_version: LAYERED_SURFACE_FIELD_SCHEMA_VERSION,
    material_zone_id: spec.material_zone_id,
    material_vocabulary: spec.vocabulary,
    visible_preview_attributes: ['color'],
    preview_only_channels: [...LAYERED_SURFACE_FIELD_CHANNEL_CONTRACT.preview_only],
    exported_color_attribute: LAYERED_SURFACE_FIELD_GLTF_ATTRIBUTES.color,
    exported_custom_attributes: { ...LAYERED_SURFACE_FIELD_CHANNEL_CONTRACT.custom_attributes },
    standard_material_properties: { ...LAYERED_SURFACE_FIELD_CHANNEL_CONTRACT.standard_material_properties },
    attribute_names: [...attributeNames],
    texture_source: 'none',
    shader_source: 'built-in-MeshPhysicalMaterial@1',
    arbitrary_shader: false,
    arbitrary_code: false,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
    deterministic_fingerprint: fingerprint,
  } as const
  geometry.userData = { ...geometry.userData, layered_surface_field: metadata }
  material.userData = { ...material.userData, layered_surface_field: metadata }

  return Object.freeze({
    schema_version: LAYERED_SURFACE_FIELD_SCHEMA_VERSION,
    material_zone_id: spec.material_zone_id,
    material_type: 'MeshPhysicalMaterial',
    vertex_count: vertexCount,
    attribute_names: attributeNames,
    visible_preview_attributes: ['color'] as const,
    preview_only_channels: ['built-in-MeshPhysicalMaterial-lighting-response@1'] as const,
    exported_color_attribute: 'COLOR_0',
    exported_custom_attributes: LAYERED_SURFACE_FIELD_GLTF_CUSTOM_ATTRIBUTES,
    standard_material_properties: LAYERED_SURFACE_FIELD_CHANNEL_CONTRACT.standard_material_properties,
    texture_source: 'none',
    shader_source: 'built-in-MeshPhysicalMaterial@1',
    arbitrary_shader: false,
    arbitrary_code: false,
    renderer_invoked: false,
    quality_status: 'NOT_RUN',
    deterministic_fingerprint: fingerprint,
  })
}

/** Explicit alias for callers that describe this operation as applying a field. */
export const applyLayeredSurfaceField = bindLayeredSurfaceField

function validateGeometry(geometry: THREE.BufferGeometry): void {
  if (!(geometry instanceof THREE.BufferGeometry)) throw new LayeredSurfaceFieldError('geometry must be a Three.js BufferGeometry')
  const positions = geometry.getAttribute('position')
  if (!positions || positions.itemSize !== 3 || positions.count < 3 || positions.count > MAX_FIELD_VERTICES) {
    throw new LayeredSurfaceFieldError(`geometry position attribute must contain 3 to ${MAX_FIELD_VERTICES} vertices`)
  }
  if (![...positions.array].every(Number.isFinite)) throw new LayeredSurfaceFieldError('geometry position values must be finite')
  for (const name of ['normal', 'uv', 'sectionU'] as const) {
    const attribute = geometry.getAttribute(name)
    if (!attribute) continue
    const expectedItemSize = name === 'normal' ? 3 : name === 'uv' ? 2 : 1
    if (attribute.itemSize < expectedItemSize || attribute.count !== positions.count) {
      throw new LayeredSurfaceFieldError(`${name} attribute must match the position vertex count`)
    }
    if ([...attribute.array].some((value) => !Number.isFinite(value))) {
      throw new LayeredSurfaceFieldError(`${name} attribute values must be finite`)
    }
  }
}

function coordinateAt(
  primary: THREE.BufferAttribute | THREE.InterleavedBufferAttribute | undefined,
  secondary: THREE.BufferAttribute | THREE.InterleavedBufferAttribute | undefined,
  index: number,
  component: number,
  fallback: number,
): number {
  const attribute = primary ?? secondary
  if (!attribute || attribute.itemSize <= component) return clamp01(fallback)
  return clamp01(attribute.getComponent(index, component))
}

function smoothBand(value: number, center: number, halfWidth: number): number {
  const normalizedDistance = Math.abs(value - center) / halfWidth
  return smoothstep01(1 - normalizedDistance)
}

function smoothstep01(value: number): number {
  const x = clamp01(value)
  return x * x * (3 - 2 * x)
}

function positiveFraction(value: number): number {
  const remainder = value % 1
  return remainder < 0 ? remainder + 1 : remainder
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function fingerprintField(
  geometry: THREE.BufferGeometry,
  material: THREE.MeshPhysicalMaterial,
  spec: LayeredSurfaceFieldSpec,
  attributeNames: readonly string[],
): string {
  const values = [
    LAYERED_SURFACE_FIELD_SCHEMA_VERSION,
    spec.material_zone_id,
    spec.vocabulary,
    canonicalNumber(spec.controls.curvature),
    canonicalNumber(spec.controls.edge_wear),
    canonicalNumber(spec.controls.engraving_mask),
    ...spec.controls.scale_repeat.map(canonicalNumber),
    material.type,
    canonicalNumber(material.color.r),
    canonicalNumber(material.color.g),
    canonicalNumber(material.color.b),
    canonicalNumber(material.roughness),
    canonicalNumber(material.metalness),
    canonicalNumber(material.emissive.r),
    canonicalNumber(material.emissive.g),
    canonicalNumber(material.emissive.b),
    canonicalNumber(material.emissiveIntensity),
  ]
  const positions = geometry.getAttribute('position')
  values.push('position', positions.itemSize.toString(), positions.count.toString())
  for (const value of positions.array) values.push(canonicalNumber(value))
  for (const name of attributeNames) {
    const attribute = geometry.getAttribute(name)
    if (!attribute) throw new LayeredSurfaceFieldError(`bound attribute ${name} is missing`)
    values.push(name, attribute.itemSize.toString(), attribute.count.toString())
    for (const value of attribute.array) values.push(canonicalNumber(value))
  }
  return sha256Hex(values.join('|'))
}

function canonicalNumber(value: number): string {
  if (!Number.isFinite(value)) throw new LayeredSurfaceFieldError('field fingerprint received a non-finite number')
  return Object.is(value, -0) ? '0' : value.toPrecision(12)
}
