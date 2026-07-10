# ForgeCAD 操作与运行手册

本文把运行状态分成三层，避免用旧基线的通过结果冒充新产品能力：

- **当前可运行**：旧武神后端、ForgeCAD 基础设施和 `/cad` 参考工作台；
- **P0 目标**：通用模块化 3D 平台 + Weapon Concept Pack；
- **后续目标**：CAD / DFM Engineering Pack。

P0 不拒绝武器题材，但正式用途限定为未来概念、游戏资产、影视道具和非功能展示模型。

## 1. 当前环境

### 1.1 必需与可选依赖

必需：

- Node.js 20+；
- npm 10+；
- Python 3.9+。

可选：

- Rust + Cargo：运行或编译 Tauri；
- Chrome：执行现有浏览器 smoke；
- ComfyUI、旧本地 3D Runtime、Unity：只服务 legacy 回归。

P0 Concept 闭环不要求 build123d/OpenCascade、lib3mf 或 PrusaSlicer。它们属于后续 Engineering Pack。

### 1.2 安装

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"
```

确认环境：

```bash
node --version
npm --version
.venv/bin/python --version
```

### 1.3 启动当前 Agent

```bash
PYTHONPATH=apps/agent \
WUSHEN_LIBRARY_ROOT="$PWD/WushenForgeLibrary" \
WUSHEN_MIGRATIONS_DIR="$PWD/migrations" \
.venv/bin/python -m uvicorn wushen_agent.main:create_app \
  --factory --host 127.0.0.1 --port 8000
```

验证：

```bash
curl --fail http://127.0.0.1:8000/api/health
curl --fail http://127.0.0.1:8000/api/provider-settings
```

当前健康响应仍使用 `service=wushen-agent`。Concept Project、ModuleGraph、ChangeSet、QualityRun 与概念源包导出 API 已实现；桌面真实装配、combined GLB 和正式 R5 导出器尚未实现。

### 1.4 打开参考工作台

另开终端：

```bash
VITE_FORGE_API_BASE_URL=http://127.0.0.1:8000 npm run desktop:dev
```

当前 Vite 固定入口：

```text
http://127.0.0.1:1420/#/cad
```

这是浏览器开发壳。它可以验证布局和前端交互，但不具备 Tauri invoke、本地 supervisor 和正式桌面打包能力。

### 1.5 运行 Tauri 开发窗口

安装 Rust/Cargo 后：

```bash
npm --workspace apps/desktop run tauri -- dev
```

当前开发 supervisor 仍尝试启动：

```text
.venv/bin/python -m uvicorn wushen_agent.main:create_app
```

常用覆盖：

```bash
export WUSHEN_REPO_ROOT=/absolute/path/to/repo
export WUSHEN_AGENT_PYTHON=/absolute/path/to/python
```

日志位于 `.wushen-agent.log`。当前 supervisor 是开发机制，不是生产 sidecar 打包完成的证据。

## 2. 当前验证命令

### 2.1 静态与构建

```bash
npm run agent:check
npm run contracts:check
npm run contracts:types:check
npm run desktop:typecheck
npm run desktop:build
```

Schema、Pydantic 或 OpenAPI 模型改变后：

```bash
npm run contracts:types:generate
npm run contracts:types:check
```

生成物不能手工编辑。

### 2.2 当前最高层回归

```bash
npm run m6:gate
```

该门只验证迁移前 CreativeWeaponGraph/SkillGraph 与桌面类型。

R1 基础设施：

```bash
npm run r1:foundation-gate
```

它验证 migration 幂等、SQLite 约束、内容寻址去重、路径越界与哈希篡改检测，再执行 `m6:gate`。

R1 当前完整回归：

```bash
npm run r1:gate
npm run r2:contracts-gate
npm run r2:gate
npm run r3:workbench-gate
```

`r1:gate` 继续执行桌面生产构建和上下文连续性 smoke。`r2:contracts-gate` 只证明首批 Contract 与生成类型；`r2:gate` 进一步证明 Concept 数据与源包。`r3:workbench-gate` 在此基础上注册 3 个可渲染 GLB、绑定 ModuleGraph，并用系统 Chrome 验证工作台加载、节点选择、Connector 检查器与 ZIP 下载。它仍不证明 8–12 个正式模块、替换/吸附、AI 质量、实际 Mesh 检查器或 combined GLB/OBJ/PNG。

### 2.3 Tauri 检查

```bash
npm run desktop:tauri-check
```

缺少 Cargo 时必须记录环境阻塞，不能声称桌面包可用。

### 2.4 旧 release gate

`npm run release:gate` 仍包含旧产品的安全措辞、ComfyUI、Unity import 和旧打包条件：

- 只保留为 legacy baseline；
- 不代表 P0 Concept 产品范围；
- 不能作为 ForgeCAD 发布门；
- C01–C10 落地前不得声称新产品达到 Beta。

## 3. 数据、备份与临时环境

当前默认库：

```text
WushenForgeLibrary/
  library.db
  library.db-wal
  library.db-shm
  objects/sha256/
