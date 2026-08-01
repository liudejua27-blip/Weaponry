import { isTauriRuntime } from '../tauri/agentSupervisor.js'
import { appServerTransport } from './appServerTransport.js'
import {
  assertC111bSelectionCardIdentity,
  normalizeC111bAdornmentErrorCode,
  normalizeC111bAdornmentRetainErrorCode,
  normalizeC111bStageError,
  resolveC111bLinkCandidate,
} from './packagedC111BWebglQaLogic.js'
import {
  WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX,
  WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX,
  WORKBENCH_PBR_CAPTURE_HEIGHT_PX,
  WORKBENCH_PBR_CAPTURE_WIDTH_PX,
  WORKBENCH_PBR_RENDER_MANIFEST_SHA256,
  WORKBENCH_PBR_RENDERER_ID,
  WORKBENCH_PBR_VISUAL_ENVIRONMENT_ID,
  WORKBENCH_PBR_VISUAL_ENVIRONMENT_SHA256,
  type WorkbenchPbrAuxiliaryPass,
} from '../../features/cad-workbench/workbenchPbrCapture.js'

const SCHEMA = 'C111BPackagedWebGL@1' as const
const MAX_WAIT_MS = 180_000
const POLL_MS = 120
const CAPTURE_EVENT = 'forgecad:qa-capture-viewport@1'
const CAMERA_EVENT = 'forgecad:qa-set-camera-view@1'
const LIGHT_EVENT = 'forgecad:qa-set-light-preset@1'
const SHA256_PATTERN = /^[a-f0-9]{64}$/
const STABLE_ID_PATTERN = /^[A-Za-z0-9_.-]{1,160}$/
const BASE_PRODUCTION_SHA256 = '48ccc5c6a725936d43cb731ed5e20b93f10ef751712ed79469ea406318160b6b'
const AGENT_V2_MATERIAL_COUNT = 14
const FIXED_VIEWS = ['iso', 'front', 'back', 'left', 'right', 'top', 'gripper_iso', 'gripper_front'] as const
type FixedView = typeof FIXED_VIEWS[number]

type Config = {
  schema_version: typeof SCHEMA
  phase: 'initial' | 'restart'
  mode: 'external_reference' | 'agent_asset'
  source_sha256: string
  triangle_count: number
  primitive_count: number
  material_count: number
  expected_project_id?: string
  expected_asset_version_id?: string
  expected_snapshot_revision?: number
  expected_export_sha256?: string
}

type Source = {
  schema_version: typeof SCHEMA
  file_name: 'c111b-production.glb'
  sha256: string
  byte_size: number
  triangle_count: number
  primitive_count: number
  material_count: number
  complete_pbr_material_count: number
  bytes_base64?: string
}

type CaptureReceipt = {
  view_id: string
  relative_path: string
  sha256: string
  byte_size: number
  width: number
  height: number
  source_sha256: string
  auxiliary_relative_path: string
  auxiliary_sha256: string
  auxiliary_byte_size: number
  auxiliary_width: number
  auxiliary_height: number
  auxiliary_pass_ids: readonly WorkbenchPbrAuxiliaryPass[]
}

type RasterReadability = {
  pixel_encoding: 'display_srgb'
  display_transfer: 'wkwebview_linear_lit_surface_to_srgb'
  sample_pixel_count: number
  foreground_pixel_count: number
  foreground_coverage_bps: number
  foreground_median_luma: number
  foreground_readable_bps: number
  background_rgb: [number, number, number]
}

type Capture = CaptureReceipt & {
  readability: RasterReadability
}

type Readback = {
  schema_version: typeof SCHEMA
  project_id: string
  asset_version_id: string
  source_sha256: string
  shape_program_schema: 'ExternalGLBReference@1' | 'ShapeProgram@1'
  external_reference: boolean
  glb_byte_size?: number
  glb_triangle_count?: number
  glb_primitive_count?: number
  glb_material_count?: number
}

type Report = {
  schema_version: typeof SCHEMA
  phase: 'initial' | 'restart'
  ok: boolean
  project_id?: string
  asset_version_id?: string
  snapshot_revision?: number
  source_sha256?: string
  triangle_count?: number
  primitive_count?: number
  material_count?: number
  complete_pbr_material_count?: number
  renderer_generation?: number
  active_webgl_contexts?: number
  canvas_count?: number
  blockout_glb_kind?: string
  render_source?: string
  light_preset?: string
  renderer_id?: typeof WORKBENCH_PBR_RENDERER_ID
  render_manifest_sha256?: typeof WORKBENCH_PBR_RENDER_MANIFEST_SHA256
  visual_environment_id?: typeof WORKBENCH_PBR_VISUAL_ENVIRONMENT_ID
  visual_environment_sha256?: typeof WORKBENCH_PBR_VISUAL_ENVIRONMENT_SHA256
  output_color_space?: 'srgb'
  tone_mapping?: 'aces_filmic'
  pbr_texture_count?: number
  pbr_color_spaces?: 'valid'
  pbr_sampling_valid?: 'true'
  captures?: Capture[]
  readback?: Readback
  formal_eligible?: false
  human_benchmark_evidence?: false
  reference_comparison?: false
  thread_id?: string
  turn_id?: string
  provider_protocol_requests?: number
  product_tool_calls?: number
  input_tokens?: number
  output_tokens?: number
  prompt_cache_hit_tokens?: number
  prompt_cache_miss_tokens?: number
  same_intent_repair_attempts?: number
  same_intent_repairs_applied?: number
  provider_schema_repair_requests?: number
  product_tool_schema_repair_requests?: number
  estimated_cost_microusd?: number
  billable_variable_cost_microusd?: number
  billable_variable_cost_source?: 'native_offline_no_billable_transport' | 'native_no_agent_provider_path'
  turn_total_elapsed_ms?: number
  turn_phase_timings_ms?: Record<string, number>
  turn_trace_sha256?: string
  turn_metrics_source?: 'rust_terminal_turn_readback' | 'native_no_turn_on_restart'
  network_provider_calls?: number
  network_call_made?: boolean
  credential_reads?: number
  provider_metrics_source?: 'rust_terminal_turn_plus_native_local_mvp_counter' | 'native_local_mvp_atomic_counter' | 'native_no_agent_provider_path'
  credential_metrics_source?: 'native_structural_no_credential_source' | 'native_no_agent_provider_path'
  end_to_end_elapsed_ms?: number
  stage_timings?: Array<{ stage: string; elapsed_ms: number; duration_since_previous_ms: number }>
  timing_metrics_source?: 'native_monotonic_progress_receipts'
  restart_hydrated?: boolean
  error_code?: string
}

type ViewportPixels = {
  width: number
  height: number
  pixels: Uint8Array
  origin: 'top_left'
  auxiliaryPixels: Uint8Array
  auxiliaryWidth: number
  auxiliaryHeight: number
  auxiliaryPassIds: readonly WorkbenchPbrAuxiliaryPass[]
}

type ViewportCaptureRequest = {
  viewport: HTMLElement
  resolve: (capture: ViewportPixels) => void
  reject: (error: Error) => void
}

let runPromise: Promise<void> | null = null

/**
 * Opt-in exact-asset C111B evidence for the real packaged workbench.
 *
 * This remains inert in normal launches. It imports the exact GLB by placing
 * bytes into the same visible file input used by a user, then captures pixels
 * from the existing ModuleGraph renderer through its QA-only bridge. It does
 * not call ForgeApi from JavaScript, create a renderer, or claim visual
 * similarity/human approval.
 */
export function runPackagedC111BWebglQaOnce(): Promise<void> {
  if (!isTauriRuntime()) return Promise.resolve()
  if (!runPromise) runPromise = runPackagedC111BWebglQa()
  return runPromise
}

