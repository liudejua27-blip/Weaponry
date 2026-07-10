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
npm run r1:create-weapon-gate
npm run r1:generate3d-gate
npm run r1:worker-gate
npm run r1:unity-export-gate
npm run r1:patch-gate
npm run r1:gate
npm run r2:contracts-gate
npm run r2:gate
npm run agent:r3-module-pack-smoke
npm run agent:r3-connector-snap-smoke
npm run r3:workbench-gate
npm run r5:combined-glb-gate
npm run r5:obj-gate
npm run r5:render-gate
npm run r5:multiview-gate
npm run r5:quality-gate
npm run r5:c07-intersection-gate
```

当前页面已读取真实 Concept Project、Version、ModuleGraph、ChangeSet 时间线与 GLB；`r3:workbench-gate` 导入 10 模块参考 Pack，验证九类资产、17 Connector、9-node Graph、替换/吸附/镜像、操作时间线、Undo/Redo、爆炸视图、20 轮加载/卸载和重启恢复。`r5:combined-glb-gate` 验证单一 GLB；`r5:obj-gate` 验证 OBJ/MTL；`r5:render-gate` 验证透明/爆炸 PNG；`r5:multiview-gate` 验证 front/side/top、8 帧 turntable 和 render-set ZIP；`r5:c07-intersection-gate` 验证实际 Mesh/Assembly、triangle BVH/SAT/containment、Finding 点击聚焦与重启恢复。Tauri GPU profiling、异常间隙/对称/隐藏几何/LOD、转台视频和 DCC round-trip 仍按 R3–R5 实施。

开始制作首包前先读 [MODULE_ASSET_GUIDE.md](MODULE_ASSET_GUIDE.md)。资产 CLI 默认只做 dry-run；只有显式传入 `--import` 才会注册到本地 Agent。

仓库自带 10 模块参考包。启动 Agent 后可直接导入：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/concept_module_pack.py \
  "$PWD/assets/module-packs/weapon-concept-v1-reference" \
  --release --api-base-url http://127.0.0.1:8000 --import
```

该包用于立即开始工作台设计和 Blender 交接，不代表最终人工资产质量。

具体第一周设计步骤、运行方式和故障处理见 [OPERATIONS.md](OPERATIONS.md)。
