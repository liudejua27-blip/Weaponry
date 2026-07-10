# R1 Desktop Frontend Composition Evidence

日期：2026-07-10

范围：证明旧桌面应用已把 route-level 组合、路由监听、任务/选择业务状态、持久化和页面渲染分开，同时保持旧 Weapon 创作链与新 Concept CAD 工作台行为。它不证明 R2–R6、最终 Blender 资产、AI 质量或正式桌面打包已经完成。

## 边界变化

- `App.tsx` 从约 706 行缩为 21 行，只选择 CAD lazy route 或 `LegacyWorkbench`；
- `useAppRouting` 独占 Hash route 状态和唯一 `hashchange` listener；
- `useLegacyAppController` 拥有旧工作台任务恢复、runtime polling、retry/cancel/skip、深链应用和派生交接状态；
- `jobPersistence` 隔离最近任务、桌面通知与浏览器 Notification；
- `assetSelectors` 隔离当前/最近 Version Asset 派生逻辑；
- `LegacyWorkbench` 只组合 AppShell、页面与浮层，不持有本地 state/effect；
- `CadWorkbenchPanel` 与 `Preview3DPanel` 继续动态导入，生产构建保持独立 chunks。

React 拆分遵循独立职责 hook、单一全局 listener、精确 effect/callback 依赖和 route-level dynamic import 原则。413 行 legacy controller 仍可继续按 Job/Selection 分解，但不再污染组合根、持久化或渲染层。

## 结构门

`scripts/smoke_r1_foundation.py` 断言：

- `App.tsx` 不超过 30 行；
- `App.tsx` 不得出现 `useState`、`useEffect`、`localStorage`、`getJob` 或 `setInterval`；
- controller 必须拥有 restore/runtime/retry/cancel；
- routing 必须拥有 parse/write/hashchange；
- legacy render 不得拥有 state/effect/persistence；
- CAD 与 Preview3D 的动态导入必须保留。

当前结构结果：`App.tsx=21`、controller `413`、render `324`、routing `37` 行。

## 自动门

```bash
npm run agent:r1-foundation-smoke
npm run r1:frontend-composition-gate
```

结果：通过。

前端专项门覆盖：

1. TypeScript project references 类型检查；
2. Vite 生产构建和 lazy chunks；
3. 创建 → Patch → Generate-3D → Unity Export → Library 上下文连续性；
4. queued job runtime、handoff card 和 Library 回读；
5. Weapon/Version 与 Job trace 深链恢复；
6. 10 模块 Concept 工作台选择、替换、吸附、镜像、Undo/Redo、质量定位、导出和重启恢复。

Vite 的 `GLTFLoader` 大于 500 kB 告警仍存在；它是 bundle 优化项，不是本次架构或行为门失败。
