import { useEffect, useState } from 'react'
import type { ForgeApiClient } from '../../shared/api/forgeApi'
import type { WeaponDetail, WeaponSummary } from '../../shared/types'

type WeaponVersion = NonNullable<WeaponDetail['versions']>[number]
type VersionAsset = NonNullable<NonNullable<WeaponVersion['assets']>[number]>
type CurrentModelQuality = {
  reportFileId: string | null
  status: string | null
  blockerCount: number
  warningCount: number
  triangleCount: number | null
  materialCount: number | null
  boundsValid: boolean | null
}
type AssetPreviewSummary = {
  title: string
  kind: 'json' | 'glb' | 'zip'
  rows: Array<{ label: string; value: string }>
  body: string
}
type ZipEntrySummary = {
  name: string
  compressedSize: number
  uncompressedSize: number
  compression: number
  localHeaderOffset: number
}

type Props = {
  api: ForgeApiClient
  onOpenSettings: () => void
  activeWeaponId?: string
  activeVersionId?: string
  onWeaponSelected: (weaponId: string, versionId?: string) => void
  onVersionSelected: (versionId: string) => void
  onWeaponDetailLoaded: (detail: WeaponDetail) => void
  onOpenJobTrace?: (jobId: string) => void
}

export function LibraryPanel({
  api,
  onOpenSettings,
  activeWeaponId = '',
  activeVersionId = '',
  onWeaponSelected,
  onVersionSelected,
  onWeaponDetailLoaded,
  onOpenJobTrace,
}: Props) {
  const [items, setItems] = useState<WeaponSummary[]>([])
  const [localSelectedWeaponId, setLocalSelectedWeaponId] = useState('')
  const [detail, setDetail] = useState<WeaponDetail | null>(null)
  const [status, setStatus] = useState<'loading' | 'ready' | 'empty' | 'error'>('loading')
  const [detailStatus, setDetailStatus] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle')
  const [error, setError] = useState<string | null>(null)
  const [detailError, setDetailError] = useState<string | null>(null)
  const selectedWeaponId = activeWeaponId || localSelectedWeaponId

  function load() {
    setStatus('loading')
    setError(null)
    api.listWeapons()
      .then((nextItems) => {
        setItems(nextItems)
        setLocalSelectedWeaponId((current) => current || activeWeaponId || nextItems[0]?.weapon_id || '')
        if (!activeWeaponId && nextItems[0]) onWeaponSelected(nextItems[0].weapon_id, nextItems[0].current_version_id ?? undefined)
        setStatus(nextItems.length ? 'ready' : 'empty')
      })
      .catch((caught) => {
        setItems([])
        setStatus('error')
        setError(caught instanceof Error ? caught.message : '资产库加载失败')
      })
  }

  useEffect(load, [api])

  useEffect(() => {
    if (!selectedWeaponId) {
      setDetail(null)
      setDetailStatus('idle')
      return
    }
    let cancelled = false
    setDetailStatus('loading')
    setDetailError(null)
    api.getWeapon(selectedWeaponId)
      .then((nextDetail) => {
        if (cancelled) return
        setDetail(nextDetail)
        onWeaponDetailLoaded(nextDetail)
        if (!activeVersionId && nextDetail.current_version_id) onVersionSelected(nextDetail.current_version_id)
        setDetailStatus('ready')
      })
      .catch((caught) => {
        if (cancelled) return
        setDetail(null)
        setDetailStatus('error')
        setDetailError(caught instanceof Error ? caught.message : '武器详情加载失败')
      })
    return () => {
      cancelled = true
    }
  }, [api, selectedWeaponId])

  return (
    <section className="panel-section">
      <h1>资产库</h1>
      {status === 'loading' && <p className="muted">正在读取本地资产库...</p>}
      {status === 'error' && (
        <div className="inline-error">
          <strong>资产库不可用</strong>
          <span>{error}</span>
          <code>{api.getBaseUrl()}</code>
          <div className="button-row">
            <button onClick={load}>重试</button>
            <button onClick={onOpenSettings}>打开设置</button>
          </div>
        </div>
      )}
      {status === 'empty' ? (
        <p className="muted">暂无入库武器。创建 mock 任务后会出现记录。</p>
      ) : status === 'ready' ? (
        <div className="library-browser">
          <ul className="asset-list library-weapon-list">
            {items.map((weapon) => (
              <li key={weapon.weapon_id} className={weapon.weapon_id === selectedWeaponId ? 'selected' : ''}>
                <button
                  onClick={() => {
                    setLocalSelectedWeaponId(weapon.weapon_id)
                    onWeaponSelected(weapon.weapon_id, weapon.current_version_id ?? undefined)
                  }}
                >
                  <strong>{weapon.display_name}</strong>
                  <span>{weapon.weapon_family} · {weapon.stage}</span>
                  <small>version {weapon.current_version_id ?? 'none'} · model {weapon.current_model_id ?? 'none'}</small>
                  <small>{new Date(weapon.updated_at).toLocaleString()}</small>
                </button>
              </li>
            ))}
          </ul>
          <div className="library-detail">
            {detailStatus === 'loading' && <p className="muted">正在读取版本和资产...</p>}
            {detailStatus === 'error' && (
              <div className="inline-error">
                <strong>详情不可用</strong>
                <span>{detailError}</span>
              </div>
            )}
            {detailStatus === 'ready' && detail && (
              <WeaponAssetDetail
                api={api}
                detail={detail}
                activeVersionId={activeVersionId || detail.current_version_id || ''}
                onVersionSelected={onVersionSelected}
                onOpenJobTrace={onOpenJobTrace}
              />
            )}
          </div>
        </div>
      ) : null}
    </section>
  )
}

