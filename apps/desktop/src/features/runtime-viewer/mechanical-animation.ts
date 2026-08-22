/**
 * Read-only normalization for the bounded MechanicalAnimationClip Viewer
 * projection.  This module deliberately keeps Runtime snake_case contracts
 * separate from the ephemeral Viewer read model.  It normalizes frames already
 * evaluated by Runtime, but never evaluates a pose locally or accepts a write
 * operation.
 */

const INVENTORY_SCHEMA = 'ViewerMechanicalAnimationInventory@1' as const
const LINK_SCHEMA = 'MechanicalAnimationClipLink@1' as const
const CLIP_SCHEMA = 'MechanicalAnimationClip@1' as const
const REST_FRAME_SCHEMA = 'MechanicalRestFrame@1' as const
const POSE_ACTION_SCHEMA = 'MechanicalPoseAction@1' as const
const SAMPLING_POLICY_SCHEMA = 'MechanicalAnimationSamplingPolicy@1' as const
const FRAME_PREVIEW_SCHEMA = 'MechanicalAnimationClipPreview@1' as const
const POSE_GEOMETRY_PREVIEW_SCHEMA = 'MechanicalPoseGeometryPreview@1' as const
const MAX_CLIPS = 16
const MAX_LINKS = 64
const MAX_SOURCE_NODES = 16
const MAX_CHANNELS = 64
const MAX_KEYS = 32
const MAX_TICKS = 16
const MAX_TICK = 1_000_000
const SHA256_PATTERN = /^[0-9a-f]{64}$/
const IDENTIFIER_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/

type JsonRecord = Record<string, unknown>

export type MechanicalAnimationBinding = {
  projectId: string
  candidateId: string
  artifactId: string
}

export type MechanicalAnimationClipSummary = {
  clipId: string
  clipObjectSha256: string
  clipSha256: string
  restFrameSha256: string
  poseActionSha256: string
  sourceReplayWorkerCohortSha256: string
  materializationStatus: 'runtime-owned-immutable-cas-clip'
  createdAt: string | null
}

export type ViewerMechanicalAnimationInventory = {
  schemaVersion: typeof INVENTORY_SCHEMA
  status: 'Ready' | 'Unavailable'
  readOnly: boolean
  runtimeWritePerformed: boolean
  persistentUserDataTouched: boolean
  projectId: string | null
  candidateId: string | null
  artifactId: string | null
  clipCount: number
  maxClips: typeof MAX_CLIPS
  clips: readonly MechanicalAnimationClipSummary[]
  qualityStatus: 'structural_only' | 'unavailable'
  limitations: readonly string[]
  canonicalSha256: string | null
  code: string | null
}

export type MechanicalAnimationJointType = 'fixed' | 'revolute' | 'prismatic'
export type MechanicalAnimationValueUnit = 'none' | 'radian' | 'meter'

export type MechanicalRestFrameLink = {
  linkId: string
  partId: string
  sourceNodeIds: readonly string[]
  jointType: MechanicalAnimationJointType
  restTranslationM: readonly [number, number, number]
  restRotationQuatXyzw: readonly [number, number, number, number]
  axisLocal: readonly [number, number, number] | null
  limitMin: number | null
  limitMax: number | null
  valueUnit: MechanicalAnimationValueUnit
}

export type MechanicalRestFrame = {
  schemaVersion: typeof REST_FRAME_SCHEMA
  restFrameId: string
  projectId: string
  artifactId: string
  candidateId: string
  programSha256: string
  coordinateSystem: 'forgecad-rh-y-up-m@1'
  transformConvention: 'column-vector-trs-quaternion@1'
  rootLinkId: string
  links: readonly MechanicalRestFrameLink[]
  parentMap: readonly { childLinkId: string; parentLinkId: string }[]
  evaluationOrder: readonly string[]
  parentMapSha256: string
  canonicalSha256: string
}

export type MechanicalPoseActionKey = {
  timeTicks: number
  value: number
}

export type MechanicalPoseActionChannel = {
  linkId: string
  valueUnit: Exclude<MechanicalAnimationValueUnit, 'none'>
  keys: readonly MechanicalPoseActionKey[]
}

export type MechanicalPoseAction = {
  schemaVersion: typeof POSE_ACTION_SCHEMA
  actionId: string
  projectId: string
  candidateId: string
  restFrameSha256: string
  programSha256: string
  timebaseHz: 1000
  durationTicks: number
  interpolation: 'linear@1'
  extrapolation: 'clamp@1'
  unkeyedPolicy: 'rest@1'
  channels: readonly MechanicalPoseActionChannel[]
  canonicalSha256: string
}

export type MechanicalAnimationSamplingPolicy = {
  schemaVersion: typeof SAMPLING_POLICY_SCHEMA
  timebaseHz: 1000
  interpolation: 'scalar-linear-integer-ticks-clamped'
  unkeyed: 'rest'
  sampleTimeTicks: readonly number[]
  maxSamples: typeof MAX_TICKS
  framePreviewBatchSize: 1
}

export type MechanicalAnimationSourceReplay = {
  workerBuildCohortSha256: string
  firstArtifactSha256: string
  repeatArtifactSha256: string
  byteExactWithCandidateArtifact: true
  strictReadbackPassed: true
}

export type MechanicalAnimationClip = {
  schemaVersion: typeof CLIP_SCHEMA
  clipId: string
  projectId: string
  candidateId: string
  artifactId: string
  artifactReadbackSha256: string
  geometryCandidateEvidenceSha256: string
  programSha256: string
  operatorCatalogSha256: string
  readbackConfigSha256: string
  requestSha256: string
  restFrame: MechanicalRestFrame
  restFrameSha256: string
  poseAction: MechanicalPoseAction
  poseActionSha256: string
  samplingPolicy: MechanicalAnimationSamplingPolicy
  samplingPolicySha256: string
  sourceReplay: MechanicalAnimationSourceReplay
  materializationStatus: 'runtime-owned-immutable-cas-clip'
  qualityStatus: 'structural_only'
  limitations: readonly string[]
  canonicalSha256: string
}

export type MechanicalAnimationClipLink = {
  schemaVersion: typeof LINK_SCHEMA
  projectId: string
  candidateId: string
  artifactId: string
  artifactReadbackSha256: string
  geometryCandidateEvidenceSha256: string
  programSha256: string
  operatorCatalogSha256: string
  readbackConfigSha256: string
  clipId: string
  requestSha256: string
  clipObjectSha256: string
  clipSha256: string
  restFrameSha256: string
  poseActionSha256: string
  sourceReplayWorkerCohortSha256: string
  materializationStatus: 'runtime-owned-immutable-cas-clip'
  clip: MechanicalAnimationClip
  canonicalSha256: string
}

export type MechanicalAnimationClipReadback = {
  status: 'Ready' | 'Unavailable'
  link: MechanicalAnimationClipLink | null
  code: string | null
}

export type MechanicalAnimationPartDelta = {
  partId: string
  translationM: readonly [number, number, number]
  rotationQuatXyzw: readonly [number, number, number, number]
  scale: readonly [1, 1, 1]
}