async function runPackagedC111BWebglQa(): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  let config: Config | null
  try {
    config = await invoke<Config | null>('forgecad_c111b_webview_qa_config')
  } catch {
    return
  }
  if (config === null) return
  if (
    config.schema_version !== SCHEMA
    || !['initial', 'restart'].includes(config.phase)
    || !['external_reference', 'agent_asset'].includes(config.mode)
    || config.source_sha256 !== BASE_PRODUCTION_SHA256
    || config.triangle_count !== 138_248
    || config.primitive_count !== 157
    || config.material_count !== (config.mode === 'agent_asset' ? AGENT_V2_MATERIAL_COUNT : 12)
    || config.phase === 'restart' && !SHA256_PATTERN.test(config.expected_export_sha256 ?? '')
  ) {
    await reportFailure(config.phase, 'C111B_CONFIG_INVALID')
    return
  }
  document.documentElement.dataset.forgecadC111bPhase = config.phase
  try {
    if (config.phase === 'initial') await runInitial(config)
    else await runRestart(config)
  } catch (caught) {
    const code = caught instanceof Error && /^C111B_[A-Z0-9_]{1,120}$/.test(caught.message)
      ? caught.message
      : 'C111B_EXECUTION_FAILED'
    await reportFailure(config.phase, code)
  }
}

async function runInitial(config: Config): Promise<void> {
  if (config.mode === 'agent_asset') {
    await runAgentInitial(config)
    return
  }
  await waitForHealth()
  const root = await waitFor<HTMLElement>(
    () => document.querySelector<HTMLElement>('[data-testid="cad-workbench"]'),
    'C111B_WORKBENCH_MISSING',
  )
  await reportProgress('workbench_ready')
  await waitFor(() => root.dataset.qaProjectId || null, 'C111B_PROJECT_NOT_READY')
  await ensureAgentProject()
  const source = await loadSource(true)
  const projectId = requiredStable(root.dataset.qaProjectId, 'C111B_PROJECT_ID_INVALID')
  await injectVisibleGlb(source)
  await reportProgress('visible_import_requested')
  const active = await waitForActiveAsset(root, 'C111B_IMPORTED_ASSET_MISSING')
  const viewport = await waitForExternalViewport('C111B_IMPORTED_VIEWPORT_MISSING')
  const readback = await invokeReadback(projectId, active.assetVersionId)
  await reportProgress('external_asset_ready')
  const captures = await captureEightViews(viewport, source.sha256)
  await reportProgress('external_captures_ready')
  await reportSuccess(config, buildReport(config, root, active, viewport, source, readback, captures, false))
}

async function runRestart(config: Config): Promise<void> {
  if (config.mode === 'agent_asset') {
    await runAgentRestart(config)
    return
  }
  await waitForHealth()
  const root = await waitFor<HTMLElement>(
    () => document.querySelector<HTMLElement>('[data-testid="cad-workbench"]'),
    'C111B_RESTART_WORKBENCH_MISSING',
  )
  await reportProgress('external_restart_workbench_ready')
  const source = await loadSource(false)
  const projectId = requiredStable(root.dataset.qaProjectId, 'C111B_RESTART_PROJECT_INVALID')
  const active = await waitForActiveAsset(root, 'C111B_RESTART_ASSET_MISSING')
  await reportProgress('external_restart_snapshot_hydrated')
  const viewport = await waitForExternalViewport('C111B_RESTART_VIEWPORT_MISSING')
  const readback = await invokeReadback(projectId, active.assetVersionId)
  const captures = await captureEightViews(viewport, source.sha256)
  await reportProgress('external_restart_captures_ready')
  await reportSuccess(config, buildReport(config, root, active, viewport, source, readback, captures, true))
}

const C111_AGENT_BRIEF = '非功能展示用未来机械臂概念，强调装甲连杆、可见关节、克制蓝色饰条和统一材质分区'

async function runAgentInitial(config: Config): Promise<void> {
  await waitForHealth()
  const root = await waitFor<HTMLElement>(
    () => document.querySelector<HTMLElement>('[data-testid="cad-workbench"]'),
    'C111B_AGENT_WORKBENCH_MISSING',
  )
  await reportProgress('agent_workbench_ready')
  await waitFor(() => root.dataset.qaProjectId || null, 'C111B_AGENT_PROJECT_NOT_READY')
  const composer = await waitFor<HTMLTextAreaElement>(
    () => document.querySelector<HTMLTextAreaElement>('[aria-label="设计需求"]:not(:disabled)'),
    'C111B_AGENT_COMPOSER_DISABLED',
  )
  setTextarea(composer, C111_AGENT_BRIEF)
  const send = await waitFor<HTMLButtonElement>(
    () => document.querySelector<HTMLButtonElement>('[aria-label="发送设计需求"]:not(:disabled)'),
    'C111B_AGENT_SEND_DISABLED',
  )
  send.click()
  await reportProgress('agent_brief_sent')
  let result: HTMLElement
  try {
    result = await waitForMutation<HTMLElement>(
      () => {
        if (document.querySelector<HTMLElement>('[data-generation-state="failed"]')) {
          throw new Error('C111B_AGENT_GENERATION_FAILED')
        }
        return document.querySelector<HTMLElement>('[data-generation-state="ready"]')
      },
      'C111B_AGENT_SINGLE_RESULT_NOT_READY',
    )
  } catch (caught) {
    if (caught instanceof Error && caught.message === 'C111B_AGENT_GENERATION_FAILED') {
      throw await readAgentTurnFailure()
    }
    throw caught
  }
  if (!root.dataset.qaSingleResultPreviewId || root.dataset.qaSingleResultProfile !== 'production_concept') {
    throw new Error('C111B_AGENT_SINGLE_RESULT_LINEAGE_INVALID')
  }
  const previewViewport = await waitForAgentViewport('C111B_AGENT_PREVIEW_VIEWPORT_MISSING')
  if (previewViewport.dataset.blockoutGlbKind !== 'compiled_agent_production_pbr') {
    throw new Error('C111B_AGENT_PREVIEW_PROFILE_INVALID')
  }
  const save = result.querySelector<HTMLButtonElement>('[aria-label="保存为可编辑模型"]')
  if (!save || save.disabled) throw new Error('C111B_AGENT_CONFIRM_MISSING')
  save.click()
  const v1 = await waitForActiveAsset(root, 'C111B_AGENT_V1_SNAPSHOT_MISSING')
  await reportProgress('agent_v1_confirmed')
  const selectionCardSelector = `[aria-label="分件候选"][data-agent-asset-version-id="${v1.assetVersionId}"][data-active-agent-asset-version-id="${v1.assetVersionId}"]`
  const advancedEntry = await waitFor<HTMLElement>(
    () => document.querySelector<HTMLElement>(selectionCardSelector)
      ?? document.querySelector<HTMLButtonElement>('[data-qa-action="open-advanced-settings"]:not(:disabled), [aria-label="打开高级设置"]:not(:disabled)'),
    'C111B_AGENT_ADVANCED_ACTIONS_MISSING',
    30_000,
  )
  if (advancedEntry instanceof HTMLButtonElement) advancedEntry.click()
  const selectionCard = await waitForMutation<HTMLElement>(
    () => document.querySelector<HTMLElement>(selectionCardSelector),
    'C111B_AGENT_SELECTION_CARD_MISSING',
  )
  assertC111bSelectionCardIdentity(
    selectionCard.dataset.agentAssetVersionId,
    selectionCard.dataset.activeAgentAssetVersionId,
    v1.assetVersionId,
  )
  await reportProgress('agent_selection_card_ready')
  await reportProgress(`agent_link_candidates_${linkPartCandidateSummary()}`)
  const part = await waitFor<HTMLButtonElement>(() => {
    const buttons = [...selectionCard.querySelectorAll<HTMLButtonElement>('[data-qa-part-role]:not(:disabled)')]
    if (buttons.length === 0) return null
    const partId = resolveC111bLinkCandidate(buttons.map((button) => ({
      partId: button.dataset.qaPartId ?? null,
      role: button.dataset.qaPartRole ?? null,
      materialZoneIds: (button.dataset.qaMaterialZoneIds ?? '').split(/\s+/).filter(Boolean),
    })))
    return buttons.find((button) => button.dataset.qaPartId === partId) ?? null
  },
    'C111B_AGENT_LINK_PART_MISSING',
    30_000,
  )
  const selectedPartId = part.dataset.qaPartId
  if (!selectedPartId) throw new Error('C111B_AGENT_LINK_PART_ID_MISSING')
  part.click()
  await waitForMutation<HTMLButtonElement>(
    () => selectionCard.querySelector<HTMLButtonElement>(
      `[data-qa-part-id="${selectedPartId}"][aria-pressed="true"]`,
    ),
    'C111B_AGENT_LINK_PART_NOT_SELECTED',
  )
  await reportProgress('agent_link_part_selected')
  const adorn = await waitFor<HTMLButtonElement>(
    () => document.querySelector<HTMLButtonElement>('[aria-label="添加外观细节"]:not(:disabled)'),
    'C111B_AGENT_ADORNMENT_ACTION_MISSING',
  )
  adorn.click()
  const adornmentDrawer = await waitForMutation<HTMLElement>(
    () => document.querySelector<HTMLElement>('[role="dialog"][aria-label="添加外观细节"]'),
    'C111B_AGENT_ADORNMENT_DRAWER_MISSING',
  )
  if (
    adornmentDrawer.dataset.targetPartId !== selectedPartId
    || adornmentDrawer.dataset.targetMaterialZoneId !== 'zone_arm_link_shell'
    || !adornmentDrawer.querySelector(
      '[data-testid="surface-adornment-design-surface"][data-material-zone-id="zone_arm_link_shell"]',
    )
  ) throw new Error('C111B_AGENT_ADORNMENT_TARGET_INVALID')
  await reportProgress('agent_adornment_drawer_ready')
  await retainSurfaceAdornment()
  const v2 = await waitForRetainedActiveAsset(root, v1.assetVersionId)
  await waitForSnapshotIdle('C111B_AGENT_V2_SNAPSHOT_NOT_IDLE')
  const close = document.querySelector<HTMLButtonElement>('button[aria-label="关闭添加外观细节"]')
  close?.click()
  await waitForMutation(
    () => document.querySelector('[role="dialog"][aria-label="添加外观细节"]') === null ? document.documentElement : null,
    'C111B_AGENT_ADORNMENT_DRAWER_CLOSE_FAILED',
  )
  await reportProgress('agent_v2_confirmed')
  await reportProgress('agent_production_viewport_requested')
  const viewport = await withC111bStageError(
    'C111B_AGENT_PRODUCTION_VIEWPORT_FAILED',
    () => waitForAgentViewport('C111B_AGENT_PRODUCTION_VIEWPORT_MISSING'),
  )
  await reportProgress('agent_production_viewport_ready')
  const projectId = requiredStable(root.dataset.qaProjectId, 'C111B_AGENT_PROJECT_ID_INVALID')
  await reportProgress('agent_readback_requested')
  const readback = await withC111bStageError(
    'C111B_AGENT_READBACK_FAILED',
    () => invokeReadback(projectId, v2.assetVersionId, 'agent_asset'),
  )
  await reportProgress('agent_readback_ready')
  await reportProgress('agent_export_requested')
  const exportedBytes = await withC111bStageError(
    'C111B_AGENT_EXPORT_FAILED',
    () => readCurrentAgentGlbBytes(),
  )
  const exportedSha = await sha256(exportedBytes)
  if (exportedSha !== readback.source_sha256) throw new Error('C111B_AGENT_EXPORT_HASH_DRIFT')
  await reportProgress('agent_export_readback_ready')
  const captures = await captureEightViews(viewport, exportedSha)
  await reportProgress('agent_captures_ready')
  await reportSuccess(config, buildAgentReport(config, root, v2, viewport, readback, captures, false))
}

