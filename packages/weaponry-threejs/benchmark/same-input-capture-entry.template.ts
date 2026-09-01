import * as THREE from 'three'

import objectSculptSpec from './dragonfang-like-objects-sculpt-spec.json' with { type: 'json' }
import normalizationContract from './upstream-render-normalization.contract.json' with { type: 'json' }
import { adaptImg2ThreeJsGroupToCompiledScene } from './img2threejs-compiled-scene-adapter.ts'
import { captureKnifeAovs, sha256Hex, type KnifeBrowserCaptureResult, type KnifeCaptureAovId } from './weaponry-source/knife-browser-capture.ts'
import { compileKnifeScene, type CompiledKnifeScene } from './weaponry-source/knife-scene-compiler.ts'
import {
  importImg2ThreeJsKnifeSpec,
  type Img2ThreeJsComponentMapping,
  type Img2ThreeJsImportReceipt,
  type Img2ThreeJsMaterialMapping,
} from './weaponry-source/img2threejs-object-sculpt-adapter.ts'
import { createKnifeViewRig, type KnifeViewRig } from './weaponry-source/knife-view-evaluation.ts'
import { createDragonfangLikeBaselineModel } from './generated/DragonfangLikeBaseline.ts'

/**
 * Temporary same-input browser bridge.  The generated upstream module and the
 * Weaponry compatibility path are both fed the exact copied JSON fixture.
 * This module only emits capture evidence; it does not persist image bytes or
 * make a visual-quality decision.
 */

export const SAME_INPUT_CAPTURE_SCHEMA = 'WeaponryThreeJsSameInputCapture@1' as const
export const SAME_INPUT_FACTORY_SHA256 = '__FACTORY_SHA256__'
export const SAME_INPUT_SPEC_SHA256 = '__SPEC_SHA256__'
export const SAME_INPUT_CONTRACT_SHA256 = '__CONTRACT_SHA256__'

const REQUIRED_AOV_IDS = Object.freeze([...normalizationContract.aov_contract.required]) as readonly KnifeCaptureAovId[]
const VIEW_IDS = Object.freeze([...normalizationContract.fixed_view_rig.views.map((view) => view.view_id)]) as readonly string[]
const FRAME_WIDTH = normalizationContract.fixed_view_rig.frame_width
const FRAME_HEIGHT = normalizationContract.fixed_view_rig.frame_height
const MARGIN = normalizationContract.fixed_view_rig.margin
const TARGET_MAX_EXTENT = normalizationContract.scene_normalization.target_max_extent
const EPSILON = normalizationContract.scene_normalization.epsilon
const PINNED_UPSTREAM_REVISION = '9fbd0ca5bbcc3b13bebe712745d6784d33db0b85'
const PINNED_UPSTREAM_TREE = '0ee3c2a6d781407808df98b33174539842f85fcc'

interface NormalizedScene {
  readonly root: THREE.Group
  readonly source_bounds: BoundsSummary
  readonly normalized_bounds: BoundsSummary
  readonly source_center: readonly [number, number, number]
  readonly source_size: readonly [number, number, number]
  readonly uniform_scale: number
}

interface BoundsSummary {
  readonly min: readonly [number, number, number]
  readonly max: readonly [number, number, number]
  readonly center: readonly [number, number, number]
  readonly size: readonly [number, number, number]
  readonly max_extent: number
}

interface CaptureBytes {
  readonly view_id: string
  readonly aov_id: KnifeCaptureAovId
  readonly png_sha256: string
  readonly png_size_bytes: number
  readonly png_bytes: Uint8Array
}

interface CaptureSummary {
  readonly capture_name: string
  readonly manifest: KnifeBrowserCaptureResult['manifest']
  readonly receipt: KnifeBrowserCaptureResult['receipt']
  readonly sink_records: readonly CaptureBytes[]
  readonly input_fingerprint: string
  readonly source_mesh_count: number
  readonly source_triangle_count: number
  readonly renderable_part_ids: readonly string[]
  readonly renderable_material_zone_ids: readonly string[]
  readonly normalized_scene: NormalizedScene
}

interface PixelAggregate {
  readonly pair_count: number
  readonly mean_absolute_rgba_error: number
  readonly mean_exact_rgba_fraction: number
  readonly mean_silhouette_iou: number
  readonly mean_part_id_exact_fraction: number
  readonly mean_material_id_exact_fraction: number
}

export interface SameInputCaptureResult {
  readonly schema_version: typeof SAME_INPUT_CAPTURE_SCHEMA
  readonly status: 'PASS_SAME_INPUT_BROWSER_CAPTURE'
  readonly quality_status: 'NOT_RUN'
  readonly visual_superiority: 'NOT_PROVEN'
  readonly network_used: false
  readonly input: {
    readonly schema_version: string
    readonly target_name: string
    readonly object_sculpt_spec: Record<string, unknown>
    readonly component_ids: readonly string[]
    readonly material_ids: readonly string[]
    readonly component_count: number
    readonly material_count: number
    readonly source_spec_sha256: string
    readonly compatibility_import_receipt: ReturnType<typeof importImg2ThreeJsKnifeSpec>['receipt']
  }
  readonly normalization: {
    readonly contract_id: string
    readonly contract_schema_version: string
    readonly contract_sha256: string
    readonly target_max_extent: number
    readonly formula: string
    readonly same_contract: true
    readonly baseline: NormalizationSummary
    readonly compatibility_import: NormalizationSummary
  }
  readonly rig: {
    readonly rig_id: string
    readonly rig_fingerprint: string
    readonly frame_width: number
    readonly frame_height: number
    readonly margin: number
    readonly view_ids: readonly string[]
    readonly same_rig: true
    readonly camera_bindings_equal: true
    readonly camera_bindings: readonly CameraBindingSummary[]
  }
  readonly renderer_cohort: {
    readonly renderer: 'browser-webgl@1'
    readonly capture_mode: 'browser-canvas-to-png@1'
    readonly same_renderer_instance: true
    readonly aov_ids: readonly KnifeCaptureAovId[]
    readonly capture_count: 2
    readonly external_network_used: false
  }
  readonly captures: readonly CaptureEvidence[]
  readonly structure: StructureComparison
  readonly pixel_metrics: PixelComparison
}

