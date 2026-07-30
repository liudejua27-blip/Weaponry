#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const DESKTOP_SOURCE = join(ROOT, 'apps', 'desktop', 'src')
const output = await mkdtemp(join(tmpdir(), 'forgecad-surface-adornment-adapter-'))

try {
  const bundle = join(output, 'surface-adornment-adapter-smoke.mjs')
  const result = spawnSync(join(ROOT, 'node_modules', '.bin', 'esbuild'), [
    join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'surfaceAdornmentAdapterLoader.smoke.ts'),
    '--bundle',
    '--platform=node',
    '--format=esm',
    `--outfile=${bundle}`,
  ], { cwd: ROOT, encoding: 'utf8' })
  if (result.status !== 0) {
    process.stderr.write(result.stdout)
    process.stderr.write(result.stderr)
    process.exit(result.status ?? 1)
  }
  const module = await import(pathToFileURL(bundle).href)
  await module.runSurfaceAdornmentAdapterLoaderSmoke()
  console.log('Surface adornment adapter loader smoke passed')
} finally {
  await rm(output, { recursive: true, force: true })
}
