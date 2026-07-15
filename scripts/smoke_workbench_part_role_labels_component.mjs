#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtemp, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const DESKTOP_SOURCE = join(ROOT, 'apps', 'desktop', 'src')
const output = await mkdtemp(join(tmpdir(), 'forgecad-c101-part-role-labels-'))

try {
  const result = spawnSync(
    join(ROOT, 'node_modules', '.bin', 'tsc'),
    [
      '--target', 'ES2022', '--module', 'ESNext', '--moduleResolution', 'Bundler',
      '--strict', '--skipLibCheck', '--esModuleInterop', '--allowSyntheticDefaultImports',
      '--outDir', output, '--rootDir', DESKTOP_SOURCE,
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'partRoleLabels.ts'),
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'partRoleLabels.smoke.ts'),
    ],
    { cwd: ROOT, encoding: 'utf8' },
  )
  if (result.status !== 0) {
    process.stderr.write(result.stdout)
    process.stderr.write(result.stderr)
    process.exit(result.status ?? 1)
  }
  await symlink(join(ROOT, 'node_modules'), join(output, 'node_modules'), 'junction')
  await writeFile(join(output, 'package.json'), '{"type":"module"}\n', 'utf8')
  const module = await import(pathToFileURL(join(output, 'features', 'cad-workbench', 'partRoleLabels.smoke.js')).href)
  module.runPartRoleLabelsSmoke()
  console.log('C101 part-role Chinese label smoke passed')
} finally {
  await rm(output, { recursive: true, force: true })
}
