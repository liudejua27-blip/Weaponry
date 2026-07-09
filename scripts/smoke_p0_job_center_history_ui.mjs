#!/usr/bin/env node
import { spawn, spawnSync } from 'node:child_process'
import { mkdtemp, mkdir, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT_DIR = join(ROOT, 'output', 'playwright')
const JOB_CENTER_SCREENSHOT = join(OUTPUT_DIR, 'p0-job-center-history.png')
const JOB_CENTER_FAILURE_SCREENSHOT = join(OUTPUT_DIR, 'p0-job-center-history-failure.png')

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'wushen_p0_job_center_'))
  const libraryRoot = join(tempRoot, 'WushenForgeLibrary')
  const dbPath = join(libraryRoot, 'library.db')
  const agentPort = await freePort()
  const vitePort = await freePort()
  const agentBaseUrl = `http://127.0.0.1:${agentPort}`
  const viteBaseUrl = `http://127.0.0.1:${vitePort}`
  const processes = []

  try {
    const agent = spawnAgent(agentPort, libraryRoot, viteBaseUrl)
    processes.push(agent)
    await waitForHttp(`${agentBaseUrl}/api/health`, agent, 'agent health')

    const success = await createWeapon(agentBaseUrl, 'ui-history-success')
    const failedFilter = await createWeapon(agentBaseUrl, 'ui-history-failed-filter')
    const failedAction = await createWeapon(agentBaseUrl, 'ui-history-failed-action')
    const waitingCancel = await createWeapon(agentBaseUrl, 'ui-history-waiting-cancel')
    mutateJob(dbPath, 'timestamp', success.job_id, '2026-07-05T09:00:00+00:00')
    mutateJob(dbPath, 'failed', failedFilter.job_id, '2026-07-05T09:04:00+00:00')
    mutateJob(dbPath, 'failed', failedAction.job_id, '2026-07-05T09:03:00+00:00')
    mutateJob(dbPath, 'waiting', waitingCancel.job_id, '2026-07-05T09:02:00+00:00')

    const vite = spawn(
      'npm',
      ['--workspace', 'apps/desktop', 'run', 'dev', '--', '--host', '127.0.0.1', '--port', String(vitePort)],
      {
        cwd: ROOT,
        env: { ...process.env, VITE_FORGE_API_BASE_URL: agentBaseUrl },
        stdio: ['ignore', 'pipe', 'pipe'],
      }
    )
    processes.push(vite)
    await waitForHttp(viteBaseUrl, vite, 'vite frontend')

    const result = await runJobCenterUi(viteBaseUrl, {
      success,
      failedFilter,
      failedAction,
      waitingCancel,
    })
    console.log(JSON.stringify({ ok: true, ...result }, null, 2))
  } finally {
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
  }
}

