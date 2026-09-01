/**
 * Single append-only vocabulary for the Three.js knife objective layer.
 * Existing IDs retain their historical meaning; intrinsic IDs never
 * reinterpret a legacy metric that was previously NOT_COMPUTABLE.
 */
export const KNIFE_OBJECTIVE_METRIC_CATALOG = Object.freeze([
  metric('silhouette-iou', 'raster', 'maximize', 'bounded-01', 'visual-evidence', 'authorized-reference-silhouette@1', 'authorized reference mask and frozen camera are required'),
  metric('boundary-f1', 'raster', 'maximize', 'bounded-01', 'visual-evidence', 'authorized-reference-boundary@1', 'authorized reference boundary and frozen camera are required'),
  metric('symmetric-chamfer', 'raster', 'minimize', 'nonnegative', 'visual-evidence', 'authorized-reference-contour-distance@1', 'authorized reference contour and frozen camera are required'),
  metric('p95-contour-distance', 'raster', 'minimize', 'nonnegative', 'visual-evidence', 'authorized-reference-contour-distance@1', 'authorized reference contour and frozen camera are required'),
  metric('tip-landmark-error', 'raster', 'minimize', 'nonnegative', 'visual-evidence', 'authorized-reference-landmark@1', 'authorized reference tip landmark is required'),
  metric('belly-depth-error', 'raster', 'minimize', 'nonnegative', 'visual-evidence', 'authorized-reference-landmark@1', 'authorized reference belly landmark is required'),
  metric('thickness-continuity', 'raster', 'maximize', 'bounded-01', 'visual-evidence', 'dedicated-thickness-evidence@1', 'dedicated thickness evidence is required; intrinsic scores use a new ID'),
  metric('normal-continuity', 'raster', 'maximize', 'bounded-01', 'visual-evidence', 'dedicated-normal-evidence@1', 'dedicated normal evidence is required; intrinsic scores use a new ID'),
  metric('part-id-coverage', 'raster', 'maximize', 'bounded-01', 'structural-proxy', 'eight-view-depth-resolved-part-id-union@1', 'fixed-rig Part-ID receipt is required'),
  metric('material-id-coverage', 'raster', 'maximize', 'bounded-01', 'structural-proxy', 'eight-view-depth-resolved-material-id-union@1', 'fixed-rig Material-ID receipt is required'),
  metric('negative-space-error', 'raster', 'minimize', 'nonnegative', 'structural-proxy', 'bound-guard-opening-target@1', 'a bound guard opening target is required; assembly proxies use a new ID'),
  metric('fps-occupancy', 'raster', 'maximize', 'bounded-01', 'structural-proxy', 'fps-hold-depth-resolved-mask-occupancy@1', 'fixed FPS_HOLD mask is required'),
  metric('blade-section-profile-continuity', 'blade', 'maximize', 'bounded-01', 'structural-proxy', 'KnifeIntrinsicMorphology@1/section-profile-continuity', 'valid ordered blade sections are required'),
  metric('blade-curve-g1', 'blade', 'maximize', 'bounded-01', 'structural-proxy', 'KnifeIntrinsicMorphology@1/curve-g1-proxy', 'two nondegenerate independent blade curves are required'),
  metric('blade-tip-taper', 'blade', 'maximize', 'bounded-01', 'structural-proxy', 'KnifeIntrinsicMorphology@1/tip-taper', 'root, belly and tip sections are required'),
  metric('blade-extrema-headroom', 'blade', 'maximize', 'bounded-01', 'structural-proxy', 'KnifeIntrinsicMorphology@1/extrema-budget-headroom', 'two nondegenerate independent blade curves are required'),
  metric('assembly-ratio-prior-score', 'assembly', 'maximize', 'bounded-01', 'structural-proxy', 'KnifeAssemblyIntrinsicMetrics@1/ratio-prior-score', 'guard, grip and pommel bounds are required'),
  metric('assembly-attachment-continuity', 'assembly', 'maximize', 'bounded-01', 'structural-proxy', 'KnifeAssemblyIntrinsicMetrics@1/attachment-continuity', 'blade-root, guard, grip and pommel bounds are required'),
  metric('assembly-material-readability', 'assembly', 'maximize', 'bounded-01', 'structural-proxy', 'KnifeAssemblyIntrinsicMetrics@1/material-zone-readability', 'at least two adjacent MaterialZones are required'),
  metric('assembly-complexity-efficiency', 'assembly', 'maximize', 'bounded-01', 'structural-proxy', 'KnifeAssemblyIntrinsicMetrics@1/complexity-efficiency', 'compiled geometry and declared budgets are required'),
] as const)

export type KnifeObjectiveMetricCatalogEntry = (typeof KNIFE_OBJECTIVE_METRIC_CATALOG)[number]
export type KnifeObjectiveMetricId = KnifeObjectiveMetricCatalogEntry['id']
export type KnifeObjectiveMetricOwner = KnifeObjectiveMetricCatalogEntry['owner']

export const KNIFE_OBJECTIVE_METRIC_IDS = Object.freeze(
  KNIFE_OBJECTIVE_METRIC_CATALOG.map((entry) => entry.id),
) as readonly KnifeObjectiveMetricId[]

export function knifeObjectiveMetricCatalogEntry(id: KnifeObjectiveMetricId): KnifeObjectiveMetricCatalogEntry {
  const entry = KNIFE_OBJECTIVE_METRIC_CATALOG.find((candidate) => candidate.id === id)
  if (!entry) throw new Error(`unknown knife objective metric ${id}`)
  return entry
}

export function isIntrinsicKnifeObjectiveMetric(id: KnifeObjectiveMetricId): boolean {
  const owner = knifeObjectiveMetricCatalogEntry(id).owner
  return owner === 'blade' || owner === 'assembly'
}

function metric<
  const Id extends string,
  const Owner extends 'raster' | 'blade' | 'assembly',
  const Direction extends 'maximize' | 'minimize',
  const Domain extends 'bounded-01' | 'nonnegative',
  const Evidence extends 'structural-proxy' | 'visual-evidence',
>(
  id: Id,
  owner: Owner,
  direction: Direction,
  value_domain: Domain,
  evidence_class: Evidence,
  basis_schema: string,
  not_computable_when: string,
) {
  return Object.freeze({ id, owner, direction, value_domain, evidence_class, basis_schema, not_computable_when })
}
