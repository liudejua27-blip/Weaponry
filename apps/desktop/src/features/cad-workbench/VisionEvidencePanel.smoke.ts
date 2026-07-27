import {
  buildMultimodalDesignRequest,
  validateVisionEvidenceSelection,
  visionConnectionLabel,
} from './VisionEvidencePanel.js'
import type { ReferenceEvidenceRecord, ReferenceEvidenceTarget } from './referenceEvidenceDrawerLogic.js'

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

const target: ReferenceEvidenceTarget = {
  projectId: 'project_pv006b_workbench',
  domainPackId: 'pack_robotic_arm_concept',
  baseAssetVersionId: 'assetver_pv006b_active',
}

const front: ReferenceEvidenceRecord = {
  evidenceId: 'refevid_pv006b_front',
  contentSha256: 'a'.repeat(64),
  kind: 'image',
  fileName: 'front.png',
  sourceStatement: 'test fixture',
  licenseStatement: 'test fixture',
  missingViews: [],
  uncertainties: [],
}

const side: ReferenceEvidenceRecord = {
  ...front,
  evidenceId: 'refevid_pv006b_side',
  contentSha256: 'b'.repeat(64),
  fileName: 'side.png',
}

export function runVisionEvidencePanelSmoke(): void {
  assert(
    visionConnectionLabel(true, false) === '已配置·未验证',
    'saving credentials must not be presented as a verified provider connection',
  )
  assert(
    visionConnectionLabel(true, true) === '已验证',
    'only a completed visual analysis may present a verified connection',
  )
  assert(
    validateVisionEvidenceSelection('multiview', [front], '保持主体比例')?.includes('至少需要两份'),
    'multiview must fail closed when only one sealed image is selected',
  )
  assert(
    validateVisionEvidenceSelection('multiview', [front, side], '保持主体比例') === null,
    'multiview must accept two distinct sealed images',
  )
  const request = buildMultimodalDesignRequest({
    requestId: 'mmreq_pv006b_workbench',
    turnId: 'turn_pv006b_workbench',
    target,
    instruction: '保持几何，只借鉴蓝色发光流线。',
    evidences: [front],
    role: 'surface',
    activeAssetVersionId: target.baseAssetVersionId,
    selectedPartId: 'part_forearm',
    selectedMaterialZoneId: 'zone_shell',
    preserveGeometry: true,
    preserveMaterialSurface: false,
  })
  assert(request.reference_inputs.length === 1, 'the workbench must bind the exact selected evidence only')
  assert(request.reference_inputs[0]?.evidence_sha256 === front.contentSha256, 'the workbench must send the sealed evidence hash')
  assert(request.selection?.part_ids[0] === 'part_forearm', 'local editing must carry the selected Part identity')
  assert(request.selection?.material_zone_ids[0] === 'zone_shell', 'local editing must carry the selected Material Zone identity')
  assert(request.locks.preserve_geometry, 'surface-only editing must lock current geometry')
}
