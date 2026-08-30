# Weaponry 刀类模块评估与优化顺序 — 2026-08-30

状态：`WPN-ARCH-RUNTIME-STORE-SPLIT-001 / DONE_SOURCE_PHYSICAL_BATCH_001 / OVERALL_PHYSICAL_EXTRACTION_PENDING`

> 2026-08-30 current addendum：Runtime Evaluation reference-comparison/visual-review family 已物理迁出，Runtime root **52,542→51,603**；Store Delivery ApprovalLifecycle 已物理迁出，Store root **79,841→78,865**；compat session/checkpoint/recovery 已物理迁出，`agentic_write_tools.rs` **16,674→16,532**。Runtime root modules 仍为 92，MCP default/compat root 为 996/19,332，fresh dep-info 为 10/44。architecture-fast cohort=`81a58a3d5c07bafbea82b80f3b9ab74f387e06b63380d5c8845199f56d217ee5`，90/0/0 PASS；Store/MCP full=192/192、41/41、237/237 PASS。
> 这只是第一批，不是五域迁移完成。Evaluation 仍有 37 operation 未迁出，Store ReadModel/QualityEvidence、socket/anchor、recovery 等仍集中，compat 聚合仍为 16,532 行。下一原子=`WPN-ARCH-RUNTIME-STORE-SPLIT-002`，RETIRE 继续 blocked。Archify 当前图=`docs/architecture/weaponry-runtime-current.html`；无 High→Low→UV→Bake、视觉、引擎、人审或商业质量晋级。

当前源码元数据：`schema_count=658`、`schema_set_sha256=29784beef684ae4334bfc2983f19fec25694c632ed11e0840bd12b0e9838f0f1`、`runtime_source_sha256=893be325dbd1f057791e3cfed815b7fd2c17517379b09c9ad6df795a9ab6483c`、`compat_mcp_source_sha256=5a5dcd163643eb378736568178e3dca65098a552f2a52fbf0be3907a6bfe0cfd`、`truth_canonical_sha256=8c77ccb9d3829553444fdd04904076cd26ad3037bc929cc464a20c015fcb0172`；source-only compatibility summary `cohort=null`、`131/95/226`、SHA-256=`1eb6cf5125e4d72aa2e8eef0139ff11de8c69b615d47cb66f70b666fb83377ca`。

本评估只描述当前源码结构与本轮架构升级。它不证明 High→Low→UV→Bake、材质、FPS、引擎、真人验收或商业质量。

## 第一性原理结论

当前产品边界已经比代码边界清晰：Codex 默认只看到 11 个刀类 façade，中央 Contract 决定领域归属，Runtime 仍是唯一写者。`WPN-ARCH-MCP-SPLIT-001` 进一步把 active session、active manifest、共享 result adapter 和兼容 Runtime handler 从两个二进制根物理抽出；fresh dep-info 证明默认 MCP 精确编译 10 个本 crate 源文件，兼容二进制编译 43 个，默认编译图不包含 raw compatibility registry/handler。

请求边界已经闭合：125 个 active operation 全部由 MCP 消费 package-owned closed request Schema，Runtime parser fallback 为 0；本轮还补上此前漏执行的 `minProperties/maxProperties` validator 语义。真正的问题转为两项：一是 validator 的复杂组合关键字与预算负向覆盖仍需持续扩充，不能把“有 Schema”误写为“完整实现 JSON Schema 标准”；二是 **Runtime 与 Store 的架构意图尚未完成物理迁移**：五域（Authoring、Evaluation、Surface、Presentation、Delivery）已经取得 direct typed service/router，但两个巨型根仍保存大量跨域实现；ApprovalLifecycle 虽已迁出，ReadModel、QualityEvidence、socket/anchor 以及其余 Presentation/recovery repository 仍未抽尽。MCP 根已缩小，但 compatibility 生产实现仍有 `16,532` 行聚合文件与 `19,332` 行二进制根，因此“完成第一批纵切”不等于仓库整体架构已经完成。

因此现在不能做两件事：

