import {
  projectReferenceGuidedRebuildPreviewProposal,
  projectReferenceGuidedRebuildPlanRead,
  type ReferenceGuidedRebuildPlanRead,
  type ReferenceResultPair,
} from './forgeApi.js'

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}

const sourceSha256 = 'a'.repeat(64)
const resultSha256 = 'b'.repeat(64)

const sealedRead: ReferenceGuidedRebuildPlanRead = {
  schema_version: 'ReferenceGuidedRebuildPlanRead@1',
  reference_guided_rebuild_plan: {
    schema_version: 'ReferenceGuidedRebuildPlan@1',
    rebuild_plan_id: 'rebuildplan_r007b_smoke',
    project_id: 'project_r007b_smoke',
    evidence_id: 'reference_r007b_smoke',
    base_asset_version_id: 'assetver_r007b_base',
    domain_pack_id: 'pack_robotic_arm_concept',
    recipe_id: 'recipe_c106_arm_gallery_industrial',
    recipe_registry_sha256: sourceSha256,
    rebuild_summary: '受限机械臂外观重建。',
    retained_evidence: ['可见连杆比例'],
    intended_differences: ['使用经过审阅的 Recipe'],
    unresolved_uncertainties: ['隐藏结构未知'],
    status: 'confirmed',
    preview_change_set_id: 'changeset_r007b_smoke',
    confirmed_asset_version_id: 'assetver_r007b_result',
    created_at: '2026-07-19T00:00:00Z',
    updated_at: '2026-07-19T00:00:01Z',
  },
  reference_surface_analysis: {
    schema_version: 'ReferenceSurfaceAnalysis@1',
    analysis_id: 'analysis_r007b_smoke',
    rebuild_plan_id: 'rebuildplan_r007b_smoke',
    evidence_id: 'reference_r007b_smoke',
    source_object_sha256: sourceSha256,
    domain_pack_id: 'pack_robotic_arm_concept',
    target_root_recipe: {
      schema_version: 'ComponentRecipeRef@1',
      recipe_id: 'recipe_c106_arm_gallery_industrial',
      version: 1,
      recipe_sha256: resultSha256,
    },
    c106_registry_sha256: sourceSha256,
    surface_skill_id: 'skill_a005_surface_adornment',
    surface_skill_version: 2,
    surface_skill_sha256: resultSha256,
    fidelity_ceiling: 'single_image_visible_surface_only',
    bindings: [{
      binding_id: 'binding_r007b_smoke',
      observation_kind: 'silhouette',
      observation_index: 0,
      target_recipe: {
        schema_version: 'ComponentRecipeRef@1',
        recipe_id: 'recipe_c106_arm_gallery_industrial',
        version: 1,
        recipe_sha256: resultSha256,
      },
      target_part_role: 'arm_link',
      target_material_zone_id: 'zone_main',
      target_surface_slot_id: 'surface_main',
    }],
    retained_observation_kinds: ['silhouette'],
    intentionally_changed: ['reviewed_recipe_component_substitution'],
    unresolved: ['hidden_structure'],
    created_at: '2026-07-19T00:00:00Z',
  },
  reference_result_pair: {
    source_object_sha256: sourceSha256,
    result_asset_version_id: 'assetver_r007b_result',
    result_glb_sha256: resultSha256,
  },
}

const invalidResultPair: ReferenceResultPair = {
  source_object_sha256: sourceSha256,
  result_asset_version_id: null,
  // @ts-expect-error R007B cannot silently widen a frozen result hash to a number.
  result_glb_sha256: 42,
}
void invalidResultPair

export function runR007BForgeApiProjectionSmoke(): void {
  const accepted = projectReferenceGuidedRebuildPlanRead(sealedRead)
  assert(accepted?.reference_surface_analysis?.analysis_id === 'analysis_r007b_smoke', 'R007B projection must retain the sealed analysis identity')
  assert(accepted.reference_result_pair.result_glb_sha256 === resultSha256, 'R007B projection must preserve the Rust-provided result GLB identity')

  const mismatchedEvidence = structuredClone(sealedRead)
  assert(mismatchedEvidence.reference_surface_analysis !== null, 'smoke fixture requires a sealed R007B analysis')
  mismatchedEvidence.reference_surface_analysis.evidence_id = 'reference_other'
  assert(projectReferenceGuidedRebuildPlanRead(mismatchedEvidence) === null, 'R007B projection must reject analysis/evidence identity mismatch')

  const copiedSource = structuredClone(sealedRead)
  copiedSource.reference_result_pair.result_glb_sha256 = sourceSha256
  assert(projectReferenceGuidedRebuildPlanRead(copiedSource) === null, 'R007B projection must reject a result that claims the source GLB hash')

  const malformedResultHash = structuredClone(sealedRead)
  malformedResultHash.reference_result_pair.result_glb_sha256 = 'not-a-sha256'
  assert(projectReferenceGuidedRebuildPlanRead(malformedResultHash) === null, 'R007B projection must reject malformed Rust provenance')

  const draft = structuredClone(sealedRead)
  draft.reference_guided_rebuild_plan.status = 'draft'
  delete draft.reference_guided_rebuild_plan.preview_change_set_id
  delete draft.reference_guided_rebuild_plan.confirmed_asset_version_id
  draft.reference_result_pair.result_asset_version_id = null
  draft.reference_result_pair.result_glb_sha256 = null
  assert(projectReferenceGuidedRebuildPlanRead(draft) !== null, 'R007B projection must accept Rust-omitted optional plan fields before preview')

  const draftAnalysis = draft.reference_surface_analysis
  assert(draftAnalysis !== null, 'R007B POST fixture requires a frozen draft analysis')
  const draftBaseAssetVersionId = draft.reference_guided_rebuild_plan.base_asset_version_id
  assert(typeof draftBaseAssetVersionId === 'string', 'R007B POST fixture requires an editable base asset')
  const previewProposal = {
    change_set_id: 'changeset_r007b_smoke',
    project_id: draft.reference_guided_rebuild_plan.project_id,
    base_asset_version_id: draftBaseAssetVersionId,
    summary: '受限机械臂参考重建。',
    operations: [{ operation_id: 'op_r007b_smoke', op: 'replace_part' }],
    status: 'proposed',
    created_at: draft.reference_guided_rebuild_plan.created_at,
    updated_at: draft.reference_guided_rebuild_plan.updated_at,
    reference_guided_rebuild_plan: draft.reference_guided_rebuild_plan,
    reference_surface_analysis: draftAnalysis,
  }
  const acceptedProposal = projectReferenceGuidedRebuildPreviewProposal(previewProposal)
  assert(
    acceptedProposal?.changeSet.change_set_id === 'changeset_r007b_smoke'
      && acceptedProposal.planRead.reference_surface_analysis?.analysis_id === 'analysis_r007b_smoke'
      && acceptedProposal.planRead.reference_result_pair.result_glb_sha256 === null,
    'R007B POST must use the same sealed plan-read projection with an explicit draft-only null result pair',
  )

  const missingFrozenAnalysis = { ...previewProposal, reference_surface_analysis: null }
  assert(
    projectReferenceGuidedRebuildPreviewProposal(missingFrozenAnalysis) === null,
    'R007B POST must fail closed when a ChangeSet lacks the frozen surface analysis',
  )

  const crossProjectProposal = { ...previewProposal, project_id: 'project_other' }
  assert(
    projectReferenceGuidedRebuildPreviewProposal(crossProjectProposal) === null,
    'R007B POST must reject a ChangeSet whose project differs from its sealed plan',
  )
}
