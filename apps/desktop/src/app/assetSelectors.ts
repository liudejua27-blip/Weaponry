import type { WeaponDetail } from '../shared/types'
import type { WeaponVersion } from './providers/SelectionProvider'

export type VersionAsset = NonNullable<WeaponVersion['assets']>[number]

export function findConceptOrPatchAsset(version: WeaponVersion | null): VersionAsset | null {
  const assets = [...(version?.assets ?? [])].reverse()
  return assets.find((asset) => asset.role === 'concept_patch')
    ?? assets.find((asset) => asset.role === 'concept_image')
    ?? null
}

export function findAssetByRole(version: WeaponVersion | null, role: string): VersionAsset | null {
  return [...(version?.assets ?? [])].reverse().find((asset) => asset.role === role) ?? null
}

export function findLatestAssetByRole(detail: WeaponDetail | null, role: string): VersionAsset | null {
  return [...(detail?.versions ?? [])]
    .reverse()
    .flatMap((version) => [...(version.assets ?? [])].reverse())
    .find((asset) => asset.role === role) ?? null
}

export function findLatestConceptOrPatchAsset(detail: WeaponDetail | null): VersionAsset | null {
  return [...(detail?.versions ?? [])]
    .reverse()
    .map((version) => findConceptOrPatchAsset(version))
    .find((asset): asset is VersionAsset => Boolean(asset)) ?? null
}
