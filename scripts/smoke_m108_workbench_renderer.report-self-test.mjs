#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, relative } from 'node:path'

const REPORT_SCHEMA = 'M108WorkbenchRendererReportSelfTest@1'
const CAPTURE_SCHEMA = 'M108WorkbenchCapture@1'
const screenshotHashes = [
  'a'.repeat(64),
  'b'.repeat(64),
  'c'.repeat(64),
  'd'.repeat(64),
]

const tempRoot = await mkdtemp(join(tmpdir(), 'forgecad_m108_report_self_test_'))
try {
  const outputRoot = join(tempRoot, 'retained-output')
  const manifestPath = join(outputRoot, 'run-1', 'workbench-captures', 'capture-manifest.json')
  const manifest = {
    schema_version: CAPTURE_SCHEMA,
    captures: screenshotHashes.map((screenshot_sha256, index) => ({
      fixture_id: `fixture-${index + 1}`,
      screenshot_sha256,
    })),
  }
  const manifestBytes = Buffer.from(`${JSON.stringify(manifest)}\n`, 'utf8')
  await mkdir(join(outputRoot, 'run-1', 'workbench-captures'), { recursive: true })
  await writeFile(manifestPath, manifestBytes)
  const parsed = JSON.parse((await readFile(manifestPath)).toString('utf8'))
  const captures = Array.isArray(parsed.captures) ? parsed.captures : []
  if (parsed.schema_version !== CAPTURE_SCHEMA || captures.length !== 4) {
    throw new Error('synthetic manifest schema or capture count is invalid')
  }
  if (new Set(captures.map((capture) => capture.screenshot_sha256)).size !== 4) {
    throw new Error('synthetic manifest screenshot hashes are not distinct')
  }
  const report = {
    schema_version: REPORT_SCHEMA,
    phase: 'report_self_test',
    subsystem: 'm108_manifest_evidence',
    stable_error_code: null,
    manifest_schema: parsed.schema_version,
    fixture_count: captures.length,
    capture_count: captures.length,
    manifest_sha256: createHash('sha256').update(manifestBytes).digest('hex'),
    screenshot_sha256: captures.map((capture) => capture.screenshot_sha256).sort(),
    artifact_path: relative(outputRoot, manifestPath),
    path_redacted: true,
    ok: true,
  }
  const serialized = JSON.stringify(report)
  if (/\/(?:tmp|Users)\//.test(serialized) || serialized.includes('prompt') || serialized.includes('body')) {
    throw new Error('report self-test emitted a disallowed path or payload field')
  }
  console.log(serialized)
} catch (error) {
  console.log(JSON.stringify({
    schema_version: REPORT_SCHEMA,
    phase: 'report_self_test',
    subsystem: 'm108_manifest_evidence',
    stable_error_code: 'M108_REPORT_SELF_TEST_FAILED',
    ok: false,
  }))
  process.exitCode = 1
} finally {
  await rm(tempRoot, { recursive: true, force: true })
}
