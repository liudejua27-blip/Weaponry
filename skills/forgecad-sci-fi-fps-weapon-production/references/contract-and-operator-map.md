# Contract and operator map

This reference is for the Codex orchestration Skill, not Runtime truth. Read it when
the user asks for an original fictional weapon-style asset or when a typed handoff is
being assembled.

## Typed entry and exit

Use `FictionalEnergyRifleProfile@1` as the bounded creative input and
`FictionalEnergyRiflePlan@1` as the structural planning output. The profile must keep:

- `scope: fictional-game-asset` and `nonfunctional_asset: true`;
- project, representation, coordinate-frame, and Operator Catalog SHA-256 bindings;
- `reference_policy.quality_claim_allowed: false` and
  `visual_match_claim_allowed: false`;
- `quality_contract.strict_glb_readback`, `joint_multiview_compare`,
  `pbr_after_silhouette_gate`, and `human_review_required` all true.

The plan is not a candidate. Its invariants are `quality_status: structural_only`,
`candidate_created: false`, `runtime_write_performed: false`, and
`hq_360_status: BLOCKED_REFERENCE_COVERAGE`.

## Bounded macro map

| Kit intent | Product-owned operator | Typical visual role |
| --- | --- | --- |
| `forgecad.kit.housing@1` | `forgecad.geometry.panel@1` | receiver shell, spine, shell layers |
| `forgecad.kit.panel@1` | `forgecad.geometry.panel@1` | inset plate, ridge, shell break |
| `forgecad.kit.frame@1` | `forgecad.geometry.panel@1` | internal frame or support silhouette |
| `forgecad.kit.vent@1` | `forgecad.geometry.vent-array@1` | vent or grille rhythm |
| `forgecad.kit.joint@1` | `forgecad.geometry.joint-stack@1` | hinge, collar, connector layer |
| `forgecad.kit.sensor@1` | `forgecad.geometry.primitive@2` | optic/sensor/emitter housing |

The Runtime must verify the live Operator Catalog before use. The Bundle lock is a
declaration, not an executable plugin. `part-output`, `mirror`, `array`, Boolean, and
surface operators may be used only when the live catalog and the relevant active
Skill explicitly advertise them; never infer availability from this map.

## Stage and quality rules

Use `primary-form → secondary-structure → tertiary-detail → uv-pbr → final-review`.
For each action preserve the same project/reference/candidate/camera/observation
lineage and rerun strict readback plus fixed AOV comparison. A single three-quarter
image can at most support a partial visible-view conclusion. The five identity views
`front/back/left/right/rear-three-quarter` are required for any 360 claim.

The Runtime owns all metrics and quality labels. Codex may explain a failed gate using
the returned evidence, but must not calculate a replacement score, promote a
candidate, or overwrite a QualityReport.

## Material and asset declarations

The draft Bundle includes material metadata only; no texture, model, or external asset
payload is present. Use the existing glTF metallic-roughness subset and preserve
base-color, metallic, roughness, normal, AO, emissive, and clearcoat semantics when
the live Appearance/AssetPack route is available. Every real asset requires a
content hash, SPDX license, license-text hash, provenance, allowed-use declaration,
and modification status. A missing or ambiguous license is a hard stop.

## Prohibited input and output

Reject paths, URLs, raw image bytes, shell/Python/JavaScript snippets, environment
variables, secrets, model/provider credentials, arbitrary Blender state, and named
commercial-game source material. Do not produce manufacturing drawings, dimensions,
materials recipes, performance claims, operating instructions, or safety conclusions.
