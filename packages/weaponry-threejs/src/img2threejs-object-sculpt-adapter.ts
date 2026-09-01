import {
  IMG2THREEJS_SOURCE_IDENTITY,
  freezeImg2ThreeJsSourceEnvelope,
  type Img2ThreeJsTessellationTier,
  type Img2ThreeJsSourceComponent,
  type Img2ThreeJsSourceEnvelope,
  type Img2ThreeJsSourceGeometry,
  type Img2ThreeJsSourceMaterial,
  type Img2ThreeJsSourceIdentity,
  type Img2ThreeJsSourceTransform,
  type Img2ThreeJsSourceVec2,
  type Img2ThreeJsSourceVec3,
} from './img2threejs-source-envelope.ts'
import type {
  KnifeAssembly,
  KnifeAssemblyAxis,
  KnifeAssemblyPrimitiveSpec,
  KnifeMaterialZone,
  KnifeSceneProgram,
  KnifeVec3,
} from './knife-scene-program.ts'
import {
  KNIFE_IMPORT_TRANSFORM_POLICY,
  type KnifeImportTransformPolicy,
} from './knife-scene-program.ts'

const PINNED_UPSTREAM = '9fbd0ca5bbcc3b13bebe712745d6784d33db0b85'
const STABLE_ID_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/
const MAX_SOURCE_COMPONENTS = 64
const MAX_SOURCE_MATERIALS = 64
const MAX_SOURCE_COORDINATE = 4
const MAX_ROTATION = Math.PI * 2
const MIN_SCALE = 1e-4
const MAX_SCALE = 4
const MAX_BLADE_THICKNESS = 1
const MAX_ASSEMBLY_TRIANGLES = 45_000

export class Img2ThreeJsImportError extends Error {
  constructor(message: string) {
    super(`IMG2THREEJS_IMPORT_INVALID: ${message}`)
    this.name = 'Img2ThreeJsImportError'
  }
}

export type Img2ThreeJsImportProjection = 'exact' | 'lossy' | 'unsupported'
export type Img2ThreeJsImportMappingStatus = 'MAPPED' | 'UNSUPPORTED'
export type Img2ThreeJsFullAssemblyStatus = 'COMPILED' | 'BLOCKED_UNSUPPORTED_COMPONENTS'

export interface Img2ThreeJsComponentMapping {
  readonly source_component_id: string
  readonly source_order: number
  readonly source_role: string
  readonly source_primitive: string
  readonly source_material_id: string | null
  readonly target_part_ids: readonly string[]
  readonly target_material_zone_id: string | null
  readonly status: Img2ThreeJsImportMappingStatus
  readonly projection: Img2ThreeJsImportProjection
  readonly reason?: string
}

export interface Img2ThreeJsMaterialMapping {
  readonly source_material_id: string
  readonly source_order: number
  readonly target_material_zone_id: string | null
  readonly status: Img2ThreeJsImportMappingStatus
  readonly projection: Img2ThreeJsImportProjection
  readonly reason?: string
}

export interface Img2ThreeJsImportReceipt {
  readonly schema_version: 'Img2ThreeJsKnifeImportReceipt@1'
  readonly upstream_revision: typeof PINNED_UPSTREAM
  readonly source_schema_version: string
  readonly source_identity: Img2ThreeJsSourceIdentity
  readonly source_target_name: string
  readonly source_blade_component_id: string
  readonly imported_station_count: number
  /** Every source component ID observed before projection. */
  readonly imported_component_ids: readonly string[]
  /** Components whose closed primitive/transform/material projection succeeded. */
  readonly mapped_component_ids: readonly string[]
  /** Components represented by the returned KnifeSceneProgram. */
  readonly preserved_component_ids: readonly string[]
  /** Components that cannot be represented by the closed compatibility route. */
  readonly unsupported_component_ids: readonly string[]
  /** Historical alias; it now contains only explicitly unsupported IDs. */
  readonly ignored_component_ids: readonly string[]
  readonly imported_material_ids: readonly string[]
  readonly mapped_material_ids: readonly string[]
  readonly preserved_material_ids: readonly string[]
  readonly unsupported_material_ids: readonly string[]
  readonly component_mappings: readonly Img2ThreeJsComponentMapping[]
  readonly material_mappings: readonly Img2ThreeJsMaterialMapping[]
  readonly transform_policy: KnifeImportTransformPolicy
  readonly full_assembly_status: Img2ThreeJsFullAssemblyStatus
  readonly full_assembly_blocked_by: readonly string[]
  readonly deterministic_fingerprint: string
  readonly execution_performed: false
  readonly network_used: false
  readonly quality_status: 'NOT_RUN'
}

export interface ImportedKnifeSceneProgram {
  readonly program: KnifeSceneProgram
  readonly receipt: Img2ThreeJsImportReceipt
}

interface ParsedSourceComponent {
  readonly raw: Record<string, unknown>
  readonly component_id: string
  readonly source_order: number
  readonly role: string
  readonly primitive: string
  readonly material_id: string | null
  readonly material_error?: string
  readonly parent_id: string | null
  readonly parent_error?: string
  readonly transform: Img2ThreeJsSourceTransform
  readonly transform_error?: string
}

interface ParsedSourceMaterial {
  readonly raw: Record<string, unknown>
  readonly material_id: string
  readonly source_order: number
  readonly zone: KnifeMaterialZone | undefined
  readonly reason?: string
}

interface MappedComponent {
  readonly component: ParsedSourceComponent
  readonly mapping: Img2ThreeJsComponentMapping
  readonly source_envelope_component?: Img2ThreeJsSourceComponent
  readonly spec?: KnifeAssemblyPrimitiveSpec
}

interface BladeBuild {
  readonly source_component: Img2ThreeJsSourceComponent
  readonly material_id: string
  readonly edge_material_id: string
  readonly station_count: number
  readonly blade_surface: KnifeSceneProgram['blade_surface']
}

/**
 * Static compatibility import only. It reads a bounded ObjectSculptSpec-shaped
 * value and projects the closed primitive subset into a first-party
 * KnifeSceneProgram. No upstream code is executed.
 */
