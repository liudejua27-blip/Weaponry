# ForgeCAD Schema Contract

版本：2026-07-18

Schema 是桌面端、本地 Agent、领域包、组件库、材质库、几何 worker 和导出的稳定边界。所有 JSON 必须包含 `schema_version`，并在写入不可变对象前验证。

## 1. 当前已实现合同

当前仓库同时存在两组合同：

```text
packages/weapon-spec/      legacy Weapon/Unity runtime
packages/concept-spec/     当前通用机械概念 Agent 工作台
```

D005 新增 `MechanicalStyleToken@1`、`DomainSemanticProportionRecipe@1` 与 `ResolvedSemanticProportionOptions@1`；A004 新增 Pydantic/OpenAPI `ForgeCADProductToolRegistry@1`、`ProductToolManifest` 与持久化 Tool Item 使用的 `AgentActionToolEvent@1`。K001 新增代码所有的 `ForgeCADAppServerProtocolManifest@1`，冻结 `forgecad.app-server/1`、JSON-RPC 方法/通知、能力、队列/帧限制、canonical hash、显式 method+segment compatibility 路由白名单与只读资源边界。旧 K001 manifest 的 `state_owner=python_compatibility_adapter` 和 `persistent_state_writers=[python_fastapi]` 作为历史迁移 fixture 保持字节稳定；K003 当前所有权由 initialize 的 Rust owner、SQLite ownership marker 和 packaged/layered Gate 证明。这些合同均已进入生成类型、固定 fixture 或任务 Gate。

当前 Concept 合同包括兼容的 `WeaponConceptSpec@1`、`ModuleGraph@1`、Module Asset/Pack、ChangeSet、Quality、Export，以及已落地的 `DomainPackManifest@1`、`DomainInferenceResult@1`、`ConceptScopeDecision@1`、`VisualIntentMapping@1`、`MechanicalConceptSpec@1`、`AssemblyGraph@1`、`MaterialPreset@1`、`MaterialTextureObject@1`、`EditableParameterBinding@1`、`AgentAssetVersion@1`、`AgentAssetChangeSet@1`、`AgentComponent@1` 和 `AgentStructureSuggestion@1`。C105 另冻结 `EditableComponentRecipe@1`、`ComponentRecipeRef@1`、`ComponentRecipeInstantiationRequest@1`、`ComponentRecipeCandidate@1` 与 `ComponentRecipeInstanceProvenance@1` 的合同；其完整运行时/版本/恢复 Gate 仍在进行，不能因 Schema 存在即标记 C105 完成。这些合同已经有 JSON Schema、TypeScript/Python registry 或 OpenAPI 类型与 smoke；G3 已有受限 ShapeProgram/领域 blockout 生成链，G6 已有声明式 Connector 吸附与 GLB readback，G6.5 可引入只读 `ExternalGLBReference@1`。G807 另有运行时版本化变体目录；G812 在 build/segment OpenAPI 请求与响应中增加可选、受限的 `variant_id`，G813 再增加仅为 `0..2` 的 `variation_index`（旧响应缺失时默认为 `0`）。G815 的 `VisualIntentMapping@1` 将三张方向的有限轮廓、细节、色彩和展示姿态分类映射到同一 Pack 已审核的 0–3 视觉族；实际 ID/index 只用于同一方向三项视觉预览的一致性，经候选 JSON 与已保存的 ShapeProgram/AssemblyGraph 可追溯；它们不改变 `ModuleAssetManifest@1` 或 `ActiveDesignSnapshot` 合同，也不开放自由参数。G811 已将当前 AssetVersion 的受限声明接入零基础步进控件，不开放自由参数、单位换算或新几何执行；真实碰撞、外部 GLB 的自动重建与深度分件仍未完成。

生成与漂移检查：

```bash
npm run contracts:types:generate
npm run contracts:types:check
```

## 2. 目标通用机械合同

