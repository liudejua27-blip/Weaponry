import type * as Api from './generated/api-types'

export type JobStatus = Api.JobAcceptedResponse['status']
export type JobEvent = Api.JobEvent
export type JobSummary = Api.JobSummary
export type JobListResponse = Api.JobListResponse
export type JobActionAuditEntry = Api.JobActionAuditEntry
export type JobActionListResponse = Api.JobActionListResponse
export type JobActionResponse = Api.JobActionResponse
export type CreateWeaponRequest = Api.CreateWeaponRequest
export type CreateWeaponResponse = Api.JobAcceptedResponse
export type CreativeGraphResponse = Api.CreativeGraphResponse
export type CreativeInterpretationCandidate = Api.CreativeInterpretationCandidate
export type CreativeInterpretationRequest = Api.CreativeInterpretationRequest
export type CreativeInterpretationResponse = Api.CreativeInterpretationResponse
export type CreativeRecastConfirmRequest = Api.CreativeRecastConfirmRequest
export type CreativeRecastConfirmResponse = Api.CreativeRecastConfirmResponse
export type ExportUnityRequest = Api.ExportUnityRequest
export type Generate3DRequest = Api.Generate3DRequest
export type AssetFileResponse = Api.AssetFileResponse
export type AssetRevealResponse = Api.AssetRevealResponse
export type AssetUploadRequest = Api.AssetUploadRequest
export type AssetUploadResponse = Api.AssetUploadResponse
export type JobDetail = Api.JobDetail
export type JobRuntimeStateResponse = Api.JobRuntimeStateResponse
export type PatchWeaponRequest = Api.PatchWeaponRequest
export type WeaponDetail = Api.WeaponDetail
export type WeaponSummary = Api.WeaponSummary
export type ProviderSettings = Api.ProviderSettings
export type RuntimeRecoveryResponse = Api.RuntimeRecoveryResponse
export type RuntimeWorkOnceResponse = Api.RuntimeWorkOnceResponse
export type HealthResponse = Api.HealthResponse
export type ConceptProjectDetail = Api.ConceptProjectDetail
export type ConceptProjectSummary = Api.ConceptProjectSummary
export type ConceptProjectListResponse = Api.ConceptProjectListResponse
export type ConceptVersionDetail = Api.ConceptVersionDetail
export type ConceptVersionSummary = Api.ConceptVersionSummary
export type CreateConceptProjectRequest = Api.CreateConceptProjectRequest
export type ModuleAssetListResponse = Api.ModuleAssetListResponse
export type ModuleAssetRecord = Api.ModuleAssetRecord
export type UpdateModuleAssetCatalogMetadataRequest = Api.UpdateModuleAssetCatalogMetadataRequest
export type ModuleGraph = Api.ModuleGraph
export type ModuleGraphRecord = Api.ModuleGraphRecord
export type Transform = Api.Transform
export type DesignVariantListResponse = Api.DesignVariantListResponse
export type DesignVariantRecord = Api.DesignVariantRecord
export type InterpretDesignBriefRequest = Api.InterpretDesignBriefRequest
export type DesignBriefRecord = Api.DesignBriefRecord
export type GenerateDesignVariantsRequest = Api.GenerateDesignVariantsRequest
export type SelectDesignVariantRequest = Api.SelectDesignVariantRequest
export type CreateConceptExportRequest = Api.CreateConceptExportRequest
export type ConceptExportRecord = Api.ConceptExportRecord
export type DesignChangeSet = Api.DesignChangeSet
export type ProposeChangeSetRequest = Api.ProposeChangeSetRequest
export type ProposeConnectorSnapRequest = Api.ProposeConnectorSnapRequest
export type PlanDesignChangeSetRequest = Api.PlanDesignChangeSetRequest
export type PlannedChangeSetRecord = Api.PlannedChangeSetRecord
export type ChangeSetPreviewResponse = Api.ChangeSetPreviewResponse
export type ChangeSetConfirmResponse = Api.ChangeSetConfirmResponse
export type ChangeSetTimelineResponse = Api.ChangeSetTimelineResponse
export type ChangeSetTimelineItem = Api.ChangeSetTimelineItem
export type CreateChangeSetAuditExportRequest = Api.CreateChangeSetAuditExportRequest
export type ChangeSetAuditExportRecord = Api.ChangeSetAuditExportRecord
export type ChangeSetAuditExportListResponse = Api.ChangeSetAuditExportListResponse
export type InspectConceptVersionRequest = Api.InspectConceptVersionRequest
export type QualityRunRecord = Api.QualityRunRecord
export type ConceptJobRecord = Api.ConceptJobRecord
export type QualityFinding = Api.QualityFinding

export type AgentThreadStatus = 'idle' | 'active' | 'error' | 'archived'
export type AgentTurnStatus =
  | 'queued'
  | 'running'
  | 'waiting_for_approval'
  | 'waiting_for_clarification'
  | 'completed'
  | 'failed'
  | 'cancelled'
export type AgentItemType =
  | 'user_message'
  | 'assistant_message'
  | 'plan'
  | 'tool_call'
  | 'tool_result'
  | 'preview'
  | 'approval_request'
  | 'clarification'
  | 'artifact'
