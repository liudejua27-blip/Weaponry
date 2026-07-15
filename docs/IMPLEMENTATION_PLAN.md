# ForgeCAD 通用机械概念 3D Agent 实施计划

版本：2026-07-12
原则：先完成通用 Core 和四领域最小纵向切片，再扩展资产数量或工程能力

后续 Codex 的具体领取顺序、任务 ID 和交付模板以 [CODEX_EXECUTION_PLAN.md](CODEX_EXECUTION_PLAN.md) 与 [CODEX_TASK_INDEX.md](CODEX_TASK_INDEX.md) 为准；本文件保留产品里程碑和退出条件。

## 1. 产品退出目标

零基础用户只配置大模型 API，即可完成：

```text
描述未来武器道具、汽车、飞机或机械臂创意
→ 获得完整外观方向与 3D blockout
→ 自动分件
→ 逐部件修改、替换、调比例和换材质
→ 检查、渲染和导出可编辑资产
```

默认不安装本地神经模型、ComfyUI、Blender 插件、CUDA 或模型权重。

## 2. 执行硬规则

- Core 不出现领域专属类名；武器、汽车、飞机和机械臂语义进入 Domain Pack。
- 新建设计先生成完整外观，不能只交付孤立部件。
- 不让大模型输出并执行任意 Python、JavaScript 或 shell。
- 不直接覆盖父版本、原始资产或锁定部件。
- 不增加第二个 Three.js renderer。
- 不把 Weapon reference fixture 冒充四领域完成证据。
- 不把视觉材质、概念 Mesh 或 Joint preview 说成工程结论。
- 所有 secret/file-overreach 风险必须有自动门：密钥不入源码/日志/工件，文件访问不越过允许根目录。

## 3. 当前基线

已完成并复用：桌面壳、本地 API、SQLite、对象存储、Project/Version、ModuleGraph、ChangeSet、单视口、组件替换、质量、导出、Job/Event/SSE 和 OpenAI-compatible Planner 边界。

当前可运行证据同时覆盖四领域 Agent slice 与历史 Weapon Concept reference pack：通用合同、四领域最小 registry、ShapeProgram validator、通用概念规划 Provider port、轻量 Geometry Worker、分件候选、AgentAssetVersion、AgentComponent 注册/替换、Connector 声明式对齐、未声明 sibling 的 AABB 重叠预警和受限 GLB 导出/readback 均已实现。`ActiveDesignSnapshot` 的 Agent 路径已覆盖恢复、选择、preview、质量、不可变回退/前进与 GLB 导出，legacy 只读重建已需显式授权；完整并发矩阵、精确网格碰撞/运动学、外部 GLB 自动重建/深度分件、真实 Provider truth set 和 packaged sidecar 仍未完成。

当前启动入口仍是 `wushen_agent.main`。删除 legacy 必须发生在新入口具备同等启动、恢复、质量和导出证据之后。

## 4. 里程碑总览

| 阶段 | 交付 | 退出条件 |
| --- | --- | --- |
| G0 产品合同 | ADR、领域包、材质、工作台和操作文档 | 文档与安全门通过 |
| G1 Agent Kernel | Thread/Turn/Item/Approval、SSE、取消恢复 | 本地确定性 Kernel smoke；真实 Provider 仍待 G4 |
| G2 General Contracts | DomainPack、MechanicalConceptSpec、AssemblyGraph、MaterialPreset | Python/TS/JSON Schema 一致 |
| G3 Shape Runtime | ShapeProgram、validator、轻量 Geometry Worker | 四领域 48 个后端 blockout、GLB 与 topology/readback smoke；前端变体目录和复杂自由曲面仍待扩展 |
| G4 Full-look Loop | Brief → 完整外观 → blockout → 多视图 | Planner 与 blockout API 已连通；R001 已完成 Snapshot 主视口相机/灯光预设，R002/R003 已完成四视图与条件式透明爆炸软件 PNG，R004 已完成与当前预览指纹一致的 PNG/manifest 图包；真实 API truth set 和更高质量渲染仍待 |
| G5 Segmentation | 分件候选、层级、Material Zone、Connector/Joint 候选 | 四领域 12 个候选图通过；真实碰撞/运动学仍待 |
| G6 Asset Editing | 部件修改、替换、材质、版本、组件注册、对齐、概念重叠预警和 GLB 导出 | 资产级纵向切片与 readback 门通过；不是精确碰撞 |
| G6.5 External GLB Reference | 安全导入、内容寻址、同视口显示、原样导出 | 自包含 glTF 2.0、预算和访问器门通过；自动重建/深度分件仍待 |
| G7 Zero-beginner UX | 单输入框、步骤卡、确认条、用途导出 | 新用户可用性通过 |
| G8 Release & Cleanup | 性能、打包、新入口、legacy 退出 | C01–C12 通过 |

