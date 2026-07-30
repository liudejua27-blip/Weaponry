import type { ModuleAssetRecord } from '../../shared/types'

export type CadWorkbenchCatalogModule = Pick<ModuleAssetRecord, 'manifest'>

const MODULE_CATEGORY_LABELS: Record<ModuleAssetRecord['manifest']['category'], string> = {
  core_shell: '核心外壳',
  front_shell: '前部外壳',
  rear_shell: '后部外壳',
  grip_shell: '握持外壳',
  top_accessory: '顶部附件',
  side_accessory: '侧部附件',
  lower_structure: '下部结构',
  storage_visual: '储存视觉',
  armor_panel: '装甲面板',
}

export function buildSelectedModuleLabel(
  selectedModule: CadWorkbenchCatalogModule | null,
  fallbackLabel: string,
): string {
  if (!selectedModule) return fallbackLabel
  return MODULE_CATEGORY_LABELS[selectedModule.manifest.category] ?? selectedModule.manifest.category
}
