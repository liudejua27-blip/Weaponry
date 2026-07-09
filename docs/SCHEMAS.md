# Schema Contract

Schemas define the stable contract between frontend, Agent backend, asset library, and Unity export. JSON documents must include `schema_version` and be validated before they are written to the immutable asset store.

本文件按 **当前已落库** 与 **目标态建议** 两层维护，避免把“规划字段”与“当前可运行契约”混在同一层。

Schema files live in:

```text
packages/weapon-spec/schemas/
```

Generated contract artifacts:

```text
packages/weapon-spec/generated/types.ts
apps/agent/wushen_agent/generated/schema_registry.py
packages/weapon-spec/generated/openapi.json
apps/desktop/src/shared/generated/api-types.ts
```

Generation and drift check:

```text
npm run contracts:types:generate
npm run contracts:types:check
```

## Schema List

### 当前契约（代码可回放）

| File | Purpose |
| --- | --- |
| `weapon-design-spec.schema.json` | Canonical weapon art direction and generation plan |
| `asset-record.schema.json` | Asset library metadata record |
| `patch-manifest.schema.json` | Canvas mask and local edit instruction contract |
| `job-event.schema.json` | Append-only Agent event stream contract |
| `model-generation-input.schema.json` | Input to rough 3D generation |
| `unity-material.schema.json` | Unity Toon Shader material intent |
| `quality-report.schema.json` | Quality gate report for image/model/export |

### 目标态扩展契约（第一阶段设计目标）

| File | Purpose |
| --- | --- |
| `creative-weapon-graph.schema.json`（待补充） | 结构解释（任意输入重诠释）与`combat_affordances` |
| `skill-graph.schema.json`（待补充） | 绑定结构图的玩法/动作卡图 |
| `creative-interp-request.schema.json`（待补充） | 结构解释与候选返回参数 |
| `creative-interp-confirm.schema.json`（待补充） | 候选确认 + 版本引用 |

## Invariants

- `scale_policy` is always game-relative and must not contain real-world dimensions。
- API keys、provider secret、raw auth header 不得出现在 schema 字段中。
- 所有文件引用使用 asset id 或库内相对路径，禁止 exportable 文件写绝对路径。
- 生成资产必须能追踪到 `job_id`、`weapon_id`、`version_id`。
- Patch 版本必须引用 source version。
- Unity 导出包必须是快照；后续库变更不能改写已产出清单。
- `weapon_family` 只能作兼容回放字段，不参与第一分类路由。
- `creative_interpretation_response` 必须返回 `2~3` 条候选，禁止空列表与单类别硬编码。
- `creative_interpretation_confirm` 必须回传可追溯的 `interpretation_id` + `selected_candidate_id`。
- 目标态 `creative_graph` 与 `skill_graph` 必须形成主-从链路（`skill_graph.origin_graph_ref -> creative_graph_id`）。
- `interpretation` 阶段输出不得作为分类依据。第一主决策是 `combat_affordances`、`structure_graph`、`protected_regions`、`skill_anchor_points`。
- `ready` 或 `resampled_ready` 状态下 `candidate_count` 只能是 `2` 或 `3`；低于 2 条不是可用降级状态。
- 候选未确认错误属于 API 状态错误，统一使用 `INTERPRETATION_NOT_CONFIRMED`，不得用 `JOB_ACTION_CONFLICT` 混写。
- 4 层解释闭环是目标态硬约束（目标态字段级要求）：
  - 结构层：`structure_graph.nodes/edges/annotations` 与 `anchor_points` 必须可序列化；
  - 交互层：`interaction_graph`（可选，建议用于交互行为提示）；
  - 功能层：`combat_affordances` 至少 1 项，且与 `recast_profile` 不冲突；
  - 资产层：`protected_regions` 与 `skill_anchor_points` 必须存在（可空数组不合格时置为 blocker）。

## Versioning

Use semantic schema strings:

```text
WeaponDesignSpec@1
CreativeWeaponGraph@1 (target)
SkillGraph@1 (target)
CreativeInterpretationRequest@1 (target)
CreativeInterpretationResponse@1 (target)
CreativeInterpretationConfirm@1 (target)
JobEvent@1
AssetRecord@1
PatchManifest@1
ModelGenerationInput@1
UnityMaterial@1
QualityReport@1
```

`WeaponDesignSpec@2` 目标态会引用 `creative_graph_id` 和 `skill_graph_id`，并保留 `weapon_family` 兼容回放字段。

### `creative-weapon-graph.schema.json` 目标字段（当前设计目标）

