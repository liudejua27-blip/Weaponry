import { useCallback } from 'react'
import type { Dispatch, MutableRefObject, SetStateAction } from 'react'

import type { AgentAssetChangeSet } from '../../shared/types'

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

type ReferenceEvidenceRebuildPlanByChangeSet = {
  projectId: string
  baseAssetVersionId: string
  evidenceId: string
  sourceObjectSha256: string
  rebuildPlanId: string
}

type UseCadWorkbenchPanelProjectResetStateInput = {
  setAgentAssetChangeSet: Dispatch<SetStateAction<AgentAssetChangeSet | null>>
  setAgentCandidateSelectedPartId: Dispatch<SetStateAction<string | null>>
  setSurfaceAdornmentOpen: Dispatch<SetStateAction<boolean>>
  setReferenceEvidenceOpen: Dispatch<SetStateAction<boolean>>
  setStyleOptionsOpen: Dispatch<SetStateAction<boolean>>
  setMaterialOptionsOpen: Dispatch<SetStateAction<boolean>>
  replaceReferenceViewport: (next: ReferenceViewportState | null) => void
  referenceEvidenceRequestEpochRef: MutableRefObject<number>
  referenceRebuildPlanByChangeSetRef: MutableRefObject<Map<string, ReferenceEvidenceRebuildPlanByChangeSet>>
}

type UseCadWorkbenchPanelProjectResetStateResult = {
  resetProjectScopedState: () => void
  resetProjectDrawerState: () => void
}

export function useCadWorkbenchPanelProjectResetState({
  setAgentAssetChangeSet,
  setAgentCandidateSelectedPartId,
  setSurfaceAdornmentOpen,
  setReferenceEvidenceOpen,
  setStyleOptionsOpen,
  setMaterialOptionsOpen,
  replaceReferenceViewport,
  referenceEvidenceRequestEpochRef,
  referenceRebuildPlanByChangeSetRef,
}: UseCadWorkbenchPanelProjectResetStateInput): UseCadWorkbenchPanelProjectResetStateResult {
  const resetProjectScopedState = useCallback(() => {
    setAgentAssetChangeSet(null)
    setAgentCandidateSelectedPartId(null)
    setSurfaceAdornmentOpen(false)
    setReferenceEvidenceOpen(false)
    replaceReferenceViewport(null)
    referenceEvidenceRequestEpochRef.current += 1
    referenceRebuildPlanByChangeSetRef.current.clear()
  }, [
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    setSurfaceAdornmentOpen,
    setReferenceEvidenceOpen,
    replaceReferenceViewport,
    referenceEvidenceRequestEpochRef,
    referenceRebuildPlanByChangeSetRef,
  ])

  const resetProjectDrawerState = useCallback(() => {
    setStyleOptionsOpen(false)
    setMaterialOptionsOpen(false)
  }, [setStyleOptionsOpen, setMaterialOptionsOpen])

  return {
    resetProjectScopedState,
    resetProjectDrawerState,
  }
}
