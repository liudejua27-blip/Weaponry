# ForgeCAD

ForgeCAD 是面向零基础用户的本地优先、轻量化 **通用机械概念 3D Agent**。用户连接兼容的大模型 API，在唯一 CAD 工作台中描述创意；Agent 提供设计方向，本机受限几何运行时生成可编辑的低多边形概念模型。

首批同级领域：

```text
未来武器概念道具 / 汽车与地面载具 / 飞机与航空器 / 机械臂与机器人机构
```

第一阶段只交付概念数字资产，所有交付都带明确的非制造说明。未来武器结果属于虚构游戏美术资产、影视道具或非功能展示模型，不输出可用于现实制造武器的精确图纸、功能机构、制造尺寸、材料配方或加工流程。汽车、飞机和机械臂结果也不提供安全、适航、结构、动力学或认证结论。

## 当前产品状态

当前是本机 Alpha，不是生产安装包。

已经实现：

- Tauri + React 桌面壳；
- FastAPI、SQLite 和内容寻址对象库；
- Agent Thread/Turn/Item/Approval 基础；
- 四个最小 Domain Pack；
- 三方向 Planner；
- 受限 ShapeProgram Geometry Worker（`box`/`cylinder` 及已通过门禁的 G801–G806 操作）；
- 48 个确定性 blockout 变体和分件候选（后端目录 Gate；前端当前仍展示三方向）；
- AgentAssetVersion、预览/确认 ChangeSet；
- 受限部件比例、位置、关节姿态、材质、项目内组件替换，以及由现有装配/几何事实生成的拆分或合并候选（均先预览再确认；不支持自由网格拆分）；
- 13 个六类视觉材质预设（无纹理时安全降级为参数材质）；
- 受控视觉纹理对象登记、来源/许可证摘要和缺失对象回退；Material Zone UI 提供当前部件/区域上下文、中文分类筛选、关键词搜索、稳定 zone 选择、领域兼容筛选、带 zone 的预览动作和参数外观安全回退；
- Agent GLB 检查、导出和回读；
- 外部自包含 GLB 只读参考导入；
- macOS Keychain Provider 配置。

尚未完成：

- Agent 路径的 Project、AgentAsset、Selection、Preview、Quality、不可变回退/前进和 GLB Export 已由 `ActiveDesignSnapshot` 绑定；当前 Agent-first 工作台 smoke 已通过，广泛多客户端压力矩阵与所有兼容 UI 的一致性仍待后续任务；
- 对含糊或不支持的领域显示单问题澄清 UI 已有服务端与 focused smoke；工作台 F001 characterization 已通过本机 Chrome，四领域真实 Provider 评测仍未完成；
- 前端将 48 个后端 blockout 变体接入可理解的选择流程、概念多视图和更完整的轻量造型语言；G807 后端多样性 Gate 已通过，但尚未形成前端变体目录与通用生成能力；
- 自动深度分件、自由 split/merge、精确碰撞和运动学；当前仅支持证据充分的拆分/合并候选；
- 任意版本历史浏览；部件锁定、隐藏和单独查看已实现为当前 Agent Snapshot 的受限状态，不是工程装配约束；
- Agent 转台视频、OBJ/MP4 和工程渲染；R002–R004 已支持四视图、条件式透明爆炸概念 PNG 预览/单图下载，以及与当前预览 fingerprint 一致的 PNG/manifest 图包，但不创建版本；
- 工作台的核心 Snapshot 回归已由当前 `desktop:r3-concept-workbench-smoke` 覆盖导入参考、可编辑资产 v2→v5、质量、导出和重启恢复；广泛多客户端并发 E2E、原生安装恢复和发布验证仍未完成；
- 工作台 F002–F004 已提取 Agent 对话、步骤 Item、选择卡和四类抽屉，F005 已由 `WorkbenchDrawerStack` 收敛为组合边界，F006 已补齐最小点击目标、键盘焦点、中文无障碍标签、状态播报和抽屉 Escape/焦点返回；FGC-T002 已拆出 12 个独立工作台 E2E 场景并全绿，FGC-T003 已通过单 WebGL、抽屉/重载资源、内存和 bundle 预算门禁；FGC-G801 已将受控 wedge/capsule 运行时加入确定性 GLB smoke，FGC-G802 已加入 profile/extrude，FGC-G803 已加入完整/半角 revolve，FGC-G804 已加入 mirror/array/radial_array，FGC-G805 已加入受限 union/subtract 失败边界，FGC-G806 已加入受控 bevel_approx/surface_panel，G807 已通过四领域 48 个结构不同 blockout 的多样性 Gate；FGC-R001 已将主视口相机/灯光预设绑定到 ActiveDesignSnapshot，FGC-R002 已接入四视图概念 PNG 预览与单图下载；FGC-M103 已完成受控纹理对象与参数回退，FGC-M104 已完成 Material Zone UI 检视与检索，FGC-M105 已完成稳定 zone 选择与带 zone 的 ChangeSet 预览，FGC-M106 已完成四领域兼容筛选，FGC-M107 已完成 zone 选择的 Snapshot/CAS 持久化及重启、undo/redo 保留，FGC-C101 已将稳定内部部件角色显示为中文并对未知角色安全回退；其余 Alpha 阻断仍记录在任务索引；
- 真实 Provider 四领域质量评测；
- 非空 packaged sidecar、签名、公证和独立安装验证。

