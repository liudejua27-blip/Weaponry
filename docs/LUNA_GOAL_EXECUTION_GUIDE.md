# Luna Goal 执行指南：ForgeCAD 单用户 MVP

版本：2026-08-09
状态：Luna 强制执行协议；MVP host golden path 已收口；MCP010 质量轨道已批准
当前任务：`FGC-MCP010A done`；`FGC-MCP010B–FGC-MCP010F blocked`（等待后续独立 Goal）

## 1. Goal 目标

Luna 是仓库开发执行者，不是 ForgeCAD 运行时 Agent、Provider 或状态真值。当前 Goal 的代码主线和真实 Codex CLI 已完成一条用户授权图片 → typed 3D → PBR/fixed render → quality → approval/version → CAS GLB receipt 的 MVP host golden path；下一步只补视觉/回退/Viewer 证据和可选产品化，不继续堆复杂后台治理。

不要把 Goal 写成“完善整个软件”后无边界并行修改。一次只领取一个 `FGC-MCPxxx`，先完成退出 Gate，再进入下一项。MCP005–009 是已完成的 functional core；MCP010A–F 严格串行，MCP011–013 保留可靠性、分发和正式发布职责。

## 2. 每次启动完整阅读

1. `/Users/liuchongjiang/Documents/武神/AGENTS.md`
2. `docs/DOCUMENTATION_MAP.md`
3. `docs/DOCUMENTATION_STATUS.md`
4. `docs/CODEX_HANDOFF.md`
5. `docs/ADR/0025-codex-only-mcp-3d-runtime.md`
6. `docs/RESET_MIGRATION_PLAN.md`
7. `docs/CODEX_EXECUTION_PLAN.md`
8. `docs/CODEX_TASK_INDEX.md`
9. `docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`
10. `docs/AUTHORITATIVE_STATE.md`
11. `docs/MVP_DELIVERY_PLAN.md`
12. `docs/MVP_TOOL_CATALOG.md`
13. 本文件
14. 当前任务对应的 MCP/Schema/Compiler/Viewer/Skill/Test/Packaging 合同。

若冲突，按 `DOCUMENTATION_MAP.md` 解决；没有明确权威时先修文档，不自行混合两套架构。

## 3. Goal 建议文本

用户已批准以下 Goal；010A 已完成，Luna 仍须按 A–F 串行执行：

```text
按照 AGENTS.md、docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md、docs/CODEX_TASK_INDEX.md 和本指南，保护 dirty worktree，一次只执行一个原子任务。FGC-MCP010A 已完成权威重排、同 revision 用户级开发 App 激活和真实 Codex capability/build-hash Gate；证据包含第二次重启后的 30 个工具、Runtime Ready、临时 project_create/readback 和相同 build hash。MCP010B–F 保持 blocked，后续严格按 B 合同真值 → C 固定渲染/参考比较 → D 高细节 Operator → E 离线 AssetPack/UV/PBR → F Viewer/真实机器人/人工门执行。当前单图只允许 PARTIAL_VISIBLE_VIEW_PASS；补齐五张全身视图前 HQ_360_PASS=BLOCKED_REFERENCE_COVERAGE。禁止旧 Provider、付费 API、远程 image-to-3D、任意 Python/BlenderMCP、手工 GLB、heartbeat 或插件市场。
```

真实 host 证据按下面的顺序运行；`<AUTHORIZED_REFERENCE>` 必须是用户明确授权的本地 PNG/JPEG，命令输出不得写入 Git、日志或 receipt：

```bash
cd /Users/liuchongjiang/Documents/武神
export FORGECAD_MCP005_REFERENCE="<AUTHORIZED_REFERENCE>"
export FORGECAD_MCP007_REFERENCE="$FORGECAD_MCP005_REFERENCE"
export FORGECAD_MCP007_CODEX_E2E=1
export FORGECAD_MCP009_REFERENCE="$FORGECAD_MCP005_REFERENCE"
export FORGECAD_MCP009_CODEX_E2E=1
npm run release:mvp
script/test_mcp005.sh
script/test_mcp007.sh
script/test_mcp009.sh
```