async function runJobCenterUi(viteBaseUrl, jobs) {
  const browser = await launchSystemBrowser()
  const page = await browser.newPage({ viewport: { width: 1500, height: 1200 }, deviceScaleFactor: 1 })
  try {
    await mkdir(OUTPUT_DIR, { recursive: true })
    await page.addInitScript((seed) => {
      localStorage.setItem('wushen.recentJobId', seed.successJobId)
      localStorage.setItem('wushen.recentJobIds', JSON.stringify(seed.recentJobIds))
      localStorage.setItem('wushen.desktopNotifications', JSON.stringify(seed.desktopNotifications))
    }, {
      successJobId: jobs.success.job_id,
      recentJobIds: [jobs.waitingCancel.job_id, jobs.failedAction.job_id, jobs.success.job_id],
      desktopNotifications: [
        {
          id: `${jobs.failedAction.job_id}:failed`,
          jobId: jobs.failedAction.job_id,
          jobType: 'generate_3d',
          status: 'failed',
          weaponId: jobs.failedAction.weapon_id,
          message: 'generate_3d failed，需要查看任务中心。',
          createdAt: '2026-07-05T09:03:30.000Z',
        },
      ],
    })
    await page.goto(viteBaseUrl, { waitUntil: 'domcontentloaded' })
    await page.getByRole('button', { name: '任务中心' }).click()
    const center = page.locator('.job-center')
    await center.waitFor({ timeout: 20_000 })
    await assertText(center, ['任务中心', '搜索历史任务', '手动恢复 job', '最近任务唤醒', '桌面通知中心', jobs.failedFilter.job_id])
    await waitForText(center.locator('.job-wakeup-panel'), jobs.waitingCancel.job_id)
    await waitForText(center.locator('.desktop-notification-panel'), jobs.failedAction.job_id)
    await center.locator('.job-wakeup-panel').getByRole('button', { name: new RegExp(jobs.waitingCancel.job_id) }).click()
    await waitForText(center, `已恢复 ${jobs.waitingCancel.job_id}`)
    await center.locator('.desktop-notification-panel').getByRole('button', { name: new RegExp(jobs.failedAction.job_id) }).click()
    await waitForText(center, `已恢复 ${jobs.failedAction.job_id}`)

    await center.getByLabel('搜索历史任务').fill(jobs.failedFilter.job_id.slice(-8))
    await waitForText(center, jobs.failedFilter.job_id)
    await waitForNotText(center.locator('.job-history-list'), jobs.success.job_id)
    await clickJob(center, jobs.failedFilter.job_id)
    await assertText(center, ['失败原因', 'PROVIDER_TIMEOUT', 'Synthetic provider timeout'])

    await center.getByLabel('搜索历史任务').fill('')
    await center.getByLabel('状态').selectOption('failed')
    await center.getByLabel('失败原因').fill('PROVIDER_TIMEOUT')
    await waitForText(center, jobs.failedAction.job_id)
    await assertText(center.locator('.job-history-list'), [jobs.failedFilter.job_id, jobs.failedAction.job_id])
    await waitForNotText(center.locator('.job-history-list'), jobs.waitingCancel.job_id)
    await page.reload({ waitUntil: 'domcontentloaded' })
    await center.waitFor({ timeout: 20_000 })
    await waitForText(center, jobs.failedAction.job_id)
    if (await center.getByLabel('状态').inputValue() !== 'failed') throw new Error('Job status filter was not restored after reload')
    if (await center.getByLabel('失败原因').inputValue() !== 'PROVIDER_TIMEOUT') throw new Error('Job error filter was not restored after reload')
    await waitForNotText(center.locator('.job-history-list'), jobs.waitingCancel.job_id)

    await center.getByLabel('状态').selectOption('')
    await center.getByLabel('失败原因').fill('')
    await center.getByLabel('搜索历史任务').fill(jobs.failedAction.job_id)
    await waitForText(center, jobs.failedAction.job_id)
    await clickJob(center, jobs.failedAction.job_id)
    const retryResponse = page.waitForResponse((response) => {
      const request = response.request()
      return request.method() === 'POST' && new URL(response.url()).pathname.endsWith(`/api/jobs/${jobs.failedAction.job_id}/retry-from/rough3d_submit`)
    }, { timeout: 30_000 })
    await center.locator('.job-center-detail .job-drawer').getByRole('button', { name: '请求从失败步骤重试' }).click()
    const retry = await retryResponse
    if (!retry.ok()) throw new Error(`retry-from failed: ${retry.status()} ${await retry.text()}`)
    await waitForText(center, 'Action 审计')
    await waitForText(center, 'retry_from_step')
    await waitForText(center, 'retrying')
    await center.locator('.action-audit').getByRole('button', { name: /定位事件/ }).first().click()
    await waitForText(center.locator('.trace-step.action-highlight'), 'Action 审计定位到事件')
    await waitForText(center.locator('.action-audit li.selected'), 'retry_from_step')

    await center.getByLabel('搜索历史任务').fill(jobs.waitingCancel.job_id)
    await waitForText(center, jobs.waitingCancel.job_id)
    await clickJob(center, jobs.waitingCancel.job_id)
    await assertText(center, ['等待 Provider 返回', 'mock_3d · Provider 生成中'])
    const cancelResponse = page.waitForResponse((response) => {
      const request = response.request()
      return request.method() === 'POST' && new URL(response.url()).pathname.endsWith(`/api/jobs/${jobs.waitingCancel.job_id}/cancel`)
    }, { timeout: 30_000 })
    await center.locator('.job-center-detail .job-drawer').getByRole('button', { name: '请求取消' }).click()
    const cancel = await cancelResponse
    if (!cancel.ok()) throw new Error(`cancel failed: ${cancel.status()} ${await cancel.text()}`)
    await waitForText(center, 'cancel')
    await waitForText(center, '已取消')

    await center.locator('#manual-job-id').fill(jobs.success.job_id)
    await center.getByRole('button', { name: '恢复并订阅事件流' }).click()
    await waitForText(center, `已恢复 ${jobs.success.job_id}`)
    await waitForText(center, jobs.success.job_id)
    await page.screenshot({ path: JOB_CENTER_SCREENSHOT, fullPage: true })
    await assertScreenshot(JOB_CENTER_SCREENSHOT, 10_000)
    await browser.close()
    return {
      screenshot: JOB_CENTER_SCREENSHOT,
      failed_job_id: jobs.failedFilter.job_id,
      retry_job_id: jobs.failedAction.job_id,
      cancel_job_id: jobs.waitingCancel.job_id,
      restored_job_id: jobs.success.job_id,
    }
  } catch (error) {
    await captureFailure(page)
    await browser.close()
    throw error
  }
}

