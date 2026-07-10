# ForgeCAD 操作与运行手册

本文区分两种状态：

- **A. 当前可运行基线**：仓库今天真实存在的武神 Forge / Weapon / Unity 代码，用于冻结、回归和迁移开发。
- **B. 目标 ForgeCAD Runtime**：CAD/DFM 重构后的运行契约，目前是实施要求，不代表已经可运行。

不要用 A 的成功结果证明 CAD、STEP、3MF 或 DFM 已经完成。

## 1. 当前可运行基线

### 1.1 环境要求

必需：

- Node.js 20 或更高；
- npm 10 或更高；
- Python 3.9 或更高。

可选：

- Rust + Cargo：编译或运行 Tauri 桌面壳；
- Chrome：运行现有浏览器 UI smoke；
- ComfyUI：旧概念图 Provider；
- 旧本地 3D Runtime：旧神经 3D Provider；
- Unity：只用于旧 Unity release gate。

CAD 重构后还会需要 build123d/OpenCascade、lib3mf，以及可选 PrusaSlicer；它们目前不在 `apps/agent/pyproject.toml` 中。

### 1.2 安装

在仓库根目录执行：

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"
```

检查版本：

```bash
node --version
npm --version
.venv/bin/python --version
```

### 1.3 启动本地 Agent

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

预期健康响应应标识 `service=wushen-agent` 和 `status=ok`。如果 8000 端口上是别的服务，Tauri supervisor 会报告 `wrong_service`，不会把它当成当前 Agent。

### 1.4 运行前端开发壳

另开终端：

```bash
VITE_FORGE_API_BASE_URL=http://127.0.0.1:8000 npm run desktop:dev
```

这会打开或提供 `http://127.0.0.1:5173`。它只是 Vite 浏览器开发壳，不是最终桌面交付形态；浏览器中不能使用 Tauri invoke 和本地 supervisor 能力。

### 1.5 运行 Tauri 本地桌面窗口

安装 Rust/Cargo 后执行：

```bash
npm --workspace apps/desktop run tauri -- dev
```

当前 Tauri 开发 supervisor 会尝试启动：

```text
.venv/bin/python -m uvicorn wushen_agent.main:create_app
```

并设置仓库内 `WushenForgeLibrary` 与 `migrations` 路径。它属于开发期 Python 进程管理，不是已完成的生产 sidecar 打包。

常用覆盖项：

```bash
export WUSHEN_REPO_ROOT=/absolute/path/to/repo
export WUSHEN_AGENT_PYTHON=/absolute/path/to/python
```

开发 supervisor 日志位于仓库根目录 `.wushen-agent.log`。

## 2. 当前验证命令

### 2.1 最小静态检查

```bash
npm run agent:check
npm run contracts:check
npm run contracts:types:check
npm run desktop:typecheck
npm run desktop:build
```

Schema 或 Pydantic/OpenAPI 模型发生变化后先生成：

```bash
npm run contracts:types:generate
```

然后重新运行 `contracts:types:check`。生成物不应手工编辑。

### 2.2 当前最高层领域回归

```bash
npm run m6:gate
```

它验证迁移前的 CreativeWeaponGraph/SkillGraph 切片和桌面类型，不验证 ForgeCAD CAD/DFM 能力。

R1 基础设施切片使用：

```bash
npm run r1:foundation-gate
```

该门先验证 migration 幂等、SQLite 连接约束、内容寻址去重、路径越界和 hash 篡改检测，再执行完整 `m6:gate`。

R1 当前完整回归使用：

```bash
npm run r1:gate
```

它在基础设施门后继续执行桌面生产构建和上下文连续性 UI smoke。

### 2.3 Rust/Tauri 编译检查

```bash
npm run desktop:tauri-check
```

没有 Cargo 时应诚实记录为环境阻塞，不能据此声称桌面打包可用。

### 2.4 旧发布门的处理

`npm run release:gate` 当前仍检查旧的虚构武器安全文案、ComfyUI、Unity import 和旧打包条件。产品转向后：