interface NormalizationSummary {
  readonly source_bounds: BoundsSummary
  readonly normalized_bounds: BoundsSummary
  readonly source_center: readonly [number, number, number]
  readonly source_size: readonly [number, number, number]
  readonly uniform_scale: number
  readonly max_extent_error: number
  readonly center_error: number
}

interface CameraBindingSummary {
  readonly view_id: string
  readonly projection: string
  readonly camera_fingerprint: string
  readonly matrix_world: readonly number[]
  readonly matrix_world_inverse: readonly number[]
  readonly projection_matrix: readonly number[]
}

interface CaptureEvidence {
  readonly capture_name: string
  readonly manifest_id: string
  readonly manifest_sha256: string
  readonly receipt_sha256: string
  readonly program_fingerprint: string
  readonly scene_fingerprint: string
  readonly render_status: string
  readonly quality_status: string
  readonly renderer_invoked: boolean
  readonly view_count: number
  readonly aov_count: number
  readonly png_count: number
  readonly png_total_bytes: number
  readonly normalized_scene: NormalizationSummary
  readonly part_ids: readonly string[]
  readonly material_zone_ids: readonly string[]
}

interface StructureComparison {
  readonly same_input_spec: true
  readonly baseline_component_count: number
  readonly compatibility_imported_component_count: number
  readonly baseline_renderable_part_count: number
  readonly compatibility_renderable_part_count: number
  readonly baseline_triangle_count: number
  readonly compatibility_triangle_count: number
  readonly baseline_part_ids: readonly string[]
  readonly compatibility_part_ids: readonly string[]
  readonly common_part_ids: readonly string[]
  readonly missing_from_compatibility_import: readonly string[]
  readonly source_component_ids: readonly string[]
  readonly imported_component_ids: readonly string[]
  readonly mapped_component_ids: readonly string[]
  readonly preserved_component_ids: readonly string[]
  readonly unsupported_component_ids: readonly string[]
  readonly ignored_component_ids: readonly string[]
  readonly component_mappings: readonly Img2ThreeJsComponentMapping[]
  readonly source_material_ids: readonly string[]
  readonly imported_material_ids: readonly string[]
  readonly mapped_material_ids: readonly string[]
  readonly preserved_material_ids: readonly string[]
  readonly unsupported_material_ids: readonly string[]
  readonly material_mappings: readonly Img2ThreeJsMaterialMapping[]
  readonly full_assembly_status: Img2ThreeJsImportReceipt['full_assembly_status']
  readonly full_assembly_blocked_by: readonly string[]
  readonly component_id_parity: boolean
  readonly material_id_parity: boolean
  readonly stable_part_ids: true
  readonly stable_material_ids: true
  readonly all_input_components_preserved_by_compatibility_import: boolean
  readonly comparable_capture_cohort: true
  readonly classification: 'STRUCTURAL_PARITY' | 'NOT_PROVEN'
  readonly reason: string
}

interface ImportIdentityEvidence {
  readonly imported_component_ids: readonly string[]
  readonly mapped_component_ids: readonly string[]
  readonly preserved_component_ids: readonly string[]
  readonly unsupported_component_ids: readonly string[]
  readonly ignored_component_ids: readonly string[]
  readonly component_mappings: readonly Img2ThreeJsComponentMapping[]
  readonly imported_material_ids: readonly string[]
  readonly mapped_material_ids: readonly string[]
  readonly preserved_material_ids: readonly string[]
  readonly unsupported_material_ids: readonly string[]
  readonly material_mappings: readonly Img2ThreeJsMaterialMapping[]
  readonly full_assembly_status: Img2ThreeJsImportReceipt['full_assembly_status']
  readonly full_assembly_blocked_by: readonly string[]
  readonly component_id_parity: boolean
  readonly material_id_parity: boolean
  readonly full_parity: boolean
}

interface PixelComparison {
  readonly reference_available: false
  readonly compared_outputs: readonly [string, string]
  readonly pairwise: Readonly<Record<string, PixelAggregate>>
  readonly classification: 'METRICALLY_SUPERIOR' | 'NOT_PROVEN'
  readonly reason: string
}

