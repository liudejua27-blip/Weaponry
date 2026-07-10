import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type {
  CreateWeaponResponse,
  JobDetail,
  JobRuntimeStateResponse,
  WeaponDetail,
} from '../shared/types'
import {
  findAssetByRole,
  findConceptOrPatchAsset,
  findLatestAssetByRole,
  findLatestConceptOrPatchAsset,
} from './assetSelectors'
import {
  isTerminalJobStatus,
  readDesktopNotifications,
  readLastJobId,
  readRecentJobIds,
  rememberDesktopNotification,
  rememberLastJobId,
  rememberRecentJob,
} from './jobPersistence'
import { useJobEvents } from './providers/JobEventProvider'
import { useRuntime } from './providers/RuntimeProvider'
import { useSelection } from './providers/SelectionProvider'
import { routeHasResource, routeKey, writeHashRoute } from './routing'
import { useAppRouting } from './useAppRouting'

export function useLegacyAppController() {
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
  const { view, setView, pendingRoute, navigateToView } = useAppRouting()
  const [activeJobId, setActiveJobId] = useState<string | null>(() => (
    pendingRoute.kind === 'job' ? pendingRoute.jobId : null
  ))
  const [activeJobDetail, setActiveJobDetail] = useState<JobDetail | null>(null)
  const [activeJobRuntime, setActiveJobRuntime] = useState<JobRuntimeStateResponse | null>(null)
  const [jobActionStatus, setJobActionStatus] = useState<string | null>(null)
  const [recentJobIds, setRecentJobIds] = useState<string[]>(readRecentJobIds)
  const [desktopNotifications, setDesktopNotifications] = useState(readDesktopNotifications)
  const appliedRouteRef = useRef('')

  const sourceImage = useMemo(
    () => findConceptOrPatchAsset(activeVersion)
      ?? findLatestConceptOrPatchAsset(activeWeaponDetail),
    [activeVersion, activeWeaponDetail],
  )
  const roughGlb = useMemo(
    () => findAssetByRole(activeVersion, 'rough_raw_glb')
      ?? findLatestAssetByRole(activeWeaponDetail, 'rough_raw_glb'),
    [activeVersion, activeWeaponDetail],
  )
  const unityMaterial = useMemo(
    () => findAssetByRole(activeVersion, 'unity_material_json')
      ?? findLatestAssetByRole(activeWeaponDetail, 'unity_material_json'),
    [activeVersion, activeWeaponDetail],
  )
  const unityExport = useMemo(
    () => findLatestAssetByRole(activeWeaponDetail, 'unity_export_package'),
    [activeWeaponDetail],
  )

  const handleWeaponSelected = useCallback((weaponId: string, versionId?: string) => {
    setActiveWeaponId(weaponId)
    setActiveVersionId(versionId ?? '')
  }, [setActiveVersionId, setActiveWeaponId])

  const handleVersionSelected = useCallback((versionId: string) => {
    setActiveVersionId(versionId)
  }, [setActiveVersionId])

  const handleLibraryWeaponSelected = useCallback((weaponId: string, versionId?: string) => {
    handleWeaponSelected(weaponId, versionId)
    writeHashRoute(versionId
      ? { kind: 'weapon', weaponId, versionId }
      : { kind: 'weapon', weaponId })
  }, [handleWeaponSelected])

  const handleLibraryVersionSelected = useCallback((versionId: string) => {
    handleVersionSelected(versionId)
    const weaponId = activeWeaponId || activeWeaponDetail?.weapon_id
    if (weaponId) writeHashRoute({ kind: 'weapon', weaponId, versionId })
  }, [activeWeaponDetail?.weapon_id, activeWeaponId, handleVersionSelected])

  const selectJobOutputVersion = useCallback((job: JobDetail, detail: WeaponDetail) => {
    const outputVersionId = jobOutputVersionId(job)
    if (outputVersionId && (detail.versions ?? []).some(
      (version) => version.version_id === outputVersionId,
    )) {
      setActiveVersionId(outputVersionId)
    }
  }, [setActiveVersionId])

  const rememberJob = useCallback((jobId: string) => {
    setRecentJobIds(rememberRecentJob(jobId))
  }, [])

  const publishJobNotification = useCallback((job: JobDetail) => {
    if (!isTerminalJobStatus(job.status)) return
    setDesktopNotifications(rememberDesktopNotification(job))
  }, [])

  const subscribeJobEvents = useCallback((id: string) => (
    subscribeToJobEvents(id, {
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
  ), [api, handleWeaponDetailLoaded, publishJobNotification, selectJobOutputVersion, subscribeToJobEvents])

  const refreshJobRuntime = useCallback((jobId: string) => (
    api.getJobRuntime(jobId)
      .then((runtime) => {
        setActiveJobRuntime(runtime)
        return runtime
      })
      .catch(() => {
        setActiveJobRuntime(null)
        return null
      })
  ), [api])

  const restoreJob = useCallback((jobId: string) => {
    setJobActionStatus('正在恢复任务...')
    return api.getJob(jobId)
      .then((detail) => {
        setActiveJobDetail(detail)
        setActiveJobId(detail.job_id)
        setActiveWeaponId(detail.weapon_id)
        replaceEvents((detail.events ?? []).filter((event) => event.job_id === detail.job_id))
        refreshJobRuntime(detail.job_id).catch(() => undefined)
        rememberLastJobId(detail.job_id)
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
  }, [
    api,
    clearWeaponDetail,
    handleWeaponDetailLoaded,
    publishJobNotification,
    refreshJobRuntime,
    rememberJob,
    replaceEvents,
    selectJobOutputVersion,
    setActiveWeaponId,
  ])

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
      setJobActionStatus(
        `正在从链接打开资产版本 ${pendingRoute.versionId ?? pendingRoute.weaponId}...`,
      )
      api.getWeapon(pendingRoute.weaponId)
        .then((detail) => {
          handleWeaponDetailLoaded(detail)
          if (pendingRoute.versionId) {
            const hasVersion = (detail.versions ?? []).some(
              (version) => version.version_id === pendingRoute.versionId,
            )
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
        .catch((caught) => setJobActionStatus(
          caught instanceof Error ? caught.message : String(caught),
        ))
    }
  }, [
    api,
    handleWeaponDetailLoaded,
    pendingRoute,
    restoreJob,
    serviceStatus,
    setActiveVersionId,
    setActiveWeaponId,
    setView,
  ])

  useEffect(() => {
    if (serviceStatus !== 'connected' || activeJobId || routeHasResource(pendingRoute)) return
    const recentJobId = readLastJobId()
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
    rememberLastJobId(response.job_id)
    rememberJob(response.job_id)
    restoreJob(response.job_id).catch(() => {
      api.getWeapon(response.weapon_id)
        .then(handleWeaponDetailLoaded)
        .catch(clearWeaponDetail)
    })
  }, [
    api,
    clearWeaponDetail,
    handleWeaponDetailLoaded,
    rememberJob,
    resetEvents,
    restoreJob,
    setActiveWeaponId,
  ])

  const retryJob = useCallback((jobId: string) => {
    setJobActionStatus('正在提交任务重试请求...')
    api.retryJob(jobId)
      .then((response) => {
        setJobActionStatus(`${response.message} · ${response.previous_status} -> ${response.status}`)
        refreshJobRuntime(jobId).catch(() => undefined)
        restoreJob(jobId).catch(() => undefined)
      })
      .catch((caught) => setJobActionStatus(
        caught instanceof Error ? caught.message : String(caught),
      ))
  }, [api, refreshJobRuntime, restoreJob])

  const retryJobFromStep = useCallback((jobId: string, stepName: string) => {
    setJobActionStatus(`正在请求从 ${stepName} 重试...`)
    api.retryJobFromStep(jobId, stepName)
      .then((response) => {
        setJobActionStatus(`${response.message} · retry_from=${response.retry_from ?? stepName}`)
        refreshJobRuntime(jobId).catch(() => undefined)
        restoreJob(jobId).catch(() => undefined)
      })
      .catch((caught) => setJobActionStatus(
        caught instanceof Error ? caught.message : String(caught),
      ))
  }, [api, refreshJobRuntime, restoreJob])

  const cancelJob = useCallback((jobId: string) => {
    setJobActionStatus('正在提交取消请求...')
    api.cancelJob(jobId)
      .then((response) => {
        setJobActionStatus(`${response.message} · ${response.previous_status} -> ${response.status}`)
        refreshJobRuntime(jobId).catch(() => undefined)
        restoreJob(jobId).catch(() => undefined)
      })
      .catch((caught) => setJobActionStatus(
        caught instanceof Error ? caught.message : String(caught),
      ))
  }, [api, refreshJobRuntime, restoreJob])

  const skip3D = useCallback(() => {
    navigateToView('library')
    setJobActionStatus('已跳到资产库，继续使用当前概念图或 patch 资产；3D 可稍后重新生成。')
  }, [navigateToView])

  const openJobTraceFromLibrary = useCallback((jobId: string) => {
    writeHashRoute({ kind: 'job', jobId })
    setView('jobs')
    setJobActionStatus('正在从资产库打开生成轨迹...')
    restoreJob(jobId).catch(() => undefined)
  }, [restoreJob, setView])

  const restoreJobFromJobCenter = useCallback((jobId: string) => {
    writeHashRoute({ kind: 'job', jobId })
    return restoreJob(jobId)
  }, [restoreJob])

  return {
    api,
    serviceStatus,
    agentSupervisor,
    agentActionError,
    checkService,
    startLocalAgent,
    stopLocalAgent,
    restartLocalAgent,
    events,
    streamStatus,
    resetEvents,
    view,
    navigateToView,
    activeWeaponId,
    activeVersionId,
    activeWeaponDetail,
    activeVersion,
    activeJobId,
    activeJobDetail,
    activeJobRuntime,
    jobActionStatus,
    recentJobIds,
    desktopNotifications,
    sourceImage,
    roughGlb,
    unityMaterial,
    unityExport,
    handleWeaponDetailLoaded,
    handleWeaponSelected,
    handleVersionSelected,
    handleLibraryWeaponSelected,
    handleLibraryVersionSelected,
    subscribeJobEvents,
    handleJobAccepted,
    retryJob,
    retryJobFromStep,
    cancelJob,
    skip3D,
    openJobTraceFromLibrary,
    restoreJobFromJobCenter,
  }
}

export type LegacyAppController = ReturnType<typeof useLegacyAppController>

function jobOutputVersionId(job: JobDetail): string {
  const versionId = job.outputs?.current_version_id
  return typeof versionId === 'string' ? versionId : ''
}
