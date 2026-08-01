import type { StartAgentTurnRequest } from '../../shared/types.js'
import type { MultimodalDesignRequest, VisualEvidenceGraph } from '../../shared/tauri/visionEvidence.js'

export type UniversalAuthorTransportContext = {
  request: MultimodalDesignRequest
  graph: VisualEvidenceGraph
}

export type AgentTurnRequestPayloadInput = {
  clientRequestId: string
  message: string
  clarificationDomainPackId?: string
  multimodalContext?: UniversalAuthorTransportContext
  gameAssetDelivery?: StartAgentTurnRequest['game_asset_delivery']
}

/**
 * Build only the client-owned wire projection for a Turn.
 *
 * The author_context deliberately contains selectors and the evidence graph,
 * not evidence hashes, Project/Turn IDs, active Snapshot facts or capability
 * claims. Rust reads the selected sealed evidence and reconstructs those
 * values before creating UniversalAuthorRequest@1. A universal image Turn
 * must not also carry the legacy multimodal envelope: sending both creates
 * two competing comparison sources. The legacy envelope remains available
 * to older callers that submit it directly, but this product builder never
 * emits it for the universal workbench path.
 */
export function buildAgentTurnRequestPayload(input: AgentTurnRequestPayloadInput): StartAgentTurnRequest {
  const { multimodalContext } = input
  // The workbench is category-open.  A legacy clarification id may still be
  // supplied by an old presentation or replayed fixture, but it must never
  // suppress the universal author context.  Doing so lets an active asset
  // fall back to plan_complete_concept and makes an ordinary free-form
  // request look like a domain-selection answer.  Legacy callers that need
  // the old wire contract can still construct StartAgentTurnRequest directly;
  // the product workbench always enters the Rust-sealed universal route.
  return {
    client_request_id: input.clientRequestId,
    message: input.message,
    author_context: multimodalContext
      ? {
          references: multimodalContext.request.reference_inputs.map((reference) => ({
            evidence_id: reference.evidence_id,
            role: reference.role,
            ...(reference.view_id ? { view_hint: reference.view_id } : {}),
          })),
          visual_evidence_graph: multimodalContext.graph,
        }
      : { references: [] },
    ...(input.gameAssetDelivery ? { game_asset_delivery: input.gameAssetDelivery } : {}),
  }
}