function spawnAgent(port, libraryRoot, viteBaseUrl) {
  return spawn(
    join(ROOT, '.venv', 'bin', 'python'),
    ['-m', 'uvicorn', 'wushen_agent.main:create_app', '--factory', '--host', '127.0.0.1', '--port', String(port)],
    {
      cwd: ROOT,
      env: {
        ...process.env,
        WUSHEN_LIBRARY_ROOT: libraryRoot,
        WUSHEN_MIGRATIONS_DIR: join(ROOT, 'migrations'),
        WUSHEN_CORS_ORIGINS: viteBaseUrl,
        WUSHEN_RECOVER_ON_STARTUP: '0',
        WUSHEN_LLM_PROVIDER: 'mock',
        WUSHEN_IMAGE_PROVIDER: 'mock',
        WUSHEN_3D_PROVIDER: 'mock',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    }
  )
}

async function createWeapon(baseUrl, clientRequestId) {
  return jsonRequest(baseUrl, '/api/weapons', {
    method: 'POST',
    idempotencyKey: clientRequestId,
    body: {
      client_request_id: clientRequestId,
      text: `${clientRequestId} 赤金国风龙纹长剑，3渲2，逼真外观，仅作为虚构 Unity 游戏资产`,
      sketch_asset_id: null,
      reference_asset_ids: [],
      auto_run: true,
      target: { phase: 'concept_to_rough_3d', engine: 'unity', output_format: 'glb' },
    },
  })
}

function mutateJob(dbPath, mode, jobId, updatedAt) {
  const code = String.raw`
import json
import sqlite3
import sys

db_path, mode, job_id, updated_at = sys.argv[1:5]
step = "rough3d_submit"

def canonical(data):
    return json.dumps(data, ensure_ascii=False, sort_keys=True, separators=(",", ":"))

with sqlite3.connect(db_path) as conn:
    conn.row_factory = sqlite3.Row
    row = conn.execute("SELECT weapon_id FROM generation_jobs WHERE job_id = ?", (job_id,)).fetchone()
    if row is None:
        raise SystemExit(f"missing job {job_id}")
    weapon_id = row["weapon_id"]
    next_seq = conn.execute("SELECT COALESCE(MAX(seq), 0) + 1 FROM agent_events WHERE job_id = ?", (job_id,)).fetchone()[0]
    event_id = f"evt_{job_id}_{next_seq:04d}"
    attempt = int(conn.execute("SELECT COALESCE(MAX(attempt), 1) FROM job_steps WHERE job_id = ? AND step_name = ?", (job_id, step)).fetchone()[0] or 1)

    if mode == "timestamp":
        conn.execute("UPDATE generation_jobs SET updated_at = ? WHERE job_id = ?", (updated_at, job_id))
    elif mode == "failed":
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'failed', current_step = ?, error_code = 'PROVIDER_TIMEOUT',
                error_message = 'Synthetic provider timeout for job center browser smoke.',
                updated_at = ?, finished_at = ?
            WHERE job_id = ?
            """,
            (step, updated_at, updated_at, job_id),
        )
        conn.execute(
            """
            UPDATE job_steps
            SET status = 'failed', error_code = 'PROVIDER_TIMEOUT',
                error_message = 'synthetic provider timeout', finished_at = ?,
                checkpoint_json = ?, resumable_after_restart = 1
            WHERE job_id = ? AND step_name = ?
            """,
            (updated_at, canonical({"step": step, "status": "failed", "resume_policy": "restart_step"}), job_id, step),
        )
        conn.execute(
            """
            INSERT INTO agent_events (
              event_id, job_id, seq, weapon_id, step, level, status, message,
              artifact_asset_id, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, 'error', 'failed', ?, NULL, ?, ?)
            """,
            (event_id, job_id, next_seq, weapon_id, step, "Synthetic provider timeout for job center browser smoke.", canonical({"progress": 0.9, "error_code": "PROVIDER_TIMEOUT"}), updated_at),
        )
    elif mode == "waiting":
        provider_task_record_id = f"ptask_waiting_{job_id}"
        provider_task_id = f"smoke_waiting_{job_id}"
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'waiting_provider', current_step = ?, updated_at = ?, finished_at = NULL
            WHERE job_id = ?
            """,
            (step, updated_at, job_id),
        )
        conn.execute(
            """
            UPDATE job_steps
            SET status = 'waiting_provider', provider_task_id = ?, finished_at = NULL,
                checkpoint_json = ?, resumable_after_restart = 1, cancel_state = 'none'
            WHERE job_id = ? AND step_name = ?
            """,
            (provider_task_id, canonical({"step": step, "status": "waiting_provider"}), job_id, step),
        )
        conn.execute(
            """
            INSERT INTO provider_tasks (
              task_record_id, job_id, step_name, attempt, provider_kind, provider_id,
              provider_task_id, status, last_seen_at, metadata_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, 'three_d', 'mock_3d', ?, 'polling', ?, '{}', ?, ?)
            ON CONFLICT(job_id, step_name, attempt, provider_task_id) DO UPDATE SET
              status = 'polling', last_seen_at = excluded.last_seen_at, updated_at = excluded.updated_at
            """,
            (provider_task_record_id, job_id, step, attempt, provider_task_id, updated_at, updated_at, updated_at),
        )
        conn.execute(
            """
            INSERT INTO job_checkpoints (
              checkpoint_id, job_id, step_name, attempt, status, resume_policy,
              provider_task_record_id, state_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, 'leased', 'manual_review', ?, ?, ?, ?)
            ON CONFLICT(job_id, step_name, attempt) DO UPDATE SET
              status = 'leased', resume_policy = 'manual_review',
              provider_task_record_id = excluded.provider_task_record_id,
              state_json = excluded.state_json, updated_at = excluded.updated_at
            """,
            (f"chk_waiting_{job_id}", job_id, step, attempt, provider_task_record_id, canonical({"provider_task_id": provider_task_id}), updated_at, updated_at),
        )
        conn.execute(
            """
            INSERT INTO agent_events (
              event_id, job_id, seq, weapon_id, step, level, status, message,
              artifact_asset_id, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, 'info', 'waiting_provider', ?, NULL, ?, ?)
            """,
            (event_id, job_id, next_seq, weapon_id, step, "Synthetic provider wait for job center browser smoke.", canonical({"progress": 0.55, "provider_task_id": provider_task_id}), updated_at),
        )
    else:
        raise SystemExit(f"unknown mode {mode}")
    conn.commit()
`
  const result = spawnSync(join(ROOT, '.venv', 'bin', 'python'), ['-c', code, dbPath, mode, jobId, updatedAt], {
    cwd: ROOT,
    encoding: 'utf8',
  })
  if (result.status !== 0) {
    throw new Error(`job mutation failed for ${mode}/${jobId}: ${result.stderr || result.stdout}`)
  }
}

async function jsonRequest(baseUrl, path, options = {}) {
  const response = await fetch(baseUrl + path, {
    method: options.method || 'GET',
    headers: {
      'Content-Type': 'application/json',
      ...(options.idempotencyKey ? { 'Idempotency-Key': options.idempotencyKey } : {}),
    },
    body: options.body ? JSON.stringify(options.body) : undefined,
  })
  if (!response.ok) throw new Error(`${options.method || 'GET'} ${path} failed: ${response.status} ${await response.text()}`)
  return response.json()
}

async function waitForHttp(url, child, label) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`${label} process exited early with code ${child.exitCode}`)
    try {
      const response = await fetch(url)
      if (response.ok) return
    } catch {
      // keep polling
    }
    await sleep(200)
  }
  throw new Error(`${label} did not become ready: ${url}`)
}