| Schema | 作用 |
| --- | --- |
| `DomainPackManifest@1` | 领域、模板、Connector、Joint、材质和质量/导出 Profile |
| `DomainInferenceResult@1` | 在创建计划前表达唯一识别、含糊候选或不支持；不是可持久化资产 |
| `ConceptScopeDecision@1` | DomainInference 后、Planner 前的本地范围决策；不是 Project、资产、Snapshot 或版本真值 |
| `VisualIntentMapping@1` | legacy Planner 文本方向的本机受限外观分类到既有视觉族；F026 只消费第一条文本方向，不包含尺寸、脚本、自由网格或工程参数 |
| `MechanicalConceptSpec@1` | 完整外观意图、设计语言、包围盒、姿态、材料意图和生成阶段 |
| `AssemblyGraph@1` | 分层部件、几何来源、变换、连接、关节和材质区 |
| `ShapeProgramRuntimeManifest@1` | 版本化运行时操作与 Worker executor 的唯一清单；JSON Schema enum 由此生成 |
| `ShapeProgram@1` | 受控程序化几何操作；未知或缺执行器在任一运行时入口以 `UNSUPPORTED_RUNTIME_OPERATION` 拒绝 |
| `ProfileSketch@1` | 受限二维 line/quadratic/cubic 轮廓、闭合/绕序、孔洞、规范 bounds 与统一重采样声明 |
| `ProfileSectionSet@1` | 沿一个主轴排序的 2–12 个截面引用、有限 scale/twist/cap 与统一重采样策略 |
| `GeometryCompileReadback@1` | 同一次 ShapeProgram 编译后从 GLB 回读的 hash、triangle、bounds、mesh/primitive/material、operation/output role，以及 normal/UV0/tangent、稳定 face→part/zone 与 edge-finish 事实 |
| `EditableParameterBinding@1` | 一个 Agent Part 的非执行式、用户可读数值路径声明：稳定 ID、范围、步长、单位和显示名称 |
| `EditableComponentRecipe@1` | 代码所有、已审阅、仅限非功能视觉用途的组件定义：受限 ShapeProgram 模板、轮廓/截面引用、G808 绑定、connector/pivot、Material Zone、固定 child slot、质量和来源/许可证边界 |
| `ComponentRecipeRef@1` | `recipe_id + version + recipe_sha256` 的不可变引用；永远不以“最新同名 Recipe”重写已有资产 |
| `ComponentRecipeInstantiationRequest@1` | Rust-only 的临时展开请求；区分不绑定项目的 `initial_candidate` 与绑定 project/base/snapshot 的 `active_asset_edit`，不携带 world transform、代码、URL 或路径 |
| `ComponentRecipeCandidate@1` | 只读 Recipe 展开证据，含 expanded ShapeProgram/AssemblyGraph、registry/candidate hash 与 provenance；不是第二条资产版本链或已完成 GLB |
| `ComponentRecipeInstanceProvenance@1` | 成功确认后保存在 AssemblyGraph 的实例路径、Recipe ref、registry hash、parent/slot、领域、审阅与许可证事实 |
| `SurfaceLayerProgram@1` | C107 受限二维 Design Surface：规范化向量路径、内置 decal、normal/roughness/emissive mask、对称和 UV frame；禁止 SVG 字符串、脚本、URL、文件路径与任意 shader |
| `SurfaceLayerLowering@1` / `RestrictedSurfaceLayerInput@1` | Rust 校验后生成的密封 lowering 与 canonical SHA；Python 只能消费该 DTO，将 A005 与 retained 五通道 PBR 绑定到一个已验证 Material Zone，最终 GLB/readback 保留完整 hash provenance |
| `MaterialPreset@1` | 可追溯 metallic-roughness PBR 预设 |
| `MaterialBinding@1` | Part Material Zone 到材质预设的绑定 |
| `DesignChangeSet@2` | legacy Concept 工作台的部件、连接和参数修改 |
| `AgentAssetVersion@1` | 通用机械 Agent 的不可变可编辑资产快照 |
| `AgentAssetChangeSet@1` | Agent 资产部件比例、位置、关节姿态、连接器吸附、替换、视觉材质及受限结构建议的 ghost preview/confirm |
| `AgentStructureSuggestion@1` | 由现有 AssemblyGraph、role、ShapeProgram 输出与连接事实派生的只读拆分/合并候选 |
| `AgentAssetQualityReport@1` | 含稳定 `quality_report_id` 的不可变 Agent 资产检查：装配、连接器兼容/引用、ShapeProgram、材质引用和三角预算 |
| `AgentComponent@1` | 当前项目内可复用的 Agent 部件几何快照与来源 |
| `AgentAssetExport@1` | 当前 Agent 资产的轻量 GLB 导出摘要与内嵌数据 |
| `ActiveDesignSnapshot@1` | Project 下唯一活动设计、选择、预览、质量、导出、主视口视觉引用和部件显示/保护状态；S001–S008、R001、C104 已冻结、持久化并接入桌面 Agent 工作台，广泛多客户端压力矩阵仍待验证 |
| `ActiveDesignRenderPreset@1` | Agent asset 的相机视图与灯光预设；只控制主视口，不代表工程照明或多视图导出 |
| `AgentAssetRenderView@1` | 单张 Agent 资产概念 PNG，含相机视图、透明背景、尺寸、PNG readback、SHA-256 与来源资产版本；爆炸候选附带稳定 `part_ids` |
| `AgentAssetRenderSet@1` | 四视图（iso/front/side/top）及条件式 `exploded_iso` 的只读派生结果与稳定 fingerprint；不属于版本真值 |
| `AgentThread@1` | 设计会话 |
| `AgentTurn@1` | 一次用户请求和预算/状态 |
| `AgentItem@1` | 消息、计划、工具、预览、澄清、批准和工件 |
| `ForgeCADProductToolRegistry@1` | 代码所有、不可动态扩展的 13 项产品工具清单，包含稳定 ID、输入/输出 Schema 和审批策略 |
| `AgentActionToolEvent@1` | 同一 Turn 的 tool call/result 公开事实：call/tool ID、状态、耗时、幂等键、失败类别与审批策略；不含隐藏推理 |
| `ForgeCADAppServerProtocolManifest@1` | K001 桌面协议 manifest：`forgecad.app-server/1` initialize、JSON-RPC 方法/通知、能力、limits、cursor/canonical hash、显式 compatibility route allowlist 与 `forgecad-resource` 只读无状态边界；fixture 内 owner 是 K001 历史迁移快照，K003 当前由 Rust core 单写 |
| `ApprovalRequest@1` | 永久副作用确认 |
| `ModelQualityReport@1` | 通用 Mesh/Assembly/Material/Domain Finding |

