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

当前健康响应仍使用 `service=wushen-agent`。Concept Project、ModuleGraph、ChangeSet、首版实际 Mesh/Assembly QualityRun、概念源包以及 combined GLB/OBJ/MTL、preview/exploded、front/side/top、8 帧 turntable 和 MP4 导出 API 已实现；正式资产渲染性能、真实 DCC round-trip 和完整 R5 检查矩阵尚未实现。

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
npm run r1:create-weapon-gate
npm run r1:generate3d-gate
npm run r1:worker-gate
npm run r1:unity-export-gate
npm run r1:patch-gate
npm run r1:foundation-gate
npm run r1:frontend-composition-gate
```

`r1:create-weapon-gate` 验证创建 Provider 编排；`r1:generate3d-gate` 验证同步/排队入口与 3D Provider；`r1:worker-gate` 固定 claim/lease/dispatch、恢复和 JobAction；`r1:unity-export-gate` 验证 Manifest/ZIP 与包预检；`r1:patch-gate` 验证 mask/manifest、ComfyUI、负例、追加版本和质量报告。`r1:foundation-gate` 汇总上述门、执行 `m6:gate`，并通过 AST 证明 `asset_store.py` 的完整 workflows 已全部迁出、`App.tsx` 只保留应用组合。`r1:frontend-composition-gate` 执行类型检查、生产构建、上下文连续性、运行时交接、深链和 CAD 工作台 E2E。未配置 Unity executable 时仍只证明 preflight，不证明编辑器 batchmode import。

R1 当前完整回归：

```bash
npm run r1:gate
npm run r2:contracts-gate
npm run r2:gate
npm run r3:workbench-gate
npm run r3:change-set-audit-gate
npm run r5:obj-gate
npm run r5:render-gate
npm run r5:multiview-gate
npm run r5:quality-gate
npm run r5:c07-intersection-gate
```

`r1:gate` 聚合后端 foundation 与前端 composition 两组门。`r2:contracts-gate` 只证明首批 Contract 与生成类型；`r2:gate` 进一步证明 Concept 数据、源包，以及 Brief/Variant/Graph validate/QualityRun/Export 的 JobEvent@2 轨迹。`r3:workbench-gate` 导入 10 模块参考 Pack，验证九类/17 Connector/9-node Graph、真实桌面交互和 20 轮 GPU 生命周期；另用 100 组含镜像数学样本验证 Connector。`r5:obj-gate` 验证 OBJ/MTL；`r5:render-gate` 验证透明/爆炸 PNG；`r5:multiview-gate` 验证三个正交视图、8 帧 turntable、render-set ZIP 和单 Export 复用；`r5:presentation-gate` 增加轮廓抗锯齿、软接触阴影、确定性 MP4 与 DCC 可用性预检；`r5:quality-gate` 与 `r5:c07-intersection-gate` 验证实际 GLB Mesh/Assembly、triangle BVH/SAT/containment 和 Finding 点击聚焦。它们仍不证明人工 Blender 最终资产矩阵上的 ≥95%、Tauri GPU profiling、AI 质量、异常间隙、对称/隐藏几何/LOD、照片级渲染或真实 DCC round-trip。

`r3:change-set-audit-gate` 专门验证 migration 0012、逆序 keyset cursor、filter-bound cursor、全文搜索、status/operation 过滤、preview rejected 与 confirm stale diagnostic、24 条桌面加载更多和 Agent 重启回读。`next_cursor` 是 opaque 值，不得解析或跨过滤条件保存复用。

专项 Connector 门：

```bash
npm run assets:module-pack-gate
npm run assets:blender-starter-preflight
npm run agent:r3-connector-snap-smoke
npm run r5:combined-glb-gate
npm run agent:r5-mesh-assembly-quality-smoke
```

该门同时验证 `GET /api/v1/projects/{project_id}/change-sets` 的 replace/mirror 操作时间线与 Agent 重启回读。

Blender starter 的预检只验证 authoring source、三模块稳定 ID 与导出合同是否齐全。真实构建必须显式配置 Blender：

```bash
FORGECAD_BLENDER_EXECUTABLE=/Applications/Blender.app/Contents/MacOS/Blender \
  npm run assets:blender-starter-build
```

未安装时预检返回 `blocked_blender_not_configured`，不得据此声称 `.blend`、Blender GLB 或缩略图已生成。输出默认写入 `output/blender/weapon-concept-v1-starter`，不修改提交中的 reference Pack。

人工修改三份 `.blend` 后，使用只读 re-export，不能重跑 starter build：

```bash
npm run assets:blender-reexport-preflight
FORGECAD_BLENDER_EXECUTABLE=/Applications/Blender.app/Contents/MacOS/Blender \
  npm run assets:blender-reexport
