# Frontend Contract

The desktop frontend is a Tauri + React production workbench. It is not a landing page and not a generic chat UI.

## Routes

```text
/forge
/weapons/:weaponId
/weapons/:weaponId/versions/:versionId
/library
/jobs
/jobs/:jobId
/settings/providers
/settings/library
/about
```

## Module Layout

```text
apps/desktop/src/
  app/
  features/create/
  features/library/
  features/canvas/
  features/structure/
  features/skills/
  features/preview3d/
  features/jobs/
  shared/api/
  shared/tauri/
  shared/state/
  shared/types/
```

`shared/tauri/agentSupervisor.ts` is the only frontend module that may call Tauri `invoke` for Agent process management. In Tauri mode it supplies the Agent runtime `baseUrl` to the HTTP API client; in browser/Vite mode the API client remains compatible with `VITE_FORGE_API_BASE_URL` or the localhost default.

FastAPI only enables CORS for local Vite development origins. The frontend should not rely on browser-wide provider access; model, ComfyUI, and asset writes still go through the Agent API.

## State Ownership

- Server state: TanStack Query.
- UI state: Zustand or equivalent lightweight local store.
- Long-running jobs: backend event stream is the source of truth.
- Asset files: referenced by asset id; React state stores metadata only.

## API Client

Desktop API component types are generated from FastAPI OpenAPI:

```text
apps/desktop/src/shared/generated/api-types.ts
```

`apps/desktop/src/shared/types.ts` remains as a small alias layer so feature modules do not import generated files directly.

The API client wraps:

```text
createWeapon()
patchWeapon()
uploadVersionAsset()
interpretWeaponObject()
confirmCreativeRecast()
getCreativeGraph()
generateSkills()
generateSkillSlot()
generateRough3D()
getWeapon()
getWeaponVersions()
listWeapons()
getJob()
listJobs()
listJobActions()
subscribeJobEvents()
retryJob()
retryJobFromStep()
cancelJob()
getAssetMetadata()
getAssetFileUrl()
listProviders()
testProvider()
```

Every mutating method accepts `client_request_id` and sends `Idempotency-Key`.

UX 约束（与后端一致）：

- 创建页不得提供“武器类型/分类”下拉框，也不得在第一次输入时要求用户绑定到 `武器`、`weapon_family`。
- `interpretation` 面板必须展示 2~3 条候选，禁止显示 1 条以下的“默认分类模板”列表。
- interpretation 视图必须绑定 `interpretation_id` 与 `candidate_count`，候选数不是 2~3 时显示 `interpretation_not_ready`。
- 只有在候选确认成功后才开启概念图、Patch 与 3D 按钮；未确认前给出明确阻断提示。
- API 返回 `INTERPRETATION_NOT_CONFIRMED` 时，UI 只允许引导用户回到候选确认，不显示类型选择器。
- API 返回 `PROVIDER_BAD_OUTPUT` 且 `details.reason` 指向解释候选不足或缺字段时，UI 显示“重试解释/补充结构标注”，不得把该输入标为不可作为武器。

`Preview3DPanel` owns the first desktop generate-3d control. It selects the current `concept_patch` when present, otherwise the current or latest `concept_image`, submits `generateRough3D()`, forwards the returned job id to the app-level event stream, and refreshes weapon detail from the app-level refresh key so async worker-loop completion makes the new `rough_3d` version previewable without leaving the workbench.

## CanvasAdapter

```ts
interface CanvasAdapter {
  loadBackground(assetId: string): Promise<void>
  exportSketch(): Promise<CanvasExport>
  exportMask(selectionId: string): Promise<MaskExport>
  getPatchManifest(selectionId: string): PatchManifest
  getStructureExport(): Promise<{
    shapes: string[]
    bindings: Record<string, string>
    semanticAnnotations: Array<{ kind: string; value: unknown }>
    anchors: Array<{ id: string; x: number; y: number; kind: string }>
    protectedRegions: Array<{ id: string; vertices: Array<{ x: number; y: number }> }>
  }>
  getAnnotations(): Array<{ kind: string; value: unknown }>
}
```