1. 不能把新增 Router/Repository 文件当成物理拆分完成；
2. 不能在共享脏 worktree 中按名字批量删除旧模块，因为仍有 replay、CAS root、migration、Viewer 或 evidence consumer。

## 模块矩阵

| 模块 | 当前规模 | 所有权清晰度 | 物理隔离 | 当前判断 | 下一升级 |
|---|---:|---|---|---|---|
| `forgecad-contracts` | 11 Rust 文件 / 11,787 行；658 schemas | 清晰 | 清晰 | 五域 mapping 是编译期单一真值；active request closure 为 125/125 | 保持 package-owned Schema 单一来源；新增能力必须同步 validator 与负向 fixture |
| `forgecad-core` | 3 Rust 文件 / 3,915 行 | 清晰 | 清晰 | KnifeCurve、ModifierGraph、EvaluatedMesh pure kernel 边界正确；能力宽度仍不足以证明成熟刀类 DCC | 保持无 Store/IPC；补刀类 sweep/loft、拓扑与 modifier evaluator，不把 evidence 逻辑放进 Core |
| `forgecad-runtime` | `lib.rs` 51,603 行、92 个根模块声明 | 部分 | 部分 | 五域 typed Router/service 均已直达；Evaluation 第一 family 已迁出，巨型根仍含 37 个 Evaluation operation 与其他跨域实现 | 下一原子继续按五域物理抽取 Runtime；每批必须使根文件净下降；保留唯一 Runtime 写者 |
| `forgecad-store` | `lib.rs` 78,865 行 | 部分 | 部分 | migration 单一 owner 正确；ApprovalLifecycle 已迁出；socket/anchor、ReadModel/QualityEvidence、其余 Presentation/recovery records 仍集中 | 按 repository gap 抽取，连接、migration 与 CAS owner 不复制 |
| `forgecad-mcp` | 默认 `main.rs` 996 行；`agentic_write_tools.rs` 16,532 行；`compat_main.rs` 19,332 行 | 外部清晰、内部部分 | 默认/兼容及入口职责已隔离 | 默认 11 façade、125/125 请求闭合与五域中央 Router 正确；session/checkpoint/recovery 又完成物理抽出，226 compatibility replay 保持 | 后续按 compatibility 领域继续拆 `agentic_write_tools.rs`，但不得抢占 Runtime/Store 聚合根拆分 |
| `forgecad-worker-protocol` | 1 Rust 文件 / 1,358 行 | 清晰 | 清晰 | typed、bounded、无网络的协议边界合理；新 bake materialization producer 尚 unavailable | 只接受刀类 Surface/Evaluation 所需 additive message；禁止脚本/路径/URL 扩张 |
| Geometry Worker | 20 Rust 文件 / 31,031 行 | 部分 | 清晰 | 隔离与 same-cohort 身份成立，但承担较多历史通用几何路径 | 将刀类 profile/sweep/retopo/bake 请求按 Surface aggregate 收口；历史 operator 进入 qualification/compatibility |
| High Worker | 11 Rust 文件 / 8,117 行 | 部分 | 清晰 | Native High 有结构能力和身份门，但不等于成熟 sculpt | 只补刀类 bounded crease/bevel/detail/sculpt-like operator 与 deterministic readback |
| Render Worker | 7 Rust 文件 / 3,092 行 | 清晰 | 清晰 | 九 AOV/固定渲染隔离合理；视觉与商业质量仍未通过 | 维持 renderer 无状态；把刀类材质、FPS 相机与 review evidence owner 放在 Runtime 域 |
| Viewer | 13 TS/TSX 文件 / 8,446 行 | 清晰 | 清晰 | 只读模型与临时视图状态边界正确 | 后续只消费新的 domain read model；不得成为第二写者或质量判定真值 |
| Skills / profiles | 默认 11 façade / 125 active operation；兼容 226 raw operation | 清晰 | compatibility 已物理隔离 | Codex 顶层动作面已收敛，125 项均有 closed request Schema | 先物理拆分，再做 consumer-zero Skill/archive 退役 |

