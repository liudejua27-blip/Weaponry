import { isTauriRuntime } from '../tauri/agentSupervisor.js'

const PROBE_SCHEMA = 'ForgeCADArmWebViewQa@1' as const
const MAX_WAIT_MS = 180_000
const POLL_MS = 120
const ARM_BRIEF = '流线三关节维护机械臂，固定基座、双连杆、旋转腕部和夹爪'
const SHA256_PATTERN = /^[a-f0-9]{64}$/
const STABLE_ID_PATTERN = /^[A-Za-z0-9_.-]{1,160}$/
const QA_VIEWPORT_CAPTURE_EVENT = 'forgecad:qa-capture-viewport@1'
const QA_VIEWPORT_CAPTURE_TIMEOUT_MS = 12_000

type ProbeConfig = {
  schema_version: typeof PROBE_SCHEMA
  phase: 'initial' | 'restart'
  reference_class: R007bReferenceClass
  r007b_visual_evidence: boolean
}

type R007bReferenceClass = 'single_image' | 'multi_view_contact_sheet' | 'strict_glb_readback'

type ProbeReport = {
  schema_version: typeof PROBE_SCHEMA
  phase: 'initial' | 'restart'
  ok: boolean
  project_id?: string
  turn_id?: string
  preview_id?: string
  preview_artifact_sha256?: string
  v1_asset_version_id?: string
  v2_asset_version_id?: string
  v3_asset_version_id?: string
  snapshot_revision?: number
  renderer_generation?: number
  active_webgl_contexts?: number
  production_glb_render_source?: 'glb_pbr'
  a005_preview_seen?: boolean
  r007b_preview_seen?: boolean
  r007b_v3_confirmed?: boolean
  v3_glb_download_confirmed?: boolean
  v3_production_glb?: QaGlbCapture
  v3_viewport_screenshot?: QaPngCapture
  visual_fidelity_validated?: false
  restart_hydrated?: boolean
  r007b_visual_run?: Record<string, unknown>
  error_code?: string
}

type QaPngCapture = {
  relative_path: string
  sha256: string
  byte_size: number
  width: number
  height: number
}

type QaGlbCapture = {
  relative_path: string
  sha256: string
  byte_size: number
  triangle_count: number
  complete_pbr_material_count: number
}

type QaCaptureRequest = {
  schema_version: typeof PROBE_SCHEMA
  phase: 'initial'
  kind: 'v3_viewport_png' | 'v3_production_glb' | 'r007b_reference_png' | 'r007b_result_png'
  bytes_base64: string
}

type QaViewportPixels = {
  width: number
  height: number
  pixels: Uint8Array
}

type QaViewportCaptureRequest = {
  viewport: HTMLElement
  resolve: (capture: QaViewportPixels) => void
  reject: (error: Error) => void
}

type QaR007bLineage = {
  evidenceId: string
  sourceObjectSha256: string
  rebuildPlanId: string
  analysisId: string
  fidelityCeiling: string
  previewChangeSetId: string
  confirmedAssetVersionId: string | null
  resultGlbSha256: string | null
}

let probePromise: Promise<void> | null = null

/**
 * Opt-in F026 acceptance exercise for the *rendered packaged WebView*.
 *
 * This module deliberately has no ForgeApi/AppServer imports.  It interacts
 * only with accessible controls which a zero-base user can see, then gathers
 * bounded DOM facts from the one existing Three.js host.  Consequently it
 * cannot pass by talking to the Rust bridge directly, installing a proxy, or
 * creating a second renderer.  Normal launches are inert: the Rust command
 * returns `null` unless the explicit QA environment is present.
 */
export function runPackagedArmWebviewQaOnce(): Promise<void> {
  if (!isTauriRuntime()) return Promise.resolve()
  if (!probePromise) probePromise = runPackagedArmWebviewQa()
  return probePromise
}

async function runPackagedArmWebviewQa(): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  let config: ProbeConfig | null
  try {
    config = await invoke<ProbeConfig | null>('forgecad_arm_webview_qa_config')
  } catch {
    return
  }
  if (config === null) return
  if (
    config.schema_version !== PROBE_SCHEMA
    || !['initial', 'restart'].includes(config.phase)
    || !['single_image', 'multi_view_contact_sheet', 'strict_glb_readback'].includes(config.reference_class)
    || typeof config.r007b_visual_evidence !== 'boolean'
  ) {
    await reportFailure('initial', 'QA_CONFIG_INVALID')
    return
  }
  try {
    if (config.phase === 'restart') {
      await runRestart(config)
    } else {
      await runInitial(config)
    }
  } catch (caught) {
    const code = caught instanceof Error && /^QA_[A-Z0-9_]{1,120}$/.test(caught.message)
      ? caught.message
      : 'QA_EXECUTION_FAILED'
    await reportFailure(config.phase, code)
  }
}