## 5. G0：产品合同与文档

### 交付

- ADR-0008：通用机械概念 Agent 与四领域包；
- `DOMAIN_PACKS.md`：四包角色、模板、Connector、Joint 和验收；
- `MATERIAL_SYSTEM.md`：视觉 PBR 材质与工程边界；
- README、DESIGN、工作台、前端、Quickstart 和操作手册同步；
- 历史 Weapon 文档标记为当前兼容 fixture，不重写历史证据。

### 退出条件

- 仓库文档无断链；
- 不再把 Weapon Concept 描述为唯一产品范围；
- 当前实现与目标能力明确分开；
- 安全、密钥和发布文档门通过。

## 6. G1：Agent Kernel

合同：

```text
AgentThread@1
AgentTurn@1
AgentItem@1
ApprovalRequest@1
ProviderUsage@1
```

第一批工具只读：

```text
inspect_project
inspect_selection
inspect_assembly
list_domain_packs
list_material_presets
```

当前已完成：迁移、Thread/Turn/Item/Approval 持久化、幂等 replay、取消、Approval resolve、SSE cursor replay、HTTP 路由和桌面 API client；macOS Tauri Provider 配置入口由 Rust supervisor 负责 Keychain 与子进程注入；显式 Provider check 会区分离线状态与真实调用。当前仍未完成：流式模型 delta、项目上下文工具、ShapeProgram 工具和跨重启运行中的 Turn 恢复；G4 通用 Planner port 已支持 deterministic 与 OpenAI-compatible JSON Schema 调用，但真实 API truth set 仍需用户显式运行评测。

退出条件：状态机、幂等、取消、cursor replay、Provider 超时和重启恢复有自动测试；API Key 不进入 SQLite、Event 或日志。

## 7. G2：通用机械合同

### DomainPackManifest@1

实现领域角色、模板、Connector、Joint、材质集合、质量和导出 Profile registry。

### MechanicalConceptSpec@1

实现领域、完整外观 envelope、比例、姿态、设计语言、材质意图和 generation stage。

### AssemblyGraph@1

实现分层 PartNode、geometry source、pivot、Connector、Joint、Material Zone、editable parameter、lock 和 provenance。

### MaterialPreset@1

实现金属、聚合物、橡胶、复合材料、透明、涂层和自然材料的最小 PBR preset registry。

### 兼容迁移

实现 WeaponConceptSpec/ModuleGraph → 新合同 adapter。旧 ID、hash、Version 和原对象必须保留；不得原地重写数据库历史。

退出条件：JSON Schema、Python、TypeScript 和 OpenAPI 无漂移；unknown field、环、孤儿、非法 Joint 和不存在材质全部拒绝。

当前进度：四个 JSON Schema、四个最小领域 registry manifest、生成的 TypeScript/Python registry 和 `npm run agent:g2-contracts-smoke` 已落地；语义检查已覆盖严格字段、非功能边界和装配图环/孤儿/连接器引用。旧 ModuleGraph adapter 与正式美术资产晋级仍属于兼容/发布阶段，不是通用 Agent 运行时的必要前置。

## 8. G3：ShapeProgram 与几何 Worker

当前已落地 ShapeProgram@1 schema、只校验不执行的 validator，以及一个不依赖 Torch/模型权重的轻量 Geometry Worker。Worker 解释受限 `box`/`cylinder` 操作，输出完整概念 blockout、GLB、AssemblyGraph、bounds 和 topology hash：

`npm run agent:g3-shape-program-smoke` 已覆盖严格字段、有限值、参数范围、有序引用和非执行边界。

```text
profile / extrude / revolve / sweep / loft / shell
primitive / boolean / mirror / array / radial_array
bevel_approx / fillet_approx / surface_panel
pivot / connector / material_zone
```

技术候选：`manifold3d` 执行实体和布尔，`trimesh` 分析并读写 GLB，Three.js 继续只负责显示。

fixture 最低要求：

- 12 个 future weapon prop；
- 12 个 vehicle；
- 12 个 aircraft；
- 12 个 robotic arm；
- 每类至少 3 个完整 blockout，不是孤立零件。

退出条件：相同 seed/program 得到相同 topology hash；导出 GLB 可由受限 readback 校验；非法/超预算输入执行前拒绝；worker 崩溃不带崩桌面或 FastAPI；依赖树没有 torch/tensorflow/CUDA/模型权重。当前已完成四领域 48 个确定性 blockout、ShapeProgram→GLB→readback smoke，以及 G801–G807 的受控操作/多样性门禁（含 bevel_approx/surface_panel）；复杂 profile/loft、前端变体目录仍未完成。外部 GLB 现可安全作为参考导入，但还不能自动转换为 ShapeProgram。

