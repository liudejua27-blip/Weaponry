# ForgeCAD 收敛执行计划

版本：2026-07-29
状态：当前唯一实施顺序；历史里程碑从 Git/ADR/evidence 查询

## 1. 产品终点

Forge Studio 的产品终点是类别开放的通用参考条件 3D Agent：

> 非专业创作者和小型内容团队上传合法参考并描述目标对象；千问理解图片与验收外观，DeepSeek 编写受限设计程序，Agent 按部件选择程序化、形变或本地混合表示；Rust 校验、编译、版本化和恢复；结果是一个可编辑、可回读、可继续修改的 GLB/PBR，或者一份诚实的能力/视图不足诊断。

机械硬表面是当前最成熟的程序化回归路径，不再是入口类别边界。角色、有机物、布料、毛发和环境等进入通用 `SubjectProfile`，再按可用表示执行；没有合适表示时返回 typed limitation，不得回退到机械臂模板。工程 B-Rep、制造和认证仍不是本产品目标。

本计划以 [ADR-0022](ADR/0022-universal-reference-conditioned-3d-agent.md) 为产品范围最高决策，同时继承 ADR-0020 的小团队成本约束和 ADR-0021 的高自由度/`1 + 1` 路线：不训练基础 3D 模型、不建设常驻 GPU 集群、不要求用户安装 CUDA/大权重，不通过反复调用模型优化同一对象。全部表示只能进入同一 Rust-owned 真值链。

Luna 或其他长程开发模型按 [LUNA_GOAL_EXECUTION_GUIDE](LUNA_GOAL_EXECUTION_GUIDE.md) 执行本计划。开发模型不是产品 Provider，Goal 状态也不能替代任务索引和 Gate。

## 2. 已完成基础

以下阶段只保留为依赖基线，不再并行扩展：

| 历史阶段 | 当前结果 |
| --- | --- |
| S1 ActiveDesignSnapshot | S001–S008 已建立活动版本、选择、质量、预览和导出单一真值 |
| S2 领域澄清 | D001–D003 已实现 `recognized / ambiguous / unsupported` 和单问题澄清 |
| S3 状态机/E2E/CI | T001–T003、F001–F026 已覆盖当前工作台拆分、单 renderer 和核心 E2E |
| G8 轻量几何扩展 | G801–G826、Q003 已实现受限 Profile/Extrude/Revolve/Loft/Sweep/CSG/PBR/readback |
| V1 多视图概念渲染 | R001–R005 已实现当前 Agent GLB 的固定视图和概念图包 |
| R1 sidecar、恢复、安装和发布 | macOS arm64 本机 Alpha 和恢复有证据；跨平台 sidecar/签名/公证仍 blocked |
| Rust-first | K001–K003 已完成产品状态、Provider、SQLite/CAS 所有权迁移 |
| 程序化视觉 MVP 工程链 | PV001–PV005、PV006A/B、PV008 已完成机械臂工程闭环 |

这些完成项不证明视觉质量或任意类别自由生成。

## 3. 当前依赖主链

```text
VP201–VP204 已完成的 procedural typed-program 底座
→ U001 通用产品/文档/任务决策
→ U002 SubjectProfile + VisualFeatureContract + RepresentationPlan
→ U003 UniversalAssetSource + component/detail/material/projection
→ U004A DeepSeek/千问唯一 AI Provider（已完成）
→ U004 procedural + deformable + local hybrid 统一执行/readback
→ U005 跨类别真实未见集 + 1+1 时间/成本 + 独立真人门
→ 付费设计伙伴验证
→ 打包、质量驱动的算子/Provider 扩展和交付优化
```

同一时刻只领取 `CODEX_TASK_INDEX.md` 中一个任务。VP201–VP204、U001、U001A、U002、U003 和 U004A 已完成；U004 当前为唯一 `in_progress`。U002 已建立 Rust-sealed 通用 author request、开放对象理解、视觉验收和逐部件表示规划，并切断未知对象到 C111/机械臂的默认回退；U003 已建立 Rust 派生的统一资产源、外观证据合同及当前程序化结果的 GLB/readback/固定视图 exact-lineage。U004A 已删除 Fal/Hunyuan 远程生成运行时，并用代码 Gate 把 AI Provider 固定为 DeepSeek 与千问。U004 接下来扩展本地 procedural/deformable/local-hybrid 和 Appearance Compiler；兼容 `mesh_seed.generic_v1` 保持 unavailable。E005 保留为 hard-surface regression substrate。C111B 的工程、时间和显示可读性 Gate 保留为回归；其 reference comparison 与真人门仍未通过，不得改写为完成。