```

数据库和对象目录必须一起备份。

安全备份：

1. 停止 Agent、worker 和 Tauri supervisor；
2. 确认数据库没有写入；
3. 复制整个 `WushenForgeLibrary`；
4. 校验数据库、WAL/SHM（若存在）和对象目录；
5. 正式工具落地后优先使用 SQLite backup API。

测试使用独立库：

```bash
export WUSHEN_LIBRARY_ROOT="$PWD/.tmp/dev-library"
export WUSHEN_MIGRATIONS_DIR="$PWD/migrations"
```

当前没有安全清空生产库的统一命令。不要对真实资产库执行递归删除。

## 4. 当前 Provider

默认 mock Provider 最适合迁移回归。

旧 OpenAI-compatible Adapter：

```bash
export WUSHEN_LLM_PROVIDER=openai_compatible
export WUSHEN_LLM_BASE_URL=https://api.openai.com/v1
export WUSHEN_LLM_MODEL=<model-name>
export WUSHEN_LLM_API_KEY=<secret>
```

密钥只能来自环境变量或 secret file，不得进入源码、日志、Job event、资产或导出包。

旧 ComfyUI 与神经 3D Provider 不进入 P0 权威模块链路。生成式图片可以作为风格参考，但不能成为 `ModuleGraph`。

## 5. 设计者的第一周操作路径

这一节是“具体怎么开始设计”的执行顺序。

### Day 1：冻结首个 Brief

项目只做一个：`寒地巡逻 S1`。

```text
类型：未来模块化短武器概念
用途：游戏资产 / 影视道具 / 非功能展示
气质：寒地、紧凑、工业、硬表面
比例：约 230 mm 长，握持角 15°，整体偏厚重
辨识点：石墨黑、枪灰、少量信号红；顶部轮廓清晰
排除：真实工作机构、弹道、承压、制造就绪声明
```

验收物：一份 `WeaponConceptSpec` 示例 JSON、两张正交草图或参考图、模块清单。

### Day 2：做 8–12 个首批 GLB

优先制作：

- 核心外壳 1 个；
- 前部外壳 2 个；
- 后部外壳 1 个；
- 握持外壳 2 个；
- 顶部附件 1–2 个；
- 侧板 2 个；
- 能源/储存视觉模块 1 个。

每个模块：

- 原点和轴向一致；
- 应用变换后再导出；
- 米制/毫米约定固定；
- 名称、材质槽、LOD 和碰撞体命名一致；
- 先保证拓扑和连接，再追求数量。

### Day 3：标注连接器

核心至少标注：

```text
core.front
core.rear
core.top
core.bottom
core.left
core.right
core.grip
core.side_panel_left
core.side_panel_right
```

用一个人工编写的 `module-manifest.json` 先跑通对齐。不要先做自由拖拽装配。

### Day 4：完成最短工作台闭环

```text
打开项目
→ 从组件库替换前部或顶部模块
→ 查看连接器吸附
→ 调整整体比例/握持角/细节密度
→ 保存为新版本
```

### Day 5：接 AI ChangeSet

先只支持三类语句：

- “让轮廓更紧凑”；
- “换一个更低的顶部附件”；
- “增加红色装饰并保持核心外壳不变”。

AI 必须返回结构化操作，先 ghost preview，再由用户确认。

### Day 6：做模型检查与导出

人工构造四个失败样本：连接器不兼容、浮空模块、模块穿插、非法缩放。导出至少覆盖 GLB、PNG、Manifest 和 JSON 报告。

### Day 7：桌面回归

从干净临时库完整走一遍：新建 → Brief → 方案 → 替换 → ChangeSet → 检查 → 导出 → 重启恢复。

## 6. P0 目标运行契约

### 6.1 目标进程

```text
Tauri Desktop
└─ forgecad-agent sidecar    127.0.0.1:8000
   ├─ API / workflow
   ├─ module composition worker
   ├─ model-quality worker
   └─ render/export worker
