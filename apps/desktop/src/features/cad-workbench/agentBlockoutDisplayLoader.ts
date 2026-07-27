import { ForgeApiError, type AgentAssetGlbBinary, type AgentAssetChangeSetPreviewGlb } from '../../shared/api/forgeApi'
import type { AgentAssetChangeSet, AgentPartEditOperation } from '../../shared/types'
import { type AgentBlockoutGlbPayload } from './agentBlockoutDisplayState'
import { compileSurfaceAdornmentDraft } from './cadWorkbenchPanelLogic.js'
import {
  readReferenceRebuildComparisonPlan,
  readReferenceRebuildExactLineage,
} from './referenceEvidenceDrawerLogic.js'
import type {
  ReferenceEvidenceRecord,
  ReferenceEvidenceTarget,
  ReferenceRebuildExactLineage,
  ReferenceRebuildPreviewResponse,
} from './referenceEvidenceDrawerLogic.js'
import type {
  SurfaceAdornmentDraft,
  SurfaceAdornmentPreviewResponse,
  SurfaceAdornmentTarget,
} from './SurfaceAdornmentDrawer'

type BlockoutDisplayGlbLoader = {
  loadAgentAssetPreviewGlb: (assetVersionId: string) => Promise<AgentAssetGlbBinary>
  loadAgentAssetProductionGlb: (assetVersionId: string) => Promise<AgentAssetGlbBinary>
}

type BlockoutShapeSetter = (projectId: string | null, shapeProgram: Record<string, unknown> | null) => number | null
type BlockoutGlbSetter = (
  projectId: string | null,
  requestId: number,
  glbBase64: AgentBlockoutGlbPayload,
  glbKind: 'external_reference' | 'compiled_agent_preview_pbr' | 'compiled_agent_production_pbr' | null,
) => boolean

export type BlockoutDisplayAgentApi = BlockoutDisplayGlbLoader

export type BlockoutDisplayProjectState = {
  projectId: string | null
  requestId: number
  isCurrentActiveDesignRequest: (requestId: number) => boolean
  setBlockoutGlb: (projectId: string | null, requestId: number, glbBase64: AgentBlockoutGlbPayload, glbKind: 'external_reference' | 'compiled_agent_preview_pbr' | 'compiled_agent_production_pbr' | null) => boolean
  setAssistantNote: (note: string) => void
  isCurrentDisplayRequest: () => boolean
}

export type ReferenceRebuildPlanBinding = {
  projectId: string
  baseAssetVersionId: string
  evidenceId: string
  sourceObjectSha256: string
  rebuildPlanId: string
}

type ReferenceRebuildApi = {
  proposeReferenceGuidedRebuildPreview: (projectId: string, input: {
    client_request_id: string
    evidence_id: string
    domain_pack_id: string
    base_asset_version_id: string
  }) => Promise<{ changeSet: { change_set_id: string }, planRead: unknown }>
  previewAgentAssetChangeSet: (changeSetId: string, idempotencyKey: string) => Promise<AgentAssetChangeSet>
  getReferenceGuidedRebuildPlan: (projectId: string, rebuildPlanId: string) => Promise<unknown>
  exportAgentAssetChangeSetPreviewGlb: (changeSetId: string) => Promise<AgentAssetChangeSetPreviewGlb>
  rejectAgentAssetChangeSet: (changeSetId: string, idempotencyKey: string) => Promise<unknown>
}

type ReferenceRebuildPreviewCallbacks = {
  setBlockoutShapeProgram: BlockoutShapeSetter
  setBlockoutGlb: BlockoutGlbSetter
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  replaceReferenceViewport: () => void
  setPlanBinding: (changeSetId: string, binding: ReferenceRebuildPlanBinding) => void
  deletePlanBinding: (changeSetId: string) => void
  resolveFailureMessage: (error: unknown) => string
  currentEpoch: () => number
}

function readReferenceGuidedRebuildPlanProjectId(value: unknown): string | null {
  if (!value || typeof value !== 'object') return null
  const top = value as { reference_guided_rebuild_plan?: { project_id?: unknown } }
  const nested = top.reference_guided_rebuild_plan
  return nested && typeof nested === 'object' && typeof nested.project_id === 'string' ? nested.project_id : null
}

