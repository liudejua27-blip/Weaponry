import { isTauriRuntime } from './agentSupervisor.js'

export type VisualQualityTier = 'fast_preview' | 'standard_asset' | 'collectible_asset'

export type VisualProviderConfig = {
  provider: string
  configured: boolean
  storage: string
  requiresOsPrompt: boolean
}

export type VisualInputEvidence = {
  evidence_id: string
  object_sha256: string
  media_type: 'image/png' | 'image/jpeg' | 'image/webp' | 'model/gltf-binary'
  rights_confirmed: boolean
  remote_processing_authorized: boolean
}

export type VisualDesignBrief = {
  schema_version: 'VisualDesignBrief@1'
  brief_id: string
  project_id: string
  turn_id: string
  input_kind: 'text' | 'image' | 'text_and_image'
  user_intent_sha256: string
  object_class: string
  visual_summary: string
  style_terms: string[]
  material_terms: string[]
  input_evidence: VisualInputEvidence[]
}

export type ConceptReferenceArtifact = {
  schema_version: 'ConceptReferenceArtifact@1'
  reference_id: string
  brief_id: string
  image_object_sha256: string
  media_type: string
  provider_id: string
  provider_job_id: string
  isolated_subject: boolean
  clean_background: boolean
  hidden_surface_policy: 'multiview_supported' | 'ai_inferred'
}

export type NeuralVisualGlbInspection = {
  sha256: string
  byte_size: number
  triangle_count: number
  mesh_count: number
  primitive_count: number
  material_count: number
  node_count: number
  pbr_channels: Array<'base_color' | 'normal' | 'roughness' | 'metallic' | 'ambient_occlusion' | 'emissive'>
  every_primitive_has_uv0: boolean
  every_primitive_has_tangent: boolean
}

export type GenerateVisualAssetInput = {
  client_request_id: string
  project_id: string
  turn_id: string
  user_intent: string
  quality_tier: VisualQualityTier
  input_evidence?: VisualInputEvidence[]
}

export type GenerateVisualAssetOutput = {
  brief: VisualDesignBrief
  conceptReference: ConceptReferenceArtifact
  inspection: NeuralVisualGlbInspection
  conceptPngBase64: string
  glbBase64: string
}

export type VisualGenerationProgress = {
  clientRequestId: string
  stage: 'understanding' | 'concept_image' | 'neural_3d' | 'readback' | 'recovery' | 'ready'
  detail: string
}

export type VisualRemoteJobRecord = {
  schema_version: 'VisualRemoteJobRecord@1'
  client_request_id: string
  project_id: string
  turn_id: string
  state: { stage: 'concept_submitted' | 'neural_submitted' | 'completed' | 'failed' | 'cancelled' }
  created_at: string
  updated_at: string
}

export async function getVisualProviderConfig(): Promise<VisualProviderConfig | null> {
  if (!isTauriRuntime()) return null
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<VisualProviderConfig>('get_visual_provider_config')
}

export async function saveVisualProviderConfig(falApiKey: string): Promise<VisualProviderConfig> {
  if (!isTauriRuntime()) throw new Error('浏览器预览不能保存远程视觉生成密钥。')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<VisualProviderConfig>('save_visual_provider_config', {
    request: { fal_api_key: falApiKey },
  })
}

export async function clearVisualProviderConfig(): Promise<VisualProviderConfig> {
  if (!isTauriRuntime()) throw new Error('浏览器预览不能清除远程视觉生成密钥。')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<VisualProviderConfig>('clear_visual_provider_config')
}

export async function generateVisualAsset(input: GenerateVisualAssetInput): Promise<GenerateVisualAssetOutput> {
  if (!isTauriRuntime()) throw new Error('真实视觉资产生成只在 Forge Studio 桌面端可用。')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<GenerateVisualAssetOutput>('generate_visual_asset', { input })
}

export async function cancelVisualAssetGeneration(clientRequestId: string): Promise<boolean> {
  if (!isTauriRuntime()) return false
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<boolean>('cancel_visual_asset_generation', { clientRequestId })
}

export async function listRecoverableVisualAssetGenerations(
  projectId?: string,
): Promise<VisualRemoteJobRecord[]> {
  if (!isTauriRuntime()) return []
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<VisualRemoteJobRecord[]>('list_recoverable_visual_asset_generations', {
    projectId: projectId ?? null,
  })
}

export async function resumeVisualAssetGeneration(
  clientRequestId: string,
): Promise<GenerateVisualAssetOutput> {
  if (!isTauriRuntime()) throw new Error('浏览器预览不能恢复远程视觉生成。')
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<GenerateVisualAssetOutput>('resume_visual_asset_generation', { clientRequestId })
}

export async function listenVisualGenerationProgress(
  listener: (progress: VisualGenerationProgress) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) return () => undefined
  const { listen } = await import('@tauri-apps/api/event')
  return listen<VisualGenerationProgress>('forgecad://visual-generation-progress', (event) => {
    listener(event.payload)
  })
}
