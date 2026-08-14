# ForgeCAD MCP Runtime 合同

版本：2026-08-10
状态：MCP005–MCP009 MVP functional core 和真实 Codex CLI host golden path 已完成；MCP010B structural source Gate PASS 但 Darwin OS memory hard cap deferred/NOT_RUN；MCP010C 当前源码为 36 read + 23 opt-in write = 59 个工具，fixed renderer/九 AOV/reference comparison/review raw Gate PASS；MCP010D/E offline Operator/AssetPack/UV/PBR/MikkTSpace raw Gate PASS；MCP010F Viewer source 与轮廓目标/相机拟合/边界误差/多 Part 误差表 source Gate PASS，并新增 Runtime-owned `primary_form_repair_prepare` 单动作 staged prepare/evaluate；Agentic observe/plan projection 与 durable session/checkpoint/RepairIntent prepare/readback 也已通过隔离 source/transport/restart receipt；通用单动作 orchestrator、Repair 应用、packaged/live C/D/E/F、真实用户 likeness、Viewer/PBR likeness/360仍 NOT_RUN/BLOCKED
P0 required 客户端：Codex Desktop、Codex CLI
未来兼容客户端：Codex IDE / VS Code / Cursor / Windsurf；其他 MCP Client

## 1. 合同目标

`forgecad-mcp` 将模型无关的 ForgeCAD Runtime 暴露给 Codex。它不包含 Agent、聊天、图片理解、模型 SDK、Provider、项目数据库或几何算法。所有工具输入先经公开 JSON Schema 验证，再调用本机 Runtime；所有输出都包含稳定 ID、Schema 版本、hash、lineage、能力状态和可恢复错误。

P0 使用 MCP `stdio`。Streamable HTTP、远程多租户、OAuth 和通用 MCP Client 均不在范围内。MCP Tasks/Skills 等可选协议扩展不能成为 P0 前置条件；长任务使用普通工具返回持久 `RuntimeJob`。

当前工具账本校正：源码默认 36 个只读工具，显式 write opt-in 后共 59 个（36 read + 23 write）；下文旧数量仅指历史中间 receipt，不覆盖当前 manifest。

Stage 0 运行合同快照：当前共有 102 Schema，唯一 `in_progress` 是 `FGC-MCP010F`，统一机器入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。Agentic observe/plan/critic/evidence 只读 projection 的隔离证据为 `docs/evidence/mcp010f/agentic-runtime-observe-plan-20260813.json`；durable session/checkpoint/RepairIntent prepare/readback 的隔离重启证据为 `docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`，它只证明受限持久化和 CAS-only restore intent，不证明 orchestrator 或 Repair 应用。attempt35 仅是 provisional retained observation，它的结果为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`：camera-fit hash `354caf27…f95788` 与 reference-compare camera hash `8cd20605…a535` 为 `MISMATCH`；packaged Viewer binding 为 `NOT_RUN_DIFFERENT_COHORT_AND_ARTIFACT`。因此 source/raw/transport/build/AX smoke 只能证明对应合同或链路，不能声明视觉、人评或 packaged E2E PASS。

<!-- forgecad-stage0: schemas=102 schema_set_sha256=608c97613e2f643674060bdf412b9a0ae3e4bb79492445bdf5433d34883a0caf read_tools=36 write_tools=23 total_tools=59 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json latest_completed=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json -->

## 2. 进程和信任边界

```text
Codex ──stdio── forgecad-mcp ──local authenticated IPC── forgecad-runtime
                       │                                  ├── SQLite V1
                       ├── short launcher election       ├── CAS
                       └── shared Runtime handoff         └── later workers