旧 `WeaponConceptSpec/ModuleGraph`、Weapon API、ComfyUI、神经 3D 和 Unity 仍作为兼容基线存在，不是新产品主路径。

## 轻量运行方式

ForgeCAD 默认不安装 TripoSR、Stable Fast 3D、Hunyuan3D、ComfyUI、CUDA 或模型权重。

```text
大模型 API
  交流与结构化规划
        ↓
本机受限 Geometry Worker
  ShapeProgram 校验、简单几何、分件、检查和 GLB
        ↓
单一 Three.js 主视图
  选择、预览与确认
```

大模型不能输出并执行任意 Python、JavaScript 或 shell。所有永久修改先形成候选，用户确认后才创建不可变子版本。

## 当前最小工作流

```text
明确描述汽车/飞机/机械臂/未来武器概念道具
→ 选择三个完整方向之一
→ 生成轻量 blockout
→ 查看分件候选
→ 保存 AgentAssetVersion
→ 调整一个部件或视觉材质
→ 预览并确认子版本
→ 运行轻量检查
→ 导出 Agent GLB
```

当前领域识别使用四领域的有限关键词/同义词词表。输入应明确包含“汽车”“飞机”“机械臂”或“未来武器概念道具”；含糊或不支持的输入会在生成计划前安全停止，不会默认变成武器或写入资产。当前工作台会把停止结果显示为一个只问一次的普通语言澄清问题；F001 已验证澄清、预览、提交、恢复和单 WebGL canvas。

## 本机启动

安装开发依赖：

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"
```

运行本机 Tauri 测试版：

```bash
script/build_and_run.sh --verify
```

当前 supervisor 会报告 `mode=local-dev-python`，依赖开发机 Python。空 sidecar 会让 `release:packaging-readiness` 保持失败；这是正确的发布阻断。

浏览器开发预览和 Provider secret file 见 [开发与调试](docs/DEVELOPMENT.md)。打包、sidecar 和平台发布边界见 [打包说明](docs/PACKAGING.md)。

## 当前核心验证

```bash
npm run agent:check
npm run contracts:types:check
npm run desktop:typecheck
npm run desktop:tauri-check
npm run desktop:f006-accessibility-smoke
npm run desktop:t002-workbench-e2e-scenarios

npm run agent:g1-kernel-smoke
npm run agent:g2-contracts-smoke
npm run agent:g3-shape-program-smoke
npm run agent:g4-mechanical-planner-smoke
npm run agent:g5-geometry-worker-smoke
npm run agent:g6-segmentation-smoke
npm run agent:g6-material-catalog-smoke
npm run agent:g6-asset-editing-smoke
npm run agent:g6-component-registry-smoke
npm run agent:g7-external-glb-import-smoke
npm run agent:g801-shape-primitive-smoke
npm run agent:g802-profile-extrude-smoke
npm run agent:g803-revolve-smoke
npm run agent:g804-transform-arrays-smoke
npm run agent:g805-boolean-smoke
npm run agent:g806-bevel-surface-panel-smoke
npm run agent:g807-blockout-diversity-smoke
```

`npm run desktop:r3-concept-workbench-smoke` 已覆盖当前 Agent Snapshot 的预览、确认、质量、不可变回退/前进、重启恢复与 GLB 导出；它不是完整的并发或原生安装验证。`npm run release:packaging-readiness` 当前仍预期失败，因为 packaged sidecar 尚未构建。

几何扩展的当前边界：G801–G807 提供受控 wedge/capsule、profile/extrude、revolve、mirror/array/radial_array、受限 union/subtract、低多边形 bevel_approx、±Y surface_panel，以及四领域各 12 个结构不同的 blockout 变体。它们是低多边形概念几何，不是 B-Rep、精确布尔、碰撞/强度分析或制造能力；R002–R004 已提供当前 Agent 资产的多视图、条件式爆炸概念图和只含 PNG/manifest 的图包，但真实 Provider 仍未完成。

## 架构方向

```text
Tauri + React
├── Agent 会话与步骤
├── 单一 Three.js 视口
├── 选中部件简单动作
└── 检查与用途导出
          │ HTTP + SSE
          ▼