export async function loadAgentAssetDisplayViews(
  api: BlockoutDisplayAgentApi,
  state: BlockoutDisplayProjectState,
  assetVersionId: string,
  blockoutDisplayRequestId: number,
  isImportedReference: boolean,
): Promise<void> {
  const { projectId, isCurrentActiveDesignRequest, isCurrentDisplayRequest, setAssistantNote, setBlockoutGlb } = state

  try {
    const preview = await api.loadAgentAssetPreviewGlb(assetVersionId)
    const previewKind = preview.artifactProfileId === 'external_reference'
      ? 'external_reference'
      : 'compiled_agent_preview_pbr'
    if (!setBlockoutGlb(projectId, blockoutDisplayRequestId, preview.glb, previewKind)) return
    if (preview.artifactProfileId === 'external_reference') return

    setAssistantNote('已加载轻量编辑预览；生产级概念工件正在按需生成，完成后会在同一视口中替换。')

    try {
      const production = await api.loadAgentAssetProductionGlb(assetVersionId)
      if (production.artifactProfileId !== 'production_concept') {
        throw new Error('Production GLB response did not use the production concept profile')
      }
      if (!setBlockoutGlb(projectId, blockoutDisplayRequestId, production.glb, 'compiled_agent_production_pbr')) return
      setAssistantNote(`生产级概念工件已加载：${production.triangleCount.toLocaleString()} 三角形、512×512 PBR 纹理；当前仍是可编辑概念资产，不是制造 CAD。`)
    } catch {
      if (!isCurrentActiveDesignRequest(state.requestId)) return
      setAssistantNote('生产级概念工件暂未加载；同源轻量预览仍可编辑，正式质量检查和下载不会使用该预览冒充最终结果。')
    }
    return
  } catch {
    if (isImportedReference || !isCurrentActiveDesignRequest(state.requestId)) {
      if (!setBlockoutGlb(projectId, blockoutDisplayRequestId, null, null)) return
      setAssistantNote('导入参考模型的原始 GLB 不可读取；不会影响其他项目版本。')
      return
    }

    if (!isCurrentDisplayRequest()) {
      if (!isCurrentActiveDesignRequest(state.requestId)) return
      if (!setBlockoutGlb(projectId, blockoutDisplayRequestId, null, null)) return
      setAssistantNote('当前 Agent 资产的预览与生产 PBR GLB 均不可读取；视口已明确回退为参数外观，没有继续显示旧材质。')
      return
    }

    try {
      const production = await api.loadAgentAssetProductionGlb(assetVersionId)
      if (production.artifactProfileId !== 'production_concept') {
        throw new Error('Production GLB response did not use the production concept profile')
      }
      if (!setBlockoutGlb(projectId, blockoutDisplayRequestId, production.glb, 'compiled_agent_production_pbr')) return
      setAssistantNote(`轻量预览不可用，已直接加载生产级概念工件：${production.triangleCount.toLocaleString()} 三角形、512×512 PBR 纹理。`)
    } catch {
      if (!isCurrentActiveDesignRequest(state.requestId)) return
      if (!setBlockoutGlb(projectId, blockoutDisplayRequestId, null, null)) return
      setAssistantNote('当前 Agent 资产的预览与生产 PBR GLB 均不可读取；视口已明确回退为参数外观，没有继续显示旧材质。')
    }
  }
}

export async function restoreAgentAssetBlockoutPreview(
  api: BlockoutDisplayAgentApi,
  setBlockoutShapeProgram: BlockoutShapeSetter,
  setBlockoutGlb: BlockoutGlbSetter,
  projectId: string | null,
  assetVersionId: string,
  shapeProgram: Record<string, unknown> | null,
): Promise<boolean> {
  const restoreRequestId = setBlockoutShapeProgram(projectId, shapeProgram)
  if (restoreRequestId === null) return false
  try {
    const preview = await api.loadAgentAssetPreviewGlb(assetVersionId)
    return setBlockoutGlb(
      projectId,
      restoreRequestId,
      preview.glb,
      preview.artifactProfileId === 'external_reference'
        ? 'external_reference'
        : 'compiled_agent_preview_pbr',
    )
  } catch {
    return false
  }
}

type BlockoutDisplayRestoreResult = 'project_switched' | 'production' | 'preview' | 'unreadable'