export function importImg2ThreeJsKnifeSpec(value: unknown): ImportedKnifeSceneProgram {
  const source = object(value, 'ObjectSculptSpec')
  const schemaVersion = text(source.schemaVersion, 'schemaVersion')
  const targetName = text(source.targetName, 'targetName')
  validateSourceCoordinateFrame(source.coordinateFrame)
  const components = parseComponents(source.componentTree)
  const sourceMaterials = parseMaterials(source.materials)
  const materialById = new Map(sourceMaterials.map((material) => [material.material_id, material]))
  const validMaterials = sourceMaterials.filter((material): material is ParsedSourceMaterial & { zone: KnifeMaterialZone } => material.zone !== undefined)
  const validMaterialById = new Map(validMaterials.map((material) => [material.material_id, material]))

  const bladeCandidates = components.filter((component) => component.primitive === 'ground-blade' && (component.role === 'blade' || component.component_id === 'blade'))
  if (bladeCandidates.length !== 1) throw new Img2ThreeJsImportError('exactly one ground-blade component with role/id blade is required')
  const bladeComponent = bladeCandidates[0]
  const bladeBuild = buildBlade(bladeComponent, validMaterialById, materialById)

  const mappedComponents = components.map((component) => (
    component === bladeComponent
      ? mapBladeComponent(component, bladeBuild)
      : mapAssemblyComponent(component, validMaterialById)
  ))
  const cardinalityAdjusted = enforceAssemblyCardinality(mappedComponents)
  const unsupportedComponents = cardinalityAdjusted.filter((result) => result.mapping.status === 'UNSUPPORTED')
  const unsupportedMaterials = sourceMaterials.filter((material) => material.zone === undefined)
  const blocked = unsupportedComponents.length > 0 || unsupportedMaterials.length > 0
  const fullAssemblyStatus: Img2ThreeJsFullAssemblyStatus = blocked ? 'BLOCKED_UNSUPPORTED_COMPONENTS' : 'COMPILED'

  const assemblyComponents = cardinalityAdjusted.filter((result) => result.spec !== undefined && result.mapping.status === 'MAPPED')
  const semanticAssembly = blocked ? undefined : buildAssembly(assemblyComponents)
  const sourceEnvelope = blocked
    ? undefined
    : buildSourceEnvelope(schemaVersion, targetName, cardinalityAdjusted, validMaterials, resolveSourceTessellation(source.performanceBudget))

  const materialZones = validMaterials
    .map((material) => material.zone!)
    .sort((left, right) => left.material_zone_id.localeCompare(right.material_zone_id))
  const assemblyParts = blocked ? [] : assemblyComponents
    .map((result) => result.component)
    .sort((left, right) => left.source_order - right.source_order || left.component_id.localeCompare(right.component_id))
    .map((component) => ({
      part_id: component.component_id,
      role: component.role as 'guard' | 'grip' | 'pommel' | 'fastener' | 'gem' | 'relief',
      source_class: 'inferred' as const,
      material_zone_id: component.material_id!,
      frozen: false,
    }))
  const parts: KnifeSceneProgram['parts'] = [
    { part_id: 'blade-body', role: 'blade-body', source_class: 'inferred', material_zone_id: bladeBuild.material_id, frozen: false },
    { part_id: 'cutting-edge', role: 'cutting-edge', source_class: 'inferred', material_zone_id: bladeBuild.edge_material_id, frozen: false },
    ...assemblyParts,
  ]
  const unknowns = sourceUnknowns(source, cardinalityAdjusted, unsupportedMaterials)
  const program: KnifeSceneProgram = {
    schema_version: 'KnifeSceneProgram@1',
    asset_id: safeId(targetName),
    family: 'original-knife',
    design_basis: 'img2threejs-compatible-import',
    coordinate_convention: 'weapon-front-z-up-right-handed@1',
    blade_surface: bladeBuild.blade_surface,
    ...(sourceEnvelope ? { source_envelope: sourceEnvelope } : {}),
    ...(semanticAssembly ? { assembly: semanticAssembly } : {}),
    parts,
    material_zones: materialZones,
    presentation: {
      camera_set: 'knife-fixed-eight-view@1',
      renderer: 'threejs-browser-authority@1',
      aovs: ['beauty', 'silhouette', 'depth', 'normal', 'part-id', 'material-id', 'wireframe', 'curvature'],
    },
    budgets: { max_triangles: MAX_ASSEMBLY_TRIANGLES, max_draw_calls: 16, max_texture_bytes: 67108864 },
    unknowns,
    canonical_sha256: '',
  }

  const importedComponentIds = sortedUnique(components.map((component) => component.component_id))
  const mappedComponentIds = sortedUnique(cardinalityAdjusted.filter((result) => result.mapping.status === 'MAPPED').map((result) => result.component.component_id))
  const representedPartIds = new Set(parts.map((part) => part.part_id))
  const preservedComponentIds = sortedUnique(components
    .filter((component) => component.component_id === bladeComponent.component_id || representedPartIds.has(component.component_id))
    .map((component) => component.component_id))
  const unsupportedComponentIds = sortedUnique(unsupportedComponents.map((result) => result.component.component_id))
  const importedMaterialIds = sortedUnique(sourceMaterials.map((material) => material.material_id))
  const mappedMaterialIds = sortedUnique(validMaterials.map((material) => material.material_id))
  const preservedMaterialIds = sortedUnique(materialZones.map((zone) => zone.material_zone_id))
  const unsupportedMaterialIds = sortedUnique(unsupportedMaterials.map((material) => material.material_id))
  const blockedBy = Object.freeze([
    ...unsupportedComponentIds.map((id) => `component:${id}`),
    ...unsupportedMaterialIds.map((id) => `material:${id}`),
  ].sort())
  const componentMappings = Object.freeze(cardinalityAdjusted.map((result) => result.mapping).sort((left, right) => left.source_order - right.source_order || left.source_component_id.localeCompare(right.source_component_id)))
  const materialMappings = Object.freeze(sourceMaterials.map((material) => materialMapping(material)).sort((left, right) => left.source_order - right.source_order || left.source_material_id.localeCompare(right.source_material_id)))
  const receiptBase = {
    schema_version: 'Img2ThreeJsKnifeImportReceipt@1' as const,
    upstream_revision: PINNED_UPSTREAM as typeof PINNED_UPSTREAM,
    source_schema_version: schemaVersion,
    source_identity: IMG2THREEJS_SOURCE_IDENTITY,
    source_target_name: targetName,
    source_blade_component_id: bladeComponent.component_id,
    imported_station_count: bladeBuild.station_count,
    imported_component_ids: Object.freeze(importedComponentIds),
    mapped_component_ids: Object.freeze(mappedComponentIds),
    preserved_component_ids: Object.freeze(preservedComponentIds),
    unsupported_component_ids: Object.freeze(unsupportedComponentIds),
    ignored_component_ids: Object.freeze(unsupportedComponentIds),
    imported_material_ids: Object.freeze(importedMaterialIds),
    mapped_material_ids: Object.freeze(mappedMaterialIds),
    preserved_material_ids: Object.freeze(preservedMaterialIds),
    unsupported_material_ids: Object.freeze(unsupportedMaterialIds),
    component_mappings: componentMappings,
    material_mappings: materialMappings,
    transform_policy: KNIFE_IMPORT_TRANSFORM_POLICY,
    full_assembly_status: fullAssemblyStatus,
    full_assembly_blocked_by: blockedBy,
    execution_performed: false as const,
    network_used: false as const,
    quality_status: 'NOT_RUN' as const,
  }
  const deterministicFingerprint = importFingerprint(program, receiptBase)
  const receipt: Img2ThreeJsImportReceipt = Object.freeze({
    ...receiptBase,
    deterministic_fingerprint: deterministicFingerprint,
  })

  return { program, receipt }
}

