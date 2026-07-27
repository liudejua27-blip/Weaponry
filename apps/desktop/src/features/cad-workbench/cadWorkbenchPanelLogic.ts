import type { SurfaceAdornmentDraft } from './SurfaceAdornmentDrawer.js'
import { ForgeApiError } from '../../shared/api/forgeApi'
import type { AgentPartEditOperation, AssemblyDeltaProgram } from '../../shared/types'

const COVERAGE_BY_DRAFT: Record<SurfaceAdornmentDraft['coverage'], 'center_band' | 'edge_band' | 'full_zone' | 'symmetric_pair'> = {
  center: 'center_band',
  edge: 'edge_band',
  full: 'full_zone',
  symmetric: 'symmetric_pair',
}

const QUALITY_STATUS_LABELS = {
  passed: '通过',
  warning: '需复核',
  failed: '失败',
} as const

const PACK_VEHICLE_RE = /(car|vehicle|truck|auto|汽车|车辆|载具)/
const PACK_AIRCRAFT_RE = /(plane|aircraft|drone|jet|飞机|飞行|无人机)/
const PACK_ARM_RE = /(arm|robot|joint|机械臂|机器人|关节)/
const OPERATION_ID_SANITIZE_RE = /[^A-Za-z0-9_-]/g

export function buildAgentPartEditOperations(delta: AssemblyDeltaProgram): AgentPartEditOperation[] {
  return delta.operations.map((operation, index) => {
    const operationId = `op_${operation.operation_id.replace(OPERATION_ID_SANITIZE_RE, '_').slice(0, 112)}_${index}`
    if (operation.op === 'add_reviewed_recipe') {
      return {
        operation_id: operationId,
        op: operation.op,
        part_id: operation.parent_part_id,
        new_part_id: operation.new_part_id,
        parent_connector_id: operation.parent_connector_id,
        child_connector_id: operation.child_connector_id,
        recipe_id: operation.recipe_id,
        slot_id: operation.slot_id,
        transform: operation.transform,
      } as AgentPartEditOperation
    }
    if (operation.op === 'set_joint_pose') {
      return {
        operation_id: operationId,
        op: operation.op,
        part_id: operation.part_id,
        joint_id: operation.joint_id,
        pose: operation.pose,
      } as AgentPartEditOperation
    }
    return {
      ...operation,
      operation_id: operationId,
    } as AgentPartEditOperation
  })
}

export function compileSurfaceAdornmentDraft(draft: SurfaceAdornmentDraft) {
  const intensity: 'subtle' | 'balanced' | 'pronounced' = draft.intensity === 'bold'
    ? 'pronounced'
    : draft.intensity
  const coverage = COVERAGE_BY_DRAFT[draft.coverage]

  if (draft.kind === 'streamline') {
    return { kind: 'flowline' as const, motif: 'double_flowline' as const, intensity, coverage }
  }

  if (draft.kind === 'texture') {
    return {
      kind: 'micro_surface' as const,
      motif: draft.motif === 'parallel' ? 'parallel_groove' as const : 'hex_microgrid' as const,
      intensity,
      coverage,
    }
  }

  return {
    kind: 'normal_relief' as const,
    motif: draft.motif === 'radial' || draft.motif === 'technical_mark'
      ? 'chevron_relief' as const
      : 'parallel_groove' as const,
    intensity,
    coverage,
  }
}

export function referenceRebuildFailureMessage(error: unknown): string {
  if (error instanceof ForgeApiError && error.code === 'REFERENCE_REBUILD_C106_BASE_REQUIRED') {
    return '请先生成并确认机械臂生产基准，再使用参考重建；当前设计没有变化。'
  }
  return error instanceof Error ? error.message : '参考引导重建预览失败；当前设计没有变化。'
}

export function qualityStatusLabel(status?: 'passed' | 'warning' | 'failed' | 'not_run') {
  if (!status || status === 'not_run') return '未运行'
  return QUALITY_STATUS_LABELS[status]
}

export function inferImportDomainPack(fileName: string): 'pack_future_weapon_prop' | 'pack_vehicle_concept' | 'pack_aircraft_concept' | 'pack_robotic_arm_concept' {
  const value = fileName.toLowerCase()
  if (PACK_VEHICLE_RE.test(value)) return 'pack_vehicle_concept'
  if (PACK_AIRCRAFT_RE.test(value)) return 'pack_aircraft_concept'
  if (PACK_ARM_RE.test(value)) return 'pack_robotic_arm_concept'
  return 'pack_future_weapon_prop'
}

export function errorText(caught: unknown): string {
  return caught instanceof Error ? caught.message : String(caught)
}
