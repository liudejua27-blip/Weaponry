# ForgeCAD 测试策略

2026-08-22 `CandidateMaterialSurfaceQuality@1` public positive fixture：`Geometry → CandidateTopologyQuality@1 → AppearanceProgram@3 → TextureBuild@2 → SurfaceBake@1 → AppearanceSourceLineage@1 → CandidateMaterialSurfaceQuality@1` 的 `prepare → same-key replay → get → Runtime drop/reopen → restart get` 通过 **1/1（111.72s）**；Runtime focused **5/5**、Store full **74/74**、Contracts **350**。CAS inventory unchanged；stable `artifact_id` 与 GLB object SHA-256、MaterialPack CAS kind 精确区分，合法 UV/tangent rebuild 不计入 geometry-preservation 漂移。该结果仅为 `structural_only`；V2 animated-socket-particles 仍无完整 public `prepare → Store → restart get`，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/candidate-material-surface-quality-public-positive-source-gate-20260822.json`。

最终同 cohort 修订口径：强制 build cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` 下 material focused **5/5（112.63s）**；Runtime full **310 passed / 0 failed / 20 ignored**（330 total，201.91s），且 public material fixture 明确在该 full run 内执行。此前 **111.72s** 仅为 public fixture 单测时长；两者都只支持 `structural_only`，不提升 visual/commercial、human/engine 或 stage/confirm/version/export 状态。

数值口径：当前 source 为 **375 schemas / 26/26 active operators / 85 read + 64 write = 149 tools**；本文的 291/118、284/116、271/112、264/110、257/108、231/100、229/99、227/98、221/96、215/94、210/92、204/91、201/90、197/90、195/90、193/90、191/90、187/89、177/84、175/83、173/82、170/80、168/79、166/78、164/78、162/77、160/76 仅作 historical prior slice 保留。

2026-08-22 `FictionalEnergyVfxAnimatedSocketParticlesSequence@2` 双候选 source slice：Contracts **350**；Store V2 focused **2/2**、Store full **74/74**；Runtime V2 仅低层 focused **6/6**、cargo check **PASS**；MCP V2 **3/3**；同 cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` Runtime full **309 passed / 0 failed / 20 ignored**（191.06s）、MCP full **128 passed / 0 failed / 0 ignored**（1.93s），这些是全量回归，不是 V2 public `prepare → Store → restart get` 正向 fixture。V1/V2 隔离；V2 仅证明 1..16 frame、geometry/appearance 双 candidate/delivery/AnchorSet bridge 以及 Store FK/reachability/idempotence/conflict/rollback 的结构面。完整双候选 public Runtime `prepare → Store → restart get` 正向 fixture 尚不存在，durable end-to-end=`NOT_RUN` / `BLOCKED_FIXTURE_CHAIN`，不能声称正向 durable。该 slice 为 `structural_only`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/fictional-energy-vfx-animated-socket-particles-v2-dual-candidate-source-gate-20260822.json`。

2026-08-20 `bevel@2` focused matrix：Worker 全量 61/61 覆盖 direct authoring mesh、单稳定 edge、segments/profile/width/edge variation、16/24-triangle strict readback、solid/determinism，以及 boundary/non-manifold/multi-edge/oversize/forbidden executable fields fail-closed；Runtime 233/0/12 ignored、MCP 86/86、contracts 195、Skills 12 与 raw stdio 26/26 catalog PASS。它是 source structural matrix，不证明当前候选、visual/PBR/human/package/live/export-restart/HQ360。

2026-08-20 `energy-core@1` focused matrix：Worker 3/3 与全量 60/60 覆盖四组件、4 Parts/768 triangles、Part/source/material exact mapping、deterministic strict GLB/lineage、boundary/non-manifold/winding、负/微小非零 inner radius、solid exact-zero、relationship/unknown field/budget/hash drift；Runtime 233/0/12 ignored、MCP 86/86、contracts 195、Runtime Skill/Profile/Modifier/PDK 与 raw stdio 25/25 PASS。它是 source structural matrix，不证明当前候选、visual/PBR/human/package/live/export-restart/HQ360。