function validateSourceCoordinateFrame(value: unknown): void {
  const frame = object(value, 'coordinateFrame')
  const keys = Object.keys(frame).sort()
  const expected = ['forward', 'units', 'up']
  if (keys.length !== expected.length || keys.some((key, index) => key !== expected[index])) {
    throw new Img2ThreeJsImportError('coordinateFrame keys are not closed')
  }
  if (frame.up !== '+Y' || frame.forward !== '+Z' || frame.units !== 'normalized design units') {
    throw new Img2ThreeJsImportError('coordinateFrame must be +Y up, +Z forward, normalized design units')
  }
}

function buildBlade(
  component: ParsedSourceComponent,
  validMaterialById: ReadonlyMap<string, ParsedSourceMaterial & { zone: KnifeMaterialZone }>,
  allMaterialById: ReadonlyMap<string, ParsedSourceMaterial>,
): BladeBuild {
  if (component.transform_error) throw new Img2ThreeJsImportError(`blade ${component.component_id} transform unsupported: ${component.transform_error}`)
  if (component.material_error || !component.material_id) throw new Img2ThreeJsImportError(`blade ${component.component_id} material is unsupported`)
  const material = validMaterialById.get(component.material_id)
  if (!material) {
    const sourceMaterial = allMaterialById.get(component.material_id)
    throw new Img2ThreeJsImportError(`blade material ${component.material_id} is unsupported${sourceMaterial?.reason ? `: ${sourceMaterial.reason}` : ''}`)
  }
  const sourceEnvelopeComponent = sourceEnvelopeComponentFor(component)
  if (sourceEnvelopeComponent.primitive !== 'ground-blade') throw new Img2ThreeJsImportError('blade source envelope primitive drifted')
  const geometry = sourceEnvelopeComponent.geometry
  if (geometry.primitive !== 'ground-blade') throw new Img2ThreeJsImportError('blade source geometry primitive drifted')
  const longitudinalAxis = transformedAxis(component.transform, [1, 0, 0], 'blade longitudinal axis')
  const depthAxis = transformedAxis(component.transform, [0, 0, 1], 'blade depth axis')
  if (longitudinalAxis.axis !== 'x' || depthAxis.axis !== 'z') throw new Img2ThreeJsImportError('blade transform must preserve canonical x/z axes')
  const transformedStations = geometry.stations.map((station) => ({
    spine: transformPoint(component.transform, [station[0], station[1], 0]),
    edge: transformPoint(component.transform, [station[0], station[2], 0]),
  }))
  if (transformedStations.some((station) => Math.abs(station.spine[0] - station.edge[0]) > 1e-4)) throw new Img2ThreeJsImportError('blade transform separates spine and edge longitudinal coordinates')
  const firstDelta = transformedStations[transformedStations.length - 1].spine[0] - transformedStations[0].spine[0]
  if (Math.abs(firstDelta) <= 1e-6) throw new Img2ThreeJsImportError('blade stations need a positive transformed longitudinal span')
  const orderedStations = firstDelta < 0 ? [...transformedStations].reverse() : transformedStations
  for (let index = 1; index < orderedStations.length; index += 1) {
    if (orderedStations[index].spine[0] <= orderedStations[index - 1].spine[0]) throw new Img2ThreeJsImportError('blade transformed stations must remain strictly longitudinal')
  }
  const xMin = orderedStations[0].spine[0]
  const xMax = orderedStations[orderedStations.length - 1].spine[0]
  const span = xMax - xMin
  const normalizedX = (x: number): number => ((x - xMin) / span) * 2 - 1 + component.transform.position[0]
  const spinePoints = orderedStations.map((station) => [normalizedX(station.spine[0]), station.spine[1], station.spine[2]] as KnifeVec3)
  const edgePoints = orderedStations.map((station) => [normalizedX(station.edge[0]), station.edge[1], station.edge[2]] as KnifeVec3)
  const calibrationIndices = uniqueCalibrationIndices(orderedStations.length)
  const roles = ['root', 'shoulder', 'belly', 'tip'] as const
  const sections = calibrationIndices.map((stationIndex, index) => {
    const station = orderedStations[stationIndex]
    const halfWidth = Math.max(Math.hypot(station.spine[1] - station.edge[1], station.spine[2] - station.edge[2]) * 0.5, 0.001)
    const tipScale = index === roles.length - 1 ? 0.12 : 1
    return {
      section_id: `section-${roles[index]}`,
      role: roles[index],
      u: (station.spine[0] - xMin) / span,
      half_width: halfWidth,
      thickness: Math.max(geometry.thickness * component.transform.scale[2] * tipScale, 0.001),
      edge_offset: 0,
      spine_offset: 0,
      asymmetry: 0,
      twist: 0,
    }
  })
  const edgeMaterialId = chooseEdgeMaterialId(allMaterialById, component.material_id)
  return {
    source_component: sourceEnvelopeComponent,
    material_id: material.zone.material_zone_id,
    edge_material_id: edgeMaterialId,
    station_count: geometry.stations.length,
    blade_surface: {
      spine_curve: { curve_id: 'imported-spine', basis: 'nurbs-like', control_points: spinePoints },
      cutting_edge_curve: { curve_id: 'imported-cutting-edge', basis: 'nurbs-like', control_points: edgePoints },
      sections,
      surface_roles: ['blade-body', 'cutting-edge', 'spine', 'root-transition'],
    },
  }
}

function mapBladeComponent(component: ParsedSourceComponent, blade: BladeBuild): MappedComponent {
  return {
    component,
    source_envelope_component: blade.source_component,
    mapping: {
      source_component_id: component.component_id,
      source_order: component.source_order,
      source_role: component.role,
      source_primitive: component.primitive,
      source_material_id: component.material_id,
      target_part_ids: Object.freeze(['blade-body', 'cutting-edge']),
      target_material_zone_id: blade.material_id,
      status: 'MAPPED',
      projection: 'exact',
    },
  }
}

function mapAssemblyComponent(
  component: ParsedSourceComponent,
  validMaterialById: ReadonlyMap<string, ParsedSourceMaterial & { zone: KnifeMaterialZone }>,
): MappedComponent {
  const base = {
    source_component_id: component.component_id,
    source_order: component.source_order,
    source_role: component.role,
    source_primitive: component.primitive,
    source_material_id: component.material_id,
  }
  const unsupported = (reason: string): MappedComponent => ({
    component,
    mapping: {
      ...base,
      target_part_ids: Object.freeze([]),
      target_material_zone_id: null,
      status: 'UNSUPPORTED',
      projection: 'unsupported',
      reason: boundedReason(reason),
    },
  })
  if (component.material_error || !component.material_id) return unsupported(component.material_error ?? 'source material ID is missing')
  if (!validMaterialById.has(component.material_id)) return unsupported(`source material ${component.material_id} is invalid or missing`)
  if (component.parent_error) return unsupported(component.parent_error)
  if (component.transform_error) return unsupported(component.transform_error)
  if (component.component_id === 'blade-body' || component.component_id === 'cutting-edge') return unsupported('component ID is reserved by the blade dual-track projection')
  try {
    const sourceEnvelopeComponent = sourceEnvelopeComponentFor(component)
    const spec = semanticSpecFor(component, sourceEnvelopeComponent)
    return {
      component,
      source_envelope_component: sourceEnvelopeComponent,
      spec,
      mapping: {
        ...base,
        target_part_ids: Object.freeze([component.component_id]),
        target_material_zone_id: component.material_id,
        status: 'MAPPED',
        projection: 'exact',
      },
    }
  } catch (error) {
    return unsupported(error instanceof Error ? error.message : 'closed component projection failed')
  }
}

