# ForgeCAD 本机开发与调试

版本：2026-07-13
适用对象：桌面、Agent、合同和测试开发者

## 1. 环境要求

- macOS 本机开发路径；
- Node.js 20；
- Python 3.11 优先，项目最低合同为 Python 3.10（Starlette 安全修复要求）；
- Rust 由仓库脚本解析本机 rustup toolchain；
- Chrome 仅用于浏览器 E2E；
- 不需要安装 TripoSR、Stable Fast 3D、Hunyuan3D、ComfyUI、CUDA 或模型权重。

初始化：

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"
```

## 2. 原生 Tauri 路径

主要本机验证命令：

```bash
script/build_and_run.sh --verify
```

成功只证明：

- 当前机器能够完成前端和 Tauri 构建；
- 桌面进程可以启动；
- 本地 Agent 健康检查通过；
- supervisor 当前处于 `local-dev-python`。

### 本机 Agent supervisor

本机 Alpha 由 Rust supervisor 启动仓库内的 Python Agent；它不是对外安装包的一部分。启动时只接受 `GET /api/health` 返回的本机 ForgeCAD Agent，并在错误服务、超时或窗口退出时只停止自己创建的子进程。

- 开发覆盖变量 `WUSHEN_REPO_ROOT`、`WUSHEN_AGENT_PYTHON` 仅用于本机调试；不得进入发布配置；
- `PYTHONPATH`、library root 与 migrations path 只由 supervisor 传入，不从界面或模型输出取得；
- Rust CSP 只允许本机 Agent HTTP 与应用资源；默认 capability 不授予任意文件访问；
- 日志用于本机故障诊断，但不得写入 Provider Key、绝对资产路径或用户内容全文。

`PACKAGING.md` 是 sidecar/二进制/安装包的唯一发布合同；当 `mode=packaged-sidecar` 尚未实际通过 gate 时，不得把本机 supervisor 说成可分发产品。

它不证明 packaged sidecar、独立安装、签名、公证或其他机器可运行。

Rust 静态检查：

```bash
npm run desktop:tauri-check
```

工作台行为基线（前端拆分前必须先跑）：

```bash
npm run desktop:f001-workbench-characterization
npm run desktop:f002-agent-conversation-smoke
npm run desktop:f003-agent-selection-card-smoke
npm run desktop:f004-workbench-drawers-smoke
npm run desktop:f006-accessibility-smoke
npm run desktop:f007-workbench-lifecycle-smoke
npm run desktop:f008-agent-conversation-state-smoke
npm run desktop:f009-agent-blockout-display-state-smoke
npm run desktop:f010-agent-asset-workspace-state-smoke
npm run desktop:f011-legacy-compatibility-display-smoke
npm run desktop:f012-component-library-preferences-smoke
npm run desktop:f013-viewport-display-preferences-smoke
npm run desktop:f014-legacy-module-graph-workspace-smoke
npm run desktop:f015-legacy-module-graph-overlay-smoke
npm run desktop:f016-agent-render-presentation-smoke
npm run desktop:t002-workbench-e2e-scenarios
npm run desktop:d3-domain-clarification-smoke
npm run desktop:r3-concept-workbench-smoke
```

F001 已加入 `.github/workflows/forgecad-core.yml`，并在本机 Chrome 通过；它验证了项目加载、legacy 显式转换、澄清、预览不写盘、Agent 提交、Snapshot/导出对齐、重启恢复和单 WebGL canvas。F006 accessibility smoke 已加入 desktop job，覆盖静态尺寸/字号、按钮类型、aria 语义、dialog 初始焦点和键盘 Escape/焦点返回；F007–F016 的回归边界见任务索引；F017 edit-assist-presentation smoke 覆盖当前 asset/part 的候选和建议过滤、context 切换清空、迟到成功/失败拒绝及失败不伪造建议，并断言展示状态不含 Snapshot、质量、ChangeSet、导出、asset head 或 renderer。T002 场景套件在 workbench-e2e job 中输出 12 个独立报告；T003 在同一 job 中输出 `output/playwright/fgt003-performance.json`，验证单 canvas/context、重复抽屉/重载、GC 后内存、几何/纹理和 bundle 阈值；CI runner 的结果仍以对应 commit 为准。

## 3. 浏览器开发预览

终端一：

```bash
PYTHONPATH=apps/agent \
WUSHEN_LIBRARY_ROOT="$PWD/WushenForgeLibrary" \
WUSHEN_MIGRATIONS_DIR="$PWD/migrations" \
.venv/bin/python -m uvicorn wushen_agent.main:create_app \
  --factory --host 127.0.0.1 --port 8000
