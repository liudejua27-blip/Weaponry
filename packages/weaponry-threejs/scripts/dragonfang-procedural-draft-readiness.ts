/**
 * Produce a candidate-bound Procedural Draft readiness receipt for the
 * checked-in Dragonfang R2 artifacts.
 *
 * The default invocation is deliberately expected to remain BLOCKED: the R2
 * artifact has no baseline-bound KnifeCandidateStructuralDelta@1.  This
 * script never treats an older GLB, browser screenshot receipt, or summary
 * metric as a structural delta.  A future run may provide an explicit
 * baseline KnifeSceneProgram@1 with --baseline-program; both programs are
 * then compiled and measured in the same fixed eight-view rig.
 */

import {
  compileKnifeSceneProgram,
  createKnifeFixedEightViewRig,
  evaluateKnifeRig,
  evaluateProceduralDraftReadiness,
  measureKnifePartVisibilityMetrics,
  measureProceduralDraftStructuralDelta,
  sha256Hex,
  type CompiledKnifeScene,
  type KnifeSceneProgram,
} from '../src/index.ts'

declare const process: { readonly argv: readonly string[]; cwd(): string }

// This package intentionally has no Node type dependency. Keep the runtime
// file access local to this focused script while retaining a typecheckable
// TypeScript entry point.
// @ts-ignore Node built-in is available to the direct Node invocation.
const { readFile, writeFile } = await import('node:fs/promises') as unknown as {
  readonly readFile: (path: string) => Promise<Uint8Array>
  readonly writeFile: (path: string, data: string, encoding: 'utf8') => Promise<void>
}

const REPO_ROOT = decodeURIComponent(new URL('../../..', import.meta.url).pathname).replace(/\/$/, '')
const PACKAGE_ROOT = `${REPO_ROOT}/packages/weaponry-threejs`
const DEFAULTS = Object.freeze({
  program: `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-first-slice.json`,
  receipt: `${PACKAGE_ROOT}/artifacts/dragonfang-kukri-assembly-r2.receipt.json`,
  glb: `${PACKAGE_ROOT}/artifacts/dragonfang-kukri-assembly-r2.glb`,
  browser_receipt: `${PACKAGE_ROOT}/artifacts/dragonfang-kukri-assembly-r2.browser-receipt.json`,
  metric_status: `${PACKAGE_ROOT}/DRAGONFANG_METRIC_STATUS.json`,
  r1_receipt: `${PACKAGE_ROOT}/artifacts/dragonfang-kukri-assembly-r1.receipt.json`,
  r1_browser_receipt: `${PACKAGE_ROOT}/artifacts/dragonfang-kukri-assembly-r1.browser-receipt.json`,
  r1_glb: `${PACKAGE_ROOT}/artifacts/dragonfang-kukri-assembly-r1.glb`,
  output: `${PACKAGE_ROOT}/artifacts/dragonfang-kukri-assembly-r2.procedural-draft-readiness.json`,
  audit_output: `${PACKAGE_ROOT}/artifacts/dragonfang-kukri-assembly-r2.procedural-draft-readiness.audit.json`,
})

interface CliOptions {
  readonly program: string
  readonly receipt: string
  readonly glb: string
  readonly browser_receipt: string
  readonly metric_status: string
  readonly baseline_program?: string
  readonly baseline_receipt?: string
  readonly output: string
  readonly audit_output: string
}

interface JsonObject {
  readonly [key: string]: unknown
}

interface FileAudit {
  readonly path: string
  readonly exists: boolean
  readonly sha256?: string
  readonly bytes?: number
  readonly parse_status?: 'PARSED' | 'NOT_JSON' | 'NOT_READ'
  readonly reason?: string
}

interface BaselineAudit {
  readonly status: 'BLOCKED_MISSING_RECOMPUTABLE_BASELINE' | 'MEASURED_NONZERO' | 'REJECTED'
  readonly required_inputs: readonly string[]
  readonly provided_baseline_program: FileAudit | null
  readonly inspected_prior_evidence: readonly {
    readonly kind: string
    readonly path: string
    readonly usable: false
    readonly reason: string
  }[]
}