export type MechanicalAnimationFramePreview = {
  schemaVersion: typeof FRAME_PREVIEW_SCHEMA
  projectId: string
  candidateId: string
  artifactId: string
  clipId: string
  clipSha256: string
  clipObjectSha256: string
  restFrameSha256: string
  poseActionSha256: string
  sampleTimeTicks: number
  frameSha256: string
  evaluatedPoseSha256: string
  posedProgramSha256: string
  transientArtifactSha256: string
  workerBuildCohortSha256: string
  partDeltas: readonly MechanicalAnimationPartDelta[]
  runtimeWritePerformed: false
  persistentUserDataTouched: false
  qualityStatus: 'structural_only'
  canonicalSha256: string
}

export type MechanicalAnimationFrameReadback = {
  status: 'Ready' | 'Unavailable'
  frame: MechanicalAnimationFramePreview | null
  code: string | null
}

export type MechanicalAnimationPartOwnerDescriptor = {
  nodeId: string
  partId: string | null
  ancestorPartId: string | null
  directRootChild: boolean
  identityTransform: boolean
  isBone: boolean
  isSkinnedMesh: boolean
}

export type MechanicalAnimationPartOwnerValidation = {
  status: 'Ready' | 'Unavailable'
  ownerNodeIdsByPartId: ReadonlyMap<string, string>
  code: string | null
}

export function validateMechanicalAnimationPartOwners(
  descriptors: readonly MechanicalAnimationPartOwnerDescriptor[],
  expectedPartIds: readonly string[],
  embeddedAnimationCount: number,
): MechanicalAnimationPartOwnerValidation {
  if (embeddedAnimationCount !== 0) return { status: 'Unavailable', ownerNodeIdsByPartId: new Map(), code: 'GLB_EMBEDDED_ANIMATION_UNSUPPORTED' }
  if (expectedPartIds.length === 0 || expectedPartIds.length > MAX_LINKS || new Set(expectedPartIds).size !== expectedPartIds.length) return { status: 'Unavailable', ownerNodeIdsByPartId: new Map(), code: 'GLB_PART_OWNER_EXPECTATION_INVALID' }
  if (descriptors.some((descriptor) => descriptor.isBone || descriptor.isSkinnedMesh)) return { status: 'Unavailable', ownerNodeIdsByPartId: new Map(), code: 'GLB_SKIN_OR_BONE_UNSUPPORTED' }
  const expected = new Set(expectedPartIds)
  const owners = new Map<string, string>()
  for (const descriptor of descriptors) {
    if (!descriptor.partId) continue
    if (!expected.has(descriptor.partId) || descriptor.ancestorPartId || !descriptor.directRootChild || !descriptor.identityTransform || owners.has(descriptor.partId)) return { status: 'Unavailable', ownerNodeIdsByPartId: new Map(), code: 'GLB_PART_OWNER_MAPPING_INVALID' }
    owners.set(descriptor.partId, descriptor.nodeId)
  }
  if (owners.size !== expected.size || expectedPartIds.some((partId) => !owners.has(partId))) return { status: 'Unavailable', ownerNodeIdsByPartId: new Map(), code: 'GLB_PART_OWNER_MAPPING_INCOMPLETE' }
  return { status: 'Ready', ownerNodeIdsByPartId: owners, code: null }
}

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function stringValue(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null
}

function identifierValue(value: unknown): string | null {
  const text = stringValue(value)
  return text && IDENTIFIER_PATTERN.test(text) ? text : null
}

function shaValue(value: unknown): string | null {
  const text = stringValue(value)
  return text && SHA256_PATTERN.test(text) ? text : null
}

function finiteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null
}

function boundedNumber(value: unknown, minimum: number, maximum: number): number | null {
  const number = finiteNumber(value)
  return number !== null && number >= minimum && number <= maximum ? number : null
}

function stringArray(value: unknown, maximum: number): string[] | null {
  if (!Array.isArray(value) || value.length === 0 || value.length > maximum) return null
  const result = value.map((item) => identifierValue(item))
  return result.every((item): item is string => item !== null) ? result : null
}

function numberTuple<T extends 3 | 4>(value: unknown, length: T, minimum: number, maximum: number): T extends 3 ? [number, number, number] | null : [number, number, number, number] | null {
  if (!Array.isArray(value) || value.length !== length) return null as T extends 3 ? [number, number, number] | null : [number, number, number, number] | null
  const result = value.map((item) => boundedNumber(item, minimum, maximum))
  if (!result.every((item): item is number => item !== null)) return null as T extends 3 ? [number, number, number] | null : [number, number, number, number] | null
  return result as T extends 3 ? [number, number, number] : [number, number, number, number]
}

function exactBinding(record: JsonRecord, binding: MechanicalAnimationBinding): boolean {
  return record.project_id === binding.projectId
    && record.candidate_id === binding.candidateId
    && record.artifact_id === binding.artifactId
}

function unavailableCode(payload: unknown, fallback: string): string {
  return isRecord(payload) && typeof payload.code === 'string' && payload.code.length > 0
    ? payload.code
    : fallback
}

export function unavailableMechanicalAnimationInventory(
  binding?: Partial<MechanicalAnimationBinding>,
  code = 'MECHANICAL_ANIMATION_INVENTORY_UNAVAILABLE',
): ViewerMechanicalAnimationInventory {
  return {
    schemaVersion: INVENTORY_SCHEMA,
    status: 'Unavailable',
    readOnly: true,
    runtimeWritePerformed: false,
    persistentUserDataTouched: false,
    projectId: binding?.projectId ?? null,
    candidateId: binding?.candidateId ?? null,
    artifactId: binding?.artifactId ?? null,
    clipCount: 0,
    maxClips: MAX_CLIPS,
    clips: [],
    qualityStatus: 'unavailable',
    limitations: ['read-only-projection-unavailable'],
    canonicalSha256: null,
    code,
  }
}

function normalizeClipSummary(value: unknown): MechanicalAnimationClipSummary | null {
  if (!isRecord(value)) return null
  const clipId = identifierValue(value.clip_id)
  const clipObjectSha256 = shaValue(value.clip_object_sha256)
  const clipSha256 = shaValue(value.clip_sha256)
  const restFrameSha256 = shaValue(value.rest_frame_sha256)
  const poseActionSha256 = shaValue(value.pose_action_sha256)
  const sourceReplayWorkerCohortSha256 = shaValue(value.source_replay_worker_cohort_sha256)
  if (!clipId || !clipObjectSha256 || !clipSha256 || !restFrameSha256 || !poseActionSha256 || !sourceReplayWorkerCohortSha256) return null
  if (value.materialization_status !== 'runtime-owned-immutable-cas-clip') return null
  const createdAt = value.created_at === undefined || value.created_at === null ? null : stringValue(value.created_at)
  if (value.created_at !== undefined && value.created_at !== null && createdAt === null) return null
  return {
    clipId,
    clipObjectSha256,
    clipSha256,
    restFrameSha256,
    poseActionSha256,
    sourceReplayWorkerCohortSha256,
    materializationStatus: 'runtime-owned-immutable-cas-clip',
    createdAt,
  }
}

