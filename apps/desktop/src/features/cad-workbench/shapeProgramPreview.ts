import * as THREE from 'three'
import { RoundedBoxGeometry } from 'three/examples/jsm/geometries/RoundedBoxGeometry.js'

type Vector3 = [number, number, number]
type PrimitiveKind = 'box' | 'bevel_box' | 'cylinder' | 'capsule' | 'wedge'

type PreviewPrimitive = {
  kind: PrimitiveKind
  partRole: string
  position: Vector3
  rotation: Vector3
  axis: Vector3
  materialId: string | null
  size?: Vector3
  radius?: number
  height?: number
  bevelRadius?: number
  bevelSegments?: number
}

type Operation = {
  operation_id?: unknown
  op?: unknown
  inputs?: unknown
  args?: unknown
}

export type ShapeProgramPreviewOptions = {
  materialOverride: string | null
  selectedAgentPartId: string | null
  hiddenAgentPartIds: string[]
  isolatedAgentPartId: string | null
  lockedAgentPartIds: string[]
}

const LOCAL_Y = new THREE.Vector3(0, 1, 0)

/**
 * Interpret only the same bounded visual primitives that the Geometry Worker
 * already accepts. This is a display adapter, never a geometry truth or an
 * editable ShapeProgram executor.
 */
export function buildShapeProgramPreview(
  program: Record<string, unknown>,
  options: ShapeProgramPreviewOptions,
): THREE.Group {
  const group = new THREE.Group()
  group.name = 'AgentShapeProgramPreview'
  group.userData.forgecadPreviewDisplayOnly = true

  const operations = new Map<string, Operation>()
  for (const candidate of Array.isArray(program.operations) ? program.operations : []) {
    if (!candidate || typeof candidate !== 'object') continue
    const operation = candidate as Operation
    if (typeof operation.operation_id === 'string') operations.set(operation.operation_id, operation)
  }
  const resolving = new Set<string>()
  const resolved = new Map<string, PreviewPrimitive[]>()

  const resolve = (operationId: string): PreviewPrimitive[] => {
    const cached = resolved.get(operationId)
    if (cached) return cached
    if (resolving.has(operationId)) return []
    resolving.add(operationId)
    const operation = operations.get(operationId)
    const args = record(operation?.args)
    const op = typeof operation?.op === 'string' ? operation.op : ''
    const inputs = stringArray(operation?.inputs)
    const common = {
      partRole: stringValue(args.part_role, `part_${operationId}`),
      position: vector3(args.position, [0, 0, 0]),
      rotation: vector3(args.rotation, [0, 0, 0]),
      axis: normalizedAxis(args.axis),
      materialId: typeof args.material_id === 'string' ? args.material_id : options.materialOverride,
    }
    let value: PreviewPrimitive[] = []
    if (op === 'box') {
      value = [{ kind: 'box', ...common, size: positiveVector3(args.size, [100, 100, 100]) }]
    } else if (op === 'cylinder') {
      value = [{ kind: 'cylinder', ...common, radius: positiveNumber(args.radius, 50), height: positiveNumber(args.height, 100) }]
    } else if (op === 'capsule') {
      value = [{ kind: 'capsule', ...common, radius: positiveNumber(args.radius, 50), height: positiveNumber(args.height, 100) }]
    } else if (op === 'wedge') {
      value = [{ kind: 'wedge', ...common, size: positiveVector3(args.size, [100, 100, 100]) }]
    } else if (op === 'bevel_approx') {
      const source = inputs.length ? resolve(inputs[0]) : []
      if (source.length === 1 && source[0].kind === 'box') {
        const base = source[0]
        value = [{
          ...base,
          kind: 'bevel_box',
          partRole: stringValue(args.part_role, base.partRole),
          materialId: typeof args.material_id === 'string' ? args.material_id : base.materialId,
          bevelRadius: positiveNumber(args.radius, Math.min(...(base.size ?? [100, 100, 100])) * 0.04),
          bevelSegments: boundedInteger(args.segments, 1, 1, 3),
        }]
      }
    } else if (op === 'surface_panel') {
      const source = inputs.length ? resolve(inputs[0]) : []
      const base = source.find((item) => item.kind === 'box' || item.kind === 'bevel_box')
      if (base?.size) {
        const panelSize = positiveVector3(args.size, [base.size[0] * 0.6, Math.max(1, Math.min(base.size[1] * 0.08, 20)), base.size[2] * 0.6])
        const axis = vector3(args.axis, [0, 1, 0])
        const sign = axis[1] < 0 ? -1 : 1
        const offset = vector3(args.position, [0, 0, 0])
        value = [
          ...source,
          {
            kind: 'box',
            partRole: stringValue(args.part_role, 'surface_panel'),
            position: [base.position[0] + offset[0], base.position[1] + sign * (base.size[1] / 2 + panelSize[1] / 2), base.position[2] + offset[2]],
            rotation: base.rotation,
            axis: LOCAL_Y.toArray() as Vector3,
            materialId: typeof args.material_id === 'string' ? args.material_id : options.materialOverride,
            size: panelSize,
          },
        ]
      }
    }
    resolving.delete(operationId)
    resolved.set(operationId, value)
    return value
  }

  const primitives: PreviewPrimitive[] = []
  for (const output of Array.isArray(program.outputs) ? program.outputs : []) {
    if (!output || typeof output !== 'object') continue
    const operationId = (output as { operation_id?: unknown }).operation_id
    if (typeof operationId === 'string') primitives.push(...resolve(operationId))
  }
  if (primitives.length === 0) throw new Error('Agent ShapeProgram 没有可显示的轻量形体')

  for (const primitive of primitives) {
    const mesh = new THREE.Mesh(createGeometry(primitive), blockoutMaterial(primitive.materialId, group.children.length))
    mesh.position.set(...primitive.position)
    mesh.rotation.set(...primitive.rotation)
    alignToAxis(mesh, primitive.axis)
    mesh.castShadow = true
    mesh.receiveShadow = true
    mesh.userData.agentBlockout = true
    mesh.userData.partRole = primitive.partRole
    mesh.userData.forgecadPreviewPrimitive = primitive.kind
    mesh.userData.forgecadDisplayOnly = true
    const selected = Boolean(options.selectedAgentPartId && options.selectedAgentPartId.endsWith(`_${primitive.partRole}`))
    const isHidden = options.hiddenAgentPartIds.some((candidate) => candidate.endsWith(`_${primitive.partRole}`))
    const isIsolatedAway = Boolean(options.isolatedAgentPartId && !options.isolatedAgentPartId.endsWith(`_${primitive.partRole}`))
    const isLocked = options.lockedAgentPartIds.some((candidate) => candidate.endsWith(`_${primitive.partRole}`))
    if (selected) {
      mesh.material.emissive.set('#1f64a8')
      mesh.material.emissiveIntensity = 0.42
    } else if (isLocked) {
      mesh.material.emissive.set('#9b6e21')
      mesh.material.emissiveIntensity = 0.24
    }
    mesh.visible = !isHidden && !isIsolatedAway
    mesh.userData.agentPartLocked = isLocked
    const edges = new THREE.LineSegments(
      new THREE.EdgesGeometry(mesh.geometry, 28),
      new THREE.LineBasicMaterial({ color: '#06111d', transparent: true, opacity: 0.26 }),
    )
    edges.userData.forgecadEdgeOverlay = true
    edges.renderOrder = 4
    mesh.add(edges)
    group.add(mesh)
  }
  group.userData.forgecadPreviewPrimitiveKinds = [...new Set(primitives.map((item) => item.kind))]
  return group
}

