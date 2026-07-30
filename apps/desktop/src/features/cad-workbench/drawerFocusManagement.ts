import type { RefObject } from 'react'

const DRAWER_FOCUSABLE_SELECTOR = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])'

export function bindDrawerFocusTrap(
  drawerRef: RefObject<HTMLElement | null>,
  closeAllDrawers: () => void,
): () => void {
  const drawer = drawerRef.current
  if (!drawer) return () => {}

  const focusInitialControl = () => {
    const initial = drawer.querySelector<HTMLElement>('[data-dialog-initial-focus="true"]')
      ?? drawer.querySelector<HTMLElement>(DRAWER_FOCUSABLE_SELECTOR)
    initial?.focus()
  }

  const frame = window.requestAnimationFrame(focusInitialControl)

  const onDrawerKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      closeAllDrawers()
      return
    }

    if (event.key !== 'Tab') return

    const focusable = Array.from(
      drawer.querySelectorAll<HTMLElement>(DRAWER_FOCUSABLE_SELECTOR),
    ).filter((element) => !element.hasAttribute('disabled') && element.offsetParent !== null)

    if (focusable.length === 0) {
      event.preventDefault()
      drawer.focus()
      return
    }

    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    if (!drawer.contains(document.activeElement)) {
      event.preventDefault()
      first.focus()
    } else if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  window.addEventListener('keydown', onDrawerKeyDown, true)

  return () => {
    window.cancelAnimationFrame(frame)
    window.removeEventListener('keydown', onDrawerKeyDown, true)
  }
}
