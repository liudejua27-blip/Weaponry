export {
  DEFAULT_WEAPONRY_INPUT_MAPPING,
  WEAPONRY_INPUT_MAPPING_SCHEMA,
  createWeaponryInputMapping,
  isWeaponryAction,
  resolveWeaponryAction,
} from './actions.ts'

export {
  WEAPONRY_CAMERA_SCHEMA,
  WEAPONRY_FPS_CAMERA_STATE,
  WEAPONRY_INSPECT_CAMERA_STATE,
  applyWeaponryCameraState,
  createWeaponryCamera,
  type WeaponryCameraFrame,
} from './camera.ts'

export {
  WEAPONRY_COLLIDER_PROXY_SCHEMA,
  createWeaponryColliderProxyDescriptors,
  projectWeaponryColliderIntent,
  type WeaponryColliderProxyDescriptor,
} from './colliders.ts'

export {
  WEAPONRY_PRESENTATION_MODE,
  applyWeaponryPresentationPose,
  captureWeaponryPresentationBaseline,
  type WeaponryPresentationBaseline,
} from './presentation-pose.ts'

export {
  WEAPONRY_FRAME_BUDGET_SCHEMA,
  WEAPONRY_FRAME_BUDGET_SNAPSHOT_SCHEMA,
  createWeaponryFrameBudgetMonitor,
  type WeaponryFrameBudgetConfig,
  type WeaponryFrameBudgetMonitor,
  type WeaponryFrameBudgetSnapshot,
  type WeaponryFrameTimingSample,
} from './performance.ts'

export {
  WEAPONRY_SIMULATION_SCHEMA,
  advanceWeaponrySimulation,
  createWeaponrySimulationState,
  dispatchWeaponryAction,
  setWeaponryExploded,
  setWeaponryPartVisible,
  setWeaponrySelectedPart,
  validateWeaponrySimulationState,
  weaponryActionDurationMs,
} from './simulation.ts'

export {
  createR8KnifeControllerBridge,
  createWeaponryR8ControllerBridge,
  loadR8KnifeDeliveryFromBytes,
  loadWeaponryR8Delivery,
  type LoadWeaponryR8Options,
  type WeaponryR8ControllerBridge,
  type WeaponryR8Delivery,
  type WeaponryR8GlbParser,
} from './r8-bridge.ts'

export {
  createR8WeaponryEngine,
  createWeaponryThreeJsEngine,
  createWeaponryThreeJsEngineFromRoot,
  snapshotWeaponryEngineState,
  type WeaponryEngineSnapshot,
  type WeaponryThreeJsEngine,
} from './engine.ts'

export {
  WEAPONRY_ANIMATION_MODE,
  WEAPONRY_ANIMATION_MIXER_SCHEMA,
  WEAPONRY_ANIMATION_SCHEMA,
  createWeaponryAnimationClipSet,
  createWeaponryAnimationMixerAdapter,
  validateWeaponryAnimationClipSet,
  type WeaponryAnimationClipDescriptor,
  type WeaponryAnimationClipSet,
  type WeaponryAnimationMarker,
  type WeaponryAnimationMarkerKind,
  type WeaponryAnimationMixerAdapter,
  type WeaponryAnimationMixerOptions,
  type WeaponryAnimationMixerSnapshot,
} from './animation.ts'

export {
  WEAPONRY_FPS_BINDING_MODE,
  WEAPONRY_FPS_HAND_ID,
  WEAPONRY_FPS_SOCKET_BINDING_SCHEMA,
  WEAPONRY_GRIP_SOCKET_ID,
  createWeaponryRightHandGripBinding,
  createWeaponryRightHandTarget,
  type WeaponryFpsSocketBinding,
  type WeaponryFpsSocketBindingSnapshot,
} from './fps.ts'

export {
  WEAPONRY_RAPIER_COLLISION_GROUPS,
  WEAPONRY_RAPIER_PREVIEW_GRAVITY,
  WEAPONRY_RAPIER_PREVIEW_SCHEMA,
  WEAPONRY_RAPIER_PREVIEW_SNAPSHOT_SCHEMA,
  createWeaponryRapierPreviewBridge,
  type WeaponryRapierPreviewBridge,
  type WeaponryRapierPreviewColliderSnapshot,
  type WeaponryRapierPreviewModule,
  type WeaponryRapierPreviewOptions,
  type WeaponryRapierPreviewSnapshot,
  type WeaponryRapierRootTransform,
} from './physics.ts'

export {
  WEAPONRY_ACTIONS,
  WEAPONRY_ACTION,
  WEAPONRY_CAMERA_STATES,
  WEAPONRY_INPUT_PHASES,
  WEAPONRY_THREEJS_ENGINE_BRIDGE_SCHEMA,
  WEAPONRY_THREEJS_ENGINE_SCHEMA,
  type WeaponryAction,
  type WeaponryActionName,
  type WeaponryCameraStateName,
  type WeaponryEngineApplyResult,
  type WeaponryInputEvent,
  type WeaponryInputMapping,
  type WeaponryInputPhase,
  type WeaponryInteractionState,
  type WeaponryInteractionStateName,
  type WeaponryPartPick,
  type WeaponrySimulationOptions,
  type WeaponrySimulationState,
  type WeaponryVec3,
} from './types.ts'
