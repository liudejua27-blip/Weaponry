import * as THREE from 'three'

import {
  applyWeaponryPresentationPose,
  captureWeaponryPresentationBaseline,
  type WeaponryPresentationBaseline,
} from './presentation-pose.ts'
import {
  WEAPONRY_ACTIONS,
  type WeaponryAction,
} from './types.ts'

/** Closed identity for the deterministic rigid-pivot animation adapter. */
export const WEAPONRY_ANIMATION_SCHEMA = 'WeaponryThreeJsAnimationClipSet@1' as const
export const WEAPONRY_ANIMATION_MIXER_SCHEMA = 'WeaponryThreeJsAnimationMixer@1' as const
export const WEAPONRY_ANIMATION_MODE = 'RIGID_ROOT_PIVOT_NO_SKELETON' as const

export type WeaponryAnimationMarkerKind =
  | 'action-start'
  | 'hit-window-open'
  | 'hit-window-close'
  | 'inspect-hold'
  | 'action-end'

export interface WeaponryAnimationMarker {
  readonly marker_id: string
  readonly action: WeaponryAction
  readonly kind: WeaponryAnimationMarkerKind
  readonly time_s: number
}

export interface WeaponryAnimationClipDescriptor {
  readonly action: WeaponryAction
  readonly clip_name: string
  readonly duration_s: number
  readonly settles_to_idle: boolean
  readonly holds_at_end: boolean
  readonly markers: readonly WeaponryAnimationMarker[]
}

export interface WeaponryAnimationClipSet {
  readonly schema_version: typeof WEAPONRY_ANIMATION_SCHEMA
  readonly animation_mode: typeof WEAPONRY_ANIMATION_MODE
  readonly root_object_name: string
  readonly baseline: WeaponryPresentationBaseline
  readonly clips: Readonly<Record<WeaponryAction, THREE.AnimationClip>>
  readonly descriptors: Readonly<Record<WeaponryAction, WeaponryAnimationClipDescriptor>>
}

export interface WeaponryAnimationMixerSnapshot {
  readonly schema_version: typeof WEAPONRY_ANIMATION_MIXER_SCHEMA
  readonly animation_mode: typeof WEAPONRY_ANIMATION_MODE
  readonly action: WeaponryAction
  readonly status: 'idle' | 'playing' | 'holding'
  readonly time_s: number
  readonly duration_s: number
  readonly progress: number
  readonly markers_emitted: number
  readonly last_marker_id: string | null
}

export interface WeaponryAnimationMixerOptions {
  readonly baseline?: WeaponryPresentationBaseline
  readonly on_marker?: (marker: WeaponryAnimationMarker) => void
}

export interface WeaponryAnimationMixerAdapter {
  readonly schema_version: typeof WEAPONRY_ANIMATION_MIXER_SCHEMA
  readonly animation_mode: typeof WEAPONRY_ANIMATION_MODE
  readonly root: THREE.Object3D
  readonly mixer: THREE.AnimationMixer
  readonly clip_set: WeaponryAnimationClipSet
  play(action: WeaponryAction): WeaponryAnimationMixerSnapshot
  toggleInspect(): WeaponryAnimationMixerSnapshot
  advance(delta_s: number): WeaponryAnimationMixerSnapshot
  reset(): WeaponryAnimationMixerSnapshot
  snapshot(): WeaponryAnimationMixerSnapshot
  onMarker(listener: (marker: WeaponryAnimationMarker) => void): () => void
  dispose(): void
}

interface ActionTiming {
  readonly duration_s: number
  readonly settles_to_idle: boolean
  readonly holds_at_end: boolean
  readonly markers: readonly [
    WeaponryAnimationMarkerKind,
    ...WeaponryAnimationMarkerKind[],
  ]
}

