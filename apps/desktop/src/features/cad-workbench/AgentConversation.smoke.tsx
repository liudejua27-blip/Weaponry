import { isValidElement, type ReactNode } from 'react'
import { AgentConversation, type AgentConversationProps } from './AgentConversation.js'
import { agentProcessSteps } from './AgentStepItem.js'
import { deriveCandidatePreviewQuality } from './candidatePreviewQualityLogic.js'
import { buildCandidatePreviewQualityPresentation } from './candidatePreviewQualityPresentation.js'
import { validateCandidateGeometry } from './candidatePreviewValidation.js'

const baseProps: AgentConversationProps = {
  loading: false,
  projectExists: true,
  projectIsEmpty: false,
  legacyCompatibility: { source: 'none', isLegacyReadOnly: false, showRebuildGuidance: false, rebuildActionEnabled: false },
  onRequestLegacyAgentRebuild: () => undefined,
  onOpenLegacyDetails: () => undefined,
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
  assistantNote: '等待输入',
  errorMessage: null,
  blockoutPreviewPresentation: { tone: 'ready', title: '完整外观预览已准备好', detail: '可以确认保存为可编辑模型，或继续用自然语言修改。' },
  agentPlanSourcePresentation: { tone: 'offline', title: '本机离线规划', detail: '当前方向由本机规则生成，尚未调用模型服务，不能代表真实模型质量。' },
  conceptFamilySuggestions: [['汽车', '设计一辆汽车'], ['飞机', '设计一架飞机']],
  presentationProfile: 'showcase',
  styleOptionsOpen: true,
  showAdvancedControls: true,
  onAssistantModeChange: () => undefined,
  onSuggestionSelect: () => undefined,
  onPresentationProfileChange: () => undefined,
  onClarificationSelect: () => undefined,
  agentClarification: {
    status: 'ambiguous',
    kind: 'domain',
    question: '你想从哪一类对象开始？',
    options: [{ domain_pack_id: 'pack_aircraft_concept', label: '飞机与航空器', prompt: '设计一架飞机' }],
  },
  agentKernelItems: [
    {
      item_id: 'item_smoke_plan',
      thread_id: 'thread_smoke',
      turn_id: 'turn_smoke',
      sequence: 1,
      item_type: 'plan',
      status: 'completed',
      payload: { message: '已理解整体外观目标' },
      created_at: '2026-07-13T00:00:00Z',
    },
    {
      item_id: 'item_smoke_tool_call',
      thread_id: 'thread_smoke',
      turn_id: 'turn_smoke',
      sequence: 2,
      item_type: 'tool_call',
      status: 'pending',
      payload: { call_id: 'call_smoke_evaluate', tool_name: 'evaluate_candidate', arguments: { brief: '受限测试需求' } },
      created_at: '2026-07-13T00:00:01Z',
    },
    {
      item_id: 'item_smoke_tool_result',
      thread_id: 'thread_smoke',
      turn_id: 'turn_smoke',
      sequence: 3,
      item_type: 'tool_result',
      status: 'completed',
      payload: {
        call_id: 'call_smoke_evaluate',
        tool_name: 'evaluate_candidate',
        result: {
          hard_gate_passed: false,
          visual_reference_comparison_report: {
            failure_codes: ['REFERENCE_MICRO_MISMATCH'],
            macro_similarity_bps: 9000,
            meso_similarity_bps: 7000,
            micro_similarity_bps: 3500,
          },
          visual_convergence_report: {
            schema_version: 'VisualConvergenceReport@2',
            fixed_view_count: 8,
            repair_attempt_count: 1,
            detail_coverage: { macro_bound: 1, meso_bound: 0, micro_bound: 0, critical_unresolved: 1 },
          },
        },
      },
      created_at: '2026-07-13T00:00:03Z',
    },
    {
      item_id: 'item_smoke_failure',
      thread_id: 'thread_smoke',
      turn_id: 'turn_smoke',
      sequence: 4,
      item_type: 'tool_result',
      status: 'failed',
      payload: { call_id: 'call_smoke_patch', tool_name: 'patch_forge_visual_program', error_code: 'VISUAL_REPAIR_PATCH_NOT_LOCAL' },
      created_at: '2026-07-13T00:00:04Z',
    },
    {
      item_id: 'item_smoke_live_call',
      thread_id: 'thread_smoke',
      turn_id: 'turn_smoke',
      sequence: 5,
      item_type: 'tool_call',
      status: 'pending',
      payload: { call_id: 'call_smoke_build', tool_name: 'build_candidate_geometry', arguments: { program: { schema_version: 'ForgeVisualProgram@1' } } },
      created_at: '2026-07-13T00:00:05Z',
    },
  ],
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
  candidatePreviewQualityPresentation: buildCandidatePreviewQualityPresentation(true, null),
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
  if (typeof node.type === 'object' && node.type !== null && 'type' in node.type) {
    const memoType = node.type as { type?: unknown }
    if (typeof memoType.type === 'function') {
      return collectText((memoType.type as (props: unknown) => ReactNode)(node.props))
    }
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
  if (typeof node.type === 'object' && node.type !== null && 'type' in node.type) {
    const memoType = node.type as { type?: unknown }
    if (typeof memoType.type === 'function') {
      return hasAriaLabel((memoType.type as (props: unknown) => ReactNode)(node.props), expected)
    }
  }
  const props = node.props as { 'aria-label'?: string; children?: ReactNode }
  return props['aria-label'] === expected || hasAriaLabel(props.children, expected)
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

export function runAgentConversationSmoke(): void {
  const quality = deriveCandidatePreviewQuality(baseProps.agentKernelItems)
  const output = AgentConversation({
    ...baseProps,
    candidatePreviewQualityPresentation: buildCandidatePreviewQualityPresentation(true, quality),
  })
  const text = collectText(output)
  const processSteps = agentProcessSteps(baseProps.agentKernelItems)
  assert(!text.includes('空项目已就绪'), 'an existing Agent asset must not render the empty Project state')
  assert(text.includes('先确认设计对象'), 'conversation must render clarification state')
  assert(text.includes('飞机与航空器'), 'conversation must render domain choice')
  assert(text.includes('可核验生成过程'), 'conversation must render the auditable process group')
  assert(text.includes('已理解整体外观目标'), 'conversation must render step item payload')
  assert(processSteps.some((step) => step.stage === '执行质量与参考比对' && step.tool === 'evaluate_candidate'), 'agent process projection must label the audited tool stage')
  const evaluated = processSteps.find((step) => step.tool === 'evaluate_candidate')
  assert(evaluated?.inputEvidence === '输入证据：文字设计需求' && evaluated.duration === '2.0 秒', 'process projection must include bounded input evidence and duration')
  assert(evaluated.detail === '可核验证据：8 个固定视图已记录' && evaluated.repairCount === 1, 'process projection must include bounded quality and repair evidence')
  assert(processSteps.some((step) => step.failureCode === 'VISUAL_REPAIR_PATCH_NOT_LOCAL'), 'process projection must include the stable failure reason code')
  assert(processSteps.some((step) => step.tool === 'build_candidate_geometry' && step.status === 'pending'), 'process projection must retain the live bounded tool action')
  assert(text.includes('不展示模型推理'), 'conversation must state that private reasoning is excluded')
  assert(quality?.warnings.some((warning) => warning.includes('参考相似度不足：表面细节')), 'reference similarity deficit must remain a visible warning')
  assert(quality?.warnings.some((warning) => warning.includes('细节覆盖不足')), 'incomplete detail coverage must remain a visible warning')
  assert(text.includes('当前唯一候选 · 同一 3D 视口') && text.includes('继续修复'), 'candidate quality panel must keep a loadable candidate visible while warning')
  assert(quality?.stages.map((stage) => stage.label).join('、') === '轮廓、结构、形体、材质、表面、灯光、检查', 'quality panel must expose the seven bounded visual stages')
  const validGeometry = validateCandidateGeometry([{
    position: { values: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]), count: 3 },
    index: { values: new Uint16Array([0, 1, 2]), count: 3 },
  }])
  assert(validGeometry.ok && validGeometry.triangleCount === 1, 'a real triangle candidate must remain previewable')
  const nonFiniteGeometry = validateCandidateGeometry([{
    position: { values: new Float32Array([0, 0, 0, Number.NaN, 0, 0, 0, 1, 0]), count: 3 },
    index: { values: new Uint16Array([0, 1, 2]), count: 3 },
  }])
  assert(!nonFiniteGeometry.ok && nonFiniteGeometry.reason === 'non_finite_coordinate', 'non-finite coordinates must be a structural preview failure')
  assert(!text.includes('Agent 完整外观方向') && !text.includes('紧凑救援机'), 'F026 conversation must not render direction choices')
  assert(text.includes('只构建并展示一个当前结果'), 'F026 conversation must describe its one-result presentation boundary')
  assert(!text.includes('variant_family_index') && !text.includes('detail_density'), 'direction cards must not expose visual mapping internals')
  assert(text.includes('完整外观预览已准备好') && text.includes('确认保存为可编辑模型'), 'conversation must render the shared preview presentation')
  assert(text.includes('本机离线规划') && text.includes('不能代表真实模型质量'), 'conversation must describe the actual plan source without provider internals')
  assert(text.includes('外观生成质量') && text.includes('快速草图') && text.includes('展示模型'), 'conversation must present the two beginner-facing visual quality choices')
  assert(hasAriaLabel(output, '外观生成质量'), 'conversation must expose an accessible visual quality control')
  const configuredText = collectText(AgentConversation({
    ...baseProps,
    providerConfig: {
      base_url: 'https://api.example.test',
      model: 'private-provider-model-id',
      configured: true,
      storage: 'private_secret_file',
      metadata_status: 'valid',
      secret_status: 'available',
      supervisor_status: 'running',
      capability_status: 'ready',
      failure_code: null,
    },
  }))
  assert(configuredText.includes('DeepSeek 已配置') && !configuredText.includes('private-provider-model-id'), 'provider status must identify DeepSeek without exposing the configured model identifier')
  assert(!hasAriaLabel(output, '设计需求'), 'the fixed F026 composer, not the scrollable timeline, must own the input')
  const emptyProjectOutput = AgentConversation({
    ...baseProps,
    projectIsEmpty: true,
    agentPlan: null,
  })
  const emptyProjectText = collectText(emptyProjectOutput)
  assert(
    emptyProjectText.includes('空项目已就绪') && emptyProjectText.includes('生成第一个 3D 资产'),
    'an existing empty project must direct the user to generate the first Agent asset',
  )
  assert(!emptyProjectText.includes('准备展示组件'), 'empty project must not expose the legacy workbench initializer')
  const noProjectText = collectText(AgentConversation({ ...baseProps, projectExists: false, projectIsEmpty: false, agentPlan: null }))
  assert(noProjectText.includes('从左侧开始新设计') && !noProjectText.includes('创建第一个设计'), 'F026 timeline must defer project creation to the left rail')
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
