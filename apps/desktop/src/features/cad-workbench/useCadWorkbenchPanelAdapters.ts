import { useMemo } from 'react'
import type { RefObject } from 'react'
import type { AgentAssetChangeSet } from '../../shared/types'
import type { AgentBlockoutGlbKind, AgentBlockoutGlbPayload } from './agentBlockoutDisplayState'
import { createReferenceEvidenceAdapter } from './referenceEvidenceAdapterLoader'
import { createSurfaceAdornmentAdapter } from './surfaceAdornmentAdapterLoader'
import type { SurfaceAdornmentAdapter } from './SurfaceAdornmentDrawer'
import type { ReferenceEvidenceAdapter } from './referenceEvidenceDrawerLogic.js'

type ReferenceRebuildPlanBinding = {
  projectId: string
  baseAssetVersionId: string
  evidenceId: string
  sourceObjectSha256: string
  rebuildPlanId: string
}

type CadWorkbenchPanelReferenceViewportState = {
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

type CadWorkbenchPanelAdaptersInput = {
  api: Parameters<typeof createSurfaceAdornmentAdapter>[0] & Parameters<typeof createReferenceEvidenceAdapter>[0]
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setBlockoutGlb: (
    projectId: string | null,
    requestId: number,
    glbBase64: AgentBlockoutGlbPayload | null,
    glbKind: AgentBlockoutGlbKind | null,
  ) => boolean
  setBlockoutShapeProgram: (projectId: string | null, shapeProgram: Record<string, unknown> | null) => number | null
  clearAgentAssetWorkspaceQuality: (projectId: string) => void
  refreshActiveDesign: (projectId: string) => Promise<unknown>
  conceptProjectId: string | null
  activeAgentAssetVersionId: string | null
  activeAgentAssetVersionShapeProgram: Record<string, unknown> | null
  referenceEvidenceRequestEpochRef: RefObject<number>
  referenceRebuildPlanByChangeSetRef: RefObject<Map<string, ReferenceRebuildPlanBinding>>
  setReferenceViewport: (next: CadWorkbenchPanelReferenceViewportState | null) => void
}

type CadWorkbenchPanelAdapters = {
  surfaceAdornmentAdapter: SurfaceAdornmentAdapter
  referenceEvidenceAdapter: ReferenceEvidenceAdapter
}

export function useCadWorkbenchPanelAdapters({
  api,
  setAgentAssetChangeSet,
  setBlockoutGlb,
  setBlockoutShapeProgram,
  clearAgentAssetWorkspaceQuality,
  refreshActiveDesign,
  conceptProjectId,
  activeAgentAssetVersionId,
  activeAgentAssetVersionShapeProgram,
  referenceEvidenceRequestEpochRef,
  referenceRebuildPlanByChangeSetRef,
  setReferenceViewport,
}: CadWorkbenchPanelAdaptersInput): CadWorkbenchPanelAdapters {
  const surfaceAdornmentAdapter = useMemo(
    () => createSurfaceAdornmentAdapter(api, {
      setAgentAssetChangeSet,
      setBlockoutGlb,
      setBlockoutShapeProgram,
      clearAgentAssetWorkspaceQuality,
      refreshActiveDesign,
      isCurrentAsset: (assetVersionId) => Boolean(activeAgentAssetVersionId === assetVersionId),
      getCurrentProjectId: () => conceptProjectId,
      getCurrentAssetVersion: () => {
        if (!conceptProjectId || !activeAgentAssetVersionId) return null
        return {
          projectId: conceptProjectId,
          assetVersionId: activeAgentAssetVersionId,
          shapeProgram: activeAgentAssetVersionShapeProgram,
        }
      },
    }),
    [
      activeAgentAssetVersionId,
      activeAgentAssetVersionShapeProgram,
      api,
      clearAgentAssetWorkspaceQuality,
      conceptProjectId,
      refreshActiveDesign,
      setAgentAssetChangeSet,
      setBlockoutGlb,
      setBlockoutShapeProgram,
    ],
  )

  const referenceEvidenceAdapter = useMemo(
    () => createReferenceEvidenceAdapter(api, {
      setAgentAssetChangeSet,
      setBlockoutShapeProgram,
      setBlockoutGlb,
      clearAgentAssetWorkspaceQuality,
      refreshActiveDesign,
    getCurrentEpoch: () => referenceEvidenceRequestEpochRef.current ?? 0,
    bumpEpoch: () => {
      const nextEpoch = (referenceEvidenceRequestEpochRef.current ?? 0) + 1
      referenceEvidenceRequestEpochRef.current = nextEpoch
      return nextEpoch
    },
      getCurrentProjectId: () => conceptProjectId,
      setPlanBinding: (changeSetId, binding) => {
        referenceRebuildPlanByChangeSetRef.current.set(changeSetId, binding)
      },
      deletePlanBinding: (changeSetId) => {
        referenceRebuildPlanByChangeSetRef.current.delete(changeSetId)
      },
      clearPlanBindings: () => {
        referenceRebuildPlanByChangeSetRef.current.clear()
      },
      getPlanBinding: (changeSetId) => referenceRebuildPlanByChangeSetRef.current.get(changeSetId),
      setReferenceViewport,
    }),
    [
      api,
      clearAgentAssetWorkspaceQuality,
      conceptProjectId,
      refreshActiveDesign,
      referenceEvidenceRequestEpochRef,
      referenceRebuildPlanByChangeSetRef,
      setAgentAssetChangeSet,
      setBlockoutGlb,
      setBlockoutShapeProgram,
      setReferenceViewport,
    ],
  )

  return { surfaceAdornmentAdapter, referenceEvidenceAdapter }
}