```

- `forgecad-mcp` 是 Codex 的唯一 MVP 入口；它拥有 stdio，会异步启动或连接同一数据根的共享 Runtime，不等待 Runtime ready。外部 `FORGECAD_RUNTIME_SOCKET`/`FORGECAD_RUNTIME_TOKEN` 仍可用于独立诊断，但普通配置不需要携带它们；
- MCP 进程无数据库写权限，无任意项目文件系统权限，不监听 TCP 端口；
- 多个 MCP 适配器没有可认证 Ready handoff 时，仅通过短时 `ipc/launcher.lock` 选出启动者；选主者复核/清理 stale handoff 并发起 Runtime spawn，spawn 成功后立即释放该锁。launcher lock 不授予 SQLite/CAS 写权限，也不是 Runtime 存活租约；
- Runtime 在打开数据库和 migration 之前取得 OS 独占 `runtime.writer.lock`，它才是最终唯一写者。MVP 不使用数据库 TTL lease、heartbeat、fencing、daemon、broker 或 stale takeover；第二个 Runtime 返回 `RUNTIME_BUSY`；
- 已经 Ready 的共享 Runtime 不属于某一个 MCP stdio 会话；正常适配器退出不终止它，只有显式 authenticated shutdown/update 流程主动停止。Runtime 跨会话存活不等于未完成 Job 已支持 checkpoint；
- Runtime 校验 project scope、base revision、idempotency key、candidate hash、approval receipt 和 tool capability；
- Worker 只接受受限内部协议，不接受 Codex 生成的 Python、JavaScript、shell、URL 或绝对文件路径；
- 工具失败时返回 typed error，不回退 legacy HTTP、Provider 或第二状态写者。

MCP010A 第一次真实 Desktop 重启暴露了 stale handoff、多适配器监督和单客户端 IPC 阻塞问题；失败 receipt 保持原样。共享 Runtime/IPC 修复的 focused/aggregate tests、同 cohort Dev.app 重建、package verify、隔离 probe 与第二次真实 Desktop 重启后的 30 工具/Ready/cohort/project readback Gate 均已 PASS；成功 receipt 为 `docs/evidence/mcp010a/codex-desktop-post-restart-success.json`。本节仍不宣称 MCP010B–F 的视觉质量能力。

## 3. Server 信息与能力协商

Server 名：`forgecad`。Server instructions 的前 512 字符必须自包含地说明：这是本地 3D Runtime；任何设计 tool 或其他 Skill 前先读取 `ponytail-preflight@0.1.0`；永久写入需候选与用户批准；长任务返回 job；禁止发送任意代码和未授权路径。

MCP003 使用 2025-era 的有状态 stdio 生命周期：`2025-11-25` 是 ForgeCAD 的规范版本，同时明确兼容 Codex 当前 stdio 默认发送的 `2025-06-18`。初始化必须包含 `protocolVersion`、`capabilities`、`clientInfo`；只接受这两个版本，并在响应中返回实际协商的版本。不匹配的版本、缺失参数或 Runtime 合同会 fail closed。初始化后，诊断可先调用 `capabilities_get`；进入设计链路时必须先 `skill_get(ponytail-preflight@0.1.0)`，随后才可调用其余 tool/Skill。返回：

- `runtime_version`、`contract_versions`、`tool_manifest_hash`；
- Viewer/Runtime/Worker/Skill 状态；
- 支持的几何、材质、纹理、渲染、质量、导入/导出格式；
- 图片导入模式及尺寸/MIME 上限；
- 每个工具 `available | unavailable | degraded` 与 limitation；
- 当前项目授权范围和写审批策略。

若 Server/Runtime/合同版本不兼容，所有写工具 fail closed；只允许诊断和导出备份。

## 4. 资源

| URI | 内容 | 约束 |
|---|---|---|
| `forgecad://capabilities` | 当前能力快照 | 无目标能力伪装 |
| `forgecad://projects/{project_id}/snapshot` | `ActiveDesignSnapshot` | 单一当前投影 |
| `forgecad://projects/{project_id}/selection` | Viewer 当前临时选择 | 非版本真值 |
| `forgecad://candidates/{candidate_id}` | 候选、readback、quality 摘要 | hash-bound |
| `forgecad://jobs/{job_id}` | Job 与最近事件 | 可重启读取 |
| `forgecad://versions/{version_id}` | 不可变版本和工件 manifest | 只读 |
| `forgecad://renders/{render_set_id}/{pass}` | 固定视图/AOV 图像 | MCP008 生成四个 bounded PNG pass；binary 仍走受保护的 Viewer IPC |
| `forgecad://skills/{skill_id}/{version}` | first-party Skill manifest + checked-in knowledge | MCP006 development-only registry；只读 metadata/knowledge，不含可执行 payload |
| `forgecad://artifacts/{artifact_id}` | hash-bound 工件元数据 | MCP007 通过 `artifact_readback_get` 读取；binary blob 仍不内联 |
| `forgecad://operators/catalog` | Runtime-owned `OperatorCatalog@1` | MCP010B V2 authoring catalog；必须与 `operator_catalog_get`、capabilities 和 V2 artifact/readback digest 相同 |

MCP003 当前已实现 capabilities、项目 snapshot/selection、candidate、job、version 的 JSON projection 和对应 resource templates；MCP005 增加 references，MCP006 增加 first-party Skill manifest resources，MCP007 增加 artifact metadata/readback，MCP008 增加 RenderSet metadata，MCP010B 新增 operator catalog resource 的可调用镜像。MCP raw tool 不内联原始 GLB/PNG bytes；可选 Viewer 通过 authenticated `artifact_bytes_get` 读 CAS bytes。资源 URI 只接受 `forgecad://` 和受限 opaque ID，不接受文件路径、URL、查询串或 `..`。

