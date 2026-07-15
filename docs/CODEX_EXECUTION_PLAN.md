# ForgeCAD Codex 执行总计划

版本：2026-07-15
状态：后续实现工作的权威顺序

执行前先阅读 [DOCUMENTATION_STATUS.md](DOCUMENTATION_STATUS.md)。本计划定义目标顺序，不等同于当前用户能力；当前能力以 USER_GUIDE 和 Gate 矩阵为准。

## 1. 最终目标

零基础用户打开唯一 CAD 工作台，用一句话描述未来武器概念道具、汽车、飞机、机械臂或后续机械领域；Agent 帮助用户完成：

```text
创意理解
→ Agent 内部生成、编译和评审多个候选
→ 只展示一个最佳完整外观及轻量概念 3D/概念视图
→ 自动分件候选
→ 可编辑 Agent 资产
→ 部件比例、姿态、替换和材质
→ 检查、版本和用途导出
```

软件默认不安装本地神经 3D 模型、CUDA、ComfyUI 或模型权重。

## 2. 当前基线

当前已有 G1–G7 最小纵向切片：Agent Kernel、通用合同、ShapeProgram validator、box/cylinder Worker、三方向 Planner、四领域 48 个后端 blockout 变体、分件候选、AgentAssetVersion、受限 ChangeSet、13 个六类视觉材质、项目内组件、GLB 导出/readback 和只读 GLB 参考导入。前端仍只暴露三方向，48 变体目录不是已实现用户能力。

当前生产阻断：

- Agent Snapshot 的核心编辑、不可变回退/前进、重启和并发 CAS 矩阵已完成；
- 含糊/不支持领域已经在服务端安全停止，并已由 D003 持久化 clarification Item 和单问题 UI；四领域真实 Provider truth set 仍未完成；
- 工作台核心 E2E 已通过，任务级 CAS 竞争已有 smoke；尚未拆成广泛多客户端压力和原生安装场景；
- 几何造型单一；
- Agent 四视图 PNG、条件式爆炸概念图、只含当前 PNG/manifest 的 R004 ZIP，以及 R005 的直接 GLB/概念图下载 UI 已提供；本机 `.app` 启动已通过，但原生 WebView 点击下载仍受当前自动化会话的 macOS 辅助功能授权阻断；仍无转台视频或工程渲染；
- `FGC-M101` 已完成 MaterialPreset@1 的视觉 PBR 字段和旧 payload 迁移；`FGC-M102` 已完成 13 个六类视觉材质预设；`FGC-M103` 已完成受控纹理对象、来源/许可证边界和参数回退；`FGC-M104` 已完成 Material Zone UI 检视、六类筛选、关键词搜索和真实纹理回退摘要；`FGC-M105` 已完成稳定 zone 选择、部件槽绑定和带 zone 的 ChangeSet 预览；`FGC-M106` 已完成基于真实 `allowed_domains` 的四领域兼容筛选；`FGC-M107` 已完成 Material Zone 选择的 Snapshot 持久化、CAS、重启和 undo/redo 保留；`FGC-C103` 已完成 AssemblyGraph/ShapeProgram 事实驱动的只读拆分/合并建议和受限 ChangeSet 预览确认；`FGC-C104` 已完成 Snapshot 持久化的部件锁定、隐藏与单独查看，锁定会阻止相关 ChangeSet，显示状态不创建资产版本或第二 renderer；`FGC-G808` 已完成受限 Part 参数映射的 JSON/Pydantic/OpenAPI 合同，`FGC-G809` 已使非空声明在既有 ChangeSet 中强制路径/范围/步长并冻结旧资产六路径兼容，`FGC-G810` 已让四领域新 blockout 的真实单一 box/wedge 输出生成有界比例声明，`FGC-G811` 已将真实声明接到当前 AssetVersion 的零基础步进控件；四项均不开放自由参数、单位换算或新几何执行；
- `FGC-Q002` 已冻结 `GET /active-design` 的兼容初始化边界，并将质量检查收紧为 Snapshot ETag + Idempotency-Key 的重放写入；广泛多客户端压力仍未覆盖；
- Material Zone UI 已把纹理存在性、来源摘要、参数回退、稳定 zone 选择和领域兼容筛选呈现给零基础用户；当前确定性 blockout 多数只有一个 zone，zone 选择的 Snapshot 重启持久化和更多正式资产槽位仍未完成；
- 备份已覆盖 `agent_imported_glbs.object_path`，恢复 smoke 已通过 API 回读 Agent head、Snapshot 与 export source/version；
- packaged sidecar 为空；
- 真实 Provider truth set 未完成。

