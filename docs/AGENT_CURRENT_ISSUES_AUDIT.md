# ForgeCAD 项目收敛、img2threejs 对比与方向审计

版本：2026-08-01
状态：当前产品/架构收敛结论；外部项目事实为当日核验快照，不是 ForgeCAD 已实现能力

## 1. 结论先行

Forge Studio 可以把自己定义成 `img2threejs` 的产品化升级方向，但“终极形态”必须落成可执行架构和分层质量门，而不是无限口号。2026-07-29 的 ADR-0022 将准确目标改为：

> Forge Studio 是 AI-native、轻量、类别开放、可编辑、可审计的通用视觉资产编译器。千问理解图片，DeepSeek 编写受限设计程序，系统按部件选择程序化、形变或本地混合表示；Rust 负责合同、能力路由、编译、版本和证据；输入什么对象就以什么对象为目标，不能用现有模板或第三方远程 Mesh API 冒充成功。

项目一直无法完成，不是因为缺少更多 API、更多三角形或更多文档，而是因为完成定义长期包含两个不同问题：

1. 可验证的软件工程闭环：状态、版本、GLB、恢复、导出、安全；
2. 无上限的创作目标：各种对象、各种风格、任意图片、持续高质量。

前者已完成大半，后者过去没有表示分层、分布、质量和成本边界，因此永远不会自然结束。新路线不是重新限制输入类别，而是**类别同时开放、能力逐层实现、质量逐类报告**：机械硬表面是当前成熟表示，角色/生物/自然物需要本地形变和混合表示；没有能力时明确失败或请求更多视图。

## 2. 为什么长期无法完成

### 2.1 目标函数无界

“各种高质量 3D 模型”同时包含机械硬表面、角色、有机物、布料、毛发、场景、扫描重建、工程 CAD 和动画资产。它们所需的表示、数据、评测和工具完全不同。一个 P0 同时覆盖这些类别，没有可执行的退出条件。

正确边界是：入口不按类别拒绝；每个请求先形成 `SubjectProfile` 和 `RepresentationPlan`。每条表示能力必须有未见 Brief、失败样例、质量门和成本边界，Domain Pack 只增强知识，不再决定准入。

### 2.2 工程正确性被误当成视觉质量

ForgeCAD 的 hash、manifold、triangle budget、PBR 通道、readback、Snapshot、重启和导出证据很强，但它们只证明“工件正确”，不证明“模型好看”。提高到 100k 三角形或 1K PBR 也不会自动产生可信比例、关节层级、装甲嵌合和细节组织。

视觉质量必须单独由冻结参考、固定视图、结构细节覆盖和独立真人评分证明。M108A 与 M108B 的拆分方向正确，不能再合并。

### 2.3 缺失中间一层“生成式设计语言”

当前真实 DeepSeek 可以提交紧凑 `ForgeVisualAuthoringIntent@1`，VP201–VP204 也已经建立 typed DAG、宏/repeat、高层几何和 1+1 编译复用，但正式产品 author 仍主要复用机械 hard-surface substrate。它已经不只是固定参数表，却还不是能覆盖通用对象的统一资产语言。

真正缺失的是：

- 类别开放的 `SubjectProfile` 和显著视觉特征合同；
- 逐部件选择 procedural/deformable/local-hybrid 的 `RepresentationPlan`；
- 统一承载几何、部件、材质、投影和证据的 `UniversalAssetSource`；
- 角色、生物、植物、软体和环境所需的本地形变/曲面表示；
- 细节清单到真实 source/Part/output/zone/texture/view 的完整映射。

这正是 U002→U004 的意义。继续堆固定整机 Recipe 只会扩大目录，不会获得通用自由度。

### 2.4 基础设施投入早于视觉能力验证

项目在单一状态、不可变版本、CAS、迁移、packaging、Provider、恢复和大量 Gate 上投入很深，这些能力有长期价值，但视觉语言还没有先证明“一个高质量结果可以稳定产生”。因此用户看到的是可靠的 Alpha blockout，而不是高质量创作工具。

后续新增基础设施必须直接服务于 U002–U005 的通用入口、表示能力和跨类别质量；如果不能提高用户可见质量、可编辑性、速度或单位经济，应暂缓。

