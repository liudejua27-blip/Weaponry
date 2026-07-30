import { useMemo } from 'react'
import type { ActiveDesignPartDisplay, ActiveDesignSnapshot, AgentMaterialPreset } from '../../shared/types'
import { activeDesignPartDisplay, activeDesignPartIsLocked } from './activeDesignMachine'
import { resolveAgentMaterialDisplayId } from './agentMaterialPreselectionPresentationState'
import { compatibleQuickMaterialPresets } from './agentMaterialQuickActions'
import { buildSurfaceAdornmentDisabledReason } from './surfaceAdornmentPresentation'
import { resolveCadWorkbenchPanelMaterialPreselectionContext } from './cadWorkbenchPanelMaterialPreselectionContext'
import { displayPartRole } from './partRoleLabels.js'
import type { AgentMaterialPreselectionPresentationState } from './agentMaterialPreselectionPresentationState'
import type { SurfaceAdornmentTarget } from './SurfaceAdornmentDrawer'

type CadWorkbenchPanelPartLike = {
  part_id: string
  role: string
  material_zone_ids: readonly string[]
}

type UseCadWorkbenchPanelMaterialStateInput = {
  conceptProjectId: string | null
  activeAgentAssetVersionId: string | null
  activeDesignSnapshot: ActiveDesignSnapshot | null
  activeDesignIsIdle: boolean
  isExternalGlbReference: boolean
  selectedAgentPart: CadWorkbenchPanelPartLike | null
  selectedMaterialZoneId: string | null
  legacyDesignReadOnly: boolean
  hasAgentPlan: boolean
  hasActiveAgentAssetVersion: boolean
  materialBindings: Record<string, unknown> | null | undefined
  hasAgentAssetChangeSet: boolean
  surfaceAdornmentOpen: boolean
  activeMaterialDomain: string | null
  materialPresets: readonly AgentMaterialPreset[]
  agentMaterialPreselectionPresentation: AgentMaterialPreselectionPresentationState
}

export type CadWorkbenchPanelMaterialState = {
  activePartDisplay: ActiveDesignPartDisplay | null
  selectedAgentPartLocked: boolean
  selectedPartRoleLabel: string
  surfaceAdornmentTarget: SurfaceAdornmentTarget | null
  surfaceAdornmentDisabledReason: string | null
  materialPreselectionContext: ReturnType<typeof resolveCadWorkbenchPanelMaterialPreselectionContext>
  appearanceMaterialId: string
  quickMaterialPresets: readonly AgentMaterialPreset[]
}

export function useCadWorkbenchPanelMaterialState({
  conceptProjectId,
  activeAgentAssetVersionId,
  activeDesignSnapshot,
  activeDesignIsIdle,
  isExternalGlbReference,
  selectedAgentPart,
  selectedMaterialZoneId,
  legacyDesignReadOnly,
  hasAgentPlan,
  hasActiveAgentAssetVersion,
  materialBindings,
  hasAgentAssetChangeSet,
  surfaceAdornmentOpen,
  activeMaterialDomain,
  materialPresets,
  agentMaterialPreselectionPresentation,
}: UseCadWorkbenchPanelMaterialStateInput): CadWorkbenchPanelMaterialState {
  const activePartDisplay = useMemo(
    () => activeDesignPartDisplay(activeDesignSnapshot),
    [activeDesignSnapshot],
  )
  const selectedAgentPartLocked = useMemo(
    () => (selectedAgentPart ? activeDesignPartIsLocked(activeDesignSnapshot, selectedAgentPart.part_id) : false),
    [activeDesignSnapshot, selectedAgentPart],
  )
  const selectedPartRoleLabel = selectedAgentPart
    ? `已选部件 · ${displayPartRole(selectedAgentPart.role)}`
    : '当前预览部件'
  const surfaceAdornmentTarget = useMemo<SurfaceAdornmentTarget | null>(() => {
    if (!conceptProjectId || !activeAgentAssetVersionId || !selectedAgentPart || !selectedMaterialZoneId) return null
    return {
      projectId: conceptProjectId,
      assetVersionId: activeAgentAssetVersionId,
      partId: selectedAgentPart.part_id,
      partLabel: displayPartRole(selectedAgentPart.role),
      materialZoneId: selectedMaterialZoneId,
      materialZoneLabel: `材质区 ${selectedMaterialZoneId}`,
    }
  }, [activeAgentAssetVersionId, conceptProjectId, selectedAgentPart, selectedMaterialZoneId])

  const surfaceAdornmentDisabledReason = useMemo(
    () => buildSurfaceAdornmentDisabledReason({
      hasActiveAgentAssetVersion,
      isExternalGlbReference,
      hasSelectedPart: Boolean(selectedAgentPart),
      isSelectedPartLocked: selectedAgentPartLocked,
      hasMaterialZone: Boolean(selectedMaterialZoneId),
      hasChangeSet: hasAgentAssetChangeSet,
      isSurfaceAdornmentOpen: surfaceAdornmentOpen,
      isDesignIdle: activeDesignIsIdle,
    }),
    [
      hasActiveAgentAssetVersion,
      activeDesignIsIdle,
      isExternalGlbReference,
      hasAgentAssetChangeSet,
      surfaceAdornmentOpen,
      selectedAgentPart,
      selectedAgentPartLocked,
      selectedMaterialZoneId,
    ],
  )
  const materialPreselectionContext = useMemo(() => resolveCadWorkbenchPanelMaterialPreselectionContext({
    projectId: conceptProjectId,
    isExternalGlbReference,
    legacyDesignReadOnly,
    assetVersionId: activeAgentAssetVersionId,
    selectedPartId: selectedAgentPart?.part_id ?? null,
    selectedMaterialZoneId: selectedMaterialZoneId || null,
    hasAgentAssetVersion: hasActiveAgentAssetVersion,
    hasAgentPlan,
  }), [
    activeAgentAssetVersionId,
    conceptProjectId,
    hasAgentPlan,
    isExternalGlbReference,
    legacyDesignReadOnly,
    selectedAgentPart?.part_id,
    selectedMaterialZoneId,
  ])

  const committedMaterialBinding = selectedAgentPart && selectedMaterialZoneId
    ? materialBindings?.[`${selectedAgentPart.part_id}:${selectedMaterialZoneId}`] ?? null
    : null
  const committedMaterialId = typeof committedMaterialBinding === 'string' ? committedMaterialBinding : null
  const appearanceMaterialId = resolveAgentMaterialDisplayId(
    agentMaterialPreselectionPresentation,
    materialPreselectionContext,
    committedMaterialId,
  )
  const quickMaterialPresets = useMemo(
    () => compatibleQuickMaterialPresets(materialPresets, activeMaterialDomain),
    [activeMaterialDomain, materialPresets],
  )

  return {
    activePartDisplay,
    selectedAgentPartLocked,
    selectedPartRoleLabel,
    surfaceAdornmentTarget,
    surfaceAdornmentDisabledReason,
    materialPreselectionContext,
    appearanceMaterialId,
    quickMaterialPresets,
  }
}