```

实际执行前必须看到 `ready_for_read_only_export`。runner 校验 source 集合/文件头、输出隔离和 Blender；导出脚本读取 `ForgeCADBlenderAuthoring@1` metadata 与 Connector Empty，拒绝未应用 Transform/Modifier、UV/材质/命名漂移；执行后验证 source SHA-256 未变化和输出 Pack 合同。专项静态/负例门为 `npm run assets:blender-authoring-preflight-gate`。

combined GLB 可从 `GET /api/v1/exports/{export_id}/combined.glb` 独立下载，也同时存在于 ZIP 的 `Model/combined.glb`；两者 SHA-256 必须一致。

创建导出时传 `"include_combined_obj": true`，OBJ 和 MTL 会分别写入 `Model/combined.obj`、`Model/combined.mtl`。可通过以下地址独立下载：

```text
GET /api/v1/exports/{export_id}/combined.obj
GET /api/v1/exports/{export_id}/combined.mtl
```

OBJ 坐标单位固定为米，与 combined GLB 一致。单独下载 OBJ 后还应下载同一 Export 的 `combined.mtl`；需要完整来源和哈希时应下载源 ZIP。MTL 只投影基础颜色、透明度、粗糙度近似高光和自发光，不等价于 glTF PBR 材质。

创建导出时传 `"include_render_png": true`，透明预览和爆炸图写入 `Renders/preview.png`、`Renders/exploded.png`。需要 MP4 时同时传 `"include_turntable_video": true`；该选项依赖 FFmpeg，可用 `FORGECAD_FFMPEG_EXECUTABLE` 指定可执行文件：

```text
GET /api/v1/exports/{export_id}/preview.png
GET /api/v1/exports/{export_id}/exploded.png
GET /api/v1/exports/{export_id}/turntable.mp4
```

当前固定 640×640 RGBA8 技术预览，使用确定性轮廓 coverage 和半透明软接触阴影。透明背景是 PNG alpha，不应以查看器显示的黑/白底判断失败；使用 alpha 像素或支持透明棋盘格的查看器确认。exploded 图的临时位移不创建新 Version。它不替代 Blender/Cycles、实时 Three.js 工作室渲染或正式营销图。

同一 Export 还包含：

```text
Renders/views/front.png
Renders/views/side.png
Renders/views/top.png
Renders/turntable/frame-000.png ... frame-007.png
Renders/turntable.mp4
Renders/render-set.zip
```

直接接口为 `/views/{view}.png`、`/turntable/{frame}.png`、`/turntable.mp4` 和 `/renders.zip`。front 从 +Z 看向原点，side 从 +X，top 从 +Y；turntable 绕 Y 轴均匀采样 8 个方向，MP4 固定 8 fps。工作台首次创建完整交付包后，同一 Version 的格式下载复用该 Export；执行新 QualityRun 会清空桌面缓存的最近 Export，下一次下载自动创建包含新报告的包。

真实 DCC 往返必须提供不可变 combined GLB；预检和强制门分别为：

```bash
npm run assets:dcc-roundtrip-preflight
PYTHONPATH=apps/agent .venv/bin/python scripts/check_dcc_roundtrip.py \
  --input-glb /absolute/path/to/combined.glb --require-dcc
```

只有输出 `dcc_roundtrip_validated` 才表示真实导入/再导出通过。`blocked_dcc_not_configured` 只是环境诊断；安装 Blender/Assimp 并设置 `FORGECAD_BLENDER_EXECUTABLE` 或 `FORGECAD_ASSIMP_EXECUTABLE` 后重跑。runner 拒绝覆盖输入和写入提交中的 Module Pack，并比较输入 SHA-256 与往返前后 vertex/triangle count。

实际几何检查使用 `POST /api/v1/versions/{version_id}/quality-runs:inspect`，请求必须带 `Idempotency-Key`：

```bash
curl -X POST "http://127.0.0.1:8000/api/v1/versions/VER_ID/quality-runs:inspect" \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: manual-quality-001' \
  -d '{"client_request_id":"manual-quality-001","ruleset_version":"weapon-concept-geometry/1.1"}'
```

报告状态为 `warning` 时可以继续概念评审，但必须复核 Findings；`assembly.unconnected_triangle_intersection` 的测量值包含 `surface_pairs`、`containment`、`tested_pairs` 和 `capped`。点击桌面 Finding 会选择并聚焦第一个关联节点。`failed` 表示确定性几何或 Connector 门失败。两者都不代表结构强度、制造可行性或使用安全结论。

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

先复制 `docs/examples/module-pack` 模板，并严格执行 [MODULE_ASSET_GUIDE.md](MODULE_ASSET_GUIDE.md)。建议首个正式包制作 10–12 个，确保九个 category 都有覆盖。

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

每次导出先做只读校验：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/concept_module_pack.py \
  "$PWD/assets/module-packs/weapon-concept-v1-reference" --release
```

不要在 dry-run 失败时绕过校验直接调用注册 API。

启动 Agent 后导入仓库参考包：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/concept_module_pack.py \
  "$PWD/assets/module-packs/weapon-concept-v1-reference" \
  --release --api-base-url http://127.0.0.1:8000 --import
```

参考包可运行但不是最终美术；Blender 交接应保留现有 module/asset/connector ID，以内容哈希和 Version 追踪 GLB 更新。

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

自动门已覆盖退化面、开放/非流形边、法线缺失、Connector 5 mm 错位、未连接组件 triangle BVH/SAT 穿插与封闭网格包含，浏览器也验证 Finding 点击聚焦。OBJ/MTL、透明/爆炸 PNG、三正交视图、8 帧 turntable、MP4、轮廓抗锯齿与软阴影已完成技术预览切片；继续补异常间隙、对称、隐藏几何和 LOD 样本、正式资产渲染性能、真实 DCC 往返与 HTML 报告。

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
