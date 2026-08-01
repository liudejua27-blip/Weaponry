import type { ChangeEvent } from 'react'
import type { AgentItem, MechanicalConceptPlan } from '../../shared/types'
import type { ProviderConfigMetadata } from '../../shared/tauri/agentSupervisor'
import { hasAgentToolInvocation, parseUniversalAuthorPresentation, type AgentClarification, type AgentClarificationOption } from './agentConversationState.js'
import { AgentStepItem, type AgentProcessStep, agentProcessSteps } from './AgentStepItem.js'
import { ASSISTANT_QUICK_MODIFY_PRESETS } from './cadWorkbenchQuickModifyPresets.js'
import { LegacyCompatibilityNotice } from './LegacyCompatibilityNotice.js'
import type { LegacyCompatibilityDisplay } from './legacyCompatibilityDisplay.js'
import type { AgentBlockoutPreviewPresentation } from './agentBlockoutPreviewPresentation.js'
import type { AgentPlanSourcePresentation } from './agentPlanSourcePresentation.js'
import type { CandidatePreviewQualityPresentation } from './candidatePreviewQualityPresentation.js'
import { CandidatePreviewQualityPanel } from './candidatePreviewQuality.js'
import { providerConfigPresentation } from './providerConnectionPresentation.js'

export type { AgentClarification, AgentClarificationOption } from './agentConversationState.js'

export type AgentConversationSuggestion = readonly [label: string, prompt: string]

const MAX_VISIBLE_PROCESS_STEPS = 6
const EMPTY_PROCESS_STEPS: readonly AgentProcessStep[] = []

function buildRecentProcessSteps(
  agentKernelItems: readonly AgentItem[],
  enabled: boolean,
  agentPlan: MechanicalConceptPlan | null,
  compatibilityFallbackKey?: string,
): readonly AgentProcessStep[] {
  if (!enabled) return EMPTY_PROCESS_STEPS
  const steps = agentProcessSteps(agentKernelItems)
  if (steps.length > 0) {
    return steps.length <= MAX_VISIBLE_PROCESS_STEPS ? steps : steps.slice(-MAX_VISIBLE_PROCESS_STEPS)
  }

  // The legacy compatibility adapter can return a real plan result without
  // preserving the planner call metadata. Keep that result visible as an
  // explicitly labelled compatibility trace; it is never a V003 decision,
  // preview, asset version, or substitute geometry source.
  if (agentKernelItems.length === 0 && !compatibilityFallbackKey) return EMPTY_PROCESS_STEPS
  const planKey = agentPlan?.plan_id || agentKernelItems[agentKernelItems.length - 1]?.item_id || compatibilityFallbackKey || 'compatibility-plan'
  return [
    {
      key: `compatibility-plan-call-${planKey}`,
      itemType: 'tool_call',
      stage: '兼容规划调用',
      tool: 'plan_complete_concept',
      status: 'completed',
      inputEvidence: '输入证据：文字设计需求',
      duration: null,
      failureCode: null,
      repairCount: null,
      detail: '已记录兼容规划调用，未进入正式 3D 结果链。',
    },
    {
      key: `compatibility-plan-result-${planKey}`,
      itemType: 'tool_result',
      stage: '兼容规划结果',
      tool: 'plan_complete_concept',
      status: 'completed',
      inputEvidence: null,
      duration: null,
      failureCode: null,
      repairCount: null,
      detail: '已返回完整外观规划，等待正式结果决策。',
    },
  ]
}

function normalizeErrorMessage(message: string | null | undefined): { summary: string; details: string | null } {
  if (!message) return { summary: '', details: null }
  const lines = message.split('\n').map((item) => item.trim()).filter(Boolean)
  if (!lines.length) return { summary: message, details: null }

  const technicalMessageHint = lines[0]?.toLowerCase() ?? ''
  const userSummary = /agent pipeline failed|rust validation error|socket undefined|http 410|http 500|compatibility route|failed to fetch|未捕获异常/.test(technicalMessageHint)
    ? '模型生成没有完成，请重新尝试。'
    : lines[0] ?? message

  return {
    summary: userSummary,
    details: userSummary === lines[0] && lines.length <= 1
      ? null
      : [userSummary === lines[0] ? null : lines[0], ...lines.slice(1)]
        .filter(Boolean)
        .join('\n') || null,
  }
}

function readPlanSpecText(spec: Record<string, unknown>, keys: readonly string[]): string | null {
  for (const key of keys) {
    const value = spec[key]
    if (typeof value === 'string' && value.trim()) return value.trim()
  }
  return null
}

