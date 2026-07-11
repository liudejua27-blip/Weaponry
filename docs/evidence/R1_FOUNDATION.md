# R1 通用基础设施证据

日期：2026-07-10
状态：当前 R1 退出边界完成；后端完整 workflows 已迁出 facade，前端 `App.tsx` 已成为 21 行组合根。

## 已完成切片

后端：

- 建立 `forgecad_agent` 新包并修正 editable package discovery；
- 提取 `SQLiteConnectionFactory`；
- 提取幂等 `SQLiteMigrationRunner`；
- 提取带路径约束和 SHA-256 校验的 `ContentAddressedStore`；
- 提取 Idempotency、Asset、Job 和 Checkpoint Repository；
- 建立 transaction-scoped `SQLiteUnitOfWork`；
- 旧 `SQLiteAssetStore` 作为 Facade 使用新基础设施；
- 旧资产裸读取统一改为内容寻址存储读取；
- 将 Job detail/list/action/runtime/event 查询迁入 `LegacyJobQueryService`，并通过 Repository 统一读取；
- 将 Job cancel/retry/retry-from-step 事务迁入 `LegacyJobCommandService`，包括 action audit、event、checkpoint 与 Provider cancel 协调；
- 将 mask/manifest 上传验证、幂等与内容寻址写入迁入 `LegacyAssetUploadService`；所有通用资产 INSERT 经 `AssetRepository.add`；
- 将版本激活、武器库 read model、资产元数据和安全文件解析迁入 `LegacyLibraryService`；
- 将 interpretation/recast confirm/creative graph 工作流迁入 `LegacyCreativeRecastService`；
- 将旧同步 Create Weapon 的 LLM → Image → 3D → Quality → JobEvent 编排迁入 `LegacyCreateWeaponService`；`SQLiteAssetStore.create_weapon` 只保留代理与幂等异常映射；
- 将 Generate-3D 的 runtime 选择、同步执行与排队事务迁入 `LegacyGenerate3DService`；worker claim/poll/commit 随后继续迁入下一条 service；
- 将 worker claim/lease/dispatch、Generate-3D draft/submit/poll/fetch/cancel/commit 迁入 `LegacyWorkerService`；Unity worker handler 作为注入端口保持兼容；
- 将 Unity Export 的 sync/queue/worker、输入验证、Manifest 与 ZIP builder 迁入 `LegacyUnityExportService`，Worker Runtime 直接注入 export handler；
- 将 Patch 的 mask/manifest、Image Provider、质量报告、ProviderTask、Checkpoint 与 child Version 编排迁入 `LegacyPatchService`；删除未调用的 mock SVG helper；
- `asset_store.py` 从约 3608 行降至 1449 行；10 个 workflow facade 方法最长 20 行，结构 smoke 禁止 Provider/worker/export/patch 编排重新回流；
- 提取 FastAPI settings/CORS/base app factory，并兼容 `FORGECAD_CORS_ORIGINS`。
- 将 legacy asset、job、system、weapon routes 和错误映射拆出 `main.py`；
- `main.py` 从约 458 行降至约 54 行，只保留应用组装和 worker 生命周期。

前端：

- 提取 Hash router；
- 提取 RuntimeProvider；
- RuntimeProvider 统一拥有 API base URL、健康状态和 Agent supervisor 生命周期。
- 提取 JobEventProvider，统一事件合并、游标、SSE 和 stream status；
- 提取 SelectionProvider，统一当前 weapon/version/detail；
- 提取 AppShell，统一导航和桌面顶栏。
- 将旧工作台任务恢复、轮询、命令和派生状态迁入 `useLegacyAppController`；
- 将 Hash route/listener、最近任务与桌面通知持久化、资产选择器分别迁入 `useAppRouting`、`jobPersistence` 和 `assetSelectors`；
- 将旧页面 JSX 迁入无本地 state/effect 的 `LegacyWorkbench`，保留 `Preview3DPanel` 动态导入；
- `App.tsx` 从约 706 行降至 21 行，只组合 controller、legacy workbench 和独立 lazy CAD route；
- 新增懒加载 `#/cad` 工作台：九区布局、参数化武器 Three.js 视口、组件分类/选择、视图工具、DFM 与导出状态；
- 使用 Phosphor 统一图标并补充 license ledger；
- 完成 1536 × 1024 参考图对照与交互 QA，证据见仓库根目录 `design-qa.md`。

## 验证结果

```bash
.venv/bin/ruff check \
  apps/agent/forgecad_agent \
  scripts/smoke_r1_foundation.py \
  apps/agent/wushen_agent/main.py \
  apps/agent/wushen_agent/asset_store.py
```

结果：通过。

```bash
npm run r1:foundation-gate
```

结果：通过。

覆盖：

- 12 个 migrations 在新库首次应用；
- 第二次 migration 无重复应用；
- foreign keys 开启，busy timeout 为 5000 ms；
- 相同 payload 使用同一个内容寻址路径；
- `../` 路径被拒绝；
- 文件被篡改后 SHA-256 校验失败；
- ForgeCAD CORS 新变量与默认本地 origin 生效；
- contract、generated artifacts、Python compile、M6 smoke、desktop typecheck 全部通过。

