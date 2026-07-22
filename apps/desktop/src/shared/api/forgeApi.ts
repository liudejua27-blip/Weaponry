import type {
  ApiErrorEnvelope,
  AssetFileResponse,
  AssetRevealResponse,
  AssetUploadRequest,
  AssetUploadResponse,
  CreativeGraphResponse,
  CreativeInterpretationRequest,
  CreativeInterpretationResponse,
  CreativeRecastConfirmRequest,
  CreativeRecastConfirmResponse,
  CreateWeaponRequest,
  CreateWeaponResponse,
  ExportUnityRequest,
  Generate3DRequest,
  HealthResponse,
  JobActionListResponse,
  JobActionResponse,
  JobDetail,
  JobListResponse,
  JobEvent,
  JobRuntimeStateResponse,
  PatchWeaponRequest,
  ProviderSettings,
  RuntimeRecoveryResponse,
  RuntimeWorkOnceResponse,
  WeaponDetail,
  WeaponSummary,
  ConceptProjectDetail,
  ConceptProjectListResponse,
  ConceptVersionDetail,
  CreateConceptProjectRequest,
  ModuleAssetListResponse,
  ModuleAssetRecord,
  UpdateModuleAssetCatalogMetadataRequest,
  ModuleGraphRecord,
  DesignVariantListResponse,
  DesignVariantRecord,
  InterpretDesignBriefRequest,
  DesignBriefRecord,
  GenerateDesignVariantsRequest,
  SelectDesignVariantRequest,
  CreateConceptExportRequest,
  ConceptExportRecord,
  ProposeChangeSetRequest,
  ProposeConnectorSnapRequest,
  PlanDesignChangeSetRequest,
  PlannedChangeSetRecord,
  DesignChangeSet,
  ChangeSetPreviewResponse,
  ChangeSetConfirmResponse,
  ChangeSetTimelineResponse,
  CreateChangeSetAuditExportRequest,
  ChangeSetAuditExportRecord,
  ChangeSetAuditExportListResponse,
  InspectConceptVersionRequest,
  QualityRunRecord,
  ConceptJobRecord,
  AgentThreadDetail,
  AgentThreadListResponse,
  AgentEvent,
  AgentItem,
  AgentTurn,
  AgentApproval,
  AgentApprovalResolution,
  CreateAgentThreadRequest,
  StartAgentTurnRequest,
  CreateAgentApprovalRequest,
  ResolveAgentApprovalRequest,
  DomainPackManifest,
  BuildAgentBlockoutRequest,
  BuildAgentBlockoutResponse,
  RenderAgentBlockoutConceptPreviewRequest,
  AgentBlockoutConceptPreview,
  SegmentAgentBlockoutRequest,
  SegmentAgentBlockoutResponse,
  AgentMaterialPreset,
  AgentMaterialTextureListResponse,
  AgentMaterialTextureObject,
  RegisterAgentMaterialTextureRequest,
  CommitAgentBlockoutRequest,
  AgentAssetVersion,
  ProposeAgentAssetChangeSetRequest,
  AgentAssetChangeSet,
  AgentAssetChangeSetConfirmResponse,
  AgentAssetQualityReport,
  AgentAssetExportResponse,
  AgentAssetRenderSet,
  ImportAgentGlbRequest,
  ImportAgentGlbResponse,
  AgentProviderCheckResponse,
  AgentComponentRecord,
  AgentComponentCandidate,
  AgentStructureSuggestionList,
  ResolvedSemanticProportionOptions,
  SaveAgentComponentRequest,
  ActiveDesignSnapshot,
  ActiveDesignNavigation,
  SelectActiveDesignRequest,
  SetActiveDesignPartDisplayRequest,
  SetActiveDesignRenderPresetRequest,
  ConvertLegacyActiveDesignRequest,
  NavigateActiveDesignRequest,
  LegacyActiveDesignConversionResponse,
} from '../types'
import {
  appServerTransport,
  isNativeDesktopRuntime,
  type NativeRequestOptions,
} from './appServerTransport.js'
import { AppServerProtocolError, type JsonRpcNotification } from './appServerProtocol.js'

const DEFAULT_BASE_URL = appServerTransport.getCompatibilityBaseUrl()
const NATIVE_LIST_LIMIT = 200
const NATIVE_REPLAY_MAX_PAGES = 32
const NATIVE_REPLAY_TRANSIENT_RECOVERY_ATTEMPTS = 1
const NATIVE_TURN_WAIT_MS = 125_000
const NATIVE_TURN_READ_POLL_MS = 250

type NativeTurnCancellation = {
  threadId: string
  cancellationId: string
  cancellationToken: string
}

type NativeProviderCancellation = {
  executionId: string
  cancellationId: string
  cancellationToken: string
}

type NativeItemList = {
  items: AgentItem[]
  nextSequence: number | null
}

type NativeProviderStatus = 'unconfigured' | 'ready' | 'failed' | 'cancelled'
type NativeProviderFailureCategory =
  | 'invalid_request'
  | 'authentication'
  | 'balance'
  | 'rate_limited'
  | 'server_unavailable'
  | 'timeout'
  | 'network'
  | 'empty_content'
  | 'invalid_json'
  | 'schema_violation'
  | 'budget_exceeded'
  | 'cancelled'

type NativeProviderPreflight = {
  executionId: string
  status: Exclude<NativeProviderStatus, 'cancelled'>
  providerId: string | null
  configured: boolean
  failureCategory: NativeProviderFailureCategory | null
}

type NativeProviderCheck = {
  executionId: string
  providerId: string
  status: NativeProviderStatus
  networkCallMade: boolean
  failureCategory: NativeProviderFailureCategory | null
}

function protocolRequest(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  return appServerTransport.request(typeof input === 'string' ? input : input.toString(), init)
}

export class ForgeApiError extends Error {
  constructor(
    message: string,
    readonly code: string,
    readonly recoverable: boolean,
    readonly details: Record<string, unknown>,
    readonly status: number
  ) {
    super(message)
    this.name = 'ForgeApiError'
  }
}

/** A Snapshot response carries the server revision ETag without owning UI state. */
export type ActiveDesignApiResponse<T> = {
  data: T
  etag: string | null
}

export type AgentAssetChangeSetPreviewGlb = {
  glb: ArrayBuffer
  sha256: string | null
  baseAssetVersionId: string | null
  triangleCount: number | null
}

export type AgentAssetGlbBinary = {
  glb: ArrayBuffer
  artifactProfileId: 'external_reference' | 'interactive_preview' | 'production_concept'
  artifactProfileSha256: string | null
  shapeProgramSha256: string | null
  glbSha256: string
  triangleCount: number
  byteSize: number
}

export type SingleResultPreviewIdentity = {
  projectId: string
  turnId: string
  previewId: string
  artifactSha256: string
  artifactProfileId: 'interactive_preview' | 'production_concept'
}

export type SingleResultPreviewGlb = AgentAssetGlbBinary & {
  projectId: string
  turnId: string
  previewId: string
}

export type SingleResultRejectResponse = {
  preview_id: string
  rejected: boolean
  permanent_side_effects: 0
}

export type AgentAssetGlbDownload = Omit<AgentAssetGlbBinary, 'glb'> & {
  blob: Blob
  filename: string
}

export type AgentRenderPackageDownload = {
  blob: Blob
  filename: string
  renderSetSha256: string | null
}

/** R007 UI contract. The reference object stays read-only in Rust-owned CAS. */
export type ReferenceEvidenceCreateRequest = {
  client_request_id: string
  project_id: string
  domain_pack_id: string
  kind: 'image' | 'glb'
  file_name: string
  media_type: string
  source_statement: string
  license_statement: string
  missing_views: string[]
  /** User-declared evidence coverage. It is never inferred from an empty missing_views list. */
  reference_class?: 'single_image' | 'multi_view_contact_sheet' | 'glb_readback'
  user_notes?: string
  content_base64?: string
  imported_asset_version_id?: string
}

export type ReferenceEvidenceKind = 'image' | 'glb'
export type ReferenceEvidenceClass = 'single_image' | 'multi_view_contact_sheet' | 'glb_readback'

/** Bounded readback facts only; source topology and CAS locations never reach the UI. */
export type ReferenceGlbReadbackFacts = {
  sha256: string
  byte_size: number
  triangle_count: number
  bounds_mm: [number, number, number]
  mesh_count: number
  primitive_count: number
  material_count: number
  node_count: number
}

export type ReferenceImageSurfaceFacts = {
  width: number
  height: number
  aspect_ratio_milli: number
  dominant_color_buckets: Array<'black' | 'gray' | 'white' | 'blue' | 'cyan' | 'red' | 'yellow' | 'green' | 'violet'>
  brightness: 'dark' | 'balanced' | 'bright'
  edge_density: 'low' | 'medium' | 'high'
  foreground_bbox_normalized: [number, number, number, number]
  contact_sheet_layout_evidence: boolean
  foreground_confidence: 'low' | 'medium'
}

export type ReferenceEvidenceObservations = {
  silhouette_summary: string
  proportion_ranges: string[]
  material_zone_observations: string[]
  visible_part_hypotheses: Array<{ role: string; confidence: 'low' | 'medium' | 'high'; visible_basis: string }>
  uncertainties: string[]
  image_surface_facts?: ReferenceImageSurfaceFacts
}

/** Presentation-safe, Rust-owned R007 evidence. No object path or source bytes are present. */
export type ReferenceEvidenceRecord = {
  schema_version: 'ReferenceEvidence@1'
  evidence_id: string
  project_id: string
  kind: ReferenceEvidenceKind
  reference_class: ReferenceEvidenceClass
  domain_pack_id: string
  source_file_name: string
  source_media_type: string
  source_object_sha256: string
  source_imported_asset_version_id?: string
  source_statement: string
  license_statement: string
  missing_views: string[]
  user_notes: string
  observations: ReferenceEvidenceObservations
  created_at: string
  glb_inspection?: ReferenceGlbReadbackFacts
}

export type ReferenceEvidenceCreateResponse = {
  schema_version: 'ReferenceEvidenceCreateResponse@1'
  reference_evidence: ReferenceEvidenceRecord
}

export type ReferenceEvidenceSummary = ReferenceEvidenceRecord

export type ReferenceComponentRecipeRef = {
  schema_version: 'ComponentRecipeRef@1'
  recipe_id: string
  version: number
  recipe_sha256: string
}

export type ReferenceGuidedRebuildPlanStatus = 'draft' | 'previewed' | 'confirmed' | 'rejected'

export type ReferenceGuidedRebuildPlanRecord = {
  schema_version: 'ReferenceGuidedRebuildPlan@1'
  rebuild_plan_id: string
  project_id: string
  evidence_id: string
  base_asset_version_id?: string
  domain_pack_id: string
  recipe_id: string
  recipe_registry_sha256: string
  rebuild_summary: string
  retained_evidence: string[]
  intended_differences: string[]
  unresolved_uncertainties: string[]
  status: ReferenceGuidedRebuildPlanStatus
  preview_change_set_id?: string
  confirmed_asset_version_id?: string
  created_at: string
  updated_at: string
}

export type ReferenceSurfaceObservationKind = 'silhouette' | 'proportion' | 'visible_part' | 'material_zone'
export type ReferenceSurfaceFidelityCeiling =
  | 'single_image_visible_surface_only'
  | 'multi_view_image_visible_surface_only'
  | 'strict_glb_readback_visible_bounds_only'
export type ReferenceSurfaceIntentionalChange =
  | 'non_functional_recipe_interpretation'
  | 'reviewed_recipe_component_substitution'
  | 'material_preset_normalization'
  | 'surface_adornment_normalization'
export type ReferenceSurfaceUnresolved =
  | 'missing_views'
  | 'hidden_structure'
  | 'exact_dimensions'
  | 'material_physics'
  | 'functional_behavior'

export type ReferenceSurfaceBinding = {
  binding_id: string
  observation_kind: ReferenceSurfaceObservationKind
  observation_index: number
  target_part_slot_id?: string
  target_recipe: ReferenceComponentRecipeRef
  target_part_role: string
  target_material_zone_id: string
  target_surface_slot_id: string
}

/** Frozen Rust analysis; TypeScript presents it but never recalculates identity hashes. */
export type ReferenceSurfaceAnalysis = {
  schema_version: 'ReferenceSurfaceAnalysis@1'
  analysis_id: string
  rebuild_plan_id: string
  evidence_id: string
  source_object_sha256: string
  domain_pack_id: string
  target_root_recipe: ReferenceComponentRecipeRef
  c106_registry_sha256: string
  surface_skill_id: string
  surface_skill_version: number
  surface_skill_sha256: string
  fidelity_ceiling: ReferenceSurfaceFidelityCeiling
  bindings: ReferenceSurfaceBinding[]
  retained_observation_kinds: ReferenceSurfaceObservationKind[]
  intentionally_changed: ReferenceSurfaceIntentionalChange[]
  unresolved: ReferenceSurfaceUnresolved[]
  glb_readback_facts?: Omit<ReferenceGlbReadbackFacts, 'byte_size'>
  created_at: string
}

/** Exact immutable identities from the Rust-owned evidence/result pair; never a client hash calculation. */
export type ReferenceResultPair = {
  source_object_sha256: string
  result_asset_version_id: string | null
  result_glb_sha256: string | null
}

export type ReferenceEvidenceProjectRead = {
  schema_version: 'ReferenceEvidenceProjectRead@1'
  reference_evidence: ReferenceEvidenceSummary[]
  reference_guided_rebuild_plans: ReferenceGuidedRebuildPlanRecord[]
}

export type ReferenceGuidedRebuildPlanRead = {
  schema_version: 'ReferenceGuidedRebuildPlanRead@1'
  reference_guided_rebuild_plan: ReferenceGuidedRebuildPlanRecord
  reference_surface_analysis: ReferenceSurfaceAnalysis | null
  reference_result_pair: ReferenceResultPair
}

/**
 * POST `reference-guided-rebuild:preview` is deliberately not a loose
 * ChangeSet response.  Rust returns the proposed ChangeSet together with the
 * immutable draft plan and its frozen analysis.  Normalize its *known absent*
 * result pair to the same sealed read model used by GET; no analysis, hash, or
 * visual assertion is inferred in the client.
 */
export type ReferenceGuidedRebuildPreviewProposal = {
  changeSet: AgentAssetChangeSet
  planRead: ReferenceGuidedRebuildPlanRead
}

const REFERENCE_SHA256_PATTERN = /^[a-f0-9]{64}$/
const REFERENCE_REBUILD_STATUSES = new Set<ReferenceGuidedRebuildPlanStatus>([
  'draft', 'previewed', 'confirmed', 'rejected',
])

function isReferenceRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isReferenceSha256(value: unknown): value is string {
  return typeof value === 'string' && REFERENCE_SHA256_PATTERN.test(value)
}

function isNullableReferenceId(value: unknown): value is string | null {
  return value === null || (typeof value === 'string' && value.length > 0)
}

/**
 * Accept only the frozen Rust projection. This verifies returned identities
 * agree, but deliberately never recomputes a source/result hash in JS.
 */
