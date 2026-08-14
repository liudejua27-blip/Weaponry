import { useState, type ChangeEvent } from 'react'
import {
  ArrowLeft,
  ArrowRight,
  Check,
  CheckCircle,
  Cube,
  DownloadSimple,
  FolderOpen,
  GearSix,
  ImageSquare,
  Info,
  LockKey,
  MagicWand,
  PlayCircle,
  Plus,
  ShieldCheck,
  Sparkle,
  Stack,
  UploadSimple,
  WarningCircle,
} from '@phosphor-icons/react'
import { RuntimeViewer } from './features/runtime-viewer/RuntimeViewer'

export type ForgePage = 'home' | 'create' | 'workbench' | 'check' | 'export'

type PageProps = {
  onNavigate: (page: ForgePage) => void
  onNotice: (message: string) => void
}

const NAV_ITEMS: Array<[ForgePage, string]> = [
  ['home', '项目'],
  ['create', '创建模型'],
  ['workbench', '工作台'],
  ['check', '检查确认'],
  ['export', '导出'],
]

function ProductShell({ page, onNavigate, children }: { page: ForgePage; onNavigate: (page: ForgePage) => void; children: React.ReactNode }) {
  return <main className="product-shell">
    <header className="product-topbar">
      <button type="button" className="product-brand" onClick={() => onNavigate('home')} aria-label="返回 ForgeCAD 项目首页">
        <span className="product-brand-mark"><Cube size={18} weight="duotone" /></span>
        <strong>ForgeCAD</strong>
      </button>
      <span className="product-topbar-divider" aria-hidden="true" />
      <nav className="product-nav" aria-label="产品页面">
        {NAV_ITEMS.map(([target, label]) => <button type="button" key={target} className={page === target ? 'product-nav-active' : ''} onClick={() => onNavigate(target)}>{label}</button>)}
      </nav>
      <div className="product-topbar-spacer" />
      <span className="product-status"><span />等待 Codex / Runtime</span>
      <button type="button" className="product-icon-button" aria-label="设置" title="设置"><GearSix size={17} /></button>
    </header>
    {children}
  </main>
}

function HomePage({ onNavigate }: PageProps) {
  return <div className="home-page">
    <section className="home-hero">
      <div className="home-hero-copy">
        <div className="home-title-line"><span className="home-title-rule" /><span>本地可验证的 3D 工作台</span></div>
        <h1>让 Codex 负责复杂操作，<br /><span>你负责观察与确认。</span></h1>
        <p>上传一张你有权使用的参考图，告诉 Codex 你想要什么。ForgeCAD 把每次生成变成可观察、可比较、可回退的模型版本。</p>
        <div className="home-hero-actions">
          <button type="button" className="primary-button" onClick={() => onNavigate('create')}><Plus size={18} />新建模型</button>
          <button type="button" className="secondary-button" onClick={() => onNavigate('workbench')}><PlayCircle size={18} />进入工作台</button>
        </div>
        <div className="home-trust-line"><ShieldCheck size={15} />永久修改先经过准备、回读、质量检查和确认</div>
      </div>
      <div className="home-hero-visual" aria-label="ForgeCAD 工作台预览">
        <div className="hero-visual-toolbar"><span><span className="hero-live-dot" />Codex 控制状态</span><span>只读预览</span></div>
        <div className="hero-viewport">
          <div className="hero-grid" aria-hidden="true" />
          <div className="hero-orbit hero-orbit-one" aria-hidden="true" />
          <div className="hero-orbit hero-orbit-two" aria-hidden="true" />
          <div className="hero-model-mark"><Cube size={46} weight="duotone" /></div>
          <div className="hero-model-label"><span>当前视口</span><strong>等待 Runtime 候选</strong></div>
          <div className="hero-cursor-note"><Sparkle size={14} />语义操作会在这里可见</div>
        </div>
        <div className="hero-visual-footer"><span>场景树</span><span>3D Viewport</span><span>属性</span><span>版本</span></div>
      </div>
    </section>

    <section className="home-section">
      <div className="section-heading-row"><div><span className="section-kicker">你的项目</span><h2>继续工作</h2></div><button type="button" className="text-link" onClick={() => onNavigate('create')}>查看全部 <ArrowRight size={15} /></button></div>
      <div className="project-cards">
        <button type="button" className="project-card project-card-featured" onClick={() => onNavigate('workbench')}><div className="project-card-art project-card-art-robot"><Cube size={32} weight="duotone" /></div><div className="project-card-copy"><div><strong>Robot_A</strong><span>最近查看</span></div><p>等待 Codex 提交设计，当前没有 confirmed 版本。</p><small>工作台 · 只读状态</small></div><ArrowRight className="project-card-arrow" size={18} /></button>
        <button type="button" className="project-card" onClick={() => onNavigate('create')}><div className="project-card-art project-card-art-reference"><ImageSquare size={28} weight="duotone" /></div><div className="project-card-copy"><div><strong>新建模型</strong><span>从参考图开始</span></div><p>用三步准备参考图、需求和生成范围。</p><small>创建向导</small></div><ArrowRight className="project-card-arrow" size={18} /></button>
        <button type="button" className="project-card" onClick={() => onNavigate('check')}><div className="project-card-art project-card-art-check"><ShieldCheck size={28} weight="duotone" /></div><div className="project-card-copy"><div><strong>检查确认</strong><span>了解质量状态</span></div><p>先看外形、结构和材质是否有足够证据。</p><small>当前没有 candidate-bound 证据</small></div><ArrowRight className="project-card-arrow" size={18} /></button>
      </div>
    </section>

    <section className="home-principles">
      <div><span className="section-kicker">ForgeCAD 的工作方式</span><h2>三件事始终清楚</h2></div>
      <div className="principle-grid"><article><span>01</span><h3>你看到的是模型，不是日志</h3><p>Codex 的复杂调用留在操作详情里，主界面只显示人能理解的语义动作。</p></article><article><span>02</span><h3>每次修改都有版本</h3><p>失败不会覆盖旧版本；比较、恢复和确认都是明确的用户动作。</p></article><article><span>03</span><h3>未知就显示未知</h3><p>单张参考图不能证明背面和完整 360°，界面不会把推测包装成事实。</p></article></div>
    </section>
  </div>
}

