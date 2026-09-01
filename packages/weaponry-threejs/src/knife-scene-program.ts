import type { Img2ThreeJsSourceEnvelope } from './img2threejs-source-envelope.ts'

/**
 * The closed, normalized input consumed by the Three.js knife route.
 *
 * This mirrors the accepted KnifeSceneProgram@1 contract without making the
 * browser compiler a Runtime/Store owner. The compiler never mutates this
 * value and all generated objects are derived from it.
 */

export type KnifeVec3 = readonly [number, number, number]

export type KnifeFamily = 'kukri' | 'tanto' | 'karambit' | 'bayonet' | 'machete' | 'original-knife'

export type KnifeDesignBasis =
  | 'authorized-reference-inspired'
  | 'original-design'
  | 'img2threejs-compatible-import'

export type KnifeCurveBasis = 'bezier' | 'nurbs-like'

/**
 * Closed transform policy used by the static img2threejs compatibility
 * importer.  This is metadata for the importer boundary, not a caller-facing
 * arbitrary transform channel.
 */
export const KNIFE_IMPORT_TRANSFORM_POLICY_SCHEMA = 'KnifeImportTransformPolicy@1' as const
export type KnifeImportTransformPolicySchema = typeof KNIFE_IMPORT_TRANSFORM_POLICY_SCHEMA

export interface KnifeImportTransformPolicy {
  readonly schema_version: KnifeImportTransformPolicySchema
  readonly coordinate_convention: 'weapon-front-z-up-right-handed@1'
  readonly position_mode: 'translation-preserved-normalized-design-units@1'
  readonly scale_mode: 'positive-axis-scale-applied-to-bounded-descriptor@1'
  readonly rotation_mode: 'axis-aligned-quarter-turn-euler-only@1'
  readonly position_abs_max: number
  readonly scale_min_exclusive: number
  readonly scale_max: number
  readonly rotation_abs_max_radians: number
  readonly quarter_turn_tolerance_radians: number
  readonly unsupported_behavior: 'mark-component-unsupported-and-block-full-assembly@1'
  readonly blade_longitudinal_normalization: 'transformed-station-x-span-to-minus-one-plus-one@1'
}

export const KNIFE_IMPORT_TRANSFORM_POLICY: KnifeImportTransformPolicy = Object.freeze({
  schema_version: KNIFE_IMPORT_TRANSFORM_POLICY_SCHEMA,
  coordinate_convention: 'weapon-front-z-up-right-handed@1',
  position_mode: 'translation-preserved-normalized-design-units@1',
  scale_mode: 'positive-axis-scale-applied-to-bounded-descriptor@1',
  rotation_mode: 'axis-aligned-quarter-turn-euler-only@1',
  position_abs_max: 4,
  scale_min_exclusive: 1e-4,
  scale_max: 4,
  rotation_abs_max_radians: Math.PI * 2,
  quarter_turn_tolerance_radians: 1e-4,
  unsupported_behavior: 'mark-component-unsupported-and-block-full-assembly@1',
  blade_longitudinal_normalization: 'transformed-station-x-span-to-minus-one-plus-one@1',
})

export interface KnifeCurve {
  readonly curve_id: string
  readonly basis: KnifeCurveBasis
  readonly control_points: readonly KnifeVec3[]
}

export type KnifeSectionRole = 'root' | 'shoulder' | 'belly' | 'tip' | 'intermediate'

export interface KnifeSection {
  readonly section_id: string
  readonly role: KnifeSectionRole
  readonly u: number
  readonly half_width: number
  readonly thickness: number
  readonly edge_offset: number
  readonly spine_offset: number
  readonly asymmetry: number
  readonly twist: number
}

export type KnifeSurfaceRole =
  | 'blade-body'
  | 'cutting-edge'
  | 'spine'
  | 'root-transition'
  | 'ricasso'
  | 'fuller'

export interface KnifeBladeSurface {
  readonly spine_curve: KnifeCurve
  readonly cutting_edge_curve: KnifeCurve
  readonly sections: readonly KnifeSection[]
  readonly surface_roles: readonly KnifeSurfaceRole[]
}

export type KnifePartRole =
  | 'blade-body'
  | 'cutting-edge'
  | 'guard'
  | 'grip'
  | 'pommel'
  | 'fastener'
  | 'gem'
  | 'relief'
  | 'helper'

/**
 * Closed, semantic assembly primitives.  Their coordinates are in the same
 * weapon-front-z-up-right-handed frame as the blade.  A primitive only
 * describes a bounded derived shape; it never becomes Runtime/Store/CAS
 * truth and it is always bound to an existing semantic part by part_id.
 */
