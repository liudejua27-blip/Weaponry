import type { ReferenceEvidenceTarget } from './referenceEvidenceDrawerLogic.js'

const DEFAULT_REFERENCE_DOMAIN_PACK_ID = 'pack_unclassified'

export type ReferenceViewportImage = {
  url: string
  evidenceId: string
  sourceObjectSha256: string
  referenceClass: 'single_image' | 'multi_view_contact_sheet'
}

export type ReferenceViewportShapeProgram = Record<string, unknown> | null
export type ReferenceViewportGlbKind =
  | 'external_reference'
  | 'compiled_agent_pbr'
  | 'compiled_agent_preview_pbr'
  | 'compiled_agent_production_pbr'
  | null

export type ReferenceViewportViewState = {
  readonly referenceEvidenceTarget: ReferenceEvidenceTarget | null
  readonly viewportGlb: ArrayBuffer | string | null
  readonly viewportGlbKind: ReferenceViewportGlbKind
  readonly viewportShapeProgram: ReferenceViewportShapeProgram
  readonly viewportReferenceImage: ReferenceViewportImage | null
}

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

type ReferenceViewportInput = {
  projectId: string | null
  activeAgentAssetVersionDomainPackId: string | null
  agentPlanDomainPackId: string | null
  activeAgentAssetVersionId: string | null
  isExternalGlbReference: boolean
  referenceViewport: ReferenceViewportState | null
  blockoutGlbBase64: string | ArrayBuffer | null
  blockoutGlbKind: ReferenceViewportGlbKind
  blockoutShapeProgram: ReferenceViewportShapeProgram
}

export function buildReferenceViewportPresentation(input: ReferenceViewportInput): ReferenceViewportViewState {
  const {
    projectId,
    activeAgentAssetVersionDomainPackId,
    agentPlanDomainPackId,
    activeAgentAssetVersionId,
    isExternalGlbReference,
    referenceViewport,
    blockoutGlbBase64,
    blockoutGlbKind,
    blockoutShapeProgram,
  } = input

  const hasProject = projectId !== null
  const activeReferenceViewport = hasProject && referenceViewport?.projectId === projectId
    ? referenceViewport
    : null
  const referenceEvidenceTarget = hasProject
    ? {
      projectId,
      domainPackId: activeAgentAssetVersionDomainPackId ?? agentPlanDomainPackId ?? DEFAULT_REFERENCE_DOMAIN_PACK_ID,
      baseAssetVersionId: isExternalGlbReference ? null : activeAgentAssetVersionId,
    }
    : null
  const viewportGlb = activeReferenceViewport
    ? activeReferenceViewport.kind === 'glb' ? activeReferenceViewport.glb : null
    : blockoutGlbBase64
  const viewportGlbKind = activeReferenceViewport
    ? activeReferenceViewport.kind === 'glb' ? 'external_reference' : null
    : blockoutGlbKind
  const viewportShapeProgram = activeReferenceViewport ? null : blockoutShapeProgram
  const viewportReferenceImage = activeReferenceViewport?.kind === 'image'
    ? {
      url: activeReferenceViewport.imageUrl,
      evidenceId: activeReferenceViewport.evidenceId,
      sourceObjectSha256: activeReferenceViewport.sourceObjectSha256,
      referenceClass: activeReferenceViewport.referenceClass,
    }
    : null

  return {
    referenceEvidenceTarget,
    viewportGlb,
    viewportGlbKind,
    viewportShapeProgram,
    viewportReferenceImage,
  }
}
