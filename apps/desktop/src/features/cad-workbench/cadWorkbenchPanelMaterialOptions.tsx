import { MaterialDrawer } from './MaterialDrawer'
import type { ReactElement } from 'react'
import type { AgentMaterialPreset } from '../../shared/types'

type CadWorkbenchPanelMaterialOptionsProps = {
  open: boolean
  hasShapeProgram: boolean
  isExternalGlbReference: boolean
  materialPresets: readonly AgentMaterialPreset[]
  quickMaterialPresets: readonly AgentMaterialPreset[]
  appearanceMaterialId: string
  selectedPartLabel: string
  selectedMaterialZoneId: string
  hasAgentAssetVersion: boolean
  quickMaterialDisabled: boolean
  activeMaterialDomain: string | null
  compatibilityOnly: boolean
  materialQuery: string
  materialCategory: AgentMaterialPreset['category'] | 'all'
  catalogLoading: boolean
  catalogMessage: string | null
  selectedMaterialZoneIds: readonly string[]
  materialEditorDisabled: boolean
  onQuickMaterialPreset: (materialId: string, materialName: string) => void
  onMaterialChange: (materialId: string) => void
  onMaterialZoneChange: (zoneId: string) => void
  onMaterialCompatibilityChange: (value: boolean) => void
  onMaterialQueryChange: (query: string) => void
  onMaterialCategoryChange: (category: AgentMaterialPreset['category'] | 'all') => void
  onCatalogMaterialPreview: (materialId: string, materialName: string) => void
  onCatalogMaterialPreviewNote: (materialName: string) => void
}

export function CadWorkbenchPanelMaterialOptions({
  open,
  hasShapeProgram,
  isExternalGlbReference,
  materialPresets,
  quickMaterialPresets,
  appearanceMaterialId,
  selectedPartLabel,
  selectedMaterialZoneId,
  hasAgentAssetVersion,
  quickMaterialDisabled,
  activeMaterialDomain,
  compatibilityOnly,
  materialQuery,
  materialCategory,
  catalogLoading,
  catalogMessage,
  selectedMaterialZoneIds,
  materialEditorDisabled,
  onQuickMaterialPreset,
  onMaterialChange,
  onMaterialZoneChange,
  onMaterialCompatibilityChange,
  onMaterialQueryChange,
  onMaterialCategoryChange,
  onCatalogMaterialPreview,
  onCatalogMaterialPreviewNote,
}: CadWorkbenchPanelMaterialOptionsProps): ReactElement | null {
  if (!open || !hasShapeProgram || isExternalGlbReference || materialPresets.length === 0) return null

  const selectedZoneLabel = selectedMaterialZoneId
    ? `材质区 ${selectedMaterialZoneId}`
    : '主材质区'

  return (
    <div className="agent-material-preview" aria-label="视觉材质目录">
      <div className="assistant-directions-heading">
        <span>换一个视觉材质</span>
        <small>{hasAgentAssetVersion ? '先预览，再确认版本' : '只影响当前预览'}</small>
      </div>
      <div className="agent-material-preview-list">
        {quickMaterialPresets.map((preset) => (
          <button
            key={preset.material_id}
            type="button"
            className={appearanceMaterialId === preset.material_id ? 'active' : ''}
            onClick={() => {
              onQuickMaterialPreset(preset.material_id, preset.display_name)
            }}
            disabled={quickMaterialDisabled}
          >
            {preset.display_name}
          </button>
        ))}
      </div>
      <details className="agent-material-catalog-details" data-testid="agent-material-catalog">
        <summary>全部 {materialPresets.length} 项材质、分类与材质区</summary>
        <MaterialDrawer
          materialPresets={materialPresets}
          selectedMaterialId={appearanceMaterialId}
          selectedPartLabel={selectedPartLabel}
          selectedZoneLabel={selectedZoneLabel}
          materialZoneIds={selectedMaterialZoneIds}
          selectedZoneId={selectedMaterialZoneId}
          activeDomain={activeMaterialDomain}
          compatibilityOnly={compatibilityOnly}
          query={materialQuery}
          category={materialCategory}
          catalogLoading={catalogLoading}
          catalogMessage={catalogMessage}
          disabled={materialEditorDisabled}
          onMaterialChange={onMaterialChange}
          onZoneChange={onMaterialZoneChange}
          onCompatibilityChange={onMaterialCompatibilityChange}
          onQueryChange={onMaterialQueryChange}
          onCategoryChange={onMaterialCategoryChange}
          onPreviewMaterial={(preset) => onCatalogMaterialPreview(preset.material_id, preset.display_name)}
          onPreviewNote={(preset) => onCatalogMaterialPreviewNote(preset.display_name)}
        />
      </details>
    </div>
  )
}
