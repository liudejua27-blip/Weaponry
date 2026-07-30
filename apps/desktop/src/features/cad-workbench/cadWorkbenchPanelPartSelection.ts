import type {
  SelectActiveDesignRequest,
  SetActiveDesignPartDisplayRequest,
} from '../../shared/types'

export const PART_DISPLAY_BUSY_NOTICE = '正在同步当前设计版本；同步完成后才能修改部件显示状态。'
export const PART_SELECTION_NOT_ACTIVE_NOTICE = '当前显示的模型不是活动设计版本，正在同步后重试。'

type PartMaterialZoneRecord = {
  part_id: string
  material_zone_ids: readonly string[]
}
export function resolvePartFirstMaterialZoneId(
  parts: readonly PartMaterialZoneRecord[] | null | undefined,
  partId: string,
): string | null {
  if (!parts) return null
  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index]
    if (part?.part_id === partId) {
      return part.material_zone_ids[0] ?? null
    }
  }
  return null
}

export function buildSelectPartRequest(
  snapshotRevision: number,
  partId: string,
  materialZoneId: string | null,
): SelectActiveDesignRequest {
  return {
    client_request_id: `active-design-select-${Date.now()}`,
    snapshot_revision: snapshotRevision,
    selected_part_id: partId,
    selected_material_zone_id: materialZoneId,
  }
}

export function buildSelectZoneRequest(
  snapshotRevision: number,
  partId: string,
  zoneId: string,
): SelectActiveDesignRequest {
  return {
    client_request_id: `active-design-zone-${Date.now()}`,
    snapshot_revision: snapshotRevision,
    selected_part_id: partId,
    selected_material_zone_id: zoneId,
  }
}

export function buildPartDisplayRequest(
  snapshotRevision: number,
  action: SetActiveDesignPartDisplayRequest['action'],
  partId?: string,
): SetActiveDesignPartDisplayRequest {
  return {
    client_request_id: `active-design-part-display-${action}-${Date.now()}`,
    snapshot_revision: snapshotRevision,
    action,
    ...(partId ? { part_id: partId } : {}),
  }
}
