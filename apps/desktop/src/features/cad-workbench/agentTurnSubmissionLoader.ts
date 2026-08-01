import type { ForgeApi } from '../../shared/api/forgeApi'
import type { AgentAssetChangeSet, AgentItem, AssemblyDeltaProgram } from '../../shared/types'
import { createAgentTurnEventCollector } from './agentTurnEventStream'
import { resolveAgentTurnRecordFailure } from './agentTurnRecordFailure'
import {
  parseCandidatePbrCapturePending,
  parseUniversalAuthorPresentation,
  type AgentClarificationOption,
  type CandidatePbrCapturePendingPresentation,
} from './agentConversationState'
import { readSingleResultDecisionFromAgentItems } from './singleResultDecisionPresentationState'
import type { AgentTurnPresentation } from './agentConversationState'
import type { SingleResultDecision, SingleResultDecisionPresentationAction } from './singleResultDecisionPresentationState'
import {
  buildAgentTurnRequestPayload,
  type UniversalAuthorTransportContext,
} from './agentTurnRequestPayload.js'

type AgentTurnSubmissionApi = Pick<
  ForgeApi,
  'createAgentThread' |
  'subscribeAgentThreadEvents' |
  'startAgentTurn' |
  'rejectSingleResultPreview' |
  'loadSingleResultPreviewGlb'
>

type AgentTurn = Awaited<ReturnType<AgentTurnSubmissionApi['startAgentTurn']>>
type SingleResultPreviewGlb = Awaited<ReturnType<AgentTurnSubmissionApi['loadSingleResultPreviewGlb']>>

export type AgentTurnRecordResult = {
  recorded: boolean
  clarification: boolean
  cancelled: boolean
  failed: boolean
  plan: AgentTurnPresentation['plan']
  decision: SingleResultDecision | null
  candidatePbrCapturePending: CandidatePbrCapturePendingPresentation | null
}

export type MultimodalAgentTurnContext = UniversalAuthorTransportContext

export type GameAssetDeliveryProfile = 'off' | 'game_prop_light' | 'game_prop_standard'
export type GameAssetDeliveryRequestInput = {
  schema_version: 'GameAssetDeliveryRequest@1'
  profile_id: Exclude<GameAssetDeliveryProfile, 'off'>
  lod_triangle_budgets: [number, number, number]
  target_texel_density_pixels_per_meter: number
}

export function gameAssetDeliveryRequestForProfile(
  profile: GameAssetDeliveryProfile,
): GameAssetDeliveryRequestInput | undefined {
  if (profile === 'off') return undefined
  return profile === 'game_prop_light'
    ? {
        schema_version: 'GameAssetDeliveryRequest@1',
        profile_id: profile,
        lod_triangle_budgets: [90_000, 36_000, 8_000],
        target_texel_density_pixels_per_meter: 512,
      }
    : {
        schema_version: 'GameAssetDeliveryRequest@1',
        profile_id: profile,
        lod_triangle_budgets: [150_000, 60_000, 12_000],
        target_texel_density_pixels_per_meter: 1024,
      }
}

