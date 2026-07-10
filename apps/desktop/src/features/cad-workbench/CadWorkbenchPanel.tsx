import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  ArrowsClockwise,
  ArrowsOutCardinal,
  CaretDown,
  ChartLineUp,
  ChatCircleDots,
  Check,
  ClockCounterClockwise,
  Crosshair,
  Cube,
  CursorClick,
  Export,
  Eye,
  FileArrowDown,
  FloppyDisk,
  FolderOpen,
  Funnel,
  Gear,
  GridFour,
  House,
  MagnifyingGlass,
  PaperPlaneRight,
  Plus,
  Ruler,
  SelectionAll,
  ShareNetwork,
  SlidersHorizontal,
  Sparkle,
  UserCircle,
  WarningCircle,
} from '@phosphor-icons/react'
import { forgeApi } from '../../shared/api/forgeApi'
import type { ModuleAssetRecord } from '../../shared/types'
import { ModuleGraphViewport } from './ModuleGraphViewport'
import { useConceptWorkbench } from './useConceptWorkbench'
import './cad-workbench.css'

type WorkspaceTab = 'concept' | 'assembly' | 'refine' | 'inspect' | 'showcase'
type InspectorTab = 'parameters' | 'appearance' | 'connections' | 'inspection'
type DrawerTab = 'components' | 'variants' | 'versions' | 'timeline'
type Tool = 'select' | 'move' | 'orbit' | 'measure' | 'section'
type CameraView = 'iso' | 'front' | 'top' | 'right'

type WeaponParameters = {
  overallLength: number
  bodyHeight: number
  frontShellLength: number
  gripAngle: number
  shellThickness: number
  detailDensity: number
}

type ModuleCategory = ModuleAssetRecord['manifest']['category']
type ComponentCategory = 'all' | ModuleCategory

const MODULE_CATEGORY_LABELS: Record<ModuleCategory, string> = {
  core_shell: '核心外壳',
  front_shell: '前部外壳',
  rear_shell: '后部外壳',
  grip_shell: '握持外壳',
  top_accessory: '顶部附件',
  side_accessory: '侧部附件',
  lower_structure: '下部结构',
  storage_visual: '储存视觉',
  armor_panel: '装甲面板',
}

const COMPONENT_CATEGORIES: Array<{ id: ComponentCategory; label: string }> = [
  { id: 'all', label: '全部' },
  ...Object.entries(MODULE_CATEGORY_LABELS).map(([id, label]) => ({
    id: id as ModuleCategory,
    label,
  })),
]

const TOOL_ITEMS: Array<{ id: Tool; label: string; icon: typeof CursorClick }> = [
  { id: 'select', label: '选择', icon: CursorClick },
  { id: 'move', label: '移动', icon: ArrowsOutCardinal },
  { id: 'orbit', label: '旋转视图', icon: ArrowsClockwise },
  { id: 'measure', label: '测量', icon: Ruler },
  { id: 'section', label: '截面', icon: SelectionAll },
]