## 3. 依赖主链

```text
S0 文档与合同冻结（已完成）
  ↓
S1 ActiveDesignSnapshot
  ↓
S2 领域澄清 + 版本/选择/质量/导出统一
  ↓
S3 工作台状态机 + E2E + CI
  ↓
G8 轻量几何扩展
  ↓
V1 多视图概念渲染
  ↓
M1 材质与组件扩展
  ↓
R1 sidecar、恢复、安装和发布
```

后续任务必须遵守该依赖。并行工作只能发生在不共享数据合同、迁移或同一前端状态文件的任务之间。

当前领取规则：`FGC-R002`–`FGC-R006`、`FGC-M101`–`FGC-M107`、`FGC-C101`–`FGC-C104`、`FGC-G808`–`FGC-G826`、`FGC-Q002`–`FGC-Q003`、`FGC-A003`、`FGC-E001`–`FGC-E002`、`FGC-F007`–`FGC-F024`、`FGC-P008` 与 `FGC-P002` 已完成。ADR-0010 取代原三方向目标，ADR-0011 再把视觉真实度落实为 Profile/Loft/Sweep/CSG/Recipe 的 3D 机械设计系统。G824A–G824D 已证明候选在 macOS arm64 与 Windows x64 的 provenance/readback、隔离取消、真实临时权威状态提升以及 packaged 预算/许可证；G825 已按 ADR-0013 接入唯一 Manifold Python handler和不可变 Feature History，G826 已补齐同一 GLB 的 edge finish/normal/UV0/tangent/稳定 face→part/zone 事实；A003 已建立无密钥可观察的 Provider preflight、SSE/cancel/usage/稳定错误和 no-fallback 边界。F025 是唯一 `ready`。原 `V002` 为 `superseded`。P009 保持独立发布回归任务。当前 R006/G812/G813 的三方向和三项轮换仍是 Alpha 事实，V003 完成前不能从用户指南删除；它们也不能被当作真实 Provider 或最终视觉质量。`FGC-E003` 仍是 external，只能由用户针对一次具体 run 明确授权后手工执行。

## 4. S1：ActiveDesignSnapshot

目标：Project、AgentAssetVersion、Selection、Preview、Quality 和 Export 只由一个服务端 Snapshot 绑定。

`FGC-S001`–`FGC-S008` 已完成：Snapshot 合同、存储/CAS、API、desktop client/reducer、Agent 恢复/选择/视口/质量/不可变回退/前进/GLB 导出，以及 legacy 只读/转换授权/原子提升均有对应 smoke 证据。S008 覆盖 preview、确认、质量、undo→redo、重启恢复、导出、preview 阻断和 selection/quality 的 revision 竞争；当前工作台 r3 Agent-first 路径也已通过（参考 GLB v1 → 可编辑资产 v2–v5）。legacy 兼容 UI 仍未退出、legacy 组件替换写入仍被阻断且工作台尚未完成前端组合层拆分，因此不能把整个产品称为生产级单一状态运行时。

工作包：

1. 冻结 `ActiveDesignSnapshot@1` JSON Schema、Pydantic 和 TypeScript；
2. 新增迁移和 repository；
3. 新增读取、选择和 legacy 转换 API；
4. 使用 revision/ETag 防止并发旧写；
5. 前端建立单 reducer/state machine；
6. Agent、视口、选择卡、质量和导出改读统一 selector；
7. legacy Concept 进入只读模式；
8. 增加版本一致性、重启和并发 E2E。

主要入口：

- `docs/AUTHORITATIVE_STATE.md`
- `apps/agent/forgecad_agent/application/agent_asset_editing.py`
- `apps/agent/forgecad_agent/infrastructure/db/agent_repositories.py`
- `apps/agent/forgecad_agent/api/agent_asset_routes.py`
- `apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`
- `apps/desktop/src/shared/api/forgeApi.ts`

