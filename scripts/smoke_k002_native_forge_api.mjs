#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const desktopSource = join(ROOT, 'apps', 'desktop', 'src')
const forgeApiPath = join(desktopSource, 'shared', 'api', 'forgeApi.ts')
const transportPath = join(desktopSource, 'shared', 'api', 'appServerTransport.ts')
const output = await mkdtemp(join(tmpdir(), 'forgecad-k002-native-forge-api-'))

const [forgeApiSource, transportSource] = await Promise.all([
  readFile(forgeApiPath, 'utf8'),
  readFile(transportPath, 'utf8'),
])

for (const [pattern, label] of [
  [/localStorage/, 'persistent cancellation state'],
  [/sessionStorage/, 'session-persistent cancellation state'],
  [/product-tools\/execute/, 'frontend Product Tool execution access'],
  [/reasoning_content\s*:/, 'reasoning content projection'],
]) {
  if (pattern.test(forgeApiSource)) throw new Error(`ForgeApi contains forbidden ${label}`)
}
if (transportSource.includes("| 'product-tools/execute'")) {
  throw new Error('NativeAgentMethod exposes product-tools/execute to React')
}
for (const required of [
  "nativeRequest<unknown>('thread/create'",
  "nativeRequest<unknown>('turn/start'",
  "nativeRequest<unknown>('item/list'",
  "nativeRequest<unknown>('approval/resolve'",
  "nativeRequest<unknown>('provider/check'",
  "notification.method !== 'item/updated'",
  'private readonly nativeTurnCancellations = new Map',
  'private readonly nativeProviderCancellations = new Map',
]) {
  if (!forgeApiSource.includes(required)) throw new Error(`ForgeApi is missing K002 boundary: ${required}`)
}
if (!transportSource.includes('export function isNativeDesktopRuntime(): boolean')) {
  throw new Error('transport does not export a read-only native desktop runtime decision')
}

try {
  const result = spawnSync(
    join(ROOT, 'node_modules', '.bin', 'tsc'),
    [
      '--target', 'ES2022',
      '--module', 'ESNext',
      '--moduleResolution', 'Bundler',
      '--strict',
      '--skipLibCheck',
      '--types', 'vite/client',
      '--outDir', output,
      '--rootDir', desktopSource,
      join(desktopSource, 'shared', 'api', 'appServerProtocol.ts'),
      join(desktopSource, 'shared', 'api', 'appServerTransport.ts'),
      join(desktopSource, 'shared', 'api', 'forgeApi.ts'),
      join(desktopSource, 'shared', 'api', 'forgeApi.k002.smoke.ts'),
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
  const module = await import(pathToFileURL(join(output, 'shared', 'api', 'forgeApi.k002.smoke.js')).href)
  await module.runK002NativeForgeApiSmoke()
  console.log('K002 native ForgeApi smoke passed')
} finally {
  await rm(output, { recursive: true, force: true })
}