大二进制不内联到日志或事件；通过 MCP resource link 或受限 blob 读取，并声明 MIME、字节数和 SHA-256。

## 5. 工具目录

### 5.1 只读工具

- `capabilities_get`
- `runtime_status`
- `doctor`
- `operator_catalog_get`（与 `forgecad://operators/catalog` 同一 Runtime-owned `OperatorCatalog@1`）
- `geometry_program_hash`（只校验 hash-free `GeometryProgram@2` draft 并返回 compiler-owned hash；零持久化副作用）
- `project_list`
- `project_get`
- `snapshot_get`
- `selection_get`
- `candidate_get`
- `job_get`
- `job_events_read`
- `version_list`
- `version_diff`
- `skill_list`
- `skill_get`
- `quality_get`（结构/PBR/fixed-render checks；reference compare 明确 limited）
- `artifact_readback_get`
- `reference_get`

MCP003 的工具清单固定排序并声明 `readOnlyHint=true`、`destructiveHint=false`、`idempotentHint=true`、`openWorldHint=false`。当前 MCP010B–F 源码默认有 36 个只读工具，显式 write opt-in 后为 23 个写工具；V2 authoring、AssetPack 查询、轮廓目标读取、相机拟合证据、边界误差、多 Part 误差表、render image 和 durable session/checkpoint readback 工具只校验或读取，绝不把 MCP 适配器变成状态写者。`artifact_readback_get` 已读取 GLB header/lineage/part/triangle/UV/tangent metadata，`material_pack_get` 返回离线 AssetPack manifest，`quality_get`、`silhouette_target_get`、`camera_fit_prepare`、`boundary_error_get`、`session_get` 和 `checkpoint_get` 已可用但质量比较保守标记 limited；不得以自然语言把 limited、fixture 或 unavailable 伪装成视觉 PASS。`CameraCalibrationRef@1` 是 `SilhouetteFitIntent@1` 的闭合只读引用，只携带 Runtime-owned camera hash pair，Runtime 按 candidate/target evidence 解析完整 calibration，拒绝 hash 漂移或跨 candidate 引用。MCP010F Viewer 的 AOV、对比、部件/材质区筛选、爆炸图、热图和轮廓画布仅为只读 ephemeral projection；Viewer durable lookup 同样不提供写入入口。MCP010A Dev.app 的 30-tool activation receipt、MCP003 历史的 17-read snapshot，以及 MCP010B 的 3c/f488 package receipts 都必须保留为历史。当前总源合同为 102（历史合同 + MCP010B/C/D/E/F 与 Agentic contract family），B/C/D/E/F source-focused Gate 均已按各自范围通过；历史 `bfa56ac…de9` package receipt和 `d9c23b…ac0bd` Dev.app package/Worker/raw/real-Codex structural Gate及 live Desktop structural activation均已通过。

这些工具必须声明 read-only annotation，且不能以“读取”为名创建项目、下载网络资产、运行编译或改变 GC 生命周期。`reference_get` 只返回 ReferenceEvidence 元数据，不返回原始路径或字节；当前不提供原始图片 MCP blob 读取。

### 5.2 候选/任务工具

- `project_create`（MCP004；创建项目元数据）
- `candidate_prepare`（MCP004；diagnostic 或已入 CAS 的 typed object）
- `reference_import`（MCP005；只写 CAS/ReferenceEvidence，不创建版本）
- `geometry_prepare`（MCP007 V1 compatibility；MCP010B 也接受由 `geometry_program_hash` 生成 canonical hash 的 V2 program，且 program `project_id` 必须等于 outer target project）
- `appearance_prepare`（MCP008；bounded AppearanceProgram@1）
- `quality_get`（只读；Runtime-owned hard checks + limited reference evidence）
- `change_prepare`
- `restore_prepare`
- `export_prepare`
- `job_cancel`

它们可写临时 Job/CAS/candidate 状态，但不能创建永久资产版本。由于会读取附件、占用计算或创建临时工件，Codex 配置中按 write 工具处理。`quality_get` 是只读工具，不在此组重复列出；MVP 尚未提供 `visual_review_submit`、`exploded_view_prepare` 或通用 `candidate_render` 工具。

### 5.3 MCP004 已完成基座边界