export function normalizeViewerMechanicalAnimationInventory(
  payload: unknown,
  binding: MechanicalAnimationBinding,
): ViewerMechanicalAnimationInventory {
  if (!isRecord(payload)) return unavailableMechanicalAnimationInventory(binding, 'MECHANICAL_ANIMATION_INVENTORY_INVALID')
  if (payload.status === 'Unavailable') return unavailableMechanicalAnimationInventory(binding, unavailableCode(payload, 'MECHANICAL_ANIMATION_INVENTORY_UNAVAILABLE'))
  if (payload.schema_version !== INVENTORY_SCHEMA
    || payload.status !== 'Ready'
    || payload.read_only !== true
    || payload.runtime_write_performed !== false
    || payload.persistent_user_data_touched !== false
    || !exactBinding(payload, binding)
    || payload.quality_status !== 'structural_only'
    || !Array.isArray(payload.clips)
    || payload.clips.length > MAX_CLIPS
    || payload.max_clips !== MAX_CLIPS
    || !Number.isInteger(payload.clip_count)
    || payload.clip_count !== payload.clips.length
  ) return unavailableMechanicalAnimationInventory(binding, 'MECHANICAL_ANIMATION_INVENTORY_BINDING_MISMATCH')
  const clips = payload.clips.map(normalizeClipSummary)
  if (!clips.every((item): item is MechanicalAnimationClipSummary => item !== null)) return unavailableMechanicalAnimationInventory(binding, 'MECHANICAL_ANIMATION_INVENTORY_INVALID')
  const clipIds = new Set(clips.map((item) => item.clipId))
  if (clipIds.size !== clips.length) return unavailableMechanicalAnimationInventory(binding, 'MECHANICAL_ANIMATION_INVENTORY_DUPLICATE_CLIP')
  const limitations = Array.isArray(payload.limitations) && payload.limitations.every((item) => typeof item === 'string')
    ? payload.limitations as string[]
    : null
  const canonicalSha256 = shaValue(payload.canonical_sha256)
  if (!limitations || !canonicalSha256) return unavailableMechanicalAnimationInventory(binding, 'MECHANICAL_ANIMATION_INVENTORY_INVALID')
  return {
    schemaVersion: INVENTORY_SCHEMA,
    status: 'Ready',
    readOnly: true,
    runtimeWritePerformed: false,
    persistentUserDataTouched: false,
    projectId: binding.projectId,
    candidateId: binding.candidateId,
    artifactId: binding.artifactId,
    clipCount: clips.length,
    maxClips: MAX_CLIPS,
    clips,
    qualityStatus: 'structural_only',
    limitations,
    canonicalSha256,
    code: null,
  }
}

function normalizeRestFrameLink(value: unknown): MechanicalRestFrameLink | null {
  if (!isRecord(value)) return null
  const linkId = identifierValue(value.link_id)
  const partId = identifierValue(value.part_id)
  const sourceNodeIds = stringArray(value.source_node_ids, MAX_SOURCE_NODES)
  const jointType = value.joint_type === 'fixed' || value.joint_type === 'revolute' || value.joint_type === 'prismatic' ? value.joint_type : null
  const restTranslationM = numberTuple(value.rest_translation_m, 3, -10, 10)
  const restRotationQuatXyzw = numberTuple(value.rest_rotation_quat_xyzw, 4, -1, 1)
  const axisLocal = !Object.prototype.hasOwnProperty.call(value, 'axis_local')
    ? undefined
    : value.axis_local === null ? null : numberTuple(value.axis_local, 3, -1, 1)
  const limitMin = !Object.prototype.hasOwnProperty.call(value, 'limit_min')
    ? undefined
    : value.limit_min === null ? null : boundedNumber(value.limit_min, -Math.PI, Math.PI)
  const limitMax = !Object.prototype.hasOwnProperty.call(value, 'limit_max')
    ? undefined
    : value.limit_max === null ? null : boundedNumber(value.limit_max, -Math.PI, Math.PI)
  const valueUnit = value.value_unit === 'none' || value.value_unit === 'radian' || value.value_unit === 'meter' ? value.value_unit : null
  if (!linkId || !partId || !sourceNodeIds || !jointType || !restTranslationM || !restRotationQuatXyzw || axisLocal === undefined || limitMin === undefined || limitMax === undefined || !valueUnit) return null
  if (jointType === 'fixed' && valueUnit !== 'none') return null
  if (jointType !== 'fixed' && valueUnit === 'none') return null
  if (limitMin !== null && limitMax !== null && limitMin > limitMax) return null
  return { linkId, partId, sourceNodeIds, jointType, restTranslationM, restRotationQuatXyzw, axisLocal, limitMin, limitMax, valueUnit }
}

function normalizeRestFrame(value: unknown, binding: MechanicalAnimationBinding, programSha256: string, expectedSha256: string): MechanicalRestFrame | null {
  if (!isRecord(value)
    || value.schema_version !== REST_FRAME_SCHEMA
    || value.project_id !== binding.projectId
    || value.candidate_id !== binding.candidateId
    || value.artifact_id !== binding.artifactId
    || value.program_sha256 !== programSha256
    || value.coordinate_system !== 'forgecad-rh-y-up-m@1'
    || value.transform_convention !== 'column-vector-trs-quaternion@1'
  ) return null
  const restFrameId = identifierValue(value.rest_frame_id)
  const rootLinkId = identifierValue(value.root_link_id)
  const programHash = shaValue(value.program_sha256)
  const parentMapSha256 = shaValue(value.parent_map_sha256)
  const canonicalSha256 = shaValue(value.canonical_sha256)
  if (!restFrameId || !rootLinkId || !programHash || !parentMapSha256 || !canonicalSha256 || canonicalSha256 !== expectedSha256 || !Array.isArray(value.links) || value.links.length === 0 || value.links.length > MAX_LINKS || !Array.isArray(value.parent_map) || value.parent_map.length > MAX_LINKS - 1 || !Array.isArray(value.evaluation_order) || value.evaluation_order.length !== value.links.length) return null
  const links = value.links.map(normalizeRestFrameLink)
  if (!links.every((item): item is MechanicalRestFrameLink => item !== null)) return null
  const linkIds = new Set(links.map((item) => item.linkId))
  const partIds = new Set(links.map((item) => item.partId))
  if (linkIds.size !== links.length || partIds.size !== links.length || !linkIds.has(rootLinkId)) return null
  const evaluationOrder = value.evaluation_order.map((item) => identifierValue(item))
  if (!evaluationOrder.every((item): item is string => item !== null) || new Set(evaluationOrder).size !== evaluationOrder.length || evaluationOrder.some((item) => !linkIds.has(item))) return null
  const parentMap: Array<{ childLinkId: string; parentLinkId: string }> = []
  const children = new Set<string>()
  for (const item of value.parent_map) {
    if (!isRecord(item)) return null
    const childLinkId = identifierValue(item.child_link_id)
    const parentLinkId = identifierValue(item.parent_link_id)
    if (!childLinkId || !parentLinkId || childLinkId === parentLinkId || !linkIds.has(childLinkId) || !linkIds.has(parentLinkId) || children.has(childLinkId)) return null
    children.add(childLinkId)
    parentMap.push({ childLinkId, parentLinkId })
  }
  if (children.has(rootLinkId)) return null
  const parentByChild = new Map(parentMap.map((item) => [item.childLinkId, item.parentLinkId]))
  for (const linkId of linkIds) {
    const seen = new Set<string>()
    let current: string | undefined = linkId
    while (current && parentByChild.has(current)) {
      if (seen.has(current)) return null
      seen.add(current)
      current = parentByChild.get(current)
    }
    if (current !== rootLinkId) return null
  }
  return {
    schemaVersion: REST_FRAME_SCHEMA,
    restFrameId,
    projectId: binding.projectId,
    artifactId: binding.artifactId,
    candidateId: binding.candidateId,
    programSha256: programHash,
    coordinateSystem: 'forgecad-rh-y-up-m@1',
    transformConvention: 'column-vector-trs-quaternion@1',
    rootLinkId,
    links,
    parentMap,
    evaluationOrder,
    parentMapSha256,
    canonicalSha256,
  }
}

