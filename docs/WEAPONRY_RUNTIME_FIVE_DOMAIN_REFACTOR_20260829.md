# Weaponry 刀类 Runtime 五域重构账本 — 2026-08-29

状态：`WPN-ARCH-RUNTIME-STORE-SPLIT-001_SOURCE_PHYSICAL_BATCH_001 / OVERALL_PHYSICAL_EXTRACTION_PENDING / REQUEST_SCHEMA_CLOSED_125_OF_125`

> 2026-08-30 current addendum：第一批纵切把 Runtime Evaluation reference-comparison/visual-evidence family 与 Store Delivery ApprovalLifecycle family 的真实实现移出聚合根，并把 compatibility session/checkpoint/recovery 生产职责移出其聚合文件。当前 Runtime/Store/MCP default/compat/agentic roots 为 **51,603 / 78,865 / 996 / 19,332 / 16,532** 行，Runtime root modules=92，fresh dep-info=10/44。下一步仍是 `WPN-ARCH-RUNTIME-STORE-SPLIT-002`，不是 RETIRE；完整现状见 `docs/architecture/weaponry-runtime-current.html` 与 `docs/evidence/weaponry/wpn-arch-runtime-store-split-001-source-gate-20260830.json`。

当前源码元数据：`schema_count=658`、`schema_set_sha256=29784beef684ae4334bfc2983f19fec25694c632ed11e0840bd12b0e9838f0f1`、`runtime_source_sha256=893be325dbd1f057791e3cfed815b7fd2c17517379b09c9ad6df795a9ab6483c`、`truth_canonical_sha256=8c77ccb9d3829553444fdd04904076cd26ad3037bc929cc464a20c015fcb0172`；source-only compatibility summary `cohort=null`、`131/95/226`、SHA-256=`1eb6cf5125e4d72aa2e8eef0139ff11de8c69b615d47cb66f70b666fb83377ca`。

本账本只描述当前源码的架构事实，不代表 High→Low→UV→Bake、视觉质量、真人验收或商业交付通过。

## 1. 第一性原理结论

Weaponry 的核心问题不是“工具名字太多”，而是三个不同维度被混在同一批巨型入口文件中：

1. **产品动作面**：Codex 默认应只看到刀类工作流；
2. **领域所有权**：Authoring、Evaluation、Surface、Presentation、Delivery 应各自拥有清晰的命令、查询和记录；
3. **历史兼容面**：226 个 raw operation 只负责 replay/migration，不能反向塑造当前刀类架构。

把 226 个 operation 包进 11 个 façade 只解决第一个维度。若默认路径仍先构造完整 registry，或 Runtime/Store 仍靠巨型 `lib.rs` 共享隐式状态，内部认知空间并没有真正收敛。

另一个需要纠正的假设是“每个公共能力都必须有一个 Store record”。这对读投影和纯计算不成立。可执行的不变量应是：

- durable write：`Contract → Runtime command service → Store transaction/record → MCP façade`；
- read projection：`Contract → Runtime query service → Repository projection → MCP façade`；
- bounded compute：`Contract → Runtime evaluator → typed Worker/result validation → 可选 evidence record → MCP façade`。

强行给 preflight、observe 或临时计算伪造持久记录，会制造第二套假真值。

## 2. 当前源码基线

| 根入口 | 当前行数 | 当前根模块声明 | 判断 |
|---|---:|---:|---|
| `forgecad-runtime/src/lib.rs` | 51,603 | 92 | 五域均已接 direct typed service/router；Evaluation 第一 family 已迁出，巨型根仍含其余跨域实现 |
| `forgecad-store/src/lib.rs` | 78,865 | 15 | ApprovalLifecycle 已迁出；socket/anchor、ReadModel/QualityEvidence 与其余 Presentation/recovery records 仍集中 |
| `forgecad-mcp/src/main.rs` | 996 | 默认 dep-info 10 源文件 | 默认入口、active session/manifest/schema/result adapter 已物理隔离；125/125 request Schema 闭合 |
| `forgecad-mcp/src/compat_main.rs` | 19,332 | compat dep-info 44 源文件 | 历史 226-tool replay 的显式兼容二进制；默认构建不编译；内部 aggregate 仍偏大 |

新增边界文件不会自动降低这些数字。当前成果是建立可测试的迁移接缝，不是完成代码搬迁。

