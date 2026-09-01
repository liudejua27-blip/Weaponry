#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'

const REPO_ROOT = decodeURIComponent(new URL('../../..', import.meta.url).pathname).replace(/\/$/, '')
const SOURCE_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r7.json`
const SUCCESSOR_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r7-runtime-canonical.json`
const MIGRATION_PATH = `${REPO_ROOT}/skills/weaponry-threejs-knife-studio/references/dragonfang-r7-canonical-migration.json`

const SOURCE_SEMANTIC_SHA256 = '86352241001555c2a7a9f93d24185533d2b36415311cff1ad0f65e75efdcd58d'
const SOURCE_BYTES_SHA256 = '3362820ed731b171b1bfe692bada2ebb9d9432e2149a600c4e6e7aa22a5da5f6'
const RUNTIME_SEMANTIC_SHA256 = '24b4f6e558f59c825daf1127cb5751a79204e44bd2b3b116420ef8112f8a113f'
const RUNTIME_CAS_OBJECT_SHA256 = '8eced459ec61684788d4bed2b8ea88d5d8ed54c9524a9608852bcd59a738446d'

const sourceBytes = await readFile(SOURCE_PATH)
if (sha256(sourceBytes) !== SOURCE_BYTES_SHA256) {
  throw new Error('historical r7 bytes drifted; migration refuses to rewrite the source identity')
}
const source = JSON.parse(sourceBytes.toString('utf8'))
if (source.canonical_sha256 !== SOURCE_SEMANTIC_SHA256) {
  throw new Error('historical r7 semantic identity drifted')
}

const successor = structuredClone(source)
successor.canonical_sha256 = ''
const computedRuntimeSha = sha256(canonicalJson(successor))
if (computedRuntimeSha !== RUNTIME_SEMANTIC_SHA256) {
  throw new Error(`Runtime canonical successor drifted: ${computedRuntimeSha}`)
}
successor.canonical_sha256 = computedRuntimeSha
const successorBytes = Buffer.from(`${JSON.stringify(successor, null, 2)}\n`)

const sourcePayload = structuredClone(source)
const successorPayload = structuredClone(successor)
sourcePayload.canonical_sha256 = ''
successorPayload.canonical_sha256 = ''
if (JSON.stringify(sourcePayload) !== JSON.stringify(successorPayload)) {
  throw new Error('canonical migration changed the KnifeSceneProgram payload')
}

const migrationDraft = {
  schema_version: 'KnifeSceneProgramCanonicalMigration@1',
  task_id: 'WPN-THREE-CANONICAL-MIGRATION-011',
  asset_id: source.asset_id,
  source: {
    program_path: relative(SOURCE_PATH),
    canonicalization_policy: 'python-json-dumps-sort-keys-compact-float-exponent@legacy',
    semantic_sha256: SOURCE_SEMANTIC_SHA256,
    program_bytes_sha256: SOURCE_BYTES_SHA256,
  },
  successor: {
    program_path: relative(SUCCESSOR_PATH),
    canonicalization_policy: 'canonical-json-sha256-excluding-canonical-sha256@1',
    number_serialization_policy: 'serde-json-number-to-string-compatible@1',
    semantic_sha256: RUNTIME_SEMANTIC_SHA256,
    program_bytes_sha256: sha256(successorBytes),
    runtime_cas_object_sha256: RUNTIME_CAS_OBJECT_SHA256,
  },
  invariants: {
    semantic_payload_equal_excluding_canonical_sha256: true,
    geometry_changed: false,
    assembly_changed: false,
    material_changed: false,
    historical_receipt_rewritten: false,
    quality_status_promoted: false,
  },
  status: 'IDENTITY_SUCCESSOR_ONLY',
  visual_status: 'NOT_RUN',
  human_status: 'NOT_RUN',
  commercial_status: 'NOT_RUN',
  canonical_sha256: '',
}
const migration = {
  ...migrationDraft,
  canonical_sha256: sha256(canonicalJson(migrationDraft)),
}

await writeFile(SUCCESSOR_PATH, successorBytes)
await writeFile(MIGRATION_PATH, `${JSON.stringify(migration, null, 2)}\n`, 'utf8')

console.log(JSON.stringify({
  status: migration.status,
  source_semantic_sha256: SOURCE_SEMANTIC_SHA256,
  successor_semantic_sha256: RUNTIME_SEMANTIC_SHA256,
  successor_program_bytes_sha256: migration.successor.program_bytes_sha256,
  migration_sha256: migration.canonical_sha256,
}, null, 2))

function relative(path) {
  return path.startsWith(`${REPO_ROOT}/`) ? path.slice(REPO_ROOT.length + 1) : path
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
