#!/usr/bin/env node

import { spawn } from 'node:child_process'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { delimiter, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))
function run(command, args, env = {}) {
  return new Promise((resolveRun, rejectRun) => {
    const child = spawn(command, args, {
      cwd: ROOT,
      env: { ...process.env, ...env },
      stdio: 'inherit',
    })
    child.once('error', rejectRun)
    child.once('exit', (code, signal) => {
      if (code === 0) {
        resolveRun()
        return
      }
      rejectRun(new Error(`${command} exited with ${code ?? signal ?? 'unknown status'}`))
    })
  })
}

export async function main() {
  const requestedOutput = process.env.FORGECAD_M108_RENDERER_OUTPUT_DIR
  const kitRoot = requestedOutput
    ? resolve(ROOT, requestedOutput)
    : await mkdtemp(join(tmpdir(), 'forgecad_m108_renderer_'))
  const cleanup = !requestedOutput
  try {
    await run(
      join(ROOT, '.venv', 'bin', 'python'),
      [join(ROOT, 'scripts', 'prepare_m108_visual_benchmark.py'), '--output', kitRoot],
      { PYTHONPATH: [join(ROOT, 'apps', 'agent'), join(ROOT, 'scripts')].join(delimiter) },
    )
    await run(
      process.execPath,
      [join(ROOT, 'scripts', 'smoke_r3_concept_workbench_ui.mjs')],
      {
        FORGECAD_AGENT_FIRST_ONLY: '1',
        FORGECAD_M108_WORKBENCH_CAPTURE: '1',
        FORGECAD_M108_KIT_DIR: kitRoot,
        FORGECAD_REQUIRE_BROWSER_DOWNLOADS: process.env.FORGECAD_REQUIRE_BROWSER_DOWNLOADS ?? '0',
      },
    )
    console.log(`M108 workbench renderer smoke passed: live environment, display scale, PBR color space/sampling and GPU budgets (${kitRoot})`)
  } finally {
    if (cleanup) await rm(kitRoot, { recursive: true, force: true })
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main()
}
