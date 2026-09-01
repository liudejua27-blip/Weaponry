import * as THREE from 'three'

import { captureKnifeAovs, sha256Hex } from './weaponry-source/knife-browser-capture.ts'
import { createKnifeViewRig } from './weaponry-source/knife-view-evaluation.ts'
import normalizationContract from './upstream-render-normalization.contract.json' with { type: 'json' }
import { adaptImg2ThreeJsGroupToCompiledScene } from './img2threejs-compiled-scene-adapter.ts'
import { createUpstreamBaselineScene } from './upstream-entry.ts'

/** Browser-only benchmark bridge for the existing Weaponry capture contract. */

export const UPSTREAM_AOV_ENTRY_SCHEMA = 'WeaponryThreeJsUpstreamAovEntry@1' as const
export { THREE }

export interface UpstreamCaptureBinding {
  readonly scene: THREE.Scene
  readonly compiled: ReturnType<typeof adaptImg2ThreeJsGroupToCompiledScene>
  readonly rig: ReturnType<typeof createKnifeViewRig>
}

export interface UpstreamCaptureResult {
  readonly result: ReturnType<typeof captureKnifeAovs>
  readonly sink_records: readonly {
    readonly view_id: string
    readonly aov_id: string
    readonly png_sha256: string
    readonly png_size_bytes: number
  }[]
}

export function createUpstreamCaptureBinding(): UpstreamCaptureBinding {
  const baseline = createUpstreamBaselineScene()
  const compiled = adaptImg2ThreeJsGroupToCompiledScene(baseline.root, {
    source_fingerprint: normalizationContract.source.factory_sha256,
    group_name: 'img2threejs-capture-group',
  })
  const scene = baseline.scene
  scene.name = 'img2threejs-browser-capture-scene'
  scene.background = new THREE.Color(0x080a0d)
  scene.add(new THREE.HemisphereLight(0xffffff, 0x20242b, 1.4))
  const key = new THREE.DirectionalLight(0xffffff, 2.2)
  key.position.set(2.5, 3.5, 4.5)
  scene.add(key)
  scene.add(compiled.group)
  scene.updateMatrixWorld(true)
  const rig = createKnifeViewRig({
    frame_width: normalizationContract.fixed_view_rig.frame_width,
    frame_height: normalizationContract.fixed_view_rig.frame_height,
    margin: normalizationContract.fixed_view_rig.margin,
  })
  if (rig.deterministic_fingerprint !== '3fa0202473e3352b') {
    throw new Error(`fixed rig fingerprint drifted: ${rig.deterministic_fingerprint}`)
  }
  return { scene, compiled, rig }
}

export function captureUpstreamAovs(renderer: THREE.WebGLRenderer): UpstreamCaptureResult {
  const binding = createUpstreamCaptureBinding()
  const sinkRecords: {
    view_id: string
    aov_id: string
    png_sha256: string
    png_size_bytes: number
  }[] = []
  const result = captureKnifeAovs({
    renderer,
    scene: binding.scene,
    compiled: binding.compiled,
    rig: binding.rig,
    manifest_id: 'img2threejs-upstream-aov-baseline',
    clear_color: 0x000000,
    capture_sink: (viewId, aovId, pngBytes) => {
      sinkRecords.push({
        view_id: viewId,
        aov_id: aovId,
        png_sha256: sha256Hex(pngBytes),
        png_size_bytes: pngBytes.byteLength,
      })
    },
  })
  return { result, sink_records: Object.freeze(sinkRecords) }
}