Surface 与 MCP-SPLIT 前序原子曾记录 Runtime/Store `52,854/80,878` 与 `52,542/79,841` 行；这些 historical snapshot 及其 receipt 保持原样。当前源码只采用 `51,603/78,865`，不回写任何前序 receipt。

## 3. 已落地的边界

### 3.1 默认 MCP 与 compatibility

- 默认 Knife profile 只发布 11 个 bounded façade；
- 125 个当前 backing operation 已变成唯一 façade owner，重复归属为 0；
- 默认 `tools/list`、默认 capability projection 和默认 `tools/call` profile 路由不再先构造 226-tool raw registry；
- 历史 registry 已集中到 `compatibility_registry.rs`，131 read / 95 write / 226 total 的 replay 语义不变；
- Cargo feature `legacy-compatibility-registry` 已默认关闭；默认非 test 构建不编译 raw registry composition，显式 compatibility 请求稳定 fail closed；
- `forgecad-mcp-compat` 现在是 feature-gated 的显式兼容二进制；fresh dep-info 证明默认二进制只编译 6 个 MCP 源文件，兼容二进制编译 39 个，旧 handlers/raw registry 不再进入默认编译图。
- 物理隔离不等于参数合同闭合：默认 125 个 operation 只有 12 个 MCP request Schema 为 closed，113 个仍由 Runtime parser fail closed；这应在扩大 Surface 前修复。

### 3.2 Runtime 五域

`runtime_operation_router.rs` 现在从 checked-in Knife profile 生成 125 条 operation→façade route，再从
`forgecad-contracts::weaponry_domain_map` 取得唯一 façade→domain 与 execution-target 归属。Contract
capability map 补充 4 条尚未公开进 profile 的能力级 route，Runtime 总目录为 128 条；未知 operation、
cross-domain envelope 与 MCP-local target 均在 handler/Store/CAS 之前 fail closed。

`authoring_service.rs` 是第一个物理服务切片：AuthoringMesh transaction、AuthoringMesh V2 durable、
Knife Curve ModifierGraph、Knife Curve EvaluatedMesh、foundation import/materialization 直接调用各自 typed
 Runtime 方法，其余 Authoring operation 暂时受控转发到旧 `dispatch_ipc`。Surface 的 15 个 active operation
（8 read / 7 write）现在由 direct typed `surface_service` 执行；只有两个
`production_weapon_retopology_cage_source_bundle_*` 是 compatibility-only alias。Evaluation 的 41 个 active
operation（30 read / 11 write）现由 `evaluation_service` direct typed 承接；Presentation 的 `presentation_service`
与 Delivery 的 `delivery_service` 也已直达 typed Router，均保留 compatibility bridge。因此五域 service 已接线，
但不能宣称五域源码与 Store records 已物理抽完。

`runtime_status` 是唯一显式 MCP-local control-plane projection。它仍需通过 `weapon_preflight` 的闭合 route，
但中央 Contract 将 execution target 标成 `McpAdapter`；Runtime 收到该 typed envelope 会返回
`RUNTIME_OPERATION_TARGET_MISMATCH`，不会伪造 supervisor/transport 状态。

| Domain | 唯一 façade owner |
|---|---|
| Authoring | `weapon_preflight`、`reference_intake`、`authoring_transaction`、`recovery` |
| Evaluation | `observe`、`quality_review`、`job` |
| Surface | `surface_pipeline`（15 active operation；direct typed `surface_service`） |
| Presentation | `fps_presentation` |
| Delivery | `delivery`、`approval` |

这五个域是**并列路由边界**，不是 Authoring→Evaluation→Surface→Presentation→Delivery 的强制串行调用链。工作流可以按 Gate 形成阶段顺序，但源码依赖不能因此被画成层层调用。

### 3.2.1 Surface operation inventory 与当前映射

下表是 Knife profile 的 exact Surface allowlist，而不是 `surface_service.rs` 的 compatibility set。前者为
15 个 active operation（8 read / 7 write）；后者另外保留两个 bundle alias，仅供旧调用者 replay。

