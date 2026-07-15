#!/usr/bin/env node
import { readFile } from 'node:fs/promises'
import { spawnSync } from 'node:child_process'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const panelPath = join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench', 'CadWorkbenchPanel.tsx')
const cssPath = join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench', 'cad-workbench.css')
const lifecyclePath = join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench', 'useWorkbenchLifecycle.ts')
const componentPaths = [
  join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench', 'AgentConversation.tsx'),
  join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench', 'AgentSelectionCard.tsx'),
  join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench', 'ComponentDrawer.tsx'),
  join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench', 'MaterialDrawer.tsx'),
  join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench', 'QualityDrawer.tsx'),
  join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench', 'ExportDrawer.tsx'),
]

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

const componentSmoke = spawnSync(process.execPath, [join(ROOT, 'scripts', 'smoke_workbench_drawers_component.mjs')], {
  cwd: ROOT,
  encoding: 'utf8',
})
if (componentSmoke.status !== 0) {
  process.stderr.write(componentSmoke.stdout ?? '')
  process.stderr.write(componentSmoke.stderr ?? '')
  process.exit(componentSmoke.status ?? 1)
}

const [panel, css, lifecycle, ...components] = await Promise.all([
  readFile(panelPath, 'utf8'),
  readFile(cssPath, 'utf8'),
  readFile(lifecyclePath, 'utf8'),
  ...componentPaths.map((path) => readFile(path, 'utf8')),
])

assert(css.includes('FGC-F006 accessibility baseline'), 'CSS must declare the F006 accessibility baseline')
assert(css.includes('min-width: 1180px') && css.includes('min-height: 760px'), 'workbench must declare the minimum supported viewport')
assert(css.includes('min-height: 40px'), 'primary actions must have a 40px target')
assert(css.includes('component-library-resize-handle:focus-visible'), 'resize handle must have a visible keyboard focus state')
assert(!/font-size:\s*(?:[0-9]|10)px/.test(css), 'user-facing workbench CSS must not contain text below 11px')

assert(panel.includes('drawerFocusRef'), 'workbench must keep a DOM-only drawer focus reference')
assert(panel.includes("event.key === 'Escape'"), 'workbench must close drawers with Escape')
assert(panel.includes('focusInitialControl'), 'workbench must focus the first control when a drawer opens')
assert(lifecycle.includes('restoreDrawerFocus') && lifecycle.includes('drawerTriggerRef'), 'workbench lifecycle must return focus to the drawer trigger')
assert(panel.includes('onResizeKeyDown'), 'component drawer resize must have a keyboard path')
assert(panel.includes('aria-live="polite"'), 'workbench status must use aria-live')

const joinedComponents = components.join('\n')
assert(joinedComponents.includes('role="dialog"'), 'drawer components must expose dialog semantics')
assert(joinedComponents.includes('data-dialog-initial-focus="true"'), 'drawer close controls must be initial focus targets')
assert(joinedComponents.includes('aria-pressed'), 'choice controls must expose pressed state')
assert(joinedComponents.includes('aria-label'), 'component actions must expose Chinese accessible labels')

console.log('F006 workbench accessibility smoke passed')
