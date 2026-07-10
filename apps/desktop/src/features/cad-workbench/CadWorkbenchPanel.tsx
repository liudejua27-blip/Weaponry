import { useEffect, useMemo, useRef, useState } from 'react'
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
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { RoundedBoxGeometry } from 'three/examples/jsm/geometries/RoundedBoxGeometry.js'
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

const VERSION_ITEMS = [
  { id: 'v5', label: '优化枪管结构', time: '10:30' },
  { id: 'v4', label: '调整握把角度', time: '昨天' },
  { id: 'v3', label: '增加瞄准镜', time: '昨天' },
  { id: 'v2', label: '初始参数化设计', time: '上周' },
]

const COMPONENTS = [
  { id: 'core-shell-01', name: '核心外壳_01', type: '核心外壳', icon: SelectionAll },
  { id: 'front-shell-01', name: '前部外壳_01', type: '前部外壳', icon: Ruler },
  { id: 'grip-shell-01', name: '握持外壳_01', type: '握持外壳', icon: GridFour },
  { id: 'top-accessory-01', name: '顶部附件_01', type: '顶部附件', icon: ChartLineUp },
  { id: 'top-accessory-02', name: '顶部附件_02', type: '顶部附件', icon: Crosshair },
  { id: 'storage-visual-01', name: '储存视觉_01', type: '视觉模块', icon: Cube },
  { id: 'front-shell-02', name: '前部外壳_02', type: '前部外壳', icon: ArrowsClockwise },
]

const COMPONENT_CATEGORIES = ['全部', '核心外壳', '前部外壳', '握持外壳', '顶部附件', '视觉模块'] as const
type ComponentCategory = (typeof COMPONENT_CATEGORIES)[number]

const TOOL_ITEMS: Array<{ id: Tool; label: string; icon: typeof CursorClick }> = [
  { id: 'select', label: '选择', icon: CursorClick },
  { id: 'move', label: '移动', icon: ArrowsOutCardinal },
  { id: 'orbit', label: '旋转视图', icon: ArrowsClockwise },
  { id: 'measure', label: '测量', icon: Ruler },
  { id: 'section', label: '截面', icon: SelectionAll },
]