| Contract operation（MCP façade） | Contract mapping | Runtime typed function | Store record / repository | 当前边界 |
|---|---|---|---|---|
| `appearance_prepare`（`surface_pipeline`） | operation-level map 未单列 | `Runtime::prepare_appearance_candidate` | Candidate/appearance artifact、RenderSet、Quality records（Store root） | direct typed service；不是 formal bake |
| `appearance_source_lineage_prepare/get`（`surface_pipeline`） | operation-level map 未单列 | `Runtime::appearance_source_lineage_prepare/get` | `AppearanceSourceLineageLinkRecord`（Store root） | direct typed service；lineage sidecar |
| `hero_uv_durable_prepare/get`（`surface_pipeline`） | `HeroUvDurableLink@1`（`Partial`） | `Runtime::hero_uv_durable_prepare/get` | `HeroUvDurableRecord`（Store root） | direct typed service；repository 尚未吸收 |
| `low_quad_draft_durable_prepare/get`（`surface_pipeline`） | `LowQuadDraftDurableLink@1`（`Partial`） | `Runtime::low_quad_draft_durable_prepare/get` | `LowQuadDraftDurableRecord`（Store root） | direct typed service；repository 尚未吸收 |
| `production_weapon_form_quality_v2_preflight_get`（`surface_pipeline`） | operation-level map 未单列 | `Runtime::production_weapon_form_quality_v2_preflight_get` | read-only Stage/FormQuality projection（Store root） | query/preflight；不伪造 durable record |
| `production_weapon_formal_high_prepare/get`（`surface_pipeline`） | `ProductionWeaponHighArtifact@1`（`Complete`） | `Runtime::production_weapon_formal_high_prepare/get` | `ProductionWeaponFormalHighCommitBundle` / formal-high tables（Store root） | direct typed service；aggregate 外 |
| `production_weapon_high_low_bake_prepare/get` + `production_weapon_high_low_bake_preflight_get`（`surface_pipeline`） | `ProductionWeaponHighLowBakeReceipt@1`（`Partial`，capability=`formal_high_low_cage_bake`） | `Runtime::production_weapon_high_low_bake_prepare/get`、`Runtime::production_weapon_high_low_bake_preflight_get` | borrowed `Store::surface_repository()` → `ProductionWeaponHighLowBakeCommitBundle`；preflight 是 source projection | 首个 Surface aggregate seam；新 bake materialization producer unavailable，prepare 仅 exact replay/readback |
| `production_weapon_retopology_cage_source_prepare/get`（`surface_pipeline`） | operation-level map 未单列 | `Runtime::production_weapon_retopology_cage_source_prepare/get` | `ProductionWeaponRetopologyCageSourceBundle@1`（Store root） | direct typed service；source-only Low/Cage bundle |

Compatibility-only（不计入 15 active）：`production_weapon_retopology_cage_source_bundle_prepare/get`，仍落到同名
Runtime typed methods，不能成为新的 Contract façade 或新 Store truth。Exact source anchors：profile
`packages/forgecad-contracts/profiles/weaponry-knife-p0.json:122-162`；service dispatch
`apps/desktop/src-tauri/crates/forgecad-runtime/src/surface_service.rs:48-105`；中央 formal mapping
`apps/desktop/src-tauri/crates/forgecad-contracts/src/weaponry_domain_map.rs:374-386`；borrowed repository
`apps/desktop/src-tauri/crates/forgecad-store/src/surface_repository.rs:86-152`。

### 3.3 Store 五域

`repository_boundaries.rs` 登记同名五域的 record family、table family、migration owner、CAS root policy 和 extraction status。迁移仍只有：

`Store::migrate → migrations-runtime-v1/0001_runtime.sql`

这避免五个 repository 各自建立迁移真值。`authoring_repository.rs` 已增加第一个只借用 `&Store` 的
物理 repository，并被 Runtime 的 AuthoringMesh transaction、Knife Curve ModifierGraph、Knife Curve
EvaluatedMesh 三个 durable family 真正调用；它不新建连接、CAS 或 migration owner。Evaluation 的
`EvaluationRepository<'store>/JobRepository<'store>` 同样只借用 Store，并承接 Job/Event/Checkpoint aggregate；
它不拥有连接、migration sequence 或独立 CAS root。`PresentationRepository<'store>` 与
`DeliveryRepository<'store>` 已各有 direct typed seam，其中 Delivery 先承接 `GameAssetDeliveryLinkRecord`；
ReadModel、QualityEvidence、ApprovalLifecycle、game weapon socket/anchor、其余 Authoring/Surface family 与其余
Presentation records 仍在 Store root，因此 79,841 行集中实现尚未完成迁移。

