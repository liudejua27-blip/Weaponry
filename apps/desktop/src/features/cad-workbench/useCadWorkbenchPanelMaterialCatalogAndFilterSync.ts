import { useEffect } from 'react'

import type { ForgeApi } from '../../shared/api/forgeApi'
import type { ComponentCatalogContext } from './componentCatalogPresentationState'
import type { AgentMaterialCatalogContext } from './agentMaterialCatalogPresentationState'
import type { ModuleAssetRecord, AgentMaterialPreset } from '../../shared/types'
import { buildCadWorkbenchPanelMaterialCatalogContext, buildCadWorkbenchPanelMaterialFilterContext } from './cadWorkbenchPanelMaterialContext'
import type { AgentMaterialFilterContext } from './agentMaterialFilterPresentationState'
import { DEFAULT_AGENT_MATERIAL_PRESETS } from './cadWorkbenchPanelPrompts'
import type { AgentMaterialPreselectionContext } from './agentMaterialPreselectionPresentationState'
import type { CadWorkbenchPanelMaterialPreselectionContext } from './cadWorkbenchPanelMaterialPreselectionContext'

type UseCadWorkbenchPanelMaterialCatalogAndFilterSyncInput = {
  api: ForgeApi
  conceptProjectId: string | null
  legacyDetailsEnabled: boolean
  conceptProjectPackId: string | null
  activeAgentAssetVersionId: string | null
  activeAgentAssetVersionDomainPackId: string | null
  agentPlanDomainPackId: string | null
  isExternalGlbReference: boolean
  hasActiveAsset: boolean
  hasAgentPlan: boolean
  legacyDesignReadOnly: boolean
  materialPreselectionContext: CadWorkbenchPanelMaterialPreselectionContext
  openComponentCatalog: (context: ComponentCatalogContext) => void
  startComponentCatalogRead: (context: ComponentCatalogContext) => number | null
  receiveComponentCatalog: (context: ComponentCatalogContext, requestId: number, modules: ModuleAssetRecord[]) => boolean
  failComponentCatalog: (context: ComponentCatalogContext, requestId: number) => boolean
  openAgentMaterialCatalogPresentation: (context: AgentMaterialCatalogContext) => void
  startAgentMaterialCatalogRead: (context: AgentMaterialCatalogContext) => number | null
  receiveAgentMaterialCatalog: (context: AgentMaterialCatalogContext, requestId: number, materialPresets: AgentMaterialPreset[]) => boolean
  failAgentMaterialCatalog: (context: AgentMaterialCatalogContext, requestId: number, fallbackPresets: AgentMaterialPreset[]) => boolean
  openAgentMaterialFilterPresentation: (context: AgentMaterialFilterContext) => void
  openAgentMaterialPreselectionPresentation: (context: AgentMaterialPreselectionContext) => void
}

export function useCadWorkbenchPanelMaterialCatalogAndFilterSync({
  api,
  conceptProjectId,
  legacyDetailsEnabled,
  conceptProjectPackId,
  activeAgentAssetVersionId,
  activeAgentAssetVersionDomainPackId,
  agentPlanDomainPackId,
  isExternalGlbReference,
  hasActiveAsset,
  hasAgentPlan,
  legacyDesignReadOnly,
  materialPreselectionContext,
  openComponentCatalog,
  startComponentCatalogRead,
  receiveComponentCatalog,
  failComponentCatalog,
  openAgentMaterialCatalogPresentation,
  startAgentMaterialCatalogRead,
  receiveAgentMaterialCatalog,
  failAgentMaterialCatalog,
  openAgentMaterialFilterPresentation,
  openAgentMaterialPreselectionPresentation,
}: UseCadWorkbenchPanelMaterialCatalogAndFilterSyncInput): void {
  useEffect(() => {
    const context = {
      projectId: conceptProjectId,
      packId: legacyDetailsEnabled ? conceptProjectPackId : null,
      source: legacyDetailsEnabled ? 'legacy' as const : 'none' as const,
    }
    openComponentCatalog(context)
    if (!context.packId || context.source !== 'legacy') return

    const requestId = startComponentCatalogRead(context)
    if (requestId === null) return
    void api
      .listModuleAssets(context.packId)
      .then((response) => {
        receiveComponentCatalog(context, requestId, response.items ?? [])
      })
      .catch(() => {
        failComponentCatalog(context, requestId)
      })
  }, [
    api,
    conceptProjectId,
    conceptProjectPackId,
    legacyDetailsEnabled,
    failComponentCatalog,
    openComponentCatalog,
    receiveComponentCatalog,
    startComponentCatalogRead,
  ])

  useEffect(() => {
    const context = buildCadWorkbenchPanelMaterialCatalogContext({
      conceptProjectId,
      activeAssetDomainPackId: activeAgentAssetVersionDomainPackId,
      activeAssetVersionId: activeAgentAssetVersionId,
      agentPlanDomainPackId,
      isExternalGlbReference,
      hasActiveAsset,
      hasAgentPlan,
    })
    openAgentMaterialCatalogPresentation(context)
    const requestId = startAgentMaterialCatalogRead(context)
    if (requestId === null) return
    void api.listAgentMaterials()
      .then((items) => {
        receiveAgentMaterialCatalog(context, requestId, items)
      })
      .catch(() => {
        failAgentMaterialCatalog(context, requestId, DEFAULT_AGENT_MATERIAL_PRESETS)
      })
  }, [
    activeAgentAssetVersionDomainPackId,
    activeAgentAssetVersionId,
    agentPlanDomainPackId,
    api,
    conceptProjectId,
    failAgentMaterialCatalog,
    hasActiveAsset,
    hasAgentPlan,
    isExternalGlbReference,
    openAgentMaterialCatalogPresentation,
    receiveAgentMaterialCatalog,
    startAgentMaterialCatalogRead,
  ])

  useEffect(() => {
    const context = buildCadWorkbenchPanelMaterialFilterContext({
      conceptProjectId,
      activeAssetDomainPackId: activeAgentAssetVersionDomainPackId,
      activeAssetVersionId: activeAgentAssetVersionId,
      agentPlanDomainPackId,
      isExternalGlbReference,
      legacyDesignReadOnly,
      hasActiveAsset,
      hasAgentPlan,
    })
    openAgentMaterialFilterPresentation(context)
  }, [
    activeAgentAssetVersionDomainPackId,
    conceptProjectId,
    agentPlanDomainPackId,
    activeAgentAssetVersionId,
    hasActiveAsset,
    hasAgentPlan,
    isExternalGlbReference,
    legacyDesignReadOnly,
    openAgentMaterialFilterPresentation,
  ])

  useEffect(() => {
    openAgentMaterialPreselectionPresentation(materialPreselectionContext)
  }, [materialPreselectionContext, openAgentMaterialPreselectionPresentation])
}
