export type CandidateGeometryBuffer = {
  values: ArrayLike<number>
  count: number
}

export type CandidateGeometryMesh = {
  position: CandidateGeometryBuffer | null
  index: CandidateGeometryBuffer | null
}

export type CandidateGeometryValidation = {
  ok: boolean
  reason: 'empty_geometry' | 'missing_position' | 'non_finite_coordinate' | 'invalid_face_index' | 'degenerate_face' | null
  meshCount: number
  triangleCount: number
}

export function validateCandidateGeometry(meshes: readonly CandidateGeometryMesh[]): CandidateGeometryValidation {
  if (meshes.length === 0) return failure('empty_geometry', 0, 0)
  let triangleCount = 0
  for (const mesh of meshes) {
    const position = mesh.position
    if (!position || position.count < 3 || !Number.isInteger(position.count)) {
      return failure('missing_position', meshes.length, triangleCount)
    }
    if (!allFinite(position.values)) return failure('non_finite_coordinate', meshes.length, triangleCount)
    const index = mesh.index
    if (index) {
      if (!Number.isInteger(index.count) || index.count < 3 || index.count % 3 !== 0) {
        return failure('invalid_face_index', meshes.length, triangleCount)
      }
      for (let offset = 0; offset < index.count; offset += 3) {
        const a = index.values[offset]
        const b = index.values[offset + 1]
        const c = index.values[offset + 2]
        if (![a, b, c].every((value) => Number.isInteger(value) && value >= 0 && value < position.count)) {
          return failure('invalid_face_index', meshes.length, triangleCount)
        }
        if (a === b || b === c || a === c) return failure('degenerate_face', meshes.length, triangleCount)
      }
      triangleCount += index.count / 3
    } else {
      if (position.count % 3 !== 0) return failure('invalid_face_index', meshes.length, triangleCount)
      triangleCount += position.count / 3
    }
  }
  return triangleCount > 0
    ? { ok: true, reason: null, meshCount: meshes.length, triangleCount }
    : failure('empty_geometry', meshes.length, triangleCount)
}

function allFinite(values: ArrayLike<number>): boolean {
  for (let index = 0; index < values.length; index += 1) {
    if (!Number.isFinite(values[index])) return false
  }
  return true
}

function failure(
  reason: Exclude<CandidateGeometryValidation['reason'], null>,
  meshCount: number,
  triangleCount: number,
): CandidateGeometryValidation {
  return { ok: false, reason, meshCount, triangleCount }
}
