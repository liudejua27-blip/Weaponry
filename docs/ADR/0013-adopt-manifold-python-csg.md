# ADR-0013：选择 Manifold Python 作为唯一生产 CSG 实现

- 状态：Accepted
- 日期：2026-07-15
- 决策者：项目维护者
- 取代：ADR-0012 的“不采用候选”结论
- 生效边界：G825 已按本 ADR 完成生产依赖、默认 handler 和不可变 Feature History 集成

## 背景

ADR-0012 要求 provenance、隔离取消、权威状态原子提升、packaged 预算/许可证和 Windows x64 实机证据全部通过后，才能只选择一个生产 CSG。G824A–G824D 已依次补齐这些证据。

Windows Actions run `29383382978` 的 `g824d-windows-packaged-candidate` job 在真实 Windows x64 frozen executable 内通过，并上传 `evaluations/csg-g824d/windows-report.json`。报告经 `check_g824d_windows_packaged_candidate.py` 独立校验：五组有效 fixture 的 provenance/GLB readback 通过，near-degenerate 在写出前稳定拒绝；cancel、timeout 和 ready-before-promotion 均回收进程且不提升 SQLite、对象库或部分 GLB；Version/head/Snapshot 的注入失败整体回滚、成功整体提交。

## 决策

G825 只允许接入 `manifold3d==3.5.2`（上游 commit `11235e6b8ebea2dbed8aec4285685aafd3d95667`）作为唯一生产 CSG 实现。

- 执行宿主继续是现有 Python sidecar，不新增 JS/WASM host，也不在 WebView 执行权威几何。
- 不允许 Python/WASM 双默认、静默 fallback 或失败后退回旧 box CSG 并冒充成功。
- G825 已修改生产 lock/SBOM、runtime handler 和不可变 feature node；只有该任务的当前 Gate 证据代表受限生产 CSG 已集成。
- 候选进程不得接收 SQLite、对象库或 Snapshot 路径；完整 GLB 只能在事务外 staging，校验通过后由单一 UnitOfWork 提升。
- `CSG_CANCELLED`、`CSG_TIMEOUT`、`CSG_DEGENERATE_OUTPUT` 与不支持操作必须显式失败，不能留下部分 GLB 或权威状态。

## 选择理由

Python 与 WASM 的几何/provenance 结果均满足隔离 benchmark，但当前权威运行时已经是 Python sidecar。macOS 候选总包体 24,207,728 bytes、相对增量 4,762,192 bytes、冷启动回归 992.951 ms、进程树峰值 87,376 KiB，均在预算内；Windows executable 为 35,788,283 bytes，健康启动 2,528.125 ms。选择 Python 避免新增第二执行宿主，同时保留同源 Manifold 内核和 Apache-2.0 许可证边界。

## 后果与回滚

- G824D 与 G825 均标记 `done`，G826 变为唯一 `ready` 任务。
- 当前产品只有受限、有界、封闭输入的 Manifold union/subtract；用户指南可描述该后端事实，但不得宣称自由或工程级通用 CSG。
- 若 G825 无法满足确定性、provenance、取消、预算或零部分提升门，移除新增生产依赖和 handler，恢复 ADR-0012 的不采用状态；不得保留隐式 fallback。