async function runInitial(config: ProbeConfig): Promise<void> {
  const root = await waitFor<HTMLElement>(() => document.querySelector<HTMLElement>('[data-testid="cad-workbench"]'), 'QA_WORKBENCH_MISSING')
  await reportProgress('workbench_ready')
  await waitForProject(root)
  await reportProgress('project_ready')
  const composer = await waitFor<HTMLTextAreaElement>(
    () => document.querySelector<HTMLTextAreaElement>('[aria-label="设计需求"]:not(:disabled)'),
    'QA_COMPOSER_DISABLED',
  )
  setTextarea(composer, ARM_BRIEF)
  const send = await waitFor<HTMLButtonElement>(() => document.querySelector<HTMLButtonElement>('[aria-label="发送设计需求"]:not(:disabled)'), 'QA_SEND_DISABLED')
  send.click()
  await reportProgress('brief_sent')

  const result = await waitForMutation<HTMLElement>(
    () => document.querySelector<HTMLElement>('[data-generation-state="ready"]'),
    'QA_SINGLE_RESULT_NOT_READY',
  )
  await reportProgress('single_result_ready')
  const decision = readVisibleDecision(root)
  if (!decision) throw new Error('QA_SINGLE_RESULT_LINEAGE_MISSING')
  const viewport = await waitForViewport('QA_PREVIEW_VIEWPORT_MISSING')
  assertProductionRenderer(viewport)
  await reportProgress('preview_ready')
  const save = result.querySelector<HTMLButtonElement>('[aria-label="保存为可编辑模型"]')
  if (!save || save.disabled) throw new Error('QA_CONFIRM_MISSING')
  save.click()

  const v1 = await waitForActiveAsset(root, null, 'QA_V1_SNAPSHOT_MISSING')
  await reportProgress('v1_confirmed')
  const firstPart = await waitFor<HTMLButtonElement>(
    // The drawer's default engraving/parallel/center program is explicitly
    // allowed by C106's link-armor slot.  Selecting an arbitrary first part
    // could correctly hit a different closed Recipe policy (for example the
    // base permits flowline/pattern only) and would not exercise A005.
    () => document.querySelector<HTMLButtonElement>('[aria-label="选择部件 连杆护甲"]:not(:disabled)'),
    'QA_C106_LINK_ARMOR_MISSING',
  )
  firstPart.click()
  await waitForMutation<HTMLButtonElement>(() => {
    const selected = document.querySelector<HTMLButtonElement>('[aria-label="选择部件 连杆护甲"][aria-pressed="true"]')
    return selected ?? null
  }, 'QA_C106_LINK_ARMOR_SNAPSHOT_NOT_SELECTED')
  await reportProgress('part_selected')
  const adorn = await waitFor<HTMLButtonElement>(
    () => document.querySelector<HTMLButtonElement>('[aria-label="添加外观细节"]:not(:disabled)'),
    'QA_A005_ACTION_MISSING',
  )
  adorn.click()
  await waitForMutation<HTMLElement>(() => document.querySelector<HTMLElement>('[role="dialog"][aria-label="添加外观细节"]'), 'QA_A005_DRAWER_MISSING')
  await reportProgress('a005_open')
  await previewAndRetainSurfaceAdornment()
  await reportProgress('a005_retained')
  const v2 = await waitForActiveAsset(root, v1.assetVersionId, 'QA_A005_V2_SNAPSHOT_MISSING')
  await waitForAgentSnapshotIdle('QA_A005_PREVIEW_NOT_CLEARED')
  await reportProgress('v2_ready')
  await closeCompletedDrawer('关闭添加外观细节', '[role="dialog"][aria-label="添加外观细节"]', 'QA_A005_DRAWER_CLOSE_MISSING')
  const r007b = await previewAndRetainReferenceGuidedRebuild(
    root,
    v2.assetVersionId,
    config.reference_class,
    config.r007b_visual_evidence,
  )
  await reportProgress('r007b_retained')
  const v3 = await waitForActiveAsset(root, v2.assetVersionId, 'QA_R007B_V3_SNAPSHOT_MISSING')
  await waitForAgentSnapshotIdle('QA_R007B_PREVIEW_NOT_CLEARED')
  await reportProgress('v3_ready')
  const confirmedLineage = readR007bLineage('confirmed')
  if (
    r007b.referenceSourceObjectSha256 !== null
    && r007b.referenceSourceObjectSha256 !== confirmedLineage.sourceObjectSha256
  ) {
    throw new Error('QA_R007B_REFERENCE_IMAGE_SOURCE_LINEAGE_MISMATCH')
  }
  if (
    r007b.referenceEvidenceId !== null
    && r007b.referenceEvidenceId !== confirmedLineage.evidenceId
  ) {
    throw new Error('QA_R007B_REFERENCE_IMAGE_EVIDENCE_LINEAGE_MISMATCH')
  }
  await closeCompletedDrawer('关闭参考证据', '[role="dialog"][aria-label="添加参考证据"]', 'QA_R007B_DRAWER_CLOSE_MISSING')
  const finalViewport = await waitForViewport('QA_V2_VIEWPORT_MISSING')
  assertProductionRenderer(finalViewport)
  const r007bResultScreenshot = config.r007b_visual_evidence
    ? await captureRenderedViewportScreenshot(finalViewport, 'r007b_result_png')
    : null
  const v3ViewportScreenshot = await captureV3ViewportScreenshot(finalViewport)
  const v3ProductionGlb = await downloadCurrentV3Glb()
  const rendererGeneration = positiveDataset(finalViewport, 'rendererGeneration')
  let r007bVisualRun: Record<string, unknown> | undefined
  if (config.r007b_visual_evidence) {
    if (!r007b.referenceScreenshot || !r007bResultScreenshot) throw new Error('QA_R007B_VISUAL_CAPTURE_MISSING')
    const coreProjection = await readR007bCoreProjection({
      projectId: requiredDataset(root, 'qaProjectId'),
      rebuildPlanId: confirmedLineage.rebuildPlanId,
      previewChangeSetId: r007b.previewChangeSetId,
      confirmedAssetVersionId: v3.assetVersionId,
    })
    r007bVisualRun = {
      ...coreProjection,
      run_id: `r007b-packaged-${config.reference_class}-${v3.assetVersionId}`,
      captured_at: new Date().toISOString(),
      workbench: {
        runtime_kind: 'packaged_tauri_webview',
        real_workbench: true,
        fixture_or_proxy_used: false,
        provider_network_calls: 0,
        credential_reads: 0,
      },
      geometry_readback: {
        ...(coreProjection.geometry_readback as Record<string, unknown>),
        triangle_count: v3ProductionGlb.triangle_count,
        glb_sha256: v3ProductionGlb.sha256,
      },
      renderer: {
        renderer_id: 'ForgeCADWorkbenchRenderer@1',
        renderer_generation: rendererGeneration,
        reference_renderer_generation: r007b.referenceRendererGeneration,
        result_renderer_generation: rendererGeneration,
        same_renderer: r007b.referenceRendererGeneration === rendererGeneration,
        canvas_count: 1,
        active_webgl_contexts: positiveDataset(finalViewport, 'activeWebglContexts'),
        load_state: finalViewport.dataset.blockoutLoadState,
      },
      screenshots: {
        reference: visualCaptureReceipt(r007b.referenceScreenshot, 'reference', r007b.referenceRendererGeneration, confirmedLineage.sourceObjectSha256, null),
        result: visualCaptureReceipt(r007bResultScreenshot, 'result', rendererGeneration, v3ProductionGlb.sha256, v3.assetVersionId),
      },
    }
  }
  await reportProgress('v3_glb_downloaded')
  await reportSuccess(config, {
    ...decision,
    v1_asset_version_id: v1.assetVersionId,
    v2_asset_version_id: v2.assetVersionId,
    v3_asset_version_id: v3.assetVersionId,
    snapshot_revision: v3.snapshotRevision,
    renderer_generation: rendererGeneration,
    active_webgl_contexts: positiveDataset(finalViewport, 'activeWebglContexts'),
    production_glb_render_source: 'glb_pbr',
    a005_preview_seen: true,
    r007b_preview_seen: r007b.previewSeen,
    r007b_v3_confirmed: r007b.retained,
    v3_glb_download_confirmed: true,
    v3_production_glb: v3ProductionGlb,
    v3_viewport_screenshot: v3ViewportScreenshot,
    // A decoded same-V3 screenshot and production GLB readback prove the
    // engineering lineage only. Semantic image quality remains M108B work.
    visual_fidelity_validated: false,
    restart_hydrated: false,
    r007b_visual_run: r007bVisualRun,
  })
}

async function closeCompletedDrawer(buttonLabel: string, drawerSelector: string, errorCode: string): Promise<void> {
  const close = await waitFor<HTMLButtonElement>(
    () => document.querySelector<HTMLButtonElement>(`button[aria-label="${buttonLabel}"]:not(:disabled)`),
    errorCode,
  )
  close.click()
  await waitForMutation<HTMLElement>(
    () => document.querySelector(drawerSelector) === null ? document.documentElement : null,
    errorCode,
  )
}

async function waitForAgentSnapshotIdle(errorCode: string): Promise<void> {
  await waitForMutation<HTMLElement>(() => {
    const readout = document.querySelector<HTMLElement>('.viewport-readout span:first-child')
    return readout?.textContent?.includes('当前视口绑定 Agent Snapshot') ? readout : null
  }, errorCode)
}

async function runRestart(config: ProbeConfig): Promise<void> {
  // Phase two intentionally does not create, edit or confirm anything.  The
  // launcher supplies the first-run expected values to Rust; Rust rejects a
  // restart report whose DOM identity drifts from that durable truth.
  const root = await waitFor<HTMLElement>(() => document.querySelector<HTMLElement>('[data-testid="cad-workbench"]'), 'QA_WORKBENCH_MISSING')
  const active = await waitForActiveAsset(root, null, 'QA_RESTART_SNAPSHOT_MISSING')
  const viewport = await waitForViewport('QA_RESTART_VIEWPORT_MISSING')
  assertProductionRenderer(viewport)
  await reportProgress('restart_hydrated')
  await reportSuccess(config, {
    project_id: requiredDataset(root, 'qaProjectId'),
    turn_id: undefined,
    preview_id: undefined,
    preview_artifact_sha256: undefined,
    v1_asset_version_id: undefined,
    v3_asset_version_id: active.assetVersionId,
    snapshot_revision: active.snapshotRevision,
    renderer_generation: positiveDataset(viewport, 'rendererGeneration'),
    active_webgl_contexts: positiveDataset(viewport, 'activeWebglContexts'),
    production_glb_render_source: 'glb_pbr',
    a005_preview_seen: false,
    r007b_preview_seen: false,
    r007b_v3_confirmed: false,
    v3_glb_download_confirmed: false,
    restart_hydrated: true,
  })
}

