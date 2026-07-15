import type { MechanicalConceptPlan } from '../../shared/types.js'
import { selectAgentPlanSourcePresentation } from './agentPlanSourcePresentation.js'

function plan(provider_id: string): MechanicalConceptPlan {
  return {
    schema_version: 'MechanicalConceptPlan@1', plan_id: 'plan_smoke', domain_pack_id: 'pack_vehicle_concept', brief: '概念车', generation_stage: 'blockout', spec: {}, directions: [{
      direction_id: 'direction_smoke', title: '紧凑概念车', summary: '完整外观', silhouette: 'balanced', primary_part_roles: ['body'], material_direction: '深色',
    }, {
      direction_id: 'direction_smoke_2', title: '探索概念车', summary: '完整外观', silhouette: 'extended', primary_part_roles: ['body'], material_direction: '深色',
    }, {
      direction_id: 'direction_smoke_3', title: '重型概念车', summary: '完整外观', silhouette: 'industrial', primary_part_roles: ['body'], material_direction: '深色',
    }], provider_id,
  }
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

export function runAgentPlanSourcePresentationSmoke(): void {
  assert(selectAgentPlanSourcePresentation(null) === null, 'no plan must not invent a source')
  const offline = selectAgentPlanSourcePresentation(plan('deterministic_mechanical_planner'))
  assert(offline?.tone === 'offline' && offline.title === '本机离线规划' && offline.detail.includes('尚未调用模型服务'), 'deterministic result must not be presented as a real Provider result')
  const connected = selectAgentPlanSourcePresentation(plan('openai_compatible_mechanical_planner'))
  assert(connected?.tone === 'connected' && connected.title.includes('已连接模型服务') && !connected.detail.includes('openai_compatible'), 'real provider source must be visible without its internal identifier')
  const unknown = selectAgentPlanSourcePresentation(plan('future_provider_internal'))
  assert(unknown?.tone === 'notice' && unknown.title.includes('待确认') && !unknown.detail.includes('future_provider_internal'), 'unknown source must not leak internal identifiers')
}
