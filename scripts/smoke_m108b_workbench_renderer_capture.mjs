#!/usr/bin/env node
// Development-only M108B-04 workbench capture runner.  It creates twelve
// controlled temporary fixtures from existing verified GLB bytes solely to
// exercise the single-renderer lifecycle; it is never a formal visual kit.

import { spawn } from 'node:child_process'
import { cp, lstat, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { delimiter, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  M108B_DEVELOPMENT_FIXTURE_ORIGINS,
  M108B_RENDERER_DEVELOPMENT_PLAN_SCHEMA,
  safeCaptureStem,
  sha256,
  validateDevelopmentPlan,
} from './m108b_renderer_capture_evidence.mjs'

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)))

function run(command, args, env = {}) {
  return new Promise((resolveRun, rejectRun) => {
    const child = spawn(command, args, { cwd: ROOT, env: { ...process.env, ...env }, stdio: 'inherit' })
    child.once('error', rejectRun)
    child.once('exit', (code, signal) => code === 0 ? resolveRun() : rejectRun(new Error(`${command} exited with ${code ?? signal ?? 'unknown status'}`)))
  })
}

function controlledPlan(source) {
  if (source?.schema_version !== 'M108VisualBenchmarkKit@1' || !Array.isArray(source.fixtures) || source.fixtures.length !== 4) {
    throw new Error('M108B renderer development runner needs the temporary four-fixture M108 preflight source')
  }
  const fixtures = source.fixtures.flatMap((fixture, domainIndex) => [0, 1, 2].map((variant) => ({
    fixture_id: `development_${fixture.domain_pack_id.replace('pack_', '')}_${variant + 1}`,
    domain_pack_id: fixture.domain_pack_id,
    preflight_order: domainIndex * 3 + variant + 1,
    file: fixture.file,
    glb_sha256: fixture.glb_sha256,
    source_triangle_count: fixture.triangle_count,
    bounds_mm: fixture.bounds_mm,
    visual_environment: fixture.visual_environment,
  })))
  return {
    schema_version: M108B_RENDERER_DEVELOPMENT_PLAN_SCHEMA,
    evidence_origin: 'workbench_runtime_capture',
    formal_eligible: false,
    human_benchmark_evidence: false,
    provider_calls: 0,
    fixture_origin: 'controlled_development_fixture',
    score_status: 'not_scored',
    note: 'Temporary repeated controlled GLB inputs exercise twelve ordered workbench loads only; they are not frozen M108B fixtures and cannot be scored.',
    fixtures,
  }
}

function assertSafeRelativeFile(value, field, suffix) {
  if (typeof value !== 'string' || !value.endsWith(suffix) || value.startsWith('/') || value.includes('\\') || value.split('/').includes('..')) {
    throw new Error(`M108B preflight source has unsafe ${field}`)
  }
  return value
}

