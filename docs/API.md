# API Contract

This document defines the production contract between the Tauri/React desktop app and the local Python FastAPI Agent service.

Current contract (v1, kept for compatibility) is still driven by `WeaponDesignSpec@1`.
Target contract is structure-first and category-agnostic:

```text
text/sketch -> structure interpretation -> Creative Recast -> CreativeWeaponGraph -> WeaponDesignSpec -> concept image -> patch -> skill graph -> rough 3D -> Unity export folder
```

Current v1 loop:

```text
text/sketch -> WeaponDesignSpec@1 -> concept image -> patch version -> asset library -> rough 3D model -> Unity export folder
```

兼容说明：

- `WeaponDesignSpec@1` 与 `weapon_family` 暂时仍保留为兼容字段，不作为核心抽象。
- 下一阶段将把 `WeaponDesignSpec@1` 与 `weapon_family` 降级为 `creative_graph_id` / `skill_graph_id` 入口；`weapon_family` 不再作为分类决策主键。

## Principles

- The frontend never calls LLM, ComfyUI, or 3D providers directly.
- Long-running work returns `job_id` immediately and streams progress through job events.
- Current mock implementation still completes some jobs inside the request handler; the runtime metadata contract below is the bridge toward true queued worker execution.
- 目标态流程不按武器类别路由；`interpretation` 与 `recast/confirm` 先于概念图/3D 执行，`weapon_family` 不能决定首要分支。
- `POST /api/weapons` 兼容入口不得绕过候选确认闭环；`auto_run` 时默认触发 `interpretation`，并返回解释闭环上下文。
- `interpretation` 是第一决策产物：未进入确认状态前不允许触发 concept/patch/generate-3d/export-unity。
- 结构闭环必须通过 4 层表达产出：`structure_graph`、`combat_affordances`、`protected_regions`、`skill_anchor_points`；先行决策不得是“是否像某类武器”。
- 4 层表达要求：
  - 结构层：`structure_graph` 与 `anchors`（握持点、攻击源、核心轴线、可动关节、连接关系）
  - 交互层：`interaction_graph`（吸附/旋转/位移/放置行为）
  - 功能层：`combat_affordances`（至少 1 项）
  - 资产层：`protected_regions`、`recast_profile`、`unity_handoff`（socket / scale_policy / axis_hint）
- Every mutating request accepts an idempotency key.
- All generated files are committed through the AssetStore, not arbitrary paths.
- API keys are never returned by the API and never written into events, SQLite rows, asset files, or export packages.
- Browser dev builds may call FastAPI from localhost Vite ports only. Production desktop access should still go through the controlled local Agent endpoint.

## Common Headers

```text
X-Wushen-Client-Version: desktop version
Idempotency-Key: required for POST requests that create work
```

`Idempotency-Key` is scoped to the active library. The backend must return the existing job if the same key and request hash are submitted again.

## Status Enums

`JobStatus`:

```text
created | queued | running | waiting_provider | waiting_user | retrying | succeeded | failed | cancelled | partial_succeeded
```

`StepStatus`:

```text
queued | running | waiting_provider | succeeded | failed | skipped | cancelled
```

`AssetStage`:

```text
draft | concept | patched | rough_3d | exported | failed
```

## Error Envelope

All non-2xx responses use:

```json
{
  "error": {
    "code": "PROVIDER_UNCONFIGURED",
    "message": "ComfyUI provider is not configured.",
    "recoverable": true,
    "details": {}
  }
}
```

Error codes:

```text
INVALID_REQUEST
IDEMPOTENCY_CONFLICT
AGENT_OFFLINE
PROVIDER_UNCONFIGURED
PROVIDER_AUTH_FAILED
PROVIDER_TIMEOUT
PROVIDER_BAD_OUTPUT
INVALID_LLM_JSON
COMFYUI_WORKFLOW_INVALID
ASSET_FILE_MISSING
ASSET_PERMISSION_DENIED
MASK_EMPTY
MASK_SIZE_MISMATCH
GLB_INVALID
GLB_TOO_LARGE
QUALITY_CHECK_FAILED
SAFETY_BOUNDARY_BLOCKED
LOCAL_IO_ERROR
RATE_LIMITED
JOB_ACTION_CONFLICT
INTERPRETATION_NOT_CONFIRMED
INVALID_INTERPRETATION_ID
INVALID_INTERPRETATION_CANDIDATE
INVALID_EVENT_CURSOR
JOB_NOT_FOUND
WEAPON_NOT_FOUND
VERSION_NOT_FOUND
```

解释闭环错误码规则：

- `INTERPRETATION_NOT_CONFIRMED`：用户或 `auto_run` 在未执行 `/recast/confirm` 前调用 `concept / patch / generate-3d / export-unity`。
- `PROVIDER_BAD_OUTPUT`：LLM/provider 返回的解释候选在一次重采样后仍不足 2 条，或缺失 `combat_affordances`、`anchor_points`、`protected_regions`、`risk_tags` 等必需字段。
- `INVALID_INTERPRETATION_ID`：确认请求引用不存在、过期、已废弃或不属于该 `weapon_id` 的 `interpretation_id`。
- `INVALID_INTERPRETATION_CANDIDATE`：候选 id 不属于本次解释、`selected_candidate_rank` 不匹配、候选已被替换，或候选状态不是可确认。
- `JOB_ACTION_CONFLICT` 只用于 retry/cancel 等任务动作冲突；不得替代解释未确认错误。

## Create Weapon

```text
POST /api/weapons
```

Request:

```json
{
  "client_request_id": "req_20260704_0001",
  "text": "防弹裤神化为腰部炮台，带能量纹路与风雷护甲环",
  "sketch_asset_id": null,
  "reference_asset_ids": [],
  "auto_run": true,
  "target": {
    "phase": "concept_to_rough_3d",
    "engine": "unity"
  }
}
```

Response `202 Accepted`:

```json
{
  "weapon_id": "weapon_20260704_0001",
  "job_id": "job_20260704_0001",
  "status": "queued",
  "needs_confirm": true,
  "candidate_count": 2,
  "interpretation_id": "interp_20260704_0001",
  "candidate_sort_policy": {
    "primary": "rank",
    "secondary": "confidence",
    "stable_seed": "20260704::auto_run"
  },
  "event_stream_url": "/api/jobs/job_20260704_0001/events"
}
```

Rules:

- `auto_run=true` 下，`POST /api/weapons` 不能以单条类型模板直接进入概念图；必须先走结构解释。
- `POST /api/weapons` 不再使用武器类别作为前置条件：`text`、`sketch_asset_id`、`reference_asset_ids` 必须先进入可解释结构分支。
- 当 `auto_run=true` 时，首次执行必须返回 `interpretation_id`、`needs_confirm=true` 与 `candidate_count=2|3`；不允许返回单一默认类型候选。
- 同一个输入重复跑时，允许轻微抖动，但至少保留一个候选 `rank` 与 `combat_affordances` 的核心方向；否则先重采样一次，仍失败则返回 `PROVIDER_BAD_OUTPUT`。
- `generate-3d` / `export-unity` 在未确认前一律返回 `INTERPRETATION_NOT_CONFIRMED`。
- The 3D step only starts after image quality checks pass.
- The backend may return `partial_succeeded` later if concept generation succeeds but rough 3D fails.
- `source_object` 与 `recast_summary` 在目标态作为结构解释与重构结果返回。当前实现保持兼容，在现版本响应中可作为 `metadata` 扩展字段预留。
- `weapon_family` 不被视为主流程分类字段；其作用是兼容检索与回放。
- 推荐兼容场景也返回 `creative_graph_id + skill_graph_id` 与 `structure_interpretation_id`，确保回溯链路与编辑历史一致。

示例字段扩展（v2目标）：

```json
{
  "weapon_id": "weapon_20260704_0001",
  "job_id": "job_20260704_0001",
  "status": "queued",
  "event_stream_url": "/api/jobs/job_20260704_0001/events",
  "needs_confirm": true,
  "candidate_count": 2,
  "source_object": "pants",
  "recast_summary": "腰部环形炮台/护甲强化架构",
  "combat_affordances": ["blunt", "area_control", "mobility"],
  "creative_graph_id": "cg_20260704_0001",
  "skill_graph_id": "sg_20260704_0001"
}
```

## List Weapons

```text
GET /api/weapons?stage=rough_3d&query=雷步腰炮神兵&limit=50&cursor=...
```

Response:

```json
{
  "items": [
    {
      "weapon_id": "weapon_20260704_0001",
      "display_name": "雷步腰炮神兵",
      "weapon_family": "wearable_artifact",
      "stage": "rough_3d",
      "source_object": "pants",
      "recast_summary": "腰部环形炮台与防御外壳",
      "combat_affordances": ["projectile", "mobility", "defense"],
      "creative_graph_asset_id": "cg_20260704_0001",
      "skill_graph_asset_id": "sg_20260704_0001",
      "current_version_id": "ver_0001",
      "current_model_id": "model_0001",
      "thumbnail_asset_id": "file_preview_0001",
      "updated_at": "2026-07-04T22:00:00+08:00"
    }
  ],
  "next_cursor": null
}
```

## Get Weapon Detail

```text
GET /api/weapons/{weapon_id}
```

Response:

```json
{
  "weapon_id": "weapon_20260704_0001",
  "display_name": "雷步腰炮神兵",
  "stage": "rough_3d",
  "current_version_id": "ver_0001",
  "versions": [
    {
      "version_id": "ver_0001",
      "parent_version_id": null,
      "job_id": "job_0001",
      "version_no": 1,
      "version_type": "rough_3d",
      "status": "committed",
      "assets": [
        {
          "asset_id": "file_concept_0001",
          "role": "concept_image",
          "version_id": "ver_0001",
          "logical_path": "weapons/weapon_20260704_0001/versions/ver_0001/concept.svg",
          "sha256": "64-char-sha256",
          "byte_size": 12345,
          "mime_type": "image/svg+xml",
          "width": 1280,
          "height": 720,
          "created_at": "2026-07-04T22:00:00+08:00"
        }
      ]
    }
  ],
  "current_spec": {},
  "current_model": {},
  "source_object": "pants",
  "recast_summary": "腰部环形炮台与防御外壳",
  "combat_affordances": ["projectile", "mobility", "defense"],
  "creative_graph_asset_id": "cg_20260704_0001",
  "skill_graph_asset_id": "sg_20260704_0001",
  "latest_jobs": []
}
```

Rules:

- Desktop Patch Mode uses this endpoint to locate the current version and its latest `concept_image` or `concept_patch` asset.
- Asset file bytes still require the controlled asset file API; this detail endpoint only returns metadata.

## Structure Interpretation and Creative Recast

目标态新增端点（目标合同）：

```text
POST /api/weapons/{weapon_id}/interpretation
POST /api/weapons/{weapon_id}/recast/confirm
GET /api/weapons/{weapon_id}/creative-graph
POST /api/weapons/{weapon_id}/skill-graph
```

设计说明：

