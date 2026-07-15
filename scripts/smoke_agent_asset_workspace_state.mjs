#!/usr/bin/env node
import { buildSync } from 'esbuild'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = resolve(fileURLToPath(new URL('..', import.meta.url)))
const out = await mkdtemp(join(tmpdir(), 'forgecad-f010-'))
try {
  const outfile = join(out, 'smoke.mjs')
  buildSync({ entryPoints: [join(root, 'apps/desktop/src/features/cad-workbench/agentAssetWorkspaceState.smoke.ts')], bundle: true, platform: 'node', format: 'esm', outfile })
  const module = await import(pathToFileURL(outfile).href)
  module.runAgentAssetWorkspaceStateSmoke()
  console.log('F010 Agent asset workspace state smoke passed')
} finally { await rm(out, { recursive: true, force: true }) }
