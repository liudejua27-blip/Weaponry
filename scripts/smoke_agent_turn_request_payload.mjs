#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtemp, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const SOURCE = join(ROOT, 'apps', 'desktop', 'src')
const output = await mkdtemp(join(tmpdir(), 'forgecad-u002-author-transport-'))

try {
  const result = spawnSync(join(ROOT, 'node_modules', '.bin', 'tsc'), [
    '--target', 'ES2022',
    '--module', 'ESNext',
    '--moduleResolution', 'Bundler',
    '--strict',
    '--skipLibCheck',
    '--lib', 'ES2022,DOM,DOM.Iterable',
    '--rootDir', SOURCE,
    '--outDir', output,
    join(SOURCE, 'features', 'cad-workbench', 'agentTurnRequestPayload.ts'),
    join(SOURCE, 'features', 'cad-workbench', 'agentTurnRequestPayload.smoke.ts'),
    join(SOURCE, 'shared', 'types.ts'),
    join(SOURCE, 'shared', 'generated', 'api-types.ts'),
    join(SOURCE, 'shared', 'tauri', 'visionEvidence.ts'),
  ], { cwd: ROOT, encoding: 'utf8' })
  if (result.status !== 0) {
    process.stderr.write(result.stdout)
    process.stderr.write(result.stderr)
    process.exit(result.status ?? 1)
  }
  await symlink(join(ROOT, 'node_modules'), join(output, 'node_modules'), 'junction')
  await writeFile(join(output, 'package.json'), '{"type":"module"}\n', 'utf8')
  const module = await import(pathToFileURL(join(output, 'features', 'cad-workbench', 'agentTurnRequestPayload.smoke.js')).href)
  module.runAgentTurnRequestPayloadSmoke()
  console.log('U002 universal author transport payload smoke passed')
} finally {
  await rm(output, { recursive: true, force: true })
}
