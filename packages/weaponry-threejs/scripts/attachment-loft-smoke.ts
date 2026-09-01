import * as THREE from 'three'

import {
  compileKnifeAttachmentLoft,
  KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION,
  KnifeAttachmentLoftError,
  type KnifeAttachmentLoftRole,
  type KnifeAttachmentLoftSpec,
  type KnifeAttachmentLoftVec3,
} from '../src/knife-attachment-loft.ts'

const ringPointIds = ['top-front', 'top-back', 'bottom-back', 'bottom-front']

function makeSpec(attachmentId: string, role: KnifeAttachmentLoftRole, side: 1 | -1): KnifeAttachmentLoftSpec {
  const sectionCenters = [
    [0, side * 0.18, 0],
    [0.18, side * 0.21, 0.015],
    [0.38, side * 0.16, 0.035],
  ] as const
  const scales = [
    [0.055, 0.045],
    [0.07, 0.05],
    [0.04, 0.032],
  ] as const
  const sections = sectionCenters.map((center, sectionIndex) => {
    const [radiusY, radiusZ] = scales[sectionIndex]
    const ring = ringPointIds.map((pointId, ringIndex) => {
      const angle = (ringIndex / ringPointIds.length) * Math.PI * 2
      const asymmetry = ringIndex === 1 ? side * 0.008 : ringIndex === 3 ? -side * 0.004 : 0
      return {
        point_id: pointId,
        position: [
          center[0],
          center[1] + Math.cos(angle) * radiusY + asymmetry,
          center[2] + Math.sin(angle) * radiusZ,
        ] as const,
      }
    })
    return { section_id: `${attachmentId}-section-${sectionIndex}`, ring }
  })
  return {
    schema_version: KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION,
    attachment_id: attachmentId,
    role,
    sections,
    cap_ends: true,
  }
}

const cases = [
  makeSpec('upper-jaw-loft', 'guard-upper-jaw', 1),
  makeSpec('lower-jaw-loft', 'guard-lower-jaw', -1),
  makeSpec('horn-loft', 'guard-horn', 1),
  makeSpec('eye-shell-loft', 'guard-eye-shell', -1),
  makeSpec('pommel-hook-loft', 'pommel-hook', -1),
]

const results = cases.map((spec) => {
  const first = compileKnifeAttachmentLoft(spec)
  const second = compileKnifeAttachmentLoft(spec)
  if (first.deterministic_fingerprint !== second.deterministic_fingerprint) {
    throw new Error(`${spec.attachment_id} is not deterministic`)
  }
  const geometry = first.geometry
  const position = geometry.getAttribute('position')
  const index = geometry.getIndex()
  if (!index || first.welded_indexed !== true || first.vertex_count !== position.count || index.count !== first.triangle_count * 3) {
    throw new Error(`${spec.attachment_id} is not an indexed welded mesh`)
  }
  if (new Set(index.array).size !== position.count || index.count <= position.count) {
    throw new Error(`${spec.attachment_id} did not share its ring vertices through indices`)
  }
  const normal = geometry.getAttribute('normal')
  if (!normal || [...normal.array].some((value) => !Number.isFinite(value))) {
    throw new Error(`${spec.attachment_id} has invalid derived normals`)
  }
  return {
    attachment_id: first.attachment_id,
    role: first.role,
    sections: first.section_count,
    ring_points: first.ring_point_count,
    vertices: first.vertex_count,
    triangles: first.triangle_count,
    fingerprint: first.deterministic_fingerprint,
    welded_indexed: first.welded_indexed,
  }
})

function expectFailure(spec: KnifeAttachmentLoftSpec, code: KnifeAttachmentLoftError['code'], label: string): void {
  try {
    compileKnifeAttachmentLoft(spec)
  } catch (error) {
    if (error instanceof KnifeAttachmentLoftError && error.code === code) return
    const received = error instanceof KnifeAttachmentLoftError ? error.code : String(error)
    throw new Error(`${label} failed with an unexpected error: ${received}`)
  }
  throw new Error(`${label} was not rejected`)
}

const degenerateBase = makeSpec('degenerate-loft', 'custom-attachment', 1)
const degenerateRing: readonly KnifeAttachmentLoftVec3[] = [
  [0, 0.22, 0],
  [0, 0.15, 0],
  [0, 0.08, 0],
  [0, -0.03, 0],
]
const degenerate: KnifeAttachmentLoftSpec = {
  ...degenerateBase,
  sections: degenerateBase.sections.map((section, sectionIndex) => ({
    ...section,
    ring: section.ring.map((point, pointIndex) => ({
      ...point,
      position: sectionIndex === 0
        ? degenerateRing[pointIndex]
        : point.position,
    })),
  })),
}
expectFailure(degenerate, 'DEGENERATE_PROFILE', 'zero-length ring edge')

const selfIntersectingBase = makeSpec('self-intersecting-loft', 'custom-attachment', 1)
const selfIntersectingRing: readonly KnifeAttachmentLoftVec3[] = [
  [0, -0.055, -0.045],
  [0, 0.055, 0.045],
  [0, -0.055, 0.045],
  [0, 0.055, -0.045],
  [0, 0, -0.02],
]
const selfIntersecting: KnifeAttachmentLoftSpec = {
  ...selfIntersectingBase,
  sections: selfIntersectingBase.sections.map((section, sectionIndex) => ({
    ...section,
    ring: selfIntersectingRing.map((position, pointIndex) => ({
      point_id: `self-point-${pointIndex}`,
      position: sectionIndex === 0 ? position : [0.2, position[1], position[2]] as const,
    })),
  })),
}
expectFailure(selfIntersecting, 'SELF_INTERSECTION', 'self-intersecting ring')

const nonFiniteBase = makeSpec('non-finite-loft', 'custom-attachment', 1)
const nonFinite: KnifeAttachmentLoftSpec = {
  ...nonFiniteBase,
  sections: nonFiniteBase.sections.map((section, sectionIndex) => ({
    ...section,
    ring: section.ring.map((point, pointIndex) => ({
      ...point,
      position: sectionIndex === 0 && pointIndex === 0
        ? [Number.NaN, point.position[1], point.position[2]] as const
        : point.position,
    })),
  })),
}
expectFailure(nonFinite, 'INVALID_SPEC', 'non-finite position')

const overBudgetBase = makeSpec('over-budget-loft', 'custom-attachment', 1)
const overBudget: KnifeAttachmentLoftSpec = {
  ...overBudgetBase,
  sections: Array.from({ length: 65 }, (_, sectionIndex) => ({
    section_id: `over-budget-section-${sectionIndex}`,
    ring: overBudgetBase.sections[0].ring.map((point) => ({
      ...point,
      position: [sectionIndex * 0.01, point.position[1], point.position[2]] as const,
    })),
  })),
}
expectFailure(overBudget, 'BUDGET_EXCEEDED', 'over-budget loft')

console.log(JSON.stringify({
  schema_version: KNIFE_ATTACHMENT_LOFT_SCHEMA_VERSION,
  cases: results,
  rejection_codes: ['INVALID_SPEC', 'DEGENERATE_PROFILE', 'SELF_INTERSECTION', 'BUDGET_EXCEEDED'],
  geometry_source: 'closed-ring-section-loft@1',
  primitive_fallback_used: false,
  renderer_invoked: false,
  quality_status: 'NOT_RUN',
}))
