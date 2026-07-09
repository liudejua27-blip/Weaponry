# Implementation Plan

This plan turns the production design into buildable milestones. The goal is a production-grade first phase, not a throwaway prototype.

Product boundary: every milestone ships fictional game-art weapon appearance assets for Unity workflows. The system may pursue high visual realism, but it must not output real-world manufacturable weapon drawings, dimensions, material recipes, fabrication steps, assembly instructions, or process parameters.

## 设计更新运行规则（goal mode）

每次“设计完成”后执行同一组闭环动作：

- 更新边界：确认本次目标、范围和非制造约束仍完整。
- 更新文档：先更新 `README.md` 和 `docs/DESIGN.md` 对应章节。
- 固化计划：`IMPLEMENTATION_PLAN.md` 增加本轮新发现、验收优先级、阻塞项和下一步行动。
- 验收证据：没有可复现验证项之前，不将设计提升为下一阶段。
- 责任分配：当前阶段最多 8 个子 Agent，超过 4 人位子时仅允许短期专家位（Security/Unity/Provider/Packaging）。

执行顺序约束：

1. 先定义功能目标和质量门（safety / provider / packaging / docs）。
2. 再确认 `API` / `SCHEMA` / `DATABASE` / `FRONTEND` 合同是否需要更新。
3. 设计后在 `M* Notes` 和 gates 中补齐脚本或证据项。
4. 只有在证据条目可复用前，才触发实现切片。

GPT Pro 方案落地执行规则（建议本阶段前置）：

- 优先冻结“结构解释闭环”而非模型渲染细节：只要 interpretation + recast/confirm + 2~3 候选可稳定复用，才能进入本轮概念图优化。
- 所有新增子智能体行动以“证据先行”为前提，先补脚本/复现参数，再补文档，再开实现。
- 并发受限：系统默认 `8`，任何阶段如需超过则以职责复用替代新并发实例。
- 对非武器输入回归，加入每周“复跑一致性”条目，不满足时优先阻断 provider 优化而非放宽分类规则。

阶段推进规则（硬约束）：

- 只要有一个生产门禁为 blocker（安全、密钥、provider、打包、Unity 或 release 文档），本轮不进入下一阶段，直接把该项留到下一轮 `本轮设计闭环待办`。
- 每一轮必须至少形成“阻塞项证据 + 下一步动作 + 负责人”三件套；缺一不可则视为未闭环。

证据模板：

- `output/release/README.md`
- `output/release/_TEMPLATE/report.json`
- `output/release/_TEMPLATE/trace.txt`
- `output/release/_TEMPLATE/artifacts.txt`

当前门禁快照（本轮基线）：

| 记录ID | 门禁 | 状态 | 证据来源 | 失败归档分类 | 负责人 |
| --- | --- | --- | --- | --- | --- |
| GATE-01 | `release:safety-scope` | 进行中（已落地边界条款） | `scripts/check_release_safety_scope.py`、`docs/DESIGN.md`、`docs/API.md`、`docs/UNITY_IMPORT_SMOKE.md` | `scope_violation`、`non_manufacturing_drift`、`safety_phrase_missing` | Quality & Safety / Verification |
| GATE-02 | `release:secrets-files` | 待排查 | `scripts/check_release_secrets_files.py`、`apps/desktop/src-tauri/tauri.conf.json`、`apps/agent/wushen_agent/asset_store.py` | `secret_literal`、`path_leak`、`reveal_exposure`、`absolute_path_reject` | Verification / Packaging |
| GATE-03 | `release:prompt-quality` | 待补齐 | `scripts/check_release_prompt_quality.py`、`docs/PROMPT_QUALITY_SET.md` | `prompt_coverage_gap`、`quality_threshold_shortfall`、`negative_prompt_missing` | Quality & Safety |
| GATE-04 | `release:docs-walkthrough` | 待补齐 | `scripts/check_release_docs_walkthrough.py`、`docs/QUICKSTART.md`、`docs/API.md`、`docs/M4_PATCH_ASSETSTORE.md` | `walkthrough_gap`、`endpoint_mismatch`、`script_ref_missing` | Backend Architecture / Verification |
| GATE-05 | `release:packaging-readiness` | 待处理 | `scripts/check_release_packaging_readiness.py`、`docs/PACKAGING.md`、`apps/desktop/src-tauri/` | `sidecar_binary_missing`、`externalbin_mismatch`、`packaged_mode_missing`、`packaging_artifacts_missing` | Packaging/Distribution |
| GATE-06 | `release:license-sbom` | 待确认 | `scripts/check_release_license_sbom.py`、`package-lock.json`、`apps/agent/requirements-release.lock`、`docs/THIRD_PARTY_LICENSES.md` | `license_forbidden`、`lockfile_missing`、`external_review_pending` | Packaging / Verification |
| GATE-07 | `provider manual sf3d/triposr` | 待真实验收 | `scripts/smoke_p0_local_3d_runtime_sf3d_manual.py`、`scripts/smoke_p0_local_3d_runtime_triposr_manual.py`、`docs/LOCAL_3D_RUNTIME.md` | `backend_install`、`no_glb_output`、`invalid_glb`、`timeout`、`oom` | Provider Specialist |
| GATE-08 | `runtime recovery` | 待补齐 | `scripts/smoke_p0_provider_runtime_boundary.py`、`scripts/smoke_p0_runtime_recovery.py`、`scripts/smoke_p0_generate3d_worker_loop.py` | `cursor_invalid`、`cancel_conflict`、`checkpoint_stale`、`retry_state_mismatch` | Runtime / Backend Architecture |
| GATE-09 | `runtime boundary` | 基础有，但未闭环 | `scripts/smoke_p0_runtime_recovery.py`、`GET /api/jobs/{job_id}/runtime`、`apps/agent/wushen_agent/main.py`、`apps/agent/wushen_agent/asset_store.py` | `runtime_action_mismatch`、`cancel_not_propagated`、`runner_lease_stuck` | Runtime / Backend Architecture |
| GATE-10 | `unity import gate` | 阻塞（环境依赖） | `npm run unity:import:gate`、`docs/UNITY_IMPORT_SMOKE.md` | `unity_not_configured`、`unity_import_failed`、`manifest_path_invalid` | Verification / Provider Specialist |

## 目标优先级（执行顺序）

按本轮文档重构目标固定下一轮顺序：

1. `M6` 自由结构生成闭环：`CreativeWeaponGraph` 文档合同、Creative Recast Agent、结构解释候选确认、自由度滑块、`SkillGraph`。
2. `GATE-03` / `GATE-04`：prompt quality 与 docs walkthrough 先跟随结构解释闭环升级。
3. `GATE-01` / `GATE-02`：安全边界与密钥/文件边界复核，确保高拟真外观不漂移到现实制造说明。
4. `GATE-07` / `GATE-08`：真实 3D provider 与 runtime recovery 在结构合同冻结后继续推进。
5. `GATE-05` / `GATE-10`：桌面打包与 Unity gate 在 provider/runtime 证据稳定后推进。

