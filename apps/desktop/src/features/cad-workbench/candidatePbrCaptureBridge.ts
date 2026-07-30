import {
  WORKBENCH_PBR_RENDERER_ID,
  WORKBENCH_PBR_RENDER_MANIFEST_SHA256,
  WORKBENCH_PBR_CAPTURE_HEIGHT_PX,
  WORKBENCH_PBR_CAPTURE_WIDTH_PX,
  captureWorkbenchPbrViews,
  type WorkbenchPbrProjectionCameraBinding,
} from './workbenchPbrCapture.js'
import type { SingleResultDecision } from './singleResultDecisionPresentationState.js'

export type CandidatePbrCaptureIssue = {
  schemaVersion: 'CandidatePbrCaptureSession@1'
  sessionId: string
  projectId: string
  turnId: string
  candidateGlbSha256: string
  shapeProgramSha256: string
  compileReadbackSha256: string
  artifactProfileId: 'interactive_preview' | 'production_concept'
  renderManifestSha256: typeof WORKBENCH_PBR_RENDER_MANIFEST_SHA256
  expectedRendererId: typeof WORKBENCH_PBR_RENDERER_ID
  expiresAtUnixMs: number
  requiredViewIds: readonly string[]
  maxViewBytes: number
  maxTotalBytes: number
  captureWidthPx: number
  captureHeightPx: number
  projectionCameraBindings: readonly WorkbenchPbrProjectionCameraBinding[]
  glbBase64: string
}

export type CandidatePbrCaptureReceipt = {
  schemaVersion: 'NativeCandidatePbrCaptureEvidence@1'
  sessionId: string
  candidateGlbSha256: string
  rendererId: typeof WORKBENCH_PBR_RENDERER_ID
  renderManifestSha256: typeof WORKBENCH_PBR_RENDER_MANIFEST_SHA256
  captureSha256: string
  viewIds: readonly string[]
}

export type CandidatePbrCaptureResume = {
  schemaVersion: 'NativeCandidatePbrCaptureResume@1'
  executionId: string
  projectId: string
  turnId: string
  candidateGlbSha256: string
  status: 'preview_ready' | 'repair_required' | 'capture_required' | 'authorization_required'
  hardGatePassed: boolean
  previewId: string | null
  singleResultDecision: SingleResultDecision | null
  visualRepairTargetProjection: Record<string, unknown> | null
}

export type CandidatePbrVisualComparisonAuthorization = {
  authorizationId: string
  authorizationBindingSha256: string
  expiresAtUnixMs: number
  maximumCalls: number
  maximumVariableCostMicrousd: number
}

/**
 * Issues one Rust-bound candidate inspection. The returned GLB is ephemeral:
 * callers must load it into the already-mounted workbench renderer and must
 * not treat it as a result preview, CAS resource, or exportable asset.
 */
export async function issueCandidatePbrCapture(input: {
  executionId: string
  projectId: string
  turnId: string
}): Promise<CandidatePbrCaptureIssue> {
  return invokeDesktop<CandidatePbrCaptureIssue>('forgecad_candidate_pbr_capture_issue', { request: input })
}

/**
 * Captures exactly the Rust-issued view set from the existing renderer and
 * atomically submits it. The bridge sends PNG bytes only; Rust derives the
 * GLB, renderer, manifest, image hashes and total byte proof from the live
 * session rather than trusting this WebView.
 */
