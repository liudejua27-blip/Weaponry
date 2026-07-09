import { useCallback, useEffect, useMemo, useState } from 'react'
import { JobTimeline } from './JobTimeline'
import type { forgeApi } from '../../shared/api/forgeApi'
import type { JobActionAuditEntry, JobDetail, JobRuntimeStateResponse, JobStatus, JobSummary } from '../../shared/types'

type Props = {
  api: typeof forgeApi
  activeJobId?: string | null
  actionStatus?: string | null
  onRestoreJob: (jobId: string) => Promise<JobDetail>
  recentJobIds?: string[]
  desktopNotifications?: DesktopJobNotification[]
  onRetryJob?: (jobId: string) => void
  onRetryFromStep?: (jobId: string, stepName: string) => void
  onCancelJob?: (jobId: string) => void
  onOpenSettings?: () => void
  onSkip3D?: () => void
}

type DesktopJobNotification = {
  id: string
  jobId: string
  jobType: string
  status: string
  weaponId: string
  message: string
  createdAt: string
}

const STATUS_FILTERS: Array<{ value: '' | JobStatus; label: string }> = [
  { value: '', label: '全部' },
  { value: 'running', label: '运行中' },
  { value: 'waiting_provider', label: '等待 Provider' },
  { value: 'waiting_user', label: '等待处理' },
  { value: 'failed', label: '失败' },
  { value: 'partial_succeeded', label: '部分完成' },
  { value: 'cancelled', label: '已取消' },
  { value: 'succeeded', label: '已完成' },
]

const JOB_TYPE_LABELS: Record<string, string> = {
  create_weapon: '创建方案',
  patch_image: '局部修改',
  generate_3d: '3D 粗模',
  export_unity: 'Unity 导出',
}

const STATUS_LABELS: Record<string, string> = {
  created: '已创建',
  queued: '排队中',
  running: '运行中',
  waiting_provider: '等待 Provider',
  waiting_user: '等待处理',
  retrying: '等待重试',
  succeeded: '已完成',
  failed: '失败',
  cancelled: '已取消',
  partial_succeeded: '部分完成',
}

const FILTER_STORAGE_KEY = 'wushen.jobCenter.filters'