扩展几何操作前必须执行候选 benchmark：现有 Python worker、JSCAD 语义、Manifold Python/WASM、Trimesh 和 glTF 验证/优化只作为候选。采用前记录体积、内存、冷启动、确定性、平台打包和许可证；最终默认安装包只能保留一个权威几何执行组合。详见 [GitHub 参考架构](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md)。

## 9. G4：完整外观 Agent 闭环

当前已落地 `MechanicalConceptPlan@1` 的内存模型与 Planner port：默认使用 deterministic fallback；设置 `FORGECAD_AGENT_PROVIDER=openai_compatible` 后通过本机 secret file 调用 OpenAI-compatible Chat Completions。离线 fake-provider smoke 已覆盖 JSON Schema 请求、三方向响应和未配置拒绝。真实外网仍未纳入 smoke；方向确认后现在可调用轻量 Geometry Worker 生成预览，但还不会写入正式版本。

新增工具：

```text
infer_domain_pack
plan_complete_concept
author_shape_program
validate_shape_program
build_blockout
render_concept_views
```

流程：

```text
Brief
→ 推断领域
→ 3 个完整方向卡
→ 用户选择
→ 完整 blockout
→ front / side / top / perspective renders
→ 用户确认方向
```

真实 Provider 评测每领域至少 20 Brief。指标：领域推断、完整性、结构化输出、首次预览时间、token、费用和失败原因。

退出条件：Brief 结构化成功率 ≥90%；完整外观率 100%；每领域至少 5 个真实 API 候选通过人工方向审阅；确认前正式 Version 副作用为 0。

## 10. G5：自动分件与装配候选

当前已实现 `POST /api/v1/agent/blockouts:segment`。它根据 Domain Pack 的部件角色，为每个 blockout 输出稳定的 `part_id`、父子层级、位置/尺寸、Material Zone 候选和可编辑参数；结果仍标记为 `candidate`，不会覆盖正式 Version。

```bash
npm run agent:g6-segmentation-smoke
```

桌面 Agent 面板会显示候选部件摘要，并提供“保存为可编辑模型”。确认后写入独立 `AgentAssetVersion`；当前 AssemblyGraph 已生成关系级 mount Connector 和机械臂 revolute Joint，Agent ChangeSet 支持关节姿态后代重定位及声明式 `snap_part_to_connector` 对齐。真实碰撞约束、动力学、split/merge 仍待后续。

新增工具：

```text
propose_segmentation
preview_segmentation
split_part
merge_parts
assign_connector
assign_joint
set_pivot
validate_assembly
```

分件必须以领域角色为依据：车辆分车身/座舱/移动件/灯组；飞机分机身/翼面/舱罩/发动机舱；机械臂保持基座到末端工具的运动链；未来武器道具保持主体与视觉模块。

退出条件：每领域 10 个分件样例无环、无孤儿、可逐部件选择；父节点变更后子树按 Connector/Joint 重定位；连接器存在、类型和法线校验；用户可预览并撤销 split/merge。

## 11. G6：可编辑资产、部件与材质

新增工具：

```text
modify_selected_part
replace_part
set_part_parameter
set_part_transform
set_joint_pose
apply_material_preset
save_reusable_component
run_asset_quality
export_project
```

当前已实现：独立 Agent asset version、稳定部件层级、ghost ChangeSet、部件位置/比例/关节姿态修改、视觉材质绑定、`AgentComponent@1` 注册/同角色替换、声明式 Connector 对齐、v2/v3 确认、只读 `AgentAssetQualityReport@1` 和受限 ShapeProgram→GLB 导出/readback。G6.5 已提供外部 GLB 的安全参考导入：只接受自包含 glTF 2.0、验证访问器/预算/哈希后存入内容寻址库，并可在同一视口显示与原样导出；它不会冒充可编辑 ShapeProgram。真实 Connector 碰撞/运动学、pivot 语义和外部 GLB 的自动重建/深度分件仍待完成。

退出条件：局部修改成功率 ≥85%；锁定部件保持率 100%；兼容替换成功率 ≥95%；材质只影响目标 Zone；GLB 层级、材质和 pivot 回读成功率 ≥98%。当前 smoke 已覆盖候选→v1→预览→v2，以及基础参数拒绝；完整退出条件仍未满足。

## 12. G7：零基础用户界面

首屏：项目/保存/撤销/检查/导出，唯一 Agent 会话，唯一 3D 主视图，选中部件简单卡和候选确认条。