MCP004 当前已在 Runtime 和 authenticated local IPC 实现并测试以下 typed 方法：`project_create`、`candidate_prepare`、`candidate_confirm`、`candidate_reject`、`restore_prepare`、`restore_confirm`、`export_prepare`、`export_confirm`、`job_cancel`。MCP007 增加 `geometry_prepare`，MCP008 增加 `appearance_prepare`，MCP009 增加 `change_prepare`。`candidate_prepare` 接受已经存在于 Runtime CAS 的 prepared object hash，或受限的 `request.typed=diagnostic` 非视觉合同对象；两条路径都不接受图片路径、任意代码或网络 URL。`quality_get` 现在执行 Runtime-owned geometry/GLB/UV/tangent/PBR/fixed-render hard checks，并可返回明确 `limited` 的 reference aspect comparison。`restore_prepare` 只接受 project 内已 confirmed 且 quality-passing 的历史 version，并以当前 head 绑定新 candidate；`restore_confirm` 在单一 SQLite 事务中创建当前 head 的新子版本，历史版本不被覆盖。`export_prepare/export_confirm` 支持 `manifest-json/diagnostic` 和 `glb/mvp-glb`；GLB 只允许 confirmed quality-passing Runtime GLB，confirm 返回 CAS output hash/receipt，不写任意本机路径。

当前 `forgecad-mcp` 源码的默认 stdio tool manifest 包含 36 个只读工具；显式 authenticated IPC + `FORGECAD_MCP_ENABLE_MCP004_WRITES=1` 时列出 59 个工具，即 36 个只读 + 23 个写工具（MCP004/005/007/008/009/010C/F 与 Agentic projection/durable prepare）。Agentic 的四个 projection 工具、`session_get` 和 `checkpoint_get` 是只读 Runtime-owned surface；`session_create_or_resume`、`checkpoint_prepare`、`checkpoint_restore_prepare` 是显式 approval-gated prepare，不直接 confirm candidate/version。`operator_catalog_get` 与 resource 完全镜像；`geometry_program_hash` 拒绝预填 hash、unknown/V1/catalog mismatch 和无效 draft，且没有 Store/CAS/Job/event 写入。`silhouette_target_get`、`camera_fit_prepare`、`silhouette_fit_prepare`、`part_contour_fit_prepare`、`silhouette_part_error_get`、`silhouette_candidate_compare`、`boundary_error_get` 只读 target/RenderSet 或运行有界相机、Rig、Part 和候选搜索；`reference_mask_prepare` 与 `reference_mask_refine_prepare` 才写入不可变 CAS target。`material_pack_get` 只读取并校验 first-party manifest；`render_pass_get` 只 CAS 读取 RenderSet@2 并返回 PNG image block；`reference_compare_prepare` 生成 candidate/reference-bound camera、九 AOV、mask、metrics、diff，不创建版本；两个 review 工具只保存 typed evidence。V2 physical contract 为 position ±10 m、dimension/height ≤10 m、radius/radii ≤5 m。`runtime_status` 和 `doctor` 只读取生命周期状态，不运行 fixture、confirm、签名或完整验收。视觉证据工具声明 `requiresConfirmation`/write boundary，Runtime 不把 receipt当密码学身份认证。Runtime contract/version 由同版本 launcher 和 Runtime 事务合同保证，不把 client name 或一段 status 字符串当成安全边界。MCP/Runtime 不可用时 initialize 仍成功，依赖 Runtime 的调用返回结构化 `RUNTIME_UNAVAILABLE`、`retryable=true`。

MCP004 可按当前任务范围标为 done；其历史 evidence 中 reference/Geometry/GLB/signing 的 NOT_RUN/BLOCKED 保持不变，并分别转到 MCP005、MCP007–009 和 MCP013。

MCP010F 的 `silhouette_rig_hash` 是默认只读的 Runtime-owned authoring helper：它只接收候选绑定、无 `canonical_sha256` 的 `SilhouetteRig@1` draft，返回唯一 Rig hash，不创建 candidate、Job、版本或 CAS 对象。Codex/Luna 应先调用它，再把返回 hash 放回不变的 Rig draft，避免在自然语言、脚本或客户端中复制 canonical JSON 算法。

### 5.4 MVP 工具开放顺序

| Task | 新工具/能力 | 永久版本写入 |
|---|---|---|
| MCP005 | `reference_import`、`reference_get` | 否；只创建 ReferenceEvidence/CAS |
| MCP006 | `skill_list/get`、Skill resource 可用实现；development-only Bundle metadata | 否；创建 typed plan/candidate |
| MCP007 | `geometry_prepare`、`artifact_readback_get`、Viewer candidate/artifact read model | 否 |
| MCP008 | `appearance_prepare`、四 pass fixed render、Viewer artifact bytes | 否 |
| MCP009 | `quality_get`、`version_diff`、`change_prepare`、`glb/mvp-glb` export | confirm/export 依赖现有 approval 事务；reference compare limited |
| MCP010B | `operator_catalog_get`、`geometry_program_hash`、V2 `geometry_prepare` / `ArtifactReadback@2` | hash/catalog 读取零永久版本；prepare 仍只创建候选，需严格 readback 才可继续 |
| MCP010F | Viewer read model、AOV/compare/Part/MaterialZone/explosion/heatmap controls | 只读 Runtime projection；Viewer 不启动 Runtime、不写 SQLite/CAS/候选/版本 |

