# 执行证据索引

> 本目录记录当前 Weapon Concept 兼容基线和既有发布门的历史/现状证据。产品已由 ADR-0008 升级为通用机械概念 3D Agent；G2–G6 的通用合同、ShapeProgram、12 个轻量 blockout、分件候选、视觉材质、AgentComponent 注册/替换、Connector 对齐、GLB readback 和资产级 ChangeSet，以及 G6.5 的外部 GLB 安全参考导入，均有独立 smoke/工作台证据。真实碰撞/运动学、外部 GLB 自动重建/深度分件、Provider truth set 和 packaged sidecar 仍未完成。

本目录是历史与审计证据，不是用户手册、当前 API 或目标设计。后续 Codex 默认不应递归加载全部 evidence；先读 [文档地图](../DOCUMENTATION_MAP.md) 和 [能力—Gate 矩阵](CAPABILITY_GATE_MATRIX.md)，只在追溯具体 Gate 时打开对应证据。

- [仓库完整性与 CI 基线](REPOSITORY_INTEGRITY.md)
- [能力与自动化 Gate 矩阵](CAPABILITY_GATE_MATRIX.md)
- [Codex 当前交接](../CODEX_HANDOFF.md)
- [Codex 执行总计划](../CODEX_EXECUTION_PLAN.md)
- [Codex 原子任务索引](../CODEX_TASK_INDEX.md)

- [R0 基线冻结](R0_BASELINE.md)
- [R1 通用基础设施](R1_FOUNDATION.md)
- [R3 ChangeSet 审计归档](R3_CHANGE_SET_AUDIT.md)
- [R3 Library 备份与恢复](R3_LIBRARY_BACKUP_RESTORE.md)
- [R3 Library 恢复演练与参考容量基线](R3_LIBRARY_RECOVERY_DRILL.md)
- [R3 Blender Authoring Starter 真实构建](R3_BLENDER_STARTER.md)
- [R3 正式 Blender 模块人工审阅门](R3_FORMAL_MODULE_REVIEW.md)
- [R3 正式资产原创许可证声明](R3_FORMAL_ASSET_LICENSE_DECLARATION.md)
- [R3 首次运行闭环与视口性能优化](R3_FIRST_RUN_AND_VIEWPORT_OPTIMIZATION.md)
- [R3 组件资产目录与审阅状态](R3_ASSET_CATALOG.md)
- [R5 工作台 combined GLB Blender 往返](R5_PRESENTATION_DELIVERY.md)
- [R6 Tauri 打包完整性门](R6_PACKAGING_READINESS.md)
- [P0 当前 C01–C10 发布证据审计](P0_CURRENT_READINESS_AUDIT.md)

证据文件记录命令、结果、覆盖范围和未完成项。它们不能替代对应阶段的完整退出审计。
