/**
 * Fail-closed normalization for the Runtime-owned, candidate-bound provenance
 * graph. This module never derives quality, reads CAS, or performs writes.
 */

const GRAPH_SCHEMA = 'ViewerProvenanceGraph@1' as const
const MAX_NODES = 64
const MAX_EDGES = 128
const SHA256 = /^[0-9a-f]{64}$/
const IDENTIFIER = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/

// TODO(MCP010F): consume Modifier Apply history only after Runtime exposes a
// candidate-bound immutable projection with its own closed contract, sidecar
// object hash, canonical hash, source/derived bindings, and explicit read-only
// semantics. Until that interface exists, reject a future-looking field here
// instead of silently ignoring it or reconstructing history from Job/event
// payloads in the Viewer.
const UNSUPPORTED_MODIFIER_APPLY_PROJECTION_KEYS = [
  'modifier_apply_history',
  'modifier_apply_sidecar',
  'modifier_apply_sidecars',
  'modifier_apply_history_nodes',
] as const

type JsonRecord = Record<string, unknown>

export type ProvenanceGraphBinding = {
  projectId: string
  candidateId: string
  candidateStateSha256: string
  artifactId: string
}

export type ProvenanceNodeKind =
  | 'candidate' | 'geometry-evidence' | 'reference' | 'geometry-program'
  | 'operator-node' | 'artifact' | 'artifact-readback' | 'geometry-quality'
  | 'render-set' | 'render-pass' | 'comparison-report' | 'visual-quality'
  | 'mechanical-animation-clip'

export type ProvenanceNodeStatus =
  | 'verified' | 'structural_only' | 'quality_target_not_met'
  | 'quality_passed' | 'not_run' | 'blocked' | 'unavailable'

export type ProvenanceGraphNode = {
  nodeId: string
  kind: ProvenanceNodeKind
  label: string
  contractSchema: string | null
  objectSha256: string | null
  canonicalSha256: string | null
  status: ProvenanceNodeStatus
}

export type ProvenanceGraphEdge = {
  edgeId: string
  fromNodeId: string
  toNodeId: string
  relation: string
}

export type ViewerProvenanceGraph = {
  schemaVersion: typeof GRAPH_SCHEMA
  status: 'Ready' | 'Unavailable'
  readOnly: boolean
  runtimeWritePerformed: boolean
  persistentUserDataTouched: boolean
  binding: ProvenanceGraphBinding | null
  geometryCandidateEvidenceSha256: string | null
  complete: boolean
  truncated: boolean
  branchStatus: { geometry: 'verified'; visual: 'verified' | 'unavailable'; animation: 'verified' | 'unavailable' } | null
  omittedKinds: readonly string[]
  unknowns: readonly string[]
  nodes: readonly ProvenanceGraphNode[]
  edges: readonly ProvenanceGraphEdge[]
  qualityStatus: 'structural_only' | 'unavailable'
  canonicalSha256: string | null
  code: string | null
}

const NODE_KINDS = new Set<ProvenanceNodeKind>([
  'candidate', 'geometry-evidence', 'reference', 'geometry-program', 'operator-node',
  'artifact', 'artifact-readback', 'geometry-quality', 'render-set', 'render-pass',
  'comparison-report', 'visual-quality', 'mechanical-animation-clip',
])
const NODE_STATUSES = new Set<ProvenanceNodeStatus>([
  'verified', 'structural_only', 'quality_target_not_met', 'quality_passed',
  'not_run', 'blocked', 'unavailable',
])
const RELATIONS = new Set([
  'binds', 'contains', 'feeds', 'materializes', 'readback', 'evaluates',
  'references', 'renders', 'contains-pass', 'compares', 'summarizes', 'animates',
])

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function sha(value: unknown): string | null {
  return typeof value === 'string' && SHA256.test(value) ? value : null
}

function identifier(value: unknown): string | null {
  return typeof value === 'string' && IDENTIFIER.test(value) ? value : null
}

function stringList(value: unknown, max: number): string[] | null {
  if (!Array.isArray(value) || value.length > max || !value.every((item) => typeof item === 'string' && item.length > 0 && item.length <= 128)) return null
  const list = value as string[]
  return new Set(list).size === list.length ? list : null
}

