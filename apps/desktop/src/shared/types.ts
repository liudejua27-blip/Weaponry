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

export type ApiErrorEnvelope = {
  error: {
    code: string
    message: string
    recoverable: boolean
    details: Record<string, unknown>
  }
}
