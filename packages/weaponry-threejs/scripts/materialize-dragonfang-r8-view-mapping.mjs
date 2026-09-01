#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const REPO = path.resolve(HERE, '../../..')
const REFERENCES = path.join(REPO, 'skills/weaponry-threejs-knife-studio/references')
const parentPath = path.join(REFERENCES, 'dragonfang-r7-reference-view-mapping.json')
const programPath = path.join(REFERENCES, 'dragonfang-procedural-successor-r8-blade.json')
const manifestPath = path.join(REPO, 'packages/weaponry-threejs/evidence/rendered-r8-fixed-views/manifest.json')
const outputPath = path.join(REFERENCES, 'dragonfang-r8-reference-view-mapping.json')

const args = process.argv.slice(2)
const referenceArgumentIndex = args.indexOf('--reference')
const referencePath = referenceArgumentIndex === -1 ? undefined : args[referenceArgumentIndex + 1]
if (!referencePath || referenceArgumentIndex + 2 !== args.length) {
  throw new Error('WPN_THREE_R8_VIEW_MAPPING_INVALID: provide exactly --reference <authorized-reference-path>')
}

function canonicalJson(value) {
  if (value === null) return 'null'
  if (typeof value === 'boolean' || typeof value === 'number' || typeof value === 'string') return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
}

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const assert = (condition, message) => { if (!condition) throw new Error(`WPN_THREE_R8_VIEW_MAPPING_INVALID: ${message}`) }

const parent = JSON.parse(await readFile(parentPath, 'utf8'))
const program = JSON.parse(await readFile(programPath, 'utf8'))
const manifestBytes = await readFile(manifestPath)
const manifest = JSON.parse(manifestBytes)
const referenceSha256 = sha256(await readFile(referencePath))
assert(parent.canonical_sha256 === 'bb3239cf72b1d468fd536f3179d9fa533135eb867a8c4ab5074274a7ca76cf6d', 'r7 mapping identity drifted')
assert(program.canonical_sha256 === '0c495db1c8ff2c0079cd5dafc3270eaafa9eae0ae1d0f41b6099d65db8ec51e1', 'r8 program identity drifted')
assert(manifest.program_sha256 === program.canonical_sha256 && manifest.view_count === 8 && manifest.aov_count === 48, 'r8 render manifest binding drifted')
assert(referenceSha256 === parent.reference_sha256, 'authorized reference bytes drifted')

const cameras = new Map(manifest.cameras.map((camera) => [camera.view_id, camera.camera_sha256]))
const mappings = parent.mappings.map((entry) => ({
  ...entry,
  camera_sha256: entry.render_view_id === null ? null : cameras.get(entry.render_view_id),
}))
assert(mappings.every((entry) => entry.render_view_id === null || typeof entry.camera_sha256 === 'string'), 'mapped camera binding is incomplete')

const mapping = {
  ...parent,
  mapping_id: 'dragonfang-r8-reference-view-mapping-001',
  program_sha256: program.canonical_sha256,
  aov_manifest_sha256: sha256(manifestBytes),
  preview_worker_cohort_sha256: manifest.preview_worker_cohort_sha256,
  camera_binding_policy: 'program-bound-fixed-camera-sha256@1',
  mappings,
  geometry_modified: false,
  canonical_sha256: '',
}
mapping.canonical_sha256 = sha256(canonicalJson(mapping))
await writeFile(outputPath, `${JSON.stringify(mapping, null, 2)}\n`)
process.stdout.write(`${JSON.stringify({ status: 'PASS_PROGRAM_BOUND_NOT_APPROVED', mapping_sha256: mapping.canonical_sha256, program_sha256: mapping.program_sha256, manifest_sha256: mapping.aov_manifest_sha256, front_camera_sha256: mappings[0].camera_sha256 }, null, 2)}\n`)