MCP005/007/009 的真实 Codex receipts 已为 `PASS`；复核时必须严格观察
`project_create → reference_import → geometry_prepare → artifact_readback_get → appearance_prepare → artifact_readback_get → quality_get → candidate_confirm → version_list → export_prepare → export_confirm → version_list`。Codex 的缺参重试、额外调用、OAuth 无工具、超时或 Desktop attachment 不可验证时，保留 `BLOCKED/NOT_RUN`，并把 observed prefix、Runtime/host 错误和下一条安全动作写入对应 evidence；不要放宽 probe 来制造完整闭环 PASS。

## 4. 任务领取协议

修改文件前记录：

```text
Task ID:
Dependency status:
Base commit / branch:
git status -sb:
git diff --check:
Existing dirty files in owned paths:
Owned paths:
Forbidden paths:
Current capability:
Target capability:
Baseline commands and exit codes:
Exit gates:
External dependency decisions:
Destructive actions / user approval:
```

若任务索引没有 `ready`，不得自行跳任务。领取时只把该任务设 `in_progress`；任何其他任务保持 `blocked`。

### 4.1 MCP010 当前领取规则

- 当前 010A 已由用户批准并完成真实 Desktop Gate，标为 `done`；
- 成功 receipt 必须保留，第一次失败 receipt 也不得改写；
- 010A 已 done，未经后续独立 Goal 不得开始 B 的 Schema/代码；之后每次只将直接后继改为 ready/in_progress；
- 当前事实仍是 44 Schema、17 read + 13 write、十个 Skill `0.1.0`；目标能力只写 planned/unavailable。

## 5. FGC-MCP005 已完成记录

### 5.1 已完成 Gate

- `MVP_DELIVERY_PLAN.md` 的 MCP005；
- `MCP_RUNTIME_CONTRACT.md` 的参考导入；
- `SCHEMAS.md`、`DATABASE.md`、`TEST_STRATEGY.md`；
- CAS/Runtime/MCP 当前实现和 `docs/evidence/mcp004/manifest.json`；
- `EXTERNAL_PROJECT_ADOPTION.md` 中 `image-rs/image`、`img2threejs`、`img2css` decision。

MCP005 已完成：

1. `ReferenceEvidence@1`、import/get request/result Schema 与 Runtime records/migration；
2. PNG/JPEG decoder limits、MIME/魔数/截断/超限/hash mismatch、authorized root/outside-root/symlink negative tests；
3. Store/Runtime CAS admission；migration 在 OS 文件锁之后；永久状态不保存原路径；
4. MCP `reference_import/reference_get` 与 `supports_reference_import=true`；
5. `image-rs/image` 只启用 PNG/JPEG，依赖锁已更新；
6. 真实 Codex CLI 隔离 Runtime 导入用户授权 PNG，源 SHA-256 与 CAS object hash 一致；证据只留 hash/尺寸/MIME/授权；
7. Desktop 当前 bridge 诚实记录 `NOT_RUN / unavailable`；
8. 证据、状态账本、handoff 和下一任务已更新。

### 5.2 禁止扩展

图片进入 CAS 不是建模完成。当前 MCP007–009 已打开受限 geometry/GLB、UV/tangent/PBR、固定渲染、limited quality、stable-Part change、immutable version/restore 和 CAS export；仍禁止 Blender、资产下载和远程 Provider。

## 6. MCP006–009 执行摘要

### MCP006（已完成）

先 Schema/validator，再 first-party Skills。MCP006 已完成 44 个 contracts schema、十项 registry manifest、十个独立标准 Bundle、trust hash、`skill_list/get` 和只读 resource、DAG/单位/finite/预算 validator、负向 fixture、benchmark receipt、LICENSE/NOTICE/SBOM/provenance 绑定；`uv-pbr` 已声明为 product-owned bounded geometry consumer。Codex 提交 typed program，ForgeCAD 不调用 LLM。MVP Bundle 用 canonical hash + first-party trust root；不省略许可证/SBOM/provenance，但分发签名/撤销延后。它只证明声明式能力可审计，不证明通用视觉质量。