function hasUnsupportedModifierApplyProjection(payload: JsonRecord): boolean {
  return Object.keys(payload).some((key) => {
    const normalizedKey = key.toLowerCase().replaceAll('-', '_')
    return UNSUPPORTED_MODIFIER_APPLY_PROJECTION_KEYS.includes(key as typeof UNSUPPORTED_MODIFIER_APPLY_PROJECTION_KEYS[number])
      || (normalizedKey.includes('modifier') && normalizedKey.includes('apply'))
  })
}

export function unavailableProvenanceGraph(
  binding?: Partial<ProvenanceGraphBinding>,
  code = 'PROVENANCE_GRAPH_UNAVAILABLE',
): ViewerProvenanceGraph {
  return {
    schemaVersion: GRAPH_SCHEMA,
    status: 'Unavailable',
    readOnly: true,
    runtimeWritePerformed: false,
    persistentUserDataTouched: false,
    binding: binding?.projectId && binding.candidateId && binding.candidateStateSha256 && binding.artifactId
      ? binding as ProvenanceGraphBinding : null,
    geometryCandidateEvidenceSha256: null,
    complete: false,
    truncated: false,
    branchStatus: null,
    omittedKinds: [],
    unknowns: [],
    nodes: [],
    edges: [],
    qualityStatus: 'unavailable',
    canonicalSha256: null,
    code,
  }
}

function normalizeNode(value: unknown): ProvenanceGraphNode | null {
  if (!isRecord(value)) return null
  const nodeId = typeof value.node_id === 'string' && /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$/.test(value.node_id) ? value.node_id : null
  const kind = NODE_KINDS.has(value.kind as ProvenanceNodeKind) ? value.kind as ProvenanceNodeKind : null
  const label = typeof value.label === 'string' && value.label.length > 0 && value.label.length <= 128 ? value.label : null
  const contractSchema = value.contract_schema === null ? null : typeof value.contract_schema === 'string' ? value.contract_schema : undefined
  const objectSha256 = value.object_sha256 === null ? null : sha(value.object_sha256) ?? undefined
  const canonicalSha256 = value.canonical_sha256 === null ? null : sha(value.canonical_sha256) ?? undefined
  const status = NODE_STATUSES.has(value.status as ProvenanceNodeStatus) ? value.status as ProvenanceNodeStatus : null
  if (!nodeId || !kind || !label || contractSchema === undefined || objectSha256 === undefined || canonicalSha256 === undefined || !status) return null
  return { nodeId, kind, label, contractSchema, objectSha256, canonicalSha256, status }
}

function normalizeEdge(value: unknown): ProvenanceGraphEdge | null {
  if (!isRecord(value)) return null
  const edgeId = typeof value.edge_id === 'string' && /^edge:[0-9]{1,4}$/.test(value.edge_id) ? value.edge_id : null
  const fromNodeId = typeof value.from_node_id === 'string' ? value.from_node_id : null
  const toNodeId = typeof value.to_node_id === 'string' ? value.to_node_id : null
  const relation = typeof value.relation === 'string' && RELATIONS.has(value.relation) ? value.relation : null
  return edgeId && fromNodeId && toNodeId && relation ? { edgeId, fromNodeId, toNodeId, relation } : null
}

function isAcyclic(nodes: readonly ProvenanceGraphNode[], edges: readonly ProvenanceGraphEdge[]): boolean {
  const indegree = new Map(nodes.map((node) => [node.nodeId, 0]))
  const outgoing = new Map(nodes.map((node) => [node.nodeId, [] as string[]]))
  for (const edge of edges) {
    indegree.set(edge.toNodeId, (indegree.get(edge.toNodeId) ?? 0) + 1)
    outgoing.get(edge.fromNodeId)?.push(edge.toNodeId)
  }
  const queue = [...indegree].filter(([, degree]) => degree === 0).map(([id]) => id)
  let visited = 0
  while (queue.length > 0) {
    const id = queue.shift() as string
    visited += 1
    for (const next of outgoing.get(id) ?? []) {
      const degree = (indegree.get(next) ?? 0) - 1
      indegree.set(next, degree)
      if (degree === 0) queue.push(next)
    }
  }
  return visited === nodes.length
}

