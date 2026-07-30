import { useEffect, useMemo } from 'react'
import { projectLegacyGraphSelectionNodes } from './legacyGraphProjection.js'
import type { LegacyModuleGraphNode } from './legacyModuleGraphWorkspaceState.js'
import {
  type LegacyGraphWorkspaceInput,
  resolveLegacyGraphWorkspaceInput,
} from './cadWorkbenchPanelWorkspaceHelpers'

type LegacyWorkbenchGraphRecord = {
  graph: {
    nodes: readonly {
      readonly node_id: string
    }[]
    graph_id: string | null
    root_node_id: string | null
  }
}

type UseCadWorkbenchPanelLegacyGraphWorkspaceSyncInput = {
  isLegacyReadOnly: boolean
  legacyDetailsEnabled: boolean
  conceptProjectId: string | null
  conceptGraphRecord: LegacyWorkbenchGraphRecord | null
  legacyModuleGraphWorkspacePreferenceKey: string | null
  legacyModuleGraphOverlayContextKey: string | null
  openLegacyModuleGraphWorkspace: (projectId: string | null) => void
  openLegacyModuleGraphOverlay: (
    projectId: string | null,
    graphId: string | null,
    hiddenNodeIds: readonly string[],
  ) => void
  reconcileLegacyModuleGraphSelection: (selectionNodes: readonly LegacyModuleGraphNode[], rootNodeId: string | null) => void
  reconcileLegacyModuleGraphOverlayNodes: (overlayNodeIds: readonly string[]) => void
}

export function useCadWorkbenchPanelLegacyGraphWorkspaceSync({
  isLegacyReadOnly,
  legacyDetailsEnabled,
  conceptProjectId,
  conceptGraphRecord,
  legacyModuleGraphWorkspacePreferenceKey,
  legacyModuleGraphOverlayContextKey,
  openLegacyModuleGraphWorkspace,
  openLegacyModuleGraphOverlay,
  reconcileLegacyModuleGraphSelection,
  reconcileLegacyModuleGraphOverlayNodes,
}: UseCadWorkbenchPanelLegacyGraphWorkspaceSyncInput): LegacyGraphWorkspaceInput {
  const legacyGraphWorkspaceInput = useMemo(
    () => resolveLegacyGraphWorkspaceInput(
      isLegacyReadOnly,
      legacyDetailsEnabled,
      conceptProjectId,
      conceptGraphRecord?.graph?.graph_id ?? null,
    ),
    [
      conceptGraphRecord?.graph?.graph_id,
      conceptProjectId,
      isLegacyReadOnly,
      legacyDetailsEnabled,
    ],
  )

  useEffect(() => {
    openLegacyModuleGraphWorkspace(legacyGraphWorkspaceInput.projectIdForWorkspace)
  }, [legacyGraphWorkspaceInput.projectIdForWorkspace, openLegacyModuleGraphWorkspace])

  useEffect(() => {
    openLegacyModuleGraphOverlay(
      legacyGraphWorkspaceInput.projectIdForOverlay,
      legacyGraphWorkspaceInput.graphIdForOverlay,
      legacyGraphWorkspaceInput.hiddenNodeIds,
    )
  }, [
    legacyGraphWorkspaceInput.graphIdForOverlay,
    legacyGraphWorkspaceInput.hiddenNodeIds,
    legacyGraphWorkspaceInput.projectIdForOverlay,
    openLegacyModuleGraphOverlay,
  ])

  useEffect(() => {
    if (!conceptGraphRecord) return
    const { selectionNodes, overlayNodeIds } = projectLegacyGraphSelectionNodes(conceptGraphRecord.graph.nodes)
    reconcileLegacyModuleGraphSelection(selectionNodes, conceptGraphRecord.graph.root_node_id)
    reconcileLegacyModuleGraphOverlayNodes(overlayNodeIds)
  }, [
    conceptGraphRecord,
    legacyModuleGraphOverlayContextKey,
    legacyModuleGraphWorkspacePreferenceKey,
    reconcileLegacyModuleGraphSelection,
    reconcileLegacyModuleGraphOverlayNodes,
  ])

  return legacyGraphWorkspaceInput
}
