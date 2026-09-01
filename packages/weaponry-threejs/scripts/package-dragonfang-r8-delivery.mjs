#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { copyFile, cp, mkdir, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { GLTFExporter } from 'three/examples/jsm/exporters/GLTFExporter.js'
import { build as viteBuild } from 'vite'

import {
  compileKnifeSceneProgram,
  makeKnifeSceneActionReady,
} from '../src/index.ts'

class NodeFileReader {
  result = null
  error = null
  onloadend = null
  onerror = null

  readAsArrayBuffer(blob) {
    blob.arrayBuffer().then(
      (value) => { this.result = value; this.onloadend?.({ target: this }) },
      (error) => { this.error = error; this.onerror?.(error) },
    )
  }

  readAsDataURL(blob) {
    blob.arrayBuffer().then(
      (value) => {
        const mime = blob.type || 'application/octet-stream'
        this.result = `data:${mime};base64,${Buffer.from(value).toString('base64')}`
        this.onloadend?.({ target: this })
      },
      (error) => { this.error = error; this.onerror?.(error) },
    )
  }
}

globalThis.FileReader ??= NodeFileReader

const here = dirname(fileURLToPath(import.meta.url))
const packageRoot = resolve(here, '..')
const repositoryRoot = resolve(packageRoot, '../..')
const programPath = resolve(repositoryRoot, 'skills/weaponry-threejs-knife-studio/references/dragonfang-procedural-successor-r8-blade.json')
const sourceGlbPath = resolve(packageRoot, 'artifacts/dragonfang-kukri-procedural-r8-blade.glb')
const renderManifestPath = resolve(packageRoot, 'evidence/rendered-r8-fixed-views/manifest.json')
const mappingPath = resolve(repositoryRoot, 'skills/weaponry-threejs-knife-studio/references/dragonfang-r8-reference-view-mapping.json')
const comparisonPath = resolve(packageRoot, 'evidence/dragonfang-r8-front-comparison/comparison.receipt.json')
const liveReceiptPath = resolve(repositoryRoot, 'docs/evidence/weaponry/wpn-three-r8-package-delivery-014-live.json')
const loaderPath = resolve(packageRoot, 'delivery/load-knife-delivery.mjs')
const threeLicensePath = resolve(repositoryRoot, 'node_modules/three/LICENSE')
const img2threejsAdoptionPath = resolve(packageRoot, 'adoption/img2threejs/9fbd0ca5bbcc3b13bebe712745d6784d33db0b85')
const outputDir = resolve(packageRoot, 'deliveries/dragonfang-r8')
const temporaryDir = `${outputDir}.tmp-${process.pid}`

const programBytes = await readFile(programPath)
const sourceGlbBytes = await readFile(sourceGlbPath)
const renderManifestBytes = await readFile(renderManifestPath)
const mappingBytes = await readFile(mappingPath)
const comparisonBytes = await readFile(comparisonPath)
const liveReceiptBytes = await readFile(liveReceiptPath).catch((error) => {
  if (error?.code === 'ENOENT') return null
  throw error
})
const loaderBytes = await readFile(loaderPath)
const threeLicenseBytes = await readFile(threeLicensePath)
const img2threejsLicenseBytes = await readFile(join(img2threejsAdoptionPath, 'LICENSES/Apache-2.0.txt'))
const standaloneLoaderBytes = await bundleStandaloneLoader(loaderPath)
const program = JSON.parse(programBytes.toString('utf8'))
const renderManifest = JSON.parse(renderManifestBytes.toString('utf8'))
const mapping = JSON.parse(mappingBytes.toString('utf8'))
const comparison = JSON.parse(comparisonBytes.toString('utf8'))
const liveReceipt = liveReceiptBytes === null ? null : JSON.parse(liveReceiptBytes.toString('utf8'))

