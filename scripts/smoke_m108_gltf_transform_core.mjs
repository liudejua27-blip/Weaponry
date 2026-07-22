#!/usr/bin/env node
/**
 * FGC-M108: evaluate whether glTF Transform core reader/writer can
 * not become a second, lossy asset truth for ForgeCAD showcase GLBs.
 */

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

import { NodeIO } from '@gltf-transform/core';
import { ALL_EXTENSIONS } from '@gltf-transform/extensions';
import { validateBytes } from 'gltf-validator';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const python = process.env.PYTHON ?? join(
  ROOT,
  '.venv',
  process.platform === 'win32' ? 'Scripts/python.exe' : 'bin/python',
);

const validatorFixtureDirectory = mkdtempSync(join(tmpdir(), 'forgecad-m108-transform-'));
process.on('exit', () => {
  rmSync(validatorFixtureDirectory, { recursive: true, force: true });
});
const fixtureResult = spawnSync(
  python,
  [
    join(ROOT, 'scripts', 'smoke_m108_visual_pbr.py'),
    '--emit-validator-directory',
    validatorFixtureDirectory,
  ],
  {
    cwd: ROOT,
    encoding: 'utf8',
    env: {
      ...process.env,
      PYTHONPATH: [join(ROOT, 'apps', 'agent'), join(ROOT, 'scripts'), process.env.PYTHONPATH]
        .filter(Boolean)
        .join(process.platform === 'win32' ? ';' : ':'),
    },
    maxBuffer: 1024 * 1024,
  },
);
assert.equal(fixtureResult.status, 0, fixtureResult.stderr || fixtureResult.stdout);

const fixtures = JSON.parse(fixtureResult.stdout);
assert.equal(fixtures.length, 4, 'expected one raw showcase GLB fixture per supported domain');

const readbackProgram = [
  'import json, sys',
  'from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts',
  'facts = read_shape_program_glb_facts(open(sys.argv[1], \'rb\').read())',
  'payload = {\'material_zone_faces\': facts.material_zone_faces, \'visual_texture_sets\': facts.visual_texture_sets}',
  'print(json.dumps(payload, ensure_ascii=True, sort_keys=True, separators=(\',\', \':\')))',
].join('; ');

function runReadback(payload) {
  const fixtureDirectory = mkdtempSync(join(tmpdir(), 'forgecad-m108-readback-'));
  const fixturePath = join(fixtureDirectory, 'asset.glb');
  writeFileSync(fixturePath, Buffer.from(payload));
  try {
    return spawnSync(python, ['-c', readbackProgram, fixturePath], {
      cwd: ROOT,
      encoding: 'utf8',
      env: {
        ...process.env,
        PYTHONPATH: [join(ROOT, 'apps', 'agent'), join(ROOT, 'scripts'), process.env.PYTHONPATH]
          .filter(Boolean)
          .join(process.platform === 'win32' ? ';' : ':'),
      },
      maxBuffer: 20 * 1024 * 1024,
    });
  } finally {
    rmSync(fixtureDirectory, { recursive: true, force: true });
  }
}