本轮新增约束：不因“类别模板”扩展 scope。每一轮设计提交只允许新增结构解释、可选解释候选、affordance 及其在 3D 交付链路中的约束映射。

验收剧本（示例）：

- 防弹裤神炮
- 木棍大炮
- 镜子召唤门
- 椅子王座炮台
- 铃铛封印阵
- 树枝龙骨炮

阶段动作：

- 先把 `CreativeWeaponGraph`、`combat_affordances`、`SkillGraph` 以及结构解释闭环写进 DESIGN/API。
- 再把 `structure_version / skill_version` 引入 SCHEMAS/DATABASE/FRONTEND 文档。
- 最后补齐 QUICKSTART/PROMPT_QUALITY_SET 的非标准输入示例与目标兼容说明。

说明：

- 当前门禁矩阵的优先级规则与 `README.md`、`docs/DESIGN.md`保持一致。
- 新一轮实现只可围绕“非 blocker 项”的具体任务和验证动作展开。
- 证据默认落入 `output/release/<记录ID>/`：`report.json`（命令输出）、`trace.txt`（执行日志）、`artifacts.txt`（截图/模型/ZIP/日志清单）。

## 本轮设计闭环待办（生产级 goal 下一步）

| 记录ID | 负责人 | 目标 | 证据与验收 | 优先级 |
| --- | --- | --- | --- | --- |
| ACT-01 | 后端架构/文档 | 完成 `CreativeWeaponGraph` 与 `SkillGraph` 目标契约 | 在 `DESIGN/API/SCHEMAS/DATABASE/FRONTEND` 明确 `creative_graph_id`、`skill_graph_id` 与结构解释字段；输出兼容方案 | P0 |
| ACT-02 | Frontend Agent A/B | 完成结构解释选择 UI 与 4 个自由度滑块设计 | `FRONTEND/QUICKSTART` 覆盖结构候选确认、重试解释、滑块状态、Patch/3D 阻断规则 | P0 |
| ACT-03 | Quality & Safety Agent | 固化自由输入 prompt 集与非制造边界 | `PROMPT_QUALITY_SET` 覆盖非武器对象、2~3 候选、稳定排序、无制造参数 | P0 |
| ACT-04 | Backend Architecture | 将“第一阶段决策冻结 v0.1”写入 DESIGN/README 与脚本配置门禁清单 | 运行配置项覆盖 structure-first、Creative Recast、SkillGraph、3D 展台，不以 `weapon_family` 为主入口 | P0 |
| ACT-05 | Provider Specialist | 完成真实 3D provider 比对（SF3D / TripoSR） | `agent:p0-local-3d-runtime-sf3d-manual`、`agent:p0-local-3d-runtime-triposr-manual`，提交输出对比和失败分类 | P1 |
| ACT-06 | Runtime Agent | 补齐恢复与 cancel 边界 | provider `task_id` 恢复策略、超时/暂停/重试映射、worker checkpoint 重建路径的 gate 补充 | P1 |
| ACT-07 | Packaging/Distribution Agent | 清理打包 readiness blocker | `Cargo.lock`、sidecar 目标三端二进制、`release:packaging-readiness` 可通过前置项 | P1 |
| ACT-08 | Verification Agent | 推动真实 Unity gate 上线 | `npm run unity:import:gate` 从 blocked -> imported 的证据链 | P1 |

新增本轮动作

- ACT-09：统一非特化输入闭环
  - 输出：在 `DESIGN/API/SCHEMAS/FRONTEND/QUICKSTART` 增加“解释候选 + 用户确认”协议样例；更新脚本门禁中与解释链路相关的证据路径。
  - 验收：非武器对象（如 pants、椅子、镜子）可通过 `interpretation` 端到端生成 `2~3` 候选并确认后才进入概念图。
  - 扩展：补充 8 并发约束下的 12 角色映射说明（角色映射为执行策略，不是并发扩张理由）。
- ACT-09.1：统一解释闭环错误码与重采样状态
  - 输出：`API/SCHEMAS/DATABASE/FRONTEND/QUICKSTART/PROMPT_QUALITY_SET` 明确 `INTERPRETATION_NOT_CONFIRMED`、`PROVIDER_BAD_OUTPUT`、`INVALID_INTERPRETATION_*` 的边界。
  - 验收：候选不足 2 条时先重采样一次；重采样仍失败时不能继续 concept/3D/export，且前端不回退到“选择武器类型”。
  - 证据：`release:docs-walkthrough` 能扫描到候选确认、错误码、重采样、非分类入口四类文案。

执行规则：

- 每项待办至少绑定一个文档位置（README / DESIGN / IMPLEMENTATION_PLAN / M* Notes / gate 脚本）。
- 未产生可复现实证据的设计建议不进入 M 里程碑，先作为下轮待办。

### 本轮可执行设计行动（执行记录）

将待办按“输出可复现证据”切成 1~2 天内闭环动作。每项只允许出现一个状态，完成后必须更新到下一版本并归档截图/日志。

### GPT Pro 纵向闭环（5 条验收路径）

每轮进入开发前必须先形成以下 5 条闭环记录，缺一不可：

1. 任意输入 -> 结构解释候选（interpretation）
2. 候选确认 -> 创造性结构固定（creative graph + skill graph 入口）
3. 结构固定 -> 概念图生成与质检通过
4. 概念图/Patch -> 3D 展台预览（含 360）
5. 3D 交付 -> Unity ZIP + 资产清单 + manifest 一致性

闭环验收规则：

- 每条闭环都必须包含失败归类（至少一个失败码）。
- 每条闭环必须有至少一条可复现命令与输出目录（含 `trace.txt`）。
- 任何闭环缺失都视为阻塞，不可进入下一开发动作。