type AgentTurnSubmissionCallbacks = {
  startAgentConversationRequest: (projectId: string | null) => { projectId: string | null; requestId: number }
  isCurrentAgentConversationRequest: (projectId: string | null, requestId: number) => boolean
  claimAgentTurnSubmission: () => boolean
  releaseAgentTurnSubmission: () => void
  parseAgentTurnPresentation: (items: readonly AgentItem[], requestText: string) => AgentTurnPresentation
  receiveAgentTurn: (
    projectId: string | null,
    requestId: number,
    threadId: string,
    items: readonly AgentItem[],
    presentation: AgentTurnPresentation,
  ) => boolean
  receiveAgentClarification: (
    projectId: string | null,
    requestId: number,
    clarification: { status: 'ambiguous' | 'unsupported'; kind: 'domain' | 'scope'; question: string; options: AgentClarificationOption[]; originalMessage?: string },
  ) => boolean
  markAgentKernelUnavailable: (projectId: string | null, requestId: number) => boolean
  dispatchSingleResultDecision: (action: SingleResultDecisionPresentationAction) => void
  setActiveProviderTurnId: (value: string | null) => void
  clearBlockoutDisplay: (projectId: string | null) => void
  clearAgentAssetWorkspace: () => void
  setAgentAssetChangeSet: (changeSet: AgentAssetChangeSet | null) => void
  setAgentCandidateSelectedPartId: (partId: string | null) => void
  hydrateBlockoutDisplay: (projectId: string | null, data: {
    glbBase64: SingleResultPreviewGlb['glb']
    glbKind: 'compiled_agent_production_pbr' | 'compiled_agent_preview_pbr'
    shapeProgram: null
    segmentation: null
  }) => number | null
  setAssistantNote: (message: string) => void
  errorText: (caught: unknown) => string
}

export type AgentTurnSubmissionInput = {
  projectId: string | null
  projectName: string | null
  agentThreadId: string | null
  agentKernelItems: readonly AgentItem[]
  message: string
  clarificationDomainPackId?: string
  multimodalContext?: MultimodalAgentTurnContext
  gameAssetDelivery?: GameAssetDeliveryRequestInput
  intent?: AgentTurnIntent
  clarificationOptions: readonly AgentClarificationOption[]
}

export type AgentTurnIntent = 'brief' | 'change'