2026-08-19 candidate-bound Modifier Apply focused matrix：contracts 191；Store 24 tests 覆盖 30 秒 bounded same-key single-flight、CAS reservation/shared temporary cleanup、current-head/source/derived evidence 与最终 Part binding 二次校验、late SQL rollback；Runtime 240 tests 通过（另 12 个显式 isolated ignored gate），覆盖 source/derived same-cohort 双回放、target terminal 替换、非目标 Part/source/material/solid/triangle 保持、stale/foreign/hash/Python/reference 负门；MCP 86 tests 通过，raw stdio 另验证 identical replay、Job→CAS sidecar 与 Runtime/MCP 重启回读。这些仍只是 structural/durable 证据，不证明 visual/PBR/human/package/HQ360。

2026-08-19 Authoring Mesh Edit Prepare focused matrix：contracts 189；Store 覆盖原子 candidate/Job/event/audit/evidence/idempotency、current-head/scope、CAS kind/mime/size/reachability、精确 replay、key reuse 和失败零行；Runtime 覆盖 preview TOCTOU 重算、same-cohort Worker、strict GLB/readback/quality/evidence、reviewable-only、no version/confirm/export、临时 CAS 回滚；MCP 覆盖默认隐藏、显式 opt-in、closed nested schema、requiresConfirmation、bounded summary/structuredContent 和 forbidden Python 字段。该矩阵不证明 visual/PBR/human/package/live/HQ360。

2026-08-19 Authoring Topology/Edit Preview focused matrix：contracts 187；Runtime 覆盖 exact durable evidence canonical、bounded CAS、source V/E/Loop/Face、single direct Part、双 Worker replay、strict GLB readback、deterministic canonical hashes与 project/candidate/version/CAS no-write；负测覆盖 stale/cross-project、evidence tamper、unknown/unsorted/zero translate、unknown/interior/non-planar/concave extrude 与 executable fields。MCP 覆盖两个默认只读 closed tools、公开 prepare 字段可调用、完整 structuredContent、自哈希、1 MiB、compiled manifest 54+35=89；真实 raw stdio 在 isolated setup 的 11 Parts/622 triangles 场景完成 topology、translate、extrude，setup 写入与 read slice no-write 分开记录。该矩阵不证明 persistent mesh editor、BMesh/Python/plugin、package/live/visual/human/PBR/export-restart/HQ360。

2026-08-19 historical Render Evidence Replay focused matrix：该历史切片为 contracts 177，现行合同总数为 191；Runtime 覆盖 restart 后 exact integrity reread、current strict GLB/ArtifactReadback 对齐、actual fixed Geometry/Render Worker 同 cohort、persisted/first/repeat 两次重放的九 AOV raw PNG + decoded RGBA8 exactness、deterministic result 与 candidate/version/CAS inventory零写入；cohort unavailable fail closed。MCP 覆盖默认只读、closed nested request、preflight 后 dispatch、unknown Python field 拒绝和 integrity/replay 整响应 1 MiB hard Gate。该矩阵不证明跨平台 determinism、视觉质量、PBR、人评或 Blender renderer parity。

2026-08-19 historical Mechanical Pose Geometry Preview focused matrix：contracts 175；现行合同总数为 191；Runtime 覆盖 three-Part hierarchy、`PoseWorld × inverse(RestWorld)` derived program、fixed Worker compile、strict readback、deterministic repeat、candidate/version/CAS inventory零写入、transient hash不在 CAS、重复 source ownership和 policy negative；独立数值 test 覆盖 Quaternion 与 Worker X→Y→Z Euler 等价及 near-gimbal 拒绝。MCP 覆盖默认只读工具、closed nested request、bounded summary/完整 structuredContent、1 MiB 与 unknown Python field拒绝。该矩阵不证明 original asset rig/pivot provenance、Armature/skin/animation、package/live/Viewer/visual/human/360。