- `interpretation` 接收 `source_object`（可为任意文本描述、草图、参考图）并返回候选结构解释（`structure_candidates`）供用户确认。
- `recast/confirm` 将用户确认的结构解释固化为 `CreativeWeaponGraph@1`，作为 `WeaponDesignSpec` 的结构来源（兼容阶段可附在 `metadata`）。
- `creative-graph` 查询当前 `creative_graph` 版本、`combat_affordances`、`structure`（骨架线、握持点、攻击源、可动关节、技能锚点）与掩模信息。
- `skill-graph` 在确认结构后的阶段可生成 6 个技能卡（普通攻、防御/位移、控制、被动、终结、复制/回收），并与版本上下文绑定。
- `skill-graph` 还允许目标态“按技能槽重生成单卡”：传入 `regen_slots` 时只重跑对应槽位。

目标态约束：

- `interpretation` 必须返回 `2~3` 条候选（超出时按 `rank` 取前 3 条）。
  - 排序策略必须返回 `candidate_sort_policy`，包含 `stable_seed`（用于同输入复现）。
  - 候选不足 2 条时必须先进入一次重采样子流；重采样后仍不足 2 条，或候选全量退化时，返回 `PROVIDER_BAD_OUTPUT`。
- 每条候选必须有 `candidate_id`、`rank`、`confidence`、`combat_affordances`（至少 1 项）、`anchor_points`、`protected_regions`、`risk_tags` 和 `structure_graph`。
- 每个 `candidate_id` 只允许在一次 `interpretation_id` 内使用；`recast/confirm` 必须校验候选归属关系。
- `source_object` 与 `raw_description` 允许是非武器类名词，后端不得用 `weapon_family` 或固定类型模板拒绝请求。
- 未完成候选确认前不得触发 `concept`、`patch`、`generate-3d` 或 `export-unity`，统一返回 `INTERPRETATION_NOT_CONFIRMED`。

最小交互样例（目标态）：

```json
POST /api/weapons/{weapon_id}/interpretation

{
  "source_object": "chair",
  "raw_description": "把这把椅子做成可反射技能波的领域装置",
  "sketch_asset_id": null,
  "reference_asset_ids": [],
  "client_request_id": "req_interp_0001"
}
```

响应（示例）：

```json
{
  "interpretation_id": "interp_20260704_0001",
  "weapon_id": "weapon_20260704_0001",
  "status": "ready",
  "needs_confirm": true,
  "candidate_count": 2,
  "candidate_sort_policy": {
    "primary": "rank",
    "secondary": "confidence",
    "stable_seed": "chair::req_interp_0001"
  },
  "structure_candidates": [
    {
      "candidate_id": "cand_01",
      "name": "王座防御台",
      "summary": "腰部环形防御体 + 领域反射面",
      "combat_affordances": ["shield", "area_control"],
      "confidence": 0.91,
      "rank": 1,
      "anchor_points": ["knee_core", "waist_socket"],
      "protected_regions": ["waist_plate"],
      "risk_tags": ["pivot_sensitive", "center_shift_high"],
      "skill_anchor_points": ["domain_reflect_core"],
      "structure_graph": {"nodes": [], "edges": [], "zones": []}
    },
    {
      "candidate_id": "cand_02",
      "name": "折叠回旋炮座",
      "summary": "折叠态双发射口，位移后连发",
      "combat_affordances": ["ranged", "reflect", "summon"],
      "confidence": 0.82,
      "rank": 2,
      "anchor_points": ["chassis_axis", "knee_emitter"],
      "protected_regions": ["seat_core", "emitter_mount"],
      "risk_tags": ["folding_overlap", "silhouette_fragile"],
      "skill_anchor_points": ["folding_burst_port"],
      "structure_graph": {"nodes": [], "edges": [], "zones": []}
    }
  ]
}
```

确认与固化：

```json
POST /api/weapons/{weapon_id}/recast/confirm

{
  "interpretation_id": "interp_20260704_0001",
  "selected_candidate_id": "cand_02",
  "selected_candidate_rank": 2,
  "recast_mode": "stylized_artifact",
  "recast_choice_text": "折叠回旋炮座",
  "client_request_id": "req_recast_0001"
}
```

响应（示例）：

```json
{
  "weapon_id": "weapon_20260704_0001",
  "creative_graph_id": "cg_20260704_0001",
  "skill_graph_id": "sg_20260704_0001",
  "status": "confirmed",
  "next": "create_weapon"
}
```

说明：

- 推荐 `interpretation` 默认返回 2~3 个候选；未选中时仍保留 `needs_confirm=true`，不默认切换主流程到概念图。
- `recast/confirm` 阶段只负责“结构闭环确认”，概念图生成仍走常规生成链路。
- 建议将 `source_object` 与 `raw_description` 的自由文本长度都保留在 16~2500 字节内；空描述必须回退到 `INVALID_REQUEST`。

目标态校验补充：

- `selected_candidate_id` 不能跨轮复用；只在同一 `interpretation_id` 生效。
- `selected_candidate_id` 不能为空。
- `selected_candidate_rank` 必须落在 `1~3`。
- `selected_candidate_id` 与 `selected_candidate_rank` 不一致时返回 `INVALID_INTERPRETATION_CANDIDATE`。
- 确认成功后，后续 `concept / patch / generate-3d / export-unity` 必须携带或解析到同一 `creative_graph_id`；不一致时返回 `INTERPRETATION_NOT_CONFIRMED`。

示例：目标态 `POST /api/weapons/{weapon_id}/skill-graph`（按槽重生成）

```json
{
  "creative_graph_id": "cg_20260704_0001",
  "regen_slots": ["basic", "defense", "ultimate"],
  "client_request_id": "req_skill_0002"
}
```

## Patch Mode Flow

