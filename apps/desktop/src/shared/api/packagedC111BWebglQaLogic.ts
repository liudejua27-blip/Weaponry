export const C111B_AGENT_LINK_ROLE = 'upper_link_form'
export const C111B_AGENT_LINK_ZONE = 'zone_arm_link_shell'

export type C111bLinkCandidate = {
  partId: string | null
  role: string | null
  materialZoneIds: readonly string[]
}

export function assertC111bSelectionCardIdentity(
  displayedAssetVersionId: string | undefined,
  activeAssetVersionId: string | undefined,
  expectedAssetVersionId: string,
): void {
  if (
    displayedAssetVersionId !== expectedAssetVersionId
    || activeAssetVersionId !== expectedAssetVersionId
  ) throw new Error('C111B_AGENT_SELECTION_CARD_VERSION_DRIFT')
}

export function resolveC111bLinkCandidate(candidates: readonly C111bLinkCandidate[]): string {
  const matches = candidates.filter((candidate) => (
    candidate.role === C111B_AGENT_LINK_ROLE
    && candidate.materialZoneIds.includes(C111B_AGENT_LINK_ZONE)
  ))
  if (matches.length === 0) throw new Error('C111B_AGENT_LINK_PART_MISSING')
  if (matches.some((candidate) => !candidate.partId)) {
    throw new Error('C111B_AGENT_LINK_PART_ID_MISSING')
  }
  // C111B intentionally expands several visible operations into Parts that
  // share one globally unique Material Zone. The durable edit targets that
  // Zone, not a local Part operation, so use the canonical stable Part ID only
  // as a deterministic selection anchor for the visible workflow.
  return [...new Set(matches.map((candidate) => candidate.partId as string))].sort()[0]!
}

export function normalizeC111bAdornmentErrorCode(errorCode: string | null | undefined): string {
  return errorCode && /^[A-Z0-9_]{1,80}$/.test(errorCode)
    ? `C111B_AGENT_ADORNMENT_${errorCode}`
    : 'C111B_AGENT_ADORNMENT_PREVIEW_FAILED'
}

export function normalizeC111bAdornmentRetainErrorCode(errorCode: string | null | undefined): string {
  return errorCode && /^[A-Z0-9_]{1,80}$/.test(errorCode)
    ? `C111B_AGENT_ADORNMENT_RETAIN_${errorCode}`
    : 'C111B_AGENT_ADORNMENT_RETAIN_FAILED'
}

export type C111bStageFailureCode =
  | 'C111B_AGENT_PRODUCTION_VIEWPORT_FAILED'
  | 'C111B_AGENT_READBACK_FAILED'
  | 'C111B_AGENT_EXPORT_FAILED'

export function normalizeC111bStageError(
  caught: unknown,
  fallbackCode: C111bStageFailureCode,
): string {
  const raw = typeof caught === 'object' && caught !== null && 'code' in caught
    && typeof caught.code === 'string'
    ? caught.code
    : caught instanceof Error
      ? caught.message
      : typeof caught === 'string'
        ? caught
        : ''
  if (/^C111B_[A-Z0-9_]{1,120}$/.test(raw)) return raw
  const suffix = raw
    .toUpperCase()
    .replace(/[^A-Z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
  if (!suffix) return fallbackCode
  const maxSuffixLength = 126 - fallbackCode.length - 1
  return `${fallbackCode}_${suffix.slice(0, maxSuffixLength).replace(/_+$/g, '')}`
}

export type C111bAdornmentPreviewOutcome =
  | { status: 'activation_required' }
  | { status: 'preview_ready'; changeSetId: string }

export type C111bAdornmentFlowPort = {
  preview: () => Promise<C111bAdornmentPreviewOutcome>
  enable: () => Promise<void>
  retain: (changeSetId: string) => Promise<void>
}

export async function driveC111bAdornmentFlow(port: C111bAdornmentFlowPort): Promise<string> {
  let outcome = await port.preview()
  if (outcome.status === 'activation_required') {
    await port.enable()
    outcome = await port.preview()
    if (outcome.status === 'activation_required') {
      throw new Error('C111B_AGENT_ADORNMENT_ACTIVATION_REPEATED')
    }
  }
  await port.retain(outcome.changeSetId)
  return outcome.changeSetId
}
