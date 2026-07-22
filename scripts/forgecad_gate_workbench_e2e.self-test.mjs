#!/usr/bin/env node

// Contract-only self-test for the T002 five-facet gate.  It uses injected
// result records and an occupied loopback port; it never starts the Agent,
// Vite, browser, packaged app, or a Provider.
import { createServer } from 'node:net'
import {
  buildGateReport,
  detectPackagedPortConflicts,
  errorCode,
  EXIT_CODES,
  stableErrorCode,
  stableProjectState,
} from './smoke_workbench_e2e_scenarios.mjs'

const originalPorts = process.env.FORGECAD_T002_PACKAGED_PORTS

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

function assertReportShape(report) {
  assert(report.schema_version === 'ForgeCADWorkbenchE2EGateReport@1', 'report schema must be stable')
  assert(Number.isInteger(report.exit_code), 'report must expose a numeric exit code')
  assert(typeof report.phase === 'string' && typeof report.subsystem === 'string', 'report must expose phase/subsystem')
  assert('stable_error_code' in report, 'report must expose stable_error_code')
  for (const facet of ['browser', 'renderer', 'quality', 'export']) {
    assert(report.facets[facet], `missing ${facet} facet`)
    assert(['passed', 'failed', 'not_run'].includes(report.facets[facet].status), `${facet} facet status is not stable`)
  }
  const serialized = JSON.stringify(report)
  for (const forbidden of ['prompt', 'body', 'secret', 'api_key', 'source_text']) {
    assert(!serialized.toLowerCase().includes(forbidden), `report leaked forbidden field: ${forbidden}`)
  }
}

const emptyProject = {
  project_id: 'prj_self_test',
  current_version_id: null,
  versions: [{ version_id: 'ver_concept_v1' }],
}
const emptyResponse = { error: { code: 'ACTIVE_DESIGN_NOT_FOUND', message: 'stable test error' } }
assert(errorCode(emptyResponse) === 'ACTIVE_DESIGN_NOT_FOUND', 'empty project error code must remain stable')
assert(stableProjectState(emptyProject) === stableProjectState({ ...emptyProject }), 'empty project state oracle must be deterministic')

const passed = buildGateReport([
  {
    id: 'T002-01-bootstrap-single-canvas',
    status: 'passed',
    phase: 'workbench_e2e',
    subsystem: 'desktop_workbench',
    stable_error_code: null,
    project_id: 'prj_self_test',
    assertions: ['empty_active_design_404', 'empty_active_design_no_side_effect'],
  },
])
assertReportShape(passed)
assert(passed.ok === false && passed.exit_code === EXIT_CODES.failed, 'partial injected run must fail closed')
assert(passed.facets.browser.status === 'not_run', 'partial run must not claim browser facet pass')
assert(passed.facets.quality.status === 'not_run', 'partial run must not claim quality facet pass')

const failed = buildGateReport([
  {
    id: 'T002-12-confirm-quality-export-reload',
    status: 'failed',
    phase: 'workbench_e2e',
    subsystem: 'quality_export',
    stable_error_code: 'QUALITY_ASSERTION_FAILED',
  },
])
assertReportShape(failed)
assert(failed.exit_code === EXIT_CODES.failed, 'failed scenario must use exit code 1')
assert(failed.facets.quality.status === 'failed', 'quality failure must stay in quality facet')
assert(failed.facets.export.status === 'failed', 'quality/export scenario failure must stay visible in export facet')
assert(stableErrorCode(new Error('timed out waiting for viewport')) === 'RENDERER_ASSERTION_FAILED', 'renderer timeout classification must be stable')
assert(stableErrorCode(new Error('quality check failed')) === 'QUALITY_ASSERTION_FAILED', 'quality classification must be stable')

const server = createServer(() => undefined)
await new Promise((resolve, reject) => {
  server.once('error', reject)
  server.listen(0, '127.0.0.1', resolve)
})
const port = server.address().port
process.env.FORGECAD_T002_PACKAGED_PORTS = String(port)
try {
  const occupied = await detectPackagedPortConflicts()
  assert(occupied.includes(port), 'self-test must detect an occupied packaged-main port')
} finally {
  await new Promise((resolve) => server.close(resolve))
  if (originalPorts === undefined) delete process.env.FORGECAD_T002_PACKAGED_PORTS
  else process.env.FORGECAD_T002_PACKAGED_PORTS = originalPorts
}

console.log(JSON.stringify({ schema_version: 'ForgeCADWorkbenchE2EGateSelfTest@1', ok: true, exit_code: EXIT_CODES.passed }))
