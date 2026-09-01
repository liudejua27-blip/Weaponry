#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { spawn } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const REPO = path.resolve(HERE, '../../..')
const PROGRAM_PATH = path.join(REPO, 'skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r8-blade.json')
const WORKER_ROOT = path.join(REPO, 'apps/desktop/src-tauri/target/weaponry-threejs-worker')
const WORKER_MANIFEST_PATH = path.join(WORKER_ROOT, 'weaponry-threejs-worker-manifest.json')
const OUTPUT_DIR = path.join(REPO, 'packages/weaponry-threejs/evidence/rendered-r8-fixed-views')
const EXPECTED_PROGRAM_SHA256 = '0c495db1c8ff2c0079cd5dafc3270eaafa9eae0ae1d0f41b6099d65db8ec51e1'
const VIEW_IDS = ['FRONT', 'BACK', 'TOP', 'BOTTOM', 'LEFT', 'RIGHT', 'REAR_THREE_QUARTER', 'FPS_HOLD']
const AOV_IDS = ['beauty', 'alpha-silhouette', 'semantic-id', 'depth', 'normal', 'roughness-material-id']
const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])
const MAX_RESPONSE_BYTES = 64 * 1024 * 1024

function canonicalJson(value) {
  if (value === null) return 'null'
  if (typeof value === 'boolean' || typeof value === 'number' || typeof value === 'string') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

function assert(condition, message) {
  if (!condition) throw new Error(`WPN_THREE_R8_PREVIEW_INVALID: ${message}`)
}

function sealedHash(value) {
  const draft = structuredClone(value)
  draft.canonical_sha256 = ''
  return sha256(canonicalJson(draft))
}

async function invokePackagedWorker(request) {
  const executable = path.join(WORKER_ROOT, 'runtime/node')
  const child = spawn(executable, ['--experimental-strip-types', 'worker/scripts/fixed-worker.mjs'], {
    cwd: WORKER_ROOT,
    env: { ...process.env, WEAPONRY_THREEJS_BROWSER_EXECUTABLE: path.join(WORKER_ROOT, 'browser/browser/chrome-headless-shell') },
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  const stdout = []
  const stderr = []
  let stdoutBytes = 0
  child.stdout.on('data', (chunk) => {
    stdoutBytes += chunk.length
    if (stdoutBytes > MAX_RESPONSE_BYTES) child.kill('SIGKILL')
    else stdout.push(chunk)
  })
  child.stderr.on('data', (chunk) => stderr.push(chunk))
  child.stdin.end(Buffer.from(canonicalJson(request)))
  const status = await new Promise((resolve, reject) => {
    child.once('error', reject)
    child.once('close', (code, signal) => resolve({ code, signal }))
  })
  assert(status.code === 0, `packaged Worker failed (${status.code ?? status.signal}): ${Buffer.concat(stderr).toString('utf8').trim()}`)
  return JSON.parse(Buffer.concat(stdout).toString('utf8'))
}

const programBytes = await readFile(PROGRAM_PATH)
const program = JSON.parse(programBytes)
const semanticPreimage = { ...program, canonical_sha256: '' }
const programSha256 = sha256(canonicalJson(semanticPreimage))
const programObjectSha256 = sha256(canonicalJson(program))
assert(programSha256 === EXPECTED_PROGRAM_SHA256 && program.canonical_sha256 === programSha256, 'r8 program semantic identity drifted')

const workerManifestBytes = await readFile(WORKER_MANIFEST_PATH)
const workerManifest = JSON.parse(workerManifestBytes)
assert(workerManifest.schema_version === 'WeaponryThreeJsPackagedWorkerManifest@1', 'packaged Worker manifest schema drifted')
assert(workerManifest.preview_runtime?.packaged === true, 'packaged Chromium is unavailable')

const result = await invokePackagedWorker({
  schema_version: 'WeaponryThreeJsFixedWorkerRequest@1',
  operation: 'preview',
  program_sha256: programSha256,
  program_object_sha256: programObjectSha256,
  program,
  max_response_bytes: MAX_RESPONSE_BYTES,
})

assert(result.schema_version === 'WeaponryThreeJsFixedWorkerResult@1' && result.operation === 'preview' && result.status === 'preview-ready', 'Worker result envelope drifted')
assert(result.canonical_sha256 === sealedHash(result), 'Worker result canonical hash drifted')
assert(result.program_sha256 === programSha256 && result.program_object_sha256 === programObjectSha256, 'Worker result program binding drifted')
assert(result.renderer_invoked === true && result.preview_view_count === 8 && result.preview_aov_count === 48, 'Worker did not produce the fixed 8-view/48-AOV set')
assert(result.visual_status === 'NOT_RUN' && result.human_status === 'NOT_RUN' && result.commercial_status === 'NOT_RUN', 'Worker result overclaimed quality')
assert(Array.isArray(result.preview_views) && result.preview_views.length === 8, 'preview view set is incomplete')
assert(Array.isArray(result.preview_payloads) && result.preview_payloads.length === 48, 'preview payload set is incomplete')

await mkdir(OUTPUT_DIR, { recursive: true })
const payloads = new Map(result.preview_payloads.map((item) => [`${item.view_id}/${item.aov_id}`, item]))
const files = []
const cameras = []
for (let viewIndex = 0; viewIndex < VIEW_IDS.length; viewIndex += 1) {
  const viewId = VIEW_IDS[viewIndex]
  const view = result.preview_views[viewIndex]
  assert(view.view_id === viewId && view.width === 512 && view.height === 512, `${viewId} view identity or dimensions drifted`)
  assert(view.worker_cohort_sha256 === result.preview_worker_cohort_sha256, `${viewId} Worker cohort drifted`)
  assert(Array.isArray(view.passes) && view.passes.length === AOV_IDS.length, `${viewId} AOV set is incomplete`)
  cameras.push({ view_id: viewId, camera_sha256: view.camera_sha256 })
  for (let passIndex = 0; passIndex < AOV_IDS.length; passIndex += 1) {
    const aovId = AOV_IDS[passIndex]
    const pass = view.passes[passIndex]
    const payload = payloads.get(`${viewId}/${aovId}`)
    assert(pass.aov_id === aovId && pass.mime === 'image/png', `${viewId}/${aovId} pass metadata drifted`)
    assert(payload?.mime_type === 'image/png', `${viewId}/${aovId} payload is unavailable`)
    const png = Buffer.from(payload.base64, 'base64')
    assert(png.subarray(0, 8).equals(PNG_SIGNATURE), `${viewId}/${aovId} is not a PNG`)
    assert(png.length === pass.bytes && sha256(png) === pass.sha256 && pass.object_sha256 === pass.sha256, `${viewId}/${aovId} payload hash drifted`)
    const filename = `${viewId.toLowerCase()}-${aovId}.png`
    await writeFile(path.join(OUTPUT_DIR, filename), png)
    files.push({ path: filename, view_id: viewId, aov_id: aovId, camera_sha256: view.camera_sha256, width: 512, height: 512, bytes: png.length, sha256: pass.sha256 })
  }
}

const receipt = {
  schema_version: 'WeaponryThreeJsFixedWorkerPreviewReceipt@1',
  operation: 'preview',
  status: 'preview-ready',
  worker_id: result.worker_id,
  program_sha256: programSha256,
  program_object_sha256: programObjectSha256,
  deterministic_fingerprint: result.deterministic_fingerprint,
  triangle_count: result.triangle_count,
  part_ids: result.part_ids,
  preview_manifest: result.preview_manifest,
  preview_runtime_id: result.preview_runtime_id,
  preview_runtime_sha256: result.preview_runtime_sha256,
  preview_dependency_lock_sha256: result.preview_dependency_lock_sha256,
  preview_worker_cohort_sha256: result.preview_worker_cohort_sha256,
  preview_view_count: result.preview_view_count,
  preview_aov_count: result.preview_aov_count,
  cameras,
  renderer_invoked: true,
  visual_status: 'NOT_RUN',
  human_status: 'NOT_RUN',
  commercial_status: 'NOT_RUN',
  source_worker_result_sha256: result.canonical_sha256,
  canonical_sha256: '',
}
receipt.canonical_sha256 = sealedHash(receipt)
const receiptBytes = Buffer.from(`${JSON.stringify(receipt, null, 2)}\n`)
await writeFile(path.join(OUTPUT_DIR, 'worker-result.receipt.json'), receiptBytes)

const manifest = {
  schema_version: 'WeaponryThreeJsFixedViewEvidenceManifest@2',
  task_id: 'WPN-THREE-R8-RENDER-COMPARE-013',
  asset_id: 'Dragonfang Kukri',
  program_sha256: programSha256,
  program_object_sha256: programObjectSha256,
  program_file_sha256: sha256(programBytes),
  packaged_worker_manifest_sha256: sha256(workerManifestBytes),
  packaged_worker_build_cohort_sha256: workerManifest.build_cohort_sha256,
  preview_runtime_sha256: result.preview_runtime_sha256,
  preview_dependency_lock_sha256: result.preview_dependency_lock_sha256,
  preview_worker_cohort_sha256: result.preview_worker_cohort_sha256,
  worker_result_sha256: result.canonical_sha256,
  worker_receipt_sha256: receipt.canonical_sha256,
  renderer_invoked: true,
  fixed_view_policy: 'weaponry-threejs-fixed-eight-view-rig@1',
  framing_policy: 'full-asset-baseline-no-candidate-refit@1',
  view_count: 8,
  aov_count: 48,
  frame_width: 512,
  frame_height: 512,
  cameras,
  files,
  geometry_modified: false,
  quality_status: 'MEASURED_NOT_APPROVED',
  visual_status: 'NOT_APPROVED',
  human_status: 'NOT_RUN',
  commercial_status: 'NOT_RUN',
  canonical_sha256: '',
}
manifest.canonical_sha256 = sealedHash(manifest)
const manifestBytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`)
await writeFile(path.join(OUTPUT_DIR, 'manifest.json'), manifestBytes)

process.stdout.write(`${JSON.stringify({
  status: 'PASS_RENDERED_NOT_APPROVED',
  program_sha256: programSha256,
  preview_worker_cohort_sha256: result.preview_worker_cohort_sha256,
  front_camera_sha256: cameras[0].camera_sha256,
  view_count: 8,
  aov_count: 48,
  manifest_sha256: sha256(manifestBytes),
  manifest_canonical_sha256: manifest.canonical_sha256,
}, null, 2)}\n`)
