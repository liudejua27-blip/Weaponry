/**
 * Candidate-bound AuthoringMesh@1 diagnostics for the read-only Viewer.
 *
 * The Runtime owns the projection and its hashes.  This module only validates
 * the returned envelope before it reaches the inspector; it never derives a
 * mesh, topology result, or quality status from the GLB/scene.
 */

export type AuthoringMeshBinding = {
  projectId: string
  candidateId: string
  artifactId: string
  artifactReadbackSha256: string
  programSha256: string
  operatorCatalogSha256: string
  readbackConfigSha256: string
  authoringNodeId: string
  partId: string
}

export type AuthoringMeshCounts = {
  vertex_count: number
  edge_count: number
  half_edge_count: number
  corner_count: number
  face_count: number
  loop_count: number
  ring_count: number
  boundary_edge_count: number
  boundary_half_edge_count: number
  hard_edge_count: number
  crease_edge_count: number
  uv_seam_count: number
}

export type AuthoringMeshTopology = {
  boundary_edge_count: number
  boundary_half_edge_count: number
  non_manifold_edge_count: 0
  orientation_conflict_count: 0
  status: 'closed_manifold' | 'manifold_with_boundary'
  validation_status: 'passed'
  rejection_policy: 'fail-closed-on-non-manifold@1'
  face_cycle_policy: 'next-prev-complete-mutual@1'
  twin_policy: 'boundary-only-null-symmetric@1'
  boundary_policy: 'single-half-edge-per-boundary-edge@1'
}

export type AuthoringMeshProjection = {
  schema_version: 'AuthoringMesh@1'
  mesh_id: string
  mesh_sha256: string
  lineage: {
    project_id: string
    candidate_id: string
    artifact_id: string
    artifact_readback_sha256: string
    program_sha256: string
    operator_catalog_sha256: string
    readback_config_sha256: string
    authoring_node_id: string
    part_id: string
    lineage_status: 'candidate-program-artifact-readback-bound@1'
    lineage_sha256: string
  }
  counts: AuthoringMeshCounts
  topology: AuthoringMeshTopology
  original_identity: {
    identity_kind: 'runtime-derived-original-authoring@1'
    topology_sha256: string
    element_id_policy: 'stable-within-authoring-mesh-lineage@1'
    source_lineage_sha256: string
  }
  evaluated_identity: {
    identity_kind: 'runtime-derived-evaluated-artifact-readback@1'
    artifact_id: string
    artifact_readback_sha256: string
    element_id_policy: 'artifact-local-no-authoring-bijection@1'
    correspondence_policy: 'non-bijective-derived-only@1'
    source_lineage_sha256: string
  }
  cross_version_stable: false
  runtime_write_performed: false
  persistent_user_data_touched: false
  quality_status: 'structural_only'
  authoring_mesh_policy_sha256: string
  max_response_bytes: 1048576
  canonical_sha256: string
}

export type AuthoringMeshReadback = {
  status: 'Ready' | 'Unavailable'
  code: string | null
  binding: AuthoringMeshBinding | null
  mesh: AuthoringMeshProjection | null
}

const AUTHORING_MESH_SCHEMA = 'AuthoringMesh@1'
const AUTHORING_MESH_POLICY_SHA256 = 'aa72cadabba90ddb43dd0014cfa434ab9b13f4e072b09258072f37334c72e709'
const MAX_RESPONSE_BYTES = 1024 * 1024

function isSha256(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{64}$/.test(value)
}

function isIdentifier(value: unknown): value is string {
  return typeof value === 'string' && /^[A-Za-z0-9_.-]{1,128}$/.test(value)
}

function isFiniteCount(value: unknown, maximum: number): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= maximum
}

function isSameBinding(value: unknown, binding: AuthoringMeshBinding): boolean {
  if (!value || typeof value !== 'object') return false
  const record = value as Record<string, unknown>
  return record.project_id === binding.projectId
    && record.candidate_id === binding.candidateId
    && record.artifact_id === binding.artifactId
    && record.artifact_readback_sha256 === binding.artifactReadbackSha256
    && record.program_sha256 === binding.programSha256
    && record.operator_catalog_sha256 === binding.operatorCatalogSha256
    && record.readback_config_sha256 === binding.readbackConfigSha256
    && record.authoring_node_id === binding.authoringNodeId
    && record.part_id === binding.partId
}

function isCounts(value: unknown): value is AuthoringMeshCounts {
  if (!value || typeof value !== 'object') return false
  const counts = value as Record<string, unknown>
  return isFiniteCount(counts.vertex_count, 8192)
    && counts.vertex_count >= 1
    && isFiniteCount(counts.edge_count, 16384)
    && counts.edge_count >= 1
    && isFiniteCount(counts.half_edge_count, 32768)
    && counts.half_edge_count >= 1
    && isFiniteCount(counts.corner_count, 32768)
    && counts.corner_count >= 1
    && isFiniteCount(counts.face_count, 8192)
    && counts.face_count >= 1
    && isFiniteCount(counts.loop_count, 32768)
    && counts.loop_count >= 1
    && isFiniteCount(counts.ring_count, 8192)
    && isFiniteCount(counts.boundary_edge_count, 16384)
    && isFiniteCount(counts.boundary_half_edge_count, 32768)
    && isFiniteCount(counts.hard_edge_count, 16384)
    && isFiniteCount(counts.crease_edge_count, 16384)
    && isFiniteCount(counts.uv_seam_count, 16384)
}

