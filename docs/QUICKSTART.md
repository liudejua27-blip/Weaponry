# ForgeCAD Quickstart

ForgeCAD 第一阶段是“通用 AI 模块化 3D 平台 + Weapon Concept Pack”，支持未来武器概念、游戏资产、影视道具和非功能展示模型。CAD/DFM Engineering Pack 是后续独立路线。

## 运行当前参考工作台

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"

PYTHONPATH=apps/agent \
WUSHEN_LIBRARY_ROOT="$PWD/WushenForgeLibrary" \
WUSHEN_MIGRATIONS_DIR="$PWD/migrations" \
.venv/bin/python -m uvicorn wushen_agent.main:create_app \
  --factory --host 127.0.0.1 --port 8000
```

另开终端：

```bash
VITE_FORGE_API_BASE_URL=http://127.0.0.1:8000 npm run desktop:dev
```

打开 `http://127.0.0.1:1420/#/cad`。

## 跑当前门禁

```bash
npm run desktop:typecheck
npm run desktop:build
npm run desktop:p0-context-continuity-smoke
npm run r1:gate
npm run r2:contracts-gate
npm run r2:gate
npm run agent:r3-module-pack-smoke
npm run agent:r3-connector-snap-smoke
npm run r3:workbench-gate
```

当前页面已读取真实 Concept Project、Version、ModuleGraph 与 GLB；`r3:workbench-gate` 使用 4 个米制 GLB fixture 验证拖拽候选、ChangeSet 替换、Connector 重定位、Undo/Redo、爆炸视图和重启恢复，并用 9 个最小 GLB 验证 Module Pack 工具链。100 组合成 Connector 样本全部通过，但不代表正式资产已达到 ≥95%。镜像、正式 10–12 个资产、实际 Mesh 检查和 combined GLB/OBJ/PNG 仍按 R3–R5 实施。

开始制作首包前先读 [MODULE_ASSET_GUIDE.md](MODULE_ASSET_GUIDE.md)。资产 CLI 默认只做 dry-run；只有显式传入 `--import` 才会注册到本地 Agent。

具体第一周设计步骤、运行方式和故障处理见 [OPERATIONS.md](OPERATIONS.md)。