function CreatePage({ onNavigate, onNotice }: PageProps) {
  const [step, setStep] = useState(1)
  const [referenceName, setReferenceName] = useState('尚未选择参考图')
  const [description, setDescription] = useState('根据图片生成一个可编辑的机器人模型，整体比例保持一致，装甲风格更简洁。')
  const [purpose, setPurpose] = useState('通用')
  const [precision, setPrecision] = useState('标准')
  const [coverage, setCoverage] = useState('仅正面可见部分')

  const chooseReference = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.currentTarget.files?.[0]
    if (file) setReferenceName(file.name)
  }

  const nextStep = () => setStep((value) => Math.min(3, value + 1))
  const previousStep = () => setStep((value) => Math.max(1, value - 1))

  return <div className="create-page">
    <div className="create-page-heading"><div><span className="section-kicker">新建模型</span><h1>用三步准备一次生成</h1><p>只收集当前真正需要的信息。详细属性可以在“我的信息”和工作台里继续补充。</p></div><div className="create-safety-note"><LockKey size={17} /><span>参考图只在你确认后进入本地 Runtime</span></div></div>
    <div className="wizard-layout">
      <section className="wizard-card">
        <div className="wizard-steps" aria-label="创建步骤">{[['参考图', '上传授权参考'], ['描述', '告诉 Codex 目标'], ['准备', '确认生成范围']].map(([label, detail], index) => { const number = index + 1; return <button type="button" key={label} className={`wizard-step ${step === number ? 'wizard-step-active' : ''} ${step > number ? 'wizard-step-done' : ''}`} onClick={() => setStep(number)}><span>{step > number ? <Check size={15} /> : number}</span><div><strong>{label}</strong><small>{detail}</small></div></button> })}</div>
        {step === 1 && <div className="wizard-content"><div className="wizard-content-heading"><span className="wizard-number">01</span><div><h2>你想从哪张图开始？</h2><p>拖入 PNG / JPG。单张图片无法证明背面和完整 360°，我们会把未知区域保留下来。</p></div></div><label className="upload-dropzone"><input type="file" accept="image/png,image/jpeg" onChange={chooseReference} /><span className="upload-icon"><UploadSimple size={25} /></span><strong>{referenceName === '尚未选择参考图' ? '选择参考图' : referenceName}</strong><small>{referenceName === '尚未选择参考图' ? 'PNG / JPG · 只读取本地文件名' : '参考图已选中，尚未提交给 Runtime'}</small></label><div className="observability-card"><div><span className="observability-dot observability-dot-seen" /><strong>正面</strong><small>已观察</small></div><div><span className="observability-dot observability-dot-partial" /><strong>侧面</strong><small>部分可见</small></div><div><span className="observability-dot observability-dot-unknown" /><strong>背面</strong><small>未知</small></div></div></div>}
        {step === 2 && <div className="wizard-content"><div className="wizard-content-heading"><span className="wizard-number">02</span><div><h2>用一句话描述目标</h2><p>你不需要知道 Operator、Skill 或几何参数；告诉 Codex 结果应该是什么。</p></div></div><textarea className="intent-input" value={description} onChange={(event) => setDescription(event.target.value)} aria-label="模型需求" /><div className="intent-templates"><span>快捷模板</span><button type="button" onClick={() => setDescription('做成适合游戏渲染的机械机器人，保持参考图的肩部比例。')}>游戏角色</button><button type="button" onClick={() => setDescription('生成一个结构清晰、方便继续修改的科幻机器人草模。')}>机械机器人</button><button type="button" onClick={() => setDescription('根据参考图做一个适合展示的影视概念模型。')}>影视概念模型</button></div></div>}
        {step === 3 && <div className="wizard-content"><div className="wizard-content-heading"><span className="wizard-number">03</span><div><h2>确认生成范围</h2><p>这些选项会影响 Codex 的计划，不会改变你的参考图事实边界。</p></div></div><div className="option-stack"><label><span>模型用途</span><select value={purpose} onChange={(event) => setPurpose(event.target.value)}><option>游戏</option><option>渲染</option><option>3D 打印</option><option>通用</option></select></label><label><span>模型精度</span><select value={precision} onChange={(event) => setPrecision(event.target.value)}><option>快速</option><option>标准</option><option>精细</option></select></label><label><span>生成范围</span><select value={coverage} onChange={(event) => setCoverage(event.target.value)}><option>仅正面可见部分</option><option>推测完整模型</option></select></label></div><div className="unknown-note"><Info size={17} /><p>当前选择“{coverage}”。背面和不可见结构会标记为推测或未知，不会自动变成已验证事实。</p></div></div>}
        <div className="wizard-footer"><button type="button" className="secondary-button" onClick={previousStep} disabled={step === 1}><ArrowLeft size={16} />上一步</button><span>第 {step} / 3 步</span>{step < 3 ? <button type="button" className="primary-button" onClick={nextStep}>下一步<ArrowRight size={16} /></button> : <button type="button" className="primary-button" onClick={() => { onNotice('创建向导已完成；等待 Codex / Runtime 接收准备请求。'); onNavigate('workbench') }}>准备生成<MagicWand size={17} /></button>}</div>
      </section>
      <aside className="wizard-summary"><div className="summary-heading"><span>准备摘要</span><CheckCircle size={17} /></div><div className="summary-reference"><span className="summary-thumbnail"><ImageSquare size={24} /></span><div><strong>{referenceName}</strong><small>授权状态：尚未提交</small></div></div><dl><div><dt>需求</dt><dd>{description.length > 42 ? `${description.slice(0, 42)}…` : description}</dd></div><div><dt>用途</dt><dd>{purpose}</dd></div><div><dt>精度</dt><dd>{precision}</dd></div><div><dt>范围</dt><dd>{coverage}</dd></div></dl><div className="summary-bottom"><WarningCircle size={16} /><span>生成前仍需由你确认。这里不会自动发送消息、上传文件或提交不可逆修改。</span></div></aside>
    </div>
  </div>
}

