import type { AgentMaterialPreset } from '../../shared/types'

const CATEGORY_LABELS: Record<AgentMaterialPreset['category'], string> = {
  metal: '金属',
  polymer: '塑料',
  rubber: '橡胶',
  composite: '复合材料',
  glass: '玻璃',
  coating: '涂层',
  natural: '自然材质',
  emissive: '发光',
}

const CATEGORY_ORDER: Array<AgentMaterialPreset['category'] | 'all'> = [
  'all', 'metal', 'polymer', 'rubber', 'composite', 'glass', 'coating',
]

export type MaterialDrawerProps = {
  materialPresets: AgentMaterialPreset[]
  selectedMaterialId: string
  detailDensity?: number
  showDetailDensity?: boolean
  selectedPartLabel?: string
  selectedZoneLabel?: string
  materialZoneIds?: string[]
  selectedZoneId?: string
  activeDomain?: string | null
  compatibilityOnly?: boolean
  query?: string
  category?: AgentMaterialPreset['category'] | 'all'
  catalogLoading?: boolean
  catalogMessage?: string | null
  disabled?: boolean
  onMaterialChange: (materialId: string) => void
  onDetailDensityChange?: (value: number) => void
  onPreviewNote?: (preset: AgentMaterialPreset) => void
  onPreviewMaterial?: (preset: AgentMaterialPreset, zoneId: string) => void
  onZoneChange?: (zoneId: string) => void
  onCompatibilityChange?: (value: boolean) => void
  onQueryChange?: (value: string) => void
  onCategoryChange?: (value: AgentMaterialPreset['category'] | 'all') => void
}

/**
 * Appearance-only material controls. It deliberately does not create a
 * ChangeSet: the parent decides whether the current stage is a preview or an
 * editable Agent asset and routes confirmed edits through the authoritative
 * Snapshot.
 */
