import type { WorkbenchStatusBarPresentation } from './workbenchStatusBarPresentation'
import type { ReactElement } from 'react'
import type { AgentThreadSummary } from '../../shared/types'
import { ArrowsLeftRight } from '@phosphor-icons/react'
import {
  WORKFLOW_MODE_LABELS,
  WORKFLOW_MODE_ORDER,
  WORKFLOW_STATUS_LABELS,
  getWorkflowModeStatusTag,
  getWorkflowModeStep,
  type WorkbenchPanelWorkflowMode,
  type WorkflowState,
} from './cadWorkbenchPanelGlobalActions'

type CadWorkbenchStatusModeCard = {
  id: WorkbenchPanelWorkflowMode
  title: string
  state: WorkflowState
}

function relativeTimeLabel(value: string): string {
  const now = Date.now()
  const eventAt = new Date(value).getTime()
  if (!Number.isFinite(eventAt)) return '刚刚'
  const diff = now - eventAt
  if (!Number.isFinite(diff) || diff <= 0) return '刚刚'

  const minutes = Math.floor(diff / (1000 * 60))
  if (minutes < 5) return '刚刚'
  if (minutes < 60) return `${minutes} 分钟前`

  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} 小时前`
  if (hours < 48) return '昨天'

  const days = Math.floor(hours / 24)
  return `${days} 天前`
}

function pickThreadsForTimeline(threads: readonly AgentThreadSummary[]): AgentThreadSummary[] {
  return [...threads]
    .sort((left, right) => (
      new Date(right.updated_at ?? right.created_at).getTime() - new Date(left.updated_at ?? left.created_at).getTime()
    ))
    .slice(0, 6)
}

function statusTextOfThread(status: AgentThreadSummary['status']): string {
  if (status === 'active') return '进行中'
  if (status === 'error') return '失败'
  if (status === 'archived') return '历史版本'
  return '已完成'
}

function threadSummaryText(thread: AgentThreadSummary): string {
  const summaryText = thread.summary?.trim()
  if (summaryText && /[\u4e00-\u9fff]/.test(summaryText)) return summaryText
  return statusTextOfThread(thread.status)
}

type CadWorkbenchPanelStatusBarProps = {
  workbenchStatusBar: WorkbenchStatusBarPresentation
  showCompactSidebar: boolean
  workflowState?: Record<WorkbenchPanelWorkflowMode, WorkflowState>
  activeMode?: WorkbenchPanelWorkflowMode | null
  hasSelectedComponent?: boolean
  isProjectUnsaved?: boolean
  threadSummaries?: readonly AgentThreadSummary[]
  activeThreadId?: string | null
  historyPreview?: {
    threadId: string
    returnThreadId: string | null
    mode: 'compare' | 'restore'
    title: string
  } | null
  onModeSelect?: (mode: WorkbenchPanelWorkflowMode) => void
  onThreadSelect?: (threadId: string) => void
  onExitHistoryPreview?: () => void
  onVersionRestore?: (threadId: string) => void
  onVersionCompare?: (threadId: string) => void
  technicalMessage?: string | null
}

const BEGINNER_MODE_HINT = '零基础模式：先输入一句话开始 AI 生成，再用快速修改补充需求。'

const WORKFLOW_SUMMARY_HINT: Record<WorkbenchPanelWorkflowMode, Partial<Record<WorkflowState['status'], string>>> = {
  generate: {
    empty: '空项目，尚未开始生成。',
    ready: '可输入需求，开始 AI 生成。',
    active: 'AI 正在分析你的需求。',
    processing: 'AI 生成中，请稍后。',
    done: '模型已生成。',
    blocked: '未就绪，先完成前置步骤。',
    error: '生成失败，请重试。',
    network: '网络不稳定，稍后自动重试。',
    saving: '有未保存改动。',
  },
  modify: {
    empty: '先完成 AI 生成后再修改。',
    ready: '可开始 AI 修改。',
    active: 'AI 正在执行修改。',
    processing: 'AI 修改中，请稍后。',
    done: '修改结果已提交。',
    blocked: '需先完成一次生成。',
    error: '修改失败，请重试。',
    network: '网络不稳定，稍后再试。',
    saving: '有未确认修改，请先保存。',
  },
  preview: {
    empty: '请先生成并确认可展示版本。',
    ready: '可直接查看展示。',
    active: '展示已聚焦当前版本。',
    processing: '展示内容刷新中。',
    done: '展示完成。',
    blocked: '先确认版本后再展示。',
    error: '展示失败，请重试。',
    network: '展示同步失败，请重试。',
    saving: '有未确认变更，请先保存。',
  },
  export: {
    ready: '可开始导出。',
    active: '准备导出。',
    processing: '正在导出。',
    done: '导出已完成。',
    blocked: '先保存版本后再导出。',
    error: '导出失败，请重试。',
    network: '网络异常，稍后重试。',
    saving: '请先确认版本后导出。',
    empty: '先完成展示并确认版本后再导出。',
  },
}

function getPublicWorkflowHint(mode: WorkbenchPanelWorkflowMode, state: WorkflowState): string {
  const detailHint = WORKFLOW_SUMMARY_HINT[mode][state.status]
  if (detailHint) return detailHint
  return state.hint || WORKFLOW_STATUS_LABELS[state.status]
}

export function CadWorkbenchPanelStatusBar({
  workbenchStatusBar,
  showCompactSidebar,
  workflowState,
  activeMode,
  hasSelectedComponent = false,
  isProjectUnsaved = false,
  threadSummaries = [],
  activeThreadId,
  historyPreview = null,
  onModeSelect,
  onThreadSelect,
  onExitHistoryPreview,
  onVersionRestore,
  onVersionCompare,
  technicalMessage,
}: CadWorkbenchPanelStatusBarProps): ReactElement {
  const statusBarClassName = showCompactSidebar
    ? 'cad-status-bar is-beginner'
    : 'cad-status-bar'
  const workflowCards: CadWorkbenchStatusModeCard[] = workflowState
    ? WORKFLOW_MODE_ORDER.map((id) => ({
      id,
      title: WORKFLOW_MODE_LABELS[id],
      state: workflowState[id],
    }))
    : []
  const isModePipelineBusy = workflowState
    ? Object.values(workflowState).some(
      ({ status }) => status === 'processing' || status === 'network' || status === 'saving',
    )
  : false

  const isWorkflowModeLocked = (
    modeId: WorkbenchPanelWorkflowMode,
    modeState: WorkflowState,
  ): boolean => (
    isModePipelineBusy
    && activeMode != null
    && activeMode !== modeId
    && modeState.status !== 'done'
  )

  const getModeSwitchDisabledReason = (
    modeId: WorkbenchPanelWorkflowMode,
    modeState: WorkflowState,
  ): string | null => {
    if (modeState.status === 'blocked') return '未就绪，先完成前置步骤。'
    if (isWorkflowModeLocked(modeId, modeState)) return '当前阶段执行中，完成后可切换。'
    return null
  }
  const timelineThreads = pickThreadsForTimeline(threadSummaries)
  const showVersionTimeline = timelineThreads.length > 0
  const onTimelineNavigate = onThreadSelect ?? onVersionRestore ?? onVersionCompare
  const canNavigateThread = Boolean(onTimelineNavigate)
  const activeModeState = activeMode && workflowState ? workflowState[activeMode] : null
  const showTechnicalDetails = Boolean(
    activeModeState
    && (activeModeState.status === 'error' || activeModeState.status === 'network')
  )
  const technicalDetailText = (technicalMessage ?? activeModeState?.hint ?? '').trim()

  return (
    <footer className={statusBarClassName} role="status" aria-live="polite" aria-label="工作台状态">
      {showCompactSidebar ? (
        <>
          <span>{BEGINNER_MODE_HINT}</span>
          <span className="status-spacer" />
          <span>{workbenchStatusBar.assistantStateText}</span>
        </>
      ) : (
        <>
          <div className="cad-status-meta">
            <span>{workbenchStatusBar.assistantStateText}</span>
            <span>版本：{workbenchStatusBar.versionText}</span>
            <span className="status-spacer" />
            <span>{workbenchStatusBar.assetStateText}</span>
            <span>{hasSelectedComponent ? '已选中组件' : '未选中组件'}</span>
            <span>{workbenchStatusBar.qualityText}</span>
          </div>
          <div className="cad-status-timeline">
                {workflowCards.length > 0 ? (
            <div className="cad-workflow-timeline" role="list" aria-label="主流程阶段">
                <span className="cad-status-section-title">AI设计流程</span>
                <div className="cad-workflow-track-list" role="presentation">
                  {workflowCards.map((card) => {
                const isLocked = isWorkflowModeLocked(card.id, card.state)
                const statusTag = getWorkflowModeStatusTag(card.id, card.state.status)
                const switchDisabledReason = getModeSwitchDisabledReason(card.id, card.state)
                    const isActive = activeMode === card.id
                    const isSwitchDisabled = switchDisabledReason !== null || !onModeSelect
                    const workflowHint = getPublicWorkflowHint(card.id, card.state)
                    return (
                      <button
                        key={card.id}
                        type="button"
                        className={`cad-workflow-track-item ${isActive ? 'is-active' : ''} ${isLocked ? 'is-locked' : ''} is-${card.state.status}`}
                        role="button"
                        aria-current={isActive ? 'step' : undefined}
                        onClick={() => onModeSelect?.(card.id)}
                        disabled={isSwitchDisabled}
                        aria-label={`切换到${card.title}：${workflowHint}`}
                        title={isSwitchDisabled
                          ? (switchDisabledReason ?? '当前模式暂不可切换')
                          : isActive
                            ? `${card.title}（${workflowHint}）`
                            : `${card.title}（${statusTag}）`}
                        aria-pressed={activeMode === card.id}
                      >
                        <span className={`cad-workflow-track-dot is-${card.state.status}`} aria-hidden="true" />
                        <span className="cad-workflow-track-step" aria-hidden="true">{getWorkflowModeStep(card.id)}</span>
                        <span className="cad-workflow-track-copy">
                          <strong>{card.title}</strong>
                          <small>{workflowHint}</small>
                          <span className={`cad-workflow-track-tag is-${card.state.status}`}>{statusTag}</span>
                        </span>
                      </button>
                    )
                  })}
                </div>
                {showTechnicalDetails ? (
                  <details className="cad-status-technical">
                    <summary>查看技术详情</summary>
                    <div>{technicalDetailText || activeModeState?.hint}</div>
                  </details>
                ) : null}
              </div>
            ) : null}
            <div className="cad-version-timeline" aria-label="设计历史">
              <span className="cad-status-section-title">设计历史</span>
              {historyPreview ? (
                <div className="cad-history-preview-banner" role="status" aria-live="polite">
                  <div className="cad-history-preview-copy">
                    <strong>{historyPreview.mode === 'compare' ? '历史会话对比预览' : '历史会话恢复预览'}</strong>
                    <span>{historyPreview.title} · 当前设计版本未改变</span>
                  </div>
                  {onExitHistoryPreview ? (
                    <button type="button" onClick={onExitHistoryPreview} className="cad-history-preview-exit">
                      返回当前会话
                    </button>
                  ) : null}
                </div>
              ) : null}
              {showVersionTimeline ? (
                <div className="cad-version-timeline-track" role="list">
                  {timelineThreads.map((thread, threadIndex) => {
                    const isActive = activeThreadId === thread.thread_id
                    const canCompare = Boolean(onVersionCompare)
                    const canRestore = Boolean(onVersionRestore)
                    const versionNo = timelineThreads.length - threadIndex
                    const timelineTitle = thread.title || '未命名版本'
                    const timelineSummary = threadSummaryText(thread)

                    return (
                      <article
                        key={thread.thread_id}
                        className={`cad-version-timeline-item ${isActive ? 'is-active' : ''} ${canNavigateThread ? 'is-navigable' : ''}`}
                        role="listitem"
                        aria-label={`版本记录：${timelineTitle}，状态：${timelineSummary}`}
                        tabIndex={canNavigateThread ? 0 : -1}
                        onClick={canNavigateThread
                          ? () => onTimelineNavigate?.(thread.thread_id)
                          : undefined
                        }
                        onKeyDown={(event) => {
                          if (!canNavigateThread) return
                          if (event.key === 'Enter' || event.key === ' ') {
                            event.preventDefault()
                            onTimelineNavigate?.(thread.thread_id)
                          }
                        }}
                        title={timelineSummary}
                        aria-current={isActive ? 'step' : undefined}
                      >
                        <div className="cad-version-timeline-main">
                          <strong>{`方案 ${versionNo} · ${timelineTitle}`}</strong>
                          <small>{timelineSummary}</small>
                          <span>{relativeTimeLabel(thread.updated_at || thread.created_at)}</span>
                          <span className={`cad-version-timeline-badge ${isActive ? 'is-current' : ''}`}>
                            {isActive ? '当前版本' : '历史版本'}
                          </span>
                        </div>
                        {canRestore || canCompare ? (
                          <div className="cad-version-timeline-actions" role="presentation">
                            {canRestore ? (
                              <button
                                type="button"
                                className="cad-version-timeline-action"
                                onClick={(event) => {
                                  event.preventDefault()
                                  event.stopPropagation()
                                onVersionRestore?.(thread.thread_id)
                                }}
                                aria-label={`恢复版本：${timelineTitle}`}
                                title="打开对应历史会话；当前设计版本不会被覆盖"
                              >
                                <ArrowsLeftRight size={12} />
                                恢复此版本
                              </button>
                            ) : null}
                            {canCompare ? (
                              <button
                                type="button"
                                className="cad-version-timeline-action"
                                disabled={isActive}
                                onClick={(event) => {
                                  event.preventDefault()
                                  event.stopPropagation()
                                onVersionCompare?.(thread.thread_id)
                                }}
                                aria-label={`对比版本：${timelineTitle}`}
                                title="打开对应历史会话与当前会话对比；当前设计版本不会改变"
                              >
                                <ArrowsLeftRight size={12} />
                                与当前对比
                              </button>
                            ) : null}
                          </div>
                        ) : null}
                      </article>
                    )
                  })}
                </div>
              ) : (
                <>
                  <small className="cad-version-timeline-empty">暂无设计历史，完成一次生成后会显示版本记录。</small>
                  <div className="cad-version-timeline-empty-guide" role="note" aria-label="设计版本流程">
                    <div className="cad-version-timeline-empty-steps" aria-hidden="true">
                      <span><b>1</b>生成</span>
                      <i />
                      <span><b>2</b>修改</span>
                      <i />
                      <span><b>3</b>展示</span>
                      <i />
                      <span><b>4</b>导出</span>
                    </div>
                    <p>每次确认后的结果都会成为一个可恢复的设计版本。</p>
                  </div>
                </>
              )}
            </div>
          </div>
        </>
      )}
    </footer>
  )
}