async function runAgentRestart(config: Config): Promise<void> {
  await waitForHealth()
  const root = await waitFor<HTMLElement>(
    () => document.querySelector<HTMLElement>('[data-testid="cad-workbench"]'),
    'C111B_AGENT_RESTART_WORKBENCH_MISSING',
  )
  await reportProgress('agent_restart_workbench_ready')
  const active = await waitForActiveAsset(root, 'C111B_AGENT_RESTART_ASSET_MISSING')
  await reportProgress('agent_restart_snapshot_hydrated')
  const viewport = await waitForAgentViewport('C111B_AGENT_RESTART_VIEWPORT_MISSING')
  const projectId = requiredStable(root.dataset.qaProjectId, 'C111B_AGENT_RESTART_PROJECT_INVALID')
  const readback = await invokeReadback(projectId, active.assetVersionId, 'agent_asset')
  if (readback.source_sha256 !== config.expected_export_sha256) {
    throw new Error('C111B_AGENT_RESTART_EXPECTED_EXPORT_HASH_DRIFT')
  }
  const exportedSha = await sha256(await readCurrentAgentGlbBytes())
  if (exportedSha !== readback.source_sha256) throw new Error('C111B_AGENT_RESTART_EXPORT_HASH_DRIFT')
  await reportProgress('agent_restart_export_readback_ready')
  const captures = await captureEightViews(viewport, exportedSha)
  await reportProgress('agent_restart_captures_ready')
  await reportSuccess(config, buildAgentReport(config, root, active, viewport, readback, captures, true))
}

function buildAgentReport(
  config: Config,
  root: HTMLElement,
  active: { assetVersionId: string; snapshotRevision: number },
  viewport: HTMLElement,
  readback: Readback,
  captures: Capture[],
  restartHydrated: boolean,
): Report {
  const rendererGeneration = positiveNumber(viewport.dataset.rendererGeneration, 'C111B_AGENT_RENDERER_GENERATION_INVALID')
  const activeContexts = positiveNumber(viewport.dataset.activeWebglContexts, 'C111B_AGENT_WEBGL_CONTEXT_INVALID')
  const pbrFacts = readPackagedPbrRendererFacts(viewport, 'C111B_AGENT_RENDERER_FACTS_INVALID')
  if (
    viewport.dataset.blockoutLoadState !== 'ready'
    || viewport.dataset.blockoutGlbKind !== 'compiled_agent_production_pbr'
    || viewport.dataset.blockoutRenderSource !== 'glb_pbr'
    || Number(viewport.dataset.blockoutEmbeddedPbrMaterialCount ?? '0') < 1
    || activeContexts !== 1
    || document.querySelectorAll('canvas').length !== 1
    || viewport.dataset.lightPreset !== 'soft_studio'
  ) throw new Error('C111B_AGENT_RENDERER_FACTS_INVALID')
  return {
    schema_version: SCHEMA,
    phase: config.phase,
    ok: true,
    project_id: requiredStable(root.dataset.qaProjectId, 'C111B_AGENT_PROJECT_ID_INVALID'),
    asset_version_id: active.assetVersionId,
    snapshot_revision: active.snapshotRevision,
    source_sha256: readback.source_sha256,
    triangle_count: readback.glb_triangle_count,
    primitive_count: readback.glb_primitive_count,
    material_count: readback.glb_material_count,
    complete_pbr_material_count: Number(viewport.dataset.blockoutEmbeddedPbrMaterialCount ?? '0'),
    renderer_generation: rendererGeneration,
    active_webgl_contexts: activeContexts,
    canvas_count: 1,
    blockout_glb_kind: viewport.dataset.blockoutGlbKind,
    render_source: viewport.dataset.blockoutRenderSource,
    light_preset: viewport.dataset.lightPreset,
    ...pbrFacts,
    captures,
    readback,
    formal_eligible: false,
    human_benchmark_evidence: false,
    reference_comparison: false,
    ...(config.phase === 'initial' ? {
      thread_id: requiredStable(root.dataset.qaAgentThreadId, 'C111B_AGENT_THREAD_ID_INVALID'),
      turn_id: requiredStable(root.dataset.qaAgentTurnId, 'C111B_AGENT_TURN_ID_INVALID'),
    } : {}),
    restart_hydrated: restartHydrated,
  }
}

