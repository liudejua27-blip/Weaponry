#!/usr/bin/env node

/**
 * Source-bound regression proof for the packaged robotic-arm viewport.
 *
 * This smoke intentionally starts no browser, app or sidecar. It binds three
 * failure-prone policies to the checked-in React implementation:
 *  - a confirmed asset still requests production GLB after preview failure;
 *  - presentation-only array identity changes do not restart GLTF loading;
 *  - QA pixels are read in the one renderer's render callback.
 */

import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import process from 'node:process'

const ROOT = resolve(import.meta.dirname, '..')
const CAD_PATH = resolve(ROOT, 'apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx')
const VIEWPORT_PATH = resolve(ROOT, 'apps/desktop/src/features/cad-workbench/ModuleGraphViewport.tsx')
const QA_PATH = resolve(ROOT, 'apps/desktop/src/shared/api/packagedArmWebviewQa.ts')
const SCHEMA = 'ForgeCADArmViewportRegressionSmoke@1'
const CAPTURE_EVENT = 'forgecad:qa-capture-viewport@1'

function fail(code) {
  throw new Error(code)
}

function requireFact(condition, code) {
  if (!condition) fail(code)
}

function count(source, token) {
  return source.split(token).length - 1
}

function boundedSlice(source, startToken, endToken, code) {
  const start = source.indexOf(startToken)
  requireFact(start >= 0, `${code}_START_MISSING`)
  const end = source.indexOf(endToken, start + startToken.length)
  requireFact(end > start, `${code}_END_MISSING`)
  return source.slice(start, end)
}

function dependencyValues(props) {
  // This exact list is source-bound below. It models React's Object.is
  // dependency comparison without importing or mounting the full viewport.
  return [
    props.blockoutGlbBase64,
    props.blockoutGlbKind,
    props.blockoutShapeProgram,
    props.blockoutMaterialOverride,
  ]
}

function dependencyChanged(previous, next) {
  return previous.length !== next.length
    || previous.some((value, index) => !Object.is(value, next[index]))
}