G2 合同当前位于：

```text
packages/concept-spec/schemas/
packages/concept-spec/generated/
```

### ActiveDesignRenderPreset@1（R001）

```text
schema_version: ActiveDesignRenderPreset@1
preset_id / project_id / asset_version_id
camera_view: iso | front | top | right
light_preset: cad_neutral | soft_studio | concept_contrast
updated_at
```

它作为 `ActiveDesignSnapshot.render_preset` 的可选字段迁移；Agent Snapshot 创建和资产版本切换会写入 `iso/cad_neutral` 默认值，legacy Snapshot 永远为 null。更新必须经过 `POST /api/v1/projects/{project_id}/active-design:render-preset` 的 revision/ETag/Idempotency-Key CAS。

`AgentAssetRenderSet@1` 由 `GET /api/v1/agent/asset-versions/{asset_version_id}:render` 生成。它绑定当前活动 AgentAssetVersion，图片是软件栅格化的概念沟通结果；服务端验证 PNG signature/IHDR、RGBA 8-bit 与透明 alpha readback，并以视图 SHA-256、展示模式和爆炸候选的稳定 `part_ids` 计算 fingerprint。`exploded_iso` 只在 GLB primitive 几何组与现有 AssemblyGraph/Part 完全一一对应时出现；render-set 不创建新版本，不改变 ActiveDesignSnapshot，也不能作为质量、装配或制造结论。

