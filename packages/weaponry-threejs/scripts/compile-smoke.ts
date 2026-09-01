import fullSourceSpec from '../benchmark/dragonfang-like-objects-sculpt-spec.json' with { type: 'json' }
import { knifeSceneProgramFixture } from '../fixtures/knife-scene-program.fixture.ts'
import { img2threejsGroundBladeFixture } from '../fixtures/img2threejs-ground-blade.fixture.ts'
import type { KnifeCurveBasis, KnifeSceneProgram, KnifeSection } from '../src/knife-scene-program.ts'
import {
  KNIFE_VIEW_IDS,
  compileImg2ThreeJsSourceEnvelope,
  compileKnifeScene,
  createKnifeViewCamera,
  createKnifeViewRig,
  evaluateKnifeRig,
  importImg2ThreeJsKnifeSpec,
  KnifeSceneCompileError,
  rasterizeKnifeMask,
  type KnifeProjectionResult,
} from '../src/index.ts'
import {
  calibrateKnifeViewRig,
  createKnifeViewRigFromCalibration,
  validateKnifeViewCalibration,
} from '../src/knife-view-evaluation.ts'

const first = compileKnifeScene(knifeSceneProgramFixture)
const second = compileKnifeScene(knifeSceneProgramFixture)

if (first.deterministic_fingerprint !== second.deterministic_fingerprint) {
  throw new Error('determinism check failed: repeated compile fingerprints differ')
}
if (first.sections.length !== 4) throw new Error(`expected four fixture sections, got ${first.sections.length}`)
if (first.longitudinal_segments !== 64) {
  throw new Error(`expected 64 deterministic longitudinal segments, got ${first.longitudinal_segments}`)
}
if (first.parts.map((part) => part.part_id).join(',') !== 'blade-body,cutting-edge,guard,grip,pommel') {
  throw new Error('stable blade/assembly part ordering/IDs drifted')
}
if (first.parts.some((part) => part.mesh.userData.part_id !== part.part_id || part.material.userData.material_zone_id !== part.material_zone_id)) {
  throw new Error('stable part/material IDs are not bound to derived objects')
}
if (first.parts.some((part) => !part.geometry.getAttribute('partIdHash') || !part.geometry.getAttribute('materialZoneHash'))) {
  throw new Error('stable part/material ID attributes are missing from derived geometry')
}
if (first.assembly_status !== 'COMPILED' || first.assembly_parts.map((part) => part.part_id).join(',') !== 'guard,grip,pommel') {
  throw new Error('bounded guard/grip/pommel assembly did not compile')
}
if (first.assembly_parts.some((part) => !part.assembly_primitive || !part.center || part.mesh.userData.assembly_primitive !== part.assembly_primitive)) {
  throw new Error('assembly primitive metadata is not bound to derived meshes')
}
const belly = first.sections.find((section) => section.section_id === 'section-belly')
if (!belly || belly.top_thickness === belly.bottom_thickness || belly.edge_radius === belly.spine_radius) {
  throw new Error('fixture did not exercise asymmetric thickness and offset geometry')
}
if (first.renderer_invoked || first.quality_status !== 'NOT_RUN') {
  throw new Error('compiler must not claim browser rendering or quality')
}

