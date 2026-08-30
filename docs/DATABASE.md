# ForgeCAD Runtime V1 数据库

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

## Weaponry durable additions

数据库 successor 需要持久化授权 manifest、AuthoringDocument head、完整原子 transaction journal、
每个 immutable child revision、ModifierGraph、evaluation receipt、High/Low/UV/Cage/Bake lineage、
FPS/engine/human acceptance receipts。事务中任一 command、CAS link 或 validation 失败，所有
revision/link/job/idempotency 记录必须一起回滚；不得只持久化 final mesh 而丢失编辑历史。

> 2026-08-29 已实现 `AuthoringMesh` transaction durable slice：Store 保存 exact journal CAS object、aggregate transaction record、每个 immutable child revision、final/revision object lookup、scoped idempotency 与 GC roots。base revision 必须同时匹配 revision ID/index/SHA、mesh/lineage 与 CAS topology。所有数据库行与 reachable roots 在一个 SQLite transaction 中变为可见；CAS 文件在此前按 reservation staging，失败时清理。这是可验证的原子可见性，不是宣称 SQLite 与文件系统具有跨介质分布式 ACID。

> 2026-08-27 `FPS-FORM-04AR`：`production_weapon_form_art_baselines` 的 source identity 已升级为 `(registration_lineage_id, candidate_id, artifact_sha256, runtime_build_cohort_sha256)`；启动 migration 保留旧行/CAS，重建 cohort-aware unique index 与 update trigger，允许同一不可变 source 在新 cohort 生成新的 baseline，而不覆盖历史。proposal FormArt Store closure 对 fresh/historical receipt 使用互斥验证。真实 D1 migration、current-cohort baseline、proposal evidence 与 `PRAGMA integrity_check=ok` 已完成。

> 2026-08-26 `04AF` 持久化真值：`AuthoringMesh@2` 已在 Runtime 单写者下保存 immutable genesis 与 split-edge child revision、稳定 V/E/H/F/Corner identity、tombstone 和 CAS object，并通过 Runtime drop/reopen exact readback。该记录是 structural-only 隔离资产，未写入真实 D1 武器 lineage。CameraLock child 因缺 orientation-specific 用户回执没有创建成功 row；不得以诊断性 180° 填补数据库真值。

> 2026-08-26 现行 source：**527 schemas / 115 read + 87 write = 202 tools**。真实 D1 已持久化 source-bound AuthoringMesh genesis 与 stable-ID `MoveVertices` child revision，并由派生 candidate 完成重启回读；这不是 secondary approval。CameraLock child、Low/UV/High durable 与 MaterialLayerGraph plan 的历史 structural rows 仍不能替代完整商业资产链。

> 商业 Hero Weapon 的 durable 对象必须按 Design/Authoring/Production/Surface/Presentation/Delivery/Approval 分层保存独立 CAS roots 与 lineage；算法库内部 handle、GLB index 和引擎导入对象不得成为数据库真值。详见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 商业持久化目标：新增能力按 additive migration 保存独立 `AuthoringMeshRevision/HighRecipe/HighArtifact/LowDraft/LowAuthoringMesh/Correspondence/Cage/Bake/MaterialGraph/FpsPackage/EnginePackage/EngineValidation/HeroArtReview` link 与 CAS roots。Undo 是 revision/head 移动，不是 Worker 内可变历史；source 与 evaluated element ID 分 namespace，删除 ID 保留 tombstone 且永不复用。尚未实施的表不得在状态文档中写成已存在。

> 2026-08-26 Formal High 持久化同步：`production_weapon_formal_high_links` 已持久化 public `idempotency_key`，并以 `(project_id, session_id, idempotency_key)` 唯一索引实现 exact replay/conflict。既有未发布开发表采用 additive column + deterministic legacy artifact key 迁移，不删除旧行。Store 3/3 聚焦测试通过；完整 Runtime source-lineage positive/restart/cleanup 仍未运行，因此公共幂等已证明但端到端 Formal High capability 仍未证明。