| 执行ID | 动作 | 负责方 | 本轮输出 | 失败时重试策略 | 一次完成标准 |
| --- | --- | --- | --- | --- | --- |
| ACT-02.1 | 侧链式 sidecar 打包路径确认 | Packaging/Distribution | `docs/PACKAGING.md` 增加 sidecar 目标名、外部二进制列表、`externalBin` 与 `bundle.externalBin` 配置对齐清单；输出 `checklist` | 若测试机与目标机差异大，先用最小三端二进制组合验证启动成功率 | `npm run release:packaging-readiness` 的 sidecar 条目从前置 blocker 变为通过（或给出明确未决外部依赖） |
| ACT-07.1 | SF3D 与 TripoSR 首轮证据采集 | Provider Specialist | 两条脚本运行记录、输出模型统计（mesh/material/bounds）、失败类目（token/shape/oom） | 若单次失败，补充显存与超时参数后重试一次 | `agent:p0-local-3d-runtime-sf3d-manual` 与 `agent:p0-local-3d-runtime-triposr-manual` 各有可复现成功或失败分类报告 |
| ACT-03.1 | ComfyUI 正式工作流验收 | Frontend Agent A/B | ComfyUI inpaint 工作流真实截图、工作流替换参数、失败日志 | 先在 mock 后置，再在真实服务重试；对 `400` 与 `422` 不做盲目重试 | `workflows/comfyui/patch_inpaint_api_template.json` 有可复现真实运行日志与输入输出绑定说明 |
| ACT-04.1 | 恢复与 cancel 统一策略稿 | Runtime | 补充 `Runtime` 设计段落：`provider_task_id` 重放、checkpoint 恢复边界、取消失败分类 | 不明晰边界不允许上场，先列出枚举并让 Verification 评审 | `docs/DESIGN.md` 更新 `15.4/15.5` 后续实现切片可直接引用 |
| ACT-05.1 | 非制造边界与 prompt 风险收敛 | Quality & Safety | Negative prompt 修订版、质量阈值变更日志 | 对争议词条只记录“例外条件/禁入域”并与 Product Owner 复核 | `release:prompt-quality` 可复现路径中的判据与规则文档同步 |
| ACT-06.1 | Unity gate 推进 | Verification | `npm run unity:import:gate` 从 `blocked_unity_not_configured` 到 `imported` 的条件清单、配置镜像、截图 | 若缺 Unity 环境，生成“环境阻塞清单+可修复项”并转入 `当前门禁快照` | 运行成功一次后，门禁 `unity import gate` 从“阻塞”更新到“待验证通过” |
| ACT-07.1 | 决策冻结收口 | Backend Architecture | `docs/DESIGN.md` 新增 §16（决策冻结）和 `README.md` 对齐说明，补齐相关门禁证据条目 | 若决策与代码实现冲突，先补齐配置项再复测 release-walkthrough | `docs/DESIGN.md`/`README.md` 与 `release:docs-walkthrough` 证据可复现 |

说明：

- 本节每条记录在完成后应补到 `docs/PACKAGING.md`、`docs/LOCAL_3D_RUNTIME.md`、`docs/UNITY_IMPORT_SMOKE.md` 或相关脚本注释中。
- 若任一动作长期阻塞，需回填到 `实现阶段阻塞说明` 并降低其优先级为“P1阻塞可恢复”。

## M6: 结构解释闭环冻结（第一阶段决策锁定）

目标：把第一阶段“任意物件先解释再重构”的主结构定为文档合同唯一入口，先于所有 provider/打包动作落地闭环。

- 统一 `README/DESIGN/API/SCHEMAS/DATABASE/FRONTEND` 的解释闭环叙事（`interpretation -> recast/confirm -> WeaponDesignSpec -> concept -> patch -> generate-3d -> export`）。
- 文档层声明 `interpretation` 默认 `2~3` 候选且需确认，`weapon_family` 仅兼容字段。
- 明确 `CreativeWeaponGraph@1`、`SkillGraph@1`、`structure_interpretation` 在后续文档中的数据边界。
- 在所有 quickstart/quality 文档补齐非武器示例：防弹裤/木棍/椅子/镜子/铃铛/树枝。
- 所有变更必须先通过 `release:docs-walkthrough` 的一致性扫描。

验收条件：

- `release:docs-walkthrough` 与 `release:safety-scope` 均通过结构化叙事扫描。
- Quickstart 与 Prompt 质量集出现 `combat_affordances` 与 `2~3` 候选验证项。
- `README.md`、`DESIGN.md`、`docs/API.md` 阶段目标不以 `weapon_family` 作为第一入口。

## M7: 生产级 Release Readiness（下一阶段）

目标：把第一阶段实现从“可展示”推进到“可发布”。

### M7.1 阶段目标

- 完成 sidecar 打包闭环并清零 packaging-readiness blocker（除外部证据）。
- 完成真实 provider 与 Unity 导入证据，确保 release 质量门可落地。
- 形成可复用的生产级任务恢复策略，避免未知 cursor、provider timeout 与取消失效的隐患。

### 决策验证约束（与 DESIGN §16 对齐）

本阶段任何实现动作都不能绕开《设计冻结》（`docs/DESIGN.md` §16）约束：

- 未同步更新 `README.md`、`docs/DESIGN.md` 且未通过 `release:docs-walkthrough` 的决策调整，不得进入开发实现。
- `M6` 的 provider、ComfyUI、3D 运行时、许可和平台优先级决策变更，必须先由后端/架构与 Verification 对齐并在证据目录中生成对应 `report.json`。
- 任一 M7 action 若触及默认 API provider、ComfyUI 模式、3D provider 选型、打包策略或许可边界，必须带有 `output/release/<ID>/` 的证据归档入口并写入复现参数。

### M7.2 里程碑输出

- `docs/PACKAGING.md`：补齐 sidecar 命名规范、三平台产物签名与安装验证流程。
- `docs/LOCAL_3D_RUNTIME.md`：补齐 real provider 的运行前置条件与失败分类。
- `docs/UNITY_IMPORT_SMOKE.md`：补充本地/CI Unity import 复现步骤与阈值定义。
- `docs/M4_PATCH_ASSETSTORE.md`：补齐真实 ComfyUI workflow 和 patch 审计要求。
- `scripts`：补齐/更新以下脚本（如适配）：
  - `agent:p0-local-3d-runtime-sf3d-manual`
  - `agent:p0-local-3d-runtime-triposr-manual`
  - `agent:p0-provider-runtime-boundary-smoke`
  - `agent:p0-generate3d-worker-loop-smoke`
  - `agent:p0-export-unity-worker-smoke`

### M7.3 负责人与验收

- Packaging/Distribution Agent：
  - 输出 sidecar 与三端资源清单。
  - 触发 `npm run release:packaging-readiness` 前置通过。
- Provider Specialist：
  - 完成 SF3D / TripoSR 对比报告和可用性结论。
  - 提供 provider 取消/超时失败分类对齐到 runtime state。
- Runtime Agent：
  - 明确任务恢复矩阵（等待/超时/已提交取消/取消失败）。
  - 更新 runtime/recovery 与 worker checkpoint 文档映射。
- Verification Agent：
  - 推动 `npm run unity:import:gate` 进入 `imported` 状态并归档证据。
  - 将 `design -> plan -> evidence` 的证据链写入下一期审计。
- Quality & Safety Agent：
  - 复核非制造边界文案和 negative prompt，不合格项只允许在 `safety-scope` evidence 中被追踪。

### 进入条件

- `npm run release:packaging-readiness`：前置通过。
- `npm run unity:import:gate`：从 `blocked_unity_not_configured` 转为可复现成功。
- provider 手动验收脚本可给出最小可复现失败与成功样例。
- 具备更新后的 `IMPLEMENTATION_PLAN.md` 与 `DESIGN.md` 追踪记录，允许继续进入下一实现切片。