退出条件：工作台任意时刻只有一个活动版本、一个选择、一个预览；质量和导出引用该版本；重启恢复一致；版本冲突返回可恢复错误。

## 5. S2：领域澄清与统一操作

目标：未知或含糊输入不生成错误领域资产。

工作包：

1. 将领域推断从返回 DomainPack 改为 `recognized | ambiguous | unsupported`；`DomainInferenceResult@1` 和四领域中英关键词/同义词 fixture 已由 D001 冻结；
2. D002 已消费该 fixture，增加常见同义词、英文和组合词的实际推断行为，并在 ambiguous/unsupported 时阻止写盘；
3. D003 已创建 clarification Item，不创建 plan/blockout/version；
4. UI 已显示一个普通语言问题和四个用户可读选项；
5. 选项回答以保留原始 Brief 的新 Turn 继续；
6. 四领域与未知领域 truth set；
7. 统一撤销/回退为 Agent asset head 操作，不复用旧 Concept undo。

退出条件：领域准确率达到测试阈值；所有未知输入在写盘前停下；无默认武器回退。

## 6. S3：前端状态机、E2E 和 CI

目标：把工作台组件拆为可测试的状态与视图模块。F001 characterization 已在本机 Chrome 通过，F002–F006 已完成 AgentConversation、AgentStepItem、AgentSelectionCard、四类抽屉、组合层和可访问性收敛；FGC-T002 已将单一 r3 smoke 拆成 12 个独立工作台场景并纳入 CI；FGC-T003 已完成单 WebGL、内存和 bundle 预算门禁；FGC-G801 已完成 wedge/capsule，FGC-G802 已完成 profile/extrude，FGC-G803 已完成 revolve，FGC-G804 已完成 mirror/array/radial_array，FGC-G805 已完成受限 union/subtract，FGC-G806 已完成受控 bevel_approx/surface_panel，G807 已完成四领域 48 个后端 blockout 多样性门禁；后续需把变体目录以零基础用户可理解的方式接入前端。

建议结构：

```text
cad-workbench/
├── state/CadWorkbenchMachine.ts
├── state/selectors.ts
├── agent/AgentConversation.tsx
├── viewport/WorkbenchViewport.tsx
├── selection/SelectionCard.tsx
├── drawers/ComponentDrawer.tsx
├── drawers/MaterialDrawer.tsx
├── drawers/QualityDrawer.tsx
├── drawers/ExportDrawer.tsx
└── CadWorkbenchPanel.tsx
```

先写 characterization tests，再移动代码。F002 已完成对话/步骤边界和组件树 smoke；不得在同一任务中同时重写视觉系统和状态逻辑。

E2E 拆分：首次初始化、四领域、澄清、预览不写盘、确认版本、拒绝、材质、组件、GLB 参考、质量/导出版本一致、重启、单 WebGL context。

退出条件：所有 E2E 独立通过；G1–G7 进入 CI；主组件只负责组合；没有双方向面板和双选择真值。

## 7. G8：轻量几何扩展

目标：在不增加神经 3D 模型的情况下提升造型多样性。

实现操作前先按 [GitHub 参考架构](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md) 对现有 worker、JSCAD 操作语义、Manifold Python/WASM 和 Trimesh 做隔离 benchmark。记录安装体积、内存、冷启动、确定性、失败诊断、许可证和 macOS/Windows 打包；只选择一个生产执行组合，不直接复制上游应用。

实现顺序：

1. `wedge/capsule`；
2. `profile + extrude`；
3. `revolve`；
4. `mirror/array/radial_array`；
5. 受限 `union/subtract`；
6. `bevel_approx/surface_panel`；
7. 各领域模板迁移和预算验证。

每个操作都需要 Schema、validator、runtime、确定性 topology hash、预算拒绝、GLB readback 和失败测试。不得一次实现全部操作。

退出条件：四领域至少各 12 个明显不同的完整 blockout；非法/超预算输入执行前拒绝；普通 Mac 无模型权重依赖。

### 7.1 用户优先：CAD 设计能力闭环

以下顺序是用户在 2026-07-14 明确指定的优先级覆盖；它不放宽 ActiveDesignSnapshot、概念安全边界、preview→confirm、单一 WebGL 或非工程产品范围。每次只领取一个任务，后项在前项 Gate 通过前保持 `blocked`：

