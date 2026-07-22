# ForgeCAD 3D 机械设计系统目标操作手册

版本：v1（2026-07-17）
状态：目标操作设计；不是当前本机 Alpha 用户指南

本手册定义 ForgeCAD 完成后，零基础用户如何用一句话、参考图和少量可视化操作生成、检查和继续编辑机械概念 3D。当前真实操作以 [零基础用户指南](USER_GUIDE.md) 为准；F026 已移除三方向 UI，过渡期仅允许第一条 legacy 文本方向适配为一个临时 3D 结果。后端已有受限 Profile/Extrude/Revolve/Loft/Sweep runtime、唯一 Manifold Python union/subtract、不可变 Feature History，以及 G826 的 edge finish/UV0/tangent/稳定 face→part/zone GLB readback；零基础工作台仍没有自由轮廓、Loft、Sweep 或 CSG 编辑入口。

ADR-0015 已把视觉路线拆为两层：M108A 已建立同一 ShapeProgram 的 `interactive_preview`（128×128 v3）与 `production_concept`（512×512 v4、`GeometryCompileReadback@2`、production 质量/导出/CAS）工件管线；K003 与 C105 也已完成机制闭环。M108B 仍须用 Recipe-backed fixture 和至少三位独立真人逐领域三项中位数 `4/5` 验证生产级概念资产视觉基线。现有固定 showcase 和 C105 的 416-triangle 四领域 fixture 只能作为机制/preflight 证据，不能按本文宣称已经达到完整目标产品能力。ADR-0016 将本路线正式命名为 ForgeCAD Design Surface Compiler。

## 1. 核心操作模型

ForgeCAD 不把 HTML 面片当成模型，也不把所有产品都从立方体不断裁剪出来。目标系统借鉴 UI 组件库的组织方式，但输出仍是经过验证的 3D 几何：

```text
UI Component + Props
→ DOM

3D Component Recipe + Parameters
→ ShapeProgram
→ GeometryCompileReadback@2（production）
→ GLB
```

各层职责固定如下：

| 层 | 负责 | 不负责 |
| --- | --- | --- |
| HTML / React | 工作台、输入、抽屉、状态、确认 | 网格、UV、材质或版本真值 |
| SVG | 受限二维轮廓和截面控制点编辑 | 六个互不相关的模型面 |
| GSAP | mini/focus、抽屉、步骤、相机和爆炸图展示过渡 | 生成网格、布尔、UV、版本或质量 |
| ShapeProgram | 受限轮廓、放样、扫掠、旋转、布尔、阵列和表面处理 | 任意 Python、JavaScript、shell、URL 或路径 |
| AssemblyGraph / Recipe | 部件层级、角色、连接、pivot、材质区和可编辑绑定 | 工程载荷、公差、制造或认证结论 |
| GLB readback | 三角形、bounds、节点、材质区、UV、切线和格式事实 | 结构安全、适航、材料性能或制造可行性 |

## 2. 零基础主流程

目标工作台只要求用户表达目标并确认结果，不要求选择 Domain Pack、建模语法或三个方向。

```text
描述目标 / 可选添加参考
→ Agent 回述理解与不确定项
→ 内部选择领域包、风格 Token 和组件 Recipe
→ 内部建立轮廓与截面
→ 完成一次完整合成并编译真实 GLB
→ 硬门失败时，对同一设计意图最多原位修复两次
→ 只展示一个通过结果
→ 用户查看 3D 或继续用自然语言修改
→ 需要时编辑轮廓、组件或材质区
→ 预览
→ 确认并创建不可变子版本
→ 真实 readback 质量检查
→ 导出当前同版本 GLB
```

### 2.1 输入设计目标

推荐描述包含：对象类别、完整外观、使用场景、比例感、设计语言、颜色/材质意图和展示姿态。例如：

```text
设计一台紧凑的桌面咖啡机，圆润外壳、前置控制区、深色金属和透明水箱，整体简洁。
```

```text
设计一辆冰原探索概念车，短前后悬、封闭座舱、大轮胎、耐候外观，只用于非功能展示。
```

若类别含糊，Agent 只问一个问题；若请求涉及现实武器制造、工程尺寸、飞行安全、机器人控制或认证结论，系统在 Planner 和 Provider 前停止，不创建候选或资产。

### 2.2 可选参考

用户可添加自己有权使用的多视图图片或 GLB。参考只形成 `ReferenceEvidence@1`：