function enforceAssemblyCardinality(results: readonly MappedComponent[]): readonly MappedComponent[] {
  const singletonRoles = new Set(['guard', 'grip', 'pommel'])
  const seen = new Set<string>()
  return results.map((result) => {
    if (result.mapping.status !== 'MAPPED' || !singletonRoles.has(result.component.role)) return result
    if (!seen.has(result.component.role)) {
      seen.add(result.component.role)
      return result
    }
    return {
      component: result.component,
      mapping: {
        ...result.mapping,
        target_part_ids: Object.freeze([]),
        target_material_zone_id: null,
        status: 'UNSUPPORTED' as const,
        projection: 'unsupported' as const,
        reason: `closed assembly supports one ${result.component.role} component`,
      },
    }
  })
}

function buildAssembly(results: readonly MappedComponent[]): KnifeAssembly | undefined {
  const specs = results.map((result) => result.spec).filter((spec): spec is KnifeAssemblyPrimitiveSpec => spec !== undefined).sort(specOrder)
  if (specs.length === 0) return undefined
  const assembly: {
    guard?: Extract<KnifeAssemblyPrimitiveSpec, { primitive: 'guard' }>
    grip?: Extract<KnifeAssemblyPrimitiveSpec, { primitive: 'grip' }>
    pommel?: Extract<KnifeAssemblyPrimitiveSpec, { primitive: 'pommel' }>
    fasteners: Extract<KnifeAssemblyPrimitiveSpec, { primitive: 'fastener' }>[]
    gems: Extract<KnifeAssemblyPrimitiveSpec, { primitive: 'gem' }>[]
    reliefs: Extract<KnifeAssemblyPrimitiveSpec, { primitive: 'relief' }>[]
  } = { fasteners: [], gems: [], reliefs: [] }
  for (const spec of specs) {
    if (spec.primitive === 'guard') assembly.guard = spec
    else if (spec.primitive === 'grip') assembly.grip = spec
    else if (spec.primitive === 'pommel') assembly.pommel = spec
    else if (spec.primitive === 'fastener') assembly.fasteners.push(spec)
    else if (spec.primitive === 'gem') assembly.gems.push(spec)
    else assembly.reliefs.push(spec)
  }
  return {
    ...(assembly.guard ? { guard: assembly.guard } : {}),
    ...(assembly.grip ? { grip: assembly.grip } : {}),
    ...(assembly.pommel ? { pommel: assembly.pommel } : {}),
    ...(assembly.fasteners.length > 0 ? { fasteners: assembly.fasteners } : {}),
    ...(assembly.gems.length > 0 ? { gems: assembly.gems } : {}),
    ...(assembly.reliefs.length > 0 ? { reliefs: assembly.reliefs } : {}),
  }
}

function buildSourceEnvelope(
  sourceSchemaVersion: string,
  targetName: string,
  results: readonly MappedComponent[],
  materials: readonly (ParsedSourceMaterial & { zone: KnifeMaterialZone })[],
  tessellation: Img2ThreeJsTessellationTier,
): Img2ThreeJsSourceEnvelope {
  const components = results
    .map((result) => result.source_envelope_component)
    .filter((component): component is Img2ThreeJsSourceComponent => component !== undefined)
    .sort((left, right) => left.source_order - right.source_order || left.component_id.localeCompare(right.component_id))
  if (components.length !== results.length) throw new Img2ThreeJsImportError('full source envelope lost a mapped component')
  const envelope: Img2ThreeJsSourceEnvelope = {
    schema_version: 'Img2ThreeJsSourceEnvelope@1',
    source_schema_version: sourceSchemaVersion,
    source_identity: IMG2THREEJS_SOURCE_IDENTITY,
    target_name: targetName,
    coordinate_frame: 'source-right-x-up-y-forward-z@1',
    components,
    materials: materials.map((material) => ({
      material_id: material.material_id,
      source_order: material.source_order,
      base_color: material.zone.base_color,
      metalness: material.zone.metalness,
      roughness: material.zone.roughness,
    })).sort((left, right) => left.source_order - right.source_order || left.material_id.localeCompare(right.material_id)),
    tessellation,
    max_triangles: MAX_ASSEMBLY_TRIANGLES,
  }
  return freezeImg2ThreeJsSourceEnvelope(envelope)
}

function sourceEnvelopeComponentFor(component: ParsedSourceComponent): Img2ThreeJsSourceComponent {
  if (component.parent_error || component.parent_id !== null) throw new Img2ThreeJsImportError(component.parent_error ?? 'parent_id must be null in the root-only compatibility profile')
  if (component.transform_error) throw new Img2ThreeJsImportError(component.transform_error)
  if (component.material_error || !component.material_id) throw new Img2ThreeJsImportError('source material ID is missing or invalid')
  const role = component.role
  const primitive = component.primitive
  const transform = component.transform
  if (role === 'blade' && primitive === 'ground-blade') {
    return {
      component_id: component.component_id,
      source_order: component.source_order,
      role: 'blade',
      primitive: 'ground-blade',
      material_id: component.material_id,
      parent_id: component.parent_id,
      transform,
      geometry: parseBladeGeometry(component.raw.geometryDescriptor, `${component.component_id}.geometryDescriptor`),
    }
  }
  if ((role === 'guard' || role === 'relief') && primitive === 'extrude') {
    return {
      component_id: component.component_id,
      source_order: component.source_order,
      role,
      primitive: 'extrude',
      material_id: component.material_id,
      parent_id: component.parent_id,
      transform,
      geometry: parseExtrudeGeometry(component.raw.geometryDescriptor, `${component.component_id}.geometryDescriptor`),
    }
  }
  if (role === 'grip' && primitive === 'curve-sweep') {
    return {
      component_id: component.component_id,
      source_order: component.source_order,
      role: 'grip',
      primitive: 'curve-sweep',
      material_id: component.material_id,
      parent_id: component.parent_id,
      transform,
      geometry: parseCurveSweepGeometry(component.raw.geometryDescriptor, `${component.component_id}.geometryDescriptor`),
    }
  }
  if ((role === 'pommel' || role === 'gem') && primitive === 'sphere') {
    return {
      component_id: component.component_id,
      source_order: component.source_order,
      role,
      primitive: 'sphere',
      material_id: component.material_id,
      parent_id: component.parent_id,
      transform,
      geometry: { primitive: 'sphere' },
    }
  }
  if (role === 'fastener' && primitive === 'cylinder') {
    return {
      component_id: component.component_id,
      source_order: component.source_order,
      role: 'fastener',
      primitive: 'cylinder',
      material_id: component.material_id,
      parent_id: component.parent_id,
      transform,
      geometry: { primitive: 'cylinder' },
    }
  }
  throw new Img2ThreeJsImportError(`closed source primitive pair is unsupported: ${role}/${primitive}`)
}

