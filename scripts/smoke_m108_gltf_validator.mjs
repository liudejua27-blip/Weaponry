#!/usr/bin/env node
/** FGC-M108: run the official Khronos GLB validator over raw ForgeCAD output. */

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { validateBytes, version as validatorVersion } from 'gltf-validator';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const python = process.env.PYTHON ?? join(
  ROOT,
  '.venv',
  process.platform === 'win32' ? 'Scripts/python.exe' : 'bin/python',
);
const fixtureResult = spawnSync(
  python,
  [join(ROOT, 'scripts', 'smoke_m108_visual_pbr.py'), '--emit-validator-fixtures'],
  {
    cwd: ROOT,
    encoding: 'utf8',
    env: {
      ...process.env,
      PYTHONPATH: [join(ROOT, 'apps', 'agent'), join(ROOT, 'scripts'), process.env.PYTHONPATH]
        .filter(Boolean)
        .join(process.platform === 'win32' ? ';' : ':'),
    },
    maxBuffer: 20 * 1024 * 1024,
  },
);
assert.equal(fixtureResult.status, 0, fixtureResult.stderr || fixtureResult.stdout);

const fixtures = JSON.parse(fixtureResult.stdout);
assert.equal(validatorVersion(), '2.0.0-dev.3.10');
assert.equal(fixtures.length, 4, 'expected one raw showcase GLB fixture per supported domain');

for (const fixture of fixtures) {
  assert.equal(typeof fixture.fixture_id, 'string');
  const report = await validateBytes(
    new Uint8Array(Buffer.from(fixture.glb_base64, 'base64')),
    { format: 'glb', maxIssues: 0, writeTimestamp: false },
  );
  assert.equal(report.issues.numErrors, 0, `${fixture.fixture_id}: ${JSON.stringify(report.issues.messages)}`);
  assert.equal(report.issues.numWarnings, 0, `${fixture.fixture_id}: ${JSON.stringify(report.issues.messages)}`);
}

const malformed = await validateBytes(new Uint8Array([0, 1, 2, 3]), {
  format: 'glb',
  maxIssues: 0,
  writeTimestamp: false,
});
assert.ok(malformed.issues.numErrors > 0, 'Khronos Validator must reject malformed GLB bytes');

console.log('M108 Khronos GLB validator smoke passed: four raw ForgeCAD showcase GLBs have zero errors and warnings');