function buildReport(
  config: Config,
  root: HTMLElement,
  active: { assetVersionId: string; snapshotRevision: number },
  viewport: HTMLElement,
  source: Source,
  readback: Readback,
  captures: Capture[],
  restartHydrated: boolean,
): Report {
  const rendererGeneration = positiveNumber(viewport.dataset.rendererGeneration, 'C111B_RENDERER_GENERATION_INVALID')
  const activeContexts = positiveNumber(viewport.dataset.activeWebglContexts, 'C111B_WEBGL_CONTEXT_INVALID')
  const pbrFacts = readPackagedPbrRendererFacts(viewport, 'C111B_RENDERER_FACTS_INVALID')
  if (
    viewport.dataset.blockoutLoadState !== 'ready'
    || viewport.dataset.blockoutGlbKind !== 'external_reference'
    || viewport.dataset.blockoutRenderSource !== 'external_reference'
    || Number(viewport.dataset.blockoutEmbeddedPbrMaterialCount ?? '0') < 1
    || activeContexts !== 1
    || document.querySelectorAll('canvas').length !== 1
    || viewport.dataset.lightPreset !== 'soft_studio'
  ) throw new Error('C111B_RENDERER_FACTS_INVALID')
  return {
    schema_version: SCHEMA,
    phase: config.phase,
    ok: true,
    project_id: requiredStable(root.dataset.qaProjectId, 'C111B_PROJECT_ID_INVALID'),
    asset_version_id: active.assetVersionId,
    snapshot_revision: active.snapshotRevision,
    source_sha256: source.sha256,
    triangle_count: source.triangle_count,
    primitive_count: source.primitive_count,
    material_count: source.material_count,
    complete_pbr_material_count: source.complete_pbr_material_count,
    renderer_generation: rendererGeneration,
    active_webgl_contexts: activeContexts,
    canvas_count: 1,
    blockout_glb_kind: viewport.dataset.blockoutGlbKind,
    render_source: viewport.dataset.blockoutRenderSource,
    light_preset: viewport.dataset.lightPreset,
    ...pbrFacts,
    captures,
    readback,
    formal_eligible: false,
    human_benchmark_evidence: false,
    reference_comparison: false,
    restart_hydrated: restartHydrated,
  }
}

function readPackagedPbrRendererFacts(
  viewport: HTMLElement,
  errorCode: string,
): Pick<
  Report,
  | 'renderer_id'
  | 'render_manifest_sha256'
  | 'visual_environment_id'
  | 'visual_environment_sha256'
  | 'output_color_space'
  | 'tone_mapping'
  | 'pbr_texture_count'
  | 'pbr_color_spaces'
  | 'pbr_sampling_valid'
> {
  const data = viewport.dataset
  const pbrTextureCount = Number(data.blockoutPbrTextureCount ?? '0')
  if (
    data.pbrRendererId !== WORKBENCH_PBR_RENDERER_ID
    || data.pbrRenderManifestSha256 !== WORKBENCH_PBR_RENDER_MANIFEST_SHA256
    || data.visualEnvironmentId !== WORKBENCH_PBR_VISUAL_ENVIRONMENT_ID
    || data.visualEnvironmentSha256 !== WORKBENCH_PBR_VISUAL_ENVIRONMENT_SHA256
    || data.outputColorSpace !== 'srgb'
    || data.toneMapping !== 'aces_filmic'
    || !Number.isInteger(pbrTextureCount)
    || pbrTextureCount < 5
    || data.blockoutPbrColorSpaces !== 'valid'
    || data.blockoutPbrSamplingValid !== 'true'
  ) throw new Error(errorCode)
  return {
    renderer_id: WORKBENCH_PBR_RENDERER_ID,
    render_manifest_sha256: WORKBENCH_PBR_RENDER_MANIFEST_SHA256,
    visual_environment_id: WORKBENCH_PBR_VISUAL_ENVIRONMENT_ID,
    visual_environment_sha256: WORKBENCH_PBR_VISUAL_ENVIRONMENT_SHA256,
    output_color_space: 'srgb',
    tone_mapping: 'aces_filmic',
    pbr_texture_count: pbrTextureCount,
    pbr_color_spaces: 'valid',
    pbr_sampling_valid: 'true',
  }
}

async function ensureAgentProject(): Promise<void> {
  const button = [...document.querySelectorAll<HTMLButtonElement>('button')]
    .find((candidate) => candidate.textContent?.includes('让 Agent 重建可编辑资产'))
  if (!button) return
  if (button.disabled) throw new Error('C111B_LEGACY_CONVERSION_DISABLED')
  button.click()
  await reportProgress('legacy_conversion_requested')
  await waitForMutation(
    () => document.querySelector('button') && ![...document.querySelectorAll<HTMLButtonElement>('button')]
      .some((candidate) => candidate.textContent?.includes('让 Agent 重建可编辑资产'))
      ? document.documentElement
      : null,
    'C111B_LEGACY_CONVERSION_NOT_FINISHED',
  )
}

async function loadSource(includeBytes: boolean): Promise<Source> {
  const { invoke } = await import('@tauri-apps/api/core')
  const source = await invoke<Source>('forgecad_c111b_webview_qa_source', {
    request: { schema_version: SCHEMA, include_bytes: includeBytes },
  })
  if (
    source.schema_version !== SCHEMA
    || source.file_name !== 'c111b-production.glb'
    || source.sha256 !== BASE_PRODUCTION_SHA256
    || source.triangle_count !== 138_248
    || source.primitive_count !== 157
    || source.material_count !== 12
    || source.complete_pbr_material_count < 1
  ) throw new Error('C111B_SOURCE_INVENTORY_INVALID')
  if (includeBytes) {
    if (!source.bytes_base64) throw new Error('C111B_SOURCE_BYTES_MISSING')
    const digest = await sha256(decodeBase64(source.bytes_base64))
    if (digest !== source.sha256) throw new Error('C111B_SOURCE_BROWSER_HASH_INVALID')
  }
  return source
}

async function injectVisibleGlb(source: Source): Promise<void> {
  if (!source.bytes_base64) throw new Error('C111B_SOURCE_BYTES_MISSING')
  const input = await waitFor<HTMLInputElement>(
    () => document.querySelector<HTMLInputElement>('input[aria-label="导入 GLB 参考模型"]'),
    'C111B_VISIBLE_IMPORT_INPUT_MISSING',
  )
  const bytes = decodeBase64(source.bytes_base64)
  if (bytes.byteLength !== source.byte_size) throw new Error('C111B_SOURCE_BYTE_SIZE_INVALID')
  if (typeof DataTransfer === 'undefined') throw new Error('C111B_FILE_INJECTION_UNAVAILABLE')
  const transfer = new DataTransfer()
  const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer
  transfer.items.add(new File([buffer], source.file_name, { type: 'model/gltf-binary', lastModified: 0 }))
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'files')?.set
  if (!setter) throw new Error('C111B_FILE_INPUT_SETTER_UNAVAILABLE')
  setter.call(input, transfer.files)
  input.dispatchEvent(new Event('change', { bubbles: true }))
}

async function invokeReadback(
  projectId: string,
  assetVersionId: string,
  mode: Config['mode'] = 'external_reference',
): Promise<Readback> {
  const { invoke } = await import('@tauri-apps/api/core')
  const readback = await invoke<Readback>('forgecad_c111b_webview_qa_readback', {
    request: { schema_version: SCHEMA, project_id: projectId, asset_version_id: assetVersionId },
  })
  if (
    readback.schema_version !== SCHEMA
    || readback.project_id !== projectId
    || readback.asset_version_id !== assetVersionId
    || !SHA256_PATTERN.test(readback.source_sha256)
    || mode === 'external_reference' && (
      readback.shape_program_schema !== 'ExternalGLBReference@1'
      || readback.external_reference !== true
      || readback.source_sha256 !== BASE_PRODUCTION_SHA256
    )
    || mode === 'agent_asset' && (
      readback.shape_program_schema !== 'ShapeProgram@1'
      || readback.external_reference !== false
      || readback.glb_triangle_count !== 138_248
      || readback.glb_primitive_count !== 157
      || readback.glb_material_count !== AGENT_V2_MATERIAL_COUNT
    )
  ) throw new Error('C111B_EXACT_READBACK_INVALID')
  return readback
}

