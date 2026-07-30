import {
  WORKBENCH_PBR_RENDERER_ID,
  WORKBENCH_PBR_RENDER_MANIFEST_SHA256,
  WORKBENCH_PBR_CAPTURE_HEIGHT_PX,
  WORKBENCH_PBR_CAPTURE_WIDTH_PX,
  WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX,
  WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX,
  WORKBENCH_PBR_AUXILIARY_PASS_HEIGHT_PX,
  WORKBENCH_PBR_AUXILIARY_PASS_WIDTH_PX,
  fixedWorkbenchPbrCaptureViews,
  readWorkbenchPbrViewportIdentity,
} from './workbenchPbrCapture.js'

const SOURCE_SHA256 = 'a'.repeat(64)
const ENVIRONMENT_SHA256 = 'b'.repeat(64)

export function runWorkbenchPbrCaptureSmoke(): void {
  const viewport = fakeViewport()
  const identity = readWorkbenchPbrViewportIdentity(viewport, SOURCE_SHA256)
  assert(identity.renderer_id === WORKBENCH_PBR_RENDERER_ID, 'capture identity must name the one workbench PBR renderer')
  assert(identity.source_glb_sha256 === SOURCE_SHA256, 'capture identity must bind the exact compiled GLB hash')
  assert(identity.visual_environment_sha256 === ENVIRONMENT_SHA256, 'capture identity must bind the visual environment hash')
  assert(identity.render_manifest_sha256 === WORKBENCH_PBR_RENDER_MANIFEST_SHA256, 'capture identity must bind the Rust-owned render manifest')
  assert(WORKBENCH_PBR_CAPTURE_WIDTH_PX === 640 && WORKBENCH_PBR_CAPTURE_HEIGHT_PX === 640, 'visual comparison captures must use one fixed 640-square renderer resolution')
  assert(
    WORKBENCH_PBR_AUXILIARY_PASS_WIDTH_PX === 320
      && WORKBENCH_PBR_AUXILIARY_PASS_HEIGHT_PX === 320
      && WORKBENCH_PBR_AUXILIARY_CAPTURE_WIDTH_PX === 960
      && WORKBENCH_PBR_AUXILIARY_CAPTURE_HEIGHT_PX === 640,
    'same-renderer diagnostic contact sheet must preserve five fixed 320-square GPU pass tiles',
  )
  assert(
    fixedWorkbenchPbrCaptureViews().join(',') === 'turntable_000,turntable_045,turntable_090,turntable_135,turntable_180,turntable_225,turntable_270,turntable_315',
    'general capture views must use the generic turntable eight, never C111 fixture views',
  )

  expectFailure(() => readWorkbenchPbrViewportIdentity(fakeViewport({ blockoutGlbSha256: 'c'.repeat(64) }), SOURCE_SHA256))
  expectFailure(() => readWorkbenchPbrViewportIdentity(fakeViewport({ blockoutRenderSource: 'shape_program_fallback' }), SOURCE_SHA256))
  expectFailure(() => readWorkbenchPbrViewportIdentity(fakeViewport({ blockoutGlbKind: 'external_reference' }), SOURCE_SHA256))
  expectFailure(() => readWorkbenchPbrViewportIdentity(fakeViewport({ pbrRendererId: 'forgecad-agent-software-raster@1' }), SOURCE_SHA256))
}

function fakeViewport(overrides: Record<string, string> = {}): HTMLElement {
  return {
    dataset: {
      pbrRendererId: WORKBENCH_PBR_RENDERER_ID,
      pbrRenderManifestSha256: WORKBENCH_PBR_RENDER_MANIFEST_SHA256,
      blockoutLoadState: 'ready',
      blockoutRenderSource: 'glb_pbr',
      blockoutGlbKind: 'compiled_agent_production_pbr',
      blockoutGlbSha256: SOURCE_SHA256,
      outputColorSpace: 'srgb',
      toneMapping: 'aces_filmic',
      visualEnvironmentId: 'env_forgecad_room_studio_v2',
      visualEnvironmentSha256: ENVIRONMENT_SHA256,
      ...overrides,
    },
  } as unknown as HTMLElement
}

function expectFailure(run: () => unknown): void {
  try {
    run()
  } catch {
    return
  }
  throw new Error('expected a lineage mismatch to fail closed')
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}