export async function captureAndSubmitCandidatePbr(input: {
  viewport: HTMLElement
  issue: CandidatePbrCaptureIssue
}): Promise<CandidatePbrCaptureReceipt> {
  const issue = input.issue
  if (
    issue.expectedRendererId !== WORKBENCH_PBR_RENDERER_ID
    || issue.renderManifestSha256 !== WORKBENCH_PBR_RENDER_MANIFEST_SHA256
    || issue.requiredViewIds.join(',')
      !== 'turntable_000,turntable_045,turntable_090,turntable_135,turntable_180,turntable_225,turntable_270,turntable_315'
    || Date.now() > issue.expiresAtUnixMs
    || issue.captureWidthPx !== WORKBENCH_PBR_CAPTURE_WIDTH_PX
    || issue.captureHeightPx !== WORKBENCH_PBR_CAPTURE_HEIGHT_PX
    || !hasExactProjectionCameraBindings(issue)
  ) throw new Error('CANDIDATE_PBR_CAPTURE_ISSUE_INVALID')

  const captures = await captureWorkbenchPbrViews({
    viewport: input.viewport,
    sourceGlbSha256: issue.candidateGlbSha256,
    projectionCameraBindings: issue.projectionCameraBindings,
  })
  if (captures.length !== issue.requiredViewIds.length) {
    throw new Error('CANDIDATE_PBR_CAPTURE_VIEW_SET_INCOMPLETE')
  }
  const totalBytes = captures.reduce(
    (total, capture) => total + capture.png_bytes.byteLength + capture.auxiliary.png_bytes.byteLength,
    0,
  )
  if (
    totalBytes > issue.maxTotalBytes
    || captures.some((capture) => capture.png_bytes.byteLength > issue.maxViewBytes || capture.auxiliary.png_bytes.byteLength === 0)
    || captures.some((capture) => capture.projection_camera_binding_sha256 !== bindingShaFor(issue, capture.view_id))
  ) throw new Error('CANDIDATE_PBR_CAPTURE_BYTE_BUDGET_EXCEEDED')

  const response = await invokeDesktop<CandidatePbrCaptureReceipt>(
    'forgecad_candidate_pbr_capture_submit',
    {
      request: {
        sessionId: issue.sessionId,
        captures: await Promise.all(captures.map(async (capture) => ({
          schemaVersion: 'NativeCandidatePbrCaptureUpload@1',
          viewId: capture.view_id,
          cameraPoseSha256: capture.camera_pose_sha256,
          projectionCameraBindingSha256: capture.projection_camera_binding_sha256,
          pngBase64: bytesToBase64(capture.png_bytes),
          auxiliaryPngBase64: bytesToBase64(capture.auxiliary.png_bytes),
        }))),
      },
    },
  )
  if (
    response.sessionId !== issue.sessionId
    || response.candidateGlbSha256 !== issue.candidateGlbSha256
    || response.rendererId !== WORKBENCH_PBR_RENDERER_ID
    || response.renderManifestSha256 !== WORKBENCH_PBR_RENDER_MANIFEST_SHA256
    || response.viewIds.join(',') !== issue.requiredViewIds.join(',')
  ) throw new Error('CANDIDATE_PBR_CAPTURE_RECEIPT_MISMATCH')
  return response
}

function hasExactProjectionCameraBindings(issue: CandidatePbrCaptureIssue): boolean {
  if (issue.projectionCameraBindings.length !== issue.requiredViewIds.length) return false
  return issue.requiredViewIds.every((viewId, index) => {
    const binding = issue.projectionCameraBindings[index]
    return binding?.viewId === viewId
      && binding.candidateGlbSha256.toLowerCase() === issue.candidateGlbSha256.toLowerCase()
      && binding.schemaVersion === 'ProjectionCameraBinding@1'
      && binding.algorithmId === 'forgecad.turntable_projection_camera'
      && binding.algorithmVersion === '1'
      && binding.verticalFovMillidegrees === 38000
      && binding.frameTargetNdcMillionths === 840000
      && /^[0-9a-f]{64}$/.test(binding.bindingSha256)
      && binding.worldToClipRowMajor.length === 16
      && binding.worldToClipRowMajor.every(Number.isFinite)
  })
}

function bindingShaFor(issue: CandidatePbrCaptureIssue, viewId: string): string | null {
  return issue.projectionCameraBindings.find((binding) => binding.viewId === viewId)?.bindingSha256 ?? null
}

/**
 * Resumes only the Rust-owned evaluate → preview tail after a capture was
 * adopted. It has no geometry, model-authoring, or version-write authority.
 * `capture_required` means the single Rust-owned patch produced a new GLB;
 * the desktop must capture that exact candidate again before evaluation can
 * continue. `repair_required` is retained only for compatibility readers.
 */