```text
FGC-G819 运行时操作白名单单一真值
  → FGC-Q003 真实编译/GLB readback 质量真值
  → FGC-G820 ProfileSketch 与截面合同
  → FGC-G821 增强 Extrude/Revolve
  → FGC-G822 受限 Loft
  → FGC-G823 受限 Sweep
  → FGC-G824 Manifold Python/WASM/现有 Worker benchmark 与 ADR
  → FGC-G824A provenance、GLB readback 与隔离取消补证
  → FGC-G824B 生产式 staging 与权威状态原子提升边界
  → FGC-G824C macOS packaged candidate、预算与 SBOM 选择建议
  → FGC-G824D Windows x64 packaged provenance/lifecycle 实机证据
  → FGC-G825 单一稳健 CSG 与不可变特征历史
  → FGC-G826 edge finish、法线、UV0、tangent 与稳定 zone provenance
  → FGC-A003 DeepSeek Provider 可观察性与错误/流式生命周期
  → FGC-F025 Agent-first 工作台与 legacy 隔离
  → FGC-D005 四领域语义比例配方与受限绑定
  → FGC-A004 受限 Agent Action Loop
  → FGC-M108 多材质区与高真实度 PBR 管线
  → FGC-C105 可编辑组件配方
  → FGC-V003 内部候选评审与单一最佳结果
  → FGC-F026 Codex 式简洁工作台与左上 mini 3D
  → FGC-A005 可设计的产品专属 Skill
  → FGC-R007 参考模型引导重建
  → FGC-D006 通用机械领域包扩展
```

- G819 已建立 `ShapeProgramRuntimeManifest@1`：Schema enum 由 manifest 生成且 contracts gate 检查漂移，Pydantic `ShapeProgramPayload`、Worker executor coverage、preview/confirm、质量和导出共同消费该清单；未知/失去执行器的操作返回 `UNSUPPORTED_RUNTIME_OPERATION`，并由故障注入 smoke 验证零副作用；
- Q003 必须从同一编译/GLB readback 结果取得 triangle、bounds、operation 与失败信息，不以重复的 primitive 常数估算代替；
- G820–G823 必须按 ProfileSketch、增强 Extrude/Revolve、Loft、Sweep 四个原子任务逐项建立 Schema、Pydantic、runtime、预算、确定性 topology hash、GLB readback 和失败测试；SVG/HTML 只做编辑器，不成为几何真值；
- G824 只做现有 Worker、Manifold Python 和 Manifold WASM 的可复现实测与 ADR，不在 benchmark 任务中同时集成；G824A–G824D 已补齐 provenance/readback、隔离取消、真实 SQLite/对象库提升、macOS packaged 预算/许可证和 Windows frozen artifact。G825 已只接入 ADR-0013 选择的 Manifold Python 生产 CSG，并保存不可变 feature node/input/result hash 与 surface/material provenance，失败不输出部分 GLB；后续不得再引入第二默认内核或隐藏 fallback；
- G826 已建立受控边缘完成、法线、UV0、tangent 与 stable face/Material Zone provenance；它没有自动引入纹理资产或工程材料，M108 才消费这些真实表面事实；
- A003 已解决未配置却像“无响应”的问题：metadata/Keychain/supervisor/capability preflight、真实网络调用标记、stream/cancel、用量和 DeepSeek 400/401/402/422/429/500/503/空 JSON/Schema 错误均有 Gate；失败不静默回退并冒充 Provider 成功；
- F025 只隔离 legacy 参数、旧导出和 Graph Inspector，不移动 Agent Snapshot/CAS、ChangeSet、质量、下载或 renderer 真值；
- D005 只能声明四领域的概念比例/姿态配方和有界步长，不增加自由工程尺寸、制造参数或功能结论；
- A004 只允许 DeepSeek 调用 ForgeCAD Product Tool Registry，遵守 G819、批准、轮数、时间和费用，并按官方 thinking/tool-call 合同在同一短生命周期上下文续传 `reasoning_content`；
- M108/C105 在 G826 真实表面事实之上依次建立完整 PBR/多材质区与可编辑组件 Recipe；V003 随后才能让 Agent 自动选择已实现建模语法、Recipe 和最佳候选，只展示一个通过硬门的结果；原 V002 三方向选择目标不再实施；
- F026 只调整组合层与信息架构：3D 默认缩到左上，点击后移动同一 canvas 到中央 focus，不创建第二 renderer；
- A005 的专属 Skill 必须声明 Schema、严格工具策略、示例/eval、版本和来源，不允许任意代码/URL/路径；
- R007/D006 分别推进只读参考引导重建和新机械领域包，仍按一次一个原子任务实施。