### 2.5 路线切换留下了并行真值

项目先后经历 Weapon/Unity、通用低模 CAD、Rust-first、远程神经 3D、程序化视觉和多模态参考等路线。代码为了兼容保留是合理的，但交接、计划和任务索引曾把历史过程继续当作当前路线，造成多个 `in_progress` 和多个“下一任务”。

活动设计曾有两套状态真值；`FGC-S001`–`FGC-S008` 已用 `ActiveDesignSnapshot` 解决运行时主结构。文档也必须采用同样原则：一个当前入口、一个下一任务、历史只进 Git/ADR/evidence。

四领域识别的 `recognized | ambiguous | unsupported` 合同已经落地；主结构已解决，状态措辞仍需持续同步。

### 2.6 迭代次数不是产品进度

C111 的几十轮细化说明团队很努力，也暴露了底层表达能力不足：如果每个新外观都需要在 Rust fixture 中手工增加输出和绝对布局，迭代主要是在雕一个样板，而不是建设可以复用的生成器。

进度应改用以下指标：

- 未见 Brief 的结构差异覆盖率；
- critical detail 的真实 lineage 覆盖率；
- 同一资产连续修改的保真度；
- 每个新设计需要新增的代码量；
- 自动门通过后的独立真人评分；
- 明确失败率、成本和耗时分布。

## 3. img2threejs 当前事实