## M0: Contracts Freeze

Deliverables:

- `docs/API.md`
- `docs/SCHEMAS.md`
- `docs/DATABASE.md`
- `docs/FRONTEND.md`
- `migrations/0001_init.sql`
- JSON schema files under `packages/weapon-spec/schemas/`

Gate:

- JSON schemas parse.
- SQL migration is syntactically checkable by SQLite.
- README and DESIGN link to the contract docs.

## M1: Monorepo Skeleton

Deliverables:

- `apps/desktop`: Tauri + React + Vite skeleton
- `apps/agent`: Python FastAPI skeleton
- `packages/weapon-spec`: schemas and generated types plan
- `workflows/comfyui`: placeholder workflow templates
- `workflows/blender`: placeholder normalization scripts

Gate:

- desktop dev server starts
- agent health endpoint returns OK
- no secrets committed

Current status:

- `apps/desktop` has the first Tauri + React workbench shell.
- `apps/agent` has FastAPI endpoints and SSE job events.
- `scripts/check_contracts.py` validates JSON schema parsing and SQLite migration shape.
- SQLite AssetStore is wired into `POST /api/weapons` in M2.
- Tauri has a local-development Agent supervisor in M3; bundled production sidecar startup is not yet wired.

## M2: Mock End-to-End Agent Loop

Deliverables:

- mock LLM returns valid `WeaponDesignSpec`
- mock ComfyUI writes fixture concept image metadata
- mock 3D provider writes fixture GLB metadata
- event stream powers task timeline
- immutable asset store writes metadata and files

Gate:

- `npm run agent:m2-smoke` passes against a temporary FastAPI process and temporary library.
- Idempotency replay returns the same `job_id`; conflicting body returns `409`.
- SSE events replay in stable sequence order and support resume.
- asset consistency check passes.
- frontend typecheck and production build pass.

Current status:

- `apps/agent/wushen_agent/asset_store.py` writes SQLite rows and immutable object files.
- `scripts/smoke_m2_assetstore.py` validates HTTP API, DB rows, SSE, idempotency, and file hashes.
- `apps/desktop` shows Agent connection state, provider state, structured errors, and asset library loading/error/empty states.

## M3: Real Concept Generation

Deliverables:

- OpenAI-compatible LLM adapter
- ComfyUI adapter
- provider settings UI
- prompt/workflow/seed provenance
- image quality report

Gate:

- one manual real-provider smoke succeeds
- retry from ComfyUI failure works
- API keys do not appear in DB, logs, files, or export package

Current foundation status:

- `apps/agent/wushen_agent/providers/llm.py` defines mock and OpenAI-compatible LLM providers.
- Real provider mode is explicit through `WUSHEN_LLM_PROVIDER=openai_compatible`.
- `scripts/generate_schema_types.py` generates JSON Schema TypeScript types and Python schema registry.
- `scripts/export_openapi.py` generates FastAPI OpenAPI JSON and desktop API component types.
- `apps/agent/wushen_agent/spec_validation.py` validates `WeaponDesignSpec` provider output against JSON Schema before AssetStore commit.
- `scripts/smoke_m3_llm_adapter.py` verifies mock LLM behavior, missing real-provider key failure, settings redaction, SQLite-backed create flow, and invalid spec rejection.
- `apps/agent/wushen_agent/providers/image.py` defines mock and HTTP ComfyUI providers.
- `workflows/comfyui/concept_api_template.json` provides the first API-format workflow template with injection bindings.
- `SQLiteAssetStore` now writes prompt, negative prompt, ComfyUI workflow, concept image, and schema-valid concept quality report provenance before rough 3D.
- `scripts/smoke_m3_comfyui_adapter.py` verifies the ComfyUI HTTP protocol boundary with a fake server and checks AssetStore traceability.
- `scripts/smoke_m3_comfyui_manual.py` provides a non-default manual smoke path for a real local ComfyUI server.
- `scripts/smoke_m3_image_dimensions.py` verifies PNG, JPEG, and WebP dimension parsing, and AssetStore checks require concept image width/height.
- ComfyUI HTTP adapter retries transient network errors, HTTP `408/409/425/429/5xx`, and preserves non-retryable `400` workflow errors as `PROVIDER_BAD_OUTPUT`.
- ComfyUI workflow metadata includes checkpoint, sampler, scheduler, steps, cfg, denoise, seed, image size, and template identity.
- `npm run m3:gate` passes and includes all M2 gates.
- `apps/desktop/src-tauri/src/main.rs` includes local-development Agent supervisor commands with Wushen health identity checks, managed-child cleanup, and base URL reporting.
- `apps/desktop/src/shared/tauri/agentSupervisor.ts` wraps Tauri invoke with browser fallback and gives the desktop API client its runtime base URL in Tauri mode.

Remaining M3 work:

- Add manual real-provider smoke with user-supplied key.
- Upgrade the local-development supervisor to a bundled Tauri sidecar.
- Add production art workflow templates beyond the minimal SD basic template.
- Add production art workflow template with selected checkpoint/sampler/model provenance.

## M4: Patch Mode

Deliverables:

- canvas background loading
- mask export
- patch manifest
- patch job
- version DAG and comparison UI

Gate:

- empty and mismatched masks fail locally
- patch creates a new version and never overwrites source
- old version remains viewable

Current foundation status:

- `SQLiteAssetStore.patch_weapon(...)` validates source version ownership, source concept/patch image ownership, `patch_mask` role, `patch_manifest` role, mask dimensions, non-empty PNG mask pixels, and `PatchManifest@1` schema.
- `POST /api/weapons/{weapon_id}/versions/{version_id}/assets` accepts idempotent JSON/base64 uploads for `patch_mask` and `patch_manifest`, writes immutable objects, parses PNG dimensions, and schema-validates `PatchManifest@1`.
- `POST /api/weapons/{weapon_id}/versions/{version_id}/activate` sets a committed version as `weapons.current_version_id` and returns fresh weapon detail; this powers accept/current-version and parent rollback controls.
- `GET /api/weapons/{weapon_id}` returns versions and asset metadata so the desktop can locate a patchable `concept_image` or `concept_patch` without manual asset id entry.
- `GET /api/assets/{asset_id}` and `/file` return asset metadata and controlled immutable object bytes by asset id with library-root containment and sha256 verification.
- `apps/desktop/src/features/canvas/PatchModePanel.tsx` implements the first background-image Patch Mode UI: choose weapon/version, load source concept image, draw brush or lasso mask at source pixel size, control brush size and mask opacity, undo/redo mask edits, upload mask, upload `PatchManifest@1`, call patch job, stream resulting job events, compare parent/source image against patch result with a slider when a patch version is selected, activate a selected version, return to the parent version, and switch back to the parent source for another patch attempt.
- Successful patch jobs create a new `generation_jobs` row, append a `weapon_versions(version_type='patch')` row, update `weapons.current_version_id`, and write `patch_prompt`, `concept_patch`, `comfyui_workflow`, and schema-valid `quality_report` assets.
- Patch events are append-only and ordered as `patch_interpreter -> image_inpaint -> image_quality_check -> finalize_job`.
- `scripts/smoke_m4_patch_assetstore.py` covers success, idempotency replay, idempotency conflict, empty mask rejection, mismatched mask rejection, source preservation, version activation, event order, asset roles, and asset library validation.
- `scripts/smoke_m4_patch_http.py` covers public create -> upload mask -> upload manifest -> patch flow, upload idempotency, version activation, parent rollback, and missing-version error handling.
- `scripts/smoke_m4_migrations.py` covers upgrading older M4 SQLite libraries where `schema_migrations` already contains `0001` but later M4 tables, role constraints, or content-reuse indexes are missing.
- `scripts/smoke_m4_comfyui_patch_adapter.py` covers the fake-server ComfyUI patch adapter boundary: source upload, mask upload, workflow binding, history polling, output download, and asset metadata.
- `npm run m4:gate` includes all M3 gates plus the M4 migration, patch, HTTP patch, and ComfyUI patch adapter smokes.
- Browser verification covered create -> Patch Mode -> lasso mask -> upload mask/manifest -> completed patch job; latest screenshot: `/tmp/wushen-patch-mode-lasso-submit.png`.
- Browser verification for comparison UI covers patch version selection, parent/result image comparison rendering, and slider movement.
- Browser verification for version controls covers activating the patch result and rolling back to the parent version.
- Browser verification also covered an upgraded older local library after applying `0002`, `0003`, and `0004`.

Remaining M4 work:

- Run `workflows/comfyui/patch_inpaint_api_template.json` against a real local ComfyUI instance and replace it with a production art workflow.
- Add task-level retry policies for real provider failures.

## M5: Rough 3D and Unity Export

Deliverables:

- `ThreeDProvider` adapter
- raw/normalized/optimized GLB records
- Three.js/R3F preview
- `unity_material.json`
- export folder with manifest and hashes

Gate:

- GLB preview loads and is nonblank
- quality report has blocker/warning/info checks
- Unity import smoke succeeds or records a blocking failure

Current foundation status:

- `create_weapon` now writes a minimal valid GLB 2.0 binary asset instead of a text placeholder with a `.glb` extension.
- `apps/agent/wushen_agent/providers/three_d.py` defines the first 3D provider boundary and `mock_3d` returns raw, normalized, and optimized GLB variants, Unity material metadata, and model metrics.
- `POST /api/weapons/{weapon_id}/generate-3d` now creates an append-only `rough_3d` child version from a selected `concept_image` or `concept_patch`, writes `ModelGenerationInput@1`, GLB variants, `unity_material_json`, model `quality_report`, and a `models_3d` row.
- `scripts/check_asset_library.py` validates `rough_raw_glb` magic, version, length, JSON chunk, and non-empty BIN chunk; invalid GLB files are blockers.
- `scripts/check_asset_library.py` also validates generate-3d job roles, `rough_3d` parent version, event order, GLB variants, model quality report target, non-empty mesh metrics, finite bounds, and material evidence.
- `apps/desktop/src/features/preview3d/Preview3DPanel.tsx` resolves weapons, current/latest `rough_raw_glb`, `unity_material_json`, and the current source `concept_patch` or `concept_image`, then loads the controlled asset URL with Three.js `GLTFLoader`.
- The desktop preview presents the rough model as a 360-degree exhibition scene: pedestal, simple original placeholder character, and weapon attached to a hand socket. It supports auto-rotation, pointer-drag rotation, toon, solid, wireframe, reset view, screenshot export, bounding-box camera placement, one-click generate-3d from the current image, one-click Unity ZIP export from the current model, controlled package download, and refreshes to the latest weapon after a new job is created.
- `scripts/smoke_m5_glb_preview_contract.py` verifies the rough GLB preview contract and asset library validation.
- `scripts/smoke_m5_generate3d_http.py` verifies public create -> generate-3d, idempotency replay/conflict, child rough_3d versioning, asset roles, list de-duplication, and asset library validation.
- `scripts/smoke_m5_export_unity_http.py` verifies public create -> export-unity, idempotency replay/conflict, export versioning, ZIP contents, relative Unity paths, safety boundary manifest, and asset library validation.
- `scripts/smoke_m5_unity_import.py` verifies generated export packages through a local Unity package preflight; when a Unity executable is configured, it creates a temporary Unity project, installs `com.unity.cloud.gltfast`, extracts the package into `Assets/`, and runs Unity batchmode import validation. `npm run unity:preflight` records `UNITY_EXECUTABLE_NOT_CONFIGURED` without failing ordinary mock development; `npm run unity:import:gate` passes `--require-unity` and fails the release gate when Unity is missing or import fails.
- Browser verification covered GLB asset selection -> pedestal/character/held-weapon WebGL canvas render. Latest preview screenshot: `output/playwright/m5-final-preview3d-panel.png`.
- Browser verification also covered the asset library handoff view. Latest library screenshot: `output/playwright/m5-library-handoff.png`.
- Canvas pixel verification returned `uniqueSampledColors=178` and `nonBackgroundSamples=504`; drag rotation changed checksum from `2489209341` to `3563662150`.
- `npm run m5:gate` includes all M4 gates plus the M5 GLB, generate-3d HTTP, Unity export package HTTP, P0 provider runtime boundary smoke, P0 local HTTP 3D provider smoke, P0 local 3D runtime wrapper smoke, P0 job history search smoke, desktop runtime/handoff browser smoke, desktop job action-state browser smoke, desktop job center history browser smoke, and desktop context-continuity browser smoke.
- `npm run m5:gate` also includes the Unity package preflight. The production Unity release gate is `npm run unity:import:gate`.
- P0 runtime recovery metadata foundation is now in the M5 development gate: `npm run agent:p0-runtime-recovery-smoke` verifies migration `0006`, provider task/checkpoint read state, explicit `INVALID_EVENT_CURSOR` SSE errors, cancel marking known provider tasks as `cancel_requested` or `cancelled`, and startup/manual recovery pausing interrupted active jobs as `waiting_user`.
- P0 async generate-3d worker foundation is now in the M5 development gate: `npm run agent:p0-async-generate3d-worker-smoke` runs with async mode enabled, verifies `POST /generate-3d` returns a queued job before outputs exist, idempotency replay/conflict still works, `POST /api/runtime/work-once` completes exactly one rough_3d child version/model, provider task/checkpoint metadata is persisted, completed replay does not duplicate outputs, and manual recovery -> retry -> worker completion remains asset-library clean. `npm run agent:p0-generate3d-worker-loop-smoke` separately enables `WUSHEN_GENERATE3D_WORKER=1` and proves a startup-managed local Worker completes a queued generate-3d job without calling `work-once`.
- P0 generate-3d provider runtime boundary is now in the M5 development gate: `npm run agent:p0-provider-runtime-boundary-smoke` uses `WUSHEN_MOCK_3D_POLL_SEQUENCE=polling,succeeded` to verify submit -> waiting_provider without rough assets, later poll -> fetch -> single committed rough model, and cancel while waiting -> provider task cancelled with no late model/assets.
- P0 local HTTP 3D provider adapter is now in the M5 development gate: `npm run agent:p0-local-http-3d-provider-smoke` starts a fake SF3D-style HTTP runtime and verifies the `POST /v1/rough-models`, `GET /v1/rough-models/{task}`, `GET /v1/rough-models/{task}/result`, and `POST /v1/rough-models/{task}/cancel` protocol, including GLB base64 result ingestion, no asset writes during polling, and provider cancel without late assets.
- P0 local 3D runtime wrapper is now in the M5 development gate: `npm run agent:p0-local-3d-runtime-wrapper-smoke` starts `scripts/wushen_local_3d_runtime.py` as a real subprocess in deterministic mock mode, connects the Agent through `LocalHTTPThreeDProvider`, verifies async worker submit -> waiting_provider -> fetch -> GLB commit, and verifies cancel reaches the runtime before late assets can be written.
- P0 real-model manual runtime verification is now documented and scripted outside the default gate: `npm run agent:p0-local-3d-runtime-sf3d-manual` requires `WUSHEN_SF3D_REPO` and starts the wrapper in `sf3d-cli` mode; `npm run agent:p0-local-3d-runtime-triposr-manual` requires `WUSHEN_TRIPOSR_REPO` and starts the wrapper in `triposr-cli` mode. Both submit a PNG source through `LocalHTTPThreeDProvider`, validate GLB output, and write raw/normalized/optimized GLB plus Unity material JSON to an output directory.
- P0 async export-unity worker is now in the M5 development gate: `npm run agent:p0-export-unity-worker-smoke` verifies queued export jobs do not create an export version, export_packages row, or ZIP asset before worker execution; idempotency replay/conflict still works; `POST /api/runtime/work-once` can claim an `export_unity` job; and `WUSHEN_EXPORT_UNITY_WORKER=1` can complete an export through the startup-managed local Worker without calling `work-once`.
- P0 frontend runtime/handoff visibility is now in the desktop surface and M5 gate: `JobTimeline` receives `JobRuntimeStateResponse` and renders provider task/checkpoint/recovery state; `Preview3DPanel` shows a conservative Unity handoff card; `LibraryPanel` shows per-version handoff checklist rows; `npm run desktop:p0-runtime-handoff-smoke` verifies recent-job restore, DOM state, asset file links, Library cross-version handoff coverage, and WebGL canvas interaction.
- P0 frontend context-continuity browser coverage is now in the M5 gate: `npm run desktop:p0-context-continuity-smoke` drives the real UI through Forge create -> Patch brush mask -> patch submit -> generate-3d from `concept_patch` -> export-unity -> Library sync, and verifies request bodies, version parent chain, active Inspector/top bar version, handoff asset links, and 3D canvas interaction.
- P0 frontend job action-state browser coverage is now in the M5 gate: `npm run desktop:p0-job-action-state-smoke` restores failed, waiting-provider, and recovered jobs, verifies recovery/cancel/retry button state, clicks retry/retry-from/cancel, and checks action responses plus runtime provider task updates.
- P0 Task Center history/audit coverage is now in the M5 gate: `GET /api/jobs` provides a lightweight historical read model with query/status/job_type/error_code/keyset pagination; `GET /api/jobs/{job_id}/actions` exposes durable action audit rows; migration `0007` adds read-side indexes; `JobCenterPanel` owns server-backed search, status and failure filters, manual job-id restore, recent-job wakeup, local terminal-job notification records, selected-job detail loading, runtime, failure reason, and action audit. `npm run agent:p0-job-history-search-smoke` verifies list ordering, pagination, search, failed/error filters, `JobDetail.error`, action audit, and recovery no-repeat semantics. `npm run desktop:p0-job-center-history-smoke` verifies the real Task Center UI, saved filters, recent-job wakeup, notification-record restore, action-to-event highlighting, retry, cancel, and captures `output/playwright/p0-job-center-history.png`.