const options = parseCli(process.argv.slice(2))
const programBytes = await readBytes(options.program)
const program = JSON.parse(new TextDecoder().decode(programBytes)) as unknown
const r2Receipt = await readJson(options.receipt)
const glbBytes = await readBytes(options.glb)
const browserReceipt = await readJsonIfPresent(options.browser_receipt)
const browserReceiptBytes = await readBytesIfPresent(options.browser_receipt)
const metricStatus = await readJsonIfPresent(options.metric_status)

let compiled: CompiledKnifeScene | undefined
let compileError: string | null = null
try {
  compiled = compileKnifeSceneProgram(program as KnifeSceneProgram)
} catch (error) {
  compileError = error instanceof Error ? error.message : String(error)
}

let fixedViewMetrics
if (compiled) {
  fixedViewMetrics = measureKnifePartVisibilityMetrics(compiled)
}

const actualProgramSha256 = sha256Hex(programBytes)
const actualGlbSha256 = sha256Hex(glbBytes)
const r2ProgramBytesSha256 = stringField(r2Receipt, 'program_bytes_sha256')
const r2GlbSha256 = stringField(r2Receipt, 'glb_sha256')
const r2GlbBytes = integerField(r2Receipt, 'glb_bytes')
const r2Fingerprint = stringField(r2Receipt, 'deterministic_fingerprint')
const r2Binding = {
  program_path: relativePath(options.program),
  program_bytes_sha256: actualProgramSha256,
  receipt_program_bytes_sha256: r2ProgramBytesSha256,
  program_bytes_match_receipt: actualProgramSha256 === r2ProgramBytesSha256,
  glb_path: relativePath(options.glb),
  glb_sha256: actualGlbSha256,
  receipt_glb_sha256: r2GlbSha256,
  glb_sha256_match_receipt: actualGlbSha256 === r2GlbSha256,
  glb_bytes: glbBytes.byteLength,
  receipt_glb_bytes: r2GlbBytes,
  glb_bytes_match_receipt: glbBytes.byteLength === r2GlbBytes,
  compiled_fingerprint: compiled?.deterministic_fingerprint ?? null,
  receipt_deterministic_fingerprint: r2Fingerprint,
  compiled_fingerprint_match_receipt: compiled?.deterministic_fingerprint === r2Fingerprint,
  r2_receipt_contains_structural_delta: Object.prototype.hasOwnProperty.call(r2Receipt, 'structural_delta'),
  compiled_triangle_count: compiled?.triangle_count ?? null,
  receipt_triangle_count: integerField(r2Receipt, 'triangle_count') ?? integerField(r2Receipt, 'triangles'),
  compiled_part_count: compiled?.parts.length ?? null,
  receipt_part_count: integerField(r2Receipt, 'part_count'),
}

const baselineResult = await resolveBaseline(options, compiled)
const baselineProgramBytes = options.baseline_program ? await readBytes(options.baseline_program) : undefined
const baselineProgram = options.baseline_program
  ? JSON.parse(new TextDecoder().decode(baselineProgramBytes)) as unknown
  : undefined
const readiness = evaluateProceduralDraftReadiness({
  program_bytes: programBytes,
  ...(baselineProgram !== undefined ? { baseline_program: baselineProgram } : {}),
  ...(baselineProgramBytes ? { baseline_program_bytes: baselineProgramBytes } : {}),
  glb_payload: glbBytes,
})

const browserAudit = inspectBrowserReceipt(browserReceipt, browserReceiptBytes, compiled, r2Receipt)
const metricAudit = inspectMetricStatus(metricStatus, r2Receipt, compiled)
const audit = {
  schema_version: 'WeaponryThreeJsProceduralDraftReadinessAudit@1',
  route: 'weaponry-threejs-procedural-draft@1',
  target: 'PROCEDURAL_DRAFT',
  r2_binding: r2Binding,
  fixed_view_metrics: fixedViewMetrics
    ? {
        schema_version: fixedViewMetrics.schema_version,
        source_fingerprint: fixedViewMetrics.source_fingerprint,
        rig_fingerprint: fixedViewMetrics.rig_fingerprint,
        view_ids: fixedViewMetrics.view_ids,
        visible_part_count: fixedViewMetrics.parts.filter((part) => part.visible_view_count > 0).length,
        total_part_count: fixedViewMetrics.parts.length,
        missing_part_ids: fixedViewMetrics.missing_part_ids,
        underexposed_part_ids: fixedViewMetrics.underexposed_part_ids,
        deterministic_fingerprint: fixedViewMetrics.deterministic_fingerprint,
      }
    : { status: 'BLOCKED', reason: compileError ?? 'compiled scene is unavailable' },
  browser_receipt: browserAudit,
  historical_metric_status: metricAudit,
  baseline: baselineResult.audit,
  readiness_receipt_path: relativePath(options.output),
  readiness_receipt: readiness,
}