async function recipeBackedPreflightPlan(preflightRoot, sourcePrefix = '') {
  const sourcePath = join(preflightRoot, 'm108b-formal-source-draft.json')
  const sourceBytes = await readFile(sourcePath)
  const source = JSON.parse(sourceBytes)
  if (
    source?.schema_version !== 'M108BFormalFixtureSourceManifest@1'
    || source.fixture_origin !== 'recipe_backed_production'
    || source.frozen_before_scoring !== false
    || source.formal_eligible !== false
    || source.human_benchmark_evidence !== false
    || source.selection_status !== 'not_scored'
    || source.score_status !== 'not_scored'
    || source.provider_calls !== 0
    || !Array.isArray(source.fixtures)
    || source.fixtures.length !== 12
  ) throw new Error('M108B renderer preflight input is not an unfrozen non-formal twelve-asset source manifest')

  const fixtureIds = new Set()
  const hashes = new Set()
  const fixtures = await Promise.all(source.fixtures.map(async (fixture, index) => {
    if (!fixture || typeof fixture !== 'object' || typeof fixture.fixture_id !== 'string') {
      throw new Error('M108B renderer preflight source fixture is invalid')
    }
    if (fixtureIds.has(fixture.fixture_id) || hashes.has(fixture.glb_sha256)) {
      throw new Error('M108B renderer preflight source must contain unique fixture IDs and GLB hashes')
    }
    fixtureIds.add(fixture.fixture_id)
    hashes.add(fixture.glb_sha256)
    const file = assertSafeRelativeFile(fixture.source_glb, 'source_glb', '.glb')
    const readbackFile = assertSafeRelativeFile(fixture.readback_file, 'readback_file', '.json')
    const readback = JSON.parse(await readFile(join(preflightRoot, readbackFile), 'utf8'))
    const visualEnvironment = readback?.visual_environment
    const boundsMm = readback?.bounds_mm
    return {
      fixture_id: fixture.fixture_id,
      domain_pack_id: fixture.domain_pack_id,
      preflight_order: index + 1,
      file: sourcePrefix ? `${sourcePrefix}/${file}` : file,
      glb_sha256: fixture.glb_sha256,
      source_triangle_count: readback?.triangle_count,
      bounds_mm: boundsMm,
      visual_environment: visualEnvironment,
      source_preflight_fixture_sha256: sha256(Buffer.from(JSON.stringify(fixture))),
    }
  }))
  const domains = new Map()
  for (const fixture of fixtures) domains.set(fixture.domain_pack_id, (domains.get(fixture.domain_pack_id) ?? 0) + 1)
  if (domains.size !== 4 || [...domains.values()].some((count) => count !== 3)) {
    throw new Error('M108B renderer preflight input must cover exactly three assets across four domains')
  }
  return {
    schema_version: M108B_RENDERER_DEVELOPMENT_PLAN_SCHEMA,
    evidence_origin: 'workbench_runtime_capture',
    formal_eligible: false,
    human_benchmark_evidence: false,
    provider_calls: 0,
    fixture_origin: 'recipe_backed_development_preflight',
    score_status: 'not_scored',
    source_preflight_manifest: sourcePrefix ? `${sourcePrefix}/m108b-formal-source-draft.json` : 'm108b-formal-source-draft.json',
    source_preflight_manifest_sha256: sha256(sourceBytes),
    note: 'Recipe-backed, unfrozen M108B preflight assets exercise twelve ordered same-canvas workbench loads only. This runtime evidence remains non-formal and cannot be scored or used to freeze source selection.',
    fixtures,
  }
}

function parseArgs(argv) {
  const args = { preflightRoot: null }
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--preflight-root') {
      const value = argv[index + 1]
      if (!value || value.startsWith('-')) throw new Error('--preflight-root requires a directory')
      args.preflightRoot = value
      index += 1
    } else {
      throw new Error(`unknown argument: ${argv[index]}`)
    }
  }
  return args
}

function safeRunRelativeFile(value, field, suffix) {
  return assertSafeRelativeFile(value, field, suffix)
}