async function previewAndRetainSurfaceAdornment(): Promise<void> {
  let primary = await waitForMutation<HTMLButtonElement>(
    () => document.querySelector<HTMLButtonElement>('.surface-adornment-primary:not(:disabled)'),
    'QA_A005_PREVIEW_MISSING',
  )
  reportProgress('a005_primary_ready')
  // The drawer normally knows the persisted skill state.  Execute the visible
  // activation action first when present, then request the preview.  If an
  // already-enabled drawer races a server-side activation response, handle
  // that second visible activation without double-clicking the same control.
  if (primary.textContent?.includes('启用外观细节能力')) {
    primary.click()
    reportProgress('a005_primary_clicked')
    primary = await waitForMutation<HTMLButtonElement>(
      () => {
        const button = document.querySelector<HTMLButtonElement>('.surface-adornment-primary:not(:disabled)')
        return button?.textContent?.includes('预览外观细节') ? button : null
      },
      'QA_A005_PREVIEW_AFTER_ENABLE_MISSING',
    )
  }
  if (!primary.textContent?.includes('预览外观细节')) throw new Error('QA_A005_PRIMARY_ACTION_INVALID')
  primary.click()
  reportProgress('a005_primary_clicked')
  const outcome = await waitForMutation<
    { kind: 'activation' | 'retain'; button: HTMLButtonElement } | { kind: 'failed'; message: string; errorCode: string }
  >(() => {
    const button = document.querySelector<HTMLButtonElement>('.surface-adornment-primary:not(:disabled)')
    if (button?.textContent?.includes('启用外观细节能力')) return { kind: 'activation', button }
    const retain = document.querySelector<HTMLButtonElement>('.surface-adornment-actions .surface-adornment-primary:not(:disabled)')
    if (retain) return { kind: 'retain', button: retain }
    const failed = document.querySelector<HTMLElement>('[role="dialog"][aria-label="添加外观细节"] .surface-adornment-status.failed')
    return failed?.textContent ? { kind: 'failed', message: failed.textContent.trim(), errorCode: failed.getAttribute('data-error-code') ?? '' } : null
  }, 'QA_A005_OUTCOME_MISSING')
  if (outcome.kind === 'failed') {
    throw new Error(classifyA005Failure(outcome.message, outcome.errorCode))
  } else if (outcome.kind === 'activation') {
    reportProgress('a005_activation_ready')
    outcome.button.click()
    const preview = await waitForMutation<HTMLButtonElement>(() => {
      const button = document.querySelector<HTMLButtonElement>('.surface-adornment-primary:not(:disabled)')
      return button?.textContent?.includes('预览外观细节') ? button : null
    }, 'QA_A005_PREVIEW_AFTER_ENABLE_MISSING')
    preview.click()
    reportProgress('a005_preview_clicked')
  } else {
    reportProgress('a005_retain_ready')
    outcome.button.click()
    return
  }
  const finalOutcome = await waitForMutation<{ kind: 'retain'; button: HTMLButtonElement } | { kind: 'failed'; message: string; errorCode: string }>(
    () => {
      const retain = document.querySelector<HTMLButtonElement>('.surface-adornment-actions .surface-adornment-primary:not(:disabled)')
      if (retain) return { kind: 'retain', button: retain }
      const failed = document.querySelector<HTMLElement>('[role="dialog"][aria-label="添加外观细节"] .surface-adornment-status.failed')
      return failed?.textContent ? { kind: 'failed', message: failed.textContent.trim(), errorCode: failed.getAttribute('data-error-code') ?? '' } : null
    },
    'QA_A005_PREVIEW_NOT_RENDERED',
  )
  if (finalOutcome.kind === 'failed') throw new Error(classifyA005Failure(finalOutcome.message, finalOutcome.errorCode))
  reportProgress('a005_retain_ready')
  finalOutcome.button.click()
}

/**
 * Exercises the R007B path through precisely the same visible controls that
 * a user uses.  The input image exists only in this WebView: no fixture file,
 * object-store shortcut, ForgeApi import, bridge proxy, or accessibility
 * automation is involved.  It is deliberately a small non-derivative visual
 * cue, because the acceptance test proves lifecycle/lineage rather than any
 * claim of image similarity.
 */