async function waitForAgentViewport(errorCode: string): Promise<HTMLElement> {
  const viewport = await waitForMutation<HTMLElement>(() => {
    const candidate = document.querySelector<HTMLElement>('[aria-label="真实 ModuleGraph 三维视口"]')
    if (!candidate) return null
    if (candidate.dataset.blockoutLoadState === 'failed') throw new Error('C111B_AGENT_VIEWPORT_LOAD_FAILED')
    return candidate.dataset.blockoutLoadState === 'ready'
      && candidate.dataset.blockoutGlbKind === 'compiled_agent_production_pbr'
      && candidate.dataset.blockoutRenderSource === 'glb_pbr'
      ? candidate
      : null
  }, errorCode)
  await setLight(viewport)
  return viewport
}

async function retainSurfaceAdornment(): Promise<void> {
  let primary = await waitForMutation<HTMLButtonElement>(
    () => document.querySelector<HTMLButtonElement>('[data-adornment-action="preview"]:not(:disabled), [data-adornment-action="enable"]:not(:disabled)'),
    'C111B_AGENT_ADORNMENT_PREVIEW_MISSING',
  )
  if (primary.dataset.adornmentAction === 'enable') {
    primary.click()
    primary = await waitForMutation<HTMLButtonElement>(() => {
      return document.querySelector<HTMLButtonElement>('[data-adornment-action="preview"]:not(:disabled)')
    }, 'C111B_AGENT_ADORNMENT_ENABLE_FAILED')
    await reportProgress('agent_adornment_enabled')
  }
  if (primary.dataset.adornmentAction !== 'preview') throw new Error('C111B_AGENT_ADORNMENT_PRIMARY_INVALID')
  primary.click()
  await reportProgress('agent_adornment_preview_requested')
  let outcome = await waitForAdornmentPreviewOutcome()
  if (outcome.dataset.adornmentAction === 'enable') {
    outcome.click()
    await reportProgress('agent_adornment_enable_requested')
    primary = await waitForMutation<HTMLButtonElement>(() => {
      return document.querySelector<HTMLButtonElement>('[data-adornment-action="preview"]:not(:disabled)')
    }, 'C111B_AGENT_ADORNMENT_ENABLE_FAILED')
    await reportProgress('agent_adornment_enabled')
    primary.click()
    await reportProgress('agent_adornment_preview_requested_after_enable')
    outcome = await waitForAdornmentPreviewOutcome()
  }
  if (outcome.dataset.adornmentAction !== 'retain' || !outcome.closest('.surface-adornment-actions')) {
    throw new Error('C111B_AGENT_ADORNMENT_RETAIN_INVALID')
  }
  await reportProgress('agent_adornment_preview_ready')
  outcome.click()
  await reportProgress('agent_adornment_retain_requested')
}

async function waitForAdornmentPreviewOutcome(): Promise<HTMLButtonElement> {
  try {
    return await waitForMutation<HTMLButtonElement>(() => {
      const retain = document.querySelector<HTMLButtonElement>('[data-adornment-action="retain"]:not(:disabled)')
      if (retain) return retain
      const enable = document.querySelector<HTMLButtonElement>('[data-adornment-action="enable"]:not(:disabled)')
      if (enable) return enable
      const failed = document.querySelector<HTMLElement>(
        '[role="dialog"][aria-label="添加外观细节"] .surface-adornment-status.failed[data-error-code]:not([data-error-code=""])',
      )
      if (failed?.dataset.errorCode) {
        throw new Error(normalizeC111bAdornmentErrorCode(failed.dataset.errorCode))
      }
      return null
    }, 'C111B_AGENT_ADORNMENT_RETAIN_MISSING')
  } catch (caught) {
    if (caught instanceof Error && caught.message === 'C111B_AGENT_ADORNMENT_RETAIN_MISSING') {
      const stage = document.documentElement.dataset.qaSurfaceAdornmentStage
      const suffix = stage?.toUpperCase().replace(/[^A-Z0-9_]/g, '_').slice(0, 72) || 'UNKNOWN'
      throw new Error(`C111B_AGENT_ADORNMENT_TIMEOUT_${suffix}`)
    }
    throw caught
  }
}

async function waitForSnapshotIdle(errorCode: string): Promise<void> {
  await waitForMutation<HTMLElement>(() => {
    const readout = document.querySelector<HTMLElement>('.viewport-readout span:first-child')
    return readout?.textContent?.includes('当前视口绑定 Agent Snapshot') ? readout : null
  }, errorCode)
}

function setTextarea(textarea: HTMLTextAreaElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set
  if (!setter) throw new Error('C111B_AGENT_TEXTAREA_SETTER_MISSING')
  setter.call(textarea, value)
  textarea.dispatchEvent(new Event('input', { bubbles: true }))
}

async function readCurrentAgentGlbBytes(): Promise<Uint8Array> {
  const originalCreateObjectUrl = URL.createObjectURL.bind(URL)
  let downloadedBlob: Blob | null = null
  URL.createObjectURL = (value: Blob | MediaSource): string => {
    if (value instanceof Blob) downloadedBlob = value
    return originalCreateObjectUrl(value)
  }
  try {
    const exportEntry = await waitFor<HTMLElement>(
      () => document.querySelector<HTMLButtonElement>('button[aria-label="导出"]:not(:disabled)')
        ?? document.querySelector<HTMLButtonElement>('[data-qa-action="open-advanced-settings"]:not(:disabled), [aria-label="打开高级设置"]:not(:disabled)'),
      'C111B_AGENT_EXPORT_ACTION_MISSING',
    )
    if (exportEntry instanceof HTMLButtonElement && exportEntry.getAttribute('aria-label') !== '导出') {
      exportEntry.click()
    }
    const openExport = await waitFor<HTMLButtonElement>(
      () => document.querySelector<HTMLButtonElement>('button[aria-label="导出"]:not(:disabled)'),
      'C111B_AGENT_EXPORT_ACTION_MISSING',
    )
    openExport.click()
    const drawer = await waitForMutation<HTMLElement>(
      () => document.querySelector<HTMLElement>('[role="dialog"][data-forgecad-drawer="export"]'),
      'C111B_AGENT_EXPORT_DRAWER_MISSING',
    )
    const download = await waitFor<HTMLButtonElement>(
      () => [...drawer.querySelectorAll<HTMLButtonElement>('button')]
        .find((button) => button.textContent?.includes('下载 3D 模型 (GLB)') && !button.disabled) ?? null,
      'C111B_AGENT_EXPORT_DOWNLOAD_MISSING',
    )
    download.click()
    downloadedBlob = await waitFor<Blob>(() => downloadedBlob, 'C111B_AGENT_EXPORT_BLOB_MISSING')
  } finally {
    URL.createObjectURL = originalCreateObjectUrl
  }
  const close = document.querySelector<HTMLButtonElement>('button[aria-label="关闭导出"]')
  close?.click()
  await waitForMutation(
    () => document.querySelector('[role="dialog"][data-forgecad-drawer="export"]') === null
      ? document.documentElement
      : null,
    'C111B_AGENT_EXPORT_DRAWER_CLOSE_FAILED',
  )
  if (!downloadedBlob) throw new Error('C111B_AGENT_EXPORT_BLOB_MISSING')
  return new Uint8Array(await downloadedBlob.arrayBuffer())
}