function WeaponAssetDetail({
  api,
  detail,
  activeVersionId,
  onVersionSelected,
  onOpenJobTrace,
}: {
  api: ForgeApiClient
  detail: WeaponDetail
  activeVersionId: string
  onVersionSelected: (versionId: string) => void
  onOpenJobTrace?: (jobId: string) => void
}) {
  const versions = detail.versions ?? []
  const allAssets = versions.flatMap((version) => (version.assets ?? []).map((asset) => ({ ...asset, version_no: version.version_no, version_type: version.version_type })))
  const exportAssets = allAssets.filter((asset) => asset.role === 'unity_export_package')
  const modelAssets = allAssets.filter((asset) => asset.role.includes('glb') || asset.role === 'unity_material_json')
  const currentModelQuality = extractCurrentModelQuality(detail.current_model)
  const [previewAsset, setPreviewAsset] = useState<VersionAsset | null>(null)

  return (
    <div className="library-detail-content">
      <div className="library-summary">
        <strong>{detail.display_name}</strong>
        <span>{detail.weapon_family} · {detail.stage}</span>
        <small>当前版本 {detail.current_version_id ?? 'none'} · 当前模型 {detail.current_model_id ?? 'none'}</small>
      </div>

      <div className="library-kpis">
        <span>{versions.length} versions</span>
        <span>{allAssets.length} assets</span>
        <span>{modelAssets.length} model files</span>
        <span>{exportAssets.length} exports</span>
      </div>

      {exportAssets.length > 0 && (
        <div className="library-export-strip">
          <strong>Unity Export</strong>
          {exportAssets.map((asset) => (
            <a key={asset.asset_id} href={api.getAssetFileUrl(asset.asset_id)} download>
              {asset.asset_id} · {formatBytes(asset.byte_size)}
            </a>
          ))}
        </div>
      )}

      <VersionDagMap versions={versions} activeVersionId={activeVersionId} onVersionSelected={onVersionSelected} />

      <div className="library-version-list">
        {versions.map((version) => (
          <article
            key={version.version_id}
            className={`library-version-card ${version.version_id === activeVersionId ? 'selected' : ''}`}
          >
            <header>
              <strong>v{version.version_no} · {version.version_type}</strong>
              <span>{version.status}</span>
            </header>
            <small>{version.version_id}</small>
            {version.parent_version_id && <small>parent {version.parent_version_id}</small>}
            <button className="link-button" onClick={() => onVersionSelected(version.version_id)}>
              {version.version_id === activeVersionId ? '当前上下文版本' : '设为上下文版本'}
            </button>
            <VersionAssetPreview api={api} version={version} />
            <VersionHandoffActions api={api} version={version} />
            <VersionProvenanceSummary version={version} versions={versions} onOpenJobTrace={onOpenJobTrace} />
            <VersionHandoffChecklist api={api} version={version} currentModelQuality={currentModelQuality} />
            <div className="library-asset-table">
              {(version.assets ?? []).map((asset) => (
	                <div key={asset.asset_id} className="library-asset-row">
	                  <span>{asset.role}</span>
	                  <code>{asset.asset_id}</code>
	                  <small>{asset.mime_type} · {formatBytes(asset.byte_size)}</small>
	                  <button
	                    type="button"
	                    className="link-button"
	                    disabled={!isPreviewableAsset(asset)}
	                    onClick={() => setPreviewAsset(asset)}
	                  >
	                    预览
	                  </button>
	                  <a href={api.getAssetFileUrl(asset.asset_id)} download={downloadName(asset)}>
	                    file
	                  </a>
	                </div>
	              ))}
	            </div>
	          </article>
	        ))}
	      </div>
	      <AssetPreviewDrawer api={api} asset={previewAsset} onClose={() => setPreviewAsset(null)} />
	    </div>
	  )
}

