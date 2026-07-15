import { ChatCircleDots, Cube, PaperPlaneRight, Plus, Sparkle } from '@phosphor-icons/react'
import type { ChangeEvent } from 'react'
import type { AgentItem, MechanicalConceptPlan } from '../../shared/types'
import type { ProviderConfigMetadata } from '../../shared/tauri/agentSupervisor'
import type { AgentClarification, AgentClarificationOption } from './agentConversationState.js'
import { AgentStepItem } from './AgentStepItem.js'
import { LegacyCompatibilityNotice } from './LegacyCompatibilityNotice.js'
import type { LegacyCompatibilityDisplay } from './legacyCompatibilityDisplay.js'
import type { AgentBlockoutPreviewPresentation } from './agentBlockoutPreviewPresentation.js'
import type { AgentPlanSourcePresentation } from './agentPlanSourcePresentation.js'
import type { AgentDirectionConceptPreview } from './agentDirectionConceptPreviewState.js'
import { providerConfigPresentation } from './providerConnectionPresentation.js'

export type { AgentClarification, AgentClarificationOption } from './agentConversationState.js'

export type AgentConversationSuggestion = readonly [label: string, prompt: string]

export type AgentConversationProps = {
  loading: boolean
  projectExists: boolean
  projectNeedsInitialization: boolean
  legacyCompatibility: LegacyCompatibilityDisplay
  onCreateStarterProject: () => void
  onInitializeCurrentProject: () => void
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
  chatInput: string
  assistantNote: string
  errorMessage?: string | null
  blockoutPreviewPresentation: AgentBlockoutPreviewPresentation | null
  agentPlanSourcePresentation: AgentPlanSourcePresentation | null
  directionConceptPreviews: Readonly<Record<string, AgentDirectionConceptPreview>>
  conceptFamilySuggestions: readonly AgentConversationSuggestion[]
  presentationProfile: 'quick_sketch' | 'showcase'
  onAssistantModeChange: (mode: 'brief' | 'change') => void
  onChatInputChange: (value: string) => void
  onRunAssistantAction: () => void
  onSuggestionSelect: (prompt: string) => void
  onPresentationProfileChange: (profile: 'quick_sketch' | 'showcase') => void
  onClarificationSelect: (option: AgentClarificationOption) => void
  agentClarification: AgentClarification | null
  agentKernelItems: AgentItem[]
  agentKernelUnavailable: boolean
  agentPlan: MechanicalConceptPlan | null
  onPreviewDirection: (directionId: string) => void
}

export function AgentConversation({
  loading,
  projectExists,
  projectNeedsInitialization,
  legacyCompatibility,
  onCreateStarterProject,
  onInitializeCurrentProject,
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
  chatInput,
  assistantNote,
  errorMessage,
  blockoutPreviewPresentation,
  agentPlanSourcePresentation,
  directionConceptPreviews,
  conceptFamilySuggestions,
  presentationProfile,
  onAssistantModeChange,
  onChatInputChange,
  onRunAssistantAction,
  onSuggestionSelect,
  onPresentationProfileChange,
  onClarificationSelect,
  agentClarification,
  agentKernelItems,
  agentKernelUnavailable,
  agentPlan,
  onPreviewDirection,
}: AgentConversationProps) {
  const providerPresentation = providerConfigPresentation(providerConfig)
  return (
    <>
      {!projectExists && !loading && (
        <button type="button" className="empty-action" onClick={onCreateStarterProject}>
          <Plus size={14} /> 创建第一个设计
        </button>
      )}
      {projectExists && projectNeedsInitialization && (
        <button type="button" className="empty-action" onClick={onInitializeCurrentProject} disabled={loading || legacyCompatibility.isLegacyReadOnly}>
          <Cube size={14} /> 准备展示组件
        </button>
      )}
      <p className="agent-welcome">用一句话描述汽车、飞机、机械臂或未来概念道具；我会先记录理解，再生成可预览、可继续修改的方向。</p>
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
            <button type="button" onClick={onTestProvider} disabled={providerSaving || !providerPresentation.ready}>测试连接（会联网）</button>
            <button type="button" className="primary" onClick={onSaveProvider} disabled={providerSaving}>{providerSaving ? '保存并连接中…' : '保存并连接'}</button>
          </div>
          <small>浏览器调试预览不提供 Keychain；请按操作文档使用 secret file 启动 Agent。</small>
        </div>
      )}
      <div className="assistant-composer agent-composer">
        <ChatCircleDots size={17} />
        <input
          value={chatInput}
          onChange={(event) => onChatInputChange(event.target.value)}
          onKeyDown={(event) => event.key === 'Enter' && onRunAssistantAction()}
          placeholder={assistantMode === 'change' && selectedNode ? `告诉我怎么调整这个${selectedModuleLabel}…` : '描述你想设计的道具…'}
          aria-label="设计需求"
        />
        <button type="button" onClick={onRunAssistantAction} aria-label="发送设计需求" disabled={loading || !projectExists}>
          <PaperPlaneRight size={16} weight="fill" />
        </button>
      </div>
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
      <div className="presentation-profile" aria-label="外观生成质量">
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
      </div>
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
      {agentPlan && (
        <div className="assistant-directions agent-plan-directions" aria-label="Agent 完整外观方向">
          <div className="assistant-directions-heading">
            <span>Agent 完整外观方向</span>
            <small>先预览，不覆盖当前设计</small>
          </div>
          {agentPlanSourcePresentation && (
            <div className={`agent-plan-source ${agentPlanSourcePresentation.tone}`} role="status" aria-live="polite">
              <strong>{agentPlanSourcePresentation.title}</strong>
              <small>{agentPlanSourcePresentation.detail}</small>
            </div>
          )}
          {agentPlan.directions.map((direction) => (
            <button key={direction.direction_id} type="button" onClick={() => onPreviewDirection(direction.direction_id)}>
              {directionConceptPreviews[direction.direction_id]?.status === 'ready' && directionConceptPreviews[direction.direction_id]?.imageDataUrl && (
                <img
                  className="agent-direction-concept-image"
                  src={directionConceptPreviews[direction.direction_id].imageDataUrl}
                  alt={`${direction.title}的软件概念外观预览`}
                />
              )}
              <strong>{direction.title}</strong>
              <span>{direction.summary}</span>
              <small>{directionConceptPreviews[direction.direction_id]?.status === 'loading'
                ? '正在生成软件概念图… · 生成轻量 blockout'
                : directionConceptPreviews[direction.direction_id]?.status === 'failed'
                  ? '概念图暂不可用，仍可生成轻量 blockout'
                  : directionConceptPreviews[direction.direction_id]?.status === 'ready'
                    ? '软件概念图已准备好 · 生成轻量 blockout'
                    : '生成轻量 blockout'}</small>
            </button>
          ))}
        </div>
      )}
    </>
  )
}