只有 producer、Runtime validator、negative tests、capability 状态和真实 evidence 同任务完成后，工具才可从 unavailable 变 available。不能先把空工具列出再用自然语言结果伪装实现。

隔离 source-built real Codex CLI 已完成 `project_create → reference_import → capabilities_get → operator_catalog_get → geometry_program_hash → geometry_prepare → artifact_readback_get` 的 V2 structural Gate；attempt 1 保持 `BLOCKED`，attempt 2 为历史 pre-semantic-Part-sink 的 `PASS`，且 candidate 未确认。固定同级 Worker 的 timeout/crash/FD isolation 和 accepted-result peak-RSS gate 另已通过，但 Darwin 512 MiB OS 总内存硬门为 `NOT_RUN`。3c/f488 Dev.app 的 V2 raw probe、packaged Worker structural E2E 和授权参考 CLI 链也都是历史 package receipt；f488 的候选未确认、为 12 Part/896 triangle，且 `chest-shell` 按顺序绑定 chest-shell/chest-panel。历史 `bfa56ac…de9` Dev.app receipt保留；当前 `d9c23b…ac0bd` Dev.app则通过 fresh package/Worker/raw/real-Codex structural Gate和 live Desktop structural activation，并产生相同结构规模的未确认 12 Part/896 triangle/161104-byte candidate；这些 receipt 都不证明参考相似度、材质/PBR V2、export/restore、Viewer hash 或 360°。

MCP005 已满足上述条件：Runtime `supports_reference_import=true`，`reference_import` 在显式 authenticated IPC opt-in 下可用，`reference_get` 为只读工具；真实 Codex CLI evidence 见 `docs/evidence/mcp005/codex-cli-reference-e2e.json`。Codex Desktop 当前 bridge 仍是 `NOT_RUN / unavailable`，不得写成 Desktop PASS。MCP005 的成功只证明真实图片字节进入 CAS，不证明视觉理解、几何或 GLB。

MCP006 已完成 historical development-only Bundle Gate：Runtime 已加载历史 first-party registry，`supports_skill_registry=true`，并通过 `skill_list`、`skill_get` 与 Skill resource 只读暴露 manifest。当前 registry 有 12 个 Bundle；`ponytail-preflight@0.1.0` 的 `skill_get` 同时返回 checked-in `SkillKnowledge@1`，MCP adapter 在新 session 中会先要求读取它。MCP010B 当前源码另外加载并验证 `primitive-blockout@0.2.0`，其 `forgecad.geometry.primitive@2` 是当前唯一 active V2 Skill consumer；历史 Bundle 和新 Bundle 均包含本地合同 schema、Recipe、operator/validator allowlist、合成正/负 fixture、benchmark receipt、许可证、SBOM、provenance 和 development trust manifest。`scripts/check_mcp006_skills.py` 校验 DAG、单位、finite、预算、canonical hash、路径/脚本/网络 capability，并 fail closed。它们不是“已签名安装包”，不执行任意代码，不替代 Geometry/Render 结果；distribution signing/revocation 延后 MCP012–013。

MCP007 已完成 geometry Gate：`geometry_prepare` 只接受 canonical `GeometryProgram@1`，当前 allowlist 为 product-owned box/cylinder/sphere primitive；Runtime 写入 geometry GLB CAS，创建 reviewable candidate/quality report，返回 `GeometryPrepareResult@1` 与 strict `ArtifactReadback@1`。MCP008 已在其上完成 bounded Appearance/Render；MCP009 已完成 limited quality/change/version/export functional core。真实 Codex CLI geometry/readback slice 已 PASS（14 parts/516 triangles，见 `docs/evidence/mcp007/codex-cli-geometry.json`）；`docs/evidence/mcp009/codex-cli-appearance-export.json` 另记录真实图片附件到 appearance、quality、confirm、version 和 CAS-only GLB export 的十二调用 host golden path。MCP010A 已通过最小 Desktop activation write probe；完整 Desktop 3D write、packaged、像素/视觉 gates 仍保持 `BLOCKED/NOT_RUN`，不得把有限主链路扩展成通用质量结论。

### 5.5 永久写工具

- `project_create`（MVP 直接创建项目元数据；完整 prepare/confirm 项目策略仍是后续合同）
- `candidate_confirm`
- `candidate_reject`
- `restore_confirm`
- `export_confirm`

MVP 不暴露 `skill_install_confirm` 或 `skill_disable_confirm`；Skill Bundle 只能读取 first-party development registry，第三方安装留 MCP012。