await writeFile(options.output, `${JSON.stringify(readiness, null, 2)}\n`, 'utf8')
await writeFile(options.audit_output, `${JSON.stringify(audit, null, 2)}\n`, 'utf8')

console.log(JSON.stringify({
  schema_version: readiness.schema_version,
  route: readiness.route,
  status: readiness.status,
  decision: readiness.decision,
  asset_id: readiness.asset_id,
  gate_statuses: Object.fromEntries(Object.entries(readiness.gates).map(([name, gate]) => [name, gate.status])),
  r2_binding: r2Binding,
  structural_delta: {
    status: readiness.gates.structural_delta.status,
    reason: readiness.gates.structural_delta.reason,
    required_inputs: baselineResult.audit.required_inputs,
  },
  readiness_receipt: relativePath(options.output),
  audit_receipt: relativePath(options.audit_output),
}, null, 2))

async function resolveBaseline(
  cli: CliOptions,
  candidate: CompiledKnifeScene | undefined,
): Promise<{
  readonly structural_delta?: ReturnType<typeof measureProceduralDraftStructuralDelta>
  readonly audit: BaselineAudit
}> {
  const requiredInputs = Object.freeze([
    'an explicit baseline KnifeSceneProgram@1 JSON from the same semantic Part/compiler cohort, or a serialized KnifeEightViewEvaluation with full fixed-view masks',
    'the same KnifeFixedEightViewRig@1 identity and frame resolution for baseline and R2 candidate',
    'a baseline program-bytes SHA-256 (or equivalent exact source binding) so the delta can be replayed rather than inferred from screenshots',
  ])

  const inspectedPriorEvidence = Object.freeze([
    {
      kind: 'R1 compile receipt',
      path: relativePath(DEFAULTS.r1_receipt),
      usable: false as const,
      reason: 'legacy receipt has only an older program_fingerprint and no current-cohort fixed-view evaluation masks',
    },
    {
      kind: 'R1 browser receipt',
      path: relativePath(DEFAULTS.r1_browser_receipt),
      usable: false as const,
      reason: 'rendered capture metadata has no KnifeEightViewEvaluation mask binding and uses a different rig fingerprint',
    },
    {
      kind: 'R1 GLB',
      path: relativePath(DEFAULTS.r1_glb),
      usable: false as const,
      reason: 'derived GLB bytes alone do not provide a baseline KnifeSceneProgram or typed fixed-view mask evaluation',
    },
    {
      kind: 'DRAGONFANG_METRIC_STATUS summary',
      path: relativePath(DEFAULTS.metric_status),
      usable: false as const,
      reason: 'its structural_delta summary is from the knowledge-candidate workbench, omits baseline/candidate source fingerprints, and is not R2-bound',
    },
  ])

  if (!cli.baseline_program) {
    return {
      audit: {
        status: 'BLOCKED_MISSING_RECOMPUTABLE_BASELINE',
        required_inputs: requiredInputs,
        provided_baseline_program: null,
        inspected_prior_evidence: inspectedPriorEvidence,
      },
    }
  }

  const baselineBytes = await readBytes(cli.baseline_program)
  const baselineValue = JSON.parse(new TextDecoder().decode(baselineBytes)) as unknown
  const baselineFileAudit: FileAudit = {
    path: relativePath(cli.baseline_program),
    exists: true,
    sha256: sha256Hex(baselineBytes),
    bytes: baselineBytes.byteLength,
    parse_status: 'PARSED',
  }
  if (!candidate) {
    return {
      audit: {
        status: 'REJECTED',
        required_inputs: requiredInputs,
        provided_baseline_program: baselineFileAudit,
        inspected_prior_evidence: inspectedPriorEvidence,
      },
    }
  }

  let baselineCompiled: CompiledKnifeScene
  try {
    baselineCompiled = compileKnifeSceneProgram(baselineValue as KnifeSceneProgram)
  } catch (error) {
    return {
      audit: {
        status: 'REJECTED',
        required_inputs: requiredInputs,
        provided_baseline_program: {
          ...baselineFileAudit,
          reason: error instanceof Error ? error.message : String(error),
        },
        inspected_prior_evidence: inspectedPriorEvidence,
      },
    }
  }

  const baselineReceipt = cli.baseline_receipt ? await readJson(cli.baseline_receipt) : null
  const receiptFingerprint = stringField(baselineReceipt, 'deterministic_fingerprint')
    ?? stringField(baselineReceipt, 'program_fingerprint')
  const receiptProgramBytesSha = stringField(baselineReceipt, 'program_bytes_sha256')
  const samePartIds = baselineCompiled.parts.map((part) => part.part_id).join('|')
    === candidate.parts.map((part) => part.part_id).join('|')
  const baselineReceiptBinding = (!baselineReceipt || (
    (!receiptFingerprint || receiptFingerprint === baselineCompiled.deterministic_fingerprint)
    && (!receiptProgramBytesSha || receiptProgramBytesSha === baselineFileAudit.sha256)
  ))
  if (!samePartIds || !baselineReceiptBinding) {
    return {
      audit: {
        status: 'REJECTED',
        required_inputs: requiredInputs,
        provided_baseline_program: {
          ...baselineFileAudit,
          reason: !samePartIds
            ? 'baseline and candidate do not share the exact compiled Part-ID order'
            : 'baseline receipt does not bind the provided baseline program to its current compiled fingerprint/bytes',
        },
        inspected_prior_evidence: inspectedPriorEvidence,
      },
    }
  }

  const rig = createKnifeFixedEightViewRig()
  const structuralDelta = measureProceduralDraftStructuralDelta(
    evaluateKnifeRig(baselineCompiled, rig),
    evaluateKnifeRig(candidate, rig),
  )
  return {
    structural_delta: structuralDelta,
    audit: {
      status: structuralDelta.status === 'MEASURED_NONZERO' ? 'MEASURED_NONZERO' : 'REJECTED',
      required_inputs: requiredInputs,
      provided_baseline_program: baselineFileAudit,
      inspected_prior_evidence: inspectedPriorEvidence,
    },
  }
}