Remaining M5 work:

- Promote the exhibition rig from frontend-only preview composition into a documented Unity import convention once real Unity export packages are wired.

- Run `agent:p0-local-3d-runtime-sf3d-manual` and `agent:p0-local-3d-runtime-triposr-manual` against real checkouts, capture output evidence, and choose the default open-source provider path based on license, install reliability, output quality, and Unity import behavior.
- Evaluate Hunyuan3D-2 after the wrapper pattern is proven, because it has heavier VRAM/texture/runtime requirements and separate model-weight/license concerns.
- Extend provider runtime recovery beyond the current mock boundary: resume polling from persisted provider task ids after restart, classify timeout/quota/provider-output failures, and record `provider_cancel_unsupported` for providers that cannot cancel.
- Calibrate model quality thresholds against real SF3D/TripoSR output, then add provider-specific texture checks, material-slot expectations, and Unity import warnings beyond the current parsed-GLB metrics.
- Configure Unity in local CI and require `unity_import_status=imported` before the Unity release gate can pass.

## Ongoing 8-Slot Review & Implementation Loop

The product design loop uses an 8-slot model. By default we keep focused ownership and only expand a few slots when evidence pressure appears. A complex release audit may add specialists, but hard cap is still eight active subagents.

| Agent | Ownership | Typical output |
| --- | --- | --- |
| Frontend Agent A | Workbench task flow, 3D preview ergonomics, asset handoff UX | UI change list with files and Playwright checks |
| Frontend Agent B | Visual hierarchy, desktop information architecture, Codex/Claude Code style Agent patterns | Interaction critique and design alternatives |
| Backend/Architecture Agent | API contracts, asset library, provider adapters, export/import pipeline | Architecture risks, schema/API changes, smoke coverage |
| Runtime Agent | Async workers, recovery, checkpoints, queue semantics | Worker behavior model, race-condition analysis, replay safety and idempotency checks |
| Packaging Agent | Tauri sidecar/build scripts, installer behavior, release assets | Packaging blockers, missing binaries/assets, lockfile/license readiness |
| Quality Agent | Non-manufacturing boundary, prompt safety, quality report gate logic | Risk list, threshold tuning recommendations, policy guardrails |
| Verification Agent | Release gates, safety boundary, build/test matrix, docs completeness | Gate audit and missing evidence list |
| Provider Specialist | ComfyUI / 3D providers / Unity import providers | Protocol compatibility report, failure mode notes, runtime readiness recommendations |

