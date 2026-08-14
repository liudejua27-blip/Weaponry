# ForgeCAD 测试策略

版本：2026-08-09
状态：MCP001–009 focused Gates 已建立；FGC-MCP010A done；MCP010B structural source Gate、MCP010C renderer/compare source Gate、MCP010D operator/Skill source Gate、MCP010E AssetPack/PBR source Gate 和 MCP010F Viewer/contour source Gate 已通过；Agentic observe/plan projection、scene/stage 嵌套只读 projection conformance 与 durable session/checkpoint/RepairIntent prepare/readback isolated Gate 已通过；durable/reference/DesignSpec producer、单动作 orchestrator、Repair 应用、同一候选的 packaged/human/PBR/export/360 子门仍 `NOT_RUN/BLOCKED`

Stage 0 机器真值入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。源码门 PASS 不等于产品质量 PASS：attempt35 只是 provisional retained observation，它为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，camera 绑定 `MISMATCH`，packaged Viewer binding 为 `NOT_RUN_DIFFERENT_COHORT_AND_ARTIFACT`。

<!-- forgecad-stage0: schemas=101 schema_set_sha256=a48a823ce7d51b214978c966b4cfb27243857f7e6cf594b7c9f4ec47ad1a0c1e read_tools=35 write_tools=21 total_tools=56 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260814-primary-form-framing-bound-viewer.json latest_completed=real-codex-cli-current-20260814-primary-form-coverage-bound-viewer.json -->

## 1. 证据层级

1. Schema/static/fuzz；
2. Core deterministic unit/property；
3. Store transaction/crash/recovery；
4. Worker sandbox/geometry/render；
5. Agentic Runtime durable session/checkpoint：schema fixture、approval/binding、SQLite/CAS persistence、immutable failed checkpoint、CAS-only RepairIntent、Runtime/MCP restart readback 和 public-contract receipt checker；
6. Runtime integration；
7. MCP conformance 和真实 Codex 宿主；
8. Viewer browser + packaged WebView/GPU；
9. 完整 reference→candidate→approval→version→restore→export；
10. 安装/升级/回滚/灾难恢复；
11. 跨类别独立真人质量。

低层通过不能替代高层；每层分别标 PASS/FAIL/BLOCKED/NOT_RUN。

## 2. CI Gate

- contracts generation/check、unknown/oversize/adversarial；
- Rust fmt/clippy/unit/integration；Runtime process lock 在退出后立即释放；Runtime 缺失/启动失败/ready 后崩溃时 MCP initialize 仍成功且 stdio 保持；最多一次有界 restart/backoff；fixture 只在 test child-local env/tempdir 中运行；
- SQLite single-writer、kill、disk-full、WAL、migration、backup/restore；
- geometry/readback/GLB header/lineage validator（MCP007 PASS）；appearance/UV/PBR/fixed-render validator（MCP008 focused PASS）；
- Skill DAG/operator/budget/hash/SBOM/license；分发签名/撤销在 MCP012/013；
- MCP tools/resources/schema/annotations/errors/idempotency/timeout；
- Codex Desktop/CLI smoke（真实发布版本，当前 P0 required）；Codex IDE/VS Code/Cursor/Windsurf 兼容 smoke 为未来非阻塞 Gate；
- Viewer typecheck/build/E2E、单 renderer、a11y、尺寸、GPU fallback；
- packaging/notarization/install/upgrade；
- security/secret/path/content-scope；
- visual benchmark/human gate。

## 3. 强制失败路径

stale base、重复 idempotency、hash mismatch、approval reject/expire、quality hard fail、attachment symlink/越权/炸弹、unknown Skill Operator、DAG cycle、Worker timeout/crash/late result、MCP/Viewer/Runtime kill、disk full、CAS corrupt、renderer unavailable、license/signature revocation。

任何失败不得创建永久版本或泄露内部数据。

## 4. MVP 硬表面 Benchmark

首个 benchmark 使用用户授权的白色硬表面机器人参考，原图只进 CAS，不复制到仓库。Evidence 至少包含 source/CAS hash、尺寸/MIME/授权、typed programs、真实 GLB/readback、wireframe/part-ID、PBR/固定 render、reference metrics、QualityReport、版本 DAG 和用户评分；MCP007 目前只提供 structural geometry/readback，不代表 PBR、渲染或参考相似度。

MVP 不预设一个没有校准的相似度数字作为营销门。MCP009 真实 Codex receipt 已记录 hard checks、fixed render 和明确 limited aspect-ratio evidence；silhouette/landmark/region threshold、真实 Codex typed visual review 和人工接受仍未运行，不能把 host golden path 写成视觉质量通过。

## 5. 通用 3D Benchmark（post-MVP）

数据集按类别、视图数、材质、几何表示和难度分层；机械只能是一类。每条保存授权参考、target claims、RenderSet/AOV、readback、QualityReport、timing/memory、Codex review 和盲评。报告展示每类失败和最差分位，不只展示平均分。

结构指标与视觉指标分开：manifold/UV/PBR/GLB 绿色不等于参考相似；视觉好看也不等于可编辑、版本和导出正确。

## 5.1 MCP010 原子测试矩阵

| Task | 必测 |
|---|---|
| 010A | docs/integrity、安全/许可证、同 revision binaries、raw stdio/CLI、用户重启后的真实 Codex capability/project/build hash |
| 010B | V2 Schema、损坏 index/source/hash/winding/UV、primitive topology/normal、五次 deterministic GLB/readback |
| 010C | synthetic camera recovery、z-buffer/occlusion、九 AOV hash、mask/IoU/F1/landmark/region、四个 MCP 工具和错误合同 |
| 010D | 每个已实现 Operator 正/负 fixture、预算/超时/崩溃、mirror/Part lineage；当前 11 个 D Operator 与 fixed sibling Worker Gate PASS；Manifold 恶意输入/FFI/determinism/source-ID adoption 仍 NOT_RUN，boolean unavailable |
| 010E | AssetPack/hash/license/SBOM、颜色空间、UV/tangent、无 external URI、纹理预算、Runtime readback + glTF Validator |
| 010F | Viewer 单 context、compare/selection/isolate/explosion/a11y、真实 Codex change/confirm/restore/export/restart 同 hash、人工评分 |

当前三分之四参考的阈值和评分见 `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`。单图通过只能写 `PARTIAL_VISIBLE_VIEW_PASS`；五张补充参考未到齐时 360 Gate 必须是 `BLOCKED_REFERENCE_COVERAGE`。

## 6. 真实 Codex Gate

自写 MCP client、fixture 或手工复制附件不能替代 Codex。MVP 已由真实 Codex CLI（带授权 image attachment）证明 reference bytes、geometry/appearance/readback、quality、write approval、version 和 CAS-only GLB export；MCP007 14 parts/516 triangles 与 MCP009 15 parts/580 triangles receipt 均 PASS。Viewer/restart/change/restore 同 hash、Desktop attachment/write surface、像素指标和人工门仍分别补证或明确 unavailable。正式发布才要求 signed package 上的 Desktop + CLI 全量路径。IDE/其他 Client 是未来 Gate。

## 7. Evidence manifest

每个任务 evidence 包含环境、commit/worktree、命令/exit code、合同/二进制/资产 hash、原始 artifacts、日志脱敏证明、未运行项和 blocker。Agentic durable slice 的机器入口是 `scripts/probe_agentic_runtime.py`，合同校验是 `scripts/check_agentic_runtime_receipt.py`，receipt 为 `docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`。Markdown 总结不替代机器收据。