async function freePort() {
  const net = await import('node:net')
  return new Promise((resolvePort, reject) => {
    const server = net.createServer()
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      server.close(() => resolvePort(address.port))
    })
    server.on('error', reject)
  })
}

async function launchSystemBrowser() {
  const executablePath = process.env.WUSHEN_BROWSER_EXECUTABLE
  if (executablePath) return chromium.launch({ executablePath, headless: true })
  const channel = process.env.WUSHEN_BROWSER_CHANNEL || 'chrome'
  try {
    return await chromium.launch({ channel, headless: true })
  } catch (error) {
    throw new Error(`Browser smoke requires system Chrome or WUSHEN_BROWSER_EXECUTABLE. Launch failed: ${error}`)
  }
}

function stopProcess(child) {
  return new Promise((resolveStop) => {
    if (!child || child.exitCode !== null) {
      resolveStop()
      return
    }
    const timer = setTimeout(() => {
      child.kill('SIGKILL')
      resolveStop()
    }, 5000)
    child.once('exit', () => {
      clearTimeout(timer)
      resolveStop()
    })
    child.kill('SIGTERM')
  })
}

async function clickJob(center, jobId) {
  const row = center.locator('.job-history-list button').filter({ hasText: jobId })
  await row.waitFor({ timeout: 20_000 })
  await row.click()
  await waitForText(center.locator('.job-center-detail'), jobId)
}