### MCP007（已完成）

MCP007 已通过 `npm run mcp007:test`：Geometry Worker library/binary 接受 canonical `GeometryProgram@1`，生成确定性 glTF 2.0 GLB；Runtime 生成 geometry candidate/quality report，MCP 通过 authenticated IPC 暴露 `geometry_prepare` 和 `artifact_readback_get`，Viewer read model 读取候选与 artifact lineage。当前 fixture 覆盖 14 个语义机器人部件和 finite/index/budget/unknown-operator/no-version-on-failure；实体 GLB/PBR/render 已由 MCP008 接上。真实 Codex CLI geometry receipt 已 PASS（用户授权 PNG → 14-part/516-triangle typed geometry）；外观/质量/导出已由 MCP009 十二调用 receipt 覆盖，但不等于像素相似度或真人高质量结论。

### MCP008（已完成功能核心）

`npm run mcp008:test` 已通过：hash-bound 三种 MaterialZone、UV/tangent/glTF PBR lowering、beauty/silhouette/normal/part-ID 固定 PNG、Runtime readback 和 Three.js GLB canvas。Viewer 只读，不复制状态；renderer 不依赖 Viewer。真实 Codex appearance/readback 已在 MCP009 receipt 中 PASS；限制仍为无纹理烘焙/UDIM/全 AOV、像素相似度和视觉评分。证据：`docs/evidence/mcp008/`、`docs/evidence/mcp009/`。

### MCP009（MVP host golden path 已完成）

`npm run mcp009:test` 已通过 24 个 Runtime tests + 16 个 MCP tests；真实 Codex CLI 已完成十二调用 reference→geometry→appearance→readback→quality→candidate_confirm→version_list→CAS-only export。`quality_get` 仍只返回明确 `limited` aspect-ratio；`change_prepare` 要求当前 base version、稳定 Part ID、allowlisted operation 和新 typed programs；confirm/reject/restore 保持 immutable/idempotent；`mvp-glb` export 只消费 confirmed quality-passing CAS GLB，返回 output hash 和 receipt，不写任意本机路径。限制：像素级 silhouette/landmark/region compare、真人评分和 Desktop write 未运行。证据：`docs/evidence/mcp009/`。

## 7. GitHub / Skill / Plugin 纪律

用户允许 GitHub 研究和配置，不等于允许任意安装。必须遵循：

```text
search/read source + release + license
→ classify library/tool/asset/reference-only
→ pin exact revision
→ adoption receipt + LICENSE hash + transitive SBOM
→ isolated malicious/resource/determinism benchmark
→ accepted decision
→ only then edit lockfile/build/package
```

允许优先评估：image-rs/image、gltf-rs/gltf、Manifold、xatlas、mikktspace、glTF-Validator、glTF-Transform。`approved-for-evaluation` 不等于 `adopted`。

禁止安装：BlenderMCP、FreeCAD MCP、任意 Python CAD MCP、远程 image-to-3D Provider、自动下载模型权重、GitHub prompt/Skill pack。MCP010E 仅允许 Codex 将计划点名的 CC0 文件一次性下载到本机 adoption cache；逐资产 hash/license/SBOM/provenance 通过后才能编入 first-party 离线 AssetPack。Runtime、安装器和 Viewer 不联网、不调用素材 API。

## 8. 实施纪律

- 保留 dirty worktree，不 reset/clean/checkout 用户修改；
- 文件修改用 patch；不 commit/push/merge，除非用户明确要求；
- 新公开合同先 Schema、生成类型、validator、negative tests；
- Runtime 唯一写库；MCP/Viewer/Worker 不开 SQLite；
- 永久写必须绑定 project/base/candidate/artifact/quality/approval/idempotency；
- 不记录 secret、prompt、原图副本、用户名、绝对路径；
- 不开网络服务、8000、Provider、任意 shell/Python/JavaScript；
- 失败路径不创建版本，不以 fallback 假成功；
- 任何质量数字同时记录阈值、实测值、fixture 和 limitation。

