import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from 'react'
import type { WeaponDetail } from '../../shared/types'
import { parseHashRoute } from '../routing'

export type WeaponVersion = NonNullable<WeaponDetail['versions']>[number]

type SelectionContextValue = {
  activeWeaponId: string
  activeVersionId: string
  activeWeaponDetail: WeaponDetail | null
  activeVersion: WeaponVersion | null
  setActiveWeaponId: (weaponId: string) => void
  setActiveVersionId: (versionId: string) => void
  loadWeaponDetail: (detail: WeaponDetail) => void
  clearWeaponDetail: () => void
}

const SelectionContext = createContext<SelectionContextValue | null>(null)

export function SelectionProvider({ children }: { children: ReactNode }) {
  const initialRoute = useMemo(() => parseHashRoute(), [])
  const [activeWeaponId, setActiveWeaponId] = useState(
    initialRoute.kind === 'weapon' ? initialRoute.weaponId : '',
  )
  const [activeVersionId, setActiveVersionId] = useState(
    initialRoute.kind === 'weapon' ? initialRoute.versionId ?? '' : '',
  )
  const [activeWeaponDetail, setActiveWeaponDetail] = useState<WeaponDetail | null>(null)

  const activeVersion = useMemo(() => {
    const versions = activeWeaponDetail?.versions ?? []
    return versions.find((version) => version.version_id === activeVersionId)
      ?? versions.find((version) => version.version_id === activeWeaponDetail?.current_version_id)
      ?? versions.at(-1)
      ?? null
  }, [activeVersionId, activeWeaponDetail])

  const loadWeaponDetail = useCallback((detail: WeaponDetail) => {
    setActiveWeaponDetail(detail)
    setActiveWeaponId(detail.weapon_id)
    setActiveVersionId((current) => {
      if (current && (detail.versions ?? []).some((version) => version.version_id === current)) {
        return current
      }
      return detail.current_version_id || (detail.versions ?? []).at(-1)?.version_id || ''
    })
  }, [])

  const clearWeaponDetail = useCallback(() => setActiveWeaponDetail(null), [])

  const value = useMemo<SelectionContextValue>(() => ({
    activeWeaponId,
    activeVersionId,
    activeWeaponDetail,
    activeVersion,
    setActiveWeaponId,
    setActiveVersionId,
    loadWeaponDetail,
    clearWeaponDetail,
  }), [
    activeVersion,
    activeVersionId,
    activeWeaponDetail,
    activeWeaponId,
    clearWeaponDetail,
    loadWeaponDetail,
  ])

  return <SelectionContext.Provider value={value}>{children}</SelectionContext.Provider>
}

export function useSelection(): SelectionContextValue {
  const value = useContext(SelectionContext)
  if (value === null) throw new Error('useSelection must be used inside SelectionProvider.')
  return value
}