`AgentAssetRenderPackage@1` 是 R004 ZIP 内唯一的 `manifest.json` 合同，而不是新的 Agent 资产导出类型。它引用一个当前 `render_set_sha256`，逐项列出受控 PNG 文件名、来源 asset version、视图 SHA-256、尺寸、展示/背景模式和可选爆炸候选 `part_ids`；不保存 base64、GLB、源文件、路径、工程数据或写入时间。服务端使用固定 member 顺序、ZIP 时间戳和权限生成包，以便相同当前 render-set 的下载可逐字节复现；请求指纹不再匹配时拒绝，不把另一组图片伪装成用户刚预览的结果。

ShapeProgram@1 的 JSON Schema、Pydantic `ShapeProgramPayload` 与 Python validator 已通过 `ShapeProgramRuntimeManifest@1` 对齐；manifest 位于 `packages/concept-spec/fixtures/shape-program-runtime-manifest.json`，生成器将 operation names 写入 JSON Schema enum，合同检查与运行时都会拒绝漂移。Geometry Worker 执行 manifest 声明的受限操作并构建概念 Mesh/GLB；preview、confirm、质量和导出共用该接受/拒绝边界。Q003 的 `GeometryCompileReadback@1` 将 program/GLB hash、triangle、bounds、operation/output/material 事实与当次编译绑定；质量与导出各保留授权边界，但共享这一运行时证据。旧 `legacy_estimate` 报告只以 unavailable 隔离读取。G5/G6 可输出分件候选、确认 AgentAssetVersion 并经 ChangeSet 编辑；G6.5 的 `ExternalGLBReference@1` 仍为只读参考。复杂实体、真实碰撞和外部 GLB 自动重建仍未实现。

G820 新增的 `ProfileSketch@1` 只接受 normalized `[-1,1]` 坐标、最多 64 段的 line/quadratic/cubic、最多 8 个孔洞和 `8..256` 重采样数；Pydantic 再验证闭合/开放、实际绕序、控制点 bounds、自交、孔洞包含/重叠和总段预算。`ProfileSectionSet@1` 只接受 `2..12` 个严格递增位置、已注册 closed cross-section、统一重采样数、`0.25..4` scale、`-45..45°` twist 和首尾 cap policy。规范化把外轮廓统一为 counter-clockwise、孔洞统一为 clockwise，并以排序键、稳定数字和 canonical JSON 计算 SHA-256。ShapeProgram 的可选 `profile_inputs` 同时保存 canonical payload、合同版本和 hash；三者不一致即拒绝。G821 消费单 Profile，G822 消费 section set；Sweep 仍未实现。

G821 让现有 `profile` 通过 `profile_input_id` 与二维 `profile_scale` 消费上述 canonical payload；`extrude` 增加受限 `cap_start/cap_end`，`revolve` 增加受限 seam cap 与 `8..64` radial segments。旧 `args.points` 仍按原合同执行，不能混入新参数。G822 新增唯一 manifest 中的 `loft`：必须引用一个 `profile_section_set`，使用二维 `cross_section_scale`、有界 `axis_length` 和当前唯一 `linear` continuity；不允许 operation input、孔洞 Loft、自由控制网格或相邻截面超过 45° 的翻转风险。G823 新增唯一 manifest 中的 `sweep`：必须引用一个 closed/hole-free `profile_sketch`，并声明 2–32 点有界 path、open/closed、有限 twist 和显式 cap；闭合路径禁止 cap/twist，零长度、过短段、frame 翻转和明显自交会拒绝。G826 使 `GeometryCompileReadback@1` 从真实 GLB accessor/index 回读 UV0/normal/tangent、UV bounds、closed/boundary/non-manifold/degenerate、Loft/Sweep side/seam/cap/trim ranges，以及 `primitive_id`、`part_instance_id` 和 Material Zone face set。每个三角面写出 `_FORGECAD_FACE_ID` 与 `_FORGECAD_SOURCE_FACE_ID` 顶点属性，因而顶点/索引重排不能丢失面身份；缺失/非单位/非正交 tangent、UV 退化、空 zone、重复 primitive/zone、range 未覆盖或预算超限均使 readback 失败。`bevel_approx` 只记录 `bevel_approximation + xz_perimeter + radius_ratio <= 0.25 + subdivisions <= 3`，不表示精确 fillet。