function semanticSpecFor(component: ParsedSourceComponent, source: Img2ThreeJsSourceComponent): KnifeAssemblyPrimitiveSpec {
  const transform = component.transform
  switch (source.role) {
    case 'guard': {
      if (source.geometry.primitive !== 'extrude') throw new Img2ThreeJsImportError('guard geometry drifted')
      const bounds = transformedProfileBounds(source.geometry.profile_2d, source.geometry.depth, transform)
      return {
        primitive: 'guard',
        part_id: component.component_id,
        center: transform.position,
        style: 'classic',
        span: boundedDimension(bounds.size[1], 2, 'guard.span'),
        thickness: boundedDimension(bounds.size[0], 1, 'guard.thickness'),
        depth: boundedDimension(bounds.size[2], 1, 'guard.depth'),
      }
    }
    case 'grip': {
      if (source.geometry.primitive !== 'curve-sweep') throw new Img2ThreeJsImportError('grip geometry drifted')
      transformedAxis(transform, [1, 0, 0], 'grip longitudinal axis')
      const spine = source.geometry.spine.map((point) => transformPoint(transform, point))
      const span = monotonicSpan(spine.map((point) => point[0]), 'grip spine')
      const crossRadius = Math.max(...source.geometry.cross_section.map((point) => Math.hypot(point[0], point[1]))) * Math.max(...transform.scale)
      const centerlineRadius = Math.max(...spine.map((point) => Math.hypot(point[1] - transform.position[1], point[2] - transform.position[2])))
      return {
        primitive: 'grip',
        part_id: component.component_id,
        center: transform.position,
        style: 'classic',
        length: boundedDimension(span, 2, 'grip.length'),
        radius: boundedDimension(centerlineRadius + crossRadius, 1, 'grip.radius'),
        taper: 0,
        facets: 10,
      }
    }
    case 'pommel': {
      const size = transformedUnitSphereBounds(transform).size
      return {
        primitive: 'pommel',
        part_id: component.component_id,
        center: transform.position,
        style: 'classic',
        length: boundedDimension(size[0], 2, 'pommel.length'),
        radius: boundedDimension(size[1] * 0.5, 1, 'pommel.radius'),
        depth: boundedDimension(size[2], 1, 'pommel.depth'),
      }
    }
    case 'fastener': {
      const axis = transformedAxis(transform, [0, 1, 0], 'fastener axis').axis
      return {
        primitive: 'fastener',
        part_id: component.component_id,
        center: transform.position,
        radius: boundedDimension(Math.max(transform.scale[0], transform.scale[2]) * 0.5, 0.5, 'fastener.radius'),
        depth: boundedDimension(transform.scale[1], 1, 'fastener.depth'),
        axis,
      }
    }
    case 'gem': {
      const axis = transformedAxis(transform, [0, 0, 1], 'gem axis').axis
      return {
        primitive: 'gem',
        part_id: component.component_id,
        center: transform.position,
        radius: boundedDimension(Math.max(transform.scale[0], transform.scale[1]) * 0.5, 0.5, 'gem.radius'),
        depth: boundedDimension(transform.scale[2], 1, 'gem.depth'),
        axis,
      }
    }
    case 'relief': {
      if (source.geometry.primitive !== 'extrude') throw new Img2ThreeJsImportError('relief geometry drifted')
      const axis = transformedAxis(transform, [0, 0, 1], 'relief axis').axis
      const bounds = transformedProfileBounds(source.geometry.profile_2d, source.geometry.depth, transform)
      const dimensions = reliefDimensions(bounds.size, axis)
      return {
        primitive: 'relief',
        part_id: component.component_id,
        center: transform.position,
        width: boundedDimension(dimensions.width, 2, 'relief.width'),
        height: boundedDimension(dimensions.height, 1, 'relief.height'),
        depth: boundedDimension(dimensions.depth, 0.5, 'relief.depth'),
        shape: 'panel',
        axis,
      }
    }
    case 'blade':
      throw new Img2ThreeJsImportError('blade has a dedicated dual-track projection')
  }
}

function parseComponents(value: unknown): ParsedSourceComponent[] {
  const rawComponents = array(value, 'componentTree')
  if (rawComponents.length === 0 || rawComponents.length > MAX_SOURCE_COMPONENTS) throw new Img2ThreeJsImportError(`componentTree must contain 1 to ${MAX_SOURCE_COMPONENTS} entries`)
  const seen = new Set<string>()
  return rawComponents.map((item, index) => {
    const raw = object(item, `componentTree[${index}]`)
    const componentId = stableId(raw.id, `componentTree[${index}].id`)
    if (seen.has(componentId)) throw new Img2ThreeJsImportError(`duplicate component ID ${componentId}`)
    seen.add(componentId)
    const role = sourceToken(raw.role, 'unknown')
    const primitive = sourceToken(raw.primitive, 'unknown')
    const material = optionalStableId(raw.material, `componentTree[${index}].material`)
    const parent = optionalParentId(raw.parent, `componentTree[${index}].parent`)
    let transform: Img2ThreeJsSourceTransform = identityTransform()
    let transformError: string | undefined
    try {
      transform = parseTransform(raw.transform, `componentTree[${index}].transform`)
    } catch (error) {
      transformError = boundedReason(error instanceof Error ? error.message : 'transform is invalid')
    }
    return {
      raw,
      component_id: componentId,
      source_order: index,
      role,
      primitive,
      material_id: material.id,
      material_error: material.reason,
      parent_id: parent.id,
      parent_error: parent.reason,
      transform,
      transform_error: transformError,
    }
  })
}

function parseMaterials(value: unknown): ParsedSourceMaterial[] {
  const rawMaterials = array(value, 'materials')
  if (rawMaterials.length === 0 || rawMaterials.length > MAX_SOURCE_MATERIALS) throw new Img2ThreeJsImportError(`materials must contain 1 to ${MAX_SOURCE_MATERIALS} entries`)
  const seen = new Set<string>()
  return rawMaterials.map((item, index) => {
    const raw = object(item, `materials[${index}]`)
    const materialId = stableId(raw.id, `materials[${index}].id`)
    if (seen.has(materialId)) throw new Img2ThreeJsImportError(`duplicate material ID ${materialId}`)
    seen.add(materialId)
    try {
      return { raw, material_id: materialId, source_order: index, zone: materialZone(materialId, raw) }
    } catch (error) {
      return {
        raw,
        material_id: materialId,
        source_order: index,
        zone: undefined,
        reason: boundedReason(error instanceof Error ? error.message : 'material is invalid'),
      }
    }
  })
}