function makeMultiPointProgram(
  basis: KnifeCurveBasis,
  pointCount: number,
  sectionCount: number,
): KnifeSceneProgram {
  const curve = (curveId: string, side: 1 | -1) => ({
    curve_id: curveId,
    basis,
    control_points: Array.from({ length: pointCount }, (_, index) => {
      const u = index / Math.max(pointCount - 1, 1)
      const railOffset = 0.08 * (1 - u) + 0.24 * Math.sin(Math.PI * u)
      return [
        -1 + 2 * u,
        side * railOffset,
        side * 0.018 * Math.sin(Math.PI * u),
      ] as const
    }),
  })
  const sections = Array.from({ length: sectionCount }, (_, index) => {
    const u = index / Math.max(sectionCount - 1, 1)
    const role: KnifeSection['role'] = index === 0
      ? 'root'
      : index === sectionCount - 1
        ? 'tip'
        : index === Math.floor(sectionCount / 2)
          ? 'belly'
          : 'intermediate'
    return {
      section_id: index === 0
        ? 'section-root'
        : index === sectionCount - 1
          ? 'section-tip'
          : `section-${index.toString().padStart(2, '0')}`,
      role,
      u,
      half_width: 0.11 + 0.23 * Math.sin(Math.PI * u) + 0.015 * (1 - u),
      thickness: 0.018 + 0.054 * (1 - u),
      edge_offset: -0.1 * Math.sin(Math.PI * u),
      spine_offset: 0.028 * Math.sin(Math.PI * u),
      asymmetry: 0.16 * Math.sin(Math.PI * u),
      twist: 0.05 * Math.sin(Math.PI * u),
    }
  })
  return {
    ...knifeSceneProgramFixture,
    asset_id: `compiler-smoke-${basis}-${pointCount}-${sectionCount}`,
    blade_surface: {
      ...knifeSceneProgramFixture.blade_surface,
      spine_curve: curve(`smoke-${basis}-spine-${pointCount}`, 1),
      cutting_edge_curve: curve(`smoke-${basis}-edge-${pointCount}`, -1),
      sections,
    },
  }
}

const multiSectionCases: string[] = []
for (const basis of ['bezier', 'nurbs-like'] as const) {
  for (const pointCount of [6, 12] as const) {
    for (const sectionCount of [6, 12] as const) {
      const program = makeMultiPointProgram(basis, pointCount, sectionCount)
      const compiled = compileKnifeScene(program)
      const repeated = compileKnifeScene(program)
      if (compiled.sections.length !== sectionCount || compiled.parts.length !== 5 || compiled.triangle_count <= 0) {
        throw new Error(`multi-section ${basis}/${pointCount}/${sectionCount} compile output is incomplete`)
      }
      if (compiled.deterministic_fingerprint !== repeated.deterministic_fingerprint) {
        throw new Error(`multi-section ${basis}/${pointCount}/${sectionCount} compile is not deterministic`)
      }
      if (compiled.sections[0].role !== 'root' || compiled.sections.at(-1)?.role !== 'tip') {
        throw new Error(`multi-section ${basis}/${pointCount}/${sectionCount} root/tip section roles drifted`)
      }
      multiSectionCases.push(`${basis}:${pointCount}points/${sectionCount}sections`)
    }
  }
}

function expectCompileRejection(program: KnifeSceneProgram, expectedCode: string, label: string): void {
  try {
    compileKnifeScene(program)
  } catch (error) {
    if (error instanceof KnifeSceneCompileError && error.code === expectedCode) return
    throw new Error(`${label} rejected with an unexpected compiler error`)
  }
  throw new Error(`${label} exceeded the closed compiler budget without rejection`)
}

expectCompileRejection(makeMultiPointProgram('bezier', 65, 6), 'INVALID_CURVE', '65-point bezier curve')
expectCompileRejection(makeMultiPointProgram('nurbs-like', 6, 33), 'INVALID_PROGRAM', '33-section blade surface')