尚未抽取的 Authoring family 明确为 ProjectionIndex、AuthoringMeshDurable、IdentityLineage、NativeHigh、
FoundationMaterialization、WeaponFoundationImport、AgenticSession、Checkpoint、ActionRun。

### 3.4 MCP 五域 Router

默认 Knife `tools/call` 现在先通过 profile 的闭合 façade/operation validator，再由 Contract 中央映射解析
domain 与 execution target。Runtime-owned operation 通过
`Runtime::invoke_weaponry_operation(domain, operation, payload)` 或 authenticated local IPC 的
`weaponry_domain_operation {domain, operation, payload}` 进入 Runtime；compatibility profile 继续走独立旧
dispatch，只有显式 feature/test 才编译真实 226-tool registry。五域均由 typed service 直接接收默认路由；MCP 没有复制
operation→domain 表。

## 4. 当前映射事实与缺口

| 公共 façade | Runtime 域 | Store 边界 | 持久化语义 | 当前状态 |
|---|---|---|---|---|
| `weapon_preflight` | Authoring | Authoring query | 无新增 record | 映射成立；不应伪造 record |
| `reference_intake` | Authoring | Authoring repository | durable reference/CAS | 已有实现，待物理归档 |
| `observe` | Evaluation | Evaluation projection | 多记录只读投影 | 缺独立 ReadModel repository |
| `authoring_transaction` | Authoring | Authoring repository | 单事务 journal + revisions + CAS | transaction 主链已闭合；同 façade 仍聚合其他 authoring route |
| `surface_pipeline` | Surface | 借用 `&Store` 的 `SurfaceRepository` | High/Low/UV/Cage/Bake/PBR 多记录聚合 | formal `high_low_cage_bake` 中央映射为 `Partial`；仅完成首个 aggregate seam，materialization producer unavailable |
| `fps_presentation` | Presentation | Presentation repository | package/camera/socket/clip records | 历史 FPS family 仍混在 active 根 |
| `quality_review` | Evaluation | Evaluation repository | quality/evidence 多记录聚合 | 缺统一 QualityEvidence owner |
| `delivery` | Delivery | Delivery repository | export/LOD/socket/delivery records | 11 个 Delivery active operation 已 direct typed；新 `GameAssetDeliveryLinkRecord` repository；socket/anchor 与其余 records 仍集中 |
| `approval` | Delivery | Delivery repository | immutable version/approval/export transaction | direct typed service 已接线；ApprovalLifecycle 仍未抽取 |
| `recovery` | Authoring | Authoring repository | session/checkpoint/restore/repair records | 需独立 RecoveryRepository |
| `job` | Evaluation | Evaluation repository | job/event/checkpoint records | 需独立 JobRepository |

因此“所有 11 项已经严格一一对应 Store record”目前是错误陈述。已经稳定的是 façade→domain 的唯一归属，以及部分能力级 Contract/Runtime/Store/MCP 映射；Store 文件内列出的 gap 才是下一批迁移事实。

### 4.1 Delivery current-source slice（WPN-ARCH-DELIVERY-001）

Delivery 当前 active profile 精确为 **11 个 operation = 4 read / 7 write**，由 `delivery_service` 通过
`RuntimeOperationRouter → delivery_service::invoke` 直达；compatibility bridge 仍为
`Runtime::dispatch_ipc → delivery_service::invoke`，service 不回入旧 dispatch。中央 Contract mapping 覆盖 9 个
capability，但全部为 `Partial`；只有 `version_diff` request schema closed，Delivery closure=`1/11`，其余 10 个
由 Runtime parser fail closed。Store 的 `DeliveryRepository<'store>` 目前只承接
`GameAssetDeliveryLinkRecord`（record/get/list/commit），不拥有 SQLite connection、migration sequence 或独立
CAS root。`ApprovalLifecycle`、game weapon socket/anchor、ReadModel/QualityEvidence 与其余 Presentation repository
仍是明确 gap。

