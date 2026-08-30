# Weaponry deletion manifest — 2026-08-29

## Applied batch D-001

Reason: the repository-only original sci-fi Skill conflicts with the authorized
CrossFire product direction and the new atomic transaction model.  It was not an
active Runtime Bundle and had no registry entry.

Deleted tracked paths:

- `skills/forgecad-sci-fi-fps-weapon-production/SKILL.md`
- `skills/forgecad-sci-fi-fps-weapon-production/agents/openai.yaml`
- `skills/forgecad-sci-fi-fps-weapon-production/references/contract-and-operator-map.md`

Replacement:

- `skills/weaponry-crossfire-agent-native-dcc/SKILL.md`
- `skills/weaponry-crossfire-agent-native-dcc/agents/openai.yaml`
- `skills/weaponry-crossfire-agent-native-dcc/references/action-space-and-gates.md`

Recovery: every deleted path is tracked in `HEAD` and can be inspected with
`git show HEAD:<path>`.  No Bundle registry, CAS, SQLite, reference, candidate,
evidence or generated output was deleted.

## Applied batch D-002 — Blender unavailable placeholder

ADR-0030 does not restore this deleted placeholder. A future knife prototype worker is a new,
fixed-provider architecture with closed jobs and Rust readback; it must not resurrect the old
generic capability/task surface or claim Blender is the product truth.

Reason: the public product surface must not expose an unavailable Blender
capability or transport boundary.  ADR-0030 permits a future fixed Blender/plugin
prototype only as an isolated internal provider for the knife cohort; it does
not restore a public route, Runtime truth owner, active Skill or package
dependency.

Deleted tracked paths:

- `apps/desktop/src-tauri/crates/forgecad-runtime/src/production_blender_worker_capability.rs`
- `packages/forgecad-contracts/schemas/blender-worker-capability.schema.json`
- `packages/forgecad-contracts/schemas/blender-worker-capability-get-request.schema.json`
- `packages/forgecad-contracts/schemas/blender-worker-capability-get-result.schema.json`
- `packages/forgecad-contracts/schemas/blender-task-request.schema.json`
- `packages/forgecad-contracts/schemas/blender-task-result.schema.json`
- `packages/forgecad-contracts/schemas/blender-task-error.schema.json`

Removed/updated surfaces:

- Runtime module registration and IPC dispatch;
- MCP tool/schema/preflight/dispatch/summary/test route;
- `forgecad-contracts` capability/task types and `forgecad-worker-protocol`
  Blender task types, validators and tests;
- contracts manifest entries and the dedicated contract-checker assertions.

Recovery: the deletion is isolated and reversible from the shared recovery baseline;
the original tracked paths remain inspectable with `git show HEAD:<path>`.
Historical evidence is not rewritten.  The current cohort's source/tool manifest,
benchmarks and related receipts still require a later explicit rebuild; this batch
does not claim current-cohort truth or alter `docs/evidence/**`.

## Pending batch D-003 — versioned subject-specific surfaces

The `fictional_energy_*`, `production_weapon_*`, `game_weapon_*` and V1/V2/V3
families require reachability classification before deletion:

- `retain`: animation, VFX, sockets, LOD/collision and game delivery still needed;
- `migrate`: weapon form operations expressible as macros over AuthoringMesh and
  ModifierGraph;
- `legacy`: old public route needed only to replay existing candidates/evidence;
- `delete`: unreachable route with no active manifest, persisted record or test
  consumer.

The migration must ship before removal.  Tool-count reduction without a replacement
would shrink, not expand, Codex's effective action space.

## Explicitly preserved

- all dirty tracked and untracked MCP010F/04BE/UV/PBR work;
- `docs/evidence/**`, output candidates and user references;
- animation/VFX/delivery modules pending profile reachability;
- `.forgecad-target`, `output` and `WushenForgeLibrary` generated/local directories.