function normalizePoseAction(value: unknown, binding: MechanicalAnimationBinding, programSha256: string, restFrameSha256: string, expectedSha256: string, restLinks: ReadonlyMap<string, MechanicalRestFrameLink>): MechanicalPoseAction | null {
  if (!isRecord(value)
    || value.schema_version !== POSE_ACTION_SCHEMA
    || value.project_id !== binding.projectId
    || value.candidate_id !== binding.candidateId
    || value.program_sha256 !== programSha256
    || value.rest_frame_sha256 !== restFrameSha256
    || value.timebase_hz !== 1000
    || value.interpolation !== 'linear@1'
    || value.extrapolation !== 'clamp@1'
    || value.unkeyed_policy !== 'rest@1'
    || !Array.isArray(value.channels)
    || value.channels.length === 0
    || value.channels.length > MAX_CHANNELS
  ) return null
  const actionId = identifierValue(value.action_id)
  const restHash = shaValue(value.rest_frame_sha256)
  const programHash = shaValue(value.program_sha256)
  const canonicalSha256 = shaValue(value.canonical_sha256)
  const durationTicks = typeof value.duration_ticks === 'number' && Number.isInteger(value.duration_ticks) && value.duration_ticks >= 1 && value.duration_ticks <= MAX_TICK ? value.duration_ticks : null
  if (!actionId || !restHash || !programHash || !canonicalSha256 || canonicalSha256 !== expectedSha256 || durationTicks === null) return null
  const channels: MechanicalPoseActionChannel[] = []
  const channelIds = new Set<string>()
  for (const item of value.channels) {
    if (!isRecord(item)) return null
    const linkId = identifierValue(item.link_id)
    const valueUnit = item.value_unit === 'radian' || item.value_unit === 'meter' ? item.value_unit : null
    if (!linkId || !valueUnit || channelIds.has(linkId)) return null
    const link = restLinks.get(linkId)
    if (!link || link.valueUnit !== valueUnit || !Array.isArray(item.keys) || item.keys.length === 0 || item.keys.length > MAX_KEYS) return null
    const keys: MechanicalPoseActionKey[] = []
    let previousTick = -1
    for (const key of item.keys) {
      if (!isRecord(key)) return null
      const timeTicks = typeof key.time_ticks === 'number' && Number.isInteger(key.time_ticks) && key.time_ticks >= 0 && key.time_ticks <= durationTicks ? key.time_ticks : null
      const number = boundedNumber(key.value, -Math.PI, Math.PI)
      if (timeTicks === null || number === null || timeTicks <= previousTick) return null
      previousTick = timeTicks
      keys.push({ timeTicks, value: number })
    }
    channelIds.add(linkId)
    channels.push({ linkId, valueUnit, keys })
  }
  return { schemaVersion: POSE_ACTION_SCHEMA, actionId, projectId: binding.projectId, candidateId: binding.candidateId, restFrameSha256: restHash, programSha256: programHash, timebaseHz: 1000, durationTicks, interpolation: 'linear@1', extrapolation: 'clamp@1', unkeyedPolicy: 'rest@1', channels, canonicalSha256 }
}

function normalizeSamplingPolicy(value: unknown): MechanicalAnimationSamplingPolicy | null {
  if (!isRecord(value)
    || value.schema_version !== SAMPLING_POLICY_SCHEMA
    || value.timebase_hz !== 1000
    || value.interpolation !== 'scalar-linear-integer-ticks-clamped'
    || value.unkeyed !== 'rest'
    || value.max_samples !== MAX_TICKS
    || value.frame_preview_batch_size !== 1
    || !Array.isArray(value.sample_time_ticks)
    || value.sample_time_ticks.length === 0
    || value.sample_time_ticks.length > MAX_TICKS
  ) return null
  const ticks: number[] = []
  let previous = -1
  for (const item of value.sample_time_ticks) {
    if (typeof item !== 'number' || !Number.isInteger(item) || item < 0 || item > MAX_TICK || item <= previous) return null
    previous = item
    ticks.push(item)
  }
  return { schemaVersion: SAMPLING_POLICY_SCHEMA, timebaseHz: 1000, interpolation: 'scalar-linear-integer-ticks-clamped', unkeyed: 'rest', sampleTimeTicks: ticks, maxSamples: MAX_TICKS, framePreviewBatchSize: 1 }
}

function normalizeSourceReplay(value: unknown): MechanicalAnimationSourceReplay | null {
  if (!isRecord(value) || value.byte_exact_with_candidate_artifact !== true || value.strict_readback_passed !== true) return null
  const workerBuildCohortSha256 = shaValue(value.worker_build_cohort_sha256)
  const firstArtifactSha256 = shaValue(value.first_artifact_sha256)
  const repeatArtifactSha256 = shaValue(value.repeat_artifact_sha256)
  if (!workerBuildCohortSha256 || !firstArtifactSha256 || !repeatArtifactSha256) return null
  return { workerBuildCohortSha256, firstArtifactSha256, repeatArtifactSha256, byteExactWithCandidateArtifact: true, strictReadbackPassed: true }
}

function normalizeClip(value: unknown, binding: MechanicalAnimationBinding, expectedSummary?: MechanicalAnimationClipSummary): MechanicalAnimationClip | null {
  if (!isRecord(value)
    || value.schema_version !== CLIP_SCHEMA
    || !exactBinding(value, binding)
    || value.materialization_status !== 'runtime-owned-immutable-cas-clip'
    || value.quality_status !== 'structural_only'
  ) return null
  const clipId = identifierValue(value.clip_id)
  const artifactReadbackSha256 = shaValue(value.artifact_readback_sha256)
  const geometryCandidateEvidenceSha256 = shaValue(value.geometry_candidate_evidence_sha256)
  const programSha256 = shaValue(value.program_sha256)
  const operatorCatalogSha256 = shaValue(value.operator_catalog_sha256)
  const readbackConfigSha256 = shaValue(value.readback_config_sha256)
  const requestSha256 = shaValue(value.request_sha256)
  const restFrameSha256 = shaValue(value.rest_frame_sha256)
  const poseActionSha256 = shaValue(value.pose_action_sha256)
  const samplingPolicySha256 = shaValue(value.sampling_policy_sha256)
  const canonicalSha256 = shaValue(value.canonical_sha256)
  if (!clipId || !artifactReadbackSha256 || !geometryCandidateEvidenceSha256 || !programSha256 || !operatorCatalogSha256 || !readbackConfigSha256 || !requestSha256 || !restFrameSha256 || !poseActionSha256 || !samplingPolicySha256 || !canonicalSha256 || (expectedSummary && (expectedSummary.clipId !== clipId || expectedSummary.clipSha256 !== canonicalSha256))) return null
  const restFrame = normalizeRestFrame(value.rest_frame, binding, programSha256, restFrameSha256)
  if (!restFrame) return null
  const restLinks = new Map(restFrame.links.map((link) => [link.linkId, link]))
  const poseAction = normalizePoseAction(value.pose_action, binding, programSha256, restFrameSha256, poseActionSha256, restLinks)
  const samplingPolicy = normalizeSamplingPolicy(value.sampling_policy)
  const sourceReplay = normalizeSourceReplay(value.source_replay)
  if (!poseAction || !samplingPolicy || !sourceReplay || sourceReplay.workerBuildCohortSha256 !== (expectedSummary?.sourceReplayWorkerCohortSha256 ?? sourceReplay.workerBuildCohortSha256)) return null
  if (poseAction.canonicalSha256 !== poseActionSha256 || restFrame.canonicalSha256 !== restFrameSha256 || value.sampling_policy_sha256 !== samplingPolicySha256 || value.clip_id !== clipId) return null
  const limitations = Array.isArray(value.limitations) && value.limitations.every((item) => typeof item === 'string') ? value.limitations as string[] : null
  if (!limitations || !limitations.some((item) => /skinning|armature|ik|nla|fcurve|timeline/i.test(item))) return null
  if (sourceReplay.firstArtifactSha256 !== binding.artifactId || sourceReplay.repeatArtifactSha256 !== binding.artifactId) return null
  return { schemaVersion: CLIP_SCHEMA, clipId, projectId: binding.projectId, candidateId: binding.candidateId, artifactId: binding.artifactId, artifactReadbackSha256, geometryCandidateEvidenceSha256, programSha256, operatorCatalogSha256, readbackConfigSha256, requestSha256, restFrame, restFrameSha256, poseAction, poseActionSha256, samplingPolicy, samplingPolicySha256, sourceReplay, materializationStatus: 'runtime-owned-immutable-cas-clip', qualityStatus: 'structural_only', limitations, canonicalSha256 }
}

