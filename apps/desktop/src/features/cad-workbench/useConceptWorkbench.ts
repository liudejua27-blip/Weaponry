import { useCallback, useEffect, useState } from 'react'
import { forgeApi } from '../../shared/api/forgeApi'
import type {
  ConceptExportRecord,
  ConceptProjectDetail,
  ConceptProjectSummary,
  ConceptVersionDetail,
  ChangeSetPreviewResponse,
  ChangeSetAuditExportRecord,
  ChangeSetTimelineItem,
  DesignChangeSet,
  DesignBriefRecord,
  DesignVariantRecord,
  ModuleAssetRecord,
  ModuleGraphRecord,
  Transform,
  PlannedChangeSetRecord,
  QualityRunRecord,
} from '../../shared/types'

const ACTIVE_PROJECT_KEY = 'forgecad.activeConceptProjectId'

export type ChangeSetTimelineFilters = {
  query: string
  status: '' | 'proposed' | 'previewed' | 'confirmed' | 'rejected' | 'stale'
  operation: '' | 'add_module' | 'remove_module' | 'replace_module' | 'connect' | 'disconnect'
    | 'set_transform' | 'set_mirror' | 'set_style' | 'set_parameter'
}

const EMPTY_TIMELINE_FILTERS: ChangeSetTimelineFilters = {
  query: '',
  status: '',
  operation: '',
}

type ConceptWorkbenchState = {
  projects: ConceptProjectSummary[]
  project: ConceptProjectDetail | null
  version: ConceptVersionDetail | null
  graphRecord: ModuleGraphRecord | null
  modules: ModuleAssetRecord[]
  variants: DesignVariantRecord[]
  brief: DesignBriefRecord | null
  pendingChange: PlannedChangeSetRecord | null
  pendingReplacement: DesignChangeSet | null
  pendingManualChange: DesignChangeSet | null
  pendingPreview: ChangeSetPreviewResponse | null
  loading: boolean
  error: string | null
  statusMessage: string
  lastExport: ConceptExportRecord | null
  lastAuditExport: ChangeSetAuditExportRecord | null
  timeline: ChangeSetTimelineItem[]
  timelineNextCursor: string | null
  timelineLoading: boolean
  timelineFilters: ChangeSetTimelineFilters
  qualityRun: QualityRunRecord | null
}

const INITIAL_STATE: ConceptWorkbenchState = {
  projects: [],
  project: null,
  version: null,
  graphRecord: null,
  modules: [],
  variants: [],
  brief: null,
  pendingChange: null,
  pendingReplacement: null,
  pendingManualChange: null,
  pendingPreview: null,
  loading: true,
  error: null,
  statusMessage: '正在读取本地 Concept 数据…',
  lastExport: null,
  lastAuditExport: null,
  timeline: [],
  timelineNextCursor: null,
  timelineLoading: false,
  timelineFilters: EMPTY_TIMELINE_FILTERS,
  qualityRun: null,
}

