#!/usr/bin/env node
import { buildSync } from 'esbuild'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
const root = resolve(fileURLToPath(new URL('..', import.meta.url))), out = await mkdtemp(join(tmpdir(), 'forgecad-f021-'))
try { const outfile = join(out, 'smoke.mjs'); buildSync({ entryPoints: [join(root, 'apps/desktop/src/features/cad-workbench/componentCatalogPresentationState.smoke.ts')], bundle: true, platform: 'node', format: 'esm', outfile }); const mod = await import(pathToFileURL(outfile).href); mod.runComponentCatalogPresentationStateSmoke(); console.log('F021 component catalog presentation state smoke passed') } finally { await rm(out, { recursive: true, force: true }) }
