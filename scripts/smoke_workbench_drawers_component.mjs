#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtemp, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const DESKTOP_SOURCE = join(ROOT, 'apps', 'desktop', 'src')
const output = await mkdtemp(join(tmpdir(), 'forgecad-f004-drawers-'))

try {
  const result = spawnSync(
    join(ROOT, 'node_modules', '.bin', 'tsc'),
    [
      '--target', 'ES2022',
      '--module', 'ESNext',
      '--moduleResolution', 'Bundler',
      '--jsx', 'react-jsx',
      '--strict',
      '--skipLibCheck',
      '--esModuleInterop',
      '--allowSyntheticDefaultImports',
      '--outDir', output,
      '--rootDir', DESKTOP_SOURCE,
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'ComponentDrawer.tsx'),
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'MaterialDrawer.tsx'),
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'QualityDrawer.tsx'),
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'ExportDrawer.tsx'),
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'WorkbenchDrawers.smoke.tsx'),
      join(DESKTOP_SOURCE, 'shared', 'types.ts'),
      join(DESKTOP_SOURCE, 'shared', 'generated', 'api-types.ts'),
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
  const module = await import(pathToFileURL(join(output, 'features', 'cad-workbench', 'WorkbenchDrawers.smoke.js')).href)
  module.runWorkbenchDrawersSmoke()
  console.log('F004 workbench drawer component smoke passed')
} finally {
  await rm(output, { recursive: true, force: true })
}