export function CadWorkbenchPanel({ onOpenLegacy }: { onOpenLegacy: () => void }) {
  const [activeTab, setActiveTab] = useState<WorkspaceTab>('concept')
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>('parameters')
  const [drawerTab, setDrawerTab] = useState<DrawerTab>('components')
  const [activeTool, setActiveTool] = useState<Tool>('select')
  const [cameraView, setCameraView] = useState<CameraView>('iso')
  const [showGrid, setShowGrid] = useState(true)
  const [wireframe, setWireframe] = useState(false)
  const [activeVersion, setActiveVersion] = useState('v5')
  const [selectedComponent, setSelectedComponent] = useState('core-shell-01')
  const [componentCategory, setComponentCategory] = useState<ComponentCategory>('全部')
  const [componentQuery, setComponentQuery] = useState('')
  const [chatInput, setChatInput] = useState('')
  const [assistantNote, setAssistantNote] = useState(
    '设计一把未来科幻风格的手枪，具有模块化结构和可替换组件。已生成两套候选结构。',
  )
  const [exportFormat, setExportFormat] = useState('GLB')
  const [parameters, setParameters] = useState<WeaponParameters>({
    overallLength: 230,
    bodyHeight: 54,
    frontShellLength: 120,
    gripAngle: 15,
    shellThickness: 2.5,
    detailDensity: 68,
  })

  const visibleComponents = useMemo(() => {
    const query = componentQuery.trim().toLowerCase()
    const categoryItems = componentCategory === '全部'
      ? COMPONENTS
      : COMPONENTS.filter((component) => component.type === componentCategory)
    if (!query) return categoryItems
    return categoryItems.filter((component) => (
      component.name.toLowerCase().includes(query)
      || component.type.toLowerCase().includes(query)
    ))
  }, [componentCategory, componentQuery])

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
          <IconAction icon={Plus} label="新建" />
          <IconAction icon={FolderOpen} label="打开" />
          <IconAction icon={FloppyDisk} label="保存" />
          <IconAction icon={ClockCounterClockwise} label="撤销" />
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
                <strong>寒地巡逻 S1</strong>
              </div>
              <CaretDown size={14} />
            </div>
            <div className="version-heading">版本历史</div>
            <div className="version-list">
              {VERSION_ITEMS.map((version) => (
                <button
                  key={version.id}
                  className={activeVersion === version.id ? 'active' : ''}
                  onClick={() => setActiveVersion(version.id)}
                >
                  <span>{version.id}</span>
                  <strong>{version.label}</strong>
                  <small>{version.time}</small>
                </button>
              ))}
            </div>
          </section>

          <section className="cad-panel assistant-panel">
            <div className="cad-panel-title">
              <span><Sparkle size={16} weight="fill" /> AI 设计助手</span>
              <span className="assistant-state">在线</span>
            </div>
            <div className="assistant-message">{assistantNote}</div>
            <div className="assistant-suggestions">
              <button onClick={() => setAssistantNote('方案 A：短枪管与紧凑握把，强调模块化和便携性。')}>紧凑方案</button>
              <button onClick={() => setAssistantNote('方案 B：延长上导轨与枪管护罩，提升未来工业感。')}>长导轨方案</button>
            </div>
            <button className="secondary-action">应用此方案到设计</button>
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
            <button className="primary-action" onClick={() => setAssistantNote('参数已更新，已生成 ModuleGraph 组合预览；确认后将创建新版本。')}>
              生成组合预览
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
            <WeaponViewport
              parameters={parameters}
              cameraView={cameraView}
              showGrid={showGrid}
              wireframe={wireframe}
              selectedComponent={selectedComponent}
            />
            <div className="view-cube"><Cube size={28} weight="duotone" /></div>
            <div className="viewport-viewbar">
              <IconButton icon={House} label="等轴" active={cameraView === 'iso'} onClick={() => setCameraView('iso')} />
              <IconButton icon={Crosshair} label="正视" active={cameraView === 'front'} onClick={() => setCameraView('front')} />
              <IconButton icon={GridFour} label="顶视" active={cameraView === 'top'} onClick={() => setCameraView('top')} />
              <IconButton icon={Cube} label="右视" active={cameraView === 'right'} onClick={() => setCameraView('right')} />
              <IconButton icon={ArrowsOutCardinal} label="适配窗口" />
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
                        key={category}
                        className={componentCategory === category ? 'active' : ''}
                        onClick={() => setComponentCategory(category)}
                      >
                        {category}
                      </button>
                    ))}
                  </nav>
                  <div className="component-grid">
                    {visibleComponents.map((component) => {
                      const ComponentIcon = component.icon
                      return (
                        <button
                          key={component.id}
                          className={selectedComponent === component.id ? 'active' : ''}
                          onClick={() => setSelectedComponent(component.id)}
                        >
                          <span className="component-visual"><ComponentIcon size={34} weight="duotone" /></span>
                          <strong>{component.name}</strong>
                          <small>{component.type}</small>
                        </button>
                      )
                    })}
                  </div>
                </>
              ) : (
                <div className="drawer-placeholder">
                  {drawerTab === 'variants' && <><strong>候选方案</strong><button>紧凑巡逻型</button><button>长轮廓展示型</button></>}
                  {drawerTab === 'versions' && <><strong>版本分支</strong><button>v5 当前版本</button><button>v4 父版本</button></>}
                  {drawerTab === 'timeline' && <><strong>设计时间线</strong><span>10:30 应用顶部附件 ChangeSet</span><span>10:12 锁定核心外壳</span></>}
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
              <label className="wide-field"><span>组件名称</span><input value={selectedComponent} readOnly /></label>
              <div className="axis-group"><span>位置（mm）</span><div><AxisField axis="X" value="0.00" /><AxisField axis="Y" value="0.00" /><AxisField axis="Z" value="0.00" /></div></div>
              <div className="axis-group"><span>旋转（°）</span><div><AxisField axis="X" value="0.00" /><AxisField axis="Y" value="0.00" /><AxisField axis="Z" value="0.00" /></div></div>
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
              <DfmRow label="core.front" value="已连接" ok />
              <DfmRow label="core.grip" value="已锁定" ok />
              <DfmRow label="core.top" value="已连接" ok />
              <DfmRow label="core.right" value="可用" ok />
            </div>}
            {inspectorTab === 'inspection' && <>
              <DfmRow label="网格完整性" value="通过" ok />
              <DfmRow label="模块相交" value="0 处" ok />
              <DfmRow label="连接器对齐" value="6 / 6" ok />
              <DfmRow label="对称偏差" value="0.4 mm" ok />
              <div className="dfm-suggestion"><WarningCircle size={15} /> 提示：当前是非功能性概念模型，不代表制造或安全验证。</div>
              <button className="secondary-action">查看模型检查报告</button>
            </>}
          </section>

          <section className="cad-panel export-panel">
            <div className="cad-panel-title"><span><Export size={16} /> 展示与导出</span></div>
            <div className="export-formats">
              {['GLB', 'OBJ', 'PNG', 'REPORT'].map((format) => (
                <button
                  key={format}
                  className={exportFormat === format ? 'active' : ''}
                  onClick={() => setExportFormat(format)}
                >
                  {format}
                </button>
              ))}
            </div>
            <div className="export-summary">
              <span><FileArrowDown size={15} /> 当前格式</span>
              <strong>{exportFormat}</strong>
            </div>
            <button className="primary-action"><FileArrowDown size={16} /> 创建概念交付包</button>
          </section>
        </aside>
      </div>

      <footer className="cad-status-bar">
        <span>{({ concept: '概念', assembly: '组装', refine: '精修', inspect: '检查', showcase: '展示' } as Record<WorkspaceTab, string>)[activeTab]}阶段</span>
        <span>选择：{selectedComponent}</span>
        <span>模型：ModuleGraph 预览</span>
        <span>单位：mm</span>
        <span>网格：10 mm</span>
        <span className="status-spacer" />
        <span>右键：上下文菜单</span>
      </footer>
    </div>
  )
}