Mask rules:

- white means repaint
- black means preserve
- mask dimensions equal the source concept image
- empty masks and mismatched masks fail before API submission

### 结构解释与神化标注（目标态）

前端自由画布目标态从“只做遮罩”扩展到“结构语义标注”：

- 骨架线
- 握持点
- 攻击源点
- 能量流向
- 材质分区
- 可动关节
- 技能锚点
- 保护区
- Patch mask 与结构说明可同源保留

结构模式提供“候选解释列表 + 用户确认按钮 + Patch/生成继续”：

- 用户先在画布看到 2~3 个结构解释候选；
- 选中一个解释后再发起概念图生成；
- 再次进入 Patch 时可改动结构锚点后快速重生成；
- 结构导出随版本保留，作为 `creative_graph_asset_id` 与 `skill_graph_id` 的输入。
- 每条候选必须带上最小结构注释（`anchor_points`）与受保护区域提示，避免后续局部修改改坏核心结构。
- 每次解释确认仅允许单选；候选列表应覆盖不同 `combat_affordances`（至少两条在能力上有显著差异）。

建议前端标注工具（第一目标）:

- 骨架线：体量、弯曲、对齐方向
- 握持点：单手、双手、背负、穿戴、悬浮
- 攻击源：刃口、炮口、阵眼、导能口、冲击区
- 能量流：从核心到终端的路径与方向
- 材质区：可视化的材质层级标注（骨、玉、金属、符纸等）
- 可动关节：展开/折叠/旋转/组合点
- 技能锚点：大招核心、被动触发点、蓄力点、召唤点
- 保护区：用户希望“保持不变”的禁改区域
- 目标层：`recast` 引导与结构约束标注（不包含制造参数）

M4 current frontend slice:

- `PatchModePanel` reads `GET /api/weapons/{weapon_id}` and finds the active `concept_image` or `concept_patch`.
- It loads the source image through the controlled `GET /api/assets/{asset_id}/file` endpoint.
- It renders a canvas overlay at the source image pixel size, supports brush and lasso mask tools, brush size control, overlay opacity control, undo/redo, and clearing.
- It uploads the mask through `uploadVersionAsset()`, creates and uploads `PatchManifest@1`, then calls `patchWeapon()`.
- When the selected version is a patch version, it resolves the parent version image and renders a slider comparison against the current patch result.
- It can activate the selected patch version, return the weapon to the parent version, or switch the editor back to the parent version for another patch attempt.
- This is the API-connected foundation for Patch Mode. Full provider-backed retry policies remain future Agent work.
`StructurePanel`（目标态）显示/编辑结构注释，支持 4 个自由度滑块：

- 形态自由度
- 神化程度
- 玩法复杂度
- 资产可用性

结构化生成时的前端原则：

- 用户不先选“武器类别”，而是先看到解释候选。
- 每次只允许选择一个结构解释作为当前生成入口。
- 每个解释候选展示：`combat_affordances`、`保护区域`、`技能锚点`、`失真风险提示`。
- `Patch` 与 `generate-3d` 共享同一 `creative_graph` 版本上下文，避免用户在局部修改后丢失解释闭环。
- `confirm` 面板中的候选渲染必须展示：
  - `name` / `summary`
  - `combat_affordances`
  - `anchor_points` 与 `protected_regions`
  - `risk_tags`（排序后显示）
  - `confidence`
- 候选列表默认按 `rank` 升序展示，并在 UI 文本中说明“可用性优先级与稳定度”。
- 候选不足 2 条时显示 `structure_missing` 提示并阻断概念图入口；若后端返回 `resample.attempted=false`，前端可触发一次“重试解释”，若返回 `PROVIDER_BAD_OUTPUT` 则要求用户补充草图标注或描述。
- 画布标注工具的导出结构必须与 `creative_graph` payload 一致（字段名：`anchor_points`、`protected_regions`、`combat_affordances`、`structure_graph`）。

前端验收建议（非类别回归）：