快速门的默认编译还暴露出约 100 条 Runtime warning 与 56 条 MCP test-build warning。它们不能直接等同于“可删除代码”，因为部分来自 feature/test 构建差异；但它们是下一轮 consumer-zero 审计的高价值入口。禁止用一次性 `allow(dead_code)` 或盲目 `cargo fix` 抹掉信号。

## 本轮已实施升级

- 新增 `runtime:architecture-fast`：fresh target 构建 Runtime、Geometry、High、Render 四身份并校验同一 source cohort；运行 11 组 Contract/Store/Runtime/MCP 架构与刀类 Authoring focused tests。
- 当前 final same-cohort target=`/tmp/weaponry-arch-schema-125-reader1024`、cohort=`265914b6699d101eb69030947c2419e26e7a99ceef52a63a3c834989af88f28c` 的 architecture-fast 为
  `87 passed / 0 failed / 0 ignored`，13 个步骤全部通过，耗时 182 秒，硬预算 900 秒，`source_drift=false`；local receipt SHA-256 为
  `6487663b3aed0a0c80a63ebad7ff6c344f1fd0ccc283f4a52f7ed3e703fc74f8`。本轮没有重跑 full Runtime qualification，完整 `554/0/37` 历史基线继续保留，不被替换。
- CLI、Desktop、IDE 的 ForgeCAD `tool_timeout_sec` 从 60 统一为 180，与 Runtime IPC 合同一致。
- 37 个 ignored 现在由 checked-in policy 逐项闭合：21 platform-limited、10 fixture-required、3 historical-compatibility、3 real-coverage-gap；checker 扫描完整 Runtime 源码树。必须明确：这只是 inventory，current-cohort execution=`0/37`、`NOT_PROVEN`。
- Archify 单屏图将默认刀类调用路径、Runtime 唯一写者、受限 Worker、显式兼容边界与尚未拆分的巨型根放在同一视图中；showcase 9/9、composition 0 error/0 warning，四个桌面视口明暗模式 containment PASS，并已人工检查截图。
- `forgecad-mcp` 增加显式兼容二进制；默认 dep-info 为 7 个源文件、兼容 dep-info 为 40 个，历史 manifest 仍为 131 read / 95 write / 226 total。
- MCP active session、active manifest、共享 result adapter 与 compatibility Runtime handler 已物理抽出；默认 `main.rs` 从 1,081 降至 996 行，compat `main.rs` 从 20,514 降至 19,332 行，`agentic_write_tools.rs` 从 22,800 降至 16,674 行，其 6,102 行测试迁入独立测试模块。
- fresh dep-info 默认精确 10 个 MCP 源文件、compatibility 43 个；默认图不含 compatibility registry/handler。默认与显式 compatibility 完整 MCP 测试分别 `41/41`、`237/237` PASS，历史 manifest 保持 131/95/226。
- Runtime 52,542、Store 79,841 与 Runtime 92 个根模块没有净下降。因此本轮只算 MCP 物理拆分完成，不能把它写成五域持久层/Runtime 物理迁移完成。
- final architecture-fast same-cohort=`bb681a794bc9aa775c939e5c94a3308d30c3d792487e8a1eb98d32daa349bc64`，`88 passed / 0 failed / 0 ignored`、185s、`source_drift=false`；本轮未重跑 full Runtime qualification，历史 `554/0/37` 保持，37 ignored 的本 cohort execution 仍为 0。
- 默认 11 façade summary 现在如实报告 active operation=125、executable/closed request schema=125、schema-blocked=0、Runtime fallback=0、closure=`COMPLETE`。

### 当前 11 façade 请求闭合度

| façade | closed / active | 当前状态 |
|---|---:|---|
| weapon_preflight | 6 / 6 | 闭合 |
| reference_intake | 5 / 5 | 闭合 |
| observe | 10 / 10 | 闭合 |
| authoring_transaction | 15 / 15 | 闭合 |
| surface_pipeline | 15 / 15 | 闭合 |
| fps_presentation | 21 / 21 | 闭合 |
| quality_review | 23 / 23 | 闭合 |
| delivery | 6 / 6 | 闭合 |
| approval | 5 / 5 | 闭合 |
| recovery | 11 / 11 | 闭合 |
| job | 8 / 8 | 闭合 |

