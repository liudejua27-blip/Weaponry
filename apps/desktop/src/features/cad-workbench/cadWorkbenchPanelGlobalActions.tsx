import {
  ArrowsClockwise,
  Check,
  ClockCounterClockwise,
  DotsThreeVertical,
  Export,
  FolderOpen,
  SpinnerGap,
  Sparkle,
  Eye,
  Lock,
  MagicWand,
  PencilSimple,
} from '@phosphor-icons/react'
import type { ReactElement } from 'react'

export type WorkbenchPanelWorkflowMode = 'generate' | 'modify' | 'preview' | 'export'

export type WorkflowStepStatus =
  | 'empty'
  | 'ready'
  | 'active'
  | 'processing'
  | 'done'
  | 'blocked'
  | 'error'
  | 'network'
  | 'saving'

export type WorkflowState = {
  status: WorkflowStepStatus
  hint: string
}

export type GlobalActionState = {
  canUndo: boolean
  canRedo: boolean
  canImport: boolean
  importingGlb: boolean
  importLabel: string
}

type CadWorkbenchPanelGlobalActionsProps = {
  actions: GlobalActionState
  activeMode: WorkbenchPanelWorkflowMode
  workflowState: Record<WorkbenchPanelWorkflowMode, WorkflowState>
  onUndo: () => void
  onRedo: () => void
  onImport: () => void
  onCheck: () => void
  onOpenAdvanced?: () => void
  onModeSelect: (mode: WorkbenchPanelWorkflowMode) => void
  canGenerateMode?: boolean
  canModifyMode?: boolean
  canPreviewMode?: boolean
  canExportMode?: boolean
  canCheck?: boolean
  showAdvancedActions?: boolean
}

export const WORKFLOW_MODE_ORDER: readonly WorkbenchPanelWorkflowMode[] = ['generate', 'modify', 'preview', 'export'] as const

export const WORKFLOW_MODE_LABELS: Record<WorkbenchPanelWorkflowMode, string> = {
  generate: 'AI生成',
  modify: '修改',
  preview: '展示',
  export: '导出',
}

export const WORKFLOW_STATUS_LABELS: Record<WorkflowStepStatus, string> = {
  empty: '未开始',
  blocked: '未就绪',
  ready: '可开始',
  active: '进行中',
  processing: '进行中',
  done: '已完成',
  error: '请重试',
  network: '网络异常',
  saving: '未保存',
}

export const WORKFLOW_MODE_STATUS_TAG: Record<WorkbenchPanelWorkflowMode, Partial<Record<WorkflowStepStatus, string>>> = {
  generate: {
    empty: '未开始',
    ready: '可开始生成',
    active: '正在分析/生成',
    processing: '模型生成中',
    done: '模型生成完成',
    blocked: '未就绪',
    error: '生成失败',
    network: '网络异常',
    saving: '模型未保存',
  },
  modify: {
    ready: '可开始修改',
    active: '正在理解修改需求',
    processing: '修改中',
    done: '修改已完成',
    blocked: '未就绪',
    error: '修改失败',
    network: '网络异常',
    saving: '有未保存改动',
    empty: '未开始',
  },
  preview: {
    ready: '可查看展示',
    active: '展示已聚焦',
    processing: '正在准备展示',
    done: '可查看展示',
    blocked: '未就绪',
    error: '展示失败',
    network: '网络异常',
    saving: '有未确认改动',
    empty: '未开始',
  },
  export: {
    ready: '可开始导出',
    active: '准备导出',
    processing: '导出中',
    done: '导出可执行',
    blocked: '未就绪',
    error: '导出失败',
    network: '网络异常',
    saving: '请先确认版本',
    empty: '未开始',
  },
}

const workflowIcons: Record<WorkbenchPanelWorkflowMode, ReactElement> = {
  generate: <MagicWand size={14} />,
  modify: <PencilSimple size={14} />,
  preview: <Eye size={14} />,
  export: <Export size={14} />,
}

const WORKFLOW_MODE_BLOCK_HINT: Record<WorkbenchPanelWorkflowMode, string> = {
  generate: '先创建项目，再开始 AI 生成。',
  modify: '先完成一次生成，才能进入 AI 修改。',
  preview: '先确认可展示版本，再进入展示。',
  export: '先确认并提交当前版本，再导出。',
}

export function getWorkflowModeStatusTag(
  modeId: WorkbenchPanelWorkflowMode,
  status: WorkflowStepStatus,
): string {
  const modeTag = WORKFLOW_MODE_STATUS_TAG[modeId][status]
  return modeTag || WORKFLOW_STATUS_LABELS[status]
}

