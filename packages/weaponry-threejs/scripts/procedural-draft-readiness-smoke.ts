import { GLTFExporter } from 'three/examples/jsm/exporters/GLTFExporter.js'

import {
  compileKnifeSceneProgram,
  evaluateProceduralDraftReadiness,
  type CompiledKnifeScene,
} from '../src/index.ts'
import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import type { KnifeSceneProgram } from '../src/knife-scene-program.ts'

/** Minimal FileReader bridge required by GLTFExporter in a Node smoke. */
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
        this.result = `data:${blob.type || 'application/octet-stream'};base64,${base64(new Uint8Array(value))}`
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

const baselineProgram = knifeSceneProgramFixture as unknown as KnifeSceneProgram
const baselineGuard = baselineProgram.assembly?.guard
if (!baselineGuard) throw new Error('fixture must contain a guard for structural-delta smoke')
const candidateProgram: KnifeSceneProgram = {
  ...baselineProgram,
  assembly: {
    ...baselineProgram.assembly,
    guard: {
      ...baselineGuard,
      center: [-1.04, 0.12, 0],
      span: 0.7,
    },
  },
}
const baselineProgramBytes = new TextEncoder().encode(JSON.stringify(baselineProgram))
const candidateProgramBytes = new TextEncoder().encode(JSON.stringify(candidateProgram))
const candidate = await compileAndExport(candidateProgram)
const glbBytes = candidate.glb

const ready = evaluateProceduralDraftReadiness({
  program_bytes: candidateProgramBytes,
  baseline_program: baselineProgram,
  baseline_program_bytes: baselineProgramBytes,
  glb_payload: glbBytes,
})
if (ready.status !== 'THREEJS_DESIGN_READY'
  || ready.decision !== 'THREEJS_DESIGN_READY'
  || ready.gates.closed_program.status !== 'PASS'
  || ready.gates.compilation.status !== 'PASS'
  || ready.gates.parts_materials.status !== 'PASS'
  || ready.gates.fixed_view_observability.status !== 'PASS'
  || ready.gates.structural_delta.status !== 'PASS'
  || ready.gates.budgets.status !== 'PASS'
  || ready.gates.glb_receipt.status !== 'PASS'
  || ready.gates.glb_receipt.mesh_count !== candidate.compiled.parts.length
  || ready.gates.glb_receipt.material_count !== new Set(candidate.compiled.parts.map((part) => part.material_zone_id)).size
  || ready.gates.glb_receipt.triangle_count !== candidate.compiled.triangle_count
  || ready.likeness_status !== 'NOT_REQUESTED'
  || ready.visual_status !== 'NOT_REQUESTED'
  || ready.human_status !== 'NOT_RUN'
  || ready.commercial_status !== 'NOT_RUN') {
  throw new Error(`procedural draft should be ready: ${JSON.stringify(ready)}`)
}

const blockedWithoutDelta = evaluateProceduralDraftReadiness({
  program_bytes: candidateProgramBytes,
  glb_payload: glbBytes,
})
if (blockedWithoutDelta.status !== 'BLOCKED' || blockedWithoutDelta.gates.structural_delta.status !== 'BLOCKED') {
  throw new Error('missing structural delta did not fail closed')
}

const blockedWithUnknownProgramField = evaluateProceduralDraftReadiness({
  program_bytes: new TextEncoder().encode(JSON.stringify({ ...candidateProgram, caller_script: 'not-accepted' })),
  baseline_program: baselineProgram,
  baseline_program_bytes: baselineProgramBytes,
  glb_payload: glbBytes,
})
if (blockedWithUnknownProgramField.status !== 'BLOCKED' || blockedWithUnknownProgramField.gates.closed_program.status !== 'BLOCKED') {
  throw new Error('unknown program field did not fail closed')
}

const tamperedGlb = glbBytes.slice()
tamperedGlb[0] ^= 1
const blockedWithTamperedGlb = evaluateProceduralDraftReadiness({
  program_bytes: candidateProgramBytes,
  baseline_program: baselineProgram,
  baseline_program_bytes: baselineProgramBytes,
  glb_payload: tamperedGlb,
})
if (blockedWithTamperedGlb.status !== 'BLOCKED' || blockedWithTamperedGlb.gates.glb_receipt.status !== 'BLOCKED') {
  throw new Error('tampered GLB payload did not fail closed')
}

console.log(JSON.stringify({
  schema_version: ready.schema_version,
  route: ready.route,
  status: ready.status,
  decision: ready.decision,
  asset_id: ready.asset_id,
  gates: Object.fromEntries(Object.entries(ready.gates).map(([name, gate]) => [name, gate.status])),
  triangle_count: ready.gates.budgets.triangle_count,
  draw_call_count: ready.gates.budgets.draw_call_count,
  visible_part_count: ready.gates.fixed_view_observability.visible_part_count,
  fixed_view_count: ready.gates.fixed_view_observability.view_ids.length,
  structural_delta: ready.gates.structural_delta.changed_pixel_count,
  program_bytes_sha256: ready.program_bytes_sha256,
  glb_bytes: ready.gates.glb_receipt.glb_bytes,
  glb_sha256: ready.gates.glb_receipt.glb_sha256,
  glb_readback: {
    mesh_count: ready.gates.glb_receipt.mesh_count,
    mesh_node_count: ready.gates.glb_receipt.mesh_node_count,
    material_count: ready.gates.glb_receipt.material_count,
    triangle_count: ready.gates.glb_receipt.triangle_count,
  },
  likeness_status: ready.likeness_status,
  visual_status: ready.visual_status,
  human_status: ready.human_status,
  commercial_status: ready.commercial_status,
  negative_cases: {
    missing_structural_delta: blockedWithoutDelta.status,
    unknown_program_field: blockedWithUnknownProgramField.status,
    tampered_glb_payload: blockedWithTamperedGlb.status,
  },
}))

async function exportGlb(compiled: ReturnType<typeof compileKnifeSceneProgram>): Promise<Uint8Array> {
  compiled.group.updateMatrixWorld(true)
  const result = await new GLTFExporter().parseAsync(compiled.group, {
    binary: true,
    onlyVisible: true,
    trs: false,
  })
  if (!(result instanceof ArrayBuffer)) throw new Error('GLTFExporter did not return binary GLB bytes')
  return new Uint8Array(result)
}

async function compileAndExport(program: KnifeSceneProgram): Promise<{
  readonly compiled: CompiledKnifeScene
  readonly glb: Uint8Array
}> {
  const compiled = compileKnifeSceneProgram(program)
  return { compiled, glb: await exportGlb(compiled) }
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
