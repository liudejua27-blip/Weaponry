import { displayPartRole, isJointPartRole } from './partRoleLabels.js'

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

export function runPartRoleLabelsSmoke(): void {
  const cases = [
    ['primary_body', '主体外壳'],
    ['wheel_or_track', '车轮或履带'],
    ['cockpit_canopy', '座舱罩'],
    ['cargo_wing_left', '机翼'],
    ['shoulder_joint', '肩部关节'],
    ['joint_elbow', '肘部关节'],
    ['visual_panel_1', '外观面板'],
    ['visual_groove_1', '外观分缝'],
    ['visual_cable_slot_1', '线缆槽点缀'],
    ['visual_vent_1', '散热孔点缀'],
    ['visual_fastener_1', '紧固件点缀'],
    ['base_form', '底座主体'],
    ['turntable', '旋转底座'],
    ['joint_housing', '关节外壳'],
    ['link_armor', '连杆护甲'],
    ['cable_harness', '线缆束'],
    ['end_effector_form', '末端执行器'],
    ['surface_trim', '表面装饰'],
  ] as const
  for (const [role, expected] of cases) {
    assert(displayPartRole(role) === expected, `role ${role} should display as ${expected}`)
  }
  assert(displayPartRole('vendor_private_thing') === '未命名部件', 'unknown roles must not be guessed')
  assert(isJointPartRole('shoulder_joint'), 'known joint role must expose the joint-only action')
  assert(!isJointPartRole('fuselage'), 'non-joint role must not expose a joint-only action')
}
