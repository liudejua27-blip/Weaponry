#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'

import { GLTFExporter } from 'three/examples/jsm/exporters/GLTFExporter.js'

import {
  compileKnifeSceneProgram,
  createKnifeFixedEightViewRig,
  evaluateKnifeRig,
  measureProceduralDraftStructuralDelta,
} from '../src/index.ts'

const REPO_ROOT = decodeURIComponent(new URL('../../..', import.meta.url).pathname).replace(/\/$/, '')
const PARENT_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r7-runtime-canonical.json`
const MIGRATION_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-r7-canonical-migration.json`
const LEDGER_PARENT_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r6-intrinsic.json`
const REFERENCE_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-front-blade-reference.json`
const PROGRAM_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r8-blade.json`
const LEDGER_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r8-blade.json`
const GLB_PATH = `${REPO_ROOT}/packages/weaponry-threejs/artifacts/dragonfang-kukri-procedural-r8-blade.glb`
const RECEIPT_PATH = `${REPO_ROOT}/packages/weaponry-threejs/artifacts/dragonfang-kukri-procedural-r8-blade.receipt.json`

const EXPECTED_PARENT_SHA256 = '24b4f6e558f59c825daf1127cb5751a79204e44bd2b3b116420ef8112f8a113f'
class NodeFileReader {
  result = null
  error = null
  onloadend = null
  onerror = null

  readAsArrayBuffer(blob) {
    blob.arrayBuffer().then(
      (value) => { this.result = value; this.onloadend?.({ target: this }) },
      (error) => { this.error = error; this.onerror?.(error) },
    )
  }

  readAsDataURL(blob) {
    blob.arrayBuffer().then(
      (value) => {
        this.result = `data:${blob.type || 'application/octet-stream'};base64,${Buffer.from(value).toString('base64')}`
        this.onloadend?.({ target: this })
      },
      (error) => { this.error = error; this.onerror?.(error) },
    )
  }
}

globalThis.FileReader ??= NodeFileReader

const [parentBytes, migrationBytes, ledgerParentBytes, referenceBytes] = await Promise.all([
  readFile(PARENT_PATH),
  readFile(MIGRATION_PATH),
  readFile(LEDGER_PARENT_PATH),
  readFile(REFERENCE_PATH),
])
const parent = JSON.parse(parentBytes.toString('utf8'))
const migration = JSON.parse(migrationBytes.toString('utf8'))
const ledgerParent = JSON.parse(ledgerParentBytes.toString('utf8'))
const reference = JSON.parse(referenceBytes.toString('utf8'))

if (parent.canonical_sha256 !== EXPECTED_PARENT_SHA256
  || migration.successor.semantic_sha256 !== EXPECTED_PARENT_SHA256
  || migration.invariants.semantic_payload_equal_excluding_canonical_sha256 !== true) {
  throw new Error('r8 requires the closed Runtime-canonical r7 identity successor')
}
if (reference.schema_version !== 'KnifeContourReference@1') {
  throw new Error('r8 requires a closed KnifeContourReference@1')
}

const candidate = structuredClone(parent)
candidate.blade_surface.spine_curve.control_points = [
  [-1, 0.092627, 0],
  [-0.714286, 0.154, 0],
  [-0.428571, 0.197, 0],
  [-0.142857, 0.215, 0],
  [0.142857, 0.2, 0],
  [0.428571, 0.137, 0],
  [0.714286, 0.03, 0],
  [1, -0.062, 0],
]
candidate.blade_surface.cutting_edge_curve.control_points = [
  [-1, -0.091974, 0],
  [-0.714286, -0.05, 0.000046],
  [-0.428571, 0.027, 0.000369],
  [-0.142857, 0.015, 0.001244],
  [0.142857, -0.05, 0.002948],
  [0.428571, -0.107, 0.005759],
  [0.714286, -0.1, 0.009951],
  [1, -0.068, 0.015802],
]

const sectionCalibration = [
  [0.12, -0.04, 0.02],
  [0.17, -0.055, 0.022],
  [0.22, -0.068, 0.026],
  [0.245, -0.085, 0.029],
  [0.27, -0.105, 0.032],
  [0.285, -0.125, 0.034],
  [0.13, -0.052, 0.006],
  [0.004, 0, -0.006],
]
if (candidate.blade_surface.sections.length !== sectionCalibration.length) {
  throw new Error('r8 section calibration count drifted')
}
candidate.blade_surface.sections.forEach((section, index) => {
  const [halfWidth, edgeOffset, spineOffset] = sectionCalibration[index]
  section.half_width = halfWidth
  section.edge_offset = edgeOffset
  section.spine_offset = spineOffset
})
candidate.canonical_sha256 = ''
candidate.canonical_sha256 = sha256(canonicalJson(candidate))