export async function runSameInputCapture(renderer: THREE.WebGLRenderer): Promise<SameInputCaptureResult> {
  validateClosedContract()
  if (!renderer || renderer.domElement.width !== FRAME_WIDTH || renderer.domElement.height !== FRAME_HEIGHT) {
    throw new Error('same-input capture requires the fixed contract canvas dimensions')
  }

  const imported = importImg2ThreeJsKnifeSpec(objectSculptSpec)
  const componentIds = componentIdsFromSpec(objectSculptSpec)
  const materialIds = materialIdsFromSpec(objectSculptSpec)
  const importEvidence = validateImportReceipt(imported.receipt, componentIds, materialIds)
  const compatibilityCompiled = compileKnifeScene(imported.program)
  const upstreamScene = makeNormalizedScene(
    createDragonfangLikeBaselineModel({ castShadow: false, receiveShadow: false }),
    'img2threejs-baseline',
  )
  const compatibilityScene = makeNormalizedScene(compatibilityCompiled.group, 'weaponry-compatibility-import')
  const upstreamAdapted = adaptImg2ThreeJsGroupToCompiledScene(upstreamScene.root, {
    source_fingerprint: SAME_INPUT_FACTORY_SHA256,
    group_name: 'img2threejs-same-input-group',
  })
  const rig = createKnifeViewRig({ frame_width: FRAME_WIDTH, frame_height: FRAME_HEIGHT, margin: MARGIN })
  if (rig.deterministic_fingerprint !== '3fa0202473e3352b') throw new Error('fixed rig fingerprint drifted')

  const upstreamRenderScene = makeRenderScene(upstreamAdapted.group, 'img2threejs-same-input-scene')
  const compatibilityRenderScene = makeRenderScene(compatibilityScene.root, 'weaponry-same-input-scene')
  const upstreamCapture = captureOne(
    renderer,
    upstreamRenderScene,
    upstreamAdapted as unknown as CompiledKnifeScene,
    rig,
    'pinned-img2threejs-baseline',
    upstreamScene,
  )
  const compatibilityCapture = captureOne(
    renderer,
    compatibilityRenderScene,
    compatibilityCompiled,
    rig,
    'weaponry-compatibility-import',
    compatibilityScene,
  )

  const cameraBindings = summarizeCameraBindings(upstreamCapture.manifest)
  const compatibilityCameraBindings = summarizeCameraBindings(compatibilityCapture.manifest)
  if (JSON.stringify(cameraBindings) !== JSON.stringify(compatibilityCameraBindings)) {
    throw new Error('same-input captures did not use identical fixed camera bindings')
  }

  const pixelMetrics = await compareCapturePixels(upstreamCapture.sink_records, compatibilityCapture.sink_records)
  const baselinePartIds = upstreamAdapted.parts.map((part) => part.part_id)
  const compatibilityPartIds = compatibilityCompiled.parts.map((part) => part.part_id)
  const missingFromCompatibility = componentIds.filter((componentId) => !importEvidence.preserved_component_ids.includes(componentId))
  const allInputComponentsPreserved = importEvidence.full_parity

  return {
    schema_version: SAME_INPUT_CAPTURE_SCHEMA,
    status: 'PASS_SAME_INPUT_BROWSER_CAPTURE',
    quality_status: 'NOT_RUN',
    visual_superiority: 'NOT_PROVEN',
    network_used: false,
    input: {
      schema_version: text(objectSculptSpec.schemaVersion),
      target_name: text(objectSculptSpec.targetName),
      object_sculpt_spec: objectSculptSpec as unknown as Record<string, unknown>,
      component_ids: componentIds,
      material_ids: materialIds,
      component_count: componentIds.length,
      material_count: materialIds.length,
      source_spec_sha256: SAME_INPUT_SPEC_SHA256,
      compatibility_import_receipt: imported.receipt,
    },
    normalization: {
      contract_id: normalizationContract.contract_id,
      contract_schema_version: normalizationContract.schema_version,
      contract_sha256: SAME_INPUT_CONTRACT_SHA256,
      target_max_extent: TARGET_MAX_EXTENT,
      formula: normalizationContract.scene_normalization.scale_formula,
      same_contract: true,
      baseline: normalizationSummary(upstreamScene),
      compatibility_import: normalizationSummary(compatibilityScene),
    },
    rig: {
      rig_id: rig.rig_id,
      rig_fingerprint: rig.deterministic_fingerprint,
      frame_width: rig.frame_width,
      frame_height: rig.frame_height,
      margin: rig.margin,
      view_ids: [...VIEW_IDS],
      same_rig: true,
      camera_bindings_equal: true,
      camera_bindings: cameraBindings,
    },
    renderer_cohort: {
      renderer: 'browser-webgl@1',
      capture_mode: 'browser-canvas-to-png@1',
      same_renderer_instance: true,
      aov_ids: [...REQUIRED_AOV_IDS],
      capture_count: 2,
      external_network_used: false,
    },
    captures: [captureEvidence(upstreamCapture), captureEvidence(compatibilityCapture)],
    structure: {
      same_input_spec: true,
      baseline_component_count: componentIds.length,
      compatibility_imported_component_count: importEvidence.imported_component_ids.length,
      baseline_renderable_part_count: upstreamAdapted.parts.length,
      compatibility_renderable_part_count: compatibilityCompiled.parts.length,
      baseline_triangle_count: upstreamAdapted.triangle_count,
      compatibility_triangle_count: compatibilityCompiled.triangle_count,
      baseline_part_ids: baselinePartIds,
      compatibility_part_ids: compatibilityPartIds,
      common_part_ids: baselinePartIds.filter((partId) => compatibilityPartIds.includes(partId)).sort(),
      missing_from_compatibility_import: missingFromCompatibility,
      source_component_ids: componentIds,
      imported_component_ids: importEvidence.imported_component_ids,
      mapped_component_ids: importEvidence.mapped_component_ids,
      preserved_component_ids: importEvidence.preserved_component_ids,
      unsupported_component_ids: importEvidence.unsupported_component_ids,
      ignored_component_ids: importEvidence.ignored_component_ids,
      component_mappings: importEvidence.component_mappings,
      source_material_ids: materialIds,
      imported_material_ids: importEvidence.imported_material_ids,
      mapped_material_ids: importEvidence.mapped_material_ids,
      preserved_material_ids: importEvidence.preserved_material_ids,
      unsupported_material_ids: importEvidence.unsupported_material_ids,
      material_mappings: importEvidence.material_mappings,
      full_assembly_status: importEvidence.full_assembly_status,
      full_assembly_blocked_by: importEvidence.full_assembly_blocked_by,
      component_id_parity: importEvidence.component_id_parity,
      material_id_parity: importEvidence.material_id_parity,
      stable_part_ids: true,
      stable_material_ids: true,
      all_input_components_preserved_by_compatibility_import: allInputComponentsPreserved,
      comparable_capture_cohort: true,
      classification: allInputComponentsPreserved ? 'STRUCTURAL_PARITY' : 'NOT_PROVEN',
      reason: allInputComponentsPreserved
        ? 'The compatibility import retained and mapped all source components/materials under the same bounded ID/capture cohort; this is structural parity only, not superiority.'
        : 'One or more source component/material IDs are not fully retained or mapped; structural parity remains NOT_PROVEN.',
    },
    pixel_metrics: {
      reference_available: false,
      compared_outputs: ['pinned-img2threejs-baseline', 'weaponry-compatibility-import'],
      pairwise: pixelMetrics,
      classification: 'NOT_PROVEN',
      reason: 'The fixture contains no authorized reference pixel target or threshold; pairwise differences are measurements, not a quality ranking.',
    },
  }
}

