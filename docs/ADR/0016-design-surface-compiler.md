# ADR-0016：ForgeCAD Design Surface Compiler

- 状态：Accepted（分阶段实现）
- 日期：2026-07-18
- 决策者：项目维护者
- 补充：ADR-0011 的轮廓/特征/组件配方路线、ADR-0014 的 Rust-first 所有权和 ADR-0015 的双档工件/视觉验收拆分
- 取代：ADR-0011 中“生成多个完整候选后评分选最佳”的目标策略；V003 改为单次完整合成与最多两次同意图原位修复

## 背景

用 HTML/CSS/SVG 先设计平面再折叠成 3D，对包装、折纸、低多边形展示有效，但不能直接承载 ForgeCAD 的封闭几何、连续曲率、法线、UV、PBR、稳定材质区、GLB 回读和不可变版本真值。然而，“先在二维设计外观语言，再编译为三维”本身是正确方向。

ForgeCAD 因此不把 DOM 面片折成最终模型，而是将这一想法升级为 **ForgeCAD Design Surface Compiler**：一套“二维设计表面驱动的三维机械编译器”。用户和 Agent 操作受限的轮廓、截面、材质区、图案、流线与组件 Recipe；编译器将它们降低为已验证的 ShapeProgram/AssemblyGraph，再生成可回读 GLB。

## 决策

### 1. 三种语言分层

1. **Design Surface Language** 表达用户和 Agent 的设计意图：
   - `MechanicalStyleToken@1`：相对比例、边缘性格、曲面张力、细节密度和材质语言；
   - `ProfileSketch@1` / `ProfileSectionSet@1`：正、侧、顶轮廓与沿主轴排序的共享截面；
   - `SurfaceZoneMap@1`（目标合同）：稳定 Part/Material Zone、连续边界、层级和遮罩；
   - `SurfaceAdornmentProgram@1`：雕刻感、接缝、图案、流线、decal/normal/roughness 细节；
   - `EditableComponentRecipe@1`：可复用部件、connector/pivot、child slot、受限参数和材质区。
2. **Geometry Intermediate Language** 是 Rust 验证和降低后的真正几何程序：`ShapeProgram@1` + `AssemblyGraph@1`。只允许 G819 运行时白名单中已存在的 Extrude/Revolve/Loft/Sweep/CSG/Array/表面操作。
3. **Artifact Language** 是不可变派生结果：`interactive_preview` 和 `production_concept` GLB、`GeometryCompileReadback@2`、质量、导出与 CAS 对象。

HTML/React/SVG/GSAP 只是设计语言的编辑器和过渡层，不是上述三层中任何一层的持久化真值。

### 2. 编译管线

```text
用户描述 + 可选授权参考
→ Rust Agent 建立 DesignBrief / ReferenceEvidence
→ 选择 Domain Pack / Style Token / Recipe
→ 建立共享 Profile / Section / Surface Zone
→ Rust 校验并降低为 ShapeProgram + AssemblyGraph
→ RestrictedGeometryExecutor 执行 Loft / Sweep / Revolve / CSG
→ A005 烘焙表面图案、流线与五通道 PBR
→ GLB 真实 readback / quality hard gate
→ 同一候选最多两次原位修复
→ 只展示一个通过结果
→ preview → confirm 创建不可变子版本
```

编译是纯派生过程：临时网格、截图和动画不能进入版本头。只有用户确认密封预览后，Rust Core 才能在一个事务中创建新版本、前进 head 和 `ActiveDesignSnapshot`。

### 3. 所有权与运行边界

- Rust app-server/core 拥有设计语言合同、注册表、Agent 调度、降低、版本、Snapshot、ChangeSet、质量、导出、SQLite/WAL 和 CAS。
- Python 只是 capability-gated `RestrictedGeometryExecutor`，执行 Rust 已展开和验证的几何/PBR 编译请求，不获得 Provider Key、数据库、对象库路径或 Snapshot 写权限。
- React/Three.js 只展示 Rust 返回的合同和同源 GLB；单工作台始终只有一个 WebGL renderer/context。
- 任何尚未进入 Schema + registry + validator + executor + readback + Gate 的节点都是不支持，不能被 Agent 以文本描述冒充执行。

### 4. 轻量与生产概念档

- 编辑和交互继续使用有界的 `interactive_preview`，快速重新编译。
- 展示、正式质量、下载和导出使用按需 `production_concept`。
- 当前 M108A 的 512×512 v4 PBR 是已验证生产工件基线，不是最终视觉上限。M108B 之后单独增加自适应 production profile，按需使用更高网格密度、1K/2K 压缩 PBR、LOD 和延迟加载；该扩展必须有独立的显存、体积、冷启动、确定性和 macOS 打包 Gate。

## 当前实施阶段

| 阶段 | 状态 | 责任 |
| --- | --- | --- |
| F026 Codex 工作台 | 已实现 | 左侧项目/对话/组件，中央会话，右侧唯一 3D，底部 composer |
| A005 表面外观 Skill | 已实现 | 受限图案/流线/雕刻感语言、双档五通道 PBR 与 provenance |
| R007 参考证据与重建链 | 实现中 | 只读授权证据、CAS、不确定性、Recipe-backed ChangeSet 生命周期；仍需完整验收与视觉评审 |
| V003 单次唯一结果 | 待实现 | 单次完整合成、硬门、最多两次原位修复、失败零版本副作用 |
| 生产级 Recipe 扩展 | 待实现 | 机械臂优先；主壳、关节、连杆、夹爪、线缆、盖板和细节层级 |
| M108B 生产级视觉基线 | blocked | 四领域 Recipe-backed 资产与三位独立真人逐领域 `4/5` |
| 自适应 production profile | 待新任务 | 保留轻量预览，按需升级网格、1K/2K 压缩 PBR 和 LOD |

## 实施顺序

```text
A005（done）
→ R007
→ V003
→ 机械臂优先的生产级 Recipe 扩展
→ M108B 四领域正式视觉验收
→ 自适应 production profile / 1K–2K 压缩 PBR / LOD
```

每一步都使用独立任务卡和 Gate。不在 V003 中同时扩展几何运行时，不在 M108B 中改写版本所有权，不以更大纹理替代 Recipe 完整度。

## 后果

- 保留 ForgeCAD 当前 ShapeProgram、AssemblyGraph、ActiveDesignSnapshot、ChangeSet、GLB readback 和双档工件架构，不需要推翻现有代码。
- “平面设计”被精确定义为轮廓、截面、表面区域和细节语言，而不是六个互不相关的 DOM 面。
- 零基础用户只描述对象、参考和风格；Agent 自动选择语法、编译、检查和修复。
- 轻量不再意味着低质量唯一档；预览档与生产概念档分离，但共用同一设计程序和版本真值。
- 这一决策定义的是游戏/影视道具和产品概念展示资产，不引入制造尺寸、公差、功能机构、结构/适航/安全结论。

## 被否决方案

- 将 HTML/CSS 面片作为最终实体或 GLB 真值。
- 对所有对象使用 box + CSG，忽略轮廓、截面和组件结构。
- 在一个 Turn 中生成多个完整模型后评分选一个；这会放大 Provider/几何/纹理成本并让失败语义不清晰。
- 使用未经授权参考、反向搜索或复制第三方网格。
- 用单张概念图、高分辨率纹理或代理评分宣称生产级视觉已达标。
