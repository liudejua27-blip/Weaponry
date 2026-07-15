#!/usr/bin/env node
/**
 * FGC-M108: evaluate whether glTF Transform core reader/writer can
 * not become a second, lossy asset truth for ForgeCAD showcase GLBs.
 */

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
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
assert.equal(fixtures.length, 4, 'expected one raw showcase GLB fixture per supported domain');

const readbackProgram = [
  'from dataclasses import asdict',
  'import json, sys',
  'from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts',
  'facts = read_shape_program_glb_facts(sys.stdin.buffer.read())',
  'print(json.dumps(asdict(facts), ensure_ascii=True, sort_keys=True, separators=(\',\', \':\')))',
].join('; ');

function readback(payload) {
  const result = spawnSync(python, ['-c', readbackProgram], {
    cwd: ROOT,
    input: Buffer.from(payload),
    encoding: 'utf8',
    env: {
      ...process.env,
      PYTHONPATH: [join(ROOT, 'apps', 'agent'), join(ROOT, 'scripts'), process.env.PYTHONPATH]
        .filter(Boolean)
        .join(process.platform === 'win32' ? ';' : ':'),
    },
    maxBuffer: 20 * 1024 * 1024,
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

function requiredMappingFacts(facts) {
  return {
    triangle_count: facts.triangle_count,
    mesh_count: facts.mesh_count,
    primitive_count: facts.primitive_count,
    material_count: facts.material_count,
    uv0_primitive_count: facts.uv0_primitive_count,
    normal_primitive_count: facts.normal_primitive_count,
    tangent_primitive_count: facts.tangent_primitive_count,
    material_zone_faces: facts.material_zone_faces,
    visual_texture_sets: facts.visual_texture_sets
      .map(({ material_index, maps, ...textureSet }) => ({
        ...textureSet,
        maps: maps.map(({ glb_image_index, glb_texture_index, ...map }) => map),
      }))
      .sort((left, right) => left.material_id.localeCompare(right.material_id)),
    visual_environment: facts.visual_environment,
  };
}

const byteSizes = [];

for (const fixture of fixtures) {
  const source = new Uint8Array(Buffer.from(fixture.glb_base64, 'base64'));
  const io = new NodeIO().registerExtensions(ALL_EXTENSIONS);
  const document = await io.readBinary(source);
  const rewritten = await io.writeBinary(document);
  const report = await validateBytes(rewritten, {
    format: 'glb',
    maxIssues: 0,
    writeTimestamp: false,
  });
  assert.equal(report.issues.numErrors, 0, `${fixture.fixture_id}: rewritten GLB has errors`);
  assert.equal(report.issues.numWarnings, 0, `${fixture.fixture_id}: rewritten GLB has warnings`);
  assert.ok(rewritten.byteLength > 0, `${fixture.fixture_id}: writer produced no GLB bytes`);
  const sourceFacts = readback(source);
  const rewrittenFacts = readback(rewritten);
  assert.deepEqual(
    requiredMappingFacts(rewrittenFacts),
    requiredMappingFacts(sourceFacts),
    `${fixture.fixture_id}: core reader/writer changed the required Part/zone/material PBR mapping`,
  );
  assert.notDeepEqual(
    rewrittenFacts,
    sourceFacts,
    `${fixture.fixture_id}: expected core writer to demonstrate non-identical ForgeCAD readback`,
  );
  byteSizes.push({ fixture_id: fixture.fixture_id, source: source.byteLength, rewritten: rewritten.byteLength });
}

console.log(JSON.stringify({
  ok: true,
  decision: 'reject_core_writer_as_export_transform',
  note: 'glTF Transform preserves the required Part/zone/material PBR mapping, but reindexes resources and changes ForgeCAD readback; it remains evaluation-only and cannot replace the immutable compiled GLB.',
  byte_sizes: byteSizes,
}));
process.exit(0);
