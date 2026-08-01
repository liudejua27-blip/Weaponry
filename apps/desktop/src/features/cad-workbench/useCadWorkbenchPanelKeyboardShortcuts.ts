import { useEffect } from 'react'

type CadWorkbenchPanelKeyboardShortcutsInput = {
  onUndo: () => void
  onRedo: () => void
  onSave: () => void
  onFocusSelectedComponent: () => void
  onEscape: () => boolean
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement
    || target.isContentEditable
}

/** Keep the common desktop shortcuts in one place without creating another renderer or state owner. */
export function useCadWorkbenchPanelKeyboardShortcuts({
  onUndo,
  onRedo,
  onSave,
  onFocusSelectedComponent,
  onEscape,
}: CadWorkbenchPanelKeyboardShortcutsInput): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        if (!onEscape()) return
        event.preventDefault()
        return
      }

      const commandKey = event.metaKey || event.ctrlKey
      const key = event.key.toLowerCase()
      if (commandKey && key === 's') {
        event.preventDefault()
        onSave()
        return
      }

      if (isEditableTarget(event.target)) return
      if (commandKey && key === 'z') {
        event.preventDefault()
        if (event.shiftKey) onRedo()
        else onUndo()
        return
      }
      if (commandKey && key === 'y') {
        event.preventDefault()
        onRedo()
        return
      }
      if (!commandKey && !event.altKey && key === 'f') {
        event.preventDefault()
        onFocusSelectedComponent()
      }
    }

    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [onEscape, onFocusSelectedComponent, onRedo, onSave, onUndo])
}