export function projectReferenceGuidedRebuildPlanRead(value: unknown): ReferenceGuidedRebuildPlanRead | null {
  if (!isReferenceRecord(value) || value.schema_version !== 'ReferenceGuidedRebuildPlanRead@1') return null
  const plan = value.reference_guided_rebuild_plan
  const analysis = value.reference_surface_analysis
  const pair = value.reference_result_pair
  if (!isReferenceRecord(plan) || !isReferenceRecord(pair)) return null
  if (
    plan.schema_version !== 'ReferenceGuidedRebuildPlan@1'
    || typeof plan.rebuild_plan_id !== 'string'
    || typeof plan.project_id !== 'string'
    || typeof plan.evidence_id !== 'string'
    || (plan.base_asset_version_id !== undefined && !isNullableReferenceId(plan.base_asset_version_id))
    || typeof plan.domain_pack_id !== 'string'
    || typeof plan.recipe_id !== 'string'
    || !isReferenceSha256(plan.recipe_registry_sha256)
    || typeof plan.rebuild_summary !== 'string'
    || !Array.isArray(plan.retained_evidence) || !plan.retained_evidence.every((item) => typeof item === 'string')
    || !Array.isArray(plan.intended_differences) || !plan.intended_differences.every((item) => typeof item === 'string')
    || !Array.isArray(plan.unresolved_uncertainties) || !plan.unresolved_uncertainties.every((item) => typeof item === 'string')
    || typeof plan.status !== 'string'
    || !REFERENCE_REBUILD_STATUSES.has(plan.status as ReferenceGuidedRebuildPlanStatus)
    || (plan.preview_change_set_id !== undefined && !isNullableReferenceId(plan.preview_change_set_id))
    || (plan.confirmed_asset_version_id !== undefined && !isNullableReferenceId(plan.confirmed_asset_version_id))
    || !isReferenceSha256(pair.source_object_sha256)
    || !isNullableReferenceId(pair.result_asset_version_id)
    || (pair.result_glb_sha256 !== null && !isReferenceSha256(pair.result_glb_sha256))
  ) return null

  if (analysis !== null) {
    if (
      !isReferenceRecord(analysis)
      || analysis.schema_version !== 'ReferenceSurfaceAnalysis@1'
      || typeof analysis.analysis_id !== 'string'
      || analysis.rebuild_plan_id !== plan.rebuild_plan_id
      || analysis.evidence_id !== plan.evidence_id
      || analysis.domain_pack_id !== plan.domain_pack_id
      || analysis.source_object_sha256 !== pair.source_object_sha256
      || !isReferenceSha256(analysis.source_object_sha256)
      || typeof analysis.fidelity_ceiling !== 'string'
      || ![
        'single_image_visible_surface_only',
        'multi_view_image_visible_surface_only',
        'strict_glb_readback_visible_bounds_only',
      ].includes(analysis.fidelity_ceiling)
      || !Array.isArray(analysis.bindings)
    ) return null
  }

  // Rust serializes absent optional plan fields by omitting them, while the
  // explicit result-pair projection always uses JSON null for an absent value.
  const confirmedAssetVersionId = plan.confirmed_asset_version_id ?? null
  if (!isNullableReferenceId(confirmedAssetVersionId) || pair.result_asset_version_id !== confirmedAssetVersionId) return null
  if (plan.status === 'confirmed') {
    if (pair.result_asset_version_id === null || pair.result_glb_sha256 === null || pair.result_glb_sha256 === pair.source_object_sha256) return null
  } else if (pair.result_asset_version_id !== null || pair.result_glb_sha256 !== null) {
    return null
  }
  return value as ReferenceGuidedRebuildPlanRead
}

/**
 * Projects the POST payload through the same sealed plan-read validator as a
 * persisted GET.  A draft can only have a null result pair; the source hash is
 * copied from Rust's frozen analysis solely to express that absence.  This is
 * intentionally stricter than treating the response as an arbitrary
 * `AgentAssetChangeSet`, because a missing analysis must stop the workflow
 * before a preview GLB can be displayed.
 */
export function projectReferenceGuidedRebuildPreviewProposal(value: unknown): ReferenceGuidedRebuildPreviewProposal | null {
  if (!isReferenceRecord(value)) return null
  const changeSet = value
  const plan = value.reference_guided_rebuild_plan
  const analysis = value.reference_surface_analysis
  if (
    !isReferenceRecord(plan)
    || !isReferenceRecord(analysis)
    || typeof changeSet.change_set_id !== 'string'
    || typeof changeSet.project_id !== 'string'
    || typeof changeSet.base_asset_version_id !== 'string'
    || changeSet.status !== 'proposed'
    || !Array.isArray(changeSet.operations)
    || plan.status !== 'draft'
    || plan.project_id !== changeSet.project_id
    || plan.base_asset_version_id !== changeSet.base_asset_version_id
    || typeof analysis.source_object_sha256 !== 'string'
  ) return null

  const planRead = projectReferenceGuidedRebuildPlanRead({
    schema_version: 'ReferenceGuidedRebuildPlanRead@1',
    reference_guided_rebuild_plan: plan,
    reference_surface_analysis: analysis,
    reference_result_pair: {
      source_object_sha256: analysis.source_object_sha256,
      result_asset_version_id: null,
      result_glb_sha256: null,
    },
  })
  if (!planRead || planRead.reference_surface_analysis === null) return null
  return { changeSet: changeSet as AgentAssetChangeSet, planRead }
}

export type ReferenceEvidenceContent = {
  blob: Blob
  mediaType: 'image/png' | 'image/jpeg' | 'image/webp' | 'model/gltf-binary'
  fileName: string
}

export type ActiveDesignErrorKind =
  | 'stale'
  | 'legacy_read_only'
  | 'not_found'
  | 'invalid'
  | 'idempotency_conflict'
  | 'unknown'

export type ActiveDesignErrorState = {
  kind: ActiveDesignErrorKind
  message: string
  shouldReloadSnapshot: boolean
  assetChanged: false
}

/**
 * Stable, presentation-safe error mapping for the future workbench reducer.
 * This module does not decide when to render an error or mutate a Snapshot.
 */
export function mapActiveDesignError(error: unknown): ActiveDesignErrorState {
  if (!(error instanceof ForgeApiError)) {
    return {
      kind: 'unknown',
      message: '暂时无法读取当前设计，模型没有被此操作修改。请稍后重试。',
      shouldReloadSnapshot: false,
      assetChanged: false,
    }
  }
  switch (error.code) {
    case 'ACTIVE_DESIGN_STALE':
      return {
        kind: 'stale',
        message: '当前设计已在别处更新。已保留模型，请刷新后再继续。',
        shouldReloadSnapshot: true,
        assetChanged: false,
      }
    case 'ACTIVE_DESIGN_LEGACY_READ_ONLY':
      return {
        kind: 'legacy_read_only',
        message: '这是旧版只读设计。请先让 Agent 重建设计资产，再进行部件调整。',
        shouldReloadSnapshot: false,
        assetChanged: false,
      }
    case 'PROJECT_NOT_FOUND':
    case 'ACTIVE_DESIGN_NOT_FOUND':
      return {
        kind: 'not_found',
        message: '未找到可打开的设计。请先创建或打开一个项目。',
        shouldReloadSnapshot: false,
        assetChanged: false,
      }
    case 'ACTIVE_DESIGN_INVALID':
    case 'ACTIVE_DESIGN_HEAD_INVALID':
    case 'ACTIVE_DESIGN_LEGACY_INVALID':
      return {
        kind: 'invalid',
        message: '当前设计信息不完整，模型没有被修改。请重新打开项目或联系维护人员。',
        shouldReloadSnapshot: true,
        assetChanged: false,
      }
    case 'IDEMPOTENCY_CONFLICT':
      return {
        kind: 'idempotency_conflict',
        message: '这一步已用不同内容提交过。模型没有被本次操作修改，请刷新后重试。',
        shouldReloadSnapshot: true,
        assetChanged: false,
      }
    default:
      return {
        kind: 'unknown',
        message: error.message || '暂时无法完成此操作，模型没有被此操作修改。请稍后重试。',
        shouldReloadSnapshot: error.status === 409,
        assetChanged: false,
      }
  }
}

export class ForgeApiClient {
  private readonly nativeTurnCancellations = new Map<string, NativeTurnCancellation>()
  private readonly nativeProviderCancellations = new Map<string, NativeProviderCancellation>()
  private readonly nativeApprovalParents = new Map<string, { threadId: string; turnId: string }>()

  constructor(private baseUrl = DEFAULT_BASE_URL) {
    appServerTransport.configureBrowserBaseUrl(baseUrl)
  }

  getBaseUrl(): string {
    return this.baseUrl
  }

  setBaseUrl(baseUrl: string): void {
    this.baseUrl = baseUrl.replace(/\/$/, '')
    appServerTransport.configureBrowserBaseUrl(this.baseUrl)
  }

