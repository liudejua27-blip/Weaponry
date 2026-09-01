# Architecture and truth

## Route

`Codex → 11 façade MCP profile → Runtime → Objective/Knowledge/Program → Compiler → Browser Render → Metrics → Store/CAS → Viewer/Delivery`

The current Viewer already uses Three.js to read GLB. That is not this production route. The production route
must compile a canonical `KnifeSceneProgram@1` and bind every derivative to it.

The current implementation is a bounded in-process workbench through ObjectiveFunction,
fixed-view evidence, strict GLB-byte readiness and export. Runtime/Store/CAS/MCP persistence
is still pending; do not present an in-memory Studio decision as durable project truth.

## Upstream compatibility

Pinned baseline: `img2threejs@9fbd0ca5bbcc3b13bebe712745d6784d33db0b85`, Apache-2.0.

Accepted inputs include ObjectSculptSpec, detail inventory, pass state, component/material hierarchy, generated
factory and comparison artifacts. Preserve their source hash and license. Normalize them before a native edit:

1. map component IDs to stable Part IDs;
2. map material entries to MaterialZones;
3. replace the main blade primitive with a `blade_loft` node;
4. freeze source and imported hashes;
5. record unsupported or unknown fields instead of dropping them silently.

The adapter and compiler may emit TypeScript. They may not write SQLite/CAS, execute caller-supplied code, or
change the objective. Runtime validates and commits accepted bytes.

## Truth layers

- Design truth: authorized brief/reference plus classified knowledge.
- Program truth: canonical `KnifeSceneProgram@1`.
- Goal truth: canonical `KnifeObjectiveLedger@1` revision.
- Derived truth: factory, group, GLB, render and metric receipt bound to program and cohort.
- Delivery truth: package manifest and approval receipt.
