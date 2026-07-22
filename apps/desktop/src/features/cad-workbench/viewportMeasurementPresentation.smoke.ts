import {
  formatViewportMeasurement,
  readViewportMeasurement,
  type ViewportMeasurementPoint,
} from './viewportMeasurementPresentation.js'

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

const start: ViewportMeasurementPoint = {
  nodeId: 'agent_blockout', position: [0, 0, 0], normal: [0, 1, 0],
}
const end: ViewportMeasurementPoint = {
  nodeId: 'agent_blockout', position: [3, 4, 0], normal: [1, 0, 0],
}

export function runViewportMeasurementPresentationSmoke(): void {
  const distance = readViewportMeasurement('distance', start, end)
  assert(distance?.value === 5 && distance.unit === 'mm', 'measurement must use existing workbench-millimetre hit positions')
  assert(formatViewportMeasurement(distance) === '点到点：5.0 mm', 'distance readout must expose a stable millimetre label')

  const angle = readViewportMeasurement('normal_angle', start, end)
  assert(angle?.value === 90 && angle.unit === '°', 'normal angle must come from the existing WebGL face normals')
  assert(readViewportMeasurement('distance', start, null) === null, 'a partial two-click measurement must not invent a result')
}