function CheckPage({ onNavigate, onNotice }: PageProps) {
  return <div className="check-page">
    <div className="check-heading"><div><span className="section-kicker">检查确认</span><h1>先判断证据，再决定下一步</h1><p>专业指标保留在详细证据里；普通界面只告诉你外形、结构和材质目前能不能继续。</p></div><button type="button" className="secondary-button" onClick={() => onNavigate('workbench')}><ArrowLeft size={16} />返回工作台</button></div>
    <div className="stage-timeline" aria-label="创作阶段"><span className="stage-timeline-active">参考图</span><i /><span>大体外形</span><i /><span>结构</span><i /><span>细节</span><i /><span>材质</span><i /><span>完成</span></div>
    <section className="check-layout"><div className="check-compare-card"><div className="check-card-header"><div><span className="section-kicker">当前候选</span><h2>与参考图对比</h2></div><span className="status-label status-label-warn">尚未运行</span></div><div className="check-compare-grid"><div className="check-canvas"><span>参考图</span><div><ImageSquare size={28} /><strong>等待 candidate-bound 参考图</strong><small>未读取任何授权图片</small></div></div><div className="check-canvas"><span>当前版本</span><div><Cube size={28} /><strong>等待 RenderSet</strong><small>Viewer 不生成质量结论</small></div></div></div><div className="check-compare-footer"><span>Camera lock · 512×512</span><span>Reference: —</span><span>RenderSet: —</span></div></div><aside className="check-result-card"><div className="check-card-header"><div><span className="section-kicker">用户可读结果</span><h2>三件事</h2></div><ShieldCheck size={20} /></div><div className="result-row"><span className="result-icon result-icon-warn"><WarningCircle size={17} /></span><div><strong>外形</strong><small>需要参考图和候选进行比较</small></div><span>未开始</span></div><div className="result-row"><span className="result-icon result-icon-lock"><LockKey size={17} /></span><div><strong>结构</strong><small>轮廓门通过后才会解锁</small></div><span>锁定</span></div><div className="result-row"><span className="result-icon result-icon-lock"><LockKey size={17} /></span><div><strong>材质</strong><small>当前阶段不提前承诺</small></div><span>未开始</span></div><div className="check-guidance"><Info size={16} /><p>发现问题后，Codex 只能基于 candidate-bound 证据生成下一轮修正意图；不会直接覆盖当前版本。</p></div><button type="button" className="primary-button primary-button-wide" onClick={() => onNotice('当前没有可安全生成的修正意图；请先让 Codex 提交候选和固定视图证据。')}>让 Codex 检查</button></aside></section>
  </div>
}

