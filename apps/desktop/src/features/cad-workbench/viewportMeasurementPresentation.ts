export type ViewportMeasurementPoint = {
  nodeId: string
  position: [number, number, number]
  normal: [number, number, number]
}

export type ViewportMeasurementMode = 'distance' | 'normal_angle'

export type ViewportMeasurementReadout = {
  mode: ViewportMeasurementMode
  value: number
  unit: 'mm' | '°'
  label: '点到点' | '表面法线夹角'
}

/**
 * Viewport measurements are an ephemeral inspection aid. The positions come
 * directly from the one existing WebGL scene in workbench millimetres; no
 * model, Snapshot, or localStorage state is authored here.
 */
export function readViewportMeasurement(
  mode: ViewportMeasurementMode,
  start: ViewportMeasurementPoint | null,
  end: ViewportMeasurementPoint | null,
): ViewportMeasurementReadout | null {
  if (!start || !end) return null
  if (mode === 'normal_angle') {
    const a = normalize(start.normal)
    const b = normalize(end.normal)
    const dot = clamp(a[0] * b[0] + a[1] * b[1] + a[2] * b[2], -1, 1)
    return { mode, value: Math.acos(dot) * 180 / Math.PI, unit: '°', label: '表面法线夹角' }
  }
  const dx = end.position[0] - start.position[0]
  const dy = end.position[1] - start.position[1]
  const dz = end.position[2] - start.position[2]
  return { mode, value: Math.hypot(dx, dy, dz), unit: 'mm', label: '点到点' }
}

export function formatViewportMeasurement(readout: ViewportMeasurementReadout | null): string {
  if (!readout) return ''
  return `${readout.label}：${readout.value.toFixed(readout.unit === 'mm' ? 1 : 1)} ${readout.unit}`
}

function normalize(value: [number, number, number]): [number, number, number] {
  const length = Math.hypot(value[0], value[1], value[2])
  if (length < 1e-6) return [0, 1, 0]
  return [value[0] / length, value[1] / length, value[2] / length]
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}