function validateImportReceipt(
  receipt: Img2ThreeJsImportReceipt,
  expectedComponentIds: readonly string[],
  expectedMaterialIds: readonly string[],
): ImportIdentityEvidence {
  if (receipt.schema_version !== 'Img2ThreeJsKnifeImportReceipt@1') throw new Error('compatibility import receipt schema drifted')
  if (receipt.upstream_revision !== PINNED_UPSTREAM_REVISION) throw new Error('compatibility import receipt is not bound to the pinned upstream revision')
  if (receipt.source_schema_version !== text(objectSculptSpec.schemaVersion) || receipt.source_target_name !== text(objectSculptSpec.targetName)) {
    throw new Error('compatibility import receipt target/schema binding drifted')
  }
  const sourceIdentity = receipt.source_identity
  if (
    !sourceIdentity
    || sourceIdentity.revision !== PINNED_UPSTREAM_REVISION
    || sourceIdentity.tree !== PINNED_UPSTREAM_TREE
    || !/^[a-f0-9]{64}$/.test(sourceIdentity.generator_sha256)
    || !/^[a-f0-9]{64}$/.test(sourceIdentity.validator_sha256)
  ) throw new Error('compatibility import source identity is not pinned')
  if (receipt.execution_performed !== false || receipt.network_used !== false || receipt.quality_status !== 'NOT_RUN') {
    throw new Error('compatibility import receipt crossed its closed boundary')
  }
  if (!Number.isInteger(receipt.imported_station_count) || receipt.imported_station_count < 2) {
    throw new Error('compatibility import station count is not bounded')
  }

  const componentSet = new Set(expectedComponentIds)
  const materialSet = new Set(expectedMaterialIds)
  const sourceComponents = objectSculptSpec.componentTree.map((component) => ({
    id: text(component.id),
    role: text(component.role),
    primitive: text(component.primitive),
    material: typeof component.material === 'string' ? component.material : null,
  }))
  const sourceComponentById = new Map(sourceComponents.map((component) => [component.id, component]))
  if (sourceComponentById.size !== expectedComponentIds.length || sourceComponents.some((component, index) => component.id !== expectedComponentIds[index])) {
    throw new Error('ObjectSculptSpec component order drifted before importer validation')
  }
  const sourceMaterialIds = objectSculptSpec.materials.map((material) => text(material.id))
  if (sourceMaterialIds.length !== expectedMaterialIds.length || sourceMaterialIds.some((id, index) => id !== expectedMaterialIds[index])) {
    throw new Error('ObjectSculptSpec material order drifted before importer validation')
  }
  const bladeIds = sourceComponents.filter((component) => component.primitive === 'ground-blade' && (component.role === 'blade' || component.id === 'blade')).map((component) => component.id)
  if (bladeIds.length !== 1 || receipt.source_blade_component_id !== bladeIds[0]) throw new Error('compatibility import blade binding is not closed')

  const importedComponents = stableIdList(receipt.imported_component_ids, 'imported_component_ids', componentSet)
  const mappedComponents = stableIdList(receipt.mapped_component_ids, 'mapped_component_ids', componentSet)
  const preservedComponents = stableIdList(receipt.preserved_component_ids, 'preserved_component_ids', componentSet)
  const unsupportedComponents = stableIdList(receipt.unsupported_component_ids, 'unsupported_component_ids', componentSet)
  const ignoredComponents = stableIdList(receipt.ignored_component_ids, 'ignored_component_ids', componentSet)
  const importedMaterials = stableIdList(receipt.imported_material_ids, 'imported_material_ids', materialSet)
  const mappedMaterials = stableIdList(receipt.mapped_material_ids, 'mapped_material_ids', materialSet)
  const preservedMaterials = stableIdList(receipt.preserved_material_ids, 'preserved_material_ids', materialSet)
  const unsupportedMaterials = stableIdList(receipt.unsupported_material_ids, 'unsupported_material_ids', materialSet)
  if (!isSubset(mappedComponents, importedComponents) || !isSubset(preservedComponents, importedComponents) || !isSubset(unsupportedComponents, importedComponents) || !isSubset(ignoredComponents, importedComponents)) {
    throw new Error('component importer receipt contains IDs outside imported_component_ids')
  }
  if (!isSubset(mappedMaterials, importedMaterials) || !isSubset(preservedMaterials, importedMaterials) || !isSubset(unsupportedMaterials, importedMaterials)) {
    throw new Error('material importer receipt contains IDs outside imported_material_ids')
  }
  if (!sameIdSet(ignoredComponents, unsupportedComponents)) throw new Error('ignored_component_ids must remain the unsupported-component alias')

  const componentMappings = receipt.component_mappings
  if (!Array.isArray(componentMappings) || componentMappings.length !== importedComponents.length) {
    throw new Error('component_mappings is not one-to-one with imported_component_ids')
  }
  const componentMappingIds = new Set<string>()
  let componentMappingExact = true
  for (const [index, rawMapping] of componentMappings.entries()) {
    const mapping = rawMapping as Partial<Img2ThreeJsComponentMapping>
    const componentId = text(mapping.source_component_id)
    const sourceComponent = sourceComponentById.get(componentId)
    if (!sourceComponent || componentMappingIds.has(componentId)) throw new Error(`component_mappings[${index}] has an unknown or duplicate source component ID`)
    componentMappingIds.add(componentId)
    const sourceOrder = mapping.source_order
    if (!Number.isInteger(sourceOrder) || sourceOrder === undefined || sourceOrder < 0 || sourceOrder >= expectedComponentIds.length || expectedComponentIds[sourceOrder] !== componentId) {
      throw new Error(`component_mappings[${index}] source_order is not the closed source order`)
    }
    if (mapping.source_role !== sourceComponent.role || mapping.source_primitive !== sourceComponent.primitive || mapping.source_material_id !== sourceComponent.material) {
      throw new Error(`component_mappings[${index}] source semantics drifted from ObjectSculptSpec`)
    }
    const targetPartIds = stableIdList(mapping.target_part_ids, `component_mappings[${index}].target_part_ids`)
    const targetMaterial = mapping.target_material_zone_id
    if (targetMaterial !== null && (typeof targetMaterial !== 'string' || !/^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/.test(targetMaterial))) {
      throw new Error(`component_mappings[${index}].target_material_zone_id is invalid`)
    }
    if (mapping.status !== 'MAPPED' && mapping.status !== 'UNSUPPORTED') throw new Error(`component_mappings[${index}] status is outside the closed vocabulary`)
    if (mapping.projection !== 'exact' && mapping.projection !== 'lossy' && mapping.projection !== 'unsupported') throw new Error(`component_mappings[${index}] projection is outside the closed vocabulary`)
    if (mapping.status === 'MAPPED') {
      if (targetPartIds.length === 0 || targetMaterial !== sourceComponent.material || mapping.projection === 'unsupported') throw new Error(`component_mappings[${index}] is marked MAPPED without an exact target`)
      if (mapping.projection !== 'exact') componentMappingExact = false
    } else {
      if (targetPartIds.length !== 0 || targetMaterial !== null || mapping.projection !== 'unsupported') throw new Error(`component_mappings[${index}] is marked UNSUPPORTED with a target`)
      componentMappingExact = false
    }
  }
  if (!sameIdSet([...componentMappingIds], importedComponents)) throw new Error('component_mappings do not cover exactly imported_component_ids')

  const materialMappings = receipt.material_mappings
  if (!Array.isArray(materialMappings) || materialMappings.length !== importedMaterials.length) {
    throw new Error('material_mappings is not one-to-one with imported_material_ids')
  }
  const materialMappingIds = new Set<string>()
  let materialMappingExact = true
  for (const [index, rawMapping] of materialMappings.entries()) {
    const mapping = rawMapping as Partial<Img2ThreeJsMaterialMapping>
    const materialId = text(mapping.source_material_id)
    if (!materialSet.has(materialId) || materialMappingIds.has(materialId)) throw new Error(`material_mappings[${index}] has an unknown or duplicate source material ID`)
    materialMappingIds.add(materialId)
    const sourceOrder = mapping.source_order
    if (!Number.isInteger(sourceOrder) || sourceOrder === undefined || sourceOrder < 0 || sourceOrder >= expectedMaterialIds.length || expectedMaterialIds[sourceOrder] !== materialId) {
      throw new Error(`material_mappings[${index}] source_order is not the closed source order`)
    }
    const targetMaterial = mapping.target_material_zone_id
    if (targetMaterial !== null && (typeof targetMaterial !== 'string' || !/^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/.test(targetMaterial))) throw new Error(`material_mappings[${index}] target zone is invalid`)
    if (mapping.status !== 'MAPPED' && mapping.status !== 'UNSUPPORTED') throw new Error(`material_mappings[${index}] status is outside the closed vocabulary`)
    if (mapping.projection !== 'exact' && mapping.projection !== 'lossy' && mapping.projection !== 'unsupported') throw new Error(`material_mappings[${index}] projection is outside the closed vocabulary`)
    if (mapping.status === 'MAPPED') {
      if (targetMaterial !== materialId || mapping.projection === 'unsupported') throw new Error(`material_mappings[${index}] is marked MAPPED without an exact target zone`)
      if (mapping.projection !== 'exact') materialMappingExact = false
    } else {
      if (targetMaterial !== null || mapping.projection !== 'unsupported') throw new Error(`material_mappings[${index}] is marked UNSUPPORTED with a target`)
      materialMappingExact = false
    }
  }
  if (!sameIdSet([...materialMappingIds], importedMaterials)) throw new Error('material_mappings do not cover exactly imported_material_ids')

  if (receipt.full_assembly_status !== 'COMPILED' && receipt.full_assembly_status !== 'BLOCKED_UNSUPPORTED_COMPONENTS') throw new Error('full_assembly_status is outside the closed vocabulary')
  if (!Array.isArray(receipt.full_assembly_blocked_by) || new Set(receipt.full_assembly_blocked_by).size !== receipt.full_assembly_blocked_by.length || receipt.full_assembly_blocked_by.some((entry) => typeof entry !== 'string' || !/^(component|material):[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/.test(entry))) {
    throw new Error('full_assembly_blocked_by is not a stable bounded list')
  }
  if (receipt.full_assembly_status === 'COMPILED' && receipt.full_assembly_blocked_by.length > 0) throw new Error('COMPILED import contains full_assembly_blocked_by entries')

  const componentIdParity = sameIdSet(importedComponents, expectedComponentIds)
    && sameIdSet(mappedComponents, expectedComponentIds)
    && sameIdSet(preservedComponents, expectedComponentIds)
    && unsupportedComponents.length === 0
    && ignoredComponents.length === 0
    && componentMappingExact
  const materialIdParity = sameIdSet(importedMaterials, expectedMaterialIds)
    && sameIdSet(mappedMaterials, expectedMaterialIds)
    && sameIdSet(preservedMaterials, expectedMaterialIds)
    && unsupportedMaterials.length === 0
    && materialMappingExact
  return {
    imported_component_ids: importedComponents,
    mapped_component_ids: mappedComponents,
    preserved_component_ids: preservedComponents,
    unsupported_component_ids: unsupportedComponents,
    ignored_component_ids: ignoredComponents,
    component_mappings: Object.freeze([...componentMappings]),
    imported_material_ids: importedMaterials,
    mapped_material_ids: mappedMaterials,
    preserved_material_ids: preservedMaterials,
    unsupported_material_ids: unsupportedMaterials,
    material_mappings: Object.freeze([...materialMappings]),
    full_assembly_status: receipt.full_assembly_status,
    full_assembly_blocked_by: Object.freeze([...receipt.full_assembly_blocked_by]),
    component_id_parity: componentIdParity,
    material_id_parity: materialIdParity,
    full_parity: componentIdParity && materialIdParity && receipt.full_assembly_status === 'COMPILED' && receipt.full_assembly_blocked_by.length === 0,
  }
}

function stableIdList(value: unknown, label: string, allowed?: ReadonlySet<string>): readonly string[] {
  if (!Array.isArray(value)) throw new Error(`${label} is not a stable ID list`)
  const ids = value.map((candidate, index) => {
    if (typeof candidate !== 'string' || !/^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/.test(candidate)) throw new Error(`${label}[${index}] is not a bounded stable ID`)
    if (allowed && !allowed.has(candidate)) throw new Error(`${label}[${index}] is outside the closed source ID set`)
    return candidate
  })
  if (new Set(ids).size !== ids.length) throw new Error(`${label} contains duplicate IDs`)
  return Object.freeze(ids)
}

function isSubset(subset: readonly string[], superset: readonly string[]): boolean {
  const supersetIds = new Set(superset)
  return subset.every((id) => supersetIds.has(id))
}

function sameIdSet(left: readonly string[], right: readonly string[]): boolean {
  return left.length === right.length && isSubset(left, right) && isSubset(right, left)
}

function validateClosedContract(): void {
  if (SAME_INPUT_FACTORY_SHA256.length !== 64 || !/^[a-f0-9]{64}$/.test(SAME_INPUT_FACTORY_SHA256)) {
    throw new Error('generated factory hash marker was not bound by the benchmark runner')
  }
  if (SAME_INPUT_SPEC_SHA256.length !== 64 || !/^[a-f0-9]{64}$/.test(SAME_INPUT_SPEC_SHA256)) {
    throw new Error('ObjectSculptSpec hash marker was not bound by the benchmark runner')
  }
  if (SAME_INPUT_CONTRACT_SHA256.length !== 64 || !/^[a-f0-9]{64}$/.test(SAME_INPUT_CONTRACT_SHA256)) {
    throw new Error('normalization contract hash marker was not bound by the benchmark runner')
  }
  if (SAME_INPUT_FACTORY_SHA256 !== normalizationContract.source.factory_sha256) {
    throw new Error('generated factory hash does not match the normalization contract')
  }
  if (normalizationContract.scope.network_allowed !== false || normalizationContract.scope.quality_claim !== 'NOT_COMPUTED') {
    throw new Error('normalization contract crosses the network or quality boundary')
  }
  if (REQUIRED_AOV_IDS.join('|') !== 'beauty|silhouette|depth|normal|part-id|material-id|wireframe') {
    throw new Error('normalization contract AOV order is not closed')
  }
  if (VIEW_IDS.join('|') !== 'FRONT|BACK|TOP|BOTTOM|LEFT|RIGHT|REAR_THREE_QUARTER|FPS_HOLD') {
    throw new Error('normalization contract view order is not closed')
  }
  if (FRAME_WIDTH !== 256 || FRAME_HEIGHT !== 256 || MARGIN !== 0.08 || TARGET_MAX_EXTENT <= EPSILON) {
    throw new Error('normalization contract fixed capture dimensions drifted')
  }
}

function makeNormalizedScene(sourceRoot: THREE.Group, namespace: string): NormalizedScene {
  sourceRoot.updateMatrixWorld(true)
  const boundsBefore = new THREE.Box3().setFromObject(sourceRoot)
  const sourceCenter = boundsBefore.getCenter(new THREE.Vector3())
  const sourceSize = boundsBefore.getSize(new THREE.Vector3())
  const sourceExtent = Math.max(sourceSize.x, sourceSize.y, sourceSize.z)
  if (![sourceExtent, sourceCenter.x, sourceCenter.y, sourceCenter.z].every(Number.isFinite) || sourceExtent <= EPSILON) {
    throw new Error(`${namespace} source bounds are empty or non-finite`)
  }
  const uniformScale = TARGET_MAX_EXTENT / sourceExtent
  const root = new THREE.Group()
  root.name = `${namespace}-normalized`
  root.scale.setScalar(uniformScale)
  root.position.copy(sourceCenter).multiplyScalar(-uniformScale)
  root.add(sourceRoot)
  assignStableObjectIds(root, namespace)
  root.updateMatrixWorld(true)
  const boundsAfter = new THREE.Box3().setFromObject(root)
  return {
    root,
    source_bounds: boundsSummary(boundsBefore),
    normalized_bounds: boundsSummary(boundsAfter),
    source_center: [sourceCenter.x, sourceCenter.y, sourceCenter.z],
    source_size: [sourceSize.x, sourceSize.y, sourceSize.z],
    uniform_scale: uniformScale,
  }
}

function makeRenderScene(root: THREE.Group, name: string): THREE.Scene {
  const scene = new THREE.Scene()
  scene.name = name
  scene.background = new THREE.Color(0x080a0d)
  scene.add(new THREE.HemisphereLight(0xffffff, 0x20242b, 1.4))
  const key = new THREE.DirectionalLight(0xffffff, 2.2)
  key.position.set(2.5, 3.5, 4.5)
  scene.add(key)
  scene.add(root)
  assignStableObjectIds(scene, `${name}-scene`)
  scene.updateMatrixWorld(true)
  return scene
}

function captureOne(
  renderer: THREE.WebGLRenderer,
  scene: THREE.Scene,
  compiled: CompiledKnifeScene,
  rig: KnifeViewRig,
  captureName: string,
  normalizedScene: NormalizedScene,
): CaptureSummary {
  const sinkRecords: CaptureBytes[] = []
  const result = captureKnifeAovs({
    renderer,
    scene,
    compiled,
    rig,
    manifest_id: `same-input-${captureName}`,
    clear_color: 0x000000,
    capture_sink: (viewId, aovId, pngBytes) => sinkRecords.push({
      view_id: viewId,
      aov_id: aovId,
      png_sha256: sha256Hex(pngBytes),
      png_size_bytes: pngBytes.byteLength,
      png_bytes: pngBytes.slice(),
    }),
  })
  if (sinkRecords.length !== 56) throw new Error(`${captureName} did not produce 8x7 PNG byte streams`)
  return {
    capture_name: captureName,
    manifest: result.manifest,
    receipt: result.receipt,
    sink_records: Object.freeze(sinkRecords),
    input_fingerprint: compiled.deterministic_fingerprint,
    source_mesh_count: compiled.parts.length,
    source_triangle_count: compiled.triangle_count,
    renderable_part_ids: Object.freeze(compiled.parts.map((part) => part.part_id)),
    renderable_material_zone_ids: Object.freeze([...new Set(compiled.parts.map((part) => part.material_zone_id))].sort()),
    normalized_scene: normalizedScene,
  }
}

function summarizeCameraBindings(manifest: KnifeBrowserCaptureResult['manifest']): readonly CameraBindingSummary[] {
  return Object.freeze(manifest.views.map((view) => ({
    view_id: view.view_id,
    projection: view.camera.projection,
    camera_fingerprint: view.camera.camera_fingerprint,
    matrix_world: [...view.camera.matrix_world],
    matrix_world_inverse: [...view.camera.matrix_world_inverse],
    projection_matrix: [...view.camera.projection_matrix],
  })))
}

function captureEvidence(capture: CaptureSummary): CaptureEvidence {
  return {
    capture_name: capture.capture_name,
    manifest_id: capture.manifest.manifest_id,
    manifest_sha256: capture.manifest.canonical_sha256,
    receipt_sha256: capture.receipt.canonical_sha256,
    program_fingerprint: capture.manifest.program_fingerprint,
    scene_fingerprint: capture.manifest.scene_fingerprint,
    render_status: capture.manifest.render_status,
    quality_status: capture.manifest.quality_status,
    renderer_invoked: capture.manifest.renderer_invoked,
    view_count: capture.manifest.views.length,
    aov_count: capture.manifest.views.reduce((count, view) => count + view.aovs.length, 0),
    png_count: capture.sink_records.length,
    png_total_bytes: capture.sink_records.reduce((total, item) => total + item.png_size_bytes, 0),
    normalized_scene: normalizationSummary(capture.normalized_scene),
    part_ids: capture.renderable_part_ids,
    material_zone_ids: capture.renderable_material_zone_ids,
  }
}

function normalizationSummary(scene: NormalizedScene): NormalizationSummary {
  return {
    source_bounds: scene.source_bounds,
    normalized_bounds: scene.normalized_bounds,
    source_center: scene.source_center,
    source_size: scene.source_size,
    uniform_scale: scene.uniform_scale,
    max_extent_error: Math.abs(scene.normalized_bounds.max_extent - TARGET_MAX_EXTENT),
    center_error: Math.max(...scene.normalized_bounds.center.map((value) => Math.abs(value))),
  }
}

async function compareCapturePixels(
  left: readonly CaptureBytes[],
  right: readonly CaptureBytes[],
): Promise<Readonly<Record<string, PixelAggregate>>> {
  const rightByKey = new Map(right.map((record) => [`${record.view_id}/${record.aov_id}`, record]))
  const buckets = new Map<string, {
    pair_count: number
    absolute_error: number
    exact_fraction: number
    silhouette_iou: number
    part_id_exact: number
    material_id_exact: number
  }>()
  for (const leftRecord of left) {
    const rightRecord = rightByKey.get(`${leftRecord.view_id}/${leftRecord.aov_id}`)
    if (!rightRecord) throw new Error(`missing paired AOV ${leftRecord.view_id}/${leftRecord.aov_id}`)
    const leftPixels = await decodePngPixels(leftRecord.png_bytes)
    const rightPixels = await decodePngPixels(rightRecord.png_bytes)
    if (leftPixels.width !== rightPixels.width || leftPixels.height !== rightPixels.height || leftPixels.data.length !== rightPixels.data.length) {
      throw new Error(`paired AOV dimensions differ for ${leftRecord.view_id}/${leftRecord.aov_id}`)
    }
    const metric = pixelMetric(leftPixels.data, rightPixels.data, leftRecord.aov_id)
    const bucket = buckets.get(leftRecord.aov_id) ?? { pair_count: 0, absolute_error: 0, exact_fraction: 0, silhouette_iou: 0, part_id_exact: 0, material_id_exact: 0 }
    bucket.pair_count += 1
    bucket.absolute_error += metric.absolute_error
    bucket.exact_fraction += metric.exact_fraction
    bucket.silhouette_iou += metric.silhouette_iou
    bucket.part_id_exact += metric.part_id_exact
    bucket.material_id_exact += metric.material_id_exact
    buckets.set(leftRecord.aov_id, bucket)
  }
  const result: Record<string, PixelAggregate> = {}
  for (const aovId of REQUIRED_AOV_IDS) {
    const bucket = buckets.get(aovId)
    if (!bucket || bucket.pair_count !== 8) throw new Error(`pixel metric bucket is incomplete for ${aovId}`)
    result[aovId] = {
      pair_count: bucket.pair_count,
      mean_absolute_rgba_error: finiteMetric(bucket.absolute_error / bucket.pair_count, `absolute error ${aovId}`),
      mean_exact_rgba_fraction: finiteMetric(bucket.exact_fraction / bucket.pair_count, `exact fraction ${aovId}`),
      mean_silhouette_iou: finiteMetric(bucket.silhouette_iou / bucket.pair_count, `silhouette IoU ${aovId}`),
      mean_part_id_exact_fraction: finiteMetric(bucket.part_id_exact / bucket.pair_count, `part ID exact ${aovId}`),
      mean_material_id_exact_fraction: finiteMetric(bucket.material_id_exact / bucket.pair_count, `material ID exact ${aovId}`),
    }
  }
  return Object.freeze(result)
}

interface PixelBuffer {
  readonly width: number
  readonly height: number
  readonly data: Uint8ClampedArray
}

async function decodePngPixels(bytes: Uint8Array): Promise<PixelBuffer> {
  const blob = new Blob([bytes], { type: 'image/png' })
  const bitmap = await createImageBitmap(blob)
  const canvas = document.createElement('canvas')
  canvas.width = bitmap.width
  canvas.height = bitmap.height
  const context = canvas.getContext('2d', { willReadFrequently: true })
  if (!context) throw new Error('2D canvas pixel decoder is unavailable')
  context.clearRect(0, 0, bitmap.width, bitmap.height)
  context.drawImage(bitmap, 0, 0)
  const image = context.getImageData(0, 0, bitmap.width, bitmap.height)
  bitmap.close()
  return { width: image.width, height: image.height, data: image.data }
}

function pixelMetric(left: Uint8ClampedArray, right: Uint8ClampedArray, aovId: KnifeCaptureAovId): {
  readonly absolute_error: number
  readonly exact_fraction: number
  readonly silhouette_iou: number
  readonly part_id_exact: number
  readonly material_id_exact: number
} {
  let absoluteError = 0
  let exactPixels = 0
  let leftMask = 0
  let rightMask = 0
  let intersection = 0
  let union = 0
  let partIdExact = 0
  let materialIdExact = 0
  const pixelCount = left.length / 4
  for (let offset = 0; offset < left.length; offset += 4) {
    let channelEqual = true
    for (let channel = 0; channel < 4; channel += 1) {
      const difference = Math.abs(left[offset + channel] - right[offset + channel])
      absoluteError += difference / 255
      if (difference !== 0) channelEqual = false
    }
    if (channelEqual) exactPixels += 1
    const leftCovered = left[offset] !== 0 || left[offset + 1] !== 0 || left[offset + 2] !== 0
    const rightCovered = right[offset] !== 0 || right[offset + 1] !== 0 || right[offset + 2] !== 0
    if (leftCovered) leftMask += 1
    if (rightCovered) rightMask += 1
    if (leftCovered && rightCovered) intersection += 1
    if (leftCovered || rightCovered) union += 1
    if (aovId === 'part-id' && channelEqual) partIdExact += 1
    if (aovId === 'material-id' && channelEqual) materialIdExact += 1
  }
  const silhouetteIou = union === 0 ? 1 : intersection / union
  return {
    absolute_error: absoluteError / (pixelCount * 4),
    exact_fraction: exactPixels / pixelCount,
    silhouette_iou: silhouetteIou,
    part_id_exact: aovId === 'part-id' ? partIdExact / pixelCount : 0,
    material_id_exact: aovId === 'material-id' ? materialIdExact / pixelCount : 0,
  }
}

function boundsSummary(bounds: THREE.Box3): BoundsSummary {
  const center = bounds.getCenter(new THREE.Vector3())
  const size = bounds.getSize(new THREE.Vector3())
  return {
    min: [center.x - size.x * 0.5, center.y - size.y * 0.5, center.z - size.z * 0.5],
    max: [center.x + size.x * 0.5, center.y + size.y * 0.5, center.z + size.z * 0.5],
    center: [center.x, center.y, center.z],
    size: [size.x, size.y, size.z],
    max_extent: Math.max(size.x, size.y, size.z),
  }
}

function componentIdsFromSpec(spec: typeof objectSculptSpec): readonly string[] {
  const ids = spec.componentTree.map((component) => text(component.id))
  if (new Set(ids).size !== ids.length) throw new Error('ObjectSculptSpec component IDs are not unique')
  return Object.freeze(ids)
}

function materialIdsFromSpec(spec: typeof objectSculptSpec): readonly string[] {
  const ids = spec.materials.map((material) => text(material.id))
  if (new Set(ids).size !== ids.length) throw new Error('ObjectSculptSpec material IDs are not unique')
  return Object.freeze(ids)
}

function text(value: unknown): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 160) throw new Error('closed input text is invalid')
  return value
}