const ACTION_TIMING: Readonly<Record<WeaponryAction, ActionTiming>> = Object.freeze({
  idle: {
    duration_s: 1 / 60,
    settles_to_idle: false,
    holds_at_end: true,
    markers: ['action-start', 'action-end'],
  },
  light: {
    duration_s: 0.24,
    settles_to_idle: true,
    holds_at_end: false,
    markers: ['action-start', 'hit-window-open', 'hit-window-close', 'action-end'],
  },
  heavy: {
    duration_s: 0.52,
    settles_to_idle: true,
    holds_at_end: false,
    markers: ['action-start', 'hit-window-open', 'hit-window-close', 'action-end'],
  },
  inspect: {
    duration_s: 0.8,
    settles_to_idle: false,
    holds_at_end: true,
    markers: ['action-start', 'inspect-hold', 'action-end'],
  },
  sheath: {
    duration_s: 0.32,
    settles_to_idle: true,
    holds_at_end: false,
    markers: ['action-start', 'action-end'],
  },
})

/**
 * Builds fixed clips from the same presentation pose function used by the
 * engine bridge. Tracks target only the rigid root transform; no bones,
 * skeleton, gameplay damage, or real-world weapon semantics are involved.
 */
export function createWeaponryAnimationClipSet(
  root: THREE.Object3D,
  baseline = captureWeaponryPresentationBaseline(root),
): WeaponryAnimationClipSet {
  const clips = {} as Record<WeaponryAction, THREE.AnimationClip>
  const descriptors = {} as Record<WeaponryAction, WeaponryAnimationClipDescriptor>
  for (const action of WEAPONRY_ACTIONS) {
    const timing = ACTION_TIMING[action]
    const markers = markersForAction(action, timing)
    const keyProgress = action === 'inspect' ? [0, 0.5, 1] : [0, 0.5, 1]
    const times = keyProgress.map((progress) => timing.duration_s * progress)
    const positions: number[] = []
    const quaternions: number[] = []
    for (const progress of keyProgress) {
      const pose = samplePresentationPose(baseline, action, progress)
      positions.push(pose.position.x, pose.position.y, pose.position.z)
      quaternions.push(pose.quaternion.x, pose.quaternion.y, pose.quaternion.z, pose.quaternion.w)
    }
    const clipName = `weaponry-r8-${action}-presentation`
    clips[action] = new THREE.AnimationClip(clipName, timing.duration_s, [
      new THREE.VectorKeyframeTrack('.position', times, positions),
      new THREE.QuaternionKeyframeTrack('.quaternion', times, quaternions),
    ])
    descriptors[action] = Object.freeze({
      action,
      clip_name: clipName,
      duration_s: timing.duration_s,
      settles_to_idle: timing.settles_to_idle,
      holds_at_end: timing.holds_at_end,
      markers,
    })
  }
  const clipSet: WeaponryAnimationClipSet = {
    schema_version: WEAPONRY_ANIMATION_SCHEMA,
    animation_mode: WEAPONRY_ANIMATION_MODE,
    root_object_name: root.name,
    baseline,
    clips: Object.freeze(clips),
    descriptors: Object.freeze(descriptors),
  }
  validateWeaponryAnimationClipSet(clipSet)
  return Object.freeze(clipSet)
}