export function normalizeMechanicalAnimationClipLink(
  payload: unknown,
  binding: MechanicalAnimationBinding,
  expectedSummary?: MechanicalAnimationClipSummary,
): MechanicalAnimationClipReadback {
  if (!isRecord(payload)) return { status: 'Unavailable', link: null, code: 'MECHANICAL_ANIMATION_CLIP_INVALID' }
  if (payload.status === 'Unavailable') return { status: 'Unavailable', link: null, code: unavailableCode(payload, 'MECHANICAL_ANIMATION_CLIP_UNAVAILABLE') }
  if (payload.schema_version !== LINK_SCHEMA || !exactBinding(payload, binding) || payload.materialization_status !== 'runtime-owned-immutable-cas-clip') return { status: 'Unavailable', link: null, code: 'MECHANICAL_ANIMATION_CLIP_BINDING_MISMATCH' }
  const fields = ['artifact_readback_sha256', 'geometry_candidate_evidence_sha256', 'program_sha256', 'operator_catalog_sha256', 'readback_config_sha256', 'request_sha256', 'clip_object_sha256', 'clip_sha256', 'rest_frame_sha256', 'pose_action_sha256', 'source_replay_worker_cohort_sha256', 'canonical_sha256']
  if (fields.some((field) => !shaValue(payload[field]))) return { status: 'Unavailable', link: null, code: 'MECHANICAL_ANIMATION_CLIP_INVALID' }
  const normalizedClip = normalizeClip(payload.clip, binding, expectedSummary)
  if (!normalizedClip) return { status: 'Unavailable', link: null, code: 'MECHANICAL_ANIMATION_CLIP_INVALID' }
  const clipId = identifierValue(payload.clip_id)
  const clipObjectSha256 = shaValue(payload.clip_object_sha256)
  const clipSha256 = shaValue(payload.clip_sha256)
  const restFrameSha256 = shaValue(payload.rest_frame_sha256)
  const poseActionSha256 = shaValue(payload.pose_action_sha256)
  const artifactReadbackSha256 = shaValue(payload.artifact_readback_sha256)
  const geometryCandidateEvidenceSha256 = shaValue(payload.geometry_candidate_evidence_sha256)
  const programSha256 = shaValue(payload.program_sha256)
  const operatorCatalogSha256 = shaValue(payload.operator_catalog_sha256)
  const readbackConfigSha256 = shaValue(payload.readback_config_sha256)
  const requestSha256 = shaValue(payload.request_sha256)
  const sourceReplayWorkerCohortSha256 = shaValue(payload.source_replay_worker_cohort_sha256)
  const canonicalSha256 = shaValue(payload.canonical_sha256)
  if (!clipId || !clipObjectSha256 || !clipSha256 || !restFrameSha256 || !poseActionSha256 || !artifactReadbackSha256 || !geometryCandidateEvidenceSha256 || !programSha256 || !operatorCatalogSha256 || !readbackConfigSha256 || !requestSha256 || !sourceReplayWorkerCohortSha256 || !canonicalSha256
    || clipId !== normalizedClip.clipId
    || clipSha256 !== normalizedClip.canonicalSha256
    || restFrameSha256 !== normalizedClip.restFrameSha256
    || poseActionSha256 !== normalizedClip.poseActionSha256
    || artifactReadbackSha256 !== normalizedClip.artifactReadbackSha256
    || geometryCandidateEvidenceSha256 !== normalizedClip.geometryCandidateEvidenceSha256
    || programSha256 !== normalizedClip.programSha256
    || operatorCatalogSha256 !== normalizedClip.operatorCatalogSha256
    || readbackConfigSha256 !== normalizedClip.readbackConfigSha256
    || requestSha256 !== normalizedClip.requestSha256
    || sourceReplayWorkerCohortSha256 !== normalizedClip.sourceReplay.workerBuildCohortSha256
  ) return { status: 'Unavailable', link: null, code: 'MECHANICAL_ANIMATION_CLIP_BINDING_MISMATCH' }
  return {
    status: 'Ready',
    code: null,
    link: { schemaVersion: LINK_SCHEMA, projectId: binding.projectId, candidateId: binding.candidateId, artifactId: binding.artifactId, artifactReadbackSha256, geometryCandidateEvidenceSha256, programSha256, operatorCatalogSha256, readbackConfigSha256, clipId, requestSha256, clipObjectSha256, clipSha256, restFrameSha256, poseActionSha256, sourceReplayWorkerCohortSha256, materializationStatus: 'runtime-owned-immutable-cas-clip', clip: normalizedClip, canonicalSha256 },
  }
}

function normalizePartDelta(value: unknown, allowedPartIds: ReadonlySet<string>): MechanicalAnimationPartDelta | null {
  if (!isRecord(value)) return null
  const partId = identifierValue(value.part_id)
  const deltaPose = isRecord(value.delta_pose) ? value.delta_pose : null
  const translationM = deltaPose ? numberTuple(deltaPose.translation_m, 3, -10, 10) : null
  const rotationQuatXyzw = deltaPose ? numberTuple(deltaPose.rotation_quat_xyzw, 4, -1, 1) : null
  if (!partId || !allowedPartIds.has(partId) || !translationM || !rotationQuatXyzw || !deltaPose || !Array.isArray(deltaPose.scale) || deltaPose.scale.length !== 3 || deltaPose.scale.some((item) => item !== 1)) return null
  const quaternionLengthSquared = rotationQuatXyzw.reduce((sum, item) => sum + item * item, 0)
  if (Math.abs(quaternionLengthSquared - 1) > 1e-5) return null
  return { partId, translationM, rotationQuatXyzw, scale: [1, 1, 1] }
}

