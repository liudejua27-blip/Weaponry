import { buildAgentTurnRequestPayload } from './agentTurnRequestPayload.js'

const SHA256 = 'a'.repeat(64)

export function runAgentTurnRequestPayloadSmoke(): void {
  const graph = {
    schema_version: 'VisualEvidenceGraph@1' as const,
    graph_id: 'legacy_graph_fixture',
    request_id: 'mmreq_fixture',
    request_sha256: SHA256,
    project_id: 'project_fixture',
    domain_pack_id: 'pack_unclassified',
    provider: {
      provider_id: 'qwen',
      model_id: 'qwen_fixture',
      provider_response_sha256: SHA256,
      analyzed_at: '2026-07-31T00:00:00Z',
    },
    claims: [],
  }
  const request = {
    schema_version: 'MultimodalDesignRequest@1' as const,
    request_id: 'mmreq_fixture',
    project_id: 'project_fixture',
    turn_id: 'turn_fixture',
    domain_pack_id: 'pack_unclassified',
    instruction: '按图片理解一个银白色科幻装甲外观。',
    reference_inputs: [
      {
        evidence_id: 'refevid_front_fixture',
        evidence_sha256: SHA256,
        role: 'primary_silhouette' as const,
        view_id: 'front',
      },
      {
        evidence_id: 'refevid_detail_fixture',
        evidence_sha256: SHA256,
        role: 'surface' as const,
        view_id: 'detail',
      },
    ],
    active_asset_version_id: null,
    selection: null,
    locks: {
      preserve_geometry: false,
      preserve_material_surface: false,
      locked_part_ids: [],
      locked_material_zone_ids: [],
    },
  }
  const payload = buildAgentTurnRequestPayload({
    clientRequestId: 'agent-turn-fixture',
    message: request.instruction,
    multimodalContext: {
      request,
      graph,
    },
  })

  assert(payload.author_context?.references.length === 2, 'universal author transport must preserve every selected sealed evidence selector')
  assert(payload.author_context?.references[0]?.evidence_id === 'refevid_front_fixture', 'transport must preserve the exact sealed evidence identity')
  assert(payload.author_context?.references[1]?.view_hint === 'detail', 'transport must preserve the user-declared view hint')
  assert(payload.author_context?.visual_evidence_graph === graph, 'transport must pass the complete visual graph to Rust for normalization')
  assert(!('evidence_sha256' in (payload.author_context?.references[0] ?? {})), 'client transport must not self-report evidence hashes')
  assert(payload.multimodal_context === undefined, 'universal image Turns must not send a competing legacy multimodal source')
  assert(payload.author_context?.visual_evidence_graph === graph, 'visual comparison authorization is issued later against Rust-owned capture scope')
  assert(!('project_id' in (payload.author_context ?? {})), 'client transport must not self-report Project truth')
  assert(!('turn_id' in (payload.author_context ?? {})), 'client transport must not self-report Turn truth')

  const textOnly = buildAgentTurnRequestPayload({
    clientRequestId: 'agent-turn-text-fixture',
    message: '生成一个开放类别的外观概念。',
  })
  assert(textOnly.author_context?.references.length === 0, 'text-only Turn must still advertise the universal author entry')
  assert(textOnly.multimodal_context === undefined, 'text-only Turn must not create a fake visual context')

  const staleClarification = buildAgentTurnRequestPayload({
    clientRequestId: 'agent-turn-stale-clarification-fixture',
    message: '生成一只写实家猫。',
    clarificationDomainPackId: 'pack_robotic_arm_concept',
  })
  assert(staleClarification.author_context?.references.length === 0, 'legacy clarification state must not suppress the universal author entry')
  assert(staleClarification.clarification_domain_pack_id === undefined, 'the workbench must not emit a legacy domain selector on the universal route')
}

function assert(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message)
}