export function validateWeaponryAnimationClipSet(clipSet: WeaponryAnimationClipSet): void {
  if (!clipSet || clipSet.schema_version !== WEAPONRY_ANIMATION_SCHEMA) {
    throw new Error('WEAPONRY_ANIMATION_INVALID_CLIP_SET: schema_version')
  }
  if (clipSet.animation_mode !== WEAPONRY_ANIMATION_MODE) {
    throw new Error('WEAPONRY_ANIMATION_INVALID_CLIP_SET: animation_mode')
  }
  for (const action of WEAPONRY_ACTIONS) {
    const timing = ACTION_TIMING[action]
    const clip = clipSet.clips[action]
    const descriptor = clipSet.descriptors[action]
    if (!clip || !descriptor || descriptor.action !== action) {
      throw new Error(`WEAPONRY_ANIMATION_INVALID_CLIP_SET: missing ${action}`)
    }
    if (clip.name !== descriptor.clip_name || clip.duration !== timing.duration_s) {
      throw new Error(`WEAPONRY_ANIMATION_INVALID_CLIP_SET: duration/name ${action}`)
    }
    if (clip.tracks.length !== 2 || clip.tracks.some((track) => track.name !== '.position' && track.name !== '.quaternion')) {
      throw new Error(`WEAPONRY_ANIMATION_INVALID_CLIP_SET: rigid tracks ${action}`)
    }
    if (descriptor.markers.length !== timing.markers.length) {
      throw new Error(`WEAPONRY_ANIMATION_INVALID_CLIP_SET: marker count ${action}`)
    }
    let previousTime = -1
    descriptor.markers.forEach((marker, index) => {
      if (marker.action !== action || marker.kind !== timing.markers[index]) {
        throw new Error(`WEAPONRY_ANIMATION_INVALID_CLIP_SET: marker kind ${action}`)
      }
      if (!Number.isFinite(marker.time_s) || marker.time_s < 0 || marker.time_s > timing.duration_s || marker.time_s < previousTime) {
        throw new Error(`WEAPONRY_ANIMATION_INVALID_CLIP_SET: marker time ${action}`)
      }
      previousTime = marker.time_s
    })
  }
}