```

重任务使用隔离工作目录和明确的 CPU、内存、时间、三角面与输出大小限制。P0 不要求 CAD Runtime 常驻进程。

建议健康端点：

```http
GET /api/v1/health
GET /api/v1/readiness
```

`readiness` 分别报告 database、object store、module pack、GLB pipeline、quality worker、renderer 和 exporter。

### 6.2 P0 环境变量

```text
FORGECAD_LIBRARY_ROOT
FORGECAD_MIGRATIONS_DIR
FORGECAD_AGENT_PORT
FORGECAD_LLM_PROVIDER
FORGECAD_LLM_BASE_URL
FORGECAD_LLM_MODEL
FORGECAD_LLM_API_KEY / _FILE
FORGECAD_RENDER_PROVIDER
FORGECAD_WORKER_TIMEOUT_SECONDS
FORGECAD_MAX_TRIANGLES
FORGECAD_WEAPON_PACK_ROOT
```

兼容期优先读取 `FORGECAD_*`，回退 `WUSHEN_*` 并打印弃用警告；不得永久双写。

### 6.3 P0 最小验收

```text
创建 Weapon Concept 项目
→ 输入“寒地巡逻 S1”Brief
→ 生成并确认 WeaponConceptSpec
→ 从 8–12 个模块生成两个方案
→ 使用连接器组合 GLB
→ 以 DesignChangeSet 修改并保护锁定核心
→ 运行 Graph / Mesh / Assembly 检查
→ 导出 GLB + PNG + Manifest + Report
→ 重启桌面并恢复项目、版本和 Job
```

## 7. P0 故障处置

### 模块包无法加载

1. 检查 pack manifest 的 schema version；
2. 校验每个 GLB 的对象键和 SHA-256；
3. 确认 module id、category 和 connector id 唯一；
4. 将 pack 标记为 unavailable，不静默跳过损坏模块；
5. UI 显示缺失模块，不用相似资产自动替换已确认版本。

### 连接器不匹配或模块浮空

- 禁止提交为已确认版本；
- 报告 node、两端 connector type 和实测变换；
- 允许返回编辑状态修复；
- AI 只能提出重连 ChangeSet，不能绕过验证。

### GLB 组合或回读失败

- artifact 保留但标记 `validation_failed`；
- 不进入正式导出包；
- 记录源模块哈希、组合器版本和错误节点；
- 不用 PNG 成功掩盖 GLB 失败。

### 模型检查不可用

- UI 区分 `not_run`、`failed`、`warning` 和 `passed`；
- 核心 Graph 检查不可用时禁止正式导出；
- 非关键渲染检查可以降级，但报告必须写明未运行项。

### Job 卡住或重启未恢复

- 查询 `/api/v1/jobs/{job_id}` 与 events；
- 检查最后成功 step、attempt 和 heartbeat；
- 检查 Agent 日志；
- 保留 library 快照后再做数据修复；
- 恢复逻辑不能重复提交版本或重复登记资产。

## 8. C01–C10 P0 发布门

```text
C01 concept contracts and generated types
C02 database migrations and repositories
C03 module pack integrity and content hashes
C04 connector compatibility and deterministic assembly
C05 viewport selection and GLB composition
C06 DesignChangeSet preview, locks and version commit
C07 Graph / Mesh / Assembly quality truth set
C08 jobs, retry, cancellation and restart recovery
C09 GLB / OBJ / PNG / Manifest / Report exports
C10 packaged desktop E2E on a clean machine
```

发布证据包含命令、退出码、平台、pack/ruleset 版本、失败样本、工件哈希和已知限制。

## 9. Engineering Pack 运行契约（后续）

Engineering Pack 才增加：

```text
forgecad-cad-runtime
build123d / OpenCascade
STEP / 3MF round-trip
DFM rules and optional slicer
```

它拥有独立 readiness、资源限制、校准几何和发布门。Concept P0 的通过结果不能证明工程制造能力。

路线见 [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md)，合同与架构见 [DESIGN.md](DESIGN.md)。
