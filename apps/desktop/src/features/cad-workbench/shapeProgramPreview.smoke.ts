import * as THREE from 'three'
import { buildShapeProgramPreview } from './shapeProgramPreview.js'

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

const program = {
  schema_version: 'ShapeProgram@1',
  operations: [
    { operation_id: 'op_box', op: 'box', inputs: [], args: { part_role: 'body', position: [0, 0, 0], size: [240, 120, 100], material_id: 'mat_automotive_paint' } },
    { operation_id: 'op_cylinder', op: 'cylinder', inputs: [], args: { part_role: 'wheel', position: [-180, 0, 0], radius: 48, height: 80, axis: [0, 0, 1] } },
    { operation_id: 'op_capsule', op: 'capsule', inputs: [], args: { part_role: 'nacelle', position: [180, 0, 0], radius: 35, height: 150, axis: [1, 0, 0] } },
    { operation_id: 'op_wedge', op: 'wedge', inputs: [], args: { part_role: 'nose', position: [0, 100, 0], size: [180, 90, 120] } },
    { operation_id: 'op_bevel', op: 'bevel_approx', inputs: ['op_box'], args: { part_role: 'beveled_shell', radius: 12, segments: 2 } },
    { operation_id: 'op_light', op: 'box', inputs: [], args: { part_role: 'visual_light_strip_1', position: [0, 150, 80], size: [90, 12, 8], material_id: 'mat_emissive_blue' } },
  ],
  outputs: [
    { operation_id: 'op_box' },
    { operation_id: 'op_cylinder' },
    { operation_id: 'op_capsule' },
    { operation_id: 'op_wedge' },
    { operation_id: 'op_bevel' },
    { operation_id: 'op_light' },
  ],
}

export function runShapeProgramPreviewSmoke(): void {
  const group = buildShapeProgramPreview(program, {
    materialOverride: null,
    selectedAgentPartId: 'part_3_nacelle',
    hiddenAgentPartIds: ['part_4_nose'],
    isolatedAgentPartId: null,
    lockedAgentPartIds: [],
  })
  const kinds = group.userData.forgecadPreviewPrimitiveKinds as string[]
  for (const kind of ['box', 'cylinder', 'capsule', 'wedge', 'bevel_box']) {
    assert(kinds.includes(kind), `preview must retain ${kind}`)
  }
  assert(group.children.length === 6, 'each output primitive must produce a visible mesh adapter')
  const meshes = group.children.filter((child): child is THREE.Mesh => child instanceof THREE.Mesh)
  assert(meshes.length === 6, 'preview children must be meshes')
  assert(meshes.every((mesh) => mesh.castShadow && mesh.receiveShadow && mesh.userData.forgecadDisplayOnly), 'preview meshes must be display-only shadow receivers/casters')
  assert(meshes.every((mesh) => mesh.geometry.getAttribute('position').count > 0), 'each preview mesh must contain geometry')
  const capsule = meshes.find((mesh) => mesh.userData.partRole === 'nacelle')
  assert(capsule?.material instanceof THREE.MeshPhysicalMaterial && capsule.material.emissiveIntensity > 0.2, 'selected capsule must retain selection highlight')
  const wedge = meshes.find((mesh) => mesh.userData.partRole === 'nose')
  assert(wedge?.visible === false, 'hidden wedge must stay hidden in the full primitive preview')
  const paintedBox = meshes.find((mesh) => mesh.userData.partRole === 'body')
  assert(paintedBox?.material instanceof THREE.MeshPhysicalMaterial && paintedBox.material.clearcoat > 0.5, 'visual automotive paint must preserve its presentation clearcoat')
  const lightStrip = meshes.find((mesh) => mesh.userData.partRole === 'visual_light_strip_1')
  assert(lightStrip?.material instanceof THREE.MeshPhysicalMaterial && lightStrip.material.emissiveIntensity > 1, 'showcase light strips must use the bounded emissive presentation material')
  group.traverse((object) => {
    if (object instanceof THREE.Mesh || object instanceof THREE.LineSegments) object.geometry.dispose()
    if (object instanceof THREE.Mesh) object.material.dispose()
    if (object instanceof THREE.LineSegments) object.material.dispose()
  })
}