assert(program.canonical_sha256 === '0c495db1c8ff2c0079cd5dafc3270eaafa9eae0ae1d0f41b6099d65db8ec51e1', 'r8 program identity drifted')
assert(sha256(sourceGlbBytes) === 'cfe9055ad025f80afeb62a12a882ea8cc856ba1ca68b30ba4523c0cdcd73d10d', 'r8 source GLB identity drifted')
assert(renderManifest.program_sha256 === program.canonical_sha256, 'render manifest is not bound to r8')
assert(mapping.program_sha256 === program.canonical_sha256, 'view mapping is not bound to r8')
assert(comparison.candidate.program_sha256 === program.canonical_sha256, 'comparison receipt is not bound to r8')

const compiled = compileKnifeSceneProgram(program)
const controller = makeKnifeSceneActionReady(compiled, program)
verifyActionRuntime(controller, compiled)

const exported = await new GLTFExporter().parseAsync(compiled.group, {
  binary: true,
  onlyVisible: true,
  trs: true,
})
assert(exported instanceof ArrayBuffer, 'GLTFExporter did not produce binary GLB')
const deliveryGlb = Buffer.from(exported)
const sourceGlb = parseGlb(sourceGlbBytes)
const packagedGlb = parseGlb(deliveryGlb)
verifyGlbRuntime(packagedGlb.json, controller.metadata)
verifyGeometryFrozen(sourceGlb, packagedGlb, controller.metadata.part_ids)
if (liveReceipt !== null) {
  assert(liveReceipt.status === 'PASS_RUNTIME_STORE_CAS_ACTION_READY_DELIVERY', 'live Runtime/CAS receipt did not pass')
  assert(liveReceipt.program_sha256 === program.canonical_sha256, 'live Runtime/CAS receipt program drifted')
  assert(liveReceipt.glb_sha256 === sha256(deliveryGlb), 'live Runtime/CAS receipt GLB drifted')
  assert(liveReceipt.glb_object_sha256 === liveReceipt.glb_sha256, 'live Runtime/CAS object hash drifted')
  assert(liveReceipt.glb_bytes === deliveryGlb.length, 'live Runtime/CAS byte length drifted')
  assert(liveReceipt.triangle_count === compiled.triangle_count, 'live Runtime/CAS triangle count drifted')
  assert(liveReceipt.part_count === controller.metadata.part_ids.length, 'live Runtime/CAS part count drifted')
}

const glbName = 'dragonfang-kukri-r8-action-ready.glb'
const programName = 'knife-scene-program-r8.json'
const loaderName = 'load-knife-delivery.mjs'
const standaloneLoaderName = 'load-knife-delivery.standalone.mjs'
const noticeName = 'THIRD_PARTY_NOTICES.md'
const readmeName = 'README.md'
const liveReceiptName = 'evidence/runtime-cas-live.receipt.json'
const notice = `# Third-party notices\n\n- Three.js 0.185.1 is bundled into the standalone delivery loader and is licensed under MIT. The full license is in LICENSES/MIT-Three.txt.\n- img2threejs commit 9fbd0ca5bbcc3b13bebe712745d6784d33db0b85 informed the Spec -> build -> fixed-view workflow and is licensed under Apache-2.0. No upstream executable source is bundled. Its license is in LICENSES/Apache-2.0-img2threejs.txt.\n- The Dragonfang r8 procedural asset program and delivery adapter are Weaponry first-party work. The authorized reference image is intentionally not redistributed in this package.\n`
const readme = `# Dragonfang Kukri r8 — Three.js delivery\n\nThis directory is a geometry-frozen, action-ready delivery of the accepted r8 approximation. It is not a commercial-art, human-review, Unreal, animation, UV/bake, or visual-quality PASS.\n\n## Load\n\nThe standalone module already contains the pinned Three.js runtime and GLTFLoader:\n\n\`\`\`js\nimport { loadKnifeDelivery } from './load-knife-delivery.standalone.mjs'\n\nconst { root, controller, manifest } = await loadKnifeDelivery({\n  baseUrl: new URL('./', import.meta.url),\n})\nscene.add(root)\ncontroller.setExploded(0.6)\ncontroller.setPartVisible('relief-dragon-spine', false)\n\`\`\`\n\nRaycast hits can be resolved with \`controller.resolvePart(hit.object)\`. The package contains 13 stable part pivots, three named sockets, two collider intents and two destruction groups.\n\nThe original source GLB is retained under \`provenance/\` for geometry-byte comparison. The 48 fixed-view PNG/AOV files are retained under \`evidence/fixed-views/\`; the authorized reference image is not bundled.\n`