2026-07-30 再次核验其公开 README/Architecture：上游当前明确把自身定位为宿主 Agent 驱动的 code-only procedural Three.js Skill，输出 `ObjectSculptSpec` 与 `THREE.Group` factory；它以 detail inventory、逐 pass render/comparison sheet 和 host vision 评审建立质量循环。公开 README 现列出 object/character/hybrid、creature body plans 和 CS2 专用审查路线，但同一 README 仍把角色、环境、游戏导出、rigging 等能力按版本继续演进，并明确单图不能保证隐藏面或精确几何。因此应把它视为“开放对象分析 + 程序化视觉迭代”的优秀参考，而不是已经完成的通用照片级 3D 引擎。[上游 README](https://github.com/img2threejs/img2threejs) 与 [Architecture](https://raw.githubusercontent.com/img2threejs/img2threejs/main/docs/ARCHITECTURE.md) 为本段一手快照。

此前钉死的 `v1.5.0`/commit 级描述已经过期，不能继续作为当前架构判断。2026-07-30 的公开 README 已把 object、character、hybrid、creature body plan 和 CS2 审查路线列入说明，但仍将角色、环境、游戏导出与 rigging 标为持续演进；因此不能从 README 推出任意类别的照片级重建已完成。稳定事实仍是：它由宿主 Agent 提供视觉、编辑与截图，核心交付是可执行的 `ObjectSculptSpec`/`THREE.Group` TypeScript，而非具有版本、readback、确认和导出的桌面资产真值。

上游 `docs/TOKEN_COST.md` 把一个对象估算为约 `80k–180k` token、`5–8` 次 render-review cycle，并说明只是工程估算而非实测保证。该数字揭示其自由度的真实交换：宿主 coding Agent 可写近乎任意 Three.js，表达上限高，但串行时间、代码量、稳定部件身份和产品安全边界没有被压缩。ForgeCAD 应吸收其观察与质量方法，不采用其多轮完整写码成本。

## 4. ForgeCAD 与 img2threejs 的本质对比

| 维度 | img2threejs | ForgeCAD 当前 | 结论 |
| --- | --- | --- | --- |
| 产品形态 | Agent Skill/方法库 | Tauri 桌面产品 Alpha | ForgeCAD 产品闭环更深 |
| 设计真值 | `ObjectSculptSpec` + TypeScript `THREE.Group` factory | Rust-owned `ForgeVisualProgram`、`ShapeProgram`、`AssemblyGraph` | ForgeCAD 更安全、可版本化 |
| 创作自由度 | 宿主 Agent 可直接写较自由的 Three.js 程序 | VP201–VP204 与 E005-R1 已有紧凑程序化源；通用对象和混合表示未实现 | img2threejs 表达上限高；ForgeCAD 下一缺口是 U002–U004 通用表示 |
| 视觉方法 | detail inventory、八阶段构建、对比图、5–8 次常见视觉循环 | E005-R2/R3 已有 1+1 compare/PBR/receipt Core，但真实四模态和跨类别运行仍为 0 | 方法合同已吸收，U005 仍需真实质量证据 |
| 资产交付 | 当前以代码/渲染为主，glTF 在 roadmap | 真实 GLB/PBR/readback/CAS/export | ForgeCAD 交付链明显更完整 |
| 编辑与版本 | 代码修改，缺少产品级资产版本头 | preview→confirm、不可变子版本、undo/redo、Snapshot | ForgeCAD 是关键升级 |
| 分件语义 | component tree/socket，面向 Three.js 场景 | Part/Connector/Material Zone/AssemblyGraph | ForgeCAD 更适合后续编辑和资产管理 |
| 失败恢复 | 有界修复策略 | 取消、幂等、旧 revision、重启、迟到结果屏障 | ForgeCAD 可靠性更高 |
| 安全边界 | 可生成 TypeScript | 禁止任意 JS/Python/shell/URL/path | ForgeCAD 更适合零基础产品 |
| 速度/成本 | 上游估算单对象约 80k–180k token、5–8 次 review cycle | 目标 1 author + 最多 1 visual patch；当前只测过约 3.8–4.8s geometry-only | ForgeCAD 路线更适合产品，但正式 Provider wall-clock 尚未验证 |
| 当前视觉广度 | showcase 横跨机械/角色/专用类别，但不是冻结未见分布 | 机械程序化工程链完成；通用类别和 U005 正式质量为 0 run | ForgeCAD 最大短板仍是通用表示和首版可见质量证据 |

## 5. ForgeCAD 相对 img2threejs 的升级点

ForgeCAD 真正有价值的升级不是“更复杂”，而是把程序化 3D 从一次性生成脚本变成可靠资产系统：

1. **Rust-owned typed IR**：模型只能提交受限意图，不能执行任意代码。
2. **单一资产真值**：程序、装配、材质、GLB、readback、质量和导出通过 exact lineage 绑定。
3. **不可变版本与确认边界**：永久修改统一 preview→confirm→child version。
4. **语义分件**：Part、Connector、Material Zone 和 AssemblyGraph 为后续局部修改提供稳定身份。
5. **本地优先和可恢复**：SQLite/WAL、CAS、幂等、取消、重启和备份是产品能力，不是 demo 脚本。
6. **真实交付工件**：双档 GLB、五通道 PBR、Khronos/readback、导出和资产包已经进入同一链。
7. **证据与安全**：参考来源、观察/推断/未知、程序绑定、版本 hash 和失败零副作用可审计。
8. **一个工作台一个 renderer**：避免卡片式多个 WebGL context 带来的资源和状态分叉。

这些升级使 ForgeCAD 更像“3D 领域的 IDE + compiler + Git object model”，而不是一个 prompt-to-mesh demo。

## 6. ForgeCAD 仍落后的地方

- formal author 直接输出独立 `ForgeVisualGeometryProgram@2`，没有继承 VP202 的参数、宏和 repeat，token 压缩没有进入正式路径；
- 当前 source 缺通用 rigid hierarchy、Assembly/attachment、Surface binding 和 detail motif，首轮质量仍依赖模型输出大量低层节点；
- E005 engineering Gate 不读取 Brief、must-show/must-not-show、参考图或候选渲染，技术合法的普通模型会直接通过而没有视觉 patch；
- patch 请求只有原 task、source 和工程 failed gate，没有 sealed 参考、候选四/八视图或可见误差；patch 操作也不能改 profile、section、旋转、附件、detail motif 和 Surface/PBR；
- VP204 formal review 固定使用 320×320 四视图 `interactive_preview`，Surface/PBR 为空，不能代表高质量 production review；
- 当前 30 task 的图片输入只是 `image_description` 文字，没有真实 sealed 单图、多视图或当前资产局部编辑；
- 当前约 3.8–4.8 秒只证明 geometry-only/sidecar 路径，不含真实 Provider author、视觉比较和 UI wall-clock；
- 文档和 Gate 数量曾超过有效产品决策数量，必须优先修复上述三项 P0，而不是继续堆批处理基础设施。

## 7. 未来趋势判断

方向符合趋势，但必须采用结构化混合路线，而不是把“纯 primitive 拼装”或“黑盒神经 3D”当成产品真值。

外部信号很一致：

- [Microsoft TRELLIS.2](https://github.com/microsoft/TRELLIS.2) 正在把结构化 3D latent、PBR 和网格后处理放入同一生成链；
- [Tencent Hunyuan3D-2.1](https://github.com/Tencent-Hunyuan/Hunyuan3D-2.1) 证明 PBR shape+texture 的质量持续提高，但显存和依赖不适合 ForgeCAD 的默认轻量安装；
- [PartCrafter](https://github.com/wgsxm/PartCrafter) 强调语义部件级生成，说明“可分解、可编辑”正在成为比单一封闭网格更重要的方向；
- [Alliance for OpenUSD](https://aousd.org/) 推动跨工具场景、材料和几何互操作，说明标准化资产真值与 provenance 会越来越重要。

由此推断，未来有竞争力的产品不会只输出一个黑盒 mesh，而是：视觉模型提供证据和审美反馈，设计模型生成结构化受限程序，本地表示提供部件和编辑语义，确定性编译器提供版本、互操作、性能和可信交付。ForgeCAD 选择 DeepSeek+千问而非第三方 Mesh API，长期方向与这个趋势一致，但本地有机表示的研发压力更高。

## 8. 推荐的“单产品真值、双 AI 职责、本地编译”

```text
用户 Brief / sealed 参考
        ├─ 千问：observed/inferred/unknown 视觉证据与比较
        └─ DeepSeek：typed authoring / 最多一次 patch
                                ↓
                 ForgeVisualProgram / AssemblyGraph
                                ↓
       Local procedural / deformable / Appearance Compiler
                                ↓
              ShapeProgram / Surface Program / GLB readback
                                ↓
             Snapshot / ChangeSet / immutable version / export
```

DeepSeek/千问输出只能作为受检程序或证据，必须经过分件、lowering、readback 和版本链；不能直接成为项目 head。默认安装不包含本地大权重，也没有第三方远程 Mesh 服务入口。

## 9. 现在应该怎样完成

### 阶段 A：U002 建立诚实的类别开放入口

实现 `SubjectProfile@1`、`VisualFeatureContract@1`、`RepresentationPlan@1` 和统一 sealed multimodal author request。纯文本、真实单图、多视图和已有资产必须走同一入口；删除未知对象到 C111、机械臂或未来武器的静默回退。当前没有表示能力时返回 typed limitation，不创建假资产。

### 阶段 B：U003 建立通用资产源与外观编译

用 `UniversalAssetSource@1` 统一部件、轮廓、结构、材质、相机假设、投影和 detail claim，并把每个重要视觉特征追到输入证据、编译结果、GLB/PBR readback 和验收视图。高质量不能只靠贴图或英雄角度掩盖错误几何。

### 阶段 C：U004 建设本地多表示但保持单一真值

现有程序化 hard-surface 继续作为低成本分支；受限 parametric/deformable 和 local-hybrid 只有在格式、拓扑、Part/Zone、CAS、版本和 readback 全部成立时才接入。千问负责视觉证据，DeepSeek 负责编写受限源；两者关闭时既有资产仍可打开和导出。

### 阶段 D：U005 做跨类别正式验收

冻结机械、角色、生物、植物、家具、建筑/环境、载具和混合对象八类任务，覆盖纯文本、真实单图、多视图和已有资产编辑。每类分别报告首轮/最多一次 patch、身份、轮廓、结构、材质、跨视图、可编辑性、时间、成本、失败分类和独立真人评分；不允许总体平均掩盖单类失败。

### 阶段 E：用真实用户和单位经济决定商业范围

在 U005 证据后让 5–10 家付费设计伙伴验证创建→局部修改→导出。产品入口继续类别开放，但商业承诺按已通过的表示能力和质量等级分层；未通过的类别明确 limitation，不恢复模板白名单，也不用无限 Provider 循环补质量。

## 10. 代码与文档清理判定

### 本轮可删除

以下旧远程视觉/网格封装已删除，且 F026 与 Provider policy Gate 要求主工作台不引用它们：

- `VisualGenerationCard.tsx`
- `useVisualGeneration.ts`
- `shared/tauri/visualGeneration.ts`
- 旧远程 Mesh Rust adapters、Tauri commands、credential store、resume probe 和 TypeScript transport。

### 当前不能删除

- `wushen_agent.main`、旧 Weapon/Concept 路由：仍被启动、迁移、release safety/secrets 和兼容测试引用；
- Concept/ModuleGraph 数据模型：仍承担旧库只读与显式转换；
- 旧 remote-job SQLite migration 与数据合同：只用于历史库读取，不得恢复命令、凭据或网络；
- legacy fixture 与迁移脚本：删除会破坏升级和旧库恢复证明。

这些代码只有按“新启动入口稳定→发布 Gate 脱离 legacy→旧库转换演练→删除写路径→保留最小只读 adapter”的顺序完成后才能删除。以文件名或 `rg` 命中数量直接判定废弃会破坏可运行仓库。

### 文档清理规则

- 当前状态只保留在 README、`DOCUMENTATION_STATUS`、`CODEX_HANDOFF` 和任务索引；
- 路线原因保留在 ADR；
- 历史命令和长检查点进入 Git 历史/evidence，不再复制进当前 handoff；
- 旧 Weapon/Concept 兼容事实只从 `COMPATIBILITY_MIGRATION.md` 进入；
- 一个事实只有一份权威文档，其他位置只链接。

## 11. 潜力与风险

潜力是高的，但不是“比所有 text-to-3D 更强”。真正稀缺的市场位置是：比黑盒生成模型更可编辑、可恢复、可审计；比传统 CAD 更适合零基础概念创作；比 Three.js 代码生成更接近可交付资产产品。

最大机会：面向创作者的“图片/描述→可继续编辑的可信 3D 资产”，把千问视觉理解、DeepSeek 设计程序、本地分件/材质/局部修复/版本/导出组合成自主产品。最大风险：视觉质量持续落后于专用黑盒 3D 服务、本地混合表示建设周期过长、每类对象都退化为手写规则，以及用 Gate 数量掩盖用户看得见的质量不足。

建议继续投入的前提是：U002 能取消模板回退，U003 能把外观细节真正绑定到几何/材质/投影，U004 的本地 deformable/local-hybrid 在角色、生物和自然物上提供可测量净收益，U005 能在跨类别未见分布上给出诚实质量、时间和成本。失败时应按表示能力降级或收窄商业承诺，不能恢复输入类别白名单或第三方 Mesh 兜底。

## 12. 2026-07-28 历史产品决策与 2026-07-29 取代关系

以下机械硬表面优先决策保留为历史和当前成熟度排序，但其产品类别范围已被 ADR-0022 取代：

> 先做轻量、外观优先、可编辑、可审计的机械硬表面 3D Agent；用程序化设计源保证结构、可编辑和低边际成本。该历史表述中“可替换视觉/神经 Provider”已由 ADR-0023 取代为 DeepSeek/千问唯一 AI Provider 和本地编译链。

该决策带来五个具体变化：

1. **第一市场切入收窄**：机械臂、机器人、工业设备、未来设备和虚构硬表面道具优先；四领域不再同时作为首个商业版本退出条件；
2. **外观质量可判定**：按 macro/meso/micro/PBR/presentation/usability 六层冻结验收合同，自动事实门与独立真人门分开；
3. **小公司成本可控制**：不训练基础模型、不自建 GPU 集群、不默认分发大权重，联网推理有预算、缓存、一次 author 加最多一次 typed patch 和停止条件；
4. **壁垒从模型转向系统**：垂直 benchmark、typed editable IR、exact-lineage、局部修复、版本/恢复/导出和真实用户工作流共同构成产品价值；
5. **商业验证早于横向扩张**：先用 5–10 家付费设计伙伴证明重复创建→修改→导出，再根据质量、成本和四周留存决定领域晋级。

当前最高产品/架构决定见 [ADR-0022](ADR/0022-universal-reference-conditioned-3d-agent.md)；ADR-0020 的成本纪律和 ADR-0021 的 typed program/1+1 纪律继续有效。U001/U002 已完成类别开放理解与表示规划，U003 已完成当前程序化切片的通用资产源与 exact-lineage，U004 现为唯一 `in_progress` 父任务；VP201–VP204 工程链已完成，C111B/E005 保留为 procedural regression；新表示、真实投影、跨类别正式 run、真人质量和生产发布仍未完成。

## 13. 2026-07-30 上游代码复核与当前代码清理顺序

本轮以 `img2threejs/img2threejs@9a8ecf129a58c1b557a1f03f7727f6295672cd51` 逐文件复核其 README、`SKILL.md`、`docs/ARCHITECTURE.md`、`new_sculpt_spec.py`、`generate_threejs_factory.py` 与 review loop。结论不是“复制一个更自由的生成器”，而是把它的**视觉计划纪律**接入 ForgeCAD 已有的受限资产编译器。

### 上游真正可借鉴与不能借鉴的部分

| 上游部件 | 实际价值 | ForgeCAD 的升级方式 | 不能作出的推断 |
| --- | --- | --- | --- |
| `ObjectSculptSpec`、component tree、`detailInventory` | 先描述对象、部件、轮廓和身份细节，再写几何，避免一开始就落到模板 | `SubjectProfile + VisualFeatureContract + RepresentationPlan` 已把这些拆为 sealed、可验的产品合同；下一步将高显著性 detail 逐项连到 source node、Part/Zone 和验收视图 | 开放文本对象标签不等于每一类都有执行器 |
| 分阶段 build 与 fixed-camera comparison | 把轮廓、结构、材质、微细节的失败定位到可解释阶段 | 当前 U004 用同一 Three.js renderer、GLB/readback hash、八视图 capture 和一次 typed patch 替代多次任意写代码 | 截图对比、VLM 评分或 PBR 通道存在不等于模型达到高质量 |
| `geometry_for()` 与 Three.js factory | 用有限 geometry family 组合，可以快速制作可信硬表面展示资产 | `ForgeVisualGeometryProgram@2` + restricted worker 负责真实 GLB、Part/Zone、readback 与可编辑性，而非交给模型输出任意 TypeScript | 上游的 schema/primitive 列表也存在未实现分支；不能把它当作通用 mesh reconstruction engine |
| character/projection 路线 | 正确承认单图隐藏面，并用相机/投影/置信度约束视觉承诺 | ForgeCAD 保留 `observed/inferred/hidden`、camera hypothesis 和 unobserved texel mask；只有本地 deformable/hybrid 能力完成后才开放执行 | 上游目前以 humanoid template 和路线文档为主，不证明人物、生物或环境已被通用重建 |

### 当前代码的真实分层

```text
React/Tauri Workbench (一个 renderer、输入/证据/预览/确认)
  -> Tauri bounded bridge (不暴露密钥、nonce 或任意路径)
  -> Rust app-server (Turn、Provider、工具、预算、暂停/恢复)
  -> Rust core (Subject/Profile/Plan/UAS/GLB readback/Snapshot/Version/CAS)
  -> capability-gated Python geometry/PBR worker (只接收 sealed input)
  -> GLB + 五通道 PBR + 同源多视图 evidence
```

`apps/desktop/src-tauri/crates/forgecad-app-server/src/product_tools/native_executor.rs`、`forgecad-core/src/repository.rs` 和 Python `geometry_worker.py` 仍是超大边界文件；它们不是“废弃代码”，但已是下一轮可维护性风险。应按职责拆出 universal author/build、candidate PBR evaluation、legacy read adapter、asset persistence 与 worker protocol，保持外部 contract 不变，再各自加 focused Gate。前端的 `CadWorkbenchPanel` 已在拆分中；所有 `useCadWorkbenchPanel*` 模块必须继续只服务一个 renderer 和一个 ActiveDesignSnapshot，不再产生第二个展示/版本状态。

### 清理决策矩阵

| 范围 | 现在的处理 | 删除前的硬条件 |
| --- | --- | --- |
| Fal/Hunyuan/旧 remote-mesh command、凭据、UI transport | 已从产品运行时删除；Provider policy Gate 防回归 | 不恢复；历史 schema/migration 只读即可 |
| `mvp_arm_provider.rs`、C111/E005 author source 与 fixture | 从通用主路径隔离，只作明确机械硬表面回归 | `procedural.generic_hard_surface_v1` 有真实千问比较、packaged E2E、确认/版本/导出与未见分布结果后，迁为 test-only；不得作为未知输入回退 |
| Concept/ModuleGraph、旧 Weapon route、legacy migration | 保留最小只读/显式转换路径 | 新启动、发布、恢复与旧库转换演练都不依赖旧写路径；随后先删除写路径，再保留只读 adapter |
| `neural_visual_generation` 与旧 mesh-seed contracts | 只兼容读取，所有 execution capability 必须 unavailable | 新本地 deformable/hybrid 已有等价 source、readback、迁移与 U005 回归；此时再按迁移计划移除 |
| 文档、历史 evidence | 当前树只保留权威事实，历史留 Git | 不重新引入阶段报告、旧产品手册或把 regression 当能力证据 |

### 质量优先的下一步

1. 完成 U004 的 **multi-zone Appearance Compiler**：将显著性前几项特征绑定到多个 Part/Material Zone，分别形成几何 relief、normal、roughness、albedo/emissive 或显式 unknown；不能再只选择一个 hero zone。
2. 建立 **reference camera + constrained projection/rasterization**：只有相机、遮挡和 texel coverage 均可验证时，才将已观测颜色投到 GLB；未观察面必须保留 mask/不确定性，而非复制正面。
3. 把 **VP204 geometry DSL** 从 fixture 接到 generic hard-surface capability，并按“轮廓、主结构、连接/负空间、材质区、微细节”一次 author；DeepSeek 只能写 typed IR，Rust 只执行当前 worker 已实现的操作。
4. 在用户明确逐次授权后，用同一 renderer 的八视图让千问比较 reference/candidate；只允许一次 typed patch，并由失败分类决定补图、限流或停止。不要重新引入上游式 5–8 次自由写码循环。
5. 冻结未见的科幻装甲、机器人、工业设备与虚构非功能游戏道具基准；先以盲测与真人外观评分证明硬表面路线，再为角色/生物单独引入 deformable/hybrid，而不是用硬表面 DSL 冒充全部类别。

## 14. 2026-08-01 第一阶段工作台与会话专项审查

用户提供的目标图确认了新的优先级：先让中央工作台稳定展示完整、高质量、材质可信的唯一 3D 结果，导出暂时降级。1280×720 实机检查暴露了比视觉皮肤更早的结构问题：首屏无 Turn 即显示失败、右栏内部滚动与底部历史裁切、中央视口受挤压、快速修改重复、品牌 slot 碰撞，以及 7,106 行 CSS 的同选择器覆盖链。`CadWorkbenchPanel` 当前 2,632 行并让 F025 `<2200` 责任门失败；`ModuleGraphViewport` 2,744 行，渲染生命周期仍与大量 overlay 混合。

DeepSeek adapter 本身已经处理 thinking、Tool Calls、`reasoning_content` 和 cache token 回执；更大的缺口在 ForgeCAD 自己的会话构造：最近消息只按数量截断、thread summary 没有形成稳定更新链、动态 snapshot/evidence 会改变前缀、缓存命中缺少可检查 prefix receipt。下一步不是增加聊天按钮，而是建立 Rust-owned stable prefix、结构化 project memory、token-budget compaction 和 `PromptPrefixReceipt@1`。

清理已按“无运行时引用 + 无 schema/migration/fixture 责任 + 有替代 Gate”执行：三方向前端 state/hook 和组件库偏好 state/hook 连同孤立 smoke 已删除；后端 R006、legacy migration、C111/E005 fixture、Snapshot/Export 和 test-only compatibility reader 保留。四 Luna 的文件所有权、插件/Skill、Gate 与合并顺序见 [U004 第一阶段高质量工作台总图](U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md)。
