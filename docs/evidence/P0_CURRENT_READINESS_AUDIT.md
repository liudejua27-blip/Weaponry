# P0 当前发布证据审计（非最终发布审计）

日期：2026-07-11

## 结论

当前仓库的 P0 技术链已完成多条可重复纵向验证，但 **不具备 Beta 或发布资格**。本文件将 C01–C10 逐项映射到当前命令和证据，避免将 fixture、待审 Blender candidate、deterministic Planner 或 Vite 浏览器测试误写成正式发布证据。

本审计不替代最终 C01–C10 发布审计；正式资产、真实 Provider 与干净机打包验证到位后，必须重新执行并补充各自工件 hash、平台、失败样本和环境记录。

## 本次复验命令

```bash
npm run r2:gate
npm run r3:workbench-gate
npm run r3:change-set-audit-gate
npm run r4:planner-gate
npm run r5:quality-gate
npm run r5:presentation-gate
npm run agent:r4-evaluation-preflight
npm run release:packaging-readiness
```

本机已复验的 R2、R4、R5 技术门均通过；Vite build 仍提示单个 `GLTFLoader` chunk 大于 500 kB，属于性能优化 warning，不是此次技术门失败。Provider 预检与 packaging readiness 预期未就绪，详见 C08/C10。

## C01–C10 状态

| Gate | 当前技术证据 | 当前结论 | 发布仍需补齐 |
| --- | --- | --- | --- |
| C01 Contracts | `r2:contracts-gate` 通过：9 个 Concept 合同、Python/TS 生成物、OpenAPI、unknown-field 等负例 | 技术门通过 | 最终发布审计时重跑并归档版本/hash |
| C02 Database | `r2:gate` 通过：16 migrations、Project/Version、Module registry、ChangeSet、FK/legacy 依赖负例 | 技术门通过 | 在发布安装包与用户 Library 上重跑恢复链 |
| C03 Module Assets | reference Pack 的 release 校验、导入、幂等和重启通过；十模块 Blender candidate 已导入/DCC 往返 | 发布证据不完整 | 最终许可证、独立 reviewer、`formal_release_10_12` promotion report |
| C04 Connectors | 合成 100/100、参考工作台、candidate 2/2 front replacement 与单节点镜像矩阵均可复验 | 发布证据不完整 | 审核通过的 10–12 模块替换/镜像矩阵 ≥95%；修复或禁止 candidate 中质量 warning 分支 |
| C05 Viewport | 桌面浏览器 smoke：选择、隐藏、聚焦、overlay、爆炸、20 轮 lifecycle、1 canvas/1 context 通过 | Vite/浏览器技术门通过 | 正式资产和已打包 Tauri 的性能/资源证据 |
| C06 ChangeSet | ghost preview、lock、confirm/reject/stale diagnostic、immutable Version、Undo/Redo、时间线与重启回读通过 | 技术门通过 | 真实 Provider 达标后以正式资产回归 |
| C07 Quality | `r5:quality-gate` 通过：`weapon-concept-geometry/1.3` 覆盖网格、LOD、密度、预算、对称、Connector、BVH/SAT/containment 与定位 | 规则链通过 | 正式 Blender truth set 的误报/漏报、耗时和内存；不把 warning 组合当通过 |
| C08 Jobs | JobEvent@2、idempotency、cancel/retry/recovery 既有链路存在；Concept 主链当前仍是同步记录 completed job | 未完成 | 按计划将 Concept jobs worker 化，补取消、重试、partial success 与 readiness；真实 Provider 评测先完成 |
| C09 Export | `r5:presentation-gate` 通过：GLB/OBJ/MTL/PNG/views/turntable/MP4、hash、direct download、restart 与桌面下载 | 技术门通过 | 最终批准 Blender 资产的整套导出、DCC 与纹理交换性能证据 |
| C10 Desktop E2E | packaging smoke 证明空 sidecar 会被拒绝；readiness 检出四个 target sidecar 均为空/不可执行/无有效平台头 | 阻断 | 冻结 Agent sidecar、Cargo/Rust、签名、安装/卸载与干净机 Brief→Export→restart 验证 |

## 候选资产的明确限制

十模块 Blender visual candidate 可用于链路验证，当前证据包括导入、9 节点 Graph、DCC 往返、展示导出、恢复演练和 Connector matrix。它固定为 `unclassified`，`formal_asset_evidence_eligible=false`。

独立镜像矩阵中，grip/top/side/armor 质量通过；front/rear/lower/storage 会出现 `assembly.unconnected_triangle_intersection` warning。Connector 对齐、Version 与 Export 成功不表示组合无相交或可交付。

## 进入最终发布审计的顺序

1. 提供最终许可证的 10–12 个 Blender 模块，保持稳定 Module/Asset/Connector 合同。
2. 由非作者 reviewer 完成 `FormalModuleReview@1`，生成 `formal_release_10_12` promotion report。
3. 对正式资产运行 Connector ≥95%、质量 truth set、展示/DCC、三轮恢复及代表性用户 Library 演练。
4. 在明确授权的真实 Provider 上执行 80 次 live R4 truth set，确认质量阈值、latency 和 token。
5. worker 化 Concept jobs，完成 C08 的取消、重试、partial success、readiness 证据。
6. 在发布机替换 sidecar，完成签名、安装/卸载和干净机 C10。
7. 使用上述真实工件重新执行全量 C01–C10，再清理 legacy 生产入口。

## 关联证据

- [R3 Concept Workbench](R3_CONCEPT_WORKBENCH.md)
- [R3 Formal Module Review](R3_FORMAL_MODULE_REVIEW.md)
- [R3 Library Recovery Drill](R3_LIBRARY_RECOVERY_DRILL.md)
- [R4 Planner Evaluation](R4_PLANNER_EVALUATION.md)
- [R5 Presentation Delivery](R5_PRESENTATION_DELIVERY.md)
- [R6 Packaging Readiness](R6_PACKAGING_READINESS.md)
