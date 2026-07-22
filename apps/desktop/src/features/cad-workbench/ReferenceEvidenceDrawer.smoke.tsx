import {
  cancelReferencePreviewOnce,
  isReferenceRebuildExactLineage,
  isReferenceDrawerCloseShortcut,
  isReferencePreviewBaseStale,
  referenceEvidenceScopeKey,
  readReferenceRebuildPreviewLineage,
  readReferenceRebuildRetainLineage,
  readReferenceRebuildExactLineage,
  readReferenceRebuildComparisonPlan,
  unavailableReferenceEvidenceAdapter,
  type ReferenceEvidenceAdapter,
  type ReferenceEvidenceRecord,
  type ReferenceEvidenceTarget,
} from './ReferenceEvidenceDrawer.js'

function assert(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }

const target: ReferenceEvidenceTarget = {
  projectId: 'project_reference_smoke',
  domainPackId: 'pack_robotic_arm_concept',
  baseAssetVersionId: 'assetver_reference_smoke',
}

const evidence: ReferenceEvidenceRecord = {
  evidenceId: 'reference_evidence_smoke',
  contentSha256: 'a'.repeat(64),
  kind: 'image',
  fileName: 'authorized-arm.png',
  sourceStatement: '本人制作并上传。',
  licenseStatement: '本人拥有本项目概念重建使用权。',
  missingViews: ['rear'],
  uncertainties: ['后部不可见'],
}

const sourceSha256 = 'a'.repeat(64)
const resultSha256 = 'b'.repeat(64)

function previewLineageEnvelope() {
  return {
    reference_guided_rebuild_plan: {
      rebuild_plan_id: 'rebuildplan_reference_smoke',
      evidence_id: evidence.evidenceId,
      status: 'previewed',
      preview_change_set_id: 'changeset_reference_smoke',
      confirmed_asset_version_id: null,
    },
    reference_surface_analysis: {
      analysis_id: 'refsrfanalysis_reference_smoke',
      rebuild_plan_id: 'rebuildplan_reference_smoke',
      evidence_id: evidence.evidenceId,
      source_object_sha256: sourceSha256,
      fidelity_ceiling: 'single_image_visible_surface_only',
    },
  }
}