最终 architecture-fast same-cohort 为 `641a87b74c6ac1f28c5db25efadb52125f04624ee36ce1600f08ffdb43ccfbad`，
`82 passed / 0 failed / 0 ignored`，190s，`source_drift=false`；本轮未重跑 Runtime full qualification，完整
`554 passed / 0 failed / 37 ignored` 保持为前一完整基线。该结果只证明架构 source/fast regression，不提升
High→Low→UV→Bake、视觉、引擎、人审或商业质量。

## 5. 后续物理迁移原子

严格按依赖顺序执行，每个原子必须保持 compatibility replay、Runtime 单写者、CAS reachability 与旧 receipt 不变：

1. `WPN-ARCH-COMPAT-FEATURE-001`（第一阶段已落地）：compatibility registry 已改为默认关闭的 Cargo feature，未启用时显式 compatibility profile fail closed；后续仍需迁出旧 handler/Runtime adapter；
2. `WPN-ARCH-CONTRACT-MAP-001`（已落地）：五域 façade mapping、持久语义和 MCP-local execution target 已移到 `forgecad-contracts` 单一编译期权威源；
3. `WPN-ARCH-MCP-ROUTER-001`（本轮 source complete）：默认 Knife 调用已经走中央映射和 typed domain envelope；compatibility 仍独立；
4. `WPN-ARCH-RUNTIME-AUTHORING-001A`（本轮 first slice）：Curve、ModifierGraph、EvaluatedMesh、AuthoringMesh transaction 等 direct typed method 已进入 `authoring_service`；其余 Authoring 和其他四域仍转发旧 dispatch；
5. `WPN-ARCH-STORE-AUTHORING-001A`（本轮 first slice）：AuthoringRepository 已承接 transaction/CurveGraph/EvaluatedMesh 三个 family；Recovery 与其余 Authoring family 未抽取；
6. `WPN-ARCH-BASELINE-002`（本轮）：用 fresh target 同时构建并校验 Runtime、Geometry、High、Render 四个身份，清理 Runtime full 基线失败；
7. `WPN-ARCH-BASELINE-FAST-003`（已落地）：fresh 四身份快速门覆盖中央 mapping、Repository boundary、Runtime service/router、timeout、Authoring transaction、Curve graph/evaluated mesh 与 MCP default/compat router；高成本 qualification 继续由完整门负责；
8. `WPN-ARCH-COMPAT-001`（source complete）：legacy handlers/raw registry 已进入显式兼容二进制；默认 Knife 编译图为 6 源、compat 为 39 源，`main.rs` 下降 19,427 行；`agentic_write_tools.rs` 仍为 22,800 行，只在兼容面继续待拆；
9. `WPN-ARCH-MCP-SCHEMA-002`（横切债务，非当前下一原子）：从 active Contract 生成 125 个 operation 的 closed request Schema，不重新编译 legacy handlers；对 optional/union 字段保留真实表达能力，未知字段在 MCP 层 fail closed；
10. `WPN-ARCH-SURFACE-001`（本轮 source slice）：15 个 active Surface operation 已走 direct typed service；formal
    High/Low/Cage/Bake 只形成中央 `Partial` mapping 与借用 `&Store` 的首个 `SurfaceRepository` aggregate seam。
    新 bake materialization producer 仍 unavailable；Evaluation/Presentation/Delivery 继续 legacy，SQL/CAS
    root walk 不在本原子内；
11. `WPN-ARCH-EVALUATION-001`（本轮 source slice）：41 个 Evaluation operation 已走 direct typed service；Job/Event/Checkpoint 使用 borrowed repository，ReadModel/QualityEvidence 仍未抽取；
12. `WPN-ARCH-PRESENTATION-001`（本轮 direct typed service 已接线）：只保留刀类 FPS package/rig/clip 公共能力；Presentation repository 仍部分抽取，socket/anchor 等 record family 不得伪称完成；
13. `WPN-ARCH-DELIVERY-001`（本轮 direct typed service 已接线）：Delivery 11 ops 与 `GameAssetDeliveryLinkRecord` repository 已形成 source slice；ApprovalLifecycle、socket/anchor 与其余 Delivery records 仍未抽取；
14. `WPN-ARCH-MCP-SCHEMA-002`（下一原子）：将 125 个 active operation 的 request Schema 从 12/125 闭合到完整字段面，不恢复 legacy handler 默认编译；
15. `WPN-ARCH-MCP-SPLIT-001`：继续拆出 session、manifest、schema validation、compat adapter、result adapter，重点收窄 `agentic_write_tools.rs` 22,800 行兼容内部职责；
16. `WPN-ARCH-RETIRE-001`：只有 replacement、compatibility replay、consumer-zero、recovery receipt 和 dirty-worktree deletion gate 全部通过后，才物理删除历史模块。

