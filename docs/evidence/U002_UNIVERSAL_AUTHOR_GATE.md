# FGC-U002 类别开放 author Gate

日期：2026-07-29
状态：PASS（本机脏工作区；未 commit、未联网、未调用收费 Provider）

## 已证明

- 八类开放 `SubjectProfile@1` fixture 可表达开放类别、部件、macro/meso/micro、材质、证据状态、遮挡和不确定性；
- Rust-sealed `UniversalAuthorRequest@1` 绑定真实 Project/Turn/Snapshot、sealed evidence、选择/锁定和 capability manifest hash；
- `VisualFeatureContract@1`、`RepresentationPlan@1` 与 request/profile/feature/capability hash 交叉校验，悬空引用、伪 observed、未知 capability 与 Provider 自报 executable 均 fail closed；
- 验证后的机械臂通过 `author_universal_asset` 降级到现有 `ForgeVisualAuthoringIntent@1`/视觉程序；
- 未具备表示的对象返回 typed limitation，worker、程序、几何、预览和永久版本副作用均为 0；
- 工作台不再显示四领域选择器，limitation 保留已确认活动资产；R3 的版本、Snapshot、质量、GLB 下载和重启恢复回归通过；
- 两处 C111 自动回退已切断，C111/E005 只能由明确 capability 或冻结 fixture 进入。

## 主要 Gate

```bash
npm run agent:u002-universal-author-contract-gate
npm run desktop:u002-universal-author-workbench-smoke
npm run contracts:types:check
npm run agent:check
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
npm run desktop:typecheck
npm run desktop:build
npm run desktop:r3-concept-workbench-smoke
```

focused Rust Gate 另覆盖 protocol `author_context`、Core 合同、app-server 正/负路径和主程序 `cargo check`。

## 没有证明

- 猫、人物、动物、植物、家具、建筑、环境或混合对象已经可生成；
- `UniversalAssetSource`、deformable、local-hybrid 或通用 Appearance Compiler 已实现；
- 任意图片的视觉相似度、照片级质量、真人 `4/5`、生产安装或商业成本已经达标。

这些退出条件分别属于 U003、U004、U005 和发布任务。
