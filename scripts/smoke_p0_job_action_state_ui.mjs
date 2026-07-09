#!/usr/bin/env node
import { spawn, spawnSync } from 'node:child_process'
import { mkdtemp, mkdir, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright-core'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
const OUTPUT_DIR = join(ROOT, 'output', 'playwright')
const FAILED_RETRY_SCREENSHOT = join(OUTPUT_DIR, 'p0-job-trace-failed-retry.png')
const RETRY_FROM_SCREENSHOT = join(OUTPUT_DIR, 'p0-job-trace-retry-from.png')
const CANCEL_SCREENSHOT = join(OUTPUT_DIR, 'p0-job-trace-waiting-provider-cancel.png')
const RECOVERED_SCREENSHOT = join(OUTPUT_DIR, 'p0-job-trace-recovered-waiting-user.png')
const FAILURE_SCREENSHOT = join(OUTPUT_DIR, 'p0-job-trace-action-state-failure.png')

async function main() {
  const tempRoot = await mkdtemp(join(tmpdir(), 'wushen_p0_action_ui_'))
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

    const failedRetry = await createWeapon(agentBaseUrl, 'ui-action-failed-retry')
    const failedRetryFrom = await createWeapon(agentBaseUrl, 'ui-action-failed-retry-from')
    const waitingCancel = await createWeapon(agentBaseUrl, 'ui-action-waiting-cancel')
    const recovered = await createWeapon(agentBaseUrl, 'ui-action-recovered')
    mutateJob(dbPath, 'failed', failedRetry.job_id)
    mutateJob(dbPath, 'failed', failedRetryFrom.job_id)
    mutateJob(dbPath, 'interrupted', recovered.job_id)
    const recovery = await jsonRequest(agentBaseUrl, '/api/runtime/recover', { method: 'POST' })
    if (!recovery.items?.some((item) => item.job_id === recovered.job_id)) {
      throw new Error(`runtime recovery did not include ${recovered.job_id}: ${JSON.stringify(recovery)}`)
    }
    mutateJob(dbPath, 'waiting', waitingCancel.job_id)

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

    const result = await runActionStateUi(viteBaseUrl, agentBaseUrl, {
      failedRetry,
      failedRetryFrom,
      waitingCancel,
      recovered,
    })
    console.log(JSON.stringify({ ok: true, ...result }, null, 2))
  } finally {
    await Promise.all(processes.reverse().map(stopProcess))
    await rm(tempRoot, { recursive: true, force: true })
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

async function runActionStateUi(viteBaseUrl, agentBaseUrl, jobs) {
  const browser = await launchSystemBrowser()
  try {
    await mkdir(OUTPUT_DIR, { recursive: true })
    const retryResult = await verifyFailedRetry(browser, viteBaseUrl, jobs.failedRetry.job_id)
    const retryFromResult = await verifyRetryFrom(browser, viteBaseUrl, jobs.failedRetryFrom.job_id)
    const cancelResult = await verifyWaitingCancel(browser, viteBaseUrl, agentBaseUrl, jobs.waitingCancel.job_id)
    const recoveredResult = await verifyRecoveredRetry(browser, viteBaseUrl, jobs.recovered.job_id)
    return {
      failed_retry_job_id: jobs.failedRetry.job_id,
      retry_from_job_id: jobs.failedRetryFrom.job_id,
      cancel_job_id: jobs.waitingCancel.job_id,
      recovered_job_id: jobs.recovered.job_id,
      retry: retryResult,
      retry_from: retryFromResult,
      cancel: cancelResult,
      recovered: recoveredResult,
      screenshots: {
        failed_retry: FAILED_RETRY_SCREENSHOT,
        retry_from: RETRY_FROM_SCREENSHOT,
        cancel: CANCEL_SCREENSHOT,
        recovered: RECOVERED_SCREENSHOT,
      },
    }
  } finally {
    await browser.close()
  }
}

async function verifyFailedRetry(browser, viteBaseUrl, jobId) {
  const page = await openJobPage(browser, viteBaseUrl, jobId)
  try {
    const drawer = jobDrawer(page)
    await assertDrawerText(drawer, ['生成失败', '3D 粗模生成', 'PROVIDER_TIMEOUT', '可请求恢复'])
    await assertButtonState(drawer, '请求重试任务', true)
    await assertButtonState(drawer, '请求从失败步骤重试', true)
    await assertButtonState(drawer, '请求取消', false)
    await assertButtonState(drawer, '跳过 3D', true)
    const responsePromise = waitForPost(page, new RegExp(`/api/jobs/${jobId}/retry$`))
    await drawer.getByRole('button', { name: '请求重试任务' }).click()
    const response = await responsePromise
    await assertOkResponse(response, 'retry action')
    const payload = await response.json()
    assertEqual(payload.status, 'retrying', 'retry response status')
    await waitForDrawerText(drawer, '等待重试')
    await drawer.screenshot({ path: FAILED_RETRY_SCREENSHOT })
    await assertScreenshot(FAILED_RETRY_SCREENSHOT, 8_000)
    await page.close()
    return { status: payload.status, retry_from: payload.retry_from ?? null }
  } catch (error) {
    await captureFailure(page)
    await page.close()
    throw error
  }
}

async function verifyRetryFrom(browser, viteBaseUrl, jobId) {
  const page = await openJobPage(browser, viteBaseUrl, jobId)
  try {
    const drawer = jobDrawer(page)
    await assertDrawerText(drawer, ['生成失败', '3D 粗模生成', 'PROVIDER_TIMEOUT'])
    await assertButtonState(drawer, '请求重试任务', true)
    await assertButtonState(drawer, '请求从失败步骤重试', true)
    const responsePromise = waitForPost(page, new RegExp(`/api/jobs/${jobId}/retry-from/rough3d_submit$`))
    await drawer.getByRole('button', { name: '请求从失败步骤重试' }).click()
    const response = await responsePromise
    await assertOkResponse(response, 'retry-from action')
    const payload = await response.json()
    assertEqual(payload.status, 'retrying', 'retry-from response status')
    assertEqual(payload.retry_from, 'rough3d_submit', 'retry-from response step')
    await waitForDrawerText(drawer, 'retry_from')
    await drawer.screenshot({ path: RETRY_FROM_SCREENSHOT })
    await assertScreenshot(RETRY_FROM_SCREENSHOT, 8_000)
    await page.close()
    return { status: payload.status, retry_from: payload.retry_from }
  } catch (error) {
    await captureFailure(page)
    await page.close()
    throw error
  }
}

async function verifyWaitingCancel(browser, viteBaseUrl, agentBaseUrl, jobId) {
  const page = await openJobPage(browser, viteBaseUrl, jobId)
  try {
    const drawer = jobDrawer(page)
    await assertDrawerText(drawer, ['等待 Provider 返回', 'PROVIDER TASK', 'mock_3d · Provider 生成中', '可请求取消'])
    await assertButtonState(drawer, '请求重试任务', false)
    await assertButtonState(drawer, '请求从失败步骤重试', false)
    await assertButtonState(drawer, '请求取消', true)
    const responsePromise = waitForPost(page, new RegExp(`/api/jobs/${jobId}/cancel$`))
    await drawer.getByRole('button', { name: '请求取消' }).click()
    const response = await responsePromise
    await assertOkResponse(response, 'cancel action')
    const payload = await response.json()
    assertEqual(payload.status, 'cancelled', 'cancel response status')
    const runtime = await jsonRequest(agentBaseUrl, `/api/jobs/${jobId}/runtime`)
    if (!runtime.provider_tasks?.some((task) => task.status === 'cancel_requested' || task.status === 'cancelled')) {
      throw new Error(`cancel did not mark provider task cancel requested or cancelled: ${JSON.stringify(runtime)}`)
    }
    await waitForDrawerText(drawer, '已取消')
    await drawer.screenshot({ path: CANCEL_SCREENSHOT })
    await assertScreenshot(CANCEL_SCREENSHOT, 8_000)
    await page.close()
    return { status: payload.status, provider_task_status: runtime.provider_tasks.at(-1)?.status ?? 'unknown' }
  } catch (error) {
    await captureFailure(page)
    await page.close()
    throw error
  }
}

async function verifyRecoveredRetry(browser, viteBaseUrl, jobId) {
  const page = await openJobPage(browser, viteBaseUrl, jobId)
  try {
    const drawer = jobDrawer(page)
    await assertDrawerText(drawer, ['等待处理', '可请求恢复', '可请求取消', 'Agent restart recovery paused job'])
    await assertButtonState(drawer, '请求重试任务', true)
    await assertButtonState(drawer, '请求从失败步骤重试', false)
    await assertButtonState(drawer, '请求取消', true)
    await drawer.screenshot({ path: RECOVERED_SCREENSHOT })
    await assertScreenshot(RECOVERED_SCREENSHOT, 8_000)
    const responsePromise = waitForPost(page, new RegExp(`/api/jobs/${jobId}/retry$`))
    await drawer.getByRole('button', { name: '请求重试任务' }).click()
    const response = await responsePromise
    await assertOkResponse(response, 'recovered retry action')
    const payload = await response.json()
    assertEqual(payload.status, 'retrying', 'recovered retry response status')
    await waitForDrawerText(drawer, '等待重试')
    await page.close()
    return { status: payload.status, retry_from: payload.retry_from ?? null }
  } catch (error) {
    await captureFailure(page)
    await page.close()
    throw error
  }
}

async function openJobPage(browser, viteBaseUrl, jobId) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1200 }, deviceScaleFactor: 1 })
  await page.addInitScript((recentJobId) => {
    localStorage.setItem('wushen.recentJobId', recentJobId)
  }, jobId)
  await page.goto(viteBaseUrl, { waitUntil: 'networkidle' })
  const drawer = jobDrawer(page)
  await drawer.waitFor({ timeout: 20_000 })
  await waitForDrawerText(drawer, jobId)
  await page.locator('.runtime-mini:not(.empty)').last().waitFor({ timeout: 20_000 })
  return page
}

