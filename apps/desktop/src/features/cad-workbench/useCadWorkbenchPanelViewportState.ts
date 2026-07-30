import { useMemo } from 'react'
import { buildReferenceViewportPresentation } from './referenceViewportPresentation'
import { buildViewportReadoutText } from './cadWorkbenchPanelViewportReadout'

type ReferenceViewportState =
  | {
    projectId: string
    evidenceId: string
    sourceObjectSha256: string
    referenceClass: 'single_image' | 'multi_view_contact_sheet' | 'strict_glb_readback'
    kind: 'glb'
    glb: ArrayBuffer
  }
  | {
    projectId: string
    evidenceId: string
    sourceObjectSha256: string
    referenceClass: 'single_image' | 'multi_view_contact_sheet'
    kind: 'image'
    imageUrl: string
  }

type UseCadWorkbenchPanelViewportStateInput = {
  conceptProjectId: string | null
  activeAgentAssetVersionDomainPackId: string | null
  agentPlanDomainPackId: string | null
  activeAgentAssetVersionId: string | null
  isExternalGlbReference: boolean
  legacyDetailsEnabled: boolean
  isPreviewActive: boolean
  hasActiveAgentAsset: boolean
  referenceViewport: ReferenceViewportState | null
  blockoutGlbBase64: string | ArrayBuffer | null
  blockoutGlbKind: 'external_reference' | 'compiled_agent_pbr' | 'compiled_agent_preview_pbr' | 'compiled_agent_production_pbr' | null
  blockoutShapeProgram: Record<string, unknown> | null
}

export function useCadWorkbenchPanelViewportState({
  conceptProjectId,
  activeAgentAssetVersionDomainPackId,
  agentPlanDomainPackId,
  activeAgentAssetVersionId,
  isExternalGlbReference,
  legacyDetailsEnabled,
  isPreviewActive,
  hasActiveAgentAsset,
  referenceViewport,
  blockoutGlbBase64,
  blockoutGlbKind,
  blockoutShapeProgram,
}: UseCadWorkbenchPanelViewportStateInput): ReturnType<typeof buildReferenceViewportPresentation> & {
  viewportReadoutText: string
} {
  const viewportState = useMemo(
    () => buildReferenceViewportPresentation({
      projectId: conceptProjectId,
      activeAgentAssetVersionDomainPackId,
      agentPlanDomainPackId,
      activeAgentAssetVersionId,
      isExternalGlbReference,
      referenceViewport,
      blockoutGlbBase64,
      blockoutGlbKind,
      blockoutShapeProgram,
    }),
    [
      conceptProjectId,
      activeAgentAssetVersionDomainPackId,
      agentPlanDomainPackId,
      activeAgentAssetVersionId,
      isExternalGlbReference,
      referenceViewport?.evidenceId,
      referenceViewport?.kind,
      referenceViewport?.kind === 'glb' ? referenceViewport.glb : undefined,
      referenceViewport?.projectId,
      referenceViewport?.kind === 'image' ? referenceViewport.imageUrl : undefined,
      referenceViewport?.referenceClass,
      referenceViewport?.sourceObjectSha256,
      blockoutGlbBase64,
      blockoutGlbKind,
      blockoutShapeProgram,
    ],
  )

  const viewportReadoutText = useMemo(
    () => buildViewportReadoutText({
      isPreviewActive,
      hasActiveAgentAsset,
      legacyDetailsEnabled,
    }),
    [isPreviewActive, hasActiveAgentAsset, legacyDetailsEnabled],
  )

  return {
    ...viewportState,
    viewportReadoutText,
  }
}