## 4. 冻结回归：C111B 黄金视觉资产

目的：保存现有轻量编译链已经证明的工程事实，并避免继续用大量 token 手工修同一资产。C111B 状态为 `superseded`，不是 `done`。

必须同时满足：

- service panel、joint stack、auxiliary linkage、cable clamp、gripper hinge、decal、wear 都有真实 lineage；
- PBR 五通道被 GLB 实际消费，微表面不只是统一噪声；
- production 保持 80k–150k triangle 预算，但 triangle 数不计作结构质量；
- 同一 GLB 的固定八视图通过 reference comparison；
- 唯一 preview、confirm、Snapshot、export、restart exact-lineage；
- 自动门通过后再收集三位独立真人评分。

禁止：横向扩四领域、增加第二 renderer、用 bloom/纹理分辨率/代理评分替代结构与真人门。

2026-07-28 性能与可读性检查点：production A005/SurfaceLayer 五通道 Python 标量烘焙完成有界 NumPy 向量化，同一单次 C111 production compile 从 `103.174s` 降至 `22.504s` 且字节/hash 不变。最新真实 packaged run `c111b_4ad12339eca842b2993cf2d320498416` 工程链与像素可读性门 PASS。冻结合同的 `120s` 目标绑定 `author/lower/compile_readback/render/evaluate/preview` 六阶段 Agent Turn；该 Turn 为 `94.402s`，其中 lower `93.989s`，因此生成性能门 PASS。initial `220.240s`、restart `20.262s`、total `240.502s` 是额外包含 V1 保存、A005 V2 preview→confirm、导出、八视图和新进程恢复的 QA workflow wall time，不得误作生成门。packaged QA 现把 WKWebView 线性受光表面明确转换到显示 sRGB，并由 TypeScript 与 Rust 双侧硬断言 96×96 样本的前景覆盖 ≥100 bps、中位亮度 ≥24、可读前景 ≥5000 bps；初次八视图实测中位亮度 `34–52`、可读前景 `7251–8868 bps`，重启后 PNG hash 与指标逐张一致。减少冷启动/重复编译仍是响应速度优化项，但不再阻断当前 120s 生成 Gate，且不能降低 exact-lineage、PBR、triangle 或单结果门。

2026-07-28 reference 检查点：通用 PV006C 原先用 Core 内固定阈值，未消费 C111B fixture 的 7600/6500/5000。现已升级为 `VisualReferenceComparisonInput@2`，把 Rust-owned `VisualReferenceAcceptancePolicy@1` 与 request/evidence graph/program/GLB/八视图一起 hash-seal；C111 policy 从 frozen v2 fixture 原始字节解析阈值、关键可见性与 source contract SHA，Provider/WebView 不能选择或降低，非 C111 program 继续使用原通用政策。v2 把 0 网络/0 可变成本的生成预算与需显式授权的视觉比较预算拆开；Rust Core 0044 迁移、短期授权、实际 Turn 绑定、调用前原子预留、取消/失败保守结算和 `VisualReferenceComparisonReport@2` 预算证据现已实现。focused Core 账本测试证明三次 ceiling 为 `33334+33333+33333=100000 microusd`，第四次、过期、谱系漂移和跨 Turn 复用均预网络失败；PV006C 聚合 Gate、F026、typecheck/contracts 通过，测试没有联网或费用。当前 Project sealed reference pixels、显式真实调用授权和真实 Provider comparison report仍 `NOT RUN`；新 packaged run 的工程、120s 六阶段生成和显示域像素可读性门均 PASS，但视觉仍明显 schematic，不能据此宣称与授权参考相似，因此阶段 A 尚未退出。

## 5. 阶段 A：VP201–VP203 高自由度设计语言

### VP201 最小 typed program

已实现 `ForgeVisualProgram@2` envelope、typed parameters/nodes、Rust validator、canonical hash、静态预算、source map 和到现有 ShapeProgram 的最小 lowering。该完成不包含 UI、神经 seed 或完整宏系统。

### VP202 组合与静态展开

已实现无副作用纯宏、词法作用域、有界 repeat/array 和 `ExpandedVisualDAG@1`。递归、孤儿宏、作用域捕获、重叠 repeat、动态代码和运行期无界展开均 fail closed。

### VP203 高层几何语言