function isTopology(value: unknown, counts: AuthoringMeshCounts): value is AuthoringMeshTopology {
  if (!value || typeof value !== 'object') return false
  const topology = value as Record<string, unknown>
  return isFiniteCount(topology.boundary_edge_count, 16384)
    && isFiniteCount(topology.boundary_half_edge_count, 32768)
    && topology.boundary_edge_count === counts.boundary_edge_count
    && topology.boundary_half_edge_count === counts.boundary_half_edge_count
    && topology.non_manifold_edge_count === 0
    && topology.orientation_conflict_count === 0
    && (topology.status === 'closed_manifold' || topology.status === 'manifold_with_boundary')
    && topology.validation_status === 'passed'
    && topology.rejection_policy === 'fail-closed-on-non-manifold@1'
    && topology.face_cycle_policy === 'next-prev-complete-mutual@1'
    && topology.twin_policy === 'boundary-only-null-symmetric@1'
    && topology.boundary_policy === 'single-half-edge-per-boundary-edge@1'
}

function isProjection(value: unknown, binding: AuthoringMeshBinding): value is AuthoringMeshProjection {
  if (!value || typeof value !== 'object') return false
  let serialized: string
  try {
    serialized = JSON.stringify(value)
  } catch {
    return false
  }
  if (serialized.length > MAX_RESPONSE_BYTES) return false
  const mesh = value as Record<string, unknown>
  const counts = mesh.counts
  const topology = mesh.topology
  const lineage = mesh.lineage
  const original = mesh.original_identity
  const evaluated = mesh.evaluated_identity
  if (mesh.schema_version !== AUTHORING_MESH_SCHEMA
    || !isIdentifier(mesh.mesh_id)
    || !isSha256(mesh.mesh_sha256)
    || !isSameBinding(lineage, binding)
    || (lineage as Record<string, unknown> | undefined)?.lineage_status !== 'candidate-program-artifact-readback-bound@1'
    || !isSha256((lineage as Record<string, unknown> | undefined)?.lineage_sha256)
    || !isCounts(counts)
    || !isTopology(topology, counts)
    || mesh.mesh_identity_derivation !== 'runtime-derived-from-candidate-program-artifact-readback@1'
    || !isSha256(mesh.mesh_identity_sha256)
    || mesh.identity_policy !== 'runtime-derived-original-evaluated-non-bijective@1'
    || mesh.cross_version_stable !== false
    || !original || typeof original !== 'object'
    || (original as Record<string, unknown>).identity_kind !== 'runtime-derived-original-authoring@1'
    || !isSha256((original as Record<string, unknown>).topology_sha256)
    || (original as Record<string, unknown>).element_id_policy !== 'stable-within-authoring-mesh-lineage@1'
    || (original as Record<string, unknown>).position_space !== 'authoring-local@1'
    || (original as Record<string, unknown>).namespace !== 'original'
    || !isSha256((original as Record<string, unknown>).source_lineage_sha256)
    || !evaluated || typeof evaluated !== 'object'
    || (evaluated as Record<string, unknown>).identity_kind !== 'runtime-derived-evaluated-artifact-readback@1'
    || (evaluated as Record<string, unknown>).artifact_id !== binding.artifactId
    || (evaluated as Record<string, unknown>).artifact_readback_sha256 !== binding.artifactReadbackSha256
    || (evaluated as Record<string, unknown>).element_id_policy !== 'artifact-local-no-authoring-bijection@1'
    || (evaluated as Record<string, unknown>).position_space !== 'evaluated-local@1'
    || (evaluated as Record<string, unknown>).namespace !== 'evaluated'
    || (evaluated as Record<string, unknown>).correspondence_policy !== 'non-bijective-derived-only@1'
    || !isSha256((evaluated as Record<string, unknown>).source_lineage_sha256)
    || mesh.authoring_mesh_policy_sha256 !== AUTHORING_MESH_POLICY_SHA256
    || mesh.max_response_bytes !== MAX_RESPONSE_BYTES
    || mesh.runtime_write_performed !== false
    || mesh.persistent_user_data_touched !== false
    || mesh.quality_status !== 'structural_only'
    || !isSha256(mesh.canonical_sha256)) return false

  return true
}

export function unavailableAuthoringMesh(
  binding: AuthoringMeshBinding | null = null,
  code = 'AUTHORING_MESH_UNAVAILABLE',
): AuthoringMeshReadback {
  return { status: 'Unavailable', code, binding, mesh: null }
}

export function normalizeAuthoringMesh(
  payload: unknown,
  binding: AuthoringMeshBinding,
): AuthoringMeshReadback {
  if (payload && typeof payload === 'object') {
    const envelope = payload as Record<string, unknown>
    if (envelope.status === 'Unavailable') {
      return unavailableAuthoringMesh(
        binding,
        typeof envelope.code === 'string' ? envelope.code : 'AUTHORING_MESH_UNAVAILABLE',
      )
    }
  }
  if (!isProjection(payload, binding)) {
    return unavailableAuthoringMesh(binding, 'AUTHORING_MESH_BINDING_MISMATCH')
  }
  return { status: 'Ready', code: null, binding, mesh: payload }
}

export function isCurrentAuthoringMeshResponse(
  active: boolean,
  requestId: number,
  currentRequestId: number,
  payload: AuthoringMeshReadback,
  binding: AuthoringMeshBinding,
): boolean {
  return active
    && requestId === currentRequestId
    && payload.status === 'Ready'
    && payload.mesh !== null
    && payload.binding?.projectId === binding.projectId
    && payload.binding.candidateId === binding.candidateId
    && payload.binding.artifactId === binding.artifactId
    && payload.binding.partId === binding.partId
}