- 它只保留为 legacy baseline 证据；
- 它不是 ForgeCAD release gate；
- 在 C01–C10 新门落地前，不得声称新产品达到发布条件。

## 3. 当前数据与备份

默认库：

```text
WushenForgeLibrary/
  library.db
  library.db-wal
  library.db-shm
  objects/sha256/
```

数据库与对象目录共同构成资产库，不能只备份 `library.db` 而丢失 `objects/sha256`。

### 3.1 安全备份

1. 停止 Agent 和 Tauri 管理的子进程。
2. 确认没有 Uvicorn/worker 正在写库。
3. 复制整个 `WushenForgeLibrary` 目录到带时间戳的备份位置。
4. 校验备份包含数据库、WAL/SHM（如果仍存在）和对象目录。

不要在进程写入期间直接打包活动数据库。正式迁移工具应使用 SQLite backup API 或一致性快照。

### 3.2 使用临时库做测试

为了避免污染真实资产：

```bash
export WUSHEN_LIBRARY_ROOT="$PWD/.tmp/dev-library"
export WUSHEN_MIGRATIONS_DIR="$PWD/migrations"
```

测试结束前先确认该路径确实是临时路径，再删除。不要把用户资产库作为 smoke 输入。

### 3.3 数据重置

当前没有“安全清空生产库”的统一命令。开发期需要重置时：

1. 停止所有进程；
2. 仅对明确的临时 `WUSHEN_LIBRARY_ROOT` 操作；
3. 保留失败样本或先备份；
4. 重新启动，让 migration runner 创建新库。

本文不提供递归删除命令，避免误删真实资产。

## 4. 当前 Provider 配置

默认 Provider 是 mock，最适合做迁移前回归。

OpenAI-compatible 旧 LLM Adapter：

```bash
export WUSHEN_LLM_PROVIDER=openai_compatible
export WUSHEN_LLM_BASE_URL=https://api.openai.com/v1
export WUSHEN_LLM_MODEL=<model-name>
export WUSHEN_LLM_API_KEY=<secret>
```

密钥只能来自环境变量或 secret file。不得写入源码、日志、Job event、资产、导出包或文档示例。

旧 ComfyUI 与神经 3D 配置见历史文档；它们在目标架构中只保留为概念参考 Provider，不参与权威 CAD 构建。

## 5. 常见故障

### Agent 无法启动

检查：

```bash
.venv/bin/python -c "import fastapi, pydantic, uvicorn"
test -d migrations
lsof -nP -iTCP:8000 -sTCP:LISTEN
```

典型原因：虚拟环境未安装、工作目录错误、migration 路径错误、8000 被占用或数据库无写权限。

### 浏览器页面能打开，但桌面能力不可用

原因通常是运行了 Vite 壳。改用：

```bash
npm --workspace apps/desktop run tauri -- dev
```

并确认 Rust/Cargo 与 Tauri 构建条件齐全。

### Tauri 显示 wrong_service

端口 8000 已被其他服务占用。先确认进程归属，再停止正确的进程或修改开发配置。不要让 supervisor 杀死它没有启动的服务。

### 合同检查失败

如果改动了 Schema 或 API 模型：

```bash
npm run contracts:types:generate
npm run contracts:check
npm run contracts:types:check
```

如果生成后仍漂移，检查 Schema registry、Pydantic DTO 和 OpenAPI 是否代表同一合同。

### Job 卡住或重启后未恢复

- 查询 `/api/jobs/{job_id}` 与 `/api/jobs/{job_id}/runtime`；
- 读取 `/api/jobs/{job_id}/events`，确认最后成功的 step；
- 检查 `.wushen-agent.log` 与 Agent stderr；
- 确认 `WUSHEN_RECOVER_ON_STARTUP` 未被设为 `0`；
- 保留 library 快照后再做手工数据修复。

## 6. 目标 ForgeCAD Runtime 契约（尚未实现）

本节是 R1–R6 的操作验收要求。

### 6.1 目标进程

