/**
 * User-facing names for stable Agent part roles.
 *
 * `role` remains the durable AssemblyGraph/ChangeSet key.  This boundary only
 * translates known roles for the zero-basis workbench and never infers a
 * domain, function, safety property, or editable capability from an unknown
 * string.
 */
export const PART_ROLE_LABELS: Readonly<Record<string, string>> = {
  primary_body: '主体外壳',
  secondary_body: '辅助外壳',
  mobility: '底部支撑组件',
  trim: '装饰面板',
  transparent: '透明外罩',
  body_shell: '车身外壳',
  cabin: '座舱',
  wheel_or_track: '车轮或履带',
  wheel: '车轮',
  track: '履带',
  lighting: '灯光组件',
  trim_panel: '装饰面板',
  fuselage: '机身',
  cockpit_canopy: '座舱罩',
  main_wing: '主翼',
  tail_surface: '尾翼',
  nacelle: '发动机舱',
  base: '底座',
  shoulder_joint: '肩部关节',
  upper_link: '上臂连杆',
  elbow_joint: '肘部关节',
  joint_elbow: '肘部关节',
  forearm_link: '前臂连杆',
  wrist_joint: '腕部关节',
  end_effector: '末端执行器',
  body: '主体',
  body_panel: '主体面板',
  surface_panel: '表面面板',
  visual_panel: '外观面板',
  visual_groove: '外观分缝',
  visual_guard: '外观护板',
  visual_light_strip: '灯带点缀',
  visual_cable_slot: '线缆槽点缀',
  visual_vent: '散热孔点缀',
  visual_fastener: '紧固件点缀',
  // C106 robotic-arm production Recipe roles.  Keep these exact mappings
  // alongside the stable AssemblyGraph keys so the workbench can describe a
  // production concept asset without showing an internal identifier.
  base_form: '底座主体',
  turntable: '旋转底座',
  joint_housing: '关节外壳',
  link_armor: '连杆护甲',
  cable_harness: '线缆束',
  end_effector_form: '末端执行器',
  surface_trim: '表面装饰',
}

const ROLE_PATTERNS: ReadonlyArray<readonly [RegExp, string]> = [
  [/cockpit|canopy/, '座舱罩'],
  [/fuselage/, '机身'],
  [/wing/, '机翼'],
  [/tail|fin/, '尾翼'],
  [/nacelle|engine/, '发动机舱'],
  [/wheel/, '车轮'],
  [/track/, '履带'],
  [/shoulder.*joint|joint.*shoulder/, '肩部关节'],
  [/elbow.*joint|joint.*elbow/, '肘部关节'],
  [/wrist.*joint|joint.*wrist/, '腕部关节'],
  [/joint/, '关节'],
  [/upper.*link/, '上臂连杆'],
  [/forearm.*link/, '前臂连杆'],
  [/end.*effector|gripper|tool/, '末端执行器'],
  [/base/, '底座'],
  [/cabin/, '座舱'],
  [/light/, '灯光组件'],
  [/transparent|glass/, '透明外罩'],
  [/visual.*panel/, '外观面板'],
  [/visual.*groove|groove/, '外观分缝'],
  [/visual.*guard|guard/, '外观护板'],
  [/visual.*cable.*slot|cable.*slot/, '线缆槽点缀'],
  [/visual.*vent|vent/, '散热孔点缀'],
  [/visual.*fastener|fastener/, '紧固件点缀'],
  [/visual.*light|light/, '灯带点缀'],
  [/trim|panel/, '装饰面板'],
  [/body|shell/, '主体外壳'],
]

function normalizedRole(role: string | null | undefined): string {
  return typeof role === 'string' ? role.trim().toLowerCase().replaceAll('-', '_') : ''
}

/** Return a stable, Chinese display name without exposing internal role IDs. */
export function displayPartRole(role: string | null | undefined): string {
  const normalized = normalizedRole(role)
  if (!normalized) return '未命名部件'
  const exact = PART_ROLE_LABELS[normalized]
  if (exact) return exact
  const pattern = ROLE_PATTERNS.find(([matcher]) => matcher.test(normalized))
  return pattern?.[1] ?? '未命名部件'
}

/** Joint affordances are only enabled for explicit stable joint role names. */
export function isJointPartRole(role: string | null | undefined): boolean {
  return /(^|_)joint(_|$)|shoulder_joint|elbow_joint|wrist_joint/.test(normalizedRole(role))
}