## 6. 验收指标

- 默认非 test 构建不编译 compatibility registry；
- 11 façade 与 125 backing operation 均只有一个领域 owner；
- 五域 mapping 只有一个权威定义，跨 crate 漂移会编译或测试失败；
- Runtime/Store/MCP 根文件行数按物理迁移批次持续下降，不能只增加 façade 文件；
- compatibility build 仍能重放 131/95/226 source manifest；
- 历史 evidence 不改写；结构 Gate 不替代视觉、真人、引擎和商业质量 Gate。

## 7. Router 轮次验证与 Baseline 收敛

- Contract 中央映射 focused：4/4 PASS；Runtime Router：5/5 PASS；Store boundary：8/8 PASS；
- Store full：179/179 PASS；MCP 默认 22/22、显式 compatibility 230/230 PASS；
- Authoring transaction、Knife Curve ModifierGraph、Knife Curve EvaluatedMesh focused 均 PASS；
- Router 结束时 Runtime full 的旧基线为 536 passed / 18 failed / 37 ignored。它不是可以继续物理迁移的可信起点。
- 新 `script/test_runtime_workers.sh` 只接受 fresh target，在同一 source cohort 下构建 Runtime、Geometry Worker、High Worker 和 Render Worker，先验证四个 build identity，再运行 Runtime full，并在测试后再次拒绝 source drift。
- 第一次统一 Geometry/Render cohort 的诊断全量为 547 passed / 7 failed / 37 ignored；这证明旧 18 项不是一个原因。剩余 7 项是漏建 High Worker 的 3 条路径，以及 4 条 Authoring topology/identity fixture 的真实合同与拓扑问题。
- timeout 旧断言已对齐 180s Runtime IPC 合同；art-decision fixture 不再重复 PartOutput 且保持真实 sink 状态；composite delta 对普通路径严格绑定完整 PartOutput，对注册 U-topology 只允许指定 roots 变更。
- Authoring proof 已在 Contract/Runtime/Store 统一为 `AuthoringMeshTopologyOperationProof@1`；split fixture 使用不产生退化 fan 三角形的 winding，collapse 在 face-loop 重建时验证 deterministic child edge set。所有修复保留 Worker 面积/拓扑硬门、Runtime 唯一写者和 CAS/hash readback。
- 定向 same-cohort 验证为 Authoring identity 4/4、Authoring topology 1/1、Native High 3/3、Hero UV 6/6、Low Quad 3/3 PASS。最终 cohort `a6f28cdef0528decbab7cee341f2b3ae4c06cd1adb522a86c9de63d416ef9ff9` 的 Runtime full 为 **554 passed / 0 failed / 37 ignored**（591 total），libtest 10022.89s、harness 总耗时 10147.37s；四身份一致，测试前后无 source drift。`WPN-ARCH-BASELINE-002` 因此标记 done。
- 结构性缺口仍然存在：Codex 外层 `tool_timeout_sec=60` 与 Runtime IPC 180s 不一致；多条动画/GLB fixture 在常规 full 中运行多分钟甚至更久；因此需要 `WPN-ARCH-BASELINE-FAST-003`，而不是直接扩大 Surface。
- 本轮不产生视觉、High/Low/UV/Bake、引擎、真人或商业质量晋级。

## 8. Fast baseline 收口

- CLI、Desktop、IDE 的 `tool_timeout_sec` 已统一为 180，与 Runtime IPC request timeout 一致；checker 同时绑定四处源标记。
- `apps/desktop/src-tauri/crates/forgecad-runtime/ignored-tests.json` 将 37 个 ignored 主分类闭合为：21 platform-limited、10 fixture-required、3 historical-compatibility、3 real-coverage-gap；checker 扫描完整 Runtime `src/**/*.rs`。它证明 inventory，不证明执行；current-cohort execution=`0/37`、`NOT_PROVEN`。
- `script/test_runtime_architecture_fast.sh` 仍使用 fresh target，并由 `test_runtime_workers.sh --architecture-fast` 构建和校验 Runtime、Geometry、High、Render 四身份；它不运行 animation/GLB/2K、视觉或历史数据库 qualification。
- 前序 Surface same-cohort=`21911f161f4433ac0f40e3da5d47d0018b14fb8329eafe2792eec650fc29692f` 的 architecture-fast
  为 53 passed / 0 failed / 0 ignored；194s；local receipt SHA-256=`595a42990c22efe716e390e8bbf58b168c13f18fbe7ad6004b0c85507f5e1e2c`，作为不可变前序证据保留。
