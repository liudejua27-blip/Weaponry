#!/usr/bin/env node

// Fixed, stdin/stdout-only Worker for the Weaponry Three.js knife route.
// Callers can submit only a closed KnifeSceneProgram and one allowlisted
// operation. No script, module, URL or filesystem path crosses this boundary.

import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { GLTFExporter } from 'three/examples/jsm/exporters/GLTFExporter.js'

import { compileKnifeSceneProgram, makeKnifeSceneActionReady } from '../src/index.ts'
import { runBrowserPreview } from './browser-preview-worker.mjs'

const REQUEST_SCHEMA = 'WeaponryThreeJsFixedWorkerRequest@1'
const RESULT_SCHEMA = 'WeaponryThreeJsFixedWorkerResult@1'
const WORKER_ID = 'weaponry-threejs-fixed-knife-worker@1'
const PREVIEW_RUNTIME_ID = 'weaponry-threejs-fixed-preview-worker@1'
const EXPECTED_THREE_VERSION = '0.185.1'
const EXPECTED_VITE_VERSION = '7.3.6'
const OPERATIONS = new Set(['build', 'preview', 'export'])
const REQUEST_FIELDS = new Set([
  'schema_version',
  'operation',
  'program_sha256',
  'program_object_sha256',
  'program',
  'max_response_bytes',
])
const MAX_INPUT_BYTES = 1024 * 1024
const MAX_RESPONSE_BYTES = 64 * 1024 * 1024
const FIXED_PREVIEW = Object.freeze({
  schema_version: 'WeaponryThreeJsPreviewManifest@1',
  view_ids: ['FRONT', 'BACK', 'TOP', 'BOTTOM', 'LEFT', 'RIGHT', 'REAR_THREE_QUARTER', 'FPS_HOLD'],
  frame_width: 512,
  frame_height: 512,
  margin: 0.08,
  capture: 'settled',
  aovs: 'required',
  framing: 'full-asset-baseline',
})

class NodeFileReader {
  result = null
  error = null
  onloadend = null
  onerror = null

  readAsArrayBuffer(blob) {
    blob.arrayBuffer().then(
      (value) => {
        this.result = value
        this.onloadend?.({ target: this })
      },
      (error) => {
        this.error = error
        this.onerror?.(error)
      },
    )
  }

  readAsDataURL(blob) {
    blob.arrayBuffer().then(
      (value) => {
        const mime = blob.type || 'application/octet-stream'
        this.result = `data:${mime};base64,${Buffer.from(value).toString('base64')}`
        this.onloadend?.({ target: this })
      },
      (error) => {
        this.error = error
        this.onerror?.(error)
      },
    )
  }
}

globalThis.FileReader ??= NodeFileReader

