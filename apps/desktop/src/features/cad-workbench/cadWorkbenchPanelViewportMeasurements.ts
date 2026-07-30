import { useCallback, useMemo, useState } from 'react'
import {
  formatViewportMeasurement,
  readViewportMeasurement,
  type ViewportMeasurementMode,
} from './viewportMeasurementPresentation'
import { buildViewportMeasurementPrompt } from './cadWorkbenchPanelViewportReadout'
import { type ViewportMeasurementPoint } from './ModuleGraphViewport'

type CadWorkbenchPanelMeasurementReadout = ReturnType<
  typeof readViewportMeasurement
>

type MeasurementAnnotation = {
  id: string
  readout: CadWorkbenchPanelMeasurementReadout
}

const EMPTY_MEASUREMENT_ANNOTATIONS: ReadonlyArray<{
  readonly id: string
  readonly readoutText: string
}> = []

type CadWorkbenchPanelViewportMeasurementAnnotation = {
  id: string
  readoutText: string
}

type CadWorkbenchPanelViewportMeasurementState = {
  measurementMode: ViewportMeasurementMode
  measurementReadoutText: string | null
  measurementPrompt: string
  measurementAnnotations: readonly CadWorkbenchPanelViewportMeasurementAnnotation[]
  handleMeasurePoint: (point: ViewportMeasurementPoint) => void
  clearMeasurements: () => void
  pinMeasurement: () => void
  setMeasurementMode: (mode: ViewportMeasurementMode) => void
}

const MAX_MEASUREMENT_ANNOTATIONS = 4

function buildMeasurementOverlayAnnotations(
  measurementAnnotations: readonly MeasurementAnnotation[],
): readonly CadWorkbenchPanelViewportMeasurementAnnotation[] {
  if (measurementAnnotations.length === 0) return EMPTY_MEASUREMENT_ANNOTATIONS
  const currentAnnotations = new Array<CadWorkbenchPanelViewportMeasurementAnnotation>(measurementAnnotations.length)
  for (let index = 0; index < measurementAnnotations.length; index += 1) {
    const annotation = measurementAnnotations[index]
    if (!annotation) continue
    currentAnnotations[index] = {
      id: annotation.id,
      readoutText: formatViewportMeasurement(annotation.readout),
    }
  }
  return currentAnnotations
}

function buildMeasurementAnnotationsWithLimit(current: MeasurementAnnotation[]): MeasurementAnnotation[] {
  const nextLength = Math.min(current.length + 1, MAX_MEASUREMENT_ANNOTATIONS)
  const offset = current.length + 1 - nextLength
  const next = new Array<MeasurementAnnotation>(nextLength)

  for (let index = 0; index < nextLength - 1; index += 1) {
    next[index] = current[offset + index]
  }

  return next
}

export function useCadWorkbenchPanelViewportMeasurements(): CadWorkbenchPanelViewportMeasurementState {
  const [measurementMode, setMeasurementMode] = useState<ViewportMeasurementMode>('distance')
  const [measurementStart, setMeasurementStart] = useState<ViewportMeasurementPoint | null>(null)
  const [measurementEnd, setMeasurementEnd] = useState<ViewportMeasurementPoint | null>(null)
  const [measurementAnnotations, setMeasurementAnnotations] = useState<MeasurementAnnotation[]>([])

  const measurementReadout = useMemo(
    () => readViewportMeasurement(measurementMode, measurementStart, measurementEnd),
    [measurementEnd, measurementMode, measurementStart],
  )
  const handleMeasurePoint = useCallback((point: ViewportMeasurementPoint) => {
    if (!measurementStart || measurementEnd) {
      setMeasurementStart(point)
      setMeasurementEnd(null)
      return
    }
    setMeasurementEnd(point)
  }, [measurementEnd, measurementStart])
  const clearMeasurements = useCallback(() => {
    setMeasurementStart(null)
    setMeasurementEnd(null)
    setMeasurementAnnotations([])
  }, [])
  const pinMeasurement = useCallback(() => {
    if (!measurementReadout) return
    setMeasurementAnnotations((current) => {
      const next = buildMeasurementAnnotationsWithLimit(current)
      next[next.length - 1] = {
        id: `measurement-${Date.now().toString(36)}`,
        readout: measurementReadout,
      }
      return next
    })
    setMeasurementStart(null)
    setMeasurementEnd(null)
  }, [measurementReadout])
  const measurementReadoutText = useMemo(
    () => (measurementReadout ? formatViewportMeasurement(measurementReadout) : null),
    [measurementReadout],
  )
  const measurementPrompt = useMemo(
    () => buildViewportMeasurementPrompt({
      hasMeasurementStart: Boolean(measurementStart),
      hasMeasurementEnd: Boolean(measurementEnd),
    }),
    [measurementEnd, measurementStart],
  )
  const measurementOverlayAnnotations = useMemo(
    () => buildMeasurementOverlayAnnotations(measurementAnnotations),
    [measurementAnnotations],
  )

  return {
    measurementMode,
    measurementReadoutText,
    measurementPrompt,
    measurementAnnotations: measurementOverlayAnnotations,
    handleMeasurePoint,
    clearMeasurements,
    pinMeasurement,
    setMeasurementMode,
  }
}
