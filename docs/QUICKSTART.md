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
```

当前页面是产品方向和交互参考，Three.js 程序化模型不是最终 GLB 模块资产；Concept API、连接器、ChangeSet、质量检查和导出仍按 R2–R6 实施。

具体第一周设计步骤、运行方式和故障处理见 [OPERATIONS.md](OPERATIONS.md)。