The main rollout keeps critical-path implementation local, uses these agents for bounded parallel review, and folds accepted findings into this implementation plan. When more than four are needed, add only sharply scoped specialists, such as Security, Unity Pipeline, or Provider Adapter, and keep the total at eight or fewer.

Current 8-agent review findings:

- P0 frontend context: first slice completed. `App` is now the source for selected weapon id, selected version id, active job id, and loaded weapon detail. Forge, Patch, Library, Inspector, and 3D Preview receive shared context props and report selection/job changes upward.
- P0 workbench surface: first slice completed. The empty non-Patch main-stage placeholder is now an asset-context workbench showing the active source image, version, rough GLB status, Unity export status, and safety boundary.
- P0 Agent execution: first frontend slice completed. The bottom `JobTimeline` is now a grouped Agent trace with task summary, stream state, progress, step cards, metadata, artifact ids, state-gated recovery actions, job hydration through `GET /api/jobs/{job_id}`, recent-job restart recovery, and SSE resume via `after`.
- P0 backend job actions: first slice completed. `JobEvent.seq` is public, `job_actions` audit rows are migrated in, retry/cancel/retry-from update persisted job state where allowed, append action events, reject invalid terminal actions, and are covered by `agent:p0-job-actions-smoke`.
- P0 backend runtime metadata: first slice completed. Migration `0006` adds `provider_tasks`, `job_checkpoints`, runner/lease/checkpoint/cancel-intent fields, provider task ids per step, and cancel state; `GET /api/jobs/{job_id}/runtime` and `POST /api/runtime/recover` expose conservative recovery state; cancel marks known active provider tasks as `cancel_requested` or `cancelled`; unknown SSE cursors produce `INVALID_EVENT_CURSOR`; this is covered by `agent:p0-runtime-recovery-smoke`.
- P0 async generate-3d worker: third opt-in slice completed. With `WUSHEN_GENERATE3D_WORKER=1`, generate-3d returns queued before outputs exist and the FastAPI startup loop claims queued/retrying/waiting-provider jobs automatically with lease cleanup, provider task/checkpoint metadata, provider submit/poll/fetch/cancel phases, rough_3d output commit, and asset-library validation. Default M5 remains synchronous for compatibility, and `work-once` remains a local/test stepping hook.
- P0 local HTTP 3D adapter and runtime wrapper: first real-runtime boundary completed. `WUSHEN_3D_PROVIDER=local_http` connects the Agent to a separate local 3D service via stable JSON/base64 GLB endpoints, keeps model dependencies out of the desktop Agent, validates GLB headers before asset commit, and is covered by `agent:p0-local-http-3d-provider-smoke`. `scripts/wushen_local_3d_runtime.py` now implements that service contract with deterministic `mock` mode, an `sf3d-cli` backend path that calls a local Stable Fast 3D `run.py`, and a `triposr-cli` backend path that calls local TripoSR `run.py --model-save-format glb`; the wrapper subprocess path is covered by `agent:p0-local-3d-runtime-wrapper-smoke`, and the real-checkout paths have opt-in manual verifiers via `agent:p0-local-3d-runtime-sf3d-manual` and `agent:p0-local-3d-runtime-triposr-manual`.
- P0 model quality metrics: first parsed-GLB slice completed. `AssetStore` parses the optimized GLB JSON chunk before writing the model quality report, records triangle, vertex, mesh, primitive, material, texture, image, PBR, bounds, center, extents, and longest-axis evidence, and mirrors the same metrics into `models_3d.quality_report_json`; asset-library validation and `agent:m5-glb-smoke` now treat missing mesh/bounds evidence as blockers and missing material evidence as warnings.
- P0 async export-unity worker: first opt-in slice completed. With `WUSHEN_EXPORT_UNITY_ASYNC=1`, export-unity returns queued before outputs exist and `work-once` commits exactly one export version/package. With `WUSHEN_EXPORT_UNITY_WORKER=1` or `WUSHEN_RUNTIME_WORKER=1`, the FastAPI startup loop claims/completes queued export jobs automatically. Default M5 export remains synchronous for compatibility.
- P0 frontend runtime trace and handoff visibility: twelfth slice completed. The active job runtime is fetched in `App` and shown in both timeline instances; 3D preview and Library now surface Unity handoff completeness and conservative mismatch/fallback warnings from existing asset roles. The 3D preview handoff card also exposes parsed model quality evidence from `current_model.quality_report.metrics`, including triangles, meshes, vertices, materials, textures, longest axis, center/extents, PBR, and bounds state. It also exposes `current_model.orientation_policy`, including forward axis, long axis, pivot, fallback pivot, and game-relative scale policy. The Library handoff checklist now aligns version report assets with the current model quality report when possible and shows QC status, blocker/warning counts, triangle/material counts, and bounds readiness. The Library detail surface also has a clickable version DAG strip that shows root/parent version relationships across concept, patch, rough_3d, and export versions. Each Library version card now exposes a provenance summary with job id, root/parent source, creation timestamp, and output asset roles. Version cards also expose handoff actions: concept/patch thumbnails or non-image summaries, JSON/GLB/ZIP preview drawers backed by controlled asset file reads, total file count/size, controlled batch download for that version, a Unity ZIP direct link when present, a local “open ZIP location” action through the Agent reveal API without returning absolute paths, and a “view generation trace” action that restores the version job into Task Center. ZIP preview parses the central directory, deflated manifest.json, package root, Unity payload counts, relative path safety, and manifest file coverage. The desktop shell now also supports hash deep links for `#/jobs/:jobId` and `#/weapons/:weaponId/versions/:versionId`, with browser coverage in `desktop:p1-deeplink-smoke`.
- P0 Task Center history and audit: third slice completed. The `/jobs` workspace now has server-backed history search, status/error filtering, manual job-id restore, recent-job wakeup, local terminal-job notification records, selected-job detail inspection, runtime, failure reason panel, and `job_actions` audit list. Row selection inspects history without changing active workbench context; restore actions explicitly subscribe the selected job as active. Query/status/error filters and recent job ids are persisted in local storage and restored after reload, terminal jobs write local notification-center records, optional system notifications use user-granted Notification permission, and action audit rows with event ids can locate/highlight the corresponding Agent Timeline step.
- P0 backend worker architecture: still open. Run and harden the `sf3d-cli` and `triposr-cli` backends against real model environments, add restart resume from persisted provider task ids, broaden checkpoint resume beyond rough3d, and expose structured `JobDetail.steps`.
- P0 frontend Agent trace findings from this round: preserve the bottom drawer as the operation ledger; map raw statuses into Chinese `排队中/执行中/等待 Provider/取消请求已提交/已取消/等待重试/已恢复执行/需要处理`; keep cancel/resume copy honest until backend worker proves provider stop/resume; group exports by version/model in the next asset handoff UI slice.
- P0 Unity release gate: keep `unity:preflight` in normal M5 development gate, but require `unity:import:gate` with a configured Unity executable before claiming production release readiness.
- P0 release safety-scope gate: first executable slice completed. `npm run release:safety-scope` validates WeaponDesignSpec schema locks, fallback/normalized negative prompt exclusions, mock asset pipeline export, Unity ZIP safety manifest, README boundary, exported spec safety boundary, model quality report non-manufacturing evidence, safe ZIP paths, and required README/DESIGN/API/Unity boundary phrases. `npm run release:gate` now aggregates `release:safety-scope` and `unity:import:gate`, so production release remains blocked on real Unity import until Unity is configured.
- P0 release secret/file-overreach gate: first executable slice completed. `npm run release:secrets-files` scans source/docs/scripts for committed secret-like literals, verifies Tauri production CSP is not disabled, verifies a capabilities file exists, dynamically creates and exports a mock asset, checks reveal responses do not expose local paths, confirms internal asset resolution still has a server-only path for `FileResponse`, and rejects a malicious absolute `asset_files.object_path` with `ASSET_PERMISSION_DENIED`. Tauri now has a restrictive first-pass CSP and `src-tauri/capabilities/default.json` with `core:default` only.
- P0 release prompt-quality gate: first executable slice completed. `npm run release:prompt-quality` runs the documented 20-prompt deterministic mock planner set, verifies schema-valid WeaponDesignSpec output, at least 18/20 non-classification structure-interpretation outputs (structure intent + affordance mapping), at least 16/20 style/material/silhouette readiness, 0/20 manufacturing unsafe outputs, at most 2/20 unguarded image artifact risks, and at least 15/20 single-image-to-3D readiness. The fallback/normalized negative prompt now also preserves watermark, unreadable text, broken subject, and missing subject exclusions.
- P0 release docs walkthrough gate: first executable slice completed. `npm run release:docs-walkthrough` checks that README links Quickstart, package scripts exist for all documented release commands, Quickstart covers install, local Agent startup, desktop startup, mock asset loop, provider configuration, Unity import and release gates, and API/ComfyUI/local-3D/Unity docs cover the same core endpoint and environment-variable contracts.
- P0 release packaging-readiness gate: first executable slice completed. `npm run release:packaging-readiness` checks Tauri bundle metadata, CSP/capabilities, production icons, `Cargo.lock`, `bundle.externalBin`, target-suffixed `binaries/wushen-agent-*` sidecar files, Rust `packaged-sidecar` mode, and packaging docs. It intentionally blocks the current local-dev-only supervisor until the Python Agent is frozen as a sidecar and Tauri packaging is configured.
- P0 release license/SBOM gate: second executable slice completed. `npm run release:license-sbom` scans `package-lock.json` license expressions, verifies `apps/agent/requirements-release.lock` pins the Python Agent runtime dependency closure with license metadata and matches the active `.venv`, requires a committed Tauri `Cargo.lock`, and checks `docs/THIRD_PARTY_LICENSES.md` for external runtime/model review status. `npm run release:gate` now runs safety scope, secret/file-overreach, prompt-quality, docs walkthrough, packaging-readiness, license/SBOM, and Unity import gates; production release is blocked until desktop sidecar packaging, the Rust dependency lock, and external model/runtime license reviews are cleared.
- P1 3D quality layer: raw/normalized/optimized GLB variants, material count, model quality status, blocker/warning asset evidence, Unity export availability, parsed GLB metrics, orientation policy, pivot state, and scale policy are now visible in the 3D panel. Remaining work is to ingest provider-specific import warnings, show measured post-normalization transform state, and calibrate thresholds against real model output.
- P1 asset handoff: Library now has grouped handoff sections, model quality badges for current-model report alignment, a clickable version DAG strip, first-slice version provenance summaries, thumbnails/non-image previews, JSON metadata, GLB header/chunk, and Unity ZIP manifest preview drawers, controlled version-level batch downloads, a restricted reveal/open-location API for Unity ZIP assets, job trace restore links from each version, and hash deep-link support for restored job/version state. Task Center now preserves filters, wakes recent tasks, keeps local terminal-job notification records, and links action audit rows back to timeline evidence. Remaining work is production release gates.
- P1 quality and safety gates: safety scope, secret/file-overreach, prompt-quality, docs walkthrough, packaging-readiness, and license/SBOM now have executable release gates. Remaining gates before any production-grade first-stage release are packaged sidecar implementation, production icon assets, Rust lock/license blocker cleanup, real provider evidence, and Unity import.

