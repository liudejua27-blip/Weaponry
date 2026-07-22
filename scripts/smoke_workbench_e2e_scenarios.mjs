#!/usr/bin/env node

// FGC-T002: split the broad workbench regression into readable, named browser
// scenarios. This is a compatibility workbench regression: the deterministic
// local Planner is expected to fail the V003 decision contract visibly and
// without a Snapshot write. Formal V003 generation is proven separately by
// the Rust app-server + rendered Playwright fixture gate reported below.
import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'
import {
  agentGeometryTimeoutMs,
  assertGeometryCompileReadbackQuality,
  legacyLifecycleTestOracleEnvironment,
  inspectCompatHttpRequest,
  waitForCompatHttpResponse,
} from './workbench_agent_blockout_test_helper.mjs'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT_DIR = join(ROOT, 'output', 'playwright', 'fgt002-scenarios')
const TIMEOUT_MS = 20_000
const API_TIMEOUT_MS = TIMEOUT_MS
const RUNTIME_START_TIMEOUT_MS = 45_000
const RUNTIME_STOP_TIMEOUT_MS = 15_000
const EXIT_CODES = Object.freeze({ passed: 0, failed: 1, blocked: 2, internal: 3 })
const PACKAGED_PORT_ENV = 'FORGECAD_T002_PACKAGED_PORTS'
const SCENARIO_FILTER_ENV = 'FORGECAD_T002_SCENARIO'
const DEFAULT_PACKAGED_PORTS = [8000]
const EXPECTED_SCENARIO_COUNT = 14
const SCENARIO_FILTER = process.env[SCENARIO_FILTER_ENV]?.trim() || null

// The browser scenario body deliberately keeps its historical local names.
// These slots are replaced for each isolated scenario and never shared across
// scenario lifetimes.
let context = null
let agentBaseUrl = null
let viteBaseUrl = null
let projectId = null
let activeRuntime = null
let shutdownRequested = false

