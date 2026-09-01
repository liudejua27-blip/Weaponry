import { KNIFE_VIEW_IDS, type KnifeViewId, type KnifeViewRigOptions } from './knife-view-evaluation.ts'

export const KNIFE_PREVIEW_MANIFEST_SCHEMA = 'WeaponryThreeJsPreviewManifest@1' as const
export type KnifePreviewCaptureMode = 'capture-ready' | 'settled'
export type KnifePreviewFraming = 'blade-comparison' | 'full-asset-baseline'

export interface KnifePreviewManifest {
  readonly schema_version: typeof KNIFE_PREVIEW_MANIFEST_SCHEMA
  readonly view_ids: readonly KnifeViewId[]
  readonly frame_width?: number
  readonly frame_height?: number
  readonly margin?: number
  readonly capture?: KnifePreviewCaptureMode
  readonly aovs?: 'required'
  readonly framing?: KnifePreviewFraming
}

export interface KnifePreviewRequest {
  readonly manifest: KnifePreviewManifest
  readonly selected_view_ids: readonly KnifeViewId[]
  readonly rig_options: KnifeViewRigOptions
  readonly capture_mode: KnifePreviewCaptureMode
  readonly capture_aovs: boolean
  readonly framing: KnifePreviewFraming
}

export class KnifePreviewManifestError extends Error {
  constructor(message: string) {
    super(`KNIFE_PREVIEW_MANIFEST_INVALID: ${message}`)
    this.name = 'KnifePreviewManifestError'
  }
}

/**
 * Parse the closed preview manifest without reading files, URLs, or executing
 * caller code.  The normalized view order always follows the canonical rig.
 */
export function parseKnifePreviewManifest(value: unknown): KnifePreviewManifest {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new KnifePreviewManifestError('manifest must be an object')
  }
  const source = value as Record<string, unknown>
  const allowedKeys = new Set(['schema_version', 'view_ids', 'frame_width', 'frame_height', 'margin', 'capture', 'aovs', 'framing'])
  for (const key of Object.keys(source)) {
    if (!allowedKeys.has(key)) throw new KnifePreviewManifestError(`unknown field ${key}`)
  }
  if (source.schema_version !== KNIFE_PREVIEW_MANIFEST_SCHEMA) {
    throw new KnifePreviewManifestError(`schema_version must be ${KNIFE_PREVIEW_MANIFEST_SCHEMA}`)
  }
  if (!Array.isArray(source.view_ids) || source.view_ids.length < 1 || source.view_ids.length > KNIFE_VIEW_IDS.length) {
    throw new KnifePreviewManifestError('view_ids must contain one to eight fixed views')
  }

  const selected = new Set<KnifeViewId>()
  for (const raw of source.view_ids) {
    const viewId = normalizeViewId(raw)
    if (selected.has(viewId)) throw new KnifePreviewManifestError(`duplicate view ${viewId}`)
    selected.add(viewId)
  }
  const viewIds = KNIFE_VIEW_IDS.filter((viewId) => selected.has(viewId))
  const frameWidth = optionalFrameDimension(source.frame_width, 'frame_width')
  const frameHeight = optionalFrameDimension(source.frame_height, 'frame_height')
  const margin = optionalMargin(source.margin)
  const capture = optionalCapture(source.capture)
  const aovs = optionalAovs(source.aovs)
  const framing = optionalFraming(source.framing)

  const manifest: KnifePreviewManifest = {
    schema_version: KNIFE_PREVIEW_MANIFEST_SCHEMA,
    view_ids: Object.freeze([...viewIds]),
    ...(frameWidth === undefined ? {} : { frame_width: frameWidth }),
    ...(frameHeight === undefined ? {} : { frame_height: frameHeight }),
    ...(margin === undefined ? {} : { margin }),
    ...(capture === undefined ? {} : { capture }),
    ...(aovs === undefined ? {} : { aovs }),
    ...(framing === undefined ? {} : { framing }),
  }
  return Object.freeze(manifest)
}

/**
 * Resolve a local browser query.  `view=FRONT`, `views=FRONT,BACK`, or an
 * URI-encoded JSON `manifest={...}` are accepted; no query can name a view
 * outside the fixed eight-view vocabulary.
 */