function canonicalJson(value) {
  if (value === null) return 'null'
  if (typeof value === 'boolean' || typeof value === 'number' || typeof value === 'string') {
    return JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

function packageFileHash(relativePath) {
  return sha256(readFileSync(new URL(`../${relativePath}`, import.meta.url)))
}

function previewDependencyLock() {
  const threePackage = readPackageManifest('three')
  const vitePackage = readPackageManifest('vite')
  if (threePackage.version !== EXPECTED_THREE_VERSION || vitePackage.version !== EXPECTED_VITE_VERSION) {
    throw new Error(`preview dependency drifted: three=${threePackage.version}, vite=${vitePackage.version}`)
  }
  return Object.freeze({
    three: EXPECTED_THREE_VERSION,
    vite: EXPECTED_VITE_VERSION,
    browser_capture: 'WebGLRenderer-preserveDrawingBuffer@1',
  })
}

function readPackageManifest(name) {
  const candidates = [
    new URL(`../node_modules/${name}/package.json`, import.meta.url),
    new URL(`../../node_modules/${name}/package.json`, import.meta.url),
    new URL(`../../../node_modules/${name}/package.json`, import.meta.url),
  ]
  for (const candidate of candidates) {
    try {
      return JSON.parse(readFileSync(candidate, 'utf8'))
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error
    }
  }
  throw new Error(`preview dependency manifest is unavailable: ${name}`)
}

function computePreviewDependencyLockSha256(lock) {
  return sha256(canonicalJson(lock))
}

function computePreviewRuntimeSha256(browserRuntime) {
  const source = {
    worker_id: WORKER_ID,
    runtime_id: PREVIEW_RUNTIME_ID,
    three_revision: '0.185.1',
    source_files: {
      fixed_worker: packageFileHash('scripts/fixed-worker.mjs'),
      browser_launcher: packageFileHash('scripts/browser-preview-worker.mjs'),
      browser_entry: packageFileHash('preview/worker-main.ts'),
      capture: packageFileHash('src/knife-preview-worker-capture.ts'),
    },
    browser: browserRuntime,
  }
  return sha256(canonicalJson(source))
}

function computePreviewWorkerCohortSha256(runtimeSha256, dependencySha256) {
  return sha256(canonicalJson({
    worker_id: WORKER_ID,
    runtime_id: PREVIEW_RUNTIME_ID,
    runtime_sha256: runtimeSha256,
    dependency_lock_sha256: dependencySha256,
    three_revision: '0.185.1',
  }))
}

function exactObject(value, fields, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${label} must be an object`)
  const keys = Object.keys(value)
  if (keys.length !== fields.size || keys.some((key) => !fields.has(key))) throw new Error(`${label} fields are not closed`)
  return value
}

async function readRequest() {
  const chunks = []
  let size = 0
  for await (const chunk of process.stdin) {
    size += chunk.length
    if (size > MAX_INPUT_BYTES) throw new Error('request exceeds fixed Worker input budget')
    chunks.push(chunk)
  }
  if (size === 0) throw new Error('request is empty')
  return JSON.parse(Buffer.concat(chunks).toString('utf8'))
}

async function exportGlb(group) {
  const exporter = new GLTFExporter()
  const result = await exporter.parseAsync(group, { binary: true, onlyVisible: true, trs: true })
  if (!(result instanceof ArrayBuffer)) throw new Error('fixed exporter did not return GLB bytes')
  return Buffer.from(result)
}

function sealResult(value) {
  value.canonical_sha256 = ''
  value.canonical_sha256 = sha256(canonicalJson(value))
  const bytes = Buffer.from(canonicalJson(value))
  if (bytes.length > MAX_RESPONSE_BYTES) throw new Error('result exceeds fixed Worker response budget')
  return bytes
}

try {
  const request = exactObject(await readRequest(), REQUEST_FIELDS, 'request')
  if (request.schema_version !== REQUEST_SCHEMA || !OPERATIONS.has(request.operation)) {
    throw new Error('request schema_version or operation is not allowlisted')
  }
  if (request.max_response_bytes !== MAX_RESPONSE_BYTES) throw new Error('max_response_bytes differs from fixed policy')
  const programFields = new Set([
    'schema_version', 'asset_id', 'family', 'design_basis', 'coordinate_convention', 'blade_surface',
    'parts', 'material_zones', 'presentation', 'budgets', 'unknowns', 'canonical_sha256',
  ])
  if (request.program && Object.hasOwn(request.program, 'assembly')) programFields.add('assembly')
  const program = exactObject(request.program, programFields, 'program')
  const semanticPreimage = { ...program, canonical_sha256: '' }
  const programSha256 = sha256(canonicalJson(semanticPreimage))
  const programObjectBytes = Buffer.from(canonicalJson(program))
  const programObjectSha256 = sha256(programObjectBytes)
  if (program.canonical_sha256 !== programSha256
    || request.program_sha256 !== programSha256
    || request.program_object_sha256 !== programObjectSha256) {
    throw new Error('program semantic/object binding differs from fixed Worker input')
  }

  const compiled = compileKnifeSceneProgram(program)
  if (request.operation !== 'preview') makeKnifeSceneActionReady(compiled, program)
  const browserPreview = request.operation === 'preview' ? await runBrowserPreview(program) : null
  const glb = request.operation === 'preview' ? null : await exportGlb(compiled.group)
  if (browserPreview !== null && browserPreview.three_revision !== EXPECTED_THREE_VERSION) {
    throw new Error(`browser Three.js revision drifted: ${browserPreview.three_revision}`)
  }
  const dependencyLock = browserPreview === null ? null : previewDependencyLock()
  const previewRuntimeSha256 = browserPreview === null ? null : computePreviewRuntimeSha256(browserPreview.runtime)
  const previewDependencySha256 = browserPreview === null ? null : computePreviewDependencyLockSha256(dependencyLock)
  const previewWorkerCohortSha256 = browserPreview === null ? null : computePreviewWorkerCohortSha256(previewRuntimeSha256, previewDependencySha256)
  const previewViews = browserPreview === null ? null : browserPreview.capture.views.map((view) => ({
    view_id: view.view_id,
    camera_sha256: view.camera_sha256,
    worker_cohort_sha256: previewWorkerCohortSha256,
    width: view.width,
    height: view.height,
    passes: view.passes,
  }))
  const result = {
    schema_version: RESULT_SCHEMA,
    operation: request.operation,
    status: request.operation === 'preview' ? 'preview-ready' : request.operation === 'build' ? 'built' : 'exported',
    worker_id: WORKER_ID,
    program_sha256: programSha256,
    program_object_sha256: programObjectSha256,
    deterministic_fingerprint: compiled.deterministic_fingerprint,
    triangle_count: compiled.triangle_count,
    part_ids: compiled.parts.map((part) => part.part_id),
    preview_manifest: browserPreview?.capture?.manifest ?? FIXED_PREVIEW,
    glb_sha256: glb === null ? null : sha256(glb),
    glb_bytes: glb === null ? 0 : glb.length,
    glb_base64: glb === null ? null : glb.toString('base64'),
    renderer_invoked: browserPreview !== null,
    visual_status: 'NOT_RUN',
    human_status: 'NOT_RUN',
    commercial_status: 'NOT_RUN',
    ...(browserPreview === null ? {} : {
      preview_runtime_id: PREVIEW_RUNTIME_ID,
      preview_runtime_sha256: previewRuntimeSha256,
      preview_dependency_lock_sha256: previewDependencySha256,
      preview_worker_cohort_sha256: previewWorkerCohortSha256,
      preview_view_count: 8,
      preview_aov_count: 48,
      preview_views: previewViews,
      // PNG bytes are transport data, not a quality verdict. Runtime must
      // split them into per-pass CAS objects before persisting the receipt.
      preview_payloads: browserPreview.capture.payloads,
    }),
    canonical_sha256: '',
  }
  process.stdout.write(sealResult(result))
} catch (error) {
  process.stderr.write(`WEAPONRY_THREEJS_FIXED_WORKER_INVALID: ${error instanceof Error ? error.message : String(error)}\n`)
  process.exitCode = 2
}