function planDomainLabel(domainPackId: string | undefined): string | null {
  if (!domainPackId) return null
  const labels: Record<string, string> = {
    pack_aircraft_concept: '飞行器概念',
    pack_vehicle_concept: '车辆概念',
    pack_robotic_arm_concept: '机器人概念',
    pack_future_weapon_prop: '虚构道具',
  }
  return labels[domainPackId] ?? domainPackId.replace(/^pack_/, '').replace(/_concept$/, '')
}

function buildPlanAnalysis(agentPlan: MechanicalConceptPlan | null) {
  const direction = agentPlan?.directions[0] ?? null
  const spec = agentPlan?.spec ?? {}
  const value = (text: string | null | undefined) => text?.trim() || '待分析'

  return [
    { label: '用途', value: value(readPlanSpecText(spec, ['purpose', 'use_case', 'application']) ?? planDomainLabel(agentPlan?.domain_pack_id)) },
    { label: '环境', value: value(readPlanSpecText(spec, ['environment', 'scene', 'context'])) },
    {
      label: '结构',
      value: value(direction?.primary_part_roles?.length ? direction.primary_part_roles.join(' / ') : direction?.summary),
    },
    { label: '风格', value: value(direction?.material_direction ?? direction?.silhouette) },
    { label: '模型类型', value: value(agentPlan ? (agentPlan.generation_stage === 'blockout' ? '概念预览' : agentPlan.generation_stage) : null) },
  ] as const
}