永久工具必须绑定：

```json
{
  "project_id": "...",
  "base_version_id": "...",
  "prepared_object_id": "...",
  "prepared_object_sha256": "...",
  "quality_report_id": "...",
  "approval_receipt_id": "...",
  "idempotency_key": "..."
}
```

Runtime 必须确认审批未过期、范围和 hash 完全一致、基线未漂移、硬质量门通过。确认结果是不可变版本；同一幂等键重复调用返回同一结果。

请求中的 `approval_receipt_id` 在 MVP 中只是 Codex approval context 的 opaque id；Runtime 不信任它作为最终凭证。confirm/reject/export 成功或记录过期审批时，由 Runtime 在事务内生成 `receipt-...` 的最终持久化 receipt，并在结果中返回该 ID。它是宿主审批流程证据，不是密码学人类签名。

### 5.6 MCP010C 当前工具（source raw 已验证；真实/packaged 视觉门未运行）

| 工具 | Annotation/确认 | 目标合同 |
|---|---|---|
| `render_pass_get` | read-only/idempotent | 只返回已持久化且 hash-bound 的 PNG image block |
| `reference_compare_prepare` | write/temporary | 生成 camera、mask、metrics、diff；不创建版本 |
| `visual_review_submit` | write/evidence | 保存绑定 candidate/render/pass/region 的 typed review |
| `human_visual_review_submit` | write/evidence + confirmation | 保存用户评分；Runtime receipt 不作为密码学身份认证 |

`quality_get` 保持既有只读名称，现可读回 candidate-bound `QualityReport@2`；source synthetic/raw PASS 不等于用户图片 likeness PASS。当前 source manifest 为 36 read + 23 opt-in write = 59；Agentic projection 只读、可重建，durable session/checkpoint/RepairIntent 只代表已验证的 prepare/readback receipt，不是 QualityReport 或 confirmed version 真值；空工具、自然语言结果或 target Schema不能改变 capability 状态。同一 provisional observation 绑定的 packaged Viewer E2E、live C/D/E/F、人评阈值、真实 PBR likeness 和 HQ_360 仍必须独立记录。

MCP010E 的 first-party 离线 AssetPack 由应用资源和 Runtime CAS bootstrap 提供，不新增通用 `material_pack_install` 工具；publisher/install/disable/upgrade/revoke 属 MCP012。

## 6. 参考图片导入

`reference_import` 只接受以下二选一来源：

1. `inline_content`：受合同限制的 MIME、尺寸和字节数；
2. `codex_local_file`：Codex 提供的本地附件路径，但必须位于启动时显式授权的 attachment roots 或 OS 单文件授权内。

路径处理顺序固定：canonicalize → 拒绝 symlink/目录/设备文件 → 检查 root → MIME sniff → size/dimension/decompression-bomb 检查 → 计算 hash → 复制到 CAS → 丢弃原始路径。日志和永久对象不得保存用户名或绝对路径。

P0 Gate 必须分别在 Codex Desktop 和 Codex CLI 上证明实际附件字节能进入 CAS；客户端只让 Codex“看见图片”不算通过。Codex IDE/VS Code/Cursor/Windsurf 的附件传输保留为未来兼容 Gate，不阻塞当前 MCP003/MCP004。若某客户端不能传附件，能力快照必须明确 `unavailable`，不得静默用语言描述替代原图。

## 7. Job 合同

预计超过 10 秒的操作必须在 2 秒内返回：

```json
{
  "schema_version": "RuntimeJob@1",
  "job_id": "job_...",
  "kind": "candidate_compile",
  "state": "queued",
  "project_id": "...",
  "request_sha256": "...",
  "created_at": "...",
  "poll_after_ms": 1000,
  "cancel_supported": true
}
```

状态：`queued | running | waiting_for_input | succeeded | failed | cancelled`。事件为单调序列、可分页和重放；事件只引用 CAS，不含 prompt、图片字节、密钥和绝对路径。Runtime 重启后，终态可读取；非终态只能按已提交 checkpoint 续接，否则转为 typed failure，不能假装继续。

## 8. 错误合同

至少支持：

- `CAPABILITY_UNAVAILABLE`
- `CONTRACT_VERSION_UNSUPPORTED`
- `PROJECT_SCOPE_DENIED`
- `REFERENCE_TRANSFER_UNAVAILABLE`
- `REFERENCE_REJECTED`
- `STALE_BASE_VERSION`
- `CANDIDATE_HASH_MISMATCH`
- `APPROVAL_REQUIRED`
- `APPROVAL_EXPIRED`
- `QUALITY_HARD_GATE_FAILED`
- `SKILL_UNTRUSTED`
- `WORKER_BUDGET_EXCEEDED`
- `JOB_CANCELLED`
- `RUNTIME_RECOVERY_REQUIRED`

