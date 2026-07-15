import {
  ArrowsClockwise,
  ArrowsOutCardinal,
  ChartLineUp,
  Crosshair,
  Cube,
  GridFour,
  Funnel,
  MagnifyingGlass,
  Ruler,
  SelectionAll,
  Star,
} from '@phosphor-icons/react'
import type { KeyboardEvent as ReactKeyboardEvent, PointerEvent as ReactPointerEvent, RefObject } from 'react'
import type { ModuleAssetRecord } from '../../shared/types'

export type ModuleCategory = ModuleAssetRecord['manifest']['category']
export type ComponentDrawerMode = 'recommended' | 'all'
export type ReviewStatus = 'draft' | 'pending_review' | 'approved' | 'restricted'
export type QualityStatus = 'passed' | 'warning' | 'failed' | 'unavailable'
export type ComponentFilter = ComponentCategory | 'installed' | 'compatible' | 'favorites' | 'recent'
export type ComponentCategory = 'all' | ModuleCategory

export const MODULE_CATEGORY_LABELS: Record<ModuleCategory, string> = {
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

export const COMPONENT_CATEGORIES: Array<{ id: ComponentFilter; label: string }> = [
  { id: 'all', label: '全部组件' },
  { id: 'installed', label: '当前装配' },
  { id: 'compatible', label: '可替换' },
  { id: 'favorites', label: '收藏' },
  { id: 'recent', label: '最近使用' },
  ...Object.entries(MODULE_CATEGORY_LABELS).map(([id, label]) => ({
    id: id as ModuleCategory,
    label,
  })),
]

export const REVIEW_STATUS_LABELS: Record<ReviewStatus, string> = {
  draft: '草稿',
  pending_review: '待审',
  approved: '已批准',
  restricted: '受限',
}

export const ORIGIN_CLAIM_LABELS = {
  self_declared_original: '本人原创声明',
  third_party: '第三方来源',
  unknown: '来源待补充',
} as const

export const QUALITY_STATUS_LABELS: Record<QualityStatus, string> = {
  passed: '通过',
  warning: '警告',
  failed: '失败',
  unavailable: '未检查',
}

type GraphNodeSummary = {
  node_id: string
  module_id: string
  locked?: boolean
}

export type ComponentDrawerProps = {
  mode: ComponentDrawerMode
  selectedModuleLabel: string
  componentCategory: ComponentFilter
  reviewStatusFilter: ReviewStatus | ''
  categories: Array<{ id: ComponentFilter; label: string }>
  filterCounts: Record<'all' | 'installed' | 'compatible' | 'favorites' | 'recent', number>
  query: string
  displayedComponents: ModuleAssetRecord[]
  totalModuleCount: number
  selectedLibraryModule: ModuleAssetRecord | null
  selectedLibraryModuleId: string
  selectedNode: GraphNodeSummary | null
  selectedModuleCategory: ModuleCategory | null
  graphNodes: GraphNodeSummary[]
  favoriteModuleIds: string[]
  thumbnailFailures: ReadonlySet<string>
  drawerRef?: RefObject<HTMLElement | null>
  canReplaceSelected: boolean
  expanded: boolean
  loading: boolean
  legacyDesignReadOnly: boolean
  agentAssetActive: boolean
  qualityStatusFor: (moduleId: string) => QualityStatus
  onResizeStart: (event: ReactPointerEvent<HTMLDivElement>) => void
  onResizeKeyDown?: (event: ReactKeyboardEvent<HTMLDivElement>) => void
  onCategoryChange: (category: ComponentFilter) => void
  onReviewStatusChange: (status: ReviewStatus | '') => void
  onQueryChange: (query: string) => void
  onModeToggle: () => void
  onClose: () => void
  onSelectModule: (module: ModuleAssetRecord) => void
  onToggleFavorite: (moduleId: string) => void
  onThumbnailError: (moduleId: string) => void
  onLocateModule: (module: ModuleAssetRecord) => void
  onPreviewReplace: () => void
  onDiscardReplacement: () => void
  onConfirmReplacement: () => void
  thumbnailUrl: (moduleId: string) => string
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

export function ComponentDrawer({
  mode,
  selectedModuleLabel,
  componentCategory,
  reviewStatusFilter,
  categories,
  filterCounts,
  query,
  displayedComponents,
  totalModuleCount,
  selectedLibraryModule,
  selectedLibraryModuleId,
  selectedNode,
  selectedModuleCategory,
  graphNodes,
  favoriteModuleIds,
  thumbnailFailures,
  drawerRef,
  canReplaceSelected,
  expanded,
  loading,
  legacyDesignReadOnly,
  agentAssetActive,
  qualityStatusFor,
  onResizeStart,
  onResizeKeyDown,
  onCategoryChange,
  onReviewStatusChange,
  onQueryChange,
  onModeToggle,
  onClose,
  onSelectModule,
  onToggleFavorite,
  onThumbnailError,
  onLocateModule,
  onPreviewReplace,
  onDiscardReplacement,
  onConfirmReplacement,
  thumbnailUrl,
}: ComponentDrawerProps) {
  return (
    <section
      ref={drawerRef}
      className={`component-library contextual-library ${mode === 'all' ? 'expanded' : ''}`}
      role="dialog"
      aria-modal="false"
      aria-label={mode === 'recommended' ? `替换${selectedModuleLabel}` : '组件库'}
      data-forgecad-drawer="component"
      tabIndex={-1}
    >
      <div
        className="component-library-resize-handle"
        onPointerDown={onResizeStart}
        onKeyDown={onResizeKeyDown}
        role="separator"
        aria-orientation="horizontal"
        aria-label="调整组件库高度"
        tabIndex={0}
      />
      <div className="component-library-header">
        <div className="component-library-title">
          <Cube size={15} weight="duotone" />
          {mode === 'recommended' ? `替换「${selectedModuleLabel}」` : '选择展示组件'}
        </div>
        <div className="component-library-header-actions">
          {mode === 'all' && <>
            <select
              className="component-status-filter"
              aria-label="部件审阅状态"
              value={reviewStatusFilter}
              onChange={(event) => onReviewStatusChange(event.target.value as ReviewStatus | '')}
            >
              <option value="">全部状态</option>
              {Object.entries(REVIEW_STATUS_LABELS).map(([status, label]) => (
                <option key={status} value={status}>{label}</option>
              ))}
            </select>
            <div className="component-search">
              <MagnifyingGlass size={15} />
              <input value={query} onChange={(event) => onQueryChange(event.target.value)} placeholder="搜索名称、描述或标签…" aria-label="搜索组件" />
              <Funnel size={14} />
            </div>
          </>}
          <button type="button" className="component-drawer-toggle text" onClick={onModeToggle} aria-label={mode === 'recommended' ? '查看更多组件' : '返回推荐组件'}>
            {mode === 'recommended' ? '查看更多' : '返回推荐'}
          </button>
          <button type="button" className="component-drawer-toggle" data-dialog-initial-focus="true" onClick={onClose} aria-label="关闭组件选择">×</button>
        </div>
      </div>
      <div className="component-library-body">
        {mode === 'all' && <nav className="component-categories">
          {categories.map((category) => (
            <button type="button" key={category.id} className={componentCategory === category.id ? 'active' : ''} aria-pressed={componentCategory === category.id} onClick={() => onCategoryChange(category.id)}>
              <span>{category.label}</span>
              <small>{category.id in filterCounts ? filterCounts[category.id as keyof typeof filterCounts] : displayedComponents.filter((item) => item.manifest.category === category.id).length}</small>
            </button>
          ))}
        </nav>}
        <div className="component-library-content">
          <div className="module-replace-bar">
            <span>{mode === 'recommended' ? '只显示当前部件可用的替换建议' : '选择一个组件后，可先预览再决定保留'}</span>
            <span className="component-result-count">显示 {displayedComponents.length} / {totalModuleCount}</span>
            <button
              type="button"
              onClick={onPreviewReplace}
              disabled={!canReplaceSelected || loading || legacyDesignReadOnly || agentAssetActive}
              title={agentAssetActive ? 'Agent 资产请在分件候选中替换' : '通过 ChangeSet preview/confirm 创建子版本'}
            >预览替换</button>
          </div>
          <div className="component-grid">
            {displayedComponents.map((component) => {
              const ComponentIcon = componentIconFor(component.manifest.category)
              const graphNode = graphNodes.find((node) => node.module_id === component.manifest.module_id)
              const isActiveNode = Boolean(graphNode && selectedNode?.node_id === graphNode.node_id)
              const isCandidate = selectedLibraryModuleId === component.manifest.module_id
              const isInstalled = Boolean(graphNode)
              const compatible = Boolean(selectedNode && !selectedNode.locked && selectedModuleCategory === component.manifest.category)
              const metadata = component.catalog_metadata
              const reviewStatus = metadata.review_status as ReviewStatus
              const qualityStatus = qualityStatusFor(component.manifest.module_id)
              const thumbnailFailed = thumbnailFailures.has(component.manifest.module_id)
              return (
                <button
                  key={component.manifest.module_id}
                  type="button"
                  aria-label={`选择组件 ${metadata.display_name}`}
                  aria-pressed={isCandidate}
                  className={`component-card ${isActiveNode ? 'active' : ''} ${isCandidate ? 'candidate' : ''}`.trim()}
                  draggable
                  onDragStart={(event) => {
                    event.dataTransfer.effectAllowed = 'copy'
                    event.dataTransfer.setData('application/x-forgecad-module-id', component.manifest.module_id)
                    event.dataTransfer.setData('text/plain', component.manifest.module_id)
                  }}
                  onClick={() => onSelectModule(component)}
                >
                  <span className="component-visual">
                    {!thumbnailFailed && <img src={thumbnailUrl(component.manifest.module_id)} alt={`${component.manifest.module_id} 模块缩略图`} onError={() => onThumbnailError(component.manifest.module_id)} />}
                    <span className="component-icon-fallback" hidden={!thumbnailFailed}><ComponentIcon size={34} weight="duotone" /></span>
                    <span className={`component-state ${reviewStatus}`}>{REVIEW_STATUS_LABELS[reviewStatus]}</span>
                  </span>
                  <strong>{metadata.display_name}</strong>
                  <small>{MODULE_CATEGORY_LABELS[component.manifest.category]} · {component.manifest.triangle_count.toLocaleString()} tris · {(component.manifest.connectors ?? []).length} 接口</small>
                  <span className="component-card-activity">{isActiveNode ? '当前节点' : isCandidate ? '替换候选' : isInstalled ? '已装配' : compatible ? '可替换' : QUALITY_STATUS_LABELS[qualityStatus]}</span>
                </button>
              )
            })}
            {displayedComponents.length === 0 && <div className="component-empty"><strong>暂时没有可直接替换的组件</strong><span>当前组件库没有同类、兼容且可用的展示组件；你可以继续用 Agent 调整，或稍后添加原创组件。</span></div>}
          </div>
        </div>
        {expanded && selectedLibraryModule && (
          <aside className="component-inspector" data-testid="component-inspector">
            <div className="component-inspector-visual">
              {!thumbnailFailures.has(selectedLibraryModule.manifest.module_id) && <img src={thumbnailUrl(selectedLibraryModule.manifest.module_id)} alt={`${selectedLibraryModule.catalog_metadata.display_name} 预览`} onError={() => onThumbnailError(selectedLibraryModule.manifest.module_id)} />}
              <span>{selectedLibraryModule.catalog_metadata.catalog_path}</span>
            </div>
            <div className="component-inspector-heading">
              <div><strong>{selectedLibraryModule.catalog_metadata.display_name}</strong><span>{selectedLibraryModule.manifest.module_id}</span></div>
              <button type="button" className={favoriteModuleIds.includes(selectedLibraryModule.manifest.module_id) ? 'active' : ''} onClick={() => onToggleFavorite(selectedLibraryModule.manifest.module_id)} aria-label="切换组件收藏" title="切换组件收藏">
                <Star size={16} weight={favoriteModuleIds.includes(selectedLibraryModule.manifest.module_id) ? 'fill' : 'regular'} />
              </button>
            </div>
            <p>{selectedLibraryModule.catalog_metadata.description}</p>
            <div className="component-inspector-statuses">
              <span className={`review-status ${selectedLibraryModule.catalog_metadata.review_status}`}>{REVIEW_STATUS_LABELS[selectedLibraryModule.catalog_metadata.review_status as ReviewStatus]}</span>
              <span className={`quality-status ${qualityStatusFor(selectedLibraryModule.manifest.module_id)}`}>质量：{QUALITY_STATUS_LABELS[qualityStatusFor(selectedLibraryModule.manifest.module_id)]}</span>
            </div>
            <dl className="component-spec-list">
              <div><dt>尺寸</dt><dd>{selectedLibraryModule.manifest.bounds_mm.map((value) => `${value} mm`).join(' × ')}</dd></div>
              <div><dt>几何</dt><dd>{selectedLibraryModule.manifest.triangle_count.toLocaleString()} tris · {selectedLibraryModule.manifest.material_slots.length} 材质槽</dd></div>
              <div><dt>连接器</dt><dd>{(selectedLibraryModule.manifest.connectors ?? []).length} 个 · {(selectedLibraryModule.manifest.connectors ?? []).map((item) => item.slot).join('、') || '无'}</dd></div>
              <div><dt>适配</dt><dd>{canReplaceSelected ? '可替换当前节点' : selectedNode?.locked ? '当前节点已锁定' : '选择同分类节点后验证'}</dd></div>
              <div><dt>来源</dt><dd>{ORIGIN_CLAIM_LABELS[selectedLibraryModule.catalog_metadata.origin_claim ?? 'unknown']}</dd></div>
              <div><dt>审阅</dt><dd>{selectedLibraryModule.catalog_metadata.reviewer_name ? `${selectedLibraryModule.catalog_metadata.reviewer_name} · ${selectedLibraryModule.catalog_metadata.reviewed_at ? `已记录 ${selectedLibraryModule.catalog_metadata.reviewed_at}` : '已指派，待完成'}` : '等待独立审阅'}</dd></div>
            </dl>
            {(selectedLibraryModule.catalog_metadata.tags ?? []).length > 0 && <div className="component-tags">{(selectedLibraryModule.catalog_metadata.tags ?? []).map((tag) => <span key={tag}>#{tag}</span>)}</div>}
            <div className="component-inspector-actions">
              <button type="button" onClick={() => onLocateModule(selectedLibraryModule)}>{graphNodes.some((node) => node.module_id === selectedLibraryModule.manifest.module_id) ? '定位主视图' : '设置替换候选'}</button>
              <button type="button" className="primary" disabled={!canReplaceSelected || loading || legacyDesignReadOnly || agentAssetActive} onClick={onPreviewReplace}>预览替换</button>
            </div>
            {selectedLibraryModule && <div className="component-replacement-preview" data-testid="component-replacement-preview">
              <span>幽灵预览由父层控制；确认后才会创建新版本。</span>
              <button type="button" onClick={onDiscardReplacement} disabled={loading || legacyDesignReadOnly}>放弃</button>
              <button type="button" className="confirm" onClick={onConfirmReplacement} disabled={loading || legacyDesignReadOnly}>确认并创建新版本</button>
            </div>}
          </aside>
        )}
      </div>
    </section>
  )
}