export function parseKnifePreviewQuery(search: string): KnifePreviewRequest {
  if (typeof search !== 'string') throw new KnifePreviewManifestError('search must be text')
  const params = new URLSearchParams(search.startsWith('?') ? search.slice(1) : search)
  const allowedQueryKeys = new Set(['view', 'views', 'manifest', 'capture', 'aovs', 'framing'])
  params.forEach((_value, key) => {
    if (!allowedQueryKeys.has(key)) throw new KnifePreviewManifestError(`unknown query field ${key}`)
  })
  const manifestParam = params.get('manifest')
  const viewParam = params.get('view')
  const viewsParam = params.get('views')
  if (viewParam !== null && viewsParam !== null) {
    throw new KnifePreviewManifestError('use either view or views, not both')
  }

  let manifest: KnifePreviewManifest
  if (manifestParam !== null) {
    if (viewParam !== null || viewsParam !== null) {
      throw new KnifePreviewManifestError('manifest cannot be combined with view or views')
    }
    let decoded: unknown
    try {
      decoded = JSON.parse(manifestParam)
    } catch {
      throw new KnifePreviewManifestError('manifest query value must be URI-encoded JSON')
    }
    manifest = parseKnifePreviewManifest(decoded)
  } else {
    const rawViews = viewsParam ?? viewParam ?? 'FRONT'
    const entries = rawViews === 'ALL' ? [...KNIFE_VIEW_IDS] : rawViews.split(',').map((entry) => entry.trim()).filter(Boolean)
    manifest = parseKnifePreviewManifest({
      schema_version: KNIFE_PREVIEW_MANIFEST_SCHEMA,
      view_ids: entries,
    })
  }

  const queryCapture = params.get('capture')
  if (queryCapture !== null) {
    const capture = optionalCapture(queryCapture)
    if (capture === undefined) throw new KnifePreviewManifestError('capture must be capture-ready or settled')
    if (manifest.capture !== undefined && manifest.capture !== capture) {
      throw new KnifePreviewManifestError('query capture conflicts with manifest capture')
    }
    manifest = parseKnifePreviewManifest({ ...manifest, capture })
  }

  const queryAovs = params.get('aovs')
  if (queryAovs !== null) {
    const aovs = optionalAovs(queryAovs)
    if (aovs === undefined) throw new KnifePreviewManifestError('aovs must be required')
    if (manifest.aovs !== undefined && manifest.aovs !== aovs) {
      throw new KnifePreviewManifestError('query aovs conflicts with manifest aovs')
    }
    manifest = parseKnifePreviewManifest({ ...manifest, aovs })
  }

  const queryFraming = params.get('framing')
  if (queryFraming !== null) {
    const framing = optionalFraming(queryFraming)
    if (framing === undefined) throw new KnifePreviewManifestError('framing must be blade-comparison or full-asset-baseline')
    if (manifest.framing !== undefined && manifest.framing !== framing) {
      throw new KnifePreviewManifestError('query framing conflicts with manifest framing')
    }
    manifest = parseKnifePreviewManifest({ ...manifest, framing })
  }

  return Object.freeze({
    manifest,
    selected_view_ids: manifest.view_ids,
    rig_options: {
      ...(manifest.frame_width === undefined ? {} : { frame_width: manifest.frame_width }),
      ...(manifest.frame_height === undefined ? {} : { frame_height: manifest.frame_height }),
      ...(manifest.margin === undefined ? {} : { margin: manifest.margin }),
    },
    capture_mode: manifest.capture ?? 'settled',
    capture_aovs: manifest.aovs === 'required',
    framing: manifest.framing ?? 'blade-comparison',
  })
}

function normalizeViewId(value: unknown): KnifeViewId {
  if (typeof value !== 'string') throw new KnifePreviewManifestError('view IDs must be text')
  const normalized = value.trim().toUpperCase().replaceAll('-', '_')
  const viewId = KNIFE_VIEW_IDS.find((candidate) => candidate === normalized)
  if (!viewId) throw new KnifePreviewManifestError(`unsupported fixed view ${value}`)
  return viewId
}

function optionalFrameDimension(value: unknown, name: string): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || (value as number) < 16 || (value as number) > 2048) {
    throw new KnifePreviewManifestError(`${name} must be an integer in [16, 2048]`)
  }
  return value as number
}

function optionalMargin(value: unknown): number | undefined {
  if (value === undefined) return undefined
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value >= 0.45) {
    throw new KnifePreviewManifestError('margin must be finite and in [0, 0.45)')
  }
  return value
}

function optionalCapture(value: unknown): KnifePreviewCaptureMode | undefined {
  if (value === undefined) return undefined
  if (value !== 'capture-ready' && value !== 'settled') {
    throw new KnifePreviewManifestError('capture must be capture-ready or settled')
  }
  return value
}

function optionalAovs(value: unknown): 'required' | undefined {
  if (value === undefined) return undefined
  if (value !== 'required') throw new KnifePreviewManifestError('aovs must be required')
  return value
}

function optionalFraming(value: unknown): KnifePreviewFraming | undefined {
  if (value === undefined) return undefined
  if (value !== 'blade-comparison' && value !== 'full-asset-baseline') {
    throw new KnifePreviewManifestError('framing must be blade-comparison or full-asset-baseline')
  }
  return value
}
