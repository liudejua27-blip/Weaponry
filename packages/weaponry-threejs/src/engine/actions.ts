import {
  WEAPONRY_ACTIONS,
  type WeaponryAction,
  type WeaponryInputEvent,
  type WeaponryInputMapping,
} from './types.ts'

export const WEAPONRY_INPUT_MAPPING_SCHEMA = 'WeaponryThreeJsInputMapping@1' as const

/**
 * Keyboard codes intentionally use DOM KeyboardEvent.code names. The map is
 * closed on the action side, while unknown physical keys simply resolve to
 * no action.
 */
export const DEFAULT_WEAPONRY_INPUT_MAPPING: WeaponryInputMapping = Object.freeze({
  bindings: Object.freeze({
    MouseLeft: 'light',
    MouseRight: 'heavy',
    KeyI: 'inspect',
    KeyE: 'inspect',
    KeyQ: 'sheath',
    Escape: 'idle',
  }),
})

export function createWeaponryInputMapping(
  overrides: Readonly<Record<string, WeaponryAction>> = {},
): WeaponryInputMapping {
  const bindings: Record<string, WeaponryAction> = { ...DEFAULT_WEAPONRY_INPUT_MAPPING.bindings }
  for (const [code, action] of Object.entries(overrides)) {
    if (!code || !isWeaponryAction(action)) {
      throw new Error(`WEAPONRY_ENGINE_INVALID_INPUT_BINDING: ${code}`)
    }
    bindings[code] = action
  }
  return Object.freeze({ bindings: Object.freeze(bindings) })
}

export function resolveWeaponryAction(
  input: WeaponryInputEvent | string,
  mapping: WeaponryInputMapping = DEFAULT_WEAPONRY_INPUT_MAPPING,
): WeaponryAction | null {
  const code = typeof input === 'string' ? input : input.code
  if (!code || (typeof input !== 'string' && input.phase === 'released')) return null
  const action = mapping.bindings[code]
  return isWeaponryAction(action) ? action : null
}

export function isWeaponryAction(value: unknown): value is WeaponryAction {
  return typeof value === 'string' && (WEAPONRY_ACTIONS as readonly string[]).includes(value)
}