## 3. DomainPackManifest@1

必需字段：

```text
schema_version / pack_id / domain
display_name / description / non_functional_only
templates[] / connector_types[] / joint_types[]
material_preset_ids[] / quality_profile_id / export_profile_id
```

领域包只能引用 registry 中存在的模板、组件和材质，禁止可执行代码、URL、绝对路径和 Provider 配置。

### DomainInferenceResult@1（D001 已冻结，服务待 D002）

```text
schema_version
status: recognized | ambiguous | unsupported
domain_pack_id: 仅 recognized 有且只有一个
candidate_domain_pack_ids: recognized 为同一个候选；ambiguous 为 2–4 个；unsupported 为空
matched_terms: recognized/ambiguous 的词表命中；unsupported 为空
```

它是计划前的纯分类结果，不能创建 Project、Plan、Blockout、Version、质量或导出记录。四领域中英关键词/同义词 fixture 位于 `packages/concept-spec/fixtures/domain-inference-keywords.json`。D001 只冻结合同；旧运行时的默认武器回退将在 D002 替换。

## 4. MechanicalConceptSpec@1（G2 当前字段）

```text
schema_version
concept_id / project_id
domain_pack_id / brief
design_language { keywords, silhouette, detail_density, color_direction }
envelope { min_mm, max_mm }
pose { position, rotation }
full_look { completeness, generation_stage, primary_part_roles, preview_views }
material_intents[]
non_functional_only
```

`generation_stage` 只能是：

```text
blockout | segmented_concept | editable_asset
```

Spec 表达视觉设计约束，不保存工程制造结论。

## 5. AssemblyGraph@1（G2 当前字段）

```text
graph_id / concept_id / root_part_id
parts[] {
  part_id / role / parent_part_id
  geometry_source
  transform / locked
  connectors[]
  joints[]
  material_zones[]
  editable_parameters[]
  provenance
}
connections[]
component_recipe_instances[]?  // 仅 Recipe-backed 资产；legacy 图安全视为空
```

不变量：node ID 唯一；root 存在；无环；parent/child 双向一致；geometry source 已注册；Connector/Joint 引用存在；Material Zone ID 在 Part 内唯一；锁定节点不能被普通 ChangeSet 修改。

## 6. ShapeProgram@1（G3 合同，受限概念几何运行时已扩展）

```text
schema_version / units / seed
parameters[]
operations[]
outputs[]
metadata
```

当前唯一允许集合由 `ShapeProgramRuntimeManifest@1` 定义：box、cylinder、capsule、wedge、profile、extrude、revolve、loft、sweep、mirror、array、radial_array、union、subtract、bevel_approx、surface_panel。`prism`、translate/rotate/scale、intersect、fillet_approx、pivot、Connector 和 Material Zone 从未拥有当前 Worker 执行器，现已在 Schema/Pydantic/Worker/质量入口/导出前统一拒绝。G801–G804 已实现基础 primitive、轮廓、旋转和复制；G805 的旧有限 box fixture 已由 G825 显式迁移到唯一 `manifold3d==3.5.2` handler，不存在旧 box fallback；G806 实现受限低多边形 bevel_approx 和 ±Y surface_panel；G807 使用这些受控操作组成 48 个四领域变体。任意 mesh 修复、intersect、自由 fillet 和自由曲面仍由 validator/worker 拒绝。

`GeometryCompileReadback@1.feature_history` 由同次 Worker 编译与 GLB extras 回读。每个 `GeometryFeatureNodeReadback@1` 按 ShapeProgram 顺序保存 node/op、输入 node/hash、规范参数 hash、node input/result/provenance hash、runtime manifest、CSG kernel/version、深度、triangle/closed 和 material/zone/surface role；union/subtract 必须声明唯一 Manifold kernel。旧 G824 证据 GLB 可只读返回空历史，但任何新 Worker 编译缺少历史都会失败。CSG 只接受封闭输入、有限深度/输入数/三角预算；取消、超时、近退化、非封闭和 provenance 丢失均返回稳定错误且不写部分 GLB。