function createGeometry(primitive: PreviewPrimitive): THREE.BufferGeometry {
  if (primitive.kind === 'cylinder') {
    return new THREE.CylinderGeometry(primitive.radius, primitive.radius, primitive.height, 24, 1, false)
  }
  if (primitive.kind === 'capsule') {
    const radius = Math.min(primitive.radius ?? 50, (primitive.height ?? 100) / 2)
    return new THREE.CapsuleGeometry(radius, Math.max(0.001, (primitive.height ?? 100) - radius * 2), 6, 24)
  }
  if (primitive.kind === 'wedge') return wedgeGeometry(primitive.size ?? [100, 100, 100])
  const size = primitive.size ?? [100, 100, 100]
  const radius = primitive.kind === 'bevel_box'
    ? Math.min(primitive.bevelRadius ?? 1, ...size.map((value) => value / 2))
    : Math.min(Math.max(Math.min(...size) * 0.035, 2), 28)
  return new RoundedBoxGeometry(size[0], size[1], size[2], primitive.kind === 'bevel_box' ? primitive.bevelSegments ?? 1 : 2, radius)
}

function wedgeGeometry(size: Vector3): THREE.BufferGeometry {
  const [width, height, depth] = size
  const hx = width / 2
  const hy = height / 2
  const hz = depth / 2
  const vertices = [
    -hx, -hy, -hz, hx, -hy, -hz, hx, -hy, hz, -hx, -hy, hz, -hx, hy, -hz, -hx, hy, hz,
  ]
  const indices = [
    0, 1, 2, 0, 2, 3,
    0, 4, 5, 0, 5, 3,
    1, 2, 5, 1, 5, 4,
    0, 1, 4,
    3, 5, 2,
  ]
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3))
  geometry.setIndex(indices)
  geometry.computeVertexNormals()
  return geometry
}

