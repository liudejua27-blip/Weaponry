import {
  KNIFE_PREVIEW_MANIFEST_SCHEMA,
  KNIFE_VIEW_IDS,
  parseKnifePreviewQuery,
} from '../src/index.ts'

const all = parseKnifePreviewQuery('?views=ALL&capture=settled')
if (all.selected_view_ids.join(',') !== KNIFE_VIEW_IDS.join(',') || all.capture_mode !== 'settled') {
  throw new Error('ALL fixed-view query did not resolve to the canonical eight views')
}

const capture = parseKnifePreviewQuery('?views=ALL&capture=settled&aovs=required')
if (!capture.capture_aovs) throw new Error('required AOV query did not enable browser capture')

const fullAsset = parseKnifePreviewQuery('?views=ALL&capture=settled&framing=full-asset-baseline')
if (fullAsset.framing !== 'full-asset-baseline') throw new Error('full asset framing did not remain explicit')

const encodedManifest = encodeURIComponent(JSON.stringify({
  schema_version: KNIFE_PREVIEW_MANIFEST_SCHEMA,
  view_ids: ['BACK', 'FRONT'],
  frame_width: 128,
  frame_height: 96,
  margin: 0.1,
}))
const manifest = parseKnifePreviewQuery(`?manifest=${encodedManifest}&capture=capture-ready`)
if (manifest.selected_view_ids.join(',') !== 'FRONT,BACK' || manifest.rig_options.frame_width !== 128 || manifest.capture_mode !== 'capture-ready') {
  throw new Error('closed preview manifest did not normalize or preserve its bounded fields')
}

let rejected = false
try {
  parseKnifePreviewQuery('?view=UNKNOWN')
} catch {
  rejected = true
}
if (!rejected) throw new Error('unknown fixed view was not rejected')

console.log(JSON.stringify({
  schema_version: KNIFE_PREVIEW_MANIFEST_SCHEMA,
  all_views: all.selected_view_ids,
  manifest_views: manifest.selected_view_ids,
  capture_modes: [all.capture_mode, manifest.capture_mode],
  unknown_view_rejected: rejected,
}))