  async checkHealth(): Promise<HealthResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/health`)
    return readJson<HealthResponse>(response)
  }

  async createAgentThread(input: CreateAgentThreadRequest): Promise<AgentThreadDetail> {
    if (isNativeDesktopRuntime()) {
      const commandId = nextNativeCommandId('thread_create')
      const raw = await appServerTransport.nativeRequest<unknown>('thread/create', {
        schema_version: 'AgentThreadCommand@1',
        command_id: commandId,
        command: { operation: 'create', request: input },
      })
      const thread = readNativeThreadResult(raw, commandId, 'thread')
      this.rememberNativeThread(thread)
      return thread
    }
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/threads`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<AgentThreadDetail>(response)
  }

  async listAgentThreads(): Promise<AgentThreadListResponse> {
    if (isNativeDesktopRuntime()) {
      const commandId = nextNativeCommandId('thread_list')
      const raw = await appServerTransport.nativeRequest<unknown>('thread/list', {
        schema_version: 'AgentThreadCommand@1',
        command_id: commandId,
        command: { operation: 'list', include_archived: false, limit: NATIVE_LIST_LIMIT },
      }, { retrySafe: true })
      return { items: readNativeThreadListResult(raw, commandId), next_cursor: null }
    }
    return readJson<AgentThreadListResponse>(await protocolRequest(`${this.baseUrl}/api/v1/agent/threads`))
  }

  async listAgentDomainPacks(): Promise<DomainPackManifest[]> {
    return readJson<DomainPackManifest[]>(await protocolRequest(`${this.baseUrl}/api/v1/agent/domain-packs`))
  }

  async listAgentMaterials(): Promise<AgentMaterialPreset[]> {
    return readJson<AgentMaterialPreset[]>(await protocolRequest(`${this.baseUrl}/api/v1/agent/materials`))
  }

  async listAgentMaterialTextures(params: { texture_role?: string; source?: string; q?: string } = {}): Promise<AgentMaterialTextureListResponse> {
    const query = new URLSearchParams()
    if (params.texture_role) query.set('texture_role', params.texture_role)
    if (params.source) query.set('source', params.source)
    if (params.q) query.set('q', params.q)
    const suffix = query.toString() ? `?${query.toString()}` : ''
    return readJson<AgentMaterialTextureListResponse>(await protocolRequest(`${this.baseUrl}/api/v1/agent/material-textures${suffix}`))
  }

  async registerAgentMaterialTexture(input: RegisterAgentMaterialTextureRequest, idempotencyKey: string): Promise<AgentMaterialTextureObject> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/material-textures`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Idempotency-Key': idempotencyKey },
      body: JSON.stringify(input),
    })
    return readJson<AgentMaterialTextureObject>(response)
  }

  async checkAgentProvider(checkId?: string): Promise<AgentProviderCheckResponse> {
    if (isNativeDesktopRuntime()) return this.checkNativeAgentProvider(checkId)
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/provider:check`, {
      method: 'POST',
      headers: checkId ? { 'X-Provider-Check-Id': checkId } : undefined,
    })
    return readJson<AgentProviderCheckResponse>(response)
  }

  async cancelAgentProviderCheck(checkId: string): Promise<{ check_id: string; cancel_requested: boolean }> {
    if (isNativeDesktopRuntime()) {
      const capability = this.nativeProviderCancellations.get(checkId)
      if (!capability) {
        throw nativeApiError(
          'PROVIDER_CANCELLATION_NOT_AVAILABLE',
          '当前模型检查的取消凭据已不在本次桌面会话中。',
          true,
        )
      }
      const raw = await appServerTransport.nativeRequest<unknown>('provider/cancel', {
        schema_version: 'ProviderCancelCommand@1',
        execution_id: capability.executionId,
        cancellation_id: capability.cancellationId,
        cancellation_token: capability.cancellationToken,
      })
      const result = readNativeProviderCancelResult(raw, capability)
      if (result.accepted || result.alreadyTerminal) this.nativeProviderCancellations.delete(checkId)
      return { check_id: checkId, cancel_requested: result.accepted }
    }
    return readJson<{ check_id: string; cancel_requested: boolean }>(
      await protocolRequest(`${this.baseUrl}/api/v1/agent/provider-checks/${encodeURIComponent(checkId)}/cancel`, { method: 'POST' }),
    )
  }

  async buildAgentBlockout(input: BuildAgentBlockoutRequest): Promise<BuildAgentBlockoutResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/blockouts`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<BuildAgentBlockoutResponse>(response)
  }

  async renderAgentBlockoutConceptPreview(input: RenderAgentBlockoutConceptPreviewRequest): Promise<AgentBlockoutConceptPreview> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/blockouts:concept-preview`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(input),
    })
    return readJson<AgentBlockoutConceptPreview>(response)
  }

  async segmentAgentBlockout(input: SegmentAgentBlockoutRequest): Promise<SegmentAgentBlockoutResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/blockouts:segment`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<SegmentAgentBlockoutResponse>(response)
  }

  async commitAgentBlockout(input: CommitAgentBlockoutRequest): Promise<AgentAssetVersion> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/blockouts:commit`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<AgentAssetVersion>(response)
  }

  async getAgentAssetVersion(assetVersionId: string): Promise<AgentAssetVersion> {
    return readJson<AgentAssetVersion>(await protocolRequest(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}`))
  }

  async importAgentGlb(input: ImportAgentGlbRequest): Promise<ImportAgentGlbResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/imports:glb`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<ImportAgentGlbResponse>(response)
  }

  async createReferenceEvidence(input: ReferenceEvidenceCreateRequest): Promise<ReferenceEvidenceCreateResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/reference-evidence:create`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify({ schema_version: 'ReferenceEvidenceCreateRequest@1', ...input }),
    })
    return readJson<ReferenceEvidenceCreateResponse>(response)
  }

  /**
   * Returns only metadata and immutable plan identities for the current
   * Project. The response deliberately has no object path or arbitrary CAS
   * lookup key.
   */
  async listProjectReferenceEvidence(projectId: string): Promise<ReferenceEvidenceProjectRead> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/projects/${encodeURIComponent(projectId)}/reference-evidence`,
      { cache: 'no-store' },
    )
    return readJson<ReferenceEvidenceProjectRead>(response)
  }

  /** Reads one persisted comparison record, including the frozen R007B analysis when present. */
  async getReferenceGuidedRebuildPlan(projectId: string, rebuildPlanId: string): Promise<ReferenceGuidedRebuildPlanRead> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/projects/${encodeURIComponent(projectId)}/reference-guided-rebuild-plans/${encodeURIComponent(rebuildPlanId)}`,
      { cache: 'no-store' },
    )
    const projected = projectReferenceGuidedRebuildPlanRead(await readJson<unknown>(response))
    if (!projected) {
      throw new ForgeApiError('Reference rebuild plan response does not satisfy the sealed R007B projection', 'REFERENCE_REBUILD_PLAN_READ_INVALID', false, {}, 502)
    }
    return projected
  }

  /**
   * Reads the sealed source bytes only through its Project/evidence identity.
   * Neither callers nor URLs can supply an object hash or filesystem path.
   */
  async loadReferenceEvidenceContent(projectId: string, evidenceId: string): Promise<ReferenceEvidenceContent> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/projects/${encodeURIComponent(projectId)}/reference-evidence/${encodeURIComponent(evidenceId)}:content`,
      { headers: { Accept: 'image/png,image/jpeg,image/webp,model/gltf-binary' }, cache: 'no-store' },
    )
    if (!response.ok) await readJson<never>(response)
    const mediaType = response.headers.get('content-type')?.split(';')[0]
    if (mediaType !== 'image/png' && mediaType !== 'image/jpeg' && mediaType !== 'image/webp' && mediaType !== 'model/gltf-binary') {
      throw new ForgeApiError('Reference evidence returned an unsupported media type', 'REFERENCE_EVIDENCE_MEDIA_TYPE_INVALID', false, {}, 502)
    }
    const disposition = response.headers.get('content-disposition') ?? ''
    const fileName = disposition.match(/filename=\"?([^\";]+)\"?/i)?.[1] || `${evidenceId}.${mediaType === 'model/gltf-binary' ? 'glb' : 'image'}`
    const blob = await response.blob()
    if (blob.size === 0) {
      throw new ForgeApiError('Reference evidence is empty', 'REFERENCE_EVIDENCE_EMPTY', false, {}, 502)
    }
    return { blob, mediaType, fileName }
  }

  async proposeReferenceGuidedRebuildPreview(
    projectId: string,
    input: {
      client_request_id: string
      evidence_id: string
      domain_pack_id: string
      base_asset_version_id: string
    },
  ): Promise<ReferenceGuidedRebuildPreviewProposal> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/projects/${encodeURIComponent(projectId)}/reference-guided-rebuild:preview`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': input.client_request_id,
        },
        body: JSON.stringify({ schema_version: 'ReferenceGuidedRebuildPreviewRequest@1', ...input }),
      },
    )
    const projected = projectReferenceGuidedRebuildPreviewProposal(await readJson<unknown>(response))
    if (!projected) {
      throw new ForgeApiError(
        'Reference rebuild preview response does not satisfy the sealed R007B projection',
        'REFERENCE_REBUILD_PREVIEW_PROPOSAL_INVALID',
        false,
        {},
        502,
      )
    }
    return projected
  }

  async qualityAgentAssetVersion(
    assetVersionId: string,
    input: { idempotencyKey: string; ifMatch: string },
  ): Promise<AgentAssetQualityReport> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:quality`, {
      method: 'POST',
      headers: {
        'Idempotency-Key': input.idempotencyKey,
        'If-Match': input.ifMatch,
      },
    })
    return readJson<AgentAssetQualityReport>(response)
  }

  async getAgentQualityReport(qualityReportId: string): Promise<AgentAssetQualityReport> {
    return readJson<AgentAssetQualityReport>(
      await protocolRequest(`${this.baseUrl}/api/v1/agent/quality-reports/${encodeURIComponent(qualityReportId)}`),
    )
  }

  async saveAgentComponent(assetVersionId: string, input: SaveAgentComponentRequest): Promise<AgentComponentRecord> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/components`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Idempotency-Key': input.client_request_id },
      body: JSON.stringify(input),
    })
    return readJson<AgentComponentRecord>(response)
  }

  async exportAgentAssetGlb(assetVersionId: string): Promise<AgentAssetExportResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:export`, { method: 'POST' })
    return readJson<AgentAssetExportResponse>(response)
  }

  async loadAgentAssetPreviewGlb(assetVersionId: string): Promise<AgentAssetGlbBinary> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:preview.glb`,
      { headers: { Accept: 'model/gltf-binary' }, cache: 'no-store' },
    )
    return readAgentAssetGlbBinary(response)
  }

  async loadSingleResultPreviewGlb(input: SingleResultPreviewIdentity): Promise<SingleResultPreviewGlb> {
    const response = await protocolRequest(singleResultRoute(this.baseUrl, input, ':preview.glb'), {
      headers: {
        Accept: 'model/gltf-binary',
        'If-Match': singleResultArtifactEtag(input.artifactSha256),
        'Cache-Control': 'no-store',
      },
      cache: 'no-store',
    })
    const binary = await readAgentAssetGlbBinary(response)
    const projectId = response.headers.get('x-forgecad-project-id')
    const turnId = response.headers.get('x-forgecad-turn-id')
    const previewId = response.headers.get('x-forgecad-preview-id')
    if (
      projectId !== input.projectId
      || turnId !== input.turnId
      || previewId !== input.previewId
      || binary.glbSha256 !== input.artifactSha256
      || binary.artifactProfileId !== input.artifactProfileId
    ) {
      throw new ForgeApiError('Single-result preview identity does not match its sealed decision', 'SINGLE_RESULT_PREVIEW_IDENTITY_MISMATCH', false, {}, 502)
    }
    return { ...binary, projectId, turnId, previewId }
  }

  async confirmSingleResultPreview(
    input: SingleResultPreviewIdentity & { clientRequestId: string; summary: string },
  ): Promise<AgentAssetVersion> {
    const response = await protocolRequest(singleResultRoute(this.baseUrl, input, ':confirm'), {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.clientRequestId,
        'If-Match': singleResultArtifactEtag(input.artifactSha256),
      },
      body: JSON.stringify({
        client_request_id: input.clientRequestId,
        expected_artifact_sha256: input.artifactSha256,
        summary: input.summary,
      }),
    })
    return readJson<AgentAssetVersion>(response)
  }

  async rejectSingleResultPreview(
    input: SingleResultPreviewIdentity & { clientRequestId: string },
  ): Promise<SingleResultRejectResponse> {
    const response = await protocolRequest(singleResultRoute(this.baseUrl, input, ':reject'), {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.clientRequestId,
        'If-Match': singleResultArtifactEtag(input.artifactSha256),
      },
      body: JSON.stringify({
        client_request_id: input.clientRequestId,
        expected_artifact_sha256: input.artifactSha256,
      }),
    })
    return readJson<SingleResultRejectResponse>(response)
  }

  async loadAgentAssetProductionGlb(assetVersionId: string): Promise<AgentAssetGlbBinary> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:model.glb`,
      { headers: { Accept: 'model/gltf-binary' }, cache: 'no-store' },
    )
    return readAgentAssetGlbBinary(response)
  }

  async downloadAgentAssetProductionGlb(assetVersionId: string): Promise<AgentAssetGlbDownload> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:model.glb`,
      { headers: { Accept: 'model/gltf-binary' }, cache: 'no-store' },
    )
    if (!response.ok) await readJson<never>(response)
    const metadata = readAgentAssetGlbHeaders(response)
    const disposition = response.headers.get('content-disposition') ?? ''
    const filename = disposition.match(/filename="?([^";]+)"?/i)?.[1]
      || `${assetVersionId}.glb`
    const blob = await response.blob()
    if (blob.size !== metadata.byteSize || blob.size === 0) {
      throw new ForgeApiError('Production concept GLB byte size does not match its readback headers', 'MODEL_GLB_SIZE_MISMATCH', false, {}, 502)
    }
    return { ...metadata, blob, filename }
  }

  async renderAgentAssetViews(
    assetVersionId: string,
    options: { width?: number; height?: number } = {},
  ): Promise<AgentAssetRenderSet> {
    const params = new URLSearchParams()
    if (options.width !== undefined) params.set('width', String(options.width))
    if (options.height !== undefined) params.set('height', String(options.height))
    const query = params.toString()
    return readJson<AgentAssetRenderSet>(
      await protocolRequest(
        `${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:render${query ? `?${query}` : ''}`,
      ),
    )
  }

  async downloadAgentAssetRenderPackage(
    assetVersionId: string,
    renderSet: Pick<AgentAssetRenderSet, 'width' | 'height' | 'render_set_sha256'>,
  ): Promise<AgentRenderPackageDownload> {
    const params = new URLSearchParams({
      width: String(renderSet.width),
      height: String(renderSet.height),
      render_set_sha256: renderSet.render_set_sha256,
    })
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:render-package?${params.toString()}`,
      { cache: 'no-store' },
    )
    if (!response.ok) await readJson<never>(response)
    const disposition = response.headers.get('content-disposition') ?? ''
    const filename = disposition.match(/filename="?([^";]+)"?/i)?.[1]
      || `${assetVersionId}-concept-views.zip`
    return {
      blob: await response.blob(),
      filename,
      renderSetSha256: response.headers.get('x-forgecad-render-set-sha256'),
    }
  }

  async listAgentComponents(projectId: string, filters: { domain_pack_id?: string; role?: string; q?: string } = {}): Promise<AgentComponentRecord[]> {
    const params = new URLSearchParams({ project_id: projectId })
    Object.entries(filters).forEach(([key, value]) => value && params.set(key, value))
    return readJson<AgentComponentRecord[]>(await protocolRequest(`${this.baseUrl}/api/v1/agent/components?${params.toString()}`))
  }

  async listAgentComponentCandidates(assetVersionId: string, partId: string): Promise<AgentComponentCandidate[]> {
    const params = new URLSearchParams({ part_id: partId })
    return readJson<AgentComponentCandidate[]>(
      await protocolRequest(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/components:compatible?${params.toString()}`),
    )
  }

  async listAgentStructureSuggestions(assetVersionId: string): Promise<AgentStructureSuggestionList> {
    return readJson<AgentStructureSuggestionList>(
      await protocolRequest(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/structure-suggestions`),
    )
  }

  async listAgentSemanticProportions(assetVersionId: string, partId: string): Promise<ResolvedSemanticProportionOptions> {
    return readJson<ResolvedSemanticProportionOptions>(
      await protocolRequest(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/parts/${encodeURIComponent(partId)}/semantic-proportions`),
    )
  }

  async proposeAgentAssetChangeSet(
    assetVersionId: string,
    input: ProposeAgentAssetChangeSetRequest,
  ): Promise<AgentAssetChangeSet> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/change-sets`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<AgentAssetChangeSet>(response)
  }

  async enableSurfaceAdornmentSkill(clientRequestId: string): Promise<{ status: 'enabled' }> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/skills/surface-adornment:enable`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': clientRequestId,
      },
      body: JSON.stringify({
        schema_version: 'EnableSurfaceAdornmentSkillRequest@1',
        client_request_id: clientRequestId,
        confirm_enable: true,
      }),
    })
    return readJson<{ status: 'enabled' }>(response)
  }

  async proposeSurfaceAdornmentPreview(
    assetVersionId: string,
    input: {
      client_request_id: string
      part_id: string
      material_zone_id: string
      kind: 'normal_relief' | 'pattern' | 'flowline' | 'micro_surface'
      motif: 'parallel_groove' | 'chevron_relief' | 'hex_microgrid' | 'double_flowline'
      intensity: 'subtle' | 'balanced' | 'pronounced'
      coverage: 'full_zone' | 'center_band' | 'edge_band' | 'symmetric_pair'
    },
  ): Promise<AgentAssetChangeSet> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/surface-adornments:preview`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': input.client_request_id,
        },
        body: JSON.stringify({
          schema_version: 'SurfaceAdornmentPreviewRequest@1',
          ...input,
        }),
      },
    )
    return readJson<AgentAssetChangeSet>(response)
  }

  async previewAgentAssetChangeSet(changeSetId: string, idempotencyKey: string): Promise<AgentAssetChangeSet> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:preview`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<AgentAssetChangeSet>(response)
  }

  async exportAgentAssetChangeSetPreviewGlb(changeSetId: string): Promise<AgentAssetChangeSetPreviewGlb> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:preview.glb`,
      { headers: { Accept: 'model/gltf-binary' }, cache: 'no-store' },
    )
    if (!response.ok) await readJson<never>(response)
    const triangleCountHeader = response.headers.get('X-ForgeCAD-Preview-Triangle-Count')
    const glb = await response.arrayBuffer()
    if (glb.byteLength === 0) {
      throw new ForgeApiError('ChangeSet preview GLB is empty', 'PREVIEW_GLB_EMPTY', false, {}, 502)
    }
    return {
      glb,
      sha256: response.headers.get('X-ForgeCAD-Preview-GLB-SHA256'),
      baseAssetVersionId: response.headers.get('X-ForgeCAD-Base-Asset-Version-ID'),
      triangleCount: triangleCountHeader === null ? null : Number(triangleCountHeader),
    }
  }

  async confirmAgentAssetChangeSet(changeSetId: string, idempotencyKey: string): Promise<AgentAssetChangeSetConfirmResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:confirm`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<AgentAssetChangeSetConfirmResponse>(response)
  }

  async rejectAgentAssetChangeSet(changeSetId: string, idempotencyKey: string): Promise<AgentAssetChangeSet> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:reject`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<AgentAssetChangeSet>(response)
  }

  async getAgentThread(threadId: string): Promise<AgentThreadDetail> {
    if (isNativeDesktopRuntime()) return this.readNativeAgentThread(threadId)
    return readJson<AgentThreadDetail>(
      await protocolRequest(`${this.baseUrl}/api/v1/agent/threads/${encodeURIComponent(threadId)}`),
    )
  }

  async getActiveDesign(projectId: string): Promise<ActiveDesignApiResponse<ActiveDesignSnapshot>> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design`)
    return readJsonWithEtag<ActiveDesignSnapshot>(response)
  }

  async getActiveDesignNavigation(projectId: string): Promise<ActiveDesignNavigation> {
    return readJson<ActiveDesignNavigation>(
      await protocolRequest(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:navigation`),
    )
  }

  async setActiveDesignRenderPreset(
    projectId: string,
    input: SetActiveDesignRenderPresetRequest,
    options: { ifMatch?: string } = {},
  ): Promise<ActiveDesignApiResponse<ActiveDesignSnapshot>> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:render-preset`,
      {
        method: 'POST',
        headers: activeDesignHeaders(input.client_request_id, options.ifMatch),
        body: JSON.stringify(input),
      },
    )
    return readJsonWithEtag<ActiveDesignSnapshot>(response)
  }

  async setActiveDesignPartDisplay(
    projectId: string,
    input: SetActiveDesignPartDisplayRequest,
    options: { ifMatch?: string } = {},
  ): Promise<ActiveDesignApiResponse<ActiveDesignSnapshot>> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:part-display`,
      {
        method: 'POST',
        headers: activeDesignHeaders(input.client_request_id, options.ifMatch),
        body: JSON.stringify(input),
      },
    )
    return readJsonWithEtag<ActiveDesignSnapshot>(response)
  }

  async selectActiveDesignPart(
    projectId: string,
    input: SelectActiveDesignRequest,
    options: { ifMatch?: string } = {},
  ): Promise<ActiveDesignApiResponse<ActiveDesignSnapshot>> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:select`, {
      method: 'POST',
      headers: activeDesignHeaders(input.client_request_id, options.ifMatch),
      body: JSON.stringify(input),
    })
    return readJsonWithEtag<ActiveDesignSnapshot>(response)
  }

  async convertLegacyActiveDesign(
    projectId: string,
    input: ConvertLegacyActiveDesignRequest,
    options: { ifMatch?: string } = {},
  ): Promise<ActiveDesignApiResponse<LegacyActiveDesignConversionResponse>> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:convert-legacy`,
      {
        method: 'POST',
        headers: activeDesignHeaders(input.client_request_id, options.ifMatch),
        body: JSON.stringify(input),
      },
    )
    return readJsonWithEtag<LegacyActiveDesignConversionResponse>(response)
  }

  async undoActiveDesign(
    projectId: string,
    input: NavigateActiveDesignRequest,
    options: { ifMatch?: string } = {},
  ): Promise<ActiveDesignApiResponse<ActiveDesignSnapshot>> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:undo`, {
      method: 'POST',
      headers: activeDesignHeaders(input.client_request_id, options.ifMatch),
      body: JSON.stringify(input),
    })
    return readJsonWithEtag<ActiveDesignSnapshot>(response)
  }

  async redoActiveDesign(
    projectId: string,
    input: NavigateActiveDesignRequest,
    options: { ifMatch?: string } = {},
  ): Promise<ActiveDesignApiResponse<ActiveDesignSnapshot>> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:redo`, {
      method: 'POST',
      headers: activeDesignHeaders(input.client_request_id, options.ifMatch),
      body: JSON.stringify(input),
    })
    return readJsonWithEtag<ActiveDesignSnapshot>(response)
  }

  async startAgentTurn(threadId: string, input: StartAgentTurnRequest): Promise<AgentTurn> {
    if (isNativeDesktopRuntime()) return this.startNativeAgentTurn(threadId, input)
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/threads/${encodeURIComponent(threadId)}/turns`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': input.client_request_id,
        },
        body: JSON.stringify(input),
      },
    )
    return readJson<AgentTurn>(response)
  }

  async cancelAgentTurn(turnId: string, idempotencyKey: string): Promise<AgentTurn> {
    if (isNativeDesktopRuntime()) {
      const capability = this.nativeTurnCancellations.get(turnId)
      if (!capability) {
        throw nativeApiError(
          'TURN_CANCELLATION_NOT_AVAILABLE',
          '当前 Turn 的取消凭据已不在本次桌面会话中，请刷新会话后确认运行状态。',
          true,
        )
      }
      let notificationError: unknown = null
      let pendingWake = false
      let wakeResolver: (() => void) | null = null
      const wake = (): void => {
        pendingWake = true
        wakeResolver?.()
        wakeResolver = null
      }
      const waitForWake = (): Promise<void> => {
        if (pendingWake) {
          pendingWake = false
          return Promise.resolve()
        }
        return new Promise<void>((resolve) => { wakeResolver = resolve })
      }
      // Attach before sending turn/cancel so a terminal notification emitted
      // before the command response cannot be missed. A bounded read poll is
      // still authoritative when notification delivery is delayed or lost.
      const unsubscribeNotifications = appServerTransport.subscribeNotifications((notification) => {
        try {
          if (notification.method === 'stream/resyncRequired') {
            wake()
            return
          }
          const notificationTurnId = readNativeLifecycleNotificationTurnId(
            notification,
            capability.threadId,
          )
          if (notificationTurnId === turnId) wake()
        } catch (error) {
          notificationError = error
          wake()
        }
      })
      try {
        const commandId = nextNativeCommandId('turn_cancel')
        const raw = await appServerTransport.nativeRequest<unknown>('turn/cancel', {
          schema_version: 'AgentTurnCommand@1',
          command_id: commandId,
          command: {
            operation: 'cancel',
            thread_id: capability.threadId,
            turn_id: turnId,
            cancellation_id: capability.cancellationId,
            cancellation_token: capability.cancellationToken,
          },
        })
        readNativeTurnCancellationResult(
          raw,
          commandId,
          capability.threadId,
          turnId,
          capability.cancellationId,
        )

        const deadline = Date.now() + NATIVE_TURN_WAIT_MS
        let authoritative = await this.readNativeAgentTurn(capability.threadId, turnId)
        while (!isNativeTurnTerminal(authoritative)) {
          if (notificationError) throw notificationError
          const remaining = deadline - Date.now()
          if (remaining <= 0) {
            throw nativeApiError(
              'TURN_CANCELLATION_TIMEOUT',
              '取消请求已送达，但等待 Rust Agent Turn 进入可读终态超时；请刷新会话确认最终状态。',
              true,
            )
          }
          await waitForNativeWake(waitForWake(), Math.min(remaining, NATIVE_TURN_READ_POLL_MS))
          if (notificationError) throw notificationError
          authoritative = await this.readNativeAgentTurn(capability.threadId, turnId)
        }
        this.nativeTurnCancellations.delete(turnId)
        this.rememberNativeTurn(authoritative)
        return authoritative
      } finally {
        wake()
        unsubscribeNotifications()
      }
    }
    return readJson<AgentTurn>(
      await protocolRequest(`${this.baseUrl}/api/v1/agent/turns/${encodeURIComponent(turnId)}/cancel`, {
        method: 'POST',
        headers: { 'Idempotency-Key': idempotencyKey },
      }),
    )
  }

  async createAgentApproval(
    threadId: string,
    input: CreateAgentApprovalRequest,
  ): Promise<AgentApproval> {
    if (isNativeDesktopRuntime()) {
      const commandId = nextNativeCommandId('approval_create')
      const raw = await appServerTransport.nativeRequest<unknown>('approval/create', {
        schema_version: 'AgentApprovalCommand@1',
        command_id: commandId,
        command: { operation: 'create', thread_id: threadId, request: input },
      })
      const approval = readNativeApprovalResult(raw, commandId)
      this.rememberNativeApproval(approval)
      return approval
    }
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/threads/${encodeURIComponent(threadId)}/approvals`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': input.client_request_id,
        },
        body: JSON.stringify(input),
      },
    )
    return readJson<AgentApproval>(response)
  }

  async resolveAgentApproval(
    approvalId: string,
    input: ResolveAgentApprovalRequest,
  ): Promise<AgentApprovalResolution> {
    if (isNativeDesktopRuntime()) {
      const parent = this.nativeApprovalParents.get(approvalId)
      if (!parent) {
        throw nativeApiError(
          'APPROVAL_PARENT_NOT_AVAILABLE',
          '当前审批的 Thread/Turn 身份尚未载入本次桌面会话，请先刷新该会话。',
          true,
        )
      }
      const commandId = nextNativeCommandId('approval_resolve')
      const raw = await appServerTransport.nativeRequest<unknown>('approval/resolve', {
        schema_version: 'AgentApprovalCommand@1',
        command_id: commandId,
        command: {
          operation: 'resolve',
          thread_id: parent.threadId,
          turn_id: parent.turnId,
          approval_id: approvalId,
          request: input,
        },
      })
      const approval = readNativeApprovalResult(raw, commandId)
      this.rememberNativeApproval(approval)
      const turn = await this.readNativeAgentTurn(parent.threadId, parent.turnId)
      this.rememberNativeTurn(turn)
      return { approval, turn }
    }
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/agent/approvals/${encodeURIComponent(approvalId)}/resolve`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': input.client_request_id,
        },
        body: JSON.stringify(input),
      },
    )
    return readJson<AgentApprovalResolution>(response)
  }

  getAgentEventsUrl(threadId: string, after = 0): string {
    const url = new URL(appServerTransport.resourceUrl(`/api/v1/agent/threads/${encodeURIComponent(threadId)}/events`))
    url.searchParams.set('after', String(after))
    return url.toString()
  }

  subscribeAgentThreadEvents(
    threadId: string,
    handlers: {
      onEvent: (event: AgentEvent) => void
      onOpen?: () => void
      onError?: (error: Event) => void
      onReplayComplete?: () => void
    },
    after = 0,
  ): () => void {
    if (isNativeDesktopRuntime()) {
      return this.subscribeNativeAgentThreadEvents(threadId, handlers, after)
    }
    const path = `/api/v1/agent/threads/${encodeURIComponent(threadId)}/events?after=${encodeURIComponent(String(after))}`
    return appServerTransport.subscribeSse(path, {
      onOpen: handlers.onOpen,
      onEvent: (event, data) => {
        if (event === 'agent.item') handlers.onEvent(JSON.parse(data) as AgentEvent)
        else if (event === 'agent.replay.complete') handlers.onReplayComplete?.()
      },
      onError: (error) => handlers.onError?.(protocolEventError(error)),
    })
  }

  private async checkNativeAgentProvider(checkId?: string): Promise<AgentProviderCheckResponse> {
    const publicCheckId = checkId ?? nextNativeCommandId('provider_check_public')
    const executionId = nextNativeCommandId('provider_check')
    const capability: NativeProviderCancellation = {
      executionId,
      cancellationId: nextNativeCommandId('provider_cancel'),
      cancellationToken: createNativeCancellationToken('provider'),
    }
    this.nativeProviderCancellations.set(publicCheckId, capability)
    try {
      const raw = await appServerTransport.nativeRequest<unknown>('provider/check', {
        schema_version: 'ProviderCheckCommand@1',
        execution_id: executionId,
        provider_id: 'deepseek',
        timeout_ms: 30_000,
        cancellation_id: capability.cancellationId,
        cancellation_token: capability.cancellationToken,
      })
      return mapNativeProviderCheck(readNativeProviderCheckResult(raw, executionId, 'deepseek'))
    } finally {
      this.nativeProviderCancellations.delete(publicCheckId)
    }
  }

  private async readNativeAgentThread(
    threadId: string,
    options: NativeRequestOptions = { retrySafe: true },
  ): Promise<AgentThreadDetail> {
    const commandId = nextNativeCommandId('thread_read')
    const raw = await appServerTransport.nativeRequest<unknown>('thread/read', {
      schema_version: 'AgentThreadCommand@1',
      command_id: commandId,
      command: { operation: 'read', thread_id: threadId },
    }, options)
    const thread = readNativeThreadResult(raw, commandId, 'thread')
    if (thread.thread_id !== threadId) throw nativeContractError('thread/read returned a different thread_id')
    this.rememberNativeThread(thread)
    return thread
  }

  private async readNativeAgentTurn(
    threadId: string,
    turnId: string,
    options: NativeRequestOptions = { retrySafe: true },
  ): Promise<AgentTurn> {
    const commandId = nextNativeCommandId('turn_read')
    const raw = await appServerTransport.nativeRequest<unknown>('turn/read', {
      schema_version: 'AgentTurnCommand@1',
      command_id: commandId,
      command: { operation: 'read', thread_id: threadId, turn_id: turnId },
    }, options)
    const turn = readNativeTurnResult(raw, commandId, 'turn').turn
    if (turn.thread_id !== threadId || turn.turn_id !== turnId) {
      throw nativeContractError('turn/read returned a different thread_id or turn_id')
    }
    this.rememberNativeTurn(turn)
    return turn
  }

  private async listNativeAgentItems(
    threadId: string,
    turnId: string,
    afterSequence: number,
    options: NativeRequestOptions,
  ): Promise<NativeItemList> {
    const commandId = nextNativeCommandId('item_list')
    const raw = await appServerTransport.nativeRequest<unknown>('item/list', {
      schema_version: 'AgentItemCommand@1',
      command_id: commandId,
      command: {
        operation: 'list',
        thread_id: threadId,
        turn_id: turnId,
        after_sequence: afterSequence,
        limit: NATIVE_LIST_LIMIT,
      },
    }, options)
    const result = readNativeItemListResult(raw, commandId)
    for (const item of result.items) {
      if (item.thread_id !== threadId || item.turn_id !== turnId) {
        throw nativeContractError('item/list returned an item outside the requested Thread/Turn')
      }
    }
    return result
  }

  private async replayNativeAgentItems(
    threadId: string,
    after: number,
    signal: AbortSignal,
  ): Promise<AgentEvent[]> {
    const thread = await this.readNativeAgentThread(threadId, { retrySafe: true, signal })
    const events: AgentEvent[] = []
    for (const turn of thread.turns) {
      let cursor = after
      let page = 0
      while (page < NATIVE_REPLAY_MAX_PAGES) {
        const result = await this.listNativeAgentItems(
          threadId,
          turn.turn_id,
          cursor,
          { retrySafe: true, signal },
        )
        for (const item of result.items) {
          events.push({ sequence: item.sequence, thread_id: threadId, turn_id: turn.turn_id, item })
        }
        if (result.nextSequence === null) break
        const last = result.items.at(-1)
        if (!last || last.sequence <= cursor) {
          throw nativeContractError('item/list pagination did not advance its sequence cursor')
        }
        cursor = last.sequence
        page += 1
      }
      if (page >= NATIVE_REPLAY_MAX_PAGES) {
        throw nativeContractError('item/list replay exceeded the bounded page limit')
      }
    }
    return events.sort((left, right) => left.sequence - right.sequence)
  }

  private subscribeNativeAgentThreadEvents(
    threadId: string,
    handlers: {
      onEvent: (event: AgentEvent) => void
      onOpen?: () => void
      onError?: (error: Event) => void
      onReplayComplete?: () => void
    },
    after: number,
  ): () => void {
    const replayAbort = new AbortController()
    const buffered = new Map<number, AgentEvent>()
    const delivered = new Set<string>()
    let replayComplete = false
    let stopped = false

    const deliver = (event: AgentEvent): void => {
      if (stopped || event.sequence < after) return
      const fingerprint = nativeAgentEventFingerprint(event)
      if (delivered.has(fingerprint)) return
      delivered.add(fingerprint)
      handlers.onEvent(event)
    }

    const unsubscribeNotifications = appServerTransport.subscribeNotifications((notification) => {
      if (stopped || notification.method !== 'item/updated') return
      try {
        const event = readNativeItemNotification(notification, threadId)
        if (!event) return
        if (replayComplete) deliver(event)
        else buffered.set(event.sequence, event)
      } catch (error) {
        handlers.onError?.(protocolEventError(error))
      }
    })

    // The replay reads immutable, persisted Thread/Item rows only. A packaged
    // process may report health before the restricted sidecar is ready for one
    // native request, so this one read-only path gets exactly one reconnect +
    // full replay attempt for ADAPTER_UNAVAILABLE. Closed-result/identity
    // failures remain terminal; never retry an accepted but invalid contract.
    const replayFromAuthoritativeStore = async (): Promise<AgentEvent[]> => {
      await appServerTransport.initialize()
      if (stopped) return []
      handlers.onOpen?.()
      return this.replayNativeAgentItems(threadId, after, replayAbort.signal)
    }
    const replayWithBoundedTransientRecovery = async (): Promise<AgentEvent[]> => {
      let firstTransientError: unknown = null
      for (let attempt = 0; attempt <= NATIVE_REPLAY_TRANSIENT_RECOVERY_ATTEMPTS; attempt += 1) {
        try {
          return await replayFromAuthoritativeStore()
        } catch (error) {
          if (
            attempt >= NATIVE_REPLAY_TRANSIENT_RECOVERY_ATTEMPTS
            || !isTransientNativeReplayFailure(error)
            || stopped
          ) {
            // If a one-time adapter recovery exposes a closed contract error,
            // that contract is the actionable terminal cause. Only repeated
            // equivalent adapter transients retain their first stable cause.
            if (!isTransientNativeReplayFailure(error)) throw error
            throw firstTransientError ?? error
          }
          if (firstTransientError === null) firstTransientError = error
          // A fresh initialized Tauri connection is required before the only
          // permitted replay retry. No mutation, cursor advance, or local
          // snapshot is performed between attempts.
          await appServerTransport.reconnect()
        }
      }
      throw firstTransientError ?? new Error('native replay recovery exhausted without a cause')
    }

    void replayWithBoundedTransientRecovery()
      .then((replayed) => {
        if (stopped) return
        const merged = new Map<number, AgentEvent>()
        for (const event of replayed) merged.set(event.sequence, event)
        for (const event of buffered.values()) merged.set(event.sequence, event)
        for (const event of [...merged.values()].sort((left, right) => left.sequence - right.sequence)) {
          deliver(event)
        }
        buffered.clear()
        replayComplete = true
        handlers.onReplayComplete?.()
      })
      .catch((error) => {
        if (!stopped && !(error instanceof DOMException && error.name === 'AbortError')) {
          handlers.onError?.(protocolEventError(error))
        }
      })

    return () => {
      if (stopped) return
      stopped = true
      replayAbort.abort()
      buffered.clear()
      unsubscribeNotifications()
    }
  }

  private async startNativeAgentTurn(threadId: string, input: StartAgentTurnRequest): Promise<AgentTurn> {
    let targetTurnId: string | null = null
    let notificationError: unknown = null
    let pendingWake = false
    let wakeResolver: (() => void) | null = null
    const bufferedTurnIds = new Set<string>()

    const wake = (): void => {
      pendingWake = true
      wakeResolver?.()
      wakeResolver = null
    }
    const waitForWake = (): Promise<void> => {
      if (pendingWake) {
        pendingWake = false
        return Promise.resolve()
      }
      return new Promise<void>((resolve) => { wakeResolver = resolve })
    }

    const unsubscribeNotifications = appServerTransport.subscribeNotifications((notification) => {
      try {
        if (notification.method === 'stream/resyncRequired') {
          wake()
          return
        }
        const notificationTurnId = readNativeLifecycleNotificationTurnId(notification, threadId)
        if (!notificationTurnId) return
        if (targetTurnId === null) bufferedTurnIds.add(notificationTurnId)
        else if (notificationTurnId === targetTurnId) wake()
      } catch (error) {
        notificationError = error
        wake()
      }
    })

    try {
      const commandId = nextNativeCommandId('turn_start')
      const raw = await appServerTransport.nativeRequest<unknown>('turn/start', {
        schema_version: 'AgentTurnCommand@1',
        command_id: commandId,
        command: { operation: 'start', thread_id: threadId, request: input },
      })
      const started = readNativeTurnResult(raw, commandId, 'started')
      if (started.turn.thread_id !== threadId) {
        throw nativeContractError('turn/start returned a Turn for another Thread')
      }
      targetTurnId = started.turn.turn_id
      this.rememberNativeTurn(started.turn)
      if (started.outcome === 'terminal_replay') {
        return started.turn
      }
      this.nativeTurnCancellations.set(started.turn.turn_id, {
        threadId,
        cancellationId: started.cancellationId,
        cancellationToken: started.cancellationToken,
      })
      if (bufferedTurnIds.has(started.turn.turn_id)) wake()

      const deadline = Date.now() + NATIVE_TURN_WAIT_MS
      let authoritative = await this.readNativeAgentTurn(threadId, started.turn.turn_id)
      while (isNativeTurnRunning(authoritative)) {
        if (notificationError) throw notificationError
        const remaining = deadline - Date.now()
        if (remaining <= 0) break
        await waitForNativeWake(waitForWake(), remaining)
        if (notificationError) throw notificationError
        authoritative = await this.readNativeAgentTurn(threadId, started.turn.turn_id)
      }
      if (isNativeTurnRunning(authoritative)) {
        authoritative = await this.readNativeAgentTurn(threadId, started.turn.turn_id)
      }
      if (isNativeTurnRunning(authoritative)) {
        throw nativeApiError(
          'PROVIDER_TIMEOUT',
          '等待 Rust Agent Turn 进入可读终态超时；已保存资产没有因前端等待而改变。',
          true,
        )
      }
      if (isNativeTurnTerminal(authoritative)) {
        this.nativeTurnCancellations.delete(authoritative.turn_id)
      }
      this.rememberNativeTurn(authoritative)
      return authoritative
    } finally {
      wake()
      unsubscribeNotifications()
    }
  }

  private rememberNativeThread(thread: AgentThreadDetail): void {
    for (const turn of thread.turns) this.rememberNativeTurn(turn)
  }

  private rememberNativeTurn(turn: AgentTurn): void {
    for (const approval of turn.approvals) this.rememberNativeApproval(approval)
    if (isNativeTurnTerminal(turn)) this.nativeTurnCancellations.delete(turn.turn_id)
  }

  private rememberNativeApproval(approval: AgentApproval): void {
    this.nativeApprovalParents.set(approval.approval_id, {
      threadId: approval.thread_id,
      turnId: approval.turn_id,
    })
  }

  async listConceptProjects(): Promise<ConceptProjectListResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects`)
    return readJson<ConceptProjectListResponse>(response)
  }

  async getConceptProject(projectId: string): Promise<ConceptProjectDetail> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects/${projectId}`)
    return readJson<ConceptProjectDetail>(response)
  }

  async createConceptProject(input: CreateConceptProjectRequest): Promise<ConceptProjectDetail> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<ConceptProjectDetail>(response)
  }

  async initializeConceptWorkbench(
    projectId: string,
    clientRequestId: string,
  ): Promise<ConceptProjectDetail> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}:initialize-workbench`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': clientRequestId,
        },
        body: JSON.stringify({ client_request_id: clientRequestId }),
      },
    )
    return readJson<ConceptProjectDetail>(response)
  }

  async getConceptVersion(versionId: string): Promise<ConceptVersionDetail> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/versions/${versionId}`)
    return readJson<ConceptVersionDetail>(response)
  }

  async listModuleAssets(
    packId?: string,
    filters?: { query?: string; reviewStatus?: string; tag?: string; catalogPath?: string },
  ): Promise<ModuleAssetListResponse> {
    const url = new URL(`${this.baseUrl}/api/v1/module-assets`)
    if (packId) url.searchParams.set('pack_id', packId)
    if (filters?.query) url.searchParams.set('query', filters.query)
    if (filters?.reviewStatus) url.searchParams.set('review_status', filters.reviewStatus)
    if (filters?.tag) url.searchParams.set('tag', filters.tag)
    if (filters?.catalogPath) url.searchParams.set('catalog_path', filters.catalogPath)
    const response = await protocolRequest(url)
    return readJson<ModuleAssetListResponse>(response)
  }

  async updateModuleAssetCatalogMetadata(
    moduleId: string,
    input: UpdateModuleAssetCatalogMetadataRequest,
  ): Promise<ModuleAssetRecord> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/module-assets/${encodeURIComponent(moduleId)}/catalog-metadata`,
      {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': input.client_request_id,
        },
        body: JSON.stringify(input),
      },
    )
    return readJson<ModuleAssetRecord>(response)
  }

  getModuleAssetFileUrl(moduleId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/module-assets/${encodeURIComponent(moduleId)}/file`)
  }

  getModuleAssetThumbnailUrl(moduleId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/module-assets/${encodeURIComponent(moduleId)}/thumbnail`)
  }

  async getModuleGraph(graphId: string): Promise<ModuleGraphRecord> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/module-graphs/${graphId}`)
    return readJson<ModuleGraphRecord>(response)
  }

  async inspectConceptVersion(
    versionId: string,
    input: InspectConceptVersionRequest,
  ): Promise<QualityRunRecord> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/versions/${versionId}/quality-runs:inspect`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<QualityRunRecord>(response)
  }

  async enqueueConceptQualityInspection(
    versionId: string,
    input: InspectConceptVersionRequest,
  ): Promise<ConceptJobRecord> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/versions/${encodeURIComponent(versionId)}/quality-runs:inspect:enqueue`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Idempotency-Key': input.client_request_id },
      body: JSON.stringify(input),
    })
    return readJson<ConceptJobRecord>(response)
  }

  async getQualityRun(qualityRunId: string): Promise<QualityRunRecord> {
    return readJson<QualityRunRecord>(
      await protocolRequest(`${this.baseUrl}/api/v1/quality-runs/${encodeURIComponent(qualityRunId)}`),
    )
  }

  async getConceptJob(jobId: string): Promise<ConceptJobRecord> {
    return readJson<ConceptJobRecord>(await protocolRequest(`${this.baseUrl}/api/v1/jobs/${encodeURIComponent(jobId)}`))
  }

  async runConceptWorkerOnce(): Promise<ConceptJobRecord | null> {
    return readJson<ConceptJobRecord | null>(await protocolRequest(`${this.baseUrl}/api/v1/concept-jobs/work-once`, { method: 'POST' }))
  }

  async listDesignVariants(projectId: string): Promise<DesignVariantListResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects/${projectId}/variants`)
    return readJson<DesignVariantListResponse>(response)
  }

  async interpretDesignBrief(
    projectId: string,
    input: InterpretDesignBriefRequest,
  ): Promise<DesignBriefRecord> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects/${projectId}/brief:interpret`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<DesignBriefRecord>(response)
  }

  async generateDesignVariants(
    projectId: string,
    input: GenerateDesignVariantsRequest,
  ): Promise<DesignVariantListResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/projects/${projectId}/variants`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<DesignVariantListResponse>(response)
  }

  async selectDesignVariant(
    projectId: string,
    variantId: string,
    input: SelectDesignVariantRequest,
  ): Promise<DesignVariantRecord> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/projects/${projectId}/variants/${variantId}:select`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': input.client_request_id,
        },
        body: JSON.stringify(input),
      },
    )
    return readJson<DesignVariantRecord>(response)
  }

  async createConceptExport(
    versionId: string,
    input: CreateConceptExportRequest,
  ): Promise<ConceptExportRecord> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/versions/${versionId}/exports`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<ConceptExportRecord>(response)
  }

  getConceptExportFileUrl(exportId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/file`)
  }

  getConceptCombinedGlbUrl(exportId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/combined.glb`)
  }

  getConceptCombinedObjUrl(exportId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/combined.obj`)
  }

  getConceptCombinedMtlUrl(exportId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/combined.mtl`)
  }

  getConceptPreviewPngUrl(exportId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/preview.png`)
  }

  getConceptExplodedPngUrl(exportId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/exploded.png`)
  }

  getConceptRenderSetUrl(exportId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/renders.zip`)
  }

  getConceptRenderViewUrl(exportId: string, viewName: 'front' | 'side' | 'top'): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/views/${viewName}.png`)
  }

  getConceptTurntableFrameUrl(exportId: string, frameIndex: number): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/turntable/${frameIndex}.png`)
  }

  getConceptTurntableVideoUrl(exportId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/exports/${encodeURIComponent(exportId)}/turntable.mp4`)
  }

  async proposeChangeSet(
    versionId: string,
    input: ProposeChangeSetRequest,
  ): Promise<DesignChangeSet> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/versions/${versionId}/change-sets`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<DesignChangeSet>(response)
  }

  async proposeConnectorSnap(
    versionId: string,
    input: ProposeConnectorSnapRequest,
  ): Promise<DesignChangeSet> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/versions/${encodeURIComponent(versionId)}/change-sets:connector-snap`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': input.client_request_id,
        },
        body: JSON.stringify(input),
      },
    )
    return readJson<DesignChangeSet>(response)
  }

  async planChangeSet(
    versionId: string,
    input: PlanDesignChangeSetRequest,
  ): Promise<PlannedChangeSetRecord> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/versions/${versionId}/change-sets:plan`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<PlannedChangeSetRecord>(response)
  }

  async listChangeSets(
    projectId: string,
    input: {
      cursor?: string
      limit?: number
      q?: string
      status?: 'proposed' | 'previewed' | 'confirmed' | 'rejected' | 'stale'
      operation?: 'add_module' | 'remove_module' | 'replace_module' | 'connect' | 'disconnect'
        | 'set_transform' | 'set_mirror' | 'set_style' | 'set_parameter'
    } = {},
  ): Promise<ChangeSetTimelineResponse> {
    const url = new URL(`${this.baseUrl}/api/v1/projects/${projectId}/change-sets`)
    if (input.cursor) url.searchParams.set('cursor', input.cursor)
    if (input.limit) url.searchParams.set('limit', String(input.limit))
    if (input.q) url.searchParams.set('q', input.q)
    if (input.status) url.searchParams.set('status', input.status)
    if (input.operation) url.searchParams.set('operation', input.operation)
    const response = await protocolRequest(url)
    return readJson<ChangeSetTimelineResponse>(response)
  }

  async createChangeSetAuditExport(
    projectId: string,
    input: CreateChangeSetAuditExportRequest,
  ): Promise<ChangeSetAuditExportRecord> {
    const response = await protocolRequest(
      `${this.baseUrl}/api/v1/projects/${projectId}/change-set-audit-exports`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Idempotency-Key': input.client_request_id,
        },
        body: JSON.stringify(input),
      },
    )
    return readJson<ChangeSetAuditExportRecord>(response)
  }

  async listChangeSetAuditExports(
    projectId: string,
    limit = 20,
  ): Promise<ChangeSetAuditExportListResponse> {
    const url = new URL(
      `${this.baseUrl}/api/v1/projects/${projectId}/change-set-audit-exports`,
    )
    url.searchParams.set('limit', String(limit))
    const response = await protocolRequest(url)
    return readJson<ChangeSetAuditExportListResponse>(response)
  }

  getChangeSetAuditExportFileUrl(auditExportId: string): string {
    return appServerTransport.resourceUrl(`/api/v1/change-set-audit-exports/${encodeURIComponent(auditExportId)}/file`)
  }

  async previewChangeSet(changeSetId: string, idempotencyKey: string): Promise<ChangeSetPreviewResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/change-sets/${changeSetId}:preview`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<ChangeSetPreviewResponse>(response)
  }

  async confirmChangeSet(changeSetId: string, idempotencyKey: string): Promise<ChangeSetConfirmResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/change-sets/${changeSetId}:confirm`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<ChangeSetConfirmResponse>(response)
  }

  async rejectChangeSet(changeSetId: string, idempotencyKey: string): Promise<DesignChangeSet> {
    const response = await protocolRequest(`${this.baseUrl}/api/v1/change-sets/${changeSetId}:reject`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<DesignChangeSet>(response)
  }

  async createWeapon(input: CreateWeaponRequest): Promise<CreateWeaponResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<CreateWeaponResponse>(response)
  }

  async listWeapons(): Promise<WeaponSummary[]> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons`)
    const data = await readJson<{ items: WeaponSummary[] }>(response)
    return data.items
  }

  async getWeapon(weaponId: string): Promise<WeaponDetail> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons/${weaponId}`)
    return readJson<WeaponDetail>(response)
  }

  async createInterpretation(weaponId: string, input: CreativeInterpretationRequest): Promise<CreativeInterpretationResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons/${weaponId}/interpretation`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<CreativeInterpretationResponse>(response)
  }

  async confirmCreativeRecast(weaponId: string, input: CreativeRecastConfirmRequest): Promise<CreativeRecastConfirmResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons/${weaponId}/recast/confirm`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<CreativeRecastConfirmResponse>(response)
  }

  async getCreativeGraph(weaponId: string): Promise<CreativeGraphResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons/${weaponId}/creative-graph`)
    return readJson<CreativeGraphResponse>(response)
  }

  async getAssetMetadata(assetId: string): Promise<AssetFileResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/assets/${assetId}`)
    return readJson<AssetFileResponse>(response)
  }

  getAssetFileUrl(assetId: string): string {
    return appServerTransport.resourceUrl(`/api/assets/${assetId}/file`)
  }

  async revealAsset(assetId: string, input: { dryRun?: boolean } = {}): Promise<AssetRevealResponse> {
    const url = new URL(`${this.baseUrl}/api/assets/${assetId}/reveal`)
    if (input.dryRun) url.searchParams.set('dry_run', 'true')
    const response = await protocolRequest(url, {
      method: 'POST',
    })
    return readJson<AssetRevealResponse>(response)
  }

  async getJob(jobId: string): Promise<JobDetail> {
    const response = await protocolRequest(`${this.baseUrl}/api/jobs/${jobId}`)
    return readJson<JobDetail>(response)
  }

  async listJobs(input: {
    query?: string
    status?: string
    jobType?: string
    errorCode?: string
    cursor?: string
    limit?: number
  } = {}): Promise<JobListResponse> {
    const url = new URL(`${this.baseUrl}/api/jobs`)
    if (input.query?.trim()) url.searchParams.set('query', input.query.trim())
    if (input.status) url.searchParams.set('status', input.status)
    if (input.jobType) url.searchParams.set('job_type', input.jobType)
    if (input.errorCode) url.searchParams.set('error_code', input.errorCode)
    if (input.cursor) url.searchParams.set('cursor', input.cursor)
    if (input.limit) url.searchParams.set('limit', String(input.limit))
    const response = await protocolRequest(url)
    return readJson<JobListResponse>(response)
  }

  async getJobRuntime(jobId: string): Promise<JobRuntimeStateResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/jobs/${jobId}/runtime`)
    return readJson<JobRuntimeStateResponse>(response)
  }

  async listJobActions(jobId: string, input: { cursor?: string; limit?: number } = {}): Promise<JobActionListResponse> {
    const url = new URL(`${this.baseUrl}/api/jobs/${jobId}/actions`)
    if (input.cursor) url.searchParams.set('cursor', input.cursor)
    if (input.limit) url.searchParams.set('limit', String(input.limit))
    const response = await protocolRequest(url)
    return readJson<JobActionListResponse>(response)
  }

  async recoverRuntime(): Promise<RuntimeRecoveryResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/runtime/recover`, {
      method: 'POST',
    })
    return readJson<RuntimeRecoveryResponse>(response)
  }

  async workOnce(): Promise<RuntimeWorkOnceResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/runtime/work-once`, {
      method: 'POST',
    })
    return readJson<RuntimeWorkOnceResponse>(response)
  }

  async retryJob(jobId: string): Promise<JobActionResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/jobs/${jobId}/retry`, {
      method: 'POST',
    })
    return readJson<JobActionResponse>(response)
  }

  async retryJobFromStep(jobId: string, stepName: string): Promise<JobActionResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/jobs/${jobId}/retry-from/${encodeURIComponent(stepName)}`, {
      method: 'POST',
    })
    return readJson<JobActionResponse>(response)
  }

  async cancelJob(jobId: string): Promise<JobActionResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/jobs/${jobId}/cancel`, {
      method: 'POST',
    })
    return readJson<JobActionResponse>(response)
  }

  async uploadVersionAsset(weaponId: string, versionId: string, input: AssetUploadRequest): Promise<AssetUploadResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons/${weaponId}/versions/${versionId}/assets`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<AssetUploadResponse>(response)
  }

  async activateVersion(weaponId: string, versionId: string): Promise<WeaponDetail> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons/${weaponId}/versions/${versionId}/activate`, {
      method: 'POST',
    })
    return readJson<WeaponDetail>(response)
  }

  async patchWeapon(weaponId: string, input: PatchWeaponRequest): Promise<CreateWeaponResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons/${weaponId}/patch`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<CreateWeaponResponse>(response)
  }

  async generateRough3D(weaponId: string, input: Generate3DRequest): Promise<CreateWeaponResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons/${weaponId}/generate-3d`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<CreateWeaponResponse>(response)
  }

  async exportUnityPackage(weaponId: string, input: ExportUnityRequest): Promise<CreateWeaponResponse> {
    const response = await protocolRequest(`${this.baseUrl}/api/weapons/${weaponId}/export-unity`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<CreateWeaponResponse>(response)
  }

  async listProviders(): Promise<ProviderSettings[]> {
    const response = await protocolRequest(`${this.baseUrl}/api/provider-settings`)
    const data = await readJson<{ providers: ProviderSettings[] }>(response)
    return data.providers
  }

  subscribeJobEvents(
    jobId: string,
    handlers: {
      onEvent: (event: JobEvent) => void
      onOpen?: () => void
      onError?: (error: Event) => void
      onStreamError?: (error: ApiErrorEnvelope['error']) => void
    },
    after?: string
  ): () => void {
    const suffix = after ? `?after=${encodeURIComponent(after)}` : ''
    return appServerTransport.subscribeSse(`/api/jobs/${encodeURIComponent(jobId)}/events${suffix}`, {
      onOpen: handlers.onOpen,
      onEvent: (event, data) => {
        if (event === 'job.event') handlers.onEvent(JSON.parse(data) as JobEvent)
        if (event === 'job.error') {
          const parsed = JSON.parse(data) as ApiErrorEnvelope
          if (parsed.error) handlers.onStreamError?.(parsed.error)
        }
      },
      onError: (error) => handlers.onError?.(protocolEventError(error)),
    })
  }
}

let nativeCommandSequence = 0

function nextNativeCommandId(scope: string): string {
  nativeCommandSequence = (nativeCommandSequence + 1) % Number.MAX_SAFE_INTEGER
  return `desktop_${scope}_${Date.now().toString(36)}_${nativeCommandSequence.toString(36)}`
}

function createNativeCancellationToken(scope: string): string {
  if (!globalThis.crypto?.getRandomValues) {
    throw nativeApiError(
      'NATIVE_SECURE_RANDOM_UNAVAILABLE',
      '当前桌面运行时无法生成短生命周期取消凭据。',
      false,
    )
  }
  const bytes = new Uint8Array(24)
  globalThis.crypto.getRandomValues(bytes)
  const encoded = Array.from(bytes, (value) => value.toString(16).padStart(2, '0')).join('')
  return `${scope}_cancel_token_${encoded}`
}

function nativeApiError(code: string, message: string, recoverable: boolean): ForgeApiError {
  return new ForgeApiError(message, code, recoverable, {}, 502)
}

function nativeContractError(message: string): ForgeApiError {
  return nativeApiError('NATIVE_AGENT_PROTOCOL_INVALID', message, false)
}

function readNativeThreadResult(
  value: unknown,
  expectedCommandId: string,
  expectedOutcome: 'thread',
): AgentThreadDetail {
  const result = readNativeCommandEnvelope(value, 'AgentThreadCommandResult@1', expectedCommandId)
  expectExactKeys(result, ['outcome', 'thread'], 'thread result outcome')
  if (result.outcome !== expectedOutcome) throw nativeContractError(`thread result must have ${expectedOutcome} outcome`)
  return readNativeAgentThreadDetail(result.thread, 'thread result.thread')
}

function readNativeThreadListResult(value: unknown, expectedCommandId: string): AgentThreadListResponse['items'] {
  const result = readNativeCommandEnvelope(value, 'AgentThreadCommandResult@1', expectedCommandId)
  expectExactKeys(result, ['outcome', 'threads'], 'thread list outcome')
  if (result.outcome !== 'threads' || !Array.isArray(result.threads)) {
    throw nativeContractError('thread/list must return the closed threads outcome')
  }
  return result.threads.map((thread, index) => readNativeAgentThreadSummary(thread, `thread list[${index}]`))
}

function readNativeTurnResult(
  value: unknown,
  expectedCommandId: string,
  expectedOutcome: 'started',
):
  | { outcome: 'started'; turn: AgentTurn; cancellationId: string; cancellationToken: string }
  | { outcome: 'terminal_replay'; turn: AgentTurn }
function readNativeTurnResult(
  value: unknown,
  expectedCommandId: string,
  expectedOutcome: 'turn',
): { turn: AgentTurn }
function readNativeTurnResult(
  value: unknown,
  expectedCommandId: string,
  expectedOutcome: 'started' | 'turn',
):
  | { outcome: 'started'; turn: AgentTurn; cancellationId: string; cancellationToken: string }
  | { outcome: 'terminal_replay'; turn: AgentTurn }
  | { turn: AgentTurn } {
  const result = readNativeCommandEnvelope(value, 'AgentTurnCommandResult@1', expectedCommandId)
  if (expectedOutcome === 'started') {
    if (result.outcome === 'started') {
      expectExactKeys(
        result,
        ['outcome', 'turn', 'cancellation_id', 'cancellation_token'],
        'turn started outcome',
      )
      const turn = readNativeAgentTurn(result.turn, 'turn started.turn')
      if (!isNativeTurnRunning(turn)) throw nativeContractError('turn/start started outcome must be queued or running')
      return {
        outcome: 'started',
        turn,
        cancellationId: expectStableId(result.cancellation_id, 'turn started.cancellation_id'),
        cancellationToken: expectStableId(result.cancellation_token, 'turn started.cancellation_token'),
      }
    }
    if (result.outcome === 'turn') {
      expectExactKeys(result, ['outcome', 'turn'], 'turn terminal replay outcome')
      const turn = readNativeAgentTurn(result.turn, 'turn terminal replay.turn')
      if (!isNativeTurnTerminal(turn)) {
        throw nativeContractError('turn/start replay without cancellation capability must already be terminal')
      }
      return { outcome: 'terminal_replay', turn }
    }
    throw nativeContractError('turn/start must return the closed started or terminal turn outcome')
  }
  expectExactKeys(result, ['outcome', 'turn'], 'turn read outcome')
  if (result.outcome !== 'turn') throw nativeContractError('turn/read must return the closed turn outcome')
  return { turn: readNativeAgentTurn(result.turn, 'turn read.turn') }
}

function readNativeTurnCancellationResult(
  value: unknown,
  expectedCommandId: string,
  expectedThreadId: string,
  expectedTurnId: string,
  expectedCancellationId: string,
): boolean {
  const result = readNativeCommandEnvelope(value, 'AgentTurnCommandResult@1', expectedCommandId)
  expectExactKeys(
    result,
    ['outcome', 'thread_id', 'turn_id', 'cancellation_id', 'accepted'],
    'turn cancellation outcome',
  )
  if (
    result.outcome !== 'cancellation_accepted'
    || result.thread_id !== expectedThreadId
    || result.turn_id !== expectedTurnId
    || result.cancellation_id !== expectedCancellationId
  ) {
    throw nativeContractError('turn/cancel returned an invalid or mismatched cancellation outcome')
  }
  return expectBoolean(result.accepted, 'turn cancellation.accepted')
}

function readNativeItemListResult(value: unknown, expectedCommandId: string): NativeItemList {
  const result = readNativeCommandEnvelope(value, 'AgentItemCommandResult@1', expectedCommandId)
  expectExactKeys(result, ['outcome', 'items', 'next_sequence'], 'item list outcome', ['next_sequence'])
  if (result.outcome !== 'items' || !Array.isArray(result.items)) {
    throw nativeContractError('item/list must return the closed items outcome')
  }
  const items = result.items.map((item, index) => readNativeAgentItem(item, `item list[${index}]`))
  for (let index = 1; index < items.length; index += 1) {
    if (items[index].sequence <= items[index - 1].sequence) {
      throw nativeContractError('item/list sequences must be strictly increasing')
    }
  }
  const nextSequence = result.next_sequence === undefined || result.next_sequence === null
    ? null
    : expectPositiveInteger(result.next_sequence, 'item list.next_sequence')
  if (nextSequence !== null && items.length > 0 && nextSequence <= items[items.length - 1].sequence) {
    throw nativeContractError('item/list next_sequence must follow the final returned item')
  }
  return { items, nextSequence }
}

function readNativeApprovalResult(value: unknown, expectedCommandId: string): AgentApproval {
  const result = readNativeCommandEnvelope(value, 'AgentApprovalCommandResult@1', expectedCommandId)
  expectExactKeys(result, ['outcome', 'approval'], 'approval outcome')
  if (result.outcome !== 'approval') throw nativeContractError('approval command must return the closed approval outcome')
  return readNativeAgentApproval(result.approval, 'approval result.approval')
}

function readNativeProviderPreflightResult(
  value: unknown,
  expectedExecutionId: string,
): NativeProviderPreflight {
  const result = expectRecord(value, 'provider preflight result')
  expectExactKeys(
    result,
    ['schema_version', 'execution_id', 'status', 'provider_id', 'configured', 'network_call_made', 'failure_category'],
    'provider preflight result',
    ['provider_id', 'failure_category'],
  )
  expectSchema(result, 'ProviderPreflightResult@1', 'provider preflight result')
  if (result.execution_id !== expectedExecutionId) throw nativeContractError('provider preflight execution_id mismatch')
  const status = expectEnum(result.status, ['unconfigured', 'ready', 'failed'] as const, 'provider preflight.status')
  const providerId = readOptionalStableId(result.provider_id, 'provider preflight.provider_id')
  const configured = expectBoolean(result.configured, 'provider preflight.configured')
  if (result.network_call_made !== false) throw nativeContractError('provider preflight must be network-free')
  const failureCategory = readNativeProviderFailureCategory(result.failure_category, 'provider preflight.failure_category')
  if (status === 'ready' && (!configured || !providerId || failureCategory !== null)) {
    throw nativeContractError('ready provider preflight requires configured identity and no failure')
  }
  if (status === 'unconfigured' && (configured || failureCategory !== null)) {
    throw nativeContractError('unconfigured provider preflight cannot claim configuration or failure')
  }
  if (status === 'failed' && failureCategory === null) {
    throw nativeContractError('failed provider preflight requires a failure category')
  }
  return { executionId: expectedExecutionId, status, providerId, configured, failureCategory }
}

function readNativeProviderCheckResult(
  value: unknown,
  expectedExecutionId: string,
  expectedProviderId: string,
): NativeProviderCheck {
  const result = expectRecord(value, 'provider check result')
  expectExactKeys(
    result,
    ['schema_version', 'execution_id', 'provider_id', 'status', 'network_call_made', 'usage', 'failure_category'],
    'provider check result',
    ['usage', 'failure_category'],
  )
  expectSchema(result, 'ProviderCheckResult@1', 'provider check result')
  if (result.execution_id !== expectedExecutionId || result.provider_id !== expectedProviderId) {
    throw nativeContractError('provider/check returned mismatched execution or provider identity')
  }
  const status = expectEnum(
    result.status,
    ['unconfigured', 'ready', 'failed', 'cancelled'] as const,
    'provider check.status',
  )
  const networkCallMade = expectBoolean(result.network_call_made, 'provider check.network_call_made')
  const failureCategory = readNativeProviderFailureCategory(result.failure_category, 'provider check.failure_category')
  if (result.usage !== undefined) readNativeProviderUsage(result.usage)
  if (status === 'ready' && (!networkCallMade || failureCategory !== null)) {
    throw nativeContractError('ready provider check requires network_call_made and no failure')
  }
  if (status === 'unconfigured' && (networkCallMade || failureCategory !== null || result.usage !== undefined)) {
    throw nativeContractError('unconfigured provider check must stop before network and usage')
  }
  if ((status === 'failed' || status === 'cancelled') && failureCategory === null) {
    throw nativeContractError('failed or cancelled provider check requires a failure category')
  }
  return {
    executionId: expectedExecutionId,
    providerId: expectedProviderId,
    status,
    networkCallMade,
    failureCategory,
  }
}

function readNativeProviderCancelResult(
  value: unknown,
  expected: NativeProviderCancellation,
): { accepted: boolean; alreadyTerminal: boolean } {
  const result = expectRecord(value, 'provider cancel result')
  expectExactKeys(
    result,
    ['schema_version', 'execution_id', 'cancellation_id', 'accepted', 'already_terminal'],
    'provider cancel result',
  )
  expectSchema(result, 'ProviderCancelResult@1', 'provider cancel result')
  if (result.execution_id !== expected.executionId || result.cancellation_id !== expected.cancellationId) {
    throw nativeContractError('provider/cancel returned mismatched execution or cancellation identity')
  }
  const accepted = expectBoolean(result.accepted, 'provider cancel.accepted')
  const alreadyTerminal = expectBoolean(result.already_terminal, 'provider cancel.already_terminal')
  if (accepted === alreadyTerminal) {
    throw nativeContractError('provider/cancel must report exactly one of accepted or already_terminal')
  }
  return { accepted, alreadyTerminal }
}

function readNativeCommandEnvelope(
  value: unknown,
  schemaVersion: string,
  expectedCommandId: string,
): Record<string, unknown> {
  const envelope = expectRecord(value, `${schemaVersion} envelope`)
  expectExactKeys(envelope, ['schema_version', 'command_id', 'result'], `${schemaVersion} envelope`)
  expectSchema(envelope, schemaVersion, `${schemaVersion} envelope`)
  if (envelope.command_id !== expectedCommandId) throw nativeContractError(`${schemaVersion} command_id mismatch`)
  return expectRecord(envelope.result, `${schemaVersion}.result`)
}

type NativeAgentThreadSummary = Omit<AgentThreadDetail, 'turns'>

function readNativeAgentThreadSummary(value: unknown, context: string): NativeAgentThreadSummary {
  const record = expectRecord(value, context)
  expectExactKeys(
    record,
    ['thread_id', 'project_id', 'title', 'status', 'summary', 'provider_id', 'created_at', 'updated_at', 'last_turn_id'],
    context,
    ['project_id', 'last_turn_id'],
  )
  return {
    thread_id: expectStableId(record.thread_id, `${context}.thread_id`),
    project_id: readOptionalStableId(record.project_id, `${context}.project_id`),
    title: expectNonEmptyString(record.title, `${context}.title`),
    status: expectEnum(record.status, ['idle', 'active', 'error', 'archived'] as const, `${context}.status`),
    summary: expectString(record.summary, `${context}.summary`),
    provider_id: expectStableId(record.provider_id, `${context}.provider_id`),
    created_at: expectNonEmptyString(record.created_at, `${context}.created_at`),
    updated_at: expectNonEmptyString(record.updated_at, `${context}.updated_at`),
    last_turn_id: readOptionalStableId(record.last_turn_id, `${context}.last_turn_id`),
  }
}

function readNativeAgentThreadDetail(value: unknown, context: string): AgentThreadDetail {
  const record = expectRecord(value, context)
  expectExactKeys(
    record,
    ['thread_id', 'project_id', 'title', 'status', 'summary', 'provider_id', 'created_at', 'updated_at', 'last_turn_id', 'turns'],
    context,
    ['project_id', 'last_turn_id'],
  )
  const summary = readNativeAgentThreadSummary(
    Object.fromEntries(Object.entries(record).filter(([key]) => key !== 'turns')),
    `${context}.summary`,
  )
  if (!Array.isArray(record.turns)) throw nativeContractError(`${context}.turns must be an array`)
  const turns = record.turns.map((turn, index) => readNativeAgentTurn(turn, `${context}.turns[${index}]`))
  const turnIds = new Set<string>()
  for (const turn of turns) {
    if (turn.thread_id !== summary.thread_id || turnIds.has(turn.turn_id)) {
      throw nativeContractError(`${context}.turns must preserve parent identity and unique turn_id`)
    }
    turnIds.add(turn.turn_id)
  }
  if (summary.last_turn_id && !turnIds.has(summary.last_turn_id)) {
    throw nativeContractError(`${context}.last_turn_id must reference a returned Turn`)
  }
  return { ...summary, turns }
}

function readNativeAgentTurn(value: unknown, context: string): AgentTurn {
  const record = expectRecord(value, context)
  expectExactKeys(
    record,
    ['turn_id', 'thread_id', 'request_text', 'status', 'error_code', 'error_message', 'usage', 'created_at', 'updated_at', 'items', 'approvals'],
    context,
    ['error_code', 'error_message'],
  )
  const turnId = expectStableId(record.turn_id, `${context}.turn_id`)
  const threadId = expectStableId(record.thread_id, `${context}.thread_id`)
  const usage = expectRecord(record.usage, `${context}.usage`)
  assertNoForbiddenNativeFields(usage, `${context}.usage`)
  if (!Array.isArray(record.items) || !Array.isArray(record.approvals)) {
    throw nativeContractError(`${context}.items and approvals must be arrays`)
  }
  const items = record.items.map((item, index) => readNativeAgentItem(item, `${context}.items[${index}]`))
  const approvals = record.approvals.map((approval, index) => (
    readNativeAgentApproval(approval, `${context}.approvals[${index}]`)
  ))
  let previousSequence = 0
  const itemIds = new Set<string>()
  for (const item of items) {
    if (
      item.thread_id !== threadId
      || item.turn_id !== turnId
      || item.sequence <= previousSequence
      || itemIds.has(item.item_id)
    ) {
      throw nativeContractError(`${context}.items must preserve parent identity, order and unique IDs`)
    }
    previousSequence = item.sequence
    itemIds.add(item.item_id)
  }
  const approvalIds = new Set<string>()
  for (const approval of approvals) {
    const item = items.find((candidate) => candidate.item_id === approval.item_id)
    if (
      approval.thread_id !== threadId
      || approval.turn_id !== turnId
      || !item
      || item.item_type !== 'approval_request'
      || approvalIds.has(approval.approval_id)
    ) {
      throw nativeContractError(`${context}.approvals must reference unique approval_request Items in the same Turn`)
    }
    approvalIds.add(approval.approval_id)
  }
  return {
    turn_id: turnId,
    thread_id: threadId,
    request_text: expectNonEmptyString(record.request_text, `${context}.request_text`),
    status: expectEnum(
      record.status,
      ['queued', 'running', 'waiting_for_approval', 'waiting_for_clarification', 'completed', 'failed', 'cancelled'] as const,
      `${context}.status`,
    ),
    error_code: readOptionalStableId(record.error_code, `${context}.error_code`),
    error_message: readOptionalString(record.error_message, `${context}.error_message`),
    usage,
    created_at: expectNonEmptyString(record.created_at, `${context}.created_at`),
    updated_at: expectNonEmptyString(record.updated_at, `${context}.updated_at`),
    items,
    approvals,
  }
}

function readNativeAgentItem(value: unknown, context: string): AgentItem {
  const record = expectRecord(value, context)
  expectExactKeys(
    record,
    ['item_id', 'thread_id', 'turn_id', 'sequence', 'item_type', 'status', 'payload', 'created_at'],
    context,
  )
  const payload = expectRecord(record.payload, `${context}.payload`)
  assertNoForbiddenNativeFields(payload, `${context}.payload`)
  return {
    item_id: expectStableId(record.item_id, `${context}.item_id`),
    thread_id: expectStableId(record.thread_id, `${context}.thread_id`),
    turn_id: expectStableId(record.turn_id, `${context}.turn_id`),
    sequence: expectPositiveInteger(record.sequence, `${context}.sequence`),
    item_type: expectEnum(
      record.item_type,
      ['user_message', 'assistant_message', 'plan', 'tool_call', 'tool_result', 'preview', 'approval_request', 'clarification', 'artifact'] as const,
      `${context}.item_type`,
    ),
    status: expectEnum(record.status, ['pending', 'completed', 'failed', 'cancelled'] as const, `${context}.status`),
    payload,
    created_at: expectNonEmptyString(record.created_at, `${context}.created_at`),
  }
}

function readNativeAgentApproval(value: unknown, context: string): AgentApproval {
  const record = expectRecord(value, context)
  expectExactKeys(
    record,
    ['approval_id', 'thread_id', 'turn_id', 'item_id', 'action', 'status', 'payload', 'created_at', 'resolved_at'],
    context,
    ['resolved_at'],
  )
  const payload = expectRecord(record.payload, `${context}.payload`)
  assertNoForbiddenNativeFields(payload, `${context}.payload`)
  return {
    approval_id: expectStableId(record.approval_id, `${context}.approval_id`),
    thread_id: expectStableId(record.thread_id, `${context}.thread_id`),
    turn_id: expectStableId(record.turn_id, `${context}.turn_id`),
    item_id: expectStableId(record.item_id, `${context}.item_id`),
    action: expectNonEmptyString(record.action, `${context}.action`),
    status: expectEnum(record.status, ['pending', 'approved', 'rejected'] as const, `${context}.status`),
    payload,
    created_at: expectNonEmptyString(record.created_at, `${context}.created_at`),
    resolved_at: readOptionalString(record.resolved_at, `${context}.resolved_at`),
  }
}

function readNativeItemNotification(
  notification: JsonRpcNotification,
  expectedThreadId: string,
): AgentEvent | null {
  if (notification.method !== 'item/updated') return null
  const record = expectRecord(notification.params, 'item/updated notification')
  if (typeof record.thread_id === 'string' && record.thread_id !== expectedThreadId) return null
  expectExactKeys(
    record,
    ['schema_version', 'notification_id', 'cursor', 'sequence', 'thread_id', 'turn_id', 'item_id', 'payload'],
    'item/updated notification',
  )
  expectSchema(record, 'NativeAgentNotification@1', 'item/updated notification')
  const threadId = expectStableId(record.thread_id, 'item/updated.thread_id')
  const turnId = expectStableId(record.turn_id, 'item/updated.turn_id')
  const itemId = expectStableId(record.item_id, 'item/updated.item_id')
  expectStableId(record.notification_id, 'item/updated.notification_id')
  expectNonEmptyString(record.cursor, 'item/updated.cursor')
  const sequence = expectPositiveInteger(record.sequence, 'item/updated.sequence')
  const payload = expectRecord(record.payload, 'item/updated.payload')
  expectExactKeys(payload, ['event', 'item'], 'item/updated.payload')
  if (payload.event !== 'item_updated') throw nativeContractError('item/updated payload event must be item_updated')
  const item = readNativeAgentItem(payload.item, 'item/updated.payload.item')
  if (
    threadId !== expectedThreadId
    || item.thread_id !== threadId
    || item.turn_id !== turnId
    || item.item_id !== itemId
    || item.sequence !== sequence
  ) {
    throw nativeContractError('item/updated notification identity does not match its AgentItem')
  }
  return { sequence, thread_id: threadId, turn_id: turnId, item }
}

function readNativeLifecycleNotificationTurnId(
  notification: JsonRpcNotification,
  expectedThreadId: string,
): string | null {
  if (notification.method === 'item/updated') {
    return readNativeItemNotification(notification, expectedThreadId)?.turn_id ?? null
  }
  if (
    notification.method === 'turn/started'
    || notification.method === 'turn/completed'
    || notification.method === 'turn/failed'
    || notification.method === 'turn/cancelled'
  ) {
    return readNativeTurnNotification(notification, expectedThreadId)?.turn_id ?? null
  }
  if (notification.method === 'approval/created' || notification.method === 'approval/resolved') {
    return readNativeApprovalNotification(notification, expectedThreadId)?.turn_id ?? null
  }
  return null
}

function readNativeTurnNotification(
  notification: JsonRpcNotification,
  expectedThreadId: string,
): AgentTurn | null {
  const eventByMethod = {
    'turn/started': 'turn_started',
    'turn/completed': 'turn_completed',
    'turn/failed': 'turn_failed',
    'turn/cancelled': 'turn_cancelled',
  } as const
  const event = eventByMethod[notification.method as keyof typeof eventByMethod]
  if (!event) return null
  const record = expectRecord(notification.params, `${notification.method} notification`)
  if (typeof record.thread_id === 'string' && record.thread_id !== expectedThreadId) return null
  expectExactKeys(
    record,
    ['schema_version', 'notification_id', 'cursor', 'sequence', 'thread_id', 'turn_id', 'payload'],
    `${notification.method} notification`,
  )
  expectSchema(record, 'NativeAgentNotification@1', `${notification.method} notification`)
  const threadId = expectStableId(record.thread_id, `${notification.method}.thread_id`)
  const turnId = expectStableId(record.turn_id, `${notification.method}.turn_id`)
  expectStableId(record.notification_id, `${notification.method}.notification_id`)
  expectNonEmptyString(record.cursor, `${notification.method}.cursor`)
  expectPositiveInteger(record.sequence, `${notification.method}.sequence`)
  const payload = expectRecord(record.payload, `${notification.method}.payload`)
  expectExactKeys(payload, ['event', 'turn'], `${notification.method}.payload`)
  if (payload.event !== event) throw nativeContractError(`${notification.method} payload event mismatch`)
  const turn = readNativeAgentTurn(payload.turn, `${notification.method}.payload.turn`)
  if (threadId !== expectedThreadId || turn.thread_id !== threadId || turn.turn_id !== turnId) {
    throw nativeContractError(`${notification.method} notification identity mismatch`)
  }
  const expectedStatus = notification.method === 'turn/completed'
    ? 'completed'
    : notification.method === 'turn/failed'
      ? 'failed'
      : notification.method === 'turn/cancelled'
        ? 'cancelled'
        : null
  if (expectedStatus && turn.status !== expectedStatus) {
    throw nativeContractError(`${notification.method} Turn status mismatch`)
  }
  return turn
}

function readNativeApprovalNotification(
  notification: JsonRpcNotification,
  expectedThreadId: string,
): AgentApproval | null {
  const event = notification.method === 'approval/created'
    ? 'approval_created'
    : notification.method === 'approval/resolved'
      ? 'approval_resolved'
      : null
  if (!event) return null
  const record = expectRecord(notification.params, `${notification.method} notification`)
  if (typeof record.thread_id === 'string' && record.thread_id !== expectedThreadId) return null
  expectExactKeys(
    record,
    ['schema_version', 'notification_id', 'cursor', 'sequence', 'thread_id', 'turn_id', 'item_id', 'approval_id', 'payload'],
    `${notification.method} notification`,
  )
  expectSchema(record, 'NativeAgentNotification@1', `${notification.method} notification`)
  const threadId = expectStableId(record.thread_id, `${notification.method}.thread_id`)
  const turnId = expectStableId(record.turn_id, `${notification.method}.turn_id`)
  const itemId = expectStableId(record.item_id, `${notification.method}.item_id`)
  const approvalId = expectStableId(record.approval_id, `${notification.method}.approval_id`)
  expectStableId(record.notification_id, `${notification.method}.notification_id`)
  expectNonEmptyString(record.cursor, `${notification.method}.cursor`)
  expectPositiveInteger(record.sequence, `${notification.method}.sequence`)
  const payload = expectRecord(record.payload, `${notification.method}.payload`)
  expectExactKeys(payload, ['event', 'approval'], `${notification.method}.payload`)
  if (payload.event !== event) throw nativeContractError(`${notification.method} payload event mismatch`)
  const approval = readNativeAgentApproval(payload.approval, `${notification.method}.payload.approval`)
  if (
    threadId !== expectedThreadId
    || approval.thread_id !== threadId
    || approval.turn_id !== turnId
    || approval.item_id !== itemId
    || approval.approval_id !== approvalId
  ) {
    throw nativeContractError(`${notification.method} notification identity mismatch`)
  }
  return approval
}

function mapNativeProviderPreflight(result: NativeProviderPreflight): AgentProviderCheckResponse {
  const providerId = result.providerId ?? 'deepseek'
  if (result.status === 'unconfigured') {
    const message = '模型服务尚未在桌面 Keychain 中配置；没有发起网络请求。'
    return {
      status: 'not_configured',
      provider_id: providerId,
      model: null,
      message,
      network_call_made: false,
      connection: {
        schema_version: 'ProviderConnectionState@1',
        status: 'unconfigured',
        provider_id: providerId,
        configured: false,
        metadata_status: 'not_checked',
        secret_status: 'not_checked',
        supervisor_status: 'running',
        capability_status: 'offline',
        network_call_made: false,
        failure_code: null,
        message,
      },
    }
  }
  const failureCode = nativeProviderFailureCode(result.failureCategory)
  const message = `模型服务预检失败（${failureCode}）；没有发起网络请求。`
  return {
    status: 'failed',
    provider_id: providerId,
    model: null,
    message,
    network_call_made: false,
    connection: {
      schema_version: 'ProviderConnectionState@1',
      status: 'failed',
      provider_id: providerId,
      configured: result.configured,
      metadata_status: result.configured ? 'valid' : 'not_checked',
      secret_status: result.configured ? 'available' : 'not_checked',
      supervisor_status: 'running',
      capability_status: 'unavailable',
      network_call_made: false,
      failure_code: failureCode,
      message,
    },
  }
}

function mapNativeProviderCheck(result: NativeProviderCheck): AgentProviderCheckResponse {
  const failureCode = nativeProviderFailureCode(result.failureCategory)
  const responseStatus = result.status === 'unconfigured'
    ? 'not_configured'
    : result.status
  const message = result.status === 'ready'
    ? '模型服务已通过一次显式连接检查。'
    : result.status === 'unconfigured'
      ? '模型服务在检查前变为未配置；没有发起网络请求。'
      : result.status === 'cancelled'
        ? '本次模型服务检查已取消。'
        : `模型服务检查失败（${failureCode}）。`
  const configured = result.status !== 'unconfigured'
  return {
    status: responseStatus,
    provider_id: result.providerId,
    model: null,
    message,
    network_call_made: result.networkCallMade,
    connection: {
      schema_version: 'ProviderConnectionState@1',
      status: result.status === 'ready'
        ? 'ready'
        : result.status === 'unconfigured'
          ? 'unconfigured'
          : result.status === 'cancelled'
            ? 'degraded'
            : 'failed',
      provider_id: result.providerId,
      configured,
      metadata_status: configured ? 'valid' : 'not_checked',
      secret_status: configured ? 'available' : 'not_checked',
      supervisor_status: 'running',
      capability_status: result.status === 'ready' || result.status === 'cancelled' ? 'ready' : result.status === 'unconfigured' ? 'offline' : 'unavailable',
      network_call_made: result.networkCallMade,
      failure_code: failureCode,
      message,
    },
  }
}

function nativeProviderFailureCode(category: NativeProviderFailureCategory | null): string | null {
  if (category === null) return null
  return {
    invalid_request: 'DEEPSEEK_INVALID_REQUEST',
    authentication: 'DEEPSEEK_AUTH_FAILED',
    balance: 'DEEPSEEK_BALANCE_EXHAUSTED',
    rate_limited: 'DEEPSEEK_RATE_LIMITED',
    server_unavailable: 'DEEPSEEK_SERVER_BUSY',
    timeout: 'PROVIDER_TIMEOUT',
    network: 'PROVIDER_NETWORK_ERROR',
    empty_content: 'PROVIDER_EMPTY_CONTENT',
    invalid_json: 'PROVIDER_INVALID_JSON',
    schema_violation: 'PROVIDER_SCHEMA_MISMATCH',
    budget_exceeded: 'PROVIDER_BUDGET_EXCEEDED',
    cancelled: 'PROVIDER_CANCELLED',
  }[category]
}

function readNativeProviderFailureCategory(value: unknown, context: string): NativeProviderFailureCategory | null {
  if (value === undefined || value === null) return null
  return expectEnum(
    value,
    ['invalid_request', 'authentication', 'balance', 'rate_limited', 'server_unavailable', 'timeout', 'network', 'empty_content', 'invalid_json', 'schema_violation', 'budget_exceeded', 'cancelled'] as const,
    context,
  )
}

function readNativeProviderUsage(value: unknown): void {
  const usage = expectRecord(value, 'provider check.usage')
  expectExactKeys(
    usage,
    ['input_tokens', 'output_tokens', 'prompt_cache_hit_tokens', 'prompt_cache_miss_tokens'],
    'provider check.usage',
  )
  for (const key of ['input_tokens', 'output_tokens', 'prompt_cache_hit_tokens', 'prompt_cache_miss_tokens']) {
    expectNonNegativeInteger(usage[key], `provider check.usage.${key}`)
  }
}

function nativeAgentEventFingerprint(event: AgentEvent): string {
  return `${event.sequence}:${event.item.item_id}:${event.item.status}:${JSON.stringify(event.item.payload)}`
}

function isNativeTurnRunning(turn: AgentTurn): boolean {
  return turn.status === 'queued' || turn.status === 'running'
}

function isNativeTurnTerminal(turn: AgentTurn): boolean {
  return turn.status === 'completed' || turn.status === 'failed' || turn.status === 'cancelled'
}

async function waitForNativeWake(wake: Promise<void>, timeoutMs: number): Promise<void> {
  let timeout: ReturnType<typeof globalThis.setTimeout> | null = null
  try {
    await Promise.race([
      wake,
      new Promise<void>((resolve) => { timeout = globalThis.setTimeout(resolve, timeoutMs) }),
    ])
  } finally {
    if (timeout !== null) globalThis.clearTimeout(timeout)
  }
}

function expectRecord(value: unknown, context: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw nativeContractError(`${context} must be an object`)
  }
  return value as Record<string, unknown>
}

function expectExactKeys(
  record: Record<string, unknown>,
  allowedKeys: readonly string[],
  context: string,
  optionalKeys: readonly string[] = [],
): void {
  const allowed = new Set(allowedKeys)
  for (const key of Object.keys(record)) {
    if (!allowed.has(key)) throw nativeContractError(`${context} contains unknown field ${key}`)
  }
  const optional = new Set(optionalKeys)
  for (const key of allowedKeys) {
    if (!optional.has(key) && !(key in record)) throw nativeContractError(`${context} is missing ${key}`)
  }
}

function expectSchema(record: Record<string, unknown>, expected: string, context: string): void {
  if (record.schema_version !== expected) throw nativeContractError(`${context} schema_version must be ${expected}`)
}

function expectString(value: unknown, context: string): string {
  if (typeof value !== 'string') throw nativeContractError(`${context} must be a string`)
  return value
}

function expectNonEmptyString(value: unknown, context: string): string {
  const result = expectString(value, context)
  if (result.length === 0) throw nativeContractError(`${context} must not be empty`)
  return result
}

function expectStableId(value: unknown, context: string): string {
  const result = expectNonEmptyString(value, context)
  if (!/^[A-Za-z0-9_.-]{1,160}$/.test(result)) throw nativeContractError(`${context} must be a stable ID`)
  return result
}

function readOptionalStableId(value: unknown, context: string): string | null {
  if (value === undefined || value === null) return null
  return expectStableId(value, context)
}

function readOptionalString(value: unknown, context: string): string | null {
  if (value === undefined || value === null) return null
  return expectString(value, context)
}

function expectBoolean(value: unknown, context: string): boolean {
  if (typeof value !== 'boolean') throw nativeContractError(`${context} must be a boolean`)
  return value
}

function expectPositiveInteger(value: unknown, context: string): number {
  if (!Number.isSafeInteger(value) || Number(value) <= 0) throw nativeContractError(`${context} must be a positive integer`)
  return Number(value)
}

function expectNonNegativeInteger(value: unknown, context: string): number {
  if (!Number.isSafeInteger(value) || Number(value) < 0) throw nativeContractError(`${context} must be a non-negative integer`)
  return Number(value)
}

function expectEnum<const T extends readonly string[]>(
  value: unknown,
  allowed: T,
  context: string,
): T[number] {
  if (typeof value !== 'string' || !allowed.includes(value)) {
    throw nativeContractError(`${context} is outside the closed outcome set`)
  }
  return value as T[number]
}

function assertNoForbiddenNativeFields(value: unknown, context: string, depth = 0): void {
  if (depth > 32) throw nativeContractError(`${context} exceeds the bounded nesting depth`)
  if (Array.isArray(value)) {
    for (const item of value) assertNoForbiddenNativeFields(item, context, depth + 1)
    return
  }
  if (typeof value !== 'object' || value === null) return
  for (const [key, nested] of Object.entries(value)) {
    if (key === 'reasoning_content') throw nativeContractError(`${context} cannot contain reasoning_content`)
    assertNoForbiddenNativeFields(nested, context, depth + 1)
  }
}

function protocolEventError(error: unknown): Event {
  const event = new Event('error')
  if (error instanceof Error) Object.defineProperty(event, 'message', { value: error.message })
  const code = stableNativeReplayErrorCode(error)
  if (code !== null) Object.defineProperty(event, 'forgecad_error_code', { value: code })
  return event
}

function isTransientNativeReplayFailure(error: unknown): boolean {
  return error instanceof AppServerProtocolError
    && error.error.data?.application_code === 'ADAPTER_UNAVAILABLE'
    && error.error.data?.recoverable === true
}

function stableNativeReplayErrorCode(error: unknown): string | null {
  if (error instanceof AppServerProtocolError) {
    const code = error.error.data?.application_code
    if (typeof code === 'string' && /^[A-Z0-9_]{1,80}$/.test(code)) return code
    return `RPC_${Math.abs(error.error.code)}`
  }
  if (error instanceof ForgeApiError && /^[A-Z0-9_]{1,80}$/.test(error.code)) return error.code
  return null
}

async function readJson<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const body = await response.text()
    try {
      const parsed = JSON.parse(body) as ApiErrorEnvelope
      if (parsed.error) {
        throw new ForgeApiError(
          parsed.error.message,
          parsed.error.code,
          parsed.error.recoverable,
          parsed.error.details,
          response.status
        )
      }
    } catch (error) {
      if (error instanceof ForgeApiError) throw error
    }
    throw new ForgeApiError(body || `Request failed with ${response.status}`, 'UNKNOWN_ERROR', false, {}, response.status)
  }
  return (await response.json()) as T
}

async function readAgentAssetGlbBinary(response: Response): Promise<AgentAssetGlbBinary> {
  if (!response.ok) await readJson<never>(response)
  const metadata = readAgentAssetGlbHeaders(response)
  const glb = await response.arrayBuffer()
  if (glb.byteLength !== metadata.byteSize || glb.byteLength === 0) {
    throw new ForgeApiError('Agent asset GLB byte size does not match its readback headers', 'MODEL_GLB_SIZE_MISMATCH', false, {}, 502)
  }
  return { ...metadata, glb }
}

function readAgentAssetGlbHeaders(response: Response): Omit<AgentAssetGlbBinary, 'glb'> {
  const artifactProfileId = response.headers.get('x-forgecad-artifact-profile')
  const glbSha256 = response.headers.get('x-forgecad-glb-sha256')
  const triangleCount = Number(response.headers.get('x-forgecad-triangle-count'))
  const byteSize = Number(response.headers.get('x-forgecad-glb-byte-size'))
  if (
    !['external_reference', 'interactive_preview', 'production_concept'].includes(artifactProfileId ?? '')
    || !glbSha256?.match(/^[a-f0-9]{64}$/)
    || !Number.isInteger(triangleCount)
    || triangleCount <= 0
    || !Number.isInteger(byteSize)
    || byteSize <= 0
  ) {
    throw new ForgeApiError('Agent asset GLB response headers are incomplete', 'MODEL_GLB_HEADERS_INVALID', false, {}, 502)
  }
  return {
    artifactProfileId: artifactProfileId as AgentAssetGlbBinary['artifactProfileId'],
    artifactProfileSha256: response.headers.get('x-forgecad-artifact-profile-sha256'),
    shapeProgramSha256: response.headers.get('x-forgecad-shape-program-sha256'),
    glbSha256,
    triangleCount,
    byteSize,
  }
}

async function readJsonWithEtag<T>(response: Response): Promise<ActiveDesignApiResponse<T>> {
  const data = await readJson<T>(response)
  return { data, etag: response.headers.get('ETag') }
}

function activeDesignHeaders(clientRequestId: string, ifMatch?: string): HeadersInit {
  return {
    'Content-Type': 'application/json',
    'Idempotency-Key': clientRequestId,
    ...(ifMatch ? { 'If-Match': ifMatch } : {}),
  }
}

function singleResultRoute(
  baseUrl: string,
  input: SingleResultPreviewIdentity,
  action: ':preview.glb' | ':confirm' | ':reject',
): string {
  return `${baseUrl}/api/v1/agent/projects/${encodeURIComponent(input.projectId)}/turns/${encodeURIComponent(input.turnId)}/single-results/${encodeURIComponent(input.previewId)}${action}`
}

function singleResultArtifactEtag(artifactSha256: string): string {
  if (!/^[a-f0-9]{64}$/i.test(artifactSha256)) {
    throw new ForgeApiError('Single-result preview SHA-256 is invalid', 'SINGLE_RESULT_PREVIEW_HASH_INVALID', false, {}, 400)
  }
  return `"sha256:${artifactSha256.toLowerCase()}"`
}

export const forgeApi = new ForgeApiClient()