function AssetPreviewDrawer({
  api,
  asset,
  onClose,
}: {
  api: ForgeApiClient
  asset: VersionAsset | null
  onClose: () => void
}) {
  const [status, setStatus] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle')
  const [summary, setSummary] = useState<AssetPreviewSummary | null>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!asset) {
      setStatus('idle')
      setSummary(null)
      setError('')
      return
    }
    let cancelled = false
    setStatus('loading')
    setSummary(null)
    setError('')
    loadAssetPreview(api, asset)
      .then((nextSummary) => {
        if (cancelled) return
        setSummary(nextSummary)
        setStatus('ready')
      })
      .catch((caught) => {
        if (cancelled) return
        setError(caught instanceof Error ? caught.message : '资产预览失败')
        setStatus('error')
      })
    return () => {
      cancelled = true
    }
  }, [api, asset])

  if (!asset) return null

  return (
    <aside className="library-preview-drawer" aria-label="资产预览">
      <header>
        <div>
          <strong>资产预览</strong>
          <span>{asset.role} · {asset.asset_id}</span>
        </div>
        <button type="button" className="link-button" onClick={onClose}>关闭</button>
      </header>
      {status === 'loading' && <p className="muted">正在读取受控资产文件...</p>}
      {status === 'error' && <div className="inline-error"><strong>预览不可用</strong><span>{error}</span></div>}
      {status === 'ready' && summary && (
        <>
          <div className="library-preview-summary">
            <strong>{summary.title}</strong>
            <span>{previewKindLabel(summary.kind)}</span>
          </div>
          <dl className="library-preview-metrics">
            {summary.rows.map((row) => (
              <div key={row.label}>
                <dt>{row.label}</dt>
                <dd>{row.value}</dd>
              </div>
            ))}
          </dl>
          <pre>{summary.body}</pre>
        </>
      )}
    </aside>
  )
}

function VersionAssetPreview({ api, version }: { api: ForgeApiClient; version: WeaponVersion }) {
  const assets = version.assets ?? []
  const previewAsset = findFirstRole(assets, ['concept_patch', 'concept_image', 'patch_mask'])
  const glbCount = assets.filter((asset) => asset.role.includes('glb')).length
  const exportAsset = findRole(assets, 'unity_export_package')
  return (
    <div className="library-asset-preview">
      {previewAsset ? (
        <img
          src={api.getAssetFileUrl(previewAsset.asset_id)}
          alt={`${version.version_type} ${previewAsset.role}`}
          loading="lazy"
        />
      ) : (
        <div className="library-preview-placeholder">
          <strong>{version.version_type}</strong>
          <span>{glbCount ? `${glbCount} GLB` : exportAsset ? 'Unity ZIP' : 'no image'}</span>
        </div>
      )}
      <div>
        <strong>快速预览</strong>
        <span>{previewAsset ? previewAsset.role : '非图片版本'}</span>
        <small>
          {previewAsset?.width && previewAsset.height ? `${previewAsset.width} x ${previewAsset.height}` : `${assets.length} assets`} · {formatBytes(sumAssetBytes(assets))}
        </small>
      </div>
    </div>
  )
}

