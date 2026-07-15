# ADR-0012：G824 布尔内核 benchmark 暂不采用候选

- 状态：Accepted（拒绝采用；G825 blocked）
- 日期：2026-07-15
- 决策者：项目维护者
- 补充：ADR-0011 第 4 项的生产内核选择门

## 背景

现有 Worker 的 `union/subtract` 是明确受限的兼容实现：重叠 union 会拒绝，subtract 只处理一个轴对齐 box 减去一个贯穿 Y/Z 的 box。G825 需要稳健闭合网格布尔、不可变 feature node、材质/表面 provenance、取消和真实 GLB readback，不能仅凭 Manifold 的官方目标或单个成功案例直接接入。

G824 在临时目录固定比较：

- 当前 `ShapeProgramRuntimeManifest@1` handler；
- `manifold3d==3.5.2`，上游 tag `v3.5.2` / commit `11235e6b8ebea2dbed8aec4285685aafd3d95667`；
- `manifold-3d@3.5.1`，npm `gitHead=cc8a7f66d7d5a560da94346258c5b546af27811e`。

两种 Manifold 分发均为 Apache-2.0，包内有 LICENSE、没有独立 NOTICE。完整机器、命令、包增量、冷启动、峰值内存、四领域 fixture、coplanar/near-degenerate 和确定性结果见 `evaluations/csg-g824/report.json`。

## 决策

本轮不选择生产 CSG 内核，G825 保持 `blocked`。这不是否定 Manifold，而是当前证据尚未满足生产门：

1. Python 与 WASM 在 macOS arm64 上均完成四领域 box union/subtract、coplanar 和 near-degenerate fixture，且重复输出 hash 一致；两种绑定对相同 fixture 产生相同 mesh hash。
2. WASM 隔离包约 2.76 MB，Python + 本轮所需 NumPy 隔离目录约 36.94 MB。体积与性能只代表本机 benchmark，不代表 packaged sidecar 的最终增量。
3. 未验证 ForgeCAD `material_id → surface → part/zone` provenance 穿过布尔、重索引和 GLB readback后仍稳定；上游 mesh 字段存在不等于本产品合同通过。
4. 未验证 operation 级取消、超时回收和无部分结果；没有可接受的稳定错误码映射。
5. PyPI 提供 Windows wheel，WASM 理论上跨平台，但本轮没有 Windows 实机 runtime/packaged 验证，不能标为通过。
6. 当前兼容 handler 不具备稳健 CSG 范围，也不能被选中。

## G825 解除条件

只有以下证据全部补齐后，才新增 superseding ADR 并将 G825 改为 ready：

- 在隔离 adapter 中为每个输入 face/triangle 写入不同 source/material/zone 标识，证明 union、subtract、coplanar、near-degenerate、优化与 GLB 写出后可回读稳定 provenance；
- Worker/线程边界证明取消和超时会回收候选且不留下 partial GLB、Snapshot、Version 或缓存头；
- Windows x64 packaged sidecar 实机运行相同固定 fixture，并与 macOS arm64 比较可接受的 topology/provenance 事实；
- 记录唯一候选的包体预算、冷启动/峰值内存预算、稳定错误码和许可证/SBOM 集成；
- 由新 ADR 只选择 Python 或 WASM 一种生产实现，不能双默认或静默 fallback。

## 回滚与移除

本任务没有修改生产依赖、锁文件、runtime manifest 或 Worker handler。Python 候选只存在于临时 target 目录，WASM 候选只存在于解包的 npm tarball；删除临时目录即可完全移除。提交的 benchmark adapter、JSON 报告和本 ADR 是研究证据，可独立删除而不影响现有 G805 行为。

## 后果

- G824 可以完成，因为已经形成可复现比较与明确的“不采用”决策。
- G825 必须保持 blocked；不得为了推进计划而硬选候选。
- 当前受限 box CSG 继续作为兼容能力，但文档不得称其为稳健通用布尔。
- 后续仍可推进不依赖稳健 CSG 的任务；依赖 G825 的 G826/A003 主链需等待解除条件。

## 2026-07-15 G824A 补充证据

`evaluations/csg-g824a/report.json` 已缩小第 3、4 项证据缺口，但不改变“不采用”决策：

- Python/WASM 输入分别写入 source/material/zone property channel；四领域 union/subtract、coplanar 结果在 `simplify` 后仍可通过 original ID、face ID 与 backside 追踪。五组有效结果按 provenance 分 primitive 写入临时 GLB，ForgeCAD readback 回读 triangle、material、surface range 和自定义 provenance，两个 binding 产生相同有效 GLB hash。
- near-degenerate 输出包含退化三角形时，两条路径都在 GLB 写出前以 `CSG_DEGENERATE_OUTPUT` 拒绝，不产生部分 GLB。
- 候选隔离进程的 cancel/timeout 分别映射 `CSG_CANCELLED`/`CSG_TIMEOUT`；进程被回收，不产生候选 GLB，隔离测试中的 Snapshot/Version/cache sentinel 不变。