- 完整 `554/0/37`、10147.37s Runtime 枚举仍由默认 `script/test_runtime_workers.sh` 保留；它仍跳过 37 项，不能写成这些项的 qualification PASS。
- `WPN-ARCH-COMPAT-001` 已使默认构建不再编译 legacy handlers/raw registry，默认 `main.rs` 从 20,508 行降至 1,081 行；`agentic_write_tools.rs` 仍在兼容二进制中保持 22,800 行。
- 当前 Surface slice 的 active request Schema 仍为 12/125（Runtime-validated=113），不是 schema closure 的替代品；MCP
  default `22/22`、compatibility `230/230` 保持 PASS。新 bake materialization producer unavailable，故不产生
  materialized bake、视觉或商业晋级；历史 receipts 不改写。该段是 Surface 前序原子记录，不能由本轮 Evaluation 回执重写。

## 9. WPN-ARCH-EVALUATION-001 current-source gate

本轮 Evaluation 已完成 direct typed Runtime service，但只完成 Job aggregate 的 borrowed repository seam；`ReadModel`
与 `QualityEvidence` 仍是明确的后续缺口。Presentation、Delivery 已分别完成 direct typed service 接线，但各自的
record family 仍仅部分物理抽取，不能把本轮写成五域物理迁移完成。

- Evaluation profile 精确为 **41 个 operation = 30 read / 11 write**，分属 `observe=10`、`quality_review=23`、`job=8`。Runtime 路由为 `RuntimeOperationRouter → evaluation_service::invoke`；compatibility bridge 为 `Runtime::dispatch_ipc → evaluation_service::invoke`，无 active legacy match arm，也无 service→`dispatch_ipc` re-entry。
- command/query ownership 已修正：`authoring_mesh_transaction_prepare` 与 `authoring_mesh_v2_durable_prepare` 属 `authoring_transaction/Authoring`，对应 `get` 属 `observe/Evaluation projection`；MCP 对每个映射同时校验 façade 与 domain，`PASS_NO_RUNTIME_DOMAIN_MISMATCH`。
- Job 使用借用的 `EvaluationRepository<'store>/JobRepository<'store>`，不拥有 connection、migration sequence 或独立 CAS root；Job insert/update SQL 不回到 Store root，单一 migration owner 保持。剩余 aggregates=`ReadModel`、`QualityEvidence`。
- 当前 final 根文件为 Runtime `52,542`、Store `79,841`、MCP `main.rs=1,081` 行；MCP `agentic_write_tools.rs=22,800`、Runtime root modules=92。历史 Surface 记录与 receipt 仍保持不变。
- active request Schema 为 **12/125**（Runtime-validated=113）；Evaluation closed request Schema 为 **1/41**。字段闭合仍是 `WPN-ARCH-MCP-SCHEMA-002` 横切债务，不是本轮领域迁移的替代品。
- final same-cohort architecture-fast gate：cohort=`641a87b74c6ac1f28c5db25efadb52125f04624ee36ce1600f08ffdb43ccfbad`，`82 passed / 0 failed / 0 ignored`，`190s ≤ 900s`，receipt SHA-256=`193bc2b523e2c6b225a775bb786875e52ea7b865fe928062103e0b250c576cb1`，source drift=false；Contract map、Store boundaries、Runtime five-domain router/service、MCP default/feature 均 PASS。Runtime full 本轮未重跑，前一完整 `554/0/37` 基线保留。
- 本轮没有用户数据变更、visual promotion 或 commercial promotion；High→Low→UV→Bake、材质、FPS、引擎、人审和商业质量不变。历史 receipts（含 Surface receipt）不改写。下一原子严格为 `WPN-ARCH-MCP-SCHEMA-002 → WPN-ARCH-MCP-SPLIT-001 → WPN-ARCH-RETIRE-001`。
