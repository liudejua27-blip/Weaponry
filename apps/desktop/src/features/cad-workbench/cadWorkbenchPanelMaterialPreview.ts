import type { AgentMaterialPreset, AgentPartEditOperation } from '../../shared/types'

import { createQuickMaterialPreviewOperation } from './agentMaterialQuickActions'

const QUICK_MATERIAL_PREVIEW_OPERATION_ID = 'op_material_'

type QuickMaterialPreviewInput = {
  materialId: AgentMaterialPreset['material_id']
  partId: string
  materialZoneId: string | null | undefined
}

export function createQuickMaterialPreviewOperationForPreset(
  input: QuickMaterialPreviewInput,
): AgentPartEditOperation | null {
  const partId = input.partId.trim()
  const materialId = input.materialId.trim()
  const materialZoneId = input.materialZoneId?.trim() ?? ''
  if (!partId || !materialId || !materialZoneId) return null
  return createQuickMaterialPreviewOperation({
    operationId: `${QUICK_MATERIAL_PREVIEW_OPERATION_ID}${Date.now().toString(36)}`,
    partId,
    materialId,
    materialZoneId,
  })
}
