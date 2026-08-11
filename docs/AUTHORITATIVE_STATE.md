# ForgeCAD 权威状态与版本真值

版本：2026-08-10
状态：MCP005–009 functional truth 已实现；FGC-MCP010A done；MCP010B structural truth source Gate 已通过但 Darwin OS memory hard cap deferred/NOT_RUN；MCP010C source-focused renderer/compare/review Gate 已通过，首次真实机器人 compare/review transport 已运行但 likeness threshold `FAIL_QUALITY_TARGET_NOT_MET`，Viewer/package/人评门仍 NOT_RUN/BLOCKED；MCP010D–F blocked

## 1. 真值层级

1. **Runtime V1 SQLite + CAS**：项目、候选、版本、Job、Skill、审批和工件唯一持久真值；
2. **公开 JSON Schema + canonical serialization**：对象合法性和 hash 规则；
3. **`ActiveDesignSnapshot`**：当前项目状态的单一只读投影；
4. **Worker receipts/readback**：对特定输入和工件 hash 的事实；
5. **Render/Quality evidence**：对特定 candidate/version 的检查；
6. **MCP/Viewer projection**：可丢弃、可重建的展示；
7. **Codex 对话/自然语言**：意图与解释，不是产品状态。

GLB、图片、`.blend`、Three.js scene、prompt、Skill 文档和 Codex 评价都不能单独成为版本头。

MVP 具体规则：Reference truth 是 CAS 原始字节 + `ReferenceEvidence`，不是本机路径；Geometry truth 是 canonical `GeometryProgram` + worker receipt + mesh/GLB readback，不是 `.blend` 或 Viewer scene；Appearance truth 是 typed MaterialZone/AppearanceProgram；Render/Quality 只证明同一 candidate hash；导出是 confirmed version 的派生物，不反向成为版本头。

### 1.1 MCP010 当前与目标真值

MCP010B 当前源码增加 8 个合同，MCP010C 再增加 7 个合同，当前共 59 个 JSON Schema（MCP006 历史为 44）。B 的 `GeometryProgram@2`/strict readback/restore evidence source Gate 已通过；Darwin 512 MiB OS memory hard cap 仍 deferred/NOT_RUN。C 的 `ReferenceViewSpec@1`、`CameraCalibration@1`、`RenderSet@2`、`ReferenceComparisonReport@1`、`VisualReviewReport@1`、`HumanVisualReviewReceipt@1`、`QualityReport@2` 已由 Runtime/MCP producer/consumer 使用；固定 512×512 perspective/z-buffer renderer、九 AOV、local mask/metrics、MCP image block 和 Codex/human review 的 source raw Gate 已通过。首次真实机器人 PNG 运行也完成九 AOV/compare/review transport，但 primitive blockout 的 likeness threshold 未通过；脱敏证据见 `docs/evidence/mcp010c/real-reference-robot.json`。MCP010A/010B 的历史 Dev.app receipts仍原样保留，不能替代 C packaged/live/Viewer evidence。C synthetic/raw 和本次真实 transport receipt 都不证明用户机器人 likeness、PBR、独立人评或 360°。

`AppearanceProgram@2`、PBR V2/纹理、Viewer compare/selection/explosion 和 first-party AssetPack 仍属于 MCP010D–F。C 的 RenderSet@2/比较/评审已进入 Runtime-owned producer、CAS artifact、严格 readback、固定 render、QualityReport 和 evidence lineage；但 source raw Gate 使用 synthetic reference，仅证明合同与绑定。当前 Runtime Skill registry 的 11 项中，只有 `primitive-blockout@0.2.0` 因 primitive@2 真实 consumer 被标为 active；这不产生材质、纹理、PBR 或视觉相似度。Darwin 512 MiB OS memory hard cap、人评阈值、Viewer/package/live C 和 360 更不能由结构 PASS 推导。

最新 `d9c23b…ac0bd` 开发包记录了 Bundle 知识分支的校正：`limited` 只阻断视觉质量声明，`STRUCTURAL_BLOCKOUT` 仍需用户明确选择并经过相同 Runtime geometry/readback/approval 硬门。该包的 isolated raw/real-Codex V2 structural 通过；用户完整重启后它已成为当前 live Desktop cohort，并完成 32 工具、Ready/doctor、cohort/catalog/hash 与项目只读回读结构激活。

目标 `HumanVisualReviewReceipt` 只证明用户评分绑定到特定 reference/camera/render/candidate hash，不证明模型身份，也不能覆盖 Geometry/UV/PBR 硬门。当前单张参考只能产生 `PARTIAL_VISIBLE_VIEW_PASS`；`HQ_360_PASS` 在多视图完整前固定 blocked。

## 2. 核心对象

### Project

包含 project ID、名称、创建时间、policy/profile、active snapshot revision。无绝对本机路径和模型/Provider 信息。

### ReferenceEvidence

保存 CAS hash、MIME、尺寸、用户授权声明、导入方式、视图/相机 claims 和派生证据 lineage。原始绝对附件路径入 CAS 后丢弃。

### Candidate

未确认、可 GC 的完整构建单元，引用：base version、SubjectProfile、RepresentationPlan、AssemblyGraph、Geometry/Appearance programs、Skill receipts、artifacts/readback、RenderSet、QualityReport、SemanticChangeSet 和状态。

