import { useEffect } from 'react'
import type { JobEvent, JobRuntimeStateResponse } from '../../shared/types'

type Props = {
  events: JobEvent[]
  jobId?: string | null
  streamStatus?: 'idle' | 'connecting' | 'live' | 'reconnecting' | 'closed'
  runtime?: JobRuntimeStateResponse | null
  onSubscribe?: (jobId: string) => () => void
  actionStatus?: string | null
  onRetryJob?: (jobId: string) => void
  onRetryFromStep?: (jobId: string, stepName: string) => void
  onCancelJob?: (jobId: string) => void
  onOpenSettings?: () => void
  onSkip3D?: () => void
  highlightedEventId?: string | null
}

type StepGroup = {
  step: string
  latest: JobEvent
  events: JobEvent[]
  progress: number
  level: string
  status: string
}

const STATUS_LABELS: Record<string, string> = {
  idle: '等待任务',
  connecting: '连接事件流',
  live: '实时追踪',
  reconnecting: '重连中',
  closed: '已关闭',
  completed: '已完成并入库',
  queued: '已进入队列',
  running: '本地 Worker 执行中',
  started: '执行中',
  progress: '执行中',
  submitted: '已提交 Provider',
  polling: 'Provider 生成中',
  cancel_requested: '取消请求已发送',
  waiting_provider: '等待 Provider 返回',
  waiting_user: '等待处理',
  retrying: '等待重试',
  succeeded: '已完成',
  failed: '生成失败',
  cancelled: '已取消',
  partial_succeeded: '部分完成',
}

const STEP_LABELS: Record<string, string> = {
  request_guard: '请求校验',
  input_interpreter: '输入理解',
  weapon_spec_planner: '方案规划',
  prompt_builder: 'Prompt 构建',
  image_submit: '概念图生成',
  image_inpaint: '局部修改生成',
  image_quality_check: '图像质量检查',
  asset_librarian: '资产入库',
  rough3d_plan: '3D 输入规划',
  rough3d_submit: '3D 粗模生成',
  rough3d_poll: '等待 3D Provider',
  rough3d_fetch: '取回 3D 输出',
  model_quality_check: '模型质量检查',
  model_qc_optimize: '模型质检优化',
  asset_commit_model: '3D 资产入库',
  export_plan: '导出输入检查',
  export_manifest: 'Unity 清单生成',
  export_package: 'Unity 包入库',
  unity_export: 'Unity 导出',
  finalize_job: '收尾',
  patch_interpreter: 'Patch 理解',
}

