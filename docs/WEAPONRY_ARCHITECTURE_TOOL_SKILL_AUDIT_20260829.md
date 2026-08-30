# Weaponry architecture, Tool and Skill audit — 2026-08-29

## Current diagnosis

The repository is not short of interfaces. The current compatibility source
reports 226 public tools and 586 Schemas, while the default knife profile exposes
11 bounded façades. The imbalance is implementation placement:

| Area | Approximate Rust source lines | Interpretation |
| --- | ---: | --- |
| Runtime | 237,347 | orchestration and subject/version-specific behavior dominate |
| Store | 91,897 | persistence has become a second concentration point |
| MCP | 55,786 | thin-adapter goal is not reflected by size |
| Geometry Worker | 30,923 | actual geometry execution is comparatively small |
| High Worker | 7,407 | production high/low surface is still narrow |
| Render core + Worker | 7,041 | evidence renderer is bounded, not a DCC renderer |

This is a structural warning, not proof that every large module is wrong.  It does
show that adding more top-level weapon tools will worsen the ratio unless generic
authoring and evaluation capabilities move downward into reusable kernels.

## Tool findings

- `production_weapon_*` and `fictional_energy_*` each expose 34 tools; many encode
  subject/stage/version combinations that should become macros or presets.
- The public surface contains V1/V2/V3 generations and several read tools for
  capabilities that are permanently unavailable by policy.
- Only three top-level AuthoringMesh operations are currently exposed through a
  single-operation preview/prepare route, despite the larger public catalog.
- The modifier declaration is currently bounded to transform, mirror, array, bevel
  and normal with a short stack.  Boolean, solidify and subdivision are not yet a
  coherent non-destructive evaluation graph.

## Skill findings

The active Bundle set remains useful for intake, silhouette, semantic assembly,
mesh integrity, UV/PBR, render evidence and reference comparison.  Those should be
recomposed around the new authoring transaction and evaluated-mesh model.

The repository-only `forgecad-sci-fi-fps-weapon-production` Skill is obsolete for
the new product direction.  It forbids named commercial assets and enforces one
semantic Part per edit.  That conflicts with the reported authorized CrossFire
scope and prevents atomic multi-operation modeling.  It is replaced by the
Weaponry Skill; it was never an active Runtime Bundle and therefore its removal
does not change the installed Bundle registry.

## Module disposition

| Module family | Decision | Reason |
| --- | --- | --- |
| Runtime/store/MCP transaction core | keep, split by responsibility | sole writer, rollback and evidence remain essential |
| AuthoringMesh and feature graph | promote | foundation of open modeling action space |
| weapon-specific FormArt/production modules | migrate to macros | specialization belongs above the kernel |
| high/low/UV/bake/material/render evidence | keep and complete | required for game-ready delivery |
| mechanical pose/animation and VFX | keep behind weapon profile | required for inspect/reload/presentation, not modeling core |
| Blender headless capability placeholder | remove from public product | ADR-0030 permits only a new fixed, isolated internal knife provider; the generic public placeholder remains wrong |
| superseded versioned public APIs | legacy profile, then delete | physical deletion needs reachability/replay migration first |
| tracked historical evidence | retain/archive | evidence is not executable product code |
| generated output/target directories | preserve pending manifest | may contain user or acceptance artifacts |

## Deletion manifest protocol

Before each physical deletion batch, record exact paths, tracked/untracked state,
references from generated manifests and tests, replacement route, and recovery
source.  A clean `git diff --check`, focused tests and repository integrity gates
are required after the batch.  The dirty `main` worktree prevents broad directory
deletion, but does not prevent exact replacement of untouched obsolete files.

## Metrics for the next audit

- generic edit operations executable through one transaction contract;
- modifiers executable through one ordered graph contract;
- number of weapon macros expressed without new kernel code;
- deterministic journal replay success;
- stable-ID preservation on local repair;
- visual/human acceptance rate for authorized cohorts;
- public tool and Schema reduction after legacy migration.

Raw Tool/Schema counts must not be used as a capability success metric.

## Current cohort re-audit — 2026-08-29

The working tree reports **586 contract Schemas**, an explicit compatibility
surface of **226 MCP tools (131 read + 95 opt-in write)**, and a default knife
surface of **11 façades**. The current-source summary was rebuilt from the compiled
source-only binary and independently checked after WPN-KNIFE-PROFILE-001; historical
225/585 and earlier receipts remain immutable. A future code/profile change must
rebuild current-source truth again rather than editing those historical receipts.

