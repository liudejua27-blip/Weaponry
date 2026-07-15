import { ExportDrawer, type ExportDrawerProps } from './ExportDrawer'
import { QualityDrawer, type QualityDrawerProps } from './QualityDrawer'

export type WorkbenchDrawerStackProps = {
  exportOpen: boolean
  qualityOpen: boolean
  exportDrawer: ExportDrawerProps
  quality: QualityDrawerProps
}

/**
 * Composition-only boundary for modal/contextual drawers. It owns no design
 * state and performs no API work; the workbench supplies Snapshot-derived
 * props and callbacks that remain the single source of truth.
 */
export function WorkbenchDrawerStack({
  exportOpen,
  qualityOpen,
  exportDrawer: exportProps,
  quality,
}: WorkbenchDrawerStackProps) {
  return (
    <>
      {exportOpen && <ExportDrawer {...exportProps} />}
      {qualityOpen && <QualityDrawer {...quality} />}
    </>
  )
}