> 2026-08-26 当前持久化增量（当前 source：**515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools**）：Store 新增 additive `low_quad_draft_durable_links` 与 Hero UV durable link 路径，由 Runtime 单写者保存 exact project/candidate/state/base-version、candidate-bound current Low exact provenance、Worker result、Low/Hero UV artifacts、strict readback、CAS hashes 与 idempotency binding；公共 `hero_uv_durable_get` 为零写回读，`hero_uv_durable_prepare` 仅经 authenticated explicit write opt-in 暴露。Hero UV 的 Store→Runtime→MCP public chain 与真实 prepare→同键重放→Runtime drop/reopen→get 当前 cohort **1/1 PASS**，恰有四个 Hero CAS roots linked 并纳入 reachability/GC；getter 不新增 CAS 对象、不修改 candidate、不创建 version 或 Stage transition。该结果仅为 structural/source pass，不是 artist-authored unwrap、visual、human、engine、commercial 或 packaged pass；`FPS-HIGH-05=NOT_PASSED`、Stage=`camera-calibrated`、visual=`QUALITY_TARGET_NOT_MET`，不 confirm/version/export。证据：`docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`。

> 2026-08-26 Cage/Bake 持久化边界：Store 已有 High、Low、Cage、Correspondence、BakePlan、Diagnostic、BakeReceipt 七类记录的单事务 commit、exact replay/conflict rollback、owned CAS reachability/GC 与重启 reverify。它只接受 Runtime 生成并验证的完整 bundle；MCP/Worker/caller 不能逐表写入或提交自报 hash。当前 Runtime 对全新 prepare 因七类正式 producer 未闭合返回 `PRODUCTION_WEAPON_HIGH_LOW_BAKE_PRODUCER_UNAVAILABLE`，所以当前 D1 没有这七类正向持久化记录；public seam 存在不等于数据库已有商业 Bake 真值。

2026-08-25 `CQ-02-TYPED-TOPOLOGY-IDENTITY-LINEAGE`：`authoring_mesh_edit_preview → authoring_mesh_edit_prepare` 的 `split_edge / collapse_edge / dissolve_edge` proof 仍保持 source-element-only；下游 Runtime 只从 Store 的 exact candidate→idempotency response 恢复该 proof，并把 parent source identity 物化为 durable `AuthoringMeshIdentityLineage@1` child IDs、单调 tombstone及 one-to-many/many-to-one relation，不接受 caller identity/proof arrays。真实 split/collapse/dissolve 已分别完成 `edit preview/prepare → reviewable candidate → durable AuthoringMesh → IdentityLineage → Runtime drop/reopen/get` 独立完整链路，合计 **3/3 PASS**；Store `authoring_mesh_` **12/12**、MCP IdentityLineage **3/3**、490-schema checker与 Contracts/Store/Runtime/MCP 联合 compile PASS，工具数仍 **106 read + 78 write = 184**。general correspondence、evaluated retarget、完整 selection/undo history 与产品级 cross-version editor仍 `NOT_PROVEN`。Stage 保持 `camera-calibrated`，视觉=`QUALITY_TARGET_NOT_MET`，human/engine/distribution=`NOT_RUN`，HQ360=`BLOCKED_REFERENCE_COVERAGE`。回执：`docs/evidence/mcp010f/authoring-mesh-typed-topology-identity-lineage-materialization-source-gate-20260825.json`；原 source-proof 回执继续作为上游证据。

> 2026-08-25 目标持久化边界：商业资产阶段需要保存 Brief/ArtDecision、AuthoringMesh、High/Low/Cage、Hero UV、Bake、Material Layer、LOD/collision/socket、engine/human receipt 的 immutable identity 与 lineage；其中 Low/Hero UV 与 Cage/Bake public Store seam 已有窄幅 source 实现，但完整当前候选生产链仍不存在。不得从目标或空表推导 live asset truth；任何新增仍须由 Runtime 单写者、幂等/restart/rollback/reachability 和正向 candidate receipt 后才成为真值。