FastAPI Local Agent
├── Thread / Turn / Item / Approval
├── Provider Adapter
├── Domain Pack Registry
├── AgentAssetVersion / ChangeSet
└── Geometry Worker Port
          │
          ▼
SQLite + Content-addressed Objects
```

`ActiveDesignSnapshot` 的服务端合同、持久化、API、desktop reducer 和 Agent-first 核心路径已有证据；前端状态与工作台视图已完成首轮拆分，C101–C104 已提供中文部件名称、项目内组件替换结论、事实驱动的拆分/合并预览，以及服务端持久化的部件锁定、隐藏和单独查看。锁定会阻止相关 Agent ChangeSet，隐藏/单独查看只控制同一个主视口；它们不创建几何版本，也不是工程装配约束。后续才继续扩展轻量几何和多视图。不会通过增加本地神经 3D 模型解决造型问题。

## 文档入口

先看 [文档地图](docs/DOCUMENTATION_MAP.md)。它定义唯一权威、当前/历史/legacy 边界和已删除路线，后续 Codex 不应从搜索到的旧文件直接开始。

随后看 [文档状态账本](docs/DOCUMENTATION_STATUS.md)，确认当前能力标签、已知阻断和下一项可领取任务，再进入具体设计或代码文档。

产品与架构：

- [Codex 当前交接](docs/CODEX_HANDOFF.md)
- [Codex 执行总计划](docs/CODEX_EXECUTION_PLAN.md)
- [Codex 原子任务索引](docs/CODEX_TASK_INDEX.md)
- [Codex 完成定义](docs/CODEX_DEFINITION_OF_DONE.md)
- [产品定义](docs/PRODUCT_DEFINITION.md)
- [系统设计](docs/DESIGN.md)
- [实施计划](docs/IMPLEMENTATION_PLAN.md)
- [领域包](docs/DOMAIN_PACKS.md)
- [视觉材质](docs/MATERIAL_SYSTEM.md)
- [权威状态设计](docs/AUTHORITATIVE_STATE.md)
- [兼容迁移计划](docs/COMPATIBILITY_MIGRATION.md)
- [GitHub 参考与采用边界](docs/AGENT_GITHUB_REFERENCE_ARCHITECTURE.md)
- [插件与 Skill 操作设计](docs/AGENT_PLUGINS_SKILLS_DESIGN.md)

使用与开发：

- [Quickstart](docs/QUICKSTART.md)
- [零基础用户指南](docs/USER_GUIDE.md)
- [当前 Agent API](docs/API.md)
- [操作文档总索引](docs/OPERATIONS.md)
- [本机开发与调试](docs/DEVELOPMENT.md)
- [前端约束](docs/FRONTEND.md)

资产、质量与发布：

- [资产作者手册](docs/ASSET_AUTHORING.md)
- [模块资产制作规范](docs/MODULE_ASSET_GUIDE.md)
- [测试策略](docs/TEST_STRATEGY.md)
- [发布维护手册](docs/RELEASE_MAINTENANCE.md)
- [生产发布清单](docs/PRODUCTION_RELEASE_CHECKLIST.md)
- [故障恢复手册](docs/DISASTER_RECOVERY.md)
- [能力—Gate 矩阵](docs/evidence/CAPABILITY_GATE_MATRIX.md)
- [Legacy 兼容资料](docs/legacy/README.md)

## 贡献约束

- Core 不包含武器、汽车、飞机或机械臂专属业务名；
- 领域语义进入版本化 Domain Pack；
- ShapeProgram 永不使用 `eval`/`exec`；
- 所有永久修改先预览、再确认、再创建子版本；
- 一个工作台只维护一个 WebGL renderer；
- API Key 不进入源码、数据库事件、日志、截图或导出；
- 用户文档只写当前验证能力；目标能力写入设计/计划并明确未实现；
- legacy 资料不得重新成为产品主操作路径。

后续 Codex 在修改仓库前必须先阅读根目录 [AGENTS.md](AGENTS.md)，并从任务索引中领取一个满足依赖的原子任务。