125 个 operation 现在都在 Runtime 调用前经过 package-owned Contract 校验；未知 operation、未知根字段和超出 object property count 的请求均 fail closed。仍需继续补组合关键字、循环 `$ref` 与深度/节点预算的系统负向覆盖。

Archify 当前图为 `docs/architecture/weaponry-runtime-current.html`：spec SHA-256=`32cd418b702b461494026bd954035494d81ef3764086a87874cd9761547f8a9a`，HTML SHA-256=`14acd08dfb69c78d2de8825b54c6d6d83b0d6421fa5fd1dc77430d25c21b112a`；9/9 showcase、0 error、0 warning，1440/1600/1920/2048 明暗视口 containment 与人工截图检查均通过。旧 baseline/compact 图保留为前序快照，不作为当前交付图。

## Surface 当前架构切片（WPN-ARCH-SURFACE-001）

`surface_pipeline` 的 current active profile 是 15 个 operation（8 read / 7 write），全部由 Runtime
`surface_service` 直接 typed dispatch；`production_weapon_retopology_cage_source_bundle_prepare/get` 仅为
compatibility alias，不计入 active profile。Contract 中央 capability `formal_high_low_cage_bake` 仍为
`Partial`，其首个物理 Store seam 是借用 `&Store` 的 `SurfaceRepository`；它不复制 SQLite connection、migration
owner 或 CAS root。新 bake materialization producer unavailable，formal prepare 只能 exact replay/readback，不能
产生新的 materialized bake。

五域现在均有 direct typed service，但 Surface repository extraction 仍只覆盖首个 formal
High/Low/Cage/Bake aggregate；不要把该 source slice或五域 Router写成物理抽取完成、视觉质量或商业质量 promotion。active request
schema 当前为 125/125，blocked=0，Runtime fallback=0；Surface façade 自身为 15/15。历史 receipts
不改写，用户数据不触碰。

## Evaluation 当前架构切片（WPN-ARCH-EVALUATION-001）

本节是当前 source/evidence 冻结后的最新状态；上面的 Surface 段是前序原子记录，Surface receipt 保持原样，不能由本轮
Evaluation 回执重写。

- Evaluation active profile 精确为 **41 个 operation = 30 read / 11 write**，其中 `observe=10`、`quality_review=23`、`job=8`；41 个 operation 均已进入 `evaluation_service` 的 direct typed service。Runtime Router 走 `RuntimeOperationRouter → evaluation_service::invoke`，compatibility bridge 仍为 `Runtime::dispatch_ipc → evaluation_service::invoke`，`legacy_active_match_arms_remaining=0` 且 service 不回入 `dispatch_ipc`。
- 本轮修正了 command/query ownership：`authoring_mesh_transaction_prepare` 与 `authoring_mesh_v2_durable_prepare` 属于 `authoring_transaction/Authoring`；对应 `get` 属于 `observe/Evaluation projection`。MCP mapping 同时检查 capability façade 与 domain，状态为 `PASS_NO_RUNTIME_DOMAIN_MISMATCH`。
- Job 使用借用的 `EvaluationRepository<'store>/JobRepository<'store>`；repository 不拥有 SQLite connection、migration sequence 或独立 CAS root，Job SQL 不再在 Store root 重复实现，单一 migration owner 保持不变。剩余 Evaluation aggregates 明确为 `ReadModel`、`QualityEvidence`；Job 当前是 borrowed repository / partial subcontracts，不是所有 Evaluation record family 已搬空。
- 当前 final 根文件为 Runtime `lib.rs=52,542`、Store `lib.rs=79,841`、MCP default `main.rs=1,081` 行；MCP `agentic_write_tools.rs=22,800`、Runtime root modules=92。前序 Surface 快照与其 receipt 仍是不可变历史证据，不在本节重签。
- 当前 active request Schema 为 **125/125**，blocked=0，Runtime fallback=0；Evaluation 相关 façade 已闭合：observe=10/10、quality_review=23/23、job=8/8。这只闭合请求参数边界，不冒充领域物理迁移完成。
- final same-cohort architecture-fast gate：cohort=`641a87b74c6ac1f28c5db25efadb52125f04624ee36ce1600f08ffdb43ccfbad`，`82 passed / 0 failed / 0 ignored`，耗时 `190s`（预算 900s），local receipt SHA-256=`193bc2b523e2c6b225a775bb786875e52ea7b865fe928062103e0b250c576cb1`，source drift=false。Contract domain map、Store boundaries、Runtime five-domain router/service、MCP default/feature focused 均为 PASS；完整 Runtime qualification 本轮未重跑，历史 `554/0/37` 保持。
- 本轮没有用户数据变更，也没有 visual/commercial promotion；High→Low→UV→Bake、材质、FPS、引擎、人审与商业质量状态均不由该 architecture/source gate 改写。历史 receipts 保持不变。

