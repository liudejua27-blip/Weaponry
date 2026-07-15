import { isValidElement, type ComponentProps, type ReactElement, type ReactNode } from 'react'
import {
  COMPONENT_CATEGORIES,
  ComponentDrawer,
  type ComponentDrawerProps,
} from './ComponentDrawer.js'
import { ExportDrawer, type ExportDrawerProps } from './ExportDrawer.js'
import { MaterialDrawer } from './MaterialDrawer.js'
import { QualityDrawer, type QualityDrawerProps } from './QualityDrawer.js'

function collectText(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(collectText).join(' ')
  if (!isValidElement(node)) return ''
  if (typeof node.type === 'function') return collectText((node.type as (props: unknown) => ReactNode)(node.props))
  return collectText((node.props as { children?: ReactNode }).children)
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

function hasProp(node: ReactNode, prop: string, value?: unknown): boolean {
  if (node === null || node === undefined || typeof node === 'boolean') return false
  if (Array.isArray(node)) return node.some((child) => hasProp(child, prop, value))
  if (!isValidElement(node)) return false
  if (typeof node.type === 'function') return hasProp((node.type as (props: unknown) => ReactNode)(node.props), prop, value)
  const props = node.props as Record<string, unknown> & { children?: ReactNode }
  if (prop in props && (value === undefined || props[prop] === value)) return true
  return hasProp(props.children, prop, value)
}

function allButtonsHaveType(node: ReactNode): boolean {
  if (node === null || node === undefined || typeof node === 'boolean') return true
  if (Array.isArray(node)) return node.every(allButtonsHaveType)
  if (!isValidElement(node)) return true
  if (typeof node.type === 'function') return allButtonsHaveType((node.type as (props: unknown) => ReactNode)(node.props))
  const props = node.props as Record<string, unknown> & { children?: ReactNode }
  if (node.type === 'button' && props.type !== 'button') return false
  return allButtonsHaveType(props.children)
}

function buttons(node: ReactNode): Array<ReactElement> {
  if (node === null || node === undefined || typeof node === 'boolean') return []
  if (Array.isArray(node)) return node.flatMap(buttons)
  if (!isValidElement(node)) return []
  if (typeof node.type === 'function') return buttons((node.type as (props: unknown) => ReactNode)(node.props))
  const props = node.props as { children?: ReactNode }
  return (node.type === 'button' ? [node] : []).concat(buttons(props.children))
}

const module = {
  manifest: {
    module_id: 'module_shell_smoke',
    pack_id: 'pack_smoke',
    category: 'core_shell',
    asset_id: 'asset_shell_smoke',
    sha256: 'sha256-smoke',
    bounds_mm: [10, 20, 30],
    triangle_count: 120,
    material_slots: ['metal'],
    connectors: [],
  },
  logical_path: 'catalog/smoke.glb',
  object_path: 'objects/smoke.glb',
  byte_size: 128,
  mime_type: 'model/gltf-binary',
  created_at: '2026-07-13T00:00:00Z',
  catalog_metadata: {
    display_name: '核心外壳示例',
    description: '用于抽屉 smoke 的展示组件',
    tags: ['smoke'],
    catalog_path: '概念组件/核心外壳',
    origin_claim: 'self_declared_original',
    creator_name: 'ForgeCAD',
    review_status: 'approved',
    reviewer_name: 'reviewer',
    reviewed_at: '2026-07-13T00:00:00Z',
    updated_at: '2026-07-13T00:00:00Z',
  },
} as ComponentDrawerProps['displayedComponents'][number]

const componentProps: ComponentDrawerProps = {
  mode: 'all',
  selectedModuleLabel: '核心外壳',
  componentCategory: 'all',
  reviewStatusFilter: '',
  categories: COMPONENT_CATEGORIES,
  filterCounts: { all: 1, installed: 0, compatible: 1, favorites: 0, recent: 0 },
  query: '',
  displayedComponents: [module],
  totalModuleCount: 1,
  selectedLibraryModule: module,
  selectedLibraryModuleId: module.manifest.module_id,
  selectedNode: { node_id: 'node_shell', module_id: module.manifest.module_id },
  selectedModuleCategory: 'core_shell',
  graphNodes: [{ node_id: 'node_shell', module_id: module.manifest.module_id }],
  favoriteModuleIds: [],
  thumbnailFailures: new Set(),
  canReplaceSelected: false,
  expanded: true,
  loading: false,
  legacyDesignReadOnly: false,
  agentAssetActive: false,
  qualityStatusFor: () => 'passed',
  onResizeStart: () => undefined,
  onCategoryChange: () => undefined,
  onReviewStatusChange: () => undefined,
  onQueryChange: () => undefined,
  onModeToggle: () => undefined,
  onClose: () => undefined,
  onSelectModule: () => undefined,
  onToggleFavorite: () => undefined,
  onThumbnailError: () => undefined,
  onLocateModule: () => undefined,
  onPreviewReplace: () => undefined,
  onDiscardReplacement: () => undefined,
  onConfirmReplacement: () => undefined,
  thumbnailUrl: () => '/smoke.glb',
}

const exportProps: ExportDrawerProps = {
  exportPurpose: 'presentation',
  exportPurposeOptions: [
    { id: 'presentation', title: '展示设计', description: '用于展示', format: 'PNG' },
    { id: 'production', title: '游戏 / 影视项目', description: '展示模型', format: 'GLB' },
  ],
  agentAssetActive: false,
  activeAgentAssetVersion: null,
  activeDesignIdle: true,
  activeVersionLabel: 'v1',
  originLabel: '本人原创声明',
  hasLegacyVersion: true,
  loading: false,
  onClose: () => undefined,
  onPurposeChange: () => undefined,
  onExport: () => undefined,
  onDownloadAgentGlb: () => undefined,
  renderSet: null,
  renderLoading: false,
  renderPackageLoading: false,
  onRenderViews: () => undefined,
  onDownloadRenderView: () => undefined,
  onDownloadRenderPackage: () => undefined,
}

const qualityProps: QualityDrawerProps = {
  agentAssetActive: false,
  activeAgentAssetVersion: null,
  agentQualityReport: null,
  agentAssetChangeSet: null,
  graphReady: true,
  legacyVersionReady: true,
  legacyQualityStatus: 'passed',
  legacyFindings: [],
  loading: false,
  onClose: () => undefined,
  onFocusLegacyFinding: () => undefined,
  onInspectAgentAsset: () => undefined,
  onRunLegacyInspection: () => undefined,
}

const agentRenderSet = {
  schema_version: 'AgentAssetRenderSet@1',
  asset_version_id: 'assetver_smoke',
  renderer_id: 'forgecad-agent-software-raster@1',
  width: 128,
  height: 128,
  views: ['iso', 'front', 'side', 'top', 'exploded_iso'].map((view_id) => ({
    schema_version: 'AgentAssetRenderView@1',
    asset_version_id: 'assetver_smoke',
    view_id,
    camera_view: view_id === 'exploded_iso' ? 'iso' : view_id,
    presentation_mode: view_id === 'exploded_iso' ? 'exploded' : 'standard',
    background_mode: 'transparent',
    part_ids: view_id === 'exploded_iso' ? ['part_smoke_body', 'part_smoke_cabin'] : [],
    mime_type: 'image/png',
    width: 128,
    height: 128,
    png_base64: 'cG5n',
    sha256: 'a'.repeat(64),
    byte_size: 96,
    readback_status: 'passed',
  })),
  render_set_sha256: 'b'.repeat(64),
  exploded_view_available: true,
  exploded_unavailable_reason: null,
  render_set_byte_size: 480,
  rendered_at: '2026-07-13T00:00:00Z',
} as ExportDrawerProps['renderSet']

export function runWorkbenchDrawersSmoke(): void {
  const componentText = collectText(ComponentDrawer(componentProps))
  const materialText = collectText(MaterialDrawer({
    materialPresets: [{
      schema_version: 'MaterialPreset@1', material_id: 'mat_smoke', display_name: '磨砂金属', category: 'metal', pbr: { base_color: '#445566', metallic: 0.5, roughness: 0.5, opacity: 1 }, visual_only: true, allowed_domains: ['vehicle_concept'], provenance: 'forgecad_builtin',
    }],
    selectedMaterialId: 'mat_smoke',
    detailDensity: 50,
    onMaterialChange: () => undefined,
    onDetailDensityChange: () => undefined,
  }))
  const qualityText = collectText(QualityDrawer(qualityProps))
  const exportText = collectText(ExportDrawer(exportProps))
  assert(componentText.includes('核心外壳示例') && componentText.includes('预览替换'), 'component drawer must render asset details and replace action')
  assert(materialText.includes('磨砂金属') && materialText.includes('材质只描述外观'), 'material drawer must render visual-only boundary')
  assert(qualityText.includes('模型检查') && qualityText.includes('通过'), 'quality drawer must render current result')
  assert(exportText.includes('下载当前设计') && exportText.includes('展示设计'), 'legacy export drawer must render purpose choices')
  const agentExportText = collectText(ExportDrawer({
    ...exportProps,
    agentAssetActive: true,
    activeAgentAssetVersion: { asset_version_id: 'assetver_smoke', version_no: 1 } as NonNullable<ExportDrawerProps['activeAgentAssetVersion']>,
    renderSet: agentRenderSet,
  }))
  assert(agentExportText.includes('下载 3D 模型 (GLB)') && agentExportText.includes('概念视图') && agentExportText.includes('透视') && agentExportText.includes('爆炸概念图') && agentExportText.includes('透明背景') && agentExportText.includes('下载概念图包'), 'agent export drawer must render direct GLB, standard, exploded and ZIP concept-view actions')
  assert(!agentExportText.includes('展示设计') && !agentExportText.includes('交给三维设计师') && !agentExportText.includes('OBJ 模型') && !agentExportText.includes('概念源包'), 'agent export drawer must not expose legacy export choices')
  assert(hasProp(ComponentDrawer(componentProps), 'role', 'dialog'), 'component drawer must expose dialog semantics')
  assert(hasProp(QualityDrawer(qualityProps), 'aria-modal', 'true'), 'quality drawer must expose modal semantics')
  assert(hasProp(ExportDrawer(exportProps), 'aria-modal', 'true'), 'export drawer must expose modal semantics')
  assert(hasProp(ExportDrawer(exportProps), 'data-dialog-initial-focus', 'true'), 'export drawer must expose an initial focus control')
  assert(allButtonsHaveType(ComponentDrawer(componentProps)), 'component drawer buttons must declare type=button')
  assert(allButtonsHaveType(QualityDrawer(qualityProps)), 'quality drawer buttons must declare type=button')
  assert(allButtonsHaveType(ExportDrawer(exportProps)), 'export drawer buttons must declare type=button')
}

export function runMaterialZoneSmoke(): void {
  const presets: ComponentProps<typeof MaterialDrawer>['materialPresets'] = [
    {
      schema_version: 'MaterialPreset@1', material_id: 'mat_smoke_metal', display_name: '拉丝铝', category: 'metal',
      pbr: { base_color: '#8a9aaa', metallic: 0.8, roughness: 0.32, opacity: 1 }, visual_only: true,
      allowed_domains: ['vehicle_concept'], provenance: 'forgecad_builtin', visual_tags: ['车身', '金属'],
      thumbnail_fallback: 'parameter', texture_summary: [],
    },
    {
      schema_version: 'MaterialPreset@1', material_id: 'mat_smoke_missing', display_name: '参考织物', category: 'composite',
      pbr: { base_color: '#34495e', metallic: 0.1, roughness: 0.75, opacity: 1 }, visual_only: true,
      allowed_domains: ['vehicle_concept'], provenance: 'imported_reference', source: 'imported_reference', license: 'third_party',
      texture_summary: [{ texture_asset_id: 'asset_tex_aaaaaaaaaaaaaaaaaaaaaaaa', texture_role: 'base_color', exists: false, source: 'imported_reference', license: 'third_party' }],
    },
    {
      schema_version: 'MaterialPreset@1', material_id: 'mat_smoke_texture', display_name: '警示红涂层', category: 'coating',
      pbr: { base_color: '#c53a45', metallic: 0.2, roughness: 0.4, opacity: 1 }, visual_only: true,
      allowed_domains: ['vehicle_concept'], provenance: 'user_created', source: 'user_created', license: 'self_declared_original',
      texture_summary: [{ texture_asset_id: 'asset_tex_bbbbbbbbbbbbbbbbbbbbbbbb', texture_role: 'thumbnail', exists: true, source: 'user_created', license: 'self_declared_original' }],
    },
  ]
  const tree = MaterialDrawer({
    materialPresets: presets,
    selectedMaterialId: 'mat_smoke_texture',
    detailDensity: 50,
    selectedPartLabel: '已选部件 · 机身外壳',
    selectedZoneLabel: '材质区 primary',
    compatibilityOnly: false,
    onMaterialChange: () => undefined,
    onDetailDensityChange: () => undefined,
  })
  const text = collectText(tree)
  assert(text.includes('已选部件 · 机身外壳') && text.includes('材质区 primary'), 'material zone context must be visible')
  assert(text.includes('搜索视觉材质') && text.includes('复合材料') && text.includes('涂层'), 'material search and category filters must be visible')
  assert(text.includes('纹理已登记') && text.includes('使用参数外观') && text.includes('第三方参考'), 'material provenance and texture fallback must be visible')
  assert(hasProp(tree, 'aria-label', '搜索视觉材质'), 'material drawer must expose a search input')
  assert(allButtonsHaveType(tree), 'material zone buttons must declare type=button')
}

export function runMaterialZoneBindingSmoke(): void {
  let selectedZone = ''
  let previewedZone = ''
  const tree = MaterialDrawer({
    materialPresets: [{
      schema_version: 'MaterialPreset@1', material_id: 'mat_binding', display_name: '哑光复合材料', category: 'composite',
      pbr: { base_color: '#334455', metallic: 0.2, roughness: 0.7, opacity: 1 }, visual_only: true,
      allowed_domains: ['vehicle_concept'], provenance: 'forgecad_builtin',
    }],
    selectedMaterialId: 'mat_binding',
    detailDensity: 50,
    selectedPartLabel: '已选部件 · 机身外壳',
    selectedZoneLabel: '材质区 zone_body',
    materialZoneIds: ['zone_body', 'zone_trim'],
    selectedZoneId: 'zone_body',
    onMaterialChange: () => undefined,
    onDetailDensityChange: () => undefined,
    onZoneChange: (zoneId) => { selectedZone = zoneId },
    onPreviewMaterial: (_preset, zoneId) => { previewedZone = zoneId },
  })
  const renderedButtons = buttons(tree)
  const zoneButton = renderedButtons.find((button) => collectText(button).includes('饰条区'))
  const previewButton = renderedButtons.find((button) => collectText(button).includes('预览材质'))
  assert(zoneButton && typeof (zoneButton.props as { onClick?: () => void }).onClick === 'function', 'material zone selector must be actionable')
  assert(previewButton && typeof (previewButton.props as { onClick?: () => void }).onClick === 'function', 'material preview action must be actionable')
  ;(zoneButton.props as { onClick: () => void }).onClick()
  ;(previewButton.props as { onClick: () => void }).onClick()
  assert(selectedZone === 'zone_trim', 'material zone selection must preserve the stable zone id')
  assert(previewedZone === 'zone_body', 'material preview must send the selected zone id to the parent')
  assert(collectText(tree).includes('饰条区') && collectText(tree).includes('预览材质'), 'material zone binding UI must be visible')
}

export function runMaterialDomainFilterSmoke(): void {
  const presets: ComponentProps<typeof MaterialDrawer>['materialPresets'] = [
    {
      schema_version: 'MaterialPreset@1', material_id: 'mat_vehicle_only', display_name: '汽车漆', category: 'coating',
      pbr: { base_color: '#3d78b8', metallic: 0.3, roughness: 0.2, opacity: 1 }, visual_only: true,
      allowed_domains: ['vehicle_concept'], provenance: 'forgecad_builtin',
    },
    {
      schema_version: 'MaterialPreset@1', material_id: 'mat_aircraft_only', display_name: '航空复合材料', category: 'composite',
      pbr: { base_color: '#344451', metallic: 0.2, roughness: 0.6, opacity: 1 }, visual_only: true,
      allowed_domains: ['aircraft_concept'], provenance: 'forgecad_builtin',
    },
    {
      schema_version: 'MaterialPreset@1', material_id: 'mat_all_domains', display_name: '通用石墨', category: 'metal',
      pbr: { base_color: '#26313b', metallic: 0.7, roughness: 0.35, opacity: 1 }, visual_only: true,
      allowed_domains: ['future_weapon_prop', 'vehicle_concept', 'aircraft_concept', 'robotic_arm_concept'], provenance: 'forgecad_builtin',
    },
  ]
  const render = (domain: string) => collectText(MaterialDrawer({
    materialPresets: presets,
    selectedMaterialId: 'mat_all_domains',
    detailDensity: 50,
    activeDomain: domain,
    compatibilityOnly: true,
    onMaterialChange: () => undefined,
    onDetailDensityChange: () => undefined,
  }))
  const vehicleText = render('vehicle_concept')
  const aircraftText = render('aircraft_concept')
  const roboticText = render('robotic_arm_concept')
  assert(vehicleText.includes('汽车漆') && !vehicleText.includes('航空复合材料'), `vehicle filter must use allowed_domains: ${vehicleText}`)
  assert(aircraftText.includes('航空复合材料') && !aircraftText.includes('汽车漆'), `aircraft filter must use allowed_domains: ${aircraftText}`)
  assert(!roboticText.includes('汽车漆') && !roboticText.includes('航空复合材料') && roboticText.includes('通用石墨'), `robotic-arm filter must hide incompatible presets: ${roboticText}`)
  const switched = { value: true }
  const tree = MaterialDrawer({
    materialPresets: presets,
    selectedMaterialId: 'mat_all_domains',
    detailDensity: 50,
    activeDomain: 'vehicle_concept',
    compatibilityOnly: true,
    onMaterialChange: () => undefined,
    onDetailDensityChange: () => undefined,
    onCompatibilityChange: (value) => { switched.value = value },
  })
  const toggle = buttons(tree).find((button) => collectText(button).includes('全部视觉材质'))
  assert(toggle && typeof (toggle.props as { onClick?: () => void }).onClick === 'function', 'all-materials toggle must be actionable')
  ;(toggle.props as { onClick: () => void }).onClick()
  assert(switched.value === false, 'compatibility toggle must notify the parent')
}
