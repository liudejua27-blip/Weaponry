import * as THREE from 'three'

import { GLTFExporter } from 'three/examples/jsm/exporters/GLTFExporter.js'

import {
  LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES,
  LAYERED_SURFACE_FIELD_CHANNEL_CONTRACT,
  LAYERED_SURFACE_FIELD_COLOR_ATTRIBUTE,
  LAYERED_SURFACE_FIELD_GLTF_CUSTOM_ATTRIBUTES,
  LAYERED_SURFACE_FIELD_GLTF_ATTRIBUTES,
  LAYERED_SURFACE_FIELD_SCHEMA_VERSION,
  bindLayeredSurfaceField,
  type LayeredSurfaceFieldSpec,
} from '../src/index.ts'

class NodeFileReader {
  result: ArrayBuffer | string | null = null
  error: unknown = null
  onloadend: (() => void) | null = null
  onerror: ((error: unknown) => void) | null = null

  readAsArrayBuffer(blob: Blob): void {
    blob.arrayBuffer().then(
      (value) => {
        this.result = value
        this.onloadend?.()
      },
      (error) => {
        this.error = error
        this.onerror?.(error)
      },
    )
  }

  readAsDataURL(blob: Blob): void {
    blob.arrayBuffer().then(
      (value) => {
        this.result = `data:${blob.type || 'application/octet-stream'};base64,${base64(value)}`
        this.onloadend?.()
      },
      (error) => {
        this.error = error
        this.onerror?.(error)
      },
    )
  }
}

if (typeof globalThis.FileReader === 'undefined') {
  Object.defineProperty(globalThis, 'FileReader', { configurable: true, value: NodeFileReader })
}

const spec: LayeredSurfaceFieldSpec = {
  schema_version: 'KnifeLayeredMaterialSpec@1',
  material_zone_id: 'surface-field-smoke',
  vocabulary: 'red-lacquer-metal',
  controls: {
    curvature: 0.72,
    edge_wear: 0.65,
    engraving_mask: 0.8,
    scale_repeat: [3, 2],
  },
}

const neutralSpec: LayeredSurfaceFieldSpec = {
  ...spec,
  controls: {
    curvature: 0,
    edge_wear: 0,
    engraving_mask: 0,
    scale_repeat: [1, 1],
  },
}

const geometry = new THREE.BoxGeometry(1, 1, 1, 2, 2, 2)
const material = new THREE.MeshPhysicalMaterial({
  color: 0x8f1824,
  metalness: 0.72,
  roughness: 0.28,
  emissive: 0x180004,
  emissiveIntensity: 0.3,
})
const receipt = bindLayeredSurfaceField(geometry, material, spec)
const repeatGeometry = new THREE.BoxGeometry(1, 1, 1, 2, 2, 2)
const repeatMaterial = new THREE.MeshPhysicalMaterial({
  color: 0x8f1824,
  metalness: 0.72,
  roughness: 0.28,
  emissive: 0x180004,
  emissiveIntensity: 0.3,
})
const repeatReceipt = bindLayeredSurfaceField(repeatGeometry, repeatMaterial, spec)
if (receipt.deterministic_fingerprint !== repeatReceipt.deterministic_fingerprint) {
  throw new Error('LayeredSurfaceField@2 binding is not deterministic')
}

const neutralGeometry = new THREE.BoxGeometry(1, 1, 1, 2, 2, 2)
const neutralMaterial = new THREE.MeshPhysicalMaterial({
  color: 0x8f1824,
  metalness: 0.72,
  roughness: 0.28,
  emissive: 0x180004,
  emissiveIntensity: 0.3,
})
bindLayeredSurfaceField(neutralGeometry, neutralMaterial, neutralSpec)