- 图片记录视角、来源、许可证和内容 hash；
- GLB 经过安全导入和真实 readback，保持只读；
- Agent 只提取可见轮廓、比例区间、色块、材质区和部件假设；
- 缺失视角、遮挡和隐藏结构必须显示为不确定；
- 新结果由 ForgeCAD 受限运行时重建，不能原地编辑、复制或冒充参考模型。

### 2.3 Agent 内部建模

用户只看到聚合步骤：

```text
✓ 已理解完整外观目标
✓ 已选择适合的结构与外观语言
○ 正在生成完整模型
○ 正在检查几何、材质与可编辑性
○ 正在修复同一设计（1 / 2）
```

系统不再生成多个完整模型后评分比较。同一尝试必须通过范围、Schema、运行时白名单、预算、编译、GLB readback、Brief 覆盖、比例、角色完整、材质区、可编辑性和安全硬门。硬门只能产生可诊断修复项，且最多修复两次；仍未通过时必须明确失败并保持零版本副作用。

### 2.4 查看最佳结果

默认只显示一个 `GenerationResultCard`，包含：

- 完整外观预览；
- Agent 的合成与硬门摘要；
- 已覆盖和未覆盖的 Brief 要点；
- 当前来源是本机离线规划还是已连接模型服务；
- “查看 3D”“继续修改”“换一个思路”。

“换一个思路”创建新 Turn 和新的单次合成尝试，不覆盖已确认版本。

## 3. 3D 视口操作

### 3.1 docked 与 focus

3D 默认持续显示在工作台右侧的 docked viewport。点击后，同一个 canvas/renderer 移到中央 focus；关闭或按 Escape 后返回右侧。

切换过程中必须保持同一场景、相机、选择、材质、纹理缓存和 `ActiveDesignSnapshot.render_preset`，不得创建第二个 WebGL context，也不得因为视口移动创建资产版本。

### 3.2 选中部件

点击部件后只显示 3–5 个最常用动作：

- 继续用自然语言修改；
- 调整已声明的比例档位；
- 更换兼容组件；
- 修改当前材质区；
- 锁定、隐藏或单独查看。

没有真实参数绑定、材质区或兼容组件时，界面明确显示不可用，不猜测控制项。

## 4. Agent 如何选择建模语法

用户不选择建模方式。Agent 根据领域、部件角色、轮廓特征、Recipe 和运行时白名单决定：

| 产品或部件结构 | 首选语法 | 典型用途 |
| --- | --- | --- |
| 机柜、打印机、工业设备外壳 | Profile + Extrude + 局部 CSG | 平直面板、门、开口、控制区 |
| 汽车、飞机、咖啡机、吸尘器主壳 | 多截面 Loft + 局部 CSG | 连续外壳、流线过渡、座舱和罩体 |
| 轮胎、旋钮、轴套、关节罩 | Revolve | 轴对称部件 |
| 扶手、管路、框架、线缆外观 | Sweep | 沿路径延伸的截面 |
| 窗洞、轮拱、进气口、凹槽 | Boolean Subtract | 局部裁剪和开孔 |
| 散热片、按钮、紧固件视觉件 | Array / Radial Array | 有节奏的重复细节 |
| 机械臂、设备、工程机械 | Component Recipe + Connector | 可替换部件和层级装配 |
| 接缝、标志、细小表面纹理 | Decal / Normal / Roughness | 不必增加真实几何的视觉细节 |

立方体裁剪仍可用于局部硬表面处理，但不能成为所有对象的唯一语法。

## 5. 轮廓与截面编辑

### 5.1 为什么不是“六个面”

前、后、左、右、上、下六个独立 HTML/SVG 面容易产生缝隙、法线冲突、曲率断裂和不可连续的 UV。目标编辑器使用共享轮廓与截面：

```text
正视轮廓 + 侧视轮廓 + 顶视轮廓
+ 沿主轴排序的横截面
→ 统一重采样
→ Loft / Extrude / Revolve / Sweep
→ 封闭体与 readback
```

### 5.2 打开轮廓编辑器

只有选中的 Recipe 声明可编辑 `ProfileSketch@1` 时，才显示“调整轮廓”。打开后：

1. 选择正视、侧视、顶视或某个横截面；
2. 拖动受限 Bezier 控制点或调整对称/饱满/收尖等普通语言参数；
3. 系统实时验证闭合、绕序、交叉、点数和曲率边界；
4. 右侧只显示临时预览，不创建版本；
5. 选择“保留修改”后创建 ChangeSet 子版本，选择“取消”则丢弃。