The D-002 Blender removal is present in the working tree as one coherent
**public-surface** removal: the Runtime placeholder, public route, six Blender
schemas, MCP dispatch/validation, contract Rust types, worker-protocol Blender
task transport and dedicated checker assertions.  ADR-0030 may later add a
fixed, offline Blender/plugin prototype for the knife cohort, but that is an
isolated internal provider with its own lifecycle and is not a public MCP
capability, Runtime truth owner, active Skill or package dependency.  The
remaining `Blender`/`bpy` matches are intentional negative fixtures, historical
evidence references or boundary prose; no Blender runtime is reachable from the
active product path today.

Focused checks observed during this audit:

- `python3 scripts/check_forgecad_contracts.py`: PASS, 586 Schemas;
- `python3 scripts/check_weaponry_documentation_scope.py`: PASS, 126/126;
- `python3 scripts/check_repository_integrity.py`: PASS;
- `cargo check --workspace`: PASS, warnings only;
- `npm run release:docs-walkthrough`: must be rerun after this knife documentation
  update; its result is reported in the current handoff rather than inferred here.

## Knife-edge P0 surface (implemented source profile)

Codex should select a small number of typed, hash-bound workflow capabilities.
Each façade below may have a prepare/readback operation, but must not become a
generic catch-all or accept scripts, paths, URLs, model calls or inline geometry
truth.  The current 226 names remain compatibility routes until each façade has
replacement and replay evidence.

| P0 façade | Bounded responsibility | Current backing families |
|---|---|---|
| `weapon_preflight` | Runtime readiness, first-party preflight Skill, project/use-scope checks | `runtime_status`, `capabilities_get`, `skill_get`, `project_get/list` |
| `reference_intake` | Authorized reference import, CAS binding, view/mask inventory | `project_create`, `reference_import/get`, `reference_mask_*` |
| `observe` | Candidate, AuthoringMesh, stage, AOV, quality and evidence read model | `scene_observe_get`, `snapshot_get`, `selection_get`, `artifact_readback_get`, `quality_get`, `visual_evidence_bundle_get`, `critic_report_get` |
| `authoring_transaction` | Multi-operation AuthoringMesh journal, stable IDs, evaluated preview and strict readback | `authoring_mesh_transaction_*`, `authoring_mesh_*`, `geometry_prepare`, `change_prepare`, bounded `design_action_run_*` |
| `surface_pipeline` | High → editable Low → Hero UV → Cage/Bake → PBR with separate gates | `production_weapon_formal_high_*`, `low_quad_draft_durable_*`, `hero_uv_durable_*`, `production_weapon_high_low_bake_*`, `appearance_*` |
| `fps_presentation` | Fixed first-person/inspect/ADS views, sockets and bounded clips | `fps_presentation_package_v2_*`, `production_camera_lock_*`, mechanical/game socket families |
| `quality_review` | Same-candidate AOV/compare/contour/critic and explicit Codex/human review | `reference_compare_prepare`, `silhouette_*`, `render_evidence_*`, `candidate_*quality*`, `visual_review_submit`, `human_visual_review_submit` |
| `delivery` | LOD/collision/socket/engine-readback/export preparation | `game_asset_delivery_*`, `game_asset_lod_derive`, game socket families, `export_prepare` |
| `approval` | Candidate reject/confirm, promotion and export confirmation only after gates | `candidate_reject`, `candidate_confirm`, `cross_view_promotion_confirm`, `export_confirm` |
| `recovery` | Session/checkpoint/restore/repair intent, with no history rewrite | `session_*`, `checkpoint_*`, `restore_*`, `repair_intent_run_prepare` |
| `job` | Bounded asynchronous progress/result/cancel readback | `job_*`, `optimization_job_*`, primary-form job prepare |

This is an implemented 11-capability P0 selection surface, not a visual-quality
claim. The closed source contract is
`packages/forgecad-contracts/profiles/weaponry-knife-p0.json` with schema
`weaponry-knife-tool-profile.schema.json`; it is a separate profile manifest and
therefore does not inflate the 586 core Runtime schema count. The profile binds
each façade's read/write route classification and operation-list SHA-256, and
the focused checker verifies every listed route against the current source tool
manifest.  Under ADR-0030 the first implementation slice is one
authorized CrossFire knife plus one original control knife on the same build;
the default surface exposes exactly the 11 names above.  The explicit
`legacy-raw-tools` compatibility profile binds the complete current 131-read /
95-write / 226-total manifest and its manifest hashes; it is not a second
façade list. The default `authoring_transaction` façade also exposes two native-only
Curve/ModifierGraph operations that remain hidden from compatibility replay.
They currently persist deterministic Curve sampling and Dependency/Recompute
objects but intentionally create no mesh. An isolated
Blender prototype, if admitted for this cohort, is selected by the Runtime
provider policy and never appears in this allowlist.