function jobDrawer(page) {
  return page.locator('.job-drawer').last()
}

async function assertDrawerText(drawer, expectedItems) {
  const text = await drawer.innerText()
  for (const expected of expectedItems) assertIncludes(text, expected, 'job drawer')
}

async function waitForDrawerText(drawer, expected) {
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline) {
    const text = await drawer.innerText().catch(() => '')
    if (text.includes(expected)) return
    await drawer.page().waitForTimeout(250)
  }
  throw new Error(`Job drawer did not contain ${expected}`)
}

async function assertButtonState(drawer, name, enabled) {
  const button = drawer.getByRole('button', { name })
  await button.waitFor({ timeout: 20_000 })
  const actual = await button.isEnabled()
  if (actual !== enabled) throw new Error(`${name} expected enabled=${enabled}, got ${actual}`)
}

function waitForPost(page, pathPattern) {
  return page.waitForResponse((response) => {
    const request = response.request()
    if (request.method() !== 'POST') return false
    const pathname = new URL(response.url()).pathname
    return pathPattern.test(pathname)
  }, { timeout: 30_000 })
}

async function createWeapon(baseUrl, clientRequestId) {
  return jsonRequest(baseUrl, '/api/weapons', {
    method: 'POST',
    idempotencyKey: clientRequestId,
    body: {
      client_request_id: clientRequestId,
      text: '赤金国风龙纹长剑，3渲2，逼真外观，仅作为虚构 Unity 游戏资产',
      sketch_asset_id: null,
      reference_asset_ids: [],
      auto_run: true,
      target: { phase: 'concept_to_rough_3d', engine: 'unity', output_format: 'glb' },
    },
  })
}