Patch Mode uses a two-step local API flow:

```text
upload patch_mask + patch_manifest assets
-> POST /api/weapons/{weapon_id}/patch
```

## Upload Version Asset

```text
POST /api/weapons/{weapon_id}/versions/{version_id}/assets
```

M4 only accepts `patch_mask` and `patch_manifest`. The payload is JSON + base64 so the desktop app never asks the backend to read arbitrary user paths.

Request for `patch_mask`:

```json
{
  "client_request_id": "req_upload_mask_0001",
  "role": "patch_mask",
  "filename": "surface-mask.png",
  "mime_type": "image/png",
  "data_base64": "iVBORw0KGgo...",
  "metadata": {
    "source": "canvas"
  }
}
```

Request for `patch_manifest`:

```json
{
  "client_request_id": "req_upload_manifest_0001",
  "role": "patch_manifest",
  "filename": "patch-manifest.json",
  "mime_type": "application/json",
  "data_base64": "eyJzY2hlbWFfdmVyc2lvbiI6...",
  "metadata": {
    "source": "canvas"
  }
}
```

Response `201 Created`:

```json
{
  "weapon_id": "weapon_20260704_0001",
  "version_id": "ver_0001",
  "asset_id": "file_mask_0001",
  "role": "patch_mask",
  "logical_path": "weapons/weapon_20260704_0001/versions/ver_0001/uploads/structural-mask.png",
  "sha256": "64-char-sha256",
  "byte_size": 12345,
  "mime_type": "image/png",
  "width": 1280,
  "height": 720
}
```

Rules:

- `Idempotency-Key` is required.
- `patch_mask` must be PNG. The backend parses its real pixel width/height during upload.
- `patch_manifest` must validate against `PatchManifest@1` and match the URL `weapon_id`.
- Upload stores immutable objects and returns asset ids. It does not trigger generation by itself.
- The patch job still performs the final mask/source size check and empty-mask check before provider calls.

## Patch Concept Image

```text
POST /api/weapons/{weapon_id}/patch
```

Request:

```json
{
  "client_request_id": "req_patch_0001",
  "source_version_id": "ver_0001",
  "source_image_asset_id": "file_concept_0001",
  "mask_asset_id": "file_mask_0001",
  "patch_manifest_asset_id": "file_patch_manifest_0001",
  "target_area": "主体承力段/核心区",
  "instruction": "把核心光环边缘纹理提亮，保留整体剪影和国风构成感",
  "preserve": ["overall_silhouette", "chinese_motifs", "toon_outline"],
  "strength": "medium",
  "regenerate_3d": false,
  "provider_id": "mock_comfyui"
}
```

Response `202 Accepted`:

```json
{
  "weapon_id": "weapon_20260704_0001",
  "job_id": "job_patch_0001",
  "status": "queued"
}
```

Rules:

- M4 backend mock patch is implemented at the AssetStore layer.
- 目标态 Patch 只能针对已确认 `creative_graph_id` 的版本执行；未确认时返回 `INTERPRETATION_NOT_CONFIRMED`。
- `mask_asset_id` and `patch_manifest_asset_id` must refer to committed assets with roles `patch_mask` and `patch_manifest`.
- Patch never overwrites the source version.
- Patch output creates a new `weapon_versions` row.
- Patch output uses role `concept_patch` and keeps the same width/height as the source image.
- Empty or size-mismatched masks are rejected before provider calls.
- Patch prompt and manifest must stay inside the fictional Unity game-art boundary and must not request real-world manufacturing drawings, dimensions, material formulas, or fabrication steps.

## Activate Weapon Version

```text
POST /api/weapons/{weapon_id}/versions/{version_id}/activate
```

Response `200 OK` is the fresh `WeaponDetail` for the weapon.

Rules:

- The target version must belong to the weapon and have `status='committed'`.
- The endpoint only changes `weapons.current_version_id`; it does not create new assets or overwrite existing versions.
- Patch Mode uses this endpoint to accept a patch result as current or roll back to the parent version after comparison.

## Generate Rough 3D

```text
POST /api/weapons/{weapon_id}/generate-3d
```

Request:

```json
{
  "client_request_id": "req_3d_0001",
  "source_version_id": "ver_0002",
  "source_image_asset_id": "file_concept_or_patch",
  "provider_id": "mock_3d",
  "target_format": "glb",
  "style": "stylized_toon_weapon",
  "orientation_policy": {
    "forward_axis": "+Z",
    "long_axis": "+Y",
    "pivot": "grip_center"
  },
  "scale_policy": "normalized_game_asset_scale",
  "build_unity_export": true
}
```

Response `202 Accepted`:

```json
{
  "weapon_id": "weapon_20260704_0001",
  "job_id": "job_3d_0001",
  "status": "succeeded",
  "event_stream_url": "/api/jobs/job_3d_0001/events"
}
```

Current M5 behavior:

- The endpoint requires `Idempotency-Key`.
- 目标态 `generate-3d` 必须从已确认 `creative_graph_id` 的概念图或 Patch 版本发起；未确认时返回 `INTERPRETATION_NOT_CONFIRMED`。
- `source_version_id` must belong to the weapon and be committed.
- `source_image_asset_id` must point to a `concept_image` or `concept_patch` in the source version.
- Successful jobs append a new `weapon_versions(version_type='rough_3d')` row with `parent_version_id=source_version_id`.
- Default development behavior remains synchronous and returns `status='succeeded'` after mock 3D assets are committed.
- When `WUSHEN_GENERATE3D_WORKER=1`, `WUSHEN_GENERATE3D_ASYNC=1`, `WUSHEN_GENERATE_3D_ASYNC=1`, or `WUSHEN_GENERATE3D_RUNTIME=worker` is set, the endpoint returns `status='queued'` and persists only the job, queued steps, checkpoints, and initial event. It does not create `rough_3d` versions, models, or GLB assets until the local worker runs.
- `WUSHEN_GENERATE3D_WORKER=1` starts an opt-in local background worker loop at app startup and also implies async generate-3d enqueue. `WUSHEN_GENERATE3D_WORKER_INTERVAL_SECONDS` controls idle polling, and `WUSHEN_GENERATE3D_WORKER_ID` controls the lease runner id.
- `POST /api/runtime/work-once` remains a local/test stepping hook. It claims one queued/retrying/waiting-provider generate-3d job and advances it one provider boundary step without starting the always-on loop.
- Async generate-3d uses the provider boundary `submit_rough_model -> poll_rough_model -> fetch_rough_model`. If poll returns `submitted`, `polling`, or `unknown`, the job remains `waiting_provider`, records provider task/checkpoint state, and writes no `rough_3d` assets. A later worker tick polls the same provider task id and only fetches/commits assets after provider success.
- `WUSHEN_MOCK_3D_POLL_SEQUENCE=polling,succeeded` can be used in tests to simulate a real async 3D provider. The default mock sequence remains `succeeded` for fast local development.
- `WUSHEN_3D_PROVIDER=local_http` selects the first real-runtime adapter boundary. It requires `WUSHEN_3D_HTTP_BASE_URL` or `WUSHEN_3D_BASE_URL`; optional knobs are `WUSHEN_3D_HTTP_PROVIDER_ID`, `WUSHEN_3D_HTTP_TIMEOUT_SECONDS`, `WUSHEN_3D_HTTP_POLL_INTERVAL_SECONDS`, `WUSHEN_3D_HTTP_MAX_WAIT_SECONDS`, `WUSHEN_3D_HTTP_RETRY_ATTEMPTS`, `WUSHEN_3D_HTTP_RETRY_BACKOFF_SECONDS`, and `WUSHEN_3D_HTTP_API_KEY`.
- Successful jobs write `ModelGenerationInput@1`, `rough_raw_glb`, `rough_normalized_glb`, `rough_optimized_glb`, `unity_material_json`, and model `quality_report` assets. The model quality report parses the optimized GLB JSON chunk and records triangle, vertex, mesh, primitive, material, texture, image, PBR, finite bounds, center, extents, and longest-axis evidence for asset-library gates.
- The source version and source image are never overwritten.

### Local HTTP 3D Provider Protocol

The `local_http` provider is the stable boundary for wrapping Stable Fast 3D, TripoSR, Hunyuan3D, or a custom local image-to-3D service without embedding those heavy runtimes inside the desktop Agent.

The repo includes a first runtime wrapper service at `scripts/wushen_local_3d_runtime.py`. It implements the same protocol below and is intentionally a separate process from FastAPI/Tauri so model installs, GPU libraries, and crashes stay outside the desktop Agent.

Run deterministic mock mode:

```bash
.venv/bin/python scripts/wushen_local_3d_runtime.py \
  --backend mock \
  --host 127.0.0.1 \
  --port 8787
```

Connect the Agent to it:

```bash
WUSHEN_3D_PROVIDER=local_http
WUSHEN_3D_HTTP_BASE_URL=http://127.0.0.1:8787
WUSHEN_GENERATE3D_ASYNC=1
```

Run Stable Fast 3D CLI mode after installing a local SF3D checkout:

```bash
.venv/bin/python scripts/wushen_local_3d_runtime.py \
  --backend sf3d-cli \
  --sf3d-repo /absolute/path/to/stable-fast-3d \
  --sf3d-python /absolute/path/to/sf3d/python \
  --texture-resolution 1024
```

Run TripoSR CLI mode after installing a local TripoSR checkout:

```bash
.venv/bin/python scripts/wushen_local_3d_runtime.py \
  --backend triposr-cli \
  --triposr-repo /absolute/path/to/TripoSR \
  --triposr-python /absolute/path/to/triposr/python \
  --triposr-device mps \
  --triposr-runner "$PWD/scripts/triposr_mps_runner.py"
```

Environment equivalents are `WUSHEN_LOCAL_3D_BACKEND`, `WUSHEN_LOCAL_3D_HOST`, `WUSHEN_LOCAL_3D_PORT`, `WUSHEN_LOCAL_3D_WORK_DIR`, `WUSHEN_LOCAL_3D_TASK_TIMEOUT_SECONDS`, `WUSHEN_LOCAL_3D_MOCK_DELAY_SECONDS`, `WUSHEN_LOCAL_3D_KEEP_WORK_DIR`, `WUSHEN_SF3D_REPO`, `WUSHEN_SF3D_PYTHON`, `WUSHEN_SF3D_TEXTURE_RESOLUTION`, `WUSHEN_SF3D_REMESH_OPTION`, `WUSHEN_TRIPOSR_REPO`, `WUSHEN_TRIPOSR_PYTHON`, `WUSHEN_TRIPOSR_RUNNER`, `WUSHEN_TRIPOSR_DEVICE`, `WUSHEN_TRIPOSR_PRETRAINED_MODEL`, `WUSHEN_TRIPOSR_CHUNK_SIZE`, `WUSHEN_TRIPOSR_MC_RESOLUTION`, `WUSHEN_TRIPOSR_BAKE_TEXTURE`, `WUSHEN_TRIPOSR_TEXTURE_RESOLUTION`, and `WUSHEN_TRIPOSR_NO_REMOVE_BG`.