function VersionHandoffActions({ api, version }: { api: ForgeApiClient; version: WeaponVersion }) {
  const [revealState, setRevealState] = useState<'idle' | 'opening' | 'opened' | 'error'>('idle')
  const [revealMessage, setRevealMessage] = useState('')
  const assets = version.assets ?? []
  const downloadableAssets = assets.filter((asset) => asset.byte_size > 0)
  const exportAsset = findRole(assets, 'unity_export_package')
  const modelAssets = assets.filter((asset) => asset.role.includes('glb') || asset.role === 'unity_material_json')
  function revealExportLocation() {
    if (!exportAsset) return
    setRevealState('opening')
    setRevealMessage('')
    api.revealAsset(exportAsset.asset_id)
      .then((result) => {
        setRevealState('opened')
        setRevealMessage(`${result.target} · ${result.filename}`)
      })
      .catch((caught) => {
        setRevealState('error')
        setRevealMessage(caught instanceof Error ? caught.message : '打开位置失败')
      })
  }
  return (
    <div className="library-version-actions">
      <button
        type="button"
        onClick={() => downloadVersionAssets(api, downloadableAssets)}
        disabled={!downloadableAssets.length}
      >
        下载本版本文件
      </button>
      <span>{downloadableAssets.length} files · {formatBytes(sumAssetBytes(downloadableAssets))}</span>
      {exportAsset ? <a href={api.getAssetFileUrl(exportAsset.asset_id)} download={downloadName(exportAsset)}>下载 Unity ZIP</a> : <small>Unity ZIP missing</small>}
      <button type="button" onClick={revealExportLocation} disabled={!exportAsset || revealState === 'opening'}>
        {revealState === 'opening' ? '正在打开位置' : '打开 ZIP 位置'}
      </button>
      {revealMessage && <small className={revealState === 'error' ? 'error' : 'ready'}>{revealMessage}</small>}
      {modelAssets.length ? <small>{modelAssets.length} Unity handoff files</small> : <small>no model files</small>}
    </div>
  )
}

function VersionProvenanceSummary({
  version,
  versions,
  onOpenJobTrace,
}: {
  version: WeaponVersion
  versions: WeaponVersion[]
  onOpenJobTrace?: (jobId: string) => void
}) {
  const parent = version.parent_version_id ? versions.find((item) => item.version_id === version.parent_version_id) : null
  const assetRoles = Array.from(new Set((version.assets ?? []).map((asset) => asset.role))).sort()
  return (
    <div className="library-provenance">
      <strong>版本溯源</strong>
      <div>
        <span>job</span>
        <code>{version.job_id}</code>
      </div>
      <div>
        <span>source</span>
        <code>{parent ? `v${parent.version_no} · ${parent.version_type}` : 'root'}</code>
      </div>
      <div>
        <span>created</span>
        <code>{new Date(version.created_at).toLocaleString()}</code>
      </div>
      <div>
        <span>roles</span>
        <code>{assetRoles.length ? assetRoles.join(' / ') : 'none'}</code>
      </div>
      <button
        type="button"
        className="link-button"
        disabled={!version.job_id || !onOpenJobTrace}
        onClick={() => version.job_id && onOpenJobTrace?.(version.job_id)}
      >
        查看生成轨迹
      </button>
    </div>
  )
}

function VersionDagMap({
  versions,
  activeVersionId,
  onVersionSelected,
}: {
  versions: WeaponVersion[]
  activeVersionId: string
  onVersionSelected: (versionId: string) => void
}) {
  const versionNoById = new Map(versions.map((version) => [version.version_id, version.version_no]))
  return (
    <div className="library-version-dag">
      <strong>版本 DAG</strong>
      <div>
        {versions.map((version) => {
          const parentVersionNo = version.parent_version_id ? versionNoById.get(version.parent_version_id) : null
          return (
            <button
              key={version.version_id}
              className={version.version_id === activeVersionId ? 'active' : ''}
              onClick={() => onVersionSelected(version.version_id)}
            >
              <span>v{version.version_no}</span>
              <strong>{version.version_type}</strong>
              <small>{parentVersionNo ? `parent v${parentVersionNo}` : 'root'}</small>
            </button>
          )
        })}
      </div>
    </div>
  )
}