export type AgentItem = {
  item_id: string
  thread_id: string
  turn_id: string
  sequence: number
  item_type: AgentItemType
  status: 'pending' | 'completed' | 'failed' | 'cancelled'
  payload: Record<string, unknown>
  created_at: string
}
export type AgentApproval = {
  approval_id: string
  thread_id: string
  turn_id: string
  item_id: string
  action: string
  status: 'pending' | 'approved' | 'rejected'
  payload: Record<string, unknown>
  created_at: string
  resolved_at?: string | null
}
export type AgentTurn = {
  turn_id: string
  thread_id: string
  request_text: string
  status: AgentTurnStatus
  error_code?: string | null
  error_message?: string | null
  usage: Record<string, unknown>
  created_at: string
  updated_at: string
  items: AgentItem[]
  approvals: AgentApproval[]
}
export type AgentThreadSummary = {
  thread_id: string
  project_id?: string | null
  title: string
  status: AgentThreadStatus
  summary: string
  provider_id: string
  created_at: string
  updated_at: string
  last_turn_id?: string | null
}
export type AgentThreadDetail = AgentThreadSummary & { turns: AgentTurn[] }
export type AgentThreadListResponse = { items: AgentThreadSummary[]; next_cursor?: string | null }
export type CreateAgentThreadRequest = {
  client_request_id: string
  project_id?: string | null
  title?: string
  provider_id?: string
}
export type StartAgentTurnRequest = { client_request_id: string; message: string }
export type CreateAgentApprovalRequest = {
  client_request_id: string
  turn_id: string
  action: string
  payload?: Record<string, unknown>
}
export type ResolveAgentApprovalRequest = {
  client_request_id: string
  decision: 'approved' | 'rejected'
  note?: string
}
export type AgentApprovalResolution = { approval: AgentApproval; turn: AgentTurn }
export type AgentEvent = { sequence: number; thread_id: string; turn_id: string; item: AgentItem }
export type DomainPackManifest = Api.DomainPackManifest
export type MechanicalConceptPlan = Api.MechanicalConceptPlan
export type BuildAgentBlockoutRequest = {
  client_request_id: string
  plan: MechanicalConceptPlan
  direction_id: string
  variant_id?: string | null
  variation_index?: number
  presentation_profile?: 'quick_sketch' | 'showcase'
}
export type BuildAgentBlockoutResponse = Api.BuildAgentBlockoutResponse
export type RenderAgentBlockoutConceptPreviewRequest = {
  client_request_id: string
  plan: MechanicalConceptPlan
  direction_id: string
  variant_id?: string | null
  variation_index?: number
}
export type AgentBlockoutConceptPreview = Api.AgentBlockoutConceptPreview
export type SegmentAgentBlockoutRequest = {
  client_request_id: string
  plan: MechanicalConceptPlan
  direction_id: string
  variant_id?: string | null
  variation_index?: number
  presentation_profile?: 'quick_sketch' | 'showcase'
  artifact_id?: string | null
}
export type SegmentAgentBlockoutResponse = Api.SegmentAgentBlockoutResponse
export type AgentMaterialPreset = Api.AgentMaterialPreset
export type AgentMaterialTextureObject = Api.AgentMaterialTextureObject
export type AgentMaterialTextureListResponse = Api.AgentMaterialTextureListResponse
export type RegisterAgentMaterialTextureRequest = Api.RegisterAgentMaterialTextureRequest
export type CommitAgentBlockoutRequest = Api.CommitAgentBlockoutRequest
export type AgentAssetVersion = Api.AgentAssetVersion
export type EditableParameterBinding = Api.EditableParameterBinding
export type AgentPartEditOperation = Api.AgentPartEditOperation
export type ProposeAgentAssetChangeSetRequest = Api.ProposeAgentAssetChangeSetRequest
export type AgentAssetChangeSet = Api.AgentAssetChangeSet
export type AgentAssetChangeSetConfirmResponse = Api.AgentAssetChangeSetConfirmResponse
export type AgentAssetQualityReport = Api.AgentAssetQualityReport
export type AgentAssetExportResponse = Api.AgentAssetExportResponse
export type AgentAssetRenderView = Api.AgentAssetRenderView
export type AgentAssetRenderSet = Api.AgentAssetRenderSet
export type ImportAgentGlbRequest = Api.ImportAgentGlbRequest
export type ImportAgentGlbResponse = Api.ImportAgentGlbResponse
export type AgentProviderCheckResponse = Api.AgentProviderCheckResponse
export type AgentComponentRecord = Api.AgentComponentRecord
export type AgentComponentCompatibility = Api.AgentComponentCompatibility
export type AgentComponentCandidate = Api.AgentComponentCandidate
export type AgentStructureSuggestion = Api.AgentStructureSuggestion
export type AgentStructureSuggestionList = Api.AgentStructureSuggestionList
export type MechanicalStyleToken = Api.MechanicalStyleToken
export type ResolvedSemanticProportionOption = Api.ResolvedSemanticProportionOption
export type ResolvedSemanticProportionOptions = Api.ResolvedSemanticProportionOptions
export type ActiveDesignSnapshot = Api.ActiveDesignSnapshot
export type ActiveDesignNavigation = Api.ActiveDesignNavigation
export type NavigateActiveDesignRequest = Api.NavigateActiveDesignRequest
export type SelectActiveDesignRequest = Api.SelectActiveDesignRequest
export type ActiveDesignRenderPreset = Api.ActiveDesignRenderPreset
export type SetActiveDesignRenderPresetRequest = Api.SetActiveDesignRenderPresetRequest
export type ActiveDesignPartDisplay = Api.ActiveDesignPartDisplay
export type SetActiveDesignPartDisplayRequest = Api.SetActiveDesignPartDisplayRequest
export type ConvertLegacyActiveDesignRequest = Api.ConvertLegacyActiveDesignRequest
export type LegacyActiveDesignConversionResponse = Api.LegacyActiveDesignConversionResponse
export type SaveAgentComponentRequest = {
  client_request_id: string
  part_id: string
  display_name: string
  description?: string
}

export type ApiErrorEnvelope = {
  error: {
    code: string
    message: string
    recoverable: boolean
    details: Record<string, unknown>
  }
}
