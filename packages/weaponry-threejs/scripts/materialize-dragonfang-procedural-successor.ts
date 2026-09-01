import { GLTFExporter } from 'three/examples/jsm/exporters/GLTFExporter.js'

import dragonfangBaseline from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-first-slice.json' with { type: 'json' }
import dragonfangLedger from '../../../skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r6-intrinsic.json' with { type: 'json' }
import {
  canonicalKnifeProgramSha256,
  compileKnifeSceneProgram,
  createKnifeFixedEightViewRig,
  evaluateKnifeRig,
  evaluateProceduralDraftReadiness,
  generateKnifeObjectiveLedgerCandidates,
  measureProceduralDraftStructuralDelta,
  sha256Hex,
  type KnifeObjectiveLedger,
  type KnifeSceneProgram,
} from '../src/index.ts'

declare const process: { cwd(): string }

// @ts-ignore Node built-ins are available to this checked-in materializer.
const { readFile, writeFile } = await import('node:fs/promises') as unknown as {
  readonly readFile: (path: string) => Promise<Uint8Array>
  readonly writeFile: (path: string, data: Uint8Array | string, encoding?: 'utf8') => Promise<void>
}

const REPO_ROOT = decodeURIComponent(new URL('../../..', import.meta.url).pathname).replace(/\/$/, '')
const BASELINE_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-first-slice.json`
const SUCCESSOR_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r7.json`
const ARTIFACT_ROOT = `${REPO_ROOT}/packages/weaponry-threejs/artifacts`
const GLB_PATH = `${ARTIFACT_ROOT}/dragonfang-kukri-procedural-r7.glb`
const RECEIPT_PATH = `${ARTIFACT_ROOT}/dragonfang-kukri-procedural-r7.receipt.json`
const READINESS_PATH = `${ARTIFACT_ROOT}/dragonfang-kukri-procedural-r7.readiness.json`

/** Minimal FileReader bridge required by GLTFExporter in Node. */
class NodeFileReader {
  result: ArrayBuffer | string | null = null
  error: unknown = null
  onloadend: (() => void) | null = null
  onerror: ((error: unknown) => void) | null = null

  readAsArrayBuffer(blob: Blob): void {
    blob.arrayBuffer().then(
      (value) => { this.result = value; this.onloadend?.() },
      (error) => { this.error = error; this.onerror?.(error) },
    )
  }

  readAsDataURL(blob: Blob): void {
    blob.arrayBuffer().then(
      (value) => {
        this.result = `data:${blob.type || 'application/octet-stream'};base64,${base64(new Uint8Array(value))}`
        this.onloadend?.()
      },
      (error) => { this.error = error; this.onerror?.(error) },
    )
  }
}

if (typeof globalThis.FileReader === 'undefined') {
  Object.defineProperty(globalThis, 'FileReader', { configurable: true, value: NodeFileReader })
}

const baseline = dragonfangBaseline as unknown as KnifeSceneProgram
const ledger = dragonfangLedger as unknown as KnifeObjectiveLedger
const baselineBytes = new Uint8Array(await readFile(BASELINE_PATH))
const baselineCompiled = compileKnifeSceneProgram(baseline)
const rig = createKnifeFixedEightViewRig()
const baselineEvaluation = evaluateKnifeRig(baselineCompiled, rig)
const plan = generateKnifeObjectiveLedgerCandidates(baseline, ledger, {
  candidate_count: 3,
  seed: 0x44524736,
})

const ranked = plan.candidates.map((candidate) => {
  const compiled = compileKnifeSceneProgram(candidate.program)
  const delta = measureProceduralDraftStructuralDelta(
    baselineEvaluation,
    evaluateKnifeRig(compiled, rig),
  )
  return { candidate, compiled, delta, changed: delta.silhouette_changed_pixel_count + delta.part_id_changed_pixel_count }
}).sort((left, right) => right.changed - left.changed || left.candidate.ordinal - right.candidate.ordinal)

const selected = ranked[0]
if (!selected || selected.delta.status !== 'MEASURED_NONZERO' || selected.changed < 1) {
  throw new Error('Dragonfang successor selection produced no fixed-view structural delta')
}

// Materialize through JSON first, then let the readiness parser own the exact
// source-byte canonical policy. This avoids silently changing identities when
// a JS number was originally written as (for example) 1.0.
const normalizedDraft = JSON.parse(JSON.stringify(selected.candidate.program)) as KnifeSceneProgram
const provisionalBytes = new TextEncoder().encode(`${JSON.stringify(normalizedDraft, null, 2)}\n`)
const canonicalProjection = evaluateProceduralDraftReadiness({
  program_bytes: provisionalBytes,
  baseline_program: baseline,
  baseline_program_bytes: baselineBytes,
  glb_payload: new Uint8Array(),
})
const successorCanonical = canonicalProjection.gates.closed_program.canonical_sha256
if (!successorCanonical) throw new Error('readiness canonical projection rejected the normalized successor program')
const successor: KnifeSceneProgram = Object.freeze({
  ...normalizedDraft,
  canonical_sha256: successorCanonical,
})
const successorText = `${JSON.stringify(successor, null, 2)}\n`
const successorBytes = new TextEncoder().encode(successorText)
const successorCompiled = compileKnifeSceneProgram(successor)
const successorDelta = measureProceduralDraftStructuralDelta(
  baselineEvaluation,
  evaluateKnifeRig(successorCompiled, rig),
)
if (successorDelta.status !== 'MEASURED_NONZERO'
  || successorDelta.candidate_source_fingerprint !== successorCompiled.deterministic_fingerprint) {
  throw new Error('materialized Dragonfang successor drifted from the selected proposal')
}

