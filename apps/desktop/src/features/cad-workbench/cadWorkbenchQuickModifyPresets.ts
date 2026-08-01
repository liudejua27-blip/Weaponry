export type QuickModifyPreset = {
  label: string
  summary: string
  prompt: string
}

export const QUICK_MODIFY_PRESETS: readonly QuickModifyPreset[] = [
  {
    label: '更未来',
    summary: '增强未来感与科技语言',
    prompt: '请把当前模型改得更有未来感，提升流线造型和科技质感。',
  },
  {
    label: '更轻量',
    summary: '降低体量感，保持主体结构',
    prompt: '请让当前模型看起来更轻盈，减少沉重块面并保留主体比例。',
  },
  {
    label: '更工业',
    summary: '强化工业形态与结构逻辑',
    prompt: '请把当前模型风格调整得更工业化，强化结构逻辑和硬朗边缘。',
  },
  {
    label: '更真实',
    summary: '提高细节可信度和表面打磨',
    prompt: '请保留当前结构的基础上，优化外观细节、边缘过渡和材质表达，让模型更真实。',
  },
] as const

export const ASSISTANT_QUICK_MODIFY_PRESETS: readonly QuickModifyPreset[] = [
  {
    label: '更强',
    summary: '增强主体存在感与结构对比',
    prompt: '请增强当前设计的主体存在感和结构对比，但保持整体用途与主要部件不变。',
  },
  {
    label: '更轻',
    summary: '减轻视觉体量与厚重感',
    prompt: '请让当前设计看起来更轻盈，减少视觉厚重感，同时保留关键结构和比例。',
  },
  {
    label: '更智能',
    summary: '强化传感器与智能设备语言',
    prompt: '请让当前设计更有智能设备感，强化传感器、交互细节和科技语言，不改变核心结构。',
  },
  {
    label: '更紧凑',
    summary: '收紧布局并减少多余体积',
    prompt: '请把当前设计调整得更紧凑，收紧部件布局并减少多余体积，保持主体完整。',
  },
  {
    label: '增加部件',
    summary: '添加与当前对象一致的外观组件',
    prompt: '请根据当前对象的身份、结构和视觉语言增加一组清晰可编辑的外观组件，并保持整体比例与视觉平衡；不要把对象改成机械臂或机器人。',
  },
  {
    label: '修改颜色',
    summary: '调整整体配色与材质氛围',
    prompt: '请调整当前设计的整体配色和材质氛围，保持部件关系不变，并给出清晰的展示外观。',
  },
] as const