export async function resumeCandidatePbrCapture(input: {
  executionId: string
  projectId: string
  turnId: string
}): Promise<CandidatePbrCaptureResume> {
  const response = await invokeDesktop<CandidatePbrCaptureResume>(
    'forgecad_candidate_pbr_capture_resume',
    { request: input },
  )
  if (
    response.schemaVersion !== 'NativeCandidatePbrCaptureResume@1'
    || !['preview_ready', 'repair_required', 'capture_required', 'authorization_required'].includes(response.status)
    || response.executionId !== input.executionId
    || response.projectId !== input.projectId
    || response.turnId !== input.turnId
    || (response.status === 'preview_ready' && !response.hardGatePassed)
    || (response.status !== 'preview_ready' && response.hardGatePassed)
    || (response.status === 'preview_ready' && !response.previewId)
    || (response.status === 'preview_ready' && !response.singleResultDecision)
    || (response.status === 'preview_ready' && response.visualRepairTargetProjection !== null)
    || (response.status === 'repair_required' && response.previewId !== null)
    || (response.status === 'repair_required' && response.singleResultDecision !== null)
    || (response.status === 'capture_required' && response.previewId !== null)
    || (response.status === 'capture_required' && response.singleResultDecision !== null)
    || (response.status === 'capture_required' && response.visualRepairTargetProjection !== null)
    || (response.status === 'authorization_required' && response.previewId !== null)
    || (response.status === 'authorization_required' && response.singleResultDecision !== null)
    || (response.status === 'authorization_required' && response.visualRepairTargetProjection !== null)
    || (response.visualRepairTargetProjection !== null && !isRepairTargetProjection(response.visualRepairTargetProjection))
  ) throw new Error('CANDIDATE_PBR_CAPTURE_RESUME_INVALID')
  return response
}

/**
 * Explicitly authorizes one Qwen comparison after the exact universal GLB has
 * been rendered and captured. It creates no preview or version and carries no
 * image bytes; Rust recomputes the sealed comparison scope server-side.
 */
export async function authorizeCandidatePbrVisualComparison(input: {
  clientRequestId: string
  executionId: string
  projectId: string
  turnId: string
}): Promise<CandidatePbrVisualComparisonAuthorization> {
  const response = await invokeDesktop<CandidatePbrVisualComparisonAuthorization>(
    'authorize_candidate_pbr_visual_comparison',
    { input },
  )
  if (
    !/^visauth_[A-Za-z0-9_.:-]+$/.test(response.authorizationId)
    || !/^[0-9a-f]{64}$/.test(response.authorizationBindingSha256)
    || !Number.isFinite(response.expiresAtUnixMs)
    || response.expiresAtUnixMs <= Date.now()
    || response.maximumCalls !== 3
    || response.maximumVariableCostMicrousd !== 100000
  ) throw new Error('UNIVERSAL_VISUAL_AUTHORIZATION_RESPONSE_INVALID')
  return response
}

function isRepairTargetProjection(value: Record<string, unknown>): boolean {
  return typeof value.program_id === 'string'
    && typeof value.source_revision === 'number'
    && Number.isInteger(value.source_revision)
    && value.source_revision > 0
    && typeof value.source_program_sha256 === 'string'
    && /^[0-9a-f]{64}$/.test(value.source_program_sha256)
    && typeof value.comparison_input_sha256 === 'string'
    && /^[0-9a-f]{64}$/.test(value.comparison_input_sha256)
    && typeof value.comparison_report_sha256 === 'string'
    && /^[0-9a-f]{64}$/.test(value.comparison_report_sha256)
    && Array.isArray(value.targets)
    && value.targets.length > 0
    && value.targets.length <= 8
}

function bytesToBase64(bytes: Uint8Array): string {
  const chunkBytes = 0x8000
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += chunkBytes) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkBytes))
  }
  return window.btoa(binary)
}

async function invokeDesktop<T>(command: string, payload: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(command, payload)
}