export async function assertRunManifest(value, fixtureOrigin, kitRoot, plan) {
  if (
    value?.schema_version !== 'M108BRendererDevelopmentCaptureRun@1'
    || value.evidence_origin !== 'workbench_runtime_capture'
    || value.formal_eligible !== false
    || value.human_benchmark_evidence !== false
    || value.provider_calls !== 0
    || value.score_status !== 'not_scored'
    || value.fixture_origin !== fixtureOrigin
    || value.capture_count !== 12
    || !Array.isArray(value.captures)
    || value.captures.length !== 12
  ) throw new Error('M108B workbench development run manifest is invalid')
  if (!plan || value.source_manifest_sha256 !== sha256(await readFile(join(kitRoot, 'manifest.json')))) {
    throw new Error('M108B workbench development run manifest source plan drifted')
  }
  if (plan.fixture_origin === 'recipe_backed_development_preflight') {
    const sourceManifest = safeRunRelativeFile(plan.source_preflight_manifest, 'source_preflight_manifest', '.json')
    const sourceManifestBytes = await readFile(join(kitRoot, sourceManifest))
    if (plan.source_preflight_manifest_sha256 !== sha256(sourceManifestBytes)) {
      throw new Error('M108B workbench development preflight snapshot drifted')
    }
  }
  const expectedById = new Map(plan.fixtures.map((fixture) => [fixture.fixture_id, fixture]))
  if (expectedById.size !== 12) throw new Error('M108B workbench development source plan is invalid')
  const seenFixtureIds = new Set()
  const seenCaptureIds = new Set()
  const generations = new Set(value.captures.map((capture) => capture?.renderer_capture?.cleanup?.renderer_generation))
  if (generations.size !== 1 || generations.has(undefined)) throw new Error('M108B workbench capture did not retain one renderer generation')
  for (const capture of value.captures) {
    const evidence = capture.renderer_capture
    const fixture = expectedById.get(capture?.fixture_id)
    if (!fixture || seenFixtureIds.has(fixture.fixture_id) || capture.glb_sha256 !== fixture.glb_sha256) {
      throw new Error(`M108B workbench capture source fixture drifted: ${capture?.fixture_id}`)
    }
    seenFixtureIds.add(fixture.fixture_id)
    if (
      evidence?.schema_version !== 'M108BRendererCaptureEvidence@1'
      || evidence.evidence_origin !== 'workbench_runtime_capture'
      || evidence.formal_eligible !== false
      || evidence.human_benchmark_evidence !== false
      || evidence.provider_calls !== 0
      || evidence.score_status !== 'not_scored'
      || evidence.fixture_id !== fixture.fixture_id
      || evidence.preflight_order !== fixture.preflight_order
      || typeof evidence.capture_id !== 'string'
      || seenCaptureIds.has(evidence.capture_id)
      || evidence.source_glb_sha256 !== capture.glb_sha256
      || evidence.renderer_contract?.renderer_id !== 'ForgeCADWorkbenchRenderer@1'
      || evidence.renderer_contract?.single_webgl_context !== true
      || !/^[a-f0-9]{64}$/.test(evidence.capture_sha256 ?? '')
    ) throw new Error(`M108B workbench capture evidence is invalid: ${capture?.fixture_id}`)
    seenCaptureIds.add(evidence.capture_id)
    const captureFile = safeRunRelativeFile(evidence.capture_file, 'capture_file', '.json')
    const evidencePath = join(kitRoot, captureFile)
    const evidenceBytes = await readFile(evidencePath)
    if (sha256(evidenceBytes) !== evidence.capture_sha256) {
      throw new Error(`M108B workbench capture evidence file drifted: ${capture.fixture_id}`)
    }
    const persisted = JSON.parse(evidenceBytes)
    if (JSON.stringify(persisted) !== JSON.stringify(Object.fromEntries(
      Object.entries(evidence).filter(([key]) => key !== 'capture_file' && key !== 'capture_sha256'),
    ))) {
      throw new Error(`M108B workbench capture evidence payload drifted: ${capture.fixture_id}`)
    }
    const screenshot = safeRunRelativeFile(evidence.png?.file, 'png.file', '.png')
    if (capture.screenshot !== screenshot || capture.screenshot_sha256 !== evidence.png?.sha256 || capture.screenshot_byte_size !== evidence.png?.byte_size) {
      throw new Error(`M108B workbench screenshot link drifted: ${capture.fixture_id}`)
    }
    const pngBytes = await readFile(join(kitRoot, screenshot))
    if (pngBytes.length !== evidence.png?.byte_size || sha256(pngBytes) !== evidence.png?.sha256) {
      throw new Error(`M108B workbench screenshot file drifted: ${capture.fixture_id}`)
    }
  }
  if (seenFixtureIds.size !== expectedById.size || seenCaptureIds.size !== 12) {
    throw new Error('M108B workbench capture completeness drifted')
  }
}