async function assertText(locator, expectedItems) {
  const text = await locator.innerText({ timeout: 20_000 })
  for (const expected of expectedItems) {
    if (!text.includes(expected)) throw new Error(`missing ${expected}: ${text}`)
  }
}

async function waitForNotText(locator, unexpected) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    const text = await locator.innerText().catch(() => '')
    if (!text.includes(unexpected)) return
    await sleep(250)
  }
  const text = await locator.innerText().catch(() => '')
  throw new Error(`Timed out waiting for ${unexpected} to disappear: ${text}`)
}

async function waitForText(locator, expected) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    const text = await locator.innerText().catch(() => '')
    if (text.includes(expected)) return
    await sleep(250)
  }
  throw new Error(`Timed out waiting for ${expected}`)
}

async function assertScreenshot(path, minimumBytes) {
  const screenshot = await stat(path)
  if (screenshot.size < minimumBytes) throw new Error(`${path} looks too small: ${screenshot.size} bytes`)
}

async function captureFailure(page) {
  await mkdir(OUTPUT_DIR, { recursive: true })
  await page.screenshot({ path: JOB_CENTER_FAILURE_SCREENSHOT, fullPage: true }).catch(() => undefined)
  const bodyText = await page.locator('body').innerText({ timeout: 1000 }).catch(() => '')
  if (bodyText) console.error(bodyText.slice(0, 3000))
}

function sleep(ms) {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms))
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
