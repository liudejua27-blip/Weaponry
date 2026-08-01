import type { KeyboardEvent, MouseEvent } from 'react'
import { Microphone, MicrophoneSlash } from '@phosphor-icons/react'
import { F026Icon } from './F026Icon.js'
import {
  COMPOSER_INPUT_ARIA_LABEL,
  COMPOSER_INPUT_PLACEHOLDER,
  COMPOSER_BEGINNER_PROMPT_LABEL,
  COMPOSER_BEGINNER_PROMPTS,
  COMPOSER_MENU_ACTIONS_LABEL,
  COMPOSER_MENU_ARIA_LABEL,
  COMPOSER_PANEL_LABEL,
  COMPOSER_SEND_ARIA_LABEL,
  COMPOSER_SURFACE_ADORNMENT_CLOSED_HINT,
  COMPOSER_SURFACE_ADORNMENT_OPEN_HINT,
  resolveReferenceImportCapabilityHint,
} from './workbenchComposerPrompts.js'

export type ReferenceImportCapability = 'glb_compatible_only' | 'reference_guided_rebuild'
export type GameAssetDeliveryProfile = 'off' | 'game_prop_light' | 'game_prop_standard'

/** A presentational F026 composer.  Product actions remain callback-owned. */
export type WorkbenchComposerProps = {
  value: string
  disabled?: boolean
  sending?: boolean
  referenceImportCapability?: ReferenceImportCapability
  showAdvancedActions?: boolean
  showStarterPrompts?: boolean
  onChange: (value: string) => void
  onSend: () => void
  onOpenStyle: () => void
  onOpenMaterial: () => void
  onOpenReference: () => void
  onOpenTemplate?: () => void
  onToggleVoice?: () => void
  voiceListening?: boolean
  onOpenSurfaceAdornment?: () => void
  surfaceAdornmentDisabled?: boolean
  surfaceAdornmentDetail?: string
  gameAssetDeliveryProfile?: GameAssetDeliveryProfile
  onGameAssetDeliveryProfileChange?: (profile: GameAssetDeliveryProfile) => void
  starterPrompts?: readonly string[]
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

function handleMenuTriggerKeyDown(event: KeyboardEvent<HTMLElement>, disabled = false): void {
  const details = event.currentTarget.closest('details')
  if (!isDetailsElement(details)) return
  if (disabled) return
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

function handleMenuKeyDown(event: KeyboardEvent<HTMLDivElement>, disabled = false): void {
  const details = event.currentTarget.closest('details')
  if (!isDetailsElement(details)) return
  if (disabled) return
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
  showAdvancedActions = true,
  showStarterPrompts = true,
  onChange,
  onSend,
  onOpenStyle,
  onOpenMaterial,
  onOpenReference,
  onOpenTemplate,
  onToggleVoice,
  voiceListening = false,
  onOpenSurfaceAdornment,
  surfaceAdornmentDisabled = true,
  surfaceAdornmentDetail = COMPOSER_SURFACE_ADORNMENT_CLOSED_HINT,
  gameAssetDeliveryProfile = 'off',
  onGameAssetDeliveryProfileChange,
  starterPrompts = COMPOSER_BEGINNER_PROMPTS,
}: WorkbenchComposerProps) {
  const canSend = !disabled && !sending && value.trim().length > 0

  const send = () => {
    if (!canSend) return
    onSend()
  }
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== 'Enter' || event.shiftKey) return
    event.preventDefault()
    send()
  }
  const onTextAreaChange = (event: { target: { value: string } }) => {
    onChange(event.target.value)
  }
  const onStarterPromptClick = (event: MouseEvent<HTMLButtonElement>) => {
    const prompt = event.currentTarget.dataset?.prompt
    if (prompt !== undefined) onChange(prompt)
  }
  const invokeStyleAction = (event: MouseEvent<HTMLButtonElement>) => {
    invokeMenuAction(event, onOpenStyle)
  }
  const invokeMaterialAction = (event: MouseEvent<HTMLButtonElement>) => {
    invokeMenuAction(event, onOpenMaterial)
  }
  const invokeReferenceAction = (event: MouseEvent<HTMLButtonElement>) => {
    invokeMenuAction(event, onOpenReference)
  }
  const invokeTemplateAction = (event: MouseEvent<HTMLButtonElement>) => {
    if (onOpenTemplate) invokeMenuAction(event, onOpenTemplate)
  }
  const invokeVoiceAction = (event: MouseEvent<HTMLButtonElement>) => {
    if (onToggleVoice) invokeMenuAction(event, onToggleVoice)
  }
  const quickActions = (
    <div className="f026-composer-quick-actions" aria-label="输入辅助">
      <button type="button" onClick={onOpenReference} disabled={disabled || sending} title="添加参考图片">
        <F026Icon name="reference" />
        <span>参考图</span>
      </button>
      {onToggleVoice ? (
        <button type="button" onClick={onToggleVoice} disabled={disabled || sending} aria-pressed={voiceListening} title={voiceListening ? '停止语音输入' : '语音输入'}>
          {voiceListening ? <MicrophoneSlash size={14} aria-hidden="true" /> : <Microphone size={14} aria-hidden="true" />}
          <span>{voiceListening ? '停止语音' : '语音输入'}</span>
        </button>
      ) : null}
      {onOpenTemplate ? (
        <button type="button" onClick={onOpenTemplate} disabled={disabled || sending} title="从模板开始设计">
          <F026Icon name="components" />
          <span>使用模板</span>
        </button>
      ) : null}
      <details
        className="f026-composer-menu f026-composer-menu-beginner"
        onToggle={(event) => {
          if (disabled || sending) {
            setMenuOpen(event.currentTarget, false)
            return
          }
          setMenuOpen(event.currentTarget, event.currentTarget.open)
        }}
      >
        <summary
          aria-label={COMPOSER_MENU_ARIA_LABEL}
          aria-haspopup="menu"
          aria-disabled={disabled || sending}
          aria-expanded={false}
          aria-controls={COMPOSER_MENU_ID}
          onKeyDown={(event) => handleMenuTriggerKeyDown(event, disabled || sending)}
        >
          <F026Icon name="add" />
        </summary>
        <div
          id={COMPOSER_MENU_ID}
          role="menu"
          aria-label={COMPOSER_MENU_ACTIONS_LABEL}
          onKeyDown={(event) => handleMenuKeyDown(event, disabled || sending)}
        >
          <button type="button" role="menuitem" onClick={invokeStyleAction} disabled={disabled}>
            <F026Icon name="style" />
            <span>换外观</span>
          </button>
          <button type="button" role="menuitem" aria-label="选择材质" onClick={invokeMaterialAction} disabled={disabled}>
            <F026Icon name="material" />
            <span>换材质</span>
          </button>
          <button type="button" role="menuitem" onClick={invokeReferenceAction} disabled={disabled}>
            <F026Icon name="reference" />
            <span>添加参考</span>
            <small>{resolveReferenceImportCapabilityHint(referenceImportCapability)}</small>
          </button>
        </div>
      </details>
    </div>
  )
  const invokeSurfaceAdornmentAction = (event: MouseEvent<HTMLButtonElement>) => {
    if (onOpenSurfaceAdornment) invokeMenuAction(event, onOpenSurfaceAdornment)
  }
  const gameAssetDeliveryPicker = onGameAssetDeliveryProfileChange ? (
    <label className="f026-composer-delivery" aria-label="游戏资产输出">
      <span>游戏资产输出</span>
      <select
        aria-label="游戏资产输出规格"
        value={gameAssetDeliveryProfile}
        disabled={disabled || sending}
        onChange={(event) => onGameAssetDeliveryProfileChange(event.target.value as GameAssetDeliveryProfile)}
      >
        <option value="off">概念 GLB（默认）</option>
        <option value="game_prop_light">轻量游戏道具（LOD + 碰撞）</option>
        <option value="game_prop_standard">标准游戏道具（LOD + 碰撞）</option>
      </select>
      <small>专业修改会先生成预览，确认后才会保存到设计。</small>
    </label>
  ) : null

  if (!showAdvancedActions) {
    return (
      <div className="f026-composer-fixed" aria-label={COMPOSER_PANEL_LABEL}>
        {showStarterPrompts && starterPrompts.length > 0 && !value && (
          <div className="f026-composer-starters" aria-label={COMPOSER_BEGINNER_PROMPT_LABEL}>
            <p className="f026-composer-starters-title">快速起步（点选任一）</p>
            {starterPrompts.map((prompt) => (
              <button
                key={prompt}
                type="button"
                className="f026-composer-starter"
                data-prompt={prompt}
                onClick={onStarterPromptClick}
              >
                {prompt}
              </button>
            ))}
          </div>
        )}
        <div className="f026-composer">
          <textarea
            value={value}
            onChange={onTextAreaChange}
            onKeyDown={onKeyDown}
            placeholder={COMPOSER_INPUT_PLACEHOLDER}
            aria-label={COMPOSER_INPUT_ARIA_LABEL}
            rows={1}
            disabled={disabled}
          />
          <button
            type="button"
            className="f026-composer-send f026-composer-send-beginner"
            aria-label={COMPOSER_SEND_ARIA_LABEL}
            onClick={send}
            disabled={!canSend}
          >
            <span>开始生成</span>
            <F026Icon name="send" />
          </button>
        </div>
        {gameAssetDeliveryPicker}
        {quickActions}
        <div className="f026-composer-hint">按 Enter 发送，Shift + Enter 换行</div>
      </div>
    )
  }

  return (
    <div className="f026-composer-fixed" aria-label={COMPOSER_PANEL_LABEL}>
      {showStarterPrompts && starterPrompts.length > 0 && !value && (
        <div className="f026-composer-starters" aria-label={COMPOSER_BEGINNER_PROMPT_LABEL}>
          <p className="f026-composer-starters-title">快速起步（点选任一）</p>
          {starterPrompts.map((prompt) => (
            <button
              key={prompt}
              type="button"
              className="f026-composer-starter"
              data-prompt={prompt}
              onClick={onStarterPromptClick}
            >
              {prompt}
            </button>
          ))}
        </div>
      )}
      {showAdvancedActions ? (
        <details
          className="f026-composer-menu"
          onToggle={(event) => {
            if (disabled || sending) {
              setMenuOpen(event.currentTarget, false)
              return
            }
            setMenuOpen(event.currentTarget, event.currentTarget.open)
          }}
        >
          <summary
            aria-label={COMPOSER_MENU_ARIA_LABEL}
            aria-haspopup="menu"
            aria-disabled={disabled || sending}
            aria-expanded={false}
            aria-controls={COMPOSER_MENU_ID}
            onKeyDown={(event) => handleMenuTriggerKeyDown(event, disabled || sending)}
          >
            <F026Icon name="add" />
          </summary>
          <div
            id={COMPOSER_MENU_ID}
            role="menu"
            aria-label={COMPOSER_MENU_ACTIONS_LABEL}
            onKeyDown={(event) => handleMenuKeyDown(event, disabled || sending)}
          >
            <button type="button" role="menuitem" onClick={invokeStyleAction} disabled={disabled}>
              <F026Icon name="style" />
              <span>换外观</span>
            </button>
            <button type="button" role="menuitem" onClick={invokeMaterialAction} disabled={disabled}>
              <F026Icon name="material" />
              <span>换材质</span>
            </button>
            <button type="button" role="menuitem" onClick={invokeReferenceAction} disabled={disabled}>
              <F026Icon name="reference" />
              <span>添加参考</span>
              <small>{resolveReferenceImportCapabilityHint(referenceImportCapability)}</small>
            </button>
            {onToggleVoice && (
              <button type="button" role="menuitem" onClick={invokeVoiceAction} disabled={disabled} aria-pressed={voiceListening}>
                {voiceListening ? <MicrophoneSlash size={15} aria-hidden="true" /> : <Microphone size={15} aria-hidden="true" />}
                <span>{voiceListening ? '停止语音输入' : '语音输入'}</span>
                <small>使用系统麦克风填入需求</small>
              </button>
            )}
            {onOpenTemplate && (
              <button type="button" role="menuitem" onClick={invokeTemplateAction} disabled={disabled}>
                <F026Icon name="components" />
                <span>使用模板</span>
                <small>打开模板选择</small>
              </button>
            )}
            {onOpenSurfaceAdornment && (
              <button
                type="button"
                role="menuitem"
                onClick={invokeSurfaceAdornmentAction}
                disabled={disabled || surfaceAdornmentDisabled}
                title={surfaceAdornmentDisabled ? surfaceAdornmentDetail : undefined}
              >
                <F026Icon name="style" />
                <span>加局部装饰</span>
                <small>{surfaceAdornmentDisabled ? surfaceAdornmentDetail : COMPOSER_SURFACE_ADORNMENT_OPEN_HINT}</small>
              </button>
            )}
          </div>
        </details>
      ) : null}
      <div className="f026-composer">
        <textarea
          value={value}
          onChange={onTextAreaChange}
          onKeyDown={onKeyDown}
          placeholder={COMPOSER_INPUT_PLACEHOLDER}
          aria-label={COMPOSER_INPUT_ARIA_LABEL}
          rows={1}
          disabled={disabled}
        />
        <button
          type="button"
          className={`f026-composer-send ${showAdvancedActions ? '' : 'f026-composer-send-beginner'}`}
          aria-label={COMPOSER_SEND_ARIA_LABEL}
          onClick={send}
          disabled={!canSend}
        >
          {!showAdvancedActions ? <span>生成可编辑预览</span> : null}
          <F026Icon name="send" />
        </button>
      </div>
      {gameAssetDeliveryPicker}
      {quickActions}
      <div className="f026-composer-hint">按 Enter 发送，Shift + Enter 换行</div>
    </div>
  )
}
