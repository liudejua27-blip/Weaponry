#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtemp, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const SOURCE = join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench')
const output = await mkdtemp(join(tmpdir(), 'forgecad-u004-workbench-pbr-capture-'))

try {
  const result = spawnSync(join(ROOT, 'node_modules', '.bin', 'tsc'), [
    '--target', 'ES2022', '--module', 'ESNext', '--moduleResolution', 'Bundler', '--strict', '--skipLibCheck',
    '--outDir', output,
    join(SOURCE, 'workbenchPbrCapture.ts'),
    join(SOURCE, 'workbenchPbrCapture.smoke.ts'),
  ], { cwd: ROOT, encoding: 'utf8' })
  if (result.status !== 0) {
    process.stderr.write(result.stdout)
    process.stderr.write(result.stderr)
    process.exit(result.status ?? 1)
  }
  await symlink(join(ROOT, 'node_modules'), join(output, 'node_modules'), 'junction')
  await writeFile(join(output, 'package.json'), '{"type":"module"}\n', 'utf8')
  const module = await import(pathToFileURL(join(output, 'workbenchPbrCapture.smoke.js')).href)
  module.runWorkbenchPbrCaptureSmoke()
  console.log('U004 workbench PBR capture contract smoke passed')
} finally {
  await rm(output, { recursive: true, force: true })
}