```

终端二：

```bash
VITE_FORGE_API_BASE_URL=http://127.0.0.1:8000 npm run desktop:dev
```

打开 `http://127.0.0.1:1420/#/cad`。浏览器路径只用于前端调试，不验证 Tauri invoke、Keychain、Rust supervisor 或安装包。

## 4. Provider 配置

原生 Tauri 应通过工作台 Provider 弹窗把 Key 保存到 macOS Keychain。浏览器调试使用只读 secret file：

```bash
umask 077
mkdir -p "$HOME/.config/forgecad"
printf '%s' '<API_KEY>' > "$HOME/.config/forgecad/provider.key"

export FORGECAD_AGENT_PROVIDER=openai_compatible
export FORGECAD_AGENT_BASE_URL=https://api.deepseek.com
export FORGECAD_AGENT_MODEL=deepseek-v4-pro
export FORGECAD_AGENT_API_KEY_FILE="$HOME/.config/forgecad/provider.key"
```

不要把 API Key 放入 shell history、`.env`、SQLite、测试 fixture、日志或截图。真实调用必须由操作者显式执行；默认 smoke 使用确定性或本机 fake Provider。

保存配置后不等于已经调用 DeepSeek。原生工作台会依次显示 metadata、Keychain、受管 supervisor 与 Agent capability 四段 preflight；全部就绪后，普通 Turn 或“测试连接（会联网）”才可能发起真实请求。连接测试可取消，Provider 失败不会自动重试或静默切换为离线 Planner。浏览器调试没有 Tauri/Keychain preflight，只适合使用上面的权限受限 secret file 验证 Agent 端合同。

## 5. 当前核心验证

快速静态验证：

```bash
npm run agent:check
npm run contracts:types:check
npm run desktop:typecheck
npm run desktop:build
npm run desktop:tauri-check
```

通用机械 Agent 纵向切片：

```bash
npm run agent:unit
npm run agent:g1-kernel-smoke
npm run agent:g2-contracts-smoke
npm run agent:g3-shape-program-smoke
npm run agent:g4-mechanical-planner-smoke
npm run agent:a003-provider-gateway-smoke
npm run agent:g5-geometry-worker-smoke
npm run agent:g801-shape-primitive-smoke
npm run agent:g802-profile-extrude-smoke
npm run agent:g803-revolve-smoke
npm run agent:g804-transform-arrays-smoke
npm run agent:g805-boolean-smoke
npm run agent:g806-bevel-surface-panel-smoke
npm run desktop:a003-provider-connection-smoke
npm run agent:g807-blockout-diversity-smoke
npm run agent:g6-segmentation-smoke
npm run agent:g6-material-catalog-smoke
npm run agent:g6-asset-editing-smoke
npm run agent:g6-component-registry-smoke
npm run agent:g7-external-glb-import-smoke
```

当前桌面主流程：

```bash
npm run desktop:r3-concept-workbench-smoke
```

Rust 原生单元测试：