async function previewAndRetainReferenceGuidedRebuild(
  root: HTMLElement,
  baseAssetVersionId: string,
  referenceClass: R007bReferenceClass,
  captureVisualEvidence: boolean,
): Promise<{
  previewSeen: true
  retained: true
  previewChangeSetId: string
  referenceRendererGeneration: number
  referenceScreenshot: QaPngCapture | null
  referenceSourceObjectSha256: string | null
  referenceEvidenceId: string | null
}> {
  const referenceFile = referenceClass === 'strict_glb_readback'
    ? await createCurrentC106ReferenceGlb()
    : await createR007bReferencePng(referenceClass)
  const add = await waitFor<HTMLElement>(
    () => document.querySelector<HTMLElement>('summary[aria-label="添加风格、材质或参考"]'),
    'QA_R007B_COMPOSER_ADD_MISSING',
  )
  add.click()
  await reportProgress('r007b_menu_open')
  const menu = add.closest('details')
  if (!menu) throw new Error('QA_R007B_REFERENCE_MENU_MISSING')
  // A locked/background WKWebView may suppress the native default action of
  // a synthetic summary click. Keep exercising the real trigger, then mirror
  // the same HTML details state so the real menu button/callback remains the
  // path under test rather than bypassing it.
  if (!menu.open) menu.open = true
  await waitForMutation(() => menu.open ? menu : null, 'QA_R007B_REFERENCE_MENU_NOT_OPEN')
  const openReference = menu?.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]')[2]
  if (!openReference || !openReference.textContent?.includes('参考')) {
    throw new Error('QA_R007B_REFERENCE_ACTION_MISSING')
  }
  await reportProgress(openReference.disabled ? 'r007b_reference_action_disabled' : 'r007b_reference_action_ready')
  if (openReference.disabled) throw new Error('QA_R007B_REFERENCE_ACTION_DISABLED')
  openReference.click()
  const drawer = await waitForMutation<HTMLElement>(
    () => document.querySelector<HTMLElement>('[role="dialog"][aria-label="添加参考证据"]'),
    'QA_R007B_DRAWER_MISSING',
  )
  await reportProgress('r007b_drawer_open')

  const input = drawer.querySelector<HTMLInputElement>('input[type="file"]')
  if (!input || input.disabled) throw new Error('QA_R007B_FILE_INPUT_MISSING')
  const file = referenceFile
  const transfer = new DataTransfer()
  transfer.items.add(file)
  // This is the browser's standard file-selection event path. React receives
  // the selected File through the visible input onChange handler.
  input.files = transfer.files
  input.dispatchEvent(new Event('change', { bubbles: true }))
  await waitForMutation<HTMLElement>(
    () => drawer.querySelector<HTMLElement>('.reference-evidence-file small')?.textContent?.includes(file.name)
      ? drawer : null,
    'QA_R007B_FILE_NOT_ACCEPTED',
  )
  await reportProgress('r007b_file_selected')

  const fields = drawer.querySelectorAll<HTMLTextAreaElement>('.reference-evidence-field textarea')
  if (fields.length < 2) throw new Error('QA_R007B_PROVENANCE_FIELDS_MISSING')
  setTextarea(fields[0]!, '本机 QA 在浏览器内生成的非衍生参考图。')
  setTextarea(fields[1]!, '仅用于本项目的受限概念重建验证。')
  const missingViewInputs = [...drawer.querySelectorAll<HTMLInputElement>('.reference-evidence-views input[type="checkbox"]')]
  if (referenceClass === 'single_image') {
    for (const input of missingViewInputs.slice(2)) input.click()
    if (!missingViewInputs.slice(2).every((input) => input.checked)) throw new Error('QA_R007B_MISSING_VIEWS_NOT_DECLARED')
  } else if (referenceClass === 'multi_view_contact_sheet') {
    const multiView = [...drawer.querySelectorAll<HTMLInputElement>('input[name="reference-class"]')][1]
    if (!multiView) throw new Error('QA_R007B_REFERENCE_CLASS_MISSING')
    multiView.click()
    if (!multiView.checked) throw new Error('QA_R007B_REFERENCE_CLASS_NOT_COMMITTED')
  }
  // Yield once so React flushes the controlled provenance and missing-view
  // state before the save callback from the next render is invoked. A
  // microtask is used because a background packaged WebView may throttle rAF.
  await Promise.resolve()
  const saveEvidence = await waitFor<HTMLButtonElement>(
    () => [...drawer.querySelectorAll<HTMLButtonElement>('.reference-evidence-primary')]
      .find((button) => button.textContent?.includes('保存只读参考证据') && !button.disabled) ?? null,
    'QA_R007B_SAVE_EVIDENCE_MISSING',
  )
  saveEvidence.click()
  await reportProgress('r007b_evidence_save_requested')
  const evidenceOutcome = await waitForMutation<
    { kind: 'ready'; button: HTMLButtonElement } | { kind: 'failed'; detail: string }
  >(
    () => {
      const button = [...drawer.querySelectorAll<HTMLButtonElement>('.reference-evidence-primary')]
        .find((candidate) => candidate.textContent?.includes('生成受限重建预览') && !candidate.disabled)
      if (button) return { kind: 'ready', button }
      const failed = drawer.querySelector<HTMLElement>('.reference-evidence-status.failed')
      return failed?.textContent ? { kind: 'failed', detail: failed.textContent.trim() } : null
    },
    'QA_R007B_EVIDENCE_NOT_SAVED',
  )
  if (evidenceOutcome.kind === 'failed') throw new Error(classifyR007bEvidenceFailure(evidenceOutcome.detail))
  const buildPreview = evidenceOutcome.button
  await reportProgress('r007b_evidence_saved')
  let referenceViewport: HTMLElement
  if (referenceClass === 'strict_glb_readback') {
    const viewReference = [...drawer.querySelectorAll<HTMLButtonElement>('.reference-evidence-viewport-actions button')]
      .find((button) => button.textContent?.includes('查看参考 GLB') && !button.disabled)
    if (!viewReference) throw new Error('QA_R007B_REFERENCE_GLB_VIEW_MISSING')
    viewReference.click()
    referenceViewport = await waitForMutation(() => {
      const viewport = document.querySelector<HTMLElement>('[aria-label="真实 ModuleGraph 三维视口"]')
      return viewport?.dataset.blockoutLoadState === 'ready'
        && viewport.dataset.blockoutGlbKind === 'external_reference'
        && viewport.dataset.blockoutRenderSource === 'glb_pbr'
        ? viewport
        : null
    }, 'QA_R007B_REFERENCE_GLB_VIEW_NOT_READY')
  } else {
    const viewReference = [...drawer.querySelectorAll<HTMLButtonElement>('.reference-evidence-viewport-actions button')]
      .find((button) => button.textContent?.includes('查看参考图片') && !button.disabled)
    if (!viewReference) throw new Error('QA_R007B_REFERENCE_IMAGE_VIEW_MISSING')
    viewReference.click()
    referenceViewport = await waitForMutation(() => {
      const viewport = document.querySelector<HTMLElement>('[aria-label="真实 ModuleGraph 三维视口"]')
      return viewport?.dataset.referenceImageLoadState === 'ready'
        && viewport.dataset.referenceDisplayMode === 'reference_image'
        && viewport.dataset.referenceClass === referenceClass
        && Boolean(viewport.dataset.referenceEvidenceId)
        && /^[a-f0-9]{64}$/.test(viewport.dataset.referenceSourceObjectSha256 ?? '')
        ? viewport
        : null
    }, 'QA_R007B_REFERENCE_IMAGE_VIEW_NOT_READY')
  }
  const referenceRendererGeneration = positiveDataset(referenceViewport, 'rendererGeneration')
  const referenceSourceObjectSha256 = referenceClass === 'strict_glb_readback'
    ? null
    : requiredDataset(referenceViewport, 'referenceSourceObjectSha256')
  const referenceEvidenceId = referenceClass === 'strict_glb_readback'
    ? null
    : requiredDataset(referenceViewport, 'referenceEvidenceId')
  const referenceScreenshot = captureVisualEvidence
    ? await captureRenderedViewportScreenshot(referenceViewport, 'r007b_reference_png')
    : null
  buildPreview.click()
  await reportProgress('r007b_preview_requested')
  const retain = await waitForMutation<HTMLButtonElement>(
    () => [...drawer.querySelectorAll<HTMLButtonElement>('.reference-evidence-primary')]
      .find((button) => button.textContent?.includes('保留新版本') && !button.disabled) ?? null,
    'QA_R007B_PREVIEW_NOT_READY',
  )
  // R007B must leave the single shared renderer alive while it paints the
  // editable preview.  The production PBR assertion runs again after V3.
  const previewViewport = await waitForReferencePreviewViewport()
  if (positiveDataset(previewViewport, 'activeWebglContexts') !== 1) throw new Error('QA_R007B_CONTEXT_COUNT_INVALID')
  if (requiredDataset(root, 'qaActiveAssetVersionId') !== baseAssetVersionId) {
    throw new Error('QA_R007B_PREVIEW_ADVANCED_HEAD')
  }
  const previewLineage = readR007bLineage('previewed')
  await reportProgress('r007b_preview_ready')
  retain.click()
  await reportProgress('r007b_retain_requested')
  return {
    previewSeen: true,
    retained: true,
    previewChangeSetId: previewLineage.previewChangeSetId,
    referenceRendererGeneration,
    referenceScreenshot,
    referenceSourceObjectSha256,
    referenceEvidenceId,
  }
}

function classifyR007bEvidenceFailure(detail: string): string {
  if (detail.includes('请说明参考来源')) return 'QA_R007B_PROVENANCE_NOT_COMMITTED'
  if (detail.includes('参考文件为空')) return 'QA_R007B_REFERENCE_FILE_EMPTY'
  if (detail.includes('不支持')) return 'QA_R007B_REFERENCE_MEDIA_INVALID'
  const stable = detail.match(/\b([A-Z][A-Z0-9_]{3,80})\b/)?.[1]
  return stable ? `QA_R007B_API_${stable}` : 'QA_R007B_EVIDENCE_SAVE_FAILED'
}

