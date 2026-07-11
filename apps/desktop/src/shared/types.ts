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

export type ApiErrorEnvelope = {
  error: {
    code: string
    message: string
    recoverable: boolean
    details: Record<string, unknown>
  }
}