SVG 是 `ProfileSketch@1` 的编辑器，不是几何真值。控制点必须序列化为规范 JSON，并由后端重新验证；前端路径字符串不能直接进入 Worker。

### 5.3 失败反馈

下列情况必须在编译前拒绝：

- 轮廓未闭合或自交；
- 孔洞绕序错误或超出外轮廓；
- 截面顺序重复、相交或数量超限；
- Sweep 路径退化、frame 翻转或明显自交；
- 预算、bounds 或三角数超限；
- 请求的操作尚未进入运行时白名单。

失败时保留当前已确认资产，不生成部分成功模型。

## 6. 组件 Recipe 操作

`EditableComponentRecipe@1` 是 3D 组件库的最小复用单元。它可以包含：

```text
component_role
profiles[] / section_sets[]
geometry_features[]
parameter_bindings[]
connector / pivot
material_zones[]
child_slots[]
allowed_domains[]
quality_profile
version / provenance
```

用户操作仍保持简单：

- “把把手换成更圆润的”；
- “让座舱更靠前”；
- “把关节外壳做得更紧凑”；
- “车轮保持不变，只改车身”。

Agent 只能从同领域、同角色、连接兼容、质量通过、来源可追溯的 Recipe 中选择。锁定部件、循环父子关系、超预算、跨领域或质量失败均在预览前拒绝。

Recipe 目录不是开放资产市场。C105 的当前合同限定为 ForgeCAD 代码所有、已审阅、不可再分发的非功能视觉 Recipe；`source/review/license` 必须随实例保存。child slot 不是“任意换件”：它只能启用配方已固定的 reviewed child，真正替换仍走 C102 兼容性检查和 ChangeSet preview→confirm。实例化先做 Rust-only、零写的展开；只有同一 expanded ShapeProgram 取得实际 GLB/readback 后才能给出预览，最终组件替换、比例或材质修改仍必须 preview→confirm。Recipe ref、版本和 registry hash 失配时拒绝为 stale，不能悄悄换成目录最新版本；确认前和被拒绝的候选都不写 Snapshot 或版本。

connector 的 `normal/up`、pivot 和固定 slot local transform 共同定义局部附着。当前静态 GLB 路线只能烘焙最终平移；任何无法在 Rust 校验后烘焙的旋转/缩放、坏 frame 或循环都会失败，而不会在 Python 几何端被忽略。Python 只编译 Rust 展开的受限 ShapeProgram，不拥有 Recipe registry、项目状态、密钥或 Snapshot 写权限。

## 7. 材质区与真实外观

目标模型不是给整个对象换一种颜色，而是先建立稳定表面区域，再绑定视觉 PBR：

```text
primary_shell / secondary_shell / trim
transparent / rubber / interior / emissive
```

用户选择部件后，再选择普通语言区域和视觉材质。每个正式区域必须有真实面集合、稳定 ID、UV0、法线、切线和 GLB readback；每套纹理记录 hash、色彩空间、尺寸、来源、许可证与回退。

缺少纹理、UV 或 tangent 时必须显示“使用参数外观回退”，不能声称真实纹理已生效。视觉材料只描述外观，不提供工程材料性能。

## 8. 预览、确认、版本与质量

所有永久修改遵循同一顺序：

```text
proposed
→ previewed
→ confirmed / rejected / stale / failed
```

- preview 不创建 AgentAssetVersion；
- confirm 创建不可变子版本，并原子更新 Agent head 与 Snapshot；
- stale Snapshot 不得覆盖新版本；
- undo/redo 通过新的不可变版本恢复内容；
- Quality 只读取本次真实 production `GeometryCompileReadback@2`；
- 导出只引用与 Snapshot、质量、选择一致的当前资产版本。

质量提示只说明网格、bounds、预算、法线、UV、材质区、装配引用和 GLB 格式事实，不给出结构、安全、适航、动力学或制造结论。

## 9. DeepSeek 无响应时如何操作

当前 A003 在设计助手的模型配置区显示“未调用 DeepSeek”“等待显式调用”或稳定失败码。保存时按以下顺序验证：

1. Provider metadata 是否存在；
2. Keychain 是否可读；
3. supervisor 是否已重启；
4. 新 Agent 是否报告 Provider capability；
5. 用户主动点击“测试连接（会联网）”后，本次是否真的发起网络请求；
6. 当前请求阶段、耗时、用量与缓存；
7. 错误类别与已有资产安全状态。