assertFrozen(parent, candidate)
const changedPaths = changedLeafPaths(parent, candidate)
if (changedPaths.some((path) => !allowedPath(path))) {
  throw new Error(`r8 changed a frozen path: ${changedPaths.filter((path) => !allowedPath(path)).join(', ')}`)
}

const ledgerDraft = {
  schema_version: 'KnifeObjectiveLedger@1',
  ledger_id: 'dragonfang-blade-r8-ledger',
  revision: 7,
  parent_ledger_sha256: ledgerParent.canonical_sha256,
  program_sha256: parent.canonical_sha256,
  baseline_candidate_sha256: parent.canonical_sha256,
  stage: 'form',
  allowed_scope: ['blade-body', 'cutting-edge'],
  frozen_parts: parent.parts.map((part) => part.part_id).filter((partId) => !['blade-body', 'cutting-edge'].includes(partId)),
  hypothesis: 'A single bounded rail and section refinement can reduce the oversized leaf silhouette, converge the tip, and reduce belly depth without changing assembly, reliefs, materials, presentation, or budgets.',
  objective_metrics: [
    'silhouette-iou',
    'boundary-f1',
    'symmetric-chamfer',
    'p95-contour-distance',
    'tip-landmark-error',
    'belly-depth-error',
  ],
  regression_limits: [
    'thickness-continuity',
    'part-id-coverage',
    'material-id-coverage',
    'negative-space-error',
    'fps-occupancy',
  ],
  candidate_budget: 1,
  minimum_improvement: 0.001,
  plateau_limit: 2,
  evidence_sha256: [migration.canonical_sha256, reference.canonical_sha256],
  status: 'active',
  canonical_sha256: '',
}
const ledger = { ...ledgerDraft, canonical_sha256: sha256(canonicalJson(ledgerDraft)) }

const parentCompiled = compileKnifeSceneProgram(parent)
const candidateCompiled = compileKnifeSceneProgram(candidate)
const rig = createKnifeFixedEightViewRig()
const structuralDelta = measureProceduralDraftStructuralDelta(
  evaluateKnifeRig(parentCompiled, rig),
  evaluateKnifeRig(candidateCompiled, rig),
)
if (structuralDelta.status !== 'MEASURED_NONZERO') {
  throw new Error('r8 did not produce a fixed-eight-view structural delta')
}

candidateCompiled.group.updateMatrixWorld(true)
const exported = await new GLTFExporter().parseAsync(candidateCompiled.group, {
  binary: true,
  onlyVisible: true,
  trs: false,
})
if (!(exported instanceof ArrayBuffer)) throw new Error('GLTFExporter did not return binary GLB bytes')
const glbBytes = Buffer.from(exported)
const programBytes = Buffer.from(`${JSON.stringify(candidate, null, 2)}\n`)
const ledgerBytes = Buffer.from(`${JSON.stringify(ledger, null, 2)}\n`)

const receiptDraft = {
  schema_version: 'WeaponryThreeJsBladeSuccessorLineage@1',
  task_id: 'WPN-THREE-BLADE-R8-012',
  asset_id: candidate.asset_id,
  parent: {
    program_path: relative(PARENT_PATH),
    program_sha256: parent.canonical_sha256,
    program_bytes_sha256: sha256(parentBytes),
    canonical_migration_sha256: migration.canonical_sha256,
  },
  objective: {
    ledger_path: relative(LEDGER_PATH),
    ledger_sha256: ledger.canonical_sha256,
    reference_id: reference.reference_id,
    reference_sha256: reference.canonical_sha256,
    allowed_scope: ledger.allowed_scope,
    frozen_parts: ledger.frozen_parts,
    changed_parameter_paths: changedPaths,
  },
  successor: {
    program_path: relative(PROGRAM_PATH),
    program_sha256: candidate.canonical_sha256,
    program_bytes_sha256: sha256(programBytes),
    compiled_fingerprint: candidateCompiled.deterministic_fingerprint,
    glb_path: relative(GLB_PATH),
    glb_sha256: sha256(glbBytes),
    glb_bytes: glbBytes.length,
    triangle_count: candidateCompiled.triangle_count,
    part_ids: candidateCompiled.parts.map((part) => part.part_id),
  },
  frozen_scope_proof: {
    assembly_sha256: sha256(canonicalJson(parent.assembly)),
    parts_sha256: sha256(canonicalJson(parent.parts)),
    material_zones_sha256: sha256(canonicalJson(parent.material_zones)),
    presentation_sha256: sha256(canonicalJson(parent.presentation)),
    budgets_sha256: sha256(canonicalJson(parent.budgets)),
    unknowns_sha256: sha256(canonicalJson(parent.unknowns)),
  },
  structural_delta: structuralDelta,
  status: 'BLADE_SUCCESSOR_COMPILED_REVIEW_ONLY',
  renderer_invoked: false,
  visual_status: 'NOT_RUN',
  quality_status: 'NOT_RUN',
  human_status: 'NOT_RUN',
  commercial_status: 'NOT_RUN',
  canonical_sha256: '',
}
const receipt = { ...receiptDraft, canonical_sha256: sha256(canonicalJson(receiptDraft)) }