```text
Tauri Desktop
├─ forgecad-agent sidecar      127.0.0.1:8000
└─ forgecad-cad-runtime        local IPC or loopback-only port
```

CAD Runtime 默认无外网，使用一次性工作目录，并限制 CPU、内存、时间、Feature 数和输出大小。Agent 健康不能替代 CAD Runtime readiness。

建议健康端点：

```http
GET /api/v1/health
GET /api/v1/readiness
GET /api/v1/runtime/cad/health
GET /api/v1/runtime/slicer/health
```

`readiness` 应分别报告数据库、对象存储、CAD 内核、导出器和可选切片器，不能只有一个总布尔值。

### 6.2 目标环境变量

```text
FORGECAD_LIBRARY_ROOT
FORGECAD_MIGRATIONS_DIR
FORGECAD_AGENT_PORT
FORGECAD_CAD_RUNTIME_URL
FORGECAD_CAD_RUNTIME_TIMEOUT_SECONDS
FORGECAD_LLM_PROVIDER
FORGECAD_LLM_BASE_URL
FORGECAD_LLM_MODEL
FORGECAD_LLM_API_KEY / _FILE
FORGECAD_RENDER_PROVIDER
FORGECAD_SLICER_MODE=disabled|user_installed_cli|managed_external_runtime
FORGECAD_PRUSASLICER_EXECUTABLE
```

兼容期允许优先读取 `FORGECAD_*`、回退 `WUSHEN_*`，但必须打印弃用警告，且不得永久双写配置。

### 6.3 目标最小验收

```text
创建 L 型支架
→ 回答 blocker
→ 构建 B-Rep
→ STEP/3MF 回读
→ 查看 GLB 和拓扑映射
→ 运行 DFM
→ 修改厚度并保护孔距
→ 创建子版本
→ 导出制造包
→ 重启桌面并恢复项目/Job 历史
```

### 6.4 目标运行事件

每个耗时任务至少记录：

- job/step/attempt；
- build/version/design id；
- input hash；
- runtime/compiler/kernel version；
- timeout、cancel 和 retry 原因；
- artifact id、sha256 与 validation status；
- 用户可读消息和机器可读 error code。

日志中禁止出现 API key、用户文件原始绝对路径或未清洗的 Provider 响应。

## 7. 目标故障处置

### CAD Runtime 崩溃或超时

1. Worker 标记当前 attempt 失败，不提交成功版本。
2. 回收子进程和临时目录。
3. 保存结构化 compiler diagnostics 和资源统计。
4. 可恢复错误按策略最多重试 2–3 次。
5. 超过上限后返回失败 feature、局部测量和建议参数。

### STEP 或 3MF 回读失败

- artifact 保留但标记 `validation_failed`；
- 禁止进入生产导出包；
- 记录 exporter、内核、格式版本和 hash；
- 不用 STL 成功掩盖 STEP/3MF 失败。

### DFM 服务不可用

- 几何验证失败时始终阻断生产导出；
- 可选切片器不可用可以降级；
- 核心 DFM ruleset 不可用时禁止将输出标记为 manufacturing-ready；
- UI 必须区分“未检查”“检查失败”和“检查通过”。

### 数据迁移失败

- 事务回滚；
- 原库保持只读不变；
- importer 生成逐记录报告；
- 不自动把 CreativeWeaponGraph 转成 FeatureGraph；
- 修复 importer 后用相同输入可重复运行。

## 8. ForgeCAD 发布检查（规划）

发布候选必须依次通过：

```text
C01 contracts
C02 database
C03 templates
C04 geometry regression
C05 STEP/3MF round-trip
C06 ChangeSet / locked interfaces
C07 DFM truth set
C08 jobs / recovery
C09 sandbox
C10 desktop E2E
dependency license + SBOM
packaged sidecar launch
clean-machine install and uninstall
```

发布证据至少包含：测试命令、退出码、runtime 版本、平台、样本集版本、失败归档、工件 hash 和已知限制。

路线和门禁定义见 [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md)，架构与合同见 [DESIGN.md](DESIGN.md)。