export function MaterialDrawer({
  materialPresets,
  selectedMaterialId,
  detailDensity,
  showDetailDensity = false,
  disabled = false,
  onMaterialChange,
  onDetailDensityChange,
  onPreviewNote,
  selectedPartLabel = '当前选中部件',
  selectedZoneLabel = '主材质区',
  materialZoneIds = [],
  selectedZoneId = materialZoneIds[0] ?? '',
  activeDomain = null,
  compatibilityOnly = true,
  query = '',
  category = 'all',
  catalogLoading = false,
  catalogMessage = null,
  onPreviewMaterial,
  onZoneChange,
  onCompatibilityChange,
  onQueryChange,
  onCategoryChange,
}: MaterialDrawerProps) {
  const normalizedQuery = query.trim().toLocaleLowerCase()
  const filteredPresets = materialPresets.filter((preset) => {
    if (category !== 'all' && preset.category !== category) return false
    const compatible = Boolean(activeDomain && preset.allowed_domains.includes(activeDomain))
    if (compatibilityOnly && preset.material_id !== selectedMaterialId && !compatible) return false
    if (!normalizedQuery) return true
    return [preset.display_name, preset.category, ...(preset.visual_tags ?? [])]
      .join(' ')
      .toLocaleLowerCase()
      .includes(normalizedQuery)
  })
  const selectedPreset = materialPresets.find((preset) => preset.material_id === selectedMaterialId)

  const textureState = (preset: AgentMaterialPreset): { label: string; detail: string } => {
    const textures = preset.texture_summary ?? []
    if (textures.some((texture) => texture.exists)) {
      return { label: '纹理已登记', detail: `${textures.filter((texture) => texture.exists).length} 个受控对象` }
    }
    if (textures.length > 0) return { label: '使用参数外观', detail: '纹理对象不可用，已安全回退' }
    if (preset.thumbnail_fallback === 'unavailable') return { label: '无缩略图', detail: '仍可使用参数外观' }
    return { label: '参数外观', detail: '无需纹理文件' }
  }

  const provenanceLabel = (preset: AgentMaterialPreset): string => {
    if (preset.license === 'third_party') return '第三方参考'
    if (preset.license === 'self_declared_original') return '本人原创声明'
    if (preset.source === 'user_created') return '用户创建'
    if (preset.source === 'imported_reference') return '导入参考'
    if (preset.provenance === 'forgecad_builtin') return 'ForgeCAD 内置'
    return '来源待补充'
  }

  const selectedCompatible = Boolean(activeDomain && selectedPreset?.allowed_domains.includes(activeDomain))

  const zoneLabel = (zoneId: string): string => {
    const known: Record<string, string> = {
      primary: '主材质区',
      secondary: '次材质区',
      accent: '强调色区',
      transparent: '透明区',
      emissive: '发光区',
      rubber: '橡胶区',
      interior: '内部区',
      trim: '饰条区',
    }
    const suffix = zoneId.startsWith('zone_') ? zoneId.slice('zone_'.length) : zoneId
    return known[suffix] ?? `材质区 ${suffix.replaceAll('_', ' ')}`
  }

  return (
    <div className="material-drawer" data-testid="material-drawer">
      <div className="material-zone-context" aria-label="当前材质区">
        <strong>{selectedPartLabel}</strong>
        <span>{selectedZoneLabel} · 只改变外观</span>
      </div>
      {(catalogLoading || catalogMessage) && (
        <p className="material-catalog-status" aria-live="polite">
          {catalogLoading ? '正在更新视觉材质目录…' : catalogMessage}
        </p>
      )}
      {materialZoneIds.length > 0 && (
        <div className="material-zone-filter" aria-label="选择材质区">
          <span>材质区</span>
          <div>
            {materialZoneIds.map((zoneId) => (
              <button
                key={zoneId}
                type="button"
                className={selectedZoneId === zoneId ? 'active' : ''}
                aria-pressed={selectedZoneId === zoneId}
                disabled={disabled}
                onClick={() => onZoneChange?.(zoneId)}
              >
                {zoneLabel(zoneId)}
              </button>
            ))}
          </div>
        </div>
      )}
      <label className="wide-field material-search-field">
        <span>搜索视觉材质</span>
        <input
          aria-label="搜索视觉材质"
          type="search"
          placeholder="名称或标签"
          value={query}
          disabled={disabled}
          onChange={(event) => onQueryChange?.(event.target.value)}
        />
      </label>
      <div className="material-category-filter" aria-label="材质分类">
        {CATEGORY_ORDER.map((item) => (
          <button
            key={item}
            type="button"
            className={category === item ? 'active' : ''}
            aria-pressed={category === item}
            disabled={disabled}
            onClick={() => onCategoryChange?.(item)}
          >
            {item === 'all' ? '全部' : CATEGORY_LABELS[item]}
          </button>
        ))}
      </div>
      <div className="material-compatibility-filter" aria-label="材质适配范围">
        <button
          type="button"
          className={compatibilityOnly ? 'active' : ''}
          aria-pressed={compatibilityOnly}
          disabled={!activeDomain || disabled}
          onClick={() => onCompatibilityChange?.(true)}
        >适合当前设计</button>
        <button
          type="button"
          className={!compatibilityOnly ? 'active' : ''}
          aria-pressed={!compatibilityOnly}
          disabled={disabled}
          onClick={() => onCompatibilityChange?.(false)}
        >全部视觉材质</button>
        {!activeDomain && <small>领域尚未确认，暂不判断适配性</small>}
      </div>
      <label className="wide-field">
        <span>视觉材质</span>
        <select value={selectedMaterialId} disabled={disabled} onChange={(event) => onMaterialChange(event.target.value)}>
          {filteredPresets.map((preset) => <option key={preset.material_id} value={preset.material_id}>{preset.display_name}</option>)}
        </select>
      </label>
      {showDetailDensity && typeof detailDensity === 'number' && onDetailDensityChange && (
        <label className="property-number">
          <span>细节密度</span>
          <input aria-label="细节密度百分比" type="number" min="0" max="100" value={detailDensity} disabled={disabled} onChange={(event) => onDetailDensityChange(Number(event.target.value))} />
          <small>%</small>
        </label>
      )}
      {selectedPreset && (
        <div className="material-selection-summary" aria-live="polite">
          <div className="material-swatch" style={{ backgroundColor: selectedPreset.pbr.base_color }} aria-hidden="true" />
          <div>
            <strong>{selectedPreset.display_name}</strong>
            <small>{textureState(selectedPreset).label} · {provenanceLabel(selectedPreset)}{activeDomain && !selectedCompatible ? ' · 当前领域未标为适配' : ''}</small>
          </div>
        </div>
      )}
      <div className="appearance-material-list" aria-label="材质预设列表">
        {filteredPresets.map((preset) => (
          <div
            key={preset.material_id}
            className={selectedMaterialId === preset.material_id ? 'active' : ''}
          >
            <button
              type="button"
              className="material-card-select"
              aria-pressed={selectedMaterialId === preset.material_id}
              aria-label={`选择视觉材质 ${preset.display_name}`}
              disabled={disabled}
              onClick={() => {
                onMaterialChange(preset.material_id)
                onPreviewNote?.(preset)
              }}
            >
              <span className="material-card-title"><i className="material-swatch" style={{ backgroundColor: preset.pbr.base_color }} aria-hidden="true" />{preset.display_name}</span>
              <small>{CATEGORY_LABELS[preset.category]} · {textureState(preset).label}</small>
              <small className="material-card-detail">{textureState(preset).detail} · {provenanceLabel(preset)}</small>
            </button>
            <button
              type="button"
              className="material-card-preview"
              disabled={disabled || !selectedZoneId}
              onClick={() => onPreviewMaterial?.(preset, selectedZoneId)}
            >预览材质</button>
          </div>
        ))}
        {filteredPresets.length === 0 && <p className="material-empty">没有匹配的视觉材质，试试名称或分类。</p>}
      </div>
      <small className="material-boundary">材质只描述外观，不代表强度、重量或制造结论。</small>
    </div>
  )
}
