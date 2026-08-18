# Sci-fi FPS weapon Bundle proposal

Status: `target-design` / `development-only` / `not registered` / `execution unavailable`.

This directory is intentionally outside `packages/forgecad-skills/bundles/**` so the
active Skill checker and Runtime registry cannot expose an unreviewed proposal. It is
an auditable Bundle-shaped draft for an original, nonfunctional game/film visual
asset. It contains no model, texture, executable, script, network capability, or
commercial-game source material.

The profile/plan contracts are copied from the current first-party
`FictionalEnergyRifleProfile@1` and `FictionalEnergyRiflePlan@1` contracts. The recipe
locks the current bounded Geometry Operator Catalog and the validators are existing
first-party builtins. The proposal still cannot execute as a Runtime Skill because:

1. `sci-fi-fps-weapon@1.0.0` is not in `packages/forgecad-skills/registry.json`.
2. No weapon-specific first-party MaterialPack/asset payload and no registered
   weapon Bundle consumer are available. Material metadata is declaration-only.
3. No real user reference, multi-view likeness, PBR likeness, human review,
   export/restart, or 360 benchmark receipt is claimed.

Do not add this proposal to `registry.json` or copy it into active `bundles/**` until
the maintainer supplies a consumer, an offline licensed AssetPack/MaterialPack (or
explicitly accepts a no-payload route), registry/manifest updates, negative and
deterministic Runtime evidence, and the required visual/human/release gates.

`registry-entry.proposed.json` is a review aid only. Registration would additionally
require a top-level recipe at `packages/forgecad-skills/recipes/sci-fi-fps-weapon.recipe.json`,
the active Bundle materialization, checker/registry expected-set updates, aggregate
registry/trust/benchmark hashes, and current Runtime/consumer evidence. None of those
active-tree changes are made by this proposal.

The proposal deliberately does not declare cyan/gold assets. The currently known
material route is limited to the first-party glTF PBR subset declared in
`materials/index.json`; a missing color/material asset is a typed limitation, not a
reason to invent or download one.