async function main() {
  installShutdownCleanup()
  const results = []
  await mkdir(OUTPUT_DIR, { recursive: true })

  const occupiedPackagedPorts = await detectPackagedPortConflicts()
  if (occupiedPackagedPorts.length > 0) {
    const report = buildGateReport(results, {
      phase: 'preflight',
      subsystem: 'port_isolation',
      stable_error_code: 'PACKAGED_PORT_IN_USE',
      exit_code: EXIT_CODES.blocked,
      run_status: 'blocked',
      blocked_ports: occupiedPackagedPorts,
    })
    await writeGateReport(report)
    console.error(JSON.stringify(report, null, 2))
    process.exitCode = EXIT_CODES.blocked
    return
  }

  try {
    await runScenario(results, 'T002-01-bootstrap-single-canvas', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = await waitForProjectId(agentBaseUrl)
        const before = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}`)
        const firstSnapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        const secondSnapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(firstSnapshot.status === 404, 'empty project active-design must return 404')
        assert(errorCode(firstSnapshot.body) === 'ACTIVE_DESIGN_NOT_FOUND', 'empty project must expose stable ACTIVE_DESIGN_NOT_FOUND')
        assert(secondSnapshot.status === 404, 'repeated empty project active-design read must remain 404')
        assert(errorCode(secondSnapshot.body) === 'ACTIVE_DESIGN_NOT_FOUND', 'repeated empty project read must keep stable error code')
        const after = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}`)
        assert(stableProjectState(before) === stableProjectState(after), 'empty active-design GET must not mutate the project')
        assert(await page.locator('.weapon-viewport canvas').count() === 1, 'workbench must have one WebGL canvas')
        assert(await page.getByLabel('设计需求', { exact: true }).count() === 1, 'Agent input must be present')
        return evidence(projectId, firstSnapshot, ['initial_workbench', 'single_canvas', 'agent_input', 'empty_active_design_404', 'empty_active_design_no_side_effect'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-02-legacy-explicit-handoff', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        const notice = page.getByLabel('旧版设计转换')
        if (await notice.count() === 1 && await notice.isVisible()) {
          await notice.getByRole('button', { name: '让 Agent 重建可编辑资产', exact: true }).click()
          await page.getByText(/已准备 legacy 只读设计的 Agent 重建输入/).waitFor({ timeout: TIMEOUT_MS })
        }
        const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(snapshot.body?.active_design?.source !== 'agent_asset', 'legacy handoff must not activate an Agent asset')
        return evidence(projectId, snapshot, ['explicit_legacy_handoff', 'legacy_write_barrier'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-03-ambiguous-clarification-write-barrier', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await sendBrief(page, '设计一台能飞的无人机载具')
        const clarification = page.getByLabel('需要确认设计类别')
        await clarification.waitFor({ timeout: TIMEOUT_MS })
        await assertText(clarification, ['先确认设计对象', '汽车与地面载具', '飞机与航空器'])
        const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(snapshot.body?.active_design?.source !== 'agent_asset', 'ambiguous input must not create an Agent asset')
        assert((await page.locator('.f026-agent-timeline').innerText()).includes('同时接近多个方向'), 'clarification must explain ambiguity')
        await clarification.getByRole('button', { name: '汽车与地面载具', exact: true }).click()
        await assertCompatibilityV003Rejection(page, agentBaseUrl, projectId, snapshot, 'T002 clarification continuation')
        return evidence(projectId, snapshot, ['one_question', 'zero_asset_write', 'choice_rejects_legacy_planner', 'compatibility_v003_contract_failure'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-03b-scope-stop-before-direction', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await sendBrief(page, '请设计一把现实枪械，并给出加工图纸和制造尺寸。')
        const stop = page.getByLabel('当前请求超出概念范围')
        await stop.waitFor({ timeout: TIMEOUT_MS })
        await assertText(stop, ['请换一种外观创意描述', '未发送给模型', '没有创建 3D 模型、版本或导出'])
        assert(await page.getByLabel('Agent 完整外观方向').count() === 0, 'scope stop must not show directions')
        const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(snapshot.body?.active_design?.source !== 'agent_asset', 'scope stop must not create an Agent asset')
        return evidence(projectId, snapshot, ['local_scope_stop', 'no_direction_card', 'zero_asset_write'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-04b-single-turn-action-loop-no-write', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        const before = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        let automaticDirectionPreviewRequests = 0
        page.on('request', (request) => {
          const observed = inspectCompatHttpRequest(request)
          if (observed?.method === 'POST' && observed.path === '/api/v1/agent/blockouts:concept-preview') automaticDirectionPreviewRequests += 1
        })
        await sendBrief(page, '设计一辆双座冰原探索汽车，完整封闭车身，深色耐候外观，作为非功能展示模型。')
        const beforeSelect = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(beforeSelect.body?.active_design?.asset_version_id === before.body?.active_design?.asset_version_id, 'Action Loop preview must not advance the active asset version')
        await assertCompatibilityV003Rejection(
          page,
          agentBaseUrl,
          projectId,
          before,
          'T002 Action Loop automatic result preview',
        )
        const timeline = page.locator('.f026-agent-timeline')
        await assertText(timeline, ['工具', 'tool_call', '结果', 'tool_result'])
        assert(automaticDirectionPreviewRequests === 0, 'the desktop must not splice three hidden concept-preview requests after the Turn')
        assert(await page.locator('img.agent-direction-concept-image').count() === 0, 'A004 direction cards use the server Action Loop instead of three automatic images')
        assert(await page.locator('.weapon-viewport canvas').count() === 1, 'Action Loop items must not create a second WebGL canvas')
        const after = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(after.body?.active_design?.asset_version_id === before.body?.active_design?.asset_version_id, 'Action Loop and selected blockout must not write an asset version')
        return evidence(projectId, after, ['single_turn_tool_lifecycle', 'no_hidden_direction_preview_requests', 'compatibility_v003_contract_failure', 'preview_no_version_write', 'single_canvas'])
      } finally { await page.close() }
    })

    for (const [scenarioId, brief, labels] of [
      ['T002-04-car-brief', '设计一辆双座冰原探索汽车，完整封闭车身，大轮胎，短前后悬，深色耐候外观。', ['car_direction']],
      ['T002-05-aircraft-brief', '设计一架紧凑的垂直起降概念飞机，宽机身、双短翼、深色舱罩，用于科幻救援展示。', ['aircraft_direction']],
      ['T002-06-robotic-arm-brief', '设计一台三关节维护机械臂，固定基座、两段连杆、旋转腕部和夹持末端。', ['robotic_arm_direction']],
      ['T002-07-future-prop-brief', '设计一个厚重、紧凑、非功能性的未来武器概念道具，用于游戏展示。', ['future_prop_direction', 'non_functional_scope']],
    ]) {
      await runScenario(results, scenarioId, async () => {
        const page = await openWorkbench(context, viteBaseUrl)
        try {
          projectId = projectId ?? await waitForProjectId(agentBaseUrl)
          const before = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
          await sendBrief(page, brief)
          await assertCompatibilityV003Rejection(
            page,
            agentBaseUrl,
            projectId,
            before,
            `${scenarioId} automatic result preview`,
          )
          const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
          assert(snapshot.body?.active_design?.source !== 'agent_asset', 'automatic result preview must remain preview-only')
          return evidence(projectId, snapshot, labels.concat(['compatibility_v003_contract_failure', 'preview_only']))
        } finally { await page.close() }
      })
    }

    await runScenario(results, 'T002-08-preview-does-not-write-version', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        const before = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        await sendBrief(page, '设计一辆双座冰原探索汽车，完整封闭车身，作为非功能展示模型。')
        await assertCompatibilityV003Rejection(
          page,
          agentBaseUrl,
          projectId,
          before,
          'T002 preview-only automatic result',
        )
        const after = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(after.body?.active_design?.asset_version_id === before.body?.active_design?.asset_version_id, 'preview must not advance asset version')
        assert(after.body?.preview === before.body?.preview || after.body?.preview === null, 'preview response must remain Snapshot-owned')
        return evidence(projectId, after, ['preview_no_version_write', 'snapshot_version_unchanged'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-09-commit-editable-agent-asset', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        const snapshot = await ensureCompatibilitySeedAgentAsset(page, agentBaseUrl, projectId, 'T002 asset seed')
        assert(snapshot.active_design?.source === 'agent_asset', 'commit must activate Agent asset')
        assert(snapshot.export?.source_version_id === snapshot.active_design?.asset_version_id, 'export source must follow active asset')
        return evidence(projectId, snapshot, ['compatibility_seed_not_v003', 'editable_asset_commit', 'export_snapshot_alignment'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-10-part-selection-and-material-zone', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await ensureCompatibilitySeedAgentAsset(page, agentBaseUrl, projectId, 'T002 material-zone setup')
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        // A selected display-only showcase detail deliberately has no editable
        // parameters.  This scenario exercises the bounded ChangeSet path, so
        // choose an actual server-declared adjustable part rather than relying
        // on list order across the four domain packs.
        const firstPart = page.getByLabel('分件候选').locator('.agent-segmentation-list button').filter({ hasText: '可调整' }).first()
        await firstPart.waitFor({ timeout: TIMEOUT_MS })
        const selectionResponse = waitForCompatHttpResponse(page, {
          method: 'POST',
          path: `/api/v1/projects/${projectId}/active-design:select`,
        })
        await firstPart.click()
        const selection = (await selectionResponse).body.json()
        assert(typeof selection.selected_part_id === 'string', 'selection response must include selected_part_id')
        assert(typeof selection.selected_material_zone_id === 'string', 'selection response must include the stable selected Material Zone')
        await page.getByLabel('添加风格、材质或参考').click()
        await page.getByRole('menuitem', { name: '选择材质', exact: true }).click()
        const materials = page.getByLabel('视觉材质目录')
        await materials.waitFor({ timeout: TIMEOUT_MS })
        const proposeResponse = waitForCompatHttpResponse(page, {
          method: 'POST',
          path: /\/api\/v1\/agent\/asset-versions\/[^/]+\/change-sets$/,
        })
        const previewGlbResponse = waitForCompatHttpResponse(page, {
          method: 'GET',
          path: /\/api\/v1\/agent\/change-sets\/[^/]+:preview\.glb$/,
          timeout: agentGeometryTimeoutMs(),
        })
        await materials.locator('.agent-material-preview-list').getByRole('button', { name: '拉丝铝', exact: true }).click()
        const proposed = await proposeResponse
        assert(proposed.ok, 'material ChangeSet proposal must succeed')
        const proposedBody = proposed.request.body.json()
        const materialOperation = proposedBody.operations?.[0]
        assert(materialOperation?.op === 'apply_material_preset', 'quick material action must propose apply_material_preset')
        assert(materialOperation.part_id === selection.selected_part_id, 'quick material action must target the Snapshot-selected part')
        assert(materialOperation.material_zone_id === selection.selected_material_zone_id, 'quick material action must target the Snapshot-selected Material Zone')
        assert(materialOperation.material_id === 'mat_aluminum', 'quick material action must preserve the chosen compatible preset id')
        const compiledPreview = await previewGlbResponse
        assert(compiledPreview.ok, 'compiled ChangeSet preview GLB request must succeed')
        const previewHeaders = compiledPreview.headers
        assert((previewHeaders['content-type'] ?? '').includes('model/gltf-binary'), 'preview must return binary GLB media type')
        assert(/^[a-f0-9]{64}$/.test(previewHeaders['x-forgecad-preview-glb-sha256'] ?? ''), 'preview must expose its GLB SHA256')
        assert(previewHeaders['x-forgecad-base-asset-version-id'] === selection.active_design?.asset_version_id, 'preview GLB must identify the active base asset')
        await page.waitForFunction(
          () => {
            const viewport = document.querySelector('.weapon-viewport')
            return viewport?.getAttribute('data-blockout-load-state') === 'ready'
              && viewport.getAttribute('data-blockout-glb-kind') === 'compiled_agent_preview_pbr'
              && viewport.getAttribute('data-blockout-render-source') === 'glb_pbr'
          },
          undefined,
          { timeout: agentGeometryTimeoutMs() },
        )
        const preview = page.getByLabel('可编辑资产修改预览')
        await preview.waitFor({ timeout: TIMEOUT_MS })
        const snapshot = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(snapshot.preview?.change_set_id, 'material preview must be persisted in Snapshot')
        const rejectResponse = waitForCompatHttpResponse(page, {
          method: 'POST',
          path: /\/api\/v1\/agent\/change-sets\/[^/]+:reject$/,
        })
        await preview.getByRole('button', { name: '取消修改', exact: true }).click()
        assert((await rejectResponse).ok, 'material ChangeSet rejection must succeed')
        await preview.waitFor({ state: 'detached', timeout: TIMEOUT_MS })
        const afterCancel = await waitForSnapshot(agentBaseUrl, projectId, (current) => current.preview == null)
        return evidence(projectId, afterCancel, ['compatibility_seed_not_v003', 'part_selection_snapshot', 'material_zone_action', 'compiled_pbr_preview', 'cancel_clears_preview'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-11-changeset-preview-cancel', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await ensureCompatibilitySeedAgentAsset(page, agentBaseUrl, projectId, 'T002 cancel setup')
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        await selectAdjustablePart(page, projectId)
        await page.getByLabel('部件可调参数').getByRole('button', { name: '减小 长度比例', exact: true }).click()
        const preview = page.getByLabel('可编辑资产修改预览')
        await preview.waitFor({ timeout: TIMEOUT_MS })
        const withPreview = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(withPreview.preview?.change_set_id, 'preview must be persisted in Snapshot')
        const rejectResponse = waitForCompatHttpResponse(page, {
          method: 'POST',
          path: /\/api\/v1\/agent\/change-sets\/[^/]+:reject$/,
        })
        await preview.getByRole('button', { name: '取消修改', exact: true }).click()
        assert((await rejectResponse).ok, 'changeset rejection request must succeed')
        await preview.waitFor({ state: 'detached', timeout: TIMEOUT_MS })
        const afterCancel = await waitForSnapshot(agentBaseUrl, projectId, (snapshot) => snapshot.preview == null)
        assert(afterCancel.preview === null, 'cancel must clear preview without creating a version')
        return evidence(projectId, afterCancel, ['compatibility_seed_not_v003', 'changeset_preview', 'cancel_clears_preview'])
      } finally { await page.close() }
    })

    await runScenario(results, 'T002-12-confirm-quality-export-reload', async () => {
      const page = await openWorkbench(context, viteBaseUrl)
      try {
        projectId = projectId ?? await waitForProjectId(agentBaseUrl)
        await ensureCompatibilitySeedAgentAsset(page, agentBaseUrl, projectId, 'T002 quality/export setup')
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        await selectAdjustablePart(page, projectId)
        const before = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        await page.getByLabel('部件可调参数').getByRole('button', { name: '增大 长度比例', exact: true }).click()
        const preview = page.getByLabel('可编辑资产修改预览')
        await preview.waitFor({ timeout: TIMEOUT_MS })
        await preview.getByRole('button', { name: '保留并创建新版本', exact: true }).click()
        await page.getByLabel('分件候选').getByText(/可编辑资产 v\d+/, { exact: false }).waitFor({ timeout: TIMEOUT_MS })
        const committed = await waitForSnapshot(agentBaseUrl, projectId, (snapshot) => snapshot.active_design?.asset_version_id !== before.active_design?.asset_version_id)
        assert(committed.active_design?.asset_version_id !== before.active_design?.asset_version_id, 'confirm must create a new immutable version')
        await page.getByRole('button', { name: '检查', exact: true }).click()
        const quality = page.locator('.quality-drawer')
        await quality.waitFor({ timeout: TIMEOUT_MS })
        await assertText(quality, ['模型检查', '当前版本'])
        const qualityResponse = waitForCompatHttpResponse(page, {
          method: 'POST',
          path: /\/api\/v1\/agent\/asset-versions\/[^/]+:quality$/,
          timeout: agentGeometryTimeoutMs(),
        }).then((response) => ({ response, error: null }), (error) => ({ response: null, error }))
        await quality.getByRole('button', { name: '检查当前 Agent 资产', exact: true }).click()
        const qualityOutcome = await qualityResponse
        if (qualityOutcome.error) {
          throw new Error(`quality check response timed out: ${qualityOutcome.error instanceof Error ? qualityOutcome.error.message : String(qualityOutcome.error)}`)
        }
        const qualityResult = qualityOutcome.response
        if (!qualityResult.ok) {
          throw new Error(`quality check request failed: ${qualityResult.status} ${qualityResult.body.text.slice(0, 2000)}`)
        }
        const qualityPayload = qualityResult.body.json()
        assertGeometryCompileReadbackQuality(qualityPayload, 'T002 Agent quality report')
        assert(['passed', 'warning'].includes(qualityPayload.status), `quality check returned unexpected status: ${JSON.stringify(qualityPayload)}`)
        await quality.locator('.drawer-heading strong').getByText(/^(通过|需复核)$/).waitFor({ timeout: TIMEOUT_MS })
        const afterQuality = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(qualityPayload.asset_version_id === afterQuality.active_design?.asset_version_id, 'quality response must reference the active version')
        assert(afterQuality.quality?.asset_version_id === afterQuality.active_design?.asset_version_id, 'quality must reference active version')
        assert(afterQuality.quality?.quality_report_id === qualityPayload.quality_report_id, 'Snapshot must reference this quality response')
        await quality.getByRole('button', { name: '关闭模型检查', exact: true }).click()
        await quality.waitFor({ state: 'detached', timeout: TIMEOUT_MS })
        await page.getByRole('button', { name: '导出', exact: true }).click()
        const exportDrawer = page.locator('.export-drawer')
        await exportDrawer.waitFor({ timeout: TIMEOUT_MS })
        await assertText(exportDrawer, ['选择你现在需要的内容', '下载 3D 模型 (GLB)', '概念视图'])
        assert(await exportDrawer.getByText('交给三维设计师', { exact: true }).count() === 0, 'Agent export drawer must not show legacy export choices')
        const renderResponse = waitForCompatHttpResponse(page, {
          method: 'GET',
          path: /\/api\/v1\/agent\/asset-versions\/[^/]+:render\?/,
        })
        await exportDrawer.getByRole('button', { name: '生成概念图', exact: true }).click()
        assert((await renderResponse).ok, 'agent concept render request must succeed')
        await exportDrawer.locator('.agent-concept-view-card').nth(4).waitFor({ timeout: TIMEOUT_MS })
        await assertText(exportDrawer, ['爆炸概念图', '透明背景'])
        const packageResponse = waitForCompatHttpResponse(page, {
          method: 'GET',
          path: /\/api\/v1\/agent\/asset-versions\/[^/]+:render-package\?/,
        })
        const packageDownload = page.waitForEvent('download')
        await exportDrawer.getByRole('button', { name: '下载概念图包', exact: true }).click()
        assert((await packageResponse).ok, 'agent concept render package request must succeed')
        const packageArtifact = await packageDownload
        assert(packageArtifact.suggestedFilename().endsWith('-concept-views.zip'), 'concept package must download a bounded ZIP')
        await packageArtifact.path()
        const glbDownload = page.waitForEvent('download')
        await exportDrawer.getByRole('button', { name: '下载 3D 模型 (GLB)', exact: true }).click()
        const glbArtifact = await glbDownload
        assert(glbArtifact.suggestedFilename().endsWith('.glb'), 'Agent drawer must download a GLB directly')
        await glbArtifact.path()
        const beforeReloadCanvas = await page.locator('.weapon-viewport canvas').count()
        await page.reload({ waitUntil: 'networkidle' })
        await page.getByLabel('分件候选').waitFor({ timeout: TIMEOUT_MS })
        const afterReload = await jsonRequest(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
        assert(await page.locator('.weapon-viewport canvas').count() === 1 && beforeReloadCanvas === 1, 'reload must preserve one WebGL canvas')
        assert(afterReload.active_design?.asset_version_id === afterQuality.active_design?.asset_version_id, 'reload must restore active asset')
        return evidence(projectId, afterReload, ['compatibility_seed_not_v003', 'immutable_confirm', 'quality_alignment', 'direct_glb_download', 'render_package_download', 'reload_restore', 'single_canvas'])
      } finally { await page.close() }
    })

    const report = buildGateReport(results)
    await writeGateReport(report)
    if (!report.ok) {
      console.error(JSON.stringify(report, null, 2))
      process.exitCode = report.exit_code
      return
    }
    console.log(JSON.stringify(report, null, 2))
  } catch (error) {
    const report = buildGateReport(results, {
      phase: 'runner',
      subsystem: 'desktop_workbench_runner',
      stable_error_code: 'GATE_RUNNER_INTERNAL_ERROR',
      exit_code: EXIT_CODES.internal,
      run_status: 'failed',
    })
    await writeGateReport(report)
    console.error(JSON.stringify(report, null, 2))
    process.exitCode = EXIT_CODES.internal
  }
}

async function runScenario(results, id, fn) {
  if (SCENARIO_FILTER && SCENARIO_FILTER !== id) return
  const startedAtMs = Date.now()
  const startedAt = new Date().toISOString()
  let runtime = null
  try {
    runtime = await withinTimeout(
      startIsolatedRuntime(id),
      RUNTIME_START_TIMEOUT_MS,
      `${id} isolated runtime startup`,
    )
    context = runtime.context
    agentBaseUrl = runtime.agentBaseUrl
    viteBaseUrl = runtime.viteBaseUrl
    projectId = null
    const evidence = await withinTimeout(fn(), scenarioWallClockTimeoutMs(), `${id} scenario body`)
    const result = {
      id,
      status: 'passed',
      phase: 'workbench_e2e',
      subsystem: 'desktop_workbench',
      stable_error_code: null,
      started_at: startedAt,
      elapsed_ms: Date.now() - startedAtMs,
      ...evidence,
    }
    results.push(result)
    await writeFile(join(OUTPUT_DIR, `${id}.json`), `${JSON.stringify(result, null, 2)}\n`)
  } catch (error) {
    const diagnostic = (error instanceof Error ? `${error.name}: ${error.message}` : String(error)).slice(0, 4000)
    const result = {
      id,
      status: 'failed',
      phase: 'workbench_e2e',
      subsystem: subsystemForScenario(id),
      stable_error_code: stableErrorCode(error),
      started_at: startedAt,
      elapsed_ms: Date.now() - startedAtMs,
      diagnostic,
    }
    results.push(result)
    await writeFile(join(OUTPUT_DIR, `${id}.json`), `${JSON.stringify(result, null, 2)}\n`)
  } finally {
    const runtimeToStop = runtime ?? activeRuntime
    await withinTimeout(
      stopIsolatedRuntime(runtimeToStop),
      RUNTIME_STOP_TIMEOUT_MS,
      `${id} isolated runtime cleanup`,
    ).catch((cleanupError) => {
      // Cleanup is a hard failure boundary rather than a best-effort detail:
      // an orphaned test server can poison the next scenario or a packaged
      // port check.  Preserve the original scenario result but surface this
      // separately in its persisted diagnostic.
      const detail = cleanupError instanceof Error ? cleanupError.message : String(cleanupError)
      const result = results.at(-1)
      if (result && result.id === id) {
        result.status = 'failed'
        result.stable_error_code = 'RUNTIME_CLEANUP_TIMEOUT'
        result.diagnostic = `${result.diagnostic ?? 'scenario completed'}; cleanup: ${detail}`.slice(0, 4000)
      } else {
        results.push({
          id,
          status: 'failed',
          phase: 'workbench_e2e',
          subsystem: 'desktop_workbench_runner',
          stable_error_code: 'RUNTIME_CLEANUP_TIMEOUT',
          started_at: startedAt,
          diagnostic: detail.slice(0, 4000),
        })
      }
    })
    const persisted = results.at(-1)
    if (persisted?.id === id) {
      persisted.elapsed_ms = Date.now() - startedAtMs
      await writeFile(join(OUTPUT_DIR, `${id}.json`), `${JSON.stringify(persisted, null, 2)}\n`)
    }
    context = null
    agentBaseUrl = null
    viteBaseUrl = null
    projectId = null
    if (activeRuntime === runtimeToStop) activeRuntime = null
  }
}

async function startIsolatedRuntime(id) {
  const tempRoot = await mkdtemp(join(tmpdir(), `forgecad-t002-${safeId(id)}-`))
  const libraryRoot = join(tempRoot, 'library')
  const agentPort = await freePort()
  const vitePort = await freePort()
  const runtime = {
    tempRoot,
    processes: [],
    browser: null,
    context: null,
    agentBaseUrl: `http://127.0.0.1:${agentPort}`,
    viteBaseUrl: `http://127.0.0.1:${vitePort}`,
  }
  activeRuntime = runtime
  try {
    const agent = spawn(
      join(ROOT, '.venv', 'bin', 'python'),
      ['-m', 'uvicorn', 'wushen_agent.test_oracle:create_app', '--factory', '--host', '127.0.0.1', '--port', String(agentPort)],
      {
        cwd: ROOT,
        env: legacyLifecycleTestOracleEnvironment(process.env, {
          WUSHEN_LIBRARY_ROOT: libraryRoot,
          WUSHEN_MIGRATIONS_DIR: join(ROOT, 'migrations'),
          WUSHEN_CORS_ORIGINS: runtime.viteBaseUrl,
          WUSHEN_LOCAL_WORKER_ENABLED: '0',
          FORGECAD_CONCEPT_WORKER_ENABLED: '1',
          FORGECAD_CONCEPT_PLANNER_PROVIDER: 'deterministic_rules',
        }),
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    drainProcessOutput(agent)
    runtime.processes.push(agent)
    await waitForHttp(`${runtime.agentBaseUrl}/api/health`, agent, 'Agent')

    const vite = spawn(
      process.execPath,
      [join(ROOT, 'node_modules', 'vite', 'bin', 'vite.js'), '--host', '127.0.0.1', '--port', String(vitePort)],
      {
        cwd: join(ROOT, 'apps', 'desktop'),
        env: { ...process.env, VITE_FORGE_API_BASE_URL: runtime.agentBaseUrl },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    drainProcessOutput(vite)
    runtime.processes.push(vite)
    await waitForHttp(runtime.viteBaseUrl, vite, 'Vite')

    runtime.browser = await launchBrowser()
    runtime.context = await runtime.browser.newContext({ viewport: { width: 1440, height: 960 } })
    return runtime
  } catch (error) {
    await stopIsolatedRuntime(runtime)
    throw error
  }
}

async function stopIsolatedRuntime(runtime) {
  if (!runtime) return
  const failures = []
  // Playwright owns the context as a browser child. Closing both concurrently
  // races the context teardown against the browser transport, which can leave
  // browser.close() waiting even though every scenario assertion completed.
  // Close the ownership chain in order. System Chrome can terminate and drop
  // the Playwright transport without resolving close(); only accept that
  // bounded timeout when the public connection state proves it is gone.
  for (const cleanup of [
    runtime.context && (() => withinTimeout(runtime.context.close(), 5_000, 'browser context close')),
    runtime.browser && (async () => {
      try {
        await withinTimeout(runtime.browser.close(), 5_000, 'browser close')
      } catch (error) {
        if (runtime.browser.isConnected()) throw error
      }
    }),
  ].filter(Boolean)) {
    try {
      await cleanup()
    } catch (error) {
      failures.push(error)
    }
  }
  const processCleanup = await Promise.allSettled(runtime.processes.reverse().map(stopProcess))
  failures.push(...processCleanup.filter((entry) => entry.status === 'rejected').map((entry) => entry.reason))
  await withinTimeout(rm(runtime.tempRoot, { recursive: true, force: true }), 5_000, 'temporary library cleanup')
  if (failures.length > 0) {
    throw new Error(`runtime cleanup failed: ${failures.map((failure) => String(failure)).join('; ').slice(0, 2000)}`)
  }
}

async function openWorkbench(context, baseUrl) {
  const page = await context.newPage()
  await page.goto(`${baseUrl}/#/cad`, { waitUntil: 'networkidle' })
  await page.waitForSelector('[data-testid="cad-workbench"]', { timeout: TIMEOUT_MS })
  await page.getByLabel('设计需求', { exact: true }).waitFor({ timeout: TIMEOUT_MS })
  await page.getByRole('button', { name: '发送设计需求', exact: true }).waitFor({ timeout: TIMEOUT_MS })
  return page
}

async function sendBrief(page, brief) {
  const input = page.getByLabel('设计需求', { exact: true })
  await input.fill(brief)
  const send = page.getByRole('button', { name: '发送设计需求', exact: true })
  await send.waitFor({ timeout: TIMEOUT_MS })
  await send.click()
}

async function selectAdjustablePart(page, projectId) {
  const firstPart = page.getByLabel('分件候选').locator('.agent-segmentation-list button').filter({ hasText: '可调整' }).first()
  await firstPart.waitFor({ timeout: TIMEOUT_MS })
  const selectionResponse = waitForCompatHttpResponse(page, {
    method: 'POST',
    path: `/api/v1/projects/${projectId}/active-design:select`,
  })
  await firstPart.click()
  const selection = (await selectionResponse).body.json()
  assert(typeof selection.selected_part_id === 'string', 'selection response must include selected_part_id')
  assert(typeof selection.selected_material_zone_id === 'string', 'selection response must include the stable selected Material Zone')
  await page.getByLabel('部件可调参数').waitFor({ timeout: TIMEOUT_MS })
  return selection
}

async function assertCompatibilityV003Rejection(page, baseUrl, projectId, beforeSnapshot, label) {
  const failure = page.locator('[data-generation-state="failed"][aria-label="生成失败"]')
  await failure.waitFor({ timeout: TIMEOUT_MS })
  const text = await failure.innerText()
  assert(text.includes('Agent 没有返回正式的单一结果决策'), `${label} must expose the V003 decision-contract rejection`)
  assert(await page.getByLabel('当前临时结果').count() === 0, `${label} must not present a legacy Planner result as a V003 result`)
  assert(await page.getByLabel('Agent 完整外观方向').count() === 0, `${label} must not restore direction selection`)
  assert(await page.locator('.weapon-viewport canvas').count() === 1, `${label} must retain one WebGL canvas`)
  const afterSnapshot = await requestStatus(baseUrl, `/api/v1/projects/${projectId}/active-design`)
  assertStableSnapshot(beforeSnapshot, afterSnapshot, `${label} rejection`)
  return afterSnapshot
}

function assertStableSnapshot(before, after, label) {
  assert(after.status === before.status, `${label} must preserve active-design response status`)
  assert(
    JSON.stringify({
      revision: after.body?.revision ?? null,
      active_asset_version_id: after.body?.active_design?.asset_version_id ?? null,
      preview: after.body?.preview ?? null,
      error_code: errorCode(after.body),
    }) === JSON.stringify({
      revision: before.body?.revision ?? null,
      active_asset_version_id: before.body?.active_design?.asset_version_id ?? null,
      preview: before.body?.preview ?? null,
      error_code: errorCode(before.body),
    }),
    `${label} must not create or alter an ActiveDesignSnapshot`,
  )
}

async function ensureCompatibilitySeedAgentAsset(page, baseUrl, projectId, label) {
  const current = await requestStatus(baseUrl, `/api/v1/projects/${projectId}/active-design`)
  if (current.body?.active_design?.source === 'agent_asset') return current.body

  // T002 is a compatibility/workbench regression, not the V003 proof path.
  // It explicitly opts into the isolated Python oracle and creates an asset
  // through its bounded build -> segment -> commit protocol solely to prepare
  // the later material/quality/export UI facets.  This setup is intentionally
  // never rendered as a generated "current result", never calls the Rust
  // V003 decision route, and is labelled in each scenario's evidence.
  const seed = `t002-compat-seed-${Date.now()}`
  const thread = await jsonPost(baseUrl, '/api/v1/agent/threads', {
    client_request_id: `${seed}-thread`,
    project_id: projectId,
    title: 'T002 compatibility asset seed',
    provider_id: 'deterministic_kernel',
  }, `${seed}-thread`)
  const turn = await jsonPost(baseUrl, `/api/v1/agent/threads/${thread.thread_id}/turns`, {
    client_request_id: `${seed}-turn`,
    message: '设计一辆双座冰原探索汽车，完整封闭车身，作为非功能展示模型。',
  }, `${seed}-turn`)
  const plan = turn.items
    ?.find((item) => item?.item_type === 'tool_result' && item?.payload?.result?.plan)
    ?.payload?.result?.plan
  assert(plan?.schema_version === 'MechanicalConceptPlan@1', `${label} compatibility seed must receive a bounded legacy fixture plan`)
  const directionId = plan.directions?.[0]?.direction_id
  assert(typeof directionId === 'string', `${label} compatibility seed plan must have a fixture direction`)
  const build = await jsonPost(baseUrl, '/api/v1/agent/blockouts', {
    client_request_id: `${seed}-build`,
    plan,
    direction_id: directionId,
    presentation_profile: 'showcase',
  }, `${seed}-build`)
  const segmented = await jsonPost(baseUrl, '/api/v1/agent/blockouts:segment', {
    client_request_id: `${seed}-segment`,
    plan,
    direction_id: directionId,
    artifact_id: build.artifact_id,
    presentation_profile: 'showcase',
  }, `${seed}-segment`)
  assert(segmented.artifact_id === build.artifact_id, `${label} compatibility seed must preserve build/segment artifact identity`)
  const committedVersion = await jsonPost(baseUrl, '/api/v1/agent/blockouts:commit', {
    client_request_id: `${seed}-commit`,
    artifact_id: segmented.artifact_id,
    project_id: projectId,
    summary: 'T002 compatibility seed asset; not a V003 generation result',
  }, `${seed}-commit`)
  assert(typeof committedVersion.asset_version_id === 'string', `${label} compatibility seed commit must return an Agent asset`)
  await page.reload({ waitUntil: 'networkidle' })
  try {
    await page.getByLabel('分件候选').getByText(/可编辑资产 v\d+/, { exact: false }).waitFor({ timeout: TIMEOUT_MS })
  } catch (error) {
    const workbench = page.getByTestId('cad-workbench')
    const pageProjectId = await workbench.getAttribute('data-qa-project-id').catch(() => null)
    const pageAssetVersionId = await workbench.getAttribute('data-qa-active-asset-version-id').catch(() => null)
    const snapshot = await requestStatus(baseUrl, `/api/v1/projects/${projectId}/active-design`)
    throw new Error(
      `${error instanceof Error ? error.message : String(error)}; `
      + `reload diagnostic page_project=${pageProjectId ?? 'missing'} expected_project=${projectId} `
      + `page_asset=${pageAssetVersionId ?? 'missing'} committed_asset=${committedVersion.asset_version_id} `
      + `snapshot_status=${snapshot.status} snapshot_asset=${snapshot.body?.active_design?.asset_version_id ?? 'missing'} `
      + `candidate_count=${await page.getByLabel('分件候选').count()}`,
    )
  }
  const committed = await jsonRequest(baseUrl, `/api/v1/projects/${projectId}/active-design`)
  assert(committed.active_design?.source === 'agent_asset', `${label} compatibility seed must activate an Agent asset`)
  assert(committed.active_design?.asset_version_id === committedVersion.asset_version_id, `${label} Snapshot must bind the explicit compatibility seed version`)
  return committed
}

async function ensureLegacyConversion(page, agentBaseUrl, projectId) {
  const notice = page.getByLabel('旧版设计转换')
  if (await notice.count() === 1 && await notice.isVisible()) {
    const conversionResponse = waitForCompatHttpResponse(page, {
      method: 'POST',
      path: `/api/v1/projects/${projectId}/active-design:convert-legacy`,
    })
    await notice.getByRole('button', { name: '让 Agent 重建可编辑资产', exact: true }).click()
    const response = await conversionResponse
    assert(response.ok, `legacy conversion handoff failed: ${response.status}`)
    await page.getByText(/已准备 legacy 只读设计的 Agent 重建输入/).waitFor({ timeout: TIMEOUT_MS })
  }
  // A fresh browser page has no in-memory conversion flag. Re-assert the
  // durable server intent before committing a candidate so this scenario does
  // not depend on the previous page's React state.
  const snapshot = await requestStatus(agentBaseUrl, `/api/v1/projects/${projectId}/active-design`)
  // A newly-created project has no ActiveDesignSnapshot until the first Agent
  // asset is committed.  This is an expected empty-project state, not a
  // reason to bootstrap a fake Snapshot or fail the later commit scenario.
  if (snapshot.status === 404 && errorCode(snapshot.body) === 'ACTIVE_DESIGN_NOT_FOUND') return
  if (snapshot.body?.active_design?.source === 'legacy_concept_read_only') {
    const response = await fetch(`${agentBaseUrl}/api/v1/projects/${projectId}/active-design:convert-legacy`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Idempotency-Key': `t002-legacy-${Date.now()}` },
      body: JSON.stringify({ client_request_id: `t002-legacy-${Date.now()}`, snapshot_revision: snapshot.body.revision }),
      signal: AbortSignal.timeout(API_TIMEOUT_MS),
    })
    if (!response.ok) throw new Error(`durable legacy conversion handoff failed: ${response.status} ${await response.text()}`)
  }
}

function evidence(projectId, snapshotResponse, assertions) {
  const body = snapshotResponse.body ?? snapshotResponse
  return {
    project_id: projectId,
    active_asset_version_id: body.active_design?.asset_version_id ?? null,
    snapshot_revision: body.revision ?? body.snapshot?.revision ?? null,
    assertions,
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message)
}

function errorCode(body) {
  return body?.error?.code ?? null
}

function stableProjectState(project) {
  return JSON.stringify({
    project_id: project?.project_id ?? null,
    current_version_id: project?.current_version_id ?? null,
    version_ids: Array.isArray(project?.versions)
      ? project.versions.map((version) => version?.version_id ?? null)
      : [],
  })
}

function safeId(value) {
  return String(value).replace(/[^A-Za-z0-9_-]/g, '_')
}

function subsystemForScenario(id) {
  if (id === 'T002-12-confirm-quality-export-reload') return 'quality_export'
  if (id === 'T002-10-part-selection-and-material-zone' || id === 'T002-11-changeset-preview-cancel') return 'renderer'
  return 'browser'
}

function stableErrorCode(error) {
  const message = error instanceof Error ? error.message : String(error)
  if (/\bport\b|EADDRINUSE|PACKAGED_PORT/i.test(message)) return 'PORT_CONFLICT'
  if (/SingleResultDecision|正式的单一结果决策|terminal generation failure/i.test(message)) return 'V003_DECISION_CONTRACT_FAILED'
  if (/quality|检查当前|readback/i.test(message)) return 'QUALITY_ASSERTION_FAILED'
  if (/export|导出|download|下载|render-package/i.test(message)) return 'EXPORT_ASSERTION_FAILED'
  if (/canvas|viewport|GLB|renderer|render|材质|zone/i.test(message)) return 'RENDERER_ASSERTION_FAILED'
  if (/timeout|timed out|within .*ms|did not become ready/i.test(message)) return 'WORKBENCH_TIMEOUT'
  return 'BROWSER_ASSERTION_FAILED'
}

const FACET_SCENARIOS = Object.freeze({
  browser: [
    'T002-01-bootstrap-single-canvas',
    'T002-02-legacy-explicit-handoff',
    'T002-03-ambiguous-clarification-write-barrier',
    'T002-03b-scope-stop-before-direction',
    'T002-04b-single-turn-action-loop-no-write',
    'T002-04-car-brief',
    'T002-05-aircraft-brief',
    'T002-06-robotic-arm-brief',
    'T002-07-future-prop-brief',
    'T002-08-preview-does-not-write-version',
    'T002-09-commit-editable-agent-asset',
    'T002-10-part-selection-and-material-zone',
    'T002-11-changeset-preview-cancel',
    'T002-12-confirm-quality-export-reload',
  ],
  renderer: [
    'T002-01-bootstrap-single-canvas',
    'T002-04b-single-turn-action-loop-no-write',
    'T002-04-car-brief',
    'T002-05-aircraft-brief',
    'T002-06-robotic-arm-brief',
    'T002-07-future-prop-brief',
    'T002-08-preview-does-not-write-version',
    'T002-09-commit-editable-agent-asset',
    'T002-10-part-selection-and-material-zone',
    'T002-11-changeset-preview-cancel',
    'T002-12-confirm-quality-export-reload',
  ],
  quality: ['T002-12-confirm-quality-export-reload'],
  export: ['T002-12-confirm-quality-export-reload'],
})

function buildGateReport(results, overrides = {}) {
  const failed = results.filter((item) => item.status === 'failed')
  const skipped = results.filter((item) => item.status === 'skipped')
  const incomplete = results.length !== EXPECTED_SCENARIO_COUNT || skipped.length > 0
  const statusForFacet = (ids) => {
    const covered = results.filter((item) => ids.includes(item.id))
    const failedIds = covered.filter((item) => item.status === 'failed').map((item) => item.id)
    const skippedIds = covered.filter((item) => item.status === 'skipped').map((item) => item.id)
    return {
      status: failedIds.length > 0 ? 'failed' : skippedIds.length > 0 || covered.length !== ids.length ? 'not_run' : 'passed',
      scenario_ids: ids,
      failed_scenario_ids: failedIds,
    }
  }
  const defaultError = failed[0]?.stable_error_code ?? null
  return {
    schema_version: 'ForgeCADWorkbenchE2EGateReport@1',
    gate_id: 'FGC-T002',
    phase: overrides.phase ?? 'workbench_e2e',
    subsystem: overrides.subsystem ?? 'desktop_workbench',
    stable_error_code: overrides.stable_error_code ?? (incomplete ? 'SCENARIO_SET_INCOMPLETE' : defaultError),
    run_status: overrides.run_status ?? 'completed',
    ok: overrides.ok ?? (failed.length === 0 && !incomplete),
    exit_code: overrides.exit_code ?? (failed.length === 0 && !incomplete ? EXIT_CODES.passed : EXIT_CODES.failed),
    blocked_ports: overrides.blocked_ports ?? [],
    expected_scenario_count: EXPECTED_SCENARIO_COUNT,
    scenario_count: results.length,
    facets: {
      browser: statusForFacet(FACET_SCENARIOS.browser),
      renderer: statusForFacet(FACET_SCENARIOS.renderer),
      quality: statusForFacet(FACET_SCENARIOS.quality),
      export: statusForFacet(FACET_SCENARIOS.export),
    },
    formal_v003_evidence: {
      status: 'separate_required_gate',
      gate_id: 'FGC-V003',
      command: 'npm run desktop:v003-rust-fixture-workbench-playwright-e2e',
      ownership: 'rust_app_server_native_product_tools',
      assertion: 'One Rust-owned SingleResultDecision@1 produces one production GLB preview and one confirmed atomic asset.',
    },
    scenarios: results,
  }
}

async function writeGateReport(report) {
  await writeFile(join(OUTPUT_DIR, 'report.json'), `${JSON.stringify(report, null, 2)}\n`)
}

async function detectPackagedPortConflicts() {
  const configured = process.env[PACKAGED_PORT_ENV]
  const ports = (configured ? configured.split(',') : DEFAULT_PACKAGED_PORTS)
    .map((value) => Number(String(value).trim()))
    .filter((value) => Number.isInteger(value) && value > 0 && value <= 65535)
  const occupied = []
  for (const port of ports) {
    if (await isPortOccupied(port)) occupied.push(port)
  }
  return occupied
}

async function isPortOccupied(port) {
  const net = await import('node:net')
  return new Promise((resolveOccupied) => {
    const socket = net.createConnection({ host: '127.0.0.1', port })
    const finish = (occupied) => {
      socket.destroy()
      resolveOccupied(occupied)
    }
    socket.once('connect', () => finish(true))
    socket.once('error', () => finish(false))
    socket.setTimeout(250, () => finish(false))
  })
}

async function assertText(locator, expected) {
  const text = await locator.innerText()
  for (const phrase of expected) assert(text.includes(phrase), `missing text ${phrase}: ${text}`)
}

async function waitForProjectId(baseUrl) {
  const deadline = Date.now() + TIMEOUT_MS
  while (Date.now() < deadline) {
    const response = await requestStatus(baseUrl, '/api/v1/projects')
    const id = response.body?.items?.[0]?.project_id
    if (response.status === 200 && id) return id
    await sleep(200)
  }
  throw new Error('workbench did not create a starter project')
}

async function requestStatus(baseUrl, path) {
  const response = await fetchWithGateTimeout(baseUrl, path)
  let body = null
  try { body = await response.json() } catch {}
  return { status: response.status, body }
}

async function jsonRequest(baseUrl, path) {
  const response = await fetchWithGateTimeout(baseUrl, path)
  const body = await response.json()
  if (!response.ok) throw new Error(`${response.status} ${path}: ${JSON.stringify(body)}`)
  return body
}

async function jsonPost(baseUrl, path, body, idempotencyKey) {
  let response
  try {
    response = await fetch(`${baseUrl}${path}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Idempotency-Key': idempotencyKey,
      },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(API_TIMEOUT_MS),
    })
  } catch (error) {
    throw new Error(`POST ${path} failed within ${API_TIMEOUT_MS}ms: ${error instanceof Error ? error.message : String(error)}`)
  }
  const payload = await response.json().catch(() => null)
  if (!response.ok) throw new Error(`${response.status} ${path}: ${JSON.stringify(payload)}`)
  return payload
}

async function fetchWithGateTimeout(baseUrl, path) {
  try {
    return await fetch(`${baseUrl}${path}`, { signal: AbortSignal.timeout(API_TIMEOUT_MS) })
  } catch (error) {
    throw new Error(`GET ${path} failed within ${API_TIMEOUT_MS}ms: ${error instanceof Error ? error.message : String(error)}`)
  }
}

async function waitForSnapshot(baseUrl, projectId, predicate) {
  const deadline = Date.now() + TIMEOUT_MS
  let latest = null
  while (Date.now() < deadline) {
    latest = await jsonRequest(baseUrl, `/api/v1/projects/${projectId}/active-design`)
    if (predicate(latest)) return latest
    await sleep(150)
  }
  throw new Error(`Snapshot did not reach expected state: ${JSON.stringify(latest)}`)
}

async function launchBrowser() {
  const executablePath = process.env.WUSHEN_BROWSER_EXECUTABLE
  if (executablePath) return chromium.launch({ executablePath, headless: true })
  if (process.platform === 'darwin') {
    const macChrome = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
    if (existsSync(macChrome)) return chromium.launch({ executablePath: macChrome, headless: true })
  }
  return chromium.launch({ channel: process.env.WUSHEN_BROWSER_CHANNEL || 'chrome', headless: true })
}

async function waitForHttp(url, child, label) {
  const deadline = Date.now() + TIMEOUT_MS
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`${label} exited with ${child.exitCode}`)
    try { if ((await fetch(url, { signal: AbortSignal.timeout(1_000) })).ok) return } catch {}
    await sleep(200)
  }
  throw new Error(`${label} did not become ready: ${url}`)
}

function drainProcessOutput(child) {
  // CI pipes are much smaller than local macOS pipes. Leaving access logs
  // unread eventually blocks Uvicorn/Vite and makes unrelated API reads time
  // out late in the scenario suite.
  child.stdout?.resume()
  child.stderr?.resume()
}

async function freePort() {
  const net = await import('node:net')
  return new Promise((resolvePort, reject) => {
    const server = net.createServer()
    server.once('error', reject)
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      server.close(() => resolvePort(address.port))
    })
  })
}

function stopProcess(child) {
  if (!child || child.exitCode !== null) return Promise.resolve()
  return new Promise((resolveStop) => {
    const timer = setTimeout(() => { child.kill('SIGKILL'); resolveStop() }, 3_000)
    child.once('exit', () => { clearTimeout(timer); resolveStop() })
    child.kill('SIGTERM')
  })
}

function scenarioWallClockTimeoutMs() {
  // Keep the test's outer wall-clock cap strictly larger than the configured
  // geometry budget plus the ordinary UI/API settling allowance.  This is a
  // deadlock guard, not a tighter replacement for a legitimate slow build.
  return agentGeometryTimeoutMs() + (TIMEOUT_MS * 3)
}

function installShutdownCleanup() {
  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.once(signal, () => {
      if (shutdownRequested) return
      shutdownRequested = true
      void withinTimeout(
        stopIsolatedRuntime(activeRuntime),
        RUNTIME_STOP_TIMEOUT_MS,
        `signal ${signal} runtime cleanup`,
      ).finally(() => {
        process.exit(signal === 'SIGINT' ? 130 : 143)
      })
    })
  }
}

async function withinTimeout(promise, timeoutMs, label) {
  let timeout = null
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timeout = setTimeout(() => reject(new Error(`${label} exceeded ${timeoutMs}ms`)), timeoutMs)
      }),
    ])
  } finally {
    if (timeout !== null) clearTimeout(timeout)
  }
}

function sleep(milliseconds) { return new Promise((resolveSleep) => setTimeout(resolveSleep, milliseconds)) }

export {
  EXIT_CODES,
  buildGateReport,
  detectPackagedPortConflicts,
  errorCode,
  stableErrorCode,
  stableProjectState,
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(async () => {
    const report = buildGateReport([], {
      phase: 'runner',
      subsystem: 'desktop_workbench_runner',
      stable_error_code: 'GATE_RUNNER_INTERNAL_ERROR',
      exit_code: EXIT_CODES.internal,
      run_status: 'failed',
    })
    await mkdir(OUTPUT_DIR, { recursive: true }).catch(() => undefined)
    await writeGateReport(report).catch(() => undefined)
    console.error(JSON.stringify(report, null, 2))
    process.exitCode = EXIT_CODES.internal
  })
}