export async function main() {
  const args = parseArgs(process.argv.slice(2))
  const requestedOutput = process.env.FORGECAD_M108B_RENDERER_OUTPUT_DIR
  const preflightRoot = args.preflightRoot ? resolve(ROOT, args.preflightRoot) : null
  const kitRoot = requestedOutput ? resolve(ROOT, requestedOutput) : await mkdtemp(join(tmpdir(), 'forgecad_m108b_renderer_'))
  const cleanup = !requestedOutput
  try {
    if (requestedOutput) {
      const outputStat = await lstat(kitRoot).catch(() => null)
      if (outputStat) {
        if (!outputStat.isDirectory() || outputStat.isSymbolicLink()) {
          throw new Error('M108B renderer output must be a real directory')
        }
        if ((await readdir(kitRoot)).length > 0) {
          throw new Error('M108B renderer output directory must be empty')
        }
      } else {
        await mkdir(kitRoot)
      }
    }
    let plan
    if (preflightRoot) {
      if (preflightRoot === kitRoot) throw new Error('M108B renderer output directory must differ from --preflight-root')
      const sourceStat = await lstat(preflightRoot).catch(() => null)
      if (!sourceStat?.isDirectory() || sourceStat.isSymbolicLink()) throw new Error('--preflight-root must be a real existing directory')
      await run(join(ROOT, '.venv', 'bin', 'python'), [join(ROOT, 'scripts', 'prepare_m108b_asset_preflight.py'), '--verify', '--output', preflightRoot], {
        PYTHONPATH: [join(ROOT, 'apps', 'agent'), join(ROOT, 'scripts')].join(delimiter),
      })
      // Capture evidence must remain replayable after its caller cleans the
      // preflight directory.  Snapshot the verified input instead of leaving a
      // symlink to mutable external evidence, then verify the snapshot again.
      const preflightSnapshot = join(kitRoot, 'preflight-source')
      await cp(preflightRoot, preflightSnapshot, { recursive: true, dereference: true, errorOnExist: true })
      await run(join(ROOT, '.venv', 'bin', 'python'), [join(ROOT, 'scripts', 'prepare_m108b_asset_preflight.py'), '--verify', '--output', preflightSnapshot], {
        PYTHONPATH: [join(ROOT, 'apps', 'agent'), join(ROOT, 'scripts')].join(delimiter),
      })
      plan = await recipeBackedPreflightPlan(preflightSnapshot, 'preflight-source')
    } else {
      await run(join(ROOT, '.venv', 'bin', 'python'), [join(ROOT, 'scripts', 'prepare_m108_visual_benchmark.py'), '--output', kitRoot], {
        PYTHONPATH: [join(ROOT, 'apps', 'agent'), join(ROOT, 'scripts')].join(delimiter),
      })
      const preflight = JSON.parse(await readFile(join(kitRoot, 'manifest.json'), 'utf8'))
      plan = controlledPlan(preflight)
    }
    validateDevelopmentPlan(plan)
    await writeFile(join(kitRoot, 'manifest.json'), `${JSON.stringify(plan, null, 2)}\n`, 'utf8')
    await run(process.execPath, [join(ROOT, 'scripts', 'smoke_r3_concept_workbench_ui.mjs')], {
      FORGECAD_AGENT_FIRST_ONLY: '1',
      FORGECAD_M108B_WORKBENCH_CAPTURE: '1',
      FORGECAD_M108_KIT_DIR: kitRoot,
      FORGECAD_REQUIRE_BROWSER_DOWNLOADS: process.env.FORGECAD_REQUIRE_BROWSER_DOWNLOADS ?? '0',
    })
    const runManifestPath = join(kitRoot, 'workbench-captures', 'm108b-development-capture-manifest.json')
    const runManifest = JSON.parse(await readFile(runManifestPath, 'utf8'))
    await assertRunManifest(runManifest, plan.fixture_origin, kitRoot, plan)
    console.log(`M108B development renderer capture passed: captures=12, formal_eligible=false, fixture_origin=${plan.fixture_origin}, output=${kitRoot}`)
  } finally {
    if (cleanup) await rm(kitRoot, { recursive: true, force: true })
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main()