2026-08-19 Subdivision artifact-lineage sidecar focused matrix：contracts 173；Store 10/10 覆盖 candidate/node 唯一、exact request/evidence/CAS kind/mime/1 MiB、相同 link 幂等与冲突/missing CAS、Link 与 reachable 同事务、linked/reachable 拒绝回滚、仅本次 created-new temporary unlinked sidecar 的失败清理，以及 8 路同 hash 并发 put 仅一个 creator；Runtime 覆盖 explicit prepare、immutable canonical sidecar、reopen/restart getter、candidate/version no-write、不同 request、cross-candidate与 corrupt CAS rejection，并在 14×14 level2 / 5,408 triangles / 17,162 elements上验证完整 Link ≤1 MiB；MCP 覆盖默认只读 getter、显式 write opt-in、closed shared input、Link round-trip、disabled-write与工具数。filesystem CAS staging 与 SQLite 不声明跨介质原子；通用 GC 仍属 MCP011。该矩阵不证明跨版本 ID 或视觉质量。receipt：`docs/evidence/mcp010f/blender-subdivision-artifact-lineage-sidecar-source-gate-20260819.json`。

2026-08-19 Subdivision artifact-lineage focused matrix：contracts 170；Runtime 覆盖 V2 geometry prepare → durable evidence/readback、full GLB byte replay、唯一 direct source primitive、128 triangles、四个 control-quad ranges、canonical/hash、read-only state equality和 reopen/restart；错 readback hash与重哈希错误 triangle range拒绝。实际 artifact-bound 大样例为 14×14 level2、5,408 triangles、17,162 lineage elements，完整 MCP envelope 在 1 MiB 下保留至少 4 KiB 余量；16×16 只属于 topology preview，不宣称 artifact-bound 上限。MCP 覆盖 closed read-only tool、bounded summary + complete structuredContent、1 MiB和 stale binding error。该矩阵不证明 persisted sidecar、glTF V/E/C identity、跨版本稳定或视觉质量。receipt：`docs/evidence/mcp010f/blender-subdivision-artifact-lineage-source-gate-20260819.json`。

2026-08-19 Subdivision root-lineage focused matrix：Worker 覆盖 3×3 level2 的 442 elements、最大 16×16 level2 的 22,802 elements、确定性与 budget/unknown fail-closed；Runtime 覆盖独立 V/E/Q/T、control root、edge-chain、quad-range、crease-chain、lineage/canonical 重验及 candidate/version/CAS no-write；MCP 覆盖 closed envelope、bounded summary、完整 structuredContent 与整个 response 1 MiB。root 篡改、重复 evaluated edge、错误 quad root、错误 operator、0/25,001 budget 均拒绝；该矩阵不证明 artifact/GLB lineage 或视觉质量。

2026-08-19 crease-aware Subdivision focused matrix：Worker 覆盖 smooth/dart/two-edge crease/three-edge corner/boundary-junction/level-1-to-2 decay、字节确定 GLB、strict readback，以及 2×2、boundary/reversed/duplicate/unsorted/non-adjacent edge、sharpness 0/3/fractional、129-edge、unknown field 和 stale catalog 负向；Runtime 覆盖 normalized request hash、request-bound result validator、read-only projection/no CAS write 与真实 `geometry_prepare` 128-triangle candidate/readback/no-version；MCP 覆盖 closed oneOf、1 MiB full wire、unknown/cross-branch/fractional fail-closed。package/live/render/visual/human/360 未由这些测试运行。