当前 Store 明确分离 `authoring_mesh_projection_indexes`、`authoring_mesh_durable_records`、`authoring_mesh_identity_lineage_durable_records` 与 edit `write_idempotency` response：Runtime按 target candidate/request hash只读恢复 exact typed proof，caller不能替换 correspondence/tombstone arrays。IdentityLineage transaction验证 immediate-parent active/current active关系、闭合 cardinality、operation hash、单调 tombstone及ID non-reuse，并拒绝重复 parent relation。Store `authoring_mesh_` focused **12/12**、联合 cargo check、Runtime真实 split/collapse/dissolve 独立 full-chain restart **3/3** 与MCP **3/3** PASS。generated child IDs已写入IdentityLineage durable CAS/record；完整编辑历史和商业级 AuthoringMesh仍 `NOT_PROVEN`。

版本：2026-08-09
状态：MCP004 事务基座、MCP005 reference evidence/CAS、MCP007–009 3D artifact/render/quality/change/export lineage functional core 已实现

## 1. 断代策略

新 Runtime 使用独立 `migrations-runtime-v1/0001_runtime.sql`，不在旧 migration 上继续叠加。MCP002 Store 创建 schema marker、Project/Snapshot/Candidate/DesignAssetVersion/Job/Event/Checkpoint/Object/Artifact/Approval/Audit 表，并在事务中校验版本。已有数据库若没有 `schema_meta` 会直接拒绝，避免误打开旧 Library。旧 `WushenForgeLibrary`、SQLite 和 CAS 只读备份；新 Runtime 不自动打开、迁移或写入它们。Runtime 在打开 Store 前取得 OS 独占 writer 文件锁。

一次性离线导出器可在后续任务读取旧库、校验工件并生成中立 manifest；用户显式选择后再导入新项目。失败不得修改任一数据库。删除旧用户数据需要单独明确授权。

## 2. V1 表域

```text
projects
active_design_snapshots
reference_evidence
candidates
semantic_change_sets
design_asset_versions
approval_receipts
write_idempotency
runtime_jobs
runtime_job_events
runtime_job_checkpoints
objects
artifact_manifests
material_graphs
texture_sets
uv_layouts
render_sets
quality_reports
skill_bundles
skill_execution_receipts
export_manifests
audit_events
```

不创建 threads、turns、items、conversations、Provider registry/budget/snapshot、coding/search/vision/remote-3D jobs、legacy concept/module/weapon 表。

## 3. 单写者与事务

只有 `forgecad-runtime` 取得 SQLite/CAS 写权限。MCP、Viewer、Workers 不持有 DB handle。SQLite 使用 WAL、foreign keys、busy timeout、明确 transaction boundary 和 schema version。Runtime 进程持有 OS 独占 writer 文件锁；第二实例立即返回 `RUNTIME_BUSY`，进程退出或崩溃后由操作系统释放锁。MVP 不使用数据库 TTL lease 或 heartbeat。

永久 confirm 在一笔事务中：验证 base/candidate/quality/approval/idempotency/CAS → 写 immutable version → 更新 snapshot → 写 audit。restore 以历史 confirmed version 为内容来源、以当前 head 为父，永不移动历史指针；diagnostic export 只写 hash-bound manifest/CAS 和 export receipt，不写本机路径。事务失败无部分版本；相同 idempotency key 重放原结果，不同 request hash 拒绝。MCP004 核心通过 authenticated IPC 提供，并由显式 opt-in wire adapter 转发；默认 MCP stdio 仍保持 MCP003 只读。

## 4. CAS

