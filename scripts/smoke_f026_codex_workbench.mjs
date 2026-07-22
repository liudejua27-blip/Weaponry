#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const DESKTOP_SOURCE = join(ROOT, 'apps', 'desktop', 'src')
const WORKBENCH_SOURCE = join(DESKTOP_SOURCE, 'features', 'cad-workbench')
const output = await mkdtemp(join(tmpdir(), 'forgecad-f026-codex-workbench-'))

try {
  const sources = [
    'AgentConversation.tsx',
    'AgentConversation.smoke.tsx',
    'AgentSelectionCard.tsx',
    'AgentSelectionCard.smoke.tsx',
    'GenerationResultCard.tsx',
    'GenerationResultCard.smoke.tsx',
    'WorkbenchComposer.tsx',
    'WorkbenchComposer.smoke.tsx',
    'SurfaceAdornmentDrawer.tsx',
    'SurfaceAdornmentDrawer.smoke.tsx',
    'ReferenceEvidenceDrawer.tsx',
    'ReferenceEvidenceDrawer.smoke.tsx',
    'WorkbenchSidebar.tsx',
    'WorkbenchSidebar.smoke.tsx',
    'agentBlockoutDisplayState.ts',
    'agentBlockoutPreviewPresentation.ts',
    'agentBlockoutPreviewPresentation.smoke.ts',
    'viewportDockState.ts',
    'viewportDockState.smoke.ts',
    'viewportMeasurementPresentation.ts',
    'viewportMeasurementPresentation.smoke.ts',
  ].map((file) => join(WORKBENCH_SOURCE, file))
  const result = spawnSync(join(ROOT, 'node_modules', '.bin', 'tsc'), [
    '--target', 'ES2022', '--module', 'ESNext', '--moduleResolution', 'Bundler', '--jsx', 'react-jsx',
    '--strict', '--skipLibCheck', '--esModuleInterop', '--allowSyntheticDefaultImports', '--types', 'vite/client',
    '--outDir', output, '--rootDir', DESKTOP_SOURCE,
    ...sources,
    join(DESKTOP_SOURCE, 'shared', 'types.ts'),
    join(DESKTOP_SOURCE, 'shared', 'generated', 'api-types.ts'),
  ], { cwd: ROOT, encoding: 'utf8' })
  if (result.status !== 0) {
    process.stderr.write(result.stdout)
    process.stderr.write(result.stderr)
    process.exit(result.status ?? 1)
  }

  await symlink(join(ROOT, 'node_modules'), join(output, 'node_modules'), 'junction')
  await writeFile(join(output, 'package.json'), '{"type":"module"}\n', 'utf8')
  for (const [file, exportName] of [
    ['AgentConversation.smoke.js', 'runAgentConversationSmoke'],
    ['AgentSelectionCard.smoke.js', 'runAgentSelectionCardSmoke'],
    ['GenerationResultCard.smoke.js', 'runGenerationResultCardSmoke'],
    ['WorkbenchComposer.smoke.js', 'runWorkbenchComposerSmoke'],
    ['SurfaceAdornmentDrawer.smoke.js', 'runSurfaceAdornmentDrawerSmoke'],
    ['ReferenceEvidenceDrawer.smoke.js', 'runReferenceEvidenceDrawerSmoke'],
    ['WorkbenchSidebar.smoke.js', 'runWorkbenchSidebarSmoke'],
    ['agentBlockoutPreviewPresentation.smoke.js', 'runAgentBlockoutPreviewPresentationSmoke'],
    ['viewportDockState.smoke.js', 'runViewportDockStateSmoke'],
    ['viewportMeasurementPresentation.smoke.js', 'runViewportMeasurementPresentationSmoke'],
  ]) {
    const module = await import(pathToFileURL(join(output, 'features', 'cad-workbench', file)).href)
    await module[exportName]()
  }

  const [panel, conversation, selection, previewPresentation, composer, adornment, referenceEvidence, dockState, forgeApi, packagedArmQa, viewport, conceptWorkbench] = await Promise.all([
    readFile(join(WORKBENCH_SOURCE, 'CadWorkbenchPanel.tsx'), 'utf8'),
    readFile(join(WORKBENCH_SOURCE, 'AgentConversation.tsx'), 'utf8'),
    readFile(join(WORKBENCH_SOURCE, 'AgentSelectionCard.tsx'), 'utf8'),
    readFile(join(WORKBENCH_SOURCE, 'agentBlockoutPreviewPresentation.ts'), 'utf8'),
    readFile(join(WORKBENCH_SOURCE, 'WorkbenchComposer.tsx'), 'utf8'),
    readFile(join(WORKBENCH_SOURCE, 'SurfaceAdornmentDrawer.tsx'), 'utf8'),
    readFile(join(WORKBENCH_SOURCE, 'ReferenceEvidenceDrawer.tsx'), 'utf8'),
    readFile(join(WORKBENCH_SOURCE, 'viewportDockState.ts'), 'utf8'),
    readFile(join(DESKTOP_SOURCE, 'shared', 'api', 'forgeApi.ts'), 'utf8'),
    readFile(join(DESKTOP_SOURCE, 'shared', 'api', 'packagedArmWebviewQa.ts'), 'utf8'),
    readFile(join(WORKBENCH_SOURCE, 'ModuleGraphViewport.tsx'), 'utf8'),
    readFile(join(WORKBENCH_SOURCE, 'useConceptWorkbench.ts'), 'utf8'),
  ])

  assert((panel.match(/<ModuleGraphViewport/g) ?? []).length === 1, 'F026 must keep exactly one mounted viewport component')
  assert(
    panel.includes('blockoutGlbBase64={viewportGlb}')
      && panel.includes('blockoutGlbKind={viewportGlbKind}')
      && panel.includes('blockoutShapeProgram={viewportShapeProgram}')
      && panel.includes('blockoutMaterialOverride={viewportShapeProgram ? appearanceMaterialId : null}'),
    'R007B reference/result A/B must feed the selected viewport source into the one existing renderer',
  )
  assert(
    panel.includes('const viewportGlb = referenceViewportActive')
      && panel.includes('const viewportShapeProgram = referenceViewportActive ? null : agentBlockoutShapeProgram')
      && panel.includes('replaceReferenceViewport(null)'),
    'R007B must return to the result by changing inputs instead of replacing the canvas or camera owner',
  )
  const referenceDrawerStart = panel.indexOf('<ReferenceEvidenceDrawer')
  const referenceDrawerEnd = panel.indexOf('/>', referenceDrawerStart)
  assert(
    referenceDrawerStart >= 0
      && referenceDrawerEnd > referenceDrawerStart
      && panel.slice(referenceDrawerStart, referenceDrawerEnd).includes('replaceReferenceViewport(null)'),
    'R007B closing the reference drawer must clear the transient view and return to the current result',
  )
  assert((panel.match(/<WorkbenchComposer/g) ?? []).length === 1, 'F026 must keep one fixed composer')
  assert(
    conceptWorkbench.includes("AGENT_FIRST_WORKBENCH_SELECTION_SCHEMA = 'agent-first-v1'")
      && conceptWorkbench.includes('localStorage.removeItem(ACTIVE_PROJECT_KEY)')
      && conceptWorkbench.includes('直接描述你想生成的机械概念'),
    'F026 first launch must not automatically reopen a pre-Agent legacy project and make the current app look stale',
  )
  assert(!panel.includes('const presentationDirection = kernelResult.plan?.directions[0]'), 'a missing V003 decision must not fall back to the legacy planner first direction')
  assert(panel.includes('state="compatibility_result"'), 'existing legacy previews must remain explicitly labelled as compatibility results')
  assert(panel.includes("presentation.state === 'ready'") && panel.includes('loadSingleResultPreviewGlb'), 'formal ready must come from the sealed V003 decision and load its exact GLB')
  assert(panel.includes('confirmSingleResultPreview') && forgeApi.includes("':preview.glb'") && forgeApi.includes("':confirm'"), 'formal V003 preview and confirm must use the Rust-owned single-result routes')
  assert(panel.includes("singleResultDecisionPresentation.presentation.state === 'processing'"), 'the fixed composer must stay in its sending state while one formal V003 turn is compiling')
  assert(!panel.includes('data-variant-rank') && !panel.includes('Agent 完整外观方向'), 'the workbench must not restore direction-card selectors')
  assert(!conversation.includes('.directions.map(') && conversation.includes('只构建并展示一个当前结果'), 'conversation must not render planner directions and must describe the one-result boundary')
  for (const source of [conversation, selection, previewPresentation]) {
    assert(!source.includes('换一版外观'), 'F026 presentation sources must not expose appearance rotation')
    assert(!source.includes('选择其他方向'), 'F026 presentation sources must not restore direction selection')
  }
  assert(composer.includes('f026-composer-fixed') && composer.includes('<details') && composer.includes('<summary'), 'composer must remain fixed and expose its + actions through one native menu')
  assert(panel.includes('<SurfaceAdornmentDrawer') && panel.includes('surfaceAdornmentDisabledReason'), 'A005 appearance detail entry must be target-gated by the active saved asset, part, and zone')
  assert(panel.includes('agentAssetChangeSet && !surfaceAdornmentOpen'), 'an A005-owned preview must keep its open drawer available so the user can retain or cancel it')
  assert(adornment.includes("status: 'unavailable'") && adornment.includes("status: 'preview_ready'") && adornment.includes('surfaceAdornmentPreviewEndpoint'), 'A005 UI must use explicit preview states and a centralized endpoint contract')
  assert(packagedArmQa.includes('[aria-label="选择部件 连杆护甲"][aria-pressed="true"]'), 'packaged A005 QA must wait for Rust Snapshot part selection before opening the drawer')
  assert(packagedArmQa.includes("['', 'none', 'compiled_agent_preview_pbr', 'compiled_agent_production_pbr']"), 'packaged QA must treat empty and lightweight preview hydration as pending while requiring the production PBR terminal state')
  assert(adornment.includes('细节类型') && adornment.includes('图案') && adornment.includes('强度') && adornment.includes('覆盖区域') && !adornment.includes('Skill'), 'A005 appearance detail UI must remain ordinary-language and constrained')
  assert(
    adornment.includes('<svg')
      && adornment.includes('surface-adornment-design-surface')
      && adornment.includes('data-surface-truth="editor_only"')
      && adornment.includes('真实 PBR 与 GLB'),
    'A005 must use SVG only as a constrained two-dimensional surface editor and keep retained PBR/GLB as the model truth',
  )
  assert(
    panel.includes("measureEnabled={activeTool === 'measure'}")
      && panel.includes('onMeasurePoint={handleMeasurePoint}')
      && panel.includes('data-testid="measurement-overlay"')
      && panel.includes('固定标注'),
    'the existing one-renderer viewport must connect the measure tool to real raycast points and an ephemeral inspection overlay',
  )
  assert(
    viewport.includes('[...moduleRoot.children, ...blockoutRoot.children]')
      && viewport.includes('runtime.controls.enabled = !props.measureEnabled')
      && viewport.includes('data-measure-enabled'),
    'measurement must raycast both legacy and Agent GLB roots through the existing renderer and freeze orbit for its two-click interaction',
  )
  assert(panel.includes('<ReferenceEvidenceDrawer') && panel.includes('referenceEvidenceRequestEpochRef'), 'R007 reference input must be project-scoped and invalidate late responses')
  const referenceAdapterSource = panel.slice(panel.indexOf('const referenceEvidenceAdapter'), panel.indexOf('const navigateAgentAsset'))
  assert(referenceAdapterSource.includes('content_base64: contentBase64') && referenceAdapterSource.includes("media_type: kind === 'glb' ? 'model/gltf-binary' : file.type"), 'R007 must send image and GLB bytes to the zero-version-side-effect evidence endpoint')
  assert(!referenceAdapterSource.includes('api.importAgentGlb('), 'R007 must not create an external AgentAssetVersion before rebuild confirmation')
  assert(
    referenceAdapterSource.includes('const glb = await content.blob.arrayBuffer()')
      && referenceAdapterSource.indexOf('const glb = await content.blob.arrayBuffer()')
        < referenceAdapterSource.lastIndexOf('epoch !== referenceEvidenceRequestEpochRef.current'),
    'R007B reference GLB decoding must re-check the project epoch before updating the shared viewport',
  )
  const referenceCancelSource = referenceAdapterSource.slice(
    referenceAdapterSource.indexOf('cancel: async (changeSetId) => {'),
    referenceAdapterSource.indexOf('loadHistory: async'),
  )
  assert(
    referenceCancelSource.includes('api.getActiveDesign(binding.projectId)')
      && referenceCancelSource.includes('snapshot.preview !== null')
      && !referenceCancelSource.includes('refreshActiveDesign('),
    'R007B close must use read-only Snapshot verification without triggering target hydration before Drawer onClose',
  )
  assert(
    referenceEvidence.includes('const cancelAttemptRef = useRef(0)')
      && referenceEvidence.includes('cancelAttempt !== cancelAttemptRef.current')
      && !referenceEvidence.includes('token !== requestTokenRef.current) return\n      if (result.status'),
    'Drawer cancellation completion must be independent from history/request-token refresh while real context reset still invalidates it',
  )
  assert(
    referenceEvidence.includes('const scopeKey = referenceEvidenceScopeKey(target)')
      && referenceEvidence.includes('const retainAttemptRef = useRef(0)')
      && referenceEvidence.includes('retainAttempt !== retainAttemptRef.current')
      && referenceEvidence.includes("setStatus('preview_stale')"),
    'Drawer must preserve confirmed lineage across its own base advance while blocking an old preview after an external base advance',
  )
  assert(composer.includes('reference_guided_rebuild'), 'composer must advertise the actual R007 reference-guided rebuild capability')
  assert(referenceEvidence.includes('来源说明') && referenceEvidence.includes('使用授权 / 权利声明') && referenceEvidence.includes('缺失视角'), 'R007 must collect provenance, license boundary and missing-view uncertainty')
  assert(referenceEvidence.includes('不会成为可编辑模型') && referenceEvidence.includes('新的可编辑版本'), 'R007 must separate read-only reference evidence from newly rebuilt geometry')
  assert(referenceEvidence.includes('保留') && referenceEvidence.includes('主动改变') && referenceEvidence.includes('仍未知'), 'R007B must present the three bounded reference/rebuild comparison columns')
  assert(referenceEvidence.includes('单张图片只约束') && referenceEvidence.includes('保真度上限'), 'R007B must make single-image and missing-view fidelity limits explicit')
  assert(!referenceEvidence.includes('相似度分数') || referenceEvidence.includes('不显示相似度分数'), 'R007B must not present a similarity score')
  assert(dockState.includes("'docked' | 'focus'") && !dockState.includes('canvas:'), 'viewport dock state must remain a pure dock/focus presentation state')

  console.log(JSON.stringify({
    ok: true,
    task: 'FGC-F026',
    assertions: [
      'single_generation_result_card',
      'fixed_composer_plus_menu',
      'docked_focus_presentation_state',
      'single_viewport_component',
      'no_direction_selection_or_appearance_rotation',
    ],
  }, null, 2))
} finally {
  await rm(output, { recursive: true, force: true })
}

function assert(condition, message) {
  if (!condition) throw new Error(message)
}