function makeSemanticAssemblyProgram(): KnifeSceneProgram {
  return {
    ...knifeSceneProgramFixture,
    asset_id: 'compiler-smoke-semantic-assembly',
    assembly: {
      guard: {
        primitive: 'guard',
        style: 'dragon-guard',
        part_id: 'guard',
        center: [-1.04, 0.02, 0],
        span: 0.46,
        thickness: 0.08,
        depth: 0.16,
        jaw_gap: 0.12,
        upper_jaw: { span: 0.38, thickness: 0.07, depth: 0.1, offset_y: 0, offset_z: 0, curvature: 0.06 },
        lower_jaw: { span: 0.38, thickness: 0.07, depth: 0.1, offset_y: 0, offset_z: 0, curvature: 0.06 },
        horns: [
          { feature_id: 'horn-left', side: -1, length: 0.18, radius: 0.05, sweep: 0.28, offset_z: 0.08 },
          { feature_id: 'horn-right', side: 1, length: 0.18, radius: 0.05, sweep: -0.28, offset_z: 0.08 },
        ],
        eye_sockets: [
          { feature_id: 'eye-left', side: -1, radius: 0.055, depth: 0.08, offset_y: -0.1, offset_z: 0.09 },
          { feature_id: 'eye-right', side: 1, radius: 0.055, depth: 0.08, offset_y: 0.1, offset_z: 0.09 },
        ],
      },
      grip: {
        primitive: 'grip',
        style: 'segmented-grip',
        part_id: 'grip',
        center: [-1.47, 0.01, 0],
        length: 0.7,
        radius: 0.12,
        taper: -0.2,
        facets: 10,
        centerline: [
          [-0.35, 0, 0],
          [-0.12, 0.035, 0.015],
          [0.1, -0.025, 0.025],
          [0.35, 0.01, 0],
        ],
        segments: [
          { feature_id: 'grip-wrap-a', start_u: 0, end_u: 0.34, radius_scale: 1 },
          { feature_id: 'grip-wrap-b', start_u: 0.34, end_u: 0.67, radius_scale: 0.96 },
          { feature_id: 'grip-wrap-c', start_u: 0.67, end_u: 1, radius_scale: 1.04 },
        ],
        metal_frames: [
          { feature_id: 'grip-frame-a', at: 0.04, width: 0.08, thickness: 0.018 },
          { feature_id: 'grip-frame-b', at: 0.5, width: 0.08, thickness: 0.018 },
          { feature_id: 'grip-frame-c', at: 0.96, width: 0.08, thickness: 0.018 },
        ],
        fasteners: [
          { feature_id: 'grip-fastener-a', at: 0.2, side: 1, radius: 0.018, depth: 0.05 },
          { feature_id: 'grip-fastener-b', at: 0.5, side: -1, radius: 0.018, depth: 0.05 },
          { feature_id: 'grip-fastener-c', at: 0.8, side: 1, radius: 0.018, depth: 0.05 },
        ],
      },
      pommel: {
        primitive: 'pommel',
        style: 'hooked-pommel',
        part_id: 'pommel',
        center: [-1.88, 0.01, 0],
        length: 0.24,
        radius: 0.13,
        depth: 0.18,
        hook: { length: 0.28, radius: 0.04, bend: 0.8, direction: 1 },
        gem_seat: {
          feature_id: 'pommel-gem-seat',
          radius: 0.055,
          depth: 0.06,
          offset_x: 0,
          offset_y: 0,
          offset_z: 0.01,
          axis: 'z',
        },
      },
      fasteners: [],
      gems: [],
      reliefs: [],
    },
  }
}