async function createR007bReferencePng(referenceClass: Exclude<R007bReferenceClass, 'strict_glb_readback'>): Promise<File> {
  // Canvas encoders are allowed to add ICC/profile chunks, which the
  // security boundary intentionally rejects. Generate a tiny metadata-free
  // RGBA PNG in the WebView instead: signature + IHDR + IDAT + IEND only.
  const width = 640
  const height = 480
  const scanlines = new Uint8Array(height * (1 + width * 4))
  for (let y = 0; y < height; y += 1) {
    const row = y * (1 + width * 4)
    scanlines[row] = 0
    for (let x = 0; x < width; x += 1) {
      const offset = row + 1 + x * 4
      const isContactSheet = referenceClass === 'multi_view_contact_sheet'
      const panel = isContactSheet ? Math.floor(x / 160) : 0
      const localX = isContactSheet ? x % 160 : x
      // A declared contact sheet must contain locally observable layout
      // evidence, not merely four repeated motifs touching edge-to-edge. Keep
      // a 24 px background-only divider after each 136 px panel so Rust Core's
      // conservative full-projection-gap detector can prove that visible
      // regions exist on both sides of a real separator.
      const insidePanel = !isContactSheet || localX < 136
      const jointCenterX = isContactSheet ? 88 + panel * 5 : 220
      const onFlowline = insidePanel && Math.abs(y - Math.round(390 - localX * 0.45 - panel * 24)) <= 4
      const onJoint = insidePanel && (localX - jointCenterX) ** 2 + (y - 250 + panel * 20) ** 2 <= 1_600
      scanlines[offset] = onJoint ? 216 : onFlowline ? 74 : 16
      scanlines[offset + 1] = onJoint ? 231 : onFlowline ? 163 : 26
      scanlines[offset + 2] = onJoint ? 255 : onFlowline ? 255 : 43
      scanlines[offset + 3] = 255
    }
  }
  const compressed = new Uint8Array(await new Response(
    new Blob([scanlines]).stream().pipeThrough(new CompressionStream('deflate')),
  ).arrayBuffer())
  const ihdr = new Uint8Array(13)
  const view = new DataView(ihdr.buffer)
  view.setUint32(0, width)
  view.setUint32(4, height)
  ihdr.set([8, 6, 0, 0, 0], 8)
  const bytes = concatBytes(
    new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]),
    pngChunk('IHDR', ihdr),
    pngChunk('IDAT', compressed),
    pngChunk('IEND', new Uint8Array()),
  )
  return new File([bytes.buffer as ArrayBuffer], `forgecad-r007b-${referenceClass}.png`, { type: 'image/png', lastModified: 0 })
}

async function createCurrentC106ReferenceGlb(): Promise<File> {
  const blob = await readCurrentGlbBlobFromVisibleExport(true)
  return new File([await blob.arrayBuffer()], 'forgecad-r007b-strict-c106.glb', {
    type: 'model/gltf-binary',
    lastModified: 0,
  })
}

function pngChunk(kind: string, data: Uint8Array): Uint8Array {
  const type = new TextEncoder().encode(kind)
  const chunk = new Uint8Array(12 + data.length)
  const view = new DataView(chunk.buffer)
  view.setUint32(0, data.length)
  chunk.set(type, 4)
  chunk.set(data, 8)
  view.setUint32(8 + data.length, crc32(concatBytes(type, data)))
  return chunk
}

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff
  for (const byte of bytes) {
    crc ^= byte
    for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1))
  }
  return (crc ^ 0xffffffff) >>> 0
}

function concatBytes(...parts: Uint8Array[]): Uint8Array {
  const output = new Uint8Array(parts.reduce((total, part) => total + part.length, 0))
  let offset = 0
  for (const part of parts) {
    output.set(part, offset)
    offset += part.length
  }
  return output
}

async function captureV3ViewportScreenshot(viewport: HTMLElement): Promise<QaPngCapture> {
  return captureRenderedViewportScreenshot(viewport, 'v3_viewport_png')
}

async function captureRenderedViewportScreenshot(
  viewport: HTMLElement,
  kind: 'v3_viewport_png' | 'r007b_reference_png' | 'r007b_result_png',
): Promise<QaPngCapture> {
  // ModuleGraphViewport owns the only renderer/context.  Request its pixels
  // through the QA-only same-realm event so readPixels runs immediately after
  // renderer.render(), before a browser may discard the default framebuffer.
  const { width, height, pixels } = await requestRenderedViewportPixels(viewport)
  if (width < 320 || height < 240 || width * height > 8_400_000) {
    throw new Error('QA_V3_VIEWPORT_SCREENSHOT_CANVAS_INVALID')
  }
  const raster = document.createElement('canvas')
  raster.width = width
  raster.height = height
  const context = raster.getContext('2d', { willReadFrequently: true })
  if (!context) throw new Error('QA_V3_VIEWPORT_SCREENSHOT_CONTEXT_MISSING')
  const image = context.createImageData(width, height)
  const rowBytes = width * 4
  for (let sourceY = 0; sourceY < height; sourceY += 1) {
    const sourceOffset = sourceY * rowBytes
    const targetOffset = (height - sourceY - 1) * rowBytes
    image.data.set(pixels.subarray(sourceOffset, sourceOffset + rowBytes), targetOffset)
  }
  context.putImageData(image, 0, 0)
  assertVisibleRaster(context, raster.width, raster.height)
  const blob = await new Promise<Blob | null>((resolve) => raster.toBlob(resolve, 'image/png'))
  if (!blob || blob.size === 0) throw new Error('QA_V3_VIEWPORT_SCREENSHOT_UNAVAILABLE')
  const receipt = await captureQaBinary(kind, blob)
  if (!isPngCapture(receipt, kind) || receipt.width !== width || receipt.height !== height) {
    throw new Error('QA_V3_VIEWPORT_SCREENSHOT_CAPTURE_INVALID')
  }
  return receipt
}

function requestRenderedViewportPixels(viewport: HTMLElement): Promise<QaViewportPixels> {
  if (!viewport.isConnected) return Promise.reject(new Error('QA_V3_VIEWPORT_SCREENSHOT_VIEWPORT_DISPOSED'))
  return new Promise<QaViewportPixels>((resolve, reject) => {
    let settled = false
    const finish = (callback: () => void) => {
      if (settled) return
      settled = true
      window.clearTimeout(timeout)
      callback()
    }
    const timeout = window.setTimeout(() => {
      finish(() => reject(new Error('QA_V3_VIEWPORT_SCREENSHOT_RENDER_TIMEOUT')))
    }, QA_VIEWPORT_CAPTURE_TIMEOUT_MS)
    const request: QaViewportCaptureRequest = {
      viewport,
      resolve: (capture) => finish(() => {
        if (!Number.isInteger(capture.width) || !Number.isInteger(capture.height) || capture.width <= 0 || capture.height <= 0) {
          reject(new Error('QA_V3_VIEWPORT_SCREENSHOT_CAPTURE_INVALID'))
          return
        }
        if (!(capture.pixels instanceof Uint8Array) || capture.pixels.byteLength !== capture.width * capture.height * 4) {
          reject(new Error('QA_V3_VIEWPORT_SCREENSHOT_CAPTURE_INVALID'))
          return
        }
        resolve(capture)
      }),
      reject: (error) => finish(() => reject(error instanceof Error ? error : new Error('QA_V3_VIEWPORT_SCREENSHOT_READBACK_FAILED'))),
    }
    viewport.dispatchEvent(new CustomEvent<QaViewportCaptureRequest>(QA_VIEWPORT_CAPTURE_EVENT, { detail: request }))
  })
}