export async function recordAgentTurn(
  api: AgentTurnSubmissionApi,
  callbacks: AgentTurnSubmissionCallbacks,
  input: AgentTurnSubmissionInput,
): Promise<AgentTurnRecordResult> {
  const {
    startAgentConversationRequest,
    isCurrentAgentConversationRequest,
    claimAgentTurnSubmission,
    releaseAgentTurnSubmission,
    parseAgentTurnPresentation,
    receiveAgentTurn,
    receiveAgentClarification,
    markAgentKernelUnavailable,
    dispatchSingleResultDecision,
    setActiveProviderTurnId,
    clearBlockoutDisplay,
    clearAgentAssetWorkspace,
    setAgentAssetChangeSet,
    setAgentCandidateSelectedPartId,
    hydrateBlockoutDisplay,
    setAssistantNote,
    errorText,
  } = callbacks

  if (!claimAgentTurnSubmission()) {
    return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null, candidatePbrCapturePending: null }
  }

  const {
    projectId,
    projectName,
    agentThreadId: currentThreadId,
    agentKernelItems,
    message,
    clarificationDomainPackId,
    multimodalContext,
    gameAssetDelivery,
    intent = 'brief',
    clarificationOptions,
  } = input

  try {
    const { requestId } = startAgentConversationRequest(projectId)
    dispatchSingleResultDecision({ type: 'request_started', projectId, requestId, detail: 'Agent 正在构建并检查 3D 结果。' })

    try {
      let threadId = currentThreadId
      if (!threadId) {
        const created = await api.createAgentThread({
          client_request_id: `agent-thread-${Date.now()}`,
          project_id: projectId,
          title: projectName ? `${projectName} · Agent` : '新建设计会话',
        })
        threadId = created.thread_id
        if (!isCurrentAgentConversationRequest(projectId, requestId)) {
          return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null, candidatePbrCapturePending: null }
        }
      }

      const eventCollector = createAgentTurnEventCollector({
        existingKernelItems: agentKernelItems,
        projectId,
        requestId,
        threadId,
        isCurrentRequest: isCurrentAgentConversationRequest,
        setActiveProviderTurnId,
        parseAgentTurnPresentation,
        receiveAgentTurn,
        message,
      })
      const unsubscribeThreadEvents = api.subscribeAgentThreadEvents(
        threadId,
        { onEvent: eventCollector.onEvent },
        eventCollector.afterSequence,
      )
      const turnPromise = api.startAgentTurn(threadId, buildAgentTurnRequestPayload({
        clientRequestId: `agent-turn-${Date.now()}`,
        message,
        clarificationDomainPackId,
        multimodalContext,
        gameAssetDelivery,
      }))
      let turn: AgentTurn
      try {
        turn = await turnPromise
      } finally {
        unsubscribeThreadEvents()
        setActiveProviderTurnId(null)
      }

      const presentation = parseAgentTurnPresentation(turn.items, turn.request_text)
      if (!receiveAgentTurn(projectId, requestId, threadId, turn.items, presentation)) {
        return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null, candidatePbrCapturePending: null }
      }
      if (turn.status === 'cancelled') {
        dispatchSingleResultDecision({ type: 'request_cancelled', projectId, requestId })
        setAssistantNote('本次模型请求已取消；没有创建计划、资产版本或导出。')
        return { recorded: true, clarification: false, cancelled: true, failed: false, plan: null, decision: null, candidatePbrCapturePending: null }
      }
      if (presentation.clarification) {
        dispatchSingleResultDecision({ type: 'request_cancelled', projectId, requestId })
        clearBlockoutDisplay(projectId)
        clearAgentAssetWorkspace()
        setAgentAssetChangeSet(null)
        setAgentCandidateSelectedPartId(null)
        setAssistantNote(presentation.clarification.question)
        return { recorded: true, clarification: true, cancelled: false, failed: false, plan: null, decision: null, candidatePbrCapturePending: null }
      }

      const universalAuthor = parseUniversalAuthorPresentation(turn.items)
      if (universalAuthor?.outcome === 'limitation' || universalAuthor?.outcome === 'clarification_required') {
        // A typed limitation is a successful understanding Turn with zero
        // candidate/version side effects. Preserve the confirmed active model
        // and clear only this Turn's single-result waiting state.
        dispatchSingleResultDecision({ type: 'request_cancelled', projectId, requestId })
        setAssistantNote(universalAuthor.message ?? (
          universalAuthor.outcome === 'limitation'
            ? '对象已理解，但当前表示能力不足；没有生成错误模板或替换现有模型。'
            : '对象身份或目标存在冲突，请补充说明后重试。'
        ))
        return { recorded: true, clarification: universalAuthor.outcome === 'clarification_required', cancelled: false, failed: false, plan: null, decision: null, candidatePbrCapturePending: null }
      }

      const candidatePbrCapturePending = parseCandidatePbrCapturePending(turn.items)
      if (candidatePbrCapturePending) {
        setAssistantNote('候选模型已完成严格编译与回读，正在使用当前工作台的同一 PBR 视口进行八视图验收。')
        return { recorded: true, clarification: false, cancelled: false, failed: false, plan: null, decision: null, candidatePbrCapturePending }
      }

      const decision = readSingleResultDecisionFromAgentItems(turn.items, { projectId, turnId: turn.turn_id })
      // A persisted SingleResultDecision is the sealed, Rust-owned terminal
      // contract for this Turn. Compatibility presentation may also derive an
      // AssemblyDelta-shaped plan from the same transcript; that advisory
      // field must never suppress a verified new candidate or leave the
      // previous GLB visible after a successful material/style regeneration.
      if (!decision) {
        const missingDecisionError = 'Agent 没有返回正式的单一结果决策；这次生成没有形成可用结果，当前设计没有变化。请换一种描述后再试。'
        dispatchSingleResultDecision({ type: 'request_failed', projectId, requestId, error: missingDecisionError })
        setAssistantNote(missingDecisionError)
        return { recorded: true, clarification: false, cancelled: false, failed: true, plan: null, decision: null, candidatePbrCapturePending: null }
      }

      if (decision) {
        dispatchSingleResultDecision({ type: 'decision_received', projectId, requestId, decision })
        if (decision.state === 'ready_for_preview') {
          try {
            const preview = await api.loadSingleResultPreviewGlb({
              projectId: decision.project_id,
              turnId: decision.turn_id,
              previewId: decision.preview.preview_id,
              artifactSha256: decision.preview.artifact_sha256,
              artifactProfileId: decision.preview.artifact_profile_id,
            })
            if (!isCurrentAgentConversationRequest(projectId, requestId)) {
              return { recorded: false, clarification: false, cancelled: true, failed: false, plan: null, decision: null, candidatePbrCapturePending: null }
            }
            clearAgentAssetWorkspace()
            setAgentAssetChangeSet(null)
            setAgentCandidateSelectedPartId(null)
            hydrateBlockoutDisplay(projectId, {
              glbBase64: preview.glb,
              glbKind: preview.artifactProfileId === 'production_concept'
                ? 'compiled_agent_production_pbr'
                : 'compiled_agent_preview_pbr',
              shapeProgram: null,
              segmentation: null,
            })
          } catch (caught) {
            const error = `正式结果已通过质量门，但 3D 预览读取失败：${errorText(caught)}`
            dispatchSingleResultDecision({ type: 'request_failed', projectId, requestId, error })
            setAssistantNote(error)
            return { recorded: true, clarification: false, cancelled: false, failed: true, plan: null, decision: null, candidatePbrCapturePending: null }
          }
        }
        return { recorded: true, clarification: false, cancelled: false, failed: false, plan: presentation.plan, decision, candidatePbrCapturePending: null }
      }

      const missingDecisionError = 'Agent 没有返回正式的单一结果决策；这次生成没有形成可用结果，当前设计没有变化。请换一种描述后再试。'
      dispatchSingleResultDecision({ type: 'request_failed', projectId, requestId, error: missingDecisionError })
      setAssistantNote(missingDecisionError)
      return { recorded: true, clarification: false, cancelled: false, failed: true, plan: null, decision: null, candidatePbrCapturePending: null }
    } catch (caught) {
      return resolveAgentTurnRecordFailure({
        caught,
        projectId,
        requestId,
        message,
        clarificationOptions,
        isCurrentRequest: isCurrentAgentConversationRequest,
        receiveAgentClarification,
        setAssistantNote,
        dispatchSingleResultDecision,
        markKernelUnavailable: markAgentKernelUnavailable,
      })
    }
  } finally {
    releaseAgentTurnSubmission()
  }
}

