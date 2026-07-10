import { useCallback, useEffect, useState } from 'react'
import { forgeApi } from '../../shared/api/forgeApi'
import type {
  ConceptExportRecord,
  ConceptProjectDetail,
  ConceptProjectSummary,
  ConceptVersionDetail,
  ChangeSetTimelineItem,
  DesignVariantRecord,
  ModuleAssetRecord,
  ModuleGraphRecord,
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
  loading: boolean
  error: string | null
  statusMessage: string
  lastExport: ConceptExportRecord | null
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
  loading: true,
  error: null,
  statusMessage: '正在读取本地 Concept 数据…',
  lastExport: null,
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
      const [moduleResponse, variantResponse, timelineResponse, version] = await Promise.all([
        forgeApi.listModuleAssets(project.profile.pack_id),
        forgeApi.listDesignVariants(projectId),
        forgeApi.listChangeSets(projectId, { limit: 20 }),
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
      const projects = (await forgeApi.listConceptProjects()).items ?? []
      await loadProject(created.project_id, created.current_version_id ?? undefined, projects)
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: 'Starter Project 创建失败。',
      }))
    }
  }, [loadProject])

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
        ruleset_version: 'weapon-concept-geometry/1.1',
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

  const replaceModule = useCallback(async (nodeId: string, moduleId: string) => {
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
      await forgeApi.previewChangeSet(proposed.change_set_id, `${clientRequestId}-preview`)
      const confirmed = await forgeApi.confirmChangeSet(
        proposed.change_set_id,
        `${clientRequestId}-confirm`,
      )
      const nextVersionId = confirmed.project.current_version_id ?? undefined
      await loadProject(project.project_id, nextVersionId)
      setState((current) => ({
        ...current,
        statusMessage: `替换已确认并创建新版本：${nextVersionId ?? 'unknown'}。`,
      }))
      return confirmed
    } catch (caught) {
      setState((current) => ({
        ...current,
        loading: false,
        error: errorMessage(caught),
        statusMessage: '模块替换 preview/confirm 失败。',
      }))
      return null
    }
  }, [loadProject, state.graphRecord, state.project, state.version])

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
    createExport,
    runQualityInspection,
    replaceModule,
    setMirror,
    searchTimeline,
    loadMoreTimeline,
  }
}

function errorMessage(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught)
}