function mutateJob(dbPath, mode, jobId) {
  const code = String.raw`
import json
import sqlite3
import sys
from datetime import datetime, timezone

db_path, mode, job_id = sys.argv[1:4]
step = "rough3d_submit"
now = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")

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
    attempt_row = conn.execute("SELECT COALESCE(MAX(attempt), 1) FROM job_steps WHERE job_id = ? AND step_name = ?", (job_id, step)).fetchone()
    attempt = int(attempt_row[0] or 1)

    if mode == "failed":
        provider_task_record_id = f"ptask_failed_{job_id}"
        provider_task_id = f"smoke_failed_{job_id}"
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'failed', current_step = ?, error_code = 'PROVIDER_TIMEOUT',
                error_message = 'Synthetic provider timeout for browser action-state smoke.',
                updated_at = ?, finished_at = ?
            WHERE job_id = ?
            """,
            (step, now, now, job_id),
        )
        conn.execute(
            """
            UPDATE job_steps
            SET status = 'failed', error_code = 'PROVIDER_TIMEOUT',
                error_message = 'synthetic provider timeout', finished_at = ?,
                checkpoint_json = ?, resumable_after_restart = 1
            WHERE job_id = ? AND step_name = ?
            """,
            (now, canonical({"step": step, "status": "failed", "resume_policy": "restart_step"}), job_id, step),
        )
        conn.execute(
            """
            INSERT INTO agent_events (
              event_id, job_id, seq, weapon_id, step, level, status, message,
              artifact_asset_id, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, 'error', 'failed', ?, NULL, ?, ?)
            """,
            (
                event_id,
                job_id,
                next_seq,
                weapon_id,
                step,
                "Synthetic provider timeout for browser action-state smoke.",
                canonical({"progress": 0.9, "error_code": "PROVIDER_TIMEOUT"}),
                now,
            ),
        )
        conn.execute(
            """
            INSERT INTO provider_tasks (
              task_record_id, job_id, step_name, attempt, provider_kind, provider_id,
              provider_task_id, status, last_seen_at, metadata_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, 'three_d', 'mock_3d', ?, 'failed', ?, ?, ?, ?)
            ON CONFLICT(job_id, step_name, attempt, provider_task_id) DO UPDATE SET
              status = 'failed', metadata_json = excluded.metadata_json, last_seen_at = excluded.last_seen_at, updated_at = excluded.updated_at
            """,
            (provider_task_record_id, job_id, step, attempt, provider_task_id, now, canonical({"error_code": "PROVIDER_TIMEOUT"}), now, now),
        )
        conn.execute(
            """
            INSERT INTO job_checkpoints (
              checkpoint_id, job_id, step_name, attempt, status, resume_policy,
              provider_task_record_id, state_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, 'ready', 'restart_step', ?, ?, ?, ?)
            ON CONFLICT(job_id, step_name, attempt) DO UPDATE SET
              status = 'ready', resume_policy = 'restart_step',
              provider_task_record_id = excluded.provider_task_record_id,
              state_json = excluded.state_json, updated_at = excluded.updated_at
            """,
            (f"chk_{job_id}_{step}_{attempt}", job_id, step, attempt, provider_task_record_id, canonical({"error_code": "PROVIDER_TIMEOUT"}), now, now),
        )
    elif mode == "waiting":
        provider_task_record_id = f"ptask_wait_{job_id}"
        provider_task_id = f"mock_waiting_{job_id}"
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'waiting_provider', current_step = ?, provider_task_id = ?,
                updated_at = ?, finished_at = NULL
            WHERE job_id = ?
            """,
            (step, provider_task_id, now, job_id),
        )
        conn.execute(
            """
            UPDATE job_steps
            SET status = 'waiting_provider', provider_task_id = ?,
                resumable_after_restart = 1, cancel_state = 'none', finished_at = NULL
            WHERE job_id = ? AND step_name = ?
            """,
            (provider_task_id, job_id, step),
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
            (provider_task_record_id, job_id, step, attempt, provider_task_id, now, now, now),
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
            (f"chk_wait_{job_id}_{step}_{attempt}", job_id, step, attempt, provider_task_record_id, canonical({"provider_task_id": provider_task_id}), now, now),
        )
        conn.execute(
            """
            INSERT INTO agent_events (
              event_id, job_id, seq, weapon_id, step, level, status, message,
              artifact_asset_id, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, 'info', 'waiting_provider', ?, NULL, ?, ?)
            """,
            (
                event_id,
                job_id,
                next_seq,
                weapon_id,
                step,
                "Synthetic provider wait for browser action-state smoke.",
                canonical({"progress": 0.55, "provider_task_id": provider_task_id}),
                now,
            ),
        )
    elif mode == "interrupted":
        provider_task_record_id = f"ptask_restart_{job_id}"
        provider_task_id = f"mock_restart_{job_id}"
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'running', current_step = ?, provider_task_id = ?,
                updated_at = ?, finished_at = NULL
            WHERE job_id = ?
            """,
            (step, provider_task_id, now, job_id),
        )
        conn.execute(
            """
            UPDATE job_steps
            SET status = 'running', provider_task_id = ?,
                resumable_after_restart = 1, cancel_state = 'none', finished_at = NULL
            WHERE job_id = ? AND step_name = ?
            """,
            (provider_task_id, job_id, step),
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
            (provider_task_record_id, job_id, step, attempt, provider_task_id, now, now, now),
        )
        conn.execute(
            """
            INSERT INTO agent_events (
              event_id, job_id, seq, weapon_id, step, level, status, message,
              artifact_asset_id, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, 'warning', 'progress', ?, NULL, ?, ?)
            """,
            (
                event_id,
                job_id,
                next_seq,
                weapon_id,
                step,
                "Synthetic interrupted provider task for browser action-state smoke.",
                canonical({"progress": 0.55, "provider_task_id": provider_task_id}),
                now,
            ),
        )
    else:
        raise SystemExit(f"unknown mode {mode}")
    conn.commit()
`
  const result = spawnSync(join(ROOT, '.venv', 'bin', 'python'), ['-c', code, dbPath, mode, jobId], {
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

async function captureFailure(page) {
  await mkdir(OUTPUT_DIR, { recursive: true })
  await page.screenshot({ path: FAILURE_SCREENSHOT, fullPage: true }).catch(() => undefined)
  const bodyText = await page.locator('body').innerText({ timeout: 1000 }).catch(() => '')
  if (bodyText) console.error(bodyText.slice(0, 3000))
}

async function assertOkResponse(response, label) {
  if (response.ok()) return
  throw new Error(`${label} failed: ${response.status()} ${await response.text()}`)
}

async function assertScreenshot(path, minimumBytes) {
  const screenshot = await stat(path)
  if (screenshot.size < minimumBytes) throw new Error(`${path} looks too small: ${screenshot.size} bytes`)
}

function assertIncludes(text, expected, label) {
  if (!text.includes(expected)) throw new Error(`${label} missing ${expected}: ${text}`)
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) throw new Error(`${label} expected ${expected}, got ${actual}`)
}

function sleep(ms) {
  return new Promise((resolveSleep) => setTimeout(resolveSleep, ms))
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