错误已区分：请求格式、API Key、余额不足、参数/模型、请求过多、服务故障、超时/网络、空 JSON、无效 JSON、Schema 不符，以及 Action Loop 的 Tool Call 上限、重复 ID、stale Snapshot 和未注册工具。连接测试和普通 Turn 都可以取消；取消会传播到 Provider 和可取消 Worker。选择真实 Provider 后失败不会静默回退为“DeepSeek 已成功”；已有资产保持不变，由用户显式重试。`GET /api/v1/agent/product-tools` 可用于核对当前固定工具合同，但不会执行工具；原始 `reasoning_content` 不会出现在 Item 或日志中。F026 已将顶栏收敛为面向用户的连接状态，未暴露 Provider 技术细节。

## 10. GSAP 动画边界

GSAP 只用于提高操作连续性：

- mini 3D 移到中央 focus；
- Agent 步骤依次出现；
- 抽屉和确认条；
- 相机平滑过渡；
- 爆炸图展示；
- 预览、确认、取消和恢复反馈。

实现使用可暂停、反向和取消的 Timeline，优先动画 `x/y/scale/rotation/autoAlpha`，并通过 `gsap.matchMedia()` 支持 `prefers-reduced-motion`。动画状态不创建版本、不写 Snapshot、不改变几何或质量事实；状态机完成切换后，动画只反映该状态。

## 11. 当前与目标对照

| 能力 | 当前本机 Alpha 软件 | 目标状态 |
| --- | --- | --- |
| 结果生成 | F026 过渡期只显示第一条 legacy 文本方向适配的一个临时 3D 结果；它没有 V003 Gate 结论 | Agent 单次合成，失败时最多两次同意图原位修复，只显示一个通过结果 |
| 主形体 | 已有受限 Profile/Loft/Sweep/Revolve/CSG runtime，但固定 showcase 仍以 primitive 和绝对坐标组合为主 | Recipe 自动选择轮廓、Loft、Sweep、Revolve 与受限 CSG |
| 轮廓编辑 | 未提供 | 受限 SVG/ProfileSketch 编辑 |
| 布尔 | 单一 Manifold Python 受限 union/subtract 已实现，无自由 UI | Recipe 内受限采用并保留 feature/source provenance |
| 材质/工件 | M108A 已完成 preview 128 v3 与 production 512 v4、多区、UV/tangent、真实 readback/CAS；固定 showcase 仍不是 M108B 视觉完成证据 | M108B Recipe-backed 真人视觉 4/5 |
| 组件 | C102 项目内受限替换；C105 已实现 8 项、四领域、Rust-owned 的 Recipe Registry/展开/版本生命周期，但 416 triangles 只是机制证据 | 扩展为生产级 Recipe 目录，并经 M108B 验收完整外观 |
| 视口 | 当前大视口 | 右侧 docked，点击同一 canvas 中央 focus |
| Provider | A003 preflight、stream、cancel、usage 和稳定错误已实现；K001–K003 已由 Rust app-server/core 单一拥有 Provider/Tool、产品状态和持久化，Python 只执行受限几何；本机仍可能未配置 Provider | Rust-first 边界维持，后续只在受限 Tool/审批合同内扩展 |
| 产品范围 | 四个首批领域 | 逐包晋级生活机械，不使用万能 fallback |

## 12. 目标验收清单

只有同时满足以下条件，本文流程才能进入当前用户指南：

- G819 运行时白名单单一真值完成，未实现操作零静默忽略；
- Q003 质量只读取真实编译/GLB readback；
- ProfileSketch、Extrude/Revolve 增强、Loft、Sweep 逐项有 Schema、Worker、readback、预算和失败 Gate；
- 稳健布尔候选经过许可证、体积、冷启动、内存、确定性、材质区和 macOS/Windows 打包 benchmark；
- 边缘处理、法线、UV0、tangent 和稳定 Material Zone 有真实回读；
- M108A production 工件与 K003 Rust-first 已通过；C105 Recipe 的完整 preview/confirm、版本/恢复、production readback Gate，M108B 视觉基线、内部最佳候选和 docked/focus 仍须通过各自任务 Gate；
- 工作台始终只有一个 WebGL renderer/context；
- 四领域 Recipe-backed 视觉基准达到逐领域三项真人中位数 `4/5`，新领域只有通过 `draft → evaluated → enabled` 才可自动使用；
- USER_GUIDE、能力—Gate 矩阵和 handoff 已按真实结果同步。