Current automated verification: `npm run agent:p0-local-3d-runtime-wrapper-smoke` starts the wrapper as a real subprocess in mock mode, drives the Agent async worker through submit -> wait -> fetch, verifies GLB asset commits, and verifies cancellation reaches the runtime before any late asset write.

Current manual verification: `npm run agent:p0-local-3d-runtime-sf3d-manual` requires `WUSHEN_SF3D_REPO` and starts the same wrapper in `sf3d-cli` mode against a real Stable Fast 3D checkout. `npm run agent:p0-local-3d-runtime-triposr-manual` requires `WUSHEN_TRIPOSR_REPO` and starts the wrapper in `triposr-cli` mode against a real TripoSR checkout. Installation and release criteria are documented in [Local 3D Runtime](LOCAL_3D_RUNTIME.md). Real GPU validation and provider-specific restart resume are still release work.

Submit:

```text
POST {WUSHEN_3D_HTTP_BASE_URL}/v1/rough-models
```

Request body:

```json
{
  "schema_version": "WushenThreeDProviderRequest@1",
  "weapon_id": "weapon_0001",
  "model_id": "model_0001",
  "source_image_asset_id": "file_concept",
  "source_image": {
    "logical_path": "weapons/weapon_0001/versions/ver_0001/concept.png",
    "mime_type": "image/png",
    "data_base64": "..."
  },
  "target_format": "glb",
  "style": "stylized_toon_weapon",
  "orientation_policy": {"forward_axis": "+Z", "long_axis": "+Y", "pivot": "grip_center"},
  "scale_policy": "normalized_game_asset_scale",
  "output_contract": {
    "asset_type": "fictional_game_art",
    "non_manufacturing_asset": true,
    "preferred_format": "glb"
  }
}
```

Submit response:

```json
{
  "provider_task_id": "sf3d_task_001",
  "status": "polling",
  "metadata": {"engine": "stable-fast-3d"}
}
```

Poll:

```text
GET {base_url}/v1/rough-models/{provider_task_id}
```

Poll response:

```json
{
  "provider_task_id": "sf3d_task_001",
  "status": "polling",
  "progress": 0.45,
  "metadata": {"engine": "stable-fast-3d"}
}
```

`status` may be `submitted`, `polling`, `succeeded`, `failed`, `cancelled`, or `unknown`. Failed responses may include `error.code` and `error.message`.

Fetch result:

```text
GET {base_url}/v1/rough-models/{provider_task_id}/result
```

Result response:

```json
{
  "provider_task_id": "sf3d_task_001",
  "raw_glb_base64": "...",
  "normalized_glb_base64": "...",
  "optimized_glb_base64": "...",
  "unity_material_json": {"schema_version": "UnityMaterial@1"},
  "metrics": {"triangle_count": 12000, "mesh_count": 1, "material_count": 1},
  "metadata": {"engine": "stable-fast-3d", "non_manufacturing_asset": true}
}
```

`raw_glb_base64` is required. `normalized_glb_base64` and `optimized_glb_base64` may be omitted; the Agent will carry the previous stage forward. The adapter validates the GLB magic/version/declared length before writing any asset rows.

Cancel:

```text
POST {base_url}/v1/rough-models/{provider_task_id}/cancel
```

Cancel response:

```json
{
  "provider_task_id": "sf3d_task_001",
  "status": "cancelled",
  "metadata": {"engine": "stable-fast-3d"}
}
```

`status` may be `cancelled`, `cancel_requested`, `unsupported`, or `unknown`.

## Export Unity Package

```text
POST /api/weapons/{weapon_id}/export-unity
```

Request:

```json
{
  "client_request_id": "req_export_0001",
  "model_id": "model_rough_0001",
  "export_type": "unity_glb",
  "include_source_spec": true,
  "include_quality_reports": true
}
```

Response `202 Accepted`:

```json
{
  "weapon_id": "weapon_20260704_0001",
  "job_id": "job_export_0001",
  "status": "succeeded",
  "event_stream_url": "/api/jobs/job_export_0001/events"
}
```

Current M5 behavior:

- The endpoint requires `Idempotency-Key`.
- 目标态 `export-unity` 必须包含已确认 `creative_graph_id` 与 `skill_graph_id` 的溯源快照；缺失确认链路时返回 `INTERPRETATION_NOT_CONFIRMED`。
- `model_id` may be omitted; the latest model for the weapon is used.
- Default development behavior remains synchronous and returns `status='succeeded'` after the Unity ZIP snapshot is committed.
- When `WUSHEN_EXPORT_UNITY_ASYNC=1`, `WUSHEN_EXPORT_UNITY_WORKER=1`, or `WUSHEN_RUNTIME_WORKER=1` is set, the endpoint returns `status='queued'` and persists only the job, queued steps, checkpoints, and initial event. It does not create an `export` version, `export_packages` row, or `unity_export_package` ZIP asset until the local worker runs.
- `WUSHEN_EXPORT_UNITY_WORKER=1` and `WUSHEN_RUNTIME_WORKER=1` start the local background worker loop at app startup. `WUSHEN_LOCAL_WORKER_INTERVAL_SECONDS` controls idle polling, and `WUSHEN_LOCAL_WORKER_ID` controls the lease runner id.
- Successful jobs append a new `weapon_versions(version_type='export')` row with `parent_version_id` pointing to the model version.
- The package is stored as a `unity_export_package` asset with `mime_type='application/zip'`.
- `export_packages` records a `UnityExportManifest@1` snapshot and a relative package path.
- ZIP entries are Unity-friendly relative paths under `Assets/WushenForge/Weapons/{weapon_id}/`.
- The ZIP contains `Models/rough_optimized.glb`, `Materials/unity_material.json`, `Specs/weapon_spec.json`, `Reports/model_quality_report.json`, `manifest.json`, and `README_WUSHEN.txt`.
- 目标态 ZIP 还应包含 `Specs/creative_weapon_graph.json` 与 `Specs/skill_graph.json`，用于结构与技能溯源。
- The package manifest declares the fictional game-art / non-manufacturing boundary.