export type KnifeAssemblyAxis = 'x' | 'y' | 'z'

export type KnifeReliefShape = 'panel' | 'diamond'

export interface KnifeDragonJawSpec {
  readonly span: number
  readonly thickness: number
  readonly depth: number
  readonly offset_y: number
  readonly offset_z: number
  readonly curvature: number
}

export interface KnifeDragonHornSpec {
  readonly feature_id: string
  readonly side: -1 | 1
  readonly length: number
  readonly radius: number
  readonly sweep: number
  readonly offset_z: number
}

export interface KnifeDragonEyeSocketSpec {
  readonly feature_id: string
  readonly side: -1 | 1
  readonly radius: number
  readonly depth: number
  readonly offset_y: number
  readonly offset_z: number
}

interface KnifeGuardBaseSpec {
  readonly primitive: 'guard'
  readonly part_id: string
  readonly center: KnifeVec3
  /** Crossguard span along the blade's local y axis. */
  readonly span: number
  /** Guard thickness along x (blade longitudinal axis). */
  readonly thickness: number
  /** Guard depth along z (front/back axis). */
  readonly depth: number
}

export interface KnifeClassicGuardSpec extends KnifeGuardBaseSpec {
  readonly style?: 'classic'
}

export interface KnifeDragonGuardSpec extends KnifeGuardBaseSpec {
  readonly style: 'dragon-guard'
  /** Gap between the upper and lower jaw rails; it is real open space. */
  readonly jaw_gap: number
  readonly upper_jaw: KnifeDragonJawSpec
  readonly lower_jaw: KnifeDragonJawSpec
  readonly horns: readonly KnifeDragonHornSpec[]
  readonly eye_sockets: readonly KnifeDragonEyeSocketSpec[]
}

export type KnifeGuardSpec = KnifeClassicGuardSpec | KnifeDragonGuardSpec

export interface KnifeGripSegmentSpec {
  readonly feature_id: string
  readonly start_u: number
  readonly end_u: number
  readonly radius_scale: number
}

export interface KnifeGripFrameSpec {
  readonly feature_id: string
  readonly at: number
  readonly width: number
  readonly thickness: number
}

export interface KnifeGripFastenerFeatureSpec {
  readonly feature_id: string
  readonly at: number
  readonly side: -1 | 1
  readonly radius: number
  readonly depth: number
}

interface KnifeGripBaseSpec {
  readonly primitive: 'grip'
  readonly part_id: string
  readonly center: KnifeVec3
  /** Handle length along x. */
  readonly length: number
  /** Nominal radial size in the y/z plane. */
  readonly radius: number
  /** Bounded end-to-end radius delta, expressed as a fraction of radius. */
  readonly taper: number
  /** Bounded radial tessellation count. */
  readonly facets: number
}

export interface KnifeClassicGripSpec extends KnifeGripBaseSpec {
  readonly style?: 'classic'
}

export interface KnifeSegmentedGripSpec extends KnifeGripBaseSpec {
  readonly style: 'segmented-grip'
  /** Local, x-ordered points for the curved grip centerline. */
  readonly centerline: readonly KnifeVec3[]
  readonly segments: readonly KnifeGripSegmentSpec[]
  readonly metal_frames: readonly KnifeGripFrameSpec[]
  readonly fasteners: readonly KnifeGripFastenerFeatureSpec[]
}

export type KnifeGripSpec = KnifeClassicGripSpec | KnifeSegmentedGripSpec

export interface KnifePommelHookSpec {
  readonly length: number
  readonly radius: number
  /** Bounded fraction controlling how far the hook turns back. */
  readonly bend: number
  readonly direction: -1 | 1
}

export interface KnifePommelGemSeatSpec {
  readonly feature_id: string
  readonly radius: number
  readonly depth: number
  readonly offset_x: number
  readonly offset_y: number
  readonly offset_z: number
  readonly axis: KnifeAssemblyAxis
}

interface KnifePommelBaseSpec {
  readonly primitive: 'pommel'
  readonly part_id: string
  readonly center: KnifeVec3
  /** Pommel span along x. */
  readonly length: number
  /** Pommel radius along y. */
  readonly radius: number
  /** Pommel depth along z. */
  readonly depth: number
}

export interface KnifeClassicPommelSpec extends KnifePommelBaseSpec {
  readonly style?: 'classic'
}