- 非武器输入（例如防弹裤/木棍/椅子/镜子/钥匙/花盆/风车）必须能进入解释面板。
- 对比两个候选在 UI 中至少显示 1 个不同 `combat_affordances` 组合（如 `shield+area_control` 与 `mobility+projectile`）。
- “重试解释”只允许在同一 `interpretation_id` 里替换低置信候选，已确认项不变更；用户取消确认后才可重新触发全量候选。

结构标注优先策略（目标态）：

- 默认仅显示结构解释，不显示“类型模板”入口。
- 仅单选一个候选；切换候选后自动置空 `concept/patch/3d` 按钮，并更新确认状态文案。
- `Patch` 与 `generate-3d` 按钮始终读取当前 `creative_graph_id` 与 `version_id`，避免“局部修改后从原始候选继续生成”。

## GLB Preview

```ts
type GlbPreviewInput = {
  assetId: string
  glbUrl: string
  unityMaterialUrl?: string
  previewMode: 'solid' | 'toon' | 'wireframe' | 'normal'
}
```

Preview requirements:

- lazy-load the 3D panel
- show recoverable load errors
- auto-center model by bounding box
- support reset view, wireframe, toon/solid toggle, screenshot
- dispose geometry, material, and textures when switching models

M5 current frontend slice:

- `Preview3DPanel` reads `listWeapons()` and `GET /api/weapons/{weapon_id}` to resolve previewable rough models.
- It prefers the current version's `rough_raw_glb`; if the current version is a patch without regenerated 3D, it falls back to the latest available `rough_raw_glb`.
- It loads the model through the controlled `GET /api/assets/{asset_id}/file` endpoint with Three.js `GLTFLoader`.
- It renders a WebGL exhibition scene in the inspector, with pedestal, simple character, held weapon, auto-rotation, pointer drag rotation, `toon`, `solid`, `wireframe`, reset, and screenshot controls.
- It can submit generate-3d and export-unity jobs from the same 3D panel, then forwards the returned job id to the global event stream.
- It refreshes after a new job is created so the newly generated weapon becomes the default preview target.
- `LibraryPanel` reads `GET /api/weapons/{weapon_id}` and exposes a version-by-version asset handoff view: concept images, patch images, GLB variants, Unity material JSON, quality reports, and Unity export ZIP assets all use controlled asset file URLs.
- Browser verification sampled the WebGL canvas after loading, confirmed nonblank pedestal/character/held-weapon pixels, verified drag rotation changes the canvas checksum, and captured `output/playwright/m5-final-preview3d-panel.png`.
- Browser verification also captured `output/playwright/m5-library-handoff.png`, showing asset library handoff links for GLB, Unity material, quality report, and Unity export ZIP assets.

P0 unified workspace context slice:

- `App` now owns the selected weapon id, selected version id, active job id, and latest loaded `WeaponDetail`.
- `CreateWeaponPanel`, `LibraryPanel`, `PatchModePanel`, and `Preview3DPanel` report accepted jobs and selected weapon/version changes back to `App`.
- Forge, Library, Patch, Inspector, and 3D Preview now share the same active weapon/version context instead of each panel silently drifting to its own first list item.
- The main stage now renders the current asset context: weapon name, structure tag（兼容字段）、stage、active version、source concept/patch image、rough GLB status、Unity export status、and the non-manufacturing safety boundary.
- The Inspector now starts with a context summary for weapon id, version id, model id, available asset roles, and safety boundary before the 3D preview controls.
- Browser verification captured `output/playwright/p0-unified-context-workbench.png`; selecting a different weapon in Library updated the top bar, main stage, Inspector, and 3D weapon select to the same weapon/version.
- Browser verification also captured `output/playwright/p0-library-selection-sync.png`，展示同一武器条目在 Library、top bar、主舞台、Inspector 与 3D Preview 中一致高亮。示例版本为 `weapon_0491be7f34ff / ver_97e93060390d`，遗留 100 B mock GLB 仅用于兼容性错误边界验证。

P0 recoverable Agent trace slice:

- `App` now keeps `JobDetail`, active job id, event history, stream status, and action feedback together. When a job is accepted, the desktop calls `GET /api/jobs/{job_id}` to hydrate existing events, replaces the current timeline with events for that job only, restores the related weapon detail, and stores `wushen.recentJobId` for restart recovery.
- SSE subscription now resumes from the latest known event id through the existing `after` parameter, ignores events for other jobs, and reports `job.error` frames such as `INVALID_EVENT_CURSOR` in the action feedback area. The frontend now sorts by public `JobEvent.seq`, with event-id suffix and `created_at` only kept as legacy fallback.
- `JobTimeline` is now an Agent trace drawer: task summary, progress bar, stream state, grouped step cards with Chinese labels, artifact id, metadata summary, and recovery action row.
- `forgeApi` exposes `retryJob()`, `retryJobFromStep()`, and `cancelJob()` through generated `JobActionResponse`. Buttons are state-gated: retry actions require a failed step, cancel requires a running/waiting status, and skip-3D is only enabled for 3D-related failures.
- Backend retry/cancel/retry-from now persist action requests: `generation_jobs` state changes, `job_steps` attempt/cancel state changes, `job_actions` audit rows, and append-only `agent_events` with public `seq`. Rough3d cancel now calls the Provider cancel boundary when an active provider task id exists; generic checkpoint execution across all steps is still future backend worker work.
- Runtime `retrying` is no longer treated as resumable, so a retry request does not leave the UI offering another retry action that the backend would reject. Failed and waiting-user jobs remain resumable; active/waiting jobs remain cancellable.
- `forgeApi` also exposes `getJobRuntime()` and `recoverRuntime()` for the runtime UI slice. These endpoints surface provider task/checkpoint metadata and conservative restart recovery. For rough3d jobs, provider task status can now move through submitted/polling/succeeded/cancel_requested/cancelled; checkpoint execution beyond the current 3D provider boundary remains backend work.
- Async generate-3d compatibility slice: the 3D panel treats non-terminal `JobAcceptedResponse.status` values such as `queued` as accepted background work instead of assuming a GLB exists immediately; the Agent trace maps queued/running/waiting-provider/retry/cancel states to Chinese labels; the App refreshes weapon detail on terminal job events while keeping intermediate worker events in the drawer.
- Async export-unity compatibility slice: export worker jobs now use the same Agent trace drawer and localized `export_plan`, `export_manifest`, `export_package`, `finalize_job` step labels.
- Job-output context continuity is now explicit: when a restored or terminal job exposes `outputs.current_version_id`, `App` selects that output version after loading weapon detail. This keeps top bar, Inspector, Patch Mode, 3D Preview, and Library synchronized after patch, generate-3d, and export jobs instead of preserving a stale pre-job version.
- Patch Mode now initializes its mask canvas idempotently against the active source asset id and pixel dimensions after the canvas ref exists. This prevents a visible source image from pairing with a default 300x150 canvas and producing `MASK_SIZE_MISMATCH` on submit.
- Runtime Trace Mini Panel is now rendered inside `JobTimeline`. `App` owns the active `JobRuntimeStateResponse`, refreshes it through `GET /api/jobs/{job_id}/runtime`, and passes it to both the Task Center timeline and the bottom drawer. The panel shows runtime status/current step, latest provider task, active checkpoint, resumable/cancellable state, and provider last-seen; rough3d provider statuses are localized as submitted/polling/cancel requested/cancelled/succeeded rather than shown only as raw enum text.
- Unity handoff visibility is now part of the 3D and Library surfaces. `Preview3DPanel` derives a conservative handoff state from `WeaponDetail.versions[].assets` plus `current_model`, then shows raw/normalized/optimized GLB, Unity material, quality report, export ZIP, model id/status, quality status, fallback GLB, and old-ZIP warnings. `LibraryPanel` shows a per-version Unity handoff checklist so asset completeness is visible without opening every file row.
- `npm run desktop:p0-runtime-handoff-smoke` now scripts this browser verification with `playwright-core` and system Chrome. It starts an isolated FastAPI Agent and Vite frontend on random ports, injects `WUSHEN_CORS_ORIGINS`, seeds mock generate-3d/export-unity worker data, restores `wushen.recentJobId`, and asserts the Runtime Trace Mini Panel, Unity handoff card, controlled asset file links, cross-version Library handoff coverage, and nonblank/interactive WebGL canvas.
- Browser verification captures `output/playwright/p0-runtime-handoff-workbench.png`, `output/playwright/p0-runtime-handoff-runtime.png`, `output/playwright/p0-runtime-handoff-card.png`, and `output/playwright/p0-runtime-handoff-library.png`, showing provider task/checkpoint runtime state, the Unity handoff card with GLB/material/report/ZIP state plus fallback messaging, and Library handoff rows across rough/export versions.
- `npm run desktop:p0-context-continuity-smoke` now scripts the real UI continuity path: Forge create, Patch Mode brush mask, patch submit, generate-3d request from the resulting `concept_patch`, export-unity request with the generated model id, and Library selection sync. It captures `output/playwright/p0-context-patch-brush.png`, `output/playwright/p0-context-patch-comparison.png`, `output/playwright/p0-context-3d-handoff.png`, and `output/playwright/p0-context-library-sync.png`.
- `npm run desktop:p0-job-action-state-smoke` now scripts failed/retry/cancel/recovery action-state coverage. It restores synthetic failed, waiting-provider, and recovered jobs, asserts recovery action buttons, clicks retry/retry-from/cancel, verifies action responses and runtime provider task state, and captures `output/playwright/p0-job-trace-failed-retry.png`, `output/playwright/p0-job-trace-retry-from.png`, `output/playwright/p0-job-trace-waiting-provider-cancel.png`, and `output/playwright/p0-job-trace-recovered-waiting-user.png`.
- P0 Task Center history slice: `/jobs` now renders `JobCenterPanel` in the main workspace instead of squeezing a second `JobTimeline` into the left rail. It provides server-backed historical job search, status filtering, failure-code filtering, manual job id restore, selected-job detail loading, runtime mini panel, failure reason panel, and `job_actions` audit list. Selecting a history row only inspects it; `恢复到工作台` or manual restore calls the app-level `restoreJob()` and subscribes that job as the active context.
- `forgeApi.listJobs()` wraps `GET /api/jobs` with `query/status/job_type/error_code/cursor/limit`, and `forgeApi.listJobActions()` wraps `GET /api/jobs/{job_id}/actions`. The Task Center treats these read models as source-of-truth for history and action audit rather than scraping text from the timeline.
- `npm run desktop:p0-job-center-history-smoke` now scripts the Task Center browser verification. It seeds succeeded, failed, retryable, and waiting-provider jobs, verifies history search, failed/error-code filtering, manual job-id restore, retry-from action audit refresh, waiting-provider cancel, and captures `output/playwright/p0-job-center-history.png`.
- Browser verification captured `output/playwright/p0-jobtimeline-success-trace.png`, showing a successful mock create job as 7 grouped Agent steps with artifact ids and provider metadata rather than a flat event list.
- Browser verification also captured `output/playwright/p0-jobtimeline-task-center.png`, showing the same recovered trace in the Task Center view.

## Error UX

## 当前桌面信息架构（CAD-only）

桌面端已移除迁移前 Forge、Patch、任务中心、资产库和独立设置页面。`App` 只渲染 `CadWorkbenchPanel`：项目和版本位于左侧，真实注册模块位于底部组件抽屉，参数/检查/导出集中在右侧；所有设计修改先形成 ChangeSet 预览，再创建不可变版本。历史段落仅记录旧界面的迁移证据，不代表当前产品入口。

Errors are shown in three places:

- inline in the current step
- in the append-only job timeline
- as a recovery action: retry step, open settings, select file, cancel, or skip 3D

Error codes must match `docs/API.md`.

## Frontend Tests

- Unit: type guards, event reducers, error mapping, URL/path helpers.
- API contract: mock create weapon, patch, generate 3D, job events, reconnect.
- Component: task timeline, asset library, provider settings, patch form.
- Canvas pixel tests: mask size, black/white semantics, edge cases.
- R3F/GLB tests: load fixture GLB, auto-center, error boundary, dispose.
- E2E: text prompt -> job events -> concept asset -> patch -> GLB preview.
