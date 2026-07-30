import type { ComponentType } from 'react'
import {
  ArrowsClockwise,
  CursorClick,
  Ruler,
} from '@phosphor-icons/react'

type Tool = 'select' | 'move' | 'rotate' | 'scale' | 'orbit' | 'measure' | 'section'

export type CameraView = 'iso' | 'front' | 'top' | 'right'

export type LightPreset = 'cad_neutral' | 'soft_studio' | 'concept_contrast'

type WorkbenchToolItem = {
  id: Tool
  label: string
  icon: ComponentType<{ size: number }>
  implemented: boolean
  unavailableReason?: string
}

export const VIEWPORT_TOOLBAR_ITEMS: ReadonlyArray<WorkbenchToolItem> = [
  { id: 'select', label: '选择', icon: CursorClick, implemented: true },
  { id: 'orbit', label: '旋转视图', icon: ArrowsClockwise, implemented: true },
  { id: 'measure', label: '测量', icon: Ruler, implemented: true },
]
