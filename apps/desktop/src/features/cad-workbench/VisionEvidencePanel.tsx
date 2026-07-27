import { useEffect, useMemo, useRef, useState } from 'react'
import {
  analyzeVisualEvidence,
  cancelVisualEvidenceAnalysis,
  clearVisionEvidenceProviderConfig,
  getVisionEvidenceProviderConfig,
  saveVisionEvidenceProviderConfig,
  type MultimodalDesignRequest,
  type VisionEvidenceProviderConfig,
  type VisionReferenceRole,
  type VisualEvidenceGraph,
} from '../../shared/tauri/visionEvidence.js'
import type { ReferenceEvidenceRecord, ReferenceEvidenceTarget } from './referenceEvidenceDrawerLogic.js'

const ROLE_OPTIONS: ReadonlyArray<{ value: VisionReferenceRole; label: string; detail: string }> = [
  { value: 'primary_silhouette', label: '主体轮廓', detail: '约束整体比例、姿态和辨识度。' },
  { value: 'structure', label: '结构细节', detail: '约束部件层级、接缝、凹槽和装甲分割。' },
  { value: 'material', label: '材质', detail: '约束金属、陶瓷、橡胶等 PBR 外观。' },
  { value: 'surface', label: '表面语言', detail: '约束纹理、磨损、图案和发光流线。' },
  { value: 'local_detail', label: '局部修改', detail: '只引导当前选中部件或材质区。' },
  { value: 'style', label: '风格', detail: '借鉴视觉语言，不复制具体结构。' },
  { value: 'multiview', label: '多视图', detail: '至少选择两份不同视角的已保存图片。' },
]
const CLAIM_LEVEL_ORDER = ['macro', 'meso', 'micro'] as const
type VisionEvidenceClaim = VisualEvidenceGraph['claims'][number]
type ClaimsByLevel = {
  macro: VisionEvidenceClaim[]
  meso: VisionEvidenceClaim[]
  micro: VisionEvidenceClaim[]
}

type BuildRequestInput = {
  requestId: string
  turnId: string
  target: ReferenceEvidenceTarget
  instruction: string
  evidences: ReferenceEvidenceRecord[]
  role: VisionReferenceRole
  activeAssetVersionId: string | null
  selectedPartId: string | null
  selectedMaterialZoneId: string | null
  preserveGeometry: boolean
  preserveMaterialSurface: boolean
}

export function buildMultimodalDesignRequest(input: BuildRequestInput): MultimodalDesignRequest {
  const hasActiveAsset = Boolean(input.activeAssetVersionId)
  const partIds = hasActiveAsset && input.selectedPartId ? [input.selectedPartId] : []
  const materialZoneIds = hasActiveAsset && input.selectedMaterialZoneId ? [input.selectedMaterialZoneId] : []
  return {
    schema_version: 'MultimodalDesignRequest@1',
    request_id: input.requestId,
    project_id: input.target.projectId,
    turn_id: input.turnId,
    domain_pack_id: input.target.domainPackId ?? 'pack_unknown_concept',
    instruction: input.instruction.trim(),
    reference_inputs: input.evidences.map((evidence) => ({
      evidence_id: evidence.evidenceId,
      evidence_sha256: evidence.contentSha256,
      role: input.role,
    })),
    active_asset_version_id: input.activeAssetVersionId,
    selection: hasActiveAsset && (partIds.length > 0 || materialZoneIds.length > 0)
      ? { part_ids: partIds, material_zone_ids: materialZoneIds }
      : null,
    locks: {
      preserve_geometry: hasActiveAsset && input.preserveGeometry,
      preserve_material_surface: hasActiveAsset && input.preserveMaterialSurface,
      locked_part_ids: [],
      locked_material_zone_ids: [],
    },
  }
}