const WORKFLOW_MODE_STATUS_HINT: Record<WorkbenchPanelWorkflowMode, Partial<Record<WorkflowStepStatus, string>>> = {
  generate: {
    empty: '先新建或打开项目后输入需求。',
    ready: '可输入一句话开始生成',
    active: '正在理解你的需求',
    processing: '生成中，请稍候',
    network: '网络异常，稍后重试',
    done: '生成完成',
    saving: '有未保存改动',
    error: '生成失败，请重试',
  },
  modify: {
    blocked: '先完成一次生成',
    ready: '当前模型可修改，输入你的修改需求。',
    active: '正在准备修改内容',
    processing: '修改中，请稍候',
    network: '网络异常，稍后重试',
    done: '修改已完成，可继续发起新修改。',
    saving: '有未确认改动，请先保存',
    error: '修改失败，请重试',
  },
  preview: {
    blocked: '先确认版本再查看',
    ready: '可直接查看当前版本',
    active: '正在聚焦展示',
    processing: '展示刷新中',
    network: '展示加载失败',
    done: '展示完成',
    saving: '有未保存改动，先确认',
    error: '展示失败，请重试',
  },
  export: {
    blocked: '先保存并确认版本',
    ready: '可导出，先确认导出参数。',
    active: '正在准备导出',
    processing: '导出中，请稍候',
    network: '导出受网络影响',
    done: '导出可执行',
    saving: '请先确认版本后再导出',
    error: '导出失败，请重试',
  },
}

function getModeHint(state: WorkflowState): string {
  if (state.hint) return state.hint
  return WORKFLOW_STATUS_LABELS[state.status]
}

function getModeStatusHint(modeId: WorkbenchPanelWorkflowMode, state: WorkflowState): string {
  const statusHint = WORKFLOW_MODE_STATUS_HINT[modeId][state.status]
  return statusHint || getModeHint(state)
}

function getModeA11yLabel(modeId: WorkbenchPanelWorkflowMode, state: WorkflowState): string {
  const mode = WORKFLOW_MODE_LABELS[modeId]
  const status = WORKFLOW_STATUS_LABELS[state.status]
  const hint = getModeStatusHint(modeId, state)
  return `${mode}，状态：${status}，${hint}`
}

function getModeLockHint(mode: WorkbenchPanelWorkflowMode): string {
  return `${WORKFLOW_MODE_LABELS[mode]}正忙，建议完成当前步骤后再进入。`
}

function getMachineVisualState(status: WorkflowStepStatus): 'completed' | 'running' | 'error' | 'saving' | 'blocked' | 'ready' {
  if (status === 'done') return 'completed'
  if (status === 'processing' || status === 'active') return 'running'
  if (status === 'error' || status === 'network') return 'error'
  if (status === 'saving') return 'saving'
  if (status === 'blocked' || status === 'empty') return 'blocked'
  return 'ready'
}

export function getWorkflowModeStep(mode: WorkbenchPanelWorkflowMode): string {
  const index = WORKFLOW_MODE_ORDER.indexOf(mode)
  return index >= 0 ? `0${index + 1}` : ''
}

