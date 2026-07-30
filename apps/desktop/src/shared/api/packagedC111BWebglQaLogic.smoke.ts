import {
  assertC111bSelectionCardIdentity,
  driveC111bAdornmentFlow,
  normalizeC111bAdornmentErrorCode,
  normalizeC111bAdornmentRetainErrorCode,
  normalizeC111bStageError,
  resolveC111bLinkCandidate,
  type C111bAdornmentPreviewOutcome,
} from './packagedC111BWebglQaLogic.js'

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

function expectCode(run: () => unknown, code: string): void {
  try {
    run()
  } catch (caught) {
    assert(caught instanceof Error && caught.message === code, `expected ${code}`)
    return
  }
  throw new Error(`expected ${code}`)
}

export async function runPackagedC111bWebglQaLogicSmoke(): Promise<void> {
  assertC111bSelectionCardIdentity('asset_v1', 'asset_v1', 'asset_v1')
  expectCode(
    () => assertC111bSelectionCardIdentity('asset_v0', 'asset_v1', 'asset_v1'),
    'C111B_AGENT_SELECTION_CARD_VERSION_DRIFT',
  )
  const partId = resolveC111bLinkCandidate([
    { partId: 'part_lower', role: 'lower_link_form', materialZoneIds: ['zone_arm_link_shell'] },
    { partId: 'part_upper', role: 'upper_link_form', materialZoneIds: ['zone_arm_link_shell', 'zone_trim'] },
  ])
  assert(partId === 'part_upper', 'C111B must resolve the stable upper-link shell target')
  expectCode(() => resolveC111bLinkCandidate([]), 'C111B_AGENT_LINK_PART_MISSING')
  assert(resolveC111bLinkCandidate([
    { partId: 'part_upper_a', role: 'upper_link_form', materialZoneIds: ['zone_arm_link_shell'] },
    { partId: 'part_upper_b', role: 'upper_link_form', materialZoneIds: ['zone_arm_link_shell'] },
  ]) === 'part_upper_a', 'global Material Zone selection must use the canonical stable Part anchor')

  const activationTrace: string[] = []
  const activationOutcomes: C111bAdornmentPreviewOutcome[] = [
    { status: 'activation_required' },
    { status: 'preview_ready', changeSetId: 'changeset_second_preview' },
  ]
  const retained = await driveC111bAdornmentFlow({
    async preview() { activationTrace.push('preview'); return activationOutcomes.shift()! },
    async enable() { activationTrace.push('enable') },
    async retain(changeSetId) { activationTrace.push(`retain:${changeSetId}`) },
  })
  assert(retained === 'changeset_second_preview', 'retain must use the second preview ChangeSet')
  assert(
    activationTrace.join(',') === 'preview,enable,preview,retain:changeset_second_preview',
    'activation flow must be preview -> enable -> preview -> retain exactly once',
  )

  const readyTrace: string[] = []
  await driveC111bAdornmentFlow({
    async preview() { readyTrace.push('preview'); return { status: 'preview_ready', changeSetId: 'changeset_ready' } },
    async enable() { readyTrace.push('enable') },
    async retain(changeSetId) { readyTrace.push(`retain:${changeSetId}`) },
  })
  assert(readyTrace.join(',') === 'preview,retain:changeset_ready', 'enabled fast path must not activate again')

  assert(
    normalizeC111bAdornmentErrorCode('RESTRICTED_GEOMETRY_INPUT_INVALID')
      === 'C111B_AGENT_ADORNMENT_RESTRICTED_GEOMETRY_INPUT_INVALID',
    'stable geometry error must remain diagnostic',
  )
  assert(
    normalizeC111bAdornmentErrorCode('bad-error') === 'C111B_AGENT_ADORNMENT_PREVIEW_FAILED'
      && normalizeC111bAdornmentErrorCode('A'.repeat(81)) === 'C111B_AGENT_ADORNMENT_PREVIEW_FAILED'
      && normalizeC111bAdornmentErrorCode(null) === 'C111B_AGENT_ADORNMENT_PREVIEW_FAILED',
    'invalid adornment errors must fail closed to the bounded preview code',
  )
  assert(
    normalizeC111bAdornmentRetainErrorCode('CHANGE_SET_BASE_VERSION_MISMATCH')
      === 'C111B_AGENT_ADORNMENT_RETAIN_CHANGE_SET_BASE_VERSION_MISMATCH',
    'stable retain error must remain diagnostic',
  )
  assert(
    normalizeC111bAdornmentRetainErrorCode('bad-error') === 'C111B_AGENT_ADORNMENT_RETAIN_FAILED'
      && normalizeC111bAdornmentRetainErrorCode(null) === 'C111B_AGENT_ADORNMENT_RETAIN_FAILED',
    'invalid retain errors must fail closed to the bounded retain code',
  )
  assert(
    normalizeC111bStageError(
      'C111B packaged WebGL QA Agent production GLB inventory or hash is invalid.',
      'C111B_AGENT_READBACK_FAILED',
    ) === 'C111B_AGENT_READBACK_FAILED_C111B_PACKAGED_WEBGL_QA_AGENT_PRODUCTION_GLB_INVENTORY_OR_HASH_IS_INVALID',
    'native string rejections must retain a bounded diagnostic suffix',
  )
  assert(
    normalizeC111bStageError(
      { code: 'MODEL_GLB_SIZE_MISMATCH' },
      'C111B_AGENT_EXPORT_FAILED',
    ) === 'C111B_AGENT_EXPORT_FAILED_MODEL_GLB_SIZE_MISMATCH',
    'typed product errors must retain their stable code',
  )
  assert(
    normalizeC111bStageError(
      new Error('C111B_LIGHT_TIMEOUT'),
      'C111B_AGENT_PRODUCTION_VIEWPORT_FAILED',
    ) === 'C111B_LIGHT_TIMEOUT',
    'existing C111B errors must pass through unchanged',
  )
  assert(
    normalizeC111bStageError(null, 'C111B_AGENT_READBACK_FAILED') === 'C111B_AGENT_READBACK_FAILED'
      && normalizeC111bStageError('x'.repeat(400), 'C111B_AGENT_READBACK_FAILED').length <= 126,
    'unknown and oversized stage errors must remain stable and bounded',
  )
}
