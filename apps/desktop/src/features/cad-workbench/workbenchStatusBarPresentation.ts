import type { ActiveDesignSnapshot } from '../../shared/types'

export type WorkbenchStatusBarPresentation = {
  assistantStateText: string
  assetStateText: string
  versionText: string
  qualityText: string
}

const QUALITY_STATUS_LABELS = {
  passed: '通过',
  warning: '需复核',
  failed: '失败',
  unavailable: '不可用',
} as const

function qualityStatusLabel(status?: 'passed' | 'warning' | 'failed' | 'not_run' | 'unavailable'): string {
  if (!status || status === 'not_run') return '未运行'
  return QUALITY_STATUS_LABELS[status]
}

export function buildWorkbenchStatusBarPresentation(input: {
  conceptLoading: boolean
  conceptLegacyDetailsEnabled: boolean
  activeAgentAssetVersionVersionNo: number | null
  activeDesignSnapshot: ActiveDesignSnapshot | null
  activeVersionSummary: { version_no: number | string } | null
  conceptQualityStatus?: 'passed' | 'warning' | 'failed' | 'not_run' | 'unavailable'
  agentQualityStatus?: 'passed' | 'warning' | 'failed' | 'unavailable' | null | undefined
}): WorkbenchStatusBarPresentation {
  const {
    conceptLoading,
    conceptLegacyDetailsEnabled,
    activeAgentAssetVersionVersionNo,
    activeDesignSnapshot,
    activeVersionSummary,
    conceptQualityStatus,
    agentQualityStatus,
  } = input

  const assistantStateText = conceptLoading ? 'Agent 正在处理' : '设计就绪'
  const agentAssetSource = activeDesignSnapshot?.active_design.source === 'agent_asset'

  const assetStateText = activeAgentAssetVersionVersionNo !== null
    ? 'Agent 资产可编辑'
    : conceptLegacyDetailsEnabled
      ? '旧版信息只读'
      : '等待 Agent 资产'

  const versionText = agentAssetSource
    ? `Agent v${activeAgentAssetVersionVersionNo ?? '同步中'}`
    : activeDesignSnapshot
      ? '旧版只读设计'
      : activeVersionSummary
        ? `v${activeVersionSummary.version_no}`
        : '草稿'

  const reportStatus = agentAssetSource
    ? qualityStatusLabel(agentQualityStatus ?? undefined)
    : qualityStatusLabel(conceptQualityStatus)

  const qualityText = `${reportStatus} · 模型检查`

  return {
    assistantStateText,
    assetStateText,
    versionText,
    qualityText,
  }
}