function materialMapping(material: ParsedSourceMaterial): Img2ThreeJsMaterialMapping {
  return material.zone
    ? {
        source_material_id: material.material_id,
        source_order: material.source_order,
        target_material_zone_id: material.material_id,
        status: 'MAPPED',
        projection: 'exact',
      }
    : {
        source_material_id: material.material_id,
        source_order: material.source_order,
        target_material_zone_id: null,
        status: 'UNSUPPORTED',
        projection: 'unsupported',
        reason: material.reason ?? 'source material is unsupported',
      }
}

function materialZone(id: string, source: Record<string, unknown>): KnifeMaterialZone {
  const color = typeof source.baseColor === 'string' ? source.baseColor : typeof source.color === 'string' ? source.color : ''
  if (!/^#[0-9a-f]{6}$/i.test(color)) throw new Img2ThreeJsImportError(`material ${id} baseColor must be #RRGGBB`)
  return {
    material_zone_id: id,
    model: 'mesh-standard-layered@1',
    base_color: color,
    metalness: scalar(source.metalness, 0.5, `material ${id}.metalness`),
    roughness: scalar(source.roughness, 0.5, `material ${id}.roughness`),
  }
}

function scalar(value: unknown, fallback: number, label: string): number {
  if (value === undefined) return fallback
  const number = typeof value === 'number'
    ? value
    : value && typeof value === 'object' && typeof (value as Record<string, unknown>).base === 'number'
      ? (value as Record<string, number>).base
      : Number.NaN
  if (!Number.isFinite(number) || number < 0 || number > 1) throw new Img2ThreeJsImportError(`${label} must be finite and in [0,1]`)
  return number
}

function parseBladeGeometry(value: unknown, label: string): Extract<Img2ThreeJsSourceGeometry, { primitive: 'ground-blade' }> {
  const descriptor = object(value, label)
  const bladeSpec = object(descriptor.bladeSpec, `${label}.bladeSpec`)
  const stations = parseStations(bladeSpec.stations)
  const thickness = boundedFinite(bladeSpec.thickness, `${label}.bladeSpec.thickness`, 1e-5, MAX_BLADE_THICKNESS)
  const grindFrac = optionalBoundedFinite(bladeSpec.grindFrac, 0.55, `${label}.bladeSpec.grindFrac`, 0, 1)
  const swedgeFromTipFrac = optionalBoundedFinite(bladeSpec.swedgeFromTipFrac, 0.34, `${label}.bladeSpec.swedgeFromTipFrac`, 0, 1)
  return {
    primitive: 'ground-blade',
    stations,
    thickness,
    grind_frac: grindFrac,
    swedge_from_tip_frac: swedgeFromTipFrac,
  }
}

function parseExtrudeGeometry(value: unknown, label: string): Extract<Img2ThreeJsSourceGeometry, { primitive: 'extrude' }> {
  const descriptor = object(value, label)
  const profile = object(descriptor.profile2D, `${label}.profile2D`)
  const points = parseVec2List(profile.points, `${label}.profile2D.points`, 3, 64)
  const depth = boundedFinite(profile.depth, `${label}.profile2D.depth`, 1e-5, 1)
  return { primitive: 'extrude', profile_2d: points, depth }
}

function parseCurveSweepGeometry(value: unknown, label: string): Extract<Img2ThreeJsSourceGeometry, { primitive: 'curve-sweep' }> {
  const descriptor = object(value, label)
  const sweep = object(descriptor.curveSweep, `${label}.curveSweep`)
  const spine = parseVec3List(sweep.spine, `${label}.curveSweep.spine`, 3, 64)
  const crossSection = object(sweep.crossSection, `${label}.curveSweep.crossSection`)
  const crossPoints = parseVec2List(crossSection.points, `${label}.curveSweep.crossSection.points`, 3, 64)
  if (sweep.closed !== undefined && typeof sweep.closed !== 'boolean') throw new Img2ThreeJsImportError(`${label}.curveSweep.closed must be boolean`)
  return { primitive: 'curve-sweep', spine, cross_section: crossPoints, closed: sweep.closed ?? false }
}

function parseStations(value: unknown): readonly Img2ThreeJsSourceVec3[] {
  const raw = array(value, 'bladeSpec.stations')
  if (raw.length < 4 || raw.length > 64) throw new Img2ThreeJsImportError('bladeSpec.stations count must be in [4, 64]')
  const stations = raw.map((item, index) => {
    const point = parseVec3(item, `bladeSpec.stations[${index}]`, MAX_SOURCE_COORDINATE)
    if (point[1] <= point[2]) throw new Img2ThreeJsImportError(`station ${index} has crossing spine/edge rails`)
    return point
  })
  for (let index = 1; index < stations.length; index += 1) {
    if (stations[index][0] <= stations[index - 1][0]) throw new Img2ThreeJsImportError('blade stations must be strictly longitudinal')
  }
  return stations
}

function parseVec2List(value: unknown, label: string, minimum: number, maximum: number): readonly Img2ThreeJsSourceVec2[] {
  const values = array(value, label)
  if (values.length < minimum || values.length > maximum) throw new Img2ThreeJsImportError(`${label} count must be in [${minimum}, ${maximum}]`)
  return values.map((item, index) => parseVec2(item, `${label}[${index}]`, MAX_SOURCE_COORDINATE))
}

function parseVec3List(value: unknown, label: string, minimum: number, maximum: number): readonly Img2ThreeJsSourceVec3[] {
  const values = array(value, label)
  if (values.length < minimum || values.length > maximum) throw new Img2ThreeJsImportError(`${label} count must be in [${minimum}, ${maximum}]`)
  return values.map((item, index) => parseVec3(item, `${label}[${index}]`, MAX_SOURCE_COORDINATE))
}

function parseVec2(value: unknown, label: string, maximum: number): Img2ThreeJsSourceVec2 {
  if (!Array.isArray(value) || value.length !== 2) throw new Img2ThreeJsImportError(`${label} must be [x,y]`)
  return [boundedFinite(value[0], `${label}[0]`, -maximum, maximum), boundedFinite(value[1], `${label}[1]`, -maximum, maximum)]
}

function parseVec3(value: unknown, label: string, maximum: number): Img2ThreeJsSourceVec3 {
  if (!Array.isArray(value) || value.length !== 3) throw new Img2ThreeJsImportError(`${label} must be [x,y,z]`)
  return [boundedFinite(value[0], `${label}[0]`, -maximum, maximum), boundedFinite(value[1], `${label}[1]`, -maximum, maximum), boundedFinite(value[2], `${label}[2]`, -maximum, maximum)]
}

function parseTransform(value: unknown, label: string): Img2ThreeJsSourceTransform {
  if (value === undefined) return identityTransform()
  const raw = object(value, label)
  const unsupportedKeys = Object.keys(raw).filter((key) => key !== 'position' && key !== 'rotation' && key !== 'scale')
  if (unsupportedKeys.length > 0) {
    throw new Img2ThreeJsImportError(`${label} contains unsupported keys: ${unsupportedKeys.sort().join(',')}`)
  }
  const position = parseVec3(raw.position ?? [0, 0, 0], `${label}.position`, KNIFE_IMPORT_TRANSFORM_POLICY.position_abs_max)
  const rotation = parseVec3(raw.rotation ?? [0, 0, 0], `${label}.rotation`, MAX_ROTATION)
  const scale = parseVec3(raw.scale ?? [1, 1, 1], `${label}.scale`, MAX_SCALE)
  for (const [index, item] of scale.entries()) if (item <= MIN_SCALE) throw new Img2ThreeJsImportError(`${label}.scale[${index}] must be > ${MIN_SCALE}`)
  for (const [index, angle] of rotation.entries()) {
    const quarterTurn = Math.round(angle / (Math.PI * 0.5))
    if (Math.abs(angle - quarterTurn * Math.PI * 0.5) > KNIFE_IMPORT_TRANSFORM_POLICY.quarter_turn_tolerance_radians) throw new Img2ThreeJsImportError(`${label}.rotation[${index}] must be an axis-aligned quarter turn`)
  }
  return {
    position,
    rotation_xyz: rotation,
    scale,
    pivot: [0, 0, 0],
    rotation_order: 'XYZ',
  }
}

function identityTransform(): Img2ThreeJsSourceTransform {
  return {
    position: [0, 0, 0],
    rotation_xyz: [0, 0, 0],
    scale: [1, 1, 1],
    pivot: [0, 0, 0],
    rotation_order: 'XYZ',
  }
}

function transformPoint(transform: Img2ThreeJsSourceTransform, point: Img2ThreeJsSourceVec3): KnifeVec3 {
  const matrix = rotationMatrix(transform.rotation_xyz)
  const local: KnifeVec3 = [
    (point[0] - transform.pivot[0]) * transform.scale[0],
    (point[1] - transform.pivot[1]) * transform.scale[1],
    (point[2] - transform.pivot[2]) * transform.scale[2],
  ]
  const rotated = multiplyMatrix(matrix, local)
  return [rotated[0] + transform.pivot[0] + transform.position[0], rotated[1] + transform.pivot[1] + transform.position[1], rotated[2] + transform.pivot[2] + transform.position[2]]
}

function transformVector(transform: Img2ThreeJsSourceTransform, vector: Img2ThreeJsSourceVec3): KnifeVec3 {
  const matrix = rotationMatrix(transform.rotation_xyz)
  return multiplyMatrix(matrix, [vector[0] * transform.scale[0], vector[1] * transform.scale[1], vector[2] * transform.scale[2]])
}

function rotationMatrix(rotation: Img2ThreeJsSourceVec3): readonly [number, number, number, number, number, number, number, number, number] {
  const x = snapQuarter(Math.cos(rotation[0]))
  const sx = snapQuarter(Math.sin(rotation[0]))
  const y = snapQuarter(Math.cos(rotation[1]))
  const sy = snapQuarter(Math.sin(rotation[1]))
  const z = snapQuarter(Math.cos(rotation[2]))
  const sz = snapQuarter(Math.sin(rotation[2]))
  return [
    y * z, z * sx * sy - x * sz, sx * sz + x * z * sy,
    y * sz, x * z + sx * sy * sz, x * sy * sz - z * sx,
    -sy, y * sx, x * y,
  ]
}

function multiplyMatrix(matrix: readonly [number, number, number, number, number, number, number, number, number], point: KnifeVec3): KnifeVec3 {
  return [matrix[0] * point[0] + matrix[1] * point[1] + matrix[2] * point[2], matrix[3] * point[0] + matrix[4] * point[1] + matrix[5] * point[2], matrix[6] * point[0] + matrix[7] * point[1] + matrix[8] * point[2]]
}

function snapQuarter(value: number): number {
  if (Math.abs(value) <= 1e-8) return 0
  if (Math.abs(Math.abs(value) - 1) <= 1e-8) return Math.sign(value)
  return value
}

function transformedAxis(transform: Img2ThreeJsSourceTransform, axis: KnifeVec3, label: string): { readonly axis: KnifeAssemblyAxis; readonly sign: -1 | 1 } {
  const transformed = transformVector(transform, axis)
  const length = Math.hypot(transformed[0], transformed[1], transformed[2])
  if (!Number.isFinite(length) || length <= MIN_SCALE) throw new Img2ThreeJsImportError(`${label} has a degenerate scale`)
  const vector: KnifeVec3 = [transformed[0] / length, transformed[1] / length, transformed[2] / length]
  const magnitudes = vector.map((value) => Math.abs(value))
  const dominant = magnitudes.indexOf(Math.max(...magnitudes))
  if (Math.abs(magnitudes[dominant] - 1) > 1e-4 || magnitudes.some((value, index) => index !== dominant && value > 1e-4)) throw new Img2ThreeJsImportError(`${label} must map to one canonical axis`)
  return { axis: (['x', 'y', 'z'] as const)[dominant], sign: vector[dominant] < 0 ? -1 : 1 }
}

interface Bounds {
  readonly min: KnifeVec3
  readonly max: KnifeVec3
  readonly size: KnifeVec3
}

function transformedProfileBounds(points: readonly Img2ThreeJsSourceVec2[], depth: number, transform: Img2ThreeJsSourceTransform): Bounds {
  const transformed = points.flatMap((point) => [transformPoint(transform, [point[0], point[1], 0]), transformPoint(transform, [point[0], point[1], depth])])
  return boundsOf(transformed)
}

function transformedUnitSphereBounds(transform: Img2ThreeJsSourceTransform): Bounds {
  const points: KnifeVec3[] = []
  for (const x of [-0.5, 0.5]) for (const y of [-0.5, 0.5]) for (const z of [-0.5, 0.5]) points.push(transformPoint(transform, [x, y, z]))
  return boundsOf(points)
}

function boundsOf(points: readonly KnifeVec3[]): Bounds {
  const min: [number, number, number] = [Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY]
  const max: [number, number, number] = [Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY]
  for (const point of points) for (let index = 0; index < 3; index += 1) {
    min[index] = Math.min(min[index], point[index])
    max[index] = Math.max(max[index], point[index])
  }
  return { min, max, size: [max[0] - min[0], max[1] - min[1], max[2] - min[2]] }
}

function reliefDimensions(size: KnifeVec3, axis: KnifeAssemblyAxis): { readonly width: number; readonly height: number; readonly depth: number } {
  if (axis === 'x') return { width: size[1], height: size[2], depth: size[0] }
  if (axis === 'y') return { width: size[0], height: size[2], depth: size[1] }
  return { width: size[0], height: size[1], depth: size[2] }
}

function monotonicSpan(values: readonly number[], label: string): number {
  if (values.length < 2) throw new Img2ThreeJsImportError(`${label} needs at least two points`)
  let direction = Math.sign(values[values.length - 1] - values[0])
  if (direction === 0) throw new Img2ThreeJsImportError(`${label} has no longitudinal span`)
  if (direction < 0) direction = -1
  for (let index = 1; index < values.length; index += 1) if (direction * (values[index] - values[index - 1]) <= 1e-6) throw new Img2ThreeJsImportError(`${label} must remain monotonic on canonical x`)
  return Math.abs(values[values.length - 1] - values[0])
}

function boundedDimension(value: number, maximum: number, label: string): number {
  if (!Number.isFinite(value) || value <= 1e-5 || value > maximum) throw new Img2ThreeJsImportError(`${label} must be finite and in (1e-5, ${maximum}]`)
  return value
}

function chooseEdgeMaterialId(materials: ReadonlyMap<string, ParsedSourceMaterial>, bladeMaterialId: string): string {
  for (const candidate of ['substrate', 'edge', 'cutting-edge']) {
    if (materials.get(candidate)?.zone) return candidate
  }
  return bladeMaterialId
}

function sourceUnknowns(source: Record<string, unknown>, results: readonly MappedComponent[], materials: readonly ParsedSourceMaterial[]): readonly string[] {
  const values: string[] = []
  const silhouette = source.silhouette
  if (silhouette && typeof silhouette === 'object' && !Array.isArray(silhouette)) {
    const hidden = (silhouette as Record<string, unknown>).hidden
    if (Array.isArray(hidden)) for (const item of hidden) if (typeof item === 'string' && item.length > 0 && item.length <= 120) values.push(`source-hidden:${item}`)
  }
  for (const result of results) if (result.mapping.status === 'UNSUPPORTED') values.push(`unsupported-component:${result.component.component_id}:${result.mapping.reason ?? 'closed projection failed'}`)
  for (const material of materials) if (!material.zone) values.push(`unsupported-material:${material.material_id}:${material.reason ?? 'invalid'}`)
  return Object.freeze([...new Set(values)].slice(0, 32))
}

function importFingerprint(program: KnifeSceneProgram, receipt: Omit<Img2ThreeJsImportReceipt, 'deterministic_fingerprint'>): string {
  return fnv1a64(JSON.stringify({
    upstream_revision: receipt.upstream_revision,
    source_schema_version: receipt.source_schema_version,
    source_identity: receipt.source_identity,
    source_target_name: receipt.source_target_name,
    source_blade_component_id: receipt.source_blade_component_id,
    imported_station_count: receipt.imported_station_count,
    imported_component_ids: receipt.imported_component_ids,
    mapped_component_ids: receipt.mapped_component_ids,
    preserved_component_ids: receipt.preserved_component_ids,
    unsupported_component_ids: receipt.unsupported_component_ids,
    imported_material_ids: receipt.imported_material_ids,
    mapped_material_ids: receipt.mapped_material_ids,
    preserved_material_ids: receipt.preserved_material_ids,
    unsupported_material_ids: receipt.unsupported_material_ids,
    component_mappings: receipt.component_mappings,
    material_mappings: receipt.material_mappings,
    full_assembly_status: receipt.full_assembly_status,
    full_assembly_blocked_by: receipt.full_assembly_blocked_by,
    asset_id: program.asset_id,
    parts: program.parts,
    material_zones: program.material_zones,
    sections: program.blade_surface.sections,
    source_envelope: program.source_envelope ?? null,
  }))
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

function resolveSourceTessellation(value: unknown): Img2ThreeJsTessellationTier {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return 'hero'
  const target = (value as Record<string, unknown>).targetTriangles
  if (typeof target !== 'number' || !Number.isFinite(target) || target <= 0) return 'hero'
  if (target <= 6_000) return 'low'
  if (target <= 60_000) return 'standard'
  return 'hero'
}

function specOrder(left: KnifeAssemblyPrimitiveSpec, right: KnifeAssemblyPrimitiveSpec): number {
  const order: Readonly<Record<KnifeAssemblyPrimitiveSpec['primitive'], number>> = { guard: 0, grip: 1, pommel: 2, fastener: 3, gem: 4, relief: 5 }
  return order[left.primitive] - order[right.primitive] || left.part_id.localeCompare(right.part_id)
}

function sortedUnique(values: readonly string[]): string[] {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right))
}

