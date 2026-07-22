import type { KeyboardEvent, MouseEvent } from 'react'
import { F026Icon } from './F026Icon.js'

export type ReferenceImportCapability = 'glb_compatible_only' | 'reference_guided_rebuild'

/** A presentational F026 composer.  Product actions remain callback-owned. */
export type WorkbenchComposerProps = {
  value: string
  disabled?: boolean
  sending?: boolean
  referenceImportCapability?: ReferenceImportCapability
  onChange: (value: string) => void
  onSend: () => void
  onOpenStyle: () => void
  onOpenMaterial: () => void
  onOpenReference: () => void
  onOpenSurfaceAdornment?: () => void
  surfaceAdornmentDisabled?: boolean
  surfaceAdornmentDetail?: string
}

const COMPOSER_MENU_ID = 'f026-composer-actions'

function isDetailsElement(value: Element | null): value is HTMLDetailsElement {
  return typeof HTMLDetailsElement !== 'undefined' && value instanceof HTMLDetailsElement
}

function menuItems(menu: HTMLElement): HTMLButtonElement[] {
  return [...menu.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]')]
    .filter((button) => !button.disabled)
}

function setMenuOpen(details: HTMLDetailsElement, open: boolean): void {
  details.open = open
  details.querySelector<HTMLElement>('summary')?.setAttribute('aria-expanded', open ? 'true' : 'false')
}

function focusMenuItem(details: HTMLDetailsElement, index: number): void {
  window.requestAnimationFrame(() => {
    const items = menuItems(details)
    if (items.length === 0) return
    items[Math.max(0, Math.min(index, items.length - 1))]?.focus()
  })
}

function handleMenuTriggerKeyDown(event: KeyboardEvent<HTMLElement>): void {
  const details = event.currentTarget.closest('details')
  if (!isDetailsElement(details)) return
  if (event.key === 'Escape' && details.open) {
    event.preventDefault()
    setMenuOpen(details, false)
    event.currentTarget.focus()
    return
  }
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
  event.preventDefault()
  setMenuOpen(details, true)
  focusMenuItem(details, event.key === 'ArrowUp' || event.key === 'End' ? Number.MAX_SAFE_INTEGER : 0)
}

function handleMenuKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
  const details = event.currentTarget.closest('details')
  if (!isDetailsElement(details)) return
  if (event.key === 'Escape') {
    event.preventDefault()
    setMenuOpen(details, false)
    details.querySelector<HTMLElement>('summary')?.focus()
    return
  }
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
  const items = menuItems(event.currentTarget)
  if (items.length === 0) return
  event.preventDefault()
  const currentIndex = Math.max(0, items.indexOf(document.activeElement as HTMLButtonElement))
  const nextIndex = event.key === 'Home'
    ? 0
    : event.key === 'End'
      ? items.length - 1
      : event.key === 'ArrowDown'
        ? (currentIndex + 1) % items.length
        : (currentIndex - 1 + items.length) % items.length
  items[nextIndex]?.focus()
}

function invokeMenuAction(event: MouseEvent<HTMLButtonElement>, action: () => void): void {
  const details = event.currentTarget.closest('details')
  if (isDetailsElement(details)) setMenuOpen(details, false)
  action()
  if (isDetailsElement(details)) {
    window.requestAnimationFrame(() => details.querySelector<HTMLElement>('summary')?.focus())
  }
}

export function WorkbenchComposer({
  value,
  disabled = false,
  sending = false,
  referenceImportCapability = 'glb_compatible_only',
  onChange,
  onSend,
  onOpenStyle,
  onOpenMaterial,
  onOpenReference,
  onOpenSurfaceAdornment,
  surfaceAdornmentDisabled = true,
  surfaceAdornmentDetail = '请先保存设计并选择部件与材质区。',
}: WorkbenchComposerProps) {
  const canSend = !disabled && !sending && value.trim().length > 0
  const referenceDetail = referenceImportCapability === 'reference_guided_rebuild'
    ? '参考图与 GLB 可用于引导重建。'
    : '当前仅兼容 GLB；参考图引导重建待 R007。'

  const send = () => {
    if (!canSend) return
    onSend()
  }
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== 'Enter' || event.shiftKey) return
    event.preventDefault()
    send()
  }

  return (
    <div className="f026-composer-fixed" aria-label="设计输入">
      <details
        className="f026-composer-menu"
        onToggle={(event) => setMenuOpen(event.currentTarget, event.currentTarget.open)}
      >
        <summary
          aria-label="添加风格、材质或参考"
          aria-haspopup="menu"
          aria-expanded={false}
          aria-controls={COMPOSER_MENU_ID}
          onKeyDown={handleMenuTriggerKeyDown}
        >
          <F026Icon name="add" />
        </summary>
        <div id={COMPOSER_MENU_ID} role="menu" aria-label="设计附加操作" onKeyDown={handleMenuKeyDown}>
          <button type="button" role="menuitem" onClick={(event) => invokeMenuAction(event, onOpenStyle)} disabled={disabled}>
            <F026Icon name="style" />
            <span>选择风格</span>
          </button>
          <button type="button" role="menuitem" onClick={(event) => invokeMenuAction(event, onOpenMaterial)} disabled={disabled}>
            <F026Icon name="material" />
            <span>选择材质</span>
          </button>
          <button type="button" role="menuitem" onClick={(event) => invokeMenuAction(event, onOpenReference)} disabled={disabled}>
            <F026Icon name="reference" />
            <span>参考图 / GLB</span>
            <small>{referenceDetail}</small>
          </button>
          {onOpenSurfaceAdornment && (
            <button
              type="button"
              role="menuitem"
              onClick={(event) => invokeMenuAction(event, onOpenSurfaceAdornment)}
              disabled={disabled || surfaceAdornmentDisabled}
              title={surfaceAdornmentDisabled ? surfaceAdornmentDetail : undefined}
            >
              <F026Icon name="style" />
              <span>添加外观细节</span>
              <small>{surfaceAdornmentDisabled ? surfaceAdornmentDetail : '在已选材质区预览，再决定是否保留。'}</small>
            </button>
          )}
        </div>
      </details>
      <div className="f026-composer">
        <textarea
          value={value}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={onKeyDown}
          placeholder="描述你想设计的 3D 概念模型…"
          aria-label="设计需求"
          rows={1}
          disabled={disabled}
        />
        <button
          type="button"
          className="f026-composer-send"
          aria-label="发送设计需求"
          onClick={send}
          disabled={!canSend}
        >
          <F026Icon name="send" />
        </button>
      </div>
    </div>
  )
}
