import type { AgentComponentCandidate, AgentPartEditOperation, AgentStructureSuggestion } from '../../shared/types'

export const EDIT_NO_ASSET_NOTICE = '当前没有可编辑机械臂资产；请先生成并确认一个机械臂。'
export const EDIT_VERSION_MISMATCH_NOTICE = '当前机械臂版本已经变化；这条修改已安全丢弃，请重新描述一次。'
export const ASSET_SAVE_SUCCESS_TEMPLATE = '已保存「{displayName}」到当前项目的 Agent 部件库。'
export const MATERIAL_PRESET_NO_ZONE_NOTICE = '当前部件没有可写入的稳定材质区；未创建 ChangeSet。'

export function buildReplaceComponentOperation(
  partId: string,
  candidate: AgentComponentCandidate,
): AgentPartEditOperation {
  return {
    operation_id: `op_replace_${Date.now().toString(36)}`,
    op: 'replace_part',
    part_id: partId,
    replacement_component_id: candidate.component.component_id,
  }
}

export function buildStructureSuggestionOperation(suggestion: AgentStructureSuggestion): AgentPartEditOperation {
  return suggestion.kind === 'split_part'
    ? {
      operation_id: `op_split_${Date.now().toString(36)}`,
      op: 'split_part',
      part_id: suggestion.part_id,
      structure_suggestion_id: suggestion.suggestion_id,
    }
    : {
      operation_id: `op_merge_${Date.now().toString(36)}`,
      op: 'merge_parts',
      part_id: suggestion.part_id,
      target_part_id: suggestion.target_part_id ?? undefined,
      structure_suggestion_id: suggestion.suggestion_id,
    }
}

export function buildMaterialPresetSummary(
  selectedMaterialZoneId: string | null | undefined,
  presetDisplayName: string,
): string {
  return selectedMaterialZoneId
    ? `将${selectedMaterialZoneId}换成${presetDisplayName}`
    : `将材质区换成${presetDisplayName}`
}

export function buildSavedComponentDisplayName(roleLabel: string): string {
  return `${roleLabel} · 可复用部件`
}

export function buildSavedComponentDescription(versionNo: number): string {
  return `来自 Agent 资产 v${versionNo} 的概念部件`
}

export function buildSavedComponentSaveNotice(displayName: string): string {
  return ASSET_SAVE_SUCCESS_TEMPLATE.replace('{displayName}', displayName)
}

export function buildBlockoutMaterialPreviewNote(presetDisplayName: string): string {
  return `已将 blockout 预览材质切换为「${presetDisplayName}」；保存为可编辑模型后才能确认材质版本。`
}

export function buildMaterialPreviewNote(presetDisplayName: string): string {
  return `已将预览材质切换为「${presetDisplayName}」；确认前不会写入版本。`
}
