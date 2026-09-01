#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'

import { GLTFExporter } from 'three/examples/jsm/exporters/GLTFExporter.js'

import { compileKnifeSceneProgram } from '../src/index.ts'

class NodeFileReader {
  result = null
  error = null
  onloadend = null
  onerror = null

  readAsArrayBuffer(blob) {
    blob.arrayBuffer().then(
      (value) => {
        this.result = value
        this.onloadend?.({ target: this })
      },
      (error) => {
        this.error = error
        this.onerror?.(error)
      },
    )
  }

  readAsDataURL(blob) {
    blob.arrayBuffer().then(
      (value) => {
        const mime = blob.type || 'application/octet-stream'
        this.result = `data:${mime};base64,${Buffer.from(value).toString('base64')}`
        this.onloadend?.({ target: this })
      },
      (error) => {
        this.error = error
        this.onerror?.(error)
      },
    )
  }
}

globalThis.FileReader ??= NodeFileReader

const [inputArg, outputArg] = process.argv.slice(2)
if (!inputArg || !outputArg) {
  console.error('usage: compile-program.mjs <KnifeSceneProgram.json> <output.glb>')
  process.exit(2)
}

const input = resolve(inputArg)
const output = resolve(outputArg)
const programBytes = await readFile(input)
const program = JSON.parse(programBytes.toString('utf8'))
const compiled = compileKnifeSceneProgram(program)

const exporter = new GLTFExporter()
const result = await exporter.parseAsync(compiled.group, {
  binary: true,
  onlyVisible: true,
  trs: false,
})
if (!(result instanceof ArrayBuffer)) throw new Error('GLTFExporter did not return binary GLB bytes')
const glb = Buffer.from(result)
await writeFile(output, glb)

console.log(JSON.stringify({
  schema_version: 'WeaponryThreeJsCompileReceipt@1',
  compiler: 'weaponry-threejs-knife-compiler@1',
  asset_id: program.asset_id,
  program_bytes_sha256: createHash('sha256').update(programBytes).digest('hex'),
  glb_sha256: createHash('sha256').update(glb).digest('hex'),
  glb_bytes: glb.length,
  triangles: compiled.triangle_count,
  longitudinal_segments: compiled.longitudinal_segments,
  part_ids: compiled.parts.map((part) => part.part_id),
  deterministic_fingerprint: compiled.deterministic_fingerprint,
  renderer_invoked: false,
  quality_status: 'NOT_RUN',
}))