async function main() {
  const [cad, viewport, qa] = await Promise.all([
    readFile(CAD_PATH, 'utf8'),
    readFile(VIEWPORT_PATH, 'utf8'),
    readFile(QA_PATH, 'utf8'),
  ])

  const hydration = boundedSlice(
    cad,
    'const blockoutDisplayRequestId = hydrateBlockoutDisplay(projectId, {',
    '\n      return response.data',
    'CONFIRMED_ASSET_HYDRATION',
  )
  const previewCall = hydration.indexOf('api.loadAgentAssetPreviewGlb(version.asset_version_id)')
  const recoveryCatch = hydration.indexOf('}).catch(async () => {', previewCall)
  requireFact(previewCall >= 0 && recoveryCatch > previewCall, 'PREVIEW_FAILURE_RECOVERY_MISSING')
  const recovery = hydration.slice(recoveryCatch)
  const productionCall = recovery.indexOf('api.loadAgentAssetProductionGlb(version.asset_version_id)')
  const profileCheck = recovery.indexOf("production.artifactProfileId !== 'production_concept'")
  const productionCommit = recovery.indexOf("'compiled_agent_production_pbr'")
  requireFact(productionCall >= 0, 'PREVIEW_FAILURE_DID_NOT_REQUEST_PRODUCTION')
  requireFact(profileCheck > productionCall, 'RECOVERED_PRODUCTION_PROFILE_NOT_CHECKED')
  requireFact(productionCommit > profileCheck, 'RECOVERED_PRODUCTION_NOT_COMMITTED_TO_VIEWPORT')
  requireFact(
    recovery.includes('isImportedReference || !isCurrentActiveDesignRequest(requestId)'),
    'PREVIEW_FAILURE_RECOVERY_REQUEST_GUARD_MISSING',
  )
  requireFact(
    recovery.includes('预览与生产 PBR GLB 均不可读取'),
    'PREVIEW_AND_PRODUCTION_FAILURE_NOT_EXPLICIT',
  )

  const loadEffectStart = viewport.indexOf(
    'useEffect(() => {\n    const runtime = runtimeRef.current\n    if (!runtime) return\n    restoreModuleGraphPresentation(runtime, propsRef.current)',
  )
  requireFact(loadEffectStart >= 0, 'BLOCKOUT_GLTF_EFFECT_MISSING')
  const loadEffectEnd = viewport.indexOf('\n\n  useEffect(() => {', loadEffectStart + 20)
  requireFact(loadEffectEnd > loadEffectStart, 'BLOCKOUT_GLTF_EFFECT_END_MISSING')
  const loadEffect = viewport.slice(loadEffectStart, loadEffectEnd)
  requireFact(loadEffect.includes("import('three/examples/jsm/loaders/GLTFLoader.js')"), 'BLOCKOUT_GLTF_LOADER_MISSING')
  requireFact(loadEffect.includes('let cancelled = false'), 'BLOCKOUT_GLTF_CANCELLATION_GUARD_MISSING')
  requireFact(loadEffect.includes('cancelled = true'), 'BLOCKOUT_GLTF_CLEANUP_MISSING')

  const dependencyBlock = boundedSlice(
    loadEffect,
    '  }, [',
    '\n  ])',
    'BLOCKOUT_GLTF_DEPENDENCIES',
  )
  for (const required of [
    'props.blockoutGlbBase64',
    'props.blockoutGlbKind',
    'props.blockoutShapeProgram',
    'props.blockoutMaterialOverride',
  ]) {
    requireFact(dependencyBlock.includes(required), `BLOCKOUT_GLTF_DEPENDENCY_MISSING_${required}`)
  }
  for (const presentationOnly of [
    'props.hiddenAgentPartIds',
    'props.lockedAgentPartIds',
    'props.selectedAgentPartId',
    'props.isolatedAgentPartId',
  ]) {
    requireFact(!dependencyBlock.includes(presentationOnly), `BLOCKOUT_GLTF_PRESENTATION_DEPENDENCY_${presentationOnly}`)
  }
  const presentationEffectStart = loadEffectEnd + 2
  const presentationEffectEnd = viewport.indexOf('\n\n  useEffect(() => {', presentationEffectStart + 20)
  requireFact(presentationEffectEnd > presentationEffectStart, 'BLOCKOUT_PRESENTATION_EFFECT_END_MISSING')
  const presentationEffect = viewport.slice(presentationEffectStart, presentationEffectEnd)
  requireFact(
    presentationEffect.includes('applyAgentBlockoutMeshVisualState(child, props)'),
    'BLOCKOUT_PRESENTATION_UPDATE_MISSING',
  )
  requireFact(
    presentationEffect.includes('props.hiddenAgentPartIds')
      && presentationEffect.includes('props.lockedAgentPartIds'),
    'BLOCKOUT_PRESENTATION_ARRAY_DEPENDENCIES_MISSING',
  )
  requireFact(
    !presentationEffect.includes('GLTFLoader') && !presentationEffect.includes('let cancelled = false'),
    'BLOCKOUT_PRESENTATION_EFFECT_RESTARTS_GLTF_LOAD',
  )

  const stableGlb = 'sealed-production-glb'
  const stableShape = { schema_version: 'ShapeProgram@1' }
  const stableMaterial = { material_id: 'mat_service_blue' }
  const firstProps = {
    blockoutGlbBase64: stableGlb,
    blockoutGlbKind: 'compiled_agent_production_pbr',
    blockoutShapeProgram: stableShape,
    blockoutMaterialOverride: stableMaterial,
    hiddenAgentPartIds: ['part_a'],
    lockedAgentPartIds: ['part_b'],
  }
  const rerenderProps = {
    ...firstProps,
    hiddenAgentPartIds: [...firstProps.hiddenAgentPartIds],
    lockedAgentPartIds: [...firstProps.lockedAgentPartIds],
  }
  requireFact(
    !dependencyChanged(dependencyValues(firstProps), dependencyValues(rerenderProps)),
    'PRESENTATION_ARRAY_IDENTITY_RESTARTED_GLTF_LOAD',
  )
  requireFact(
    dependencyChanged(
      dependencyValues(firstProps),
      dependencyValues({ ...rerenderProps, blockoutGlbBase64: 'new-sealed-production-glb' }),
    ),
    'NEW_GLB_DID_NOT_RESTART_GLTF_LOAD',
  )

  requireFact(count(viewport, 'new THREE.WebGLRenderer(') === 1, 'VIEWPORT_RENDERER_CONSTRUCTOR_NOT_UNIQUE')
  requireFact(count(qa, 'new THREE.WebGLRenderer(') === 0, 'QA_CREATED_SECOND_RENDERER')
  requireFact(count(viewport, `'${CAPTURE_EVENT}'`) === 1, 'VIEWPORT_CAPTURE_EVENT_IDENTITY_DRIFT')
  requireFact(count(qa, `'${CAPTURE_EVENT}'`) === 1, 'QA_CAPTURE_EVENT_IDENTITY_DRIFT')
  const renderCallback = boundedSlice(
    viewport,
    'const render = () => {',
    '\n    const scheduleRender = () => {',
    'SAME_FRAME_CAPTURE',
  )
  const renderCall = renderCallback.indexOf('renderer.render(scene, camera)')
  const readPixels = renderCallback.indexOf('context.readPixels(')
  requireFact(renderCall >= 0 && readPixels > renderCall, 'CAPTURE_NOT_READ_AFTER_SAME_RENDER_CALL')
  requireFact(renderCallback.includes('const capture = pendingQaCapture'), 'CAPTURE_PENDING_REQUEST_NOT_CONSUMED')
  requireFact(renderCallback.includes('pendingQaCapture = null'), 'CAPTURE_REQUEST_NOT_SINGLE_SHOT')
  requireFact(
    viewport.includes('host.addEventListener(FORGECAD_QA_VIEWPORT_CAPTURE_EVENT, onQaViewportCapture)')
      && viewport.includes('host.removeEventListener(FORGECAD_QA_VIEWPORT_CAPTURE_EVENT, onQaViewportCapture)'),
    'CAPTURE_EVENT_LIFECYCLE_INCOMPLETE',
  )
  requireFact(
    qa.includes('viewport.dispatchEvent(new CustomEvent<QaViewportCaptureRequest>(QA_VIEWPORT_CAPTURE_EVENT'),
    'QA_CAPTURE_NOT_DISPATCHED_TO_EXISTING_VIEWPORT',
  )

  const viewportWait = boundedSlice(
    qa,
    'async function waitForViewport(errorCode: string): Promise<HTMLElement> {',
    '\n\nfunction viewportProfileErrorCode(',
    'QA_VIEWPORT_WAIT',
  )
  const observeCall = viewportWait.indexOf('observer.observe(document.documentElement')
  const afterObserveRead = viewportWait.indexOf('const afterObserve = classify(readViewport())')
  requireFact(observeCall >= 0, 'QA_VIEWPORT_MUTATION_OBSERVER_MISSING')
  requireFact(afterObserveRead > observeCall, 'QA_VIEWPORT_POST_OBSERVE_RECHECK_MISSING')
  requireFact(
    viewportWait.includes('deadline = setTimeout(() => settle(new Error(errorCode)), 180_000)'),
    'QA_VIEWPORT_LOCAL_DEADLINE_MISSING',
  )
  requireFact(viewportWait.includes('if (settled) return'), 'QA_VIEWPORT_SETTLE_GUARD_MISSING')
  requireFact(viewportWait.includes('clearTimeout(deadline)'), 'QA_VIEWPORT_DEADLINE_CLEANUP_MISSING')

  process.stdout.write(`${JSON.stringify({
    schema_version: SCHEMA,
    status: 'pass',
    confirmed_asset_preview_failure: {
      production_requested: true,
      production_profile_checked: true,
      request_identity_guarded: true,
      both_failures_explicit: true,
    },
    gltf_load_dependency_stability: {
      equivalent_hidden_locked_array_rerender_restarts_load: false,
      production_glb_change_restarts_load: true,
      cancellation_guard_retained: true,
    },
    same_frame_capture: {
      renderer_constructor_count: 1,
      qa_created_renderer: false,
      read_pixels_after_render: true,
      capture_event_lifecycle_complete: true,
    },
    viewport_wait: {
      post_observe_recheck: true,
      local_fail_closed_deadline: true,
      single_settlement_guard: true,
    },
  })}\n`)
}

main().catch((error) => {
  process.stderr.write(`${JSON.stringify({
    schema_version: SCHEMA,
    status: 'fail',
    error: error instanceof Error ? error.message : String(error),
  })}\n`)
  process.exitCode = 1
})
