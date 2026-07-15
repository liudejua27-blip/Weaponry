import { isValidElement, type ReactNode } from 'react'
import { AgentConversation, type AgentConversationProps } from './AgentConversation.js'

const baseProps: AgentConversationProps = {
  loading: false,
  projectExists: true,
  projectNeedsInitialization: false,
  legacyCompatibility: { source: 'none', isLegacyReadOnly: false, showRebuildGuidance: false, rebuildActionEnabled: false },
  onCreateStarterProject: () => undefined,
  onInitializeCurrentProject: () => undefined,
  onRequestLegacyAgentRebuild: () => undefined,
  providerConfig: null,
  providerSetupOpen: false,
  providerBaseUrl: 'https://api.example.test',
  providerModel: 'test-model',
  providerApiKey: '',
  providerSaving: false,
  onToggleProviderSetup: () => undefined,
  onProviderBaseUrlChange: () => undefined,
  onProviderModelChange: () => undefined,
  onProviderApiKeyChange: () => undefined,
  onCancelProviderSetup: () => undefined,
  onTestProvider: () => undefined,
  onSaveProvider: () => undefined,
  activeProviderTurnId: null,
  onCancelProviderTurn: () => undefined,
  assistantMode: 'brief',
  selectedNode: null,
  selectedModuleLabel: '',
  chatInput: '',
  assistantNote: '等待输入',
  errorMessage: null,
  blockoutPreviewPresentation: { tone: 'ready', title: '完整外观预览已准备好', detail: '可以保存为可编辑模型，或先换一版外观。' },
  agentPlanSourcePresentation: { tone: 'offline', title: '本机离线规划', detail: '当前方向由本机规则生成，尚未调用模型服务，不能代表真实模型质量。' },
  directionConceptPreviews: {
    direction_smoke: { status: 'ready', imageDataUrl: 'data:image/png;base64,cG5n' },
  },
  conceptFamilySuggestions: [['汽车', '设计一辆汽车'], ['飞机', '设计一架飞机']],
  presentationProfile: 'showcase',
  onAssistantModeChange: () => undefined,
  onChatInputChange: () => undefined,
  onRunAssistantAction: () => undefined,
  onSuggestionSelect: () => undefined,
  onPresentationProfileChange: () => undefined,
  onClarificationSelect: () => undefined,
  agentClarification: {
    status: 'ambiguous',
    kind: 'domain',
    question: '你想从哪一类对象开始？',
    options: [{ domain_pack_id: 'pack_aircraft_concept', label: '飞机与航空器', prompt: '设计一架飞机' }],
  },
  agentKernelItems: [{
    item_id: 'item_smoke_plan',
    thread_id: 'thread_smoke',
    turn_id: 'turn_smoke',
    sequence: 1,
    item_type: 'plan',
    status: 'completed',
    payload: { message: '已理解整体外观目标' },
    created_at: '2026-07-13T00:00:00Z',
  }],
  agentKernelUnavailable: false,
  agentPlan: {
    schema_version: 'MechanicalConceptPlan@1',
    plan_id: 'plan_smoke',
    domain_pack_id: 'pack_aircraft_concept',
    brief: '展示型飞机',
    generation_stage: 'blockout',
    spec: {
      visual_intent_mapping: {
        schema_version: 'VisualIntentMapping@1',
        directions: [{ variant_family_index: 2, detail_density: 'dense' }],
      },
    },
    directions: [{
      direction_id: 'direction_smoke',
      title: '紧凑救援机',
      summary: '完整外观与清晰分件',
      silhouette: 'balanced',
      primary_part_roles: ['机身'],
      material_direction: '哑光复合材料',
    }],
    provider_id: 'deterministic_rules',
  },
  onPreviewDirection: () => undefined,
}

function collectText(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(collectText).join(' ')
  if (!isValidElement(node)) return ''
  if (typeof node.type === 'function') {
    const renderFunction = node.type as (props: unknown) => ReactNode
    return collectText(renderFunction(node.props))
  }
  return collectText((node.props as { children?: ReactNode }).children)
}

function hasAriaLabel(node: ReactNode, expected: string): boolean {
  if (node === null || node === undefined || typeof node === 'boolean') return false
  if (Array.isArray(node)) return node.some((child) => hasAriaLabel(child, expected))
  if (!isValidElement(node)) return false
  if (typeof node.type === 'function') {
    const renderFunction = node.type as (props: unknown) => ReactNode
    return hasAriaLabel(renderFunction(node.props), expected)
  }
  const props = node.props as { 'aria-label'?: string; children?: ReactNode }
  return props['aria-label'] === expected || hasAriaLabel(props.children, expected)
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

export function runAgentConversationSmoke(): void {
  const output = AgentConversation(baseProps)
  const text = collectText(output)
  assert(text.includes('先确认设计对象'), 'conversation must render clarification state')
  assert(text.includes('飞机与航空器'), 'conversation must render domain choice')
  assert(text.includes('Agent 步骤'), 'conversation must render kernel step group')
  assert(text.includes('已理解整体外观目标'), 'conversation must render step item payload')
  assert(text.includes('Agent 完整外观方向'), 'conversation must render direction group')
  assert(text.includes('紧凑救援机'), 'conversation must render direction card')
  assert(text.includes('软件概念图已准备好'), 'direction cards must describe the available image as a software concept preview')
  assert(!text.includes('variant_family_index') && !text.includes('detail_density'), 'direction cards must not expose visual mapping internals')
  assert(text.includes('完整外观预览已准备好') && text.includes('可以保存为可编辑模型'), 'conversation must render the shared preview presentation')
  assert(text.includes('本机离线规划') && text.includes('不能代表真实模型质量'), 'conversation must describe the actual plan source without provider internals')
  assert(text.includes('外观生成质量') && text.includes('快速草图') && text.includes('展示模型'), 'conversation must present the two beginner-facing visual quality choices')
  assert(hasAriaLabel(output, '外观生成质量'), 'conversation must expose an accessible visual quality control')
  const configuredText = collectText(AgentConversation({
    ...baseProps,
    providerConfig: {
      base_url: 'https://api.example.test',
      model: 'private-provider-model-id',
      configured: true,
      storage: 'keychain',
      metadata_status: 'valid',
      secret_status: 'available',
      supervisor_status: 'running',
      capability_status: 'ready',
      failure_code: null,
    },
  }))
  assert(configuredText.includes('模型服务已配置') && !configuredText.includes('private-provider-model-id'), 'provider status must not expose the configured model identifier')
  assert(hasAriaLabel(output, '设计需求'), 'conversation must expose an accessible input label')
  const scopeOutput = AgentConversation({
    ...baseProps,
    agentClarification: {
      status: 'unsupported',
      kind: 'scope',
      question: '这个请求涉及现实制造、安全、控制或性能内容。',
      options: [],
    },
    agentPlan: null,
  })
  const scopeText = collectText(scopeOutput)
  assert(scopeText.includes('请换一种外观创意描述') && scopeText.includes('当前请求未发送给模型'), 'scope stop must state the safe local boundary')
  assert(hasAriaLabel(scopeOutput, '当前请求超出概念范围'), 'scope stop must expose an accessible boundary label')
}