export function validateVisionEvidenceSelection(
  role: VisionReferenceRole,
  evidences: ReferenceEvidenceRecord[],
  instruction: string,
): string | null {
  if (!instruction.trim()) return '请描述你希望参考图如何影响模型。'
  if (evidences.length === 0) return '请至少选择一份已保存的图片证据。'
  if (evidences.some((item) => item.kind !== 'image')) return '视觉模型只读取图片；GLB 继续使用严格 readback。'
  if (role === 'multiview' && evidences.length < 2) return '多视图分析至少需要两份已保存图片。'
  return null
}

export type VisionEvidencePanelProps = {
  target: ReferenceEvidenceTarget
  evidences: ReferenceEvidenceRecord[]
  preferredEvidenceId: string | null
  initialInstruction: string
  activeAssetVersionId: string | null
  selectedPartId: string | null
  selectedMaterialZoneId: string | null
  onMessage?: (message: string) => void
  onUseEvidence?: (input: {
    instruction: string
    request: MultimodalDesignRequest
    graph: VisualEvidenceGraph
  }) => Promise<void>
}

function claimLevelLabel(level: VisualEvidenceGraph['claims'][number]['level']): string {
  if (level === 'macro') return '宏观轮廓'
  if (level === 'meso') return '中频结构'
  return '微观表面'
}

function claimStatusLabel(status: VisualEvidenceGraph['claims'][number]['status']): string {
  if (status === 'observed') return '可见'
  if (status === 'inferred') return '推断'
  return '未知'
}

function visibleErrorMessage(caught: unknown, fallback: string): string {
  if (caught instanceof Error && caught.message.trim()) return caught.message
  if (typeof caught === 'string' && caught.trim()) return caught
  return fallback
}

export function visionConnectionLabel(configured: boolean, verified: boolean): string {
  if (verified) return '已验证'
  return configured ? '已配置·未验证' : '未配置'
}