function assertVisibleRaster(context: CanvasRenderingContext2D, width: number, height: number): void {
  // Sample the whole viewport, not only its top-left studio background.  The
  // mechanical arm is intentionally framed near the centre and a uniform
  // corner is valid.  A bounded 64×64 downsample still rejects transparent or
  // flat captures without allocating another full-resolution pixel buffer.
  const sampleCanvas = document.createElement('canvas')
  sampleCanvas.width = Math.min(width, 64)
  sampleCanvas.height = Math.min(height, 64)
  const sampleContext = sampleCanvas.getContext('2d', { willReadFrequently: true })
  if (!sampleContext) throw new Error('QA_V3_VIEWPORT_SCREENSHOT_CONTEXT_MISSING')
  sampleContext.drawImage(context.canvas, 0, 0, sampleCanvas.width, sampleCanvas.height)
  const sample = sampleContext.getImageData(0, 0, sampleCanvas.width, sampleCanvas.height).data
  let opaque = 0
  let minLuma = 255
  let maxLuma = 0
  for (let offset = 0; offset < sample.length; offset += 4) {
    if (sample[offset + 3]! > 0) opaque += 1
    const luma = Math.round(
      sample[offset]! * 0.2126 + sample[offset + 1]! * 0.7152 + sample[offset + 2]! * 0.0722,
    )
    minLuma = Math.min(minLuma, luma)
    maxLuma = Math.max(maxLuma, luma)
  }
  if (opaque < (sample.length / 4) * 0.9 || maxLuma - minLuma < 4) {
    throw new Error('QA_V3_VIEWPORT_SCREENSHOT_BLANK')
  }
}

async function downloadCurrentV3Glb(): Promise<QaGlbCapture> {
  // The production action itself creates the download Blob. Observe that
  // exact Blob at the standard URL.createObjectURL boundary rather than
  // invoking ForgeApi or a second hidden request. This preserves the same
  // visible user path while allowing Rust to validate and preserve the final
  // V3 bytes as QA evidence.
  const downloadedBlob = await readCurrentGlbBlobFromVisibleExport(false)
  const receipt = await captureQaBinary('v3_production_glb', downloadedBlob)
  if (!isGlbCapture(receipt)) throw new Error('QA_V3_GLB_READBACK_INVALID')
  return receipt
}

async function readCurrentGlbBlobFromVisibleExport(closeDrawer: boolean): Promise<Blob> {
  const originalCreateObjectUrl = URL.createObjectURL.bind(URL)
  let downloadedBlob: Blob | null = null
  URL.createObjectURL = (value: Blob | MediaSource): string => {
    if (value instanceof Blob) downloadedBlob = value
    return originalCreateObjectUrl(value)
  }
  try {
    await clickVisibleV3GlbDownload()
    // The byte-producing browser boundary is the download truth. UI copy can
    // change independently and older conversation messages may remain in the
    // DOM, but the real visible button must create the exact Blob that Rust
    // subsequently validates and seals.
    downloadedBlob = await waitFor<Blob>(() => downloadedBlob, 'QA_V3_GLB_DOWNLOAD_BLOB_MISSING')
  } finally {
    URL.createObjectURL = originalCreateObjectUrl
  }
  if (closeDrawer) {
    const close = document.querySelector<HTMLButtonElement>('button[aria-label="关闭导出"]')
    if (!close) throw new Error('QA_V3_EXPORT_DRAWER_CLOSE_MISSING')
    close.click()
    await waitForMutation(() => document.querySelector('[data-forgecad-drawer="export"]') === null ? document.documentElement : null, 'QA_V3_EXPORT_DRAWER_CLOSE_FAILED')
  }
  return downloadedBlob
}

async function clickVisibleV3GlbDownload(): Promise<void> {
  const openExport = await waitFor<HTMLButtonElement>(
    () => document.querySelector<HTMLButtonElement>('button[aria-label="导出"]:not(:disabled)'),
    'QA_V3_EXPORT_ACTION_MISSING',
  )
  openExport.click()
  await reportProgress('v3_export_open_requested')
  const drawer = await waitForMutation<HTMLElement>(
    () => document.querySelector<HTMLElement>('[role="dialog"][data-forgecad-drawer="export"]'),
    'QA_V3_EXPORT_DRAWER_MISSING',
  )
  await reportProgress('v3_export_drawer_ready')
  const download = await waitFor<HTMLButtonElement>(
    () => [...drawer.querySelectorAll<HTMLButtonElement>('button')]
      .find((button) => button.textContent?.includes('下载 3D 模型 (GLB)') && !button.disabled) ?? null,
    'QA_V3_GLB_DOWNLOAD_ACTION_MISSING',
  )
  download.click()
  await reportProgress('v3_export_download_clicked')
}

async function captureQaBinary(
  kind: QaCaptureRequest['kind'],
  blob: Blob,
): Promise<QaPngCapture | QaGlbCapture> {
  const { invoke } = await import('@tauri-apps/api/core')
  const bytes = new Uint8Array(await blob.arrayBuffer())
  const receipt = await invoke<QaPngCapture | QaGlbCapture>('forgecad_arm_webview_qa_capture', {
    capture: {
      schema_version: PROBE_SCHEMA,
      phase: 'initial',
      kind,
      bytes_base64: encodeBase64(bytes),
    } satisfies QaCaptureRequest,
  })
  if (!receipt || !SHA256_PATTERN.test(receipt.sha256) || !Number.isInteger(receipt.byte_size) || receipt.byte_size !== bytes.byteLength) {
    throw new Error('QA_CAPTURE_RECEIPT_INVALID')
  }
  return receipt
}

function encodeBase64(bytes: Uint8Array): string {
  // Avoid spreading a multi-megabyte GLB into one JavaScript call frame.
  const chunkSize = 0x8000
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize))
  }
  return btoa(binary)
}

function isPngCapture(
  value: QaPngCapture | QaGlbCapture,
  kind: 'v3_viewport_png' | 'r007b_reference_png' | 'r007b_result_png',
): value is QaPngCapture {
  const pathValid = kind === 'v3_viewport_png'
    ? value.relative_path === 'qa-artifacts/arm-webview/initial/v3_viewport_png.png'
    : value.relative_path.endsWith(kind === 'r007b_reference_png' ? '/reference.png' : '/result.png')
  return 'width' in value
    && pathValid
    && Number.isInteger(value.width)
    && Number.isInteger(value.height)
    && value.width >= 320
    && value.height >= 240
}

function readR007bLineage(expectedStatus: 'previewed' | 'confirmed'): QaR007bLineage {
  const element = document.querySelector<HTMLElement>('[aria-label="参考重建证据谱系"]')
  if (!element || element.dataset.qaLineageStatus !== expectedStatus) throw new Error('QA_R007B_LINEAGE_MISSING')
  const evidenceId = requiredDataset(element, 'qaEvidenceId')
  const sourceObjectSha256 = requiredDataset(element, 'qaSourceObjectSha256')
  const rebuildPlanId = requiredDataset(element, 'qaRebuildPlanId')
  const analysisId = requiredDataset(element, 'qaAnalysisId')
  const fidelityCeiling = requiredDataset(element, 'qaFidelityCeiling')
  const previewChangeSetId = requiredDataset(element, 'qaPreviewChangeSetId')
  const confirmedAssetVersionId = element.dataset.qaConfirmedAssetVersionId || null
  const resultGlbSha256 = element.dataset.qaResultGlbSha256 || null
  if (
    ![evidenceId, rebuildPlanId, analysisId, previewChangeSetId].every((value) => STABLE_ID_PATTERN.test(value))
    || !SHA256_PATTERN.test(sourceObjectSha256)
    || (expectedStatus === 'confirmed' && (!confirmedAssetVersionId || !resultGlbSha256 || !SHA256_PATTERN.test(resultGlbSha256)))
  ) throw new Error('QA_R007B_LINEAGE_INVALID')
  return { evidenceId, sourceObjectSha256, rebuildPlanId, analysisId, fidelityCeiling, previewChangeSetId, confirmedAssetVersionId, resultGlbSha256 }
}