function inspectBrowserReceipt(
  value: JsonObject | null,
  bytes: Uint8Array | null,
  compiled: CompiledKnifeScene | undefined,
  receipt: JsonObject,
): JsonObject {
  if (!value) return { status: 'NOT_READ', reason: 'browser receipt was unavailable' }
  const preview = isObject(value.preview) ? value.preview : value
  const viewIds = Array.isArray(preview.selected_view_ids) ? preview.selected_view_ids : []
  const sourceFingerprint = stringField(preview, 'source_fingerprint')
  const expectedBrowserSha = stringField(receipt, 'browser_receipt_sha256')
  return {
    status: 'INSPECTED_NOT_A_STRUCTURAL_DELTA',
    schema_version: stringField(value, 'schema_version'),
    asset_id: stringField(preview, 'asset_id'),
    source_fingerprint: sourceFingerprint,
    source_fingerprint_matches_r2: sourceFingerprint !== null
      && sourceFingerprint === compiled?.deterministic_fingerprint,
    selected_view_count: viewIds.length,
    selected_view_ids: viewIds,
    browser_receipt_sha256: bytes ? sha256Hex(bytes) : null,
    expected_receipt_sha256: expectedBrowserSha,
    browser_receipt_sha256_matches_r2: bytes !== null && expectedBrowserSha !== null
      && sha256Hex(bytes) === expectedBrowserSha,
    note: 'browser AOV metadata is supplemental candidate evidence; it is not passed as structural_delta',
  }
}

