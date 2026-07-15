import type { LegacyCompatibilityDisplay } from './legacyCompatibilityDisplay.js'

export type LegacyCompatibilityNoticeProps = {
  display: LegacyCompatibilityDisplay
  onRequestLegacyAgentRebuild: () => void | Promise<void>
}

/** Keeps legacy conversion guidance separate from the Agent-first conversation. */
export function LegacyCompatibilityNotice({
  display,
  onRequestLegacyAgentRebuild,
}: LegacyCompatibilityNoticeProps) {
  if (!display.showRebuildGuidance) return null
  return (
    <div className="agent-legacy-notice" aria-label="旧版设计转换">
      <strong>这是旧版只读设计</strong>
      <span>原设计会保留不变。确认后，Agent 会根据你的新描述重建设计为可编辑资产。</span>
      <button type="button" onClick={onRequestLegacyAgentRebuild} disabled={!display.rebuildActionEnabled}>
        让 Agent 重建可编辑资产
      </button>
    </div>
  )
}