已将 profile/extrude/revolve/loft/sweep/boolean/array/mirror、supporting box、Part 和 Material Zone 组合进同一 source map。三份非 C111 fixture 形成不同 operation/Part/Zone/GLB 指纹并通过 restricted readback；Surface 完整 binding 仍由既有 A005/SurfaceLayer 路线承担，不在 VP203 擅自扩 Shape operation。

阶段退出指标已通过：新对象只需编写程序数据，不新增整机专用 Rust/Python lowering；重复/悬空引用、轮廓/截面/路径/布尔/阵列和预算/capability 越权均在 worker/网络前拒绝；最终 GLB readback 可追到 v2 source node。该退出不代表 E005 分布和真人视觉质量通过。

## 6. 阶段 B：VP204 低往返编译循环

模型默认只提交一次完整 program；hard gate 后最多一次 typed patch。缓存、增量 lowering/compile 和固定多视图不允许触发整份创意重写。

阶段已退出：Rust-owned authoring session/receipt、canonical node hash、语义依赖失效、完整程序 CAS/cache、bounded operation-fragment cache、sidecar stale-handle 一次恢复与 app-server coordinator 已通过零网络 focused Gate。变化 rotor 已动态证明未变 `op_rotor` 的真实编译片段命中和变化 `op_rotor_bank` miss，证据进入 Rust receipt。Provider v2 Product Tool 路由归后续 A006，不是本阶段退出条件。

目标而非当前端到端能力：无 patch P50 ≤32 秒、P90 ≤70 秒；一次 patch 后 P90 ≤105 秒。当前 VP203 三资产离线 Gate 的 geometry-only P50/P90 已低于前两项阈值，exact replay cache hit 为毫秒级，但明确排除 Provider authoring、真实视觉评估和 UI 交付延迟。继续记录 author/validate/expand/lower/compile/readback/render/evaluate/preview 分段时间、token、Provider 调用、cache hit 和成本。

## 7. 阶段 C：E005 / A006 / Q004 分布验收

当前 `in_progress`：E005 已冻结 30 条任务及 task/source/Provider-authorization/run/structural-matrix/human-review/distribution 七份合同，跑通单条离线 source 的生产 adapter→真实 sidecar→Schema receipt，并以 focused tests 覆盖取消/超时/replay/full+fragment cache/typed patch。结构证据已绑定 patch 后 source 的 canonical semantic graph 与最终 GLB 的材质/序列/平移/轴交换/统一缩放无关几何指纹，聚合器重算 435 对；真人合同重算 3×30/90 份七维中位数。当前两者都只有 not-run fixture/合成合同自测，不是正式结果。

下一子门不得简化为“拿到授权就直接跑”。步骤 1 的 SQLite 原子 reservation/dispatch/settlement/recovery 已完成并以 30 author + 30 patch、token/成本/单次与批次时间 focused Gate 通过；0046/Core batch checkpoint 也已证明单 claim、原子 receipt seal、未触网恢复和触网不确定性 `reconciliation_required`。步骤 2 已完成不可变 prepare-once wire request、真实 body SHA/价格派生、permit-only one dispatch、transport 关联键、usage 上界和逐 reservation formal-receipt evidence 合同；步骤 2a 的生产 VP204 verifier/resume、author+patch usage/phases 合并和 Rust formal receipt writer也已完成。

2026-07-29 的代码红队审查发现正式付费运行前有三个 P0 质量依赖。2b）E005-R1 统一紧凑 author source 已完成。2c）E005-R2 已完成 dynamic + formal-call + recovery/receipt Core：sealed reference bytes + unified source + generic TurntableEight 进入一次联合 visual compare/proposal 调用，Rust 后置派生 report 并密封 typed patch；accept 为一次 build，patch 为两次 build且不发第三次 VLM。不可复制的 multimodal prepared request 已接同一 0045 `Patch` reservation，预网络绑定 authorization/provider/model/pricing，完整 fixture 为 1 Author + 1 visual，OpenAI-compatible adapter 已覆盖 exact body、idempotency header、usage 与 strict proposal schema，且不使用旧 0044 双重预算。0047 检查点使 Author accounted 后可安全重启并只恢复 visual；任何 visual dispatch/未知状态进入 reconciliation，不自动重试。`E005VisualSession@1`/收据绑定真实 R2 phase、usage、TurntableEight、GLB/readback 和视觉决定，不冒充 VP204。SurfacePlan→受限 A005/PBR 与 surface tuning 已进入 R2 candidate；R3 contract slice 也已完成同源单次 `production_concept`/640px/TurntableEight 编译、11 zone/55 map PBR、完整 input hash、production evidence 和 receipt upgrade。当前缺口已收窄为 repository-backed 真实 sealed 单图/多视图/活动资产输入、completed-visual→production 跨重启恢复、完整阶段 wall-clock 和正式 batch/startup；因此立即接 30 题仍只能证明合同。正式顺序继续为：2d）先完成 R2 真实输入产品门，再补完 E005-R3 跨重启 production review/四模态/阶段 wall-clock；2e）完成 batch/startup 接线且保持 dispatch 后零自动 retry；3）用户确认精确 Provider/model/pricing/disclosure 与额度；4）运行 30 条并保留所有失败；5）生成有内容校验的盲评包；6）收集 3×30 真人评分；7）由 distribution validator 聚合。任何前置缺失时保持 `formal_eligible=false`。