type BriefInstructionResultCallbacks = {
  setAssistantNote: (message: string) => void
  setChatInput: (value: string) => void
  legacyDesignReadOnly: boolean
}

export function applyBriefInstructionResult(
  result: AgentTurnRecordResult,
  callbacks: BriefInstructionResultCallbacks,
): void {
  const { legacyDesignReadOnly, setAssistantNote, setChatInput } = callbacks
  if (result.cancelled) return
  if (result.failed) {
    setChatInput('')
    return
  }
  if (result.clarification) {
    setChatInput('')
    return
  }
  if (result.candidatePbrCapturePending) {
    setAssistantNote('候选模型正在接受同一工作台渲染器的 PBR 八视图验收；验收通过前不会创建预览、版本或导出。')
    setChatInput('')
    return
  }
  if (result.decision) {
    setAssistantNote(result.decision.state === 'ready_for_preview'
      ? '本次唯一结果已通过正式生成质量门；确认前不会创建可编辑版本。'
      : '本次正式生成未产生可展示结果；当前设计没有变化。')
    setChatInput('')
    return
  }
  if (legacyDesignReadOnly) {
    setAssistantNote(result.recorded
      ? '请先点击“让 Agent 重建可编辑资产”，并确认本地 Agent 已启动。旧版设计不会被修改。'
      : '这次生成暂时无法形成可编辑结果，当前设计没有变化。请补充外观或部件描述后再试。'
    )
    setChatInput('')
    return
  }
  setAssistantNote(result.recorded
    ? 'Agent 计划没有返回可构建结果；当前设计没有变化。'
    : '当前 Agent 计划未记录成功；不会调用旧版 Planner 作为替代。')
  setChatInput('')
}

