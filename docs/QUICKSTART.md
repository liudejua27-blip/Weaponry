# ForgeCAD Quickstart

版本：2026-07-13
目标：在开发机上启动当前本机 Alpha 并验证最小 Agent 闭环

ForgeCAD 是轻量通用机械概念 3D Agent。当前首批领域是未来武器概念道具、汽车、飞机和机械臂；几何主要由受限 `box`/`cylinder` ShapeProgram 生成，不需要安装本地神经 3D 模型、CUDA、ComfyUI 或 Blender。

## 1. 安装开发依赖

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"
```

## 2. 启动本机 Tauri 测试版

```bash
script/build_and_run.sh --verify
```

预期结果：

```text
local_tauri_app_running: true
local_agent_healthy: true
agent_mode: local-dev-python
```

`local-dev-python` 表示应用仍依赖开发机 Python。当前 sidecar 是空占位文件，因此这不是可分发安装包。

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

打开 `http://127.0.0.1:1420/#/cad`。浏览器路径不验证 Keychain、Tauri invoke、Rust supervisor 或安装包。

## 4. 最小验证

```bash
npm run agent:check
npm run contracts:types:check
npm run desktop:typecheck
npm run desktop:tauri-check

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
npm run agent:s8-active-design-navigation-smoke
```

`npm run desktop:r3-concept-workbench-smoke` 已验证当前 Agent 核心流程，包括 preview/confirm、质量、撤销/重做、重启恢复和 GLB 导出。它不代表完整并发、原生安装或发布验证已完成。

## 5. 当前用户闭环

```text
明确写出汽车/飞机/机械臂/未来武器概念道具
→ 查看一个临时兼容 3D 结果（仅取 legacy Planner 第一条文本方向；不是 V003）
→ 生成简单完整 blockout
→ 查看分件候选
→ 保存 AgentAssetVersion
→ 对单个部件进行受限比例/材质/组件修改
→ 确认创建子版本
→ 必要时撤销或重做已确认修改
→ 运行轻量检查
→ 在“下载当前设计”中直接下载 Agent GLB，或生成/下载概念 PNG 与概念图包
```

当前不包含转台视频、自由拆分/合并、任意版本历史浏览或 Agent OBJ/MP4/源包导出。部件锁定、隐藏和单独查看属于当前 Agent Snapshot 的受限工作台状态，不是工程装配约束。Agent 下载抽屉只显示“下载 3D 模型 (GLB)”、“生成概念图”和生成后的单图/概念图包动作；当前视图可下载为只含 PNG 与 manifest 的概念图包。它们都是只读预览，不会创建版本，也不含模型源文件或工程资料。

## 6. 下一步阅读

- 零基础测试：[USER_GUIDE.md](USER_GUIDE.md)
- 开发调试：[DEVELOPMENT.md](DEVELOPMENT.md)
- 当前 API：[API.md](API.md)
- 数据真值：[AUTHORITATIVE_STATE.md](AUTHORITATIVE_STATE.md)
- 测试策略：[TEST_STRATEGY.md](TEST_STRATEGY.md)
- 发布阻断：[PRODUCTION_RELEASE_CHECKLIST.md](PRODUCTION_RELEASE_CHECKLIST.md)
- 全部文档入口：[OPERATIONS.md](OPERATIONS.md)
- 文档状态账本：[DOCUMENTATION_STATUS.md](DOCUMENTATION_STATUS.md)
- 后续 Codex 交接：[CODEX_HANDOFF.md](CODEX_HANDOFF.md)
- 原子任务索引：[CODEX_TASK_INDEX.md](CODEX_TASK_INDEX.md)