async function waitForExternalViewport(errorCode: string): Promise<HTMLElement> {
  const viewport = await waitForMutation<HTMLElement>(() => {
    const candidate = document.querySelector<HTMLElement>('[aria-label="真实 ModuleGraph 三维视口"]')
    if (!candidate) return null
    if (candidate.dataset.blockoutLoadState === 'failed') throw new Error('C111B_VIEWPORT_LOAD_FAILED')
    return candidate.dataset.blockoutLoadState === 'ready'
      && candidate.dataset.blockoutGlbKind === 'external_reference'
      && candidate.dataset.blockoutRenderSource === 'external_reference'
      ? candidate
      : null
  }, errorCode)
  await setLight(viewport)
  return viewport
}

async function setLight(viewport: HTMLElement): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const timeout = window.setTimeout(() => reject(new Error('C111B_LIGHT_TIMEOUT')), 10_000)
    viewport.dispatchEvent(new CustomEvent(LIGHT_EVENT, {
      detail: {
        viewport,
        preset: 'soft_studio',
        resolve: () => { window.clearTimeout(timeout); resolve() },
        reject: (error: unknown) => { window.clearTimeout(timeout); reject(error instanceof Error ? error : new Error('C111B_LIGHT_FAILED')) },
      },
    }))
  })
  if (viewport.dataset.lightPreset !== 'soft_studio') throw new Error('C111B_LIGHT_PRESET_INVALID')
}

async function captureEightViews(viewport: HTMLElement, sourceSha256: string): Promise<Capture[]> {
  const captures: Capture[] = []
  for (const view of FIXED_VIEWS) {
    await setCamera(viewport, view)
    const pixels = await requestViewportPixels(viewport)
    const { blob, readability } = await pixelsToPng(pixels)
    const auxiliaryBlob = await auxiliaryPixelsToPng(pixels)
    const receipt = await capturePng(view, blob, auxiliaryBlob, sourceSha256)
    if (receipt.source_sha256 !== sourceSha256) throw new Error('C111B_CAPTURE_SOURCE_LINEAGE_INVALID')
    const readabilityFailure = rasterReadabilityFailure(view, readability)
    if (readabilityFailure) throw new Error(readabilityFailure)
    captures.push({ ...receipt, readability })
  }
  return captures
}

function rasterReadabilityFailure(view: FixedView, readability: RasterReadability): string | null {
  if (
    readability.foreground_coverage_bps >= 100
    && readability.foreground_median_luma >= 24
    && readability.foreground_readable_bps >= 5000
  ) return null
  return `C111B_PNG_UNREADABLE_${view.toUpperCase()}_C${readability.foreground_coverage_bps}_M${readability.foreground_median_luma}_R${readability.foreground_readable_bps}`
}

async function setCamera(viewport: HTMLElement, view: FixedView): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const timeout = window.setTimeout(() => reject(new Error('C111B_CAMERA_TIMEOUT')), 10_000)
    viewport.dispatchEvent(new CustomEvent(CAMERA_EVENT, {
      detail: {
        viewport,
        view,
        resolve: () => { window.clearTimeout(timeout); resolve() },
        reject: (error: unknown) => { window.clearTimeout(timeout); reject(error instanceof Error ? error : new Error('C111B_CAMERA_FAILED')) },
      },
    }))
  })
  if (viewport.dataset.cameraView !== view) throw new Error('C111B_CAMERA_VIEW_INVALID')
  await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
}

async function requestViewportPixels(viewport: HTMLElement): Promise<ViewportPixels> {
  return new Promise<ViewportPixels>((resolve, reject) => {
    let settled = false
    const timeout = window.setTimeout(() => finish(() => reject(new Error('C111B_PIXEL_CAPTURE_TIMEOUT'))), 12_000)
    const finish = (callback: () => void) => {
      if (settled) return
      settled = true
      window.clearTimeout(timeout)
      callback()
    }
    const request: ViewportCaptureRequest = {
      viewport,
      resolve: (capture) => finish(() => {
        if (
          capture.width <= 0
          || capture.height <= 0
          || capture.pixels.byteLength !== capture.width * capture.height * 4
          || capture.origin !== 'top_left'
          || capture.width !== WORKBENCH_PBR_CAPTURE_WIDTH_PX
          || capture.height !== WORKBENCH_PBR_CAPTURE_HEIGHT_PX
          || capture.auxiliaryWidth !== WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX
          || capture.auxiliaryHeight !== WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX
          || capture.auxiliaryPixels.byteLength !== capture.auxiliaryWidth * capture.auxiliaryHeight * 4
          || capture.auxiliaryPassIds.join(',') !== 'silhouette,normal,depth,part_id,material_id'
        ) {
          reject(new Error('C111B_PIXEL_CAPTURE_INVALID'))
        } else resolve(capture)
      }),
      reject: (error) => finish(() => reject(error instanceof Error ? error : new Error('C111B_PIXEL_CAPTURE_FAILED'))),
    }
    viewport.dispatchEvent(new CustomEvent<ViewportCaptureRequest>(CAPTURE_EVENT, { detail: request }))
  })
}

async function pixelsToPng(capture: ViewportPixels): Promise<{ blob: Blob; readability: RasterReadability }> {
  const raster = document.createElement('canvas')
  raster.width = capture.width
  raster.height = capture.height
  const context = raster.getContext('2d', { willReadFrequently: true })
  if (!context) throw new Error('C111B_PNG_CONTEXT_MISSING')
  const image = context.createImageData(capture.width, capture.height)
  const rowBytes = capture.width * 4
  for (let sourceY = 0; sourceY < capture.height; sourceY += 1) {
    const sourceOffset = sourceY * rowBytes
    const targetOffset = sourceY * rowBytes
    image.data.set(capture.pixels.subarray(sourceOffset, sourceOffset + rowBytes), targetOffset)
  }
  normalizeWkWebviewDisplayTransfer(image.data, capture.width, capture.height)
  context.putImageData(image, 0, 0)
  const readability = assertVisibleRaster(context, capture.width, capture.height)
  const blob = await new Promise<Blob | null>((resolve) => raster.toBlob(resolve, 'image/png'))
  if (!blob || blob.size === 0) throw new Error('C111B_PNG_UNAVAILABLE')
  return { blob, readability }
}

function normalizeWkWebviewDisplayTransfer(pixels: Uint8ClampedArray, width: number, height: number): void {
  // WKWebView exposes lit surface channels from the WebGL backing store in
  // linear space while its compositor presents those channels as sRGB. The
  // scene clear colour is already display-encoded, so converting every pixel
  // would incorrectly lift the studio background. Resolve the modal clear
  // colour from a bounded sample and transfer only definite foreground.
  const sampleStep = Math.max(1, Math.floor(Math.sqrt(width * height / (96 * 96))))
  const colorCounts = new Map<number, number>()
  for (let y = 0; y < height; y += sampleStep) {
    for (let x = 0; x < width; x += sampleStep) {
      const offset = (y * width + x) * 4
      const packed = (pixels[offset]! << 16) | (pixels[offset + 1]! << 8) | pixels[offset + 2]!
      colorCounts.set(packed, (colorCounts.get(packed) ?? 0) + 1)
    }
  }
  const backgroundPacked = [...colorCounts.entries()].sort((left, right) => right[1] - left[1])[0]?.[0]
  if (backgroundPacked === undefined) throw new Error('C111B_PNG_BACKGROUND_UNAVAILABLE')
  const background = [
    (backgroundPacked >> 16) & 0xff,
    (backgroundPacked >> 8) & 0xff,
    backgroundPacked & 0xff,
  ] as const
  for (let offset = 0; offset < pixels.length; offset += 4) {
    const distance = Math.max(
      Math.abs(pixels[offset]! - background[0]),
      Math.abs(pixels[offset + 1]! - background[1]),
      Math.abs(pixels[offset + 2]! - background[2]),
    )
    if (pixels[offset + 3]! === 0 || distance <= 10) continue
    pixels[offset] = linearByteToSrgb(pixels[offset]!)
    pixels[offset + 1] = linearByteToSrgb(pixels[offset + 1]!)
    pixels[offset + 2] = linearByteToSrgb(pixels[offset + 2]!)
  }
}