export function useConceptWorkbench() {
  const [state, setState] = useState<ConceptWorkbenchState>(INITIAL_STATE)

  const loadProject = useCallback(async (
    projectId: string,
    preferredVersionId?: string,
    knownProjects?: ConceptProjectSummary[],
  ) => {
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在加载 Project、Version 与 ModuleGraph…',
    }))
    try {
      const project = await forgeApi.getConceptProject(projectId)
      const versions = project.versions ?? []
      const versionId = preferredVersionId
        && versions.some((item) => item.version_id === preferredVersionId)
        ? preferredVersionId
        : project.current_version_id ?? versions.at(-1)?.version_id
      const [
        moduleResponse,
        variantResponse,
        timelineResponse,
        auditExportResponse,
        version,
      ] = await Promise.all([
        forgeApi.listModuleAssets(project.profile.pack_id),
        forgeApi.listDesignVariants(projectId),
        forgeApi.listChangeSets(projectId, { limit: 20 }),
        forgeApi.listChangeSetAuditExports(projectId, 1),
        versionId ? forgeApi.getConceptVersion(versionId) : Promise.resolve(null),
      ])
      const graphRecord = version?.module_graph_id
        ? await forgeApi.getModuleGraph(version.module_graph_id)
        : null
      localStorage.setItem(ACTIVE_PROJECT_KEY, projectId)
      setState((current) => ({
        ...current,
        projects: knownProjects ?? current.projects,
        project,
        version,
        graphRecord,
        modules: moduleResponse.items ?? [],
        variants: variantResponse.items ?? [],
        brief: null,
        pendingChange: null,
        pendingReplacement: null,
        pendingManualChange: null,
        pendingPreview: null,
        lastAuditExport: auditExportResponse.items?.[0] ?? null,
        timeline: timelineResponse.items ?? [],
        timelineNextCursor: timelineResponse.next_cursor ?? null,
        timelineLoading: false,
        timelineFilters: EMPTY_TIMELINE_FILTERS,
        qualityRun: null,
        loading: false,
        error: null,
        statusMessage: graphRecord
          ? `已加载 ${graphRecord.graph.nodes.length} 个 ModuleGraph 节点。`
          : '当前版本尚未绑定 ModuleGraph；可先注册模块并创建组合。',
      }))
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: 'Concept 数据加载失败。',
      }))
    }
  }, [])

  const refresh = useCallback(async () => {
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在同步本地 Concept 项目…',
    }))
    try {
      const response = await forgeApi.listConceptProjects()
      const projects = response.items ?? []
      if (projects.length === 0) {
        setState((current) => ({
          ...current,
          projects,
          project: null,
          version: null,
          graphRecord: null,
          modules: [],
          variants: [],
          brief: null,
          pendingChange: null,
          pendingReplacement: null,
          pendingManualChange: null,
          pendingPreview: null,
          lastAuditExport: null,
          timeline: [],
          timelineNextCursor: null,
          timelineLoading: false,
          timelineFilters: EMPTY_TIMELINE_FILTERS,
          qualityRun: null,
          loading: false,
          error: null,
          statusMessage: '尚无 Concept Project。创建“寒地巡逻 S1”开始设计。',
        }))
        return
      }
      const storedId = localStorage.getItem(ACTIVE_PROJECT_KEY)
      const activeId = projects.some((project) => project.project_id === storedId)
        ? storedId!
        : projects[0].project_id
      await loadProject(activeId, undefined, projects)
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '无法连接本地 Concept API。',
      }))
    }
  }, [loadProject])

  useEffect(() => {
    refresh().catch(() => undefined)
  }, [refresh])

  const selectProject = useCallback((projectId: string) => {
    loadProject(projectId).catch(() => undefined)
  }, [loadProject])

  const selectVersion = useCallback(async (versionId: string) => {
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: `正在读取版本 ${versionId}…`,
    }))
    try {
      const version = await forgeApi.getConceptVersion(versionId)
      const graphRecord = version.module_graph_id
        ? await forgeApi.getModuleGraph(version.module_graph_id)
        : null
      setState((current) => ({
        ...current,
        version,
        graphRecord,
        qualityRun: null,
        brief: null,
        pendingChange: null,
        pendingReplacement: null,
        pendingManualChange: null,
        pendingPreview: null,
        loading: false,
        statusMessage: graphRecord
          ? `已切换到 V${version.version_no} · ${graphRecord.graph.nodes.length} 个节点。`
          : `已切换到 V${version.version_no}；该版本没有 ModuleGraph。`,
      }))
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '版本切换失败。',
      }))
    }
  }, [])

  const createStarterProject = useCallback(async () => {
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在创建“寒地巡逻 S1”…',
    }))
    try {
      const created = await forgeApi.createConceptProject({
        client_request_id: `desktop-concept-${Date.now()}`,
        name: '寒地巡逻 S1',
        intended_uses: ['game_asset', 'film_prop', 'non_functional_display'],
        style: {
          keywords: ['寒地', '工业', '紧凑', '硬表面'],
          palette: ['graphite', 'gunmetal', 'signal_red'],
          detail_density: 0.68,
        },
        proportions: {
          overall_length_mm: 230,
          body_height_mm: 54,
          grip_angle_deg: 15,
        },
        constraints: {
          symmetry: 'mostly_symmetric',
          max_triangle_count: 180000,
        },
        assumptions: ['非功能性概念模型，不用于真实制造或使用'],
      })
      const initialized = await forgeApi.initializeConceptWorkbench(
        created.project_id,
        `desktop-initialize-workbench-${Date.now().toString(36)}`,
      )
      const projects = (await forgeApi.listConceptProjects()).items ?? []
      await loadProject(
        initialized.project_id,
        initialized.current_version_id ?? undefined,
        projects,
      )
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: 'Starter Project 创建失败。',
      }))
    }
  }, [loadProject])

  const initializeCurrentProject = useCallback(async () => {
    const project = state.project
    if (!project) return null
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在安装内置 Module Pack 并创建首个 ModuleGraph…',
    }))
    try {
      const initialized = await forgeApi.initializeConceptWorkbench(
        project.project_id,
        `desktop-initialize-workbench-${Date.now().toString(36)}`,
      )
      const projects = (await forgeApi.listConceptProjects()).items ?? []
      await loadProject(
        initialized.project_id,
        initialized.current_version_id ?? undefined,
        projects,
      )
      return initialized
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '内置 Module Pack 或初始 ModuleGraph 创建失败。',
      }))
      return null
    }
  }, [loadProject, state.project])

  const createExport = useCallback(async () => {
    if (!state.version?.module_graph_id) {
      setState((current) => ({
        ...current,
        error: '当前版本没有已验证 ModuleGraph，不能创建概念交付包。',
      }))
      return null
    }
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在创建可追溯概念交付包…',
    }))
    try {
      const result = await forgeApi.createConceptExport(state.version.version_id, {
        client_request_id: `desktop-export-${Date.now()}`,
        profile: 'game_asset',
        include_modules: true,
        include_combined_glb: true,
        include_combined_obj: true,
        include_render_png: true,
        include_turntable_video: true,
        include_quality_report: true,
      })
      setState((current) => ({
        ...current,
        loading: false,
        lastExport: result,
        statusMessage: `导出完成 · ${result.package_byte_size} bytes · ${result.package_sha256.slice(0, 12)}…`,
      }))
      return result
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '概念交付包创建失败。',
      }))
      return null
    }
  }, [state.version])

  const planBrief = useCallback(async (sourceText: string) => {
    let project = state.project
    if (!project) {
      setState((current) => ({
        ...current,
        error: '请先创建或选择一个 Concept Project。',
      }))
      return null
    }
    if (!state.version?.module_graph_id) {
      const initialized = await initializeCurrentProject()
      if (!initialized) return null
      project = initialized
    }
    const suffix = Date.now().toString(36)
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在解释 Brief 并生成 A/B/C 模块方案…',
    }))
    try {
      const brief = await forgeApi.interpretDesignBrief(project.project_id, {
        client_request_id: `desktop-brief-${suffix}`,
        source_text: sourceText,
        reference_asset_ids: [],
        generator: 'auto',
      })
      const generated = await forgeApi.generateDesignVariants(project.project_id, {
        client_request_id: `desktop-variants-${suffix}`,
        brief_id: brief.brief_id,
        count: 3,
        generator: 'auto',
      })
      const provenance = brief.planner_provenance
      setState((current) => ({
        ...current,
        loading: false,
        brief,
        variants: generated.items ?? [],
        statusMessage: `Planner 完成 · ${provenance.generator} · ${generated.items?.length ?? 0} 个方案${
          provenance.fallback_used ? ' · 已降级' : ''
        }。`,
      }))
      return { brief, variants: generated.items ?? [] }
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: 'Brief/Module Planner 执行失败。',
      }))
      return null
    }
  }, [initializeCurrentProject, state.project, state.version])

  const selectVariant = useCallback(async (variantId: string) => {
    const project = state.project
    const variant = state.variants.find((item) => item.variant_id === variantId)
    if (!project || !variant || !state.graphRecord) return null
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: `正在选择方案 ${variant.rank}…`,
    }))
    try {
      const selected = await forgeApi.selectDesignVariant(project.project_id, variantId, {
        client_request_id: `desktop-select-variant-${Date.now().toString(36)}`,
      })
      setState((current) => ({
        ...current,
        loading: false,
        variants: current.variants.map((item) => ({
          ...item,
          status: item.variant_id === selected.variant_id ? 'selected' : 'rejected',
        })),
        graphRecord: current.graphRecord
          ? {
              ...current.graphRecord,
              graph: selected.module_graph,
              graph_sha256: `planner-preview:${selected.variant_id}`,
            }
          : null,
        qualityRun: null,
        statusMessage: `已选择 ${selected.name}；当前为 Planner 预览，尚未创建子版本。`,
      }))
      return selected
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '方案选择失败。',
      }))
      return null
    }
  }, [state.graphRecord, state.project, state.variants])

  const planChange = useCallback(async (
    instruction: string,
    context: { selectedNodeId?: string; selectedModuleId?: string } = {},
  ) => {
    const project = state.project
    const version = state.version
    if (!project || !version?.module_graph_id) {
      setState((current) => ({
        ...current,
        error: '必须先加载带有效 ModuleGraph 的 Concept Version。',
      }))
      return null
    }
    const suffix = Date.now().toString(36)
    const clientRequestId = `desktop-change-plan-${suffix}`
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在规划受限 DesignChangeSet 并执行 ghost preview…',
    }))
    try {
      const planned = await forgeApi.planChangeSet(version.version_id, {
        client_request_id: clientRequestId,
        instruction,
        generator: 'auto',
        selected_node_id: context.selectedNodeId || null,
        selected_module_id: context.selectedModuleId || null,
      })
      const preview = await forgeApi.previewChangeSet(
        planned.change_set.change_set_id,
        `${clientRequestId}-preview`,
      )
      setState((current) => ({
        ...current,
        loading: false,
        pendingChange: planned,
        pendingReplacement: null,
        pendingManualChange: null,
        pendingPreview: preview,
        graphRecord: current.graphRecord
          ? {
              ...current.graphRecord,
              graph: preview.preview_graph,
              graph_sha256: `ghost-preview:${preview.preview_sha256}`,
            }
          : null,
        qualityRun: null,
        statusMessage: `幽灵预览就绪 · ${planned.planner_provenance.generator} · ${planned.change_set.operations.length} 个操作；等待确认。`,
      }))
      return { planned, preview }
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        pendingChange: null,
        pendingReplacement: null,
        pendingManualChange: null,
        pendingPreview: null,
        error: errorMessage(caught),
        statusMessage: '自然语言 Change Planner 或 ghost preview 失败。',
      }))
      return null
    }
  }, [state.project, state.version])

  const confirmPlannedChange = useCallback(async () => {
    const project = state.project
    const pending = state.pendingChange
    if (!project || !pending) return null
    const suffix = Date.now().toString(36)
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在确认 ghost preview 并创建不可变子版本…',
    }))
    try {
      const confirmed = await forgeApi.confirmChangeSet(
        pending.change_set.change_set_id,
        `desktop-change-confirm-${suffix}`,
      )
      const nextVersionId = confirmed.project.current_version_id ?? undefined
      await loadProject(project.project_id, nextVersionId)
      setState((current) => ({
        ...current,
        statusMessage: `AI 修改已确认并创建新版本：${nextVersionId ?? 'unknown'}。`,
      }))
      return confirmed
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: 'ChangeSet 确认失败；当前版本未被覆盖。',
      }))
      return null
    }
  }, [loadProject, state.pendingChange, state.project])

  const discardPlannedChange = useCallback(async () => {
    const project = state.project
    const version = state.version
    const pending = state.pendingChange
    if (!project || !version || !pending) return null
    const suffix = Date.now().toString(36)
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在放弃 ghost preview…',
    }))
    try {
      const rejected = await forgeApi.rejectChangeSet(
        pending.change_set.change_set_id,
        `desktop-change-reject-${suffix}`,
      )
      await loadProject(project.project_id, version.version_id)
      setState((current) => ({
        ...current,
        statusMessage: '已放弃 AI 修改预览；当前版本保持不变。',
      }))
      return rejected
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '放弃 ChangeSet 预览失败。',
      }))
      return null
    }
  }, [loadProject, state.pendingChange, state.project, state.version])

  const runQualityInspection = useCallback(async () => {
    const version = state.version
    if (!version?.module_graph_id) {
      setState((current) => ({
        ...current,
        error: '当前版本没有已验证 ModuleGraph，不能运行几何检查。',
      }))
      return null
    }
    const clientRequestId = `desktop-quality-${Date.now()}`
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在读取不可变 GLB 并执行 Mesh/Assembly 检查…',
    }))
    try {
      const result = await forgeApi.inspectConceptVersion(version.version_id, {
        client_request_id: clientRequestId,
        ruleset_version: 'weapon-concept-geometry/1.3',
      })
      const findingCount = result.report.findings?.length ?? 0
      setState((current) => ({
        ...current,
        loading: false,
        qualityRun: result,
        lastExport: null,
        statusMessage: `检查完成 · ${result.report.status} · ${findingCount} 项结果。`,
      }))
      return result
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: 'Mesh/Assembly 检查失败。',
      }))
      return null
    }
  }, [state.version])

  const previewModuleReplacement = useCallback(async (nodeId: string, moduleId: string) => {
    const project = state.project
    const version = state.version
    const graph = state.graphRecord?.graph
    if (!project || !version || !graph) {
      setState((current) => ({ ...current, error: '必须先加载 Project、Version 与 ModuleGraph。' }))
      return null
    }
    const node = graph.nodes.find((item) => item.node_id === nodeId)
    if (!node) {
      setState((current) => ({ ...current, error: `Graph 节点不存在：${nodeId}` }))
      return null
    }
    if (node.locked) {
      setState((current) => ({ ...current, error: `节点 ${nodeId} 已锁定，不能替换。` }))
      return null
    }
    if (node.module_id === moduleId) {
      setState((current) => ({ ...current, error: '候选模块与当前节点模块相同。' }))
      return null
    }

    const suffix = Date.now().toString(36)
    const clientRequestId = `desktop-replace-${suffix}`
    const changeSetId = `change_desktop_replace_${suffix}`
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: `正在预览 ${node.module_id} → ${moduleId}…`,
    }))
    try {
      const proposed = await forgeApi.proposeChangeSet(version.version_id, {
        client_request_id: clientRequestId,
        change_set: {
          schema_version: 'DesignChangeSet@1',
          change_set_id: changeSetId,
          project_id: project.project_id,
          base_version_id: version.version_id,
          summary: `Replace ${node.module_id} with ${moduleId} on ${nodeId}.`,
          operations: [{
            operation_id: `op_replace_${suffix}`,
            op: 'replace_module',
            node_id: nodeId,
            module_id: moduleId,
          }],
          protected_node_ids: graph.nodes.filter((item) => item.locked).map((item) => item.node_id),
          status: 'proposed',
        },
      })
      const preview = await forgeApi.previewChangeSet(proposed.change_set_id, `${clientRequestId}-preview`)
      setState((current) => ({
        ...current,
        loading: false,
        pendingChange: null,
        pendingReplacement: proposed,
        pendingManualChange: null,
        pendingPreview: preview,
        graphRecord: current.graphRecord
          ? {
              ...current.graphRecord,
              graph: preview.preview_graph,
              graph_sha256: `ghost-preview:${preview.preview_sha256}`,
            }
          : null,
        statusMessage: `替换预览就绪：${node.module_id} → ${moduleId}；确认后才创建子版本。`,
      }))
      return preview
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        pendingReplacement: null,
        pendingPreview: null,
        error: errorMessage(caught),
        statusMessage: '模块替换预览失败。',
      }))
      return null
    }
  }, [loadProject, state.graphRecord, state.project, state.version])

  const confirmModuleReplacement = useCallback(async () => {
    const project = state.project
    const replacement = state.pendingReplacement
    if (!project || !replacement) return null
    const suffix = Date.now().toString(36)
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在确认模块替换并创建不可变子版本…',
    }))
    try {
      const confirmed = await forgeApi.confirmChangeSet(replacement.change_set_id, `desktop-replace-confirm-${suffix}`)
      const nextVersionId = confirmed.project.current_version_id ?? undefined
      await loadProject(project.project_id, nextVersionId)
      setState((current) => ({
        ...current,
        pendingReplacement: null,
        pendingPreview: null,
        statusMessage: `替换已确认并创建新版本：${nextVersionId ?? 'unknown'}。`,
      }))
      return confirmed
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '模块替换确认失败；当前版本未被覆盖。',
      }))
      return null
    }
  }, [loadProject, state.pendingReplacement, state.project])

  const discardModuleReplacement = useCallback(async () => {
    const project = state.project
    const version = state.version
    const replacement = state.pendingReplacement
    if (!project || !version || !replacement) return null
    const suffix = Date.now().toString(36)
    try {
      await forgeApi.rejectChangeSet(replacement.change_set_id, `desktop-replace-reject-${suffix}`)
      await loadProject(project.project_id, version.version_id)
      setState((current) => ({
        ...current,
        pendingReplacement: null,
        pendingPreview: null,
        statusMessage: '已放弃模块替换预览；当前版本保持不变。',
      }))
      return true
    } catch (caught) {
      setState((current) => ({
        ...current,
        error: errorMessage(caught),
        statusMessage: '放弃模块替换预览失败。',
      }))
      return null
    }
  }, [loadProject, state.pendingReplacement, state.project, state.version])

  const previewNodeTransform = useCallback(async (nodeId: string, transform: Transform) => {
    const project = state.project
    const version = state.version
    const graph = state.graphRecord?.graph
    if (!project || !version || !graph) {
      setState((current) => ({ ...current, error: '必须先加载 Project、Version 与 ModuleGraph。' }))
      return null
    }
    const node = graph.nodes.find((item) => item.node_id === nodeId)
    if (!node) {
      setState((current) => ({ ...current, error: `Graph 节点不存在：${nodeId}` }))
      return null
    }
    if (node.locked || node.node_id === graph.root_node_id) {
      setState((current) => ({ ...current, error: `节点 ${nodeId} 受保护，不能变换。` }))
      return null
    }
    if (!isValidTransform(transform)) {
      setState((current) => ({ ...current, error: '位置和旋转必须是有限数值，缩放必须大于 0。' }))
      return null
    }
    if (sameTransform(node.transform, transform)) {
      setState((current) => ({ ...current, statusMessage: '变换没有变化；未创建 ChangeSet。' }))
      return null
    }

    const suffix = Date.now().toString(36)
    const clientRequestId = `desktop-transform-${suffix}`
    const changeSetId = `change_desktop_transform_${suffix}`
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: `正在预览 ${nodeId} 的变换…`,
    }))
    try {
      const proposed = await forgeApi.proposeChangeSet(version.version_id, {
        client_request_id: clientRequestId,
        change_set: {
          schema_version: 'DesignChangeSet@1',
          change_set_id: changeSetId,
          project_id: project.project_id,
          base_version_id: version.version_id,
          summary: `Transform ${nodeId}.`,
          operations: [{
            operation_id: `op_transform_${suffix}`,
            op: 'set_transform',
            node_id: nodeId,
            transform,
          }],
          protected_node_ids: graph.nodes
            .filter((item) => item.locked || item.node_id === graph.root_node_id)
            .map((item) => item.node_id),
          status: 'proposed',
        },
      })
      const preview = await forgeApi.previewChangeSet(proposed.change_set_id, `${clientRequestId}-preview`)
      setState((current) => ({
        ...current,
        loading: false,
        pendingChange: null,
        pendingReplacement: null,
        pendingManualChange: proposed,
        pendingPreview: preview,
        graphRecord: current.graphRecord
          ? {
              ...current.graphRecord,
              graph: preview.preview_graph,
              graph_sha256: `ghost-preview:${preview.preview_sha256}`,
            }
          : null,
        qualityRun: null,
        statusMessage: `变换幽灵预览就绪：${nodeId}；确认后才创建子版本。`,
      }))
      return preview
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        pendingManualChange: null,
        pendingPreview: null,
        error: errorMessage(caught),
        statusMessage: '变换 ChangeSet 预览失败。',
      }))
      return null
    }
  }, [state.graphRecord, state.project, state.version])

  const previewConnectorSnap = useCallback(async (nodeId: string) => {
    const version = state.version
    if (!version?.module_graph_id) {
      setState((current) => ({ ...current, error: '必须先加载带有效 ModuleGraph 的 Concept Version。' }))
      return null
    }
    const suffix = Date.now().toString(36)
    const clientRequestId = `desktop-connector-snap-${suffix}`
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: `正在计算 ${nodeId} 的 Connector 吸附…`,
    }))
    try {
      const proposed = await forgeApi.proposeConnectorSnap(version.version_id, {
        client_request_id: clientRequestId,
        node_id: nodeId,
      })
      const preview = await forgeApi.previewChangeSet(proposed.change_set_id, `${clientRequestId}-preview`)
      setState((current) => ({
        ...current,
        loading: false,
        pendingChange: null,
        pendingReplacement: null,
        pendingManualChange: proposed,
        pendingPreview: preview,
        graphRecord: current.graphRecord
          ? {
              ...current.graphRecord,
              graph: preview.preview_graph,
              graph_sha256: `ghost-preview:${preview.preview_sha256}`,
            }
          : null,
        qualityRun: null,
        statusMessage: `Connector 吸附预览就绪：${nodeId}；确认后才创建子版本。`,
      }))
      return preview
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        pendingManualChange: null,
        pendingPreview: null,
        error: errorMessage(caught),
        statusMessage: 'Connector 吸附预览失败。',
      }))
      return null
    }
  }, [state.version])

  const confirmManualChange = useCallback(async () => {
    const project = state.project
    const pending = state.pendingManualChange
    if (!project || !pending) return null
    const suffix = Date.now().toString(36)
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在确认变换并创建不可变子版本…',
    }))
    try {
      const confirmed = await forgeApi.confirmChangeSet(pending.change_set_id, `desktop-transform-confirm-${suffix}`)
      const nextVersionId = confirmed.project.current_version_id ?? undefined
      await loadProject(project.project_id, nextVersionId)
      setState((current) => ({
        ...current,
        statusMessage: `变换已确认并创建新版本：${nextVersionId ?? 'unknown'}。`,
      }))
      return confirmed
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '变换确认失败；当前版本未被覆盖。',
      }))
      return null
    }
  }, [loadProject, state.pendingManualChange, state.project])

  const discardManualChange = useCallback(async () => {
    const project = state.project
    const version = state.version
    const pending = state.pendingManualChange
    if (!project || !version || !pending) return null
    const suffix = Date.now().toString(36)
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: '正在放弃变换幽灵预览…',
    }))
    try {
      await forgeApi.rejectChangeSet(pending.change_set_id, `desktop-transform-reject-${suffix}`)
      await loadProject(project.project_id, version.version_id)
      setState((current) => ({
        ...current,
        pendingManualChange: null,
        pendingPreview: null,
        statusMessage: '已放弃变换预览；当前版本保持不变。',
      }))
      return true
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '放弃变换预览失败。',
      }))
      return null
    }
  }, [loadProject, state.pendingManualChange, state.project, state.version])

  const setMirror = useCallback(async (
    nodeId: string,
    mirrorAxis: 'none' | 'x' | 'y' | 'z',
  ) => {
    const project = state.project
    const version = state.version
    const graph = state.graphRecord?.graph
    if (!project || !version || !graph) {
      setState((current) => ({ ...current, error: '必须先加载 Project、Version 与 ModuleGraph。' }))
      return null
    }
    const node = graph.nodes.find((item) => item.node_id === nodeId)
    if (!node) {
      setState((current) => ({ ...current, error: `Graph 节点不存在：${nodeId}` }))
      return null
    }
    if (node.locked) {
      setState((current) => ({ ...current, error: `节点 ${nodeId} 已锁定，不能镜像。` }))
      return null
    }
    if ((node.mirror_axis ?? 'none') === mirrorAxis) return null

    const suffix = Date.now().toString(36)
    const clientRequestId = `desktop-mirror-${suffix}`
    const changeSetId = `change_desktop_mirror_${suffix}`
    setState((current) => ({
      ...current,
      loading: true,
      error: null,
      statusMessage: `正在预览 ${nodeId} 的 ${mirrorAxis.toUpperCase()} 镜像…`,
    }))
    try {
      const proposed = await forgeApi.proposeChangeSet(version.version_id, {
        client_request_id: clientRequestId,
        change_set: {
          schema_version: 'DesignChangeSet@1',
          change_set_id: changeSetId,
          project_id: project.project_id,
          base_version_id: version.version_id,
          summary: `Set ${nodeId} mirror axis to ${mirrorAxis}.`,
          operations: [{
            operation_id: `op_mirror_${suffix}`,
            op: 'set_mirror',
            node_id: nodeId,
            mirror_axis: mirrorAxis,
          }],
          protected_node_ids: graph.nodes.filter((item) => item.locked).map((item) => item.node_id),
          status: 'proposed',
        },
      })
      await forgeApi.previewChangeSet(proposed.change_set_id, `${clientRequestId}-preview`)
      const confirmed = await forgeApi.confirmChangeSet(
        proposed.change_set_id,
        `${clientRequestId}-confirm`,
      )
      const nextVersionId = confirmed.project.current_version_id ?? undefined
      await loadProject(project.project_id, nextVersionId)
      setState((current) => ({
        ...current,
        statusMessage: `镜像已确认并创建新版本：${nextVersionId ?? 'unknown'}。`,
      }))
      return confirmed
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '模块镜像 preview/confirm 失败。',
      }))
      return null
    }
  }, [loadProject, state.graphRecord, state.project, state.version])

  const searchTimeline = useCallback(async (filters: ChangeSetTimelineFilters) => {
    const projectId = state.project?.project_id
    if (!projectId) return
    setState((current) => ({ ...current, timelineLoading: true, error: null }))
    try {
      const response = await forgeApi.listChangeSets(projectId, {
        limit: 20,
        q: filters.query.trim() || undefined,
        status: filters.status || undefined,
        operation: filters.operation || undefined,
      })
      setState((current) => ({
        ...current,
        timeline: response.items ?? [],
        timelineNextCursor: response.next_cursor ?? null,
        timelineLoading: false,
        timelineFilters: filters,
      }))
    } catch (caught) {
      setState((current) => ({
        ...current,
        timelineLoading: false,
        error: errorMessage(caught),
      }))
    }
  }, [state.project?.project_id])

  const createChangeSetAuditExport = useCallback(async (
    filters: ChangeSetTimelineFilters = state.timelineFilters,
  ) => {
    const projectId = state.project?.project_id
    if (!projectId) return null
    const clientRequestId = `desktop-change-audit-${Date.now().toString(36)}`
    setState((current) => ({
      ...current,
      timelineLoading: true,
      error: null,
      statusMessage: '正在生成不可变 ChangeSet 审计归档…',
    }))
    try {
      const result = await forgeApi.createChangeSetAuditExport(projectId, {
        client_request_id: clientRequestId,
        query: filters.query.trim() || null,
        status: filters.status || null,
        operation: filters.operation || null,
        include_jsonl: true,
        include_csv: true,
        retention_class: 'project_lifetime',
        max_records: 5000,
      })
      setState((current) => ({
        ...current,
        timelineLoading: false,
        lastAuditExport: result,
        statusMessage: `审计归档完成 · ${result.record_count} 条 · ${result.package_sha256.slice(0, 12)}…`,
      }))
      return result
    } catch (caught) {
      setState((current) => ({
        ...current,
        timelineLoading: false,
        error: errorMessage(caught),
        statusMessage: 'ChangeSet 审计归档创建失败。',
      }))
      return null
    }
  }, [state.project?.project_id, state.timelineFilters])

  const loadMoreTimeline = useCallback(async () => {
    const projectId = state.project?.project_id
    const cursor = state.timelineNextCursor
    if (!projectId || !cursor || state.timelineLoading) return
    setState((current) => ({ ...current, timelineLoading: true, error: null }))
    try {
      const filters = state.timelineFilters
      const response = await forgeApi.listChangeSets(projectId, {
        cursor,
        limit: 20,
        q: filters.query.trim() || undefined,
        status: filters.status || undefined,
        operation: filters.operation || undefined,
      })
      setState((current) => ({
        ...current,
        timeline: [...current.timeline, ...(response.items ?? [])],
        timelineNextCursor: response.next_cursor ?? null,
        timelineLoading: false,
      }))
    } catch (caught) {
      setState((current) => ({
        ...current,
        timelineLoading: false,
        error: errorMessage(caught),
      }))
    }
  }, [
    state.project?.project_id,
    state.timelineFilters,
    state.timelineLoading,
    state.timelineNextCursor,
  ])

  return {
    ...state,
    refresh,
    selectProject,
    selectVersion,
    createStarterProject,
    initializeCurrentProject,
    createExport,
    planBrief,
    selectVariant,
    planChange,
    confirmPlannedChange,
    discardPlannedChange,
    runQualityInspection,
    previewModuleReplacement,
    confirmModuleReplacement,
    discardModuleReplacement,
    previewNodeTransform,
    previewConnectorSnap,
    confirmManualChange,
    discardManualChange,
    setMirror,
    searchTimeline,
    createChangeSetAuditExport,
    loadMoreTimeline,
  }
}

function errorMessage(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught)
}

function isValidTransform(transform: Transform) {
  return [transform.position, transform.rotation, transform.scale].every((values) => (
    values.length === 3 && values.every((value) => Number.isFinite(value))
  )) && transform.scale.every((value) => value > 0)
}

function sameTransform(left: Transform, right: Transform) {
  return [left.position, left.rotation, left.scale].every((values, index) => {
    const other = [right.position, right.rotation, right.scale][index]
    return values.every((value, valueIndex) => Math.abs(value - other[valueIndex]) < 1e-6)
  })
}