错误包含机器可读 code、safe message、retryable、next action 和 evidence IDs；不得返回 stack trace、原始请求、密钥或本机绝对路径。Runtime 已连接但拒绝请求时使用 `INVALID_INPUT`、`STORE_ERROR`、`RUNTIME_BUSY` 或 `IPC_ERROR` 等非 retryable typed code；只有连接/启动/ready handoff 故障使用 `RUNTIME_UNAVAILABLE`。MCP 会丢弃 IPC 错误细节中的路径和用户输入，避免把本机路径泄露给 Codex。

## 9. Codex 配置基线

开发期基线位于 `config/codex/desktop.toml`、`config/codex/cli.toml`、`config/codex/ide.toml`。基线不设置 `CODEX_MCP_PROTOCOL_VERSION`，因此使用 Codex 的 2025-era 默认兼容路径：

```toml
[mcp_servers.forgecad]
      command = "forgecad-mcp"
args = ["serve", "--stdio"]
enabled = true
startup_timeout_sec = 20
tool_timeout_sec = 60
required = false
default_tools_approval_mode = "writes"
```

项目级 `.codex/config.toml` 只允许稳定 `forgecad-mcp` 入口；发布安装器负责把它解析到 `/Applications/ForgeCAD Runtime.app/Contents/Resources/forgecad-mcp` 或等价稳定路径。`required=false` 是生产容错边界：ForgeCAD 故障不得阻断 Codex startup/resume。fixture、临时 Runtime data dir、token 值和用户 Library 路径不能进入正式配置。

## 10. MCP002 已通过的合同 Gate

- 首批 Project/Candidate/Version/Snapshot/Job/Event/Audit/CAS Schema、Rust records 和 manifest 无漂移检查；
- Runtime V1 migration、legacy database rejection、WAL/foreign keys/busy timeout、事务回滚、重启和 backup/restore；
- CAS SHA-256、容量限制、临时文件 + fsync + 原子 rename、missing/corrupt/hash mismatch；
- Runtime migration 前 OS 文件锁、第二 Runtime `RUNTIME_BUSY`、进程退出自动释放锁；
- Unix socket 0600、token hash + constant-time comparison、错误 token fail closed；
- MCP crate 不依赖 SQLite，IPC read dispatch 只返回结构化 Runtime projection。

## 11. MCP003 已完成的本地合同 Gate

- `docs/evidence/mcp003/protocol-snapshot.json` 固定 MCP `2025-11-25`、Codex `2025-06-18` 兼容版本、initialize 字段、method、tools、annotations、resource templates 和 1 MiB projection 上限；
- MCP003 历史 `resources/list`、`resources/read`、`resources/templates/list` 与 17 个只读工具（含 `runtime_status`/`doctor`/`reference_get`）由当时 Rust 单元测试及静态合同检查覆盖；MCP010B 曾在此基础上增加两个默认 read tool，形成历史 19-read 中间 source receipt，不改写原 receipt；当前 C/D/E/F 源码增量已扩展为 33 个只读工具和 18 个显式 opt-in 写工具。
- `npm run mcp003:stdio` 的历史 receipt 校验四个响应、17 个只读工具、能力资源和协议不兼容 fail-closed。历史 19-read MCP010B raw probe 仍只作为中间 receipt 保留；当前 source manifest 应由 MCP010C–F 的 raw probes 校验 36 个只读工具和 23 个写工具。两者都是传输层证据，不等于 required Codex Desktop/CLI 宿主 E2E，也不把 IDE 变成当前 P0 Gate；MCP005 的 reference CLI admission 另由 `script/test_mcp005.sh` 和对应 Codex receipt 覆盖。
- 官方 `@modelcontextprotocol/sdk` 的 `StdioClientTransport` 历史 MCP003 独立探测列出 17 个只读工具、1 个资源并读回 capabilities；当前 MCP010B–F source probe 应验证 33 个只读工具和 18 个显式写工具，并只对当前实际 probe 到的资源数量作出声明。历史 14-tool receipt 仍只描述旧会话，不覆盖当前 manifest；
- Server/Runtime contract mismatch、协议版本不支持、非法 URI、非法 opaque ID 和未实现能力均 fail closed；
- Desktop/CLI/IDE 配置基线不含 secret、绝对路径或现代协议 opt-in；所有基线使用 `forgecad-mcp` 单入口，`docs/evidence/mcp003/host-matrix.json` 记录 required protocol adapter、Codex CLI、Codex Desktop PASS，Desktop 实际 `initialize.protocolVersion=2025-06-18`，forced mismatch 为 `HOST_OVERRIDE_IGNORED / NOT_APPLICABLE`；IDE 保持 `OPTIONAL_NOT_IN_SCOPE`。

