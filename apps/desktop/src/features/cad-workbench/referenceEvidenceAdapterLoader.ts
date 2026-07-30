import type { ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetChangeSet } from '../../shared/types'
import { arrayBufferToBase64 } from './cadWorkbenchPanelFileUtils'
import {
  loadReferenceEvidenceHistory,
  readReferenceRebuildExactLineage,
  type ReferenceEvidenceAdapter,
  type ReferenceEvidenceRecord,
  type ReferenceEvidenceTarget,
} from './referenceEvidenceDrawerLogic.js'
import {
  previewReferenceGuidedRebuild,
  restoreAgentAssetBlockoutPreviewWithProductionFallback,
} from './agentBlockoutDisplayLoader.js'
import type { AgentBlockoutGlbPayload, AgentBlockoutGlbKind } from './agentBlockoutDisplayState'
import { referenceRebuildFailureMessage } from './cadWorkbenchPanelLogic.js'

type ReferenceViewportState = {
  projectId: string
  evidenceId: string
  sourceObjectSha256: string
  referenceClass: 'single_image' | 'multi_view_contact_sheet' | 'strict_glb_readback'
  kind: 'glb'
  glb: ArrayBuffer
} | {
  projectId: string
  evidenceId: string
  sourceObjectSha256: string
  referenceClass: 'single_image' | 'multi_view_contact_sheet'
  kind: 'image'
  imageUrl: string
}

type ReferenceRebuildPlanBinding = {
  projectId: string
  baseAssetVersionId: string
  evidenceId: string
  sourceObjectSha256: string
  rebuildPlanId: string
}

type ReferenceEvidenceAdapterCallbacks = {
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setBlockoutShapeProgram: (projectId: string | null, shapeProgram: Record<string, unknown> | null) => number | null
  setBlockoutGlb: (
    projectId: string | null,
    requestId: number,
    glbBase64: AgentBlockoutGlbPayload | null,
    glbKind: AgentBlockoutGlbKind | null,
  ) => boolean
  clearAgentAssetWorkspaceQuality: (projectId: string) => void
  refreshActiveDesign: (projectId: string) => Promise<unknown>
  getCurrentEpoch: () => number
  bumpEpoch: () => number
  getCurrentProjectId: () => string | null
  setPlanBinding: (changeSetId: string, binding: ReferenceRebuildPlanBinding) => void
  deletePlanBinding: (changeSetId: string) => void
  clearPlanBindings: () => void
  getPlanBinding: (changeSetId: string) => ReferenceRebuildPlanBinding | undefined
  setReferenceViewport: (next: ReferenceViewportState | null) => void
}

type ReferenceEvidenceHistoryApi = Pick<
  ForgeApi,
  'listProjectReferenceEvidence' |
  'getReferenceGuidedRebuildPlan'
>
type ReferenceEvidenceReferenceApi = Pick<
  ForgeApi,
  'createReferenceEvidence' |
  'getReferenceGuidedRebuildPlan' |
  'confirmAgentAssetChangeSet' |
  'rejectAgentAssetChangeSet' |
  'proposeReferenceGuidedRebuildPreview' |
  'previewAgentAssetChangeSet' |
  'exportAgentAssetChangeSetPreviewGlb' |
  'loadReferenceEvidenceContent' |
  'getActiveDesign' |
  'loadAgentAssetPreviewGlb' |
  'loadAgentAssetProductionGlb'
>

const REFERENCE_IMAGE_CLASS_WARNING_MESSAGE = '参考证据不是可在同一视口显示的图片。'
const REFERENCE_GLB_CLASS_WARNING_MESSAGE = '参考证据不是可在 3D 视口读取的 GLB。'
const REFERENCE_SWITCHED_IMAGE_MESSAGE = '项目已切换；没有加载过期的参考图片。'
const REFERENCE_SWITCHED_GLB_MESSAGE = '项目已切换；没有加载过期的参考 GLB。'
const REFERENCE_IMAGE_VIEW_MESSAGE = '已在同一个 3D 视口显示只读参考图片；它只是纹理化对照，不成为几何或版本真值。'
const REFERENCE_GLB_VIEW_MESSAGE = '已在同一个 3D 视口查看只读参考 GLB；它不会成为可编辑资产。'
const REFERENCE_IMAGE_FALLBACK_MESSAGE = '参考图片无法读取；已回到当前结果。'
const REFERENCE_GLB_FALLBACK_MESSAGE = '参考 GLB 无法读取；当前结果保持不变。'