不变量：`additionalProperties=false`；有限数值；引用有序无环；禁止代码、路径和 URL；operation、深度、array、bounds 和 triangle budget 有硬上限；canonical JSON 和 runtime version 进入 hash。

### 6.1 EditableParameterBinding@1（G808–G811；受限步进 UI）

每个 `BlockoutPartCandidate` 可选携带最多六个 `editable_parameter_bindings`。每项都必须包含 `editparam_` 稳定 ID、当前执行器已认识的六个 position/scale 数值路径之一、零基础用户可读的显示名称、`millimeter` 或 `ratio`、默认值、最小/最大值和正步长。Pydantic 同时校验有限数值、范围、单位-路径匹配、缩放 `0.1..10`、位置 `-100000..100000`，以及同一 Part 内 ID/路径唯一；旧资产没有该字段时安全默认为空。

它不运行表达式、代码、URL 或路径，不增加新的 ChangeSet path，也不代表工程尺寸、制造参数或现实武器功能。G809 已使既有 `set_part_parameter` 在非空声明存在时按该 Part 的路径、范围和步长校验；G810 使四领域新 blockout 的单一 `box`/`wedge` 输出生成三条 `scale.x/y/z` 声明（`0.6..1.4`、步长 `0.1`），而重复 role 与当前 cylinder/capsule 输出保持空声明，避免假装为独立参数。历史资产的空列表仅保留原六路径和全局概念边界兼容，绝不开放任意参数。G811 的桌面控件只读取当前 AssetVersion 的 AssemblyGraph 值或该绑定的声明默认值，并以一个声明步长创建 preview；它不保存本地参数草稿，确认仍由既有 preview→confirm 创建版本。

### 6.2 MechanicalStyleToken / DomainSemanticProportionRecipe（D005）

`MechanicalStyleToken@1` 只保存版本、中文名称、离散比例/边缘/表面/细节/对称/材质调色板/灯光语言、允许领域和 builtin 来源。`DomainSemanticProportionRecipe@1` 将普通语言意图绑定到 `primary_form`、`cabin_form`、`upper_link_form` 等有限语义部件槽，以及唯一的 `transform.scale.x/y/z` 路径和 `-1|+1` 声明步长；它不包含 mm、自由表达式或 ShapeProgram operation。

`ResolvedSemanticProportionOptions@1` 是活动资产的只读派生结果，绑定 asset/part/domain、runtime manifest、ShapeProgram/GLB hash、锁定状态与选项。每个选项都带真实 G808 binding 的 current/target/min/max/step/unit，以及 G826 readback 的非空 `source_operation_ids`。解析失败返回明确 `unavailable_message`，不能静默猜测。该对象不进入 Snapshot 或 localStorage，也不替代 ChangeSet。

### 6.3 EditableComponentRecipe@1（C105，in progress）

`EditableComponentRecipeRegistry@1` 是代码所有的 first-party 视觉目录，而不是用户可写模板市场。每项 `EditableComponentRecipe@1` 必须同时声明：唯一 Recipe ID/version、受限 `ShapeProgram@1` template、Profile/ProfileSectionSet canonical input、feature→operation 映射、G808/D005 parameter binding、局部正交 connector/pivot、Material Zone/目录材质、child slot、允许领域、三角预算，以及 source/review/license。当前目录的 `source_kind=forgecad_first_party`、`reviewer_kind=forgecad_internal`、`license_id=ForgeCAD-Internal-Visual-Only`、`redistributable=false` 都是强制边界：它们只描述非功能概念外观，不可作为第三方素材再分发、工程材料或制造资料。

