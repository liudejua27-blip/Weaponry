#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const SOURCE = join(ROOT, 'apps', 'desktop', 'src', 'shared', 'api')
const output = await mkdtemp(join(tmpdir(), 'forgecad-c111b-webgl-qa-logic-'))

try {
  const result = spawnSync(join(ROOT, 'node_modules', '.bin', 'tsc'), [
    '--target', 'ES2022',
    '--module', 'ESNext',
    '--moduleResolution', 'Bundler',
    '--strict',
    '--skipLibCheck',
    '--outDir', output,
    '--rootDir', SOURCE,
    join(SOURCE, 'packagedC111BWebglQaLogic.ts'),
    join(SOURCE, 'packagedC111BWebglQaLogic.smoke.ts'),
  ], { cwd: ROOT, encoding: 'utf8' })
  if (result.status !== 0) {
    process.stderr.write(result.stdout)
    process.stderr.write(result.stderr)
    process.exit(result.status ?? 1)
  }
  await writeFile(join(output, 'package.json'), '{"type":"module"}\n', 'utf8')
  const module = await import(pathToFileURL(join(output, 'packagedC111BWebglQaLogic.smoke.js')).href)
  await module.runPackagedC111bWebglQaLogicSmoke()
  console.log('C111B packaged WebGL QA logic smoke passed')
} finally {
  await rm(output, { recursive: true, force: true })
}