export function normalizeMechanicalAnimationFramePreview(
  payload: unknown,
  binding: MechanicalAnimationBinding,
  clipLink: MechanicalAnimationClipLink,
  sampleTimeTicks: number,
): MechanicalAnimationFrameReadback {
  if (!isRecord(payload)) return { status: 'Unavailable', frame: null, code: 'MECHANICAL_ANIMATION_FRAME_INVALID' }
  if (payload.status === 'Unavailable') return { status: 'Unavailable', frame: null, code: unavailableCode(payload, 'MECHANICAL_ANIMATION_FRAME_UNAVAILABLE') }
  const geometry = isRecord(payload.pose_geometry_preview) ? payload.pose_geometry_preview : null
  const transientArtifact = geometry && isRecord(geometry.transient_artifact) ? geometry.transient_artifact : null
  const workerReplay = geometry && isRecord(geometry.worker_replay) ? geometry.worker_replay : null
  if (payload.schema_version !== FRAME_PREVIEW_SCHEMA
    || !exactBinding(payload, binding)
    || payload.clip_id !== clipLink.clipId
    || payload.clip_object_sha256 !== clipLink.clipObjectSha256
    || payload.clip_sha256 !== clipLink.clipSha256
    || payload.rest_frame_sha256 !== clipLink.restFrameSha256
    || payload.pose_action_sha256 !== clipLink.poseActionSha256
    || payload.source_replay_worker_cohort_sha256 !== clipLink.sourceReplayWorkerCohortSha256
    || payload.sample_time_ticks !== sampleTimeTicks
    || payload.geometry_materialization !== 'transient-double-worker-glb-not-persisted'
    || payload.runtime_write_performed !== false
    || payload.persistent_user_data_touched !== false
    || payload.quality_status !== 'structural_only'
    || !geometry
    || geometry.schema_version !== POSE_GEOMETRY_PREVIEW_SCHEMA
    || geometry.project_id !== binding.projectId
    || geometry.candidate_id !== binding.candidateId
    || geometry.source_artifact_id !== binding.artifactId
    || geometry.source_program_sha256 !== clipLink.programSha256
    || geometry.operator_catalog_sha256 !== clipLink.operatorCatalogSha256
    || geometry.readback_config_sha256 !== clipLink.readbackConfigSha256
    || geometry.rest_frame_sha256 !== clipLink.restFrameSha256
    || geometry.pose_action_sha256 !== clipLink.poseActionSha256
    || geometry.sample_time_ticks !== sampleTimeTicks
    || geometry.geometry_materialization !== 'transient-worker-glb-not-persisted'
    || geometry.runtime_write_performed !== false
    || geometry.persistent_user_data_touched !== false
    || geometry.validator_status !== 'passed'
    || geometry.quality_status !== 'structural_only'
    || !transientArtifact
    || transientArtifact.delivery !== 'hash-and-readback-only-no-cas-object'
    || !workerReplay
    || workerReplay.byte_exact !== true
    || workerReplay.metadata_exact !== true
    || workerReplay.first_build_cohort_sha256 !== clipLink.sourceReplayWorkerCohortSha256
    || workerReplay.repeat_build_cohort_sha256 !== clipLink.sourceReplayWorkerCohortSha256
    || !Array.isArray(geometry.part_deltas)
    || geometry.part_deltas.length === 0
    || geometry.part_deltas.length > MAX_LINKS
  ) return { status: 'Unavailable', frame: null, code: 'MECHANICAL_ANIMATION_FRAME_BINDING_MISMATCH' }
  const hashes = {
    frameSha256: shaValue(payload.frame_sha256),
    evaluatedPoseSha256: shaValue(geometry.evaluated_pose_sha256),
    posedProgramSha256: shaValue(geometry.posed_program_sha256),
    transientArtifactSha256: shaValue(transientArtifact.artifact_sha256),
    canonicalSha256: shaValue(payload.canonical_sha256),
  }
  if (Object.values(hashes).some((value) => !value)) return { status: 'Unavailable', frame: null, code: 'MECHANICAL_ANIMATION_FRAME_INVALID' }
  if (workerReplay.first_artifact_sha256 !== hashes.transientArtifactSha256 || workerReplay.repeat_artifact_sha256 !== hashes.transientArtifactSha256) return { status: 'Unavailable', frame: null, code: 'MECHANICAL_ANIMATION_FRAME_REPLAY_MISMATCH' }
  const allowedPartIds = new Set(clipLink.clip.restFrame.links.map((link) => link.partId))
  const partDeltas = geometry.part_deltas.map((value) => normalizePartDelta(value, allowedPartIds))
  if (!partDeltas.every((value): value is MechanicalAnimationPartDelta => value !== null)
    || new Set(partDeltas.map((value) => value.partId)).size !== partDeltas.length
    || partDeltas.length !== allowedPartIds.size
    || [...allowedPartIds].some((partId) => !partDeltas.some((delta) => delta.partId === partId))
  ) return { status: 'Unavailable', frame: null, code: 'MECHANICAL_ANIMATION_FRAME_PART_DELTA_INVALID' }
  return {
    status: 'Ready',
    code: null,
    frame: {
      schemaVersion: FRAME_PREVIEW_SCHEMA,
      projectId: binding.projectId,
      candidateId: binding.candidateId,
      artifactId: binding.artifactId,
      clipId: clipLink.clipId,
      clipSha256: clipLink.clipSha256,
      clipObjectSha256: clipLink.clipObjectSha256,
      restFrameSha256: clipLink.restFrameSha256,
      poseActionSha256: clipLink.poseActionSha256,
      sampleTimeTicks,
      frameSha256: hashes.frameSha256!,
      evaluatedPoseSha256: hashes.evaluatedPoseSha256!,
      posedProgramSha256: hashes.posedProgramSha256!,
      transientArtifactSha256: hashes.transientArtifactSha256!,
      workerBuildCohortSha256: clipLink.sourceReplayWorkerCohortSha256,
      partDeltas,
      runtimeWritePerformed: false,
      persistentUserDataTouched: false,
      qualityStatus: 'structural_only',
      canonicalSha256: hashes.canonicalSha256!,
    },
  }
}

