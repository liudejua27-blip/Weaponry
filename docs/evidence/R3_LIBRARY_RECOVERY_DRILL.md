# R3 Library Recovery Drill Evidence

日期：2026-07-10

## 范围

证明同一套生产 `backup → verify → restore` 能在 10 模块 Arctic Patrol S1 reference Library 上重复运行，形成 `ForgeCADLibraryRecoveryDrillReport@1`，并由恢复后的真实 Agent 回读 Project、Version、Module registry 和全部 Module GLB。该证据是 reference fixture 基线，不是人工 Blender 最终资产或代表性用户库结论。

## 自动证据

```bash
npm run agent:r3-library-recovery-drill-smoke
npm run r3:library-backup-gate
```

烟测通过真实 API 导入仓库 reference Pack，形成 10 Module、17 Connector、1 Project、1 Version 和 1 个 9-node Graph，并额外放入一个未引用对象候选。随后执行两轮备份、独立验证、隔离恢复和 Agent 回读，再以首份报告作为 baseline 完成第三轮容量对比。

## 已证明

- 两轮数据库 hash、对象集合 hash、schema/table count 和 capacity 完全一致；源库写入会以 `SOURCE_CHANGED_DURING_DRILL` 阻断；
- 每轮恢复 Agent 均回读 1 Project、1 Version、10 Module，并下载全部 10 个 GLB 校验响应头和 payload SHA-256；
- 清单统计 10 个引用、10 个唯一对象和 1 个未引用候选；默认成功后删除临时 backup/restore，只留下报告；
- `--baseline-report` 记录旧报告 hash，并在未变化库上得到全部容量字段 delta=0；
- 输出位于源 Library 内或已存在目录会失败；
- 10 个 reference GLB 的 generator 被识别，`formal_blender_10_12` 申报以 `FORMAL_ASSET_EVIDENCE_REJECTED` 失败，不能把 fixture 冒充正式资产。

本机一次两轮样本约为：备份载荷 754 KB；backup+internal verify 中位数 27 ms、独立 verify 6.6 ms、restore+verify 15.8 ms、Agent 启动与完整回读 651 ms。数值仅用于确认报告链路和后续比较方法，不能作为 SLA 或正式规模结论。

另对工作区当前静止的 legacy Library 执行一轮 `unclassified` 演练：113 条引用合并为 108 个唯一对象，逻辑对象 258,168 bytes、物理对象 233,411 bytes、去重 24,757 bytes，另有 1 个 492-byte 未引用候选；备份载荷 860,099 bytes，backup+internal verify 80.7 ms、独立 verify 17.8 ms、restore+verify 62.5 ms、Agent 启动与回读 610.3 ms。该库没有 Concept Module，因此这组结果只证明现有旧数据兼容性，不是代表性用户库或正式资产证据。

## 未证明

- 人工 Blender 10–12 模块首包与代表性用户 Library 各至少三轮的正式报告；
- 峰值磁盘、峰值内存、跨机器/跨操作系统性能或长期容量趋势；
- 保留周期、reference-aware GC、自动调度、加密异地复制、WORM 或 legal hold；
- `formal_blender_10_12` 的自动资格检查不能代替人工资产质量审阅。