这些事实证明候选可以承载 ForgeCAD 所需 provenance，并证明一种可回收的隔离执行边界；它们没有把候选接入真实 Worker、数据库事务、Snapshot/Version/cache 提升流程，因此不能替代 G825 的生产零副作用测试。Windows x64 packaged sidecar 仍未实机执行，唯一生产候选也仍未由 superseding ADR 选择。G825 继续 blocked。

## 2026-07-15 G824B 权威状态提升补充证据

`evaluations/csg-g824b/report.json` 使用全量迁移后的真实临时 SQLite、`ContentAddressedStore` 和 `SQLiteUnitOfWork`，补齐 G824A 未覆盖的权威状态生命周期：

- 候选子进程没有接收 SQLite 或对象库路径，只能在事务外的独立 staging 目录生成 GLB；
- Python/WASM 分别在 kernel running cancel、kernel running timeout、valid GLB ready before promotion 三个窗口终止，`agent_asset_versions`、head、ChangeSet、Snapshot、quality、import、idempotency 与对象库 fingerprint 全部不变；
- ready-before-promotion 窗口已存在 hash 与 ForgeCAD readback 通过的完整 GLB，终止后仍只删除 staging，不产生 CAS 孤儿对象；
- Version/head/Snapshot 在同一真实 UnitOfWork 中注入失败时整体回滚，成功时整体提交并共同指向 v2。

因此 provenance、隔离取消和生产式权威状态零部分提升门已经有本机证据。该时点仍没有 Windows x64 packaged sidecar、唯一候选的 packaged 预算/许可证证据或 superseding ADR；后续 G824C 已补齐其中的 macOS packaged 证据并建议 Python，但没有改变本 ADR 的“不正式采用”结论。

## 2026-07-15 G824C macOS packaged candidate 补充证据

`evaluations/csg-g824c/report.json` 已在隔离临时目录使用当前 `sidecar_entry.py` 实际构建并启动含 `manifold3d==3.5.2` 与 NumPy 的 PyInstaller onefile sidecar：

- 当前基线为 19,445,536 bytes，候选为 24,207,728 bytes，增量 4,762,192 bytes，低于 48 MiB 总包体和 28 MiB 增量预算；
- 同轮基线/候选冷启动为 18,250.329/19,243.281 ms，相对回归 992.951 ms，低于 5 秒相对预算；候选完整进程树峰值 RSS 为 87,376 KiB，低于 300 MiB；
- archive 检查、runtime hook 强制 import、arm64 Mach-O 与真实 `/api/health` 均通过；PyInstaller 对隔离 target 中的 NumPy 需要显式 hidden import `numpy._core._exceptions`；
- `manifold3d` 的 Apache-2.0 和 NumPy 的 BSD-3-Clause/捆绑许可证文件均记录版本与 SHA-256；本轮没有 Provider 调用，也没有覆盖仓库 sidecar 或改变生产依赖；
- WASM 候选虽约 2.76 MB，但现有权威执行边界是 Python sidecar。采用 WASM 将要求新增第二个 JS/WASM host 或把权威几何移入 WebView，二者都不是已采用架构。

基于已完成的 macOS provenance、生命周期与 packaged 证据，G824C 推荐 `manifold_python` 作为后续唯一候选，但推荐状态是 `recommended_pending_windows_runtime`，不是正式采用。ADR-0012 仍有效：只有 Windows x64 packaged sidecar 运行同一 provenance/lifecycle fixture 后，才能由 superseding ADR 正式选择 Python 并允许 G825 修改生产依赖。

## 2026-07-15 G824D Windows 证据执行器状态

仓库已新增 `g824d_packaged_candidate_runtime_hook.py`、`benchmark_g824d_windows_packaged_candidate.py` 和独立 `windows-2022` CI job。该 runner 构建当前 sidecar 入口，并要求 frozen executable 自身执行六组 provenance/readback、near-degenerate 写出前拒绝和三个候选中断窗口；外层真实临时 SQLite、对象库和 UnitOfWork 再验证零权威提升与原子回滚/提交。报告只允许记录机器、固定版本、hash、readback 和生命周期事实，Provider 配置会被移除。

这只是可执行验证路径，不是 Windows 已通过证据。当前工作区没有真实 Windows x64 artifact，因此 G824D 保持 `in_progress`，本 ADR 仍不采用候选。只有 artifact 经 `check_g824d_windows_packaged_candidate.py` 通过后，才能追加平台结论并起草 superseding ADR。