export function CadWorkbenchPanel({ onOpenLegacy }: { onOpenLegacy: () => void }) {
  const concept = useConceptWorkbench()
  const [activeTab, setActiveTab] = useState<WorkspaceTab>('concept')
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>('parameters')
  const [drawerTab, setDrawerTab] = useState<DrawerTab>('components')
  const [activeTool, setActiveTool] = useState<Tool>('select')
  const [cameraView, setCameraView] = useState<CameraView>('iso')
  const [showGrid, setShowGrid] = useState(true)
  const [wireframe, setWireframe] = useState(false)
  const [selectedComponent, setSelectedComponent] = useState('')
  const [selectedLibraryModuleId, setSelectedLibraryModuleId] = useState('')
  const [hiddenNodeIds, setHiddenNodeIds] = useState<string[]>([])
  const [focusedNodeId, setFocusedNodeId] = useState<string | null>(null)
  const [showConnectors, setShowConnectors] = useState(false)
  const [explodeFactor, setExplodeFactor] = useState(0)
  const [componentCategory, setComponentCategory] = useState<ComponentCategory>('all')
  const [componentQuery, setComponentQuery] = useState('')
  const [chatInput, setChatInput] = useState('')
  const [assistantNote, setAssistantNote] = useState(
    '输入修改要求后，系统将生成结构化 DesignChangeSet 预览；确认前不会覆盖当前版本。',
  )
  const [exportFormat, setExportFormat] = useState('SOURCE ZIP')
  const [parameters, setParameters] = useState<WeaponParameters>({
    overallLength: 230,
    bodyHeight: 54,
    frontShellLength: 120,
    gripAngle: 15,
    shellThickness: 2.5,
    detailDensity: 68,
  })

  useEffect(() => {
    const spec = concept.version?.spec
    if (!spec) return
    setParameters((current) => ({
      ...current,
      overallLength: spec.proportions.overall_length_mm,
      bodyHeight: spec.proportions.body_height_mm,
      gripAngle: spec.proportions.grip_angle_deg,
      detailDensity: Math.round(spec.style.detail_density * 100),
    }))
  }, [concept.version])

  useEffect(() => {
    const nodes = concept.graphRecord?.graph.nodes ?? []
    if (nodes.length === 0) {
      setSelectedComponent('')
      setSelectedLibraryModuleId('')
      return
    }
    setSelectedComponent((current) => {
      const nextNode = nodes.find((node) => node.node_id === current) ?? nodes[0]
      setSelectedLibraryModuleId(nextNode.module_id)
      return nextNode.node_id
    })
    setHiddenNodeIds((current) => current.filter((nodeId) => nodes.some((node) => node.node_id === nodeId)))
  }, [concept.graphRecord])

  const visibleComponents = useMemo(() => {
    const query = componentQuery.trim().toLowerCase()
    const categoryItems = componentCategory === 'all'
      ? concept.modules
      : concept.modules.filter((component) => component.manifest.category === componentCategory)
    if (!query) return categoryItems
    return categoryItems.filter((component) => (
      component.manifest.module_id.toLowerCase().includes(query)
      || MODULE_CATEGORY_LABELS[component.manifest.category].toLowerCase().includes(query)
    ))
  }, [componentCategory, componentQuery, concept.modules])

  const getModuleFileUrl = useCallback(
    (moduleId: string) => forgeApi.getModuleAssetFileUrl(moduleId),
    [],
  )
  const selectedNode = concept.graphRecord?.graph.nodes.find(
    (node) => node.node_id === selectedComponent,
  ) ?? null
  const selectedModule = concept.modules.find(
    (module) => module.manifest.module_id === selectedNode?.module_id,
  ) ?? null
  const selectedLibraryModule = concept.modules.find(
    (module) => module.manifest.module_id === selectedLibraryModuleId,
  ) ?? null
  const canReplaceSelected = Boolean(
    selectedNode
    && selectedLibraryModule
    && !selectedNode.locked
    && selectedNode.module_id !== selectedLibraryModule.manifest.module_id
    && selectedModule?.manifest.category === selectedLibraryModule.manifest.category,
  )
  const activeVersionSummary = (concept.project?.versions ?? []).find(
    (item) => item.version_id === concept.version?.version_id,
  )
  const undoVersionId = activeVersionSummary?.parent_version_id ?? null
  const redoVersionId = (concept.project?.versions ?? [])
    .filter((item) => item.parent_version_id === concept.version?.version_id)
    .sort((left, right) => right.version_no - left.version_no)[0]?.version_id ?? null

  const selectGraphNode = useCallback((nodeId: string) => {
    setSelectedComponent(nodeId)
    const node = concept.graphRecord?.graph.nodes.find((item) => item.node_id === nodeId)
    if (node) setSelectedLibraryModuleId(node.module_id)
  }, [concept.graphRecord])

  const handleCreateExport = useCallback(async () => {
    const result = await concept.createExport()
    if (result) window.location.assign(forgeApi.getConceptExportFileUrl(result.export_id))
  }, [concept])

  const handleReplaceSelected = useCallback(() => {
    if (!selectedNode || !selectedLibraryModule) return
    concept.replaceModule(selectedNode.node_id, selectedLibraryModule.manifest.module_id)
      .catch(() => undefined)
  }, [concept, selectedLibraryModule, selectedNode])

  const toggleSelectedNodeVisibility = useCallback(() => {
    if (!selectedNode) return
    setHiddenNodeIds((current) => (
      current.includes(selectedNode.node_id)
        ? current.filter((nodeId) => nodeId !== selectedNode.node_id)
        : [...current, selectedNode.node_id]
    ))
  }, [selectedNode])

  const handleModuleDrop = useCallback((nodeId: string, moduleId: string) => {
    selectGraphNode(nodeId)
    setSelectedLibraryModuleId(moduleId)
    setAssistantNote(`已将 ${moduleId} 设为 ${nodeId} 的替换候选；点击“替换并创建新版本”后才会提交 ChangeSet。`)
  }, [selectGraphNode])

  const updateParameter = (key: keyof WeaponParameters, value: number) => {
    setParameters((current) => ({ ...current, [key]: value }))
  }

  const submitAssistantInstruction = () => {
    const instruction = chatInput.trim()
    if (!instruction) return
    setAssistantNote(`修改计划已生成：“${instruction}”。关键组件接口保持锁定，确认后将创建新版本。`)
    setChatInput('')
  }

  return (
    <div className="cad-workbench" data-testid="cad-workbench">
      <header className="cad-command-bar">
        <button className="cad-brand" onClick={onOpenLegacy} aria-label="返回迁移前工作台">
          <span className="cad-brand-mark"><Cube size={18} weight="fill" /></span>
          <span>ForgeCAD</span>
        </button>
        <div className="cad-file-actions" aria-label="文件操作">
          <IconAction icon={Plus} label="新建" onClick={() => concept.createStarterProject()} />
          <IconAction icon={FolderOpen} label="同步" onClick={() => concept.refresh()} />
          <IconAction icon={FloppyDisk} label="保存" onClick={() => setAssistantNote('参数仍是本地草稿；请通过 ChangeSet 确认后创建不可变新版本。')} />
          <IconAction
            icon={ClockCounterClockwise}
            label="撤销"
            onClick={() => undoVersionId && concept.selectVersion(undoVersionId)}
            disabled={!undoVersionId || concept.loading}
            title="切换到当前版本的 parent"
          />
          <IconAction
            icon={ArrowsClockwise}
            label="重做"
            onClick={() => redoVersionId && concept.selectVersion(redoVersionId)}
            disabled={!redoVersionId || concept.loading}
            title="切换到最近的 child version"
          />
        </div>
        <nav className="cad-mode-tabs" aria-label="工作模式">
          {([
            ['concept', '概念'],
            ['assembly', '组装'],
            ['refine', '精修'],
            ['inspect', '检查'],
            ['showcase', '展示'],
          ] as Array<[WorkspaceTab, string]>).map(([id, label]) => (
            <button
              key={id}
              className={activeTab === id ? 'active' : ''}
              onClick={() => setActiveTab(id)}
            >
              {label}
            </button>
          ))}
        </nav>
        <div className="cad-global-actions">
          <IconButton icon={ShareNetwork} label="共享" />
          <IconButton icon={Gear} label="设置" />
          <UserCircle size={25} weight="duotone" />
          <CaretDown size={14} />
        </div>
      </header>

      <div className="cad-layout">
        <aside className="cad-left-rail">
          <section className="cad-panel project-panel">
            <div className="cad-panel-heading">
              <div>
                <span className="eyebrow">项目</span>
                <strong>{concept.project?.name ?? '尚未创建 Concept Project'}</strong>
              </div>
              <CaretDown size={14} />
            </div>
            {concept.projects.length > 1 && (
              <label className="project-select">
                <span>切换项目</span>
                <select
                  value={concept.project?.project_id ?? ''}
                  onChange={(event) => concept.selectProject(event.target.value)}
                >
                  {concept.projects.map((project) => (
                    <option key={project.project_id} value={project.project_id}>{project.name}</option>
                  ))}
                </select>
              </label>
            )}
            <div className="version-heading">版本历史</div>
            <div className="version-list">
              {(concept.project?.versions ?? []).slice().reverse().map((version) => (
                <button
                  key={version.version_id}
                  className={concept.version?.version_id === version.version_id ? 'active' : ''}
                  onClick={() => concept.selectVersion(version.version_id)}
                >
                  <span>V{version.version_no}</span>
                  <strong>{version.summary}</strong>
                  <small>{formatVersionTime(version.created_at)}</small>
                </button>
              ))}
              {!concept.project && !concept.loading && (
                <button className="empty-action" onClick={() => concept.createStarterProject()}>
                  <Plus size={14} /> 创建“寒地巡逻 S1”
                </button>
              )}
            </div>
          </section>

          <section className="cad-panel assistant-panel">
            <div className="cad-panel-title">
              <span><Sparkle size={16} weight="fill" /> AI 设计助手</span>
              <span className="assistant-state">在线</span>
            </div>
            <div className="assistant-message">{concept.error ?? assistantNote}</div>
            <div className={`concept-runtime-state ${concept.error ? 'error' : ''}`}>
              {concept.loading ? '同步中 · ' : ''}{concept.statusMessage}
            </div>
            <div className="assistant-suggestions">
              <button onClick={() => setAssistantNote('方案 A：短枪管与紧凑握把，强调模块化和便携性。')}>紧凑方案</button>
              <button onClick={() => setAssistantNote('方案 B：延长上导轨与枪管护罩，提升未来工业感。')}>长导轨方案</button>
            </div>
            <button className="secondary-action" disabled title="R4 Change Planner 待实现">R4 ChangeSet 待接入</button>
          </section>

          <section className="cad-panel quick-parameters">
            <div className="cad-panel-title"><span><SlidersHorizontal size={16} /> 参数化输入</span></div>
            <ParameterInput
              label="整体长度"
              value={parameters.overallLength}
              unit="mm"
              onChange={(value) => updateParameter('overallLength', value)}
            />
            <ParameterInput
              label="前部长度"
              value={parameters.frontShellLength}
              unit="mm"
              onChange={(value) => updateParameter('frontShellLength', value)}
            />
            <ParameterInput
              label="握把角度"
              value={parameters.gripAngle}
              unit="°"
              onChange={(value) => updateParameter('gripAngle', value)}
            />
            <ParameterInput
              label="细节密度"
              value={parameters.detailDensity}
              unit="%"
              onChange={(value) => updateParameter('detailDensity', value)}
            />
            <button className="primary-action" onClick={() => setAssistantNote('参数草稿已记录在本地 UI，尚未写入 Version；下一步通过 DesignChangeSet 预览与确认。')}>
              记录参数草稿
            </button>
          </section>

          <div className="assistant-composer">
            <ChatCircleDots size={17} />
            <input
              value={chatInput}
              onChange={(event) => setChatInput(event.target.value)}
              onKeyDown={(event) => event.key === 'Enter' && submitAssistantInstruction()}
              placeholder="输入设计需求…"
            />
            <button onClick={submitAssistantInstruction} aria-label="发送设计需求">
              <PaperPlaneRight size={16} weight="fill" />
            </button>
          </div>
        </aside>

        <main className="cad-center-stage">
          <div className="viewport-shell">
            <div className="viewport-toolbar" aria-label="CAD 视口工具">
              {TOOL_ITEMS.map((tool) => (
                <IconButton
                  key={tool.id}
                  icon={tool.icon}
                  label={tool.label}
                  active={activeTool === tool.id}
                  onClick={() => setActiveTool(tool.id)}
                />
              ))}
              <span className="toolbar-divider" />
              <IconButton
                icon={GridFour}
                label="网格"
                active={showGrid}
                onClick={() => setShowGrid((current) => !current)}
              />
              <IconButton
                icon={Eye}
                label="线框"
                active={wireframe}
                onClick={() => setWireframe((current) => !current)}
              />
            </div>
            <ModuleGraphViewport
              graphRecord={concept.graphRecord}
              modules={concept.modules}
              cameraView={cameraView}
              showGrid={showGrid}
              wireframe={wireframe}
              selectedNodeId={selectedComponent}
              hiddenNodeIds={hiddenNodeIds}
              focusNodeId={focusedNodeId}
              showConnectors={showConnectors}
              explodeFactor={explodeFactor}
              getModuleFileUrl={getModuleFileUrl}
              onSelectNode={selectGraphNode}
              onDropModule={handleModuleDrop}
            />
            <div className="view-cube"><Cube size={28} weight="duotone" /></div>
            <div className="viewport-viewbar">
              <IconButton icon={House} label="等轴" active={cameraView === 'iso'} onClick={() => setCameraView('iso')} />
              <IconButton icon={Crosshair} label="正视" active={cameraView === 'front'} onClick={() => setCameraView('front')} />
              <IconButton icon={GridFour} label="顶视" active={cameraView === 'top'} onClick={() => setCameraView('top')} />
              <IconButton icon={Cube} label="右视" active={cameraView === 'right'} onClick={() => setCameraView('right')} />
              <IconButton
                icon={ArrowsOutCardinal}
                label="爆炸视图"
                active={explodeFactor > 0}
                onClick={() => setExplodeFactor((current) => current > 0 ? 0 : 0.42)}
              />
            </div>
            <div className="viewport-readout">
              <span>{activeTool === 'measure' ? '测量模式：选择两个几何点' : `${activeTool} 工具已启用`}</span>
              <span>单位：mm</span>
            </div>
          </div>

          <section className="component-library">
            <div className="component-library-header">
              <nav className="drawer-tabs" aria-label="底部工作区">
                {([
                  ['components', '组件'],
                  ['variants', '方案'],
                  ['versions', '版本'],
                  ['timeline', '时间线'],
                ] as Array<[DrawerTab, string]>).map(([id, label]) => (
                  <button key={id} className={drawerTab === id ? 'active' : ''} onClick={() => setDrawerTab(id)}>
                    {label}
                  </button>
                ))}
              </nav>
              <div className="component-search">
                <MagnifyingGlass size={15} />
                <input
                  value={componentQuery}
                  onChange={(event) => setComponentQuery(event.target.value)}
                  placeholder="搜索组件…"
                />
                <Funnel size={14} />
              </div>
            </div>
            <div className="component-library-body">
              {drawerTab === 'components' ? (
                <>
                  <nav className="component-categories">
                    {COMPONENT_CATEGORIES.map((category) => (
                      <button
                        key={category.id}
                        className={componentCategory === category.id ? 'active' : ''}
                        onClick={() => setComponentCategory(category.id)}
                      >
                        {category.label}
                      </button>
                    ))}
                  </nav>
                  <div className="component-library-content">
                    <div className="module-replace-bar">
                      <span>节点：{selectedNode?.node_id ?? '未选择'}</span>
                      <span>候选：{selectedLibraryModule?.manifest.module_id ?? '未选择'}</span>
                      <button
                        onClick={handleReplaceSelected}
                        disabled={!canReplaceSelected || concept.loading}
                        title={selectedNode?.locked ? '锁定节点不能替换' : '通过 ChangeSet preview/confirm 创建子版本'}
                      >
                        替换并创建新版本
                      </button>
                    </div>
                    <div className="component-grid">
                      {visibleComponents.map((component) => {
                        const ComponentIcon = componentIconFor(component.manifest.category)
                        const graphNode = concept.graphRecord?.graph.nodes.find(
                          (node) => node.module_id === component.manifest.module_id,
                        )
                        const isActiveNode = Boolean(graphNode && selectedComponent === graphNode.node_id)
                        const isCandidate = selectedLibraryModuleId === component.manifest.module_id
                        return (
                          <button
                            key={component.manifest.module_id}
                            className={`${isActiveNode ? 'active' : ''} ${isCandidate ? 'candidate' : ''}`.trim()}
                            draggable
                            onDragStart={(event) => {
                              event.dataTransfer.effectAllowed = 'copy'
                              event.dataTransfer.setData('application/x-forgecad-module-id', component.manifest.module_id)
                              event.dataTransfer.setData('text/plain', component.manifest.module_id)
                            }}
                            onClick={() => {
                              setSelectedLibraryModuleId(component.manifest.module_id)
                              if (graphNode) selectGraphNode(graphNode.node_id)
                            }}
                          >
                            <span className="component-visual"><ComponentIcon size={34} weight="duotone" /></span>
                            <strong>{component.manifest.module_id}</strong>
                            <small>{MODULE_CATEGORY_LABELS[component.manifest.category]} · {component.manifest.triangle_count.toLocaleString()} tris</small>
                          </button>
                        )
                      })}
                      {visibleComponents.length === 0 && (
                        <div className="component-empty">
                          <strong>Module Pack 为空</strong>
                          <span>先通过 ModuleAssetManifest 注册 GLB，工作台不会用假组件替代。</span>
                        </div>
                      )}
                    </div>
                  </div>
                </>
              ) : (
                <div className="drawer-placeholder">
                  {drawerTab === 'variants' && <>
                    <strong>候选方案</strong>
                    {concept.variants.map((variant) => (
                      <button key={variant.variant_id}>{variant.rank}. {variant.name} · {variant.status}</button>
                    ))}
                    {concept.variants.length === 0 && <span>当前项目尚无已持久化方案。</span>}
                  </>}
                  {drawerTab === 'versions' && <>
                    <strong>版本分支</strong>
                    {(concept.project?.versions ?? []).slice().reverse().map((version) => (
                      <button key={version.version_id} onClick={() => concept.selectVersion(version.version_id)}>
                        V{version.version_no} · {version.summary}
                      </button>
                    ))}
                  </>}
                  {drawerTab === 'timeline' && <>
                    <strong>设计时间线</strong>
                    {(concept.project?.versions ?? []).slice().reverse().map((version) => (
                      <span key={version.version_id}>{formatVersionTime(version.created_at)} · V{version.version_no} {version.summary}</span>
                    ))}
                  </>}
                </div>
              )}
            </div>
          </section>
        </main>

        <aside className="cad-right-rail">
          <section className="cad-panel properties-panel">
            <div className="cad-panel-title"><span><SlidersHorizontal size={16} /> 属性面板</span></div>
            <nav className="inspector-tabs" aria-label="属性分类">
              {([
                ['parameters', '参数'],
                ['appearance', '外观'],
                ['connections', '连接'],
                ['inspection', '检查'],
              ] as Array<[InspectorTab, string]>).map(([id, label]) => (
                <button key={id} className={inspectorTab === id ? 'active' : ''} onClick={() => setInspectorTab(id)}>
                  {label}
                </button>
              ))}
            </nav>
            {inspectorTab === 'parameters' && <>
              <label className="wide-field"><span>Graph 节点</span><input value={selectedNode?.node_id ?? '未选择'} readOnly /></label>
              <label className="wide-field"><span>模块资产</span><input value={selectedModule?.manifest.module_id ?? '—'} readOnly /></label>
              <div className="node-actions">
                <button onClick={toggleSelectedNodeVisibility} disabled={!selectedNode}>
                  <Eye size={13} /> {selectedNode && hiddenNodeIds.includes(selectedNode.node_id) ? '显示' : '隐藏'}
                </button>
                <button onClick={() => selectedNode && setFocusedNodeId(selectedNode.node_id)} disabled={!selectedNode}>
                  <Crosshair size={13} /> 聚焦
                </button>
                <button className={showConnectors ? 'active' : ''} onClick={() => setShowConnectors((current) => !current)}>
                  <ShareNetwork size={13} /> Connector
                </button>
              </div>
              <div className="axis-group"><span>位置</span><div>
                <AxisField axis="X" value={formatAxis(selectedNode?.transform.position[0])} />
                <AxisField axis="Y" value={formatAxis(selectedNode?.transform.position[1])} />
                <AxisField axis="Z" value={formatAxis(selectedNode?.transform.position[2])} />
              </div></div>
              <div className="axis-group"><span>旋转（rad）</span><div>
                <AxisField axis="X" value={formatAxis(selectedNode?.transform.rotation[0])} />
                <AxisField axis="Y" value={formatAxis(selectedNode?.transform.rotation[1])} />
                <AxisField axis="Z" value={formatAxis(selectedNode?.transform.rotation[2])} />
              </div></div>
              <div className="property-divider" />
              <div className="property-heading">概念比例 <CaretDown size={13} /></div>
              <PropertyNumber label="整体长度" value={parameters.overallLength} unit="mm" onChange={(value) => updateParameter('overallLength', value)} />
              <PropertyNumber label="主体高度" value={parameters.bodyHeight} unit="mm" onChange={(value) => updateParameter('bodyHeight', value)} />
              <PropertyNumber label="前部长度" value={parameters.frontShellLength} unit="mm" onChange={(value) => updateParameter('frontShellLength', value)} />
              <PropertyNumber label="握持角度" value={parameters.gripAngle} unit="°" onChange={(value) => updateParameter('gripAngle', value)} />
              <PropertyNumber label="外壳厚度" value={parameters.shellThickness} unit="mm" onChange={(value) => updateParameter('shellThickness', value)} />
            </>}
            {inspectorTab === 'appearance' && <>
              <label className="wide-field"><span>表面主题</span><select defaultValue="arctic"><option value="arctic">寒地石墨</option><option value="industrial">工业枪灰</option><option value="prototype">原型树脂</option></select></label>
              <PropertyNumber label="细节密度" value={parameters.detailDensity} unit="%" onChange={(value) => updateParameter('detailDensity', value)} />
              <div className="appearance-swatches"><button aria-label="石墨黑" /><button aria-label="枪灰" /><button aria-label="信号红" /></div>
            </>}
            {inspectorTab === 'connections' && <div className="connection-list">
              {(selectedModule?.manifest.connectors ?? []).map((connector) => {
                const connected = (concept.graphRecord?.graph.edges ?? []).some((edge) => (
                  edge.from_connector_id === connector.connector_id
                  || edge.to_connector_id === connector.connector_id
                ))
                return (
                  <DfmRow
                    key={connector.connector_id}
                    label={connector.slot}
                    value={connected ? '已连接' : '可用'}
                    ok={connected}
                  />
                )
              })}
              {!selectedModule && <span className="muted-inspector">选择一个 Graph 节点查看真实 Connector。</span>}
            </div>}
            {inspectorTab === 'inspection' && <>
              <DfmRow label="Graph 状态" value={concept.graphRecord?.validation_status ?? '未运行'} ok={concept.graphRecord?.validation_status === 'valid'} />
              <DfmRow label="模块节点" value={String(concept.graphRecord?.graph.nodes.length ?? 0)} ok={Boolean(concept.graphRecord)} />
              <DfmRow label="连接边" value={String(concept.graphRecord?.graph.edges?.length ?? 0)} ok={Boolean(concept.graphRecord)} />
              <DfmRow label="Mesh 检查" value="R5 待实现" ok={false} />
              <div className="dfm-suggestion"><WarningCircle size={15} /> 提示：当前是非功能性概念模型，不代表制造或安全验证。</div>
              <button className="secondary-action" disabled title="R5 实际检查器待实现">R5 检查报告待实现</button>
            </>}
          </section>

          <section className="cad-panel export-panel">
            <div className="cad-panel-title"><span><Export size={16} /> 展示与导出</span></div>
            <div className="export-formats">
              {[
                { id: 'SOURCE ZIP', enabled: true },
                { id: 'GLB', enabled: false },
                { id: 'OBJ', enabled: false },
                { id: 'PNG', enabled: false },
              ].map((format) => (
                <button
                  key={format.id}
                  className={exportFormat === format.id ? 'active' : ''}
                  onClick={() => format.enabled && setExportFormat(format.id)}
                  disabled={!format.enabled}
                  title={format.enabled ? '当前可用' : 'R5 实现'}
                >
                  {format.id}
                </button>
              ))}
            </div>
            <div className="export-summary">
              <span><FileArrowDown size={15} /> 当前格式</span>
              <strong>{exportFormat}</strong>
            </div>
            {concept.lastExport && (
              <div className="last-export">{concept.lastExport.export_id} · {concept.lastExport.package_sha256.slice(0, 10)}…</div>
            )}
            <button
              className="primary-action"
              onClick={handleCreateExport}
              disabled={!concept.version?.module_graph_id || concept.loading}
            >
              <FileArrowDown size={16} /> 创建并下载概念源包
            </button>
          </section>
        </aside>
      </div>

      <footer className="cad-status-bar">
        <span>{({ concept: '概念', assembly: '组装', refine: '精修', inspect: '检查', showcase: '展示' } as Record<WorkspaceTab, string>)[activeTab]}阶段</span>
        <span>选择：{selectedComponent || '无'}</span>
        <span>模型：{concept.graphRecord ? `${concept.graphRecord.graph.nodes.length} nodes` : '未绑定 ModuleGraph'}</span>
        <span>单位：mm</span>
        <span>网格：10 mm</span>
        <span className="status-spacer" />
        <span>右键：上下文菜单</span>
      </footer>
    </div>
  )
}

