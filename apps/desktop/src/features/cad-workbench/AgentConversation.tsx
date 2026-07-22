import { Sparkle } from '@phosphor-icons/react'
import type { ChangeEvent } from 'react'
import type { AgentItem, MechanicalConceptPlan } from '../../shared/types'
import type { ProviderConfigMetadata } from '../../shared/tauri/agentSupervisor'
import type { AgentClarification, AgentClarificationOption } from './agentConversationState.js'
import { AgentStepItem } from './AgentStepItem.js'
import { LegacyCompatibilityNotice } from './LegacyCompatibilityNotice.js'
import type { LegacyCompatibilityDisplay } from './legacyCompatibilityDisplay.js'
import type { AgentBlockoutPreviewPresentation } from './agentBlockoutPreviewPresentation.js'
import type { AgentPlanSourcePresentation } from './agentPlanSourcePresentation.js'
import { providerConfigPresentation } from './providerConnectionPresentation.js'

export type { AgentClarification, AgentClarificationOption } from './agentConversationState.js'

export type AgentConversationSuggestion = readonly [label: string, prompt: string]

export type AgentConversationProps = {
  loading: boolean
  projectExists: boolean
  projectIsEmpty: boolean
  legacyCompatibility: LegacyCompatibilityDisplay
  onRequestLegacyAgentRebuild: () => void | Promise<void>
  onOpenLegacyDetails: () => void | Promise<void>
  providerConfig: ProviderConfigMetadata | null
  providerSetupOpen: boolean
  providerBaseUrl: string
  providerModel: string
  providerApiKey: string
  providerSaving: boolean
  onToggleProviderSetup: () => void
  onProviderBaseUrlChange: (value: string) => void
  onProviderModelChange: (value: string) => void
  onProviderApiKeyChange: (value: string) => void
  onCancelProviderSetup: () => void
  onTestProvider: () => void | Promise<void>
  onSaveProvider: () => void | Promise<void>
  activeProviderTurnId: string | null
  onCancelProviderTurn: () => void | Promise<void>
  assistantMode: 'brief' | 'change'
  selectedNode: string | null
  selectedModuleLabel: string
  assistantNote: string
  errorMessage?: string | null
  blockoutPreviewPresentation: AgentBlockoutPreviewPresentation | null
  agentPlanSourcePresentation: AgentPlanSourcePresentation | null
  conceptFamilySuggestions: readonly AgentConversationSuggestion[]
  presentationProfile: 'quick_sketch' | 'showcase'
  styleOptionsOpen: boolean
  onAssistantModeChange: (mode: 'brief' | 'change') => void
  onSuggestionSelect: (prompt: string) => void
  onPresentationProfileChange: (profile: 'quick_sketch' | 'showcase') => void
  onClarificationSelect: (option: AgentClarificationOption) => void
  agentClarification: AgentClarification | null
  agentKernelItems: AgentItem[]
  agentKernelUnavailable: boolean
  agentPlan: MechanicalConceptPlan | null
}

