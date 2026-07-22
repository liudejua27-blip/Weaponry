#!/usr/bin/env node

/**
 * Source-bound regression proof for R007B's transient reference-image mode.
 *
 * It intentionally starts no desktop app, browser, sidecar, server or port.
 * The workbench is permitted to show a user-authorized reference image in its
 * existing renderer, but that presentation must remain a project-scoped,
 * read-only input.  It cannot become a ShapeProgram, Snapshot, version,
 * quality report or export source.
 */

import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import process from 'node:process'

const ROOT = resolve(import.meta.dirname, '..')
const PANEL_PATH = resolve(ROOT, 'apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx')
const VIEWPORT_PATH = resolve(ROOT, 'apps/desktop/src/features/cad-workbench/ModuleGraphViewport.tsx')
const DRAWER_PATH = resolve(ROOT, 'apps/desktop/src/features/cad-workbench/ReferenceEvidenceDrawer.tsx')
const QA_PATH = resolve(ROOT, 'apps/desktop/src/shared/api/packagedArmWebviewQa.ts')
const SCHEMA = 'R007BReferenceImageViewportRegressionSmoke@1'

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

function containsAny(source, tokens) {
  return tokens.some(token => source.includes(token))
}

async function main() {
  const [panel, viewport, drawer, qa] = await Promise.all([
    readFile(PANEL_PATH, 'utf8'),
    readFile(VIEWPORT_PATH, 'utf8'),
    readFile(DRAWER_PATH, 'utf8'),
    readFile(QA_PATH, 'utf8'),
  ])

  // The panel owns an image reference only as a transient, project-scoped
  // display input. The main result GLB and ShapeProgram are left intact.
  requireFact(panel.includes("kind: 'image'"), 'IMAGE_REFERENCE_VIEWPORT_VARIANT_MISSING')
  for (const field of ['projectId', 'evidenceId', 'sourceObjectSha256', 'referenceClass', 'imageUrl']) {
    requireFact(panel.includes(field), `IMAGE_REFERENCE_IDENTITY_FIELD_MISSING_${field}`)
  }
  requireFact(
    panel.includes("const referenceViewportActive = referenceViewport?.projectId === concept.project?.project_id"),
    'IMAGE_REFERENCE_PROJECT_SCOPE_GUARD_MISSING',
  )
  const routing = boundedSlice(
    panel,
    'const referenceViewportActive = referenceViewport?.projectId === concept.project?.project_id',
    '\n  const materialPreselectionSource',
    'IMAGE_REFERENCE_VIEWPORT_ROUTING',
  )
  requireFact(
    routing.includes("referenceViewport?.kind === 'image'")
      && routing.includes('const viewportReferenceImage = referenceViewportActive'),
    'IMAGE_REFERENCE_ROUTING_MISSING',
  )
  requireFact(
    routing.includes("const viewportShapeProgram = referenceViewportActive ? null : agentBlockoutShapeProgram"),
    'IMAGE_REFERENCE_MUST_NOT_COMBINE_WITH_RESULT_SHAPE_PROGRAM',
  )
  requireFact(
    panel.includes('referenceImage={viewportReferenceImage}'),
    'IMAGE_REFERENCE_NOT_PASSED_TO_SINGLE_VIEWPORT',
  )
  requireFact(count(panel, '<ModuleGraphViewport') === 1, 'IMAGE_REFERENCE_CREATED_SECOND_VIEWPORT')

  // A project change, result return and explicit close must all clear the
  // transient state. A late async load is rejected by both epoch and project.
  const projectReset = boundedSlice(
    panel,
    'useEffect(() => {\n    openConversationProject(concept.project?.project_id ?? null)',
    '\n  }, [concept.project?.project_id',
    'IMAGE_REFERENCE_PROJECT_RESET',
  )
  requireFact(projectReset.includes('replaceReferenceViewport(null)'), 'IMAGE_REFERENCE_PROJECT_RESET_MISSING')
  const adapter = boundedSlice(
    panel,
    'const referenceEvidenceAdapter',
    '\n\n  const navigateAgentAsset',
    'IMAGE_REFERENCE_ADAPTER',
  )
  const imageView = boundedSlice(
    adapter,
    'viewReferenceImage: async',
    '\n    viewResult:',
    'IMAGE_REFERENCE_VIEW_OPERATION',
  )
  requireFact(
    imageView.includes('api.loadReferenceEvidenceContent(target.projectId, evidence.evidenceId)'),
    'IMAGE_REFERENCE_CONTENT_MUST_USE_PROJECT_AND_EVIDENCE_ID',
  )
  requireFact(
    imageView.includes('URL.createObjectURL') && imageView.includes("kind: 'image'"),
    'IMAGE_REFERENCE_OBJECT_URL_OR_KIND_MISSING',
  )
  requireFact(
    imageView.includes('epoch !== referenceEvidenceRequestEpochRef.current')
      && imageView.includes('concept.project?.project_id !== target.projectId'),
    'IMAGE_REFERENCE_LATE_PROJECT_RESPONSE_GUARD_MISSING',
  )
  requireFact(
    imageView.includes('sourceObjectSha256: evidence.contentSha256')
      && imageView.includes('referenceClass:'),
    'IMAGE_REFERENCE_SOURCE_HASH_OR_CLASS_NOT_BOUND',
  )
  requireFact(
    imageView.includes('replaceReferenceViewport({')
      && imageView.includes('imageUrl'),
    'IMAGE_REFERENCE_TRANSIENT_VIEW_STATE_NOT_SET',
  )
  requireFact(
    imageView.includes('参考图片无法读取；已回到当前结果。'),
    'IMAGE_REFERENCE_LOAD_FAILURE_NOT_EXPLICIT',
  )
  for (const forbiddenWrite of [
    'createAgentAsset', 'createChangeSet', 'previewChangeSet', 'confirmChangeSet',
    'refreshActiveDesign(', 'setBlockoutShapeProgram(', 'setBlockoutGlb(',
    'createQuality', 'exportAgent', 'downloadAgent',
  ]) {
    requireFact(!imageView.includes(forbiddenWrite), `IMAGE_REFERENCE_FORBIDDEN_PRODUCT_WRITE_${forbiddenWrite}`)
  }
  const viewResult = boundedSlice(adapter, '\n    viewResult:', '\n  }), [', 'IMAGE_REFERENCE_RETURN_TO_RESULT')
  requireFact(viewResult.includes('replaceReferenceViewport(null)'), 'IMAGE_REFERENCE_RETURN_TO_RESULT_MISSING')
  const closePaths = boundedSlice(panel, '<ReferenceEvidenceDrawer', '/>', 'IMAGE_REFERENCE_DRAWER_CLOSE')
  requireFact(closePaths.includes('replaceReferenceViewport(null)'), 'IMAGE_REFERENCE_DRAWER_CLOSE_DOES_NOT_CLEAR')

  // The drawer must expose image viewing through its existing adapter and
  // still explain that reference material remains read-only.
  requireFact(drawer.includes('viewReferenceImage?:'), 'IMAGE_REFERENCE_DRAWER_ADAPTER_METHOD_MISSING')
  requireFact(drawer.includes('查看参考图'), 'IMAGE_REFERENCE_VISIBLE_ACTION_MISSING')
  requireFact(drawer.includes('不会成为可编辑模型'), 'IMAGE_REFERENCE_READ_ONLY_COPY_MISSING')
  requireFact(!drawer.includes('new THREE.WebGLRenderer('), 'DRAWER_CREATED_WEBGL_RENDERER')

  // The viewport takes the image as a display layer in its one persistent
  // renderer. Source identity and failure state are exposed to the host so the
  // packaged test can reject a stale/unknown/failed texture rather than
  // accidentally accepting the prior result canvas.
  requireFact(viewport.includes('referenceImage:'), 'VIEWPORT_REFERENCE_IMAGE_PROP_MISSING')
  requireFact(
    viewport.includes("host.dataset.referenceDisplayMode = 'reference_image'")
      && viewport.includes("host.dataset.referenceDisplayMode = 'result'")
      && viewport.includes("host.dataset.referenceDisplayMode = 'failed'"),
    'VIEWPORT_REFERENCE_DISPLAY_MODE_MISSING',
  )
  for (const attribute of [
    'data-reference-display-mode',
    'data-reference-evidence-id',
    'data-reference-source-object-sha256',
    'data-reference-class',
    'data-reference-image-load-state',
  ]) {
    requireFact(viewport.includes(attribute), `VIEWPORT_REFERENCE_ATTRIBUTE_MISSING_${attribute}`)
  }
  requireFact(count(viewport, 'new THREE.WebGLRenderer(') === 1, 'VIEWPORT_REFERENCE_CREATED_SECOND_RENDERER')
  requireFact(
    containsAny(viewport, ['new THREE.TextureLoader(', 'TextureLoader().load(', 'textureLoader.load(']),
    'VIEWPORT_REFERENCE_TEXTURE_LOADER_MISSING',
  )
  const referenceEffect = boundedSlice(
    viewport,
    'useEffect(() => {\n    const runtime = runtimeRef.current\n    const host = hostRef.current\n    if (!runtime || !host) return\n    const referenceImage = props.referenceImage',
    '\n\n  const graphHash',
    'VIEWPORT_REFERENCE_EFFECT',
  )
  requireFact(
    referenceEffect.includes('sourceObjectSha256') && referenceEffect.includes('evidenceId'),
    'VIEWPORT_REFERENCE_EFFECT_IDENTITY_UNBOUND',
  )
  requireFact(
    referenceEffect.includes('setReferenceImageLoadState') && referenceEffect.includes("'failed'"),
    'VIEWPORT_REFERENCE_EFFECT_FAILURE_STATE_MISSING',
  )
  requireFact(
    containsAny(referenceEffect, ['clearReferenceImage', 'referenceImageRoot.clear()', 'referenceImageRoot.remove', 'clearObjectChildren(runtime.referenceImageRoot)']),
    'VIEWPORT_REFERENCE_FAILURE_OR_RETURN_DOES_NOT_CLEAR_LAYER',
  )
  requireFact(
    !referenceEffect.includes('new THREE.WebGLRenderer(')
      && !referenceEffect.includes('ShapeProgram')
      && !referenceEffect.includes('fetch('),
    'VIEWPORT_REFERENCE_EFFECT_ESCAPES_READ_ONLY_PRESENTATION_BOUNDARY',
  )

  // Existing GLB reload contract remains true: only an actual GLB input
  // change reloads the result; reference image UI does not add a second GLTF
  // loader or mutate the dependency model.
  const gltfEffect = boundedSlice(
    viewport,
    'useEffect(() => {\n    const runtime = runtimeRef.current\n    if (!runtime) return\n    restoreModuleGraphPresentation(runtime, propsRef.current)',
    '\n\n  useEffect(() => {',
    'RESULT_GLB_LOAD_EFFECT',
  )
  requireFact(gltfEffect.includes('props.blockoutGlbBase64'), 'RESULT_GLB_RELOAD_INPUT_MISSING')
  requireFact(gltfEffect.includes('let cancelled = false') && gltfEffect.includes('cancelled = true'), 'RESULT_GLB_CANCELLATION_GUARD_MISSING')
  requireFact(
    gltfEffect.includes('if (props.referenceImage)')
      && gltfEffect.includes('referenceImageIdentity'),
    'REFERENCE_IMAGE_RESULT_ROUTE_OR_RETURN_RELOAD_MISSING',
  )
  requireFact(
    count(gltfEffect, "import('three/examples/jsm/loaders/GLTFLoader.js')") === 1,
    'REFERENCE_IMAGE_MUST_NOT_ADD_A_SECOND_RESULT_GLB_LOADER',
  )

  // QA must use the same already-mounted viewport capture path. It may save
  // evidence bytes, but it cannot construct a renderer itself.
  requireFact(qa.includes("'r007b_reference_png'"), 'QA_REFERENCE_IMAGE_CAPTURE_KIND_MISSING')
  requireFact(qa.includes('captureRenderedViewportScreenshot(referenceViewport'), 'QA_REFERENCE_IMAGE_CAPTURE_NOT_FROM_MOUNTED_VIEWPORT')
  requireFact(count(qa, 'new THREE.WebGLRenderer(') === 0, 'QA_REFERENCE_IMAGE_CREATED_SECOND_RENDERER')
  requireFact(
    qa.includes('const insidePanel = !isContactSheet || localX < 136')
      && qa.includes('const onFlowline = insidePanel')
      && qa.includes('const onJoint = insidePanel'),
    'QA_CONTACT_SHEET_FULL_DIVIDER_EVIDENCE_MISSING',
  )
  // Rust's local analyzer accepts a contact sheet only when a complete
  // background divider is at least max(floor(width / 30), 2) pixels wide and
  // detected foreground exists on both sides. Keep this arithmetic here so a
  // future visual-only tweak cannot silently reintroduce a 20 px divider for
  // the fixed 640 px QA fixture (whose Rust requirement is 21 px).
  const contactSheetGenerator = boundedSlice(
    qa,
    'const width = 640',
    'const compressed = new Uint8Array',
    'QA_CONTACT_SHEET_GENERATOR',
  )
  const width = Number(contactSheetGenerator.match(/const width = (\d+)/)?.[1])
  const stride = Number(contactSheetGenerator.match(/Math\.floor\(x \/ (\d+)\)/)?.[1])
  const panelWidth = Number(contactSheetGenerator.match(/localX < (\d+)/)?.[1])
  requireFact(
    Number.isInteger(width) && Number.isInteger(stride) && Number.isInteger(panelWidth)
      && width > 0 && stride > panelWidth && width % stride === 0,
    'QA_CONTACT_SHEET_LAYOUT_CONSTANTS_INVALID',
  )
  const dividerWidth = stride - panelWidth
  const rustMinimumGap = Math.max(Math.floor(width / 30), 2)
  requireFact(
    dividerWidth >= rustMinimumGap,
    'QA_CONTACT_SHEET_DIVIDER_TOO_NARROW_FOR_RUST_GATE',
  )
  const boundedMutationWait = boundedSlice(
    qa,
    'async function waitForMutation<T>',
    '\n\nasync function reportSuccess',
    'QA_BOUNDED_MUTATION_WAIT',
  )
  requireFact(
    boundedMutationWait.includes('window.setTimeout')
      && boundedMutationWait.includes('MAX_WAIT_MS')
      && boundedMutationWait.includes('reject(new Error(errorCode))')
      && boundedMutationWait.includes('window.setInterval(check, POLL_MS)'),
    'QA_MUTATION_WAIT_CAN_HANG_FOREVER',
  )
  requireFact(
    qa.includes("downloadedBlob = await waitFor<Blob>(() => downloadedBlob, 'QA_V3_GLB_DOWNLOAD_BLOB_MISSING')")
      && qa.includes("await reportProgress('v3_export_download_clicked')"),
    'QA_GLB_DOWNLOAD_DOES_NOT_WAIT_FOR_REAL_BLOB_BOUNDARY',
  )

  process.stdout.write(`${JSON.stringify({
    schema_version: SCHEMA,
    status: 'pass',
    assertions: {
      transient_read_only_image_input: true,
      production_visible_in_unique_renderer: true,
      project_close_result_cleanup: true,
      source_hash_and_failed_load_explicit: true,
      contact_sheet_full_divider_evidence: true,
      bounded_mutation_wait: true,
      current_glb_download_blob_boundary: true,
      result_glb_reload_contract_preserved: true,
      no_shape_program_snapshot_version_quality_or_export_write: true,
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
