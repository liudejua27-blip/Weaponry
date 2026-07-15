#!/usr/bin/env node

import { readFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const SOURCE = join(ROOT, 'apps', 'desktop', 'src', 'features', 'cad-workbench')

const [panel, conceptHook, inspector, legacyNotice, exportDrawer, qualityDrawer, drawerStack] = await Promise.all([
  readFile(join(SOURCE, 'CadWorkbenchPanel.tsx'), 'utf8'),
  readFile(join(SOURCE, 'useConceptWorkbench.ts'), 'utf8'),
  readFile(join(SOURCE, 'WorkbenchInspectorRail.tsx'), 'utf8'),
  readFile(join(SOURCE, 'LegacyCompatibilityNotice.tsx'), 'utf8'),
  readFile(join(SOURCE, 'ExportDrawer.tsx'), 'utf8'),
  readFile(join(SOURCE, 'QualityDrawer.tsx'), 'utf8'),
  readFile(join(SOURCE, 'WorkbenchDrawerStack.tsx'), 'utf8'),
])

assert((panel.match(/<ModuleGraphViewport/g) ?? []).length === 1, 'workbench must keep exactly one viewport component')
assert(panel.split('\n').length < 2200, 'CadWorkbenchPanel must remain below the F025 responsibility budget')
for (const forbidden of [
  'concept.planBrief(',
  'concept.planChange(',
  'concept.createExport(',
  'concept.runQualityInspection(',
  'concept.previewNodeTransform(',
  'concept.previewModuleReplacement(',
]) {
  assert(!panel.includes(forbidden), `Agent orchestration still calls legacy command: ${forbidden}`)
}

assert(conceptHook.includes('loadLegacyDetails = legacyDetailsEnabledRef.current'), 'legacy detail reads must be gated')
assert(conceptHook.includes('if (requestId !== loadProjectRequestRef.current) return'), 'late legacy reads must be rejected')
assert(conceptHook.includes('loadProjectRequestRef.current += 1'), 'closing legacy details must invalidate in-flight reads')
assert(legacyNotice.includes('查看旧版只读信息') && inspector.includes('if (!legacyDetailsOpen) return null'), 'legacy details must require explicit entry')
assert(inspector.includes('Graph Inspector · 只读') && inspector.includes('旧参数 · 只读'), 'legacy Graph and parameters must be read-only')
assert(inspector.includes('此处不创建新导出'), 'legacy surface must not create a historical export')

for (const source of [exportDrawer, qualityDrawer, drawerStack]) {
  for (const forbidden of ['onRunLegacyInspection', 'onFocusLegacyFinding', 'onPurposeChange', 'onExport:', 'ComponentDrawer']) {
    assert(!source.includes(forbidden), `Agent drawer stack leaked legacy responsibility: ${forbidden}`)
  }
}
for (const forbidden of ['SOURCE ZIP', 'OBJ 模型', '导出当前版本']) {
  assert(!exportDrawer.includes(forbidden), `Agent export drawer exposed legacy format: ${forbidden}`)
}

console.log(JSON.stringify({
  ok: true,
  task: 'FGC-F025',
  panel_lines: panel.split('\n').length,
  assertions: [
    'single_viewport_component',
    'parent_responsibility_budget',
    'no_agent_to_legacy_commands',
    'explicit_legacy_read_gate',
    'late_response_invalidation',
    'readonly_graph_parameters_export',
    'agent_only_quality_export_drawers',
  ],
}, null, 2))

function assert(condition, message) {
  if (!condition) throw new Error(message)
}
