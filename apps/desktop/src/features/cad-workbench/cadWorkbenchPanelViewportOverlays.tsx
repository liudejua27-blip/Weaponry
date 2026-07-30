import { ArrowsOutCardinal, Crosshair, Cube, GridFour, House } from '@phosphor-icons/react'
import type { ComponentType, ReactElement } from 'react'
import type { CameraView, LightPreset } from './cadWorkbenchPanelTools'
import type { ViewportMeasurementMode } from './viewportMeasurementPresentation'
import type { ViewportTool } from './viewportDisplayPreferencesState'

type CadWorkbenchPanelViewportAnnotation = {
  id: string
  readoutText: string
}

type CadWorkbenchPanelViewportOverlaysProps = {
  activeTool: ViewportTool
  ghostPreview: boolean
  measurementMode: ViewportMeasurementMode
  measurementPrompt: string
  measurementReadoutText: string | null
  viewportReadoutText: string
  measurementAnnotations: readonly CadWorkbenchPanelViewportAnnotation[]
  onMeasurementModeChange: (next: ViewportMeasurementMode) => void
  onPinMeasurement: () => void
  onClearMeasurements: () => void
  cameraView: CameraView
  lightPreset: LightPreset
  explodeFactor: number
  onCameraViewChange: (next: CameraView) => void
  onLightPresetChange: (next: LightPreset) => void
  onToggleExplode: () => void
}

const MEASURE_MODE_DISTANCE_TEXT = '点到点'
const MEASURE_MODE_ANGLE_TEXT = '法线夹角'
const VIEWBAR_LIGHT_OPTIONS = [
  { value: 'cad_neutral' as const, label: 'CAD 中性' },
  { value: 'soft_studio' as const, label: '柔和棚拍' },
  { value: 'concept_contrast' as const, label: '概念对比' },
] as const

export function CadWorkbenchPanelViewportOverlays({
  activeTool,
  ghostPreview,
  measurementMode,
  measurementPrompt,
  measurementReadoutText,
  viewportReadoutText,
  measurementAnnotations,
  onMeasurementModeChange,
  onPinMeasurement,
  onClearMeasurements,
  cameraView,
  lightPreset,
  explodeFactor,
  onCameraViewChange,
  onLightPresetChange,
  onToggleExplode,
}: CadWorkbenchPanelViewportOverlaysProps): ReactElement {
  const measurementLine = measurementPrompt.length > 0
    ? measurementPrompt
    : measurementReadoutText
      ? `测量：${measurementReadoutText}`
      : '单位：mm'
  return (
    <>
      {activeTool === 'measure' ? (
        <div className="measurement-overlay" data-testid="measurement-overlay" role="status" aria-live="polite">
          <strong>测量</strong>
          <div className="measurement-mode-toggle" aria-label="测量模式">
            <button
              type="button"
              className={measurementMode === 'distance' ? 'active' : ''}
              aria-pressed={measurementMode === 'distance'}
              onClick={() => onMeasurementModeChange('distance')}
            >{MEASURE_MODE_DISTANCE_TEXT}</button>
            <button
              type="button"
              className={measurementMode === 'normal_angle' ? 'active' : ''}
              aria-pressed={measurementMode === 'normal_angle'}
              onClick={() => onMeasurementModeChange('normal_angle')}
            >{MEASURE_MODE_ANGLE_TEXT}</button>
          </div>
          <span>{measurementLine}</span>
          {measurementReadoutText ? <button type="button" onClick={onPinMeasurement}>固定标注</button> : null}
          <button type="button" onClick={onClearMeasurements}>清除</button>
          {measurementAnnotations.length > 0 ? (
            <div className="measurement-annotations" data-testid="measurement-annotations">
              {measurementAnnotations.map((annotation, index) => (
                <span key={annotation.id}>标注 {index + 1}<em>{annotation.readoutText}</em></span>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
      {ghostPreview ? (
        <div className="ghost-preview-badge" data-testid="ghost-preview-badge">
          幽灵预览 · 尚未写入版本
        </div>
      ) : null}
      <div className="view-cube"><Cube size={28} weight="duotone" /></div>
      <div className="viewport-viewbar">
        <IconButton icon={House} label="等轴" active={cameraView === 'iso'} onClick={() => onCameraViewChange('iso')} />
        <IconButton icon={Crosshair} label="正视" active={cameraView === 'front'} onClick={() => onCameraViewChange('front')} />
        <IconButton icon={GridFour} label="顶视" active={cameraView === 'top'} onClick={() => onCameraViewChange('top')} />
        <IconButton icon={Cube} label="右视" active={cameraView === 'right'} onClick={() => onCameraViewChange('right')} />
        <label className="viewport-light-preset">
          <span>灯光</span>
          <select aria-label="灯光预设" value={lightPreset} onChange={(event) => onLightPresetChange(event.target.value as LightPreset)}>
            {VIEWBAR_LIGHT_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <IconButton
          icon={ArrowsOutCardinal}
          label="爆炸视图"
          active={explodeFactor > 0}
          onClick={onToggleExplode}
        />
      </div>
      <div className="viewport-readout">
        <span>{viewportReadoutText}</span>
        <span>{measurementLine}</span>
      </div>
    </>
  )
}

function IconButton({
  icon: Icon,
  label,
  active = false,
  onClick,
  disabled = false,
  title,
}: {
  icon: ComponentType<{ size: number }>
  label: string
  active?: boolean
  onClick?: () => void
  disabled?: boolean
  title?: string
}) {
  return (
    <button
      className={active ? 'active' : ''}
      onClick={onClick}
      disabled={disabled}
      title={title ?? label}
      aria-label={label}
    >
      <Icon size={17} />
    </button>
  )
}