function VersionHandoffChecklist({
  api,
  version,
  currentModelQuality,
}: {
  api: ForgeApiClient
  version: WeaponVersion
  currentModelQuality: CurrentModelQuality | null
}) {
  const assets = version.assets ?? []
  const report = findRole(assets, 'quality_report')
  return (
    <div className="library-handoff-checklist">
      <strong>Unity handoff</strong>
      <LibraryQualityBadge version={version} report={report} currentModelQuality={currentModelQuality} />
      <LibraryHandoffItem label="raw" asset={findRole(assets, 'rough_raw_glb')} api={api} />
      <LibraryHandoffItem label="normalized" asset={findRole(assets, 'rough_normalized_glb')} api={api} />
      <LibraryHandoffItem label="optimized" asset={findRole(assets, 'rough_optimized_glb')} api={api} />
      <LibraryHandoffItem label="material" asset={findRole(assets, 'unity_material_json')} api={api} />
      <LibraryHandoffItem label="report" asset={report} api={api} />
      <LibraryHandoffItem label="zip" asset={findRole(assets, 'unity_export_package')} api={api} />
    </div>
  )
}

function LibraryQualityBadge({
  version,
  report,
  currentModelQuality,
}: {
  version: WeaponVersion
  report?: VersionAsset
  currentModelQuality: CurrentModelQuality | null
}) {
  const isCurrentModelReport = Boolean(report && currentModelQuality?.reportFileId === report.asset_id)
  if (isCurrentModelReport && currentModelQuality) {
    const severity = currentModelQuality.blockerCount > 0 ? 'blocker' : currentModelQuality.warningCount > 0 ? 'warning' : 'ready'
    return (
      <div className={`library-quality-badge ${severity}`}>
        <span>QC {currentModelQuality.status ?? 'unknown'}</span>
        <small>
          blockers {currentModelQuality.blockerCount} · warnings {currentModelQuality.warningCount} · triangles {formatMetric(currentModelQuality.triangleCount)} · materials {formatMetric(currentModelQuality.materialCount)} · bounds {currentModelQuality.boundsValid === false ? 'invalid' : 'ready'}
        </small>
      </div>
    )
  }
  if (version.version_type === 'rough_3d') {
    return (
      <div className={`library-quality-badge ${report ? 'present' : 'blocker'}`}>
        <span>{report ? 'QC report present' : 'QC report missing'}</span>
        <small>{report ? 'not current model report snapshot' : 'rough model handoff is incomplete'}</small>
      </div>
    )
  }
  if (report) {
    return (
      <div className="library-quality-badge present">
        <span>QC report present</span>
        <small>{version.version_type} quality gate</small>
      </div>
    )
  }
  return (
    <div className="library-quality-badge muted">
      <span>QC not applicable</span>
      <small>{version.version_type}</small>
    </div>
  )
}

function LibraryHandoffItem({
  label,
  asset,
  api,
}: {
  label: string
  asset?: VersionAsset
  api: ForgeApiClient
}) {
  return (
    <span className={asset ? 'ready' : 'missing'}>
      {label}
      {asset ? <a href={api.getAssetFileUrl(asset.asset_id)} download>{asset.asset_id}</a> : <small>missing</small>}
    </span>
  )
}

function findRole<T extends { role: string }>(assets: T[], role: string): T | undefined {
  return assets.find((asset) => asset.role === role)
}

function findFirstRole<T extends { role: string }>(assets: T[], roles: string[]): T | undefined {
  for (const role of roles) {
    const found = findRole(assets, role)
    if (found) return found
  }
  return undefined
}

function isPreviewableAsset(asset: VersionAsset) {
  return asset.mime_type === 'application/json'
    || asset.mime_type === 'model/gltf-binary'
    || asset.mime_type === 'application/zip'
    || asset.logical_path.endsWith('.glb')
    || asset.logical_path.endsWith('.zip')
}

async function loadAssetPreview(api: ForgeApiClient, asset: VersionAsset): Promise<AssetPreviewSummary> {
  const response = await fetch(api.getAssetFileUrl(asset.asset_id))
  if (!response.ok) throw new Error(`asset file request failed: ${response.status}`)
  if (asset.mime_type === 'application/json') {
    const text = await response.text()
    return summarizeJsonAsset(asset, text)
  }
  if (asset.mime_type === 'model/gltf-binary' || asset.logical_path.endsWith('.glb')) {
    const buffer = await response.arrayBuffer()
    return summarizeGlbAsset(asset, buffer)
  }
  if (asset.mime_type === 'application/zip' || asset.logical_path.endsWith('.zip')) {
    const buffer = await response.arrayBuffer()
    return summarizeZipAsset(asset, buffer)
  }
  throw new Error(`unsupported preview mime type: ${asset.mime_type}`)
}