## 五域当前合并快照（2026-08-30，physical batch 001 后）

Authoring、Evaluation、Surface、Presentation、Delivery 已全部完成 direct typed Router/service 接线；这只表示当前请求不再依赖 active legacy match arm，不表示五域代码已物理搬空。第一批物理纵切后的 Runtime/Store/MCP structural snapshot 为 **51,603 / 78,865 / 996** 行，Runtime root modules 为 **92**，MCP `agentic_write_tools.rs` 为 **16,532** 行、compat root 为 **19,332** 行。兼容 registry 仍为 **131 read + 95 write = 226**，已 feature 隔离并继续拆出 session/checkpoint/recovery，但兼容生产代码仍然庞大。

Delivery 当前 active profile 为 **6 operations**，request closure 已达 **6/6**；Approval 为 **5/5**。这只证明 MCP 请求边界闭合，不改变中央 capability 的 `Partial` 成熟度。Store 已迁出 `ApprovalLifecycle`；game weapon `socket/anchor`、`ReadModel/QualityEvidence` 与其余 Presentation/recovery repository 仍未抽尽。

当前 active request schema 为 **125/125**，blocked=0，Runtime fallback=0。`WPN-ARCH-RUNTIME-STORE-SPLIT-001` 只完成 physical batch 001；下一步必须是 `WPN-ARCH-RUNTIME-STORE-SPLIT-002`，在 Runtime/Store 聚合根继续真实物理迁移并闭合退役门后才允许进入 `WPN-ARCH-RETIRE-001`。本轮仅完成架构/source/request-boundary 回归；没有提升 High→Low→UV→Bake、视觉、引擎、人审或商业质量。

## 删除与整理结论

当前可立即删除的 active 产品 family：**没有**。这不是保守，而是当前尚未证明 consumer-zero 和 replay replacement。

优先退役候选只有已被 Projection@2 替代的 fictional-energy/V1 路径；另一条旧 Presentation 测试暴露的是 `MechanicalAnimationGlb@2` consumer 缺口，已改列 real coverage gap，不能伪装成可直接退役。删除前必须同时满足：

`replacement PASS → compatibility replay PASS → active consumer=0 → migration/CAS root/recovery PASS → dirty-worktree deletion gate`

后续架构原子顺序固定为：

`WPN-ARCH-RUNTIME-STORE-SPLIT-002 → WPN-ARCH-RETIRE-001`

当前下一原子不是重做已完成的五域 direct typed 接线，也不是继续拆 MCP 测试，而是继续按五域物理迁出 Runtime/Store 聚合根；随后才是在满足 consumer-zero/replay/recovery 门后的退役。

每个物理迁移原子必须给出根文件净减少、默认编译图减少、Contract→Runtime→Store→MCP 映射和 compatibility replay 四类证据；否则只算新增抽象，不算优化完成。