function optionalStableId(value: unknown, label: string): { readonly id: string | null; readonly reason?: string } {
  if (value === undefined || value === null) return { id: null, reason: `${label} is missing` }
  if (typeof value !== 'string' || !STABLE_ID_PATTERN.test(value)) return { id: null, reason: `${label} must be a bounded stable ID` }
  return { id: value }
}

function optionalParentId(value: unknown, label: string): { readonly id: string | null; readonly reason?: string } {
  if (value === undefined || value === null) return { id: null }
  if (typeof value !== 'string' || !STABLE_ID_PATTERN.test(value)) return { id: null, reason: `${label} must be null or a bounded stable ID` }
  return { id: null, reason: `${label} must be null in the root-only compatibility profile` }
}

function sourceToken(value: unknown, fallback: string): string {
  return typeof value === 'string' && value.length > 0 && value.length <= 64 ? value : fallback
}

function stableId(value: unknown, label: string): string {
  if (typeof value !== 'string' || !STABLE_ID_PATTERN.test(value)) throw new Img2ThreeJsImportError(`${label} must be a bounded stable ID`)
  return value
}

function boundedFinite(value: unknown, label: string, minimum: number, maximum: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < minimum || value > maximum) throw new Img2ThreeJsImportError(`${label} must be finite and in [${minimum}, ${maximum}]`)
  return value
}