function previewKindLabel(kind: AssetPreviewSummary['kind']) {
  if (kind === 'glb') return 'GLB header / glTF JSON chunk'
  if (kind === 'zip') return 'ZIP manifest / Unity package entries'
  return 'JSON metadata preview'
}

function summarizeJsonAsset(asset: VersionAsset, text: string): AssetPreviewSummary {
  const parsed = JSON.parse(text) as unknown
  const record = isRecord(parsed) ? parsed : {}
  const keys = Object.keys(record)
  return {
    title: `${asset.role} JSON`,
    kind: 'json',
    rows: [
      { label: 'schema', value: stringOrNull(record.schema_version) ?? 'none' },
      { label: 'keys', value: keys.slice(0, 8).join(' / ') || 'none' },
      { label: 'bytes', value: formatBytes(asset.byte_size) },
    ],
    body: clipText(JSON.stringify(parsed, null, 2), 6000),
  }
}

function summarizeGlbAsset(asset: VersionAsset, buffer: ArrayBuffer): AssetPreviewSummary {
  if (buffer.byteLength < 12) throw new Error('GLB file is too small')
  const view = new DataView(buffer)
  const magic = view.getUint32(0, true)
  const version = view.getUint32(4, true)
  const declaredLength = view.getUint32(8, true)
  const chunks: Array<{ type: string; length: number; offset: number }> = []
  let jsonText = ''
  let binLength = 0
  let offset = 12
  while (offset + 8 <= buffer.byteLength) {
    const chunkLength = view.getUint32(offset, true)
    const chunkType = view.getUint32(offset + 4, true)
    const dataOffset = offset + 8
    const type = glbChunkType(chunkType)
    chunks.push({ type, length: chunkLength, offset: dataOffset })
    if (type === 'JSON') {
      jsonText = new TextDecoder('utf-8').decode(buffer.slice(dataOffset, dataOffset + chunkLength)).trim()
    }
    if (type === 'BIN') binLength += chunkLength
    offset = dataOffset + chunkLength
  }
  const gltf = jsonText ? JSON.parse(jsonText) as Record<string, unknown> : {}
  const assetInfo = isRecord(gltf.asset) ? gltf.asset : {}
  return {
    title: `${asset.role} GLB`,
    kind: 'glb',
    rows: [
      { label: 'GLB header', value: `${magic === 0x46546c67 ? 'glTF' : `0x${magic.toString(16)}`} · v${version} · ${formatBytes(declaredLength)}` },
      { label: 'chunks', value: chunks.map((chunk) => `${chunk.type} ${formatBytes(chunk.length)}`).join(' / ') || 'none' },
      { label: 'meshes', value: countArray(gltf.meshes) },
      { label: 'materials', value: countArray(gltf.materials) },
      { label: 'textures', value: `${countArray(gltf.textures)} textures / ${countArray(gltf.images)} images` },
      { label: 'nodes', value: `${countArray(gltf.nodes)} nodes / ${countArray(gltf.scenes)} scenes` },
      { label: 'generator', value: stringOrNull(assetInfo.generator) ?? 'none' },
      { label: 'BIN', value: formatBytes(binLength) },
    ],
    body: clipText(JSON.stringify(gltf, null, 2), 6000),
  }
}