const semanticAssemblyFirst = compileKnifeScene(makeSemanticAssemblyProgram())
const semanticAssemblySecond = compileKnifeScene(makeSemanticAssemblyProgram())
if (semanticAssemblyFirst.deterministic_fingerprint !== semanticAssemblySecond.deterministic_fingerprint) {
  throw new Error('semantic guard/grip/pommel assembly is not deterministic')
}
if (semanticAssemblyFirst.parts.map((part) => part.part_id).join(',') !== 'blade-body,cutting-edge,guard,grip,pommel') {
  throw new Error('semantic assembly changed stable part ordering')
}
const semanticAssemblyDescriptors = new Map(
  semanticAssemblyFirst.assembly_parts.map((part) => [part.part_id, part.geometry.userData.descriptor as Record<string, unknown>]),
)
if (semanticAssemblyDescriptors.get('guard')?.style !== 'dragon-guard'
  || semanticAssemblyDescriptors.get('guard')?.jaw_gap !== 0.12
  || semanticAssemblyDescriptors.get('guard')?.horn_count !== 2
  || semanticAssemblyDescriptors.get('guard')?.eye_socket_count !== 2
  || semanticAssemblyDescriptors.get('grip')?.style !== 'segmented-grip'
  || semanticAssemblyDescriptors.get('grip')?.segment_count !== 3
  || semanticAssemblyDescriptors.get('grip')?.metal_frame_count !== 3
  || semanticAssemblyDescriptors.get('grip')?.fastener_count !== 3
  || semanticAssemblyDescriptors.get('pommel')?.style !== 'hooked-pommel'
  || semanticAssemblyDescriptors.get('pommel')?.hook_length !== 0.28) {
  throw new Error('semantic assembly descriptors lost required feature counts')
}
if (semanticAssemblyFirst.assembly_parts.some((part) => part.geometry.getAttribute('position').count <= 0
  || part.mesh.userData.part_id !== part.part_id
  || part.material.userData.material_zone_id !== part.material_zone_id)) {
  throw new Error('semantic assembly geometry or stable ID binding is incomplete')
}
let segmentedGripFastenerCountRejected = false
try {
  const invalid = makeSemanticAssemblyProgram()
  const invalidProgram = {
    ...invalid,
    assembly: {
      ...invalid.assembly,
      grip: { ...invalid.assembly!.grip!, fasteners: [] },
    },
  } as unknown as KnifeSceneProgram
  compileKnifeScene(invalidProgram)
} catch (error) {
  segmentedGripFastenerCountRejected = error instanceof KnifeSceneCompileError && error.code === 'INVALID_PROGRAM'
}
if (!segmentedGripFastenerCountRejected) throw new Error('segmented-grip accepted an out-of-range fastener count')

const rig = createKnifeViewRig({ frame_width: 128, frame_height: 96 })
if (rig.views.map((view) => view.view_id).join(',') !== KNIFE_VIEW_IDS.join(',')) {
  throw new Error('fixed rig view IDs/order drifted')
}
for (const viewId of KNIFE_VIEW_IDS) {
  const camera = createKnifeViewCamera(rig, viewId)
  if (camera.userData.renderer_invoked || camera.userData.quality_status !== 'NOT_RUN') {
    throw new Error(`camera ${viewId} crossed the rendering/quality boundary`)
  }
}
const viewEvaluation = evaluateKnifeRig(first, rig)
if (viewEvaluation.views.length !== KNIFE_VIEW_IDS.length || viewEvaluation.receipt.quality_status !== 'NOT_RUN') {
  throw new Error('eight-view structural evaluation receipt is incomplete')
}
if (viewEvaluation.views.some((view) => view.receipt.renderer_invoked || view.mask.receipt.rasterizer !== 'software-triangle-mask@2')) {
  throw new Error('view evaluation crossed the browser renderer boundary')
}

function makeClipRegressionProjection(
  viewId: typeof KNIFE_VIEW_IDS[number],
  vertices: KnifeProjectionResult['vertices'],
): KnifeProjectionResult {
  const view = rig.views.find((candidate) => candidate.view_id === viewId)
  if (!view) throw new Error(`missing fixed view ${viewId}`)
  return {
    schema_version: 'WeaponryThreeJsProjection@1',
    rig,
    view,
    part_ids: ['clip-regression-part'],
    material_zone_ids: ['clip-regression-material'],
    vertices,
    triangles: [{
      a: 0,
      b: 1,
      c: 2,
      part_index: 0,
      material_index: 0,
      part_id: 'clip-regression-part',
      material_zone_id: 'clip-regression-material',
    }],
    receipt: {
      schema_version: 'WeaponryThreeJsProjectionReceipt@1',
      rig_schema_version: rig.schema_version,
      rig_fingerprint: rig.deterministic_fingerprint,
      source_fingerprint: '0000000000000000',
      view_id: view.view_id,
      frame_width: rig.frame_width,
      frame_height: rig.frame_height,
      projection_type: view.projection,
      projected_vertex_count: vertices.length,
      projected_triangle_count: 1,
      clip_visible_vertex_count: vertices.filter((vertex) => vertex.clip_visible).length,
      renderer_invoked: false,
      quality_status: 'NOT_RUN',
      deterministic_fingerprint: '0000000000000000',
    },
  }
}

