import { isTauriRuntime } from './agentSupervisor.js'

type NormalizedRegion = { left: number; top: number; right: number; bottom: number }

export type VisionReferenceRole =
  | 'primary_silhouette'
  | 'structure'
  | 'material'
  | 'surface'
  | 'local_detail'
  | 'style'
  | 'multiview'
  | 'existing_asset'

export type MultimodalDesignRequest = {
  schema_version: 'MultimodalDesignRequest@1'
  request_id: string
  project_id: string
  turn_id: string
  domain_pack_id: string
  instruction: string
  reference_inputs: Array<{
    evidence_id: string
    evidence_sha256: string
    role: VisionReferenceRole
    view_id?: string | null
    region?: NormalizedRegion | null
  }>
  active_asset_version_id?: string | null
  selection?: {
    part_ids: string[]
    material_zone_ids: string[]
    reference_region?: NormalizedRegion | null
  } | null
  locks: {
    preserve_geometry: boolean
    preserve_material_surface: boolean
    locked_part_ids: string[]
    locked_material_zone_ids: string[]
  }
}

export type VisualEvidenceGraph = {
  schema_version: 'VisualEvidenceGraph@1'
  graph_id: string
  request_id: string
  request_sha256: string
  project_id: string
  domain_pack_id: string
  provider: {
    provider_id: string
    model_id: string
    provider_response_sha256: string
    analyzed_at: string
  }
  claims: Array<{
    claim_id: string
    level: 'macro' | 'meso' | 'micro'
    status: 'observed' | 'inferred' | 'unknown'
    target: 'geometry' | 'assembly' | 'material' | 'surface' | 'style' | 'evaluation_only'
    description: string
    critical: boolean
    confidence_bps: number
    source_evidence_ids: string[]
    source_view_id?: string | null
    source_region?: NormalizedRegion | null
  }>
}

export type VisionEvidenceProviderConfig = {
  baseUrl: string
  model: string
  configured: boolean
  storage: 'private_secret_file'
  requiresOsPrompt: false
}

export async function getVisionEvidenceProviderConfig(): Promise<VisionEvidenceProviderConfig | null> {
  if (!isTauriRuntime()) return null
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<VisionEvidenceProviderConfig>('get_vision_evidence_provider_config')
}

export async function saveVisionEvidenceProviderConfig(input: {
  baseUrl: string
  model: string
  apiKey: string
}): Promise<VisionEvidenceProviderConfig> {
  if (!isTauriRuntime()) throw new Error('浏览器预览不能保存视觉理解服务密钥。')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<VisionEvidenceProviderConfig>('save_vision_evidence_provider_config', {
    request: {
      base_url: input.baseUrl,
      model: input.model,
      api_key: input.apiKey,
    },
  })
}

export async function clearVisionEvidenceProviderConfig(): Promise<VisionEvidenceProviderConfig> {
  if (!isTauriRuntime()) throw new Error('浏览器预览不能清除视觉理解服务密钥。')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<VisionEvidenceProviderConfig>('clear_vision_evidence_provider_config')
}

export async function analyzeVisualEvidence(
  clientRequestId: string,
  request: MultimodalDesignRequest,
): Promise<{ request: MultimodalDesignRequest; visualEvidenceGraph: VisualEvidenceGraph }> {
  if (!isTauriRuntime()) throw new Error('参考图视觉理解只在 Forge Studio 桌面端可用。')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<{ request: MultimodalDesignRequest; visualEvidenceGraph: VisualEvidenceGraph }>('analyze_visual_evidence', {
    input: { client_request_id: clientRequestId, request },
  })
}

export async function cancelVisualEvidenceAnalysis(clientRequestId: string): Promise<boolean> {
  if (!isTauriRuntime()) return false
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<boolean>('cancel_visual_evidence_analysis', {
    clientRequestId,
  })
}

export type VisualReferenceComparisonAuthorization = {
  authorizationId: string
  authorizationBindingSha256: string
  expiresAtUnixMs: number
  maximumCalls: 3
  maximumVariableCostMicrousd: 100000
}

export async function authorizeVisualReferenceComparison(
  clientRequestId: string,
  request: MultimodalDesignRequest,
  visualEvidenceGraph: VisualEvidenceGraph,
): Promise<VisualReferenceComparisonAuthorization> {
  if (!isTauriRuntime()) throw new Error('参考图视觉比较授权只在 Forge Studio 桌面端可用。')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<VisualReferenceComparisonAuthorization>('authorize_visual_reference_comparison', {
    input: {
      client_request_id: clientRequestId,
      request,
      visual_evidence_graph: visualEvidenceGraph,
      maximum_calls: 3,
      maximum_variable_cost_microusd: 100000,
    },
  })
}