async function readR007bCoreProjection(input: {
  projectId: string
  rebuildPlanId: string
  previewChangeSetId: string
  confirmedAssetVersionId: string
}): Promise<Record<string, unknown>> {
  const { invoke } = await import('@tauri-apps/api/core')
  const value = await invoke<Record<string, unknown>>('forgecad_arm_webview_qa_r007b_lineage', {
    request: {
      schema_version: PROBE_SCHEMA,
      project_id: input.projectId,
      rebuild_plan_id: input.rebuildPlanId,
      preview_change_set_id: input.previewChangeSetId,
      confirmed_asset_version_id: input.confirmedAssetVersionId,
    },
  })
  if (!value || value.reference_class === undefined || typeof value.geometry_readback !== 'object') {
    throw new Error('QA_R007B_CORE_PROJECTION_INVALID')
  }
  return value
}

function visualCaptureReceipt(
  capture: QaPngCapture,
  captureKind: 'reference' | 'result',
  rendererGeneration: number,
  lineageSha256: string,
  assetVersionId: string | null,
): Record<string, unknown> {
  return {
    capture_kind: captureKind,
    relative_path: capture.relative_path,
    sha256: capture.sha256,
    byte_size: capture.byte_size,
    width: capture.width,
    height: capture.height,
    renderer_generation: rendererGeneration,
    lineage_sha256: lineageSha256,
    asset_version_id: assetVersionId,
    displayed_reference_kind: captureKind === 'reference' ? 'same_renderer_read_only_reference' : 'production_result',
  }
}

function isGlbCapture(value: QaPngCapture | QaGlbCapture): value is QaGlbCapture {
  return 'triangle_count' in value
    && value.relative_path === 'qa-artifacts/arm-webview/initial/v3_production_glb.glb'
    && Number.isInteger(value.triangle_count)
    && value.triangle_count >= 12_000
    && value.triangle_count <= 24_000
    && Number.isInteger(value.complete_pbr_material_count)
    && value.complete_pbr_material_count > 0
}

async function waitForReferencePreviewViewport(): Promise<HTMLElement> {
  return waitForMutation(() => {
    const viewport = document.querySelector<HTMLElement>('[aria-label="真实 ModuleGraph 三维视口"]')
    if (!viewport) return null
    if (viewport.dataset.blockoutLoadState === 'failed') throw new Error(classifyViewportLoadFailure(viewport))
    return viewport.dataset.blockoutLoadState === 'ready'
      && viewport.dataset.blockoutGlbKind === 'compiled_agent_preview_pbr'
      && viewport.dataset.blockoutRenderSource === 'glb_pbr'
      ? viewport
      : null
  }, 'QA_R007B_PREVIEW_VIEWPORT_MISSING')
}

function classifyA005Failure(message: string, errorCode = ''): string {
  if (/^[A-Z0-9_]{1,80}$/.test(errorCode)) return `QA_A005_API_${errorCode}`
  if (message.includes('当前项目已切换')) return 'QA_A005_PROJECT_SWITCHED'
  if (message.includes('没有返回可验证模型')) return 'QA_A005_PREVIEW_MODEL_MISSING'
  if (message.includes('GLB 与当前模型版本不一致')) return 'QA_A005_GLB_VERSION_MISMATCH'
  if (message.includes('已被更新的请求取代')) return 'QA_A005_PREVIEW_SUPERSEDED'
  if (/未选择|不存在|部件|材质区/.test(message)) return 'QA_A005_TARGET_INVALID'
  if (/版本|谱系|Snapshot/i.test(message)) return 'QA_A005_LINEAGE_INVALID'
  if (/GLB|PBR|编译|预览/.test(message)) return 'QA_A005_MODEL_PREVIEW_FAILED'
  if (/启用|能力|Skill/i.test(message)) return 'QA_A005_ACTIVATION_FAILED'
  return 'QA_A005_PREVIEW_FAILED'
}

function readVisibleDecision(root: HTMLElement): Pick<ProbeReport, 'project_id' | 'turn_id' | 'preview_id' | 'preview_artifact_sha256'> | null {
  const project_id = root.dataset.qaProjectId
  const turn_id = root.dataset.qaSingleResultTurnId
  const preview_id = root.dataset.qaSingleResultPreviewId
  const preview_artifact_sha256 = root.dataset.qaSingleResultArtifactSha256
  if (!project_id || !turn_id || !preview_id || !preview_artifact_sha256) return null
  if (![project_id, turn_id, preview_id].every((value) => STABLE_ID_PATTERN.test(value)) || !SHA256_PATTERN.test(preview_artifact_sha256)) return null
  if (root.dataset.qaSingleResultProfile !== 'production_concept') return null
  return { project_id, turn_id, preview_id, preview_artifact_sha256 }
}

async function waitForProject(root: HTMLElement): Promise<void> {
  // Project bootstrap is asynchronous. Missing data while React is still
  // hydrating is a normal pending state, not a malformed DOM fact. Reserve
  // requiredDataset() for facts that must already exist at report time.
  await waitFor(() => root.dataset.qaProjectId || null, 'QA_PROJECT_NOT_READY')
}

async function waitForActiveAsset(root: HTMLElement, previous: string | null, errorCode: string): Promise<{ assetVersionId: string; snapshotRevision: number }> {
  return waitForMutation(() => {
    const assetVersionId = root.dataset.qaActiveAssetVersionId
    const snapshotRevision = Number(root.dataset.qaActiveSnapshotRevision)
    if (!assetVersionId || !STABLE_ID_PATTERN.test(assetVersionId) || !Number.isInteger(snapshotRevision) || snapshotRevision <= 0) return null
    if (previous && assetVersionId === previous) return null
    return { assetVersionId, snapshotRevision }
  }, errorCode)
}

async function waitForViewport(errorCode: string): Promise<HTMLElement> {
  // Record entry before any polling or GLB readiness assertion.  This is a
  // bounded native heartbeat, not a viewport substitute: it distinguishes a
  // stale packaged frontend or main-thread stall from a real renderer timeout.
  await reportProgress('viewport_wait_started')
  const readViewport = () => document.querySelector<HTMLElement>('[aria-label="真实 ModuleGraph 三维视口"]')
  const classify = (viewport: HTMLElement | null): HTMLElement | Error | null => {
    if (!viewport) return null
    if (viewport.dataset.blockoutLoadState === 'failed') {
      reportProgress('viewport_load_failed')
      return new Error(classifyViewportLoadFailure(viewport))
    }
    if (
      viewport.dataset.blockoutLoadState === 'ready'
      && viewport.dataset.blockoutRenderSource === 'glb_pbr'
      && viewport.dataset.blockoutGlbKind === 'compiled_agent_production_pbr'
    ) return viewport
    const glbKind = viewport.dataset.blockoutGlbKind ?? ''
    const loadState = viewport.dataset.blockoutLoadState ?? ''
    // Hydration deliberately renders the sealed lightweight PBR preview first
    // and replaces it with the same asset version's production profile.  A
    // ready preview is therefore a pending transition, not evidence that the
    // final viewport used the wrong profile.
    if (
      !['', 'none', 'compiled_agent_preview_pbr', 'compiled_agent_production_pbr'].includes(glbKind)
      || !['empty', 'loading', 'ready'].includes(loadState)
    ) {
      reportProgress('viewport_profile_invalid')
      return new Error(viewportProfileErrorCode(viewport))
    }
    return null
  }
  const immediate = classify(readViewport())
  if (immediate instanceof Error) throw immediate
  if (immediate) return immediate
  const pendingViewport = readViewport()
  if (pendingViewport) {
    const kind = pendingViewport.dataset.blockoutGlbKind ?? 'none'
    const state = pendingViewport.dataset.blockoutLoadState ?? 'empty'
    const source = pendingViewport.dataset.blockoutRenderSource ?? 'empty'
    if (kind === 'compiled_agent_preview_pbr' && state === 'ready') {
      reportProgress('viewport_pending_preview_ready')
    } else if (kind === 'compiled_agent_production_pbr' && state === 'loading') {
      reportProgress('viewport_pending_production_loading')
    } else if (kind === 'compiled_agent_production_pbr' && state === 'ready' && source !== 'glb_pbr') {
      reportProgress('viewport_pending_production_source')
    } else if (kind === 'none' || state === 'empty') {
      reportProgress('viewport_pending_empty')
    } else {
      reportProgress('viewport_pending_other')
    }
  }

  // The native QA run is intentionally allowed while the macOS session is
  // locked. WebKit can throttle background setTimeout polling indefinitely,
  // whereas the real React viewport publishes its load state through DOM
  // attributes. Observe those attributes directly. Keep a local deadline as
  // well as the launcher's independent outer deadline so a missed or missing
  // load-state publication cannot leave this promise pending forever.
  return new Promise<HTMLElement>((resolve, reject) => {
    let settled = false
    let deadline: ReturnType<typeof setTimeout> | null = null
    const settle = (next: HTMLElement | Error) => {
      if (settled) return
      settled = true
      observer.disconnect()
      if (deadline !== null) clearTimeout(deadline)
      if (next instanceof Error) reject(next)
      else resolve(next)
    }
    const observer = new MutationObserver(() => {
      const next = classify(readViewport())
      if (next === null) return
      settle(next)
    })
    observer.observe(document.documentElement, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['data-blockout-load-state', 'data-blockout-render-source', 'data-blockout-glb-kind'],
    })
    deadline = setTimeout(() => settle(new Error(errorCode)), 180_000)

    // Close the read-before-observe TOCTOU window. If React published the only
    // loading -> ready mutation between the initial classify() and observe(),
    // this post-observe read sees the final state without waiting for another
    // mutation that may never arrive.
    const afterObserve = classify(readViewport())
    if (afterObserve !== null) settle(afterObserve)
  }).catch((error) => {
    if (error instanceof Error) throw error
    throw new Error(errorCode)
  })
}