export function normalizeViewerProvenanceGraph(
  payload: unknown,
  binding: ProvenanceGraphBinding,
): ViewerProvenanceGraph {
  if (!isRecord(payload)) return unavailableProvenanceGraph(binding, 'PROVENANCE_GRAPH_INVALID')
  if (payload.status === 'Unavailable') return unavailableProvenanceGraph(binding, typeof payload.code === 'string' ? payload.code : 'PROVENANCE_GRAPH_UNAVAILABLE')
  if (hasUnsupportedModifierApplyProjection(payload)) return unavailableProvenanceGraph(binding, 'PROVENANCE_GRAPH_MODIFIER_APPLY_HISTORY_UNSUPPORTED')
  if (payload.schema_version !== GRAPH_SCHEMA || payload.status !== 'Ready'
    || payload.read_only !== true || payload.runtime_write_performed !== false
    || payload.persistent_user_data_touched !== false || payload.complete !== true
    || payload.truncated !== false || payload.project_id !== binding.projectId
    || payload.candidate_id !== binding.candidateId
    || payload.candidate_state_sha256 !== binding.candidateStateSha256
    || payload.artifact_id !== binding.artifactId
    || payload.max_nodes !== MAX_NODES || payload.max_edges !== MAX_EDGES
    || payload.quality_status !== 'structural_only'
    || !sha(payload.geometry_candidate_evidence_sha256) || !sha(payload.canonical_sha256)
    || !Array.isArray(payload.nodes) || payload.nodes.length < 7 || payload.nodes.length > MAX_NODES
    || !Array.isArray(payload.edges) || payload.edges.length < 6 || payload.edges.length > MAX_EDGES
    || payload.node_count !== payload.nodes.length || payload.edge_count !== payload.edges.length
    || !isRecord(payload.branch_status) || payload.branch_status.geometry !== 'verified'
    || (payload.branch_status.visual !== 'verified' && payload.branch_status.visual !== 'unavailable')
    || (payload.branch_status.animation !== 'verified' && payload.branch_status.animation !== 'unavailable')
  ) return unavailableProvenanceGraph(binding, 'PROVENANCE_GRAPH_BINDING_MISMATCH')
  const nodes = payload.nodes.map(normalizeNode)
  const edges = payload.edges.map(normalizeEdge)
  if (!nodes.every((node): node is ProvenanceGraphNode => node !== null)
    || !edges.every((edge): edge is ProvenanceGraphEdge => edge !== null)) return unavailableProvenanceGraph(binding, 'PROVENANCE_GRAPH_INVALID')
  const nodeIds = new Set(nodes.map((node) => node.nodeId))
  const edgeIds = new Set(edges.map((edge) => edge.edgeId))
  const edgeKeys = new Set(edges.map((edge) => `${edge.fromNodeId}\u0000${edge.toNodeId}\u0000${edge.relation}`))
  if (nodeIds.size !== nodes.length || edgeIds.size !== edges.length || edgeKeys.size !== edges.length
    || edges.some((edge) => !nodeIds.has(edge.fromNodeId) || !nodeIds.has(edge.toNodeId))
    || !isAcyclic(nodes, edges)) return unavailableProvenanceGraph(binding, 'PROVENANCE_GRAPH_TOPOLOGY_INVALID')
  const requiredRoots = ['candidate', 'geometry-evidence', 'geometry-program', 'artifact', 'artifact-readback', 'geometry-quality']
  if (requiredRoots.some((nodeId) => !nodeIds.has(nodeId))) return unavailableProvenanceGraph(binding, 'PROVENANCE_GRAPH_ROOT_MISSING')
  const omittedKinds = stringList(payload.omitted_kinds, 8)
  const unknowns = stringList(payload.unknowns, 16)
  if (!omittedKinds || !unknowns) return unavailableProvenanceGraph(binding, 'PROVENANCE_GRAPH_INVALID')
  return {
    schemaVersion: GRAPH_SCHEMA,
    status: 'Ready',
    readOnly: true,
    runtimeWritePerformed: false,
    persistentUserDataTouched: false,
    binding,
    geometryCandidateEvidenceSha256: payload.geometry_candidate_evidence_sha256 as string,
    complete: true,
    truncated: false,
    branchStatus: payload.branch_status as ViewerProvenanceGraph['branchStatus'],
    omittedKinds,
    unknowns,
    nodes,
    edges,
    qualityStatus: 'structural_only',
    canonicalSha256: payload.canonical_sha256 as string,
    code: null,
  }
}