## Tool compatibility disposition

The exact current family counts are:

| Family | Tools | Disposition |
|---|---:|---|
| `fictional_energy_*` | 34 | Compatibility/legacy; migrate VFX, trails, bloom, particles and attachment records to generic FPS presentation before route retirement |
| `mechanical_*` | 11 | Retain; Viewer and FPS presentation depend on bounded rigid clips/poses |
| `game_asset_*` | 3 | Retain; delivery/LOD/readback path |
| `game_weapon_*` | 12 | Retain; sockets/anchors are delivery/presentation truth |
| `production_weapon_form_art_*` | 16 | Compatibility/legacy; migrate D1 diagnostics and repair proposals to generic Part/action/evidence contracts |
| other `production_weapon_*` | 18 | Retain or migrate by stage; High/Low/UV/Bake/camera gates are product-chain responsibilities |
| all other generic/transaction/quality/recovery routes | 132 | Retain as backing implementation, then collapse behind the P0 façades only after replay/readback coverage |
| **total** | **226** | No further physical route deletion is authorized by this audit |

The `fictional_energy_*` family is not independently dead: current quality and
delivery code cross-calls its links, and it shares game socket/mechanical
dependencies.  Mechanical and game delivery are explicitly retained by ADR-0029;
they are not scope-cut merely because their names are not modeling primitives.

## Skill disposition

There are **12 active registry Skills**.  The P0 stable subset is the nine
generic intake/assembly/silhouette/integrity/render/compare/export/preflight
Skills: `ponytail-preflight`, `reference-intake`, `subject-profile`,
`semantic-assembly`, `silhouette-blockout`, `mesh-integrity`,
`render-evidence`, `reference-compare` and `local-edit-and-export`.

`primitive-blockout@0.2.0`, `hard-surface-detail@0.2.0` and `uv-pbr@0.2.0`
remain compatibility-backed rather than being deleted.  Their current recipes
and schemas still name `forgecad.geometry.energy-core@1` and the robot/fictional
material packs; they require successor Bundle versions, not in-place edits.
The three `archive/superseded` Bundle families remain immutable.  The
repository-only `skills/forgecad-sci-fi-fps-weapon-production/**` deletion is
D-001 and its replacement is the Weaponry Skill; do not repeat that deletion.

## Quarantine and delete batches

### Q-001 — non-product proposals and packs

Quarantine candidates after successor references are removed and provenance is
frozen:

```text
packages/forgecad-skills/proposals/sci-fi-fps-weapon/1.0.0/**
packages/forgecad-assets/forgecad-hard-surface-robot/1.0.0/**
packages/forgecad-assets/forgecad-fictional-energy-weapon/1.0.0/**
packages/forgecad-assets/forgecad-fictional-energy-weapon-2k/1.0.0/**
packages/forgecad-assets/forgecad-fps-production-foundation/0.1.0-proposal/**
```

These are not safe to move on dirty `main`: Worker `include_str!` references,
active Bundle material enums, modified proposal text and historical receipts
still point at them.  Replacement requires a generic weapon AssetPack with
license, SBOM, hash and provenance receipts.

### D-003 — subject-specific route retirement

Do not delete the 34 fictional-energy routes or the 16 D1 FormArt routes until
the generic P0 façade has a bounded replacement, legacy records replay through
it, all route/Schema/manifest/test consumers are zero, and historical evidence
has been frozen or superseded without rewriting it.  Mechanical/game routes are
retained; High/Low/UV/Bake/delivery routes are retained or migrated by their
stage contract.

### D-004 — P0 façade rollout

This rollout is source-complete: typed façades are default and compatibility is
explicit. It is still not a deletion batch. Authorized CrossFire and original-control
visual cohorts have not been proven through the same Runtime build, so route/schema
physical deletion remains blocked. Never use the façade count as a visual or
commercial quality claim.

No historical `docs/evidence/**`, user reference, CAS object, candidate output,
or generated directory is a delete candidate.  Historical records may only be
frozen, archived or superseded with a pointer and recovery receipt.