## 9. 验证分类

```text
PASS      当前工作树实际运行成功
FAIL      已运行且失败
BLOCKED   权限、宿主、硬件或外部状态阻断
NOT_RUN   本轮没有运行
```

focused ≠ aggregate ≠ real Codex ≠ packaged ≠ visual ≠ human。必须分别写。历史 receipt 不覆盖当前二进制，文档总结不覆盖 GLB/render/raw report。

共同 Gate：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
npm run contracts:check
git diff --check
```

任务专属命令在对应 evidence manifest 中固定；依赖变更后必须证明 offline build。

## 10. 每轮 handoff

必须更新：

- `CODEX_HANDOFF.md`：实际做了什么、命令/exit、真实运行、blocked/not-run、下一动作；
- `CODEX_TASK_INDEX.md`：只有满足退出条件才变状态；
- `DOCUMENTATION_STATUS.md` 和 capability matrix：当前能力；
- 对应合同、用户指南和 evidence manifest；
- 外部依赖 receipt、THIRD_PARTY_LICENSES/SBOM（若采用）。

## 11. Goal 状态句

完成单个任务：

```text
FGC-MCPxxx completed: all listed exit gates passed on <commit/worktree>; next ready task is FGC-MCPyyy.
```

未完成：

```text
FGC-MCPxxx not complete: <PASS/FAIL/BLOCKED/NOT_RUN evidence>; next safe action is <one action>.
```

MVP 关闭：

```text
ForgeCAD MVP completed for the first hard-surface reference benchmark on <commit/worktree>; universal high-quality image-to-3D and production distribution remain out of scope.
```

## 12. 当前可执行的 MVP 工具顺序

真实 Codex host 具备 MCP write opt-in 后，按下面顺序调用；每一步把返回的 ID/hash传给下一步，禁止从模型自由猜测 hash：

1. `project_create`：创建本地项目，记录 `project_id`。
2. `reference_import`：仅提交用户授权的 PNG/JPEG 字节或受授权 root 下的 Codex 文件，保存 `reference_id/object_sha256`。
3. `skill_list` / `skill_get`：读取十个 first-party development Skill 的 manifest；Skill metadata 不是执行代码。
4. Codex 根据参考图输出 `GeometryProgram@1`；调用 `geometry_prepare`，检查 `ArtifactReadback@1` 的 `part_ids/triangle_count/validator_status`。
5. 输出 hash-bound `AppearanceProgram@1`；调用 `appearance_prepare`，检查 UV/tangent、MaterialZone、`RenderSet@1` 四个固定 pass。
6. 调用 `quality_get`；如果提供 `reference_id`，把 compare 的 `status=limited` 作为比例提示，不把它写成像素相似度。
7. 用户拒绝候选时调用 `candidate_reject`；确认时只把 Runtime 返回的 candidate/artifact/quality hash 放入 `candidate_confirm`，并生成新的 immutable version。
8. 局部修改时读取当前 `version_id`，只提交一个 `change_set.part_id` 和 allowlisted operation，再调用 `change_prepare`；它仍需完整新 Geometry/Appearance programs，失败不得移动 head。
9. 修改后再次 `quality_get`，用户批准后 `candidate_confirm`；需要回退时 `restore_prepare` → `restore_confirm`，永远创建新子版本。
10. 导出时 `export_prepare(format=glb, profile=mvp-glb)` → 用户确认 → `export_confirm`，保存 `output_sha256` 和 receipt；当前输出留在 CAS，不能把任意路径传入 Runtime。

真实验收必须另外记录：Codex host 类型、MCP initialize 版本、参考源字节 hash、Geometry/Appearance canonical hash、GLB hash、RenderSet hash、QualityReport、approval receipt、version DAG、Viewer readback、重启后的 hash 和真人评分。任何一项没有运行，都写 `NOT_RUN`；宿主不可用写 `BLOCKED`。