对象按 SHA-256 寻址，DB 保存 `sha256`、size、MIME、kind、created_at、reachability class。`object_path` 只在 Store 内部派生，不保存用户路径。写入采用临时文件、fsync、hash recheck、原子 rename；同 hash 不同字节、容量超限、缺失或篡改均 fail closed。MCP002 已通过 CAS roundtrip、capacity、hash mismatch、corruption 和 backup/restore tests；Hero UV durable prepare 额外证明四个 Hero CAS roots 的 link/reachability/GC 归属，通用 GC/reachability policy 仍留给 MCP011。

事件和日志不存大内容。GC 先计算 reachability，再 quarantine，经过 grace period 和 manifest 复验后删除；confirmed version、audit、approval、export 和旧库备份不可 GC。

MCP005 的图片原始字节直接进入 CAS；数据库只保存 opaque reference ID、CAS sha256、MIME、byte size、width/height、授权声明和派生 object refs，不保存 attachment absolute path。MCP007–009 的 geometry/GLB/render/quality 也只保存 CAS refs 与 lineage。新增 migration 必须在 Runtime 取得 OS 文件锁之后执行。

## 5. 路径与隐私

数据库不保存原 Codex attachment path、用户名、HOME、workspace absolute path、secret、prompt 或图片原始字节。导出包保证 `no export package contains absolute local paths`，只包含相对逻辑路径和 CAS/manifest hash。

## 6. 迁移、备份、恢复

- migration 单向、事务化、可在副本测试；
- 升级前 consistent DB snapshot + CAS manifest；
- 恢复先在隔离 Library 验证 integrity/reachability/version，再原子切换；
- migration 失败保持旧版本可读，不能半升级；
- 灾难恢复定期证明全量和增量备份，见 `DISASTER_RECOVERY.md`。

## 7. Gate

并发双 writer、kill -9、磁盘满、CAS missing/corrupt、WAL recovery、duplicate idempotency、stale base、migration failure、backup/restore、GC race 和路径泄露均必须自动测试。

## 8. 商业资产持久化边界（future / queued）

商业阶段目标对象包括 `AuthoringMesh@1`/identity lineage、`HighMeshArtifact@1`、`LowMeshArtifact@1`、`HeroUvLayout@1`、`CageArtifact@1`、`HighLowBakeReceipt@1`、`MaterialLayerGraph@1`、`HeroLodSet@1`/collision/socket、`EngineValidationReceipt@1` 与 `HeroArtReviewReceipt@1`。这些目标对象当前不得被推断成已存在的 V1 表；新增表/记录必须先通过 additive migration、Runtime 单写者、幂等/restart/rollback、CAS reachability/GC 与同 cohort producer/consumer Gate。

每个持久对象必须保存 immutable identity、parent/input/output/module/contract/worker cohort hashes、candidate/export binding、budget/fixture receipt、LICENSE/NOTICE/SBOM/provenance/signature refs。目标 Worker 的 `ForgeCadModule@1` manifest 只能由 Runtime 写入 link；MCP、Viewer、Worker 不持有 DB handle。没有签名和完整 receipt 时只保存诊断或 `queued` 状态，不能改变 Stage、confirmed head、version 或 export。

当前 Hero UV durable 的四个 CAS roots 已由 Runtime link 并纳入 reachability/GC，`hero_uv_durable_get` 是零写回读；真实 prepare→same-key replay→Runtime drop/reopen→get 为 1/1 PASS，但仍是 structural/source，不是 artist unwrap、visual、engine 或 commercial durable truth。当前 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools** 继续作为 source 口径。
## 2026-08-26 CameraLock registration child

`agentic_production_camera_lock_registration_lineages` 是 `ProductionCameraLock@1` 的 additive、success-only 不可变子表；它把 exact parent receipt、GeometryProgram、semantic ordering、authored orientation 和 RegisteredRigV2 CAS objects 绑定到同一 candidate/state/artifact/reference。Store 在同一事务内验证 parent、CAS kind/mime/hash/canonical、promotable orientation、replay/conflict，再把所有 lineage objects 设为 reachable。该表不更新 ProductionStage head，不确认 candidate，不创建 version/export。