function viewportProfileErrorCode(viewport: HTMLElement): string {
  const token = (value: string | undefined) => (value || 'missing')
    .toUpperCase()
    .replace(/[^A-Z0-9]+/g, '_')
    .slice(0, 36)
  return [
    'QA_VIEWPORT_PROFILE',
    token(viewport.dataset.blockoutGlbKind),
    token(viewport.dataset.blockoutLoadState),
    token(viewport.dataset.blockoutRenderSource),
  ].join('_')
}

function classifyViewportLoadFailure(viewport: HTMLElement): string {
  // The renderer already shows its loader error in the adjacent status line.
  // Convert it to a bounded stable code instead of sending arbitrary WebKit or
  // GLTFLoader text across the native bridge or writing it to supervisor logs.
  const message = viewport.parentElement
    ?.querySelector<HTMLElement>('.viewport-data-state.blockout-failed span')
    ?.textContent
    ?.trim()
    ?? ''
  const pbrGap = message.match(/\[(PBR_[A-Z_]+)\]/)?.[1]
  if (pbrGap) return `QA_VIEWPORT_LOAD_${pbrGap}`
  if (message.includes('完整 PBR 纹理材质')) return 'QA_VIEWPORT_LOAD_PBR_INCOMPLETE'
  if (message.includes('没有 scene')) return 'QA_VIEWPORT_LOAD_SCENE_MISSING'
  if (/texture|image|bitmap|blob/i.test(message)) return 'QA_VIEWPORT_LOAD_TEXTURE_DECODE_FAILED'
  if (/buffer|accessor|mesh|gltf|glb/i.test(message)) return 'QA_VIEWPORT_LOAD_GLTF_PARSE_FAILED'
  return 'QA_VIEWPORT_LOAD_FAILED'
}

function assertProductionRenderer(viewport: HTMLElement): void {
  if (viewport.dataset.blockoutGlbKind !== 'compiled_agent_production_pbr') {
    throw new Error('QA_VIEWPORT_PROFILE_INVALID')
  }
  if (viewport.dataset.blockoutLoadState !== 'ready') {
    throw new Error('QA_VIEWPORT_LOAD_STATE_INVALID')
  }
  if (viewport.dataset.blockoutRenderSource !== 'glb_pbr') {
    throw new Error('QA_VIEWPORT_RENDER_SOURCE_INVALID')
  }
  if (positiveDataset(viewport, 'activeWebglContexts') !== 1) {
    throw new Error('QA_VIEWPORT_CONTEXT_COUNT_INVALID')
  }
  if (positiveDataset(viewport, 'rendererGeneration') < 1) {
    throw new Error('QA_VIEWPORT_GENERATION_INVALID')
  }
}

function setTextarea(textarea: HTMLTextAreaElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set
  if (!setter) throw new Error('QA_TEXTAREA_SETTER_MISSING')
  setter.call(textarea, value)
  textarea.dispatchEvent(new Event('input', { bubbles: true }))
}

function requiredDataset(element: HTMLElement, key: keyof DOMStringMap): string {
  const value = element.dataset[key]
  if (!value) throw new Error('QA_DOM_FACT_MISSING')
  return value
}

function positiveDataset(element: HTMLElement, key: keyof DOMStringMap): number {
  const number = Number(element.dataset[key])
  if (!Number.isInteger(number) || number <= 0) throw new Error('QA_RENDERER_FACT_INVALID')
  return number
}

async function waitFor<T>(reader: () => T | null, errorCode: string): Promise<T> {
  const deadline = Date.now() + MAX_WAIT_MS
  while (Date.now() < deadline) {
    const value = reader()
    if (value !== null) return value
    await new Promise<void>((resolve) => window.setTimeout(resolve, POLL_MS))
  }
  throw new Error(errorCode)
}

async function waitForMutation<T>(reader: () => T | null, errorCode: string): Promise<T> {
  const immediate = reader()
  if (immediate !== null) return immediate
  return new Promise<T>((resolve, reject) => {
    let settled = false
    const observer = new MutationObserver(check)
    const interval = window.setInterval(check, POLL_MS)
    const timeout = window.setTimeout(() => {
      finish()
      reject(new Error(errorCode))
    }, MAX_WAIT_MS)
    function finish(): void {
      if (settled) return
      settled = true
      observer.disconnect()
      window.clearInterval(interval)
      window.clearTimeout(timeout)
    }
    function check(): void {
      if (settled) return
      const value = reader()
      if (value === null) return
      finish()
      resolve(value)
    }
    observer.observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true })
    // Close the race between the immediate read above and observer setup.
    check()
  })
}

async function reportSuccess(config: ProbeConfig, evidence: Omit<ProbeReport, 'schema_version' | 'phase' | 'ok' | 'error_code'>): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('forgecad_arm_webview_qa_report', {
    report: { schema_version: PROBE_SCHEMA, phase: config.phase, ok: true, ...evidence } satisfies ProbeReport,
  })
}

async function reportFailure(phase: ProbeConfig['phase'], error_code: string): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('forgecad_arm_webview_qa_report', {
    report: { schema_version: PROBE_SCHEMA, phase, ok: false, error_code } satisfies ProbeReport,
  }).catch(() => undefined)
}

function reportProgress(stage: string): void {
  // Progress is diagnostic only.  It must never hold the real DOM/renderer
  // acceptance chain hostage if a WebKit IPC acknowledgement is delayed after
  // native logging.  Success and failure reports remain awaited and validated
  // by Rust; this call has no authority to pass the gate.
  void import('@tauri-apps/api/core')
    .then(({ invoke }) => invoke('forgecad_arm_webview_qa_progress', { stage }))
    .catch(() => undefined)
}