export async function runReferenceEvidenceDrawerSmoke(): Promise<void> {
  const unavailable = await unavailableReferenceEvidenceAdapter.previewRebuild(target, evidence)
  assert(unavailable.status === 'unavailable' && unavailable.message.includes('当前模型没有变化'), 'unavailable R007 adapter must fail closed')
  assert(
    referenceEvidenceScopeKey(target) === referenceEvidenceScopeKey({ ...target, baseAssetVersionId: 'assetver_reference_confirmed' }),
    'a same-project/domain base advance during retain must preserve the current evidence and confirmed lineage',
  )
  assert(
    referenceEvidenceScopeKey(target) !== referenceEvidenceScopeKey({ ...target, projectId: 'project_other' }),
    'switching Project must reset the reference drawer scope',
  )
  assert(
    isReferencePreviewBaseStale('assetver_reference_smoke', 'assetver_external_advance')
      && !isReferencePreviewBaseStale('assetver_reference_smoke', 'assetver_reference_smoke'),
    'an unconfirmed preview must become non-confirmable when its active base advances externally',
  )

  const comparison = readReferenceRebuildComparisonPlan({
    reference_guided_rebuild_plan: {
      retained_evidence: ['保留可见连杆比例'],
      intended_differences: ['以受限 Recipe 重建，不复制参考网格'],
      unresolved_uncertainties: ['后部不可见'],
      future_structural_field: { ignored: true },
    },
  })
  assert(comparison?.retainedEvidence[0] === '保留可见连杆比例', 'R007B comparison must read retained evidence only from the existing plan field')
  assert(comparison?.intendedDifferences[0]?.includes('Recipe'), 'R007B comparison must read intended differences only from the existing plan field')
  assert(comparison?.unresolvedUncertainties[0] === '后部不可见', 'R007B comparison must preserve unresolved uncertainty')
  assert(readReferenceRebuildComparisonPlan({ reference_guided_rebuild_plan: { unrelated: true } }) === null, 'R007B comparison must not invent missing plan evidence')

  const previewLineage = readReferenceRebuildExactLineage(previewLineageEnvelope(), {
    evidenceId: evidence.evidenceId,
    sourceObjectSha256: sourceSha256,
    previewChangeSetId: 'changeset_reference_smoke',
  })
  assert(
    previewLineage?.status === 'previewed'
      && previewLineage.resultGlbSha256 === null
      && previewLineage.fidelityCeiling === 'single_image_visible_surface_only',
    'R007B preview must expose only an exact frozen identity and its bounded evidence range',
  )
  assert(
    readReferenceRebuildPreviewLineage({
      status: 'preview_ready',
      changeSetId: 'changeset_reference_smoke',
      summary: '预览已就绪。',
      lineage: previewLineage,
    })?.status === 'previewed',
    'drawer must only accept a preview whose exact lineage binds the displayed ChangeSet',
  )
  assert(
    readReferenceRebuildPreviewLineage({
      status: 'preview_ready',
      changeSetId: 'changeset_other',
      summary: '预览已就绪。',
      lineage: previewLineage,
    }) === null,
    'drawer must fail closed when a preview lineage belongs to another ChangeSet',
  )

  const confirmedEnvelope = {
    ...previewLineageEnvelope(),
    reference_guided_rebuild_plan: {
      ...previewLineageEnvelope().reference_guided_rebuild_plan,
      status: 'confirmed',
      confirmed_asset_version_id: 'assetver_reference_result_smoke',
    },
    reference_result_pair: {
      source_object_sha256: sourceSha256,
      result_asset_version_id: 'assetver_reference_result_smoke',
      result_glb_sha256: resultSha256,
    },
  }
  const confirmedLineage = readReferenceRebuildExactLineage(confirmedEnvelope)
  assert(
    confirmedLineage?.status === 'confirmed'
      && confirmedLineage.confirmedAssetVersionId === 'assetver_reference_result_smoke'
      && confirmedLineage.resultGlbSha256 === resultSha256,
    'R007B confirmation must bind the immutable result asset and a distinct production GLB hash',
  )
  assert(
    readReferenceRebuildRetainLineage({
      status: 'retained',
      summary: '已创建新版本。',
      lineage: confirmedLineage,
    })?.resultGlbSha256 === resultSha256,
    'drawer must only accept a retained result after Rust binds a distinct production GLB',
  )
  assert(
    readReferenceRebuildRetainLineage({
      status: 'retained',
      summary: '已创建新版本。',
      lineage: previewLineage,
    }) === null,
    'drawer must not relabel a preview lineage as a retained result',
  )

  const rejectedEnvelope = {
    ...previewLineageEnvelope(),
    reference_guided_rebuild_plan: {
      ...previewLineageEnvelope().reference_guided_rebuild_plan,
      status: 'rejected',
    },
  }
  assert(
    readReferenceRebuildExactLineage(rejectedEnvelope)?.status === 'rejected',
    'R007B rejected preview must remain visible as a rejected identity without a result asset',
  )

  const mismatchedSource = previewLineageEnvelope()
  mismatchedSource.reference_surface_analysis.source_object_sha256 = resultSha256
  assert(
    readReferenceRebuildExactLineage(mismatchedSource, { sourceObjectSha256: sourceSha256 }) === null,
    'R007B lineage must reject a source hash that is not the saved evidence hash',
  )
  const copiedResult = {
    ...confirmedEnvelope,
    reference_result_pair: { ...confirmedEnvelope.reference_result_pair, result_glb_sha256: sourceSha256 },
  }
  assert(
    readReferenceRebuildExactLineage(copiedResult) === null,
    'R007B lineage must reject a result GLB that reuses the source object hash',
  )
  const missingConfirmedResult = {
    ...confirmedEnvelope,
    reference_result_pair: { ...confirmedEnvelope.reference_result_pair, result_glb_sha256: null },
  }
  assert(
    readReferenceRebuildExactLineage(missingConfirmedResult) === null,
    'R007B lineage must reject a confirmed plan without an exact production GLB hash',
  )
  const forbiddenVisualField = {
    ...previewLineageEnvelope(),
    reference_surface_analysis: {
      ...previewLineageEnvelope().reference_surface_analysis,
      similarity_score: 99,
    },
  }
  assert(
    readReferenceRebuildExactLineage(forbiddenVisualField) === null,
    'R007B lineage must fail closed on similarity, score, vision, pixel or provider fields',
  )
  assert(
    isReferenceRebuildExactLineage(confirmedLineage),
    'drawer must only render the already-validated frozen exact-lineage projection',
  )
  assert(
    !isReferenceRebuildExactLineage({ ...confirmedLineage, provider_score: 1 }),
    'drawer must reject adapter data that attempts to add a provider or score claim',
  )

  let cancelCalls = 0
  let releaseCancel!: () => void
  const cancelBarrier = new Promise<void>((resolve) => { releaseCancel = resolve })
  const cancelGuard = { current: false }
  const deferredCancelAdapter = {
    cancel: async () => {
      cancelCalls += 1
      await cancelBarrier
    },
  }
  const pendingCancel = cancelReferencePreviewOnce(
    deferredCancelAdapter,
    'changeset_reference_smoke',
    cancelGuard,
  )
  assert(cancelGuard.current && cancelCalls === 1, 'cancel must remain pending until the adapter reaches its authoritative terminal state')
  const duplicateCancel = await cancelReferencePreviewOnce(
    deferredCancelAdapter,
    'changeset_reference_smoke',
    cancelGuard,
  )
  assert(duplicateCancel.status === 'pending' && cancelCalls === 1, 'repeated close or Escape must not issue a second reject while cancellation is pending')
  releaseCancel()
  const cancelled = await pendingCancel
  assert(cancelled.status === 'cancelled' && !cancelGuard.current, 'successful cancellation must release the pending guard only after adapter completion')

  let failOnce = true
  const retryGuard = { current: false }
  const retryAdapter = {
    cancel: async () => {
      if (failOnce) {
        failOnce = false
        throw new Error('Snapshot refresh failed')
      }
    },
  }
  const failedCancel = await cancelReferencePreviewOnce(retryAdapter, 'changeset_reference_smoke', retryGuard)
  assert(
    failedCancel.status === 'failed'
      && failedCancel.message.includes('预览仍保留，请重试')
      && !retryGuard.current,
    'failed cancellation must keep the preview explicit and release the guard for retry',
  )
  assert(
    (await cancelReferencePreviewOnce(retryAdapter, 'changeset_reference_smoke', retryGuard)).status === 'cancelled',
    'a failed cancellation must be retryable',
  )
  assert(
    isReferenceDrawerCloseShortcut('Escape') && !isReferenceDrawerCloseShortcut('Enter'),
    'Escape must use the same close lifecycle while unrelated keys do not close the drawer',
  )

  let previewEvidenceId = ''
  let retainedId = ''
  let cancelledId = ''
  let invalidated = 0
  const mock: ReferenceEvidenceAdapter = {
    invalidate() { invalidated += 1 },
    async createEvidence() { return { status: 'created', evidence } },
    async previewRebuild(_target, receivedEvidence) {
      previewEvidenceId = receivedEvidence.evidenceId
      return { status: 'preview_ready', changeSetId: 'changeset_reference_smoke', summary: '真实参考重建预览已就绪。' }
    },
    async retain(changeSetId) { retainedId = changeSetId; return { status: 'retained', summary: '已创建新版本。' } },
    async cancel(changeSetId) { cancelledId = changeSetId },
  }
  const created = await mock.createEvidence({
    target,
    // The adapter mock does not read bytes; a typed sentinel keeps this smoke
    // portable to Node runners without browser File globals.
    file: { name: evidence.fileName } as File,
    sourceStatement: evidence.sourceStatement,
    licenseStatement: evidence.licenseStatement,
    missingViews: evidence.missingViews,
    referenceClass: 'single_image',
    notes: '后部遮挡。',
  })
  assert(created.status === 'created' && created.evidence.contentSha256.length === 64, 'evidence requires immutable content hash before rebuild')
  if (created.status !== 'created') return
  const preview = await mock.previewRebuild(target, created.evidence)
  assert(preview.status === 'preview_ready' && preview.changeSetId === 'changeset_reference_smoke' && previewEvidenceId === evidence.evidenceId, 'rebuild preview must bind the precise evidence id')
  await mock.retain('changeset_reference_smoke')
  await mock.cancel('changeset_reference_smoke')
  mock.invalidate?.()
  assert(retainedId === 'changeset_reference_smoke' && cancelledId === 'changeset_reference_smoke' && invalidated === 1, 'R007 must cover retain, cancel and late-response invalidation')
}