Boolean Operand Lineage source/focused Gate 覆盖 union/difference/intersection deterministic result、非 Boolean node、run budget 0/overflow、unknown field、request canonical tamper、program/catalog/node/operand/run/hash/canonical 双层绑定、哈希一致但 operation/operand/source-lineage 伪造的 Worker 输出拒绝、Manifold run 从 0 开始且三角面对齐，以及 Runtime/MCP candidate/version/CAS no-write。测试只证明 evaluated-face operand lineage 的结构语义，不证明原始 authoring face 或视觉质量。receipt：`docs/evidence/mcp010f/blender-boolean-operand-lineage-source-gate-20260819.json`。

Render Evidence Integrity 追加覆盖 artifact/reference 原始 CAS byte/hash/size readback、ArtifactReadback artifact ID 等值、JSON CAS 1 MiB 上限，以及 Quality/comparison metric、status、hard-gate、eligibility divergence 负向。该 Gate 仍只是 source structural integrity，不是视觉或人评通过。

2026-08-19 Render Evidence Integrity historical source/focused Gate 覆盖 162-schema closed contract、current candidate restart readback、ArtifactReadback/JSON object hash、camera identity、九 AOV/mask bytes hash与 RGBA8/512×512 解码、RenderProfile/threshold lineage、deterministic repeat、stale camera fail-closed 和 Runtime no-write。该 slice 当时机器面为 162 schemas、19/19 active operators、44 read + 33 opt-in write = 77 tools；历史 attempt35 断言继续原样失败/缺失。

2026-08-19 Mechanical Pose Sequence Preview historical source/focused Gate 覆盖 160-schema check、0/500/1000 tick exact samples、单 tick/序列中点 hash 等价、deterministic repeat、semantic result tamper、unsorted/duplicate/17-sample/duration/null-action/未知字段负向，以及 candidate/version/CAS no-write。该 slice 当时机器面为 160 schemas、19/19 active operators、43 read + 33 opt-in write = 76 tools；source structural PASS 不代替 package/live/Viewer/visual/human/360 Gate。

2026-08-18 historical Parametric Group v2 source/focused Gate：该 slice 当时为 158 schemas，覆盖 closed-schema、三模板确定性、same-template/different-instance hash、semantic result tamper、未知字段/脚本/URL/路径、wrong parameter branch 与 candidate/version/CAS no-write 回归。

2026-08-18 historical Mechanical pose focused Gate：该 slice 当时通过 contracts 156-schema check；Runtime 三层 root/fixed → revolute → prismatic hierarchy、500-tick midpoint、local/world TRS、同义重排 canonical hash、null action rest、candidate/version no-write PASS；cycle、limit、foreign candidate、tampered input 与未知 script 全部拒绝。MCP closed inline Schema round-trip 与 nested unknown field negative PASS。Worker/materialization/package/live/visual/human 未运行。

2026-08-18 Subdivision evaluation v2 focused Gate：contracts 151-schema check；Runtime 0/1/2 levels 精确 8/32/128 triangles、9/25/81 vertices、4/16/64 quads、8/16/32 boundary edges，重复 result 相等，现有 Worker compile triangle count 一致，控制点变化改变 cage/program hash；solid、point-count、adaptive、crease、triangle-budget、unknown nested field 与 input-hash drift 全部拒绝。MCP 第五 closed branch round-trip/unknown/cross-branch/solid negative PASS；Runtime 前后 candidate/version/CAS 不变。package/live/render/visual/human 未运行。

2026-08-18 TopologySnapshot focused Gate：覆盖 primitive closed manifold、开放 SubD cage、bounded same-Part Boolean、corner normal/UV/tangent、重复 canonical hash、read-only candidate/version count、错误 readback hash、1-face no-truncation、513-face request 与 MCP nested unknown field。完整 Runtime 无 feature run 中 10 个既有 render-dependent tests 因固定 sibling 缺失返回 `GEOMETRY_WORKER_UNAVAILABLE`，必须与 topology focused PASS 分列；正式 source Gate 继续使用仓库同 cohort worker 脚本。视觉/package/live Gate 不由本 slice 替代。

