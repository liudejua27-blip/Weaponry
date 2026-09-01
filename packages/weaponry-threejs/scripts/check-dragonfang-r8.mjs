#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'

const REPO_ROOT = decodeURIComponent(new URL('../../..', import.meta.url).pathname).replace(/\/$/, '')
const path = (relative) => `${REPO_ROOT}/${relative}`
const sourcePath = path('skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r7.json')
const parentPath = path('skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r7-runtime-canonical.json')
const migrationPath = path('skills/weaponry-threejs-knife-studio/references/dragonfang-r7-canonical-migration.json')
const programPath = path('skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r8-blade.json')
const ledgerPath = path('skills/weaponry-threejs-knife-studio/references/dragonfang-objective-ledger-r8-blade.json')
const referencePath = path('skills/weaponry-threejs-knife-studio/references/dragonfang-front-blade-reference.json')
const receiptPath = path('packages/weaponry-threejs/artifacts/dragonfang-kukri-procedural-r8-blade.receipt.json')
const glbPath = path('packages/weaponry-threejs/artifacts/dragonfang-kukri-procedural-r8-blade.glb')

const [sourceBytes, parentBytes, migrationBytes, programBytes, ledgerBytes, referenceBytes, receiptBytes, glbBytes] = await Promise.all([
  readFile(sourcePath), readFile(parentPath), readFile(migrationPath), readFile(programPath),
  readFile(ledgerPath), readFile(referencePath), readFile(receiptPath), readFile(glbPath),
])
const source = JSON.parse(sourceBytes)
const parent = JSON.parse(parentBytes)
const migration = JSON.parse(migrationBytes)
const program = JSON.parse(programBytes)
const ledger = JSON.parse(ledgerBytes)
const reference = JSON.parse(referenceBytes)
const receipt = JSON.parse(receiptBytes)

assert(source.canonical_sha256 === '86352241001555c2a7a9f93d24185533d2b36415311cff1ad0f65e75efdcd58d', 'legacy r7 semantic identity drifted')
assert(sha256(sourceBytes) === '3362820ed731b171b1bfe692bada2ebb9d9432e2149a600c4e6e7aa22a5da5f6', 'legacy r7 bytes drifted')
assert(runtimeCanonical(parent) === parent.canonical_sha256, 'Runtime canonical parent hash drifted')
assert(migration.source.semantic_sha256 === source.canonical_sha256, 'migration source binding drifted')
assert(migration.successor.semantic_sha256 === parent.canonical_sha256, 'migration successor binding drifted')
assert(migration.successor.program_bytes_sha256 === sha256(parentBytes), 'migration successor bytes drifted')
assert(canonicalHash(migration) === migration.canonical_sha256, 'migration canonical hash drifted')

const sourcePayload = structuredClone(source)
const parentPayload = structuredClone(parent)
sourcePayload.canonical_sha256 = ''
parentPayload.canonical_sha256 = ''
assert(JSON.stringify(sourcePayload) === JSON.stringify(parentPayload), 'identity migration changed program payload')

assert(runtimeCanonical(program) === program.canonical_sha256, 'r8 program canonical hash drifted')
assert(canonicalHash(ledger) === ledger.canonical_sha256, 'r8 ledger canonical hash drifted')
assert(canonicalHash(receipt) === receipt.canonical_sha256, 'r8 receipt canonical hash drifted')
assert(ledger.program_sha256 === parent.canonical_sha256, 'r8 ledger does not bind canonical parent')
assert(ledger.evidence_sha256.includes(reference.canonical_sha256), 'r8 ledger does not bind admitted contour evidence')
assert(receipt.parent.program_sha256 === parent.canonical_sha256, 'r8 receipt parent binding drifted')
assert(receipt.successor.program_sha256 === program.canonical_sha256, 'r8 receipt program binding drifted')
assert(receipt.successor.program_bytes_sha256 === sha256(programBytes), 'r8 program bytes drifted')
assert(receipt.successor.glb_sha256 === sha256(glbBytes), 'r8 GLB bytes drifted')
assert(receipt.successor.glb_bytes === glbBytes.length, 'r8 GLB size drifted')
assert(receipt.status === 'BLADE_SUCCESSOR_COMPILED_REVIEW_ONLY', 'r8 crossed review-only status')
assert(receipt.visual_status === 'NOT_RUN' && receipt.commercial_status === 'NOT_RUN', 'r8 crossed quality boundary')

for (const key of ['assembly', 'parts', 'material_zones', 'presentation', 'budgets', 'unknowns']) {
  assert(canonicalJson(parent[key]) === canonicalJson(program[key]), `r8 changed frozen field ${key}`)
}
assert(JSON.stringify(ledger.allowed_scope) === JSON.stringify(['blade-body', 'cutting-edge']), 'r8 scope widened')
assert(ledger.frozen_parts.length === 11, 'r8 frozen Part cohort drifted')
assert(receipt.structural_delta.status === 'MEASURED_NONZERO', 'r8 has no fixed-view structural delta')

console.log(JSON.stringify({
  status: 'PASS',
  migration_sha256: migration.canonical_sha256,
  parent_program_sha256: parent.canonical_sha256,
  successor_program_sha256: program.canonical_sha256,
  successor_glb_sha256: receipt.successor.glb_sha256,
  frozen_part_count: ledger.frozen_parts.length,
  visual_status: receipt.visual_status,
}))

function assert(condition, message) {
  if (!condition) throw new Error(`WPN_THREE_R8_INVALID: ${message}`)
}

function runtimeCanonical(value) {
  const draft = structuredClone(value)
  draft.canonical_sha256 = ''
  return sha256(canonicalJson(draft))
}

function canonicalHash(value) {
  const draft = structuredClone(value)
  draft.canonical_sha256 = ''
  return sha256(canonicalJson(draft))
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex')
}

function canonicalJson(value) {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value)
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error('canonical JSON rejects non-finite numbers')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  if (typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
  }
  throw new Error(`canonical JSON rejects ${typeof value}`)
}
