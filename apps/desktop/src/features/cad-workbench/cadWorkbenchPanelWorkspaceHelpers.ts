type WorkbenchSidebarPart = {
  part_id: string
  role: string
  material_zone_ids: readonly string[]
}

const EMPTY_NODE_IDS: readonly string[] = []
const EMPTY_WORKBENCH_PARTS: readonly WorkbenchSidebarPart[] = []
const DEFAULT_HIDDEN_NODE_IDS = ['node_storage']

export type LegacyGraphWorkspaceInput = {
  projectIdForWorkspace: string | null
  projectIdForOverlay: string | null
  graphIdForOverlay: string | null
  hiddenNodeIds: readonly string[]
}

export function resolveWorkbenchSidebarParts(
  agentParts: readonly WorkbenchSidebarPart[] | null | undefined,
  blockoutParts: readonly WorkbenchSidebarPart[] | null | undefined,
): readonly WorkbenchSidebarPart[] {
  if (agentParts && agentParts.length > 0) return agentParts
  if (blockoutParts && blockoutParts.length > 0) return blockoutParts
  return EMPTY_WORKBENCH_PARTS
}

export function resolveLegacyGraphWorkspaceInput(
  isLegacyReadOnly: boolean,
  legacyDetailsEnabled: boolean,
  projectId: string | null,
  graphId: string | null,
): LegacyGraphWorkspaceInput {
  if (!isLegacyReadOnly || !legacyDetailsEnabled) {
    return {
      projectIdForWorkspace: null,
      projectIdForOverlay: null,
      graphIdForOverlay: null,
      hiddenNodeIds: EMPTY_NODE_IDS,
    }
  }

  return {
    projectIdForWorkspace: projectId,
    projectIdForOverlay: projectId,
    graphIdForOverlay: graphId,
    hiddenNodeIds: DEFAULT_HIDDEN_NODE_IDS,
  }
}