function IconAction({
  icon: Icon,
  label,
  onClick,
  disabled = false,
  title,
}: {
  icon: typeof Plus
  label: string
  onClick?: () => void
  disabled?: boolean
  title?: string
}) {
  return <button onClick={onClick} disabled={disabled} title={title}><Icon size={15} /><span>{label}</span></button>
}

function IconButton({
  icon: Icon,
  label,
  active = false,
  onClick,
}: {
  icon: typeof CursorClick
  label: string
  active?: boolean
  onClick?: () => void
}) {
  return (
    <button className={active ? 'active' : ''} onClick={onClick} title={label} aria-label={label}>
      <Icon size={17} />
    </button>
  )
}

function ParameterInput({
  label,
  value,
  unit,
  onChange,
}: {
  label: string
  value: number
  unit: string
  onChange: (value: number) => void
}) {
  return (
    <label className="parameter-row">
      <span>{label}</span>
      <input type="number" value={value} onChange={(event) => onChange(Number(event.target.value))} />
      <small>{unit}</small>
    </label>
  )
}

function PropertyNumber({
  label,
  value,
  unit,
  onChange,
}: {
  label: string
  value: number
  unit: string
  onChange: (value: number) => void
}) {
  return (
    <label className="property-number">
      <span>{label}</span>
      <input type="number" value={value} onChange={(event) => onChange(Number(event.target.value))} />
      <small>{unit}</small>
    </label>
  )
}

function AxisField({ axis, value }: { axis: string; value: string }) {
  return <label className={`axis-field axis-${axis.toLowerCase()}`}><span>{axis}</span><input value={value} readOnly /></label>
}

function DfmRow({ label, value, ok }: { label: string; value: string; ok: boolean }) {
  return (
    <div className="dfm-row">
      <span>{label}</span>
      <strong>{value}</strong>
      {ok ? <Check size={15} weight="bold" /> : <WarningCircle size={15} />}
    </div>
  )
}

function componentIconFor(category: ModuleCategory) {
  const icons: Record<ModuleCategory, typeof Cube> = {
    core_shell: SelectionAll,
    front_shell: Ruler,
    rear_shell: ArrowsClockwise,
    grip_shell: GridFour,
    top_accessory: ChartLineUp,
    side_accessory: Crosshair,
    lower_structure: ArrowsOutCardinal,
    storage_visual: Cube,
    armor_panel: SelectionAll,
  }
  return icons[category]
}

function formatVersionTime(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

function formatAxis(value: number | undefined): string {
  return typeof value === 'number' && Number.isFinite(value) ? value.toFixed(2) : '—'
}
