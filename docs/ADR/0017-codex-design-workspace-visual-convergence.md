# ADR-0017：Codex 式三维设计工作区与视觉收敛编译

- 状态：Accepted（目标设计；分阶段实现）
- 日期：2026-07-25
- 决策者：项目维护者
- 补充：ADR-0014 的 Rust-first 所有权、ADR-0015 的工件/视觉验收拆分、ADR-0016 的 Design Surface Compiler
- 外部方法参考：[`hoainho/img2threejs`](https://github.com/hoainho/img2threejs) commit `c9077d5ecce834f6802d6742b4a5b2c682d6279d`，Apache-2.0；当前只参考方法，不引入依赖或上游运行时

## 1. 决策摘要

ForgeCAD 冻结为一种明确的产品模型：

> ForgeCAD 是面向零基础用户的 Codex 式三维设计 Agent。用户通过自然语言和可选授权参考描述机械视觉资产；AI 在受控设计工作区中编写和修改机械形态、轮廓、结构、材质与表面程序；Rust 负责合同、安全校验、降低、编译编排、版本、真实回读和恢复；系统反复编译、渲染和修复同一个设计，最终交付精致、轻量、可继续修改的 GLB。

高自由度不来自任意网格、任意脚本或大量固定整机模板，而来自：

1. 可表达新拓扑、轮廓、截面和表面布局的受限设计语言；
2. 可读取、增量修改、编译和恢复的 `DesignWorkspace@1`；
3. Operator Recipe、Generative Pattern Recipe、Domain Design Pack 和小型视觉资产库；
4. 参考证据、细节清单、固定构建阶段和真实渲染反馈组成的视觉收敛编译；
5. DeepSeek 负责设计判断和工具编排，Rust 合同决定什么可以真正执行；
6. 一次完整合成、同一意图最多两次原位修复、只展示一个结果。

本 ADR 不改变 ForgeCAD 当前核心路线，不采用 img2threejs 的 TypeScript 运行时或 `THREE.Group` 作为资产真值。

## 2. 为什么需要这项决策

ForgeCAD 已经具备 Rust-owned Agent/产品状态、受限 ShapeProgram、Loft/Sweep/Revolve/CSG、双档 GLB、Material Zone、A005、R007、V003、F026 和版本恢复。当前主要缺口不再是基础生命周期，而是：

- 机械臂黄金资产仍未达到目标参考图的关节层级、装甲嵌合、末端结构和微表面丰富度；
- 现有 C106/C110/C111 的大部分拓扑和操作图仍由代码目录固定；
- DeepSeek 能选择或调整受审路径，但还不能像 Codex 修改源码一样创建完整的新形态程序；
- 表面细节虽然可以编译，但缺少“参考中每一个关键细节必须映射到实际输出”的完整性合同；
- 自动质量门能证明 GLB、PBR、provenance 和版本正确，却不能证明视觉目标已经收敛；
- 继续增加固定整机 Recipe 会提高目录数量，但不会自然获得自由设计能力。

因此下一阶段必须同时解决两件事：

1. 先用 C111A 把一条机械臂黄金路径做到视觉可信，证明目标质量可以由当前轻量管线承载；
2. 再从黄金路径提取可生成的设计语言、工具、Recipe 和视觉收敛方法，使 Agent 能编写新的设计，而不是只选择现有整机。

## 3. 从 img2threejs 采用什么

img2threejs 的高价值部分是视觉收敛方法：

- 参考适用性检查；
- 复杂度和质量合同先于生成；
- `detailInventory` 强制枚举可识别细节；
- blockout、structure、form、material、surface、lighting、optimization 的固定阶段；
- 每一阶段都需要真实渲染证据；
- 确定性几何/图像检查先运行，视觉模型只处理需要判断的剩余问题；
- 多角度检查防止单视角薄片或退化造型；
- 有界修复和重复缺陷/振荡/平台期停止条件；
- 程序化纹理、接缝、磨损、标签和发光细节的实现模式。

这些思想分别映射到 ForgeCAD 已有真值：

| img2threejs 方法 | ForgeCAD 落点 |
| --- | --- |
| ObjectSculptSpec | `DesignWorkspace@1` 中的 typed design sources |
| detailInventory | `VisualDetailInventory@1` |
| component tree / sockets | `MechanicalMorphologyProgram@1` + `AssemblyGraph@1` |
| primitive/procedural geometry | Profile/Section/Recipe 降低后的 `ShapeProgram@1` |
| material/local overrides | Material Zone + `SurfaceAdornmentProgram@1` |
| staged passes | `DesignBuildLedger@1`，只记录同一草稿的派生阶段 |
| browser render | ForgeCAD 单 renderer 固定视图和受控离屏渲染 |
| deterministic visual checks | `VisualConvergenceReport@1` 的 hard gates |
| agent vision review | hard gates 之后的开发/产品视觉评审，不替代 M108B 真人门 |
| correction loop | V003 同一意图最多两次原位修复 |

## 4. 明确不采用什么

不采用以下上游能力作为 ForgeCAD 产品底座：

- 不让 Agent 生成或执行任意 TypeScript/JavaScript；
- 不把 `THREE.Group`、Canvas DOM 或浏览器内存对象作为几何/版本真值；
- 不使用 Three.js factory 取代 Rust `ShapeProgram`、`AssemblyGraph`、Snapshot 或 GLB readback；
- 不把单张参考图推断为已知背面、内部结构、工程尺寸或功能机构；
- 不采用上游长循环替换 V003 的最多两次原位修复；
- 不把自动视觉分数或 Codex/自智能体评分当作 M108B 三位独立真人 `4/5`；
- 不直接复制上游 generator、Skill 或 grimoire 进入安装包。

如果未来复制上游源代码或文字分类，而不只是独立重写其思想，必须锁定 commit、保留 Apache-2.0 NOTICE/归属、更新第三方台账、SBOM、体积/内存/打包 Gate 和删除方案。

## 5. 目标产品工作模型

Codex/Claude Code 与 ForgeCAD 的对应关系冻结为：

| Codex/Claude Code | ForgeCAD |
| --- | --- |
| 用户需求 | 模型描述和可选参考 |
| 源代码仓库 | `DesignWorkspace@1` |
| 编程语言 | 机械视觉设计语言 |
| 源文件 | Morphology/Profile/Section/Shape/Assembly/Surface Program |
| SDK/代码库 | Operator Recipe、Generative Pattern、Domain Pack、视觉标准件 |
| 编译器 | Rust Design Compiler + Restricted Geometry Executor |
| 单元/集成测试 | 合同、几何、拓扑、材质、版本和视觉 hard gates |
| 运行程序 | `interactive_preview` GLB |
| 浏览器截图 | 固定视角、多视角 PBR render bundle |
| 修复编译/测试错误 | 修改同一 DesignWorkspace |
| Git commit | 用户确认后的不可变 AssetVersion/ChangeSet |
| 构建产物 | `production_concept` GLB |

用户不选择 Skill、Recipe、候选、几何语法或专业参数。用户只描述目标、选择可选风格/材质/参考，并确认是否保留结果。

## 6. DesignWorkspace@1

Agent 必须拥有一个可读取、搜索、修改、编译和恢复的内部设计工作区：

```text
DesignWorkspace@1
├── identity
│   ├── workspace_id
│   ├── project_id
│   ├── turn_id
│   ├── base_asset_version_id?
│   ├── revision
│   └── source_hash
├── brief
│   ├── user_intent
│   ├── domain_scope
│   ├── style_intent
│   └── reference_evidence
├── morphology
│   └── main.morphology
├── profiles
│   ├── profile_sketches[]
│   └── section_sets[]
├── geometry
│   └── shape_sources[]
├── assembly
│   └── assembly_source
├── surfaces
│   ├── material_zones[]
│   ├── adornment_programs[]
│   └── texel_policy
├── visual_contract
│   ├── detail_inventory
│   ├── required_views
│   └── quality_thresholds
├── reviews
│   ├── build_ledger
│   └── convergence_reports[]
└── build
    ├── preview_artifact?
    ├── production_artifact?
    └── compile_readback?
```

### 6.1 状态与版本边界

- Workspace 是 Rust-owned 的当前设计草稿，不是第二条 `AgentAssetVersion` 链；
- 每次工具修改提交 `DesignWorkspacePatch@1`，必须包含预期 revision、typed AST 操作、预算和幂等键；
- 修改产生新的 workspace revision 和 source hash，但不推进项目 head；
- 编译工件、截图和比较图都是按 source hash 缓存的派生物；
- 只有 V003 hard gates 通过且用户确认后，Rust 才把密封的 workspace source、ShapeProgram、AssemblyGraph、Surface Program 和 readback 原子写为一个不可变资产版本；
- 取消、失败、迟到结果、旧 revision、项目切换和超预算都不能创建 AssetVersion 或改变 ActiveDesignSnapshot；
- 草稿可以在 Rust SQLite/CAS 中恢复，但必须使用独立 namespace、TTL/显式清理和配额，不能冒充已保存资产。

## 7. MechanicalMorphologyProgram@1

该合同表达形态和结构意图，而不是直接提交网格：

```text
MechanicalMorphologyProgram@1
├── domain_pack_ref
├── proportion_language
├── scaffold_nodes[]
├── scaffold_connections[]
├── topology_families[]
├── negative_spaces[]
├── shell_layers[]
├── motion_visuals[]
├── material_zone_intents[]
├── surface_intents[]
├── budgets
└── provenance
```

它允许 Agent：

- 动态增加、删除或重新连接受限形态节点；
- 选择 serial-chain、parallel-link、open-frame、enclosed-shell 等已审核 topology family；
- 创建负空间、开放骨架、分层装甲和视觉关节层级；
- 为新节点生成 ProfileSketch 和 ProfileSectionSet；
- 给部件声明 Material Zone、表面细节意图和 connector/pivot；
- 在预算内组合 Operator Recipe 和 Generative Pattern。

它不允许 Agent：

- 提交任意 mesh bytes、B-Rep、脚本、URL 或文件路径；
- 声明未注册的几何 operation；
- 绕过领域、安全、连接、面数、纹理和编译预算；
- 把视觉 joint、线缆或开孔描述为真实运动学、布线、结构或制造事实。

Rust lowering 必须输出已有 `ShapeProgram@1`、`AssemblyGraph@1`、Material Zone 和 A005 合同；Python 仍只看到已验证的受限编译请求。

## 8. Recipe 的四层结构

### 8.1 Operator Recipe

最小、可组合的确定性生成能力，例如：

- layered shell；
- rounded housing；
- open rail；
- cable sweep；
- panel split；
- fastening array；
- joint ring stack；
- transition shell。

### 8.2 Generative Pattern Recipe

能产生显著变体的机械视觉模式，例如：

- 多层旋转关节；
- 开放双轨连杆；
- 装甲基座；
- 精密夹爪；
- 维护面板系统；
- 线束与夹具系统。

Pattern 接受受限的形态、Profile、Section、数量和相对比例输入，不是固定整机。

### 8.3 Domain Design Pack

包含领域语义、审美规律、允许的 topology families、修复策略、安全边界和评测集。机械臂通过后再扩展车辆、飞行器、虚构道具和生活机械。

### 8.4 Optional Asset Library

只保存适合复用的小型视觉标准件：螺钉、标签、接口、线夹、按钮和标准纹理。固定整机资产不能成为自由生成主路径。

## 9. VisualDetailInventory@1

视觉细节不再只存在于 prompt、Skill 文本或评审说明中。每个目标设计必须建立可追踪清单：

```text
VisualDetailInventory@1
├── inventory_id
├── reference_set_id?
├── complexity_tier
├── required_counts
│   ├── macro
│   ├── meso
│   └── micro
├── items[]
│   ├── detail_id
│   ├── scale_band
│   ├── kind
│   ├── part_role / evidence_region
│   ├── importance
│   ├── confidence
│   ├── observed | inferred
│   ├── expected_visual_effect
│   ├── maps_to[]
│   │   ├── morphology_node_id?
│   │   ├── shape_operation_id?
│   │   ├── material_zone_id?
│   │   └── adornment_program_id?
│   └── status
│       └── planned | lowered | readback_verified | unresolved
└── unresolved_summary
```

`kind` 至少覆盖：

- 轮廓转折、负空间、装甲嵌合；
- 关节环、轴承盒视觉层级、开放桁架、执行器外观；
- 线缆、线夹、护套和末端工具结构；
- 倒角、接缝、凹槽、凸脊、紧固件、维护盖板；
- decal、警示标识、流线、发光 trim；
- normal、roughness、拉丝、喷涂、磨损、污渍和微网格。

强制规则：

1. 每个 critical 项必须映射到实际 Morphology/Shape/Material Zone/A005 输出；
2. `planned` 文字不计为完成；
3. 编译后必须由 GLB/readback 或 render evidence 证明进入结果；
4. 无法从参考观察的背面或遮挡面必须标为 inferred/unknown；
5. critical unresolved 项阻止唯一结果展示；
6. 对无参考的纯文本创作，Inventory 来自用户 Brief、Domain Pack 和 Skill 的质量合同，仍需映射和回读。

## 10. 固定构建阶段

同一个 workspace 按固定顺序收敛：

```text
intent-and-reference
→ silhouette-blockout
→ structural-pass
→ form-refinement
→ material-pass
→ surface-pass
→ lighting-and-presentation
→ optimization-and-export
```

`DesignBuildLedger@1` 记录每阶段：

- 输入 source hash；
- 允许修改的 typed source 区域；
- 编译 artifact/readback hash；
- required Detail Inventory 覆盖；
- hard-gate 结果；
- unresolved defects；
- 下一动作。

阶段不是用户可见的多个方向，也不创建多个资产版本。它们只是同一草稿的有序内部状态。未来阶段不能在前一阶段未通过时静默解锁。

## 11. 确定性优先的视觉收敛 Gate

`VisualConvergenceReport@1` 的运行顺序固定为：

### 11.1 合同和来源门

- workspace/schema/registry/hash 完整；
- 参考来源、许可、视角和不确定性有效；
- Domain Pack、Skill、Recipe 和运行时 manifest 精确匹配；
- Detail Inventory 达到复杂度最低覆盖且 critical 项都有映射。

### 11.2 编译和 GLB 门

- ShapeProgram/AssemblyGraph/Surface Program 校验通过；
- preview/production 都来自同一 source hash；
- GLB 可回读，bounds、triangle、primitive、material、UV0、normal、tangent 和 provenance 有效；
- Part、Connector、Material Zone 和 A005 lineage 可追踪；
- 无部分工件、旧质量报告或隐藏 fallback。

### 11.3 几何与结构视觉门

- 必需部件完整；
- attachment/contact 不悬空；
- critical 部件不存在明显 AABB 穿插；
- 参考相机下轮廓、占画比例和关键关节位置在允许误差内；
- 非平面目标在至少两个非参考角度不退化为薄片；
- 负空间、连杆厚度和末端执行器从多视图保持可读。

### 11.4 材质与表面门

- base color、metallic-roughness、normal、occlusion、emissive 独立有效；
- Material Zone 谱系一致；
- critical decal/flowline/groove/fastener/roughness 项进入对应输出；
- 不用更高纹理分辨率或无意义细分替代缺失结构；
- texel density、LOD 和不同 profile 保持同一 Material Zone 谱系。

### 11.5 渲染和视觉判断门

- 使用同一受控 renderer、固定相机、背景和灯光；
- 生成 reference-match view、iso、front、back、left、right、top 和必要 close-up；
- 确定性 hard gate 失败时不调用视觉模型；
- hard gate 通过后，VLM/Agent 只判断难以确定化的比例、材质可读性和表面层次；
- 自动视觉结果只能用于开发收敛，不能替代 M108B 独立真人评分。

## 12. 有界原位修复

V003 规则保持不变：

- 初始完整合成一次；
- 只有失败报告限定为同一 Brief、Domain Pack、核心形态意图和 provenance 内可修复字段时，最多进行两次原位修复；
- 每次修复必须修改同一个 workspace，并重新运行完整受影响 Gate；
- critical detail 连续两次未解决、结构方向振荡、分数平台期、预算/时间耗尽或输入证据不足时停止；
- 停止时明确报告缺失视角、当前能力或质量阻断，不生成低质量正式版本；
- “换一个思路”创建新 Turn 和新 workspace，不展开隐藏候选。

## 13. Agent 工具模型

DeepSeek 通过受限 Product Tool 操作 typed sources：

### 13.1 理解与检查

- `inspect_design_workspace`
- `inspect_reference_evidence`
- `inspect_visual_detail_inventory`
- `inspect_part`
- `inspect_material_zone`
- `inspect_compile_readback`
- `inspect_visual_defects`

### 13.2 形态与轮廓

- `create_morphology_program`
- `add_scaffold_node`
- `connect_scaffold_nodes`
- `create_negative_space`
- `change_silhouette`
- `replace_topology_family`
- `create_profile_sketch`
- `edit_profile_curve`
- `create_section_set`

### 13.3 几何、装配与硬表面

- `loft_shell`
- `sweep_structure`
- `revolve_housing`
- `extrude_panel`
- `subtract_opening`
- `add_transition_shell`
- `split_armor_panel`
- `add_panel_gap`
- `route_cable_harness`
- `add_fastener_pattern`
- `add_end_effector`

### 13.4 表面

- `create_material_zone`
- `apply_pbr_material`
- `draw_flowline`
- `add_decal`
- `add_normal_relief`
- `add_roughness_variation`
- `add_edge_wear`
- `set_texel_density`

### 13.5 编译、修复与交付

- `compile_preview`
- `render_fixed_views`
- `run_geometry_gate`
- `run_surface_gate`
- `repair_current_design`
- `prepare_single_result`
- `confirm_version`
- `export_glb`

每个工具必须有 Rust schema、同命名空间 registry、预算、revision/CAS、取消、幂等、失败零版本副作用和测试。Skill 只能教 Agent 何时调用工具，不能凭文本增加工具或 ShapeProgram operation。

## 14. Skill 结构

机械臂首个正式设计 Skill 应包含：

```text
robotic-arm-hard-surface.skill
├── intent and reference interpretation
├── morphology planning
├── joint hierarchy grammar
├── open-frame and enclosed-link grammar
├── cable routing and clamp grammar
├── armor layering and negative-space rules
├── end-effector grammar
├── material and surface detailing rules
├── VisualDetailInventory rubric
├── multi-view defect checklist
├── bounded repair strategies
├── allowed Product Tools
└── positive / negative / stop examples
```

Skill 负责设计知识；Tool 负责可执行动作；Rust 合同负责安全；Compiler 负责真实模型；readback 和 render evidence 负责证明结果。

## 15. 性能与轻量化

轻量化通过分层工件和增量编译实现，而不是降低最终质量：

- `interactive_preview` 使用较低截面采样、低分辨率 PBR 和按需可见部件；
- `production_concept` 使用更高几何密度、1K/2K 压缩 PBR、LOD 和延迟加载；
- 两档共享同一 workspace source、ShapeProgram、AssemblyGraph、Material Zone 和 Surface Program 谱系；
- profile 只改变允许的采样、纹理和压缩策略，不改变设计语义；
- 编译缓存以 source/subtree hash、runtime manifest、artifact profile 和 material lineage 为 key；
- 局部 Surface 修改优先复用未变化几何；
- Rust 向 Python 传递 artifact handle/受限对象，不在多个子进程重复重编译同一 production GLB；
- 单工作台始终只有一个 WebGL context。

## 16. 验收标准

### 16.1 机械臂黄金资产

- C111A 完成 Product Tool→唯一结果→preview→confirm→Snapshot→A005→production export→新进程恢复；
- 冻结目标参考、六视图和关键 close-up；
- 三位未参与实现的真人对 proportion、material readability、surface detail 的中位数均达到 `4/5`；
- 任何自动/VLM 分数不能替代真人门。

### 16.2 机械臂设计自由度

冻结至少 12 条此前未用于实现的机械臂 Brief，覆盖紧凑封闭、开放工业、双轨、重型维护、轻量服务、不同末端工具和不同表面语言。验收要求：

- 不同语义产生可解释的 topology/profile/section/part/material-zone 差异；
- 不能只有换色或缩放；
- 每个结果通过编译、readback、多视图、版本和导出；
- 连续五轮自然语言修改仍在同一项目/版本谱系中；
- 非法拓扑、超预算、越界和证据不足稳定拒绝；
- 未达到支持范围时明确停止，不用默认机械臂掩盖失败。

### 16.3 企业级正确性

- Rust 是唯一产品状态和设计源合同所有者；
- Python 无 Provider Key、SQLite/CAS 路径和 Snapshot 写权限；
- 每次工具修改有 revision、幂等、取消和审计；
- 所有永久变更 preview→confirm；
- 工作区/编译/readback/结果/版本 hash exact-lineage；
- 重启、断线、迟到结果、旧 revision 和并发冲突可恢复；
- CI 保留五层 Gate 和最终聚合 Gate。

## 17. 实施顺序

```text
C111A：用 VisualDetailInventory 深化并关闭机械臂黄金资产
→ C112：从黄金资产抽取 Operator/Generative Pattern
→ C113：DesignWorkspace@1 与 DesignWorkspacePatch@1
→ C114：MechanicalMorphologyProgram@1 与 Rust lowering
→ C115：生成式 ProfileSketch/ProfileSectionSet authoring
→ A006：Codex 式 typed Product Tool 设计循环
→ Q004：确定性优先的 VisualConvergenceReport 与固定构建阶段
→ E004：未见 Brief 的机械臂自由度/连续修改验收
→ M108B：机械臂正式真人门，然后逐领域扩展 kit
→ D006：按 Domain Pack 晋级扩展生活机械
→ M109：四领域自适应 production profile、KTX2/LOD/设备分级
```

同一时刻只领取一个原子任务。C111A 未达到退出条件前，不并行建立完整通用语言；允许先在 C111A 内以 fixture 形式验证 Detail Inventory 和固定视图方法，但不能将其宣传为通用能力。

## 18. 近期最小闭环

近期唯一重点仍是机械臂，不扩展更多整机目录：

1. 为当前目标图建立 C111A `VisualDetailInventory`，按 macro/meso/micro 枚举 critical 细节；
2. 将每项绑定到现有 Recipe、Shape operation、Material Zone 或 A005 program；
3. 优先解决掌壳盒体、关节轴承盒层级、装甲嵌合、线缆固定、指节/远端过渡；
4. 重新编译 preview/production GLB；
5. 冻结 reference-match view、六视图和末端 close-up；
6. 只在自动 hard gates 通过后进行视觉评审；
7. 接入正式 Product Tool/SingleResultDecision/ActiveDesignSnapshot 生命周期；
8. 完成真人 `4/5` 后，再从这个资产抽取生成器。

## 19. 后果

### 正面

- ForgeCAD 不再被定义为组件拼装器或 text-to-mesh 黑盒；
- 视觉质量、自由度和企业级版本正确性进入同一编译模型；
- 参考图中的细节不再停留在 prompt，而必须进入可回读输出；
- Agent 可以逐步获得接近 Codex 修改源码的设计自由度，同时保持 Rust 安全边界；
- C111A 的投入会转化为可复用生成能力，而不是只得到一个孤立 showcase。

### 成本

- 需要新增高层 typed AST、workspace patch、lowering、增量编译和视觉 Gate；
- 视觉质量不能只靠单元测试，需要冻结参考、真实渲染和独立真人评审；
- 机械臂通过前，不能诚实宣称“任意描述自由生成各种 3D 模型”；
- Domain Pack 扩展必须逐领域建立语言、Recipe、Skill 和 benchmark。

## 20. 被否决方案

- 直接安装 img2threejs 并让 Agent 输出任意 Three.js；
- 把 Canvas/DOM/HTML/CSS 作为最终模型；
- 继续只增加固定整机 Recipe；
- 取消 Rust 限制以换取“自由”；
- 一个 Turn 生成多个完整模型后比较；
- 用更大纹理、更多三角形或 bloom 掩盖结构不足；
- 用 VLM、自智能体或自动图像分数代替 M108B 真人门；
- 在机械臂黄金路径未成立前同时铺开汽车、飞机、道具和生活机械。
