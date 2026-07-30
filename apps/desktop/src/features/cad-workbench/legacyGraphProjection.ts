type LegacyGraphNode = {
  node_id: string
  module_id?: string | null
  locked?: boolean
}

export type LegacyGraphSelectionNode = {
  nodeId: string
  moduleId: string
  locked: boolean
}

export type LegacyGraphProjection = {
  selectionNodes: LegacyGraphSelectionNode[]
  overlayNodeIds: string[]
}

export function projectLegacyGraphSelectionNodes(nodes: readonly LegacyGraphNode[]): LegacyGraphProjection {
  const total = nodes.length
  const selectionNodes: LegacyGraphSelectionNode[] = new Array(total)
  const overlayNodeIds: string[] = new Array(total)
  for (let index = 0; index < total; index += 1) {
    const node = nodes[index]
    selectionNodes[index] = {
      nodeId: node.node_id,
      moduleId: node.module_id ?? '',
      locked: Boolean(node.locked),
    }
    overlayNodeIds[index] = node.node_id
  }
  return { selectionNodes, overlayNodeIds }
}
