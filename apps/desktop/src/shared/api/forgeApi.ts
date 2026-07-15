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

const DEFAULT_BASE_URL = import.meta.env.VITE_FORGE_API_BASE_URL || 'http://127.0.0.1:8000'

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

export type AgentRenderPackageDownload = {
  blob: Blob
  filename: string
  renderSetSha256: string | null
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
  constructor(private baseUrl = DEFAULT_BASE_URL) {}

  getBaseUrl(): string {
    return this.baseUrl
  }

  setBaseUrl(baseUrl: string): void {
    this.baseUrl = baseUrl.replace(/\/$/, '')
  }

  async checkHealth(): Promise<HealthResponse> {
    const response = await fetch(`${this.baseUrl}/api/health`)
    return readJson<HealthResponse>(response)
  }

  async createAgentThread(input: CreateAgentThreadRequest): Promise<AgentThreadDetail> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/threads`, {
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
    return readJson<AgentThreadListResponse>(await fetch(`${this.baseUrl}/api/v1/agent/threads`))
  }

  async listAgentDomainPacks(): Promise<DomainPackManifest[]> {
    return readJson<DomainPackManifest[]>(await fetch(`${this.baseUrl}/api/v1/agent/domain-packs`))
  }

  async listAgentMaterials(): Promise<AgentMaterialPreset[]> {
    return readJson<AgentMaterialPreset[]>(await fetch(`${this.baseUrl}/api/v1/agent/materials`))
  }

  async listAgentMaterialTextures(params: { texture_role?: string; source?: string; q?: string } = {}): Promise<AgentMaterialTextureListResponse> {
    const query = new URLSearchParams()
    if (params.texture_role) query.set('texture_role', params.texture_role)
    if (params.source) query.set('source', params.source)
    if (params.q) query.set('q', params.q)
    const suffix = query.toString() ? `?${query.toString()}` : ''
    return readJson<AgentMaterialTextureListResponse>(await fetch(`${this.baseUrl}/api/v1/agent/material-textures${suffix}`))
  }

  async registerAgentMaterialTexture(input: RegisterAgentMaterialTextureRequest, idempotencyKey: string): Promise<AgentMaterialTextureObject> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/material-textures`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Idempotency-Key': idempotencyKey },
      body: JSON.stringify(input),
    })
    return readJson<AgentMaterialTextureObject>(response)
  }

  async checkAgentProvider(): Promise<AgentProviderCheckResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/provider:check`, { method: 'POST' })
    return readJson<AgentProviderCheckResponse>(response)
  }

  async buildAgentBlockout(input: BuildAgentBlockoutRequest): Promise<BuildAgentBlockoutResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/blockouts`, {
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
    const response = await fetch(`${this.baseUrl}/api/v1/agent/blockouts:concept-preview`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(input),
    })
    return readJson<AgentBlockoutConceptPreview>(response)
  }

  async segmentAgentBlockout(input: SegmentAgentBlockoutRequest): Promise<SegmentAgentBlockoutResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/blockouts:segment`, {
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
    const response = await fetch(`${this.baseUrl}/api/v1/agent/blockouts:commit`, {
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
    return readJson<AgentAssetVersion>(await fetch(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}`))
  }

  async importAgentGlb(input: ImportAgentGlbRequest): Promise<ImportAgentGlbResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/imports:glb`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<ImportAgentGlbResponse>(response)
  }

  async qualityAgentAssetVersion(
    assetVersionId: string,
    input: { idempotencyKey: string; ifMatch: string },
  ): Promise<AgentAssetQualityReport> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:quality`, {
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
      await fetch(`${this.baseUrl}/api/v1/agent/quality-reports/${encodeURIComponent(qualityReportId)}`),
    )
  }

  async saveAgentComponent(assetVersionId: string, input: SaveAgentComponentRequest): Promise<AgentComponentRecord> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/components`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Idempotency-Key': input.client_request_id },
      body: JSON.stringify(input),
    })
    return readJson<AgentComponentRecord>(response)
  }

  async exportAgentAssetGlb(assetVersionId: string): Promise<AgentAssetExportResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}:export`, { method: 'POST' })
    return readJson<AgentAssetExportResponse>(response)
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
      await fetch(
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
    const response = await fetch(
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
    return readJson<AgentComponentRecord[]>(await fetch(`${this.baseUrl}/api/v1/agent/components?${params.toString()}`))
  }

  async listAgentComponentCandidates(assetVersionId: string, partId: string): Promise<AgentComponentCandidate[]> {
    const params = new URLSearchParams({ part_id: partId })
    return readJson<AgentComponentCandidate[]>(
      await fetch(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/components:compatible?${params.toString()}`),
    )
  }

  async listAgentStructureSuggestions(assetVersionId: string): Promise<AgentStructureSuggestionList> {
    return readJson<AgentStructureSuggestionList>(
      await fetch(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/structure-suggestions`),
    )
  }

  async proposeAgentAssetChangeSet(
    assetVersionId: string,
    input: ProposeAgentAssetChangeSetRequest,
  ): Promise<AgentAssetChangeSet> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/asset-versions/${encodeURIComponent(assetVersionId)}/change-sets`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': input.client_request_id,
      },
      body: JSON.stringify(input),
    })
    return readJson<AgentAssetChangeSet>(response)
  }

  async previewAgentAssetChangeSet(changeSetId: string, idempotencyKey: string): Promise<AgentAssetChangeSet> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:preview`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<AgentAssetChangeSet>(response)
  }

  async confirmAgentAssetChangeSet(changeSetId: string, idempotencyKey: string): Promise<AgentAssetChangeSetConfirmResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:confirm`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<AgentAssetChangeSetConfirmResponse>(response)
  }

  async rejectAgentAssetChangeSet(changeSetId: string, idempotencyKey: string): Promise<AgentAssetChangeSet> {
    const response = await fetch(`${this.baseUrl}/api/v1/agent/change-sets/${encodeURIComponent(changeSetId)}:reject`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<AgentAssetChangeSet>(response)
  }

  async getAgentThread(threadId: string): Promise<AgentThreadDetail> {
    return readJson<AgentThreadDetail>(
      await fetch(`${this.baseUrl}/api/v1/agent/threads/${encodeURIComponent(threadId)}`),
    )
  }

  async getActiveDesign(projectId: string): Promise<ActiveDesignApiResponse<ActiveDesignSnapshot>> {
    const response = await fetch(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design`)
    return readJsonWithEtag<ActiveDesignSnapshot>(response)
  }

  async getActiveDesignNavigation(projectId: string): Promise<ActiveDesignNavigation> {
    return readJson<ActiveDesignNavigation>(
      await fetch(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:navigation`),
    )
  }

  async setActiveDesignRenderPreset(
    projectId: string,
    input: SetActiveDesignRenderPresetRequest,
    options: { ifMatch?: string } = {},
  ): Promise<ActiveDesignApiResponse<ActiveDesignSnapshot>> {
    const response = await fetch(
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
    const response = await fetch(
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
    const response = await fetch(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:select`, {
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
    const response = await fetch(
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
    const response = await fetch(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:undo`, {
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
    const response = await fetch(`${this.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/active-design:redo`, {
      method: 'POST',
      headers: activeDesignHeaders(input.client_request_id, options.ifMatch),
      body: JSON.stringify(input),
    })
    return readJsonWithEtag<ActiveDesignSnapshot>(response)
  }

  async startAgentTurn(threadId: string, input: StartAgentTurnRequest): Promise<AgentTurn> {
    const response = await fetch(
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
    return readJson<AgentTurn>(
      await fetch(`${this.baseUrl}/api/v1/agent/turns/${encodeURIComponent(turnId)}/cancel`, {
        method: 'POST',
        headers: { 'Idempotency-Key': idempotencyKey },
      }),
    )
  }

  async createAgentApproval(
    threadId: string,
    input: CreateAgentApprovalRequest,
  ): Promise<AgentApproval> {
    const response = await fetch(
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
    const response = await fetch(
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
    const url = new URL(`${this.baseUrl}/api/v1/agent/threads/${encodeURIComponent(threadId)}/events`)
    url.searchParams.set('after', String(after))
    return url.toString()
  }

  async listConceptProjects(): Promise<ConceptProjectListResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/projects`)
    return readJson<ConceptProjectListResponse>(response)
  }

  async getConceptProject(projectId: string): Promise<ConceptProjectDetail> {
    const response = await fetch(`${this.baseUrl}/api/v1/projects/${projectId}`)
    return readJson<ConceptProjectDetail>(response)
  }

  async createConceptProject(input: CreateConceptProjectRequest): Promise<ConceptProjectDetail> {
    const response = await fetch(`${this.baseUrl}/api/v1/projects`, {
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
    const response = await fetch(
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
    const response = await fetch(`${this.baseUrl}/api/v1/versions/${versionId}`)
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
    const response = await fetch(url)
    return readJson<ModuleAssetListResponse>(response)
  }

  async updateModuleAssetCatalogMetadata(
    moduleId: string,
    input: UpdateModuleAssetCatalogMetadataRequest,
  ): Promise<ModuleAssetRecord> {
    const response = await fetch(
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
    return `${this.baseUrl}/api/v1/module-assets/${encodeURIComponent(moduleId)}/file`
  }

  getModuleAssetThumbnailUrl(moduleId: string): string {
    return `${this.baseUrl}/api/v1/module-assets/${encodeURIComponent(moduleId)}/thumbnail`
  }

  async getModuleGraph(graphId: string): Promise<ModuleGraphRecord> {
    const response = await fetch(`${this.baseUrl}/api/v1/module-graphs/${graphId}`)
    return readJson<ModuleGraphRecord>(response)
  }

  async inspectConceptVersion(
    versionId: string,
    input: InspectConceptVersionRequest,
  ): Promise<QualityRunRecord> {
    const response = await fetch(`${this.baseUrl}/api/v1/versions/${versionId}/quality-runs:inspect`, {
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
    const response = await fetch(`${this.baseUrl}/api/v1/versions/${encodeURIComponent(versionId)}/quality-runs:inspect:enqueue`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Idempotency-Key': input.client_request_id },
      body: JSON.stringify(input),
    })
    return readJson<ConceptJobRecord>(response)
  }

  async getQualityRun(qualityRunId: string): Promise<QualityRunRecord> {
    return readJson<QualityRunRecord>(
      await fetch(`${this.baseUrl}/api/v1/quality-runs/${encodeURIComponent(qualityRunId)}`),
    )
  }

  async getConceptJob(jobId: string): Promise<ConceptJobRecord> {
    return readJson<ConceptJobRecord>(await fetch(`${this.baseUrl}/api/v1/jobs/${encodeURIComponent(jobId)}`))
  }

  async runConceptWorkerOnce(): Promise<ConceptJobRecord | null> {
    return readJson<ConceptJobRecord | null>(await fetch(`${this.baseUrl}/api/v1/concept-jobs/work-once`, { method: 'POST' }))
  }

  async listDesignVariants(projectId: string): Promise<DesignVariantListResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/projects/${projectId}/variants`)
    return readJson<DesignVariantListResponse>(response)
  }

  async interpretDesignBrief(
    projectId: string,
    input: InterpretDesignBriefRequest,
  ): Promise<DesignBriefRecord> {
    const response = await fetch(`${this.baseUrl}/api/v1/projects/${projectId}/brief:interpret`, {
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
    const response = await fetch(`${this.baseUrl}/api/v1/projects/${projectId}/variants`, {
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
    const response = await fetch(
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
    const response = await fetch(`${this.baseUrl}/api/v1/versions/${versionId}/exports`, {
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
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/file`
  }

  getConceptCombinedGlbUrl(exportId: string): string {
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/combined.glb`
  }

  getConceptCombinedObjUrl(exportId: string): string {
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/combined.obj`
  }

  getConceptCombinedMtlUrl(exportId: string): string {
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/combined.mtl`
  }

  getConceptPreviewPngUrl(exportId: string): string {
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/preview.png`
  }

  getConceptExplodedPngUrl(exportId: string): string {
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/exploded.png`
  }

  getConceptRenderSetUrl(exportId: string): string {
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/renders.zip`
  }

  getConceptRenderViewUrl(exportId: string, viewName: 'front' | 'side' | 'top'): string {
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/views/${viewName}.png`
  }

  getConceptTurntableFrameUrl(exportId: string, frameIndex: number): string {
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/turntable/${frameIndex}.png`
  }

  getConceptTurntableVideoUrl(exportId: string): string {
    return `${this.baseUrl}/api/v1/exports/${encodeURIComponent(exportId)}/turntable.mp4`
  }

  async proposeChangeSet(
    versionId: string,
    input: ProposeChangeSetRequest,
  ): Promise<DesignChangeSet> {
    const response = await fetch(`${this.baseUrl}/api/v1/versions/${versionId}/change-sets`, {
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
    const response = await fetch(
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
    const response = await fetch(`${this.baseUrl}/api/v1/versions/${versionId}/change-sets:plan`, {
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
    const response = await fetch(url)
    return readJson<ChangeSetTimelineResponse>(response)
  }

  async createChangeSetAuditExport(
    projectId: string,
    input: CreateChangeSetAuditExportRequest,
  ): Promise<ChangeSetAuditExportRecord> {
    const response = await fetch(
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
    const response = await fetch(url)
    return readJson<ChangeSetAuditExportListResponse>(response)
  }

  getChangeSetAuditExportFileUrl(auditExportId: string): string {
    return `${this.baseUrl}/api/v1/change-set-audit-exports/${encodeURIComponent(auditExportId)}/file`
  }

  async previewChangeSet(changeSetId: string, idempotencyKey: string): Promise<ChangeSetPreviewResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/change-sets/${changeSetId}:preview`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<ChangeSetPreviewResponse>(response)
  }

  async confirmChangeSet(changeSetId: string, idempotencyKey: string): Promise<ChangeSetConfirmResponse> {
    const response = await fetch(`${this.baseUrl}/api/v1/change-sets/${changeSetId}:confirm`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<ChangeSetConfirmResponse>(response)
  }

  async rejectChangeSet(changeSetId: string, idempotencyKey: string): Promise<DesignChangeSet> {
    const response = await fetch(`${this.baseUrl}/api/v1/change-sets/${changeSetId}:reject`, {
      method: 'POST',
      headers: { 'Idempotency-Key': idempotencyKey },
    })
    return readJson<DesignChangeSet>(response)
  }

  async createWeapon(input: CreateWeaponRequest): Promise<CreateWeaponResponse> {
    const response = await fetch(`${this.baseUrl}/api/weapons`, {
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
    const response = await fetch(`${this.baseUrl}/api/weapons`)
    const data = await readJson<{ items: WeaponSummary[] }>(response)
    return data.items
  }

  async getWeapon(weaponId: string): Promise<WeaponDetail> {
    const response = await fetch(`${this.baseUrl}/api/weapons/${weaponId}`)
    return readJson<WeaponDetail>(response)
  }

  async createInterpretation(weaponId: string, input: CreativeInterpretationRequest): Promise<CreativeInterpretationResponse> {
    const response = await fetch(`${this.baseUrl}/api/weapons/${weaponId}/interpretation`, {
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
    const response = await fetch(`${this.baseUrl}/api/weapons/${weaponId}/recast/confirm`, {
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
    const response = await fetch(`${this.baseUrl}/api/weapons/${weaponId}/creative-graph`)
    return readJson<CreativeGraphResponse>(response)
  }

  async getAssetMetadata(assetId: string): Promise<AssetFileResponse> {
    const response = await fetch(`${this.baseUrl}/api/assets/${assetId}`)
    return readJson<AssetFileResponse>(response)
  }

  getAssetFileUrl(assetId: string): string {
    return `${this.baseUrl}/api/assets/${assetId}/file`
  }

  async revealAsset(assetId: string, input: { dryRun?: boolean } = {}): Promise<AssetRevealResponse> {
    const url = new URL(`${this.baseUrl}/api/assets/${assetId}/reveal`)
    if (input.dryRun) url.searchParams.set('dry_run', 'true')
    const response = await fetch(url, {
      method: 'POST',
    })
    return readJson<AssetRevealResponse>(response)
  }

  async getJob(jobId: string): Promise<JobDetail> {
    const response = await fetch(`${this.baseUrl}/api/jobs/${jobId}`)
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
    const response = await fetch(url)
    return readJson<JobListResponse>(response)
  }

  async getJobRuntime(jobId: string): Promise<JobRuntimeStateResponse> {
    const response = await fetch(`${this.baseUrl}/api/jobs/${jobId}/runtime`)
    return readJson<JobRuntimeStateResponse>(response)
  }

  async listJobActions(jobId: string, input: { cursor?: string; limit?: number } = {}): Promise<JobActionListResponse> {
    const url = new URL(`${this.baseUrl}/api/jobs/${jobId}/actions`)
    if (input.cursor) url.searchParams.set('cursor', input.cursor)
    if (input.limit) url.searchParams.set('limit', String(input.limit))
    const response = await fetch(url)
    return readJson<JobActionListResponse>(response)
  }

  async recoverRuntime(): Promise<RuntimeRecoveryResponse> {
    const response = await fetch(`${this.baseUrl}/api/runtime/recover`, {
      method: 'POST',
    })
    return readJson<RuntimeRecoveryResponse>(response)
  }

  async workOnce(): Promise<RuntimeWorkOnceResponse> {
    const response = await fetch(`${this.baseUrl}/api/runtime/work-once`, {
      method: 'POST',
    })
    return readJson<RuntimeWorkOnceResponse>(response)
  }

  async retryJob(jobId: string): Promise<JobActionResponse> {
    const response = await fetch(`${this.baseUrl}/api/jobs/${jobId}/retry`, {
      method: 'POST',
    })
    return readJson<JobActionResponse>(response)
  }

  async retryJobFromStep(jobId: string, stepName: string): Promise<JobActionResponse> {
    const response = await fetch(`${this.baseUrl}/api/jobs/${jobId}/retry-from/${encodeURIComponent(stepName)}`, {
      method: 'POST',
    })
    return readJson<JobActionResponse>(response)
  }

  async cancelJob(jobId: string): Promise<JobActionResponse> {
    const response = await fetch(`${this.baseUrl}/api/jobs/${jobId}/cancel`, {
      method: 'POST',
    })
    return readJson<JobActionResponse>(response)
  }

  async uploadVersionAsset(weaponId: string, versionId: string, input: AssetUploadRequest): Promise<AssetUploadResponse> {
    const response = await fetch(`${this.baseUrl}/api/weapons/${weaponId}/versions/${versionId}/assets`, {
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
    const response = await fetch(`${this.baseUrl}/api/weapons/${weaponId}/versions/${versionId}/activate`, {
      method: 'POST',
    })
    return readJson<WeaponDetail>(response)
  }

  async patchWeapon(weaponId: string, input: PatchWeaponRequest): Promise<CreateWeaponResponse> {
    const response = await fetch(`${this.baseUrl}/api/weapons/${weaponId}/patch`, {
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
    const response = await fetch(`${this.baseUrl}/api/weapons/${weaponId}/generate-3d`, {
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
    const response = await fetch(`${this.baseUrl}/api/weapons/${weaponId}/export-unity`, {
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
    const response = await fetch(`${this.baseUrl}/api/provider-settings`)
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
    const url = new URL(`${this.baseUrl}/api/jobs/${jobId}/events`)
    if (after) url.searchParams.set('after', after)
    const source = new EventSource(url.toString())
    source.onopen = handlers.onOpen ?? null
    source.addEventListener('job.event', (message) => {
      handlers.onEvent(JSON.parse((message as MessageEvent).data) as JobEvent)
    })
    source.addEventListener('job.error', (message) => {
      const parsed = JSON.parse((message as MessageEvent).data) as ApiErrorEnvelope
      if (parsed.error) handlers.onStreamError?.(parsed.error)
    })
    source.onerror = handlers.onError ?? null
    return () => source.close()
  }
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

export const forgeApi = new ForgeApiClient()