### 7.2 3D 机械设计系统的几何边界

新的几何子链不使用“HTML 六面拼接”或“所有对象只从立方体裁剪”。目标语法路由为：

```text
平直外壳       → Profile + Extrude + 局部 CSG
连续主壳       → ordered sections + Loft + 局部 CSG
轴对称部件     → Revolve
管路/框架      → Sweep
重复视觉细节   → Array / Radial Array
装配级完整产品 → EditableComponentRecipe + Connector
浅表面细节     → decal / normal / roughness
```

每个 operation 只有在 G819 manifest 中具有真实执行器、预算和 readback 后才可被 Agent/Recipe/Skill 使用。前端 SVG 只序列化 `ProfileSketch@1`；GSAP 只处理 mini/focus、步骤、抽屉、相机和确认动画；可选 SDF/体素只允许产生可丢弃候选，不能进入 editable asset 真值。

## 8. V1：概念视图

目标：从同一活动 Agent 资产生成三分之四、正面、侧面、顶部和可选爆炸图。

约束：

- 复用同一场景/渲染管线；
- 不创建持久第二 WebGL renderer；
- 相机、背景、灯光和尺寸可复现；
- 输出绑定活动 AgentAssetVersion 和 Snapshot revision；
- 概念图不能回退旧 Concept export；
- R002/R003 的四视图与条件式爆炸 PNG，以及 R004 的当前 PNG/manifest ZIP 在 Agent 资产测试通过后可进入用户指南；转台视频和工程渲染仍不得承诺。

退出条件：R002 已满足四视图 hash/provenance/readback、重复生成一致性和桌面单图下载；R003 已满足映射事实驱动的条件式爆炸概念图、透明 alpha readback、重复生成一致性和单图下载；R004 已满足当前 PNG/manifest ZIP 的来源、hash/readback、稳定 member 顺序、重复字节一致和 stale fingerprint 拒绝；R005 已满足 Agent-only 直接 GLB/单图/图包 UI 和浏览器下载 E2E，本机 `.app` 启动/Agent 健康检查也已通过，原生 WebView 点击仍须在授予 macOS 辅助功能权限的会话复验。转台视频与任何工程渲染仍属于后续任务，不得回填为已完成。

## 9. M1：材质与组件

目标：从 13 个六类参数材质继续扩展为可检索、可追溯的视觉材料目录。

分批目录：金属、聚合物、橡胶、复合材料、透明、涂层、木材/皮革/织物。所有项必须有稳定 ID、PBR 参数、纹理对象、来源、许可证、版本和预览。

组件扩展遵守 Domain Pack role、Connector/Joint、质量、原创和审阅状态；待审、受限或质量失败资产不能成为正式默认候选。

退出条件：材质只影响目标 Zone；GLB 回读一致；组件兼容率达到阈值；不把视觉材料冒充工程材料。

## 10. R1：生产化

工作包：

1. 备份覆盖 Agent imported GLB 对象；
2. Python/Rust 单元测试和依赖审计；
3. 构建目标平台非空 sidecar；
4. supervisor 只在开发构建允许 `local-dev-python`；
5. 新机器安装、首次初始化、Provider、生成、编辑、导出和重启恢复；
6. 真实 Provider 四领域评测；
7. 刘邦完成正式资产独立审阅；
8. macOS/Windows 签名与外部发布。

退出条件以 `docs/PRODUCTION_RELEASE_CHECKLIST.md` 为准；任何必需项失败则发布状态为 blocked。

## 11. 每阶段交付格式

每个阶段交付必须包含：

- 合同/ADR；
- 迁移和回滚；
- 实现与生成类型；
- 单元、集成和 E2E；
- 当前能力文档；
- Gate 命令与真实结果；
- 未完成项和下一任务 ID。

不得只交付 UI、只交付 Schema 或只交付 smoke。