/** Creates an AnimationMixer adapter with deterministic marker/readback state. */
export function createWeaponryAnimationMixerAdapter(
  root: THREE.Object3D,
  clipSet = createWeaponryAnimationClipSet(root),
  options: WeaponryAnimationMixerOptions = {},
): WeaponryAnimationMixerAdapter {
  validateWeaponryAnimationClipSet(clipSet)
  const baseline = options.baseline ?? clipSet.baseline
  const mixer = new THREE.AnimationMixer(root)
  const listeners = new Set<(marker: WeaponryAnimationMarker) => void>()
  if (options.on_marker) listeners.add(options.on_marker)
  let currentAction: WeaponryAction = 'idle'
  let status: WeaponryAnimationMixerSnapshot['status'] = 'idle'
  let timeS = 0
  let markerCursor = 0
  let lastMarkerId: string | null = null
  let disposed = false

  const emitMarker = (marker: WeaponryAnimationMarker): void => {
    markerCursor += 1
    lastMarkerId = marker.marker_id
    for (const listener of listeners) listener(marker)
  }

  const applyIdle = (): void => {
    mixer.stopAllAction()
    applyWeaponryPresentationPose(root, baseline, 'idle', 0)
  }

  const play = (requestedAction: WeaponryAction): WeaponryAnimationMixerSnapshot => {
    assertLive()
    if (!(WEAPONRY_ACTIONS as readonly string[]).includes(requestedAction)) {
      throw new Error(`WEAPONRY_ANIMATION_UNKNOWN_ACTION: ${String(requestedAction)}`)
    }
    const action = requestedAction === 'inspect' && currentAction === 'inspect' ? 'idle' : requestedAction
    const descriptor = clipSet.descriptors[action]
    mixer.stopAllAction()
    const mixerAction = mixer.clipAction(clipSet.clips[action], root)
    mixerAction.reset()
    mixerAction.setLoop(THREE.LoopOnce, 1)
    mixerAction.clampWhenFinished = true
    mixerAction.play()
    currentAction = action
    status = action === 'idle' ? 'idle' : 'playing'
    timeS = 0
    markerCursor = 0
    lastMarkerId = null
    mixer.update(0)
    for (const marker of descriptor.markers) {
      if (marker.time_s === 0) emitMarker(marker)
    }
    return snapshot()
  }

  const advance = (deltaS: number): WeaponryAnimationMixerSnapshot => {
    assertLive()
    if (!Number.isFinite(deltaS) || deltaS < 0) throw new Error('WEAPONRY_ANIMATION_INVALID_DELTA: expected finite non-negative seconds')
    const descriptor = clipSet.descriptors[currentAction]
    const previousTime = timeS
    const nextTime = Math.min(descriptor.duration_s, timeS + deltaS)
    mixer.update(nextTime - timeS)
    timeS = nextTime
    for (const marker of descriptor.markers) {
      if (marker.time_s > previousTime && marker.time_s <= nextTime) emitMarker(marker)
    }
    if (nextTime >= descriptor.duration_s && descriptor.settles_to_idle) {
      applyIdle()
      currentAction = 'idle'
      status = 'idle'
      timeS = 0
    } else if (nextTime >= descriptor.duration_s && descriptor.holds_at_end) {
      status = currentAction === 'inspect' ? 'holding' : 'idle'
    }
    return snapshot()
  }

  const reset = (): WeaponryAnimationMixerSnapshot => {
    assertLive()
    applyIdle()
    currentAction = 'idle'
    status = 'idle'
    timeS = 0
    markerCursor = 0
    lastMarkerId = null
    return snapshot()
  }

  const snapshot = (): WeaponryAnimationMixerSnapshot => {
    const descriptor = clipSet.descriptors[currentAction]
    return Object.freeze({
      schema_version: WEAPONRY_ANIMATION_MIXER_SCHEMA,
      animation_mode: WEAPONRY_ANIMATION_MODE,
      action: currentAction,
      status,
      time_s: timeS,
      duration_s: descriptor.duration_s,
      progress: descriptor.duration_s === 0 ? 1 : Math.min(1, timeS / descriptor.duration_s),
      markers_emitted: markerCursor,
      last_marker_id: lastMarkerId,
    })
  }

  const adapter: WeaponryAnimationMixerAdapter = {
    schema_version: WEAPONRY_ANIMATION_MIXER_SCHEMA,
    animation_mode: WEAPONRY_ANIMATION_MODE,
    root,
    mixer,
    clip_set: clipSet,
    play,
    toggleInspect() {
      return play(currentAction === 'inspect' ? 'idle' : 'inspect')
    },
    advance,
    reset,
    snapshot,
    onMarker(listener) {
      if (typeof listener !== 'function') throw new Error('WEAPONRY_ANIMATION_INVALID_MARKER_LISTENER')
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    dispose() {
      if (disposed) return
      disposed = true
      mixer.stopAllAction()
      mixer.uncacheRoot(root)
      listeners.clear()
    },
  }
  reset()
  return Object.freeze(adapter)

  function assertLive(): void {
    if (disposed) throw new Error('WEAPONRY_ANIMATION_ADAPTER_DISPOSED')
  }
}

function markersForAction(action: WeaponryAction, timing: ActionTiming): readonly WeaponryAnimationMarker[] {
  const positions: Readonly<Record<WeaponryAnimationMarkerKind, number>> = {
    'action-start': 0,
    'hit-window-open': action === 'light' ? 0.10 : action === 'heavy' ? 0.22 : 0,
    'hit-window-close': action === 'light' ? 0.17 : action === 'heavy' ? 0.36 : 0,
    'inspect-hold': 0.4,
    'action-end': timing.duration_s,
  }
  return Object.freeze(timing.markers.map((kind) => Object.freeze({
    marker_id: `weaponry-r8-${action}-${kind}`,
    action,
    kind,
    time_s: positions[kind],
  })))
}

function samplePresentationPose(
  baseline: WeaponryPresentationBaseline,
  action: WeaponryAction,
  progress: number,
): { readonly position: THREE.Vector3; readonly quaternion: THREE.Quaternion } {
  const scratch = new THREE.Object3D()
  applyWeaponryPresentationPose(scratch, baseline, action, progress)
  return { position: scratch.position.clone(), quaternion: scratch.quaternion.clone() }
}