```bash
script/with_rust_toolchain.sh cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

依赖审计在 CI `dependency-audits` job 中执行并上传 `dependency-audit-reports`；本机可复现：

```bash
npm audit --audit-level=high --json
.venv/bin/pip-audit -r apps/agent/requirements-release.lock --format=json
cargo audit --file apps/desktop/src-tauri/Cargo.lock --json
```

任何高危结果都保持失败。2026-07-13 升级 FastAPI 0.139.0 / Starlette 1.3.1 后，本机 Python 审计为 0 vulnerabilities；Rust 审计为 0 vulnerabilities。上游版本变化后必须重新执行。

截至 2026-07-13，该 E2E 已通过 Agent-first 路径：同一项目先导入参考 GLB v1，生成可编辑资产从 v2 开始，确认/回退/重做后恢复到 v5。测试同时确认 Agent 激活时旧 ModuleGraph 替换入口被禁用；R005 还验证 Agent 下载抽屉直接下载 GLB 和概念图包，不显示旧用途/OBJ/源包。多客户端并发、原生安装和发布仍不在该门内。

### 5.1 R005 本机 Tauri 下载验收

先构建并启动不含真实 Provider 调用的本机开发包：

```bash
FORGECAD_LOCAL_VISUAL_PACK=0 ./script/build_and_run.sh --verify
```

该命令在 2026-07-13 已验证 `.app` 进程和 `local-dev-python` Agent 健康检查。浏览器 E2E 已验证 Agent 下载抽屉直接下载 GLB 和指纹受限的概念图包；原生 WebView 的真实点击仍需在有 macOS 辅助功能权限的会话中手工完成。若用 `osascript -l JavaScript` 自动化，先在“系统设置 → 隐私与安全性 → 辅助功能”允许运行测试的 Codex 或终端；未授权时记录 `osascript 不允许辅助访问`，不要把它伪装成下载通过。验收时依次点击“下载 3D 模型 (GLB)”、生成后下载一张 PNG、下载概念图包，并确认应用内版本、选择和质量没有变化，主视图仍只有一个 canvas。

## 6. 合同变更流程

1. 修改 `packages/concept-spec/schemas/`；
2. 修改对应 Pydantic 模型和语义校验；
3. 运行 `npm run contracts:types:generate`；
4. 检查生成差异；
5. 运行 `npm run contracts:types:check`；
6. 增加正向、非法字段、非法引用和预算失败测试；
7. 更新 [API.md](API.md) 和 [AUTHORITATIVE_STATE.md](AUTHORITATIVE_STATE.md)。

不得手工修改生成文件来掩盖 Schema 漂移。

## 7. 数据库与本机数据

默认开发库：

```text
WushenForgeLibrary/library.db
WushenForgeLibrary/objects/
```

迁移目录由 `WUSHEN_MIGRATIONS_DIR` 指定。测试应使用临时 Library，不得覆盖用户的真实本地库。涉及迁移、版本头或对象引用时，先运行备份/恢复 smoke：

```bash
npm run agent:r3-library-backup-restore-smoke
npm run agent:r3-library-recovery-drill-smoke
```

## 8. 调试信息最小集

问题报告应包含：

- commit/branch 和工作区是否干净；
- Tauri 或浏览器运行方式；
- Agent 模式和 `/api/health`；
- 项目 ID、Agent Thread ID、Turn ID、AgentAssetVersion ID；
- 输入、预期、实际和最短复现步骤；
- 失败命令与完整错误码；
- 不含密钥和绝对私有路径的日志片段。

数据恢复和损坏处理见 [DISASTER_RECOVERY.md](DISASTER_RECOVERY.md)。

## 9. 何时使用插件、Skill 和 GitHub 参考

开发工具选择以 [插件与 Skill 操作设计](AGENT_PLUGINS_SKILLS_DESIGN.md) 为准：GitHub 核验用 `@github`，零基础流程审查用 `@product-design`，React 拆分用 `build-web-apps:react-best-practices`，工作台回归用前端测试/Playwright Skill，GLB 管线用 `game-studio:web-3d-asset-pipeline`。

候选开源项目以 [GitHub 参考架构](AGENT_GITHUB_REFERENCE_ARCHITECTURE.md) 为准。不要直接 clone 到仓库或复制源码；先写小型 spike/benchmark，比较体积、内存、冷启动、确定性、许可证和 macOS/Windows 打包，再决定是否更新 lock 与 SBOM。