function inspectMetricStatus(
  value: JsonObject | null,
  receipt: JsonObject,
  compiled: CompiledKnifeScene | undefined,
): JsonObject {
  const boundedSearch = isObject(value?.bounded_search) ? value.bounded_search : null
  const summary = isObject(value?.knowledge_candidate_workbench)
    ? value.knowledge_candidate_workbench
    : null
  const delta = isObject(summary?.structural_delta)
    ? summary.structural_delta
    : isObject(boundedSearch?.structural_delta) ? boundedSearch.structural_delta : null
  const selectedProgramSha = stringField(summary, 'selected_program_sha256')
    ?? stringField(boundedSearch, 'selected_program_sha256')
  const r2ProgramSha = stringField(receipt, 'program_bytes_sha256')
  return {
    status: delta ? 'PRESENT_BUT_NOT_R2_REPLAYABLE' : 'NOT_PRESENT',
    selected_program_sha256: selectedProgramSha,
    selected_program_sha256_matches_r2_bytes: selectedProgramSha !== null && selectedProgramSha === r2ProgramSha,
    compiled_fingerprint: compiled?.deterministic_fingerprint ?? null,
    structural_delta_summary: delta,
    reason: delta
      ? 'summary lacks baseline_source_fingerprint/candidate_source_fingerprint and does not bind to the R2 compiled fingerprint'
      : 'no historical structural delta summary was found',
  }
}

function parseCli(argv: readonly string[]): CliOptions {
  const values = new Map<string, string>()
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (!arg.startsWith('--')) throw new Error(`unsupported argument ${arg}`)
    const key = arg.slice(2).replaceAll('-', '_')
    const value = argv[index + 1]
    if (!value || value.startsWith('--')) throw new Error(`argument --${key.replaceAll('_', '-')} requires a value`)
    values.set(key, value)
    index += 1
  }
  const get = (key: string, fallback?: string): string | undefined => values.get(key) ?? fallback
  const result = {
    program: absolutePath(get('program', relativePath(DEFAULTS.program))!),
    receipt: absolutePath(get('receipt', relativePath(DEFAULTS.receipt))!),
    glb: absolutePath(get('glb', relativePath(DEFAULTS.glb))!),
    browser_receipt: absolutePath(get('browser_receipt', relativePath(DEFAULTS.browser_receipt))!),
    metric_status: absolutePath(get('metric_status', relativePath(DEFAULTS.metric_status))!),
    ...(get('baseline_program') ? { baseline_program: absolutePath(get('baseline_program')!) } : {}),
    ...(get('baseline_receipt') ? { baseline_receipt: absolutePath(get('baseline_receipt')!) } : {}),
    output: absolutePath(get('output', relativePath(DEFAULTS.output))!),
    audit_output: absolutePath(get('audit_output', relativePath(DEFAULTS.audit_output))!),
  }
  return result
}

async function readBytes(path: string): Promise<Uint8Array> {
  return new Uint8Array(await readFile(path))
}

async function readJson(path: string): Promise<JsonObject> {
  const value = JSON.parse(new TextDecoder().decode(await readBytes(path))) as unknown
  if (!isObject(value)) throw new Error(`${relativePath(path)} must contain a JSON object`)
  return value
}

async function readJsonIfPresent(path: string): Promise<JsonObject | null> {
  try {
    return await readJson(path)
  } catch {
    return null
  }
}

async function readBytesIfPresent(path: string): Promise<Uint8Array | null> {
  try {
    return await readBytes(path)
  } catch {
    return null
  }
}

function isObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function stringField(value: JsonObject | null | undefined, key: string): string | null {
  const field = value?.[key]
  return typeof field === 'string' ? field : null
}

function integerField(value: JsonObject | null | undefined, key: string): number | null {
  const field = value?.[key]
  return typeof field === 'number' && Number.isInteger(field) ? field : null
}

function relativePath(path: string): string {
  const prefix = `${REPO_ROOT}/`
  return path.startsWith(prefix) ? path.slice(prefix.length) : path
}

function absolutePath(path: string): string {
  return path.startsWith('/') ? path : `${REPO_ROOT}/${path}`
}