export function VisionEvidencePanel({
  target,
  evidences,
  preferredEvidenceId,
  initialInstruction,
  activeAssetVersionId,
  selectedPartId,
  selectedMaterialZoneId,
  onMessage,
  onUseEvidence,
}: VisionEvidencePanelProps) {
  const imageEvidences = useMemo(
    () => {
      const seen = new Set<string>()
      const unique: ReferenceEvidenceRecord[] = []
      for (const item of evidences) {
        if (item.kind !== 'image' || seen.has(item.evidenceId)) continue
        seen.add(item.evidenceId)
        unique.push(item)
      }
      return unique
    },
    [evidences],
  )
  const [selectedIds, setSelectedIds] = useState<string[]>([])
  const [role, setRole] = useState<VisionReferenceRole>('primary_silhouette')
  const [instruction, setInstruction] = useState(initialInstruction)
  const [preserveGeometry, setPreserveGeometry] = useState(false)
  const [preserveMaterialSurface, setPreserveMaterialSurface] = useState(false)
  const [config, setConfig] = useState<VisionEvidenceProviderConfig | null>(null)
  const [baseUrl, setBaseUrl] = useState('')
  const [model, setModel] = useState('qwen3.7-plus')
  const [apiKey, setApiKey] = useState('')
  const [configBusy, setConfigBusy] = useState(false)
  const [connectionVerified, setConnectionVerified] = useState(false)
  const [analysisBusy, setAnalysisBusy] = useState(false)
  const [generationBusy, setGenerationBusy] = useState(false)
  const [detail, setDetail] = useState('')
  const [graph, setGraph] = useState<VisualEvidenceGraph | null>(null)
  const [analyzedRequest, setAnalyzedRequest] = useState<MultimodalDesignRequest | null>(null)
  const activeRequestRef = useRef<string | null>(null)
  const analysisEpochRef = useRef(0)
  const selectedIdsSet = useMemo(() => new Set(selectedIds), [selectedIds])
  const selectedEvidences = useMemo(
    () => imageEvidences.filter((item) => selectedIdsSet.has(item.evidenceId)),
    [imageEvidences, selectedIdsSet],
  )
  const claimsByLevel = useMemo(() => {
    const buckets: ClaimsByLevel = {
      macro: [],
      meso: [],
      micro: [],
    }
    if (!graph) return buckets
    for (let index = 0; index < graph.claims.length; index += 1) {
      const claim = graph.claims[index]
      if (claim.level === 'macro' || claim.level === 'meso' || claim.level === 'micro') {
        buckets[claim.level].push(claim)
      }
    }
    return buckets
  }, [graph])

  useEffect(() => {
    if (preferredEvidenceId && imageEvidences.some((item) => item.evidenceId === preferredEvidenceId)) {
      setSelectedIds((current) => current.length > 0 ? current : [preferredEvidenceId])
    }
  }, [imageEvidences, preferredEvidenceId])

  useEffect(() => {
    if (!instruction.trim() && initialInstruction.trim()) setInstruction(initialInstruction)
  }, [initialInstruction, instruction])

  useEffect(() => {
    let live = true
    void getVisionEvidenceProviderConfig()
      .then((next) => {
        if (!live || !next) return
        setConfig(next)
        setBaseUrl(next.baseUrl)
        setModel(next.model)
      })
      .catch(() => {
        if (live) setDetail('无法读取视觉理解服务配置；浏览器预览不会保存密钥。')
      })
    return () => {
      live = false
      analysisEpochRef.current += 1
      const active = activeRequestRef.current
      activeRequestRef.current = null
      if (active) void cancelVisualEvidenceAnalysis(active)
    }
  }, [])

  const validation = validateVisionEvidenceSelection(role, selectedEvidences, instruction)
  const selectedRole = ROLE_OPTIONS.find((item) => item.value === role) ?? ROLE_OPTIONS[0]

  const toggleEvidence = (evidenceId: string) => {
    if (analysisBusy || generationBusy) return
    setSelectedIds((current) => {
      const next = new Set(current)
      if (next.has(evidenceId)) next.delete(evidenceId)
      else next.add(evidenceId)
      return [...next]
    })
    setGraph(null)
    setAnalyzedRequest(null)
  }

  const saveConfig = async () => {
    if (!baseUrl.trim() || !model.trim() || !apiKey.trim() || configBusy) return
    setConfigBusy(true)
    setDetail('正在保存到本机权限受限的私密文件…')
    try {
      const next = await saveVisionEvidenceProviderConfig({
        baseUrl: baseUrl.trim(),
        model: model.trim(),
        apiKey,
      })
      setConfig(next)
      setConnectionVerified(false)
      setApiKey('')
      setDetail('视觉理解服务配置已安全保存；尚未联网验证，提取一次视觉证据后才会标记为已验证。')
    } catch (caught) {
      setDetail(visibleErrorMessage(caught, '视觉理解服务配置失败。'))
    } finally {
      setConfigBusy(false)
    }
  }

  const clearConfig = async () => {
    if (configBusy || analysisBusy) return
    setConfigBusy(true)
    try {
      const next = await clearVisionEvidenceProviderConfig()
      setConfig(next)
      setConnectionVerified(false)
      setApiKey('')
      setDetail('视觉理解服务密钥已从本机私密文件清除。')
    } catch (caught) {
      setDetail(visibleErrorMessage(caught, '清除视觉理解服务配置失败。'))
    } finally {
      setConfigBusy(false)
    }
  }

  const analyze = async () => {
    if (analysisBusy || validation) {
      if (validation) setDetail(validation)
      return
    }
    if (!config?.configured) {
      setDetail('请先配置视觉理解服务。密钥只保存在本机私密文件。')
      return
    }
    const suffix = `${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`
    const clientRequestId = `vision_evidence_${suffix}`
    const epoch = ++analysisEpochRef.current
    activeRequestRef.current = clientRequestId
    setAnalysisBusy(true)
    setGraph(null)
    setAnalyzedRequest(null)
    setDetail('正在读取已封存图片并提取宏观、中频和微观视觉证据…')
    const request = buildMultimodalDesignRequest({
      requestId: `mmreq_${suffix}`,
      turnId: `turn_${suffix}`,
      target,
      instruction,
      evidences: selectedEvidences,
      role,
      activeAssetVersionId,
      selectedPartId,
      selectedMaterialZoneId,
      preserveGeometry,
      preserveMaterialSurface,
    })
    try {
      const next = await analyzeVisualEvidence(clientRequestId, request)
      if (epoch !== analysisEpochRef.current || activeRequestRef.current !== clientRequestId) return
      setGraph(next.visualEvidenceGraph)
      setAnalyzedRequest(next.request)
      setConnectionVerified(true)
      setDetail(`视觉证据已通过 Rust 校验：${next.visualEvidenceGraph.claims.length} 项；尚未修改模型或创建版本。`)
      onMessage?.('参考图片已转换为可追溯视觉证据；下一步由 Agent 将证据编译进同一个设计程序。')
    } catch (caught) {
      if (epoch !== analysisEpochRef.current) return
      setConnectionVerified(false)
      setDetail(visibleErrorMessage(caught, '视觉证据分析失败；当前模型没有变化。'))
    } finally {
      if (epoch === analysisEpochRef.current) {
        activeRequestRef.current = null
        setAnalysisBusy(false)
      }
    }
  }

  const cancel = async () => {
    const active = activeRequestRef.current
    if (!active) return
    analysisEpochRef.current += 1
    activeRequestRef.current = null
    setAnalysisBusy(false)
    setDetail('已请求取消；任何迟到结果都会被丢弃。')
    await cancelVisualEvidenceAnalysis(active).catch(() => false)
  }

  const useEvidence = async () => {
    if (!onUseEvidence || !graph || !analyzedRequest || analysisBusy || generationBusy) return
    setGenerationBusy(true)
    setDetail('正在将已验证证据绑定到同一个 Rust Agent Turn…')
    try {
      await onUseEvidence({
        instruction: analyzedRequest.instruction,
        request: analyzedRequest,
        graph,
      })
      setDetail('证据已交给 Agent；只有生成、GLB 回读和质量门通过才会出现唯一预览。')
    } catch (caught) {
      setDetail(visibleErrorMessage(caught, '多模态 Agent Turn 启动失败；当前模型没有变化。'))
    } finally {
      setGenerationBusy(false)
    }
  }

  return (
    <section className="vision-evidence-panel" aria-label="视觉证据分析">
      <header>
        <div>
          <strong>视觉模型 · 只读证据分析</strong>
          <small>视觉模型只描述图片；Rust 校验来源、置信度和未知项，不允许 Provider 直接修改几何。</small>
        </div>
        <span className={connectionVerified ? 'configured' : ''}>
          {visionConnectionLabel(Boolean(config?.configured), connectionVerified)}
        </span>
      </header>

      <details className="vision-provider-config" open={!config?.configured}>
        <summary>视觉理解服务配置</summary>
        <label><span>OpenAI 兼容 Base URL</span><input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} disabled={configBusy || analysisBusy} placeholder="https://…/compatible-mode/v1" /></label>
        <label><span>视觉模型</span><input value={model} onChange={(event) => setModel(event.target.value)} disabled={configBusy || analysisBusy} placeholder="qwen3.7-plus" /></label>
        <label><span>API Key</span><input type="password" value={apiKey} onChange={(event) => setApiKey(event.target.value)} disabled={configBusy || analysisBusy} autoComplete="off" placeholder={config?.configured ? '输入新密钥以替换当前配置' : '仅保存到本机私密文件'} /></label>
        <small>不写入项目、日志或 Git；不使用 macOS 钥匙串，因此不会出现系统密码弹窗。</small>
        <div className="reference-evidence-actions">
          {config?.configured && <button type="button" onClick={() => void clearConfig()} disabled={configBusy || analysisBusy}>清除密钥</button>}
          <button type="button" className="reference-evidence-primary" onClick={() => void saveConfig()} disabled={configBusy || analysisBusy || !apiKey.trim() || !baseUrl.trim() || !model.trim()}>{configBusy ? '正在保存…' : config?.configured ? '替换配置' : '安全保存'}</button>
        </div>
      </details>

      <label className="reference-evidence-field">
        <span>这组参考如何影响设计</span>
        <textarea value={instruction} onChange={(event) => { setInstruction(event.target.value); setGraph(null); setAnalyzedRequest(null) }} disabled={analysisBusy || generationBusy} rows={3} placeholder="例如：保持机械臂结构，只借鉴蓝黑材质、装甲分割和发光流线。" />
      </label>

      <fieldset className="reference-evidence-views" disabled={analysisBusy || generationBusy}>
        <legend>参考角色</legend>
        {ROLE_OPTIONS.map((option) => <label key={option.value}><input type="radio" name="vision-reference-role" checked={role === option.value} onChange={() => { setRole(option.value); setGraph(null); setAnalyzedRequest(null) }} />{option.label}</label>)}
        <small>{selectedRole.detail}</small>
      </fieldset>

      <fieldset className="vision-evidence-selection" disabled={analysisBusy || generationBusy}>
        <legend>送入视觉模型的已保存图片</legend>
        {imageEvidences.length === 0
          ? <small>先保存至少一张授权图片。GLB 使用严格 readback，不直接发给视觉模型。</small>
          : imageEvidences.map((item) => (
            <label key={item.evidenceId}>
              <input type="checkbox" checked={selectedIdsSet.has(item.evidenceId)} onChange={() => toggleEvidence(item.evidenceId)} />
              <span>{item.fileName}</span>
              <code>{item.contentSha256.slice(0, 10)}…</code>
            </label>
          ))}
      </fieldset>

      {activeAssetVersionId && (
        <fieldset className="reference-evidence-views" disabled={analysisBusy}>
          <legend>继续设计边界</legend>
          <label><input type="checkbox" checked={preserveGeometry} onChange={(event) => setPreserveGeometry(event.target.checked)} />保持现有几何，只调整材质/表面</label>
          <label><input type="checkbox" checked={preserveMaterialSurface} onChange={(event) => setPreserveMaterialSurface(event.target.checked)} />保持材质/表面，只调整形体</label>
          <small>{selectedPartId ? `当前部件：${selectedPartId}` : '未选择局部部件，将作用于整个模型。'}{selectedMaterialZoneId ? ` · 材质区：${selectedMaterialZoneId}` : ''}</small>
        </fieldset>
      )}

      <div className="reference-evidence-actions">
        {analysisBusy && <button type="button" onClick={() => void cancel()}>取消分析</button>}
        <button type="button" className="reference-evidence-primary" disabled={analysisBusy || generationBusy || Boolean(validation) || !config?.configured} onClick={() => void analyze()}>{analysisBusy ? '正在分析…' : '提取视觉证据'}</button>
      </div>
      {validation && <small>{validation}</small>}
      {detail && <p className="reference-evidence-status" role="status" aria-live="polite">{detail}</p>}

      {graph && (
        <section className="vision-evidence-claims" aria-label="已验证视觉证据">
          <header><strong>Rust 已验证证据</strong><small>{graph.provider.model_id} · {graph.graph_id}</small></header>
          {CLAIM_LEVEL_ORDER.map((level) => {
            const claims = claimsByLevel[level]
            return (
              <div key={level}>
                <strong>{claimLevelLabel(level)}</strong>
                {claims.length > 0 ? <ul>{claims.map((claim) => <li key={claim.claim_id}><span>{claimStatusLabel(claim.status)}</span>{claim.description}</li>)}</ul> : <small>没有返回此层证据。</small>}
              </div>
            )
          })}
          <p className="reference-fidelity-ceiling"><strong>当前边界</strong><span>本步骤只形成证据图，不创建 ChangeSet、版本或 GLB。未观察到的部分保持未知。</span></p>
          {onUseEvidence && analyzedRequest && (
            <div className="reference-evidence-actions">
              <button type="button" className="reference-evidence-primary" disabled={generationBusy || analysisBusy} onClick={() => void useEvidence()}>
                {generationBusy ? 'Agent 正在生成…' : activeAssetVersionId ? '使用这些证据继续修改' : '使用这些证据生成 3D'}
              </button>
            </div>
          )}
        </section>
      )}
    </section>
  )
}
