import type { AgentMaterialPreset } from '../../shared/types'
import type { AgentClarificationOption } from './agentConversationState'

const DEFAULT_CONCEPT_BRIEF = '一个轮廓清晰、比例协调、关键外观明确并适合继续编辑的 3D 对象'

const CONCEPT_FAMILY_SUGGESTIONS = [
  ['写实家猫', '一只站立的写实短毛家猫，体态自然，面部、四肢和毛发表面特征清晰'],
  ['玻璃山谷住宅', '一座位于山谷中的现代玻璃住宅，建筑体块、露台、岩石和植被关系明确'],
  ['陶瓷茶具', '一套手工白瓷茶具，壶、杯、托盘分件清楚，釉面有细腻粗糙度变化'],
  ['游戏道具', '一个用于游戏美术的原创科幻道具，按参考描述保持真实对象身份，分件和材质区域清晰'],
] as const

// U002 no longer asks users to choose one of four Domain Packs. This empty
// compatibility input keeps old error adapters typed without exposing a
// category admission UI.
const DEFAULT_AGENT_CLARIFICATION_OPTIONS: readonly AgentClarificationOption[] = []

const DEFAULT_AGENT_MATERIAL_PRESETS: AgentMaterialPreset[] = [
  { schema_version: 'MaterialPreset@1', material_id: 'mat_graphite', display_name: '石墨深灰', category: 'metal', pbr: { base_color: '#26313b', metallic: 0.78, roughness: 0.34, opacity: 1 }, visual_only: true, allowed_domains: ['*'], provenance: 'forgecad_builtin' },
  { schema_version: 'MaterialPreset@1', material_id: 'mat_aluminum', display_name: '拉丝铝', category: 'metal', pbr: { base_color: '#8a9aa8', metallic: 0.88, roughness: 0.28, opacity: 1 }, visual_only: true, allowed_domains: ['*'], provenance: 'forgecad_builtin' },
  { schema_version: 'MaterialPreset@1', material_id: 'mat_automotive_paint', display_name: '亮面涂层', category: 'coating', pbr: { base_color: '#3d78b8', metallic: 0.38, roughness: 0.2, opacity: 1 }, visual_only: true, allowed_domains: ['*'], provenance: 'forgecad_builtin' },
]

export {
  CONCEPT_FAMILY_SUGGESTIONS,
  DEFAULT_AGENT_CLARIFICATION_OPTIONS,
  DEFAULT_AGENT_MATERIAL_PRESETS,
  DEFAULT_CONCEPT_BRIEF,
}