```bash
npm run desktop:build
```

结果：通过。Vite 报告 `GLTFLoader` chunk 超过 500 kB；CAD 与 Preview3D 动态导入仍独立成块，该告警是后续 bundle 优化项，不影响本次 R1 correctness。

```bash
npm run desktop:p0-context-continuity-smoke
```

结果：通过。旧创建、Patch、3D、Unity、版本切换和资产库同步链路在 Provider/AppShell 重构后保持连续。

应用内浏览器验证 `#/cad`：模式、工具、组件分类/选择、参数输入、AI 指令和导出格式均产生可观察状态变化；console error/warn 为 0。

聚合命令：

```bash
npm run r1:gate
```

结果：通过。它覆盖 R1 foundation、完整 M6 gate、类型检查、桌面生产构建、上下文连续性、运行时交接、深链恢复和 CAD 工作台 UI smoke。

前端组合专项门：

```bash
npm run r1:frontend-composition-gate
```

结果：通过。结构断言固定 `App.tsx` 为 21 行组合根并禁止 `useState`、`useEffect`、任务 API、定时器和 `localStorage` 回流；四条桌面 E2E 证明旧创作链、任务交接、URL 恢复和 lazy CAD route 保持工作。详细证据见 [R1_FRONTEND_COMPOSITION.md](R1_FRONTEND_COMPOSITION.md)。

Job 查询/动作/恢复提取后补充运行：

```bash
npm run agent:p0-job-history-search-smoke
npm run agent:p0-job-actions-smoke
npm run agent:p0-runtime-recovery-smoke
```

结果：全部通过。搜索/分页、action audit、cancel/retry、event cursor、provider task/checkpoint 和重启恢复保持原合同。

Provider cancel 与异步 worker 补充运行：

```bash
npm run agent:p0-async-generate3d-worker-smoke
npm run agent:p0-generate3d-worker-loop-smoke
```

结果：全部通过；Job command 提取后 provider task 取消/恢复与异步 3D worker 提交仍保持原行为。

Asset Repository/上传提取后补充运行：

```bash
npm run agent:m2-smoke
npm run agent:m4-patch-smoke
npm run agent:m4-patch-http-smoke
```

结果：全部通过；内容寻址资产创建、mask/manifest 校验、上传幂等和 Patch 追加版本保持原合同。

Create Weapon Provider workflow 提取后补充运行：

```bash
npm run r1:create-weapon-gate
```

结果：通过。结构门确认 facade 不再包含 `plan_weapon_spec`、`generate_concept`、资产写入或事件编排；行为门覆盖默认 Provider、幂等 replay/409 conflict、LLM adapter、ComfyUI 首次成功/重试和可解析 GLB/质量报告，仍产生 7 个 JobEvent 与 11 个内容寻址资产。

Generate-3D 入口提取后补充运行：

```bash
npm run r1:generate3d-gate
```

结果：通过。结构门确认 sync/queue facade 只代理；行为门覆盖同步 HTTP、异步排队、恢复、Provider submit/poll/fetch/cancel、常驻 worker、Local HTTP 3D Provider 和本地 runtime wrapper，且父版本不覆盖、幂等 replay/409、ProviderTask、Checkpoint 与内容寻址资产合同不变。

Worker Runtime 提取后补充运行：

```bash
npm run r1:worker-gate
```

结果：通过。结构门确认 `run_worker_once` facade 只代理；行为门覆盖 claim/lease、等待 Provider 重入、唯一 commit、取消抑制、Unity worker dispatch、runtime recovery 与 JobAction 协作。

Unity Export 提取后补充运行：

```bash
npm run r1:unity-export-gate
```

结果：通过。同步/排队/worker、幂等 replay/409、相对 ZIP 路径、Manifest hash、6 项 package preflight 和资产 reveal 保持；本机未配置 Unity executable，因此真实 batchmode import 仍是明确环境阻塞。

Patch 提取后补充运行：

```bash
npm run r1:patch-gate
```

结果：通过。Store/HTTP/ComfyUI、mask 空白/尺寸、manifest 引用、幂等、追加版本、ProviderTask、Checkpoint 与质量报告合同保持；AST facade 审计确认所有完整 legacy workflows 已迁出。

Library/Version 提取后补充运行：

```bash
npm run agent:p1-asset-reveal-smoke
npm run desktop:p1-deeplink-smoke
npm run desktop:p0-context-continuity-smoke
```

结果：全部通过；版本激活、资产 SHA/路径验证、Finder reveal dry-run、版本深链和 Library 同步保持原合同。

## 当前边界之外

- `asset_store.py` 仍保留共享资产/模型质量/事件写入 helper，可继续下沉为 repository/adapter，但已不承担完整业务 workflow；
- `useLegacyAppController` 仍是 413 行 legacy 应用控制器；它已经与组合根、路由监听、持久化、选择器和渲染分离，后续可按 Job/Selection 继续拆分，但不阻塞当前 R1 退出条件；
- R2–R6 的正式资产、AI 指标、完整质量规则、Beta 和发布仍必须使用各阶段证据，不因 R1 完成而自动完成。
