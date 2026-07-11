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
npm run r1:frontend-composition-gate
npm run r1:gate
npm run r2:contracts-gate
npm run r2:gate
npm run assets:module-pack-gate
npm run assets:blender-authoring-preflight-gate
npm run agent:r3-connector-snap-smoke
npm run r3:workbench-gate
npm run r3:change-set-audit-gate
npm run r3:library-backup-gate
npm run r4:planner-gate
npm run agent:r4-evaluation-baseline
npm run r5:combined-glb-gate
npm run r5:obj-gate
npm run r5:render-gate
npm run r5:multiview-gate
npm run r5:presentation-gate
npm run r5:quality-gate
npm run r5:c07-intersection-gate
npm run r5:c07-localization-gate
npm run r5:c07-policy-gate
npm run release:safety-scope
npm run release:secrets-files
npm run release:prompt-quality
npm run release:docs-walkthrough
npm run release:packaging-readiness
npm run release:license-sbom
npm run release:gate
```

当前页面已读取真实 Concept Project、Version、ModuleGraph、ChangeSet 时间线与 GLB；`r3:workbench-gate` 验证 10 模块参考 Pack、替换/吸附/镜像、Undo/Redo、爆炸视图和视口生命周期；`r3:change-set-audit-gate` 还验证当前筛选的 JSONL/CSV + hash Manifest ZIP、`project_lifetime`、桌面下载与重启回读；`r3:library-backup-gate` 验证 SQLite Backup API、引用对象/容量 Manifest、tamper/overwrite/secret negatives、隔离恢复，以及 10 模块参考库的多轮耗时/容量报告和全部 Module hash 回读；`r4:planner-gate` 验证 Brief/Module/Change Planner、半透明 ghost 与显式 confirm 链。它们不代表真实模型 AI 指标、法规级 WORM/legal hold、加密异地备份或正式资产规模性能已达标。R5 各门继续验证 GLB/OBJ/PNG/MP4、展示交付和 `weapon-concept-geometry/1.3`。

正式备份/验证/恢复/演练命令只在 [OPERATIONS.md](OPERATIONS.md) 维护；Quickstart 不复制完整流程。

`agent:r4-evaluation-baseline` 验证固定 20/20/20/20 truth set 和评测器，但仍不是 AI 证据。配置真实 OpenAI-compatible Provider 后，只有操作者明确运行 `npm run agent:r4-evaluation-live`，完整 80 次调用、token usage 和全部阈值通过，报告才可能标记为真实 Provider 证据；具体成本和失败码见 [OPERATIONS.md](OPERATIONS.md)。

MP4 依赖 `ffmpeg`。如果不在 PATH，可设置 `FORGECAD_FFMPEG_EXECUTABLE`。DCC 环境先运行 `npm run assets:dcc-roundtrip-preflight`；只有带 `--input-glb` 的检查返回 `dcc_roundtrip_validated` 才能声称 Blender/Assimp 往返已通过。当前证据已覆盖 starter core、工作台组合后的 visual-v2 三模块与 10 模块 reference combined GLB；它们不代表正式美术资产验收。

开始制作首包前先读 [MODULE_ASSET_GUIDE.md](MODULE_ASSET_GUIDE.md) 和 [MODULE_NAMING_STANDARD.md](MODULE_NAMING_STANDARD.md)。资产 CLI 默认只做 dry-run；只有显式传入 `--import` 才会注册到本地 Agent。

Blender re-export 通过不等于正式资产；`FormalModuleReview@1` 草稿、独立人工审批和晋级报告命令只在 [OPERATIONS.md](OPERATIONS.md) 维护。

仓库自带 10 模块参考包。启动 Agent 后可直接导入：

```bash
PYTHONPATH=apps/agent .venv/bin/python scripts/concept_module_pack.py \
  "$PWD/assets/module-packs/weapon-concept-v1-reference" \
  --release --api-base-url http://127.0.0.1:8000 --import
```

该包用于立即开始工作台设计和 Blender 交接，不代表最终人工资产质量。

具体第一周设计步骤、运行方式和故障处理见 [OPERATIONS.md](OPERATIONS.md)。