export async function restoreAgentAssetBlockoutPreviewWithProductionFallback(
  api: BlockoutDisplayAgentApi,
  setBlockoutShapeProgram: BlockoutShapeSetter,
  setBlockoutGlb: BlockoutGlbSetter,
  projectId: string | null,
  assetVersionId: string,
  shapeProgram: Record<string, unknown> | null,
): Promise<BlockoutDisplayRestoreResult> {
  const displayRequestId = setBlockoutShapeProgram(projectId, shapeProgram)
  if (displayRequestId === null) return 'project_switched'

  try {
    const production = await api.loadAgentAssetProductionGlb(assetVersionId)
    if (production.artifactProfileId !== 'production_concept') {
      throw new Error('Production GLB response did not use the production concept profile')
    }
    if (!setBlockoutGlb(projectId, displayRequestId, production.glb, 'compiled_agent_production_pbr')) {
      return 'project_switched'
    }
    return 'production'
  } catch {
    try {
      const preview = await api.loadAgentAssetPreviewGlb(assetVersionId)
      if (!setBlockoutGlb(
        projectId,
        displayRequestId,
        preview.glb,
        preview.artifactProfileId === 'external_reference'
          ? 'external_reference'
          : 'compiled_agent_preview_pbr',
      )) {
        return 'project_switched'
      }
      return 'preview'
    } catch {
      setBlockoutGlb(projectId, displayRequestId, null, null)
      return 'unreadable'
    }
  }
}

type ChangeSetPreviewAgentApi = BlockoutDisplayGlbLoader & {
  proposeAgentAssetChangeSet: (assetVersionId: string, input: {
    client_request_id: string
    summary: string
    operations: AgentPartEditOperation[]
  }) => Promise<AgentAssetChangeSet>
  previewAgentAssetChangeSet: (changeSetId: string, idempotencyKey: string) => Promise<AgentAssetChangeSet>
  exportAgentAssetChangeSetPreviewGlb: (changeSetId: string) => Promise<AgentAssetChangeSetPreviewGlb>
  rejectAgentAssetChangeSet: (changeSetId: string, idempotencyKey: string) => Promise<unknown>
}

type SurfaceAdornmentApi = BlockoutDisplayGlbLoader & {
  proposeSurfaceAdornmentPreview: (assetVersionId: string, input: {
    client_request_id: string
    part_id: string
    material_zone_id: string
    kind: 'normal_relief' | 'pattern' | 'flowline' | 'micro_surface'
    motif: 'parallel_groove' | 'chevron_relief' | 'hex_microgrid' | 'double_flowline'
    intensity: 'subtle' | 'balanced' | 'pronounced'
    coverage: 'center_band' | 'edge_band' | 'full_zone' | 'symmetric_pair'
  }) => Promise<AgentAssetChangeSet>
  previewAgentAssetChangeSet: (changeSetId: string, idempotencyKey: string) => Promise<AgentAssetChangeSet>
  exportAgentAssetChangeSetPreviewGlb: (changeSetId: string) => Promise<AgentAssetChangeSetPreviewGlb>
  rejectAgentAssetChangeSet: (changeSetId: string, idempotencyKey: string) => Promise<unknown>
}

type SurfaceAdornmentPreviewCallbacks = {
  isCurrentAsset: (assetVersionId: string) => boolean
  setBlockoutShapeProgram: BlockoutShapeSetter
  setBlockoutGlb: BlockoutGlbSetter
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
}

export async function previewAgentAssetChangeSet(
  api: ChangeSetPreviewAgentApi,
  setBlockoutShapeProgram: BlockoutShapeSetter,
  setBlockoutGlb: BlockoutGlbSetter,
  projectId: string | null,
  assetVersionId: string,
  shapeProgram: Record<string, unknown> | null,
  summary: string,
  operations: readonly AgentPartEditOperation[],
): Promise<AgentAssetChangeSet | null> {
  let previewChangeSetId: string | null = null
  let displayRequestId: number | null = null
  try {
    const proposed = await api.proposeAgentAssetChangeSet(assetVersionId, {
      client_request_id: `agent-asset-change-${Date.now()}`,
      summary,
      operations: operations as AgentPartEditOperation[],
    })
    previewChangeSetId = proposed.change_set_id
    const preview = await api.previewAgentAssetChangeSet(proposed.change_set_id, `agent-asset-preview-${Date.now()}`)
    if (!preview.preview) throw new Error('ChangeSet preview did not return an Agent asset candidate')

    displayRequestId = setBlockoutShapeProgram(projectId, preview.preview.shape_program)
    if (displayRequestId === null) throw new Error('ChangeSet preview no longer belongs to the open project')

    const compiled = await api.exportAgentAssetChangeSetPreviewGlb(preview.change_set_id)
    if (
      compiled.baseAssetVersionId !== assetVersionId
      || !compiled.sha256?.match(/^[a-f0-9]{64}$/)
      || !Number.isInteger(compiled.triangleCount ?? NaN)
      || (compiled.triangleCount ?? 0) <= 0
    ) {
      throw new Error('ChangeSet preview GLB metadata does not match the active asset version')
    }
    if (!setBlockoutGlb(projectId, displayRequestId, compiled.glb, 'compiled_agent_preview_pbr')) {
      throw new Error('ChangeSet preview display was superseded by a newer request')
    }
    return preview
  } catch {
    if (previewChangeSetId) {
      await api.rejectAgentAssetChangeSet(previewChangeSetId, `agent-asset-preview-cleanup-${Date.now()}`).catch(() => undefined)
    }
    if (displayRequestId !== null && setBlockoutGlb(projectId, displayRequestId, null, null)) {
      await restoreAgentAssetBlockoutPreview(
        api,
        setBlockoutShapeProgram,
        setBlockoutGlb,
        projectId,
        assetVersionId,
        shapeProgram,
      )
    }
    return null
  }
}

