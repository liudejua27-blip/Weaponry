#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const DESKTOP_SOURCE = join(ROOT, 'apps', 'desktop', 'src')
const output = await mkdtemp(join(tmpdir(), 'forgecad-f002-conversation-'))

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
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'AgentStepItem.tsx'),
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'AgentConversation.tsx'),
      join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'AgentConversation.smoke.tsx'),
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
  const module = await import(pathToFileURL(join(output, 'features', 'cad-workbench', 'AgentConversation.smoke.js')).href)
  module.runAgentConversationSmoke()
  const [conversationSource, conceptHookSource, panelSource] = await Promise.all([
    readFile(join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'AgentConversation.tsx'), 'utf8'),
    readFile(join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'useConceptWorkbench.ts'), 'utf8'),
    readFile(join(DESKTOP_SOURCE, 'features', 'cad-workbench', 'CadWorkbenchPanel.tsx'), 'utf8'),
  ])
  for (const [label, source] of [
    ['AgentConversation', conversationSource],
    ['useConceptWorkbench', conceptHookSource],
    ['CadWorkbenchPanel', panelSource],
  ]) {
    for (const forbidden of ['准备展示组件', 'initializeCurrentProject', 'initializeConceptWorkbench']) {
      if (source.includes(forbidden)) throw new Error(`${label} still exposes the legacy empty-project initializer: ${forbidden}`)
    }
  }
  if (
    !conversationSource.includes('data-testid="agent-empty-project"')
    || !panelSource.includes('projectIsEmpty={projectIsEmpty}')
    || !panelSource.includes("activeDesignSnapshot?.active_design.source === 'agent_asset'")
    || !panelSource.includes('!projectHasActiveAgentSnapshot')
  ) {
    throw new Error('empty Project must render the direct Agent first-asset state')
  }
  console.log('F002 AgentConversation component smoke passed')
} finally {
  await rm(output, { recursive: true, force: true })
}