export function isCurrentProvenanceGraphResponse(
  active: boolean,
  currentRequestId: number,
  responseRequestId: number,
): boolean {
  return active && currentRequestId === responseRequestId
}

type Deferred<T> = {
  promise: Promise<T>
  resolve: (value: T) => void
  reject: (reason: unknown) => void
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

/** Executable race fixture for the same request-generation guard used by the Viewer effect. */
export async function provenanceGraphDeferredResponseSelfCheck(): Promise<{ passed: boolean; checks: string[] }> {
  let currentRequestId = 0
  let state = 'initial'
  let loading = false

  const launch = (pending: Promise<string>): number => {
    const requestId = ++currentRequestId
    loading = true
    void pending.then((value) => {
      if (!isCurrentProvenanceGraphResponse(true, currentRequestId, requestId)) return
      state = value
    }).catch(() => {
      if (!isCurrentProvenanceGraphResponse(true, currentRequestId, requestId)) return
      state = 'error'
    }).finally(() => {
      if (isCurrentProvenanceGraphResponse(true, currentRequestId, requestId)) loading = false
    })
    return requestId
  }
  const flushPromiseChain = async (): Promise<void> => {
    await Promise.resolve()
    await Promise.resolve()
    await Promise.resolve()
  }

  const staleSuccess = deferred<string>()
  const latestSuccess = deferred<string>()
  launch(staleSuccess.promise)
  launch(latestSuccess.promise)
  staleSuccess.resolve('stale-success')
  await staleSuccess.promise
  await flushPromiseChain()
  const staleSuccessRejected = state === 'initial'
  const staleFinallyRejected = loading
  latestSuccess.resolve('latest-success')
  await latestSuccess.promise
  await flushPromiseChain()
  const latestSuccessWins = state === 'latest-success' && loading === false

  const staleError = deferred<string>()
  const nextLatest = deferred<string>()
  launch(staleError.promise)
  launch(nextLatest.promise)
  staleError.reject(new Error('stale-error'))
  await staleError.promise.catch(() => undefined)
  await flushPromiseChain()
  const staleErrorRejected = state === 'latest-success' && loading
  nextLatest.resolve('next-latest')
  await nextLatest.promise
  await flushPromiseChain()
  const nextLatestWins = state === 'next-latest' && loading === false

  const checks = [
    staleSuccessRejected ? 'stale-success-rejected' : 'stale-success-overwrote',
    staleFinallyRejected ? 'stale-finally-rejected' : 'stale-finally-cleared-loading',
    latestSuccessWins ? 'latest-success-wins' : 'latest-success-failed',
    staleErrorRejected ? 'stale-error-rejected' : 'stale-error-overwrote',
    nextLatestWins ? 'next-latest-wins' : 'next-latest-failed',
  ]
  return {
    passed: staleSuccessRejected && staleFinallyRejected && latestSuccessWins && staleErrorRejected && nextLatestWins,
    checks,
  }
}

function hash(fill: string): string { return fill.repeat(64) }

export function provenanceGraphNormalizerSelfCheck(): { passed: boolean; checks: string[] } {
  const binding: ProvenanceGraphBinding = { projectId: 'project-a', candidateId: 'candidate-a', candidateStateSha256: hash('a'), artifactId: hash('b') }
  const nodes = [
    ['candidate', 'candidate'], ['geometry-evidence', 'geometry-evidence'], ['geometry-program', 'geometry-program'],
    ['operator:1', 'operator-node'], ['artifact', 'artifact'], ['artifact-readback', 'artifact-readback'], ['geometry-quality', 'geometry-quality'],
  ].map(([node_id, kind]) => ({ node_id, kind, label: node_id, contract_schema: null, object_sha256: null, canonical_sha256: hash('c'), status: kind === 'operator-node' ? 'structural_only' : 'verified' }))
  const pairs = [
    ['candidate', 'geometry-evidence', 'binds'], ['geometry-evidence', 'geometry-program', 'binds'],
    ['geometry-program', 'operator:1', 'contains'], ['geometry-program', 'artifact', 'materializes'],
    ['artifact', 'artifact-readback', 'readback'], ['artifact-readback', 'geometry-quality', 'evaluates'],
  ]
  const edges = pairs.map(([from_node_id, to_node_id, relation], index) => ({ edge_id: `edge:${index + 1}`, from_node_id, to_node_id, relation }))
  const raw = { schema_version: GRAPH_SCHEMA, status: 'Ready', read_only: true, runtime_write_performed: false, persistent_user_data_touched: false, project_id: binding.projectId, candidate_id: binding.candidateId, candidate_state_sha256: binding.candidateStateSha256, artifact_id: binding.artifactId, geometry_candidate_evidence_sha256: hash('d'), max_nodes: MAX_NODES, max_edges: MAX_EDGES, complete: true, truncated: false, branch_status: { geometry: 'verified', visual: 'unavailable', animation: 'unavailable' }, omitted_kinds: ['modifier-apply-history'], unknowns: ['visual-evidence-unavailable'], node_count: nodes.length, edge_count: edges.length, nodes, edges, quality_status: 'structural_only', limitations: ['read-only'], canonical_sha256: hash('e') }
  const ready = normalizeViewerProvenanceGraph(raw, binding)
  const crossCandidate = normalizeViewerProvenanceGraph(raw, { ...binding, candidateId: 'candidate-b' })
  const staleState = normalizeViewerProvenanceGraph(raw, { ...binding, candidateStateSha256: hash('f') })
  const wrongArtifact = normalizeViewerProvenanceGraph(raw, { ...binding, artifactId: hash('f') })
  const dangling = normalizeViewerProvenanceGraph({ ...raw, edges: [...edges.slice(0, -1), { ...edges.at(-1), to_node_id: 'missing' }] }, binding)
  const duplicate = normalizeViewerProvenanceGraph({ ...raw, edges: [...edges.slice(0, -1), { ...edges[0], edge_id: `edge:${edges.length}` }] }, binding)
  const cycleEdges = [...edges, { edge_id: `edge:${edges.length + 1}`, from_node_id: 'geometry-quality', to_node_id: 'candidate', relation: 'summarizes' }]
  const cycle = normalizeViewerProvenanceGraph({ ...raw, edge_count: cycleEdges.length, edges: cycleEdges }, binding)
  const modifierApplyHistory = normalizeViewerProvenanceGraph({
    ...raw,
    modifier_apply_history: [{
      source_candidate_id: binding.candidateId,
      sidecar_object_sha256: hash('f'),
    }],
  }, binding)
  const checks = [
    ready.status === 'Ready' ? 'positive-ready' : 'positive-failed',
    crossCandidate.status === 'Unavailable' ? 'cross-candidate-fail-closed' : 'cross-candidate-accepted',
    staleState.status === 'Unavailable' ? 'stale-state-fail-closed' : 'stale-state-accepted',
    wrongArtifact.status === 'Unavailable' ? 'artifact-mismatch-fail-closed' : 'artifact-mismatch-accepted',
    dangling.status === 'Unavailable' ? 'dangling-edge-fail-closed' : 'dangling-edge-accepted',
    duplicate.status === 'Unavailable' ? 'duplicate-edge-fail-closed' : 'duplicate-edge-accepted',
    cycle.status === 'Unavailable' ? 'cycle-fail-closed' : 'cycle-accepted',
    ready.omittedKinds.includes('modifier-apply-history') ? 'modifier-history-omission-explicit' : 'modifier-history-omission-hidden',
    modifierApplyHistory.status === 'Unavailable' && modifierApplyHistory.code === 'PROVENANCE_GRAPH_MODIFIER_APPLY_HISTORY_UNSUPPORTED'
      ? 'modifier-history-unsupported-fail-closed' : 'modifier-history-unsupported-accepted',
  ]
  return {
    passed: checks.every((item) => item === 'positive-ready' || item === 'modifier-history-omission-explicit' || item.endsWith('fail-closed')),
    checks,
  }
}
