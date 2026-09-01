#!/usr/bin/env node

// Fixed browser launcher for the packaged Three.js preview route.  It owns
// no geometry semantics: the browser entry performs compilation and capture,
// while this launcher only supplies the closed program and transports the
// browser-produced PNG/AOV bundle back to the typed Worker.

import { createServer } from 'node:http'
import { createHash } from 'node:crypto'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { existsSync, readFileSync } from 'node:fs'
import { spawn, execFileSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer as createViteServer } from 'vite'

const PACKAGE_ROOT = fileURLToPath(new URL('..', import.meta.url))
const PREVIEW_ENTRY = '/preview/worker-main.ts'
const BROWSER_TIMEOUT_MS = 45_000
const SHA256_PATTERN = /^[a-f0-9]{64}$/

function findPackagedBrowser() {
  const manifestUrl = new URL('../../weaponry-threejs-worker-manifest.json', import.meta.url)
  if (!existsSync(manifestUrl)) return null
  let manifest
  try {
    manifest = JSON.parse(readFileSync(manifestUrl, 'utf8'))
  } catch (error) {
    throw new Error(`packaged preview manifest is invalid: ${error instanceof Error ? error.message : String(error)}`)
  }
  const relativePath = manifest?.preview_runtime?.executable_path
  if (typeof relativePath !== 'string' || relativePath.length === 0 || relativePath.includes('..')) {
    throw new Error('packaged preview manifest has no safe browser executable path')
  }
  const browser = fileURLToPath(new URL(`../../${relativePath}`, import.meta.url))
  if (!existsSync(browser)) throw new Error('packaged preview browser executable is missing')
  return browser
}

function findBrowser() {
  const packaged = findPackagedBrowser()
  if (packaged) return packaged
  // Source-live is an explicit development mode. It is never an implicit
  // fallback for a packaged worker, so a release cannot accidentally bind to
  // a user's unpinned system browser.
  if (process.env.WPN_THREEJS_PREVIEW_SOURCE_LIVE !== '1') {
    throw new Error('packaged preview browser resource is unavailable; source-live requires WPN_THREEJS_PREVIEW_SOURCE_LIVE=1')
  }
  const configured = process.env.WPN_CHROME_EXECUTABLE
  const candidates = configured ? [configured] : [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    '/usr/bin/google-chrome',
    '/usr/bin/chromium',
  ]
  const browser = candidates.find((candidate) => existsSync(candidate))
  if (!browser) throw new Error('source-live headless browser runtime is not installed')
  return browser
}

function sha256File(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex')
}

function browserVersion(browser) {
  try {
    return execFileSync(browser, ['--version'], { encoding: 'utf8', timeout: 5_000 }).trim()
  } catch {
    return 'unknown'
  }
}

function browserRuntimeIdentity(browser) {
  const executable_sha256 = sha256File(browser)
  if (!SHA256_PATTERN.test(executable_sha256)) throw new Error('browser executable hash is invalid')
  return {
    schema_version: 'WeaponryThreeJsBrowserRuntime@1',
    browser_id: 'google-chrome-headless@1',
    executable_sha256,
    version: browserVersion(browser),
    three_revision: '0.185.1',
    network_policy: 'bundled-static-only@1',
  }
}

function jsonScript(value) {
  return JSON.stringify(value).replaceAll('<', '\\u003c').replaceAll('>', '\\u003e').replaceAll('&', '\\u0026')
}

function cdpClient(socketUrl) {
  const socket = new globalThis.WebSocket(socketUrl)
  let nextId = 1
  const pending = new Map()
  const opened = new Promise((resolveOpen, rejectOpen) => {
    socket.addEventListener('open', resolveOpen)
    socket.addEventListener('error', rejectOpen)
  })
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data)
    if (!message.id) return
    const waiter = pending.get(message.id)
    if (!waiter) return
    pending.delete(message.id)
    if (message.error) waiter.reject(new Error(message.error.message || 'CDP command failed'))
    else waiter.resolve(message.result)
  })
  return {
    async command(method, params = {}) {
      await opened
      const id = nextId++
      return new Promise((resolveCommand, rejectCommand) => {
        pending.set(id, { resolve: resolveCommand, reject: rejectCommand })
        socket.send(JSON.stringify({ id, method, params }))
      })
    },
    close() { socket.close() },
  }
}

async function fetchJson(url) {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`browser debugging endpoint returned ${response.status}`)
  return response.json()
}

async function waitForFile(path, timeoutMs) {
  const started = Date.now()
  while (Date.now() - started < timeoutMs) {
    if (existsSync(path)) return readFile(path, 'utf8')
    await new Promise((resolveWait) => setTimeout(resolveWait, 50))
  }
  throw new Error('headless browser did not publish a debugging endpoint')
}