export function isCurrentMechanicalAnimationFrameResponse(active: boolean, currentRequestId: number, responseRequestId: number): boolean {
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

/** Executable race fixture for the request-generation guard used by the frame effect. */
export async function mechanicalAnimationFrameDeferredResponseSelfCheck(): Promise<{ passed: boolean; checks: string[] }> {
  let currentRequestId = 0
  let state = 'initial'
  let loading = false

  const launch = (pending: Promise<string>): number => {
    const requestId = ++currentRequestId
    loading = true
    void pending.then((value) => {
      if (!isCurrentMechanicalAnimationFrameResponse(true, currentRequestId, requestId)) return
      state = value
    }).catch(() => {
      if (!isCurrentMechanicalAnimationFrameResponse(true, currentRequestId, requestId)) return
      state = 'error'
    }).finally(() => {
      if (isCurrentMechanicalAnimationFrameResponse(true, currentRequestId, requestId)) loading = false
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

export type MechanicalAnimationHierarchyRow = MechanicalRestFrameLink & {
  parentLinkId: string | null
  depth: number
}

export function mechanicalAnimationHierarchyRows(clip: MechanicalAnimationClip | null): MechanicalAnimationHierarchyRow[] {
  if (!clip) return []
  const parentByChild = new Map(clip.restFrame.parentMap.map((item) => [item.childLinkId, item.parentLinkId]))
  const linksById = new Map(clip.restFrame.links.map((link) => [link.linkId, link]))
  return clip.restFrame.evaluationOrder.map((linkId) => {
    let current = linkId
    let depth = 0
    const seen = new Set<string>()
    while (parentByChild.has(current) && !seen.has(current)) {
      seen.add(current)
      current = parentByChild.get(current) ?? current
      depth += 1
    }
    const link = linksById.get(linkId)
    return link ? { ...link, parentLinkId: parentByChild.get(linkId) ?? null, depth } : null
  }).filter((row): row is MechanicalAnimationHierarchyRow => row !== null)
}

function selfCheckHash(seed: string): string {
  return seed.repeat(64).slice(0, 64)
}

export type MechanicalAnimationNormalizerSelfCheck = {
  passed: boolean
  checks: readonly string[]
}

/**
 * Small source-level contract check used by the Viewer source gate.  It is
 * intentionally pure and does not call Tauri, Runtime, CAS, or localStorage.
 */
export function mechanicalAnimationNormalizerSelfCheck(): MechanicalAnimationNormalizerSelfCheck {
  const hashes = {
    artifact: selfCheckHash('a'),
    program: selfCheckHash('b'),
    restFrame: selfCheckHash('c'),
    poseAction: selfCheckHash('d'),
    clip: selfCheckHash('e'),
    clipObject: selfCheckHash('f'),
    evidence: selfCheckHash('1'),
    catalog: selfCheckHash('2'),
    readback: selfCheckHash('3'),
    request: selfCheckHash('4'),
    cohort: selfCheckHash('5'),
    parentMap: selfCheckHash('6'),
    sampling: selfCheckHash('7'),
    link: selfCheckHash('8'),
    frame: selfCheckHash('9'),
    evaluated: selfCheckHash('a1'),
    posed: selfCheckHash('b1'),
    transient: selfCheckHash('c1'),
  }
  const binding: MechanicalAnimationBinding = { projectId: 'project-a', candidateId: 'candidate-a', artifactId: hashes.artifact }
  const rawClip = {
    schema_version: CLIP_SCHEMA,
    clip_id: 'clip-a',
    project_id: binding.projectId,
    candidate_id: binding.candidateId,
    artifact_id: binding.artifactId,
    artifact_readback_sha256: hashes.artifact,
    geometry_candidate_evidence_sha256: hashes.evidence,
    program_sha256: hashes.program,
    operator_catalog_sha256: hashes.catalog,
    readback_config_sha256: hashes.readback,
    request_sha256: hashes.request,
    rest_frame: {
      schema_version: REST_FRAME_SCHEMA,
      rest_frame_id: 'rest-a', project_id: binding.projectId, artifact_id: binding.artifactId, candidate_id: binding.candidateId,
      program_sha256: hashes.program, coordinate_system: 'forgecad-rh-y-up-m@1', transform_convention: 'column-vector-trs-quaternion@1', root_link_id: 'link-a',
      links: [{ link_id: 'link-a', part_id: 'part-a', source_node_ids: ['node-a'], joint_type: 'revolute', rest_translation_m: [0, 0, 0], rest_rotation_quat_xyzw: [0, 0, 0, 1], axis_local: [0, 1, 0], limit_min: -1, limit_max: 1, value_unit: 'radian' }],
      parent_map: [], evaluation_order: ['link-a'], parent_map_sha256: hashes.parentMap, canonical_sha256: hashes.restFrame,
    },
    rest_frame_sha256: hashes.restFrame,
    pose_action: {
      schema_version: POSE_ACTION_SCHEMA, action_id: 'action-a', project_id: binding.projectId, candidate_id: binding.candidateId, rest_frame_sha256: hashes.restFrame, program_sha256: hashes.program,
      timebase_hz: 1000, duration_ticks: 1000, interpolation: 'linear@1', extrapolation: 'clamp@1', unkeyed_policy: 'rest@1', channels: [{ link_id: 'link-a', value_unit: 'radian', keys: [{ time_ticks: 0, value: 0 }, { time_ticks: 1000, value: 1 }] }], canonical_sha256: hashes.poseAction,
    },
    pose_action_sha256: hashes.poseAction,
    sampling_policy: { schema_version: SAMPLING_POLICY_SCHEMA, timebase_hz: 1000, interpolation: 'scalar-linear-integer-ticks-clamped', unkeyed: 'rest', sample_time_ticks: [0, 500, 1000], max_samples: MAX_TICKS, frame_preview_batch_size: 1 },
    sampling_policy_sha256: hashes.sampling,
    source_replay: { worker_build_cohort_sha256: hashes.cohort, first_artifact_sha256: hashes.artifact, repeat_artifact_sha256: hashes.artifact, byte_exact_with_candidate_artifact: true, strict_readback_passed: true },
    materialization_status: 'runtime-owned-immutable-cas-clip', quality_status: 'structural_only',
    limitations: ['caller-authored-rest-frame-not-artifact-rig-provenance', 'rigid-parts-only-no-skinning-or-deformation', 'no-ik-constraints-nla-fcurves-drivers-or-timeline'], canonical_sha256: hashes.clip,
  }
  const rawLink = {
    schema_version: LINK_SCHEMA, project_id: binding.projectId, candidate_id: binding.candidateId, artifact_id: binding.artifactId,
    artifact_readback_sha256: hashes.artifact, geometry_candidate_evidence_sha256: hashes.evidence, program_sha256: hashes.program, operator_catalog_sha256: hashes.catalog, readback_config_sha256: hashes.readback,
    clip_id: 'clip-a', request_sha256: hashes.request, clip_object_sha256: hashes.clipObject, clip_sha256: hashes.clip, rest_frame_sha256: hashes.restFrame, pose_action_sha256: hashes.poseAction, source_replay_worker_cohort_sha256: hashes.cohort,
    materialization_status: 'runtime-owned-immutable-cas-clip', clip: rawClip, canonical_sha256: hashes.link,
  }
  const rawInventory = { schema_version: INVENTORY_SCHEMA, status: 'Ready', read_only: true, runtime_write_performed: false, persistent_user_data_touched: false, project_id: binding.projectId, candidate_id: binding.candidateId, artifact_id: binding.artifactId, clip_count: 1, max_clips: MAX_CLIPS, clips: [{ clip_id: 'clip-a', clip_object_sha256: hashes.clipObject, clip_sha256: hashes.clip, rest_frame_sha256: hashes.restFrame, pose_action_sha256: hashes.poseAction, source_replay_worker_cohort_sha256: hashes.cohort, materialization_status: 'runtime-owned-immutable-cas-clip' }], quality_status: 'structural_only', limitations: ['caller-authored-rigid-links-only'], canonical_sha256: hashes.link }
  const inventory = normalizeViewerMechanicalAnimationInventory(rawInventory, binding)
  const link = normalizeMechanicalAnimationClipLink(rawLink, binding, inventory.clips[0])
  const mismatch = normalizeMechanicalAnimationClipLink(rawLink, { ...binding, candidateId: 'candidate-b' }, inventory.clips[0])
  const rawFrame = {
    schema_version: FRAME_PREVIEW_SCHEMA, project_id: binding.projectId, candidate_id: binding.candidateId, artifact_id: binding.artifactId,
    clip_id: 'clip-a', clip_object_sha256: hashes.clipObject, clip_sha256: hashes.clip, rest_frame_sha256: hashes.restFrame, pose_action_sha256: hashes.poseAction,
    sample_time_ticks: 500, frame_sha256: hashes.frame, source_replay_worker_cohort_sha256: hashes.cohort,
    pose_geometry_preview: {
      schema_version: POSE_GEOMETRY_PREVIEW_SCHEMA, project_id: binding.projectId, candidate_id: binding.candidateId, source_artifact_id: binding.artifactId,
      source_program_sha256: hashes.program, operator_catalog_sha256: hashes.catalog, readback_config_sha256: hashes.readback, rest_frame_sha256: hashes.restFrame, pose_action_sha256: hashes.poseAction,
      sample_time_ticks: 500, evaluated_pose_sha256: hashes.evaluated, posed_program_sha256: hashes.posed,
      part_deltas: [{ part_id: 'part-a', delta_pose: { translation_m: [0.1, 0, 0], rotation_quat_xyzw: [0, 0, 0, 1], scale: [1, 1, 1] } }],
      transient_artifact: { artifact_sha256: hashes.transient, delivery: 'hash-and-readback-only-no-cas-object' },
      worker_replay: { first_build_cohort_sha256: hashes.cohort, repeat_build_cohort_sha256: hashes.cohort, first_artifact_sha256: hashes.transient, repeat_artifact_sha256: hashes.transient, byte_exact: true, metadata_exact: true },
      geometry_materialization: 'transient-worker-glb-not-persisted', runtime_write_performed: false, persistent_user_data_touched: false, validator_status: 'passed', quality_status: 'structural_only',
    },
    geometry_materialization: 'transient-double-worker-glb-not-persisted', runtime_write_performed: false, persistent_user_data_touched: false, quality_status: 'structural_only', canonical_sha256: hashes.frame,
  }
  const frame = link.link ? normalizeMechanicalAnimationFramePreview(rawFrame, binding, link.link, 500) : { status: 'Unavailable' as const, frame: null, code: 'NO_LINK' }
  const staleFrame = link.link ? normalizeMechanicalAnimationFramePreview(rawFrame, binding, link.link, 0) : { status: 'Unavailable' as const, frame: null, code: 'NO_LINK' }
  const expandedLink = link.link ? {
    ...link.link,
    clip: {
      ...link.link.clip,
      restFrame: {
        ...link.link.clip.restFrame,
        links: [...link.link.clip.restFrame.links, { ...link.link.clip.restFrame.links[0]!, linkId: 'link-b', partId: 'part-b' }],
      },
    },
  } : null
  const partialFrame = expandedLink ? normalizeMechanicalAnimationFramePreview(rawFrame, binding, expandedLink, 500) : { status: 'Unavailable' as const, frame: null, code: 'NO_LINK' }
  const provenanceFields = [
    'artifact_readback_sha256',
    'geometry_candidate_evidence_sha256',
    'program_sha256',
    'operator_catalog_sha256',
    'readback_config_sha256',
  ] as const
  const provenanceMismatchRejected = provenanceFields.every((field) => {
    const tamperedLink = { ...rawLink, [field]: selfCheckHash('9') }
    const readback = normalizeMechanicalAnimationClipLink(tamperedLink, binding, inventory.clips[0])
    return readback.status === 'Unavailable' && readback.link === null
  })
  const ownerDescriptors: MechanicalAnimationPartOwnerDescriptor[] = [
    { nodeId: 'node-a', partId: 'part-a', ancestorPartId: null, directRootChild: true, identityTransform: true, isBone: false, isSkinnedMesh: false },
  ]
  const ownerMap = validateMechanicalAnimationPartOwners(ownerDescriptors, ['part-a'], 0)
  const duplicateOwner = validateMechanicalAnimationPartOwners([...ownerDescriptors, { ...ownerDescriptors[0]!, nodeId: 'node-b' }], ['part-a'], 0)
  const nestedOwner = validateMechanicalAnimationPartOwners([{ ...ownerDescriptors[0]!, directRootChild: false }], ['part-a'], 0)
  const unknownOwner = validateMechanicalAnimationPartOwners([{ ...ownerDescriptors[0]!, partId: 'part-b' }], ['part-a'], 0)
  const transformedOwner = validateMechanicalAnimationPartOwners([{ ...ownerDescriptors[0]!, identityTransform: false }], ['part-a'], 0)
  const boneOwner = validateMechanicalAnimationPartOwners([{ ...ownerDescriptors[0]!, isBone: true }], ['part-a'], 0)
  const skinnedOwner = validateMechanicalAnimationPartOwners([{ ...ownerDescriptors[0]!, isSkinnedMesh: true }], ['part-a'], 0)
  const embeddedAnimation = validateMechanicalAnimationPartOwners(ownerDescriptors, ['part-a'], 1)
  const checks = [
    ...(inventory.status === 'Ready' && inventory.clips.length === 1 ? ['inventory-ready'] : ['inventory-failed']),
    ...(link.status === 'Ready' && link.link?.clip.restFrame.links.length === 1 ? ['link-hierarchy-ready'] : ['link-hierarchy-failed']),
    ...(mismatch.status === 'Unavailable' && mismatch.link === null ? ['candidate-mismatch-fail-closed'] : ['candidate-mismatch-accepted']),
    ...(provenanceMismatchRejected ? ['provenance-hash-mismatch-fail-closed'] : ['provenance-hash-mismatch-accepted']),
    ...(frame.status === 'Ready' && frame.frame?.partDeltas.length === 1 ? ['frame-preview-ready'] : ['frame-preview-failed']),
    ...(staleFrame.status === 'Unavailable' && staleFrame.frame === null ? ['frame-tick-mismatch-fail-closed'] : ['frame-tick-mismatch-accepted']),
    ...(partialFrame.status === 'Unavailable' && partialFrame.frame === null ? ['partial-part-delta-fail-closed'] : ['partial-part-delta-accepted']),
    ...(ownerMap.status === 'Ready' && ownerMap.ownerNodeIdsByPartId.get('part-a') === 'node-a' ? ['part-owner-map-ready'] : ['part-owner-map-failed']),
    ...(duplicateOwner.status === 'Unavailable' ? ['duplicate-part-owner-fail-closed'] : ['duplicate-part-owner-accepted']),
    ...(nestedOwner.status === 'Unavailable' ? ['nested-part-owner-fail-closed'] : ['nested-part-owner-accepted']),
    ...(unknownOwner.status === 'Unavailable' ? ['unknown-part-owner-fail-closed'] : ['unknown-part-owner-accepted']),
    ...(transformedOwner.status === 'Unavailable' ? ['nonidentity-part-owner-fail-closed'] : ['nonidentity-part-owner-accepted']),
    ...(boneOwner.status === 'Unavailable' ? ['bone-part-owner-fail-closed'] : ['bone-part-owner-accepted']),
    ...(skinnedOwner.status === 'Unavailable' ? ['skinned-part-owner-fail-closed'] : ['skinned-part-owner-accepted']),
    ...(embeddedAnimation.status === 'Unavailable' ? ['embedded-animation-fail-closed'] : ['embedded-animation-accepted']),
  ]
  return { passed: checks.every((check) => check.endsWith('ready') || check.endsWith('fail-closed')), checks }
}
