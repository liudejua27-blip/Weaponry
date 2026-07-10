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
  DesignChangeSet,
  ChangeSetPreviewResponse,
  ChangeSetConfirmResponse,
  ChangeSetTimelineResponse,
  InspectConceptVersionRequest,
  QualityRunRecord,
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

  async getConceptVersion(versionId: string): Promise<ConceptVersionDetail> {
    const response = await fetch(`${this.baseUrl}/api/v1/versions/${versionId}`)
    return readJson<ConceptVersionDetail>(response)
  }

  async listModuleAssets(packId?: string): Promise<ModuleAssetListResponse> {
    const url = new URL(`${this.baseUrl}/api/v1/module-assets`)
    if (packId) url.searchParams.set('pack_id', packId)
    const response = await fetch(url)
    return readJson<ModuleAssetListResponse>(response)
  }

  getModuleAssetFileUrl(moduleId: string): string {
    return `${this.baseUrl}/api/v1/module-assets/${encodeURIComponent(moduleId)}/file`
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

export const forgeApi = new ForgeApiClient()
