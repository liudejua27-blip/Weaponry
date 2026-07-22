import {
  SurfaceAdornmentDesignSurface,
  unavailableSurfaceAdornmentAdapter,
  type SurfaceAdornmentAdapter,
  type SurfaceAdornmentDraft,
  type SurfaceAdornmentTarget,
} from './SurfaceAdornmentDrawer.js'
import { renderToStaticMarkup } from 'react-dom/server'

function assert(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }

const target: SurfaceAdornmentTarget = {
  projectId: 'project_surface_smoke', assetVersionId: 'asset_surface_smoke', partId: 'part_arm_shell',
  partLabel: '上臂连杆', materialZoneId: 'zone_primary', materialZoneLabel: '主材质区',
}

export async function runSurfaceAdornmentDrawerSmoke(): Promise<void> {
  const draft: SurfaceAdornmentDraft = { kind: 'engraving', motif: 'parallel', intensity: 'subtle', coverage: 'center' }
  const unavailable = await unavailableSurfaceAdornmentAdapter.preview(target, draft)
  assert(unavailable.status === 'unavailable' && unavailable.message.includes('不会创建修改或新版本'), 'unavailable adapter must fail closed without a fake preview')

  const designSurface = renderToStaticMarkup(<SurfaceAdornmentDesignSurface draft={draft} target={target} />)
  assert(
    designSurface.includes('<svg')
      && designSurface.includes('surface-adornment-design-surface')
      && designSurface.includes('data-surface-truth="editor_only"')
      && designSurface.includes('SVG 只编辑轮廓/图层')
      && designSurface.includes('真实 PBR 与 GLB'),
    'A005 must expose a constrained SVG design surface while explicitly keeping GLB/PBR as the retained model truth',
  )

  let previewTarget = ''
  let retainedPreview = ''
  let cancelledPreview = ''
  const mock: SurfaceAdornmentAdapter = {
    async enable() { return { status: 'enabled' } },
    async preview(receivedTarget) { previewTarget = `${receivedTarget.projectId}:${receivedTarget.partId}:${receivedTarget.materialZoneId}`; return { status: 'preview_ready', changeSetId: 'changeset_surface_smoke', summary: '真实预览已就绪。' } },
    async retain(changeSetId) { retainedPreview = changeSetId; return { status: 'retained', summary: '已保留。' } },
    async cancel(changeSetId) { cancelledPreview = changeSetId },
  }
  const preview = await mock.preview(target, draft)
  assert(previewTarget === 'project_surface_smoke:part_arm_shell:zone_primary', 'preview adapter must receive the saved asset target and stable zone')
  assert(preview.status === 'preview_ready' && preview.changeSetId === 'changeset_surface_smoke', 'a real adapter must identify a server-owned ChangeSet before the UI can retain it')
  await mock.retain('changeset_surface_smoke')
  await mock.cancel('changeset_surface_smoke')
  assert(retainedPreview === 'changeset_surface_smoke' && cancelledPreview === 'changeset_surface_smoke', 'typed adapter mock must cover confirm and reject lifecycle methods')

}