async function summarizeZipAsset(asset: VersionAsset, buffer: ArrayBuffer): Promise<AssetPreviewSummary> {
  const entries = readZipEntries(buffer)
  const manifestEntry = entries.find((entry) => entry.name.endsWith('/manifest.json') || entry.name === 'manifest.json')
  const unsafeEntries = entries.filter((entry) => !isSafeZipEntry(entry.name))
  let manifest: Record<string, unknown> = {}
  let manifestText = ''
  if (manifestEntry) {
    manifestText = await readZipEntryText(buffer, manifestEntry)
    manifest = JSON.parse(manifestText) as Record<string, unknown>
  }
  const packageRoot = stringOrNull(manifest.package_root) ?? commonZipRoot(entries) ?? 'unknown'
  const fileEntries = Array.isArray(manifest.files) ? manifest.files : []
  const manifestNames = fileEntries
    .map((item) => isRecord(item) ? stringOrNull(item.path) : null)
    .filter((item): item is string => Boolean(item))
  const missingManifestNames = manifestNames.filter((name) => !entries.some((entry) => entry.name === name))
  const glbEntries = entries.filter((entry) => entry.name.endsWith('.glb'))
  const jsonEntries = entries.filter((entry) => entry.name.endsWith('.json'))
  const readmeEntries = entries.filter((entry) => entry.name.endsWith('.txt') || entry.name.endsWith('.md'))
  return {
    title: `${asset.role} ZIP`,
    kind: 'zip',
    rows: [
      { label: 'ZIP package', value: `${entries.length} entries · ${formatBytes(asset.byte_size)}` },
      { label: 'package root', value: packageRoot },
      { label: 'manifest.json', value: manifestEntry ? `${formatBytes(manifestEntry.uncompressedSize)} · ${fileEntries.length} manifest files` : 'missing' },
      { label: 'Unity payload', value: `${glbEntries.length} GLB / ${jsonEntries.length} JSON / ${readmeEntries.length} text` },
      { label: 'path safety', value: unsafeEntries.length ? `${unsafeEntries.length} unsafe entries` : 'all relative safe paths' },
      { label: 'manifest coverage', value: missingManifestNames.length ? `${missingManifestNames.length} missing ZIP entries` : manifestNames.length ? 'all manifest files present' : 'no manifest file list' },
    ],
    body: clipText(JSON.stringify({
      package_root: packageRoot,
      manifest_preview: manifest,
      zip_entries: entries.map((entry) => ({
        path: entry.name,
        compression: zipCompressionLabel(entry.compression),
        compressed_size: entry.compressedSize,
        uncompressed_size: entry.uncompressedSize,
      })),
      unsafe_entries: unsafeEntries.map((entry) => entry.name),
      missing_manifest_entries: missingManifestNames,
    }, null, 2), 8000),
  }
}

function readZipEntries(buffer: ArrayBuffer): ZipEntrySummary[] {
  const view = new DataView(buffer)
  const eocdOffset = findEndOfCentralDirectory(view)
  if (eocdOffset < 0) throw new Error('ZIP end-of-central-directory record was not found')
  const entryCount = view.getUint16(eocdOffset + 10, true)
  const centralDirectoryOffset = view.getUint32(eocdOffset + 16, true)
  const entries: ZipEntrySummary[] = []
  let offset = centralDirectoryOffset
  const decoder = new TextDecoder('utf-8')
  for (let index = 0; index < entryCount; index += 1) {
    if (offset + 46 > buffer.byteLength || view.getUint32(offset, true) !== 0x02014b50) {
      throw new Error('ZIP central directory is malformed')
    }
    const compression = view.getUint16(offset + 10, true)
    const compressedSize = view.getUint32(offset + 20, true)
    const uncompressedSize = view.getUint32(offset + 24, true)
    const nameLength = view.getUint16(offset + 28, true)
    const extraLength = view.getUint16(offset + 30, true)
    const commentLength = view.getUint16(offset + 32, true)
    const localHeaderOffset = view.getUint32(offset + 42, true)
    const nameStart = offset + 46
    const name = decoder.decode(buffer.slice(nameStart, nameStart + nameLength))
    entries.push({ name, compressedSize, uncompressedSize, compression, localHeaderOffset })
    offset = nameStart + nameLength + extraLength + commentLength
  }
  return entries
}

function findEndOfCentralDirectory(view: DataView): number {
  const minimumOffset = Math.max(0, view.byteLength - 65_557)
  for (let offset = view.byteLength - 22; offset >= minimumOffset; offset -= 1) {
    if (view.getUint32(offset, true) === 0x06054b50) return offset
  }
  return -1
}

async function readZipEntryText(buffer: ArrayBuffer, entry: ZipEntrySummary): Promise<string> {
  const view = new DataView(buffer)
  const offset = entry.localHeaderOffset
  if (offset + 30 > buffer.byteLength || view.getUint32(offset, true) !== 0x04034b50) {
    throw new Error(`ZIP local header is malformed for ${entry.name}`)
  }
  const nameLength = view.getUint16(offset + 26, true)
  const extraLength = view.getUint16(offset + 28, true)
  const dataOffset = offset + 30 + nameLength + extraLength
  const compressed = new Uint8Array(buffer.slice(dataOffset, dataOffset + entry.compressedSize))
  let bytes: ArrayBuffer
  if (entry.compression === 0) {
    bytes = compressed.buffer.slice(compressed.byteOffset, compressed.byteOffset + compressed.byteLength)
  } else if (entry.compression === 8) {
    bytes = await inflateRaw(compressed)
  } else {
    throw new Error(`Unsupported ZIP compression ${entry.compression} for ${entry.name}`)
  }
  return new TextDecoder('utf-8').decode(bytes)
}