DeepSeek 只调用 Rust-owned typed Product Tool：inspect、author、patch、compile、render、evaluate、repair。工具按最小集合进入 registry；每次动作有 revision、预算、幂等、取消和结果校验。

固定编译阶段：

```text
intent/reference
→ silhouette
→ structure
→ form
→ material
→ surface
→ lighting
→ optimization/export
```

`VisualDetailInventory`、`DesignBuildLedger` 和 `VisualConvergenceReport` 必须映射真实 geometry/material/surface/readback。VLM 只能在确定性 hard gate 通过后处理审美判断，不能覆盖失败；ADR-0021 路线最多一次 typed patch。

冻结 30 条未见机械硬表面任务，覆盖纯文字、真实 sealed 单图、真实 sealed 多视图、结构族、复杂度、风格和当前资产局部编辑。现有 `text_plus_image_description` fixture 只保留为文字分布预检，不能冒充 image-to-3D。success@1 统计首轮与一次 patch 后结果；首次真人外观 `≥4/5` 目标为 70%，一次 patch 后为 85%。自动门、视觉 Provider、独立真人证据分别报告。

Showcase 通过不等于产品通过。评测必须冻结且不参与实现：

- 机械臂结构自由度 Brief；
- 纯文字、文字+单图、文字+多视图、活动模型+局部参考；
- 成功或明确失败，不允许默认模板掩盖失败；
- 同一资产连续修改与 source-level patch；
- GLB、视口、导出和重启同 hash；
- 自动 Gate 与独立真人评分分开记录。

PV006C/E005 已有合同继续证明真实授权图片证据、文本 Provider、comparison、最多一次 patch 和 exact-lineage；它们作为 U002–U005 可复用底座，不能单独证明通用类别已支持。

冻结集最终扩展为 30 条机械硬表面任务，并记录首轮/修复后真人评分、端到端时间、Provider 调用、缓存命中、修复次数、失败分类和估算可变成本。现有 PV006 的 20 条门保持不变；增加到 30 条属于后续商业验证层，不得通过改小既有任务阈值制造完成。

## 8. 阶段 D：U004、设计伙伴验证与跨类别质量扩展

U003/U004A 完成后，由 U004 评估本地形变、程序化和 local-hybrid 是否在各类对象上带来可测量净质量收益；U005 通过后，再用 5–10 家付费设计伙伴验证重复创建→修改→导出、四周留存、支持成本和单位经济。

U004 当前排序是质量优先：先解决裁切主体完整性、身份/轮廓保持、中观结构、可见微细节和 PBR，再优化等待时间。时间、费用、内存和失败率必须记录且有安全上限，但在真人视觉门建立前，不以降低面数、跳过参考准备或减少必要质量步骤换取更快结果。

每个质量分层必须建立：

- SubjectProfile 与适用 Representation capability；
- Operator/Generative Pattern、本地形变模板或混合编译策略；
- 角色、比例、部件和材质语法；
- 未见 Brief、越界输入和 stop condition；
- 正式 production kit 与独立真人 benchmark。

类别从入口上同时开放；优化顺序由真实失败分布决定。首个跨类别冻结集必须包含机械/产品、角色/人形、动物/生物、植物/自然物、家具/生活物、建筑/环境、载具和混合对象，角色/生物不再另立产品准入决策。

M108B 保留为多个已晋级领域的正式 production kit 与独立真人跨领域基准，不再阻塞第一机械硬表面商业切片。它仍必须满足原四领域/真人协议，不能因为优先级后移而降低阈值；M109 的跨领域纹理/LOD/设备分级继续等待 M108B。

## 9. 自主 Provider 与本地表示能力