function WeaponViewport({
  parameters,
  cameraView,
  showGrid,
  wireframe,
  selectedComponent,
}: {
  parameters: WeaponParameters
  cameraView: CameraView
  showGrid: boolean
  wireframe: boolean
  selectedComponent: string
}) {
  const hostRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const host = hostRef.current
    if (!host) return
    const scene = new THREE.Scene()
    scene.background = new THREE.Color('#101823')
    scene.fog = new THREE.Fog('#101823', 380, 720)

    const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 1400)
    setCameraPosition(camera, cameraView)

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    renderer.outputColorSpace = THREE.SRGBColorSpace
    renderer.shadowMap.enabled = true
    host.appendChild(renderer.domElement)

    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = true
    controls.target.set(0, 20, 0)
    controls.minDistance = 210
    controls.maxDistance = 620

    scene.add(new THREE.HemisphereLight('#d9e7ff', '#17202a', 3.1))
    scene.add(new THREE.AmbientLight('#8aa2c4', 0.75))
    const keyLight = new THREE.DirectionalLight('#ffffff', 4.2)
    keyLight.position.set(120, 180, 140)
    keyLight.castShadow = true
    scene.add(keyLight)
    const rimLight = new THREE.DirectionalLight('#4895ff', 1.8)
    rimLight.position.set(-140, 80, -100)
    scene.add(rimLight)

    const grid = new THREE.GridHelper(420, 42, '#2f66ff', '#263747')
    grid.visible = showGrid
    scene.add(grid)
    scene.add(new THREE.AxesHelper(28))

    const model = createWeaponModel(parameters, wireframe, selectedComponent)
    scene.add(model)

    const resize = () => {
      const width = host.clientWidth
      const height = host.clientHeight
      renderer.setSize(width, height, false)
      camera.aspect = width / Math.max(height, 1)
      camera.updateProjectionMatrix()
    }
    const observer = new ResizeObserver(resize)
    observer.observe(host)
    resize()

    let animationFrame = 0
    const render = () => {
      controls.update()
      renderer.render(scene, camera)
      animationFrame = requestAnimationFrame(render)
    }
    render()

    return () => {
      cancelAnimationFrame(animationFrame)
      observer.disconnect()
      controls.dispose()
      scene.traverse((object) => {
        if (object instanceof THREE.Mesh || object instanceof THREE.LineSegments) {
          object.geometry.dispose()
          const materials = Array.isArray(object.material) ? object.material : [object.material]
          materials.forEach((material) => material.dispose())
        }
      })
      renderer.dispose()
      renderer.domElement.remove()
    }
  }, [cameraView, parameters, selectedComponent, showGrid, wireframe])

  return <div className="weapon-viewport" ref={hostRef} aria-label="可交互未来手枪三维视口" />
}

