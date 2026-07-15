import type { MechanicalConceptPlan } from '../../shared/types.js'

export type AgentPlanSourcePresentation = {
  tone: 'offline' | 'connected' | 'notice'
  title: string
  detail: string
}

/** Reads the source recorded on a completed plan; it never reads credentials or calls a Provider. */
export function selectAgentPlanSourcePresentation(
  plan: MechanicalConceptPlan | null,
): AgentPlanSourcePresentation | null {
  if (!plan) return null
  if (plan.provider_id === 'deterministic_mechanical_planner') {
    return {
      tone: 'offline',
      title: '本机离线规划',
      detail: '当前方向由本机规则生成，尚未调用模型服务，不能代表真实模型质量。',
    }
  }
  if (plan.provider_id === 'openai_compatible_mechanical_planner') {
    return {
      tone: 'connected',
      title: '已连接模型服务生成',
      detail: '本次方向已由已连接的模型服务生成；仍请先查看预览再保存。',
    }
  }
  return {
    tone: 'notice',
    title: '规划来源待确认',
    detail: '请先查看预览；当前结果不会覆盖已保存设计。',
  }
}
