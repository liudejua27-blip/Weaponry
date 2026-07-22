#!/usr/bin/env node
// Execute the R007B API projection smoke, rather than relying on typecheck to
// prove that the Rust-provided source/result provenance survives into the UI.
import { buildSync } from 'esbuild'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const output = await mkdtemp(join(tmpdir(), 'forgecad-r007b-api-projection-'))

try {
  const outfile = join(output, 'r007b-api-projection.mjs')
  buildSync({
    entryPoints: [join(ROOT, 'apps', 'desktop', 'src', 'shared', 'api', 'forgeApi.r007b.smoke.ts')],
    bundle: true,
    format: 'esm',
    platform: 'node',
    target: 'node20',
    outfile,
    logLevel: 'silent',
  })
  const smoke = await import(pathToFileURL(outfile).href)
  smoke.runR007BForgeApiProjectionSmoke()
  console.log(JSON.stringify({
    schema_version: 'R007BReferenceApiProjectionSmoke@1',
    status: 'pass',
    assertions: ['runtime_projection', 'analysis_identity', 'distinct_result_glb_sha256', 'no_client_similarity_score'],
  }))
} finally {
  await rm(output, { recursive: true, force: true })
}
