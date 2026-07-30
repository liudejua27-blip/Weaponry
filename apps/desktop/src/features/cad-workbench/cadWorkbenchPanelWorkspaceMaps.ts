type LegacyGraphNodeIndex = {
  readonly node_id: string
  readonly module_id?: string | null
}

const EMPTY_NODE_ID_TO_MODULE_ID = new Map<string, string>()
const EMPTY_PART_ID_TO_PART = new Map<string, never>()

export function buildLegacyGraphNodeModuleIdMap(
  nodes: readonly LegacyGraphNodeIndex[] | null | undefined,
): ReadonlyMap<string, string> {
  if (!nodes || nodes.length === 0) return EMPTY_NODE_ID_TO_MODULE_ID

  const mapping = new Map<string, string>()
  for (let index = 0; index < nodes.length; index += 1) {
    const node = nodes[index]
    if (!node || typeof node.node_id !== 'string' || node.node_id.length === 0) continue
    const moduleId = typeof node.module_id === 'string' ? node.module_id : ''
    mapping.set(node.node_id, moduleId)
  }
  return mapping
}

export function buildAgentPartByIdMap<TPart extends { part_id: string }>(
  parts: readonly TPart[] | null | undefined,
): ReadonlyMap<string, TPart> {
  if (!parts?.length) {
    return EMPTY_PART_ID_TO_PART as ReadonlyMap<string, TPart>
  }

  const mapping = new Map<string, TPart>()
  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index]
    if (!part || part.part_id.length === 0) continue
    mapping.set(part.part_id, part)
  }
  return mapping
}