async function inflateRaw(data: Uint8Array): Promise<ArrayBuffer> {
  const DecompressionCtor = (globalThis as unknown as { DecompressionStream?: new (format: string) => TransformStream<Uint8Array, Uint8Array> }).DecompressionStream
  if (!DecompressionCtor) throw new Error('Browser does not support ZIP deflate preview')
  const payload = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength) as ArrayBuffer
  const stream = new Blob([payload]).stream().pipeThrough(new DecompressionCtor('deflate-raw'))
  return new Response(stream).arrayBuffer()
}

function isSafeZipEntry(name: string): boolean {
  return Boolean(name)
    && !name.startsWith('/')
    && !name.startsWith('\\')
    && !/^[a-zA-Z]:/.test(name)
    && !name.split(/[\\/]+/).includes('..')
}

function commonZipRoot(entries: ZipEntrySummary[]): string | null {
  const first = entries.find((entry) => entry.name.includes('/'))?.name.split('/')[0]
  if (!first) return null
  return entries.every((entry) => entry.name === first || entry.name.startsWith(`${first}/`)) ? first : null
}

function zipCompressionLabel(value: number): string {
  if (value === 0) return 'stored'
  if (value === 8) return 'deflated'
  return `method ${value}`
}

function glbChunkType(value: number) {
  if (value === 0x4e4f534a) return 'JSON'
  if (value === 0x004e4942) return 'BIN'
  return `0x${value.toString(16)}`
}

function countArray(value: unknown) {
  return Array.isArray(value) ? String(value.length) : '0'
}

function clipText(value: string, maxLength: number) {
  return value.length > maxLength ? `${value.slice(0, maxLength)}\n... clipped ...` : value
}

function extractCurrentModelQuality(currentModel: unknown): CurrentModelQuality | null {
  if (!isRecord(currentModel)) return null
  const qualityReport = isRecord(currentModel.quality_report) ? currentModel.quality_report : {}
  const metrics = isRecord(qualityReport.metrics) ? qualityReport.metrics : {}
  const checks = Array.isArray(qualityReport.checks) ? qualityReport.checks : []
  return {
    reportFileId: stringOrNull(qualityReport.quality_report_file_id),
    status: stringOrNull(qualityReport.status),
    blockerCount: countChecks(checks, 'blocker', ['failed', 'blocked']),
    warningCount: countChecks(checks, 'warning', ['warning', 'failed', 'skipped']),
    triangleCount: numberOrNull(metrics.triangle_count),
    materialCount: numberOrNull(metrics.material_count),
    boundsValid: booleanOrNull(metrics.bounds_valid),
  }
}

function countChecks(checks: unknown[], level: string, statuses: string[]) {
  return checks.filter((check) => {
    if (!isRecord(check)) return false
    return check.level === level && statuses.includes(String(check.status || ''))
  }).length
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function stringOrNull(value: unknown): string | null {
  return typeof value === 'string' && value.length ? value : null
}

function numberOrNull(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return null
}

function booleanOrNull(value: unknown): boolean | null {
  return typeof value === 'boolean' ? value : null
}

function formatMetric(value: number | null) {
  return value === null ? 'missing' : Math.round(value).toLocaleString('en-US')
}

function downloadName(asset: { logical_path: string }) {
  return asset.logical_path.split('/').pop() || 'wushen-asset'
}

function sumAssetBytes(assets: { byte_size: number }[]) {
  return assets.reduce((total, asset) => total + asset.byte_size, 0)
}

function downloadVersionAssets(api: ForgeApiClient, assets: VersionAsset[]) {
  assets.forEach((asset, index) => {
    window.setTimeout(() => {
      const link = document.createElement('a')
      link.href = api.getAssetFileUrl(asset.asset_id)
      link.download = downloadName(asset)
      link.rel = 'noopener'
      document.body.appendChild(link)
      link.click()
      link.remove()
    }, index * 80)
  })
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / 1024 / 1024).toFixed(1)} MB`
}