export function AgentConversation({
  loading,
  projectExists,
  projectIsEmpty,
  legacyCompatibility,
  onRequestLegacyAgentRebuild,
  onOpenLegacyDetails,
  providerConfig,
  providerSetupOpen,
  providerBaseUrl,
  providerModel,
  providerApiKey,
  providerSaving,
  onToggleProviderSetup,
  onProviderBaseUrlChange,
  onProviderModelChange,
  onProviderApiKeyChange,
  onCancelProviderSetup,
  onTestProvider,
  onSaveProvider,
  activeProviderTurnId,
  onCancelProviderTurn,
  assistantMode,
  selectedNode,
  selectedModuleLabel,
  assistantNote,
  errorMessage,
  blockoutPreviewPresentation,
  agentPlanSourcePresentation,
  conceptFamilySuggestions,
  presentationProfile,
  styleOptionsOpen,
  onAssistantModeChange,
  onSuggestionSelect,
  onPresentationProfileChange,
  onClarificationSelect,
  agentClarification,
  agentKernelItems,
  agentKernelUnavailable,
  agentPlan,
}: AgentConversationProps) {
  const providerPresentation = providerConfigPresentation(providerConfig)
  return (
    <>
      {!projectExists && !loading && (
        <div className="agent-empty-project" data-testid="agent-no-project" role="status">
          <strong>从左侧开始新设计</strong>
          <span>创建项目后即可在下方描述模型；工作台不会预先生成方向或资产。</span>
        </div>
      )}
      {projectExists && projectIsEmpty && !legacyCompatibility.isLegacyReadOnly && (
        <div className="agent-empty-project" data-testid="agent-empty-project" role="status">
          <strong>空项目已就绪</strong>
          <span>直接在下方描述你想要的模型；Agent 会生成第一个 3D 资产，无需先准备旧组件。</span>
        </div>
      )}
      <p className="agent-welcome">用一句话描述汽车、飞机、机械臂或未来概念道具；我会记录理解、执行受限步骤，并只在工作台展示当前结果。</p>
      <LegacyCompatibilityNotice
        display={legacyCompatibility}
        onRequestLegacyAgentRebuild={onRequestLegacyAgentRebuild}
        onOpenLegacyDetails={onOpenLegacyDetails}
      />
      <div className="provider-setup-entry" aria-label="模型服务状态">
        <span className={providerPresentation.ready ? 'connected' : ''}>
          {providerPresentation.label}
        </span>
          <button type="button" onClick={onToggleProviderSetup} aria-expanded={providerSetupOpen} aria-controls="forgecad-provider-setup">
          {providerSetupOpen ? '收起配置' : '配置模型服务'}
        </button>
      </div>
      {providerSetupOpen && (
        <div id="forgecad-provider-setup" className="provider-setup-card" aria-label="配置模型服务">
          <strong>连接你的大模型 API</strong>
          <small>API Key 只保存到 macOS Keychain，不写入项目、版本或导出包。</small>
          <label><span>API Base URL</span><input value={providerBaseUrl} onChange={(event: ChangeEvent<HTMLInputElement>) => onProviderBaseUrlChange(event.target.value)} placeholder="https://api.deepseek.com" /></label>
          <label><span>Model</span><input value={providerModel} onChange={(event: ChangeEvent<HTMLInputElement>) => onProviderModelChange(event.target.value)} placeholder="deepseek-v4-pro" /></label>
          <label><span>API Key</span><input type="password" value={providerApiKey} onChange={(event: ChangeEvent<HTMLInputElement>) => onProviderApiKeyChange(event.target.value)} placeholder="只在本次配置时输入" autoComplete="off" /></label>
          <div className="provider-setup-actions">
            <button type="button" onClick={onCancelProviderSetup} disabled={providerSaving}>取消</button>
            <button type="button" onClick={onTestProvider} disabled={providerSaving || !providerPresentation.canTest}>测试连接（会联网）</button>
            <button type="button" className="primary" onClick={onSaveProvider} disabled={providerSaving}>{providerSaving ? '保存并连接中…' : '保存并连接'}</button>
          </div>
          <small>浏览器调试预览不提供 Keychain；请按操作文档使用 secret file 启动 Agent。</small>
        </div>
      )}
      {activeProviderTurnId && (
        <button type="button" className="empty-action" onClick={onCancelProviderTurn}>
          取消本次模型请求
        </button>
      )}
      <div className="concept-family-suggestions" aria-label="概念家族">
        <span>从一个方向开始</span>
        <div>
          {conceptFamilySuggestions.map(([label, prompt]) => (
            <button key={label} type="button" onClick={() => { onAssistantModeChange('brief'); onSuggestionSelect(prompt) }}>{label}</button>
          ))}
        </div>
      </div>
      {styleOptionsOpen && <div className="presentation-profile" aria-label="外观生成质量">
        <span>外观生成质量</span>
        <div>
          <button type="button" aria-pressed={presentationProfile === 'quick_sketch'} onClick={() => onPresentationProfileChange('quick_sketch')}>
            快速草图
            <small>先看整体轮廓</small>
          </button>
          <button type="button" className="primary" aria-pressed={presentationProfile === 'showcase'} onClick={() => onPresentationProfileChange('showcase')}>
            展示模型
            <small>增加外观分层细节</small>
          </button>
        </div>
      </div>}
      {selectedNode && (
        <button
          type="button"
          className={`agent-selection-context ${assistantMode === 'change' ? 'active' : ''}`}
          onClick={() => onAssistantModeChange('change')}
        >正在调整：{selectedModuleLabel}</button>
      )}
      <div
        className={`assistant-message ${errorMessage ? 'error' : ''}`}
        role={errorMessage ? 'alert' : 'status'}
        aria-live={errorMessage ? 'assertive' : 'polite'}
      >{errorMessage ?? assistantNote}</div>
      {blockoutPreviewPresentation && (
        <div className={`agent-blockout-status ${blockoutPreviewPresentation.tone}`} role={blockoutPreviewPresentation.tone === 'error' ? 'alert' : 'status'} aria-live={blockoutPreviewPresentation.tone === 'error' ? 'assertive' : 'polite'}>
          <strong>{blockoutPreviewPresentation.title}</strong>
          <small>{blockoutPreviewPresentation.detail}</small>
        </div>
      )}
      {agentClarification && (
        <div className="agent-clarification" role="group" aria-label={agentClarification.kind === 'scope' ? '当前请求超出概念范围' : '需要确认设计类别'} aria-live="polite">
          <strong>{agentClarification.kind === 'scope' ? '请换一种外观创意描述' : '先确认设计对象'}</strong>
          {agentClarification.kind === 'domain' && agentClarification.status === 'ambiguous' && <small>这段创意同时接近多个方向，请选择一个对象类别继续。</small>}
          <p>{agentClarification.question}</p>
          {agentClarification.kind === 'domain' ? (
            <>
              <div className="agent-clarification-options">
                {agentClarification.options.map((option) => (
                  <button key={option.domain_pack_id} type="button" onClick={() => onClarificationSelect(option)} disabled={loading}>
                    {option.label}
                  </button>
                ))}
              </div>
              <small>选择后会保留你的原始创意并开启新一轮规划；在你选择前不会创建 3D 模型或版本。</small>
            </>
          ) : (
            <small>当前请求未发送给模型，也没有创建 3D 模型、版本或导出。你可以改为描述完整外观、分件、比例或视觉材质。</small>
          )}
        </div>
      )}
      {agentKernelItems.length > 0 && (
        <div className="agent-kernel-events" role="log" aria-live="polite" aria-label="Agent 步骤">
          <div className="agent-kernel-events-title">
            <span>Agent 步骤</span>
            <small>{agentKernelUnavailable ? '兼容模式' : '已记录'}</small>
          </div>
          {agentKernelItems.slice(-4).map((item) => <AgentStepItem key={item.item_id} item={item} />)}
        </div>
      )}
      {agentPlan && agentPlanSourcePresentation && (
        <div className={`agent-plan-source ${agentPlanSourcePresentation.tone}`} role="status" aria-live="polite" data-testid="f026-plan-source">
          <strong>{agentPlanSourcePresentation.title}</strong>
          <small>{agentPlanSourcePresentation.detail}</small>
          <small>工作台只构建并展示一个当前结果；不会要求你在多个方向中选择。</small>
        </div>
      )}
    </>
  )
}
