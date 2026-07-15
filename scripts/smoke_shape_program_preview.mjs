#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtemp, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const SOURCE = join(ROOT, 'apps', 'desktop', 'src')
const output = await mkdtemp(join(tmpdir(), 'forgecad-g816-preview-'))

try {
  const result = spawnSync(join(ROOT, 'node_modules', '.bin', 'tsc'), [
    '--target', 'ES2022', '--module', 'ESNext', '--moduleResolution', 'Bundler', '--strict', '--skipLibCheck',
    '--outDir', output, '--rootDir', SOURCE,
    join(SOURCE, 'features', 'cad-workbench', 'shapeProgramPreview.ts'),
    join(SOURCE, 'features', 'cad-workbench', 'shapeProgramPreview.smoke.ts'),
  ], { cwd: ROOT, encoding: 'utf8' })
  if (result.status !== 0) {
    process.stderr.write(result.stdout)
    process.stderr.write(result.stderr)
    process.exit(result.status ?? 1)
  }
  await symlink(join(ROOT, 'node_modules'), join(output, 'node_modules'), 'junction')
  await writeFile(join(output, 'package.json'), '{"type":"module"}\n', 'utf8')
  const module = await import(pathToFileURL(join(output, 'features', 'cad-workbench', 'shapeProgramPreview.smoke.js')).href)
  module.runShapeProgramPreviewSmoke()
  console.log('G816 ShapeProgram presentation preview smoke passed: full primitives, display-only bevel, PBR presentation and visibility state')
} finally {
  await rm(output, { recursive: true, force: true })
}