Latest P0 frontend verification:

- `output/playwright/p0-unified-context-workbench.png` shows Forge main stage and Inspector driven by the same active weapon/version context.
- `output/playwright/p0-library-selection-sync.png` shows Library selection state synchronized to top bar, main stage, Inspector summary, and 3D Preview weapon selector as `weapon_0491be7f34ff / ver_97e93060390d`.
- A legacy 100 B mock GLB correctly surfaced as a 3D preview load error while the shared context remained stable.
- Gates run after this slice: `npm run desktop:typecheck`, `npm run desktop:build`, `npm run desktop:p0-runtime-handoff-smoke`, `npm run m5:gate`, and `git diff --check`.
- `npm run m5:gate` passed through Unity package preflight. Production Unity import remains blocked until `WUSHEN_UNITY_EXECUTABLE` or `UNITY_EXECUTABLE` is configured for `npm run unity:import:gate`.
- `output/playwright/p0-jobtimeline-success-trace.png` shows the upgraded Agent trace drawer after a successful mock create job: task summary, 7 grouped steps, progress, artifact ids, metadata, and state-gated recovery actions.
- `output/playwright/p0-jobtimeline-task-center.png` shows the same recovered trace inside the Task Center view, so the task page no longer degrades to an empty flat log.
- `output/playwright/p0-job-center-history.png` shows the upgraded Task Center with history search, failure filtering, selected job detail, runtime trace, and action audit after retry/cancel interactions.
- `npm run agent:p0-job-actions-smoke` covers public `JobEvent.seq`, terminal-action rejection, persisted retry-from, persisted cancel, `job_actions`, and SSE `after` replay of newly appended action events.
- `output/playwright/p0-runtime-handoff-workbench.png`, `output/playwright/p0-runtime-handoff-runtime.png`, `output/playwright/p0-runtime-handoff-card.png`, and `output/playwright/p0-runtime-handoff-library.png` are generated by the scripted browser smoke for runtime/handoff visibility.

Remaining P0/frontend-release work:

- Production release gates now that job/version links, Task Center audit affordances, ZIP manifest inspection, recent-task wakeup, local notification records, safety-scope, secret/file-overreach, and license/SBOM gate entry points are covered.

Each milestone ends with:

- doc update
- contract update if needed
- test/gate update
- release notes draft