2026-08-17 P2 source/package Gate：`repair_intent_run_prepare` 的 CAS intent/observation/reference/camera/candidate 绑定、bounded action scope、staged-only 与 no-confirm boundary 已由 MCP/Runtime focused tests、完整 `script/test_mcp010f.sh` 与最终 Dev.app 真实参考 transport 覆盖；receipt：`docs/evidence/mcp010f/repair-intent-run-source-gate-20260817.json`。packaged transport PASS 但 camera evidence gate blocked；Repair apply、live restart 与视觉质量仍分别 `NOT_RUN/BLOCKED`。

版本：2026-08-09
状态：MCP001–009 focused Gates 已建立；FGC-MCP010A done；MCP010B structural source Gate、MCP010C renderer/compare source Gate、MCP010D operator/Skill source Gate、MCP010E AssetPack/PBR source Gate 和 MCP010F Viewer/contour/Mechanical pose source Gate 已通过；Agentic observe/plan projection、scene/stage 嵌套只读 projection conformance 与 durable session/checkpoint/RepairIntent prepare/readback isolated Gate 已通过；Mechanical pose 仅为 read projection，不是 Armature/skin/animation asset；durable/reference/DesignSpec producer、通用单动作 orchestrator、Repair 应用、同一候选的 packaged/human/PBR/export/360 子门仍 `NOT_RUN/BLOCKED`

Stage 0 机器真值入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。源码门 PASS 不等于产品质量 PASS：attempt35 只是 provisional retained observation，它为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，camera 绑定 `MISMATCH`。packaged Viewer 当前只能记为 `PASS_CURRENT_COHORT_BOUND_READ_MODEL`；UI E2E、正式 VoiceOver、视觉和人评仍分别 `NOT_RUN`。

<!-- forgecad-stage0: schemas=411 schema_set_sha256=b8fb7befc5870a51fe3919767c8953065e4e0da718c6eed1eda2d1c858a45f30 read_tools=91 write_tools=69 total_tools=160 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->

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
| 010D | 每个已实现 Operator 正/负 fixture、预算/超时/崩溃、mirror/Part lineage；当前 19-entry/19-active source catalog 与 fixed sibling Worker Gate PASS；Manifold bounded C ABI/FFI、恶意输入/确定性/source-ID/残余切线 focused Gate 已通过，任意 mesh Boolean 仍 unavailable |
| 010E | AssetPack/hash/license/SBOM、颜色空间、UV/tangent、无 external URI、纹理预算、Runtime readback + glTF Validator |
| 010F | Viewer 单 context、compare/selection/isolate/explosion/a11y、真实 Codex change/confirm/restore/export/restart 同 hash、人工评分 |

当前三分之四参考的阈值和评分见 `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`。单图通过只能写 `PARTIAL_VISIBLE_VIEW_PASS`；五张补充参考未到齐时 360 Gate 必须是 `BLOCKED_REFERENCE_COVERAGE`。

## 6. 真实 Codex Gate

自写 MCP client、fixture 或手工复制附件不能替代 Codex。MVP 已由真实 Codex CLI（带授权 image attachment）证明 reference bytes、geometry/appearance/readback、quality、write approval、version 和 CAS-only GLB export；MCP007 14 parts/516 triangles 与 MCP009 15 parts/580 triangles receipt 均 PASS。Viewer/restart/change/restore 同 hash、Desktop attachment/write surface、像素指标和人工门仍分别补证或明确 unavailable。正式发布才要求 signed package 上的 Desktop + CLI 全量路径。IDE/其他 Client 是未来 Gate。

## 7. Evidence manifest

每个任务 evidence 包含环境、commit/worktree、命令/exit code、合同/二进制/资产 hash、原始 artifacts、日志脱敏证明、未运行项和 blocker。Agentic durable slice 的机器入口是 `scripts/probe_agentic_runtime.py`，合同校验是 `scripts/check_agentic_runtime_receipt.py`，receipt 为 `docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`。Markdown 总结不替代机器收据。