const noticeBytes = Buffer.from(notice)
const readmeBytes = Buffer.from(readme)
const fixedViewTree = await treeDigest(resolve(packageRoot, 'evidence/rendered-r8-fixed-views'))
const programObjectBytes = Buffer.from(canonicalJson(program))
const metadata = controller.metadata
const manifest = {
  schema_version: 'WeaponryThreeJsKnifeDelivery@1',
  task_id: 'WPN-THREE-R8-PACKAGE-DELIVERY-014',
  asset_id: 'Dragonfang Kukri',
  asset_version: 'r8',
  delivery_kind: 'threejs-action-ready-glb@1',
  acceptance_basis: 'USER_ACCEPTED_APPROXIMATION',
  geometry_policy: 'r8-vertex-index-material-buffers-frozen-delivery-hierarchy-added@1',
  program: {
    path: programName,
    semantic_sha256: program.canonical_sha256,
    object_sha256: sha256(programObjectBytes),
    file_sha256: sha256(programBytes),
  },
  source_glb: {
    path: 'provenance/dragonfang-kukri-r8-source.glb',
    sha256: sha256(sourceGlbBytes),
    bytes: sourceGlbBytes.length,
  },
  delivery_glb: {
    path: glbName,
    sha256: sha256(deliveryGlb),
    bytes: deliveryGlb.length,
    triangles: compiled.triangle_count,
    draw_calls: compiled.parts.length,
    deterministic_fingerprint: compiled.deterministic_fingerprint,
    geometry_buffers_modified: false,
    hierarchy_modified: true,
  },
  action_runtime: metadata,
  files: [
    { path: loaderName, sha256: sha256(loaderBytes) },
    { path: standaloneLoaderName, sha256: sha256(standaloneLoaderBytes) },
    { path: readmeName, sha256: sha256(readmeBytes) },
    { path: noticeName, sha256: sha256(noticeBytes) },
    ...(liveReceiptBytes === null ? [] : [{ path: liveReceiptName, sha256: sha256(liveReceiptBytes) }]),
  ],
  fixed_view_evidence: {
    path: 'evidence/fixed-views',
    tree_sha256: fixedViewTree.sha256,
    file_count: fixedViewTree.file_count,
    manifest_sha256: sha256(renderManifestBytes),
    manifest_canonical_sha256: renderManifest.canonical_sha256,
    view_count: renderManifest.view_count,
    aov_count: renderManifest.aov_count,
    preview_worker_cohort_sha256: renderManifest.preview_worker_cohort_sha256,
  },
  reference_view_mapping: {
    file_sha256: sha256(mappingBytes),
    canonical_sha256: mapping.canonical_sha256,
  },
  comparison: {
    file_sha256: sha256(comparisonBytes),
    canonical_sha256: comparison.canonical_sha256,
    comparison_status: comparison.comparison_status,
    agent_decision: comparison.agent_visual_review.decision,
  },
  runtime_cas_persistence: liveReceipt === null ? 'NOT_RUN' : {
    status: liveReceipt.status,
    receipt_path: liveReceiptName,
    receipt_file_sha256: sha256(liveReceiptBytes),
    receipt_canonical_sha256: liveReceipt.canonical_sha256,
    build_cohort_sha256: liveReceipt.build_cohort_sha256,
    execution_id: liveReceipt.execution_id,
    worker_result_sha256: liveReceipt.worker_result_sha256,
    worker_result_object_sha256: liveReceipt.worker_result_object_sha256,
    glb_sha256: liveReceipt.glb_sha256,
    glb_object_sha256: liveReceipt.glb_object_sha256,
    exact_replay_status: liveReceipt.exact_replay_status,
    runtime_reopen_get_status: liveReceipt.runtime_reopen_get_status,
  },
  dependency_lock: {
    three_version: '0.185.1',
    standalone_loader_sha256: sha256(standaloneLoaderBytes),
    three_license_sha256: sha256(threeLicenseBytes),
    img2threejs_revision: '9fbd0ca5bbcc3b13bebe712745d6784d33db0b85',
    img2threejs_license_sha256: sha256(img2threejsLicenseBytes),
  },
  visual_status: 'NOT_APPROVED',
  human_status: 'NOT_RUN',
  engine_status: 'NOT_RUN',
  commercial_status: 'NOT_RUN',
  canonical_sha256: '',
}
manifest.canonical_sha256 = sha256(Buffer.from(canonicalJson(manifest)))
const manifestBytes = Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`)

await rm(temporaryDir, { recursive: true, force: true })
await mkdir(temporaryDir, { recursive: true })
await mkdir(join(temporaryDir, 'LICENSES'), { recursive: true })
await mkdir(join(temporaryDir, 'provenance'), { recursive: true })
await mkdir(join(temporaryDir, 'evidence'), { recursive: true })
await Promise.all([
  writeFile(join(temporaryDir, glbName), deliveryGlb),
  writeFile(join(temporaryDir, programName), programBytes),
  copyFile(loaderPath, join(temporaryDir, loaderName)),
  writeFile(join(temporaryDir, standaloneLoaderName), standaloneLoaderBytes),
  writeFile(join(temporaryDir, 'LICENSES/MIT-Three.txt'), threeLicenseBytes),
  writeFile(join(temporaryDir, 'LICENSES/Apache-2.0-img2threejs.txt'), img2threejsLicenseBytes),
  writeFile(join(temporaryDir, 'provenance/dragonfang-kukri-r8-source.glb'), sourceGlbBytes),
  writeFile(join(temporaryDir, noticeName), noticeBytes),
  writeFile(join(temporaryDir, readmeName), readmeBytes),
  writeFile(join(temporaryDir, 'delivery-manifest.json'), manifestBytes),
])
await cp(resolve(packageRoot, 'evidence/rendered-r8-fixed-views'), join(temporaryDir, 'evidence/fixed-views'), { recursive: true })
await copyFile(mappingPath, join(temporaryDir, 'evidence/reference-view-mapping.json'))
await copyFile(comparisonPath, join(temporaryDir, 'evidence/comparison.receipt.json'))
if (liveReceiptBytes !== null) await writeFile(join(temporaryDir, liveReceiptName), liveReceiptBytes)
await rm(outputDir, { recursive: true, force: true })
await mkdir(dirname(outputDir), { recursive: true })
await rename(temporaryDir, outputDir)

process.stdout.write(`${JSON.stringify({
  status: 'PASS_ACTION_READY_DELIVERY_NOT_VISUAL_APPROVAL',
  output_dir: outputDir,
  program_sha256: program.canonical_sha256,
  source_glb_sha256: sha256(sourceGlbBytes),
  delivery_glb_sha256: manifest.delivery_glb.sha256,
  delivery_glb_bytes: deliveryGlb.length,
  triangles: compiled.triangle_count,
  parts: metadata.part_ids.length,
  pivots: metadata.pivot_ids.length,
  sockets: metadata.socket_ids.length,
  collider_intents: metadata.collider_intents.length,
  destruction_groups: metadata.destruction_groups.length,
  manifest_canonical_sha256: manifest.canonical_sha256,
  manifest_file_sha256: sha256(manifestBytes),
}, null, 2)}\n`)

function verifyActionRuntime(runtime, scene) {
  assert(runtime.partMeshes.size === scene.parts.length, 'mesh coverage is incomplete')
  assert(runtime.partPivots.size === scene.parts.length, 'pivot coverage is incomplete')
  assert(runtime.sockets.size === 3, 'socket coverage is incomplete')
  for (const [partId, mesh] of runtime.partMeshes) {
    assert(runtime.resolvePart(mesh) === partId, `pick resolver drifted for ${partId}`)
  }
  const before = new Map([...runtime.partPivots].map(([id, pivot]) => [id, pivot.position.clone()]))
  runtime.setExploded(1)
  assert([...runtime.partPivots].some(([id, pivot]) => !pivot.position.equals(before.get(id))), 'explode did not move parts')
  runtime.setExploded(0)
  for (const [id, pivot] of runtime.partPivots) assert(pivot.position.equals(before.get(id)), `explode reset drifted for ${id}`)
  const destructionParts = runtime.metadata.destruction_groups.flatMap((group) => group.part_ids).sort()
  assert(destructionParts.join('\u0000') === [...runtime.metadata.part_ids].sort().join('\u0000'), 'destruction groups do not cover all parts exactly')
}

function parseGlb(bytes) {
  assert(bytes.length >= 20 && bytes.toString('utf8', 0, 4) === 'glTF', 'invalid GLB magic')
  assert(bytes.readUInt32LE(4) === 2, 'GLB is not glTF 2.0')
  assert(bytes.readUInt32LE(8) === bytes.length, 'GLB declared length drifted')
  const jsonLength = bytes.readUInt32LE(12)
  assert(bytes.readUInt32LE(16) === 0x4e4f534a, 'GLB first chunk is not JSON')
  const jsonEnd = 20 + jsonLength
  const binLength = bytes.readUInt32LE(jsonEnd)
  assert(bytes.readUInt32LE(jsonEnd + 4) === 0x004e4942, 'GLB second chunk is not BIN')
  assert(jsonEnd + 8 + binLength === bytes.length, 'GLB BIN length drifted')
  return {
    json: JSON.parse(bytes.subarray(20, jsonEnd).toString('utf8').trimEnd()),
    bin: bytes.subarray(jsonEnd + 8),
  }
}

function verifyGlbRuntime(gltf, metadata) {
  assert(gltf.asset?.version === '2.0', 'exported asset is not glTF 2.0')
  const runtimeNodes = gltf.nodes.filter((node) => node.extras?.sculptRuntime?.schema_version === 'WeaponryThreeJsKnifeActionRuntime@1')
  assert(runtimeNodes.length === 1, 'exported GLB must contain one action-ready root')
  const pivots = new Set(gltf.nodes.map((node) => node.extras?.pivot_id).filter(Boolean))
  const sockets = new Set(gltf.nodes.map((node) => node.extras?.socket_id).filter(Boolean))
  assert(metadata.pivot_ids.every((pivotId) => pivots.has(pivotId)), 'exported pivot coverage is incomplete')
  assert(metadata.socket_ids.every((socketId) => sockets.has(socketId)), 'exported socket coverage is incomplete')
}

async function bundleStandaloneLoader(entry) {
  const output = await viteBuild({
    configFile: false,
    root: packageRoot,
    logLevel: 'silent',
    build: {
      write: false,
      minify: 'esbuild',
      sourcemap: false,
      lib: { entry, formats: ['es'], fileName: () => 'load-knife-delivery.standalone.mjs' },
      rollupOptions: { output: { inlineDynamicImports: true } },
    },
  })
  const outputs = Array.isArray(output) ? output.flatMap((item) => item.output) : output.output
  const entryChunk = outputs.find((item) => item.type === 'chunk' && item.isEntry)
  assert(entryChunk?.code, 'standalone delivery loader was not bundled')
  return Buffer.from(entryChunk.code)
}

async function treeDigest(root) {
  const files = []
  async function visit(current) {
    for (const entry of await readdir(current, { withFileTypes: true })) {
      const path = join(current, entry.name)
      if (entry.isDirectory()) await visit(path)
      else if (entry.isFile()) files.push(path)
    }
  }
  await visit(root)
  files.sort()
  const digest = createHash('sha256')
  for (const path of files) {
    const bytes = await readFile(path)
    const relative = path.slice(root.length + 1).replaceAll('\\', '/')
    digest.update(relative).update('\0').update(String(bytes.length)).update('\0').update(bytes).update('\0')
  }
  return { sha256: digest.digest('hex'), file_count: files.length }
}

function verifyGeometryFrozen(source, delivery, partIds) {
  for (const partId of partIds) {
    const sourcePrimitives = primitivesForPart(source.json, partId)
    const deliveryPrimitives = primitivesForPart(delivery.json, partId)
    assert(sourcePrimitives.length === deliveryPrimitives.length, `primitive count changed for ${partId}`)
    for (let index = 0; index < sourcePrimitives.length; index += 1) {
      const before = sourcePrimitives[index]
      const after = deliveryPrimitives[index]
      assert((before.mode ?? 4) === (after.mode ?? 4), `primitive mode changed for ${partId}`)
      const beforeAttributes = Object.keys(before.attributes ?? {}).sort()
      const afterAttributes = Object.keys(after.attributes ?? {}).sort()
      assert(beforeAttributes.join('\u0000') === afterAttributes.join('\u0000'), `attributes changed for ${partId}`)
      for (const attribute of beforeAttributes) {
        const beforePayload = accessorPayload(source, before.attributes[attribute])
        const afterPayload = accessorPayload(delivery, after.attributes[attribute])
        assert(beforePayload === afterPayload, `${attribute} accessor bytes changed for ${partId}`)
      }
      assert(accessorPayload(source, before.indices) === accessorPayload(delivery, after.indices), `index bytes changed for ${partId}`)
      assert(
        canonicalJson(source.json.materials?.[before.material] ?? null) === canonicalJson(delivery.json.materials?.[after.material] ?? null),
        `material changed for ${partId}`,
      )
    }
  }
}

function primitivesForPart(gltf, partId) {
  const node = gltf.nodes.find((candidate) => candidate.name === `knife-part:${partId}`)
  assert(node && Number.isInteger(node.mesh), `mesh node missing for ${partId}`)
  const mesh = gltf.meshes[node.mesh]
  assert(mesh && Array.isArray(mesh.primitives) && mesh.primitives.length > 0, `mesh primitives missing for ${partId}`)
  return mesh.primitives
}

function accessorPayload(document, accessorIndex) {
  if (accessorIndex === undefined || accessorIndex === null) return 'null'
  const accessor = document.json.accessors[accessorIndex]
  assert(accessor && Number.isInteger(accessor.bufferView), `accessor ${accessorIndex} is invalid`)
  const view = document.json.bufferViews[accessor.bufferView]
  assert(view && (view.buffer ?? 0) === 0, `accessor ${accessorIndex} does not use the GLB buffer`)
  const componentBytes = new Map([[5120, 1], [5121, 1], [5122, 2], [5123, 2], [5125, 4], [5126, 4]]).get(accessor.componentType)
  const componentCount = new Map([['SCALAR', 1], ['VEC2', 2], ['VEC3', 3], ['VEC4', 4], ['MAT2', 4], ['MAT3', 9], ['MAT4', 16]]).get(accessor.type)
  assert(componentBytes && componentCount, `accessor ${accessorIndex} component shape is invalid`)
  const elementBytes = componentBytes * componentCount
  const stride = view.byteStride ?? elementBytes
  const start = (view.byteOffset ?? 0) + (accessor.byteOffset ?? 0)
  const chunks = []
  for (let index = 0; index < accessor.count; index += 1) {
    chunks.push(document.bin.subarray(start + index * stride, start + index * stride + elementBytes))
  }
  const metadata = {
    componentType: accessor.componentType,
    type: accessor.type,
    count: accessor.count,
    normalized: accessor.normalized ?? false,
    min: accessor.min ?? null,
    max: accessor.max ?? null,
  }
  return `${canonicalJson(metadata)}:${sha256(Buffer.concat(chunks))}`
}

function canonicalJson(value) {
  if (value === null) return 'null'
  if (typeof value === 'string' || typeof value === 'boolean') return JSON.stringify(value)
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error('canonical JSON cannot encode non-finite numbers')
    return Object.is(value, -0) ? '0' : JSON.stringify(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(',')}}`
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex')
}

function assert(condition, message) {
  if (!condition) throw new Error(`DRAGONFANG_R8_DELIVERY_INVALID: ${message}`)
}
