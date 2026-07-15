import { ComponentDrawer, type ComponentDrawerProps } from './ComponentDrawer'
import { ExportDrawer, type ExportDrawerProps } from './ExportDrawer'
import { QualityDrawer, type QualityDrawerProps } from './QualityDrawer'

export type WorkbenchDrawerStackProps = {
  componentDrawerOpen: boolean
  exportOpen: boolean
  qualityOpen: boolean
  component: ComponentDrawerProps
  exportDrawer: ExportDrawerProps
  quality: QualityDrawerProps
}

/**
 * Composition-only boundary for modal/contextual drawers. It owns no design
 * state and performs no API work; the workbench supplies Snapshot-derived
 * props and callbacks that remain the single source of truth.
 */
export function WorkbenchDrawerStack({
  componentDrawerOpen,
  exportOpen,
  qualityOpen,
  component,
  exportDrawer: exportProps,
  quality,
}: WorkbenchDrawerStackProps) {
  return (
    <>
      {componentDrawerOpen && <ComponentDrawer {...component} />}
      {exportOpen && <ExportDrawer {...exportProps} />}
      {qualityOpen && <QualityDrawer {...quality} />}
    </>
  )
}