运行时 AI 只允许 DeepSeek 与千问。DeepSeek 负责受限程序 author/patch，千问负责图片证据和候选比较；任何第三方聚合图像/网格 API 均不得作为默认或实验兜底。本地 Rust/Python 几何、确定性 PBR、图像处理和用户导入合法 GLB 继续可用，因为它们不属于 AI Provider。

U004 必须优先补齐：

- 可表达角色、生物、植物和软体比例/姿态的受限 deformable source；
- 将程序化结构、形变曲面、材质区和微细节结合的 local-hybrid compiler；
- 千问 observed/inferred/unknown 证据到 geometry/PBR channel 的可审计映射；
- GLB/readback、Part/Zone、固定多视图、最多一次 typed patch、版本和导出 exact-lineage。

兼容 schema 中的 `mesh_seed` 继续 unavailable。若某类对象缺少受检本地表示，返回 typed limitation；不得恢复远程 Mesh API，也不得以三角形数量、Provider 成功或渲染特效代替视觉相似度和真人门。

## 10. 90 天小团队验证窗口

该窗口用于排序和 Go/No-Go，不是绕过任务依赖的日期承诺：

| 时间窗口 | 唯一主线 | 退出证据 |
| --- | --- | --- |
| 第 1–21 天 | VP201–VP202 语言核 | typed program、validator/hash/source map、纯宏/有界展开和静态预算 |
| 第 22–49 天 | VP203–VP204 高层组合与低往返 | 未见 Brief 产生不同 topology/profile/part/material 指纹；一次 author + 最多一次 patch |
| 第 50–77 天 | E005/A006/Q004/PV006C | 30 条未见任务、真实多模态组合、质量/时间/成本/失败分布 |
| 第 71–90 天 | 5–10 家设计伙伴封闭验证 | 重复创建→修改→导出、付费、四周留存和支持成本 |

依赖未退出时窗口顺延；不得为了追赶日期横向启动领域扩展。

商业验证目标均为 `target`：30 条未见任务；首次真人 `≥4/5` 达 70%，一次 patch 后达 85%；无 patch P50 ≤32 秒/P90 ≤70 秒、一次 patch 后 P90 ≤105 秒；严重回归 `<10%`；可变成本不高于实收收入 25%；至少 5 家付费伙伴且第 4 周核心闭环周留存不少于 50%。

## 11. 发布与 legacy 删除

发布工作不与 VP201–E005 主线抢占当前任务，但现有阻断保持：

- 非空跨平台 sidecar；
- 签名、公证、全新机安装和升级；
- 密钥恢复和备份；
- 广泛多客户端并发 E2E。

legacy 删除必须按 M0–M6 迁移顺序完成。启动、发布 Gate、旧库转换和恢复仍引用的代码不是死代码。

## 12. 每个任务的交付格式

1. 记录任务 ID、依赖和任务前基线；
2. 只修改任务范围内代码/合同/文档；
3. 提供成功、失败、取消、重启/幂等证据；
4. 运行任务 Gate 和基础文档/安全门；
5. 更新任务索引、handoff、能力矩阵；
6. 明确 PASS、FAIL、KNOWN FAIL、NOT RUN；
7. 未满足退出条件时不得写“基本完成”。

触发联网、计费或外部评审的任务还必须记录：操作者授权、Provider/model、最大请求/图片/token、最大 wall time、估算成本上限、cache key、停止条件和脱敏证据路径。默认网络调用数为 0；真实 Provider、视觉 Provider、packaged app、真人评分和付费伙伴必须各自标记 `PASS / FAIL / KNOWN FAIL / NOT RUN`。

## 13. 项目停止继续扩张的条件

发生以下任一情况时先做产品复盘，不新增领域：

- VP201–VP203 仍需为每个对象增加整机专用 lowering；
- VP204 无法把默认创意调用限制为一次 author + 最多一次 patch；
- E005 未见任务通过率、真人质量或方差不可接受；
- 单次生成成本/耗时无法进入桌面产品预算；
- 神经模型质量进步使纯程序化路线长期失去用户价值。
- 冻结分布的可变成本超过实收收入 25%，且模型路由/缓存/计费无法修正；
- 封闭试用没有至少 5 家付费伙伴，或第 4 周核心闭环周留存低于 50%。

此时应在 UI 中按表示能力诚实降级，优先接入可替换的按量 Provider 或收窄商业承诺；不得把对象静默替换成已支持模板，也不得用“通用”掩盖失败类别。