function optionalBoundedFinite(value: unknown, fallback: number, label: string, minimum: number, maximum: number): number {
  return value === undefined ? fallback : boundedFinite(value, label, minimum, maximum)
}

function boundedReason(value: string): string {
  return value.replace(/\s+/g, ' ').slice(0, 160)
}

function object(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Img2ThreeJsImportError(`${label} must be an object`)
  return value as Record<string, unknown>
}

function array(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) throw new Img2ThreeJsImportError(`${label} must be an array`)
  return value
}

function text(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 160) throw new Img2ThreeJsImportError(`${label} must be bounded text`)
  return value
}

function safeId(value: string): string {
  const normalized = value.normalize('NFKD').replace(/[^a-zA-Z0-9_.-]+/g, '-').replace(/^-+|-+$/g, '')
  const candidate = /^[a-zA-Z]/.test(normalized) ? normalized : `knife-${normalized}`
  return (candidate || 'imported-knife').slice(0, 64)
}

function uniqueCalibrationIndices(count: number): [number, number, number, number] {
  const shoulder = Math.max(1, Math.min(count - 3, Math.round((count - 1) * 0.28)))
  const belly = Math.max(shoulder + 1, Math.min(count - 2, Math.round((count - 1) * 0.68)))
  return [0, shoulder, belly, count - 1]
}