async function startVite(program, cacheDir) {
  const vite = await createViteServer({
    root: PACKAGE_ROOT,
    cacheDir,
    appType: 'custom',
    logLevel: 'error',
    optimizeDeps: { noDiscovery: true, include: [] },
    server: { middlewareMode: true, cors: true },
  })
  const server = createServer(async (request, response) => {
    if (request.url === '/__weaponry_preview_worker.html') {
      const html = `<!doctype html><html><body><script>window.__WPN_PREVIEW_PROGRAM__=${jsonScript(program)};window.addEventListener('error',(event)=>{window.__WPN_THREEJS_PREVIEW_ERROR__=event.error?.stack||event.message||'browser module error'});window.addEventListener('unhandledrejection',(event)=>{window.__WPN_THREEJS_PREVIEW_ERROR__=event.reason?.stack||String(event.reason)})</script><script type="module" src="${PREVIEW_ENTRY}"></script></body></html>`
      response.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' })
      response.end(html)
      return
    }
    // Vite's Connect middleware is normally mounted by Vite itself.  The
    // fixed worker owns a tiny HTTP server instead, so explicitly serve the
    // transformed module request before falling through to static middleware.
    const pathname = new URL(request.url ?? '/', 'http://127.0.0.1').pathname
    if (pathname.endsWith('.ts') || pathname.endsWith('.js') || pathname.startsWith('/@')) {
      try {
        const transformed = await vite.transformRequest(pathname)
        if (transformed?.code) {
          response.writeHead(200, { 'content-type': 'text/javascript; charset=utf-8', 'cache-control': 'no-store' })
          response.end(transformed.code)
          return
        }
      } catch (error) {
        response.writeHead(500, { 'content-type': 'text/plain; charset=utf-8' })
        response.end(`preview module transform failed: ${error instanceof Error ? error.message : String(error)}`)
        return
      }
    }
    vite.middlewares(request, response, () => {
      if (!response.writableEnded) {
        response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' })
        response.end('preview resource not found')
      }
    })
  })
  await new Promise((resolveListen, rejectListen) => {
    server.once('error', rejectListen)
    server.listen(0, '127.0.0.1', resolveListen)
  })
  const address = server.address()
  if (!address || typeof address === 'string') throw new Error('preview server did not bind a TCP port')
  return { vite, server, url: `http://127.0.0.1:${address.port}/__weaponry_preview_worker.html` }
}

async function runBrowserPreview(program) {
  const browser = findBrowser()
  const runtime = browserRuntimeIdentity(browser)
  const profile = await mkdtemp(join(tmpdir(), 'weaponry-threejs-preview-browser-'))
  const server = await startVite(program, join(profile, 'vite-cache'))
  const chrome = spawn(browser, [
    '--headless=new',
    '--disable-gpu',
    '--enable-unsafe-swiftshader',
    '--use-angle=swiftshader',
    '--disable-dev-shm-usage',
    '--no-sandbox',
    '--remote-debugging-port=0',
    `--user-data-dir=${profile}`,
    '--window-size=512,512',
    'about:blank',
  ], { stdio: 'ignore' })
  try {
    const activePortText = await waitForFile(join(profile, 'DevToolsActivePort'), BROWSER_TIMEOUT_MS)
    const [portText] = activePortText.trim().split(/\s+/)
    const debuggingBase = `http://127.0.0.1:${portText}`
    const pages = await fetchJson(`${debuggingBase}/json/list`)
    const page = pages.find((candidate) => candidate.type === 'page')
    if (!page?.webSocketDebuggerUrl) throw new Error('headless browser page target is unavailable')
    const cdp = cdpClient(page.webSocketDebuggerUrl)
    await cdp.command('Runtime.enable')
    await cdp.command('Page.enable')
    await cdp.command('Page.navigate', { url: server.url })
    const started = Date.now()
    while (Date.now() - started < BROWSER_TIMEOUT_MS) {
      const evaluation = await cdp.command('Runtime.evaluate', {
        expression: 'JSON.stringify(globalThis.__WPN_THREEJS_PREVIEW_RESULT__ || {error: globalThis.__WPN_THREEJS_PREVIEW_ERROR__ || null})',
        returnByValue: true,
      })
      const encoded = evaluation?.result?.value
      if (typeof encoded === 'string') {
        const value = JSON.parse(encoded)
        if (value?.schema_version === 'WeaponryThreeJsBrowserPreviewWorkerResult@1') {
          cdp.close()
          return { ...value, runtime }
        }
        if (value?.error) throw new Error(`browser preview failed: ${value.error}`)
      }
      await new Promise((resolveWait) => setTimeout(resolveWait, 100))
    }
    const diagnostic = await cdp.command('Runtime.evaluate', {
      expression: `JSON.stringify({
        ready_state: document.readyState,
        progress: globalThis.__WPN_THREEJS_PREVIEW_PROGRESS__ || null,
        error: globalThis.__WPN_THREEJS_PREVIEW_ERROR__ || null,
        scripts: [...document.scripts].map((script) => script.src || 'inline'),
        resources: performance.getEntriesByType('resource').slice(-8).map((entry) => entry.name),
      })`,
      returnByValue: true,
    })
    throw new Error(`browser preview did not produce a result before timeout: ${diagnostic?.result?.value || 'no diagnostic'}`)
  } finally {
    if (!chrome.killed) chrome.kill('SIGTERM')
    await new Promise((resolveExit) => {
      if (chrome.exitCode !== null) {
        resolveExit()
        return
      }
      chrome.once('exit', resolveExit)
      setTimeout(resolveExit, 2_000)
    })
    await server.vite.close()
    await new Promise((resolveClose) => server.server.close(() => resolveClose()))
    await rm(profile, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
  }
}

export { runBrowserPreview }