export function JobCenterPanel({
  api,
  activeJobId,
  actionStatus,
  onRestoreJob,
  recentJobIds = [],
  desktopNotifications = [],
  onRetryJob,
  onRetryFromStep,
  onCancelJob,
  onOpenSettings,
  onSkip3D,
}: Props) {
  const savedFilters = useMemo(() => readSavedFilters(), [])
  const [query, setQuery] = useState(savedFilters.query)
  const [status, setStatus] = useState<'' | JobStatus>(savedFilters.status)
  const [errorCode, setErrorCode] = useState(savedFilters.errorCode)
  const [manualJobId, setManualJobId] = useState('')
  const [jobs, setJobs] = useState<JobSummary[]>([])
  const [nextCursor, setNextCursor] = useState<string | null>(null)
  const [selectedJobId, setSelectedJobId] = useState<string | null>(activeJobId ?? null)
  const [selectedDetail, setSelectedDetail] = useState<JobDetail | null>(null)
  const [selectedRuntime, setSelectedRuntime] = useState<JobRuntimeStateResponse | null>(null)
  const [actions, setActions] = useState<JobActionAuditEntry[]>([])
  const [highlightedEventId, setHighlightedEventId] = useState<string | null>(null)
  const [listStatus, setListStatus] = useState<'idle' | 'loading' | 'error'>('idle')
  const [detailStatus, setDetailStatus] = useState<'idle' | 'loading' | 'error'>('idle')
  const [message, setMessage] = useState<string | null>(null)
  const [notificationPermission, setNotificationPermission] = useState(() => readNotificationPermission())

  const filters = useMemo(() => ({
    query: query.trim(),
    status,
    errorCode: errorCode.trim(),
  }), [errorCode, query, status])

  useEffect(() => {
    writeSavedFilters(filters)
  }, [filters])

  const loadJobs = useCallback((cursor?: string) => {
    setListStatus('loading')
    return api.listJobs({
      query: filters.query,
      status: filters.status || undefined,
      errorCode: filters.errorCode || undefined,
      cursor,
      limit: 20,
    })
      .then((response) => {
        setJobs((current) => cursor ? [...current, ...(response.items ?? [])] : response.items ?? [])
        setNextCursor(response.next_cursor ?? null)
        setListStatus('idle')
        if (!selectedJobId && response.items?.[0]) setSelectedJobId(response.items[0].job_id)
        return response
      })
      .catch((caught) => {
        setListStatus('error')
        setMessage(caught instanceof Error ? caught.message : String(caught))
        throw caught
      })
  }, [api, filters, selectedJobId])

  const loadSelectedJob = useCallback((jobId: string) => {
    setDetailStatus('loading')
    setMessage(null)
    setHighlightedEventId(null)
    return Promise.all([
      api.getJob(jobId),
      api.getJobRuntime(jobId).catch(() => null),
      api.listJobActions(jobId, { limit: 30 }).catch(() => ({ items: [] as JobActionAuditEntry[], next_cursor: null })),
    ])
      .then(([detail, runtime, actionList]) => {
        setSelectedDetail(detail)
        setSelectedRuntime(runtime)
        setActions(actionList.items ?? [])
        setDetailStatus('idle')
        return detail
      })
      .catch((caught) => {
        setDetailStatus('error')
        setSelectedDetail(null)
        setSelectedRuntime(null)
        setActions([])
        setMessage(caught instanceof Error ? caught.message : String(caught))
        throw caught
      })
  }, [api])

  useEffect(() => {
    loadJobs().catch(() => undefined)
  }, [loadJobs])

  useEffect(() => {
    if (!activeJobId || selectedJobId) return
    setSelectedJobId(activeJobId)
  }, [activeJobId, selectedJobId])

  useEffect(() => {
    if (!selectedJobId) return
    loadSelectedJob(selectedJobId).catch(() => undefined)
  }, [loadSelectedJob, selectedJobId])

  const restoreSelectedJob = useCallback((jobId: string) => {
    setMessage('正在恢复并订阅事件流...')
    onRestoreJob(jobId)
      .then((detail) => {
        setSelectedJobId(detail.job_id)
        setSelectedDetail(detail)
        setMessage(`已恢复并订阅 ${detail.job_id}`)
        loadJobs().catch(() => undefined)
        loadSelectedJob(detail.job_id).catch(() => undefined)
      })
      .catch((caught) => setMessage(caught instanceof Error ? caught.message : String(caught)))
  }, [loadJobs, loadSelectedJob, onRestoreJob])

  const restoreManualJob = useCallback(() => {
    const jobId = manualJobId.trim()
    if (!jobId) {
      setMessage('请输入 job id')
      return
    }
    restoreSelectedJob(jobId)
  }, [manualJobId, restoreSelectedJob])

  const refreshAfterAction = useCallback((jobId: string) => {
    window.setTimeout(() => {
      loadJobs().catch(() => undefined)
      loadSelectedJob(jobId).catch(() => undefined)
    }, 600)
  }, [loadJobs, loadSelectedJob])

  const retrySelectedJob = useCallback((jobId: string) => {
    onRetryJob?.(jobId)
    refreshAfterAction(jobId)
  }, [onRetryJob, refreshAfterAction])

  const retrySelectedJobFromStep = useCallback((jobId: string, stepName: string) => {
    onRetryFromStep?.(jobId, stepName)
    refreshAfterAction(jobId)
  }, [onRetryFromStep, refreshAfterAction])

  const cancelSelectedJob = useCallback((jobId: string) => {
    onCancelJob?.(jobId)
    refreshAfterAction(jobId)
  }, [onCancelJob, refreshAfterAction])

  const locateActionEvent = useCallback((eventId: string) => {
    setHighlightedEventId(eventId)
    setMessage(`已定位 action 对应事件 ${eventId}`)
  }, [])

  const resetFilters = useCallback(() => {
    setQuery('')
    setStatus('')
    setErrorCode('')
  }, [])

  const requestNotifications = useCallback(() => {
    if (typeof window === 'undefined' || !('Notification' in window)) {
      setNotificationPermission('unsupported')
      return
    }
    Notification.requestPermission()
      .then((permission) => setNotificationPermission(permission))
      .catch(() => setNotificationPermission('denied'))
  }, [])

  const visibleErrorCodes = useMemo(() => {
    const codes = new Set(jobs.map((job) => job.error_code).filter(Boolean) as string[])
    return Array.from(codes).sort()
  }, [jobs])

  return (
    <section className="job-center" aria-label="任务中心">
      <div className="job-center-sidebar">
        <div className="job-center-block">
          <h1>任务中心</h1>
          <p className="muted">搜索历史任务、恢复 job、查看 action 审计和失败原因。</p>
        </div>

        <div className="manual-restore">
          <label htmlFor="manual-job-id">手动恢复 job</label>
          <div className="manual-restore-row">
            <input
              id="manual-job-id"
              value={manualJobId}
              onChange={(event) => setManualJobId(event.target.value)}
              placeholder="例如 job_20260704_0001"
            />
            <button onClick={restoreManualJob}>恢复并订阅事件流</button>
          </div>
        </div>

        <section className="job-wakeup-panel" aria-label="最近任务唤醒">
          <header>
            <strong>最近任务唤醒</strong>
            <span>{recentJobIds.length} 条</span>
          </header>
          {recentJobIds.length ? (
            <ol>
              {recentJobIds.slice(0, 5).map((jobId) => (
                <li key={jobId}>
                  <button onClick={() => restoreSelectedJob(jobId)}>
                    <span>唤醒任务</span>
                    <code>{jobId}</code>
                  </button>
                </li>
              ))}
            </ol>
          ) : (
            <p className="muted">暂无最近任务。</p>
          )}
        </section>

        <section className="desktop-notification-panel" aria-label="桌面通知中心">
          <header>
            <strong>桌面通知中心</strong>
            <span>{notificationPermissionLabel(notificationPermission)}</span>
          </header>
          {notificationPermission === 'default' && (
            <button className="notification-permission-button" onClick={requestNotifications}>启用系统通知</button>
          )}
          {desktopNotifications.length ? (
            <ol>
              {desktopNotifications.slice(0, 5).map((notification) => (
                <li key={notification.id}>
                  <div>
                    <span className={`job-state-chip ${notification.status}`}>{STATUS_LABELS[notification.status] ?? notification.status}</span>
                    <small>{new Date(notification.createdAt).toLocaleString()}</small>
                  </div>
                  <p>{notification.message}</p>
                  <button onClick={() => restoreSelectedJob(notification.jobId)}>
                    打开任务 <code>{notification.jobId}</code>
                  </button>
                </li>
              ))}
            </ol>
          ) : (
            <p className="muted">任务完成、失败或取消后会在这里留下本机通知记录。</p>
          )}
        </section>

        <div className="job-filters" aria-label="任务过滤">
          <label>
            搜索历史任务
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="job id / 武器名 / 步骤 / 错误码"
            />
          </label>
          <label>
            状态
            <select value={status} onChange={(event) => setStatus(event.target.value as '' | JobStatus)}>
              {STATUS_FILTERS.map((item) => <option key={item.value || 'all'} value={item.value}>{item.label}</option>)}
            </select>
          </label>
          <label>
            失败原因
            <input
              list="job-error-codes"
              value={errorCode}
              onChange={(event) => setErrorCode(event.target.value)}
              placeholder="PROVIDER_TIMEOUT"
            />
            <datalist id="job-error-codes">
              {visibleErrorCodes.map((code) => <option key={code} value={code} />)}
            </datalist>
          </label>
          <button onClick={() => loadJobs().catch(() => undefined)}>刷新列表</button>
          <button onClick={resetFilters}>清空筛选</button>
        </div>
        <small className="job-filter-saved">筛选条件会保存在本机，下次打开任务中心自动恢复。</small>

        {message && <div className="job-center-message">{message}</div>}
        {listStatus === 'loading' && <div className="muted">正在加载历史任务...</div>}

        <ol className="job-history-list">
          {jobs.map((job) => (
            <li key={job.job_id} className={selectedJobId === job.job_id ? 'selected' : ''}>
              <button onClick={() => setSelectedJobId(job.job_id)}>
                <span>
                  <strong>{JOB_TYPE_LABELS[job.type] ?? job.type}</strong>
                  <small>{job.weapon_name ?? job.weapon_id ?? '无关联武器'}</small>
                </span>
                <span className={`job-state-chip ${job.status}`}>{STATUS_LABELS[job.status] ?? job.status}</span>
                <code>{job.job_id}</code>
                <small>{job.current_step ?? 'no step'} · {job.event_count} event · {job.action_count} action</small>
                {job.error_code && <small className="error">{job.error_code}: {job.error_message}</small>}
              </button>
            </li>
          ))}
        </ol>
        {!jobs.length && listStatus !== 'loading' && <div className="empty-inline">暂无匹配任务，输入 job id 可直接恢复。</div>}
        {nextCursor && <button className="load-more" onClick={() => loadJobs(nextCursor).catch(() => undefined)}>加载更多</button>}
      </div>

      <div className="job-center-detail">
        <header className="job-detail-header">
          <div>
            <span>历史任务详情</span>
            <strong>{selectedDetail?.job_id ?? selectedJobId ?? '未选择任务'}</strong>
            <small>{selectedDetail ? `${JOB_TYPE_LABELS[selectedDetail.type] ?? selectedDetail.type} · ${STATUS_LABELS[selectedDetail.status] ?? selectedDetail.status}` : '选择左侧任务查看执行轨迹'}</small>
          </div>
          <div className="button-row">
            <button disabled={!selectedJobId} onClick={() => selectedJobId && loadSelectedJob(selectedJobId).catch(() => undefined)}>刷新详情</button>
            <button disabled={!selectedJobId} onClick={() => selectedJobId && restoreSelectedJob(selectedJobId)}>恢复到工作台</button>
          </div>
        </header>

        {detailStatus === 'loading' && <div className="muted">正在加载任务详情...</div>}
        {selectedDetail ? (
          <>
            <div className="job-detail-metrics">
              <StatusMetric label="状态" value={STATUS_LABELS[selectedDetail.status] ?? selectedDetail.status} />
              <StatusMetric label="当前步骤" value={selectedDetail.current_step ?? 'no step'} />
              <StatusMetric label="输出版本" value={stringValue(selectedDetail.outputs?.current_version_id)} />
              <StatusMetric label="输出模型" value={stringValue(selectedDetail.outputs?.current_model_id)} />
            </div>
            {selectedDetail.error && (
              <div className="failure-panel">
                <strong>失败原因</strong>
                <span>{selectedDetail.error.code}</span>
                <p>{selectedDetail.error.message}</p>
              </div>
            )}
            <JobTimeline
              events={(selectedDetail.events ?? []).slice().sort(compareJobEvents)}
              jobId={selectedDetail.job_id}
              streamStatus={selectedDetail.status === 'succeeded' ? 'closed' : 'idle'}
              runtime={selectedRuntime}
              actionStatus={actionStatus}
              onRetryJob={onRetryJob ? retrySelectedJob : undefined}
              onRetryFromStep={onRetryFromStep ? retrySelectedJobFromStep : undefined}
              onCancelJob={onCancelJob ? cancelSelectedJob : undefined}
              onOpenSettings={onOpenSettings}
              onSkip3D={onSkip3D}
              highlightedEventId={highlightedEventId}
            />
            <ActionAuditList actions={actions} highlightedEventId={highlightedEventId} onLocateEvent={locateActionEvent} />
          </>
        ) : (
          <div className="empty-stage compact">
            <strong>选择一个历史任务</strong>
            <p>任务中心会显示事件轨迹、Runtime、失败原因和 action 审计。查看历史不会自动切换当前工作台上下文。</p>
          </div>
        )}
      </div>
    </section>
  )
}