const position = geometry.getAttribute('position')
const requiredAttributes = Object.values(LAYERED_SURFACE_FIELD_ATTRIBUTE_NAMES)
for (const name of requiredAttributes) {
  const attribute = geometry.getAttribute(name)
  if (!attribute || attribute.count !== position.count || [...attribute.array].some((value) => !Number.isFinite(value))) {
    throw new Error(`surface field attribute ${name} is missing or non-finite`)
  }
}
const colors = geometry.getAttribute(LAYERED_SURFACE_FIELD_COLOR_ATTRIBUTE)
if (!colors || colors.itemSize !== 3 || colors.count !== position.count
  || [...colors.array].some((value) => !Number.isFinite(value) || value < 0 || value > 1)) {
  throw new Error('surface field visible color channel is missing or out of bounds')
}
if (!differs(colors.array, neutralGeometry.getAttribute(LAYERED_SURFACE_FIELD_COLOR_ATTRIBUTE).array)) {
  throw new Error('surface field controls did not reach the visible color channel')
}
if (material.vertexColors !== true || !(material instanceof THREE.MeshPhysicalMaterial)) {
  throw new Error('surface field did not bind a MeshPhysicalMaterial vertex-color preview')
}
if (material.userData.layered_surface_field?.arbitrary_shader !== false
  || material.userData.layered_surface_field?.arbitrary_code !== false
  || material.userData.layered_surface_field?.texture_source !== 'none') {
  throw new Error('surface field crossed the closed preview boundary')
}

const scene = new THREE.Scene()
scene.add(new THREE.Mesh(geometry, material))
const exported = await new GLTFExporter().parseAsync(scene, { binary: false })
if (exported instanceof ArrayBuffer) throw new Error('surface field smoke expected JSON glTF output')
const gltf = exported as { meshes?: Array<{ primitives?: Array<{ attributes?: Record<string, number> }> }>; materials?: Array<{ pbrMetallicRoughness?: { metallicFactor?: number; roughnessFactor?: number }; emissiveFactor?: number[] }> }
const attributes = gltf.meshes?.[0]?.primitives?.[0]?.attributes
if (!attributes) throw new Error('surface field was not represented in the exported glTF primitive')
if (attributes[LAYERED_SURFACE_FIELD_GLTF_ATTRIBUTES.color] === undefined) {
  throw new Error(`exported glTF attribute ${LAYERED_SURFACE_FIELD_GLTF_ATTRIBUTES.color} is missing`)
}
for (const gltfName of Object.values(LAYERED_SURFACE_FIELD_GLTF_CUSTOM_ATTRIBUTES)) {
  if (attributes[gltfName] === undefined) throw new Error(`exported glTF attribute ${gltfName} is missing`)
}
const exportedMaterial = gltf.materials?.[0]
if (!exportedMaterial?.pbrMetallicRoughness
  || exportedMaterial.pbrMetallicRoughness.metallicFactor !== material.metalness
  || exportedMaterial.pbrMetallicRoughness.roughnessFactor !== material.roughness
  || !Array.isArray(exportedMaterial.emissiveFactor)) {
  throw new Error('standard MeshPhysicalMaterial PBR properties did not export truthfully')
}

console.log(JSON.stringify({
  schema_version: LAYERED_SURFACE_FIELD_SCHEMA_VERSION,
  material_zone_id: receipt.material_zone_id,
  material_type: receipt.material_type,
  vertex_count: receipt.vertex_count,
  visible_preview_attributes: receipt.visible_preview_attributes,
  preview_only_channels: receipt.preview_only_channels,
  exported_color_attribute: receipt.exported_color_attribute,
  exported_custom_attributes: Object.values(receipt.exported_custom_attributes),
  standard_material_properties: receipt.standard_material_properties,
  channel_contract: LAYERED_SURFACE_FIELD_CHANNEL_CONTRACT,
  gltf_attributes_verified: Object.keys(attributes).filter((name) => name === 'COLOR_0' || name.startsWith('_SURFACE')),
  deterministic_fingerprint: receipt.deterministic_fingerprint,
  renderer_invoked: receipt.renderer_invoked,
  quality_status: receipt.quality_status,
}))

geometry.dispose()
repeatGeometry.dispose()
neutralGeometry.dispose()
material.dispose()
repeatMaterial.dispose()
neutralMaterial.dispose()

function differs(left: ArrayLike<number>, right: ArrayLike<number>): boolean {
  if (left.length !== right.length) return false
  for (let index = 0; index < left.length; index += 1) {
    if (Math.abs(left[index] - right[index]) > 1e-6) return true
  }
  return false
}

function base64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer)
  if (typeof btoa === 'function') {
    let binary = ''
    for (let offset = 0; offset < bytes.length; offset += 0x8000) {
      binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000))
    }
    return btoa(binary)
  }
  const bufferConstructor = (globalThis as unknown as {
    Buffer?: { from(value: Uint8Array): { toString(encoding: 'base64'): string } }
  }).Buffer
  if (!bufferConstructor) throw new Error('base64 encoder is unavailable')
  return bufferConstructor.from(bytes).toString('base64')
}