const orthographicOutsideMask = rasterizeKnifeMask(makeClipRegressionProjection('FRONT', [
  { x_px: 36, y_px: 24, depth_ndc: 2, clip_visible: false, clip_outcode: 0x20 },
  { x_px: 92, y_px: 24, depth_ndc: 2, clip_visible: false, clip_outcode: 0x20 },
  { x_px: 64, y_px: 72, depth_ndc: 2, clip_visible: false, clip_outcode: 0x20 },
]))
if (orthographicOutsideMask.receipt.covered_pixel_count !== 0) {
  throw new Error('orthographic far/frustum-out triangle leaked into the software mask')
}

const perspectiveBehindMask = rasterizeKnifeMask(makeClipRegressionProjection('FPS_HOLD', [
  { x_px: 36, y_px: 24, depth_ndc: 0, clip_visible: false, clip_outcode: 0x40 },
  { x_px: 92, y_px: 24, depth_ndc: 0, clip_visible: false, clip_outcode: 0x40 },
  { x_px: 64, y_px: 72, depth_ndc: 0, clip_visible: false, clip_outcode: 0x40 },
]))
if (perspectiveBehindMask.receipt.covered_pixel_count !== 0) {
  throw new Error('perspective camera-behind triangle leaked into the software mask')
}

const crossingFrustumMask = rasterizeKnifeMask(makeClipRegressionProjection('FRONT', [
  { x_px: -32, y_px: 48, depth_ndc: 0, clip_visible: false, clip_outcode: 0x01 },
  { x_px: 160, y_px: 48, depth_ndc: 0, clip_visible: false, clip_outcode: 0x02 },
  { x_px: 64, y_px: -24, depth_ndc: 0, clip_visible: false, clip_outcode: 0x04 },
]))
if (crossingFrustumMask.receipt.covered_pixel_count === 0) {
  throw new Error('cross-frustum triangle was incorrectly rejected because all vertices were individually outside')
}
const secondViewEvaluation = evaluateKnifeRig(compileKnifeScene(knifeSceneProgramFixture), rig)
if (viewEvaluation.receipt.deterministic_fingerprint !== secondViewEvaluation.receipt.deterministic_fingerprint) {
  throw new Error('determinism check failed: repeated eight-view evaluation fingerprints differ')
}

const calibrated = calibrateKnifeViewRig(first, {
  focus_part_ids: ['cutting-edge', 'blade-body'],
  frame_width: 128,
  frame_height: 96,
  margin: 0.08,
})
const repeatedCalibration = calibrateKnifeViewRig(compileKnifeScene(knifeSceneProgramFixture), {
  focus_part_ids: ['blade-body', 'cutting-edge'],
  frame_width: 128,
  frame_height: 96,
  margin: 0.08,
})
if (calibrated.calibration.focus_part_ids.join(',') !== 'blade-body,cutting-edge') {
  throw new Error('calibration focus IDs were not canonicalized')
}
if (calibrated.calibration.deterministic_fingerprint !== repeatedCalibration.calibration.deterministic_fingerprint
  || calibrated.rig.deterministic_fingerprint !== repeatedCalibration.rig.deterministic_fingerprint) {
  throw new Error('determinism check failed: repeated baseline calibration fingerprints differ')
}
if (!Object.isFrozen(calibrated.calibration)
  || !Object.isFrozen(calibrated.calibration.focus_part_ids)
  || !Object.isFrozen(calibrated.calibration.world_aabb)
  || !Object.isFrozen(calibrated.rig)
  || !Object.isFrozen(calibrated.receipt)) {
  throw new Error('calibration, receipt, and calibrated rig must be frozen')
}
if (validateKnifeViewCalibration(calibrated.calibration) !== calibrated.calibration.deterministic_fingerprint
  || calibrated.receipt.renderer_invoked
  || calibrated.receipt.quality_status !== 'NOT_RUN') {
  throw new Error('calibration receipt crossed the renderer or quality boundary')
}
const reusedCalibrationRig = createKnifeViewRigFromCalibration(calibrated.calibration)
if (reusedCalibrationRig.deterministic_fingerprint !== calibrated.rig.deterministic_fingerprint
  || reusedCalibrationRig.calibration.source_fingerprint !== first.deterministic_fingerprint
  || reusedCalibrationRig.views.some((view) => view.target.some((value, index) => value !== calibrated.calibration.center[index]))) {
  throw new Error('calibrated rig was not reproducibly reusable without candidate refit')
}
if (calibrated.rig.views.filter((view) => view.projection === 'orthographic').some((view) => view.ortho_height !== calibrated.calibration.ortho_height)) {
  throw new Error('calibrated orthographic views did not bind the baseline ortho height')
}
let missingFocusRejected = false
try {
  calibrateKnifeViewRig(first, { frame_width: 128, frame_height: 96 } as never)
} catch {
  missingFocusRejected = true
}
if (!missingFocusRejected) throw new Error('calibration silently selected focus parts when focus_part_ids was omitted')
for (const invalidFocus of [['blade-body', 'blade-body'], ['missing-part']]) {
  let rejected = false
  try {
    calibrateKnifeViewRig(first, { focus_part_ids: invalidFocus } as never)
  } catch {
    rejected = true
  }
  if (!rejected) throw new Error(`invalid calibration focus IDs were accepted: ${invalidFocus.join(',')}`)
}

