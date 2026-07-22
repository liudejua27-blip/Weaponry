import type { AgentBlockoutDisplayState } from './agentBlockoutDisplayState.js'

export type AgentBlockoutPreviewPresentation = {
  tone: 'working' | 'ready' | 'notice' | 'error'
  title: string
  detail: string
}

/**
 * Converts an already-authoritative display projection into one small,
 * beginner-readable status. This selector neither starts work nor owns any
 * version, asset, Snapshot, quality, export, or renderer state.
 */
export function selectAgentBlockoutPreviewPresentation(
  state: AgentBlockoutDisplayState,
): AgentBlockoutPreviewPresentation | null {
  if (!state.directionId) return null
  if (state.directionPreviewLoading) {
    return {
      tone: 'working',
      title: '正在生成完整外观预览',
      detail: '正在整理外观和可编辑部件，不会影响已保存设计。',
    }
  }
  if (state.previewError === 'segmentation_failed') {
    return {
      tone: 'notice',
      title: '完整外观已生成',
      detail: '暂时不能整理可编辑部件。请修改描述后重新生成；当前设计没有被写入。',
    }
  }
  if (state.previewError === 'blockout_failed') {
    return {
      tone: 'error',
      title: '这次预览没有生成成功',
      detail: '已保存设计没有变化。可再试一次，或修改描述后重新生成。',
    }
  }
  if (state.segmentation) {
    return {
      tone: 'ready',
      title: '完整外观预览已准备好',
      detail: '可以确认保存为可编辑模型，或继续用自然语言修改。',
    }
  }
  return null
}