```text
source_object: free text，保留原始输入引用（可为任意物件/几何）
recast_summary: 文本 + tag 列表（神化方式、角色映射、约束）
example_sources: ["防弹裤", "木棍", "椅子", "镜子", "伞", "门", "戒指", "树枝"]
combat_affordances: array（melee、pierce、range、energy、summon、shield、mobility、transform、reflect、healing、chain 等；至少 1 个）
structure_graph: {
  nodes: [
    { id, type(core/grip/source/emitter/motion_joint/protected), label, position, params },
    ...
  ],
  edges: [
    { from, to, relation(support/flow/connects/attaches/refract/recover) },
    ...
  ],
  zones: [
    { id, role, material, editable },
    ...
  ],
  annotations: [
    { kind(bind/emit/flow/material/skill_anchor/restricted/forbid), target_node_id, value },
    ...
  ],
  raw_annotations: object
}
protected_regions: [
  { id, zone_id, reason, editable }
]
skill_anchor_points: [
  { id, node_id, trigger, cooldown_hint, seed_payload, name, slot_hint }
]
versioning: {
  parent_graph_id,
  base_on,
  provenance,
  source_interpretation_id
}
```

`skill-graph.schema.json` 约束目标字段（当前设计目标）：

```text
skills: [
  {
    id, name, slot(normal/basic/charge/defense/mobility/control/passive/ultimate/summon),
    trigger, cost, cooldown, range, radius,
    effects: [{ type, params }],
    visuals: { vfx_anchor, color, anim_key },
    constraints: { can_cancel, can_chain, ai_hint }
  }
]
origin_graph_ref: 绑定 creative_graph_id
balance_tags: [ tag ]
meta: { origin_input, recast_mode, seed_reason }
```

`creative-interp-request.schema.json` / `creative-interp-response.schema.json` / `creative-interp-confirm.schema.json`（规划目标示意）：

```text
creative_interpretation_request = {
  source_object: string,                    # 原始语义输入（可为任意物件/几何文本）
  raw_description: string,                  # 用户原始补充说明（可空）
  # 非武器输入不是异常输入：source_object 可为“椅子/钥匙/防弹裤/镜子”等。
  weapon_id,
  source_input: { text, sketch_ref?, image_ref?, reference_assets? },
  affordance_hints: [string]?,             # 可选：用户想要的玩法倾向（如 area_control）
  freeform_seed: string?,                  # 可选：风格/形体自由度提示语
  candidate_count_hint: 3,                  # 运行时建议值，缺省 3，允许范围 2~3
  max_candidates: 3,
}

creative_interpretation_response = {
  interpretation_id,
  weapon_id,
  status: "ready" | "resampled_ready" | "failed",
  candidate_count,                         # ready/resampled_ready 时只能为 2 或 3
  needs_confirm: true,
  stable_seed: string,
  failure_code?: "PROVIDER_BAD_OUTPUT" | "INVALID_LLM_JSON",
  resample: {
    attempted: boolean,
    preserved_candidate_id?: string,
    reason?: "under_min_candidates" | "duplicate_affordance_axis" | "missing_required_fields"
  },
  candidate_sort_policy: {
    primary: "rank",
    secondary: "confidence",
    stable_seed
  },
  candidates: [
    {
      candidate_id,
      name,
      summary,
      recast_summary,
      rank,
      combat_affordances,
      confidence,
      anchor_points,
      protected_regions,
      risk_tags,
      skill_anchor_points,
      structure_graph,
      candidate_seed,                      # 与每条候选绑定的结构重构约束快照（可选）
    }
  ]
}

creative_interpretation_confirm = {
  interpretation_id,
  weapon_id,
  selected_candidate_id,
  selected_candidate_idempotency_token,     # 可选：重放防重放字段
  selected_candidate_rank,
  selected_structure_id,
  confirmed_at,
  recast_mode,
  client_request_id
}
```

约束说明：

- `candidates` 必须至少 2 个、最多 3 个；按 `rank` 升序、`confidence` 降序返回，稳定排序要求可复现实验复现同输入时输出顺序一致。
- 如果第一次 provider 输出不足 2 个候选，后端应执行一次重采样并保留一个稳定候选锚点；重采样后仍不足 2 个时，`status=failed`，`failure_code=PROVIDER_BAD_OUTPUT`，且不得写入可确认 `creative_graph`。
- `interpretation_id` 在一次 `/interpretation` 调用内固定；所有 `candidate_id` 仅在该 `interpretation_id` 命名空间内有效，`/recast/confirm` 必须验重。
- `/recast/confirm` 只允许 `selected_candidate_id` 落在当次 `interpretation_id` 命名空间内，且该候选 `rank` 必须在 `1~3`。
- `source_object` 与 `raw_description` 必须保留到版本链字段，作为后续审计和重建输入证据。
- `risk_tags` 由重铸模块生成，至少包含一个结构性风险标签（如 `pivot_sensitive`、`center_shift`、`overreach`）以支持后续 Patch 风险提示。
- `creative_interpretation_request` 可接受 `source_object` 与 `raw_description` 的非武器语义；实现不得因 `source_object` 不是武器名词而返回 `INVALID_REQUEST`。

Breaking schema changes require a migration plan and test fixtures for old assets.