export function JobTimeline({
  events,
  jobId,
  streamStatus = 'idle',
  runtime,
  onSubscribe,
  actionStatus,
  onRetryJob,
  onRetryFromStep,
  onCancelJob,
  onOpenSettings,
  onSkip3D,
  highlightedEventId,
}: Props) {
  useEffect(() => {
    if (!jobId || !onSubscribe) return
    return onSubscribe(jobId)
  }, [jobId, onSubscribe])

  useEffect(() => {
    if (!highlightedEventId) return
    document.getElementById(eventDomId(highlightedEventId))?.scrollIntoView({ block: 'center', behavior: 'smooth' })
  }, [highlightedEventId])

  const steps = groupEventsByStep(events)
  const latestEvent = events.at(-1) ?? null
  const failedStep = [...steps].reverse().find((step) => step.status === 'failed' || step.level === 'error') ?? null
  const hasRough3DFailure = Boolean(failedStep?.step.toLowerCase().includes('rough3d') || failedStep?.step.toLowerCase().includes('3d'))
  const eventDerivedCanCancel = latestEvent ? ['created', 'queued', 'running', 'waiting_provider', 'waiting_user', 'retrying', 'started', 'progress'].includes(latestEvent.status) : false
  const canCancel = runtime?.cancellable ?? eventDerivedCanCancel
  const canResume = runtime?.resumable ?? Boolean(failedStep)
  const displayStatus = latestEvent?.step === 'finalize_job' && latestEvent.status === 'succeeded'
    ? 'completed'
    : latestEvent && latestEvent.status !== 'succeeded'
      ? latestEvent.status
      : streamStatus
  const progress = steps.length ? Math.max(...steps.map((step) => step.progress)) : 0

  return (
    <section className="job-drawer">
      <div className="job-trace-header">
        <div>
          <span>Agent 执行轨迹</span>
          <small>{jobId ? jobId : '等待任务'}</small>
        </div>
        <div className={`job-stream-state ${displayStatus}`}>{STATUS_LABELS[displayStatus] ?? displayStatus}</div>
      </div>

      <div className="job-trace-summary">
        <div className="job-progress">
          <span style={{ width: `${Math.round(progress * 100)}%` }} />
        </div>
        <span className="muted">
        {events.length ? `${events.length} 条事件 · ${steps.length} 个步骤` : '创建任务后，本地 Worker 会自动领取队列任务；这里显示可追踪、可恢复的 Agent 步骤。'}
        </span>
        {latestEvent && <span className="muted">最近 #{latestEvent.seq}：{latestEvent.message}</span>}
      </div>

      <div className="job-actions" aria-label="任务恢复动作">
        <button disabled={!jobId || !canResume || !onRetryJob} onClick={() => jobId && onRetryJob?.(jobId)}>请求重试任务</button>
        <button
          disabled={!jobId || !failedStep || !onRetryFromStep}
          onClick={() => jobId && failedStep && onRetryFromStep?.(jobId, failedStep.step)}
        >
          请求从失败步骤重试
        </button>
        <button disabled={!jobId || !canCancel || !onCancelJob} onClick={() => jobId && onCancelJob?.(jobId)}>请求取消</button>
        <button disabled={!onOpenSettings} onClick={onOpenSettings}>打开设置</button>
        <button disabled={!onSkip3D || !hasRough3DFailure} onClick={onSkip3D}>跳过 3D</button>
        {actionStatus && <span className="muted">{actionStatus}</span>}
      </div>

      <RuntimeTraceMiniPanel runtime={runtime} />

      <ol className="timeline agent-trace">
        {steps.map((step) => {
          const isHighlighted = Boolean(highlightedEventId && step.events.some((event) => event.id === highlightedEventId))
          return (
          <li
            key={step.step}
            id={isHighlighted ? eventDomId(highlightedEventId as string) : undefined}
            className={`trace-step ${step.level} ${step.status} ${isHighlighted ? 'action-highlight' : ''}`}
          >
            <header>
              <strong>{STEP_LABELS[step.step] ?? step.step}</strong>
              <span>{STATUS_LABELS[step.status] ?? step.status}</span>
            </header>
            <p>{step.latest.message}</p>
            <small>
              {step.events.length} event · {Math.round(step.progress * 100)}% ·{' '}
              seq {step.events[0]?.seq ?? '?'}-{step.latest.seq} ·{' '}
              {step.latest.created_at ? new Date(step.latest.created_at).toLocaleTimeString() : 'pending time'}
            </small>
            {isHighlighted && <small className="trace-highlight-note">Action 审计定位到事件 {highlightedEventId}</small>}
            {step.latest.artifact_asset_id && <code>{step.latest.artifact_asset_id}</code>}
            <MetadataSummary metadata={step.latest.metadata ?? {}} />
          </li>
          )
        })}
      </ol>
    </section>
  )
}

function eventDomId(eventId: string): string {
  return `trace-event-${eventId.replace(/[^a-zA-Z0-9_-]/g, '_')}`
}

function RuntimeTraceMiniPanel({ runtime }: { runtime?: JobRuntimeStateResponse | null }) {
  if (!runtime) {
    return (
      <div className="runtime-mini empty">
        <span>Runtime</span>
        <small>等待任务运行时状态</small>
      </div>
    )
  }
  const providerTasks = runtime.provider_tasks ?? []
  const checkpoints = runtime.checkpoints ?? []
  const latestTask = providerTasks.at(-1) ?? null
  const latestCheckpoint = checkpoints.at(-1) ?? null
  const activeCheckpoint = [...checkpoints].reverse().find((checkpoint) => checkpoint.status !== 'completed') ?? latestCheckpoint
  return (
    <div className="runtime-mini">
      <div>
        <span>Runtime</span>
        <strong>{STATUS_LABELS[runtime.status] ?? runtime.status}</strong>
        <small>{runtime.current_step ? STEP_LABELS[runtime.current_step] ?? runtime.current_step : 'no active step'}</small>
      </div>
      <div>
        <span>Provider Task</span>
        {latestTask ? (
          <>
            <strong>{latestTask.provider_id} · {STATUS_LABELS[latestTask.status] ?? latestTask.status}</strong>
            <small>{latestTask.step} · attempt {latestTask.attempt}</small>
            <code>{latestTask.provider_task_id ?? latestTask.task_record_id}</code>
          </>
        ) : (
          <small>本地资产步骤，无外部 provider task</small>
        )}
      </div>
      <div>
        <span>Checkpoint</span>
        {activeCheckpoint ? (
          <>
            <strong>{STEP_LABELS[activeCheckpoint.step] ?? activeCheckpoint.step} · {activeCheckpoint.status}</strong>
            <small>{activeCheckpoint.resume_policy} · attempt {activeCheckpoint.attempt}</small>
          </>
        ) : (
          <small>暂无 checkpoint</small>
        )}
      </div>
      <div>
        <span>Recovery</span>
        <strong>{runtime.resumable ? '可请求恢复' : '不可恢复'} · {runtime.cancellable ? '可请求取消' : '不可取消'}</strong>
        <small>{latestTask?.last_seen_at ? `last seen ${new Date(latestTask.last_seen_at).toLocaleTimeString()}` : 'no provider heartbeat'}</small>
      </div>
    </div>
  )
}

function groupEventsByStep(events: JobEvent[]): StepGroup[] {
  const groups = new Map<string, JobEvent[]>()
  for (const event of events) {
    const current = groups.get(event.step) ?? []
    current.push(event)
    groups.set(event.step, current)
  }
  return Array.from(groups.entries()).map(([step, groupedEvents]) => {
    const latest = groupedEvents.at(-1) as JobEvent
    return {
      step,
      latest,
      events: groupedEvents,
      progress: latest.progress ?? 0,
      level: latest.level ?? 'info',
      status: latest.status,
    }
  })
}

function MetadataSummary({ metadata }: { metadata: Record<string, unknown> }) {
  const entries = Object.entries(metadata).filter(([key, value]) => key !== 'progress' && value !== null && value !== undefined)
  if (!entries.length) return null

  return (
    <dl className="trace-metadata">
      {entries.slice(0, 4).map(([key, value]) => (
        <div key={key}>
          <dt>{key}</dt>
          <dd>{formatMetadataValue(value)}</dd>
        </div>
      ))}
    </dl>
  )
}

function formatMetadataValue(value: unknown): string {
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  return JSON.stringify(value)
}