function createWeaponModel(
  parameters: WeaponParameters,
  wireframe: boolean,
  selectedComponent: string,
): THREE.Group {
  const group = new THREE.Group()
  const materialFor = (id: string, color = '#697582', metalness = 0.78) => {
    const selected = selectedComponent === id
    return new THREE.MeshStandardMaterial({
      color,
      emissive: selected ? '#173f72' : '#000000',
      emissiveIntensity: selected ? 0.38 : 0,
      metalness,
      roughness: 0.3,
      wireframe,
    })
  }
  const addPart = (
    id: string,
    geometry: THREE.BufferGeometry,
    position: [number, number, number],
    rotation: [number, number, number] = [0, 0, 0],
    color?: string,
    metalness?: number,
  ) => {
    const mesh = new THREE.Mesh(geometry, materialFor(id, color, metalness))
    mesh.position.set(...position)
    mesh.rotation.set(...rotation)
    mesh.castShadow = true
    mesh.receiveShadow = true
    mesh.userData.componentId = id
    group.add(mesh)
    if (!wireframe) {
      const outline = new THREE.LineSegments(
        new THREE.EdgesGeometry(geometry, 34),
        new THREE.LineBasicMaterial({ color: '#aebdce', transparent: true, opacity: 0.18 }),
      )
      outline.position.copy(mesh.position)
      outline.rotation.copy(mesh.rotation)
      group.add(outline)
    }
    return mesh
  }

  const scale = THREE.MathUtils.clamp(parameters.overallLength / 230, 0.7, 1.35)
  const bodyHeight = THREE.MathUtils.clamp(parameters.bodyHeight, 42, 72)
  const bodyLength = 154 * scale
  const bodyDepth = 38 * scale

  addPart('core-shell-01', new RoundedBoxGeometry(bodyLength, bodyHeight, bodyDepth, 5, 5), [18, 24, 0], [0, 0, -0.035], '#74808d')
  addPart('core-shell-01', new RoundedBoxGeometry(bodyLength * 0.82, 15, bodyDepth + 5, 4, 2.5), [27, 54, 0], [0, 0, -0.02], '#46515d')
  addPart('core-shell-01', new RoundedBoxGeometry(bodyLength * 0.57, 14, bodyDepth - 3, 3, 3), [-1, 1, 0], [0, 0, 0.03], '#202832')
  addPart('core-shell-01', new RoundedBoxGeometry(70, 17, bodyDepth + 1.5, 3, 3), [5, 15, 0], [0, 0, -0.03], '#414c58')

  const frontShellLength = THREE.MathUtils.clamp(parameters.frontShellLength, 82, 165)
  addPart('front-shell-01', new THREE.CylinderGeometry(10, 10, frontShellLength, 32), [-58, 26, 0], [0, 0, Math.PI / 2], '#343c46')
  addPart('front-shell-01', new THREE.CylinderGeometry(15, 15, frontShellLength * 0.72, 12), [-35, 26, 0], [0, 0, Math.PI / 2], '#4d5864')
  addPart('front-shell-02', new THREE.CylinderGeometry(18, 18, 15, 32), [-58 - frontShellLength / 2, 26, 0], [0, 0, Math.PI / 2], '#2a323b')
  addPart('front-shell-02', new THREE.TorusGeometry(10, 2.2, 12, 32), [-66 - frontShellLength / 2, 26, 0], [0, Math.PI / 2, 0], '#151b22')
  for (let index = 0; index < 4; index += 1) {
    addPart('front-shell-01', new RoundedBoxGeometry(7, 20, bodyDepth + 5, 2, 1.5), [-52 + index * 15, 30, 0], [0, 0, -0.3], '#252e38')
  }

  const gripAngle = THREE.MathUtils.degToRad(THREE.MathUtils.clamp(parameters.gripAngle, 5, 28))
  addPart('grip-shell-01', new RoundedBoxGeometry(43, 92, 35, 6, 7), [42, -43, 0], [0, 0, -gripAngle], '#303945', 0.42)
  addPart('grip-shell-01', new RoundedBoxGeometry(31, 74, 39, 5, 5), [46, -44, 0], [0, 0, -gripAngle], '#1e2731', 0.24)
  for (let index = 0; index < 6; index += 1) {
    addPart('grip-shell-01', new THREE.BoxGeometry(29, 2, 40.5), [42 + index * 2.1, -66 + index * 10, 0], [0, 0, -gripAngle], '#74808d')
  }
  addPart('storage-visual-01', new RoundedBoxGeometry(24, 70, 27, 4, 4), [43, -48, 0], [0, 0, -gripAngle], '#181f27')
  addPart('storage-visual-01', new RoundedBoxGeometry(31, 9, 34, 3, 3), [55, -86, 0], [0, 0, -gripAngle], '#323b46')

  addPart('core-shell-01', new THREE.TorusGeometry(18, 3.6, 12, 32, Math.PI * 1.55), [12, -11, 0], [0, 0, 0.4], '#38424d')
  addPart('core-shell-01', new RoundedBoxGeometry(7, 22, 7, 3, 1.5), [8, -7, 0], [0, 0, -0.2], '#1c232b')

  for (const [x, rotation] of [[43, -0.32], [59, -0.28], [75, -0.24]] as Array<[number, number]>) {
    addPart('core-shell-01', new RoundedBoxGeometry(7, 24, 2.4, 2, 1), [x, 27, bodyDepth / 2 + 1.2], [0, 0, rotation], '#ad3036', 0.45)
  }
  for (const x of [-27, 18, 82]) {
    addPart('core-shell-01', new THREE.CylinderGeometry(3.8, 3.8, 2.2, 24), [x, 7, bodyDepth / 2 + 1.2], [Math.PI / 2, 0, 0], '#202832')
  }

  const railStart = -32
  for (let index = 0; index < 10; index += 1) {
    addPart('top-accessory-01', new RoundedBoxGeometry(10, 5, bodyDepth + 5, 2, 1), [railStart + index * 13, 64, 0], [0, 0, -0.02], '#252e38')
  }
  for (let index = 0; index < 7; index += 1) {
    addPart('top-accessory-01', new RoundedBoxGeometry(9, 4, bodyDepth + 1, 2, 1), [-37 + index * 13, -1, 0], [0, 0, 0.02], '#202832')
  }
  addPart('top-accessory-02', new RoundedBoxGeometry(34, 20, 24, 4, 4), [34, 78, 0], [0, 0, -0.02], '#303946')
  addPart('top-accessory-02', new THREE.CylinderGeometry(8, 8, 26, 24), [15, 78, 0], [0, Math.PI / 2, 0], '#1d252e')
  addPart('top-accessory-02', new THREE.TorusGeometry(6, 1.5, 10, 24), [1, 78, 0], [0, Math.PI / 2, 0], '#cf4141')

  for (const x of [-34, -8, 76]) {
    addPart('core-shell-01', new RoundedBoxGeometry(12, 21, bodyDepth + 1, 3, 2), [x, 25, 0], [0, 0, -0.04], '#242c35')
  }

  group.rotation.set(-0.08, -0.3, 0)
  group.position.y = 20
  return group
}

function setCameraPosition(camera: THREE.PerspectiveCamera, view: CameraView) {
  const positions: Record<CameraView, [number, number, number]> = {
    iso: [155, 145, 420],
    front: [0, 45, 490],
    top: [0, 490, 0.01],
    right: [490, 45, 0],
  }
  camera.position.set(...positions[view])
}

function IconAction({ icon: Icon, label }: { icon: typeof Plus; label: string }) {
  return <button><Icon size={15} /><span>{label}</span></button>
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
