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
→ U004 procedural + deformable + local hybrid + bounded local mesh patch 统一执行/readback
→ U005 跨类别真实未见集 + 1+1 时间/成本 + 独立真人门
→ 付费设计伙伴验证
→ 打包、质量驱动的算子/Provider 扩展和交付优化
```

同一时刻只领取 `CODEX_TASK_INDEX.md` 中一个任务。VP201–VP204、U001、U001A、U002、U003 和 U004A 已完成；U004 当前为唯一 `in_progress`。U002 已建立 Rust-sealed 通用 author request、开放对象理解、视觉验收和逐部件表示规划，并切断未知对象到 C111/机械臂的默认回退；U003 已建立 Rust 派生的统一资产源、外观证据合同及当前程序化结果的 GLB/readback/固定视图 exact-lineage。U004A 已删除 Fal/Hunyuan 远程生成运行时，并用代码 Gate 把 AI Provider 固定为 DeepSeek 与千问。U004 接下来扩展本地 procedural/deformable/local-hybrid 和 Appearance Compiler；当前 `mesh_seed.local_patch_v1` 只允许对已审查 procedural mesh 做 bounded local patch，通用 `mesh_seed.generic_v1` 及任意导入网格仍保持 unavailable。E005 保留为 hard-surface regression substrate。C111B 的工程、时间和显示可读性 Gate 保留为回归；其 reference comparison 与真人门仍未通过，不得改写为完成。

2026-07-30 U004 P4.1 local hard-surface Hybrid 图片闭环：新增 sealed-image bridge E2E，验证同一 UAS@2 资产由程序化主壳和受限 lattice 饰条组成，并通过 Rust Part/Material Zone/source contract/readback、同 renderer 八视图/五 pass capture、Qwen-compatible authorization、evaluate、preview、confirm、版本化、导出和确认幂等。测试不联网、不调用真实 DeepSeek/千问/Fal；其余任意 mesh、角色/生物/布料、packaged GPU、真实 Provider 质量和 U005 真人门仍未完成。

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

2026-07-30 P2.7 已补齐参考外观回执的跨层闭合：桌面 Rust bridge 从 Worker 原始 GLB readback 提取受限 projection receipt，UAS@2 将 request/program/final GLB/compile-readback/worker receipt 封存到同一 AppearanceEvidenceBundle。缺失回执或任何 source/camera/zone/texture/fusion 漂移都在 candidate 前 fail-closed。该证据只证明参考像素进入被接受的 PBR artifact，不把 lineage 当作视觉相似度。
2026-07-31 P2.12 final GLB pixel truth：Rust bridge 新增独立 GLB parser，沿 `material → baseColorTexture → image → bufferView` 读取最终 artifact 的真实 PNG，重新计算 base-color 与 unobserved mask 的 hash、字节数和尺寸；核心 readback 仅放行 base-color 的 `imported_reference/unknown`，其余四个 PBR 通道仍需 builtin contract。U004 sealed-image fake geometry fixture 同步嵌入真实、尺寸匹配的 base-color/mask PNG，`u004_universal_image_valid_glb_preview_confirm_and_export_round_trip` 通过，避免“Worker 回执声称投影、GLB 仍是底图”的假阳性。该切片不扩大几何类别，也不证明真实千问、照片级相似度、packaged GPU 或跨类别质量。

2026-07-31 U004 P2.13 Rust-owned contour profile fit：参考 sealed image bytes 与同一 GPU auxiliary silhouette tile 各自派生 16 个水平前景占用采样；相机拟合在既有包围盒 IoU/中心误差之外，若两侧 profile 都可用则还必须通过 bounded profile error。profile 只作为低维轮廓证据，不保存 mask/像素、不生成视觉分数；缺失 sealed content 的历史 fixture 兼容包围盒拟合，错误 profile 和显著冲突 fail-closed。该切片只降低离散相机误选并改善后续 UV/PBR 编译输入，真实千问、packaged GPU、未见输入和跨类别视觉质量仍未运行。
2026-07-31 U004 P2.14 Rust-owned reference/candidate visual metrics：对 exact sealed reference 与同一 GPU/PBR capture 计算确定性的 silhouette profile error、foreground bounds IoU、颜色桶重叠、亮度和边缘密度一致性；摘要以 `RustReferenceVisualMetrics@1` transient DTO 和 hash 进入 `VisualReferenceConvergenceEvidence`，可解码的明显偏差合并为收敛 failure code，旧最小 PNG 只报告 `not_available`。通过 app-server metric focused test 与 cargo check；该指标不替代千问语义比较、真人评分或照片级/跨类别质量。
2026-07-30 P2.8 Provider 合同投影：通用 author 工具现在从同一套 checked-in 合同生成完整 Provider-facing typed schema，并把 Rust capability manifest/hash 和 available/unavailable 分支作为只读上下文提供给 DeepSeek。此修复解决 Provider 不知道当前表示能力、误选 unavailable capability 或漏填三份合同的问题；最终 request/profile/feature/plan hash、部件引用、能力可执行性和 limitation 仍由 Rust 校验。Product Tool validator 同步支持公开合同的 64 位小写 SHA-256 模式。通过 product_tools 103 项、app-server 266 项、U004 candidate PBR contract Gate、contracts types generate/check；没有联网或收费 Provider。P2.8 只减少合同返工，不扩大几何能力，也不改变照片级相似度、角色/生物表示或正式跨类别质量结论。

2026-07-31 P2.9 工作台 universal image author transport：将桌面 `author_context` 组装提取为唯一 `buildAgentTurnRequestPayload`，工作台只发送 sealed evidence ID、角色、view hint 和完整视觉图；不允许客户端自报 evidence hash、Project、Turn 或 capability。Universal Turn 不再同时发送旧 `multimodal_context`；旧字段仅供历史调用单独兼容，Rust 协议对双来源请求 fail-closed。`desktop:u002-universal-author-workbench-smoke`、F026 单视口回归、desktop typecheck、protocol 42 tests 和 diff check 通过。该切片关闭图片未进入 universal author Turn 的桌面传输缺口；完整用户像素投影结果、真实 Provider、packaged GPU、照片级质量、未见输入和跨类别质量仍未完成。

2026-07-30 P0.2 通用图片 bridge 闭环：Rust desktop bridge 新增 hermetic `desktop:u004-universal-image-bridge-e2e`，用真实 CAS sealed PNG 和 `pack_unclassified` evidence 生成 UAS@2 generic hard-surface candidate，提交同一 renderer 的八视图及五个 GPU auxiliary pass，绑定一次性 Qwen-compatible authorization 后完成 evaluate、universal preview、compat confirm、版本化和 export；确认重放保持同一 head，并断言没有 legacy `ForgeVisualProgram`/C111 fallback。第二个 focused test 验证 unavailable `mesh_seed.generic_v1` limitation 不调用 worker，也不创建 preview、Snapshot、version 或 export。该 fixture 的 comparison provider 标记 `network_call_made=false`，不调用真实千问、DeepSeek 或付费服务；真实 Qwen 相似度、packaged GPU、照片级质量、未见输入和跨类别质量仍未完成。
2026-07-30 P0.3 通用图片主路径协议闭合：工作台参考图现在只提交 Rust-sealed `author_context`，不再将旧 `multimodal_context` 与 Universal candidate 同时交给比较源；`ValidatedUniversalAuthorContext` 在 SubjectProfile 产生后将旧只读 `VisualEvidenceGraph@1` 投影为绑定 request/profile 的 `VisualEvidenceGraph@2`，同级无证据特征保持 hidden/conflicting。候选完成同一 renderer 八视图后才显示一次千问授权卡，授权前不联网、不创建 preview/version。通过 app-server 3 项 context tests、`cargo check -p forgecad-app-server`、desktop typecheck 和 diff check；真实 Provider、packaged GPU、未见输入、照片级和跨类别质量仍未完成。
2026-07-31 P0.3 GPU render provenance seal：在同一 renderer 八视图/五 pass capture 的基础上，Rust session 现在封存 code-owned visual environment ID/hash 与固定 render manifest；Tauri issue/submit 与 Core submission/evidence 对每张 capture 逐项重验，Universal comparison input 追加 `VisualReferenceRenderContract@1`，把 renderer、环境、manifest、sRGB 和 ACES Filmic 绑定在同一验收 hash 上。修复 legacy adapter fixture 的可选字段初始化后，bridge E2E、真实浏览器 GPU/PBR Playwright、完整 candidate capture Gate、contracts、desktop typecheck 和发布静态 Gate 均通过。仍不能宣称真实千问相似度、packaged GPU、未见输入或跨类别外观质量完成。
2026-07-31 P0.4 native concept-render entry seal：macOS/Tauri 的用户概念图入口现在先检查工作台视口声明的 `forgecad-workbench-pbr@1`、`glb_pbr` ready 和精确 GLB hash，再调用同一 GPU/PBR renderer；旧软件光栅、ShapeProgram fallback 和加载中视口在入口直接拒绝。新增 loader smoke 覆盖 legacy renderer 早拒绝，U004 PBR smoke 与 desktop typecheck 通过。浏览器兼容软件光栅保留为显式诊断路径；仍不能宣称真实千问、packaged GPU、照片级或跨类别外观质量完成。
2026-07-31 P0.5 packaged GPU/PBR evidence contract：将现有真实 macOS packaged C111B Agent WebView QA 收紧为同一工作台 GPU/PBR renderer 的可审计证据，报告并由 Rust 校验 renderer ID、render manifest、visual environment ID/hash、sRGB/ACES、嵌入 PBR texture count；每个固定八视图的 `960×640` 五类 auxiliary PNG 与 beauty PNG 一起进入受限 Rust capture command，由 Rust 读取 IHDR、重新计算 hash、写固定 QA 工件并重验 dimensions/pass IDs/字节/hash，不新建第二 renderer、不把 C111B 当通用产品入口。`desktop:build`、`desktop:tauri-build-app`、logic smoke、Rust cargo check/focused test、U004 PBR smoke、typecheck/contracts 通过；真实 packaged LaunchServices 运行因当前 macOS 锁屏安全前置检查返回 `C111B_PACKAGED_SCREEN_LOCKED`，待解锁后重跑，故 packaged GPU、真实千问、未见输入和跨类别质量仍未退出。

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

2026-07-31 U004 P1.8 bounded appearance color semantics：Appearance Compiler 现在将 sealed 视觉/材质文字映射到六个 Rust-owned `base_color_token`，并贯通 `SurfaceLayerProgram@1`、retained five-channel PBR bake、GLB/readback 与 schema/generated types。该能力只改变 reviewed base material 的受限色彩表达，保持旧 JSON 兼容与未知 token fail-closed；下一步仍应优先完成 packaged loader、真实千问比较、未见输入和 U005 真人视觉门，不得用颜色 token 代替质量验收。

2026-07-31 U004 P1.9 bounded surface finish semantics：Appearance Compiler 现在还将 sealed 视觉/材质文字映射到八个 Rust-owned `surface_finish_token`，贯通 retained five-channel PBR 的 metallic/roughness 输出和 GLB/readback lineage。它补齐了颜色之外的金属、涂层、陶瓷、橡胶、玻璃和发光区差异；仍保持无任意 shader/scalar，下一步继续优先 packaged loader、真实千问比较、未见输入和正式视觉质量门。

2026-07-31 U004 P2.10 相机拟合门控：将参考外观投影从“首次候选默认相机烘焙”改为“首次几何候选→同源 GPU 轮廓拟合→Rust 二次 UV/PBR 编译→新候选重新采集”。任何 unresolved/default camera 都不能进入参考 bake；拟合失败停止在 typed capture failure，不能生成错误投影、Qwen scope、preview 或版本。该切片只关闭相机/GLB/readback lineage 漂移，下一阶段仍是完整 ActionLoop、真实千问、packaged GPU 和视觉质量证据。

2026-07-31 U004 P2.11 两阶段实际桥接与 PBR 材质完整性门：valid sealed-image bridge 通过真实 `AppServerBridge::resume_candidate_pbr_capture` 完成 `capture_required → 重新采集 → authorization_required`，再进入授权/比较；前端同源 capture 现在要求真实完整五通道材质、嵌入纹理数量、色彩空间和采样均合法。该切片把二阶段行为从内部 executor 测试提升到桌面桥接边界，并阻断低质量 PBR 候选进入验收；仍不等同真实千问、packaged GPU、照片级相似度或真人质量。

2026-07-31 U004 P2.15：完成 reference surface facts → Rust Appearance Compiler 的低维绑定切片。新增 `ReferenceSurfaceAppearanceBinding@1` 与 `foreground_dominant_color_buckets`；同 Project/semantic hash 的 sealed image 才能进入绑定，显式 profile/feature 语义优先，事实仅作为 bounded fallback。generic procedural、local lattice、Hybrid、local mesh patch 和 typed patch 共用同一重编译方法，避免第二套资产真值。退出证据：Core UAS focused、app-server U004 14 项、contracts generation/type check、schema drift/negative tests；不退出真实 Qwen、packaged GPU、照片材质恢复、未见输入或 U005 真人门。
2026-07-31 U004 P2.16：收紧 reference fallback 的材质区作用域。参考图低维事实仅可补充外壳、装甲、外部面板等兼容 reviewed base material；内部结构、accent trim、橡胶、玻璃、发光和警示色不会被全局 hint 覆盖，显式 feature/material 语义与 black/gray 词映射优先。退出证据：Core UAS 7 项 focused tests、`git diff --check`；无 Schema 版本变化，不退出真实 Qwen、packaged GPU、照片材质恢复、未见输入或 U005 真人门。
2026-07-31 U004 P2.17：将 reference fallback 从“兼容材质区”再收紧到“兼容材质区 + exact observed feature region”。重编译时 Rust 复用 `ReferenceAppearanceBinding@1` 的同一 evidence、view、feature、Subject Part/Material Zone 绑定；只有命中 observed 且声明 appearance channel 的区域才可消费 `ReferenceSurfaceAppearanceBinding@1`，无证据 sibling zone 保持 reviewed catalog 语义。退出证据：Core UAS 7 项 focused tests、`git diff --check`；无 Schema 版本变化，不退出真实 Qwen、packaged GPU、照片材质恢复、未见输入或 U005 真人门。

2026-08-01 U004 native visual-exterior route closure：新增通用外观 fixture 的真实 native compile/readback 回归。author route 现在由 Rust 选择 `direction_universal_visual_exterior`，通过 UAS@2 和受限 geometry 编译出 GLB/readback；route 在同一阶段明确返回 `CANDIDATE_PBR_CAPTURE_REQUIRED`，不越过工作台 renderer，也不创建 preview/version。capture resume 只广告 open-category visual repair provider，并保持一次 typed patch 上限。app-server 全量 281 passed / 0 failed；仍不退出真实 DeepSeek 桌面端到端、真实千问、packaged GPU、照片级/跨类别质量、未见输入或 U005 真人门。

2026-08-01 U004 category-open eligibility correction：`procedural.generic_visual_exterior_v1` 不再要求 Provider 自报 `visual_exterior` trait；animal/quadruped、植物、建筑等合法开放类别可直接进入 Rust-reviewed exterior proxy，而其身份与未知部分仍由 SubjectProfile/VisualFeatureContract 记录。Core/native focused regressions pass；这只修复 author 准入误拒，不解锁专用 organic/deformable/neural 表示或真实视觉质量。

2026-08-01 U004 runtime prompt correction：主 Rust system prompt 与 capability manifest 已同步。`generic_visual_exterior` 现在明确作为任意对象可见非功能外观代理向在线 Provider 广告，不再把角色、生物、植物、家具、建筑和环境一概要求为 limitation；对象身份仍由 SubjectProfile 保留，deformable/mesh-seed 等未实现表示继续 typed limitation。native runtime policy regression 与 app-server 281/281 通过；真实 DeepSeek→GLB、真实千问、packaged GPU 和跨类别质量仍是未完成 Gate。

2026-07-31 FGC-P002 本机 packaged Alpha 重建：重新生成 macOS arm64 frozen sidecar，完成 P008 结构 readiness、真实 sidecar `/api/health`、首次初始化、受限几何归属、重启恢复和 Rust ownership 静态 Gate；随后真实 LaunchServices Tauri `.app`、K002 和 K003 原生双启动均通过，K003 还验证 Rust-owned K001/K002/K003 状态、GLB/readback/render package 与重启语义 hash。期间修复 K003 smoke 对 supervisor 动态归属字段的过时完全相等断言，保留严格基础字段、字段集合和格式校验。该切片只收紧本机 packaged 证据，不改变 U004 的视觉质量边界；生产发布仍受签名、公证、安装、跨平台 sidecar 和真实视觉证据阻断。
