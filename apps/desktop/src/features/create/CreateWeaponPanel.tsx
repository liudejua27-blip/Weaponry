import { useState } from 'react'
import type { ForgeApiClient } from '../../shared/api/forgeApi'
import type { CreateWeaponResponse, CreativeInterpretationCandidate, CreativeInterpretationResponse, CreativeRecastConfirmResponse } from '../../shared/types'

type Props = {
  api: ForgeApiClient
  onJobCreated: (response: CreateWeaponResponse) => void
  onEventsReset: () => void
}

export function CreateWeaponPanel({ api, onJobCreated, onEventsReset }: Props) {
  const [sourceObject, setSourceObject] = useState('防弹裤')
  const [text, setText] = useState('一条被称作防弹裤的国风神兵物件，可以像炮台、护体阵或位移机关一样被重诠释，外观高拟真但只作为虚构 Unity 游戏资产')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [isInterpreting, setIsInterpreting] = useState(false)
  const [isConfirming, setIsConfirming] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [weaponId, setWeaponId] = useState<string | null>(null)
  const [interpretation, setInterpretation] = useState<CreativeInterpretationResponse | null>(null)
  const [selectedCandidateId, setSelectedCandidateId] = useState<string | null>(null)
  const [confirmResult, setConfirmResult] = useState<CreativeRecastConfirmResponse | null>(null)

  async function submit() {
    setIsSubmitting(true)
    setError(null)
    setInterpretation(null)
    setSelectedCandidateId(null)
    setConfirmResult(null)
    onEventsReset()
    try {
      const clientRequestId = `desktop_${Date.now()}`
      const response = await api.createWeapon({
        client_request_id: clientRequestId,
        text,
        sketch_asset_id: null,
        reference_asset_ids: [],
        auto_run: true,
        target: {
          phase: 'concept_to_rough_3d',
          engine: 'unity',
          output_format: 'glb',
        },
      })
      setWeaponId(response.weapon_id)
      onJobCreated(response)
      await runInterpretation(response.weapon_id)
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : '创建任务失败')
    } finally {
      setIsSubmitting(false)
    }
  }

  async function runInterpretation(targetWeaponId = weaponId) {
    if (!targetWeaponId) {
      setError('请先创建或选择一个资产。')
      return
    }
    setIsInterpreting(true)
    setError(null)
    setConfirmResult(null)
    try {
      const response = await api.createInterpretation(targetWeaponId, {
        client_request_id: `interp_${Date.now()}`,
        source_object: sourceObject,
        raw_description: text,
        desired_style: '3渲2国风神兵，高拟真虚构 Unity 资产',
        freedom_level: 'strange',
        mythology_level: 'guofeng_divine',
        gameplay_complexity: 'multi_stage',
        asset_priority: 'lowpoly_first',
      })
      setInterpretation(response)
      setSelectedCandidateId(response.candidates?.[0]?.candidate_id ?? null)
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : '结构解释失败')
    } finally {
      setIsInterpreting(false)
    }
  }

  async function confirmCandidate() {
    const selected = interpretation?.candidates?.find((candidate) => candidate.candidate_id === selectedCandidateId)
    if (!weaponId || !interpretation || !selected) {
      setError('请选择一个结构候选。')
      return
    }
    setIsConfirming(true)
    setError(null)
    try {
      const response = await api.confirmCreativeRecast(weaponId, {
        client_request_id: `confirm_${Date.now()}`,
        interpretation_id: interpretation.interpretation_id,
        selected_candidate_id: selected.candidate_id,
        selected_candidate_rank: selected.rank,
        recast_mode: 'stylized_artifact',
        recast_choice_text: selected.recast_summary,
      })
      setConfirmResult(response)
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : '候选确认失败')
    } finally {
      setIsConfirming(false)
    }
  }

  const candidates = interpretation?.candidates ?? []
  const candidateReady = candidates.length >= 2 && candidates.length <= 3 && interpretation?.status !== 'failed'

  return (
    <section className="panel-section">
      <h1>Forge 工作台</h1>
      <label htmlFor="source-object">源对象</label>
      <input id="source-object" value={sourceObject} onChange={(event) => setSourceObject(event.target.value)} />
      <label htmlFor="weapon-prompt">创意描述</label>
      <textarea id="weapon-prompt" value={text} onChange={(event) => setText(event.target.value)} rows={8} />
      <button className="primary" aria-label="自动生成 mock 武器" onClick={submit} disabled={isSubmitting}>
        {isSubmitting ? '创建中...' : '创建资产并生成结构候选'}
      </button>
      {weaponId && (
        <button onClick={() => runInterpretation()} disabled={isInterpreting || isSubmitting}>
          {isInterpreting ? '解释中...' : '重新生成 2~3 个结构候选'}
        </button>
      )}
      {error && <p className="error">{error}</p>}
      {interpretation && (
        <div className="recast-panel">
          <div className="recast-header">
            <strong>Creative Recast 候选</strong>
            <span>{interpretation.candidate_count} 个候选 · seed {interpretation.stable_seed}</span>
          </div>
          {!candidateReady && (
            <p className="inline-error">候选数量或状态不满足 M6 规则，不能继续确认。</p>
          )}
          <div className="candidate-list">
            {candidates.map((candidate) => (
              <CandidateOption
                key={candidate.candidate_id}
                candidate={candidate}
                selected={candidate.candidate_id === selectedCandidateId}
                onSelect={() => setSelectedCandidateId(candidate.candidate_id)}
              />
            ))}
          </div>
          <button className="primary" onClick={confirmCandidate} disabled={!candidateReady || !selectedCandidateId || isConfirming}>
            {isConfirming ? '确认中...' : '确认所选结构'}
          </button>
        </div>
      )}
      {confirmResult && (
        <div className="recast-confirmed">
          <strong>已固定结构闭环</strong>
          <code>{confirmResult.creative_graph_id}</code>
          <code>{confirmResult.skill_graph_id}</code>
          <small>后续 concept / patch / 3D / Unity 应从这组 graph id 追溯。</small>
        </div>
      )}
    </section>
  )
}

function CandidateOption({
  candidate,
  selected,
  onSelect,
}: {
  candidate: CreativeInterpretationCandidate
  selected: boolean
  onSelect: () => void
}) {
  return (
    <button className={`candidate-option${selected ? ' selected' : ''}`} onClick={onSelect} type="button">
      <span className="candidate-rank">#{candidate.rank}</span>
      <strong>{candidate.name}</strong>
      <small>{candidate.summary}</small>
      <span className="candidate-affordances">{(candidate.combat_affordances ?? []).join(' / ')}</span>
      <small>anchor: {(candidate.anchor_points ?? []).join(', ')}</small>
      <small>protected: {(candidate.protected_regions ?? []).join(', ')}</small>
      <small>risk: {(candidate.risk_tags ?? []).join(', ')}</small>
    </button>
  )
}