function readback(payload) {
  const result = runReadback(payload);
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

function expectForgeCadReadbackRejected(payload, fixtureId) {
  const result = runReadback(payload);
  assert.notEqual(result.status, 0, `${fixtureId}: rewritten GLB unexpectedly remained ForgeCAD-readback valid`);
  const diagnostic = `${result.stderr ?? ''}\n${result.stdout ?? ''}`;
  if (/GLB PBR sampling state does not match the fixed repeat\/linear contract/.test(diagnostic)) {
    return 'fixed_texture_sampling_state_changed';
  }
  assert.match(
    diagnostic,
    /GLB visual material parameters do not match the built-in PBR truth/,
    `${fixtureId}: writer was rejected for an unexpected reason`,
  );
  return 'explicit_default_pbr_parameters_removed';
}

function expectedPartZoneMaterialMapping(facts) {
  const textureSetByMaterial = new Map(
    facts.visual_texture_sets.map((textureSet) => [
      textureSet.material_id,
      {
        visual_texture_set_id: textureSet.visual_texture_set_id,
        texture_material_id: textureSet.texture_material_id,
      },
    ]),
  );
  return facts.material_zone_faces
    .map((zone) => {
      const textureIdentity = textureSetByMaterial.get(zone.material_id);
      assert.ok(textureIdentity, `missing texture identity for ${zone.material_id}`);
      return {
        primitive_id: zone.primitive_id,
        part_instance_id: zone.part_instance_id,
        material_zone_id: zone.material_zone_id,
        material_id: zone.material_id,
        ...textureIdentity,
      };
    })
    .sort((left, right) => left.primitive_id.localeCompare(right.primitive_id));
}

function documentPartZoneMaterialMapping(document) {
  const mapping = [];
  for (const mesh of document.getRoot().listMeshes()) {
    for (const primitive of mesh.listPrimitives()) {
      const primitiveExtras = primitive.getExtras();
      const materialExtras = primitive.getMaterial()?.getExtras() ?? {};
      mapping.push({
        primitive_id: primitiveExtras.forgecad_primitive_id,
        part_instance_id: primitiveExtras.forgecad_part_instance_id,
        material_zone_id: primitiveExtras.forgecad_material_zone_id,
        material_id: primitiveExtras.forgecad_material_id,
        visual_texture_set_id: materialExtras.forgecad_visual_texture_set_id,
        texture_material_id: materialExtras.forgecad_texture_material_id,
      });
    }
  }
  return mapping.sort((left, right) => left.primitive_id.localeCompare(right.primitive_id));
}

const byteSizes = [];
const rejectionReasons = [];

for (const fixture of fixtures) {
  const source = new Uint8Array(readFileSync(fixture.glb_path));
  const io = new NodeIO().registerExtensions(ALL_EXTENSIONS);
  const document = await io.readBinary(source);
  const sourceFacts = readback(source);
  const expectedMapping = expectedPartZoneMaterialMapping(sourceFacts);
  assert.deepEqual(
    documentPartZoneMaterialMapping(document),
    expectedMapping,
    `${fixture.fixture_id}: core reader changed the required Part/zone/material mapping`,
  );
  const rewritten = await io.writeBinary(document);
  const report = await validateBytes(rewritten, {
    format: 'glb',
    maxIssues: 0,
    writeTimestamp: false,
  });
  assert.equal(report.issues.numErrors, 0, `${fixture.fixture_id}: rewritten GLB has errors`);
  assert.equal(report.issues.numWarnings, 0, `${fixture.fixture_id}: rewritten GLB has warnings`);
  assert.ok(rewritten.byteLength > 0, `${fixture.fixture_id}: writer produced no GLB bytes`);
  const rewrittenDocument = await io.readBinary(rewritten);
  assert.deepEqual(
    documentPartZoneMaterialMapping(rewrittenDocument),
    expectedMapping,
    `${fixture.fixture_id}: core writer changed the standard-readable Part/zone/material mapping`,
  );
  rejectionReasons.push({
    fixture_id: fixture.fixture_id,
    reason: expectForgeCadReadbackRejected(rewritten, fixture.fixture_id),
  });
  byteSizes.push({ fixture_id: fixture.fixture_id, source: source.byteLength, rewritten: rewritten.byteLength });
}

console.log(JSON.stringify({
  ok: true,
  decision: 'reject_core_writer_as_export_transform',
  note: 'glTF Transform preserves the standard-readable Part/zone/material mapping but changes fixed texture sampling state and may remove explicit default PBR parameters required by ForgeCAD readback. Its output is intentionally rejected and cannot replace the immutable compiled GLB.',
  rejection_reasons: rejectionReasons,
  byte_sizes: byteSizes,
}));
process.exit(0);
