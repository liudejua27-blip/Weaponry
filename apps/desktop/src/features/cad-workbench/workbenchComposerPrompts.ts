export type ReferenceImportCapability = 'glb_compatible_only' | 'reference_guided_rebuild'

export const COMPOSER_PANEL_LABEL = '设计输入'
export const COMPOSER_MENU_ARIA_LABEL = '添加风格、材质或参考'
export const COMPOSER_MENU_ACTIONS_LABEL = '设计附加操作'
export const COMPOSER_INPUT_PLACEHOLDER = '描述你想设计的 3D 概念模型…'
export const COMPOSER_SEND_ARIA_LABEL = '发送设计需求'
export const COMPOSER_INPUT_ARIA_LABEL = '设计需求'
export const COMPOSER_REFERENCE_SUMMARY_GLB_ONLY = '当前仅兼容 GLB；参考图引导重建待 R007。'
export const COMPOSER_REFERENCE_SUMMARY_REFERENCE_GUIDED = '参考图与 GLB 可用于引导重建。'
export const COMPOSER_SURFACE_ADORNMENT_CLOSED_HINT = '请先保存设计并选择部件与材质区。'
export const COMPOSER_SURFACE_ADORNMENT_OPEN_HINT = '在已选材质区预览，再决定是否保留。'
export const COMPOSER_BEGINNER_PROMPT_LABEL = '新手起步'
export const COMPOSER_BEGINNER_PROMPTS = [
  '一台用于展示的工业机械臂，重点突出可调节夹持器。',
  '一台适合城市巡航的未来感电动车',
  '一台可用于科幻展示的轻型飞行器',
] as const

export const COMPOSER_DEFAULT_BEGINNER_PROMPT = COMPOSER_BEGINNER_PROMPTS[0]

export function resolveReferenceImportCapabilityHint(
  capability: ReferenceImportCapability,
): string {
  return capability === 'reference_guided_rebuild'
    ? COMPOSER_REFERENCE_SUMMARY_REFERENCE_GUIDED
    : COMPOSER_REFERENCE_SUMMARY_GLB_ONLY
}
