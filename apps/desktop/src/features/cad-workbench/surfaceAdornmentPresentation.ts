export type SurfaceAdornmentDisabledReasonInput = {
  hasActiveAgentAssetVersion: boolean
  isExternalGlbReference: boolean
  hasSelectedPart: boolean
  isSelectedPartLocked: boolean
  hasMaterialZone: boolean
  hasChangeSet: boolean
  isSurfaceAdornmentOpen: boolean
  isDesignIdle: boolean
}

export function buildSurfaceAdornmentDisabledReason(input: SurfaceAdornmentDisabledReasonInput): string | null {
  if (!input.hasActiveAgentAssetVersion) {
    return '请先确认保存当前设计，再添加外观细节。'
  }
  if (input.isExternalGlbReference) {
    return '导入参考模型不能直接编辑；请先让 Agent 重建为可编辑设计。'
  }
  if (!input.hasSelectedPart) {
    return '请先从左侧选择一个部件。'
  }
  if (input.isSelectedPartLocked) {
    return '当前部件已锁定，请先解除锁定。'
  }
  if (!input.hasMaterialZone) {
    return '当前部件没有可编辑的材质区。'
  }
  // A005 预览保留/取消生命周期已在 A005 侧层承载；当存在未提交变更时
  // 仅阻断新入口，避免隐藏当前预览的关闭与确认动作。
  if (input.hasChangeSet && !input.isSurfaceAdornmentOpen) {
    return '请先保留或取消当前预览，再添加外观细节。'
  }
  if (!input.isDesignIdle) {
    return '正在同步当前设计，请稍后再试。'
  }
  return null
}