## Job Detail

```text
GET /api/jobs/{job_id}
```

Response:

```json
{
  "job_id": "job_20260704_0001",
  "weapon_id": "weapon_20260704_0001",
  "status": "running",
  "current_step": "image_poll",
  "created_at": "2026-07-04T22:00:00+08:00",
  "started_at": "2026-07-04T22:00:03+08:00",
  "finished_at": null,
  "outputs": {
    "current_version_id": "ver_0001",
    "current_model_id": "model_0001",
    "asset_ids": ["file_spec", "file_prompt", "file_workflow", "file_concept", "file_quality"],
    "asset_roles": {
      "file_workflow": "comfyui_workflow",
      "file_quality": "quality_report"
    }
  },
  "steps": []
}
```

## Job Events

```text
GET /api/jobs/{job_id}/events?after=evt_0001
```

Transport is SSE in M2 because it maps well to append-only events. The endpoint supports `?after=evt_id`, legacy `?last_event_id=evt_id`, and `Last-Event-ID`.

If `after` references an event id that does not belong to this job, the stream emits a `job.error` frame with `INVALID_EVENT_CURSOR` rather than silently returning an empty stream.

SSE frame:

```text
id: evt_0002
event: job.event
data: {"id":"evt_0002", "...":"..."}
```

Error frame:

```text
event: job.error
data: {"error":{"code":"INVALID_EVENT_CURSOR","message":"Unknown event cursor for this job: evt_missing"}}
```

Event:

```json
{
  "id": "evt_0002",
  "job_id": "job_20260704_0001",
  "weapon_id": "weapon_20260704_0001",
  "step": "image_submit",
  "level": "info",
  "status": "succeeded",
  "message": "ComfyUI workflow submitted.",
  "artifact_asset_id": "file_concept",
  "metadata": {
    "provider": "comfyui",
    "provider_task_id": "prompt_abc",
    "workflow_asset_id": "file_workflow",
    "progress": 0.62
  },
  "created_at": "2026-07-04T22:00:10+08:00"
}
```

Traceability rules:

- `outputs.asset_roles` must expose `comfyui_workflow` and `quality_report` for successful create-weapon jobs.
- `image_submit` event metadata records `provider_task_id` and `workflow_asset_id`.
- `image_quality_check` event metadata records `target_asset_id`, `target_sha256`, and `quality_report_asset_id`.
- `rough3d_submit` event metadata records `gated_by=<quality_report_asset_id>`.

## Job History and Audit

```text
GET /api/jobs?query=&status=&job_type=&error_code=&cursor=&limit=
GET /api/jobs/{job_id}/actions?cursor=&limit=
```

`GET /api/jobs` is the task-center read model. It returns lightweight job summaries only; full events stay behind `GET /api/jobs/{job_id}` and SSE.

Supported filters:

- `query`: partial match against job id, weapon id, current step, error code, error message, or weapon display name.
- `status`: exact `JobStatus`.
- `job_type`: exact `create_weapon`, `patch_image`, `generate_3d`, or `export_unity`.
- `error_code`: exact failure code such as `PROVIDER_TIMEOUT`.
- `cursor`: keyset cursor from the previous response.
- `limit`: 1-100, default 25.

Response:

```json
{
  "items": [
    {
      "job_id": "job_20260704_0001",
      "weapon_id": "weapon_20260704_0001",
      "weapon_name": "雷步腰炮神兵",
      "type": "generate_3d",
      "status": "failed",
      "current_step": "rough3d_submit",
      "error_code": "PROVIDER_TIMEOUT",
      "error_message": "Provider did not return before timeout.",
      "event_count": 8,
      "action_count": 1,
      "latest_event_status": "failed",
      "latest_event_message": "Rough 3D provider timeout.",
      "output_version_id": null,
      "output_model_id": null,
      "created_at": "2026-07-04T22:00:00+08:00",
      "updated_at": "2026-07-04T22:05:00+08:00",
      "finished_at": "2026-07-04T22:05:00+08:00"
    }
  ],
  "next_cursor": null
}
```

`GET /api/jobs/{job_id}/actions` returns the durable user-action audit for one job:

```json
{
  "items": [
    {
      "action_id": "action_20260704_0001",
      "job_id": "job_20260704_0001",
      "action_type": "retry_from_step",
      "requested_step": "rough3d_submit",
      "status": "accepted",
      "previous_job_status": "failed",
      "resulting_job_status": "retrying",
      "event_id": "evt_job_20260704_0001_0010",
      "message": "Retry requested from step rough3d_submit.",
      "metadata": {
        "retry_from": "rough3d_submit"
      },
      "created_at": "2026-07-04T22:06:00+08:00"
    }
  ],
  "next_cursor": null
}
```

`GET /api/jobs/{job_id}` now also fills `error.code` and `error.message` from `generation_jobs.error_code/error_message` when a job failed.

## Job Runtime State

```text
GET /api/jobs/{job_id}/runtime
POST /api/runtime/recover
POST /api/runtime/work-once
```

`GET /runtime` returns the provider task and checkpoint read model used by recovery UI and the future worker:

