import { validateImg2ThreeJsSourceEnvelope } from './img2threejs-source-envelope.ts'
import {
  fingerprintKnifeKnowledgeProgram,
  validateKnifeKnowledgeNativeProgram,
} from './knife-knowledge-candidate-generator.ts'
import type { KnifeDesignBasis, KnifeSceneProgram } from './knife-scene-program.ts'

export const KNIFE_NATIVE_SUCCESSOR_REQUEST_SCHEMA = 'KnifeNativeSuccessorPrepare@1' as const
export const KNIFE_NATIVE_SUCCESSOR_PLAN_SCHEMA = 'KnifeNativeSuccessorPlan@1' as const

export interface KnifeNativeSuccessorRequest {
  readonly schema_version: typeof KNIFE_NATIVE_SUCCESSOR_REQUEST_SCHEMA
  readonly source_program: KnifeSceneProgram
  readonly successor_asset_id: string
  readonly successor_design_basis: Exclude<KnifeDesignBasis, 'img2threejs-compatible-import'>
  readonly mutable_part_ids: readonly string[]
}

export interface KnifeNativeSuccessorPlan {
  readonly schema_version: typeof KNIFE_NATIVE_SUCCESSOR_PLAN_SCHEMA
  readonly source_program_fingerprint: string
  readonly source_envelope_fingerprint: string
  readonly source_component_count: number
  readonly source_material_count: number
  readonly successor_program_fingerprint: string
  readonly successor_program: KnifeSceneProgram
  readonly mutable_part_ids: readonly string[]
  readonly frozen_part_ids: readonly string[]
  readonly direct_source_mutation_performed: false
  readonly status: 'NATIVE_SUCCESSOR_PREPARED_REVIEW_ONLY'
  readonly visual_status: 'NOT_REVIEWED'
  readonly quality_status: 'NOT_RUN'
  readonly deterministic_fingerprint: string
}

export class KnifeNativeSuccessorError extends Error {
  constructor(message: string) {
    super(`KNIFE_NATIVE_SUCCESSOR_INVALID: ${message}`)
    this.name = 'KnifeNativeSuccessorError'
  }
}

/**
 * Fork a pinned compatibility program into a native, independently editable
 * successor. The source program and envelope remain untouched; their identity
 * is carried by this plan rather than hidden inside the successor geometry.
 */
export function prepareKnifeNativeSuccessor(input: KnifeNativeSuccessorRequest): KnifeNativeSuccessorPlan {
  assertExactKeys(input as unknown as Record<string, unknown>, [
    'schema_version',
    'source_program',
    'successor_asset_id',
    'successor_design_basis',
    'mutable_part_ids',
  ], 'request')
  if (input.schema_version !== KNIFE_NATIVE_SUCCESSOR_REQUEST_SCHEMA) fail('schema_version drifted')
  if (!isStableId(input.successor_asset_id)) fail('successor_asset_id must be a stable ID')
  if (input.successor_design_basis !== 'authorized-reference-inspired' && input.successor_design_basis !== 'original-design') {
    fail('successor_design_basis must be a native design basis')
  }
  const source = input.source_program
  if (!source || typeof source !== 'object' || source.design_basis !== 'img2threejs-compatible-import' || !source.source_envelope) {
    fail('source_program must be an img2threejs-compatible program with a source envelope')
  }
  try {
    validateImg2ThreeJsSourceEnvelope(source.source_envelope)
  } catch (error) {
    fail(error instanceof Error ? error.message : 'source envelope is invalid')
  }
  if (!Array.isArray(input.mutable_part_ids) || input.mutable_part_ids.length < 1 || input.mutable_part_ids.length > 32) {
    fail('mutable_part_ids must contain 1 to 32 IDs')
  }
  const partIds = source.parts.map((part) => part.part_id)
  const known = new Set(partIds)
  const mutable = [...input.mutable_part_ids]
  if (new Set(mutable).size !== mutable.length || mutable.some((partId) => !isStableId(partId) || !known.has(partId))) {
    fail('mutable_part_ids must be unique IDs from source_program.parts')
  }
  const mutableSet = new Set(mutable)
  const successor = clone(source) as Mutable<KnifeSceneProgram>
  delete successor.source_envelope
  successor.asset_id = input.successor_asset_id
  successor.design_basis = input.successor_design_basis
  successor.canonical_sha256 = ''
  successor.parts = source.parts.map((part) => ({ ...part, frozen: !mutableSet.has(part.part_id) }))
  const frozen = partIds.filter((partId) => !mutableSet.has(partId))
  const successorProgram = deepFreeze(successor as KnifeSceneProgram)
  try {
    validateKnifeKnowledgeNativeProgram(successorProgram)
  } catch (error) {
    fail(error instanceof Error ? error.message : 'native successor is invalid')
  }

  const draft = {
    schema_version: KNIFE_NATIVE_SUCCESSOR_PLAN_SCHEMA,
    source_program_fingerprint: fingerprintKnifeKnowledgeProgram(source),
    source_envelope_fingerprint: fingerprint(source.source_envelope),
    source_component_count: source.source_envelope.components.length,
    source_material_count: source.source_envelope.materials.length,
    successor_program_fingerprint: fingerprintKnifeKnowledgeProgram(successorProgram),
    successor_program: successorProgram,
    mutable_part_ids: Object.freeze(mutable),
    frozen_part_ids: Object.freeze(frozen),
    direct_source_mutation_performed: false as const,
    status: 'NATIVE_SUCCESSOR_PREPARED_REVIEW_ONLY' as const,
    visual_status: 'NOT_REVIEWED' as const,
    quality_status: 'NOT_RUN' as const,
    deterministic_fingerprint: '',
  }
  draft.deterministic_fingerprint = fingerprint({ ...draft, deterministic_fingerprint: '' })
  return deepFreeze(draft)
}

type Mutable<T> = { -readonly [K in keyof T]: T[K] }

function assertExactKeys(value: Record<string, unknown>, expected: readonly string[], label: string): void {
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    fail(`${label} contains unknown or missing fields`)
  }
}

function isStableId(value: unknown): value is string {
  return typeof value === 'string' && /^[a-zA-Z][a-zA-Z0-9_.-]{0,63}$/.test(value)
}

function clone<T>(value: T): T {
  if (Array.isArray(value)) return value.map((child) => clone(child)) as T
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value as Record<string, unknown>).map(([key, child]) => [key, clone(child)])) as T
  }
  return value
}

function deepFreeze<T>(value: T): T {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child)
  return Object.freeze(value)
}

function fingerprint(value: unknown): string {
  let hash = 0xcbf29ce484222325n
  for (const character of canonicalJson(value)) {
    hash ^= BigInt(character.codePointAt(0)!)
    hash = BigInt.asUintN(64, hash * 0x100000001b3n)
  }
  return hash.toString(16).padStart(16, '0')
}

function canonicalJson(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (value && typeof value === 'object') {
    const record = value as Record<string, unknown>
    return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(',')}}`
  }
  fail('fingerprint input contains a non-JSON value')
}

function fail(message: string): never {
  throw new KnifeNativeSuccessorError(message)
}