export function createReferenceEvidenceAdapter(
  api: ReferenceEvidenceReferenceApi & ReferenceEvidenceHistoryApi,
  callbacks: ReferenceEvidenceAdapterCallbacks,
): ReferenceEvidenceAdapter {
  const {
    clearAgentAssetWorkspaceQuality,
    clearPlanBindings,
    deletePlanBinding,
    getCurrentEpoch,
    getCurrentProjectId,
    getPlanBinding,
    refreshActiveDesign,
    setAgentAssetChangeSet,
    setBlockoutGlb,
    setBlockoutShapeProgram,
    setPlanBinding,
    setReferenceViewport,
    bumpEpoch,
  } = callbacks

  const isSameProject = (projectId: string | null) => getCurrentProjectId() === projectId

  return {
    invalidate: () => {
      bumpEpoch()
      clearPlanBindings()
    },
    createEvidence: async ({
      target,
      file,
      sourceStatement,
      licenseStatement,
      missingViews,
      referenceClass,
      notes,
    }) => {
      const epoch = getCurrentEpoch()
      try {
        const kind = file.name.toLowerCase().endsWith('.glb') || file.type === 'model/gltf-binary' ? 'glb' as const : 'image' as const
        const contentBase64 = arrayBufferToBase64(await file.arrayBuffer())
        if (epoch !== getCurrentEpoch()) {
          return { status: 'unavailable', message: '参考输入已关闭或项目已切换；未继续创建重建预览。' }
        }
        const created = await api.createReferenceEvidence({
          client_request_id: `reference-evidence-${Date.now()}`,
          project_id: target.projectId,
          domain_pack_id: target.domainPackId ?? 'pack_unclassified',
          kind,
          file_name: file.name,
          media_type: kind === 'glb' ? 'model/gltf-binary' : file.type,
          source_statement: sourceStatement,
          license_statement: licenseStatement,
          missing_views: missingViews,
          ...(kind === 'image' && referenceClass ? { reference_class: referenceClass } : {}),
          ...(notes ? { user_notes: notes } : {}),
          content_base64: contentBase64,
        })
        if (epoch !== getCurrentEpoch()) {
          return { status: 'unavailable', message: '参考输入已关闭或项目已切换；证据已保持只读，未继续生成预览。' }
        }
        const record = created.reference_evidence
        return {
          status: 'created',
          evidence: {
            evidenceId: record.evidence_id,
            contentSha256: record.source_object_sha256,
            kind: record.kind,
            fileName: record.source_file_name,
            sourceStatement: record.source_statement,
            licenseStatement: record.license_statement,
            missingViews: record.missing_views,
            uncertainties: record.observations?.uncertainties ?? [],
            referenceClass: record.reference_class,
          },
        }
      } catch (caught) {
        return {
          status: 'failed',
          message: caught instanceof Error ? caught.message : '保存参考证据失败；当前设计没有变化。',
        }
      }
    },
    previewRebuild: async (target, evidence: ReferenceEvidenceRecord) => {
      return previewReferenceGuidedRebuild(
        api,
        {
          setBlockoutShapeProgram,
          setBlockoutGlb,
          setAgentAssetChangeSet,
          replaceReferenceViewport: () => setReferenceViewport(null),
          setPlanBinding: (changeSetId, binding) => {
            setPlanBinding(changeSetId, binding)
          },
          deletePlanBinding: (changeSetId) => {
            deletePlanBinding(changeSetId)
          },
          resolveFailureMessage: referenceRebuildFailureMessage,
          currentEpoch: getCurrentEpoch,
        },
        target,
        evidence,
      )
    },
    retain: async (changeSetId) => {
      const binding = getPlanBinding(changeSetId)
      if (!binding || !isSameProject(binding.projectId)) {
        return {
          status: 'failed',
          message: '参考重建预览缺少当前项目的冻结谱系，未执行确认。',
        }
      }
      const epoch = getCurrentEpoch()
      try {
        const previewRead = await api.getReferenceGuidedRebuildPlan(binding.projectId, binding.rebuildPlanId)
        const previewLineage = readReferenceRebuildExactLineage(previewRead, {
          evidenceId: binding.evidenceId,
          sourceObjectSha256: binding.sourceObjectSha256,
          previewChangeSetId: changeSetId,
        })
        if (
          previewRead.reference_guided_rebuild_plan.project_id !== binding.projectId
          || !previewLineage
          || previewLineage.status !== 'previewed'
        ) {
          throw new Error('确认前参考谱系已发生变化，未创建新版本。')
        }
        if (
          epoch !== getCurrentEpoch()
          || !isSameProject(binding.projectId)
        ) {
          return {
            status: 'unavailable',
            message: '项目已切换；没有把旧项目的参考预览确认到当前项目。',
          }
        }

        const confirmed = await api.confirmAgentAssetChangeSet(changeSetId, `reference-rebuild-confirm-${Date.now()}`)
        const confirmedRead = await api.getReferenceGuidedRebuildPlan(binding.projectId, binding.rebuildPlanId)
        const lineage = readReferenceRebuildExactLineage(confirmedRead, {
          evidenceId: binding.evidenceId,
          sourceObjectSha256: binding.sourceObjectSha256,
          previewChangeSetId: changeSetId,
        })
        if (
          confirmedRead.reference_guided_rebuild_plan.project_id !== binding.projectId
          || !lineage
          || lineage.status !== 'confirmed'
          || lineage.confirmedAssetVersionId !== confirmed.asset_version.asset_version_id
        ) {
          throw new Error('新版本已提交，但返回的生产 GLB 谱系无法验证；请重新打开项目核对。')
        }
        if (
          epoch !== getCurrentEpoch()
          || !isSameProject(binding.projectId)
        ) {
          deletePlanBinding(changeSetId)
          return {
            status: 'unavailable',
            message: '参考重建已在原项目确认；当前项目保持不变，请返回原项目查看结果。',
          }
        }

        const projectId = getCurrentProjectId()
        let retainedDisplaySummary = ''
        if (projectId) {
          clearAgentAssetWorkspaceQuality(projectId)
          await refreshActiveDesign(projectId)
          const restoreResult = await restoreAgentAssetBlockoutPreviewWithProductionFallback(
            api,
            setBlockoutShapeProgram,
            setBlockoutGlb,
            projectId,
            confirmed.asset_version.asset_version_id,
            confirmed.asset_version.shape_program,
          )
          if (restoreResult === 'project_switched') {
            retainedDisplaySummary = ' 当前项目已切换，结果保留在原项目。'
          } else if (restoreResult === 'preview') {
            retainedDisplaySummary = ' 生产工件暂不可用，当前明确显示同源轻量预览。'
          } else if (restoreResult === 'unreadable') {
            retainedDisplaySummary = ' 新版本已保存，但其 PBR 视图暂不可读取；没有继续显示旧版本。'
          }
        }
        setAgentAssetChangeSet(null)
        deletePlanBinding(changeSetId)
        return {
          status: 'retained',
          summary: `已保留参考引导重建并创建可编辑资产 v${confirmed.asset_version.version_no}。${retainedDisplaySummary}`,
          lineage,
        }
      } catch (caught) {
        return {
          status: 'failed',
          message: caught instanceof Error ? caught.message : '确认参考引导重建失败；当前版本未被覆盖。',
        }
      }
    },
    cancel: async (changeSetId) => {
      const binding = getPlanBinding(changeSetId)
      if (!binding) throw new Error('参考重建预览缺少原项目绑定；未执行取消。')
      const epoch = getCurrentEpoch()
      await api.rejectAgentAssetChangeSet(changeSetId, `reference-rebuild-reject-${Date.now()}`)
      const readback = await api.getActiveDesign(binding.projectId)
      const snapshot = readback.data
      if (
        snapshot.project_id !== binding.projectId
        || (snapshot.preview !== null && snapshot.preview !== undefined)
        || snapshot.active_design.project_id !== binding.projectId
        || !('asset_version_id' in snapshot.active_design)
        || snapshot.active_design.asset_version_id !== binding.baseAssetVersionId
      ) {
        throw new Error('取消后的当前设计读回不一致；抽屉保持打开，请重试。')
      }
      deletePlanBinding(changeSetId)
      if (epoch === getCurrentEpoch() && isSameProject(binding.projectId)) {
        setAgentAssetChangeSet(null)
        setReferenceViewport(null)
      }
    },
    loadHistory: async (target: ReferenceEvidenceTarget) => loadReferenceEvidenceHistory(api, target.projectId),
    loadContent: async (target: ReferenceEvidenceTarget, evidence: ReferenceEvidenceRecord) => {
      const content = await api.loadReferenceEvidenceContent(target.projectId, evidence.evidenceId)
      return content.blob
    },
    viewReferenceImage: async (target: ReferenceEvidenceTarget, evidence: ReferenceEvidenceRecord) => {
      const epoch = getCurrentEpoch()
      try {
        const content = await api.loadReferenceEvidenceContent(target.projectId, evidence.evidenceId)
        if (!content.mediaType.startsWith('image/')) {
          setReferenceViewport(null)
          return { status: 'failed', message: REFERENCE_IMAGE_CLASS_WARNING_MESSAGE }
        }
        if (epoch !== getCurrentEpoch() || !isSameProject(target.projectId)) {
          return { status: 'unavailable', message: REFERENCE_SWITCHED_IMAGE_MESSAGE }
        }
        const referenceClass = evidence.referenceClass === 'multi_view_contact_sheet'
          ? 'multi_view_contact_sheet'
          : 'single_image'
        const imageUrl = URL.createObjectURL(content.blob)
        if (epoch !== getCurrentEpoch() || !isSameProject(target.projectId)) {
          URL.revokeObjectURL(imageUrl)
          return { status: 'unavailable', message: REFERENCE_SWITCHED_IMAGE_MESSAGE }
        }
        setReferenceViewport({
          projectId: target.projectId,
          evidenceId: evidence.evidenceId,
          sourceObjectSha256: evidence.contentSha256,
          referenceClass,
          kind: 'image',
          imageUrl,
        })
        return { status: 'ready', message: REFERENCE_IMAGE_VIEW_MESSAGE }
      } catch (caught) {
        setReferenceViewport(null)
        return { status: 'failed', message: caught instanceof Error ? caught.message : REFERENCE_IMAGE_FALLBACK_MESSAGE }
      }
    },
    viewReferenceGlb: async (target: ReferenceEvidenceTarget, evidence: ReferenceEvidenceRecord) => {
      const epoch = getCurrentEpoch()
      try {
        const content = await api.loadReferenceEvidenceContent(target.projectId, evidence.evidenceId)
        if (content.mediaType !== 'model/gltf-binary') {
          return { status: 'failed', message: REFERENCE_GLB_CLASS_WARNING_MESSAGE }
        }
        if (epoch !== getCurrentEpoch() || !isSameProject(target.projectId)) {
          return { status: 'unavailable', message: REFERENCE_SWITCHED_GLB_MESSAGE }
        }
        const glb = await content.blob.arrayBuffer()
        if (epoch !== getCurrentEpoch() || !isSameProject(target.projectId)) {
          return { status: 'unavailable', message: REFERENCE_SWITCHED_GLB_MESSAGE }
        }
        setReferenceViewport({
          projectId: target.projectId,
          evidenceId: evidence.evidenceId,
          sourceObjectSha256: evidence.contentSha256,
          referenceClass: 'strict_glb_readback',
          kind: 'glb',
          glb,
        })
        return { status: 'ready', message: REFERENCE_GLB_VIEW_MESSAGE }
      } catch (caught) {
        return {
          status: 'failed',
          message: caught instanceof Error ? caught.message : REFERENCE_GLB_FALLBACK_MESSAGE,
        }
      }
    },
    viewResult: (target: ReferenceEvidenceTarget) => {
      if (isSameProject(target.projectId)) setReferenceViewport(null)
    },
  }
}