await writeFile(PROGRAM_PATH, programBytes)
await writeFile(LEDGER_PATH, ledgerBytes)
await writeFile(GLB_PATH, glbBytes)
await writeFile(RECEIPT_PATH, `${JSON.stringify(receipt, null, 2)}\n`, 'utf8')

console.log(JSON.stringify({
  status: receipt.status,
  parent_program_sha256: parent.canonical_sha256,
  successor_program_sha256: candidate.canonical_sha256,
  successor_glb_sha256: receipt.successor.glb_sha256,
  successor_glb_bytes: receipt.successor.glb_bytes,
  triangle_count: receipt.successor.triangle_count,
  changed_leaf_count: changedPaths.length,
  frozen_part_count: ledger.frozen_parts.length,
  receipt_sha256: receipt.canonical_sha256,
  visual_status: receipt.visual_status,
}, null, 2))

function assertFrozen(parentProgram, candidateProgram) {
  for (const key of ['assembly', 'parts', 'material_zones', 'presentation', 'budgets', 'unknowns']) {
    if (canonicalJson(parentProgram[key]) !== canonicalJson(candidateProgram[key])) {
      throw new Error(`r8 changed frozen program field ${key}`)
    }
  }
  if (canonicalJson(parentProgram.blade_surface.surface_roles) !== canonicalJson(candidateProgram.blade_surface.surface_roles)) {
    throw new Error('r8 changed frozen blade surface roles')
  }
  for (const curveName of ['spine_curve', 'cutting_edge_curve']) {
    const source = parentProgram.blade_surface[curveName]
    const target = candidateProgram.blade_surface[curveName]
    if (source.curve_id !== target.curve_id || source.basis !== target.basis) {
      throw new Error(`r8 changed stable ${curveName} identity`)
    }
  }
  parentProgram.blade_surface.sections.forEach((source, index) => {
    const target = candidateProgram.blade_surface.sections[index]
    for (const key of Object.keys(source)) {
      if (!['half_width', 'edge_offset', 'spine_offset'].includes(key)
        && canonicalJson(source[key]) !== canonicalJson(target[key])) {
        throw new Error(`r8 changed frozen section field ${source.section_id}.${key}`)
      }
    }
  })
}

function changedLeafPaths(left, right, path = '') {
  if (canonicalJson(left) === canonicalJson(right)) return []
  if (Array.isArray(left) && Array.isArray(right)) {
    return left.flatMap((value, index) => changedLeafPaths(value, right[index], `${path}[${index}]`))
  }
  if (left && right && typeof left === 'object' && typeof right === 'object') {
    return [...new Set([...Object.keys(left), ...Object.keys(right)])].sort()
      .flatMap((key) => changedLeafPaths(left[key], right[key], path ? `${path}.${key}` : key))
  }
  return [path]
}

function allowedPath(path) {
  if (path === 'canonical_sha256') return true
  if (/^blade_surface\.(spine_curve|cutting_edge_curve)\.control_points\[\d+\]\[\d+\]$/.test(path)) return true
  if (/^blade_surface\.sections\[\d+\]\.(half_width|edge_offset|spine_offset)$/.test(path)) return true
  return false
}

function relative(path) {
  return path.startsWith(`${REPO_ROOT}/`) ? path.slice(REPO_ROOT.length + 1) : path
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex')
}

function canonicalJson(value) {
  if (value === undefined) return 'null'
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value)
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error('canonical JSON rejects non-finite numbers')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  }
  throw new Error(`canonical JSON rejects ${typeof value}`)
}
