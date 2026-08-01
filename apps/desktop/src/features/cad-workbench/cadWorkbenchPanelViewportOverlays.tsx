import { ArrowsOutCardinal, Crosshair, Cube, GridFour } from '@phosphor-icons/react'
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
              title="测量两点之间的距离"
              onClick={() => onMeasurementModeChange('distance')}
            >{MEASURE_MODE_DISTANCE_TEXT}</button>
            <button
              type="button"
              className={measurementMode === 'normal_angle' ? 'active' : ''}
              aria-pressed={measurementMode === 'normal_angle'}
              title="测量两个面的角度"
              onClick={() => onMeasurementModeChange('normal_angle')}
            >{MEASURE_MODE_ANGLE_TEXT}</button>
          </div>
          <span>{measurementLine}</span>
          {measurementReadoutText ? <button type="button" title="固定当前测量标注" onClick={onPinMeasurement}>固定标注</button> : null}
          <button type="button" title="清除所有测量标注" onClick={onClearMeasurements}>清除</button>
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
      <div className="viewport-control-hints" aria-label="三维画布快捷操作">
        <span>旋转视角：左键拖拽</span>
        <span>平移画布：中键拖拽</span>
        <span>放大缩小：滚轮</span>
        <span>适应窗口：双击画布</span>
      </div>
      <div className="viewport-viewbar">
        <IconButton icon={Crosshair} label="正视图" active={cameraView === 'front'} onClick={() => onCameraViewChange('front')} />
        <IconButton icon={GridFour} label="侧视图" active={cameraView === 'right'} onClick={() => onCameraViewChange('right')} />
        <IconButton
          icon={ArrowsOutCardinal}
          label="爆炸图"
          ariaLabel="爆炸视图"
          active={explodeFactor > 0}
          onClick={onToggleExplode}
        />
        <IconButton
          icon={ArrowsOutCardinal}
          label="渲染图"
          active={lightPreset === 'concept_contrast'}
          onClick={() => onLightPresetChange(lightPreset === 'concept_contrast' ? 'cad_neutral' : 'concept_contrast')}
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
  ariaLabel,
}: {
  icon: ComponentType<{ size: number }>
  label: string
  active?: boolean
  onClick?: () => void
  disabled?: boolean
  title?: string
  ariaLabel?: string
}) {
  return (
    <button
      className={active ? 'active' : ''}
      onClick={onClick}
      disabled={disabled}
      title={title ?? label}
      aria-label={ariaLabel ?? label}
    >
      <Icon size={17} />
      <span className="viewport-viewbar-label">{label}</span>
    </button>
  )
}
