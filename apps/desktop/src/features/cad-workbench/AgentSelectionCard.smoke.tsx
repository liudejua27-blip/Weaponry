import { isValidElement, type ReactElement, type ReactNode } from 'react'
import { AgentSelectionCard, type AgentSelectionCardProps } from './AgentSelectionCard.js'

const part = {
  part_id: 'part_joint',
  role: 'joint_elbow',
  parent_part_id: null,
  position_mm: [0, 0, 0],
  size_mm: [20, 20, 20],
  material_zone_ids: ['zone_joint'],
  editable_parameters: ['transform.scale.x'],
  editable_parameter_bindings: [{
    schema_version: 'EditableParameterBinding@1' as const,
    parameter_id: 'editparam_joint_length_x',
    path: 'transform.scale.x' as const,
    display_name: '长度比例',
    unit: 'ratio' as const,
    default: 1,
    min: 0.6,
    max: 1.4,
    step: 0.1,
  }],
  provenance: 'agent_generated' as const,
}

const assetVersion = {
  schema_version: 'AgentAssetVersion@1' as const,
  asset_version_id: 'asset_smoke',
  project_id: 'project_smoke',
  parent_asset_version_id: null,
  version_no: 1,
  status: 'committed' as const,
  summary: 'smoke asset',
  stage: 'editable_asset' as const,
  plan_id: 'plan_smoke',
  direction_id: 'direction_smoke',
  domain_pack_id: 'pack_robotic_arm_concept',
  artifact_id: 'artifact_smoke',
  parts: [part],
  shape_program: {},
  assembly_graph: {
    parts: [{
      part_id: part.part_id,
      transform: { position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] },
    }],
  },
  created_at: '2026-07-13T00:00:00Z',
}

const props: AgentSelectionCardProps = {
  segmentation: {
    artifact_id: 'artifact_smoke',
    plan_id: 'plan_smoke',
    direction_id: 'direction_smoke',
    variation_index: 1,
    domain_pack_id: 'pack_robotic_arm_concept',
    segmentation_status: 'candidate',
    parts: [part],
    assembly_graph: {
      parts: [{
        part_id: part.part_id,
        transform: { position: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] },
      }],
    },
  },
  agentAssetVersion: assetVersion,
  activeAgentAssetVersion: assetVersion,
  selectedPart: part,
  selectedPartId: part.part_id,
  partDisplay: {
    schema_version: 'ActiveDesignPartDisplay@1',
    project_id: 'prj_smoke',
    asset_version_id: 'assetver_smoke',
    locked_part_ids: [],
    hidden_part_ids: [],
    isolated_part_id: null,
  },
  isSelectedPartLocked: false,
  isExternalGlbReference: false,
  isSnapshotActionPending: false,
  agentAssetChangeSet: null,
  agentComponentCandidates: [{
    component: {
      component_id: 'agentcomp_smoke',
      project_id: 'project_smoke',
      domain_pack_id: 'pack_robotic_arm_concept',
      role: 'joint_elbow',
      display_name: '可复用肘关节',
      description: 'smoke',
      source_asset_version_id: 'assetver_smoke',
      source_part_id: 'part_joint',
      part_template: part,
      shape_operation: {},
      source_quality_status: 'passed',
      created_at: '2026-07-13T00:00:00Z',
      updated_at: '2026-07-13T00:00:00Z',
    },
    compatibility: {
      component_id: 'agentcomp_smoke',
      target_asset_version_id: 'assetver_smoke',
      target_part_id: 'part_joint',
      eligible: true,
      source_quality_status: 'passed',
      reason_codes: ['component_active', 'same_domain_pack', 'same_role', 'source_quality_passed', 'target_connectors_preserved'],
    },
  }],
  agentStructureSuggestions: [{
    suggestion_id: 'structure_merge_parts_smoke',
    kind: 'merge_parts',
    asset_version_id: 'assetver_smoke',
    part_id: 'part_joint',
    target_part_id: 'part_joint',
    affected_part_ids: ['part_joint'],
    source_facts: ['direct_leaf_connection'],
    summary: '将两个已连接的外观部件合并为一个可编辑部件',
  }],
  structureSuggestionUnavailableMessage: null,
  semanticProportions: {
    asset_version_id: 'asset_smoke',
    part_id: part.part_id,
    domain_pack_id: 'pack_robotic_arm_concept',
    runtime_manifest_version: 'ShapeProgramRuntimeManifest@1',
    shape_program_sha256: 'a'.repeat(64),
    glb_sha256: 'b'.repeat(64),
    locked: false,
    options: [{
      recipe_id: 'proportion_arm_sleek',
      style_token: {
        token_id: 'style_aerodynamic_sleek', version: '1', display_name: '修长流线', description: '延展主方向比例',
        proportion_profile: 'elongated', edge_language: 'controlled', surface_tension: 'taut', detail_density: 'low',
        symmetry: 'assembly_driven', material_palette: 'technical_composite', lighting_profile: 'concept_contrast',
        allowed_domains: ['pack_robotic_arm_concept'], visual_only: true, provenance: 'forgecad_builtin',
      },
      display_name: '上臂更修长', description: '延展上臂连杆的视觉跨度。', path: 'transform.scale.x',
      current_value: 1, target_value: 1.1, min: 0.6, max: 1.4, step: 0.1, unit: 'ratio', source_operation_ids: ['op_joint'],
    }],
  },
  editAssistLoading: false,
  blockoutPreviewPresentation: { tone: 'ready', title: '完整外观预览已准备好', detail: '可以保存为可编辑模型，或先换一版外观。' },
  onSelectPart: () => undefined,
  onCommitBlockout: () => undefined,
  onRegenerateBlockout: () => undefined,
  onPreviewEdit: () => undefined,
  onSaveSelectedComponent: () => undefined,
  onReplaceComponent: () => undefined,
  onPreviewStructureSuggestion: () => undefined,
  onSetPartDisplay: () => undefined,
  onInspectAsset: () => undefined,
  onRejectChange: () => undefined,
  onConfirmChange: () => undefined,
}