const imported = importImg2ThreeJsKnifeSpec(img2threejsGroundBladeFixture)
const importedScene = compileKnifeScene(imported.program)
if (imported.receipt.upstream_revision !== '9fbd0ca5bbcc3b13bebe712745d6784d33db0b85') {
  throw new Error('bounded img2threejs import lost the pinned upstream revision')
}
if (imported.receipt.execution_performed || imported.receipt.network_used || importedScene.parts.length !== 2 || importedScene.assembly_status !== 'NOT_PRESENT') {
  throw new Error('bounded img2threejs import crossed its static adapter boundary')
}

const fullImported = importImg2ThreeJsKnifeSpec(fullSourceSpec as unknown)
const fullEnvelope = fullImported.program.source_envelope
if (!fullEnvelope) throw new Error('full img2threejs fixture did not produce a closed source envelope')
const fullCompatibility = compileImg2ThreeJsSourceEnvelope(fullEnvelope)
const fullScene = compileKnifeScene(fullImported.program)
if (fullImported.receipt.imported_component_ids.length !== 7
  || fullImported.receipt.mapped_component_ids.length !== 7
  || fullImported.receipt.preserved_component_ids.length !== 7
  || fullImported.receipt.unsupported_component_ids.length !== 0
  || fullImported.receipt.imported_material_ids.length !== 4
  || fullImported.receipt.mapped_material_ids.length !== 4
  || fullImported.receipt.unsupported_material_ids.length !== 0
  || fullCompatibility.triangle_count !== 1049
  || fullScene.triangle_count !== 1049
  || fullScene.assembly_status !== 'COMPILED') {
  throw new Error('full img2threejs fixture did not preserve the closed 7-component/4-material exact mesh cohort')
}
if (fullEnvelope.components.map((component) => component.component_id).join(',') !== 'blade,guard-dragon-head,grip,pommel,grip-fastener,dragon-eye,blade-relief'
  || fullEnvelope.materials.map((material) => material.material_id).join(',') !== 'blade-red,ornament-gold,grip-black,ruby-accent') {
  throw new Error('full img2threejs source order drifted')
}
if (fullImported.receipt.source_identity.revision !== '9fbd0ca5bbcc3b13bebe712745d6784d33db0b85'
  || fullImported.receipt.source_identity.tree !== '0ee3c2a6d781407808df98b33174539842f85fcc') {
  throw new Error('full img2threejs source identity is not pinned')
}