function linearByteToSrgb(value: number): number {
  const linear = value / 255
  const encoded = linear <= 0.0031308
    ? linear * 12.92
    : 1.055 * linear ** (1 / 2.4) - 0.055
  return Math.round(Math.min(Math.max(encoded, 0), 1) * 255)
}

function assertVisibleRaster(context: CanvasRenderingContext2D, width: number, height: number): RasterReadability {
  const sampleCanvas = document.createElement('canvas')
  sampleCanvas.width = Math.min(width, 96)
  sampleCanvas.height = Math.min(height, 96)
  const sampleContext = sampleCanvas.getContext('2d', { willReadFrequently: true })
  if (!sampleContext) throw new Error('C111B_PNG_SAMPLE_CONTEXT_MISSING')
  sampleContext.drawImage(context.canvas, 0, 0, sampleCanvas.width, sampleCanvas.height)
  const sample = sampleContext.getImageData(0, 0, sampleCanvas.width, sampleCanvas.height).data
  let opaque = 0
  let minLuma = 255
  let maxLuma = 0
  const colorCounts = new Map<number, number>()
  for (let offset = 0; offset < sample.length; offset += 4) {
    if (sample[offset + 3]! > 0) opaque += 1
    const packed = (sample[offset]! << 16) | (sample[offset + 1]! << 8) | sample[offset + 2]!
    colorCounts.set(packed, (colorCounts.get(packed) ?? 0) + 1)
    const luma = Math.round(sample[offset]! * 0.2126 + sample[offset + 1]! * 0.7152 + sample[offset + 2]! * 0.0722)
    minLuma = Math.min(minLuma, luma)
    maxLuma = Math.max(maxLuma, luma)
  }
  if (opaque < (sample.length / 4) * 0.9 || maxLuma - minLuma < 4) throw new Error('C111B_PNG_BLANK')
  const backgroundPacked = [...colorCounts.entries()].sort((left, right) => right[1] - left[1])[0]?.[0]
  if (backgroundPacked === undefined) throw new Error('C111B_PNG_BACKGROUND_UNAVAILABLE')
  const background: [number, number, number] = [
    (backgroundPacked >> 16) & 0xff,
    (backgroundPacked >> 8) & 0xff,
    backgroundPacked & 0xff,
  ]
  const foregroundLuma: number[] = []
  for (let offset = 0; offset < sample.length; offset += 4) {
    const distance = Math.max(
      Math.abs(sample[offset]! - background[0]),
      Math.abs(sample[offset + 1]! - background[1]),
      Math.abs(sample[offset + 2]! - background[2]),
    )
    if (sample[offset + 3]! === 0 || distance <= 10) continue
    foregroundLuma.push(Math.round(
      sample[offset]! * 0.2126 + sample[offset + 1]! * 0.7152 + sample[offset + 2]! * 0.0722,
    ))
  }
  foregroundLuma.sort((left, right) => left - right)
  const samplePixelCount = sample.length / 4
  const foregroundPixelCount = foregroundLuma.length
  const foregroundCoverageBps = Math.round(foregroundPixelCount * 10_000 / samplePixelCount)
  const foregroundMedianLuma = foregroundLuma[Math.floor(foregroundPixelCount / 2)] ?? 0
  const readableCount = foregroundLuma.filter((value) => value >= 24).length
  const foregroundReadableBps = foregroundPixelCount > 0
    ? Math.round(readableCount * 10_000 / foregroundPixelCount)
    : 0
  return {
    pixel_encoding: 'display_srgb',
    display_transfer: 'wkwebview_linear_lit_surface_to_srgb',
    sample_pixel_count: samplePixelCount,
    foreground_pixel_count: foregroundPixelCount,
    foreground_coverage_bps: foregroundCoverageBps,
    foreground_median_luma: foregroundMedianLuma,
    foreground_readable_bps: foregroundReadableBps,
    background_rgb: background,
  }
}

async function auxiliaryPixelsToPng(capture: ViewportPixels): Promise<Blob> {
  const raster = document.createElement('canvas')
  raster.width = capture.auxiliaryWidth
  raster.height = capture.auxiliaryHeight
  const context = raster.getContext('2d', { willReadFrequently: true })
  if (!context) throw new Error('C111B_AUXILIARY_PNG_CONTEXT_MISSING')
  const image = context.createImageData(capture.auxiliaryWidth, capture.auxiliaryHeight)
  image.data.set(capture.auxiliaryPixels)
  context.putImageData(image, 0, 0)
  const blob = await new Promise<Blob | null>((resolve) => raster.toBlob(resolve, 'image/png'))
  if (!blob || blob.size === 0) throw new Error('C111B_AUXILIARY_PNG_UNAVAILABLE')
  return blob
}

async function capturePng(
  view: FixedView,
  blob: Blob,
  auxiliaryBlob: Blob,
  sourceSha256: string,
): Promise<CaptureReceipt> {
  const { invoke } = await import('@tauri-apps/api/core')
  const bytes = new Uint8Array(await blob.arrayBuffer())
  const auxiliaryBytes = new Uint8Array(await auxiliaryBlob.arrayBuffer())
  const receipt = await invoke<CaptureReceipt>('forgecad_c111b_webview_qa_capture', {
    capture: {
      schema_version: SCHEMA,
      phase: currentPhase(),
      view_id: view,
      source_sha256: sourceSha256,
      bytes_base64: encodeBase64(bytes),
      auxiliary_width: WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX,
      auxiliary_height: WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX,
      auxiliary_pass_ids: ['silhouette', 'normal', 'depth', 'part_id', 'material_id'],
      auxiliary_bytes_base64: encodeBase64(auxiliaryBytes),
    },
  })
  if (
    receipt.view_id !== view
    || receipt.relative_path !== `qa-artifacts/c111b-webgl/${currentPhase()}/${view}.png`
    || !SHA256_PATTERN.test(receipt.sha256)
    || receipt.byte_size !== bytes.byteLength
    || receipt.width < 320
    || receipt.height < 240
    || receipt.auxiliary_relative_path !== `qa-artifacts/c111b-webgl/${currentPhase()}/${view}.auxiliary.png`
    || !SHA256_PATTERN.test(receipt.auxiliary_sha256)
    || receipt.auxiliary_byte_size !== auxiliaryBytes.byteLength
    || receipt.auxiliary_width !== WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX
    || receipt.auxiliary_height !== WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX
    || receipt.auxiliary_pass_ids.join(',') !== 'silhouette,normal,depth,part_id,material_id'
  ) throw new Error('C111B_CAPTURE_RECEIPT_INVALID')
  return receipt
}

function currentPhase(): 'initial' | 'restart' {
  // The native config is authoritative; this DOM-side value is used only to
  // address the fixed capture receipt and is set once per WebView process.
  return (document.documentElement.dataset.forgecadC111bPhase === 'restart' ? 'restart' : 'initial')
}

async function reportSuccess(config: Config, report: Report): Promise<void> {
  if (report.phase !== config.phase) throw new Error('C111B_REPORT_PHASE_INVALID')
  const { invoke } = await import('@tauri-apps/api/core')
  // Reports are only sent after all eight captures.
  await invoke('forgecad_c111b_webview_qa_report', { report })
}

async function reportFailure(phase: 'initial' | 'restart', errorCode: string): Promise<void> {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('forgecad_c111b_webview_qa_report', {
      report: ({ schema_version: SCHEMA, phase, ok: false, error_code: errorCode } satisfies Report),
    })
  } catch {
    // The launcher still records the absence of a success marker. Never mask
    // the original bounded failure with a reporting transport error.
  }
}

async function reportProgress(stage: string): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('forgecad_c111b_webview_qa_progress', { stage })
}

