import { useCallback, useEffect, useRef, useState } from 'react'
import {
  cancelVisualAssetGeneration,
  generateVisualAsset,
  getVisualProviderConfig,
  listRecoverableVisualAssetGenerations,
  listenVisualGenerationProgress,
  resumeVisualAssetGeneration,
  saveVisualProviderConfig,
  type GenerateVisualAssetOutput,
  type VisualProviderConfig,
  type VisualQualityTier,
  type VisualInputEvidence,
} from '../../shared/tauri/visualGeneration.js'

export type VisualGenerationState =
  | { status: 'idle'; detail: string }
  | { status: 'needs_configuration'; detail: string }
  | { status: 'recoverable'; detail: string; clientRequestId: string }
  | { status: 'generating'; detail: string; clientRequestId: string }
  | { status: 'ready'; detail: string; result: GenerateVisualAssetOutput }
  | { status: 'failed'; detail: string }

export function useVisualGeneration(options: {
  projectId: string | null
  onReady: (result: GenerateVisualAssetOutput) => void
}) {
  const [config, setConfig] = useState<VisualProviderConfig | null>(null)
  const [state, setState] = useState<VisualGenerationState>({
    status: 'idle',
    detail: '可以生成新的视觉资产。',
  })
  const activeRequestRef = useRef<string | null>(null)
  const projectIdRef = useRef(options.projectId)
  const onReadyRef = useRef(options.onReady)

  useEffect(() => {
    projectIdRef.current = options.projectId
  }, [options.projectId])
  useEffect(() => {
    onReadyRef.current = options.onReady
  }, [options.onReady])

  useEffect(() => {
    let live = true
    void getVisualProviderConfig()
      .then((metadata) => {
        if (live) setConfig(metadata)
      })
      .catch(() => {
        if (live) setConfig(null)
      })
    return () => {
      live = false
    }
  }, [])

  useEffect(() => {
    let dispose: (() => void) | undefined
    void listenVisualGenerationProgress((progress) => {
      if (activeRequestRef.current !== progress.clientRequestId) return
      setState({
        status: progress.stage === 'ready' ? 'generating' : 'generating',
        detail: progress.detail,
        clientRequestId: progress.clientRequestId,
      })
    }).then((unlisten) => {
      dispose = unlisten
    })
    return () => dispose?.()
  }, [])

  useEffect(() => {
    let live = true
    if (activeRequestRef.current) {
      void cancelVisualAssetGeneration(activeRequestRef.current)
      activeRequestRef.current = null
    }
    setState({ status: 'idle', detail: '可以生成新的视觉资产。' })
    if (options.projectId) {
      void listRecoverableVisualAssetGenerations(options.projectId)
        .then((records) => {
          if (!live || projectIdRef.current !== options.projectId || records.length === 0) return
          setState({
            status: 'recoverable',
            detail: records[0].state.stage === 'concept_submitted'
              ? '上次概念图任务已提交，可继续恢复。'
              : '上次神经 3D 任务已提交，可继续恢复。',
            clientRequestId: records[0].client_request_id,
          })
        })
        .catch(() => undefined)
    }
    return () => {
      live = false
    }
  }, [options.projectId])

  const configure = useCallback(async (falApiKey: string) => {
    const saved = await saveVisualProviderConfig(falApiKey)
    setConfig(saved)
    if (saved.configured) {
      setState({ status: 'idle', detail: '视觉生成服务已配置；尚未发起远程请求。' })
    }
    return saved
  }, [])

  const start = useCallback(async (
    userIntent: string,
    qualityTier: VisualQualityTier = 'standard_asset',
    inputEvidence: VisualInputEvidence[] = [],
  ) => {
    const projectId = projectIdRef.current
    if (!projectId) {
      setState({ status: 'failed', detail: '请先创建或打开一个项目。' })
      return null
    }
    if (!config?.configured) {
      setState({
        status: 'needs_configuration',
        detail: '请先配置远程视觉生成服务；尚未发送描述或产生费用。',
      })
      return null
    }
    if (activeRequestRef.current) return null
    const clientRequestId = `visual_generation_${Date.now().toString(36)}`
    const turnId = `visual_turn_${Date.now().toString(36)}`
    activeRequestRef.current = clientRequestId
    setState({ status: 'generating', detail: '正在理解视觉意图', clientRequestId })
    try {
      const result = await generateVisualAsset({
        client_request_id: clientRequestId,
        project_id: projectId,
        turn_id: turnId,
        user_intent: userIntent,
        quality_tier: qualityTier,
        input_evidence: inputEvidence,
      })
      if (activeRequestRef.current !== clientRequestId || projectIdRef.current !== projectId) {
        return null
      }
      activeRequestRef.current = null
      setState({ status: 'ready', detail: '唯一神经 3D 候选已通过结构 readback。', result })
      onReadyRef.current(result)
      return result
    } catch (caught) {
      if (activeRequestRef.current !== clientRequestId) return null
      activeRequestRef.current = null
      setState({
        status: 'failed',
        detail: caught instanceof Error ? caught.message : String(caught),
      })
      return null
    }
  }, [config?.configured])

  const resume = useCallback(async () => {
    if (state.status !== 'recoverable' || activeRequestRef.current) return null
    const requestId = state.clientRequestId
    const projectId = projectIdRef.current
    activeRequestRef.current = requestId
    setState({ status: 'generating', detail: '正在恢复上次远程任务', clientRequestId: requestId })
    try {
      const result = await resumeVisualAssetGeneration(requestId)
      if (activeRequestRef.current !== requestId || projectIdRef.current !== projectId) return null
      activeRequestRef.current = null
      setState({ status: 'ready', detail: '唯一神经 3D 候选已通过结构 readback。', result })
      onReadyRef.current(result)
      return result
    } catch (caught) {
      if (activeRequestRef.current !== requestId) return null
      activeRequestRef.current = null
      setState({
        status: 'failed',
        detail: caught instanceof Error ? caught.message : String(caught),
      })
      return null
    }
  }, [state])

  const cancel = useCallback(async () => {
    const requestId = activeRequestRef.current
      ?? (state.status === 'recoverable' ? state.clientRequestId : null)
    if (!requestId) return
    const wasRecoverable = state.status === 'recoverable'
    const cancelled = await cancelVisualAssetGeneration(requestId)
    if (cancelled && (activeRequestRef.current === requestId || wasRecoverable)) {
      activeRequestRef.current = null
      setState({ status: 'idle', detail: '本次视觉生成已取消；没有创建资产版本。' })
    }
  }, [state])

  return { config, state, configure, start, resume, cancel }
}