```json
{
  "job_id": "job_20260704_0001",
  "status": "waiting_provider",
  "current_step": "rough3d_submit",
  "resumable": false,
  "cancellable": true,
  "provider_tasks": [
    {
      "task_record_id": "ptask_0001",
      "job_id": "job_20260704_0001",
      "step": "rough3d_submit",
      "attempt": 1,
      "provider_kind": "three_d",
      "provider_id": "mock_3d",
      "provider_task_id": "mock_3d_model_0001",
      "status": "polling",
      "cancel_requested_at": null,
      "last_seen_at": "2026-07-04T22:00:10+08:00",
      "metadata": {},
      "created_at": "2026-07-04T22:00:03+08:00",
      "updated_at": "2026-07-04T22:00:10+08:00"
    }
  ],
  "checkpoints": [
    {
      "checkpoint_id": "ckpt_0001",
      "job_id": "job_20260704_0001",
      "step": "rough3d_submit",
      "attempt": 1,
      "status": "ready",
      "resume_policy": "restart_step",
      "provider_task_record_id": "ptask_0001",
      "state": {
        "resume_from": "rough3d_submit"
      },
      "created_at": "2026-07-04T22:00:03+08:00",
      "updated_at": "2026-07-04T22:00:10+08:00"
    }
  ]
}
```

`POST /api/runtime/recover` scans active jobs after startup or supervisor recovery. Current behavior is conservative: interrupted active jobs are paused as `waiting_user`, a recovery event is appended, and the runtime checkpoint is marked for manual review. The endpoint does not execute provider polling or checkpoint resume yet.

```json
{
  "recovered_count": 1,
  "items": [
    {
      "job_id": "job_20260704_0001",
      "weapon_id": "weapon_20260704_0001",
      "previous_status": "running",
      "status": "waiting_user",
      "resume_from_step": "rough3d_submit",
      "provider_task_id": "mock_3d_model_0001",
      "event_id": "evt_job_20260704_0001_0008",
      "message": "Agent restart recovery paused job at step rough3d_submit."
    }
  ]
}
```

`POST /api/runtime/work-once` is an opt-in test/local stepping hook for the local worker. In normal worker-loop mode, set `WUSHEN_GENERATE3D_WORKER=1`, `WUSHEN_EXPORT_UNITY_WORKER=1`, or `WUSHEN_RUNTIME_WORKER=1` and let startup claim queued jobs automatically. The stepping hook claims at most one queued/retrying/waiting-provider `generate_3d` job or one queued/retrying `export_unity` job and returns:

```json
{
  "claimed": true,
  "job_id": "job_3d_0001",
  "job_type": "generate_3d",
  "status": "succeeded",
  "message": "Worker completed generate-3D job with status succeeded."
}
```

## Retry and Cancel

```text
POST /api/jobs/{job_id}/retry
POST /api/jobs/{job_id}/retry-from/{step_name}
POST /api/jobs/{job_id}/cancel
```

Retry response:

```json
{
  "action_id": "action_20260704_0001",
  "job_id": "job_20260704_0001",
  "status": "retrying",
  "previous_status": "failed",
  "current_step": "image_generator",
  "event_id": "evt_job_20260704_0001_0012",
  "message": "Retry requested from step image_generator.",
  "event_stream_url": "/api/jobs/job_20260704_0001/events",
  "retry_from": "image_generator"
}
```

Cancel response:

```json
{
  "action_id": "action_20260704_0002",
  "job_id": "job_20260704_0001",
  "status": "cancelled",
  "previous_status": "waiting_provider",
  "current_step": "image_submit",
  "event_id": "evt_job_20260704_0001_0013",
  "message": "Cancel requested for job at step image_submit.",
  "event_stream_url": "/api/jobs/job_20260704_0001/events"
}
```

Current implementation note: retry, retry-from-step, and cancel now persist job action requests. Accepted actions update `generation_jobs`, write a `job_actions` audit row, and append an `agent_events` action event with public `seq`. Cancel also marks a known active rough3d provider task as `cancel_requested` and calls provider cancel; providers that confirm cancellation, including `mock_3d`, update the provider task to `cancelled`. Generic checkpoint resume across all steps is still future worker work.

## Asset Files

```text
GET /api/assets/{file_id}
GET /api/assets/{file_id}/file
POST /api/assets/{file_id}/reveal
POST /api/assets/import
```

`GET /api/assets/{file_id}` returns metadata. `GET /file` streams the immutable object-store file by asset id with permission checks, library-root containment checks, and sha256 verification. `POST /reveal` asks the local Agent to open the asset in the OS file manager after the same containment/hash checks; `dry_run=true` validates the request for automated tests without opening a window. The API never accepts arbitrary file paths for reads or reveal actions, and reveal responses do not return local absolute paths.

Import request:

```json
{
  "client_request_id": "req_import_0001",
  "role": "reference",
  "source": {
    "kind": "user_selected_file",
    "path": "/user/selected/path/reference.png"
  }
}
```

## Provider Settings

```text
GET /api/provider-settings
```

Provider response never includes secrets:

```json
{
  "providers": [
    {
      "provider_id": "openai_compatible_llm",
      "kind": "llm",
      "type": "openai_compatible",
      "display_name": "OpenAI-compatible LLM",
      "enabled": false,
      "status": "missing_config",
      "base_url": "https://api.openai.com/v1",
      "has_secret": false,
      "updated_at": "2026-07-05T00:00:00+08:00"
    }
  ]
}
```

Future provider management endpoints:

```text
PUT /api/providers/{provider_id}
POST /api/providers/{provider_id}/test
```

M3 only exposes read-only provider state from environment configuration.

## Health

```text
GET /api/health
GET /api/health/providers
```

`/health` is used by the Tauri shell to detect whether the Agent sidecar is online.
