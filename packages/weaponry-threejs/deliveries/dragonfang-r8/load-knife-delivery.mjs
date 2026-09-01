import * as THREE from 'three'
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js'

const RUNTIME_SCHEMA = 'WeaponryThreeJsKnifeActionRuntime@1'

export async function loadKnifeDelivery(options = {}) {
  const baseUrl = options.baseUrl ?? '.'
  const manifestName = options.manifestName ?? 'delivery-manifest.json'
  const verify = options.verify !== false
  const manifestUrl = new URL(manifestName, ensureDirectoryUrl(baseUrl))
  const manifest = await fetchJson(manifestUrl)
  if (verify) {
    const supplied = manifest.canonical_sha256
    const preimage = { ...manifest, canonical_sha256: '' }
    const computed = await sha256(new TextEncoder().encode(canonicalJson(preimage)))
    if (supplied !== computed) throw new Error(`KNIFE_DELIVERY_MANIFEST_HASH_MISMATCH: ${computed}`)
  }
  assertLocalPath(manifest.delivery_glb.path)
  const glbUrl = new URL(manifest.delivery_glb.path, manifestUrl)
  const glbBytes = await fetchBytes(glbUrl)
  if (verify) {
    const digest = await sha256(glbBytes)
    if (digest !== manifest.delivery_glb.sha256) {
      throw new Error(`KNIFE_DELIVERY_GLB_HASH_MISMATCH: ${digest}`)
    }
  }

  const gltf = await new GLTFLoader().parseAsync(glbBytes, baseUrlFrom(glbUrl))
  let root = null
  gltf.scene.traverse((object) => {
    if (!root && object.userData?.sculptRuntime?.schema_version === RUNTIME_SCHEMA) root = object
  })
  if (!root) throw new Error('KNIFE_DELIVERY_RUNTIME_MISSING: action-ready root not found')
  return { gltf, root, manifest, controller: createKnifeDeliveryController(root) }
}

export function createKnifeDeliveryController(root) {
  const metadata = root.userData?.sculptRuntime
  if (metadata?.schema_version !== RUNTIME_SCHEMA) {
    throw new Error('KNIFE_DELIVERY_RUNTIME_MISSING: invalid action-ready root')
  }
  const partMeshes = new Map()
  const partPivots = new Map()
  const sockets = new Map()
  root.traverse((object) => {
    const partId = object.userData?.part_id
    if (object.isMesh && typeof partId === 'string') {
      if (partMeshes.has(partId)) throw new Error(`KNIFE_DELIVERY_DUPLICATE_PART: ${partId}`)
      partMeshes.set(partId, object)
    }
    if (typeof object.userData?.pivot_id === 'string' && typeof partId === 'string') {
      if (object.userData.pivot_id !== `pivot-${partId}`) throw new Error(`KNIFE_DELIVERY_PIVOT_ID_MISMATCH: ${partId}`)
      if (partPivots.has(partId)) throw new Error(`KNIFE_DELIVERY_DUPLICATE_PIVOT: ${partId}`)
      partPivots.set(partId, object)
    }
    const socketId = object.userData?.socket_id
    if (typeof socketId === 'string') {
      if (sockets.has(socketId)) throw new Error(`KNIFE_DELIVERY_DUPLICATE_SOCKET: ${socketId}`)
      sockets.set(socketId, object)
    }
  })
  for (const partId of metadata.part_ids) {
    if (!partMeshes.has(partId) || !partPivots.has(partId)) {
      throw new Error(`KNIFE_DELIVERY_PART_MISSING: ${partId}`)
    }
  }
  return {
    root,
    partMeshes,
    partPivots,
    sockets,
    setExploded(amount) {
      if (!Number.isFinite(amount) || amount < 0 || amount > 1) {
        throw new Error('KNIFE_DELIVERY_EXPLODE_RANGE: amount must be within 0..1')
      }
      for (const pivot of partPivots.values()) {
        const rest = vector(pivot.userData.rest_position, 'rest_position')
        const delta = vector(pivot.userData.explode_vector, 'explode_vector')
        pivot.position.set(
          rest[0] + delta[0] * amount,
          rest[1] + delta[1] * amount,
          rest[2] + delta[2] * amount,
        )
      }
      root.updateMatrixWorld(true)
    },
    setPartVisible(partId, visible) {
      const pivot = partPivots.get(partId)
      if (!pivot) throw new Error(`KNIFE_DELIVERY_UNKNOWN_PART: ${partId}`)
      pivot.visible = Boolean(visible)
    },
    resolvePart(object) {
      for (let cursor = object; cursor; cursor = cursor.parent) {
        const partId = cursor.userData?.part_id
        if (typeof partId === 'string' && partMeshes.has(partId)) return partId
        if (cursor === root) break
      }
      return null
    },
  }
}

function vector(value, field) {
  if (!Array.isArray(value) || value.length !== 3 || value.some((entry) => !Number.isFinite(entry))) {
    throw new Error(`KNIFE_DELIVERY_INVALID_VECTOR: ${field}`)
  }
  return value
}

async function fetchJson(url) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`KNIFE_DELIVERY_FETCH_FAILED: ${response.status} ${url}`)
  return response.json()
}

async function fetchBytes(url) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`KNIFE_DELIVERY_FETCH_FAILED: ${response.status} ${url}`)
  return response.arrayBuffer()
}

async function sha256(bytes) {
  const digest = await crypto.subtle.digest('SHA-256', bytes)
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

function ensureDirectoryUrl(value) {
  const base = value instanceof URL ? value : new URL(value, globalThis.location?.href ?? 'http://localhost/')
  return base.href.endsWith('/') ? base : new URL(`${base.href}/`)
}

function assertLocalPath(value) {
  if (typeof value !== 'string' || value.length === 0 || value.startsWith('/') || value.includes('..') || /^[a-z]+:/i.test(value)) {
    throw new Error(`KNIFE_DELIVERY_EXTERNAL_PATH_REJECTED: ${String(value)}`)
  }
}

function canonicalJson(value) {
  if (value === null) return 'null'
  if (typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value)
  if (typeof value === 'number') return Object.is(value, -0) ? '0' : JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
}

function baseUrlFrom(url) {
  return new URL('.', url).href
}

export { THREE }