export type ExecuteBriefInstructionRequest = {
  requestText: string
  clarificationDomainPackId?: string
  multimodalContext?: MultimodalAgentTurnContext
  gameAssetDelivery?: GameAssetDeliveryRequestInput
  defaultBrief: string
  legacyDesignReadOnly: boolean
  setAssistantNote: (message: string) => void
  setChatInput: (value: string) => void
  recordAgentTurn: (
    message: string,
    clarificationDomainPackId?: string,
    multimodalContext?: MultimodalAgentTurnContext,
    gameAssetDelivery?: GameAssetDeliveryRequestInput,
    intent?: AgentTurnIntent,
  ) => Promise<AgentTurnRecordResult>
}

export async function submitBriefInstructionWithText(
  input: ExecuteBriefInstructionRequest,
): Promise<void> {
  const instruction = input.requestText.trim() || input.defaultBrief
  input.setAssistantNote(`正在解释 Brief：“${instruction}”`)
  const kernelResult = await input.recordAgentTurn(
    instruction,
    input.clarificationDomainPackId,
    input.multimodalContext,
    input.gameAssetDelivery,
    'brief',
  )
  applyBriefInstructionResult(kernelResult, {
    setAssistantNote: input.setAssistantNote,
    setChatInput: input.setChatInput,
    legacyDesignReadOnly: input.legacyDesignReadOnly,
  })
}

type ChangeInstructionResultCallbacks = {
  setAssistantNote: (message: string) => void
  setChatInput: (value: string) => void
  previewAgentAssemblyDelta: (delta: AssemblyDeltaProgram) => Promise<void>
}

export async function applyChangeInstructionResult(
  result: AgentTurnRecordResult,
  callbacks: ChangeInstructionResultCallbacks,
): Promise<void> {
  const { previewAgentAssemblyDelta, setAssistantNote, setChatInput } = callbacks
  if (result.failed || result.clarification) {
    setChatInput('')
    return
  }
  if (result.cancelled) return
  if (result.plan?.assembly_delta) {
    await previewAgentAssemblyDelta(result.plan.assembly_delta)
  } else {
    setAssistantNote(result.recorded
      ? '这次修改没有找到明确的改动，当前设计没有变化。请说明要增加、替换或调整的部件。'
      : '修改意图未记录成功；当前资产没有变化。')
  }
  setChatInput('')
}

export type ExecuteChangeInstructionRequest = {
  requestText: string
  legacyDesignReadOnly: boolean
  setAssistantNote: (message: string) => void
  setChatInput: (value: string) => void
  recordAgentTurn: (
    message: string,
    clarificationDomainPackId?: string,
    multimodalContext?: MultimodalAgentTurnContext,
    gameAssetDelivery?: GameAssetDeliveryRequestInput,
    intent?: AgentTurnIntent,
  ) => Promise<AgentTurnRecordResult>
  previewAgentAssemblyDelta: (delta: AssemblyDeltaProgram) => Promise<void>
}

export async function submitChangeInstruction(
  input: ExecuteChangeInstructionRequest,
): Promise<void> {
  if (input.legacyDesignReadOnly) {
    input.setAssistantNote('旧版设计为只读状态。请先让 Agent 重建可编辑资产。')
    return
  }
  const instruction = input.requestText.trim()
  if (!instruction) return
  input.setAssistantNote(`正在规划修改：“${instruction}”`)
  // `recordAgentTurn` keeps the legacy clarification slot before the intent
  // slot for wire compatibility.  Passing "change" as the second positional
  // argument silently serialized it as clarification_domain_pack_id, which
  // made Rust reject every change request as a multimodal/clarification
  // conflict before a Turn could be persisted.
  const kernelResult = await input.recordAgentTurn(
    instruction,
    undefined,
    undefined,
    undefined,
    'change',
  )
  await applyChangeInstructionResult(kernelResult, {
    setAssistantNote: input.setAssistantNote,
    setChatInput: input.setChatInput,
    previewAgentAssemblyDelta: input.previewAgentAssemblyDelta,
  })
}