实例持久化只保存 `ComponentRecipeRef@1` 与 registry SHA-256；确认后的 `AssemblyGraph.component_recipe_instances[]` 还保存 instance path、parent instance/slot、domain、source/review/license、quality 与 policy version。Recipe 内容或 registry 中的“最新版本”绝不能重写旧 `AgentAssetVersion`。如果 ref/hash/版本不再能在当前代码所有 registry 中精确解析，或 registry 发现不一致，操作必须以 stale/invalid 拒绝；旧资产按其已保存 hash 保持可读，迁移只能显式创建新的 preview→confirm 子版本。

`ComponentRecipeInstantiationRequest@1` 只允许两种上下文：

- `initial_candidate` 的 project、base asset、Snapshot revision、target part 和 ChangeSet 全为 null；它只产生暂存候选，不能伪造项目、版本或 Snapshot；
- `active_asset_edit` 必须带当前 project/base asset/Snapshot revision，并在 Rust core 重新检查 head、CAS、领域、目标 Part 和 C104 lock；展开本身仍为零写，之后才可进入既有 ChangeSet preview。

`slot_bindings[]` 只能显式启用 Recipe 已声明的固定、已审阅 child Recipe；它不接受任意 child 或“换一个组件”。项目内组件替换继续使用 C102 的兼容性读取与 preview→confirm 路径。父/child Recipe graph、instance path、slot ID 和 parent/child connector 必须无环、有限并可验证。child world placement 由 parent world、parent connector、slot local transform、child connector inverse 和 child root local transform 确定性组合；当前静态 GLB/worker 边界只接受最终可烘焙的平移，残余旋转/缩放或非正交 frame 必须 fail closed，不能在 Python 侧悄悄丢弃。

Recipe 展开、canonical hash、child graph/connector 校验和 AssemblyGraph/provenance 由 Rust core 完成。Python `RestrictedGeometryExecutor` 只接收已展开的 `RestrictedGeometryInput`/ShapeProgram，绝不接收 Recipe registry、connector graph、project/SQLite/CAS 路径、Provider Key 或 Snapshot 写权限。`ComponentRecipeCandidate@1` 的 `expanded` 仅证明 Rust 展开成功；只有同一 ShapeProgram 的 `interactive_preview` 或 `production_concept` artifact profile hash、实际 GLB 和 `GeometryCompileReadback@2` 之后，才可声称对应预览或 production 工件存在。最终替换、比例和材质变更仍一律走 ChangeSet preview→confirm。

当前四领域的低复杂度 Recipe/GLB fixture 仅用来验证 C105 合同、展开和跨语言 readback 线路；它们不是 M108B 的 Recipe-backed production visual kit，也没有满足独立真人逐领域 `4/5` 或“生产级概念资产”结论。

## 7. MaterialPreset 与 Binding

`MaterialPreset@1` 保留旧 payload 的必需字段，并支持完整的视觉 PBR 扩展：

```text
pbr:
  base_color / metallic / roughness / opacity
  base_color_texture_asset_id? / normal_texture_asset_id?
  normal_strength / emissive_color / emissive_strength
  transmission / ior / clearcoat / clearcoat_roughness / texture_scale[2]
visual_tags[] / source? / license? / version?
```

`source`、`license` 和 `version` 是向后兼容元数据；旧 payload 缺失时分别从 `provenance`、视觉内置默认和 `1` 迁移。纹理字段只能引用内部 `asset_...` 对象；M103 新增 `MaterialTextureObject@1`，只登记受控 PNG/JPEG/WebP 内容寻址对象、尺寸、哈希、来源和许可证，不接受 URL 或绝对路径。`visual_only=true` 永远保留，所有字段只描述显示效果，不推断真实材料工程属性。

`MaterialTextureObject@1` 的 `object_path` 是库内相对路径，API 不返回绝对路径；`source`/`license` 必须满足 `forgecad_builtin → not_applicable`、`user_created → self_declared_original|unknown`、`imported_reference → third_party|unknown`。第三方来源必须带人工提供的 `license_ref`，系统不自动判断许可证。缺失或哈希不匹配的对象在目录中显示 `exists=false`，材质安全回退到参数外观。

MaterialBinding 只把 `node_id + material_zone_id` 绑定到 `material_id`，可附带颜色和纹理缩放 override。它不修改几何，也不推断真实材料工程属性。

