import { readAgentAssetGpuPbrSourceGlbSha256 } from './agentRenderPresentationLoader.js'

const SOURCE_SHA256 = 'a'.repeat(64)

export function runAgentRenderPresentationLoaderSmoke(): void {
  const valid = fakeViewport()
  assert(
    readAgentAssetGpuPbrSourceGlbSha256(valid) === SOURCE_SHA256,
    'native Agent concept rendering must bind the exact mounted GLB hash',
  )
  assert(
    readAgentAssetGpuPbrSourceGlbSha256(fakeViewport()) === SOURCE_SHA256,
    'native Agent concept rendering must accept only the mounted workbench PBR renderer',
  )
  assert(
    readAgentAssetGpuPbrSourceGlbSha256(fakeViewport({ pbrRendererId: 'forgecad-agent-software-raster@1' })) === null,
    'native Agent concept rendering must reject the legacy software renderer before capture',
  )
  assert(
    readAgentAssetGpuPbrSourceGlbSha256(fakeViewport({ blockoutRenderSource: 'shape_program_fallback' })) === null,
    'native Agent concept rendering must not fall back to ShapeProgram display materials',
  )
  assert(
    readAgentAssetGpuPbrSourceGlbSha256(fakeViewport({ blockoutLoadState: 'loading' })) === null,
    'native Agent concept rendering must wait for the PBR GLB to finish loading',
  )
  assert(
    readAgentAssetGpuPbrSourceGlbSha256(null) === null,
    'a missing viewport must fail closed instead of invoking the software renderer',
  )
}

function fakeViewport(overrides: Record<string, string> = {}): HTMLElement {
  return {
    dataset: {
      blockoutLoadState: 'ready',
      blockoutRenderSource: 'glb_pbr',
      blockoutGlbSha256: SOURCE_SHA256,
      pbrRendererId: 'forgecad-workbench-pbr@1',
      ...overrides,
    },
  } as unknown as HTMLElement
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
