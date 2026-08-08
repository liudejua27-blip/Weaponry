# ForgeCAD 测试策略

版本：2026-08-08
状态：目标测试体系；MCP001 删除旧 Provider/U004 Gate，MCP002 建立 Store/CAS/IPC focused Gate，MCP003 增加 MCP protocol/resources focused Gate

## 1. 证据层级

1. Schema/static/fuzz；
2. Core deterministic unit/property；
3. Store transaction/crash/recovery；
4. Worker sandbox/geometry/render；
5. Runtime integration；
6. MCP conformance 和真实 Codex 宿主；
7. Viewer browser + packaged WebView/GPU；
8. 完整 reference→candidate→approval→version→restore→export；
9. 安装/升级/回滚/灾难恢复；
10. 跨类别独立真人质量。

低层通过不能替代高层；每层分别标 PASS/FAIL/BLOCKED/NOT_RUN。

## 2. CI Gate

- contracts generation/check、unknown/oversize/adversarial；
- Rust fmt/clippy/unit/integration；
- SQLite single-writer、kill、disk-full、WAL、migration、backup/restore；
- geometry/appearance/readback/GLB validator；
- Skill DAG/operator/budget/signature/SBOM/license/revocation；
- MCP tools/resources/schema/annotations/errors/idempotency/timeout；
- Codex Desktop/CLI/IDE smoke（真实发布版本）；
- Viewer typecheck/build/E2E、单 renderer、a11y、尺寸、GPU fallback；
- packaging/notarization/install/upgrade；
- security/secret/path/content-scope；
- visual benchmark/human gate。

## 3. 强制失败路径

stale base、重复 idempotency、hash mismatch、approval reject/expire、quality hard fail、attachment symlink/越权/炸弹、unknown Skill Operator、DAG cycle、Worker timeout/crash/late result、MCP/Viewer/Runtime kill、disk full、CAS corrupt、renderer unavailable、license/signature revocation。

任何失败不得创建永久版本或泄露内部数据。

## 4. 3D Benchmark

数据集按类别、视图数、材质、几何表示和难度分层；机械只能是一类。每条保存授权参考、target claims、RenderSet/AOV、readback、QualityReport、timing/memory、Codex review 和盲评。报告展示每类失败和最差分位，不只展示平均分。

结构指标与视觉指标分开：manifold/UV/PBR/GLB 绿色不等于参考相似；视觉好看也不等于可编辑、版本和导出正确。

## 5. 真实 Codex Gate

自写 MCP client、fixture 或手工复制附件不能替代 Codex。每个受支持宿主需证明 tools/resources 发现、附件原始字节、write approval、长 Job、cancel、重启和 packaged Viewer 同一 hash。

## 6. Evidence manifest

每个任务 evidence 包含环境、commit/worktree、命令/exit code、合同/二进制/资产 hash、原始 artifacts、日志脱敏证明、未运行项和 blocker。Markdown 总结不替代机器收据。