function alignToAxis(mesh: THREE.Mesh, axis: Vector3): void {
  const direction = new THREE.Vector3(...axis)
  if (direction.lengthSq() < 1e-6 || direction.equals(LOCAL_Y)) return
  const alignment = new THREE.Quaternion().setFromUnitVectors(LOCAL_Y, direction.normalize())
  mesh.quaternion.multiply(alignment)
}

function blockoutMaterial(materialId: string | null, index: number): THREE.MeshPhysicalMaterial {
  const presets: Record<string, { color: string; metalness: number; roughness: number; clearcoat?: number; transmission?: number; emissive?: string; emissiveIntensity?: number }> = {
    mat_graphite: { color: '#26313b', metalness: 0.78, roughness: 0.3 },
    mat_aluminum: { color: '#aab9c4', metalness: 0.92, roughness: 0.22 },
    mat_automotive_paint: { color: '#3d78b8', metalness: 0.36, roughness: 0.16, clearcoat: 0.86 },
    mat_rubber: { color: '#15191d', metalness: 0.02, roughness: 0.78 },
    mat_composite: { color: '#344451', metalness: 0.22, roughness: 0.5 },
    mat_dark_glass: { color: '#172a3d', metalness: 0.08, roughness: 0.1, transmission: 0.24 },
    mat_signal_red: { color: '#c4493d', metalness: 0.4, roughness: 0.24, clearcoat: 0.48, emissive: '#260403' },
    mat_emissive_blue: { color: '#176fdf', metalness: 0.12, roughness: 0.2, clearcoat: 0.35, emissive: '#0b6fff', emissiveIntensity: 1.15 },
  }
  const fallback: { color: string; metalness: number; roughness: number; clearcoat?: number; transmission?: number; emissive?: string; emissiveIntensity?: number } = index % 3 === 0
    ? { color: '#3b78a8', metalness: 0.52, roughness: 0.3 }
    : index % 3 === 1
      ? { color: '#647f99', metalness: 0.62, roughness: 0.28 }
      : { color: '#c26b4f', metalness: 0.42, roughness: 0.3 }
  const preset = (materialId && presets[materialId]) || fallback
  return new THREE.MeshPhysicalMaterial({
    color: preset.color,
    metalness: preset.metalness,
    roughness: preset.roughness,
    clearcoat: preset.clearcoat ?? 0.12,
    clearcoatRoughness: 0.18,
    transmission: preset.transmission ?? 0,
    ior: preset.transmission ? 1.45 : 1.5,
    transparent: Boolean(preset.transmission),
    opacity: preset.transmission ? 0.82 : 1,
    emissive: preset.emissive ?? '#071725',
    emissiveIntensity: preset.emissiveIntensity ?? (preset.emissive ? 0.18 : 0.12),
    envMapIntensity: 1.08,
  })
}

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {}
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : []
}

function stringValue(value: unknown, fallback: string): string {
  return typeof value === 'string' && value ? value : fallback
}

function vector3(value: unknown, fallback: Vector3): Vector3 {
  if (!Array.isArray(value) || value.length !== 3 || value.some((item) => typeof item !== 'number' || !Number.isFinite(item))) return fallback
  return [value[0], value[1], value[2]]
}

function positiveVector3(value: unknown, fallback: Vector3): Vector3 {
  const vector = vector3(value, fallback)
  return vector.every((item) => item > 0) ? vector : fallback
}

function positiveNumber(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : fallback
}

function boundedInteger(value: unknown, fallback: number, min: number, max: number): number {
  return typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max ? value : fallback
}

function normalizedAxis(value: unknown): Vector3 {
  const axis = vector3(value, [0, 1, 0])
  return axis[0] === 0 && axis[1] === 0 && axis[2] === 0 ? [0, 1, 0] : axis
}
