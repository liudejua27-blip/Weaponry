import { Suspense, lazy } from 'react'
import { PatchModePanel } from '../features/canvas/PatchModePanel'
import { CreateWeaponPanel } from '../features/create/CreateWeaponPanel'
import { JobCenterPanel } from '../features/jobs/JobCenterPanel'
import { JobTimeline } from '../features/jobs/JobTimeline'
import { LibraryPanel } from '../features/library/LibraryPanel'
import { ProviderPanel } from '../features/settings/ProviderPanel'
import { forgeApi } from '../shared/api/forgeApi'
import type { WeaponDetail } from '../shared/types'
import { AppShell } from './AppShell'
import type { VersionAsset } from './assetSelectors'
import type { WeaponVersion } from './providers/SelectionProvider'
import type { LegacyAppController } from './useLegacyAppController'

const Preview3DPanel = lazy(() => import('../features/preview3d/Preview3DPanel').then(
  (module) => ({ default: module.Preview3DPanel }),
))

export function LegacyWorkbench({ controller }: { controller: LegacyAppController }) {
  const {
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
  } = controller

  return (
    <AppShell
      view={view}
      title={activeWeaponDetail?.display_name ?? '第一阶段'}
      subtitle={activeWeaponDetail
        ? `${activeWeaponDetail.weapon_family} · ${activeWeaponDetail.stage} · ${activeVersion?.version_id ?? 'no version'}`
        : '1 个方案 -> 概念图 -> 局部修改 -> 资产库 -> 3D 粗模'}
      serviceStatus={serviceStatus}
      serviceLabel={serviceStatus === 'connected'
        ? (activeJobId
          ? `${activeJobDetail?.status ?? 'tracking'} · ${activeJobDetail?.current_step ?? activeJobId}`
          : 'Agent 空闲')
        : '本地 Agent 未连接'}
      onNavigate={navigateToView}
    >
      <section className={view === 'jobs' ? 'workbench jobs-workbench' : 'workbench'}>
        <div className="left-panel">
          {view === 'forge' ? (
            <CreateWeaponPanel
              api={api}
              onJobCreated={handleJobAccepted}
              onEventsReset={resetEvents}
            />
          ) : null}
          {view === 'patch' ? (
            <section className="panel-section">
              <h1>局部修改</h1>
              <p className="muted">选择已入库概念图，生成 mask 和 PatchManifest，再提交一个追加式新版本。</p>
              <div className="source-summary">
                <strong>边界</strong>
                <span>只做虚构 Unity 游戏美术资产；外观可高拟真，但不输出制造图纸、尺寸、材料配方或工艺步骤。</span>
              </div>
            </section>
          ) : null}
          {view === 'library' ? (
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
          ) : null}
          {view === 'settings' ? (
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
          ) : null}
        </div>

        <div className="main-stage">
          <div className="stage-header">
            <span>{view === 'patch' ? 'Patch 画布' : view === 'jobs' ? '任务中心' : '主预览'}</span>
            <span className="muted">
              {view === 'jobs' ? '历史任务 / 恢复 / Action 审计' : '概念图 / Patch 画布 / 3D 预览'}
            </span>
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

        {view !== 'jobs' ? (
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
        ) : null}
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
    return (
      <p className="muted">
        当前没有选中的武器资产。创建或从资产库选择一个武器后，Inspector 会显示版本、Spec、材质和 Unity 元数据。
      </p>
    )
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

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / 1024 / 1024).toFixed(1)} MB`
}