successorCompiled.group.updateMatrixWorld(true)
const exported = await new GLTFExporter().parseAsync(successorCompiled.group, {
  binary: true,
  onlyVisible: true,
  trs: false,
})
if (!(exported instanceof ArrayBuffer)) throw new Error('GLTFExporter did not return binary GLB bytes')
const glbBytes = new Uint8Array(exported)
const readiness = evaluateProceduralDraftReadiness({
  program_bytes: successorBytes,
  baseline_program: baseline,
  baseline_program_bytes: baselineBytes,
  glb_payload: glbBytes,
})
if (readiness.status !== 'THREEJS_DESIGN_READY') {
  throw new Error(`Dragonfang successor readiness remained blocked: ${readiness.reasons.join('; ')}`)
}

const receiptDraft = {
  schema_version: 'WeaponryThreeJsProceduralSuccessorLineage@1',
  route: 'weaponry-threejs-procedural-draft@1',
  asset_id: successor.asset_id,
  baseline: {
    program_path: relative(BASELINE_PATH),
    program_bytes_sha256: sha256Hex(baselineBytes),
    program_sha256: canonicalKnifeProgramSha256(baseline),
    compiled_fingerprint: baselineCompiled.deterministic_fingerprint,
  },
  generation: {
    objective_ledger_path: 'skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r6-intrinsic.json',
    objective_ledger_sha256: ledger.canonical_sha256,
    candidate_plan_fingerprint: plan.deterministic_fingerprint,
    candidate_count: plan.generated_candidate_count,
    seed: plan.seed,
    selection_policy: 'maximum-fixed-eight-view-structural-delta-then-lowest-ordinal@1',
    selected_candidate_id: selected.candidate.candidate_id,
    selected_candidate_ordinal: selected.candidate.ordinal,
    selected_mutation_scope: selected.candidate.mutation_scope,
    changed_parameter_paths: selected.candidate.changed_parameter_paths,
    changes: selected.candidate.changes,
    proposal_status: selected.candidate.proposal_status,
  },
  successor: {
    program_path: relative(SUCCESSOR_PATH),
    program_bytes_sha256: sha256Hex(successorBytes),
    program_sha256: successorCanonical,
    compiled_fingerprint: successorCompiled.deterministic_fingerprint,
    glb_path: relative(GLB_PATH),
    glb_sha256: sha256Hex(glbBytes),
    glb_bytes: glbBytes.byteLength,
    triangle_count: successorCompiled.triangle_count,
    part_ids: successorCompiled.parts.map((part) => part.part_id),
    material_zone_ids: [...new Set(successorCompiled.parts.map((part) => part.material_zone_id))].sort(),
  },
  structural_delta: successorDelta,
  readiness: {
    receipt_path: relative(READINESS_PATH),
    status: readiness.status,
    deterministic_fingerprint: readiness.deterministic_fingerprint,
  },
  status: 'SUCCESSOR_MATERIALIZED_REVIEW_ONLY',
  likeness_status: 'NOT_REQUESTED',
  visual_status: 'NOT_REVIEWED',
  quality_status: 'NOT_RUN',
  human_status: 'NOT_RUN',
  commercial_status: 'NOT_RUN',
  runtime_persistence_status: 'NOT_RUN',
  canonical_sha256: '',
}
const receipt = {
  ...receiptDraft,
  canonical_sha256: sha256Hex(canonicalJson(receiptDraft)),
}

await writeFile(SUCCESSOR_PATH, successorBytes)
await writeFile(GLB_PATH, glbBytes)
await writeFile(RECEIPT_PATH, `${JSON.stringify(receipt, null, 2)}\n`, 'utf8')
await writeFile(READINESS_PATH, `${JSON.stringify(readiness, null, 2)}\n`, 'utf8')

console.log(JSON.stringify({
  status: readiness.status,
  baseline_program_bytes_sha256: receipt.baseline.program_bytes_sha256,
  successor_program_sha256: receipt.successor.program_sha256,
  successor_program_bytes_sha256: receipt.successor.program_bytes_sha256,
  successor_compiled_fingerprint: receipt.successor.compiled_fingerprint,
  selected_candidate_id: receipt.generation.selected_candidate_id,
  selected_mutation_scope: receipt.generation.selected_mutation_scope,
  structural_delta: successorDelta,
  glb_sha256: receipt.successor.glb_sha256,
  glb_bytes: receipt.successor.glb_bytes,
  triangle_count: receipt.successor.triangle_count,
  lineage_receipt_sha256: receipt.canonical_sha256,
  visual_status: receipt.visual_status,
  commercial_status: receipt.commercial_status,
}, null, 2))

function relative(path: string): string {
  return path.startsWith(`${REPO_ROOT}/`) ? path.slice(REPO_ROOT.length + 1) : path
}

function canonicalJson(value: unknown): string {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value)
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error('canonical JSON rejects non-finite numbers')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (typeof value === 'object') {
    const record = value as Record<string, unknown>
    return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(',')}}`
  }
  throw new Error(`canonical JSON rejects ${typeof value}`)
}

function base64(bytes: Uint8Array): string {
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