function collectText(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(collectText).join(' ')
  if (!isValidElement(node)) return ''
  if (typeof node.type === 'function') {
    const renderFunction = node.type as (value: unknown) => ReactNode
    return collectText(renderFunction(node.props))
  }
  return collectText((node.props as { children?: ReactNode }).children)
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

function findHostButton(node: ReactNode, label: string): ReactElement<{ onClick?: () => unknown; disabled?: boolean }> | null {
  if (node === null || node === undefined || typeof node === 'boolean' || typeof node === 'string' || typeof node === 'number') return null
  if (Array.isArray(node)) {
    for (const child of node) {
      const result = findHostButton(child, label)
      if (result) return result
    }
    return null
  }
  if (!isValidElement(node)) return null
  if (typeof node.type === 'function') {
    const renderFunction = node.type as (value: unknown) => ReactNode
    return findHostButton(renderFunction(node.props), label)
  }
  const props = node.props as { 'aria-label'?: string; children?: ReactNode }
  if (node.type === 'button' && props['aria-label'] === label) {
    return node as ReactElement<{ onClick?: () => unknown; disabled?: boolean }>
  }
  return findHostButton(props.children, label)
}

export function runAgentSelectionCardSmoke(): void {
  const text = collectText(AgentSelectionCard(props))
  assert(text.includes('分件候选'), 'selection card must render candidate heading')
  assert(text.includes('肘部关节') && !text.includes('joint_elbow'), 'selection card must render a Chinese part role without exposing its internal key')
  assert(text.includes('可调参数') && text.includes('长度比例') && text.includes('比例（ratio）'), 'selection card must render declared parameter details')
  assert(text.includes('范围') && text.includes('0.6') && text.includes('1.4') && text.includes('每次') && text.includes('0.1'), 'selection card must render declared parameter bounds and step')
  assert(!text.includes('缩短 20%') && !text.includes('放大 20%'), 'selection card must not retain hard-coded scale actions')
  assert(text.includes('关节左转 15°'), 'selection card must render joint action')
  assert(text.includes('替换： 可复用肘关节'), 'selection card must render compatible component action')
  assert(text.includes('来源检查通过') && text.includes('保留当前连接位置'), 'selection card must render a factual compatibility explanation')
  assert(text.includes('预览合并') && text.includes('只会依据当前已知的部件关系'), 'selection card must render evidence-bound structure suggestion actions')
  assert(text.includes('锁定此部件') && text.includes('隐藏此部件') && text.includes('只看这个部件'), 'selection card must expose plain-language part protection and display actions')
  assert(text.includes('外观比例配方') && text.includes('上臂更修长'), 'selection card must render a resolved semantic proportion recipe')
  assert(text.includes('不代表尺寸、结构、性能或制造结论'), 'semantic proportion UI must retain the non-engineering boundary')
  assert(text.includes('检查这个模型'), 'selection card must render quality action')
  const previewProps: AgentSelectionCardProps = {
    ...props,
    agentAssetVersion: null,
    activeAgentAssetVersion: null,
    selectedPart: undefined,
    selectedPartId: null,
  }
  const previewText = collectText(AgentSelectionCard(previewProps))
  assert(previewText.includes('完整外观预览已准备好') && previewText.includes('可以保存为可编辑模型'), 'selection card must use the shared beginner preview presentation')
  assert(previewText.includes('换一版外观') && previewText.includes('当前第') && previewText.includes('/ 3 版') && previewText.includes('不影响已保存设计'), 'selection card must offer a plain-language preview-only appearance rotation')

  const calls: Array<{ operation: unknown; summary: string }> = []
  const controlProps: AgentSelectionCardProps = {
    ...props,
    onPreviewEdit: (operation, summary) => { calls.push({ operation, summary }) },
  }
  const decrease = findHostButton(AgentSelectionCard(controlProps), '减小 长度比例')
  assert(decrease && !decrease.props.disabled && decrease.props.onClick, 'declared parameter decrement must be available')
  decrease.props.onClick()
  assert(calls.length === 1, 'declared parameter decrement must request exactly one ChangeSet preview')
  assert(JSON.stringify(calls[0].operation).includes('"path":"transform.scale.x"') && JSON.stringify(calls[0].operation).includes('"value":0.9'), 'declared parameter decrement must use the binding path and step')
  assert(calls[0].summary.includes('长度比例') && calls[0].summary.includes('0.9 比例（ratio）'), 'preview summary must identify the declared value')
  const recipe = findHostButton(AgentSelectionCard(controlProps), '上臂更修长；延展上臂连杆的视觉跨度。')
  assert(recipe && !recipe.props.disabled && recipe.props.onClick, 'resolved semantic recipe must be available')
  recipe.props.onClick()
  assert(Number(calls.length) === 2 && JSON.stringify(calls[1].operation).includes('"value":1.1'), 'semantic recipe must request one existing bounded parameter preview')

  let regenerated = 0
  const regenerate = findHostButton(AgentSelectionCard({ ...previewProps, onRegenerateBlockout: () => { regenerated += 1 } }), '换一版外观')
  assert(regenerate && !regenerate.props.disabled && regenerate.props.onClick, 'preview candidate must expose the appearance rotation action')
  regenerate.props.onClick()
  assert(regenerated === 1, 'appearance rotation must request one new preview and not commit an asset')

  const unavailableText = collectText(AgentSelectionCard({
    ...props,
    selectedPart: { ...part, editable_parameter_bindings: [] },
  }))
  assert(unavailableText.includes('暂不支持单独调整比例') && !unavailableText.includes('可调参数'), 'empty declarations must not create guessed controls')

  const locked = findHostButton(AgentSelectionCard({ ...props, isSelectedPartLocked: true }), '减小 长度比例')
  assert(locked?.props.disabled, 'locked part must disable declared parameter controls')
}