export async function previewReferenceGuidedRebuild(
  api: ReferenceRebuildApi,
  callbacks: ReferenceRebuildPreviewCallbacks,
  target: ReferenceEvidenceTarget,
  evidence: ReferenceEvidenceRecord,
): Promise<ReferenceRebuildPreviewResponse> {
  const epoch = callbacks.currentEpoch()
  let changeSetId: string | null = null
  try {
    if (!target.baseAssetVersionId) {
      return {
        status: 'unavailable',
        message: '请先生成并确认机械臂生产基准，再使用参考重建；当前设计没有变化。',
      }
    }

    const proposed = await api.proposeReferenceGuidedRebuildPreview(target.projectId, {
      client_request_id: `reference-rebuild-${Date.now()}`,
      evidence_id: evidence.evidenceId,
      domain_pack_id: target.domainPackId ?? 'pack_robotic_arm_concept',
      base_asset_version_id: target.baseAssetVersionId,
    })
    changeSetId = proposed.changeSet.change_set_id

    if (readReferenceGuidedRebuildPlanProjectId(proposed.planRead) !== target.projectId) {
      throw new Error('参考重建计划没有返回可验证的冻结证据谱系。')
    }

    const draftLineage = readReferenceRebuildExactLineage(proposed.planRead, {
      evidenceId: evidence.evidenceId,
      sourceObjectSha256: evidence.contentSha256,
    })
    if (!draftLineage || draftLineage.status !== 'draft') {
      throw new Error('参考重建计划没有返回可验证的冻结证据谱系。')
    }

    const preview = await api.previewAgentAssetChangeSet(changeSetId, `reference-rebuild-preview-${Date.now()}`)
    if (!preview.preview) throw new Error('参考引导重建没有返回可验证的 ShapeProgram 预览。')

    const planRead = await api.getReferenceGuidedRebuildPlan(target.projectId, draftLineage.rebuildPlanId)
    if (readReferenceGuidedRebuildPlanProjectId(planRead) !== target.projectId) {
      throw new Error('参考重建预览与冻结证据谱系不一致，已拒绝此次预览。')
    }

    const lineage = readReferenceRebuildExactLineage(planRead, {
      evidenceId: evidence.evidenceId,
      sourceObjectSha256: evidence.contentSha256,
      previewChangeSetId: changeSetId,
    }) as ReferenceRebuildExactLineage | null
    if (!lineage || lineage.status !== 'previewed' || lineage.rebuildPlanId !== draftLineage.rebuildPlanId) {
      throw new Error('参考重建预览与冻结证据谱系不一致，已拒绝此次预览。')
    }

    const compiled = await api.exportAgentAssetChangeSetPreviewGlb(changeSetId)
    if (!compiled.sha256?.match(/^[a-f0-9]{64}$/) || !Number.isInteger(compiled.triangleCount) || (compiled.triangleCount ?? 0) <= 0) {
      throw new Error('参考引导重建预览没有返回可验证 GLB。')
    }

    if (epoch !== callbacks.currentEpoch()) {
      await api.rejectAgentAssetChangeSet(changeSetId, `reference-rebuild-late-reject-${Date.now()}`).catch(() => undefined)
      return { status: 'unavailable', message: '参考预览已过期并被取消；当前设计没有变化。' }
    }

    callbacks.replaceReferenceViewport()

    const displayRequestId = callbacks.setBlockoutShapeProgram(target.projectId, preview.preview.shape_program)
    if (displayRequestId === null || !callbacks.setBlockoutGlb(
      target.projectId,
      displayRequestId,
      compiled.glb,
      'compiled_agent_preview_pbr',
    )) {
      await api.rejectAgentAssetChangeSet(changeSetId, `reference-rebuild-display-reject-${Date.now()}`).catch(() => undefined)
      return { status: 'unavailable', message: '当前项目已切换；参考预览已取消。' }
    }

    callbacks.setAgentAssetChangeSet(preview)
    callbacks.setPlanBinding(changeSetId, {
      projectId: target.projectId,
      baseAssetVersionId: target.baseAssetVersionId,
      evidenceId: evidence.evidenceId,
      sourceObjectSha256: evidence.contentSha256,
      rebuildPlanId: lineage.rebuildPlanId,
    })

    return {
      status: 'preview_ready',
      changeSetId,
      summary: '已在同一个 3D 视口加载新的可编辑机械臂重建预览；参考源仍保持只读，保留后才创建版本。',
      comparison: readReferenceRebuildComparisonPlan(planRead) ?? undefined,
      lineage,
    }
  } catch (caught) {
    if (changeSetId) {
      await api.rejectAgentAssetChangeSet(changeSetId, `reference-rebuild-cleanup-${Date.now()}`).catch(() => undefined)
      callbacks.deletePlanBinding(changeSetId)
    }
    return {
      status: 'failed',
      message: callbacks.resolveFailureMessage(caught),
    }
  }
}