## 8. DesignChangeSet@2

操作白名单：

```text
add_part / remove_part / replace_part
split_part / merge_parts
set_parameter / replace_shape_program
set_transform / set_pivot / set_mirror
set_connector / set_joint_pose
set_material_binding
```

ChangeSet 必须包含 before/after 引用、目标节点、锁定检查、preview artifact、actor、Provider provenance、instruction 和结果 Version。确认前不得修改正式 Graph。

## 9. Agent 合同

Turn 状态：

```text
queued | running | waiting_for_approval | waiting_for_clarification | completed | failed | cancelled
```

Item 类型：

```text
user_message | assistant_message | plan | tool_call | tool_result
preview | approval_request | artifact
```

API Key、Authorization header、绝对路径和原始敏感 Provider 响应不得进入这些合同。

### ProviderConnectionState@1 / ProviderExecutionTrace@1（A003）

`ProviderConnectionState@1` 只描述当前进程是否 `unconfigured/offline/ready/degraded/failed`，以及 metadata、secret、supervisor、capability 和 `network_call_made` 的脱敏状态。`ProviderExecutionTrace@1` 每条只保存 trace ID、阶段、attempt、latency、usage/cache token 和稳定错误码。两者的 JSON Schema、Pydantic、生成 TypeScript 和 OpenAPI 同源；合同中没有 API Key、Authorization、Base URL、完整 prompt/response 或 `reasoning_content`。

### ActiveDesignSnapshot@1（S001–S003 已冻结、M107/C104 扩展持久化状态）

Snapshot 是服务端工作台真值的合同，不是前端缓存。它把 agent 与 legacy 设计建模为判别联合，避免同一 Snapshot 同时携带冲突活动版本：

```text
project_id
active_design
  agent_asset: project_id + asset_version_id + assembly_graph_id
  legacy_concept_read_only: project_id + legacy_version_id + module_graph_id
selected_part_id?
selected_material_zone_id?（可选；必须属于选中 Part 的真实 zone，legacy 为 null）
part_display?（可选；`ActiveDesignPartDisplay@1`，Agent asset only）
preview?  (project_id + change_set_id + base_asset_version_id)
quality?  (project_id + quality_report_id + asset_version_id)
export    (source + project_id + source_version_id)
revision / updated_at
```

`ActiveDesignPartDisplay@1` 包含当前 `project_id`、`asset_version_id`、去重的 `locked_part_ids`/`hidden_part_ids` 与可选 `isolated_part_id`。Pydantic 语义校验会拒绝跨 Project 引用、与活动 Agent version 不一致的 preview/quality/export/part_display、legacy state 中的 Agent part selection 或 part display，以及任一额外字段。S002 已提供 Snapshot 数据库表、repository 和 revision CAS；S003 已提供 GET/select/legacy-rebuild hand-off API 与 revision/ETag；S004–S008 已提供 desktop reducer、Agent 工作台接入、legacy 只读转换、质量/导出绑定和不可变回退/前进；C104 为 part display 增加同一 CAS 边界和稳定 part ID 归一化。

## 10. 兼容迁移

`WeaponConceptSpec@1` 和 `ModuleGraph@1` 通过显式 compatibility adapter 转换到目标合同：

```text
WeaponConceptSpec@1 → MechanicalConceptSpec@1
ModuleGraph@1       → AssemblyGraph@1
Module material slots → Material Zone + Binding
```

转换结果必须记录 source schema、source object hash 和 adapter version。不得覆盖原 JSON、原 Version 或当前数据库记录。

## 11. 版本与发布规则

- Schema 字符串使用 `<Name>@<major>`；
- 可选字段和兼容 enum 扩展可以在实现版本内推进；
- 破坏性字段、语义或不变量变化必须升级 major；
- Python、TypeScript、OpenAPI 和 JSON Schema 必须由同一权威源生成；
- unknown field、非法引用和越权字段必须成为自动门；
- 文档草案不能进入“当前已实现”列表，直到迁移、API、UI 和回读测试同时通过。
