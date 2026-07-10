# R1 通用基础设施证据

日期：2026-07-10
状态：进行中，本文只证明首个基础设施切片，不代表 R1 完成。

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

- 8 个旧 migrations 在新库首次应用；
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

结果：通过。Vite 报告旧 `Preview3DPanel` chunk 超过 500 kB；这是 R4 查看器替换前的已知旧前端负债，不影响本次 R1 correctness。

```bash
npm run desktop:p0-context-continuity-smoke
```

结果：通过。旧创建、Patch、3D、Unity、版本切换和资产库同步链路在 Provider/AppShell 重构后保持连续。

聚合命令：

```bash
npm run r1:gate
```

结果：通过。它覆盖 R1 foundation smoke、完整 M6 gate、桌面生产构建和上下文连续性 UI smoke。

## 尚未完成

- `asset_store.py` 仍包含大量领域 SQL、Provider 和工作流；
- Job query/action/event Repository 尚未完整提取；
- `App.tsx` 仍包含旧任务恢复、通知和多页面业务组合；
- 尚未进入 R2 新 CAD 合同和数据库。