function StatusMetric({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

function ActionAuditList({
  actions,
  highlightedEventId,
  onLocateEvent,
}: {
  actions: JobActionAuditEntry[]
  highlightedEventId?: string | null
  onLocateEvent: (eventId: string) => void
}) {
  return (
    <section className="action-audit" aria-label="Action 审计">
      <header>
        <strong>Action 审计</strong>
        <span>{actions.length} 条</span>
      </header>
      {actions.length ? (
        <ol>
          {actions.map((action) => (
            <li key={action.action_id} className={action.event_id && action.event_id === highlightedEventId ? 'selected' : ''}>
              <div>
                <strong>{action.action_type}</strong>
                <span>{action.status}</span>
              </div>
              <p>{action.message}</p>
              <small>{`${action.previous_job_status} -> ${action.resulting_job_status}`} · {action.requested_step ?? 'no step'} · {new Date(action.created_at).toLocaleString()}</small>
              {action.event_id && (
                <button className="link-button" onClick={() => onLocateEvent(action.event_id as string)}>
                  定位事件 {action.event_id}
                </button>
              )}
            </li>
          ))}
        </ol>
      ) : (
        <p className="muted">暂无人工 action。重试、从失败步骤重试和取消请求会在这里留下审计记录。</p>
      )}
    </section>
  )
}

function compareJobEvents(a: NonNullable<JobDetail['events']>[number], b: NonNullable<JobDetail['events']>[number]) {
  return (a.seq ?? 0) - (b.seq ?? 0)
}

function stringValue(value: unknown): string {
  return typeof value === 'string' && value ? value : '暂无'
}

function readSavedFilters(): { query: string; status: '' | JobStatus; errorCode: string } {
  try {
    const raw = localStorage.getItem(FILTER_STORAGE_KEY)
    const parsed = raw ? JSON.parse(raw) : {}
    return {
      query: typeof parsed.query === 'string' ? parsed.query : '',
      status: isJobStatus(parsed.status) ? parsed.status : '',
      errorCode: typeof parsed.errorCode === 'string' ? parsed.errorCode : '',
    }
  } catch {
    return { query: '', status: '', errorCode: '' }
  }
}

function writeSavedFilters(filters: { query: string; status: string; errorCode: string }): void {
  localStorage.setItem(FILTER_STORAGE_KEY, JSON.stringify(filters))
}

function isJobStatus(value: unknown): value is JobStatus {
  return typeof value === 'string' && STATUS_FILTERS.some((item) => item.value === value && item.value !== '')
}

function readNotificationPermission(): NotificationPermission | 'unsupported' {
  if (typeof window === 'undefined' || !('Notification' in window)) return 'unsupported'
  return Notification.permission
}

function notificationPermissionLabel(permission: NotificationPermission | 'unsupported'): string {
  if (permission === 'granted') return '系统通知已启用'
  if (permission === 'denied') return '系统通知已关闭'
  if (permission === 'unsupported') return '系统通知不可用'
  return '系统通知未启用'
}