function makeGeneralLocalPrompt(selectedNode: string | null, selectedModuleLabel: string) {
  if (!selectedNode) return null
  const componentName = selectedModuleLabel || '当前组件'
  return `请仅调整“${componentName}”的外观局部细节（如切边、曲面、接缝），其余结构保持不变。`
}

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
  compatibilityDecisionRejected?: boolean
  onFocusComposer?: () => void
  blockoutPreviewPresentation: AgentBlockoutPreviewPresentation | null
  agentPlanSourcePresentation: AgentPlanSourcePresentation | null
  conceptFamilySuggestions: readonly AgentConversationSuggestion[]
  presentationProfile: 'quick_sketch' | 'showcase'
  styleOptionsOpen: boolean
  showAdvancedControls: boolean
  onAssistantModeChange: (mode: 'brief' | 'change') => void
  onSuggestionSelect: (prompt: string) => void
  onPresentationProfileChange: (profile: 'quick_sketch' | 'showcase') => void
  onClarificationSelect: (option: AgentClarificationOption) => void
  onQuickModify?: (request: string) => void | Promise<void>
  canQuickModify?: boolean
  agentClarification: AgentClarification | null
  agentKernelItems: readonly AgentItem[]
  agentKernelUnavailable: boolean
  agentPlan: MechanicalConceptPlan | null
  candidatePreviewQualityPresentation: CandidatePreviewQualityPresentation
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
  compatibilityDecisionRejected = false,
  onFocusComposer,
  blockoutPreviewPresentation,
  agentPlanSourcePresentation,
  conceptFamilySuggestions,
  presentationProfile,
  styleOptionsOpen,
  showAdvancedControls,
  onAssistantModeChange,
  onSuggestionSelect,
  onPresentationProfileChange,
  onClarificationSelect,
  onQuickModify,
  canQuickModify = true,
  agentClarification,
  agentKernelItems,
  agentKernelUnavailable,
  agentPlan,
  candidatePreviewQualityPresentation,
}: AgentConversationProps) {
  const providerPresentation = providerConfigPresentation(providerConfig)
  // Persisted tool lifecycle is user-facing progress evidence, not a
  // professional parameter. Show it whenever the Agent returned real items;
  // AgentStepItem still exposes only bounded labels and proof metadata.
  const compatibilityFallbackKey = !hasAgentToolInvocation(agentKernelItems, 'author_universal_asset')
    && (compatibilityDecisionRejected || errorMessage?.includes('没有返回正式的单一结果决策'))
    ? 'compatibility-v003-rejection'
    : undefined
  const processSteps = buildRecentProcessSteps(
    agentKernelItems,
    agentKernelItems.length > 0 || agentPlan !== null || compatibilityFallbackKey !== undefined,
    agentPlan,
    compatibilityFallbackKey,
  )
  const universalAuthor = parseUniversalAuthorPresentation(agentKernelItems)
  const { summary: errorSummary, details: errorDetails } = normalizeErrorMessage(errorMessage)
  const beginnerWelcomeText = showAdvancedControls
    ? '描述任意对象或添加参考图；我会先形成对象理解、视觉验收要求与逐部件表示计划。'
    : '描述任意对象或添加参考图；先理解，再生成可执行预览或说明当前能力限制。'
  const canRunQuickModify = Boolean(onQuickModify) && canQuickModify
  const localSelectionPrompt = makeGeneralLocalPrompt(selectedNode, selectedModuleLabel)
  const planAnalysis = buildPlanAnalysis(agentPlan)
  const progressHint = `${assistantNote} ${errorMessage ?? ''}`
  const activeGenerationStep = /预览|视图|渲染/.test(progressHint)
    ? 4
    : /装配|回读|核验|检查/.test(progressHint)
      ? 3
      : /创建|构建|组件/.test(progressHint)
        ? 2
        : /结构|方案|规划/.test(progressHint)
          ? 1
          : 0
  const generationProgressLabels = ['正在理解需求', '正在生成结构方案', '正在创建组件', '正在装配模型', '正在生成预览']
  const generationProgressPercent = [16, 38, 58, 78, 96][activeGenerationStep] ?? 16

  return (
    <div className="agent-conversation-shell">
      {loading ? (
        <section className="agent-generation-progress" aria-live="polite" aria-label="生成进度">
          <div className="agent-generation-progress-heading">
            <strong>正在生成</strong>
            <span>{generationProgressPercent}% · {activeGenerationStep + 1} / {generationProgressLabels.length}</span>
          </div>
          <div
            className="agent-generation-progress-track"
            role="progressbar"
            aria-label="模型生成进度"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={generationProgressPercent}
          >
            <span style={{ width: `${generationProgressPercent}%` }} />
          </div>
          <ol>
            {generationProgressLabels.map((label, index) => (
              <li
                key={label}
                className={index < activeGenerationStep ? 'is-complete' : index === activeGenerationStep ? 'is-active' : 'is-pending'}
              >
                <span className="agent-generation-progress-index">{index < activeGenerationStep ? '✓' : index + 1}</span>
                <span>{label}</span>
              </li>
            ))}
          </ol>
        </section>
      ) : null}
      <section className="agent-conversation-section">
        <div className="agent-conversation-section-title">你的需求</div>
        {!projectExists && !loading && (
          <div className="agent-empty-project" data-testid="agent-no-project" role="status">
            <strong>从左侧开始新设计</strong>
            <span>创建项目后即可在下方描述模型；工作台不会预先生成方向或资产。</span>
          </div>
        )}
        {projectExists && projectIsEmpty && !legacyCompatibility.isLegacyReadOnly && (
          <div className="agent-empty-project" data-testid="agent-empty-project" role="status">
            <strong>空项目已就绪</strong>
            <span>直接在下方描述你想要的模型；AI 会生成第一个 3D 资产，无需先准备旧组件。</span>
          </div>
        )}
        <p className="agent-welcome">{beginnerWelcomeText}</p>
        <LegacyCompatibilityNotice
          display={legacyCompatibility}
          onRequestLegacyAgentRebuild={onRequestLegacyAgentRebuild}
          onOpenLegacyDetails={onOpenLegacyDetails}
        />
      </section>

      <section className="agent-conversation-section">
        <div className="agent-conversation-section-title">AI分析</div>
        <div className={`agent-analysis-card ${agentPlan ? 'has-plan' : 'is-pending'}`} data-testid="agent-analysis-card">
          <div className="agent-analysis-card-heading">
            <span className="agent-analysis-spark" aria-hidden="true">✦</span>
            <strong>{agentPlan ? '已识别设计方向' : '等待需求分析'}</strong>
            <span className="agent-analysis-card-status">{agentPlan ? '已形成计划' : '待输入'}</span>
          </div>
          <div className="agent-analysis-grid">
            {planAnalysis.map((item) => (
              <div className="agent-analysis-field" key={item.label}>
                <span>{item.label}</span>
                <strong>{item.value}</strong>
              </div>
            ))}
          </div>
          {showAdvancedControls && agentPlan ? <small className="agent-analysis-source">来源：{agentPlan.provider_id}{agentPlan.model ? ` · ${agentPlan.model}` : ''}</small> : null}
        </div>
        <div className="provider-setup-entry" aria-label="DeepSeek 模型服务状态" hidden={!showAdvancedControls}>
          <span className={providerPresentation.ready ? 'connected' : ''}>
            {providerPresentation.label}
          </span>
          {showAdvancedControls ? (
            <button type="button" onClick={onToggleProviderSetup} aria-expanded={providerSetupOpen} aria-controls="forgecad-provider-setup">
              {providerSetupOpen ? '收起 DeepSeek 配置' : '配置 DeepSeek'}
            </button>
          ) : null}
        </div>
        {showAdvancedControls && providerSetupOpen && (
          <div id="forgecad-provider-setup" className="provider-setup-card" aria-label="配置 DeepSeek 模型服务">
            <strong>连接你的大模型 API</strong>
            <small>只接受 DeepSeek 官方 endpoint 与 deepseek-* 模型；API Key 仅保存到本机私密文件。</small>
            <label><span>DeepSeek Base URL</span><input value={providerBaseUrl} onChange={(event: ChangeEvent<HTMLInputElement>) => onProviderBaseUrlChange(event.target.value)} placeholder="https://api.deepseek.com" /></label>
            <label><span>DeepSeek 模型</span><input value={providerModel} onChange={(event: ChangeEvent<HTMLInputElement>) => onProviderModelChange(event.target.value)} placeholder="deepseek-v4-pro" /></label>
            <label><span>API Key</span><input type="password" value={providerApiKey} onChange={(event: ChangeEvent<HTMLInputElement>) => onProviderApiKeyChange(event.target.value)} placeholder="只在本次配置时输入" autoComplete="off" /></label>
            <div className="provider-setup-actions">
              <button type="button" onClick={onCancelProviderSetup} disabled={providerSaving}>取消</button>
              <button type="button" onClick={onTestProvider} disabled={providerSaving || !providerPresentation.canTest}>测试连接（会联网）</button>
              <button type="button" className="primary" onClick={onSaveProvider} disabled={providerSaving}>{providerSaving ? '保存并连接中…' : '保存并连接'}</button>
            </div>
            <small>本机 Alpha 不使用 macOS 钥匙串，因此不会因频繁重建反复索要系统密码。</small>
          </div>
        )}
        {activeProviderTurnId && (
          <button type="button" className="empty-action" onClick={onCancelProviderTurn}>
            取消本次模型请求
          </button>
        )}
        <div
          className={`assistant-message ${errorMessage ? 'error' : ''}`}
          role={errorMessage ? 'alert' : 'status'}
          aria-live={errorMessage ? 'assertive' : 'polite'}
        >
          {errorMessage ? (
            <>
              <span>{errorSummary || assistantNote}</span>
              {errorDetails ? (
                <details className="assistant-error-details">
                  <summary>查看技术详情</summary>
                  <pre>{errorDetails}</pre>
                </details>
              ) : null}
              {onFocusComposer ? (
                <div className="assistant-error-actions">
                  <button type="button" className="empty-action" onClick={onFocusComposer}>
                    返回输入框
                  </button>
                </div>
              ) : null}
            </>
          ) : assistantNote}
        </div>
        {universalAuthor && (
          <div className={`agent-universal-author ${universalAuthor.outcome}`} role="status" aria-live="polite" data-testid="u002-universal-author-card">
            <strong>{universalAuthor.identityLabel ?? '对象理解已完成'}</strong>
            {universalAuthor.category && <small>识别对象：{universalAuthor.category}</small>}
            {universalAuthor.keyFeatures.length > 0 && <p>关键外观：{universalAuthor.keyFeatures.join('；')}</p>}
            {universalAuthor.outcome === 'limitation' && (
              <>
                <p>{universalAuthor.message ?? '当前没有可执行的高质量表示能力，未生成替代模板。'}</p>
                {universalAuthor.suggestedViews.length > 0 && <small>建议补充视图：{universalAuthor.suggestedViews.join('、')}</small>}
              </>
            )}
            {universalAuthor.outcome === 'clarification_required' && <p>{universalAuthor.message ?? '对象身份或目标存在冲突，需要补充说明。'}</p>}
            {universalAuthor.outcome === 'executable' && <small>当前表示能力可执行，结果仍需通过编译、回读和多视图验收。</small>}
          </div>
        )}
        {blockoutPreviewPresentation && (
          <div className={`agent-blockout-status ${blockoutPreviewPresentation.tone}`} role={blockoutPreviewPresentation.tone === 'error' ? 'alert' : 'status'} aria-live={blockoutPreviewPresentation.tone === 'error' ? 'assertive' : 'polite'}>
            <strong>{blockoutPreviewPresentation.title}</strong>
            <small>{blockoutPreviewPresentation.detail}</small>
          </div>
        )}
      </section>

      {showAdvancedControls ? (
        <section className="agent-conversation-section">
          <details className="agent-advanced-panel" aria-label="专业参数">
            <summary className="agent-conversation-section-title agent-advanced-panel-summary">专业参数</summary>
            <div className="agent-advanced-panel-body">
              <div className="concept-family-suggestions" aria-label="概念家族">
                <span>从一个方向开始</span>
                <div>
                  {conceptFamilySuggestions.map(([label, prompt]) => (
                    <button key={label} type="button" onClick={() => { onAssistantModeChange('brief'); onSuggestionSelect(prompt) }}>{label}</button>
                  ))}
                </div>
              </div>

              {styleOptionsOpen ? (
                <div className="presentation-profile" aria-label="外观生成质量">
                  <span>外观生成质量</span>
                  <div>
                    <button
                      type="button"
                      aria-pressed={presentationProfile === 'quick_sketch'}
                      onClick={() => onPresentationProfileChange('quick_sketch')}
                    >
                      快速草图
                      <small>先看整体轮廓</small>
                    </button>
                    <button
                      type="button"
                      className="primary"
                      aria-pressed={presentationProfile === 'showcase'}
                      onClick={() => onPresentationProfileChange('showcase')}
                    >
                      展示模型
                      <small>增加外观分层细节</small>
                    </button>
                  </div>
                </div>
              ) : null}

              <CandidatePreviewQualityPanel
                presentation={candidatePreviewQualityPresentation}
              />

              {processSteps.length > 0 ? (
                <div
                  className="agent-kernel-events f026-agent-timeline"
                  role="log"
                  aria-live="polite"
                  aria-label="生成过程步骤"
                >
                  <div className="agent-kernel-events-title">
                    <span>可核验生成过程</span>
                    <small>{agentKernelUnavailable ? '兼容模式' : '仅展示已持久化步骤，不展示模型推理'}</small>
                  </div>
                  {processSteps.map((step) => <AgentStepItem key={step.key} step={step} />)}
                </div>
              ) : null}

              {showAdvancedControls && agentPlan && agentPlanSourcePresentation ? (
                <div
                  className={`agent-plan-source ${agentPlanSourcePresentation.tone}`}
                  role="status"
                  aria-live="polite"
                  data-testid="f026-plan-source"
                >
                  <strong>{agentPlanSourcePresentation.title}</strong>
                  <small>{agentPlanSourcePresentation.detail}</small>
                  <small>工作台只构建并展示一个当前结果；不会要求你在多个方向中选择。</small>
                </div>
              ) : null}
            </div>
          </details>
        </section>
      ) : null}

      <section className="agent-conversation-section">
        <div className="agent-conversation-section-title">当前选中组件</div>
        {selectedNode ? (
          <>
            <button
              type="button"
              className={`agent-selection-context ${assistantMode === 'change' ? 'active' : ''}`}
              onClick={() => onAssistantModeChange('change')}
            >
              正在调整：{selectedModuleLabel}
            </button>
            {localSelectionPrompt ? <small>已选中组件可单独局部调整，未选择则默认做全局优化。</small> : null}
          </>
        ) : (
          <div className="agent-empty-project">
            <strong>未选中组件</strong>
            <span>你可以先点击 3D 模型中的组件；或先做全局修改，再按部件进行局部微调。</span>
          </div>
        )}
      </section>

      <section className="agent-conversation-section">
        <div className="agent-conversation-section-title">快速修改</div>
        <div className="agent-quick-modify" aria-label="快速修改建议">
          {ASSISTANT_QUICK_MODIFY_PRESETS.map((template) => (
            <button
              key={template.label}
              type="button"
              className="agent-quick-modify-action"
              title={template.summary}
              disabled={!canRunQuickModify}
              onClick={() => { void onQuickModify?.(template.prompt) }}
            >
              <span>{template.label}</span>
              <small>{template.summary}</small>
            </button>
          ))}
          {!canRunQuickModify ? (
            <div className="agent-quick-modify-note" role="note">
              <strong>先完成 AI 生成</strong>
              <small>完成第一轮生成并进入修改模式后再使用快速修改，避免无上下文误改。</small>
            </div>
          ) : null}
          {selectedNode && localSelectionPrompt ? <small>当前有组件选中，输入需求时可描述“仅调整该组件”。</small> : null}
        </div>
      </section>

      {agentClarification ? (
        <section className="agent-conversation-section">
          <div className="agent-conversation-section-title">确认需求</div>
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
        </section>
        ) : null}


    </div>
  )
}
