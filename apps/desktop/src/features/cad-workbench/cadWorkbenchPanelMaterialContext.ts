type CadWorkbenchPanelMaterialCatalogContext = {
  projectId: string | null
  domainPackId: string | null
  source: 'external_glb' | 'agent_asset' | 'blockout' | 'none'
  assetVersionId: string | null
}

type CadWorkbenchPanelMaterialContextSource = {
  projectId: string | null
  domainPackId: string | null
  source: 'external_glb' | 'legacy' | 'agent_asset' | 'blockout' | 'none'
}

export function buildCadWorkbenchPanelMaterialCatalogContext(input: {
  conceptProjectId: string | null
  activeAssetDomainPackId: string | null
  activeAssetVersionId: string | null
  agentPlanDomainPackId: string | null
  isExternalGlbReference: boolean
  hasActiveAsset: boolean
  hasAgentPlan: boolean
}): CadWorkbenchPanelMaterialCatalogContext {
  const domainPackId = input.activeAssetDomainPackId ?? input.agentPlanDomainPackId
  if (input.isExternalGlbReference) {
    return {
      assetVersionId: input.activeAssetVersionId,
      projectId: input.conceptProjectId,
      domainPackId,
      source: 'external_glb',
    }
  }

  if (input.hasActiveAsset) {
    return {
      assetVersionId: input.activeAssetVersionId,
      projectId: input.conceptProjectId,
      domainPackId,
      source: 'agent_asset',
    }
  }

  if (input.hasAgentPlan) {
    return {
      assetVersionId: input.activeAssetVersionId,
      projectId: input.conceptProjectId,
      domainPackId,
      source: 'blockout',
    }
  }

  return {
    assetVersionId: input.activeAssetVersionId,
    projectId: input.conceptProjectId,
    domainPackId,
    source: 'none',
  }
}

export function buildCadWorkbenchPanelMaterialFilterContext(input: {
  conceptProjectId: string | null
  activeAssetDomainPackId: string | null
  activeAssetVersionId: string | null
  agentPlanDomainPackId: string | null
  isExternalGlbReference: boolean
  legacyDesignReadOnly: boolean
  hasActiveAsset: boolean
  hasAgentPlan: boolean
}): CadWorkbenchPanelMaterialContextSource {
  const domainPackId = input.activeAssetDomainPackId ?? input.agentPlanDomainPackId
  if (input.isExternalGlbReference) {
    return {
      projectId: input.conceptProjectId,
      domainPackId,
      source: 'external_glb',
    }
  }

  if (input.legacyDesignReadOnly) {
    return {
      projectId: input.conceptProjectId,
      domainPackId,
      source: 'legacy',
    }
  }

  if (input.hasActiveAsset && input.activeAssetVersionId) {
    return {
      projectId: input.conceptProjectId,
      domainPackId,
      source: 'agent_asset',
    }
  }

  if (input.hasAgentPlan) {
    return {
      projectId: input.conceptProjectId,
      domainPackId,
      source: 'blockout',
    }
  }

  return {
    projectId: input.conceptProjectId,
    domainPackId,
    source: 'none',
  }
}