不要求用户选择 Domain Pack、Mode、Skill、pipeline、Connector、Joint、GLB 或 PBR。组件和材质仅在用户提出替换/换材质时打开按需抽屉。

固定可用性任务：

1. 用一句话生成完整模型；
2. 选择并缩短一个部件；
3. 更换一个部件；
4. 只给一个材质区换材质；
5. 调整一个关节姿态；
6. 撤销修改；
7. 导出 GLB 和概念图。

每个领域至少 5 名第一次使用者，≥70% 无口头指导完成；首次有效导出中位数 <5 分钟。

当前状态：部分实现但未退出。主工作台已提供 Agent 输入、三方向、blockout、分件候选、受限部件动作、材质、检查、不可变撤销/重做和 GLB 导出；Agent 路径的版本/选择/preview/质量/GLB 导出已接入 Snapshot，核心浏览器 smoke 已通过。仍缺未知领域澄清、完整并发、前端拆分、字号/点击目标验收和原生安装 E2E。当前用户能力以 [USER_GUIDE.md](USER_GUIDE.md) 为准。

## 13. G8：发布与 legacy 退出

### 性能目标

- blockout preview 默认 ≤100k triangles；
- editable asset 默认 ≤250k triangles；
- 一个 WebGL canvas/context；
- Provider 离线不影响已有项目打开、调整和导出；
- Thread、Version、候选和导出通过重启恢复。

### 删除顺序

1. 新 ForgeCAD Agent 入口承载桌面启动；
2. 当前 smoke 和打包检查迁移到新入口；
3. 四领域 E2E 与 Weapon 兼容 adapter 通过；
4. release gate 不再要求旧端点/文档；
5. 建立 baseline tag 和数据迁移说明；
6. 删除旧 image/three_d/Patch/Unity/Weapon runtime 与脚本；
7. 清理旧环境变量、依赖和兼容 UI。

## 14. C01–C12 质量门

| Gate | 内容 | 阻断条件 |
| --- | --- | --- |
| C01 Agent Contracts | Thread/Turn/Item/Approval | 漂移或历史丢失 |
| C02 General Domain | Pack/Spec/Assembly/Material Schema | 领域语义泄漏到 Core |
| C03 Policy | Tool/approval/path/secret | 越权或密钥泄漏 |
| C04 Geometry | Shape fixture/determinism/GLB | 不稳定、空网格、超预算 |
| C05 Full Look | 四领域完整 blockout | 只生成局部或缺主要角色 |
| C06 Segmentation | hierarchy/pivot/connector/joint | 环、孤儿、不可选择 |
| C07 Viewport | 单 canvas/选择/预览/释放 | renderer 重建或状态不同步 |
| C08 Change | preview/confirm/reject/undo | 确认前写正式 Version |
| C09 Material | preset/license/zone/GLB | 伪造来源或错改区域 |
| C10 Quality | mesh/assembly/domain findings | 严重问题漏报 |
| C11 Provider | timeout/cancel/usage/failure | 静默 fallback 或不可取消 |
| C12 Desktop | install/init/4 packs/export/restart | 主链失败或依赖模型环境 |

## 15. 下一批具体任务

执行顺序以生产正确性优先：

1. 完成 [ActiveDesignSnapshot](AUTHORITATIVE_STATE.md) 的完整并发与兼容 UI 退出条件；
2. 修复领域推断，未知或含糊输入进入单问题澄清，澄清前不创建方向或版本；
3. 扩充版本、选择、ChangeSet、质量、撤销/回退和导出源的一致性 E2E；
4. 拆分工作台 E2E，并把 G1–G7 加入 CI；
5. 将 2,611 行 `CadWorkbenchPanel` 拆分为状态机、Agent、视口、选择卡和抽屉模块；
6. 扩展轻量 ShapeProgram/Geometry Worker；优先 profile/extrude/revolve/mirror/array/受限 boolean，不增加本地神经 3D 模型；
7. 增加 Python/Rust 单元测试、依赖审计、性能和可访问性门；
8. 完成真实 Provider 四领域 truth set；
9. 构建非空 packaged sidecar并执行全新机器安装/升级/恢复；
10. 按 [兼容迁移计划](COMPATIBILITY_MIGRATION.md) 退出 legacy 启动链和默认 release gate。

文档重构已先行完成：主 API/操作手册已与 legacy 隔离，用户指南只描述当前能力，测试、发布、恢复和迁移合同已独立维护。

每个后续任务必须满足 [Codex 完成定义](CODEX_DEFINITION_OF_DONE.md)，并在结束时更新 [当前交接](CODEX_HANDOFF.md)。