async function waitForHealth(): Promise<void> {
  await waitFor(async () => {
    try {
      // Packaged WebViews are intentionally denied direct loopback access by
      // the Tauri CSP. Reuse the product transport so packaged mode reaches
      // the Rust-owned app-server bridge while browser development keeps its
      // loopback compatibility path.
      const response = await appServerTransport.request('/api/health', { cache: 'no-store' })
      return response.ok ? true : null
    } catch {
      return null
    }
  }, 'C111B_AGENT_HEALTH_TIMEOUT')
}

async function readAgentTurnFailure(): Promise<Error> {
  const root = document.querySelector<HTMLElement>('[data-testid="cad-workbench"]')
  const threadId = root?.dataset.qaAgentThreadId
  const turnId = root?.dataset.qaAgentTurnId
  if (!threadId || !turnId || !STABLE_ID_PATTERN.test(threadId) || !STABLE_ID_PATTERN.test(turnId)) {
    return new Error('C111B_AGENT_TURN_LINEAGE_MISSING')
  }
  try {
    const raw = await appServerTransport.nativeRequest<unknown>('turn/read', {
      schema_version: 'AgentTurnCommand@1',
      command_id: `c111b_qa_turn_read_${Date.now()}`,
      command: { operation: 'read', thread_id: threadId, turn_id: turnId },
    }, { retrySafe: true })
    const envelope = isRecord(raw) && isRecord(raw.result) ? raw.result : raw
    const turn = isRecord(envelope) && isRecord(envelope.turn)
      ? envelope.turn
      : isRecord(envelope) && typeof envelope.turn_id === 'string'
        ? envelope
        : null
    if (!turn) return new Error('C111B_AGENT_TURN_PAYLOAD_INVALID')
    const errorCode = turn && typeof turn.error_code === 'string' ? turn.error_code : null
    if (errorCode && /^[A-Z0-9_]{1,120}$/.test(errorCode)) {
      return new Error(`C111B_AGENT_TURN_${errorCode}`)
    }
    const items = turn && Array.isArray(turn.items) ? turn.items : []
    for (const item of [...items].reverse()) {
      if (!isRecord(item)) continue
      const payload = isRecord(item.payload) ? item.payload : null
      const toolResult = payload && isRecord(payload.tool_result) ? payload.tool_result : null
      const itemErrorCode = [
        payload?.error_code,
        toolResult?.error_code,
        isRecord(payload?.result) ? payload.result.error_code : null,
      ].find((value): value is string => typeof value === 'string' && /^[A-Z0-9_]{1,120}$/.test(value))
      if (itemErrorCode) return new Error(`C111B_AGENT_ITEM_${itemErrorCode}`)
    }
    const turnStatus = turn && typeof turn.status === 'string' ? turn.status : null
    if (turnStatus && /^[a-z_]{1,80}$/.test(turnStatus)) {
      return new Error(`C111B_AGENT_TURN_STATUS_${turnStatus.toUpperCase()}`)
    }
    if (turn?.status === 'failed') return new Error('C111B_AGENT_TURN_FAILED_NO_CODE')
    if (turn?.status === 'completed') return new Error('C111B_AGENT_TURN_COMPLETED_NO_DECISION')
  } catch {
    // Keep the bounded QA failure stable if the diagnostic read itself is not
    // available; it never changes pass/fail authority.
    return new Error('C111B_AGENT_TURN_READ_FAILED')
  }
  return new Error('C111B_AGENT_TURN_ERROR_CODE_MISSING')
}

async function waitForActiveAsset(
  root: HTMLElement,
  errorCode: string,
  previous: string | null = null,
): Promise<{ assetVersionId: string; snapshotRevision: number }> {
  return waitForMutation(() => {
    const assetVersionId = root.dataset.qaActiveAssetVersionId
    const snapshotRevision = Number(root.dataset.qaActiveSnapshotRevision)
    if (previous && assetVersionId === previous) return null
    return assetVersionId && STABLE_ID_PATTERN.test(assetVersionId) && Number.isInteger(snapshotRevision) && snapshotRevision > 0
      ? { assetVersionId, snapshotRevision }
      : null
  }, errorCode)
}

async function waitForRetainedActiveAsset(
  root: HTMLElement,
  previous: string,
): Promise<{ assetVersionId: string; snapshotRevision: number }> {
  return waitForMutation(() => {
    const failed = document.querySelector<HTMLElement>(
      '[role="dialog"][aria-label="添加外观细节"] .surface-adornment-status.failed[data-error-code]:not([data-error-code=""])',
    )
    if (failed?.dataset.errorCode) {
      throw new Error(normalizeC111bAdornmentRetainErrorCode(failed.dataset.errorCode))
    }
    const assetVersionId = root.dataset.qaActiveAssetVersionId
    const snapshotRevision = Number(root.dataset.qaActiveSnapshotRevision)
    if (assetVersionId === previous) return null
    return assetVersionId && STABLE_ID_PATTERN.test(assetVersionId)
      && Number.isInteger(snapshotRevision) && snapshotRevision > 0
      ? { assetVersionId, snapshotRevision }
      : null
  }, 'C111B_AGENT_V2_SNAPSHOT_MISSING')
}

async function waitFor<T>(
  reader: () => T | Promise<T | null> | T | null,
  errorCode: string,
  timeoutMs = MAX_WAIT_MS,
): Promise<T> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const value = await reader()
    if (value !== null && value !== undefined && value !== false) return value as T
    await new Promise((resolve) => window.setTimeout(resolve, POLL_MS))
  }
  throw new Error(errorCode)
}

async function withC111bStageError<T>(
  fallbackCode: Parameters<typeof normalizeC111bStageError>[1],
  run: () => Promise<T>,
): Promise<T> {
  try {
    return await run()
  } catch (caught) {
    throw new Error(normalizeC111bStageError(caught, fallbackCode))
  }
}

function linkPartCandidateSummary(): string {
  const candidates = Array.from(document.querySelectorAll<HTMLButtonElement>('[data-qa-part-role]'))
    .slice(0, 16)
    .map((button) => {
      const role = button.dataset.qaPartRole ?? 'unknown'
      const zones = (button.dataset.qaMaterialZoneIds ?? 'none').replaceAll(' ', '-')
      return `${role}-${zones}`
    })
    .join('_')
    .replace(/[^A-Za-z0-9_-]/g, '_')
  // The native progress command accepts a stable ID with a bounded total
  // length. Leave room for the `agent_link_candidates_` prefix.
  return (candidates || 'none').slice(0, 96)
}

async function waitForMutation<T>(
  reader: () => T | null,
  errorCode: string,
  timeoutMs = MAX_WAIT_MS,
): Promise<T> {
  const immediate = reader()
  if (immediate !== null) return immediate
  return new Promise<T>((resolve, reject) => {
    let done = false
    const observer = new MutationObserver(() => {
      try {
        const value = reader()
        if (value === null || done) return
        done = true
        observer.disconnect()
        window.clearTimeout(timeout)
        resolve(value)
      } catch (error) {
        done = true
        observer.disconnect()
        window.clearTimeout(timeout)
        reject(error)
      }
    })
    observer.observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true })
    const timeout = window.setTimeout(() => {
      if (done) return
      done = true
      observer.disconnect()
      reject(new Error(errorCode))
    }, timeoutMs)
  })
}

function requiredStable(value: string | undefined, errorCode: string): string {
  if (!value || !STABLE_ID_PATTERN.test(value)) throw new Error(errorCode)
  return value
}

function positiveNumber(value: string | undefined, errorCode: string): number {
  const number = Number(value)
  if (!Number.isInteger(number) || number <= 0) throw new Error(errorCode)
  return number
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function decodeBase64(value: string): Uint8Array {
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return bytes
}

function encodeBase64(bytes: Uint8Array): string {
  const chunkSize = 0x8000
  let binary = ''
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize))
  }
  return btoa(binary)
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer
  const digest = await crypto.subtle.digest('SHA-256', buffer)
  return [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, '0')).join('')
}