function finiteMetric(value: number, label: string): number {
  if (!Number.isFinite(value) || value < 0 || value > 1) throw new Error(`${label} is outside [0,1]`)
  return value
}

function assignStableObjectIds(root: THREE.Object3D, namespace: string): void {
  let ordinal = 0
  root.traverse((object) => {
    const component = object.userData?.sculptComponent
    const partId = typeof component?.id === 'string' ? component.id : object.userData?.part_id
    const semantic = typeof partId === 'string' ? partId : object.name || object.type
    overrideUuid(object, stableUuid(`${namespace}:${semantic}:${ordinal}`))
    const mesh = object as THREE.Mesh
    if (mesh.isMesh && mesh.geometry instanceof THREE.BufferGeometry) {
      overrideUuid(mesh.geometry, stableUuid(`${namespace}:geometry:${semantic}:${ordinal}`))
      const materials = Array.isArray(mesh.material) ? mesh.material : [mesh.material]
      materials.forEach((material, materialIndex) => {
        overrideUuid(material, stableUuid(`${namespace}:material:${semantic}:${ordinal}:${materialIndex}`))
      })
    }
    ordinal += 1
  })
}

function overrideUuid(object: { readonly uuid: string }, uuid: string): void {
  Object.defineProperty(object, 'uuid', { configurable: true, enumerable: true, value: uuid, writable: true })
}

function stableUuid(value: string): string {
  const raw = `${fnv1a64(`${value}:0`)}${fnv1a64(`${value}:1`)}${fnv1a64(`${value}:2`)}${fnv1a64(`${value}:3`)}`
  return `${raw.slice(0, 8)}-${raw.slice(8, 12)}-${raw.slice(12, 16)}-${raw.slice(16, 20)}-${raw.slice(20, 32)}`
}

function fnv1a64(value: string): string {
  let hash = 0xcbf29ce484222325n
  const prime = 0x100000001b3n
  const mask = 0xffffffffffffffffn
  for (let index = 0; index < value.length; index += 1) {
    hash ^= BigInt(value.charCodeAt(index))
    hash = (hash * prime) & mask
  }
  return hash.toString(16).padStart(16, '0')
}