function ExportPage({ onNavigate, onNotice }: PageProps) {
  const [purpose, setPurpose] = useState('通用')
  const [format, setFormat] = useState('GLB')
  return <div className="export-page">
    <div className="export-heading"><div><span className="section-kicker">导出</span><h1>把已确认的版本带走</h1><p>导出只针对 Runtime 已确认的不可变版本。当前没有 confirmed head，因此按钮保持等待状态。</p></div><button type="button" className="secondary-button" onClick={() => onNavigate('workbench')}><ArrowLeft size={16} />返回工作台</button></div>
    <div className="export-layout"><section className="export-form-card"><div className="export-form-header"><span className="export-number">01</span><div><h2>选择用途</h2><p>用途只影响推荐的导出预设，不会改变模型真值。</p></div></div><div className="purpose-grid">{['游戏', '渲染', '3D 打印', '通用'].map((item) => <button type="button" key={item} className={purpose === item ? 'purpose-option purpose-option-active' : 'purpose-option'} onClick={() => setPurpose(item)}><span>{item === '游戏' ? <PlayCircle size={21} /> : item === '渲染' ? <Sparkle size={21} /> : item === '3D 打印' ? <Cube size={21} /> : <Stack size={21} />}</span><strong>{item}</strong><small>{item === '游戏' ? '实时引擎' : item === '渲染' ? '固定渲染' : item === '3D 打印' ? '实体检查' : '保留更多信息'}</small></button>)}</div><div className="export-form-header export-form-header-secondary"><span className="export-number">02</span><div><h2>导出格式</h2><p>格式选项会在确认版本和资产包可用时启用。</p></div></div><div className="format-select-row"><button type="button" className={format === 'GLB' ? 'format-option format-option-active' : 'format-option'} onClick={() => setFormat('GLB')}><strong>GLB</strong><span>推荐 · 当前 Viewer 可读</span></button><button type="button" className={format === 'OBJ' ? 'format-option format-option-active' : 'format-option'} onClick={() => setFormat('OBJ')}><strong>OBJ</strong><span>仅几何 · 材质可能分离</span></button></div></section><aside className="export-summary"><div className="summary-heading"><span>导出检查</span><DownloadSimple size={18} /></div><div className="export-target"><span className="export-target-icon"><FolderOpen size={22} /></span><div><strong>当前确认版本</strong><small>暂无 confirmed head</small></div></div><div className="export-checklist"><div><span className="checklist-dot checklist-dot-warn" /><span>不可变版本</span><strong>等待</strong></div><div><span className="checklist-dot checklist-dot-warn" /><span>GLB 回读</span><strong>等待</strong></div><div><span className="checklist-dot checklist-dot-lock" /><span>材质与许可证</span><strong>锁定</strong></div></div><div className="export-selected"><span>用途</span><strong>{purpose}</strong><span>格式</span><strong>{format}</strong></div><button type="button" className="primary-button primary-button-wide" onClick={() => onNotice('导出尚未启动：需要先由 Codex 确认一个不可变版本。')} disabled><DownloadSimple size={17} />请求导出</button><small className="export-footnote">导出会保留 license / provenance；不会把本机路径或 secret 写入包内。</small></aside></div>
  </div>
}

export default function App() {
  const [page, setPage] = useState<ForgePage>('home')
  const [notice, setNotice] = useState<string | null>(null)

  const onNavigate = (nextPage: ForgePage) => {
    setNotice(null)
    setPage(nextPage)
  }

  if (page === 'workbench') return <RuntimeViewer onNavigate={onNavigate} />

  return <ProductShell page={page} onNavigate={onNavigate}>
    {page === 'home' && <HomePage onNavigate={onNavigate} onNotice={setNotice} />}
    {page === 'create' && <CreatePage onNavigate={onNavigate} onNotice={setNotice} />}
    {page === 'check' && <CheckPage onNavigate={onNavigate} onNotice={setNotice} />}
    {page === 'export' && <ExportPage onNavigate={onNavigate} onNotice={setNotice} />}
    {notice && <div className="product-notice" role="status"><Info size={16} /><span>{notice}</span><button type="button" aria-label="关闭提示" onClick={() => setNotice(null)}>关闭</button></div>}
  </ProductShell>
}