export function CadWorkbenchPanelGlobalActions({
  actions,
  activeMode,
  workflowState,
  onUndo,
  onRedo,
  onImport,
  onCheck,
  onModeSelect,
  onOpenAdvanced,
  canGenerateMode = true,
  canModifyMode = true,
  canPreviewMode = true,
  canExportMode = true,
  canCheck = false,
  showAdvancedActions = true,
}: CadWorkbenchPanelGlobalActionsProps): ReactElement {
  const modes: Array<{ id: WorkbenchPanelWorkflowMode; disabled: boolean }> = [
    { id: 'generate', disabled: !canGenerateMode },
    { id: 'modify', disabled: !canModifyMode },
    { id: 'preview', disabled: !canPreviewMode },
    { id: 'export', disabled: !canExportMode },
  ]

  const isPipelineBusy = Object.values(workflowState).some(
    ({ status }) => status === 'processing' || status === 'network' || status === 'saving',
  )
  const isModeLocked = (modeId: WorkbenchPanelWorkflowMode): boolean => (
    isPipelineBusy
    && activeMode !== modeId
    && workflowState[modeId].status !== 'done'
  )

  const getModeSwitchHint = (modeId: WorkbenchPanelWorkflowMode, state: WorkflowState): string => {
    if (state.status === 'blocked') {
      return WORKFLOW_MODE_BLOCK_HINT[modeId]
    }

    if (state.status === 'saving') {
      return getWorkflowModeStatusTag(modeId, 'saving')
    }

    if (isModeLocked(modeId) && modeId !== activeMode) {
      return getModeLockHint(modeId)
    }

    return getModeStatusHint(modeId, state)
  }
  const importBusy = actions.importingGlb
  const completedModeCount = WORKFLOW_MODE_ORDER.reduce((total, modeId) => {
    return total + (workflowState[modeId].status === 'done' ? 1 : 0)
  }, 0)
  const completedModeRate = Math.round((completedModeCount / WORKFLOW_MODE_ORDER.length) * 100)
  const order: Record<WorkbenchPanelWorkflowMode, number> = {
    generate: 0,
    modify: 1,
    preview: 2,
    export: 3,
  }
  const activeModeIndex = WORKFLOW_MODE_ORDER.indexOf(activeMode)
  const nextActionHint = (() => {
    let result = '流程完成'
    for (let offset = 1; offset < WORKFLOW_MODE_ORDER.length; offset += 1) {
      const nextModeId = WORKFLOW_MODE_ORDER[activeModeIndex + offset]
      if (!nextModeId) break

      const nextModeState = workflowState[nextModeId].status
      if (nextModeState === 'blocked') {
        result = `${WORKFLOW_MODE_LABELS[nextModeId]}（尚未就绪）`
      } else if (nextModeState === 'done') {
        result = `${WORKFLOW_MODE_LABELS[nextModeId]}（可回看）`
      } else {
        result = `${WORKFLOW_MODE_LABELS[nextModeId]}（${getWorkflowModeStatusTag(nextModeId, nextModeState)}）`
      }
      break
    }

    return result
  })()
  const activeModeState = workflowState[activeMode]
  const activeModeStatusLabel = getWorkflowModeStatusTag(activeMode, activeModeState.status)
  const activeModeStatusHint = getModeStatusHint(activeMode, activeModeState)

  const focusModeByOffset = (modeId: WorkbenchPanelWorkflowMode, delta: number): WorkbenchPanelWorkflowMode => {
    const currentIndex = order[modeId]
    const nextIndex = (currentIndex + delta + modes.length) % modes.length
    return modes[nextIndex].id
  }

  const findAccessibleModeByOffset = (
    modeId: WorkbenchPanelWorkflowMode,
    delta: number,
  ): WorkbenchPanelWorkflowMode | null => {
    for (let offset = 1; offset <= modes.length; offset += 1) {
      const candidate = focusModeByOffset(modeId, delta * offset)
      if (canMoveTo(candidate)) {
        return candidate
      }
    }
    return null
  }

  const canMoveTo = (modeId: WorkbenchPanelWorkflowMode): boolean => {
    const isMode = modes.find((entry) => entry.id === modeId)
    const state = workflowState[modeId]
    if (state.status === 'blocked') return false
    if (isMode?.disabled) return false
    if (modeId === activeMode) return true
    return !isModeLocked(modeId)
  }

  const workflowMachine = WORKFLOW_MODE_ORDER.map((modeId, index) => {
    const status = workflowState[modeId].status
    const modeStateHint = getModeSwitchHint(modeId, workflowState[modeId])
    const canEnterMode = canMoveTo(modeId)
    return {
      id: modeId,
      step: index + 1,
      isCurrent: activeMode === modeId,
      status,
      statusTag: getWorkflowModeStatusTag(modeId, status),
      visualState: getMachineVisualState(status),
      isLocked: isModeLocked(modeId),
      label: WORKFLOW_MODE_LABELS[modeId],
      tooltip: modeStateHint,
      canEnterMode,
    }
  })

  return (
    <div className="cad-global-actions" aria-label="工作区工具">
      <div className="cad-workflow-mode-switch" role="radiogroup" aria-label="主流程状态机">
        {modes.map((mode, index) => {
          const modeStatus = workflowState[mode.id].status
          const modeShortcut = index + 1
          const isLocked = isModeLocked(mode.id)
          const isModeButtonDisabled = mode.disabled || modeStatus === 'blocked' || (isLocked && mode.id !== activeMode)
          const isActive = activeMode === mode.id
          const modeHint = getModeSwitchHint(mode.id, workflowState[mode.id])
          const modeTag = getWorkflowModeStatusTag(mode.id, modeStatus)
          const isModeBusy = modeStatus === 'processing' || modeStatus === 'network'
          const isSaving = modeStatus === 'saving'
          const modeTitle = isLocked
                  ? '当前阶段执行中，完成后可切换'
                  : modeHint
          const bridgeState = (() => {
            if (index < activeModeIndex) return 'is-completed'
            if (index === activeModeIndex) return 'is-current'
            return modeStatus === 'blocked' || isLocked ? 'is-blocked' : 'is-upcoming'
          })()

          return (
            <span key={mode.id} className="cad-workflow-mode-item">
              <button
                type="button"
                className={`cad-workflow-mode cad-workflow-mode--${mode.id} cad-workflow-mode--${workflowState[mode.id].status} ${
                  isLocked ? 'cad-workflow-mode--locked' : ''
                } ${
                  isActive ? 'cad-workflow-mode--active' : ''
                }`}
                data-workbench-mode={mode.id}
                data-workbench-mode-status={workflowState[mode.id].status}
                role="radio"
                aria-selected={isActive}
                aria-checked={isActive}
                aria-pressed={isActive}
                aria-current={isActive ? 'step' : undefined}
                aria-busy={isModeBusy || isSaving}
                aria-disabled={isModeButtonDisabled ? true : undefined}
                aria-keyshortcuts={`Left Right Alt+${modeShortcut}`}
                tabIndex={isActive ? 0 : -1}
                title={`${WORKFLOW_MODE_LABELS[mode.id]} · ${modeTag} · ${modeTitle} · 快捷键：Alt+${modeShortcut}`}
                onClick={() => {
                  if (isModeButtonDisabled) {
                    return
                  }
                  onModeSelect(mode.id)
                }}
                onKeyDown={(event) => {
                  if (event.key === 'ArrowLeft') {
                    event.preventDefault()
                    const previous = findAccessibleModeByOffset(mode.id, -1)
                    if (previous) onModeSelect(previous)
                    return
                  }
                  if (event.key === 'ArrowRight') {
                    event.preventDefault()
                    const next = findAccessibleModeByOffset(mode.id, 1)
                    if (next) onModeSelect(next)
                    return
                  }
                  if (/^[1-4]$/.test(event.key)) {
                    event.preventDefault()
                    const selectedMode = WORKFLOW_MODE_ORDER[Number(event.key) - 1]
                    if (!selectedMode) return

                    const selectedState = workflowState[selectedMode]
                    if (selectedState.status === 'blocked' || (selectedMode !== mode.id && isModeLocked(selectedMode))) {
                      return
                    }
                    onModeSelect(selectedMode)
                  }
                }}
                disabled={isModeButtonDisabled}
                aria-label={mode.id === 'export' ? '流程导出' : getModeA11yLabel(mode.id, workflowState[mode.id])}
              >
                <span className="cad-workflow-mode-shortcut" aria-hidden="true">{modeShortcut}</span>
                {(isModeButtonDisabled
                  || isLocked
                  || modeStatus === 'saving')
                  ? <Lock size={12} className="cad-workflow-mode-lock" aria-hidden="true" />
                  : null}
                <span className="cad-workflow-mode-step" aria-hidden="true">{getWorkflowModeStep(mode.id)}</span>
                {isModeBusy ? (
                  <SpinnerGap size={14} className="cad-workflow-mode-spin" aria-hidden="true" />
                ) : workflowIcons[mode.id]}
                <span
                  className={`cad-workflow-mode-dot cad-workflow-mode-dot--${modeStatus}`}
                  aria-hidden="true"
                />
          <span className="cad-workflow-mode-copy">
            <span className="cad-workflow-mode-main">
              {WORKFLOW_MODE_LABELS[mode.id]}
            </span>
            <span className={`cad-workflow-mode-tag is-${modeStatus}`} aria-hidden="true">
              {isLocked ? '进行中 · 稍后可切换' : modeTag}
            </span>
          </span>
              </button>
              {index < modes.length - 1 ? (
                <span
                  className={`cad-workflow-mode-bridge ${bridgeState}`}
                  aria-hidden="true"
                />
              ) : null}
            </span>
          )
        })}
      </div>
      <div className="cad-workflow-mode-summary" role="status" aria-live="polite">
        <span className="cad-workflow-mode-summary-label">
          {`${WORKFLOW_MODE_LABELS[activeMode]} · ${activeModeStatusLabel}`}
        </span>
        <span
          className="cad-workflow-mode-summary-progress"
          role="progressbar"
          aria-label={`流程进度 ${completedModeCount}/4`}
          aria-valuemin={0}
          aria-valuemax={WORKFLOW_MODE_ORDER.length}
          aria-valuenow={completedModeCount}
        >
          <span
            className="cad-workflow-mode-summary-progress-fill"
            style={{ width: `${completedModeRate}%` }}
            aria-hidden="true"
          />
          <span className="cad-workflow-mode-summary-progress-text">进度 {completedModeCount}/4</span>
        </span>
        <span className="cad-workflow-mode-inline-status">
          {`下一步：${nextActionHint}`}
        </span>
      </div>
      <span className="cad-workflow-mode-inline-status" role="status" aria-live="polite">
        {activeModeStatusHint}
      </span>
      <span className="cad-workflow-mode-inline-status">
        快捷键：Alt+1~4 · ←→
      </span>
      <span className="cad-workflow-mode-state-machine-wrap" role="status" aria-live="polite">
        <span className="cad-workflow-state-machine-title">流程状态机</span>
        <span
          className="cad-workflow-state-machine"
          role="list"
          aria-label="AI 设计流程状态机"
          title="AI 设计流程状态机"
        >
          {workflowMachine.map((node) => (
            <span className="cad-workflow-state-machine-item" key={node.id} role="listitem">
              <button
                type="button"
                className={`cad-workflow-state-machine-node is-${node.visualState} ${node.isCurrent ? 'is-current' : ''} ${node.isLocked ? 'is-locked' : ''}`}
                disabled={!node.canEnterMode}
                tabIndex={node.isCurrent ? 0 : -1}
                aria-keyshortcuts="Left Right"
                title={`${node.label} · ${node.statusTag} · ${node.tooltip}`}
                aria-label={`${node.label}步骤：${node.statusTag}，${node.isCurrent ? '当前步骤' : node.tooltip}`}
                aria-current={node.isCurrent ? 'step' : undefined}
                onClick={() => {
                  if (node.canEnterMode && !node.isCurrent) {
                    onModeSelect(node.id)
                  }
                }}
                onKeyDown={(event) => {
                  if (event.key === 'ArrowLeft') {
                    event.preventDefault()
                    const previous = findAccessibleModeByOffset(node.id, -1)
                    if (previous) onModeSelect(previous)
                    return
                  }
                  if (event.key === 'ArrowRight') {
                    event.preventDefault()
                    const next = findAccessibleModeByOffset(node.id, 1)
                    if (next) onModeSelect(next)
                  }
                }}
              >
                <span className="cad-workflow-state-machine-step" aria-hidden="true">{node.step}</span>
                <span className="cad-workflow-state-machine-main">{node.label}</span>
                <span className="cad-workflow-state-machine-tag">{node.statusTag}</span>
              </button>
            </span>
          ))}
        </span>
      </span>
      <div className="cad-workflow-mode-mobile-summary" role="status" aria-live="polite">
        <span>{`${WORKFLOW_MODE_LABELS[activeMode]} · ${activeModeStatusLabel}（进度 ${completedModeCount}/4）`}</span>
        <span>{activeModeStatusHint}</span>
        <span>{`下一步：${nextActionHint} · Alt+1~4 / ←→`}</span>
      </div>
      <details className="cad-global-actions-more" aria-label="更多工具" open={canCheck}>
        <summary className="text-action cad-global-actions-more-trigger">
          <DotsThreeVertical size={14} />
          更多
        </summary>
        <div className="cad-global-actions-more-body">
          <button
            type="button"
            className={`text-action ${importBusy ? 'is-busy' : ''}`}
            onClick={onImport}
            disabled={!actions.canImport || importBusy}
            title={actions.importLabel}
          >
            {importBusy ? (
              <SpinnerGap size={16} className="cad-global-actions-spin" />
            ) : (
              <FolderOpen size={16} />
            )}
            {importBusy ? '正在导入参考模型' : actions.importLabel}
          </button>
          {canCheck ? (
            <button
              type="button"
              className="text-action"
              onClick={onCheck}
              aria-label="质量检查"
              title="检查当前资产质量与可用性"
            ><Check size={16} /> 质量检查</button>
          ) : null}
          {showAdvancedActions ? (
            <>
            <button
              type="button"
              className="text-action"
              onClick={onUndo}
              disabled={!actions.canUndo}
                title="回到上一个模型状态（Cmd/Ctrl+Z）"
              ><ClockCounterClockwise size={16} /> 上一步</button>
              <button
                type="button"
                className="text-action"
                onClick={onRedo}
                disabled={!actions.canRedo}
                title="恢复下一个模型状态（Cmd/Ctrl+Shift+Z）"
              ><ArrowsClockwise size={16} /> 恢复</button>
            </>
          ) : null}
          {onOpenAdvanced ? (
            <button
              type="button"
              className="text-action"
              onClick={onOpenAdvanced}
            ><Sparkle size={15} /> 修改模式</button>
          ) : null}
        </div>
      </details>
    </div>
  )
}