export async function previewSurfaceAdornment(
  api: SurfaceAdornmentApi,
  callbacks: SurfaceAdornmentPreviewCallbacks,
  target: SurfaceAdornmentTarget,
  draft: SurfaceAdornmentDraft,
): Promise<SurfaceAdornmentPreviewResponse> {
  if (!callbacks.isCurrentAsset(target.assetVersionId)) {
    return { status: 'unavailable', message: '当前模型已切换，请重新选择部件。' }
  }

  let changeSetId: string | null = null
  let displayRequestId: number | null = null
  let failureStage = 'SURFACE_ADORNMENT_PROPOSE_FAILED'

  try {
    const proposed = await api.proposeSurfaceAdornmentPreview(
      target.assetVersionId,
      {
        client_request_id: `surface-adornment-${Date.now()}`,
        part_id: target.partId,
        material_zone_id: target.materialZoneId,
        ...compileSurfaceAdornmentDraft(draft),
      },
    )

    changeSetId = proposed.change_set_id
    failureStage = 'SURFACE_ADORNMENT_CHANGE_SET_PREVIEW_FAILED'

    const preview = await api.previewAgentAssetChangeSet(proposed.change_set_id, `surface-adornment-preview-${Date.now()}`)
    if (!preview.preview) {
      throw new Error('外观细节预览没有返回可验证模型。')
    }

    failureStage = 'SURFACE_ADORNMENT_VIEWPORT_STAGE_FAILED'
    displayRequestId = callbacks.setBlockoutShapeProgram(target.projectId, preview.preview.shape_program)
    if (displayRequestId === null) throw new Error('当前项目已切换。')

    failureStage = 'SURFACE_ADORNMENT_PREVIEW_GLB_FAILED'
    const compiled = await api.exportAgentAssetChangeSetPreviewGlb(preview.change_set_id)

    failureStage = 'SURFACE_ADORNMENT_PREVIEW_GLB_IDENTITY_FAILED'
    if (
      compiled.baseAssetVersionId !== target.assetVersionId
      || !compiled.sha256?.match(/^[a-f0-9]{64}$/)
      || !Number.isInteger(compiled.triangleCount)
      || (compiled.triangleCount ?? 0) <= 0
    ) {
      throw new Error('外观细节 GLB 与当前模型版本不一致。')
    }

    failureStage = 'SURFACE_ADORNMENT_VIEWPORT_COMMIT_FAILED'
    if (!callbacks.setBlockoutGlb(target.projectId, displayRequestId, compiled.glb, 'compiled_agent_preview_pbr')) {
      throw new Error('外观细节预览已被更新的请求取代。')
    }

    callbacks.setAgentAssetChangeSet(preview)
    return {
      status: 'preview_ready',
      changeSetId: preview.change_set_id,
      summary: '已在同一个 3D 视口中加载真实 PBR 外观细节预览；保留后才创建新版本。',
    }
  } catch (caught) {
    if (changeSetId) {
      await api.rejectAgentAssetChangeSet(changeSetId, `surface-adornment-cleanup-${Date.now()}`).catch(() => undefined)
    }
    if (caught instanceof ForgeApiError && caught.code === 'SURFACE_ADORNMENT_SKILL_DISABLED') {
      return { status: 'activation_required', message: caught.message }
    }

    if (displayRequestId !== null) {
      callbacks.setBlockoutGlb(target.projectId, displayRequestId, null, null)
    }

    return {
      status: 'failed',
      message: caught instanceof Error ? caught.message : '外观细节预览失败；当前版本没有变化。',
      errorCode: caught instanceof ForgeApiError
        ? caught.code
        : typeof caught === 'object' && caught !== null && 'code' in caught && typeof caught.code === 'string'
          ? caught.code
          : failureStage,
    }
  }
}