## 12. 协议版本边界与宿主诊断

MCP `2026-07-28` 是另一种协议时代：它移除了 `initialize`/`notifications/initialized` 和会话状态，使用 `server/discover`、每请求 `_meta` 与 `requestState`。ForgeCAD MCP003 不宣称支持该现代 wire mode；把它和 2025-era 状态机混在一个未标注的进程中会造成错误的安全和生命周期假设。若配置了 `CODEX_MCP_PROTOCOL_VERSION=2026-07-28`，MCP003 应明确失败，而不是静默降级。待 Codex 宿主和 ForgeCAD 分别完成现代 stdio adapter 合同后，再以独立任务引入。

本地 Codex app-server 诊断和真实 Desktop handshake 已证明当前宿主发送 `2025-06-18`，ForgeCAD 返回相同值；此前只接受 `2025-11-25` 会返回 `CONTRACT_VERSION_UNSUPPORTED`。Desktop 的 `launchctl CODEX_MCP_PROTOCOL_VERSION=2026-07-28` override 被宿主忽略，不能产生真实 mismatch，因此 Desktop 记录 `HOST_OVERRIDE_IGNORED / NOT_APPLICABLE`，禁止代理改写请求伪造 PASS。一次真实、认证的 `codex exec` 只读模型回合完成了 `capabilities_get` 和 `selection_get`，未发生写事务；设置 `CODEX_MCP_PROTOCOL_VERSION=2026-07-28` 的第二次真实回合明确返回 unsupported-protocol、没有工具调用、静默降级或副作用。因此 MCP003 将 2025-06-18 作为显式兼容版本保留，protocol adapter/raw probe 与真实 CLI 共同承担协议负面测试。

## 13. 完整合同 Gate（MVP + release）

- 当前 P0 required：protocol adapter 与 Codex Desktop/CLI smoke；Codex IDE 不属于当前 MCP003/MCP004 发布阻断；
- tools/list、resources/list、Schema、annotations 和 server instructions snapshot；
- 每个工具成功、非法输入、重复请求、stale base、越权、取消、重启和超时测试；
- Viewer 或普通 MCP 适配器关闭不破坏、也不主动终止已经 Ready 的共享 Runtime；显式 authenticated shutdown/update 才停止 Runtime。已确认数据必须可重启读取；Runtime 崩溃或缺少兼容 checkpoint 时，非终态 Job 明确失败；

注意：截至本版本，[官方 `@modelcontextprotocol/conformance`](https://github.com/modelcontextprotocol/conformance) 的 server 命令以 Streamable HTTP URL 为入口，而 MCP003 产品合同固定为本地 stdio。不能把一个临时 HTTP 代理的结果写成 stdio Server 已通过官方 conformance；要关闭这一项，必须另立 transport adapter 合同并分别验证，或采用官方支持 stdio 的 runner。当前证据诚实保留为 `NOT_RUN`，但它不是当前 P0 required host Gate，也不能反向阻塞 MCP003/MCP004。
- Runtime 关闭时 MCP 明确失败且不启动 legacy sidecar；
- 任何永久版本都能回溯到请求 hash、候选、质量、审批、Skill 和工件 hash。

## 14. 版本参考

- [OpenAI Codex MCP 文档](https://developers.openai.com/codex/mcp/)和 [Codex MCP connection manager](https://github.com/openai/codex/blob/main/codex-rs/codex-mcp/src/connection_manager.rs)决定 P0 实际 Codex 配置、默认 legacy 版本和显式 modern opt-in；
- [MCP lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)和 [MCP resources](https://modelcontextprotocol.io/specification/2025-11-25/server/resources)用于协议/资源设计；
- [MCP 2026-07-28 发布说明](https://blog.modelcontextprotocol.io/posts/2026-07-28/)只用于规划未来无握手/无会话 adapter，不代表 MCP003 已实现；
- [MCP Tasks extension](https://modelcontextprotocol.io/extensions/tasks/overview)和 Skills-over-MCP 仍需按客户端能力协商，P0 不依赖它们。

MCP 规范与 Codex 已发布行为可能不同步。`FGC-MCP003` 已 pin 协议版本和配置基线，protocol adapter、认证 CLI 只读/负面回合和真实 Desktop handshake/read-only 回合已有证据；IDE、其他 MCP Client 与官方 transport conformance 仍按未来/非阻塞范围记录。不得把本地适配器或无模型回合 app-server 诊断扩大成未执行的宿主能力，也不得依赖已废弃 Roots/Sampling/Logging。