export interface KnifeHookedPommelSpec extends KnifePommelBaseSpec {
  readonly style: 'hooked-pommel'
  readonly hook: KnifePommelHookSpec
  readonly gem_seat: KnifePommelGemSeatSpec
}

export type KnifePommelSpec = KnifeClassicPommelSpec | KnifeHookedPommelSpec

export interface KnifeFastenerSpec {
  readonly primitive: 'fastener'
  readonly part_id: string
  readonly center: KnifeVec3
  readonly radius: number
  readonly depth: number
  readonly axis: KnifeAssemblyAxis
}

export interface KnifeGemSpec {
  readonly primitive: 'gem'
  readonly part_id: string
  readonly center: KnifeVec3
  readonly radius: number
  readonly depth: number
  readonly axis: KnifeAssemblyAxis
}

export interface KnifeReliefSpec {
  readonly primitive: 'relief'
  readonly part_id: string
  readonly center: KnifeVec3
  readonly width: number
  readonly height: number
  readonly depth: number
  readonly shape: KnifeReliefShape
  readonly axis: KnifeAssemblyAxis
}

export type KnifeAssemblyPrimitiveSpec =
  | KnifeGuardSpec
  | KnifeGripSpec
  | KnifePommelSpec
  | KnifeFastenerSpec
  | KnifeGemSpec
  | KnifeReliefSpec

/** Optional semantic assembly attached to a KnifeSceneProgram. */
export interface KnifeAssembly {
  readonly guard?: KnifeGuardSpec
  readonly grip?: KnifeGripSpec
  readonly pommel?: KnifePommelSpec
  readonly fasteners?: readonly KnifeFastenerSpec[]
  readonly gems?: readonly KnifeGemSpec[]
  readonly reliefs?: readonly KnifeReliefSpec[]
}

/** Explicit alias for callers that use the word Spec for the full assembly. */
export type KnifeAssemblySpec = KnifeAssembly

export type KnifeGuardPrimitive = KnifeGuardSpec
export type KnifeGripPrimitive = KnifeGripSpec
export type KnifePommelPrimitive = KnifePommelSpec
export type KnifeFastenerPrimitive = KnifeFastenerSpec
export type KnifeGemPrimitive = KnifeGemSpec
export type KnifeReliefPrimitive = KnifeReliefSpec

export type KnifeSourceClass = 'observed' | 'inferred' | 'design-prior' | 'original-choice'

export interface KnifePart {
  readonly part_id: string
  readonly role: KnifePartRole
  readonly source_class: KnifeSourceClass
  readonly material_zone_id: string
  readonly frozen: boolean
}

export interface KnifeMaterialZone {
  readonly material_zone_id: string
  readonly model: 'mesh-standard-layered@1'
  readonly base_color: string
  readonly metalness: number
  readonly roughness: number
}

export type KnifeAov =
  | 'beauty'
  | 'silhouette'
  | 'depth'
  | 'normal'
  | 'part-id'
  | 'material-id'
  | 'wireframe'
  | 'curvature'
  | 'uv-stretch'

export interface KnifePresentation {
  readonly camera_set: 'knife-fixed-eight-view@1'
  readonly renderer: 'threejs-browser-authority@1'
  readonly aovs: readonly KnifeAov[]
}

export interface KnifeBudgets {
  readonly max_triangles: number
  readonly max_draw_calls: number
  readonly max_texture_bytes: number
}

export interface KnifeSceneProgram {
  readonly schema_version: 'KnifeSceneProgram@1'
  readonly asset_id: string
  readonly family: KnifeFamily
  readonly design_basis: KnifeDesignBasis
  readonly coordinate_convention: 'weapon-front-z-up-right-handed@1'
  readonly blade_surface: KnifeBladeSurface
  /**
   * Optional closed source envelope used by the pinned img2threejs
   * compatibility route.  When present, the scene compiler uses its exact
   * primitive geometry; the semantic assembly remains a bounded projection
   * for IDs/material bindings and never replaces the source envelope.
   */
  readonly source_envelope?: Img2ThreeJsSourceEnvelope
  /** Omit for a blade-only program; present assembly entries are compiled only when explicitly bound. */
  readonly assembly?: KnifeAssembly
  readonly parts: readonly KnifePart[]
  readonly material_zones: readonly KnifeMaterialZone[]
  readonly presentation: KnifePresentation
  readonly budgets: KnifeBudgets
  readonly unknowns: readonly string[]
  /** Empty until the Runtime canonicalizes the program; the compiler does not invent it. */
  readonly canonical_sha256: string
}