Candidate 状态：`prepared → compiling → evaluating → reviewable → confirmed | rejected | failed | expired`。只有 `reviewable` 且 hard gates 通过者可 confirm。

### DesignAssetVersion

不可变提交，至少包含：version ID、project、parent version、confirmed candidate hash、assembly/program/material/texture/artifact manifests、quality、approval、created_at 和 canonical digest。任何修改都创建新子版本。

### ActiveDesignSnapshot

```text
project_id
snapshot_revision
confirmed_version_id | null
review_candidate_id | null
runtime_capabilities_digest
selection_projection_revision | null
updated_at
```

Snapshot 不复制完整模型，不合并两套 `vN`，不按导出格式切换版本链。Viewer/localStorage 不能写它。

### RuntimeJob

持久 job ID、kind、project/candidate scope、request hash、state、event cursor、checkpoint/result/error refs 和取消状态。事件只追加、可重放；大内容只引用 CAS。

### SkillExecutionReceipt

绑定 Bundle/Recipe/Operator/Validator/asset/SBOM/signature hash、canonical input/output、预算和结果。不记录模型 prompt 或任意执行环境。

### ApprovalReceipt

由 Runtime 接收 Codex write approval 后创建，绑定 user-visible summary、tool、project、base version、prepared object ID/hash、quality report、expiry、decision 和 session。它不证明模型身份，只证明本地审批事务。

## 3. 写入不变量

- 只有 Runtime 进程持有 SQLite/CAS 写权限；启动时先取得 OS 独占 writer 文件锁，第二实例返回 `RUNTIME_BUSY`；
- `prepare` 不移动 confirmed head；
- `confirm` 在单一 SQLite 事务中校验 base/hash/quality/approval/idempotency、写版本、更新 snapshot、追加 audit；
- 同一 idempotency key + request hash 返回同一结果；同 key 不同 hash 拒绝；
- stale base 不自动 rebase 或 last-write-wins；
- rejected/failed/expired candidate 永不确认；
- 质量报告只能附着其 input artifact/candidate hash；
- export 只能引用 confirmed version，或明确标记为 unconfirmed diagnostic；
- CAS 对象以内容 hash 寻址，DB 事务提交前验证存在/尺寸/hash；
- GC 只能删除无 reachability 的临时候选工件，不能删除已确认版本、审批、audit 或其依赖。

## 4. 局部修改

`SemanticChangeSet` 必须引用 base version、Part/MaterialZone/source-map 稳定 ID 和 allowlisted operation。Runtime 校验 scope 后生成新 candidate，重新编译受影响 DAG 并复用未影响 hash。不能接受任意 JSON pointer、vertex buffer patch、脚本或路径。

Viewer selection 只是提示；prepare 时必须重新绑定当前 snapshot/part。Part 已不存在或版本漂移时返回 typed conflict。

## 5. Undo、Reject、Restore

- `undo/redo`：只作用于同一未确认 candidate 的 typed change stack；
- `reject`：终止 candidate，不改 confirmed version；
- `restore_prepare(version_id)`：从历史内容产生基于当前头的新 candidate；
- `restore_confirm`：批准后创建当前头的子版本，历史版本保持不变；
- 禁止移动数据库指针伪装新版本，禁止覆盖旧 GLB/CAS 对象。

## 6. 爆炸图

默认爆炸图是由 confirmed AssemblyGraph 派生的 `ExplodedViewPlan`。临时距离只存在 Viewer；保存计划必须产生 candidate/change/approval/version。Plan 引用稳定 Part ID，不能以渲染 primitive 顺序作为唯一身份。

## 7. 导出

`export_prepare` 生成 manifest 与 CAS-backed artifact reference，绑定 confirmed version、format/profile、artifact hashes、validator/readback、license/provenance 和 toolchain。MVP `glb/mvp-glb` 的 `export_confirm` 原子确认 receipt 并返回 `output_sha256`，不写任意本机路径；filesystem/package target 属 MCP013。导出目录不得成为版本真值。

如果 Viewer、candidate、quality、export 的 version/hash 不一致，导出 fail closed。导出包不包含绝对本机路径、secret、prompt、原始 Codex attachment path 或未授权资产。

## 8. 旧数据

旧 `ConceptVersion`、`ModuleGraph`、`AgentAssetVersion`、Thread/Turn/Item、Provider 和 migrations 仅属于只读归档。新 Runtime V1 不自动打开旧 DB，也不把旧 `vN` 投影为当前 snapshot。

一次性离线工具可以读取备份、校验旧工件、生成中立 export manifest，再由用户显式导入新项目。失败不修改旧库或新库。用户数据删除需要独立明确授权。

## 9. 重启与灾难恢复

重启时 Runtime：取得 OS writer 文件锁 → 验证 DB migration/version → CAS reachability → snapshot/version hashes。MVP 不使用 TTL lease、heartbeat 或 stale takeover；未完成 Job 的跨 MCP 会话恢复暂不承诺，无法安全恢复的 Job 转为 typed failure。已确认版本必须在 MCP/Viewer 不可用时仍可离线备份和校验。
