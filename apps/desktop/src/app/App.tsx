import { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { CreateWeaponPanel } from '../features/create/CreateWeaponPanel'
import { PatchModePanel } from '../features/canvas/PatchModePanel'
import { JobCenterPanel } from '../features/jobs/JobCenterPanel'
import { JobTimeline } from '../features/jobs/JobTimeline'
import { LibraryPanel } from '../features/library/LibraryPanel'
import { ProviderPanel } from '../features/settings/ProviderPanel'
import { forgeApi } from '../shared/api/forgeApi'
import { AppShell } from './AppShell'
import { useJobEvents } from './providers/JobEventProvider'
import { useRuntime } from './providers/RuntimeProvider'
import { useSelection, type WeaponVersion } from './providers/SelectionProvider'
import {
  parseHashRoute,
  routeHasResource,
  routeKey,
  routeView,
  writeHashRoute,
  type AppRoute,
  type View,
} from './routing'
import type { CreateWeaponResponse, JobDetail, JobRuntimeStateResponse, WeaponDetail } from '../shared/types'

type VersionAsset = NonNullable<WeaponVersion['assets']>[number]
type DesktopJobNotification = {
  id: string
  jobId: string
  jobType: string
  status: string
  weaponId: string
  message: string
  createdAt: string
}

const Preview3DPanel = lazy(() => import('../features/preview3d/Preview3DPanel').then((module) => ({ default: module.Preview3DPanel })))
const CadWorkbenchPanel = lazy(() => import('../features/cad-workbench/CadWorkbenchPanel').then((module) => ({ default: module.CadWorkbenchPanel })))

export function App() {
  const {
    api,
    serviceStatus,
    agentSupervisor,
    agentActionError,
    checkService,
    startLocalAgent,
    stopLocalAgent,
    restartLocalAgent,
  } = useRuntime()
  const {
    events,
    streamStatus,
    replaceEvents,
    resetEvents,
    subscribe: subscribeToJobEvents,
  } = useJobEvents()
  const {
    activeWeaponId,
    activeVersionId,
    activeWeaponDetail,
    activeVersion,
    setActiveWeaponId,
    setActiveVersionId,
    loadWeaponDetail: handleWeaponDetailLoaded,
    clearWeaponDetail,
  } = useSelection()
  const initialRoute = useMemo(() => parseHashRoute(), [])
  const [view, setView] = useState<View>(() => routeView(initialRoute) ?? 'forge')
  const [pendingRoute, setPendingRoute] = useState<AppRoute>(initialRoute)
  const [activeJobId, setActiveJobId] = useState<string | null>(() => initialRoute.kind === 'job' ? initialRoute.jobId : null)
  const [activeJobDetail, setActiveJobDetail] = useState<JobDetail | null>(null)
  const [activeJobRuntime, setActiveJobRuntime] = useState<JobRuntimeStateResponse | null>(null)
  const [jobActionStatus, setJobActionStatus] = useState<string | null>(null)
  const [recentJobIds, setRecentJobIds] = useState<string[]>(() => readRecentJobIds())
  const [desktopNotifications, setDesktopNotifications] = useState<DesktopJobNotification[]>(() => readDesktopNotifications())
  const appliedRouteRef = useRef('')
  const sourceImage = useMemo(
    () => findConceptOrPatchAsset(activeVersion) ?? findLatestConceptOrPatchAsset(activeWeaponDetail),
    [activeVersion, activeWeaponDetail]
  )
  const roughGlb = useMemo(() => findAssetByRole(activeVersion, 'rough_raw_glb') ?? findLatestAssetByRole(activeWeaponDetail, 'rough_raw_glb'), [activeVersion, activeWeaponDetail])
  const unityMaterial = useMemo(() => findAssetByRole(activeVersion, 'unity_material_json') ?? findLatestAssetByRole(activeWeaponDetail, 'unity_material_json'), [activeVersion, activeWeaponDetail])
  const unityExport = useMemo(() => findLatestAssetByRole(activeWeaponDetail, 'unity_export_package'), [activeWeaponDetail])

  useEffect(() => {
    const syncRoute = () => {
      const route = parseHashRoute()
      setPendingRoute(route)
      const nextView = routeView(route)
      if (nextView) setView(nextView)
    }
    window.addEventListener('hashchange', syncRoute)
    return () => window.removeEventListener('hashchange', syncRoute)
  }, [])

  const handleWeaponSelected = useCallback((weaponId: string, versionId?: string) => {
    setActiveWeaponId(weaponId)
    setActiveVersionId(versionId ?? '')
  }, [])

  const handleVersionSelected = useCallback((versionId: string) => {
    setActiveVersionId(versionId)
  }, [])

  const navigateToView = useCallback((nextView: View) => {
    writeHashRoute({ kind: 'view', view: nextView })
    setView(nextView)
  }, [])

  const handleLibraryWeaponSelected = useCallback((weaponId: string, versionId?: string) => {
    handleWeaponSelected(weaponId, versionId)
    writeHashRoute(versionId ? { kind: 'weapon', weaponId, versionId } : { kind: 'weapon', weaponId })
  }, [handleWeaponSelected])

  const handleLibraryVersionSelected = useCallback((versionId: string) => {
    handleVersionSelected(versionId)
    const weaponId = activeWeaponId || activeWeaponDetail?.weapon_id
    if (weaponId) writeHashRoute({ kind: 'weapon', weaponId, versionId })
  }, [activeWeaponDetail?.weapon_id, activeWeaponId, handleVersionSelected])

  const selectJobOutputVersion = useCallback((job: JobDetail, detail: WeaponDetail) => {
    const outputVersionId = jobOutputVersionId(job)
    if (outputVersionId && (detail.versions ?? []).some((version) => version.version_id === outputVersionId)) {
      setActiveVersionId(outputVersionId)
    }
  }, [])

  const rememberJob = useCallback((jobId: string) => {
    setRecentJobIds(rememberRecentJob(jobId))
  }, [])

  const publishJobNotification = useCallback((job: JobDetail) => {
    if (!isTerminalJobStatus(job.status)) return
    setDesktopNotifications(rememberDesktopNotification(job))
  }, [])

  const subscribeJobEvents = useCallback(
    (id: string) => {
      return subscribeToJobEvents(id, {
        onTerminal: (event) => {
          api.getJobRuntime(id).then(setActiveJobRuntime).catch(() => undefined)
          api.getJob(id)
            .then((jobDetail) => {
              setActiveJobDetail(jobDetail)
              publishJobNotification(jobDetail)
              if (event.weapon_id) {
                api.getWeapon(event.weapon_id)
                  .then((weaponDetail) => {
                    handleWeaponDetailLoaded(weaponDetail)
                    selectJobOutputVersion(jobDetail, weaponDetail)
                  })
                  .catch(() => undefined)
              }
            })
            .catch(() => undefined)
        },
        onStreamError: (error) => setJobActionStatus(`${error.code}: ${error.message}`),
      })
    },
    [api, handleWeaponDetailLoaded, publishJobNotification, selectJobOutputVersion, subscribeToJobEvents]
  )
  const refreshJobRuntime = useCallback((jobId: string) => {
    return api.getJobRuntime(jobId)
      .then((runtime) => {
        setActiveJobRuntime(runtime)
        return runtime
      })
      .catch(() => {
        setActiveJobRuntime(null)
        return null
      })
  }, [api])

  const restoreJob = useCallback((jobId: string) => {
    setJobActionStatus('正在恢复任务...')
    return api.getJob(jobId)
      .then((detail) => {
        setActiveJobDetail(detail)
        setActiveJobId(detail.job_id)
        setActiveWeaponId(detail.weapon_id)
        replaceEvents((detail.events ?? []).filter((event) => event.job_id === detail.job_id))
        refreshJobRuntime(detail.job_id).catch(() => undefined)
        localStorage.setItem('wushen.recentJobId', detail.job_id)
        rememberJob(detail.job_id)
        publishJobNotification(detail)
        return api.getWeapon(detail.weapon_id)
          .then((weaponDetail) => {
            handleWeaponDetailLoaded(weaponDetail)
            selectJobOutputVersion(detail, weaponDetail)
            setJobActionStatus(`已恢复 ${detail.job_id}`)
            return detail
          })
          .catch(() => {
            clearWeaponDetail()
            setJobActionStatus(`已恢复 ${detail.job_id}，但武器详情暂不可用`)
            return detail
          })
      })
      .catch((caught) => {
        setJobActionStatus(caught instanceof Error ? caught.message : String(caught))
        throw caught
      })
  }, [api, clearWeaponDetail, handleWeaponDetailLoaded, publishJobNotification, refreshJobRuntime, rememberJob, replaceEvents, selectJobOutputVersion])

  useEffect(() => {
    if (serviceStatus !== 'connected') return
    const key = routeKey(pendingRoute)
    if (!key || appliedRouteRef.current === key) return
    appliedRouteRef.current = key

    if (pendingRoute.kind === 'job') {
      setView('jobs')
      setJobActionStatus(`正在从链接恢复 ${pendingRoute.jobId}...`)
      restoreJob(pendingRoute.jobId).catch(() => undefined)
      return
    }

    if (pendingRoute.kind === 'weapon') {
      setView('library')
      setActiveWeaponId(pendingRoute.weaponId)
      setActiveVersionId(pendingRoute.versionId ?? '')
      setJobActionStatus(`正在从链接打开资产版本 ${pendingRoute.versionId ?? pendingRoute.weaponId}...`)
      api.getWeapon(pendingRoute.weaponId)
        .then((detail) => {
          handleWeaponDetailLoaded(detail)
          if (pendingRoute.versionId) {
            const hasVersion = (detail.versions ?? []).some((version) => version.version_id === pendingRoute.versionId)
            if (hasVersion) {
              setActiveVersionId(pendingRoute.versionId)
              setJobActionStatus(`已打开链接版本 ${pendingRoute.versionId}`)
            } else {
              setJobActionStatus(`链接版本不存在：${pendingRoute.versionId}`)
            }
          } else {
            setJobActionStatus(`已打开链接武器 ${pendingRoute.weaponId}`)
          }
        })
        .catch((caught) => setJobActionStatus(caught instanceof Error ? caught.message : String(caught)))
    }
  }, [api, handleWeaponDetailLoaded, pendingRoute, restoreJob, serviceStatus])

  useEffect(() => {
    if (serviceStatus !== 'connected' || activeJobId || routeHasResource(pendingRoute)) return
    const recentJobId = localStorage.getItem('wushen.recentJobId')
    if (recentJobId) restoreJob(recentJobId).catch(() => undefined)
  }, [activeJobId, pendingRoute, restoreJob, serviceStatus])

  useEffect(() => {
    if (serviceStatus !== 'connected' || !activeJobId) {
      setActiveJobRuntime(null)
      return
    }
    let cancelled = false
    const refresh = () => {
      api.getJobRuntime(activeJobId)
        .then((runtime) => {
          if (!cancelled) setActiveJobRuntime(runtime)
        })
        .catch(() => {
          if (!cancelled) setActiveJobRuntime(null)
        })
    }
    refresh()
    const timer = window.setInterval(refresh, 2500)
    return () => {
      cancelled = true
      window.clearInterval(timer)
    }
  }, [activeJobId, api, serviceStatus])

  const handleJobAccepted = useCallback((response: CreateWeaponResponse) => {
    setActiveJobId(response.job_id)
    setActiveWeaponId(response.weapon_id)
    setActiveJobDetail(null)
    setActiveJobRuntime(null)
    resetEvents()
    localStorage.setItem('wushen.recentJobId', response.job_id)
    rememberJob(response.job_id)
    restoreJob(response.job_id).catch(() => {
      api.getWeapon(response.weapon_id)
        .then(handleWeaponDetailLoaded)
        .catch(() => {
          clearWeaponDetail()
        })
    })
  }, [api, clearWeaponDetail, handleWeaponDetailLoaded, rememberJob, resetEvents, restoreJob])

  const retryJob = useCallback((jobId: string) => {
    setJobActionStatus('正在提交任务重试请求...')
    api.retryJob(jobId)
      .then((response) => {
        setJobActionStatus(`${response.message} · ${response.previous_status} -> ${response.status}`)
        refreshJobRuntime(jobId).catch(() => undefined)
        restoreJob(jobId).catch(() => undefined)
      })
      .catch((caught) => setJobActionStatus(caught instanceof Error ? caught.message : String(caught)))
  }, [api, restoreJob])

  const retryJobFromStep = useCallback((jobId: string, stepName: string) => {
    setJobActionStatus(`正在请求从 ${stepName} 重试...`)
    api.retryJobFromStep(jobId, stepName)
      .then((response) => {
        setJobActionStatus(`${response.message} · retry_from=${response.retry_from ?? stepName}`)
        refreshJobRuntime(jobId).catch(() => undefined)
        restoreJob(jobId).catch(() => undefined)
      })
      .catch((caught) => setJobActionStatus(caught instanceof Error ? caught.message : String(caught)))
  }, [api, restoreJob])

  const cancelJob = useCallback((jobId: string) => {
    setJobActionStatus('正在提交取消请求...')
    api.cancelJob(jobId)
      .then((response) => {
        setJobActionStatus(`${response.message} · ${response.previous_status} -> ${response.status}`)
        refreshJobRuntime(jobId).catch(() => undefined)
        restoreJob(jobId).catch(() => undefined)
      })
      .catch((caught) => setJobActionStatus(caught instanceof Error ? caught.message : String(caught)))
  }, [api, restoreJob])

  const skip3D = useCallback(() => {
    navigateToView('library')
    setJobActionStatus('已跳到资产库，继续使用当前概念图或 patch 资产；3D 可稍后重新生成。')
  }, [navigateToView])

  const openJobTraceFromLibrary = useCallback((jobId: string) => {
    writeHashRoute({ kind: 'job', jobId })
    setView('jobs')
    setJobActionStatus('正在从资产库打开生成轨迹...')
    restoreJob(jobId).catch(() => undefined)
  }, [restoreJob])

  const restoreJobFromJobCenter = useCallback((jobId: string) => {
    writeHashRoute({ kind: 'job', jobId })
    return restoreJob(jobId)
  }, [restoreJob])

  if (view === 'cad') {
    return (
      <Suspense fallback={<div className="panel-section"><p className="muted">正在加载 CAD 工作台...</p></div>}>
        <CadWorkbenchPanel onOpenLegacy={() => navigateToView('forge')} />
      </Suspense>
    )
  }

  return (
    <AppShell
      view={view}
      title={activeWeaponDetail?.display_name ?? '第一阶段'}
      subtitle={activeWeaponDetail
        ? `${activeWeaponDetail.weapon_family} · ${activeWeaponDetail.stage} · ${activeVersion?.version_id ?? 'no version'}`
        : '1 个方案 -> 概念图 -> 局部修改 -> 资产库 -> 3D 粗模'}
      serviceStatus={serviceStatus}
      serviceLabel={serviceStatus === 'connected'
        ? (activeJobId ? `${activeJobDetail?.status ?? 'tracking'} · ${activeJobDetail?.current_step ?? activeJobId}` : 'Agent 空闲')
        : '本地 Agent 未连接'}
      onNavigate={navigateToView}
    >
        <section className={view === 'jobs' ? 'workbench jobs-workbench' : 'workbench'}>
          <div className="left-panel">
            {view === 'forge' && <CreateWeaponPanel api={api} onJobCreated={handleJobAccepted} onEventsReset={resetEvents} />}
            {view === 'patch' && (
              <section className="panel-section">
                <h1>局部修改</h1>
                <p className="muted">选择已入库概念图，生成 mask 和 PatchManifest，再提交一个追加式新版本。</p>
                <div className="source-summary">
                  <strong>边界</strong>
                  <span>只做虚构 Unity 游戏美术资产；外观可高拟真，但不输出制造图纸、尺寸、材料配方或工艺步骤。</span>
                </div>
              </section>
            )}
            {view === 'library' && (
              <LibraryPanel
                api={api}
                activeWeaponId={activeWeaponId}
                activeVersionId={activeVersionId}
                onOpenSettings={() => navigateToView('settings')}
                onWeaponSelected={handleLibraryWeaponSelected}
                onVersionSelected={handleLibraryVersionSelected}
                onWeaponDetailLoaded={handleWeaponDetailLoaded}
                onOpenJobTrace={openJobTraceFromLibrary}
              />
            )}
            {view === 'settings' && (
              <ProviderPanel
                api={api}
                serviceStatus={serviceStatus}
                agentSupervisor={agentSupervisor}
                agentActionError={agentActionError}
                onRetryHealth={checkService}
                onStartAgent={startLocalAgent}
                onStopAgent={stopLocalAgent}
                onRestartAgent={restartLocalAgent}
              />
            )}
          </div>

          <div className="main-stage">
            <div className="stage-header">
              <span>{view === 'patch' ? 'Patch 画布' : view === 'jobs' ? '任务中心' : '主预览'}</span>
              <span className="muted">{view === 'jobs' ? '历史任务 / 恢复 / Action 审计' : '概念图 / Patch 画布 / 3D 预览'}</span>
            </div>
            {view === 'patch' ? (
              <PatchModePanel
                api={api}
                activeWeaponId={activeWeaponId}
                activeVersionId={activeVersionId}
                onWeaponSelected={handleWeaponSelected}
                onVersionSelected={handleVersionSelected}
                onWeaponDetailLoaded={handleWeaponDetailLoaded}
                onJobCreated={handleJobAccepted}
                onEventsReset={resetEvents}
              />
            ) : view === 'jobs' ? (
              <JobCenterPanel
                api={api}
                activeJobId={activeJobId}
                actionStatus={jobActionStatus}
                onRestoreJob={restoreJobFromJobCenter}
                recentJobIds={recentJobIds}
                desktopNotifications={desktopNotifications}
                onRetryJob={retryJob}
                onRetryFromStep={retryJobFromStep}
                onCancelJob={cancelJob}
                onOpenSettings={() => navigateToView('settings')}
                onSkip3D={skip3D}
              />
            ) : (
              <CurrentAssetStage
                api={api}
                detail={activeWeaponDetail}
                version={activeVersion}
                sourceImage={sourceImage}
                roughGlb={roughGlb}
                unityExport={unityExport}
              />
            )}
          </div>

          {view !== 'jobs' && (
            <div className="inspector">
              <h2>Inspector</h2>
              <AssetInspectorSummary
                detail={activeWeaponDetail}
                version={activeVersion}
                sourceImage={sourceImage}
                roughGlb={roughGlb}
                unityMaterial={unityMaterial}
                unityExport={unityExport}
              />
              <Suspense fallback={<div className="preview-card"><p className="muted">正在加载 3D 预览...</p></div>}>
                <Preview3DPanel
                  api={api}
                  refreshKey={`${activeJobId ?? 'no-job'}:${activeJobDetail?.status ?? 'no-status'}:${activeWeaponDetail?.current_version_id ?? 'no-version'}`}
                  activeWeaponId={activeWeaponId}
                  activeVersionId={activeVersionId}
                  onWeaponSelected={handleWeaponSelected}
                  onVersionSelected={handleVersionSelected}
                  onWeaponDetailLoaded={handleWeaponDetailLoaded}
                  onJobCreated={handleJobAccepted}
                  onEventsReset={resetEvents}
                />
              </Suspense>
            </div>
          )}
        </section>

        <JobTimeline
          events={events}
          jobId={activeJobId}
          streamStatus={streamStatus}
          runtime={activeJobRuntime}
          onSubscribe={subscribeJobEvents}
          actionStatus={jobActionStatus}
          onRetryJob={retryJob}
          onRetryFromStep={retryJobFromStep}
          onCancelJob={cancelJob}
          onOpenSettings={() => navigateToView('settings')}
          onSkip3D={skip3D}
        />
    </AppShell>
  )
}

function CurrentAssetStage({
  api,
  detail,
  version,
  sourceImage,
  roughGlb,
  unityExport,
}: {
  api: typeof forgeApi
  detail: WeaponDetail | null
  version: WeaponVersion | null
  sourceImage: VersionAsset | null
  roughGlb: VersionAsset | null
  unityExport: VersionAsset | null
}) {
  if (!detail) {
    return (
      <div className="empty-stage">
        <div className="weapon-silhouette" />
        <p>创建或选择一个武器后，这里会显示当前概念图、版本和 3D 资产状态。</p>
      </div>
    )
  }

  return (
    <section className="asset-stage">
      <div className="asset-stage-header">
        <div>
          <h1>{detail.display_name}</h1>
          <p>{detail.weapon_family} · {detail.stage} · v{version?.version_no ?? '?'}</p>
        </div>
        <span className="status-pill connected">{version?.status ?? 'no version'}</span>
      </div>

      <div className="asset-stage-grid">
        <div className="asset-preview-frame">
          {sourceImage ? (
            <img src={api.getAssetFileUrl(sourceImage.asset_id)} alt={`${detail.display_name} 当前源图`} />
          ) : (
            <div className="empty-stage inline">
              <div className="weapon-silhouette" />
              <p>当前版本暂无概念图或 patch 图。</p>
            </div>
          )}
        </div>
        <div className="asset-stage-facts">
          <StatusRow label="当前版本" value={version ? `v${version.version_no} · ${version.version_type}` : 'none'} />
          <StatusRow label="源图" value={sourceImage ? `${sourceImage.role} · ${sourceImage.asset_id}` : '待生成概念图'} />
          <StatusRow label="3D 粗模" value={roughGlb ? `可预览 GLB · ${formatBytes(roughGlb.byte_size)}` : '待生成 3D'} />
          <StatusRow label="Unity 导出" value={unityExport ? `已导出 ZIP · ${formatBytes(unityExport.byte_size)}` : '未导出'} />
          <StatusRow label="安全边界" value="虚构 Unity 游戏美术资产 / 非制造说明" />
        </div>
      </div>
    </section>
  )
}

function AssetInspectorSummary({
  detail,
  version,
  sourceImage,
  roughGlb,
  unityMaterial,
  unityExport,
}: {
  detail: WeaponDetail | null
  version: WeaponVersion | null
  sourceImage: VersionAsset | null
  roughGlb: VersionAsset | null
  unityMaterial: VersionAsset | null
  unityExport: VersionAsset | null
}) {
  if (!detail) {
    return <p className="muted">当前没有选中的武器资产。创建或从资产库选择一个武器后，Inspector 会显示版本、Spec、材质和 Unity 元数据。</p>
  }

  const roles = [
    sourceImage?.role,
    roughGlb?.role,
    unityMaterial?.role,
    unityExport?.role,
  ].filter(Boolean)

  return (
    <div className="inspector-summary">
      <strong>{detail.display_name}</strong>
      <StatusRow label="weapon" value={detail.weapon_id} />
      <StatusRow label="version" value={version?.version_id ?? detail.current_version_id ?? 'none'} />
      <StatusRow label="model" value={detail.current_model_id ?? 'none'} />
      <StatusRow label="assets" value={roles.length ? roles.join(' / ') : 'no active assets'} />
      <small>只做虚构 Unity 游戏美术资产；外观可高拟真，但不输出制造图纸、尺寸、材料配方或工艺步骤。</small>
    </div>
  )
}

function StatusRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="status-row">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

function jobOutputVersionId(job: JobDetail): string {
  const versionId = job.outputs?.current_version_id
  return typeof versionId === 'string' ? versionId : ''
}

function readRecentJobIds(): string[] {
  try {
    const raw = localStorage.getItem('wushen.recentJobIds')
    const parsed = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string').slice(0, 20) : []
  } catch {
    return []
  }
}

function rememberRecentJob(jobId: string): string[] {
  try {
    const current = readRecentJobIds()
    const next = [jobId, ...current.filter((item) => item !== jobId)].slice(0, 20)
    localStorage.setItem('wushen.recentJobIds', JSON.stringify(next))
    return next
  } catch {
    const next = [jobId]
    localStorage.setItem('wushen.recentJobIds', JSON.stringify(next))
    return next
  }
}

function readDesktopNotifications(): DesktopJobNotification[] {
  try {
    const raw = localStorage.getItem('wushen.desktopNotifications')
    const parsed = raw ? JSON.parse(raw) : []
    if (!Array.isArray(parsed)) return []
    return parsed.filter((item): item is DesktopJobNotification => {
      return isRecord(item)
        && typeof item.id === 'string'
        && typeof item.jobId === 'string'
        && typeof item.jobType === 'string'
        && typeof item.status === 'string'
        && typeof item.weaponId === 'string'
        && typeof item.message === 'string'
        && typeof item.createdAt === 'string'
    }).slice(0, 20)
  } catch {
    return []
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function rememberDesktopNotification(job: JobDetail): DesktopJobNotification[] {
  const notification: DesktopJobNotification = {
    id: `${job.job_id}:${job.status}`,
    jobId: job.job_id,
    jobType: job.type,
    status: job.status,
    weaponId: job.weapon_id,
    message: job.status === 'succeeded'
      ? `${job.type} 已完成，可回到任务中心查看交接产物。`
      : `${job.type} ${job.status}，需要查看任务中心。`,
    createdAt: new Date().toISOString(),
  }
  const current = readDesktopNotifications()
  const next = [notification, ...current.filter((item) => item.id !== notification.id)].slice(0, 20)
  localStorage.setItem('wushen.desktopNotifications', JSON.stringify(next))
  maybeSendBrowserNotification(notification)
  return next
}

function maybeSendBrowserNotification(notification: DesktopJobNotification): void {
  if (typeof window === 'undefined' || !('Notification' in window)) return
  if (Notification.permission !== 'granted') return
  try {
    new Notification('武神 Forge 任务更新', {
      body: `${notification.jobId} · ${notification.message}`,
      tag: notification.id,
      silent: true,
    })
  } catch {
    // Browser notification failures should never break local task recovery.
  }
}

function findConceptOrPatchAsset(version: WeaponVersion | null): VersionAsset | null {
  const assets = [...(version?.assets ?? [])].reverse()
  return assets.find((asset) => asset.role === 'concept_patch')
    ?? assets.find((asset) => asset.role === 'concept_image')
    ?? null
}

function findAssetByRole(version: WeaponVersion | null, role: string): VersionAsset | null {
  return [...(version?.assets ?? [])].reverse().find((asset) => asset.role === role) ?? null
}

function findLatestAssetByRole(detail: WeaponDetail | null, role: string): VersionAsset | null {
  return [...(detail?.versions ?? [])]
    .reverse()
    .flatMap((version) => [...(version.assets ?? [])].reverse())
    .find((asset) => asset.role === role) ?? null
}

function findLatestConceptOrPatchAsset(detail: WeaponDetail | null): VersionAsset | null {
  return [...(detail?.versions ?? [])]
    .reverse()
    .map((version) => findConceptOrPatchAsset(version))
    .find((asset): asset is VersionAsset => Boolean(asset)) ?? null
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / 1024 / 1024).toFixed(1)} MB`
}

function isTerminalJobStatus(status: string) {
  return ['failed', 'cancelled', 'partial_succeeded', 'succeeded'].includes(status)
}
