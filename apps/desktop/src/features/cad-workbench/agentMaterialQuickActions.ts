import type { AgentMaterialPreset, AgentPartEditOperation } from '../../shared/types.js'

export function compatibleQuickMaterialPresets(
  presets: AgentMaterialPreset[],
  activeDomain: string | null,
  limit = 5,
): AgentMaterialPreset[] {
  if (!activeDomain) return []
  return presets
    .filter((preset) => preset.allowed_domains.includes(activeDomain))
    .slice(0, limit)
}

export function createQuickMaterialPreviewOperation(input: {
  operationId: string
  partId: string
  materialId: string
  materialZoneId: string
}): AgentPartEditOperation | null {
  if (!input.partId.trim() || !input.materialZoneId.trim()) return null
  return {
    operation_id: input.operationId,
    op: 'apply_material_preset',
    part_id: input.partId,
    material_id: input.materialId,
    material_zone_id: input.materialZoneId,
  }
}