console.log(JSON.stringify({
  compiler: 'weaponry-threejs-knife-compiler@1',
  asset_id: knifeSceneProgramFixture.asset_id,
  sections: first.sections.length,
  parts: first.parts.map((part) => ({ part_id: part.part_id, material_zone_id: part.material_zone_id })),
  triangles: first.triangle_count,
  longitudinal_segments: first.longitudinal_segments,
  deterministic_fingerprint: first.deterministic_fingerprint,
  assembly: first.assembly_parts.map((part) => ({
    part_id: part.part_id,
    primitive: part.assembly_primitive,
    center: part.center,
    triangles: part.geometry.getIndex() ? part.geometry.getIndex()!.count / 3 : 0,
  })),
  renderer_invoked: first.renderer_invoked,
  quality_status: first.quality_status,
  multi_section_support: {
    cases: multiSectionCases,
    required_window: '6-12 points and sections',
    max_control_points: 64,
    max_sections: 32,
    over_control_points_rejected: true,
    over_sections_rejected: true,
  },
  semantic_assembly_support: {
    styles: ['dragon-guard', 'segmented-grip', 'hooked-pommel'],
    dragon_guard: { jaw_gap: 0.12, horns: 2, eye_sockets: 2 },
    segmented_grip: { segments: 3, metal_frames: 3, fasteners: 3 },
    hooked_pommel: { hook_length: 0.28, gem_seat: true },
    invalid_fastener_count_rejected: segmentedGripFastenerCountRejected,
  },
  fixed_view_evaluation: {
    rig_fingerprint: rig.deterministic_fingerprint,
    view_ids: viewEvaluation.receipt.view_ids,
    view_fingerprints: viewEvaluation.views.map((view) => ({
      view_id: view.view_id,
      projection: view.receipt.projection_fingerprint,
      mask: view.receipt.mask_fingerprint,
      covered_pixels: view.mask.receipt.covered_pixel_count,
    })),
    deterministic_fingerprint: viewEvaluation.receipt.deterministic_fingerprint,
    renderer_invoked: viewEvaluation.receipt.renderer_invoked,
    quality_status: viewEvaluation.receipt.quality_status,
  },
  calibrated_view_rig: {
    calibration_schema_version: calibrated.calibration.schema_version,
    focus_part_ids: calibrated.calibration.focus_part_ids,
    world_aabb: calibrated.calibration.world_aabb,
    center: calibrated.calibration.center,
    ortho_height: calibrated.calibration.ortho_height,
    depth_span: calibrated.calibration.depth_span,
    calibration_fingerprint: calibrated.calibration.deterministic_fingerprint,
    rig_fingerprint: calibrated.rig.deterministic_fingerprint,
    receipt_fingerprint: calibrated.receipt.deterministic_fingerprint,
    renderer_invoked: calibrated.receipt.renderer_invoked,
    quality_status: calibrated.receipt.quality_status,
  },
  img2threejs_import: {
    source_schema_version: imported.receipt.source_schema_version,
    station_count: imported.receipt.imported_station_count,
    ignored_component_ids: imported.receipt.ignored_component_ids,
    triangles: importedScene.triangle_count,
    execution_performed: imported.receipt.execution_performed,
    quality_status: imported.receipt.quality_status,
  },
  img2threejs_full_import: {
    source_identity: fullImported.receipt.source_identity,
    imported_component_count: fullImported.receipt.imported_component_ids.length,
    mapped_component_count: fullImported.receipt.mapped_component_ids.length,
    preserved_component_count: fullImported.receipt.preserved_component_ids.length,
    unsupported_component_ids: fullImported.receipt.unsupported_component_ids,
    imported_material_count: fullImported.receipt.imported_material_ids.length,
    mapped_material_count: fullImported.receipt.mapped_material_ids.length,
    unsupported_material_ids: fullImported.receipt.unsupported_material_ids,
    component_mappings: fullImported.receipt.component_mappings,
    material_mappings: fullImported.receipt.material_mappings,
    compatibility_triangles: fullCompatibility.triangle_count,
    scene_triangles: fullScene.triangle_count,
    assembly_status: fullScene.assembly_status,
    deterministic_fingerprint: fullImported.receipt.deterministic_fingerprint,
  },
}))
