import { createSurfaceAdornmentAdapter } from './surfaceAdornmentAdapterLoader.js'

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

function assertEqual(actual: number, expected: number, message: string): void {
  if (actual !== expected) throw new Error(`${message}; expected ${expected}, got ${actual}`)
}

export async function runSurfaceAdornmentAdapterLoaderSmoke(): Promise<void> {
  const calls: string[] = []
  let previewLoads = 0
  let shapeStages = 0
  let viewportCommits = 0

  const api = {
    async confirmAgentAssetChangeSet() {
      calls.push('confirm')
      return {
        asset_version: {
          asset_version_id: 'asset_v2',
          version_no: 2,
          shape_program: { schema_version: 'ShapeProgram@1', operations: [] },
        },
      }
    },
    async rejectAgentAssetChangeSet() {
      calls.push('reject')
      return {}
    },
    async loadAgentAssetPreviewGlb() {
      previewLoads += 1
      calls.push('load_preview')
      return {
        artifactProfileId: 'preview_editable',
        glb: 'Z2xi',
      }
    },
  }

  const adapter = createSurfaceAdornmentAdapter(api as never, {
    setAgentAssetChangeSet(changeSet) {
      calls.push(changeSet === null ? 'clear_change_set' : 'set_change_set')
    },
    setBlockoutShapeProgram() {
      shapeStages += 1
      calls.push('stage_shape')
      return 7
    },
    setBlockoutGlb() {
      viewportCommits += 1
      calls.push('commit_viewport')
      return true
    },
    clearAgentAssetWorkspaceQuality() {
      calls.push('clear_quality')
    },
    async refreshActiveDesign() {
      calls.push('refresh_active_design')
    },
    isCurrentAsset() {
      return true
    },
    getCurrentProjectId() {
      return 'project_1'
    },
    getCurrentAssetVersion() {
      return {
        projectId: 'project_1',
        assetVersionId: 'asset_v2',
        shapeProgram: { schema_version: 'ShapeProgram@1', operations: [] },
      }
    },
  })

  const retained = await adapter.retain('changeset_1')
  assert(retained.status === 'retained', 'retain should return a retained receipt')
  assert(
    calls.join(',') === 'confirm,clear_quality,refresh_active_design,clear_change_set',
    `retain must leave preview-to-production ownership with refreshActiveDesign; got ${calls.join(',')}`,
  )
  assertEqual(previewLoads, 0, 'retain must not start a newer preview-only display request')
  assertEqual(shapeStages, 0, 'retain must not supersede refreshActiveDesign display hydration')
  assertEqual(viewportCommits, 0, 'retain must not overwrite the refreshActiveDesign viewport request')

  calls.length = 0
  await adapter.cancel('changeset_2')
  assert(
    calls.join(',') === 'reject,stage_shape,load_preview,commit_viewport,refresh_active_design',
    `cancel should still restore a safe preview before refreshing truth; got ${calls.join(',')}`,
  )
  assertEqual(previewLoads, 1, 'cancel should retain its preview recovery path')
  assertEqual(shapeStages, 1, 'cancel should stage the visible preview exactly once')
  assertEqual(viewportCommits, 1, 'cancel should restore the visible preview exactly once')
}
